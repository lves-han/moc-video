//! Native round state. Playback addresses never enter a WebView response.
use crate::{
    contracts::ApiError,
    host::maccms::{self, Http},
    requests::{Job, Requests},
    service,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct NativeLine {
    pub raw: Value,
    pub headers: HeaderMap,
    pub url: String,
    pub status: String,
    pub latency_ms: Option<u64>,
}
pub struct Round {
    pub id: String,
    pub target: Value,
    pub metadata_ref: Value,
    pub metadata_source_id: String,
    pub metadata_snapshot: crate::source_dispatch::SourceSnapshot,
    pub lookup_id: Option<String>,
    pub sources: HashMap<String, Value>,
    pub candidates: HashMap<String, Value>,
    pub lines: HashMap<String, NativeLine>,
    pub query_done: bool,
    pub pending: usize,
    pub pending_confirm: usize,
    pub selected: Option<String>,
    pub manual: bool,
    pub error: Option<String>,
    pub started: Instant,
    pub stop: Arc<AtomicBool>,
    pub job: Arc<Job>,
    pub job_id: String,
    pub service: service::Config,
    pub permits: Arc<tokio::sync::Semaphore>,
}
#[derive(Default)]
pub struct Rounds {
    pub active: Option<String>,
    pub records: HashMap<String, Arc<Mutex<Round>>>,
}

impl Rounds {
    pub fn get(&self, id: &str) -> Result<Arc<Mutex<Round>>, ApiError> {
        if self.active.as_deref() != Some(id) {
            return Err(ApiError::new(
                "SESSION_EXPIRED",
                "Line round is no longer current",
            ));
        }
        self.records
            .get(id)
            .cloned()
            .ok_or_else(|| ApiError::new("SESSION_EXPIRED", "Line round expired"))
    }
}

pub fn stop(round: &Arc<Mutex<Round>>, requests: &Arc<Requests>) {
    let (job_id, lookup, service) = {
        let state = round.lock().expect("round state");
        state.stop.store(true, Ordering::SeqCst);
        (
            state.job_id.clone(),
            state.lookup_id.clone(),
            state.service.clone(),
        )
    };
    let _ = requests.cancel(&uuid::Uuid::new_v4().to_string(), &job_id);
    if let Some(lookup) = lookup {
        std::thread::spawn(move || {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                let _ = runtime.block_on(service.cancel_lookup(&lookup));
            }
        });
    }
}

