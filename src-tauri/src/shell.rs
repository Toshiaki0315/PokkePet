//! Tauri との接着層（トレイ常駐・ポップアップ制御・IPC コマンド・バックグラウンドループ）。
//! ロジックは持たず、`service` に委譲する。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem};
use tauri_plugin_notification::NotificationExt;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::game::GameState;
use crate::service::{should_notify_hatch, GameService, StateView};
use crate::settings::{Settings, SettingsService};

const LOOP_INTERVAL: Duration = Duration::from_secs(10);
const STATE_EVENT: &str = "state-changed";
const SETTINGS_EVENT: &str = "settings-changed";
const SETTINGS_WINDOW: &str = "settings";
/// ポップアップの表示/非表示（true=表示）。孵化の演出など「開いている間だけ」再生するものに使う。
const POPUP_EVENT: &str = "popup-visibility";

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[tauri::command]
pub fn get_state(svc: State<GameService>) -> StateView {
    svc.snapshot()
}

#[tauri::command]
pub fn perform_action(svc: State<GameService>, action: String) -> Result<StateView, String> {
    let action = action.parse()?;
    svc.perform(action, now_unix()).map_err(|e| e.to_string())
}

/// 孵化の演出を見せ終えたことを記録する。
#[tauri::command]
pub fn ack_hatch(svc: State<GameService>) -> Result<StateView, String> {
    svc.ack_hatch().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(svc: State<SettingsService>) -> Settings {
    svc.get()
}

/// 設定を検証・保存し、すべてのウィンドウへ `settings-changed` を通知する。
#[tauri::command]
pub fn save_settings(app: AppHandle, svc: State<SettingsService>, settings: Settings) -> Result<Settings, String> {
    let saved = svc.update(settings)?;
    let _ = app.emit(SETTINGS_EVENT, saved.clone());
    Ok(saved)
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("save.json");
    let state = GameState::load(&path, now_unix()).unwrap_or_else(|e| {
        eprintln!("failed to load save, starting fresh: {e}");
        GameState::new(now_unix())
    });
    // 動作確認用: POKKEPET_TIME_SCALE=60 で時間経過が60倍になる。
    let scale = std::env::var("POKKEPET_TIME_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    app.manage(GameService::new(state, path).with_time_scale(scale));

    let settings_path = dir.join("settings.json");
    let settings = Settings::load(&settings_path).unwrap_or_else(|e| {
        eprintln!("failed to load settings, using defaults: {e}");
        Settings::default()
    });
    app.manage(SettingsService::new(settings, settings_path));

    setup_popup(app)?;
    setup_tray(app)?;
    spawn_loop(app.handle().clone());
    Ok(())
}

fn setup_popup(app: &tauri::App) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let w = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => hide_popup(&w),
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide_popup(&w);
        }
        _ => {}
    });
    Ok(())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "設定…", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "settings" => open_settings(app),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event
            else {
                return;
            };
            let Some(window) = tray.app_handle().get_webview_window("main") else {
                return;
            };
            toggle_popup(&window, rect.position.to_physical(window.scale_factor().unwrap_or(1.0)), rect.size.to_physical::<u32>(window.scale_factor().unwrap_or(1.0)).height);
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// 設定ウィンドウを開く（すでにあれば前面に出す）。
fn open_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, SETTINGS_WINDOW, WebviewUrl::App("index.html".into()))
        .title("PokkePet 設定")
        .inner_size(380.0, 470.0)
        .resizable(false)
        .center()
        .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("failed to open settings window: {e}"),
    }
}

fn hide_popup(window: &WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit(POPUP_EVENT, false);
}

fn toggle_popup(window: &WebviewWindow, anchor: PhysicalPosition<i32>, icon_height: u32) {
    if window.is_visible().unwrap_or(false) {
        hide_popup(window);
        return;
    }
    let width = window.outer_size().map_or(0, |s| s.width) as i32;
    let x = anchor.x - width / 2;
    let y = anchor.y + icon_height as i32;
    let _ = window.set_position(PhysicalPosition::new(x.max(0), y));
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit(POPUP_EVENT, true);
}

fn popup_is_open(app: &AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

/// 孵化をmacOSの通知で知らせる（ポップアップを開いていないとき）。
fn notify_hatch(app: &AppHandle) {
    let shown = app.notification().builder().title("PokkePet").body("卵が孵ったよ！会いにきてね").show();
    if let Err(e) = shown {
        eprintln!("failed to show notification: {e}");
    }
}

fn spawn_loop(handle: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(LOOP_INTERVAL);
        let svc = handle.state::<GameService>();
        match svc.tick(now_unix()) {
            Ok(tick) => {
                let _ = handle.emit(STATE_EVENT, tick.view);
                if should_notify_hatch(tick.hatched, popup_is_open(&handle)) {
                    notify_hatch(&handle);
                }
            }
            Err(e) => eprintln!("tick failed: {e}"),
        }
    });
}
