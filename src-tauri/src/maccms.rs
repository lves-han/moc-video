//! Anonymous MacCMS JSON protocol and the public-HTTP boundary.
//! Upstream URLs stay in native records; wire references contain only identities.
use crate::{
    contracts::{ApiError, Contracts, wire_size},
    requests::Job,
    source_dispatch::{SourceSnapshot, identity},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Method, header::HeaderMap};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

const RESPONSE_LIMIT: usize = 8 * 1024 * 1024;
const PROBE_LIMIT: usize = 64 * 1024;

fn upstream(message: &str) -> ApiError {
    ApiError::new("UPSTREAM_FAILED", message)
}

fn limit() -> ApiError {
    ApiError::new(
        "RESOURCE_LIMIT",
        "MacCMS metadata exceeds the response budget",
    )
}

fn unsafe_target(config: bool) -> ApiError {
    ApiError::new(
        if config {
            "BAD_REQUEST"
        } else {
            "UPSTREAM_FAILED"
        },
        "unsafe_target: public HTTP target validation failed",
    )
    .with_details(json!({"reason":"unsafe_target", "stage":"public_http"}))
}

pub fn http_url(value: &str, config: bool) -> Result<Url, ApiError> {
    let url = Url::parse(value).map_err(|_| unsafe_target(config))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(unsafe_target(config));
    }
    Ok(url)
}

// Conservative public-unicast policy, including the IANA special-purpose ranges.
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let n = u32::from(ip);
            ![
                (Ipv4Addr::new(0, 0, 0, 0), 8),
                (Ipv4Addr::new(10, 0, 0, 0), 8),
                (Ipv4Addr::new(100, 64, 0, 0), 10),
                (Ipv4Addr::new(127, 0, 0, 0), 8),
                (Ipv4Addr::new(169, 254, 0, 0), 16),
                (Ipv4Addr::new(172, 16, 0, 0), 12),
                (Ipv4Addr::new(192, 0, 0, 0), 24),
                (Ipv4Addr::new(192, 0, 2, 0), 24),
                (Ipv4Addr::new(192, 88, 99, 0), 24),
                (Ipv4Addr::new(192, 168, 0, 0), 16),
                (Ipv4Addr::new(198, 18, 0, 0), 15),
                (Ipv4Addr::new(198, 51, 100, 0), 24),
                (Ipv4Addr::new(203, 0, 113, 0), 24),
                (Ipv4Addr::new(224, 0, 0, 0), 3),
            ]
            .iter()
            .any(|(base, bits)| n >> (32 - bits) == u32::from(*base) >> (32 - bits))
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let s = ip.segments();
            // Global unicast 2000::/3, excluding protocol assignments,
            // documentation, 6to4 (embedded IPv4), and reserved blocks.
            s[0] & 0xe000 == 0x2000
                && !(s[0] == 0x2001 && s[1] <= 0x1ff)
                && !(s[0] == 0x2001 && s[1] == 0xdb8)
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] & 0xf000 == 0)
        }
    }
}

#[derive(Clone, Default)]
pub struct Http {
    #[cfg(test)]
    pub(crate) nameserver: Option<SocketAddr>,
}

