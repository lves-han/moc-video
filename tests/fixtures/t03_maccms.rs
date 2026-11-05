//! T0.20: identity and protocol integrity for every JSON source promoted by T0.19.
//! Each test runs per source over its captured responses; real media stays out of scope.
use super::t02_json::{RESPONSES, record, stage};
use super::*;

const PLACEHOLDER: &str = "maccms.fixture.test:0";

fn sources() -> Vec<Value> {
    let data: Value = serde_json::from_str(RESPONSES).unwrap();
    assert_eq!(data["sources"].as_array().unwrap().len(), 8);
    data["sources"].as_array().unwrap().clone()
}

fn config_id(index: usize) -> String {
    format!("{:08x}-1111-4111-8111-111111111111", index + 1)
}

fn port_body(value: &Value, port: u16) -> Reply {
    Reply::raw(
        value
            .to_string()
            .replace(PLACEHOLDER, &format!("maccms.fixture.test:{port}")),
        "application/json",
    )
}

/// Source N replays its own captured responses under `/s/N/...`.
fn serve_all(sources: Vec<Value>) -> Fixture {
    Fixture::new(move |request, port| {
        let index: usize = request.target.split('/').nth(2).unwrap().parse().unwrap();
        port_body(&record(&sources[index], stage(request))["response"], port)
    })
}

/// Serves the given items and answers `ids` with the matching item alone.
fn serve_items(classes: Value, items: Value) -> Fixture {
    Fixture::new(move |request, port| {
        let mut list = items.as_array().unwrap().clone();
        if let Some(id) = request.query.get("ids") {
            list.retain(|item| identifier(&item["vod_id"]).unwrap() == *id);
        }
        let mut value = response(json!(list), classes.clone());
        if let Some(page) = request.query.get("pg") {
            value["page"] = json!(page);
        }
        port_body(&value, port)
    })
}

fn config(id: &str, name: &Value, url: String) -> Value {
    json!({"kind":"maccms","config_id":id,"name":name,"api_url":url,"enabled":true})
}

fn save_configs(host: &Host, configs: Vec<Value>) {
    let mut settings = ok(host, "get_settings", json!({}));
    settings["source_configs"] = json!(configs);
    ok(host, "save_settings", settings);
}

fn sync_config(host: &Host, id: &str) -> Value {
    let result = ok(
        host,
        "sync_catalog",
        json!({"kind":"maccms","config_id":id}),
    );
    result["sources"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["origin"]["config_id"] == id)
        .unwrap()
        .clone()
}

fn catalog_for(host: &Host, id: &str) -> Catalog {
    Catalog::sync(
        host.capture_source(id, false).unwrap(),
        &Http::default(),
        &job(host, "http_source_sync"),
    )
    .unwrap()
}

fn content_ref(provider: &Value, vod_id: &str) -> Value {
    json!({"source_id":provider["id"],"source_revision":provider["source_revision"],
        "data":serde_json::to_string(&["maccms", vod_id]).unwrap()})
}

fn detail_item(source: &Value) -> Value {
    record(source, "detail_id")["response"]["list"][0].clone()
}

fn detail_id(source: &Value) -> String {
    identifier(&detail_item(source)["vod_id"]).unwrap()
}

fn identities(host: &Host, id: &str, provider: &Value, source: &Value) -> Value {
    let catalog = catalog_for(host, id);
    let sections: Vec<_> = catalog
        .sections
        .iter()
        .map(|section| json!([section.id, section.type_id, section.title, section.index]))
        .collect();
    let cards = catalog
        .search(
            &Http::default(),
            "captured-search",
            1,
            &job(host, "default_metadata"),
        )
        .unwrap();
    let reference = content_ref(provider, &detail_id(source));
    let details = &catalog
        .details(&Http::default(), &reference, &job(host, "default_metadata"))
        .unwrap()
        .value["details"];
    let groups: Vec<_> = details["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            let episodes: Vec<_> = group["episodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|episode| json!([episode["id"], episode["ref"]]))
                .collect();
            json!([group["id"], episodes])
        })
        .collect();
    let cards: Vec<_> = cards
        .value
        .as_array()
        .unwrap()
        .iter()
        .map(|card| json!([card["id"], card["ref"]]))
        .collect();
    json!({"source":provider["id"],"revision":provider["source_revision"],"sections":sections,
        "cards":cards,"content":details["id"],"groups":groups})
}

