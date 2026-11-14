use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}
impl Store {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("service.sqlite3"),
        }
    }
    pub fn get(&self, key: &str) -> rusqlite::Result<Value> {
        let key = key.to_owned();
        crate::db::sync_with(&self.path, |conn| Self::get_conn(conn, &key))
    }
    fn get_conn(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<Value> {
        let data: Option<(u64, u64, Option<i64>, Option<u64>)> = conn
            .query_row(
                "SELECT ok_count,fail_count,last_ok_at,last_latency_ms FROM line_stats WHERE logical_key=?1",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let (ok, failed, last, latency) = data.unwrap_or((0, 0, None, None));
        Ok(json!({"ok":ok,"failed":failed,"last_ok_at":last,"last_latency_ms":latency}))
    }
    pub async fn get_async(&self, key: &str) -> rusqlite::Result<Value> {
        let key = key.to_owned();
        crate::db::run_sqlite(self.path.clone(), move |conn| Self::get_conn(conn, &key)).await
    }
    pub async fn get_many(&self, keys: Vec<String>) -> rusqlite::Result<HashMap<String, Value>> {
        if keys.is_empty() {
            return Ok(HashMap::new());
        }
        crate::db::run_sqlite(self.path.clone(), move |conn| {
            let mut map = HashMap::with_capacity(keys.len());
            let mut stmt = conn.prepare_cached(
                "SELECT ok_count,fail_count,last_ok_at,last_latency_ms FROM line_stats WHERE logical_key=?1",
            )?;
            for key in keys {
                let data: Option<(u64, u64, Option<i64>, Option<u64>)> = stmt
                    .query_row([&key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                    .optional()?;
                let (ok, failed, last, latency) = data.unwrap_or((0, 0, None, None));
                map.insert(
                    key,
                    json!({"ok":ok,"failed":failed,"last_ok_at":last,"last_latency_ms":latency}),
                );
            }
            Ok(map)
        })
        .await
    }
    pub fn record(
        &self,
        line: &crate::state::Line,
        outcome: &str,
        latency: Option<u64>,
    ) -> rusqlite::Result<()> {
        if line.locator.is_none() {
            return Ok(());
        }
        let logical_key = line.logical_key.clone();
        let source_id = line.source_id.clone();
        let content_id = line.content_id.clone();
        let outcome = outcome.to_owned();
        crate::db::sync_with(&self.path, |conn| {
            Self::record_conn(conn, &logical_key, &source_id, &content_id, &outcome, latency)
        })
    }
    pub async fn record_async(
        &self,
        line: &crate::state::Line,
        outcome: &str,
        latency: Option<u64>,
    ) -> rusqlite::Result<()> {
        if line.locator.is_none() {
            return Ok(());
        }
        let logical_key = line.logical_key.clone();
        let source_id = line.source_id.clone();
        let content_id = line.content_id.clone();
        let outcome = outcome.to_owned();
        crate::db::run_sqlite(self.path.clone(), move |conn| {
            Self::record_conn(conn, &logical_key, &source_id, &content_id, &outcome, latency)
        })
        .await
    }
    fn record_conn(
        conn: &rusqlite::Connection,
        logical_key: &str,
        source_id: &str,
        content_id: &str,
        outcome: &str,
        latency: Option<u64>,
    ) -> rusqlite::Result<()> {
        let ok = u8::from(outcome == "playing");
        let failed = u8::from(outcome == "failed");
        let last = (outcome == "playing").then(crate::metadata::cache::now);
        conn.execute("INSERT INTO line_stats(logical_key,source_id,content_id,ok_count,fail_count,last_ok_at,last_latency_ms) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(logical_key) DO UPDATE SET ok_count=ok_count+excluded.ok_count,fail_count=fail_count+excluded.fail_count,last_ok_at=COALESCE(excluded.last_ok_at,line_stats.last_ok_at),last_latency_ms=COALESCE(excluded.last_latency_ms,line_stats.last_latency_ms)",params![logical_key,source_id,content_id,ok,failed,last,latency])?;
        Ok(())
    }
}
