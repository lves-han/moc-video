//! Stremio catalog/meta only. Playback fields never become native media records.
use super::maccms::{Http, Image, Response, http_url};
use crate::{
    contracts::{ApiError, Contracts, wire_size},
    requests::Job,
    source_dispatch::{SourceSnapshot, identity},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::header::HeaderMap;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;
use uuid::Uuid;

const LIMIT: usize = 8 * 1024 * 1024;
const CACHE_BYTES: usize = 16 * 1024 * 1024;

fn upstream(message: &str) -> ApiError {
    ApiError::new("UPSTREAM_FAILED", message)
}
fn limited(message: &str) -> ApiError {
    ApiError::new("RESOURCE_LIMIT", message)
}
fn text(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| !s.trim().is_empty())
}
fn plain(value: &str) -> String {
    let document = dom_query::Document::fragment(value);
    document.select("script, style").remove();
    document.formatted_text().trim().to_owned()
}

fn fetch(http: &Http, url: Url, job: &Job, config: bool) -> Result<Response, ApiError> {
    if job.budget.max_concurrency == 1 {
        return http.get(url, HeaderMap::new(), job, LIMIT, false, config);
    }
    if job.aggregate_remaining_ms()? == 0 {
        return Err(ApiError::new(
            "UPSTREAM_TIMEOUT",
            "Stremio aggregate reached its cleanup reserve",
        ));
    }
    let child = job.child(&Uuid::new_v4().to_string(), "default_metadata")?;
    // A child inherits the original deadline and cancellation. Checking the
    // parent's cleanup reserve stops all in-flight children, including DNS/body
    // I/O, while leaving the parent able to serialize successful columns.
    let result = std::thread::scope(|scope| {
        let worker =
            scope.spawn(move || http.get(url, HeaderMap::new(), &child, LIMIT, false, config));
        while !worker.is_finished() {
            if job.aggregate_remaining_ms().is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        worker
            .join()
            .unwrap_or_else(|_| Err(ApiError::internal("Stremio HTTP worker failed")))
    });
    job.check()?;
    match result {
        Err(error) if error.code == "CANCELLED" && job.aggregate_remaining_ms()? == 0 => {
            Err(ApiError::new(
                "UPSTREAM_TIMEOUT",
                "Stremio aggregate reached its cleanup reserve",
            ))
        }
        result => result,
    }
}
pub fn locale(value: &str) -> &'static str {
    if value
        .split(['-', '_'])
        .next()
        .is_some_and(|s| s.eq_ignore_ascii_case("zh"))
    {
        "zh-CN"
    } else {
        "en"
    }
}

// Only the independently exercised TMDB path convention is translated. Other
// addons retain their configured English endpoint and report Chinese fallback.
fn tmdb_host(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("tmdb.elfhosted.com" | "94c8cb9f702d-tmdb-addon.baby-beamup.club")
    ) || cfg!(test)
        && url.host_str() == Some("maccms.fixture.test")
        && url.path().starts_with("/tmdb/")
}

fn language_url(url: &Url, language: &str) -> Result<Url, ApiError> {
    let segments: Vec<_> = url
        .path_segments()
        .ok_or_else(|| ApiError::bad("Invalid manifest path"))?
        .collect();
    if !tmdb_host(url) {
        return Ok(url.clone());
    }
    let language = if locale(language) == "en" {
        "en-US"
    } else {
        "zh-CN"
    };
    let mut segments: Vec<String> = segments.into_iter().map(str::to_owned).collect();
    // The test-only /tmdb/ prefix uses exactly the upstream's language segment.
    let prefix = usize::from(cfg!(test) && segments.first().is_some_and(|s| s == "tmdb"));
    if segments.len() == prefix + 1 {
        segments.insert(prefix, language.into());
    } else if segments.len() == prefix + 2 && matches!(segments[prefix].as_str(), "en-US" | "zh-CN")
    {
        segments[prefix] = language.into();
    } else {
        return Err(ApiError::bad(
            "TMDB configuration encoding has no verified language mapping",
        ));
    }
    let mut result = url.clone();
    result.set_path(&format!("/{}", segments.join("/")));
    Ok(result)
}

pub fn resource_url(
    manifest: &Url,
    resource: &str,
    kind: &str,
    id: &str,
    extra: Option<&str>,
) -> Result<Url, ApiError> {
    if resource.is_empty()
        || kind.is_empty()
        || id.is_empty()
        || [resource, kind].iter().any(|s| matches!(*s, "." | ".."))
        || extra.is_some() && matches!(id, "." | "..")
    {
        return Err(ApiError::bad(
            "Stremio resource contains an invalid path segment",
        ));
    }
    let mut url = manifest.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| ApiError::bad("Invalid manifest URL"))?;
        path.pop();
        path.push(resource).push(kind);
        if let Some(extra) = extra {
            path.push(id).push(&format!("{extra}.json"));
        } else {
            path.push(&format!("{id}.json"));
        }
    }
    Ok(url)
}

#[derive(Clone)]
struct Addon {
    url: Url,
    key: String,
    manifest: Value,
    localized: bool,
}

impl Addon {
    fn supports(&self, name: &str, kind: &str, id: &str) -> bool {
        self.manifest["resources"]
            .as_array()
            .expect("Validated resources")
            .iter()
            .any(|resource| {
                let filters = if resource.as_str() == Some(name) {
                    &self.manifest
                } else if resource["name"] == name {
                    resource
                } else {
                    return false;
                };
                filters["types"]
                    .as_array()
                    .is_some_and(|types| types.iter().any(|t| t == kind))
                    && (name == "catalog"
                        || filters.get("idPrefixes").is_none_or(|prefixes| {
                            prefixes.as_array().is_some_and(|prefixes| {
                                prefixes
                                    .iter()
                                    .any(|p| p.as_str().is_some_and(|p| id.starts_with(p)))
                            })
                        }))
            })
    }
}

#[derive(Clone)]
struct Section {
    addon: usize,
    kind: String,
    catalog_id: String,
    title: String,
    skip: bool,
    id: String,
    handle: String,
    index: usize,
}

#[derive(Clone)]
struct Cursor {
    offset: usize,
    seen: HashSet<String>,
}

