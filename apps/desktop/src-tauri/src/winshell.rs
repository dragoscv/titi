//! Windows 11 shell integration: explicit AppUserModelID, taskbar thumbnail
//! toolbar (Talk / Mute), jump list (groups + tasks) and actionable toasts.
//! Everything is native so it keeps working with the WebView hidden.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;
use windows::core::{w, Interface, HSTRING, PWSTR};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
use windows::Win32::Storage::Packaging::Appx::GetCurrentApplicationUserModelId;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    DefSubclassProc, DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, ITaskbarList3, SHStrDupW, SetCurrentProcessExplicitAppUserModelID, SetWindowSubclass, ShellLink,
    TaskbarList, THBF_ENABLED, THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, RegisterWindowMessageW, HICON, LR_DEFAULTCOLOR, WM_COMMAND};

/// Same id the NSIS installer stamps on the Start-menu shortcut (bundle identifier).
pub const AUMID: &str = "ro.titi.desktop";

/// Package AUMID (`<family>!Titi`) when running from the MSIX, else None.
pub fn packaged_aumid() -> Option<String> {
    unsafe {
        let mut len = 0u32;
        if GetCurrentApplicationUserModelId(&mut len, None) != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        if GetCurrentApplicationUserModelId(&mut len, Some(PWSTR(buf.as_mut_ptr()))) != ERROR_SUCCESS {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize]))
    }
}

fn is_dev_build() -> bool {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.ends_with("target\\debug") || d.ends_with("target\\release"))).unwrap_or(true)
}

/// Unpackaged: group taskbar button, jump list and toasts under one id. Call before any window exists.
pub fn set_process_aumid() {
    if packaged_aumid().is_none() {
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(AUMID));
        }
    }
}

fn toast_app_id() -> String {
    // dev builds have no Start-menu shortcut carrying the AUMID → toasts would be dropped
    packaged_aumid().unwrap_or_else(|| if is_dev_build() { tauri_winrt_notification::Toast::POWERSHELL_APP_ID.to_string() } else { AUMID.to_string() })
}

// ---- thumbnail toolbar -------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbAction {
    Talk,
    Mute,
}

const ID_TALK: u32 = 1;
const ID_MUTE: u32 = 2;

static MAIN: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
static ICONS: OnceLock<[isize; 4]> = OnceLock::new(); // talk, stop, mute, unmute
static ON_CLICK: OnceLock<Box<dyn Fn(ThumbAction) + Send + Sync>> = OnceLock::new();
static TALKING: AtomicBool = AtomicBool::new(false);
static MUTED: AtomicBool = AtomicBool::new(false);
static ADDED: AtomicBool = AtomicBool::new(false);

fn icon(png: &[u8]) -> isize {
    unsafe { CreateIconFromResourceEx(png, true, 0x0003_0000, 32, 32, LR_DEFAULTCOLOR).map(|h| h.0 as isize).unwrap_or(0) }
}

fn tip(dst: &mut [u16; 260], s: &str) {
    for (d, c) in dst.iter_mut().zip(s.encode_utf16().chain(std::iter::once(0))) {
        *d = c;
    }
}

fn buttons() -> [THUMBBUTTON; 2] {
    let ic = ICONS.get().copied().unwrap_or([0; 4]);
    let talking = TALKING.load(Ordering::Relaxed);
    let muted = MUTED.load(Ordering::Relaxed);
    let mut b = [THUMBBUTTON::default(); 2];
    for x in b.iter_mut() {
        x.dwMask = THB_ICON | THB_TOOLTIP | THB_FLAGS;
        x.dwFlags = THBF_ENABLED;
    }
    b[0].iId = ID_TALK;
    b[0].hIcon = HICON(ic[if talking { 1 } else { 0 }] as *mut _);
    tip(&mut b[0].szTip, if talking { "Stop talking" } else { "Talk" });
    b[1].iId = ID_MUTE;
    b[1].hIcon = HICON(ic[if muted { 3 } else { 2 }] as *mut _);
    tip(&mut b[1].szTip, if muted { "Unmute microphone" } else { "Mute microphone" });
    b
}

fn taskbar() -> Option<ITaskbarList3> {
    unsafe {
        let t: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
        t.HrInit().ok()?;
        Some(t)
    }
}

