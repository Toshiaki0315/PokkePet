//! Tauri との接着層（トレイ常駐・ポップアップ制御・IPC コマンド・バックグラウンドループ）。
//! ロジックは持たず、`service` に委譲する。

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::calls::{Call, CallGate};
use crate::card::{card_file_name, decode_png_data_url};
use crate::dex::DexView;
use crate::diary::DiaryView;
use crate::game::GameState;
use crate::minigame::Side;
use crate::paths::data_dir;
use crate::sleep::local_utc_offset;
use crate::system::{parse_battery, parse_dark_mode, parse_hid_idle_secs, SystemInfo};
use crate::souvenirs::SouvenirView;
use crate::service::{should_notify_graduation, should_notify_hatch, tray_title, GameService, MiniGameReport, StateView};
use crate::settings::{Settings, SettingsService, POPUP_WIDTH};

/// Mac の状態を読む間隔（ループの何回に 1 回か。10 秒 × 3 = 30 秒）。
const SYSTEM_POLL_EVERY: u32 = 3;
/// デバッグログのファイル（データフォルダの `debug.log`）。
pub struct DebugLogPath(PathBuf);

const LOOP_INTERVAL: Duration = Duration::from_secs(10);
const STATE_EVENT: &str = "state-changed";
const SETTINGS_EVENT: &str = "settings-changed";
const SETTINGS_WINDOW: &str = "settings";
const DEX_WINDOW: &str = "dex";
const DIARY_WINDOW: &str = "diary";
const SOUVENIRS_WINDOW: &str = "souvenirs";
const EGG_CODE_WINDOW: &str = "eggcode";
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

/// マウスでなでられた。
#[tauri::command]
pub fn stroke_pet(app: AppHandle, svc: State<GameService>) -> Result<StateView, String> {
    let view = svc.stroke(now_unix()).map_err(|e| e.to_string())?;
    update_tray_title(&app, &view);
    Ok(view)
}

/// 電源ボタン。ペットを無理やり寝かせる（もう一度押すと起こす）。
#[tauri::command]
pub fn toggle_power(app: AppHandle, svc: State<GameService>) -> Result<StateView, String> {
    let view = svc.toggle_power(now_unix())?;
    update_tray_title(&app, &view);
    Ok(view)
}

