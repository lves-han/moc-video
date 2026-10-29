//! Owned HTTP fixture drives the production invoke/dispatch/adapters/stores.
use super::*;

const METADATA_CONFIG: &str = "22222222-2222-4222-8222-222222222222";
fn meta() -> Value {
    json!({"id":"series:original","type":"series","name":"Same title","releaseInfo":"2024","videos":[{"id":"raw:season0:0.5","title":"Special","season":0,"episode":0.5},{"id":"raw:season2:1","title":"Season two","season":2,"episode":1}]})
}
fn metadata_reply(request: &Request) -> Option<Reply> {
    if request.target.contains("manifest.json") {
        Some(Reply::json(
            json!({"id":"tmdb-addon","version":"1.0.0","name":"Metadata fixture","description":"controlled","types":["series"],"resources":["catalog","meta"],"catalogs":[{"id":"all","type":"series"}]}),
        ))
    } else if request.target.contains("/catalog/") {
        Some(Reply::json(json!({"metas":[meta()]})))
    } else if request.target.contains("/meta/") {
        let mut value = meta();
        if request.target.contains("/zh-CN/") {
            value["name"] = json!("已有别名");
        }
        Some(Reply::json(json!({"meta":value})))
    } else {
        None
    }
}
fn fixture() -> Fixture {
    Fixture::new(|request, port| {
        if let Some(reply) = metadata_reply(request) {
            return reply;
        }
        if request.target.starts_with("/probe") {
            return Reply::raw(
                "#EXTM3U\n#EXTINF:1,\nchunk.ts",
                "application/vnd.apple.mpegurl",
            );
        }
        let mut raw = item(port);
        raw["vod_name"] = json!("Same title");
        raw["vod_year"] = Value::Null;
        raw["vod_play_url"] = json!(format!(
            "Special 0.5$http://maccms.fixture.test:{port}/probe?x=$keep%2F#Other season$http://maccms.fixture.test:{port}/b.mp4$$$Dub$http://maccms.fixture.test:{port}/dub.mp4"
        ));
        Reply::json(response(json!([raw]), json!([])))
    })
}
fn setup_both(
    fixture: &Fixture,
    clock: Arc<dyn Clock>,
) -> (TempDir, Arc<Host>, Value, Value, Value) {
    let temp = TempDir::new().unwrap();
    let host = Host::with_clock(temp.path().join("settings.json"), clock).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([
        {"kind":"maccms","config_id":CONFIG,"name":"Playback fixture","enabled":true,"api_url":fixture.url("/api?original=a%2Fb")},
        {"kind":"stremio","config_id":METADATA_CONFIG,"name":"Metadata fixture","enabled":true,"manifest_urls":[fixture.url("/tmdb/manifest.json")]}]);
    ok(&host, "save_settings", settings);
    let provider = sync(&host);
    let sources = ok(
        &host,
        "sync_catalog",
        json!({"kind":"stremio","config_id":METADATA_CONFIG}),
    )["sources"]["providers"]
        .clone();
    let metadata = sources[1].clone();
    let card =
        ok(&host, "get_home", home_input(&metadata))["page"]["sections"][0]["items"][0].clone();
    let details = ok(&host, "get_details", json!({"ref":card["ref"]}));
    let input = json!({"metadata_ref":details["metadata_ref"],"video_id":"raw:season0:0.5","playback_source":{"source_id":provider["id"],"source_revision":provider["source_revision"]},"selection":null});
    (temp, host, provider, metadata, input)
}
fn inspect(host: &Host, provider: &Value) -> Value {
    let card =
        ok(host, "get_home", home_input(provider))["page"]["sections"][0]["items"][0].clone();
    ok(host, "get_details", json!({"ref":card["ref"]}))["details"].clone()
}
fn choose(input: &Value, details: &Value, group: usize, episode: usize) -> Value {
    let mut input = input.clone();
    input["selection"] = json!({"content_ref":details["ref"],"group_id":details["groups"][group]["id"],"episode_id":details["groups"][group]["episodes"][episode]["id"]});
    input
}
fn resolve_input(context: &Value) -> Value {
    json!({"match_id":context["match_id"],"episode_ref":context["episode_ref"]})
}

