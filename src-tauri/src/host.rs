use crate::{
    contracts::{ApiError, COMMANDS, Contracts, envelope, wire_size},
    gateway::Gateway,
    history::History,
    matching::{self, MatchStore},
    media_cache::MediaCache,
    requests::{Clock, Job, MonotonicClock, Requests},
    resolution::{self, ResolutionStore},
    settings::SettingsFile,
    source_dispatch::{SourceDispatch, SourceKind, SourceSnapshot, identity},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[path = "maccms.rs"]
pub mod maccms;
#[cfg(test)]
#[path = "../../tests/fixtures/observation_redaction.rs"]
mod observation_redaction;
#[path = "stremio.rs"]
pub mod stremio;

struct Committed {
    settings: Value,
    sources: SourceDispatch,
    gateway: Option<Gateway>,
    maccms: std::collections::HashMap<String, maccms::Catalog>,
    stremio: std::collections::HashMap<String, stremio::Catalog>,
    metadata_epoch: u64,
    metadata_requests: std::collections::HashMap<String, String>,
    matches: MatchStore,
    resolutions: ResolutionStore,
    current_resolution: Option<String>,
    resolve_sequence: u64,
    history: History,
    resolution_contexts: std::collections::HashMap<String, Value>,
    session_bindings: std::collections::HashMap<String, SessionBinding>,
}

/// The identity a playback Session confirmed when it opened; it outlives match and candidate GC.
struct SessionBinding {
    context: Value,
    playback_epoch: Option<u64>,
}

pub struct Host {
    pub host_epoch: String,
    pub requests: Arc<Requests>,
    pub clock: Arc<dyn Clock>,
    store: SettingsFile,
    media_cache: Arc<MediaCache>,
    committed: Mutex<Committed>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub maximized: bool,
}

pub struct Call {
    command: String,
    args: Value,
    job: Option<Job>,
    metadata_epoch: Option<u64>,
    match_sequence: Option<u64>,
    resolve_sequence: Option<u64>,
}

impl Host {
    pub fn open(path: PathBuf) -> Result<Arc<Self>, ApiError> {
        Self::with_clock(path, Arc::new(MonotonicClock::default()))
    }

    pub fn with_clock(path: PathBuf, clock: Arc<dyn Clock>) -> Result<Arc<Self>, ApiError> {
        let history_path = path.with_file_name("yingxiang-video.db");
        let cache_path = path.with_file_name("media-cache");
        let (store, mut settings) = SettingsFile::load(path)?;
        let mut sources = SourceDispatch::default();
        sources.replace_configs(
            settings["source_configs"]
                .as_array()
                .expect("Validated source configs"),
        );
        let original = settings.clone();
        if let Some(key) = original["last_source_key"].as_str()
            && let Ok(source) = sources.source(key, None)
        {
            if source["kind"] == "stremio" && settings["last_metadata_config_id"].is_null() {
                settings["last_metadata_config_id"] = source["origin"]["config_id"].clone();
            } else if source["kind"] != "stremio" && settings["last_playback_source_key"].is_null()
            {
                settings["last_playback_source_key"] = json!(key);
            }
        }
        if settings != original {
            store.save(&settings)?;
        }
        Ok(Arc::new(Self {
            host_epoch: Uuid::new_v4().to_string(),
            requests: Arc::default(),
            clock,
            store,
            media_cache: MediaCache::new(cache_path, crate::media_cache::limit(&settings)),
            committed: Mutex::new(Committed {
                settings,
                sources,
                gateway: None,
                maccms: Default::default(),
                stremio: Default::default(),
                metadata_epoch: 0,
                metadata_requests: Default::default(),
                matches: MatchStore::default(),
                resolutions: ResolutionStore::default(),
                current_resolution: None,
                history: History::new(history_path),
                resolution_contexts: Default::default(),
                session_bindings: Default::default(),
                resolve_sequence: 0,
            }),
        }))
    }

    pub fn error(&self, args: &Value, error: ApiError) -> Value {
        envelope(&args["request_id"], &self.host_epoch, Err(error))
    }

    // Admission happens on receipt, before queuing work on the blocking executor.
    pub fn admit(&self, command: &str, args: Value, authorized: bool) -> Result<Call, Value> {
        let received_ms = self.clock.now_ms();
        let prepare = || -> Result<Option<Job>, ApiError> {
            if !authorized {
                return Err(ApiError::new(
                    "UNAUTHORIZED",
                    "Command requires the packaged main WebView",
                ));
            }
            if !COMMANDS.contains(&command) {
                return Err(ApiError::bad("Unknown command"));
            }
            wire_size(&args, 1024 * 1024)?;
            Contracts::get().validate(&format!("Args_{command}"), &args)?;
            if command == "cancel_request" {
                return Ok(None);
            }
            let operation = self.operation(command, &args["input"]);
            self.requests
                .begin_received(
                    args["request_id"].as_str().expect("Validated request id"),
                    &operation,
                    self.clock.clone(),
                    received_ms,
                )
                .map(Some)
        };
        match prepare() {
            Ok(job) => {
                let mut committed = self.committed.lock().expect("Host commit lock");
                let key = match command {
                    "get_home" | "search_catalog" => Some(&args["input"]),
                    "get_details" => Some(&args["input"]["ref"]),
                    _ => None,
                };
                let metadata_epoch = key
                    .filter(|key| {
                        committed
                            .sources
                            .source(key["source_id"].as_str().unwrap_or(""), None)
                            .is_ok_and(|source| source["kind"] == "stremio")
                    })
                    .map(|key| {
                        if let Some(job) = &job {
                            job.bind_source(key["source_id"].as_str().expect("Source id"));
                        }
                        committed.metadata_requests.insert(
                            args["request_id"].as_str().expect("Request UUID").into(),
                            key["source_id"].as_str().expect("Source id").into(),
                        );
                        committed.metadata_epoch
                    });
                let match_sequence =
                    (command == "match_playback").then(|| committed.matches.begin(&args["input"]));
                if matches!(command, "match_playback" | "resolve_episode") {
                    committed.resolve_sequence += 1;
                }
                let resolve_sequence =
                    (command == "resolve_episode").then_some(committed.resolve_sequence);
                Ok(Call {
                    command: command.into(),
                    args,
                    job,
                    metadata_epoch,
                    match_sequence,
                    resolve_sequence,
                })
            }
            Err(error) => Err(self.error(&args, error)),
        }
    }

    pub fn invoke(&self, command: &str, args: Value, authorized: bool) -> Value {
        match self.admit(command, args, authorized) {
            Ok(call) => self.execute(call),
            Err(error) => error,
        }
    }

    pub fn execute(&self, call: Call) -> Value {
        let mut error_details = None;
        let mut result = self.execute_inner(&call, &mut error_details);
        if matches!(
            call.command.as_str(),
            "match_playback" | "resolve_episode" | "open_playback"
        ) {
            let valid = call
                .job
                .as_ref()
                .expect("Playback job")
                .check()
                .and_then(|_| {
                    if let Some(sequence) = call.match_sequence {
                        self.committed
                            .lock()
                            .expect("Host commit lock")
                            .matches
                            .check(sequence)?;
                    }
                    if call.resolve_sequence.is_some_and(|sequence| {
                        sequence
                            != self
                                .committed
                                .lock()
                                .expect("Host commit lock")
                                .resolve_sequence
                    }) {
                        return Err(ApiError::cancelled());
                    }
                    Ok(())
                });
            if let Err(error) = valid {
                if let Ok(value) = &result {
                    let mut committed = self.committed.lock().expect("Host commit lock");
                    if call.command == "open_playback" {
                        if let Some(gateway) = &committed.gateway {
                            gateway.close(value["session_id"].as_str().expect("Session ID"));
                        }
                    } else if call.command == "resolve_episode" {
                        if let Some(id) = value["id"].as_str() {
                            committed.resolutions.release(id);
                            if committed.current_resolution.as_deref() == Some(id) {
                                committed.current_resolution = None;
                            }
                        }
                    } else if let Some(id) = value["context"]["match_id"].as_str() {
                        committed.matches.release(id);
                    }
                }
                error_details = None;
                result = Err(error);
            }
        }
        if let Some(epoch) = call.metadata_epoch {
            let mut committed = self.committed.lock().expect("Host commit lock");
            committed
                .metadata_requests
                .remove(call.args["request_id"].as_str().expect("Request UUID"));
            if epoch != committed.metadata_epoch {
                result = Err(ApiError::cancelled());
            } else if let Err(error) = call.job.as_ref().expect("Metadata job").check() {
                result = Err(error);
            }
        }
        let unsafe_target = result
            .as_ref()
            .err()
            .is_some_and(|error| error.message.starts_with("unsafe_target:"));
        let mut response = envelope(&call.args["request_id"], &self.host_epoch, result);
        if let Some(details) = error_details {
            response["error"]["details"] = details;
        }
        if unsafe_target && response["error"]["details"].is_null() {
            response["error"]["details"] =
                json!({"reason": "unsafe_target", "stage": "public_http"});
        }
        if let Err(error) = wire_size(&response, 8 * 1024 * 1024) {
            return self.error(&call.args, error);
        }
        if Contracts::get()
            .validate(&format!("Result_{}", call.command), &response)
            .is_err()
        {
            return self.error(
                &call.args,
                ApiError::internal("Host result violates the working protocol"),
            );
        }
        response
    }

    fn operation(&self, command: &str, input: &Value) -> String {
        match command {
            "runtime_status" => "health".into(),
            "runtime_prepare" | "get_resource_status" | "match_playback" => command.into(),
            "resolve_episode" => "resolve".into(),
            "sync_catalog" => if input["kind"] == "cloudstream" {
                "default_metadata"
            } else {
                "http_source_sync"
            }
            .into(),
            "get_home" => {
                let committed = self.committed.lock().expect("Host commit lock");
                committed
                    .sources
                    .source(input["source_id"].as_str().unwrap_or(""), None)
                    .ok()
                    .and_then(|source| source["kind"].as_str())
                    .map(|kind| crate::home::operation(kind, input))
                    .unwrap_or_else(|| "default_metadata".into())
            }
            _ => "default_metadata".into(),
        }
    }

    fn execute_inner(
        &self,
        call: &Call,
        error_details: &mut Option<Value>,
    ) -> Result<Value, ApiError> {
        let input = &call.args["input"];
        if call.command == "cancel_request" {
            return self.requests.cancel(
                call.args["request_id"].as_str().expect("Request UUID"),
                input["target_request_id"].as_str().expect("Target UUID"),
            );
        }
        let job = call.job.as_ref().expect("Ordinary call job");
        job.check()?;
        // Capture under the commit lock, perform HTTP outside it, then recheck
        // before publication. Settings and cancellation stay responsive to I/O.
        match call.command.as_str() {
            "sync_catalog" => return self.sync_catalog(input, job, error_details),
            "get_home" => return self.route_content(input, job, "home", call.metadata_epoch),
            "search_catalog" => {
                if input["query"].as_str().is_none_or(|q| q.trim().is_empty()) {
                    return Err(ApiError::bad("Search requires a nonblank query"));
                }
                return self.route_content(input, job, "search", call.metadata_epoch);
            }
            "get_details" => {
                return self.route_content(&input["ref"], job, "details", call.metadata_epoch);
            }
            "resolve_episode" => {
                return self.resolve(input, job, call.resolve_sequence.expect("Resolve sequence"));
            }
            "match_playback" => {
                return self.match_playback(
                    input,
                    job,
                    call.match_sequence.expect("Match sequence"),
                    error_details,
                );
            }
            "get_media_cache" => return self.media_cache.status_checked(|| job.check()),
            "clear_media_cache" => {
                job.check()?;
                return self.media_cache.clear_checked(|| job.check());
            }
            _ => {}
        }
        let mut committed = self
            .committed
            .lock()
            .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
        job.check()?;
        let result = match call.command.as_str() {
            "runtime_status" => self.runtime(&committed),
            "runtime_prepare" => {
                if input["target"] == "cloudstream" {
                    return Err(ApiError::new(
                        "CAPABILITY_UNAVAILABLE",
                        "Cloudstream is no longer supported; add a JSON source instead",
                    ));
                }
                if committed.gateway.is_none() {
                    committed.gateway = Some(Gateway::with_media_cache(
                        self.clock.clone(),
                        self.media_cache.clone(),
                    )?);
                }
                self.runtime(&committed)
            }
            "get_sources" => Ok(committed.sources.sources()),
            "get_settings" => Ok(committed.settings.clone()),
            "save_settings" => {
                job.check()?;
                // After replacement starts, finish publication even if cancellation arrives.
                self.store.save_checked(input, || job.check())?;
                if input["last_playback_source_key"]
                    != committed.settings["last_playback_source_key"]
                    || input["last_metadata_config_id"]
                        != committed.settings["last_metadata_config_id"]
                {
                    committed.matches.invalidate();
                }
                let affected = committed.sources.replace_configs(
                    input["source_configs"]
                        .as_array()
                        .expect("Validated configs"),
                );
                if stremio::locale(input["ui_locale"].as_str().unwrap_or("en"))
                    != stremio::locale(committed.settings["ui_locale"].as_str().unwrap_or("en"))
                {
                    committed.metadata_epoch += 1;
                    for (request_id, _) in committed.metadata_requests.drain() {
                        self.requests
                            .cancel(&Uuid::new_v4().to_string(), &request_id)?;
                    }
                }
                committed.settings = input.clone();
                committed.maccms.retain(|id, _| !affected.contains(id));
                committed.stremio.retain(|id, _| !affected.contains(id));
                committed.matches.revoke(&affected);
                committed.resolutions.revoke(&affected);
                let owned = |context: &Value| {
                    context["episode_ref"]["source_id"]
                        .as_str()
                        .is_some_and(|id| affected.iter().any(|source| source == id))
                };
                committed
                    .resolution_contexts
                    .retain(|_, context| !owned(context));
                committed
                    .session_bindings
                    .retain(|_, binding| !owned(&binding.context));
                if let Some(gateway) = &committed.gateway {
                    gateway.revoke(&affected);
                }
                self.requests.cancel_sources(&affected);
                let confirmed = committed.settings.clone();
                self.media_cache
                    .configure(crate::media_cache::limit(&confirmed));
                drop(committed);
                return Ok(confirmed);
            }
            "start_interaction" | "finish_interaction" => Err(ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "Cloudstream is no longer supported; add a JSON source instead",
            )),
            "release_resolution" => {
                committed
                    .resolution_contexts
                    .remove(input["resolution_id"].as_str().unwrap_or(""));
                if committed.current_resolution.as_deref() == input["resolution_id"].as_str() {
                    committed.current_resolution = None;
                }
                committed
                    .resolutions
                    .release(input["resolution_id"].as_str().expect("Resolution ID"));
                Ok(json!({"ok":true}))
            }
            "open_playback" => {
                let selected = committed.resolutions.selected(
                    input["resolution_id"].as_str().expect("Resolution ID"),
                    input["stream_id"].as_str().expect("Stream ID"),
                    self.clock.now_ms(),
                )?;
                committed
                    .sources
                    .source(&selected.link.provider, Some(&selected.source_revision))?;
                if committed.gateway.is_none() {
                    committed.gateway = Some(Gateway::with_media_cache(
                        self.clock.clone(),
                        self.media_cache.clone(),
                    )?);
                }
                let gateway = committed.gateway.as_ref().expect("Gateway ready");
                let view = gateway.open(selected)?;
                if let Err(error) = job.check() {
                    gateway.close(view["session_id"].as_str().expect("Session ID"));
                    return Err(error);
                }
                if let Some(context) = input["resolution_id"]
                    .as_str()
                    .and_then(|id| committed.resolution_contexts.get(id))
                    .cloned()
                {
                    committed.session_bindings.insert(
                        view["session_id"].as_str().expect("Session ID").to_owned(),
                        SessionBinding {
                            context,
                            playback_epoch: None,
                        },
                    );
                }
                Ok(view)
            }
            "close_playback" => {
                if let Some(gateway) = &committed.gateway {
                    gateway.close(input["session_id"].as_str().expect("Session ID"));
                }
                committed
                    .session_bindings
                    .remove(input["session_id"].as_str().expect("Session ID"));
                Ok(json!({"ok":true}))
            }
            "get_resource_status" => {
                let kind = input["kind"].as_str().expect("Resource kind");
                let id = input["target_id"].as_str().expect("Resource ID");
                if let Some(gateway) = &committed.gateway {
                    gateway.status(kind, id)
                } else {
                    Ok(
                        json!({"kind":kind,"target_id":id,"state":"expired","error":ApiError::new("SESSION_EXPIRED","Resource is not registered")}),
                    )
                }
            }
            // Committed history must report its result, whatever arrives after the write.
            "list_history" => return committed.history.list(),
            "list_watchlist" => return committed.history.list_watchlist(),
            "add_watchlist" => return committed.history.add_watchlist(input),
            "remove_watchlist" => {
                return committed
                    .history
                    .remove_watchlist(input["content_id"].as_str().expect("Content ID"));
            }
            "save_history" => return save_history(&mut committed, input),
            "remove_history" => {
                return committed.history.remove(
                    input["content_id"].as_str().expect("Content ID"),
                    input["write_sequence"].as_u64().expect("Write sequence"),
                );
            }
            "clear_history" => {
                return committed
                    .history
                    .clear(input["write_sequence"].as_u64().expect("Write sequence"));
            }
            _ => Err(ApiError::bad("Unknown command")),
        };
        // No callback or delayed result may publish after cancellation/deadline.
        job.check()?;
        result
    }

    fn runtime(&self, committed: &Committed) -> Result<Value, ApiError> {
        let health = committed
            .gateway
            .as_ref()
            .map(Gateway::health)
            .transpose()?;
        Ok(
            json!({"platform": platform(), "phase": if health.is_some() {"ready"} else {"checking"},
            "engine_epoch": self.host_epoch, "health": health, "error": null,
            "cloudstream": {"phase": "stopped", "engine_epoch": null, "display_mode": "none", "health": null, "error": null}}),
        )
    }

    fn sync_catalog(
        &self,
        input: &Value,
        job: &Job,
        error_details: &mut Option<Value>,
    ) -> Result<Value, ApiError> {
        let kind = SourceKind::parse(&input["kind"])?;
        if kind == SourceKind::Cloudstream {
            return Err(ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "Cloudstream is no longer supported; add a JSON source instead",
            ));
        }
        let captured = {
            let mut committed = self
                .committed
                .lock()
                .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
            let captured = committed.sources.snapshot(
                input["config_id"].as_str().expect("Config UUID"),
                &self.host_epoch,
                true,
            )?;
            if captured.config["kind"] != input["kind"] {
                return Err(ApiError::bad("Source kind does not match configuration"));
            }
            job.bind_source(&captured.source_id);
            captured
        };
        if kind == SourceKind::Stremio {
            let result = stremio::Catalog::sync(captured.clone(), &maccms::Http::default(), job);
            let mut committed = self
                .committed
                .lock()
                .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
            let result = match job.check() {
                Ok(()) => result,
                Err(error) if error.code == "CANCELLED" => return Err(error),
                Err(error) => Err(stremio::SyncFailure {
                    error,
                    indices: (0..captured.config["manifest_urls"]
                        .as_array()
                        .expect("Manifest URLs")
                        .len())
                        .collect(),
                }),
            };
            committed
                .sources
                .check_snapshot(&captured, &self.host_epoch, None, true)?;
            let catalog = match result {
                Ok(catalog) => catalog,
                Err(failure) => {
                    let details = json!({"reason": "manifest_sync_failed", "failed_addon_indices": failure.indices});
                    let mut error = json!(failure.error);
                    if matches!(
                        failure.error.code.as_str(),
                        "UPSTREAM_FAILED" | "UPSTREAM_TIMEOUT"
                    ) {
                        error["details"] = details.clone();
                        *error_details = Some(details);
                    }
                    if failure.error.code != "CANCELLED" {
                        committed.sources.fail_unpublished_sync(
                            &captured,
                            &self.host_epoch,
                            error,
                        )?;
                    }
                    return Err(failure.error);
                }
            };
            let changed = committed
                .stremio
                .get(&captured.source_id)
                .is_none_or(|previous| previous.source != catalog.source);
            committed
                .sources
                .publish_sync(&captured, &self.host_epoch, catalog.source.clone())?;
            // Identical manifests retain session references, cursors and cache.
            if changed {
                committed
                    .matches
                    .revoke(std::slice::from_ref(&captured.source_id));
                committed
                    .resolutions
                    .revoke(std::slice::from_ref(&captured.source_id));
                if let Some(gateway) = &committed.gateway {
                    gateway.revoke(std::slice::from_ref(&captured.source_id));
                }
                for (request_id, source_id) in &committed.metadata_requests {
                    if *source_id == captured.source_id {
                        self.requests
                            .cancel(&Uuid::new_v4().to_string(), request_id)?;
                    }
                }
                committed.stremio.insert(captured.source_id, catalog);
            }
            return Ok(
                json!({"kind": kind, "config_id": captured.config["config_id"], "changed": changed, "sources": committed.sources.sources(), "cloudstream_result": null}),
            );
        }
        let result = maccms::Catalog::sync(captured.clone(), &maccms::Http::default(), job);
        let mut committed = self
            .committed
            .lock()
            .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
        job.check()?;
        committed
            .sources
            .check_snapshot(&captured, &self.host_epoch, None, true)?;
        let mut catalog = result?;
        let previous = committed.maccms.get(&captured.source_id);
        let changed = previous.is_none_or(|previous| {
            previous.source != catalog.source || previous.sections != catalog.sections
        });
        if let Some(previous) = previous {
            catalog.images = previous.images.clone();
        }
        committed
            .sources
            .publish_sync(&captured, &self.host_epoch, catalog.source.clone())?;
        committed.maccms.insert(captured.source_id, catalog);
        Ok(
            json!({"kind": kind, "config_id": captured.config["config_id"],
            "changed": changed, "sources": committed.sources.sources(), "cloudstream_result": null}),
        )
    }

    fn route_content(
        &self,
        key: &Value,
        job: &Job,
        capability: &str,
        page_epoch: Option<u64>,
    ) -> Result<Value, ApiError> {
        fn search_lock<'a>(
            gate: &'a Mutex<()>,
            job: &Job,
        ) -> Result<std::sync::MutexGuard<'a, ()>, ApiError> {
            loop {
                job.check()?;
                match gate.try_lock() {
                    Ok(guard) => return Ok(guard),
                    Err(std::sync::TryLockError::WouldBlock) => {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(_) => return Err(ApiError::internal("Search gate poisoned")),
                }
            }
        }
        let mut committed = self
            .committed
            .lock()
            .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
        let source = committed
            .sources
            .source(
                key["source_id"]
                    .as_str()
                    .ok_or_else(|| ApiError::bad("Missing source_id"))?,
                key["source_revision"].as_str(),
            )?
            .clone();
        job.bind_source(source["id"].as_str().expect("Source id"));
        if capability == "home" && key["catalog_only"] == true && source["kind"] != "stremio" {
            return Err(ApiError::bad(
                "catalog_only is supported only by Stremio sources",
            ));
        }
        match SourceKind::parse(&source["kind"])? {
            SourceKind::Stremio if capability == "resolve" => Err(ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "Stremio provides metadata only",
            )),
            SourceKind::Cloudstream => Err(ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "Cloudstream is no longer supported; add a JSON source instead",
            )),
            SourceKind::Stremio => {
                if page_epoch.is_some_and(|epoch| epoch != committed.metadata_epoch) {
                    return Err(ApiError::cancelled());
                }
                let epoch = committed.metadata_epoch;
                let language =
                    stremio::locale(committed.settings["ui_locale"].as_str().unwrap_or("en"));
                let captured = committed.sources.snapshot(
                    source["origin"]["config_id"].as_str().expect("Config UUID"),
                    &self.host_epoch,
                    false,
                )?;
                let catalog = committed
                    .stremio
                    .get(&captured.source_id)
                    .cloned()
                    .ok_or_else(|| {
                        ApiError::new("ENGINE_NOT_READY", "Stremio manifests must be synchronized")
                    })?;
                drop(committed);
                let http = maccms::Http::default();
                let _search = if capability == "search" {
                    Some(search_lock(&catalog.search_gate, job)?)
                } else {
                    None
                };
                let result = if capability == "home" {
                    catalog.home(&http, key, language, job)
                } else if capability == "search" {
                    catalog.search(&http, key["query"].as_str().expect("Query"), language, job)
                } else {
                    catalog.details(&http, key, language, job)
                };
                let mut committed = self
                    .committed
                    .lock()
                    .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
                job.check()?;
                if epoch != committed.metadata_epoch {
                    return Err(ApiError::cancelled());
                }
                committed
                    .sources
                    .check_snapshot(&captured, &self.host_epoch, None, false)?;
                let mut output = result?;
                catalog.commit(&output)?;
                self.image_access(
                    &mut committed,
                    &captured.source_id,
                    &mut output.value,
                    |id| catalog.image_record(id),
                )?;
                if capability == "search" {
                    output
                        .value
                        .as_object_mut()
                        .expect("Search object")
                        .remove("images");
                }
                Ok(output.value)
            }
            SourceKind::Maccms => {
                let captured = committed.sources.snapshot(
                    source["origin"]["config_id"].as_str().expect("Config UUID"),
                    &self.host_epoch,
                    false,
                )?;
                let catalog = committed
                    .maccms
                    .get(&captured.source_id)
                    .cloned()
                    .ok_or_else(|| {
                        ApiError::new("ENGINE_NOT_READY", "MacCMS catalog must be synchronized")
                    })?;
                if capability == "resolve" {
                    return Err(ApiError::bad("Playback match is not registered"));
                }
                drop(committed);
                let http = maccms::Http::default();
                let _search = if capability == "search" {
                    Some(search_lock(&catalog.search_gate, job)?)
                } else {
                    None
                };
                let result = if capability == "home" {
                    catalog.home(&http, key, job).map(|home| {
                        (
                            home.mapped,
                            key["section_ref"].is_null().then_some(home.sections),
                        )
                    })
                } else if capability == "search" {
                    catalog
                        .search(&http, key["query"].as_str().expect("Query"), 1, job)
                        .map(|mut mapped| {
                            mapped.value = json!({"items":mapped.value});
                            (mapped, None)
                        })
                } else {
                    catalog
                        .details(&http, key, job)
                        .map(|mapped| (mapped, None))
                };
                let mut committed = self
                    .committed
                    .lock()
                    .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
                job.check()?;
                committed
                    .sources
                    .check_snapshot(&captured, &self.host_epoch, None, false)?;
                let (mut mapped, sections) = result?;
                let current = committed
                    .maccms
                    .get_mut(&captured.source_id)
                    .ok_or_else(ApiError::cancelled)?;
                current.register_images(mapped.images)?;
                if let Some(sections) = sections {
                    current.sections = sections;
                }
                if capability == "details" {
                    committed
                        .matches
                        .register_details(mapped.value["details"].clone(), self.clock.now_ms());
                }
                let images = committed.maccms[&captured.source_id].images.clone();
                self.image_access(
                    &mut committed,
                    &captured.source_id,
                    &mut mapped.value,
                    |id| images.get(id).cloned(),
                )?;
                if capability == "search" {
                    mapped
                        .value
                        .as_object_mut()
                        .expect("Search object")
                        .remove("images");
                }
                Ok(mapped.value)
            }
        }
    }

    fn match_playback(
        &self,
        input: &Value,
        job: &Job,
        sequence: u64,
        error_details: &mut Option<Value>,
    ) -> Result<Value, ApiError> {
        if input["metadata_ref"]["identity"]["namespace"] == "maccms" {
            return self.match_maccms(input, job, sequence);
        }
        let (metadata_snapshot, metadata_catalog, provider_snapshot, provider, catalog, previous) = {
            let mut committed = self.committed.lock().expect("Host commit lock");
            committed.matches.check(sequence)?;
            let metadata_source = committed
                .sources
                .source(
                    input["metadata_ref"]["content_ref"]["source_id"]
                        .as_str()
                        .expect("Metadata source"),
                    input["metadata_ref"]["content_ref"]["source_revision"].as_str(),
                )?
                .clone();
            if metadata_source["kind"] != "stremio" {
                return Err(ApiError::bad("Matching requires Stremio metadata"));
            }
            let provider = committed
                .sources
                .source(
                    input["playback_source"]["source_id"]
                        .as_str()
                        .expect("Provider source"),
                    input["playback_source"]["source_revision"].as_str(),
                )?
                .clone();
            if provider["kind"] == "stremio" {
                return Err(ApiError::new(
                    "CAPABILITY_UNAVAILABLE",
                    "Stremio provides metadata only",
                ));
            }
            if provider["kind"] == "cloudstream" {
                return Err(ApiError::new(
                    "CAPABILITY_UNAVAILABLE",
                    "Cloudstream is no longer supported; add a JSON source instead",
                ));
            }
            let metadata_snapshot = committed.sources.snapshot(
                metadata_source["origin"]["config_id"]
                    .as_str()
                    .expect("Metadata config"),
                &self.host_epoch,
                false,
            )?;
            let provider_snapshot = committed.sources.snapshot(
                provider["origin"]["config_id"]
                    .as_str()
                    .expect("Provider config"),
                &self.host_epoch,
                false,
            )?;
            let metadata_catalog = committed
                .stremio
                .get(&metadata_snapshot.source_id)
                .cloned()
                .ok_or_else(|| {
                    ApiError::new("ENGINE_NOT_READY", "Synchronize Stremio metadata first")
                })?;
            let catalog = committed
                .maccms
                .get(&provider_snapshot.source_id)
                .cloned()
                .ok_or_else(|| ApiError::new("ENGINE_NOT_READY", "Synchronize MacCMS first"))?;
            let previous = if input["selection"].is_null() {
                None
            } else {
                let selection = &input["selection"];
                if selection["content_ref"]["source_id"] != provider_snapshot.source_id
                    || selection["content_ref"]["source_revision"]
                        != provider_snapshot.source_revision
                {
                    return Err(ApiError::bad(
                        "Selection belongs to another provider revision",
                    ));
                }
                Some(
                    committed
                        .matches
                        .registered(selection, self.clock.now_ms())?,
                )
            };
            (
                metadata_snapshot,
                metadata_catalog,
                provider_snapshot,
                provider,
                catalog,
                previous,
            )
        };
        job.bind_source(&provider_snapshot.source_id);
        job.bind_source(&metadata_snapshot.source_id);
        matching::budget(job)?;
        let http = maccms::Http::default();
        let child = job.aggregate_child(&Uuid::new_v4().to_string(), "default_metadata")?;
        let metadata =
            metadata_catalog.details(&http, &input["metadata_ref"]["content_ref"], "en", &child)?;
        if metadata.value["metadata_ref"] != input["metadata_ref"] {
            return Err(ApiError::bad(
                "Metadata identity does not belong to this reference",
            ));
        }
        let requested_video = matching::video(
            &metadata.value,
            input["video_id"].as_str().expect("Video ID"),
        )?;
        let aliases = metadata_catalog.match_aliases(&input["metadata_ref"]["content_ref"]);
        let (proposal, confirmation, mapped) = if let Some(previous) = previous {
            matching::budget(job)?;
            let child = job.aggregate_child(&Uuid::new_v4().to_string(), "default_metadata")?;
            let current = catalog.details(&http, &input["selection"]["content_ref"], &child)?;
            let old_episode = matching::selection(&previous, &input["selection"])?;
            let episode = matching::selection(&current.value["details"], &input["selection"])?;
            if old_episode != episode {
                return Err(ApiError::new(
                    "REVISION_MISMATCH",
                    "Selected episode changed; inspect details again",
                ));
            }
            (
                matching::Proposal {
                    candidates: vec![matching::candidate(&current.value["details"])],
                    selected: Some((current.value["details"].clone(), input["selection"].clone())),
                },
                "manual",
                Some(current),
            )
        } else {
            if provider["capabilities"]["search"] != true {
                *error_details = Some(
                    json!({"reason":"search_unavailable", "source_id":provider["id"], "source_revision":provider["source_revision"], "home":provider["capabilities"]["home"], "details":provider["capabilities"]["details"]}),
                );
                return Err(ApiError::new(
                    "CAPABILITY_UNAVAILABLE",
                    "Browse this provider home/details to select a title, or choose another provider",
                ));
            }
            let proposal = matching::search(
                &metadata.value,
                &requested_video,
                &aliases,
                job,
                |title, child| {
                    let mapped = catalog.match_search(&http, title, child)?;
                    Ok(mapped
                        .value
                        .as_array()
                        .expect("Mapped search cards")
                        .iter()
                        .take(10)
                        .map(|details| matching::Candidate {
                            details: details.clone(),
                            external_identity: None,
                            external_videos: Default::default(),
                        })
                        .collect())
                },
            )?;
            (proposal, "external_id", None)
        };
        let mut committed = self.committed.lock().expect("Host commit lock");
        matching::budget(job)?;
        committed.matches.check(sequence)?;
        committed
            .sources
            .check_snapshot(&metadata_snapshot, &self.host_epoch, None, false)?;
        committed
            .sources
            .check_snapshot(&provider_snapshot, &self.host_epoch, None, false)?;
        metadata_catalog.commit(&metadata)?;
        if let Some(mapped) = mapped {
            committed
                .maccms
                .get_mut(&provider_snapshot.source_id)
                .ok_or_else(ApiError::cancelled)?
                .register_images(mapped.images)?;
        }
        let mut candidates = proposal.candidates;
        let context = if let Some((details, selected)) = proposal.selected {
            candidates = vec![matching::candidate(&details)];
            Some(committed.matches.issue(
                input,
                &details,
                &selected,
                (metadata_snapshot, provider_snapshot),
                confirmation,
                self.clock.now_ms(),
            )?)
        } else {
            None
        };
        Ok(
            json!({"status":if context.is_some() {"matched"} else if candidates.is_empty() {"not_found"} else {"needs_confirmation"}, "metadata_ref":input["metadata_ref"], "video_id":input["video_id"], "candidates":candidates, "context":context}),
        )
    }

    fn match_maccms(&self, input: &Value, job: &Job, sequence: u64) -> Result<Value, ApiError> {
        let reference = &input["metadata_ref"]["content_ref"];
        if input["playback_source"]["source_id"] != reference["source_id"]
            || input["playback_source"]["source_revision"] != reference["source_revision"]
        {
            return Err(ApiError::bad(
                "MacCMS metadata requires the same playback source and revision",
            ));
        }
        let (snapshot, catalog) = {
            let mut committed = self.committed.lock().expect("Host commit lock");
            committed.matches.check(sequence)?;
            let source = committed
                .sources
                .source(
                    reference["source_id"].as_str().expect("Source"),
                    reference["source_revision"].as_str(),
                )?
                .clone();
            if source["kind"] != "maccms" || source["capabilities"]["resolve"] != true {
                return Err(ApiError::bad("Same-source playback requires MacCMS"));
            }
            let snapshot = committed.sources.snapshot(
                source["origin"]["config_id"].as_str().expect("Config"),
                &self.host_epoch,
                false,
            )?;
            let catalog = committed
                .maccms
                .get(&snapshot.source_id)
                .cloned()
                .ok_or_else(|| ApiError::new("ENGINE_NOT_READY", "Synchronize MacCMS first"))?;
            (snapshot, catalog)
        };
        job.bind_source(&snapshot.source_id);
        matching::budget(job)?;
        let child = job.aggregate_child(&Uuid::new_v4().to_string(), "default_metadata")?;
        let mapped = catalog.details(&maccms::Http::default(), reference, &child)?;
        if mapped.value["metadata_ref"] != input["metadata_ref"] {
            return Err(ApiError::bad(
                "MacCMS metadata identity differs from current details",
            ));
        }
        let episode =
            matching::video(&mapped.value, input["video_id"].as_str().expect("Video ID"))?;
        let details = &mapped.value["details"];
        let group = details["groups"]
            .as_array()
            .expect("Groups")
            .iter()
            .find(|g| {
                g["episodes"]
                    .as_array()
                    .expect("Episodes")
                    .contains(&episode)
            })
            .ok_or_else(|| ApiError::bad("MacCMS episode group is missing"))?;
        let mut selection =
            json!({"content_ref":reference,"group_id":group["id"],"episode_id":episode["id"]});
        let mut committed = self.committed.lock().expect("Host commit lock");
        matching::budget(job)?;
        committed.matches.check(sequence)?;
        if !input["selection"].is_null() {
            let chosen = &input["selection"];
            let previous = committed.matches.registered(chosen, self.clock.now_ms())?;
            let old = matching::selection(&previous, chosen)?;
            let current = matching::selection(details, chosen)?;
            if old != current {
                return Err(ApiError::new(
                    "REVISION_MISMATCH",
                    "Selected episode changed; inspect details again",
                ));
            }
            // Keep the requested metadata video identity when switching a
            // route. Both current groups must uniquely name the same episode.
            let selected_group = details["groups"]
                .as_array()
                .expect("Groups")
                .iter()
                .find(|g| g["id"] == chosen["group_id"])
                .ok_or_else(|| ApiError::bad("Selected route is missing"))?;
            let unique = |g: &Value| {
                g["episodes"]
                    .as_array()
                    .expect("Episodes")
                    .iter()
                    .filter(|e| e["label"] == episode["label"])
                    .count()
                    == 1
            };
            if current["label"] != episode["label"]
                || !unique(group)
                || !unique(selected_group)
                || current["season"] != episode["season"]
                || current["number"] != episode["number"]
            {
                return Err(ApiError::bad(
                    "Selected route does not uniquely identify the requested video",
                ));
            }
            selection = chosen.clone();
        }
        committed
            .sources
            .check_snapshot(&snapshot, &self.host_epoch, None, false)?;
        committed
            .maccms
            .get_mut(&snapshot.source_id)
            .ok_or_else(ApiError::cancelled)?
            .register_images(mapped.images)?;
        let context = committed.matches.issue(
            input,
            details,
            &selection,
            (snapshot.clone(), snapshot),
            "same_source",
            self.clock.now_ms(),
        )?;
        Ok(
            json!({"status":"matched","metadata_ref":input["metadata_ref"],"video_id":input["video_id"],"candidates":[matching::candidate(details)],"context":context}),
        )
    }

    fn image_access(
        &self,
        committed: &mut Committed,
        source: &str,
        value: &mut Value,
        mut lookup: impl FnMut(&str) -> Option<maccms::Image>,
    ) -> Result<(), ApiError> {
        let mut references = std::collections::HashMap::new();
        crate::details::images(value, &mut |reference| {
            let id = reference["resource_id"].as_str().expect("Image ID");
            references.insert(
                id.to_owned(),
                lookup(id).ok_or_else(|| ApiError::internal("DTO image has no native record"))?,
            );
            Ok(())
        })?;
        if !references.is_empty() && committed.gateway.is_none() {
            committed.gateway = Some(Gateway::with_media_cache(
                self.clock.clone(),
                self.media_cache.clone(),
            )?);
        }
        let mut access = serde_json::Map::new();
        for (id, image) in references {
            access.insert(
                id,
                committed
                    .gateway
                    .as_ref()
                    .expect("Image gateway")
                    .image(source, image)?,
            );
        }
        fn capabilities(value: &mut Value, access: &serde_json::Map<String, Value>) {
            match value {
                Value::Object(fields)
                    if fields.contains_key("resource_id") && fields.contains_key("capability") =>
                {
                    let id = fields["resource_id"].as_str().expect("Image ID");
                    let url = access[id]["url"].as_str().expect("Image URL");
                    fields.insert(
                        "capability".into(),
                        json!(url.split_once("?cap=").expect("Image cap URL").1),
                    );
                }
                Value::Object(fields) => {
                    for child in fields.values_mut() {
                        capabilities(child, access);
                    }
                }
                Value::Array(items) => {
                    for child in items {
                        capabilities(child, access);
                    }
                }
                _ => {}
            }
        }
        capabilities(value, &access);
        value["images"] = Value::Object(access);
        Ok(())
    }

    fn resolve(&self, input: &Value, job: &Job, sequence: u64) -> Result<Value, ApiError> {
        let reference = &input["episode_ref"];
        let (record, catalog, previous_resolution) = {
            let mut committed = self.committed.lock().expect("Host commit lock");
            let source = committed.sources.source(
                reference["source_id"].as_str().expect("Episode source"),
                reference["source_revision"].as_str(),
            )?;
            if source["kind"] == "stremio" {
                return Err(ApiError::new(
                    "CAPABILITY_UNAVAILABLE",
                    "Stremio provides metadata only",
                ));
            }
            let record = committed.matches.get(
                input["match_id"].as_str().expect("Match ID"),
                reference,
                self.clock.now_ms(),
            )?;
            committed
                .sources
                .check_snapshot(&record.metadata, &self.host_epoch, None, false)?;
            committed
                .sources
                .check_snapshot(&record.provider, &self.host_epoch, None, false)?;
            let catalog = committed
                .maccms
                .get(&record.provider.source_id)
                .cloned()
                .ok_or_else(|| ApiError::new("ENGINE_NOT_READY", "Provider is unavailable"))?;
            (record, catalog, committed.current_resolution.clone())
        };
        job.bind_source(&record.provider.source_id);
        job.bind_source(&record.metadata.source_id);
        let media = catalog.resolve(&maccms::Http::default(), reference, job)?;
        let mut committed = self.committed.lock().expect("Host commit lock");
        job.check()?;
        if sequence != committed.resolve_sequence {
            return Err(ApiError::cancelled());
        }
        committed
            .sources
            .check_snapshot(&record.metadata, &self.host_epoch, None, false)?;
        committed
            .sources
            .check_snapshot(&record.provider, &self.host_epoch, None, false)?;
        committed.matches.get(
            input["match_id"].as_str().expect("Match ID"),
            reference,
            self.clock.now_ms(),
        )?;
        let link = resolution::Link {
            original: json!({"url":media.url}),
            provider: record.provider.source_id,
            url: media.url,
            headers: media.headers,
            referer: String::new(),
            extractor_data: None,
            interceptor: None,
            cookies: None,
            label: record.label,
            quality: None,
            quality_label: None,
            kind: media.kind.into(),
            mime_type: media.mime_type,
            playable: media.playable,
            reason: media.reason.map(str::to_owned),
        };
        let value =
            committed
                .resolutions
                .register(reference, vec![link], vec![], self.clock.now_ms())?;
        if let Some(old) = previous_resolution {
            committed.resolutions.release(&old);
        }
        committed.current_resolution = value["id"].as_str().map(str::to_owned);
        if let Some(id) = value["id"].as_str() {
            committed
                .resolution_contexts
                .insert(id.to_owned(), record.context.clone());
        }
        Ok(value)
    }

    pub fn window_bounds(&self) -> Option<WindowBounds> {
        let committed = self.committed.lock().ok()?;
        let bounds = &committed.settings["window_bounds"];
        Some(WindowBounds {
            x: bounds["x"].as_f64()?,
            y: bounds["y"].as_f64()?,
            width: bounds["width"].as_f64()?,
            height: bounds["height"].as_f64()?,
            maximized: bounds["maximized"].as_bool()?,
        })
    }

    pub fn capture_source(&self, config_id: &str, sync: bool) -> Result<SourceSnapshot, ApiError> {
        self.committed
            .lock()
            .map_err(|_| ApiError::internal("Host commit lock poisoned"))?
            .sources
            .snapshot(config_id, &self.host_epoch, sync)
    }

    pub fn publish_source(
        &self,
        captured: &SourceSnapshot,
        source: Value,
        job: &Job,
    ) -> Result<(), ApiError> {
        let mut committed = self
            .committed
            .lock()
            .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
        job.check()?;
        committed
            .sources
            .publish_sync(captured, &self.host_epoch, source)
    }

    pub fn gateway_address(&self) -> Option<std::net::SocketAddr> {
        self.committed
            .lock()
            .expect("Host commit lock")
            .gateway
            .as_ref()
            .map(|gateway| gateway.address)
    }
}