pub async fn drive(round: Arc<Mutex<Round>>, body: Value, confirmation: bool) {
    let confirmed_candidate = body["candidate_id"].as_str().map(str::to_owned);
    let (service, stop, permits) = {
        let round = round.lock().expect("round state");
        (
            round.service.clone(),
            round.stop.clone(),
            round.permits.clone(),
        )
    };
    let path: &[&str] = if confirmation {
        &["v1", "mappings"]
    } else {
        &["v1", "lookups"]
    };
    let result = service
        .events(
            path,
            body,
            |value| event(&round, value, &permits, confirmation),
            stop.clone(),
        )
        .await;
    if stop.load(Ordering::SeqCst) {
        let lookup = { round.lock().expect("round state").lookup_id.clone() };
        if let Some(id) = lookup {
            let _ = service.cancel_lookup(&id).await;
        }
    }
    {
        let mut state = round.lock().expect("round state");
        if confirmation {
            state.pending_confirm = state.pending_confirm.saturating_sub(1);
            if result.is_ok() {
                if let Some(id) = confirmed_candidate {
                    state.candidates.remove(&id);
                }
            }
        } else {
            state.query_done = true;
        }
        if let Err(error) = result {
            if error.code != "CANCELLED" {
                state.error = Some(error.message);
            }
        }
        state.settle();
    }
    loop {
        let done = {
            let mut state = round.lock().expect("round state");
            state.settle();
            state.pending == 0 || stop.load(Ordering::SeqCst)
        };
        if done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

pub fn launch(round: Arc<Mutex<Round>>, body: Value, confirmation: bool) -> Result<(), ApiError> {
    use std::sync::atomic::AtomicUsize;
    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    let count = ACTIVE.fetch_add(1, Ordering::SeqCst);
    if count >= 16 {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        return Err(ApiError::new("RESOURCE_LIMIT", "Too many line requests"));
    }
    std::thread::spawn(move || {
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime.block_on(drive(round, body, confirmation)),
            Err(_) => {
                let mut state = round.lock().expect("round state");
                state.error = Some("Line runtime unavailable".into());
                if confirmation {
                    state.pending_confirm = state.pending_confirm.saturating_sub(1);
                } else {
                    state.query_done = true;
                }
            }
        }
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    });
    Ok(())
}

impl Round {
    fn settle(&mut self) {
        if self.started.elapsed() >= Duration::from_secs(45) {
            for line in self
                .lines
                .values_mut()
                .filter(|line| line.status == "testing")
            {
                line.status = "timeout".into();
            }
            self.pending = 0;
            self.pending_confirm = 0;
            self.query_done = true;
        }
        if !self.manual && self.query_done && self.pending == 0 && self.pending_confirm == 0 {
            self.selected = self
                .lines
                .iter()
                .filter(|(_, line)| line.status == "ready")
                .min_by_key(|(_, line)| line.latency_ms.unwrap_or(u64::MAX))
                .map(|(id, _)| id.clone());
        }
    }
    pub fn snapshot(&mut self) -> Value {
        self.settle();
        let mut lines:Vec<_>=self.lines.iter().map(|(id,line)|json!({"line_id":id,"source_id":line.raw["source_id"],"source_revision":line.raw["source_revision"],"source_name":self.sources.get(line.raw["source_id"].as_str().unwrap_or("")).map(|s|s["name"].clone()).unwrap_or(Value::Null),"line_name":line.raw["line_name"],"episode_label":line.raw["episode_label"],"kind":line.raw["kind"],"quality":line.raw["quality"],"history":line.raw["history"],"probe_status":line.status,"latency_ms":line.latency_ms})).collect();
        lines.sort_by(|a, b| a["line_id"].as_str().cmp(&b["line_id"].as_str()));
        json!({"round_id":self.id,"lookup_id":self.lookup_id,"query_done":self.query_done,"pending":self.pending,"pending_confirm":self.pending_confirm,"selected_line_id":self.selected,"lines":lines,"sources":self.sources.values().cloned().collect::<Vec<_>>(),"candidates":self.candidates.values().cloned().collect::<Vec<_>>(),"error":self.error})
    }
}

fn header_map(value: &Value) -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    for (name, value) in value
        .as_object()
        .ok_or_else(|| ApiError::bad("Invalid line headers"))?
    {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| ApiError::bad("Invalid line headers"))?;
        if matches!(
            name.as_str(),
            "host" | "content-length" | "connection" | "transfer-encoding" | "range"
        ) {
            return Err(ApiError::bad("Unsafe line header"));
        }
        let value = HeaderValue::from_str(
            value
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid line headers"))?,
        )
        .map_err(|_| ApiError::bad("Invalid line headers"))?;
        headers.insert(name, value);
    }
    Ok(headers)
}

