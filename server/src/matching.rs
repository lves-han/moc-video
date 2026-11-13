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

/// A trailing four-digit number is a year unless the metadata title itself ends
/// with it, as in 《银翼杀手2049》 or 《1917》.
fn title_year_against<'a>(title: &'a str, names: &[String]) -> (&'a str, Option<u32>) {
    let (base, year) = title_year(title);
    let Some(value) = year else {
        return (base, None);
    };
    let digits = value.to_string();
    let named = names.iter().any(|name| {
        let name = moc_core::matching::normalize(name);
        name.strip_suffix(digits.as_str())
            .is_some_and(|rest| !rest.ends_with(|c: char| c.is_ascii_digit()))
    });
    if named { (title, None) } else { (base, year) }
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

fn season_number(text: &str) -> Option<u32> {
    if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
        return text.parse().ok().filter(|n| (1..=99).contains(n));
    }
    let digit = |text: &str| -> Option<u32> {
        let mut chars = text.chars();
        let value = match chars.next()? {
            '一' => 1,
            '二' | '两' => 2,
            '三' => 3,
            '四' => 4,
            '五' => 5,
            '六' => 6,
            '七' => 7,
            '八' => 8,
            '九' => 9,
            _ => return None,
        };
        chars.next().is_none().then_some(value)
    };
    match text.split_once('十') {
        Some((tens, ones)) => {
            let tens = if tens.is_empty() { 1 } else { digit(tens)? };
            let ones = if ones.is_empty() { 0 } else { digit(ones)? };
            Some(tens * 10 + ones)
        }
        None => digit(text),
    }
}

/// Season markers in a normalized source title: 「第X季」, "season X" or a standalone "sX".
fn season_markers(title: &str) -> Vec<(std::ops::Range<usize>, u32)> {
    let mut found = Vec::new();
    for (start, _) in title.match_indices('第') {
        let after = start + '第'.len_utf8();
        if let Some(length) = title[after..].find('季')
            && let Some(season) = season_number(&title[after..after + length])
        {
            found.push((start..after + length + '季'.len_utf8(), season));
        }
    }
    let bytes = title.as_bytes();
    for at in 0..bytes.len() {
        if at > 0 && bytes[at - 1].is_ascii_alphanumeric() {
            continue;
        }
        let number = if bytes[at..].starts_with(b"season") {
            at + 6 + usize::from(bytes.get(at + 6) == Some(&b' '))
        } else if bytes[at] == b's' {
            at + 1
        } else {
            continue;
        };
        let length = bytes
            .get(number..)
            .unwrap_or_default()
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        let stop = number + length;
        if !(1..=2).contains(&length) || bytes.get(stop).is_some_and(u8::is_ascii_alphanumeric) {
            continue;
        }
        if let Some(season) = season_number(&title[number..stop]) {
            found.push((at..stop, season));
        }
    }
    found
}

/// Season named by a normalized source title, when all of its markers agree.
pub fn title_season(title: &str) -> Option<u32> {
    let markers = season_markers(title);
    let first = markers.first()?.1;
    markers
        .iter()
        .all(|(_, season)| *season == first)
        .then_some(first)
}

fn without_season(title: &str) -> String {
    let mut markers = season_markers(title);
    markers.sort_by_key(|(range, _)| range.start);
    let (mut output, mut last) = (String::new(), 0);
    for (range, _) in markers {
        if range.start >= last {
            output.push_str(&title[last..range.start]);
            output.push(' ');
            last = range.end;
        }
    }
    output.push_str(&title[last..]);
    moc_core::matching::normalize(&output)
}

