use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
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
    pub fn list_sync(
        &self,
        metadata_source: &str,
        kind: &str,
        meta_id: &str,
    ) -> rusqlite::Result<Vec<Value>> {
        crate::db::sync_with(&self.path, |conn| {
            Self::list_conn(conn, metadata_source, kind, meta_id)
        })
    }
    pub async fn list(
        &self,
        metadata_source: &str,
        kind: &str,
        meta_id: &str,
    ) -> rusqlite::Result<Vec<Value>> {
        let metadata_source = metadata_source.to_owned();
        let kind = kind.to_owned();
        let meta_id = meta_id.to_owned();
        crate::db::run_sqlite(self.path.clone(), move |conn| {
            Self::list_conn(conn, &metadata_source, &kind, &meta_id)
        })
        .await
    }
    fn list_conn(
        conn: &rusqlite::Connection,
        metadata_source: &str,
        kind: &str,
        meta_id: &str,
    ) -> rusqlite::Result<Vec<Value>> {
        let mut statement = conn.prepare("SELECT id,metadata_source_id,media_type,meta_id,source_id,source_revision,content_id,confirmed_by,evidence,updated_at,revoked,season FROM mappings WHERE metadata_source_id=?1 AND media_type=?2 AND meta_id=?3 ORDER BY updated_at DESC")?;
        let rows = statement.query_map(params![metadata_source, kind, meta_id], row)?;
        rows.collect()
    }
    pub async fn revoked(
        &self,
        metadata_source: &str,
        kind: &str,
        meta_id: &str,
        source_id: &str,
    ) -> rusqlite::Result<HashSet<String>> {
        let metadata_source = metadata_source.to_owned();
        let kind = kind.to_owned();
        let meta_id = meta_id.to_owned();
        let source_id = source_id.to_owned();
        crate::db::run_sqlite(self.path.clone(), move |conn| {
            let mut statement = conn.prepare("SELECT content_id FROM mapping_revocations WHERE metadata_source_id=?1 AND media_type=?2 AND meta_id=?3 AND source_id=?4")?;
            let rows = statement
                .query_map(params![metadata_source, kind, meta_id, source_id], |row| {
                    row.get(0)
                })?;
            rows.collect()
        })
        .await
    }
    #[cfg(test)]
    pub fn save(
        &self,
        metadata: &Value,
        source: &Value,
        content_id: &str,
        evidence: &Value,
        confirmed_by: &str,
    ) -> Result<Value, &'static str> {
        let metadata = metadata.clone();
        let source = source.clone();
        let content_id = content_id.to_owned();
        let evidence = evidence.clone();
        let confirmed_by = confirmed_by.to_owned();
        let mut out: Option<Result<Value, &'static str>> = None;
        let _ = crate::db::sync_with(&self.path, |conn| {
            out = Some(Self::save_conn(
                conn,
                &metadata,
                &source,
                &content_id,
                &evidence,
                &confirmed_by,
                false,
            ));
            Ok(())
        });
        out.unwrap_or(Err("mapping database failed"))
    }
    #[cfg(test)]
    pub fn save_checked_sync(
        &self,
        metadata: &Value,
        source: &Value,
        content_id: &str,
        evidence: &Value,
        confirmed_by: &str,
    ) -> Result<Value, &'static str> {
        let mut out: Option<Result<Value, &'static str>> = None;
        let _ = crate::db::sync_with(&self.path, |conn| {
            out = Some(Self::save_conn(
                conn,
                metadata,
                source,
                content_id,
                evidence,
                confirmed_by,
                true,
            ));
            Ok(())
        });
        out.unwrap_or(Err("mapping database failed"))
    }
    pub async fn save_checked(
        &self,
        metadata: &Value,
        source: &Value,
        content_id: &str,
        evidence: &Value,
        confirmed_by: &str,
    ) -> Result<Value, &'static str> {
        let metadata = metadata.clone();
        let source = source.clone();
        let content_id = content_id.to_owned();
        let evidence = evidence.clone();
        let confirmed_by = confirmed_by.to_owned();
        crate::db::run(self.path.clone(), "mapping database failed", move |conn| {
            Self::save_conn(
                conn,
                &metadata,
                &source,
                &content_id,
                &evidence,
                &confirmed_by,
                true,
            )
        })
        .await
    }
    fn save_conn(
        conn: &mut rusqlite::Connection,
        metadata: &Value,
        source: &Value,
        content_id: &str,
        evidence: &Value,
        confirmed_by: &str,
        check_sources: bool,
    ) -> Result<Value, &'static str> {
        let metadata_source = required(metadata, "metadata_source_id")?;
        let kind = required(metadata, "type")?;
        let meta_id = required(metadata, "meta_id")?;
        let source_id = required(source, "id")?;
        let season = season(metadata)?;
        if !matches!(kind, "movie" | "series")
            || content_id.is_empty()
            || !matches!(confirmed_by, "auto" | "user")
            || !evidence.is_object()
        {
            return Err("invalid mapping");
        }
        let revision = source["revision"]
            .as_u64()
            .ok_or("invalid source revision")?;
        // Read source versions and write the mapping in one write transaction.
        // A deferred read transaction can lose its WAL snapshot when cache writes race it.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| "mapping database failed")?;
        if check_sources {
            let current: Option<(u64, bool)> = tx
                .query_row(
                    "SELECT revision,enabled FROM playback_sources WHERE id=?1",
                    [source_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|_| "mapping database failed")?;
            if current != Some((revision, true)) {
                return Err("source changed");
            }
            if metadata_source != "tmdb" {
                let current: Option<u64> = tx
                    .query_row(
                        "SELECT revision FROM metadata_sources WHERE id=?1",
                        [metadata_source],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|_| "mapping database failed")?;
                if current != metadata["metadata_revision"].as_u64() {
                    return Err("metadata source changed");
                }
            }
        }
        let existing:Option<Value>=tx.query_row("SELECT id,metadata_source_id,media_type,meta_id,source_id,source_revision,content_id,confirmed_by,evidence,updated_at,revoked,season FROM mappings WHERE metadata_source_id=?1 AND media_type=?2 AND meta_id=?3 AND season=?4 AND source_id=?5",params![metadata_source,kind,meta_id,season,source_id],row).optional().map_err(|_|"mapping database failed")?;
        if confirmed_by == "auto" {
            let revoked:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM mapping_revocations WHERE metadata_source_id=?1 AND media_type=?2 AND meta_id=?3 AND source_id=?4 AND content_id=?5)",params![metadata_source,kind,meta_id,source_id,content_id],|r|r.get(0)).map_err(|_|"mapping database failed")?;
            if revoked {
                return Err("mapping was revoked");
            }
            if let Some(old) = existing
                .as_ref()
                .filter(|v| v["confirmed_by"] == "user" && v["revoked"] == false)
            {
                return Ok(old.clone());
            }
        } else {
            tx.execute("DELETE FROM mapping_revocations WHERE metadata_source_id=?1 AND media_type=?2 AND meta_id=?3 AND source_id=?4 AND content_id=?5",params![metadata_source,kind,meta_id,source_id,content_id]).map_err(|_|"mapping database failed")?;
        }
        let id = existing
            .as_ref()
            .and_then(|v| v["id"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        tx.execute("INSERT INTO mappings(id,metadata_source_id,media_type,meta_id,source_id,source_revision,content_id,confirmed_by,evidence,updated_at,revoked,season) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0,?11) ON CONFLICT(metadata_source_id,media_type,meta_id,season,source_id) DO UPDATE SET source_revision=excluded.source_revision,content_id=excluded.content_id,confirmed_by=excluded.confirmed_by,evidence=excluded.evidence,updated_at=excluded.updated_at,revoked=0",params![id,metadata_source,kind,meta_id,source_id,revision,content_id,confirmed_by,evidence.to_string(),crate::metadata::cache::now(),season]).map_err(|_|"mapping database failed")?;
        tx.commit().map_err(|_| "mapping database failed")?;
        Self::list_conn(conn, metadata_source, kind, meta_id)
            .map_err(|_| "mapping database failed")?
            .into_iter()
            .find(|value| value["id"] == id)
            .ok_or("mapping database failed")
    }
    pub async fn revoke(&self, id: &str) -> rusqlite::Result<bool> {
        let id = id.to_owned();
        crate::db::run_sqlite(self.path.clone(), move |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let found:Option<(String,String,String,String,String)>=tx.query_row("SELECT metadata_source_id,media_type,meta_id,source_id,content_id FROM mappings WHERE id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
            let Some((metadata_source, kind, meta_id, source_id, content_id)) = found else {
                return Ok(false);
            };
            tx.execute("INSERT OR IGNORE INTO mapping_revocations(metadata_source_id,media_type,meta_id,source_id,content_id) VALUES(?1,?2,?3,?4,?5)",params![metadata_source,kind,meta_id,source_id,content_id])?;
            tx.execute(
                "UPDATE mappings SET revoked=1,updated_at=?1 WHERE id=?2",
                params![crate::metadata::cache::now(), id],
            )?;
            tx.commit()?;
            Ok(true)
        })
        .await
    }

    pub async fn reusable(
        &self,
        metadata: &Value,
        source: &Value,
    ) -> Result<Option<Value>, &'static str> {
        let metadata_source = required(metadata, "metadata_source_id")?;
        let kind = required(metadata, "type")?;
        let meta_id = required(metadata, "meta_id")?;
        let source_id = required(source, "id")?;
        let season = season(metadata)?;
        let row = self
            .list(metadata_source, kind, meta_id)
            .await
            .map_err(|_| "mapping database failed")?
            .into_iter()
            .find(|row| {
                row["source_id"] == source_id && row["season"] == season && row["revoked"] == false
            });
        let Some(row) = row else { return Ok(None) };
        let content = row["content_id"].as_str().ok_or("invalid stored mapping")?;
        let detail = match crate::adapters::detail(source, content).await {
            Ok(value) => value,
            Err("MacCMS content identity is missing or ambiguous") => return Ok(None),
            Err(error) => return Err(error),
        };
        if !crate::matching::same_identity(
            &row["evidence"],
            &crate::matching::evidence(metadata, &detail),
        ) {
            return Ok(None);
        }
        let lines = crate::matching::lines_for(metadata, &detail);
        if lines.is_empty() {
            return Ok(None);
        }
        let current = self
            .list(metadata_source, kind, meta_id)
            .await
            .map_err(|_| "mapping database failed")?
            .into_iter()
            .find(|value| value["id"] == row["id"]);
        if current.as_ref() != Some(&row) {
            return Ok(None);
        }
        Ok(Some(json!({"mapping":row,"detail":detail,"lines":lines})))
    }
}

