//! TV7's controlled search sources run through the production Host.
use super::*;

fn fixture7() -> Fixture {
    Fixture::new(move |request, port| {
        if request.target.starts_with("/fail") {
            return Reply {
                status: 503,
                ..Reply::raw("controlled source failure", "text/plain")
            };
        }
        if request.target.contains("manifest.json") {
            let mut manifest = super::super::super::tv3::manifest3();
            manifest["types"] = json!(["movie", "series", "tv"]);
            for kind in ["movie", "series", "tv"] {
                manifest["catalogs"].as_array_mut().unwrap().push(json!({"id":"tv7-search","name":"Search","type":kind,"extra":[{"name":"search","isRequired":true}]}));
            }
            // A second required extra makes this catalog unusable for search.
            manifest["catalogs"].as_array_mut().unwrap().push(json!({"id":"not-searchable","type":"movie","extra":[{"name":"search"},{"name":"token","isRequired":true}]}));
            return Reply::json(manifest);
        }
        if request.target.starts_with("/poster.png") || request.target.starts_with("/backdrop.png")
        {
            return super::super::super::tv3::upstream3(request, port, 0);
        }
        if request.target.contains("/catalog/") || request.target.contains("/meta/") {
            let url = url::Url::parse(&format!("http://fixture{}", request.target)).unwrap();
            let parts: Vec<_> = url.path_segments().unwrap().collect();
            let at = parts
                .iter()
                .position(|p| *p == "catalog" || *p == "meta")
                .unwrap();
            let kind = parts[at + 1];
            let extra: HashMap<String, String> = url::form_urlencoded::parse(
                parts
                    .get(at + 3)
                    .unwrap_or(&"")
                    .trim_end_matches(".json")
                    .as_bytes(),
            )
            .into_owned()
            .collect();
            if parts[at] == "meta" {
                let index = parts[at + 2]
                    .trim_end_matches(".json")
                    .trim_start_matches("tmdb:")
                    .parse::<usize>()
                    .unwrap()
                    - 1;
                let mut meta = media5(index, port);
                meta["type"] = json!(kind);
                return Reply::json(json!({"meta":meta}));
            }
            assert!(
                !request.target.contains("not-searchable"),
                "undeclared required extra must never be guessed"
            );
            let query = extra.get("search").map(String::as_str);
            let mut rows: Vec<_> = if query == Some("none") {
                vec![]
            } else {
                (0..3)
                    .map(|i| {
                        let mut meta = media5(i, port);
                        meta["type"] = json!(kind);
                        if let Some(query) = query {
                            meta["name"] = json!(format!("{query} {kind} {i}"));
                        }
                        meta
                    })
                    .collect()
            };
            // Duplicate results must still yield one stable content identity.
            if let Some(first) = rows.first().cloned() {
                rows.push(first);
            }
            return Reply {
                body_delay: if query == Some("slow") {
                    Duration::from_secs(3)
                } else {
                    Duration::ZERO
                },
                ..Reply::json(json!({"metas":rows}))
            };
        }
        let query = request.query.get("wd").map(String::as_str);
        let mut rows: Vec<_> = if query == Some("none") {
            vec![]
        } else {
            ["1:1:0", "2:1:0", "3:1:0"]
                .iter()
                .map(|id| {
                    let mut item = super::super::media4(port, id);
                    if let Some(query) = query {
                        item["vod_name"] = json!(format!("{query} {id}"));
                    }
                    item
                })
                .collect()
        };
        if let Some(id) = request.query.get("ids") {
            rows.retain(|item| item["vod_id"] == *id);
        }
        Reply {
            body_delay: if query == Some("slow") {
                Duration::from_secs(3)
            } else {
                Duration::ZERO
            },
            ..Reply::json(response(json!(rows), super::super::classes4(false)))
        }
    })
}

fn setup7(fixture: &Fixture) -> (TempDir, Arc<Host>, Value, Value) {
    let (temp, host, playback) = super::super::setup4(fixture);
    let sources = ok(&host, "get_sources", json!({}));
    let metadata = sources["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "stremio")
        .unwrap()
        .clone();
    (temp, host, metadata, playback)
}
fn query7(source: &Value, query: &str) -> Value {
    json!({"source_id":source["id"],"source_revision":source["source_revision"],"query":query})
}

#[test]
fn tv7_declared_search_catalogs_maccms_wd_and_closed_inputs() {
    let fixture = fixture7();
    let (_temp, host, metadata, playback) = setup7(&fixture);
    assert_eq!(metadata["capabilities"]["search"], true);
    fixture.requests.lock().unwrap().clear();
    let found = ok(&host, "search_catalog", query7(&metadata, "海 洋/🎬?&"));
    assert_eq!(found.as_object().unwrap().len(), 1);
    assert_eq!(found["items"].as_array().unwrap().len(), 9);
    let requests = fixture.requests.lock().unwrap().clone();
    let catalogs: Vec<_> = requests
        .iter()
        .filter(|r| r.target.contains("/catalog/"))
        .collect();
    assert_eq!(catalogs.len(), 3);
    for request in catalogs {
        assert!(request.target.contains("/tv7-search/"));
        let extra = request
            .target
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".json");
        let pairs: HashMap<String, String> = url::form_urlencoded::parse(extra.as_bytes())
            .into_owned()
            .collect();
        assert_eq!(pairs.get("search").unwrap(), "海 洋/🎬?&");
        assert_eq!(pairs.len(), 1);
    }
    let local = ok(&host, "search_catalog", query7(&playback, "电影 & ="));
    assert_eq!(local["items"].as_array().unwrap().len(), 3);
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.query.get("wd").is_some_and(|v| v == "电影 & ="))
    );
    for source in [&metadata, &playback] {
        assert_eq!(
            ok(&host, "search_catalog", query7(source, "none")),
            json!({"items":[]})
        );
        error(
            &call(&host, "search_catalog", query7(source, "  ")),
            "BAD_REQUEST",
        );
        let mut stale = query7(source, "x");
        stale["source_revision"] = json!("stale");
        assert!(call(&host, "search_catalog", stale).get("error").is_some());
        for key in ["url", "cap", "cookie"] {
            let mut invalid = query7(source, "x");
            invalid[key] = json!("unexpected");
            error(&call(&host, "search_catalog", invalid), "BAD_REQUEST");
        }
    }
    error(
        &host.invoke(
            "search_catalog",
            json!({"request_id":Uuid::new_v4().to_string(),"input":query7(&metadata,"x")}),
            false,
        ),
        "UNAUTHORIZED",
    );
    assert_eq!(host.requests.active_count(), 0);
    println!(
        "TV7_SEARCH:{}",
        json!({"stremio":9,"catalog_requests":3,"maccms":3,"encoded_query":true,"rejected":11,"active_requests":0})
    );
}

