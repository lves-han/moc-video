//! Real native HTTP gateway fixtures. Provider callbacks are synthetic; original
//! Android/Yjys interceptor execution remains the separate N02 gate.
use super::*;
use crate::resolution::HttpManifestContext;
use std::{fs, path::Path, process::Command};

fn hls_link(url: String) -> Link {
    Link {
        kind: "hls".into(),
        mime_type: Some("application/vnd.apple.mpegurl".into()),
        ..link(url)
    }
}
fn hls_session(gateway: &Gateway, url: String) -> Value {
    let mut store = ResolutionStore::default();
    let r = register(
        &mut store,
        hls_link(url),
        vec![],
        gateway.shared.clock.now_ms(),
    );
    open(gateway, &mut store, &r)
}
fn text(response: &Response) -> &str {
    assert_eq!(response.status, 200);
    std::str::from_utf8(&response.body).unwrap()
}
// Independent observation of rendered local references, without the production
// HLS parser. Fixtures have URI values in quotes or whole URI lines.
fn local_uris(body: &str, view: &Value) -> Vec<String> {
    let root = url::Url::parse(media(view)).unwrap();
    let prefix = format!(
        "http://127.0.0.1:{}/media/{}/",
        root.port().unwrap(),
        id(view)
    );
    let mut remaining = body;
    let mut found = Vec::new();
    while let Some(start) = remaining.find("http://127.0.0.1:") {
        remaining = &remaining[start..];
        let end = remaining.find(['"', '\r', '\n']).unwrap_or(remaining.len());
        let uri = &remaining[..end];
        // Avoid assertion Debug printing a capability on failure.
        assert!(uri.starts_with(&prefix), "child gateway/session changed");
        assert!(cap(uri) == cap(media(view)), "child capability changed");
        let parsed = url::Url::parse(uri).unwrap();
        assert_eq!(
            parsed
                .query_pairs()
                .map(|(k, _)| k.into_owned())
                .collect::<Vec<_>>(),
            ["cap"]
        );
        found.push(uri.to_owned());
        remaining = &remaining[end..];
    }
    found
}
fn assert_body_headers(response: &Response) {
    assert_eq!(
        response.headers["content-length"],
        response.body.len().to_string()
    );
    assert_eq!(
        response.headers["content-type"],
        "application/vnd.apple.mpegurl"
    );
    assert_eq!(
        response.headers["etag"],
        format!("\"{:x}\"", Sha256::digest(&response.body))
    );
    for name in [
        "content-encoding",
        "content-range",
        "accept-ranges",
        "last-modified",
        "set-cookie",
    ] {
        assert!(!response.headers.contains_key(name));
    }
}

#[test]
fn t10_structured_independent_uri_and_timeline_fixtures() {
    // Each pair freezes all bytes outside the URI span, including commas inside
    // strings, unknown tags, CRLF, IVs, sequences and implicit byterange offsets.
    let fixtures = [
        (
            "#EXT-X-STREAM-INF:BANDWIDTH=100,CODECS=\"avc1,mp4a\"\r\n../v.m3u8?a=1%2F2+x\r\n",
            vec!["../v.m3u8?a=1%2F2+x"],
        ),
        (
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"中,文\",URI=\"a.m3u8\"\n",
            vec!["a.m3u8"],
        ),
        (
            "#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"s\",NAME=\"S\",URI=\"s.m3u8\"\n",
            vec!["s.m3u8"],
        ),
        (
            "#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=90,URI=\"/i.m3u8\"\n",
            vec!["/i.m3u8"],
        ),
        (
            "#EXT-X-KEY:METHOD=AES-128,URI=\"key?k=A%2FB+z&cap=upstream\",IV=0x00000000000000000000000000000025\n",
            vec!["key?k=A%2FB+z&cap=upstream"],
        ),
        (
            "#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"https://cdn.invalid/key\",KEYFORMAT=\"identity\"\n",
            vec!["https://cdn.invalid/key"],
        ),
        (
            "#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"100@0\"\n",
            vec!["init.mp4"],
        ),
        (
            "#EXT-X-MEDIA-SEQUENCE:37\n#EXT-X-DISCONTINUITY-SEQUENCE:2\n#EXT-X-BYTERANGE:100@100\nseg.mp4\n#EXT-X-DISCONTINUITY\n#EXT-X-BYTERANGE:100\nseg.mp4\n#EXT-X-ENDLIST\n",
            vec!["seg.mp4", "seg.mp4"],
        ),
        (
            "#EXT-X-PART:DURATION=0.5,URI=\"p.m4s\",INDEPENDENT=YES\n#EXT-X-PRELOAD-HINT:TYPE=PART,URI=\"next.m4s\",BYTERANGE-START=9\n#EXT-X-RENDITION-REPORT:URI=\"../a.m3u8\",LAST-MSN=37,LAST-PART=1\n",
            vec!["p.m4s", "next.m4s", "../a.m3u8"],
        ),
        (
            "#EXT-X-FUTURE:NOTE=\"comma,URI=ignored\",URI=\"https://cdn.invalid/a?sig=a%2Fb+z\",X=7\n#EXT-X-CONTENT-STEERING:SERVER-URI=\"steer.json\"\n#EXT-X-FUTURE-SCALAR:abc?x=y\n",
            vec!["https://cdn.invalid/a?sig=a%2Fb+z", "steer.json"],
        ),
        (
            "#EXT-X-KEY:METHOD=NONE\n#comment untouched\n#EXTINF:2,title URI=ignored\n//cdn.invalid/seg.ts\n",
            vec!["//cdn.invalid/seg.ts"],
        ),
    ];
    let gateway = Gateway::start().unwrap();
    let view = hls_session(&gateway, "https://source.invalid/start.m3u8".into());
    let req = Request {
        method: Method::GET,
        path: url::Url::parse(media(&view)).unwrap().path().into(),
        cap: Some(cap(media(&view))),
        headers: HeaderMap::new(),
        origin: None,
    };
    let lease = gateway.shared.acquire(&req).unwrap();
    for (index, (fixture, expected)) in fixtures.iter().enumerate() {
        let original = format!("#EXTM3U\r\n{fixture}");
        let refs = crate::gateway::hls::references(&original).unwrap();
        assert_eq!(
            refs.iter()
                .map(|r| &original[r.span.clone()])
                .collect::<Vec<_>>(),
            *expected
        );
        let base = url::Url::parse("https://source.invalid/final/base.m3u8?sig=parent").unwrap();
        let rewritten = rewrite_manifest(&lease, &base, &original).unwrap();
        let rendered = std::str::from_utf8(&rewritten).unwrap();
        let uris = local_uris(rendered, &view);
        assert_eq!(uris.len(), expected.len());
        let mut restored = rendered.to_owned();
        for (uri, remote) in uris.iter().zip(expected) {
            restored = restored.replacen(uri, remote, 1);
            let resource = url::Url::parse(uri)
                .unwrap()
                .path()
                .rsplit('/')
                .next()
                .unwrap()
                .to_owned();
            let records = gateway.shared.records.lock().unwrap();
            assert!(
                records.sessions[id(&view)].resources[&resource].url
                    == base.join(remote).unwrap().as_str(),
                "relative/signed URI changed"
            );
        }
        assert!(
            restored == original,
            "fixture {index}: non-URI manifest bytes changed"
        );
        println!(
            "T10_VECTOR:{}",
            json!({"kind":"parser","fixture":index,"uri_count":uris.len(),"preserved":restored==original})
        );
    }
}

