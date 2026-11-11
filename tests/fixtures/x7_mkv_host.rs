//! T0.44 local MKV playback on the real Host and gateway: on-demand fMP4 HLS,
//! keyframe-aligned seeking, DTS to AAC, embedded subtitles, cleanup and errors.
use super::*;
use std::path::Path;

fn find<'a>(data: &'a [u8], path: &[&[u8; 4]]) -> Option<&'a [u8]> {
    let mut at = 0;
    while at + 8 <= data.len() {
        let size = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        if size < 8 || at + size > data.len() {
            return None;
        }
        if &data[at + 4..at + 8] == path[0] {
            let body = &data[at + 8..at + size];
            return if path.len() == 1 { Some(body) } else { find(body, &path[1..]) };
        }
        at += size;
    }
    None
}

/// Seconds at which a media segment's first video fragment starts (tfdt / mdhd timescale).
fn start_seconds(init: &[u8], segment: &[u8]) -> f64 {
    let mdhd = find(init, &[b"moov", b"trak", b"mdia", b"mdhd"]).unwrap();
    let timescale = if mdhd[0] == 1 { u32::from_be_bytes(mdhd[20..24].try_into().unwrap()) } else { u32::from_be_bytes(mdhd[12..16].try_into().unwrap()) };
    let tfdt = find(segment, &[b"moof", b"traf", b"tfdt"]).unwrap();
    let base = if tfdt[0] == 1 { u64::from_be_bytes(tfdt[4..12].try_into().unwrap()) } else { u64::from(u32::from_be_bytes(tfdt[4..8].try_into().unwrap())) };
    base as f64 / f64::from(timescale)
}

fn open_files(name: &str) -> usize {
    let output = std::process::Command::new("lsof").args(["-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&output.stdout).lines().filter(|l| l.contains(name)).count()
}

#[test]
fn x7_local_mkv_remux_seek_audio_subtitles_cleanup() {
    let temp = TempDir::new().unwrap();
    let library = temp.path().join("Movies");
    std::fs::create_dir_all(&library).unwrap();
    let media = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/media");
    for name in ["x7_h264_aac_srt.mkv", "x7_hevc_dts_ass.mkv", "x7_mpeg4.mkv"] {
        std::fs::copy(media.join(name), library.join(name)).unwrap();
    }
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["local_paths"] = json!([library.to_string_lossy()]);
    ok(&host, "save_settings", settings);
    let list = ok(&host, "list_local_videos", json!({}));
    assert!(list["items"].as_array().unwrap().iter().all(|i| i["supported"] == true), "MKV is listed as playable");
    let id = |name: &str| list["items"].as_array().unwrap().iter().find(|i| i["name"] == name).unwrap()["id"].clone();
    error(&call(&host, "open_local", json!({"id": id("x7_mpeg4.mkv")})), "MEDIA_UNSUPPORTED");

    let opened = ok(&host, "open_local", json!({"id": id("x7_h264_aac_srt.mkv")}));
    let session = &opened["session"];
    assert_eq!(session["stream_kind"], "hls");
    let (status, playlist) = resource(session["media_url"].as_str().unwrap());
    let playlist = String::from_utf8(playlist).unwrap();
    assert_eq!(status, 200);
    let urls: Vec<&str> = playlist.lines().filter(|l| l.starts_with("http://")).collect();
    assert!(playlist.contains("#EXT-X-PLAYLIST-TYPE:VOD") && playlist.contains("#EXT-X-ENDLIST") && urls.len() == 3, "{playlist}");
    let init_url = playlist.split("URI=\"").nth(1).unwrap().split('"').next().unwrap();
    let (_, init) = resource(init_url);
    // The middle segment is muxed from its own keyframe: its timeline starts at 4 s.
    let (status, middle) = resource(urls[1]);
    assert_eq!(status, 200);
    let start = start_seconds(&init, &middle);
    assert!((start - 4.0).abs() < 0.05, "middle segment starts at {start}");
    #[cfg(unix)]
    {
        let original = library.join("x7_h264_aac_srt.mkv");
        let backup = library.join("backup.mkv");
        std::fs::rename(&original, &backup).unwrap();
        std::os::unix::fs::symlink(media.join("x7_hevc_dts_ass.mkv"), &original).unwrap();
        assert_ne!(resource(urls[1]).0, 200, "a substituted MKV is rejected");
        std::fs::remove_file(&original).unwrap();
        std::fs::rename(&backup, &original).unwrap();
    }
    let mut whole = init.clone();
    for url in &urls {
        whole.extend(resource(url).1);
    }
    std::fs::write(temp.path().join("h264.mp4"), &whole).unwrap();
    let subtitle = &session["subtitles"][0];
    assert_eq!((subtitle["language"].as_str(), subtitle["source_format"].as_str()), (Some("chi"), Some("srt")));
    let (_, vtt) = resource(subtitle["url"].as_str().unwrap());
    let vtt = String::from_utf8(vtt).unwrap();
    assert!(vtt.starts_with("WEBVTT") && vtt.contains("Hello subtitle") && vtt.contains("第二句字幕"), "{vtt}");

    let hevc = ok(&host, "open_local", json!({"id": id("x7_hevc_dts_ass.mkv")}));
    let (_, hevc_playlist) = resource(hevc["session"]["media_url"].as_str().unwrap());
    let hevc_playlist = String::from_utf8(hevc_playlist).unwrap();
    let hevc_init_url = hevc_playlist.split("URI=\"").nth(1).unwrap().split('"').next().unwrap();
    let hevc_init = String::from_utf8_lossy(&resource(hevc_init_url).1).into_owned();
    assert!(hevc_init.contains("hvc1") && hevc_init.contains("mp4a"), "HEVC copied as hvc1 and DTS re-encoded to AAC");
    let mut whole = resource(hevc_init_url).1;
    for url in hevc_playlist.lines().filter(|l| l.starts_with("http://")) {
        whole.extend(resource(url).1);
    }
    std::fs::write(temp.path().join("hevc.mp4"), &whole).unwrap();
    assert!(String::from_utf8(resource(hevc["session"]["subtitles"][0]["url"].as_str().unwrap()).1).unwrap().contains("Hello subtitle"));
    // Optional real-WKWebView playback through this gateway (acceptance sets X7_WKCHECK).
    let mut played = Vec::new();
    if let Ok(tool) = std::env::var("X7_WKCHECK") {
        for url in [session["media_url"].as_str().unwrap(), hevc["session"]["media_url"].as_str().unwrap()] {
            let output = std::process::Command::new(&tool).args([url, "0.6"]).output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            let row: Value = serde_json::from_str(text.lines().find_map(|l| l.strip_prefix("WKCHECK:")).expect("WKWebView result")).unwrap();
            let seek = row["seek"].as_f64().unwrap();
            assert!(seek > 6.0 && row["t"].as_f64().unwrap() > seek + 1.0 && row["err"].is_null() && row["w"] == 320, "WKWebView seek playback failed: {row}");
            played.push(row);
        }
    }
    if let Ok(dir) = std::env::var("X7_KEEP") {
        for name in ["h264.mp4", "hevc.mp4"] {
            std::fs::copy(temp.path().join(name), Path::new(&dir).join(name)).unwrap();
        }
    }

    // Closing ends the sessions; no FFmpeg file handle or temporary output remains.
    for opened in [&opened, &hevc] {
        ok(&host, "close_playback", json!({"session_id": opened["session"]["session_id"]}));
    }
    assert_ne!(resource(urls[1]).0, 200);
    assert_eq!(open_files("x7_"), 0, "MKV files are closed");
    let leftovers = std::fs::read_dir(std::env::temp_dir()).unwrap().flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&format!("moc-remux-{}-", std::process::id()))).count();
    assert_eq!(leftovers, 0, "temporary segment files are removed");
    println!("X7_MKV:{}", json!({"segments": urls.len(), "middle_start": (start * 1000.0).round() / 1000.0, "subtitle": "chi", "wkwebview": played}));
}

