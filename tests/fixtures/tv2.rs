//! TV2 uses the existing real HTTP fixture and production Host boundary.
use super::*;

fn tv_manifest(chinese: bool) -> Value {
    let mut catalogs = Vec::new();
    for kind in ["movie", "series"] {
        for (id, options) in [
            (
                "tmdb.top",
                if chinese {
                    json!(["喜剧", "剧情", "动作冒险"])
                } else {
                    json!(["Drama", "Comedy", "Action & Adventure"])
                },
            ),
            (
                "tmdb.latest",
                if chinese {
                    json!(["喜剧", "剧情", "动作冒险"])
                } else {
                    json!(["Drama", "Comedy", "Action & Adventure"])
                },
            ),
            ("tmdb.year", json!(["2024", "2023"])),
            ("tmdb.language", json!(["Mandarin", "English"])),
            ("tmdb.trending", json!(["Day", "Week"])),
        ] {
            catalogs.push(
                json!({"id":id,"type":kind,"name":if chinese {"中文栏目"} else {"Catalog"},
                "pageSize":20,"extra":[{"name":"genre","options":options},{"name":"skip"}]}),
            );
        }
    }
    let mut value = manifest(json!(["catalog", "meta"]), json!(catalogs));
    value["id"] = json!("tmdb-addon");
    value
}

fn route(request: &Request) -> (String, String, HashMap<String, String>) {
    let url = Url::parse(&format!("http://fixture{}", request.target)).unwrap();
    let parts: Vec<_> = url.path_segments().unwrap().collect();
    let catalog = parts.iter().position(|s| *s == "catalog").unwrap();
    let extra = parts
        .get(catalog + 3)
        .unwrap_or(&"")
        .trim_end_matches(".json");
    (
        parts[catalog + 1].into(),
        parts[catalog + 2].trim_end_matches(".json").into(),
        url::form_urlencoded::parse(extra.as_bytes())
            .into_owned()
            .collect(),
    )
}

fn tv_reply(request: &Request, sparse: bool) -> Reply {
    let chinese = request.target.contains("/zh-CN/");
    if request.target.ends_with("manifest.json") {
        return Reply::json(tv_manifest(chinese));
    }
    let (kind, id, extras) = route(request);
    let skip = extras
        .get("skip")
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(0);
    let requested = extras.get("genre").map(String::as_str);
    let metas: Vec<_> = (skip..skip + 20).map(|i| {
        let genre = if let ("tmdb.top" | "tmdb.latest", Some(genre)) = (id.as_str(), requested) {
            genre
        } else if i % if sparse { 50 } else { 4 } == 0 { if chinese {"剧情"} else {"Drama"} }
        else if chinese {"喜剧"} else {"Comedy"};
        let year = if id == "tmdb.year" {requested.unwrap_or("2024")}
            else if i % 2 == 0 {"2024"} else {"2023"};
        json!({"id":format!("tmdb:{}",i+1),"type":kind,"name":if chinese {"中文影片"} else {"English movie"},
            "year":year,"genres":[genre],"description":if chinese {"中文简介"} else {"English plot"},
            "background":"https://images.example.com/backdrop.jpg","imdbRating":"8.2"})
    }).collect();
    Reply::json(json!({"metas":metas}))
}

fn tv_setup(sparse: bool) -> (Fixture, TempDir, Arc<Host>, Value, Value) {
    let fixture = Fixture::new(move |r| tv_reply(r, sparse));
    let (temp, host) = setup(vec![fixture.url("/tmdb/zh-CN/manifest.json")]);
    let source = sync(&host);
    let data = ok(&host, "get_home", home_input(&source));
    (fixture, temp, host, source, data)
}

fn filtered(host: &Host, source: &Value, section: &Value, filter: Value, page: u64) -> Value {
    let mut input = home_input(source);
    input["section_ref"] = section["section_ref"].clone();
    input["filter"] = filter;
    input["page"] = json!(page);
    ok(host, "get_home", input)["page"]["sections"][0].clone()
}

fn trace(fixture: &Fixture, name: &str) {
    let targets: Vec<_> = fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.target.clone())
        .collect();
    println!("TV2_TRACE:{}", json!({"test":name,"targets":targets}));
}

