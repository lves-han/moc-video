//! T0.18: SQLite history through the production Host and real playback Sessions.
use super::*;
use rusqlite::Connection;

struct Playing {
    _fixture: Fixture,
    temp: TempDir,
    host: Arc<Host>,
    context: Value,
    view: Value,
}

fn playing() -> Playing {
    let fixture = gateway_fixture(Arc::new(AtomicBool::new(false)));
    let (temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    Playing {
        _fixture: fixture,
        temp,
        host,
        context,
        view,
    }
}

fn record(context: &Value, position: u64, updated: u64) -> Value {
    let identity = &context["metadata_ref"]["identity"];
    let text = |field: &str| identity[field].as_str().unwrap().to_owned();
    json!({"content_id":crate::source_dispatch::identity("metadata",&[&text("namespace"),&text("type"),&text("canonical_id")]),
        "metadata_identity":identity,"video_id":context["metadata_video_id"],"season_number":0,"episode_number":0.5,
        "title":"引号'; DROP TABLE watch_history; --","kind":"series","poster_cache_key":null,
        "source_id":context["episode_ref"]["source_id"],"source_name":"Playback fixture","position_ms":position,
        "duration_ms":60000,"completed":false,"updated_at":updated})
}

fn save_input(playing: &Playing, record: Value, sequence: u64) -> Value {
    json!({"record":record,"session_id":playing.view["session_id"],"match_id":playing.context["match_id"],
        "playback_epoch":3,"write_sequence":sequence})
}

fn listed(host: &Host) -> Value {
    ok(host, "list_history", json!({}))
}

fn database(playing: &Playing) -> std::path::PathBuf {
    playing.temp.path().join("moc-video.db")
}

#[test]
fn t18_sqlite_file_columns_index_restart_and_parameter_binding() {
    let playing = playing();
    assert_eq!(listed(&playing.host)["write_sequence"], 0);
    assert_eq!(listed(&playing.host)["entries"], json!([]));
    let saved = record(&playing.context, 1000, 10);
    assert_eq!(
        ok(
            &playing.host,
            "save_history",
            save_input(&playing, saved.clone(), 1)
        ),
        saved
    );
    let connection = Connection::open(database(&playing)).unwrap();
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
    let mut statement = connection
        .prepare("PRAGMA table_info(watch_history)")
        .unwrap();
    let columns: Vec<String> = statement
        .query_map([], |row| row.get(1))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(columns.len(), 19);
    for column in [
        "metadata_addon_key",
        "canonical_id",
        "video_id",
        "episode_number",
        "poster_cache_key",
        "completed",
        "updated_at",
    ] {
        assert!(columns.iter().any(|name| name == column), "{column}");
    }
    let index: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='watch_history_recent'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(index, 1);
    let path = playing.temp.path().join("settings.json");
    let restarted = Host::open(path).unwrap();
    let after = listed(&restarted);
    assert_eq!(
        after["entries"],
        json!([saved]),
        "survives a restart and the quoted title stays text"
    );
    assert_eq!(
        after["write_sequence"], 0,
        "a new Host starts its own sequence"
    );
}

#[test]
fn t18_migration_failure_and_newer_version_keep_the_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("中文 目录").join("settings.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let database = path.with_file_name("moc-video.db");
    let connection = Connection::open(&database).unwrap();
    // A half-made table makes version 1 fail; the transaction must leave these rows.
    connection
        .execute_batch(
            "CREATE TABLE watch_history (kept TEXT); INSERT INTO watch_history VALUES ('原数据');",
        )
        .unwrap();
    drop(connection);
    let host = Host::open(path.clone()).unwrap();
    let result = call(&host, "list_history", json!({}));
    error(&result, "INTERNAL_ERROR");
    assert!(
        !result.to_string().contains("中文"),
        "no local path in errors"
    );
    let connection = Connection::open(&database).unwrap();
    let kept: String = connection
        .query_row("SELECT kept FROM watch_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(kept, "原数据");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 0);
    connection
        .execute_batch("DROP TABLE watch_history; PRAGMA user_version = 9;")
        .unwrap();
    drop(connection);
    let host = Host::open(path).unwrap();
    let result = call(&host, "list_history", json!({}));
    error(&result, "INTERNAL_ERROR");
    assert_eq!(result["error"]["details"]["reason"], "newer_version");
    assert_eq!(result["error"]["retryable"], false);
}

#[test]
fn t18_sequences_barriers_and_identical_retry() {
    let playing = playing();
    let host = &playing.host;
    let first = record(&playing.context, 1000, 50);
    ok(host, "save_history", save_input(&playing, first.clone(), 5));
    // An older sequence loses even with a newer clock; a newer one wins with an older clock.
    let stale = record(&playing.context, 9000, 99);
    error(
        &call(host, "save_history", save_input(&playing, stale, 5)),
        "CANCELLED",
    );
    error(
        &call(
            host,
            "save_history",
            save_input(&playing, record(&playing.context, 9000, 99), 4),
        ),
        "CANCELLED",
    );
    assert_eq!(listed(host)["entries"], json!([first]));
    let rolled_back = record(&playing.context, 2000, 7);
    ok(
        host,
        "save_history",
        save_input(&playing, rolled_back.clone(), 6),
    );
    assert_eq!(listed(host)["entries"], json!([rolled_back]));
    assert_eq!(listed(host)["write_sequence"], 6);

    // A locked database fails the write; only the identical payload may retry that sequence.
    let locker = Connection::open(database(&playing)).unwrap();
    locker.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    let pending = save_input(&playing, record(&playing.context, 3000, 60), 7);
    let result = call(host, "save_history", pending.clone());
    error(&result, "INTERNAL_ERROR");
    assert_eq!(result["error"]["details"]["reason"], "busy");
    assert_eq!(result["error"]["retryable"], true);
    let different = save_input(&playing, record(&playing.context, 3500, 60), 7);
    error(&call(host, "save_history", different), "BAD_REQUEST");
    locker.execute_batch("COMMIT;").unwrap();
    ok(
        host,
        "close_playback",
        json!({"session_id":playing.view["session_id"]}),
    );
    // Already accepted: the retry commits even though its Session has since closed.
    ok(host, "save_history", pending.clone());
    assert_eq!(listed(host)["entries"][0]["position_ms"], 3000);
    // The Session is closed, so a new snapshot was never accepted.
    error(
        &call(
            host,
            "save_history",
            save_input(&playing, record(&playing.context, 4000, 61), 8),
        ),
        "SESSION_EXPIRED",
    );
    assert_eq!(listed(host)["entries"][0]["position_ms"], 3000);
    error(&call(host, "save_history", pending), "CANCELLED");
}

#[test]
fn t18_delete_and_clear_block_older_writes_from_returning() {
    let playing = playing();
    let host = &playing.host;
    let saved = record(&playing.context, 1000, 10);
    let id = saved["content_id"].clone();
    ok(host, "save_history", save_input(&playing, saved.clone(), 1));
    // A write that failed before the delete must not come back afterwards.
    let locker = Connection::open(database(&playing)).unwrap();
    locker.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    let late = save_input(&playing, saved.clone(), 2);
    error(&call(host, "save_history", late.clone()), "INTERNAL_ERROR");
    locker.execute_batch("COMMIT;").unwrap();
    assert_eq!(
        ok(
            host,
            "remove_history",
            json!({"content_id":id,"write_sequence":3})
        ),
        json!({"ok":true})
    );
    assert_eq!(listed(host)["entries"], json!([]));
    error(&call(host, "save_history", late), "CANCELLED");
    assert_eq!(listed(host)["entries"], json!([]));
    assert_eq!(
        ok(
            host,
            "remove_history",
            json!({"content_id":id,"write_sequence":4})
        ),
        json!({"ok":true})
    );
    error(
        &call(
            host,
            "remove_history",
            json!({"content_id":id,"write_sequence":4}),
        ),
        "CANCELLED",
    );
    // A new capture with a higher sequence may write again.
    ok(host, "save_history", save_input(&playing, saved, 5));
    assert_eq!(listed(host)["entries"].as_array().unwrap().len(), 1);
    ok(host, "clear_history", json!({"write_sequence":6}));
    assert_eq!(listed(host)["entries"], json!([]));
    ok(host, "clear_history", json!({"write_sequence":7}));
    error(
        &call(host, "clear_history", json!({"write_sequence":7})),
        "CANCELLED",
    );
}

#[test]
fn t18_snapshot_must_match_the_session_identity() {
    let playing = playing();
    let host = &playing.host;
    let good = record(&playing.context, 1000, 10);
    let attempts: Vec<(&str, Value)> = vec![
        ("match", json!({"match_id":"a-legal-but-different-match"})),
        ("session", json!({"session_id":"unknown-session"})),
    ];
    for (name, change) in attempts {
        let mut input = save_input(&playing, good.clone(), 1);
        for (key, value) in change.as_object().unwrap() {
            input[key] = value.clone();
        }
        let result = call(host, "save_history", input);
        assert!(result.get("error").is_some(), "{name}: {result}");
    }
    let wrong = |field: &str, value: Value| {
        let mut changed = good.clone();
        changed[field] = value;
        error(
            &call(host, "save_history", save_input(&playing, changed, 1)),
            "BAD_REQUEST",
        );
    };
    wrong("video_id", json!("another-video"));
    wrong("source_id", json!("another-source"));
    wrong("content_id", json!("f".repeat(64)));
    let mut identity = good["metadata_identity"].clone();
    identity["meta_id"] = json!("another-title");
    wrong("metadata_identity", identity);
    assert_eq!(
        listed(host)["entries"],
        json!([]),
        "nothing rejected reached the database"
    );
    ok(host, "save_history", save_input(&playing, good.clone(), 1));
    // The Session's first accepted epoch is the only one it takes.
    let mut old_epoch = save_input(&playing, good, 2);
    old_epoch["playback_epoch"] = json!(2);
    error(&call(host, "save_history", old_epoch), "BAD_REQUEST");
}

#[test]
fn t18_active_session_keeps_saving_after_match_records_are_collected() {
    let delay = Arc::new(AtomicBool::new(false));
    let fixture = gateway_fixture(delay.clone());
    let clock = Arc::new(TestClock(AtomicU64::new(1)));
    let (temp, host, provider, _, input) = setup_both(&fixture, clock.clone());
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    // An open stream keeps the Session active while the match and resolution leases run out.
    delay.store(true, Ordering::SeqCst);
    let parsed = url::Url::parse(view["media_url"].as_str().unwrap()).unwrap();
    let mut client = TcpStream::connect(("127.0.0.1", parsed.port().unwrap())).unwrap();
    let before = fixture.requests.lock().unwrap().len();
    write!(
        client,
        "GET {}?{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        parsed.path(),
        parsed.query().unwrap()
    )
    .unwrap();
    let start = Instant::now();
    while fixture.requests.lock().unwrap().len() == before {
        assert!(start.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
    clock
        .0
        .fetch_add(crate::matching::LEASE_MS + 1_000, Ordering::SeqCst);
    let stale = call(&host, "resolve_episode", resolve_input(&context));
    assert!(
        stale.get("error").is_some(),
        "the match has expired: {stale}"
    );
    let playing = Playing {
        _fixture: fixture,
        temp,
        host,
        context,
        view,
    };
    ok(
        &playing.host,
        "save_history",
        save_input(&playing, record(&playing.context, 1000, 10), 1),
    );
    assert_eq!(listed(&playing.host)["entries"][0]["position_ms"], 1000);
    delay.store(false, Ordering::SeqCst);
    drop(client);
}

#[test]
fn t18_non_uuid_match_id_is_carried_and_bad_handles_are_refused() {
    let mut playing = playing();
    let session = playing.view["session_id"].as_str().unwrap().to_owned();
    let legal = "host-match-not-a-uuid";
    playing
        .host
        .committed
        .lock()
        .unwrap()
        .session_bindings
        .get_mut(&session)
        .unwrap()
        .context["match_id"] = json!(legal);
    playing.context["match_id"] = json!(legal);
    let saved = record(&playing.context, 1000, 10);
    ok(
        &playing.host,
        "save_history",
        save_input(&playing, saved.clone(), 1),
    );
    for bad in [json!(""), json!("m".repeat(161)), json!("unknown-handle")] {
        let mut input = save_input(&playing, saved.clone(), 2);
        input["match_id"] = bad;
        let args = json!({"request_id":id(),"input":input});
        let result = playing.host.invoke("save_history", args, true);
        assert_eq!(result["error"]["code"], "BAD_REQUEST", "{result}");
    }
    assert_eq!(
        listed(&playing.host)["entries"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn t18_unusable_database_path_reports_and_recovers_with_the_same_snapshot() {
    let playing = playing();
    let path = database(&playing);
    std::fs::create_dir(&path).unwrap();
    let pending = save_input(&playing, record(&playing.context, 1000, 10), 1);
    let result = call(&playing.host, "save_history", pending.clone());
    error(&result, "INTERNAL_ERROR");
    assert_eq!(
        result["error"]["details"]["reason"], "permission",
        "{result}"
    );
    assert!(
        !result
            .to_string()
            .contains(playing.temp.path().to_str().unwrap())
    );
    error(
        &call(&playing.host, "list_history", json!({})),
        "INTERNAL_ERROR",
    );
    std::fs::remove_dir(&path).unwrap();
    ok(&playing.host, "save_history", pending);
    assert_eq!(
        listed(&playing.host)["entries"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn t18_every_close_request_waits_for_the_same_save() {
    use crate::closing::{IDLE, RELEASED, SAVING, request, unanswered};
    let state = std::sync::atomic::AtomicU8::new(IDLE);
    assert_eq!(
        request(&state),
        (false, true),
        "the first request asks the page to save"
    );
    assert_eq!(
        request(&state),
        (false, false),
        "a repeat while signalled keeps waiting"
    );
    state.store(SAVING, Ordering::SeqCst);
    assert!(
        !unanswered(&state),
        "an answering page is never cut off by the timer"
    );
    assert_eq!(
        request(&state),
        (false, false),
        "a repeat while saving keeps waiting"
    );
    state.store(IDLE, Ordering::SeqCst);
    assert_eq!(
        request(&state),
        (false, true),
        "after a failed save the next close starts over"
    );
    assert!(unanswered(&state), "a page that never answers is released");
    assert_eq!(state.load(Ordering::SeqCst), RELEASED);
    assert_eq!(request(&state), (true, false));
}

#[test]
fn t18_window_bounds_return_after_a_restart() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings.json");
    let host = Host::open(path.clone()).unwrap();
    assert_eq!(host.window_bounds(), None);
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["window_bounds"] = json!({"x":-40,"y":25,"width":1280,"height":720,"maximized":false});
    ok(&host, "save_settings", settings.clone());
    drop(host);
    let host = Host::open(path).unwrap();
    assert_eq!(
        host.window_bounds(),
        Some(crate::host::WindowBounds {
            x: -40.0,
            y: 25.0,
            width: 1280.0,
            height: 720.0,
            maximized: false
        })
    );
    settings["window_bounds"]["width"] = json!(300);
    error(&call(&host, "save_settings", settings), "BAD_REQUEST");
}

#[test]
fn t18_source_change_stops_new_snapshots_but_keeps_stored_history() {
    let playing = playing();
    let host = &playing.host;
    let saved = record(&playing.context, 1000, 10);
    ok(host, "save_history", save_input(&playing, saved.clone(), 1));
    let mut settings = ok(host, "get_settings", json!({}));
    settings["source_configs"][0]["enabled"] = json!(false);
    ok(host, "save_settings", settings);
    error(
        &call(
            host,
            "save_history",
            save_input(&playing, record(&playing.context, 2000, 20), 2),
        ),
        "SESSION_EXPIRED",
    );
    assert_eq!(listed(host)["entries"], json!([saved]));
}

#[test]
fn t18_fifty_switches_release_every_session_and_request() {
    let playing = playing();
    let host = &playing.host;
    let mut previous = playing.view.clone();
    let mut sequence = 0;
    for position in 0..50_u64 {
        sequence += 1;
        ok(
            host,
            "save_history",
            json!({"record":record(&playing.context, position + 1, position + 1),
            "session_id":previous["session_id"],"match_id":playing.context["match_id"],"playback_epoch":3,"write_sequence":sequence}),
        );
        let resolution = ok(host, "resolve_episode", resolve_input(&playing.context));
        let view = ok(host, "open_playback", open_input(&resolution));
        ok(
            host,
            "close_playback",
            json!({"session_id":previous["session_id"]}),
        );
        ok(
            host,
            "release_resolution",
            json!({"resolution_id":resolution["id"]}),
        );
        previous = view;
    }
    ok(
        host,
        "close_playback",
        json!({"session_id":previous["session_id"]}),
    );
    assert_eq!(listed(host)["entries"][0]["position_ms"], 50);
    assert_eq!(host.requests.active_count(), 0);
    assert!(host.committed.lock().unwrap().session_bindings.is_empty());
}
