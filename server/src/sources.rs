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
        let mut stmt=conn.prepare("SELECT id,client_config_id,name,kind,api,ext,headers,enabled,parse_ids,revision,status,checked_at FROM playback_sources ORDER BY rowid")?;
        let rows = stmt.query_map([], row)?;
        rows.collect()
    }
    pub fn get(&self, id: &str) -> rusqlite::Result<Option<Value>> {
        self.conn()?.query_row("SELECT id,client_config_id,name,kind,api,ext,headers,enabled,parse_ids,revision,status,checked_at FROM playback_sources WHERE id=?1",[id],row).optional()
    }
    pub fn remove(&self, id: &str) -> rusqlite::Result<bool> {
        Ok(self
            .conn()?
            .execute("DELETE FROM playback_sources WHERE id=?1", [id])?
            > 0)
    }
    pub fn status(&self, id: &str, revision: u64, status: &str) -> rusqlite::Result<bool> {
        Ok(self.conn()?.execute(
            "UPDATE playback_sources SET status=?1,checked_at=?2 WHERE id=?3 AND revision=?4",
            params![status, crate::metadata::cache::now(), id, revision],
        )? > 0)
    }
    pub fn save(&self, input: &Value, id: Option<&str>) -> Result<Value, &'static str> {
        let value = validate(input)?;
        let mut conn = self.conn().map_err(|_| "source database failed")?;
        let tx = conn.transaction().map_err(|_| "source database failed")?;
        let client = value["client_config_id"].as_str();
        let existing = if let Some(id) = id {
            tx.query_row("SELECT id,client_config_id,name,kind,api,ext,headers,enabled,parse_ids,revision,status,checked_at FROM playback_sources WHERE id=?1",[id],row).optional().map_err(|_|"source database failed")?
                .ok_or("source not found").map(Some)?
        } else if let Some(client) = client {
            tx.query_row("SELECT id,client_config_id,name,kind,api,ext,headers,enabled,parse_ids,revision,status,checked_at FROM playback_sources WHERE client_config_id=?1",[client],row).optional().map_err(|_|"source database failed")?
        } else {
            None
        };
        if let Some(old) = &existing {
            if id.is_some() && old["client_config_id"] != value["client_config_id"] {
                return Err("client identity cannot change");
            }
        }
        let source_id = existing
            .as_ref()
            .and_then(|v| v["id"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let changed = existing.as_ref().is_none_or(|old| {
            [
                "name",
                "kind",
                "api",
                "ext",
                "headers",
                "enabled",
                "parse_ids",
            ]
            .iter()
            .any(|key| old[*key] != value[*key])
        });
        let revision = existing
            .as_ref()
            .and_then(|v| v["revision"].as_u64())
            .unwrap_or(0)
            + u64::from(changed);
        let status = if value["enabled"] == false {
            "disabled"
        } else if changed {
            "configured"
        } else {
            existing
                .as_ref()
                .and_then(|v| v["status"].as_str())
                .unwrap_or("configured")
        };
        let checked = if changed {
            None
        } else {
            existing.as_ref().and_then(|v| v["checked_at"].as_i64())
        };
        tx.execute("INSERT INTO playback_sources(id,client_config_id,name,kind,api,ext,headers,enabled,parse_ids,revision,status,checked_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(id) DO UPDATE SET name=excluded.name,kind=excluded.kind,api=excluded.api,ext=excluded.ext,headers=excluded.headers,enabled=excluded.enabled,parse_ids=excluded.parse_ids,revision=excluded.revision,status=excluded.status,checked_at=excluded.checked_at",
            params![source_id,client,value["name"].as_str(),value["kind"].as_str(),value["api"].as_str(),value["ext"].to_string(),value["headers"].to_string(),value["enabled"].as_bool(),value["parse_ids"].to_string(),revision,status,checked]).map_err(|_|"source database failed")?;
        tx.commit().map_err(|_| "source database failed")?;
        self.get(&source_id)
            .map_err(|_| "source database failed")?
            .ok_or("source database failed")
    }
}

pub fn validate(input: &Value) -> Result<Value, &'static str> {
    let name = input["name"]
        .as_str()
        .filter(|v| !v.trim().is_empty() && v.len() <= 128)
        .ok_or("invalid source name")?;
    let kind = input["kind"]
        .as_str()
        .filter(|v| matches!(*v, "maccms_json" | "maccms_xml" | "t4_query" | "t4_drpyjs0"))
        .ok_or("unsupported source kind")?;
    let api = input["api"].as_str().ok_or("missing source URL")?;
    let parsed = Url::parse(api).map_err(|_| "invalid source URL")?;
    if !crate::metadata::http::valid(&parsed) || parsed.fragment().is_some() {
        return Err("invalid source URL");
    }
    let client = input["client_config_id"].as_str();
    if input
        .get("client_config_id")
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        return Err("invalid client identity");
    }
    if client.is_some_and(|v| v.is_empty() || v.len() > 256) {
        return Err("invalid client identity");
    }
    let enabled = input
        .get("enabled")
        .map(|v| v.as_bool().ok_or("invalid enabled state"))
        .transpose()?
        .unwrap_or(true);
    let headers = input
        .get("headers")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let values = headers.as_object().ok_or("invalid source headers")?;
    if values.len() > 32 {
        return Err("too many source headers");
    }
    for (name, value) in values {
        let header = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| "invalid source header")?;
        if matches!(
            header.as_str(),
            "host" | "content-length" | "connection" | "proxy-authorization"
        ) {
            return Err("disallowed source header");
        }
        reqwest::header::HeaderValue::from_str(value.as_str().ok_or("invalid source header")?)
            .map_err(|_| "invalid source header")?;
    }
    let parse_ids = input
        .get("parse_ids")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| json!([]));
    if !parse_ids.as_array().is_some_and(|a| {
        a.len() <= 32
            && a.iter()
                .all(|v| v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256))
    }) {
        return Err("invalid parser IDs");
    }
    let ext = input.get("ext").cloned().unwrap_or(Value::Null);
    if ext.to_string().len() > 64 * 1024 {
        return Err("source extension too large");
    }
    Ok(
        json!({"name":name,"kind":kind,"api":api,"client_config_id":client,"enabled":enabled,"headers":headers,"parse_ids":parse_ids,"ext":ext}),
    )
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let parse = |text: String| {
        serde_json::from_str::<Value>(&text).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    Ok(
        json!({"id":row.get::<_,String>(0)?,"client_config_id":row.get::<_,Option<String>>(1)?,"name":row.get::<_,String>(2)?,"kind":row.get::<_,String>(3)?,"api":row.get::<_,String>(4)?,"ext":parse(row.get(5)?)?,"headers":parse(row.get(6)?)?,"enabled":row.get::<_,bool>(7)?,"parse_ids":parse(row.get(8)?)?,"revision":row.get::<_,u64>(9)?,"status":row.get::<_,String>(10)?,"checked_at":row.get::<_,Option<i64>>(11)?}),
    )
}
