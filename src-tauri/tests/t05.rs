use moc_video::{
    commands,
    contracts::{APP, COMMANDS, Contracts, authorized},
    host::Host,
    requests::{Clock, Requests, budgets},
    settings,
    source_dispatch::{SourceDispatch, identity},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tempfile::TempDir;
use uuid::Uuid;

const DEFAULT_SOURCES: &str = include_str!("../../src/data/defaultSources.json");
const CONFIG: &str = "11111111-1111-4111-8111-111111111111";
const SECOND: &str = "22222222-2222-4222-8222-222222222222";
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../../protocol/examples/contract-cases.json")).unwrap()
}
fn setup() -> (TempDir, Arc<Host>) {
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    (temp, host)
}
fn args(input: Value) -> Value {
    json!({"request_id": id(), "input": input})
}
fn call(host: &Host, command: &str, input: Value) -> Value {
    let request = args(input);
    let response = host.invoke(command, request.clone(), true);
    Contracts::get()
        .validate(&format!("Result_{command}"), &response)
        .unwrap();
    println!(
        "T05_OBSERVATION:{}",
        json!({"command": command, "args": request, "response": response})
    );
    response
}
fn ok(host: &Host, command: &str, input: Value) -> Value {
    let response = call(host, command, input);
    assert!(response.get("error").is_none(), "{response}");
    response["data"].clone()
}
fn error(response: &Value, code: &str) {
    assert_eq!(response["error"]["code"], code, "{response}");
    assert!(response.get("data").is_none());
}
fn mac() -> Value {
    json!({"kind": "maccms", "config_id": CONFIG, "name": "匿名接口", "enabled": true, "api_url": "https://maccms.fixture.invalid/api.php/provide/vod/?q=a%2Fb&x=1"})
}
fn stremio() -> Value {
    json!({"kind": "stremio", "config_id": SECOND, "name": "Metadata", "enabled": true, "manifest_urls": ["https://catalog.fixture.invalid/config/manifest.json?lang=zh-CN"]})
}
fn configured(host: &Host, configs: Vec<Value>) -> Value {
    let mut settings = ok(host, "get_settings", json!({}));
    settings["source_configs"] = json!(configs);
    ok(host, "save_settings", settings)
}