#[test]
fn tv2_single_conditions_and_priority_routing() {
    let (fixture, _temp, host, source, data) = tv_setup(false);
    let section = &data["page"]["sections"][0];
    for (filter, catalog, genre) in [
        (json!({"genre":"Drama"}), "tmdb.top", "Drama"),
        (json!({"year":"2024"}), "tmdb.year", "2024"),
        (json!({"language":"Mandarin"}), "tmdb.language", "Mandarin"),
        (json!({"year":"2024","genre":"Drama"}), "tmdb.year", "2024"),
        (
            json!({"language":"Mandarin","genre":"Drama"}),
            "tmdb.language",
            "Mandarin",
        ),
        (
            json!({"language":"Mandarin","year":"2024"}),
            "tmdb.language",
            "Mandarin",
        ),
        (
            json!({"language":"Mandarin","year":"2024","genre":"Drama"}),
            "tmdb.language",
            "Mandarin",
        ),
    ] {
        let result = filtered(&host, &source, section, filter.clone(), 1);
        let cards = result["items"].as_array().unwrap();
        assert!(!cards.is_empty());
        for card in cards {
            if filter["genre"] == "Drama" {
                assert!(card["genres"].as_array().unwrap().contains(&json!("Drama")));
            }
            if filter["year"] == "2024" {
                assert_eq!(card["year"], 2024);
            }
        }
        let requests = fixture.requests.lock().unwrap();
        assert!(
            requests
                .iter()
                .filter(|r| r.target.contains("/catalog/"))
                .map(route)
                .any(|(_, id, extras)| id == catalog
                    && extras.get("genre").map(String::as_str) == Some(genre)
                    && extras.contains_key("skip")),
            "filter was not sent"
        );
    }
    let series = &data["page"]["sections"][5];
    filtered(
        &host,
        &source,
        series,
        json!({"genre":"Action & Adventure"}),
        1,
    );
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.target.contains("/catalog/series/"))
            .map(route)
            .any(|(_, id, e)| id == "tmdb.latest"
                && e.get("genre").map(String::as_str) == Some("Action & Adventure"))
    );
    trace(&fixture, "single-priority");
}

#[test]
fn tv2_scan_limit_progress_and_cursor_isolation() {
    let (fixture, _temp, host, source, data) = tv_setup(true);
    let section = &data["page"]["sections"][0];
    let filter = json!({"language":"Mandarin","genre":"Drama"});
    let first = filtered(&host, &source, section, filter.clone(), 1);
    assert_eq!(first["scanned_count"], 100);
    assert_eq!(first["matched_count"], 2);
    assert_eq!(first["items"].as_array().unwrap().len(), 2);
    assert_eq!(first["has_next"], true);
    let mut invalid = home_input(&source);
    invalid["section_ref"] = section["section_ref"].clone();
    invalid["page"] = json!(2);
    invalid["filter"] = json!({"language":"English","genre":"Drama"});
    error(&call(&host, "get_home", invalid.clone()), "BAD_REQUEST");
    filtered(&host, &source, section, invalid["filter"].clone(), 1);
    let second = filtered(&host, &source, section, filter.clone(), 2);
    assert_eq!(second["scanned_count"], 200);
    assert_eq!(second["matched_count"], 4);
    let first_ids: HashSet<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].clone().to_string())
        .collect();
    assert!(
        second["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| !first_ids.contains(&i["id"].to_string()))
    );
    // Reset a different filter without invalidating the original page-3 cursor.
    filtered(&host, &source, section, json!({"genre":"Comedy"}), 1);
    assert_eq!(
        filtered(&host, &source, section, filter.clone(), 3)["scanned_count"],
        300
    );
    let english_pages = fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.target.contains("/catalog/"))
        .map(route)
        .filter(|(_, id, e)| {
            id == "tmdb.language" && e.get("genre").map(String::as_str) == Some("English")
        })
        .count();
    assert_eq!(english_pages, 5);
    trace(&fixture, "scan-cursor");
}