#[test]
fn t03_every_source_identity_is_stable_across_restart_and_independent() {
    let sources = sources();
    let fixture = serve_all(sources.clone());
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings.json");
    let configs: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            config(
                &config_id(index),
                &source["name"],
                fixture.url(&format!("/s/{index}/api")),
            )
        })
        .collect();
    let host = Host::open(path.clone()).unwrap();
    save_configs(&host, configs.clone());
    let mut first = Vec::new();
    let mut handles = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let id = config_id(index);
        let provider = sync_config(&host, &id);
        assert_eq!(
            provider["id"],
            identity("http_source", &["maccms", &id]),
            "H vector"
        );
        assert_eq!(
            provider["source_revision"],
            identity(
                "maccms_revision",
                &[configs[index]["api_url"].as_str().unwrap(), "json", "1"]
            ),
            "H vector"
        );
        assert_eq!(provider["name"], source["name"]);
        first.push(identities(&host, &id, &provider, source));
        handles.push(catalog_for(&host, &id).sections[0].handle.clone());
    }
    let unique = |field: &str| {
        first
            .iter()
            .map(|row| row[field].to_string())
            .collect::<HashSet<_>>()
            .len()
    };
    assert_eq!(unique("source"), sources.len());
    assert_eq!(unique("revision"), sources.len());
    drop(host);
    let restarted = Host::open(path).unwrap();
    for (index, source) in sources.iter().enumerate() {
        let id = config_id(index);
        let provider = sync_config(&restarted, &id);
        assert_eq!(identities(&restarted, &id, &provider, source), first[index]);
        // Section handles belong to one Host run; an old one cannot page the new Host.
        let mut input = home_input(&provider);
        input["section_ref"] = json!(handles[index]);
        error(&call(&restarted, "get_home", input), "BAD_REQUEST");
    }
    assert_eq!(restarted.requests.active_count(), 0);
    println!(
        "T03_FIXTURE:{}",
        json!({"sources":sources.iter().map(|source| &source["name"]).collect::<Vec<_>>(),"online_media":"NOT RUN"})
    );
}

#[test]
fn t03_endpoint_disable_delete_and_readd_invalidate_old_references() {
    let sources = sources();
    let fixture = serve_all(sources.clone());
    for (index, source) in sources.iter().enumerate() {
        let vod_id = detail_id(source);
        let (_temp, host) = setup(fixture.url(&format!("/s/{index}/api")));
        let provider = sync(&host);
        let old = content_ref(&provider, &vod_id);
        ok(&host, "get_details", json!({"ref":old}));
        let details = |reference: &Value| call(&host, "get_details", json!({"ref":reference}));

        let edited = config(
            CONFIG,
            &source["name"],
            fixture.url(&format!("/s/{index}/edited")),
        );
        save_configs(&host, vec![edited.clone()]);
        error(&details(&old), "REVISION_MISMATCH");
        let current = sync(&host);
        assert_eq!(
            current["id"], provider["id"],
            "the UUID keeps the source id"
        );
        assert_ne!(current["source_revision"], provider["source_revision"]);
        error(&details(&old), "REVISION_MISMATCH");
        let reference = content_ref(&current, &vod_id);
        ok(&host, "get_details", json!({"ref":reference}));

        let mut disabled = edited.clone();
        disabled["enabled"] = json!(false);
        save_configs(&host, vec![disabled]);
        error(&details(&reference), "ENGINE_NOT_READY");

        save_configs(&host, vec![]);
        error(&details(&reference), "SOURCE_NOT_FOUND");

        // The source screen gives every added source a new UUID.
        let added = "99999999-1111-4111-8111-111111111111";
        save_configs(
            &host,
            vec![config(
                added,
                &source["name"],
                fixture.url(&format!("/s/{index}/edited")),
            )],
        );
        let readded = sync_config(&host, added);
        assert_ne!(readded["id"], provider["id"]);
        error(&details(&reference), "SOURCE_NOT_FOUND");
        ok(
            &host,
            "get_details",
            json!({"ref":content_ref(&readded, &vod_id)}),
        );

        // ADR-052: the same UUID and endpoint are the same source by the identity formula.
        save_configs(&host, vec![]);
        save_configs(&host, vec![edited]);
        let same = sync(&host);
        assert_eq!(same["id"], current["id"]);
        assert_eq!(same["source_revision"], current["source_revision"]);
        ok(&host, "get_details", json!({"ref":reference}));
        assert_eq!(host.requests.active_count(), 0);
    }
}

