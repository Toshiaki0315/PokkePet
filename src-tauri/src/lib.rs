pub mod calls;
pub mod dex;
pub mod diary;
pub mod evolution;
pub mod expression;
pub mod game;
pub mod minigame;
pub mod service;
pub mod settings;
pub mod sleep;
pub mod status;

mod shell;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(shell::setup)
        .invoke_handler(tauri::generate_handler![shell::get_state, shell::perform_action, shell::ack_hatch, shell::set_name, shell::graduate, shell::play_minigame, shell::get_dex, shell::get_diary, shell::get_settings, shell::save_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
