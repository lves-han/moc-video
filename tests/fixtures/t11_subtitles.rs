//! Real native conversion and authenticated HTTP; original CS execution is N02.
use super::*;

const VTT: &str = "\u{feff}WEBVTT\r\n\r\nNOTE original comment\r\nkept out of cues\r\n\r\ncue-id\r\n00:01.250 --> 00:03.750 line:90%\r\n<v 中文><b>你好</b> &amp; world\r\n第二行\r\n";
const SRT: &str = "1\r\n00:00:01,250 --> 00:00:03,750\r\n<b>你好</b> &amp; world\r\n2 < 3 <literal>\r\n\r\n2\r\n01:02:03,004 --> 01:02:04,005\r\n<i>later</i> &#x4E2D; &quot;quote&quot;\r\n";
const ASS: &str = "[Script Info]\nScriptType: v4.00+\nWrapStyle: 0\n[Events]\nFormat: Layer, End, Start, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nComment: 0,0:00:04.00,0:00:00.00,Default,,0,0,0,,not a cue\nDialogue: 0,0:00:03.75,0:00:01.25,Default,,0,0,0,,{\\b1}你好{\\b0} & world\\N第二行,comma\\nsoft\\hspace {literal} 2 < 3\nDialogue: 0,1:02:04.01,1:02:03.00,Default,,0,0,0,,{\\pos(1,2)\\k20}later{\\p1}m 0 0 l 1 1{\\p0}text{\\q2}\\nnew\n";

fn subtitle(url: String, format: &str) -> Subtitle {
    Subtitle {
        original: json!({"url":url}),
        url,
        headers: HeaderMap::new(),
        label: format!("original {format}"),
        language: Some("zh-CN".into()),
        format: format.into(),
    }
}
fn track(view: &Value, index: usize) -> &str {
    view["subtitles"][index]["url"].as_str().unwrap()
}

#[test]
fn t11_structured_text_timeline_and_independent_vectors() {
    let samples = [
        (
            "vtt",
            VTT,
            "WEBVTT\n\nNOTE original comment\nkept out of cues\n\ncue-id\n00:01.250 --> 00:03.750 line:90%\n<v 中文><b>你好</b> &amp; world\n第二行\n\n",
        ),
        (
            "srt",
            SRT,
            "WEBVTT\n\n00:00:01.250 --> 00:00:03.750\n你好 &amp; world\n2 &lt; 3 &lt;literal&gt;\n\n01:02:03.004 --> 01:02:04.005\nlater 中 \"quote\"\n\n",
        ),
        (
            "ass",
            ASS,
            "WEBVTT\n\n00:00:01.250 --> 00:00:03.750\n你好 &amp; world\n第二行,comma soft\u{a0}space  2 &lt; 3\n\n01:02:03.000 --> 01:02:04.010\nlatertext\nnew\n\n",
        ),
    ];
    for (format, input, expected) in samples {
        let actual =
            String::from_utf8(subtitle::convert(input.as_bytes(), format).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            subtitle::convert(input.as_bytes(), "unknown").unwrap(),
            actual.as_bytes()
        );
        println!(
            "T11_VECTOR:{}",
            json!({"kind":"conversion","format":format,"input":input,"output":actual})
        );
    }
    assert_eq!(
        subtitle::convert(b"WEBVTT\n\n", "vtt").unwrap(),
        b"WEBVTT\n\n"
    );
    let styled = "WEBVTT sample\n\nSTYLE\n::cue { color: lime; }\n\nREGION\nid:lower\nwidth:80%\n\ncue-id\n00:00.000 --> 00:01.000 region:lower\n<c.green>styled</c>\n\n";
    assert_eq!(
        subtitle::convert(styled.as_bytes(), "vtt").unwrap(),
        styled.as_bytes()
    );
    let wrap = ASS.replace("WrapStyle: 0", "WrapStyle: 2");
    assert!(
        String::from_utf8(subtitle::convert(wrap.as_bytes(), "ass").unwrap())
            .unwrap()
            .contains("comma\nsoft")
    );
    let drawings =
        "[Events]\nFormat: Start,End,Text\nDialogue: 0:00:00.00,0:00:01.00,{\\p1}m 0 0 l 1 1\n";
    assert_eq!(
        subtitle::convert(drawings.as_bytes(), "ass").unwrap(),
        b"WEBVTT\n\n"
    );
    let unsorted =
        "2\n00:00:02,000 --> 00:00:04,000\nsecond\n\n1\n00:00:01,000 --> 00:00:03,000\nfirst\n";
    assert_eq!(subtitle::convert(unsorted.as_bytes(), "srt").unwrap(),
        b"WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nfirst\n\n00:00:02.000 --> 00:00:04.000\nsecond\n\n");
    let angles = format!("1\n00:00:00,000 --> 00:00:01,000\n{}", "<".repeat(100_000));
    let actual = String::from_utf8(subtitle::convert(angles.as_bytes(), "srt").unwrap()).unwrap();
    assert_eq!(actual.matches("&lt;").count(), 100_000);
    let braces = format!(
        "[Events]\nFormat: Start,End,Text\nDialogue: 0:00:00.00,0:00:01.00,{}",
        "{".repeat(100_000)
    );
    let actual = String::from_utf8(subtitle::convert(braces.as_bytes(), "ass").unwrap()).unwrap();
    assert_eq!(actual.matches('{').count(), 100_000);
}

