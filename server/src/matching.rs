use serde_json::{Value, json};
use std::collections::HashSet;

pub fn queries(metadata: &Value) -> Vec<String> {
    let title = metadata["title"].as_str().unwrap_or("").trim();
    let mut output = Vec::new();
    if !title.is_empty() {
        output.push(title.to_owned());
    }
    let alternatives = metadata["original_title"]
        .as_str()
        .into_iter()
        .chain(metadata["english_title"].as_str())
        .chain(
            metadata["aliases"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        );
    for candidate in alternatives {
        let candidate = candidate.trim();
        if !candidate.is_empty()
            && !output.iter().any(|value| {
                moc_core::matching::normalize(value) == moc_core::matching::normalize(candidate)
            })
        {
            output.push(candidate.to_owned());
            break;
        }
    }
    output.truncate(2);
    output
}

fn year(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| (1000..=9999).contains(n))
        .or_else(|| {
            value
                .as_str()
                .and_then(|s| s.get(..4))
                .and_then(|s| s.parse().ok())
                .filter(|n| (1000..=9999).contains(n))
        })
}

fn title_year(title: &str) -> (&str, Option<u32>) {
    let trimmed = title
        .trim()
        .trim_end_matches(|c: char| matches!(c, ')' | ']' | '）' | '】') || c.is_whitespace());
    if trimmed.len() < 4 {
        return (title, None);
    }
    if !trimmed.as_bytes()[trimmed.len() - 4..]
        .iter()
        .all(u8::is_ascii_digit)
    {
        return (title, None);
    }
    let suffix = &trimmed[trimmed.len() - 4..];
    let Some(year) = suffix
        .parse::<u32>()
        .ok()
        .filter(|n| (1000..=9999).contains(n))
    else {
        return (title, None);
    };
    let base = trimmed[..trimmed.len() - 4].trim_end_matches(|c: char| {
        matches!(c, '(' | '[' | '（' | '【' | '-' | '_') || c.is_whitespace()
    });
    (base, Some(year))
}

fn source_kind(value: &str) -> Option<&'static str> {
    let value = value.trim().to_ascii_lowercase();
    if value.contains("电影")
        || matches!(
            value.as_str(),
            "movie"
                | "film"
                | "动作片"
                | "喜剧片"
                | "剧情片"
                | "科幻片"
                | "恐怖片"
                | "爱情片"
                | "战争片"
                | "纪录片"
                | "动画片"
        )
    {
        Some("movie")
    } else if value.contains("电视剧")
        || value.contains("连续剧")
        || value.contains("剧集")
        || value.ends_with('剧')
        || matches!(value.as_str(), "series" | "tv")
    {
        Some("series")
    } else {
        None
    }
}

fn derived(title: &str, category: &str) -> bool {
    [
        "解说", "解說", "预告", "預告", "花絮", "片段", "彩蛋", "幕后", "幕後",
    ]
    .iter()
    .any(|word| title.contains(word) || category.contains(word))
}