#[test]
fn t10_invalid_uri_drm_and_atomic_registry_limits() {
    for value in [
        "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=unquoted\n",
        "#EXTM3U\n#EXT-X-MAP:URI=\"unterminated\n",
        "#EXTM3U\n#EXT-X-MAP:URI=\"a\",URI=\"b\"\n",
        "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n",
        "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128\n",
        "#EXTM3U\n{$undefined}/seg.ts\n",
        "#EXTM3U\n#EXT-X-MAP:URI=\"{$missing}\"\n",
        "#EXTM3U\n bad.ts\n",
        "<html>failed</html>",
    ] {
        assert!(crate::gateway::hls::references(value).is_err());
    }
    for value in [
        "METHOD=SAMPLE-AES,URI=\"key\"",
        "METHOD=AES-128,KEYFORMAT=\"com.widevine\",URI=\"key\"",
    ] {
        assert_eq!(
            crate::gateway::hls::references(&format!("#EXTM3U\n#EXT-X-KEY:{value}\n"))
                .err()
                .unwrap()
                .code,
            "MEDIA_UNSUPPORTED"
        );
    }
    let fixture = Fixture::new(|r, _| match r.target.as_str() {
        "/oversize.m3u8" => Reply {
            length: Some(hls::LIMIT + 1),
            ..Reply::body("#EXTM3U\n", "application/vnd.apple.mpegurl")
        },
        "/chunked.m3u8" => Reply {
            chunked: true,
            repeat: 129,
            ..Reply::body(vec![b'#'; 65536], "application/vnd.apple.mpegurl")
        },
        "/invalid.m3u8" => Reply::body(
            "#EXTM3U\nok.ts\n#EXT-X-MAP:URI=\"broken\n",
            "application/vnd.apple.mpegurl",
        ),
        "/scheme.m3u8" => Reply::body(
            "#EXTM3U\nok.ts\nfile:///secret\n",
            "application/vnd.apple.mpegurl",
        ),
        "/expanded.m3u8" => Reply::body(
            format!("#EXTM3U\n{}", "x\n".repeat(60_000)),
            "application/vnd.apple.mpegurl",
        ),
        "/unsupported.m3u8" => {
            Reply::body("encoded", "application/vnd.apple.mpegurl").header("Content-Encoding", "br")
        }
        "/drm.m3u8" => Reply::body(
            "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"key\"\n",
            "application/vnd.apple.mpegurl",
        ),
        _ => panic!("unexpected upstream resource"),
    });
    let gateway = Gateway::start().unwrap();
    for (path, status, code) in [
        ("/oversize.m3u8", 413, "RESOURCE_LIMIT"),
        ("/chunked.m3u8", 413, "RESOURCE_LIMIT"),
        ("/invalid.m3u8", 502, "UPSTREAM_FAILED"),
        ("/scheme.m3u8", 502, "UPSTREAM_FAILED"),
        ("/expanded.m3u8", 413, "RESOURCE_LIMIT"),
        ("/unsupported.m3u8", 502, "UPSTREAM_FAILED"),
        ("/drm.m3u8", 502, "MEDIA_UNSUPPORTED"),
    ] {
        let view = hls_session(&gateway, fixture.url(path));
        let response = fetch(media(&view), "GET", &[]);
        assert_eq!(response.status, status);
        assert_eq!(
            serde_json::from_slice::<Value>(&response.body).unwrap()["error"]["code"],
            code
        );
        assert_eq!(
            gateway.shared.records.lock().unwrap().sessions[id(&view)]
                .resources
                .len(),
            1
        );
        assert_eq!(state(&gateway, &view)["state"], "failed");
    }
}

