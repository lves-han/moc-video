//! Current production gateway under a 20Mbps paced consumer for 180 seconds.
//! The fixture retains one 8KiB block; socket backpressure bounds the producer.
use super::*;

#[test]
#[ignore = "T22/A2 product endurance; explicit resource-wrapped invocation required"]
fn t22_gateway_twenty_mbps_three_minutes() {
    // Full product endurance belongs to T22/A2, not the ordinary Rust unit suite.
    let path = std::env::var_os("T22_STREAM_SAMPLE").expect("T22/A2 sample path required");
    let path = std::path::PathBuf::from(path);
    let source = Fixture::new(|_, _| Reply {
        repeat: 65536,
        ..Reply::body(vec![0x5a; 8192], "video/mp4")
    });
    let gateway = Gateway::start().unwrap();
    let address = gateway.address;
    let view = session(&gateway, source.url("/twenty-mbps.mp4"));
    let mut reader = connect(media(&view), "GET", &[]);
    let head = read_head(&mut reader);
    assert_eq!(head.status, 200);
    assert_eq!(
        head.headers["content-length"],
        (65536usize * 8192).to_string()
    );
    let started = Instant::now();
    let mut bytes = 0u64;
    let mut samples = Vec::new();
    let mut block = [0u8; 25000];
    while bytes < 450_000_000 {
        reader.read_exact(&mut block).unwrap();
        assert!(block.iter().all(|byte| *byte == 0x5a));
        bytes += block.len() as u64;
        let due = Duration::from_secs_f64(bytes as f64 / 2_500_000.0);
        if let Some(wait) = due.checked_sub(started.elapsed()) {
            thread::sleep(wait);
        }
        if started.elapsed().as_secs() >= samples.len() as u64 {
            let row = json!({"pid":std::process::id(),"seconds":started.elapsed().as_secs_f64(),
                "bytes":bytes,"produced":source.state.produced.load(Ordering::SeqCst)});
            let temporary = path.with_extension("tmp");
            std::fs::write(&temporary, row.to_string()).unwrap();
            std::fs::rename(temporary, &path).unwrap();
            samples.push(row);
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    assert!(bytes >= 450_000_000, "20Mbps payload incomplete");
    let cancelled_at = Instant::now();
    drop(reader);
    source.quiet();
    gateway.close(id(&view));
    assert_eq!(fetch(media(&view), "GET", &[]).status, 410);
    let produced = source.state.produced.load(Ordering::SeqCst);
    thread::sleep(Duration::from_millis(250));
    assert_eq!(source.state.produced.load(Ordering::SeqCst), produced);
    assert_eq!(source.state.active.load(Ordering::SeqCst), 0);
    assert_eq!(gateway.shared.records.lock().unwrap().sessions.len(), 0);
    drop(gateway);
    let listener = TcpListener::bind(address).unwrap();
    drop(listener);
    let source_port = source.port;
    drop(source);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, source_port)).unwrap();
    drop(listener);
    println!(
        "T22_STREAM:{}",
        json!({"seconds":seconds,"bytes":bytes,"target_bps":20_000_000,
        "rate_control":"25kB each 10ms paced consumer; real upstream socket backpressure",
        "samples":samples,"producer_bytes":produced,"cancel_seconds":cancelled_at.elapsed().as_secs_f64(),
        "active_requests":0,"live_sessions":0,"ports_released":true,"payload_checked":true})
    );
}
