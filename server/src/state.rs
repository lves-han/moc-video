use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct Candidate {
    pub source: Value,
    pub content_id: String,
    pub detail: Value,
}
pub struct Lookup {
    pub metadata: Value,
    pub expires_at: i64,
    pub cancelled: watch::Sender<bool>,
    pub candidates: HashMap<String, Candidate>,
    pub active: bool,
}
#[derive(Clone)]
pub struct Line {
    pub source_id: String,
    pub source_revision: u64,
    pub content_id: String,
    pub target: Value,
    pub locator: Option<Value>,
    pub logical_key: String,
    pub flag: String,
    pub episode_label: String,
    pub url: String,
    pub headers: Value,
    pub quality: Option<String>,
    pub needs_sniffing: bool,
    pub kind_hint: Option<String>,
    pub unsupported_reason: Option<String>,
    pub expires_at: i64,
    pub play_expires_at: i64,
}
#[derive(Clone)]
pub struct Template {
    pub source: Value,
    pub content_id: String,
    pub lines: Vec<Value>,
}
pub struct CacheEntry {
    pub expires_at: i64,
    pub template: Template,
}
#[derive(Default)]
pub struct Registry {
    pub lookups: HashMap<String, Lookup>,
    pub lines: HashMap<String, Line>,
    pub cache: HashMap<String, CacheEntry>,
    /// Lookups cancelled through DELETE, kept until their lease ends so a late
    /// confirmation is answered CANCELLED instead of "expired".
    pub cancelled: HashMap<String, i64>,
    pub mapping_generation: u64,
    pub parser_generation: u64,
}
impl Registry {
    pub fn bytes(&self) -> usize {
        let candidates = self
            .lookups
            .values()
            .map(|lookup| {
                lookup.metadata.to_string().len()
                    + lookup
                        .candidates
                        .values()
                        .map(|candidate| {
                            candidate.source.to_string().len()
                                + candidate.content_id.len()
                                + candidate.detail.to_string().len()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>();
        let lines = self
            .lines
            .values()
            .map(|line| {
                line.url.len()
                    + line.headers.to_string().len()
                    + line.target.to_string().len()
                    + line.flag.len()
                    + line.episode_label.len()
                    + line.content_id.len()
                    + 512
            })
            .sum::<usize>();
        let cached = self
            .cache
            .values()
            .map(|entry| {
                entry.template.source.to_string().len()
                    + entry.template.content_id.len()
                    + entry
                        .template
                        .lines
                        .iter()
                        .map(|line| line.to_string().len())
                        .sum::<usize>()
            })
            .sum::<usize>();
        candidates.saturating_add(lines).saturating_add(cached)
    }
    pub fn prune(&mut self, now: i64) {
        self.lookups.retain(|_, value| {
            value.expires_at > now || (value.active && value.expires_at.saturating_add(30) > now)
        });
        self.lines.retain(|_, value| value.expires_at > now);
        self.cache.retain(|_, value| value.expires_at > now);
        self.cancelled.retain(|_, expires_at| *expires_at > now);
    }
    pub fn changed(&mut self, source_id: &str) {
        self.cache
            .retain(|_, value| value.template.source["id"] != source_id);
        // Source revision is checked again at every confirmation, refresh and report.
    }
}

#[derive(Clone)]
pub struct Runtime {
    pub registry: Arc<Mutex<Registry>>,
    pub handle_ttl: i64,
    pub cache_ttl: i64,
}
impl Runtime {
    pub fn from_env() -> Result<Self, &'static str> {
        fn value(name: &str, default: i64) -> Result<i64, &'static str> {
            let parsed = std::env::var(name)
                .ok()
                .map(|v| v.parse::<i64>())
                .transpose()
                .map_err(|_| "invalid lookup lifetime")?
                .unwrap_or(default);
            if !(1..=86_400).contains(&parsed) {
                return Err("invalid lookup lifetime");
            }
            Ok(parsed)
        }
        Ok(Self {
            registry: Arc::new(Mutex::new(Registry::default())),
            handle_ttl: value("MOC_HANDLE_TTL_SECONDS", 900)?,
            cache_ttl: value("MOC_LINE_CACHE_SECONDS", 60)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn bounded_registry_rejects_new_lines() {
        let mut registry = Registry::default();
        let (sender, _) = watch::channel(false);
        registry.lookups.insert(
            "large".into(),
            Lookup {
                metadata: json!({"title":"x".repeat(MAX_BYTES)}),
                expires_at: i64::MAX,
                cancelled: sender,
                candidates: HashMap::new(),
                active: true,
            },
        );
        let runtime = Runtime {
            registry: Arc::new(Mutex::new(Registry::default())),
            handle_ttl: 900,
            cache_ttl: 60,
        };
        let dir = tempfile::tempdir().unwrap();
        let reports = crate::reports::Store::new(dir.path());
        let template = Template {
            source: json!({"id":"s","revision":1,"headers":{}}),
            content_id: "c".into(),
            lines: vec![],
        };
        assert!(
            crate::line_cache::issue(&mut registry, &runtime, &json!({}), &template, &reports)
                .is_err()
        );
    }

    #[test]
    fn active_lookup_survives_initial_lease_until_round_finishes() {
        let mut registry = Registry::default();
        let (sender, _) = watch::channel(false);
        registry.lookups.insert(
            "running".into(),
            Lookup {
                metadata: json!({}),
                expires_at: 10,
                cancelled: sender,
                candidates: HashMap::new(),
                active: true,
            },
        );
        registry.prune(11);
        assert!(registry.lookups.contains_key("running"));
        registry.lookups.get_mut("running").unwrap().active = false;
        registry.prune(11);
        assert!(!registry.lookups.contains_key("running"));
    }
}
