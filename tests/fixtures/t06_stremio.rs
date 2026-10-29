//! Production HTTP/Host behavior using the existing Stremio fixture boundary.
use super::*;

#[test]
fn t06_stremio_order_duplicate_titles_retry_and_original_offset() {
    let fail = Arc::new(AtomicBool::new(true));
    let dynamic = Arc::new(AtomicBool::new(false));
    let (failure, changed) = (fail.clone(), dynamic.clone());
    let fixture = Fixture::new(move |r| {
        if r.target.ends_with("manifest.json") {
            let mut catalogs = vec![
                json!({"id":"a", "type":"movie", "name":"Same title", "extra":[{"name":"skip"}]}),
                json!({"id":"required", "type":"movie", "extra":[{"name":"search", "isRequired":true}]}),
                json!({"id":"b", "type":"movie", "name":"Same title"}),
                json!({"id":"c", "type":"movie", "name":"C"}),
            ];
            if changed.load(Ordering::SeqCst) {
                catalogs.push(json!({"id":"dynamic", "type":"movie", "name":"Dynamic"}));
            }
            return Reply::json(manifest(json!(["catalog", "meta"]), json!(catalogs)));
        }
        if r.target.contains("/b.json") && failure.load(Ordering::SeqCst) {
            return Reply::fail();
        }
        if r.target.contains("skip=2") {
            return Reply::json(json!({"metas":[meta("tt002", "movie"), meta("tt003", "movie")]}));
        }
        if r.target.contains("skip=4") {
            return Reply::json(json!({"metas":[]}));
        }
        standard(r)
    });
    let (_temp, host) = setup(vec![fixture.url("/configured/path/manifest.json")]);
    let source = sync(&host);
    let full = ok(&host, "get_home", home_input(&source));
    assert_eq!(full["page"]["sections"][0]["index"], 0);
    assert_eq!(full["page"]["sections"][1]["index"], 2);
    assert_eq!(full["page"]["failed_sections"][0]["index"], 1);
    let mut retry = home_input(&source);
    retry["section_ref"] = full["page"]["failed_sections"][0]["section_ref"].clone();
    error(&call(&host, "get_home", retry.clone()), "UPSTREAM_FAILED");
    fail.store(false, Ordering::SeqCst);
    let repaired = ok(&host, "get_home", retry.clone());
    assert_eq!(repaired["page"]["sections"].as_array().unwrap().len(), 1);
    assert_eq!(repaired["page"]["sections"][0]["index"], 1);
    assert_eq!(repaired["page"]["failed_sections"], json!([]));
    assert_eq!(
        repaired["page"]["sections"][0]["title"],
        full["page"]["sections"][0]["title"]
    );
    assert_ne!(
        repaired["page"]["sections"][0]["id"],
        full["page"]["sections"][0]["id"]
    );
    retry["section_ref"] = full["page"]["sections"][0]["section_ref"].clone();
    retry["page"] = json!(2);
    let next = ok(&host, "get_home", retry.clone());
    assert_eq!(next["page"]["sections"][0]["page"], 2);
    assert_eq!(next["page"]["sections"][0]["index"], 0);
    assert_eq!(
        next["page"]["sections"][0]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(next["page"]["sections"][0]["has_next"], true);
    retry["page"] = json!(3);
    let empty = ok(&host, "get_home", retry.clone());
    assert_eq!(empty["page"]["sections"][0]["items"], json!([]));
    assert_eq!(empty["page"]["sections"][0]["has_next"], false);
    assert_eq!(empty["page"]["sections"][0]["next_page"], Value::Null);
    let targets: Vec<_> = fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.target.clone())
        .collect();
    assert!(
        targets
            .iter()
            .any(|t| t == "/configured/path/catalog/movie/a/skip=2.json")
    );
    assert!(
        targets
            .iter()
            .any(|t| t == "/configured/path/catalog/movie/a/skip=4.json")
    );
    assert!(!targets.iter().any(|t| t.contains("/required")));
    retry["section_ref"] = full["page"]["sections"][1]["section_ref"].clone();
    retry["page"] = json!(2);
    error(&call(&host, "get_home", retry.clone()), "BAD_REQUEST");
    dynamic.store(true, Ordering::SeqCst);
    let refreshed_source = sync(&host);
    assert_ne!(
        source["source_revision"],
        refreshed_source["source_revision"]
    );
    let refreshed = ok(&host, "get_home", home_input(&refreshed_source));
    assert_eq!(refreshed["page"]["sections"].as_array().unwrap().len(), 4);
    assert_eq!(refreshed["page"]["sections"][3]["index"], 3);
    error(&call(&host, "get_home", retry.clone()), "REVISION_MISMATCH");
    retry["source_revision"] = refreshed_source["source_revision"].clone();
    error(&call(&host, "get_home", retry), "BAD_REQUEST");
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t06_stremio_column_localization_shares_30s_deadline() {
    let temp = TempDir::new().unwrap();
    let clock = Arc::new(FakeClock::default());
    let server_clock = clock.clone();
    let fixture = Fixture::new(move |r| {
        if r.target.ends_with("manifest.json") {
            let mut manifest = standard_manifest();
            manifest["id"] = json!("tmdb-addon");
            manifest["version"] = json!("3.1.7");
            return Reply::json(manifest);
        }
        if r.target.contains("/zh-CN/") {
            return Reply::delayed(json!({"metas":[]}));
        }
        server_clock.0.store(20000, Ordering::SeqCst);
        Reply::json(json!({"metas":[meta("tmdb:27205", "movie")]}))
    });
    let host = Host::with_clock(temp.path().join("settings.json"), clock.clone()).unwrap();
    configure(&host, vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["ui_locale"] = json!("zh-CN");
    ok(&host, "save_settings", settings);
    let source = sync(&host);
    let worker_host = host.clone();
    let (finished, output) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let response = call(&worker_host, "get_home", home_input(&source));
        finished.send(response).unwrap();
    });
    fixture.wait_for("/tmdb/zh-CN/catalog/movie/featured.json");
    clock.0.store(30000, Ordering::SeqCst);
    let within_column_deadline = output.recv_timeout(Duration::from_secs(2));
    // Always let an old implementation finish too, so a failed assertion keeps
    // no fixture worker alive. The parent still has its 1000ms cleanup reserve.
    if within_column_deadline.is_err() {
        clock.0.store(44000, Ordering::SeqCst);
    }
    worker.join().unwrap();
    let response = within_column_deadline.expect("localization renewed the column deadline");
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(
        response["data"]["page"]["sections"][0]["items"][0]["title"],
        "Synthetic same title"
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t06_stremio_reserve_cancellation_empty_and_all_failed_regressions() {
    n01_aggregate_45s_reserve_partial_and_cancellation();
    n01_partial_home_failed_placeholders_retry_and_all_failure();
    n01_skip_raw_count_dedup_empty_and_no_new_items();
}
