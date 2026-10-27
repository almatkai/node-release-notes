use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MAX_RECORDS: usize = 12_000;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    pub label: String,
    pub value: String,
    pub color: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Version {
    pub version: String,
    pub date: String,
    pub lts: Option<String>,
    pub tags: Vec<Tag>,
}
fn string<'a>(v: &'a Value, key: &str, max: usize, error: &str) -> Result<&'a str, String> {
    let text = v[key].as_str().ok_or_else(|| error.to_string())?;
    if text.len() > max {
        return Err(format!("{key} exceeds {max} bytes"));
    }
    Ok(text)
}
pub fn versions(v: Value) -> Result<Vec<Version>, String> {
    let rows = v.as_array().ok_or("Node.js versions is not an array")?;
    if rows.is_empty() {
        return Err("Node.js versions is empty".into());
    }
    if rows.len() > MAX_RECORDS {
        return Err(format!("Node.js versions exceeds {MAX_RECORDS} records"));
    }
    let mut out: Vec<Version> = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        if !row.is_object() && !row.is_array() {
            return Err("Element of Node.js versions is not an object".into());
        }
        let version = string(
            row,
            "version",
            256,
            "Element of Node.js versions does not have a 'version' property",
        )?;
        let lts = match row.get("lts") {
            None | Some(Value::Bool(false)) => None,
            Some(Value::String(text)) if text.len() <= 256 => Some(text.clone()),
            Some(Value::String(_)) => return Err("lts exceeds 256 bytes".into()),
            _ => return Err("Element of Node.js versions does not have a 'lts' property".into()),
        };
        let date = string(
            row,
            "date",
            128,
            "Element of Node.js versions does not have a 'date' property",
        )?;
        let mut tags = Vec::new();
        let mut tag = |label: String, value: &str, color: &str| {
            tags.push(Tag {
                label,
                value: value.into(),
                color: color.into(),
            });
        };
        if index == 0 {
            tag("Current".into(), "current", "yellow");
        }
        if let Some(text) = lts.as_ref().filter(|s| !s.is_empty()) {
            tag(format!("LTS: {text}"), "lts", "blue");
        }
        if out.last().map(|r| major(&r.version)) != Some(major(version)) {
            tag("Latest".into(), "latest", "green");
        }
        out.push(Version {
            version: version.into(),
            date: date.into(),
            lts,
            tags,
        });
    }
    Ok(out)
}
pub fn major(version: &str) -> &str {
    version.split('.').next().unwrap_or("")
}
pub fn sections(rows: &[Version], filter: &str, search: &str) -> Vec<(String, Vec<usize>)> {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if filter == "lts" && row.lts.as_ref().is_none_or(|s| s.is_empty()) {
            continue;
        }
        let key = major(&row.version);
        if let Some((_, items)) = groups.iter_mut().find(|(s, _)| s == key) {
            items.push(i);
        } else {
            groups.push((key.into(), vec![i]));
        }
    }
    // For canonical Node versions keys start with 'v'; feed order is authoritative.
    // JS integer-like object keys and prototype names are outside the safe URL domain.
    if filter == "latest" {
        groups = vec![(
            "Latest".into(),
            groups
                .iter()
                .filter_map(|(_, ids)| ids.first().copied())
                .collect(),
        )];
    }
    let query = search.trim().to_lowercase();
    for (_, items) in &mut groups {
        items.retain(|i| {
            let row = &rows[*i];
            query.is_empty()
                || row.version.to_lowercase().contains(&query)
                || row
                    .tags
                    .iter()
                    .flat_map(|t| {
                        t.label
                            .to_lowercase()
                            .split(": ")
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .any(|k| k.contains(&query))
        });
    }
    groups.retain(|(_, ids)| !ids.is_empty());
    groups
}
/// Source accepts arbitrary strings. Navigation is deliberately narrower: one
/// canonical Node release segment, never slash/query/fragment/percent traversal.
pub fn safe_version(v: &str) -> Result<(), String> {
    if v.len() > 128 || !v.starts_with('v') {
        return Err("Unsafe Node release version".into());
    }
    let (core, suffix) = v[1..]
        .split_once('-')
        .map_or((&v[1..], None), |(a, b)| (a, Some(b)));
    let parts: Vec<_> = core.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        || suffix.is_some_and(|s| {
            s.is_empty()
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
        })
    {
        return Err("Unsafe Node release version".into());
    }
    Ok(())
}
pub fn docs_url(v: &str) -> Result<String, String> {
    safe_version(v)?;
    Ok(format!("https://nodejs.org/docs/{v}/api/"))
}
pub fn release_url(text: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(text).map_err(|_| "Invalid Release Notes URL")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Unsafe Release Notes URL".into());
    }
    let path = url
        .path()
        .strip_prefix("/nodejs/node/releases/tag/")
        .ok_or("Unsafe Release Notes path")?;
    safe_version(path)?;
    if url.as_str() != text {
        return Err("Noncanonical Release Notes URL".into());
    }
    Ok(text.into())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Release {
    pub name: String,
    pub body: String,
    pub html_url: String,
    pub published_at: String,
}
pub fn release(v: Value) -> Result<Release, String> {
    if !v.is_object() {
        return Err("Invalid GitHub release object".into());
    }
    // GitHub can return null name/body; source nullish name and falsy body become ''.
    let optional = |key: &str, max| -> Result<String, String> {
        match v.get(key) {
            None | Some(Value::Null) => Ok(String::new()),
            Some(Value::String(s)) if s.len() <= max => Ok(s.clone()),
            _ => Err(format!("Invalid GitHub {key}")),
        }
    };
    let html_url = release_url(string(&v, "html_url", 2048, "Invalid GitHub html_url")?)?;
    Ok(Release {
        name: optional("name", 4096)?,
        body: optional("body", 1_048_576)?,
        html_url,
        published_at: string(&v, "published_at", 128, "Invalid GitHub published_at")?.into(),
    })
}
fn parse_date(text: &str) -> Option<DateTime<Local>> {
    if let Ok(date) = DateTime::parse_from_rfc3339(text) {
        return Some(date.with_timezone(&Local));
    }
    if text.len() == 10 {
        return NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|d| Utc.from_utc_datetime(&d).with_timezone(&Local));
    }
    None
}
/// Finite native adaptation: system local timezone, fixed English Gregorian
/// formatting. ISO date / RFC3339 only; not universal JS Date/Intl equivalence.
pub fn date_accessory(text: &str) -> Value {
    if text.is_empty() {
        return json!({});
    }
    let (short, long) = match parse_date(text) {
        None => ("Invalid Date".into(), "Invalid Date".into()),
        Some(d) => (
            format!("{}/{}/{:02}", d.month(), d.day(), d.year().rem_euclid(100)),
            format!("{} {}, {}", d.format("%B"), d.day(), d.year()),
        ),
    };
    json!({"text":{"value":short},"icon":"Icon.Calendar","tooltip":long})
}
pub fn published(text: &str) -> String {
    match parse_date(text) {
        None => "Invalid Date".into(),
        Some(d) => format!(
            "{}/{}/{}, {}:{:02}:{:02} {}",
            d.month(),
            d.day(),
            d.year(),
            if d.hour() % 12 == 0 {
                12
            } else {
                d.hour() % 12
            },
            d.minute(),
            d.second(),
            if d.hour() < 12 { "AM" } else { "PM" }
        ),
    }
}

/// Exact source template before native rendering hardening.
pub fn detail_markdown(version: &str, data: Option<&Release>, error: Option<&str>) -> String {
    format!(
        "# {version}\n## Release Name: \n{}\n\n{}",
        data.map_or("", |d| d.name.as_str()),
        data.filter(|d| !d.body.is_empty())
            .map(|d| d.body.as_str())
            .or(error)
            .unwrap_or("")
    )
}
