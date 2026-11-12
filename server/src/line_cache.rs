use crate::{
    reports,
    state::{Line, Registry, Runtime, Template},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

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
                source["revision"],
                content_id,
                unique
            ])
            .to_string()
        )
    )
}

pub fn cache_key(target: &Value, source: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                target["metadata_source_id"],
                target["type"],
                target["meta_id"],
                target["video_id"],
                source["id"],
                source["revision"]
            ])
            .to_string()
        )
    )
}

pub fn output(line_id: &str, line: &Line, reports: &reports::Store) -> Value {
    let url = Url::parse(&line.url)
        .ok()
        .filter(crate::metadata::http::valid);
    let kind = if url
        .as_ref()
        .is_some_and(|u| u.path().to_ascii_lowercase().ends_with(".m3u8"))
    {
        "hls"
    } else if url.is_some() {
        "file"
    } else {
        "unsupported"
    };
    let play = url
        .as_ref()
        .map(|_| json!({"url":line.url,"headers":line.headers}));
    let mut value = json!({"line_id":line_id,"source_id":line.source_id,"source_revision":line.source_revision,
        "line_name":line.flag,"episode_label":line.episode_label,"kind":kind,"quality":null,"target":line.target,
        "expires_at":line.play_expires_at,"history":reports.get(&line.logical_key).unwrap_or_else(|_|json!({"ok":0,"failed":0,"last_ok_at":null}))});
    if let Some(play) = play {
        value["play"] = play;
    } else {
        value["reason"] = json!("unsupported_protocol_or_script");
    }
    value
}

pub fn issue(
    registry: &mut Registry,
    runtime: &Runtime,
    target: &Value,
    template: &Template,
    reports: &reports::Store,
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
        let line = Line {
            source_id: source_id.clone(),
            source_revision: revision,
            content_id: template.content_id.clone(),
            target: target.clone(),
            logical_key: key(target, &template.source, &template.content_id, &locator),
            locator,
            flag: item["flag"].as_str().unwrap_or("").to_owned(),
            episode_label: item["episode"]["label"].as_str().unwrap_or("").to_owned(),
            url: url.to_owned(),
            headers: template.source["headers"].clone(),
            expires_at: now + runtime.handle_ttl,
            play_expires_at: now + runtime.cache_ttl,
        };
        let id = uuid::Uuid::new_v4().to_string();
        output_lines.push(output(&id, &line, reports));
        registry.lines.insert(id, line);
    }
    Ok(output_lines)
}
