use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use url::Url;

#[derive(Deserialize)]
struct XmlResponse {
    list: XmlList,
}
#[derive(Deserialize)]
struct XmlList {
    #[serde(default, rename = "video")]
    videos: Vec<XmlVideo>,
}
#[derive(Deserialize)]
struct XmlVideo {
    id: String,
    name: String,
    #[serde(default, rename = "type")]
    category: String,
    #[serde(default)]
    year: String,
    #[serde(default)]
    dl: Option<XmlLines>,
}
#[derive(Deserialize)]
struct XmlLines {
    #[serde(default, rename = "dd")]
    lines: Vec<XmlLine>,
}
#[derive(Deserialize)]
struct XmlLine {
    #[serde(rename = "@flag")]
    flag: String,
    #[serde(rename = "$text", default)]
    value: String,
}

pub(crate) fn headers(source: &Value) -> Result<HeaderMap, &'static str> {
    let mut headers = HeaderMap::new();
    for (name, value) in source["headers"]
        .as_object()
        .ok_or("invalid source headers")?
    {
        headers.insert(
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| "invalid source headers")?,
            HeaderValue::from_str(value.as_str().ok_or("invalid source headers")?)
                .map_err(|_| "invalid source headers")?,
        );
    }
    Ok(headers)
}

async fn query(source: &Value, parameters: &[(&str, &str)]) -> Result<Vec<Value>, &'static str> {
    if source["enabled"] != true {
        return Err("source is disabled");
    }
    let mut url = Url::parse(source["api"].as_str().ok_or("invalid source URL")?)
        .map_err(|_| "invalid source URL")?;
    let old = url
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|pair| {
            let key = url::form_urlencoded::parse(pair.as_bytes())
                .next()
                .map(|(key, _)| key);
            !key.is_some_and(|key| parameters.iter().any(|(name, _)| key == *name))
        })
        .collect::<Vec<_>>()
        .join("&");
    url.set_query((!old.is_empty()).then_some(old.as_str()));
    url.query_pairs_mut()
        .extend_pairs(parameters.iter().copied());
    let bytes = crate::metadata::http::bytes(url, headers(source)?)
        .await
        .map_err(|_| "source request failed")?;
    if source["kind"] == "maccms_xml" {
        parse_xml(&bytes)
    } else {
        parse_json(&bytes)
    }
}

fn parse_json(bytes: &[u8]) -> Result<Vec<Value>, &'static str> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid MacCMS JSON")?;
    if value
        .get("code")
        .is_some_and(|code| code != 1 && code != "1")
    {
        return Err("MacCMS returned an error code");
    }
    if value.get("closed").is_some_and(|value| {
        !matches!(value, Value::Null | Value::Bool(false))
            && *value != json!(0)
            && *value != json!("0")
    }) {
        return Err("MacCMS source is closed");
    }
    value["list"]
        .as_array()
        .cloned()
        .ok_or("MacCMS list is missing")
}

fn parse_xml(bytes: &[u8]) -> Result<Vec<Value>, &'static str> {
    let text = std::str::from_utf8(bytes).map_err(|_| "invalid MacCMS XML encoding")?;
    let parsed: XmlResponse = quick_xml::de::from_str(text).map_err(|_| "invalid MacCMS XML")?;
    Ok(parsed.list.videos.into_iter().map(|video| {
        let lines=video.dl.map(|dl|dl.lines).unwrap_or_default();
        json!({"vod_id":video.id,"vod_name":video.name,"type_name":video.category,"vod_year":video.year,
            "vod_play_from":lines.iter().map(|v|v.flag.as_str()).collect::<Vec<_>>().join("$$$"),
            "vod_play_url":lines.iter().map(|v|v.value.as_str()).collect::<Vec<_>>().join("$$$")})
    }).collect())
}

fn identity(item: &Value) -> Result<String, &'static str> {
    if let Some(value) = item["vod_id"].as_str().filter(|v| !v.is_empty()) {
        return Ok(value.to_owned());
    }
    if let Some(value) = item["vod_id"].as_u64() {
        return Ok(value.to_string());
    }
    Err("MacCMS content ID is missing")
}

pub(crate) fn card(item: &Value) -> Result<Value, &'static str> {
    let id = identity(item)?;
    let title = item["vod_name"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .ok_or("MacCMS title is missing")?;
    Ok(
        json!({"vod_id":id,"vod_name":title,"vod_year":item["vod_year"],"type_name":item["type_name"]}),
    )
}

