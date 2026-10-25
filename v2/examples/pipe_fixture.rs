//! QA executable: real Adapter + exact SDK, synthetic captured transport only.
//! Never included in a distributable; no environment endpoint override.
use node_release_notes_native::{http::Transport, Adapter, FEED};
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use tezbar_extension_sdk::BoxFuture;
struct Captured {
    mode: String,
    feed: AtomicUsize,
    detail: AtomicUsize,
}
impl Transport for Captured {
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            let feed = url == FEED;
            let n = if feed {
                self.feed.fetch_add(1, Ordering::SeqCst)
            } else {
                self.detail.fetch_add(1, Ordering::SeqCst)
            };
            eprintln!("CAPTURE {} {}", if feed { "feed" } else { "detail" }, n + 1);
            let failure_mode = if feed { "fail-feed" } else { "fail-detail" };
            if self.mode == failure_mode && n == 0 {
                return Err("Captured completed failure".into());
            }
            if (self.mode == "slow-detail" && !feed) || (self.mode == "slow-open" && feed) {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            let text = if feed {
                include_str!("../qa/fixtures/feed.json")
            } else if self.mode == "malicious" {
                include_str!("../qa/fixtures/malicious-release.json")
            } else {
                include_str!("../qa/fixtures/release.json")
            };
            serde_json::from_str(text).map_err(|e| e.to_string())
        })
    }
}
#[tokio::main(worker_threads = 2)]
async fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let result =
        tezbar_extension_sdk::serve(Adapter::with_transport(std::sync::Arc::new(Captured {
            mode,
            feed: AtomicUsize::new(0),
            detail: AtomicUsize::new(0),
        })))
        .await;
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