#[test]
fn t05_contract_commands_and_envelopes() {
    let (_temp, host) = setup();
    let schema: Value = serde_json::from_str(APP).unwrap();
    assert_eq!(COMMANDS.len(), 38);
    assert!(COMMANDS.contains(&"get_media_cache"));
    assert!(COMMANDS.contains(&"clear_media_cache"));
    for command in [
        "get_live_channels",
        "open_live",
        "list_local_videos",
        "open_local",
        "start_line_round",
        "get_line_round",
        "confirm_line_candidate",
        "cancel_line_round",
        "play_line",
        "refresh_line",
        "report_line_playback",
    ] {
        assert!(COMMANDS.contains(&command));
    }
    assert_eq!(
        schema["x-command-contracts"].as_object().unwrap().len(),
        COMMANDS.len()
    );
    let cases = fixture()["cases"].as_array().unwrap().clone();
    for command in COMMANDS {
        assert!(schema["x-command-contracts"].get(command).is_some());
        let case = cases
            .iter()
            .find(|case| case["name"] == format!("args_{command}"))
            .unwrap();
        let response = host.invoke(command, case["value"].clone(), true);
        Contracts::get()
            .validate(&format!("Result_{command}"), &response)
            .unwrap();
        println!(
            "T05_OBSERVATION:{}",
            json!({"command": command, "args": case["value"], "response": response})
        );
        let mut bad = case["value"].clone();
        bad["input"]["unexpected"] = json!(true);
        error(&host.invoke(command, bad, true), "BAD_REQUEST");
        let mut drift = case["value"].clone();
        drift["requestId"] = drift["request_id"].clone();
        drift.as_object_mut().unwrap().remove("request_id");
        error(&host.invoke(command, drift, true), "BAD_REQUEST");
        let mut extra = case["value"].clone();
        extra["path"] = json!("arbitrary");
        error(&host.invoke(command, extra, true), "BAD_REQUEST");
        let mut invalid_id = case["value"].clone();
        invalid_id["request_id"] = json!("not-a-uuid");
        error(&host.invoke(command, invalid_id, true), "BAD_REQUEST");
    }
    for case in cases.iter().filter(|case| case["document"] == "app") {
        let schema = case["schema"].as_str().unwrap();
        assert_eq!(
            Contracts::get().validate(schema, &case["value"]).is_ok(),
            case["expected_valid"] == true,
            "{}",
            case["name"]
        );
    }
    error(&host.invoke("spawn", args(json!({})), true), "BAD_REQUEST");
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t05_http_only_preparation_and_gateway() {
    let (_temp, host) = setup();
    let before = ok(&host, "runtime_status", json!({}));
    assert_eq!(before["phase"], "checking");
    assert!(before["health"].is_null());
    let ready = ok(&host, "runtime_prepare", json!({}));
    assert_eq!(ready["phase"], "ready");
    assert_eq!(ready["health"]["media_gateway_ready"], true);
    assert_eq!(ready["engine_epoch"], host.host_epoch);
    assert_eq!(ready["cloudstream"]["phase"], "stopped");
    assert!(ready["cloudstream"]["engine_epoch"].is_null());
    let address = host.gateway_address().unwrap();
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .write_all(b"GET /host-health HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 401 "));
    assert_eq!(
        ok(&host, "runtime_prepare", json!({"target": "application"})),
        ready
    );
    error(
        &call(&host, "runtime_prepare", json!({"target": "cloudstream"})),
        "CAPABILITY_UNAVAILABLE",
    );
    assert_eq!(ok(&host, "runtime_status", json!({})), ready);
    // A fresh install seeds the bundled TMDB and MacCMS presets; no Cloudstream provider appears.
    let sources = ok(&host, "get_sources", json!({}));
    assert_eq!(sources["plugins"], json!([]));
    let presets: Vec<Value> = serde_json::from_str(DEFAULT_SOURCES).unwrap();
    let providers = sources["providers"].as_array().unwrap();
    assert_eq!(providers.len(), presets.len() + 1);
    for (provider, preset) in providers.iter().zip(&presets) {
        assert_eq!(provider["name"], preset["name"]);
        assert_eq!(provider["kind"], preset["kind"]);
        assert_ne!(provider["kind"], "cloudstream");
    }
    let tmdb = &providers[presets.len()];
    assert_eq!(tmdb["name"], "TMDB");
    assert_eq!(tmdb["kind"], "stremio");
    assert_eq!(tmdb["origin"]["config_id"], settings::TMDB_CONFIG_ID);
    assert_eq!(
        tmdb["origin"]["manifest_urls"],
        json!(["https://tmdb.elfhosted.com/en-US/manifest.json"])
    );
    drop(host);
    assert!(TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err());
}

