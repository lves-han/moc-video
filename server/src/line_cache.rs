use crate::state::{Line, Registry, Runtime, Template};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use url::Url;

/// Logical line identity shared by refresh checks and persistent statistics. The
/// source revision stays out of it so renaming or toggling a source keeps its
/// history; revision checks elsewhere still retire old handles and cache entries.
pub fn key(target: &Value, source: &Value, content_id: &str, locator: &Option<Value>) -> String {
    let unique = locator
        .clone()
        .unwrap_or_else(|| json!({"unidentified":uuid::Uuid::new_v4().to_string()}));
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                target["metadata_source_id"],
                target["type"],
                target["meta_id"],
                target["video_id"],
                source["id"],
                content_id,
                unique
            ])
            .to_string()
        )
    )
}

pub fn cache_key(target: &Value, source: &Value, parser_generation: u64) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                target["metadata_source_id"],
                target["type"],
                target["meta_id"],
                target["video_id"],
                source["id"],
                source["revision"],
                parser_generation
            ])
            .to_string()
        )
    )
}

/// A recent actual playback failure takes precedence over an older server probe.
pub fn recent_failure(value: &Value, revision: u64, now: i64) -> bool {
    value["last_outcome"] == "failed" && value["outcome_source_revision"].as_u64() == Some(revision)
        && value["reported_at"].as_i64().is_some_and(|reported| reported <= now && now - reported < 300)
}

pub fn output(line_id: &str, line: &Line, history: &HashMap<String, Value>) -> Value {
    let url = Url::parse(&line.url)
        .ok()
        .filter(crate::metadata::http::valid);
    let unsupported_format = url.as_ref().is_some_and(|value| {
        let path = value.path().to_ascii_lowercase();
        path.ends_with(".mpd") || path.ends_with(".mkv")
    });
    let kind = if line.needs_sniffing || unsupported_format || line.unsupported_reason.is_some() {
        "unsupported"
    } else if url.is_some() && line.kind_hint.as_deref() == Some("hls")
        || url
            .as_ref()
            .is_some_and(|u| u.path().to_ascii_lowercase().ends_with(".m3u8"))
    {
        "hls"
    } else if url.is_some() {
        "file"
    } else if matches!(line.cached_kind.as_deref(), Some("hls" | "file")) {
        line.cached_kind.as_deref().unwrap_or("unsupported")
    } else {
        "unsupported"
    };
    let playable = kind != "unsupported";
    let mut value = json!({"line_id":line_id,"source_id":line.source_id,"source_revision":line.source_revision,
        "line_name":line.flag,"episode_label":line.episode_label,"kind":kind,"quality":line.quality,"size_bytes":line.size_bytes,"description":line.description,"availability":line.availability,"latency_ms":line.latency_ms,"availability_checked_at":line.availability_checked_at,"availability_expires_at":line.availability_expires_at,"target":line.target,
        "expires_at":line.play_expires_at,"playlist":line.playlist,"history":history.get(&line.logical_key).cloned().unwrap_or_else(||json!({"ok":0,"failed":0,"last_ok_at":null}))});
    if line.availability == "failed" && history.get(&line.logical_key).is_some_and(|value| recent_failure(value, line.source_revision, crate::metadata::cache::now())) {
        value["reason"] = json!("recent_playback_failure");
    }
    if !playable {
        value["reason"] = json!(if let Some(reason) = line.unsupported_reason.as_deref() {
            reason
        } else if line.needs_sniffing {
            "needs_sniffing"
        } else if unsupported_format {
            "unsupported_media_format"
        } else {
            "unsupported_protocol_or_script"
        });
    }
    value
}

