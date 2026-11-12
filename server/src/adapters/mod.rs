pub mod maccms;
pub mod t4_drpyjs0;
pub mod t4_query;
pub mod t4_rest;

use serde_json::Value;

pub async fn search(source: &Value, keyword: &str) -> Result<Vec<Value>, &'static str> {
    match source["kind"].as_str() {
        Some("maccms_json" | "maccms_xml") => maccms::search(source, keyword).await,
        Some("t4_query") => t4_query::search(source, keyword).await,
        Some("t4_drpyjs0") => t4_drpyjs0::search(source, keyword).await,
        Some("t4_rest") => t4_rest::search(source, keyword).await,
        _ => Err("unsupported source kind"),
    }
}

pub async fn detail(source: &Value, id: &str) -> Result<Value, &'static str> {
    match source["kind"].as_str() {
        Some("maccms_json" | "maccms_xml") => maccms::detail(source, id).await,
        Some("t4_query") => t4_query::detail(source, id).await,
        Some("t4_drpyjs0") => t4_drpyjs0::detail(source, id).await,
        Some("t4_rest") => t4_rest::detail(source, id).await,
        _ => Err("unsupported source kind"),
    }
}

pub async fn resolve(source: &Value, lines: &[Value]) -> Result<Vec<Value>, &'static str> {
    let mut output = Vec::new();
    for item in lines {
        if item["episode"]["resolved"] == true
            || matches!(source["kind"].as_str(), Some("maccms_json" | "maccms_xml"))
        {
            output.push(item.clone());
            continue;
        }
        let flag = item["flag"].as_str().ok_or("T4 line flag missing")?;
        let play = item["episode"]["url"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or("T4 playback ID missing")?;
        let lines = match source["kind"].as_str() {
            Some("t4_query") => t4_query::play(source, flag, play, item).await?,
            Some("t4_drpyjs0") => t4_drpyjs0::play(source, flag, play, item).await?,
            Some("t4_rest") => t4_rest::play(source, flag, play, item).await?,
            _ => return Err("unsupported source kind"),
        };
        output.extend(lines);
        if output.len() > 1024 {
            return Err("too many T4 lines");
        }
    }
    Ok(output)
}
