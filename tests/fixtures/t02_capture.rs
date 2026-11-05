//! Preparatory public evidence acquisition. Explicitly ignored in ordinary tests.
//! Uses production DNS/peer/TLS/redirect/cancellation checks; never logs raw URLs.
use super::*;
use sha2::{Digest, Sha256};

const INVENTORY: &str = include_str!("../../docs/compatibility-report.json");

fn text(value: &str) -> String {
    // Metadata may embed URLs inside HTML or prose. Remove the whole URL token,
    // including arbitrary signature parameter names, before serializing evidence.
    let mut result = value.to_owned();
    loop {
        let position = [result.find("http://"), result.find("https://")]
            .into_iter()
            .flatten()
            .min();
        let Some(start) = position else { break };
        let end = result[start..]
            .char_indices()
            .find(|(_, c)| c.is_whitespace() || matches!(c, '\'' | '"' | '<' | '>'))
            .map_or(result.len(), |(offset, _)| start + offset);
        result.replace_range(start..end, "[native-url-redacted]");
    }
    result
}

fn scalar(value: &Value) -> Value {
    match value {
        Value::String(value) => json!(text(value)),
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        _ => json!({"invalid_field_type": if value.is_array() {"array"} else {"object"}}),
    }
}

fn playback(value: &Value) -> Value {
    let Some(value) = value.as_str() else {
        return scalar(value);
    };
    json!(
        value
            .split("$$$")
            .enumerate()
            .map(|(group, value)| {
                value
                    .split('#')
                    .enumerate()
                    .map(|(episode, entry)| {
                        if entry.is_empty() {
                            return String::new();
                        }
                        let (label, resource) =
                            if entry.starts_with("http://") || entry.starts_with("https://") {
                                (None, entry)
                            } else if let Some((label, resource)) = entry.split_once('$') {
                                (Some(label), resource)
                            } else {
                                (None, entry)
                            };
                        let replacement = if resource.is_empty() {
                            String::new()
                        } else if let Ok(url) = http_url(resource, false) {
                            let extension = if url.path().ends_with(".m3u8") {
                                "m3u8"
                            } else if url.path().ends_with(".mp4") {
                                "mp4"
                            } else {
                                "bin"
                            };
                            format!(
                                "http://maccms.fixture.test:0/t02/{group}/{episode}.{extension}"
                            )
                        } else {
                            "native-unsupported-resource".into()
                        };
                        match label {
                            Some(label) => format!("{}${replacement}", text(label)),
                            None => replacement,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("#")
            })
            .collect::<Vec<_>>()
            .join("$$$")
    )
}

fn projection(raw: &Value) -> Value {
    let mut safe = json!({});
    for field in [
        "code",
        "msg",
        "page",
        "pagecount",
        "limit",
        "total",
        "closed",
    ] {
        if let Some(value) = raw.get(field) {
            safe[field] = scalar(value);
        }
    }
    if let Some(classes) = raw.get("class") {
        safe["class"] = classes.as_array().map_or_else(
            || scalar(classes),
            |classes| {
                json!(
                    classes
                        .iter()
                        .map(|class| {
                            let mut safe = json!({});
                            for key in ["type_id", "type_name"] {
                                if let Some(value) = class.get(key) {
                                    safe[key] = scalar(value);
                                }
                            }
                            safe
                        })
                        .collect::<Vec<_>>()
                )
            },
        );
    }
    if let Some(items) = raw.get("list") {
        safe["list"] = items.as_array().map_or_else(
            || scalar(items),
            |items| {
                json!(
                    items
                        .iter()
                        .map(|item| {
                            let mut safe = json!({});
                            for field in [
                                "vod_id",
                                "vod_name",
                                "type_id",
                                "type_name",
                                "vod_year",
                                "vod_score",
                                "vod_isend",
                                "vod_remarks",
                                "vod_play_from",
                                "vod_content",
                                "vod_actor",
                                "vod_director",
                                "vod_area",
                                "vod_lang",
                                "vod_class",
                            ] {
                                if let Some(value) = item.get(field) {
                                    safe[field] = scalar(value);
                                }
                            }
                            if let Some(value) = item.get("vod_play_url") {
                                safe["vod_play_url"] = playback(value);
                            }
                            if let Some(value) = item.get("vod_pic") {
                                safe["vod_pic"] = if value.as_str().is_some_and(|s| !s.is_empty()) {
                                    json!("/t02-redacted-poster.png")
                                } else {
                                    scalar(value)
                                };
                            }
                            safe
                        })
                        .collect::<Vec<_>>()
                )
            },
        );
    }
    safe
}

fn fetch(
    http: &Http,
    url: Url,
    job: &Job,
    maximum: usize,
    probe: bool,
    record: &mut Value,
) -> Result<Response, ApiError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ApiError::internal("Capture runtime initialization failed"))?;
    runtime.block_on(watched(
        job,
        Duration::from_millis(job.child_timeout_ms()?),
        async {
            let mut headers = HeaderMap::new();
            if probe {
                headers.insert("range", "bytes=0-65535".parse().unwrap());
            }
            let mut response = http
                .open(url, headers, Method::GET, job.connect_timeout_ms()?, true)
                .await?;
            record["http_status"] = json!(response.status().as_u16());
            // HTTP reachability is recorded separately, before any protocol verdict.
            if !response.status().is_success() {
                record["status"] = json!(if matches!(
                    response.status().as_u16(),
                    401 | 403 | 407 | 429 | 451
                ) {
                    "ACCESS_CONTROL_STOP"
                } else {
                    "HTTP_FAILED_STOP"
                });
                return Err(upstream(
                    "Public capture HTTP request failed; candidate stopped",
                ));
            }
            if !probe
                && response
                    .content_length()
                    .is_some_and(|size| size > maximum as u64)
            {
                return Err(limit());
            }
            let (url, headers) = (response.url().clone(), response.headers().clone());
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(http_error)? {
                job.check()?;
                if probe {
                    body.extend_from_slice(&chunk[..chunk.len().min(maximum - body.len())]);
                    if body.len() == maximum {
                        break;
                    }
                } else {
                    if body.len() + chunk.len() > maximum {
                        return Err(limit());
                    }
                    body.extend_from_slice(&chunk);
                }
            }
            record["bytes"] = json!(body.len());
            record["raw_sha256"] = json!(format!("{:x}", Sha256::digest(&body)));
            Ok(Response { url, headers, body })
        },
    ))
}

fn capture(
    http: &Http,
    host: &Host,
    base: &str,
    stage: &str,
    parameters: &[(&str, &str)],
    records: &mut Vec<Value>,
) -> Result<Value, ApiError> {
    let mut record = json!({"stage":stage,"status":"NOT RUN"});
    let result = (|| -> Result<Value, ApiError> {
        let response = fetch(
            http,
            api_url(base, parameters)?,
            &job(host, "http_source_sync"),
            RESPONSE_LIMIT,
            false,
            &mut record,
        )?;
        let body = String::from_utf8_lossy(&response.body).to_ascii_lowercase();
        if [
            "cloudflare",
            "captcha",
            "cf-chl-",
            "access denied",
            "验证码",
            "访问受限",
        ]
        .iter()
        .any(|marker| body.contains(marker))
        {
            record["status"] = json!("ACCESS_CONTROL_STOP");
            return Err(upstream(
                "Public capture encountered access control; candidate stopped",
            ));
        }
        let raw: Value = serde_json::from_slice(&response.body).map_err(|_| {
            record["status"] = json!("NON_JSON_STOP");
            upstream("Public capture returned non-JSON; candidate stopped")
        })?;
        record["response"] = projection(&raw);
        record["adapter_decode"] = match decode(&response) {
            Ok(_) => json!({"status":"PASS"}),
            Err(error) => json!({"status":"FAIL","code":error.code,"message":error.message}),
        };
        record["status"] = json!("JSON_CAPTURED");
        Ok(raw)
    })();
    if let Err(error) = &result {
        if record["status"] == "NOT RUN" {
            record["status"] = json!("FAILED_STOP");
        }
        record["error"] = json!({"code":error.code,"message":error.message});
    }
    records.push(record);
    result
}

fn direct_episode<'a>(original: &Value, groups: &'a [Value]) -> Option<(usize, &'a Value)> {
    // Public JSON may include both an HTML player and a direct media route.
    // Select a declared MP4/HLS route without executing the HTML player.
    original["vod_play_url"]
        .as_str()?
        .split("$$$")
        .enumerate()
        .find_map(|(group, entries)| {
            entries.split('#').enumerate().find_map(|(episode, entry)| {
                let resource = if entry.starts_with("http://") || entry.starts_with("https://") {
                    entry
                } else {
                    entry.split_once('$')?.1
                };
                let url = http_url(resource, false).ok()?;
                let path = url.path().to_ascii_lowercase();
                if path.ends_with(".m3u8") || path.ends_with(".mp4") {
                    Some((group, &groups.get(group)?["episodes"][episode]))
                } else {
                    None
                }
            })
        })
}

