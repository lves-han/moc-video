//! Production Stremio/Host requests against an owned loopback HTTP fixture.
use super::*;

fn full_meta(kind: &str) -> Value {
    let mut value = meta("原始:详情/😀+%2F", kind);
    value["name"] = json!("中文详情 & 特别篇 😀");
    value["poster"] = json!("https://images.fixture.invalid/poster.png?raw=%2F");
    value["background"] = json!("https://images.fixture.invalid/backdrop.png");
    value["cast"] = json!(["演员甲", "演员乙", "无照片演员"]);
    value["app_extras"] = json!({"cast":[{"name":"演员乙", "character":"<b>角色乙</b>", "photo":"https://images.fixture.invalid/actor.png"}, {"name":"演员甲", "character":"主角", "photo":"https://images.fixture.invalid/poster.png?raw=%2F"}]});
    value["director"] = json!(["导演中文"]);
    value["country"] = json!("中国, 日本");
    value["language"] = json!("中文, 日本語");
    value["runtime"] = json!("2h28min");
    value["imdbRating"] = json!("0");
    value["releaseInfo"] = json!("2024-");
    if kind != "movie" {
        value["videos"] = json!((0..200).map(|index| {
            let season = match index {0 | 1 | 198 => Some(2), 2 => Some(0), 199 => None, _ => Some(1)};
            let episode = match index {0 => 10.0, 1 => 2.0, 2 => 0.5, _ => index as f64};
            json!({"id":format!("raw:{index}/中文?x=%2F&y=$"), "title":format!("第{episode}集 中文"), "season":season, "episode":episode,
                "overview":"<p>中文 &amp; 简介</p>", "thumbnail":format!("https://images.fixture.invalid/episode/{index}.png"),
                "runtime":"45 min", "released":"2026-10-29T08:00:00+08:00", "streams":[{"url":"https://media.invalid/ignored.mp4"}]})
        }).collect::<Vec<_>>());
    }
    value
}
fn vector(raw: &Value, value: &Value, registered: &HashMap<String, Image>) {
    let urls: HashMap<_, _> = registered
        .iter()
        .map(|(id, image)| (id.clone(), image.url.clone()))
        .collect();
    let mut value = value.clone();
    redact(&mut value);
    let mut images: HashMap<String, Value> = registered
        .iter()
        .map(|(id, image)| (id.clone(), image.reference.clone()))
        .collect();
    for image in images.values_mut() {
        redact(image);
    }
    println!(
        "T07_VECTOR:{}",
        json!({"adapter":"stremio", "fixture":true, "synthetic":false, "raw":raw, "reference":value["details"]["ref"], "details":value["details"], "metadata_ref":value["metadata_ref"], "registered":images, "image_urls":urls})
    );
}

