//! Synthetic protocol fixtures. Real addon responses have a separate live test
//! and evidence directory; none of these values are presented as real metadata.
#[path = "t06_stremio.rs"]
mod t06;
#[path = "t07_stremio.rs"]
mod t07;
use super::*;
use crate::{host::Host, requests::Clock, settings};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tempfile::TempDir;
use uuid::Uuid;

const CONFIG: &str = "22222222-2222-4222-8222-222222222222";

#[derive(Clone)]
struct Request {
    target: String,
    headers: HashMap<String, String>,
}
struct Reply {
    status: u16,
    value: Value,
    delay: Duration,
    headers: Vec<(String, String)>,
}
impl Reply {
    fn json(value: Value) -> Self {
        Self {
            status: 200,
            value,
            delay: Duration::ZERO,
            headers: vec![],
        }
    }
    fn fail() -> Self {
        Self {
            status: 503,
            ..Self::json(json!({"error": "synthetic upstream failure"}))
        }
    }
    fn delayed(value: Value) -> Self {
        Self {
            delay: Duration::from_secs(60),
            ..Self::json(value)
        }
    }
}
struct Fixture {
    port: u16,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    requests: Arc<Mutex<Vec<Request>>>,
}
impl Fixture {
    fn new(handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(vec![]));
        let (stopped, observed, handler) = (stop.clone(), requests.clone(), Arc::new(handler));
        let worker = thread::spawn(move || {
            let mut workers = Vec::new();
            while !stopped.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (stop, requests, handler) =
                            (stopped.clone(), observed.clone(), handler.clone());
                        workers.push(thread::spawn(move || {
                            serve(stream, stop, requests, &*handler)
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            port,
            stop,
            worker: Some(worker),
            requests,
        }
    }
    fn url(&self, path: &str) -> String {
        format!("http://maccms.fixture.test:{}{path}", self.port)
    }
    fn wait_for(&self, path: &str) {
        let started = Instant::now();
        while !self
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.target == path)
        {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "request did not arrive: {path}"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn serve(
    mut stream: TcpStream,
    stop: Arc<AtomicBool>,
    observed: Arc<Mutex<Vec<Request>>>,
    handler: &dyn Fn(&Request) -> Reply,
) {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut buffer = Vec::new();
    let started = Instant::now();
    while !buffer.windows(4).any(|s| s == b"\r\n\r\n") {
        if stop.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(5) {
            return;
        }
        let mut chunk = [0; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => return,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => return,
        }
        assert!(buffer.len() <= 1024 * 1024);
    }
    let input = String::from_utf8(buffer).unwrap();
    let mut lines = input.lines();
    let request = Request {
        target: lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .into(),
        headers: lines
            .filter_map(|s| s.split_once(':'))
            .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().into()))
            .collect(),
    };
    observed.lock().unwrap().push(request.clone());
    let reply = handler(&request);
    let started = Instant::now();
    while started.elapsed() < reply.delay {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    let body = reply.value.to_string();
    let mut head = format!(
        "HTTP/1.1 {} Fixture\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        reply.status,
        body.len()
    );
    for (key, value) in reply.headers {
        head.push_str(&format!("{key}: {value}\r\n"));
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).is_ok() {
        let _ = stream.write_all(body.as_bytes());
    }
}

fn manifest(resources: Value, catalogs: Value) -> Value {
    json!({"id": "fixture.metadata", "version": "1.0.0", "name": "Synthetic addon", "description": "synthetic contract fixture",
        "types": ["movie", "series"], "resources": resources, "catalogs": catalogs})
}
fn standard_manifest() -> Value {
    manifest(
        json!(["catalog", "meta"]),
        json!([{"id":"featured", "type":"movie", "name":"Same title", "extra":[{"name":"skip"}]}]),
    )
}
fn meta(id: &str, kind: &str) -> Value {
    json!({"id":id, "type":kind, "name":"Synthetic same title", "description":"<b>English plot</b><script>secret()</script>",
        "year":"2024", "imdbRating":"8.2", "genres":["Drama"], "cast":["Actor"], "director":["Director"],
        "streams":[{"url":"http://127.0.0.1/forbidden", "infoHash":"not-a-stream", "ytId":"ignored", "proxyHeaders":{"request":{"Cookie":"secret"}}}],
        "infoHash":"ignored", "ytId":"ignored"})
}
fn standard(request: &Request) -> Reply {
    if request.target.ends_with("manifest.json") {
        Reply::json(standard_manifest())
    } else if request.target.contains("/catalog/") {
        Reply::json(json!({"metas":[meta("tt001", "movie"), meta("tt002", "movie")]}))
    } else if request.target.contains("/meta/") {
        let id = request
            .target
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".json");
        Reply::json(
            json!({"meta":meta(id, if request.target.contains("/series/") {"series"} else {"movie"})}),
        )
    } else {
        panic!("unexpected metadata request: {}", request.target);
    }
}
use crate::host::observation_redaction::{diagnostic, redact};
fn observation(command: &str, args: &Value, response: &Value) -> Result<Value, ApiError> {
    // Validate original values before a schema-shaped placeholder can hide them.
    Contracts::get().validate(&format!("Result_{command}"), response)?;
    let mut observation =
        json!({"command":command, "args":args, "response":response, "capabilities_redacted":true});
    redact(&mut observation);
    Ok(observation)
}
fn call(host: &Host, command: &str, input: Value) -> Value {
    let args = json!({"request_id":Uuid::new_v4().to_string(), "input":input});
    let response = host.invoke(command, args.clone(), true);
    let observation = observation(command, &args, &response).unwrap();
    println!("N01_OBSERVATION:{observation}");
    response
}
fn ok(host: &Host, command: &str, input: Value) -> Value {
    let response = call(host, command, input);
    assert!(response.get("error").is_none(), "{}", diagnostic(&response));
    response["data"].clone()
}
fn error(response: &Value, code: &str) {
    assert_eq!(response["error"]["code"], code, "{}", diagnostic(response));
    assert!(response.get("data").is_none());
}
fn configure(host: &Host, urls: Vec<String>) {
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([{"kind":"stremio", "config_id":CONFIG, "name":"Synthetic metadata", "enabled":true, "manifest_urls":urls}]);
    ok(host, "save_settings", settings);
}
fn setup(urls: Vec<String>) -> (TempDir, Arc<Host>) {
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    configure(&host, urls);
    (temp, host)
}
fn sync(host: &Host) -> Value {
    ok(
        host,
        "sync_catalog",
        json!({"kind":"stremio", "config_id":CONFIG}),
    )["sources"]["providers"][0]
        .clone()
}
fn source(host: &Host) -> Value {
    ok(host, "get_sources", json!({}))["providers"][0].clone()
}
fn home_input(source: &Value) -> Value {
    json!({"source_id":source["id"], "source_revision":source["source_revision"], "page":1})
}
fn first_card(host: &Host, source: &Value) -> Value {
    ok(host, "get_home", home_input(source))["page"]["sections"][0]["items"][0].clone()
}
fn job(host: &Host, operation: &str) -> Job {
    host.requests
        .begin(&Uuid::new_v4().to_string(), operation, host.clock.clone())
        .unwrap()
}
fn catalog(host: &Host) -> Catalog {
    Catalog::sync(
        host.capture_source(CONFIG, false).unwrap(),
        &Http::default(),
        &job(host, "http_source_sync"),
    )
    .map_err(|f| f.error)
    .unwrap()
}
fn set_locale(host: &Host, value: &str) {
    let mut settings = ok(host, "get_settings", json!({}));
    settings["ui_locale"] = json!(value);
    ok(host, "save_settings", settings);
}

#[test]
fn n01_manifest_atomic_first_failure_refresh_and_edit() {
    let failed = Arc::new(AtomicBool::new(true));
    let server_failed = failed.clone();
    let fixture = Fixture::new(move |r| {
        if server_failed.load(Ordering::SeqCst) && r.target == "/b/manifest.json" {
            Reply::fail()
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![
        fixture.url("/prefix/a/manifest.json"),
        fixture.url("/b/manifest.json"),
    ]);
    let response = call(
        &host,
        "sync_catalog",
        json!({"kind":"stremio","config_id":CONFIG}),
    );
    error(&response, "UPSTREAM_FAILED");
    assert_eq!(
        response["error"]["details"],
        json!({"reason":"manifest_sync_failed","failed_addon_indices":[1]})
    );
    let initial = source(&host);
    assert_eq!(initial["source_revision"], "unpublished");
    assert_eq!(initial["runtime_status"], "failed");
    assert!(
        initial["capabilities"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v == false)
    );
    error(
        &call(&host, "get_home", home_input(&initial)),
        "ENGINE_NOT_READY",
    );
    failed.store(false, Ordering::SeqCst);
    let published = sync(&host);
    assert_eq!(published["runtime_status"], "ready");
    let card = first_card(&host, &published);
    assert_eq!(
        ok(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":CONFIG})
        )["changed"],
        false
    );
    failed.store(true, Ordering::SeqCst);
    error(
        &call(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":CONFIG}),
        ),
        "UPSTREAM_FAILED",
    );
    assert_eq!(source(&host), published);
    ok(&host, "get_details", json!({"ref":card["ref"]}));
    let queued = host
        .admit(
            "get_home",
            json!({"request_id":Uuid::new_v4().to_string(), "input":home_input(&published)}),
            true,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    configure(&host, vec![fixture.url("/b/manifest.json")]);
    error(&host.execute(queued), "CANCELLED");
    let edited = source(&host);
    assert_eq!(edited["source_revision"], "unpublished");
    error(
        &call(&host, "get_details", json!({"ref":card["ref"]})),
        "REVISION_MISMATCH",
    );
    error(
        &call(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":CONFIG}),
        ),
        "UPSTREAM_FAILED",
    );
    assert_eq!(source(&host)["runtime_status"], "failed");
}

#[test]
fn n01_resource_filters_required_extras_order_and_same_titles() {
    let fixture = Fixture::new(|r| {
        if r.target == "/first/manifest.json" {
            let mut m = manifest(
                json!(["catalog",{"name":"meta","types":["movie"],"idPrefixes":["tt"]}]),
                json!([
                {"type":"movie","id":"z","name":"Same"}, {"type":"movie","id":"required","extra":[{"name":"genre","isRequired":true,"options":["Drama"]}]},
                {"type":"series","id":"blocked"}, {"type":"movie","id":"a","name":"Same"}]),
            );
            m["types"] = json!(["movie"]);
            m["idPrefixes"] = json!(["does-not-apply-to-catalog"]);
            Reply::json(m)
        } else if r.target == "/second/manifest.json" {
            let mut m = manifest(
                json!([{"name":"catalog","types":["movie"]}, {"name":"meta","types":["series"]}]),
                json!([{"type":"movie","id":"z","name":"Same"}]),
            );
            m["types"] = json!(["unused"]);
            Reply::json(m)
        } else if r.target.contains("/catalog/") {
            Reply::json(json!({"metas":[meta("tt001","movie"),meta("tt002","movie")]}))
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![
        fixture.url("/first/manifest.json"),
        fixture.url("/second/manifest.json"),
    ]);
    let source = sync(&host);
    assert_eq!(source["capabilities"]["resolve"], false);
    assert_eq!(source["capabilities"]["details"], true);
    let response = ok(&host, "get_home", home_input(&source));
    let sections = response["page"]["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 3);
    assert_eq!(
        sections
            .iter()
            .map(|s| s["index"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        sections
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect::<HashSet<_>>()
            .len(),
        3
    );
    assert_ne!(sections[0]["items"][0]["id"], sections[0]["items"][1]["id"]);
    assert_ne!(sections[0]["items"][0]["id"], sections[2]["items"][0]["id"]);
    assert!(sections.iter().all(|s| s["has_next"] == false));
    let requests = fixture.requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .all(|r| !r.target.contains("required") && !r.target.contains("blocked"))
    );
    drop(requests);
    let cat = catalog(&host);
    assert!(cat.addons[0].supports("meta", "movie", "tt001"));
    assert!(!cat.addons[0].supports("meta", "movie", "xx001"));
    assert!(!cat.addons[0].supports("meta", "series", "tt001"));
    assert!(cat.addons[1].supports("meta", "series", "anything"));
}

#[test]
fn n01_prefix_query_rejection_and_path_segment_encoding() {
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(vec![
        fixture.url("/custom/config/manifest.json?language=en&keep=a%2Fb"),
    ]);
    error(
        &call(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":CONFIG}),
        ),
        "BAD_REQUEST",
    );
    assert!(fixture.requests.lock().unwrap().is_empty());
    let url: Url = fixture
        .url("/custom/%E4%B8%AD/manifest.json?keep=a%2Fb")
        .parse()
        .unwrap();
    let result = resource_url(
        &url,
        "catalog",
        "movie/type",
        "id/%?# 空",
        Some("skip=2&genre=A%20B"),
    )
    .unwrap();
    assert_eq!(result.query(), Some("keep=a%2Fb"));
    assert!(
        result
            .path()
            .starts_with("/custom/%E4%B8%AD/catalog/movie%2Ftype/id%2F%25%3F%23%20%E7%A9%BA/")
    );
    assert!(result.path().ends_with("skip=2&genre=A%2520B.json"));
    assert!(resource_url(&url, "catalog", "..", "id", None).is_err());
    assert!(resource_url(&url, "catalog", "movie", "..", Some("skip=2")).is_err());
    configure(&host, vec![fixture.url("/custom/prefix/manifest.json")]);
    let source = sync(&host);
    first_card(&host, &source);
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.target == "/custom/prefix/catalog/movie/featured.json")
    );
}

#[test]
fn n01_skip_raw_count_dedup_empty_and_no_new_items() {
    let fixture = Fixture::new(|r| {
        if r.target.contains("skip=3") {
            Reply::json(json!({"metas":[meta("tt002","movie"),meta("tt003","movie")]}))
        } else if r.target.contains("skip=5") {
            Reply::json(json!({"metas":[meta("tt003","movie")]}))
        } else if r.target.contains("/catalog/") {
            Reply::json(
                json!({"metas":[meta("tt001","movie"),meta("tt001","movie"),meta("tt002","movie")]}),
            )
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![fixture.url("/addon/manifest.json")]);
    let source = sync(&host);
    let home = ok(&host, "get_home", home_input(&source));
    let first = &home["page"]["sections"][0];
    assert_eq!(first["items"].as_array().unwrap().len(), 2);
    let mut input = home_input(&source);
    input["section_ref"] = first["section_ref"].clone();
    input["page"] = json!(2);
    let second = ok(&host, "get_home", input.clone());
    let second = &second["page"]["sections"][0];
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["next_page"], 3);
    input["page"] = json!(3);
    let third = ok(&host, "get_home", input.clone());
    assert_eq!(third["page"]["sections"][0]["items"], json!([]));
    assert_eq!(third["page"]["sections"][0]["has_next"], false);
    input["page"] = json!(4);
    error(&call(&host, "get_home", input), "BAD_REQUEST");
    let empty = Fixture::new(|r| {
        if r.target.contains("/catalog/") {
            Reply::json(json!({"metas":[]}))
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![empty.url("/manifest.json")]);
    let source = sync(&host);
    let response = ok(&host, "get_home", home_input(&source));
    assert_eq!(response["page"]["sections"][0]["has_next"], false);
}

#[test]
fn n01_meta_owner_fallback_filters_warnings_and_all_failed() {
    let fixture = Fixture::new(|r| {
        if r.target == "/first/manifest.json" {
            Reply::json(manifest(
                json!([{"name":"meta","types":["movie"],"idPrefixes":["wrong"]}]),
                json!([]),
            ))
        } else if r.target == "/owner/meta/movie/tt001.json" {
            Reply::fail()
        } else if r.target == "/fallback/meta/movie/tt001.json" {
            Reply::json(json!({"meta":meta("tt001","movie")}))
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![
        fixture.url("/first/manifest.json"),
        fixture.url("/fallback/manifest.json"),
        fixture.url("/owner/manifest.json"),
    ]);
    let source = sync(&host);
    let home = ok(&host, "get_home", home_input(&source));
    let card = &home["page"]["sections"][1]["items"][0];
    let details = ok(&host, "get_details", json!({"ref":card["ref"]}));
    assert_eq!(details["details"]["ref"], card["ref"]);
    assert_eq!(details["details"]["id"], card["id"]);
    assert_eq!(details["details"]["warnings"][0]["addon_index"], 2);
    assert_eq!(details["details"]["warnings"][0]["operation"], "meta");
    let requests = fixture.requests.lock().unwrap();
    let metas: Vec<_> = requests
        .iter()
        .filter(|r| r.target.contains("/meta/"))
        .map(|r| r.target.as_str())
        .collect();
    assert_eq!(
        metas,
        [
            "/owner/meta/movie/tt001.json",
            "/fallback/meta/movie/tt001.json"
        ]
    );
    drop(requests);
    let failed = Fixture::new(|r| {
        if r.target.contains("/meta/") {
            Reply::fail()
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![failed.url("/manifest.json")]);
    let source = sync(&host);
    let card = first_card(&host, &source);
    error(
        &call(&host, "get_details", json!({"ref":card["ref"]})),
        "UPSTREAM_FAILED",
    );
}

#[test]
fn n01_catalog_and_meta_separate_addons_and_stream_only() {
    let fixture = Fixture::new(|r| {
        if r.target == "/catalog/manifest.json" {
            Reply::json(manifest(
                json!(["catalog"]),
                standard_manifest()["catalogs"].clone(),
            ))
        } else if r.target == "/meta/manifest.json" {
            Reply::json(manifest(json!(["meta"]), json!([])))
        } else if r.target == "/stream/manifest.json" {
            Reply::json(manifest(json!(["stream", "subtitles"]), json!([])))
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![
        fixture.url("/catalog/manifest.json"),
        fixture.url("/meta/manifest.json"),
        fixture.url("/stream/manifest.json"),
    ]);
    let source = sync(&host);
    assert_eq!(source["capabilities"]["home"], true);
    assert_eq!(source["capabilities"]["details"], true);
    assert_eq!(source["capabilities"]["resolve"], false);
    let card = first_card(&host, &source);
    ok(&host, "get_details", json!({"ref":card["ref"]}));
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.target == "/meta/meta/movie/tt001.json")
    );
    configure(&host, vec![fixture.url("/stream/manifest.json")]);
    let source = sync(&host);
    assert_eq!(source["runtime_status"], "failed");
    assert_eq!(source["error"]["code"], "CAPABILITY_UNAVAILABLE");
    assert!(
        source["capabilities"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v == false)
    );
    error(
        &call(&host, "get_home", home_input(&source)),
        "CAPABILITY_UNAVAILABLE",
    );
}

#[test]
fn n01_movie_and_original_video_order_inline_media_ignored() {
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
    let source = sync(&host);
    let card = first_card(&host, &source);
    let response = ok(&host, "get_details", json!({"ref":card["ref"]}));
    assert_eq!(
        response["details"]["groups"][0]["dub_status"],
        "unspecified"
    );
    assert_eq!(response["details"]["status"], "unknown");
    let episode = &response["details"]["groups"][0]["episodes"][0];
    let fields: Vec<String> =
        serde_json::from_str(episode["ref"]["data"].as_str().unwrap()).unwrap();
    assert_eq!(fields.last().unwrap(), "tt001");
    assert_eq!(response["details"]["plot"], "English plot");
    let encoded = response.to_string();
    for field in [
        "streams",
        "infoHash",
        "ytId",
        "proxyHeaders",
        "secret",
        "127.0.0.1",
    ] {
        assert!(!encoded.contains(field), "leaked field: {field}");
    }
    let cat = catalog(&host);
    let mut value = meta("tt-series", "series");
    value["videos"] = json!([
        {"id":"raw:2/10?x","title":"Ten","season":2,"episode":10,"streams":[{"ytId":"ignored"}]},
        {"id":"raw:0:0.5","title":"Special","season":0,"episode":0.5}, {"id":"raw:1:1","title":"One","season":1,"episode":1}]);
    let mut output = Output::default();
    let groups = cat.videos(0, 0, &value, "content", &mut output).unwrap();
    assert_eq!(groups[0]["dub_status"], "unspecified");
    let episodes: Vec<_> = groups
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["episodes"].as_array().unwrap())
        .collect();
    assert_eq!(groups.as_array().unwrap().len(), 3);
    assert_eq!(groups[0]["season_number"], 2);
    assert_eq!(groups[1]["season_number"], 0);
    assert_eq!(groups[2]["season_number"], 1);
    assert_eq!(
        episodes
            .iter()
            .map(|e| e["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Ten", "Special", "One"]
    );
    assert_eq!(episodes[1]["season"], 0);
    assert_eq!(episodes[1]["number"], 0.5);
    let fields: Vec<String> =
        serde_json::from_str(episodes[0]["ref"]["data"].as_str().unwrap()).unwrap();
    assert_eq!(fields.last().unwrap(), "raw:2/10?x");
    let request = json!({"episode_ref":episode["ref"],"match_id":Uuid::new_v4().to_string()});
    error(
        &call(&host, "resolve_episode", request),
        "CAPABILITY_UNAVAILABLE",
    );
    value["videos"][1]["id"] = value["videos"][0]["id"].clone();
    assert_eq!(
        cat.videos(0, 0, &value, "content", &mut output)
            .unwrap_err()
            .code,
        "UPSTREAM_FAILED"
    );
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| !r.target.contains("/stream/") && !r.target.contains("/subtitles/"))
    );
}

#[test]
fn n01_partial_home_failed_placeholders_retry_and_all_failure() {
    let fixture = Fixture::new(|r| {
        if r.target.starts_with("/bad/catalog/") {
            Reply::fail()
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![
        fixture.url("/good/manifest.json"),
        fixture.url("/bad/manifest.json"),
    ]);
    let source = sync(&host);
    let response = ok(&host, "get_home", home_input(&source));
    assert_eq!(response["page"]["sections"][0]["index"], 0);
    let failed = &response["page"]["failed_sections"][0];
    assert_eq!(failed["index"], 1);
    let mut input = home_input(&source);
    input["section_ref"] = failed["section_ref"].clone();
    error(&call(&host, "get_home", input), "UPSTREAM_FAILED");
    configure(&host, vec![fixture.url("/bad/manifest.json")]);
    let source = sync(&host);
    error(
        &call(&host, "get_home", home_input(&source)),
        "UPSTREAM_FAILED",
    );
}

#[test]
fn n01_public_http_headers_cookies_addresses_and_foreign_refs() {
    let fixture = Fixture::new(|r| {
        let mut reply = standard(r);
        reply
            .headers
            .push(("Set-Cookie".into(), "session=never-replay".into()));
        reply
    });
    let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
    let source = sync(&host);
    let card = first_card(&host, &source);
    ok(&host, "get_details", json!({"ref":card["ref"]}));
    let canonical = card["ref"]["data"].as_str().unwrap();
    for data in [
        format!(" {canonical}"),
        canonical.replacen("\"stremio\"", r#""\u0073tremio""#, 1),
    ] {
        assert_ne!(data, canonical);
        assert_eq!(
            serde_json::from_str::<Value>(&data).unwrap(),
            serde_json::from_str::<Value>(canonical).unwrap()
        );
        let mut reference = card["ref"].clone();
        reference["data"] = json!(data);
        let response = call(&host, "get_details", json!({"ref":reference}));
        error(&response, "BAD_REQUEST");
        assert_eq!(
            response["error"]["message"],
            "Stremio content reference is not canonical"
        );
    }
    for request in fixture.requests.lock().unwrap().iter() {
        for name in ["cookie", "authorization", "proxy-authorization"] {
            assert!(!request.headers.contains_key(name));
        }
    }
    let cat = catalog(&host);
    let mut output = Output::default();
    for address in [
        "file:///etc/passwd",
        "http://127.0.0.1/image",
        "http://[::ffff:127.0.0.1]/image",
        "http://user:pass@public.example/image",
    ] {
        assert!(cat.image(0, Some(address), &mut output).is_err());
    }
    let mut foreign = home_input(&source);
    foreign["section_ref"] = json!("foreign");
    error(&call(&host, "get_home", foreign), "BAD_REQUEST");
    for data in [
        json!(["stremio", "foreign", "movie", "tt001"]).to_string(),
        json!(["stremio", cat.addons[0].key, "movie", ""]).to_string(),
    ] {
        let mut reference = card["ref"].clone();
        reference["data"] = json!(data);
        error(
            &call(&host, "get_details", json!({"ref":reference})),
            "BAD_REQUEST",
        );
    }
    configure(&host, vec!["http://127.0.0.1/manifest.json".into()]);
    error(
        &call(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":CONFIG}),
        ),
        "BAD_REQUEST",
    );
    if std::env::var("N01_PROXY_TEST").as_deref() == Ok("1") {
        for key in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            assert_eq!(std::env::var(key).unwrap(), "http://127.0.0.1:1");
        }
        println!("N01_PROXY_ENV:true");
    }
}

#[derive(Default)]
struct FakeClock(AtomicU64);
impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[test]
fn n01_aggregate_45s_reserve_partial_and_cancellation() {
    let fixture = Fixture::new(|r| {
        if r.target.starts_with("/slow/catalog/") {
            Reply::delayed(json!({"metas":[]}))
        } else {
            standard(r)
        }
    });
    let temp = TempDir::new().unwrap();
    let clock = Arc::new(FakeClock::default());
    let host = Host::with_clock(temp.path().join("settings.json"), clock.clone()).unwrap();
    configure(
        &host,
        vec![
            fixture.url("/fast/manifest.json"),
            fixture.url("/slow/manifest.json"),
        ],
    );
    let source = sync(&host);
    let id = Uuid::new_v4().to_string();
    let call = host
        .admit(
            "get_home",
            json!({"request_id":id,"input":home_input(&source)}),
            true,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(call.job.as_ref().unwrap().budget.total_ms, 45000); // Host budget selection, with real HTTP under a controlled clock.
    let worker_host = host.clone();
    let worker = thread::spawn(move || worker_host.execute(call));
    fixture.wait_for("/fast/catalog/movie/featured.json");
    fixture.wait_for("/slow/catalog/movie/featured.json");
    thread::sleep(Duration::from_millis(150));
    clock.0.store(44000, Ordering::SeqCst);
    let response = worker.join().unwrap();
    assert!(response.get("error").is_none(), "{}", diagnostic(&response));
    assert_eq!(response["data"]["page"]["sections"][0]["index"], 0);
    assert_eq!(response["data"]["page"]["failed_sections"][0]["index"], 1);
    assert_eq!(
        response["data"]["page"]["failed_sections"][0]["error"]["code"],
        "UPSTREAM_TIMEOUT"
    );
    let id = Uuid::new_v4().to_string();
    let args = json!({"request_id":id,"input":home_input(&source)});
    let call = host
        .admit("get_home", args, true)
        .unwrap_or_else(|e| panic!("{e}"));
    let worker_host = host.clone();
    let worker = thread::spawn(move || worker_host.execute(call));
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        ok(&host, "cancel_request", json!({"target_request_id":id}))["cancelled"],
        true
    );
    error(&worker.join().unwrap(), "CANCELLED");
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn n01_sync_sequence_and_edit_disable_delete_readd_cancel() {
    let count = Arc::new(AtomicUsize::new(0));
    let server_count = count.clone();
    let fixture = Fixture::new(move |r| {
        if r.target.ends_with("manifest.json") && server_count.fetch_add(1, Ordering::SeqCst) == 0 {
            Reply::delayed(standard_manifest())
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
    let worker_host = host.clone();
    let worker = thread::spawn(move || {
        call(
            &worker_host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":CONFIG}),
        )
    });
    fixture.wait_for("/manifest.json");
    let latest = sync(&host);
    configure(&host, vec![fixture.url("/edited/manifest.json")]);
    error(&worker.join().unwrap(), "CANCELLED");
    assert_eq!(source(&host)["source_revision"], "unpublished");
    assert_ne!(source(&host)["source_revision"], latest["source_revision"]);
    let captured = host.capture_source(CONFIG, false).unwrap();
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["source_configs"][0]["enabled"] = json!(false);
    ok(&host, "save_settings", settings.clone());
    assert_eq!(
        host.capture_source(CONFIG, false).unwrap_err().code,
        "ENGINE_NOT_READY"
    );
    settings["source_configs"] = json!([]);
    ok(&host, "save_settings", settings);
    configure(&host, vec![fixture.url("/edited/manifest.json")]);
    assert!(host.capture_source(CONFIG, false).unwrap().generation > captured.generation);
    assert_eq!(source(&host)["source_revision"], "unpublished");
}

#[test]
fn n01_manifest_latest_sync_timeout_and_user_cancel() {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let fixture = Fixture::new(move |r| {
        if r.target.ends_with("manifest.json") && seen.fetch_add(1, Ordering::SeqCst) == 0 {
            let mut value = standard_manifest();
            value["version"] = json!("0.9.0");
            Reply {
                delay: Duration::from_millis(400),
                ..Reply::json(value)
            }
        } else {
            standard(r)
        }
    });
    let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
    let worker_host = host.clone();
    let worker = thread::spawn(move || {
        call(
            &worker_host,
            "sync_catalog",
            json!({"kind":"stremio", "config_id":CONFIG}),
        )
    });
    fixture.wait_for("/manifest.json");
    let latest = sync(&host);
    error(&worker.join().unwrap(), "CANCELLED");
    assert_eq!(source(&host), latest);

    let delayed = Fixture::new(|_| Reply::delayed(standard_manifest()));
    let temp = TempDir::new().unwrap();
    let clock = Arc::new(FakeClock::default());
    let host = Host::with_clock(temp.path().join("settings.json"), clock.clone()).unwrap();
    configure(&host, vec![delayed.url("/manifest.json")]);
    let worker_host = host.clone();
    let worker = thread::spawn(move || {
        call(
            &worker_host,
            "sync_catalog",
            json!({"kind":"stremio", "config_id":CONFIG}),
        )
    });
    delayed.wait_for("/manifest.json");
    ok(&host, "runtime_status", json!({}));
    clock.0.store(45000, Ordering::SeqCst);
    let failed = worker.join().unwrap();
    error(&failed, "UPSTREAM_TIMEOUT");
    assert_eq!(failed["error"]["details"]["reason"], "manifest_sync_failed");
    assert_eq!(
        failed["error"]["details"]["failed_addon_indices"],
        json!([0])
    );
    assert_eq!(source(&host)["source_revision"], "unpublished");
    assert_eq!(source(&host)["runtime_status"], "failed");
    let id = Uuid::new_v4().to_string();
    let pending = host
        .admit(
            "sync_catalog",
            json!({"request_id": id, "input": {"kind":"stremio", "config_id":CONFIG}}),
            true,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let worker_host = host.clone();
    let worker = thread::spawn(move || worker_host.execute(pending));
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        ok(&host, "cancel_request", json!({"target_request_id":id}))["cancelled"],
        true
    );
    error(&worker.join().unwrap(), "CANCELLED");
    assert_eq!(host.requests.active_count(), 0);
}

fn bilingual(request: &Request) -> Reply {
    if request.target.ends_with("manifest.json") {
        let mut m = standard_manifest();
        m["id"] = json!("tmdb-addon");
        m["version"] = json!("3.1.7");
        return Reply::json(m);
    }
    let mut value = meta("tmdb:27205", "movie");
    if request.target.contains("zh-CN") {
        value["name"] = json!("盗梦空间（合成）");
        value["description"] = json!("");
        value["genres"] = json!(["剧情"]);
        value["cast"] = json!(["演员"]);
    }
    if request.target.contains("/catalog/") {
        Reply::json(json!({"metas":[value]}))
    } else {
        Reply::json(json!({"meta":value}))
    }
}

#[test]
fn n01_language_field_fallback_identity_cache_and_locale_mapping() {
    for l in ["zh", "zh-CN", "zh-TW", "zh-Hant", "ZH_hk"] {
        assert_eq!(locale(l), "zh-CN");
    }
    for l in ["en", "ja", "fr", "en-US", ""] {
        assert_eq!(locale(l), "en");
    }
    let fixture = Fixture::new(bilingual);
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    let cat = catalog(&host);
    assert_eq!(
        language_url(
            &fixture.url("/tmdb/zh-CN/manifest.json").parse().unwrap(),
            "ja"
        )
        .unwrap()
        .as_str(),
        fixture.url("/tmdb/en-US/manifest.json")
    );
    for language in ["ja", "fr"] {
        let output = cat
            .home(
                &Http::default(),
                &home_input(&source),
                language,
                &job(&host, "stremio_home"),
            )
            .unwrap();
        assert_eq!(
            output.value["page"]["sections"][0]["items"][0]["title"],
            "Synthetic same title"
        );
    }
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| !r.target.contains("/ja/")
                && !r.target.contains("/fr/")
                && !r.target.contains("zh-CN"))
    );
    let en = first_card(&host, &source);
    let en_details = ok(&host, "get_details", json!({"ref":en["ref"]}));
    set_locale(&host, "zh-CN");
    let zh = first_card(&host, &source);
    let zh_details = ok(&host, "get_details", json!({"ref":zh["ref"]}));
    assert_eq!(zh["title"], "盗梦空间（合成）");
    assert_eq!(en["id"], zh["id"]);
    assert_eq!(en["ref"], zh["ref"]);
    assert_eq!(en_details["metadata_ref"], zh_details["metadata_ref"]);
    assert_eq!(zh_details["details"]["plot"], "English plot");
    assert_eq!(zh_details["details"]["tags"], json!(["剧情"]));
    assert_eq!(zh_details["details"]["actors"][0]["name"], "演员");
    assert!(
        zh_details["details"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |w| w["error"]["details"]["reason"] == "missing_field:description"
                    && w["error"]["details"]["effective_locale"] == "en"
            )
    );
    assert_eq!(zh_details["metadata_ref"]["identity"]["namespace"], "tmdb");
    assert_eq!(
        zh_details["metadata_ref"]["identity"]["canonical_id"],
        "27205"
    );
    let count = fixture.requests.lock().unwrap().len();
    set_locale(&host, "en");
    assert_eq!(first_card(&host, &source)["title"], en["title"]);
    set_locale(&host, "zh-CN");
    assert_eq!(first_card(&host, &source)["title"], zh["title"]);
    assert_eq!(fixture.requests.lock().unwrap().len(), count);
    assert_eq!(
        self::source(&host)["source_revision"],
        source["source_revision"]
    );
}

#[test]
fn n01_language_request_failure_english_failure_and_fallback_cache() {
    let fixture = Fixture::new(|r| {
        if r.target.contains("zh-CN") {
            Reply::fail()
        } else {
            bilingual(r)
        }
    });
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    set_locale(&host, "zh-CN");
    let card = first_card(&host, &source);
    let details = ok(&host, "get_details", json!({"ref":card["ref"]}));
    assert_eq!(details["details"]["title"], "Synthetic same title");
    assert!(
        details["details"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["error"]["details"]["reason"] == "localized_request_upstream_failed")
    );
    let count = fixture.requests.lock().unwrap().len();
    ok(&host, "get_details", json!({"ref":card["ref"]}));
    assert_eq!(fixture.requests.lock().unwrap().len(), count);
    let failed = Fixture::new(|r| {
        if r.target.contains("/meta/") {
            Reply::fail()
        } else {
            bilingual(r)
        }
    });
    let (_temp, host) = setup(vec![failed.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    set_locale(&host, "zh-CN");
    let card = first_card(&host, &source);
    error(
        &call(&host, "get_details", json!({"ref":card["ref"]})),
        "UPSTREAM_FAILED",
    );
    let unknown = Fixture::new(standard);
    let (_temp, host) = setup(vec![unknown.url("/manifest.json")]);
    let source = sync(&host);
    set_locale(&host, "zh-CN");
    let card = first_card(&host, &source);
    let details = ok(&host, "get_details", json!({"ref":card["ref"]}));
    assert_eq!(
        details["details"]["warnings"][0]["error"]["details"]["reason"],
        "unverified_language_mapping"
    );
}

#[test]
fn n01_localized_video_matching_and_mismatched_ids() {
    let mut base = meta("series", "series");
    base["videos"] = json!([{"id":"v2","title":"Second","overview":"English second","season":2,"episode":2},{"id":"v1","title":"First","season":0,"episode":0.5}]);
    let mut local = meta("series", "series");
    local["videos"] = json!([{"id":"v1","title":"第一","season":99,"episode":99},{"id":"unknown","title":"wrong"}]);
    let original = base.clone();
    let mut output = Output::default();
    overlay(&mut base, &local, 0, "meta", &mut output);
    assert_eq!(base["videos"][0], original["videos"][0]);
    assert_eq!(base["videos"][1]["title"], "第一");
    assert_eq!(base["videos"][1]["season"], 0);
    assert_eq!(base["videos"][1]["episode"], 0.5);
    assert!(!output.diagnostics.is_empty());
    let fixture = Fixture::new(|r| {
        if r.target.contains("zh-CN/meta/") {
            let mut wrong = meta("tmdb:wrong", "movie");
            wrong["name"] = json!("wrong");
            Reply::json(json!({"meta":wrong}))
        } else {
            bilingual(r)
        }
    });
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    let card = first_card(&host, &source);
    set_locale(&host, "zh-CN");
    let details = ok(&host, "get_details", json!({"ref":card["ref"]}));
    assert_eq!(details["details"]["title"], card["title"]);
    assert!(
        details["details"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["error"]["details"]["reason"] == "localized_identity_mismatch")
    );
}

#[test]
fn n01_chinese_english_chinese_race_and_queued_calls() {
    let delayed = Arc::new(AtomicBool::new(true));
    let server = delayed.clone();
    let fixture = Fixture::new(move |r| {
        if r.target.contains("zh-CN/catalog/") && server.swap(false, Ordering::SeqCst) {
            Reply::delayed(json!({"metas":[meta("tmdb:27205","movie")]}))
        } else {
            bilingual(r)
        }
    });
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    set_locale(&host, "zh-CN");
    let id = Uuid::new_v4().to_string();
    let old = host
        .admit(
            "get_home",
            json!({"request_id":id,"input":home_input(&source)}),
            true,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let worker_host = host.clone();
    let worker = thread::spawn(move || worker_host.execute(old));
    fixture.wait_for("/tmdb/zh-CN/catalog/movie/featured.json");
    let queued = host
        .admit(
            "get_home",
            json!({"request_id":Uuid::new_v4().to_string(),"input":home_input(&source)}),
            true,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    set_locale(&host, "en");
    let en = first_card(&host, &source);
    set_locale(&host, "zh-CN");
    let zh = first_card(&host, &source);
    assert_ne!(en["title"], zh["title"]);
    error(&worker.join().unwrap(), "CANCELLED");
    error(&host.execute(queued), "CANCELLED");
    assert_eq!(first_card(&host, &source)["title"], zh["title"]);
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn n01_language_storage_failure_preserves_confirmed_locale() {
    let fixture = Fixture::new(bilingual);
    let (temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    let queued = host
        .admit(
            "get_home",
            json!({"request_id":Uuid::new_v4().to_string(), "input":home_input(&source)}),
            true,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let path = temp.path().join("settings.json");
    let backup = temp.path().join("confirmed-settings.json");
    std::fs::rename(&path, &backup).unwrap();
    std::fs::create_dir(&path).unwrap();
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["ui_locale"] = json!("zh-CN");
    error(&call(&host, "save_settings", settings), "INTERNAL_ERROR");
    assert_eq!(ok(&host, "get_settings", json!({}))["ui_locale"], "en");
    let response = host.execute(queued);
    assert!(response.get("error").is_none(), "{}", diagnostic(&response));
    assert_eq!(
        response["data"]["page"]["sections"][0]["items"][0]["title"],
        "Synthetic same title"
    );
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| !r.target.contains("zh-CN"))
    );
    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(&backup, &path).unwrap();
    let restarted = Host::open(path).unwrap();
    assert_eq!(ok(&restarted, "get_settings", json!({}))["ui_locale"], "en");
}

#[test]
fn n01_cache_bounds_revision_and_synthetic_vectors() {
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
    let cat = catalog(&host);
    let mut output = Output::default();
    for index in 0..140 {
        output
            .cache
            .push((index.to_string(), json!({"value":index})));
    }
    cat.commit(&output).unwrap();
    assert_eq!(cat.state.lock().unwrap().cache.len(), 128);
    assert!(cat.state.lock().unwrap().cache_bytes <= CACHE_BYTES);
    let vectors: Value =
        serde_json::from_str(include_str!("../../protocol/examples/contract-cases.json")).unwrap();
    let vectors = vectors["metadata_identity_vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 2);
    for vector in vectors {
        assert_eq!(
            identity("addon", &[vector["manifest_url"].as_str().unwrap()]),
            vector["addon_key"]
        );
        assert_eq!(
            identity(
                "metadata",
                &[
                    vector["namespace"].as_str().unwrap(),
                    vector["type"].as_str().unwrap(),
                    vector["canonical_id"].as_str().unwrap()
                ]
            ),
            vector["content_id"]
        );
    }
    assert!(include_str!("../../protocol/examples/contract-cases.json").contains("fixture"));
    let source = sync(&host);
    let card = first_card(&host, &source);
    let args = json!({"request_id":Uuid::new_v4().to_string(), "input":home_input(&source)});
    let mut response = host.invoke("get_home", args.clone(), true);
    let poster = "/data/page/sections/0/items/0/poster";
    *response.pointer_mut(poster).unwrap() = json!({
        "resource_id":"synthetic-image", "path":"/images/synthetic-image",
        "capability":"s".repeat(43), "expires_at":"2026-10-28T11:00:00Z"
    });
    response["data"]["images"].as_object_mut().unwrap().insert(
        "synthetic-image".into(),
        json!({
            "resource_id":"synthetic-image",
            "url":format!(
                "http://127.0.0.1:1234/images/synthetic-image?cap={}&keep=a%2Fb",
                "s".repeat(43)
            ),
            "expires_at":"2026-10-28T11:00:00Z"
        }),
    );
    let image_url = "/data/images/synthetic-image/url";
    let original = response.clone();
    let observed = observation("get_home", &args, &response).unwrap();
    Contracts::get()
        .validate("Result_get_home", &observed["response"])
        .unwrap();
    let capability = format!("{poster}/capability");
    let mut expected = response.clone();
    *expected.pointer_mut(&capability).unwrap() = json!("r".repeat(43));
    for image in expected["data"]["images"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        let mut url = Url::parse(image["url"].as_str().unwrap()).unwrap();
        let pairs: Vec<_> = url
            .query_pairs()
            .map(|(key, value)| {
                let value = if key == "cap" {
                    "r".repeat(43)
                } else {
                    value.into_owned()
                };
                (key.into_owned(), value)
            })
            .collect();
        url.query_pairs_mut().clear().extend_pairs(pairs);
        image["url"] = json!(url.as_str());
    }
    assert!(
        observed["response"] == expected,
        "safe observation changed stable response fields"
    );
    assert!(response == original, "observing mutated the real response");
    assert_eq!(observed["args"], args);
    assert_eq!(observed["capabilities_redacted"], true);
    assert_eq!(
        response.pointer(&capability).unwrap(),
        &json!("s".repeat(43))
    );
    for invalid in [
        json!("REDACTED"),
        json!("s".repeat(42)),
        json!("s".repeat(44)),
        json!("/".repeat(43)),
        Value::Null,
        json!(7),
    ] {
        let mut response = response.clone();
        *response.pointer_mut(&capability).unwrap() = invalid;
        assert!(observation("get_home", &args, &response).is_err());
    }
    let mut invalid_url = response.clone();
    *invalid_url.pointer_mut(image_url).unwrap() = json!(format!(
        "http://127.0.0.1:1234/images/synthetic-image?cap={}",
        "s".repeat(42)
    ));
    assert!(observation("get_home", &args, &invalid_url).is_err());
    configure(&host, vec![fixture.url("/other/manifest.json")]);
    let newer = sync(&host);
    assert_ne!(source["source_revision"], newer["source_revision"]);
    error(
        &call(&host, "get_details", json!({"ref":card["ref"]})),
        "REVISION_MISMATCH",
    );
}

#[test]
fn n01_live_selected_tmdb_en_zh_evidence() {
    // This assertion deliberately fails without real responses. No fixture or
    // cached protocol vector can satisfy the live portion of N01.
    let folder = std::env::var_os("N01_LIVE_EVIDENCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("evidence/acceptance")
                .join(format!("N01-verify-live-{}", Uuid::new_v4()))
        });
    std::fs::create_dir_all(&folder).unwrap();
    let (_temp, host) = setup(vec![
        "https://tmdb.elfhosted.com/en-US/manifest.json".into(),
    ]);
    let http = Http::default();
    let mut records = Vec::new();
    let mut responses = Vec::new();
    for language in ["en-US", "zh-CN"] {
        for route in ["manifest.json", "meta/movie/tmdb:27205.json"] {
            let url = format!("https://tmdb.elfhosted.com/{language}/{route}");
            let received = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
            let response = http
                .get(
                    url.parse().unwrap(),
                    HeaderMap::new(),
                    &job(&host, "default_metadata"),
                    LIMIT,
                    false,
                    true,
                )
                .unwrap_or_else(|e| panic!("live TMDB {language} {route}: {}", e.code));
            let file = format!(
                "{}-{}.json",
                language,
                if route == "manifest.json" {
                    "manifest"
                } else {
                    "meta"
                }
            );
            std::fs::write(folder.join(&file), &response.body).unwrap();
            let value: Value = serde_json::from_slice(&response.body).unwrap();
            if route == "manifest.json" {
                Contracts::get()
                    .validate("StremioManifest", &value)
                    .unwrap();
                assert_eq!(value["id"], "tmdb-addon");
                assert!(text(&value["version"]).is_some());
            } else {
                valid_meta(&value["meta"], "movie", Some("tmdb:27205"), true).unwrap();
                responses.push(value.clone());
            }
            records.push(json!({"requested_url":url,"final_url":response.url.as_str(),"method":"GET","headers":{},"received_utc":received,"body":file,"sha256":format!("{:x}",Sha256::digest(&response.body)),"bytes":response.body.len(),"manifest_version":if route=="manifest.json"{value["version"].clone()}else{Value::Null}}));
        }
    }
    assert_ne!(responses[0]["meta"]["name"], responses[1]["meta"]["name"]);
    assert!(
        responses[1]["meta"]["name"]
            .as_str()
            .unwrap()
            .chars()
            .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    );
    std::fs::write(
        folder.join("requests.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
    let source = sync(&host);
    ok(&host, "get_home", home_input(&source));
    let reference = json!({"source_id": source["id"], "source_revision": source["source_revision"], "data": json!(["stremio", identity("addon", &["https://tmdb.elfhosted.com/en-US/manifest.json"]), "movie", "tmdb:27205"]).to_string()});
    let en = ok(&host, "get_details", json!({"ref":reference}));
    set_locale(&host, "zh-CN");
    let zh = ok(&host, "get_details", json!({"ref":reference}));
    assert_eq!(en["metadata_ref"], zh["metadata_ref"]);
    assert_ne!(en["details"]["title"], zh["details"]["title"]);
    assert_eq!(source["capabilities"]["resolve"], false);
    println!("N01_LIVE_EVIDENCE:{}", folder.display());
}