#[test]
fn t10_http_master_tracks_keys_map_and_unknown_tags() {
    let fixture = Fixture::new(|r, port| match r.target.as_str() {
        "/entry/start.m3u8" => Reply::redirect("/final/master.m3u8?sig=redirect".into()),
        "/final/master.m3u8?sig=redirect" => Reply::body(
            format!(
                "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"中,文\",URI=\"audio/list.m3u8?sig=a%2Fb+x\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"s\",NAME=\"S\",URI=\"/subs/list.m3u8\"\n#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=10,URI=\"iframe.m3u8\"\n#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"key?cap=upstream&sig=a%2Fb+x\"\n#EXT-X-STREAM-INF:BANDWIDTH=100,AUDIO=\"a\",SUBTITLES=\"s\"\nhttp://maccms.fixture.test:{port}/final/video.m3u8?sig=v%2Fz+x\n"
            ),
            "application/vnd.apple.mpegurl",
        ),
        "/final/audio/list.m3u8?sig=a%2Fb+x" => Reply::body(
            "#EXTM3U\n#EXTINF:2,\n../audio.aac?sig=a%2Fb+x\n#EXT-X-ENDLIST\n",
            "audio/mpegurl",
        ),
        "/subs/list.m3u8" => Reply::body(
            "#EXTM3U\n#EXTINF:2,\ncaption.vtt\n#EXT-X-ENDLIST\n",
            "application/x-mpegurl",
        ),
        "/final/iframe.m3u8" => Reply::body(
            "#EXTM3U\n#EXT-X-I-FRAMES-ONLY\n#EXTINF:2,\niframe.ts\n#EXT-X-ENDLIST\n",
            "application/vnd.apple.mpegurl",
        ),
        "/final/video.m3u8?sig=v%2Fz+x" => Reply::body(
            "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:37\n#EXT-X-DISCONTINUITY-SEQUENCE:2\n#EXT-X-KEY:METHOD=AES-128,URI=\"key?cap=upstream&sig=a%2Fb+x\",IV=0x00000000000000000000000000000025\n#EXT-X-MAP:URI=\"blob.mp4?sig=range\",BYTERANGE=\"4@0\"\n#EXT-X-BYTERANGE:4@4\n#EXTINF:1,\nblob.mp4?sig=range\n#EXT-X-DISCONTINUITY\n#EXT-X-BYTERANGE:4\n#EXTINF:1,\nblob.mp4?sig=range\n#EXT-X-PART:DURATION=0.5,URI=\"part.m4s\"\n#EXT-X-PRELOAD-HINT:TYPE=PART,URI=\"next.m4s\"\n#EXT-X-RENDITION-REPORT:URI=\"audio/list.m3u8?sig=a%2Fb+x\",LAST-MSN=37\n#EXT-X-FUTURE:FOO=\"comma,URI=ignored\",URI=\"future.bin\",X=7\n#EXT-X-ENDLIST\n",
            "application/vnd.apple.mpegurl",
        ),
        "/final/blob.mp4?sig=range" => {
            let body = b"INITSEG1SEG2";
            if let Some(range) = r.headers.get("range") {
                let (a, b) = range
                    .strip_prefix("bytes=")
                    .unwrap()
                    .split_once('-')
                    .unwrap();
                let (a, b): (usize, usize) = (a.parse().unwrap(), b.parse().unwrap());
                Reply {
                    status: 206,
                    ..Reply::body(&body[a..=b], "video/mp4")
                        .header("Content-Range", format!("bytes {a}-{b}/12"))
                }
            } else {
                Reply::body(body.as_slice(), "video/mp4")
            }
        }
        "/subs/caption.vtt" => Reply::body("WEBVTT\n\n00:00.000 --> 00:02.000\n字幕\n", "text/vtt"),
        value if value.starts_with("/final/key?") => {
            Reply::body(vec![0x25; 16], "application/octet-stream")
        }
        "/final/audio.aac?sig=a%2Fb+x"
        | "/final/iframe.ts"
        | "/final/part.m4s"
        | "/final/next.m4s"
        | "/final/future.bin" => Reply::body(vec![3; 128], "application/octet-stream"),
        _ => panic!("unregistered fixture target"),
    });
    let gateway = Gateway::start().unwrap();
    let mut source = hls_link(fixture.url("/entry/start.m3u8"));
    source
        .headers
        .insert("authorization", "Provider credential".parse().unwrap());
    source.referer = "https://provider.invalid/detail".into();
    source.interceptor = Some(Arc::new(HttpContext(Box::new(|_, h| {
        h.insert("x-original", "source-instance".parse().unwrap());
        Ok(())
    }))));
    source.cookies = Some(Arc::new(HttpContext(Box::new(|url, h| {
        assert_eq!(url.host_str(), Some("maccms.fixture.test"));
        h.insert("cookie", "provider=original".parse().unwrap());
        Ok(())
    }))));
    let mut store = ResolutionStore::default();
    let r = register(&mut store, source, vec![], 0);
    let view = open(&gateway, &mut store, &r);
    let master = fetch(
        media(&view),
        "GET",
        &[
            ("Authorization", "Bearer browser-must-terminate"),
            ("Cookie", "browser=must-terminate"),
        ],
    );
    assert_body_headers(&master);
    assert!(!text(&master).contains("maccms.fixture.test"));
    let children = local_uris(text(&master), &view);
    assert_eq!(children.len(), 5);
    let mut all = children.clone();
    for child in &children {
        let response = fetch(child, "GET", &[]);
        assert_eq!(response.status, 200);
        if response.headers["content-type"].contains("mpegurl") {
            assert_body_headers(&response);
            let uris = local_uris(text(&response), &view);
            if text(&response).contains("#EXT-X-BYTERANGE") {
                assert!(
                    uris[1] == uris[2] && uris[2] == uris[3],
                    "map/implicit byterange identity changed"
                );
                for (range, body) in [
                    ("bytes=0-3", b"INIT"),
                    ("bytes=4-7", b"SEG1"),
                    ("bytes=8-11", b"SEG2"),
                ] {
                    let partial = fetch(&uris[1], "GET", &[("Range", range)]);
                    assert_eq!(partial.status, 206);
                    assert_eq!(partial.body, body);
                }
                assert!(text(&response).contains("IV=0x00000000000000000000000000000025"));
                assert!(text(&response).contains("#EXT-X-BYTERANGE:4\n"));
                assert!(text(&response).contains("#EXT-X-FUTURE:FOO=\"comma,URI=ignored\",URI=\""));
            }
            for uri in &uris {
                let response = fetch(uri, "GET", &[]);
                assert_eq!(response.status, 200);
                if response.headers["content-type"].contains("mpegurl") {
                    for sub in local_uris(text(&response), &view) {
                        assert_eq!(fetch(&sub, "GET", &[]).status, 200);
                    }
                }
            }
            all.extend(uris);
        } else {
            assert_eq!(response.body, vec![0x25; 16]);
        }
    }
    let repeated = fetch(media(&view), "GET", &[]);
    assert!(
        repeated.body == master.body,
        "refresh changed resource identity"
    );
    let registry_count = gateway.shared.records.lock().unwrap().sessions[id(&view)]
        .resources
        .len();
    let requests = fixture.state.requests.lock().unwrap();
    for request in requests.iter() {
        assert_eq!(request.headers["x-original"], "source-instance");
        assert_eq!(request.headers["cookie"], "provider=original");
        assert_eq!(request.headers["authorization"], "Provider credential");
        assert_eq!(
            request.headers["referer"],
            "https://provider.invalid/detail"
        );
        let observed = format!("{} {:?}", request.target, request.headers);
        assert!(
            !observed.contains(&cap(media(&view))) && !observed.contains(&gateway.shared.token)
        );
        assert!(!observed.contains("browser-must-terminate"));
    }
    let targets: Vec<_> = requests.iter().map(|r| r.target.clone()).collect();
    for required in [
        "/final/master.m3u8?sig=redirect",
        "/final/key?cap=upstream&sig=a%2Fb+x",
        "/final/blob.mp4?sig=range",
        "/subs/caption.vtt",
        "/final/audio.aac?sig=a%2Fb+x",
        "/final/future.bin",
    ] {
        assert!(targets.iter().any(|t| t == required));
    }
    println!(
        "T10_VECTOR:{}",
        json!({"kind":"http","requests":targets,"references":all.len(),"same_cap":true,"registry_count":registry_count,"refreshed_stable":repeated.body==master.body})
    );
}

