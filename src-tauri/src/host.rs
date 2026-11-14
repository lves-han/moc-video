use crate::{
    contracts::{ApiError, COMMANDS, Contracts, envelope, wire_size},
    gateway::Gateway,
    history::History,
    line_round::{self, Round, Rounds},
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

#[path = "live.rs"]
pub mod live;
#[path = "local.rs"]
pub mod local;
#[path = "maccms.rs"]
pub mod maccms;
#[cfg(test)]
#[path = "../../tests/fixtures/observation_redaction.rs"]
mod observation_redaction;
#[path = "source_cache.rs"]
mod source_cache;
#[path = "stremio.rs"]
pub mod stremio;
#[path = "tmdb_api.rs"]
pub mod tmdb_api;

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
    service_resolutions: std::collections::HashMap<String, ServicePlayback>,
    active_service_session: Option<String>,
    line_rounds: Rounds,
    // Last fetched channels per live config: (list revision, channels).
    live: std::collections::HashMap<String, (String, Vec<live::Channel>)>,
    // Latest local library scan: file id → path.
    local: std::collections::HashMap<String, PathBuf>,
}

/// The identity a playback Session confirmed when it opened; it outlives match and candidate GC.
struct SessionBinding {
    context: Value,
    playback_epoch: Option<u64>,
    service: Option<ServicePlayback>,
}

#[derive(Clone)]
struct ServicePlayback {
    source_id: String,
    source_revision: u64,
    line_id: String,
    latency_ms: Option<u64>,
    config: crate::service::Config,
    reported: std::collections::HashSet<String>,
    metadata_snapshot: SourceSnapshot,
}

pub struct Host {
    pub host_epoch: String,
    pub requests: Arc<Requests>,
    pub clock: Arc<dyn Clock>,
    store: SettingsFile,
    media_cache: Arc<MediaCache>,
    source_cache: source_cache::SourceCache,
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

fn cache_episode_owner(context: &Value) -> Option<String> {
    let metadata = &context["metadata_ref"]["identity"];
    Some(identity(
        "episode_cache",
        &[
            metadata["config_id"].as_str()?,
            metadata["namespace"].as_str()?,
            metadata["type"].as_str()?,
            metadata["meta_id"].as_str()?,
            context["metadata_video_id"].as_str()?,
        ],
    ))
}

impl Host {
    pub fn open(path: PathBuf) -> Result<Arc<Self>, ApiError> {
        Self::with_clock(path, Arc::new(MonotonicClock::default()))
    }

