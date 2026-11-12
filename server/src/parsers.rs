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
        let mut stmt = conn.prepare("SELECT payload FROM json_parsers ORDER BY rowid")?;
        let rows = stmt.query_map([], row)?;
        rows.collect()
    }
    pub fn remove(&self, id: &str) -> rusqlite::Result<bool> {
        Ok(self
            .conn()?
            .execute("DELETE FROM json_parsers WHERE id=?1", [id])?
            > 0)
    }
    pub fn save(&self, input: &Value, id: Option<&str>) -> Result<Value, &'static str> {
        let mut value = validate(input)?;
        let mut conn = self.conn().map_err(|_| "parser database failed")?;
        let tx = conn.transaction().map_err(|_| "parser database failed")?;
        let old = if let Some(id) = id {
            Some(
                tx.query_row("SELECT payload FROM json_parsers WHERE id=?1", [id], row)
                    .optional()
                    .map_err(|_| "parser database failed")?
                    .ok_or("parser not found")?,
            )
        } else if let Some(client) = value["client_config_id"].as_str() {
            tx.query_row(
                "SELECT payload FROM json_parsers WHERE client_config_id=?1",
                [client],
                row,
            )
            .optional()
            .map_err(|_| "parser database failed")?
        } else {
            None
        };
        if id.is_some()
            && old
                .as_ref()
                .is_some_and(|v| v["client_config_id"] != value["client_config_id"])
        {
            return Err("client identity cannot change");
        }
        let parser_id = old
            .as_ref()
            .and_then(|v| v["id"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let changed = old.as_ref().is_none_or(|v| {
            [
                "name",
                "api",
                "flags",
                "headers",
                "enabled",
                "url_param",
                "url_field",
            ]
            .iter()
            .any(|key| v[*key] != value[*key])
        });
        value["id"] = json!(parser_id);
        value["revision"] = json!(
            old.as_ref()
                .and_then(|v| v["revision"].as_u64())
                .unwrap_or(0)
                + u64::from(changed)
        );
        tx.execute("INSERT INTO json_parsers(id,client_config_id,payload) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![parser_id,value["client_config_id"].as_str(),value.to_string()]).map_err(|_| "parser database failed")?;
        tx.commit().map_err(|_| "parser database failed")?;
        Ok(value)
    }
}
fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let text: String = row.get(0)?;
    serde_json::from_str(&text).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}
fn validate(input: &Value) -> Result<Value, &'static str> {
    let name = input["name"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 128)
        .ok_or("invalid parser name")?;
    let api = input["api"].as_str().ok_or("missing parser URL")?;
    let url = Url::parse(api).map_err(|_| "invalid parser URL")?;
    if !crate::metadata::http::valid(&url) || url.fragment().is_some() {
        return Err("invalid parser URL");
    }
    let client = input
        .get("client_config_id")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 256)
                .ok_or("invalid client identity")
        })
        .transpose()?;
    let enabled = input
        .get("enabled")
        .map(|v| v.as_bool().ok_or("invalid enabled state"))
        .transpose()?
        .unwrap_or(true);
    let flags = input.get("flags").cloned().unwrap_or_else(|| json!([]));
    if !flags.as_array().is_some_and(|a| {
        a.len() <= 32
            && a.iter()
                .all(|v| v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 128))
    }) {
        return Err("invalid parser flags");
    }
    let headers = input.get("headers").cloned().unwrap_or_else(|| json!({}));
    let values = headers
        .as_object()
        .filter(|v| v.len() <= 32)
        .ok_or("invalid parser headers")?;
    for (name, value) in values {
        let header = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| "invalid parser header")?;
        if matches!(
            header.as_str(),
            "host" | "content-length" | "connection" | "transfer-encoding" | "proxy-authorization"
        ) {
            return Err("disallowed parser header");
        }
        reqwest::header::HeaderValue::from_str(value.as_str().ok_or("invalid parser header")?)
            .map_err(|_| "invalid parser header")?;
    }
    let field = |key: &str, default: &'static str| -> Result<String, &'static str> {
        let val = input
            .get(key)
            .map(|v| v.as_str().ok_or("invalid parser field"))
            .transpose()?
            .unwrap_or(default);
        if val.is_empty()
            || val.len() > 128
            || !val
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
        {
            return Err("invalid parser field");
        }
        Ok(val.to_owned())
    };
    Ok(
        json!({"client_config_id":client,"name":name,"kind":"json","api":api,"flags":flags,"headers":headers,"enabled":enabled,"url_param":field("url_param","url")?,"url_field":field("url_field","url")?}),
    )
}
