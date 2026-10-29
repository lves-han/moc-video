//! Native candidates own network context. Releasing a table drops only its Arc.
use crate::{
    contracts::{ApiError, Contracts, wire_size},
    requests::Job,
};
use reqwest::header::HeaderMap;
use serde_json::{Value, json};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub type Context = Arc<dyn Any + Send + Sync>;
#[derive(Clone)]
pub struct Link {
    pub original: Value,
    pub provider: String,
    pub url: String,
    pub headers: HeaderMap,
    pub referer: String,
    pub extractor_data: Option<String>,
    pub interceptor: Option<Context>,
    pub cookies: Option<Context>,
    pub label: String,
    pub quality: Option<u32>,
    pub quality_label: Option<String>,
    pub kind: String,
    pub mime_type: Option<String>,
    pub playable: bool,
    pub reason: Option<String>,
}
#[derive(Clone)]
pub struct Subtitle {
    pub original: Value,
    pub url: String,
    pub headers: HeaderMap,
    pub label: String,
    pub language: Option<String>,
    pub format: String,
}
#[derive(Default)]
struct Collected {
    closed: bool,
    links: Vec<Link>,
    subtitles: Vec<Subtitle>,
    bytes: usize,
    error: Option<ApiError>,
}
#[derive(Clone, Default)]
pub struct Callbacks(Arc<Mutex<Collected>>);
impl Callbacks {
    pub fn link(&self, link: Link) -> Result<(), ApiError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| ApiError::internal("Resolution callbacks poisoned"))?;
        if state.closed {
            return Err(ApiError::cancelled());
        }
        let size = serde_json::to_vec(&link.original)
            .map_err(|_| ApiError::internal("Original link serialization failed"))?
            .len()
            .saturating_add(link.url.len())
            .saturating_add(link.referer.len())
            .saturating_add(link.extractor_data.as_ref().map_or(0, String::len))
            .saturating_add(
                link.headers
                    .iter()
                    .map(|(k, v)| k.as_str().len() + v.len())
                    .sum::<usize>(),
            );
        if state.links.len() >= 4096
            || state.bytes.saturating_add(size) > 8 * 1024 * 1024
            || link
                .extractor_data
                .as_ref()
                .is_some_and(|s| s.len() > 524288)
        {
            let error = ApiError::new(
                "RESOURCE_LIMIT",
                "Resolution callbacks exceed their byte or count budget",
            );
            state.error = Some(error.clone());
            return Err(error);
        }
        state.bytes += size;
        state.links.push(link);
        Ok(())
    }
    pub fn subtitle(&self, subtitle: Subtitle) -> Result<(), ApiError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| ApiError::internal("Resolution callbacks poisoned"))?;
        if state.closed {
            return Err(ApiError::cancelled());
        }
        let size = serde_json::to_vec(&subtitle.original)
            .map_err(|_| ApiError::internal("Original subtitle serialization failed"))?
            .len()
            .saturating_add(subtitle.url.len())
            .saturating_add(
                subtitle
                    .headers
                    .iter()
                    .map(|(k, v)| k.as_str().len() + v.len())
                    .sum::<usize>(),
            );
        if state.subtitles.len() >= 4096 || state.bytes.saturating_add(size) > 8 * 1024 * 1024 {
            let error = ApiError::new(
                "RESOURCE_LIMIT",
                "Resolution callbacks exceed their byte or count budget",
            );
            state.error = Some(error.clone());
            return Err(error);
        }
        state.bytes += size;
        state.subtitles.push(subtitle);
        Ok(())
    }
    fn finish(&self) -> Result<(Vec<Link>, Vec<Subtitle>), ApiError> {
        let mut state = self.0.lock().expect("Resolution callbacks");
        state.closed = true;
        let links = std::mem::take(&mut state.links);
        let subtitles = std::mem::take(&mut state.subtitles);
        if let Some(error) = state.error.take() {
            return Err(error);
        }
        Ok((links, subtitles))
    }
}

/// MainAPI boundary. The transport honors the passed deadline/cancellation;
/// callbacks are sealed on every return path, including false and exceptions.
pub fn load_links(
    reference: &Value,
    job: &Job,
    check_current: impl Fn() -> Result<(), ApiError>,
    fetch: impl FnOnce(&str, bool, Callbacks, &Job) -> Result<bool, ApiError>,
) -> Result<(Vec<Link>, Vec<Subtitle>), ApiError> {
    job.check()?;
    check_current()?;
    let data = reference["data"]
        .as_str()
        .ok_or_else(|| ApiError::bad("Missing original episode data"))?;
    let callbacks = Callbacks::default();
    let result = fetch(data, false, callbacks.clone(), job);
    let collected = callbacks.finish();
    job.check()?;
    check_current()?;
    result?;
    let collected = collected?;
    if collected.0.is_empty() {
        return Err(no_links());
    }
    Ok(collected)
}

