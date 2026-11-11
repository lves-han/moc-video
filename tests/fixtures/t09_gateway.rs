//! Real sockets through the production gateway, stores and public HTTP boundary.
//! Source callback contexts are synthetic; original Android providers remain N02.
#[path = "t10_hls.rs"]
mod t10;
#[path = "t11_subtitles.rs"]
mod t11;
#[path = "t22_gateway.rs"]
mod t22;
#[path = "t24_cache_gateway.rs"]
mod t24_cache;
use super::*;
use crate::resolution::{Link, ResolutionStore, Subtitle};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader},
    net::{IpAddr, Ipv4Addr, UdpSocket},
    sync::atomic::AtomicUsize,
    time::Instant,
};

#[derive(Default)]
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
#[derive(Clone)]
struct RawRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
}
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    repeat: usize,
    length: Option<usize>,
    chunked: bool,
    header_delay: Duration,
    body_delay: Duration,
    gate: Option<Arc<AtomicBool>>,
}
impl Reply {
    fn body(body: impl Into<Vec<u8>>, mime: &str) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".into(), mime.into())],
            body: body.into(),
            repeat: 1,
            length: None,
            chunked: false,
            header_delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            gate: None,
        }
    }
    fn redirect(to: String) -> Self {
        Self {
            status: 302,
            headers: vec![("Location".into(), to)],
            ..Self::body("", "text/plain")
        }
    }
    fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}