#[test]
fn t11_ass_comments_and_empty_lines_preserve_complete_cues() {
    let input = concat!(
        "[Script Info]\nWrapStyle: 0\n[Events]\nFormat: Start,End,Text\n",
        "Dialogue: 0:00:04.25,0:00:05.75,前{literal}中文，2 < 3！{注释\\b1}后{\\b0}\\N第二行\n",
        "Dialogue: 0:00:06.25,0:00:07.75,\\N\\N前\\N\\N\\N后\\N\\N\n",
        "Dialogue: 0:00:08.25,0:00:09.75,{\\q2}\\n前\\n\\n后\\h尾\\n\n",
        "Dialogue: 0:00:10.25,0:00:11.75,\\N前{\\p1}m 0 0 l 1 1{comment}{\\p0}\\N\\N后\\N\n",
    );
    let output = String::from_utf8(subtitle::convert(input.as_bytes(), "ass").unwrap()).unwrap();
    assert_eq!(
        output,
        concat!(
            "WEBVTT\n\n00:00:04.250 --> 00:00:05.750\n前中文，2 &lt; 3！后\n第二行\n\n",
            "00:00:06.250 --> 00:00:07.750\n<c></c>\n<c></c>\n前\n<c></c>\n<c></c>\n后\n<c></c>\n<c></c>\n\n",
            "00:00:08.250 --> 00:00:09.750\n<c></c>\n前\n<c></c>\n后\u{a0}尾\n<c></c>\n\n",
            "00:00:10.250 --> 00:00:11.750\n<c></c>\n前\n<c></c>\n后\n<c></c>\n\n",
        )
    );
    assert_eq!(
        subtitle::convert(input.as_bytes(), "unknown").unwrap(),
        output.as_bytes()
    );
    println!(
        "T11_VECTOR:{}",
        json!({"kind":"ass-boundaries","input":input,"output":output})
    );
}

