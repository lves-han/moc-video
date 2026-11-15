//! Private media bytes only. A gateway lease authorizes every lookup; keys never contain persisted credentials.
use crate::contracts::ApiError;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description::well_known::Rfc2822};
use uuid::Uuid;

pub const DEFAULT_LIMIT: u64 = 256 * 1024 * 1024;
const HEADER: u64 = 4096;
const CACHE_TTL: u64 = 7 * 24 * 60 * 60;
const HEADERS: &[&str] = &[
    "content-type",
    "content-range",
    "accept-ranges",
    "etag",
    "last-modified",
];

pub fn limit(settings: &Value) -> u64 {
    settings["media_cache_max_bytes"]
        .as_u64()
        .unwrap_or(DEFAULT_LIMIT)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn io_error(operation: &str, error: &std::io::Error) -> ApiError {
    let mut error_value = ApiError::new("INTERNAL_ERROR", "Media cache file operation failed");
    error_value.retryable = true;
    error_value.details = Some(
        json!({"reason":"media_cache_io","operation":operation,"kind":format!("{:?}",error.kind())}),
    );
    error_value
}
fn owned_key(name: &str) -> Option<&str> {
    let key = name.strip_suffix(".cache")?;
    (key.len() == 64
        && key
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
    .then_some(key)
}

/// Include every actual upstream header, not browser credentials or a Session capability.
pub fn key(source: &str, revision: &str, url: &str, headers: &HeaderMap) -> String {
    let mut hash = Sha256::new();
    for value in [source.as_bytes(), revision.as_bytes(), url.as_bytes()] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value);
    }
    let mut values: Vec<_> = headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_bytes()))
        .collect();
    values.sort_by(|left, right| left.0.cmp(right.0));
    for (name, value) in values {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value);
    }
    format!("{:x}", hash.finalize())
}
fn directives(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all("cache-control")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(',').map(|part| part.trim().to_ascii_lowercase()))
        .collect()
}
pub fn request_bypasses(headers: &HeaderMap) -> bool {
    // Fetch directly for explicit client freshness directives; never override no-store.
    headers.contains_key("cache-control") || headers.contains_key("pragma")
}
fn http_time(headers: &HeaderMap, name: &str) -> Option<u64> {
    let value = headers.get(name)?.to_str().ok()?;
    let timestamp = OffsetDateTime::parse(value, &Rfc2822)
        .ok()?
        .unix_timestamp();
    u64::try_from(timestamp).ok()
}
fn freshness(headers: &HeaderMap, at: u64) -> Option<u64> {
    if headers
        .get_all("cache-control")
        .iter()
        .any(|value| value.to_str().is_err())
    {
        return None;
    }
    let control = directives(headers);
    if control.iter().any(|part| {
        matches!(
            part.split('=').next().map(str::trim),
            Some("no-store" | "no-cache")
        )
    }) || headers.get_all("vary").iter().any(|v| {
        v.to_str()
            .is_ok_and(|v| v.split(',').any(|p| p.trim() == "*"))
    }) {
        return None;
    }
    let received_age = match headers.get("age") {
        Some(value) => value.to_str().ok()?.parse::<u64>().ok()?,
        None => 0,
    };
    let date = if headers.contains_key("date") {
        http_time(headers, "date")?
    } else {
        at
    };
    let age = received_age.max(at.saturating_sub(date));
    let maximums: Vec<_> = control
        .iter()
        .filter_map(|part| {
            let (name, value) = part.split_once('=')?;
            (name.trim() == "max-age").then_some(value.trim())
        })
        .collect();
    if maximums.len() > 1 || control.iter().any(|part| part.trim() == "max-age") {
        return None;
    }
    let maximum = if let Some(value) = maximums.first() {
        Some(value.trim_matches('"').parse::<u64>().ok()?)
    } else {
        None
    };
    if headers.contains_key("expires") && http_time(headers, "expires").is_none() {
        return None;
    }
    let lifetime = if let Some(value) = maximum {
        value.saturating_sub(age)
    } else if let Some(expires) = http_time(headers, "expires") {
        expires
            .saturating_sub(http_time(headers, "date").unwrap_or(at))
            .saturating_sub(age)
    } else {
        let modified = http_time(headers, "last-modified")?;
        if modified > date {
            return None;
        }
        ((date - modified) / 10).min(60).saturating_sub(age)
    };
    (lifetime > 0).then_some(lifetime)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    status: u16,
    body_len: u64,
    created: u64,
    expires: u64,
    headers: BTreeMap<String, String>,
    #[serde(default)]
    episode: Option<String>,
    #[serde(default)]
    resource: Option<String>,
}
impl Metadata {
    fn headers(&self) -> Option<HeaderMap> {
        let mut headers = HeaderMap::new();
        for (name, value) in &self.headers {
            if !HEADERS.contains(&name.as_str()) {
                return None;
            }
            headers.insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).ok()?,
                value.parse().ok()?,
            );
        }
        Some(headers)
    }
}
struct Entry {
    metadata: Metadata,
    size: u64,
    used: u64,
    readers: usize,
    retired: bool,
}
struct Reservation {
    bytes: u64,
    generation: u64,
    path: PathBuf,
    active: bool,
}
struct State {
    limit: u64,
    episode_mode: bool,
    ready: bool,
    episode_order: Vec<String>,
    generation: u64,
    sequence: u64,
    last_sweep: u64,
    initialized: bool,
    available: bool,
    directory_lock: Option<File>,
    entries: HashMap<String, Entry>,
    pending: HashMap<Uuid, Reservation>,
    last_error: Option<ApiError>,
}
impl State {
    fn used(&self) -> u64 {
        self.entries
            .values()
            .map(|e| e.size)
            .chain(self.pending.values().map(|p| p.bytes))
            .fold(0, u64::saturating_add)
    }
    fn status(&self) -> Value {
        let pending: u64 = self
            .entries
            .values()
            .filter(|e| e.retired)
            .map(|e| e.size)
            .chain(
                self.pending
                    .values()
                    .filter(|p| p.generation != self.generation || !p.active)
                    .map(|p| p.bytes),
            )
            .sum();
        json!({"max_bytes":self.limit,"used_bytes":self.used(),"pending_bytes":pending,
            "entry_count":self.entries.values().filter(|e|!e.retired).count(),"last_error":self.last_error})
    }
}
pub struct MediaCache {
    root: Option<PathBuf>,
    state: Mutex<State>,
    blob_lock: Mutex<()>,
    image_lock: Mutex<()>,
}
impl MediaCache {
    /// Store an application-owned cache value in the same budget as media bytes.
    pub(crate) fn put_blob(self: &Arc<Self>, key: &str, bytes: &[u8]) -> Result<(), ApiError> {
        self.put_blob_at(key, bytes, now())
    }