#[test]
fn t03_same_name_and_same_id_sources_stay_separate() {
    for source in sources() {
        let name = source["name"].clone();
        let item = |title: &str, path: &str| {
            let mut value = detail_item(&source);
            value["vod_id"] = json!("100");
            value["vod_name"] = json!(title);
            value["vod_play_from"] = json!("线路");
            value["vod_play_url"] = json!(format!("第1集$http://{PLACEHOLDER}/{path}.mp4"));
            value
        };
        let (left, right) = (item("左侧影片", "left"), item("右侧影片", "right"));
        let classes = json!([{"type_id":"1","type_name":"相同分类"}]);
        let fixture = Fixture::new(move |request, port| {
            let list = if request.target.starts_with("/left") {
                &left
            } else {
                &right
            };
            let mut value = response(json!([list]), classes.clone());
            value["page"] = json!(request.query.get("pg").map_or("1", String::as_str));
            port_body(&value, port)
        });
        let (a, b) = (config_id(0), config_id(1));
        let temp = TempDir::new().unwrap();
        let host = Host::open(temp.path().join("settings.json")).unwrap();
        save_configs(
            &host,
            vec![
                config(&a, &name, fixture.url("/left/api")),
                config(&b, &name, fixture.url("/right/api")),
            ],
        );
        let (first, second) = (sync_config(&host, &a), sync_config(&host, &b));
        assert_eq!(first["name"], second["name"]);
        assert_ne!(first["id"], second["id"]);
        assert_ne!(first["source_revision"], second["source_revision"]);
        let (left_catalog, right_catalog) = (catalog_for(&host, &a), catalog_for(&host, &b));
        assert_ne!(left_catalog.sections[0].id, right_catalog.sections[0].id);
        assert_ne!(
            left_catalog.sections[0].handle,
            right_catalog.sections[0].handle
        );
        let left_ref = content_ref(&first, "100");
        let right_ref = content_ref(&second, "100");
        let left_view = &ok(&host, "get_details", json!({"ref":left_ref}))["details"];
        let right_view = &ok(&host, "get_details", json!({"ref":right_ref}))["details"];
        assert_eq!(left_view["title"], "左侧影片");
        assert_eq!(right_view["title"], "右侧影片");
        assert_ne!(left_view["id"], right_view["id"]);
        assert_ne!(left_view["groups"][0]["id"], right_view["groups"][0]["id"]);
        let (left_episode, right_episode) = (
            &left_view["groups"][0]["episodes"][0],
            &right_view["groups"][0]["episodes"][0],
        );
        assert_ne!(left_episode["id"], right_episode["id"]);
        let http = Http::default();
        let resolved = |catalog: &Catalog, reference: &Value| {
            catalog.resolve(&http, reference, &job(&host, "resolve"))
        };
        assert!(
            resolved(&left_catalog, &left_episode["ref"])
                .unwrap()
                .url
                .ends_with("/left.mp4")
        );
        assert!(
            resolved(&right_catalog, &right_episode["ref"])
                .unwrap()
                .url
                .ends_with("/right.mp4")
        );
        // Neither a catalog nor a Host call accepts the other source's reference.
        assert_eq!(
            resolved(&left_catalog, &right_episode["ref"])
                .unwrap_err()
                .code,
            "BAD_REQUEST"
        );
        let mut mixed = left_ref.clone();
        mixed["source_revision"] = second["source_revision"].clone();
        error(
            &call(&host, "get_details", json!({"ref":mixed})),
            "REVISION_MISMATCH",
        );
        let mut input = home_input(&first);
        input["section_ref"] = json!(right_catalog.sections[0].handle);
        error(&call(&host, "get_home", input), "BAD_REQUEST");
    }
}

