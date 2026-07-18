mod capture;
mod commands;
mod model;
mod process;
mod protocol;

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
            commands::stop_capture
        ])
        .run(tauri::generate_context!())
        .expect("error while running PacketLens");
}