    pub(crate) fn put_blob_at(
        self: &Arc<Self>,
        key: &str,
        bytes: &[u8],
        created: u64,
    ) -> Result<(), ApiError> {
        let _guard = self
            .blob_lock
            .lock()
            .map_err(|_| ApiError::internal("Cache lock unavailable"))?;
        let at = now();
        if created > at || created.saturating_add(CACHE_TTL) <= at {
            return Err(ApiError::bad("Cache value has expired"));
        }
        let cache_key = format!("{:x}", Sha256::digest(key.as_bytes()));
        let owner = format!("blob-{cache_key}");
        {
            let mut state = self.state.lock().expect("cache lock");
            if !state.ready {
                return Err(ApiError::internal("Legacy cache cleanup is pending"));
            }
            if !self.prepare(&mut state) {
                return Err(state
                    .last_error
                    .clone()
                    .unwrap_or_else(|| ApiError::internal("Cache unavailable")));
            }
            self.prune_expired(&mut state);
            if state.entries.contains_key(&cache_key) && !self.remove(&mut state, &cache_key) {
                return Err(ApiError::internal("Cache value is in use"));
            }
        }
        let headers = HeaderMap::new();
        let mut writer = self
            .begin_episode_at(
                cache_key.clone(),
                200,
                &headers,
                &headers,
                Some(bytes.len() as u64),
                self.generation(),
                Some(&owner),
            )
            .ok_or_else(|| ApiError::internal("Cache capacity unavailable"))?;
        writer.metadata.created = created;
        writer.metadata.expires = created + CACHE_TTL;
        if key.starts_with("web:catalog:") {
            writer.metadata.resource = Some("web:catalog".to_owned());
        } else if key.starts_with("web:details:") {
            writer.metadata.resource = Some("web:details".to_owned());
        }
        if !writer.append(bytes) {
            return Err(ApiError::internal("Cache write failed"));
        }
        writer.finish();
        if self.get(&cache_key).is_none() {
            return Err(ApiError::internal("Cache publish failed"));
        }
        Ok(())
    }

    pub(crate) fn get_blob(self: &Arc<Self>, key: &str) -> Option<Vec<u8>> {
        let cache_key = format!("{:x}", Sha256::digest(key.as_bytes()));
        let mut reader = self.get(&cache_key)?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    pub(crate) fn delete_blob(&self, key: &str) -> Result<(), ApiError> {
        let _guard = self
            .blob_lock
            .lock()
            .map_err(|_| ApiError::internal("Cache lock unavailable"))?;
        let cache_key = format!("{:x}", Sha256::digest(key.as_bytes()));
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApiError::internal("Cache lock unavailable"))?;
        if !self.prepare(&mut state) {
            return Err(state
                .last_error
                .clone()
                .unwrap_or_else(|| ApiError::internal("Cache unavailable")));
        }
        if let Some(entry) = state.entries.get_mut(&cache_key) {
            entry.retired = true;
        }
        if !self.remove(&mut state, &cache_key)
            && state
                .entries
                .get(&cache_key)
                .is_some_and(|entry| entry.readers == 0)
        {
            return Err(state
                .last_error
                .clone()
                .unwrap_or_else(|| ApiError::internal("Cache deletion failed")));
        }
        Ok(())
    }

