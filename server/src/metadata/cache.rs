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

pub fn title_key(source: &Value, kind: &Value, id: &Value) -> String {
    serde_json::json!([source, kind, id]).to_string()
}
fn detail_key(key: &str) -> Option<String> {
    let parts: Value = serde_json::from_str(key).ok()?;
    let parts = parts.as_array()?;
    if parts.len() != 5 || parts[2].as_str()? != "meta" { return None; }
    let resource: Value = serde_json::from_str(parts[4].as_str()?).ok()?;
    let resource = resource.as_array()?;
    let offset = if parts[0] == "tmdb" { 0 } else { 1 };
    Some(title_key(&parts[0], resource.get(offset)?, resource.get(offset + 1)?))
}

fn trim_details(tx: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM detail_media_owners WHERE title_key IN (SELECT title_key FROM detail_cache_access ORDER BY id DESC LIMIT -1 OFFSET 1000)", [])?;
    tx.execute("DELETE FROM detail_media_assets WHERE image_key NOT IN (SELECT image_key FROM detail_media_owners)", [])?;
    tx.execute("DELETE FROM metadata_cache WHERE cache_key IN (SELECT cache_key FROM detail_cache_entries WHERE title_key IN (SELECT title_key FROM detail_cache_access ORDER BY id DESC LIMIT -1 OFFSET 1000))", [])?;
    tx.execute("DELETE FROM detail_cache_entries WHERE title_key IN (SELECT title_key FROM detail_cache_access ORDER BY id DESC LIMIT -1 OFFSET 1000)", [])?;
    tx.execute("DELETE FROM detail_cache_access WHERE title_key IN (SELECT title_key FROM detail_cache_access ORDER BY id DESC LIMIT -1 OFFSET 1000)", [])?;
    Ok(())
}

