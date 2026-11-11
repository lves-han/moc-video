//! T0.23: configuration and sync transactions around live playback of two JSON sources.
use super::*;

const SECOND: &str = "33333333-3333-4333-8333-333333333333";

/// Metadata, media and MacCMS catalog; `/b/` answers as a second source and `fail` breaks only catalogs.
fn sources_fixture(fail: Arc<AtomicBool>) -> Fixture {
    Fixture::new(move |request, port| {
        if let Some(reply) = metadata_reply(request) {
            return reply;
        }
        if request.target.contains(".mp4") {
            return Reply::raw(vec![0x5a; 4096], "video/mp4");
        }
        if fail.load(Ordering::SeqCst) {
            return Reply {
                status: 500,
                ..Reply::raw("unavailable", "text/plain")
            };
        }
        let mut raw = item(port);
        raw["vod_name"] = json!("Same title");
        raw["vod_year"] = Value::Null;
        raw["vod_play_from"] = json!("线路");
        raw["vod_play_url"] = json!(format!(
            "Special$http://maccms.fixture.test:{port}/a.mp4#Second$http://maccms.fixture.test:{port}/b.mp4"
        ));
        Reply::json(response(json!([raw]), json!([])))
    })
}

struct Two {
    fixture: Fixture,
    fail: Arc<AtomicBool>,
    temp: TempDir,
    host: Arc<Host>,
    first: Value,
    second: Value,
    first_view: Value,
    second_view: Value,
}

fn play(host: &Host, provider: &Value, input: &Value) -> Value {
    let mut input = input.clone();
    input["playback_source"] =
        json!({"source_id":provider["id"],"source_revision":provider["source_revision"]});
    let context = selected(host, provider, &input);
    let resolution = ok(host, "resolve_episode", resolve_input(&context));
    ok(host, "open_playback", open_input(&resolution))
}

