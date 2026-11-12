mod adapters;
mod config;
mod db;
mod line_cache;
mod lookups;
mod mappings;
mod matching;
mod metadata;
mod parsers;
mod reports;
mod sources;
mod state;

use axum::{
    Json, Router,
    extract::{OriginalUri, Path, Query, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use subtle::ConstantTimeEq;

#[derive(Clone)]
struct AppState {
    token: Arc<str>,
    tmdb: Arc<metadata::tmdb::Tmdb>,
    cache: metadata::cache::Cache,
    home: Arc<tokio::sync::RwLock<metadata::home::Home>>,
    sources: metadata::sources::Store,
    playback_sources: sources::Store,
    parsers: parsers::Store,
    mappings: mappings::Store,
    lookup_runtime: state::Runtime,
    reports: reports::Store,
    metadata_gate: Arc<tokio::sync::Mutex<()>>,
}

async fn authorize(State(token): State<Arc<str>>, request: Request, next: Next) -> Response {
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    if provided.is_some_and(|value| bool::from(value.as_bytes().ct_eq(token.as_bytes()))) {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"error":{"code":"UNAUTHORIZED","message":"Invalid or missing bearer token","retryable":false}}))).into_response()
    }
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let home = state.home.read().await;
    let source_count = state.playback_sources.list().ok().map(|v| v.len());
    Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"status":"ok","tmdb":if state.tmdb.is_configured() { "configured" } else { "not_configured" },"snapshot_at":home.built_at,"snapshot_error":home.error,"sources":source_count}),
    )
}

async fn catalog(
    state: &AppState,
    lang: &str,
    kind: &str,
    id: &str,
    extra: &str,
) -> Result<serde_json::Value, &'static str> {
    if metadata::home::is_home(kind, id) {
        let home = state.home.read().await;
        let extras: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(extra.as_bytes())
                .into_owned()
                .collect();
        let first_pages = extras.keys().all(|key| key == "skip")
            && extras.get("skip").is_none_or(|v| v == "0" || v == "20");
        if first_pages {
            let payload = home.payload.as_ref().ok_or("home snapshot not ready")?;
            return metadata::home::find(payload, lang, kind, id, extra)
                .ok_or("home snapshot is incomplete");
        }
    }
    let key = state.cache.key(
        "tmdb",
        "catalog",
        lang,
        &serde_json::json!([kind, id, extra]).to_string(),
    );
    if let Some(value) = state
        .cache
        .get(&key)
        .map_err(|_| "metadata cache read failed")?
    {
        return Ok(value);
    }
    let value = state.tmdb.catalog(lang, kind, id, extra).await?;
    let ttl = if id == "tmdb.search" { 3600 } else { 6 * 3600 };
    state
        .cache
        .put(&key, &value, ttl)
        .map_err(|_| "metadata cache write failed")?;
    Ok(value)
}

async fn meta(
    state: &AppState,
    lang: &str,
    kind: &str,
    id: &str,
) -> Result<serde_json::Value, &'static str> {
    let key = state.cache.key(
        "tmdb",
        "meta",
        lang,
        &serde_json::json!([kind, id]).to_string(),
    );
    if let Some(value) = state
        .cache
        .get(&key)
        .map_err(|_| "metadata cache read failed")?
    {
        return Ok(value);
    }
    let value = state.tmdb.meta(lang, kind, id).await?;
    let airing = kind == "series"
        && value["meta"]["releaseInfo"]
            .as_str()
            .is_some_and(|s| s.ends_with('-'));
    state
        .cache
        .put(&key, &value, if airing { 6 * 3600 } else { 24 * 3600 })
        .map_err(|_| "metadata cache write failed")?;
    Ok(value)
}

