//! Replays projected public HTTP responses. Real media remains a separate gate.
use super::*;

pub(super) const RESPONSES: &str = include_str!("t02/public-responses.json");

pub(super) fn record<'a>(source: &'a Value, stage: &str) -> &'a Value {
    source["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["stage"] == stage)
        .unwrap()
}

pub(super) fn stage(request: &Request) -> &'static str {
    if request.query.contains_key("ids") {
        "detail_id"
    } else if request.query.contains_key("wd") {
        if request.query["wd"] == "captured-zero-query" {
            "zero_search"
        } else {
            "search"
        }
    } else if request.query.contains_key("t") {
        if request.query.get("pg").is_some_and(|page| page == "2") {
            "category_page_2"
        } else {
            "category_page_1"
        }
    } else if request.query.get("ac").is_some_and(|ac| ac == "detail") {
        "detail_page"
    } else {
        "list"
    }
}

fn serve_public(source: Value) -> Fixture {
    Fixture::new(move |request, port| {
        let body = record(&source, stage(request))["response"]
            .to_string()
            .replace(
                "maccms.fixture.test:0",
                &format!("maccms.fixture.test:{port}"),
            );
        Reply::raw(body, "application/json")
    })
}

#[test]
fn t02_public_json_list_search_pagination_details_and_complete_groups() {
    let data: Value = serde_json::from_str(RESPONSES).unwrap();
    let mut validated = 0;
    let mut total_episodes = 0;
    for source in data["sources"].as_array().unwrap() {
        let fixture = serve_public(source.clone());
        let (_temp, host) = setup(fixture.url("/api"));
        let catalog = catalog(&host);
        let http = Http::default();
        let list = &record(source, "list")["response"];
        assert_eq!(
            catalog.sections.len(),
            list["class"].as_array().unwrap().len()
        );
        for (section, original) in catalog
            .sections
            .iter()
            .zip(list["class"].as_array().unwrap())
        {
            assert_eq!(section.type_id, identifier(&original["type_id"]).unwrap());
            assert_eq!(section.title, original["type_name"].as_str().unwrap());
        }
        for page in [1, 2] {
            let section = &catalog.sections[0];
            let mapped = catalog
                .home(
                    &http,
                    &json!({"source_id":catalog.source["id"],"source_revision":catalog.source["source_revision"],"page":page,"section_ref":section.handle}),
                    &job(&host, "default_metadata"),
                )
                .unwrap();
            let expected = &record(source, &format!("category_page_{page}"))["response"];
            let actual = &mapped.mapped.value["page"]["sections"][0];
            assert_eq!(actual["page"], page);
            assert_eq!(
                actual["items"].as_array().unwrap().len(),
                expected["list"].as_array().unwrap().len()
            );
            assert_eq!(
                actual["has_next"],
                !expected["list"].as_array().unwrap().is_empty()
                    && number(&expected["pagecount"], 0).unwrap() > page
            );
        }
        for (keyword, stage) in [
            ("captured-search", "search"),
            ("captured-zero-query", "zero_search"),
        ] {
            let mapped = catalog
                .search(&http, keyword, 1, &job(&host, "default_metadata"))
                .unwrap();
            let expected = record(source, stage)["response"]["list"]
                .as_array()
                .unwrap();
            assert_eq!(mapped.value.as_array().unwrap().len(), expected.len());
            for (card, item) in mapped.value.as_array().unwrap().iter().zip(expected) {
                assert_eq!(card["title"], item["vod_name"]);
            }
        }
        let original = &record(source, "detail_id")["response"]["list"][0];
        let reference = catalog.card(original, &mut vec![]).unwrap()["ref"].clone();
        let mapped = catalog
            .details(&http, &reference, &job(&host, "default_metadata"))
            .unwrap();
        let groups = mapped.value["details"]["groups"].as_array().unwrap();
        let names: Vec<_> = original["vod_play_from"]
            .as_str()
            .unwrap()
            .split("$$$")
            .collect();
        let entries: Vec<_> = original["vod_play_url"]
            .as_str()
            .unwrap()
            .split("$$$")
            .collect();
        assert_eq!(groups.len(), names.len());
        assert_eq!(groups.len(), entries.len());
        for ((group, name), entries) in groups.iter().zip(names).zip(entries) {
            assert_eq!(group["title"], name);
            let expected: Vec<_> = entries.split('#').collect();
            let episodes = group["episodes"].as_array().unwrap();
            assert_eq!(episodes.len(), expected.len());
            total_episodes += episodes.len();
            for (index, (episode, original)) in episodes.iter().zip(expected).enumerate() {
                let label = original
                    .split_once('$')
                    .map(|(label, _)| label.to_owned())
                    .unwrap_or_else(|| (index + 1).to_string());
                assert_eq!(episode["index"], index);
                assert_eq!(episode["label"], label);
                assert_eq!(episode["ref"]["source_id"], catalog.source["id"]);
            }
            // Projected suffix and DNS exercise resolution, while all upstream
            // media bytes remain explicitly unverified by this fixture test.
            if entries.split('#').next().unwrap().ends_with(".m3u8") {
                let link = catalog
                    .resolve(&http, &episodes[0]["ref"], &job(&host, "resolve"))
                    .unwrap();
                assert!(link.playable);
                assert_eq!(link.kind, "hls");
            }
        }
        assert_eq!(host.requests.active_count(), 0);
        validated += 1;
    }
    assert_eq!(validated, 8);
    assert!(total_episodes >= 1200);
    println!(
        "T02_FIXTURE:{}",
        json!({"sources":validated,"episodes":total_episodes,"online_media":"NOT VERIFIED"})
    );
}