#[test]
fn t11_malformed_encoding_format_and_bounded_expansion() {
    let bad = [
        ("srt", ""),
        ("srt", "<html>login required</html>"),
        ("srt", "1\n00:00:03,000 --> 00:00:02,000\nbackwards"),
        ("srt", "1\n00:61:03,000 --> 00:62:03,000\ninvalid minutes"),
        ("srt", "1\n00:00:03,00 --> 00:00:04,000\nshort fraction"),
        (
            "srt",
            "1\n18446744073709551616:00:00,000 --> 00:00:04,000\noverflow",
        ),
        ("srt", "1\n00:00:03,000 --> 00:00:04,000"),
        (
            "vtt",
            "WEBVTT\n\n00:03.000 --> 00:04.000\nx\n\n00:02.000 --> 00:03.000\ny",
        ),
        ("vtt", "WEBVTT\n00:00.000 --> 00:01.000\nx"),
        ("vtt", "WEBVTT\n\n00:00.000 --> 00:01.000\nx --> y"),
        (
            "ass",
            "[Events]\nDialogue: 0:00:00.00,0:00:01.00,missing format",
        ),
        (
            "ass",
            "[Events]\nFormat: Start,Text,End\nDialogue: 0:00:00.00,text,0:00:01.00",
        ),
        (
            "ass",
            "[Events]\nFormat: Start,End,Text\nDialogue: 0:00:01.00,0:00:00.00,text",
        ),
        ("vtt", "1\n00:00:00,000 --> 00:00:01,000\nx"),
        ("unknown", "not subtitles"),
        ("vtt", "WEBVTT\n\n\0"),
    ];
    for (format, text) in bad {
        assert_eq!(
            subtitle::convert(text.as_bytes(), format).unwrap_err().code,
            "UPSTREAM_FAILED"
        );
    }
    assert_eq!(
        subtitle::convert(&[0xff, 0xfe], "unknown")
            .unwrap_err()
            .code,
        "UPSTREAM_FAILED"
    );
    assert_eq!(
        subtitle::convert(&vec![b'a'; subtitle::LIMIT + 1], "srt")
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    let expansion = format!(
        "1\n00:00:00,000 --> 00:00:01,000\n{}",
        "&".repeat(subtitle::LIMIT / 4)
    );
    assert_eq!(
        subtitle::convert(expansion.as_bytes(), "srt")
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    println!(
        "T11_VECTOR:{}",
        json!({"kind":"invalid","malformed":bad.len()+1,"input_limit":true,"output_limit":true})
    );
}

#[test]
fn t11_http_auth_queries_transformed_head_and_conditional() {
    let fixture = Fixture::new(|r, port| {
        if r.target.starts_with("/redirect") {
            return Reply::redirect(format!(
                "http://maccms.fixture.test:{port}/captions.srt?sig=A%2FB+x&cap=upstream"
            ));
        }
        let text = if r.target.contains(".vtt") {
            VTT
        } else if r.target.contains(".ass") {
            ASS
        } else {
            SRT
        };
        Reply::body(text, "application/octet-stream")
            .header("ETag", "\"upstream\"")
            .header("Accept-Ranges", "bytes")
    });
    let gateway = Gateway::start().unwrap();
    let mut store = ResolutionStore::default();
    let mut selected = link(fixture.url("/movie.mp4"));
    selected.referer = "https://provider.invalid/original".into();
    selected
        .headers
        .insert("x-source", "source-header".parse().unwrap());
    selected.cookies = Some(Arc::new(HttpContext(Box::new(|url, headers| {
        assert!(url.path().contains("captions") || url.path() == "/redirect");
        headers.insert("cookie", "fixture-session=original".parse().unwrap());
        Ok(())
    }))));
    let subtitles = [
        ("vtt", "/captions.vtt?sig=A%2FB+x"),
        ("srt", "/redirect"),
        ("ass", "/captions.ass"),
    ]
    .into_iter()
    .map(|(format, path)| {
        let mut s = subtitle(fixture.url(path), format);
        s.headers
            .insert("x-caption", "caption-header".parse().unwrap());
        s
    })
    .collect();
    let resolution = register(&mut store, selected, subtitles, 0);
    let view = open(&gateway, &mut store, &resolution);
    for (i, format) in ["vtt", "srt", "ass"].into_iter().enumerate() {
        let dto = &view["subtitles"][i];
        assert_eq!(dto["format"], "vtt");
        assert_eq!(dto["source_format"], format);
        assert_eq!(cap(track(&view, i)), cap(media(&view)));
        let response = fetch(
            track(&view, i),
            "GET",
            &[
                ("Origin", "tauri://localhost"),
                ("Cookie", "browser=must-not-forward"),
                ("Range", "bytes=2-9"),
            ],
        );
        assert_eq!(response.status, 200);
        assert_eq!(response.headers["content-type"], "text/vtt; charset=utf-8");
        assert_eq!(
            response.headers["access-control-allow-origin"],
            "tauri://localhost"
        );
        assert!(!response.headers.contains_key("content-range"));
        assert!(!response.headers.contains_key("accept-ranges"));
        let etag = format!("\"{:x}\"", Sha256::digest(&response.body));
        assert_eq!(response.headers["etag"], etag);
        let head = fetch(track(&view, i), "HEAD", &[]);
        assert_eq!(head.status, 200);
        assert!(head.body.is_empty());
        assert_eq!(
            head.headers["content-length"].parse::<usize>().unwrap(),
            response.body.len()
        );
        assert_eq!(head.headers["etag"], etag);
        assert_eq!(
            fetch(track(&view, i), "GET", &[("If-None-Match", &etag)]).status,
            304
        );
        println!(
            "T11_VECTOR:{}",
            json!({"kind":"http","format":format,"output":String::from_utf8(response.body).unwrap(),"head_length":head.headers["content-length"],"etag":etag,"same_cap":true,"conditional_status":304})
        );
    }
    let requests = fixture.state.requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|r| r.target == "/captions.srt?sig=A%2FB+x&cap=upstream")
    );
    for r in requests.iter() {
        assert_eq!(r.method, "GET");
        assert!(!r.headers.contains_key("range"));
        assert!(!r.headers.contains_key("if-none-match"));
        assert_eq!(r.headers["x-source"], "source-header");
        assert_eq!(r.headers["x-caption"], "caption-header");
        assert_eq!(r.headers["referer"], "https://provider.invalid/original");
        assert_eq!(r.headers["cookie"], "fixture-session=original");
    }
}