#[derive(Default)]
struct FixtureState {
    stop: AtomicBool,
    active: AtomicUsize,
    produced: AtomicU64,
    requests: Mutex<Vec<RawRequest>>,
}
struct Fixture {
    port: u16,
    state: Arc<FixtureState>,
    worker: Option<JoinHandle<()>>,
}
struct Active(Arc<FixtureState>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}
fn wait_for(mut predicate: impl FnMut() -> bool) {
    let start = Instant::now();
    while !predicate() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "owned fixture did not reach expected state"
        );
        thread::sleep(Duration::from_millis(2));
    }
}
impl Fixture {
    fn new(handler: impl Fn(&RawRequest, u16) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(FixtureState::default());
        let running = state.clone();
        let handler = Arc::new(handler);
        let worker = thread::spawn(move || {
            let mut calls = Vec::new();
            while !running.stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let state = running.clone();
                        let handler = handler.clone();
                        calls.push(thread::spawn(move || {
                            fixture_serve(stream, port, state, &*handler)
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                }
            }
            for call in calls {
                call.join().unwrap();
            }
        });
        Self {
            port,
            state,
            worker: Some(worker),
        }
    }
    fn url(&self, path: &str) -> String {
        format!("http://maccms.fixture.test:{}{path}", self.port)
    }
    fn quiet(&self) {
        wait_for(|| self.state.active.load(Ordering::SeqCst) == 0);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn fixture_wait(
    stream: &TcpStream,
    state: &FixtureState,
    duration: Duration,
    gate: Option<&AtomicBool>,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < duration || gate.is_some_and(|g| !g.load(Ordering::SeqCst)) {
        if state.stop.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(10) {
            return false;
        }
        let mut byte = [0];
        if matches!(stream.peek(&mut byte), Ok(0)) {
            return false;
        }
        thread::sleep(Duration::from_millis(2));
    }
    true
}
fn fixture_serve(
    mut stream: TcpStream,
    port: u16,
    state: Arc<FixtureState>,
    handler: &dyn Fn(&RawRequest, u16) -> Reply,
) {
    state.active.fetch_add(1, Ordering::SeqCst);
    let _active = Active(state.clone());
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3 {
        return;
    }
    let mut req = RawRequest {
        method: parts[0].into(),
        target: parts[1].into(),
        headers: HashMap::new(),
    };
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        if line == "\r\n" {
            break;
        }
        let Some((k, v)) = line.split_once(':') else {
            return;
        };
        req.headers.insert(k.to_ascii_lowercase(), v.trim().into());
    }
    state.requests.lock().unwrap().push(req.clone());
    let reply = handler(&req, port);
    if !fixture_wait(&stream, &state, reply.header_delay, None) {
        return;
    }
    let mut head = format!("HTTP/1.1 {} Fixture\r\nConnection: close\r\n", reply.status);
    for (k, v) in &reply.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if reply.chunked {
        head.push_str("Transfer-Encoding: chunked\r\n\r\n");
    } else {
        head.push_str(&format!(
            "Content-Length: {}\r\n\r\n",
            reply.length.unwrap_or(reply.body.len() * reply.repeat)
        ));
    }
    if stream.write_all(head.as_bytes()).is_err() || req.method == "HEAD" {
        return;
    }
    if !fixture_wait(&stream, &state, reply.body_delay, None) {
        return;
    }
    for index in 0..reply.repeat {
        if index == 1 && !fixture_wait(&stream, &state, Duration::ZERO, reply.gate.as_deref()) {
            return;
        }
        for chunk in reply.body.chunks(8192) {
            if state.stop.load(Ordering::SeqCst) {
                return;
            }
            if reply.chunked && write!(stream, "{:x}\r\n", chunk.len()).is_err() {
                return;
            }
            if stream.write_all(chunk).is_err() {
                return;
            }
            state
                .produced
                .fetch_add(chunk.len() as u64, Ordering::SeqCst);
            if reply.chunked && stream.write_all(b"\r\n").is_err() {
                return;
            }
        }
    }
    if reply.chunked {
        let _ = stream.write_all(b"0\r\n\r\n");
    }
}
struct Response {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
fn connect(url: &str, method: &str, headers: &[(&str, &str)]) -> BufReader<TcpStream> {
    let url = url::Url::parse(url).unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let target = format!(
        "{}{}",
        url.path(),
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    write!(
        stream,
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n"
    )
    .unwrap();
    for (k, v) in headers {
        write!(stream, "{k}: {v}\r\n").unwrap();
    }
    stream.write_all(b"\r\n").unwrap();
    BufReader::new(stream)
}
fn read_head(reader: &mut BufReader<TcpStream>) -> Response {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let status = line
        .split_whitespace()
        .nth(1)
        .expect("HTTP status")
        .parse()
        .unwrap();
    let mut headers = HashMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        assert!(!line.is_empty(), "truncated HTTP headers");
        let (k, v) = line.split_once(':').unwrap();
        headers.insert(k.to_ascii_lowercase(), v.trim().to_owned());
    }
    Response {
        status,
        headers,
        body: vec![],
    }
}
fn fetch(url: &str, method: &str, headers: &[(&str, &str)]) -> Response {
    let mut reader = connect(url, method, headers);
    let mut result = read_head(&mut reader);
    if method != "HEAD" {
        if result.headers.get("transfer-encoding").map(String::as_str) == Some("chunked") {
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let n = usize::from_str_radix(line.trim(), 16).unwrap();
                if n == 0 {
                    break;
                }
                let start = result.body.len();
                result.body.resize(start + n, 0);
                reader.read_exact(&mut result.body[start..]).unwrap();
                let mut crlf = [0; 2];
                reader.read_exact(&mut crlf).unwrap();
                assert_eq!(&crlf, b"\r\n");
            }
        } else {
            reader.read_to_end(&mut result.body).unwrap();
        }
    }
    result
}
fn reference() -> Value {
    json!({"source_id":"native-fixture","source_revision":"rev1","data":"原episode/%2F/$"})
}
fn link(url: String) -> Link {
    Link {
        original: json!({"url":url,"name":"original"}),
        provider: "native-fixture".into(),
        url,
        headers: HeaderMap::new(),
        referer: String::new(),
        extractor_data: None,
        interceptor: None,
        cookies: None,
        label: "original link".into(),
        quality: None,
        quality_label: None,
        kind: "progressive".into(),
        mime_type: Some("video/mp4".into()),
        playable: true,
        reason: None,
    }
}
fn register(store: &mut ResolutionStore, link: Link, subtitles: Vec<Subtitle>, now: u64) -> Value {
    store
        .register(&reference(), vec![link], subtitles, now)
        .unwrap()
}
fn open(gateway: &Gateway, store: &mut ResolutionStore, resolution: &Value) -> Value {
    gateway
        .open(
            store
                .selected(
                    resolution["id"].as_str().unwrap(),
                    resolution["streams"][0]["id"].as_str().unwrap(),
                    gateway.shared.clock.now_ms(),
                )
                .unwrap(),
        )
        .unwrap()
}
fn session(gateway: &Gateway, url: String) -> Value {
    let mut store = ResolutionStore::default();
    let r = register(&mut store, link(url), vec![], gateway.shared.clock.now_ms());
    open(gateway, &mut store, &r)
}
fn media(view: &Value) -> &str {
    view["media_url"].as_str().unwrap()
}
fn cap(url: &str) -> String {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "cap")
        .unwrap()
        .1
        .into_owned()
}
fn id(view: &Value) -> &str {
    view["session_id"].as_str().unwrap()
}
fn state(gateway: &Gateway, view: &Value) -> Value {
    gateway.status("playback", id(view)).unwrap()
}
fn image_record(url: String) -> Image {
    let id = Uuid::new_v4().to_string();
    Image {
        reference: json!({"resource_id":id,"path":format!("/images/{id}"),"capability":capability().unwrap(),"expires_at":expires_after(IMAGE_IDLE_MS).unwrap()}),
        url,
        headers: HeaderMap::new(),
    }
}
fn projection(view: &Value) -> Value {
    let u = url::Url::parse(media(view)).unwrap();
    let mut result = view.clone();
    result["media_url"] = json!({"scheme":u.scheme(),"host":u.host_str(),"path":u.path(),"query_keys":u.query_pairs().map(|(k,_)|k.into_owned()).collect::<Vec<_>>(),"cap_bytes":URL_SAFE_NO_PAD.decode(cap(media(view))).unwrap().len(),"cap_sha256":format!("{:x}",Sha256::digest(cap(media(view)).as_bytes()))});
    result
}

#[test]
fn t09_capabilities_bearer_cors_csp_and_resource_isolation() {
    let fixture = Fixture::new(|_, _| Reply::body(vec![7; 256], "video/mp4"));
    let gateway = Gateway::start().unwrap();
    let first = session(&gateway, fixture.url("/video?upstream=secret"));
    let second = session(&gateway, fixture.url("/video2"));
    let a = gateway
        .image("a", image_record(fixture.url("/a.png")))
        .unwrap();
    let b = gateway
        .image("a", image_record(fixture.url("/b.png")))
        .unwrap();
    let values = [
        cap(media(&first)),
        cap(media(&second)),
        cap(a["url"].as_str().unwrap()),
        cap(b["url"].as_str().unwrap()),
        gateway.shared.token.clone(),
    ];
    let unique: std::collections::HashSet<_> = values.iter().collect();
    assert_eq!(unique.len(), 5);
    for c in &values {
        assert_eq!(URL_SAFE_NO_PAD.decode(c).unwrap().len(), 32);
        assert!(c.bytes().any(|b| b != c.as_bytes()[0]));
    }
    let path = url::Url::parse(media(&first)).unwrap();
    let bare = format!("http://{}{}", gateway.address, path.path());
    assert_eq!(fetch(&bare, "GET", &[]).status, 401);
    assert_eq!(
        fetch(&format!("{bare}?cap={}", values[1]), "GET", &[]).status,
        401
    );
    assert_eq!(
        fetch(&format!("{bare}?cap={}", values[2]), "GET", &[]).status,
        401
    );
    let control = format!("http://{}/host-health?cap={}", gateway.address, values[0]);
    assert_eq!(fetch(&control, "GET", &[]).status, 401);
    let native = fetch(
        &bare,
        "HEAD",
        &[("Authorization", &format!("Bearer {}", gateway.shared.token))],
    );
    assert_eq!(native.status, 200);
    assert!(native.body.is_empty());
    let allowed = fetch(media(&first), "GET", &[("Origin", "tauri://localhost")]);
    assert_eq!(allowed.status, 200);
    assert_eq!(
        allowed.headers["access-control-allow-origin"],
        "tauri://localhost"
    );
    assert!(
        !allowed
            .headers
            .contains_key("access-control-allow-credentials")
    );
    let denied = fetch(media(&first), "GET", &[("Origin", "https://evil.invalid")]);
    assert_eq!(denied.status, 401);
    assert!(!denied.headers.contains_key("access-control-allow-origin"));
    assert_eq!(fetch(media(&first), "POST", &[]).status, 400);
    assert_eq!(
        fetch(
            &media(&first).replace(path.path(), "/media/foreign/unknown"),
            "GET",
            &[]
        )
        .status,
        410
    );
    assert_eq!(
        fetch(
            &format!("{bare}?cap={}&cap={}", values[0], values[0]),
            "GET",
            &[]
        )
        .status,
        401
    );
    let preflight = fetch(
        &bare,
        "OPTIONS",
        &[
            ("Origin", "tauri://localhost"),
            ("Access-Control-Request-Method", "GET"),
            ("Access-Control-Request-Headers", "Range, If-Range"),
        ],
    );
    assert_eq!(preflight.status, 204);
    assert_eq!(
        fetch(
            &bare,
            "OPTIONS",
            &[
                ("Origin", "tauri://localhost"),
                ("Access-Control-Request-Method", "GET"),
                ("Access-Control-Request-Headers", "Authorization")
            ]
        )
        .status,
        400
    );
    let config: Value =
        serde_json::from_str(include_str!("../../src-tauri/tauri.conf.json")).unwrap();
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    for directive in [
        "script-src 'self'",
        "object-src 'none'",
        "frame-src 'none'",
        "img-src 'self'",
        "media-src 'self'",
        "connect-src 'self'",
    ] {
        assert!(csp.contains(directive));
    }
    assert!(!csp.contains("unsafe-eval"));
    println!(
        "T09_VECTOR:{}",
        json!({"kind":"binding","session":projection(&first),"resolution_id":first["resolution_id"],"stream_id":first["stream_id"],"reference":reference(),"other_cap_sha256":format!("{:x}",Sha256::digest(values[1].as_bytes())),"status":allowed.status,"cors":allowed.headers["access-control-allow-origin"],"unauthorized":denied.status})
    );
}

#[test]
fn t09_native_headers_cookie_referer_interceptor_and_no_local_secrets() {
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 3)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();
    let fixture = Fixture::new(move |r, _| {
        let valid = r.headers.get("cookie").map(String::as_str) == Some("provider=original")
            && r.headers.get("referer").map(String::as_str)
                == Some("https://provider.invalid/watch?signature=original")
            && r.headers.get("x-original").map(String::as_str) == Some("intercepted")
            && r.headers.get("authorization").map(String::as_str) == Some("Provider original");
        if r.target.starts_with("/image-required") {
            return Reply {
                status: if valid { 200 } else { 403 },
                ..Reply::body(png.clone(), "image/png")
            };
        }
        Reply {
            status: if valid { 200 } else { 403 },
            ..Reply::body(vec![3; 128], "video/mp4")
        }
    });
    let gateway = Gateway::start().unwrap();
    let mut selected = link(fixture.url("/required?signature=upstream"));
    selected.referer = "https://provider.invalid/watch?signature=original".into();
    selected
        .headers
        .insert("authorization", "Provider original".parse().unwrap());
    selected.interceptor = Some(Arc::new(HttpContext(Box::new(|_, h| {
        h.insert("x-original", "intercepted".parse().unwrap());
        Ok(())
    }))));
    selected.cookies = Some(Arc::new(HttpContext(Box::new(|_, h| {
        h.insert("cookie", "provider=original".parse().unwrap());
        Ok(())
    }))));
    let mut store = ResolutionStore::default();
    let r = register(&mut store, selected, vec![], 0);
    let view = open(&gateway, &mut store, &r);
    let response = fetch(
        media(&view),
        "GET",
        &[
            ("Cookie", "browser=wrong"),
            ("Referer", "https://evil.invalid/"),
            ("Authorization", &format!("Bearer {}", gateway.shared.token)),
            ("X-Original", "browser"),
        ],
    );
    assert_eq!(response.status, 200);
    let requests = fixture.state.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let upstream = &requests[0];
    assert!(upstream.target.contains("signature=upstream"));
    let observed = format!("{} {:?}", upstream.target, upstream.headers);
    let returned = format!(
        "{:?} {:?} {}",
        response.headers,
        response.body,
        state(&gateway, &view)
    );
    for secret in [cap(media(&view)), gateway.shared.token.clone()] {
        assert!(!observed.contains(&secret));
        assert!(!returned.contains(&secret));
        assert!(!String::from_utf8_lossy(&response.body).contains(&secret));
    }
    assert_eq!(upstream.headers["accept-encoding"], "identity");
    assert!(!upstream.headers.contains_key("origin"));
    drop(requests);
    let mut failing = link(fixture.url("/required?signature=must-not-leak"));
    failing.interceptor = Some(Arc::new(HttpContext(Box::new(|url, _| {
        Err(ApiError::new("UPSTREAM_FAILED", url.as_str()))
    }))));
    let r = register(&mut store, failing, vec![], 0);
    let failed = open(&gateway, &mut store, &r);
    let response = fetch(media(&failed), "GET", &[]);
    assert_eq!(response.status, 502);
    let recorded = state(&gateway, &failed).to_string();
    assert!(!recorded.contains("signature"));
    assert!(!recorded.contains("must-not-leak"));
    assert!(!String::from_utf8_lossy(&response.body).contains("signature"));
    let mut native_image = image_record(fixture.url("/image-required?signature=upstream-image"));
    for (name, value) in [
        ("cookie", "provider=original"),
        (
            "referer",
            "https://provider.invalid/watch?signature=original",
        ),
        ("x-original", "intercepted"),
        ("authorization", "Provider original"),
    ] {
        native_image.headers.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    let image = gateway.image("native-fixture", native_image).unwrap();
    let response = fetch(
        image["url"].as_str().unwrap(),
        "GET",
        &[
            ("Cookie", "browser=wrong"),
            ("Authorization", &format!("Bearer {}", gateway.shared.token)),
        ],
    );
    assert_eq!(response.status, 200);
    assert_eq!(response.headers["content-type"], "image/png");
    let requests = fixture.state.requests.lock().unwrap();
    let image_request = requests.last().unwrap();
    assert!(
        image_request
            .target
            .starts_with("/image-required?signature=upstream-image")
    );
    let observed = format!("{} {:?}", image_request.target, image_request.headers);
    for secret in [
        gateway.shared.token.clone(),
        cap(image["url"].as_str().unwrap()),
    ] {
        assert!(!observed.contains(&secret));
    }
}

#[test]
fn t09_range_if_range_200_206_416_and_head() {
    let fixture = Fixture::new(|r, _| {
        let bytes: Vec<_> = (0..=255).cycle().take(4096).collect();
        let mut reply = Reply::body(bytes.clone(), "video/mp4")
            .header("ETag", "\"original\"")
            .header("Accept-Ranges", "bytes");
        if r.headers
            .get("if-range")
            .is_some_and(|v| v != "\"original\"")
        {
            return reply;
        }
        if let Some(range) = r.headers.get("range") {
            let (start, end) = range
                .strip_prefix("bytes=")
                .unwrap()
                .split_once('-')
                .unwrap();
            let start = if start.is_empty() {
                4096 - end.parse::<usize>().unwrap()
            } else {
                start.parse().unwrap()
            };
            let end = if end.is_empty() || range.starts_with("bytes=-") {
                4095
            } else {
                end.parse::<usize>().unwrap().min(4095)
            };
            if start >= 4096 {
                return Reply {
                    status: 416,
                    ..Reply::body("", "video/mp4").header("Content-Range", "bytes */4096")
                };
            }
            reply.status = 206;
            reply.body = bytes[start..=end].to_vec();
            reply = reply.header("Content-Range", format!("bytes {start}-{end}/4096"));
        }
        reply
    });
    let gateway = Gateway::start().unwrap();
    let view = session(&gateway, fixture.url("/range"));
    let whole = fetch(media(&view), "GET", &[]);
    assert_eq!(whole.status, 200);
    assert_eq!(whole.body.len(), 4096);
    for (range, expected) in [
        ("bytes=10-19", (10, 19)),
        ("bytes=4090-", (4090, 4095)),
        ("bytes=-7", (4089, 4095)),
    ] {
        let result = fetch(
            media(&view),
            "GET",
            &[("Range", range), ("If-Range", "\"original\"")],
        );
        assert_eq!(result.status, 206);
        assert_eq!(
            result.headers["content-range"],
            format!("bytes {}-{}/4096", expected.0, expected.1)
        );
        assert_eq!(
            result.body,
            (expected.0..=expected.1)
                .map(|n| (n % 256) as u8)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            result.headers["content-length"],
            result.body.len().to_string()
        );
        println!(
            "T09_VECTOR:{}",
            json!({"kind":"range","requested":range,"status":result.status,"content_range":result.headers["content-range"],"content_length":result.headers["content-length"],"bytes":result.body})
        );
    }
    let unchanged = fetch(
        media(&view),
        "GET",
        &[("Range", "bytes=10-19"), ("If-Range", "\"stale\"")],
    );
    assert_eq!(unchanged.status, 200);
    assert_eq!(unchanged.body, whole.body);
    assert!(!unchanged.headers.contains_key("content-range"));
    let outside = fetch(media(&view), "GET", &[("Range", "bytes=9999-")]);
    assert_eq!(outside.status, 416);
    assert_eq!(outside.headers["content-range"], "bytes */4096");
    assert!(outside.body.is_empty());
    assert_eq!(state(&gateway, &view)["state"], "ready");
    let head = fetch(media(&view), "HEAD", &[("Range", "bytes=1-3")]);
    assert_eq!(head.status, 206);
    assert_eq!(head.headers["content-length"], "3");
    assert!(head.body.is_empty());
    for bad in ["bytes=3-2", "items=1-2", "bytes=0-1,3-4", "bytes=-0"] {
        assert_eq!(fetch(media(&view), "GET", &[("Range", bad)]).status, 400);
    }
}

#[test]
fn t09_gzip_redirects_private_hops_and_html() {
    const GZIP: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 2, 19, 75, 73, 77, 206, 79, 73, 77, 81, 200, 77, 77, 201, 76,
        84, 40, 72, 172, 204, 201, 79, 76, 73, 25, 21, 28, 21, 28, 21, 28, 21, 28, 21, 28, 21, 28,
        21, 28, 21, 68, 21, 4, 0, 152, 159, 90, 251, 52, 8, 0, 0,
    ];
    let fixture = Fixture::new(|r, port| match r.target.as_str() {
        "/gzip" => Reply::body(GZIP, "video/mp4")
            .header("Content-Encoding", "gzip")
            .header("Set-Cookie", "never=browser")
            .header("X-Upstream", "private"),
        "/gzip-crc" => {
            let mut broken = GZIP.to_vec();
            broken[GZIP.len() - 8] ^= 1;
            Reply::body(broken, "video/mp4").header("Content-Encoding", "gzip")
        }
        "/gzip-truncated" => {
            Reply::body(&GZIP[..GZIP.len() - 8], "video/mp4").header("Content-Encoding", "gzip")
        }
        "/redirect" => Reply::redirect("/gzip".into()),
        "/cross" => Reply::redirect(format!("http://other.fixture.test:{port}/capture")),
        "/private" => Reply::redirect(format!("http://127.0.0.1:{port}/must-not-reach")),
        "/loop" => Reply::redirect("/loop".into()),
        "/html" => Reply::body("<html>upstream error</html>", "text/html"),
        "/hidden-html" => Reply::body("<html>upstream error</html>", "video/mp4"),
        "/bad-partial" => Reply {
            status: 206,
            ..Reply::body("bad", "video/mp4").header("Content-Range", "bytes 0-4/100")
        },
        _ => Reply::body(vec![9; 128], "video/mp4"),
    });
    let gateway = Gateway::start().unwrap();
    for path in ["/gzip", "/redirect"] {
        let view = session(&gateway, fixture.url(path));
        let r = fetch(media(&view), "GET", &[]);
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"decoded media payload".repeat(100));
        assert!(!r.headers.contains_key("content-encoding"));
        assert!(!r.headers.contains_key("set-cookie"));
        assert!(!r.headers.contains_key("x-upstream"));
        assert_eq!(r.headers["transfer-encoding"], "chunked");
        assert_eq!(state(&gateway, &view)["state"], "ready");
    }
    for path in ["/gzip-crc", "/gzip-truncated"] {
        let view = session(&gateway, fixture.url(path));
        let mut reader = connect(media(&view), "GET", &[]);
        let response = read_head(&mut reader);
        let mut body = Vec::new();
        reader.read_to_end(&mut body).unwrap();
        // Decode failures may arrive before headers or after streaming starts.
        if response.status == 200 {
            assert_eq!(response.headers["transfer-encoding"], "chunked");
            assert!(
                !body.ends_with(b"0\r\n\r\n"),
                "invalid gzip completed successfully"
            );
        } else {
            assert_eq!(response.status, 502);
            let error: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(error["error"]["code"], "UPSTREAM_FAILED");
        }
        wait_for(|| state(&gateway, &view)["state"] == "failed");
        assert_eq!(state(&gateway, &view)["error"]["code"], "UPSTREAM_FAILED");
    }
    let mut l = link(fixture.url("/cross"));
    for (name, value) in [
        ("authorization", "Provider auth"),
        ("cookie", "provider=cookie"),
        ("referer", "https://provider.invalid/"),
        ("x-secret", "private"),
    ] {
        l.headers.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    let mut store = ResolutionStore::default();
    let r = register(&mut store, l, vec![], 0);
    let view = open(&gateway, &mut store, &r);
    assert_eq!(fetch(media(&view), "GET", &[]).status, 200);
    let captured = fixture
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.target == "/capture")
        .unwrap()
        .clone();
    for name in ["authorization", "cookie", "referer", "x-secret"] {
        assert!(!captured.headers.contains_key(name));
    }
    for path in ["/private", "/loop", "/html", "/hidden-html"] {
        let view = session(&gateway, fixture.url(path));
        assert_eq!(fetch(media(&view), "GET", &[]).status, 502);
        assert_eq!(state(&gateway, &view)["state"], "failed");
    }
    assert!(
        !fixture
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.target == "/must-not-reach")
    );
    let bad = session(&gateway, fixture.url("/bad-partial"));
    assert_eq!(
        fetch(media(&bad), "GET", &[("Range", "bytes=0-4")]).status,
        502
    );
}

