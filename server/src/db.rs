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
    Ok(())
}
