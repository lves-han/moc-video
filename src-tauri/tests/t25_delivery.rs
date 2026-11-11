//! README smoke command: fresh native initialization, persistence and diagnostics.
use moc_video::host::Host;
use serde_json::{Value, json};
use uuid::Uuid;

fn call(host: &Host, command: &str, input: Value) -> Value {
    host.invoke(
        command,
        json!({"request_id": Uuid::new_v4().to_string(), "input": input}),
        true,
    )
}

fn data(host: &Host, command: &str, input: Value) -> Value {
    let response = call(host, command, input);
    assert!(response["error"].is_null(), "{response}");
    response["data"].clone()
}

#[test]
fn fresh_profile_initializes_runs_and_retains_settings() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    assert!(!path.exists());
    let host = Host::open(path.clone()).unwrap();
    let ready = data(&host, "runtime_prepare", json!({}));
    assert_eq!(ready["phase"], "ready");
    assert_eq!(ready["health"]["media_gateway_ready"], true);
    assert_eq!(data(&host, "runtime_status", json!({}))["phase"], "ready");
    let mut settings = data(&host, "get_settings", json!({}));
    assert_eq!(settings["schema_version"], 2);
    assert!(!settings["source_configs"].as_array().unwrap().is_empty());
    settings["ui_locale"] = json!("zh-CN");
    data(&host, "save_settings", settings.clone());
    let diagnostic = call(
        &host,
        "get_resource_status",
        json!({"kind": "playback", "target_id": Uuid::new_v4().to_string()}),
    );
    assert!(diagnostic["error"].is_null(), "{diagnostic}");
    assert_eq!(diagnostic["data"]["state"], "expired");
    drop(host);
    let reopened = Host::open(path).unwrap();
    assert_eq!(data(&reopened, "get_settings", json!({})), settings);
    assert_eq!(
        data(&reopened, "list_history", json!({}))["entries"],
        json!([])
    );
}

#[test]
fn renamed_product_keeps_existing_history_database() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    let host = Host::open(path.clone()).unwrap();
    data(&host, "list_history", json!({}));
    drop(host);
    let current = folder.path().join("moc-video.db");
    let legacy = folder.path().join("yingxiang-video.db");
    std::fs::rename(&current, &legacy).unwrap();
    let reopened = Host::open(path).unwrap();
    assert_eq!(
        data(&reopened, "list_history", json!({}))["entries"],
        json!([])
    );
    assert!(legacy.is_file());
    assert!(!current.exists());
}