#[test]
fn t09_slow_timeout_truncation_disconnect_and_close_cancel() {
    let fixture = Fixture::new(|r, _| match r.target.as_str() {
        "/slow" => Reply {
            header_delay: Duration::from_millis(60),
            body_delay: Duration::from_millis(60),
            ..Reply::body(vec![1; 128], "video/mp4")
        },
        "/truncated" => Reply {
            length: Some(4096),
            ..Reply::body(vec![2; 64], "video/mp4")
        },
        _ => Reply {
            body_delay: Duration::from_secs(5),
            ..Reply::body(vec![3; 128], "video/mp4")
        },
    });
    let gateway = Gateway::start().unwrap();
    gateway.shared.idle_ms.store(200, Ordering::SeqCst);
    let slow = session(&gateway, fixture.url("/slow"));
    assert_eq!(fetch(media(&slow), "GET", &[]).status, 200);
    let timeout = session(&gateway, fixture.url("/stall"));
    assert_eq!(fetch(media(&timeout), "GET", &[]).status, 504);
    assert_eq!(
        state(&gateway, &timeout)["error"]["code"],
        "UPSTREAM_TIMEOUT"
    );
    fixture.quiet();
    let truncated = session(&gateway, fixture.url("/truncated"));
    let r = fetch(media(&truncated), "GET", &[]);
    assert_eq!(r.status, 200);
    assert!(r.body.len() < 4096);
    wait_for(|| state(&gateway, &truncated)["state"] == "failed");
    assert_eq!(
        state(&gateway, &truncated)["error"]["code"],
        "UPSTREAM_FAILED"
    );
    gateway.shared.idle_ms.store(30_000, Ordering::SeqCst);
    let disconnected = session(&gateway, fixture.url("/disconnect"));
    let client = connect(media(&disconnected), "GET", &[]);
    wait_for(|| fixture.state.active.load(Ordering::SeqCst) == 1);
    drop(client);
    fixture.quiet();
    wait_for(|| gateway.shared.records.lock().unwrap().sessions[id(&disconnected)].active == 0);
    assert_eq!(state(&gateway, &disconnected)["state"], "ready");
    let closed = session(&gateway, fixture.url("/close"));
    let client = connect(media(&closed), "GET", &[]);
    wait_for(|| fixture.state.active.load(Ordering::SeqCst) == 1);
    let start = Instant::now();
    gateway.close(id(&closed));
    fixture.quiet();
    assert!(start.elapsed() < Duration::from_secs(2));
    drop(client);
    assert_eq!(fetch(media(&closed), "GET", &[]).status, 410);
    gateway.close(id(&closed));
    let parallel = session(&gateway, fixture.url("/concurrency"));
    let clients: Vec<_> = (0..RESOURCE_REQUESTS)
        .map(|_| connect(media(&parallel), "GET", &[]))
        .collect();
    wait_for(|| fixture.state.active.load(Ordering::SeqCst) == RESOURCE_REQUESTS);
    assert_eq!(fetch(media(&parallel), "GET", &[]).status, 413);
    assert_eq!(gateway.health().unwrap()["status"], "ready");
    assert_eq!(
        gateway.shared.records.lock().unwrap().sessions[id(&parallel)].active,
        RESOURCE_REQUESTS
    );
    drop(clients);
    fixture.quiet();
    wait_for(|| gateway.shared.records.lock().unwrap().sessions[id(&parallel)].active == 0);
    assert_eq!(state(&gateway, &parallel)["state"], "ready");
}