#[test]
fn t11_release_expiry_switch_and_cap_revocation() {
    let fixture = Fixture::new(|r, _| {
        if r.target == "/subs" {
            assert_eq!(
                r.headers.get("x-lifetime").map(String::as_str),
                Some("original")
            );
            assert_eq!(
                r.headers.get("cookie").map(String::as_str),
                Some("fixture=retained")
            );
            assert_eq!(
                r.headers.get("referer").map(String::as_str),
                Some("https://provider.invalid/retained")
            );
        }
        Reply::body(SRT, "text/plain")
    });
    let clock = Arc::new(TestClock::default());
    let gateway = Gateway::with_clock(clock.clone()).unwrap();
    let mut store = ResolutionStore::default();
    let mut chosen = link(fixture.url("/movie"));
    chosen
        .headers
        .insert("x-lifetime", "original".parse().unwrap());
    chosen.referer = "https://provider.invalid/retained".into();
    chosen.cookies = Some(Arc::new(HttpContext(Box::new(|_, headers| {
        headers.insert("cookie", "fixture=retained".parse().unwrap());
        Ok(())
    }))));
    let resolution = register(
        &mut store,
        chosen,
        vec![subtitle(fixture.url("/subs"), "srt")],
        0,
    );
    let old = open(&gateway, &mut store, &resolution);
    clock.0.store(299_999, Ordering::SeqCst);
    assert_eq!(fetch(track(&old, 0), "GET", &[]).status, 200);
    clock.0.store(300_001, Ordering::SeqCst);
    assert!(
        store
            .selected(
                resolution["id"].as_str().unwrap(),
                resolution["streams"][0]["id"].as_str().unwrap(),
                300_001
            )
            .is_err()
    );
    store.release(resolution["id"].as_str().unwrap());
    assert_eq!(fetch(track(&old, 0), "GET", &[]).status, 200);
    let next = register(
        &mut store,
        link(fixture.url("/next")),
        vec![subtitle(fixture.url("/next-sub"), "srt")],
        300_001,
    );
    let current = open(&gateway, &mut store, &next);
    gateway.close(id(&old));
    gateway.close(id(&old));
    assert_eq!(fetch(track(&old, 0), "GET", &[]).status, 410);
    assert_eq!(fetch(track(&current, 0), "GET", &[]).status, 200);
    let wrong = track(&current, 0).replace(&cap(track(&current, 0)), &cap(track(&old, 0)));
    assert_eq!(fetch(&wrong, "GET", &[]).status, 401);
    gateway.revoke(&["native-fixture".into()]);
    assert_eq!(fetch(track(&current, 0), "GET", &[]).status, 410);
    fixture.quiet();
    println!(
        "T11_VECTOR:{}",
        json!({"kind":"lifetime","at_ms":300001,"resolution_expired":true,"authenticated_after_expiry":true,"continued_status":200,"old_status":410,"wrong_cap":401,"revoked_status":410,"active_requests":fixture.state.active.load(Ordering::SeqCst)})
    );
}

