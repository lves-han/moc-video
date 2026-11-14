use crate::{AppState, state::Template};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};
use url::Url;

const TTL: i64 = 86_400;
const FAILED_TTL: i64 = 300;

async fn template_key(state: &AppState, target: &Value, source: &Value) -> Result<String, &'static str> {
    let parsers = state.parsers.list().await.map_err(|_| "parser database failed")?;
    let versions: Vec<_> = parsers
        .iter()
        .map(|p| json!([p["id"], p["revision"], p["enabled"]]))
        .collect();
    Ok(format!(
        "line-info:v1:{:x}",
        Sha256::digest(
            json!([target, source["id"], source["revision"], versions])
                .to_string()
                .as_bytes()
        )
    ))
}

pub async fn load(
    state: &AppState,
    target: &Value,
    source: &Value,
) -> Result<Option<Template>, &'static str> {
    let key = template_key(state, target, source).await?;
    let Some(value) = state.cache.get_async(&key).await.map_err(|_| "line cache failed")? else {
        return Ok(None);
    };
    let Some(content_id) = value["content_id"].as_str() else {
        return Err("invalid line cache");
    };
    let lines = value["lines"]
        .as_array()
        .cloned()
        .ok_or("invalid line cache")?;
    Ok(Some(Template {
        source: source.clone(),
        content_id: content_id.to_owned(),
        lines,
    }))
}

pub async fn store(state: &AppState, target: &Value, template: &Template) -> Result<(), &'static str> {
    let mut lines = Vec::new();
    let now = crate::metadata::cache::now();
    let mut expires_at = now + TTL;
    for item in &template.lines {
        let episode = &item["episode"];
        if episode["availability"] == "unknown" {
            continue;
        }
        if let Some(expiry) = episode["availability_expires_at"].as_i64() {
            expires_at = expires_at.min(expiry);
        }
        let url = episode["url"].as_str().unwrap_or("");
        let parsed = Url::parse(url).ok().filter(crate::metadata::http::valid);
        let unsupported = episode["requires_parse"] == true
            || episode["unsupported_reason"].as_str().is_some()
            || parsed.as_ref().is_none_or(|u| {
                let p = u.path().to_ascii_lowercase();
                p.ends_with(".mkv") || p.ends_with(".mpd")
            });
        if !unsupported && episode["availability"] == "ready" && episode["locator"].is_null() {
            return Ok(());
        }
        let kind = if unsupported {
            "unsupported"
        } else if episode["kind_hint"] == "hls"
            || parsed
                .as_ref()
                .is_some_and(|u| u.path().to_ascii_lowercase().ends_with(".m3u8"))
        {
            "hls"
        } else {
            "file"
        };
        lines.push(json!({"flag":item["flag"], "episode":{"label":episode["label"], "locator":episode["locator"], "url":"", "resolved":true,
            "cached_kind":kind, "requires_parse":episode["requires_parse"], "unsupported_reason":episode["unsupported_reason"],
            "quality":episode["quality"], "description":episode["description"], "size_bytes":episode["size_bytes"],
            "availability":episode["availability"], "availability_key":episode["availability_key"], "latency_ms":episode["latency_ms"],
            "availability_checked_at":episode["availability_checked_at"], "availability_expires_at":episode["availability_expires_at"]}}));
    }
    if lines.is_empty() || expires_at <= now {
        return Ok(());
    }
    let key = template_key(state, target, &template.source).await?;
    state
        .cache
        .put_async(
            &key,
            &json!({"content_id":template.content_id,"lines":lines}),
            expires_at - now,
        )
        .await
        .map_err(|_| "line cache failed")
}

pub async fn invalidate(state: &AppState, target: &Value, source: &Value) -> Result<(), &'static str> {
    let key = template_key(state, target, source).await?;
    state
        .cache
        .remove_async(&key)
        .await
        .map_err(|_| "line cache failed")
}

fn probe_key(
    target: &Value,
    source: &Value,
    content_id: &str,
    locator: &Value,
    versions: &[Value],
) -> String {
    format!(
        "availability:v1:{:x}",
        Sha256::digest(
            json!([
                target,
                source["id"],
                source["revision"],
                content_id,
                locator,
                versions
            ])
            .to_string()
            .as_bytes()
        )
    )
}

pub async fn invalidate_line(state: &AppState, line: &crate::state::Line) -> Result<(), &'static str> {
    let parsers = state.parsers.list().await.map_err(|_| "parser database failed")?;
    let versions: Vec<_> = parsers
        .iter()
        .map(|p| json!([p["id"], p["revision"], p["enabled"]]))
        .collect();
    let source = json!({"id":line.source_id,"revision":line.source_revision});
    if let Some(key) = line
        .availability_key
        .as_deref()
        .filter(|key| key.starts_with("availability:v1:"))
    {
        return state
            .cache
            .remove_async(key)
            .await
            .map_err(|_| "availability cache failed");
    }
    let locator = line.locator.clone().unwrap_or_else(|| json!({"url":line.url,"flag":line.flag,"label":line.episode_label,"quality":line.quality}));
    let key = probe_key(
        &line.target,
        &source,
        &line.content_id,
        &locator,
        &versions,
    );
    state
        .cache
        .remove_async(&key)
        .await
        .map_err(|_| "availability cache failed")
}

