use rusqlite::params;
use serde_json::Value;
use std::{fs::OpenOptions, io, path::Path};

const MAPPINGS: &str = "CREATE TABLE IF NOT EXISTS mappings (
        id TEXT PRIMARY KEY, metadata_source_id TEXT NOT NULL, media_type TEXT NOT NULL,
        meta_id TEXT NOT NULL, source_id TEXT NOT NULL, source_revision INTEGER NOT NULL,
        content_id TEXT NOT NULL, confirmed_by TEXT NOT NULL, evidence TEXT NOT NULL,
        updated_at INTEGER NOT NULL, revoked INTEGER NOT NULL DEFAULT 0,
        season INTEGER NOT NULL DEFAULT 0,
        UNIQUE(metadata_source_id,media_type,meta_id,season,source_id));";

/// Mappings created before the season key keep their rows. A series row takes
/// the season its source title names, otherwise the first season.
fn add_mapping_seasons(conn: &mut rusqlite::Connection) -> rusqlite::Result<()> {
    let legacy: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='mappings')
         AND NOT EXISTS(SELECT 1 FROM pragma_table_info('mappings') WHERE name='season')",
        [],
        |row| row.get(0),
    )?;
    if !legacy {
        return Ok(());
    }
    let tx = conn.transaction()?;
    tx.execute_batch("ALTER TABLE mappings RENAME TO mappings_without_season;")?;
    tx.execute_batch(MAPPINGS)?;
    let rows = {
        let mut statement = tx.prepare(
            "SELECT id,metadata_source_id,media_type,meta_id,source_id,source_revision,content_id,
             confirmed_by,evidence,updated_at,revoked FROM mappings_without_season",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for row in rows {
        let season = if row.2 == "series" {
            serde_json::from_str::<Value>(&row.8)
                .ok()
                .and_then(|evidence| {
                    evidence["source_title"]
                        .as_str()
                        .and_then(crate::matching::title_season)
                })
                .unwrap_or(1)
        } else {
            0
        };
        tx.execute(
            "INSERT INTO mappings(id,metadata_source_id,media_type,meta_id,source_id,source_revision,
             content_id,confirmed_by,evidence,updated_at,revoked,season)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9, row.10, season
            ],
        )?;
    }
    tx.execute_batch("DROP TABLE mappings_without_season;")?;
    tx.commit()
}

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
    let mut conn = rusqlite::Connection::open(path)?;
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
    conn.execute_batch("CREATE TABLE IF NOT EXISTS json_parsers (id TEXT PRIMARY KEY, client_config_id TEXT UNIQUE, payload TEXT NOT NULL);")?;
    add_mapping_seasons(&mut conn)?;
    conn.execute_batch(MAPPINGS)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mapping_revocations (
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mappings_gain_season_key_and_keep_rows() {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(dir.path().join("service.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE mappings (
            id TEXT PRIMARY KEY, metadata_source_id TEXT NOT NULL, media_type TEXT NOT NULL,
            meta_id TEXT NOT NULL, source_id TEXT NOT NULL, source_revision INTEGER NOT NULL,
            content_id TEXT NOT NULL, confirmed_by TEXT NOT NULL, evidence TEXT NOT NULL,
            updated_at INTEGER NOT NULL, revoked INTEGER NOT NULL DEFAULT 0,
            UNIQUE(metadata_source_id,media_type,meta_id,source_id));",
        )
        .unwrap();
        for (id, kind, content, title) in [
            ("a", "movie", "101", "电影"),
            ("b", "series", "201", "剧集第二季"),
            ("c", "series", "301", "其他剧"),
        ] {
            conn.execute(
                "INSERT INTO mappings VALUES(?1,'tmdb',?2,?3,'source',1,?4,'user',?5,1,0)",
                params![
                    id,
                    kind,
                    format!("tmdb:{id}"),
                    content,
                    json!({"source_title":title}).to_string()
                ],
            )
            .unwrap();
        }
        drop(conn);
        initialize(dir.path()).unwrap();
        initialize(dir.path()).unwrap();
        let store = crate::mappings::Store::new(dir.path());
        let season =
            |kind: &str, id: &str| store.list("tmdb", kind, id).unwrap()[0]["season"].clone();
        assert_eq!(season("movie", "tmdb:a"), 0);
        assert_eq!(season("series", "tmdb:b"), 2);
        assert_eq!(season("series", "tmdb:c"), 1);
        assert_eq!(
            store.list("tmdb", "series", "tmdb:b").unwrap()[0]["confirmed_by"],
            "user"
        );
        // Each season of one show now keeps its own source content.
        let source = json!({"id":"source","revision":1});
        for (season, content) in [(1, "s1"), (2, "s2"), (1, "s1-new")] {
            let meta = json!({"metadata_source_id":"tmdb","type":"series","meta_id":"tmdb:show","episode":{"season":season,"number":1}});
            store
                .save(&meta, &source, content, &json!({}), "user")
                .unwrap();
        }
        let rows = store.list("tmdb", "series", "tmdb:show").unwrap();
        let mut kept: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    row["season"].as_u64().unwrap(),
                    row["content_id"].as_str().unwrap(),
                )
            })
            .collect();
        kept.sort();
        assert_eq!(kept, vec![(1, "s1-new"), (2, "s2")]);
    }
}
