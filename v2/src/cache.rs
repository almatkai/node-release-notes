//! Bounded extension-private cache, no TTL and no legacy/profile migration.
//! Cache is an optimization only: each new list/detail entry still revalidates.
use crate::model::{self, Release, Version};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tezbar_extension_sdk::ExtensionContext;
const MAX_CACHE: usize = 8 * 1024 * 1024;
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Cache {
    pub feed: Vec<Version>,
    pub details: Vec<(String, Release)>,
}
fn path(ctx: &ExtensionContext) -> PathBuf {
    ctx.paths.data_dir.join("node-cache.json")
}
impl Cache {
    pub fn read(ctx: &ExtensionContext) -> Result<Self, String> {
        let path = path(ctx);
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_CACHE as u64 {
            return Err("Invalid/oversized native cache".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_CACHE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_CACHE {
            return Err("Native cache exceeds 8 MiB".into());
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("Native cache: {e}"))?;
        let feed = if value["feed"].as_array().is_some_and(|a| a.is_empty()) {
            vec![]
        } else {
            model::versions(value["feed"].clone())?
        };
        let entries = value["details"]
            .as_array()
            .filter(|a| a.len() <= 16)
            .ok_or("Invalid native detail cache")?;
        let mut details = Vec::new();
        for entry in entries {
            let key = entry[0].as_str().ok_or("Invalid native cache key")?;
            model::safe_version(key)?;
            details.push((key.into(), model::release(entry[1].clone())?));
        }
        Ok(Self { feed, details })
    }
    fn value(&self) -> Value {
        json!({"feed":self.feed.iter().map(|r|{let mut row=json!({"version":r.version,"date":r.date});if let Some(lts)=&r.lts{row["lts"]=json!(lts);}row}).collect::<Vec<_>>(),"details":self.details})
    }
    pub fn save(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(&self.value()).map_err(|e| e.to_string())?;
        while bytes.len() > MAX_CACHE && !self.details.is_empty() {
            self.details.remove(0);
            bytes = serde_json::to_vec(&self.value()).map_err(|e| e.to_string())?;
        }
        if bytes.len() > MAX_CACHE {
            return Err("Feed cache exceeds 8 MiB; fresh view remains complete".into());
        }
        // Check canonical bounded JSON data individually before it enters this store.
        // No hidden truncation of a body/record; eviction removes whole cache entries.
        fs::create_dir_all(&ctx.paths.data_dir).map_err(|e| e.to_string())?;
        reject_symlink(&path(ctx))?;
        let mut tmp =
            tempfile::NamedTempFile::new_in(&ctx.paths.data_dir).map_err(|e| e.to_string())?;
        tmp.write_all(&bytes).map_err(|e| e.to_string())?;
        tmp.as_file().sync_all().map_err(|e| e.to_string())?;
        tmp.persist(path(ctx)).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn detail(&self, version: &str) -> Option<Release> {
        self.details
            .iter()
            .rev()
            .find(|(v, _)| v == version)
            .map(|(_, d)| d.clone())
    }
    pub fn put(&mut self, version: &str, release: Release) {
        self.details.retain(|(v, _)| v != version);
        self.details.push((version.into(), release));
        if self.details.len() > 16 {
            self.details.remove(0);
        }
    }
}
fn reject_symlink(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
        return Err("Native cache is not a regular file".into());
    }
    Ok(())
}
