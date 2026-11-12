//! T0.40 live lists on the real Host and gateway: M3U/TXT detection, refreshed
//! live playlists, no media cache, no watch history and explicit errors.
use super::*;

const M3U: &str = "6a1d2c3b-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const TXT: &str = "7b2e3d4c-5f6a-4b7c-8d9e-0f1a2b3c4d5e";
const BAD: &str = "8c3f4e5d-6a7b-4c8d-9e0f-1a2b3c4d5e6f";

fn live_fixture() -> Fixture {
    let sequence = Arc::new(AtomicUsize::new(0));
    Fixture::new(move |request, port| {
        let base = format!("http://maccms.fixture.test:{port}");
        match request.target.split('?').next().unwrap() {
            "/list.m3u" => Reply::raw(
                format!(
                    "#EXTM3U\n#EXTINF:-1 tvg-logo=\"{base}/logo.png\" group-title=\"News\",News One\n{base}/live/index.m3u8\n\
                 #EXTINF:-1 group-title=\"News\",Clip\n{base}/clip.mp4\n#EXTINF:-1 group-title=\"Multicast\",Multicast\nrtp://239.0.0.1:5000\n"
                ),
                "audio/x-mpegurl",
            ),
            "/list.txt" => Reply::raw(
                format!("Sports,#genre#\nSport One,{base}/live/index.m3u8\n"),
                "text/plain",
            ),
            "/bad.html" => Reply::raw("<html><body>not a list</body></html>", "text/html"),
            "/live/index.m3u8" => {
                // Each request advances the live window by one segment.
                let n = sequence.fetch_add(1, Ordering::SeqCst);
                Reply::raw(
                    format!(
                        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:{n}\n#EXTINF:2.0,\nseg{n}.ts\n#EXTINF:2.0,\nseg{}.ts\n",
                        n + 1
                    ),
                    "application/vnd.apple.mpegurl",
                )
            }
            path if path.starts_with("/live/seg") => Reply::raw(vec![0x47; 4096], "video/mp2t"),
            "/clip.mp4" => Reply::raw(vec![0x5a; 8192], "video/mp4"),
            _ => Reply {
                status: 404,
                ..Reply::raw("missing", "text/plain")
            },
        }
    })
}

fn live_settings(host: &Host, fixture: &Fixture, configs: &[(&str, &str)]) {
    let mut settings = ok(host, "get_settings", json!({}));
    settings["source_configs"] = json!([]);
    settings["live_configs"] = json!(configs.iter().map(|(id, path)| json!({"config_id": id, "name": path, "url": fixture.url(path), "enabled": true})).collect::<Vec<_>>());
    ok(host, "save_settings", settings);
}

fn upstream(fixture: &Fixture, prefix: &str) -> usize {
    fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.target.starts_with(prefix))
        .count()
}

#[test]
fn x3_live_lists_gateway_refresh_cache_and_history() {
    let fixture = live_fixture();
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    live_settings(
        &host,
        &fixture,
        &[(M3U, "/list.m3u"), (TXT, "/list.txt"), (BAD, "/bad.html")],
    );

    let m3u = ok(&host, "get_live_channels", json!({"config_id": M3U}));
    assert_eq!(m3u["format"], "m3u");
    let names: Vec<_> = m3u["channels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["group"].as_str().unwrap(),
                c["name"].as_str().unwrap(),
                c["supported"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        names,
        vec![
            ("News", "News One", true),
            ("News", "Clip", true),
            ("Multicast", "Multicast", false)
        ]
    );
    assert_eq!(m3u["channels"][0]["logo"], fixture.url("/logo.png"));
    let txt = ok(&host, "get_live_channels", json!({"config_id": TXT}));
    assert_eq!(
        (txt["format"].as_str(), txt["channels"][0]["group"].as_str()),
        (Some("txt"), Some("Sports"))
    );
    error(
        &call(&host, "get_live_channels", json!({"config_id": BAD})),
        "UPSTREAM_FAILED",
    );
    error(
        &call(
            &host,
            "get_live_channels",
            json!({"config_id": "9d4a5f6e-7b8c-4d9e-8f0a-1b2c3d4e5f6a"}),
        ),
        "SOURCE_NOT_FOUND",
    );

    let revision = m3u["revision"].as_str().unwrap();
    error(
        &call(
            &host,
            "open_live",
            json!({"config_id": M3U, "revision": revision, "index": 2}),
        ),
        "MEDIA_UNSUPPORTED",
    );
    error(
        &call(
            &host,
            "open_live",
            json!({"config_id": M3U, "revision": "stale", "index": 0}),
        ),
        "REVISION_MISMATCH",
    );
    error(
        &call(
            &host,
            "open_live",
            json!({"config_id": M3U, "revision": revision, "index": 9}),
        ),
        "BAD_REQUEST",
    );

    let view = ok(
        &host,
        "open_live",
        json!({"config_id": M3U, "revision": revision, "index": 0}),
    );
    assert_eq!(view["stream_kind"], "hls");
    let media = view["media_url"].as_str().unwrap();
    let (status, first) = resource(media);
    assert_eq!(status, 200);
    let (_, second) = resource(media);
    let (first, second) = (
        String::from_utf8(first).unwrap(),
        String::from_utf8(second).unwrap(),
    );
    assert_eq!(
        upstream(&fixture, "/live/index.m3u8"),
        2,
        "every playlist request reaches upstream"
    );
    assert_ne!(first, second, "the refreshed live window is served");
    assert!(
        !second.contains("maccms.fixture.test") && second.contains("/media/"),
        "segments go through the gateway"
    );
    let segment = second.lines().find(|l| l.starts_with("http://")).unwrap();
    for _ in 0..2 {
        assert_eq!(resource(segment).0, 200);
    }
    assert_eq!(
        upstream(&fixture, "/live/seg"),
        2,
        "live segments are not served from the media cache"
    );
    assert_eq!(ok(&host, "get_media_cache", json!({}))["used_bytes"], 0);
    let clip = ok(
        &host,
        "open_live",
        json!({"config_id": M3U, "revision": revision, "index": 1}),
    );
    assert_eq!(clip["stream_kind"], "progressive");
    assert_eq!(resource(clip["media_url"].as_str().unwrap()).0, 200);
    assert_eq!(ok(&host, "get_media_cache", json!({}))["used_bytes"], 0);
    assert_eq!(
        ok(&host, "list_history", json!({}))["entries"],
        json!([]),
        "live playback writes no history"
    );

    ok(
        &host,
        "close_playback",
        json!({"session_id": view["session_id"]}),
    );
    assert_ne!(
        resource(media).0,
        200,
        "a closed live session stops serving"
    );
    // Editing the list drops its channels and stops its remaining stream.
    live_settings(&host, &fixture, &[(TXT, "/list.txt")]);
    assert_ne!(resource(clip["media_url"].as_str().unwrap()).0, 200);
    error(
        &call(
            &host,
            "open_live",
            json!({"config_id": M3U, "revision": revision, "index": 0}),
        ),
        "SOURCE_NOT_FOUND",
    );
    println!(
        "X3_LIVE:{}",
        json!({"formats": [m3u["format"], txt["format"]], "playlist_requests": 2, "segment_requests": 2, "cache_used": 0})
    );
}