#[test]
fn tv7_cancellation_and_per_source_serial_queue() {
    let fixture = fixture7();
    let (_temp, host, metadata, _) = setup7(&fixture);
    fixture.requests.lock().unwrap().clear();
    let id = Uuid::new_v4().to_string();
    let worker = host.clone();
    let input = query7(&metadata, "slow");
    let worker_id = id.clone();
    let first = thread::spawn(move || {
        worker.invoke(
            "search_catalog",
            json!({"request_id":worker_id,"input":input}),
            true,
        )
    });
    let start = Instant::now();
    while !fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.target.contains("search=slow"))
    {
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(5));
    }
    let second_id = Uuid::new_v4().to_string();
    let worker = host.clone();
    let input = query7(&metadata, "queued");
    let worker_id = second_id.clone();
    let second = thread::spawn(move || {
        worker.invoke(
            "search_catalog",
            json!({"request_id":worker_id,"input":input}),
            true,
        )
    });
    thread::sleep(Duration::from_millis(100));
    assert!(
        !fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.target.contains("search=queued"))
    );
    ok(
        &host,
        "cancel_request",
        json!({"target_request_id":second_id}),
    );
    error(&second.join().unwrap(), "CANCELLED");
    ok(&host, "cancel_request", json!({"target_request_id":id}));
    error(&first.join().unwrap(), "CANCELLED");
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(host.requests.active_count(), 0);
    assert!(
        !ok(&host, "search_catalog", query7(&metadata, "fresh"))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    println!(
        "TV7_CANCEL:{}",
        json!({"running_cancelled":true,"queued_cancelled":true,"serial":true,"recovered":true,"active_requests":host.requests.active_count()})
    );
}

fn seed7(host: &Host, metadata: &Value) -> Value {
    let cards = ok(host, "get_home", home_input(metadata))["page"]["sections"][0]["items"].clone();
    let now = time::OffsetDateTime::now_utc().unix_timestamp() * 1000;
    let mut records = Vec::new();
    for (i, card) in cards.as_array().unwrap().iter().take(3).enumerate() {
        let details = ok(host, "get_details", json!({"ref":card["ref"]}));
        let record = json!({"content_id":details["details"]["id"],"metadata_identity":details["metadata_ref"]["identity"],"video_id":"video:1:3","season_number":1,"episode_number":3,
            "title":details["details"]["title"],"kind":details["details"]["kind"],"poster_cache_key":null,"source_id":"fixture","source_name":"Fixture","position_ms":7000,"duration_ms":20000,"completed":i==2,"updated_at":now-(i as i64)*86400000});
        host.committed
            .lock()
            .unwrap()
            .history
            .commit((i + 1) as u64, &json!({"record":record}))
            .unwrap();
        if i == 0 {
            let mut watch = record.clone();
            for key in [
                "video_id",
                "season_number",
                "episode_number",
                "source_id",
                "source_name",
                "position_ms",
                "duration_ms",
                "completed",
                "updated_at",
            ] {
                watch.as_object_mut().unwrap().remove(key);
            }
            ok(host, "add_watchlist", watch);
        }
        records.push(record);
    }
    json!({"ok":true,"records":records})
}

#[test]
fn tv7_packaged_host_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let fixture = fixture7();
    let (_temp, host, metadata, playback) = setup7(&fixture);
    let mut ready = seed7(&host, &metadata);
    ready["metadata"] = metadata;
    ready["playback"] = playback;
    let publish = |path: &std::path::Path, value: &Value| {
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec(value).unwrap()).unwrap();
        std::fs::rename(temp, path).unwrap();
    };
    publish(&directory.join("ready.json"), &ready);
    let start = Instant::now();
    let (mut sequence, mut homes) = (0, 0);
    let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(start.elapsed() < Duration::from_secs(1800));
            thread::sleep(Duration::from_millis(5));
        }
        let request: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let target = directory.join(format!("response-{sequence}.json"));
        match request["operation"].as_str().unwrap_or("invoke") {
            "finish" => {
                for worker in workers {
                    worker.join().unwrap();
                }
                publish(&target, &json!({"ok":true}));
                break;
            }
            "invoke" => {
                let command = request["command"].as_str().unwrap().to_owned();
                homes += usize::from(command == "get_home");
                let host = host.clone();
                workers.push(thread::spawn(move || {
                    let result = host.invoke(&command, request["args"].clone(), true);
                    Contracts::get()
                        .validate(&format!("Result_{command}"), &result)
                        .unwrap();
                    publish(&target, &result);
                }));
            }
            other => panic!("Unsupported TV7 operation {other}"),
        }
    }
    println!(
        "T14_VECTOR:{}",
        json!({"get_home":homes,"active_requests":host.requests.active_count()})
    );
}
