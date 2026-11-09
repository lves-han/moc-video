//! TV4 exercises real classification mapping, HTTP filtering and same-source matches.
use super::*;

const LIMITED: &str = "88888888-8888-4888-8888-888888888888";
fn classes4(limited: bool) -> Value {
    if limited {
        return json!([{"type_id":"2","type_name":"大陆综艺"}]);
    }
    json!([{"type_id":"1","type_name":"电视剧/国产剧"},{"type_id":"2","type_name":"内地综艺"},
        {"type_id":"3","type_name":"短剧大全"},{"type_id":"4","type_name":"体育赛事"},
        {"type_id":"5","type_name":"伦理片"},{"type_id":"6","type_name":"擦边短剧"},
        {"type_id":"7","type_name":"预告片"},{"type_id":"8","type_name":"电影解说"},{"type_id":"9","type_name":"港台综艺"}])
}

fn media4(port: u16, id: &str) -> Value {
    let fields: Vec<_> = id.split(':').collect();
    let class = fields[0];
    let alternate = fields.get(2) == Some(&"1");
    let mut raw = item(port);
    raw["vod_id"] = json!(id);
    raw["type_id"] = json!(class);
    raw["type_name"] = json!(match class {
        "1" => "国产剧",
        "2" => "大陆综艺",
        "3" => "短剧",
        "4" => "体育赛事",
        "9" => "港台综艺",
        _ => "伦理片",
    });
    raw["vod_name"] = json!(format!("MacCMS {id}"));
    raw["vod_pic"] = if id == "2:1:1" {
        Value::Null
    } else {
        json!(format!(
            "http://maccms.fixture.test:{port}/poster.png?i={class}"
        ))
    };
    raw["vod_year"] = json!(if alternate { "2025" } else { "2026" });
    raw["vod_area"] = json!(if alternate { "香港" } else { "大陆" });
    raw["vod_lang"] = json!(if alternate { "粤语" } else { "普通话" });
    raw["vod_score"] = json!(if alternate { "0" } else { "8.5" });
    raw["vod_remarks"] = json!("更新至第08集 · 很长的更新状态用于省略校验");
    raw["vod_play_from"] = json!("同源线路");
    raw["vod_play_url"] = json!(format!(
        "第1集$http://maccms.fixture.test:{port}/media/a.mp4#第2集$http://maccms.fixture.test:{port}/media/b.mp4"
    ));
    raw
}

fn fixture4(media: Option<std::path::PathBuf>, failure: Arc<AtomicBool>) -> Fixture {
    Fixture::new(move |request, port| {
        if request.target.starts_with("/tmdb/") || request.target.starts_with("/poster.png") {
            return super::tv3::upstream3(request, port, 0);
        }
        if request.target.starts_with("/media/") {
            let name = request
                .target
                .trim_start_matches("/media/")
                .split('?')
                .next()
                .unwrap();
            assert!(["a.mp4", "b.mp4"].contains(&name));
            return Reply::raw(
                media
                    .as_ref()
                    .map(|folder| std::fs::read(folder.join(name)).unwrap())
                    .unwrap_or_else(|| vec![0; 64]),
                "video/mp4",
            );
        }
        if failure.load(Ordering::SeqCst)
            && request
                .query
                .get("ac")
                .is_some_and(|value| value == "detail")
            && !request.query.contains_key("ids")
        {
            return Reply {
                status: 503,
                ..Reply::raw("TV4 controlled source unavailable", "text/plain")
            };
        }
        let page = request
            .query
            .get("pg")
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or(1);
        let list = if let Some(id) = request.query.get("ids") {
            vec![media4(port, id)]
        } else {
            (0..2)
                .map(|n| {
                    media4(
                        port,
                        &format!(
                            "{}:{page}:{n}",
                            request.query.get("t").map(String::as_str).unwrap_or("1")
                        ),
                    )
                })
                .collect()
        };
        let mut value = response(
            json!(list),
            classes4(request.target.starts_with("/limited")),
        );
        value["page"] = json!(page);
        value["pagecount"] = json!(8);
        value["total"] = json!(16);
        Reply::json(value)
    })
}