pub struct Response {
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Http {
    fn allowed(&self, url: &Url, ip: IpAddr) -> bool {
        // This fixed allowlist is compiled only into Rust unit-test binaries.
        #[cfg(test)]
        if matches!(
            url.host_str(),
            Some("maccms.fixture.test" | "other.fixture.test")
        ) && ip == IpAddr::V4(Ipv4Addr::LOCALHOST)
        {
            return true;
        }
        let _ = url;
        public_ip(ip)
    }

    pub(crate) fn check_addresses(
        &self,
        url: &Url,
        addresses: &[SocketAddr],
        config: bool,
    ) -> Result<(), ApiError> {
        if addresses.is_empty() || addresses.iter().any(|a| !self.allowed(url, a.ip())) {
            return Err(unsafe_target(config));
        }
        Ok(())
    }

    pub(crate) fn check_peer(
        &self,
        url: &Url,
        addresses: &[SocketAddr],
        peer: Option<SocketAddr>,
    ) -> Result<(), ApiError> {
        if !peer.is_some_and(|peer| addresses.contains(&peer) && self.allowed(url, peer.ip())) {
            return Err(unsafe_target(false));
        }
        Ok(())
    }

    async fn addresses(&self, url: &Url) -> Result<Vec<SocketAddr>, ApiError> {
        let host = url.host_str().ok_or_else(|| unsafe_target(false))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| unsafe_target(false))?;
        #[cfg(test)]
        {
            if self.nameserver.is_none()
                && matches!(host, "maccms.fixture.test" | "other.fixture.test")
            {
                return Ok(vec![SocketAddr::from((Ipv4Addr::LOCALHOST, port))]);
            }
        }
        if let Some(host) = url.host() {
            match host {
                url::Host::Ipv4(ip) => return Ok(vec![SocketAddr::from((ip, port))]),
                url::Host::Ipv6(ip) => return Ok(vec![SocketAddr::from((ip, port))]),
                _ => {}
            }
        }
        // Async DNS is part of the same cancellable future as connect/body I/O.
        // A blocking system lookup would keep Runtime::drop waiting past the
        // request deadline. Query both families before accepting any address.
        let mut builder = hickory_resolver::TokioResolver::builder_tokio()
            .map_err(|_| upstream("Public HTTP DNS configuration failed"))?;
        #[cfg(test)]
        if let Some(server) = self.nameserver {
            use hickory_resolver::{
                config::{NameServerConfig, ResolverConfig},
                net::runtime::TokioRuntimeProvider,
            };
            let mut nameserver = NameServerConfig::udp(server.ip());
            nameserver.connections[0].port = server.port();
            builder = hickory_resolver::TokioResolver::builder_with_config(
                ResolverConfig::from_name_servers(vec![nameserver]),
                TokioRuntimeProvider::default(),
            );
        }
        builder.options_mut().ip_strategy = hickory_resolver::config::LookupIpStrategy::Ipv4AndIpv6;
        let resolver = builder
            .build()
            .map_err(|_| upstream("Public HTTP DNS initialization failed"))?;
        resolver
            .lookup_ip(format!("{}.", host.trim_end_matches('.')))
            .await
            .map(|ips| ips.iter().map(|ip| SocketAddr::new(ip, port)).collect())
            .map_err(|_| upstream("Public HTTP DNS lookup failed"))
    }

    pub fn get(
        &self,
        url: Url,
        headers: HeaderMap,
        job: &Job,
        maximum: usize,
        probe: bool,
        config: bool,
    ) -> Result<Response, ApiError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ApiError::internal("HTTP runtime initialization failed"))?;
        let timeout = Duration::from_millis(if job.budget.max_concurrency > 1 {
            job.child_timeout_ms()?.min(job.aggregate_remaining_ms()?)
        } else {
            job.child_timeout_ms()?
        });
        runtime.block_on(watched(
            job,
            timeout,
            self.fetch(url, headers, job, maximum, probe, config),
        ))
    }

    async fn fetch(
        &self,
        url: Url,
        mut headers: HeaderMap,
        job: &Job,
        maximum: usize,
        probe: bool,
        config: bool,
    ) -> Result<Response, ApiError> {
        static SLOTS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
        let _slot = SLOTS
            .get_or_init(|| tokio::sync::Semaphore::new(4))
            .acquire()
            .await
            .map_err(|_| ApiError::internal("Public metadata slots closed"))?;
        if probe {
            headers.insert("range", "bytes=0-65535".parse().expect("Static range"));
        }
        let mut response = self
            .open(url, headers, Method::GET, job.connect_timeout_ms()?, config)
            .await?;
        if !response.status().is_success() {
            return Err(upstream(
                "Public HTTP server returned an unsuccessful status",
            ));
        }
        if !probe
            && response
                .content_length()
                .is_some_and(|n| n > maximum as u64)
        {
            return Err(limit());
        }
        let url = response.url().clone();
        let response_headers = response.headers().clone();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(http_error)? {
            job.check()?;
            if probe {
                body.extend_from_slice(&chunk[..chunk.len().min(maximum - body.len())]);
                if body.len() == maximum {
                    break;
                }
            } else {
                if body.len() + chunk.len() > maximum {
                    return Err(limit());
                }
                body.extend_from_slice(&chunk);
            }
        }
        Ok(Response {
            url,
            headers: response_headers,
            body,
        })
    }

    /// Shared connection boundary for metadata and streaming resources. The
    /// caller owns cancellation/deadlines and consumes the body without buffering.
    pub(crate) async fn open(
        &self,
        mut url: Url,
        mut headers: HeaderMap,
        method: Method,
        connect_ms: u64,
        mut config: bool,
    ) -> Result<reqwest::Response, ApiError> {
        for hop in 0..=5 {
            http_url(url.as_str(), config)?;
            if url.as_str().len()
                + headers
                    .iter()
                    .map(|(k, v)| k.as_str().len() + v.len())
                    .sum::<usize>()
                > 1024 * 1024
            {
                return Err(ApiError::new(
                    "RESOURCE_LIMIT",
                    "HTTP request exceeds 1 MiB",
                ));
            }
            let addresses = self.addresses(&url).await?;
            self.check_addresses(&url, &addresses, config)?;
            // A fresh client per connection avoids reusing a peer after DNS changes.
            // URL hostname remains intact for Host, SNI and certificate validation.
            let client = Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .pool_max_idle_per_host(0)
                .connect_timeout(Duration::from_millis(connect_ms.min(10_000)))
                .resolve_to_addrs(url.host_str().expect("Checked host"), &addresses)
                .build()
                .map_err(|_| upstream("Public HTTP client initialization failed"))?;
            let request = client
                .request(method.clone(), url.clone())
                .headers(headers.clone());
            let response = request.send().await.map_err(http_error)?;
            self.check_peer(&url, &addresses, response.remote_addr())?;
            if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
                if hop == 5 {
                    return Err(upstream("HTTP redirect limit exceeded"));
                }
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| upstream("HTTP redirect is missing a valid location"))?;
                let next = url.join(location).map_err(|_| unsafe_target(false))?;
                redirect_headers(&url, &next, &mut headers)?;
                url = next;
                config = false;
                continue;
            }
            return Ok(response);
        }
        unreachable!("bounded redirect loop")
    }
}