pub async fn propose(
    source: &Value,
    metadata: &Value,
    revoked: &HashSet<String>,
) -> Result<Value, &'static str> {
    let kind = metadata["type"]
        .as_str()
        .filter(|v| matches!(*v, "movie" | "series"))
        .ok_or("invalid metadata type")?;
    let names = queries(metadata);
    if names.is_empty() {
        return Err("metadata title missing");
    }
    let expected_year = year(&metadata["year"]);
    let mut seen = HashSet::new();
    let mut search_results = Vec::new();
    for query in &names {
        let items = crate::adapters::maccms::search(source, query).await?;
        for item in items
            .into_iter()
            .filter(|item| {
                item["vod_name"]
                    .as_str()
                    .is_some_and(|title| moc_core::matching::related(query, title))
            })
            .take(10)
        {
            let Some(id) = item["vod_id"].as_str() else {
                continue;
            };
            if seen.insert(id.to_owned()) {
                search_results.push(item);
            }
        }
    }
    let mut candidates = Vec::new();
    for item in search_results {
        let title = item["vod_name"].as_str().unwrap_or("");
        let category = item["type_name"].as_str().unwrap_or("");
        if derived(title, category) {
            continue;
        }
        let actual_kind = source_kind(category);
        if actual_kind.is_some_and(|actual| actual != kind) {
            continue;
        }
        let (base, tail_year) = title_year(title);
        let structured = year(&item["vod_year"]);
        if expected_year.is_some_and(|expected| {
            structured.is_some_and(|v| v != expected) || tail_year.is_some_and(|v| v != expected)
        }) {
            continue;
        }
        let id = item["vod_id"].as_str().expect("validated adapter ID");
        let detail = crate::adapters::maccms::detail(source, id).await?;
        let lines = lines_for(metadata, &detail);
        if lines.is_empty() {
            continue;
        }
        let exact = names
            .iter()
            .any(|name| moc_core::matching::normalize(name) == moc_core::matching::normalize(base));
        let mut mismatch = Vec::new();
        if !exact {
            mismatch.push("title");
        }
        if actual_kind.is_none() {
            mismatch.push("type_unknown");
        }
        if structured.is_none() && tail_year.is_none() {
            mismatch.push("year_missing");
        }
        let automatic = exact && actual_kind == Some(kind) && !revoked.contains(id);
        candidates.push(json!({"content_id":id,"title":title,"year":structured.or(tail_year),"kind":actual_kind,"lines":lines,"mismatch":mismatch,"auto_eligible":automatic}));
    }
    let eligible: Vec<_> = candidates
        .iter()
        .filter(|item| item["auto_eligible"] == true)
        .collect();
    let selected = (eligible.len() == 1).then(|| eligible[0]["content_id"].clone());
    Ok(json!({"queries":names,"candidates":candidates,"selected":selected}))
}