#[derive(Clone, Copy)]
struct Resource<'a> {
    addon: usize,
    operation: &'a str,
    kind: &'a str,
    id: &'a str,
    extra: Option<&'a str>,
}

#[derive(Clone, Default)]
struct State {
    cache: HashMap<String, Value>,
    cache_order: VecDeque<String>,
    cache_bytes: usize,
    pages: HashMap<(String, String, u64), Cursor>,
    pub images: HashMap<String, Image>,
    pub diagnostics: Vec<Value>,
}

#[derive(Clone)]
pub struct Catalog {
    pub captured: SourceSnapshot,
    pub source: Value,
    addons: Vec<Addon>,
    sections: Vec<Section>,
    state: Arc<Mutex<State>>,
}

pub struct SyncFailure {
    pub error: ApiError,
    pub indices: Vec<usize>,
}

#[derive(Default)]
pub struct Output {
    pub value: Value,
    cache: Vec<(String, Value)>,
    pages: Vec<((String, String, u64), Cursor)>,
    reset: Vec<(String, String)>,
    images: Vec<Image>,
    pub diagnostics: Vec<Value>,
}

impl Catalog {
    pub(crate) fn image_record(&self, id: &str) -> Option<Image> {
        self.state
            .lock()
            .expect("Stremio state")
            .images
            .get(id)
            .cloned()
    }
    /// Only already-fetched localized names for this exact addon/content are
    /// eligible aliases. Matching itself always loads the English base.
    pub fn match_aliases(&self, reference: &Value) -> Vec<String> {
        let Ok(fields) =
            serde_json::from_str::<Vec<String>>(reference["data"].as_str().unwrap_or(""))
        else {
            return vec![];
        };
        if fields.len() != 4 {
            return vec![];
        }
        let Some(owner) = self.addons.iter().position(|a| a.key == fields[1]) else {
            return vec![];
        };
        let key = identity(
            "stremio_display",
            &[
                &self.captured.source_id,
                &self.captured.source_revision,
                &owner.to_string(),
                "meta",
                &fields[2],
                &fields[3],
                "zh-CN",
                "",
            ],
        );
        let state = self.state.lock().expect("Stremio state");
        state
            .cache
            .get(&key)
            .and_then(|value| value["payload"]["meta"]["name"].as_str())
            .map(|title| vec![title.to_owned()])
            .unwrap_or_default()
    }

