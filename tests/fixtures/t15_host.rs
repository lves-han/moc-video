//! Real Host invoke bridge for the native details page, with a controlled Stremio addon.
use super::*;

fn publish(path: &std::path::Path, value: &Value) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

fn episodes(season: u64, count: u64) -> Vec<Value> {
    (1..=count)
        .map(|episode| json!({"id":format!("video:{season}:{episode}"),"title":"Episode","season":season,"episode":episode}))
        .collect()
}

// The long plot carries markup, which must reach the page as plain text only.
fn meta(id: &str, port: u16) -> Value {
    let poster = format!("http://maccms.fixture.test:{port}/poster.png?id={id}");
    match id {
        "movie:long" => {
            json!({"id":id,"type":"movie","name":"Long movie","releaseInfo":"2021","poster":poster,"genres":["Drama"],
            "description":format!("<b>Bold plot</b><script>secret()</script> {}","word ".repeat(200)),"videos":[{"id":"video:movie","title":"Feature"}]})
        }
        "series:200" => {
            json!({"id":id,"type":"series","name":"Two hundred episodes","releaseInfo":"2022","poster":poster,"videos":episodes(1,200)})
        }
        // No poster, year or plot; a later season and an unnumbered special.
        "series:sparse" => {
            let mut videos = episodes(1, 3);
            videos.extend(episodes(2, 2));
            videos.push(json!({"id":"video:special","title":"Special"}));
            json!({"id":id,"type":"series","name":"Sparse series","videos":videos})
        }
        "series:empty" => {
            json!({"id":id,"type":"series","name":"Nothing to watch","poster":poster,"videos":[]})
        }
        other => panic!("unexpected meta {other}"),
    }
}

#[test]
fn t15_host_details_webview_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let images = png();
    let upstream = Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            return Reply::json(
                json!({"id":"tmdb-addon","version":"1.0.0","name":"Metadata fixture","description":"controlled","types":["movie","series"],"resources":["catalog","meta"],
                "catalogs":[{"id":"films","name":"Films","type":"movie"},{"id":"shows","name":"Shows","type":"series"}]}),
            );
        }
        if request.target.contains("/catalog/movie") {
            return Reply::json(json!({"metas":[meta("movie:long", port)]}));
        }
        if request.target.contains("/catalog/") {
            return Reply::json(
                json!({"metas":[meta("series:200", port), meta("series:sparse", port), meta("series:empty", port)]}),
            );
        }
        if request.target.contains("/meta/") {
            let name = request
                .target
                .rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".json")
                .replace("%3A", ":");
            return Reply::json(json!({"meta":meta(&name, port)}));
        }
        if request.target.starts_with("/poster.png") {
            return Reply::raw(images.clone(), "image/png");
        }
        Reply::json(response(json!([item(port)]), json!([])))
    });
    let clock = Arc::new(crate::requests::MonotonicClock::default());
    let temp = TempDir::new().unwrap();
    let host = Host::with_clock(temp.path().join("settings.json"), clock).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([
        {"kind":"maccms","config_id":CONFIG,"name":"Playback fixture","enabled":true,"api_url":upstream.url("/api?original=a%2Fb")},
        {"kind":"stremio","config_id":METADATA_CONFIG,"name":"Metadata fixture","enabled":true,"manifest_urls":[upstream.url("/tmdb/manifest.json")]}]);
    ok(&host, "save_settings", settings);
    sync(&host);
    ok(
        &host,
        "sync_catalog",
        json!({"kind":"stremio","config_id":METADATA_CONFIG}),
    );
    ok(&host, "runtime_prepare", json!({"target":"application"}));
    publish(&directory.join("ready.json"), &json!({"ok":true}));
    let started = Instant::now();
    let (mut sequence, mut get_home) = (0, 0);
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(600),
                "details bridge timed out"
            );
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
        json!({"get_home":get_home,"active_requests":host.requests.active_count()})
    );
}
