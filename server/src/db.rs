use std::{fs::OpenOptions, io, path::Path};

pub fn initialize(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = dir.join("service.sqlite3");
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(&path).map_err(|error| {
        io::Error::new(error.kind(), format!("opening service database: {error}"))
    })?);
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS metadata_cache (cache_key TEXT PRIMARY KEY, payload TEXT NOT NULL, expires_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS home_snapshots (revision TEXT PRIMARY KEY, built_at INTEGER NOT NULL, payload TEXT NOT NULL);")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata_sources (
        id TEXT PRIMARY KEY, origin_config_id TEXT NOT NULL, origin_addon_key TEXT NOT NULL,
        origin_namespace_rule TEXT NOT NULL, origin_order INTEGER NOT NULL,
        manifest_url TEXT NOT NULL, manifest TEXT NOT NULL, revision INTEGER NOT NULL,
        UNIQUE(origin_config_id,origin_addon_key));",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS playback_sources (
        id TEXT PRIMARY KEY, client_config_id TEXT UNIQUE, name TEXT NOT NULL,
        kind TEXT NOT NULL, api TEXT NOT NULL, ext TEXT, headers TEXT NOT NULL,
        enabled INTEGER NOT NULL, parse_ids TEXT NOT NULL, revision INTEGER NOT NULL,
        status TEXT NOT NULL, checked_at INTEGER);",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mappings (
        id TEXT PRIMARY KEY, metadata_source_id TEXT NOT NULL, media_type TEXT NOT NULL,
        meta_id TEXT NOT NULL, source_id TEXT NOT NULL, source_revision INTEGER NOT NULL,
        content_id TEXT NOT NULL, confirmed_by TEXT NOT NULL, evidence TEXT NOT NULL,
        updated_at INTEGER NOT NULL, revoked INTEGER NOT NULL DEFAULT 0,
        UNIQUE(metadata_source_id,media_type,meta_id,source_id));
        CREATE TABLE IF NOT EXISTS mapping_revocations (
        metadata_source_id TEXT NOT NULL, media_type TEXT NOT NULL, meta_id TEXT NOT NULL,
        source_id TEXT NOT NULL, content_id TEXT NOT NULL,
        PRIMARY KEY(metadata_source_id,media_type,meta_id,source_id,content_id));",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS line_stats (
        logical_key TEXT PRIMARY KEY, source_id TEXT NOT NULL, content_id TEXT NOT NULL,
        ok_count INTEGER NOT NULL DEFAULT 0, fail_count INTEGER NOT NULL DEFAULT 0,
        last_ok_at INTEGER, last_latency_ms INTEGER);",
    )?;
    Ok(())
}