async fn stremio(
    State(state): State<AppState>,
    Path((token, source, lang, _resource)): Path<(String, String, String, String)>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let raw = uri.path().splitn(6, '/').nth(5).unwrap_or("");
    let decoded: Result<Vec<String>, _> = raw
        .split('/')
        .map(|segment| {
            percent_encoding::percent_decode_str(segment)
                .decode_utf8()
                .map(|s| s.into_owned())
        })
        .collect();
    let decoded = match decoded {
        Ok(value) => value,
        Err(_) => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Invalid resource encoding",
            );
        }
    };
    let result = if !bool::from(token.as_bytes().ct_eq(state.token.as_bytes())) {
        Err((
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Invalid or missing token",
        ))
    } else if !matches!(lang.as_str(), "zh-CN" | "en-US") {
        Err((
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Unsupported language",
        ))
    } else if source != "tmdb" {
        match state.sources.get(&source) {
            Ok(Some(source)) => metadata::stremio::response(&source, &state.cache, &lang, &decoded)
                .await
                .map_err(|message| {
                    if message.starts_with("unsupported") {
                        (
                            StatusCode::NOT_IMPLEMENTED,
                            "CAPABILITY_UNAVAILABLE",
                            message,
                        )
                    } else if message.starts_with("invalid")
                        || message.starts_with("required")
                        || message.starts_with("unknown")
                    {
                        (StatusCode::BAD_REQUEST, "BAD_REQUEST", message)
                    } else {
                        (StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message)
                    }
                }),
            Ok(None) => Err((
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Metadata source not found",
            )),
            Err(_) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Metadata database failed",
            )),
        }
    } else {
        let parts: Vec<_> = decoded.iter().map(String::as_str).collect();
        let strip = |s: &str| s.strip_suffix(".json").map(str::to_owned);
        let data = match parts.as_slice() {
            ["manifest.json"] => state.tmdb.manifest(&lang).await,
            ["catalog", kind, id] => match strip(id) {
                Some(id) => catalog(&state, &lang, kind, &id, "").await,
                None => Err("invalid catalog path"),
            },
            ["catalog", kind, id, extra] => match strip(extra) {
                Some(extra) => catalog(&state, &lang, kind, id, &extra).await,
                None => Err("invalid catalog path"),
            },
            ["meta", kind, id] => match strip(id) {
                Some(id) => meta(&state, &lang, kind, &id).await,
                None => Err("invalid meta path"),
            },
            _ => Err("unknown TMDB resource"),
        };
        data.map_err(|error| {
            if error == "home snapshot not ready" {
                (StatusCode::SERVICE_UNAVAILABLE, "ENGINE_NOT_READY", error)
            } else if error.starts_with("TMDB API key") {
                (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", error)
            } else if error.starts_with("unknown") || error.starts_with("invalid") {
                (StatusCode::BAD_REQUEST, "BAD_REQUEST", error)
            } else {
                (StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", error)
            }
        })
    };
    let mut response = match result {
        Ok(value) => Json(value).into_response(),
        Err((status, code, message)) => (status, Json(json!({"error":{"code":code,"message":message,"retryable":status == StatusCode::BAD_GATEWAY || status == StatusCode::SERVICE_UNAVAILABLE}}))).into_response(),
    };
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().expect("static header"),
    );
    response
}

fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":message,"retryable":status.is_server_error()}})),
    )
        .into_response()
}

async fn playback_list(State(state): State<AppState>) -> Response {
    match state.playback_sources.list() {
        Ok(mut items) => {
            for item in &mut items {
                if item["kind"] == "stremio_stream" {
                    if let Ok(Some(manifest)) =
                        adapters::stremio_stream::cached_manifest(item, &state.cache)
                    {
                        item["capabilities"] = manifest;
                    }
                }
            }
            Json(json!({"sources":items})).into_response()
        }
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Source database failed",
        ),
    }
}

async fn mapping_list(
    State(state): State<AppState>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let (Some(source), Some(kind), Some(id)) = (
        query.get("metadata_source_id"),
        query.get("type"),
        query.get("meta_id"),
    ) else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Missing metadata identity",
        );
    };
    if source.is_empty() || id.is_empty() || !matches!(kind.as_str(), "movie" | "series") {
        return api_error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Invalid metadata identity",
        );
    }
    match state.mappings.list(source, kind, id) {
        Ok(items) => Json(json!({"mappings":items})).into_response(),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Mapping database failed",
        ),
    }
}

async fn mapping_revoke(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let mut registry = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    match state.mappings.revoke(&id) {
        Ok(true) => {
            registry.cache.clear();
            registry.mapping_generation = registry.mapping_generation.wrapping_add(1);
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => api_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Mapping not found"),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Mapping database failed",
        ),
    }
}

