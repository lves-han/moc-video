//! Compiled only by the explicit T01 acceptance build. No filesystem or process command API.
use tauri::{LogicalSize, Manager, WebviewWindow};

pub fn bootstrap() -> Result<String, Box<dyn std::error::Error>> {
    let base = std::env::var("T01_FIXTURE_BASE")?;
    let cap = std::env::var("T01_FIXTURE_CAP")?;
    let url: tauri::Url = base.parse()?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || cap.len() != 64
        || !cap.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("T01 requires a loopback fixture and a 256-bit hex capability".into());
    }
    let config = serde_json::json!({"base": base, "cap": cap});
    Ok(format!(
        "if(location.origin==='tauri://localhost'||location.origin==='http://tauri.localhost')Object.defineProperty(window,'__T01_BOOTSTRAP__',{{value:{config}}});"
    ))
}

#[tauri::command]
pub async fn t01_control(
    window: WebviewWindow,
    action: String,
    value: Option<f64>,
) -> Result<serde_json::Value, String> {
    // Fixed markers locate a stalled GUI/IPC call without logging its arguments.
    eprintln!("T01_IPC:entered");
    super::require_main(&window)?;
    eprintln!("T01_IPC:authorized");
    match action.as_str() {
        "identity" => {}
        "foreground" => {
            // The harness launches the executable directly, so explicitly activate
            // its app/window before measuring foreground WebView rendering.
            #[cfg(target_os = "macos")]
            window.app_handle().show().map_err(|e| e.to_string())?;
            eprintln!("T01_IPC:application-shown");
            window.unminimize().map_err(|e| e.to_string())?;
            eprintln!("T01_IPC:restored");
            window.show().map_err(|e| e.to_string())?;
            eprintln!("T01_IPC:window-shown");
            window.set_focus().map_err(|e| e.to_string())?;
            eprintln!("T01_IPC:window-focused");
            let webview: &tauri::Webview = window.as_ref();
            webview.set_focus().map_err(|e| e.to_string())?;
            eprintln!("T01_IPC:webview-focused");
        }
        "width" => {
            let width = value.ok_or("width required")?;
            if ![360.0, 390.0, 768.0, 960.0, 1200.0].contains(&width) {
                return Err("Unsupported acceptance width".into());
            }
            window
                .set_min_size(Some(LogicalSize::new(320.0, 480.0)))
                .map_err(|e| e.to_string())?;
            window
                .set_size(LogicalSize::new(width, 800.0))
                .map_err(|e| e.to_string())?;
        }
        "zoom" => {
            let zoom = value.ok_or("zoom required")?;
            if ![1.0, 1.25, 1.5, 2.0].contains(&zoom) {
                return Err("Unsupported acceptance zoom".into());
            }
            window.set_zoom(zoom).map_err(|e| e.to_string())?;
        }
        "quit" => {
            window.close().map_err(|e| e.to_string())?;
        }
        _ => return Err("Unknown acceptance action".into()),
    }
    if action == "quit" {
        return Ok(serde_json::json!({"closing": true}));
    }
    let size = window.inner_size().map_err(|e| e.to_string())?;
    eprintln!("T01_IPC:geometry-read");
    #[cfg(target_os = "macos")]
    let (webkit_pids, top_content_inset, audio_playing) = webkit_diagnostics(&window)?;
    #[cfg(not(target_os = "macos"))]
    let (webkit_pids, top_content_inset, audio_playing) = (serde_json::Value::Null, 0.0, false);
    eprintln!("T01_IPC:webkit-read");
    let result = serde_json::json!({
        "label": window.label(), "fullscreen": window.is_fullscreen().map_err(|e|e.to_string())?,
        "visible": window.is_visible().map_err(|e|e.to_string())?,
        "focused": window.is_focused().map_err(|e|e.to_string())?,
        "minimized": window.is_minimized().map_err(|e|e.to_string())?,
        "physical_width": size.width, "physical_height": size.height,
        "scale_factor": window.scale_factor().map_err(|e|e.to_string())?,
        "pid": std::process::id(), "tauri": tauri::VERSION, "webkit_pids": webkit_pids,
        "top_content_inset": top_content_inset, "audio_playing": audio_playing
    });
    eprintln!("T01_IPC:completed");
    Ok(result)
}

#[cfg(target_os = "macos")]
fn webkit_diagnostics(window: &WebviewWindow) -> Result<(serde_json::Value, f64, bool), String> {
    use std::ffi::{c_char, c_void};
    #[link(name = "objc")]
    unsafe extern "C" {
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    let (send, receive) = std::sync::mpsc::channel();
    window
        .with_webview(move |webview| {
            // Diagnostic SPI is isolated to the acceptance feature. XPC helpers can
            // be launchd children, so ancestry alone misses their RSS and lifecycle.
            let result = unsafe {
                let get: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let pid: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32 =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let scalar: unsafe extern "C" fn(*mut c_void, *mut c_void) -> f64 =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let responds: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let boolean: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let view = webview.inner().cast::<c_void>();
                let config = get(view, sel_registerName(c"configuration".as_ptr()));
                let store = get(config, sel_registerName(c"websiteDataStore".as_ptr()));
                let mut values = serde_json::Map::new();
                let mut failure = None;
                for (name, object, selector) in [
                    ("renderer", view, c"_webProcessIdentifier"),
                    ("gpu", view, c"_gpuProcessIdentifier"),
                    ("network", store, c"_networkProcessIdentifier"),
                ] {
                    let selector = sel_registerName(selector.as_ptr());
                    if object.is_null()
                        || !responds(
                            object,
                            sel_registerName(c"respondsToSelector:".as_ptr()),
                            selector,
                        )
                    {
                        failure = Some(format!("WebKit {name} process diagnostic unavailable"));
                        break;
                    }
                    values.insert(name.into(), serde_json::json!(pid(object, selector)));
                }
                if let Some(error) = failure {
                    Err(error)
                } else {
                    // WebKit subtracts its automatic title-bar inset from the CSS
                    // viewport; Tauri inner_size reports the whole WKWebView frame.
                    let selector = sel_registerName(c"_topContentInset".as_ptr());
                    if !responds(
                        view,
                        sel_registerName(c"respondsToSelector:".as_ptr()),
                        selector,
                    ) {
                        Err("WebKit top content inset diagnostic unavailable".into())
                    } else {
                        let inset = scalar(view, selector);
                        let audio_selector = sel_registerName(c"_isPlayingAudio".as_ptr());
                        if !responds(
                            view,
                            sel_registerName(c"respondsToSelector:".as_ptr()),
                            audio_selector,
                        ) {
                            Err("WebKit audio playback diagnostic unavailable".into())
                        } else if inset.is_finite() && inset >= 0.0 {
                            Ok((
                                serde_json::Value::Object(values),
                                inset,
                                boolean(view, audio_selector),
                            ))
                        } else {
                            Err("WebKit top content inset is invalid".into())
                        }
                    }
                }
            };
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?
}
