use crate::{model, Action, Effect, Gate, Session};
use pulldown_cmark::{html, Event, Options, Parser, Tag, TagEnd};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tezbar_extension_sdk::ExtensionContext;
fn node(kind: &str, props: Value, children: Vec<Value>) -> Value {
    json!({"type":kind,"props":props,"children":children})
}
/// Raw HTML is text; arbitrary links/images are inert. Only validated metadata
/// and declared actions open URLs. Code/Markdown structure remains readable.
pub fn safe_markdown(source: &str) -> String {
    let mut destinations = Vec::new();
    let mut events = Vec::new();
    for event in Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    ) {
        match event {
            Event::Html(text) | Event::InlineHtml(text) => events.push(Event::Text(text)),
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => destinations.push(dest_url),
            Event::End(TagEnd::Link) | Event::End(TagEnd::Image) => {
                if let Some(url) = destinations.pop() {
                    events.push(Event::Text(" (".into()));
                    events.push(Event::Code(url));
                    events.push(Event::Text(")".into()));
                }
            }
            other => events.push(other),
        }
    }
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}
struct Builder<'a> {
    gate: &'a mut Gate,
    actions: Vec<Value>,
    serial: usize,
}
impl Builder<'_> {
    fn add(&mut self, title: &str, kind: &str, action: Action) -> String {
        let id = format!("n:{}:{}", self.gate.epoch, self.serial);
        self.serial += 1;
        self.gate.actions.push((id.clone(), action));
        self.actions
            .push(json!({"id":id,"title":title,"kind":kind}));
        id
    }
}
pub(crate) fn render(
    ctx: &ExtensionContext,
    s: &Session,
    gate: &Arc<Mutex<Gate>>,
) -> Result<Value, String> {
    let mut g = gate.lock().map_err(|e| e.to_string())?;
    g.epoch = g.epoch.checked_add(1).ok_or("Action epoch exhausted")?;
    g.actions.clear();
    let mut b = Builder {
        gate: &mut g,
        actions: Vec::new(),
        serial: 0,
    };
    let mut back = None;
    let mut root = if let Some(index) = s.selected {
        let row = s.rows.get(index).ok_or("Invalid selected row")?;
        if let Some(data) = &s.release {
            let docs = model::docs_url(&row.version)?;
            b.add(
                "Open Release Notes",
                "action",
                Action::Effect(Effect::Open(data.html_url.clone())),
            );
            b.add(
                "Open Docs",
                "action",
                Action::Effect(Effect::Open(docs.clone())),
            );
            b.add(
                "Copy Version",
                "action",
                Action::Effect(Effect::Copy(row.version.clone())),
            );
            b.add(
                "Copy Release Notes URL",
                "action",
                Action::Effect(Effect::Copy(data.html_url.clone())),
            );
            b.add(
                "Copy Docs URL",
                "action",
                Action::Effect(Effect::Copy(docs)),
            );
        }
        if s.error.is_some() {
            b.add("Retry Release Notes", "action", Action::FetchDetail);
        }
        back = Some(b.add("Back", "pop", Action::Back));
        let markdown = model::detail_markdown(&row.version, s.release.as_ref(), s.error.as_deref());
        let mut root = node(
            "Detail",
            json!({"markdown":safe_markdown(&markdown),"navigationTitle":format!("Details for Node {}",row.version),"isLoading":false}),
            vec![],
        );
        if let Some(data) = &s.release {
            let metadata = vec![
                node(
                    "Detail.Metadata.Label",
                    json!({"title":"Published","text":model::published(&data.published_at)}),
                    vec![],
                ),
                node(
                    "Detail.Metadata.Link",
                    json!({"title":"Release Notes URL","text":data.html_url,"target":data.html_url}),
                    vec![],
                ),
                node(
                    "Detail.Metadata.Link",
                    json!({"title":"Docs","text":model::docs_url(&row.version)?,"target":model::docs_url(&row.version)?}),
                    vec![],
                ),
                node("Detail.Metadata.Separator", json!({}), vec![]),
                node(
                    "Detail.Metadata.TagList",
                    json!({"title":"Tags"}),
                    row.tags
                        .iter()
                        .map(|t| {
                            node(
                                "Detail.Metadata.TagList.Item",
                                json!({"text":t.label,"color":t.color}),
                                vec![],
                            )
                        })
                        .collect(),
                ),
            ];
            root["metadata"] = node("Detail.Metadata", json!({}), metadata);
        }
        root
    } else {
        let filter = b.add("Filter", "action", Action::Filter);
        b.actions.pop(); // Dropdown callback is registered, never an incomplete menu action.
        let dropdown = node(
            "List.Dropdown",
            json!({"tooltip":"Filter","defaultValue":"latest","value":s.filter,"storeValue":true,"actionId":filter}),
            [("Latest", "latest"), ("LTS", "lts"), ("All", "")]
                .iter()
                .map(|(t, v)| node("List.Dropdown.Item", json!({"title":t,"value":v}), vec![]))
                .collect(),
        );
        let groups = model::sections(&s.rows, &s.filter, &s.query);
        let icon =
            reqwest::Url::from_file_path(ctx.paths.package_dir.join("resources/nodejs-icon.svg"))
                .map_err(|_| "Invalid package resource path")?
                .to_string();
        let mut children = Vec::new();
        if let Some(error) = &s.error {
            children.push(node("List.EmptyView", json!({"title":error}), vec![]));
            b.add("Retry Node Versions", "action", Action::FetchFeed);
        }
        for (title, indices) in groups {
            let mut items = Vec::new();
            for i in indices {
                let row = &s.rows[i];
                let push = b.add(
                    &format!("Show Release Notes for {}", row.version),
                    "push",
                    Action::Detail(i),
                );
                let copy = b.add(
                    "Copy Version",
                    "action",
                    Action::Effect(Effect::Copy(row.version.clone())),
                );
                let mut accessories: Vec<Value> = row
                    .tags
                    .iter()
                    .map(|t| json!({"tag":{"value":t.label,"color":t.color}}))
                    .collect();
                accessories.push(model::date_accessory(&row.date));
                let keywords: Vec<String> = row
                    .tags
                    .iter()
                    .flat_map(|t| {
                        t.label
                            .to_lowercase()
                            .split(": ")
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                items.push(node("List.Item",json!({"id":format!("row:{i}"),"title":row.version,"icon":icon,"keywords":keywords,"accessories":accessories,"actionIds":[push,copy]}),vec![]));
            }
            children.push(node("List.Section", json!({"title":title}), items));
        }
        if children.is_empty() {
            children.push(node(
                "List.EmptyView",
                json!({"title":"No matching Node versions"}),
                vec![],
            ));
        }
        node(
            "List",
            json!({"searchBarPlaceholder":"Search Node Version","searchText":s.query,"searchBarAccessory":dropdown,"isLoading":false,"__hasServerSearch":true,"__hasMore":false}),
            children,
        )
    };
    if let Some(id) = back {
        root["props"]["backActionId"] = json!(id);
    }
    if let Some(effect) = b.gate.pending.clone() {
        let keep: Vec<String> = b
            .gate
            .actions
            .iter()
            .filter(|(_, a)| !matches!(a, Action::Effect(_)))
            .map(|(id, _)| id.clone())
            .collect();
        b.actions.retain(|a| keep.iter().any(|id| a["id"] == *id));
        b.gate
            .actions
            .retain(|(_, a)| !matches!(a, Action::Effect(_)));
        let retry = b.add(
            &format!("Retry {} (may repeat)", effect.title()),
            "action",
            Action::Retry,
        );
        let dismiss = b.add("Dismiss uncertain action", "action", Action::Dismiss);
        fn attach(node: &mut Value, valid: &[Value], retry: &str, dismiss: &str) {
            if node["type"] == "List.Item" {
                if let Some(ids) = node["props"]["actionIds"].as_array_mut() {
                    ids.retain(|id| valid.iter().any(|a| a["id"] == *id));
                    ids.push(json!(retry));
                    ids.push(json!(dismiss));
                }
            }
            if let Some(children) = node["children"].as_array_mut() {
                for child in children {
                    attach(child, valid, retry, dismiss);
                }
            }
        }
        attach(&mut root, &b.actions, &retry, &dismiss);
    }
    let mut result = tezbar_extension_sdk::view(
        if s.selected.is_some() {
            "Node Release Notes"
        } else {
            "Node Versions"
        },
        root,
        b.actions,
    );
    result["refreshMs"] = Value::Null;
    if let Some(message) = b
        .gate
        .warning
        .as_deref()
        .or(s.error.as_deref())
        .or(s.cache_warning.as_deref())
    {
        result["message"] = json!(message);
    }
    if serde_json::to_vec(&result)
        .map_err(|e| e.to_string())?
        .len()
        > 12 * 1024 * 1024
    {
        return Err("Rendered view exceeds 12 MiB; no content was truncated".into());
    }
    Ok(result)
}