async fn parser_list(State(state): State<AppState>) -> Response {
    match state.parsers.list() {
        Ok(items) => Json(json!({"parsers":items})).into_response(),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Parser database failed",
        ),
    }
}
async fn parser_create(
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>,
) -> Response {
    parser_save(state, input, None)
}
async fn parser_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<serde_json::Value>,
) -> Response {
    parser_save(state, input, Some(id))
}
fn parser_save(state: AppState, input: serde_json::Value, id: Option<String>) -> Response {
    let mut reg = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    match state.parsers.save(&input, id.as_deref()) {
        Ok(item) => {
            reg.parser_generation = reg.parser_generation.wrapping_add(1);
            reg.cache.clear();
            (
                if id.is_some() {
                    StatusCode::OK
                } else {
                    StatusCode::CREATED
                },
                Json(item),
            )
                .into_response()
        }
        Err("parser not found") => {
            api_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Parser not found")
        }
        Err(message) => api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", message),
    }
}
async fn parser_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let mut reg = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    match state.parsers.remove(&id) {
        Ok(true) => {
            reg.parser_generation = reg.parser_generation.wrapping_add(1);
            reg.cache.clear();
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => api_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Parser not found"),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Parser database failed",
        ),
    }
}

async fn playback_create(
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>,
) -> Response {
    let mut registry = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    match state.playback_sources.save(&input, None) {
        Ok(item) => {
            if let Some(id) = item["id"].as_str() {
                registry.changed(id);
            }
            (StatusCode::CREATED, Json(item)).into_response()
        }
        Err(e) => api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", e),
    }
}

async fn playback_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<serde_json::Value>,
) -> Response {
    let mut registry = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    match state.playback_sources.save(&input, Some(&id)) {
        Ok(item) => {
            registry.changed(&id);
            Json(item).into_response()
        }
        Err("source not found") => {
            api_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Source not found")
        }
        Err(e) => api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", e),
    }
}

async fn playback_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let mut registry = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    match state.playback_sources.remove(&id) {
        Ok(true) => {
            registry.changed(&id);
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => api_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Source not found"),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Source database failed",
        ),
    }
}

async fn playback_check(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let source = match state.playback_sources.get(&id) {
        Ok(Some(value)) => value,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Source not found"),
        Err(_) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Source database failed",
            );
        }
    };
    if source["enabled"] != true {
        return api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", "Source is disabled");
    }
    let revision = source["revision"].as_u64().expect("stored source revision");
    if state
        .playback_sources
        .status(&id, revision, "checking")
        .is_err()
    {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Source database failed",
        );
    }
    let start = std::time::Instant::now();
    let result = async {
        if source["kind"] == "stremio_stream" {
            let manifest =
                adapters::stremio_stream::refresh_manifest(&source, &state.cache).await?;
            return Ok::<_, &'static str>((0, 0, Some(manifest)));
        }
        let keyword = query.get("query").map(String::as_str).unwrap_or("电影");
        let items = adapters::search(&source, keyword).await?;
        let sample = items.first().ok_or("source search returned no sample")?;
        let detail = adapters::detail(
            &source,
            sample["vod_id"]
                .as_str()
                .ok_or("source content ID missing")?,
        )
        .await?;
        Ok::<_, &'static str>((
            items.len(),
            detail["groups"].as_array().map_or(0, Vec::len),
            None,
        ))
    }
    .await;
    let status = if result.is_ok() { "ready" } else { "failed" };
    match state.playback_sources.status(&id, revision, status) {
        Ok(true) => {}
        Ok(false) => {
            return api_error(
                StatusCode::CONFLICT,
                "REVISION_MISMATCH",
                "Source changed during check",
            );
        }
        Err(_) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Source database failed",
            );
        }
    }
    match result {Ok((results,groups,capabilities))=>Json(json!({"id":id,"revision":revision,"status":"ready","sample_count":results,"groups":groups,"capabilities":capabilities,"elapsed_ms":start.elapsed().as_millis()})).into_response(),Err(error)=>api_error(StatusCode::BAD_GATEWAY,"UPSTREAM_FAILED",error)}
}

fn import_url(value: &serde_json::Value, base: Option<&url::Url>) -> Option<String> {
    let value = value.as_str()?;
    let parsed = url::Url::parse(value)
        .ok()
        .or_else(|| base.and_then(|base| base.join(value).ok()))?;
    if !metadata::http::valid(&parsed) || parsed.fragment().is_some() {
        return None;
    }
    Some(parsed.to_string())
}

