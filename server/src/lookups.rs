use crate::{
    AppState, line_cache, matching,
    state::{Candidate, Lookup, Template},
};
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use std::{collections::HashMap, convert::Infallible, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":message,"retryable":status.is_server_error()}})),
    )
        .into_response()
}
fn expired(reason: &str) -> Response {
    (StatusCode::GONE,Json(json!({"error":{"code":"SESSION_EXPIRED","message":"Lookup or line expired","retryable":true,"reason":reason}}))).into_response()
}
fn retire(state: &AppState, id: &str, reason: &str) -> Response {
    let mut registry = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    if let Some(line) = registry.lines.remove(id) {
        registry.changed(&line.source_id);
    }
    expired(reason)
}

fn response(rx: mpsc::Receiver<Value>) -> Response {
    let stream = ReceiverStream::new(rx).map(|value| Ok::<_, Infallible>(format!("{}\n", value)));
    (
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}
async fn send(tx: &mpsc::Sender<Value>, value: Value) -> bool {
    tx.send(value).await.is_ok()
}

async fn metadata(state: &AppState, input: &Value) -> Result<Value, &'static str> {
    let source = input["metadata_source_id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("missing metadata source")?;
    let kind = input["type"]
        .as_str()
        .filter(|s| matches!(*s, "movie" | "series"))
        .ok_or("invalid metadata type")?;
    let id = input["meta_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .ok_or("invalid metadata ID")?;
    let video_id = input["video_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .ok_or("invalid video ID")?;
    let lang = input["lang"]
        .as_str()
        .filter(|s| matches!(*s, "zh-CN" | "en-US"))
        .ok_or("invalid language")?;
    let (body, revision, namespace_prefixes) = if source == "tmdb" {
        (
            state.tmdb.meta(lang, kind, id).await?,
            0,
            Vec::<String>::new(),
        )
    } else {
        let entry = state
            .sources
            .get(source)
            .map_err(|_| "metadata database failed")?
            .ok_or("metadata source not found")?;
        let revision = entry["revision"]
            .as_u64()
            .ok_or("invalid metadata revision")?;
        let manifest = &entry["manifest"];
        let meta_resource = manifest["resources"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|value| value.as_str() == Some("meta") || value["name"] == "meta");
        let prefixes = meta_resource
            .and_then(|value| {
                if value.is_object() {
                    value.get("idPrefixes").and_then(Value::as_array)
                } else {
                    manifest["idPrefixes"].as_array()
                }
            })
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        (
            crate::metadata::stremio::response(
                &entry,
                &state.cache,
                lang,
                &["meta".into(), kind.into(), format!("{id}.json")],
            )
            .await?,
            revision,
            prefixes,
        )
    };
    let meta = &body["meta"];
    if meta["id"] != id || meta["type"] != kind {
        return Err("metadata identity mismatch");
    }
    let videos = meta["videos"].as_array().cloned().unwrap_or_default();
    let matching: Vec<_> = videos
        .iter()
        .filter(|video| video["id"] == video_id)
        .collect();
    let episode = if kind == "series" {
        if matching.len() != 1 {
            return Err("target video missing or ambiguous");
        }
        let video = matching[0];
        let season = video["season"].as_u64().ok_or("target season missing")?;
        let number = video["episode"]
            .as_u64()
            .or_else(|| video["number"].as_u64())
            .ok_or("target episode missing")?;
        json!({"season":season,"number":number})
    } else {
        let implicit = video_id == id
            || meta["imdb_id"].as_str() == Some(video_id)
            || meta["behaviorHints"]["defaultVideoId"].as_str() == Some(video_id);
        if !implicit && matching.len() != 1 {
            return Err("target video missing or ambiguous");
        }
        if matching.len() > 1 {
            return Err("target video missing or ambiguous");
        }
        Value::Null
    };
    let seasons: Vec<u64> = videos.iter().filter_map(|v| v["season"].as_u64()).collect();
    let imdb_id = meta["imdb_id"]
        .as_str()
        .filter(|value| crate::adapters::stremio_stream::imdb(value))
        .filter(|value| {
            source == "tmdb"
                || id == *value
                || video_id == *value
                || video_id
                    .strip_prefix(*value)
                    .is_some_and(|suffix| suffix.starts_with(':'))
        })
        .or_else(|| crate::adapters::stremio_stream::imdb(id).then_some(id));
    Ok(
        json!({"metadata_source_id":source,"metadata_revision":revision,"type":kind,"meta_id":id,"video_id":video_id,"lang":lang,
        "title":meta["name"],"original_title":meta["original_title"],"english_title":meta["english_title"],"aliases":meta["aliases"],"year":meta["year"],"episode":episode,"seasons":seasons,"imdb_id":imdb_id,"namespace_prefixes":namespace_prefixes}),
    )
}

fn target(metadata: &Value) -> Value {
    json!({"metadata_source_id":metadata["metadata_source_id"],"metadata_revision":metadata["metadata_revision"],"type":metadata["type"],"meta_id":metadata["meta_id"],"video_id":metadata["video_id"]})
}

fn metadata_current(state: &AppState, metadata: &Value) -> bool {
    let id = metadata["metadata_source_id"].as_str().unwrap_or("");
    if id == "tmdb" {
        return true;
    }
    state
        .sources
        .get(id)
        .ok()
        .flatten()
        .is_some_and(|source| source["revision"] == metadata["metadata_revision"])
}
fn source_current(state: &AppState, source: &Value) -> bool {
    source["id"]
        .as_str()
        .and_then(|id| state.playback_sources.get(id).ok().flatten())
        .is_some_and(|current| {
            current["enabled"] == true && current["revision"] == source["revision"]
        })
}

pub async fn create(State(state): State<AppState>, Json(input): Json<Value>) -> Response {
    let selected = match input.get("sources") {
        None | Some(Value::Null) => None,
        Some(Value::Array(items))
            if items.len() <= 32
                && items
                    .iter()
                    .all(|v| v.as_str().is_some_and(|s| !s.is_empty())) =>
        {
            Some(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            )
        }
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Invalid source list",
            );
        }
    };
    let refresh = match input.get("refresh") {
        None | Some(Value::Bool(false)) => false,
        Some(Value::Bool(true)) => true,
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Invalid refresh flag",
            );
        }
    };
    let metadata = match metadata(&state, &input).await {
        Ok(value) => value,
        Err("metadata source not found") => {
            return error(
                StatusCode::NOT_FOUND,
                "SOURCE_NOT_FOUND",
                "Metadata source not found",
            );
        }
        Err(message) if message.starts_with("TMDB API key") => {
            return error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message);
        }
        Err(message)
            if message.starts_with("invalid")
                || message.starts_with("missing")
                || message.starts_with("target")
                || message == "metadata identity mismatch" =>
        {
            return error(StatusCode::BAD_REQUEST, "BAD_REQUEST", message);
        }
        Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
    };
    let sources = match state.playback_sources.list() {
        Ok(values) => values
            .into_iter()
            .filter(|source| {
                source["enabled"] == true
                    && selected.as_ref().is_none_or(|ids| {
                        source["id"]
                            .as_str()
                            .is_some_and(|id| ids.iter().any(|v| v == id))
                    })
            })
            .collect::<Vec<_>>(),
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Source database failed",
            );
        }
    };
    let now = crate::metadata::cache::now();
    let id = uuid::Uuid::new_v4().to_string();
    let (cancel, cancel_rx) = watch::channel(false);
    {
        let mut registry = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        registry.prune(now);
        if registry.lookups.len() >= 128
            || metadata.to_string().len() > 64 * 1024
            || registry.bytes().saturating_add(metadata.to_string().len()) > crate::state::MAX_BYTES
        {
            return error(
                StatusCode::TOO_MANY_REQUESTS,
                "RESOURCE_LIMIT",
                "Too many active lookups",
            );
        }
        registry.lookups.insert(
            id.clone(),
            Lookup {
                metadata: metadata.clone(),
                expires_at: now + state.lookup_runtime.handle_ttl,
                cancelled: cancel,
                candidates: HashMap::new(),
                active: true,
            },
        );
    }
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(run(
        state, id, metadata, sources, refresh, now, cancel_rx, tx,
    ));
    response(rx)
}