fn candidate(source: &Value) -> Value {
    let base = source["api_url"].as_str().unwrap();
    let mut records = Vec::new();
    let (_temporary, host) = setup(base.into());
    let http = Http::default();
    let result = (|| -> Result<(), ApiError> {
        let list = capture(
            &http,
            &host,
            base,
            "list",
            &[("ac", "list"), ("pg", "1")],
            &mut records,
        )?;
        let details = capture(
            &http,
            &host,
            base,
            "detail_page",
            &[("ac", "detail"), ("pg", "1")],
            &mut records,
        )?;
        let first = details["list"]
            .as_array()
            .and_then(|items| items.first())
            .ok_or_else(|| upstream("Public detail page has no sample item"))?;
        let id = identifier(&first["vod_id"])?;
        let title = first["vod_name"]
            .as_str()
            .ok_or_else(|| upstream("Public sample title missing"))?;
        let original = capture(
            &http,
            &host,
            base,
            "detail_id",
            &[("ac", "detail"), ("ids", &id)],
            &mut records,
        )?;
        capture(
            &http,
            &host,
            base,
            "search",
            &[("ac", "detail"), ("wd", title), ("pg", "1")],
            &mut records,
        )?;
        let zero_keyword = format!("yingxiang-t02-zero-{}", Uuid::new_v4());
        capture(
            &http,
            &host,
            base,
            "zero_search",
            &[("ac", "detail"), ("wd", &zero_keyword), ("pg", "1")],
            &mut records,
        )?;
        if let Some(class) = list["class"].as_array().and_then(|classes| classes.first()) {
            let category = identifier(&class["type_id"])?;
            for page in ["1", "2"] {
                capture(
                    &http,
                    &host,
                    base,
                    &format!("category_page_{page}"),
                    &[("ac", "detail"), ("t", &category), ("pg", page)],
                    &mut records,
                )?;
            }
        }
        let catalog = Catalog::sync(
            host.capture_source(CONFIG, false)?,
            &http,
            &job(&host, "http_source_sync"),
        )?;
        let mut images = Vec::new();
        let reference = catalog.card(first, &mut images)?["ref"].clone();
        let mapped = catalog.details(&http, &reference, &job(&host, "default_metadata"))?;
        let groups = mapped.value["details"]["groups"].as_array().unwrap();
        records.push(json!({"stage":"adapter_details","status":"PASS","groups":groups.len(),
            "episodes":groups.iter().map(|g| g["episodes"].as_array().unwrap().len()).sum::<usize>()}));
        let (group_index, episode) = direct_episode(&original["list"][0], groups)
            .ok_or_else(|| upstream("Public sample has no declared direct MP4/HLS episode"))?;
        let link = catalog.resolve(&http, &episode["ref"], &job(&host, "resolve"))?;
        let mut record = json!({"stage":"direct_media_prefix","status":"NOT RUN","group_index":group_index,"kind":link.kind,"playable":link.playable});
        if !link.playable {
            record["status"] = json!("UNSUPPORTED");
            records.push(record);
            return Err(upstream("Public sample direct media is unsupported"));
        }
        let response = fetch(
            &http,
            http_url(&link.url, false)?,
            &job(&host, "resolve"),
            PROBE_LIMIT,
            true,
            &mut record,
        );
        match response {
            Ok(response) => {
                let (kind, _, reason) = classify(&response);
                record["observed_kind"] = json!(kind);
                record["reason"] = json!(reason);
                record["status"] = json!(if reason.is_none()
                    && matches!(kind, "progressive" | "hls")
                {
                    "PREFIX_ONLY"
                } else {
                    "FAIL"
                });
            }
            Err(error) => {
                if record["status"] == "NOT RUN" {
                    record["status"] = json!("FAILED_STOP");
                }
                record["error"] = json!({"code":error.code,"message":error.message});
            }
        }
        let failed = record["status"] != "PREFIX_ONLY";
        records.push(record);
        if failed {
            Err(upstream(
                "Public direct media verification failed; candidate stopped",
            ))
        } else {
            Ok(())
        }
    })();
    json!({"name":source["name"],"api_url":base,"utc":OffsetDateTime::now_utc().format(&Rfc3339).unwrap(),
        "capture_status":if result.is_ok() {"CAPTURED"} else {"STOPPED"},
        "error":result.err().map(|e| json!({"code":e.code,"message":e.message})),"records":records,
        "supported":false,"real_gateway_webview":"NOT RUN"})
}

