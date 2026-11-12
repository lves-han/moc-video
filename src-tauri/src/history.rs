//! Watch history in SQLite (SPEC 11.4, C16). Every change runs under the Host's commit lock,
//! so write sequences have one serial entry.
use crate::contracts::{ApiError, Contracts};
use rusqlite::{Connection, ErrorCode, params};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

const VERSION: i64 = 2;
const COLUMNS: &str = "content_id, metadata_config_id, metadata_addon_key, metadata_namespace, metadata_type, metadata_id, canonical_id, video_id, season_number, episode_number, title, kind, poster_cache_key, source_id, source_name, position_ms, duration_ms, completed, updated_at";

pub fn superseded() -> ApiError {
    ApiError::new("CANCELLED", "A newer history change already committed")
}

fn failure(error: rusqlite::Error) -> ApiError {
    let reason = match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => "busy",
        Some(ErrorCode::ReadOnly | ErrorCode::PermissionDenied | ErrorCode::CannotOpen) => {
            "permission"
        }
        Some(ErrorCode::DiskFull) => "disk_full",
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => "corrupt",
        _ => "database",
    };
    let mut error = ApiError::internal("History database operation failed");
    error.retryable = true;
    error.with_details(json!({"reason":reason}))
}

pub struct History {
    path: PathBuf,
    connection: Option<Connection>,
    committed: u64,
    // The latest accepted snapshot whose write failed, kept for an identical retry.
    pending: Option<(u64, Value)>,
}