    pub fn sync(mut captured: SourceSnapshot, http: &Http, job: &Job) -> Result<Self, SyncFailure> {
        let urls = captured.config["manifest_urls"]
            .as_array()
            .expect("Validated manifest URLs");
        let mut addons = Vec::new();
        let mut hashes = Vec::new();
        let mut failures = Vec::new();
        for chunk in urls.chunks(4) {
            let results = std::thread::scope(|scope| {
                chunk.iter().map(|value| scope.spawn(move || {
                    let configured = http_url(value.as_str().expect("Manifest URL"), true)?;
                    if !configured.path().ends_with("/manifest.json") || configured.fragment().is_some() {
                        return Err(ApiError::bad("Stremio requires a complete manifest.json URL"));
                    }
                    // No query convention has yet been validated for the selected
                    // addon. Reject explicitly rather than silently dropping it.
                    if configured.query().is_some() {
                        return Err(ApiError::bad("Query-configured Stremio manifests require a verified request convention"));
                    }
                    let url = language_url(&configured, "en")?;
                    let response = fetch(http, url.clone(), job, true)?;
                    let manifest: Value = serde_json::from_slice(&response.body).map_err(|_| upstream("Stremio manifest is not JSON"))?;
                    Contracts::get().validate("StremioManifest", &manifest).map_err(|_| upstream("Stremio manifest fields are invalid"))?;
                    let localized = tmdb_host(&url) && manifest["id"] == "tmdb-addon";
                    Ok((Addon { key: identity("addon", &[url.as_str()]), url, manifest, localized }, format!("{:x}", Sha256::digest(&response.body))))
                })).collect::<Vec<_>>().into_iter().map(|worker| worker.join().unwrap_or_else(|_| Err(ApiError::internal("Stremio manifest worker failed")))).collect::<Vec<_>>()
            });
            for result in results {
                let index = addons.len() + failures.len();
                match result {
                    Ok((addon, hash)) => {
                        addons.push(addon);
                        hashes.push(hash);
                    }
                    Err(error) => failures.push((index, error)),
                }
            }
        }
        if !failures.is_empty() {
            let error = failures
                .iter()
                .find(|(_, e)| e.code == "CANCELLED")
                .or_else(|| failures.iter().find(|(_, e)| e.code == "UPSTREAM_TIMEOUT"))
                .unwrap_or(&failures[0])
                .1
                .clone();
            return Err(SyncFailure {
                error,
                indices: failures.into_iter().map(|(i, _)| i).collect(),
            });
        }
        job.check().map_err(|error| SyncFailure {
            error,
            indices: (0..urls.len()).collect(),
        })?;
        let mut fields = vec!["1".to_owned()];
        for (configured, hash) in urls.iter().zip(hashes) {
            fields.push(configured.as_str().expect("URL").into());
            fields.push(hash);
        }
        captured.source_revision = identity(
            "stremio_revision",
            &fields.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let mut sections = Vec::new();
        let mut types = Vec::new();
        let mut details = false;
        let mut addon_keys = HashSet::new();
        for (addon_index, addon) in addons.iter().enumerate() {
            if !addon_keys.insert(addon.key.clone()) {
                return Err(SyncFailure {
                    error: upstream(
                        "Stremio manifests have duplicate canonical English identities",
                    ),
                    indices: vec![addon_index],
                });
            }
            for resource in addon.manifest["resources"].as_array().expect("Resources") {
                let filter = if resource == "meta" {
                    &addon.manifest
                } else if resource["name"] == "meta" {
                    resource
                } else {
                    continue;
                };
                if filter
                    .get("idPrefixes")
                    .is_some_and(|p| p.as_array().is_none_or(Vec::is_empty))
                {
                    continue;
                }
                for kind in filter["types"]
                    .as_array()
                    .expect("Validated resource types")
                {
                    if !kind.as_str().expect("Type").is_empty() {
                        details = true;
                        if !types.contains(kind) {
                            types.push(kind.clone());
                        }
                    }
                }
            }
            let mut unique = HashSet::new();
            for catalog in addon.manifest["catalogs"]
                .as_array()
                .expect("Manifest catalogs")
            {
                let kind = catalog["type"].as_str().expect("Catalog type");
                let id = catalog["id"].as_str().expect("Catalog id");
                if !unique.insert((kind, id)) {
                    return Err(SyncFailure {
                        error: upstream("Stremio manifest contains duplicate catalog identities"),
                        indices: vec![addon_index],
                    });
                }
                if kind.is_empty()
                    || id.is_empty()
                    || !addon.supports("catalog", kind, id)
                    || catalog["extra"]
                        .as_array()
                        .is_some_and(|extras| extras.iter().any(|e| e["isRequired"] == true))
                {
                    continue;
                }
                if sections.len() >= 4096 {
                    return Err(SyncFailure {
                        error: limited("Stremio catalog limit reached"),
                        indices: vec![addon_index],
                    });
                }
                let index = sections.len();
                if !types.contains(&catalog["type"]) {
                    types.push(catalog["type"].clone());
                }
                let section_id = identity(
                    "stremio_section",
                    &[
                        &captured.source_id,
                        &captured.source_revision,
                        addon.url.as_str(),
                        kind,
                        id,
                    ],
                );
                sections.push(Section {
                    addon: addon_index,
                    kind: kind.into(),
                    catalog_id: id.into(),
                    title: text(&catalog["name"]).unwrap_or(id).into(),
                    skip: catalog["extra"]
                        .as_array()
                        .is_some_and(|extras| extras.iter().any(|e| e["name"] == "skip")),
                    handle: identity("stremio_section_ref", &[&captured.host_epoch, &section_id]),
                    id: section_id,
                    index,
                });
            }
        }
        let usable = !sections.is_empty() || details;
        let source = json!({"id": captured.source_id, "source_revision": captured.source_revision, "kind": "stremio",
            "name": captured.config["name"], "language": "", "content_types": types,
            "capabilities": {"home": !sections.is_empty(), "details": details, "search": false, "resolve": false, "sequential_home": false},
            "runtime_status": if usable {"ready"} else {"failed"}, "error": if usable {Value::Null} else {json!(ApiError::new("CAPABILITY_UNAVAILABLE", "Stremio configuration has no catalog/meta metadata capability"))},
            "origin": {"config_id": captured.config["config_id"], "manifest_urls": captured.config["manifest_urls"]}});
        Contracts::get()
            .validate("AppSource", &source)
            .map_err(|error| SyncFailure {
                error,
                indices: (0..addons.len()).collect(),
            })?;
        Ok(Self {
            captured,
            source,
            addons,
            sections,
            state: Arc::default(),
        })
    }

    pub fn commit(&self, output: &Output) -> Result<(), ApiError> {
        // Stage the bounded state transaction as well: a limit failure must not
        // leave cache entries or cursors from an unpublished response behind.
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApiError::internal("Stremio cache lock poisoned"))?;
        let mut next = state.clone();
        for (handle, language) in &output.reset {
            next.pages
                .retain(|(h, l, _), _| h != handle || l != language);
        }
        for (key, cursor) in &output.pages {
            next.pages.insert(key.clone(), cursor.clone());
        }
        if next.pages.len() > 4096
            || next.pages.values().map(|c| c.seen.len()).sum::<usize>() > 262144
        {
            return Err(limited("Stremio pagination record limit reached"));
        }
        for (key, value) in &output.cache {
            let size = value.to_string().len();
            if size > CACHE_BYTES {
                return Err(limited("Stremio cache item exceeds its byte budget"));
            }
            if let Some(old) = next.cache.remove(key) {
                next.cache_bytes -= old.to_string().len();
                next.cache_order.retain(|k| k != key);
            }
            while next.cache.len() >= 128 || next.cache_bytes + size > CACHE_BYTES {
                let oldest = next
                    .cache_order
                    .pop_front()
                    .expect("Nonempty bounded cache");
                if let Some(old) = next.cache.remove(&oldest) {
                    next.cache_bytes -= old.to_string().len();
                }
            }
            next.cache.insert(key.clone(), value.clone());
            next.cache_order.push_back(key.clone());
            next.cache_bytes += size;
        }
        for image in &output.images {
            let id = image.reference["resource_id"].as_str().expect("Image id");
            if !next.images.contains_key(id) && next.images.len() >= 4096 {
                let oldest = next
                    .images
                    .iter()
                    .min_by_key(|(_, image)| image.reference["expires_at"].as_str().unwrap_or(""))
                    .map(|(id, _)| id.clone())
                    .expect("Full image table");
                next.images.remove(&oldest);
            }
            next.images.insert(
                image.reference["resource_id"]
                    .as_str()
                    .expect("Image id")
                    .into(),
                image.clone(),
            );
        }
        if next.images.len() > 4096
            || next.images.values().map(|i| i.url.len()).sum::<usize>() > CACHE_BYTES
        {
            return Err(limited("Stremio image registration limit reached"));
        }
        next.diagnostics = output.diagnostics.iter().take(64).cloned().collect();
        *state = next;
        Ok(())
    }