pub fn event(
    round: &Arc<Mutex<Round>>,
    value: Value,
    permits: &Arc<tokio::sync::Semaphore>,
    confirmation: bool,
) -> Result<(), ApiError> {
    let event = value["event"]
        .as_str()
        .ok_or_else(|| ApiError::bad("Invalid service event"))?;
    let mut state = round
        .lock()
        .map_err(|_| ApiError::internal("Round state unavailable"))?;
    if state.stop.load(Ordering::SeqCst) && event != "started" {
        return Err(ApiError::cancelled());
    }
    match event {
        "started" => {
            if state.lookup_id.is_some() {
                return Err(ApiError::bad("Duplicate lookup start"));
            }
            let id = value["lookup_id"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid lookup ID"))?;
            uuid::Uuid::parse_str(id).map_err(|_| ApiError::bad("Invalid lookup ID"))?;
            state.lookup_id = Some(id.to_owned());
            for source in value["sources"].as_array().into_iter().flatten() {
                if let Some(id) = source["source_id"].as_str() {
                    state.sources.insert(id.into(),json!({"source_id":id,"name":source["name"],"kind":source["kind"],"status":"pending"}));
                }
            }
            if state.stop.load(Ordering::SeqCst) {
                return Err(ApiError::cancelled());
            }
        }
        "source" => {
            let id = value["source_id"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid source event"))?;
            if !state.sources.contains_key(id) {
                return Err(ApiError::bad("Unknown round source"));
            }
            let status = value["status"]
                .as_str()
                .filter(|s| {
                    matches!(
                        *s,
                        "searching"
                            | "done"
                            | "needs_confirmation"
                            | "not_found"
                            | "no_episode"
                            | "unsupported"
                            | "failed"
                            | "timeout"
                    )
                })
                .ok_or_else(|| ApiError::bad("Invalid source status"))?;
            let entry = state
                .sources
                .entry(id.into())
                .or_insert_with(|| json!({"source_id":id,"name":null,"kind":null}));
            entry["status"] = json!(status);
        }
        "confirm" => {
            let source = value["source_id"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid candidate source"))?;
            if !state.sources.contains_key(source) {
                return Err(ApiError::bad("Unknown round source"));
            }
            let rows = value["candidates"]
                .as_array()
                .filter(|rows| rows.len() <= 256)
                .ok_or_else(|| ApiError::bad("Invalid candidates"))?;
            for row in rows {
                let id = row["candidate_id"]
                    .as_str()
                    .ok_or_else(|| ApiError::bad("Invalid candidate"))?;
                uuid::Uuid::parse_str(id).map_err(|_| ApiError::bad("Invalid candidate ID"))?;
                if state.candidates.len() >= 256 {
                    return Err(ApiError::new("RESOURCE_LIMIT", "Candidate limit reached"));
                }
                state.candidates.insert(id.into(),json!({"candidate_id":id,"source_id":source,"title":row["title"],"year":row["year"],"kind":row["kind"],"mismatch":row["mismatch"]}));
            }
        }
        "line" => {
            let line = &value["line"];
            let id = line["line_id"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid line ID"))?;
            uuid::Uuid::parse_str(id).map_err(|_| ApiError::bad("Invalid line ID"))?;
            if line["target"]["metadata_source_id"] != state.target["metadata_source_id"]
                || line["target"]["type"] != state.target["type"]
                || line["target"]["meta_id"] != state.target["meta_id"]
                || line["target"]["video_id"] != state.target["video_id"]
            {
                return Err(ApiError::bad("Line target differs from native round"));
            }
            if line["source_id"].as_str().is_none_or(str::is_empty)
                || line["source_revision"].as_u64().is_none()
                || line["expires_at"].as_i64().is_none()
            {
                return Err(ApiError::bad("Invalid line source"));
            }
            if !state
                .sources
                .contains_key(line["source_id"].as_str().unwrap_or(""))
            {
                return Err(ApiError::bad("Unknown round source"));
            }
            if state.lines.len() >= 1024 {
                return Err(ApiError::new("RESOURCE_LIMIT", "Line limit reached"));
            }
            if state.lines.contains_key(id) {
                return Err(ApiError::bad("Duplicate line ID"));
            }
            if line["kind"] == "unsupported" {
                state.lines.insert(
                    id.into(),
                    NativeLine {
                        raw: line.clone(),
                        headers: HeaderMap::new(),
                        url: String::new(),
                        status: "unsupported".into(),
                        latency_ms: None,
                    },
                );
            } else {
                let url = line["play"]["url"]
                    .as_str()
                    .ok_or_else(|| ApiError::bad("Line playback URL missing"))?
                    .to_owned();
                maccms::http_url(&url, false)?;
                let headers = header_map(&line["play"]["headers"])?;
                if !matches!(line["kind"].as_str(), Some("file" | "hls")) {
                    return Err(ApiError::bad("Unsupported line kind"));
                }
                let native = NativeLine {
                    raw: line.clone(),
                    headers,
                    url,
                    status: "testing".into(),
                    latency_ms: None,
                };
                state.lines.insert(id.into(), native.clone());
                state.pending += 1;
                let round = round.clone();
                let id = id.to_owned();
                let permits = permits.clone();
                let parent = state.job.clone();
                tokio::spawn(async move {
                    let Ok(_permit) = permits.acquire().await else {
                        return;
                    };
                    let child = parent.limited_child(
                        &uuid::Uuid::new_v4().to_string(),
                        "default_metadata",
                        5000,
                    );
                    let measured = match child {
                        Ok(job) => tokio::task::spawn_blocking(move || probe(&native, &job))
                            .await
                            .unwrap_or_else(|_| Err(ApiError::internal("Line probe failed"))),
                        Err(error) => Err(error),
                    };
                    let mut round = round.lock().expect("round state");
                    if round.stop.load(Ordering::SeqCst)
                        || round.started.elapsed() >= Duration::from_secs(45)
                    {
                        return;
                    }
                    if let Some(line) = round.lines.get_mut(&id) {
                        match measured {
                            Ok(ms) => {
                                line.status = "ready".into();
                                line.latency_ms = Some(ms);
                            }
                            Err(error) => {
                                line.status = if error.code == "UPSTREAM_TIMEOUT" {
                                    "timeout"
                                } else {
                                    "failed"
                                }
                                .into();
                            }
                        }
                    }
                    round.pending = round.pending.saturating_sub(1);
                    round.settle();
                });
            }
        }
        "done" => {
            if !confirmation {
                state.query_done = true;
            }
            state.settle();
        }
        _ => return Err(ApiError::bad("Unknown service event")),
    }
    Ok(())
}

fn probe(line: &NativeLine, job: &Job) -> Result<u64, ApiError> {
    let start = Instant::now();
    let http = Http::default();
    let mut url = maccms::http_url(&line.url, false)?;
    let mut headers = line.headers.clone();
    if line.raw["kind"] == "hls" {
        for _ in 0..2 {
            let manifest = http.get(
                url.clone(),
                headers.clone(),
                job,
                crate::gateway::hls::LIMIT,
                false,
                false,
            )?;
            let text = std::str::from_utf8(&manifest.body)
                .map_err(|_| ApiError::new("UPSTREAM_FAILED", "Invalid HLS playlist"))?;
            let refs = crate::gateway::hls::references(text)?;
            let selected = refs
                .iter()
                .find(|r| r.playlist)
                .or_else(|| refs.iter().find(|r| r.cacheable))
                .ok_or_else(|| ApiError::new("UPSTREAM_FAILED", "HLS playlist has no media"))?;
            let next = manifest
                .url
                .join(&text[selected.span.clone()])
                .map_err(|_| ApiError::new("UPSTREAM_FAILED", "Invalid HLS child"))?;
            maccms::redirect_headers(&manifest.url, &next, &mut headers)?;
            url = next;
            if !selected.playlist {
                let body = http.get(url, headers, job, 65536, true, false)?;
                if body.body.is_empty() {
                    return Err(ApiError::new("UPSTREAM_FAILED", "Empty media probe"));
                }
                return Ok(start.elapsed().as_millis() as u64);
            }
        }
        return Err(ApiError::new(
            "UPSTREAM_FAILED",
            "HLS nesting limit reached",
        ));
    }
    let body = http.get(url, headers, job, 65536, true, false)?;
    if body.body.is_empty() {
        return Err(ApiError::new("UPSTREAM_FAILED", "Empty media probe"));
    }
    Ok(start.elapsed().as_millis() as u64)
}
