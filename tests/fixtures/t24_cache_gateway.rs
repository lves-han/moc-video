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
        let entry = entry.unwrap();
        if entry.file_name() == ".lock" {
            assert_eq!(entry.metadata().unwrap().len(), 0);
            continue;
        }
        let data = std::fs::read(entry.path()).unwrap();
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

#[test]
fn t24_cache_gateway_clear_before_upstream_response_blocks_old_fill() {
    struct Release(Arc<AtomicBool>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    for lower in [false, true] {
        let gate = Arc::new(AtomicBool::new(false));
        let opened = gate.clone();
        let fixture = Fixture::new(move |_, _| {
            while !opened.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(2));
            }
            Reply::body(vec![6; 128], "video/mp4").header("Cache-Control", "max-age=3600")
        });
        let (_temp, cache, gateway) = cached();
        let view = session(&gateway, fixture.url("/late.mp4"));
        let url = media(&view).to_owned();
        let worker = thread::spawn(move || fetch(&url, "GET", &[]));
        let _release = Release(gate.clone());
        wait_for(|| count(&fixture, "/late.mp4") == 1);
        assert_eq!(
            cache.status()["used_bytes"],
            0,
            "response has not reached the cache writer"
        );
        if lower {
            cache.configure(512 * 1024);
        } else {
            assert_eq!(cache.clear().unwrap()["used_bytes"], 0);
        }
        gate.store(true, Ordering::SeqCst);
        let response = worker.join().unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, vec![6; 128]);
        assert_eq!(
            cache.status()["used_bytes"],
            0,
            "request predating the barrier refilled the cache"
        );
        assert_eq!(cache.status()["entry_count"], 0);
        assert_eq!(fetch(media(&view), "GET", &[]).body, vec![6; 128]);
        assert_eq!(count(&fixture, "/late.mp4"), 2);
        assert_eq!(
            cache.status()["entry_count"],
            1,
            "a genuinely new request can cache"
        );
    }
}

#[test]
fn episode_cache_separates_shared_urls_and_reuses_each_episode() {
    let fixture = Fixture::new(|_, _| Reply::body(vec![7; 128], "video/mp4").header("Cache-Control", "max-age=3600"));
    let temp = tempfile::tempdir().unwrap();
    let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache.clone()).unwrap();
    let mut store = ResolutionStore::default();
    let resolution = register(&mut store, link(fixture.url("/shared.mp4")), vec![], 0);
    let mut selected = || store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let open_cached = |selected, owner: &str| {
        cache.register_episode(owner).unwrap();
        let view = gateway.open(selected).unwrap();
        gateway.shared.records.lock().unwrap().sessions.get_mut(view["session_id"].as_str().unwrap()).unwrap().episode = Some(owner.to_owned());
        view
    };
    let first = open_cached(selected(), "episode-1");
    assert_eq!(fetch(media(&first), "GET", &[]).status, 200);
    assert_eq!(fetch(media(&first), "GET", &[]).status, 200);
    assert_eq!(count(&fixture, "/shared.mp4"), 1);
    let second = open_cached(selected(), "episode-2");
    assert_eq!(fetch(media(&second), "GET", &[]).status, 200);
    assert_eq!(count(&fixture, "/shared.mp4"), 2);
}

fn play_first_hls_segment(body: &[u8]) -> String {
    let text = std::str::from_utf8(body).unwrap();
    let url = text.lines().find(|line| line.starts_with("http://")).unwrap().to_owned();
    assert_eq!(fetch(&url, "GET", &[]).status, 200);
    url
}

#[test]
fn episode_hls_prefetches_current_playlist_and_reports_verified_ranges() {
    let fixture = Fixture::new(|request, _| {
        if request.target == "/playlist.m3u8" {
            Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/one.ts\n#EXTINF:4,\n/two.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl")
        } else { Reply::body(vec![7; 128], "video/mp2t") }
    });
    let temp = tempfile::tempdir().unwrap();
    let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/playlist.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-hls".into()).unwrap();
    let manifest = fetch(media(&view), "GET", &[]); assert_eq!(manifest.status, 200); play_first_hls_segment(&manifest.body);
    let mut status_url = url::Url::parse(media(&view)).unwrap(); status_url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    wait_for(|| {
        let response = fetch(status_url.as_str(), "GET", &[]);
        let data: Value = serde_json::from_slice(&response.body).unwrap();
        if data["status"] == "failed" { panic!("prefetch failed: {data}"); }
        data["status"] == "complete"
    });
    let response = fetch(status_url.as_str(), "GET", &[]);
    let data: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(data["total_ms"], 8000);
    assert_eq!(data["ranges"], json!([{"start_ms":0,"end_ms":8000}]));
    assert_eq!(count(&fixture, "/one.ts"), 1); assert_eq!(count(&fixture, "/two.ts"), 1);
    gateway.close(view["session_id"].as_str().unwrap());
    assert_eq!(fetch(status_url.as_str(), "GET", &[]).status, 401);
}

