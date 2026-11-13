//! Native round state. Playback addresses never enter a WebView response.
use crate::{
    contracts::ApiError,
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
    pub status: String,
    pub latency_ms: Option<u64>,
    pub size_bytes: Option<u64>,
}

/// Waiting budget for one stage of work: the initial lookup, a confirmation or a refresh.
pub const BUDGET: Duration = Duration::from_secs(45);

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
    pub error: Option<String>,
    pub deadline: Instant,
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
    /// New work (a confirmation or a refresh) gets its own budget, so lines that
    /// arrive after the initial lookup budget are still probed.
    pub fn extend(&mut self) {
        self.deadline = Instant::now() + BUDGET;
    }
    fn settle(&mut self) {
        if Instant::now() >= self.deadline {
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
    }
    pub fn snapshot(&mut self) -> Value {
        self.settle();
        let mut lines:Vec<_>=self.lines.iter().map(|(id,line)|json!({"line_id":id,"source_id":line.raw["source_id"],"source_revision":line.raw["source_revision"],"source_name":self.sources.get(line.raw["source_id"].as_str().unwrap_or("")).map(|s|s["name"].clone()).unwrap_or(Value::Null),"line_name":line.raw["line_name"],"episode_label":line.raw["episode_label"],"kind":line.raw["kind"],"quality":line.raw["quality"],"description":line.raw["description"],"size_bytes":line.size_bytes,"reason":line.raw["reason"],"availability_checked_at":line.raw["availability_checked_at"],"availability_expires_at":line.raw["availability_expires_at"],"history":line.raw["history"],"probe_status":line.status,"latency_ms":line.latency_ms})).collect();
        lines.sort_by(|a, b| a["line_id"].as_str().cmp(&b["line_id"].as_str()));
        json!({"round_id":self.id,"lookup_id":self.lookup_id,"query_done":self.query_done,"pending":self.pending,"pending_confirm":self.pending_confirm,"selected_line_id":self.selected,"lines":lines,"sources":self.sources.values().cloned().collect::<Vec<_>>(),"candidates":self.candidates.values().cloned().collect::<Vec<_>>(),"error":self.error})
    }
}

pub(crate) fn header_map(value: &Value) -> Result<HeaderMap, ApiError> {
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
    _permits: &Arc<tokio::sync::Semaphore>,
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
                            | "disabled"
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
            let status = if line["kind"] == "unsupported" {
                "unsupported"
            } else if !matches!(line["kind"].as_str(), Some("file" | "hls")) {
                return Err(ApiError::bad("Unsupported line kind"));
            } else {
                match line["availability"].as_str() {
                    Some("ready") => "ready",
                    Some("failed") => "failed",
                    Some("unsupported") => "unsupported",
                    _ => "unknown",
                }
            };
            state.lines.insert(
                id.into(),
                NativeLine {
                    raw: line.clone(),
                    status: status.into(),
                    latency_ms: line["latency_ms"].as_u64(),
                    size_bytes: line["size_bytes"]
                        .as_u64()
                        .filter(|size| *size > 0 && *size <= 9_007_199_254_740_991),
                },
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::{MonotonicClock, Requests};

    fn round(deadline: Instant) -> Arc<Mutex<Round>> {
        let requests = Arc::new(Requests::default());
        let job = requests
            .begin(
                &uuid::Uuid::new_v4().to_string(),
                "resolve",
                Arc::new(MonotonicClock::default()),
            )
            .unwrap();
        Arc::new(Mutex::new(Round {
            id: "round".into(),
            target: json!({"metadata_source_id":"tmdb","type":"movie","meta_id":"tmdb:1","video_id":"tt1"}),
            metadata_ref: json!({}),
            metadata_source_id: "tmdb".into(),
            metadata_snapshot: crate::source_dispatch::SourceSnapshot {
                host_epoch: "host".into(),
                guest_epoch: None,
                config: json!({}),
                generation: 0,
                sync_sequence: 0,
                source_id: "metadata".into(),
                source_revision: "1".into(),
            },
            lookup_id: Some(uuid::Uuid::new_v4().to_string()),
            sources: HashMap::from([("source-a".into(), json!({"source_id":"source-a"}))]),
            candidates: HashMap::new(),
            lines: HashMap::new(),
            query_done: true,
            pending: 0,
            pending_confirm: 0,
            selected: None,
            error: None,
            deadline,
            stop: Arc::new(AtomicBool::new(false)),
            job: Arc::new(job),
            job_id: uuid::Uuid::new_v4().to_string(),
            service: service::Config::parse("http://127.0.0.1:9/", "token").unwrap(),
            permits: Arc::new(tokio::sync::Semaphore::new(4)),
        }))
    }

    #[test]
    fn server_availability_is_shown_without_client_probe() {
        let round = round(Instant::now() + Duration::from_secs(5));
        let id = uuid::Uuid::new_v4().to_string();
        let line = json!({"line_id":id,"source_id":"source-a","source_revision":1,"kind":"file", "availability":"ready", "size_bytes":123456, "latency_ms":7,
            "expires_at":time::OffsetDateTime::now_utc().unix_timestamp()+60,
            "target":round.lock().unwrap().target.clone()});
        let permits = round.lock().unwrap().permits.clone();
        event(&round, json!({"event":"line","line":line}), &permits, false).unwrap();
        let snapshot = round.lock().unwrap().snapshot();
        assert_eq!(snapshot["lines"][0]["probe_status"], "ready");
        assert_eq!(snapshot["lines"][0]["size_bytes"], 123456);
        assert_eq!(snapshot["lines"][0]["latency_ms"], 7);
        assert!(snapshot["selected_line_id"].is_null());
    }
}
