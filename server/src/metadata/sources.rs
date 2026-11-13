use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use url::Url;

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
    pub fn list(&self) -> rusqlite::Result<Vec<Value>> {
        let conn = self.conn()?;
        let mut statement=conn.prepare("SELECT id,origin_config_id,origin_addon_key,origin_namespace_rule,origin_order,manifest_url,manifest,revision FROM metadata_sources ORDER BY origin_config_id,origin_order")?;
        let rows = statement.query_map([], row)?;
        rows.collect()
    }
    pub fn get(&self, id: &str) -> rusqlite::Result<Option<Value>> {
        self.conn()?.query_row("SELECT id,origin_config_id,origin_addon_key,origin_namespace_rule,origin_order,manifest_url,manifest,revision FROM metadata_sources WHERE id=?1",[id],row).optional()
    }
    pub fn delete(&self, id: &str) -> rusqlite::Result<bool> {
        Ok(self
            .conn()?
            .execute("DELETE FROM metadata_sources WHERE id=?1", [id])?
            > 0)
    }

    pub async fn register(&self, config: &str, item: &Value) -> Result<Value, &'static str> {
        let key = required(item, "addon_key")?;
        let original_url = required(item, "manifest_url")?;
        let url = Url::parse(original_url).map_err(|_| "invalid manifest URL")?;
        if !super::http::valid(&url) || !url.path().ends_with("/manifest.json") {
            return Err("invalid manifest URL");
        }
        let order = item["order"]
            .as_u64()
            .filter(|v| *v < 16)
            .ok_or("invalid manifest order")?;
        let namespace = item
            .get("namespace_rule")
            .filter(|v| v.is_object() || v.is_string())
            .ok_or("missing namespace rule")?;
        let manifest = super::http::json(url, Default::default())
            .await
            .map_err(|_| "manifest request failed")?;
        if manifest["id"].as_str().filter(|s| !s.is_empty()).is_none()
            || !manifest["resources"].is_array()
            || !manifest["types"].is_array()
            || manifest["name"].as_str().is_none_or(str::is_empty)
            || manifest["version"].as_str().is_none_or(str::is_empty)
            || !manifest["description"].is_string()
            || !manifest["catalogs"].is_array()
        {
            return Err("invalid Stremio manifest");
        }
        let mut conn = self.conn().map_err(|_| "source database failed")?;
        let tx = conn.transaction().map_err(|_| "source database failed")?;
        let old:Option<(String,String,String,String,i64)> = tx.query_row("SELECT id,manifest_url,manifest,origin_namespace_rule,origin_order FROM metadata_sources WHERE origin_config_id=?1 AND origin_addon_key=?2",params![config,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(|_|"source database failed")?;
        let id = old
            .as_ref()
            .map(|v| v.0.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let body = manifest.to_string();
        let rule = namespace.to_string();
        let changed =
            old.as_ref()
                .is_some_and(|(_, old_url, old_manifest, old_rule, old_order)| {
                    old_url != original_url
                        || old_manifest != &body
                        || old_rule != &rule
                        || *old_order != order as i64
                });
        tx.execute("INSERT INTO metadata_sources(id,origin_config_id,origin_addon_key,origin_namespace_rule,origin_order,manifest_url,manifest,revision) VALUES(?1,?2,?3,?4,?5,?6,?7,1) ON CONFLICT(origin_config_id,origin_addon_key) DO UPDATE SET origin_namespace_rule=excluded.origin_namespace_rule,origin_order=excluded.origin_order,manifest_url=excluded.manifest_url,manifest=excluded.manifest,revision=metadata_sources.revision+?8",params![id,config,key,rule,order,original_url,body,if changed {1} else {0}]).map_err(|_|"source database failed")?;
        tx.commit().map_err(|_| "source database failed")?;
        self.get(&id)
            .map_err(|_| "source database failed")?
            .ok_or("source database failed")
    }
}

pub fn required<'a>(item: &'a Value, key: &str) -> Result<&'a str, &'static str> {
    item[key]
        .as_str()
        .filter(|v| !v.trim().is_empty() && v.len() <= 2048)
        .ok_or("missing source field")
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let rule: String = row.get(3)?;
    let manifest: String = row.get(6)?;
    let manifest_url: String = row.get(5)?;
    let localized = Url::parse(&manifest_url)
        .ok()
        .and_then(|url| super::stremio::localized_manifest(&url, "zh-CN"))
        .is_some();
    let parse = |text: &str| {
        serde_json::from_str::<Value>(text).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    Ok(
        json!({"metadata_source_id":row.get::<_,String>(0)?,"origin_config_id":row.get::<_,String>(1)?,"origin_addon_key":row.get::<_,String>(2)?,"origin_namespace_rule":parse(&rule)?,"origin_order":row.get::<_,u64>(4)?,"manifest_url":manifest_url,"manifest":parse(&manifest)?,"revision":row.get::<_,u64>(7)?,"language_status":if localized {"localized"} else {"original"}}),
    )
}
