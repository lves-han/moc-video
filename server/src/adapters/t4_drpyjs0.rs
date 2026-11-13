use super::{maccms, t4_query};
use serde_json::Value;
use url::Url;

pub async fn search(source: &Value, keyword: &str) -> Result<Vec<Value>, &'static str> {
    if keyword.trim().is_empty() {
        return Err("search query is empty");
    }
    t4_query::items(
        &t4_query::get(
            source,
            None,
            &[("ac", "detail"), ("wd", keyword), ("pg", "1")],
            false,
        )
        .await?,
    )?
    .iter()
    .map(maccms::card)
    .collect()
}

pub async fn detail(source: &Value, id: &str) -> Result<Value, &'static str> {
    if id.is_empty() {
        return Err("content ID is empty");
    }
    let values = t4_query::items(
        &t4_query::get(source, None, &[("ac", "detail"), ("ids", id)], false).await?,
    )?;
    maccms::normalize_detail(&values, id)
}

pub async fn play(
    source: &Value,
    _flag: &str,
    play: &str,
    item: &Value,
) -> Result<Vec<Value>, &'static str> {
    let base = Url::parse(source["api"].as_str().ok_or("invalid source URL")?)
        .map_err(|_| "invalid source URL")?;
    let endpoint = base
        .join("/web/302redirect")
        .map_err(|_| "invalid redirect endpoint")?;
    let response = t4_query::get(source, Some(endpoint), &[("url", play)], false).await?;
    let mut response = response;
    let urls: Vec<String> = match &response["url"] {
        Value::String(url) => vec![url.clone()],
        Value::Array(values) => values
            .iter()
            .skip(1)
            .step_by(2)
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    if response.get("parse").is_none() {
        let declared_hls = response["type"].as_str().is_some_and(|kind| {
            matches!(
                kind.to_ascii_lowercase().as_str(),
                "hls" | "m3u8" | "application/vnd.apple.mpegurl" | "application/x-mpegurl"
            )
        });
        let direct = !urls.is_empty()
            && (declared_hls
                || urls.iter().all(|url| {
                    url.split('?').next().is_some_and(|path| {
                        [".m3u8", ".mp4", ".mpd", ".flv", ".mkv"]
                            .iter()
                            .any(|suffix| path.to_ascii_lowercase().ends_with(suffix))
                    })
                }));
        response["parse"] = Value::Bool(!direct);
    }
    if response.get("jx").is_none() {
        let vip = urls
            .iter()
            .filter_map(|url| Url::parse(url).ok())
            .filter_map(|url| url.host_str().map(str::to_owned))
            .any(|host| {
                [
                    "iqiyi.com",
                    "iq.com",
                    "mgtv.com",
                    "qq.com",
                    "youku.com",
                    "le.com",
                    "sohu.com",
                    "pptv.com",
                    "bilibili.com",
                    "tudou.com",
                ]
                .iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
            });
        response["jx"] = Value::Bool(vip);
    }
    t4_query::play_values(source, &response, item)
}