    fn raw(
        &self,
        http: &Http,
        request: Resource<'_>,
        language: &str,
        job: &Job,
        output: &mut Output,
    ) -> Result<Value, ApiError> {
        let Resource {
            addon: addon_index,
            operation: resource,
            kind,
            id,
            extra,
        } = request;
        job.check()?;
        let key = identity(
            "stremio_cache",
            &[
                &self.captured.source_id,
                &self.captured.source_revision,
                &addon_index.to_string(),
                resource,
                kind,
                id,
                language,
                extra.unwrap_or(""),
            ],
        );
        if let Some((_, value)) = output.cache.iter().find(|(k, _)| *k == key) {
            return Ok(value.clone());
        }
        if let Some(value) = self.state.lock().expect("Stremio cache").cache.get(&key) {
            return Ok(value.clone());
        }
        let addon = &self.addons[addon_index];
        let url = if language == "zh-CN" {
            language_url(&addon.url, language)?
        } else {
            addon.url.clone()
        };
        let url = resource_url(&url, resource, kind, id, extra)?;
        let response = fetch(http, url, job, false)?;
        let mut value: Value = serde_json::from_slice(&response.body)
            .map_err(|_| upstream("Stremio resource is not JSON"))?;
        if resource == "catalog" {
            let metas = value["metas"]
                .as_array_mut()
                .ok_or_else(|| upstream("Stremio catalog has no metas array"))?;
            if metas.len() > 4096 {
                return Err(limited("Stremio catalog item limit reached"));
            }
            for meta in metas {
                valid_meta(meta, kind, None, language == "en")?;
                normalize_display(meta);
            }
        } else {
            if language == "en" {
                valid_meta(&value["meta"], kind, Some(id), true)?;
            } else {
                valid_meta(
                    &value["meta"],
                    text(&value["meta"]["type"]).unwrap_or(kind),
                    None,
                    false,
                )?;
            }
            normalize_display(&mut value["meta"]);
        }
        output.cache.push((key, value.clone()));
        Ok(value)
    }

    fn diagnostic(
        output: &mut Output,
        addon: usize,
        operation: &str,
        reason: &str,
        effective: &str,
    ) {
        if output.diagnostics.len() < 64 {
            output.diagnostics.push(json!({"addon_index": addon, "operation": operation, "requested_locale": "zh-CN", "effective_locale": effective, "reason": reason}));
        }
    }

    fn localized(
        &self,
        http: &Http,
        request: Resource<'_>,
        language: &str,
        job: &Job,
        output: &mut Output,
    ) -> Result<Value, ApiError> {
        let Resource {
            addon,
            operation: resource,
            kind,
            id,
            extra,
        } = request;
        job.check()?;
        let key = identity(
            "stremio_display",
            &[
                &self.captured.source_id,
                &self.captured.source_revision,
                &addon.to_string(),
                resource,
                kind,
                id,
                language,
                extra.unwrap_or(""),
            ],
        );
        let cached = self
            .state
            .lock()
            .expect("Stremio cache")
            .cache
            .get(&key)
            .cloned();
        if let Some(cached) = cached {
            output.diagnostics.extend(
                cached["diagnostics"]
                    .as_array()
                    .expect("Display diagnostics")
                    .iter()
                    .cloned(),
            );
            return Ok(cached["payload"].clone());
        }
        let diagnostic_start = output.diagnostics.len();
        let value = self.localize_resource(http, request, language, job, output)?;
        output.cache.push((
            key,
            json!({"payload": value, "requested_locale": language,
                "effective_locale": if language == "en" || output.diagnostics[diagnostic_start..].iter().any(|d| d["reason"] == "unverified_language_mapping" || d["reason"] == "localized_identity_mismatch" || d["reason"].as_str().is_some_and(|r| r.starts_with("localized_request_"))) {"en"} else {"zh-CN"},
                "diagnostics": &output.diagnostics[diagnostic_start..]}),
        ));
        Ok(value)
    }

    fn localize_resource(
        &self,
        http: &Http,
        request: Resource<'_>,
        language: &str,
        job: &Job,
        output: &mut Output,
    ) -> Result<Value, ApiError> {
        let Resource {
            addon,
            operation: resource,
            ..
        } = request;
        let base = self.raw(http, request, "en", job, output)?;
        if language == "en" {
            return Ok(base);
        }
        if !self.addons[addon].localized {
            Self::diagnostic(output, addon, resource, "unverified_language_mapping", "en");
            return Ok(base);
        }
        let local = match self.raw(http, request, language, job, output) {
            Ok(value) => value,
            Err(error) if matches!(error.code.as_str(), "CANCELLED" | "RESOURCE_LIMIT") => {
                return Err(error);
            }
            Err(error) => {
                Self::diagnostic(
                    output,
                    addon,
                    resource,
                    &format!("localized_request_{}", error.code.to_ascii_lowercase()),
                    "en",
                );
                return Ok(base);
            }
        };
        let mut result = base.clone();
        if resource == "meta" {
            if local["meta"]["id"] != base["meta"]["id"]
                || local["meta"]["type"] != base["meta"]["type"]
            {
                Self::diagnostic(output, addon, resource, "localized_identity_mismatch", "en");
                return Ok(base);
            }
            overlay(&mut result["meta"], &local["meta"], addon, resource, output);
        } else {
            let locals = local["metas"]
                .as_array()
                .expect("Validated localized catalog");
            for meta in result["metas"].as_array_mut().expect("Validated catalog") {
                let matches: Vec<_> = locals
                    .iter()
                    .filter(|l| l["id"] == meta["id"] && l["type"] == meta["type"])
                    .collect();
                if matches.len() == 1 {
                    overlay(meta, matches[0], addon, resource, output);
                } else {
                    Self::diagnostic(
                        output,
                        addon,
                        resource,
                        "localized_id_missing_or_ambiguous",
                        "en",
                    );
                }
            }
        }
        Ok(result)
    }

