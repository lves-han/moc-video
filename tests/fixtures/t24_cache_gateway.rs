//! Cache proof uses real HTTP request counts and disk entries, with the production gateway lease checks.
use super::*;
use crate::media_cache::MediaCache;
fn cached() -> (tempfile::TempDir, Arc<MediaCache>, Gateway) {
    let temp = tempfile::tempdir().unwrap();
    let cache = MediaCache::new(temp.path().join("media-cache"), 1024 * 1024);
    let gateway =
        Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache.clone()).unwrap();
    (temp, cache, gateway)
}
fn count(fixture: &Fixture, path: &str) -> usize {
    fixture
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.target == path)
        .count()
}
#[test]
fn t24_cache_gateway_hits_ranges_headers_and_clear() {
    let fixture = Fixture::new(|request, _| {
        let mut reply = Reply::body(vec![7; 128], "video/mp4")
            .header("Cache-Control", "max-age=3600")
            .header("ETag", "\"entity-1\"");
        if let Some(range) = request.headers.get("range") {
            reply.status = 206;
            if range == "bytes=0-9" {
                reply.body = vec![1; 10];
                reply = reply.header("Content-Range", "bytes 0-9/128");
            } else {
                reply.body = vec![2; 10];
                reply = reply.header("Content-Range", "bytes 10-19/128");
            }
        }
        reply
    });
    let (temp, cache, gateway) = cached();
    let view = session(&gateway, fixture.url("/movie.mp4?signature=PRIVATE_SOURCE"));
    let path = "/movie.mp4?signature=PRIVATE_SOURCE";
    assert_eq!(fetch(media(&view), "GET", &[]).body, vec![7; 128]);
    assert_eq!(fetch(media(&view), "GET", &[]).body, vec![7; 128]);
    assert_eq!(count(&fixture, path), 1);
    assert_eq!(
        fetch(media(&view), "GET", &[("Range", "bytes=0-9")]).body,
        vec![1; 10]
    );
    assert_eq!(
        fetch(media(&view), "GET", &[("Range", "bytes=0-9")]).status,
        206
    );
    assert_eq!(count(&fixture, path), 2);
    assert_eq!(
        fetch(media(&view), "GET", &[("Range", "bytes=10-19")]).body,
        vec![2; 10]
    );
    assert_eq!(count(&fixture, path), 3);
    fetch(media(&view), "HEAD", &[]);
    assert_eq!(count(&fixture, path), 4);
    fetch(media(&view), "GET", &[("Cache-Control", "no-store")]);
    assert_eq!(count(&fixture, path), 5);
    let unauthorized = media(&view).split('?').next().unwrap();
    assert_eq!(fetch(unauthorized, "GET", &[]).status, 401);
    assert_eq!(count(&fixture, path), 5);
    assert_eq!(cache.status()["entry_count"], 3);
    for entry in std::fs::read_dir(temp.path().join("media-cache")).unwrap() {
        let data = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(!String::from_utf8_lossy(&data).contains("PRIVATE_SOURCE"));
    }
    assert_eq!(cache.clear().unwrap()["used_bytes"], 0);
    fetch(media(&view), "GET", &[]);
    assert_eq!(count(&fixture, path), 6);
    cache.configure(0);
    fetch(media(&view), "GET", &[]);
    fetch(media(&view), "GET", &[]);
    assert_eq!(count(&fixture, path), 8);
    gateway.close(view["session_id"].as_str().unwrap());
    assert_eq!(fetch(media(&view), "GET", &[]).status, 410);
}
#[test]
fn t24_cache_gateway_hls_does_not_persist_keys_or_manifests() {
    let fixture = Fixture::new(|request, _| {
        let (body, mime) = if request.target == "/list.m3u8" {
            (b"#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-KEY:METHOD=AES-128,URI=\"/keyblob\"\n#EXT-X-MAP:URI=\"/init\"\n#EXTINF:1,\n/part.ts\n#EXT-X-ENDLIST\n".to_vec(),"application/vnd.apple.mpegurl")
        } else if request.target == "/keyblob" {
            (vec![9; 16], "application/octet-stream")
        } else {
            (vec![1; 256], "video/mp2t")
        };
        Reply::body(body, mime).header("Cache-Control", "max-age=3600")
    });
    let (_temp, cache, gateway) = cached();
    let mut store = ResolutionStore::default();
    let mut value = link(fixture.url("/list.m3u8"));
    value.kind = "hls".into();
    value.mime_type = Some("application/vnd.apple.mpegurl".into());
    let registered = register(&mut store, value, vec![], 0);
    let view = open(&gateway, &mut store, &registered);
    let response = fetch(media(&view), "GET", &[]);
    assert_eq!(response.status, 200);
    let text = std::str::from_utf8(&response.body).unwrap();
    let refs = hls::references(text).unwrap();
    assert_eq!(refs.len(), 3);
    let urls: Vec<_> = refs
        .iter()
        .map(|r| text[r.span.clone()].to_owned())
        .collect();
    for url in &urls {
        assert_eq!(fetch(url, "GET", &[]).status, 200);
        assert_eq!(fetch(url, "GET", &[]).status, 200);
    }
    fetch(media(&view), "GET", &[]);
    assert_eq!(count(&fixture, "/list.m3u8"), 2);
    assert_eq!(count(&fixture, "/keyblob"), 2);
    assert_eq!(count(&fixture, "/init"), 1);
    assert_eq!(count(&fixture, "/part.ts"), 1);
    assert_eq!(cache.status()["entry_count"], 2);
    gateway.close(view["session_id"].as_str().unwrap());
    assert_eq!(fetch(&urls[2], "GET", &[]).status, 410);
}
#[test]
fn t24_cache_gateway_contexts_policy_and_unusable_disk() {
    let fixture = Fixture::new(|request, _| {
        let cache_control = if request.target == "/private.mp4" {
            "no-store"
        } else if request.target == "/revalidate.mp4" {
            "no-cache"
        } else {
            "max-age=3600"
        };
        let byte = if request
            .headers
            .get("cookie")
            .is_some_and(|v| v == "session=SECRET_A")
        {
            1
        } else {
            2
        };
        Reply::body(vec![byte; 128], "video/mp4").header("Cache-Control", cache_control)
    });
    let (_temp, cache, gateway) = cached();
    let mut views = Vec::new();
    for cookie in ["session=SECRET_A", "session=SECRET_B", "session=SECRET_A"] {
        let mut value = link(fixture.url("/context.mp4"));
        value.headers.insert("cookie", cookie.parse().unwrap());
        let mut store = ResolutionStore::default();
        let registered = register(&mut store, value, vec![], 0);
        let view = open(&gateway, &mut store, &registered);
        assert_eq!(
            fetch(media(&view), "GET", &[]).body,
            vec![if cookie.ends_with("_A") { 1 } else { 2 }; 128]
        );
        views.push(view);
    }
    assert_eq!(count(&fixture, "/context.mp4"), 2);
    for path in ["/private.mp4", "/revalidate.mp4"] {
        let view = session(&gateway, fixture.url(path));
        fetch(media(&view), "GET", &[]);
        fetch(media(&view), "GET", &[]);
        assert_eq!(count(&fixture, path), 2);
    }
    assert_eq!(cache.status()["entry_count"], 2);
    let temp = tempfile::tempdir().unwrap();
    let blocked = temp.path().join("blocked");
    std::fs::write(&blocked, b"keep").unwrap();
    let unavailable = MediaCache::new(blocked, 1024 * 1024);
    let gateway =
        Gateway::with_media_cache(Arc::new(MonotonicClock::default()), unavailable.clone())
            .unwrap();
    let view = session(&gateway, fixture.url("/still-plays.mp4"));
    assert_eq!(fetch(media(&view), "GET", &[]).body.len(), 128);
    assert!(!unavailable.status()["last_error"].is_null());
}

