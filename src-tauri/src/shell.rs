//! Tauri との接着層（トレイ常駐・ポップアップ制御・IPC コマンド・バックグラウンドループ）。
//! ロジックは持たず、`service` に委譲する。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent};

use crate::game::GameState;
use crate::service::{GameService, StateView};

const LOOP_INTERVAL: Duration = Duration::from_secs(10);
const STATE_EVENT: &str = "state-changed";

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
        WindowEvent::Focused(false) => {
            let _ = w.hide();
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = w.hide();
        }
        _ => {}
    });
    Ok(())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit])?;
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
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

fn toggle_popup(window: &WebviewWindow, anchor: PhysicalPosition<i32>, icon_height: u32) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    let width = window.outer_size().map_or(0, |s| s.width) as i32;
    let x = anchor.x - width / 2;
    let y = anchor.y + icon_height as i32;
    let _ = window.set_position(PhysicalPosition::new(x.max(0), y));
    let _ = window.show();
    let _ = window.set_focus();
}

fn spawn_loop(handle: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(LOOP_INTERVAL);
        let svc = handle.state::<GameService>();
        match svc.tick(now_unix()) {
            Ok((view, _evolutions)) => {
                let _ = handle.emit(STATE_EVENT, view);
            }
            Err(e) => eprintln!("tick failed: {e}"),
        }
    });
}
