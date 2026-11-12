//! T0.38 built-in TMDB source on the real Host: language choice, self-hosted
//! instance address and stable identity across language changes.
use super::*;

fn tmdb_settings(host: &Host, change: impl FnOnce(&mut Value)) -> Value {
    let mut settings = ok(host, "get_settings", json!({}));
    change(&mut settings);
    ok(host, "save_settings", settings)
}

fn tmdb_source(host: &Host) -> Value {
    ok(host, "get_sources", json!({}))["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["origin"]["config_id"] == settings::TMDB_CONFIG_ID)
        .cloned()
        .expect("built-in TMDB source is registered")
}

#[test]
fn x1_builtin_tmdb_language_instance_and_identity() {
    let fixture = Fixture::new(bilingual);
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    // Fresh settings: TMDB enabled, following the app language, no bundled Stremio entry.
    let fresh = ok(&host, "get_settings", json!({}));
    assert_eq!(fresh["tmdb"], json!({"enabled": true, "instance_url": "https://tmdb.elfhosted.com", "language": "follow"}));
    assert!(fresh["source_configs"].as_array().unwrap().iter().all(|c| c["kind"] == "maccms"));
    assert!(fresh.get("metadata_preset_applied").is_none());
    assert_eq!(tmdb_source(&host)["origin"]["manifest_urls"][0], "https://tmdb.elfhosted.com/en-US/manifest.json");

    // A self-hosted instance under a base path, outside the known public hosts.
    let instance = fixture.url("/custom");
    tmdb_settings(&host, |s| {
        s["source_configs"] = json!([]);
        s["tmdb"]["instance_url"] = json!(instance);
        s["ui_locale"] = json!("zh-CN");
    });
    let synced = ok(&host, "sync_catalog", json!({"kind":"stremio", "config_id": settings::TMDB_CONFIG_ID}));
    let source = tmdb_source(&host);
    assert_eq!(source["runtime_status"], "ready", "{synced}");
    assert_eq!(source["origin"]["manifest_urls"][0], format!("{instance}/en-US/manifest.json"));
    let following_zh = first_card(&host, &source);
    assert_eq!(following_zh["title"], "盗梦空间（合成）", "follow uses the app language");
    let zh_details = ok(&host, "get_details", json!({"ref": following_zh["ref"]}));

    // A chosen language stays fixed when the app language changes.
    tmdb_settings(&host, |s| s["tmdb"]["language"] = json!("en"));
    let chosen_en = first_card(&host, &tmdb_source(&host));
    assert_eq!(chosen_en["title"], "Synthetic same title");
    tmdb_settings(&host, |s| {
        s["tmdb"]["language"] = json!("zh-CN");
        s["ui_locale"] = json!("en");
    });
    let after = tmdb_source(&host);
    let chosen_zh = first_card(&host, &after);
    assert_eq!(chosen_zh["title"], "盗梦空间（合成）", "a chosen language ignores the app language");
    let en_details = ok(&host, "get_details", json!({"ref": chosen_en["ref"]}));

    // History binds to the metadata identity; it stays the same across language changes.
    assert_eq!(after["id"], source["id"]);
    assert_eq!(after["source_revision"], source["source_revision"]);
    assert_eq!(chosen_en["ref"], following_zh["ref"]);
    assert_eq!(zh_details["metadata_ref"], en_details["metadata_ref"]);
    assert_eq!(en_details["metadata_ref"]["identity"]["config_id"], settings::TMDB_CONFIG_ID);
    assert!(fixture.requests.lock().unwrap().iter().any(|r| r.target.starts_with("/custom/zh-CN/")));

    // Old URL-derived keys and newly recorded keys both survive an instance switch.
    let old_key = identity("addon", &[&format!("{instance}/en-US/manifest.json")]);
    tmdb_settings(&host, |s| s["tmdb"]["instance_url"] = json!(fixture.url("/other")));
    ok(&host, "sync_catalog", json!({"kind":"stremio", "config_id": settings::TMDB_CONFIG_ID}));
    let switched = tmdb_source(&host);
    let mut restored = chosen_en["ref"].clone();
    restored["source_revision"] = switched["source_revision"].clone();
    let current = ok(&host, "get_details", json!({"ref": restored}));
    assert_eq!(current["metadata_ref"]["identity"]["addon_key"], en_details["metadata_ref"]["identity"]["addon_key"]);
    restored["data"] = json!(json!(["stremio", old_key, "movie", "tmdb:27205"]).to_string());
    let legacy = ok(&host, "get_details", json!({"ref": restored}));
    assert_eq!(legacy["metadata_ref"]["identity"]["canonical_id"], "27205");

    // Disabling keeps the entry visible as disabled; a user source cannot take its identity.
    tmdb_settings(&host, |s| s["tmdb"]["enabled"] = json!(false));
    assert_eq!(tmdb_source(&host)["runtime_status"], "disabled");
    let mut clash = ok(&host, "get_settings", json!({}));
    clash["source_configs"] = json!([{"kind":"stremio", "config_id": settings::TMDB_CONFIG_ID, "name":"Copy", "enabled":true, "manifest_urls":[fixture.url("/x/manifest.json")]}]);
    error(&call(&host, "save_settings", clash), "BAD_REQUEST");
    let mut bad = ok(&host, "get_settings", json!({}));
    bad["tmdb"]["instance_url"] = json!("ftp://example.com");
    error(&call(&host, "save_settings", bad), "BAD_REQUEST");
    println!("X1_TMDB:{}", json!({"follow": following_zh["title"], "chosen_en": chosen_en["title"], "chosen_zh": chosen_zh["title"], "source_id": after["id"]}));
}
