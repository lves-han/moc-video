//! Private transport to the explicitly configured playback service.
use crate::{
    contracts::ApiError,
    host::maccms::{self, Response},
    requests::Job,
};
use reqwest::{
    Method, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap},
};
use serde_json::{Value, json};
use std::{sync::Mutex, time::Duration};
use url::Url;

pub const HOST: &str = "moc-service.invalid";
#[derive(Clone)]
pub struct Config {
    base: Url,
    token: String,
}
static CONFIG: Mutex<Option<Config>> = Mutex::new(None);

impl Config {
    pub fn parse(base: &str, token: &str) -> Result<Self, ApiError> {
        let mut base = maccms::http_url(base, true)?;
        if base.query().is_some()
            || base.fragment().is_some()
            || token.is_empty()
            || token.trim() != token
            || token.chars().any(char::is_control)
        {
            return Err(ApiError::bad("Service address or token is invalid"));
        }
        let path = format!("{}/", base.path().trim_end_matches('/'));
        base.set_path(&path);
        Ok(Self {
            base,
            token: token.into(),
        })
    }
    fn endpoint(&self, segments: &[&str]) -> Result<Url, ApiError> {
        let mut url = self.base.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| ApiError::bad("Invalid service address"))?;
            path.pop_if_empty();
            for segment in segments {
                if segment.is_empty() || matches!(*segment, "." | "..") {
                    return Err(ApiError::bad("Invalid service path"));
                }
                path.push(segment);
            }
        }
        Ok(url)
    }
    async fn request(
        &self,
        method: Method,
        url: Url,
        body: Option<Value>,
        bearer: bool,
    ) -> Result<(HeaderMap, Vec<u8>), ApiError> {
        if url.origin() != self.base.origin() || !url.path().starts_with(self.base.path()) {
            return Err(ApiError::bad("Service request escaped configured address"));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| ApiError::internal("Service client unavailable"))?;
        let deleting = method == Method::DELETE;
        let mut request = client.request(method, url);
        if bearer {
            request = request.header(AUTHORIZATION, format!("Bearer {}", self.token));
        }
        if let Some(body) = body {
            request = request
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string());
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| ApiError::new("UPSTREAM_FAILED", "Playback service connection failed"))?;
        if deleting && response.status() == StatusCode::NOT_FOUND {
            return Ok((response.headers().clone(), Vec::new()));
        }
        if response.status().is_redirection() {
            return Err(ApiError::new(
                "UPSTREAM_FAILED",
                "Playback service redirect refused",
            ));
        }
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(ApiError::new(
                "UNAUTHORIZED",
                "Playback service token was rejected",
            ));
        }
        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            let mut error = ApiError::new(
                "ENGINE_NOT_READY",
                "Playback service is preparing the home page",
            );
            error.retryable = true;
            return Err(error);
        }
        if !response.status().is_success() {
            return Err(ApiError::new(
                "UPSTREAM_FAILED",
                "Playback service request failed",
            ));
        }
        let headers = response.headers().clone();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ApiError::new("UPSTREAM_FAILED", "Playback service response failed"))?
        {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                return Err(ApiError::new(
                    "RESOURCE_LIMIT",
                    "Playback service response exceeds limit",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((headers, bytes))
    }
    pub async fn json(
        &self,
        method: Method,
        path: &[&str],
        body: Option<Value>,
    ) -> Result<Value, ApiError> {
        let url = self.endpoint(path)?;
        let (_, bytes) = self.request(method, url, body, true).await?;
        serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::new("UPSTREAM_FAILED", "Playback service response is invalid"))
    }
}

pub fn configure(settings: &Value) {
    let config = &settings["playback_service"];
    let next = if config["enabled"] == true {
        config["url"]
            .as_str()
            .zip(config["token"].as_str())
            .and_then(|(url, token)| Config::parse(url, token).ok())
    } else {
        None
    };
    *CONFIG.lock().expect("service configuration") = next;
}

