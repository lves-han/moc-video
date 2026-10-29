//! Real loopback HTTP through production Host and MacCMS mapping.
use super::*;

fn vector(raw: &Value, value: &Value, registered: &HashMap<String, Image>) {
    let urls: HashMap<_, _> = registered
        .iter()
        .map(|(id, image)| (id.clone(), image.url.clone()))
        .collect();
    let mut images: HashMap<String, Value> = registered
        .iter()
        .map(|(id, image)| (id.clone(), image.reference.clone()))
        .collect();
    let mut value = value.clone();
    redact(&mut value);
    for image in images.values_mut() {
        redact(image);
    }
    println!(
        "T07_VECTOR:{}",
        json!({"adapter":"maccms", "synthetic":false, "fixture":true, "raw":raw, "reference":value["details"]["ref"], "details":value["details"], "registered":images, "image_urls":urls})
    );
}

#[test]
fn t07_maccms_http_routes_200_order_metadata_and_registration() {
    let raw = Arc::new(Mutex::new(Value::Null));
    let saved = raw.clone();
    let fixture = Fixture::new(move |request, port| {
        let mut value = item(port);
        let entries: Vec<_> = (0..200).map(|index| {
            let number = if index == 0 {10} else if index == 1 {2} else {index + 1};
            format!("第{number}集 中文$http://maccms.fixture.test:{port}/video/{index}.mp4?x=$keep%2F&y=%2523")
        }).collect();
        value["vod_play_from"] = json!("同名线路$$$同名线路");
        value["vod_play_url"] = json!(format!(
            "{}$$$国语版$http://maccms.fixture.test:{port}/dub.m3u8?raw=a$b",
            entries.join("#")
        ));
        if request.query.contains_key("ids") {
            *saved.lock().unwrap() = value.clone();
        }
        Reply::json(response(json!([value]), json!([])))
    });
    let (_temp, host) = setup(fixture.url("/api?original=a%2Fb"));
    let source = sync(&host);
    let card =
        ok(&host, "get_home", home_input(&source))["page"]["sections"][0]["items"][0].clone();
    let value = ok(&host, "get_details", json!({"ref":card["ref"]}));
    let details = &value["details"];
    assert_eq!(details["ref"], card["ref"]);
    assert_eq!(details["id"], card["id"]);
    assert_eq!(details["groups"].as_array().unwrap().len(), 2);
    assert_ne!(details["groups"][0]["id"], details["groups"][1]["id"]);
    assert_eq!(
        details["groups"][0]["episodes"].as_array().unwrap().len(),
        200
    );
    assert_eq!(details["groups"][0]["episodes"][0]["label"], "第10集 中文");
    assert_eq!(details["groups"][0]["episodes"][1]["label"], "第2集 中文");
    assert_eq!(details["rating"], json!({"value":8.5,"scale":10}));
    assert_eq!(details["plot"], "剧情 & 内容");
    assert_eq!(details["actors"][0]["name"], "甲");
    assert_eq!(details["directors"][0]["name"], "丙");
    assert_eq!(details["regions"], json!(["中国", "日本"]));
    assert_eq!(details["languages"], json!(["中文", "日语"]));
    assert_eq!(details["status"], "completed");
    assert_eq!(details["update_label"], "全2集");
    assert!(details["episode_count"].is_null());
    let raw = raw.lock().unwrap();
    let cat = catalog(&host);
    let groups = cat.groups(&raw, details["id"].as_str().unwrap()).unwrap();
    let raw_groups: Vec<_> = raw["vod_play_url"].as_str().unwrap().split("$$$").collect();
    for (group_index, group) in details["groups"].as_array().unwrap().iter().enumerate() {
        assert_eq!(group["kind"], "route");
        assert_eq!(group["dub_status"], "unspecified");
        assert!(group["season_number"].is_null());
        let entries: Vec<_> = raw_groups[group_index].split('#').collect();
        for (index, episode) in group["episodes"].as_array().unwrap().iter().enumerate() {
            assert_eq!(episode["index"], index);
            let fields: Vec<String> =
                serde_json::from_str(episode["ref"]["data"].as_str().unwrap()).unwrap();
            assert_eq!(fields[1], raw["vod_id"]);
            assert_eq!(fields[2], group_index.to_string());
            assert_eq!(fields[3], index.to_string());
            assert_eq!(
                fields[4],
                identity(
                    "maccms_entry",
                    &[group["title"].as_str().unwrap(), entries[index]]
                )
            );
            assert_eq!(
                groups[group_index].1[index],
                entries[index].split_once('$').unwrap().1
            );
        }
    }
    let committed = host.committed.lock().unwrap();
    let registered = &committed.maccms[source["id"].as_str().unwrap()].images;
    crate::details::images(details, &mut |image| {
        assert_eq!(
            registered[image["resource_id"].as_str().unwrap()].reference,
            *image
        );
        Ok(())
    })
    .unwrap();
    vector(&raw, &value, registered);
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t07_maccms_http_unknown_null_zero_and_empty_episodes() {
    for (score, expected) in [
        (json!(null), Value::Null),
        (json!(""), Value::Null),
        (json!("NaN"), Value::Null),
        (json!(11), Value::Null),
        (json!(0), json!({"value":0.0,"scale":10})),
    ] {
        let expected_score = score.clone();
        let raw = Arc::new(Mutex::new(Value::Null));
        let saved = raw.clone();
        let fixture = Fixture::new(move |_, _| {
            let value = json!({"vod_id":"empty", "vod_name":"无季/无年份/无图 😀", "vod_year":"未知", "vod_score":score});
            *saved.lock().unwrap() = value.clone();
            Reply::json(response(json!([value]), json!([])))
        });
        let (_temp, host) = setup(fixture.url("/api"));
        let source = sync(&host);
        let card =
            ok(&host, "get_home", home_input(&source))["page"]["sections"][0]["items"][0].clone();
        let value = ok(&host, "get_details", json!({"ref":card["ref"]}));
        let details = &value["details"];
        assert_eq!(details["rating"], expected, "score={expected_score}");
        for field in [
            "year",
            "plot",
            "poster",
            "update_label",
            "duration_ms",
            "episode_count",
            "backdrop",
        ] {
            assert!(details[field].is_null(), "{field}");
        }
        for field in [
            "groups",
            "related",
            "actors",
            "directors",
            "regions",
            "languages",
            "tags",
        ] {
            assert_eq!(details[field], json!([]), "{field}");
        }
        assert_eq!(details["status"], "unknown");
        assert_eq!(details["kind"], "other");
        let committed = host.committed.lock().unwrap();
        let registered = &committed.maccms[source["id"].as_str().unwrap()].images;
        assert!(registered.is_empty());
        vector(&raw.lock().unwrap(), &value, registered);
    }
}

#[test]
fn t07_maccms_source_revision_canonical_and_late_details() {
    let fixture = Fixture::new(standard);
    let (_temp, host) = setup(fixture.url("/api"));
    let source = sync(&host);
    let card =
        ok(&host, "get_home", home_input(&source))["page"]["sections"][0]["items"][0].clone();
    for mutation in ["source", "revision", "noncanonical"] {
        let mut reference = card["ref"].clone();
        match mutation {
            "source" => reference["source_id"] = json!("foreign"),
            "revision" => reference["source_revision"] = json!("stale"),
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
    n00_config_edits_disable_delete_and_readd_cancel_io();
    n00_ambiguous_groups_and_unknown_metadata();
}