pub async fn inspect(
    state: &AppState,
    target: &Value,
    template: &Template,
    force: bool,
    progress: Option<tokio::sync::mpsc::Sender<Value>>,
) -> Result<Vec<Value>, &'static str> {
    let parsers = state.parsers.list().await.map_err(|_| "parser database failed")?;
    let versions: Vec<_> = parsers
        .iter()
        .map(|p| json!([p["id"], p["revision"], p["enabled"]]))
        .collect();
    let mut output = template.lines.clone();
    let mut emitted = vec![false; output.len()];
    let mut jobs = tokio::task::JoinSet::new();
    static SLOTS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let slots = SLOTS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(8)))
        .clone();
    for (index, item) in template.lines.iter().enumerate() {
        let episode = &item["episode"];
        let Some(url) = episode["url"].as_str() else {
            continue;
        };
        let parsed = Url::parse(url).ok().filter(crate::metadata::http::valid);
        let unsupported = episode["requires_parse"] == true
            || episode["unsupported_reason"].as_str().is_some()
            || parsed.as_ref().is_none_or(|url| {
                let path = url.path().to_ascii_lowercase();
                path.ends_with(".mpd") || path.ends_with(".mkv")
            });
        if unsupported {
            output[index]["episode"]["availability"] = json!("unsupported");
            if let Some(tx) = &progress { tx.send(output[index].clone()).await.map_err(|_| "lookup cancelled")?; emitted[index] = true; }
            continue;
        }
        let locator = episode.get("locator").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| json!({"url":url,"flag":item["flag"],"label":episode["label"],"quality":episode["quality"]}));
        let key = probe_key(
            target,
            &template.source,
            &template.content_id,
            &locator,
            &versions,
        );
        output[index]["episode"]["availability_key"] = json!(key);
        if !force {
            if let Some(cached) = state
                .cache
                .get_async(&key)
                .await
                .map_err(|_| "availability cache failed")?
            {
                if cached["expires_at"]
                    .as_i64()
                    .is_some_and(|expires| expires > crate::metadata::cache::now())
                {
                    apply(&mut output[index], &cached);
                    if let Some(tx) = &progress { tx.send(output[index].clone()).await.map_err(|_| "lookup cancelled")?; emitted[index] = true; }
                    continue;
                }
            }
        }
        let source = template.source.clone();
        let line = item.clone();
        let cache = state.cache.clone();
        let slots = slots.clone();
        jobs.spawn(async move {
            let permit = slots.acquire_owned().await.map_err(|_| "availability slots closed")?;
            let headers = line["episode"].get("headers").filter(|v| v.is_object()).cloned().unwrap_or_else(|| source["headers"].clone());
            let header_map = crate::adapters::maccms::headers(&json!({"headers":headers})).map_err(|_| "invalid playback headers")?;
            let url = Url::parse(line["episode"]["url"].as_str().ok_or("playback URL missing")?).map_err(|_| "invalid playback URL")?;
            let hls = line["episode"]["kind_hint"] == "hls" || url.path().to_ascii_lowercase().ends_with(".m3u8");
            let start = std::time::Instant::now();
            let result = crate::metadata::http::probe(url, header_map, hls).await;
            drop(permit);
            let now = crate::metadata::cache::now();
            let item_ttl = if result.is_ok() { TTL } else { FAILED_TTL };
            let value = json!({"status":if result.is_ok() {"ready"} else {"failed"},"latency_ms":if result.is_ok() {Some(start.elapsed().as_millis() as u64)} else {None},"size_bytes":line["episode"]["size_bytes"].as_u64().or_else(|| result.ok().flatten()),"quality":line["episode"]["quality"],"description":line["episode"]["description"],"checked_at":now,"expires_at":now+item_ttl});
            cache.put_async(&key, &value, item_ttl).await.map_err(|_| "availability cache failed")?;
            Ok::<_, &'static str>((index,value))
        });
    }
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(8));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => { jobs.abort_all(); break; }
            next = jobs.join_next() => match next {
                Some(result) => { let (index, value) = result.map_err(|_| "availability worker failed")??; apply(&mut output[index], &value); if let Some(tx) = &progress { tx.send(output[index].clone()).await.map_err(|_| "lookup cancelled")?; emitted[index] = true; } }
                None => break,
            }
        }
    }
    for (index, item) in output.iter_mut().enumerate() {
        if item["episode"]["availability"].is_null() {
            item["episode"]["availability"] = json!("unknown");
        }
        if !emitted[index] {
            if let Some(tx) = &progress { tx.send(item.clone()).await.map_err(|_| "lookup cancelled")?; }
        }
    }
    Ok(output)
}
fn apply(item: &mut Value, cached: &Value) {
    item["episode"]["availability"] = cached["status"].clone();
    item["episode"]["latency_ms"] = cached["latency_ms"].clone();
    item["episode"]["availability_checked_at"] = cached["checked_at"].clone();
    item["episode"]["availability_expires_at"] = cached["expires_at"].clone();
    for field in ["size_bytes", "quality", "description"] {
        if item["episode"][field].is_null() {
            item["episode"][field] = cached[field].clone();
        }
    }
}
