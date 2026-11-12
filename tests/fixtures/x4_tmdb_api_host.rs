//! T0.41 TMDB official API mode on the real Host: catalog/details/search through
//! the shared Stremio path, IDs equal to the addon's, custom endpoints, key errors
//! and key redaction.
use super::*;

const KEY: &str = "x4-secret-v3-key";
const TOKEN: &str = "eyJx4secret.token.sig";

fn tmdb_api_fixture() -> Fixture {
    let image = png();
    Fixture::new(move |request, _port| {
        let authorized = request.query.get("api_key").map(String::as_str) == Some(KEY)
            || request.headers.get("authorization").map(String::as_str) == Some(&format!("Bearer {TOKEN}"));
        let path = request.target.split('?').next().unwrap().to_owned();
        if path.starts_with("/addon/") {
            if path.ends_with("manifest.json") {
                return Reply::json(json!({"id":"tmdb-addon", "version":"1.0.0", "name":"TMDB", "description":"fixture",
                    "resources":["catalog", "meta"], "types":["movie"], "idPrefixes":["tmdb:"],
                    "catalogs":[{"type":"movie", "id":"tmdb.top", "name":"Popular", "extra":[{"name":"skip"}]}]}));
            }
            let movie = json!({"id":"tmdb:27205", "type":"movie", "name":"Inception", "releaseInfo":"2010"});
            return Reply::json(if path.contains("/catalog/") { json!({"metas":[movie]}) } else { json!({"meta":movie}) });
        }
        if path.starts_with("/img/") {
            return Reply::raw(image.clone(), "image/png");
        }
        if !authorized {
            return Reply { status: 401, ..Reply::json(json!({"status_message": "Invalid API key"})) };
        }
        let zh = request.query.get("language").map(String::as_str) == Some("zh-CN");
        let genres = json!({"genres": if zh { json!([{"id":18,"name":"剧情"},{"id":28,"name":"动作"}]) } else { json!([{"id":18,"name":"Drama"},{"id":28,"name":"Action"}]) }});
        let movie = json!({"id":27205,"title": if zh {"盗梦空间"} else {"Inception"},"genre_ids":[18],"release_date":"2010-07-15",
            "poster_path":"/p.jpg","backdrop_path":"/b.jpg","overview":"Dreams","vote_average":8.37});
        let show = json!({"id":1396,"name": if zh {"绝命毒师"} else {"Breaking Bad"},"genre_ids":[18],"first_air_date":"2008-01-20",
            "poster_path":"/s.jpg","backdrop_path":null,"overview":"Chemistry","vote_average":8.9});
        match path.as_str() {
            "/3/genre/movie/list" | "/3/genre/tv/list" => Reply::json(genres),
            "/3/configuration/languages" => Reply::json(json!([{"iso_639_1":"en","english_name":"English"},{"iso_639_1":"zh","english_name":"Mandarin"}])),
            "/3/discover/movie" | "/3/trending/movie/day" | "/3/search/movie" => Reply::json(json!({"results":[movie]})),
            "/3/discover/tv" | "/3/trending/tv/day" | "/3/search/tv" => Reply::json(json!({"results":[show]})),
            "/3/movie/27205" => Reply::json(json!({"id":27205,"title": if zh {"盗梦空间"} else {"Inception"},"overview":"Dreams","release_date":"2010-07-15",
                "runtime":148,"genres":[{"id":18,"name":"Drama"}],"poster_path":"/p.jpg","backdrop_path":"/b.jpg","vote_average":8.4,
                "production_countries":[{"name":"United Kingdom"}],"external_ids":{"imdb_id":"tt1375666"},
                "credits":{"cast":[{"name":"Leonardo DiCaprio","character":"Cobb","profile_path":"/c.jpg"}],"crew":[{"name":"Christopher Nolan","job":"Director"}]}})),
            "/3/tv/1396" => Reply::json(json!({"id":1396,"name":"Breaking Bad","overview":"Chemistry","first_air_date":"2008-01-20","last_air_date":"2013-09-29",
                "status":"Ended","episode_run_time":[47],"genres":[{"id":18,"name":"Drama"}],"poster_path":"/s.jpg","vote_average":8.9,
                "seasons":[{"season_number":1}],"external_ids":{"imdb_id":"tt0903747"},"created_by":[{"name":"Vince Gilligan"}],
                "credits":{"cast":[{"name":"Bryan Cranston","character":"Walter White","profile_path":null}]}})),
            "/3/tv/1396/season/1" => Reply::json(json!({"episodes":[{"episode_number":1,"name":"Pilot","overview":"Start","air_date":"2008-01-20","still_path":"/e.jpg","runtime":58},
                {"episode_number":2,"name":"Cat's in the Bag...","overview":"Next","air_date":"2008-01-27","still_path":null}]})),
            _ => Reply { status: 404, ..Reply::json(json!({"status_message": "missing"})) },
        }
    })
}