    pub fn home(
        &self,
        http: &Http,
        input: &Value,
        language: &str,
        job: &Job,
    ) -> Result<Output, ApiError> {
        if self.source["capabilities"]["home"] != true {
            return Err(ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "Stremio has no parameter-free catalog",
            ));
        }
        let language = locale(language);
        let page = input["page"]
            .as_u64()
            .filter(|p| (1..=10000).contains(p))
            .ok_or_else(|| ApiError::bad("Invalid Stremio page"))?;
        let handle = input["section_ref"].as_str();
        if page > 1 && handle.is_none() {
            return Err(ApiError::bad(
                "Stremio pagination requires a registered section",
            ));
        }
        let sections: Vec<_> = self
            .sections
            .iter()
            .filter(|s| handle.is_none_or(|h| s.handle == h))
            .collect();
        if sections.is_empty() {
            return Err(ApiError::bad(
                "Section reference is not registered for this source",
            ));
        }
        let mut output = Output::default();
        let mut success = Value::Array(Vec::new());
        let mut failed = Vec::new();
        let mut first_error = None;
        for chunk in sections.chunks(if handle.is_some() { 1 } else { 4 }) {
            let results = std::thread::scope(|scope| {
                chunk
                    .iter()
                    .map(|section| {
                        scope.spawn(move || {
                            crate::home::column(job, handle.is_none(), |child| {
                                self.column(http, section, page, language, child)
                            })
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|w| {
                        w.join().unwrap_or_else(|_| {
                            Err(ApiError::internal("Stremio catalog worker failed"))
                        })
                    })
                    .collect::<Vec<_>>()
            });
            for (section, result) in chunk.iter().zip(results) {
                match result {
                    Ok(mut column) => {
                        success
                            .as_array_mut()
                            .expect("Stremio sections array")
                            .push(column.value);
                        wire_size(&success, LIMIT)?;
                        output.cache.append(&mut column.cache);
                        output.pages.append(&mut column.pages);
                        output.reset.append(&mut column.reset);
                        output.images.append(&mut column.images);
                        output.diagnostics.append(&mut column.diagnostics);
                    }
                    Err(error) => {
                        if handle.is_some()
                            || matches!(
                                error.code.as_str(),
                                "CANCELLED" | "RESOURCE_LIMIT" | "BAD_REQUEST"
                            )
                        {
                            return Err(error);
                        }
                        first_error.get_or_insert_with(|| error.clone());
                        failed.push(crate::home::failure(
                            &section.title,
                            &section.handle,
                            section.index,
                            &error,
                        ));
                    }
                }
            }
        }
        job.check()?;
        let success = match success {
            Value::Array(sections) => sections,
            _ => unreachable!("Stremio sections array"),
        };
        let page = crate::home::page(input, success, failed, first_error)?;
        output.value = json!({"page": page, "images": {}});
        output.diagnostics.truncate(64);
        wire_size(&output.value, LIMIT)?;
        Contracts::get().validate("HomeViewData", &output.value)?;
        Ok(output)
    }

    fn column(
        &self,
        http: &Http,
        section: &Section,
        page: u64,
        language: &str,
        job: &Job,
    ) -> Result<Output, ApiError> {
        if job.budget.max_concurrency > 1 && job.aggregate_remaining_ms()? == 0 {
            return Err(ApiError::new(
                "UPSTREAM_TIMEOUT",
                "Stremio home reached its cleanup reserve",
            ));
        }
        let cursor = if page == 1 {
            Cursor {
                offset: 0,
                seen: HashSet::new(),
            }
        } else {
            self.state
                .lock()
                .expect("Stremio state")
                .pages
                .get(&(section.handle.clone(), language.into(), page))
                .cloned()
                .ok_or_else(|| ApiError::bad("Stremio page has no registered offset"))?
        };
        let extra = (page > 1).then(|| format!("skip={}", cursor.offset));
        let mut output = Output::default();
        let response = self.localized(
            http,
            Resource {
                addon: section.addon,
                operation: "catalog",
                kind: &section.kind,
                id: &section.catalog_id,
                extra: extra.as_deref(),
            },
            language,
            job,
            &mut output,
        )?;
        let metas = response["metas"].as_array().expect("Validated metas");
        let mut seen = cursor.seen;
        let mut items = Vec::new();
        for meta in metas {
            let card = self.card(section.addon, section.addon, meta, &mut output)?;
            if seen.insert(card["id"].as_str().expect("Content id").into()) {
                items.push(card);
            }
        }
        if seen.len() > 65536 {
            return Err(limited("Stremio pagination seen-ID limit reached"));
        }
        let has_next = section.skip && !metas.is_empty() && !items.is_empty() && page < 10000;
        if page == 1 {
            output.reset.push((section.handle.clone(), language.into()));
        }
        if has_next {
            output.pages.push((
                (section.handle.clone(), language.into(), page + 1),
                Cursor {
                    offset: cursor.offset + metas.len(),
                    seen,
                },
            ));
        }
        output.value = json!({"id": section.id, "title": section.title, "section_ref": section.handle, "index": section.index,
            "items": items, "has_next": has_next, "next_page": if has_next {Some(page+1)} else {None}, "page": page});
        Ok(output)
    }

    fn media_ref(&self, data: String) -> Value {
        json!({"source_id": self.captured.source_id, "source_revision": self.captured.source_revision, "data": data})
    }

    fn metadata_identity(&self, addon: usize, meta: &Value) -> Value {
        let addon = &self.addons[addon];
        let id = meta["id"].as_str().expect("Validated meta id");
        // Global IDs are recognized only for the verified TMDB implementation.
        let (namespace, canonical) = if addon.localized
            && id
                .strip_prefix("tmdb:")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            ("tmdb".into(), id.trim_start_matches("tmdb:").to_owned())
        } else if addon.localized
            && id
                .strip_prefix("tt")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            ("imdb".into(), id.to_owned())
        } else {
            (format!("addon:{}", addon.key), id.to_owned())
        };
        json!({"config_id": self.captured.config["config_id"], "addon_key": addon.key, "namespace": namespace,
            "type": meta["type"], "meta_id": id, "canonical_id": canonical})
    }