async fn playback_import(
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>,
) -> Response {
    use sha2::{Digest, Sha256};
    let format = input["format"].as_str().unwrap_or("legacy");
    if !matches!(format, "legacy" | "tvbox" | "zyfun" | "zyfun-legacy") {
        return api_error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Unknown import format",
        );
    }
    let base = match input
        .get("base_url")
        .filter(|v| !v.is_null() && v.as_str() != Some(""))
    {
        Some(value) => match value
            .as_str()
            .and_then(|v| url::Url::parse(v).ok())
            .filter(metadata::http::valid)
        {
            Some(value) => Some(value),
            None => return api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", "Invalid base URL"),
        },
        None => None,
    };
    let entries = input["sources"]
        .as_array()
        .or_else(|| input["sites"].as_array())
        .or_else(|| input["site"].as_array());
    let Some(entries) = entries.filter(|items| items.len() <= 256) else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Expected source list",
        );
    };
    let parser_entries = input["parsers"]
        .as_array()
        .or_else(|| input["parses"].as_array())
        .or_else(|| input["analyze"].as_array());
    if parser_entries.is_some_and(|p| p.len() > 256) {
        return api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", "Too many parsers");
    }
    let identity = |item: &serde_json::Value, prefix: &str| -> String {
        let key = item["client_config_id"]
            .as_str()
            .or_else(|| item["config_id"].as_str())
            .or_else(|| item["key"].as_str())
            .or_else(|| item["id"].as_str());
        if format == "legacy" && key.is_some() {
            return if item["client_config_id"].is_string() || item["config_id"].is_string() {
                key.unwrap().to_owned()
            } else {
                format!("import:{}", key.unwrap())
            };
        }
        let value = key.map(str::to_owned).unwrap_or_else(|| {
            format!(
                "{}:{}",
                item["name"],
                item["api"]
                    .as_str()
                    .or_else(|| item["url"].as_str())
                    .unwrap_or("")
            )
        });
        format!(
            "import:{prefix}:{format}:{:x}",
            Sha256::digest(value.as_bytes())
        )
    };
    let mut parser_results = Vec::new();
    let mut imported = std::collections::HashMap::new();
    for item in parser_entries.into_iter().flatten() {
        let label = item["name"].as_str().unwrap_or("");
        let json_type = match format {
            "zyfun" => item["type"] == 2 || item["kind"] == "json",
            "zyfun-legacy" => item["type"] == 1 || item["kind"] == "json",
            "tvbox" => item["type"] == 1 || item["kind"] == "json",
            _ => item["kind"] == "json" || item["type"] == 1,
        };
        if !json_type {
            parser_results
                .push(json!({"name":label,"status":"skipped","reason":"unsupported parser type"}));
            continue;
        }
        let api = item
            .get("api")
            .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
            .or_else(|| item.get("url"))
            .and_then(|v| import_url(v, base.as_ref()));
        let Some(api) = api else {
            parser_results
                .push(json!({"name":label,"status":"failed","error":"invalid parser URL"}));
            continue;
        };
        let url_param = item["url_param"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| {
                url::Url::parse(&api).ok().and_then(|url| {
                    url.query_pairs()
                        .filter(|(_, v)| v.is_empty())
                        .last()
                        .map(|(k, _)| k.into_owned())
                })
            })
            .unwrap_or_else(|| "url".to_owned());
        let candidate = json!({"client_config_id":identity(item,"parser"),"name":item["name"],"api":api,"flags":item.get("flags").or_else(||item.get("flag")).cloned().unwrap_or(json!([])),"headers":item.get("headers").filter(|v| !v.is_null()).cloned().unwrap_or(json!({})),"enabled":item.get("enabled").or_else(||item.get("isActive")).cloned().unwrap_or(json!(true)),"url_param":url_param,"url_field":item.get("url_field").cloned().unwrap_or(json!("url"))});
        let mut registry = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        match state.parsers.save(&candidate, None) {
            Ok(parser) => {
                for key in [
                    item["key"].as_str(),
                    item["id"].as_str(),
                    item["name"].as_str(),
                ]
                .into_iter()
                .flatten()
                {
                    imported.insert(key.to_owned(), parser["id"].clone());
                }
                registry.parser_generation = registry.parser_generation.wrapping_add(1);
                registry.cache.clear();
                parser_results.push(json!({"name":label,"status":"ok","parser":parser}));
            }
            Err(error) => {
                parser_results.push(json!({"name":label,"status":"failed","error":error}))
            }
        }
    }
    let mut results = Vec::new();
    for item in entries {
        let label = item["name"].as_str().unwrap_or("");
        let kind = item["kind"].as_str().or_else(|| item["type"].as_str());
        let numeric = item["type"].as_u64();
        let kind = match (kind, numeric, format) {
            (Some("maccms" | "maccms_json" | "t1_json" | "T1_JSON" | "t1Json"), _, _)
            | (_, Some(1), _) => "maccms_json",
            (Some("maccms_xml" | "t0_xml" | "T0_XML" | "t0Xml"), _, _) | (_, Some(0), _) => {
                "maccms_xml"
            }
            (Some("t4_query" | "t4_drpys" | "T4_DRPYS"), _, _)
            | (_, Some(4), "tvbox" | "legacy")
            | (_, Some(6), "zyfun" | "zyfun-legacy" | "legacy") => "t4_query",
            (Some("t4_drpyjs0" | "T4_DRPYJS0" | "drpyjs0"), _, _) | (_, Some(2), "legacy") => {
                "t4_drpyjs0"
            }
            (Some("t4_rest" | "t4_catvod" | "T4_CATVOD"), _, _)
            | (_, Some(8), "zyfun" | "zyfun-legacy" | "legacy") => "t4_rest",
            (Some("stremio_stream"), _, _) => "stremio_stream",
            _ => {
                results.push(
                    json!({"name":label,"status":"skipped","reason":"unsupported source type"}),
                );
                continue;
            }
        };
        let api = item
            .get("api")
            .or_else(|| item.get("api_url"))
            .and_then(|v| import_url(v, base.as_ref()));
        let Some(api) = api else {
            results.push(json!({"name":label,"status":"failed","error":"invalid source URL"}));
            continue;
        };
        let mut parse_ids = Vec::new();
        let mut missing = false;
        if let Some(ids) = item
            .get("parse_ids")
            .or_else(|| item.get("parsers"))
            .and_then(|v| v.as_array())
        {
            for key in ids {
                if let Some(id) = key.as_str().and_then(|key| imported.get(key)) {
                    parse_ids.push(id.clone());
                } else if format == "legacy" {
                    parse_ids.push(key.clone());
                } else {
                    missing = true;
                }
            }
        }
        if missing {
            results.push(
                json!({"name":label,"status":"failed","error":"associated parser not imported"}),
            );
            continue;
        }
        let ext = match item["ext"].as_str() {
            Some(value) if value.starts_with("./") || value.starts_with("../") => {
                match import_url(&json!(value), base.as_ref()) {
                    Some(value) => json!(value),
                    None => {
                        results.push(
                            json!({"name":label,"status":"failed","error":"invalid extension URL"}),
                        );
                        continue;
                    }
                }
            }
            _ => item["ext"].clone(),
        };
        let candidate = json!({"client_config_id":identity(item,"source"),"name":item["name"],"kind":kind,"api":api,"ext":ext,"headers":item.get("headers").or_else(|| item.get("header")).cloned().unwrap_or(json!({})),"enabled":item.get("enabled").or_else(||item.get("isActive")).cloned().unwrap_or(json!(true)),"parse_ids":parse_ids});
        let mut registry = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        match state.playback_sources.save(&candidate, None) {
            Ok(source) => {
                if let Some(id) = source["id"].as_str() {
                    registry.changed(id);
                }
                results.push(json!({"name":label,"status":"ok","source":source}));
            }
            Err(error) => results.push(json!({"name":label,"status":"failed","error":error})),
        }
    }
    Json(json!({"results":results,"parsers":parser_results})).into_response()
}

