use crate::{
    contracts::{ApiError, authorized},
    host::Host,
};
use serde_json::Value;
use std::sync::Arc;
use tauri::{
    Runtime, State, WebviewWindow,
    ipc::{InvokeBody, Request},
};

macro_rules! business_commands {
    ($($name:ident),+ $(,)?) => { $(
        #[tauri::command(rename_all = "snake_case")]
        pub async fn $name<R: Runtime>(window: WebviewWindow<R>, state: State<'_, Arc<Host>>, request: Request<'_>) -> Result<Value, ApiError> {
            let host = state.inner().clone();
            let args = match request.body() { InvokeBody::Json(args) => args.clone(), InvokeBody::Raw(_) => Value::Null };
            let allowed = window.url().is_ok_and(|url| authorized(window.label(), &url));
            let call = match host.admit(stringify!($name), args.clone(), allowed) {
                Ok(call) => call, Err(response) => return Ok(response),
            };
            let worker = host.clone();
            Ok(tauri::async_runtime::spawn_blocking(move || worker.execute(call)).await
                .unwrap_or_else(|_| host.error(&args, ApiError::internal("Host command worker failed"))))
        }
    )+ };
}

business_commands!(
    runtime_status,
    runtime_prepare,
    get_sources,
    sync_catalog,
    get_home,
    get_details,
    resolve_episode,
    open_playback,
    close_playback,
    start_interaction,
    finish_interaction,
    get_settings,
    save_settings,
    get_resource_status,
    release_resolution,
    match_playback
);

// Cancellation stays on the immediate IPC control path even when ordinary
// commands are waiting for the settings commit lock or the blocking executor.
#[tauri::command(rename_all = "snake_case")]
pub fn cancel_request<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, Arc<Host>>,
    request: Request<'_>,
) -> Value {
    let args = match request.body() {
        InvokeBody::Json(args) => args.clone(),
        InvokeBody::Raw(_) => Value::Null,
    };
    let allowed = window
        .url()
        .is_ok_and(|url| authorized(window.label(), &url));
    state.invoke("cancel_request", args, allowed)
}

pub fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        runtime_status,
        runtime_prepare,
        get_sources,
        sync_catalog,
        get_home,
        get_details,
        resolve_episode,
        open_playback,
        close_playback,
        cancel_request,
        start_interaction,
        finish_interaction,
        get_settings,
        save_settings,
        get_resource_status,
        release_resolution,
        match_playback
    ]
}
