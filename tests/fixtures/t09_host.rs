//! Invoke-to-resource tests reuse the established MacCMS/Stremio HTTP fixture.
use super::*;
use std::net::TcpStream;
#[path = "n02_host.rs"]
mod n02;
#[path = "t04_host.rs"]
mod t04;
#[path = "t11_host.rs"]
mod t11;
#[path = "t12_host.rs"]
mod t12;
#[path = "t14_host.rs"]
mod t14;
#[path = "t15_host.rs"]
mod t15;
#[path = "t17_host.rs"]
mod t17;
#[path = "t18_host.rs"]
mod t18;
#[path = "t19_host.rs"]
mod t19;
#[path = "t24_cache_host.rs"]
mod t24_cache;

fn png() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 3)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
fn gateway_fixture(delay: Arc<AtomicBool>) -> Fixture {
    let image = png();
    Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            return metadata_reply(request).unwrap();
        }
        if request.target.contains("/catalog/") || request.target.contains("/meta/") {
            let mut value = meta();
            value["poster"] = json!(format!(
                "http://maccms.fixture.test:{port}/image.png?kind=poster"
            ));
            value["background"] = json!(format!(
                "http://maccms.fixture.test:{port}/image.png?kind=background"
            ));
            value["cast"] = json!(["original actor"]);
            value["app_extras"] = json!({"cast":[{"name":"original actor","character":"original role","photo":format!("http://maccms.fixture.test:{port}/image.png?kind=actor")}]});
            value["videos"][0]["thumbnail"] = json!(format!(
                "http://maccms.fixture.test:{port}/image.png?kind=episode"
            ));
            return Reply::json(if request.target.contains("/catalog/") {
                json!({"metas":[value]})
            } else {
                json!({"meta":value})
            });
        }
        if request.target.starts_with("/image.png") || request.target.starts_with("/poster.png") {
            return Reply::raw(image.clone(), "image/png");
        }
        if request.target.starts_with("/probe") || request.target.contains(".mp4") {
            return Reply {
                body_delay: if delay.load(Ordering::SeqCst) {
                    Duration::from_secs(5)
                } else {
                    Duration::ZERO
                },
                ..Reply::raw(vec![0x5a; 65536], "video/mp4")
            };
        }
        let mut raw = item(port);
        raw["vod_name"] = json!("Same title");
        raw["vod_year"] = Value::Null;
        raw["vod_play_url"] = json!(format!(
            "Special$http://maccms.fixture.test:{port}/probe?signature=upstream#Second$http://maccms.fixture.test:{port}/b.mp4$$$Dub$http://maccms.fixture.test:{port}/dub.mp4"
        ));
        Reply::json(response(json!([raw]), json!([])))
    })
}
fn resource(url: &str) -> (u16, Vec<u8>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let response = reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(url)
                .send()
                .await
                .unwrap();
            let status = response.status().as_u16();
            (status, response.bytes().await.unwrap().to_vec())
        })
}
fn selected(host: &Host, provider: &Value, input: &Value) -> Value {
    let details = inspect(host, provider);
    ok(host, "match_playback", choose(input, &details, 0, 0))["context"].clone()
}
fn open_input(resolution: &Value) -> Value {
    json!({"resolution_id":resolution["id"],"stream_id":resolution["streams"][0]["id"]})
}
fn diagnostic(host: &Host, view: &Value) -> Value {
    ok(
        host,
        "get_resource_status",
        json!({"kind":"playback","target_id":view["session_id"]}),
    )
}

