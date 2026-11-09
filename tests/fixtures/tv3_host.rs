//! TV3's packaged window talks to the production Host through the existing bridge.
use super::*;
use time::OffsetDateTime;

const SECOND_METADATA: &str = "77777777-7777-4777-8777-777777777777";
const GENRES: [&str; 6] = [
    "Action",
    "Comedy",
    "Science Fiction",
    "Drama",
    "Animation",
    "Documentary",
];

pub(super) fn manifest3() -> Value {
    let year = OffsetDateTime::now_utc().year();
    let catalogs: Vec<_> = ["movie", "series"]
        .into_iter()
        .flat_map(|kind| {
            [
                ("tmdb.top", json!(GENRES)),
                ("tmdb.latest", json!(GENRES)),
                (
                    "tmdb.year",
                    json!([year.to_string(), (year - 1).to_string()]),
                ),
                (
                    "tmdb.language",
                    json!(["Mandarin", "Korean", "Japanese", "English"]),
                ),
                ("tmdb.trending", json!(["Day", "Week"])),
            ]
            .into_iter()
            .map(move |(id, options)| {
                json!({"id":id,"name":id,"type":kind,"pageSize":20,
                "extra":[{"name":"genre","options":options},{"name":"skip"}]})
            })
        })
        .collect();
    json!({"id":"tmdb-addon","version":"1.0.0","name":"TV3 controlled TMDB","description":"Controlled acceptance source",
        "types":["movie","series"],"resources":["catalog","meta"],"catalogs":catalogs})
}

pub(super) fn media3(
    kind: &str,
    index: usize,
    genre: &str,
    year: i32,
    label: &str,
    port: u16,
) -> Value {
    json!({"id":format!("tmdb:{}", index + 1),"type":kind,"name":format!("{label} {kind} {genre} {index}"),
        "releaseInfo":year.to_string(),"genres":[genre],"description":"A controlled story for the TV3 real-window behavior checks. Discover characters, places and ideas.",
        "imdbRating":"8.2","poster":format!("http://maccms.fixture.test:{port}/poster.png?i={index}"),
        "background":format!("http://maccms.fixture.test:{port}/backdrop.png?i={index}"),
        "videos":[{"id":format!("video:{index}"),"season":1,"episode":3,"title":"Episode 3"}]})
}

pub(super) fn upstream3(request: &Request, port: u16, mode: usize) -> Reply {
    if request.target.contains("manifest.json") {
        let mut manifest = manifest3();
        manifest["version"] = json!(format!("1.0.{mode}"));
        return Reply::json(manifest);
    }
    if request.target.starts_with("/poster.png") || request.target.starts_with("/backdrop.png") {
        let seed = request
            .query
            .get("i")
            .and_then(|i| i.parse::<u32>().ok())
            .unwrap_or(0);
        let landscape = request.target.starts_with("/backdrop");
        let (width, height) = if landscape { (960, 540) } else { (240, 360) };
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([
                (25 + (seed * 37 + x / 5) % 150) as u8,
                (35 + (seed * 13 + y / 4) % 120) as u8,
                (45 + (seed * 29 + (x + y) / 6) % 130) as u8,
            ])
        });
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        return Reply::raw(output.into_inner(), "image/png");
    }
    if request.target.contains("/catalog/") || request.target.contains("/meta/") {
        let url = url::Url::parse(&format!("http://fixture{}", request.target)).unwrap();
        let parts: Vec<_> = url.path_segments().unwrap().collect();
        let at = parts
            .iter()
            .position(|p| *p == "catalog" || *p == "meta")
            .unwrap();
        let kind = parts[at + 1];
        let label = if request.target.contains("/second/") {
            "Second"
        } else {
            "Fixture"
        };
        let year = OffsetDateTime::now_utc().year();
        if parts[at] == "meta" {
            return Reply::json(json!({"meta":media3(kind, 0, "Drama", year, label, port)}));
        }
        let id = parts[at + 2].trim_end_matches(".json");
        let extra: HashMap<String, String> = url::form_urlencoded::parse(
            parts
                .get(at + 3)
                .unwrap_or(&"")
                .trim_end_matches(".json")
                .as_bytes(),
        )
        .into_owned()
        .collect();
        let skip = extra
            .get("skip")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let selected = extra.get("genre").map(String::as_str);
        if mode == 1 && id == "tmdb.top" && selected == Some("Drama") {
            return Reply {
                status: 503,
                ..Reply::raw("TV3 controlled source failure", "text/plain")
            };
        }
        if mode == 2 || skip >= 60 {
            return Reply::json(json!({"metas":[]}));
        }
        let items: Vec<_> = (skip..skip + 20)
            .map(|index| {
                let genre = if matches!(id, "tmdb.top" | "tmdb.latest") {
                    selected.unwrap_or(GENRES[index % GENRES.len()])
                } else {
                    GENRES[index % GENRES.len()]
                };
                let release = if id == "tmdb.year" {
                    selected.and_then(|v| v.parse().ok()).unwrap_or(year)
                } else {
                    year - (index % 2) as i32
                };
                media3(
                    kind,
                    index,
                    genre,
                    release,
                    &format!(
                        "{label} {}",
                        if id == "tmdb.language" {
                            selected.unwrap_or("Mandarin")
                        } else {
                            ""
                        }
                    ),
                    port,
                )
            })
            .collect();
        return Reply::json(json!({"metas":items}));
    }
    if request.target.contains(".mp4") {
        return Reply::raw(vec![0x5a; 65536], "video/mp4");
    }
    Reply::json(response(json!([item(port)]), json!([])))
}

fn publish3(path: &std::path::Path, value: &Value) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

