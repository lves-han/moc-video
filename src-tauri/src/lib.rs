#[cfg(not(mobile))]
use tauri::WebviewWindow;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub mod commands;
pub mod contracts;
pub mod details;
pub mod fuzzy;
pub mod gateway;
pub mod history;
pub mod home;
pub mod host;
pub mod line_round;
pub mod matching;
pub mod media_cache;
pub mod prefetch;
pub mod remux;
pub mod requests;
#[cfg(not(mobile))]
mod reset;
pub mod resolution;
pub mod service;
pub mod settings;
pub mod source_dispatch;

#[cfg(not(mobile))]
fn require_main(window: &WebviewWindow) -> Result<(), String> {
    let url = window.url().map_err(|e| e.to_string())?;
    if contracts::authorized(window.label(), &url) {
        Ok(())
    } else {
        Err("Player command requires the packaged main WebView".into())
    }
}

#[cfg(not(mobile))]
#[tauri::command]
fn app_reset(window: WebviewWindow) -> Result<(), String> {
    require_main(&window)?;
    tauri::process::current_binary(&window.app_handle().env())
        .and_then(|path| reset::check_relaunch_path(&path))
        .map_err(|e| format!("App restart is unavailable: {e}"))?;
    let plan = reset::ResetPlan::for_app(window.app_handle())?;
    let lock = plan
        .schedule(&window.state::<reset::InstanceGuard>())
        .map_err(|e| format!("Could not schedule app reset: {e}"))?;
    static RESET_LOCK: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();
    RESET_LOCK
        .set(lock)
        .map_err(|_| "App reset is already in progress".to_owned())?;
    closing::STATE.store(closing::RELEASED, std::sync::atomic::Ordering::SeqCst);
    window.app_handle().restart();
}

#[cfg(not(mobile))]
#[tauri::command]
async fn app_cache(window: WebviewWindow, host: tauri::State<'_, std::sync::Arc<host::Host>>, action: String, store: String, key: String, row: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    require_main(&window)?;
    let host = host.inner().clone();
    tauri::async_runtime::spawn_blocking(move || host.web_cache(&action, &store, &key, row))
        .await.map_err(|_| "Cache worker failed".to_owned())?
        .map_err(|error| error.message)
}

#[cfg(not(mobile))]
#[tauri::command]
fn app_reset_recovery(window: WebviewWindow, action: String) -> Result<(), String> {
    require_main(&window)?;
    if window.try_state::<reset::RecoveryState>().is_none() {
        return Err("App is not in reset recovery".into());
    }
    if !matches!(action.as_str(), "retry" | "cancel") {
        return Err("Unknown reset recovery action".into());
    }
    tauri::process::current_binary(&window.app_handle().env())
        .and_then(|path| reset::check_relaunch_path(&path))
        .map_err(|e| format!("App restart is unavailable: {e}"))?;
    if action == "cancel" {
        reset::ResetPlan::for_app(window.app_handle())?
            .cancel_pending(&window.state::<reset::InstanceGuard>())
            .map_err(|e| e.to_string())?;
    }
    window.app_handle().restart();
}

#[cfg(not(mobile))]
#[tauri::command(rename_all = "snake_case")]
async fn app_service_import(
    window: WebviewWindow,
    url: String,
    token: String,
    configs: Vec<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    require_main(&window)?;
    if configs.len() > 32
        || serde_json::to_vec(&configs).map_or(true, |bytes| bytes.len() > 1024 * 1024)
    {
        return Err("Metadata source configuration is too large".into());
    }
    service::import(&url, &token, &configs)
        .await
        .map_err(|error| error.message)
}

#[cfg(not(mobile))]
#[tauri::command(rename_all = "snake_case")]
async fn app_service_delete(
    window: WebviewWindow,
    url: String,
    token: String,
    ids: Vec<String>,
) -> Result<(), String> {
    require_main(&window)?;
    if ids.len() > 16 {
        return Err("Too many metadata sources".into());
    }
    service::delete(&url, &token, &ids)
        .await
        .map_err(|error| error.message)
}

#[cfg(not(mobile))]
#[tauri::command(rename_all = "snake_case")]
async fn app_playback_sources(
    window: WebviewWindow,
    action: String,
    id: Option<String>,
    payload: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    require_main(&window)?;
    service::playback_sources(&action, id.as_deref(), payload)
        .await
        .map_err(|error| error.message)
}