pub async fn search(source: &Value, keyword: &str) -> Result<Vec<Value>, &'static str> {
    if keyword.trim().is_empty() {
        return Err("search query is empty");
    }
    query(source, &[("ac", "detail"), ("wd", keyword), ("pg", "1")])
        .await?
        .iter()
        .map(card)
        .collect()
}

pub async fn detail(source: &Value, id: &str) -> Result<Value, &'static str> {
    if id.is_empty() {
        return Err("content ID is empty");
    }
    let items = query(source, &[("ac", "detail"), ("ids", id)]).await?;
    normalize_detail(&items, id)
}

pub(crate) fn normalize_detail(items: &[Value], id: &str) -> Result<Value, &'static str> {
    let matches: Vec<_> = items
        .iter()
        .filter(|item| identity(item).ok().as_deref() == Some(id))
        .collect();
    if matches.len() != 1 {
        return Err("MacCMS content identity is missing or ambiguous");
    }
    let item = matches[0];
    let mut output = card(item)?;
    let field = |name: &str| match item.get(name) {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(value)) => Ok(value.as_str()),
        _ => Err("invalid MacCMS playback field"),
    };
    let groups = moc_core::maccms::playback(field("vod_play_from")?, field("vod_play_url")?)?;
    let mut names = HashMap::new();
    for group in &groups {
        *names.entry(group.name).or_insert(0_usize) += 1;
    }
    let mut normalized = Vec::new();
    for group in groups {
        let mut labels = HashMap::new();
        for episode in &group.episodes {
            *labels.entry(episode.label).or_insert(0_usize) += 1;
        }
        let episodes:Vec<_>=group.episodes.iter().map(|episode|json!({"label":episode.label,"url":episode.url,
            "locator":if names[group.name]==1 && !episode.label.is_empty() && labels[episode.label]==1 {Some(json!({"flag":group.name,"episode":episode.label}))} else {None}})).collect();
        normalized.push(json!({"flag":group.name,"episodes":episodes}));
    }
    output["groups"] = json!(normalized);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_xml_and_locators_preserve_urls() {
        let json_data=br#"{"code":1,"list":[{"vod_id":1,"vod_name":"Film","vod_play_from":"a$$$b","vod_play_url":"1$https://a.test/?sig=x$y#2$https://a.test/2$$$https://b.test/1"}]}"#;
        let xml_data=br#"<rss><list><video><id>1</id><name>Film</name><dl><dd flag="a"><![CDATA[1$https://a.test/?sig=x$y#2$https://a.test/2]]></dd><dd flag="b"><![CDATA[https://b.test/1]]></dd></dl></video></list></rss>"#;
        let j = parse_json(json_data).unwrap();
        let x = parse_xml(xml_data).unwrap();
        for item in [&j[0], &x[0]] {
            let groups = moc_core::maccms::playback(
                item["vod_play_from"].as_str().unwrap(),
                item["vod_play_url"].as_str().unwrap(),
            )
            .unwrap();
            assert_eq!(groups[0].episodes[0].url, "https://a.test/?sig=x$y");
            assert_eq!(groups[1].episodes[0].url, "https://b.test/1");
        }
        assert!(parse_xml(b"<rss><list><video>").is_err());
        assert!(parse_json(br#"{"code":0,"list":[]}"#).is_err());
        assert!(parse_json(br#"{"code":1,"closed":1,"list":[]}"#).is_err());
    }

    #[test]
    #[ignore = "requires controlled MacCMS fixture"]
    fn t16_live_fixture_json_xml_search_and_detail() {
        let base = std::env::var("T16_FIXTURE_HOST").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for (kind, path) in [("maccms_json", "maccms-json"), ("maccms_xml", "maccms-xml")] {
            let source = json!({"enabled":true,"kind":kind,"api":format!("{base}/{path}?sig=a%2Fb+c&wd=old"),"headers":{"X-Fixture":"allowed"}});
            let results = runtime.block_on(search(&source, "Film")).unwrap();
            assert_eq!(results[0]["vod_id"], "101");
            let result = runtime.block_on(detail(&source, "101")).unwrap();
            assert_eq!(result["groups"][0]["flag"], "hls");
            assert_eq!(
                result["groups"][0]["episodes"][0]["url"],
                "https://media.test/video?sig=a$b"
            );
            assert_eq!(
                result["groups"][0]["episodes"][0]["locator"],
                json!({"flag":"hls","episode":"Episode 1"})
            );
            assert!(result["groups"][1]["episodes"][0]["locator"].is_null());
            assert!(runtime.block_on(detail(&source, "wrong-id")).is_err());
        }
    }
}
