use node_release_notes_native::{http::Transport, Adapter};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tezbar_extension_sdk::{BoxFuture, Extension, ExtensionPaths, TestContext};
#[derive(Default)]
struct Mock {
    replies: Mutex<VecDeque<Result<Value, String>>>,
    calls: Mutex<Vec<String>>,
}
impl Mock {
    fn reply(&self, value: Result<Value, String>) {
        self.replies.lock().unwrap().push_back(value);
    }
}
impl Transport for Mock {
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(url.into());
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("uncaptured network")
        })
    }
}
fn data(name: &str) -> Value {
    serde_json::from_str(match name {
        "feed" => include_str!("../qa/fixtures/feed.json"),
        "malicious" => include_str!("../qa/fixtures/malicious-release.json"),
        _ => include_str!("../qa/fixtures/release.json"),
    })
    .unwrap()
}
fn context(caps: &[&str]) -> (tempfile::TempDir, TestContext) {
    let work = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work/unit");
    std::fs::create_dir_all(&work).unwrap();
    let dir = tempfile::tempdir_in(&work).unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = TestContext::new(
        ExtensionPaths {
            package_dir: base.join("package/versions/verified"),
            data_dir: base.join("data"),
            temp_dir: base.join("temp"),
        },
        caps,
    );
    (dir, ctx)
}
fn action(view: &Value, title: &str) -> String {
    view["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["title"] == title)
        .unwrap_or_else(|| panic!("missing {title}: {view}"))
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .into()
}
async fn call(a: &Adapter, c: &TestContext, state: &mut Value, op: &str, payload: Value) -> Value {
    let mut p = payload;
    p["sessionId"] = json!("s");
    // Model SDK staging explicitly; production pipes separately test actual runner.
    let mut stage = state.clone();
    let result = c
        .run(a.handle(&c.context, "index", op, &p, &mut stage))
        .await
        .unwrap();
    *state = stage;
    result
}
async fn invoke(a: &Adapter, c: &TestContext, s: &mut Value, v: &Value, title: &str) -> Value {
    call(
        a,
        c,
        s,
        "extension:invoke-action",
        json!({"actionId":action(v,title)}),
    )
    .await
}
#[tokio::test]
async fn production_adapter_complete_flow_no_http_on_search_heartbeat_load_more_or_effect() {
    let (_dir, c) = context(&["clipboard.write", "shell.open"]);
    let m = Arc::new(Mock::default());
    m.reply(Ok(data("feed")));
    let a = Adapter::with_transport(m.clone());
    let mut s = json!({});
    let mut v = call(&a, &c, &mut s, "extension:run-command", json!({})).await;
    assert_eq!(v["refreshMs"], Value::Null);
    assert_eq!(
        v["root"]["children"][0]["children"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(v["root"]["children"][0]["children"][0]["props"]["icon"]
        .as_str()
        .unwrap()
        .contains("/versions/verified/resources/nodejs-icon.svg"));
    let changed=call(&a,&c,&mut s,"extension:invoke-action",json!({"actionId":v["root"]["props"]["searchBarAccessory"]["props"]["actionId"].as_str().unwrap(),"formValues":{"value":""}})).await;
    assert_eq!(
        changed["root"]["props"]["searchBarAccessory"]["props"]["value"],
        ""
    );
    v = call(
        &a,
        &c,
        &mut s,
        "extension:search-text-changed",
        json!({"searchText":"Current"}),
    )
    .await;
    assert_eq!(s["query"], "Current");
    m.reply(Ok(data("release")));
    v = invoke(&a, &c, &mut s, &v, "Show Release Notes for v24.1.0").await;
    assert_eq!(v["root"]["type"], "Detail");
    let titles: Vec<_> = v["actions"]
        .as_array()
        .unwrap()
        .iter()
        .take(5)
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec![
            "Open Release Notes",
            "Open Docs",
            "Copy Version",
            "Copy Release Notes URL",
            "Copy Docs URL"
        ]
    );
    assert_eq!(
        v["root"]["metadata"]["children"][0]["props"]["text"],
        "1/1/2025, 11:59:58 PM"
    );
    for title in [
        "Open Release Notes",
        "Open Docs",
        "Copy Version",
        "Copy Release Notes URL",
        "Copy Docs URL",
    ] {
        c.reply(Ok(if title.starts_with("Open") {
            json!({"ok":true,"message":"Opened"})
        } else {
            json!({"ok":true})
        }));
        v = invoke(&a, &c, &mut s, &v, title).await;
        assert!(v.get("message").is_none());
    }
    assert_eq!(c.calls()[0]["method"], "host.shell.open");
    assert_eq!(c.calls()[2]["params"], json!({"text":"v24.1.0"}));
    let stale = action(&v, "Copy Version");
    v = call(&a, &c, &mut s, "extension:refresh-session", json!({})).await;
    assert!(c
        .run(a.handle(
            &c.context,
            "index",
            "extension:invoke-action",
            &json!({"sessionId":"s","actionId":stale}),
            &mut s
        ))
        .await
        .is_err());
    assert_eq!(
        call(&a, &c, &mut s, "extension:load-more", json!({})).await["mode"],
        "unchanged"
    );
    v = invoke(&a, &c, &mut s, &v, "Back").await;
    assert_eq!(s["filter"], "");
    assert_eq!(s["query"], "Current");
    assert_eq!(v["root"]["props"]["searchText"], "Current");
    assert_eq!(m.calls.lock().unwrap().len(), 2);
    assert_eq!(c.calls().len(), 5);
}
#[tokio::test]
async fn errors_deliberate_retry_and_exact_effect_bytes() {
    let (_dir, c) = context(&["clipboard.write", "shell.open"]);
    let m = Arc::new(Mock::default());
    m.reply(Err("Offline".into()));
    let a = Adapter::with_transport(m.clone());
    let mut s = json!({});
    let mut v = call(&a, &c, &mut s, "extension:run-command", json!({})).await;
    assert_eq!(v["message"], "Offline");
    assert_eq!(s["rows"], json!([]));
    v = call(&a, &c, &mut s, "extension:refresh-session", json!({})).await;
    assert_eq!(m.calls.lock().unwrap().len(), 1);
    m.reply(Ok(data("feed")));
    v = invoke(&a, &c, &mut s, &v, "Retry Node Versions").await;
    m.reply(Err("Not Found".into()));
    v = invoke(&a, &c, &mut s, &v, "Show Release Notes for v24.1.0").await;
    assert!(v["root"]["props"]["markdown"]
        .as_str()
        .unwrap()
        .contains("Not Found"));
    assert!(v["root"].get("metadata").is_none());
    m.reply(Ok(data("release")));
    v = invoke(&a, &c, &mut s, &v, "Retry Release Notes").await;
    for reply in [
        json!(null),
        json!(true),
        json!({"ok":true,"error":"contradiction"}),
    ] {
        c.reply(Ok(reply));
        v = invoke(&a, &c, &mut s, &v, "Open Docs").await;
        assert!(v["message"].as_str().unwrap().contains("uncertain"));
        let before = c.calls().last().unwrap()["params"].clone();
        let count = m.calls.lock().unwrap().len();
        c.reply(Ok(json!({"ok":true,"message":"Opened"})));
        v = invoke(&a, &c, &mut s, &v, "Retry Open (may repeat)").await;
        assert_eq!(c.calls().last().unwrap()["params"], before);
        assert_eq!(m.calls.lock().unwrap().len(), count);
    }
    c.reply(Err("copy failed".into()));
    v = invoke(&a, &c, &mut s, &v, "Copy Version").await;
    c.reply(Ok(json!({"ok":true})));
    v = invoke(&a, &c, &mut s, &v, "Retry Copy (may repeat)").await;
    assert!(v.get("message").is_none());
    assert_eq!(
        c.calls().last().unwrap()["params"],
        json!({"text":"v24.1.0"})
    );
}
#[tokio::test]
async fn capability_denial_and_malicious_body_are_inert_before_renderer() {
    let (_dir, c) = context(&[]);
    let m = Arc::new(Mock::default());
    m.reply(Ok(data("feed")));
    m.reply(Ok(data("malicious")));
    let a = Adapter::with_transport(m.clone());
    let mut s = json!({});
    let v = call(&a, &c, &mut s, "extension:run-command", json!({})).await;
    let v = invoke(&a, &c, &mut s, &v, "Show Release Notes for v24.1.0").await;
    let markdown = v["root"]["props"]["markdown"].as_str().unwrap();
    for tag in ["<script>", "<iframe ", "<img ", "<a ", "<button "] {
        assert!(!markdown.contains(tag), "{tag}");
    }
    assert!(markdown.contains("&lt;script&gt;"));
    assert!(markdown.contains("<pre><code class=\"language-js\">"));
    let v = invoke(&a, &c, &mut s, &v, "Copy Version").await;
    assert!(v["message"].as_str().unwrap().contains("not granted"));
    assert!(c.calls().is_empty());
}
#[tokio::test]
async fn persistent_cache_keeps_previous_data_on_failed_revalidation_without_polling() {
    let (_dir, c) = context(&[]);
    let m = Arc::new(Mock::default());
    let a = Adapter::with_transport(m.clone());
    let mut s = json!({});
    m.reply(Ok(data("feed")));
    let v = call(&a, &c, &mut s, "extension:run-command", json!({})).await;
    m.reply(Ok(data("release")));
    invoke(&a, &c, &mut s, &v, "Show Release Notes for v24.1.0").await;
    call(&a, &c, &mut s, "extension:dispose-session", json!({})).await;
    let mut fresh = json!({});
    m.reply(Err("Offline revalidation".into()));
    let v = call(&a, &c, &mut fresh, "extension:run-command", json!({})).await;
    assert_eq!(fresh["rows"].as_array().unwrap().len(), 7);
    assert_eq!(v["message"], "Offline revalidation");
    m.reply(Err("GitHub offline".into()));
    let v = invoke(&a, &c, &mut fresh, &v, "Show Release Notes for v24.1.0").await;
    assert!(v["root"]["props"]["markdown"]
        .as_str()
        .unwrap()
        .contains("Safe <strong>notes</strong>"));
    assert!(v["root"].get("metadata").is_some());
    assert_eq!(v["message"], "GitHub offline");
    action(&v, "Retry Release Notes");
    let count = m.calls.lock().unwrap().len();
    call(&a, &c, &mut fresh, "extension:refresh-session", json!({})).await;
    assert_eq!(m.calls.lock().unwrap().len(), count);
}
#[tokio::test]
async fn list_uncertain_copy_exposes_retry_in_each_row_actions_without_refetch() {
    let (_dir, c) = context(&["clipboard.write"]);
    let m = Arc::new(Mock::default());
    let a = Adapter::with_transport(m.clone());
    let mut s = json!({});
    m.reply(Ok(data("feed")));
    let v = call(&a, &c, &mut s, "extension:run-command", json!({})).await;
    c.reply(Err("Captured failure".into()));
    let v = invoke(&a, &c, &mut s, &v, "Copy Version").await;
    let retry = action(&v, "Retry Copy (may repeat)");
    assert!(
        v["root"]["children"][0]["children"][0]["props"]["actionIds"]
            .as_array()
            .unwrap()
            .contains(&json!(retry))
    );
    c.reply(Ok(json!({"ok":true})));
    invoke(&a, &c, &mut s, &v, "Retry Copy (may repeat)").await;
    assert_eq!(c.calls()[0]["params"], c.calls()[1]["params"]);
    assert_eq!(m.calls.lock().unwrap().len(), 1);
}
