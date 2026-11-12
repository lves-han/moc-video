mod config;
mod db;

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;
use std::sync::Arc;
use subtle::ConstantTimeEq;

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

async fn health() -> Json<serde_json::Value> {
    Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"status":"ok","tmdb":"not_configured","snapshot_at":null,"sources":0}),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load()?;
    db::initialize(&config.data_dir)?;
    let app =
        Router::new()
            .route("/v1/health", get(health))
            .route_layer(middleware::from_fn_with_state(
                Arc::<str>::from(config.token),
                authorize,
            ));
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    eprintln!("Service listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