#[test]
fn t03_repeated_groups_and_episodes_keep_order_and_stay_selectable() {
    for source in sources() {
        let entries = (0..200)
            .map(|n| format!("第{}集$http://{PLACEHOLDER}/route/{n}.mp4", n / 2 + 1))
            .collect::<Vec<_>>()
            .join("#");
        let mut value = detail_item(&source);
        value["vod_id"] = json!("7");
        value["vod_play_from"] = json!("线路$$$线路$$$备用");
        value["vod_play_url"] = json!(format!(
            "{entries}$$${entries}$$$http://{PLACEHOLDER}/route/only.m3u8"
        ));
        let fixture = serve_items(json!([]), json!([value]));
        let (_temp, host) = setup(fixture.url("/api"));
        let provider = sync(&host);
        let catalog = catalog(&host);
        let http = Http::default();
        let mapped = catalog
            .details(
                &http,
                &content_ref(&provider, "7"),
                &job(&host, "default_metadata"),
            )
            .unwrap();
        let groups = mapped.value["details"]["groups"].as_array().unwrap();
        let titles: Vec<_> = groups
            .iter()
            .map(|group| group["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, ["线路", "线路", "备用"]);
        assert_ne!(groups[0]["id"], groups[1]["id"]);
        let mut ids = HashSet::new();
        let mut references = HashSet::new();
        for group in groups {
            for (index, episode) in group["episodes"].as_array().unwrap().iter().enumerate() {
                assert_eq!(episode["index"], index);
                assert!(ids.insert(episode["id"].to_string()));
                assert!(references.insert(episode["ref"].to_string()));
            }
        }
        assert_eq!(ids.len(), 401, "no episode may be lost or merged");
        for group in &groups[..2] {
            let episodes = group["episodes"].as_array().unwrap();
            assert_eq!(episodes.len(), 200);
            for (n, episode) in episodes.iter().enumerate() {
                assert_eq!(episode["label"], format!("第{}集", n / 2 + 1));
            }
            for n in [0, 1, 100, 199] {
                let link = catalog
                    .resolve(&http, &episodes[n]["ref"], &job(&host, "resolve"))
                    .unwrap();
                assert!(
                    link.url.ends_with(&format!("/route/{n}.mp4")),
                    "{}",
                    link.url
                );
                assert!(link.playable);
            }
        }
        assert_eq!(groups[2]["episodes"][0]["label"], "1");
        let link = catalog
            .resolve(
                &http,
                &groups[2]["episodes"][0]["ref"],
                &job(&host, "resolve"),
            )
            .unwrap();
        assert_eq!(link.kind, "hls");
    }
}

#[test]
fn t03_numeric_forms_empty_fields_and_long_titles_are_explicit() {
    let sources = sources();
    let fixture = serve_all(sources.clone());
    for (index, source) in sources.iter().enumerate() {
        let (_temp, host) = setup(fixture.url(&format!("/s/{index}/api")));
        let catalog = catalog(&host);
        let card = |item: &Value| catalog.card(item, &mut vec![]);
        let base = detail_item(source);
        let with = |changes: Value| {
            let mut value = base.clone();
            for (key, field) in changes.as_object().unwrap() {
                value[key] = field.clone();
            }
            value
        };

        let number = card(&with(json!({"vod_id":123}))).unwrap();
        let text = card(&with(json!({"vod_id":"123"}))).unwrap();
        assert_eq!(number["id"], text["id"]);
        assert_eq!(number["ref"], text["ref"]);
        let section = |id: Value| {
            catalog
                .categories(&json!({"class":[{"type_id":id,"type_name":"分类"}]}))
                .unwrap()[0]
                .id
                .clone()
        };
        assert_eq!(section(json!(5)), section(json!("5")));
        for (page, count) in [(json!(2), json!(3)), (json!("2"), json!("3"))] {
            let response = json!({"page":page,"pagecount":count,"list":[]});
            assert!(
                catalog
                    .cards(&response, 2)
                    .unwrap()
                    .value
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                catalog.cards(&response, 1).is_err(),
                "a different page is an error"
            );
        }
        assert!(
            catalog
                .cards(&json!({"pagecount":"多","list":[]}), 1)
                .is_err()
        );

        let empty = card(&with(
            json!({"vod_pic":"","vod_year":"","vod_score":"","vod_remarks":"",
            "vod_isend":"","type_name":null}),
        ))
        .unwrap();
        assert!(empty["poster"].is_null() && empty["year"].is_null() && empty["rating"].is_null());
        assert!(empty["update_label"].is_null() && empty["provider_type"].is_null());
        assert_eq!(empty["status"], "unknown");
        let data = serde_json::to_string(&["maccms", "9"]).unwrap();
        let mapped = catalog
            .map_details(
                &with(json!({"vod_id":"9","vod_content":"","vod_actor":"","vod_director":"","vod_area":"",
                    "vod_lang":"","vod_class":"","vod_play_from":"","vod_play_url":""})),
                &catalog.media_ref(&data),
            )
            .unwrap();
        assert_eq!(mapped.value["details"]["groups"], json!([]));
        assert_eq!(mapped.value["details"]["actors"], json!([]));

        let long = "很长的片名😀".repeat(400);
        assert_eq!(
            card(&with(json!({"vod_name":long}))).unwrap()["title"],
            long
        );
        let long_details = catalog
            .map_details(
                &with(json!({"vod_id":"9","vod_name":long})),
                &catalog.media_ref(&data),
            )
            .unwrap();
        assert_eq!(long_details.value["details"]["title"], long);

        // Fields the adapter needs fail explicitly, so nothing silently disappears.
        assert!(card(&with(json!({"vod_id":""}))).is_err());
        assert!(card(&with(json!({"vod_id":null}))).is_err());
        assert!(card(&with(json!({"vod_name":null}))).is_err());
        for broken in [
            json!({"vod_play_from":"一$$$二","vod_play_url":"http://a.test/1.mp4"}),
            json!({"vod_play_from":"一","vod_play_url":"第1集$http://a.test/1.mp4#"}),
            json!({"vod_play_from":"一","vod_play_url":"第1集$"}),
        ] {
            let item = with(json!({"vod_id":"9"}));
            let mut item = item.clone();
            for (key, field) in broken.as_object().unwrap() {
                item[key] = field.clone();
            }
            assert!(
                catalog
                    .map_details(&item, &catalog.media_ref(&data))
                    .is_err()
            );
        }

        for response in [json!({}), json!({"class":null}), json!({"class":[]})] {
            let sections = catalog.categories(&response).unwrap();
            assert_eq!(sections.len(), 1);
            assert_eq!(sections[0].title, "最近更新");
        }
        assert!(catalog.categories(&json!({"class":"电影"})).is_err());
        assert!(
            catalog
                .categories(&json!({"class":[{"type_id":1,"type_name":"甲"},{"type_id":"1","type_name":"乙"}]}))
                .is_err()
        );
    }
}

#[test]
fn t03_page_tail_and_partial_failures_never_become_silent_empty_lists() {
    for source in sources() {
        let original = record(&source, "list")["response"]["class"]
            .as_array()
            .unwrap()
            .clone();
        assert!(original.len() >= 3, "{}", source["name"]);
        let classes = json!(original[..3]);
        let ids: Vec<String> = original[..3]
            .iter()
            .map(|class| identifier(&class["type_id"]).unwrap())
            .collect();
        let base = detail_item(&source);
        let named = |id: &str| {
            let mut value = base.clone();
            value["vod_id"] = json!(id);
            value["vod_name"] = json!(id);
            value
        };
        let pages = [
            json!([named("甲"), named("乙")]),
            json!([named("丙")]),
            json!([]),
        ];
        let broken = Arc::new(AtomicBool::new(false));
        let all_failed = broken.clone();
        let (paged, failed, html) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
        let fixture = Fixture::new(move |request, port| {
            if request.query.contains_key("wd") {
                return Reply::json(json!({"code":0,"msg":"failed","list":[]}));
            }
            let Some(section) = request.query.get("t") else {
                return Reply::json(
                    json!({"code":1,"page":1,"pagecount":1,"class":classes,"list":[]}),
                );
            };
            if *section == html {
                return Reply::raw("<html>down</html>", "text/html");
            }
            if *section == failed || all_failed.load(Ordering::SeqCst) {
                return Reply {
                    status: 500,
                    ..Reply::raw("failed", "text/plain")
                };
            }
            assert_eq!(*section, paged);
            let page: usize = request.query["pg"].parse().unwrap();
            let mut value =
                json!({"code":1,"page":page,"pagecount":2,"list":pages[(page - 1).min(2)]});
            if page == 9 {
                value["page"] = json!(1);
            }
            port_body(&value, port)
        });
        let (_temp, host) = setup(fixture.url("/api"));
        let provider = sync(&host);
        let home = ok(&host, "get_home", home_input(&provider))["page"].clone();
        assert_eq!(home["sections"].as_array().unwrap().len(), 1);
        assert_eq!(home["sections"][0]["index"], 0);
        let failures = home["failed_sections"].as_array().unwrap();
        assert_eq!(failures.len(), 2);
        assert_eq!(
            (failures[0]["index"].clone(), failures[1]["index"].clone()),
            (json!(1), json!(2))
        );
        assert_eq!(failures[0]["title"], original[1]["type_name"]);
        for failure in failures {
            assert_eq!(failure["error"]["code"], "UPSTREAM_FAILED");
        }

        let handle = home["sections"][0]["section_ref"].clone();
        let mut walked = Vec::new();
        let mut page = 1;
        loop {
            let mut input = home_input(&provider);
            input["section_ref"] = handle.clone();
            input["page"] = json!(page);
            let section = ok(&host, "get_home", input)["page"]["sections"][0].clone();
            assert_eq!(section["index"], 0);
            walked.extend(
                section["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item["title"].clone()),
            );
            if section["next_page"].is_null() {
                assert_eq!(section["has_next"], false);
                break;
            }
            page = section["next_page"].as_u64().unwrap();
        }
        assert_eq!(
            walked,
            [json!("甲"), json!("乙"), json!("丙")],
            "pages keep order without loss"
        );
        assert_eq!(page, 2, "the last page ends the walk");
        let requested = fixture.requests.lock().unwrap();
        assert!(
            !requested
                .iter()
                .any(|request| request.query.get("pg").is_some_and(|pg| pg == "3"))
        );
        drop(requested);
        let mut input = home_input(&provider);
        input["section_ref"] = handle.clone();
        input["page"] = json!(3);
        let tail = ok(&host, "get_home", input.clone())["page"]["sections"][0].clone();
        assert_eq!(tail["items"], json!([]));
        assert_eq!(tail["has_next"], false);
        input["page"] = json!(9);
        error(&call(&host, "get_home", input), "UPSTREAM_FAILED");
        error(
            &call(
                &host,
                "get_home",
                json!({"source_id":provider["id"],"source_revision":provider["source_revision"],
                "page":1,"section_ref":home["failed_sections"][0]["section_ref"]}),
            ),
            "UPSTREAM_FAILED",
        );
        assert!(
            catalog(&host)
                .search(&Http::default(), "任意", 1, &job(&host, "default_metadata"))
                .is_err()
        );
        broken.store(true, Ordering::SeqCst);
        error(
            &call(&host, "get_home", home_input(&provider)),
            "UPSTREAM_FAILED",
        );
        assert_eq!(host.requests.active_count(), 0);
    }
}