#[test]
fn t09_bounded_stream_backpressure_and_real_payload() {
    const SIZE: usize = 256 * 1024 * 1024;
    let fixture = Fixture::new(|_, _| Reply {
        repeat: SIZE / 8192,
        ..Reply::body(vec![0x5a; 8192], "video/mp4")
    });
    let gateway = Gateway::start().unwrap();
    let view = session(&gateway, fixture.url("/large"));
    let sample = std::env::var_os("T09_MEMORY_SAMPLE").map(std::path::PathBuf::from);
    if let Some(path) = &sample {
        std::fs::write(
            path,
            json!({"pid":std::process::id(),"phase":"baseline"}).to_string(),
        )
        .unwrap();
        thread::sleep(Duration::from_millis(150));
    }
    let mut client = connect(media(&view), "GET", &[]);
    let head = read_head(&mut client);
    assert_eq!(head.status, 200);
    assert_eq!(head.headers["content-length"], SIZE.to_string());
    if let Some(path) = &sample {
        std::fs::write(
            path,
            json!({"pid":std::process::id(),"phase":"stream"}).to_string(),
        )
        .unwrap();
    }
    thread::sleep(Duration::from_millis(150));
    let before_read = fixture.state.produced.load(Ordering::SeqCst);
    assert!(
        before_read < SIZE as u64 / 2,
        "upstream body was eagerly buffered"
    );
    let mut bytes = [0; STREAM_BUFFER];
    let mut count = 0;
    let mut hash = Sha256::new();
    loop {
        let n = client.read(&mut bytes).unwrap();
        if n == 0 {
            break;
        }
        assert!(bytes[..n].iter().all(|b| *b == 0x5a));
        count += n;
        hash.update(&bytes[..n]);
    }
    assert_eq!(count, SIZE);
    let mut expected = Sha256::new();
    for _ in 0..SIZE / 8192 {
        expected.update([0x5a; 8192]);
    }
    let actual = format!("{:x}", hash.finalize());
    assert_eq!(actual, format!("{:x}", expected.finalize()));
    fixture.quiet();
    if let Some(path) = &sample {
        std::fs::write(
            path,
            json!({"pid":std::process::id(),"phase":"finished"}).to_string(),
        )
        .unwrap();
    }
    println!(
        "T09_VECTOR:{}",
        json!({"kind":"stream","bytes":count,"first_buffer_limit":STREAM_BUFFER,"produced_before_read":before_read,"sha256":actual})
    );
}

