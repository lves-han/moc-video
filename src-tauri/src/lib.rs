#[cfg(not(mobile))]
use tauri::WebviewWindow;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub mod commands;
pub mod contracts;
pub mod details;
pub mod gateway;
pub mod home;
pub mod host;
pub mod matching;
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

#[cfg(all(not(mobile), feature = "t01-automation"))]
mod automation;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(all(not(mobile), feature = "t01-automation"))]
    let prototype: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![player_fullscreen, automation::t01_control];
    #[cfg(all(not(mobile), not(feature = "t01-automation")))]
    let prototype: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![player_fullscreen];
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

    builder
        .setup(|app| {
            let settings_path = app.path().app_data_dir()?.join("settings.json");
            app.manage(
                host::Host::open(settings_path)
                    .map_err(|error| std::io::Error::other(error.message))?,
            );
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
                .inner_size(1200.0, 800.0)
                .min_inner_size(960.0, 640.0);
            #[cfg(all(not(mobile), feature = "t01-automation"))]
            let window = window.initialization_script(automation::bootstrap()?);
            window.build()?;
            // Fail at startup if configuration accidentally creates a second player window.
            if app.webview_windows().len() != 1 {
                return Err("Expected exactly one embedded player window".into());
            }
            Ok(())
        })
        .run(tauri::tauri_build_context!())
        .expect("映象视频 Tauri host failed");
}
