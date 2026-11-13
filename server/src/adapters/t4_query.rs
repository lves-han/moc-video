use reqwest::header::{HeaderName, HeaderValue};
use serde_json::{Value, json};
use std::collections::HashMap;
use url::Url;

use super::maccms;

fn extension(source: &Value) -> Option<String> {
    match &source["ext"] {
        Value::Null => None,
        Value::String(value) if value.is_empty() => None,
        Value::String(value) => Some(value.clone()),
        value => Some(value.to_string()),
    }
}

pub(crate) async fn get(
    source: &Value,
    endpoint: Option<Url>,
    parameters: &[(&str, &str)],
    include_extend: bool,
) -> Result<Value, &'static str> {
    if source["enabled"] != true {
        return Err("source is disabled");
    }
    let mut url = match endpoint {
        Some(url) => url,
        None => Url::parse(source["api"].as_str().ok_or("invalid source URL")?)
            .map_err(|_| "invalid source URL")?,
    };
    let ext = include_extend.then(|| extension(source)).flatten();
    let old = url
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|pair| {
            let key = url::form_urlencoded::parse(pair.as_bytes())
                .next()
                .map(|(key, _)| key);
            !key.is_some_and(|key| {
                parameters.iter().any(|(name, _)| *name == key) || ext.is_some() && key == "extend"
            })
        })
        .collect::<Vec<_>>()
        .join("&");
    url.set_query((!old.is_empty()).then_some(old.as_str()));
    {
        let mut query = url.query_pairs_mut();
        query.extend_pairs(parameters.iter().copied());
        if let Some(ext) = &ext {
            query.append_pair("extend", ext);
        }
    }
    let value = crate::metadata::http::json(url, maccms::headers(source)?)
        .await
        .map_err(|_| "T4 request failed or returned invalid JSON")?;
    if value
        .get("error")
        .is_some_and(|value| !value.is_null() && value != "")
    {
        return Err("T4 source returned an error");
    }
    Ok(value)
}

pub(crate) fn items(value: &Value) -> Result<Vec<Value>, &'static str> {
    value
        .as_array()
        .or_else(|| value["list"].as_array())
        .cloned()
        .ok_or("T4 list is missing")
}

pub async fn search(source: &Value, keyword: &str) -> Result<Vec<Value>, &'static str> {
    if keyword.trim().is_empty() {
        return Err("search query is empty");
    }
    items(&get(source, None, &[("wd", keyword), ("pg", "1")], true).await?)?
        .iter()
        .map(maccms::card)
        .collect()
}

pub async fn detail(source: &Value, id: &str) -> Result<Value, &'static str> {
    if id.is_empty() {
        return Err("content ID is empty");
    }
    let values = items(&get(source, None, &[("ac", "detail"), ("ids", id)], true).await?)?;
    maccms::normalize_detail(&values, id)
}

pub(crate) fn play_values(
    source: &Value,
    response: &Value,
    item: &Value,
) -> Result<Vec<Value>, &'static str> {
    let parse_flag = |name: &str| -> Result<bool, &'static str> {
        match response.get(name) {
            None | Some(Value::Null) | Some(Value::Bool(false)) => Ok(false),
            Some(Value::Bool(true)) => Ok(true),
            Some(Value::Number(n)) if n.as_i64() == Some(0) => Ok(false),
            Some(Value::Number(n)) if n.as_i64() == Some(1) => Ok(true),
            Some(Value::String(s)) if s == "0" || s.is_empty() => Ok(false),
            Some(Value::String(s)) if s == "1" => Ok(true),
            _ => Err("invalid T4 parse flag"),
        }
    };
    let requires_parse = parse_flag("parse")? || parse_flag("jx")?;
    let usable =
        |value: &&Value| !value.is_null() && value.as_str().is_none_or(|text| !text.is_empty());
    let raw_headers = response
        .get("header")
        .filter(usable)
        .or_else(|| response.get("headers").filter(usable))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let raw_headers = if let Value::String(value) = raw_headers {
        serde_json::from_str(&value).map_err(|_| "invalid T4 playback headers")?
    } else {
        raw_headers
    };
    let mut merged = maccms::headers(source)?;
    let headers = raw_headers
        .as_object()
        .ok_or("invalid T4 playback headers")?;
    if headers.len() > 32 {
        return Err("too many T4 playback headers");
    }
    for (name, value) in headers {
        let name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| "invalid T4 playback headers")?;
        if !crate::metadata::http::allowed_header(&name) {
            return Err("unsafe T4 playback header");
        }
        let value = HeaderValue::from_str(value.as_str().ok_or("invalid T4 playback headers")?)
            .map_err(|_| "invalid T4 playback headers")?;
        merged.insert(name, value);
    }
    let mut output_headers = serde_json::Map::new();
    for (name, value) in merged.iter() {
        output_headers.insert(
            name.as_str().into(),
            json!(value.to_str().map_err(|_| "invalid T4 playback headers")?),
        );
    }
    let urls: Vec<(Option<String>, String)> = match &response["url"] {
        Value::String(url) if !url.is_empty() => vec![(None, url.clone())],
        Value::Array(values)
            if !values.is_empty() && values.len() % 2 == 0 && values.len() <= 128 =>
        {
            values
                .chunks_exact(2)
                .map(|pair| {
                    let name = pair[0]
                        .as_str()
                        .filter(|name| !name.is_empty())
                        .ok_or("invalid T4 quality name")?;
                    let url = pair[1]
                        .as_str()
                        .filter(|url| !url.is_empty())
                        .ok_or("invalid T4 playback URL")?;
                    Ok((Some(name.to_owned()), url.to_owned()))
                })
                .collect::<Result<_, &'static str>>()?
        }
        _ => return Err("T4 playback URL missing"),
    };
    let mut names = HashMap::new();
    for (name, _) in &urls {
        if let Some(name) = name {
            *names.entry(name.clone()).or_insert(0_usize) += 1;
        }
    }
    let mut result = Vec::new();
    for (quality, url) in urls {
        let mut line = item.clone();
        let original = line["episode"]["locator"].clone();
        line["episode"]["url"] = json!(url);
        line["episode"]["headers"] = json!(output_headers);
        line["episode"]["requires_parse"] = json!(requires_parse);
        line["episode"]["quality"] = json!(quality);
        let kind_hint = response["type"]
            .as_str()
            .map(str::to_ascii_lowercase)
            .and_then(|value| match value.as_str() {
                "hls" | "m3u8" | "application/vnd.apple.mpegurl" | "application/x-mpegurl" => {
                    Some("hls")
                }
                _ => None,
            });
        line["episode"]["kind_hint"] = json!(kind_hint);
        line["episode"]["resolved"] = json!(true);
        if let Some(quality) = &quality {
            if !original.is_null() && names.get(quality) == Some(&1) {
                line["episode"]["locator"]["variant"] = json!(quality);
            } else {
                line["episode"]["locator"] = Value::Null;
            }
        }
        result.push(line);
    }
    Ok(result)
}

pub async fn play(
    source: &Value,
    flag: &str,
    play: &str,
    item: &Value,
) -> Result<Vec<Value>, &'static str> {
    let response = get(source, None, &[("flag", flag), ("play", play)], true).await?;
    play_values(source, &response, item)
}