pub struct LinkRecord {
    pub stream_id: String,
    pub resolution_id: String,
    pub link: Link,
    pub subtitles: Arc<Vec<RegisteredSubtitle>>,
    pub episode_ref: Value,
    pub source_revision: String,
}
pub struct RegisteredSubtitle {
    pub id: String,
    pub subtitle: Subtitle,
}
struct Record {
    value: Value,
    streams: HashMap<String, Arc<LinkRecord>>,
    expires: u64,
}
#[derive(Default)]
pub struct ResolutionStore {
    records: HashMap<String, Record>,
}
impl ResolutionStore {
    pub fn register(
        &mut self,
        reference: &Value,
        links: Vec<Link>,
        subtitles: Vec<Subtitle>,
        now: u64,
    ) -> Result<Value, ApiError> {
        if links.is_empty() {
            return Err(no_links());
        }
        let id = Uuid::new_v4().to_string();
        let expires_at = (OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .format(&Rfc3339)
            .map_err(|_| ApiError::internal("Resolution expiry conversion failed"))?;
        let subtitles: Vec<_> = subtitles
            .into_iter()
            .map(|subtitle| RegisteredSubtitle {
                id: Uuid::new_v4().to_string(),
                subtitle,
            })
            .collect();
        let subtitle_dtos: Vec<_> = subtitles.iter().map(|s| json!({"id":s.id, "label":s.subtitle.label, "language":s.subtitle.language, "format":s.subtitle.format})).collect();
        let subtitles = Arc::new(subtitles);
        let mut streams = HashMap::new();
        let mut dtos = Vec::new();
        for mut link in links {
            if link.provider != reference["source_id"].as_str().unwrap_or("") {
                return Err(ApiError::bad("Link belongs to another provider"));
            }
            let stream_id = Uuid::new_v4().to_string();
            let playable = link.playable
                && crate::host::maccms::http_url(&link.url, false).is_ok()
                && matches!(link.kind.as_str(), "progressive" | "hls" | "dash");
            let reason = if playable {
                None
            } else {
                Some(
                    link.reason
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "unsupported_or_empty_link".into()),
                )
            };
            dtos.push(json!({"id":stream_id, "label":link.label, "quality":link.quality.filter(|q| *q > 0), "quality_label":link.quality_label,
                "kind":link.kind, "has_interceptor":link.interceptor.is_some(), "playable":playable, "reason":reason, "mime_type":link.mime_type}));
            link.playable = playable;
            link.reason = reason;
            streams.insert(
                stream_id.clone(),
                Arc::new(LinkRecord {
                    stream_id,
                    resolution_id: id.clone(),
                    link,
                    subtitles: subtitles.clone(),
                    episode_ref: reference.clone(),
                    source_revision: reference["source_revision"].as_str().unwrap_or("").into(),
                }),
            );
        }
        let value = json!({"id":id, "source_id":reference["source_id"], "source_revision":reference["source_revision"], "expires_at":expires_at,
            "streams":dtos, "subtitles":subtitle_dtos, "episode_ref":reference});
        wire_size(&value, 8 * 1024 * 1024)?;
        Contracts::get().validate("Result_resolve_episode", &json!({"request_id":Uuid::new_v4().to_string(), "engine_epoch":Uuid::new_v4().to_string(), "data":value}))?;
        self.expire(now);
        if self.records.len() >= 256 {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Resolution candidate table is full",
            ));
        }
        self.records.insert(
            id,
            Record {
                value: value.clone(),
                streams,
                expires: now.saturating_add(300_000),
            },
        );
        Ok(value)
    }
    pub fn get(
        &mut self,
        resolution_id: &str,
        stream_id: &str,
        revision: &str,
        now: u64,
    ) -> Result<Arc<LinkRecord>, ApiError> {
        self.expire(now);
        let record = self.records.get(resolution_id).ok_or_else(expired)?;
        if record.value["source_revision"] != revision {
            return Err(expired());
        }
        record.streams.get(stream_id).cloned().ok_or_else(expired)
    }
    pub fn release(&mut self, id: &str) {
        self.records.remove(id);
    }
    pub fn revoke(&mut self, sources: &[String]) {
        self.records
            .retain(|_, record| !sources.iter().any(|id| record.value["source_id"] == *id));
    }
    fn expire(&mut self, now: u64) {
        self.records.retain(|_, r| now < r.expires);
    }
}
fn expired() -> ApiError {
    ApiError::new(
        "SESSION_EXPIRED",
        "Resolution or stream expired; resolve again",
    )
}
fn no_links() -> ApiError {
    ApiError::new("UPSTREAM_FAILED", "Provider returned no links")
        .with_details(json!({"reason":"no_links"}))
}

#[cfg(test)]
#[path = "../../tests/fixtures/t08_resolution.rs"]
mod tests;
