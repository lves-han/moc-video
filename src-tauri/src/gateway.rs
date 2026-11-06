//! Loopback resource gateway. Native records own upstream context; capabilities
//! authorize registered resources, never browser-supplied upstream URLs.
use crate::{
    contracts::ApiError,
    host::maccms::{Http, Image, http_url, redirect_headers},
    media_cache::{self, MediaCache},
    requests::{Clock, MonotonicClock},
    resolution::{HttpContext, HttpManifestContext, LinkRecord},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Method, header::HeaderMap};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    future::Future,
    io::{Cursor, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use uuid::Uuid;

#[path = "hls.rs"]
mod hls;
#[path = "subtitle.rs"]
mod subtitle;

const SESSION_IDLE_MS: u64 = 300_000;
const IMAGE_IDLE_MS: u64 = 1_800_000;
pub const STREAM_BUFFER: usize = 64 * 1024;
pub const IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const IMAGE_PIXELS: u64 = 16_777_216;
const IMAGE_COUNT: usize = 4096;
const CONNECTIONS: usize = 64;
const RESOURCE_REQUESTS: usize = 16;

fn expired() -> ApiError {
    ApiError::new(
        "SESSION_EXPIRED",
        "Resource lease expired; obtain a new handle",
    )
}
fn upstream(message: &str) -> ApiError {
    ApiError::new("UPSTREAM_FAILED", message)
}
fn capability() -> Result<String, ApiError> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| ApiError::internal("Capability generation failed"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn expires_after(ms: u64) -> Result<String, ApiError> {
    (OffsetDateTime::now_utc() + time::Duration::milliseconds(ms as i64))
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("Resource expiry formatting failed"))
}
#[derive(Default)]
struct Diagnostic {
    completed: u64,
    failure: Option<(u64, ApiError)>,
}
struct Resource {
    url: String,
    headers: HeaderMap,
    kind: &'static str,
    diagnostic: Diagnostic,
}
struct Session {
    selected: Arc<LinkRecord>,
    cap: String,
    resources: HashMap<String, Resource>,
    cancelled: Arc<AtomicBool>,
    active: usize,
    last_activity: u64,
}
impl Session {
    fn live(&self, now: u64) -> bool {
        !self.cancelled.load(Ordering::SeqCst)
            && (self.active > 0 || now.saturating_sub(self.last_activity) < SESSION_IDLE_MS)
    }
}
struct RegisteredImage {
    image: Image,
    source: String,
    cancelled: Arc<AtomicBool>,
    expires: u64,
    used: u64,
    diagnostic: Diagnostic,
}
#[derive(Default)]
struct Records {
    sessions: HashMap<String, Session>,
    images: HashMap<String, RegisteredImage>,
    sequence: u64,
}
struct Shared {
    address: SocketAddr,
    records: Mutex<Records>,
    clock: Arc<dyn Clock>,
    token: String,
    stop: AtomicBool,
    idle_ms: AtomicU64,
    http: Http,
    media_cache: Arc<MediaCache>,
}
pub struct Gateway {
    pub address: SocketAddr,
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Gateway {
    pub fn start() -> Result<Self, ApiError> {
        Self::with_clock(Arc::new(MonotonicClock::default()))
    }
    pub fn with_clock(clock: Arc<dyn Clock>) -> Result<Self, ApiError> {
        Self::with_http(clock, Http::default())
    }
    pub fn with_media_cache(
        clock: Arc<dyn Clock>,
        cache: Arc<MediaCache>,
    ) -> Result<Self, ApiError> {
        Self::with_cache(clock, Http::default(), cache)
    }
    fn with_http(clock: Arc<dyn Clock>, http: Http) -> Result<Self, ApiError> {
        Self::with_cache(clock, http, MediaCache::disabled())
    }
    fn with_cache(
        clock: Arc<dyn Clock>,
        http: Http,
        cache: Arc<MediaCache>,
    ) -> Result<Self, ApiError> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| ApiError::internal("Loopback gateway bind failed"))?;
        listener
            .set_nonblocking(true)
            .map_err(|_| ApiError::internal("Loopback gateway setup failed"))?;
        let address = listener
            .local_addr()
            .map_err(|_| ApiError::internal("Loopback address unavailable"))?;
        let shared = Arc::new(Shared {
            address,
            records: Mutex::new(Records::default()),
            clock,
            token: capability()?,
            stop: AtomicBool::new(false),
            idle_ms: AtomicU64::new(30_000),
            http,
            media_cache: cache,
        });
        let running = shared.clone();
        let worker = thread::Builder::new().name("host-gateway".into()).spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return; };
            runtime.block_on(async move {
                let Ok(listener) = tokio::net::TcpListener::from_std(listener) else { return; };
                let slots = Arc::new(tokio::sync::Semaphore::new(RESOURCE_REQUESTS));
                let mut calls = tokio::task::JoinSet::new();
                let mut maintenance = tokio::time::interval(Duration::from_millis(10));
                while !running.stop.load(Ordering::SeqCst) {
                    tokio::select! {
                        accepted = listener.accept(), if calls.len() < CONNECTIONS => {
                            match accepted {
                                Ok((stream, peer)) if peer.ip().is_loopback() => { calls.spawn(serve(stream, running.clone(), slots.clone())); }
                                Ok(_) => {}, Err(_) => break,
                            }
                        }
                        _ = calls.join_next(), if !calls.is_empty() => {},
                        _ = maintenance.tick() => {
                            running.records.lock().expect("Resource registry").sweep(running.clock.now_ms());
                        },
                    }
                }
                // Aborting closes both I/O directions; lease Drop releases pins.
                calls.abort_all();
                while calls.join_next().await.is_some() {}
            });
        }).map_err(|_| ApiError::internal("Loopback gateway worker failed"))?;
        let gateway = Self {
            address,
            shared,
            worker: Some(worker),
        };
        gateway.health()?;
        Ok(gateway)
    }
    pub fn health(&self) -> Result<Value, ApiError> {
        let error = || ApiError::internal("Loopback gateway health check failed");
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(1))
            .map_err(|_| error())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|_| error())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(|_| error())?;
        write!(stream, "GET /host-health HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", self.shared.token).map_err(|_| error())?;
        let mut response = String::new();
        stream
            .take(8193)
            .read_to_string(&mut response)
            .map_err(|_| error())?;
        if response.len() > 8192 || !response.starts_with("HTTP/1.1 200 ") {
            return Err(error());
        }
        let (_, body) = response.split_once("\r\n\r\n").ok_or_else(error)?;
        let health: Value = serde_json::from_str(body).map_err(|_| error())?;
        if health != host_health() {
            return Err(error());
        }
        Ok(health)
    }
    pub fn open(&self, selected: Arc<LinkRecord>) -> Result<Value, ApiError> {
        if !selected.link.playable {
            return Err(ApiError::new(
                "MEDIA_UNSUPPORTED",
                "Selected stream is not playable",
            ));
        }
        let mut records = self
            .shared
            .records
            .lock()
            .map_err(|_| ApiError::internal("Resource registry poisoned"))?;
        let now = self.shared.clock.now_ms();
        records.sweep(now);
        if records.sessions.len() >= 256 {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Playback session table is full",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let cap = capability()?;
        let resource = selected.stream_id.clone();
        let path = format!("/media/{id}/{resource}");
        let subtitles: Vec<_> = selected.subtitles.iter().map(|s| json!({
            "id":s.id,"label":s.subtitle.label,"language":s.subtitle.language,"format":"vtt",
            "source_format":s.subtitle.format,"url":self.url(&format!("/media/{id}/{}", s.id), &cap)
        })).collect();
        let value = json!({"session_id":id,"resolution_id":selected.resolution_id,"stream_id":selected.stream_id,
            "source_id":selected.link.provider,"source_revision":selected.source_revision,"episode_ref":selected.episode_ref,
            "stream_kind":selected.link.kind,"mime_type":selected.link.mime_type,"media_url":self.url(&path,&cap),"subtitles":subtitles,"expires_at":expires_after(SESSION_IDLE_MS)?});
        let mut resources = HashMap::from([(
            resource,
            Resource {
                url: selected.link.url.clone(),
                headers: selected.link.headers.clone(),
                kind: if selected.link.kind == "hls" {
                    "manifest"
                } else {
                    "media"
                },
                diagnostic: Diagnostic::default(),
            },
        )]);
        for subtitle in selected.subtitles.iter() {
            resources.insert(
                subtitle.id.clone(),
                Resource {
                    url: subtitle.subtitle.url.clone(),
                    headers: subtitle.subtitle.headers.clone(),
                    kind: "subtitle",
                    diagnostic: Diagnostic::default(),
                },
            );
        }
        records.sessions.insert(
            id,
            Session {
                resources,
                selected,
                cap,
                cancelled: Arc::new(AtomicBool::new(false)),
                active: 0,
                last_activity: now,
            },
        );
        Ok(value)
    }
    pub fn close(&self, id: &str) {
        if let Some(s) = self
            .shared
            .records
            .lock()
            .expect("Resource registry")
            .sessions
            .remove(id)
        {
            s.cancelled.store(true, Ordering::SeqCst);
        }
    }
    pub fn revoke(&self, sources: &[String]) {
        let mut records = self.shared.records.lock().expect("Resource registry");
        records.sessions.retain(|_, s| {
            let keep = !sources.contains(&s.selected.link.provider);
            if !keep {
                s.cancelled.store(true, Ordering::SeqCst);
            }
            keep
        });
        records.images.retain(|_, i| {
            let keep = !sources.contains(&i.source);
            if !keep {
                i.cancelled.store(true, Ordering::SeqCst);
            }
            keep
        });
    }
    /// Only outgoing DTO references register/renew images. Credentials and request
    /// fields originate in the adapter, never in a resource HTTP request.
    pub fn image(&self, source: &str, image: Image) -> Result<Value, ApiError> {
        let id = image.reference["resource_id"]
            .as_str()
            .ok_or_else(|| ApiError::internal("Missing image ID"))?
            .to_owned();
        let cap = image.reference["capability"]
            .as_str()
            .ok_or_else(|| ApiError::internal("Missing image capability"))?;
        let mut canonical_cap = cap.to_owned();
        if image.reference["path"] != format!("/images/{id}")
            || !URL_SAFE_NO_PAD.decode(cap).is_ok_and(|b| b.len() == 32)
        {
            return Err(ApiError::internal("Invalid native image reference"));
        }
        http_url(&image.url, false)?;
        let mut records = self
            .shared
            .records
            .lock()
            .map_err(|_| ApiError::internal("Resource registry poisoned"))?;
        let now = self.shared.clock.now_ms();
        records.sweep(now);
        records.sequence += 1;
        let sequence = records.sequence;
        if let Some(old) = records.images.get_mut(&id) {
            if old.source != source
                || old.image.url != image.url
                || old.image.headers != image.headers
            {
                return Err(ApiError::internal(
                    "Image ID was rebound to a different resource",
                ));
            }
            old.expires = now.saturating_add(IMAGE_IDLE_MS);
            old.used = sequence;
            canonical_cap = old.image.reference["capability"]
                .as_str()
                .expect("Image cap")
                .to_owned();
        } else {
            if records.images.len() >= IMAGE_COUNT {
                let oldest = records
                    .images
                    .iter()
                    .min_by_key(|(_, i)| i.used)
                    .map(|(id, _)| id.clone())
                    .expect("Full image table");
                if let Some(old) = records.images.remove(&oldest) {
                    old.cancelled.store(true, Ordering::SeqCst);
                }
            }
            records.images.insert(
                id.clone(),
                RegisteredImage {
                    image: image.clone(),
                    source: source.into(),
                    cancelled: Arc::new(AtomicBool::new(false)),
                    expires: now.saturating_add(IMAGE_IDLE_MS),
                    used: sequence,
                    diagnostic: Diagnostic::default(),
                },
            );
        }
        Ok(
            json!({"resource_id":id,"url":self.url(&format!("/images/{id}"),&canonical_cap),"expires_at":expires_after(IMAGE_IDLE_MS)?}),
        )
    }
    fn url(&self, path: &str, cap: &str) -> String {
        format!("http://{}{path}?cap={cap}", self.address)
    }
    /// Pure lookup: no upstream, sweep, activity update or lease renewal.
    pub fn status(&self, kind: &str, id: &str) -> Result<Value, ApiError> {
        let records = self
            .shared
            .records
            .lock()
            .map_err(|_| ApiError::internal("Resource registry poisoned"))?;
        let now = self.shared.clock.now_ms();
        let failure = match kind {
            "playback" => records.sessions.get(id).filter(|s| s.live(now)).map(|s| {
                s.resources
                    .values()
                    .filter_map(|r| r.diagnostic.failure.as_ref())
                    .max_by_key(|(seq, _)| *seq)
                    .map(|(_, e)| e.clone())
            }),
            "image" => records
                .images
                .get(id)
                .filter(|i| now < i.expires)
                .map(|i| i.diagnostic.failure.as_ref().map(|(_, e)| e.clone())),
            _ => return Err(ApiError::bad("Unknown resource kind")),
        };
        let (state, error) = match failure {
            None => ("expired", Some(expired())),
            Some(Some(e)) => ("failed", Some(e)),
            Some(None) => ("ready", None),
        };
        Ok(json!({"kind":kind,"target_id":id,"state":state,"error":error}))
    }
}
impl Records {
    fn sweep(&mut self, now: u64) {
        self.sessions.retain(|_, s| {
            let live = s.live(now);
            if !live {
                s.cancelled.store(true, Ordering::SeqCst);
            }
            live
        });
        self.images.retain(|_, i| {
            let live = now < i.expires;
            if !live {
                i.cancelled.store(true, Ordering::SeqCst);
            }
            live
        });
    }
}
impl Drop for Gateway {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
fn host_health() -> Value {
    json!({"status":"ready","contract_version":"1.5.0","media_gateway_ready":true})
}

struct Request {
    method: Method,
    path: String,
    cap: Option<String>,
    headers: HeaderMap,
    origin: Option<String>,
}
async fn request(read: &mut OwnedReadHalf) -> Result<Request, ApiError> {
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0; 1024];
        let count = read
            .read(&mut buffer)
            .await
            .map_err(|_| ApiError::bad("Resource request read failed"))?;
        if count == 0 {
            return Err(ApiError::cancelled());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > 1024 * 1024 {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Resource request exceeds 1 MiB",
            ));
        }
        if bytes.windows(4).any(|b| b == b"\r\n\r\n") {
            break;
        }
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| ApiError::bad("Invalid HTTP request"))?;
    if !text.ends_with("\r\n\r\n") {
        return Err(ApiError::bad("Resource requests have no body"));
    }
    let mut lines = text.split("\r\n");
    let parts: Vec<_> = lines.next().unwrap_or("").split(' ').collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err(ApiError::bad("Invalid HTTP request line"));
    }
    let method =
        Method::from_bytes(parts[0].as_bytes()).map_err(|_| ApiError::bad("Invalid method"))?;
    let (path, query) = parts[1].split_once('?').unwrap_or((parts[1], ""));
    if !path.starts_with('/') || path.contains(['%', '#', '\\']) {
        return Err(ApiError::bad("Invalid resource path"));
    }
    let cap = if query.is_empty() {
        None
    } else {
        Some(
            query
                .strip_prefix("cap=")
                .filter(|v| {
                    v.len() == 43
                        && v.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                })
                .ok_or_else(|| ApiError::new("UNAUTHORIZED", "Invalid resource capability"))?
                .into(),
        )
    };
    let mut headers = HeaderMap::new();
    for line in lines.take_while(|l| !l.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ApiError::bad("Invalid request header"))?;
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| ApiError::bad("Invalid request header name"))?;
        let value = reqwest::header::HeaderValue::from_str(value.trim())
            .map_err(|_| ApiError::bad("Invalid request header value"))?;
        if headers.contains_key(&name) {
            return Err(ApiError::bad("Duplicate request header"));
        }
        headers.insert(name, value);
    }
    if headers.contains_key("transfer-encoding")
        || headers.get("content-length").is_some_and(|v| v != "0")
    {
        return Err(ApiError::bad("Resource requests have no body"));
    }
    let origin = headers
        .get("origin")
        .map(|h| h.to_str().map(str::to_owned))
        .transpose()
        .map_err(|_| ApiError::bad("Invalid Origin"))?;
    if origin
        .as_deref()
        .is_some_and(|o| !matches!(o, "tauri://localhost" | "http://tauri.localhost"))
    {
        return Err(ApiError::new("UNAUTHORIZED", "Resource Origin rejected"));
    }
    Ok(Request {
        method,
        path: path.into(),
        cap,
        headers,
        origin,
    })
}
struct RequestLease {
    shared: Arc<Shared>,
    target: String,
    resource: String,
    sequence: u64,
    selected: Option<Arc<LinkRecord>>,
    cancelled: Arc<AtomicBool>,
    url: String,
    headers: HeaderMap,
    kind: &'static str,
    finished: bool,
}
impl Shared {
    fn acquire(self: &Arc<Self>, req: &Request) -> Result<RequestLease, ApiError> {
        let parts: Vec<_> = req.path.split('/').collect();
        let mut records = self
            .records
            .lock()
            .map_err(|_| ApiError::internal("Resource registry poisoned"))?;
        let now = self.clock.now_ms();
        records.sweep(now);
        records.sequence += 1;
        let sequence = records.sequence;
        let bearer = req
            .headers
            .get("authorization")
            .is_some_and(|v| v.as_bytes() == format!("Bearer {}", self.token).as_bytes());
        let permitted = |expected: &str| bearer || req.cap.as_deref() == Some(expected);
        if let ["", "media", session, resource] = parts.as_slice() {
            let s = records.sessions.get_mut(*session).ok_or_else(expired)?;
            if !permitted(&s.cap) {
                return Err(ApiError::new(
                    "UNAUTHORIZED",
                    "Resource capability rejected",
                ));
            }
            let r = s.resources.get(*resource).ok_or_else(expired)?;
            s.active += 1;
            s.last_activity = now;
            Ok(RequestLease {
                shared: self.clone(),
                target: (*session).into(),
                resource: (*resource).into(),
                sequence,
                selected: Some(s.selected.clone()),
                cancelled: s.cancelled.clone(),
                url: r.url.clone(),
                headers: r.headers.clone(),
                kind: r.kind,
                finished: false,
            })
        } else if let ["", "images", id] = parts.as_slice() {
            let i = records.images.get_mut(*id).ok_or_else(expired)?;
            if !permitted(i.image.reference["capability"].as_str().expect("Image cap")) {
                return Err(ApiError::new(
                    "UNAUTHORIZED",
                    "Resource capability rejected",
                ));
            }
            i.expires = now.saturating_add(IMAGE_IDLE_MS);
            i.used = sequence;
            Ok(RequestLease {
                shared: self.clone(),
                target: (*id).into(),
                resource: (*id).into(),
                sequence,
                selected: None,
                cancelled: i.cancelled.clone(),
                url: i.image.url.clone(),
                headers: i.image.headers.clone(),
                kind: "image",
                finished: false,
            })
        } else {
            Err(expired())
        }
    }
}
impl RequestLease {
    fn finish(&mut self, result: Result<(), ApiError>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let mut records = self.shared.records.lock().expect("Resource registry");
        let diagnostic = if self.kind == "image" {
            records
                .images
                .get_mut(&self.target)
                .filter(|i| Arc::ptr_eq(&i.cancelled, &self.cancelled))
                .map(|i| &mut i.diagnostic)
        } else {
            records.sessions.get_mut(&self.target).and_then(|s| {
                s.active = s.active.saturating_sub(1);
                s.last_activity = self.shared.clock.now_ms();
                s.resources
                    .get_mut(&self.resource)
                    .map(|r| &mut r.diagnostic)
            })
        };
        if let Some(d) = diagnostic
            && self.sequence > d.completed
            && !self.cancelled.load(Ordering::SeqCst)
        {
            if result.as_ref().err().is_some_and(|e| e.code == "CANCELLED") {
                return;
            }
            d.completed = self.sequence;
            d.failure = result.err().map(|e| {
                let mut details = e
                    .details
                    .as_ref()
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                details.insert("resource_kind".into(), json!(self.kind));
                (self.sequence, e.with_details(Value::Object(details)))
            });
        }
    }
}
impl Drop for RequestLease {
    fn drop(&mut self) {
        self.finish(Err(ApiError::cancelled()));
    }
}
/// EOF cancels pending DNS/connect/body work even when upstream sends no bytes.
async fn watched<T>(
    lease: &RequestLease,
    read: &mut OwnedReadHalf,
    future: impl Future<Output = Result<T, ApiError>>,
) -> Result<T, ApiError> {
    tokio::pin!(future);
    let timeout = tokio::time::sleep(Duration::from_millis(
        lease.shared.idle_ms.load(Ordering::SeqCst),
    ));
    tokio::pin!(timeout);
    let mut extra = [0; 1];
    loop {
        if lease.cancelled.load(Ordering::SeqCst) || lease.shared.stop.load(Ordering::SeqCst) {
            return Err(ApiError::cancelled());
        }
        tokio::select! {
            result=&mut future=>return result,
            _=&mut timeout=>return Err(ApiError::new("UPSTREAM_TIMEOUT","Resource I/O idle timeout")),
            _=read.read(&mut extra)=>return Err(ApiError::cancelled()),
            _=tokio::time::sleep(Duration::from_millis(10))=>{},
        }
    }
}
async fn serve(
    stream: tokio::net::TcpStream,
    shared: Arc<Shared>,
    slots: Arc<tokio::sync::Semaphore>,
) {
    let (mut read, mut write) = stream.into_split();
    let req = match tokio::time::timeout(Duration::from_secs(1), request(&mut read)).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            let _ = error_response(&mut write, &e, None, false).await;
            return;
        }
        Err(_) => return,
    };
    let origin = req.origin.as_deref();
    let head = req.method == Method::HEAD;
    if req.path == "/host-health" {
        let authorized = req
            .headers
            .get("authorization")
            .is_some_and(|v| v.as_bytes() == format!("Bearer {}", shared.token).as_bytes());
        if !authorized {
            let _ = error_response(
                &mut write,
                &ApiError::new("UNAUTHORIZED", "Control Bearer required"),
                None,
                head,
            )
            .await;
        } else if req.method == Method::GET {
            let body = host_health().to_string();
            let headers = HeaderMap::from_iter([(
                "content-type".parse().expect("Header name"),
                "application/json".parse().expect("Header value"),
            )]);
            let _ = respond(&mut write, 200, &headers, body.as_bytes(), None, false).await;
        } else {
            let _ = error_response(
                &mut write,
                &ApiError::bad("Control method rejected"),
                None,
                head,
            )
            .await;
        }
        return;
    }
    if req.method == Method::OPTIONS {
        let valid = origin.is_some()
            && (req.path.starts_with("/media/") || req.path.starts_with("/images/"))
            && req
                .headers
                .get("access-control-request-method")
                .is_some_and(|m| m == "GET" || m == "HEAD")
            && req
                .headers
                .get("access-control-request-headers")
                .is_none_or(|h| {
                    h.to_str().is_ok_and(|v| {
                        v.split(',').all(|s| {
                            matches!(
                                s.trim().to_ascii_lowercase().as_str(),
                                "range" | "if-range" | "if-none-match" | "if-modified-since"
                            )
                        })
                    })
                });
        if valid {
            let _ = respond(&mut write, 204, &HeaderMap::new(), &[], origin, true).await;
        } else {
            let _ = error_response(
                &mut write,
                &ApiError::bad("Resource preflight rejected"),
                origin,
                true,
            )
            .await;
        }
        return;
    }
    if req.method != Method::GET && !head {
        let _ = error_response(
            &mut write,
            &ApiError::bad("Resources accept GET/HEAD"),
            origin,
            head,
        )
        .await;
        return;
    }
    let Ok(_slot) = slots.try_acquire_owned() else {
        let _ = error_response(
            &mut write,
            &ApiError::new("RESOURCE_LIMIT", "Resource concurrency limit reached"),
            origin,
            head,
        )
        .await;
        return;
    };
    let mut lease = match shared.acquire(&req) {
        Ok(l) => l,
        Err(e) => {
            let _ = error_response(&mut write, &e, origin, head).await;
            return;
        }
    };
    let mut sent = false;
    let result = transfer(&req, &lease, &mut read, &mut write, &mut sent).await;
    if let Err(e) = &result
        && !sent
        && e.code != "CANCELLED"
    {
        let _ = error_response(&mut write, e, origin, head).await;
    }
    lease.finish(result);
}
async fn transfer(
    req: &Request,
    lease: &RequestLease,
    read: &mut OwnedReadHalf,
    write: &mut OwnedWriteHalf,
    sent: &mut bool,
) -> Result<(), ApiError> {
    let url = http_url(&lease.url, false)?;
    let mut headers = lease.headers.clone();
    if let Some(selected) = &lease.selected {
        let same_origin = http_url(&selected.link.url, false)?.origin() == url.origin();
        if lease.kind == "subtitle" && same_origin {
            // Merge on demand: copying a large media header into every subtitle
            // record would amplify the bounded Resolution's retained context.
            let mut inherited = selected.link.headers.clone();
            inherited.extend(headers);
            headers = inherited;
        }
        if !selected.link.referer.is_empty() && same_origin {
            headers.insert(
                "referer",
                selected
                    .link
                    .referer
                    .parse()
                    .map_err(|_| ApiError::bad("Invalid native Referer"))?,
            );
        }
        for context in [&selected.link.interceptor, &selected.link.cookies]
            .into_iter()
            .flatten()
        {
            let request = if let Some(context) = context.downcast_ref::<HttpContext>() {
                &context.0
            } else if let Some(context) = context.downcast_ref::<HttpManifestContext>() {
                &context.request
            } else {
                return Err(ApiError::new(
                    "CAPABILITY_UNAVAILABLE",
                    "Network context requires its original provider gateway",
                ));
            };
            request(&url, &mut headers).map_err(|error| {
                let code = match error.code.as_str() {
                    "CANCELLED" => "CANCELLED",
                    "UPSTREAM_TIMEOUT" => "UPSTREAM_TIMEOUT",
                    "UNAUTHORIZED" => "UNAUTHORIZED",
                    "INTERACTION_REQUIRED" => "INTERACTION_REQUIRED",
                    "RESOURCE_LIMIT" => "RESOURCE_LIMIT",
                    _ => "UPSTREAM_FAILED",
                };
                ApiError::new(code, "Original source request context failed")
            })?;
        }
    }
    // Browser credentials terminate locally. Only representation headers are copied;
    // provider credentials come exclusively from the original native context.
    for name in [
        "host",
        "connection",
        "content-length",
        "transfer-encoding",
        "origin",
    ] {
        headers.remove(name);
    }
    headers.insert(
        "accept-encoding",
        "identity".parse().expect("Static header"),
    );
    let is_subtitle = lease.kind == "subtitle";
    let manifest = !is_subtitle
        && (lease.kind == "manifest" || url.path().to_ascii_lowercase().ends_with(".m3u8"));
    let transformed = manifest || is_subtitle;
    if let Some(range) = req.headers.get("range") {
        if lease.kind == "image" || !valid_range(range.to_str().unwrap_or("")) {
            return Err(ApiError::bad("Invalid single byte Range"));
        }
        if !transformed {
            headers.insert("range", range.clone());
            if let Some(v) = req.headers.get("if-range") {
                headers.insert("if-range", v.clone());
            }
        }
    }
    for name in ["if-none-match", "if-modified-since"] {
        if !transformed
            && lease.kind != "image"
            && let Some(value) = req.headers.get(name)
        {
            headers.insert(name, value.clone());
        }
    }
    // A transformed HEAD describes the local representation, so fetch its body.
    let method = if req.method == Method::HEAD && lease.kind != "image" && !transformed {
        Method::HEAD
    } else {
        Method::GET
    };
    let cache_key = if req.method == Method::GET
        && lease.kind == "media"
        && !transformed
        && !media_cache::request_bypasses(&req.headers)
        && !media_cache::request_bypasses(&headers)
        && !["if-range", "if-none-match", "if-modified-since"]
            .iter()
            .any(|name| req.headers.contains_key(*name))
    {
        lease.selected.as_ref().map(|selected| {
            media_cache::key(
                &selected.link.provider,
                &selected.source_revision,
                url.as_str(),
                &headers,
            )
        })
    } else {
        None
    };
    if let Some(key) = &cache_key
        && let Some(mut cached) = lease.shared.media_cache.get(key)
    {
        let mut bytes = vec![0; STREAM_BUFFER];
        // A cache read that fails before headers can safely use the original network path.
        if let Ok(first) = cached.read(&mut bytes)
            && first > 0
            && !looks_html(&bytes[..first.min(64)])
        {
            if let Some(mime) = lease
                .selected
                .as_ref()
                .filter(|selected| lease.resource == selected.stream_id)
                .and_then(|selected| selected.link.mime_type.as_ref())
            {
                cached.headers.insert(
                    "content-type",
                    mime.parse()
                        .map_err(|_| ApiError::bad("Invalid native media MIME"))?,
                );
            }
            cached
                .headers
                .insert("cache-control", "no-store".parse().expect("cache header"));
            *sent = true;
            watched(
                lease,
                read,
                write_head(
                    write,
                    cached.status,
                    &cached.headers,
                    Some(cached.length),
                    req.origin.as_deref(),
                    false,
                ),
            )
            .await?;
            write_chunk(lease, read, write, &bytes[..first], false).await?;
            loop {
                let count = cached
                    .read(&mut bytes)
                    .map_err(|_| upstream("Cached media read failed; retry playback"))?;
                if count == 0 {
                    break;
                }
                write_chunk(lease, read, write, &bytes[..count], false).await?;
            }
            return Ok(());
        }
    }
    let mut response = watched(
        lease,
        read,
        lease
            .shared
            .http
            .open(url.clone(), headers.clone(), method.clone(), 10_000, false),
    )
    .await?;
    let status = response.status().as_u16();
    if let Some(error) = crate::host::maccms::access_status(status, response.headers()) {
        return Err(error);
    }
    if response
        .headers()
        .get("content-encoding")
        .is_some_and(|value| value != "identity")
    {
        return Err(upstream("Unsupported upstream resource content encoding"));
    }
    // Body::size_hint shrinks as chunks are consumed. Capture it before reading
    // the prefix, and preserve the representation length separately for HEAD.
    let known_length = response.content_length();
    let mut output = response_headers(response.headers());
    // reqwest removes Encoding/Length during gzip decoding, but retains ETag.
    // Unknown-length streaming responses cannot safely advertise a byte validator
    // for the upstream encoded entity. Bounded transforms generate their own hash.
    if !response.headers().contains_key("content-length") {
        output.remove("etag");
        output.remove("last-modified");
    }
    if !output.contains_key("content-type")
        && let Some(mime) = lease
            .selected
            .as_ref()
            .filter(|s| lease.resource == s.stream_id)
            .and_then(|s| s.link.mime_type.as_ref())
    {
        output.insert(
            "content-type",
            mime.parse()
                .map_err(|_| ApiError::bad("Invalid native media MIME"))?,
        );
    }
    if status == 304
        && !transformed
        && lease.kind != "image"
        && (req.headers.contains_key("if-none-match")
            || req.headers.contains_key("if-modified-since"))
    {
        *sent = true;
        return watched(
            lease,
            read,
            write_head(write, 304, &output, None, req.origin.as_deref(), false),
        )
        .await;
    }
    if status == 416 {
        if !req.headers.contains_key("range")
            || !response
                .headers()
                .get("content-range")
                .is_some_and(|v| valid_unsatisfied(v.to_str().unwrap_or("")))
        {
            return Err(upstream("Invalid unsatisfied Content-Range"));
        }
        *sent = true;
        return watched(
            lease,
            read,
            respond(
                write,
                416,
                &output,
                &[],
                req.origin.as_deref(),
                req.method == Method::HEAD,
            ),
        )
        .await;
    }
    if !matches!(status, 200 | 206) {
        return Err(upstream("Resource server returned an unsuccessful status"));
    }
    if status == 206 && (!req.headers.contains_key("range") || !valid_partial(response.headers())) {
        return Err(upstream("Invalid partial resource response"));
    }
    if status == 200 {
        output.remove("content-range");
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if is_subtitle {
        return transfer_subtitle(req, lease, read, write, sent, response).await;
    }
    let manifest = manifest || is_hls_mime(&content_type);
    if manifest {
        if status != 200 {
            return Err(upstream("HLS manifest requires a complete response"));
        }
        if method == Method::HEAD {
            response = watched(
                lease,
                read,
                lease
                    .shared
                    .http
                    .open(url, headers, Method::GET, 10_000, false),
            )
            .await?;
            if response.status().as_u16() != 200 {
                return Err(upstream(
                    "HLS manifest HEAD could not obtain complete representation",
                ));
            }
        }
        return transfer_manifest(req, lease, read, write, sent, response, Vec::new()).await;
    }
    if content_type.contains("text/html") || content_type.contains("application/xhtml") {
        return Err(upstream("Resource server returned HTML"));
    }
    if lease.kind == "image" {
        if known_length.is_some_and(|n| n > IMAGE_BYTES as u64) {
            return Err(ApiError::new("RESOURCE_LIMIT", "Image exceeds 8 MiB"));
        }
        let mut body = Vec::new();
        while let Some(chunk) = watched(lease, read, async {
            response.chunk().await.map_err(resource_error)
        })
        .await?
        {
            if body.len().saturating_add(chunk.len()) > IMAGE_BYTES {
                return Err(ApiError::new("RESOURCE_LIMIT", "Image exceeds 8 MiB"));
            }
            body.extend_from_slice(&chunk);
        }
        let mime = validate_image(&body)?;
        output.insert(
            "etag",
            format!("\"{:x}\"", Sha256::digest(&body))
                .parse()
                .expect("Image ETag"),
        );
        output.insert("content-type", mime.parse().expect("Static image MIME"));
        output.insert(
            "cache-control",
            "private, max-age=60".parse().expect("Static cache header"),
        );
        *sent = true;
        return watched(
            lease,
            read,
            respond(
                write,
                200,
                &output,
                &body,
                req.origin.as_deref(),
                req.method == Method::HEAD,
            ),
        )
        .await;
    }
    output.insert(
        "cache-control",
        "no-store".parse().expect("Static cache header"),
    );
    if req.method == Method::HEAD {
        let length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        *sent = true;
        return watched(
            lease,
            read,
            write_head(write, status, &output, length, req.origin.as_deref(), false),
        )
        .await;
    }
    let mut first = Vec::with_capacity(STREAM_BUFFER);
    let mut pending = None;
    while first.len() < 64 {
        let Some(chunk) = watched(lease, read, async {
            response.chunk().await.map_err(resource_error)
        })
        .await?
        else {
            break;
        };
        let count = (64 - first.len()).min(chunk.len());
        first.extend_from_slice(&chunk[..count]);
        if count < chunk.len() {
            pending = Some(chunk.slice(count..));
            break;
        }
    }
    if looks_html(&first[..first.len().min(STREAM_BUFFER)]) {
        return Err(upstream("Resource body is HTML"));
    }
    if first.starts_with(b"#EXTM3U") && lease.kind != "image" {
        if status != 200 {
            return Err(upstream("HLS manifest requires a complete response"));
        }
        if let Some(chunk) = pending {
            if first.len().saturating_add(chunk.len()) > hls::LIMIT {
                return Err(ApiError::new(
                    "RESOURCE_LIMIT",
                    "HLS manifest exceeds 8 MiB",
                ));
            }
            first.extend_from_slice(&chunk);
        }
        return transfer_manifest(req, lease, read, write, sent, response, first).await;
    }
    let length = known_length;
    let mut cache_write = cache_key.and_then(|key| {
        lease
            .shared
            .media_cache
            .begin(key, status, response.headers(), &output, length)
    });
    *sent = true;
    watched(
        lease,
        read,
        write_head(
            write,
            status,
            &output,
            length,
            req.origin.as_deref(),
            length.is_none(),
        ),
    )
    .await?;
    let mut total = 0u64;
    if !first.is_empty() {
        total += first.len() as u64;
        write_chunk(lease, read, write, &first, length.is_none()).await?;
        cache_append(&mut cache_write, &first);
    }
    if let Some(pending) = pending {
        total += pending.len() as u64;
        write_chunk(lease, read, write, &pending, length.is_none()).await?;
        cache_append(&mut cache_write, &pending);
    }
    while let Some(chunk) = watched(lease, read, async {
        response.chunk().await.map_err(resource_error)
    })
    .await?
    {
        total = total.saturating_add(chunk.len() as u64);
        if length.is_some_and(|n| total > n) {
            return Err(upstream("Resource body exceeds Content-Length"));
        }
        write_chunk(lease, read, write, &chunk, length.is_none()).await?;
        cache_append(&mut cache_write, &chunk);
    }
    if length.is_some_and(|n| total != n) {
        return Err(upstream("Resource body ended before Content-Length"));
    }
    if length.is_none() {
        watched(lease, read, async {
            write.write_all(b"0\r\n\r\n").await.map_err(client_error)
        })
        .await?;
    }
    if let Some(cache_write) = cache_write {
        cache_write.finish();
    }
    Ok(())
}

fn cache_append(writer: &mut Option<media_cache::CacheWrite>, bytes: &[u8]) {
    // Preserve the gateway's bounded streaming buffer even when reqwest returns a large chunk.
    for chunk in bytes.chunks(STREAM_BUFFER) {
        if let Some(current) = writer
            && !current.append(chunk)
        {
            *writer = None;
            break;
        }
    }
}

async fn transfer_subtitle(
    req: &Request,
    lease: &RequestLease,
    read: &mut OwnedReadHalf,
    write: &mut OwnedWriteHalf,
    sent: &mut bool,
    mut response: reqwest::Response,
) -> Result<(), ApiError> {
    if response.status().as_u16() != 200 {
        return Err(upstream("Subtitle requires a complete response"));
    }
    if response
        .content_length()
        .is_some_and(|n| n > subtitle::LIMIT as u64)
    {
        return Err(ApiError::new("RESOURCE_LIMIT", "Subtitle exceeds 8 MiB"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = watched(lease, read, async {
        response.chunk().await.map_err(resource_error)
    })
    .await?
    {
        if body.len().saturating_add(chunk.len()) > subtitle::LIMIT {
            return Err(ApiError::new("RESOURCE_LIMIT", "Subtitle exceeds 8 MiB"));
        }
        body.extend_from_slice(&chunk);
    }
    let format = lease
        .selected
        .as_ref()
        .and_then(|s| s.subtitles.iter().find(|s| s.id == lease.resource))
        .map(|s| s.subtitle.format.as_str())
        .ok_or_else(|| ApiError::internal("Subtitle resource lost its registered format"))?;
    let body = subtitle::convert(&body, format)?;
    let mut output = HeaderMap::new();
    output.insert(
        "content-type",
        "text/vtt; charset=utf-8".parse().expect("VTT MIME"),
    );
    output.insert("cache-control", "no-store".parse().expect("Cache header"));
    output.insert(
        "etag",
        format!("\"{:x}\"", Sha256::digest(&body))
            .parse()
            .expect("VTT ETag"),
    );
    let not_modified = req
        .headers
        .get("if-none-match")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| {
            value.split(',').any(|v| {
                v.trim() == "*"
                    || v.trim().strip_prefix("W/").unwrap_or(v.trim())
                        == output["etag"].to_str().expect("VTT ETag")
            })
        });
    *sent = true;
    if not_modified {
        watched(
            lease,
            read,
            write_head(write, 304, &output, None, req.origin.as_deref(), false),
        )
        .await
    } else {
        watched(
            lease,
            read,
            respond(
                write,
                200,
                &output,
                &body,
                req.origin.as_deref(),
                req.method == Method::HEAD,
            ),
        )
        .await
    }
}

fn is_hls_mime(mime: &str) -> bool {
    matches!(
        mime.split(';').next().unwrap_or("").trim(),
        "application/vnd.apple.mpegurl"
            | "application/x-mpegurl"
            | "audio/mpegurl"
            | "audio/x-mpegurl"
    )
}

async fn transfer_manifest(
    req: &Request,
    lease: &RequestLease,
    read: &mut OwnedReadHalf,
    write: &mut OwnedWriteHalf,
    sent: &mut bool,
    mut response: reqwest::Response,
    mut body: Vec<u8>,
) -> Result<(), ApiError> {
    if response
        .headers()
        .get("content-encoding")
        .is_some_and(|value| value != "identity")
    {
        return Err(upstream("Unsupported upstream resource content encoding"));
    }
    if response
        .content_length()
        .is_some_and(|n| n > hls::LIMIT as u64)
    {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "HLS manifest exceeds 8 MiB",
        ));
    }
    let base = response.url().clone();
    let mut headers = response.headers().clone();
    while let Some(chunk) = watched(lease, read, async {
        response.chunk().await.map_err(resource_error)
    })
    .await?
    {
        if body.len().saturating_add(chunk.len()) > hls::LIMIT {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "HLS manifest exceeds 8 MiB",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    if let Some(context) = lease
        .selected
        .as_ref()
        .and_then(|s| s.link.interceptor.as_ref())
        .and_then(|c| c.downcast_ref::<HttpManifestContext>())
    {
        (context.response)(&base, &mut headers, &mut body).map_err(|error| {
            let code = match error.code.as_str() {
                "CANCELLED" => "CANCELLED",
                "UPSTREAM_TIMEOUT" => "UPSTREAM_TIMEOUT",
                "UNAUTHORIZED" => "UNAUTHORIZED",
                "INTERACTION_REQUIRED" => "INTERACTION_REQUIRED",
                "RESOURCE_LIMIT" => "RESOURCE_LIMIT",
                "MEDIA_UNSUPPORTED" => "MEDIA_UNSUPPORTED",
                _ => "UPSTREAM_FAILED",
            };
            ApiError::new(code, "Original source manifest response processing failed")
        })?;
    }
    if body.len() > hls::LIMIT {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Processed HLS manifest exceeds 8 MiB",
        ));
    }
    if looks_html(&body) {
        return Err(upstream("Resource body is HTML"));
    }
    let text = std::str::from_utf8(&body).map_err(|_| upstream("HLS manifest is not UTF-8"))?;
    let body = rewrite_manifest(lease, &base, text)?;
    let mut output = response_headers(&headers);
    for name in ["content-range", "accept-ranges", "last-modified"] {
        output.remove(name);
    }
    output.insert(
        "content-type",
        "application/vnd.apple.mpegurl".parse().expect("HLS MIME"),
    );
    output.insert("cache-control", "no-store".parse().expect("Cache header"));
    output.insert(
        "etag",
        format!("\"{:x}\"", Sha256::digest(&body))
            .parse()
            .expect("HLS ETag"),
    );
    let not_modified = req
        .headers
        .get("if-none-match")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| {
            value.split(',').any(|v| {
                v.trim() == "*"
                    || v.trim().strip_prefix("W/").unwrap_or(v.trim())
                        == output["etag"].to_str().expect("HLS ETag")
            })
        });
    *sent = true;
    if not_modified {
        watched(
            lease,
            read,
            write_head(write, 304, &output, None, req.origin.as_deref(), false),
        )
        .await
    } else {
        watched(
            lease,
            read,
            respond(
                write,
                200,
                &output,
                &body,
                req.origin.as_deref(),
                req.method == Method::HEAD,
            ),
        )
        .await
    }
}