/// A per-season source entry such as 「庆余年第二季」 names the same show as the
/// metadata title, so series titles also compare with their season marker removed.
fn exact_title(names: &[String], base: &str, kind: &str) -> bool {
    let title = moc_core::matching::normalize(base);
    let unmarked = (kind == "series").then(|| without_season(&title));
    names.iter().any(|name| {
        let name = moc_core::matching::normalize(name);
        name == title || unmarked.as_deref() == Some(name.as_str())
    })
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
        let items = crate::adapters::search(source, query).await?;
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
    let mut missing_episode = false;
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
        let (base, tail_year) = title_year_against(title, &names);
        let structured = year(&item["vod_year"]);
        if expected_year.is_some_and(|expected| {
            structured.is_some_and(|v| v != expected) || tail_year.is_some_and(|v| v != expected)
        }) {
            continue;
        }
        let id = item["vod_id"].as_str().expect("validated adapter ID");
        let detail = crate::adapters::detail(source, id).await?;
        if detail.to_string().len() > 256 * 1024 {
            return Err("candidate exceeds memory limit");
        }
        let (lines, season_unverified) = lines_with_basis(metadata, &detail);
        if lines.is_empty() {
            missing_episode = true;
            continue;
        }
        let exact = exact_title(&names, base, kind);
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
        if season_unverified {
            mismatch.push("season_unverified");
        }
        let automatic =
            exact && actual_kind == Some(kind) && !season_unverified && !revoked.contains(id);
        candidates.push(json!({"content_id":id,"title":title,"year":structured.or(tail_year),"kind":actual_kind,"lines":lines,"detail":detail,"mismatch":mismatch,"auto_eligible":automatic}));
    }
    let eligible: Vec<_> = candidates
        .iter()
        .filter(|item| item["auto_eligible"] == true)
        .collect();
    let selected = (eligible.len() == 1).then(|| eligible[0]["content_id"].clone());
    Ok(
        json!({"queries":names,"candidates":candidates,"selected":selected,"missing_episode":missing_episode}),
    )
}

