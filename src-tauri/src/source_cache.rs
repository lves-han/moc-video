//! Source bootstrap snapshots share the application's bounded cache.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io, sync::Arc};

const LIMIT: usize = 24 * 1024 * 1024;

pub struct SourceCache {
    shared: Arc<crate::media_cache::MediaCache>,
}

impl SourceCache {
    pub fn new(shared: Arc<crate::media_cache::MediaCache>) -> Self {
        Self { shared }
    }

    fn key(config: &Value) -> String {
        let encoded = serde_json::to_vec(config).expect("source configuration is JSON");
        format!("source:{:x}.json", Sha256::digest(encoded))
    }

    pub fn load(&self, config: &Value) -> Option<Value> {
        let bytes = self.shared.get_blob(&Self::key(config))?;
        let value: Value = serde_json::from_slice(&bytes).ok()?;
        (value["version"] == 1 && value["kind"] == config["kind"]).then(|| value["payload"].clone())
    }

    pub fn save(&self, config: &Value, payload: Value) -> io::Result<()> {
        let bytes =
            serde_json::to_vec(&json!({"version": 1, "kind": config["kind"], "payload": payload}))?;
        if bytes.len() > LIMIT {
            return Err(io::Error::other("Source snapshot exceeds size limit"));
        }
        self.shared
            .put_blob(&Self::key(config), &bytes)
            .map_err(|_| io::Error::other("Source cache write failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_share_the_media_size_limit() {
        let root = tempfile::tempdir().unwrap();
        let media = crate::media_cache::MediaCache::new(root.path().join("media-cache"), 5000);
        let cache = SourceCache::new(media.clone());
        let first = json!({"kind":"stremio","name":"first"});
        let second = json!({"kind":"stremio","name":"second"});
        cache.save(&first, json!(["one"])).unwrap();
        cache.save(&second, json!(["two"])).unwrap();
        assert_eq!(cache.load(&first), None);
        assert_eq!(cache.load(&second), Some(json!(["two"])));
        assert!(media.status()["used_bytes"].as_u64().unwrap() <= 5000);
    }
}
