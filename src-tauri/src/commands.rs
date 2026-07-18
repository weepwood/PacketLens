use tauri::{AppHandle, State};

use crate::{
    model::{
        CaptureRequest, CaptureSessionSummary, CaptureStatus, NetworkInterface,
        StoredPacketSummary,
    },
    AppState,
};

#[tauri::command]
pub fn list_interfaces() -> Result<Vec<NetworkInterface>, String> {
    crate::capture::list_interfaces()
}

#[tauri::command]
pub fn get_capture_status(state: State<'_, AppState>) -> CaptureStatus {
    state.capture.status()
}

#[tauri::command]
pub fn start_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CaptureRequest,
) -> Result<CaptureStatus, String> {
    state.capture.start(app, request)
}

#[tauri::command]
pub fn stop_capture(state: State<'_, AppState>) -> CaptureStatus {
    state.capture.stop()
}

#[tauri::command]
pub fn list_capture_sessions(
    app: AppHandle,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<CaptureSessionSummary>, String> {
    crate::storage::list_sessions(&app, limit.unwrap_or(100), offset.unwrap_or(0))
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_session_packets(
    app: AppHandle,
    session_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<StoredPacketSummary>, String> {
    crate::storage::list_session_packets(
        &app,
        &session_id,
        limit.unwrap_or(250),
        offset.unwrap_or(0),
    )
}

#[tauri::command(rename_all = "camelCase")]
pub fn delete_capture_session(app: AppHandle, session_id: String) -> Result<(), String> {
    crate::storage::delete_session(&app, &session_id)
}