#[test]
fn hls_prefetch_stops_on_close_and_reuses_completed_segments() {
    let fixture = Fixture::new(|request, _| {
        if request.target == "/playlist-stop.m3u8" {
            Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/first-stop.ts\n#EXTINF:4,\n/slow-stop.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl")
        } else {
            let mut result = Reply::body(vec![7; 128], "video/mp2t");
            if request.target == "/slow-stop.ts" { result.body_delay = Duration::from_secs(2); }
            result
        }
    });
    let temp = tempfile::tempdir().unwrap();
    let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache.clone()).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/playlist-stop.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let mut selected = || store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let first = gateway.open_episode(selected(), "episode-stop".into()).unwrap();
    let manifest = fetch(media(&first), "GET", &[]); assert_eq!(manifest.status, 200); play_first_hls_segment(&manifest.body);
    wait_for(|| count(&fixture, "/first-stop.ts") == 1 && count(&fixture, "/slow-stop.ts") == 1 && cache.status()["entry_count"] == 1);
    gateway.close(first["session_id"].as_str().unwrap());
    let second = gateway.open_episode(selected(), "episode-stop".into()).unwrap();
    let manifest = fetch(media(&second), "GET", &[]); assert_eq!(manifest.status, 200); play_first_hls_segment(&manifest.body);
    let mut url = url::Url::parse(media(&second)).unwrap(); url.set_path(&format!("/cache/{}", second["session_id"].as_str().unwrap()));
    wait_for(|| {
        let data: Value = serde_json::from_slice(&fetch(url.as_str(), "GET", &[]).body).unwrap();
        if data["status"] == "failed" { panic!("prefetch failed: {data}"); }
        data["status"] == "complete"
    });
    assert_eq!(count(&fixture, "/first-stop.ts"), 1);
    assert_eq!(cache.status()["entry_count"], 2);
}

#[test]
fn hls_prefetch_downloads_the_variant_requested_by_the_player() {
    let fixture = Fixture::new(|request, _| match request.target.as_str() {
        "/master.m3u8" => Reply::body("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100000\n/low.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=900000\n/high.m3u8\n", "application/vnd.apple.mpegurl"),
        "/low.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/low.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        "/high.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/high.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        _ => Reply::body(vec![9; 128], "video/mp2t"),
    });
    let temp = tempfile::tempdir().unwrap();
    let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/master.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-variant".into()).unwrap();
    let master = fetch(media(&view), "GET", &[]);
    let body = String::from_utf8(master.body).unwrap();
    let variants: Vec<_> = body.lines().filter(|line| line.starts_with("http://")).collect();
    assert_eq!(fetch(variants[0], "GET", &[]).status, 200);
    let high = fetch(variants[1], "GET", &[]); assert_eq!(high.status, 200); play_first_hls_segment(&high.body);
    wait_for(|| count(&fixture, "/high.ts") == 1);
    assert_eq!(count(&fixture, "/low.m3u8"), 1);
    assert_eq!(count(&fixture, "/low.ts"), 0);
}