#[test]
fn t07_stremio_http_movie_series_anime_seasons_200_and_images() {
    fn restore_expiry(value: &mut Value, original: &HashMap<String, Value>) {
        match value {
            Value::Object(fields)
                if fields.contains_key("resource_id") && fields.contains_key("capability") =>
            {
                value["expires_at"] =
                    original[value["resource_id"].as_str().unwrap()]["expires_at"].clone();
            }
            Value::Object(fields) => {
                for value in fields.values_mut() {
                    restore_expiry(value, original);
                }
            }
            Value::Array(items) => {
                for value in items {
                    restore_expiry(value, original);
                }
            }
            _ => {}
        }
    }
    for kind in ["movie", "series", "anime"] {
        let raw = full_meta(kind);
        let served = raw.clone();
        let fixture = Fixture::new(move |request| {
            if request.target.ends_with("manifest.json") {
                let mut value = manifest(
                    json!(["catalog", "meta"]),
                    json!([{"id":"all", "type":kind}]),
                );
                value["types"] = json!(["movie", "series", "anime"]);
                Reply::json(value)
            } else if request.target.contains("/catalog/") {
                Reply::json(json!({"metas":[served]}))
            } else {
                Reply::json(json!({"meta":served}))
            }
        });
        let (_temp, host) = setup(vec![fixture.url("/configured/manifest.json")]);
        let source = sync(&host);
        let card = first_card(&host, &source);
        let value = ok(&host, "get_details", json!({"ref":card["ref"]}));
        let details = &value["details"];
        assert_eq!(details["kind"], kind);
        assert_eq!(details["id"], card["id"]);
        assert_eq!(details["ref"], card["ref"]);
        assert_eq!(details["title"], "中文详情 & 特别篇 😀");
        assert_eq!(details["rating"], json!({"value":0.0,"scale":10}));
        assert_eq!(details["regions"], json!(["中国", "日本"]));
        assert_eq!(details["languages"], json!(["中文", "日本語"]));
        assert_eq!(details["duration_ms"], 8_880_000);
        assert_eq!(details["actors"][0]["role"], "主角");
        assert_eq!(details["actors"][1]["role"], "角色乙");
        assert_eq!(details["actors"][0]["image"], details["poster"]);
        assert!(details["actors"][2]["role"].is_null());
        assert!(details["actors"][2]["image"].is_null());
        assert_eq!(details["directors"][0]["name"], "导演中文");
        assert!(details["episode_count"].is_null());
        assert!(details["update_label"].is_null());
        assert_eq!(
            details["status"],
            if kind == "movie" {
                "unknown"
            } else {
                "ongoing"
            }
        );
        let episodes: Vec<_> = details["groups"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["episodes"].as_array().unwrap())
            .collect();
        if kind == "movie" {
            assert_eq!(episodes.len(), 1);
            let fields: Vec<String> =
                serde_json::from_str(episodes[0]["ref"]["data"].as_str().unwrap()).unwrap();
            assert_eq!(fields[4], raw["id"]);
            assert!(episodes[0]["season"].is_null());
        } else {
            assert_eq!(details["groups"].as_array().unwrap().len(), 5);
            assert_eq!(episodes.len(), 200);
            assert_eq!(episodes[0]["number"], 10.0);
            assert_eq!(episodes[1]["number"], 2.0);
            assert_eq!(episodes[2]["season"], 0);
            assert_eq!(episodes[2]["number"], 0.5);
            for (raw, episode) in raw["videos"].as_array().unwrap().iter().zip(&episodes) {
                let fields: Vec<String> =
                    serde_json::from_str(episode["ref"]["data"].as_str().unwrap()).unwrap();
                assert_eq!(fields[4], raw["id"]);
                assert_eq!(episode["duration_ms"], 2_700_000);
                assert_eq!(episode["released_at"], "2026-10-29T00:00:00Z");
                assert_eq!(episode["plot"], "中文 & 简介");
            }
            assert_eq!(details["groups"][4]["kind"], "other");
            for group in details["groups"].as_array().unwrap() {
                for (index, episode) in group["episodes"].as_array().unwrap().iter().enumerate() {
                    assert_eq!(episode["index"], index);
                }
            }
        }
        let committed = host.committed.lock().unwrap();
        let cat = &committed.stremio[source["id"].as_str().unwrap()];
        let state = cat.state.lock().unwrap();
        assert_eq!(state.images.len(), if kind == "movie" { 3 } else { 203 });
        crate::details::images(details, &mut |image| {
            assert!(
                state.images[image["resource_id"].as_str().unwrap()].reference == *image,
                "registered image differs from details DTO"
            );
            Ok(())
        })
        .unwrap();
        vector(&raw, &value, &state.images);
        drop(state);
        drop(committed);
        let before = time::OffsetDateTime::now_utc();
        let again = ok(&host, "get_details", json!({"ref":card["ref"]}));
        let after = time::OffsetDateTime::now_utc();
        let mut original_images = HashMap::new();
        crate::details::images(details, &mut |image| {
            original_images.insert(
                image["resource_id"].as_str().unwrap().to_owned(),
                image.clone(),
            );
            Ok(())
        })
        .unwrap();
        let mut stable = again["details"].clone();
        let mut compared = 0;
        crate::details::images(&stable, &mut |image| {
            let original = &original_images[image["resource_id"].as_str().unwrap()];
            let expiry = time::OffsetDateTime::parse(
                image["expires_at"].as_str().unwrap(),
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap();
            let old_expiry = time::OffsetDateTime::parse(
                original["expires_at"].as_str().unwrap(),
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap();
            assert!(expiry > old_expiry, "new DTO did not renew the image lease");
            assert!(
                expiry >= before + time::Duration::minutes(30)
                    && expiry <= after + time::Duration::minutes(30),
                "image lease is not 30 minutes from the new request"
            );
            compared += 1;
            Ok(())
        })
        .unwrap();
        assert!(compared >= original_images.len() && compared > 0);
        restore_expiry(&mut stable, &original_images);
        assert!(
            stable == *details,
            "repeat details changed stable metadata or image identity/path/capability"
        );
        assert_eq!(value["metadata_ref"]["content_ref"], card["ref"]);
        assert!(
            fixture
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|r| !r.target.contains("/stream/"))
        );
        assert_eq!(host.requests.active_count(), 0);
    }
}

#[test]
fn t07_stremio_http_null_metadata_no_season_no_images_and_empty() {
    for kind in ["movie", "series", "anime"] {
        let raw = json!({"id":"unknown", "type":kind, "name":"无季/无年份/无图 中文", "imdbRating":"unknown", "year":"unknown", "runtime":"120", "videos":[]});
        let served = raw.clone();
        let fixture = Fixture::new(move |r| {
            if r.target.ends_with("manifest.json") {
                let mut value = manifest(
                    json!(["catalog", "meta"]),
                    json!([{"id":"empty", "type":kind}]),
                );
                value["types"] = json!(["movie", "series", "anime"]);
                Reply::json(value)
            } else if r.target.contains("/catalog/") {
                Reply::json(json!({"metas":[served]}))
            } else {
                Reply::json(json!({"meta":served}))
            }
        });
        let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
        let source = sync(&host);
        let card = first_card(&host, &source);
        let value = ok(&host, "get_details", json!({"ref":card["ref"]}));
        let details = &value["details"];
        for field in [
            "year",
            "rating",
            "poster",
            "backdrop",
            "plot",
            "duration_ms",
            "update_label",
            "episode_count",
        ] {
            assert!(details[field].is_null(), "{field}");
        }
        for field in [
            "tags",
            "actors",
            "directors",
            "regions",
            "languages",
            "related",
        ] {
            assert_eq!(details[field], json!([]), "{field}");
        }
        assert_eq!(details["status"], "unknown");
        assert_eq!(
            details["groups"].as_array().unwrap().len(),
            usize::from(kind == "movie")
        );
        let committed = host.committed.lock().unwrap();
        let state = committed.stremio[source["id"].as_str().unwrap()]
            .state
            .lock()
            .unwrap();
        assert!(state.images.is_empty());
        vector(&raw, &value, &state.images);
    }
}

#[test]
fn t07_stremio_units_ratings_status_dates_and_identity_negatives() {
    for (raw, expected) in [
        ("120m", Some(7_200_000)),
        ("45 min", Some(2_700_000)),
        ("2h28min", Some(8_880_000)),
        ("0m", Some(0)),
        ("120", None),
        ("unknown", None),
        ("NaN m", None),
        ("-1m", None),
        ("1.5h", None),
        ("18446744073709551615h", None),
        ("9007199254740991m", None),
        ("2m3h", None),
        ("1m2m", None),
    ] {
        assert_eq!(duration_ms(&json!(raw)), expected, "{raw}");
    }
    for date in ["2026-10-29", "2026-10-29T10:00:00", "invalid"] {
        assert_eq!(released_at(&json!(date)), None);
    }
    assert_eq!(
        released_at(&json!("2026-10-29T08:00:00+08:00")).unwrap(),
        "2026-10-29T00:00:00Z"
    );
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
    let source = sync(&host);
    let cat = catalog(&host);
    for score in [
        json!(null),
        json!(""),
        json!("NaN"),
        json!("Infinity"),
        json!(11),
        json!(-1),
    ] {
        let mut raw = meta("score", "series");
        raw["imdbRating"] = score;
        raw["releaseInfo"] = json!("2020-2024");
        let card = cat.card(0, 0, &raw, &mut Output::default()).unwrap();
        let mapped = cat
            .map_details(0, 0, &raw, &card["ref"], &mut Output::default())
            .unwrap();
        assert!(mapped["rating"].is_null());
        assert_eq!(mapped["status"], "completed");
    }
    let card = first_card(&host, &source);
    for mutation in ["source", "revision", "addon", "noncanonical"] {
        let mut reference = card["ref"].clone();
        match mutation {
            "source" => reference["source_id"] = json!("foreign"),
            "revision" => reference["source_revision"] = json!("stale"),
            "addon" => {
                let mut fields: Vec<String> =
                    serde_json::from_str(reference["data"].as_str().unwrap()).unwrap();
                fields[1] = "foreign-addon".into();
                reference["data"] = json!(serde_json::to_string(&fields).unwrap());
            }
            _ => reference["data"] = json!(format!(" {} ", reference["data"].as_str().unwrap())),
        }
        error(
            &call(&host, "get_details", json!({"ref":reference})),
            match mutation {
                "source" => "SOURCE_NOT_FOUND",
                "revision" => "REVISION_MISMATCH",
                _ => "BAD_REQUEST",
            },
        );
    }
    n01_meta_owner_fallback_filters_warnings_and_all_failed();
    n01_localized_video_matching_and_mismatched_ids();
}

#[test]
fn t07_stremio_late_details_cancel_locale_edit_and_revision() {
    for mutation in ["cancel", "locale", "edit", "revision"] {
        let changed = Arc::new(AtomicBool::new(false));
        let revision = changed.clone();
        let fixture = Fixture::new(move |r| {
            if r.target.ends_with("manifest.json") {
                let mut value = standard_manifest();
                if revision.load(Ordering::SeqCst) {
                    value["version"] = json!("2.0.0");
                }
                return Reply::json(value);
            }
            if r.target.contains("/meta/") {
                let mut value = full_meta("movie");
                value["id"] = json!("tt001");
                return Reply::delayed(json!({"meta":value}));
            }
            standard(r)
        });
        let (_temp, host) = setup(vec![fixture.url("/manifest.json")]);
        let source = sync(&host);
        let card = first_card(&host, &source);
        let id = Uuid::new_v4().to_string();
        let worker_host = host.clone();
        let args = json!({"request_id":id, "input":{"ref":card["ref"]}});
        let worker = thread::spawn(move || worker_host.invoke("get_details", args, true));
        fixture.wait_for("/meta/movie/tt001.json");
        match mutation {
            "cancel" => {
                ok(&host, "cancel_request", json!({"target_request_id":id}));
            }
            "locale" => set_locale(&host, "zh-CN"),
            "edit" => configure(&host, vec![fixture.url("/edited/manifest.json")]),
            "revision" => {
                changed.store(true, Ordering::SeqCst);
                let next = sync(&host);
                assert_ne!(next["source_revision"], source["source_revision"]);
            }
            _ => unreachable!(),
        }
        error(&worker.join().unwrap(), "CANCELLED");
        let committed = host.committed.lock().unwrap();
        if let Some(cat) = committed.stremio.get(source["id"].as_str().unwrap()) {
            let state = cat.state.lock().unwrap();
            assert!(state.images.is_empty());
            assert!(
                state
                    .cache
                    .values()
                    .all(|value| value.get("meta").is_none())
            );
        }
        assert_eq!(host.requests.active_count(), 0);
    }
}
