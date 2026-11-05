//! Real Host invoke bridge for sources added from the UI; starts with one stale Cloudstream preference.
use super::*;

fn publish(path: &std::path::Path, value: &Value) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

fn metadata() -> Value {
    let videos: Vec<_> = (0..3)
        .map(|i| json!({"id":format!("video:{i}"),"title":format!("Episode {i}"),"season":1,"episode":i+1}))
        .collect();
    json!({"id":"series:original","type":"series","name":"Same title","releaseInfo":"2024","videos":videos})
}

fn episodes(port: u16) -> Value {
    let mut raw = item(port);
    raw["vod_name"] = json!("Same title");
    raw["vod_year"] = Value::Null;
    raw["vod_play_from"] = json!("线路");
    raw["vod_play_url"] = json!(
        (0..3)
            .map(|i| format!(
                "Episode {i}$http://maccms.fixture.test:{port}/media/{}?episode={i}",
                if i == 1 { "b.mp4" } else { "a.mp4" }
            ))
            .collect::<Vec<_>>()
            .join("#")
    );
    response(json!([raw]), json!([]))
}

#[test]
fn n02_host_sources_webview_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let media = std::path::PathBuf::from(std::env::var_os("T12_MEDIA").unwrap());
    let upstream = Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            return metadata_reply(request).unwrap();
        }
        if request.target.contains("/catalog/") {
            return Reply::json(json!({"metas":[metadata()]}));
        }
        if request.target.contains("/meta/") {
            return Reply::json(json!({"meta":metadata()}));
        }
        if request.target.starts_with("/media/") {
            let name = request
                .target
                .trim_start_matches("/media/")
                .split('?')
                .next()
                .unwrap();
            assert!(!name.contains('/') && !name.contains(".."));
            return Reply::raw(std::fs::read(media.join(name)).unwrap(), "video/mp4");
        }
        if request.target.starts_with("/fail") {
            return Reply {
                status: 500,
                ..Reply::raw("source unavailable", "text/plain")
            };
        }
        if request.target.starts_with("/refuse") {
            return Reply {
                status: 403,
                ..Reply::raw("refused", "text/plain")
            };
        }
        Reply::json(episodes(port))
    });
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings.json");
    let clock = Arc::new(crate::requests::MonotonicClock::default());
    let mut host = Host::with_clock(path.clone(), clock.clone()).unwrap();
    let mut settings = settings::defaults();
    settings["last_playback_source_key"] = json!("c".repeat(64));
    ok(&host, "save_settings", settings);
    ok(&host, "runtime_prepare", json!({"target":"application"}));
    publish(
        &directory.join("ready.json"),
        &json!({"manifest":upstream.url("/tmdb/manifest.json"),"maccms":upstream.url("/api?token=N02SECRET"),"second":upstream.url("/api?second=1"),
            "failing":upstream.url("/fail/api"),"refused":upstream.url("/refuse/api"),"stale_preference":"c".repeat(64)}),
    );
    let started = Instant::now();
    let (mut sequence, mut get_home, mut restarts) = (0, 0, 0);
    loop {
        sequence += 1;
        let request_path = directory.join(format!("request-{sequence}.json"));
        while !request_path.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(900),
                "sources bridge timed out"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let request: Value = serde_json::from_slice(&std::fs::read(request_path).unwrap()).unwrap();
        let result = match request["operation"].as_str().unwrap_or("invoke") {
            "finish" => {
                publish(
                    &directory.join(format!("response-{sequence}.json")),
                    &json!({"ok":true}),
                );
                break;
            }
            "restart" => {
                assert_eq!(host.requests.active_count(), 0);
                drop(host);
                host = Host::with_clock(path.clone(), clock.clone()).unwrap();
                ok(&host, "runtime_prepare", json!({"target":"application"}));
                restarts += 1;
                json!({"ok":true})
            }
            "invoke" => {
                let command = request["command"].as_str().unwrap();
                assert!(crate::contracts::COMMANDS.contains(&command));
                get_home += usize::from(command == "get_home");
                let result = host.invoke(command, request["args"].clone(), true);
                Contracts::get()
                    .validate(&format!("Result_{command}"), &result)
                    .unwrap();
                result
            }
            other => panic!("unsupported bridge operation {other}"),
        };
        publish(
            &directory.join(format!("response-{sequence}.json")),
            &result,
        );
    }
    println!(
        "T14_VECTOR:{}",
        json!({"get_home":get_home,"restarts":restarts,"active_requests":host.requests.active_count()})
    );
}
