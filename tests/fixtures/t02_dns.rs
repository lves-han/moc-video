//! ADR-049: exercise the shared public resolver using two owned UDP servers.
use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
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
        http.check_addresses(&url, &addresses, false).unwrap();
    }
    assert!(!failed.queries.lock().unwrap().is_empty());
    let queries = healthy.queries.lock().unwrap();
    assert!(queries.contains(&1) && queries.contains(&28));
}

#[test]
fn t02_dns_one_silent_server_keeps_other_available() {
    let silent = DnsFixture::new(|_| None);
    let healthy = DnsFixture::new(|kind| Some(if kind == 1 {
        vec![8, 8, 8, 8]
    } else {
        "2001:4860:4860::8888".parse::<std::net::Ipv6Addr>().unwrap().octets().to_vec()
    }));
    let (_temp, host) = setup("https://public.example/api".into());
    for servers in [vec![silent.address, healthy.address], vec![healthy.address, silent.address]] {
        let http = Http { nameserver: Some(servers) };
        let addresses = runtime().block_on(watched(
            &job(&host, "default_metadata"), Duration::from_secs(15),
            http.addresses(&"https://public.example/api".parse().unwrap()),
        )).unwrap();
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
    assert!(std::fs::read_to_string("/etc/hosts").unwrap().split_whitespace().any(|word| word == "broadcasthost"));
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
fn t02_dns_fake_ip_mixed_results_and_redirect_rebinding_rejected() {
    for address in ["198.18.1.114", "198.19.255.254", "10.0.0.2", "fc00::1"] {
        let ip: IpAddr = address.parse().unwrap();
        let dns = DnsFixture::new(move |kind| {
            Some(match (kind, ip) {
                (1, IpAddr::V4(ip)) => ip.octets().to_vec(),
                (28, IpAddr::V6(ip)) => ip.octets().to_vec(),
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
        let (_temp, host) = setup("https://public.example/api".into());
        let error = http
            .get(
                "https://public.example/api".parse().unwrap(),
                HeaderMap::new(),
                &job(&host, "default_metadata"),
                100,
                false,
                false,
            )
            .err()
            .unwrap();
        assert_eq!(error.details.unwrap()["reason"], "unsafe_target");
    }
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let dns = DnsFixture::new(move |kind| {
        Some(if kind == 1 {
            if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                vec![127, 0, 0, 1]
            } else {
                vec![198, 18, 1, 114]
            }
        } else {
            vec![]
        })
    });
    let fixture = Fixture::new(|_, _| Reply::redirect("/rebound".into()));
    let (_temporary, host) = setup(fixture.url("/redirect"));
    let http = Http { nameserver: Some(vec![dns.address]) };
    let error = http.get(fixture.url("/redirect").parse().unwrap(), HeaderMap::new(),
        &job(&host, "default_metadata"), 100, false, false).err().unwrap();
    assert_eq!(error.details.unwrap()["reason"], "unsafe_target");
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    n00_public_addresses_dns_rebinding_and_peer();
    n00_redirect_origin_headers_proxy_and_limits();
}
