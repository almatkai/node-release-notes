mod cache;
pub mod http;
pub mod model;
mod ui;
use http::{Http, Transport};
use model::{Release, Version};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tezbar_extension_sdk::{BoxFuture, CoreResult, Extension, ExtensionContext};
use tokio::sync::Notify;
pub const ID: &str = "com.tezbar.native.node-release-notes";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FEED: &str = "https://nodejs.org/dist/index.json";
#[derive(Clone, Default, Serialize, Deserialize)]
struct Session {
    rows: Vec<Version>,
    filter: String,
    query: String,
    selected: Option<usize>,
    release: Option<Release>,
    error: Option<String>,
    cache_warning: Option<String>,
}
#[derive(Clone)]
enum Effect {
    Copy(String),
    Open(String),
}
impl Effect {
    fn title(&self) -> &'static str {
        match self {
            Self::Copy(_) => "Copy",
            Self::Open(_) => "Open",
        }
    }
    async fn perform(&self, ctx: &ExtensionContext) -> Result<(), String> {
        let (method, params) = match self {
            Self::Copy(text) => ("host.clipboard.write", json!({"text":text})),
            Self::Open(target) => ("host.shell.open", json!({"target":target})),
        };
        validate_ack(
            &ctx.host(method, params).await?,
            matches!(self, Self::Open(_)),
        )
    }
}
/// Actual committed host shapes, never permissive truthiness.
pub fn validate_ack(value: &Value, open: bool) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Err("Malformed host acknowledgement".into());
    };
    if object.get("ok") != Some(&Value::Bool(true))
        || object.contains_key("error")
        || object
            .keys()
            .any(|k| k != "ok" && !(open && k == "message"))
        || object.get("message").is_some_and(|v| !v.is_string())
    {
        return Err("Host action failed or acknowledgement is malformed".into());
    }
    Ok(())
}
#[derive(Clone)]
enum Action {
    Filter,
    Detail(usize),
    Back,
    FetchFeed,
    FetchDetail,
    Effect(Effect),
    Retry,
    Dismiss,
}
#[derive(Default)]
struct Gate {
    epoch: u64,
    actions: Vec<(String, Action)>,
    pending: Option<Effect>,
    warning: Option<String>,
    recover: Arc<Notify>,
}
type Authorities = Arc<Mutex<HashMap<String, Arc<Mutex<Gate>>>>>;
struct Guard {
    map: Weak<Mutex<HashMap<String, Arc<Mutex<Gate>>>>>,
    id: String,
    gate: Weak<Mutex<Gate>>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let (Some(map), Some(gate)) = (self.map.upgrade(), self.gate.upgrade()) {
            if let Ok(mut map) = map.lock() {
                if map.get(&self.id).is_some_and(|v| Arc::ptr_eq(v, &gate)) {
                    map.remove(&self.id);
                }
            }
        }
    }
}
// On SDK cancellation the staged Value is discarded, while its consumed IDs
// and prepared OS effect remain authoritative. Ask the existing host refresh
// path to recover that view; this guard never replays an action.
struct ActionRecovery {
    notify: Arc<Notify>,
    complete: bool,
}
impl Drop for ActionRecovery {
    fn drop(&mut self) {
        if !self.complete {
            self.notify.notify_one();
        }
    }
}
pub struct Adapter {
    transport: Arc<dyn Transport>,
    authorities: Authorities,
    cache_lane: Mutex<()>,
}
impl Adapter {
    pub fn production() -> Result<Self, String> {
        Ok(Self::with_transport(Arc::new(Http::new()?)))
    }
    /// Tests/embedders only; production has no endpoint override.
    pub fn with_transport(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
            authorities: Arc::default(),
            cache_lane: Mutex::default(),
        }
    }
    fn register(&self, ctx: &ExtensionContext, id: &str) -> Result<Arc<Mutex<Gate>>, String> {
        let gate = Arc::new(Mutex::new(Gate::default()));
        {
            let mut map = self.authorities.lock().map_err(|e| e.to_string())?;
            if map.len() >= 128 || map.contains_key(id) {
                return Err("Session registration full/duplicate".into());
            }
            map.insert(id.into(), gate.clone());
        }
        let guard = Guard {
            map: Arc::downgrade(&self.authorities),
            id: id.into(),
            gate: Arc::downgrade(&gate),
        };
        // Construct before spawning so an unpolled canceled future also drops it.
        let recover = gate.lock().unwrap().recover.clone();
        let notifier = ctx.clone();
        ctx.spawn(async move {
            let _guard = guard;
            loop {
                recover.notified().await;
                // Event driven, no timer polling. Yield past cancellation's
                // synchronous state-lock cleanup before emitting one refresh.
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                if notifier.invalidate().is_err() {
                    break;
                }
            }
        })?;
        Ok(gate)
    }
    fn gate(&self, id: &str) -> Result<Arc<Mutex<Gate>>, String> {
        self.authorities
            .lock()
            .map_err(|e| e.to_string())?
            .get(id)
            .cloned()
            .ok_or("Session registration expired".into())
    }
    fn update_cache(
        &self,
        ctx: &ExtensionContext,
        s: &mut Session,
        change: impl FnOnce(&mut cache::Cache),
    ) {
        // Re-read/merge after the network await. Independent sessions must not
        // overwrite a cache snapshot captured before another request completed.
        let _lane = self.cache_lane.lock().unwrap();
        let mut cache = cache::Cache::read(ctx).unwrap_or_else(|e| {
            s.cache_warning = Some(format!("Native cache unavailable: {e}"));
            cache::Cache::default()
        });
        change(&mut cache);
        if let Err(e) = cache.save(ctx) {
            s.cache_warning = Some(format!("Native cache write failed: {e}"));
        }
    }
    async fn feed(&self, ctx: &ExtensionContext, s: &mut Session) {
        s.cache_warning = None;
        let cache = cache::Cache::read(ctx).unwrap_or_else(|e| {
            s.cache_warning = Some(format!("Native cache unavailable: {e}"));
            cache::Cache::default()
        });
        if s.rows.is_empty() {
            s.rows = cache.feed;
        }
        match self.transport.get(FEED).await.and_then(model::versions) {
            Ok(rows) => {
                s.rows = rows;
                s.error = None;
                let rows = s.rows.clone();
                self.update_cache(ctx, s, move |c| c.feed = rows);
            }
            Err(error) => s.error = Some(error),
        }
    }
    async fn detail(&self, ctx: &ExtensionContext, s: &mut Session) {
        s.cache_warning = None;
        let cache = cache::Cache::read(ctx).unwrap_or_else(|e| {
            s.cache_warning = Some(format!("Native cache unavailable: {e}"));
            cache::Cache::default()
        });
        let version = s.rows[s.selected.expect("selected detail")].version.clone();
        s.release = cache.detail(&version);
        let result = match model::safe_version(&version) {
            Err(e) => Err(e),
            Ok(()) => self
                .transport
                .get(&format!(
                    "https://api.github.com/repos/nodejs/node/releases/tags/{version}"
                ))
                .await
                .and_then(model::release),
        };
        match result {
            Ok(data) => {
                s.release = Some(data.clone());
                s.error = None;
                self.update_cache(ctx, s, move |c| c.put(&version, data));
            }
            Err(error) => s.error = Some(error),
        }
    }
    async fn effect(&self, ctx: &ExtensionContext, gate: &Arc<Mutex<Gate>>, effect: Effect) {
        // SDK state rolls back on cancellation; OS effects cannot. Keep one
        // bounded authoritative checkpoint for deliberate exact-byte retry.
        {
            let mut g = gate.lock().unwrap();
            g.pending = Some(effect.clone());
            g.warning = Some(format!(
                "{} outcome uncertain. Retry may repeat the action.",
                effect.title()
            ));
        }
        match effect.perform(ctx).await {
            Ok(()) => {
                let mut g = gate.lock().unwrap();
                g.pending = None;
                g.warning = None;
            }
            Err(error) => {
                gate.lock().unwrap().warning = Some(format!(
                    "{} failed or outcome uncertain: {error}. Retry may repeat it.",
                    effect.title()
                ))
            }
        }
    }
    async fn operation(
        &self,
        ctx: &ExtensionContext,
        operation: &str,
        payload: &Value,
        state: &mut Value,
    ) -> CoreResult {
        let id = payload["sessionId"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 1024)
            .ok_or("Missing/oversized sessionId")?;
        if operation == "extension:dispose-session" {
            self.authorities.lock().unwrap().remove(id);
            return Ok(json!({}));
        }
        let (gate, mut session) = if operation == "extension:run-command" {
            let gate = self.register(ctx, id)?;
            let mut session = Session {
                filter: "latest".into(),
                query: payload["searchText"].as_str().unwrap_or("").into(),
                ..Session::default()
            };
            if session.query.len() > 1024 {
                return Err("Search exceeds 1024 bytes".into());
            }
            self.feed(ctx, &mut session).await;
            (gate, session)
        } else {
            (
                self.gate(id)?,
                serde_json::from_value::<Session>(state.clone())
                    .map_err(|_| "Invalid session state")?,
            )
        };
        let mut recovery = None;
        match operation {
            "extension:run-command" | "extension:refresh-session" => {}
            "extension:load-more" => return Ok(tezbar_extension_sdk::unchanged()),
            "extension:search-text-changed" => {
                if session.selected.is_none() {
                    let query = payload["searchText"].as_str().ok_or("Missing searchText")?;
                    if query.len() > 1024 {
                        return Err("Search exceeds 1024 bytes".into());
                    }
                    session.query = query.into();
                }
            }
            "extension:invoke-action" => {
                let action_id = payload["actionId"].as_str().ok_or("Missing actionId")?;
                let action = {
                    let mut g = gate.lock().unwrap();
                    let action = g
                        .actions
                        .iter()
                        .find(|(id, _)| id == action_id)
                        .map(|(_, a)| a.clone())
                        .ok_or("Stale/discarded action; refresh the view")?;
                    // Consume displayed epoch before any await, including when
                    // the SDK will later discard this operation's staged Value.
                    g.actions.clear();
                    recovery = Some(ActionRecovery {
                        notify: g.recover.clone(),
                        complete: false,
                    });
                    action
                };
                match action {
                    Action::Filter => {
                        let filter = payload["formValues"]["value"]
                            .as_str()
                            .ok_or("Missing filter value")?;
                        if !["latest", "lts", ""].contains(&filter) {
                            return Err("Invalid filter value".into());
                        }
                        session.filter = filter.into();
                    }
                    Action::Detail(i) => {
                        session.selected = Some(i);
                        session.release = None;
                        session.error = None;
                        self.detail(ctx, &mut session).await;
                    }
                    Action::Back => {
                        session.selected = None;
                        session.release = None;
                        session.error = None;
                    }
                    Action::FetchFeed => self.feed(ctx, &mut session).await,
                    Action::FetchDetail => self.detail(ctx, &mut session).await,
                    Action::Effect(effect) => self.effect(ctx, &gate, effect).await,
                    Action::Retry => {
                        let effect = gate
                            .lock()
                            .unwrap()
                            .pending
                            .clone()
                            .ok_or("No uncertain effect")?;
                        self.effect(ctx, &gate, effect).await;
                    }
                    Action::Dismiss => {
                        let mut g = gate.lock().unwrap();
                        g.pending = None;
                        g.warning = None;
                    }
                }
            }
            _ => return Err(format!("Unsupported operation: {operation}")),
        }
        let result = ui::render(ctx, &session, &gate)?;
        *state = serde_json::to_value(session).map_err(|e| e.to_string())?;
        if let Some(recovery) = recovery.as_mut() {
            recovery.complete = true;
        }
        Ok(result)
    }
}
impl Extension for Adapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> &'static str {
        VERSION
    }
    fn commands(&self) -> &'static [&'static str] {
        &["index"]
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        operation: &'a str,
        payload: &'a Value,
        state: &'a mut Value,
    ) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            if command != "index" {
                return Err("Unknown command".into());
            }
            self.operation(ctx, operation, payload, state).await
        })
    }
}