impl Cache {
    pub fn touch_title(&self, title: &str) -> rusqlite::Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT OR REPLACE INTO detail_cache_access(title_key) VALUES(?1)", [title])?;
        trim_details(&tx)?;
        tx.commit()
    }
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
        if payload.is_some() {
            if let Some(title) = detail_key(key) {
                conn.execute("INSERT OR REPLACE INTO detail_cache_access(title_key) VALUES(?1)", [&title])?;
                conn.execute("INSERT OR REPLACE INTO detail_cache_entries(cache_key,title_key) VALUES(?1,?2)", params![key, title])?;
            }
        }
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
    pub fn stale_detail(&self, key: &str) -> rusqlite::Result<Option<Value>> {
        let Some(title) = detail_key(key) else { return Ok(None); };
        let conn = self.connection()?;
        let payload: Option<String> = conn.query_row("SELECT payload FROM metadata_cache WHERE cache_key=?1", [key], |row| row.get(0)).optional()?;
        if payload.is_some() {
            conn.execute("INSERT OR REPLACE INTO detail_cache_access(title_key) VALUES(?1)", [&title])?;
            conn.execute("INSERT OR REPLACE INTO detail_cache_entries(cache_key,title_key) VALUES(?1,?2)", params![key,title])?;
        }
        payload.map(|value| serde_json::from_str(&value).map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error)))).transpose()
    }
    pub fn remove(&self, key: &str) -> rusqlite::Result<()> {
        self.connection()?
            .execute("DELETE FROM metadata_cache WHERE cache_key=?1", [key])?;
        Ok(())
    }
    pub fn put(&self, key: &str, value: &Value, ttl: i64) -> rusqlite::Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO metadata_cache(cache_key,payload,expires_at) VALUES(?1,?2,?3)",
            params![key, value.to_string(), now() + ttl],
        )?;
        if let Some(title) = detail_key(key) {
            tx.execute("INSERT OR REPLACE INTO detail_cache_access(title_key) VALUES(?1)", [&title])?;
            tx.execute("INSERT OR REPLACE INTO detail_cache_entries(cache_key,title_key) VALUES(?1,?2)", params![key, title])?;
            trim_details(&tx)?;
        }
        tx.commit()
    }
    pub fn prepare_details(&self) -> rusqlite::Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let keys = {
            let mut statement = tx.prepare("SELECT cache_key FROM metadata_cache WHERE cache_key NOT IN (SELECT cache_key FROM detail_cache_entries) ORDER BY expires_at")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for key in keys {
            if let Some(title) = detail_key(&key) {
                tx.execute("INSERT OR IGNORE INTO detail_cache_access(title_key) VALUES(?1)", [&title])?;
                tx.execute("INSERT OR REPLACE INTO detail_cache_entries(cache_key,title_key) VALUES(?1,?2)", params![key, title])?;
            }
        }
        trim_details(&tx)?;
        tx.commit()
    }
    /// Expired rows are never read again (`get` filters them), so drop them.
    pub fn prune(&self, now: i64) -> rusqlite::Result<usize> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let removed = tx.execute("DELETE FROM metadata_cache WHERE expires_at<=?1 AND cache_key NOT IN (SELECT cache_key FROM detail_cache_entries)", [now])?;
        tx.execute("DELETE FROM detail_cache_entries WHERE cache_key NOT IN (SELECT cache_key FROM metadata_cache)", [])?;
        tx.execute("DELETE FROM detail_cache_access WHERE title_key NOT IN (SELECT title_key FROM detail_cache_entries) AND title_key NOT IN (SELECT title_key FROM detail_media_owners)", [])?;
        tx.commit()?;
        Ok(removed)
    }
    pub fn register_image(&self, title: &str, url: &str) -> rusqlite::Result<String> {
        use sha2::{Digest, Sha256};
        let key = format!("{:x}", Sha256::digest(url.as_bytes()));
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT OR IGNORE INTO detail_media_assets(image_key,url) VALUES(?1,?2)", params![key,url])?;
        tx.execute("INSERT OR IGNORE INTO detail_media_owners(title_key,image_key) VALUES(?1,?2)", params![title,key])?;
        tx.commit()?;
        Ok(key)
    }
    pub fn image(&self, key: &str) -> rusqlite::Result<Option<(String, Option<Vec<u8>>, Option<String>)>> {
        self.connection()?.query_row("SELECT url,body,mime FROM detail_media_assets WHERE image_key=?1", [key], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()
    }
    pub fn save_image(&self, key: &str, body: &[u8], mime: &str) -> rusqlite::Result<()> {
        self.connection()?.execute("UPDATE detail_media_assets SET body=?2,mime=?3 WHERE image_key=?1", params![key,body,mime])?;
        Ok(())
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
    fn detail_cache_retains_recent_titles_across_languages() {
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let cache = Cache::new(dir.path(), "v1".into());
        let detail = |id: usize, lang: &str| cache.key("tmdb", "meta", lang, &serde_json::json!(["movie", id]).to_string());
        let catalog = cache.key("tmdb", "catalog", "en-US", "all");
        cache.put(&catalog, &serde_json::json!("catalog"), 3600).unwrap();
        for id in 0..1000 { cache.put(&detail(id, "en-US"), &serde_json::json!(id), 3600).unwrap(); }
        cache.put(&detail(0, "zh-CN"), &serde_json::json!(0), 3600).unwrap();
        assert!(cache.get(&detail(1, "en-US")).unwrap().is_some());
        cache.put(&detail(1000, "en-US"), &serde_json::json!(1000), 3600).unwrap();
        assert!(cache.get(&detail(0, "en-US")).unwrap().is_some());
        assert!(cache.get(&detail(0, "zh-CN")).unwrap().is_some());
        assert!(cache.get(&detail(2, "en-US")).unwrap().is_none());
        assert!(cache.get(&catalog).unwrap().is_some());
    }

    #[test]
    fn catalog_titles_and_images_share_the_thousand_title_limit() {
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let cache = Cache::new(dir.path(), "v1".into());
        for id in 0..1000 { cache.touch_title(&format!("title-{id}")).unwrap(); }
        let old = cache.register_image("title-0", "https://example.test/old.jpg").unwrap();
        let retained = cache.register_image("title-1", "https://example.test/retained.jpg").unwrap();
        cache.touch_title("title-1000").unwrap();
        assert!(cache.image(&old).unwrap().is_none());
        assert!(cache.image(&retained).unwrap().is_some());
    }

    #[test]
    fn media_follows_recent_title_eviction_without_deleting_shared_images() {
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let cache = Cache::new(dir.path(), "v1".into());
        let detail = |id: usize| cache.key("tmdb", "meta", "en-US", &serde_json::json!(["movie", id]).to_string());
        for id in 0..1000 { cache.put(&detail(id), &serde_json::json!(id), 3600).unwrap(); }
        let first = title_key(&serde_json::json!("tmdb"), &serde_json::json!("movie"), &serde_json::json!(0));
        let second = title_key(&serde_json::json!("tmdb"), &serde_json::json!("movie"), &serde_json::json!(1));
        let shared = cache.register_image(&first, "https://example.test/shared.jpg").unwrap();
        assert_eq!(shared, cache.register_image(&second, "https://example.test/shared.jpg").unwrap());
        let unique = cache.register_image(&first, "https://example.test/old.jpg").unwrap();
        cache.save_image(&shared, b"shared", "image/jpeg").unwrap();
        cache.save_image(&unique, b"old", "image/jpeg").unwrap();
        cache.put(&detail(1000), &serde_json::json!(1000), 3600).unwrap();
        assert!(cache.image(&unique).unwrap().is_none());
        assert_eq!(cache.image(&shared).unwrap().unwrap().1.unwrap(), b"shared");
    }

    #[test]
    fn stale_details_are_retained_for_fallback_while_other_expired_rows_are_pruned() {
        let dir = tempfile::tempdir().unwrap(); crate::db::initialize(dir.path()).unwrap();
        let cache = Cache::new(dir.path(), "v1".into());
        let detail = cache.key("tmdb", "meta", "en-US", &serde_json::json!(["movie","42"]).to_string());
        let catalog = cache.key("tmdb", "catalog", "en-US", "all");
        cache.put(&detail, &serde_json::json!("detail"), -1).unwrap();
        cache.put(&catalog, &serde_json::json!("catalog"), -1).unwrap();
        assert_eq!(cache.prune(now()).unwrap(), 1);
        assert!(cache.get(&detail).unwrap().is_none());
        assert_eq!(cache.stale_detail(&detail).unwrap(), Some(serde_json::json!("detail")));
        assert!(cache.stale_detail(&catalog).unwrap().is_none());
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
