//! Tauri 2 shell. The WebView only renders the shared React UI; the engine,
//! transports and audio run natively so push-to-talk keeps working from the
//! tray with the window hidden (WebView2 throttles hidden pages).

pub mod audio;
pub mod engine;
pub mod lan;
pub mod ptt;
pub mod relay;

use std::sync::atomic::Ordering;
use std::sync::mpsc;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

use engine::{now_ms, Cmd, EngineTx, Host, Settings, Sink};

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
        let _ = self.0.emit("titi://groups", g);
    }
    fn level(&self, dbfs: f32) {
        let _ = self.0.emit("titi://level", dbfs);
    }
    fn floor(&self, talking: bool, talker: Option<String>) {
        let _ = self.0.emit("titi://floor", FloorEv { talking, talker: talker.clone() });
        update_tray(&self.0, talking, talker.as_deref());
        show_overlay(&self.0, talking && talker.is_some());
    }
    fn notify(&self, title: &str, body: &str) {
        let _ = self.0.notification().builder().title(title).body(body).show();
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
fn titi_mute(st: State<AppState>, muted: bool) -> Result<(), String> {
    call(&st.tx, move |h| h.set_muted(muted))
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
        serde_json::json!({ "node": h.node_hex(), "played": h.played, "sessions": peers, "groups": h.eng.groups.len() })
    })
}

#[tauri::command]
fn titi_cue(st: State<AppState>, kind: String) -> Result<(), String> {
    call(&st.tx, move |h| h.cue(&kind))
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

#[tauri::command]
fn titi_notify(app: AppHandle, title: String, body: String) {
    let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    if !focused {
        let _ = app.notification().builder().title(title).body(body).show();
    }
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

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,tao=warn,wry=warn")).init();
    // tungstenite's rustls build ships no default provider; pick ring once, process-wide
    let _ = rustls::crypto::ring::default_provider().install_default();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _| {
            show_main(app);
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

            // tray
            let open = MenuItem::with_id(app, "open", "Open Titi", true, None::<&str>)?;
            let mute = MenuItem::with_id(app, "mute", "Mute microphone", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &mute, &PredefinedMenuItem::separator(app)?, &quit])?;
            let tx = started.tx.clone();
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("icon"))
                .tooltip("Titi — channel free")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, e| match e.id.as_ref() {
                    "open" => show_main(app),
                    "mute" => { let _ = app.emit("titi://toggle-mute", ()); }
                    "quit" => { let _ = tx.send(Cmd::Quit); app.exit(0) }
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
            // closing the main window keeps Titi in the tray (radio stays on)
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                if w.label() == "main" { api.prevent_close(); let _ = w.hide(); }
            }
        })
        .invoke_handler(tauri::generate_handler![
            titi_init, titi_save_settings, titi_ptt, titi_mute, titi_create_group, titi_group_op, titi_join, titi_code, titi_deep_link, titi_parse_code, titi_cue, titi_devices, titi_learn_ptt, titi_notify, titi_debug
        ])
        .run(tauri::generate_context!())
        .expect("error while running titi");
}
