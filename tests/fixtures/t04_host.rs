//! T0.21: request context, fault isolation and cleanup through the production Host.
use super::*;

const SECOND: &str = "33333333-3333-4333-8333-333333333333";

/// `/failing` follows the shared mode; `/ok`, Stremio paths and media always answer.
fn isolated(mode: Arc<Mutex<&'static str>>) -> Fixture {
    Fixture::new(move |request, port| {
        if let Some(reply) = metadata_reply(request) {
            return reply;
        }
        if request.target.starts_with("/media/") {
            return match *mode.lock().unwrap() {
                "media-403" => Reply {
                    status: 403,
                    ..Reply::raw("refused", "text/plain")
                },
                _ => Reply::raw(vec![0x5a; 4096], "video/mp4"),
            };
        }
        let healthy = |port| {
            let mut raw = item(port);
            raw["vod_name"] = json!("Same title");
            raw["vod_year"] = Value::Null;
            raw["vod_play_from"] = json!("线路");
            raw["vod_play_url"] = json!(format!(
                "Special$http://maccms.fixture.test:{port}/media/a.mp4#Second$http://maccms.fixture.test:{port}/media/b.mp4"
            ));
            Reply::json(response(
                json!([raw]),
                json!([{"type_id":"1","type_name":"甲"},{"type_id":"2","type_name":"乙"},{"type_id":"3","type_name":"丙"},
                    {"type_id":"4","type_name":"丁"},{"type_id":"5","type_name":"戊"},{"type_id":"6","type_name":"己"}]),
            ))
        };
        if !request.target.starts_with("/failing") {
            return healthy(port);
        }
        match *mode.lock().unwrap() {
            "500" => Reply {
                status: 500,
                ..Reply::raw("failed", "text/plain")
            },
            "closed" => Reply::json(json!({"code":0,"list":[]})),
            "html" => Reply::raw("<html>maintenance</html>", "text/html"),
            "challenge" => Reply::raw("<title>Just a moment...</title>", "text/html"),
            "cf-503" => Reply {
                status: 503,
                headers: vec![("cf-mitigated".into(), "challenge".into())],
                ..Reply::raw("", "text/html")
            },
            "slow" => Reply {
                body_delay: Duration::from_secs(5),
                ..healthy(port)
            },
            refused @ ("401" | "403" | "407" | "429" | "451") => Reply {
                status: refused.parse().unwrap(),
                ..Reply::raw("refused", "text/plain")
            },
            _ => healthy(port),
        }
    })
}

struct Sources {
    _temp: TempDir,
    host: Arc<Host>,
    failing: Value,
    healthy: Value,
    metadata: Value,
}

fn three_sources(fixture: &Fixture) -> Sources {
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([
        {"kind":"maccms","config_id":CONFIG,"name":"来源甲","enabled":true,"api_url":fixture.url("/failing/api")},
        {"kind":"maccms","config_id":SECOND,"name":"来源乙","enabled":true,"api_url":fixture.url("/ok/api")},
        {"kind":"stremio","config_id":METADATA_CONFIG,"name":"Metadata fixture","enabled":true,"manifest_urls":[fixture.url("/tmdb/manifest.json")]}]);
    ok(&host, "save_settings", settings);
    let synced = |kind: &str, id: &str| {
        ok(&host, "sync_catalog", json!({"kind":kind,"config_id":id}))["sources"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|provider| provider["origin"]["config_id"] == id)
            .unwrap()
            .clone()
    };
    let (failing, healthy, metadata) = (
        synced("maccms", CONFIG),
        synced("maccms", SECOND),
        synced("stremio", METADATA_CONFIG),
    );
    Sources {
        _temp: temp,
        host,
        failing,
        healthy,
        metadata,
    }
}

fn others_work(sources: &Sources) {
    let home = ok(&sources.host, "get_home", home_input(&sources.healthy))["page"].clone();
    assert_eq!(home["failed_sections"], json!([]));
    let reference = home["sections"][0]["items"][0]["ref"].clone();
    ok(&sources.host, "get_details", json!({"ref":reference}));
    let metadata = ok(&sources.host, "get_home", home_input(&sources.metadata))["page"].clone();
    assert!(!metadata["sections"].as_array().unwrap().is_empty());
}

fn path_requests(fixture: &Fixture, prefix: &str) -> usize {
    fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.target.starts_with(prefix))
        .count()
}