async fn run(
    state: AppState,
    id: String,
    metadata: Value,
    sources: Vec<Value>,
    refresh: bool,
    now: i64,
    mut cancel: watch::Receiver<bool>,
    tx: mpsc::Sender<Value>,
) {
    let summaries: Vec<_> = sources
        .iter()
        .map(|s| json!({"source_id":s["id"],"name":s["name"],"kind":s["kind"]}))
        .collect();
    if !send(&tx,json!({"event":"started","lookup_id":id,"lease_expires_at":now+state.lookup_runtime.handle_ttl,"sources":summaries})).await{return;}
    let mut jobs = JoinSet::new();
    let mut pending = HashMap::new();
    let limit = std::sync::Arc::new(tokio::sync::Semaphore::new(16));
    for source in sources {
        let state = state.clone();
        let id = id.clone();
        let metadata = metadata.clone();
        let tx = tx.clone();
        let mut cancellation = cancel.clone();
        let limit = limit.clone();
        let owner = source["id"].as_str().unwrap_or("").to_owned();
        let handle=jobs.spawn(async move {
            let source_id=source["id"].as_str().unwrap_or("").to_owned();
            if !send(&tx,json!({"event":"source","source_id":source_id,"status":"searching"})).await{return;}
            let task=async {let _permit=limit.acquire().await.map_err(|_|"lookup stopped")?; process(&state,&id,&metadata,&source,refresh,&tx).await};
            tokio::select! {
                _=cancellation.changed()=>{},
                result=tokio::time::timeout(Duration::from_secs(15),task)=>{
                    match result {
                        Ok(Ok((status,lines,reason)))=>{let mut value=json!({"event":"source","source_id":source_id,"status":status,"lines":lines});if let Some(reason)=reason{value["reason"]=json!(reason);}let _=send(&tx,value).await;}
                        Ok(Err(message))=>{let _=send(&tx,json!({"event":"source","source_id":source_id,"status":"failed","error":{"code":"UPSTREAM_FAILED","message":message}})).await;}
                        Err(_)=>{let _=send(&tx,json!({"event":"source","source_id":source_id,"status":"timeout"})).await;}
                    }
                }
            }
        });
        pending.insert(handle.id(), owner);
    }
    let deadline = tokio::time::sleep(Duration::from_secs(30));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _=tx.closed()=>{jobs.abort_all();finish(&state,&id);break;},
            _=cancel.changed()=>{jobs.abort_all();break;},
            _=&mut deadline=>{jobs.abort_all();for source_id in pending.values(){if !send(&tx,json!({"event":"source","source_id":source_id,"status":"timeout"})).await{break;}}let lease=finish(&state,&id);let _=send(&tx,json!({"event":"done","status":"timeout","lease_expires_at":lease})).await;break;},
            next=jobs.join_next_with_id()=>match next {
                Some(Ok((id,())))=>{pending.remove(&id);},
                Some(Err(failure))=>{if let Some(source_id)=pending.remove(&failure.id()){let _=send(&tx,json!({"event":"source","source_id":source_id,"status":"failed","error":{"code":"INTERNAL_ERROR","message":"Source worker failed"}})).await;}},
                None=>{
                    let lease=finish(&state,&id);
                    if let Some(lease)=lease {let _=send(&tx,json!({"event":"done","lease_expires_at":lease})).await;}
                    break;
                },
            },
        }
    }
}

