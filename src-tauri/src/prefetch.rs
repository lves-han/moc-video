//! Episode prefetch planning. Completed resources remain owned by the episode cache.
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub url: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub range: Option<String>,
    pub if_range: Option<String>,
    pub memory_only: bool,
    pub cache_key: String,
}
#[derive(Default)]
struct State { playlist: String, status: &'static str, unit: &'static str, total_ms: u64, ranges: Vec<(u64,u64)>, error: Option<String> }
#[derive(Default)]
pub struct Prefetch {
    state: Mutex<State>,
    position: AtomicU64,
    duration: AtomicU64,
    generation: AtomicU64,
}
impl Prefetch {
    pub fn position(&self, ms: u64, duration: Option<u64>) { self.position.store(ms, Ordering::Relaxed); if let Some(value) = duration { self.duration.store(value, Ordering::Relaxed); } }
    pub fn snapshot(&self) -> Value {
        let s = self.state.lock().expect("prefetch state");
        json!({"status":s.status,"unit":s.unit,"total_ms":s.total_ms,"ranges":s.ranges.iter().map(|(start,end)|json!({"start_ms":start,"end_ms":end})).collect::<Vec<_>>(),"error":s.error})
    }
    pub fn fail(&self, message: &str) {
        let mut s = self.state.lock().expect("prefetch state");
        s.status = "failed"; s.error = Some(message.into());
    }
    fn fail_generation(&self, generation: u64, message: &str) {
        let mut state = self.state.lock().expect("prefetch state");
        if self.generation.load(Ordering::SeqCst) == generation { state.status = "failed"; state.error = Some(message.into()); }
    }
    pub fn start_progressive(self: &Arc<Self>, local_url: String, origin_url: String, provider: String, revision: String,
        owner: String, source_headers: reqwest::header::HeaderMap, cache: Arc<crate::media_cache::MediaCache>, cancelled: Arc<AtomicBool>) {
        let generation = {
            let mut state = self.state.lock().expect("prefetch state");
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            *state = State { playlist: local_url.clone(), status: "loading", unit: "bytes", total_ms: 0, ranges: Vec::new(), error: None };
            generation
        };
        let this = self.clone();
        tokio::spawn(async move {
            const CHUNK: u64 = 4 * 1024 * 1024;
            let client = match reqwest::Client::builder().no_proxy().connect_timeout(std::time::Duration::from_secs(15)).build() {
                Ok(value) => value, Err(_) => { this.fail_generation(generation, "缓存连接初始化失败"); return; }
            };
            let cache_key = |range: &str, validator: Option<&str>| {
                let mut headers = source_headers.clone();
                for name in ["host", "content-length", "connection", "transfer-encoding", "origin"] { headers.remove(name); }
                headers.insert("accept-encoding", "identity".parse().expect("static header"));
                headers.insert("range", range.parse().expect("generated range"));
                if let Some(value) = validator { if let Ok(value) = value.parse() { headers.insert("if-range", value); } }
                crate::media_cache::key(&format!("{provider}:{owner}"), &revision, &origin_url, &headers)
            };
            let first_range = format!("bytes=0-{}", CHUNK - 1);
            let first_key = cache_key(&first_range, None);
            let mut attempt = 0u32;
            while cache.get(&first_key).is_none() {
                let request = client.get(&local_url).header("x-moc-prefetch", "1").header(reqwest::header::RANGE, &first_range);
                let response = tokio::select! {
                    value = tokio::time::timeout(std::time::Duration::from_secs(30), request.send()) => value,
                    _ = wait_cancel(&cancelled, &this.generation, generation) => return,
                };
                let mut response = match response {
                    Ok(Ok(value)) if value.status() == reqwest::StatusCode::PARTIAL_CONTENT => value,
                    Ok(Ok(value)) if !value.status().is_server_error() && value.status() != reqwest::StatusCode::TOO_MANY_REQUESTS => { this.fail_generation(generation, "播放源不支持分段缓存"); return; }
                    _ => { if !this.retry_wait(&cancelled, generation, &mut attempt).await { return; } continue; }
                };
                let mut interrupted = false;
                loop {
                    let chunk = tokio::select! { value = tokio::time::timeout(std::time::Duration::from_secs(30), response.chunk()) => value, _ = wait_cancel(&cancelled, &this.generation, generation) => return };
                    match chunk { Ok(Ok(Some(_))) => {}, Ok(Ok(None)) => break, _ => { interrupted = true; break; } }
                }
                if interrupted { if !this.retry_wait(&cancelled, generation, &mut attempt).await { return; } continue; }
                if cache.get(&first_key).is_none() { this.fail_generation(generation, "播放源或本机存储不允许缓存该内容"); return; }
            }
            let Some(first) = cache.get(&first_key) else { this.fail_generation(generation, "播放源或本机存储不允许缓存该内容"); return; };
            let content_range = first.headers.get("content-range").and_then(|v| v.to_str().ok()).unwrap_or("");
            let parsed = content_range.strip_prefix("bytes 0-").and_then(|v| v.split_once('/')).and_then(|(end,total)| Some((end.parse::<u64>().ok()?, total.parse::<u64>().ok()?)));
            let Some((_end, total)) = parsed.filter(|(end,total)| *total > 0 && end.checked_add(1) == Some((*total).min(CHUNK)) && first.length == end + 1) else {
                this.fail_generation(generation, "播放源返回的文件分片范围无效"); return;
            };
            let validator = first.headers.get("etag").and_then(|v| v.to_str().ok()).filter(|v| !v.starts_with("W/"))
                .or_else(|| first.headers.get("last-modified").and_then(|v| v.to_str().ok())).map(str::to_owned);
            if total > CHUNK && validator.is_none() { this.fail_generation(generation, "播放源缺少文件校验信息，无法安全缓存"); return; }
            drop(first);
            if cancelled.load(Ordering::SeqCst) || this.generation.load(Ordering::SeqCst) != generation { return; }
            {
                let mut state = this.state.lock().expect("prefetch state");
                if this.generation.load(Ordering::SeqCst) != generation { return; }
                state.total_ms = total;
            }
            let count = total.div_ceil(CHUNK);
            let mut completed = std::collections::HashSet::<u64>::new();
            while (completed.len() as u64) < count {
                if cancelled.load(Ordering::SeqCst) || this.generation.load(Ordering::SeqCst) != generation { return; }
                let position = this.position.load(Ordering::Relaxed); let duration = this.duration.load(Ordering::Relaxed);
                let desired = if duration > 0 { (((position as u128 * total as u128) / duration as u128) / CHUNK as u128).min((count - 1) as u128) as u64 } else { 0 };
                let mut index = desired;
                while index < count && completed.contains(&index) { index += 1; }
                if index == count { index = 0; while completed.contains(&index) { index += 1; } }
                let start = index * CHUNK; let end = start.saturating_add(CHUNK).min(total);
                let range = format!("bytes={start}-{}", end - 1);
                let if_range = if start == 0 { None } else { validator.clone() };
                let key = if start == 0 { first_key.clone() } else { cache_key(&range, if_range.as_deref()) };
                let segment = Segment { url: local_url.clone(), start_ms: start, end_ms: end,
                    range: Some(if start == 0 { first_range.clone() } else { range }), if_range, memory_only: false, cache_key: key };
                match this.retry_segment(&client, &cache, &cancelled, generation, &segment, "bytes", total).await {
                    Ok(true) => {}, Ok(false) => return, Err(message) => { this.fail_generation(generation, message); return; }
                }
                if !this.record_segment(generation, &segment) { return; }
                completed.insert(index);
            }
            this.complete(generation, &cancelled);
        });
    }
    async fn retry_wait(&self, cancelled: &AtomicBool, generation: u64, attempt: &mut u32) -> bool {
        {
            let mut state = self.state.lock().expect("prefetch state");
            if self.generation.load(Ordering::SeqCst) != generation { return false; }
            state.status = "retrying"; state.error = Some("网络暂不可用，正在重试缓存".into());
        }
        let seconds = (1u64 << (*attempt).min(4)).min(15); *attempt = attempt.saturating_add(1);
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(seconds)) => {},
            _ = wait_cancel(cancelled, &self.generation, generation) => return false,
        }
        let mut state = self.state.lock().expect("prefetch state");
        if cancelled.load(Ordering::SeqCst) || self.generation.load(Ordering::SeqCst) != generation { return false; }
        state.status = "loading"; state.error = None; true
    }
    async fn retry_segment(&self, client: &reqwest::Client, cache: &Arc<crate::media_cache::MediaCache>, cancelled: &AtomicBool,
        generation: u64, segment: &Segment, unit: &str, total: u64) -> Result<bool, &'static str> {
        let mut attempt = 0;
        loop {
            match self.ensure_segment(client, cache, cancelled, generation, segment, unit, total).await {
                Err("缓存下载失败，请检查网络" | "缓存下载中断") => if !self.retry_wait(cancelled, generation, &mut attempt).await { return Ok(false); },
                result => return result,
            }
        }
    }
    fn complete(&self, generation: u64, cancelled: &AtomicBool) {
        let mut state = self.state.lock().expect("prefetch state");
        if !cancelled.load(Ordering::SeqCst) && self.generation.load(Ordering::SeqCst) == generation { state.status = "complete"; }
    }
    fn record_segment(&self, generation: u64, segment: &Segment) -> bool {
        let mut state = self.state.lock().expect("prefetch state");
        if self.generation.load(Ordering::SeqCst) != generation { return false; }
        if segment.end_ms > segment.start_ms {
            state.ranges.push((segment.start_ms, segment.end_ms)); state.ranges.sort_unstable();
            let mut merged: Vec<(u64,u64)> = Vec::new();
            for (start,end) in state.ranges.drain(..) {
                if let Some(last) = merged.last_mut().filter(|last| start <= last.1) { last.1 = last.1.max(end); }
                else { merged.push((start,end)); }
            }
            state.ranges = merged;
        }
        true
    }
    async fn ensure_segment(&self, client: &reqwest::Client, cache: &Arc<crate::media_cache::MediaCache>, cancelled: &AtomicBool,
        generation: u64, segment: &Segment, unit: &str, total_ms: u64) -> Result<bool, &'static str> {
        if segment.memory_only || cache.get(&segment.cache_key).is_none() {
            let mut request = client.get(&segment.url).header("x-moc-prefetch", "1");
            if let Some(range) = &segment.range { request = request.header(reqwest::header::RANGE, range); }
            if let Some(value) = &segment.if_range { request = request.header(reqwest::header::IF_RANGE, value); }
            let response = tokio::select! {
                value = tokio::time::timeout(std::time::Duration::from_secs(30), request.send()) => value.map_err(|_| "缓存下载失败，请检查网络")?.map_err(|_| "缓存下载失败，请检查网络")?,
                _ = wait_cancel(cancelled, &self.generation, generation) => return Ok(false),
            };
            if response.status().is_server_error() || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS { return Err("缓存下载失败，请检查网络"); }
            if !response.status().is_success() || (segment.range.is_some() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT) { return Err("播放源未按请求返回可缓存的分片"); }
            let mut response = response;
            loop {
                let chunk = tokio::select! {
                    value = tokio::time::timeout(std::time::Duration::from_secs(30), response.chunk()) => value.map_err(|_| "缓存下载中断")?.map_err(|_| "缓存下载中断")?,
                    _ = wait_cancel(cancelled, &self.generation, generation) => return Ok(false),
                };
                if chunk.is_none() { break; }
            }
        }
        if segment.memory_only { return Ok(true); }
        let Some(cached) = cache.get(&segment.cache_key) else { return Err("播放源或本机存储不允许缓存该内容"); };
        if let Some(range) = &segment.range {
            let expected = range.strip_prefix("bytes=").and_then(|value| value.split_once('-')).and_then(|(a,b)| Some((a.parse::<u64>().ok()?,b.parse::<u64>().ok()?)));
            let actual = cached.headers.get("content-range").and_then(|value| value.to_str().ok()).and_then(|value| value.strip_prefix("bytes ")).and_then(|value| value.split_once('/')).and_then(|(bounds,total)| {
                let (a,b) = bounds.split_once('-')?; Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?, total.parse::<u64>().ok()?))
            });
            if expected.zip(actual).is_none_or(|((start,end),(actual_start,actual_end,total))| start != actual_start || actual_end < start || total == 0 || actual_end != end.min(total - 1) || cached.length != actual_end - start + 1 || (unit == "bytes" && (actual_end + 1 != segment.end_ms || total != total_ms))) { return Err("播放源返回的文件分片范围无效"); }
            if let Some(validator) = &segment.if_range {
                let etag = cached.headers.get("etag").and_then(|v| v.to_str().ok());
                let modified = cached.headers.get("last-modified").and_then(|v| v.to_str().ok());
                if etag != Some(validator.as_str()) && modified != Some(validator.as_str()) { return Err("播放源文件已变化，请重新缓存"); }
            }
        }
        Ok(true)
    }
    pub fn start(self: &Arc<Self>, playlist: String, segments: Vec<Segment>, total_ms: u64,
        cache: Arc<crate::media_cache::MediaCache>, cancelled: Arc<AtomicBool>) {
        self.start_units(playlist, segments, total_ms, "time", cache, cancelled);
    }
    fn start_units(self: &Arc<Self>, playlist: String, mut segments: Vec<Segment>, total_ms: u64, unit: &'static str,
        cache: Arc<crate::media_cache::MediaCache>, cancelled: Arc<AtomicBool>) {
        let generation = {
            let mut s = self.state.lock().expect("prefetch state");
            if s.playlist == playlist && s.status != "failed" { return; }
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            *s = State { playlist, status: "loading", unit, total_ms, ranges: Vec::new(), error: None };
            generation
        };
        let this = self.clone();
        tokio::spawn(async move {
            let client = match reqwest::Client::builder().no_proxy().connect_timeout(std::time::Duration::from_secs(15)).build() {
                Ok(v) => v, Err(_) => { this.fail_generation(generation, "缓存连接初始化失败"); return; }
            };
            while !segments.is_empty() {
                if cancelled.load(Ordering::SeqCst) || this.generation.load(Ordering::SeqCst) != generation { return; }
                let position = this.position.load(Ordering::Relaxed);
                let duration = this.duration.load(Ordering::Relaxed);
                let position = if unit == "bytes" && duration > 0 { ((position as u128 * total_ms as u128) / duration as u128).min(total_ms as u128) as u64 } else if unit == "bytes" { 0 } else { position };
                let next = segments.iter().position(|segment| segment.end_ms == segment.start_ms)
                    .or_else(|| segments.iter().position(|segment| segment.end_ms > position)).unwrap_or(0);
                let segment = segments.remove(next);
                match this.retry_segment(&client, &cache, &cancelled, generation, &segment, unit, total_ms).await {
                    Ok(true) => {}, Ok(false) => return, Err(message) => { this.fail_generation(generation, message); return; }
                }
                if !this.record_segment(generation, &segment) { return; }
            }
            this.complete(generation, &cancelled);
        });
    }
}
/// Video and externally supplied audio must both be cached for the same interval.
pub fn include_audio(video: &mut Value, audio: &Value) {
    let video_ranges = video["ranges"].as_array().cloned().unwrap_or_default();
    let audio_ranges = audio["ranges"].as_array().cloned().unwrap_or_default();
    let mut ranges = Vec::new();
    for a in &video_ranges { for b in &audio_ranges {
        if let (Some(a0), Some(a1), Some(b0), Some(b1)) = (a["start_ms"].as_u64(), a["end_ms"].as_u64(), b["start_ms"].as_u64(), b["end_ms"].as_u64()) {
            let start = a0.max(b0); let end = a1.min(b1);
            if end > start { ranges.push(json!({"start_ms":start,"end_ms":end})); }
        }
    } }
    video["ranges"] = json!(ranges);
    if audio["status"] == "failed" { video["status"] = json!("failed"); video["error"] = audio["error"].clone(); }
    else if audio["status"] == "retrying" && video["status"] != "failed" { video["status"] = json!("retrying"); video["error"] = audio["error"].clone(); }
    else if audio["status"] != "complete" && video["status"] != "failed" && video["status"] != "retrying" { video["status"] = json!("loading"); }
}