async fn list_sources(State(state): State<AppState>) -> Response {
    match state.sources.list() {
        Ok(items) => Json(json!({"sources":items})).into_response(),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Metadata database failed",
        ),
    }
}

async fn import_sources(
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>,
) -> Response {
    let _gate = state.metadata_gate.lock().await;
    let config = match metadata::sources::required(&input, "config_id") {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", e),
    };
    let addons = match input["addons"].as_array() {
        Some(v) if !v.is_empty() && v.len() <= 16 => v,
        _ => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Expected 1 to 16 addons",
            );
        }
    };
    let mut keys = std::collections::HashSet::new();
    let mut orders = std::collections::HashSet::new();
    for item in addons {
        let key = match metadata::sources::required(item, "addon_key") {
            Ok(v) => v,
            Err(e) => return api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", e),
        };
        let order = match item["order"].as_u64().filter(|n| *n < 16) {
            Some(v) => v,
            None => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "BAD_REQUEST",
                    "Invalid addon order",
                );
            }
        };
        if !keys.insert(key) || !orders.insert(order) {
            return api_error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Duplicate addon key or order",
            );
        }
    }
    let mut results = Vec::new();
    for item in addons {
        let key = item["addon_key"].as_str().expect("validated key");
        match state.sources.register(config, item).await {
            Ok(source) => results.push(json!({"addon_key":key,"source":source,"status":"ok"})),
            Err(error) => results.push(json!({"addon_key":key,"status":"failed","error":error})),
        }
    }
    Json(json!({"config_id":config,"results":results})).into_response()
}

