//! Pinned font downloads use the same fixed-DNS HTTP transport as source data.
use crate::{
    contracts::ApiError,
    host::maccms::{Http, http_url},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

#[derive(Deserialize)]
pub(crate) struct FontSpec {
    pub(crate) id: String,
    url: String,
    pub(crate) bytes: usize,
    sha256: String,
}

fn catalog() -> &'static [FontSpec] {
    static CATALOG: OnceLock<Vec<FontSpec>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/data/fontCatalog.json"))
            .expect("Packaged font catalog")
    })
}

pub(crate) fn get(id: &str) -> Result<&'static FontSpec, ApiError> {
    catalog()
        .iter()
        .find(|font| font.id == id)
        .ok_or_else(|| ApiError::bad("Unknown packaged font"))
}

pub(crate) async fn download(http: &Http, font: &FontSpec) -> Result<Vec<u8>, ApiError> {
    let url = http_url(&font.url, false)?;
    let mut response = http
        .open(url, Default::default(), reqwest::Method::GET, 10_000, false)
        .await?;
    if !response.status().is_success() {
        return Err(ApiError::new(
            "UPSTREAM_FAILED",
            "Font server returned an unsuccessful status",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::new("UPSTREAM_FAILED", "Font transfer failed"))?
    {
        if bytes.len().saturating_add(chunk.len()) > font.bytes {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Font download exceeds pinned size",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() != font.bytes || format!("{:x}", Sha256::digest(&bytes)) != font.sha256 {
        return Err(ApiError::new(
            "UPSTREAM_FAILED",
            "Font integrity verification failed",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_pinned_font_ids_are_exposed() {
        assert_eq!(catalog().len(), 2);
        for font in catalog() {
            let url = http_url(&font.url, false).unwrap();
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some("raw.githubusercontent.com"));
            assert!(font.bytes > 0 && font.bytes <= 32 * 1024 * 1024);
            assert_eq!(font.sha256.len(), 64);
        }
        for id in ["../settings.json", "https://example.com/font", "", "system"] {
            assert!(get(id).is_err());
        }
    }

    #[test]
    #[ignore = "explicit live font download through the production fixed-DNS transport"]
    fn live_font_uses_production_http_and_matches_the_pin() {
        let font = get("noto-sans-sc").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let bytes = runtime.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(180),
                download(&Http::default(), font),
            )
            .await
            .unwrap()
            .unwrap()
        });
        assert_eq!(bytes.len(), font.bytes);
        println!(
            "NATIVE_FONT_LIVE id={} bytes={} sha256={:x}",
            font.id,
            bytes.len(),
            Sha256::digest(&bytes)
        );
    }

    #[test]
    fn native_download_enforces_status_size_and_digest() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        for (status, body, digest, expected) in [
            (
                200,
                b"font".as_slice(),
                format!("{:x}", Sha256::digest(b"font")),
                None,
            ),
            (
                200,
                b"font".as_slice(),
                "0".repeat(64),
                Some("UPSTREAM_FAILED"),
            ),
            (
                200,
                b"font-too-large".as_slice(),
                "0".repeat(64),
                Some("RESOURCE_LIMIT"),
            ),
            (
                200,
                b"fo".as_slice(),
                "0".repeat(64),
                Some("UPSTREAM_FAILED"),
            ),
            (
                503,
                b"font".as_slice(),
                "0".repeat(64),
                Some("UPSTREAM_FAILED"),
            ),
        ] {
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
            let url = format!("http://{}/font", listener.local_addr().unwrap());
            let worker = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() <= 4096);
                }
                let response = format!(
                    "HTTP/1.1 {status} Font\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                let _ = stream.write_all(body);
            });
            let font = FontSpec {
                id: "fixture".into(),
                url,
                bytes: 4,
                sha256: digest,
            };
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let result = runtime.block_on(download(&Http::default(), &font));
            worker.join().unwrap();
            if let Some(code) = expected {
                assert_eq!(result.unwrap_err().code, code);
            } else {
                assert_eq!(result.unwrap(), b"font");
            }
        }
    }
}