#[test]
fn t02_minimal_shape_accepts_a_list_without_code_only() {
    let parse = |body: Value| {
        decode(&Response {
            url: "https://public.example/api".parse().unwrap(),
            headers: HeaderMap::new(),
            body: serde_json::to_vec(&body).unwrap(),
        })
    };
    assert!(parse(json!({"pagecount":2,"list":[]})).is_ok());
    assert!(parse(json!({"code":1,"list":[]})).is_ok());
    for rejected in [
        json!({"pagecount":2}),
        json!({"list":"x"}),
        json!({"code":0,"list":[]}),
    ] {
        assert!(parse(rejected).is_err());
    }
}

#[test]
fn t02_public_json_errors_zero_results_and_bounds_remain_explicit() {
    n00_query_search_encoding_and_empty_results();
    n00_metadata_limits_and_foreign_section_refs();
    n00_protocol_numbers_closed_and_errors();
    n00_ambiguous_groups_and_unknown_metadata();
    n00_type_probe_formats_html_unknown_and_bound();
}

#[test]
fn t02_json_media_removed_after_details_returns_no_links() {
    let removed = Arc::new(AtomicBool::new(false));
    let state = removed.clone();
    let fixture = Fixture::new(move |_, port| {
        let mut value = item(port);
        if state.load(Ordering::SeqCst) {
            value["vod_play_from"] = json!("");
            value["vod_play_url"] = json!("");
        }
        Reply::json(response(json!([value]), json!([])))
    });
    let (_temporary, host) = setup(fixture.url("/api"));
    let catalog = catalog(&host);
    let reference = catalog.card(&item(fixture.port), &mut vec![]).unwrap()["ref"].clone();
    let details = catalog
        .details(
            &Http::default(),
            &reference,
            &job(&host, "default_metadata"),
        )
        .unwrap();
    let episode = &details.value["details"]["groups"][0]["episodes"][0]["ref"];
    removed.store(true, Ordering::SeqCst);
    let error = catalog
        .resolve(&Http::default(), episode, &job(&host, "resolve"))
        .unwrap_err();
    assert_eq!(error.code, "UPSTREAM_FAILED");
    assert_eq!(error.details.unwrap()["reason"], "no_links");
    assert_eq!(host.requests.active_count(), 0);
}