fn two_playing() -> Two {
    let fail = Arc::new(AtomicBool::new(false));
    let fixture = sources_fixture(fail.clone());
    let (temp, host, first, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["source_configs"].as_array_mut().unwrap().push(
        json!({"kind":"maccms","config_id":SECOND,"name":"Second","enabled":true,"api_url":fixture.url("/b/api")}),
    );
    ok(&host, "save_settings", settings);
    let second = ok(
        &host,
        "sync_catalog",
        json!({"kind":"maccms","config_id":SECOND}),
    )["sources"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["origin"]["config_id"] == SECOND)
        .unwrap()
        .clone();
    let first_view = play(&host, &first, &input);
    let second_view = play(&host, &second, &input);
    Two {
        fixture,
        fail,
        temp,
        host,
        first,
        second,
        first_view,
        second_view,
    }
}

fn media(view: &Value) -> u16 {
    resource(view["media_url"].as_str().unwrap()).0
}

fn state(host: &Host, view: &Value) -> Value {
    diagnostic(host, view)["state"].clone()
}

fn providers(host: &Host) -> Value {
    ok(host, "get_sources", json!({}))["providers"].clone()
}

#[test]
fn t19_unchanged_save_and_failed_sync_keep_the_confirmed_source_and_playback() {
    let two = two_playing();
    let host = &two.host;
    let before = providers(host);
    let unchanged = ok(host, "get_settings", json!({}));
    ok(host, "save_settings", unchanged);
    assert_eq!(providers(host), before);
    assert_eq!(media(&two.first_view), 200);
    assert_ne!(state(host, &two.first_view), "expired");
    // A failed refresh reports its cause and leaves the published source and its session alone.
    two.fail.store(true, Ordering::SeqCst);
    let failed = call(
        host,
        "sync_catalog",
        json!({"kind":"maccms","config_id":CONFIG}),
    );
    error(&failed, "UPSTREAM_FAILED");
    assert!(failed["error"]["message"].as_str().unwrap().len() > 3);
    assert_eq!(providers(host), before);
    assert_eq!(media(&two.first_view), 200);
    assert_ne!(state(host, &two.first_view), "expired");
    two.fail.store(false, Ordering::SeqCst);
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t19_editing_one_source_stops_only_its_playback_and_references() {
    for change in ["api", "disable", "delete"] {
        let two = two_playing();
        let host = &two.host;
        let reference = json!({"source_id":two.first["id"],"source_revision":two.first["source_revision"],
            "data":serde_json::to_string(&["maccms", "视频/😀+&%$#"]).unwrap()});
        ok(host, "get_details", json!({"ref":reference}));
        let mut settings = ok(host, "get_settings", json!({}));
        match change {
            "api" => {
                settings["source_configs"][0]["api_url"] = json!(two.fixture.url("/edited/api"))
            }
            "disable" => settings["source_configs"][0]["enabled"] = json!(false),
            _ => {
                settings["source_configs"].as_array_mut().unwrap().remove(0);
            }
        }
        ok(host, "save_settings", settings);
        assert_eq!(state(host, &two.first_view), "expired", "{change}");
        assert_ne!(media(&two.first_view), 200, "{change}");
        assert_eq!(
            media(&two.second_view),
            200,
            "{change}: the other source keeps playing"
        );
        assert_ne!(state(host, &two.second_view), "expired");
        let stale = call(host, "get_details", json!({"ref":reference}));
        assert!(
            stale.get("error").is_some(),
            "{change}: an old reference stays dead"
        );
        let after = providers(host);
        assert!(
            after
                .as_array()
                .unwrap()
                .iter()
                .any(|provider| provider["id"] == two.second["id"]
                    && provider["runtime_status"] == "ready")
        );
        assert!(
            !host
                .committed
                .lock()
                .unwrap()
                .session_bindings
                .contains_key(two.first_view["session_id"].as_str().unwrap())
        );
        assert_eq!(host.requests.active_count(), 0);
    }
}

#[test]
fn t19_failed_persistence_keeps_configuration_registry_and_playback() {
    let two = two_playing();
    let host = &two.host;
    let confirmed = ok(host, "get_settings", json!({}));
    let before = providers(host);
    // The settings path turns into a directory, so the atomic replacement fails.
    let path = two.temp.path().join("settings.json");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let mut edited = confirmed.clone();
    edited["source_configs"][0]["api_url"] = json!(two.fixture.url("/edited/api"));
    error(&call(host, "save_settings", edited), "INTERNAL_ERROR");
    assert_eq!(ok(host, "get_settings", json!({})), confirmed);
    assert_eq!(providers(host), before);
    assert_eq!(
        media(&two.first_view),
        200,
        "nothing was committed, so playback goes on"
    );
    assert_ne!(state(host, &two.first_view), "expired");
    // A session stopped earlier stays stopped through the failure.
    ok(
        host,
        "close_playback",
        json!({"session_id":two.second_view["session_id"]}),
    );
    let mut again = confirmed.clone();
    again["volume"] = json!(0.5);
    error(&call(host, "save_settings", again), "INTERNAL_ERROR");
    assert_eq!(state(host, &two.second_view), "expired");
    std::fs::remove_dir(&path).unwrap();
}

#[test]
fn t19_invalid_configurations_are_refused_before_anything_changes() {
    let two = two_playing();
    let host = &two.host;
    let confirmed = ok(host, "get_settings", json!({}));
    let before = providers(host);
    let mut over = confirmed.clone();
    over["source_configs"] = json!(
        (0..33)
            .map(
                |n| json!({"kind":"maccms","config_id":Uuid::new_v4().to_string(),
        "name":format!("来源{n}"),"enabled":false,"api_url":format!("https://example.test/{n}")})
            )
            .collect::<Vec<_>>()
    );
    let mut bad_url = confirmed.clone();
    bad_url["source_configs"][0]["api_url"] = json!("https://user:secret@example.test/api");
    let mut duplicate = confirmed.clone();
    let copy = duplicate["source_configs"][1].clone();
    duplicate["source_configs"]
        .as_array_mut()
        .unwrap()
        .push(copy);
    for invalid in [over, bad_url, duplicate] {
        let result = call(host, "save_settings", invalid);
        assert!(result.get("error").is_some(), "{result}");
        assert!(!result.to_string().contains("secret"));
    }
    assert_eq!(ok(host, "get_settings", json!({})), confirmed);
    assert_eq!(providers(host), before);
    assert_eq!(media(&two.first_view), 200);
}
