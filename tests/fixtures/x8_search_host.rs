//! T0.45 approximate MacCMS search on the real Host: the fixture matches `wd` as a
//! plain substring, like real MacCMS sites; the built-in TMDB source supplies aliases.
use super::*;

const TITLES: [&str; 6] = [
    "复仇者联盟4：终局之战",
    "流浪地球2",
    "流浪地球",
    "庆余年",
    "盗梦空间",
    "The Matrix",
];

fn search_fixture() -> Fixture {
    Fixture::new(|request, port| {
        let path = request.target.split('?').next().unwrap();
        if path.starts_with("/tmdb-x8/") {
            if path.ends_with("manifest.json") {
                return Reply::json(
                    json!({"id": "tmdb-addon", "version": "1.0.0", "name": "TMDB", "description": "fixture",
                    "resources": ["catalog", "meta"], "types": ["movie"], "idPrefixes": ["tmdb:"],
                    "catalogs": [{"type": "movie", "id": "tmdb.top", "name": "Popular", "extra": [{"name": "skip"}]},
                                 {"type": "movie", "id": "tmdb.search", "name": "Search", "extra": [{"name": "search", "isRequired": true}]}]}),
                );
            }
            if request.target.contains("search=Timeout") {
                return Reply {
                    header_delay: std::time::Duration::from_secs(6),
                    ..Reply::json(json!({"metas": []}))
                };
            }
            // Each locale returns its own title for the same content.
            let matrix_query: String =
                url::form_urlencoded::byte_serialize("黑客帝国".as_bytes()).collect();
            let matrix = request.target.contains(&format!("search={matrix_query}"));
            let known = !request.target.contains("search=")
                || request.target.contains("search=Inception")
                || matrix;
            let name = match (matrix, request.target.contains("/zh-CN/")) {
                (true, true) => "黑客帝国",
                (true, false) => "The Matrix",
                (false, true) => "盗梦空间",
                (false, false) => "Inception",
            };
            let id = if matrix { "tmdb:603" } else { "tmdb:27205" };
            return Reply::json(
                json!({"metas": if known { json!([{"id": id, "type": "movie", "name": name, "releaseInfo": "2010"}]) } else { json!([]) }}),
            );
        }
        let names: Vec<&str> = match request.query.get("wd") {
            Some(wd) => TITLES
                .iter()
                .copied()
                .filter(|t| t.contains(wd.as_str()))
                .collect(),
            None => TITLES.to_vec(),
        };
        let items: Vec<Value> = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let mut row = item(port);
                row["vod_id"] = json!(format!("v{i}-{name}"));
                row["vod_name"] = json!(name);
                row["type_name"] = json!("电影");
                row
            })
            .collect();
        Reply::json(response(
            json!(items),
            json!([{"type_id": "1", "type_name": "电影"}]),
        ))
    })
}

fn titles(host: &Host, source: &Value, query: &str) -> Vec<String> {
    ok(host, "search_catalog", json!({"source_id": source["id"], "source_revision": source["source_revision"], "query": query}))["items"]
        .as_array().unwrap().iter().map(|i| i["title"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn x8_approximate_maccms_search_relaxes_ranks_and_uses_tmdb_titles() {
    let fixture = search_fixture();
    let (_temp, host) = setup(fixture.url("/api.php/provide/vod/"));
    let source = sync(&host);
    let wd = |fixture: &Fixture| {
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| r.query.get("wd").cloned())
            .collect::<Vec<_>>()
    };

    assert_eq!(
        titles(&host, &source, "复仇者联盟4"),
        vec!["复仇者联盟4：终局之战"],
        "exact substring still works first"
    );
    fixture.requests.lock().unwrap().clear();
    assert_eq!(
        titles(&host, &source, "复仇者联盟 4"),
        vec!["复仇者联盟4：终局之战"]
    );
    assert_eq!(
        wd(&fixture),
        vec!["复仇者联盟 4", "复仇者联盟4"],
        "spaces removed only after the typed form found nothing"
    );
    assert_eq!(titles(&host, &source, "庆余年 第二季"), vec!["庆余年"]);
    assert_eq!(
        titles(&host, &source, "流浪地球"),
        vec!["流浪地球", "流浪地球2"],
        "closest title first"
    );
    assert_eq!(
        titles(&host, &source, "地"),
        vec!["流浪地球2", "流浪地球"],
        "single-character search reaches MacCMS"
    );

    // Without TMDB ready the English title finds nothing.
    assert!(titles(&host, &source, "Inception").is_empty());
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["tmdb"]["instance_url"] = json!(fixture.url("/tmdb-x8"));
    ok(&host, "save_settings", settings);
    ok(
        &host,
        "sync_catalog",
        json!({"kind": "stremio", "config_id": settings::TMDB_CONFIG_ID}),
    );
    let source = ok(&host, "get_sources", json!({}))["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "maccms")
        .cloned()
        .unwrap();
    assert_eq!(
        titles(&host, &source, "Inception"),
        vec!["盗梦空间"],
        "TMDB supplies the Chinese title"
    );
    assert_eq!(
        titles(&host, &source, "黑客帝国"),
        vec!["The Matrix"],
        "TMDB supplies the original-language title"
    );
    let started = std::time::Instant::now();
    assert!(
        titles(&host, &source, "Timeout").is_empty(),
        "TMDB timeout leaves the MacCMS result intact"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert!(
        titles(&host, &source, "完全不存在的片名").is_empty(),
        "no match stays an empty result"
    );
    println!(
        "X8_SEARCH:{}",
        json!({"relaxed": ["复仇者联盟 4", "庆余年 第二季"], "alias": "Inception→盗梦空间"})
    );
}