/// A >4 GB MKV prepared by the acceptance script (X7_LARGE): open, seek past 4 GB and play.
#[test]
fn x7_large_mkv_seek_past_four_gigabytes() {
    let Ok(path) = std::env::var("X7_LARGE") else { return };
    let file = Path::new(&path);
    assert!(std::fs::metadata(file).unwrap().len() > 4 * 1024 * 1024 * 1024, "fixture must exceed 4 GiB");
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["local_paths"] = json!([file.parent().unwrap().to_string_lossy()]);
    ok(&host, "save_settings", settings);
    let list = ok(&host, "list_local_videos", json!({}));
    let id = list["items"].as_array().unwrap().iter().find(|i| i["path"].as_str().unwrap().ends_with(".mkv")).unwrap()["id"].clone();
    let started = std::time::Instant::now();
    let opened = ok(&host, "open_local", json!({"id": id}));
    let open_ms = started.elapsed().as_millis();
    let (_, playlist) = resource(opened["session"]["media_url"].as_str().unwrap());
    let playlist = String::from_utf8(playlist).unwrap();
    let urls: Vec<&str> = playlist.lines().filter(|l| l.starts_with("http://")).collect();
    let (_, init) = resource(playlist.split("URI=\"").nth(1).unwrap().split('"').next().unwrap());
    // The last segments sit beyond the 4 GiB file offset.
    let n = urls.len() - 2;
    let started = std::time::Instant::now();
    let (status, segment) = resource(urls[n]);
    let segment_ms = started.elapsed().as_millis();
    assert_eq!(status, 200);
    let expected: f64 = playlist.lines().filter(|l| l.starts_with("#EXTINF:")).take(n)
        .map(|l| l.trim_start_matches("#EXTINF:").trim_end_matches(',').parse::<f64>().unwrap()).sum();
    let start = start_seconds(&init, &segment);
    assert!((start - expected).abs() < 0.05, "segment {n} starts at {start}, expected {expected}");
    let mut played = Value::Null;
    if let Ok(tool) = std::env::var("X7_WKCHECK") {
        let output = std::process::Command::new(&tool).args([opened["session"]["media_url"].as_str().unwrap(), "0.95"]).output().unwrap();
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        played = serde_json::from_str(text.lines().find_map(|l| l.strip_prefix("WKCHECK:")).expect("WKWebView result")).unwrap();
        let seek = played["seek"].as_f64().unwrap();
        assert!(seek > 0.9 * played["d"].as_f64().unwrap() && played["t"].as_f64().unwrap() > seek + 1.0 && played["err"].is_null(), "large seek failed: {played}");
    }
    ok(&host, "close_playback", json!({"session_id": opened["session"]["session_id"]}));
    println!("X7_LARGE:{}", json!({"bytes": std::fs::metadata(file).unwrap().len(), "segments": urls.len(), "open_ms": open_ms,
        "segment_ms": segment_ms, "segment_start": (start * 1000.0).round() / 1000.0, "wkwebview": played}));
}