/// Sources usually list each season as its own entry, so a series mapping belongs
/// to the target season; movies use season 0.
fn season(metadata: &Value) -> Result<u32, &'static str> {
    match metadata["type"].as_str() {
        Some("series") => metadata["episode"]["season"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("invalid mapping season"),
        _ => Ok(0),
    }
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str, &'static str> {
    value[key]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 512)
        .ok_or("invalid mapping identity")
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let evidence: String = row.get(8)?;
    let evidence: Value = serde_json::from_str(&evidence).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(
        json!({"id":row.get::<_,String>(0)?,"metadata_source_id":row.get::<_,String>(1)?,"type":row.get::<_,String>(2)?,"meta_id":row.get::<_,String>(3)?,"source_id":row.get::<_,String>(4)?,"source_revision":row.get::<_,u64>(5)?,"content_id":row.get::<_,String>(6)?,"confirmed_by":row.get::<_,String>(7)?,"evidence":evidence,"updated_at":row.get::<_,i64>(9)?,"revoked":row.get::<_,bool>(10)?,"season":row.get::<_,u32>(11)?}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn mapping_updates_survive_concurrent_cache_writes() {
        let dir = tempfile::tempdir().unwrap();
        crate::db::initialize(dir.path()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let source = rt
            .block_on(crate::sources::Store::new(dir.path()).save(
                &json!({"name":"fixture","kind":"maccms_json","api":"https://example.test/api"}),
                None,
            ))
            .unwrap();
        let cache = crate::metadata::cache::Cache::new(dir.path(), "test".into());
        let barrier = Arc::new(Barrier::new(2));
        let other = barrier.clone();
        let writer = std::thread::spawn(move || {
            other.wait();
            for n in 0..200 { cache.put(&format!("background-{n}"), &json!(n), 3600).unwrap(); }
        });
        let store = Store::new(dir.path());
        let metadata = json!({"metadata_source_id":"tmdb","type":"movie","meta_id":"tmdb:1"});
        barrier.wait();
        for _ in 0..100 { store.save_checked_sync(&metadata, &source, "1", &json!({}), "auto").unwrap(); }
        writer.join().unwrap();
        assert_eq!(store.list_sync("tmdb", "movie", "tmdb:1").unwrap().len(), 1);
    }
}
