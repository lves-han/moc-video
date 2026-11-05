//! Compiled as unit tests of the production module, so the two fixed fixture
//! hostnames are unavailable in ordinary application builds.
#[path = "t02_capture.rs"]
mod t02;
#[path = "t02_dns.rs"]
mod t02_dns;
#[path = "t02_json.rs"]
mod t02_json;
#[path = "t06_maccms.rs"]
mod t06;
#[path = "t07_maccms.rs"]
mod t07;
#[path = "t08_http.rs"]
mod t08;
use super::*;
use crate::{host::Host, requests::Clock, settings};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
};
use tempfile::TempDir;
use uuid::Uuid;

const CONFIG: &str = "11111111-1111-4111-8111-111111111111";

#[derive(Clone, Debug)]
struct Request {
    target: String,
    headers: HashMap<String, String>,
    query: HashMap<String, String>,
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    header_delay: Duration,
    body_delay: Duration,
    length: Option<usize>,
    chunked: bool,
}

impl Reply {
    fn raw(body: impl Into<Vec<u8>>, mime: &str) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".into(), mime.into())],
            body: body.into(),
            header_delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            length: None,
            chunked: false,
        }
    }
    fn json(value: Value) -> Self {
        Self::raw(value.to_string(), "application/json")
    }
    fn redirect(to: String) -> Self {
        Self {
            status: 302,
            headers: vec![("Location".into(), to)],
            ..Self::raw("", "text/plain")
        }
    }
}

struct Fixture {
    port: u16,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    active: Arc<AtomicUsize>,
}