#[test]
fn t02_direct_route_selection_preserves_later_hls_group() {
    let groups = vec![
        json!({"episodes":[{"ref":{"data":"html"}}]}),
        json!({"episodes":[{"ref":{"data":"hls"}}]}),
    ];
    let original = json!({"vod_play_url":"Play$https://public.example/player?id=1$$$Play$https://public.example/index.m3u8?signature=PRIVATE"});
    let (index, episode) = direct_episode(&original, &groups).unwrap();
    assert_eq!(index, 1);
    assert_eq!(episode["ref"]["data"], "hls");
    assert!(direct_episode(&json!({"vod_play_url":"javascript:play()"}), &groups).is_none());
}

fn publish(path: &std::path::Path, value: &Value) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

#[test]
#[ignore = "A1 only: real public JSON/media and a packaged WebView client must own this bridge"]
fn t02_live_public_gateway_webview() {
    assert_eq!(std::env::var("AGENT_WORKFLOW_RUN").as_deref(), Ok("1"));
    assert_eq!(std::env::var("RUST_TEST_THREADS").as_deref(), Ok("1"));
    let directory = std::path::PathBuf::from(std::env::var_os("T12_BRIDGE_DIR").unwrap());
    let inventory: Value = serde_json::from_str(INVENTORY).unwrap();
    let mut observations = Vec::new();
    let mut selected = None;
    for source in inventory["sources"].as_array().unwrap() {
        if selected.is_some() {
            observations.push(json!({"name":source["name"],"status":"NOT RUN","reason":"First direct media candidate selected; remaining candidates remain unpromoted"}));
            continue;
        }
        if source["probe"] != true {
            observations
                .push(json!({"name":source["name"],"status":"NOT RUN","reason":source["reason"]}));
            continue;
        }
        let observation = candidate(source);
        let prefix = observation["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["stage"] == "direct_media_prefix" && row["status"] == "PREFIX_ONLY");
        observations.push(observation);
        publish(
            &directory.join("public-observations.json"),
            &json!({"sources":observations}),
        );
        if prefix {
            selected = Some(source);
        }
    }
    publish(
        &directory.join("public-observations.json"),
        &json!({"sources":observations}),
    );
    let mut live = None;
    let mut ready = json!({"status":"NO_VERIFIED_MEDIA","sources":observations});
    // A controlled metadata addon supplies only the manual-match identity. Every
    // provider JSON/detail/resolve and all media bytes use public HTTP unchanged.
    let metadata = Fixture::new(|request, _| {
        let meta = json!({"id":"t02-selection","type":"series","name":"Manual public selection","videos":[{"id":"t02-video","title":"Manual episode","season":1,"episode":1}]});
        if request.target.contains("manifest.json") {
            Reply::json(
                json!({"id":"t02-owned-metadata","version":"1.0.0","name":"T02 metadata identity","description":"controlled manual-match identity","types":["series"],"resources":["catalog","meta"],"catalogs":[{"id":"all","type":"series"}]}),
            )
        } else if request.target.contains("/catalog/") {
            Reply::json(json!({"metas":[meta]}))
        } else {
            Reply::json(json!({"meta":meta}))
        }
    });
    if let Some(source) = selected {
        let base = source["api_url"].as_str().unwrap();
        let temporary = TempDir::new().unwrap();
        let host = Host::open(temporary.path().join("settings.json")).unwrap();
        let mut settings = settings::defaults();
        let metadata_config = "22222222-2222-4222-8222-222222222222";
        settings["source_configs"] = json!([
            {"kind":"maccms","config_id":CONFIG,"name":source["name"],"enabled":true,"api_url":base},
            {"kind":"stremio","config_id":metadata_config,"name":"T02 owned metadata","enabled":true,"manifest_urls":[metadata.url("/manifest.json")]}]);
        ok(&host, "save_settings", settings);
        let provider = sync(&host);
        let sources = ok(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":metadata_config}),
        );
        let metadata_source = sources["sources"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["kind"] == "stremio")
            .unwrap();
        ok(&host, "runtime_prepare", json!({"target":"application"}));
        let public = capture(
            &Http::default(),
            &host,
            base,
            "live_detail_page",
            &[("ac", "detail"), ("pg", "1")],
            &mut vec![],
        )
        .unwrap();
        let catalog = catalog(&host);
        let reference = catalog.card(&public["list"][0], &mut vec![]).unwrap()["ref"].clone();
        let details = ok(&host, "get_details", json!({"ref":reference}));
        let groups = details["details"]["groups"].as_array().unwrap();
        let (group, episode) = direct_episode(&public["list"][0], groups)
            .expect("Declared direct media route disappeared");
        let home = ok(&host, "get_home", home_input(metadata_source));
        let meta = ok(
            &host,
            "get_details",
            json!({"ref":home["page"]["sections"][0]["items"][0]["ref"]}),
        );
        let matched = ok(
            &host,
            "match_playback",
            json!({"metadata_ref":meta["metadata_ref"],"video_id":"t02-video",
            "playback_source":{"source_id":provider["id"],"source_revision":provider["source_revision"]},
            "selection":{"content_ref":reference,"group_id":groups[group]["id"],"episode_id":episode["id"]}}),
        );
        let context = &matched["context"];
        let resolution = ok(
            &host,
            "resolve_episode",
            json!({"match_id":context["match_id"],"episode_ref":context["episode_ref"]}),
        );
        let view = ok(
            &host,
            "open_playback",
            json!({"resolution_id":resolution["id"],"stream_id":resolution["streams"][0]["id"]}),
        );
        ready = json!({"status":"READY","source":source["name"],"group_index":group,"sources":observations,
            "view":view,"kind":if view["stream_kind"] == "hls" {"hls"} else {"mp4"},
            "protocol":{"list":true,"details":true,"complete_groups":groups.len(),"episodes":groups.iter().map(|g|g["episodes"].as_array().unwrap().len()).sum::<usize>(),"resolve":true}});
        live = Some((temporary, host, resolution, view));
    }
    publish(&directory.join("ready.json"), &ready);
    let started = Instant::now();
    let finish = directory.join("request-1.json");
    while !finish.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(1200),
            "T02 WebView client timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let request: Value = serde_json::from_slice(&std::fs::read(finish).unwrap()).unwrap();
    assert_eq!(request["operation"], "finish");
    let mut closed = false;
    if let Some((_temporary, host, resolution, view)) = live {
        ok(
            &host,
            "close_playback",
            json!({"session_id":view["session_id"]}),
        );
        ok(
            &host,
            "release_resolution",
            json!({"resolution_id":resolution["id"]}),
        );
        assert_eq!(host.requests.active_count(), 0);
        closed = true;
    }
    publish(&directory.join("response-1.json"), &json!({"ok":true}));
    println!(
        "T02_LIVE:{}",
        json!({"ready":ready["status"] == "READY","closed":closed,"source":ready["source"],"active_requests":0})
    );
}

#[test]
fn t02_capture_projection_removes_signed_urls_and_preserves_group_order() {
    let raw = json!({"code":1,"page":"1","list":[{"vod_id":7,"vod_name":"Title", "vod_pic":"https://image.test/private?secret=one",
        "vod_content":"<a href='https://host.test/path?arbitrary=private'>Plot</a>",
        "vod_play_from":"hls$$$mp4", "vod_play_url":"第10集$https://media.test/a.m3u8?signature=SECRET#第2集$https://media.test/b.m3u8?x=$SECRET$$$https://media.test/c.mp4?unusual=SECRET",
        "unrecognized_sensitive_field":"SECRET"}]});
    let safe = projection(&raw);
    let serialized = safe.to_string();
    for forbidden in [
        "SECRET",
        "media.test",
        "image.test",
        "host.test",
        "private",
        "unrecognized_sensitive_field",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "capture projection leaked a removed field"
        );
    }
    assert_eq!(safe["page"], "1");
    assert_eq!(safe["list"][0]["vod_id"], 7);
    assert_eq!(safe["list"][0]["vod_play_from"], "hls$$$mp4");
    assert_eq!(
        safe["list"][0]["vod_play_url"],
        "第10集$http://maccms.fixture.test:0/t02/0/0.m3u8#第2集$http://maccms.fixture.test:0/t02/0/1.m3u8$$$http://maccms.fixture.test:0/t02/1/0.mp4"
    );
}