#[test]
fn t11_cross_origin_and_unsafe_subtitle_targets() {
    let fixture = Fixture::new(|r, port| {
        if r.target == "/cross" {
            Reply::redirect(format!("http://other.fixture.test:{port}/captions.srt"))
        } else if r.target == "/private" {
            Reply::redirect(format!("http://127.0.0.1:{port}/never"))
        } else {
            Reply::body(SRT, "text/plain")
        }
    });
    let gateway = Gateway::start().unwrap();
    let mut store = ResolutionStore::default();
    let mut selected = link(fixture.url("/movie"));
    selected
        .headers
        .insert("authorization", "Bearer fixture-secret".parse().unwrap());
    selected
        .headers
        .insert("cookie", "fixture=private".parse().unwrap());
    selected.referer = "https://provider.invalid/ref".into();
    let cross = format!("http://other.fixture.test:{}/direct.srt", fixture.port);
    let r = register(
        &mut store,
        selected,
        vec![
            subtitle(fixture.url("/cross"), "srt"),
            subtitle(cross, "srt"),
            subtitle(fixture.url("/private"), "srt"),
        ],
        0,
    );
    let view = open(&gateway, &mut store, &r);
    assert_eq!(fetch(track(&view, 0), "GET", &[]).status, 200);
    assert_eq!(fetch(track(&view, 1), "GET", &[]).status, 200);
    let response = fetch(track(&view, 2), "GET", &[]);
    assert_eq!(response.status, 502);
    assert_eq!(
        serde_json::from_slice::<Value>(&response.body).unwrap()["error"]["details"]["reason"],
        "unsafe_target"
    );
    let requests = fixture.state.requests.lock().unwrap();
    for request in requests
        .iter()
        .filter(|r| matches!(r.target.as_str(), "/captions.srt" | "/direct.srt"))
    {
        for key in ["authorization", "cookie", "referer"] {
            assert!(!request.headers.contains_key(key));
        }
    }
    assert!(!requests.iter().any(|r| r.target == "/never"));
    println!(
        "T11_VECTOR:{}",
        json!({"kind":"isolation","cross_origin_stripped":true,"direct_origin_stripped":true,"private_target_contacted":false})
    );
}

