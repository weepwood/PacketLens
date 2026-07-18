mod capture;
mod commands;
mod model;
mod process;
mod protocol;
mod storage;

use capture::CaptureService;

#[derive(Default)]
pub struct AppState {
    capture: CaptureService,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_interfaces,
            commands::get_capture_status,
            commands::start_capture,
            commands::stop_capture,
            commands::list_capture_sessions,
            commands::list_session_packets,
            commands::delete_capture_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running PacketLens");
}