#[test]
fn t10_cross_origin_private_key_and_closed_cap() {
    let fixture = Fixture::new(|r, port| match r.target.as_str() {
        "/master.m3u8" => Reply::body(
            format!(
                "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"http://127.0.0.1:{port}/private-key\"\n#EXT-X-STREAM-INF:BANDWIDTH=1\nhttp://other.fixture.test:{port}/child.m3u8\n"
            ),
            "application/vnd.apple.mpegurl",
        ),
        "/child.m3u8" => Reply::body("#EXTM3U\nsegment.ts\n", "application/vnd.apple.mpegurl"),
        "/segment.ts" => Reply::body(vec![4; 128], "video/mp2t"),
        _ => panic!("unsafe target reached fixture"),
    });
    let gateway = Gateway::start().unwrap();
    let mut source = hls_link(fixture.url("/master.m3u8"));
    for name in ["cookie", "authorization", "x-secret"] {
        source.headers.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            "source-secret".parse().unwrap(),
        );
    }
    source.referer = "https://source.invalid/".into();
    let mut store = ResolutionStore::default();
    let r = register(&mut store, source, vec![], 0);
    let view = open(&gateway, &mut store, &r);
    let response = fetch(media(&view), "GET", &[]);
    let children = local_uris(text(&response), &view);
    let denied = fetch(&children[0], "GET", &[]);
    assert_eq!(denied.status, 502);
    assert_eq!(
        serde_json::from_slice::<Value>(&denied.body).unwrap()["error"]["details"]["reason"],
        "unsafe_target"
    );
    assert_eq!(state(&gateway, &view)["error"]["code"], "UPSTREAM_FAILED");
    assert_eq!(
        state(&gateway, &view)["error"]["details"]["reason"],
        "unsafe_target"
    );
    let child = fetch(&children[1], "GET", &[]);
    let segment = local_uris(text(&child), &view);
    assert_eq!(fetch(&segment[0], "GET", &[]).status, 200);
    for req in fixture
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.target != "/master.m3u8")
    {
        for name in ["cookie", "authorization", "referer", "x-secret"] {
            assert!(!req.headers.contains_key(name));
        }
    }
    let foreign = hls_session(&gateway, fixture.url("/master.m3u8"));
    let wrong = children[1].replace(&cap(media(&view)), &cap(media(&foreign)));
    assert_eq!(fetch(&wrong, "GET", &[]).status, 401);
    let inject = format!("{}&url={}", children[1], fixture.url("/private-key"));
    assert_eq!(fetch(&inject, "GET", &[]).status, 401);
    let bearer = format!("Bearer {}", gateway.shared.token);
    let without_cap = children[1].split('?').next().unwrap();
    assert_eq!(
        fetch(without_cap, "GET", &[("Authorization", &bearer)]).status,
        200
    );
    gateway.close(id(&view));
    assert_eq!(fetch(&children[1], "GET", &[]).status, 410);
    assert!(
        !fixture
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.target == "/private-key")
    );
}

#[test]
fn t10_source_response_gzip_order_and_entity_headers() {
    const GZIP: &[u8] = include_bytes!("t10-manifest.gz");
    let fixture = Fixture::new(|r, _| match r.target.as_str() {
        "/wrapped.m3u8" => Reply::body(GZIP, "application/vnd.apple.mpegurl")
            .header("Content-Encoding", "gzip")
            .header("ETag", "\"encoded-upstream\"")
            .header("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT")
            .header("Accept-Ranges", "bytes")
            .header("Set-Cookie", "source=private"),
        "/plain-gzip" => Reply::body(GZIP, "video/mp4")
            .header("Content-Encoding", "gzip")
            .header("ETag", "\"encoded-upstream\""),
        "/bad-crc.m3u8" => {
            let mut broken = GZIP.to_vec();
            broken[GZIP.len() - 8] ^= 1;
            Reply::body(broken, "application/vnd.apple.mpegurl").header("Content-Encoding", "gzip")
        }
        "/truncated.m3u8" => Reply::body(&GZIP[..GZIP.len() - 8], "application/vnd.apple.mpegurl")
            .header("Content-Encoding", "gzip"),
        "/processed.ts?sig=a%2Fb+x&cap=upstream" => Reply::body(vec![7; 128], "video/mp2t"),
        _ => panic!("manifest rewritten before original response processing"),
    });
    let gateway = Gateway::start().unwrap();
    let progressive = session(&gateway, fixture.url("/plain-gzip"));
    let decoded = fetch(media(&progressive), "GET", &[]);
    assert_eq!(decoded.status, 200);
    assert_eq!(
        decoded.body,
        b"SOURCE\n#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXTINF:2,\norigin.ts\n#EXT-X-ENDLIST\n"
    );
    assert!(
        !decoded.headers.contains_key("etag") && !decoded.headers.contains_key("content-encoding")
    );
    let decoded_head = fetch(media(&progressive), "HEAD", &[]);
    assert_eq!(decoded_head.status, 200);
    assert!(
        !decoded_head.headers.contains_key("etag")
            && !decoded_head.headers.contains_key("content-encoding")
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut source = hls_link(fixture.url("/wrapped.m3u8"));
    source.interceptor = Some(Arc::new(HttpManifestContext {
        request: Box::new(|_, h| {
            h.insert("x-request-context", "original".parse().unwrap());
            Ok(())
        }),
        response: Box::new(move |url, h, body| {
            assert_eq!(url.path(), "/wrapped.m3u8");
            assert!(!h.contains_key("content-encoding") && !h.contains_key("content-length"));
            assert!(
                body.starts_with(b"SOURCE\n#EXTM3U\n"),
                "HTTP decoding or source processing order changed"
            );
            *body = std::str::from_utf8(body)
                .unwrap()
                .strip_prefix("SOURCE\n")
                .unwrap()
                .replace("origin.ts", "processed.ts?sig=a%2Fb+x&cap=upstream")
                .into_bytes();
            h.insert("content-length", "999".parse().unwrap());
            h.insert("content-encoding", "gzip".parse().unwrap());
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }),
    }));
    let mut store = ResolutionStore::default();
    let r = register(&mut store, source, vec![], 0);
    let view = open(&gateway, &mut store, &r);
    let response = fetch(media(&view), "GET", &[]);
    assert_body_headers(&response);
    let uris = local_uris(text(&response), &view);
    assert_eq!(uris.len(), 1);
    assert_eq!(fetch(&uris[0], "GET", &[]).body, vec![7; 128]);
    let head = fetch(media(&view), "HEAD", &[]);
    assert_eq!(head.status, 200);
    assert_eq!(
        head.headers["content-length"],
        response.body.len().to_string()
    );
    assert_eq!(head.headers["etag"], response.headers["etag"]);
    assert!(head.body.is_empty() && !head.headers.contains_key("content-encoding"));
    let unchanged = fetch(
        media(&view),
        "GET",
        &[("If-None-Match", &response.headers["etag"])],
    );
    assert_eq!(unchanged.status, 304);
    assert!(unchanged.body.is_empty());
    assert!(!unchanged.headers.contains_key("content-length"));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    for path in ["/bad-crc.m3u8", "/truncated.m3u8"] {
        let failed = hls_session(&gateway, fixture.url(path));
        let response = fetch(media(&failed), "GET", &[]);
        assert_eq!(response.status, 502);
        assert_eq!(state(&gateway, &failed)["error"]["code"], "UPSTREAM_FAILED");
    }
    // A missing source processor fails at the invalid decoded prefix, without
    // disclosing or registering the wrapped remote reference.
    let no_processor = hls_session(&gateway, fixture.url("/wrapped.m3u8"));
    assert_eq!(fetch(media(&no_processor), "GET", &[]).status, 502);
    for (code, status) in [("RESOURCE_LIMIT", 413), ("INTERACTION_REQUIRED", 502)] {
        let mut source = hls_link(fixture.url("/wrapped.m3u8"));
        source.interceptor = Some(Arc::new(HttpManifestContext {
            request: Box::new(|_, _| Ok(())),
            response: Box::new(move |_, _, _| Err(ApiError::new(code, "source-private-detail"))),
        }));
        let r = register(&mut store, source, vec![], 0);
        let failed = open(&gateway, &mut store, &r);
        let response = fetch(media(&failed), "GET", &[]);
        assert_eq!(response.status, status);
        assert_eq!(
            serde_json::from_slice::<Value>(&response.body).unwrap()["error"]["code"],
            code
        );
        assert!(!String::from_utf8_lossy(&response.body).contains("source-private-detail"));
    }
    let mut expanded = hls_link(fixture.url("/wrapped.m3u8"));
    expanded.interceptor = Some(Arc::new(HttpManifestContext {
        request: Box::new(|_, _| Ok(())),
        response: Box::new(|_, _, body| {
            body.resize(hls::LIMIT + 1, b'x');
            Ok(())
        }),
    }));
    let r = register(&mut store, expanded, vec![], 0);
    let failed = open(&gateway, &mut store, &r);
    assert_eq!(fetch(media(&failed), "GET", &[]).status, 413);
    for request in fixture
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.headers.contains_key("x-request-context"))
    {
        assert_eq!(request.headers["x-request-context"], "original");
        assert!(!request.headers.contains_key("if-none-match"));
    }
    println!(
        "T10_VECTOR:{}",
        json!({"kind":"response_order","source_calls":calls.load(Ordering::SeqCst),"decoded_before_source":true,"source_before_rewrite":true,"body_bytes":response.body.len(),"etag":response.headers["etag"],"sha256":format!("{:x}",Sha256::digest(&response.body)),"head_length":head.headers["content-length"],"conditional_status":unchanged.status})
    );
}

