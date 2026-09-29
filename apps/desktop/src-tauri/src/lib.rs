//! Tauri 2 shell. The WebView only renders the shared React UI; the engine,
//! transports and audio run natively so push-to-talk keeps working from the
//! tray with the window hidden (WebView2 throttles hidden pages).

pub mod audio;
pub mod engine;
pub mod lan;
pub mod ptt;
pub mod relay;
#[cfg(windows)]
pub mod winshell;

use std::sync::atomic::Ordering;
use std::sync::mpsc;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
#[cfg(not(windows))]
use tauri_plugin_notification::NotificationExt;

use engine::{now_ms, Alert, Cmd, EngineTx, Host, Settings, Sink};

struct AppState {
    tx: EngineTx,
    node_id: String,
}

fn call<T: Send + 'static>(tx: &EngineTx, f: impl FnOnce(&mut Host) -> T + Send + 'static) -> Result<T, String> {
    let (rtx, rrx) = mpsc::channel();
    tx.send(Cmd::Call(Box::new(move |h| { let _ = rtx.send(f(h)); }))).map_err(|_| "engine stopped".to_string())?;
    rrx.recv_timeout(std::time::Duration::from_secs(5)).map_err(|_| "engine timeout".to_string())
}

// ---- sink: engine → UI ------------------------------------------------------

struct TauriSink(AppHandle);

#[derive(Serialize, Clone)]
struct FloorEv { talking: bool, talker: Option<String> }

impl Sink for TauriSink {
    fn ui(&self, ev: String) {
        let _ = self.0.emit("titi://ui", ev);
    }
    fn groups(&self, g: String) {
        #[cfg(windows)]
        if let Ok(v) = serde_json::from_str::<Vec<serde_json::Value>>(&g) {
            winshell::set_jump_list(v.iter().filter_map(|x| Some((x["id"].as_str()?.to_string(), x["name"].as_str()?.to_string()))).collect());
        }
        let _ = self.0.emit("titi://groups", g);
    }
    fn level(&self, dbfs: f32) {
        let _ = self.0.emit("titi://level", dbfs);
    }
    fn floor(&self, talking: bool, talker: Option<String>) {
        let _ = self.0.emit("titi://floor", FloorEv { talking, talker: talker.clone() });
        update_tray(&self.0, talking, talker.as_deref());
        show_overlay(&self.0, talking && talker.is_some());
        #[cfg(windows)]
        winshell::set_state(&self.0, Some(talking && talker.is_none()), None);
    }
    fn alert(&self, a: Alert) {
        if main_focused(&self.0) && !matches!(a, Alert::Sos { .. }) {
            return; // the in-app banner / chat already shows it
        }
        show_alert(&self.0, a);
    }
}

fn main_focused(app: &AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false)
}

