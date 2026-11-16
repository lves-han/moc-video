use hickory_resolver::{
    TokioResolver,
    config::{NameServerConfig, ResolveHosts, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
};
use reqwest::{
    StatusCode,
    dns::{Addrs, Name, Resolve, Resolving},
    header::{HeaderMap, LOCATION},
    redirect::Policy,
};
use serde_json::Value;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, OnceLock},
    time::Duration,
};
use url::Url;

struct FallbackDns {
    system: Option<TokioResolver>,
    public: Vec<TokioResolver>,
}

impl FallbackDns {
    fn new() -> Self {
        let system = TokioResolver::builder_tokio()
            .ok()
            .map(|b| b.build())
            .transpose()
            .ok()
            .flatten();
        let public = [Ipv4Addr::new(1, 1, 1, 1), Ipv4Addr::new(8, 8, 8, 8)]
            .into_iter()
            .filter_map(|ip| {
                let mut builder = TokioResolver::builder_with_config(
                    ResolverConfig::from_name_servers(vec![NameServerConfig::udp(ip.into())]),
                    TokioRuntimeProvider::default(),
                );
                builder.options_mut().use_hosts_file = ResolveHosts::Never;
                builder.build().ok()
            })
            .collect();
        Self { system, public }
    }
}

impl Resolve for FallbackDns {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        let system = self.system.clone();
        let public = self.public.clone();
        Box::pin(async move {
            if let Ok(ip) = host.parse::<IpAddr>() {
                let addrs: Addrs = Box::new(std::iter::once(SocketAddr::new(ip, 0)));
                return Ok(addrs);
            }
            let fqdn = format!("{}.", host.trim_end_matches('.'));
            if let Some(resolver) = system {
                if let Ok(ips) = resolver.lookup_ip(fqdn.clone()).await {
                    let collected: Vec<SocketAddr> =
                        ips.iter().map(|ip| SocketAddr::new(ip, 0)).collect();
                    if !collected.is_empty() {
                        let addrs: Addrs = Box::new(collected.into_iter());
                        return Ok(addrs);
                    }
                }
            }
            let mut set = tokio::task::JoinSet::new();
            for resolver in public {
                let fqdn = fqdn.clone();
                set.spawn(async move {
                    resolver.lookup_ip(fqdn).await.map(|ips| {
                        ips.iter()
                            .map(|ip| SocketAddr::new(ip, 0))
                            .collect::<Vec<_>>()
                    })
                });
            }
            while let Some(result) = set.join_next().await {
                if let Ok(Ok(ips)) = result {
                    if !ips.is_empty() {
                        let addrs: Addrs = Box::new(ips.into_iter());
                        return Ok(addrs);
                    }
                }
            }
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "DNS lookup failed",
            )) as Box<dyn std::error::Error + Send + Sync>)
        })
    }
}

fn shared_client() -> Result<&'static reqwest::Client, Error> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(8))
                .dns_resolver(Arc::new(FallbackDns::new()))
                .build()
                .ok()
        })
        .as_ref()
        .ok_or(Error::Failed)
}

#[derive(Debug)]
pub enum Error {
    Unauthorized,
    Failed,
    DecoderUnavailable,
    Timeout,
    SampleIncomplete,
}

pub fn valid(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
}

pub(crate) fn allowed_header(name: &reqwest::header::HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "host" | "content-length" | "connection" | "transfer-encoding" | "proxy-authorization"
    )
}

pub(super) fn redirect(url: &Url, next: &Url, headers: &mut HeaderMap) -> Result<(), Error> {
    if !valid(next) || (url.scheme() == "https" && next.scheme() != "https") {
        return Err(Error::Failed);
    }
    if url.origin() != next.origin() {
        if url.query_pairs().any(|(key, _)| {
            matches!(
                key.to_ascii_lowercase().as_str(),
                "api_key" | "access_token" | "token"
            )
        }) {
            return Err(Error::Failed);
        }
        *headers = headers
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
    }
    Ok(())
}

pub async fn json(url: Url, headers: HeaderMap) -> Result<Value, Error> {
    let bytes = bytes(url, headers).await?;
    serde_json::from_slice(&bytes).map_err(|_| Error::Failed)
}

pub async fn bytes(url: Url, headers: HeaderMap) -> Result<Vec<u8>, Error> {
    tokio::time::timeout(Duration::from_secs(8), fetch(url, headers, None, false))
        .await
        .map_err(|_| Error::Failed)?
        .map(|(body, _, _, _)| body)
}