    fn card(
        &self,
        addon: usize,
        image_addon: usize,
        meta: &Value,
        output: &mut Output,
    ) -> Result<Value, ApiError> {
        let identity_value = self.metadata_identity(addon, meta);
        let id = identity(
            "metadata",
            &[
                identity_value["namespace"].as_str().expect("Namespace"),
                meta["type"].as_str().expect("Meta type"),
                identity_value["canonical_id"].as_str().expect("ID"),
            ],
        );
        let data = json!(["stremio", self.addons[addon].key, meta["type"], meta["id"]]).to_string();
        let year = meta["releaseInfo"]
            .as_str()
            .or_else(|| meta["year"].as_str())
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| meta["year"].as_u64())
            .filter(|y| (1000..=9999).contains(y));
        let rating = meta["imdbRating"]
            .as_f64()
            .or_else(|| meta["imdbRating"].as_str()?.parse::<f64>().ok())
            .filter(|r| r.is_finite() && (0.0..=10.0).contains(r))
            .map(|r| json!({"value": r, "scale": 10}));
        let poster = self.image(image_addon, text(&meta["poster"]), output)?;
        let kind = match meta["type"].as_str().expect("Type") {
            "movie" => "movie",
            "series" => "series",
            "anime" => "anime",
            _ => "other",
        };
        Ok(
            json!({"id": id, "ref": self.media_ref(data), "title": plain(text(&meta["name"]).ok_or_else(|| upstream("Stremio title is missing"))?),
            "kind": kind, "poster": poster, "year": year, "rating": rating, "status": "unknown", "update_label": null, "provider_type": meta["type"], "episode_count": null}),
        )
    }

    fn image(
        &self,
        addon: usize,
        value: Option<&str>,
        output: &mut Output,
    ) -> Result<Value, ApiError> {
        let Some(value) = value else {
            return Ok(Value::Null);
        };
        let url = self.addons[addon]
            .url
            .join(value)
            .map_err(|_| upstream("Stremio image URL is invalid"))?;
        http_url(url.as_str(), false)?;
        // DNS/peer validation happens again at image fetch through the same Http.
        if let Some(host) = url.host() {
            let ip = match host {
                url::Host::Ipv4(ip) => Some(std::net::IpAddr::V4(ip)),
                url::Host::Ipv6(ip) => Some(std::net::IpAddr::V6(ip)),
                _ => None,
            };
            if ip.is_some_and(|ip| !super::maccms::public_ip(ip)) {
                return Err(upstream(
                    "unsafe_target: Stremio image address is not public",
                ));
            }
        }
        let id = identity(
            "http_image",
            &[
                &self.captured.host_epoch,
                &self.captured.source_id,
                &self.captured.source_revision,
                url.as_str(),
            ],
        );
        if let Some(image) = output
            .images
            .iter()
            .find(|i| i.reference["resource_id"] == id)
        {
            return Ok(image.reference.clone());
        }
        if let Some(mut image) = self
            .state
            .lock()
            .expect("Stremio state")
            .images
            .get(&id)
            .cloned()
        {
            image.reference["expires_at"] = json!(
                (OffsetDateTime::now_utc() + time::Duration::minutes(30))
                    .format(&Rfc3339)
                    .map_err(|_| ApiError::internal("Image expiry formatting failed"))?
            );
            output.images.push(image.clone());
            return Ok(image.reference);
        }
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| ApiError::internal("Image capability generation failed"))?;
        let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(30))
            .format(&Rfc3339)
            .map_err(|_| ApiError::internal("Image expiry formatting failed"))?;
        let reference = json!({"resource_id": id, "path": format!("/images/{id}"), "capability": URL_SAFE_NO_PAD.encode(bytes), "expires_at": expires});
        output.images.push(Image {
            reference: reference.clone(),
            url: url.to_string(),
            headers: HeaderMap::new(),
        });
        Ok(reference)
    }

    pub fn details(
        &self,
        http: &Http,
        reference: &Value,
        language: &str,
        job: &Job,
    ) -> Result<Output, ApiError> {
        if reference["source_id"] != self.captured.source_id {
            return Err(ApiError::bad("Stremio content belongs to another source"));
        }
        if reference["source_revision"] != self.captured.source_revision {
            return Err(ApiError::new(
                "REVISION_MISMATCH",
                "Stremio content revision changed",
            ));
        }
        let fields: Vec<String> = serde_json::from_str(
            reference["data"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid Stremio reference"))?,
        )
        .map_err(|_| ApiError::bad("Invalid Stremio reference"))?;
        if fields.len() != 4
            || fields[0] != "stremio"
            || fields[2].is_empty()
            || fields[2].len() > 128
            || fields[3].is_empty()
            || fields[3].len() > 512
        {
            return Err(ApiError::bad("Invalid Stremio content reference"));
        }
        let owner = self
            .addons
            .iter()
            .position(|a| a.key == fields[1])
            .ok_or_else(|| ApiError::bad("Stremio addon is not registered"))?;
        let canonical_data = json!(["stremio", fields[1], fields[2], fields[3]]).to_string();
        if canonical_data.as_str() != reference["data"].as_str().expect("Reference data") {
            return Err(ApiError::bad("Stremio content reference is not canonical"));
        }
        let (_, _, kind, id) = (&fields[0], &fields[1], &fields[2], &fields[3]);
        let mut order: Vec<_> = (0..self.addons.len()).collect();
        order.retain(|i| *i != owner);
        order.insert(0, owner);
        let mut warnings = Vec::new();
        let mut first_error = None;
        for index in order {
            if !self.addons[index].supports("meta", kind, id) {
                continue;
            }
            let mut output = Output::default();
            let response = match self.localized(
                http,
                Resource {
                    addon: index,
                    operation: "meta",
                    kind,
                    id,
                    extra: None,
                },
                locale(language),
                job,
                &mut output,
            ) {
                Ok(response) => response,
                Err(error) => {
                    if matches!(error.code.as_str(), "CANCELLED" | "RESOURCE_LIMIT") {
                        return Err(error);
                    }
                    first_error.get_or_insert_with(|| error.clone());
                    warnings
                        .push(json!({"addon_index": index, "operation": "meta", "error": error}));
                    continue;
                }
            };
            let meta = &response["meta"];
            // Fallback retains the originating addon identity and opaque ref.
            let mut details = match self.map_details(owner, index, meta, reference, &mut output) {
                Ok(details) => details,
                Err(error) => {
                    if matches!(error.code.as_str(), "CANCELLED" | "RESOURCE_LIMIT") {
                        return Err(error);
                    }
                    first_error.get_or_insert_with(|| error.clone());
                    warnings
                        .push(json!({"addon_index": index, "operation": "meta", "error": error}));
                    continue;
                }
            };
            for diagnostic in &output.diagnostics {
                warnings.push(json!({"addon_index": diagnostic["addon_index"], "operation": diagnostic["operation"], "error": {"code": "UPSTREAM_FAILED", "message": "Localized metadata used English field fallback", "retryable": false, "details": diagnostic}}));
            }
            warnings.sort_by_key(|w| w["addon_index"].as_u64().unwrap_or(0));
            warnings.truncate(64);
            details["warnings"] = json!(warnings);
            output.value = json!({"details": details, "images": {}, "metadata_ref": {"identity": self.metadata_identity(owner, meta), "content_ref": reference}});
            wire_size(&output.value, LIMIT)?;
            Contracts::get().validate("DetailsViewData", &output.value)?;
            crate::details::validate(reference, &output.value["details"])?;
            let state = self.state.lock().expect("Stremio state");
            crate::details::images(&output.value["details"], &mut |reference| {
                if output
                    .images
                    .iter()
                    .any(|image| image.reference == *reference)
                    || state
                        .images
                        .values()
                        .any(|image| image.reference == *reference)
                {
                    Ok(())
                } else {
                    Err(upstream("Stremio detail image is not registered"))
                }
            })?;
            drop(state);
            job.check()?;
            return Ok(output);
        }
        Err(first_error.unwrap_or_else(|| {
            ApiError::new(
                "CAPABILITY_UNAVAILABLE",
                "No Stremio metadata resource accepts this type and ID",
            )
        }))
    }

    fn map_details(
        &self,
        owner: usize,
        served: usize,
        meta: &Value,
        reference: &Value,
        output: &mut Output,
    ) -> Result<Value, ApiError> {
        let mut details = self.card(owner, served, meta, output)?;
        details["ref"] = reference.clone();
        details["plot"] = text(&meta["description"])
            .map(plain)
            .map(Value::String)
            .unwrap_or(Value::Null);
        details["backdrop"] = self.image(served, text(&meta["background"]), output)?;
        details["tags"] = json!(strings(&meta["genres"]));
        details["actors"] = self.actors(served, meta, output)?;
        details["directors"] = people(&meta["director"]);
        details["regions"] = json!(csv(&meta["country"]));
        details["languages"] = json!(csv(&meta["language"]));
        details["duration_ms"] = json!(duration_ms(&meta["runtime"]));
        // releaseInfo explicitly distinguishes running and completed series.
        // Other display strings carry no reliable completion information.
        if matches!(meta["type"].as_str(), Some("series" | "anime"))
            && let Some((start, end)) = text(&meta["releaseInfo"]).and_then(|s| s.split_once('-'))
            && valid_year(start)
        {
            if end.is_empty() {
                details["status"] = json!("ongoing");
            } else if valid_year(end) && end >= start {
                details["status"] = json!("completed");
            }
        }
        details["groups"] = self.videos(
            owner,
            served,
            meta,
            details["id"].as_str().expect("Content id"),
            output,
        )?;
        // Downloaded videos may be a partial list; they do not declare a total.
        details["episode_count"] = Value::Null;
        details["related"] = json!([]);
        Ok(details)
    }

    fn actors(&self, addon: usize, meta: &Value, output: &mut Output) -> Result<Value, ApiError> {
        let mut credits = Vec::new();
        for name in strings(&meta["cast"]) {
            // app_extras.cast is verified in the selected addon response
            // captured by N01. Match names rather than unrelated array indices.
            let matches: Vec<_> = meta["app_extras"]["cast"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| text(&item["name"]).map(plain).as_ref() == Some(&name))
                        .collect()
                })
                .unwrap_or_default();
            let person = (matches.len() == 1).then(|| matches[0]);
            let role = person.and_then(|p| text(&p["character"])).map(plain);
            let image = self.image(addon, person.and_then(|p| text(&p["photo"])), output)?;
            credits.push(json!({"name": name, "role": role, "image": image}));
        }
        Ok(json!(credits))
    }

    fn videos(
        &self,
        addon: usize,
        image_addon: usize,
        meta: &Value,
        content_id: &str,
        output: &mut Output,
    ) -> Result<Value, ApiError> {
        let mut videos = match meta.get("videos") {
            None | Some(Value::Null) => vec![],
            Some(Value::Array(videos)) => videos.clone(),
            _ => return Err(upstream("Stremio videos must be an array")),
        };
        if videos.is_empty() && meta["type"] == "movie" {
            videos.push(json!({"id": meta["id"], "title": meta["name"]}));
        }
        if videos.len() > 4096 {
            return Err(limited("Stremio video count limit reached"));
        }
        let mut seen = HashSet::new();
        let mut groups: Vec<Value> = Vec::new();
        for video in &videos {
            let id = text(&video["id"])
                .filter(|s| s.len() <= 512)
                .ok_or_else(|| upstream("Stremio video ID is invalid"))?;
            if !seen.insert(id) {
                return Err(upstream("Stremio video IDs are ambiguous"));
            }
            let season = match video.get("season") {
                None | Some(Value::Null) => None,
                Some(v) => Some(
                    v.as_u64()
                        .filter(|n| *n <= 9_007_199_254_740_991)
                        .ok_or_else(|| upstream("Stremio season is invalid"))?,
                ),
            };
            let number = match video.get("episode") {
                None | Some(Value::Null) => None,
                Some(v) => Some(
                    v.as_f64()
                        .filter(|n| n.is_finite() && *n >= 0.0)
                        .ok_or_else(|| upstream("Stremio episode number is invalid"))?,
                ),
            };
            let data = json!([
                "stremio_video",
                self.addons[addon].key,
                meta["type"],
                meta["id"],
                id
            ])
            .to_string();
            // Use contiguous season runs to retain the complete video order,
            // including specials and a season revisited later in the response.
            if groups
                .last()
                .is_none_or(|group| group["season_number"] != json!(season))
            {
                let ordinal = groups.len().to_string();
                let title = season
                    .map(|n| format!("Season {n}"))
                    .unwrap_or_else(|| "Videos".into());
                let kind = if season.is_some() { "season" } else { "other" };
                let group_id = identity(
                    "group",
                    &[
                        content_id,
                        &self.captured.source_revision,
                        &ordinal,
                        &title,
                        kind,
                        &season.map(|n| n.to_string()).unwrap_or_default(),
                        "unspecified",
                    ],
                );
                groups.push(json!({"id": group_id, "title": title, "kind": kind, "dub_status": "unspecified", "season_number": season, "language": null, "episodes": []}));
            }
            let group = groups.last_mut().expect("Created group");
            let group_id = group["id"].as_str().expect("Group ID").to_owned();
            let episodes = group["episodes"].as_array_mut().expect("Episodes");
            let index = episodes.len();
            episodes.push(json!({"id": identity("episode", &[content_id, &self.captured.source_revision, &group_id, &index.to_string(), &data]), "label": text(&video["title"]).or_else(|| text(&video["name"])).map(plain).unwrap_or_else(|| id.into()),
                "title": text(&video["title"]).or_else(|| text(&video["name"])).map(plain), "plot": text(&video["overview"]).or_else(|| text(&video["description"])).map(plain),
                "index": index, "season": season, "number": number, "ref": self.media_ref(data), "poster": self.image(image_addon, text(&video["thumbnail"]), output)?,
                "duration_ms": duration_ms(&video["runtime"]), "released_at": released_at(&video["released"])}));
        }
        Ok(json!(groups))
    }
}

