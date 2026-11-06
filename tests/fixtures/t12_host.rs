//! Real Host invoke/gateway bridge for the formal Vue playback view.
//! A synthetic native callback adds two lines and VTT to real MacCMS resolutions.
use super::*;
use crate::resolution::{Link, Subtitle};
#[path = "t21_trace.rs"]
mod t21_trace;

fn publish(path: &std::path::Path, value: &Value) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

fn metadata(port: u16) -> Value {
    let mut value = meta();
    value["poster"] = json!(format!("http://maccms.fixture.test:{port}/poster.png"));
    value["videos"] = json!((0..52).map(|i| json!({"id":format!("video:{i}"),"title":format!("Episode {i}"),"season":1,"episode":i+1})).collect::<Vec<_>>());
    value
}

#[test]
fn t12_host_formal_webview_bridge() {
    let directory = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from);
    let media = std::env::var_os("T12_MEDIA").map(std::path::PathBuf::from);
    assert!(
        directory.is_none() || media.is_some(),
        "live bridge requires owned media"
    );
    let images = png();
    let mode = Arc::new(AtomicUsize::new(0));
    let observed_mode = mode.clone();
    let error_requests = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let observed_errors = error_requests.clone();
    let trace = t21_trace::Trace::from_environment();
    let observed_trace = trace.clone();
    let upstream = Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            return metadata_reply(request).unwrap();
        }
        if request.target.contains("/catalog/") {
            return Reply::json(json!({"metas":[metadata(port)]}));
        }
        if request.target.contains("/meta/") {
            return Reply::json(json!({"meta":metadata(port)}));
        }
        if request.target.starts_with("/poster.png") {
            return Reply::raw(images.clone(), "image/png");
        }
        if request.target.starts_with("/subtitle.vtt") {
            let mode = observed_mode.load(Ordering::SeqCst);
            if mode != 0 {
                observed_errors.lock().unwrap().push(json!({"mode":mode,"target":request.target,"status":if mode == 4 {502} else {200},"mime":if mode == 4 {"text/plain"} else {"text/vtt"}}));
            }
            return if mode == 4 {
                Reply {
                    status: 502,
                    ..Reply::raw("subtitle unavailable", "text/plain")
                }
            } else {
                Reply::raw(
                    "WEBVTT\n\n00:00:00.000 --> 00:00:01.900\n映象 T12\n",
                    "text/vtt",
                )
            };
        }
        if request.target.starts_with("/media/") {
            let mode = observed_mode.load(Ordering::SeqCst);
            if (1..=3).contains(&mode) {
                observed_errors.lock().unwrap().push(json!({"mode":mode,"target":request.target,"status":match mode {1=>401,2=>502,_=>200},"mime":if mode == 3 {"video/mp4"} else {"text/plain"}}));
            }
            let reply = match mode {
                1 => Reply {
                    status: 401,
                    ..Reply::raw("upstream unauthorized", "text/plain")
                },
                2 => Reply {
                    status: 502,
                    ..Reply::raw("upstream unavailable", "text/plain")
                },
                3 => Reply::raw(vec![0x5a; 4096], "video/mp4"),
                _ => {
                    let name = request
                        .target
                        .trim_start_matches("/media/")
                        .split('?')
                        .next()
                        .unwrap();
                    assert!(!name.contains('/') && !name.contains(".."));
                    let mime = if name.ends_with(".m3u8") {
                        "application/vnd.apple.mpegurl"
                    } else if name.ends_with(".ts") {
                        "video/mp2t"
                    } else {
                        "video/mp4"
                    };
                    Reply::raw(
                        media
                            .as_ref()
                            .map(|folder| std::fs::read(folder.join(name)).unwrap())
                            .unwrap_or_else(|| vec![0; 64]),
                        mime,
                    )
                }
            };
            if let Some(trace) = &observed_trace {
                trace.request(request, &reply, mode);
            }
            return reply;
        }
        let mut raw = item(port);
        raw["vod_name"] = json!("Same title");
        raw["vod_year"] = Value::Null;
        raw["vod_play_from"] = json!("Native fixture");
        raw["vod_play_url"] = json!(
            (0..52)
                .map(|i| format!(
                    "Episode {i}$http://maccms.fixture.test:{port}/media/{}?episode={i}",
                    if i == 51 {
                        "index.m3u8"
                    } else if i % 2 == 0 {
                        "a.mp4"
                    } else {
                        "b.mp4"
                    }
                ))
                .collect::<Vec<_>>()
                .join("#")
        );
        Reply::json(response(json!([raw]), json!([])))
    });
    let clock = Arc::new(crate::requests::MonotonicClock::default());
    let (_temp, host, provider, metadata_source, input) = setup_both(&upstream, clock.clone());
    ok(&host, "runtime_prepare", json!({"target":"application"}));
    let details = inspect(&host, &provider);
    assert_eq!(
        details["groups"][0]["episodes"].as_array().unwrap().len(),
        52
    );
    let mut selections = Vec::new();
    for index in 0..52 {
        let mut selected = choose(&input, &details, 0, index);
        selected["video_id"] = json!(format!("video:{index}"));
        selections.push(selected);
    }
    let home = ok(&host, "get_home", home_input(&metadata_source));
    let dto = ok(
        &host,
        "get_details",
        json!({"ref":home["page"]["sections"][0]["items"][0]["ref"]}),
    );
    let initial_context = ok(&host, "match_playback", selections[0].clone())["context"].clone();
    let mut opened = Vec::<Value>::new();
    let mut resolutions = Vec::<Value>::new();
    let mut invoke_count = 0;
    let mut command_counts = std::collections::HashMap::<String, usize>::new();
    let mut unauthorized_query = false;
    if let Some(dir) = directory {
        publish(
            &dir.join("ready.json"),
            &json!({"initial_context":initial_context,"match_inputs":selections,"details":dto,"sources":ok(&host,"get_sources",json!({}))}),
        );
        let started = Instant::now();
        let mut sequence = 0;
        loop {
            sequence += 1;
            let request_path = dir.join(format!("request-{sequence}.json"));
            while !request_path.exists() {
                assert!(
                    started.elapsed() < Duration::from_secs(900),
                    "formal WebView bridge timed out"
                );
                thread::sleep(Duration::from_millis(5));
            }
            let request: Value =
                serde_json::from_slice(&std::fs::read(request_path).unwrap()).unwrap();
            let result = match request["operation"].as_str().unwrap_or("invoke") {
                "finish" => {
                    publish(
                        &dir.join(format!("response-{sequence}.json")),
                        &json!({"ok":true}),
                    );
                    break;
                }
                "mode" => {
                    let request_start = error_requests.lock().unwrap().len();
                    mode.store(
                        request["value"].as_u64().unwrap() as usize,
                        Ordering::SeqCst,
                    );
                    json!({"ok":true,"request_start":request_start})
                }
                "unauthorized-query" => {
                    unauthorized_query = true;
                    json!({"ok":true})
                }
                "expire-session" => {
                    let id = request["session_id"].as_str().unwrap();
                    ok(&host, "close_playback", json!({"session_id":id}));
                    json!({"ok":true})
                }
                "evict-images" => {
                    // Actual 4096-entry gateway LRU, without changing lease clocks.
                    let mut committed = host.committed.lock().unwrap();
                    let catalog = committed.maccms[provider["id"].as_str().unwrap()].clone();
                    let image = catalog.images.values().next().unwrap().clone();
                    let gateway = committed.gateway.as_mut().unwrap();
                    for _ in 0..4096 {
                        let mut image = image.clone();
                        let id = Uuid::new_v4().to_string();
                        image.reference["resource_id"] = json!(id);
                        image.reference["path"] = json!(format!("/images/{id}"));
                        gateway
                            .image(provider["id"].as_str().unwrap(), image)
                            .unwrap();
                    }
                    json!({"ok":true})
                }
                "observe" => {
                    let mut live = 0;
                    for view in &opened {
                        if diagnostic(&host, view)["state"] != "expired" {
                            live += 1;
                        }
                    }
                    let old_status = if opened.len() > 1 {
                        Some(resource(opened[opened.len() - 2]["media_url"].as_str().unwrap()).0)
                    } else {
                        None
                    };
                    json!({"open_total":opened.len(),"live_sessions":live,"requests":host.requests.active_count(),"source_active":upstream.active.load(Ordering::SeqCst),"old_http_status":old_status,"resolutions_issued":resolutions.len(),"source_requests":upstream.requests.lock().unwrap().len(),"command_counts":command_counts,"error_requests":error_requests.lock().unwrap().clone()})
                }
                "invoke" => {
                    let command = request["command"].as_str().unwrap();
                    assert!(crate::contracts::COMMANDS.contains(&command));
                    let args = request["args"].clone();
                    let authorized = !(command == "get_resource_status" && unauthorized_query);
                    if !authorized {
                        unauthorized_query = false;
                    }
                    let mut result = host.invoke(command, args.clone(), authorized);
                    invoke_count += 1;
                    *command_counts.entry(command.to_owned()).or_default() += 1;
                    if command == "resolve_episode" && result.get("data").is_some() {
                        let original = &result["data"];
                        let reference = original["episode_ref"].clone();
                        let selected = host
                            .committed
                            .lock()
                            .unwrap()
                            .resolutions
                            .selected(
                                original["id"].as_str().unwrap(),
                                original["streams"][0]["id"].as_str().unwrap(),
                                clock.now_ms(),
                            )
                            .unwrap();
                        let mut first: Link = selected.link.clone();
                        if trace.is_some() {
                            first.headers.insert(
                                "x-t21-context",
                                Uuid::new_v4().to_string().parse().unwrap(),
                            );
                        }
                        let mut second: Link = first.clone();
                        if trace.is_some() {
                            second.headers.insert(
                                "x-t21-context",
                                Uuid::new_v4().to_string().parse().unwrap(),
                            );
                        }
                        second.label = "Alternate native callback".into();
                        let subtitle = Subtitle {
                            original: json!({"native_callback":true}),
                            url: upstream.url("/subtitle.vtt"),
                            headers: Default::default(),
                            label: "中文".into(),
                            language: Some("zh-CN".into()),
                            format: "vtt".into(),
                        };
                        let mut committed = host.committed.lock().unwrap();
                        let value = committed
                            .resolutions
                            .register(
                                &reference,
                                vec![first, second],
                                vec![subtitle],
                                clock.now_ms(),
                            )
                            .unwrap();
                        committed
                            .resolutions
                            .release(original["id"].as_str().unwrap());
                        committed.current_resolution = value["id"].as_str().map(str::to_owned);
                        resolutions.push(value.clone());
                        result["data"] = value;
                    }
                    Contracts::get()
                        .validate(&format!("Result_{command}"), &result)
                        .unwrap();
                    if command == "open_playback" && result.get("data").is_some() {
                        if let Some(trace) = &trace {
                            let view = &result["data"];
                            let selected = host
                                .committed
                                .lock()
                                .unwrap()
                                .resolutions
                                .selected(
                                    view["resolution_id"].as_str().unwrap(),
                                    view["stream_id"].as_str().unwrap(),
                                    clock.now_ms(),
                                )
                                .unwrap();
                            trace.bind(view, &selected.link);
                        }
                        opened.push(result["data"].clone());
                    }
                    result
                }
                other => panic!("unsupported bridge operation {other}"),
            };
            publish(&dir.join(format!("response-{sequence}.json")), &result);
        }
        // T16 reuses this bridge for its shorter UI scenario.
        let minimum = std::env::var("T12_MIN_OPENS").map_or(51, |value| value.parse().unwrap());
        assert!(opened.len() >= minimum);
        assert!(
            opened
                .iter()
                .all(|view| diagnostic(&host, view)["state"] == "expired")
        );
        for resolution in &resolutions {
            let result = call(&host, "open_playback", open_input(resolution));
            assert_eq!(result["error"]["code"], "SESSION_EXPIRED");
        }
    } else {
        for selection in selections.iter().take(51) {
            let context = ok(&host, "match_playback", selection.clone())["context"].clone();
            let resolution = ok(&host, "resolve_episode", resolve_input(&context));
            let view = ok(&host, "open_playback", open_input(&resolution));
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
            assert_eq!(diagnostic(&host, &view)["state"], "expired");
        }
    }
    assert_eq!(host.requests.active_count(), 0);
    if let Some(trace) = trace {
        upstream.wait_for(|request| request.target.starts_with("/media/seg-"));
        let raw = upstream.requests.lock().unwrap();
        for view in &opened {
            let url = url::Url::parse(view["media_url"].as_str().unwrap()).unwrap();
            let cap = url.query_pairs().find(|(key, _)| key == "cap").unwrap().1;
            assert!(
                raw.iter().all(|request| {
                    !request.target.contains(cap.as_ref())
                        && request
                            .headers
                            .values()
                            .all(|value| !value.contains(cap.as_ref()))
                }),
                "local capability reached the upstream"
            );
        }
        drop(raw);
        assert_eq!(upstream.active.load(Ordering::SeqCst), 0);
        trace.finish(
            host.requests.active_count(),
            upstream.active.load(Ordering::SeqCst),
        );
    }
    println!(
        "T12_VECTOR:{}",
        json!({"kind":"host","invokes":invoke_count,"opens":opened.len(),"closed":true,"active_requests":0,"synthetic_callbacks":true})
    );
}