/// 画面で描いた成長カード（PNG の data URL）を、ピクチャフォルダに保存する。保存した場所を返す。
#[tauri::command]
pub fn save_card(app: AppHandle, svc: State<GameService>, data_url: String) -> Result<String, String> {
    let bytes = decode_png_data_url(&data_url)?;
    let dir = app.path().picture_dir().or_else(|_| app.path().download_dir()).map_err(|e| e.to_string())?;
    let now = now_unix();
    let file_name = card_file_name(svc.snapshot().name.as_deref(), now, local_utc_offset(now));
    let path = dir.join(file_name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// デバッグログのあるフォルダを Finder で開く（まだ無ければ、データフォルダを開く）。
#[tauri::command]
pub fn reveal_debug_log(app: AppHandle, path: State<DebugLogPath>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let file = &path.0;
    let opened = if file.exists() {
        app.opener().reveal_item_in_dir(file)
    } else {
        app.opener().open_path(file.parent().unwrap_or(file).to_string_lossy(), None::<&str>)
    };
    opened.map_err(|e| e.to_string())
}

/// 最初からやり直す（隠しコマンド）。今のペットとお別れして、新しい卵を迎える。
#[tauri::command]
pub fn restart_game(app: AppHandle, svc: State<GameService>, gate: State<Mutex<CallGate>>) -> Result<StateView, String> {
    let view = svc.restart()?;
    gate.lock().unwrap_or_else(|e| e.into_inner()).answered();
    update_tray_title(&app, &view);
    Ok(view)
}

/// 亡くなったペットを見送り、新しい卵を迎える。
#[tauri::command]
pub fn bury_pet(app: AppHandle, svc: State<GameService>) -> Result<StateView, String> {
    let view = svc.bury()?;
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

/// おみやげのコレクション。
#[tauri::command]
pub fn get_souvenirs(svc: State<GameService>) -> SouvenirView {
    svc.souvenirs()
}

/// おみやげを飾る／しまう。
#[tauri::command]
pub fn toggle_souvenir(app: AppHandle, svc: State<GameService>, id: String) -> Result<SouvenirView, String> {
    let view = svc.toggle_souvenir_display(&id)?;
    let _ = app.emit(STATE_EVENT, svc.snapshot()); // 画面の飾りが変わる
    Ok(view)
}

/// いまのペットの血筋の「たまごコード」（友だちに送れる）。
#[tauri::command]
pub fn get_egg_code(svc: State<GameService>) -> String {
    svc.egg_code()
}

/// 友だちのたまごコードのたまごを迎える（いまのペットとはお別れ）。
#[tauri::command]
pub fn receive_egg(app: AppHandle, svc: State<GameService>, gate: State<Mutex<CallGate>>, code: String) -> Result<StateView, String> {
    let view = svc.receive_egg(&code)?;
    gate.lock().unwrap_or_else(|e| e.into_inner()).answered();
    update_tray_title(&app, &view);
    Ok(view)
}

/// 持ち帰ったおみやげを見せた。
#[tauri::command]
pub fn ack_gift(app: AppHandle, svc: State<GameService>) -> Result<StateView, String> {
    let view = svc.ack_gift().map_err(|e| e.to_string())?;
    update_tray_title(&app, &view);
    Ok(view)
}

#[tauri::command]
pub fn get_settings(svc: State<SettingsService>) -> Settings {
    svc.get()
}

/// 設定を検証・保存し、すべてのウィンドウへ `settings-changed` を通知する。
#[tauri::command]
pub fn save_settings(app: AppHandle, svc: State<SettingsService>, game: State<GameService>, settings: Settings) -> Result<Settings, String> {
    let saved = svc.update(settings)?;
    apply_launch_at_login(&app, saved.launch_at_login);
    game.set_sleep_window(saved.sleep_window()); // おやすみ時間の変更をすぐ反映する
    game.set_strict(saved.strict_mode);
    game.set_auto_sleep(saved.auto_sleep);
    game.set_birthday(saved.birthday());
    game.set_legacy_bonus(saved.legacy_bonus());
    game.set_debug_log(saved.debug_log.then(|| app.state::<DebugLogPath>().0.clone()));
    game.log_note("設定", "設定を保存した");
    apply_popup_size(&app, &saved);
    let _ = app.emit(SETTINGS_EVENT, saved.clone());
    let _ = app.emit(STATE_EVENT, game.snapshot());
    Ok(saved)
}

/// ポップアップの高さを設定（ゲージの表示）に合わせる。位置は上端を保つので、下に伸び縮みする。
fn apply_popup_size(app: &AppHandle, settings: &Settings) {
    if let Some(window) = app.get_webview_window("main")
        && let Err(e) = window.set_size(tauri::LogicalSize::new(POPUP_WIDTH, settings.popup_height()))
    {
        eprintln!("failed to resize the popup: {e}");
    }
}

/// ログイン時の自動起動を設定に合わせる。開発版は（開発用のバイナリを登録してしまうので）何もしない。
fn apply_launch_at_login(app: &AppHandle, wanted: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    let launcher = app.autolaunch();
    match launcher.is_enabled() {
        Ok(current) if current == wanted => {}
        _ => {
            let result = if wanted { launcher.enable() } else { launcher.disable() };
            if let Err(e) = result {
                eprintln!("failed to update launch at login: {e}");
            }
        }
    }
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
    app.state::<GameService>().set_strict(settings.strict_mode);
    app.state::<GameService>().set_auto_sleep(settings.auto_sleep);
    app.state::<GameService>().set_birthday(settings.birthday());
    app.state::<GameService>().set_legacy_bonus(settings.legacy_bonus());
    app.manage(DebugLogPath(dir.join("debug.log")));
    app.state::<GameService>().set_debug_log(settings.debug_log.then(|| dir.join("debug.log")));
    app.state::<GameService>().set_events(true);
    let launch_at_login = settings.launch_at_login;
    apply_popup_size(app.handle(), &settings);
    app.manage(SettingsService::new(settings, settings_path));
    apply_launch_at_login(app.handle(), launch_at_login);

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
    let souvenirs = MenuItem::with_id(app, "souvenirs", "おみやげ…", true, None::<&str>)?;
    let egg_code = MenuItem::with_id(app, "eggcode", "たまごコード…", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&dex, &diary, &souvenirs, &egg_code, &settings, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "dex" => open_window(app, DEX_WINDOW, "PokkePet 図鑑", (640.0, 580.0)),
                "diary" => open_window(app, DIARY_WINDOW, "PokkePet 思い出", (460.0, 560.0)),
                "souvenirs" => open_window(app, SOUVENIRS_WINDOW, "PokkePet おみやげ", (460.0, 520.0)),
                "eggcode" => open_window(app, EGG_CODE_WINDOW, "PokkePet たまごコード", (460.0, 440.0)),
                "settings" => open_window(app, SETTINGS_WINDOW, "PokkePet 設定", (380.0, 760.0)),
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
            // 開く前に、いまの操作を観測して状態を最新にする（おでかけ中なら、この操作で戻ったことになる）。
            refresh_before_showing(tray.app_handle());
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
        .resizable(label == DEX_WINDOW || label == DIARY_WINDOW || label == SOUVENIRS_WINDOW)
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
    if let Some(tray) = app.tray_by_id(TRAY_ID)
        && tray.set_title(title).is_ok()
    {
        *shown = title;
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

/// 最後のキーボード・マウス操作からの秒数（`ioreg` で読む。権限は要らない）。読めなければ None。
fn read_idle_secs() -> Option<u64> {
    let output = std::process::Command::new("ioreg").args(["-c", "IOHIDSystem"]).output().ok()?;
    parse_hid_idle_secs(&String::from_utf8_lossy(&output.stdout))
}

/// 外部コマンドの標準出力（文字列）。実行できなければ None。
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program).args(args).output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Mac の状態（電池・ネット・ダークモード）を読む。どれも権限は要らない。
fn read_system_info() -> SystemInfo {
    SystemInfo {
        battery: command_output("pmset", &["-g", "batt"]).as_deref().and_then(parse_battery),
        online: std::process::Command::new("route").args(["-n", "get", "default"]).output().map_or(true, |o| o.status.success()),
        dark_mode: command_output("defaults", &["read", "-g", "AppleInterfaceStyle"]).is_some_and(|out| parse_dark_mode(&out)),
    }
}

/// Mac の状態を、30 秒ごとに読んでペットに伝える（設定「Mac の状態に反応」がオフなら、何も起きていない状態にする）。
fn update_system_info(handle: &AppHandle, round: u32) {
    if !round.is_multiple_of(SYSTEM_POLL_EVERY) {
        return;
    }
    let reacting = handle.state::<SettingsService>().get().system_reactions;
    let info = if reacting { read_system_info() } else { SystemInfo::default() };
    handle.state::<GameService>().set_system(info);
}

fn notify_break(app: &AppHandle, name: Option<&str>) {
    let title = name.unwrap_or("PokkePet");
    if let Err(e) = app.notification().builder().title(title).body("ずっと作業してるね。ちょっと休もうよ").show() {
        eprintln!("failed to show notification: {e}");
    }
}

/// Mac の操作を観測する（作業に寄り添う）。休憩の声かけの通知もここで出す。
fn observe_companionship(handle: &AppHandle, popup_open: bool) {
    let settings = handle.state::<SettingsService>().get();
    if !settings.companion_enabled {
        return;
    }
    let Some(idle) = read_idle_secs() else {
        return;
    };
    let svc = handle.state::<GameService>();
    let obs = svc.observe_activity(now_unix(), idle);
    if obs.break_due && settings.break_reminder && !popup_open {
        notify_break(handle, svc.snapshot().name.as_deref());
    }
}

/// ポップアップを開く直前に、Mac の操作を観測して時間を進め、最新の状態をみんなに知らせる。
fn refresh_before_showing(app: &AppHandle) {
    observe_companionship(app, true);
    let svc = app.state::<GameService>();
    if let Ok(tick) = svc.tick(now_unix()) {
        let _ = app.emit(STATE_EVENT, tick.view.clone());
        update_tray_title(app, &tick.view);
    }
}

fn spawn_loop(handle: AppHandle) {
    std::thread::spawn(move || {
        let mut round = 0u32;
        loop {
            std::thread::sleep(LOOP_INTERVAL);
            round = round.wrapping_add(1);
            update_system_info(&handle, round);
            observe_companionship(&handle, popup_is_open(&handle));
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
        }
    });
}