#[test]
fn t09_active_over_five_minutes_release_and_original_network_context() {
    let gate = Arc::new(AtomicBool::new(false));
    let hold = gate.clone();
    let fixture = Fixture::new(move |_, _| Reply {
        repeat: 2,
        gate: Some(hold.clone()),
        ..Reply::body(vec![5; STREAM_BUFFER], "video/mp4")
    });
    let clock = Arc::new(TestClock::default());
    let gateway = Gateway::with_clock(clock.clone()).unwrap();
    let cookie = Arc::new(Mutex::new("original".to_owned()));
    let captured = cookie.clone();
    let context = Arc::new(HttpContext(Box::new(move |_, h| {
        h.insert(
            "cookie",
            format!("provider={}", captured.lock().unwrap())
                .parse()
                .unwrap(),
        );
        Ok(())
    })));
    let weak = Arc::downgrade(&context);
    let mut l = link(fixture.url("/held"));
    l.cookies = Some(context.clone());
    let mut store = ResolutionStore::default();
    let r = register(&mut store, l, vec![], 0);
    let view = open(&gateway, &mut store, &r);
    drop(context);
    let mut client = connect(media(&view), "GET", &[]);
    assert_eq!(read_head(&mut client).status, 200);
    let mut first = [0; 64];
    client.read_exact(&mut first).unwrap();
    assert_eq!(first, [5; 64]);
    clock.0.store(300_001, Ordering::SeqCst);
    store.release(r["id"].as_str().unwrap());
    assert!(
        store
            .selected(
                r["id"].as_str().unwrap(),
                r["streams"][0]["id"].as_str().unwrap(),
                clock.now_ms()
            )
            .is_err()
    );
    assert!(weak.upgrade().is_some());
    assert_eq!(state(&gateway, &view)["state"], "ready");
    *cookie.lock().unwrap() = "renewed-after-resolution-release".into();
    gate.store(true, Ordering::SeqCst);
    let mut rest = Vec::new();
    client.read_to_end(&mut rest).unwrap();
    assert_eq!(rest.len() + 64, STREAM_BUFFER * 2);
    wait_for(|| gateway.shared.records.lock().unwrap().sessions[id(&view)].active == 0);
    assert_eq!(
        fetch(media(&view), "GET", &[]).body.len(),
        STREAM_BUFFER * 2
    );
    let requests = fixture.state.requests.lock().unwrap();
    assert_eq!(requests[0].headers["cookie"], "provider=original");
    assert_eq!(
        requests[1].headers["cookie"],
        "provider=renewed-after-resolution-release"
    );
    drop(requests);
    clock.0.store(600_000, Ordering::SeqCst);
    assert_eq!(state(&gateway, &view)["state"], "ready");
    clock.0.store(600_001, Ordering::SeqCst);
    assert_eq!(state(&gateway, &view)["state"], "expired");
    wait_for(|| weak.upgrade().is_none());
    assert_eq!(fetch(media(&view), "GET", &[]).status, 410);
    println!(
        "T09_VECTOR:{}",
        json!({"kind":"lifetime","active_at_ms":300001,"continued_bytes":STREAM_BUFFER*2,"resolution_released":true,"post_release_context":"renewed-after-resolution-release","idle_expired_at_ms":600001})
    );
}

