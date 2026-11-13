use super::{cache::Cache, http};
use axum::{extract::{Path, State}, http::{StatusCode, header}, response::{IntoResponse, Response}};
use serde_json::Value;
use subtle::ConstantTimeEq;
use url::Url;

pub fn rewrite(cache: &Cache, title: &str, token: &str, base: Option<&Url>, value: &mut Value) -> rusqlite::Result<()> {
    match value {
        Value::Object(map) => for (key, child) in map.iter_mut() {
            if matches!(key.as_str(), "poster" | "background" | "backdrop" | "logo" | "thumbnail" | "photo" | "still") {
                if let Some(url) = child.as_str().and_then(|s| Url::parse(s).ok().or_else(|| base.and_then(|base| base.join(s).ok()))).filter(http::valid) {
                    let id = cache.register_image(title, url.as_str())?;
                    *child = Value::String(format!("/stremio/{token}/image/{id}"));
                    continue;
                }
            }
            rewrite(cache, title, token, base, child)?;
        },
        Value::Array(items) => for child in items { rewrite(cache, title, token, base, child)?; },
        _ => {}
    }
    Ok(())
}
fn image_mime(body: &[u8]) -> Option<&'static str> {
    if body.starts_with(b"\x89PNG\r\n\x1a\n") { Some("image/png") }
    else if body.starts_with(b"\xff\xd8\xff") { Some("image/jpeg") }
    else if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") { Some("image/gif") }
    else if body.len() >= 12 && &body[..4] == b"RIFF" && &body[8..12] == b"WEBP" { Some("image/webp") }
    else { None }
}
pub async fn image(State(state): State<crate::AppState>, Path((token, key)): Path<(String, String)>) -> Response {
    if !bool::from(token.as_bytes().ct_eq(state.token.as_bytes())) { return StatusCode::UNAUTHORIZED.into_response(); }
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) { return StatusCode::BAD_REQUEST.into_response(); }
    let (url, cached, mime) = match state.cache.image(&key) {
        Ok(Some(row)) => row, Ok(None) => return StatusCode::NOT_FOUND.into_response(), Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let (body, mime) = if let (Some(body), Some(mime)) = (cached, mime) { (body, mime) } else {
        let url = match Url::parse(&url) { Ok(value) if http::valid(&value) => value, _ => return StatusCode::BAD_GATEWAY.into_response() };
        let body = match http::bytes(url, Default::default()).await { Ok(value) => value, Err(_) => return StatusCode::BAD_GATEWAY.into_response() };
        let Some(mime) = image_mime(&body) else { return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response() };
        if state.cache.save_image(&key, &body, mime).is_err() { return StatusCode::INTERNAL_SERVER_ERROR.into_response(); }
        (body, mime.to_owned())
    };
    ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=86400".to_owned())], body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rewrites_known_images_and_stores_bytes_by_title() {
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let cache = Cache::new(dir.path(), "v1".into());
        let mut value = serde_json::json!({"meta":{"poster":"https://example.test/poster.jpg","background":"https://example.test/bg.png","videos":[{"thumbnail":"https://example.test/still.webp"}]}});
        rewrite(&cache, "title", "test-token", None, &mut value).unwrap();
        let path = value["meta"]["poster"].as_str().unwrap();
        assert!(path.starts_with("/stremio/test-token/image/"));
        let id = path.rsplit('/').next().unwrap();
        assert_eq!(cache.image(id).unwrap().unwrap().0, "https://example.test/poster.jpg");
        cache.save_image(id, b"image-data", "image/jpeg").unwrap();
        assert_eq!(cache.image(id).unwrap().unwrap().1.unwrap(), b"image-data");
        assert_eq!(image_mime(b"\xff\xd8\xffrest"), Some("image/jpeg"));
    }
}