#[test]
fn t09_host_session_bindings_switch_release_and_old_cleanup() {
    let fixture = gateway_fixture(Arc::new(AtomicBool::new(false)));
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    let first = ok(&host, "resolve_episode", resolve_input(&context));
    let old = ok(&host, "open_playback", open_input(&first));
    for (key, expected) in [
        ("resolution_id", &first["id"]),
        ("stream_id", &first["streams"][0]["id"]),
        ("source_id", &provider["id"]),
        ("source_revision", &provider["source_revision"]),
        ("episode_ref", &context["episode_ref"]),
    ] {
        assert_eq!(&old[key], expected);
    }
    assert_eq!(old["stream_kind"], "progressive");
    assert_eq!(resource(old["media_url"].as_str().unwrap()).0, 200);
    ok(
        &host,
        "close_playback",
        json!({"session_id":old["session_id"]}),
    );
    let switched = ok(&host, "open_playback", open_input(&first));
    assert_ne!(switched["session_id"], old["session_id"]);
    assert_eq!(switched["resolution_id"], first["id"]);
    ok(
        &host,
        "release_resolution",
        json!({"resolution_id":first["id"]}),
    );
    assert_eq!(
        call(&host, "open_playback", open_input(&first))["error"]["code"],
        "SESSION_EXPIRED"
    );
    assert_eq!(resource(switched["media_url"].as_str().unwrap()).0, 200);
    let second = ok(&host, "resolve_episode", resolve_input(&context));
    let current = ok(&host, "open_playback", open_input(&second));
    for _ in 0..2 {
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
    }
    assert_eq!(diagnostic(&host, &current)["state"], "ready");
    assert_eq!(resource(current["media_url"].as_str().unwrap()).0, 200);
    assert_eq!(
        call(
            &host,
            "open_playback",
            json!({"resolution_id":second["id"],"stream_id":first["streams"][0]["id"]})
        )["error"]["code"],
        "SESSION_EXPIRED"
    );
    let no_session = ok(&host, "resolve_episode", resolve_input(&context));
    assert_eq!(
        call(&host, "open_playback", open_input(&second))["error"]["code"],
        "SESSION_EXPIRED"
    );
    ok(
        &host,
        "release_resolution",
        json!({"resolution_id":no_session["id"]}),
    );
    assert_eq!(
        call(&host, "open_playback", open_input(&no_session))["error"]["code"],
        "SESSION_EXPIRED"
    );
    assert_eq!(resource(current["media_url"].as_str().unwrap()).0, 200);
    let url = url::Url::parse(current["media_url"].as_str().unwrap()).unwrap();
    let mut safe = current.clone();
    safe["media_url"] = json!({"scheme":url.scheme(),"host":url.host_str(),"path":url.path(),"query_keys":url.query_pairs().map(|(k,_)|k.into_owned()).collect::<Vec<_>>(),"cap_bytes":32});
    println!(
        "T09_VECTOR:{}",
        json!({"kind":"host_binding","session":safe,"reference":context["episode_ref"],"resolution_id":second["id"],"stream_id":second["streams"][0]["id"],"epoch":host.host_epoch,"old_state":diagnostic(&host,&old)["state"],"current_state":diagnostic(&host,&current)["state"]})
    );
}