pub(crate) fn redirect_headers(
    from: &Url,
    to: &Url,
    headers: &mut HeaderMap,
) -> Result<(), ApiError> {
    http_url(to.as_str(), false)?;
    if from.scheme() == "https" && to.scheme() != "https" {
        return Err(upstream("HTTPS redirect downgrade rejected"));
    }
    if from.origin() != to.origin() {
        // Only public representation negotiation survives a change of origin.
        let public: HeaderMap = headers
            .iter()
            .filter(|(name, _)| {
                matches!(
                    name.as_str(),
                    "accept"
                        | "accept-language"
                        | "user-agent"
                        | "range"
                        | "if-range"
                        | "if-none-match"
                        | "if-modified-since"
                        | "accept-encoding"
                )
            })
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        *headers = public;
    }
    Ok(())
}

fn http_error(error: reqwest::Error) -> ApiError {
    if error.is_timeout() {
        ApiError::new("UPSTREAM_TIMEOUT", "Public HTTP request timed out")
    } else {
        upstream("Public HTTP request failed")
    }
}

async fn watched<T>(
    job: &Job,
    timeout: Duration,
    future: impl Future<Output = Result<T, ApiError>>,
) -> Result<T, ApiError> {
    tokio::pin!(future);
    let started = Instant::now();
    loop {
        job.check()?;
        if started.elapsed() >= timeout {
            return Err(ApiError::new(
                "UPSTREAM_TIMEOUT",
                "HTTP child deadline exhausted",
            ));
        }
        tokio::select! {
            result = &mut future => { job.check()?; return result; }
            _ = tokio::time::sleep(Duration::from_millis(10)) => {}
        }
    }
}

pub fn api_url(base: &str, parameters: &[(&str, &str)]) -> Result<Url, ApiError> {
    let mut url = http_url(base, true)?;
    // Preserve existing signed/encoded query bytes. Decode only parameter names
    // to replace the protocol's own selectors; encode new values once.
    let old = url
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|pair| {
            let key = url::form_urlencoded::parse(pair.as_bytes())
                .next()
                .map(|(key, _)| key);
            !key.is_some_and(|key| ["ac", "pg", "t", "ids", "wd"].contains(&key.as_ref()))
        })
        .collect::<Vec<_>>()
        .join("&");
    url.set_query((!old.is_empty()).then_some(old.as_str()));
    url.query_pairs_mut()
        .extend_pairs(parameters.iter().copied());
    Ok(url)
}

fn number(value: &Value, minimum: u64) -> Result<u64, ApiError> {
    let parsed = if let Some(n) = value.as_u64() {
        Some(n)
    } else {
        value
            .as_str()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse().ok())
    };
    parsed
        .filter(|&n| n >= minimum && n <= 9_007_199_254_740_991)
        .ok_or_else(|| upstream("MacCMS numeric field is invalid"))
}

fn identifier(value: &Value) -> Result<String, ApiError> {
    if let Some(s) = value.as_str().filter(|s| !s.is_empty()) {
        Ok(s.into())
    } else if value.is_u64() {
        Ok(number(value, 0)?.to_string())
    } else {
        Err(upstream("MacCMS item identifier is invalid"))
    }
}

