//! HLS inspection: resolve remote resources through the service HTTP policy,
//! then decode only local, bounded samples with network protocols disabled.
use super::http::{Error, fetch_limited, redirect};
use reqwest::header::{CONTENT_RANGE, HeaderMap, RANGE};
use serde::Serialize;
use std::{process::Stdio, time::Duration};
use url::Url;

const MANIFEST_LIMIT: usize = 8 * 1024 * 1024;
const SAMPLE_LIMIT: usize = 4 * 1024 * 1024;
static DECODE_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Clone, Debug, Serialize)]
pub struct PlaylistSummary {
    pub segment_count: u64,
    pub duration_ms: u64,
    pub complete: bool,
    pub first_segment_decoded: Option<bool>,
}
#[derive(Clone)]
struct Key {
    uri: String,
    iv: Option<String>,
}
#[derive(Clone)]
struct Resource {
    uri: String,
    range: Option<(u64, u64)>,
}
#[derive(Clone)]
struct Init {
    resource: Resource,
    key: Option<Key>,
}
struct Media {
    summary: PlaylistSummary,
    segment: Resource,
    sequence: u64,
    duration: f64,
    key: Option<Key>,
    init: Option<Init>,
}
enum Playlist {
    Variants(Vec<(bool, String)>),
    Media(Media),
}

