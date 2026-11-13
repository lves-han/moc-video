use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct Cache {
    path: PathBuf,
    revision: String,
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl Cache {
    pub fn new(dir: &Path, revision: String) -> Self {
        Self {
            path: dir.join("service.sqlite3"),
            revision,
        }
    }
    fn connection(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.path)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        Ok(conn)
    }
    pub fn key(&self, source: &str, kind: &str, lang: &str, resource: &str) -> String {
        serde_json::json!([source, self.revision, kind, lang, resource]).to_string()
    }
    pub fn get(&self, key: &str) -> rusqlite::Result<Option<Value>> {
        let conn = self.connection()?;
        let payload: Option<String> = conn
            .query_row(
                "SELECT payload FROM metadata_cache WHERE cache_key=?1 AND expires_at>?2",
                params![key, now()],
                |row| row.get(0),
            )
            .optional()?;
        payload
            .map(|s| {
                serde_json::from_str(&s).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .transpose()
    }
    pub fn remove(&self, key: &str) -> rusqlite::Result<()> {
        self.connection()?
            .execute("DELETE FROM metadata_cache WHERE cache_key=?1", [key])?;
        Ok(())
    }
    pub fn put(&self, key: &str, value: &Value, ttl: i64) -> rusqlite::Result<()> {
        self.connection()?.execute(
            "INSERT OR REPLACE INTO metadata_cache(cache_key,payload,expires_at) VALUES(?1,?2,?3)",
            params![key, value.to_string(), now() + ttl],
        )?;
        Ok(())
    }
    /// Expired rows are never read again (`get` filters them), so drop them.
    pub fn prune(&self, now: i64) -> rusqlite::Result<usize> {
        self.connection()?
            .execute("DELETE FROM metadata_cache WHERE expires_at<=?1", [now])
    }
    pub fn snapshot(&self) -> rusqlite::Result<Option<(i64, Value)>> {
        let conn = self.connection()?;
        let row: Option<(i64, String)> = conn
            .query_row(
                "SELECT built_at,payload FROM home_snapshots WHERE revision=?1",
                [&self.revision],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        row.map(|(time, body)| {
            serde_json::from_str(&body)
                .map(|value| (time, value))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
        })
        .transpose()
    }
    pub fn save_snapshot(&self, built_at: i64, value: &Value) -> rusqlite::Result<()> {
        self.connection()?.execute(
            "INSERT OR REPLACE INTO home_snapshots(revision,built_at,payload) VALUES(?1,?2,?3)",
            params![self.revision, built_at, value.to_string()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_keys_separate_sources_types_and_languages() {
        let cache = Cache::new(Path::new("."), "v1".into());
        let one = cache.key("first", "movie", "en-US", "same");
        assert_ne!(one, cache.key("second", "movie", "en-US", "same"));
        assert_ne!(one, cache.key("first", "series", "en-US", "same"));
        assert_ne!(one, cache.key("first", "movie", "zh-CN", "same"));
    }

    #[test]
    fn prune_drops_only_expired_rows() {
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let cache = Cache::new(dir.path(), "v1".into());
        cache.put("old", &serde_json::json!(1), -10).unwrap();
        cache.put("fresh", &serde_json::json!(2), 600).unwrap();
        assert_eq!(cache.prune(now()).unwrap(), 1);
        assert_eq!(cache.get("fresh").unwrap(), Some(serde_json::json!(2)));
        let rows: i64 = cache
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM metadata_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }
}