#[test]
fn t09_image_decoders_byte_pixel_limits_and_mime() {
    let mut encoded = HashMap::new();
    for (format, path) in [
        (image::ImageFormat::Png, "/png"),
        (image::ImageFormat::Jpeg, "/jpeg"),
        (image::ImageFormat::Gif, "/gif"),
        (image::ImageFormat::WebP, "/webp"),
    ] {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 3)
            .write_to(&mut bytes, format)
            .unwrap();
        encoded.insert(path.to_owned(), bytes.into_inner());
    }
    let png = encoded["/png"].clone();
    let mut dimensions = png.clone();
    dimensions[16..20].copy_from_slice(&8193u32.to_be_bytes());
    fix_png_crc(&mut dimensions);
    let mut pixels = png.clone();
    pixels[16..20].copy_from_slice(&4097u32.to_be_bytes());
    pixels[20..24].copy_from_slice(&4096u32.to_be_bytes());
    fix_png_crc(&mut pixels);
    let expected = encoded.clone();
    let fixture = Fixture::new(move |r, _| match r.target.as_str() {
        "/declared-large" => Reply {
            length: Some(IMAGE_BYTES + 1),
            ..Reply::body("", "image/png")
        },
        "/stream-large" => Reply {
            repeat: IMAGE_BYTES / 8192 + 1,
            chunked: true,
            ..Reply::body(vec![0; 8192], "image/png")
        },
        "/dimension" => Reply::body(dimensions.clone(), "image/png"),
        "/pixels" => Reply::body(pixels.clone(), "image/png"),
        "/invalid" => Reply::body("not an image", "image/jpeg"),
        path => Reply::body(encoded[path].clone(), "application/octet-stream"),
    });
    let gateway = Gateway::start().unwrap();
    for (path, mime) in [
        ("/png", "image/png"),
        ("/jpeg", "image/jpeg"),
        ("/gif", "image/gif"),
        ("/webp", "image/webp"),
    ] {
        let record = image_record(fixture.url(path));
        let access = gateway.image("images", record).unwrap();
        let response = fetch(access["url"].as_str().unwrap(), "GET", &[]);
        assert_eq!(response.status, 200);
        assert_eq!(response.body, expected[path]);
        assert_eq!(response.headers["content-type"], mime);
        assert_eq!(
            response.headers["content-length"],
            response.body.len().to_string()
        );
        assert_eq!(response.headers["cache-control"], "private, max-age=60");
        assert_eq!(
            response.headers["etag"],
            format!("\"{:x}\"", Sha256::digest(&response.body))
        );
        let head = fetch(access["url"].as_str().unwrap(), "HEAD", &[]);
        assert_eq!(head.status, 200);
        assert!(head.body.is_empty());
        assert_eq!(
            head.headers["content-length"],
            response.body.len().to_string()
        );
    }
    for path in [
        "/declared-large",
        "/stream-large",
        "/dimension",
        "/pixels",
        "/invalid",
    ] {
        let record = image_record(fixture.url(path));
        let access = gateway.image("images", record).unwrap();
        let response = fetch(access["url"].as_str().unwrap(), "GET", &[]);
        assert_eq!(response.status, if path == "/invalid" { 502 } else { 413 });
        let diagnostic = gateway
            .status("image", access["resource_id"].as_str().unwrap())
            .unwrap();
        assert_eq!(diagnostic["state"], "failed");
        assert_eq!(
            diagnostic["error"]["code"],
            if path == "/invalid" {
                "MEDIA_UNSUPPORTED"
            } else {
                "RESOURCE_LIMIT"
            }
        );
    }
    println!(
        "T09_VECTOR:{}",
        json!({"kind":"image_limits","bytes":IMAGE_BYTES,"pixels":IMAGE_PIXELS,"dimension":8192,"formats":["png","jpeg","gif","webp"],"oversized_http":413})
    );
}
fn fix_png_crc(bytes: &mut [u8]) {
    let mut crc = !0u32;
    for b in &bytes[12..29] {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 1 { 0xedb88320 } else { 0 };
        }
    }
    bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
}

