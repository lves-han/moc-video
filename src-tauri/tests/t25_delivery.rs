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
fn renamed_product_uses_only_its_current_database() {
    let folder = tempfile::tempdir().unwrap();
    let legacy = folder.path().join("yingxiang-video.db");
    std::fs::write(&legacy, b"old data must stay untouched").unwrap();
    let host = Host::open(folder.path().join("settings.json")).unwrap();
    assert_eq!(data(&host, "list_history", json!({}))["entries"], json!([]));
    assert!(folder.path().join("moc-video.db").is_file());
    assert_eq!(
        std::fs::read(&legacy).unwrap(),
        b"old data must stay untouched"
    );
}

#[test]
fn old_settings_and_history_versions_are_rejected_without_migration() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    let host = Host::open(path.clone()).unwrap();
    let mut settings = data(&host, "get_settings", json!({}));
    drop(host);
    settings["schema_version"] = json!(1);
    let bytes = serde_json::to_vec(&settings).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(Host::open(path.clone()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    settings["schema_version"] = json!(2);
    std::fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
    let database = folder.path().join("moc-video.db");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TABLE old_marker(value TEXT); INSERT INTO old_marker VALUES ('preserved'); PRAGMA user_version=1;").unwrap();
    drop(connection);
    let host = Host::open(path).unwrap();
    assert!(!call(&host, "list_history", json!({}))["error"].is_null());
    let connection = rusqlite::Connection::open(&database).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT value FROM old_marker", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "preserved"
    );
}
