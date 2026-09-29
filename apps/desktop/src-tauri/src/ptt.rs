//! System-wide hold-to-talk. On Windows a `WH_KEYBOARD_LL` + `WH_MOUSE_LL`
//! hook on its own message-loop thread gives exact press/release for bare keys
//! (F13–F24, CapsLock, ScrollLock…) and mouse side buttons, even when Titi is
//! hidden in the tray or a game has focus. The callback only flips an atomic
//! and posts to the engine channel — it must return in < 300 ms or Windows
//! silently unhooks it.
//!
//! Binding format: "F13", "CapsLock", "ScrollLock", "Pause", "Mouse4", "Mouse5",
//! "RCtrl", "RAlt", "Insert" or "VK:<decimal>". "" = disabled.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::engine::{Cmd, EngineTx};

/// 0 = off; 1..=254 = virtual-key; 0x1000|n = mouse XBUTTONn.
static BINDING: AtomicU32 = AtomicU32::new(0);
static DOWN: AtomicBool = AtomicBool::new(false);
/// While set, the next key/button press is captured as the new binding instead of talking.
static LEARN: AtomicBool = AtomicBool::new(false);
static LEARNED: Mutex<Option<std::sync::mpsc::Sender<String>>> = Mutex::new(None);
static TX: OnceLock<Mutex<EngineTx>> = OnceLock::new();
static ON_CHANGE: OnceLock<Box<dyn Fn(bool) + Send + Sync>> = OnceLock::new();

const MOUSE: u32 = 0x1000;

pub fn parse(name: &str) -> u32 {
    let n = name.trim();
    if n.is_empty() {
        return 0;
    }
    if let Some(rest) = n.strip_prefix("VK:") {
        return rest.parse().unwrap_or(0);
    }
    if let Some(f) = n.strip_prefix('F').and_then(|d| d.parse::<u32>().ok()) {
        if (1..=24).contains(&f) {
            return 0x6F + f; // VK_F1 = 0x70
        }
    }
    match n {
        "CapsLock" => 0x14,
        "ScrollLock" => 0x91,
        "Pause" => 0x13,
        "Insert" => 0x2D,
        "RCtrl" => 0xA3,
        "RAlt" => 0xA5,
        "RShift" => 0xA1,
        "Mouse4" => MOUSE | 1,
        "Mouse5" => MOUSE | 2,
        _ => 0,
    }
}

pub fn name(code: u32) -> String {
    match code {
        0 => String::new(),
        c if c & MOUSE != 0 => format!("Mouse{}", 3 + (c & 0xF)),
        c @ 0x70..=0x87 => format!("F{}", c - 0x6F),
        0x14 => "CapsLock".into(),
        0x91 => "ScrollLock".into(),
        0x13 => "Pause".into(),
        0x2D => "Insert".into(),
        0xA3 => "RCtrl".into(),
        0xA5 => "RAlt".into(),
        0xA1 => "RShift".into(),
        c => format!("VK:{c}"),
    }
}

pub fn set_binding(name: &str) {
    BINDING.store(parse(name), Ordering::Relaxed);
}

/// Waits (≤ 10 s) for the user to press the key/button they want.
pub fn learn() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    *LEARNED.lock().unwrap() = Some(tx);
    LEARN.store(true, Ordering::Relaxed);
    let r = rx.recv_timeout(std::time::Duration::from_secs(10)).ok();
    LEARN.store(false, Ordering::Relaxed);
    r
}

fn press(code: u32, down: bool) -> bool {
    if LEARN.load(Ordering::Relaxed) {
        if down {
            if let Some(tx) = LEARNED.lock().unwrap().take() {
                let _ = tx.send(name(code));
            }
        }
        return true;
    }
    let b = BINDING.load(Ordering::Relaxed);
    if b == 0 || b != code {
        return false;
    }
    // auto-repeat sends many downs; only edges matter
    if DOWN.swap(down, Ordering::Relaxed) == down {
        return true;
    }
    if let Some(tx) = TX.get() {
        let _ = tx
            .lock()
            .unwrap()
            .send(if down { Cmd::PttDown(0) } else { Cmd::PttUp });
    }
    if let Some(f) = ON_CHANGE.get() {
        f(down);
    }
    true
}

pub fn install(tx: EngineTx, on_change: impl Fn(bool) + Send + Sync + 'static) {
    let _ = TX.set(Mutex::new(tx));
    let _ = ON_CHANGE.set(Box::new(on_change));
    #[cfg(windows)]
    win::spawn();
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        WM_XBUTTONDOWN, WM_XBUTTONUP,
    };

    unsafe extern "system" fn kb(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            let k = &*(l.0 as *const KBDLLHOOKSTRUCT);
            let m = w.0 as u32;
            let down = m == WM_KEYDOWN || m == WM_SYSKEYDOWN;
            let up = m == WM_KEYUP || m == WM_SYSKEYUP;
            if (down || up) && super::press(k.vkCode, down) {
                // swallow F13–F24 (no other meaning); pass everything else through
                if (0x7C..=0x87).contains(&k.vkCode) {
                    return LRESULT(1);
                }
            }
        }
        CallNextHookEx(None, code, w, l)
    }

    unsafe extern "system" fn mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            let m = w.0 as u32;
            if m == WM_XBUTTONDOWN || m == WM_XBUTTONUP {
                let s = &*(l.0 as *const MSLLHOOKSTRUCT);
                let btn = (s.mouseData >> 16) & 0xFFFF; // XBUTTON1 = 1, XBUTTON2 = 2
                if super::press(super::MOUSE | btn, m == WM_XBUTTONDOWN) {
                    return LRESULT(1); // bound side button: don't also navigate "back"
                }
            }
        }
        CallNextHookEx(None, code, w, l)
    }

    pub fn spawn() {
        std::thread::Builder::new()
            .name("titi-ptt-hook".into())
            .spawn(|| unsafe {
                let hm = GetModuleHandleW(None).ok();
                let hinst = hm.map(|h| h.into());
                let k = SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb), hinst, 0);
                let m = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), hinst, 0);
                log::info!("ptt hooks: keyboard={} mouse={}", k.is_ok(), m.is_ok());
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
            })
            .expect("ptt hook thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_roundtrip() {
        for n in [
            "F13",
            "F24",
            "F1",
            "CapsLock",
            "ScrollLock",
            "Mouse4",
            "Mouse5",
            "RCtrl",
            "VK:200",
        ] {
            assert_eq!(name(parse(n)), n, "{n}");
        }
        assert_eq!(parse(""), 0);
        assert_eq!(parse("Nope"), 0);
        assert_eq!(parse("F13"), 0x7C);
    }
}
