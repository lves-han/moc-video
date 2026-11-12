//! TV5 durable watchlist, transactional v1 upgrade, and a production Host for WKWebView.
use super::*;
use rusqlite::{Connection, params};
#[path = "tv7_host.rs"]
mod tv7;

fn entry5() -> Value {
    let id = crate::source_dispatch::identity("metadata", &["tmdb", "series", "1"]);
    json!({"content_id":id,"metadata_identity":{"config_id":METADATA_CONFIG,"addon_key":"a".repeat(64),
        "namespace":"tmdb","type":"series","meta_id":"tmdb:1","canonical_id":"1"},
        "title":"Title'; DROP TABLE watch_history; --","kind":"series","poster_cache_key":null})
}
fn history5() -> Value {
    let mut record = entry5();
    record.as_object_mut().unwrap().extend(json!({"video_id":"video:1:3","season_number":1,"episode_number":3.0,
        "source_id":"fixture","source_name":"Fixture","position_ms":7000,"duration_ms":20000,"completed":false,"updated_at":1000}).as_object().unwrap().clone());
    record
}
fn legacy5(path: &std::path::Path) {
    let c = Connection::open(path).unwrap();
    // Frozen v1 layout: this file is authored independently of the current migration.
    c.execute_batch("CREATE TABLE watch_history (
        content_id TEXT PRIMARY KEY NOT NULL, metadata_config_id TEXT NOT NULL, metadata_addon_key TEXT NOT NULL,
        metadata_namespace TEXT NOT NULL, metadata_type TEXT NOT NULL, metadata_id TEXT NOT NULL, canonical_id TEXT NOT NULL,
        video_id TEXT, season_number INTEGER, episode_number REAL, title TEXT NOT NULL, kind TEXT NOT NULL, poster_cache_key TEXT,
        source_id TEXT NOT NULL, source_name TEXT NOT NULL, position_ms INTEGER NOT NULL CHECK(position_ms>=0),
        duration_ms INTEGER CHECK(duration_ms IS NULL OR duration_ms>0), completed INTEGER NOT NULL CHECK(completed IN(0,1)), updated_at INTEGER NOT NULL);
        CREATE INDEX watch_history_recent ON watch_history(updated_at DESC,content_id); PRAGMA user_version=1;").unwrap();
    let r = history5();
    let i = &r["metadata_identity"];
    c.execute("INSERT INTO watch_history VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![r["content_id"].as_str(),i["config_id"].as_str(),i["addon_key"].as_str(),i["namespace"].as_str(),i["type"].as_str(),
            i["meta_id"].as_str(),i["canonical_id"].as_str(),"video:1:3",1,3,r["title"].as_str(),"series",Option::<String>::None,"fixture","Fixture",7000,20000,0,1000]).unwrap();
}
fn version5(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}
#[test]
fn tv5_watchlist_crud_restart_and_closed_inputs() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings.json");
    let host = Host::open(path.clone()).unwrap();
    let entry = entry5();
    assert_eq!(
        ok(&host, "list_watchlist", json!({})),
        json!({"entries":[]})
    );
    assert_eq!(ok(&host, "add_watchlist", entry.clone()), entry);
    assert_eq!(ok(&host, "add_watchlist", entry.clone()), entry);
    assert_eq!(
        ok(&host, "list_watchlist", json!({})),
        json!({"entries":[entry.clone()]})
    );
    let restarted = Host::open(path).unwrap();
    assert_eq!(
        ok(&restarted, "list_watchlist", json!({})),
        json!({"entries":[entry.clone()]})
    );
    assert_eq!(
        ok(&restarted, "list_history", json!({}))["entries"],
        json!([])
    );
    let mut invalid = entry.clone();
    invalid["content_id"] = json!("b".repeat(64));
    error(&call(&restarted, "add_watchlist", invalid), "BAD_REQUEST");
    for field in ["url", "cap", "cookie", "session_id"] {
        let mut invalid = entry.clone();
        invalid[field] = json!("transient");
        error(&call(&restarted, "add_watchlist", invalid), "BAD_REQUEST");
    }
    let mut invalid = entry.clone();
    invalid["poster_cache_key"] = json!("http://127.0.0.1/private");
    error(&call(&restarted, "add_watchlist", invalid), "BAD_REQUEST");
    for command in ["list_watchlist", "add_watchlist", "remove_watchlist"] {
        error(
            &restarted.invoke(
                command,
                json!({"request_id":Uuid::new_v4().to_string(),"input":{}}),
                false,
            ),
            "UNAUTHORIZED",
        );
    }
    for _ in 0..2 {
        ok(
            &restarted,
            "remove_watchlist",
            json!({"content_id":entry["content_id"]}),
        );
    }
    assert_eq!(
        ok(&restarted, "list_watchlist", json!({})),
        json!({"entries":[]})
    );
    println!(
        "TV5_CRUD:{}",
        json!({"restart":true,"idempotent":true,"rejected":9,"removed":true})
    );
}
#[test]
fn tv5_migration_v1_keeps_all_history_fields() {
    let temp = TempDir::new().unwrap();
    let database = temp.path().join("moc-video.db");
    legacy5(&database);
    assert_eq!(version5(&database), 1);
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    assert_eq!(
        ok(&host, "list_watchlist", json!({})),
        json!({"entries":[]})
    );
    assert_eq!(version5(&database), 2);
    assert_eq!(
        ok(&host, "list_history", json!({}))["entries"],
        json!([history5()])
    );
    ok(&host, "add_watchlist", entry5());
    assert_eq!(
        ok(&host, "list_history", json!({}))["entries"],
        json!([history5()])
    );
    let columns: i64 = Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM pragma_table_info('watch_history')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(columns, 19);
    println!(
        "TV5_UPGRADE:{}",
        json!({"before":1,"after":2,"history_fields":19,"history_kept":true})
    );
}
#[test]
fn tv5_migration_failure_rolls_back_and_old_database_remains_usable() {
    let temp = TempDir::new().unwrap();
    let database = temp.path().join("moc-video.db");
    legacy5(&database);
    Connection::open(&database)
        .unwrap()
        .execute_batch(
            "CREATE TABLE watchlist(kept TEXT); INSERT INTO watchlist VALUES('old data');",
        )
        .unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let failed = call(&host, "list_watchlist", json!({}));
    error(&failed, "INTERNAL_ERROR");
    assert!(
        failed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("initialization failed")
    );
    assert_eq!(version5(&database), 1);
    let old = Connection::open(&database).unwrap();
    assert_eq!(
        old.query_row("SELECT title FROM watch_history", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        history5()["title"].as_str().unwrap()
    );
    assert_eq!(
        old.query_row("SELECT kept FROM watchlist", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "old data"
    );
    old.execute_batch("DROP TABLE watchlist;").unwrap();
    assert_eq!(
        ok(&host, "list_history", json!({}))["entries"],
        json!([history5()])
    );
    assert_eq!(version5(&database), 2);
    println!(
        "TV5_MIGRATION_FAILURE:{}",
        json!({"explicit_error":true,"old_version":1,"old_data_usable":true,"retry":2})
    );
}

fn media5(index: usize, port: u16) -> Value {
    let mut meta = super::super::tv3::media3("series", index, "Drama", 2026, "TV5", port);
    meta["name"] = json!(format!("TV5 series {index}"));
    meta["description"] =
        json!("A long synopsis with real plain text for the glass dialog. ".repeat(35));
    meta["cast"] = json!(["林晓", "Morgan Reed"]);
    meta["director"] = json!(["Avery Lin"]);
    meta["app_extras"] = json!({"cast":[{"name":"林晓","character":"Detective","photo":format!("http://maccms.fixture.test:{port}/poster.png?i=8")},{"name":"Morgan Reed","character":"Pilot"}]});
    meta["country"] = json!("China, UK");
    meta["language"] = json!("Mandarin, English");
    meta["runtime"] = json!("45 min");
    meta["videos"] = json!([
        {"id":"video:1:1","season":1,"episode":1,"title":"Arrival","overview":"The first chapter begins.","runtime":"15 min","thumbnail":format!("http://maccms.fixture.test:{port}/backdrop.png?i=1")},
        {"id":"video:1:2","season":1,"episode":2,"title":"Discovery","overview":"A missing image uses the blurred backdrop and episode number.","runtime":"15 min"},
        {"id":"video:1:3","season":1,"episode":3,"title":"Reunion","overview":"The saved chapter resumes from its confirmed history.","runtime":"15 min","thumbnail":format!("http://maccms.fixture.test:{port}/backdrop.png?i=3")},
        {"id":"video:2:1","season":2,"episode":1,"title":"New season","overview":"The next season.","runtime":"15 min"}]);
    meta
}
fn fixture5(media: Option<std::path::PathBuf>) -> Fixture {
    Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            let mut manifest = super::super::tv3::manifest3();
            manifest["types"] = json!(["series"]);
            manifest["catalogs"]
                .as_array_mut()
                .unwrap()
                .retain(|c| c["type"] == "series");
            return Reply::json(manifest);
        }
        if request.target.contains("/catalog/") {
            return Reply::json(json!({"metas":(0..6).map(|i|media5(i,port)).collect::<Vec<_>>()}));
        }
        if request.target.contains("/meta/") {
            let id = request
                .target
                .rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".json");
            let index = id
                .strip_prefix("tmdb:")
                .unwrap_or("1")
                .parse::<usize>()
                .unwrap()
                - 1;
            return Reply::json(json!({"meta":media5(index,port)}));
        }
        if request.target.starts_with("/poster.png") || request.target.starts_with("/backdrop.png")
        {
            return super::super::tv3::upstream3(request, port, 0);
        }
        if request.target.starts_with("/media/") {
            return Reply::raw(
                media
                    .as_ref()
                    .map(|p| std::fs::read(p.join("a.mp4")).unwrap())
                    .unwrap_or_else(|| vec![0; 64]),
                "video/mp4",
            );
        }
        let mut item = super::media4(port, "2:1:0");
        item["vod_name"] = json!("TV5 series 0");
        item["vod_play_url"] = json!(
            (1..=3)
                .map(|n| format!("第{n}集$http://maccms.fixture.test:{port}/media/a.mp4"))
                .collect::<Vec<_>>()
                .join("#")
        );
        Reply::json(response(json!([item]), super::classes4(false)))
    })
}
fn sync5(host: &Host) {
    sync(host);
    ok(
        host,
        "sync_catalog",
        json!({"kind":"stremio","config_id":METADATA_CONFIG}),
    );
    ok(host, "runtime_prepare", json!({"target":"application"}));
}
fn seed5(host: &Host, provider: &Value) -> Value {
    let sources = ok(host, "get_sources", json!({}));
    let metadata = sources["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "stremio")
        .unwrap();
    let card =
        ok(host, "get_home", home_input(metadata))["page"]["sections"][0]["items"][0].clone();
    let details = ok(host, "get_details", json!({"ref":card["ref"]}));
    let input = json!({"metadata_ref":details["metadata_ref"],"video_id":"video:1:3","playback_source":{"source_id":provider["id"],"source_revision":provider["source_revision"]},"selection":null});
    let target = inspect(host, provider);
    let context = ok(host, "match_playback", choose(&input, &target, 0, 2))["context"].clone();
    let resolution = ok(host, "resolve_episode", resolve_input(&context));
    let session = ok(host, "open_playback", open_input(&resolution));
    let record = json!({"content_id":details["details"]["id"],"metadata_identity":context["metadata_ref"]["identity"],"video_id":"video:1:3","season_number":1,"episode_number":3.0,
        "title":details["details"]["title"],"kind":"series","poster_cache_key":null,"source_id":provider["id"],"source_name":provider["name"],"position_ms":7000,"duration_ms":20000,"completed":false,"updated_at":1000});
    ok(
        host,
        "save_history",
        json!({"record":record,"session_id":session["session_id"],"match_id":context["match_id"],"playback_epoch":1,"write_sequence":1}),
    );
    ok(
        host,
        "close_playback",
        json!({"session_id":session["session_id"]}),
    );
    ok(
        host,
        "release_resolution",
        json!({"resolution_id":resolution["id"]}),
    );
    json!({"ok":true,"ref":card["ref"],"record":record})
}
#[test]
fn tv5_packaged_host_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let fixture = fixture5(std::env::var_os("T12_MEDIA").map(std::path::PathBuf::from));
    let (temp, mut host, provider) = super::setup4(&fixture);
    let ready = seed5(&host, &provider);
    let publish = |path: &std::path::Path, value: &Value| {
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
        std::fs::rename(temporary, path).unwrap();
    };
    publish(&directory.join("ready.json"), &ready);
    let start = Instant::now();
    let (mut sequence, mut homes) = (0, 0);
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(start.elapsed() < Duration::from_secs(1800));
            thread::sleep(Duration::from_millis(5));
        }
        let request: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let result = match request["operation"].as_str().unwrap_or("invoke") {
            "finish" => {
                publish(
                    &directory.join(format!("response-{sequence}.json")),
                    &json!({"ok":true}),
                );
                break;
            }
            "restart" => {
                assert_eq!(host.requests.active_count(), 0);
                let old = ok(&host, "runtime_status", json!({}))["engine_epoch"].clone();
                host = Host::open(temp.path().join("settings.json")).unwrap();
                sync5(&host);
                json!({"old_epoch":old,"new_epoch":ok(&host,"runtime_status",json!({}))["engine_epoch"],"watchlist":ok(&host,"list_watchlist",json!({})),"history":ok(&host,"list_history",json!({}))})
            }
            "evict-images" => {
                // Exercise the production 4096-entry LRU, as in the T12 fixture.
                let mut committed = host.committed.lock().unwrap();
                let catalog = committed.maccms[provider["id"].as_str().unwrap()].clone();
                let image = catalog.images.values().next().unwrap().clone();
                let gateway = committed.gateway.as_mut().unwrap();
                for _ in 0..4096 {
                    let mut image = image.clone();
                    let id = Uuid::new_v4().to_string();
                    image.reference["resource_id"] = json!(id);
                    image.reference["path"] = json!(format!("/images/{id}"));
                    gateway
                        .image(provider["id"].as_str().unwrap(), image)
                        .unwrap();
                }
                json!({"ok":true})
            }
            "invoke" => {
                let command = request["command"].as_str().unwrap();
                homes += usize::from(command == "get_home");
                let result = host.invoke(command, request["args"].clone(), true);
                Contracts::get()
                    .validate(&format!("Result_{command}"), &result)
                    .unwrap();
                result
            }
            other => panic!("Unsupported TV5 operation {other}"),
        };
        publish(
            &directory.join(format!("response-{sequence}.json")),
            &result,
        );
    }
    println!(
        "T14_VECTOR:{}",
        json!({"get_home":homes,"active_requests":host.requests.active_count()})
    );
}