pub async fn post_json(url: Url, headers: HeaderMap, body: &Value) -> Result<Value, Error> {
    let bytes = tokio::time::timeout(
        Duration::from_secs(8),
        fetch(url, headers, Some(body), false),
    )
    .await
    .map_err(|_| Error::Failed)??
    .0;
    if bytes.is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::Failed)
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub size: Option<u64>,
    pub playlist: Option<super::hls_probe::PlaylistSummary>,
}

pub async fn probe(url: Url, mut headers: HeaderMap, hls: bool) -> Result<Probe, Error> {
    if hls {
        tokio::time::timeout(Duration::from_secs(15), super::hls_probe::inspect(url, headers))
            .await.map_err(|_| Error::Timeout)?
            .map(|playlist| Probe { size: None, playlist: Some(playlist) })
    } else {
        headers.insert(reqwest::header::RANGE, "bytes=0-65535".parse().map_err(|_| Error::Failed)?);
        tokio::time::timeout(Duration::from_secs(7), probe_file(url, headers))
            .await.map_err(|_| Error::Failed)?
    }
}

fn media_body(body: &[u8], headers: &HeaderMap, encrypted: bool) -> Result<(), Error> {
    let mime = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let prefix = String::from_utf8_lossy(&body[..body.len().min(512)])
        .trim_start()
        .to_ascii_lowercase();
    if body.is_empty()
        || mime.contains("text/html")
        || mime.contains("application/json")
        || prefix.starts_with("<!doctype")
        || prefix.starts_with("<html")
        || prefix.starts_with("<?xml")
        || prefix.starts_with('{')
    {
        return Err(Error::Failed);
    }
    let known = (body.len() >= 8 && matches!(&body[4..8], b"ftyp" | b"styp" | b"moof" | b"moov"))
        || body.starts_with(b"FLV")
        || body.starts_with(b"OggS")
        || body.starts_with(b"ID3")
        || body.starts_with(b"RIFF")
        || body.starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
        || body.starts_with(&[0, 0, 1, 0xba])
        || (body[0] == 0x47 && (body.len() < 189 || body[188] == 0x47))
        || (body.len() > 1 && body[0] == 0xff && body[1] & 0xe0 == 0xe0);
    if known || (encrypted && body.len() >= 16) {
        Ok(())
    } else {
        Err(Error::Failed)
    }
}

async fn probe_file(url: Url, headers: HeaderMap) -> Result<Probe, Error> {
    let (body, response_headers, _, _) = fetch(url, headers, None, true).await?;
    media_body(&body, &response_headers, false)?;
    let size = response_headers.get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("bytes "))
        .and_then(|v| v.split_once('/')).and_then(|(_, size)| size.parse::<u64>().ok())
        .filter(|size| *size > 0 && *size <= 9_007_199_254_740_991);
    Ok(Probe { size, playlist: None })
}

async fn fetch(
    url: Url,
    headers: HeaderMap,
    body: Option<&Value>,
    probe: bool,
) -> Result<(Vec<u8>, HeaderMap, Url, HeaderMap), Error> {
    fetch_limited(url, headers, body, if probe { 65_536 } else { 8 * 1024 * 1024 }, probe).await
}

