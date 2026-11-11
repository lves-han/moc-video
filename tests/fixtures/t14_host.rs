//! Real Host invoke/gateway bridge for the native home page, with a controlled Stremio addon.
use super::*;

const SECOND_CONFIG: &str = "0f6a3a50-6b0e-4e6a-9a57-14a2f7d0c1aa";
const PAGE: usize = 100;

fn publish(path: &std::path::Path, value: &Value) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

fn card(index: usize, port: u16) -> Value {
    let name = if index == 3 {
        "An extremely long title that keeps going well past what one poster card can display on a single line".repeat(2)
    } else {
        format!("Title {index}")
    };
    let mut value = json!({"id":format!("series:{index}"),"type":"series","name":name,"releaseInfo":"2024",
        "videos":[{"id":format!("video:{index}"),"title":"Episode","season":1,"episode":1}]});
    if index % 10 != 7 {
        value["poster"] = json!(format!(
            "http://maccms.fixture.test:{port}/poster.png?i={index}"
        ));
    }
    value
}

// mode 0: "broken" fails; 1: it recovers.
fn catalog(target: &str, mode: usize, port: u16) -> Reply {
    if mode == 0 && target.contains("/broken") {
        return Reply {
            status: 500,
            ..Reply::raw("catalog unavailable", "text/plain")
        };
    }
    if target.contains("/broken") {
        return Reply::json(json!({"metas":[card(1000, port), card(1001, port)]}));
    }
    if target.contains("/extras") {
        return Reply::json(
            json!({"metas":[card(2000, port), card(2001, port), card(2002, port)]}),
        );
    }
    let skip = target
        .split("skip=")
        .nth(1)
        .and_then(|rest| rest.split('.').next())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if skip >= 300 {
        return Reply::json(json!({"metas":[]}));
    }
    // Every later page repeats five earlier cards, which Host pagination must remove.
    let start = skip.saturating_sub(if skip > 0 { 5 } else { 0 });
    Reply::json(
        json!({"metas":(start..skip + PAGE).map(|index| card(index, port)).collect::<Vec<_>>()}),
    )
}

#[test]
fn t14_host_home_webview_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let images = png();
    let mode = Arc::new(AtomicUsize::new(0));
    let observed = mode.clone();
    let upstream = Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            let catalogs = ["featured", "broken", "extras"]
                .map(|id| json!({"id":id,"name":id,"type":"series","extra":[{"name":"skip"}]}));
            return Reply::json(
                json!({"id":"tmdb-addon","version":"1.0.0","name":"Metadata fixture","description":"controlled","types":["series"],"resources":["catalog","meta"],"catalogs":catalogs}),
            );
        }
        if request.target.contains("/catalog/") {
            return catalog(&request.target, observed.load(Ordering::SeqCst), port);
        }
        if request.target.contains("/meta/") {
            let index = request.target.split(".json").next().unwrap();
            let digits = index.len() - index.bytes().rev().take_while(u8::is_ascii_digit).count();
            return Reply::json(json!({"meta":card(index[digits..].parse().unwrap(), port)}));
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
        {"kind":"maccms","config_id":SECOND_CONFIG,"name":"Second playback fixture","enabled":true,"api_url":upstream.url("/api?second=1")},
        {"kind":"stremio","config_id":METADATA_CONFIG,"name":"Metadata fixture","enabled":true,"manifest_urls":[upstream.url("/tmdb/manifest.json")]}]);
    ok(&host, "save_settings", settings);
    let provider = sync(&host);
    ok(
        &host,
        "sync_catalog",
        json!({"kind":"maccms","config_id":SECOND_CONFIG}),
    );
    ok(
        &host,
        "sync_catalog",
        json!({"kind":"stremio","config_id":METADATA_CONFIG}),
    );
    ok(&host, "runtime_prepare", json!({"target":"application"}));
    inspect(&host, &provider);
    publish(&directory.join("ready.json"), &json!({"ok":true}));
    let started = Instant::now();
    let mut sequence = 0;
    let mut get_home = 0;
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(600),
                "home bridge timed out"
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
            "mode" => {
                mode.store(
                    request["value"].as_u64().unwrap() as usize,
                    Ordering::SeqCst,
                );
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
            "invoke" => {
                let command = request["command"].as_str().unwrap();
                get_home += usize::from(command == "get_home");
                assert!(crate::contracts::COMMANDS.contains(&command));
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