fn media_get(url: &str) -> (u16, String) {
    let (status, body) = resource(url);
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[test]
fn t04_host_and_playback_work_without_android_tools() {
    if std::env::var_os("T04_NO_ANDROID").is_some() {
        assert!(std::env::var_os("ANDROID_HOME").is_none());
        for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap()) {
            for tool in ["adb", "emulator", "avdmanager", "sdkmanager"] {
                assert!(!directory.join(tool).exists(), "{tool} is on PATH");
            }
        }
        println!("T04_NO_ANDROID:{}", json!({"tools_on_path":0}));
    }
    let fixture = isolated(Arc::new(Mutex::new("ok")));
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    let (status, body) = resource(view["media_url"].as_str().unwrap());
    assert_eq!((status, body.len()), (200, 4096));
    let runtime = ok(&host, "runtime_status", json!({}));
    assert_eq!(runtime["phase"], "ready", "{runtime}");
    assert_eq!(runtime["cloudstream"]["phase"], "stopped", "{runtime}");
    ok(
        &host,
        "close_playback",
        json!({"session_id":view["session_id"]}),
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t04_one_failing_source_keeps_other_sources_and_stremio_working() {
    let mode = Arc::new(Mutex::new("ok"));
    let fixture = isolated(mode.clone());
    let sources = three_sources(&fixture);
    for (failure, code) in [
        ("500", "UPSTREAM_FAILED"),
        ("closed", "UPSTREAM_FAILED"),
        ("html", "UPSTREAM_FAILED"),
        ("403", "UPSTREAM_FAILED"),
        ("challenge", "UPSTREAM_FAILED"),
    ] {
        *mode.lock().unwrap() = failure;
        error(
            &call(&sources.host, "get_home", home_input(&sources.failing)),
            code,
        );
        others_work(&sources);
    }
    // While one source hangs, the others answer and cancelling it leaves them untouched.
    *mode.lock().unwrap() = "slow";
    let request_id = id();
    let args = json!({"request_id":request_id,"input":home_input(&sources.failing)});
    let host = sources.host.clone();
    let worker = thread::spawn(move || host.invoke("get_home", args, true));
    fixture.wait_for(|request| request.target.starts_with("/failing"));
    let started = Instant::now();
    others_work(&sources);
    assert!(started.elapsed() < Duration::from_secs(2));
    ok(
        &sources.host,
        "cancel_request",
        json!({"target_request_id":request_id}),
    );
    error(&worker.join().unwrap(), "CANCELLED");
    others_work(&sources);
    assert_eq!(sources.host.requests.active_count(), 0);
}

#[test]
fn t04_access_refusal_is_reported_and_stops_the_source() {
    let mode = Arc::new(Mutex::new("ok"));
    let fixture = isolated(mode.clone());
    let sources = three_sources(&fixture);
    let reference = json!({"source_id":sources.failing["id"],"source_revision":sources.failing["source_revision"],
        "data":serde_json::to_string(&["maccms", "1"]).unwrap()});
    let home = ok(&sources.host, "get_home", home_input(&sources.failing))["page"].clone();
    let section = home["sections"][1]["section_ref"].clone();
    for (refusal, status, signal) in [
        ("401", 401, "status"),
        ("403", 403, "status"),
        ("407", 407, "status"),
        ("429", 429, "status"),
        ("451", 451, "status"),
        ("cf-503", 503, "challenge"),
        ("challenge", 200, "challenge"),
    ] {
        *mode.lock().unwrap() = refusal;
        let expect = |result: &Value| {
            error(result, "UPSTREAM_FAILED");
            let details = &result["error"]["details"];
            assert_eq!(details["reason"], "access_control", "{result}");
            assert_eq!(details["http_status"], status, "{result}");
            assert_eq!(details["signal"], signal, "{result}");
            assert!(!result.to_string().contains("fixture.test"), "{result}");
        };
        let before = path_requests(&fixture, "/failing");
        expect(&call(
            &sources.host,
            "get_details",
            json!({"ref":reference}),
        ));
        assert_eq!(path_requests(&fixture, "/failing"), before + 1, "no retry");
        let before = path_requests(&fixture, "/failing");
        let mut input = home_input(&sources.failing);
        input["section_ref"] = section.clone();
        input["page"] = json!(2);
        expect(&call(&sources.host, "get_home", input));
        assert_eq!(path_requests(&fixture, "/failing"), before + 1, "no retry");
    }
    // Sections refused while the home is listed: later chunks are not requested.
    *mode.lock().unwrap() = "ok";
    let refuse_sections = Arc::new(AtomicBool::new(false));
    let refused = refuse_sections.clone();
    let sections = Fixture::new(move |request, port| {
        if request.query.contains_key("t") && refused.load(Ordering::SeqCst) {
            return Reply {
                status: 403,
                ..Reply::raw("refused", "text/plain")
            };
        }
        let classes: Vec<_> = (1..=9)
            .map(|n| json!({"type_id":n,"type_name":format!("栏目{n}")}))
            .collect();
        Reply::json(response(json!([item(port)]), json!(classes)))
    });
    let (_temp, host) = setup(sections.url("/api"));
    let provider = sync(&host);
    refuse_sections.store(true, Ordering::SeqCst);
    let before = sections
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.query.contains_key("t"))
        .count();
    let result = call(&host, "get_home", home_input(&provider));
    error(&result, "UPSTREAM_FAILED");
    assert_eq!(result["error"]["details"]["reason"], "access_control");
    let after = sections
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.query.contains_key("t"))
        .count();
    assert!(
        after - before <= 4,
        "requests continued after refusal: {}",
        after - before
    );
    // Other failures keep their own meaning.
    for (failure, reason) in [("500", Value::Null), ("html", Value::Null)] {
        *mode.lock().unwrap() = failure;
        let result = call(&sources.host, "get_details", json!({"ref":reference}));
        assert_eq!(result["error"]["details"]["reason"], reason, "{result}");
    }
}

