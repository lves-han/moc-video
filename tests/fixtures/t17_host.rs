//! Real Host invoke bridge for the native source manager; it starts with no configured source.
use super::*;

fn publish(path: &std::path::Path, value: &Value) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

fn meta(id: &str) -> Value {
    json!({"id":id,"type":"series","name":format!("Title {id}"),"releaseInfo":"2024","videos":[{"id":"v1","title":"Episode","season":1,"episode":1}]})
}

#[test]
fn t17_host_sources_webview_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let upstream = Fixture::new(move |request, port| {
        if request.target.contains("manifest.json") {
            return Reply::json(
                json!({"id":"tmdb-addon","version":"1.0.0","name":"Metadata fixture","description":"controlled","types":["series"],"resources":["catalog","meta"],"catalogs":[{"id":"shows","name":"Shows","type":"series"}]}),
            );
        }
        if request.target.contains("/catalog/") {
            return Reply::json(json!({"metas":[meta("s1"), meta("s2")]}));
        }
        if request.target.contains("/meta/") {
            return Reply::json(json!({"meta":meta("s1")}));
        }
        if request.target.starts_with("/fail") {
            return Reply {
                status: 500,
                ..Reply::raw("source unavailable", "text/plain")
            };
        }
        Reply::json(response(json!([item(port)]), json!([])))
    });
    let temp = TempDir::new().unwrap();
    let clock = Arc::new(crate::requests::MonotonicClock::default());
    let host = Host::with_clock(temp.path().join("settings.json"), clock).unwrap();
    ok(&host, "runtime_prepare", json!({"target":"application"}));
    publish(
        &directory.join("ready.json"),
        &json!({"manifest":upstream.url("/tmdb/manifest.json"),"maccms":upstream.url("/api?token=T17SECRET"),"second":upstream.url("/api?second=1"),"failing":upstream.url("/fail/api")}),
    );
    let started = Instant::now();
    let (mut sequence, mut get_home) = (0, 0);
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(600),
                "sources bridge timed out"
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