/// Main thread only (STA). `add` after TaskbarButtonCreated, else update.
fn apply_buttons(add: bool) {
    let hwnd = HWND(MAIN.load(Ordering::Relaxed) as *mut _);
    if hwnd.0.is_null() {
        return;
    }
    let Some(t) = taskbar() else { return };
    let b = buttons();
    unsafe {
        if add || !ADDED.load(Ordering::Relaxed) {
            match t.ThumbBarAddButtons(hwnd, &b) {
                Ok(()) => { ADDED.store(true, Ordering::Relaxed); log::info!("thumbbar: buttons added") }
                Err(e) => log::debug!("thumbbar add: {e}"), // no taskbar button yet (window hidden)
            }
        } else if let Err(e) = t.ThumbBarUpdateButtons(hwnd, &b) {
            log::debug!("thumbbar update: {e}");
        }
    }
}

unsafe extern "system" fn subclass(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if Some(&msg) == TASKBAR_CREATED.get() {
        ADDED.store(false, Ordering::Relaxed); // new taskbar button (first show, Explorer restart)
        apply_buttons(true);
    } else if msg == WM_COMMAND && ((w.0 >> 16) & 0xFFFF) as u32 == THBN_CLICKED {
        let action = match (w.0 & 0xFFFF) as u32 {
            ID_TALK => Some(ThumbAction::Talk),
            ID_MUTE => Some(ThumbAction::Mute),
            _ => None,
        };
        if let (Some(a), Some(f)) = (action, ON_CLICK.get()) {
            log::info!("thumbbar: {a:?}");
            f(a);
            return LRESULT(0);
        }
    }
    DefSubclassProc(hwnd, msg, w, l)
}

/// Install on the main window (call from `setup`, main thread).
pub fn install_thumbbar(hwnd: isize, on_click: impl Fn(ThumbAction) + Send + Sync + 'static) {
    MAIN.store(hwnd, Ordering::Relaxed);
    let _ = ON_CLICK.set(Box::new(on_click));
    let _ = ICONS.set([
        icon(include_bytes!("../icons/thumbbar/talk.png")),
        icon(include_bytes!("../icons/thumbbar/stop.png")),
        icon(include_bytes!("../icons/thumbbar/mute.png")),
        icon(include_bytes!("../icons/thumbbar/unmute.png")),
    ]);
    unsafe {
        let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarButtonCreated")));
        let ok = SetWindowSubclass(HWND(hwnd as *mut _), Some(subclass), 0x7171, 0).as_bool();
        log::info!("thumbbar: subclass={ok}");
    }
    apply_buttons(false); // window may already have its taskbar button
}

/// Reflect engine state in the toolbar (any thread).
pub fn set_state(app: &AppHandle, talking: Option<bool>, muted: Option<bool>) {
    let mut changed = false;
    if let Some(t) = talking { changed |= TALKING.swap(t, Ordering::Relaxed) != t }
    if let Some(m) = muted { changed |= MUTED.swap(m, Ordering::Relaxed) != m }
    if changed {
        let _ = app.run_on_main_thread(|| apply_buttons(false));
    }
}

pub fn talking() -> bool {
    TALKING.load(Ordering::Relaxed)
}

// ---- jump list ---------------------------------------------------------------

static LAST_JUMP: Mutex<Option<Vec<(String, String)>>> = Mutex::new(None);

/// Rebuild the jump list when the group set changes (off the UI thread).
pub fn set_jump_list(groups: Vec<(String, String)>) {
    {
        let mut last = LAST_JUMP.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_ref() == Some(&groups) {
            return;
        }
        *last = Some(groups.clone());
    }
    std::thread::spawn(move || unsafe {
        // shell link / destination-list objects are apartment-threaded; WinRT JumpList is agile
        let packaged = packaged_aumid().is_some();
        let _ = CoInitializeEx(None, if packaged { COINIT_MULTITHREADED } else { COINIT_APARTMENTTHREADED });
        let r = if packaged { jump_list_winrt(&groups) } else { jump_list_win32(&groups) };
        match r {
            Ok(()) => log::info!("jump list: {} group(s)", groups.len()),
            Err(e) => log::warn!("jump list: {e}"),
        }
    });
}

const TASKS: [(&str, &str, &str); 2] = [("--toggle-mute", "Mute / unmute microphone", "Toggle the microphone"), ("--join", "Join a group…", "Join with a code or link")];