fn rewrite_manifest(
    lease: &RequestLease,
    base: &url::Url,
    text: &str,
) -> Result<Vec<u8>, ApiError> {
    let refs = hls::references(text)?;
    let mut inherited = lease.headers.clone();
    redirect_headers(&http_url(&lease.url, false)?, base, &mut inherited)?;
    let mut records = lease
        .shared
        .records
        .lock()
        .map_err(|_| ApiError::internal("Resource registry poisoned"))?;
    let session = records
        .sessions
        .get_mut(&lease.target)
        .filter(|s| !s.cancelled.load(Ordering::SeqCst))
        .ok_or_else(ApiError::cancelled)?;
    let mut pending = HashMap::new();
    let mut output = String::with_capacity(text.len());
    let mut previous = 0;
    let mut state_bytes = session
        .resources
        .values()
        .map(|r| {
            r.url.len()
                + r.headers
                    .iter()
                    .map(|(k, v)| k.as_str().len() + v.len())
                    .sum::<usize>()
        })
        .sum::<usize>();
    for reference in refs {
        let url = base
            .join(&text[reference.span.clone()])
            .map_err(|_| upstream("Invalid HLS resource URI"))?;
        http_url(url.as_str(), false)?;
        let mut headers = inherited.clone();
        redirect_headers(base, &url, &mut headers)?;
        let kind = if reference.playlist || url.path().to_ascii_lowercase().ends_with(".m3u8") {
            "manifest"
        } else if reference.cacheable {
            "media"
        } else {
            "auxiliary"
        };
        // Stable identity preserves byterange implicit offsets and repeated live
        // refreshes. Include inherited headers so different scopes never alias.
        let mut digest = Sha256::new();
        digest.update(url.as_str().as_bytes());
        digest.update([0]);
        digest.update(kind.as_bytes());
        let mut fields: Vec<_> = headers.iter().collect();
        fields.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        for (name, value) in fields {
            digest.update([0]);
            digest.update(name.as_str().as_bytes());
            digest.update([0]);
            digest.update(value.as_bytes());
        }
        let id = format!("hls-{:x}", digest.finalize());
        if !session.resources.contains_key(&id) && !pending.contains_key(&id) {
            state_bytes = state_bytes.saturating_add(
                url.as_str().len()
                    + headers
                        .iter()
                        .map(|(k, v)| k.as_str().len() + v.len())
                        .sum::<usize>(),
            );
            if session.resources.len() + pending.len() >= 65_536 || state_bytes > hls::LIMIT {
                return Err(ApiError::new(
                    "RESOURCE_LIMIT",
                    "HLS resource registry exceeds its count or 8 MiB context budget",
                ));
            }
            pending.insert(
                id.clone(),
                Resource {
                    url: url.to_string(),
                    headers,
                    kind,
                    diagnostic: Diagnostic::default(),
                },
            );
        }
        output.push_str(&text[previous..reference.span.start]);
        output.push_str(&format!(
            "http://{}/media/{}/{id}?cap={}",
            lease.shared.address, lease.target, session.cap
        ));
        previous = reference.span.end;
        if output.len() > hls::LIMIT {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Rewritten HLS manifest exceeds 8 MiB",
            ));
        }
    }
    output.push_str(&text[previous..]);
    if output.len() > hls::LIMIT {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Rewritten HLS manifest exceeds 8 MiB",
        ));
    }
    session.resources.extend(pending);
    if let Some(resource) = session.resources.get_mut(&lease.resource) {
        resource.kind = "manifest";
    }
    Ok(output.into_bytes())
}