fn valid_meta(
    meta: &Value,
    kind: &str,
    id: Option<&str>,
    require_name: bool,
) -> Result<(), ApiError> {
    if text(&meta["id"]).is_none_or(|id| id.len() > 512)
        || meta["type"] != kind
        || kind.is_empty()
        || kind.len() > 128
        || id.is_some_and(|id| meta["id"] != id)
        || require_name && text(&meta["name"]).is_none_or(|s| plain(s).is_empty())
    {
        return Err(upstream("Stremio metadata identity or title is invalid"));
    }
    Ok(())
}
fn normalize_display(meta: &mut Value) {
    if !meta["genres"].is_array() && meta["genre"].is_array() {
        meta["genres"] = meta["genre"].clone();
    }
    if !meta["cast"].is_array()
        && let Some(cast) = meta["app_extras"]["cast"].as_array()
    {
        let names: Vec<_> = cast
            .iter()
            .filter_map(|p| text(&p["name"]))
            .map(str::to_owned)
            .collect();
        meta["cast"] = json!(names);
    }
}
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|v| v.iter().filter_map(text).map(plain).collect())
        .unwrap_or_default()
}
fn people(value: &Value) -> Value {
    json!(
        strings(value)
            .into_iter()
            .map(|name| json!({"name": name, "role": null, "image": null}))
            .collect::<Vec<_>>()
    )
}

