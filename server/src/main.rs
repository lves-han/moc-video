mod config;
mod db;
mod metadata;

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
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
    Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"status":"ok","tmdb":if state.tmdb.is_configured() { "configured" } else { "not_configured" },"snapshot_at":home.built_at,"snapshot_error":home.error,"sources":0}),
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
    Path((token, lang, resource)): Path<(String, String, String)>,
) -> Response {
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
    } else {
        let parts: Vec<_> = resource.split('/').collect();
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
        .route_layer(middleware::from_fn_with_state(token.clone(), authorize))
        .route(
            "/stremio/{token}/tmdb/{lang}/{*resource}",
            get(stremio).options(stremio_options),
        )
        .with_state(AppState {
            token,
            tmdb,
            cache,
            home,
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