    pub(crate) fn web_rows(self: &Arc<Self>, store: &str) -> Result<Vec<Value>, ApiError> {
        let tag = format!("web:{store}");
        let keys = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| ApiError::internal("Cache lock unavailable"))?;
            if !self.prepare(&mut state) {
                return Err(state
                    .last_error
                    .clone()
                    .unwrap_or_else(|| ApiError::internal("Cache unavailable")));
            }
            state
                .entries
                .iter()
                .filter(|(_, entry)| entry.metadata.resource.as_deref() == Some(tag.as_str()))
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>()
        };
        let mut rows = Vec::new();
        for key in keys {
            if let Some(mut reader) = self.get(&key) {
                let mut bytes = Vec::new();
                if reader.read_to_end(&mut bytes).is_ok() {
                    if let Ok(row) = serde_json::from_slice(&bytes) {
                        rows.push(row);
                    }
                }
            }
        }
        Ok(rows)
    }

    pub fn new(root: PathBuf, limit: u64) -> Arc<Self> {
        Self::build(Some(root), limit, false)
    }
    pub fn for_episodes(root: PathBuf) -> Arc<Self> {
        Self::build(Some(root), 1024 * 1024 * 1024, true)
    }
    pub fn for_app(root: PathBuf) -> Arc<Self> {
        let cache = Self::for_episodes(root);
        cache.state.lock().expect("cache lock").ready = false;
        cache
    }
    pub(crate) fn enable_after_legacy_cleanup(&self) {
        self.state.lock().expect("cache lock").ready = true;
    }
    pub fn disabled() -> Arc<Self> {
        Self::build(None, 0, false)
    }
    fn build(root: Option<PathBuf>, limit: u64, episode_mode: bool) -> Arc<Self> {
        Arc::new(Self {
            root,
            blob_lock: Mutex::new(()),
            image_lock: Mutex::new(()),
            state: Mutex::new(State {
                limit,
                episode_mode,
                ready: true,
                episode_order: Vec::new(),
                generation: 0,
                sequence: 0,
                last_sweep: 0,
                initialized: false,
                available: false,
                directory_lock: None,
                entries: HashMap::new(),
                pending: HashMap::new(),
                last_error: None,
            }),
        })
    }
    pub fn register_images(&self, owner: &str, keys: &[String]) -> Result<(), ApiError> {
        let _ = (owner, keys);
        Ok(())
    }
    pub fn touch_image_owner(&self, owner: &str) -> Result<(), ApiError> {
        let _ = owner;
        Ok(())
    }
    pub fn cached_image(
        self: &Arc<Self>,
        key: &str,
    ) -> Result<Option<(Vec<u8>, String)>, ApiError> {
        let _guard = self
            .image_lock
            .lock()
            .map_err(|_| ApiError::internal("Image cache lock unavailable"))?;
        let Some(bytes) = self.get_blob(&format!("image:{key}")) else {
            return Ok(None);
        };
        let Some(split) = bytes.iter().position(|byte| *byte == 0) else {
            return Ok(None);
        };
        let Ok(mime) = std::str::from_utf8(&bytes[..split]) else {
            return Ok(None);
        };
        Ok(Some((bytes[split + 1..].to_vec(), mime.to_owned())))
    }
    pub fn save_image(
        self: &Arc<Self>,
        key: &str,
        body: &[u8],
        mime: &str,
    ) -> Result<(), ApiError> {
        let _guard = self
            .image_lock
            .lock()
            .map_err(|_| ApiError::internal("Image cache lock unavailable"))?;
        let mut blob = mime.as_bytes().to_vec();
        blob.push(0);
        blob.extend_from_slice(body);
        self.put_blob(&format!("image:{key}"), &blob)?;
        Ok(())
    }
    fn path(&self, key: &str) -> PathBuf {
        self.root
            .as_ref()
            .expect("cache root")
            .join(format!("{key}.cache"))
    }
    fn prepare(&self, state: &mut State) -> bool {
        if state.initialized {
            return state.available;
        }
        state.initialized = true;
        state.last_error = None;
        state.directory_lock = None;
        state.entries.clear();
        state.episode_order.clear();
        let Some(root) = &self.root else {
            return false;
        };
        let result = (|| -> std::io::Result<()> {
            fs::create_dir_all(root)?;
            let info = fs::symlink_metadata(root)?;
            if !info.is_dir() || info.file_type().is_symlink() {
                return Err(std::io::Error::other(
                    "cache directory is not an owned directory",
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
            }
            let directory_lock = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(root.join(".lock"))?;
            directory_lock.try_lock().map_err(std::io::Error::other)?;
            state.directory_lock = Some(directory_lock);
            let mut files = Vec::new();
            for item in fs::read_dir(root)? {
                let item = item?;
                let name = item.file_name().to_string_lossy().into_owned();
                if name.starts_with("pending-") {
                    if !item.file_type()?.is_dir() {
                        fs::remove_file(item.path())?;
                    }
                    continue;
                }
                let Some(key) = owned_key(&name) else {
                    continue;
                };
                let metadata = if item.file_type()?.is_file() {
                    read_metadata(&item.path()).ok()
                } else {
                    None
                };
                let at = now();
                if let Some((metadata, size)) = metadata
                    .map(|(mut m, size)| {
                        if m.episode.is_some() {
                            m.expires = m.expires.min(m.created.saturating_add(CACHE_TTL));
                        }
                        (m, size)
                    })
                    .filter(|(m, _)| m.created <= at && m.expires > at)
                {
                    let modified = item.metadata()?.modified().unwrap_or(UNIX_EPOCH);
                    files.push((modified, key.to_owned(), metadata, size));
                } else if !item.file_type()?.is_dir() {
                    fs::remove_file(item.path())?;
                }
            }
            files.sort_by_key(|row| row.0);
            for (_, key, mut metadata, size) in files {
                if state.episode_mode {
                    let owner = metadata
                        .episode
                        .get_or_insert_with(|| "legacy".to_owned())
                        .clone();
                    if !owner.starts_with("blob-") {
                        state.episode_order.retain(|existing| existing != &owner);
                        state.episode_order.push(owner);
                    }
                }
                state.sequence += 1;
                state.entries.insert(
                    key,
                    Entry {
                        metadata,
                        size,
                        used: state.sequence,
                        readers: 0,
                        retired: false,
                    },
                );
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                state.available = true;
                if self.make_room(state, 0) {
                    true
                } else {
                    state.available = false;
                    if state.last_error.is_none() {
                        state.last_error = Some(ApiError::internal("Cache exceeds the configured limit"));
                    }
                    false
                }
            }
            Err(error) => {
                state.last_error = Some(io_error("open", &error));
                false
            }
        }
    }
    fn remove(&self, state: &mut State, key: &str) -> bool {
        let Some(entry) = state.entries.get(key) else {
            return true;
        };
        if entry.readers > 0 {
            return false;
        }
        match fs::remove_file(self.path(key)) {
            Ok(()) => {
                state.entries.remove(key);
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                state.entries.remove(key);
                true
            }
            Err(error) => {
                state.last_error = Some(io_error("remove", &error));
                false
            }
        }
    }
    fn prune_expired(&self, state: &mut State) {
        let at = now();
        if state.last_sweep <= at && at - state.last_sweep < 3600 {
            return;
        }
        state.last_sweep = at;
        let expired: Vec<_> = state
            .entries
            .iter()
            .filter(|(_, entry)| entry.metadata.created > at || entry.metadata.expires <= at)
            .map(|(key, _)| key.clone())
            .collect();
        for key in expired {
            if let Some(entry) = state.entries.get_mut(&key) {
                entry.retired = true;
            }
            self.remove(state, &key);
        }
    }
    fn make_room(&self, state: &mut State, extra: u64) -> bool {
        let mut attempted: Vec<String> = Vec::new();
        while state
            .used()
            .checked_add(extra)
            .is_none_or(|n| n > state.limit)
        {
            let candidate = state
                .entries
                .iter()
                .filter(|(key, e)| e.readers == 0 && !attempted.contains(*key))
                .min_by_key(|(_, e)| (!e.retired, e.used))
                .map(|(key, _)| key.clone());
            let Some(key) = candidate else {
                return false;
            };
            attempted.push(key.clone());
            self.remove(state, &key);
        }
        true
    }
    pub fn status(&self) -> Value {
        self.status_checked(|| Ok(()))
            .expect("unconditional cache status")
    }
    pub fn status_checked(
        &self,
        mut check: impl FnMut() -> Result<(), ApiError>,
    ) -> Result<Value, ApiError> {
        let mut state = self.state.lock().expect("cache lock");
        check()?;
        if state.initialized && !state.available {
            state.initialized = false;
        }
        self.prepare(&mut state);
        if state.available {
            self.prune_expired(&mut state);
        }
        check()?;
        Ok(state.status())
    }
    pub fn configure(&self, limit: u64) {
        let mut state = self.state.lock().expect("cache lock");
        if state.episode_mode {
            return;
        }
        if limit < state.limit {
            state.generation += 1;
        }
        state.limit = limit;
        if state.initialized && state.available {
            if !self.make_room(&mut state, 0) {
                for entry in state.entries.values_mut() {
                    entry.retired = true;
                }
            }
        } else if state.initialized {
            state.initialized = false;
        }
    }
    #[cfg(test)]
    pub fn clear(&self) -> Result<Value, ApiError> {
        self.clear_checked(|| Ok(()))
    }
    pub fn clear_checked(
        &self,
        mut check: impl FnMut() -> Result<(), ApiError>,
    ) -> Result<Value, ApiError> {
        let mut state = self.state.lock().expect("cache lock");
        check()?;
        if !state.available {
            state.initialized = false;
        }
        if !self.prepare(&mut state) {
            return Err(state
                .last_error
                .clone()
                .unwrap_or_else(|| ApiError::internal("Media cache is unavailable")));
        }
        check()?;
        state.generation += 1;
        state.last_error = None;
        for entry in state.entries.values_mut() {
            entry.retired = true;
        }
        let keys: Vec<_> = state.entries.keys().cloned().collect();
        for key in keys {
            self.remove(&mut state, &key);
        }
        let orphans: Vec<_> = state
            .pending
            .iter()
            .filter(|(_, p)| !p.active)
            .map(|(id, p)| (*id, p.path.clone()))
            .collect();
        for (id, path) in orphans {
            match fs::remove_file(path) {
                Ok(()) => {
                    state.pending.remove(&id);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    state.pending.remove(&id);
                }
                Err(error) => state.last_error = Some(io_error("remove", &error)),
            }
        }
        if let Some(error) = &state.last_error {
            Err(error.clone())
        } else {
            Ok(state.status())
        }
    }
    pub fn get(self: &Arc<Self>, key: &str) -> Option<CachedRead> {
        owned_key(&format!("{key}.cache"))?;
        let mut state = self.state.lock().expect("cache lock");
        if state.limit == 0 || !self.prepare(&mut state) {
            return None;
        }
        let entry = state.entries.get(key)?;
        if entry.retired || entry.metadata.created > now() || entry.metadata.expires <= now() {
            if let Some(entry) = state.entries.get_mut(key) {
                entry.retired = true;
            }
            self.remove(&mut state, key);
            return None;
        }
        let metadata = entry.metadata.clone();
        let file = (|| -> std::io::Result<File> {
            let path = self.path(key);
            let info = fs::symlink_metadata(&path)?;
            if !info.is_file()
                || info.file_type().is_symlink()
                || info.len() != HEADER + metadata.body_len
            {
                return Err(std::io::Error::other("invalid cache file"));
            }
            #[cfg(not(windows))]
            let mut file = File::open(path)?;
            #[cfg(windows)]
            let mut file = {
                use std::os::windows::fs::OpenOptionsExt;
                // Touching LRU timestamps requires FILE_WRITE_ATTRIBUTES on Windows.
                // Keep the cached media bytes read-only.
                fs::OpenOptions::new()
                    .access_mode(0x80000000 | 0x100)
                    .open(path)?
            };
            file.seek(SeekFrom::Start(HEADER))?;
            file.set_times(fs::FileTimes::new().set_modified(SystemTime::now()))?;
            Ok(file)
        })();
        let file = match file {
            Ok(file) => file,
            Err(error) => {
                state.last_error = Some(io_error("read", &error));
                self.remove(&mut state, key);
                return None;
            }
        };
        let headers = metadata.headers()?;
        state.sequence += 1;
        let sequence = state.sequence;
        let entry = state.entries.get_mut(key)?;
        entry.readers += 1;
        entry.used = sequence;
        Some(CachedRead {
            cache: self.clone(),
            key: key.into(),
            file: Some(file),
            status: metadata.status,
            headers,
            length: metadata.body_len,
            remaining: metadata.body_len,
        })
    }
    /// Assemble a requested byte interval from complete chunks of the same representation.
    pub fn cached_sequence(
        self: &Arc<Self>,
        resource: &str,
        requested: Option<&str>,
    ) -> Option<CachedSequence> {
        #[derive(Clone)]
        struct Piece {
            key: String,
            start: u64,
            end: u64,
            total: u64,
            validator: Option<String>,
        }
        let pieces: Vec<Piece> = {
            let mut state = self.state.lock().ok()?;
            if !self.prepare(&mut state) {
                return None;
            }
            state
                .entries
                .iter()
                .filter_map(|(key, entry)| {
                    let m = &entry.metadata;
                    if entry.retired
                        || m.resource.as_deref() != Some(resource)
                        || m.expires <= now()
                    {
                        return None;
                    }
                    let (start, end, total) = if m.status == 200 {
                        (0, m.body_len, m.body_len)
                    } else {
                        let value = m.headers.get("content-range")?.strip_prefix("bytes ")?;
                        let (bounds, total) = value.split_once('/')?;
                        let (start, end) = bounds.split_once('-')?;
                        let start = start.parse::<u64>().ok()?;
                        let end = end.parse::<u64>().ok()?.checked_add(1)?;
                        let total = total.parse::<u64>().ok()?;
                        if end.checked_sub(start)? != m.body_len || end > total {
                            return None;
                        }
                        (start, end, total)
                    };
                    let validator = m
                        .headers
                        .get("etag")
                        .filter(|v| !v.starts_with("W/"))
                        .map(|v| format!("etag:{v}"))
                        .or_else(|| {
                            m.headers
                                .get("last-modified")
                                .map(|v| format!("modified:{v}"))
                        });
                    Some(Piece {
                        key: key.clone(),
                        start,
                        end,
                        total,
                        validator,
                    })
                })
                .collect()
        };
        let total = pieces.first()?.total;
        let (start, end) = match requested {
            None => (0, total),
            Some(value) => {
                let (from, to) = value.strip_prefix("bytes=")?.split_once('-')?;
                if from.is_empty() {
                    let suffix = to.parse::<u64>().ok()?;
                    (total.saturating_sub(suffix), total)
                } else {
                    let start = from.parse::<u64>().ok()?;
                    let end = if to.is_empty() {
                        total
                    } else {
                        to.parse::<u64>().ok()?.saturating_add(1).min(total)
                    };
                    (start, end)
                }
            }
        };
        if start >= end || end > total {
            return None;
        }
        let mut position = start;
        let mut selected = Vec::new();
        let mut validator: Option<String> = None;
        while position < end {
            let part = pieces
                .iter()
                .filter(|part| part.total == total && part.start <= position && part.end > position)
                .filter(|part| selected.is_empty() || part.validator == validator)
                .max_by_key(|part| part.end)?
                .clone();
            if selected.is_empty() {
                validator = part.validator.clone();
            }
            let stop = part.end.min(end);
            selected.push((part.clone(), position - part.start, stop - position));
            position = stop;
        }
        let mut readers = Vec::new();
        for (part, offset, length) in selected {
            let mut reader = self.get(&part.key)?;
            reader.slice(offset, length).ok()?;
            readers.push(reader);
        }
        let mut headers = readers.first()?.headers.clone();
        headers.remove("content-range");
        headers.insert("accept-ranges", "bytes".parse().ok()?);
        if requested.is_some() {
            headers.insert(
                "content-range",
                format!("bytes {start}-{}/{total}", end - 1).parse().ok()?,
            );
        }
        Some(CachedSequence {
            status: if requested.is_some() { 206 } else { 200 },
            headers,
            length: end - start,
            readers,
        })
    }
    #[cfg(test)]
    pub fn begin(
        self: &Arc<Self>,
        key: String,
        status: u16,
        upstream: &HeaderMap,
        headers: &HeaderMap,
        length: Option<u64>,
    ) -> Option<CacheWrite> {
        self.begin_at(key, status, upstream, headers, length, self.generation())
    }
    pub fn register_episode(&self, owner: &str) -> Result<(), ApiError> {
        let mut state = self.state.lock().expect("cache lock");
        if !state.episode_mode {
            return Ok(());
        }
        if !self.prepare(&mut state) {
            return Err(state
                .last_error
                .clone()
                .unwrap_or_else(|| ApiError::internal("Media cache is unavailable")));
        }
        self.prune_expired(&mut state);
        state.episode_order.retain(|item| item != owner);
        state.episode_order.push(owner.to_owned());
        let existing: Vec<_> = state
            .entries
            .iter()
            .filter(|(_, entry)| entry.metadata.episode.as_deref() == Some(owner))
            .map(|(key, _)| key.clone())
            .collect();
        for key in existing {
            if let Ok(file) = fs::OpenOptions::new().write(true).open(self.path(&key)) {
                let _ = file.set_times(fs::FileTimes::new().set_modified(SystemTime::now()));
            }
        }
        Ok(())
    }
    pub fn generation(&self) -> u64 {
        self.state.lock().expect("cache lock").generation
    }
    pub fn begin_at(
        self: &Arc<Self>,
        key: String,
        status: u16,
        upstream: &HeaderMap,
        headers: &HeaderMap,
        length: Option<u64>,
        expected_generation: u64,
    ) -> Option<CacheWrite> {
        self.begin_episode_at(
            key,
            status,
            upstream,
            headers,
            length,
            expected_generation,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn begin_episode_at(
        self: &Arc<Self>,
        key: String,
        status: u16,
        upstream: &HeaderMap,
        headers: &HeaderMap,
        length: Option<u64>,
        expected_generation: u64,
        episode: Option<&str>,
    ) -> Option<CacheWrite> {
        owned_key(&format!("{key}.cache"))?;
        if !matches!(status, 200 | 206) {
            return None;
        }
        let at = now();
        let ttl = if episode.is_some() {
            CACHE_TTL
        } else {
            freshness(upstream, at)?
        };
        let stored: BTreeMap<String, String> = HEADERS
            .iter()
            .filter_map(|name| {
                headers
                    .get(*name)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| ((*name).into(), v.into()))
            })
            .collect();
        let metadata = Metadata {
            status,
            body_len: 0,
            created: at,
            expires: at.saturating_add(ttl),
            headers: stored,
            episode: episode.map(str::to_owned),
            resource: None,
        };
        if serde_json::to_vec(&metadata).ok()?.len() > HEADER as usize - 4 {
            return None;
        }
        let mut state = self.state.lock().expect("cache lock");
        if !state.ready
            || state.generation != expected_generation
            || (state.episode_mode
                && episode.is_none_or(|owner| {
                    !owner.starts_with("blob-")
                        && !state.episode_order.iter().any(|item| item == owner)
                }))
        {
            return None;
        }
        if state.limit <= HEADER
            || length.is_some_and(|n| n > state.limit - HEADER)
            || !self.prepare(&mut state)
            || !self.make_room(&mut state, HEADER)
        {
            return None;
        }
        let mut file = match tempfile::Builder::new()
            .prefix("pending-")
            .tempfile_in(self.root.as_ref()?)
        {
            Ok(file) => file,
            Err(error) => {
                state.last_error = Some(io_error("create", &error));
                return None;
            }
        };
        if let Err(error) = file.as_file_mut().seek(SeekFrom::Start(HEADER)) {
            state.last_error = Some(io_error("write", &error));
            return None;
        }
        let id = Uuid::new_v4();
        let generation = state.generation;
        state.pending.insert(
            id,
            Reservation {
                bytes: HEADER,
                generation,
                path: file.path().into(),
                active: true,
            },
        );
        Some(CacheWrite {
            cache: self.clone(),
            id,
            key,
            file: Some(file),
            metadata,
            length,
            written: 0,
        })
    }
}
fn read_metadata(path: &Path) -> std::io::Result<(Metadata, u64)> {
    let mut file = File::open(path)?;
    let mut prefix = [0; 4];
    file.read_exact(&mut prefix)?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len == 0 || len > HEADER as usize - 4 {
        return Err(std::io::Error::other("invalid cache header"));
    }
    let mut bytes = vec![0; len];
    file.read_exact(&mut bytes)?;
    let metadata: Metadata = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    let size = file.metadata()?.len();
    if !matches!(metadata.status, 200 | 206)
        || metadata.body_len == 0
        || metadata.body_len.checked_add(HEADER) != Some(size)
        || metadata.headers().is_none()
    {
        return Err(std::io::Error::other("incomplete cache entry"));
    }
    Ok((metadata, size))
}

pub struct CachedSequence {
    pub status: u16,
    pub headers: HeaderMap,
    pub length: u64,
    pub readers: Vec<CachedRead>,
}
pub struct CachedRead {
    cache: Arc<MediaCache>,
    key: String,
    file: Option<File>,
    pub status: u16,
    pub headers: HeaderMap,
    pub length: u64,
    remaining: u64,
}
impl CachedRead {
    fn slice(&mut self, offset: u64, length: u64) -> std::io::Result<()> {
        if offset
            .checked_add(length)
            .is_none_or(|end| end > self.length)
        {
            return Err(std::io::Error::other("invalid cached interval"));
        }
        self.file
            .as_mut()
            .expect("cache reader")
            .seek(SeekFrom::Start(HEADER + offset))?;
        self.length = length;
        self.remaining = length;
        Ok(())
    }
}
impl Read for CachedRead {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = bytes
            .len()
            .min(self.remaining.try_into().unwrap_or(usize::MAX));
        let result = self
            .file
            .as_mut()
            .expect("cache reader")
            .read(&mut bytes[..count]);
        let result = result.and_then(|n| {
            if n == 0 && self.remaining > 0 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "cached media truncated",
                ))
            } else {
                Ok(n)
            }
        });
        let count = match result {
            Ok(count) => count,
            Err(error) => {
                let mut state = self.cache.state.lock().expect("cache lock");
                state.last_error = Some(io_error("read", &error));
                if let Some(entry) = state.entries.get_mut(&self.key) {
                    entry.retired = true;
                }
                return Err(error);
            }
        };
        self.remaining -= count as u64;
        Ok(count)
    }
}
impl Drop for CachedRead {
    fn drop(&mut self) {
        self.file.take();
        let mut state = self.cache.state.lock().expect("cache lock");
        if let Some(entry) = state.entries.get_mut(&self.key) {
            entry.readers = entry.readers.saturating_sub(1);
        }
        if state.entries.get(&self.key).is_some_and(|e| e.retired) {
            self.cache.remove(&mut state, &self.key);
        }
    }
}
pub struct CacheWrite {
    cache: Arc<MediaCache>,
    id: Uuid,
    key: String,
    file: Option<NamedTempFile>,
    metadata: Metadata,
    length: Option<u64>,
    written: u64,
}
impl CacheWrite {
    pub fn set_resource(&mut self, key: String) {
        self.metadata.resource = Some(key);
    }
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let mut state = self.cache.state.lock().expect("cache lock");
        let valid = state
            .pending
            .get(&self.id)
            .is_some_and(|p| p.generation == state.generation);
        if self.file.is_none() || !valid || !self.cache.make_room(&mut state, bytes.len() as u64) {
            return false;
        }
        if let Some(pending) = state.pending.get_mut(&self.id) {
            pending.bytes += bytes.len() as u64;
        }
        drop(state);
        match self.file.as_mut().expect("cache file").write_all(bytes) {
            Ok(()) => {
                self.written += bytes.len() as u64;
                true
            }
            Err(error) => {
                self.cache.state.lock().expect("cache lock").last_error =
                    Some(io_error("write", &error));
                false
            }
        }
    }
    pub fn finish(mut self) {
        if self.written == 0
            || self.length.is_some_and(|n| n != self.written)
            || self.metadata.expires <= now()
        {
            return;
        }
        self.metadata.body_len = self.written;
        let header = match serde_json::to_vec(&self.metadata) {
            Ok(bytes) if bytes.len() <= HEADER as usize - 4 => bytes,
            _ => return,
        };
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let written = (|| -> std::io::Result<()> {
            file.as_file_mut().seek(SeekFrom::Start(0))?;
            file.write_all(&(header.len() as u32).to_be_bytes())?;
            file.write_all(&header)?;
            file.flush()
        })();
        if let Err(error) = written {
            self.cache.state.lock().expect("cache lock").last_error =
                Some(io_error("publish", &error));
            return;
        }
        let mut state = self.cache.state.lock().expect("cache lock");
        let Some(reservation) = state.pending.get(&self.id) else {
            return;
        };
        if reservation.generation != state.generation
            || state.entries.contains_key(&self.key)
            || (state.episode_mode
                && self.metadata.episode.as_ref().is_none_or(|owner| {
                    !owner.starts_with("blob-") && !state.episode_order.contains(owner)
                }))
        {
            return;
        }
        let size = reservation.bytes;
        match self
            .file
            .take()
            .expect("cache file")
            .persist_noclobber(self.cache.path(&self.key))
        {
            Ok(_) => {
                state.pending.remove(&self.id);
                state.sequence += 1;
                let used = state.sequence;
                state.entries.insert(
                    self.key.clone(),
                    Entry {
                        metadata: self.metadata.clone(),
                        size,
                        used,
                        readers: 0,
                        retired: false,
                    },
                );
            }
            Err(error) => {
                state.last_error = Some(io_error("publish", &error.error));
                self.file = Some(error.file);
            }
        }
    }
}
impl Drop for CacheWrite {
    fn drop(&mut self) {
        let Some(file) = self.file.take() else {
            return;
        };
        let result = file.close();
        let mut state = self.cache.state.lock().expect("cache lock");
        match result {
            Ok(()) => {
                state.pending.remove(&self.id);
            }
            Err(error) => {
                if let Some(pending) = state.pending.get_mut(&self.id) {
                    pending.active = false;
                }
                state.last_error = Some(io_error("remove", &error));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("cache-control", "max-age=3600".parse().unwrap());
        headers.insert("content-type", "video/mp4".parse().unwrap());
        headers
    }
    fn cache(limit: u64) -> (tempfile::TempDir, Arc<MediaCache>) {
        let root = tempfile::tempdir().unwrap();
        let cache = MediaCache::new(root.path().join("media-cache"), limit);
        (root, cache)
    }
    fn id(value: &str) -> String {
        key("source", "revision", value, &HeaderMap::new())
    }
    fn put(cache: &Arc<MediaCache>, key: &str, bytes: &[u8]) {
        let headers = headers();
        let mut writer = cache
            .begin(
                key.into(),
                200,
                &headers,
                &headers,
                Some(bytes.len() as u64),
            )
            .unwrap();
        assert!(writer.append(bytes));
        writer.finish();
    }
    fn body(cache: &Arc<MediaCache>, key: &str) -> Option<Vec<u8>> {
        let mut entry = cache.get(key)?;
        let mut body = Vec::new();
        entry.read_to_end(&mut body).unwrap();
        Some(body)
    }
    #[test]
    fn cached_episode_byte_ranges_are_assembled_only_with_matching_validators() {
        let root = tempfile::tempdir().unwrap();
        let cache = MediaCache::for_episodes(root.path().join("media-cache"));
        cache.register_episode("episode").unwrap();
        let put = |key: &str, range: &str, etag: &str, data: &[u8]| {
            let mut headers = headers();
            headers.insert("content-range", range.parse().unwrap());
            headers.insert("etag", etag.parse().unwrap());
            let mut writer = cache
                .begin_episode_at(
                    id(key),
                    206,
                    &headers,
                    &headers,
                    Some(data.len() as u64),
                    cache.generation(),
                    Some("episode"),
                )
                .unwrap();
            writer.set_resource("resource".into());
            assert!(writer.append(data));
            writer.finish();
        };
        put("part-a", "bytes 0-3/8", "\"version\"", b"abcd");
        assert!(
            cache
                .cached_sequence("resource", Some("bytes=2-"))
                .is_none()
        );
        put("part-b", "bytes 4-7/8", "\"version\"", b"efgh");
        let mut sequence = cache
            .cached_sequence("resource", Some("bytes=2-6"))
            .unwrap();
        assert_eq!(sequence.status, 206);
        assert_eq!(sequence.headers["content-range"], "bytes 2-6/8");
        let mut bytes = Vec::new();
        for reader in &mut sequence.readers {
            reader.read_to_end(&mut bytes).unwrap();
        }
        assert_eq!(bytes, b"cdefg");
        let mut full = cache.cached_sequence("resource", None).unwrap();
        let mut bytes = Vec::new();
        for reader in &mut full.readers {
            reader.read_to_end(&mut bytes).unwrap();
        }
        assert_eq!(bytes, b"abcdefgh");
        drop(sequence);
        drop(full);
        cache.clear().unwrap();
        put("part-a", "bytes 0-3/8", "\"old\"", b"abcd");
        put("part-b", "bytes 4-7/8", "\"new\"", b"efgh");
        assert!(cache.cached_sequence("resource", None).is_none());
    }

    #[test]
    fn media_cache_retains_more_than_fifteen_episodes_across_restart() {
        let root = tempfile::tempdir().unwrap();
        let cache = MediaCache::for_episodes(root.path().join("media-cache"));
        let headers = headers();
        let put = |owner: &str| {
            cache.register_episode(owner).unwrap();
            let key = id(owner);
            let mut writer = cache
                .begin_episode_at(
                    key,
                    200,
                    &headers,
                    &headers,
                    Some(8),
                    cache.generation(),
                    Some(owner),
                )
                .unwrap();
            assert!(writer.append(&[7; 8]));
            writer.finish();
        };
        for n in 0..15 {
            put(&format!("episode-{n}"));
        }
        cache.register_episode("episode-0").unwrap();
        put("episode-15");
        assert!(cache.get(&id("episode-0")).is_some());
        assert!(cache.get(&id("episode-1")).is_some());
        assert!(cache.get(&id("episode-15")).is_some());
        drop(cache);
        let cache = MediaCache::for_episodes(root.path().join("media-cache"));
        cache.register_episode("episode-16").unwrap();
        assert!(cache.get(&id("episode-0")).is_some());
        assert!(cache.get(&id("episode-2")).is_some());
    }

    #[test]
    fn episode_cache_evicts_by_bytes_and_expires_after_seven_days() {
        let root = tempfile::tempdir().unwrap();
        let cache = MediaCache::build(
            Some(root.path().join("media-cache")),
            2 * (HEADER + 8),
            true,
        );
        let headers = headers();
        for owner in ["first", "second", "third"] {
            cache.register_episode(owner).unwrap();
            let mut writer = cache
                .begin_episode_at(
                    id(owner),
                    200,
                    &headers,
                    &headers,
                    Some(8),
                    cache.generation(),
                    Some(owner),
                )
                .unwrap();
            assert_eq!(writer.metadata.expires - writer.metadata.created, CACHE_TTL);
            assert!(writer.append(&[7; 8]));
            writer.finish();
        }
        assert!(cache.get(&id("first")).is_none());
        assert!(cache.get(&id("second")).is_some());
        assert!(cache.get(&id("third")).is_some());
        assert!(cache.status()["used_bytes"].as_u64().unwrap() <= 2 * (HEADER + 8));
    }

    #[test]
    fn detail_images_retain_more_than_200_titles_across_restart() {
        let (root, cache) = cache(4 * 1024 * 1024);
        for id in 0..200 {
            let image = format!("image-{id}");
            cache
                .register_images(&format!("title-{id}"), std::slice::from_ref(&image))
                .unwrap();
            cache
                .save_image(&image, &[id as u8; 8], "image/png")
                .unwrap();
        }
        cache.touch_image_owner("title-0").unwrap();
        cache
            .register_images("title-200", &["image-200".into()])
            .unwrap();
        assert!(cache.cached_image("image-0").unwrap().is_some());
        assert!(cache.cached_image("image-1").unwrap().is_some());
        drop(cache);
        let restarted = MediaCache::new(root.path().join("media-cache"), 4 * 1024 * 1024);
        assert_eq!(
            restarted.cached_image("image-0").unwrap().unwrap().0,
            vec![0; 8]
        );
    }

    #[test]
    fn detail_images_expire_after_seven_days() {
        let (_, cache) = cache(1024 * 1024);
        cache.register_images("title", &["image".into()]).unwrap();
        cache.save_image("image", b"image", "image/png").unwrap();
        let blob_key = format!("{:x}", Sha256::digest(b"image:image"));
        cache
            .state
            .lock()
            .unwrap()
            .entries
            .get_mut(&blob_key)
            .unwrap()
            .metadata
            .expires = now();
        assert!(cache.cached_image("image").unwrap().is_none());
    }

    #[test]
    fn concurrent_writers_share_the_media_budget() {
        use std::sync::Barrier;
        let root = tempfile::tempdir().unwrap();
        let budget = 4 * (HEADER + 128);
        let cache = MediaCache::build(Some(root.path().join("media-cache")), budget, true);
        let barrier = Arc::new(Barrier::new(12));
        let threads: Vec<_> = (0..12)
            .map(|n| {
                let cache = cache.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let owner = format!("episode-{n}");
                    cache.register_episode(&owner).unwrap();
                    barrier.wait();
                    let headers = headers();
                    if let Some(mut writer) = cache.begin_episode_at(
                        id(&owner),
                        200,
                        &headers,
                        &headers,
                        Some(128),
                        cache.generation(),
                        Some(&owner),
                    ) {
                        if writer.append(&[1; 128]) {
                            writer.finish();
                        }
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let status = cache.status();
        assert!(status["used_bytes"].as_u64().unwrap() <= budget);
        assert!(status["entry_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn source_blobs_and_playback_share_the_same_budget() {
        let root = tempfile::tempdir().unwrap();
        let budget = 2 * (HEADER + 128);
        let cache = MediaCache::build(Some(root.path().join("media-cache")), budget, true);
        let headers = headers();
        cache.register_episode("playing").unwrap();
        let mut writer = cache
            .begin_episode_at(
                id("playing"),
                200,
                &headers,
                &headers,
                Some(128),
                cache.generation(),
                Some("playing"),
            )
            .unwrap();
        assert!(writer.append(&[7; 128]));
        writer.finish();
        let mut reader = cache.get(&id("playing")).unwrap();
        cache.put_blob("source:first", &[1; 128]).unwrap();
        cache.put_blob("source:second", &[2; 128]).unwrap();
        assert!(cache.get_blob("source:first").is_none());
        assert_eq!(cache.get_blob("source:second"), Some(vec![2; 128]));
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![7; 128]);
        assert!(cache.status()["used_bytes"].as_u64().unwrap() <= budget);
    }

    #[test]
    fn image_source_and_playback_writes_share_the_same_budget() {
        use std::sync::Barrier;
        let root = tempfile::tempdir().unwrap();
        let budget = 2 * (HEADER + 256);
        let cache = MediaCache::build(Some(root.path().join("media-cache")), budget, true);
        let headers = headers();
        cache.register_episode("playing").unwrap();
        let mut writer = cache
            .begin_episode_at(
                id("playing"),
                200,
                &headers,
                &headers,
                Some(128),
                cache.generation(),
                Some("playing"),
            )
            .unwrap();
        assert!(writer.append(&[7; 128]));
        writer.finish();
        let mut reader = cache.get(&id("playing")).unwrap();
        cache.register_images("title", &["poster".into()]).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let source = {
            let cache = cache.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                cache.put_blob("source:example", &[1; 128])
            })
        };
        barrier.wait();
        cache.save_image("poster", &[2; 128], "image/png").unwrap();
        source.join().unwrap().unwrap();
        assert!(cache.status()["used_bytes"].as_u64().unwrap() <= budget);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![7; 128]);
    }

    #[test]
    fn web_rows_share_budget_and_survive_restart() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("media-cache");
        let cache = MediaCache::build(Some(dir.clone()), 3 * (HEADER + 256), true);
        cache
            .put_blob("web:catalog:one", br#"{"key":"one","savedAt":1}"#)
            .unwrap();
        cache
            .put_blob("web:details:two", br#"{"key":"two","savedAt":1}"#)
            .unwrap();
        cache.put_blob("source:three", b"snapshot").unwrap();
        assert_eq!(cache.web_rows("details").unwrap()[0]["key"], "two");
        assert!(cache.status()["used_bytes"].as_u64().unwrap() <= 3 * (HEADER + 256));
        drop(cache);
        let cache = MediaCache::build(Some(dir), 3 * (HEADER + 256), true);
        assert_eq!(cache.web_rows("catalog").unwrap()[0]["key"], "one");
        cache
            .put_blob("web:catalog:one", br#"{"key":"updated","savedAt":2}"#)
            .unwrap();
        assert_eq!(cache.web_rows("catalog").unwrap()[0]["key"], "updated");
        cache.delete_blob("web:catalog:one").unwrap();
        assert!(cache.get_blob("web:catalog:one").is_none());
        assert!(cache.web_rows("catalog").unwrap().is_empty());
    }

    #[test]
    fn web_source_image_and_active_playback_share_one_budget() {
        use std::sync::Barrier;
        let root = tempfile::tempdir().unwrap();
        let budget = 2 * (HEADER + 512);
        let cache = MediaCache::build(Some(root.path().join("media-cache")), budget, true);
        let headers = headers();
        cache.register_episode("playing").unwrap();
        let mut writer = cache
            .begin_episode_at(
                id("playing"),
                200,
                &headers,
                &headers,
                Some(128),
                cache.generation(),
                Some("playing"),
            )
            .unwrap();
        assert!(writer.append(&[7; 128]));
        writer.finish();
        let mut reader = cache.get(&id("playing")).unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let source = {
            let cache = cache.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                cache.put_blob("source:sample", &[1; 128])
            })
        };
        let web = {
            let cache = cache.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                cache.put_blob("web:details:sample", br#"{"key":"sample","savedAt":1}"#)
            })
        };
        barrier.wait();
        cache.save_image("poster", &[2; 128], "image/png").unwrap();
        source.join().unwrap().unwrap();
        web.join().unwrap().unwrap();
        assert!(cache.status()["used_bytes"].as_u64().unwrap() <= budget);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![7; 128]);
    }

    #[test]
    fn desktop_cache_budget_is_one_gibibyte() {
        let root = tempfile::tempdir().unwrap();
        let cache = MediaCache::for_episodes(root.path().join("media-cache"));
        assert_eq!(cache.status()["max_bytes"], 1024_u64 * 1024 * 1024);
    }

    #[test]
    fn desktop_does_not_write_until_legacy_web_cache_is_cleared() {
        let root = tempfile::tempdir().unwrap();
        let cache = MediaCache::for_app(root.path().join("cache-v2"));
        assert!(cache.put_blob("source:test", b"value").is_err());
        cache.register_episode("playing").unwrap();
        let headers = headers();
        assert!(
            cache
                .begin_episode_at(
                    id("playing"),
                    200,
                    &headers,
                    &headers,
                    Some(3),
                    cache.generation(),
                    Some("playing")
                )
                .is_none()
        );
        cache.enable_after_legacy_cleanup();
        cache.put_blob("source:test", b"value").unwrap();
        assert_eq!(cache.get_blob("source:test"), Some(b"value".to_vec()));
    }

    #[test]
    fn clearing_episode_cache_keeps_active_reader_and_releases_it_afterwards() {
        let root = tempfile::tempdir().unwrap();
        let cache = MediaCache::build(Some(root.path().join("media-cache")), HEADER + 32, true);
        let headers = headers();
        cache.register_episode("playing").unwrap();
        let mut writer = cache
            .begin_episode_at(
                id("playing"),
                200,
                &headers,
                &headers,
                Some(16),
                cache.generation(),
                Some("playing"),
            )
            .unwrap();
        assert!(writer.append(&[7; 16]));
        writer.finish();
        let mut reader = cache.get(&id("playing")).unwrap();
        let cleared = cache.clear().unwrap();
        assert_eq!(cleared["pending_bytes"], HEADER + 16);
        assert!(cache.get(&id("playing")).is_none());
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![7; 16]);
        drop(reader);
        assert_eq!(cache.status()["used_bytes"], 0);
    }