#[test]
fn t09_image_sliding_lease_readonly_status_lru_and_epoch() {
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let body = png.into_inner();
    let fixture = Fixture::new(move |_, _| Reply::body(body.clone(), "image/png"));
    let clock = Arc::new(TestClock::default());
    let gateway = Gateway::with_clock(clock.clone()).unwrap();
    let original = image_record(fixture.url("/same"));
    let access = gateway.image("source", original.clone()).unwrap();
    let mut regenerated = original.clone();
    regenerated.reference["capability"] = json!(capability().unwrap());
    assert_eq!(
        gateway.image("source", regenerated).unwrap()["url"],
        access["url"]
    );
    let resource = access["resource_id"].as_str().unwrap();
    clock.0.store(IMAGE_IDLE_MS - 1, Ordering::SeqCst);
    for _ in 0..10 {
        assert_eq!(gateway.status("image", resource).unwrap()["state"], "ready");
    }
    assert!(fixture.state.requests.lock().unwrap().is_empty());
    clock.0.store(IMAGE_IDLE_MS, Ordering::SeqCst);
    assert_eq!(
        gateway.status("image", resource).unwrap()["state"],
        "expired"
    );
    assert_eq!(
        fetch(access["url"].as_str().unwrap(), "GET", &[]).status,
        410
    );
    let renewed = gateway.image("source", original.clone()).unwrap();
    assert_eq!(renewed["url"], access["url"]);
    assert_eq!(
        fetch(renewed["url"].as_str().unwrap(), "GET", &[]).status,
        200
    );
    clock.0.store(2 * IMAGE_IDLE_MS - 1, Ordering::SeqCst);
    assert_eq!(
        fetch(renewed["url"].as_str().unwrap(), "GET", &[]).status,
        200
    );
    clock.0.store(2 * IMAGE_IDLE_MS + 1, Ordering::SeqCst);
    assert_eq!(gateway.status("image", resource).unwrap()["state"], "ready");
    for index in 0..IMAGE_COUNT {
        gateway
            .image("other", image_record(fixture.url(&format!("/lru/{index}"))))
            .unwrap();
    }
    assert_eq!(
        gateway.shared.records.lock().unwrap().images.len(),
        IMAGE_COUNT
    );
    assert_eq!(
        gateway.status("image", resource).unwrap()["state"],
        "expired"
    );
    let current = gateway.image("source", original).unwrap();
    gateway.revoke(&["other".into()]);
    assert_eq!(gateway.status("image", resource).unwrap()["state"], "ready");
    gateway.revoke(&["source".into()]);
    assert_eq!(
        gateway.status("image", resource).unwrap()["state"],
        "expired"
    );
    let next = Gateway::start().unwrap();
    assert_ne!(next.shared.token, gateway.shared.token);
    assert_eq!(
        next.status("image", current["resource_id"].as_str().unwrap())
            .unwrap()["state"],
        "expired"
    );
    let unknown = gateway.status("image", "unknown").unwrap();
    assert_eq!(unknown["state"], "expired");
    assert_eq!(unknown["error"]["code"], "SESSION_EXPIRED");
}

#[test]
fn t09_diagnostics_request_sequence_and_other_resource_success() {
    let gate = Arc::new(AtomicBool::new(false));
    let pending = gate.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let fixture = Fixture::new(move |r, _| {
        if r.target == "/subtitle" {
            return Reply::body("WEBVTT\n\n", "text/vtt");
        }
        let n = calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Reply {
                status: 500,
                header_delay: Duration::from_millis(150),
                ..Reply::body("failure", "text/plain")
            }
        } else if n == 2 {
            Reply {
                status: 500,
                ..Reply::body("failure", "text/plain")
            }
        } else {
            Reply {
                gate: Some(pending.clone()),
                ..Reply::body(vec![4; 128], "video/mp4")
            }
        }
    });
    let gateway = Arc::new(Gateway::start().unwrap());
    let mut store = ResolutionStore::default();
    let subtitle = Subtitle {
        original: json!({"url":fixture.url("/subtitle")}),
        url: fixture.url("/subtitle"),
        headers: HeaderMap::new(),
        label: "original".into(),
        language: None,
        format: "vtt".into(),
    };
    let r = register(&mut store, link(fixture.url("/root")), vec![subtitle], 0);
    let view = open(&gateway, &mut store, &r);
    let g = gateway.clone();
    let old = view.clone();
    let old = thread::spawn(move || fetch(media(&old), "GET", &[]));
    wait_for(|| count.load(Ordering::SeqCst) >= 1);
    assert_eq!(fetch(media(&view), "GET", &[]).status, 200);
    assert_eq!(old.join().unwrap().status, 502);
    assert_eq!(state(&g, &view)["state"], "ready");
    assert_eq!(fetch(media(&view), "GET", &[]).status, 502);
    assert_eq!(state(&gateway, &view)["state"], "failed");
    let subtitle_id = r["subtitles"][0]["id"].as_str().unwrap();
    let url = gateway.url(
        &format!("/media/{}/{subtitle_id}", id(&view)),
        &cap(media(&view)),
    );
    assert_eq!(fetch(&url, "GET", &[]).status, 200);
    assert_eq!(state(&gateway, &view)["state"], "failed");
    assert_eq!(fetch(media(&view), "GET", &[]).status, 200);
    assert_eq!(state(&gateway, &view)["state"], "ready");
    gate.store(true, Ordering::SeqCst);
}