pub fn proxy(virtual_url: &Url, job: &Job) -> Result<Response, ApiError> {
    let config = CONFIG
        .lock()
        .map_err(|_| ApiError::internal("Service configuration unavailable"))?
        .clone()
        .ok_or_else(|| ApiError::new("ENGINE_NOT_READY", "Playback service is not configured"))?;
    let parts = virtual_url
        .path_segments()
        .ok_or_else(|| ApiError::bad("Invalid service route"))?
        .map(|part| {
            percent_encoding::percent_decode_str(part)
                .decode_utf8()
                .map(|s| s.into_owned())
                .map_err(|_| ApiError::bad("Invalid service route"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() < 3 {
        return Err(ApiError::bad("Invalid service route"));
    }
    let mut segments = vec!["stremio", config.token.as_str()];
    segments.extend(parts.iter().map(String::as_str));
    let url = config.endpoint(&segments)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ApiError::internal("Service runtime unavailable"))?;
    let timeout = Duration::from_millis(job.child_timeout_ms()?);
    let (headers, body) = runtime.block_on(maccms::watched(
        job,
        timeout,
        config.request(Method::GET, url, None, false),
    ))?;
    Ok(Response {
        url: virtual_url.clone(),
        headers,
        body,
    })
}

pub async fn import(base: &str, token: &str, configs: &[Value]) -> Result<Value, ApiError> {
    let client = Config::parse(base, token)?;
    client.json(Method::GET, &["v1", "health"], None).await?;
    let mut groups = Vec::new();
    for config in configs.iter().filter(|v| v["kind"] == "stremio") {
        let Some(config_id) = config["config_id"].as_str() else {
            continue;
        };
        let Some(urls) = config["manifest_urls"].as_array() else {
            continue;
        };
        let mut addons = Vec::new();
        let mut invalid = false;
        for (order, url) in urls.iter().enumerate() {
            let Some(url) = url.as_str() else {
                return Err(ApiError::bad("Invalid Stremio source"));
            };
            let (key, localized) = match crate::host::stremio::migration_identity(url) {
                Ok(value) => value,
                Err(_) => {
                    invalid = true;
                    break;
                }
            };
            addons.push(json!({"addon_key":key,"manifest_url":url,"order":order,"namespace_rule":{"localized":localized}}));
        }
        if invalid {
            groups.push(json!({"config_id":config_id,"status":"pending","addons":[]}));
            continue;
        }
        let result = client
            .json(
                Method::POST,
                &["v1", "metadata-sources", "import"],
                Some(json!({"config_id":config_id,"addons":addons})),
            )
            .await;
        let mut mapped = Vec::new();
        if let Ok(result) = result {
            let readback = client
                .json(Method::GET, &["v1", "metadata-sources"], None)
                .await?;
            let records = readback["sources"].as_array().ok_or_else(|| {
                ApiError::new("UPSTREAM_FAILED", "Service source list is invalid")
            })?;
            if let Some(rows) = result["results"].as_array() {
                for (index, row) in rows.iter().enumerate() {
                    let Some(expected) = addons.get(index) else {
                        break;
                    };
                    let Some(id) = row["source"]["metadata_source_id"].as_str() else {
                        continue;
                    };
                    let found = records.iter().find(|record| {
                        record["metadata_source_id"] == id
                            && record["origin_config_id"] == config_id
                            && record["origin_addon_key"] == expected["addon_key"]
                            && record["origin_order"] == expected["order"]
                            && record["origin_namespace_rule"] == expected["namespace_rule"]
                            && record["manifest_url"] == expected["manifest_url"]
                    });
                    let Some(found) = found else { continue };
                    let localized = expected["namespace_rule"]["localized"] == true
                        && found["manifest"]["id"] == "tmdb-addon";
                    mapped.push(json!({"original_url":expected["manifest_url"],"source_id":id,"addon_key":expected["addon_key"],"localized":localized}));
                }
            }
        }
        let ready = mapped.len() == urls.len();
        groups.push(json!({"config_id":config_id,"status":if ready {"ready"} else {"pending"},"addons":mapped}));
    }
    Ok(json!({"groups":groups}))
}

pub async fn delete(base: &str, token: &str, ids: &[String]) -> Result<(), ApiError> {
    let client = Config::parse(base, token)?;
    for id in ids {
        uuid::Uuid::parse_str(id).map_err(|_| ApiError::bad("Invalid metadata source ID"))?;
        client
            .request(
                Method::DELETE,
                client.endpoint(&["v1", "metadata-sources", id])?,
                None,
                true,
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{history::History, host::Host, settings, source_dispatch::identity};
    use std::{net::TcpListener, sync::Arc};

    fn call(host: &Arc<Host>, command: &str, input: Value) -> Value {
        let result = host.invoke(
            command,
            json!({"request_id":uuid::Uuid::new_v4().to_string(),"input":input}),
            true,
        );
        assert!(result.get("error").is_none(), "{command}: {result}");
        result["data"].clone()
    }

    #[test]
    fn t15_redirect_refuses_cross_origin_token() {
        use std::io::{Read, Write};
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let target_addr = target.local_addr().unwrap();
        let redirect = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = redirect.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = redirect.accept().unwrap();
            let mut buffer = [0; 4096];
            let _ = stream.read(&mut buffer).unwrap();
            write!(stream,"HTTP/1.1 302 Found\r\nLocation: http://{target_addr}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = Config::parse(&format!("http://{origin}/base"), "private-token").unwrap();
        assert!(
            runtime
                .block_on(client.json(Method::GET, &["v1", "health"], None))
                .is_err()
        );
        worker.join().unwrap();
        assert!(
            target.accept().is_err(),
            "redirect target received a connection"
        );
    }

    #[test]
    #[ignore = "requires the controlled Docker playback service and addon fixtures"]
    fn t15_service_host_group_migration_and_records() {
        let endpoint = std::env::var("T15_SERVICE_URL").unwrap();
        let addon_base = std::env::var("T15_ADDON_BASE").unwrap();
        let control = std::env::var("T15_CONTROL_URL").unwrap();
        let container = std::env::var("T15_CONTAINER").unwrap();
        let token = "app-token";
        let config_id = uuid::Uuid::new_v4().to_string();
        let urls = vec![
            format!("{addon_base}/addon-a/manifest.json"),
            format!("{addon_base}/addon-b/manifest.json"),
        ];
        let config = json!({"kind":"stremio","config_id":config_id,"name":"Combined","enabled":true,"manifest_urls":urls});
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let set_fixture = |value: u8| {
            runtime.block_on(async {
                reqwest::get(format!("{control}/control?fail_addon_b={value}"))
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
            })
        };
        let temp = tempfile::tempdir().unwrap();
        let settings_path = temp.path().join("settings.json");
        let mut settings_value = settings::defaults();
        settings_value["source_configs"] = json!([config.clone()]);
        settings::SettingsFile {
            path: settings_path.clone(),
        }
        .save(&settings_value)
        .unwrap();
        let keys: Vec<_> = urls
            .iter()
            .map(|url| crate::host::stremio::migration_identity(url).unwrap().0)
            .collect();
        {
            let mut history = History::new(temp.path().join("moc-video.db"));
            for (index, key) in keys.iter().enumerate() {
                let namespace = format!("addon:{key}");
                let meta = json!({"config_id":config_id,"addon_key":key,"namespace":namespace,"type":"movie","meta_id":"same:42","canonical_id":"same:42"});
                let content_id = identity("metadata", &[&namespace, "movie", "same:42"]);
                let entry = json!({"content_id":content_id,"metadata_identity":meta,"title":format!("Addon {}",if index==0 {"A"} else {"B"}),"kind":"movie","poster_cache_key":null});
                history.add_watchlist(&entry).unwrap();
                let mut record = entry.clone();
                record.as_object_mut().unwrap().extend(json!({"video_id":"same:42","season_number":null,"episode_number":null,"source_id":"fixture","source_name":"Fixture","position_ms":7000,"duration_ms":20000,"completed":false,"updated_at":1000+index}).as_object().unwrap().clone());
                history
                    .commit(index as u64 + 1, &json!({"record":record}))
                    .unwrap();
            }
        }
        let host = Host::open(settings_path.clone()).unwrap();
        set_fixture(1);
        let partial = runtime
            .block_on(import(&endpoint, token, &[config.clone()]))
            .unwrap();
        assert_eq!(partial["groups"][0]["status"], "pending");
        assert_eq!(partial["groups"][0]["addons"].as_array().unwrap().len(), 1);
        let first_id = partial["groups"][0]["addons"][0]["source_id"].clone();
        let mut saved = call(&host, "get_settings", json!({}));
        saved["playback_service"] =
            json!({"enabled":true,"url":endpoint,"token":token,"groups":partial["groups"]});
        call(&host, "save_settings", saved.clone());
        assert_eq!(saved["source_configs"][0], config);
        assert_eq!(
            call(&host, "list_history", json!({}))["entries"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(
            !call(&host, "get_sources", json!({}))["providers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["origin"]["config_id"] == config_id)
        );
        set_fixture(0);
        let complete = runtime
            .block_on(import(&endpoint, token, &[config.clone()]))
            .unwrap();
        assert_eq!(complete["groups"][0]["status"], "ready");
        assert_eq!(complete["groups"][0]["addons"][0]["source_id"], first_id);
        saved["playback_service"]["groups"] = complete["groups"].clone();
        call(&host, "save_settings", saved);
        call(&host, "runtime_prepare", json!({}));
        call(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":config_id}),
        );
        let source = call(&host, "get_sources", json!({}))["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["origin"]["config_id"] == config_id)
            .unwrap()
            .clone();
        assert_eq!(source["runtime_status"], "ready");
        assert!(!source.to_string().contains(token));
        let found = call(
            &host,
            "search_catalog",
            json!({"source_id":source["id"],"source_revision":source["source_revision"],"query":"film"}),
        );
        assert_eq!(found["items"].as_array().unwrap().len(), 2);
        for (key, label) in keys.iter().zip(["Addon A", "Addon B"]) {
            let reference = json!({"source_id":source["id"],"source_revision":source["source_revision"],"data":json!(["stremio",key,"movie","same:42"]).to_string()});
            let details = call(&host, "get_details", json!({"ref":reference}));
            assert_eq!(details["details"]["title"], label);
            assert_eq!(details["metadata_ref"]["identity"]["addon_key"], *key);
        }
        assert_eq!(
            call(&host, "list_watchlist", json!({}))["entries"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(
            call(&host, "list_history", json!({}))["entries"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v["position_ms"] == 7000)
        );
        let status = std::process::Command::new("docker")
            .args(["restart", &container])
            .status()
            .unwrap();
        assert!(status.success());
        let output = std::process::Command::new("docker")
            .args(["port", &container, "8787/tcp"])
            .output()
            .unwrap();
        let port = String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .rsplit(':')
            .next()
            .unwrap()
            .to_owned();
        let restarted_url = format!("http://127.0.0.1:{port}");
        let probe = Config::parse(&restarted_url, token).unwrap();
        for _ in 0..50 {
            if runtime
                .block_on(probe.json(Method::GET, &["v1", "health"], None))
                .is_ok()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let restarted = Host::open(settings_path).unwrap();
        let mut reloaded = call(&restarted, "get_settings", json!({}));
        reloaded["playback_service"]["url"] = json!(restarted_url);
        call(&restarted, "save_settings", reloaded);
        call(&restarted, "runtime_prepare", json!({}));
        call(
            &restarted,
            "sync_catalog",
            json!({"kind":"stremio","config_id":config_id}),
        );
        let source_after = call(&restarted, "get_sources", json!({}))["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["origin"]["config_id"] == config_id)
            .unwrap()
            .clone();
        for (key, label) in keys.iter().zip(["Addon A", "Addon B"]) {
            let reference = json!({"source_id":source_after["id"],"source_revision":source_after["source_revision"],"data":json!(["stremio",key,"movie","same:42"]).to_string()});
            assert_eq!(
                call(&restarted, "get_details", json!({"ref":reference}))["details"]["title"],
                label
            );
        }
        assert_eq!(
            call(&restarted, "list_watchlist", json!({}))["entries"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            call(&restarted, "list_history", json!({}))["entries"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        println!("T15_SERVICE_HOST:partial,retry,search,identity,history,watchlist,restart");
    }
}