fn api_settings(host: &Host, fixture: &Fixture, mode: &str, key: &str, locale: &str) {
    let mut settings = ok(host, "get_settings", json!({}));
    settings["source_configs"] = json!([]);
    settings["ui_locale"] = json!(locale);
    settings["tmdb"] = json!({"enabled": true, "instance_url": fixture.url("/addon"), "language": "follow",
        "mode": mode, "api_key": key, "api_url": fixture.url("/3"), "image_url": fixture.url("/img")});
    ok(host, "save_settings", settings);
}

fn tmdb(host: &Host) -> Value {
    ok(host, "get_sources", json!({}))["providers"].as_array().unwrap().iter()
        .find(|s| s["origin"]["config_id"] == settings::TMDB_CONFIG_ID).cloned().unwrap()
}

fn titles(home: &Value) -> Vec<String> {
    home["page"]["sections"].as_array().unwrap().iter()
        .flat_map(|s| s["items"].as_array().unwrap().iter().map(|i| i["title"].as_str().unwrap().to_owned())).collect()
}

#[test]
fn x4_tmdb_api_catalog_details_identity_endpoints_and_secrets() {
    let fixture = tmdb_api_fixture();
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let mut seen = String::new();
    let mut record = |value: &Value| seen.push_str(&value.to_string());

    api_settings(&host, &fixture, "stremio", KEY, "en");
    ok(&host, "sync_catalog", json!({"kind":"stremio", "config_id": settings::TMDB_CONFIG_ID}));
    let addon_source = tmdb(&host);
    let addon_home = ok(&host, "get_home", json!({"source_id": addon_source["id"], "source_revision": addon_source["source_revision"], "page": 1}));
    let addon_ref = addon_home["page"]["sections"][0]["items"][0]["ref"].clone();
    let addon_details = ok(&host, "get_details", json!({"ref": addon_ref}));
    let stremio_id = tmdb(&host)["id"].clone();
    api_settings(&host, &fixture, "api", KEY, "en");
    let synced = ok(&host, "sync_catalog", json!({"kind":"stremio", "config_id": settings::TMDB_CONFIG_ID}));
    record(&synced);
    let source = tmdb(&host);
    record(&source);
    assert_eq!(source["runtime_status"], "ready", "{synced}");
    assert_eq!(source["id"], stremio_id, "both modes are the same metadata source");
    assert_eq!(source["origin"]["manifest_urls"][0], "https://tmdb-api.invalid/en-US/manifest.json");

    let home = ok(&host, "get_home", json!({"source_id": source["id"], "source_revision": source["source_revision"], "page": 1}));
    record(&home);
    let names = titles(&home);
    assert!(names.contains(&"Inception".to_owned()) && names.contains(&"Breaking Bad".to_owned()), "{names:?}");
    let cards: Vec<Value> = home["page"]["sections"].as_array().unwrap().iter().flat_map(|s| s["items"].as_array().unwrap().clone()).collect();
    let movie = cards.iter().find(|c| c["title"] == "Inception").unwrap();
    let show = cards.iter().find(|c| c["title"] == "Breaking Bad").unwrap();
    assert!(movie["ref"].to_string().contains("tmdb:27205"), "{}", movie["ref"]);
    let poster = &home["images"][movie["poster"]["resource_id"].as_str().unwrap()]["url"];
    assert_eq!(resource(poster.as_str().unwrap()).0, 200, "posters load through the gateway from the image address");

    let movie_details = ok(&host, "get_details", json!({"ref": movie["ref"]}));
    let mut history_ref = addon_ref.clone();
    history_ref["source_revision"] = source["source_revision"].clone();
    let restored = ok(&host, "get_details", json!({"ref": history_ref}));
    assert_eq!(restored["metadata_ref"]["identity"], addon_details["metadata_ref"]["identity"]);
    history_ref["data"] = json!(json!(["stremio", identity("addon", &[&fixture.url("/addon/en-US/manifest.json")]), "movie", "tmdb:27205"]).to_string());
    assert_eq!(ok(&host, "get_details", json!({"ref": history_ref}))["details"]["title"], "Inception", "old URL-based history opens in API mode");
    let show_details = ok(&host, "get_details", json!({"ref": show["ref"]}));
    record(&movie_details);
    record(&show_details);
    assert_eq!(movie_details["metadata_ref"]["identity"]["namespace"], "tmdb");
    assert_eq!(movie_details["details"]["directors"][0]["name"], "Christopher Nolan");
    assert_eq!(movie_details["details"]["actors"][0]["role"], "Cobb");
    let episodes = show_details["details"]["groups"][0]["episodes"].as_array().unwrap();
    assert_eq!(episodes.len(), 2);
    assert!(show_details.to_string().contains("tt0903747:1:1"), "series video IDs follow the addon's IMDb form");

    let search = ok(&host, "search_catalog", json!({"source_id": source["id"], "source_revision": source["source_revision"], "query": "Inception"}));
    record(&search);
    assert!(search.to_string().contains("tmdb:27205"));

    api_settings(&host, &fixture, "api", KEY, "zh-CN");
    let zh = ok(&host, "get_home", json!({"source_id": source["id"], "source_revision": tmdb(&host)["source_revision"], "page": 1}));
    assert!(titles(&zh).contains(&"盗梦空间".to_owned()), "follows the app language");

    // A v4 read access token goes in the Authorization header.
    api_settings(&host, &fixture, "api", TOKEN, "en");
    let token = call(&host, "sync_catalog", json!({"kind":"stremio", "config_id": settings::TMDB_CONFIG_ID}));
    record(&token);
    assert!(token.get("error").is_none(), "{token}");

    api_settings(&host, &fixture, "api", "wrong-key", "en");
    let rejected = call(&host, "sync_catalog", json!({"kind":"stremio", "config_id": settings::TMDB_CONFIG_ID}));
    record(&rejected);
    record(&tmdb(&host));
    let failure = if rejected.get("error").is_some() { rejected["error"].clone() } else { tmdb(&host)["error"].clone() };
    assert_eq!(failure["code"], "UNAUTHORIZED", "{rejected}");
    assert_eq!(failure["message"], "TMDB API key was rejected");

    let requests = fixture.requests.lock().unwrap();
    assert!(requests.iter().any(|r| r.target.starts_with("/3/discover/")) && requests.iter().any(|r| r.target.starts_with("/img/")), "custom endpoints are used");
    drop(requests);
    for secret in [KEY, TOKEN, "wrong-key"] {
        assert!(!seen.contains(secret), "the API key leaked into a response");
    }
    println!("X4_TMDB_API:{}", json!({"titles": names, "episodes": episodes.len(), "error": failure["code"], "same_source": true}));
}