impl History {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            connection: None,
            committed: 0,
            pending: None,
        }
    }

    pub fn committed(&self) -> u64 {
        self.committed
    }

    fn connection(&mut self) -> Result<&mut Connection, ApiError> {
        if self.connection.is_none() {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent).map_err(|_| {
                    let mut error = ApiError::internal("History directory is unavailable");
                    error.retryable = true;
                    error.with_details(json!({"reason":"permission"}))
                })?;
            }
            let mut connection = Connection::open(&self.path).map_err(failure)?;
            connection
                .busy_timeout(Duration::from_millis(200))
                .map_err(failure)?;
            initialize_schema(&mut connection).map_err(|mut error| {
                error.message =
                    format!("History database initialization failed: {}", error.message);
                error
            })?;
            self.connection = Some(connection);
        }
        Ok(self.connection.as_mut().expect("Opened history"))
    }

    pub fn list(&mut self) -> Result<Value, ApiError> {
        let committed = self.committed;
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {COLUMNS} FROM watch_history ORDER BY updated_at DESC, content_id"
            ))
            .map_err(failure)?;
        let rows = statement
            .query_map([], |row| {
                Ok(json!({"content_id":row.get::<_, String>(0)?,
                    "metadata_identity":{"config_id":row.get::<_, String>(1)?,"addon_key":row.get::<_, String>(2)?,
                        "namespace":row.get::<_, String>(3)?,"type":row.get::<_, String>(4)?,
                        "meta_id":row.get::<_, String>(5)?,"canonical_id":row.get::<_, String>(6)?},
                    "video_id":row.get::<_, Option<String>>(7)?,"season_number":row.get::<_, Option<i64>>(8)?,
                    "episode_number":row.get::<_, Option<f64>>(9)?,"title":row.get::<_, String>(10)?,
                    "kind":row.get::<_, String>(11)?,"poster_cache_key":row.get::<_, Option<String>>(12)?,
                    "source_id":row.get::<_, String>(13)?,"source_name":row.get::<_, String>(14)?,
                    "position_ms":row.get::<_, i64>(15)?,"duration_ms":row.get::<_, Option<i64>>(16)?,
                    "completed":row.get::<_, i64>(17)? != 0,"updated_at":row.get::<_, i64>(18)?}))
            })
            .map_err(failure)?;
        let mut entries = Vec::new();
        for row in rows {
            let row = row.map_err(failure)?;
            Contracts::get()
                .validate("WatchHistoryRecord", &row)
                .map_err(|_| {
                    let mut error = ApiError::internal("History database holds an invalid record");
                    error.retryable = true;
                    error.with_details(json!({"reason":"invalid_record"}))
                })?;
            entries.push(row);
        }
        Ok(json!({"entries":entries,"write_sequence":committed}))
    }

    pub fn list_watchlist(&mut self) -> Result<Value, ApiError> {
        let mut statement = self
            .connection()?
            .prepare(
                "SELECT content_id, metadata_config_id, metadata_addon_key, metadata_namespace,
            metadata_type, metadata_id, canonical_id, title, kind, poster_cache_key
            FROM watchlist ORDER BY rowid DESC",
            )
            .map_err(failure)?;
        let rows = statement.query_map([], |row| Ok(json!({
            "content_id":row.get::<_, String>(0)?,
            "metadata_identity":{"config_id":row.get::<_, String>(1)?, "addon_key":row.get::<_, String>(2)?,
                "namespace":row.get::<_, String>(3)?, "type":row.get::<_, String>(4)?,
                "meta_id":row.get::<_, String>(5)?, "canonical_id":row.get::<_, String>(6)?},
            "title":row.get::<_, String>(7)?, "kind":row.get::<_, String>(8)?,
            "poster_cache_key":row.get::<_, Option<String>>(9)?
        }))).map_err(failure)?;
        let mut entries = Vec::new();
        for row in rows {
            let row = row.map_err(failure)?;
            validate_watchlist(&row)?;
            entries.push(row);
        }
        Ok(json!({"entries":entries}))
    }

    pub fn add_watchlist(&mut self, entry: &Value) -> Result<Value, ApiError> {
        validate_watchlist(entry)?;
        let identity = &entry["metadata_identity"];
        self.connection()?.execute(
            "INSERT INTO watchlist (content_id, metadata_config_id, metadata_addon_key, metadata_namespace,
            metadata_type, metadata_id, canonical_id, title, kind, poster_cache_key)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
            ON CONFLICT(content_id) DO UPDATE SET metadata_config_id=excluded.metadata_config_id,
            metadata_addon_key=excluded.metadata_addon_key, metadata_namespace=excluded.metadata_namespace,
            metadata_type=excluded.metadata_type, metadata_id=excluded.metadata_id, canonical_id=excluded.canonical_id,
            title=excluded.title, kind=excluded.kind, poster_cache_key=excluded.poster_cache_key",
            params![entry["content_id"].as_str(), identity["config_id"].as_str(), identity["addon_key"].as_str(),
                identity["namespace"].as_str(), identity["type"].as_str(), identity["meta_id"].as_str(),
                identity["canonical_id"].as_str(), entry["title"].as_str(), entry["kind"].as_str(), entry["poster_cache_key"].as_str()],
        ).map_err(failure)?;
        Ok(entry.clone())
    }

    pub fn remove_watchlist(&mut self, content_id: &str) -> Result<Value, ApiError> {
        self.connection()?
            .execute(
                "DELETE FROM watchlist WHERE content_id = ?1",
                params![content_id],
            )
            .map_err(failure)?;
        Ok(json!({"ok":true}))
    }

    /// True when this call repeats the pending snapshot; a different payload on its sequence is refused.
    pub fn retrying(&self, sequence: u64, input: &Value) -> Result<bool, ApiError> {
        match &self.pending {
            Some((pending, payload)) if *pending == sequence && sequence > self.committed => {
                if payload == input {
                    Ok(true)
                } else {
                    Err(ApiError::bad(
                        "A different snapshot reuses this history write sequence",
                    ))
                }
            }
            _ => Ok(false),
        }
    }

    pub fn commit(&mut self, sequence: u64, input: &Value) -> Result<Value, ApiError> {
        if sequence <= self.committed {
            return Err(superseded());
        }
        let record = &input["record"];
        let identity = &record["metadata_identity"];
        let result = self.connection().and_then(|connection| {
            connection
                .execute(
                    &format!(
                        "INSERT OR REPLACE INTO watch_history ({COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)"
                    ),
                    params![
                        record["content_id"].as_str(),
                        identity["config_id"].as_str(),
                        identity["addon_key"].as_str(),
                        identity["namespace"].as_str(),
                        identity["type"].as_str(),
                        identity["meta_id"].as_str(),
                        identity["canonical_id"].as_str(),
                        record["video_id"].as_str(),
                        record["season_number"].as_i64(),
                        record["episode_number"].as_f64(),
                        record["title"].as_str(),
                        record["kind"].as_str(),
                        record["poster_cache_key"].as_str(),
                        record["source_id"].as_str(),
                        record["source_name"].as_str(),
                        record["position_ms"].as_i64(),
                        record["duration_ms"].as_i64(),
                        record["completed"].as_bool().map(i64::from),
                        record["updated_at"].as_i64(),
                    ],
                )
                .map_err(failure)
        });
        match result {
            Ok(_) => {
                self.committed = sequence;
                self.pending = None;
                Ok(record.clone())
            }
            Err(error) => {
                self.pending = Some((sequence, input.clone()));
                Err(error)
            }
        }
    }

    pub fn remove(&mut self, content_id: &str, sequence: u64) -> Result<Value, ApiError> {
        self.barrier(
            sequence,
            "DELETE FROM watch_history WHERE content_id = ?1",
            Some(content_id),
        )
    }

    pub fn clear(&mut self, sequence: u64) -> Result<Value, ApiError> {
        self.barrier(sequence, "DELETE FROM watch_history", None)
    }

    /// A delete publishes its sequence, so older accepted or retried snapshots cannot bring the row back.
    fn barrier(&mut self, sequence: u64, sql: &str, id: Option<&str>) -> Result<Value, ApiError> {
        if sequence <= self.committed {
            return Err(superseded());
        }
        let connection = self.connection()?;
        match id {
            Some(id) => connection.execute(sql, params![id]),
            None => connection.execute(sql, []),
        }
        .map_err(failure)?;
        self.committed = sequence;
        self.pending = None;
        Ok(json!({"ok":true}))
    }
}

