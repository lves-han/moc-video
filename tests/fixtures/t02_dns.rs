//! ADR-049: exercise the shared public resolver using two owned UDP servers.
use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Explicit network diagnostic; uses the production resolver without fixture injection.
#[test]
#[ignore = "requires the current machine's real network; output is diagnostic, not source acceptance"]
fn t02_dns_live_production_resolver_diagnostic() {
    let sources: Value =
        serde_json::from_str(include_str!("../../src/data/defaultSources.json")).unwrap();
    let http = Http::default();
    assert!(http.nameserver.is_none());
    let runtime = runtime();
    for source in sources.as_array().unwrap() {
        let value = source["api_url"]
            .as_str()
            .or_else(|| source["manifest_urls"][0].as_str())
            .unwrap();
        let url: Url = value.parse().unwrap();
        let result = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), http.addresses(&url)).await
        });
        let row = match result {
            Ok(Ok(addresses)) => json!({
                "host": url.host_str(),
                "addresses": addresses.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "ip_validation": false,
            }),
            Ok(Err(error)) => json!({"host": url.host_str(), "error": error.message}),
            Err(_) => json!({"host": url.host_str(), "error": "diagnostic DNS deadline"}),
        };
        println!("DNS_DIAGNOSTIC {row}");
    }
}

#[test]
fn t02_dns_two_servers_failover_and_both_families() {
    let failed = DnsFixture::with_rcode(2, |_| Some(vec![]));
    let healthy = DnsFixture::new(|kind| {
        Some(if kind == 1 {
            vec![1, 1, 1, 1]
        } else {
            "2606:4700:4700::1111"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets()
                .to_vec()
        })
    });
    for servers in [
        vec![failed.address, healthy.address],
        vec![healthy.address, failed.address],
    ] {
        let http = Http {
            nameserver: Some(servers),
        };
        let (_temp, host) = setup("https://public.example/api".into());
        let url: Url = "https://public.example/api".parse().unwrap();
        let addresses = runtime()
            .block_on(watched(
                &job(&host, "default_metadata"),
                Duration::from_secs(15),
                http.addresses(&url),
            ))
            .unwrap();
        assert!(addresses.contains(&"1.1.1.1:443".parse().unwrap()));
        assert!(addresses.contains(&"[2606:4700:4700::1111]:443".parse().unwrap()));
    }
    assert!(!failed.queries.lock().unwrap().is_empty());
    let queries = healthy.queries.lock().unwrap();
    assert!(queries.contains(&1) && queries.contains(&28));
}

#[test]
fn t02_dns_one_silent_server_keeps_other_available() {
    let silent = DnsFixture::new(|_| None);
    let healthy = DnsFixture::new(|kind| {
        Some(if kind == 1 {
            vec![8, 8, 8, 8]
        } else {
            "2001:4860:4860::8888"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets()
                .to_vec()
        })
    });
    let (_temp, host) = setup("https://public.example/api".into());
    for servers in [
        vec![silent.address, healthy.address],
        vec![healthy.address, silent.address],
    ] {
        let http = Http {
            nameserver: Some(servers),
        };
        let addresses = runtime()
            .block_on(watched(
                &job(&host, "default_metadata"),
                Duration::from_secs(15),
                http.addresses(&"https://public.example/api".parse().unwrap()),
            ))
            .unwrap();
        assert_eq!(addresses.len(), 2);
        assert!(addresses.contains(&"8.8.8.8:443".parse::<SocketAddr>().unwrap()));
        assert!(addresses.contains(&"[2001:4860:4860::8888]:443".parse::<SocketAddr>().unwrap()));
    }
    assert!(!silent.queries.lock().unwrap().is_empty());
}

#[test]
fn t02_dns_both_failed_never_falls_back_to_hosts_or_system() {
    let first = DnsFixture::with_rcode(2, |_| Some(vec![]));
    let second = DnsFixture::with_rcode(2, |_| Some(vec![]));
    let http = Http {
        nameserver: Some(vec![first.address, second.address]),
    };
    let (_temp, host) = setup("http://broadcasthost:12345/api".into());
    // Hickory synthesizes localhost per RFC 6761 independently of hosts.
    // Use macOS's ordinary hosts alias to exercise actual hosts-file bypass.
    #[cfg(target_os = "macos")]
    assert!(
        std::fs::read_to_string("/etc/hosts")
            .unwrap()
            .split_whitespace()
            .any(|word| word == "broadcasthost")
    );
    for name in ["broadcasthost", "example.com"] {
        let url: Url = format!("http://{name}:12345/api").parse().unwrap();
        let error = runtime()
            .block_on(watched(
                &job(&host, "default_metadata"),
                Duration::from_secs(15),
                http.addresses(&url),
            ))
            .unwrap_err();
        assert_eq!(error.code, "UPSTREAM_FAILED");
        assert_eq!(error.message, "Public HTTP DNS lookup failed");
    }
    for server in [&first, &second] {
        let queries = server.queries.lock().unwrap();
        assert!(queries.contains(&1) && queries.contains(&28));
    }
}