pub fn issue(
    registry: &mut Registry,
    runtime: &Runtime,
    target: &Value,
    template: &Template,
    history: &HashMap<String, Value>,
) -> Result<Vec<Value>, &'static str> {
    let now = crate::metadata::cache::now();
    registry.prune(now);
    if registry.lines.len() + template.lines.len() > 2048 {
        return Err("line limit reached");
    }
    let estimate = template.source.to_string().len()
        + template.content_id.len()
        + template
            .lines
            .iter()
            .map(|line| line.to_string().len() + 1024)
            .sum::<usize>();
    if registry.bytes().saturating_add(estimate.saturating_mul(3)) > crate::state::MAX_BYTES {
        return Err("line memory limit reached");
    }
    let source_id = template.source["id"]
        .as_str()
        .ok_or("invalid source")?
        .to_owned();
    let revision = template.source["revision"]
        .as_u64()
        .ok_or("invalid source")?;
    let mut output_lines = Vec::new();
    for item in &template.lines {
        let Some(url) = item["episode"]["url"].as_str() else {
            continue;
        };
        let locator = item["episode"]
            .get("locator")
            .filter(|v| !v.is_null())
            .cloned();
        let logical_key = key(target, &template.source, &template.content_id, &locator);
        let recently_failed = history.get(&logical_key).is_some_and(|value| recent_failure(value, revision, now));
        let line = Line {
            source_id: source_id.clone(),
            source_revision: revision,
            parser_generation: registry.parser_generation,
            content_id: template.content_id.clone(),
            target: target.clone(),
            logical_key,
            locator,
            flag: item["flag"].as_str().unwrap_or("").to_owned(),
            episode_label: item["episode"]["label"].as_str().unwrap_or("").to_owned(),
            url: url.to_owned(),
            headers: item["episode"]
                .get("headers")
                .cloned()
                .unwrap_or_else(|| template.source["headers"].clone()),
            quality: item["episode"]["quality"].as_str().map(str::to_owned),
            description: item["episode"]["description"].as_str().map(str::to_owned),
            playlist: item["episode"].get("playlist").filter(|value| value.is_object()).cloned(),
            availability_key: item["episode"]["availability_key"]
                .as_str()
                .map(str::to_owned),
            availability: if recently_failed { "failed" } else { item["episode"]["availability"].as_str().unwrap_or("unknown") }.to_owned(),
            latency_ms: item["episode"]["latency_ms"].as_u64(),
            availability_checked_at: item["episode"]["availability_checked_at"].as_i64(),
            availability_expires_at: item["episode"]["availability_expires_at"].as_i64(),
            size_bytes: item["episode"]["size_bytes"]
                .as_u64()
                .filter(|size| *size > 0 && *size <= 9_007_199_254_740_991),
            needs_sniffing: item["episode"]["requires_parse"] == true,
            kind_hint: item["episode"]["kind_hint"].as_str().map(str::to_owned),
            cached_kind: item["episode"]["cached_kind"].as_str().map(str::to_owned),
            unsupported_reason: item["episode"]["unsupported_reason"]
                .as_str()
                .map(str::to_owned),
            expires_at: now + runtime.handle_ttl(),
            play_expires_at: now + runtime.cache_ttl(),
        };
        let id = uuid::Uuid::new_v4().to_string();
        output_lines.push(output(&id, &line, history));
        registry.lines.insert(id, line);
    }
    Ok(output_lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn actual_failure_temporarily_overrides_ready_probe_for_same_revision() {
        let dir = tempfile::tempdir().unwrap(); crate::db::initialize(dir.path()).unwrap();
        let reports = crate::reports::Store::new(dir.path()); let rt = tokio::runtime::Runtime::new().unwrap();
        let runtime = Runtime { registry: Arc::new(Mutex::new(Registry::default())), lookup: Arc::new(std::sync::RwLock::new(crate::config::Lookup::default())) };
        let target = json!({"metadata_source_id":"tmdb","type":"movie","meta_id":"tmdb:1","video_id":"tmdb:1"});
        let template = |revision| Template { source: json!({"id":"s","revision":revision,"headers":{}}), content_id:"c".into(), lines: vec![json!({"flag":"hls","episode":{"label":"full","url":"https://media.test/a.m3u8","availability":"ready","locator":{"flag":"hls","episode":"full"}}})] };
        let mut registry = Registry::default();
        let first = issue(&mut registry, &runtime, &target, &template(2), &HashMap::new()).unwrap();
        let line = registry.lines[ first[0]["line_id"].as_str().unwrap() ].clone();
        reports.record(&line, "failed", None).unwrap();
        let history = rt.block_on(reports.get_many(vec![line.logical_key.clone()])).unwrap();
        let failed = issue(&mut registry, &runtime, &target, &template(2), &history).unwrap();
        assert_eq!(failed[0]["availability"], "failed"); assert_eq!(failed[0]["reason"], "recent_playback_failure");
        let changed = issue(&mut registry, &runtime, &target, &template(3), &history).unwrap();
        assert_eq!(changed[0]["availability"], "ready");
        assert!(!recent_failure(&json!({"last_outcome":"failed","reported_at":100,"outcome_source_revision":2}), 2, 400));
        reports.record(&line, "playing", None).unwrap();
        let history = rt.block_on(reports.get_many(vec![line.logical_key.clone()])).unwrap();
        let recovered = issue(&mut registry, &runtime, &target, &template(2), &history).unwrap();
        assert_eq!(recovered[0]["availability"], "ready");
    }

    #[test]
    fn statistics_follow_the_line_across_source_revisions() {
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let reports = crate::reports::Store::new(dir.path());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let runtime = Runtime {
            registry: Arc::new(Mutex::new(Registry::default())),
            lookup: Arc::new(std::sync::RwLock::new(crate::config::Lookup::default())),
        };
        let target =
            json!({"metadata_source_id":"tmdb","type":"movie","meta_id":"tmdb:1","video_id":"tt1"});
        let template = |revision: u64| Template {
            source: json!({"id":"source","revision":revision,"headers":{}}),
            content_id: "content".into(),
            lines: vec![
                json!({"flag":"hls","episode":{"label":"正片","url":"https://media.test/a.m3u8","locator":{"flag":"hls","episode":"正片"}}}),
                json!({"flag":"dup","episode":{"label":"正片","url":"https://media.test/b.m3u8","locator":null}}),
            ],
        };
        let mut registry = Registry::default();
        // A rename or enable toggle bumps the revision between these rounds.
        for revision in [1, 2] {
            let issued = issue(
                &mut registry,
                &runtime,
                &target,
                &template(revision),
                &HashMap::new(),
            )
            .unwrap();
            for value in issued {
                let line = &registry.lines[value["line_id"].as_str().unwrap()];
                reports.record(line, "playing", Some(10)).unwrap();
            }
        }
        let history = rt
            .block_on(
                reports.get_many(
                    registry
                        .lines
                        .values()
                        .map(|l| l.logical_key.clone())
                        .collect(),
                ),
            )
            .unwrap();
        let issued = issue(&mut registry, &runtime, &target, &template(3), &history).unwrap();
        assert_eq!(issued[0]["history"]["ok"], 2);
        assert_eq!(issued[1]["history"]["ok"], 0);
        let rows: i64 = crate::db::sync_with(&dir.path().join("service.sqlite3"), |conn| {
            conn.query_row("SELECT COUNT(*) FROM line_stats", [], |row| row.get(0))
        })
        .unwrap();
        assert_eq!(rows, 1, "lines without a locator leave no statistics rows");
        let outcomes = rt.block_on(reports.source_outcomes(&target)).unwrap();
        assert_eq!(
            outcomes.get(&("source".to_owned(), 2)).unwrap().0,
            "playing"
        );
        let line = registry
            .lines
            .values()
            .find(|line| line.source_revision == 2)
            .unwrap();
        reports.record(line, "failed", None).unwrap();
        let outcomes = rt.block_on(reports.source_outcomes(&target)).unwrap();
        assert_eq!(outcomes.get(&("source".to_owned(), 2)).unwrap().0, "failed");
        assert_eq!(
            outcomes.get(&("source".to_owned(), 1)).unwrap().0,
            "playing"
        );
        let other =
            json!({"metadata_source_id":"tmdb","type":"movie","meta_id":"tmdb:2","video_id":"tt2"});
        assert!(
            rt.block_on(reports.source_outcomes(&other))
                .unwrap()
                .is_empty()
        );
        drop(reports);
        let reopened = crate::reports::Store::new(dir.path());
        assert_eq!(
            rt.block_on(reopened.source_outcomes(&target))
                .unwrap()
                .get(&("source".to_owned(), 2))
                .unwrap()
                .0,
            "failed"
        );
    }
}