fn decode(response: &Response) -> Result<Value, ApiError> {
    let value: Value = serde_json::from_slice(&response.body)
        .map_err(|_| upstream("MacCMS response is not JSON"))?;
    if value["code"] != json!(1)
        || value.get("closed").is_some_and(|v| {
            !matches!(v, Value::Null | Value::Bool(false)) && *v != json!(0) && *v != json!("0")
        })
    {
        return Err(upstream(
            "MacCMS interface is closed or returned an error code",
        ));
    }
    Contracts::get()
        .validate("MacCmsResponse", &value)
        .map_err(|_| upstream("MacCMS response fields are invalid"))?;
    for field in ["page", "pagecount", "total", "limit"] {
        if let Some(value) = value.get(field) {
            number(value, u64::from(field == "page"))?;
        }
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub type_id: String,
    pub title: String,
    pub id: String,
    pub handle: String,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct Image {
    pub reference: Value,
    pub url: String,
    pub headers: HeaderMap,
}

#[derive(Clone)]
pub struct Catalog {
    pub captured: SourceSnapshot,
    pub source: Value,
    pub sections: Vec<Section>,
    pub images: HashMap<String, Image>,
}

pub struct Mapped {
    pub value: Value,
    pub images: Vec<Image>,
}

pub struct Home {
    pub mapped: Mapped,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone)]
pub struct MediaLink {
    pub url: String,
    pub headers: HeaderMap,
    pub kind: &'static str,
    pub mime_type: Option<String>,
    pub playable: bool,
    pub reason: Option<&'static str>,
}

impl Catalog {
    fn request(
        &self,
        http: &Http,
        parameters: &[(&str, &str)],
        job: &Job,
        config: bool,
    ) -> Result<Value, ApiError> {
        let url = api_url(
            self.captured.config["api_url"].as_str().expect("API URL"),
            parameters,
        )?;
        let response = http.get(url, HeaderMap::new(), job, RESPONSE_LIMIT, false, config)?;
        decode(&response)
    }

    pub fn sync(captured: SourceSnapshot, http: &Http, job: &Job) -> Result<Self, ApiError> {
        let source = json!({"id": captured.source_id, "source_revision": captured.source_revision,
            "name": captured.config["name"], "kind": "maccms", "language": "", "content_types": ["other"],
            "capabilities": {"home": true, "search": true, "details": true, "resolve": true, "sequential_home": false},
            "runtime_status": "ready", "error": null,
            "origin": {"config_id": captured.config["config_id"], "api_url": captured.config["api_url"]}});
        let mut catalog = Self {
            captured,
            source,
            sections: vec![],
            images: HashMap::new(),
        };
        let response = catalog.request(http, &[("ac", "list"), ("pg", "1")], job, true)?;
        catalog.sections = catalog.categories(&response)?;
        Contracts::get().validate("AppSource", &catalog.source)?;
        Ok(catalog)
    }

    fn categories(&self, response: &Value) -> Result<Vec<Section>, ApiError> {
        let classes = match response.get("class") {
            None | Some(Value::Null) => &[][..],
            Some(Value::Array(classes)) => classes,
            _ => return Err(upstream("MacCMS classifications must be an array")),
        };
        let mut seen = HashSet::new();
        let mut sections = Vec::new();
        for class in classes {
            let id = identifier(&class["type_id"])?;
            if !seen.insert(id.clone()) {
                return Err(upstream("MacCMS classifications contain ambiguous IDs"));
            }
            sections.push((
                id,
                class["type_name"]
                    .as_str()
                    .ok_or_else(|| upstream("MacCMS classification title is missing"))?
                    .to_owned(),
            ));
        }
        if sections.is_empty() {
            sections.push((String::new(), "最近更新".into()));
        }
        Ok(sections
            .into_iter()
            .enumerate()
            .map(|(index, (type_id, title))| Section {
                id: identity(
                    "maccms_section",
                    &[
                        &self.captured.source_id,
                        &self.captured.source_revision,
                        &type_id,
                    ],
                ),
                handle: identity(
                    "maccms_section_ref",
                    &[
                        &self.captured.host_epoch,
                        &self.captured.source_id,
                        &self.captured.source_revision,
                        &type_id,
                        &title,
                        &index.to_string(),
                    ],
                ),
                type_id,
                title,
                index,
            })
            .collect())
    }

    pub fn home(&self, http: &Http, input: &Value, job: &Job) -> Result<Home, ApiError> {
        let page = input["page"]
            .as_u64()
            .ok_or_else(|| ApiError::bad("Home page must be an integer"))?;
        let section_ref = input["section_ref"].as_str();
        if !(1..=10000).contains(&page) || (page != 1 && section_ref.is_none()) {
            return Err(ApiError::bad(
                "Pagination requires a registered section and valid page",
            ));
        }
        let sections = if let Some(handle) = section_ref {
            vec![
                self.sections
                    .iter()
                    .find(|s| s.handle == handle)
                    .ok_or_else(|| {
                        ApiError::bad("Section reference is not registered for this source")
                    })?
                    .clone(),
            ]
        } else {
            let response = self.request(http, &[("ac", "list"), ("pg", "1")], job, false)?;
            self.categories(&response)?
        };
        let mut success = Vec::new();
        let mut failed = Vec::new();
        let mut images = Vec::new();
        let mut first_error = None;
        for chunk in sections.chunks(if section_ref.is_some() { 1 } else { 4 }) {
            if section_ref.is_none()
                && let Some(error) = crate::home::reserve_error(job)?
            {
                first_error.get_or_insert_with(|| error.clone());
                failed.extend(chunk.iter().map(|section| {
                    crate::home::failure(&section.title, &section.handle, section.index, &error)
                }));
                continue;
            }
            let results = std::thread::scope(|scope| {
                let workers: Vec<_> = chunk.iter().map(|section| scope.spawn(move || crate::home::column(job, section_ref.is_none(), |job| {
                    let page_text = page.to_string();
                    let response = self.request(http, &[("ac", "detail"), ("t", &section.type_id), ("pg", &page_text)], job, false)?;
                    let mapped = self.cards(&response, page)?;
                    let count = number(&response["pagecount"], 0)?;
                    let has_next = !mapped.value.as_array().expect("Mapped cards").is_empty() && page < count && page < 10000;
                    Ok::<_, ApiError>((json!({"id": section.id, "title": section.title, "section_ref": section.handle,
                        "index": section.index, "items": mapped.value, "has_next": has_next,
                        "next_page": if has_next { Some(page + 1) } else { None }, "page": page}), mapped.images))
                }))).collect();
                workers
                    .into_iter()
                    .map(|worker| {
                        worker.join().unwrap_or_else(|_| {
                            Err(ApiError::internal("MacCMS column worker failed"))
                        })
                    })
                    .collect::<Vec<_>>()
            });
            for (section, result) in chunk.iter().zip(results) {
                match result {
                    Ok((value, registered)) => {
                        success.push(value);
                        images.extend(registered);
                    }
                    Err(error) => {
                        if section_ref.is_some()
                            || matches!(error.code.as_str(), "CANCELLED" | "RESOURCE_LIMIT")
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
        let page = crate::home::page(input, success, failed, first_error)?;
        let value = json!({"page": page, "images": {}});
        wire_size(&value, RESPONSE_LIMIT)?;
        Ok(Home {
            mapped: Mapped { value, images },
            sections,
        })
    }

    pub fn search(
        &self,
        http: &Http,
        keyword: &str,
        page: u64,
        job: &Job,
    ) -> Result<Mapped, ApiError> {
        if !(1..=10000).contains(&page) || keyword.is_empty() {
            return Err(ApiError::bad("Search requires a keyword and valid page"));
        }
        let response = self.request(
            http,
            &[("ac", "detail"), ("wd", keyword), ("pg", &page.to_string())],
            job,
            false,
        )?;
        self.cards(&response, page)
    }

    pub fn match_search(&self, http: &Http, keyword: &str, job: &Job) -> Result<Mapped, ApiError> {
        if keyword.is_empty() {
            return Err(ApiError::bad("Search requires a keyword"));
        }
        let response = self.request(
            http,
            &[("ac", "detail"), ("wd", keyword), ("pg", "1")],
            job,
            false,
        )?;
        self.cards_limit(&response, 1, 10)
    }

    fn cards(&self, response: &Value, page: u64) -> Result<Mapped, ApiError> {
        self.cards_limit(response, page, usize::MAX)
    }

    fn cards_limit(&self, response: &Value, page: u64, maximum: usize) -> Result<Mapped, ApiError> {
        if number(&response["page"], 1)? != page {
            return Err(upstream("MacCMS returned a different page"));
        }
        number(&response["pagecount"], 0)?;
        let mut images = Vec::new();
        let items: Result<Vec<_>, _> = response["list"]
            .as_array()
            .expect("Validated list")
            .iter()
            .take(maximum)
            .map(|item| self.card(item, &mut images))
            .collect();
        let value = json!(items?);
        wire_size(&value, RESPONSE_LIMIT)?;
        Ok(Mapped { value, images })
    }

    fn media_ref(&self, data: &str) -> Value {
        json!({"source_id": self.captured.source_id, "source_revision": self.captured.source_revision, "data": data})
    }

    fn card(&self, item: &Value, images: &mut Vec<Image>) -> Result<Value, ApiError> {
        let vod_id = identifier(&item["vod_id"])?;
        let data = serde_json::to_string(&["maccms", &vod_id]).expect("String array");
        let year = item["vod_year"]
            .as_u64()
            .or_else(|| item["vod_year"].as_str()?.parse::<u64>().ok())
            .filter(|y| (1000..=9999).contains(y));
        let rating = item["vod_score"]
            .as_f64()
            .or_else(|| item["vod_score"].as_str()?.parse::<f64>().ok())
            .filter(|r| r.is_finite() && (0.0..=10.0).contains(r))
            .map(|value| json!({"value": value, "scale": 10}));
        let status = match &item["vod_isend"] {
            Value::Number(n) if n.as_u64() == Some(1) => "completed",
            Value::String(s) if s == "1" => "completed",
            Value::Number(n) if n.as_u64() == Some(0) => "ongoing",
            Value::String(s) if s == "0" => "ongoing",
            _ => "unknown",
        };
        let poster = self.image(item["vod_pic"].as_str(), images)?;
        Ok(
            json!({"id": identity("content", &[&self.captured.source_id, &data]), "ref": self.media_ref(&data),
            "title": item["vod_name"].as_str().ok_or_else(|| upstream("MacCMS title is missing"))?, "kind": "other",
            "poster": poster, "year": year, "rating": rating, "status": status,
            "update_label": item["vod_remarks"].as_str().filter(|s| !s.is_empty()),
            "provider_type": item["type_name"].as_str(), "episode_count": null}),
        )
    }

    fn image(&self, value: Option<&str>, images: &mut Vec<Image>) -> Result<Value, ApiError> {
        let Some(value) = value.filter(|s| !s.is_empty()) else {
            return Ok(Value::Null);
        };
        let base = http_url(
            self.captured.config["api_url"].as_str().expect("API URL"),
            true,
        )?;
        let url = base
            .join(value)
            .map_err(|_| upstream("MacCMS image URL is invalid"))?;
        http_url(url.as_str(), false)?;
        let id = identity(
            "http_image",
            &[
                &self.captured.host_epoch,
                &self.captured.source_id,
                &self.captured.source_revision,
                url.as_str(),
            ],
        );
        if let Some(image) = self.images.get(&id).or_else(|| {
            images
                .iter()
                .find(|image| image.reference["resource_id"] == id)
        }) {
            let mut reference = image.reference.clone();
            reference["expires_at"] = json!(expires(1800)?);
            let renewed = Image {
                reference: reference.clone(),
                url: image.url.clone(),
                headers: image.headers.clone(),
            };
            images.push(renewed);
            return Ok(reference);
        }
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| ApiError::internal("Image capability generation failed"))?;
        let reference = json!({"resource_id": id, "path": format!("/images/{id}"),
            "capability": URL_SAFE_NO_PAD.encode(random), "expires_at": expires(1800)?});
        images.push(Image {
            reference: reference.clone(),
            url: url.to_string(),
            headers: HeaderMap::new(),
        });
        Ok(reference)
    }

    pub fn details(&self, http: &Http, reference: &Value, job: &Job) -> Result<Mapped, ApiError> {
        self.check_ref(reference)?;
        let data: Vec<String> =
            serde_json::from_str(reference["data"].as_str().expect("Checked ref"))
                .map_err(|_| ApiError::bad("Invalid MacCMS content reference"))?;
        if data.len() != 2 || data[0] != "maccms" || data[1].is_empty() {
            return Err(ApiError::bad("Invalid MacCMS content reference"));
        }
        let item = self.detail_item(http, &data[1], job)?;
        self.map_details(&item, reference)
    }

    fn check_ref(&self, reference: &Value) -> Result<(), ApiError> {
        if reference["source_id"] != self.captured.source_id {
            return Err(ApiError::bad("MacCMS reference belongs to another source"));
        }
        if reference["source_revision"] != self.captured.source_revision {
            return Err(ApiError::new(
                "REVISION_MISMATCH",
                "MacCMS reference revision changed",
            ));
        }
        if reference["data"].as_str().is_none_or(|s| s.len() > 524288) {
            return Err(ApiError::bad("Invalid MacCMS opaque reference"));
        }
        Ok(())
    }

    fn detail_item(&self, http: &Http, vod_id: &str, job: &Job) -> Result<Value, ApiError> {
        let response = self.request(http, &[("ac", "detail"), ("ids", vod_id)], job, false)?;
        let items = response["list"].as_array().expect("Validated list");
        if items.len() != 1 || identifier(&items[0]["vod_id"])? != vod_id {
            return Err(upstream("MacCMS detail is missing or ambiguous"));
        }
        Ok(items[0].clone())
    }

    fn map_details(&self, item: &Value, reference: &Value) -> Result<Mapped, ApiError> {
        let mut images = Vec::new();
        let card = self.card(item, &mut images)?;
        if card["ref"] != *reference {
            return Err(ApiError::bad("MacCMS content reference is not canonical"));
        }
        let groups = self.groups(item, card["id"].as_str().expect("Content id"))?;
        let mut details = card;
        details.as_object_mut().expect("Card object").remove("ref");
        details["ref"] = reference.clone();
        details["plot"] = item["vod_content"]
            .as_str()
            .map(plain_text)
            .map(Value::String)
            .unwrap_or(Value::Null);
        details["groups"] = json!(
            groups
                .into_iter()
                .map(|(group, _)| group)
                .collect::<Vec<_>>()
        );
        details["tags"] = json!(words(item["vod_class"].as_str()));
        details["related"] = json!([]);
        details["actors"] = json!(people(item["vod_actor"].as_str()));
        details["directors"] = json!(people(item["vod_director"].as_str()));
        details["regions"] = json!(words(item["vod_area"].as_str()));
        details["languages"] = json!(words(item["vod_lang"].as_str()));
        crate::details::validate(reference, &details)?;
        crate::details::images(&details, &mut |reference| {
            if images.iter().any(|image| image.reference == *reference) {
                Ok(())
            } else {
                Err(upstream("MacCMS detail image is not registered"))
            }
        })?;
        let value = json!({"details": details, "images": {}, "metadata_ref": null});
        wire_size(&value, RESPONSE_LIMIT)?;
        Contracts::get().validate("DetailsViewData", &value)?;
        Ok(Mapped { value, images })
    }

    fn groups(
        &self,
        item: &Value,
        content_id: &str,
    ) -> Result<Vec<(Value, Vec<String>)>, ApiError> {
        let playback_field = |name: &str| match item.get(name) {
            None | Some(Value::Null) => Ok(""),
            Some(Value::String(s)) => Ok(s.as_str()),
            _ => Err(upstream("MacCMS playback field must be a string")),
        };
        let names = playback_field("vod_play_from")?;
        let urls = playback_field("vod_play_url")?;
        if names.is_empty() && urls.is_empty() {
            return Ok(vec![]);
        }
        let names: Vec<_> = names.split("$$$").collect();
        let groups: Vec<_> = urls.split("$$$").collect();
        if names.len() != groups.len()
            || names.iter().any(|name| name.is_empty())
            || groups.iter().any(|group| group.is_empty())
        {
            return Err(upstream("MacCMS playback groups are ambiguous"));
        }
        let vod_id = identifier(&item["vod_id"])?;
        let mut result = Vec::new();
        for (group_index, (title, group)) in names.into_iter().zip(groups).enumerate() {
            let group_id = identity(
                "group",
                &[
                    content_id,
                    &self.captured.source_revision,
                    &group_index.to_string(),
                    title,
                    "route",
                    "",
                    "unspecified",
                ],
            );
            let mut episodes = Vec::new();
            let mut media = Vec::new();
            for (episode_index, entry) in group.split('#').enumerate() {
                if entry.is_empty() {
                    return Err(upstream("MacCMS playback contains a missing episode"));
                }
                let (label, url) = if entry.starts_with("http://") || entry.starts_with("https://")
                {
                    (String::new(), entry)
                } else if let Some((label, url)) = entry.split_once('$') {
                    (label.to_owned(), url)
                } else {
                    (String::new(), entry)
                };
                if url.is_empty() {
                    return Err(upstream("MacCMS episode resource is missing"));
                }
                let fingerprint = identity("maccms_entry", &[title, entry]);
                let data = serde_json::to_string(&[
                    "maccms_episode",
                    &vod_id,
                    &group_index.to_string(),
                    &episode_index.to_string(),
                    &fingerprint,
                ])
                .expect("String array");
                let reference = self.media_ref(&data);
                let id = identity(
                    "episode",
                    &[
                        content_id,
                        &self.captured.source_revision,
                        &group_id,
                        &episode_index.to_string(),
                        &data,
                    ],
                );
                let label = if label.is_empty() {
                    format!("{}", episode_index + 1)
                } else {
                    label
                };
                episodes.push(json!({"id": id, "label": label, "index": episode_index, "ref": reference, "number": null, "season": null}));
                media.push(url.into());
            }
            result.push((json!({"id": group_id, "title": title, "kind": "route", "dub_status": "unspecified", "season_number": null, "episodes": episodes}), media));
        }
        Ok(result)
    }

    pub fn resolve(
        &self,
        http: &Http,
        reference: &Value,
        job: &Job,
    ) -> Result<MediaLink, ApiError> {
        self.check_ref(reference)?;
        let data: Vec<String> =
            serde_json::from_str(reference["data"].as_str().expect("Checked ref"))
                .map_err(|_| ApiError::bad("Invalid MacCMS episode reference"))?;
        if data.len() != 5 || data[0] != "maccms_episode" {
            return Err(ApiError::bad("Invalid MacCMS episode reference"));
        }
        let group: usize = data[2]
            .parse()
            .map_err(|_| ApiError::bad("Invalid MacCMS group index"))?;
        let episode: usize = data[3]
            .parse()
            .map_err(|_| ApiError::bad("Invalid MacCMS episode index"))?;
        let item = self.detail_item(http, &data[1], job)?;
        let content = serde_json::to_string(&["maccms", &data[1]]).expect("String array");
        let groups = self.groups(
            &item,
            &identity("content", &[&self.captured.source_id, &content]),
        )?;
        let (value, media) = groups
            .get(group)
            .ok_or_else(|| upstream("MacCMS selected group is missing"))?;
        if value["episodes"]
            .get(episode)
            .is_none_or(|e| e["ref"] != *reference)
        {
            return Err(upstream("MacCMS selected episode changed or is missing"));
        }
        let original = &media[episode];
        let Ok(url) = http_url(original, false) else {
            return Ok(MediaLink {
                url: original.clone(),
                headers: HeaderMap::new(),
                kind: "unknown",
                mime_type: None,
                playable: false,
                reason: Some("unsupported_protocol_or_script"),
            });
        };
        if url.as_str().len() > 1024 * 1024 {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Media request exceeds 1 MiB",
            ));
        }
        // Validate network targets even when a suffix already identifies the media.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ApiError::internal("HTTP runtime initialization failed"))?;
        let addresses = runtime.block_on(watched(
            job,
            Duration::from_millis(job.connect_timeout_ms()?),
            http.addresses(&url),
        ))?;
        http.check_addresses(&url, &addresses, false)?;
        let suffix = url
            .path()
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let (kind, mime, reason) = match suffix.as_str() {
            "mp4" => ("progressive", Some("video/mp4".into()), None),
            "m3u8" => ("hls", Some("application/vnd.apple.mpegurl".into()), None),
            _ => {
                let response = http.get(url, HeaderMap::new(), job, PROBE_LIMIT, true, false)?;
                classify(&response)
            }
        };
        job.check()?;
        Ok(MediaLink {
            url: original.clone(),
            headers: HeaderMap::new(),
            kind,
            mime_type: mime,
            playable: reason.is_none(),
            reason,
        })
    }

    pub fn register_images(&mut self, images: Vec<Image>) -> Result<(), ApiError> {
        let new_ids: HashSet<_> = images
            .iter()
            .map(|image| image.reference["resource_id"].as_str().expect("Image id"))
            .collect();
        if new_ids.len() > 4096 {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Native image registration limit reached",
            ));
        }
        for image in images {
            let id = image.reference["resource_id"].as_str().expect("Image id");
            if !self.images.contains_key(id) && self.images.len() >= 4096 {
                let oldest = self
                    .images
                    .iter()
                    .min_by_key(|(_, image)| image.reference["expires_at"].as_str().unwrap_or(""))
                    .map(|(id, _)| id.clone())
                    .expect("Full image table");
                self.images.remove(&oldest);
            }
            self.images.insert(
                image.reference["resource_id"]
                    .as_str()
                    .expect("Image id")
                    .into(),
                image,
            );
        }
        Ok(())
    }
}