#[test]
fn tv3_rejects_catalog_only_for_non_stremio_sources() {
    let fixture = fixture();
    let (_temp, host, provider, _, _) = setup_both(
        &fixture,
        Arc::new(crate::requests::MonotonicClock::default()),
    );
    let mut input = home_input(&provider);
    input["catalog_only"] = json!(true);
    let failed = call(&host, "get_home", input);
    error(&failed, "BAD_REQUEST");
    assert!(
        failed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Stremio")
    );
    assert_eq!(host.requests.active_count(), 0);
}

#[test]
fn tv3_packaged_host_bridge() {
    let Some(directory) = std::env::var_os("T12_BRIDGE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let mode = Arc::new(AtomicUsize::new(0));
    let observed = mode.clone();
    let fixture = Fixture::new(move |request, port| {
        upstream3(request, port, observed.load(Ordering::SeqCst))
    });
    let temp = TempDir::new().unwrap();
    let host = Host::open(temp.path().join("settings.json")).unwrap();
    let mut settings = settings::defaults();
    settings["source_configs"] = json!([
        {"kind":"maccms","config_id":CONFIG,"name":"Playback fixture","enabled":true,"api_url":fixture.url("/api")},
        {"kind":"stremio","config_id":METADATA_CONFIG,"name":"TV3 fixture","enabled":true,"manifest_urls":[fixture.url("/tmdb/en-US/manifest.json")]},
        {"kind":"stremio","config_id":SECOND_METADATA,"name":"TV3 second fixture","enabled":true,"manifest_urls":[fixture.url("/tmdb/second/en-US/manifest.json")]}]);
    ok(&host, "save_settings", settings);
    let provider = sync(&host);
    for id in [METADATA_CONFIG, SECOND_METADATA] {
        ok(
            &host,
            "sync_catalog",
            json!({"kind":"stremio","config_id":id}),
        );
    }
    ok(&host, "runtime_prepare", json!({"target":"application"}));
    // Persist a genuine Session-bound history row before opening the UI.
    let sources = ok(&host, "get_sources", json!({}));
    let metadata = sources["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["origin"]["config_id"] == METADATA_CONFIG)
        .unwrap();
    let first =
        ok(&host, "get_home", home_input(metadata))["page"]["sections"][5]["items"][0].clone();
    let details = ok(&host, "get_details", json!({"ref":first["ref"]}));
    let input = json!({"metadata_ref":details["metadata_ref"],"video_id":"video:0",
        "playback_source":{"source_id":provider["id"],"source_revision":provider["source_revision"]},"selection":null});
    let context = selected(&host, &provider, &input);
    let resolution = ok(&host, "resolve_episode", resolve_input(&context));
    let view = ok(&host, "open_playback", open_input(&resolution));
    let identity = &context["metadata_ref"]["identity"];
    let record = json!({"content_id":details["details"]["id"],"metadata_identity":identity,"video_id":"video:0",
        "season_number":1,"episode_number":3,"title":"Continue fixture","kind":"series","poster_cache_key":null,
        "source_id":provider["id"],"source_name":"Playback fixture","position_ms":180000,"duration_ms":900000,
        "completed":false,"updated_at":1});
    ok(
        &host,
        "save_history",
        json!({"record":record,"session_id":view["session_id"],"match_id":context["match_id"],"playback_epoch":1,"write_sequence":1}),
    );
    ok(
        &host,
        "close_playback",
        json!({"session_id":view["session_id"]}),
    );
    fixture.requests.lock().unwrap().clear();
    publish3(&directory.join("ready.json"), &json!({"ok":true}));
    let start = Instant::now();
    let mut sequence = 0;
    let mut homes = 0;
    loop {
        sequence += 1;
        let path = directory.join(format!("request-{sequence}.json"));
        while !path.exists() {
            assert!(start.elapsed() < Duration::from_secs(1800));
            thread::sleep(Duration::from_millis(5));
        }
        let request: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let result = match request["operation"].as_str().unwrap_or("invoke") {
            "finish" => {
                publish3(
                    &directory.join(format!("response-{sequence}.json")),
                    &json!({"ok":true}),
                );
                break;
            }
            "mode" => {
                mode.store(
                    request["value"].as_u64().unwrap() as usize,
                    Ordering::SeqCst,
                );
                json!({"ok":true})
            }
            "refresh" => ok(
                &host,
                "sync_catalog",
                json!({"kind":"stremio","config_id":request["config_id"]}),
            ),
            "builtin" => {
                let presets: Value =
                    serde_json::from_str(include_str!("../../src/data/defaultSources.json"))
                        .unwrap();
                let preset = presets
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["kind"] == "stremio")
                    .unwrap();
                json!({"preset":preset})
            }
            "trace" => {
                json!({"targets":fixture.requests.lock().unwrap().iter().map(|r| r.target.clone()).collect::<Vec<_>>() })
            }
            "invoke" => {
                let command = request["command"].as_str().unwrap();
                homes += usize::from(command == "get_home");
                assert!(
                    crate::contracts::COMMANDS.contains(&command),
                    "Unexpected bridge command: {command}"
                );
                let result = host.invoke(command, request["args"].clone(), true);
                Contracts::get()
                    .validate(&format!("Result_{command}"), &result)
                    .unwrap();
                result
            }
            other => panic!("Unsupported TV3 operation {other}"),
        };
        publish3(
            &directory.join(format!("response-{sequence}.json")),
            &result,
        );
    }
    println!(
        "TV3_VECTOR:{}",
        json!({"get_home":homes,"active_requests":host.requests.active_count(),
        "targets":fixture.requests.lock().unwrap().iter().map(|r| r.target.clone()).collect::<Vec<_>>() })
    );
}