#[test]
fn t09_host_image_maps_recursive_dedupe_and_generation() {
    let fixture = gateway_fixture(Arc::new(AtomicBool::new(false)));
    let (_temp, host, provider, metadata, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let home = ok(&host, "get_home", home_input(&metadata));
    let card = &home["page"]["sections"][0]["items"][0];
    let details = ok(&host, "get_details", json!({"ref":card["ref"]}));
    let mut ids = HashSet::new();
    crate::details::images(&details["details"], &mut |image| {
        let id = image["resource_id"].as_str().unwrap();
        ids.insert(id.to_owned());
        let access = &details["images"][id];
        assert_eq!(access["resource_id"], id);
        let (status, body) = resource(access["url"].as_str().unwrap());
        assert_eq!(status, 200);
        assert_eq!(body, png());
        Ok(())
    })
    .unwrap();
    assert_eq!(ids.len(), 4);
    assert_eq!(details["images"].as_object().unwrap().len(), 4);
    let poster = card["poster"]["resource_id"].as_str().unwrap();
    assert_eq!(
        home["images"][poster]["url"],
        details["images"][poster]["url"]
    );
    let before = fixture.requests.lock().unwrap().len();
    for id in &ids {
        assert_eq!(
            ok(
                &host,
                "get_resource_status",
                json!({"kind":"image","target_id":id})
            )["state"],
            "ready"
        );
    }
    assert_eq!(fixture.requests.lock().unwrap().len(), before);
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    let provider_home = ok(&host, "get_home", home_input(&provider));
    let provider_image = provider_home["images"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["source_configs"][0]["enabled"] = json!(false);
    ok(&host, "save_settings", settings);
    assert_eq!(diagnostic(&host, &view)["state"], "expired");
    assert_eq!(resource(view["media_url"].as_str().unwrap()).0, 410);
    assert_eq!(
        ok(
            &host,
            "get_resource_status",
            json!({"kind":"image","target_id":provider_image})
        )["state"],
        "expired"
    );
    assert_eq!(
        ok(
            &host,
            "get_resource_status",
            json!({"kind":"image","target_id":poster})
        )["state"],
        "ready"
    );
    assert_eq!(
        resource(details["images"][poster]["url"].as_str().unwrap()).0,
        200
    );
}

#[test]
fn t09_host_inflight_media_revoke_and_control_responsive() {
    let delay = Arc::new(AtomicBool::new(false));
    let fixture = gateway_fixture(delay.clone());
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    delay.store(true, Ordering::SeqCst);
    let url = url::Url::parse(view["media_url"].as_str().unwrap()).unwrap();
    let mut client = TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let before = fixture.requests.lock().unwrap().len();
    write!(
        client,
        "GET {}?{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        url.path(),
        url.query().unwrap()
    )
    .unwrap();
    let start = Instant::now();
    while fixture.requests.lock().unwrap().len() == before {
        assert!(start.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
    let started = Instant::now();
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["source_configs"][0]["enabled"] = json!(false);
    ok(&host, "save_settings", settings);
    assert!(started.elapsed() < Duration::from_secs(1));
    let mut byte = [0];
    assert_eq!(client.read(&mut byte).unwrap(), 0);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(diagnostic(&host, &view)["state"], "expired");
    ok(
        &host,
        "close_playback",
        json!({"session_id":view["session_id"]}),
    );
    ok(
        &host,
        "release_resolution",
        json!({"resolution_id":resolution["id"]}),
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t09_host_inflight_resolve_cancel_and_late_result_rejected() {
    let delay = Arc::new(AtomicBool::new(false));
    let fixture = gateway_fixture(delay.clone());
    let (_temp, host, provider, _, input) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let context = selected(&host, &provider, &input);
    delay.store(true, Ordering::SeqCst);
    let request_id = Uuid::new_v4().to_string();
    let call = host
        .admit(
            "resolve_episode",
            json!({"request_id":request_id,"input":resolve_input(&context)}),
            true,
        )
        .unwrap_or_else(|e| panic!("resolve admission failed: {}", e["error"]["code"]));
    let before = fixture.requests.lock().unwrap().len();
    let worker_host = host.clone();
    let worker = thread::spawn(move || worker_host.execute(call));
    let started = Instant::now();
    while fixture.requests.lock().unwrap().len() == before {
        assert!(started.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
    let stop = Instant::now();
    let cancelled = ok(
        &host,
        "cancel_request",
        json!({"target_request_id":request_id}),
    );
    assert_eq!(cancelled["cancelled"], true);
    let result = worker.join().unwrap();
    assert_eq!(result["error"]["code"], "CANCELLED");
    assert!(stop.elapsed() < Duration::from_secs(2));
    assert_eq!(host.requests.active_count(), 0);
    assert!(host.committed.lock().unwrap().current_resolution.is_none());
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "provider=secret".parse().unwrap());
    assert!(
        redirect_headers(
            &Url::parse("https://example.invalid/a").unwrap(),
            &Url::parse("http://example.invalid/b").unwrap(),
            &mut headers
        )
        .is_err()
    );
}
