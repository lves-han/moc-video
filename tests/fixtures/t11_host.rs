//! A synthetic native callback supplies tracks to the actual Host open command.
//! The optional short-lived bridge lets the packaged WKWebView consume its DTO.
use super::*;
use crate::resolution::{self, HttpContext, Link, Subtitle};

fn candidates(host: &Host, source: &Value, fixture: &Fixture, episode: &str) -> Value {
    let reference = json!({"source_id":source["id"],"source_revision":source["source_revision"],"data":episode});
    let callback_job = job(host, "resolve");
    let (links, subtitles) = resolution::load_links(
        &reference,
        &callback_job,
        || Ok(()),
        |data, casting, cb, _| {
            assert_eq!(data, episode);
            assert!(!casting);
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("x-native-context", "original".parse().unwrap());
            cb.link(Link {
                original: json!({"episode":episode}),
                provider: source["id"].as_str().unwrap().into(),
                url: fixture.url(&format!("/movie/{episode}.mp4")),
                headers,
                referer: "https://provider.invalid/context".into(),
                extractor_data: None,
                interceptor: None,
                cookies: Some(Arc::new(HttpContext(Box::new(|url, headers| {
                    assert_eq!(url.host_str(), Some("maccms.fixture.test"));
                    headers.insert("cookie", "fixture=context".parse().unwrap());
                    Ok(())
                })))),
                label: "native stream".into(),
                quality: None,
                quality_label: None,
                kind: "progressive".into(),
                mime_type: Some("video/mp4".into()),
                playable: true,
                reason: None,
            })?;
            for format in ["vtt", "srt", "ass"] {
                cb.subtitle(Subtitle {
                    original: json!({"episode":episode,"format":format}),
                    url: fixture.url(&format!(
                        "/caption/{episode}.{format}?signed=A%2FB+x&cap=upstream"
                    )),
                    headers: reqwest::header::HeaderMap::new(),
                    label: format!("{episode} {format}"),
                    language: Some("zh-CN".into()),
                    format: format.into(),
                })?;
            }
            Ok(true)
        },
    )
    .unwrap();
    host.committed
        .lock()
        .unwrap()
        .resolutions
        .register(&reference, links, subtitles, host.clock.now_ms())
        .unwrap()
}

fn wait_file(path: &std::path::Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(150),
            "WebView bridge phase timed out"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn t11_host_open_release_switch_and_live_webview() {
    let bridge = std::env::var_os("T11_BRIDGE_DIR").map(std::path::PathBuf::from);
    let media = std::env::var_os("T11_MEDIA")
        .map(|p| std::fs::read(p).unwrap())
        .unwrap_or_default();
    if bridge.is_some() {
        assert!(
            !media.is_empty(),
            "Live WebView requires the verified T01 media"
        );
    }
    let fixture = Fixture::new(move |r, port| {
        if r.target.starts_with("/movie/") {
            return Reply::raw(media.clone(), "video/mp4");
        }
        if r.target.starts_with("/caption/") {
            assert_eq!(
                r.headers.get("x-native-context").map(String::as_str),
                Some("original")
            );
            assert_eq!(
                r.headers.get("cookie").map(String::as_str),
                Some("fixture=context")
            );
            assert_eq!(
                r.headers.get("referer").map(String::as_str),
                Some("https://provider.invalid/context")
            );
            assert!(r.target.ends_with("?signed=A%2FB+x&cap=upstream"));
            let episode = if r.target.contains("first.") {
                "first"
            } else {
                "second"
            };
            let text = if r.target.contains(".vtt?") {
                format!("WEBVTT\n\n00:00:01.250 --> 00:00:03.750\n{episode} VTT\n第二行\n")
            } else if r.target.contains(".srt?") {
                format!("1\n00:00:01,250 --> 00:00:03,750\n<b>{episode} SRT</b>\n第二行\n")
            } else {
                format!(
                    concat!(
                        "[Script Info]\nWrapStyle: 0\n[Events]\nFormat: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\n",
                        "Dialogue: 0,0:00:01.25,0:00:03.75,Default,,0,0,0,,{{\\b1}}{episode} ASS{{\\b0}}\\N第二行\n",
                        "Dialogue: 0,0:00:04.25,0:00:05.75,Default,,0,0,0,,前{{literal}}中文，2 < 3！{{注释\\b1}}后{{\\b0}}\\N第二行\n",
                        "Dialogue: 0,0:00:06.25,0:00:07.75,Default,,0,0,0,,\\N\\N前\\N\\N\\N后\\N\\N\n",
                        "Dialogue: 0,0:00:08.25,0:00:09.75,Default,,0,0,0,,{{\\q2}}\\n前\\n\\n后\\h尾\\n\n",
                        "Dialogue: 0,0:00:10.25,0:00:11.75,Default,,0,0,0,,\\N前{{\\p1}}m 0 0 l 1 1{{comment}}{{\\p0}}\\N\\N后\\N\n",
                    ),
                    episode = episode
                )
            };
            return Reply::raw(text, "text/plain");
        }
        Reply::json(response(json!([item(port)]), json!([])))
    });
    let (_temp, host) = setup(fixture.url("/api"));
    let provider = sync(&host);
    let first = candidates(&host, &provider, &fixture, "first");
    let old = ok(&host, "open_playback", open_input(&first));
    assert_eq!(old["subtitles"].as_array().unwrap().len(), 3);
    ok(
        &host,
        "release_resolution",
        json!({"resolution_id":first["id"]}),
    );
    assert_eq!(
        call(&host, "open_playback", open_input(&first))["error"]["code"],
        "SESSION_EXPIRED"
    );
    for track in old["subtitles"].as_array().unwrap() {
        assert_eq!(track["format"], "vtt");
        assert_eq!(resource(track["url"].as_str().unwrap()).0, 200);
    }
    let second = candidates(&host, &provider, &fixture, "second");
    let current = ok(&host, "open_playback", open_input(&second));
    ok(
        &host,
        "release_resolution",
        json!({"resolution_id":second["id"]}),
    );
    if let Some(dir) = &bridge {
        std::fs::write(
            dir.join("ready.tmp"),
            serde_json::to_vec(&json!({"first":old,"second":current})).unwrap(),
        )
        .unwrap();
        std::fs::rename(dir.join("ready.tmp"), dir.join("ready.json")).unwrap();
        wait_file(&dir.join("close-first"));
    }
    ok(
        &host,
        "close_playback",
        json!({"session_id":old["session_id"]}),
    );
    assert_eq!(
        resource(old["subtitles"][0]["url"].as_str().unwrap()).0,
        410
    );
    // Old cleanup is repeated after the new view is live.
    ok(
        &host,
        "close_playback",
        json!({"session_id":old["session_id"]}),
    );
    ok(
        &host,
        "release_resolution",
        json!({"resolution_id":first["id"]}),
    );
    assert_eq!(
        resource(current["subtitles"][0]["url"].as_str().unwrap()).0,
        200
    );
    if let Some(dir) = &bridge {
        std::fs::write(dir.join("closed"), b"closed").unwrap();
        wait_file(&dir.join("complete"));
    }
    ok(
        &host,
        "close_playback",
        json!({"session_id":current["session_id"]}),
    );
    assert_eq!(
        resource(current["subtitles"][0]["url"].as_str().unwrap()).0,
        410
    );
    assert_eq!(host.requests.active_count(), 0);
    println!(
        "T11_VECTOR:{}",
        json!({"kind":"host","tracks":3,"released_resolution":true,"old_status":410,"new_status":200,"closed_status":410,"live_webview":bridge.is_some(),"synthetic_callbacks":true})
    );
}