#[test]
fn tv2_localized_values_facets_and_locale_cursors() {
    let (fixture, _temp, host, source, data) = tv_setup(false);
    let section = &data["page"]["sections"][0];
    set_locale(&host, "zh-CN");
    let zh = filtered(&host, &source, section, json!({"genre":"Drama"}), 1);
    assert_eq!(zh["kind"], "movie");
    assert_eq!(zh["title"], "中文栏目");
    assert!(
        zh["filters"]["genre"]["options"]
            .as_array()
            .unwrap()
            .contains(&json!({"value":"Drama","name":"剧情"}))
    );
    assert!(
        zh["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["genres"] == json!(["剧情"]) && c["plot"] == "中文简介")
    );
    let by_name = filtered(&host, &source, section, json!({"genre":"剧情"}), 1);
    assert_eq!(by_name["items"][0]["id"], zh["items"][0]["id"]);
    assert!(
        by_name["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["genres"] == json!(["剧情"]))
    );
    let combo = filtered(
        &host,
        &source,
        section,
        json!({"genre":"剧情","year":"2024"}),
        1,
    );
    assert!(
        combo["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["genres"] == json!(["剧情"]) && c["year"] == 2024)
    );
    let series = &data["page"]["sections"][5];
    filtered(
        &host,
        &source,
        series,
        json!({"genre":"Action & Adventure"}),
        1,
    );
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.target.contains("/catalog/series/"))
            .map(route)
            .any(|(_, id, e)| id == "tmdb.latest"
                && e.get("genre").map(String::as_str) == Some("动作冒险"))
    );
    let mut input = home_input(&source);
    input["section_ref"] = section["section_ref"].clone();
    input["page"] = json!(2);
    input["filter"] = json!({"genre":"Comedy"});
    error(&call(&host, "get_home", input), "BAD_REQUEST");
    let zh2 = filtered(&host, &source, section, json!({"genre":"Drama"}), 2);
    assert_eq!(zh2["scanned_count"], 40);
    set_locale(&host, "en");
    let en = filtered(&host, &source, section, json!({"genre":"Drama"}), 1);
    assert!(
        en["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["genres"] == json!(["Drama"]))
    );
    let en2 = filtered(&host, &source, section, json!({"genre":"Drama"}), 2);
    assert_eq!(en2["scanned_count"], 40);
    trace(&fixture, "localized");
}

#[test]
fn tv2_sort_routing_and_disabled_dimensions() {
    let (fixture, _temp, host, source, data) = tv_setup(false);
    let section = &data["page"]["sections"][0];
    for (sort, id, genre) in [
        ("trending_day", "tmdb.trending", "Day"),
        ("trending_week", "tmdb.trending", "Week"),
        ("popular", "tmdb.top", "Drama"),
        ("latest", "tmdb.latest", "Drama"),
    ] {
        let result = filtered(
            &host,
            &source,
            section,
            json!({"sort":sort,"genre":"Drama"}),
            1,
        );
        assert!(
            result["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["genres"] == json!(["Drama"]))
        );
        assert!(
            fixture
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.target.contains("/catalog/"))
                .map(route)
                .any(|(_, actual, e)| actual == id
                    && e.get("genre").map(String::as_str) == Some(genre))
        );
    }
    assert_eq!(
        section["filters"]["region"],
        json!({"available":false,"options":[]})
    );
    let series = &data["page"]["sections"][5];
    assert_eq!(
        series["filters"]["sort"]["options"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    for filter in [
        json!({"region":"China"}),
        json!({"sort":"popular","language":"Mandarin"}),
        json!({"sort":"latest","year":"2024"}),
        json!({"genre":"undeclared"}),
    ] {
        let mut input = home_input(&source);
        input["section_ref"] = section["section_ref"].clone();
        input["filter"] = filter;
        error(&call(&host, "get_home", input), "BAD_REQUEST");
    }
    let mut input = home_input(&source);
    input["section_ref"] = series["section_ref"].clone();
    input["filter"] = json!({"sort":"popular"});
    error(&call(&host, "get_home", input), "BAD_REQUEST");
    let unavailable = filtered(&host, &source, section, json!({"language":"Mandarin"}), 1);
    assert_eq!(
        unavailable["filters"]["sort"],
        json!({"available":false,"options":[]})
    );
    trace(&fixture, "sort-facets");
}

#[test]
fn tv2_placeholder_empty_and_recovers() {
    let empty = Arc::new(AtomicBool::new(true));
    let flag = empty.clone();
    let fixture = Fixture::new(move |r| {
        if r.target.ends_with("manifest.json") {
            return Reply::json(tv_manifest(false));
        }
        if flag.load(Ordering::SeqCst) {
            return Reply::json(json!({"metas":[{"genres":["No Results"]}]}));
        }
        tv_reply(r, false)
    });
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    let data = ok(&host, "get_home", home_input(&source));
    for section in data["page"]["sections"].as_array().unwrap() {
        assert_eq!(section["items"], json!([]));
        assert_eq!(section["has_next"], false);
        assert_eq!(section["scanned_count"], 0);
        assert_eq!(section["matched_count"], 0);
    }
    empty.store(false, Ordering::SeqCst);
    let recovered = filtered(&host, &source, &data["page"]["sections"][5], json!({}), 1);
    assert!(!recovered["items"].as_array().unwrap().is_empty());
}

#[test]
fn tv2_live_builtin_tmdb_request_samples() {
    let folder = std::env::var_os("TV2_LIVE_EVIDENCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("evidence/acceptance")
                .join(format!("TV2-live-{}", Uuid::new_v4()))
        });
    std::fs::create_dir_all(&folder).unwrap();
    let bundled: Value =
        serde_json::from_str(include_str!("../../src/data/defaultSources.json")).unwrap();
    let configured = Url::parse(bundled[0]["manifest_urls"][0].as_str().unwrap()).unwrap();
    let manifest = language_url(&configured, "en").unwrap();
    let (_temp, host) = setup(vec![manifest.to_string()]);
    let http = Http::default();
    let mut records = Vec::new();
    for (index, (language, kind, id, genre, skip)) in [
        ("en", "movie", "tmdb.top", "Drama", 0),
        ("en", "movie", "tmdb.top", "Drama", 20),
        ("zh-CN", "movie", "tmdb.top", "剧情", 0),
        ("en", "movie", "tmdb.year", "2024", 0),
        ("en", "movie", "tmdb.language", "Mandarin", 0),
        ("en", "series", "tmdb.latest", "Drama", 0),
        ("en", "movie", "tmdb.trending", "Day", 0),
        ("en", "movie", "tmdb.trending", "Week", 0),
        ("en", "movie", "tmdb.latest", "Drama", 0),
    ]
    .into_iter()
    .enumerate()
    {
        let base = language_url(&configured, language).unwrap();
        let mut url = resource_url(&base, "catalog", kind, id, None).unwrap();
        let path = url.path().trim_end_matches(".json");
        let extra = encode_extra(&[
            ("genre".into(), genre.into()),
            ("skip".into(), skip.to_string()),
        ]);
        url.set_path(&format!("{path}/{extra}.json"));
        let response = http
            .get(
                url.clone(),
                HeaderMap::new(),
                &job(&host, "default_metadata"),
                LIMIT,
                false,
                true,
            )
            .unwrap_or_else(|e| panic!("live TMDB {id} {genre} {skip}: {}", e.code));
        let body: Value = serde_json::from_slice(&response.body).unwrap();
        let file = format!("sample-{index}.json");
        std::fs::write(folder.join(&file), &response.body).unwrap();
        let metas = body["metas"].as_array().expect("actual catalog metas");
        assert!(!metas.is_empty(), "actual sample must contain content");
        for meta in metas {
            valid_meta(meta, kind, None, true).unwrap();
            assert!(
                !placeholder(meta),
                "selected actual catalog is a placeholder"
            );
            if matches!(id, "tmdb.top" | "tmdb.latest") {
                assert!(
                    meta["genres"]
                        .as_array()
                        .or_else(|| meta["genre"].as_array())
                        .expect("actual catalog genre array")
                        .contains(&json!(genre))
                );
            }
            if id == "tmdb.year" {
                assert_eq!(meta_year(meta), Some(2024));
            }
        }
        records.push(json!({"requested_url":url.as_str(),"final_url":response.url.as_str(),"body":file,
            "sha256":format!("{:x}",Sha256::digest(&response.body)),"bytes":response.body.len(),"count":metas.len(),
            "received_utc":OffsetDateTime::now_utc().format(&Rfc3339).unwrap()}));
    }
    std::fs::write(
        folder.join("requests.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
    println!("TV2_LIVE_EVIDENCE:{}", folder.display());
}

#[test]
fn tv2_upgrade_adds_once_and_respects_removal() {
    let folder = TempDir::new().unwrap();
    let path = folder.path().join("settings.json");
    let store = settings::SettingsFile { path: path.clone() };
    let mut old = settings::defaults();
    old.as_object_mut()
        .unwrap()
        .remove("metadata_preset_applied");
    old["source_configs"] = json!([{"kind":"maccms", "config_id":CONFIG,
        "name":"Existing playback", "enabled":false, "api_url":"https://example.com/api"}]);
    store.save(&old).unwrap();
    let (_, upgraded) = settings::SettingsFile::load(path.clone()).unwrap();
    assert_eq!(upgraded["metadata_preset_applied"], true);
    assert_eq!(upgraded["source_configs"].as_array().unwrap().len(), 2);
    assert_eq!(upgraded["source_configs"][0], old["source_configs"][0]);
    assert_eq!(upgraded["source_configs"][1]["kind"], "stremio");
    assert_eq!(
        settings::SettingsFile::load(path.clone()).unwrap().1,
        upgraded
    );
    let host = Host::open(path.clone()).unwrap();
    let mut removed = ok(&host, "get_settings", json!({}));
    assert_eq!(removed, upgraded);
    removed["source_configs"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    assert_eq!(ok(&host, "save_settings", removed.clone()), removed);
    drop(host);
    assert_eq!(
        settings::SettingsFile::load(path.clone()).unwrap().1,
        removed
    );
    removed["source_configs"] = json!([]);
    store.save(&removed).unwrap();
    assert_eq!(settings::SettingsFile::load(path).unwrap().1, removed);
}

#[test]
fn tv2_card_fields_and_missing_values() {
    let fixture = Fixture::new(|r| {
        if r.target.ends_with("manifest.json") {
            return Reply::json(standard_manifest());
        }
        let mut full = meta("tt001", "movie");
        full["background"] = json!("https://images.example.com/backdrop.jpg");
        if r.target.contains("/meta/") {
            return Reply::json(json!({"meta":full}));
        }
        let minimal = json!({"id":"tt002", "type":"movie", "name":"Minimal"});
        Reply::json(json!({"metas":[full, minimal]}))
    });
    let (_temp, host) = setup(vec![fixture.url("/a/manifest.json")]);
    let source = sync(&host);
    let data = ok(&host, "get_home", home_input(&source));
    let cards = data["page"]["sections"][0]["items"].as_array().unwrap();
    assert_eq!(cards[0]["plot"], "English plot");
    assert_eq!(cards[0]["genres"], json!(["Drama"]));
    let backdrop = cards[0]["backdrop"]["resource_id"].as_str().unwrap();
    assert!(
        data["images"][backdrop]["url"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:")
    );
    for field in ["backdrop", "plot", "genres", "year", "rating"] {
        assert!(
            cards[1].get(field).is_some(),
            "missing explicit nullable {field}"
        );
        assert!(
            cards[1][field].is_null(),
            "missing field must be null: {field}"
        );
    }
    let details = ok(&host, "get_details", json!({"ref":cards[0]["ref"]}));
    assert_eq!(details["details"]["tags"], json!(["Drama"]));
    assert_eq!(details["details"]["plot"], "English plot");
}

#[test]
fn tv3_catalog_descriptions_zero_content_and_old_compatibility() {
    let fixture = Fixture::new(|r| tv_reply(r, false));
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    let mut input = home_input(&source);
    input["catalog_only"] = json!(true);
    let descriptions = ok(&host, "get_home", input.clone());
    assert_eq!(descriptions["images"], json!({}));
    assert_eq!(
        descriptions["page"]["sections"].as_array().unwrap().len(),
        10
    );
    for section in descriptions["page"]["sections"].as_array().unwrap() {
        assert_eq!(section["items"], json!([]));
        assert_eq!(section["page"], 1);
        assert_eq!(section["has_next"], false);
        assert_eq!(section["next_page"], Value::Null);
        assert_eq!(section["scanned_count"], 0);
        assert_eq!(section["matched_count"], 0);
        assert!(section["section_ref"].as_str().is_some());
    }
    assert_eq!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.target.contains("/catalog/"))
            .count(),
        0
    );
    let mut next = home_input(&source);
    next["section_ref"] = descriptions["page"]["sections"][0]["section_ref"].clone();
    next["page"] = json!(2);
    error(&call(&host, "get_home", next), "BAD_REQUEST");
    input["catalog_only"] = json!(false);
    let full = ok(&host, "get_home", input);
    assert!(
        full["page"]["sections"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| !s["items"].as_array().unwrap().is_empty())
    );
    let omitted = ok(&host, "get_home", home_input(&source));
    // Access renews the image lease; content and pagination must still match.
    fn lease_independent(mut page: Value) -> Value {
        for section in page["sections"].as_array_mut().unwrap() {
            for card in section["items"].as_array_mut().unwrap() {
                for key in ["poster", "backdrop"] {
                    if let Some(image) = card[key].as_object_mut() {
                        let expiry = image.remove("expires_at").unwrap();
                        assert!(OffsetDateTime::parse(expiry.as_str().unwrap(), &Rfc3339).is_ok());
                    }
                }
            }
        }
        page
    }
    assert_eq!(
        lease_independent(omitted["page"].clone()),
        lease_independent(full["page"].clone())
    );
    assert!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.target.contains("/catalog/"))
    );
    trace(&fixture, "tv3-descriptions");
}

#[test]
fn tv3_catalog_null_equivalence_and_invalid_combinations() {
    let fixture = Fixture::new(|r| tv_reply(r, false));
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    let mut input = home_input(&source);
    input["catalog_only"] = json!(true);
    input.as_object_mut().unwrap().remove("section_ref");
    let omitted = ok(&host, "get_home", input.clone());
    input["section_ref"] = Value::Null;
    input["filter"] = Value::Null;
    assert_eq!(ok(&host, "get_home", input.clone()), omitted);
    for (field, value) in [
        ("filter", json!({})),
        ("filter", json!({"genre":"Drama"})),
        (
            "section_ref",
            omitted["page"]["sections"][0]["section_ref"].clone(),
        ),
        ("page", json!(2)),
    ] {
        let mut invalid = input.clone();
        invalid[field] = value;
        error(&call(&host, "get_home", invalid), "BAD_REQUEST");
    }
    assert_eq!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.target.contains("/catalog/"))
            .count(),
        0
    );
}

#[test]
fn tv3_same_reference_filters_concurrent_and_pagination() {
    let fixture = Fixture::new(|r| {
        let mut reply = tv_reply(r, false);
        if r.target.contains("genre=Drama") {
            reply.delay = Duration::from_millis(120);
        }
        reply
    });
    let (_temp, host) = setup(vec![fixture.url("/tmdb/en-US/manifest.json")]);
    let source = sync(&host);
    let mut input = home_input(&source);
    input["catalog_only"] = json!(true);
    let descriptions = ok(&host, "get_home", input);
    let section = &descriptions["page"]["sections"][0];
    let (drama, comedy) = thread::scope(|scope| {
        let slow = scope.spawn(|| filtered(&host, &source, section, json!({"genre":"Drama"}), 1));
        let fast = scope.spawn(|| filtered(&host, &source, section, json!({"genre":"Comedy"}), 1));
        (slow.join().unwrap(), fast.join().unwrap())
    });
    for (value, genre) in [(&drama, "Drama"), (&comedy, "Comedy")] {
        assert!(
            value["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["genres"] == json!([genre]))
        );
        let next = filtered(&host, &source, section, json!({"genre":genre}), 2);
        assert_eq!(next["scanned_count"], 40);
        assert!(
            next["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["genres"] == json!([genre]))
        );
    }
    set_locale(&host, "zh-CN");
    let translated = filtered(&host, &source, section, json!({"genre":"Drama"}), 1);
    assert!(
        translated["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["genres"] == json!(["剧情"]))
    );
    trace(&fixture, "tv3-filter-isolation");
}
