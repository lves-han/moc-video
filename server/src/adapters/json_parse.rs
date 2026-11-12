use serde_json::{Value, json};
use url::Url;

pub async fn resolve(
    store: &crate::parsers::Store,
    source: &Value,
    lines: &[Value],
) -> Result<Vec<Value>, &'static str> {
    if !lines.iter().any(|v| v["episode"]["requires_parse"] == true) {
        return Ok(lines.to_vec());
    }
    let all = store.list().map_err(|_| "parser database failed")?;
    let ids = source["parse_ids"].as_array();
    let parsers: Vec<&Value> = if ids.is_some_and(|v| !v.is_empty()) {
        ids.unwrap()
            .iter()
            .filter_map(|id| all.iter().find(|parser| parser["id"] == *id))
            .collect()
    } else {
        all.iter().collect()
    };
    let mut output = Vec::with_capacity(lines.len());
    for original in lines {
        let mut item = original.clone();
        if item["episode"]["requires_parse"] != true {
            output.push(item);
            continue;
        }
        let flag = item["flag"].as_str().unwrap_or("");
        let matching: Vec<_> = parsers
            .iter()
            .filter(|p| {
                p["enabled"] == true
                    && p["flags"].as_array().is_some_and(|flags| {
                        flags.is_empty() || flags.iter().any(|v| v == flag || v == "*")
                    })
            })
            .collect();
        for parser in &matching {
            if let Some((url, headers)) = parse(parser, &item).await {
                item["episode"]["url"] = json!(url);
                item["episode"]["headers"] = headers;
                item["episode"]["requires_parse"] = json!(false);
                item["episode"]["unsupported_reason"] = Value::Null;
                break;
            }
        }
        if !matching.is_empty() && item["episode"]["requires_parse"] == true {
            item["episode"]["unsupported_reason"] = json!("parser_failed");
        }
        output.push(item);
    }
    Ok(output)
}

async fn parse(parser: &Value, item: &Value) -> Option<(String, Value)> {
    let mut api = Url::parse(parser["api"].as_str()?).ok()?;
    let parameter = parser["url_param"].as_str()?;
    let existing = api
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|pair| {
            !url::form_urlencoded::parse(pair.as_bytes())
                .next()
                .is_some_and(|(key, _)| key == parameter)
        })
        .collect::<Vec<_>>()
        .join("&");
    api.set_query((!existing.is_empty()).then_some(existing.as_str()));
    api.query_pairs_mut()
        .append_pair(parameter, item["episode"]["url"].as_str()?);
    let headers = super::maccms::headers(parser).ok()?;
    let response = crate::metadata::http::json(api, headers).await.ok()?;
    let value = parser["url_field"]
        .as_str()?
        .split('.')
        .try_fold(&response, |v, k| v.get(k))?
        .as_str()?;
    let parsed = Url::parse(value).ok()?;
    if !crate::metadata::http::valid(&parsed) {
        return None;
    }
    let mut headers =
        super::maccms::headers(&json!({"headers":item["episode"]["headers"]})).ok()?;
    let raw = response
        .get("header")
        .or_else(|| response.get("headers"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let raw = if let Value::String(s) = raw {
        serde_json::from_str(&s).ok()?
    } else {
        raw
    };
    let response_headers = super::maccms::headers(&json!({"headers":raw})).ok()?;
    if response_headers.len() > 32 {
        return None;
    }
    for (name, value) in response_headers.iter() {
        if matches!(
            name.as_str(),
            "host" | "content-length" | "connection" | "transfer-encoding" | "proxy-authorization"
        ) {
            return None;
        }
        headers.insert(name.clone(), value.clone());
    }
    let mut result = serde_json::Map::new();
    for (name, value) in headers.iter() {
        result.insert(name.as_str().into(), json!(value.to_str().ok()?));
    }
    Some((value.to_owned(), Value::Object(result)))
}
