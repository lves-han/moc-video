use hickory_resolver::{
    config::{NameServerConfig, ResolveHosts, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
};
use reqwest::{
    StatusCode,
    header::{HeaderMap, LOCATION},
    redirect::Policy,
};
use serde_json::Value;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use url::Url;

#[derive(Debug)]
pub enum Error {
    Unauthorized,
    Failed,
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

fn redirect(url: &Url, next: &Url, headers: &mut HeaderMap) -> Result<(), Error> {
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
    tokio::time::timeout(Duration::from_secs(8), fetch(url, headers, None))
        .await
        .map_err(|_| Error::Failed)?
}

pub async fn post_json(url: Url, headers: HeaderMap, body: &Value) -> Result<Value, Error> {
    let bytes = tokio::time::timeout(Duration::from_secs(8), fetch(url, headers, Some(body)))
        .await
        .map_err(|_| Error::Failed)??;
    if bytes.is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::Failed)
}

async fn fetch(
    mut url: Url,
    mut headers: HeaderMap,
    body: Option<&Value>,
) -> Result<Vec<u8>, Error> {
    for _ in 0..=5 {
        if !valid(&url) {
            return Err(Error::Failed);
        }
        let host = url.host_str().ok_or(Error::Failed)?;
        let port = url.port_or_known_default().ok_or(Error::Failed)?;
        let addresses = if let Ok(ip) = host.parse::<IpAddr>() {
            vec![SocketAddr::new(ip, port)]
        } else {
            let mut set = tokio::task::JoinSet::new();
            for ip in [Ipv4Addr::new(1, 1, 1, 1), Ipv4Addr::new(8, 8, 8, 8)] {
                let mut builder = hickory_resolver::TokioResolver::builder_with_config(
                    ResolverConfig::from_name_servers(vec![NameServerConfig::udp(ip.into())]),
                    TokioRuntimeProvider::default(),
                );
                builder.options_mut().use_hosts_file = ResolveHosts::Never;
                let resolver = builder.build().map_err(|_| Error::Failed)?;
                let host = format!("{}.", host.trim_end_matches('.'));
                set.spawn(async move {
                    resolver.lookup_ip(host).await.map(|ips| {
                        ips.iter()
                            .map(|ip| SocketAddr::new(ip, port))
                            .collect::<Vec<_>>()
                    })
                });
            }
            let mut found = None;
            while let Some(result) = set.join_next().await {
                if let Ok(Ok(ips)) = result {
                    if !ips.is_empty() {
                        found = Some(ips);
                        break;
                    }
                }
            }
            found.ok_or(Error::Failed)?
        };
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(8))
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| Error::Failed)?;
        let request = if let Some(body) = body {
            client.post(url.clone()).json(body)
        } else {
            client.get(url.clone())
        };
        let mut response = request
            .headers(headers.clone())
            .send()
            .await
            .map_err(|_| Error::Failed)?;
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
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Failed)? {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                return Err(Error::Failed);
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(bytes);
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
}