fn finish(state: &AppState, id: &str) -> Option<i64> {
    let mut reg = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    let now = crate::metadata::cache::now();
    reg.prune(now);
    reg.lookups.get_mut(id).map(|lookup| {
        lookup.active = false;
        lookup.expires_at = now + state.lookup_runtime.handle_ttl;
        lookup.expires_at
    })
}

async fn process(
    state: &AppState,
    lookup_id: &str,
    metadata: &Value,
    source: &Value,
    refresh: bool,
    tx: &mpsc::Sender<Value>,
) -> Result<(&'static str, usize, Option<&'static str>), &'static str> {
    if !source_current(state, source) || !metadata_current(state, metadata) {
        return Err("source configuration changed");
    }
    let target = target(metadata);
    let now = crate::metadata::cache::now();
    let (cached, parser_generation) = {
        let mut reg = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        reg.prune(now);
        if !reg.lookups.contains_key(lookup_id) {
            return Err("lookup cancelled");
        }
        let generation = reg.parser_generation;
        let cached = if refresh {
            None
        } else {
            reg.cache
                .get(&line_cache::cache_key(&target, source, generation))
                .map(|entry| entry.template.clone())
        };
        (cached, generation)
    };
    if let Some(template) = cached {
        return emit_lines(state, lookup_id, metadata, &template, parser_generation, tx)
            .await
            .map(|(status, count)| (status, count, None));
    }
    if source["kind"] == "stremio_stream" {
        return match crate::adapters::stremio_stream::lookup(source, metadata, &state.cache).await?
        {
            crate::adapters::stremio_stream::ResultSet::Unsupported(reason) => {
                Ok(("unsupported", 0, Some(reason)))
            }
            crate::adapters::stremio_stream::ResultSet::Ready { request_id, lines } => {
                if lines.is_empty() {
                    return Ok(("not_found", 0, None));
                }
                let template = Template {
                    source: source.clone(),
                    content_id: request_id,
                    lines,
                };
                emit_lines(state, lookup_id, metadata, &template, parser_generation, tx)
                    .await
                    .map(|(status, count)| (status, count, None))
            }
        };
    }
    if let Some(reused) = state.mappings.reusable(metadata, source).await? {
        let template = Template {
            source: source.clone(),
            content_id: reused["mapping"]["content_id"]
                .as_str()
                .ok_or("invalid mapping")?
                .into(),
            lines: reused["lines"].as_array().cloned().unwrap_or_default(),
        };
        return emit_lines(state, lookup_id, metadata, &template, parser_generation, tx)
            .await
            .map(|(status, count)| (status, count, None));
    }
    let revoked = state
        .mappings
        .revoked(
            metadata["metadata_source_id"].as_str().unwrap_or(""),
            metadata["type"].as_str().unwrap_or(""),
            metadata["meta_id"].as_str().unwrap_or(""),
            source["id"].as_str().unwrap_or(""),
        )
        .map_err(|_| "mapping database failed")?;
    let proposal = matching::propose(source, metadata, &revoked).await?;
    let candidates = proposal["candidates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if candidates.is_empty() {
        return Ok((
            if proposal["missing_episode"] == true {
                "no_episode"
            } else {
                "not_found"
            },
            0,
            None,
        ));
    }
    if let Some(chosen) = proposal["selected"].as_str() {
        if let Some(candidate) = candidates.iter().find(|v| v["content_id"] == chosen) {
            let gate = state.metadata_gate.lock().await;
            let saved = {
                let reg = state
                    .lookup_runtime
                    .registry
                    .lock()
                    .expect("lookup registry");
                if reg.parser_generation != parser_generation
                    || tx.is_closed()
                    || !reg.lookups.contains_key(lookup_id)
                    || !source_current(state, source)
                    || !metadata_current(state, metadata)
                {
                    return Err("lookup or source changed");
                }
                let evidence = matching::evidence(metadata, &candidate["detail"]);
                state
                    .mappings
                    .save_checked(metadata, source, chosen, &evidence, "auto")
            };
            drop(gate);
            if saved
                .as_ref()
                .is_ok_and(|value| value["content_id"] == chosen)
            {
                let template = Template {
                    source: source.clone(),
                    content_id: chosen.into(),
                    lines: candidate["lines"].as_array().cloned().unwrap_or_default(),
                };
                return emit_lines(state, lookup_id, metadata, &template, parser_generation, tx)
                    .await
                    .map(|(status, count)| (status, count, None));
            }
            if let Err(error) = saved {
                if error != "mapping was revoked" {
                    return Err(error);
                }
            }
        }
    }
    let mut public = Vec::new();
    {
        let mut reg = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        if tx.is_closed() || !source_current(state, source) || !metadata_current(state, metadata) {
            return Err("source configuration changed");
        }
        let candidate_bytes = candidates
            .iter()
            .map(|candidate| candidate["detail"].to_string().len() + source.to_string().len() + 512)
            .sum::<usize>();
        if reg.bytes().saturating_add(candidate_bytes) > crate::state::MAX_BYTES {
            return Err("candidate memory limit reached");
        }
        let lookup = reg.lookups.get_mut(lookup_id).ok_or("lookup cancelled")?;
        if lookup.candidates.len() + candidates.len() > 256 {
            return Err("candidate limit reached");
        }
        for candidate in candidates {
            let Some(content_id) = candidate["content_id"].as_str() else {
                continue;
            };
            let id = uuid::Uuid::new_v4().to_string();
            lookup.candidates.insert(
                id.clone(),
                Candidate {
                    source: source.clone(),
                    content_id: content_id.into(),
                    detail: candidate["detail"].clone(),
                },
            );
            public.push(json!({"candidate_id":id,"title":candidate["title"],"year":candidate["year"],"kind":candidate["kind"],"mismatch":candidate["mismatch"]}));
        }
    }
    let _ = send(
        tx,
        json!({"event":"confirm","source_id":source["id"],"candidates":public}),
    )
    .await;
    Ok(("needs_confirmation", 0, None))
}