pub fn lines_for(metadata: &Value, detail: &Value) -> Vec<Value> {
    let kind = metadata["type"].as_str().unwrap_or("");
    let requested = metadata["episode"]["number"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .map(|number| {
            (
                metadata["episode"]["season"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok()),
                number,
            )
        });
    let seasons: Vec<u32> = metadata["seasons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
        .collect();
    let mut lines = Vec::new();
    for group in detail["groups"].as_array().into_iter().flatten() {
        let Some(episodes) = group["episodes"].as_array() else {
            continue;
        };
        if let Some(episode) =
            moc_core::matching::corresponding_episode(kind, requested, &seasons, episodes)
        {
            lines.push(json!({"flag":group["flag"],"episode":episode}));
        }
    }
    lines
}

pub fn evidence(metadata: &Value, detail: &Value) -> Value {
    json!({"metadata_title":moc_core::matching::normalize(metadata["title"].as_str().unwrap_or("")),"metadata_type":metadata["type"],"metadata_year":year(&metadata["year"]),
        "source_title":moc_core::matching::normalize(detail["vod_name"].as_str().unwrap_or("")),"source_type":source_kind(detail["type_name"].as_str().unwrap_or("")),"source_year":year(&detail["vod_year"])})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_year_handles_unicode_and_query_fallback() {
        assert_eq!(title_year("庆余年"), ("庆余年", None));
        assert_eq!(title_year("电影（2020）"), ("电影", Some(2020)));
        let meta = json!({"title":"电影","original_title":"电影","english_title":"Film","aliases":["Alias"]});
        assert_eq!(queries(&meta), vec!["电影", "Film"]);
        assert_eq!(source_kind("国产剧"), Some("series"));
        assert_eq!(source_kind("动作片"), Some("movie"));
        assert_eq!(source_kind("动漫"), None);
    }
    #[test]
    #[ignore = "requires controlled MacCMS fixture"]
    fn t17_matching_mapping_with_controlled_source() {
        let base = std::env::var("T17_FIXTURE_HOST").unwrap();
        let source = json!({"id":"source-a","revision":1,"enabled":true,"kind":"maccms_json","api":format!("{base}/maccms-match"),"headers":{"X-Fixture":"allowed"}});
        let meta = json!({"metadata_source_id":"tmdb","meta_id":"tmdb:42","type":"movie","title":"Film","original_title":"Original","english_title":"Film","year":2020});
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let proposal = rt
            .block_on(propose(&source, &meta, &HashSet::new()))
            .unwrap();
        assert_eq!(proposal["queries"], json!(["Film", "Original"]));
        assert_eq!(proposal["selected"], "101");
        let ids: Vec<_> = proposal["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v["content_id"].as_str())
            .collect();
        assert!(ids.contains(&"101") && ids.contains(&"104") && ids.contains(&"105"));
        assert!(!ids.iter().any(|id| ["102", "103", "106"].contains(id)));
        let same = json!({"metadata_source_id":"tmdb","meta_id":"tmdb:same","type":"movie","title":"Same","year":2020});
        assert!(
            rt.block_on(propose(&source, &same, &HashSet::new()))
                .unwrap()["selected"]
                .is_null()
        );
        let unknown = json!({"metadata_source_id":"tmdb","meta_id":"tmdb:unknown","type":"movie","title":"Mystery","year":2020});
        let p = rt
            .block_on(propose(&source, &unknown, &HashSet::new()))
            .unwrap();
        assert!(p["selected"].is_null());
        assert_eq!(
            p["candidates"][0]["mismatch"],
            json!(["type_unknown", "year_missing"])
        );
        let temp = tempfile::tempdir().unwrap();
        crate::db::initialize(temp.path()).unwrap();
        let store = crate::mappings::Store::new(temp.path());
        let detail = rt
            .block_on(crate::adapters::maccms::detail(&source, "101"))
            .unwrap();
        let saved = store
            .save(&meta, &source, "101", &evidence(&meta, &detail), "user")
            .unwrap();
        assert_eq!(
            store
                .save(&meta, &source, "105", &evidence(&meta, &detail), "auto")
                .unwrap()["content_id"],
            "101"
        );
        assert!(store.revoke(saved["id"].as_str().unwrap()).unwrap());
        let revoked = store
            .revoked("tmdb", "movie", "tmdb:42", "source-a")
            .unwrap();
        assert!(revoked.contains("101"));
        assert!(rt.block_on(propose(&source, &meta, &revoked)).unwrap()["selected"].is_null());
        assert!(
            store
                .save(&meta, &source, "101", &evidence(&meta, &detail), "auto")
                .is_err()
        );
        assert_eq!(
            store
                .save(&meta, &source, "101", &evidence(&meta, &detail), "user")
                .unwrap()["revoked"],
            false
        );
        let other = json!({"metadata_source_id":"other","meta_id":"tmdb:42","type":"movie","title":"Film","year":2020});
        store
            .save(&other, &source, "101", &evidence(&other, &detail), "user")
            .unwrap();
        assert_eq!(store.list("other", "movie", "tmdb:42").unwrap().len(), 1);
        assert_eq!(store.list("tmdb", "series", "tmdb:42").unwrap().len(), 0);
        let series = json!({"metadata_source_id":"tmdb","meta_id":"tmdb:show","type":"series","title":"Show","year":2020,"episode":{"season":1,"number":2},"seasons":[1,2]});
        let p = rt
            .block_on(propose(&source, &series, &HashSet::new()))
            .unwrap();
        assert_eq!(p["selected"], "201");
        assert_eq!(p["candidates"].as_array().unwrap().len(), 1);
        let series_detail = rt
            .block_on(crate::adapters::maccms::detail(&source, "201"))
            .unwrap();
        store
            .save(
                &series,
                &source,
                "201",
                &evidence(&series, &series_detail),
                "user",
            )
            .unwrap();
        assert!(
            rt.block_on(store.reusable(&series, &source))
                .unwrap()
                .is_some()
        );
        let mut updated_source = source.clone();
        updated_source["revision"] = json!(2);
        assert!(
            rt.block_on(store.reusable(&series, &updated_source))
                .unwrap()
                .is_some()
        );
        rt.block_on(async {
            reqwest::get(format!("{base}/control?add_episode=1"))
                .await
                .unwrap();
        });
        let mut new_episode = series.clone();
        new_episode["episode"]["number"] = json!(4);
        assert!(
            rt.block_on(store.reusable(&new_episode, &source))
                .unwrap()
                .is_some()
        );
        rt.block_on(async {
            reqwest::get(format!("{base}/control?changed_title=1"))
                .await
                .unwrap();
        });
        assert!(
            rt.block_on(store.reusable(&series, &source))
                .unwrap()
                .is_none()
        );
        println!("T17_MATCHING:filter,ambiguity,revoke,isolation,add-episode,identity-conflict");
    }
}
