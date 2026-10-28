use crate::{
    contracts::{ApiError, COMMANDS, Contracts, envelope, wire_size},
    gateway::Gateway,
    requests::{Clock, Job, MonotonicClock, Requests},
    settings::SettingsFile,
    source_dispatch::{SourceDispatch, SourceKind, SourceSnapshot},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[path = "maccms.rs"]
pub mod maccms;

struct Committed {
    settings: Value,
    sources: SourceDispatch,
    gateway: Option<Gateway>,
    maccms: std::collections::HashMap<String, maccms::Catalog>,
}

pub struct Host {
    pub host_epoch: String,
    pub requests: Arc<Requests>,
    pub clock: Arc<dyn Clock>,
    store: SettingsFile,
    committed: Mutex<Committed>,
}

pub struct Call {
    command: String,
    args: Value,
    job: Option<Job>,
}

impl Host {
    pub fn open(path: PathBuf) -> Result<Arc<Self>, ApiError> {
        Self::with_clock(path, Arc::new(MonotonicClock::default()))
    }

    pub fn with_clock(path: PathBuf, clock: Arc<dyn Clock>) -> Result<Arc<Self>, ApiError> {
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
            committed: Mutex::new(Committed {
                settings,
                sources,
                gateway: None,
                maccms: Default::default(),
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
            Ok(job) => Ok(Call {
                command: command.into(),
                args,
                job,
            }),
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
        let result = self.execute_inner(&call);
        let unsafe_target = result
            .as_ref()
            .err()
            .is_some_and(|error| error.message.starts_with("unsafe_target:"));
        let mut response = envelope(&call.args["request_id"], &self.host_epoch, result);
        if unsafe_target {
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
            "get_home" if input["page"] == 1 && input["section_ref"].is_null() => {
                let committed = self.committed.lock().expect("Host commit lock");
                committed
                    .sources
                    .source(input["source_id"].as_str().unwrap_or(""), None)
                    .ok()
                    .and_then(|source| source["kind"].as_str())
                    .map(|kind| format!("{kind}_home"))
                    .unwrap_or_else(|| "default_metadata".into())
            }
            _ => "default_metadata".into(),
        }
    }

    fn execute_inner(&self, call: &Call) -> Result<Value, ApiError> {
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
            "sync_catalog" => return self.sync_catalog(input, job),
            "get_home" => return self.route_content(input, job, "home"),
            "get_details" => return self.route_content(&input["ref"], job, "details"),
            "resolve_episode" => return self.route_content(&input["episode_ref"], job, "resolve"),
            "match_playback" => {
                self.route_content(&input["metadata_ref"]["content_ref"], job, "details")?;
                return self.route_content(&input["playback_source"], job, "resolve");
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
                        "ENGINE_NOT_READY",
                        "Cloudstream bridge preparation is not connected",
                    ));
                }
                if committed.gateway.is_none() {
                    committed.gateway = Some(Gateway::start()?);
                }
                self.runtime(&committed)
            }
            "get_sources" => Ok(committed.sources.sources()),
            "get_settings" => Ok(committed.settings.clone()),
            "save_settings" => {
                job.check()?;
                // After replacement starts, finish publication even if cancellation arrives.
                self.store.save_checked(input, || job.check())?;
                let affected = committed.sources.replace_configs(
                    input["source_configs"]
                        .as_array()
                        .expect("Validated configs"),
                );
                committed.settings = input.clone();
                committed.maccms.retain(|id, _| !affected.contains(id));
                self.requests.cancel_sources(&affected);
                return Ok(committed.settings.clone());
            }
            "start_interaction" | "finish_interaction" => Err(ApiError::new(
                "ENGINE_NOT_READY",
                "Cloudstream bridge is stopped",
            )),
            "open_playback" | "close_playback" | "release_resolution" | "get_resource_status" => {
                Err(ApiError::new(
                    "ENGINE_NOT_READY",
                    "Resource service is not connected",
                ))
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

    fn sync_catalog(&self, input: &Value, job: &Job) -> Result<Value, ApiError> {
        let kind = SourceKind::parse(&input["kind"])?;
        if kind == SourceKind::Cloudstream {
            return Err(ApiError::new(
                "ENGINE_NOT_READY",
                "Cloudstream bridge is stopped",
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
            return Err(ApiError::new(
                "ENGINE_NOT_READY",
                "HTTP source adapter has not published a catalog",
            ));
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

    fn route_content(&self, key: &Value, job: &Job, capability: &str) -> Result<Value, ApiError> {
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
        match SourceKind::parse(&source["kind"])? {
            SourceKind::Stremio if capability == "resolve" => Err(ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "Stremio provides metadata only",
            )),
            SourceKind::Cloudstream => Err(ApiError::new(
                "ENGINE_NOT_READY",
                "Cloudstream bridge is stopped",
            )),
            SourceKind::Stremio => Err(ApiError::new(
                "ENGINE_NOT_READY",
                "HTTP source adapter is not ready",
            )),
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
                // Match registration and ResolutionStore are connected in T08.
                if capability == "resolve" {
                    return Err(ApiError::bad("Playback match is not registered"));
                }
                drop(committed);
                let http = maccms::Http::default();
                let result = if capability == "home" {
                    catalog.home(&http, key, job).map(|home| {
                        (
                            home.mapped,
                            key["section_ref"].is_null().then_some(home.sections),
                        )
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
                let (mapped, sections) = result?;
                let current = committed
                    .maccms
                    .get_mut(&captured.source_id)
                    .ok_or_else(ApiError::cancelled)?;
                current.register_images(mapped.images)?;
                if let Some(sections) = sections {
                    current.sections = sections;
                }
                Ok(mapped.value)
            }
        }
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