fn setup4(fixture: &Fixture) -> (TempDir, Arc<Host>, Value) {
    let (temp, host) = setup(fixture.url("/api"));
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["source_configs"].as_array_mut().unwrap().extend([
        json!({"kind":"maccms","config_id":LIMITED,"name":"Limited fixture","enabled":true,"api_url":fixture.url("/limited")}),
        json!({"kind":"stremio","config_id":METADATA_CONFIG,"name":"TMDB fixture","enabled":true,"manifest_urls":[fixture.url("/tmdb/en-US/manifest.json")]})]);
    ok(&host, "save_settings", settings);
    let source = sync(&host);
    for (kind, config) in [("maccms", LIMITED), ("stremio", METADATA_CONFIG)] {
        ok(
            &host,
            "sync_catalog",
            json!({"kind":kind,"config_id":config}),
        );
    }
    ok(&host, "runtime_prepare", json!({"target":"application"}));
    (temp, host, source)
}

#[test]
fn tv4_normalize_eight_sources_and_exclude() {
    let samples: Value = serde_json::from_str(include_str!("t02/public-responses.json")).unwrap();
    let defaults: Value =
        serde_json::from_str(include_str!("../../src/data/defaultSources.json")).unwrap();
    let aliases = [
        ("国产剧", "华语剧"),
        ("内地剧", "华语剧"),
        ("大陆剧", "华语剧"),
        ("大陆综艺", "华语综艺"),
        ("内地综艺", "华语综艺"),
        ("综艺片", "综艺"),
        ("短剧大全", "短剧"),
        ("体育赛事", "体育"),
        ("韩国剧", "韩剧"),
        ("日本剧", "日剧"),
        ("香港剧", "港剧"),
        ("港澳剧", "港剧"),
        ("台湾剧", "台剧"),
        ("泰国剧", "泰剧"),
        ("马泰剧", "泰剧"),
        ("美国剧", "欧美剧"),
        ("记录片", "纪录片"),
        ("电影片", "电影"),
        ("连续剧", "电视剧"),
        ("动漫片", "动漫"),
        ("中国动漫", "国产动漫"),
        ("日本动漫", "日韩动漫"),
        ("动画片", "动画电影"),
        ("动漫电影", "动画电影"),
    ];
    let mut observations = Vec::new();
    for sample in samples["sources"].as_array().unwrap() {
        assert!(
            defaults
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["kind"] == "maccms"
                    && s["name"] == sample["name"]
                    && s["api_url"] == sample["api_url"])
        );
        let classes = sample["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["stage"] == "list")
            .unwrap()["response"]["class"]
            .clone();
        let saved = classes.clone();
        let fixture = Fixture::new(move |_, _| Reply::json(response(json!([]), saved.clone())));
        let (_temp, host) = setup(fixture.url("/api"));
        let catalog = catalog(&host);
        for class in classes.as_array().unwrap() {
            let raw = class["type_name"].as_str().unwrap();
            let leaf = raw.rsplit('/').next().unwrap();
            let actual = catalog
                .sections
                .iter()
                .find(|s| s.type_id == identifier(&class["type_id"]).unwrap());
            if [
                "伦理", "倫理", "福利", "擦边", "预告", "解说", "资讯", "公告", "头条", "搞笑",
                "汽车", "三级", "写真", "两性", "有声", "儿歌", "音乐", "影视",
            ]
            .iter()
            .any(|s| raw.contains(s))
            {
                assert!(actual.is_none(), "excluded {raw}");
            } else {
                let expected = aliases
                    .iter()
                    .find(|(from, _)| *from == leaf)
                    .map(|(_, to)| *to)
                    .unwrap_or(leaf);
                assert_eq!(
                    actual
                        .unwrap_or_else(|| panic!("missing {}: {raw}", sample["name"]))
                        .title,
                    expected,
                    "{}: {raw}",
                    sample["name"]
                );
            }
        }
        observations.push(json!({"source":sample["name"],"input":classes,"normalized":catalog.sections.iter().map(|s|json!({"id":s.type_id,"name":s.title,"category":s.category})).collect::<Vec<_>>()}));
    }
    for raw in ["国产剧", "内地剧", "大陆剧", "电视剧/国产剧"] {
        assert_eq!(normalized_category(raw).unwrap().0, "华语剧");
    }
    for raw in [
        "伦理片",
        "福利",
        "擦边短剧",
        "预告片",
        "资讯",
        "公告",
        "头条",
        "电影解说",
    ] {
        assert!(normalized_category(raw).is_none());
    }
    assert_eq!(observations.len(), 8);
    println!("TV4_CLASSES:{}", json!(observations));
}