/// Actionable toast on Windows (Join / Not now, Open, Talk); plain notification elsewhere.
fn show_alert(app: &AppHandle, a: Alert) {
    let (title, body) = match &a {
        Alert::Invite { host_name, name, members, .. } => (format!("{host_name} invites you"), format!("Join “{name}” · {members} members")),
        Alert::Text { from_name, group_name, text, .. } => (format!("{from_name} · {group_name}"), text.clone()),
        Alert::Sos { from_name, note, cancelled, .. } => (if *cancelled { format!("{from_name} is safe") } else { format!("SOS from {from_name}") }, if note.is_empty() { "Emergency".into() } else { note.clone() }),
    };
    #[cfg(windows)]
    {
        let (buttons, default_action) = match &a {
            Alert::Invite { group, host, .. } => (vec![("Join".to_string(), format!("accept:{group}:{host}")), ("Not now".to_string(), format!("decline:{group}:{host}"))], format!("open:{group}")),
            Alert::Text { group, .. } => (vec![("Open".to_string(), format!("open:{group}")), ("Hold to talk".to_string(), format!("talk:{group}"))], format!("open:{group}")),
            Alert::Sos { group, .. } => (vec![("Open".to_string(), format!("open:{group}"))], format!("open:{group}")),
        };
        let h = app.clone();
        winshell::toast(winshell::ToastSpec { title, body, buttons, default_action }, move |arg| toast_action(&h, &arg));
    }
    #[cfg(not(windows))]
    {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

/// Toast button / body → engine + UI. Runs on a WinRT thread.
fn toast_action(app: &AppHandle, arg: &str) {
    log::info!("toast action {arg}");
    let Some(st) = app.try_state::<AppState>() else { return };
    let mut it = arg.split(':');
    match (it.next(), it.next(), it.next()) {
        (Some("accept"), Some(g), Some(host)) => { let (g, host) = (g.to_string(), host.to_string()); let _ = call(&st.tx, move |h| { if let (Ok(g), Ok(n)) = (Host::gid(&g), Host::nid(&host)) { h.run(|e, t| e.accept_invite(g, n, t)) } }); let _ = app.emit("titi://route", arg.split(':').nth(1).unwrap_or_default()); show_main(app); }
        (Some("decline"), Some(g), Some(host)) => { let (g, host) = (g.to_string(), host.to_string()); let _ = call(&st.tx, move |h| { if let (Ok(g), Ok(n)) = (Host::gid(&g), Host::nid(&host)) { h.run(|e, t| e.decline_invite(g, n, t)) } }); }
        (Some("open"), Some(g), _) => { let _ = app.emit("titi://route", g); show_main(app); }
        // a toast button can't be held: start a talk, stop with the thumbbar / hotkey / tray (60 s cap in the engine)
        (Some("talk"), Some(g), _) => { let g = g.to_string(); let _ = call(&st.tx, move |h| { if let Ok(g) = Host::gid(&g) { h.eng.set_active_group(g); h.push_groups(); } }); let _ = st.tx.send(Cmd::PttDown(0)); }
        _ => {}
    }
}

/// Single source of truth for mute: engine → UI + thumbbar + tray label.
fn set_muted(app: &AppHandle, muted: bool) {
    log::info!("mute {muted}");
    if let Some(st) = app.try_state::<AppState>() {
        let _ = call(&st.tx, move |h| h.set_muted(muted));
    }
    let _ = app.emit("titi://muted", muted);
    if let Some(m) = app.try_state::<MuteItem>() {
        let _ = m.0.set_text(if muted { "Unmute microphone" } else { "Mute microphone" });
    }
    #[cfg(windows)]
    winshell::set_state(app, None, Some(muted));
}

fn toggle_mute(app: &AppHandle) {
    let now = app.try_state::<AppState>().and_then(|st| call(&st.tx, |h| h.muted).ok()).unwrap_or(false);
    set_muted(app, !now);
}

struct MuteItem(MenuItem<tauri::Wry>);

/// argv from the jump list / a second instance: --group=<id>, --join, --toggle-mute.
fn handle_args(app: &AppHandle, args: &[String]) {
    log::info!("args {:?}", &args[1.min(args.len())..]);
    for a in args {
        if let Some(g) = a.strip_prefix("--group=") { let _ = app.emit("titi://route", g); show_main(app); }
        else if a == "--join" { let _ = app.emit("titi://route", "join"); show_main(app); }
        else if a == "--toggle-mute" { toggle_mute(app); }
    }
}

fn update_tray(app: &AppHandle, talking: bool, talker: Option<&str>) {
    if let Some(t) = app.tray_by_id("main") {
        let tip = match (talking, talker) { (true, Some(n)) => format!("Titi — {n} is talking"), (true, None) => "Titi — you are talking".into(), _ => "Titi — channel free".into() };
        let _ = t.set_tooltip(Some(tip));
    }
}

fn show_overlay(app: &AppHandle, on: bool) {
    let enabled = app.try_state::<OverlayPref>().map(|p| p.0.load(Ordering::Relaxed)).unwrap_or(true);
    if let Some(w) = app.get_webview_window("overlay") {
        let main_visible = app.get_webview_window("main").and_then(|m| m.is_visible().ok()).unwrap_or(false) && app.get_webview_window("main").and_then(|m| m.is_minimized().ok()) == Some(false);
        if on && enabled && !main_visible { let _ = w.show(); } else { let _ = w.hide(); }
    }
}

struct OverlayPref(std::sync::atomic::AtomicBool);
struct CloseToTray(std::sync::atomic::AtomicBool);

/// The one exit path (tray Quit, Settings → Quit, window close with close-to-tray off):
/// release the floor, stop the engine thread, then exit the process.
fn quit(app: &AppHandle) {
    log::info!("quit");
    if let Some(st) = app.try_state::<AppState>() {
        let (tx, rx) = mpsc::channel::<()>();
        let _ = st.tx.send(Cmd::Call(Box::new(move |_| { let _ = tx.send(()); })));
        let _ = rx.recv_timeout(std::time::Duration::from_secs(2)); // queued PTT/audio drained
        let _ = st.tx.send(Cmd::Quit);
    }
    app.exit(0);
}

#[tauri::command]
fn titi_quit(app: AppHandle) {
    quit(&app);
}

// ---- commands: UI → engine --------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitInfo { node_id: String, settings: Settings, groups: String, platform: &'static str }

#[tauri::command]
fn titi_init(st: State<AppState>) -> Result<InitInfo, String> {
    let node_id = st.node_id.clone();
    call(&st.tx, move |h| InitInfo { node_id, settings: h.settings.clone(), groups: h.groups_json(), platform: std::env::consts::OS })
}

#[tauri::command]
fn titi_save_settings(app: AppHandle, st: State<AppState>, settings: Settings) -> Result<(), String> {
    ptt::set_binding(&settings.ptt_key);
    if let Some(p) = app.try_state::<OverlayPref>() { p.0.store(settings.overlay, Ordering::Relaxed) }
    if let Some(p) = app.try_state::<CloseToTray>() { p.0.store(settings.close_to_tray, Ordering::Relaxed) }
    {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let _ = if settings.autostart { al.enable() } else { al.disable() };
    }
    call(&st.tx, move |h| h.apply_settings(settings))
}

#[tauri::command]
fn titi_ptt(st: State<AppState>, down: bool, prio: Option<u8>) -> Result<(), String> {
    st.tx.send(if down { Cmd::PttDown(prio.unwrap_or(0)) } else { Cmd::PttUp }).map_err(|e| e.to_string())
}

#[tauri::command]
fn titi_mute(app: AppHandle, muted: bool) {
    set_muted(&app, muted);
}

#[tauri::command]
fn titi_create_group(st: State<AppState>, name: String) -> Result<(), String> {
    call(&st.tx, move |h| h.create_group(&name))?
}

#[tauri::command]
fn titi_group_op(st: State<AppState>, op: String, group: String, arg: Option<String>, flag: Option<bool>) -> Result<(), String> {
    call(&st.tx, move |h| -> Result<(), String> {
        let g = Host::gid(&group)?;
        match op.as_str() {
            "leave" => { h.run(|e, n| e.leave_group(g, n)); h.push_groups(); h.sync_rooms(); }
            "active" => { h.eng.set_active_group(g); h.push_groups(); }
            "duplex" => { h.run(|e, n| e.set_full_duplex(g, flag.unwrap_or(false), n)); h.push_groups(); }
            "invite" => { let node = Host::nid(arg.as_deref().unwrap_or(""))?; h.run(|e, n| e.invite_peer(g, node, n)); }
            "accept" => { let host = Host::nid(arg.as_deref().unwrap_or(""))?; h.run(|e, n| e.accept_invite(g, host, n)); }
            "decline" => { let host = Host::nid(arg.as_deref().unwrap_or(""))?; h.run(|e, n| e.decline_invite(g, host, n)); }
            "text" => { let t = arg.unwrap_or_default(); h.run(|e, n| e.send_text(g, &t, n)); }
            "sos" => { let c = flag.unwrap_or(false); h.run(|e, n| e.send_sos(g, 0.0, 0.0, "", c, n)); }
            _ => return Err(format!("unknown op {op}")),
        }
        Ok(())
    })?
}

#[tauri::command]
fn titi_join(st: State<AppState>, code: Option<String>, link: Option<String>) -> Result<(), String> {
    call(&st.tx, move |h| {
        if let Some(c) = code { h.join_by_code(&c) }
        if let Some(l) = link { h.run(|e, n| e.join_by_link(&l, n)) }
    })
}

#[tauri::command]
fn titi_code(st: State<AppState>, group: String) -> Result<Option<(String, u32)>, String> {
    call(&st.tx, move |h| Host::gid(&group).map(|g| h.eng.current_code(&g, now_ms())))?
}

#[tauri::command]
fn titi_deep_link(st: State<AppState>, group: String) -> Result<Option<String>, String> {
    call(&st.tx, move |h| Host::gid(&group).map(|g| h.eng.deep_link(&g, now_ms(), 10 * 60_000)))?
}

#[tauri::command]
fn titi_parse_code(text: String) -> bool {
    titi_core::invite::Code::parse(&text).is_ok()
}

/// Diagnostics for support + automated verification: neighbours with link class, frames played.
#[tauri::command]
fn titi_debug(st: State<AppState>) -> Result<serde_json::Value, String> {
    call(&st.tx, |h| {
        let peers: Vec<serde_json::Value> = h.eng.sessions.values().map(|s| serde_json::json!({ "link": s.link, "token": s.token, "node": s.node.map(|n| titi_core::json::hex(&n)) })).collect();
        serde_json::json!({ "node": h.node_hex(), "played": h.played, "sessions": peers, "groups": h.eng.groups.len(), "muted": h.muted })
    })
}

#[tauri::command]
fn titi_cue(st: State<AppState>, kind: String) -> Result<(), String> {
    call(&st.tx, move |h| h.cue(&kind))
}

/// Verification hook: raise the same toast a remote text message would.
#[tauri::command]
fn titi_shell_test(app: AppHandle, st: State<AppState>) -> Result<(), String> {
    let g = call(&st.tx, |h| h.eng.groups.values().next().map(|g| (titi_core::json::hex(&g.id), g.name.clone())))?.ok_or("no group")?;
    show_alert(&app, Alert::Text { group: g.0, group_name: g.1, from_name: "Titi test".into(), text: "Toast with actions".into() });
    Ok(())
}

#[derive(Serialize)]
struct Devices { inputs: Vec<audio::Device>, outputs: Vec<audio::Device> }

#[tauri::command]
async fn titi_devices() -> Devices {
    let (inputs, outputs) = tauri::async_runtime::spawn_blocking(audio::list_devices).await.unwrap_or_default();
    Devices { inputs, outputs }
}

#[tauri::command]
async fn titi_learn_ptt() -> Option<String> {
    tauri::async_runtime::spawn_blocking(ptt::learn).await.ok().flatten()
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn handle_links(app: &AppHandle, urls: Vec<String>) {
    for u in urls {
        if u.starts_with("titi://j/") || u.contains("/j/") {
            let link = if u.starts_with("titi://") { u } else { format!("titi://j/{}", u.split("/j/").nth(1).unwrap_or("")) };
            let _ = app.emit("titi://deeplink", link);
            show_main(app);
        }
    }
}

/// Installed builds have no console: log to %LOCALAPPDATA%\Titi\logs\titi.log
/// (MSIX redirects it into the package's LocalCache). One rotation at 2 MB.
fn init_logging() {
    let mut b = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,tao=warn,wry=warn"));
    if let Some(dir) = dirs::data_local_dir().map(|d| d.join("Titi").join("logs")) {
        let path = dir.join("titi.log");
        let _ = std::fs::create_dir_all(&dir);
        if std::fs::metadata(&path).map(|m| m.len() > 2 << 20).unwrap_or(false) {
            let _ = std::fs::rename(&path, dir.join("titi.1.log"));
        }
        if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            b.target(env_logger::Target::Pipe(Box::new(f)));
        }
    }
    b.init();
}

pub fn run() {
    init_logging();
    #[cfg(windows)]
    winshell::set_process_aumid();
    // tungstenite's rustls build ships no default provider; pick ring once, process-wide
    let _ = rustls::crypto::ring::default_provider().install_default();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _| {
            // jump-list tasks re-launch the exe with flags; toggling mute must not pop the window
            if !argv.iter().any(|a| a == "--toggle-mute") { show_main(app); }
            handle_args(app, &argv);
            handle_links(app, argv.into_iter().filter(|a| a.contains("titi://") || a.contains("/j/")).collect());
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_window_state::Builder::default().with_denylist(&["overlay"]).build())
        .setup(|app| {
            let dir = app.path().app_data_dir().unwrap_or_else(|_| dirs::data_dir().unwrap_or_default().join("Titi"));
            let started = engine::spawn(dir, Box::new(TauriSink(app.handle().clone())));
            ptt::set_binding(&started.settings.ptt_key);
            app.manage(OverlayPref(std::sync::atomic::AtomicBool::new(started.settings.overlay)));
            app.manage(CloseToTray(std::sync::atomic::AtomicBool::new(started.settings.close_to_tray)));
            let h2 = app.handle().clone();
            ptt::install(started.tx.clone(), move |down| { let _ = h2.emit("titi://ptt-key", down); });
            app.manage(AppState { tx: started.tx.clone(), node_id: started.node_id.clone() });

            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let h = app.handle().clone();
                app.deep_link().on_open_url(move |e| handle_links(&h, e.urls().iter().map(|u| u.to_string()).collect()));
                let args: Vec<String> = std::env::args().filter(|a| a.starts_with("titi://")).collect();
                if !args.is_empty() { let h = app.handle().clone(); std::thread::spawn(move || { std::thread::sleep(std::time::Duration::from_millis(1500)); handle_links(&h, args) }); }
            }

            let main = app.get_webview_window("main").expect("main window");
            #[cfg(windows)]
            {
                // Windows 11 Mica behind the (transparent) web content; Win10 falls back to solid
                if window_vibrancy::apply_mica(&main, Some(true)).is_err() {
                    let _ = main.set_background_color(Some(tauri::window::Color(14, 16, 19, 255)));
                }
            }
            if std::env::args().any(|a| a == "--minimized") { let _ = main.hide(); }
            #[cfg(windows)]
            if let Ok(hwnd) = main.hwnd() {
                let (h, tx) = (app.handle().clone(), started.tx.clone());
                winshell::install_thumbbar(hwnd.0 as isize, move |a| match a {
                    // taskbar buttons are clicks, not holds: toggle talking
                    winshell::ThumbAction::Talk => { let _ = tx.send(if winshell::talking() { Cmd::PttUp } else { Cmd::PttDown(0) }); }
                    winshell::ThumbAction::Mute => toggle_mute(&h),
                });
            }
            {
                let args: Vec<String> = std::env::args().collect();
                let h = app.handle().clone();
                if args.iter().any(|a| a.starts_with("--group=") || a == "--join" || a == "--toggle-mute") {
                    std::thread::spawn(move || { std::thread::sleep(std::time::Duration::from_millis(1500)); handle_args(&h, &args) });
                }
            }

            // tray
            let open = MenuItem::with_id(app, "open", "Open Titi", true, None::<&str>)?;
            let talk = MenuItem::with_id(app, "talk", "Talk / stop talking", true, None::<&str>)?;
            let mute = MenuItem::with_id(app, "mute", "Mute microphone", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Titi", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &talk, &mute, &PredefinedMenuItem::separator(app)?, &quit_item])?;
            app.manage(MuteItem(mute.clone()));
            let tx = started.tx.clone();
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("icon"))
                .tooltip("Titi — channel free")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, e| match e.id.as_ref() {
                    "open" => show_main(app),
                    "mute" => toggle_mute(app),
                    "talk" => {
                        #[cfg(windows)]
                        let on = winshell::talking();
                        #[cfg(not(windows))]
                        let on = false;
                        let _ = tx.send(if on { Cmd::PttUp } else { Cmd::PttDown(0) });
                    }
                    "quit" => quit(app),
                    _ => {}
                })
                .on_tray_icon_event(|t, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e { show_main(t.app_handle()) }
                })
                .build(app)?;

            // floating "who is talking" pill: frameless, always on top, click-through-ish, bottom-centre
            let ov = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html#overlay".into()))
                .title("Titi overlay")
                .inner_size(300.0, 64.0)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .shadow(false)
                .visible(false)
                .build()?;
            if let Ok(Some(m)) = ov.current_monitor() {
                let s = m.scale_factor();
                let sz = m.size().to_logical::<f64>(s);
                let _ = ov.set_position(tauri::LogicalPosition::new((sz.width - 300.0) / 2.0, sz.height - 140.0));
            }
            let _ = ov.set_ignore_cursor_events(true);
            Ok(())
        })
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                if w.label() == "main" {
                    api.prevent_close();
                    // "Run in the background": close hides to the tray (radio stays on); off: quit for real
                    let to_tray = w.app_handle().try_state::<CloseToTray>().map(|c| c.0.load(Ordering::Relaxed)).unwrap_or(true);
                    if to_tray { let _ = w.hide(); } else { quit(w.app_handle()) }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            titi_init, titi_save_settings, titi_ptt, titi_mute, titi_create_group, titi_group_op, titi_join, titi_code, titi_deep_link, titi_parse_code, titi_cue, titi_devices, titi_learn_ptt, titi_debug, titi_shell_test, titi_quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running titi");
}
