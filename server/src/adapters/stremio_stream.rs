use crate::metadata::{cache::Cache, http};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use url::Url;

use super::maccms;

pub enum ResultSet {
    Unsupported(&'static str),
    Ready {
        request_id: String,
        lines: Vec<Value>,
    },
}

pub fn cached_manifest(source: &Value, cache: &Cache) -> Result<Option<Value>, &'static str> {
    let id = source["id"].as_str().ok_or("source ID missing")?;
    let revision = source["revision"]
        .as_u64()
        .ok_or("source revision missing")?;
    cache
        .get(&cache.key(id, "stream_manifest", "", &revision.to_string()))
        .map_err(|_| "manifest cache failed")
}

pub async fn manifest(source: &Value, cache: &Cache) -> Result<Value, &'static str> {
    read_manifest(source, cache, false).await
}

pub async fn refresh_manifest(source: &Value, cache: &Cache) -> Result<Value, &'static str> {
    read_manifest(source, cache, true).await
}

async fn read_manifest(
    source: &Value,
    cache: &Cache,
    refresh: bool,
) -> Result<Value, &'static str> {
    let id = source["id"].as_str().ok_or("source ID missing")?;
    let revision = source["revision"]
        .as_u64()
        .ok_or("source revision missing")?;
    let key = cache.key(id, "stream_manifest", "", &revision.to_string());
    if !refresh && let Some(value) = cached_manifest(source, cache)? {
        return Ok(value);
    }
    let url = Url::parse(source["api"].as_str().ok_or("manifest URL missing")?)
        .map_err(|_| "invalid manifest URL")?;
    let value = http::json(url, maccms::headers(source)?)
        .await
        .map_err(|_| "Stremio manifest request failed")?;
    if value["id"].as_str().is_none_or(str::is_empty)
        || value["name"].as_str().is_none_or(str::is_empty)
        || value["version"].as_str().is_none_or(str::is_empty)
        || !value["resources"].is_array()
        || !value["types"].is_array()
    {
        return Err("invalid Stremio manifest");
    }
    let valid_list = |value: &Value| {
        value.as_array().is_some_and(|items| {
            items.len() <= 128
                && items.iter().all(|item| {
                    item.as_str()
                        .is_some_and(|text| !text.is_empty() && text.len() <= 512)
                })
        })
    };
    for field in ["types", "idPrefixes"] {
        if value.get(field).is_some_and(|v| !valid_list(v)) {
            return Err("invalid Stremio manifest capability");
        }
    }
    let resources = value["resources"]
        .as_array()
        .filter(|items| items.len() <= 128)
        .ok_or("invalid Stremio manifest capability")?
        .iter()
        .map(|resource| {
            if let Some(name) = resource
                .as_str()
                .filter(|name| !name.is_empty() && name.len() <= 128)
            {
                return Ok(json!(name));
            }
            let name = resource["name"]
                .as_str()
                .filter(|name| !name.is_empty() && name.len() <= 128)
                .ok_or("invalid Stremio manifest capability")?;
            let mut clean = json!({"name":name});
            for field in ["types", "idPrefixes"] {
                if let Some(field_value) = resource.get(field) {
                    if !valid_list(field_value) {
                        return Err("invalid Stremio manifest capability");
                    }
                    clean[field] = field_value.clone();
                }
            }
            Ok(clean)
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let saved = json!({"id":value["id"],"name":value["name"],"version":value["version"],"resources":resources,"types":value["types"],"idPrefixes":value["idPrefixes"]});
    cache
        .put(&key, &saved, 6 * 3600)
        .map_err(|_| "manifest cache failed")?;
    Ok(saved)
}

fn definitions<'a>(manifest: &'a Value, kind: &str) -> Vec<&'a Value> {
    manifest["resources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| {
            if entry.as_str() != Some("stream") && entry["name"] != "stream" {
                return false;
            }
            let types = entry
                .get("types")
                .and_then(Value::as_array)
                .or_else(|| manifest["types"].as_array());
            types.is_none_or(|items| items.iter().any(|value| value.as_str() == Some(kind)))
        })
        .collect()
}

