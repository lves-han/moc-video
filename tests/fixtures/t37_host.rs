//! Owned three-line MacCMS responses and actual Host/window transport for T0.37.
use super::*;

fn metadata37() -> Value {
    json!({"id":"line-series","type":"series","name":"Line memory series","releaseInfo":"2026","cast":["Stable actor"],
        "videos":[{"id":"v1","title":"第01集","season":1,"episode":1},{"id":"v2","title":"第02集","season":1,"episode":2},{"id":"v3","title":"第03集","season":1,"episode":3}]})
}

fn raw37(port: u16, mode: usize, id: &str) -> Value {
    let mut raw = item(port);
    raw["vod_id"] = json!(id);
    raw["vod_name"] = json!("Line memory series");
    raw["type_id"] = json!("1");
    raw["type_name"] = json!("国产剧");
    raw["vod_year"] = json!("2026");
    raw["vod_actor"] = json!("Stable actor");
    raw["vod_pic"] = Value::Null;
    let mut names = ["Broken", "Unsupported", "Reliable"];
    let mut routes: Vec<String> = (0..3).map(|route| {
        let mut numbers = vec![1, 2, 3];
        if mode == 3 && route == 2 { numbers.remove(1); }
        if mode == 4 && route == 2 { numbers.swap(0, 1); }
        numbers.into_iter().map(|n| {
            let label = match mode {
                5 => format!("S{}E0{n}", if n == 2 { 2 } else { 1 }),
                6 => String::new(),
                7 => format!("第0{}集", if n == 2 { 1 } else { n }),
                8 => format!("第0{n}集预告"),
                _ => format!("第0{n}集"),
            };
            let url = if route == 1 || mode == 13 && route == 2 { "javascript:unavailable".into() } else {
                format!("http://maccms.fixture.test:{port}/media/{}.mp4?episode={n}&token=T37SECRET", if route == 0 && mode != 12 { "broken" } else { "a" })
            };
            if label.is_empty() { url } else { format!("{label}${url}") }
        }).collect::<Vec<_>>().join("#")
    }).collect();
    if mode == 1 {
        names.swap(0, 2);
        routes.swap(0, 2);
    }
    if mode == 2 {
        names[1] = "Reliable";
    }
    if mode == 10 {
        raw["vod_name"] = json!("Replacement with same vod ID");
    }
    if mode == 11 {
        raw["vod_actor"] = json!("Replacement actor");
    }
    raw["vod_play_from"] = json!(names.join("$$$"));
    raw["vod_play_url"] = json!(routes.join("$$$"));
    raw
}

fn fixture37(media: Option<std::path::PathBuf>, mode: Arc<AtomicUsize>) -> Fixture {
    Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            return Reply::json(
                json!({"id":"t37-metadata","version":"1.0.0","name":"Metadata","description":"owned",
                "types":["series"],"resources":["catalog","meta"],"catalogs":[{"id":"shows","type":"series"}]}),
            );
        }
        if request.target.contains("/catalog/") {
            return Reply::json(json!({"metas":[metadata37()]}));
        }
        if request.target.contains("/meta/") {
            return Reply::json(json!({"meta":metadata37()}));
        }
        if request.target.starts_with("/media/broken.mp4") {
            return Reply::raw("not decodable video", "video/mp4");
        }
        if request.target.starts_with("/media/a.mp4") {
            return Reply::raw(
                media
                    .as_ref()
                    .map(|path| std::fs::read(path.join("a.mp4")).unwrap())
                    .unwrap_or_else(|| vec![0; 64]),
                "video/mp4",
            );
        }
        let current = mode.load(Ordering::SeqCst);
        let id = request
            .query
            .get("ids")
            .map(String::as_str)
            .unwrap_or("show");
        let mut list = vec![raw37(port, current, id)];
        if current == 9 && !request.query.contains_key("ids") {
            list.push(raw37(port, current, "same-title"));
        }
        Reply::json(response(
            json!(list),
            json!([{"type_id":"1","type_name":"国产剧"}]),
        ))
    })
}

fn setup37(fixture: &Fixture) -> (TempDir, Arc<Host>) {
    let (temp, host) = setup(fixture.url("/api"));
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["source_configs"].as_array_mut().unwrap().extend([
        json!({"kind":"stremio","config_id":METADATA_CONFIG,"name":"Metadata","enabled":true,"manifest_urls":[fixture.url("/tmdb/manifest.json")]}),
        json!({"kind":"maccms","config_id":"33333333-3333-4333-8333-333333333333","name":"Alternate","enabled":true,"api_url":fixture.url("/api?alternate=1")})
    ]);
    settings["auto_next"] = json!(false);
    ok(&host, "save_settings", settings);
    prepare37(&host);
    (temp, host)
}

