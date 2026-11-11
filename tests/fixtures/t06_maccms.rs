//! T06 exercises production Host dispatch and native HTTP, using N00's fixture.
use super::*;

#[test]
fn t06_maccms_dynamic_duplicate_titles_original_page_and_retry() {
    let version = Arc::new(AtomicUsize::new(0));
    let failure = Arc::new(AtomicBool::new(true));
    let (v, f) = (version.clone(), failure.clone());
    let fixture = Fixture::new(move |request, port| {
        let classes = if v.load(Ordering::SeqCst) == 0 {
            json!([{"type_id":"a/+%中文", "type_name":"相同标题"}, {"type_id":"b", "type_name":"相同标题"}, {"type_id":"c", "type_name":"C"}])
        } else {
            json!([{"type_id":"c", "type_name":"Dynamic C"}, {"type_id":"new", "type_name":"新栏目"}])
        };
        if request.query.get("t").is_some_and(|t| t == "b") && f.load(Ordering::SeqCst) {
            return Reply::json(json!({"code":0, "list":[]}));
        }
        let mut response = response(json!([item(port)]), classes);
        if request.query.get("pg").is_some_and(|p| p == "2") {
            response["page"] = json!(2);
            response["pagecount"] = json!(99);
            response["list"] = json!([]);
        }
        Reply::json(response)
    });
    let (_temp, host) = setup(fixture.url("/api?original=a%2Fb"));
    let source = sync(&host);
    let full = ok(&host, "get_home", home_input(&source));
    assert_eq!(full["page"]["sections"][0]["index"], 0);
    assert_eq!(full["page"]["sections"][1]["index"], 2);
    assert_eq!(full["page"]["failed_sections"][0]["index"], 1);
    let mut retry = home_input(&source);
    retry["section_ref"] = full["page"]["failed_sections"][0]["section_ref"].clone();
    error(&call(&host, "get_home", retry.clone()), "UPSTREAM_FAILED");
    failure.store(false, Ordering::SeqCst);
    let repaired = ok(&host, "get_home", retry.clone());
    assert_eq!(repaired["page"]["sections"].as_array().unwrap().len(), 1);
    assert_eq!(repaired["page"]["sections"][0]["index"], 1);
    assert_eq!(repaired["page"]["failed_sections"], json!([]));
    assert_eq!(
        full["page"]["sections"][0]["title"],
        repaired["page"]["sections"][0]["title"]
    );
    assert_ne!(
        full["page"]["sections"][0]["id"],
        repaired["page"]["sections"][0]["id"]
    );
    retry["section_ref"] = full["page"]["sections"][0]["section_ref"].clone();
    retry["page"] = json!(2);
    let next = ok(&host, "get_home", retry.clone());
    assert_eq!(next["page"]["sections"][0]["page"], 2);
    assert_eq!(next["page"]["sections"][0]["index"], 0);
    assert_eq!(next["page"]["sections"][0]["items"], json!([]));
    assert_eq!(next["page"]["sections"][0]["has_next"], false);
    assert_eq!(next["page"]["sections"][0]["next_page"], Value::Null);
    assert!(fixture.requests.lock().unwrap().iter().any(|r| {
        r.query.get("t").is_some_and(|v| v == "a/+%中文")
            && r.query.get("pg").is_some_and(|v| v == "2")
            && r.target.contains("original=a%2Fb")
    }));
    version.store(1, Ordering::SeqCst);
    let refresh = ok(&host, "get_home", home_input(&source));
    assert_eq!(refresh["page"]["sections"][0]["title"], "Dynamic C");
    assert_eq!(refresh["page"]["sections"][0]["index"], 0);
    assert_eq!(refresh["page"]["sections"].as_array().unwrap().len(), 2);
    retry["page"] = json!(1);
    error(&call(&host, "get_home", retry), "BAD_REQUEST");
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t06_maccms_reserve_keeps_completed_and_unstarted_positions() {
    let slow = Arc::new(AtomicBool::new(true));
    let blocked = slow.clone();
    let fixture = Fixture::new(move |request, port| {
        let mut reply = Reply::json(response(json!([item(port)]), json!((0..6).map(|index|
            json!({"type_id":index.to_string(), "type_name":format!("column-{index}")})).collect::<Vec<_>>())));
        if request.query.get("t").is_some_and(|t| t != "0") && blocked.load(Ordering::SeqCst) {
            reply.body_delay = Duration::from_secs(60);
        }
        reply
    });
    let temp = TempDir::new().unwrap();
    let clock = Arc::new(TestClock::default());
    let host = Host::with_clock(temp.path().join("settings.json"), clock.clone()).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([{"kind":"maccms", "config_id":CONFIG, "name":"Synthetic", "enabled":true, "api_url":fixture.url("/api")}]);
    ok(&host, "save_settings", settings);
    let source = sync(&host);
    let worker_host = host.clone();
    let request = home_input(&source);
    let worker = thread::spawn(move || call(&worker_host, "get_home", request));
    fixture.wait_for(|r| r.query.get("t").is_some_and(|t| t == "3"));
    thread::sleep(Duration::from_millis(100));
    clock.0.store(44000, Ordering::SeqCst);
    let response = worker.join().unwrap();
    assert!(response.get("error").is_none(), "{}", diagnostic(&response));
    let page = &response["data"]["page"];
    assert_eq!(page["sections"].as_array().unwrap().len(), 1);
    assert_eq!(page["sections"][0]["index"], 0);
    assert_eq!(
        page["failed_sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["index"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    assert!(
        page["failed_sections"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["error"]["code"] == "UPSTREAM_TIMEOUT")
    );
    assert!(
        !fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.query.get("t").is_some_and(|t| t == "4" || t == "5"))
    );
    slow.store(false, Ordering::SeqCst);
    let mut retry = home_input(&source);
    retry["section_ref"] = page["failed_sections"][4]["section_ref"].clone();
    let repaired = ok(&host, "get_home", retry);
    assert_eq!(repaired["page"]["sections"][0]["index"], 5);
    assert_eq!(repaired["page"]["sections"].as_array().unwrap().len(), 1);
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t06_maccms_single_default_empty_and_all_failure_regressions() {
    let fail = Arc::new(AtomicBool::new(false));
    let failure = fail.clone();
    let fixture = Fixture::new(move |_, _| {
        if failure.load(Ordering::SeqCst) {
            Reply::json(json!({"code":0, "list":[]}))
        } else {
            let mut value = response(json!([]), json!([]));
            value["pagecount"] = json!(999);
            Reply::json(value)
        }
    });
    let (_temp, host) = setup(fixture.url("/api"));
    let source = sync(&host);
    let empty = ok(&host, "get_home", home_input(&source));
    assert_eq!(empty["page"]["sections"].as_array().unwrap().len(), 1);
    let section = &empty["page"]["sections"][0];
    assert_eq!(section["title"], "最近更新");
    assert_eq!(section["items"], json!([]));
    assert_eq!(section["has_next"], false);
    assert_eq!(section["next_page"], Value::Null);
    fail.store(true, Ordering::SeqCst);
    error(
        &call(&host, "get_home", home_input(&source)),
        "UPSTREAM_FAILED",
    );
    n00_partial_home_retry_pagination_and_all_failed();
    n00_metadata_limits_and_foreign_section_refs();
}