    #[test]
    fn t24_media_cache_complete_range_keys_and_restart() {
        let (root, cache) = cache(1024 * 1024);
        let k = id("https://example.test/movie?token=PRIVATE");
        let data = vec![42; 1000];
        put(&cache, &k, &data);
        assert_eq!(body(&cache, &k), Some(data.clone()));
        let path = cache.path(&k);
        let disk = fs::read(&path).unwrap();
        assert!(!String::from_utf8_lossy(&disk[..HEADER as usize]).contains("PRIVATE"));
        assert_eq!(cache.status()["used_bytes"], HEADER + 1000);
        drop(cache);
        let restarted = MediaCache::new(root.path().join("media-cache"), 1024 * 1024);
        assert_eq!(body(&restarted, &k), Some(data));
        let mut a = HeaderMap::new();
        a.insert("range", "bytes=0-99".parse().unwrap());
        let original = key("s", "r", "https://example.test/a", &a);
        for (field, value) in [
            ("range", "bytes=100-199"),
            ("cookie", "PRIVATE"),
            ("authorization", "Bearer PRIVATE"),
        ] {
            let mut b = a.clone();
            b.insert(
                reqwest::header::HeaderName::from_bytes(field.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
            assert_ne!(original, key("s", "r", "https://example.test/a", &b));
        }
        assert_ne!(original, key("another", "r", "https://example.test/a", &a));
        assert_ne!(original, key("s", "changed", "https://example.test/a", &a));
        let mut b = HeaderMap::new();
        b.append("cookie", "one=1".parse().unwrap());
        b.append("cookie", "one=2".parse().unwrap());
        let first = key("s", "r", "u", &b);
        let mut b = HeaderMap::new();
        b.append("cookie", "one=2".parse().unwrap());
        b.append("cookie", "one=1".parse().unwrap());
        assert_ne!(first, key("s", "r", "u", &b));
    }
    #[test]
    fn t24_media_cache_lru_limit_and_partial_reservations() {
        let (_root, cache) = cache((HEADER + 64) * 2);
        let (a, b, c) = (id("a"), id("b"), id("c"));
        put(&cache, &a, &[1; 64]);
        put(&cache, &b, &[2; 64]);
        assert!(body(&cache, &a).is_some());
        put(&cache, &c, &[3; 64]);
        assert!(body(&cache, &b).is_none());
        assert_eq!(body(&cache, &a), Some(vec![1; 64]));
        assert!(cache.status()["used_bytes"].as_u64().unwrap() <= (HEADER + 64) * 2);
        cache.configure(HEADER + 10);
        cache.clear().unwrap();
        let h = headers();
        let mut writer = cache.begin(id("partial"), 200, &h, &h, None).unwrap();
        assert!(writer.append(&[0; 8]));
        assert_eq!(cache.status()["used_bytes"], HEADER + 8);
        assert!(!writer.append(&[0; 3]));
        drop(writer);
        assert_eq!(cache.status()["used_bytes"], 0);
        let mut writer = cache.begin(id("short"), 200, &h, &h, Some(10)).unwrap();
        assert!(writer.append(&[0; 9]));
        writer.finish();
        assert_eq!(cache.status()["entry_count"], 0);
    }
    #[test]
    fn t24_media_cache_clear_and_lower_limit_barrier_keeps_readers() {
        let (_root, cache) = cache(1024 * 1024);
        let k = id("held");
        put(&cache, &k, &[7; 64]);
        assert_eq!(
            cache
                .clear_checked(|| Err(ApiError::cancelled()))
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        assert_eq!(cache.status()["entry_count"], 1);
        let mut reader = cache.get(&k).unwrap();
        let h = headers();
        let mut writer = cache.begin(id("old-inflight"), 200, &h, &h, None).unwrap();
        assert!(writer.append(&[1; 8]));
        let status = cache.clear().unwrap();
        assert_eq!(status["entry_count"], 0);
        assert_eq!(status["pending_bytes"], (HEADER + 64) + (HEADER + 8));
        assert!(cache.get(&k).is_none());
        assert!(!writer.append(&[1]));
        writer.finish();
        let mut data = Vec::new();
        reader.read_to_end(&mut data).unwrap();
        assert_eq!(data, vec![7; 64]);
        drop(reader);
        assert_eq!(cache.status()["used_bytes"], 0);
        put(&cache, &k, &[7; 64]);
        let reader = cache.get(&k).unwrap();
        cache.configure(0);
        assert_eq!(cache.status()["max_bytes"], 0);
        assert_eq!(cache.status()["pending_bytes"], HEADER + 64);
        assert!(cache.get(&k).is_none());
        drop(reader);
        assert_eq!(cache.status()["used_bytes"], 0);
    }
    #[test]
    fn t24_media_cache_http_policy_and_recovery() {
        let (root, cache) = cache(1024 * 1024);
        let at = now();
        let mut h = headers();
        assert_eq!(freshness(&HeaderMap::new(), at), None);
        let mut heuristic = HeaderMap::new();
        heuristic.insert(
            "last-modified",
            OffsetDateTime::from_unix_timestamp(at as i64 - 1000)
                .unwrap()
                .format(&Rfc2822)
                .unwrap()
                .parse()
                .unwrap(),
        );
        assert_eq!(freshness(&heuristic, at), Some(60));

        for directive in [
            "no-store",
            "no-cache",
            "private, no-cache=\"set-cookie\"",
            "max-age=0",
        ] {
            h.insert("cache-control", directive.parse().unwrap());
            assert_eq!(freshness(&h, at), None);
        }
        for directive in ["no-store", "no-cache", "max-age=0"] {
            h.insert("cache-control", directive.parse().unwrap());
            assert!(request_bypasses(&h));
        }
        h = headers();
        h.insert("age", "3599".parse().unwrap());
        assert_eq!(freshness(&h, at), Some(1));
        h.insert("vary", "*".parse().unwrap());
        assert_eq!(freshness(&h, at), None);
        cache.status();
        let path = root.path().join("media-cache");
        fs::write(path.join("pending-abandoned"), b"partial").unwrap();
        fs::write(path.join(format!("{}.cache", id("corrupt"))), b"invalid").unwrap();
        let good = id("valid");
        put(&cache, &good, &[1; 64]);
        let future = id("future");
        put(&cache, &future, &[2; 64]);
        let file = cache.path(&future);
        let (mut meta, _) = read_metadata(&file).unwrap();
        meta.created = at + 86400;
        meta.expires = at + 172800;
        let encoded = serde_json::to_vec(&meta).unwrap();
        let mut out = fs::OpenOptions::new().write(true).open(file).unwrap();
        out.write_all(&(encoded.len() as u32).to_be_bytes())
            .unwrap();
        out.write_all(&encoded).unwrap();
        drop(out);
        drop(cache);
        let recovered = MediaCache::new(path.clone(), 1024 * 1024);
        assert_eq!(body(&recovered, &good), Some(vec![1; 64]));
        assert!(recovered.get(&future).is_none());
        assert!(!path.join("pending-abandoned").exists());
        assert_eq!(recovered.status()["entry_count"], 1);
        let blocked = root.path().join("not-a-directory");
        fs::write(&blocked, b"keep").unwrap();
        let unavailable = MediaCache::new(blocked, 1024);
        assert!(!unavailable.status()["last_error"].is_null());
        assert!(unavailable.clear().is_err());
    }
    #[test]
    fn t24_media_cache_directory_ownership_and_corrupt_read() {
        let (root, cache) = cache(1024 * 1024);
        let k = id("good");
        put(&cache, &k, &[3; 64]);
        let concurrent = MediaCache::new(root.path().join("media-cache"), 1024 * 1024);
        assert!(!concurrent.status()["last_error"].is_null());
        assert!(concurrent.clear().is_err());
        fs::OpenOptions::new()
            .write(true)
            .open(cache.path(&k))
            .unwrap()
            .set_len(HEADER + 1)
            .unwrap();
        assert!(cache.get(&k).is_none());
        assert_eq!(cache.status()["entry_count"], 0);
        assert!(!cache.status()["last_error"].is_null());
    }
}
