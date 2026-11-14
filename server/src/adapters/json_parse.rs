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
    let all = store.list().await.map_err(|_| "parser database failed")?;
    let ids = source["parse_ids"].as_array();
    let parsers: Vec<Value> = if ids.is_some_and(|v| !v.is_empty()) {
        ids.unwrap()
            .iter()
            .filter_map(|id| all.iter().find(|parser| parser["id"] == *id).cloned())
            .collect()
    } else {
        all
    };
    let mut output: Vec<Option<Value>> = vec![None; lines.len()];
    let mut jobs = tokio::task::JoinSet::new();
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    for (index, original) in lines.iter().enumerate() {
        let mut item = original.clone();
        if item["episode"]["requires_parse"] != true {
            output[index] = Some(item);
            continue;
        }
        let flag = item["flag"].as_str().unwrap_or("").to_owned();
        let matching: Vec<Value> = parsers
            .iter()
            .filter(|p| {
                p["enabled"] == true
                    && p["flags"].as_array().is_some_and(|flags| {
                        flags.is_empty() || flags.iter().any(|v| v == flag.as_str() || v == "*")
                    })
            })
            .cloned()
            .collect();
        let slots = slots.clone();
        jobs.spawn(async move {
            let _permit = slots.acquire().await.ok();
            let mut reason = None;
            for parser in &matching {
                match parse(parser, &item).await {
                    Ok((url, headers)) => {
                        item["episode"]["url"] = json!(url);
                        item["episode"]["headers"] = headers;
                        item["episode"]["requires_parse"] = json!(false);
                        item["episode"]["unsupported_reason"] = Value::Null;
                        reason = None;
                        break;
                    }
                    Err(failure) => reason = Some(failure),
                }
            }
            if let Some(reason) = reason {
                item["episode"]["unsupported_reason"] = json!(reason);
            }
            (index, item)
        });
    }
    while let Some(result) = jobs.join_next().await {
        let (index, item) = result.map_err(|_| "parser task failed")?;
        output[index] = Some(item);
    }
    Ok(output.into_iter().flatten().collect())
}

/// Why the last tried parser produced no playable address.
async fn parse(parser: &Value, item: &Value) -> Result<(String, Value), &'static str> {
    const CONFIG: &str = "parser_config_invalid";
    const HEADERS: &str = "parser_headers_invalid";
    let mut api = parser["api"]
        .as_str()
        .and_then(|api| Url::parse(api).ok())
        .ok_or(CONFIG)?;
    let parameter = parser["url_param"].as_str().ok_or(CONFIG)?;
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
    api.query_pairs_mut().append_pair(
        parameter,
        item["episode"]["url"]
            .as_str()
            .ok_or("parser_page_missing")?,
    );
    let headers = super::maccms::headers(parser).map_err(|_| CONFIG)?;
    let body = crate::metadata::http::bytes(api, headers)
        .await
        .map_err(|_| "parser_request_failed")?;
    // A web page or malformed body instead of the configured JSON answer.
    let response: Value = serde_json::from_slice(&body).map_err(|_| "parser_invalid_response")?;
    let value = parser["url_field"]
        .as_str()
        .ok_or(CONFIG)?
        .split('.')
        .try_fold(&response, |v, k| v.get(k))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("parser_url_missing")?;
    let parsed = Url::parse(value).map_err(|_| "parser_url_invalid")?;
    if !crate::metadata::http::valid(&parsed) {
        return Err("parser_url_invalid");
    }
    let mut headers = super::maccms::headers(&json!({"headers":item["episode"]["headers"]}))
        .map_err(|_| HEADERS)?;
    let raw = response
        .get("header")
        .or_else(|| response.get("headers"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let raw = if let Value::String(s) = raw {
        serde_json::from_str(&s).map_err(|_| HEADERS)?
    } else {
        raw
    };
    let response_headers = super::maccms::headers(&json!({"headers":raw})).map_err(|_| HEADERS)?;
    if response_headers.len() > 32 {
        return Err(HEADERS);
    }
    for (name, value) in response_headers.iter() {
        if !crate::metadata::http::allowed_header(&name) {
            return Err(HEADERS);
        }
        headers.insert(name.clone(), value.clone());
    }
    let mut result = serde_json::Map::new();
    for (name, value) in headers.iter() {
        result.insert(
            name.as_str().into(),
            json!(value.to_str().map_err(|_| HEADERS)?),
        );
    }
    Ok((value.to_owned(), Value::Object(result)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_failures_report_their_reason() {
        use std::io::{Read, Write};
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in server.incoming() {
                let mut stream = stream.unwrap();
                let mut request = [0; 4096];
                let size = stream.read(&mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..size]);
                let body = if request.contains("page-html") {
                    "<html>error</html>"
                } else if request.contains("page-missing") {
                    r#"{"data":{}}"#
                } else if request.contains("page-bad") {
                    r#"{"url":"javascript:alert(1)"}"#
                } else {
                    r#"{"url":"https://media.test/v.m3u8"}"#
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let store = crate::parsers::Store::new(dir.path());
        runtime
            .block_on(store.save(
                &json!({"name":"P","api":format!("http://{address}/?url="),"flags":["vip"]}),
                None,
            ))
            .unwrap();
        let line = |page: &str| json!({"flag":"vip","episode":{"label":"1","url":format!("https://page.test/{page}"),"requires_parse":true,"headers":{}}});
        let lines = runtime
            .block_on(resolve(
                &store,
                &json!({"parse_ids":[]}),
                &[
                    line("page-ok"),
                    line("page-html"),
                    line("page-missing"),
                    line("page-bad"),
                ],
            ))
            .unwrap();
        assert_eq!(lines[0]["episode"]["url"], "https://media.test/v.m3u8");
        assert_eq!(lines[0]["episode"]["requires_parse"], false);
        let reasons: Vec<_> = lines[1..]
            .iter()
            .map(|line| line["episode"]["unsupported_reason"].clone())
            .collect();
        assert_eq!(
            reasons,
            vec![
                json!("parser_invalid_response"),
                json!("parser_url_missing"),
                json!("parser_url_invalid")
            ]
        );
    }
}
