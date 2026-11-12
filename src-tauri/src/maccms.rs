//! Anonymous MacCMS JSON protocol and the shared HTTP boundary.
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
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
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

/// Access control is reported and stops the source; nothing retries or works around it.
fn access_control(message: &str, signal: &str, status: u16) -> ApiError {
    upstream(message).with_details(
        json!({"reason":"access_control","signal":signal,"http_status":status,"stage":"public_http"}),
    )
}

pub(crate) fn access_status(status: u16, headers: &HeaderMap) -> Option<ApiError> {
    let challenge = headers
        .get("cf-mitigated")
        .is_some_and(|value| value == "challenge");
    (challenge || matches!(status, 401 | 403 | 407 | 429 | 451)).then(|| {
        access_control(
            "Public HTTP server refused access",
            if challenge { "challenge" } else { "status" },
            status,
        )
    })
}

fn access_refused(error: &ApiError) -> bool {
    error
        .details
        .as_ref()
        .is_some_and(|details| details["reason"] == "access_control")
}

fn challenge_page(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(&body[..body.len().min(PROBE_LIMIT)]).to_lowercase();
    [
        "cf-chl",
        "challenge-platform",
        "just a moment",
        "captcha",
        "verify you are human",
    ]
    .iter()
    .any(|marker| text.contains(marker))
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

#[derive(Clone, Default)]
pub struct Http {
    #[cfg(test)]
    pub(crate) nameserver: Option<Vec<SocketAddr>>,
}

pub struct Response {
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Http {
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
        // request deadline. Keep both families, including LAN and proxy Fake-IP.
        use hickory_resolver::{
            config::{NameServerConfig, ResolveHosts, ResolverConfig},
            net::runtime::TokioRuntimeProvider,
        };
        #[allow(unused_mut)] // Test-only nameserver injection replaces this list.
        let mut servers = vec![
            NameServerConfig::udp(Ipv4Addr::new(1, 1, 1, 1).into()),
            NameServerConfig::udp(Ipv4Addr::new(8, 8, 8, 8).into()),
        ];
        #[cfg(test)]
        if let Some(addresses) = &self.nameserver {
            servers = addresses
                .iter()
                .map(|server| {
                    let mut nameserver = NameServerConfig::udp(server.ip());
                    nameserver.connections[0].port = server.port();
                    nameserver
                })
                .collect();
        }
        // hickory's pool stops at a SERVFAIL response, so one failing server would hide the
        // healthy one. Each server gets its own resolver; the first success wins and dropping
        // the set aborts the other query.
        let name = format!("{}.", host.trim_end_matches('.'));
        let mut lookups = tokio::task::JoinSet::new();
        for server in servers {
            let mut builder = hickory_resolver::TokioResolver::builder_with_config(
                ResolverConfig::from_name_servers(vec![server]),
                TokioRuntimeProvider::default(),
            );
            // Never read OS DNS, hosts, or search domains (ADR-049).
            builder.options_mut().use_hosts_file = ResolveHosts::Never;
            builder.options_mut().ip_strategy =
                hickory_resolver::config::LookupIpStrategy::Ipv4AndIpv6;
            let resolver = builder
                .build()
                .map_err(|_| upstream("Public HTTP DNS initialization failed"))?;
            let name = name.clone();
            lookups.spawn(async move {
                resolver
                    .lookup_ip(name)
                    .await
                    .map(|ips| ips.iter().collect::<Vec<_>>())
            });
        }
        while let Some(result) = lookups.join_next().await {
            if let Ok(Ok(ips)) = result {
                if ips.is_empty() {
                    continue;
                }
                return Ok(ips
                    .into_iter()
                    .map(|ip| SocketAddr::new(ip, port))
                    .collect());
            }
        }
        Err(upstream("Public HTTP DNS lookup failed"))
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
            let status = response.status().as_u16();
            return Err(
                access_status(status, response.headers()).unwrap_or_else(|| {
                    upstream(&format!("Public HTTP server returned HTTP {status}"))
                        .with_details(json!({"http_status": status,
                            "stage": if probe { "media_probe" } else { "upstream_request" }}))
                }),
            );
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
            // Bind the configured DNS result, including proxy Fake-IP/LAN addresses.
            // Keep the hostname for Host, SNI and certificate validation. IP address
            // and actual-peer restrictions were removed at the user's request.
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

fn certificate_error(error: &(dyn std::error::Error + 'static)) -> bool {
    // Schannel exposes localized OS messages. Inspect its HRESULT before text.
    let text = error.to_string();
    #[cfg(windows)]
    {
        let windows_certificate = [
            0x800B0101_u32,
            0x800B0109,
            0x800B010A,
            0x800B010C,
            0x800B010F,
            0x800B0110,
        ]
        .iter()
        .any(|code| {
            let code = *code as i32;
            error.downcast_ref::<std::io::Error>().and_then(|e| e.raw_os_error()) == Some(code)
                // native-tls 0.2 skips its inner io::Error in source(). Its
                // Display still preserves the OS message and numeric code.
                || text == std::io::Error::from_raw_os_error(code).to_string()
        });
        if windows_certificate {
            return true;
        }
    }
    text.to_lowercase().contains("certificate")
}

#[cfg(all(test, windows))]
mod windows_tls_errors {
    #[test]
    fn localized_certificate_errors_keep_their_category() {
        assert!(super::certificate_error(
            &std::io::Error::from_raw_os_error(0x800B0109_u32 as i32)
        ));
        assert!(!super::certificate_error(
            &std::io::Error::from_raw_os_error(5)
        ));
        #[cfg(windows)]
        assert!(super::certificate_error(&std::io::Error::other(
            std::io::Error::from_raw_os_error(0x800B0109_u32 as i32).to_string()
        )));
    }
}

fn http_error(error: reqwest::Error) -> ApiError {
    if error.is_timeout() {
        ApiError::new("UPSTREAM_TIMEOUT", "Public HTTP request timed out")
    } else if std::iter::successors(std::error::Error::source(&error), |source| source.source())
        .any(certificate_error)
    {
        upstream("Public HTTP certificate validation failed")
            .with_details(json!({"reason":"tls_certificate","stage":"public_http"}))
    } else {
        upstream("Public HTTP request failed")
    }
}

pub(crate) async fn watched<T>(
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
    let mut value: Value = serde_json::from_slice(&response.body).map_err(|_| {
        if challenge_page(&response.body) {
            access_control(
                "MacCMS response is a challenge page, not JSON",
                "challenge",
                200,
            )
        } else {
            upstream("MacCMS response is not JSON")
        }
    })?;
    // Some public interfaces omit code; a list is then the success signal.
    if value.get("code").is_none() && value["list"].is_array() {
        value["code"] = json!(1);
    }
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
    pub category: &'static str,
}

// Names are source-owned; IDs remain opaque and always go back to that source.
pub(crate) fn normalized_category(name: &str) -> Option<(String, &'static str)> {
    if [
        "伦理", "倫理", "福利", "擦边", "擦邊", "情色", "成人", "三级", "里番", "预告", "預告",
        "资讯", "資訊", "公告", "头条", "頭條", "解说", "解說", "花絮", "搞笑", "汽车", "写真",
        "两性", "有声", "儿歌", "音乐", "影视",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        return None;
    }
    let leaf = name
        .rsplit(['/', '／', '\\', '>'])
        .next()
        .unwrap_or(name)
        .trim();
    let (title, category) = match leaf {
        "电影" | "电影片" => ("电影", "movie"),
        "电视剧" | "连续剧" | "剧集" => ("电视剧", "series"),
        "国产剧" | "内地剧" | "大陆剧" | "华语剧" => ("华语剧", "series"),
        "香港剧" | "港澳剧" | "港剧" => ("港剧", "series"),
        "台湾剧" | "台剧" => ("台剧", "series"),
        "日本剧" | "日剧" => ("日剧", "series"),
        "韩国剧" | "韩剧" => ("韩剧", "series"),
        "泰国剧" | "泰剧" | "马泰剧" => ("泰剧", "series"),
        "美国剧" | "美剧" | "欧美剧" => ("欧美剧", "series"),
        "综艺" | "综艺片" | "综艺节目" => ("综艺", "variety"),
        "大陆综艺" | "内地综艺" | "国产综艺" | "华语综艺" => {
            ("华语综艺", "variety")
        }
        "动漫" | "动漫片" | "动画" => ("动漫", "anime"),
        "中国动漫" | "国产动漫" | "国漫" => ("国产动漫", "anime"),
        "日本动漫" | "日韩动漫" | "日漫" => ("日韩动漫", "anime"),
        "动画片" | "动漫电影" | "动画电影" => ("动画电影", "anime"),
        "短剧" | "短剧大全" | "短剧片" | "微短剧" | "精品短剧" => ("短剧", "short"),
        "体育" | "体育赛事" | "体育节目" | "体育直播" => ("体育", "sports"),
        "记录片" | "纪录片" | "纪录" => ("纪录片", "documentary"),
        other => (
            other,
            if other.contains("综艺") {
                "variety"
            } else if other.contains("动漫") || other.contains("漫剧") {
                "anime"
            } else if other.contains("短剧")
                || [
                    "古装仙侠",
                    "现代都市",
                    "穿越年代",
                    "年代穿越",
                    "言情总裁",
                    "重生民国",
                    "反转爽剧",
                    "脑洞悬疑",
                    "女频恋爱",
                    "都市脑洞",
                    "逆袭",
                ]
                .contains(&other)
            {
                "short"
            } else if [
                "足球",
                "篮球",
                "网球",
                "斯诺克",
                "台球",
                "其他赛事",
                "电竞",
                "综合体育",
            ]
            .contains(&other)
            {
                "sports"
            } else if other.ends_with('剧') {
                "series"
            } else if other.ends_with('片') {
                "movie"
            } else {
                "other"
            },
        ),
    };
    Some((title.to_owned(), category))
}

fn category_kind(category: &str) -> &str {
    if ["movie", "series", "anime"].contains(&category) {
        category
    } else {
        "other"
    }
}

fn normalized_region(value: &str) -> &str {
    match value.trim() {
        "大陆" | "内地" | "中国大陆" | "中国" => "中国大陆",
        "香港" | "中国香港" => "中国香港",
        "台湾" | "中国台湾" => "中国台湾",
        other => other,
    }
}
fn normalized_language(value: &str) -> &str {
    match value.trim() {
        "普通话" | "汉语普通话" | "国语" | "中文" => "国语",
        other => other,
    }
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
    pub search_gate: Arc<Mutex<()>>,
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
        Self::sync_cached(captured, http, job, None).map(|(catalog, _)| catalog)
    }

    pub(crate) fn sync_cached(
        captured: SourceSnapshot,
        http: &Http,
        job: &Job,
        cached: Option<&Value>,
    ) -> Result<(Self, Value), ApiError> {
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
            search_gate: Arc::default(),
        };
        let response = match cached {
            Some(value) => value.clone(),
            None => catalog.request(http, &[("ac", "list"), ("pg", "1")], job, true)?,
        };
        job.check()?;
        wire_size(&response, RESPONSE_LIMIT)?;
        catalog.sections = catalog.categories(&response)?;
        let mut categories: Vec<_> = catalog.sections.iter().map(|s| s.category).collect();
        categories.sort_unstable();
        categories.dedup();
        catalog.source["content_types"] = json!(categories);
        Contracts::get().validate("AppSource", &catalog.source)?;
        // The bootstrap only needs category names and IDs, never media URLs.
        let bootstrap = json!({"class": response.get("class").cloned().unwrap_or(Value::Null)});
        Ok((catalog, bootstrap))
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
            let name = class["type_name"]
                .as_str()
                .ok_or_else(|| upstream("MacCMS classification title is missing"))?
                .to_owned();
            if let Some((title, category)) = normalized_category(&name) {
                sections.push((id, title, category));
            }
        }
        if classes.is_empty() {
            sections.push((String::new(), "最近更新".into(), "other"));
        }
        Ok(sections
            .into_iter()
            .enumerate()
            .map(|(index, (type_id, title, category))| Section {
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
                category,
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
        let mut refused: Option<ApiError> = None;
        for chunk in sections.chunks(if section_ref.is_some() { 1 } else { 4 }) {
            // Once the source refuses access, later sections fail without another request.
            let skipped = match &refused {
                Some(error) => Some(error.clone()),
                None if section_ref.is_none() => crate::home::reserve_error(job)?,
                None => None,
            };
            if let Some(error) = skipped {
                first_error.get_or_insert_with(|| error.clone());
                failed.extend(chunk.iter().map(|section| {
                    crate::home::failure(&section.title, &section.handle, section.index, &error)
                }));
                continue;
            }
            let results = std::thread::scope(|scope| {
                let workers: Vec<_> = chunk
                    .iter()
                    .map(|section| {
                        scope.spawn(move || {
                            crate::home::column(job, section_ref.is_none(), |job| {
                                self.home_section(http, input, section, page, job)
                            })
                        })
                    })
                    .collect();
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
                        if access_refused(&error) {
                            refused.get_or_insert_with(|| error.clone());
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

    fn facets(&self, section: &Section) -> Value {
        let mut filters = crate::home::unavailable_filters();
        let mut genres = vec![section.title.clone()];
        genres.extend(
            [
                "动作", "喜剧", "爱情", "科幻", "剧情", "悬疑", "冒险", "恐怖", "历史",
            ]
            .map(str::to_owned),
        );
        genres.sort();
        genres.dedup();
        let years: Vec<_> = (1900..=OffsetDateTime::now_utc().year())
            .rev()
            .map(|y| y.to_string())
            .collect();
        for (key, options) in [
            ("genre", genres),
            ("year", years),
            (
                "region",
                [
                    "中国大陆",
                    "中国香港",
                    "中国台湾",
                    "美国",
                    "日本",
                    "韩国",
                    "英国",
                    "泰国",
                    "法国",
                ]
                .map(str::to_owned)
                .to_vec(),
            ),
            (
                "language",
                [
                    "国语", "粤语", "英语", "日语", "韩语", "法语", "德语", "泰语",
                ]
                .map(str::to_owned)
                .to_vec(),
            ),
        ] {
            filters[key] = json!({"available":true,"options":options.iter().map(|v| json!({"value":v,"name":v})).collect::<Vec<_>>()});
        }
        filters
    }

    fn home_section(
        &self,
        http: &Http,
        input: &Value,
        section: &Section,
        page: u64,
        job: &Job,
    ) -> Result<(Value, Vec<Image>), ApiError> {
        let filter = &input["filter"];
        if !filter["sort"].is_null() {
            return Err(ApiError::bad("MacCMS sorting is unavailable"));
        }
        let filtered = filter.as_object().is_some_and(|f| !f.is_empty());
        // APIs commonly ignore year/area/lang. Scan at most five source pages
        // per logical page, passing only the documented category and page.
        let batch = if filtered { 5 } else { 1 };
        let first = (page - 1) * batch + 1;
        let mut items = Vec::new();
        let mut images = Vec::new();
        let mut scanned = 0;
        let mut has_next = false;
        for upstream_page in first..first + batch {
            let response = self.request(
                http,
                &[
                    ("ac", "detail"),
                    ("t", &section.type_id),
                    ("pg", &upstream_page.to_string()),
                ],
                job,
                false,
            )?;
            if response.get("page").is_some() && number(&response["page"], 1)? != upstream_page {
                return Err(upstream("MacCMS returned a different page"));
            }
            let entries = response["list"].as_array().expect("Validated list");
            scanned += entries.len();
            for item in entries {
                if normalized_category(item["type_name"].as_str().unwrap_or("")).is_none() {
                    continue;
                }
                let card = self.card(item, &mut images)?;
                let matches = filter.as_object().is_none_or(|f| {
                    f.iter().all(|(key, selected)| {
                        let selected = selected.as_str().unwrap_or("");
                        match key.as_str() {
                            "year" => card["year"]
                                .as_u64()
                                .is_some_and(|y| y.to_string() == selected),
                            "region" => words(item["vod_area"].as_str())
                                .iter()
                                .any(|v| normalized_region(v) == normalized_region(selected)),
                            "language" => words(item["vod_lang"].as_str())
                                .iter()
                                .any(|v| normalized_language(v) == normalized_language(selected)),
                            "genre" => {
                                section.title == selected
                                    || words(item["vod_class"].as_str())
                                        .iter()
                                        .any(|v| v == selected)
                            }
                            _ => false,
                        }
                    })
                });
                if matches && !items.iter().any(|old: &Value| old["id"] == card["id"]) {
                    items.push(card);
                }
            }
            has_next = !entries.is_empty()
                && upstream_page < number(&response["pagecount"], 0)?
                && upstream_page < 10000
                && page < 10000;
            if !has_next {
                break;
            }
        }
        let used: HashSet<_> = items
            .iter()
            .filter_map(|c| c["poster"]["resource_id"].as_str())
            .collect();
        images.retain(|image| {
            image.reference["resource_id"]
                .as_str()
                .is_some_and(|id| used.contains(id))
        });
        Ok((
            json!({"id":section.id,"title":section.title,"section_ref":section.handle,"index":section.index,
            "kind":category_kind(section.category),"filters":self.facets(section),"scanned_count":scanned,"matched_count":items.len(),
            "items":items,"has_next":has_next,"next_page":if has_next {Some(page+1)} else {None},"page":page}),
            images,
        ))
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
        // The whole first page: matching drops unrelated full-text hits before
        // keeping ten, so a related title ranked below them is not lost.
        self.cards(&response, 1)
    }

    fn cards(&self, response: &Value, page: u64) -> Result<Mapped, ApiError> {
        self.cards_limit(response, page, usize::MAX)
    }

    fn cards_limit(&self, response: &Value, page: u64, maximum: usize) -> Result<Mapped, ApiError> {
        if response.get("page").is_some() && number(&response["page"], 1)? != page {
            return Err(upstream("MacCMS returned a different page"));
        }
        number(&response["pagecount"], 0)?;
        let mut images = Vec::new();
        let items: Result<Vec<_>, _> = response["list"]
            .as_array()
            .expect("Validated list")
            .iter()
            .filter(|item| normalized_category(item["type_name"].as_str().unwrap_or("")).is_some())
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

    fn item_category(&self, item: &Value) -> &'static str {
        let type_id = identifier(&item["type_id"]).ok();
        self.sections
            .iter()
            .find(|section| Some(&section.type_id) == type_id.as_ref())
            .map(|s| s.category)
            .unwrap_or_else(|| {
                normalized_category(item["type_name"].as_str().unwrap_or(""))
                    .map(|(_, kind)| kind)
                    .unwrap_or("other")
            })
    }

    fn metadata_identity(&self, item: &Value, vod_id: &str) -> Value {
        let config_id = self.captured.config["config_id"]
            .as_str()
            .expect("Config UUID");
        json!({"config_id":config_id,"addon_key":identity("maccms_addon", &[config_id]),"namespace":"maccms",
            "type":category_kind(self.item_category(item)),"meta_id":vod_id,"canonical_id":identity("maccms_content", &[config_id, vod_id])})
    }

    fn content_id(&self, item: &Value, vod_id: &str) -> String {
        let metadata = self.metadata_identity(item, vod_id);
        identity(
            "metadata",
            &[
                "maccms",
                metadata["type"].as_str().expect("Type"),
                metadata["canonical_id"].as_str().expect("ID"),
            ],
        )
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
            json!({"id": self.content_id(item, &vod_id), "ref": self.media_ref(&data),
            "title": item["vod_name"].as_str().ok_or_else(|| upstream("MacCMS title is missing"))?, "kind": category_kind(self.item_category(item)),
            "poster": poster, "year": year, "rating": rating, "status": status,
            "update_label": item["vod_remarks"].as_str().filter(|s| !s.is_empty()),
            "provider_type": item["type_name"].as_str(), "episode_count": null,
            "backdrop":null,"plot":item["vod_content"].as_str().map(plain_text).filter(|s| !s.trim().is_empty()),
            "genres":match words(item["vod_class"].as_str()) { words if words.is_empty() => None, words => Some(words) }}),
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
        // A poster is optional, so one unusable URL must not reject the title or its page.
        let Some(url) = base
            .join(value)
            .ok()
            .filter(|url| http_url(url.as_str(), false).is_ok())
        else {
            return Ok(Value::Null);
        };
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
        details
            .as_object_mut()
            .expect("Card object")
            .remove("genres");
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
        if details["tags"].as_array().is_some_and(Vec::is_empty)
            && let Some((name, _)) = normalized_category(item["type_name"].as_str().unwrap_or(""))
            && !name.is_empty()
        {
            details["tags"] = json!([name]);
        }
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
        let metadata = self.metadata_identity(item, &identifier(&item["vod_id"])?);
        let value = json!({"details": details, "images": {}, "metadata_ref": {"identity":metadata,"content_ref":reference}});
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
        let groups = moc_core::maccms::playback(names, urls).map_err(upstream)?;
        if groups.is_empty() {
            return Ok(vec![]);
        }
        let vod_id = identifier(&item["vod_id"])?;
        let mut result = Vec::new();
        for (group_index, group) in groups.into_iter().enumerate() {
            let title = group.name;
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
            for (episode_index, episode) in group.episodes.into_iter().enumerate() {
                let label = episode.label;
                let url = episode.url;
                let fingerprint = identity("maccms_entry", &[title, episode.raw]);
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
                    label.to_owned()
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
        let groups = self.groups(&item, &self.content_id(&item, &data[1]))?;
        if groups.is_empty() {
            return Err(crate::resolution::no_links());
        }
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