/// Season for episode labels that carry none. TMDB lists specials as season 0,
/// which a plain 「第N集」 label never means, so only regular seasons count. In a
/// multi-season show the source title names the season; an unmarked title is
/// taken as the first season and reported as unverified.
fn label_season(metadata: &Value, detail: &Value) -> (Option<u32>, bool) {
    let mut regular: Vec<u32> = metadata["seasons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
        .filter(|season| *season >= 1)
        .collect();
    regular.sort_unstable();
    regular.dedup();
    let title = moc_core::matching::normalize(detail["vod_name"].as_str().unwrap_or(""));
    match (title_season(&title), regular.as_slice()) {
        (Some(season), _) => (Some(season), false),
        (None, [only]) => (Some(*only), false),
        (None, [_, _, ..]) => (Some(1), true),
        (None, []) => (None, false),
    }
}

pub fn lines_for(metadata: &Value, detail: &Value) -> Vec<Value> {
    lines_with_basis(metadata, detail).0
}

/// Target lines, and whether a plain label was placed by the unverified first-season rule.
pub fn lines_with_basis(metadata: &Value, detail: &Value) -> (Vec<Value>, bool) {
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
    let (season, assumed) = label_season(metadata, detail);
    let seasons: Vec<u32> = season.into_iter().collect();
    let mut lines = Vec::new();
    let mut used_assumption = false;
    for group in detail["groups"].as_array().into_iter().flatten() {
        let Some(episodes) = group["episodes"].as_array() else {
            continue;
        };
        if let Some(episode) =
            moc_core::matching::corresponding_episode(kind, requested, &seasons, episodes)
        {
            used_assumption |= assumed
                && episode["label"]
                    .as_str()
                    .and_then(moc_core::matching::episode_coordinates)
                    .is_some_and(|(season, _)| season.is_none());
            lines.push(json!({"flag":group["flag"],"episode":episode}));
        }
    }
    (lines, used_assumption)
}

pub fn evidence(metadata: &Value, detail: &Value) -> Value {
    json!({"metadata_title":moc_core::matching::normalize(metadata["title"].as_str().unwrap_or("")),"metadata_type":metadata["type"],"metadata_year":year(&metadata["year"]),
        "source_title":moc_core::matching::normalize(detail["vod_name"].as_str().unwrap_or("")),"source_type":source_kind(detail["type_name"].as_str().unwrap_or("")),"source_year":year(&detail["vod_year"])})
}

/// Fields that must still agree before a stored mapping is reused. The metadata
/// title follows the App language, so it stays in the evidence for display only;
/// the mapping key already fixes the metadata identity.
pub fn same_identity(stored: &Value, current: &Value) -> bool {
    [
        "metadata_type",
        "metadata_year",
        "source_title",
        "source_type",
        "source_year",
    ]
    .iter()
    .all(|key| stored[*key] == current[*key])
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

    fn detail(title: &str, labels: &[&str]) -> Value {
        let episodes: Vec<_> = labels
            .iter()
            .map(|label| json!({"label":label,"url":format!("https://media.test/{label}")}))
            .collect();
        json!({"vod_name":title,"groups":[{"flag":"hls","episodes":episodes}]})
    }

    fn series(season: u64, number: u64, seasons: &[u64], title: &str) -> Value {
        json!({"type":"series","title":title,"episode":{"season":season,"number":number},"seasons":seasons})
    }

    #[test]
    fn trailing_numbers_in_metadata_titles_stay_in_the_title() {
        let names = |values: &[&str]| values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>();
        let blade = names(&["银翼杀手2049", "Blade Runner 2049"]);
        assert_eq!(
            title_year_against("银翼杀手2049", &blade),
            ("银翼杀手2049", None)
        );
        assert_eq!(
            title_year_against("Blade Runner 2049", &blade),
            ("Blade Runner 2049", None)
        );
        assert!(exact_title(
            &blade,
            title_year_against("银翼杀手 2049", &blade).0,
            "movie"
        ));
        let film = names(&["1917"]);
        assert_eq!(title_year_against("1917", &film), ("1917", None));
        assert_eq!(
            title_year_against("1917 (2019)", &film),
            ("1917", Some(2019))
        );
        // A real tail year still applies, and must agree with the metadata year.
        let wandering = names(&["流浪地球"]);
        assert_eq!(
            title_year_against("流浪地球 2019", &wandering),
            ("流浪地球", Some(2019))
        );
        // Only the whole trailing number of the metadata title counts.
        assert_eq!(
            title_year_against("Film 2049", &names(&["Film 12049"])),
            ("Film", Some(2049))
        );
    }

    #[test]
    fn title_season_markers() {
        let season = |title: &str| title_season(&moc_core::matching::normalize(title));
        assert_eq!(season("庆余年第二季"), Some(2));
        assert_eq!(season("庆余年 第2季 国语"), Some(2));
        assert_eq!(season("某剧第十二季"), Some(12));
        assert_eq!(season("Fargo Season 3"), Some(3));
        assert_eq!(season("Fargo S04"), Some(4));
        assert_eq!(season("庆余年S2"), Some(2));
        assert_eq!(season("庆余年"), None);
        assert_eq!(season("Season of the Witch"), None);
        assert_eq!(season("Fargo S02E03"), None);
        assert_eq!(season("某剧第二季 Season 3"), None);
        let names = vec!["庆余年".to_owned()];
        assert!(exact_title(&names, "庆余年第二季", "series"));
        assert!(!exact_title(&names, "庆余年第二季", "movie"));
        assert!(!exact_title(&names, "庆余年之帝王业", "series"));
    }

    #[test]
    fn plain_labels_ignore_specials_and_follow_title_seasons() {
        let plain = ["第1集", "第2集", "第3集"];
        let found = |meta: &Value, detail: &Value| {
            let (lines, assumed) = lines_with_basis(meta, detail);
            (lines.len(), assumed)
        };
        // Specials (season 0) leave a single regular season.
        assert_eq!(
            found(&series(1, 3, &[0, 1, 1], "剧"), &detail("剧", &plain)),
            (1, false)
        );
        // Multi-season show: the title names the season of plain labels.
        let show = [1, 1, 1, 2, 2, 2];
        assert_eq!(
            found(
                &series(2, 3, &show, "庆余年"),
                &detail("庆余年第二季", &plain)
            ),
            (1, false)
        );
        assert_eq!(
            found(
                &series(1, 3, &show, "庆余年"),
                &detail("庆余年第二季", &plain)
            ),
            (0, false)
        );
        // An unmarked title is offered as the first season only with confirmation.
        assert_eq!(
            found(&series(1, 3, &show, "庆余年"), &detail("庆余年", &plain)),
            (1, true)
        );
        assert_eq!(
            found(&series(2, 3, &show, "庆余年"), &detail("庆余年", &plain)),
            (0, false)
        );
        // Explicit labels keep their own season and need no assumption.
        assert_eq!(
            found(
                &series(2, 3, &show, "庆余年"),
                &detail("庆余年", &["S01E03", "S02E03"])
            ),
            (1, false)
        );
        // A plain label never stands for a special.
        assert_eq!(
            found(&series(0, 1, &[0, 1], "剧"), &detail("剧", &["第1集"])),
            (0, false)
        );
    }

    #[test]
    fn mapping_identity_ignores_metadata_language() {
        let source = json!({"vod_name":"盗梦空间","type_name":"科幻片","vod_year":"2010"});
        let zh = json!({"type":"movie","title":"盗梦空间","year":"2010"});
        let en = json!({"type":"movie","title":"Inception","year":"2010"});
        assert!(same_identity(
            &evidence(&zh, &source),
            &evidence(&en, &source)
        ));
        let renamed = json!({"vod_name":"盗梦空间 解说版","type_name":"科幻片","vod_year":"2010"});
        assert!(!same_identity(
            &evidence(&zh, &source),
            &evidence(&zh, &renamed)
        ));
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
            .block_on(crate::adapters::detail(&source, "101"))
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
            .block_on(crate::adapters::detail(&source, "201"))
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