#[test]
fn t02_dns_two_silent_servers_deadline_and_cancellation() {
    let first = DnsFixture::new(|_| None);
    let second = DnsFixture::new(|_| None);
    let servers = vec![first.address, second.address];
    let (_temp, host) = setup("https://public.example/api".into());
    let http = Http {
        nameserver: Some(servers.clone()),
    };
    let started = Instant::now();
    let error = runtime()
        .block_on(watched(
            &job(&host, "default_metadata"),
            Duration::from_millis(150),
            http.addresses(&"https://public.example/api".parse().unwrap()),
        ))
        .unwrap_err();
    assert_eq!(error.code, "UPSTREAM_TIMEOUT");
    assert!(started.elapsed() < Duration::from_secs(2));
    first.wait_for_query();
    second.wait_for_query();
    let request_id = id();
    let work = host
        .requests
        .begin(&request_id, "default_metadata", host.clock.clone())
        .unwrap();
    let http = Http {
        nameserver: Some(servers),
    };
    let before = (
        first.queries.lock().unwrap().len(),
        second.queries.lock().unwrap().len(),
    );
    let worker = thread::spawn(move || {
        http.get(
            "https://public.example/api".parse().unwrap(),
            HeaderMap::new(),
            &work,
            100,
            false,
            false,
        )
    });
    let waiting = Instant::now();
    while first.queries.lock().unwrap().len() == before.0
        || second.queries.lock().unwrap().len() == before.1
    {
        assert!(waiting.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
    let started = Instant::now();
    host.requests.cancel(&id(), &request_id).unwrap();
    assert_eq!(worker.join().unwrap().err().unwrap().code, "CANCELLED");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn t02_dns_fake_ip_mixed_results_are_preserved() {
    for address in [
        "172.19.0.59",
        "198.18.1.114",
        "198.19.255.254",
        "10.0.0.2",
        "fc00::1",
        "::1",
    ] {
        let ip: std::net::IpAddr = address.parse().unwrap();
        let dns = DnsFixture::new(move |kind| {
            Some(match (kind, ip) {
                (1, std::net::IpAddr::V4(ip)) => ip.octets().to_vec(),
                (28, std::net::IpAddr::V6(ip)) => ip.octets().to_vec(),
                (1, _) => vec![8, 8, 8, 8],
                (28, _) => "2606:4700:4700::1111"
                    .parse::<std::net::Ipv6Addr>()
                    .unwrap()
                    .octets()
                    .to_vec(),
                _ => vec![],
            })
        });
        let http = Http {
            nameserver: Some(vec![dns.address]),
        };
        let addresses = runtime()
            .block_on(http.addresses(&"https://public.example/api".parse().unwrap()))
            .unwrap();
        assert!(
            addresses.contains(&SocketAddr::new(ip, 443)),
            "missing {address}"
        );
        assert_eq!(addresses.len(), 2);
    }
    n00_http_local_addresses_and_dns_refresh();
    n00_redirect_origin_headers_proxy_and_limits();
}

/// A real source request through the production transport, including current TUN/Fake-IP.
#[test]
#[ignore = "requires live source servers; emits actual HTTP outcomes"]
fn t02_dns_live_http_proxy_compatibility() {
    let sources: Value =
        serde_json::from_str(include_str!("../../src/data/defaultSources.json")).unwrap();
    for source in sources.as_array().unwrap() {
        let value = source["api_url"]
            .as_str()
            .or_else(|| source["manifest_urls"][0].as_str())
            .unwrap();
        let url: Url = value.parse().unwrap();
        let (_temp, host) = setup(value.into());
        let result = Http::default().get(
            url.clone(),
            HeaderMap::new(),
            &job(&host, "default_metadata"),
            8 * 1024 * 1024,
            false,
            false,
        );
        let row = match result {
            Ok(response) => {
                json!({"host": url.host_str(), "http_success": true, "bytes": response.body.len(), "json": serde_json::from_slice::<Value>(&response.body).is_ok()})
            }
            Err(error) => {
                assert_ne!(
                    error.details.as_ref().map(|d| &d["reason"]),
                    Some(&json!("unsafe_target"))
                );
                json!({"host": url.host_str(), "http_success": false, "error": error.message, "details": error.details})
            }
        };
        println!("HTTP_COMPATIBILITY {row}");
    }
}