#[test]
fn t11_error_diagnostics_limits_recovery_and_drm_types() {
    let broken = Arc::new(AtomicBool::new(true));
    let bad = broken.clone();
    let racing = Arc::new(AtomicUsize::new(0));
    let calls = racing.clone();
    let race_gate = Arc::new(AtomicBool::new(false));
    let pending = race_gate.clone();
    let fixture = Fixture::new(move |r, _| match r.target.as_str() {
        "/race" => {
            let first = calls.fetch_add(1, Ordering::SeqCst) == 0;
            Reply {
                repeat: if first { 2 } else { 1 },
                gate: if first { Some(pending.clone()) } else { None },
                ..Reply::body(if first { "invalid subtitle" } else { SRT }, "text/plain")
            }
        }
        "/bad" => Reply::body(
            if bad.load(Ordering::SeqCst) {
                "bad subtitle"
            } else {
                SRT
            },
            "text/plain",
        ),
        "/huge" => Reply {
            length: Some(subtitle::LIMIT + 1),
            ..Reply::body("", "text/plain")
        },
        "/chunked" => Reply {
            chunked: true,
            repeat: subtitle::LIMIT / 8192 + 1,
            ..Reply::body(vec![b'a'; 8192], "text/plain")
        },
        "/fairplay.m3u8" => Reply::body(
            "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://key\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n",
            "application/vnd.apple.mpegurl",
        ),
        "/widevine.m3u8" => Reply::body(
            "#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES-CTR,URI=\"key\",KEYFORMAT=\"urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed\"\n",
            "application/vnd.apple.mpegurl",
        ),
        _ => Reply::body("movie", "video/mp4"),
    });
    let gateway = Gateway::start().unwrap();
    let mut store = ResolutionStore::default();
    let r = register(
        &mut store,
        link(fixture.url("/movie")),
        ["/bad", "/huge", "/chunked"]
            .into_iter()
            .map(|p| subtitle(fixture.url(p), "srt"))
            .collect(),
        0,
    );
    let view = open(&gateway, &mut store, &r);
    for (i, code) in ["UPSTREAM_FAILED", "RESOURCE_LIMIT", "RESOURCE_LIMIT"]
        .into_iter()
        .enumerate()
    {
        let response = fetch(track(&view, i), "GET", &[]);
        assert_eq!(
            serde_json::from_slice::<Value>(&response.body).unwrap()["error"]["code"],
            code
        );
        assert_eq!(
            state(&gateway, &view)["error"]["details"]["resource_kind"],
            "subtitle"
        );
    }
    // Repair this exact resource; another resource's success preserves its error.
    assert_eq!(fetch(track(&view, 0), "GET", &[]).status, 502);
    assert_eq!(fetch(media(&view), "GET", &[]).status, 200);
    assert_eq!(state(&gateway, &view)["state"], "failed");
    let only_bad = register(
        &mut store,
        link(fixture.url("/movie")),
        vec![subtitle(fixture.url("/bad"), "srt")],
        0,
    );
    let repaired = open(&gateway, &mut store, &only_bad);
    assert_eq!(fetch(track(&repaired, 0), "GET", &[]).status, 502);
    assert_eq!(fetch(media(&repaired), "GET", &[]).status, 200);
    assert_eq!(state(&gateway, &repaired)["state"], "failed");
    broken.store(false, Ordering::SeqCst);
    assert_eq!(fetch(track(&view, 0), "GET", &[]).status, 200);
    assert_eq!(state(&gateway, &view)["state"], "failed");
    assert_eq!(fetch(track(&repaired, 0), "GET", &[]).status, 200);
    assert_eq!(state(&gateway, &repaired)["state"], "ready");
    let r = register(
        &mut store,
        link(fixture.url("/movie")),
        vec![subtitle(fixture.url("/race"), "srt")],
        0,
    );
    let race = open(&gateway, &mut store, &r);
    let old_url = track(&race, 0).to_owned();
    let old = thread::spawn(move || fetch(&old_url, "GET", &[]));
    wait_for(|| racing.load(Ordering::SeqCst) > 0);
    assert_eq!(fetch(track(&race, 0), "GET", &[]).status, 200);
    race_gate.store(true, Ordering::SeqCst);
    assert_eq!(old.join().unwrap().status, 502);
    assert_eq!(state(&gateway, &race)["state"], "ready");
    for (path, expected) in [
        ("/fairplay.m3u8", "fairplay"),
        ("/widevine.m3u8", "widevine"),
    ] {
        let view = session(&gateway, fixture.url(path));
        let response = fetch(media(&view), "GET", &[]);
        let error = serde_json::from_slice::<Value>(&response.body).unwrap()["error"].clone();
        assert_eq!(error["code"], "MEDIA_UNSUPPORTED");
        assert_eq!(error["details"]["drm_type"], expected);
        println!(
            "T11_VECTOR:{}",
            json!({"kind":"drm","type":expected,"error":error})
        );
    }
    fixture.quiet();
}

#[test]
fn t11_pending_subtitle_close_cancels_before_conversion() {
    let fixture = Fixture::new(|_, _| Reply {
        body_delay: Duration::from_secs(5),
        ..Reply::body(SRT, "text/plain")
    });
    let gateway = Gateway::start().unwrap();
    let mut store = ResolutionStore::default();
    let r = register(
        &mut store,
        link(fixture.url("/movie")),
        vec![subtitle(fixture.url("/slow"), "srt")],
        0,
    );
    let view = open(&gateway, &mut store, &r);
    let mut reader = connect(track(&view, 0), "GET", &[]);
    wait_for(|| !fixture.state.requests.lock().unwrap().is_empty());
    gateway.close(id(&view));
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert!(bytes.is_empty());
    fixture.quiet();
    assert_eq!(state(&gateway, &view)["state"], "expired");
    println!(
        "T11_VECTOR:{}",
        json!({"kind":"cancel","late_bytes":bytes.len(),"active_requests":fixture.state.active.load(Ordering::SeqCst)})
    );
}
