pub mod calls;
pub mod card;
pub mod care;
pub mod companion;
pub mod debuglog;
pub mod dex;
pub mod daylog;
pub mod diary;
pub mod events;
pub mod evolution;
pub mod expression;
pub mod game;
pub mod genome;
pub mod minigame;
pub mod paths;
pub mod rhythm;
pub mod seasons;
pub mod service;
pub mod settings;
pub mod souvenirs;
pub mod sleep;
pub mod status;
pub mod system;
pub mod walk;

mod shell;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(shell::setup)
        .invoke_handler(tauri::generate_handler![shell::get_state, shell::perform_action, shell::stroke_pet, shell::bury_pet, shell::reveal_debug_log, shell::save_card, shell::get_egg_code, shell::receive_egg, shell::get_souvenirs, shell::toggle_souvenir, shell::ack_gift, shell::restart_game, shell::toggle_power, shell::ack_hatch, shell::set_name, shell::graduate, shell::play_minigame, shell::get_dex, shell::get_diary, shell::get_settings, shell::save_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