async fn emit_lines(
    state: &AppState,
    lookup_id: &str,
    metadata: &Value,
    template: &Template,
    parser_generation: u64,
    tx: &mpsc::Sender<Value>,
) -> Result<(&'static str, usize), &'static str> {
    let mut template = template.clone();
    template.lines = crate::adapters::resolve(&template.source, &template.lines).await?;
    template.lines =
        crate::adapters::json_parse::resolve(&state.parsers, &template.source, &template.lines)
            .await?;
    let _gate = state.metadata_gate.lock().await;
    let lines = {
        let mut registry = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        let now = crate::metadata::cache::now();
        registry.prune(now);
        if registry.parser_generation != parser_generation
            || tx.is_closed()
            || !registry.lookups.contains_key(lookup_id)
            || !source_current(state, &template.source)
            || !metadata_current(state, metadata)
        {
            return Err("lookup or source changed");
        }
        let revoked = state
            .mappings
            .revoked(
                metadata["metadata_source_id"].as_str().unwrap_or(""),
                metadata["type"].as_str().unwrap_or(""),
                metadata["meta_id"].as_str().unwrap_or(""),
                template.source["id"].as_str().unwrap_or(""),
            )
            .map_err(|_| "mapping database failed")?;
        if revoked.contains(&template.content_id) {
            return Err("mapping revoked");
        }
        let target = target(metadata);
        let values = line_cache::issue(
            &mut registry,
            &state.lookup_runtime,
            &target,
            &template,
            &state.reports,
        )?;
        registry.cache.insert(
            line_cache::cache_key(&target, &template.source, parser_generation),
            crate::state::CacheEntry {
                expires_at: now + state.lookup_runtime.cache_ttl,
                template: template.clone(),
            },
        );
        values
    };
    let count = lines.len();
    drop(_gate);
    for line in lines {
        if !send(tx, json!({"event":"line","line":line})).await {
            return Err("stream closed");
        }
    }
    Ok(("done", count))
}

pub async fn cancel(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let mut registry = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    let now = crate::metadata::cache::now();
    registry.prune(now);
    match registry.lookups.remove(&id) {
        Some(lookup) => {
            let _ = lookup.cancelled.send(true);
            // Bounded like the lookups themselves; beyond it a late confirmation gets 410.
            if registry.cancelled.len() < 1024 {
                registry
                    .cancelled
                    .insert(id, lookup.expires_at.max(now + 1));
            }
            StatusCode::NO_CONTENT.into_response()
        }
        None => expired("lookup_not_found"),
    }
}

/// A confirmation for a lookup that is gone: CANCELLED after DELETE, otherwise expired.
fn lookup_gone(cancelled: bool) -> Response {
    if cancelled {
        error(StatusCode::CONFLICT, "CANCELLED", "Lookup was cancelled")
    } else {
        expired("lookup_not_found")
    }
}

fn lookup_gone_now(state: &AppState, id: &str) -> Response {
    let cancelled = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry")
        .cancelled
        .contains_key(id);
    lookup_gone(cancelled)
}

pub async fn confirm(State(state): State<AppState>, Json(input): Json<Value>) -> Response {
    let (Some(lookup_id), Some(source_id), Some(candidate_id)) = (
        input["lookup_id"].as_str(),
        input["source_id"].as_str(),
        input["candidate_id"].as_str(),
    ) else {
        return error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Missing confirmation identity",
        );
    };
    let (metadata, candidate, mut cancellation, mapping_generation, parser_generation) = {
        let mut reg = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        reg.prune(crate::metadata::cache::now());
        let Some(lookup) = reg.lookups.get(lookup_id) else {
            return lookup_gone(reg.cancelled.contains_key(lookup_id));
        };
        let Some(candidate) = lookup.candidates.get(candidate_id) else {
            return error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Candidate does not belong to lookup",
            );
        };
        if candidate.source["id"] != source_id {
            return error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Candidate source mismatch",
            );
        }
        (
            lookup.metadata.clone(),
            candidate.clone(),
            lookup.cancelled.subscribe(),
            reg.mapping_generation,
            reg.parser_generation,
        )
    };
    if !source_current(&state, &candidate.source) || !metadata_current(&state, &metadata) {
        return expired("source_changed");
    }
    let detail = match tokio::select! {
        _=cancellation.changed()=>return lookup_gone_now(&state, lookup_id),
        result=crate::adapters::detail(&candidate.source, &candidate.content_id)=>result,
    } {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
    };
    if matching::evidence(&metadata, &detail) != matching::evidence(&metadata, &candidate.detail) {
        return expired("content_changed");
    }
    let lines = matching::lines_for(&metadata, &detail);
    if lines.is_empty() {
        return expired("target_missing");
    }
    let lines = match tokio::select! {
        _=cancellation.changed()=>return lookup_gone_now(&state, lookup_id),
        result=crate::adapters::resolve(&candidate.source, &lines)=>result,
    } {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
    };
    let lines = match tokio::select! {
        _=cancellation.changed()=>return lookup_gone_now(&state, lookup_id),
        result=crate::adapters::json_parse::resolve(&state.parsers, &candidate.source, &lines)=>result,
    } {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
    };
    let _gate = state.metadata_gate.lock().await;
    let result = {
        let mut reg = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        reg.prune(crate::metadata::cache::now());
        if reg.parser_generation != parser_generation {
            return expired("parser_changed");
        }
        if reg.mapping_generation != mapping_generation {
            return expired("mapping_changed");
        }
        let Some(lookup) = reg.lookups.get(lookup_id) else {
            return lookup_gone(reg.cancelled.contains_key(lookup_id));
        };
        if lookup.candidates.get(candidate_id).is_none_or(|item| {
            item.source["id"] != source_id || item.content_id != candidate.content_id
        }) {
            return error(
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                "Candidate no longer belongs to lookup",
            );
        }
        if !source_current(&state, &candidate.source) || !metadata_current(&state, &metadata) {
            return expired("source_changed");
        }
        let template = Template {
            source: candidate.source.clone(),
            content_id: candidate.content_id.clone(),
            lines,
        };
        let target = target(&metadata);
        let output = match line_cache::issue(
            &mut reg,
            &state.lookup_runtime,
            &target,
            &template,
            &state.reports,
        ) {
            Ok(lines) => lines,
            Err(message) => return error(StatusCode::TOO_MANY_REQUESTS, "RESOURCE_LIMIT", message),
        };
        let evidence = matching::evidence(&metadata, &detail);
        if let Err(problem) = state.mappings.save_checked(
            &metadata,
            &candidate.source,
            &candidate.content_id,
            &evidence,
            "user",
        ) {
            for line in &output {
                if let Some(id) = line["line_id"].as_str() {
                    reg.lines.remove(id);
                }
            }
            return match problem {
                "source changed" | "metadata source changed" => expired("source_changed"),
                _ => error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Mapping could not be saved",
                ),
            };
        }
        reg.cache.insert(
            line_cache::cache_key(&target, &candidate.source, parser_generation),
            crate::state::CacheEntry {
                expires_at: crate::metadata::cache::now() + state.lookup_runtime.cache_ttl,
                template,
            },
        );
        let lease = reg.lookups.get_mut(lookup_id).map(|lookup| {
            lookup.expires_at = crate::metadata::cache::now() + state.lookup_runtime.handle_ttl;
            lookup.expires_at
        });
        (output, lease)
    };
    let mut body = String::new();
    for line in result.0 {
        body.push_str(&json!({"event":"line","line":line}).to_string());
        body.push('\n');
    }
    body.push_str(&json!({"event":"source","source_id":source_id,"status":"done"}).to_string());
    body.push('\n');
    body.push_str(&json!({"event":"done","lease_expires_at":result.1}).to_string());
    body.push('\n');
    (
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

pub async fn refresh(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let (line, parser_generation) = {
        let mut reg = state
            .lookup_runtime
            .registry
            .lock()
            .expect("lookup registry");
        reg.prune(crate::metadata::cache::now());
        match reg.lines.get(&id) {
            Some(line) => (line.clone(), reg.parser_generation),
            None => return expired("line_not_found"),
        }
    };
    let Some(locator) = line.locator.as_ref() else {
        return retire(&state, &id, "line_identity_ambiguous");
    };
    let source = match state.playback_sources.get(&line.source_id) {
        Ok(Some(value))
            if value["enabled"] == true && value["revision"] == line.source_revision =>
        {
            value
        }
        _ => return retire(&state, &id, "source_changed"),
    };
    if !metadata_current(&state, &line.target) {
        return retire(&state, &id, "metadata_changed");
    }
    let resolved = if source["kind"] == "stremio_stream" {
        let manifest = match crate::adapters::stremio_stream::manifest(&source, &state.cache).await
        {
            Ok(value) => value,
            Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
        };
        match crate::adapters::stremio_stream::streams(
            &source,
            &manifest,
            line.target["type"].as_str().unwrap_or(""),
            &line.content_id,
        )
        .await
        {
            Ok(value) => value,
            Err("Stremio capability changed") => return retire(&state, &id, "source_changed"),
            Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
        }
    } else {
        let detail = match crate::adapters::detail(&source, &line.content_id).await {
            Ok(value) => value,
            Err("MacCMS content identity is missing or ambiguous") => {
                return retire(&state, &id, "line_not_found");
            }
            Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
        };
        let matches: Vec<_> = detail["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|group| group["flag"] == locator["flag"])
            .flat_map(|group| group["episodes"].as_array().into_iter().flatten())
            .filter(|episode| episode["label"] == locator["episode"])
            .collect();
        if matches.is_empty() {
            return retire(&state, &id, "line_not_found");
        }
        let base_locator = json!({"flag":locator["flag"],"episode":locator["episode"]});
        if matches.len() != 1 || matches[0]["locator"] != base_locator {
            return retire(&state, &id, "line_identity_ambiguous");
        }
        let requested = vec![json!({"flag":locator["flag"],"episode":matches[0]})];
        match crate::adapters::resolve(&source, &requested).await {
            Ok(value) => value,
            Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
        }
    };
    let resolved =
        match crate::adapters::json_parse::resolve(&state.parsers, &source, &resolved).await {
            Ok(value) => value,
            Err(message) => return error(StatusCode::BAD_GATEWAY, "UPSTREAM_FAILED", message),
        };
    let matching: Vec<_> = resolved
        .iter()
        .filter(|item| {
            if source["kind"] == "stremio_stream" {
                item["episode"]["stable_key"] == locator["stream_key"]
            } else {
                item["episode"]["locator"] == *locator
            }
        })
        .collect();
    if matching.len() != 1 {
        return retire(
            &state,
            &id,
            if matching.is_empty() {
                "line_not_found"
            } else {
                "line_identity_ambiguous"
            },
        );
    }
    let episode = &matching[0]["episode"];
    let Some(url) = episode["url"].as_str() else {
        return retire(&state, &id, "line_not_found");
    };
    let _gate = state.metadata_gate.lock().await;
    let mut reg = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    let now = crate::metadata::cache::now();
    reg.prune(now);
    if reg.parser_generation != parser_generation {
        return expired("parser_changed");
    }
    if !source_current(&state, &source) || !metadata_current(&state, &line.target) {
        reg.lines.remove(&id);
        return expired("source_changed");
    }
    reg.changed(&line.source_id);
    let Some(current) = reg.lines.get_mut(&id) else {
        return expired("line_not_found");
    };
    if current.logical_key != line.logical_key {
        reg.lines.remove(&id);
        return expired("line_identity_ambiguous");
    }
    current.url = url.to_owned();
    current.headers = episode
        .get("headers")
        .cloned()
        .unwrap_or_else(|| source["headers"].clone());
    current.quality = episode["quality"].as_str().map(str::to_owned);
    current.needs_sniffing = episode["requires_parse"] == true;
    current.kind_hint = episode["kind_hint"].as_str().map(str::to_owned);
    current.unsupported_reason = episode["unsupported_reason"].as_str().map(str::to_owned);
    current.play_expires_at = now + state.lookup_runtime.cache_ttl;
    Json(line_cache::output(&id, current, &state.reports)).into_response()
}

pub async fn report(State(state): State<AppState>, Json(input): Json<Value>) -> Response {
    let (Some(id), Some(outcome)) = (input["line_id"].as_str(), input["outcome"].as_str()) else {
        return error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Missing line report",
        );
    };
    if !matches!(outcome, "playing" | "failed")
        || input
            .get("error_code")
            .is_some_and(|v| !v.is_null() && v.as_str().is_none_or(|s| s.len() > 128))
    {
        return error(
            StatusCode::BAD_REQUEST,
            "BAD_REQUEST",
            "Invalid line report",
        );
    }
    let latency = match input.get("latency_ms") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_u64().filter(|v| *v <= 86_400_000) {
            Some(v) => Some(v),
            None => return error(StatusCode::BAD_REQUEST, "BAD_REQUEST", "Invalid latency"),
        },
    };
    let _gate = state.metadata_gate.lock().await;
    let mut reg = state
        .lookup_runtime
        .registry
        .lock()
        .expect("lookup registry");
    reg.prune(crate::metadata::cache::now());
    let Some(line) = reg.lines.get(id) else {
        return expired("line_not_found");
    };
    let current = state.playback_sources.get(&line.source_id).ok().flatten();
    if current
        .as_ref()
        .is_none_or(|s| s["enabled"] != true || s["revision"] != line.source_revision)
        || !metadata_current(&state, &line.target)
    {
        reg.lines.remove(id);
        return expired("source_changed");
    }
    if state.reports.record(line, outcome, latency).is_err() {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Report could not be saved",
        );
    }
    Json(json!({"ok":true})).into_response()
}
