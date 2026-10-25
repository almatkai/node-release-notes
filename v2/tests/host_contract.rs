#[path = "../integration/host_contract.rs"]
mod host;
use host::process_install as p;
use serde_json::{json, Value};
fn manifest() -> Value {
    serde_json::from_str(include_str!("../extension.json")).unwrap()
}
#[test]
fn actual_schema3_runtime_validation_and_acquisition_separation() {
    let m = manifest();
    assert!(p::check(&m).is_ok());
    assert!(p::acquisition(&m)
        .unwrap_err()
        .contains("publication/pins required"));
    for (key, value) in [
        ("schemaVersion", json!(2)),
        ("execution", json!("compiled")),
        ("protocolVersion", json!(2)),
        ("runtime", json!("javascript")),
        ("id", json!("raycast.node-release-notes")),
        ("capabilities", json!(["filesystem"])),
        ("slug", json!("../bad")),
    ] {
        let mut bad = m.clone();
        bad[key] = value;
        assert!(p::check(&bad).is_err());
    }
    for (key, value) in [
        ("refreshMs", json!(1)),
        ("timeoutMs", json!(0)),
        ("timeoutMs", json!(300001)),
        ("concurrency", json!("parallel")),
    ] {
        let mut bad = m.clone();
        bad["commands"][0][key] = value;
        assert!(p::check(&bad).is_err());
    }
    let mut bad = m.clone();
    bad["commands"]
        .as_array_mut()
        .unwrap()
        .push(m["commands"][0].clone());
    assert!(p::check(&bad).is_err());
    let mut bad = m.clone();
    bad["artifacts"]["aarch64-apple-darwin"] =
        json!({"sha256":"fake","executable":"../escape","executableSha256":"fake"});
    assert!(p::acquisition(&bad).is_err());
}
#[test]
fn actual_registry_before_after_uses_verified_version_resource_directory() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work/registry");
    std::fs::create_dir_all(&base).unwrap();
    let tmp = tempfile::tempdir_in(&base).unwrap();
    let root = tmp
        .path()
        .join("extensions/packages/com.tezbar.native.node-release-notes");
    let active = root.join("versions/verified/resources");
    std::fs::create_dir_all(&active).unwrap();
    std::fs::write(active.join("app-icon.png"), b"fixture").unwrap();
    let m = manifest();
    let cmd = &m["commands"][0];
    let before = host::registry_before::Installed {
        id: m["id"].as_str().unwrap().into(),
        slug: "node-release-notes".into(),
        path: root.clone(),
        manifest: m.clone(),
    };
    let after = host::registry_after::Installed {
        id: before.id.clone(),
        slug: before.slug.clone(),
        path: root.clone(),
        manifest: m.clone(),
        resource_dir: Some(root.join("versions/verified")),
    };
    assert!(host::registry_before::icon(&before).is_none());
    assert!(host::registry_before::command(&before, cmd).is_none());
    assert_eq!(
        host::registry_after::icon(&after),
        Some(active.join("app-icon.png").to_string_lossy().into_owned())
    );
    assert_eq!(
        host::registry_after::command(&after, cmd),
        host::registry_after::icon(&after)
    );
    let bad = host::registry_after::Installed {
        resource_dir: None,
        ..after
    };
    assert!(host::registry_after::icon(&bad).is_none());
}
#[test]
fn actual_package_inventory_accepts_complete_bytes_and_rejects_tamper() {
    use sha2::{Digest, Sha256};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work/inventory-test");
    std::fs::create_dir_all(&base).unwrap();
    let tmp = tempfile::tempdir_in(&base).unwrap();
    std::fs::create_dir(tmp.path().join("bin")).unwrap();
    std::fs::write(tmp.path().join("bin/probe"), b"binary").unwrap();
    let hash = format!("{:x}", Sha256::digest(b"binary"));
    let m = json!({"files":{"bin/probe":hash}});
    let a = json!({"executable":"bin/probe","executableSha256":hash});
    assert!(p::inventory(tmp.path(), &m, &a).is_ok());
    std::fs::write(tmp.path().join("bin/probe"), b"tampered").unwrap();
    assert!(p::inventory(tmp.path(), &m, &a).is_err());
}
#[test]
#[ignore = "Run after qa/package.py against the actual extracted development ZIP"]
fn extracted_package_uses_actual_host_inventory_verifier() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work/package-extracted");
    let m: Value =
        serde_json::from_slice(&std::fs::read(root.join("extension.json")).unwrap()).unwrap();
    assert!(p::check(&m).is_ok());
    let a = json!({"executable":"bin/node-release-notes-native","executableSha256":m["files"]["bin/node-release-notes-native"]});
    assert!(p::inventory(&root, &m, &a).is_ok());
}
#[test]
fn declared_own_replacement_routes_only_with_explicit_catalog_declaration() {
    // Catalog seam is this port's actual manifest, not a host catalog mutation.
    assert_eq!(
        host::routing_before::slug("raycast.node-release-notes"),
        Some("node-release-notes")
    );
    assert_eq!(
        host::routing_before::slug("com.tezbar.node-release-notes"),
        None
    );
    assert_eq!(
        host::routing_after::slug("com.tezbar.node-release-notes"),
        Some("node-release-notes")
    );
    assert_eq!(host::routing_after::slug("com.tezbar.undeclared"), None);
    assert_eq!(
        host::routing_after::slug("com.tezbar.native.node-release-notes"),
        Some("node-release-notes")
    );
}
