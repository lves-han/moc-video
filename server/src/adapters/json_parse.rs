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
        output.push(item);
    }
    Ok(output)
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
        let store = crate::parsers::Store::new(dir.path());
        store
            .save(
                &json!({"name":"P","api":format!("http://{address}/?url="),"flags":["vip"]}),
                None,
            )
            .unwrap();
        let line = |page: &str| json!({"flag":"vip","episode":{"label":"1","url":format!("https://page.test/{page}"),"requires_parse":true,"headers":{}}});
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
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