impl Fixture {
    fn new(handler: impl Fn(&Request, u16) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(vec![]));
        let active = Arc::new(AtomicUsize::new(0));
        let active_worker = active.clone();
        let (stop_worker, observed, handler) = (stop.clone(), requests.clone(), Arc::new(handler));
        let worker = thread::spawn(move || {
            let mut clients = Vec::new();
            while !stop_worker.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (stop, observed, handler) =
                            (stop_worker.clone(), observed.clone(), handler.clone());
                        let active = active_worker.clone();
                        clients.push(thread::spawn(move || {
                            active.fetch_add(1, Ordering::SeqCst);
                            serve(stream, port, &stop, &observed, &*handler);
                            active.fetch_sub(1, Ordering::SeqCst);
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("fixture accept failed: {e}"),
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            port,
            requests,
            stop,
            worker: Some(worker),
            active,
        }
    }
    fn url(&self, path: &str) -> String {
        format!("http://maccms.fixture.test:{}{path}", self.port)
    }
    fn wait_for(&self, predicate: impl Fn(&Request) -> bool) {
        let start = Instant::now();
        while !self.requests.lock().unwrap().iter().any(&predicate) {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "fixture request did not arrive"
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

struct DnsFixture {
    address: SocketAddr,
    queries: Arc<Mutex<Vec<u16>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl DnsFixture {
    fn new(handler: impl Fn(u16) -> Option<Vec<u8>> + Send + 'static) -> Self {
        Self::with_rcode(0, handler)
    }

    fn with_rcode(rcode: u8, handler: impl Fn(u16) -> Option<Vec<u8>> + Send + 'static) -> Self {
        let socket = std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let queries = Arc::new(Mutex::new(vec![]));
        let (halt, observed) = (stop.clone(), queries.clone());
        let worker = thread::spawn(move || {
            let mut packet = [0; 4096];
            while !halt.load(Ordering::SeqCst) {
                let (size, peer) = match socket.recv_from(&mut packet) {
                    Ok(received) => received,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        continue;
                    }
                    Err(e) => panic!("fixture DNS receive failed: {e}"),
                };
                let mut end = 12;
                while packet[end] != 0 {
                    end += 1 + usize::from(packet[end]);
                }
                end += 1;
                let kind = u16::from_be_bytes([packet[end], packet[end + 1]]);
                observed.lock().unwrap().push(kind);
                let Some(data) = handler(kind) else {
                    continue;
                };
                assert!(end + 4 <= size);
                let mut response = vec![
                    packet[0],
                    packet[1],
                    0x81,
                    0x80 | rcode,
                    0,
                    1,
                    0,
                    u8::from(!data.is_empty()),
                    0,
                    0,
                    0,
                    0,
                ];
                response.extend_from_slice(&packet[12..end + 4]);
                if !data.is_empty() {
                    response.extend_from_slice(&[0xc0, 0x0c]);
                    response.extend_from_slice(&kind.to_be_bytes());
                    response.extend_from_slice(&[0, 1, 0, 0, 0, 60]);
                    response.extend_from_slice(&(data.len() as u16).to_be_bytes());
                    response.extend_from_slice(&data);
                }
                socket.send_to(&response, peer).unwrap();
            }
        });
        Self {
            address,
            queries,
            stop,
            worker: Some(worker),
        }
    }

    fn wait_for_query(&self) {
        let start = Instant::now();
        while self.queries.lock().unwrap().is_empty() {
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(2));
        }
    }
}

impl Drop for DnsFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn delay(duration: Duration, stop: &AtomicBool) -> bool {
    let start = Instant::now();
    while start.elapsed() < duration {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        thread::sleep(Duration::from_millis(2));
    }
    !stop.load(Ordering::SeqCst)
}

fn serve(
    mut stream: TcpStream,
    port: u16,
    stop: &AtomicBool,
    observed: &Mutex<Vec<Request>>,
    handler: &dyn Fn(&Request, u16) -> Reply,
) {
    // Darwin inherits the listener's nonblocking mode. Large write_all calls
    // need blocking I/O with the bounded timeouts below to avoid truncation.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut input = Vec::new();
    let started = Instant::now();
    while !input.windows(4).any(|s| s == b"\r\n\r\n") {
        if stop.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(5) {
            return;
        }
        let mut buffer = [0; 4096];
        match stream.read(&mut buffer) {
            Ok(0) => return,
            Ok(n) => input.extend_from_slice(&buffer[..n]),
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
        assert!(input.len() <= 1024 * 1024);
    }
    let input = String::from_utf8(input).unwrap();
    let mut lines = input.lines();
    let target = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    let url = Url::parse(&format!("http://maccms.fixture.test:{port}{target}")).unwrap();
    let headers = lines
        .filter_map(|s| s.split_once(':'))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let request = Request {
        target,
        headers,
        query: url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect(),
    };
    observed.lock().unwrap().push(request.clone());
    let reply = handler(&request, port);
    if !delay(reply.header_delay, stop) {
        return;
    }
    let mut headers = format!("HTTP/1.1 {} Fixture\r\nConnection: close\r\n", reply.status);
    for (name, value) in reply.headers {
        headers.push_str(&format!("{name}: {value}\r\n"));
    }
    if reply.chunked {
        headers.push_str("Transfer-Encoding: chunked\r\n\r\n");
    } else {
        headers.push_str(&format!(
            "Content-Length: {}\r\n\r\n",
            reply.length.unwrap_or(reply.body.len())
        ));
    }
    if stream.write_all(headers.as_bytes()).is_err() || !delay(reply.body_delay, stop) {
        return;
    }
    if reply.chunked {
        for chunk in reply.body.chunks(16 * 1024) {
            if stop.load(Ordering::SeqCst)
                || write!(stream, "{:x}\r\n", chunk.len()).is_err()
                || stream.write_all(chunk).is_err()
                || stream.write_all(b"\r\n").is_err()
            {
                return;
            }
        }
        let _ = stream.write_all(b"0\r\n\r\n");
    } else {
        let _ = stream.write_all(&reply.body);
    }
}

fn response(items: Value, classes: Value) -> Value {
    json!({"code": 1, "page": "1", "pagecount": "2", "total": "2", "class": classes, "list": items})
}

fn item(port: u16) -> Value {
    json!({"vod_id": "视频/😀+&%$#", "vod_name": "影片中文 😀", "vod_pic": "/poster.png?sig=a%2Fb",
        "vod_content": "<p>剧情 &amp; 内容</p><script>window.secret</script><style>body{}</style>",
        "vod_actor": "甲,乙", "vod_director": "丙", "vod_area": "中国/日本", "vod_lang": "中文,日语",
        "vod_year": "2026", "vod_score": "8.5", "vod_isend": "1", "vod_remarks": "全2集", "vod_class": "剧情,悬疑",
        "vod_play_from": "线路甲$$$线路乙",
        "vod_play_url": format!("第10集$http://maccms.fixture.test:{port}/a.mp4?sig=a$b%23c#第2集$http://maccms.fixture.test:{port}/b.m3u8$$$http://maccms.fixture.test:{port}/c.mp4?x=$keep")})
}

fn standard(request: &Request, port: u16) -> Reply {
    let mut value = response(
        json!([item(port)]),
        json!([{"type_id": "类/#$+%", "type_name": "中文分类"}]),
    );
    if let Some(page) = request.query.get("pg") {
        value["page"] = json!(page);
    }
    Reply::json(value)
}

fn id() -> String {
    Uuid::new_v4().to_string()
}

use crate::host::observation_redaction::{diagnostic, redact};

fn call(host: &Host, command: &str, input: Value) -> Value {
    let args = json!({"request_id": id(), "input": input});
    let result = host.invoke(command, args.clone(), true);
    Contracts::get()
        .validate(&format!("Result_{command}"), &result)
        .unwrap();
    let mut observation = json!({"command": command, "args": args, "response": result, "capabilities_redacted": true});
    redact(&mut observation);
    println!("N00_OBSERVATION:{observation}");
    result
}

fn ok(host: &Host, command: &str, input: Value) -> Value {
    let result = call(host, command, input);
    assert!(result.get("error").is_none(), "{}", diagnostic(&result));
    result["data"].clone()
}

fn error(result: &Value, code: &str) {
    let mut diagnostic = result.clone();
    redact(&mut diagnostic);
    assert_eq!(result["error"]["code"], code, "{diagnostic}");
    assert!(
        result.get("data").is_none(),
        "error became an empty success"
    );
}

fn setup(url: String) -> (TempDir, Arc<Host>) {
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([{"kind": "maccms", "config_id": CONFIG, "name": "受控来源", "api_url": url, "enabled": true}]);
    ok(&host, "save_settings", settings);
    (temp, host)
}

fn sync(host: &Host) -> Value {
    let result = ok(
        host,
        "sync_catalog",
        json!({"kind": "maccms", "config_id": CONFIG}),
    );
    assert_eq!(result["kind"], "maccms");
    assert_eq!(result["config_id"], CONFIG);
    assert!(result["changed"].is_boolean());
    assert!(result["cloudstream_result"].is_null());
    assert_eq!(result["sources"], ok(host, "get_sources", json!({})));
    result["sources"]["providers"][0].clone()
}

fn job(host: &Host, operation: &str) -> Job {
    host.requests
        .begin(&id(), operation, host.clock.clone())
        .unwrap()
}

fn catalog(host: &Host) -> Catalog {
    Catalog::sync(
        host.capture_source(CONFIG, false).unwrap(),
        &Http::default(),
        &job(host, "http_source_sync"),
    )
    .unwrap()
}

fn home_input(source: &Value) -> Value {
    json!({"source_id": source["id"], "source_revision": source["source_revision"], "page": 1})
}

#[test]
fn n00_host_sync_home_details_and_native_images() {
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(fixture.url("/api?token=a%2Fb&keep=%2523"));
    let initial = ok(
        &host,
        "sync_catalog",
        json!({"kind": "maccms", "config_id": CONFIG}),
    );
    assert_eq!(initial["changed"], true);
    let source = initial["sources"]["providers"][0].clone();
    assert_eq!(source["runtime_status"], "ready");
    assert_eq!(source["capabilities"]["resolve"], true);
    let home = ok(&host, "get_home", home_input(&source));
    let section = &home["page"]["sections"][0];
    assert_eq!(section["title"], "中文分类");
    assert_eq!(section["index"], 0);
    assert_eq!(section["next_page"], 2);
    let card = &section["items"][0];
    assert_eq!(card["title"], "影片中文 😀");
    let first = ok(&host, "get_details", json!({"ref": card["ref"]}));
    let details = &first["details"];
    assert_eq!(details["id"], card["id"]);
    assert_eq!(details["ref"], card["ref"]);
    assert_eq!(details["plot"], "剧情 & 内容");
    assert_eq!(details["year"], 2026);
    assert_eq!(details["rating"]["value"], 8.5);
    assert_eq!(details["status"], "completed");
    assert_eq!(details["update_label"], "全2集");
    assert_eq!(details["actors"][1]["name"], "乙");
    assert_eq!(details["directors"][0]["name"], "丙");
    assert_eq!(details["regions"], json!(["中国", "日本"]));
    assert_eq!(details["languages"], json!(["中文", "日语"]));
    assert_eq!(details["groups"][0]["episodes"][0]["label"], "第10集");
    assert_eq!(details["groups"][0]["episodes"][1]["label"], "第2集");
    assert_eq!(details["groups"][0]["episodes"][1]["index"], 1);
    assert_eq!(details["groups"][1]["episodes"][0]["label"], "1");
    let second = ok(&host, "get_details", json!({"ref": card["ref"]}));
    assert_eq!(
        details["poster"]["resource_id"],
        second["details"]["poster"]["resource_id"]
    );
    assert!(
        details["poster"]["capability"] == second["details"]["poster"]["capability"],
        "image capability changed on repeat details"
    );
    assert_eq!(details["poster"]["capability"].as_str().unwrap().len(), 43);
    assert!(!first.to_string().contains("sig=a") && !first.to_string().contains("http://maccms"));
    let committed = host.committed.lock().unwrap();
    let catalog = &committed.maccms[source["id"].as_str().unwrap()];
    assert_eq!(catalog.images.len(), 1);
    assert!(
        catalog
            .images
            .values()
            .next()
            .unwrap()
            .url
            .ends_with("/poster.png?sig=a%2Fb")
    );
    drop(committed);
    assert_eq!(
        ok(&host, "runtime_status", json!({}))["cloudstream"]["phase"],
        "stopped"
    );
    assert!(
        host.gateway_address()
            .is_some_and(|address| address.ip().is_loopback())
    );
    assert_eq!(first["images"].as_object().unwrap().len(), 1);
    let refreshed = ok(
        &host,
        "sync_catalog",
        json!({"kind": "maccms", "config_id": CONFIG}),
    );
    assert_eq!(refreshed["changed"], false);
    assert_eq!(refreshed["sources"]["providers"][0], source);
    let after_sync = ok(&host, "get_details", json!({"ref": card["ref"]}));
    assert!(
        details["poster"]["capability"] == after_sync["details"]["poster"]["capability"],
        "image capability changed on unchanged sync"
    );
    for request in fixture.requests.lock().unwrap().iter() {
        assert_eq!(request.query["token"], "a/b");
        assert_eq!(request.query["keep"], "%23");
    }
    error(
        &call(
            &host,
            "resolve_episode",
            json!({"episode_ref": details["groups"][0]["episodes"][0]["ref"], "match_id": "unregistered"}),
        ),
        "SESSION_EXPIRED",
    );
}

#[test]
fn n00_query_search_encoding_and_empty_results() {
    let fixture = Fixture::new(|request, port| {
        if request.query.contains_key("wd") {
            Reply::json(response(json!([]), json!([])))
        } else {
            standard(request, port)
        }
    });
    let (_temp, host) = setup(fixture.url("/api?token=a%2fb&space=a%20b&wd=old&ac=old&pg=99"));
    let catalog = catalog(&host);
    let keyword = "中文 😀 /+$#% &";
    assert_eq!(
        catalog
            .search(
                &Http::default(),
                keyword,
                1,
                &job(&host, "default_metadata")
            )
            .unwrap()
            .value,
        json!([])
    );
    let requests = fixture.requests.lock().unwrap();
    let search = requests
        .iter()
        .find(|r| r.query.contains_key("wd"))
        .unwrap();
    assert_eq!(search.query["wd"], keyword);
    assert_eq!(search.query["ac"], "detail");
    assert_eq!(search.query["pg"], "1");
    assert_eq!(search.query["token"], "a/b");
    assert!(search.target.contains("token=a%2fb&space=a%20b"));
    assert_eq!(search.target.matches("wd=").count(), 1);
    assert!(search.target.contains("%24%23%25") && !search.target.contains("%2524"));
    drop(requests);
    let source = sync(&host);
    let count = fixture.requests.lock().unwrap().len();
    for page in [json!("one"), json!("2"), json!(0), json!(1.5)] {
        let mut input = home_input(&source);
        input["page"] = page;
        error(&call(&host, "get_home", input), "BAD_REQUEST");
    }
    assert_eq!(fixture.requests.lock().unwrap().len(), count);
}

#[test]
fn n00_protocol_numbers_closed_and_errors() {
    let mut valid = response(json!([]), json!([]));
    let fixture = Fixture::new(move |_, _| Reply::json(valid.clone()));
    let (_temp, host) = setup(fixture.url("/api"));
    let catalog = catalog(&host);
    assert_eq!(catalog.sections[0].title, "最近更新");
    valid = response(json!([]), json!([]));
    let raw = |value: Value| Response {
        url: "https://public.example/api".parse().unwrap(),
        headers: HeaderMap::new(),
        body: value.to_string().into_bytes(),
    };
    for page in [json!(1), json!("1")] {
        let mut v = valid.clone();
        v["page"] = page;
        assert!(decode(&raw(v)).is_ok());
    }
    for (key, value) in [
        ("code", json!(0)),
        ("code", json!("1")),
        ("closed", json!(1)),
        ("closed", json!("closed")),
        ("page", json!("one")),
        ("page", json!("0")),
        ("pagecount", json!("-1")),
        ("total", json!("1.5")),
        ("total", json!(true)),
        ("total", json!(9007199254740992u64)),
        ("list", json!(null)),
    ] {
        let mut v = valid.clone();
        v[key] = value;
        assert_eq!(
            decode(&raw(v)).unwrap_err().code,
            "UPSTREAM_FAILED",
            "{key}"
        );
    }
    let mut non_json = raw(json!({}));
    non_json.body = b"<html>closed</html>".to_vec();
    assert_eq!(decode(&non_json).unwrap_err().code, "UPSTREAM_FAILED");
    for body in [
        json!({"code": 0, "list": []}).to_string(),
        "<html>closed</html>".into(),
    ] {
        let fixture = Fixture::new(move |_, _| Reply::raw(body.clone(), "text/html"));
        let (_temp, host) = setup(fixture.url("/api"));
        error(
            &call(
                &host,
                "sync_catalog",
                json!({"kind": "maccms", "config_id": CONFIG}),
            ),
            "UPSTREAM_FAILED",
        );
    }
}

#[test]
fn n00_archived_identity_and_protocol_vectors() {
    let examples: Value =
        serde_json::from_str(include_str!("../../protocol/examples/contract-cases.json")).unwrap();
    let vector = &examples["http_identity_examples"][0];
    assert_eq!(
        identity("http_source", &["maccms", CONFIG]),
        vector["source_id"]
    );
    let parts: Vec<_> = vector["revision_parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        identity("maccms_revision", &parts),
        vector["source_revision"]
    );
    let parts: Vec<_> = vector["section_parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(identity("maccms_section", &parts), vector["section_id"]);
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(fixture.url("/api"));
    let catalog = catalog(&host);
    let vector = &examples["protocol_vectors"]["maccms"];
    let item = &vector["response"]["list"][0];
    let data = serde_json::to_string(&["maccms", "123"]).unwrap();
    let mapped = catalog
        .map_details(item, &catalog.media_ref(&data))
        .unwrap();
    let groups = &mapped.value["details"]["groups"];
    assert_eq!(groups[0]["title"], vector["expected_group_labels"][0]);
    assert_eq!(groups[1]["title"], vector["expected_group_labels"][1]);
    assert_eq!(
        groups[0]["episodes"][0]["label"],
        vector["expected_episode_labels"][0][0]
    );
    assert_eq!(
        groups[1]["episodes"][0]["label"],
        vector["expected_episode_labels"][1][0]
    );
}

#[test]
fn n00_group_order_url_fidelity_and_stale_fingerprints() {
    let changed = Arc::new(AtomicBool::new(false));
    let server_changed = changed.clone();
    let fixture = Fixture::new(move |request, port| {
        let mut value = standard(request, port);
        if server_changed.load(Ordering::SeqCst) {
            let mut parsed: Value = serde_json::from_slice(&value.body).unwrap();
            parsed["list"][0]["vod_play_url"] = json!(format!(
                "第10集$http://maccms.fixture.test:{port}/replaced.mp4$$$http://maccms.fixture.test:{port}/c.mp4"
            ));
            value = Reply::json(parsed);
        }
        value
    });
    let (_temp, host) = setup(fixture.url("/api"));
    let catalog = catalog(&host);
    let vod = item(fixture.port);
    let data = serde_json::to_string(&["maccms", vod["vod_id"].as_str().unwrap()]).unwrap();
    let mapped = catalog
        .details(
            &Http::default(),
            &catalog.media_ref(&data),
            &job(&host, "default_metadata"),
        )
        .unwrap();
    let groups = &mapped.value["details"]["groups"];
    let first = &groups[0]["episodes"][0]["ref"];
    let second = &groups[0]["episodes"][1]["ref"];
    let bare = &groups[1]["episodes"][0]["ref"];
    let link = catalog
        .resolve(&Http::default(), first, &job(&host, "resolve"))
        .unwrap();
    assert_eq!(link.url, fixture.url("/a.mp4?sig=a$b%23c"));
    assert_eq!(link.kind, "progressive");
    assert!(link.headers.is_empty());
    assert_eq!(
        catalog
            .resolve(&Http::default(), bare, &job(&host, "resolve"))
            .unwrap()
            .url,
        fixture.url("/c.mp4?x=$keep")
    );
    assert_eq!(
        catalog
            .resolve(&Http::default(), second, &job(&host, "resolve"))
            .unwrap()
            .kind,
        "hls"
    );
    let mut foreign = first.clone();
    foreign["source_id"] = json!("another-source");
    assert_eq!(
        catalog
            .resolve(&Http::default(), &foreign, &job(&host, "resolve"))
            .unwrap_err()
            .code,
        "BAD_REQUEST"
    );
    changed.store(true, Ordering::SeqCst);
    for reference in [first, second, bare] {
        assert_eq!(
            catalog
                .resolve(&Http::default(), reference, &job(&host, "resolve"))
                .unwrap_err()
                .code,
            "UPSTREAM_FAILED"
        );
    }
}

#[test]
fn n00_ambiguous_groups_and_unknown_metadata() {
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(fixture.url("/api"));
    let catalog = catalog(&host);
    for (names, urls) in [
        (json!("A$$$B"), json!("1$https://a.example/one.mp4")),
        (
            json!("A"),
            json!("1$https://a.example/one.mp4$$$2$https://a.example/two.mp4"),
        ),
        (
            json!("A"),
            json!("1$https://a.example/one.mp4##2$https://a.example/two.mp4"),
        ),
        (json!("A"), json!("缺集$")),
        (json!(""), json!("https://a.example/one.mp4")),
        (json!("A"), json!(7)),
        (json!(null), json!("")),
    ] {
        let mut value = item(fixture.port);
        value["vod_play_from"] = names;
        value["vod_play_url"] = urls;
        if value["vod_play_from"].is_null() {
            assert!(catalog.groups(&value, "content").unwrap().is_empty());
        } else {
            assert_eq!(
                catalog.groups(&value, "content").unwrap_err().code,
                "UPSTREAM_FAILED"
            );
        }
    }
    let mut value = item(fixture.port);
    value["vod_play_from"] = json!("");
    value["vod_play_url"] = json!("");
    value["vod_year"] = json!("unknown");
    value["vod_score"] = json!("NaN");
    value["vod_isend"] = Value::Null;
    let card = catalog.card(&value, &mut vec![]).unwrap();
    assert!(card["year"].is_null() && card["rating"].is_null());
    assert_eq!(card["status"], "unknown");
    assert!(catalog.groups(&value, "content").unwrap().is_empty());
}

#[test]
fn n00_type_probe_formats_html_unknown_and_bound() {
    let fixture = Fixture::new(|request, port| {
        if request.target.starts_with("/probe/") {
            return match request.target.as_str() {
                "/probe/hls" => Reply::raw("#EXTM3U\n#EXTINF:1\na.ts", "application/octet-stream"),
                "/probe/mp4" => {
                    Reply::raw(b"\0\0\0\x18ftypisom".to_vec(), "application/octet-stream")
                }
                "/probe/dash" => {
                    Reply::raw("<?xml version=\"1.0\"?><MPD></MPD>", "application/xml")
                }
                "/probe/drm" => {
                    Reply::raw("<MPD><ContentProtection/></MPD>", "application/dash+xml")
                }
                "/probe/html" => Reply::raw("<!DOCTYPE html><html><video/></html>", "video/mp4"),
                "/probe/large" => {
                    Reply::raw(vec![b'x'; 2 * 1024 * 1024], "application/octet-stream")
                }
                _ => Reply::raw("arbitrary data", "application/octet-stream"),
            };
        }
        let mut value: Value = serde_json::from_slice(&standard(request, port).body).unwrap();
        if let Some(resource) = request.query.get("resource") {
            value["list"][0]["vod_play_from"] = json!("A");
            value["list"][0]["vod_play_url"] = json!(if resource == "script" {
                "正片$javascript:secret()".into()
            } else {
                format!("正片$http://maccms.fixture.test:{port}/probe/{resource}")
            });
        }
        Reply::json(value)
    });
    let (_temp, host) = setup(fixture.url("/api"));
    for (path, kind, playable) in [
        ("hls", "hls", true),
        ("mp4", "progressive", true),
        ("dash", "dash", true),
        ("drm", "drm", false),
        ("html", "unknown", false),
        ("unknown", "unknown", false),
    ] {
        let (_temp, host) = setup(fixture.url(&format!("/api?resource={path}")));
        let catalog = catalog(&host);
        let content = catalog.media_ref(
            &serde_json::to_string(&["maccms", item(fixture.port)["vod_id"].as_str().unwrap()])
                .unwrap(),
        );
        let details = catalog
            .details(&Http::default(), &content, &job(&host, "default_metadata"))
            .unwrap();
        let link = catalog
            .resolve(
                &Http::default(),
                &details.value["details"]["groups"][0]["episodes"][0]["ref"],
                &job(&host, "resolve"),
            )
            .unwrap();
        assert_eq!(link.kind, kind);
        assert_eq!(link.playable, playable);
        assert_eq!(link.reason.is_none(), playable);
        assert_eq!(link.url, fixture.url(&format!("/probe/{path}")));
    }
    let fetched = Http::default()
        .get(
            fixture.url("/probe/large").parse().unwrap(),
            HeaderMap::new(),
            &job(&host, "resolve"),
            PROBE_LIMIT,
            true,
            false,
        )
        .unwrap();
    assert_eq!(fetched.body.len(), PROBE_LIMIT);
    for request in fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.target.starts_with("/probe/"))
    {
        assert_eq!(request.headers["range"], "bytes=0-65535");
    }
    // Unsupported protocols remain data, with a precise unplayable reason.
    let (_temp, script_host) = setup(fixture.url("/api?resource=script"));
    let catalog = catalog(&script_host);
    let reference = catalog.media_ref(
        &serde_json::to_string(&["maccms", item(fixture.port)["vod_id"].as_str().unwrap()])
            .unwrap(),
    );
    let mapped = catalog
        .details(
            &Http::default(),
            &reference,
            &job(&script_host, "default_metadata"),
        )
        .unwrap();
    assert!(!mapped.value.to_string().contains("javascript"));
    let link = catalog
        .resolve(
            &Http::default(),
            &mapped.value["details"]["groups"][0]["episodes"][0]["ref"],
            &job(&script_host, "resolve"),
        )
        .unwrap();
    assert_eq!(link.reason, Some("unsupported_protocol_or_script"));
    assert!(!link.playable);
}

#[test]
fn n00_public_addresses_dns_rebinding_and_peer() {
    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.1.2",
        "169.254.1.1",
        "0.0.0.0",
        "224.1.1.1",
        "240.1.1.1",
        "100.64.0.1",
        "198.18.0.1",
        "192.0.2.1",
        "::",
        "::1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
        "::ffff:127.0.0.1",
        "2001:db8::1",
        "2002:7f00:1::",
        "64:ff9b::7f00:1",
    ] {
        assert!(
            !public_ip(address.parse().unwrap()),
            "unsafe address: {address}"
        );
    }
    for address in [
        "8.8.8.8",
        "1.1.1.1",
        "2606:4700:4700::1111",
        "::ffff:8.8.8.8",
    ] {
        assert!(
            public_ip(address.parse().unwrap()),
            "public address: {address}"
        );
    }
    let http = Http::default();
    let url: Url = "http://public.example/api".parse().unwrap();
    let public: SocketAddr = "8.8.8.8:80".parse().unwrap();
    let private: SocketAddr = "[::ffff:127.0.0.1]:80".parse().unwrap();
    assert!(
        http.check_addresses(&url, &[public, private], false)
            .is_err()
    );
    assert!(http.check_addresses(&url, &[], false).is_err());
    assert!(http.check_peer(&url, &[public], Some(private)).is_err());
    assert!(http.check_peer(&url, &[public], None).is_err());
    let count = Arc::new(AtomicUsize::new(0));
    let dns_count = count.clone();
    let fixture = Fixture::new(|_, _| Reply::raw("safe", "text/plain"));
    let (_temp, host) = setup(fixture.url("/api"));
    let dns = DnsFixture::new(move |kind| {
        Some(if kind == 1 {
            let n = dns_count.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                vec![127, 0, 0, 1]
            } else {
                vec![127, 0, 0, 2]
            }
        } else {
            vec![]
        })
    });
    let http = Http {
        nameserver: Some(vec![dns.address]),
    };
    let fetch = || {
        http.get(
            fixture.url("/dns").parse().unwrap(),
            HeaderMap::new(),
            &job(&host, "default_metadata"),
            100,
            false,
            false,
        )
    };
    assert_eq!(fetch().unwrap().body, b"safe");
    assert_eq!(fetch().err().unwrap().code, "UPSTREAM_FAILED");
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(dns.queries.lock().unwrap().contains(&1));
    assert!(dns.queries.lock().unwrap().contains(&28));
    let mixed_dns = DnsFixture::new(|kind| {
        Some(if kind == 1 {
            vec![8, 8, 8, 8]
        } else {
            "fc00::1"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets()
                .to_vec()
        })
    });
    let mixed = Http {
        nameserver: Some(vec![mixed_dns.address]),
    };
    assert_eq!(
        mixed
            .get(
                fixture.url("/mixed").parse().unwrap(),
                HeaderMap::new(),
                &job(&host, "default_metadata"),
                100,
                false,
                false
            )
            .err()
            .unwrap()
            .code,
        "UPSTREAM_FAILED"
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    for target in [
        "file:///etc/passwd",
        "http://user:password@public.example/api",
        "javascript:foo()",
    ] {
        assert_eq!(http_url(target, true).unwrap_err().code, "BAD_REQUEST");
    }
}

#[test]
fn n00_cancellation_interrupts_dns_lookup() {
    let dns = DnsFixture::new(|_| None);
    let (_temp, host) = setup("http://maccms.fixture.test:12345/api".into());
    let request_id = id();
    let work = host
        .requests
        .begin(&request_id, "default_metadata", host.clock.clone())
        .unwrap();
    let http = Http {
        nameserver: Some(vec![dns.address]),
    };
    let worker = thread::spawn(move || {
        http.get(
            "http://maccms.fixture.test:12345/api".parse().unwrap(),
            HeaderMap::new(),
            &work,
            100,
            false,
            false,
        )
    });
    dns.wait_for_query();
    let start = Instant::now();
    host.requests.cancel(&id(), &request_id).unwrap();
    assert_eq!(worker.join().unwrap().err().unwrap().code, "CANCELLED");
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "DNS outlived cancellation"
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn n00_redirect_origin_headers_proxy_and_limits() {
    let fixture = Fixture::new(|request, port| {
        if request.target.starts_with("/cross") {
            Reply::redirect(format!("http://other.fixture.test:{port}/final"))
        } else if request.target.starts_with("/same") {
            Reply::redirect("/final".into())
        } else if request.target.starts_with("/private") {
            Reply::redirect("http://127.0.0.1:1/private".into())
        } else if request.target.starts_with("/loop") {
            Reply::redirect("/loop".into())
        } else {
            Reply::raw("done", "text/plain")
        }
    });
    let (_temp, host) = setup(fixture.url("/api"));
    let mut headers = HeaderMap::new();
    for (key, value) in [
        ("authorization", "Bearer fixture"),
        ("cookie", "fixture=1"),
        ("x-api-key", "private"),
        ("referer", "https://secret.example/path"),
        ("accept", "application/json"),
    ] {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    let http = Http::default();
    let cross = http
        .get(
            fixture.url("/cross").parse().unwrap(),
            headers.clone(),
            &job(&host, "default_metadata"),
            100,
            false,
            false,
        )
        .unwrap();
    assert_eq!(cross.url.host_str(), Some("other.fixture.test"));
    assert_eq!(cross.body, b"done");
    http.get(
        fixture.url("/same").parse().unwrap(),
        headers.clone(),
        &job(&host, "default_metadata"),
        100,
        false,
        false,
    )
    .unwrap();
    let requests = fixture.requests.lock().unwrap();
    let finals: Vec<_> = requests.iter().filter(|r| r.target == "/final").collect();
    assert!(
        !finals[0].headers.contains_key("authorization")
            && !finals[0].headers.contains_key("cookie")
            && !finals[0].headers.contains_key("x-api-key")
            && !finals[0].headers.contains_key("referer")
    );
    assert_eq!(finals[0].headers["accept"], "application/json");
    assert_eq!(finals[1].headers["authorization"], "Bearer fixture");
    drop(requests);
    assert_eq!(
        http.get(
            fixture.url("/private").parse().unwrap(),
            HeaderMap::new(),
            &job(&host, "default_metadata"),
            100,
            false,
            false
        )
        .err()
        .unwrap()
        .code,
        "UPSTREAM_FAILED"
    );
    assert_eq!(
        http.get(
            fixture.url("/loop").parse().unwrap(),
            HeaderMap::new(),
            &job(&host, "default_metadata"),
            100,
            false,
            false
        )
        .err()
        .unwrap()
        .code,
        "UPSTREAM_FAILED"
    );
    assert_eq!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.target == "/loop")
            .count(),
        6
    );
    assert!(
        redirect_headers(
            &"https://public.example/api".parse().unwrap(),
            &"http://public.example/api".parse().unwrap(),
            &mut headers
        )
        .is_err()
    );
    // n00.py starts this process with all standard proxy variables pointed at a
    // rejecting local endpoint. The real positive requests above prove no_proxy.
    if std::env::var("N00_PROXY_TEST").as_deref() == Ok("1") {
        for name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            assert_eq!(std::env::var(name).unwrap(), "http://127.0.0.1:1");
        }
        println!("N00_PROXY_ENV:true");
    }
}

#[test]
fn n00_partial_home_retry_pagination_and_all_failed() {
    let fail_all = Arc::new(AtomicBool::new(false));
    let failure = fail_all.clone();
    let fixture = Fixture::new(move |request, port| {
        let mut value = response(
            json!([item(port)]),
            json!([
            {"type_id": "a", "type_name": "A"}, {"type_id": "b", "type_name": "B"}, {"type_id": "c", "type_name": "C"}]),
        );
        if let Some(t) = request.query.get("t") {
            if t == "b" || failure.load(Ordering::SeqCst) {
                return Reply::json(json!({"code": 0, "list": []}));
            }
            if request.query.get("pg").map(String::as_str) == Some("2") {
                value["page"] = json!("2");
                value["list"] = json!([]);
            }
        }
        Reply::json(value)
    });
    let (_temp, host) = setup(fixture.url("/api"));
    let source = sync(&host);
    let home = ok(&host, "get_home", home_input(&source));
    assert_eq!(home["page"]["sections"][0]["index"], 0);
    assert_eq!(home["page"]["sections"][1]["index"], 2);
    assert_eq!(home["page"]["failed_sections"][0]["index"], 1);
    assert_eq!(
        home["page"]["failed_sections"][0]["error"]["code"],
        "UPSTREAM_FAILED"
    );
    let mut input = home_input(&source);
    input["section_ref"] = home["page"]["failed_sections"][0]["section_ref"].clone();
    error(&call(&host, "get_home", input.clone()), "UPSTREAM_FAILED");
    input["section_ref"] = home["page"]["sections"][1]["section_ref"].clone();
    input["page"] = json!(2);
    let page = ok(&host, "get_home", input);
    assert_eq!(page["page"]["sections"][0]["index"], 2);
    assert_eq!(page["page"]["sections"][0]["items"], json!([]));
    assert_eq!(page["page"]["sections"][0]["has_next"], false);
    assert!(page["page"]["sections"][0]["next_page"].is_null());
    fail_all.store(true, Ordering::SeqCst);
    error(
        &call(&host, "get_home", home_input(&source)),
        "UPSTREAM_FAILED",
    );
}

#[test]
fn n00_config_edits_disable_delete_and_readd_cancel_io() {
    for change in ["disable", "delete", "api", "rename", "delete-readd"] {
        let fixture = Fixture::new(|request, port| {
            let mut reply = standard(request, port);
            if request.query.contains_key("ids") {
                reply.body_delay = Duration::from_secs(5);
            }
            reply
        });
        let (_temp, host) = setup(fixture.url("/api"));
        let source = sync(&host);
        let home = ok(&host, "get_home", home_input(&source));
        let reference = home["page"]["sections"][0]["items"][0]["ref"].clone();
        let work_host = host.clone();
        let worker =
            thread::spawn(move || call(&work_host, "get_details", json!({"ref": reference})));
        fixture.wait_for(|r| r.query.contains_key("ids"));
        let mut settings = ok(&host, "get_settings", json!({}));
        let old = settings.clone();
        match change {
            "disable" => settings["source_configs"][0]["enabled"] = json!(false),
            "delete" | "delete-readd" => settings["source_configs"] = json!([]),
            "api" => settings["source_configs"][0]["api_url"] = json!(fixture.url("/edited")),
            "rename" => settings["source_configs"][0]["name"] = json!("renamed"),
            _ => unreachable!(),
        }
        let start = Instant::now();
        ok(&host, "save_settings", settings);
        if change == "delete-readd" {
            ok(&host, "save_settings", old);
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "settings waited for upstream I/O"
        );
        error(&worker.join().unwrap(), "CANCELLED");
        assert!(host.committed.lock().unwrap().maccms.is_empty());
        let after = ok(&host, "get_sources", json!({}));
        if change == "api" {
            assert_eq!(after["providers"][0]["id"], source["id"]);
            assert_ne!(
                after["providers"][0]["source_revision"],
                source["source_revision"]
            );
            error(
                &call(&host, "get_home", home_input(&source)),
                "REVISION_MISMATCH",
            );
        }
        assert_eq!(host.requests.active_count(), 0);
    }
}

#[test]
fn n00_cancel_pending_sync_and_keep_control_responsive() {
    let fixture = Fixture::new(|request, port| {
        let mut reply = standard(request, port);
        reply.header_delay = Duration::from_secs(5);
        reply
    });
    let (_temp, host) = setup(fixture.url("/api"));
    let request_id = id();
    let args = json!({"request_id": request_id, "input": {"kind": "maccms", "config_id": CONFIG}});
    let queued = host.admit("sync_catalog", args, true).ok().unwrap();
    let work_host = host.clone();
    let worker = thread::spawn(move || work_host.execute(queued));
    fixture.wait_for(|r| r.query.get("ac").map(String::as_str) == Some("list"));
    let start = Instant::now();
    assert_eq!(
        ok(
            &host,
            "cancel_request",
            json!({"target_request_id": request_id})
        )["cancelled"],
        true
    );
    error(&worker.join().unwrap(), "CANCELLED");
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(host.committed.lock().unwrap().maccms.is_empty());
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn n00_latest_sync_wins_without_partial_publication() {
    let counter = Arc::new(AtomicUsize::new(0));
    let server_counter = counter.clone();
    let fixture = Fixture::new(move |request, port| {
        let n = server_counter.fetch_add(1, Ordering::SeqCst);
        let mut value: Value = serde_json::from_slice(&standard(request, port).body).unwrap();
        value["class"][0]["type_name"] = json!(if n == 0 { "old" } else { "new" });
        let mut reply = Reply::json(value);
        if n == 0 {
            reply.body_delay = Duration::from_millis(300);
        }
        reply
    });
    let (_temp, host) = setup(fixture.url("/api"));
    let work_host = host.clone();
    let old = thread::spawn(move || {
        call(
            &work_host,
            "sync_catalog",
            json!({"kind": "maccms", "config_id": CONFIG}),
        )
    });
    fixture.wait_for(|_| true);
    let start = Instant::now();
    while counter.load(Ordering::SeqCst) == 0 {
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(2));
    }
    let source = sync(&host);
    error(&old.join().unwrap(), "CANCELLED");
    assert_eq!(
        host.committed.lock().unwrap().maccms[source["id"].as_str().unwrap()].sections[0].title,
        "new"
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
fn n00_deadline_queue_and_live_body_timeout() {
    let fixture = Fixture::new(|request, port| {
        let mut reply = standard(request, port);
        reply.body_delay = Duration::from_secs(5);
        reply
    });
    let temp = TempDir::new().unwrap();
    let clock = Arc::new(TestClock::default());
    let host = Host::with_clock(temp.path().join("settings.json"), clock.clone()).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([{"kind": "maccms", "config_id": CONFIG, "name": "fixture", "api_url": fixture.url("/api"), "enabled": true}]);
    ok(&host, "save_settings", settings);
    let queued = host
        .admit(
            "sync_catalog",
            json!({"request_id": id(), "input": {"kind": "maccms", "config_id": CONFIG}}),
            true,
        )
        .ok()
        .unwrap();
    clock.0.store(45000, Ordering::SeqCst);
    error(&host.execute(queued), "UPSTREAM_TIMEOUT");
    assert!(fixture.requests.lock().unwrap().is_empty());
    let work_host = host.clone();
    let worker = thread::spawn(move || {
        call(
            &work_host,
            "sync_catalog",
            json!({"kind": "maccms", "config_id": CONFIG}),
        )
    });
    fixture.wait_for(|_| true);
    clock.0.store(90000, Ordering::SeqCst);
    error(&worker.join().unwrap(), "UPSTREAM_TIMEOUT");
    assert!(host.committed.lock().unwrap().maccms.is_empty());
}

#[test]
fn n00_metadata_limits_and_foreign_section_refs() {
    for chunked in [false, true] {
        let fixture = Fixture::new(move |_, _| {
            let mut reply = Reply::raw(
                if chunked {
                    vec![b' '; RESPONSE_LIMIT + 1]
                } else {
                    vec![]
                },
                "application/json",
            );
            reply.length = Some(RESPONSE_LIMIT + 1);
            reply.chunked = chunked;
            reply
        });
        let (_temp, host) = setup(fixture.url("/api"));
        error(
            &call(
                &host,
                "sync_catalog",
                json!({"kind": "maccms", "config_id": CONFIG}),
            ),
            "RESOURCE_LIMIT",
        );
    }
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(fixture.url("/api"));
    let source = sync(&host);
    let mut input = home_input(&source);
    input["section_ref"] = json!("unregistered-or-foreign-handle");
    error(&call(&host, "get_home", input), "BAD_REQUEST");
    let mut input = home_input(&source);
    input["page"] = json!(2);
    error(&call(&host, "get_home", input), "BAD_REQUEST");
    let mut headers = HeaderMap::new();
    headers.insert("x-long", "a".repeat(1024 * 1024).parse().unwrap());
    assert_eq!(
        Http::default()
            .get(
                fixture.url("/api").parse().unwrap(),
                headers,
                &job(&host, "default_metadata"),
                100,
                false,
                true
            )
            .err()
            .unwrap()
            .code,
        "RESOURCE_LIMIT"
    );
    let (_temp, private) = setup("http://127.0.0.1:1/private?secret=fixture".into());
    let result = call(
        &private,
        "sync_catalog",
        json!({"kind": "maccms", "config_id": CONFIG}),
    );
    error(&result, "BAD_REQUEST");
    assert_eq!(result["error"]["details"]["reason"], "unsafe_target");
    assert!(!result["error"].to_string().contains("secret"));
}

#[test]
fn n00_global_metadata_slot_bound() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (current, maximum) = (active.clone(), peak.clone());
    let fixture = Fixture::new(move |_, _| {
        let n = current.fetch_add(1, Ordering::SeqCst) + 1;
        maximum.fetch_max(n, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(30));
        current.fetch_sub(1, Ordering::SeqCst);
        Reply::raw("ok", "text/plain")
    });
    let (_temp, host) = setup(fixture.url("/api"));
    thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..9 {
            let host = &host;
            let fixture = &fixture;
            workers.push(scope.spawn(move || {
                let value = Http::default()
                    .get(
                        fixture.url("/slots").parse().unwrap(),
                        HeaderMap::new(),
                        &job(host, "default_metadata"),
                        100,
                        false,
                        false,
                    )
                    .unwrap();
                assert_eq!(value.body, b"ok");
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
    assert!(peak.load(Ordering::SeqCst) <= 4);
    assert_eq!(fixture.requests.lock().unwrap().len(), 9);
}