#[test]
#[ignore = "explicit public network acquisition through original run.py; preparatory evidence, not A1"]
fn t02_capture_public_json() {
    assert_eq!(
        std::env::var("AGENT_WORKFLOW_RUN").as_deref(),
        Ok("1"),
        "original run.py required"
    );
    assert_eq!(std::env::var("RUST_TEST_THREADS").as_deref(), Ok("1"));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    assert_eq!(
        std::env::current_dir().unwrap(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    );
    let folder = root
        .join("evidence/t02-public")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&folder).unwrap();
    let inventory: Value = serde_json::from_str(INVENTORY).unwrap();
    let mut observations = Vec::new();
    for source in inventory["sources"].as_array().unwrap() {
        let result = if source["probe"] == true {
            candidate(source)
        } else {
            json!({"name":source["name"],"capture_status":"NOT RUN","reason":source["reason"],"supported":false})
        };
        observations.push(result);
        let report = json!({"card":"T0.19","purpose":"public response acquisition; A1 NOT RUN",
            "network":"ordinary local macOS native HTTP; public DNS/peer/TLS validation; no access control workaround",
            "privacy":"projected JSON; all media/image URLs replaced; raw responses/headers stay in native memory",
            "sources":observations,"supported_sources":[],"packaged_webview":"NOT RUN"});
        let temporary = folder.join("capture.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        std::fs::rename(temporary, folder.join("capture.json")).unwrap();
    }
    println!(
        "T02_CAPTURE:{}",
        folder.strip_prefix(root).unwrap().display()
    );
}
