//! T0.42 local library on the real Host and gateway: scan, Range playback, path
//! scope, missing files, unsupported formats and resumable history.
use super::*;

fn range(url: &str, value: Option<&str>) -> (u16, Option<String>, Vec<u8>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let mut request = client.get(url);
            if let Some(value) = value {
                request = request.header("range", value);
            }
            let response = request.send().await.unwrap();
            let status = response.status().as_u16();
            let content_range = response
                .headers()
                .get("content-range")
                .map(|v| v.to_str().unwrap().to_owned());
            (
                status,
                content_range,
                response.bytes().await.unwrap().to_vec(),
            )
        })
}

#[test]
fn x5_local_library_range_scope_history_and_restart() {
    let temp = TempDir::new().unwrap();
    let library = temp.path().join("Movies");
    std::fs::create_dir_all(library.join("Season")).unwrap();
    let body: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(library.join("Season/film.mp4"), &body).unwrap();
    std::fs::write(library.join("clip.mov"), b"mov").unwrap();
    std::fs::write(library.join("show.avi"), b"avi").unwrap();
    let outside = temp.path().join("secret.mp4");
    std::fs::write(&outside, b"outside").unwrap();
    let settings_path = temp.path().join("settings.json");
    let host = Host::open(settings_path.clone()).unwrap();
    let mut settings = ok(&host, "get_settings", json!({}));
    settings["local_paths"] = json!([library.to_string_lossy()]);
    ok(&host, "save_settings", settings.clone());
    let mut relative = settings.clone();
    relative["local_paths"] = json!(["Movies"]);
    error(&call(&host, "save_settings", relative), "BAD_REQUEST");

    let list = ok(&host, "list_local_videos", json!({}));
    let names: Vec<_> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["name"].as_str().unwrap(),
                i["supported"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        names,
        vec![("film.mp4", true), ("clip.mov", true), ("show.avi", false)]
    );
    let id = |name: &str| {
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == name)
            .unwrap()["id"]
            .clone()
    };
    error(
        &call(&host, "open_local", json!({"id": id("show.avi")})),
        "MEDIA_UNSUPPORTED",
    );
    error(
        &call(
            &host,
            "open_local",
            json!({"id": crate::host::local::file_id(&outside)}),
        ),
        "SOURCE_NOT_FOUND",
    );

    let opened = ok(&host, "open_local", json!({"id": id("film.mp4")}));
    let url = opened["session"]["media_url"].as_str().unwrap();
    let (status, _, full) = range(url, None);
    assert_eq!((status, full.len()), (200, body.len()));
    let (status, content_range, part) = range(url, Some("bytes=50000-50009"));
    assert_eq!(
        (status, content_range.as_deref()),
        (206, Some("bytes 50000-50009/100000"))
    );
    assert_eq!(part, body[50_000..50_010]);
    let (status, _, tail) = range(url, Some("bytes=-5"));
    assert_eq!((status, tail.as_slice()), (206, &body[99_995..]));
    assert_eq!(range(url, Some("bytes=200000-")).0, 416);
    assert_eq!(
        ok(&host, "get_media_cache", json!({}))["used_bytes"],
        0,
        "local files are not copied into the cache"
    );

    #[cfg(unix)]
    {
        let original = library.join("Season/film.mp4");
        let backup = library.join("Season/backup.mp4");
        std::fs::rename(&original, &backup).unwrap();
        std::os::unix::fs::symlink(&outside, &original).unwrap();
        assert_eq!(
            range(url, Some("bytes=0-6")).2,
            body[..7],
            "an existing session keeps its authorized file"
        );
        error(
            &call(&host, "open_local", json!({"id": id("film.mp4")})),
            "UNAUTHORIZED",
        );
        std::fs::remove_file(&original).unwrap();
        std::fs::rename(&backup, &original).unwrap();
    }

    // Progress saves through the normal history path and survives a restart.
    let mut record = opened["record"].clone();
    record["position_ms"] = json!(42_000);
    record["duration_ms"] = json!(90_000);
    let sequence = ok(&host, "list_history", json!({}))["write_sequence"]
        .as_u64()
        .unwrap()
        + 1;
    ok(
        &host,
        "save_history",
        json!({"record": record, "session_id": opened["session"]["session_id"], "match_id": opened["match_id"], "playback_epoch": 1, "write_sequence": sequence}),
    );
    drop(host);
    let host = Host::open(settings_path).unwrap();
    assert_eq!(
        ok(&host, "get_settings", json!({}))["local_paths"][0],
        library.to_string_lossy().as_ref()
    );
    let saved = ok(&host, "list_history", json!({}))["entries"][0].clone();
    assert_eq!(
        (
            saved["position_ms"].as_u64(),
            saved["metadata_identity"]["namespace"].as_str(),
            saved["video_id"].clone()
        ),
        (Some(42_000), Some("local"), id("film.mp4"))
    );

    // A moved file reports clearly; a removed root stops serving its files.
    ok(&host, "list_local_videos", json!({}));
    let reopened = ok(&host, "open_local", json!({"id": id("film.mp4")}));
    std::fs::rename(library.join("clip.mov"), temp.path().join("moved.mov")).unwrap();
    error(
        &call(&host, "open_local", json!({"id": id("clip.mov")})),
        "SOURCE_NOT_FOUND",
    );
    let mut emptied = ok(&host, "get_settings", json!({}));
    emptied["local_paths"] = json!([]);
    ok(&host, "save_settings", emptied);
    assert_ne!(
        range(reopened["session"]["media_url"].as_str().unwrap(), None).0,
        200
    );
    error(
        &call(&host, "open_local", json!({"id": id("film.mp4")})),
        "SOURCE_NOT_FOUND",
    );
    println!(
        "X5_LOCAL:{}",
        json!({"items": names.len(), "range": "206", "resume_ms": 42_000, "cache_used": 0})
    );
}
