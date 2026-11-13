use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

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
    fn conn(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.path)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        Ok(conn)
    }
    pub fn get(&self, key: &str) -> rusqlite::Result<Value> {
        let data:Option<(u64,u64,Option<i64>,Option<u64>)>=self.conn()?.query_row("SELECT ok_count,fail_count,last_ok_at,last_latency_ms FROM line_stats WHERE logical_key=?1",[key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let (ok, failed, last, latency) = data.unwrap_or((0, 0, None, None));
        Ok(json!({"ok":ok,"failed":failed,"last_ok_at":last,"last_latency_ms":latency}))
    }
    pub fn record(
        &self,
        line: &crate::state::Line,
        outcome: &str,
        latency: Option<u64>,
    ) -> rusqlite::Result<()> {
        // A line without a verifiable locator has a one-off key that no later lookup
        // can read back, so its result is not stored.
        if line.locator.is_none() {
            return Ok(());
        }
        let ok = u8::from(outcome == "playing");
        let failed = u8::from(outcome == "failed");
        let last = (outcome == "playing").then(crate::metadata::cache::now);
        self.conn()?.execute("INSERT INTO line_stats(logical_key,source_id,content_id,ok_count,fail_count,last_ok_at,last_latency_ms) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(logical_key) DO UPDATE SET ok_count=ok_count+excluded.ok_count,fail_count=fail_count+excluded.fail_count,last_ok_at=COALESCE(excluded.last_ok_at,line_stats.last_ok_at),last_latency_ms=COALESCE(excluded.last_latency_ms,line_stats.last_latency_ms)",params![line.logical_key,line.source_id,line.content_id,ok,failed,last,latency])?;
        Ok(())
    }
}