#[test]
fn t09_same_episode_multiple_streams_and_late_close_release() {
    let fixture = Fixture::new(|r, _| {
        Reply::body(vec![if r.target == "/a" { 1 } else { 2 }; 128], "video/mp4")
    });
    let gateway = Gateway::start().unwrap();
    let mut store = ResolutionStore::default();
    let resolution = store
        .register(
            &reference(),
            vec![link(fixture.url("/a")), link(fixture.url("/b"))],
            vec![],
            0,
        )
        .unwrap();
    let first = open(&gateway, &mut store, &resolution);
    let second_link = store
        .selected(
            resolution["id"].as_str().unwrap(),
            resolution["streams"][1]["id"].as_str().unwrap(),
            0,
        )
        .unwrap();
    gateway.close(id(&first));
    let second = gateway.open(second_link).unwrap();
    assert_eq!(second["resolution_id"], first["resolution_id"]);
    assert_ne!(second["stream_id"], first["stream_id"]);
    assert_ne!(cap(media(&second)), cap(media(&first)));
    assert_eq!(fetch(media(&second), "GET", &[]).body, vec![2; 128]);
    assert!(
        store
            .selected(
                resolution["id"].as_str().unwrap(),
                resolution["streams"][0]["id"].as_str().unwrap(),
                0
            )
            .is_ok()
    );
    store.release(resolution["id"].as_str().unwrap());
    gateway.close(id(&first));
    store.release(resolution["id"].as_str().unwrap());
    assert_eq!(fetch(media(&first), "GET", &[]).status, 410);
    assert_eq!(fetch(media(&second), "GET", &[]).body, vec![2; 128]);
    assert_eq!(state(&gateway, &second)["state"], "ready");
}

struct Dns {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Dns {
    fn new(answer: impl Fn(u16) -> Vec<u8> + Send + 'static) -> Self {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let running = stop.clone();
        let worker = thread::spawn(move || {
            let mut packet = [0; 2048];
            while !running.load(Ordering::SeqCst) {
                let (n, peer) = match socket.recv_from(&mut packet) {
                    Ok(v) => v,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        ) =>
                    {
                        continue;
                    }
                    Err(e) => panic!("DNS fixture: {e}"),
                };
                if n < 17 {
                    continue;
                }
                let mut end = 12;
                while end < n && packet[end] != 0 {
                    end += packet[end] as usize + 1;
                }
                end += 1;
                if end + 4 > n {
                    continue;
                }
                let kind = u16::from_be_bytes([packet[end], packet[end + 1]]);
                let data = answer(kind);
                let mut response = vec![
                    packet[0],
                    packet[1],
                    0x81,
                    0x80,
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
                    response.extend_from_slice(&[0, 1, 0, 0, 0, 0]);
                    response.extend_from_slice(&(data.len() as u16).to_be_bytes());
                    response.extend_from_slice(&data);
                }
                socket.send_to(&response, peer).unwrap();
            }
        });
        Self {
            address,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Dns {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn t09_ssrf_actual_peer_dns_mixture_and_rebinding() {
    let fixture = Fixture::new(|_, _| Reply::body(vec![2; 128], "video/mp4"));
    let generation = Arc::new(AtomicUsize::new(0));
    let current = generation.clone();
    let dns = Dns::new(move |kind| {
        if kind == 1 {
            if current.load(Ordering::SeqCst) == 0 {
                vec![127, 0, 0, 1]
            } else {
                vec![10, 0, 0, 1]
            }
        } else {
            vec![]
        }
    });
    let http = Http {
        nameserver: Some(vec![dns.address]),
    };
    let gateway = Gateway::with_http(Arc::new(MonotonicClock::default()), http.clone()).unwrap();
    let view = session(&gateway, fixture.url("/dns"));
    assert_eq!(fetch(media(&view), "GET", &[]).status, 200);
    assert_eq!(fixture.state.requests.lock().unwrap().len(), 1);
    generation.store(1, Ordering::SeqCst);
    assert_eq!(fetch(media(&view), "GET", &[]).status, 502);
    assert_eq!(fixture.state.requests.lock().unwrap().len(), 1);
    let mixed = Dns::new(|kind| {
        if kind == 1 {
            vec![127, 0, 0, 1]
        } else {
            std::net::Ipv6Addr::LOCALHOST.octets().to_vec()
        }
    });
    let gateway = Gateway::with_http(
        Arc::new(MonotonicClock::default()),
        Http {
            nameserver: Some(vec![mixed.address]),
        },
    )
    .unwrap();
    let view = session(&gateway, fixture.url("/mixed"));
    assert_eq!(fetch(media(&view), "GET", &[]).status, 502);
    assert_eq!(fixture.state.requests.lock().unwrap().len(), 1);
    let url = http_url("https://public.invalid/video", false).unwrap();
    let public: SocketAddr = "8.8.8.8:443".parse().unwrap();
    assert!(http.check_addresses(&url, &[public], false).is_ok());
    assert!(
        http.check_addresses(&url, &[public, "10.0.0.1:443".parse().unwrap()], false)
            .is_err()
    );
    assert!(
        http.check_peer(&url, &[public], Some("127.0.0.1:443".parse().unwrap()))
            .is_err()
    );
    assert!(http.check_peer(&url, &[public], None).is_err());
    for value in [
        "http://127.0.0.1/",
        "http://[::ffff:127.0.0.1]/",
        "http://169.254.169.254/",
        "http://10.0.0.1/",
    ] {
        let gateway = Gateway::start().unwrap();
        let view = session(&gateway, value.into());
        assert_eq!(fetch(media(&view), "GET", &[]).status, 502);
    }
    assert!(!crate::host::maccms::public_ip(IpAddr::V4(
        Ipv4Addr::LOCALHOST
    )));
}