pub(super) async fn fetch_limited(
    mut url: Url,
    mut headers: HeaderMap,
    body: Option<&Value>,
    limit: usize,
    truncate: bool,
) -> Result<(Vec<u8>, HeaderMap, Url, HeaderMap), Error> {
    for _ in 0..=5 {
        if !valid(&url) {
            return Err(Error::Failed);
        }
        let client = shared_client()?;
        let request = if let Some(body) = body {
            client.post(url.clone()).json(body)
        } else {
            client.get(url.clone())
        };
        let mut response = request
            .headers(headers.clone())
            .send()
            .await
            .map_err(|error| if error.is_timeout() { Error::Timeout } else { Error::Failed })?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(Error::Failed)?;
            let next = url.join(location).map_err(|_| Error::Failed)?;
            if body.is_some()
                && (url.origin() != next.origin()
                    || !matches!(
                        response.status(),
                        StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT
                    ))
            {
                return Err(Error::Failed);
            }
            redirect(&url, &next, &mut headers)?;
            url = next;
            continue;
        }
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(Error::Unauthorized);
        }
        if !response.status().is_success() {
            return Err(Error::Failed);
        }
        let response_headers = response.headers().clone();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| if error.is_timeout() { Error::Timeout } else { Error::Failed })? {
            if truncate {
                let remaining = limit.saturating_sub(bytes.len());
                bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                if bytes.len() >= limit {
                    break;
                }
            } else {
                if bytes.len().saturating_add(chunk.len()) > limit {
                    return Err(Error::Failed);
                }
                bytes.extend_from_slice(&chunk);
            }
        }
        return Ok((bytes, response_headers, url, headers));
    }
    Err(Error::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{AUTHORIZATION, COOKIE};
    #[test]
    fn redirects_protect_credentials_and_tls() {
        let source = Url::parse("https://first.example/path").unwrap();
        let other = Url::parse("https://second.example/path").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer sample".parse().unwrap());
        headers.insert(COOKIE, "example=sample".parse().unwrap());
        headers.insert("x-api-key", "sample".parse().unwrap());
        assert!(redirect(&source, &other, &mut headers).is_ok());
        assert!(
            !headers.contains_key(AUTHORIZATION)
                && !headers.contains_key(COOKIE)
                && !headers.contains_key("x-api-key")
        );
        assert!(
            redirect(
                &source,
                &Url::parse("http://first.example/path").unwrap(),
                &mut headers
            )
            .is_err()
        );
        assert!(
            redirect(
                &Url::parse("https://first.example/path?api_key=sample").unwrap(),
                &other,
                &mut headers
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn playback_probe_rejects_html_and_checks_nested_hls_segments() {
        use axum::{Router, routing::get};
        let app = Router::new()
            .route(
                "/html",
                get(|| async { ([("content-type", "video/mp4")], "<html>blocked</html>") }),
            )
            .route(
                "/movie",
                get(|| async {
                    (
                        [("content-type", "application/octet-stream")],
                        b"\0\0\0\x18ftypisom".to_vec(),
                    )
                }),
            )
            .route(
                "/master",
                get(|| async { "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n/nested\n" }),
            )
            .route(
                "/nested",
                get(|| async { "#EXTM3U\n#EXTINF:1,\n/segment\n#EXT-X-ENDLIST\n" }),
            )
            .route(
                "/bad-playlist",
                get(|| async { "#EXTM3U\n#EXTINF:1,\n/html\n#EXT-X-ENDLIST\n" }),
            )
            .route(
                "/segment",
                get(|| async { ([("content-type", "video/mp2t")], include_bytes!("testdata/video.mpegts").to_vec()) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let endpoint = |path: &str| Url::parse(&format!("http://{address}{path}")).unwrap();
        assert!(
            probe(endpoint("/html"), HeaderMap::new(), false)
                .await
                .is_err()
        );
        assert!(
            probe(endpoint("/movie"), HeaderMap::new(), false)
                .await
                .is_ok()
        );
        assert_eq!(
            probe(endpoint("/bad-playlist"), HeaderMap::new(), true)
                .await.unwrap().playlist.unwrap().first_segment_decoded,
            Some(false)
        );
        assert!(
            probe(endpoint("/master"), HeaderMap::new(), true)
                .await
                .is_ok()
        );
        task.abort();
    }

    #[tokio::test]
    async fn redirected_manifest_does_not_forward_credentials_to_child_requests() {
        use axum::{Router, routing::get};
        use std::sync::{Arc, Mutex};
        let seen = Arc::new(Mutex::new(Vec::new()));
        let manifest_seen = seen.clone();
        let segment_seen = seen.clone();
        let target = Router::new()
            .route(
                "/playlist",
                get(move |headers: HeaderMap| {
                    let seen = manifest_seen.clone();
                    async move {
                        seen.lock()
                            .unwrap()
                            .push(headers.contains_key(reqwest::header::AUTHORIZATION));
                        "#EXTM3U\n#EXTINF:1,\n/segment\n#EXT-X-ENDLIST\n"
                    }
                }),
            )
            .route(
                "/segment",
                get(move |headers: HeaderMap| {
                    let seen = segment_seen.clone();
                    async move {
                        seen.lock()
                            .unwrap()
                            .push(headers.contains_key(reqwest::header::AUTHORIZATION));
                        include_bytes!("testdata/video.mpegts").to_vec()
                    }
                }),
            );
        let target_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target_listener.local_addr().unwrap();
        let target_task =
            tokio::spawn(async move { axum::serve(target_listener, target).await.unwrap() });
        let redirect_url = format!("http://{target_addr}/playlist");
        let source = Router::new().route(
            "/redirect",
            get(move || {
                let url = redirect_url.clone();
                async move { axum::response::Redirect::temporary(&url) }
            }),
        );
        let source_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let source_addr = source_listener.local_addr().unwrap();
        let source_task =
            tokio::spawn(async move { axum::serve(source_listener, source).await.unwrap() });
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            "Bearer private".parse().unwrap(),
        );
        assert!(
            probe(
                Url::parse(&format!("http://{source_addr}/redirect")).unwrap(),
                headers,
                true
            )
            .await
            .is_ok()
        );
        assert_eq!(*seen.lock().unwrap(), vec![false, false]);
        source_task.abort();
        target_task.abort();
    }
}