/// An accepted retry skips the Session: the snapshot already holds its confirmed identity.
fn save_history(committed: &mut Committed, input: &Value) -> Result<Value, ApiError> {
    let sequence = input["write_sequence"].as_u64().expect("Write sequence");
    if committed.history.retrying(sequence, input)? {
        return committed.history.commit(sequence, input);
    }
    if sequence <= committed.history.committed() {
        return Err(crate::history::superseded());
    }
    let session = input["session_id"].as_str().expect("Session ID");
    let live = committed.gateway.as_ref().is_some_and(|gateway| {
        gateway
            .status("playback", session)
            .is_ok_and(|status| status["state"] != "expired")
    });
    let binding = committed
        .session_bindings
        .get_mut(session)
        .filter(|_| live)
        .ok_or_else(|| ApiError::new("SESSION_EXPIRED", "Playback session is closed"))?;
    let record = &input["record"];
    let context = &binding.context;
    let confirmed = &context["metadata_ref"]["identity"];
    let text = |field: &str| confirmed[field].as_str().unwrap_or("");
    let content_id = identity(
        "metadata",
        &[text("namespace"), text("type"), text("canonical_id")],
    );
    let epoch = input["playback_epoch"].as_u64().expect("Playback epoch");
    if context["match_id"] != input["match_id"]
        || record["metadata_identity"] != *confirmed
        || record["content_id"] != content_id.as_str()
        || record["video_id"] != context["metadata_video_id"]
        || record["source_id"] != context["episode_ref"]["source_id"]
        || binding.playback_epoch.is_some_and(|known| known != epoch)
    {
        return Err(ApiError::bad(
            "History snapshot does not belong to this playback session",
        ));
    }
    binding.playback_epoch = Some(epoch);
    committed.history.commit(sequence, input)
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "linux"
    }
}