#[cfg(not(mobile))]
#[tauri::command(rename_all = "snake_case")]
async fn app_probe_availability(
    window: WebviewWindow,
    metadata_source_id: String,
    kind: String,
    meta_id: String,
    video_id: String,
    lang: String,
) -> Result<String, String> {
    require_main(&window)?;
    if !matches!(kind.as_str(), "movie" | "series")
        || !matches!(lang.as_str(), "zh-CN" | "en-US")
        || [&metadata_source_id, &meta_id, &video_id].iter().any(|s| s.is_empty() || s.len() > 512 || s.chars().any(char::is_control))
    { return Err("Invalid playback target".into()); }
    static GATE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let _permit = GATE.acquire().await.map_err(|_| "Availability queue closed")?;
    let service = service::current().map_err(|e| e.message)?;
    let body = serde_json::json!({"metadata_source_id":metadata_source_id,"type":kind,"meta_id":meta_id,"video_id":video_id,"lang":lang});
    let mut lookup = None::<String>;
    let mut sources = std::collections::HashMap::<String, String>::new();
    let mut available = false;
    let mut seen_line = false;
    let mut ambiguous = false;
    let result = service.events(&["v1", "lookups"], body.clone(), |event| {
        match event["event"].as_str() {
            Some("started") => {
                lookup = event["lookup_id"].as_str().filter(|id| uuid::Uuid::parse_str(id).is_ok()).map(str::to_owned);
                for source in event["sources"].as_array().into_iter().flatten() {
                    if let Some(id) = source["source_id"].as_str() { sources.insert(id.into(), "pending".into()); }
                }
                if lookup.is_none() { ambiguous = true; }
            }
            Some("source") => {
                if let (Some(id), Some(status)) = (event["source_id"].as_str(), event["status"].as_str()) {
                    if let Some(value) = sources.get_mut(id) { *value = status.into(); } else { ambiguous = true; }
                } else { ambiguous = true; }
            }
            Some("line") => {
                seen_line = true;
                let line = &event["line"];
                let target = &line["target"];
                if ["metadata_source_id", "type", "meta_id", "video_id"].iter().any(|field| target[field] != body[field]) { ambiguous = true; }
                else if matches!(line["kind"].as_str(), Some("file" | "hls")) && line["availability"] == "ready" { available = true; }
                else if !matches!(line["availability"].as_str(), Some("failed" | "unsupported" | "ready")) { ambiguous = true; }
            }
            Some("confirm") => ambiguous = true,
            Some("done") => {},
            _ => ambiguous = true,
        }
        Ok(())
    }, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))).await;
    if let Some(id) = lookup { let _ = service.cancel_lookup(&id).await; }
    if service::current().map(|current| current.fingerprint()).ok().as_deref() != Some(service.fingerprint().as_str()) { return Err("Playback service changed".into()); }
    if !available { result.map_err(|e| e.message)?; }
    Ok(card_probe_outcome(available, ambiguous, seen_line, &sources).into())
}

#[cfg(not(mobile))]
fn card_probe_outcome(available: bool, ambiguous: bool, seen_line: bool, sources: &std::collections::HashMap<String, String>) -> &'static str {
    if available { "available" }
    else if ambiguous || sources.values().any(|s| !matches!(s.as_str(), "done" | "disabled" | "not_found" | "no_episode" | "unsupported")) { "unknown" }
    else if !seen_line && sources.values().all(|s| matches!(s.as_str(), "not_found" | "disabled")) { "unavailable_title" }
    else { "unavailable" }
}

#[cfg(all(test, not(mobile)))]
mod card_probe_tests {
    use super::card_probe_outcome;
    use std::collections::HashMap;
    #[test]
    fn failed_and_ambiguous_sources_do_not_claim_unavailability() {
        let states = |status: &str| HashMap::from([("source".to_owned(), status.to_owned())]);
        assert_eq!(card_probe_outcome(false, false, false, &states("timeout")), "unknown");
        assert_eq!(card_probe_outcome(false, true, false, &states("not_found")), "unknown");
        assert_eq!(card_probe_outcome(false, false, false, &states("not_found")), "unavailable_title");
        assert_eq!(card_probe_outcome(false, false, true, &states("done")), "unavailable");
        assert_eq!(card_probe_outcome(true, true, true, &states("failed")), "available");
    }
}