#[test]
fn t05_settings_roundtrip_registry_and_identity() {
    let (temp, host) = setup();
    let mut settings = configured(&host, vec![mac(), stremio()]);
    settings["last_metadata_config_id"] = json!(SECOND);
    settings["last_playback_source_key"] = json!(identity("http_source", &["maccms", CONFIG]));
    let settings = ok(&host, "save_settings", settings);
    let sources = ok(&host, "get_sources", json!({}));
    assert_eq!(sources["plugins"], json!([]));
    let providers = sources["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 3);
    assert_eq!(
        providers[2]["origin"]["config_id"],
        settings::TMDB_CONFIG_ID
    );
    for (index, config) in [mac(), stremio()].iter().enumerate() {
        assert_eq!(providers[index]["kind"], config["kind"]);
        assert_eq!(providers[index]["runtime_status"], "configured");
        assert_eq!(
            providers[index]["id"],
            identity(
                "http_source",
                &[
                    config["kind"].as_str().unwrap(),
                    config["config_id"].as_str().unwrap()
                ]
            )
        );
        assert_eq!(providers[index]["origin"]["config_id"], config["config_id"]);
        assert!(
            providers[index]["capabilities"]
                .as_object()
                .unwrap()
                .values()
                .all(|value| value == false)
        );
    }
    assert_eq!(providers[1]["source_revision"], "unpublished");
    let old_epoch = host.host_epoch.clone();
    drop(host);
    let recovered = Host::open(temp.path().join("settings.json")).unwrap();
    assert_ne!(recovered.host_epoch, old_epoch);
    assert_eq!(ok(&recovered, "get_settings", json!({})), settings);
    assert_eq!(ok(&recovered, "get_sources", json!({})), sources);
    for vector in fixture()["http_identity_examples"].as_array().unwrap() {
        let kind = vector["kind"].as_str().unwrap();
        assert_eq!(
            identity(
                "http_source",
                &[kind, vector["config_id"].as_str().unwrap()]
            ),
            vector["source_id"]
        );
        let parts: Vec<_> = vector["revision_parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            identity(&format!("{kind}_revision"), &parts),
            vector["source_revision"]
        );
    }
}

#[test]
fn t05_settings_validation_and_failure_atomicity() {
    let (temp, host) = setup();
    let committed = configured(&host, vec![mac()]);
    let before = fs::read(temp.path().join("settings.json")).unwrap();
    for url in [
        "file:///etc/passwd",
        "ftp://fixture.invalid/a",
        "https://u:p@fixture.invalid/a",
        "https://fixture.invalid/a#secret",
        "https://fixture.invalid/ x",
        "https://fixture.invalid/\\x",
        "//fixture.invalid/a",
        "http:///missing",
    ] {
        let mut candidate = committed.clone();
        candidate["source_configs"][0]["api_url"] = json!(url);
        error(&call(&host, "save_settings", candidate), "BAD_REQUEST");
    }
    let mut duplicate = committed.clone();
    duplicate["source_configs"] = json!([mac(), mac()]);
    error(&call(&host, "save_settings", duplicate), "BAD_REQUEST");
    let mut first = mac();
    first["config_id"] = json!("abcdefab-abcd-4abc-8abc-abcdefabcdef");
    let mut second = stremio();
    second["config_id"] = json!("ABCDEFAB-ABCD-4ABC-8ABC-ABCDEFABCDEF");
    let mut duplicate_case = committed.clone();
    duplicate_case["source_configs"] = json!([first, second]);
    error(&call(&host, "save_settings", duplicate_case), "BAD_REQUEST");
    for (field, value) in [
        ("volume", json!(1.1)),
        ("volume", json!(-0.1)),
        ("schema_version", json!(3)),
        ("ui_locale", json!("ja")),
    ] {
        let mut bad = committed.clone();
        bad[field] = value;
        error(&call(&host, "save_settings", bad), "BAD_REQUEST");
    }
    assert_eq!(fs::read(temp.path().join("settings.json")).unwrap(), before);
    assert_eq!(ok(&host, "get_settings", json!({})), committed);
    let snapshot = host.capture_source(CONFIG, false).unwrap();
    // A directory occupying the target forces the real atomic rename to fail.
    fs::rename(
        temp.path().join("settings.json"),
        temp.path().join("confirmed.json"),
    )
    .unwrap();
    fs::create_dir(temp.path().join("settings.json")).unwrap();
    let mut edited = committed.clone();
    edited["source_configs"][0]["name"] = json!("changed");
    error(&call(&host, "save_settings", edited), "INTERNAL_ERROR");
    assert_eq!(ok(&host, "get_settings", json!({})), committed);
    assert_eq!(
        host.capture_source(CONFIG, false).unwrap().generation,
        snapshot.generation
    );
    assert_eq!(
        fs::read(temp.path().join("confirmed.json")).unwrap(),
        before
    );
    fs::remove_dir(temp.path().join("settings.json")).unwrap();
    fs::rename(
        temp.path().join("confirmed.json"),
        temp.path().join("settings.json"),
    )
    .unwrap();
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn t05_old_versions_corruption_and_unknown_versions_are_rejected() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings.json");
    let bytes = b"{\"schema_version\":1}";
    fs::write(&path, bytes).unwrap();
    assert!(Host::open(path.clone()).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    for bytes in [
        b"{damaged".as_slice(),
        b"{\"schema_version\":99}",
        b"{\"schema_version\":2}",
    ] {
        fs::write(&path, bytes).unwrap();
        assert!(Host::open(path.clone()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(
            fs::read_dir(temp.path())
                .unwrap()
                .filter_map(Result::ok)
                .any(
                    |entry| entry.file_name().to_string_lossy().contains("diagnostic-")
                        && fs::read(entry.path()).unwrap() == bytes
                )
        );
    }
}

#[test]
fn t05_configuration_limits_and_utf8_budgets() {
    let (_temp, host) = setup();
    let configs: Vec<_> = (0..32)
        .map(|_| {
            let mut config = mac();
            config["config_id"] = json!(id());
            config
        })
        .collect();
    let settings = configured(&host, configs);
    assert_eq!(settings["source_configs"].as_array().unwrap().len(), 32);
    assert_eq!(
        ok(&host, "get_sources", json!({}))["providers"]
            .as_array()
            .unwrap()
            .len(),
        33
    );
    let mut too_many = settings.clone();
    too_many["source_configs"]
        .as_array_mut()
        .unwrap()
        .push(mac());
    error(&call(&host, "save_settings", too_many), "BAD_REQUEST");
    let mut config = stremio();
    config["manifest_urls"] = json!(
        (0..16)
            .map(|index| format!("https://catalog.fixture.invalid/{index}/manifest.json"))
            .collect::<Vec<_>>()
    );
    let committed = configured(&host, vec![config]);
    for urls in [
        json!([]),
        json!([
            "https://catalog.fixture.invalid/manifest.json",
            "https://catalog.fixture.invalid/manifest.json"
        ]),
        json!(
            (0..17)
                .map(|index| format!("https://catalog.fixture.invalid/{index}/manifest.json"))
                .collect::<Vec<_>>()
        ),
    ] {
        let mut bad = committed.clone();
        bad["source_configs"][0]["manifest_urls"] = urls;
        error(&call(&host, "save_settings", bad), "BAD_REQUEST");
    }
    let mut wrong_kind = committed.clone();
    wrong_kind["source_configs"][0]["api_url"] = json!("https://fixture.invalid/api");
    error(&call(&host, "save_settings", wrong_kind), "BAD_REQUEST");
    let oversized = json!({"ref": {"source_id": "unknown", "source_revision": "unknown", "data": "界".repeat(200_000)}});
    error(&call(&host, "get_details", oversized), "RESOURCE_LIMIT");
    error(
        &host.invoke(
            "get_details",
            args(json!({"data": "x".repeat(1_048_577)})),
            true,
        ),
        "RESOURCE_LIMIT",
    );
    assert_eq!(ok(&host, "get_settings", json!({})), committed);
}

#[test]
fn t05_same_lock_commits_and_atomic_cancel() {
    let (temp, host) = setup();
    let gate = Arc::new(std::sync::Barrier::new(3));
    let mut threads = Vec::new();
    for (config, volume) in [(mac(), 0.2), (stremio(), 0.7)] {
        let host = host.clone();
        let gate = gate.clone();
        threads.push(std::thread::spawn(move || {
            let mut settings = settings::defaults();
            settings["source_configs"] = json!([config]);
            settings["volume"] = json!(volume);
            gate.wait();
            assert_eq!(ok(&host, "save_settings", settings.clone()), settings);
        }));
    }
    gate.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    let current = ok(&host, "get_settings", json!({}));
    let on_disk: Value =
        serde_json::from_slice(&fs::read(temp.path().join("settings.json")).unwrap()).unwrap();
    assert_eq!(current, on_disk);
    assert_eq!(
        ok(&host, "get_sources", json!({}))["providers"][0]["kind"],
        current["source_configs"][0]["kind"]
    );
    let (store, _) = settings::SettingsFile::load(temp.path().join("settings.json")).unwrap();
    let job_id = id();
    let job = host
        .requests
        .begin(&job_id, "default_metadata", host.clock.clone())
        .unwrap();
    let result = store.save_checked(&settings::defaults(), || {
        host.requests.cancel(&id(), &job_id)?;
        job.check()
    });
    assert_eq!(result.unwrap_err().code, "CANCELLED");
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(temp.path().join("settings.json")).unwrap())
            .unwrap(),
        current
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn t05_old_source_key_does_not_restore_role_preferences() {
    for config in [mac(), stremio()] {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("settings.json");
        let mut legacy = settings::defaults();
        let source_id = identity(
            "http_source",
            &[
                config["kind"].as_str().unwrap(),
                config["config_id"].as_str().unwrap(),
            ],
        );
        legacy["source_configs"] = json!([config]);
        legacy["last_source_key"] = json!(source_id);
        fs::write(&path, legacy.to_string()).unwrap();
        let recovered = Host::open(path).unwrap();
        let preferences = ok(&recovered, "get_settings", json!({}));
        assert!(preferences["last_metadata_config_id"].is_null());
        assert!(preferences["last_playback_source_key"].is_null());
        assert_eq!(preferences["last_source_key"], source_id);
    }
}

#[test]
fn t05_generation_tombstones_and_source_isolation() {
    let (_temp, host) = setup();
    let mut settings = configured(&host, vec![mac(), stremio()]);
    let first = host.capture_source(CONFIG, true).unwrap();
    let unaffected = host.capture_source(SECOND, false).unwrap();
    let old_work = host
        .requests
        .begin(&id(), "http_source_sync", host.clock.clone())
        .unwrap();
    old_work.bind_source(&first.source_id);
    let other_work = host
        .requests
        .begin(&id(), "default_metadata", host.clock.clone())
        .unwrap();
    other_work.bind_source(&unaffected.source_id);
    settings["source_configs"][0]["enabled"] = json!(false);
    ok(&host, "save_settings", settings.clone());
    assert_eq!(old_work.check().unwrap_err().code, "CANCELLED");
    other_work.check().unwrap();
    assert!(host.capture_source(CONFIG, false).is_err());
    settings["source_configs"] = json!([stremio()]);
    ok(&host, "save_settings", settings.clone());
    settings["source_configs"] = json!([mac(), stremio()]);
    ok(&host, "save_settings", settings);
    let added = host.capture_source(CONFIG, true).unwrap();
    assert!(added.generation > first.generation);
    assert_eq!(
        host.capture_source(SECOND, false).unwrap().generation,
        unaffected.generation
    );
    let source = ok(&host, "get_sources", json!({}))["providers"][0].clone();
    let job = host
        .requests
        .begin(&id(), "http_source_sync", host.clock.clone())
        .unwrap();
    assert_eq!(
        host.publish_source(&first, source.clone(), &job)
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    let mut foreign = added.clone();
    foreign.host_epoch = id();
    assert_eq!(
        host.publish_source(&foreign, source.clone(), &job)
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    let mut guest = added.clone();
    guest.guest_epoch = Some(id());
    assert_eq!(
        host.publish_source(&guest, source.clone(), &job)
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    assert_eq!(ok(&host, "get_sources", json!({}))["providers"][0], source);
}

#[test]
fn t05_sync_sequence_rejects_late_publication() {
    let (_temp, host) = setup();
    configured(&host, vec![stremio()]);
    let older = host.capture_source(SECOND, true).unwrap();
    let newer = host.capture_source(SECOND, true).unwrap();
    assert!(newer.sync_sequence > older.sync_sequence);
    let job = host
        .requests
        .begin(&id(), "http_source_sync", host.clock.clone())
        .unwrap();
    let mut source = ok(&host, "get_sources", json!({}))["providers"][0].clone();
    source["source_revision"] = json!("new-complete-manifest");
    source["runtime_status"] = json!("ready");
    source["capabilities"]["home"] = json!(true);
    source["capabilities"]["details"] = json!(true);
    host.publish_source(&newer, source.clone(), &job).unwrap();
    let mut old_source = source.clone();
    old_source["source_revision"] = json!("late-old-manifest");
    assert_eq!(
        host.publish_source(&older, old_source, &job)
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    assert_eq!(ok(&host, "get_sources", json!({}))["providers"][0], source);
    let mut drift = source.clone();
    drift["origin"]["config_id"] = json!(CONFIG);
    assert!(host.publish_source(&newer, drift, &job).is_err());
    assert_eq!(ok(&host, "get_sources", json!({}))["providers"][0], source);
}

#[test]
fn t05_uuid_duplicates_self_cancel_and_cascade() {
    let (_temp, host) = setup();
    let parent_id = id();
    let parent = host
        .requests
        .begin(&parent_id, "http_source_sync", host.clock.clone())
        .unwrap();
    assert_eq!(
        host.requests
            .begin(
                &parent_id.to_uppercase(),
                "http_source_sync",
                host.clock.clone()
            )
            .err()
            .unwrap()
            .code,
        "BAD_REQUEST"
    );
    parent.check().unwrap();
    let child = parent.child(&id(), "default_metadata").unwrap();
    let grandchild = child.child(&id(), "default_metadata").unwrap();
    error(
        &host.invoke(
            "cancel_request",
            json!({"request_id": parent_id, "input": {"target_request_id": parent_id}}),
            true,
        ),
        "BAD_REQUEST",
    );
    parent.check().unwrap();
    error(
        &host.invoke(
            "runtime_status",
            json!({"request_id": parent_id, "input": {}}),
            true,
        ),
        "BAD_REQUEST",
    );
    let result = ok(
        &host,
        "cancel_request",
        json!({"target_request_id": parent_id}),
    );
    assert_eq!(result["cancelled"], true);
    for job in [&parent, &child, &grandchild] {
        assert_eq!(job.check().unwrap_err().code, "CANCELLED");
    }
    drop((parent, child, grandchild));
    assert_eq!(host.requests.active_count(), 0);
    assert_eq!(
        ok(
            &host,
            "cancel_request",
            json!({"target_request_id": parent_id})
        )["cancelled"],
        false
    );
    assert!(
        host.requests
            .begin("invalid", "default_metadata", host.clock.clone())
            .is_err()
    );
}

#[derive(Default)]
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[test]
fn t05_deadlines_include_queue_and_discard_old_work() {
    let temp = TempDir::new().unwrap();
    let clock = Arc::new(TestClock::default());
    let host = Host::with_clock(temp.path().join("settings.json"), clock.clone()).unwrap();
    configured(&host, vec![mac()]);
    let captured = host.capture_source(CONFIG, true).unwrap();
    let before = ok(&host, "get_sources", json!({}));
    let late = host
        .requests
        .begin(&id(), "http_source_sync", clock.clone())
        .unwrap();
    assert_eq!(late.child_timeout_ms().unwrap(), 15_000);
    assert_eq!(late.connect_timeout_ms().unwrap(), 10_000);
    assert_eq!(late.aggregate_remaining_ms().unwrap(), 44_000);
    let child = late.child(&id(), "runtime_prepare").unwrap();
    assert_eq!(child.remaining_ms().unwrap(), 15_000);
    clock.0.store(44_000, Ordering::SeqCst);
    assert_eq!(late.aggregate_remaining_ms().unwrap(), 0);
    assert_eq!(child.check().unwrap_err().code, "CANCELLED");
    assert_eq!(late.child_timeout_ms().unwrap(), 1_000);
    let queued = host
        .admit("save_settings", args(settings::defaults()), true)
        .ok()
        .unwrap();
    clock.0.store(75_000, Ordering::SeqCst);
    error(&host.execute(queued), "UPSTREAM_TIMEOUT");
    assert_eq!(
        host.publish_source(&captured, before["providers"][0].clone(), &late)
            .unwrap_err()
            .code,
        "UPSTREAM_TIMEOUT"
    );
    assert_eq!(ok(&host, "get_sources", json!({})), before);
    let requests = Arc::new(Requests::default());
    let expected = [
        ("default_metadata", 30_000, 30_000, 1),
        ("cloudstream_home", 60_000, 15_000, 1),
        ("cloudstream_home_page", 30_000, 15_000, 1),
        ("maccms_home", 45_000, 30_000, 4),
        ("stremio_home", 45_000, 30_000, 4),
        ("http_source_sync", 45_000, 15_000, 4),
        ("resolve", 45_000, 45_000, 1),
        ("match_playback", 45_000, 15_000, 4),
        ("health", 3_000, 3_000, 1),
        ("get_resource_status", 3_000, 3_000, 1),
        ("runtime_prepare", 120_000, 120_000, 1),
    ];
    assert_eq!(budgets().len(), expected.len());
    for (name, total, child_limit, concurrency) in expected {
        let budget = budgets()[name];
        assert_eq!(
            (budget.total_ms, budget.child_ms, budget.max_concurrency),
            (total, child_limit, concurrency)
        );
        let started = clock.now_ms();
        let job = requests.begin(&id(), name, clock.clone()).unwrap();
        assert_eq!(job.remaining_ms().unwrap(), total);
        let child = job.child(&id(), "runtime_prepare").unwrap();
        assert!(child.remaining_ms().unwrap() <= budget.child_ms);
        clock.0.store(started + total - 1, Ordering::SeqCst);
        assert_eq!(job.remaining_ms().unwrap(), 1);
        clock.0.store(started + total, Ordering::SeqCst);
        assert_eq!(job.check().unwrap_err().code, "UPSTREAM_TIMEOUT");
    }
}

#[test]
fn t05_cancelled_queued_settings_never_commit() {
    let (temp, host) = setup();
    let baseline = configured(&host, vec![mac()]);
    let request = args(settings::defaults());
    let queued = host
        .admit("save_settings", request.clone(), true)
        .ok()
        .unwrap();
    assert_eq!(
        ok(
            &host,
            "cancel_request",
            json!({"target_request_id": request["request_id"]})
        )["cancelled"],
        true
    );
    assert_eq!(ok(&host, "get_settings", json!({})), baseline);
    error(&host.execute(queued), "CANCELLED");
    assert_eq!(ok(&host, "get_settings", json!({})), baseline);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(temp.path().join("settings.json")).unwrap())
            .unwrap(),
        baseline
    );
}

#[test]
fn t05_unknown_sources_revisions_and_kind_rules() {
    let (_temp, host) = setup();
    configured(&host, vec![mac(), stremio()]);
    error(
        &call(
            &host,
            "get_home",
            json!({"source_id": "unknown", "source_revision": "unknown", "page": 1}),
        ),
        "SOURCE_NOT_FOUND",
    );
    let sources = ok(&host, "get_sources", json!({}));
    let source = &sources["providers"][0];
    error(
        &call(
            &host,
            "get_home",
            json!({"source_id": source["id"], "source_revision": "old-revision", "page": 1}),
        ),
        "REVISION_MISMATCH",
    );
    error(
        &call(
            &host,
            "get_home",
            json!({"source_id": source["id"], "source_revision": source["source_revision"], "page": 2}),
        ),
        "BAD_REQUEST",
    );
    error(
        &call(
            &host,
            "sync_catalog",
            json!({"kind": "stremio", "config_id": CONFIG}),
        ),
        "BAD_REQUEST",
    );
    error(
        &call(
            &host,
            "sync_catalog",
            json!({"kind": "maccms", "config_id": id()}),
        ),
        "SOURCE_NOT_FOUND",
    );
    let meta = &sources["providers"][1];
    error(
        &call(
            &host,
            "resolve_episode",
            json!({"episode_ref": {"source_id": meta["id"], "source_revision": meta["source_revision"], "data": "opaque"}, "match_id": "match-fixture"}),
        ),
        "CAPABILITY_UNAVAILABLE",
    );
}

#[test]
fn t05_permissions_and_actual_tauri_handler() {
    for (label, url) in [
        ("secondary", "tauri://localhost"),
        ("main", "https://fixture.invalid"),
        ("main", "file:///tmp/a"),
        ("main", "http://tauri.localhost:1234"),
        ("main", "http://user@tauri.localhost"),
    ] {
        assert!(!authorized(label, &url.parse().unwrap()));
    }
    let (_temp, host) = setup();
    for command in COMMANDS {
        error(
            &host.invoke(command, args(json!({})), false),
            "UNAUTHORIZED",
        );
    }
    let app = tauri::test::mock_builder()
        .manage(host)
        .invoke_handler(commands::handler())
        .build(tauri::tauri_build_context!())
        .unwrap();
    let main =
        tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::App("index.html".into()))
            .build()
            .unwrap();
    let secondary = tauri::WebviewWindowBuilder::new(
        &app,
        "secondary",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .build()
    .unwrap();
    for command in COMMANDS {
        let case = fixture()["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == format!("args_{command}"))
            .unwrap()
            .clone();
        let invoke = || tauri::webview::InvokeRequest {
            cmd: (*command).into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(windows) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: tauri::ipc::InvokeBody::Json(case["value"].clone()),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        };
        let response = tauri::test::get_ipc_response(&main, invoke());
        if matches!(*command, "match_playback" | "resolve_episode") {
            assert!(
                response.is_err(),
                "legacy playback permission must be absent"
            );
            continue;
        }
        let response: Value = response.unwrap().deserialize().unwrap();
        Contracts::get()
            .validate(&format!("Result_{command}"), &response)
            .unwrap();
        assert_ne!(response["error"]["code"], "UNAUTHORIZED");
        let denied = tauri::test::get_ipc_response(&secondary, invoke());
        match denied {
            Err(_) => {}
            Ok(response) => error(&response.deserialize::<Value>().unwrap(), "UNAUTHORIZED"),
        }
    }
}

#[test]
fn t05_ready_and_field_drift_negatives() {
    let (_temp, host) = setup();
    let ready = call(&host, "runtime_prepare", json!({}));
    for (pointer, value) in [
        ("/engine_epoch", Value::Null),
        ("/data/health", Value::Null),
        ("/data/health/status", json!("initializing")),
        ("/data/health/media_gateway_ready", json!(false)),
        ("/data/engine_epoch", json!("invalid")),
    ] {
        let mut broken = ready.clone();
        *broken.pointer_mut(pointer).unwrap() = value;
        assert!(
            Contracts::get()
                .validate("Result_runtime_prepare", &broken)
                .is_err()
        );
    }
    let mut broken = ready;
    broken["error"] = json!({"code": "BAD_REQUEST", "message": "both", "retryable": false});
    assert!(
        Contracts::get()
            .validate("Result_runtime_prepare", &broken)
            .is_err()
    );
    let mut dispatch = SourceDispatch::default();
    dispatch.replace_configs(&[stremio()]);
    let mut source = dispatch.sources()["providers"][0].clone();
    source["capabilities"]["resolve"] = json!(true);
    assert!(Contracts::get().validate("AppSource", &source).is_err());
}