fn classify(response: &Response) -> (&'static str, Option<String>, Option<&'static str>) {
    let mime = response
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
    let text = String::from_utf8_lossy(&response.body);
    let start = text.trim_start_matches('\u{feff}').trim_start();
    if mime.as_deref() == Some("text/html")
        || start.starts_with('<') && !start.starts_with("<?xml") && !start.starts_with("<MPD")
    {
        return ("unknown", mime, Some("html_playback_page"));
    }
    if start.starts_with("#EXTM3U") {
        return ("hls", Some("application/vnd.apple.mpegurl".into()), None);
    }
    if response.body.get(4..8) == Some(b"ftyp") {
        return ("progressive", Some("video/mp4".into()), None);
    }
    if start.starts_with("<MPD") || start.starts_with("<?xml") && start.contains("<MPD") {
        if start.contains("ContentProtection") {
            return ("drm", mime, Some("drm_unsupported"));
        }
        return ("dash", Some("application/dash+xml".into()), None);
    }
    match mime.as_deref() {
        Some("video/mp4") => ("progressive", mime, None),
        Some("application/vnd.apple.mpegurl" | "application/x-mpegurl") => ("hls", mime, None),
        _ => ("unknown", mime, Some("unknown_media_type")),
    }
}

fn expires(seconds: i64) -> Result<String, ApiError> {
    (OffsetDateTime::now_utc() + time::Duration::seconds(seconds))
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("Resource lease timestamp failed"))
}

fn words(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split([',', '，', '/', '、'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(plain_text)
        .collect()
}

fn people(value: Option<&str>) -> Vec<Value> {
    words(value)
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|name| json!({"name": name, "role": null, "image": null}))
        .collect()
}

fn plain_text(value: &str) -> String {
    // Parse HTML fragments as data; scripts/styles are removed with their contents.
    let document = dom_query::Document::fragment(value);
    document.select("script, style").remove();
    document.formatted_text().trim().to_owned()
}

#[cfg(test)]
#[path = "../../tests/fixtures/n00.rs"]
mod tests;