fn prefixes<'a>(manifest: &'a Value, entry: &'a Value) -> Option<&'a Vec<Value>> {
    if entry.is_object() {
        entry.get("idPrefixes").and_then(Value::as_array)
    } else {
        manifest["idPrefixes"].as_array()
    }
}

fn supports_id(manifest: &Value, entry: &Value, id: &str) -> bool {
    prefixes(manifest, entry).is_none_or(|items| {
        items.is_empty()
            || items
                .iter()
                .filter_map(Value::as_str)
                .any(|prefix| !prefix.is_empty() && id.starts_with(prefix))
    })
}

pub fn imdb(value: &str) -> bool {
    value.strip_prefix("tt").is_some_and(|digits| {
        !digits.is_empty() && digits.len() <= 20 && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub fn select_id(metadata: &Value, manifest: &Value) -> Result<String, &'static str> {
    let kind = metadata["type"].as_str().ok_or("type_unsupported")?;
    let has_stream = manifest["resources"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|entry| entry.as_str() == Some("stream") || entry["name"] == "stream");
    if !has_stream {
        return Err("stream_unsupported");
    }
    let defs = definitions(manifest, kind);
    if defs.is_empty() {
        return Err("type_unsupported");
    }
    let imdb_id = metadata["imdb_id"].as_str().filter(|id| imdb(id));
    let imdb_video = imdb_id.map(|id| {
        if kind == "series" {
            format!(
                "{id}:{}:{}",
                metadata["episode"]["season"].as_u64().unwrap_or(0),
                metadata["episode"]["number"].as_u64().unwrap_or(0)
            )
        } else {
            id.to_owned()
        }
    });
    let raw = metadata["video_id"].as_str().filter(|id| {
        metadata["metadata_source_id"] != "tmdb"
            && metadata["namespace_prefixes"]
                .as_array()
                .is_some_and(|prefixes| {
                    prefixes
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|prefix| !prefix.is_empty() && id.starts_with(prefix))
                })
    });
    for id in imdb_video.as_deref().into_iter().chain(raw) {
        if defs.iter().any(|entry| supports_id(manifest, entry, id)) {
            return Ok(id.into());
        }
    }
    if imdb_id.is_none()
        && defs.iter().any(|entry| {
            prefixes(manifest, entry).is_some_and(|values| {
                values.iter().any(|value| {
                    value
                        .as_str()
                        .is_some_and(|prefix| prefix == "tt" || prefix.starts_with("tt"))
                })
            })
        })
    {
        Err("imdb_id_missing")
    } else if imdb_id.is_none() && raw.is_none() {
        Err("id_unavailable")
    } else {
        Err("id_prefix_unsupported")
    }
}

fn stable_key(stream: &Value, url: &str) -> Option<String> {
    if let Some(hash) = stream["behaviorHints"]["videoHash"]
        .as_str()
        .filter(|value| value.len() == 16 && value.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Some(format!(
            "{:x}",
            Sha256::digest(format!("hash:{}", hash.to_ascii_lowercase()))
        ));
    }
    let parsed = Url::parse(url).ok().filter(http::valid)?;
    if parsed.query().is_some() || parsed.fragment().is_some() || parsed.path() == "/" {
        return None;
    }
    Some(format!(
        "{:x}",
        Sha256::digest(format!(
            "url:{}{}",
            parsed.origin().ascii_serialization(),
            parsed.path()
        ))
    ))
}

fn playback_headers(stream: &Value) -> Result<Value, &'static str> {
    let value = stream["behaviorHints"]["proxyHeaders"]
        .get("request")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let values = value
        .as_object()
        .ok_or("invalid Stremio playback headers")?;
    if values.len() > 32 {
        return Err("too many Stremio playback headers");
    }
    let mut headers = HeaderMap::new();
    for (name, value) in values {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| "invalid Stremio playback headers")?;
        if !crate::metadata::http::allowed_header(&name) || name == reqwest::header::RANGE {
            return Err("unsafe Stremio playback header");
        }
        let value =
            HeaderValue::from_str(value.as_str().ok_or("invalid Stremio playback headers")?)
                .map_err(|_| "invalid Stremio playback headers")?;
        headers.insert(name, value);
    }
    let mut output = serde_json::Map::new();
    for (name, value) in headers.iter() {
        output.insert(
            name.as_str().into(),
            json!(
                value
                    .to_str()
                    .map_err(|_| "invalid Stremio playback headers")?
            ),
        );
    }
    Ok(Value::Object(output))
}