fn attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let (_, mut rest) = line.split_once(':')?;
    while !rest.is_empty() {
        let (key, value) = rest.split_once('=')?;
        let (found, next) = if let Some(quoted) = value.strip_prefix('"') {
            let end = quoted.find('"')?;
            let suffix = &quoted[end + 1..];
            (
                &quoted[..end],
                if suffix.is_empty() {
                    ""
                } else {
                    suffix.strip_prefix(',')?
                },
            )
        } else {
            value.split_once(',').unwrap_or((value, ""))
        };
        if key == name {
            return Some(found);
        }
        rest = next;
    }
    None
}
fn parse_range(value: &str) -> Result<(u64, u64), Error> {
    let (length, offset) = value.split_once('@').ok_or(Error::Failed)?;
    let length = length
        .parse::<u64>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or(Error::Failed)?;
    let offset = offset.parse::<u64>().map_err(|_| Error::Failed)?;
    offset.checked_add(length - 1).ok_or(Error::Failed)?;
    Ok((offset, length))
}
fn key(line: &str) -> Result<Option<Key>, Error> {
    match attr(line, "METHOD") {
        Some("NONE") => Ok(None),
        Some("AES-128") if attr(line, "KEYFORMAT").is_none_or(|value| value == "identity") => {
            let uri = attr(line, "URI")
                .filter(|v| !v.is_empty())
                .ok_or(Error::Failed)?
                .to_owned();
            let iv = attr(line, "IV").map(str::to_owned);
            if iv.as_ref().is_some_and(|v| {
                v.len() != 34
                    || !v.starts_with("0x")
                    || !v[2..].bytes().all(|c| c.is_ascii_hexdigit())
            }) {
                return Err(Error::Failed);
            }
            Ok(Some(Key { uri, iv }))
        }
        _ => Err(Error::Failed),
    }
}
fn parse(text: &str) -> Result<Playlist, Error> {
    if text.lines().next().map(str::trim_end) != Some("#EXTM3U")
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
    {
        return Err(Error::Failed);
    }
    let mut variants = Vec::new();
    let mut pending_variant = None;
    let mut duration = None::<f64>;
    let mut total = 0f64;
    let mut count = 0u64;
    let mut valid_count = 0u64;
    let mut base_sequence = 0u64;
    let mut first = None;
    let mut current_key = None;
    let mut init = None;
    let mut next_range = None;
    let mut previous_range: Option<(String, u64)> = None;
    let mut complete = false;
    let mut gap = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with("#EXT-X-STREAM-INF:") {
            if pending_variant.is_some() {
                return Err(Error::Failed);
            }
            let video = attr(line, "RESOLUTION").is_some()
                || attr(line, "CODECS").is_some_and(|codecs| {
                    codecs.split(',').any(|codec| {
                        ["avc", "hev", "hvc", "vp9", "vp0", "av01"]
                            .iter()
                            .any(|prefix| codec.trim().starts_with(prefix))
                    })
                });
            pending_variant = Some(video);
            continue;
        }
        if line.starts_with("#EXT-X-SESSION-KEY:") {
            key(line)?;
            continue;
        }
        if line.starts_with("#EXT-X-KEY:") {
            current_key = key(line)?;
            continue;
        }
        if line.starts_with("#EXT-X-MAP:") {
            let uri = attr(line, "URI")
                .filter(|v| !v.is_empty())
                .ok_or(Error::Failed)?
                .to_owned();
            let range = attr(line, "BYTERANGE").map(parse_range).transpose()?;
            if current_key
                .as_ref()
                .is_some_and(|key: &Key| key.iv.is_none())
            {
                return Err(Error::Failed);
            }
            init = Some(Init {
                resource: Resource { uri, range },
                key: current_key.clone(),
            });
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            base_sequence = value.parse().map_err(|_| Error::Failed)?;
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXTINF:") {
            if duration.is_some() {
                return Err(Error::Failed);
            }
            let parsed = value
                .split(',')
                .next()
                .unwrap_or("")
                .parse::<f64>()
                .map_err(|_| Error::Failed)?;
            if !parsed.is_finite() || parsed <= 0.0 {
                return Err(Error::Failed);
            }
            duration = Some(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            next_range = Some(value.to_owned());
            continue;
        }
        if line == "#EXT-X-GAP" {
            gap = true;
            continue;
        }
        if line == "#EXT-X-ENDLIST" {
            complete = true;
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.contains("{$") {
            return Err(Error::Failed);
        }
        if let Some(video) = pending_variant.take() {
            variants.push((video, line.to_owned()));
            continue;
        }
        let d = duration.take().ok_or(Error::Failed)?;
        let range = if let Some(spec) = next_range.take() {
            if spec.contains('@') {
                Some(parse_range(&spec)?)
            } else {
                let length = spec
                    .parse::<u64>()
                    .ok()
                    .filter(|v| *v > 0)
                    .ok_or(Error::Failed)?;
                let (uri, end) = previous_range
                    .as_ref()
                    .filter(|(uri, _)| uri == line)
                    .ok_or(Error::Failed)?;
                let _ = uri;
                end.checked_add(length - 1).ok_or(Error::Failed)?;
                Some((*end, length))
            }
        } else {
            None
        };
        previous_range = match range {
            Some((start, length)) => Some((
                line.to_owned(),
                start.checked_add(length).ok_or(Error::Failed)?,
            )),
            None => None,
        };
        let sequence = base_sequence.checked_add(count).ok_or(Error::Failed)?;
        count = count.checked_add(1).ok_or(Error::Failed)?;
        if !gap {
            valid_count = valid_count.checked_add(1).ok_or(Error::Failed)?;
            total += d;
            if !total.is_finite() || total * 1000.0 > 9_007_199_254_740_991.0 {
                return Err(Error::Failed);
            }
            if first.is_none() {
                first = Some((
                    Resource {
                        uri: line.to_owned(),
                        range,
                    },
                    sequence,
                    d,
                    current_key.clone(),
                    init.clone(),
                ));
            }
        }
        gap = false;
    }
    if pending_variant.is_some() || duration.is_some() || next_range.is_some() {
        return Err(Error::Failed);
    }
    if !variants.is_empty() {
        return if count == 0 {
            Ok(Playlist::Variants(variants))
        } else {
            Err(Error::Failed)
        };
    }
    let (segment, sequence, duration, key, init) = first.ok_or(Error::Failed)?;
    Ok(Playlist::Media(Media {
        summary: PlaylistSummary {
            segment_count: valid_count,
            duration_ms: (total * 1000.0).round() as u64,
            complete,
            first_segment_decoded: None,
        },
        segment,
        sequence,
        duration,
        key,
        init,
    }))
}

async fn resource(
    base: &Url,
    headers: &HeaderMap,
    resource: &Resource,
    sample_limit: usize,
) -> Result<Vec<u8>, Error> {
    let target = base.join(&resource.uri).map_err(|_| Error::Failed)?;
    let mut headers = headers.clone();
    redirect(base, &target, &mut headers)?;
    let (offset, length) = resource.range.unwrap_or((0, sample_limit as u64));
    let length = length.min(sample_limit as u64);
    let end = offset.checked_add(length - 1).ok_or(Error::Failed)?;
    headers.insert(
        RANGE,
        format!("bytes={offset}-{end}")
            .parse()
            .map_err(|_| Error::Failed)?,
    );
    let (body, response, _, _) =
        fetch_limited(target, headers, None, length as usize, true).await?;
    let content_range = response.get(CONTENT_RANGE).and_then(|v| v.to_str().ok());
    if content_range.is_some_and(|value| !value.starts_with(&format!("bytes {offset}-")))
        || (offset > 0 && content_range.is_none())
    {
        return Err(Error::Failed);
    }
    if body.is_empty() || (resource.range.is_some() && body.len() != length as usize) {
        return Err(Error::Failed);
    }
    Ok(body)
}
async fn key_file(
    base: &Url,
    headers: &HeaderMap,
    spec: &Key,
    dir: &std::path::Path,
    name: &str,
) -> Result<String, Error> {
    let target = base.join(&spec.uri).map_err(|_| Error::Failed)?;
    let mut headers = headers.clone();
    redirect(base, &target, &mut headers)?;
    headers.remove(RANGE);
    let (bytes, _, _, _) = fetch_limited(target, headers, None, 16, false).await?;
    if bytes.len() != 16 {
        return Err(Error::Failed);
    }
    std::fs::write(dir.join(name), bytes).map_err(|_| Error::Failed)?;
    Ok(format!(
        "#EXT-X-KEY:METHOD=AES-128,URI=\"{name}\"{}\n",
        spec.iv
            .as_ref()
            .map(|iv| format!(",IV={iv}"))
            .unwrap_or_default()
    ))
}
async fn decode(base: &Url, headers: &HeaderMap, media: &Media) -> Result<(), Error> {
    let _permit = DECODE_SLOTS.acquire().await.map_err(|_| Error::Failed)?;
    let dir = tempfile::tempdir().map_err(|_| Error::Failed)?;
    let segment = resource(base, headers, &media.segment, SAMPLE_LIMIT).await?;
    let sample_incomplete = segment.len() == SAMPLE_LIMIT
        && media
            .segment
            .range
            .is_none_or(|(_, length)| length > SAMPLE_LIMIT as u64);
    let mp4 = media.init.is_some()
        || (segment.len() >= 8 && matches!(&segment[4..8], b"ftyp" | b"styp" | b"moof"));
    let segment_name = if mp4 { "segment.m4s" } else { "segment.ts" };
    std::fs::write(dir.path().join(segment_name), segment).map_err(|_| Error::Failed)?;
    let mut local = format!(
        "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:{}\n",
        media.duration.ceil() as u64,
        media.sequence
    );
    if let Some(init) = &media.init {
        if let Some(key) = &init.key {
            local.push_str(&key_file(base, headers, key, dir.path(), "init.key").await?);
        }
        let bytes = resource(base, headers, &init.resource, SAMPLE_LIMIT).await?;
        std::fs::write(dir.path().join("init.mp4"), bytes).map_err(|_| Error::Failed)?;
        local.push_str("#EXT-X-MAP:URI=\"init.mp4\"\n");
        if init.key.is_some() && media.key.is_none() {
            local.push_str("#EXT-X-KEY:METHOD=NONE\n");
        }
    }
    if let Some(key) = &media.key {
        local.push_str(&key_file(base, headers, key, dir.path(), "segment.key").await?);
    }
    local.push_str(&format!(
        "#EXTINF:{},\n{}\n#EXT-X-ENDLIST\n",
        media.duration, segment_name
    ));
    let manifest = dir.path().join("sample.m3u8");
    std::fs::write(&manifest, local).map_err(|_| Error::Failed)?;
    let child = tokio::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,crypto,pipe",
            "-allowed_extensions",
            "ALL",
            "-format_whitelist",
            "hls,mpegts,mov",
            "-i",
        ])
        .arg(manifest)
        .args([
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            "-threads",
            "1",
            "-f",
            "null",
            "-",
            "-progress",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| {
            eprintln!("HLS video decoder unavailable");
            Error::DecoderUnavailable
        })?;
    let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Failed)?;
    if !output.status.success()
        || !String::from_utf8_lossy(&output.stdout).lines().any(|line| {
            line.strip_prefix("frame=")
                .and_then(|v| v.trim().parse::<u64>().ok())
                .is_some_and(|n| n > 0)
        })
    {
        return Err(if sample_incomplete {
            Error::SampleIncomplete
        } else {
            Error::Failed
        });
    }
    Ok(())
}
pub async fn inspect(url: Url, headers: HeaderMap) -> Result<PlaylistSummary, Error> {
    let mut pending = vec![(url, headers, 0u8)];
    let mut visited = std::collections::HashSet::new();
    let mut timeout = false;
    let mut fallback: Option<PlaylistSummary> = None;
    while let Some((url, mut headers, depth)) = pending.pop() {
        if depth >= 3 || !visited.insert(url.clone()) {
            continue;
        }
        headers.remove(RANGE);
        let (body, response, final_url, effective_headers) =
            match fetch_limited(url, headers, None, MANIFEST_LIMIT, false).await {
                Ok(value) => value,
                Err(Error::Timeout) => {
                    timeout = true;
                    continue;
                }
                Err(_) => continue,
            };
        if let Some(range) = response.get(CONTENT_RANGE).and_then(|v| v.to_str().ok()) {
            let (span, total) = range
                .strip_prefix("bytes ")
                .and_then(|v| v.split_once('/'))
                .ok_or(Error::Failed)?;
            let (start, end) = span.split_once('-').ok_or(Error::Failed)?;
            if start != "0"
                || end.parse::<u64>().ok().and_then(|n| n.checked_add(1))
                    != total.parse::<u64>().ok()
            {
                return Err(Error::Failed);
            }
        }
        let Ok(text) = std::str::from_utf8(&body) else {
            continue;
        };
        match parse(text) {
            Ok(Playlist::Variants(mut variants)) => {
                variants.sort_by_key(|(video, _)| !video);
                for (_, variant) in variants.into_iter().rev() {
                    let Ok(next) = final_url.join(&variant) else {
                        continue;
                    };
                    let mut child_headers = effective_headers.clone();
                    if redirect(&final_url, &next, &mut child_headers).is_ok() {
                        pending.push((next, child_headers, depth + 1));
                    }
                }
            }
            Ok(Playlist::Media(mut media)) => {
                match decode(&final_url, &effective_headers, &media).await {
                    Ok(()) => {
                        media.summary.first_segment_decoded = Some(true);
                        if media.summary.complete {
                            return Ok(media.summary);
                        }
                        fallback = Some(media.summary);
                        continue;
                    }
                    Err(Error::DecoderUnavailable) => return Ok(media.summary),
                    Err(Error::Timeout | Error::SampleIncomplete) => {
                        if fallback
                            .as_ref()
                            .is_none_or(|value| value.first_segment_decoded != Some(true))
                        {
                            fallback = Some(media.summary);
                        }
                        continue;
                    }
                    Err(_) => {
                        media.summary.first_segment_decoded = Some(false);
                        if fallback.is_none() {
                            fallback = Some(media.summary);
                        }
                        continue;
                    }
                }
            }
            Err(_) => continue,
        }
    }
    if let Some(summary) = fallback {
        Ok(summary)
    } else if timeout {
        Err(Error::Timeout)
    } else {
        Err(Error::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::get};
    use std::sync::{Arc, Mutex};

    #[test]
    fn counts_full_long_playlist_and_handles_gaps() {
        let mut text = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:10\n".to_owned();
        for i in 0..3894 {
            text.push_str(&format!(
                "#EXTINF:2.0,\nsegment-{i:04}-{}.ts\n",
                "x".repeat(40)
            ));
        }
        text.push_str("#EXT-X-ENDLIST\n");
        assert!(text.len() > 65_536);
        let Playlist::Media(media) = parse(&text).unwrap() else {
            panic!("media playlist expected")
        };
        assert_eq!(media.summary.segment_count, 3894);
        assert_eq!(media.summary.duration_ms, 7_788_000);
        assert!(media.summary.complete);
        let Playlist::Media(media) = parse("#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:8\n#EXTINF:2,\n#EXT-X-GAP\nmissing.ts\n#EXTINF:3.5,\nvalid.ts\n").unwrap() else { panic!("media playlist expected") };
        assert_eq!(
            (
                media.summary.segment_count,
                media.summary.duration_ms,
                media.summary.complete,
                media.sequence
            ),
            (1, 3500, false, 9)
        );
        assert!(parse("#EXTM3U\n#EXTINF:0,\ninvalid.ts\n#EXT-X-ENDLIST\n").is_err());
        assert!(parse("#EXTM3U\n#EXTINF:3,\n").is_err());
    }

    #[tokio::test]
    async fn complete_manifest_is_read_and_first_frame_decoded() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let observed = seen.clone();
        let app = Router::new()
            .route("/master", get(|| async { "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n/list\n" }))
            .route("/list", get(move |headers: HeaderMap| {
                let observed = observed.clone(); async move {
                    observed.lock().unwrap().push(headers.contains_key(RANGE));
                    let mut manifest = "#EXTM3U\n".to_owned();
                    for i in 0..1500 { manifest.push_str(&format!("#EXTINF:1.5,\n/segment?i={i}&padding={}\n", "x".repeat(40))); }
                    manifest.push_str("#EXT-X-ENDLIST\n"); manifest
                }
            }))
            .route("/segment", get(|| async { include_bytes!("testdata/video.mpegts").to_vec() }))
            .route("/fake", get(|| async { vec![0x47; 376] }))
            .route("/bad", get(|| async { "#EXTM3U\n#EXTINF:1,\n/fake\n#EXT-X-ENDLIST\n" }))
            .route("/live", get(|| async { "#EXTM3U\n#EXTINF:1,\n/segment\n" }))
            .route("/master-alternative", get(|| async { "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n/bad\n#EXT-X-STREAM-INF:BANDWIDTH=2,RESOLUTION=160x90\n/list-small\n" }))
            .route("/master-incomplete", get(|| async { "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=160x90\n/live\n#EXT-X-STREAM-INF:BANDWIDTH=2\n/list-small\n" }))
            .route("/list-small", get(|| async { "#EXTM3U\n#EXTINF:1,\n/segment\n#EXT-X-ENDLIST\n" }))
            .route("/aes", get(|| async { "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"/key\",IV=0x00000000000000000000000000000001\n#EXTINF:1,\n/encrypted\n#EXT-X-ENDLIST\n" }))
            .route("/key", get(|| async { b"0123456789abcdef".to_vec() }))
            .route("/encrypted", get(|| async { include_bytes!("testdata/video-aes.mpegts").to_vec() }))
            .route("/fmp4", get(|| async { "#EXTM3U\n#EXT-X-MAP:URI=\"/init\"\n#EXTINF:1,\n/fmp4-segment\n#EXT-X-ENDLIST\n" }))
            .route("/init", get(|| async { include_bytes!("testdata/init.mp4").to_vec() }))
            .route("/fmp4-segment", get(|| async { include_bytes!("testdata/segment.m4s").to_vec() }))
            .route("/range-list", get(|| async { format!("#EXTM3U\n#EXTINF:1,\n#EXT-X-BYTERANGE:{}@4\n/range\n#EXT-X-ENDLIST\n", include_bytes!("testdata/video.mpegts").len()) }))
            .route("/range", get(|headers: HeaderMap| async move {
                let bytes = include_bytes!("testdata/video.mpegts");
                assert_eq!(headers.get(RANGE).unwrap().to_str().unwrap(), format!("bytes=4-{}", bytes.len() + 3));
                (axum::http::StatusCode::PARTIAL_CONTENT, [("content-range", format!("bytes 4-{}/{}", bytes.len()+3, bytes.len()+8))], bytes.to_vec())
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = |path: &str| Url::parse(&format!("http://{address}{path}")).unwrap();
        let summary = inspect(url("/master"), HeaderMap::new()).await.unwrap();
        assert_eq!(
            (summary.segment_count, summary.duration_ms, summary.complete),
            (1500, 2_250_000, true)
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec![false],
            "manifest must not be range-truncated"
        );
        assert_eq!(
            inspect(url("/bad"), HeaderMap::new())
                .await
                .unwrap()
                .first_segment_decoded,
            Some(false),
            "fake transport-stream signatures must fail actual decode"
        );
        assert!(
            !inspect(url("/live"), HeaderMap::new())
                .await
                .unwrap()
                .complete
        );
        for path in [
            "/master-alternative",
            "/master-incomplete",
            "/aes",
            "/fmp4",
            "/range-list",
        ] {
            let result = inspect(url(path), HeaderMap::new()).await;
            assert!(
                result.as_ref().is_ok_and(
                    |summary| summary.complete && summary.first_segment_decoded == Some(true)
                ),
                "{path} should decode a complete playlist: {result:?}"
            );
        }
        task.abort();
    }
}