    pub fn with_clock(path: PathBuf, clock: Arc<dyn Clock>) -> Result<Arc<Self>, ApiError> {
        let history_path = path.with_file_name("moc-video.db");
        let cache_path = path.with_file_name("media-cache");
        let source_cache_path = path.with_file_name("source-cache");
        let (store, settings) = SettingsFile::load(path)?;
        tmdb_api::configure(&settings);
        crate::service::configure(&settings);
        let mut sources = SourceDispatch::default();
        sources.replace_configs(&crate::settings::source_configs(&settings));
        Ok(Arc::new(Self {
            host_epoch: Uuid::new_v4().to_string(),
            requests: Arc::default(),
            clock,
            store,
            media_cache: MediaCache::for_episodes(cache_path),
            source_cache: source_cache::SourceCache::new(source_cache_path),
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
                service_resolutions: Default::default(),
                active_service_session: None,
                line_rounds: Rounds::default(),
                live: Default::default(),
                local: Default::default(),
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
            "match_playback" | "resolve_episode" | "open_playback" | "play_line"
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
                        if committed.active_service_session.as_deref()
                            == value["session_id"].as_str()
                        {
                            committed.active_service_session = None;
                        }
                        if let Some(id) = value["session_id"].as_str() {
                            committed.session_bindings.remove(id);
                        }
                    } else if call.command == "play_line" {
                        if let Some(id) = value["resolution"]["id"].as_str() {
                            committed.resolutions.release(id);
                            committed.resolution_contexts.remove(id);
                            committed.service_resolutions.remove(id);
                            if committed.current_resolution.as_deref() == Some(id) {
                                committed.current_resolution = None;
                            }
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
            "start_line_round" => return self.start_line_round(input, job),
            "get_line_round" => return self.get_line_round(input),
            "confirm_line_candidate" => return self.confirm_line_candidate(input),
            "cancel_line_round" => return self.cancel_line_round(input),
            "play_line" => return self.play_line(input, job),
            "refresh_line" => return self.refresh_line(input, job),
            "report_line_playback" => return self.report_line_playback(input, job),
            "open_playback" => {
                let service = {
                    self.committed
                        .lock()
                        .expect("Host commit lock")
                        .service_resolutions
                        .contains_key(input["resolution_id"].as_str().unwrap_or(""))
                };
                if service {
                    let result = self.open_service_playback(input, job);
                    if result.is_err() {
                        let id = input["resolution_id"].as_str().unwrap_or("");
                        let mut committed = self.committed.lock().expect("Host commit lock");
                        committed.resolutions.release(id);
                        committed.resolution_contexts.remove(id);
                        committed.service_resolutions.remove(id);
                        if committed.current_resolution.as_deref() == Some(id) {
                            committed.current_resolution = None;
                        }
                    }
                    return result;
                }
            }
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
            "get_live_channels" => return self.live_channels(input, job),
            "open_local" => return self.open_local(input, job),
            "list_local_videos" => {
                let settings = self
                    .committed
                    .lock()
                    .map_err(|_| ApiError::internal("Host commit lock poisoned"))?
                    .settings
                    .clone();
                // Scan outside the commit lock; a library edited meanwhile publishes nothing.
                let (view, table) = local::scan(&settings);
                job.check()?;
                let mut committed = self
                    .committed
                    .lock()
                    .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
                if committed.settings["local_paths"] != settings["local_paths"] {
                    return Err(ApiError::cancelled());
                }
                committed.local = table.into_iter().collect();
                return Ok(view);
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
            "get_settings" => Ok(crate::settings::redact_for_ui(&committed.settings)),
            "save_settings" => {
                job.check()?;
                let merged = crate::settings::merge_preserved_token(input, &committed.settings)?;
                let input = &merged;
                // After replacement starts, finish publication even if cancellation arrives.
                self.store.save_checked(input, || job.check())?;
                if input["last_playback_source_key"]
                    != committed.settings["last_playback_source_key"]
                    || input["last_metadata_config_id"]
                        != committed.settings["last_metadata_config_id"]
                {
                    committed.matches.invalidate();
                }
                let affected = committed
                    .sources
                    .replace_configs(&crate::settings::source_configs(input));
                if crate::settings::metadata_language(input)
                    != crate::settings::metadata_language(&committed.settings)
                {
                    committed.metadata_epoch += 1;
                    for (request_id, _) in committed.metadata_requests.drain() {
                        self.requests
                            .cancel(&Uuid::new_v4().to_string(), &request_id)?;
                    }
                }
                // Edited or removed live lists drop their channels and stop their streams.
                let stale: Vec<String> = committed
                    .live
                    .keys()
                    .filter(|id| {
                        live_config(input, id).ok() != live_config(&committed.settings, id).ok()
                    })
                    .cloned()
                    .collect();
                for id in &stale {
                    committed.live.remove(id);
                }
                if let Some(gateway) = &committed.gateway {
                    gateway.revoke(
                        &stale
                            .iter()
                            .map(|id| format!("live:{id}"))
                            .collect::<Vec<_>>(),
                    );
                }
                tmdb_api::configure(input);
                crate::service::configure(input);
                // A changed library forgets its scan and stops files it may no longer cover.
                if input["local_paths"] != committed.settings["local_paths"] {
                    committed.local.clear();
                    if let Some(gateway) = &committed.gateway {
                        gateway.revoke(&["local".to_owned()]);
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
                return Ok(crate::settings::redact_for_ui(&confirmed));
            }
            "start_interaction" | "finish_interaction" => Err(ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "Cloudstream is no longer supported; add a JSON source instead",
            )),
            "release_resolution" => {
                committed
                    .service_resolutions
                    .remove(input["resolution_id"].as_str().unwrap_or(""));
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
                let owner = input["resolution_id"]
                    .as_str()
                    .and_then(|id| committed.resolution_contexts.get(id))
                    .and_then(cache_episode_owner);
                let gateway = committed.gateway.as_ref().expect("Gateway ready");
                let view = if let Some(owner) = owner {
                    gateway.open_episode(selected, owner)?
                } else {
                    gateway.open(selected)?
                };
                if let Err(error) = job.check() {
                    gateway.close(view["session_id"].as_str().expect("Session ID"));
                    return Err(error);
                }
                committed.active_service_session = None;
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
                            service: None,
                        },
                    );
                }
                Ok(view)
            }
            "open_live" => {
                let config_id = input["config_id"].as_str().expect("Config UUID");
                live_config(&committed.settings, config_id)?;
                let (revision, channels) = committed
                    .live
                    .get(config_id)
                    .ok_or_else(|| ApiError::bad("Load the live channel list first"))?;
                if input["revision"] != revision.as_str() {
                    return Err(ApiError::new(
                        "REVISION_MISMATCH",
                        "Live channel list changed; reload it",
                    ));
                }
                let index = input["index"].as_u64().expect("Channel index") as usize;
                let channel = channels
                    .get(index)
                    .ok_or_else(|| ApiError::bad("Unknown live channel"))?;
                if !channel.supported() {
                    return Err(ApiError::new(
                        "MEDIA_UNSUPPORTED",
                        "Only http(s) live channels can be played",
                    ));
                }
                let record = live_record(config_id, revision, index, channel);
                if committed.gateway.is_none() {
                    committed.gateway = Some(Gateway::with_media_cache(
                        self.clock.clone(),
                        self.media_cache.clone(),
                    )?);
                }
                let gateway = committed.gateway.as_ref().expect("Gateway ready");
                let view = gateway.open_live(Arc::new(record))?;
                if let Err(error) = job.check() {
                    gateway.close(view["session_id"].as_str().expect("Session ID"));
                    return Err(error);
                }
                committed.active_service_session = None;
                Ok(view)
            }
            "close_playback" => {
                if committed.active_service_session.as_deref() == input["session_id"].as_str() {
                    committed.active_service_session = None;
                }
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

    fn open_local(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let id = input["id"].as_str().expect("File ID");
        let lock = || {
            self.committed
                .lock()
                .map_err(|_| ApiError::internal("Host commit lock poisoned"))
        };
        let (listed, path, file) = {
            let committed = lock()?;
            let listed = committed.local.get(id).cloned().ok_or_else(|| {
                ApiError::new("SOURCE_NOT_FOUND", "Refresh the local library first")
            })?;
            let (path, file) = local::checked_file(&committed.settings, &listed)?;
            (listed, path, file)
        };
        if !local::supported(&path) {
            return Err(ApiError::new(
                "MEDIA_UNSUPPORTED",
                "This video format is not supported yet",
            ));
        }
        // MKV index reading happens outside the commit lock (ADR-066/067).
        let (plan, file) = if local::remuxed(&path) {
            (
                Some(Arc::new(crate::remux::Plan::open_file(&path, file)?)),
                None,
            )
        } else {
            (None, Some(file))
        };
        job.check()?;
        let mut committed = lock()?;
        if committed.local.get(id) != Some(&listed) {
            return Err(ApiError::cancelled());
        }
        let title: String = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .chars()
            .take(512)
            .collect();
        let record = Arc::new(local_record(id, &path, &title));
        if committed.gateway.is_none() {
            committed.gateway = Some(Gateway::with_media_cache(
                self.clock.clone(),
                self.media_cache.clone(),
            )?);
        }
        let gateway = committed.gateway.as_ref().expect("Gateway ready");
        let view = match plan {
            Some(plan) => gateway.open_remux(record, plan)?,
            None => gateway.open_file(record, file.expect("Local file handle"))?,
        };
        if let Err(error) = job.check() {
            gateway.close(view["session_id"].as_str().expect("Session ID"));
            return Err(error);
        }
        committed.active_service_session = None;
        let match_id = format!("local-{}", Uuid::new_v4());
        let identity_value = local::history_identity(id);
        committed.session_bindings.insert(view["session_id"].as_str().expect("Session ID").to_owned(), SessionBinding {
            context: json!({"match_id": match_id, "metadata_ref": {"identity": identity_value}, "metadata_video_id": id,
                "episode_ref": {"source_id": "local"}}),
            playback_epoch: None,
            service: None,
        });
        let updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1)
            .max(1);
        Ok(json!({"session": view, "match_id": match_id, "record": {
            "content_id": identity("metadata", &["local", "file", id]), "metadata_identity": identity_value, "video_id": id,
            "season_number": null, "episode_number": null, "title": if title.is_empty() { id.to_owned() } else { title },
            "kind": "other", "poster_cache_key": null, "source_id": "local", "source_name": "Local video",
            "position_ms": 0, "duration_ms": null, "completed": false, "updated_at": updated}}))
    }

    fn live_channels(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let config_id = input["config_id"].as_str().expect("Config UUID");
        let config = {
            let committed = self
                .committed
                .lock()
                .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
            live_config(&committed.settings, config_id)?
        };
        // Download outside the commit lock; publish only if the configuration is unchanged.
        let url = maccms::http_url(config["url"].as_str().expect("Validated URL"), true)?;
        let response = maccms::Http::default().get(
            url,
            reqwest::header::HeaderMap::new(),
            job,
            live::LIST_LIMIT,
            false,
            true,
        )?;
        let text = std::str::from_utf8(&response.body)
            .map_err(|_| ApiError::new("UPSTREAM_FAILED", "Live list is not UTF-8 text"))?;
        let (format, channels) = live::parse(text)?;
        let digest = format!(
            "{:x}",
            <sha2::Sha256 as sha2::Digest>::digest(&response.body)
        );
        let revision = identity("live_list", &[config_id, &digest]);
        let mut committed = self
            .committed
            .lock()
            .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
        job.check()?;
        if live_config(&committed.settings, config_id).ok() != Some(config) {
            return Err(ApiError::cancelled());
        }
        let view: Vec<Value> = channels
            .iter()
            .enumerate()
            .map(|(i, c)| c.view(i))
            .collect();
        committed
            .live
            .insert(config_id.into(), (revision.clone(), channels));
        Ok(
            json!({"config_id": config_id, "revision": revision, "format": format, "channels": view}),
        )
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
        let (captured, published) = {
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
            let published = committed.stremio.contains_key(&captured.source_id)
                || committed.maccms.contains_key(&captured.source_id);
            (captured, published)
        };
        if kind == SourceKind::Stremio {
            let cached = (!published)
                .then(|| self.source_cache.load(&captured.config))
                .flatten()
                .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok());
            let mut used_cache = cached.is_some();
            let mut result = stremio::Catalog::sync_cached(
                captured.clone(),
                &maccms::Http::default(),
                job,
                cached.as_deref(),
            );
            if used_cache && result.is_err() && job.check().is_ok() {
                used_cache = false;
                result = stremio::Catalog::sync_cached(
                    captured.clone(),
                    &maccms::Http::default(),
                    job,
                    None,
                );
            }
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
            let (catalog, bodies) = match result {
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
            let sources = committed.sources.sources();
            drop(committed);
            if !used_cache
                && let Err(error) = self.source_cache.save(&captured.config, json!(bodies))
            {
                eprintln!("Source bootstrap cache write failed: {error}");
            }
            return Ok(
                json!({"kind": kind, "config_id": captured.config["config_id"], "changed": changed, "sources": sources, "cloudstream_result": null}),
            );
        }
        let cached = (!published)
            .then(|| self.source_cache.load(&captured.config))
            .flatten();
        let mut used_cache = cached.is_some();
        let mut result = maccms::Catalog::sync_cached(
            captured.clone(),
            &maccms::Http::default(),
            job,
            cached.as_ref(),
        );
        if used_cache && result.is_err() && job.check().is_ok() {
            used_cache = false;
            result =
                maccms::Catalog::sync_cached(captured.clone(), &maccms::Http::default(), job, None);
        }
        let mut committed = self
            .committed
            .lock()
            .map_err(|_| ApiError::internal("Host commit lock poisoned"))?;
        job.check()?;
        committed
            .sources
            .check_snapshot(&captured, &self.host_epoch, None, true)?;
        let (mut catalog, bootstrap) = result?;
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
        let sources = committed.sources.sources();
        drop(committed);
        if !used_cache && let Err(error) = self.source_cache.save(&captured.config, bootstrap) {
            eprintln!("Source bootstrap cache write failed: {error}");
        }
        Ok(
            json!({"kind": kind, "config_id": captured.config["config_id"],
            "changed": changed, "sources": sources, "cloudstream_result": null}),
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
                let language = crate::settings::metadata_language(&committed.settings);
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
                // The built-in TMDB source can translate an alias or English title for MacCMS search.
                let aliases = (capability == "search")
                    .then(|| identity("http_source", &["stremio", crate::settings::TMDB_CONFIG_ID]))
                    .and_then(|id| committed.stremio.get(&id).cloned())
                    .map(|tmdb| {
                        (
                            tmdb,
                            crate::settings::metadata_language(&committed.settings),
                        )
                    });
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
                    approximate_search(
                        &catalog,
                        &http,
                        key["query"].as_str().expect("Query"),
                        aliases.as_ref(),
                        job,
                    )
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
                        .filter(|details| {
                            crate::fuzzy::related(title, details["title"].as_str().unwrap_or(""))
                        })
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
        let owner_for = |reference: &Value| {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(reference.to_string().as_bytes()))
        };
        let mut owners = std::collections::HashMap::new();
        if let Some(reference) = value.get("details").and_then(|v| v.get("ref")) {
            let owner = owner_for(reference);
            let keys: Vec<_> = references
                .values()
                .map(|image| crate::media_cache::key(source, "image", &image.url, &image.headers))
                .collect();
            if self.media_cache.register_images(&owner, &keys).is_ok() {
                for id in references.keys() {
                    owners.insert(id.clone(), owner.clone());
                }
            }
        } else if let Some(sections) = value.pointer("/page/sections").and_then(Value::as_array) {
            for card in sections
                .iter()
                .flat_map(|section| section["items"].as_array().into_iter().flatten())
            {
                let Some(reference) = card.get("ref") else {
                    continue;
                };
                let owner = owner_for(reference);
                let mut ids = Vec::new();
                crate::details::images(card, &mut |image| {
                    ids.push(image["resource_id"].as_str().expect("Image ID").to_owned());
                    Ok(())
                })?;
                let keys: Vec<_> = ids
                    .iter()
                    .filter_map(|id| references.get(id))
                    .map(|image| {
                        crate::media_cache::key(source, "image", &image.url, &image.headers)
                    })
                    .collect();
                if self.media_cache.register_images(&owner, &keys).is_ok() {
                    for id in ids {
                        owners.insert(id, owner.clone());
                    }
                }
            }
        }
        let mut access = serde_json::Map::new();
        for (id, image) in references {
            let owner = owners.get(&id).cloned();
            let cache_key = owner
                .as_ref()
                .map(|_| crate::media_cache::key(source, "image", &image.url, &image.headers));
            access.insert(
                id,
                committed
                    .gateway
                    .as_ref()
                    .expect("Image gateway")
                    .image_cached(source, image, cache_key, owner)?,
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

    fn start_line_round(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let round_id = input["round_id"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Round ID missing"))?;
        Uuid::parse_str(round_id).map_err(|_| ApiError::bad("Round ID must be UUID"))?;
        let metadata_source_id = input["metadata_source_id"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Metadata source missing"))?;
        let kind = input["type"]
            .as_str()
            .filter(|v| matches!(*v, "movie" | "series"))
            .ok_or_else(|| ApiError::bad("Invalid metadata type"))?;
        let meta_id = input["meta_id"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ApiError::bad("Metadata ID missing"))?;
        let video_id = input["video_id"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| ApiError::bad("Video ID missing"))?;
        let service = crate::service::current()?;
        let (snapshot, catalog, reference, language) = {
            let mut committed = self.committed.lock().expect("Host commit lock");
            let settings = &committed.settings;
            let configured = &settings["playback_service"];
            if configured["enabled"] != true {
                return Err(ApiError::new(
                    "ENGINE_NOT_READY",
                    "Playback service is not configured",
                ));
            }
            let (config_id, addon_key) = if metadata_source_id == "tmdb" {
                (
                    crate::settings::TMDB_CONFIG_ID.to_owned(),
                    identity("addon", &[crate::settings::TMDB_CONFIG_ID]),
                )
            } else {
                configured["groups"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|group| group["status"] == "ready")
                    .find_map(|group| {
                        let addon = group["addons"]
                            .as_array()?
                            .iter()
                            .find(|addon| addon["source_id"] == metadata_source_id)?;
                        Some((
                            group["config_id"].as_str()?.to_owned(),
                            addon["addon_key"].as_str()?.to_owned(),
                        ))
                    })
                    .ok_or_else(|| ApiError::bad("Metadata source is not registered"))?
            };
            let language = if crate::settings::metadata_language(settings) == "zh-CN" {
                "zh-CN"
            } else {
                "en-US"
            };
            let snapshot = committed
                .sources
                .snapshot(&config_id, &self.host_epoch, false)?;
            if snapshot.config["service_revision"].as_str() != Some(service.fingerprint().as_str())
            {
                return Err(ApiError::new(
                    "REVISION_MISMATCH",
                    "Playback service configuration changed",
                ));
            }
            let catalog = committed
                .stremio
                .get(&snapshot.source_id)
                .cloned()
                .ok_or_else(|| ApiError::new("ENGINE_NOT_READY", "Synchronize metadata first"))?;
            let reference = json!({"source_id":snapshot.source_id,"source_revision":snapshot.source_revision,"data":json!(["stremio",addon_key,kind,meta_id]).to_string()});
            (snapshot, catalog, reference, language.to_owned())
        };
        job.bind_source(&snapshot.source_id);
        let detail = catalog.details(&maccms::Http::default(), &reference, &language, job)?;
        let identity = &detail.value["metadata_ref"]["identity"];
        if identity["config_id"] != snapshot.config["config_id"]
            || identity["type"] != kind
            || identity["meta_id"] != meta_id
        {
            return Err(ApiError::bad("Metadata identity is not registered"));
        }
        matching::video(&detail.value, video_id)?;
        let metadata_ref = detail.value["metadata_ref"].clone();
        let mut committed = self.committed.lock().expect("Host commit lock");
        job.check()?;
        committed
            .sources
            .check_snapshot(&snapshot, &self.host_epoch, None, false)?;
        if committed.line_rounds.records.contains_key(round_id) {
            return Err(ApiError::bad("Round ID already used"));
        }
        if let Some(old) = committed
            .line_rounds
            .active
            .take()
            .and_then(|id| committed.line_rounds.records.remove(&id))
        {
            line_round::stop(&old, &self.requests);
        }
        let job_id = Uuid::new_v4().to_string();
        let parent = Arc::new(
            self.requests
                .begin(&job_id, "resolve", self.clock.clone())?,
        );
        parent.bind_source(&snapshot.source_id);
        let target = json!({"metadata_source_id":metadata_source_id,"meta_id":meta_id,"type":kind,"video_id":video_id,"lang":language,"sources":null,"refresh":false});
        let round = Arc::new(Mutex::new(Round {
            id: round_id.into(),
            target: target.clone(),
            metadata_ref,
            metadata_source_id: metadata_source_id.into(),
            metadata_snapshot: snapshot,
            lookup_id: None,
            sources: Default::default(),
            candidates: Default::default(),
            lines: Default::default(),
            query_done: false,
            pending: 0,
            pending_confirm: 0,
            selected: None,
            error: None,
            deadline: std::time::Instant::now() + line_round::BUDGET,
            stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            job: parent,
            job_id,
            service,
            permits: Arc::new(tokio::sync::Semaphore::new(4)),
        }));
        line_round::launch(round.clone(), target, false)?;
        committed.line_rounds.active = Some(round_id.into());
        committed
            .line_rounds
            .records
            .insert(round_id.into(), round.clone());
        let result = round.lock().expect("round state").snapshot();
        Ok(result)
    }

    fn get_line_round(&self, input: &Value) -> Result<Value, ApiError> {
        let round = {
            self.committed
                .lock()
                .expect("Host commit lock")
                .line_rounds
                .get(input["round_id"].as_str().unwrap_or(""))?
        };
        let value = round
            .lock()
            .map_err(|_| ApiError::internal("Round state unavailable"))?
            .snapshot();
        Ok(value)
    }

    fn confirm_line_candidate(&self, input: &Value) -> Result<Value, ApiError> {
        let round = {
            self.committed
                .lock()
                .expect("Host commit lock")
                .line_rounds
                .get(input["round_id"].as_str().unwrap_or(""))?
        };
        let body = {
            let mut state = round
                .lock()
                .map_err(|_| ApiError::internal("Round state unavailable"))?;
            let candidate_id = input["candidate_id"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Candidate missing"))?;
            let source_id = input["source_id"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Source missing"))?;
            if state
                .candidates
                .get(candidate_id)
                .is_none_or(|candidate| candidate["source_id"] != source_id)
            {
                return Err(ApiError::bad("Candidate does not belong to round"));
            }
            let lookup = state
                .lookup_id
                .clone()
                .ok_or_else(|| ApiError::new("ENGINE_NOT_READY", "Lookup has not started"))?;
            if state.pending_confirm >= 16 {
                return Err(ApiError::new("RESOURCE_LIMIT", "Too many confirmations"));
            }
            state.pending_confirm += 1;
            state.extend();
            json!({"lookup_id":lookup,"source_id":source_id,"candidate_id":candidate_id})
        };
        if let Err(error) = line_round::launch(round.clone(), body, true) {
            round.lock().expect("round state").pending_confirm -= 1;
            return Err(error);
        }
        let value = round.lock().expect("round state").snapshot();
        Ok(value)
    }

    fn cancel_line_round(&self, input: &Value) -> Result<Value, ApiError> {
        let id = input["round_id"].as_str().unwrap_or("");
        let round = {
            let mut committed = self.committed.lock().expect("Host commit lock");
            let round = committed.line_rounds.get(id)?;
            committed.line_rounds.records.remove(id);
            committed.line_rounds.active = None;
            round
        };
        line_round::stop(&round, &self.requests);
        Ok(json!({"ok":true}))
    }

    fn verify_service_source(
        &self,
        config: &crate::service::Config,
        id: &str,
        revision: u64,
        job: &Job,
    ) -> Result<(), ApiError> {
        let sources = crate::service::blocking_json(
            config,
            reqwest::Method::GET,
            &["v1", "sources"],
            None,
            job,
        )?;
        if sources["sources"].as_array().is_some_and(|sources| {
            sources.iter().any(|source| {
                source["id"] == id && source["revision"] == revision && source["enabled"] == true
            })
        }) {
            Ok(())
        } else {
            Err(ApiError::new(
                "REVISION_MISMATCH",
                "Playback source changed",
            ))
        }
    }

    fn play_line(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let round_id = input["round_id"].as_str().unwrap_or("");
        let line_id = input["line_id"].as_str().unwrap_or("");
        let round = {
            self.committed
                .lock()
                .expect("Host commit lock")
                .line_rounds
                .get(round_id)?
        };
        // The service reuses a playback address only until `expires_at`. Renew the
        // chosen line in place; a line the service no longer holds answers 410 and,
        // like a refreshed line that fails its probe, sends the App back to a new search.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let stale = round
            .lock()
            .expect("round state")
            .lines
            .get(line_id)
            .is_some_and(|line| {
                line.status == "ready"
                    && line.raw["expires_at"]
                        .as_i64()
                        .is_some_and(|time| time <= now)
            });
        if stale {
            self.refresh_line(input, job)?;
            let ready = round
                .lock()
                .expect("round state")
                .lines
                .get(line_id)
                .is_some_and(|line| line.status == "ready");
            if !ready {
                return Err(ApiError::new(
                    "SESSION_EXPIRED",
                    "Refreshed line is unavailable; search again",
                ));
            }
        }
        let (line, service, snapshot, metadata_ref, video_id) = {
            let state = round.lock().expect("round state");
            let line = state
                .lines
                .get(line_id)
                .filter(|line| line.status == "ready")
                .cloned()
                .ok_or_else(|| ApiError::bad("Line is not ready"))?;
            if line.raw["expires_at"]
                .as_i64()
                .is_some_and(|time| time <= time::OffsetDateTime::now_utc().unix_timestamp())
            {
                return Err(ApiError::new("SESSION_EXPIRED", "Line needs refresh"));
            }
            (
                line,
                state.service.clone(),
                state.metadata_snapshot.clone(),
                state.metadata_ref.clone(),
                state.target["video_id"].clone(),
            )
        };
        let source_id = line.raw["source_id"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Invalid line source"))?;
        let revision = line.raw["source_revision"]
            .as_u64()
            .ok_or_else(|| ApiError::bad("Invalid line source"))?;
        let selected = crate::service::blocking_json(
            &service,
            reqwest::Method::POST,
            &["v1", "lines", line_id, "play"],
            Some(json!({})),
            job,
        )?;
        if selected["line_id"] != line_id
            || selected["source_id"] != source_id
            || selected["source_revision"] != revision
            || selected["target"] != line.raw["target"]
        {
            return Err(ApiError::bad("Selected line identity changed"));
        }
        let url = selected["play"]["url"]
            .as_str()
            .ok_or_else(|| ApiError::bad("Selected playback URL missing"))?;
        maccms::http_url(url, false)?;
        let headers = crate::line_round::header_map(&selected["play"]["headers"])?;
        let mut committed = self.committed.lock().expect("Host commit lock");
        job.check()?;
        committed.line_rounds.get(round_id)?;
        committed
            .sources
            .check_snapshot(&snapshot, &self.host_epoch, None, false)?;
        if crate::service::current()?.fingerprint() != service.fingerprint() {
            return Err(ApiError::new(
                "REVISION_MISMATCH",
                "Playback service changed",
            ));
        }
        {
            let mut state = round.lock().expect("round state");
            if state.lines.get(line_id).is_none_or(|current| {
                current.status != "ready"
                    || current.raw != line.raw
                    || current.raw["source_revision"] != revision
            }) {
                return Err(ApiError::cancelled());
            }
            state.selected = Some(line_id.into());
        }
        let provider = identity("service_source", &[source_id]);
        let source_revision = identity(
            "service_revision",
            &[source_id, &revision.to_string(), &service.fingerprint()],
        );
        let reference = json!({"source_id":provider,"source_revision":source_revision,"data":json!(["service_line",round_id,line_id]).to_string()});
        let link = resolution::Link {
            original: json!({}),
            provider: provider.clone(),
            url: url.to_owned(),
            headers,
            referer: String::new(),
            extractor_data: None,
            interceptor: None,
            cookies: None,
            label: line.raw["line_name"].as_str().unwrap_or("Line").to_owned(),
            quality: None,
            quality_label: None,
            kind: if line.raw["kind"] == "hls" {
                "hls"
            } else {
                "progressive"
            }
            .into(),
            mime_type: None,
            playable: true,
            reason: None,
        };
        let resolution =
            committed
                .resolutions
                .register(&reference, vec![link], vec![], self.clock.now_ms())?;
        let context = json!({"metadata_ref":metadata_ref,"metadata_video_id":video_id,"match_id":Uuid::new_v4().to_string(),"content_ref":reference,"group_id":identity("service_group",&[line_id]),"episode_id":identity("service_episode",&[line_id]),"episode_ref":reference,"selected_stream_id":null});
        Contracts::get().validate("PlaybackContext", &context)?;
        let id = resolution["id"].as_str().expect("Resolution ID").to_owned();
        if let Some(old) = committed.current_resolution.replace(id.clone()) {
            committed.resolutions.release(&old);
            committed.resolution_contexts.remove(&old);
            committed.service_resolutions.remove(&old);
        }
        committed
            .resolution_contexts
            .insert(id.clone(), context.clone());
        committed.service_resolutions.insert(
            id,
            ServicePlayback {
                source_id: source_id.into(),
                source_revision: revision,
                line_id: line_id.into(),
                latency_ms: line.latency_ms,
                config: service,
                reported: Default::default(),
                metadata_snapshot: snapshot,
            },
        );
        Ok(json!({"resolution":resolution,"context":context}))
    }

    fn refresh_line(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let round_id = input["round_id"].as_str().unwrap_or("");
        let line_id = input["line_id"].as_str().unwrap_or("");
        let round = {
            self.committed
                .lock()
                .expect("Host commit lock")
                .line_rounds
                .get(round_id)?
        };
        // The service answers 410 once its line handle has expired; the App then searches again.
        let (service, old) = {
            let state = round.lock().expect("round state");
            if state
                .lines
                .get(line_id)
                .is_some_and(|line| line.status == "testing")
            {
                return Err(ApiError::new(
                    "ENGINE_NOT_READY",
                    "Line probe is still running",
                ));
            }
            (
                state.service.clone(),
                state
                    .lines
                    .get(line_id)
                    .cloned()
                    .ok_or_else(|| ApiError::bad("Line not in round"))?,
            )
        };
        let value = crate::service::blocking_json(
            &service,
            reqwest::Method::POST,
            &["v1", "lines", line_id, "refresh"],
            Some(json!({})),
            job,
        )?;
        if value["line_id"] != line_id
            || value["source_id"] != old.raw["source_id"]
            || value["source_revision"] != old.raw["source_revision"]
            || value["target"] != old.raw["target"]
        {
            return Err(ApiError::bad("Refreshed line identity changed"));
        }
        {
            let mut state = round.lock().expect("round state");
            state.lines.remove(line_id);
            state.extend();
        }
        let permits = { round.lock().expect("round state").permits.clone() };
        let refreshed = (|| {
            line_round::event(&round, json!({"event":"line","line":value}), &permits, true)?;
            let state = round.lock().expect("round state");
            if state.stop.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(ApiError::cancelled());
            }
            if std::time::Instant::now() >= state.deadline {
                return Err(ApiError::new(
                    "SESSION_EXPIRED",
                    "Line round expired; search again",
                ));
            }
            Ok::<(), ApiError>(())
        })();
        if let Err(error) = refreshed {
            round
                .lock()
                .expect("round state")
                .lines
                .insert(line_id.into(), old);
            return Err(error);
        }
        let value = round.lock().expect("round state").snapshot();
        Ok(value)
    }

    fn open_service_playback(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let id = input["resolution_id"].as_str().unwrap_or("");
        let stream = input["stream_id"].as_str().unwrap_or("");
        let (binding, context) = {
            let mut committed = self.committed.lock().expect("Host commit lock");
            committed
                .resolutions
                .selected(id, stream, self.clock.now_ms())?;
            (
                committed
                    .service_resolutions
                    .get(id)
                    .cloned()
                    .ok_or_else(|| ApiError::new("SESSION_EXPIRED", "Resolution expired"))?,
                committed
                    .resolution_contexts
                    .get(id)
                    .cloned()
                    .ok_or_else(|| ApiError::new("SESSION_EXPIRED", "Playback context expired"))?,
            )
        };
        if crate::service::current()?.fingerprint() != binding.config.fingerprint() {
            return Err(ApiError::new(
                "REVISION_MISMATCH",
                "Playback service changed",
            ));
        }
        self.verify_service_source(
            &binding.config,
            &binding.source_id,
            binding.source_revision,
            job,
        )?;
        let mut committed = self.committed.lock().expect("Host commit lock");
        job.check()?;
        committed.sources.check_snapshot(
            &binding.metadata_snapshot,
            &self.host_epoch,
            None,
            false,
        )?;
        if committed.service_resolutions.get(id).is_none_or(|current| {
            current.line_id != binding.line_id || current.source_revision != binding.source_revision
        }) {
            return Err(ApiError::cancelled());
        }
        let selected = committed
            .resolutions
            .selected(id, stream, self.clock.now_ms())?;
        if committed.gateway.is_none() {
            committed.gateway = Some(Gateway::with_media_cache(
                self.clock.clone(),
                self.media_cache.clone(),
            )?);
        }
        let gateway = committed.gateway.as_ref().expect("Gateway ready");
        let view = if let Some(owner) = cache_episode_owner(&context) {
            gateway.open_episode(selected, owner)?
        } else {
            gateway.open(selected)?
        };
        if let Err(error) = job.check() {
            gateway.close(view["session_id"].as_str().expect("Session ID"));
            return Err(error);
        }
        let session = view["session_id"].as_str().expect("Session ID").to_owned();
        committed.active_service_session = Some(session.clone());
        committed.session_bindings.insert(
            session,
            SessionBinding {
                context,
                playback_epoch: None,
                service: Some(binding),
            },
        );
        Ok(view)
    }

    fn report_line_playback(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let session = input["session_id"].as_str().unwrap_or("");
        let epoch = input["playback_epoch"]
            .as_u64()
            .ok_or_else(|| ApiError::bad("Playback epoch missing"))?;
        let outcome = input["outcome"]
            .as_str()
            .filter(|value| matches!(*value, "playing" | "failed"))
            .ok_or_else(|| ApiError::bad("Invalid playback outcome"))?;
        let binding = {
            let mut committed = self.committed.lock().expect("Host commit lock");
            if committed.active_service_session.as_deref() != Some(session) {
                return Err(ApiError::cancelled());
            }
            let live = committed.gateway.as_ref().is_some_and(|gateway| {
                gateway
                    .status("playback", session)
                    .is_ok_and(|status| status["state"] != "expired")
            });
            let binding = committed
                .session_bindings
                .get_mut(session)
                .filter(|_| live)
                .ok_or_else(|| ApiError::new("SESSION_EXPIRED", "Playback session closed"))?;
            if binding.playback_epoch.is_some_and(|known| known != epoch) {
                return Err(ApiError::cancelled());
            }
            binding.playback_epoch = Some(epoch);
            let service = binding
                .service
                .as_mut()
                .ok_or_else(|| ApiError::bad("Session is not a service line"))?;
            if !service.reported.insert(outcome.to_owned()) {
                return Ok(json!({"ok":true,"reported":false}));
            }
            service.clone()
        };
        let payload = json!({"line_id":binding.line_id,"outcome":outcome,"latency_ms":binding.latency_ms,"error_code":input["error_code"]});
        let result = crate::service::blocking_json(
            &binding.config,
            reqwest::Method::POST,
            &["v1", "reports"],
            Some(payload),
            job,
        );
        match result {
            Ok(_) => Ok(json!({"ok":true,"reported":true})),
            Err(_) => Ok(
                json!({"ok":true,"reported":false,"diagnostic":"Playback service report failed"}),
            ),
        }
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
            committed.resolution_contexts.remove(&old);
            committed.service_resolutions.remove(&old);
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

/// An enabled live list configuration, by id.
fn live_config(settings: &Value, config_id: &str) -> Result<Value, ApiError> {
    settings["live_configs"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|config| config["config_id"] == config_id && config["enabled"] == true)
        .cloned()
        .ok_or_else(|| {
            ApiError::new(
                "SOURCE_NOT_FOUND",
                "Live list is not configured or disabled",
            )
        })
}

/// A live channel becomes a one-stream record; HLS unless the address names a file type.
fn live_record(
    config_id: &str,
    revision: &str,
    index: usize,
    channel: &live::Channel,
) -> resolution::LinkRecord {
    let path = channel
        .url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let progressive = [
        ".mp4", ".m4v", ".mov", ".webm", ".flv", ".ts", ".mp3", ".aac",
    ]
    .iter()
    .any(|e| path.ends_with(e));
    let provider = format!("live:{config_id}");
    resolution::LinkRecord {
        stream_id: "live".into(),
        resolution_id: format!("live-{}", Uuid::new_v4()),
        link: resolution::Link {
            original: Value::Null,
            provider: provider.clone(),
            url: channel.url.clone(),
            headers: Default::default(),
            referer: String::new(),
            extractor_data: None,
            interceptor: None,
            cookies: None,
            label: channel.name.clone(),
            quality: None,
            quality_label: None,
            kind: if progressive { "progressive" } else { "hls" }.into(),
            mime_type: None,
            playable: true,
            reason: None,
        },
        subtitles: Arc::new(Vec::new()),
        episode_ref: json!({"source_id": provider, "source_revision": revision, "data": index.to_string()}),
        source_revision: revision.into(),
    }
}

/// A checked local file becomes a one-stream record the gateway serves from disk.
fn local_record(id: &str, path: &std::path::Path, title: &str) -> resolution::LinkRecord {
    resolution::LinkRecord {
        stream_id: "file".into(),
        resolution_id: format!("local-{}", Uuid::new_v4()),
        link: resolution::Link {
            original: Value::Null,
            provider: "local".into(),
            url: path.to_string_lossy().into_owned(),
            headers: Default::default(),
            referer: String::new(),
            extractor_data: None,
            interceptor: None,
            cookies: None,
            label: title.into(),
            quality: None,
            quality_label: None,
            kind: "progressive".into(),
            mime_type: Some(local::mime(path).into()),
            playable: true,
            reason: None,
        },
        subtitles: Arc::new(Vec::new()),
        episode_ref: json!({"source_id": "local", "source_revision": "local", "data": id}),
        source_revision: "local".into(),
    }
}

/// MacCMS `wd` only matches substrings: try the typed query, then relaxed forms, then titles
/// the built-in TMDB source finds for it; results are ranked by closeness to the typed text.
fn approximate_search(
    catalog: &maccms::Catalog,
    http: &maccms::Http,
    typed: &str,
    aliases: Option<&(stremio::Catalog, &'static str)>,
    job: &Job,
) -> Result<maccms::Mapped, ApiError> {
    let finish = |mut mapped: maccms::Mapped| {
        if let Some(items) = mapped.value.as_array_mut() {
            crate::fuzzy::rank(typed, items);
        }
        mapped
    };
    let mut last = None;
    for query in crate::fuzzy::relaxed(typed) {
        let mapped = catalog.search(http, &query, 1, job)?;
        if mapped
            .value
            .as_array()
            .is_some_and(|items| !items.is_empty())
        {
            return Ok(finish(mapped));
        }
        last = Some(mapped);
    }
    // Best effort: an unreachable TMDB leaves the (empty) MacCMS answer as it is.
    if let Some((tmdb, _)) = aliases {
        let mut titles: Vec<String> = Vec::new();
        if let Ok(child) = job.limited_child(&Uuid::new_v4().to_string(), "default_metadata", 5_000)
        {
            for language in ["zh-CN", "en"] {
                if let Ok(found) = tmdb.search(http, typed, language, &child) {
                    for item in found.value["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .take(3)
                    {
                        let title = item["title"].as_str().unwrap_or("").to_owned();
                        if !title.is_empty()
                            && !titles.contains(&title)
                            && crate::fuzzy::normalize(&title) != crate::fuzzy::normalize(typed)
                        {
                            titles.push(title);
                        }
                    }
                }
            }
        }
        for title in titles {
            for query in crate::fuzzy::relaxed(&title).into_iter().take(3) {
                let mapped = catalog.search(http, &query, 1, job)?;
                if mapped
                    .value
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                {
                    let mut mapped = mapped;
                    if let Some(items) = mapped.value.as_array_mut() {
                        crate::fuzzy::rank(&title, items);
                    }
                    return Ok(mapped);
                }
            }
        }
    }
    Ok(last.unwrap_or(maccms::Mapped {
        value: json!([]),
        images: Vec::new(),
    }))
}