#[test]
fn t24_cache_gateway_clear_while_cached_http_read_continues() {
    let fixture = Fixture::new(|_, _| Reply {
        repeat: 128,
        ..Reply::body(vec![5; 65536], "video/mp4").header("Cache-Control", "max-age=3600")
    });
    let (_temp, cache, gateway) = cached();
    cache.configure(16 * 1024 * 1024);
    let view = session(&gateway, fixture.url("/large.mp4"));
    assert_eq!(fetch(media(&view), "GET", &[]).body.len(), 8 * 1024 * 1024);
    assert_eq!(count(&fixture, "/large.mp4"), 1);
    let mut reader = connect(media(&view), "GET", &[]);
    let response = read_head(&mut reader);
    assert_eq!(response.status, 200);
    let cleared = cache.clear().unwrap();
    assert_eq!(cleared["entry_count"], 0);
    assert!(cleared["pending_bytes"].as_u64().unwrap() > 0);
    let mut body = Vec::new();
    reader.read_to_end(&mut body).unwrap();
    assert_eq!(body.len(), 8 * 1024 * 1024);
    assert!(body.iter().all(|byte| *byte == 5));
    wait_for(|| cache.status()["used_bytes"] == 0);
    assert_eq!(count(&fixture, "/large.mp4"), 1);
}