#[test]
fn t10_conditionals_html_sniff_and_refresh_headers() {
    let version = Arc::new(AtomicUsize::new(0));
    let current = version.clone();
    let fixture = Fixture::new(move |r, _| match r.target.as_str() {
        "/plain" => {
            let reply = Reply::body(vec![8; 128], "video/mp4").header("ETag", "\"original\"");
            if r.headers
                .get("if-none-match")
                .is_some_and(|v| v == "\"original\"")
            {
                Reply {
                    status: 304,
                    body: vec![],
                    ..reply
                }
            } else {
                reply
            }
        }
        "/html" => Reply::body("<html>upstream error</html>", "text/html"),
        "/hidden-html" => Reply::body(" \n<html>upstream error</html>", "video/mp4"),
        "/sniff" | "/mime-only" | "/live.m3u8" => Reply::body(
            format!(
                "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:{}\n#EXTINF:1,\nsegment.ts?version={}\n",
                current.load(Ordering::SeqCst),
                current.load(Ordering::SeqCst)
            ),
            if r.target == "/sniff" {
                "application/octet-stream"
            } else {
                "application/vnd.apple.mpegurl"
            },
        )
        .header("ETag", "\"origin-manifest\"")
        .header("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT"),
        _ => Reply::body(vec![3; 128], "video/mp2t"),
    });
    let gateway = Gateway::start().unwrap();
    let plain = session(&gateway, fixture.url("/plain"));
    let response = fetch(media(&plain), "GET", &[]);
    assert_eq!(response.headers["etag"], "\"original\"");
    let unchanged = fetch(media(&plain), "GET", &[("If-None-Match", "\"original\"")]);
    assert_eq!(unchanged.status, 304);
    assert!(unchanged.body.is_empty());
    assert_eq!(state(&gateway, &plain)["state"], "ready");
    for path in ["/html", "/hidden-html"] {
        let failed = session(&gateway, fixture.url(path));
        let response = fetch(media(&failed), "GET", &[]);
        assert_eq!(response.status, 502);
        assert_eq!(
            serde_json::from_slice::<Value>(&response.body).unwrap()["error"]["code"],
            "UPSTREAM_FAILED"
        );
    }
    for path in ["/sniff", "/mime-only"] {
        let view = session(&gateway, fixture.url(path));
        let response = fetch(media(&view), "GET", &[]);
        assert_body_headers(&response);
        assert_eq!(local_uris(text(&response), &view).len(), 1);
        let head = fetch(media(&view), "HEAD", &[]);
        assert_eq!(
            head.headers["content-length"],
            response.body.len().to_string()
        );
        assert_eq!(head.headers["etag"], response.headers["etag"]);
    }
    let view = hls_session(&gateway, fixture.url("/live.m3u8"));
    let first = fetch(media(&view), "GET", &[]);
    let etag = &first.headers["etag"];
    assert_eq!(
        fetch(media(&view), "GET", &[("If-None-Match", etag)]).status,
        304
    );
    version.store(1, Ordering::SeqCst);
    let fresh = fetch(media(&view), "GET", &[("If-None-Match", etag)]);
    assert_eq!(fresh.status, 200);
    assert_ne!(fresh.headers["etag"], *etag);
    assert!(text(&fresh).contains("#EXT-X-MEDIA-SEQUENCE:1\n"));
    assert_eq!(
        fetch(media(&view), "GET", &[("Range", "bytes=0-7")]).status,
        200
    );
    assert!(fixture.state.requests.lock().unwrap().iter().any(|r| {
        r.target == "/plain"
            && r.headers
                .get("if-none-match")
                .is_some_and(|v| v == "\"original\"")
    }));
}