#[test]
fn tv4_client_filters_five_pages_and_facets() {
    let fixture = fixture4(None, Arc::new(AtomicBool::new(false)));
    let (_temp, host, source) = setup4(&fixture);
    let home = ok(&host, "get_home", home_input(&source));
    let section = &home["page"]["sections"][1];
    assert_eq!(home["page"]["sections"].as_array().unwrap().len(), 5);
    for key in ["genre", "year", "region", "language"] {
        assert_eq!(section["filters"][key]["available"], true);
    }
    assert_eq!(
        section["filters"]["sort"],
        json!({"available":false,"options":[]})
    );
    fixture.requests.lock().unwrap().clear();
    let mut input = home_input(&source);
    input["section_ref"] = section["section_ref"].clone();
    input["filter"] = json!({"year":"2026","region":"中国大陆","language":"国语","genre":"剧情"});
    let result = ok(&host, "get_home", input.clone());
    let row = &result["page"]["sections"][0];
    assert_eq!(row["scanned_count"], 10);
    assert_eq!(row["matched_count"], 5);
    assert_eq!(row["next_page"], 2);
    assert!(
        row["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["year"] == 2026 && c["rating"]["value"] == 8.5)
    );
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(requests.iter().all(|r| {
        r.query
            .keys()
            .all(|k| ["ac", "t", "pg"].contains(&k.as_str()))
    }));
    drop(requests);
    input["page"] = json!(2);
    let second = ok(&host, "get_home", input.clone());
    assert_eq!(second["page"]["sections"][0]["matched_count"], 3);
    input["page"] = json!(1);
    input["filter"] = json!({"year":"1900"});
    let empty = ok(&host, "get_home", input.clone());
    assert_eq!(empty["page"]["sections"][0]["items"], json!([]));
    input["filter"] = json!({"sort":"popular"});
    error(&call(&host, "get_home", input), "BAD_REQUEST");
    let mut observed = json!({"first":result,"second":second,"empty":empty,"requests":5});
    redact(&mut observed);
    println!("TV4_FILTER:{observed}");
}

fn same_input(host: &Host, source: &Value) -> (Value, Value) {
    let card = ok(host, "get_home", home_input(source))["page"]["sections"][1]["items"][0].clone();
    let details = ok(host, "get_details", json!({"ref":card["ref"]}));
    let id: Vec<String> = serde_json::from_str(
        details["details"]["groups"][0]["episodes"][0]["ref"]["data"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let input = json!({"metadata_ref":details["metadata_ref"],"video_id":json!(["maccms_video",id[1],id[2],id[3]]).to_string(),"playback_source":{"source_id":source["id"],"source_revision":source["source_revision"]},"selection":null});
    (details, input)
}

#[test]
fn tv4_same_source_history_and_binding_negatives() {
    let fixture = fixture4(None, Arc::new(AtomicBool::new(false)));
    let (_temp, host, source) = setup4(&fixture);
    let (details, input) = same_input(&host, &source);
    assert_eq!(details["metadata_ref"]["identity"]["namespace"], "maccms");
    let matched = ok(&host, "match_playback", input.clone());
    assert_eq!(matched["status"], "matched");
    let context = &matched["context"];
    assert_eq!(context["content_ref"], details["details"]["ref"]);
    let resolution = ok(&host, "resolve_episode", resolve_input(context));
    let session = ok(&host, "open_playback", open_input(&resolution));
    let record = json!({"content_id":details["details"]["id"],"metadata_identity":input["metadata_ref"]["identity"],"video_id":input["video_id"],"season_number":null,"episode_number":1.0,
        "title":details["details"]["title"],"kind":details["details"]["kind"],"poster_cache_key":null,"source_id":source["id"],"source_name":source["name"],"position_ms":5000,"duration_ms":12000,"completed":false,"updated_at":1});
    ok(
        &host,
        "save_history",
        json!({"record":record,"session_id":session["session_id"],"match_id":context["match_id"],"playback_epoch":1,"write_sequence":1}),
    );
    assert_eq!(
        ok(&host, "list_history", json!({}))["entries"],
        json!([record])
    );
    for mutation in ["source", "revision", "identity", "video"] {
        let mut invalid = input.clone();
        match mutation {
            "source" => invalid["playback_source"]["source_id"] = json!("wrong"),
            "revision" => invalid["playback_source"]["source_revision"] = json!("wrong"),
            "identity" => invalid["metadata_ref"]["identity"]["canonical_id"] = json!("wrong"),
            _ => invalid["video_id"] = json!("missing"),
        }
        error(&call(&host, "match_playback", invalid), "BAD_REQUEST");
    }
    ok(
        &host,
        "close_playback",
        json!({"session_id":session["session_id"]}),
    );
    ok(
        &host,
        "release_resolution",
        json!({"resolution_id":resolution["id"]}),
    );
    assert_eq!(host.requests.active_count(), 0);
    println!(
        "TV4_SAME_SOURCE:{}",
        json!({"history":record,"negative_cases":4,"matched":true})
    );
}

#[test]
fn tv4_stremio_cross_source_confirmation_kept() {
    let fixture = fixture4(None, Arc::new(AtomicBool::new(false)));
    let (_temp, host, source) = setup4(&fixture);
    let sources = ok(&host, "get_sources", json!({}));
    let metadata = sources["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "stremio")
        .unwrap();
    let card =
        ok(&host, "get_home", home_input(metadata))["page"]["sections"][0]["items"][0].clone();
    let details = ok(&host, "get_details", json!({"ref":card["ref"]}));
    let input = json!({"metadata_ref":details["metadata_ref"],"video_id":"video:0","playback_source":{"source_id":source["id"],"source_revision":source["source_revision"]},"selection":null});
    let result = ok(&host, "match_playback", input.clone());
    assert_eq!(result["status"], "needs_confirmation");
    assert!(result["context"].is_null());
    let mut forged = input;
    forged["metadata_ref"]["identity"]["namespace"] = json!("maccms");
    error(&call(&host, "match_playback", forged), "BAD_REQUEST");
    println!(
        "TV4_CROSS_SOURCE:{}",
        json!({"needs_confirmation":true,"forged_namespace_rejected":true})
    );
}

#[test]
fn tv4_packaged_host_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let media = std::env::var_os("T12_MEDIA").map(std::path::PathBuf::from);
    assert!(media.is_some());
    let failure = Arc::new(AtomicBool::new(false));
    let fixture = fixture4(media, failure.clone());
    let (_temp, host, _source) = setup4(&fixture);
    let publish = |path: &std::path::Path, value: &Value| {
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
        std::fs::rename(temporary, path).unwrap();
    };
    publish(&directory.join("ready.json"), &json!({"ok":true}));
    let start = Instant::now();
    let mut sequence = 0;
    let mut homes = 0;
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(start.elapsed() < Duration::from_secs(1800));
            thread::sleep(Duration::from_millis(5));
        }
        let request: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let result = match request["operation"].as_str().unwrap_or("invoke") {
            "mode" => {
                failure.store(request["value"] == 1, Ordering::SeqCst);
                json!({"ok":true})
            }
            "finish" => {
                publish(
                    &directory.join(format!("response-{sequence}.json")),
                    &json!({"ok":true}),
                );
                break;
            }
            "trace" => {
                json!({"targets":fixture.requests.lock().unwrap().iter().map(|r|r.target.clone()).collect::<Vec<_>>()})
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
            other => panic!("Unsupported TV4 operation {other}"),
        };
        publish(
            &directory.join(format!("response-{sequence}.json")),
            &result,
        );
    }
    println!(
        "T14_VECTOR:{}",
        json!({"get_home":homes,"active_requests":host.requests.active_count()})
    );
}