fn validate_watchlist(entry: &Value) -> Result<(), ApiError> {
    Contracts::get().validate("WatchlistEntry", entry)?;
    let identity = &entry["metadata_identity"];
    let text = |key: &str| identity[key].as_str().unwrap_or("");
    if entry["content_id"]
        != crate::source_dispatch::identity(
            "metadata",
            &[text("namespace"), text("type"), text("canonical_id")],
        )
    {
        return Err(ApiError::bad(
            "Watchlist content ID differs from its metadata identity",
        ));
    }
    Ok(())
}

/// Initialize the current schema atomically; existing older schemas are rejected.
fn initialize_schema(connection: &mut Connection) -> Result<(), ApiError> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(failure)?;
    if version > VERSION {
        let mut error = ApiError::internal("History database comes from a newer version");
        error.retryable = false;
        return Err(error.with_details(json!({"reason":"newer_version"})));
    }
    if version == VERSION {
        return Ok(());
    }
    if version != 0 {
        let mut error = ApiError::internal("Unsupported history database version");
        error.retryable = false;
        return Err(error.with_details(json!({"reason":"unsupported_version"})));
    }
    let transaction = connection.transaction().map_err(failure)?;
    transaction
        .execute_batch(
            "CREATE TABLE watch_history (
                content_id TEXT PRIMARY KEY NOT NULL,
                metadata_config_id TEXT NOT NULL,
                metadata_addon_key TEXT NOT NULL,
                metadata_namespace TEXT NOT NULL,
                metadata_type TEXT NOT NULL,
                metadata_id TEXT NOT NULL,
                canonical_id TEXT NOT NULL,
                video_id TEXT,
                season_number INTEGER,
                episode_number REAL,
                title TEXT NOT NULL,
                kind TEXT NOT NULL,
                poster_cache_key TEXT,
                source_id TEXT NOT NULL,
                source_name TEXT NOT NULL,
                position_ms INTEGER NOT NULL CHECK (position_ms >= 0),
                duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms > 0),
                completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX watch_history_recent ON watch_history (updated_at DESC, content_id);
            ",
        )
        .map_err(failure)?;
    transaction
        .execute_batch(
            "CREATE TABLE watchlist (
            content_id TEXT PRIMARY KEY NOT NULL,
            metadata_config_id TEXT NOT NULL, metadata_addon_key TEXT NOT NULL,
            metadata_namespace TEXT NOT NULL, metadata_type TEXT NOT NULL,
            metadata_id TEXT NOT NULL, canonical_id TEXT NOT NULL,
            title TEXT NOT NULL, kind TEXT NOT NULL, poster_cache_key TEXT
        );
        PRAGMA user_version = 2;",
        )
        .map_err(failure)?;
    transaction.commit().map_err(failure)
}