#[test]
fn t10_active_session_release_clock_refresh_key_segment_and_close() {
    let gate = Arc::new(AtomicBool::new(false));
    let pending = gate.clone();
    let fixture = Fixture::new(move |r, _| match r.target.as_str() {
        "/live.m3u8" => Reply::body(
            "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:37\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\"\n#EXTINF:1,\nsegment.ts\n",
            "application/vnd.apple.mpegurl",
        ),
        "/key" => Reply::body(vec![0x37; 16], "application/octet-stream"),
        "/segment.ts" => Reply {
            repeat: 2,
            gate: Some(pending.clone()),
            ..Reply::body(vec![4; 65536], "video/mp2t")
        },
        _ => panic!("unexpected lifetime resource"),
    });
    let clock = Arc::new(TestClock::default());
    let gateway = Arc::new(Gateway::with_clock(clock.clone()).unwrap());
    let cookie = Arc::new(AtomicUsize::new(0));
    let cookie_context = cookie.clone();
    let mut source = hls_link(fixture.url("/live.m3u8"));
    source.cookies = Some(Arc::new(HttpContext(Box::new(move |_, h| {
        h.insert(
            "cookie",
            format!("generation={}", cookie_context.load(Ordering::SeqCst))
                .parse()
                .unwrap(),
        );
        Ok(())
    }))));
    let mut store = ResolutionStore::default();
    let r = register(&mut store, source, vec![], 0);
    let view = open(&gateway, &mut store, &r);
    let playlist = fetch(media(&view), "GET", &[]);
    let children = local_uris(text(&playlist), &view);
    store.release(r["id"].as_str().unwrap());
    assert!(
        store
            .selected(
                r["id"].as_str().unwrap(),
                r["streams"][0]["id"].as_str().unwrap(),
                0
            )
            .is_err()
    );
    let mut stream = connect(&children[1], "GET", &[]);
    assert_eq!(read_head(&mut stream).status, 200);
    let mut first = vec![0; 65536];
    stream.read_exact(&mut first).unwrap();
    assert_eq!(first, vec![4; 65536]);
    clock.0.store(300_001, Ordering::SeqCst);
    cookie.store(1, Ordering::SeqCst);
    assert_eq!(state(&gateway, &view)["state"], "ready");
    let refreshed = fetch(media(&view), "GET", &[]);
    assert!(
        refreshed.body == playlist.body,
        "live child identity changed after candidate release"
    );
    assert_eq!(fetch(&children[0], "GET", &[]).body, vec![0x37; 16]);
    gate.store(true, Ordering::SeqCst);
    let mut remaining = Vec::new();
    stream.read_to_end(&mut remaining).unwrap();
    assert_eq!(remaining, vec![4; 65536]);
    assert_eq!(fetch(&children[1], "GET", &[]).body, vec![4; 131072]);
    let targets: Vec<_> = fixture
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.headers.get("cookie").is_some_and(|v| v == "generation=1"))
        .map(|r| r.target.clone())
        .collect();
    assert!(
        targets.contains(&"/live.m3u8".into())
            && targets.contains(&"/key".into())
            && targets.contains(&"/segment.ts".into())
    );
    wait_for(|| gateway.shared.records.lock().unwrap().sessions[id(&view)].active == 0);
    clock.0.store(600_001, Ordering::SeqCst);
    assert_eq!(fetch(&children[0], "GET", &[]).status, 410);
    assert_eq!(state(&gateway, &view)["state"], "expired");
    // A late manifest response after explicit close cannot resurrect resources.
    let slow = Fixture::new(|_, _| Reply {
        body_delay: Duration::from_millis(250),
        ..Reply::body("#EXTM3U\nchild.ts\n", "application/vnd.apple.mpegurl")
    });
    let late = hls_session(&gateway, slow.url("/late.m3u8"));
    let url = media(&late).to_owned();
    let call = thread::spawn(move || {
        let mut reader = connect(&url, "GET", &[]);
        let mut body = Vec::new();
        reader.read_to_end(&mut body).unwrap();
        body
    });
    wait_for(|| !slow.state.requests.lock().unwrap().is_empty());
    gateway.close(id(&late));
    assert!(call.join().unwrap().is_empty());
    assert!(
        !gateway
            .shared
            .records
            .lock()
            .unwrap()
            .sessions
            .contains_key(id(&late))
    );
    println!(
        "T10_VECTOR:{}",
        json!({"kind":"lifetime","active_at_ms":300001,"resolution_released":true,"continued_bytes":131072,"post_release_targets":targets,"idle_expired_at_ms":600001,"late_close_cancelled":true})
    );
}