#[test]
fn t08_maccms_http_match_manual_resolve_probe_register_release() {
    let fixture = fixture();
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let result = ok(&host, "match_playback", input.clone());
    assert_eq!(result["status"], "needs_confirmation");
    assert!(result["context"].is_null());
    assert_eq!(result["candidates"][0]["year"], Value::Null);
    let details = inspect(&host, &provider);
    let selected = choose(&input, &details, 0, 0);
    let matched = ok(&host, "match_playback", selected.clone());
    assert_eq!(matched["status"], "matched");
    let context = &matched["context"];
    assert_eq!(
        context["episode_ref"],
        details["groups"][0]["episodes"][0]["ref"]
    );
    assert!(context["selected_stream_id"].is_null());
    assert_eq!(context["metadata_video_id"], "raw:season0:0.5");
    let resolution = ok(&host, "resolve_episode", resolve_input(context));
    assert_eq!(resolution["episode_ref"], context["episode_ref"]);
    assert_eq!(resolution["streams"][0]["kind"], "hls");
    assert_eq!(resolution["streams"][0]["playable"], true);
    assert_eq!(resolution["streams"][0]["label"], "线路甲 · Special 0.5");
    let rid = resolution["id"].as_str().unwrap();
    let sid = resolution["streams"][0]["id"].as_str().unwrap();
    let revision = provider["source_revision"].as_str().unwrap();
    let retained = host
        .committed
        .lock()
        .unwrap()
        .resolutions
        .get(rid, sid, revision, host.clock.now_ms())
        .unwrap();
    assert!(retained.link.url.ends_with("/probe?x=$keep%2F"));
    assert_eq!(
        ok(&host, "release_resolution", json!({"resolution_id":rid})),
        json!({"ok":true})
    );
    ok(&host, "release_resolution", json!({"resolution_id":rid}));
    assert_eq!(
        host.committed
            .lock()
            .unwrap()
            .resolutions
            .get(rid, sid, revision, host.clock.now_ms())
            .err()
            .unwrap()
            .code,
        "SESSION_EXPIRED"
    );
    assert_eq!(retained.episode_ref, context["episode_ref"]);
    let requests = fixture.requests.lock().unwrap();
    assert!(requests.iter().any(|r| r.target.starts_with("/probe")
        && r.headers.get("range").map(String::as_str) == Some("bytes=0-65535")));
    assert!(
        requests
            .iter()
            .any(|r| r.query.get("wd").map(String::as_str) == Some("Same title"))
    );
    println!(
        "T08_VECTOR:{}",
        json!({"kind":"http","synthetic":false,"input":selected,"details":details,"match":matched,"resolution":resolution})
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t08_maccms_http_selection_forgery_video_provider_and_stremio_rejection() {
    let fixture = fixture();
    let (_temp, host, provider, metadata, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let mut wrong = input.clone();
    wrong["video_id"] = json!("wrong");
    error(&call(&host, "match_playback", wrong), "BAD_REQUEST");
    let mut wrong = input.clone();
    wrong["metadata_ref"]["identity"]["canonical_id"] = json!("forged");
    error(&call(&host, "match_playback", wrong), "BAD_REQUEST");
    let unregistered = json!({"ref":{"source_id":provider["id"],"source_revision":provider["source_revision"],"data":"unknown"},"groups":[{"id":"fake","episodes":[{"id":"fake"}]}]});
    error(
        &call(&host, "match_playback", choose(&input, &unregistered, 0, 0)),
        "BAD_REQUEST",
    );
    let details = inspect(&host, &provider);
    for path in ["group_id", "episode_id"] {
        let mut wrong = choose(&input, &details, 0, 0);
        wrong["selection"][path] =
            details["groups"][1][if path == "group_id" { "id" } else { "title" }].clone();
        error(&call(&host, "match_playback", wrong), "BAD_REQUEST");
    }
    let matched = ok(&host, "match_playback", choose(&input, &details, 0, 0));
    let mut wrong = resolve_input(&matched["context"]);
    wrong["episode_ref"] = details["groups"][0]["episodes"][1]["ref"].clone();
    error(&call(&host, "resolve_episode", wrong), "BAD_REQUEST");
    let mut wrong = resolve_input(&matched["context"]);
    wrong["match_id"] = json!("forged");
    error(&call(&host, "resolve_episode", wrong), "SESSION_EXPIRED");
    let key = json!({"source_id":metadata["id"],"source_revision":metadata["source_revision"],"data":"metadata"});
    error(
        &call(
            &host,
            "resolve_episode",
            json!({"episode_ref":key,"match_id":matched["context"]["match_id"]}),
        ),
        "CAPABILITY_UNAVAILABLE",
    );
    let mut other = input.clone();
    other["video_id"] = json!("raw:season2:1");
    ok(&host, "match_playback", other);
    error(
        &call(&host, "resolve_episode", resolve_input(&matched["context"])),
        "SESSION_EXPIRED",
    );
}

#[test]
fn t08_maccms_http_missing_search_manual_home_details_and_zero_error() {
    let fixture = fixture();
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let mut source = provider.clone();
    source["capabilities"]["search"] = json!(false);
    let snapshot = host.capture_source(CONFIG, true).unwrap();
    host.publish_source(&snapshot, source, &job(&host, "http_source_sync"))
        .unwrap();
    let result = call(&host, "match_playback", input.clone());
    error(&result, "CAPABILITY_UNAVAILABLE");
    assert_eq!(result["error"]["details"]["home"], true);
    assert_eq!(result["error"]["details"]["details"], true);
    let details = inspect(&host, &provider);
    assert_eq!(
        ok(&host, "match_playback", choose(&input, &details, 1, 0))["status"],
        "matched"
    );
    drop(fixture);
    for failed in [false, true] {
        let fixture = Fixture::new(move |request, _| {
            if let Some(reply) = metadata_reply(request) {
                return reply;
            }
            if request.query.contains_key("wd") {
                if failed {
                    Reply::raw("broken JSON", "application/json")
                } else {
                    Reply::json(response(json!([]), json!([])))
                }
            } else {
                Reply::json(response(json!([]), json!([])))
            }
        });
        let (_temp, host, _, _, input) = setup_both(
            &fixture,
            Arc::new(crate::requests::MonotonicClock::default()),
        );
        let result = call(&host, "match_playback", input);
        if failed {
            error(&result, "UPSTREAM_FAILED");
        } else {
            assert_eq!(result["data"]["status"], "not_found");
            assert_eq!(result["data"]["candidates"], json!([]));
        }
    }
}

#[test]
fn t08_maccms_http_match_expiry_order_cancel_and_configuration_change() {
    let fixture = fixture();
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let (_temp, host, provider, _, input) = setup_both(&fixture, clock.clone());
    let details = inspect(&host, &provider);
    let selected = choose(&input, &details, 0, 0);
    let args = |input: Value| json!({"request_id":id(),"input":input});
    let older = host
        .admit("match_playback", args(input.clone()), true)
        .unwrap_or_else(|_| panic!("admit"));
    let newer = host
        .admit("match_playback", args(input.clone()), true)
        .unwrap_or_else(|_| panic!("admit"));
    error(&host.execute(older), "CANCELLED");
    assert_eq!(host.execute(newer)["data"]["status"], "needs_confirmation");
    let cancelled_id = id();
    let cancelled = host
        .admit(
            "match_playback",
            json!({"request_id":cancelled_id,"input":selected}),
            true,
        )
        .unwrap_or_else(|_| panic!("admit"));
    host.requests.cancel(&id(), &cancelled_id).unwrap();
    error(&host.execute(cancelled), "CANCELLED");
    let matched = ok(&host, "match_playback", choose(&input, &details, 0, 0));
    clock.0.store(300000, Ordering::SeqCst);
    error(
        &call(&host, "resolve_episode", resolve_input(&matched["context"])),
        "SESSION_EXPIRED",
    );
    let details = inspect(&host, &provider);
    let matched = ok(&host, "match_playback", choose(&input, &details, 0, 0));
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["source_configs"][1]["name"] = json!("edited metadata");
    ok(&host, "save_settings", settings);
    error(
        &call(&host, "resolve_episode", resolve_input(&matched["context"])),
        "SESSION_EXPIRED",
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t08_maccms_http_english_alias_two_queries_ten_each_and_partial_error() {
    for failed in [false, true] {
        let fixture = Fixture::new(move |request, _| {
            if let Some(reply) = metadata_reply(request) {
                return reply;
            }
            if let Some(query) = request.query.get("wd") {
                if failed && query == "已有别名" {
                    return Reply::raw("broken JSON", "application/json");
                }
                let items: Vec<_> = (0..15).map(|index| if index < 10 {
                    json!({"vod_id":format!("{query}/{index}"), "vod_name":"Same title remake", "vod_year":if index == 0 {Value::Null} else {json!(1990+index)}})
                } else { json!({"deliberately":"outside the ten-candidate limit"}) }).collect();
                return Reply::json(response(json!(items), json!([])));
            }
            Reply::json(response(json!([]), json!([])))
        });
        let (_temp, host, _, _, input) = setup_both(
            &fixture,
            Arc::new(crate::requests::MonotonicClock::default()),
        );
        let mut settings = ok(&host, "get_settings", json!({}));
        settings["ui_locale"] = json!("zh-CN");
        ok(&host, "save_settings", settings);
        let local = ok(
            &host,
            "get_details",
            json!({"ref":input["metadata_ref"]["content_ref"]}),
        );
        assert_eq!(local["details"]["title"], "已有别名");
        assert_eq!(local["metadata_ref"], input["metadata_ref"]);
        let result = call(&host, "match_playback", input);
        if failed {
            error(&result, "UPSTREAM_FAILED");
        } else {
            assert_eq!(result["data"]["status"], "needs_confirmation");
            assert_eq!(result["data"]["candidates"].as_array().unwrap().len(), 20);
            assert!(result["data"]["context"].is_null());
        }
        let queries: Vec<_> = fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| r.query.get("wd").cloned())
            .collect();
        assert_eq!(queries, vec!["Same title", "已有别名"]);
        assert_eq!(host.requests.active_count(), 0);
    }
}