fn csv(value: &Value) -> Vec<String> {
    text(value)
        .map(|value| {
            value
                .split([',', '，'])
                .map(plain)
                .filter(|v| !v.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn valid_year(value: &str) -> bool {
    value.len() == 4
        && value
            .parse::<u64>()
            .is_ok_and(|year| (1000..=9999).contains(&year))
}

// The protocol uses human readable durations. Only explicit, verified units
// are converted (SDK's 120m/45 min and the selected addon's 2h28min).
fn duration_ms(value: &Value) -> Option<u64> {
    let mut rest = text(value)?.trim();
    let mut total = 0u64;
    let mut previous_unit = u64::MAX;
    while !rest.is_empty() {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let amount = rest[..digits].parse::<u64>().ok()?;
        rest = rest[digits..].trim_start();
        let (unit, tail) = if let Some(tail) = rest.strip_prefix("min") {
            (60_000, tail)
        } else if let Some(tail) = rest.strip_prefix('m') {
            (60_000, tail)
        } else {
            let tail = rest.strip_prefix('h')?;
            (3_600_000, tail)
        };
        if unit >= previous_unit {
            return None;
        }
        previous_unit = unit;
        total = total.checked_add(amount.checked_mul(unit)?)?;
        if total > 9_007_199_254_740_991 {
            return None;
        }
        rest = tail.trim_start();
    }
    Some(total)
}

fn released_at(value: &Value) -> Option<String> {
    OffsetDateTime::parse(text(value)?, &Rfc3339)
        .ok()?
        .to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
        .ok()
}

fn overlay(base: &mut Value, local: &Value, addon: usize, operation: &str, output: &mut Output) {
    for field in [
        "name",
        "description",
        "poster",
        "background",
        "genres",
        "cast",
        "director",
    ] {
        let valid = if ["genres", "cast", "director"].contains(&field) {
            local[field]
                .as_array()
                .is_some_and(|a| !a.is_empty() && a.iter().all(|v| text(v).is_some()))
        } else if ["poster", "background"].contains(&field) {
            valid_local_image(&local[field])
        } else {
            text(&local[field]).is_some_and(|s| !plain(s).is_empty())
        };
        if valid {
            base[field] = local[field].clone();
        } else if text(&base[field]).is_some()
            || base[field].as_array().is_some_and(|a| !a.is_empty())
        {
            Catalog::diagnostic(
                output,
                addon,
                operation,
                &format!("missing_field:{field}"),
                "en",
            );
        }
    }
    if local["app_extras"]["cast"].is_array() {
        base["app_extras"] = local["app_extras"].clone();
    }
    if let Some(videos) = base["videos"].as_array_mut() {
        for video in videos {
            let matching: Vec<_> = local["videos"]
                .as_array()
                .map(|v| v.iter().filter(|v| v["id"] == video["id"]).collect())
                .unwrap_or_default();
            if matching.len() == 1 {
                for field in ["title", "name", "overview", "description", "thumbnail"] {
                    if if field == "thumbnail" {
                        valid_local_image(&matching[0][field])
                    } else {
                        text(&matching[0][field]).is_some_and(|s| !plain(s).is_empty())
                    } {
                        video[field] = matching[0][field].clone();
                    } else if text(&video[field]).is_some() {
                        Catalog::diagnostic(
                            output,
                            addon,
                            operation,
                            &format!("missing_video_field:{field}"),
                            "en",
                        );
                    }
                }
            } else {
                Catalog::diagnostic(
                    output,
                    addon,
                    operation,
                    "localized_video_id_missing_or_ambiguous",
                    "en",
                );
            }
        }
    }
}

fn valid_local_image(value: &Value) -> bool {
    let Some(value) = text(value) else {
        return false;
    };
    let Ok(url) = http_url(value, false) else {
        return false;
    };
    match url.host() {
        Some(url::Host::Ipv4(ip)) => super::maccms::public_ip(std::net::IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => super::maccms::public_ip(std::net::IpAddr::V6(ip)),
        Some(_) => true,
        None => false,
    }
}

#[cfg(test)]
#[path = "../../tests/fixtures/n01.rs"]
mod tests;