fn ffmpeg(folder: &Path, args: &[&str], secret: Option<&str>) -> Vec<u8> {
    let output = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            if secret.is_some() { "info" } else { "error" },
            "-y",
            "-threads",
            "1",
            "-filter_threads",
            "1",
            "-filter_complex_threads",
            "1",
        ])
        .args(args)
        .current_dir(folder)
        .output()
        .expect("ffmpeg executable required for owned-media fixture");
    let mut diagnostic = String::from_utf8_lossy(&output.stderr).into_owned();
    if let Some(secret) = secret {
        diagnostic = diagnostic.replace(secret, "<redacted-cap>");
        eprintln!("T10_FFMPEG:{diagnostic}");
    }
    assert!(output.status.success(), "ffmpeg failed: {diagnostic}");
    output.stdout
}
fn frame_hashes(bytes: &[u8], stream: usize) -> Vec<String> {
    let stream = stream.to_string();
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let fields: Vec<_> = line.split(',').map(str::trim).collect();
            if fields.len() == 6 && fields[0] == stream.as_str() {
                Some(fields[5].to_owned())
            } else {
                None
            }
        })
        .collect()
}
fn signed_playlist(text: &str) -> String {
    let mut output = String::new();
    for line in text.lines() {
        if let Some((before, value)) = line.split_once("URI=\"") {
            let (uri, after) = value.split_once('"').unwrap();
            output.push_str(&format!(
                "{before}URI=\"{uri}?sig=A%2FB+x&cap=upstream\"{after}\n"
            ));
        } else if !line.starts_with('#') && !line.is_empty() {
            output.push_str(&format!("{line}?sig=A%2FB+x&cap=upstream\n"));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

// The fixture key is public test data. Encrypt each plaintext segment ourselves:
// FFmpeg's keyinfo writer emits an explicit IV even when its third line is absent.
fn sequence_aes(path: &Path, sequence: u128, decrypt: bool) -> Vec<u8> {
    let mut command = Command::new("openssl");
    command.args(["enc", "-aes-128-cbc"]);
    if decrypt {
        command.arg("-d");
    }
    let output = command
        .args(["-K", "000102030405060708090a0b0c0d0e0f", "-iv"])
        .arg(format!("{sequence:032x}"))
        .arg("-in")
        .arg(path)
        .output()
        .expect("openssl executable required for sequence-IV fixture");
    assert!(
        output.status.success(),
        "sequence-IV AES operation failed (sequence={sequence}, decrypt={decrypt}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn implicit_iv_fixture(target: &Path) {
    let path = target.join("index.m3u8");
    let original = fs::read_to_string(&path).unwrap();
    assert!(original.contains("#EXT-X-MEDIA-SEQUENCE:37\n"));
    assert!(!original.contains("#EXT-X-KEY:"));
    let playlist = original.replacen(
        "#EXT-X-MEDIA-SEQUENCE:37\n",
        "#EXT-X-MEDIA-SEQUENCE:37\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n",
        1,
    );
    assert!(!playlist.contains("IV="));
    let uris: Vec<_> = playlist
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(uris.len(), 2);
    fs::create_dir(target.join("plaintext")).unwrap();
    let mut segments = Vec::new();
    for (offset, uri) in uris.iter().enumerate() {
        let sequence = 37 + offset as u128;
        let encrypted_path = target.join(uri);
        let plain_path = target.join("plaintext").join(uri);
        fs::rename(&encrypted_path, &plain_path).unwrap();
        let plaintext = fs::read(&plain_path).unwrap();
        let ciphertext = sequence_aes(&plain_path, sequence, false);
        fs::write(&encrypted_path, &ciphertext).unwrap();
        let decrypted = sequence_aes(&encrypted_path, sequence, true);
        let wrong_iv = sequence_aes(&encrypted_path, 0, true);
        let wrong_sequence = sequence_aes(&encrypted_path, sequence + 1, true);
        // A decoder can tolerate damage in the first TS packet. Compare every
        // plaintext byte as well as the downstream 24-frame decode assertion.
        assert_eq!(decrypted, plaintext, "sequence IV plaintext differs");
        assert_ne!(wrong_iv, plaintext, "zero IV unexpectedly matched");
        assert_ne!(
            wrong_sequence, plaintext,
            "wrong sequence unexpectedly matched"
        );
        segments.push(json!({
            "uri":uri,"sequence":sequence as u64,"plaintext_bytes":plaintext.len(),
            "plaintext_sha256":format!("{:x}",Sha256::digest(&plaintext)),
            "ciphertext_sha256":format!("{:x}",Sha256::digest(&ciphertext)),
            "ciphertext_hex":ciphertext.iter().map(|b|format!("{b:02x}")).collect::<String>(),
            "decrypted_sha256":format!("{:x}",Sha256::digest(&decrypted)),
            "wrong_iv_sha256":format!("{:x}",Sha256::digest(&wrong_iv)),
            "wrong_sequence_sha256":format!("{:x}",Sha256::digest(&wrong_sequence))
        }));
    }
    fs::write(path, &playlist).unwrap();
    println!(
        "T10_VECTOR:{}",
        json!({"kind":"implicit_iv","playlist":playlist,
            "key_sha256":format!("{:x}",Sha256::digest(fs::read(target.join("key.bin")).unwrap())),
            "segments":segments})
    );
}

#[test]
fn t10_real_aes128_explicit_implicit_iv_and_fmp4_decode() {
    let folder = tempfile::tempdir().unwrap();
    ffmpeg(
        folder.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=12",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            "2",
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-g",
            "12",
            "-sc_threshold",
            "0",
            "-threads",
            "1",
            "-c:a",
            "aac",
            "-b:a",
            "48k",
            "clip.mp4",
        ],
        None,
    );
    let expected = frame_hashes(
        &ffmpeg(
            folder.path(),
            &[
                "-i", "clip.mp4", "-map", "0:v:0", "-an", "-f", "framemd5", "-",
            ],
            None,
        ),
        0,
    );
    assert_eq!(expected.len(), 24);
    let mut originals = Vec::new();
    for mode in ["aes-explicit", "aes-implicit", "fmp4"] {
        let target = folder.path().join(mode);
        fs::create_dir(&target).unwrap();
        let keyinfo = target.join("keyinfo");
        if mode.starts_with("aes") {
            fs::write(target.join("key.bin"), (0u8..16).collect::<Vec<_>>()).unwrap();
            if mode == "aes-explicit" {
                fs::write(
                    &keyinfo,
                    format!(
                        "key.bin\n{}\n00000000000000000000000000000025\n",
                        target.join("key.bin").display()
                    ),
                )
                .unwrap();
            }
        }
        let mut args = vec![
            "-i",
            "../clip.mp4",
            "-map",
            "0",
            "-c",
            "copy",
            "-hls_time",
            "1",
            "-hls_playlist_type",
            "vod",
        ];
        if mode == "fmp4" {
            args.extend([
                "-hls_segment_type",
                "fmp4",
                "-hls_flags",
                "single_file",
                "-hls_segment_filename",
                "stream.mp4",
            ]);
        } else if mode == "aes-explicit" {
            args.extend([
                "-hls_key_info_file",
                "keyinfo",
                "-hls_segment_filename",
                "segment-%03d.ts",
            ]);
        } else {
            args.extend([
                "-start_number",
                "37",
                "-hls_segment_filename",
                "segment-%03d.ts",
            ]);
        }
        args.push("index.m3u8");
        ffmpeg(&target, &args, None);
        if mode == "aes-implicit" {
            implicit_iv_fixture(&target);
        }
        eprintln!(
            "T10_SOURCE:{}",
            json!({"mode":mode,"playlist":fs::read_to_string(target.join("index.m3u8")).unwrap()})
        );
        let original = ffmpeg(
            &target,
            &[
                "-protocol_whitelist",
                "file,crypto",
                "-f",
                "hls",
                "-allowed_extensions",
                "ALL",
                "-allowed_segment_extensions",
                "ALL",
                "-extension_picky",
                "0",
                "-i",
                "index.m3u8",
                "-map",
                "0:v:0",
                "-map",
                "0:a:0",
                "-threads",
                "1",
                "-f",
                "framemd5",
                "-",
            ],
            None,
        );
        assert_eq!(
            frame_hashes(&original, 0),
            expected,
            "{mode}: original HLS video differs from the complete source"
        );
        assert!(
            !frame_hashes(&original, 1).is_empty(),
            "{mode}: original HLS audio missing"
        );
        originals.push(
            json!({"mode":mode,"video_frames":frame_hashes(&original, 0).len(),
            "audio_frames":frame_hashes(&original, 1).len(),
            "video_sha256":format!("{:x}",Sha256::digest(frame_hashes(&original, 0).join(",")))}),
        );
        fs::write(target.join("master.m3u8"),"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=500000,CODECS=\"avc1.42c00a,mp4a.40.2\"\nindex.m3u8\n").unwrap();
    }
    let assets = folder.path().to_owned();
    let fixture = Fixture::new(move |r, _| {
        let (path, query) = r.target.split_once('?').unwrap_or((&r.target, ""));
        if let Some(mode) = path
            .strip_prefix("/entry/")
            .and_then(|p| p.strip_suffix(".m3u8"))
        {
            return Reply::redirect(format!("/assets/{mode}/master.m3u8?sig=redirect"));
        }
        let relative = path
            .strip_prefix("/assets/")
            .expect("registered media fixture path");
        assert!(!relative.contains(".."));
        let file = assets.join(relative);
        let mut bytes = fs::read(&file).expect("owned HLS fixture resource");
        let mime = if path.ends_with(".m3u8") {
            if path.ends_with("/master.m3u8") {
                assert_eq!(query, "sig=redirect");
            } else {
                assert_eq!(query, "sig=A%2FB+x&cap=upstream");
            }
            bytes = signed_playlist(std::str::from_utf8(&bytes).unwrap()).into_bytes();
            "application/vnd.apple.mpegurl"
        } else {
            assert_eq!(query, "sig=A%2FB+x&cap=upstream");
            if path.ends_with(".ts") {
                "video/mp2t"
            } else {
                "application/octet-stream"
            }
        };
        eprintln!(
            "T10_REQUEST:{}",
            json!({"target":r.target,"range":r.headers.get("range")})
        );
        let total = bytes.len();
        if let Some(range) = r.headers.get("range") {
            let (a, b) = range
                .strip_prefix("bytes=")
                .unwrap()
                .split_once('-')
                .unwrap();
            let a: usize = a.parse().unwrap();
            if a >= total {
                return Reply {
                    status: 416,
                    ..Reply::body("", mime).header("Content-Range", format!("bytes */{total}"))
                };
            }
            let b = if b.is_empty() {
                total - 1
            } else {
                b.parse::<usize>().unwrap().min(total - 1)
            };
            return Reply {
                status: 206,
                ..Reply::body(bytes[a..=b].to_vec(), mime)
                    .header("Content-Range", format!("bytes {a}-{b}/{total}"))
                    .header("Accept-Ranges", "bytes")
            };
        }
        Reply::body(bytes, mime)
    });
    let gateway = Gateway::start().unwrap();
    for (mode, original) in ["aes-explicit", "aes-implicit", "fmp4"]
        .into_iter()
        .zip(originals)
    {
        let start = fixture.state.requests.lock().unwrap().len();
        let mut source = hls_link(fixture.url(&format!("/entry/{mode}.m3u8")));
        source.referer = "https://provider.invalid/owned-media".into();
        source.cookies = Some(Arc::new(HttpContext(Box::new(|_, h| {
            h.insert("cookie", "original=owned-media".parse().unwrap());
            Ok(())
        }))));
        source.interceptor = Some(Arc::new(HttpContext(Box::new(|_, h| {
            h.insert("x-context", "original-source".parse().unwrap());
            Ok(())
        }))));
        let mut store = ResolutionStore::default();
        let r = register(&mut store, source, vec![], 0);
        let view = open(&gateway, &mut store, &r);
        store.release(r["id"].as_str().unwrap());
        let output = ffmpeg(
            folder.path(),
            &[
                "-protocol_whitelist",
                "http,tcp,crypto",
                "-f",
                "hls",
                "-allowed_extensions",
                "ALL",
                "-allowed_segment_extensions",
                "ALL",
                "-extension_picky",
                "0",
                "-http_multiple",
                "0",
                "-http_persistent",
                "0",
                // FFmpeg 9.0.2's HLS reuse seeks an absolute file offset even
                // though open_input documents different HTTP AVIO bookkeeping.
                // Disable that reuse for fMP4; explicit HLS offset/end_offset
                // still send Range with seekable=0 (n9.0.2 hls.c/open_input and
                // http.c/http_connect). Keep every byte/frame assertion below.
                "-http_seekable",
                if mode == "fmp4" { "0" } else { "-1" },
                "-i",
                media(&view),
                "-map",
                "0:v:0",
                "-map",
                "0:a:0",
                "-threads",
                "1",
                "-f",
                "framemd5",
                "-",
            ],
            Some(&cap(media(&view))),
        );
        let video = frame_hashes(&output, 0);
        let audio = frame_hashes(&output, 1);
        eprintln!(
            "T10_REQUESTS:{}",
            json!({"mode":mode,"video_frames":video.len(),"audio_frames":audio.len(),"requests":fixture.state.requests.lock().unwrap()[start..].iter().map(|r|json!({"target":r.target,"range":r.headers.get("range")})).collect::<Vec<_>>()})
        );
        assert_eq!(
            video, expected,
            "{mode}: decoded video frame hashes changed through HLS gateway"
        );
        assert!(!audio.is_empty(), "audio decode missing");
        let requests = fixture.state.requests.lock().unwrap();
        let requests = &requests[start..];
        assert!(
            requests
                .iter()
                .any(|r| r.target.contains("/index.m3u8?sig=A%2FB+x&cap=upstream"))
        );
        if mode.starts_with("aes") {
            assert!(
                requests
                    .iter()
                    .any(|r| r.target.contains("/key.bin?sig=A%2FB+x&cap=upstream"))
            );
        } else {
            assert!(
                requests
                    .iter()
                    .filter(|r| r.target.contains("stream.mp4") && r.headers.contains_key("range"))
                    .count()
                    >= 2
            );
        }
        for request in requests {
            assert_eq!(request.headers["cookie"], "original=owned-media");
            assert_eq!(request.headers["x-context"], "original-source");
            assert_eq!(
                request.headers["referer"],
                "https://provider.invalid/owned-media"
            );
            assert!(
                !format!("{} {:?}", request.target, request.headers).contains(&cap(media(&view)))
            );
        }
        println!(
            "T10_VECTOR:{}",
            json!({"kind":"decode","mode":mode,"video_frames":video.len(),"audio_frames":audio.len(),"original":original,"source_sha256":format!("{:x}",Sha256::digest(expected.join(","))),"decoded_sha256":format!("{:x}",Sha256::digest(video.join(","))),"requests":requests.iter().map(|r|json!({"target":r.target,"range":r.headers.get("range")})).collect::<Vec<_>>(),"resolution_released":true,"same_cap":true})
        );
    }
}