#[test]
fn t04_refused_media_fails_once_and_never_reaches_a_local_secret_upstream() {
    let mode = Arc::new(Mutex::new("ok"));
    let fixture = isolated(mode.clone());
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    let media = view["media_url"].as_str().unwrap();
    assert_eq!(resource(media).0, 200);
    let capability = url::Url::parse(media).unwrap().query().unwrap().to_owned();
    let secret = capability.split('=').next_back().unwrap().to_owned();
    assert!(secret.len() >= 16);
    {
        let requests = fixture.requests.lock().unwrap();
        for request in requests.iter() {
            assert!(!request.target.contains(&secret), "{}", request.target);
            for (name, value) in &request.headers {
                assert!(!value.contains(&secret), "{name}");
                assert!(
                    !matches!(name.as_str(), "authorization" | "cookie"),
                    "{name}"
                );
            }
        }
    }
    *mode.lock().unwrap() = "media-403";
    let before = path_requests(&fixture, "/media/");
    let (status, body) = media_get(media);
    assert_eq!(status, 502, "{body}");
    assert!(body.contains("refused access"), "{body}");
    assert_eq!(path_requests(&fixture, "/media/"), before + 1, "no retry");
    ok(
        &host,
        "close_playback",
        json!({"session_id":view["session_id"]}),
    );
    assert_eq!(host.requests.active_count(), 0);
}

struct TlsServer {
    child: std::process::Child,
    port: u16,
    _folder: TempDir,
}

impl Drop for TlsServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// An untrusted self-signed server for the fixture name, run by the system's openssl.
fn tls_server() -> TlsServer {
    use std::process::{Command, Stdio};
    let folder = TempDir::new().unwrap();
    let (key, cert) = (
        folder.path().join("key.pem"),
        folder.path().join("cert.pem"),
    );
    let created = Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
        ])
        .args([
            "-subj",
            "/CN=maccms.fixture.test",
            "-addext",
            "subjectAltName=DNS:maccms.fixture.test",
        ])
        .arg("-keyout")
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(created.success());
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let child = Command::new("openssl")
        .args(["s_server", "-www", "-accept", &port.to_string()])
        .arg("-key")
        .arg(&key)
        .arg("-cert")
        .arg(&cert)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(20));
    }
    TlsServer {
        child,
        port,
        _folder: folder,
    }
}

#[test]
fn t04_certificate_errors_stay_failures_and_are_named() {
    let server = tls_server();
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([{"kind":"maccms","config_id":CONFIG,"name":"未受信任","enabled":true,
        "api_url":format!("https://maccms.fixture.test:{}/api", server.port)}]);
    ok(&host, "save_settings", settings);
    let result = call(
        &host,
        "sync_catalog",
        json!({"kind":"maccms","config_id":CONFIG}),
    );
    error(&result, "UPSTREAM_FAILED");
    assert_eq!(
        result["error"]["details"]["reason"], "tls_certificate",
        "{result}"
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t04_stale_work_and_exit_release_only_this_host() {
    let delay = Arc::new(AtomicBool::new(false));
    let fixture = gateway_fixture(delay.clone());
    let foreign_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let foreign = foreign_listener.local_addr().unwrap();
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    let media = view["media_url"].as_str().unwrap().to_owned();
    let parsed = url::Url::parse(&media).unwrap();
    delay.store(true, Ordering::SeqCst);
    let mut client = TcpStream::connect(("127.0.0.1", parsed.port().unwrap())).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
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
    // Exit: the stream and gateway stop, a foreign listener and the upstream stay up.
    let requests = host.requests.clone();
    drop(host);
    let start = Instant::now();
    let mut byte = [0; 1024];
    loop {
        match client.read(&mut byte) {
            Ok(0) | Err(_) => break,
            Ok(_) => assert!(start.elapsed() < Duration::from_secs(4)),
        }
    }
    assert!(start.elapsed() < Duration::from_secs(4));
    assert_eq!(requests.active_count(), 0);
    assert!(
        TcpStream::connect(("127.0.0.1", parsed.port().unwrap())).is_err(),
        "the gateway must stop with its Host"
    );
    assert!(
        TcpStream::connect(foreign).is_ok(),
        "foreign listener stays"
    );
    delay.store(false, Ordering::SeqCst);
    assert!(TcpStream::connect(("127.0.0.1", fixture.port)).is_ok());
}
