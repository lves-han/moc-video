#[cfg(not(mobile))]
use tauri::WebviewWindow;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub mod commands;
pub mod contracts;
pub mod details;
pub mod gateway;
pub mod history;
pub mod home;
pub mod host;
pub mod matching;
pub mod media_cache;
pub mod requests;
pub mod resolution;
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
fn player_fullscreen(window: WebviewWindow) -> Result<bool, String> {
    require_main(&window)?;
    let target = !window.is_fullscreen().map_err(|e| e.to_string())?;
    window.set_fullscreen(target).map_err(|e| e.to_string())?;
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
    let builder = tauri::Builder::default();
    #[cfg(all(not(mobile), feature = "t01-automation"))]
    let prototype: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        player_fullscreen,
        app_close_started,
        app_close_aborted,
        app_close_ready,
        window_bounds_now,
        automation::t01_control
    ];
    #[cfg(all(not(mobile), not(feature = "t01-automation")))]
    let prototype: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        player_fullscreen,
        app_close_started,
        app_close_aborted,
        app_close_ready,
        window_bounds_now
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
        builder.on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let (allow, signal) = closing::request(&closing::STATE);
                if allow {
                    return;
                }
                api.prevent_close();
                if signal && let Some(view) = window.app_handle().get_webview_window(window.label())
                {
                    let _ = view.eval("window.dispatchEvent(new Event('yingxiang-closing'))");
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
            let settings_path = app.path().app_data_dir()?.join("settings.json");
            let host = host::Host::open(settings_path)
                .map_err(|error| std::io::Error::other(error.message))?;
            #[cfg(not(mobile))]
            let bounds = host.window_bounds();
            app.manage(host);
            #[cfg(all(not(mobile), feature = "t01-automation"))]
            let entry = "index.html#/prototype";
            #[cfg(any(mobile, not(feature = "t01-automation")))]
            let entry = "index.html#/home";
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(entry.into()))
                .title("Yingxiang Video")
                .on_navigation(|url| {
                    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
                });
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
        .expect("映象视频 Tauri host failed");
}
