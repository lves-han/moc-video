use crate::contracts::{ApiError, Contracts};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Cloudstream,
    Maccms,
    Stremio,
}

impl SourceKind {
    pub fn parse(value: &Value) -> Result<Self, ApiError> {
        serde_json::from_value(value.clone()).map_err(|_| ApiError::bad("Unknown source kind"))
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cloudstream => "cloudstream",
            Self::Maccms => "maccms",
            Self::Stremio => "stremio",
        }
    }
}

pub fn identity(tag: &str, fields: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(tag.as_bytes());
    hash.update([0]);
    for field in fields {
        hash.update((field.len() as u32).to_be_bytes());
        hash.update(field.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

#[derive(Clone)]
struct Entry {
    config: Value,
    generation: u64,
    sync_sequence: u64,
    source: Value,
}

#[derive(Clone, Debug)]
pub struct SourceSnapshot {
    pub host_epoch: String,
    pub guest_epoch: Option<String>,
    pub config: Value,
    pub generation: u64,
    pub sync_sequence: u64,
    pub source_id: String,
    pub source_revision: String,
}

#[derive(Default)]
pub struct SourceDispatch {
    entries: HashMap<Uuid, Entry>,
    tombstones: HashMap<Uuid, u64>,
    order: Vec<Uuid>,
}

impl SourceDispatch {
    pub fn replace_configs(&mut self, configs: &[Value]) -> Vec<String> {
        let mut affected = Vec::new();
        let mut next = HashMap::new();
        let mut order = Vec::new();
        for config in configs {
            let id = Uuid::parse_str(config["config_id"].as_str().expect("Validated config UUID"))
                .expect("Validated config UUID");
            order.push(id);
            if let Some(old) = self.entries.remove(&id) {
                if old.config == *config {
                    next.insert(id, old);
                    continue;
                }
                affected.push(old.source["id"].as_str().expect("Source id").into());
            }
            let generation = self.tombstones.entry(id).or_default();
            *generation += 1;
            next.insert(
                id,
                Entry {
                    config: config.clone(),
                    generation: *generation,
                    sync_sequence: 0,
                    source: configured_source(config),
                },
            );
        }
        for (id, old) in self.entries.drain() {
            affected.push(old.source["id"].as_str().expect("Source id").into());
            *self.tombstones.entry(id).or_default() += 1;
        }
        self.entries = next;
        self.order = order;
        affected
    }

    pub fn sources(&self) -> Value {
        json!({"providers": self.order.iter().map(|id| self.entries[id].source.clone()).collect::<Vec<_>>(), "plugins": []})
    }

    pub fn source(&self, id: &str, revision: Option<&str>) -> Result<&Value, ApiError> {
        let entry = self
            .entries
            .values()
            .find(|entry| entry.source["id"] == id)
            .ok_or_else(|| ApiError::new("SOURCE_NOT_FOUND", "Source is not registered"))?;
        if revision.is_some_and(|revision| entry.source["source_revision"] != revision) {
            return Err(ApiError::new(
                "REVISION_MISMATCH",
                "Source revision changed",
            ));
        }
        if entry.config["enabled"] != true {
            return Err(ApiError::new("ENGINE_NOT_READY", "Source is disabled"));
        }
        Ok(&entry.source)
    }

    pub fn snapshot(
        &mut self,
        config_id: &str,
        host_epoch: &str,
        sync: bool,
    ) -> Result<SourceSnapshot, ApiError> {
        let id = Uuid::parse_str(config_id).map_err(|_| ApiError::bad("config_id must be UUID"))?;
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| ApiError::new("SOURCE_NOT_FOUND", "Configuration is not registered"))?;
        if entry.config["enabled"] != true {
            return Err(ApiError::new("ENGINE_NOT_READY", "Source is disabled"));
        }
        if sync {
            entry.sync_sequence += 1;
        }
        Ok(SourceSnapshot {
            host_epoch: host_epoch.into(),
            guest_epoch: None,
            config: entry.config.clone(),
            generation: entry.generation,
            sync_sequence: entry.sync_sequence,
            source_id: entry.source["id"].as_str().expect("Source id").into(),
            source_revision: entry.source["source_revision"]
                .as_str()
                .expect("Source revision")
                .into(),
        })
    }

    pub fn check_snapshot(
        &self,
        captured: &SourceSnapshot,
        host_epoch: &str,
        guest_epoch: Option<&str>,
        sync: bool,
    ) -> Result<(), ApiError> {
        if captured.host_epoch != host_epoch || captured.guest_epoch.as_deref() != guest_epoch {
            return Err(ApiError::cancelled());
        }
        let id = Uuid::parse_str(
            captured.config["config_id"]
                .as_str()
                .ok_or_else(ApiError::cancelled)?,
        )
        .map_err(|_| ApiError::cancelled())?;
        let entry = self.entries.get(&id).ok_or_else(ApiError::cancelled)?;
        if entry.config["enabled"] != true
            || entry.generation != captured.generation
            || (sync && entry.sync_sequence != captured.sync_sequence)
            || (!sync && entry.source["source_revision"] != captured.source_revision)
        {
            return Err(ApiError::cancelled());
        }
        Ok(())
    }

    pub fn publish_sync(
        &mut self,
        captured: &SourceSnapshot,
        host_epoch: &str,
        source: Value,
    ) -> Result<(), ApiError> {
        self.check_snapshot(captured, host_epoch, None, true)?;
        Contracts::get().validate("AppSource", &source)?;
        if source["id"] != captured.source_id
            || source["kind"] != captured.config["kind"]
            || source["origin"] != configured_source(&captured.config)["origin"]
        {
            return Err(ApiError::bad(
                "Published source must retain the captured configuration identity",
            ));
        }
        let id = Uuid::parse_str(captured.config["config_id"].as_str().expect("Config UUID"))
            .expect("Config UUID");
        self.entries.get_mut(&id).expect("Checked source").source = source;
        Ok(())
    }
}

fn configured_source(config: &Value) -> Value {
    let kind = config["kind"].as_str().expect("Validated source kind");
    let config_id = config["config_id"].as_str().expect("Validated config id");
    let revision = match kind {
        "maccms" => identity(
            "maccms_revision",
            &[config["api_url"].as_str().expect("API URL"), "json", "1"],
        ),
        _ => "unpublished".into(),
    };
    let origin = match kind {
        "maccms" => json!({"config_id": config_id, "api_url": config["api_url"]}),
        _ => json!({"config_id": config_id, "manifest_urls": config["manifest_urls"]}),
    };
    json!({
        "id": identity("http_source", &[kind, config_id]), "source_revision": revision,
        "name": config["name"], "language": "", "content_types": [],
        "capabilities": {"home": false, "search": false, "details": false, "resolve": false, "sequential_home": false},
        "runtime_status": if config["enabled"] == true { "configured" } else { "disabled" },
        "kind": kind, "origin": origin, "error": null,
    })
}
