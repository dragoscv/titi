//! wasm-bindgen façade over titi-core for the web PWA. Opus is done by
//! WebCodecs in the browser, so this build disables the `opus` feature and the
//! engine passes raw payloads through (`Play` carries opaque packets).

use wasm_bindgen::prelude::*;

use titi_core::engine::{Action, Config, Engine, UiEvent};
use titi_core::floor::Priority;
use titi_core::identity::Identity;
use titi_core::link::LinkClass;

#[wasm_bindgen]
pub struct WasmEngine {
    inner: Engine,
}

fn class_from(v: u8) -> LinkClass {
    LinkClass::from_u8(v).unwrap_or(LinkClass::Internet)
}

#[derive(serde::Serialize)]
#[serde(tag = "t", rename_all = "camelCase")]
enum JsAction {
    Send { link: u32, peer: Option<String>, bytes: Vec<u8> },
    Play { pcm: Vec<i16> },
    Capture { active: bool, profile: u8 },
    Ui { event: String },
    Persist { key: String, value: Vec<u8> },
    WakeAt { at_ms: f64 },
}

fn ui_json(e: &UiEvent) -> String {
    format!("{e:?}")
}

fn conv(v: Vec<Action>) -> String {
    let out: Vec<JsAction> = v
        .into_iter()
        .map(|a| match a {
            Action::Send { link, peer, bytes } => JsAction::Send { link, peer, bytes },
            Action::Play { pcm } => JsAction::Play { pcm },
            Action::Capture { active, profile } => JsAction::Capture { active, profile: profile as u8 },
            Action::Ui(e) => JsAction::Ui { event: ui_json(&e) },
            Action::Persist { key, value } => JsAction::Persist { key, value },
            Action::WakeAt(t) => JsAction::WakeAt { at_ms: t as f64 },
        })
        .collect();
    serde_json::to_string(&out).unwrap_or_else(|_| "[]".into())
}

#[wasm_bindgen]
impl WasmEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: &[u8], display_name: String, avatar_hue: u16, rng_seed: u64) -> WasmEngine {
        let id = if seed.len() == 32 {
            let mut s = [0u8; 32];
            s.copy_from_slice(seed);
            Identity::from_seed(&s)
        } else {
            Identity::generate()
        };
        let cfg = Config { display_name, avatar_hue, kdf: titi_core::crypto::KdfParams::LIGHT, relay_capable: false };
        WasmEngine { inner: Engine::new(id, cfg, rng_seed) }
    }

    pub fn identity_seed(&self) -> Vec<u8> {
        self.inner.id.seed().to_vec()
    }
    pub fn node_id(&self) -> Vec<u8> {
        self.inner.node_id().to_vec()
    }
    pub fn on_link_up(&mut self, link: u32, class: u8, now_ms: f64) -> String {
        conv(self.inner.on_link_up(link, class_from(class), None, now_ms as u64))
    }
    pub fn on_link_down(&mut self, link: u32, now_ms: f64) -> String {
        conv(self.inner.on_link_down(link, now_ms as u64))
    }
    pub fn on_peer_seen(&mut self, link: u32, token: String, now_ms: f64) -> String {
        conv(self.inner.on_peer_seen(link, token, now_ms as u64))
    }
    pub fn on_peer_lost(&mut self, link: u32, token: String, now_ms: f64) -> String {
        conv(self.inner.on_peer_lost(link, token, now_ms as u64))
    }
    pub fn on_frame(&mut self, link: u32, token: String, bytes: &[u8], now_ms: f64) -> String {
        conv(self.inner.on_frame(link, token, bytes, now_ms as u64))
    }
    /// Web passes already-encoded Opus packets (WebCodecs) as "pcm" bytes.
    pub fn on_opus_in(&mut self, packet: &[u8], now_ms: f64) -> String {
        // Reinterpret packet bytes as i16 pairs for the non-opus engine path.
        let pcm: Vec<i16> = packet.chunks(2).map(|c| i16::from_le_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect();
        conv(self.inner.on_audio_in(&pcm, now_ms as u64))
    }
    pub fn tick(&mut self, now_ms: f64) -> String {
        conv(self.inner.tick(now_ms as u64))
    }
    pub fn ptt_down(&mut self, prio: u8, now_ms: f64) -> String {
        conv(self.inner.ptt_down(Priority::from_u8(prio), now_ms as u64))
    }
    pub fn ptt_up(&mut self, now_ms: f64) -> String {
        conv(self.inner.ptt_up(now_ms as u64))
    }
    pub fn create_group(&mut self, name: String, now_ms: f64) -> Result<String, JsValue> {
        let (_, acts) = self.inner.create_group(&name, now_ms as u64).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(conv(acts))
    }
    pub fn join_by_code(&mut self, code: String, now_ms: f64) -> String {
        conv(self.inner.join_by_code(&code, now_ms as u64))
    }
    pub fn join_by_link(&mut self, url: String, now_ms: f64) -> String {
        conv(self.inner.join_by_link(&url, now_ms as u64))
    }
    pub fn set_full_duplex(&mut self, group: &[u8], on: bool, now_ms: f64) -> String {
        let Ok(g) = <[u8; 16]>::try_from(group) else { return "[]".into() };
        conv(self.inner.set_full_duplex(g, on, now_ms as u64))
    }
    pub fn send_text(&mut self, group: &[u8], text: String, now_ms: f64) -> String {
        let Ok(g) = <[u8; 16]>::try_from(group) else { return "[]".into() };
        conv(self.inner.send_text(g, &text, now_ms as u64))
    }
    pub fn current_code(&self, group: &[u8], now_ms: f64) -> Option<String> {
        let g = <[u8; 16]>::try_from(group).ok()?;
        self.inner.current_code(&g, now_ms as u64).map(|(c, s)| format!("{c}|{s}"))
    }
    pub fn deep_link(&self, group: &[u8], now_ms: f64, valid_ms: f64) -> Option<String> {
        let g = <[u8; 16]>::try_from(group).ok()?;
        self.inner.deep_link(&g, now_ms as u64, valid_ms as u64)
    }
    pub fn groups_json(&self) -> String {
        let me = self.inner.node_id();
        let items: Vec<String> = self
            .inner
            .groups
            .values()
            .map(|g| {
                format!(
                    "{{\"id\":\"{}\",\"name\":{},\"fullDuplex\":{},\"members\":{},\"active\":{},\"creator\":{}}}",
                    hex(&g.id),
                    json_str(&g.name),
                    g.full_duplex,
                    g.members.len(),
                    self.inner.active_group == Some(g.id),
                    g.creator == me
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }
    pub fn restore_groups(&mut self, data: &[u8], now_ms: f64) -> bool {
        self.inner.restore_groups(data, now_ms as u64).is_ok()
    }
}

#[wasm_bindgen]
pub fn parse_invite_code(text: &str) -> bool {
    titi_core::invite::Code::parse(text).is_ok()
}

#[wasm_bindgen]
pub fn invite_wordlist() -> Vec<String> {
    titi_core::invite::wordlist::WORDS.iter().map(|s| s.to_string()).collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