async fn wait_cancel(cancelled: &AtomicBool, generation: &AtomicU64, expected: u64) {
    loop {
        if cancelled.load(Ordering::SeqCst) || generation.load(Ordering::SeqCst) != expected { return; }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Parse a VOD media playlist. The caller resolves rewritten gateway URLs to cache keys.
pub fn segments(text: &str) -> Result<Vec<(String,u64,u64,Option<String>)>, &'static str> {
    if !text.lines().any(|line| line.trim() == "#EXT-X-ENDLIST") { return Err("直播或未完成的播放列表不支持完整预缓存"); }
    let mut result = Vec::new(); let mut time = 0u64; let mut duration = None; let mut next_range: Option<(u64,Option<u64>)> = None;
    let mut previous_url = String::new(); let mut previous_end = 0u64;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(value) = line.strip_prefix("#EXTINF:") {
            let seconds = value.split(',').next().unwrap_or("").parse::<f64>().map_err(|_| "播放列表时长无效")?;
            if !seconds.is_finite() || seconds <= 0.0 || seconds > 86400.0 { return Err("播放列表时长无效"); }
            duration = Some((seconds * 1000.0).round() as u64);
        } else if let Some(value) = line.strip_prefix("#EXT-X-KEY:") {
            if !value.split(',').any(|part| part == "METHOD=NONE") {
                let uri = value.split("URI=\"").nth(1).and_then(|s| s.split('"').next()).filter(|s| !s.is_empty()).ok_or("加密密钥地址无效")?;
                result.push((uri.to_owned(), time, time, None));
            }
        } else if let Some(value) = line.strip_prefix("#EXT-X-MAP:") {
            let uri = value.split("URI=\"").nth(1).and_then(|s| s.split('"').next()).filter(|s| !s.is_empty()).ok_or("初始化分片地址无效")?;
            let range = if let Some(value) = value.split("BYTERANGE=\"").nth(1).and_then(|s| s.split('"').next()) {
                let (length, offset) = value.split_once('@').ok_or("初始化分片范围缺少起点")?;
                let length = length.parse::<u64>().map_err(|_| "初始化分片范围无效")?;
                let start = offset.parse::<u64>().map_err(|_| "初始化分片范围无效")?;
                let end = start.checked_add(length).filter(|_| length > 0).ok_or("初始化分片范围无效")?;
                Some(format!("bytes={start}-{}", end - 1))
            } else { None };
            result.push((uri.to_owned(), time, time, range));
        } else if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            let (length,offset) = value.split_once('@').map_or((value,None), |(a,b)|(a,Some(b)));
            let length = length.parse::<u64>().map_err(|_| "分片范围无效")?;
            if length == 0 { return Err("分片范围无效"); }
            let offset = offset.map(|v| v.parse::<u64>().map_err(|_| "分片范围无效")).transpose()?;
            next_range = Some((length,offset));
        } else if !line.is_empty() && !line.starts_with('#') {
            let Some(length) = duration.take() else { continue; };
            let range = if let Some((size,offset)) = next_range.take() {
                let start = match offset { Some(v) => v, None if previous_url == line => previous_end, None => return Err("分片范围缺少起点") };
                let end = start.checked_add(size).ok_or("分片范围溢出")?;
                previous_url = line.to_owned(); previous_end = end;
                Some(format!("bytes={start}-{}",end - 1))
            } else { previous_url.clear(); None };
            let end = time.checked_add(length).ok_or("播放时长溢出")?;
            result.push((line.to_owned(),time,end,range)); time = end;
        }
    }
    if !result.iter().any(|(_, start, end, _)| end > start) { return Err("播放列表中没有可缓存的分片"); }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plans_vod_segments_with_explicit_and_implicit_ranges() {
        let text = "#EXTM3U\n#EXTINF:4.5,\n#EXT-X-BYTERANGE:5@0\na.ts\n#EXTINF:3.0,\n#EXT-X-BYTERANGE:7\na.ts\n#EXT-X-ENDLIST\n";
        assert_eq!(segments(text).unwrap(), vec![("a.ts".into(),0,4500,Some("bytes=0-4".into())),("a.ts".into(),4500,7500,Some("bytes=5-11".into()))]);
        assert!(segments("#EXTM3U\n#EXTINF:4,\na.ts\n").is_err());
    }
}
