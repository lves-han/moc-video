use super::{cache::Cache, http};
use serde_json::{Value, json};
use url::Url;

pub fn localized_manifest(original: &Url, lang: &str) -> Option<Url> {
    if !matches!(
        original.host_str(),
        Some("tmdb.elfhosted.com" | "94c8cb9f702d-tmdb-addon.baby-beamup.club")
    ) {
        return None;
    }
    let mut segments: Vec<String> = original.path_segments()?.map(str::to_owned).collect();
    if segments.last()?.as_str() != "manifest.json" {
        return None;
    }
    if segments.len() == 1 {
        segments.insert(0, lang.to_owned());
    } else if matches!(segments[segments.len() - 2].as_str(), "en-US" | "zh-CN") {
        let index = segments.len() - 2;
        segments[index] = lang.to_owned();
    } else {
        return None;
    }
    let mut result = original.clone();
    result.set_path(&format!("/{}", segments.join("/")));
    Some(result)
}

fn supported(manifest: &Value, resource: &str, kind: &str, id: &str) -> bool {
    let resource_definition = manifest["resources"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v.as_str() == Some(resource) || v["name"].as_str() == Some(resource));
    let Some(definition) = resource_definition else {
        return false;
    };
    let types = definition
        .get("types")
        .and_then(Value::as_array)
        .or_else(|| manifest["types"].as_array());
    if types.is_some_and(|items| !items.iter().any(|v| v.as_str() == Some(kind))) {
        return false;
    }
    let prefixes = definition
        .get("idPrefixes")
        .and_then(Value::as_array)
        .or_else(|| manifest["idPrefixes"].as_array());
    resource != "meta"
        || prefixes.is_none_or(|items| {
            items.is_empty()
                || items
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|p| id.starts_with(p))
        })
}

pub async fn response(
    source: &Value,
    cache: &Cache,
    lang: &str,
    segments: &[String],
) -> Result<Value, &'static str> {
    let original = Url::parse(source["manifest_url"].as_str().ok_or("invalid source")?)
        .map_err(|_| "invalid source")?;
    let source_id = source["metadata_source_id"]
        .as_str()
        .ok_or("invalid source")?;
    let version = source["revision"].as_u64().ok_or("invalid source")?;
    let localized = localized_manifest(&original, lang);
    let manifest = if let Some(url) = localized.as_ref().filter(|url| **url != original) {
        let key = cache.key(source_id, "manifest", lang, &version.to_string());
        if let Some(value) = cache.get(&key).map_err(|_| "metadata cache read failed")? {
            value
        } else {
            let value = http::json(url.clone(), Default::default())
                .await
                .map_err(|_| "Stremio upstream request failed")?;
            if !value["resources"].is_array() || !value["types"].is_array() {
                return Err("invalid Stremio manifest");
            }
            cache
                .put(&key, &value, 6 * 3600)
                .map_err(|_| "metadata cache write failed")?;
            value
        }
    } else {
        source["manifest"].clone()
    };
    let parts: Vec<_> = segments.iter().map(String::as_str).collect();
    if parts.as_slice() == ["manifest.json"] {
        return Ok(manifest);
    }
    let strip = |s: &str| s.strip_suffix(".json").map(str::to_owned);
    let (name, kind, id, extra) = match parts.as_slice() {
        ["catalog", kind, id] => (
            "catalog",
            *kind,
            strip(id).ok_or("invalid resource path")?,
            None,
        ),
        ["catalog", kind, id, extra] => (
            "catalog",
            *kind,
            (*id).to_owned(),
            Some(strip(extra).ok_or("invalid resource path")?),
        ),
        ["meta", kind, id] => (
            "meta",
            *kind,
            strip(id).ok_or("invalid resource path")?,
            None,
        ),
        _ => return Err("unknown Stremio resource"),
    };
    if !matches!(kind, "movie" | "series") || id.is_empty() || id == "." || id == ".." {
        return Err("invalid resource path");
    }
    if !supported(&manifest, name, kind, &id) {
        return Err("unsupported Stremio capability");
    }
    if name == "catalog" {
        let catalog = manifest["catalogs"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["id"].as_str() == Some(&id) && c["type"].as_str() == Some(kind))
            .ok_or("unsupported Stremio catalog")?;
        let provided: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(extra.as_deref().unwrap_or("").as_bytes())
                .into_owned()
                .collect();
        for required in catalog["extra"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| v["isRequired"] == true)
        {
            if let Some(name) = required["name"].as_str() {
                if !provided.contains_key(name) {
                    return Err("required Stremio extra missing");
                }
            }
        }
    }
    let key = cache.key(
        source_id,
        name,
        lang,
        &json!([version, kind, id, extra]).to_string(),
    );
    if let Some(value) = cache.get(&key).map_err(|_| "metadata cache read failed")? {
        return Ok(value);
    }
    let mut url = localized.unwrap_or(original);
    {
        let mut path = url.path_segments_mut().map_err(|_| "invalid source")?;
        path.pop().push(name).push(kind);
        if let Some(ref extra) = extra {
            path.push(&id).push(&format!("{extra}.json"));
        } else {
            path.push(&format!("{id}.json"));
        }
    }
    let value = http::json(url, Default::default())
        .await
        .map_err(|_| "Stremio upstream request failed")?;
    if (name == "catalog" && !value["metas"].is_array())
        || (name == "meta" && !value["meta"].is_object())
    {
        return Err("invalid Stremio response");
    }
    let ttl = if extra.as_ref().is_some_and(|v| v.contains("search=")) {
        3600
    } else if name == "catalog" {
        6 * 3600
    } else {
        24 * 3600
    };
    cache
        .put(&key, &value, ttl)
        .map_err(|_| "metadata cache write failed")?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognized_tmdb_language_paths_are_localized() {
        let source = Url::parse("https://tmdb.elfhosted.com/en-US/manifest.json").unwrap();
        assert_eq!(
            localized_manifest(&source, "zh-CN").unwrap().as_str(),
            "https://tmdb.elfhosted.com/zh-CN/manifest.json"
        );
        assert!(
            localized_manifest(
                &Url::parse("https://example.com/en-US/manifest.json").unwrap(),
                "zh-CN"
            )
            .is_none()
        );
        assert!(
            localized_manifest(
                &Url::parse("https://tmdb.elfhosted.com/opaque/manifest.json").unwrap(),
                "zh-CN"
            )
            .is_none()
        );
    }
}