#[cfg(not(mobile))]
#[tauri::command]
async fn player_fullscreen(
    window: WebviewWindow,
    target: Option<bool>,
    query: Option<bool>,
) -> Result<bool, String> {
    require_main(&window)?;
    let current = window.is_fullscreen().map_err(|e| e.to_string())?;
    if query == Some(true) {
        return Ok(current);
    }
    let target = target.unwrap_or(!current);
    window.set_fullscreen(target).map_err(|e| e.to_string())?;
    // macOS completes the window animation after accepting the request.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while window.is_fullscreen().map_err(|e| e.to_string())? != target {
        if std::time::Instant::now() >= deadline {
            return Err("Native fullscreen transition did not finish".into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Ok(target)
}

/// Window close with saving first. Every close request waits for the same save; only the page's
/// release, the user's explicit "quit anyway", or a page that never answers lets the window go.
#[cfg(not(mobile))]
pub(crate) mod closing {
    use std::sync::atomic::{AtomicU8, Ordering::SeqCst};
    pub const IDLE: u8 = 0;
    pub const SIGNALLED: u8 = 1;
    pub const SAVING: u8 = 2;
    pub const RELEASED: u8 = 3;
    pub static STATE: AtomicU8 = AtomicU8::new(IDLE);

    /// (let the window go, ask the page to save)
    pub fn request(state: &AtomicU8) -> (bool, bool) {
        match state.compare_exchange(IDLE, SIGNALLED, SeqCst, SeqCst) {
            Ok(_) => (false, true),
            Err(RELEASED) => (true, false),
            Err(_) => (false, false),
        }
    }

    /// True when the page never answered the signal, so the timer may release the window.
    pub fn unanswered(state: &AtomicU8) -> bool {
        state
            .compare_exchange(SIGNALLED, RELEASED, SeqCst, SeqCst)
            .is_ok()
    }
}

#[cfg(not(mobile))]
#[tauri::command]
fn app_close_started(window: WebviewWindow) -> Result<(), String> {
    require_main(&window)?;
    closing::STATE.store(closing::SAVING, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// Saving failed and the window stays; the next close request starts over.
#[cfg(not(mobile))]
#[tauri::command]
fn app_close_aborted(window: WebviewWindow) -> Result<(), String> {
    require_main(&window)?;
    closing::STATE.store(closing::IDLE, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// Everything was saved, or the user chose to quit without it.
#[cfg(not(mobile))]
#[tauri::command]
fn app_close_ready(window: WebviewWindow) -> Result<(), String> {
    require_main(&window)?;
    closing::STATE.store(closing::RELEASED, std::sync::atomic::Ordering::SeqCst);
    window.destroy().map_err(|e| e.to_string())
}

/// Native logical inner size and outer position, the same measures used to restore the window.
#[cfg(not(mobile))]
#[tauri::command]
fn window_bounds_now(window: WebviewWindow) -> Result<serde_json::Value, String> {
    require_main(&window)?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    let position = window
        .outer_position()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    if window.is_fullscreen().map_err(|e| e.to_string())?
        || size.width < 960.0
        || size.height < 640.0
    {
        return Ok(serde_json::Value::Null);
    }
    Ok(
        serde_json::json!({"x": position.x.round() as i64, "y": position.y.round() as i64,
        "width": size.width.round() as i64, "height": size.height.round() as i64,
        "maximized": window.is_maximized().map_err(|e| e.to_string())?}),
    )
}

#[cfg(all(not(mobile), feature = "t01-automation"))]
mod automation;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());
    #[cfg(all(not(mobile), feature = "t01-automation"))]
    let prototype: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        player_fullscreen,
        app_close_started,
        app_close_aborted,
        app_close_ready,
        window_bounds_now,
        app_reset,
        app_reset_recovery,
        app_service_import,
        app_service_delete,
        app_playback_sources,
        app_probe_availability,
        app_cache,
        automation::t01_control
    ];
    #[cfg(all(not(mobile), not(feature = "t01-automation")))]
    let prototype: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        player_fullscreen,
        app_close_started,
        app_close_aborted,
        app_close_ready,
        window_bounds_now,
        app_reset,
        app_reset_recovery,
        app_service_import,
        app_service_delete,
        app_playback_sources,
        app_probe_availability,
        app_cache
    ];
    #[cfg(mobile)]
    let prototype: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = |_| false;
    let business = commands::handler::<tauri::Wry>();
    let builder = builder.invoke_handler(move |invoke| {
        if contracts::COMMANDS.contains(&invoke.message.command()) {
            business(invoke)
        } else {
            #[cfg(all(not(mobile), feature = "t01-automation"))]
            if invoke.message.command() == "t01_control" {
                eprintln!("T01_IPC:dispatch");
            }
            prototype(invoke)
        }
    });

    #[cfg(not(mobile))]
    let builder = {
        #[cfg(target_os = "macos")]
        let overlay = std::sync::atomic::AtomicBool::new(true);
        builder.on_window_event(move |window, event| {
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::Resized(size) = event
                && window.label() == "main"
            {
                match window.scale_factor() {
                    Ok(scale) => {
                        let desktop = f64::from(size.width) / scale >= 960.0;
                        // Preserve the legacy native content inset below the
                        // desktop breakpoint; avoid recursive style changes.
                        if overlay.swap(desktop, std::sync::atomic::Ordering::Relaxed) != desktop {
                            let style = if desktop {
                                tauri::TitleBarStyle::Overlay
                            } else {
                                tauri::TitleBarStyle::Visible
                            };
                            if let Err(error) = window.set_title_bar_style(style) {
                                overlay.store(!desktop, std::sync::atomic::Ordering::Relaxed);
                                eprintln!("Window titlebar breakpoint failed: {error}");
                            }
                        }
                    }
                    Err(error) => eprintln!("Window scale query failed: {error}"),
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let (allow, signal) = closing::request(&closing::STATE);
                if allow {
                    return;
                }
                api.prevent_close();
                if signal && let Some(view) = window.app_handle().get_webview_window(window.label())
                {
                    let _ = view.eval("window.dispatchEvent(new Event('moc-closing'))");
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        if closing::unanswered(&closing::STATE) {
                            let _ = view.destroy();
                        }
                    });
                }
            }
        })
    };
    builder
        .setup(|app| {
            #[cfg(not(mobile))]
            {
                let plan = reset::ResetPlan::for_app(app)?;
                let instance = match plan.acquire_instance(std::time::Duration::from_secs(15)) {
                    Ok(guard) => guard,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        app.handle().exit(0);
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                };
                let cleanup = plan.finish_startup(&instance);
                app.manage(instance);
                if let Err(error) = cleanup {
                    app.manage(reset::RecoveryState);
                    closing::STATE.store(closing::RELEASED, std::sync::atomic::Ordering::SeqCst);
                    let recovery = serde_json::json!({"message":error.to_string()});
                    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html#/reset-recovery".into()))
                        .title("Moc Video · Reset recovery")
                        .inner_size(760.0, 560.0)
                        .data_directory(plan.recovery_profile())
                        .incognito(true)
                        .on_navigation(|url| (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                            || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost")))
                        .initialization_script(format!("Object.defineProperty(window,'__MOC_RESET_RECOVERY__',{{value:{recovery}}});"))
                        .build()?;
                    return Ok(());
                }
            }
            let data_path = app.path().app_data_dir()?;
            #[cfg(not(mobile))]
            reset::clear_runtime_cache(&app.path().app_cache_dir()?, &data_path)?;
            let settings_path = data_path.join("settings.json");
            let host = host::Host::open(settings_path)
                .map_err(|error| std::io::Error::other(error.message))?;
            // The ordinary entry queries runtime_status before mounting. Prepare
            // its local gateway first, just as the acceptance bridge does.
            #[cfg(not(mobile))]
            {
                let runtime = host.invoke(
                    "runtime_prepare",
                    serde_json::json!({"request_id": uuid::Uuid::new_v4().to_string(),
                        "input": {"target": "application"}}),
                    true,
                );
                if runtime["data"]["phase"] != "ready" {
                    return Err(std::io::Error::other(format!(
                        "Application runtime preparation failed: {runtime}"
                    ))
                    .into());
                }
            }
            #[cfg(not(mobile))]
            let bounds = host.window_bounds();
            app.manage(host);
            #[cfg(all(not(mobile), feature = "t01-automation"))]
            let entry = "index.html#/prototype";
            #[cfg(any(mobile, not(feature = "t01-automation")))]
            let entry = "index.html#/home";
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(entry.into()))
                .title("Moc Video")
                .on_navigation(|url| {
                    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
                });
            #[cfg(target_os = "macos")]
            let window = window
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true);
            #[cfg(not(mobile))]
            let window = window
                .inner_size(
                    bounds.map_or(1200.0, |bounds| bounds.width),
                    bounds.map_or(800.0, |bounds| bounds.height),
                )
                .min_inner_size(960.0, 640.0)
                .maximized(bounds.is_some_and(|bounds| bounds.maximized));
            #[cfg(not(mobile))]
            let window = match bounds {
                Some(bounds) => window.position(bounds.x, bounds.y),
                None => window,
            };
            #[cfg(all(not(mobile), feature = "t01-automation"))]
            let window = window.initialization_script(automation::bootstrap()?);
            let built = window.build()?;
            // A display may have gone since the bounds were saved; never reopen out of reach.
            #[cfg(not(mobile))]
            if let Some(bounds) = bounds {
                let reachable = built.available_monitors()?.iter().any(|monitor| {
                    let scale = monitor.scale_factor();
                    let (left, top) = (
                        f64::from(monitor.position().x) / scale,
                        f64::from(monitor.position().y) / scale,
                    );
                    let (width, height) = (
                        f64::from(monitor.size().width) / scale,
                        f64::from(monitor.size().height) / scale,
                    );
                    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + 20.0);
                    x >= left && x < left + width && y >= top && y < top + height
                });
                if !reachable {
                    built.center()?;
                }
            }
            #[cfg(mobile)]
            let _ = built;
            // Fail at startup if configuration accidentally creates a second player window.
            if app.webview_windows().len() != 1 {
                return Err("Expected exactly one embedded player window".into());
            }
            Ok(())
        })
        .run(tauri::tauri_build_context!())
        .expect("墨客视频 Tauri host failed");
}
