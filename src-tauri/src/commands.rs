use tauri::{AppHandle, State};

use crate::{
    model::{CaptureRequest, CaptureStatus, NetworkInterface},
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
