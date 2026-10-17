//! Tauri との接着層（トレイ常駐・ポップアップ制御・IPC コマンド・バックグラウンドループ）。
//! ロジックは持たず、`service` に委譲する。

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem};
use tauri_plugin_notification::NotificationExt;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::calls::{Call, CallGate};
use crate::dex::DexView;
use crate::diary::DiaryView;
use crate::game::GameState;
use crate::minigame::Side;
use crate::paths::data_dir;
use crate::service::{should_notify_graduation, should_notify_hatch, tray_title, GameService, MiniGameReport, StateView};
use crate::settings::{Settings, SettingsService};

const LOOP_INTERVAL: Duration = Duration::from_secs(10);
const STATE_EVENT: &str = "state-changed";
const SETTINGS_EVENT: &str = "settings-changed";
const SETTINGS_WINDOW: &str = "settings";
const DEX_WINDOW: &str = "dex";
const DIARY_WINDOW: &str = "diary";
/// ポップアップの表示/非表示（true=表示）。孵化の演出など「開いている間だけ」再生するものに使う。
const POPUP_EVENT: &str = "popup-visibility";
const TRAY_ID: &str = "pokkepet";

/// いま表示しているメニューバーの目印（変わったときだけ更新する）。
struct TrayMark(Mutex<Option<&'static str>>);

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[tauri::command]
pub fn get_state(svc: State<GameService>) -> StateView {
    svc.snapshot()
}

#[tauri::command]
pub fn perform_action(app: AppHandle, svc: State<GameService>, gate: State<Mutex<CallGate>>, action: String) -> Result<StateView, String> {
    let action = action.parse()?;
    let view = svc.perform(action, now_unix()).map_err(|e| e.to_string())?;
    gate.lock().unwrap_or_else(|e| e.into_inner()).answered(); // 世話をしたので、次の困りごとはすぐ呼べる
    update_tray_title(&app, &view);
    Ok(view)
}

/// ミニゲーム「どっちの手？」。`guess` は "left" か "right"。
#[tauri::command]
pub fn play_minigame(app: AppHandle, svc: State<GameService>, gate: State<Mutex<CallGate>>, guess: String) -> Result<MiniGameReport, String> {
    let side: Side = guess.parse()?;
    let report = svc.play_minigame(side, now_unix())?;
    gate.lock().unwrap_or_else(|e| e.into_inner()).answered(); // 遊んだので、次の困りごとはすぐ呼べる
    update_tray_title(&app, &report.view);
    Ok(report)
}

/// 旅立ちを見送り、次の世代の卵を迎える。すべてのウィンドウへ最新の状態を通知する。
#[tauri::command]
pub fn graduate(app: AppHandle, svc: State<GameService>, gate: State<Mutex<CallGate>>) -> Result<StateView, String> {
    let view = svc.graduate()?;
    gate.lock().unwrap_or_else(|e| e.into_inner()).answered();
    update_tray_title(&app, &view);
    let _ = app.emit(STATE_EVENT, view.clone());
    Ok(view)
}

/// 名前を付ける（付け直す）。すべてのウィンドウへ最新の状態を通知する。
#[tauri::command]
pub fn set_name(app: AppHandle, svc: State<GameService>, name: String) -> Result<StateView, String> {
    let view = svc.set_name(&name)?;
    let _ = app.emit(STATE_EVENT, view.clone());
    Ok(view)
}

/// 孵化の演出を見せ終えたことを記録する。
#[tauri::command]
pub fn ack_hatch(svc: State<GameService>) -> Result<StateView, String> {
    svc.ack_hatch().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_dex(svc: State<GameService>) -> DexView {
    svc.dex()
}

#[tauri::command]
pub fn get_diary(svc: State<GameService>) -> DiaryView {
    svc.diary()
}

#[tauri::command]
pub fn get_settings(svc: State<SettingsService>) -> Settings {
    svc.get()
}

/// 設定を検証・保存し、すべてのウィンドウへ `settings-changed` を通知する。
#[tauri::command]
pub fn save_settings(app: AppHandle, svc: State<SettingsService>, game: State<GameService>, settings: Settings) -> Result<Settings, String> {
    let saved = svc.update(settings)?;
    game.set_sleep_window(saved.sleep_window()); // おやすみ時間の変更をすぐ反映する
    let _ = app.emit(SETTINGS_EVENT, saved.clone());
    let _ = app.emit(STATE_EVENT, game.snapshot());
    Ok(saved)
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    // 開発版は別フォルダ（インストールしたアプリのセーブを上書き・流用しない）。
    let dir = data_dir(&app.path().app_data_dir()?, cfg!(debug_assertions));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("save.json");
    let state = GameState::load(&path, now_unix()).unwrap_or_else(|e| {
        eprintln!("failed to load save, starting fresh: {e}");
        GameState::new(now_unix())
    });
    // 動作確認用: POKKEPET_TIME_SCALE=60 で時間経過が60倍になる。
    let scale = std::env::var("POKKEPET_TIME_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    app.manage(GameService::new(state, path).with_time_scale(scale));
    app.manage(Mutex::new(CallGate::new()));
    app.manage(TrayMark(Mutex::new(None)));

    let settings_path = dir.join("settings.json");
    let settings = Settings::load(&settings_path).unwrap_or_else(|e| {
        eprintln!("failed to load settings, using defaults: {e}");
        Settings::default()
    });
    app.state::<GameService>().set_sleep_window(settings.sleep_window());
    app.manage(SettingsService::new(settings, settings_path));

    setup_popup(app)?;
    setup_tray(app)?;
    let view = app.state::<GameService>().snapshot();
    update_tray_title(app.handle(), &view);
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
    let dex = MenuItem::with_id(app, "dex", "図鑑…", true, None::<&str>)?;
    let diary = MenuItem::with_id(app, "diary", "思い出…", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&dex, &diary, &settings, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "dex" => open_window(app, DEX_WINDOW, "PokkePet 図鑑", (640.0, 580.0)),
                "diary" => open_window(app, DIARY_WINDOW, "PokkePet 思い出", (460.0, 560.0)),
                "settings" => open_window(app, SETTINGS_WINDOW, "PokkePet 設定", (380.0, 850.0)),
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

/// 設定・図鑑などのウィンドウを開く（すでにあれば前面に出す）。中身はウィンドウのラベルで決まる。
fn open_window(app: &AppHandle, label: &str, title: &str, (width, height): (f64, f64)) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(width, height)
        .resizable(label == DEX_WINDOW || label == DIARY_WINDOW)
        .center()
        .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("failed to open the {label} window: {e}"),
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

/// メニューバーのアイコン横の目印（空腹なら🍙など）を、変わったときだけ更新する。
fn update_tray_title(app: &AppHandle, view: &StateView) {
    let title = tray_title(view);
    let mark = app.state::<TrayMark>();
    let mut shown = mark.0.lock().unwrap_or_else(|e| e.into_inner());
    if *shown == title {
        return;
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if tray.set_title(title).is_ok() {
            *shown = title;
        }
    }
}

/// 困りごとの呼びかけ通知。おやすみ中・ポップアップを開いている間・通知オフのときは鳴らさない。
fn notify_call_if_needed(app: &AppHandle, view: &StateView, popup_open: bool) {
    let notify_calls = app.state::<SettingsService>().get().notify_calls;
    let quiet = view.sleeping || popup_open || !notify_calls;
    let call = app.state::<Mutex<CallGate>>().lock().unwrap_or_else(|e| e.into_inner()).decide(now_unix(), view.expression.bubble, quiet);
    if let Some(call) = call {
        notify_call(app, call, view.name.as_deref());
    }
}

/// 旅立ちの時が来たことを知らせる（ポップアップを開いていないとき）。
fn notify_graduation(app: &AppHandle, name: Option<&str>) {
    let title = name.unwrap_or("PokkePet");
    if let Err(e) = app.notification().builder().title(title).body("旅立ちの時がきたよ。会いにきてね").show() {
        eprintln!("failed to show notification: {e}");
    }
}

fn notify_call(app: &AppHandle, call: Call, name: Option<&str>) {
    let title = name.unwrap_or("PokkePet");
    if let Err(e) = app.notification().builder().title(title).body(call.message()).show() {
        eprintln!("failed to show notification: {e}");
    }
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
                let _ = handle.emit(STATE_EVENT, tick.view.clone());
                update_tray_title(&handle, &tick.view);
                let popup_open = popup_is_open(&handle);
                if should_notify_hatch(tick.hatched, popup_open) {
                    notify_hatch(&handle);
                }
                if should_notify_graduation(tick.graduation_ready, popup_open) {
                    notify_graduation(&handle, tick.view.name.as_deref());
                }
                notify_call_if_needed(&handle, &tick.view, popup_open);
            }
            Err(e) => eprintln!("tick failed: {e}"),
        }
    });
}