unsafe fn shell_link(exe: &std::path::Path, args: &str, title: &str, desc: &str) -> windows::core::Result<IShellLinkW> {
    let l: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
    l.SetPath(&HSTRING::from(exe.as_os_str()))?;
    l.SetArguments(&HSTRING::from(args))?;
    l.SetDescription(&HSTRING::from(desc))?;
    l.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0)?;
    // the visible label is PKEY_Title (VT_LPWSTR), not the description
    let ps: IPropertyStore = l.cast()?;
    let mut pv = PROPVARIANT::default();
    {
        let inner = &mut *pv.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = SHStrDupW(&HSTRING::from(title))?;
    }
    ps.SetValue(&PKEY_Title, &pv)?;
    ps.Commit()?;
    Ok(l)
}

fn step(what: &str, e: windows::core::Error) -> windows::core::Error {
    windows::core::Error::new(e.code(), format!("{what}: {}", e.message()))
}

unsafe fn jump_list_win32(groups: &[(String, String)]) -> windows::core::Result<()> {
    let exe = std::env::current_exe().map_err(|e| windows::core::Error::new(windows::core::HRESULT(0x8007_0002u32 as i32), e.to_string()))?;
    let dl: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
    dl.SetAppID(&HSTRING::from(AUMID)).map_err(|e| step("SetAppID", e))?;
    let mut slots = 0u32;
    let _removed: IObjectArray = dl.BeginList(&mut slots).map_err(|e| step("BeginList", e))?;
    let tasks: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
    if !groups.is_empty() {
        let coll: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        let links = groups.iter().take(slots.max(4) as usize).map(|(id, name)| shell_link(&exe, &format!("--group={id}"), name, "Open this group")).collect::<windows::core::Result<Vec<_>>>()?;
        for l in &links {
            coll.AddObject(l)?;
        }
        // "Show recently opened items" off (Start_TrackDocs=0) → custom categories are
        // E_ACCESSDENIED by design; the Tasks section is always allowed.
        if let Err(e) = dl.AppendCategory(w!("Groups"), &coll.cast::<IObjectArray>()?) {
            log::info!("jump list: custom category refused ({e}), listing groups under Tasks");
            for l in &links {
                tasks.AddObject(l)?;
            }
        }
    }
    for (a, t, d) in TASKS {
        tasks.AddObject(&shell_link(&exe, a, t, d)?)?;
    }
    dl.AddUserTasks(&tasks.cast::<IObjectArray>()?).map_err(|e| step("AddUserTasks", e))?;
    dl.CommitList().map_err(|e| step("CommitList", e))
}

fn jump_list_winrt(groups: &[(String, String)]) -> windows::core::Result<()> {
    use windows::UI::StartScreen::{JumpList, JumpListItem, JumpListSystemGroupKind};
    let jl = JumpList::LoadCurrentAsync()?.join()?;
    jl.SetSystemGroupKind(JumpListSystemGroupKind::None)?;
    let items = jl.Items()?;
    items.Clear()?;
    for (id, name) in groups {
        let it = JumpListItem::CreateWithArguments(&HSTRING::from(format!("--group={id}")), &HSTRING::from(name.as_str()))?;
        it.SetGroupName(&HSTRING::from("Groups"))?;
        it.SetDescription(&HSTRING::from("Open this group"))?;
        items.Append(&it)?;
    }
    for (a, t, d) in TASKS {
        let it = JumpListItem::CreateWithArguments(&HSTRING::from(a), &HSTRING::from(t))?;
        it.SetDescription(&HSTRING::from(d))?;
        items.Append(&it)?;
    }
    jl.SaveAsync()?.join()
}

// ---- actionable toasts -------------------------------------------------------

/// What the user picked on a toast. `None` action = toast body clicked.
pub struct ToastSpec {
    pub title: String,
    pub body: String,
    /// (label, action-argument)
    pub buttons: Vec<(String, String)>,
    /// argument passed when the body itself is clicked
    pub default_action: String,
}

pub fn toast(spec: ToastSpec, on_action: impl Fn(String) + Send + Sync + 'static) {
    use tauri_winrt_notification::{Duration, Toast};
    let default = spec.default_action.clone();
    let mut t = Toast::new(&toast_app_id()).title(&spec.title).text1(&spec.body).duration(Duration::Short);
    for (label, arg) in &spec.buttons {
        t = t.add_button(label, arg);
    }
    let t = t.on_activated(move |arg| {
        on_action(arg.unwrap_or_else(|| default.clone()));
        Ok(())
    });
    if let Err(e) = t.show() {
        log::warn!("toast: {e}");
    }
}
