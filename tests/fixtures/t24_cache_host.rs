//! Real Host/Settings IPC compatibility and cache-control permissions.
use super::*;
#[test]
fn t24_cache_host_settings_permissions_and_restart() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings.json");
    let host = Host::open(path.clone()).unwrap();
    let initial = ok(&host, "get_media_cache", json!({}));
    assert_eq!(initial["max_bytes"], 268435456);
    assert_eq!(initial["used_bytes"], 0);
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["media_cache_max_bytes"] = json!(1048576);
    ok(&host, "save_settings", settings.clone());
    assert_eq!(
        ok(&host, "get_media_cache", json!({}))["max_bytes"],
        1048576
    );
    let mut invalid = settings.clone();
    invalid["media_cache_max_bytes"] = json!(-1);
    error(&call(&host, "save_settings", invalid), "BAD_REQUEST");
    assert_eq!(ok(&host, "get_settings", json!({})), settings);
    for command in ["get_media_cache", "clear_media_cache"] {
        error(
            &call(&host, command, json!({"path":"../outside"})),
            "BAD_REQUEST",
        );
        error(
            &call(&host, command, json!({"write_sequence":1})),
            "BAD_REQUEST",
        );
        let denied = host.invoke(
            command,
            json!({"request_id":Uuid::new_v4().to_string(),"input":{}}),
            false,
        );
        error(&denied, "UNAUTHORIZED");
    }
    assert_eq!(ok(&host, "clear_media_cache", json!({}))["used_bytes"], 0);
    drop(host);
    let restarted = Host::open(path).unwrap();
    assert_eq!(
        ok(&restarted, "get_media_cache", json!({}))["max_bytes"],
        1048576
    );
    settings["media_cache_max_bytes"] = json!(0);
    ok(&restarted, "save_settings", settings);
    assert_eq!(ok(&restarted, "get_media_cache", json!({}))["max_bytes"], 0);
}

#[test]
fn t24_cache_host_limit_and_clear_keep_active_session() {
    let fixture = gateway_fixture(Arc::new(AtomicBool::new(false)));
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    let url = view["media_url"].as_str().unwrap();
    assert_eq!(resource(url).0, 200);
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["media_cache_max_bytes"] = json!(0);
    ok(&host, "save_settings", settings);
    assert_eq!(resource(url).0, 200);
    ok(&host, "clear_media_cache", json!({}));
    assert_eq!(resource(url).0, 200);
    assert_eq!(
        ok(
            &host,
            "get_resource_status",
            json!({"kind":"playback","target_id":view["session_id"]})
        )["state"],
        "ready"
    );
}