#[test]
fn episode_cache_serves_overlapping_ranges_from_verified_chunks() {
    let fixture = Fixture::new(|request, _| {
        let range = request.headers.get("range").map(String::as_str).unwrap_or("");
        let (start, end) = if range == "bytes=0-9" { (0,9) } else { (10,19) };
        let mut reply = Reply::body((start..=end).map(|v| v as u8).collect::<Vec<_>>(), "video/mp4")
            .header("Cache-Control", "max-age=3600").header("ETag", "\"same-file\"")
            .header("Content-Range", format!("bytes {start}-{end}/20"));
        reply.status = 206; reply
    });
    let temp = tempfile::tempdir().unwrap();
    let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let resolution = register(&mut store, link(fixture.url("/ranges.mp4")), vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-ranges".into()).unwrap();
    assert_eq!(fetch(media(&view), "GET", &[("Range", "bytes=0-9")]).status, 206);
    assert_eq!(fetch(media(&view), "GET", &[("Range", "bytes=10-19")]).status, 206);
    let response = fetch(media(&view), "GET", &[("Range", "bytes=2-16")]);
    assert_eq!(response.status, 206); assert_eq!(response.body, (2..=16).collect::<Vec<u8>>());
    assert_eq!(response.headers.get("content-range").unwrap(), "bytes 2-16/20");
    assert_eq!(count(&fixture, "/ranges.mp4"), 2);
}

#[test]
fn progressive_prefetch_caches_complete_verified_file_and_serves_arbitrary_range() {
    const SIZE: usize = 4 * 1024 * 1024 + 512 * 1024;
    let fixture = Fixture::new(|request, _| {
        let range = request.headers.get("range").expect("prefetch uses ranges").strip_prefix("bytes=").unwrap();
        let (first,last) = range.split_once('-').unwrap(); let start = first.parse::<usize>().unwrap();
        let end = last.parse::<usize>().unwrap().min(SIZE-1);
        let body: Vec<u8> = (start..=end).map(|index| (index % 251) as u8).collect();
        let mut reply = Reply::body(body, "video/mp4").header("ETag", "\"stable-video\"").header("Content-Range", format!("bytes {start}-{end}/{SIZE}"));
        reply.status = 206; reply
    });
    let temp = tempfile::tempdir().unwrap();
    let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let resolution = register(&mut store, link(fixture.url("/movie-progressive.mp4")), vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "progressive-episode".into()).unwrap();
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    wait_for(|| {
        let data: Value = serde_json::from_slice(&fetch(url.as_str(), "GET", &[]).body).unwrap();
        if data["status"] == "failed" { panic!("prefetch failed: {data}"); }
        data["status"] == "complete"
    });
    let status: Value = serde_json::from_slice(&fetch(url.as_str(), "GET", &[]).body).unwrap();
    assert_eq!(status["unit"], "bytes"); assert_eq!(status["total_ms"], SIZE);
    assert_eq!(status["ranges"], json!([{"start_ms":0,"end_ms":SIZE}]));
    let before = count(&fixture, "/movie-progressive.mp4");
    let response = fetch(media(&view), "GET", &[("Range", "bytes=4194300-4194310")]);
    assert_eq!(response.status, 206);
    assert_eq!(response.body, (4194300..=4194310).map(|index| (index % 251) as u8).collect::<Vec<_>>());
    assert_eq!(count(&fixture, "/movie-progressive.mp4"), before);
}

#[test]
fn progressive_prefetch_preserves_chunks_after_exit_and_reuses_them() {
    const SIZE: usize = 4 * 1024 * 1024 + 1024;
    let fixture = Fixture::new(|request, _| {
        let range = request.headers.get("range").expect("prefetch range").strip_prefix("bytes=").unwrap();
        let (first,last) = range.split_once('-').unwrap(); let start = first.parse::<usize>().unwrap();
        let end = last.parse::<usize>().unwrap().min(SIZE-1);
        let mut reply = Reply::body(vec![7; end-start+1], "video/mp4").header("ETag", "\"stable-video\"").header("Content-Range", format!("bytes {start}-{end}/{SIZE}"));
        if start > 0 { reply.body_delay = Duration::from_secs(2); }
        reply.status = 206; reply
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache.clone()).unwrap();
    let mut store = ResolutionStore::default(); let resolution = register(&mut store, link(fixture.url("/resume.mp4")), vec![], 0);
    let mut selected = || store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let first = gateway.open_episode(selected(), "resume-episode".into()).unwrap();
    wait_for(|| cache.status()["entry_count"] == 1 && count(&fixture, "/resume.mp4") >= 2);
    gateway.close(first["session_id"].as_str().unwrap());
    let second = gateway.open_episode(selected(), "resume-episode".into()).unwrap();
    let mut url = url::Url::parse(media(&second)).unwrap(); url.set_path(&format!("/cache/{}", second["session_id"].as_str().unwrap()));
    wait_for(|| {
        let data: Value = serde_json::from_slice(&fetch(url.as_str(), "GET", &[]).body).unwrap();
        if data["status"] == "failed" { panic!("prefetch failed: {data}"); }
        data["status"] == "complete"
    });
    let requests = fixture.state.requests.lock().unwrap();
    assert_eq!(requests.iter().filter(|r| r.headers.get("range").map(String::as_str) == Some("bytes=0-4194303")).count(), 1);
    assert_eq!(cache.status()["entry_count"], 2);
}

#[test]
fn hls_prefetch_includes_the_selected_separate_audio_track() {
    let fixture = Fixture::new(|request, _| match request.target.as_str() {
        "/master-audio.m3u8" => Reply::body("#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"sound\",NAME=\"main\",DEFAULT=YES,AUTOSELECT=YES,URI=\"/audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=900000,AUDIO=\"sound\"\n/video.m3u8\n", "application/vnd.apple.mpegurl"),
        "/video.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/video.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        "/audio.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/audio.aac\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        _ => Reply::body(vec![9; 128], "video/mp2t"),
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/master-audio.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-audio".into()).unwrap();
    let body = String::from_utf8(fetch(media(&view), "GET", &[]).body).unwrap();
    let video = body.lines().find(|line| line.starts_with("http://")).unwrap();
    let audio = body.lines().find(|line| line.starts_with("#EXT-X-MEDIA:")).unwrap().split("URI=\"").nth(1).unwrap().split('"').next().unwrap();
    let video = fetch(video, "GET", &[]); assert_eq!(video.status, 200); play_first_hls_segment(&video.body);
    let audio = fetch(audio, "GET", &[]); assert_eq!(audio.status, 200); play_first_hls_segment(&audio.body);
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    wait_for(|| {
        let data: Value = serde_json::from_slice(&fetch(url.as_str(), "GET", &[]).body).unwrap();
        if data["status"] == "failed" { panic!("prefetch failed: {data}"); }
        data["status"] == "complete"
    });
    assert_eq!(count(&fixture, "/video.ts"), 1); assert_eq!(count(&fixture, "/audio.aac"), 1);
    let data: Value = serde_json::from_slice(&fetch(url.as_str(), "GET", &[]).body).unwrap();
    assert_eq!(data["ranges"], json!([{"start_ms":0,"end_ms":4000}]));
}

#[test]
fn cache_progress_poll_keeps_paused_playback_alive_only_with_valid_capability() {
    let fixture = Fixture::new(|_, _| Reply::body("#EXTM3U\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"));
    let temp = tempfile::tempdir().unwrap(); let clock = Arc::new(TestClock::default());
    let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(clock.clone(), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/paused.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-paused".into()).unwrap();
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    clock.0.store(SESSION_IDLE_MS - 1, Ordering::SeqCst);
    assert_eq!(fetch(url.as_str(), "GET", &[]).status, 200);
    clock.0.store(SESSION_IDLE_MS + 1, Ordering::SeqCst);
    assert_eq!(fetch(url.as_str(), "GET", &[]).status, 200);
    let mut invalid = url.clone(); invalid.set_query(Some(&format!("cap={}", "x".repeat(43))));
    assert_eq!(fetch(invalid.as_str(), "GET", &[]).status, 401);
    clock.0.store(2 * SESSION_IDLE_MS + 2, Ordering::SeqCst);
    assert_eq!(fetch(url.as_str(), "GET", &[]).status, 401);
}

#[test]
fn encrypted_hls_prefetch_keeps_keys_in_session_memory_only() {
    let fixture = Fixture::new(|request, _| match request.target.as_str() {
        "/encrypted.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-KEY:METHOD=AES-128,URI=\"/secret.key\"\n#EXTINF:4,\n/encrypted.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        "/secret.key" => Reply::body(vec![42; 16], "application/octet-stream"),
        _ => Reply::body(vec![7; 128], "video/mp2t"),
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache.clone()).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/encrypted.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-encrypted".into()).unwrap();
    let body = String::from_utf8(fetch(media(&view), "GET", &[]).body).unwrap();
    let key = body.lines().find(|line| line.starts_with("#EXT-X-KEY:")).unwrap().split("URI=\"").nth(1).unwrap().split('"').next().unwrap();
    play_first_hls_segment(body.as_bytes());
    wait_for(|| count(&fixture, "/secret.key") == 1 && count(&fixture, "/encrypted.ts") == 1 && cache.status()["entry_count"] == 1);
    assert_eq!(fetch(key, "GET", &[]).body, vec![42; 16]);
    assert_eq!(count(&fixture, "/secret.key"), 1);
    assert_eq!(cache.status()["entry_count"], 1, "only media is persisted");
}

#[test]
fn progressive_prefetch_rejects_changed_file_validators() {
    const SIZE: usize = 4 * 1024 * 1024 + 1024;
    let fixture = Fixture::new(|request, _| {
        let range = request.headers.get("range").unwrap().strip_prefix("bytes=").unwrap();
        let (first,last) = range.split_once('-').unwrap(); let start = first.parse::<usize>().unwrap();
        let end = last.parse::<usize>().unwrap().min(SIZE-1);
        let validator = if start == 0 { "\"version-one\"" } else { "\"version-two\"" };
        let mut reply = Reply::body(vec![7; end-start+1], "video/mp4").header("ETag", validator).header("Content-Range", format!("bytes {start}-{end}/{SIZE}")); reply.status = 206; reply
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let resolution = register(&mut store, link(fixture.url("/changed.mp4")), vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "changed-episode".into()).unwrap();
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    wait_for(|| serde_json::from_slice::<Value>(&fetch(url.as_str(), "GET", &[]).body).unwrap()["status"] == "failed");
    let data: Value = serde_json::from_slice(&fetch(url.as_str(), "GET", &[]).body).unwrap();
    assert!(data["error"].as_str().unwrap().contains("变化"));
    assert_eq!(data["ranges"], json!([{"start_ms":0,"end_ms":4194304}]));
}

#[test]
fn prefetch_status_allows_only_expected_browser_headers() {
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let fixture = Fixture::new(|_, _| Reply::body("#EXTM3U\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"));
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/preflight.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-preflight".into()).unwrap();
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    let response = fetch(url.as_str(), "OPTIONS", &[("Origin", "tauri://localhost"), ("Access-Control-Request-Method", "GET"), ("Access-Control-Request-Headers", "x-moc-position-ms,x-moc-duration-ms,x-moc-playback-state")]);
    assert_eq!(response.status, 204);
    let allowed = response.headers.get("access-control-allow-headers").unwrap().to_ascii_lowercase();
    assert!(allowed.contains("x-moc-duration-ms")); assert!(allowed.contains("x-moc-playback-state"));
}

#[test]
fn hls_prefetch_retries_transient_errors_and_finishes() {
    let attempts = Arc::new(AtomicUsize::new(0)); let requests = attempts.clone();
    let fixture = Fixture::new(move |request, _| match request.target.as_str() {
        "/retry.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/warm.ts\n#EXTINF:4,\n/retry.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        "/retry.ts" if requests.fetch_add(1, Ordering::SeqCst) == 0 => { let mut reply = Reply::body("unavailable", "text/plain"); reply.status = 503; reply },
        _ => Reply::body(vec![7; 128], "video/mp2t"),
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/retry.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-retry".into()).unwrap(); let manifest = fetch(media(&view), "GET", &[]); assert_eq!(manifest.status, 200); play_first_hls_segment(&manifest.body);
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    wait_for(|| serde_json::from_slice::<Value>(&fetch(url.as_str(), "GET", &[]).body).unwrap()["status"] == "retrying");
    wait_for(|| serde_json::from_slice::<Value>(&fetch(url.as_str(), "GET", &[]).body).unwrap()["status"] == "complete");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[test]
fn hls_prefetch_cancels_network_retry_when_user_leaves() {
    let fixture = Fixture::new(|request, _| {
        if request.target == "/retry-stop.m3u8" { Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/warm-stop.ts\n#EXTINF:4,\n/retry-stop.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl") }
        else if request.target == "/warm-stop.ts" { Reply::body(vec![7; 128], "video/mp2t") }
        else { let mut reply = Reply::body("unavailable", "text/plain"); reply.status = 503; reply }
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/retry-stop.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-stop-retry".into()).unwrap(); let manifest = fetch(media(&view), "GET", &[]); assert_eq!(manifest.status, 200); play_first_hls_segment(&manifest.body);
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    wait_for(|| serde_json::from_slice::<Value>(&fetch(url.as_str(), "GET", &[]).body).unwrap()["status"] == "retrying");
    gateway.close(view["session_id"].as_str().unwrap());
    thread::sleep(Duration::from_millis(1300));
    assert_eq!(count(&fixture, "/retry-stop.ts"), 1);
}

#[test]
fn hls_prefetch_prioritizes_segments_after_the_current_position() {
    let fixture = Fixture::new(|request, _| match request.target.as_str() {
        "/seek.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/seek-0.ts\n#EXTINF:4,\n/seek-1.ts\n#EXTINF:4,\n/seek-2.ts\n#EXTINF:4,\n/seek-3.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        "/seek-0.ts" => { let mut reply = Reply::body(vec![7; 128], "video/mp2t"); reply.body_delay = Duration::from_millis(500); reply },
        _ => Reply::body(vec![7; 128], "video/mp2t"),
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/seek.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "seek-episode".into()).unwrap(); let manifest = fetch(media(&view), "GET", &[]); assert_eq!(manifest.status, 200);
    let text = String::from_utf8(manifest.body).unwrap(); let first = text.lines().find(|line| line.starts_with("http://")).unwrap().to_owned();
    let playing = thread::spawn(move || fetch(&first, "GET", &[]));
    wait_for(|| count(&fixture, "/seek-0.ts") == 1);
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    assert_eq!(fetch(url.as_str(), "GET", &[("X-Moc-Position-Ms", "8000")]).status, 200);
    assert_eq!(playing.join().unwrap().status, 200);
    wait_for(|| count(&fixture, "/seek-2.ts") == 1);
    let requests = fixture.state.requests.lock().unwrap();
    let segments: Vec<_> = requests.iter().filter(|row| row.target.starts_with("/seek-")).map(|row| row.target.as_str()).collect();
    assert_eq!(&segments[..2], &["/seek-0.ts", "/seek-2.ts"]);
}

#[test]
fn hls_shared_initial_segment_does_not_prefetch_an_unselected_variant() {
    let fixture = Fixture::new(|request, _| match request.target.as_str() {
        "/shared-master.m3u8" => Reply::body("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100000\n/shared-low.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=900000\n/shared-high.m3u8\n", "application/vnd.apple.mpegurl"),
        "/shared-low.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/shared-segment.ts\n#EXTINF:4,\n/only-low.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        "/shared-high.m3u8" => Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/shared-segment.ts\n#EXTINF:4,\n/only-high.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl"),
        _ => Reply::body(vec![7; 128], "video/mp2t"),
    });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/shared-master.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-shared-quality".into()).unwrap();
    let master = String::from_utf8(fetch(media(&view), "GET", &[]).body).unwrap();
    let variants: Vec<_> = master.lines().filter(|line| line.starts_with("http://")).collect();
    let low = fetch(variants[0], "GET", &[]); let high = fetch(variants[1], "GET", &[]);
    assert_eq!(low.status, 200); assert_eq!(high.status, 200);
    let high = String::from_utf8(high.body).unwrap(); let segments: Vec<_> = high.lines().filter(|line| line.starts_with("http://")).collect();
    assert_eq!(fetch(segments[0], "GET", &[]).status, 200);
    thread::sleep(Duration::from_millis(150));
    assert_eq!(count(&fixture, "/only-low.ts"), 0); assert_eq!(count(&fixture, "/only-high.ts"), 0);
    assert_eq!(fetch(segments[1], "GET", &[]).status, 200);
    wait_for(|| count(&fixture, "/only-high.ts") == 1);
    assert_eq!(count(&fixture, "/only-low.ts"), 0);
}

#[test]
fn paused_hls_begins_prefetch_even_before_the_first_segment() {
    let fixture = Fixture::new(|request, _| if request.target == "/paused-first.m3u8" {
        Reply::body("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n/paused-first.ts\n#EXT-X-ENDLIST\n", "application/vnd.apple.mpegurl")
    } else { Reply::body(vec![7; 128], "video/mp2t") });
    let temp = tempfile::tempdir().unwrap(); let cache = MediaCache::for_episodes(temp.path().join("media-cache"));
    let gateway = Gateway::with_media_cache(Arc::new(MonotonicClock::default()), cache).unwrap();
    let mut store = ResolutionStore::default(); let mut stream = link(fixture.url("/paused-first.m3u8")); stream.kind = "hls".into();
    let resolution = register(&mut store, stream, vec![], 0);
    let selected = store.selected(resolution["id"].as_str().unwrap(), resolution["streams"][0]["id"].as_str().unwrap(), 0).unwrap();
    let view = gateway.open_episode(selected, "episode-paused-first".into()).unwrap(); assert_eq!(fetch(media(&view), "GET", &[]).status, 200);
    assert_eq!(count(&fixture, "/paused-first.ts"), 0);
    let mut url = url::Url::parse(media(&view)).unwrap(); url.set_path(&format!("/cache/{}", view["session_id"].as_str().unwrap()));
    assert_eq!(fetch(url.as_str(), "GET", &[("X-Moc-Playback-State", "paused")]).status, 200);
    wait_for(|| serde_json::from_slice::<Value>(&fetch(url.as_str(), "GET", &[]).body).unwrap()["status"] == "complete");
    assert_eq!(count(&fixture, "/paused-first.ts"), 1);
}