async fn update_source(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut item): Json<serde_json::Value>,
) -> Response {
    let _gate = state.metadata_gate.lock().await;
    let existing = match state.sources.get(&id) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return api_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Metadata source not found",
            );
        }
        Err(_) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Metadata database failed",
            );
        }
    };
    let Some(map) = item.as_object_mut() else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Expected a source object",
        );
    };
    map.insert("addon_key".into(), existing["origin_addon_key"].clone());
    let config = existing["origin_config_id"]
        .as_str()
        .expect("stored config");
    match state.sources.register(config, &item).await {
        Ok(source) => Json(source).into_response(),
        Err(e) => api_error(StatusCode::BAD_REQUEST, "BAD_REQUEST", e),
    }
}

async fn delete_source(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let _gate = state.metadata_gate.lock().await;
    match state.sources.delete(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => api_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Metadata source not found",
        ),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Metadata database failed",
        ),
    }
}

async fn stremio_options() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, OPTIONS"),
        ],
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load()?;
    db::initialize(&config.data_dir)?;
    let token = Arc::<str>::from(config.token);
    let tmdb = Arc::new(metadata::tmdb::Tmdb::from_env()?);
    let cache = metadata::cache::Cache::new(&config.data_dir, tmdb.revision());
    let sources = metadata::sources::Store::new(&config.data_dir);
    let playback_sources = sources::Store::new(&config.data_dir);
    let parsers = parsers::Store::new(&config.data_dir);
    let mappings = mappings::Store::new(&config.data_dir);
    let lookup_runtime = state::Runtime::from_env()?;
    let reports = reports::Store::new(&config.data_dir);
    let home = Arc::new(tokio::sync::RwLock::new(metadata::home::initialize(
        &cache,
    )?));
    let zone: chrono_tz::Tz = std::env::var("TZ")
        .unwrap_or_else(|_| "Asia/Shanghai".into())
        .parse()
        .map_err(|_| "invalid TZ")?;
    let poll = std::env::var("MOC_HOME_POLL_SECONDS")
        .unwrap_or_else(|_| "60".into())
        .parse::<u64>()?;
    if poll == 0 {
        return Err("MOC_HOME_POLL_SECONDS must be positive".into());
    }
    metadata::home::start(cache.clone(), tmdb.clone(), home.clone(), zone, poll);
    let app = Router::new()
        .route("/v1/health", get(health))
        .route(
            "/v1/metadata-sources",
            get(list_sources).post(import_sources),
        )
        .route("/v1/metadata-sources/import", post(import_sources))
        .route("/v1/sources", get(playback_list).post(playback_create))
        .route("/v1/parsers", get(parser_list).post(parser_create))
        .route(
            "/v1/parsers/{id}",
            axum::routing::put(parser_update).delete(parser_delete),
        )
        .route("/v1/sources/import", post(playback_import))
        .route("/v1/mappings", get(mapping_list).post(lookups::confirm))
        .route("/v1/mappings/{id}", axum::routing::delete(mapping_revoke))
        .route("/v1/lookups", post(lookups::create))
        .route("/v1/lookups/{id}", axum::routing::delete(lookups::cancel))
        .route("/v1/lines/{id}/refresh", post(lookups::refresh))
        .route("/v1/reports", post(lookups::report))
        .route(
            "/v1/sources/{id}",
            axum::routing::put(playback_update).delete(playback_delete),
        )
        .route("/v1/sources/{id}/check", post(playback_check))
        .route(
            "/v1/metadata-sources/{id}",
            axum::routing::put(update_source).delete(delete_source),
        )
        .route_layer(middleware::from_fn_with_state(token.clone(), authorize))
        .route(
            "/stremio/{token}/{source}/{lang}/{*resource}",
            get(stremio).options(stremio_options),
        )
        .with_state(AppState {
            token,
            tmdb,
            cache,
            home,
            sources,
            playback_sources,
            parsers,
            mappings,
            lookup_runtime,
            reports,
            metadata_gate: Arc::new(tokio::sync::Mutex::new(())),
        });
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    eprintln!("Service listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
