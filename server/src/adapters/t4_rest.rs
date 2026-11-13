use super::{maccms, t4_query};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, Weak},
};
use tokio::sync::{Mutex as AsyncMutex, OnceCell};
use url::Url;

type ContextCell = Arc<OnceCell<Value>>;
type Contexts = HashMap<String, (u64, ContextCell)>;

fn contexts() -> &'static Mutex<Contexts> {
    static CONTEXTS: OnceLock<Mutex<Contexts>> = OnceLock::new();
    CONTEXTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn endpoint_gate(source: &Value) -> Result<Arc<AsyncMutex<()>>, &'static str> {
    static GATES: OnceLock<Mutex<HashMap<String, Weak<AsyncMutex<()>>>>> = OnceLock::new();
    let mut url = endpoint(source, "init")?;
    url.set_query(None);
    let mut entries = GATES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "CatVod context unavailable")?;
    entries.retain(|_, gate| gate.strong_count() > 0);
    let gate = entries
        .get(url.as_str())
        .and_then(Weak::upgrade)
        .unwrap_or_else(|| Arc::new(AsyncMutex::new(())));
    entries.insert(url.to_string(), Arc::downgrade(&gate));
    Ok(gate)
}

fn endpoint(source: &Value, route: &str) -> Result<Url, &'static str> {
    let mut url = Url::parse(source["api"].as_str().ok_or("invalid source URL")?)
        .map_err(|_| "invalid source URL")?;
    {
        let mut path = url.path_segments_mut().map_err(|_| "invalid source URL")?;
        path.pop_if_empty();
        path.push(route);
    }
    Ok(url)
}

async fn request(source: &Value, route: &str, body: &Value) -> Result<Value, &'static str> {
    let value =
        crate::metadata::http::post_json(endpoint(source, route)?, maccms::headers(source)?, body)
            .await
            .map_err(|_| "CatVod request failed or returned invalid JSON")?;
    if value
        .get("error")
        .is_some_and(|error| !error.is_null() && error != "")
    {
        return Err("CatVod source returned an error");
    }
    Ok(value)
}

async fn initialized(source: &Value) -> Result<(String, ContextCell, Value), &'static str> {
    if source["enabled"] != true {
        return Err("source is disabled");
    }
    let id = source["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("source ID missing")?
        .to_owned();
    let revision = source["revision"]
        .as_u64()
        .ok_or("source revision missing")?;
    let cell = {
        let mut entries = contexts()
            .lock()
            .map_err(|_| "CatVod context unavailable")?;
        if entries.len() >= 128 && !entries.contains_key(&id) {
            if let Some(key) = entries.keys().next().cloned() {
                entries.remove(&key);
            }
        }
        let entry = entries
            .entry(id.clone())
            .or_insert_with(|| (revision, Arc::new(OnceCell::new())));
        if entry.0 != revision {
            *entry = (revision, Arc::new(OnceCell::new()));
        }
        entry.1.clone()
    };
    let context = cell
        .get_or_try_init(|| async {
            let body = if source["ext"].is_null() {
                json!({})
            } else {
                json!({"ext":source["ext"]})
            };
            let value = request(source, "init", &body).await?;
            let context = value.get("context").cloned().unwrap_or(Value::Null);
            if context.to_string().len() > 65536 {
                return Err("CatVod context exceeds limit");
            }
            Ok::<Value, &'static str>(context)
        })
        .await?
        .clone();
    Ok((id, cell, context))
}

async fn call(source: &Value, route: &str, mut body: Value) -> Result<Value, &'static str> {
    let gate = endpoint_gate(source)?;
    let mut guard = Some(gate.lock().await);
    let (id, cell, context) = initialized(source).await?;
    let shared_context = context.is_null();
    if !shared_context {
        guard.take();
        body["context"] = context;
    }
    let result = request(source, route, &body).await;
    if shared_context || result.is_err() {
        if let Ok(mut entries) = contexts().lock() {
            if entries
                .get(&id)
                .is_some_and(|entry| Arc::ptr_eq(&entry.1, &cell))
            {
                entries.remove(&id);
            }
        }
    }
    result
}

pub async fn search(source: &Value, keyword: &str) -> Result<Vec<Value>, &'static str> {
    if keyword.trim().is_empty() {
        return Err("search query is empty");
    }
    t4_query::items(&call(source, "search", json!({"wd":keyword,"pg":1})).await?)?
        .iter()
        .map(maccms::card)
        .collect()
}

pub async fn detail(source: &Value, id: &str) -> Result<Value, &'static str> {
    if id.is_empty() {
        return Err("content ID is empty");
    }
    let values = t4_query::items(&call(source, "detail", json!({"id":id})).await?)?;
    maccms::normalize_detail(&values, id)
}

pub async fn play(
    source: &Value,
    flag: &str,
    play: &str,
    item: &Value,
) -> Result<Vec<Value>, &'static str> {
    let response = call(source, "play", json!({"flag":flag,"id":play})).await?;
    t4_query::play_values(source, &response, item)
}