pub async fn streams(
    source: &Value,
    manifest: &Value,
    kind: &str,
    request_id: &str,
) -> Result<Vec<Value>, &'static str> {
    if !definitions(manifest, kind)
        .iter()
        .any(|entry| supports_id(manifest, entry, request_id))
    {
        return Err("Stremio capability changed");
    }
    let mut url = Url::parse(source["api"].as_str().ok_or("manifest URL missing")?)
        .map_err(|_| "invalid manifest URL")?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| "invalid manifest URL")?;
        path.pop()
            .push("stream")
            .push(kind)
            .push(&format!("{request_id}.json"));
    }
    let result = http::json(url, maccms::headers(source)?)
        .await
        .map_err(|_| "Stremio stream request failed")?;
    let streams = result["streams"]
        .as_array()
        .filter(|items| items.len() <= 1024)
        .ok_or("invalid Stremio streams")?;
    let mut candidates = Vec::new();
    for stream in streams {
        if !stream["infoHash"].is_null() {
            continue;
        }
        let (url, reason) =
            if let Some(url) = stream["url"].as_str().filter(|value| !value.is_empty()) {
                (url, None)
            } else if let Some(url) = stream["externalUrl"]
                .as_str()
                .filter(|value| !value.is_empty())
            {
                (url, Some("external_url_unsupported"))
            } else {
                continue;
            };
        let label = stream["name"]
            .as_str()
            .or_else(|| stream["title"].as_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Stremio");
        let label: String = label.chars().take(512).collect();
        let key = if reason.is_none() {
            stable_key(stream, url)
        } else {
            None
        };
        let hint = stream["type"]
            .as_str()
            .map(str::to_ascii_lowercase)
            .filter(|value| {
                matches!(
                    value.as_str(),
                    "hls" | "m3u8" | "application/vnd.apple.mpegurl"
                )
            });
        candidates.push((key.clone(),json!({"flag":label,"episode":{"label":request_id,"url":url,"headers":playback_headers(stream)?,"resolved":true,"kind_hint":hint.map(|_| "hls"),"unsupported_reason":reason,"stable_key":key,"locator":null}})));
    }
    let mut counts = HashMap::new();
    for (key, _) in &candidates {
        if let Some(key) = key {
            *counts.entry(key.clone()).or_insert(0_usize) += 1;
        }
    }
    Ok(candidates
        .into_iter()
        .map(|(key, mut line)| {
            if let Some(key) = key.filter(|key| counts.get(key) == Some(&1)) {
                line["episode"]["locator"] = json!({"stream_key":key});
            }
            line
        })
        .collect())
}

pub async fn lookup(
    source: &Value,
    metadata: &Value,
    cache: &Cache,
) -> Result<ResultSet, &'static str> {
    let manifest = manifest(source, cache).await?;
    let id = match select_id(metadata, &manifest) {
        Ok(id) => id,
        Err(reason) => return Ok(ResultSet::Unsupported(reason)),
    };
    let lines = streams(
        source,
        &manifest,
        metadata["type"].as_str().ok_or("invalid metadata type")?,
        &id,
    )
    .await?;
    Ok(ResultSet::Ready {
        request_id: id,
        lines,
    })
}