fn prepare37(host: &Host) {
    ok(
        host,
        "sync_catalog",
        json!({"kind":"maccms","config_id":CONFIG}),
    );
    ok(
        host,
        "sync_catalog",
        json!({"kind":"maccms","config_id":"33333333-3333-4333-8333-333333333333"}),
    );
    ok(
        host,
        "sync_catalog",
        json!({"kind":"stremio","config_id":METADATA_CONFIG}),
    );
    ok(host, "runtime_prepare", json!({"target":"application"}));
}

#[test]
fn t37_same_source_route_keeps_history_video_and_revalidates() {
    let mode = Arc::new(AtomicUsize::new(0));
    let fixture = fixture37(None, mode.clone());
    let (_temp, host) = setup37(&fixture);
    let provider = ok(&host, "get_sources", json!({}))["providers"][0].clone();
    let data = inspect(&host, &provider);
    let episode = &data["groups"][0]["episodes"][1];
    let fields: Vec<String> =
        serde_json::from_str(episode["ref"]["data"].as_str().unwrap()).unwrap();
    let video =
        serde_json::to_string(&["maccms_video", &fields[1], &fields[2], &fields[3]]).unwrap();
    let metadata = ok(&host, "get_details", json!({"ref":data["ref"]}))["metadata_ref"].clone();
    let input = json!({"metadata_ref":metadata,"video_id":video,"playback_source":{"source_id":provider["id"],"source_revision":provider["source_revision"]},
        "selection":{"content_ref":data["ref"],"group_id":data["groups"][2]["id"],"episode_id":data["groups"][2]["episodes"][1]["id"]}});
    let matched = ok(&host, "match_playback", input.clone());
    assert_eq!(matched["context"]["metadata_video_id"], video);
    assert_eq!(matched["context"]["group_id"], data["groups"][2]["id"]);
    let resolution = ok(&host, "resolve_episode", resolve_input(&matched["context"]));
    assert_eq!(resolution["streams"][0]["playable"], true);
    mode.store(1, Ordering::SeqCst);
    let refused = call(&host, "match_playback", input);
    assert!(
        refused["error"].is_object(),
        "reordered old selection must be re-inspected"
    );
}

#[test]
fn t37_packaged_host_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let media = std::env::var_os("T12_MEDIA").map(std::path::PathBuf::from);
    assert!(media.is_some());
    let mode = Arc::new(AtomicUsize::new(0));
    let fixture = fixture37(media, mode.clone());
    let (temp, mut host) = setup37(&fixture);
    let publish = |path: &std::path::Path, value: &Value| {
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
        std::fs::rename(temporary, path).unwrap();
    };
    publish(&directory.join("ready.json"), &json!({"ok":true}));
    let started = Instant::now();
    let (mut sequence, mut homes, mut restarts) = (0, 0, 0);
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(started.elapsed() < Duration::from_secs(1500));
            thread::sleep(Duration::from_millis(5));
        }
        let request: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let result = match request["operation"].as_str().unwrap_or("invoke") {
            "finish" => {
                publish(
                    &directory.join(format!("response-{sequence}.json")),
                    &json!({"ok":true}),
                );
                break;
            }
            "mode" => {
                mode.store(
                    request["value"].as_u64().unwrap() as usize,
                    Ordering::SeqCst,
                );
                json!({"ok":true})
            }
            "restart" => {
                assert_eq!(host.requests.active_count(), 0);
                drop(host);
                host = Host::open(temp.path().join("settings.json")).unwrap();
                prepare37(&host);
                restarts += 1;
                json!({"ok":true})
            }
            "trace" => {
                json!({"media":fixture.requests.lock().unwrap().iter().filter(|r|r.target.starts_with("/media/")).map(|r|r.target.split('?').next().unwrap().to_owned()).collect::<Vec<_>>()})
            }
            "invoke" => {
                let command = request["command"].as_str().unwrap();
                assert!(crate::contracts::COMMANDS.contains(&command));
                homes += usize::from(command == "get_home");
                let result = host.invoke(command, request["args"].clone(), true);
                Contracts::get()
                    .validate(&format!("Result_{command}"), &result)
                    .unwrap();
                result
            }
            other => panic!("Unknown T37 bridge operation {other}"),
        };
        publish(
            &directory.join(format!("response-{sequence}.json")),
            &result,
        );
    }
    println!(
        "T14_VECTOR:{}",
        json!({"get_home":homes,"restarts":restarts,"active_requests":host.requests.active_count()})
    );
}