fn valid_range(value: &str) -> bool {
    let Some((start, end)) = value.strip_prefix("bytes=").and_then(|v| v.split_once('-')) else {
        return false;
    };
    let number = |s: &str| {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && s.parse::<u64>().is_ok()
    };
    if start.is_empty() {
        number(end) && end.parse::<u64>().is_ok_and(|n| n > 0)
    } else {
        number(start)
            && (end.is_empty()
                || number(end) && start.parse::<u64>().unwrap() <= end.parse::<u64>().unwrap())
    }
}
fn valid_unsatisfied(value: &str) -> bool {
    value.strip_prefix("bytes */").is_some_and(|n| {
        !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && n.parse::<u64>().is_ok()
    })
}
fn valid_partial(headers: &HeaderMap) -> bool {
    let Some(value) = headers
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
    else {
        return false;
    };
    let Some((range, total)) = value.split_once('/') else {
        return false;
    };
    let Some((start, end)) = range.split_once('-') else {
        return false;
    };
    let (Ok(start), Ok(end), Ok(total)) = (
        start.parse::<u64>(),
        end.parse::<u64>(),
        total.parse::<u64>(),
    ) else {
        return false;
    };
    start <= end
        && end < total
        && headers
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            == Some(end - start + 1)
}
fn looks_html(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes)
        .trim_start_matches(['\u{feff}', ' ', '\r', '\n', '\t'])
        .to_ascii_lowercase();
    text.starts_with("<!doctype html")
        || text.starts_with("<html")
        || text.starts_with("<head")
        || text.starts_with("<body")
}
fn validate_image(body: &[u8]) -> Result<&'static str, ApiError> {
    let bad = || ApiError::new("MEDIA_UNSUPPORTED", "Invalid or unsupported raster image");
    let format = image::guess_format(body).map_err(|_| bad())?;
    let mime = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(bad()),
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(body), format)
        .into_dimensions()
        .map_err(|_| bad())?;
    if width == 0 || height == 0 {
        return Err(bad());
    }
    if width > 8192 || height > 8192 || u64::from(width) * u64::from(height) > IMAGE_PIXELS {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Image exceeds 8192 dimensions or 16777216 pixels",
        ));
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(body), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| {
        if matches!(e, image::ImageError::Limits(_)) {
            ApiError::new("RESOURCE_LIMIT", "Image decoder allocation limit exceeded")
        } else {
            bad()
        }
    })?;
    Ok(mime)
}
fn resource_error(e: reqwest::Error) -> ApiError {
    if e.is_timeout() {
        ApiError::new("UPSTREAM_TIMEOUT", "Resource request timed out")
    } else {
        upstream("Resource transfer failed")
    }
}
fn client_error(_: std::io::Error) -> ApiError {
    ApiError::cancelled()
}
fn response_headers(headers: &HeaderMap) -> HeaderMap {
    headers
        .iter()
        .filter(|(n, _)| {
            matches!(
                n.as_str(),
                "content-type" | "content-range" | "accept-ranges" | "etag" | "last-modified"
            )
        })
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect()
}
async fn write_chunk(
    lease: &RequestLease,
    read: &mut OwnedReadHalf,
    write: &mut OwnedWriteHalf,
    bytes: &[u8],
    chunked: bool,
) -> Result<(), ApiError> {
    for chunk in bytes.chunks(STREAM_BUFFER) {
        watched(lease, read, async {
            if chunked {
                write
                    .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                    .await
                    .map_err(client_error)?;
            }
            write.write_all(chunk).await.map_err(client_error)?;
            if chunked {
                write.write_all(b"\r\n").await.map_err(client_error)?;
            }
            Ok(())
        })
        .await?;
    }
    Ok(())
}
async fn write_head(
    write: &mut OwnedWriteHalf,
    status: u16,
    headers: &HeaderMap,
    length: Option<u64>,
    origin: Option<&str>,
    chunked: bool,
) -> Result<(), ApiError> {
    let reason = reqwest::StatusCode::from_u16(status)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("Resource");
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nVary: Origin\r\n"
    );
    if let Some(origin) = origin {
        head.push_str(&format!("Access-Control-Allow-Origin: {origin}\r\nAccess-Control-Allow-Methods: GET, HEAD, OPTIONS\r\nAccess-Control-Allow-Headers: Range, If-Range, If-None-Match, If-Modified-Since\r\nAccess-Control-Expose-Headers: Content-Range, Accept-Ranges, Content-Length, ETag\r\n"));
    }
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    if let Some(length) = length {
        head.push_str(&format!("Content-Length: {length}\r\n"));
    } else if chunked {
        head.push_str("Transfer-Encoding: chunked\r\n");
    }
    head.push_str("\r\n");
    write.write_all(head.as_bytes()).await.map_err(client_error)
}
async fn respond(
    write: &mut OwnedWriteHalf,
    status: u16,
    headers: &HeaderMap,
    body: &[u8],
    origin: Option<&str>,
    head: bool,
) -> Result<(), ApiError> {
    write_head(
        write,
        status,
        headers,
        Some(body.len() as u64),
        origin,
        false,
    )
    .await?;
    if !head {
        write.write_all(body).await.map_err(client_error)?;
    }
    Ok(())
}
async fn error_response(
    write: &mut OwnedWriteHalf,
    error: &ApiError,
    origin: Option<&str>,
    head: bool,
) -> Result<(), ApiError> {
    let status = match error.code.as_str() {
        "UNAUTHORIZED" => 401,
        "SESSION_EXPIRED" => 410,
        "BAD_REQUEST" => 400,
        "RESOURCE_LIMIT" => 413,
        "UPSTREAM_TIMEOUT" => 504,
        _ => 502,
    };
    let headers = HeaderMap::from_iter([
        (
            "content-type".parse().expect("Header name"),
            "application/json".parse().expect("Header value"),
        ),
        (
            "cache-control".parse().expect("Header name"),
            "no-store".parse().expect("Header value"),
        ),
    ]);
    respond(
        write,
        status,
        &headers,
        serde_json::to_string(&json!({"error":error}))
            .expect("Error serialization")
            .as_bytes(),
        origin,
        head,
    )
    .await
}
#[cfg(test)]
#[path = "../../tests/fixtures/t09_gateway.rs"]
mod tests;
