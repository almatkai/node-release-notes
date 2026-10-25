use node_release_notes_native::{model, validate_ack};
use serde_json::{json, Value};
fn oracle() -> Value {
    serde_json::from_str(
        if std::env::var("TZ").as_deref() == Ok("America/Los_Angeles") {
            include_str!("../qa/oracle-los-angeles.json")
        } else {
            include_str!("../qa/oracle-utc.json")
        },
    )
    .unwrap()
}
#[test]
fn genuine_source_validator_tags_groups_filters_search_urls_dates_actions() {
    let o = oracle();
    for case in o["validation"].as_array().unwrap() {
        let got = model::versions(case["input"].clone());
        if let Some(error) = case["error"].as_str() {
            assert_eq!(got.unwrap_err(), error);
        } else {
            assert!(got.is_ok());
        }
    }
    let rows =
        model::versions(serde_json::from_str(include_str!("../qa/fixtures/feed.json")).unwrap())
            .unwrap();
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(
            serde_json::to_value(&row.tags).unwrap(),
            o["tagged"][i]["tagList"]
        );
        assert_eq!(model::docs_url(&row.version).unwrap(), o["docs"][i]);
        assert_eq!(
            model::date_accessory(&row.date),
            o["rows"][i]["accessories"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .clone()
        );
    }
    assert_eq!(
        serde_json::to_value(model::sections(&rows, "", "")).unwrap(),
        o["grouping"]
    );
    for filter in ["latest", "lts", ""] {
        let got:Vec<Value>=model::sections(&rows,filter,"").iter().map(|(title,ids)|json!({"title":title,"versions":ids.iter().map(|i|rows[*i].version.clone()).collect::<Vec<_>>()})).collect();
        assert_eq!(json!(got), o["views"][filter]);
    }
    assert_eq!(model::sections(&rows, "", "jod")[0].1, vec![1, 3]);
    assert_eq!(model::sections(&rows, "", "CURRENT")[0].1, vec![0]);
    assert!(model::sections(&rows, "", "nonexistent").is_empty());
    let release =
        model::release(serde_json::from_str(include_str!("../qa/fixtures/release.json")).unwrap())
            .unwrap();
    assert_eq!(
        model::detail_markdown("v24.1.0", Some(&release), None),
        o["detail"]["markdown"]
    );
    assert_eq!(
        model::detail_markdown("v22.5.0", None, Some("Not Found")),
        o["detailError"]["markdown"]
    );
    assert_eq!(
        model::published("2025-01-01T23:59:58Z"),
        o["detail"]["metadata"][0]["text"]
    );
}
#[test]
fn url_and_release_hardening_acknowledgement_truth_table() {
    for v in ["v1.2.3", "v24.0.0-rc.1"] {
        assert!(model::safe_version(v).is_ok());
    }
    for v in [
        "",
        "v1",
        "../v1.2.3",
        "v1.2.3/evil",
        "v1.2.3?x",
        "v1.2.3#x",
        "v1.2.3%2f..",
        "v1.2.3\\evil",
        "https://evil.test",
        "v1.2.3\n",
    ] {
        assert!(model::safe_version(v).is_err());
    }
    for url in [
        "https://evil.test/nodejs/node/releases/tag/v1.2.3",
        "https://github.com@evil.test/nodejs/node/releases/tag/v1.2.3",
        "http://github.com/nodejs/node/releases/tag/v1.2.3",
        "https://github.com/nodejs/node/releases/tag/v1.2.3?x",
        "https://github.com/nodejs/node/releases/tag/v1.2.3/../evil",
    ] {
        assert!(model::release_url(url).is_err());
    }
    assert!(model::release(json!({"html_url":"https://github.com/nodejs/node/releases/tag/v1.2.3","published_at":"invalid","name":null,"body":null})).is_ok());
    for v in [
        Value::Null,
        json!(true),
        json!(false),
        json!({}),
        json!({"ok":false}),
        json!({"ok":1}),
        json!({"ok":true,"error":"simultaneous"}),
        json!({"ok":true,"error":null}),
        json!({"ok":true,"message":true}),
        json!({"ok":true,"success":false}),
    ] {
        assert!(validate_ack(&v, true).is_err(), "{v}");
        assert!(validate_ack(&v, false).is_err(), "{v}");
    }
    assert!(validate_ack(&json!({"ok":true}), false).is_ok());
    assert!(validate_ack(&json!({"ok":true,"message":"Opened"}), true).is_ok());
    assert!(validate_ack(&json!({"ok":true,"message":"Opened"}), false).is_err());
}
