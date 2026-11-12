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
    Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"status":"ok","tmdb":if state.tmdb.is_configured() { "configured" } else { "not_configured" },"snapshot_at":null,"sources":0}),
    )
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
                Some(id) => state.tmdb.catalog(&lang, kind, &id, "").await,
                None => Err("invalid catalog path"),
            },
            ["catalog", kind, id, extra] => match strip(extra) {
                Some(extra) => state.tmdb.catalog(&lang, kind, id, &extra).await,
                None => Err("invalid catalog path"),
            },
            ["meta", kind, id] => match strip(id) {
                Some(id) => state.tmdb.meta(&lang, kind, &id).await,
                None => Err("invalid meta path"),
            },
            _ => Err("unknown TMDB resource"),
        };
        data.map_err(|error| {
            if error.starts_with("TMDB API key") {
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
        Err((status, code, message)) => (status, Json(json!({"error":{"code":code,"message":message,"retryable":status == StatusCode::BAD_GATEWAY}}))).into_response(),
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
    let app = Router::new()
        .route("/v1/health", get(health))
        .route_layer(middleware::from_fn_with_state(token.clone(), authorize))
        .route(
            "/stremio/{token}/tmdb/{lang}/{*resource}",
            get(stremio).options(stremio_options),
        )
        .with_state(AppState {
            token,
            tmdb: Arc::new(metadata::tmdb::Tmdb::from_env()?),
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
