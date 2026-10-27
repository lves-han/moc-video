//! A bounded loopback listener. Resource routes are added by T09.
use crate::contracts::ApiError;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Gateway {
    pub address: SocketAddr,
    token: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Gateway {
    pub fn start() -> Result<Self, ApiError> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| ApiError::internal("Loopback gateway bind failed"))?;
        listener
            .set_nonblocking(true)
            .map_err(|_| ApiError::internal("Loopback gateway setup failed"))?;
        let address = listener
            .local_addr()
            .map_err(|_| ApiError::internal("Loopback address unavailable"))?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| ApiError::internal("Gateway control token generation failed"))?;
        let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let expected = token.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        let worker = thread::Builder::new()
            .name("host-gateway".into())
            .spawn(move || {
                while !stop_worker.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, peer)) if peer.ip().is_loopback() => {
                            serve(stream, &expected, &stop_worker);
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|_| ApiError::internal("Loopback gateway worker failed"))?;
        let gateway = Self {
            address,
            token,
            stop,
            worker: Some(worker),
        };
        gateway.health()?;
        Ok(gateway)
    }

    pub fn health(&self) -> Result<Value, ApiError> {
        let error = || ApiError::internal("Loopback gateway health check failed");
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(1))
            .map_err(|_| error())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|_| error())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(|_| error())?;
        write!(stream, "GET /host-health HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", self.token).map_err(|_| error())?;
        let mut response = String::new();
        stream
            .take(8193)
            .read_to_string(&mut response)
            .map_err(|_| error())?;
        if response.len() > 8192 || !response.starts_with("HTTP/1.1 200 ") {
            return Err(error());
        }
        let (_, body) = response.split_once("\r\n\r\n").ok_or_else(error)?;
        let health: Value = serde_json::from_str(body).map_err(|_| error())?;
        if health != host_health() {
            return Err(error());
        }
        Ok(health)
    }
}

fn host_health() -> Value {
    json!({"status": "ready", "contract_version": "1.5.0", "media_gateway_ready": true})
}

fn serve(mut stream: TcpStream, token: &str, stop: &AtomicBool) {
    if stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .is_err()
        || stream
            .set_write_timeout(Some(Duration::from_millis(100)))
            .is_err()
    {
        return;
    }
    let started = Instant::now();
    let mut bytes = Vec::new();
    while bytes.len() < 8192
        && started.elapsed() < Duration::from_secs(1)
        && !stop.load(Ordering::SeqCst)
    {
        let mut buffer = [0u8; 512];
        match stream.read(&mut buffer) {
            Ok(0) => return,
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => return,
        }
        if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
            break;
        }
    }
    let Ok(request) = std::str::from_utf8(&bytes) else {
        return;
    };
    if bytes.len() > 8192 || !request.ends_with("\r\n\r\n") {
        return;
    }
    let authorized =
        request
            .lines()
            .filter_map(|line| line.split_once(':'))
            .any(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    && value.trim() == format!("Bearer {token}")
            });
    let (status, body) = if !authorized {
        ("401 Unauthorized", "{}".into())
    } else if request.starts_with("GET /host-health HTTP/1.1\r\n") {
        ("200 OK", host_health().to_string())
    } else {
        ("404 Not Found", "{}".into())
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
