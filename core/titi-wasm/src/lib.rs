//! wasm-bindgen façade over titi-core for the web PWA. Opus is done by
//! WebCodecs in the browser, so this build disables the `opus` feature: the
//! engine hands encoded packets out (`playPacket`) and takes encoded packets
//! in (`on_opus_in`). Actions are returned as one JSON array per call.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use titi_core::engine::{Action, Config, Engine, MessageBody, UiEvent};
use titi_core::floor::Priority;
use titi_core::frame::Profile;
use titi_core::identity::Identity;
use titi_core::link::LinkClass;

#[wasm_bindgen]
pub struct WasmEngine {
    inner: Engine,
}

impl WasmEngine {
    /// Shapes sends (MTU fragmentation) before converting to JSON.
    fn sh(&mut self, f: impl FnOnce(&mut Engine) -> Vec<Action>) -> String {
        let acts = f(&mut self.inner);
        conv(self.inner.shape(acts))
    }
    fn try_sh(
        &mut self,
        f: impl FnOnce(&mut Engine) -> Result<Vec<Action>, JsValue>,
    ) -> Result<String, JsValue> {
        let acts = f(&mut self.inner)?;
        Ok(conv(self.inner.shape(acts)))
    }
}

fn class_from(v: u8) -> LinkClass {
    LinkClass::from_u8(v).unwrap_or(LinkClass::Internet)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn link_str(l: LinkClass) -> &'static str {
    match l {
        LinkClass::Lan => "lan",
        LinkClass::WifiAware => "wifiAware",
        LinkClass::Nearby => "nearby",
        LinkClass::Hotspot => "hotspot",
        LinkClass::BleL2cap => "bleL2cap",
        LinkClass::BleGatt => "bleGatt",
        LinkClass::BtRfcomm => "btRfcomm",
        LinkClass::Internet => "internet",
        LinkClass::WebRtc => "webRtc",
    }
}

fn profile_str(p: Profile) -> &'static str {
    match p {
        Profile::Hq => "hq",
        Profile::Std => "std",
        Profile::Low => "low",
        Profile::Min => "min",
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum JsBody {
    Text {
        text: String,
    },
    VoiceNote {
        profile: &'static str,
        duration_ms: u32,
        opus_packets: Vec<u8>,
    },
    Location {
        lat_e7: i64,
        lon_e7: i64,
        accuracy_m: u32,
        breadcrumb: bool,
    },
    Sos {
        lat_e7: i64,
        lon_e7: i64,
        note: String,
        cancelled: bool,
    },
}

impl From<MessageBody> for JsBody {
    fn from(b: MessageBody) -> Self {
        match b {
            MessageBody::Text(t) => JsBody::Text { text: t },
            MessageBody::VoiceNote {
                profile,
                duration_ms,
                opus_packets,
            } => JsBody::VoiceNote {
                profile: profile_str(profile),
                duration_ms,
                opus_packets,
            },
            MessageBody::Location {
                lat_e7,
                lon_e7,
                accuracy_m,
                breadcrumb,
            } => JsBody::Location {
                lat_e7,
                lon_e7,
                accuracy_m,
                breadcrumb,
            },
            MessageBody::Sos {
                lat_e7,
                lon_e7,
                note,
                cancelled,
            } => JsBody::Sos {
                lat_e7,
                lon_e7,
                note,
                cancelled,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum JsUi {
    PeerDiscovered {
        node: String,
        name: String,
        hue: u16,
        link: &'static str,
        in_group: bool,
    },
    PeerLost {
        node: String,
    },
    PeerLink {
        node: String,
        link: &'static str,
        bars: u8,
        hops: u8,
    },
    FloorGranted,
    FloorDenied {
        holder: String,
    },
    FloorTaken {
        group: String,
        holder: String,
        name: String,
        prio: u8,
    },
    FloorIdle {
        group: String,
    },
    TalkWarning,
    TalkTimeout,
    InviteOffered {
        group: String,
        name: String,
        host: String,
        host_name: String,
        members: u32,
    },
    Joined {
        group: String,
        name: String,
    },
    JoinFailed {
        reason: String,
    },
    MemberJoined {
        group: String,
        node: String,
        name: String,
    },
    MemberLeft {
        group: String,
        node: String,
    },
    GroupDissolved {
        group: String,
        name: String,
    },
    Message {
        group: String,
        from: String,
        msg_uuid: String,
        sent_ms: f64,
        body: JsBody,
    },
    MessageAcked {
        msg_uuid: String,
        by: String,
    },
    Handover {
        group: String,
        state: String,
        link: Option<&'static str>,
        profile: &'static str,
    },
    Suspended {
        group: String,
    },
    Resumed {
        group: String,
    },
    ModeChanged {
        group: String,
        full_duplex: bool,
    },
    Level {
        talker: Option<String>,
        dbfs: f32,
    },
    Error {
        message: String,
    },
}

impl From<UiEvent> for JsUi {
    fn from(e: UiEvent) -> Self {
        use JsUi as J;
        match e {
            UiEvent::PeerDiscovered {
                node,
                name,
                hue,
                link,
                in_group,
                ..
            } => J::PeerDiscovered {
                node: hex(&node),
                name,
                hue,
                link: link_str(link),
                in_group,
            },
            UiEvent::PeerLost { node } => J::PeerLost { node: hex(&node) },
            UiEvent::PeerLink {
                node,
                link,
                bars,
                hops,
            } => J::PeerLink {
                node: hex(&node),
                link: link_str(link),
                bars,
                hops,
            },
            UiEvent::FloorGranted => J::FloorGranted,
            UiEvent::FloorDenied { holder } => J::FloorDenied {
                holder: hex(&holder),
            },
            UiEvent::FloorTaken {
                group,
                holder,
                name,
                prio,
            } => J::FloorTaken {
                group: hex(&group),
                holder: hex(&holder),
                name,
                prio,
            },
            UiEvent::FloorIdle { group } => J::FloorIdle { group: hex(&group) },
            UiEvent::TalkWarning => J::TalkWarning,
            UiEvent::TalkTimeout => J::TalkTimeout,
            UiEvent::InviteOffered {
                group,
                name,
                host,
                host_name,
                members,
            } => J::InviteOffered {
                group: hex(&group),
                name,
                host: hex(&host),
                host_name,
                members,
            },
            UiEvent::Joined { group, name } => J::Joined {
                group: hex(&group),
                name,
            },
            UiEvent::JoinFailed { reason } => J::JoinFailed { reason },
            UiEvent::MemberJoined { group, node, name } => J::MemberJoined {
                group: hex(&group),
                node: hex(&node),
                name,
            },
            UiEvent::MemberLeft { group, node } => J::MemberLeft {
                group: hex(&group),
                node: hex(&node),
            },
            UiEvent::GroupDissolved { group, name } => J::GroupDissolved {
                group: hex(&group),
                name,
            },
            UiEvent::Message {
                group,
                from,
                msg_uuid,
                sent_ms,
                body,
            } => J::Message {
                group: hex(&group),
                from: hex(&from),
                msg_uuid: hex(&msg_uuid),
                sent_ms: sent_ms as f64,
                body: body.into(),
            },
            UiEvent::MessageAcked { msg_uuid, by } => J::MessageAcked {
                msg_uuid: hex(&msg_uuid),
                by: hex(&by),
            },
            UiEvent::Handover {
                group,
                state,
                link,
                profile,
            } => J::Handover {
                group: hex(&group),
                state,
                link: link.map(link_str),
                profile: profile_str(profile),
            },
            UiEvent::Suspended { group } => J::Suspended { group: hex(&group) },
            UiEvent::Resumed { group } => J::Resumed { group: hex(&group) },
            UiEvent::ModeChanged { group, full_duplex } => J::ModeChanged {
                group: hex(&group),
                full_duplex,
            },
            UiEvent::Level { talker, dbfs } => J::Level {
                talker: talker.map(|t| hex(&t)),
                dbfs,
            },
            UiEvent::Error { message } => J::Error { message },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "t", rename_all = "camelCase")]
enum JsAction {
    Send {
        link: u32,
        peer: Option<String>,
        bytes: Vec<u8>,
    },
    PlayPacket {
        talker: String,
        packet: Vec<u8>,
        frames: u8,
    },
    Capture {
        active: bool,
        profile: &'static str,
    },
    Ui {
        event: JsUi,
    },
    Persist {
        key: String,
        value: Vec<u8>,
    },
    WakeAt {
        at_ms: f64,
    },
}

fn conv(v: Vec<Action>) -> String {
    let out: Vec<JsAction> = v
        .into_iter()
        .filter_map(|a| match a {
            Action::Send { link, peer, bytes } => Some(JsAction::Send { link, peer, bytes }),
            Action::Play { .. } => None, // never produced without the opus feature
            Action::PlayPacket {
                talker,
                packet,
                frames,
            } => Some(JsAction::PlayPacket {
                talker: hex(&talker),
                packet,
                frames,
            }),
            Action::Capture { active, profile } => Some(JsAction::Capture {
                active,
                profile: profile_str(profile),
            }),
            Action::Ui(e) => Some(JsAction::Ui { event: e.into() }),
            Action::Persist { key, value } => Some(JsAction::Persist { key, value }),
            Action::WakeAt(t) => Some(JsAction::WakeAt { at_ms: t as f64 }),
        })
        .collect();
    serde_json::to_string(&out).unwrap_or_else(|_| "[]".into())
}

fn gid(b: &[u8]) -> Result<[u8; 16], JsValue> {
    <[u8; 16]>::try_from(b).map_err(|_| JsValue::from_str("group id must be 16 bytes"))
}
fn nid(b: &[u8]) -> Result<[u8; 8], JsValue> {
    <[u8; 8]>::try_from(b).map_err(|_| JsValue::from_str("node id must be 8 bytes"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsGroup {
    id: String,
    name: String,
    full_duplex: bool,
    member_count: u32,
    is_active: bool,
    is_creator: bool,
    members: Vec<JsMember>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsMember {
    node: String,
    name: String,
    hue: u16,
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
        let cfg = Config {
            display_name,
            avatar_hue,
            kdf: titi_core::crypto::KdfParams::LIGHT,
            relay_capable: false,
            max_profile: None,
        };
        WasmEngine {
            inner: Engine::new(id, cfg, rng_seed),
        }
    }

    pub fn identity_seed(&self) -> Vec<u8> {
        self.inner.id.seed().to_vec()
    }
    pub fn node_id(&self) -> Vec<u8> {
        self.inner.node_id().to_vec()
    }
    pub fn set_display_name(&mut self, name: String, hue: u16) {
        self.inner.cfg.display_name = name;
        self.inner.cfg.avatar_hue = hue;
    }

    pub fn on_link_up(&mut self, link: u32, class: u8, now_ms: f64) -> String {
        self.sh(|e| e.on_link_up(link, class_from(class), None, now_ms as u64))
    }
    pub fn on_link_down(&mut self, link: u32, now_ms: f64) -> String {
        self.sh(|e| e.on_link_down(link, now_ms as u64))
    }
    pub fn on_link_stats(
        &mut self,
        link: u32,
        est_bps: u32,
        rtt_ms: u32,
        loss_pct: u8,
        now_ms: f64,
    ) -> String {
        self.sh(|e| e.on_link_stats(link, est_bps, rtt_ms, loss_pct, now_ms as u64))
    }
    pub fn on_peer_seen(&mut self, link: u32, token: String, now_ms: f64) -> String {
        self.sh(|e| e.on_peer_seen(link, token, now_ms as u64))
    }
    pub fn on_peer_lost(&mut self, link: u32, token: String, now_ms: f64) -> String {
        self.sh(|e| e.on_peer_lost(link, token, now_ms as u64))
    }
    pub fn on_frame(&mut self, link: u32, token: String, bytes: &[u8], now_ms: f64) -> String {
        self.sh(|e| e.on_frame(link, token, bytes, now_ms as u64))
    }
    /// One encoded Opus packet from WebCodecs (20 ms, 48 kHz mono).
    pub fn on_opus_in(&mut self, packet: &[u8], now_ms: f64) -> String {
        self.sh(|e| e.on_opus_in(packet, now_ms as u64))
    }
    pub fn tick(&mut self, now_ms: f64) -> String {
        self.sh(|e| e.tick(now_ms as u64))
    }

    pub fn ptt_down(&mut self, prio: u8, now_ms: f64) -> String {
        self.sh(|e| e.ptt_down(Priority::from_u8(prio), now_ms as u64))
    }
    pub fn ptt_up(&mut self, now_ms: f64) -> String {
        self.sh(|e| e.ptt_up(now_ms as u64))
    }

    pub fn create_group(&mut self, name: String, now_ms: f64) -> Result<String, JsValue> {
        self.try_sh(|e| {
            Ok(e.create_group(&name, now_ms as u64)
                .map_err(|e| JsValue::from_str(&e.to_string()))?
                .1)
        })
    }
    pub fn leave_group(&mut self, group: &[u8], now_ms: f64) -> Result<String, JsValue> {
        self.try_sh(|e| Ok(e.leave_group(gid(group)?, now_ms as u64)))
    }
    /// Creator only: delete the group for every member.
    pub fn dissolve_group(&mut self, group: &[u8], now_ms: f64) -> Result<String, JsValue> {
        self.try_sh(|e| {
            e.dissolve_group(gid(group)?, now_ms as u64)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        })
    }
    pub fn set_active_group(&mut self, group: &[u8]) -> Result<(), JsValue> {
        self.inner.set_active_group(gid(group)?);
        Ok(())
    }
    pub fn set_full_duplex(
        &mut self,
        group: &[u8],
        on: bool,
        now_ms: f64,
    ) -> Result<String, JsValue> {
        self.try_sh(|e| Ok(e.set_full_duplex(gid(group)?, on, now_ms as u64)))
    }
    pub fn invite_peer(
        &mut self,
        group: &[u8],
        node: &[u8],
        now_ms: f64,
    ) -> Result<String, JsValue> {
        self.try_sh(|e| Ok(e.invite_peer(gid(group)?, nid(node)?, now_ms as u64)))
    }
    pub fn accept_invite(
        &mut self,
        group: &[u8],
        host: &[u8],
        now_ms: f64,
    ) -> Result<String, JsValue> {
        self.try_sh(|e| Ok(e.accept_invite(gid(group)?, nid(host)?, now_ms as u64)))
    }
    pub fn decline_invite(
        &mut self,
        group: &[u8],
        host: &[u8],
        now_ms: f64,
    ) -> Result<String, JsValue> {
        self.try_sh(|e| Ok(e.decline_invite(gid(group)?, nid(host)?, now_ms as u64)))
    }
    pub fn join_by_code(&mut self, code: String, now_ms: f64) -> String {
        self.sh(|e| e.join_by_code(&code, now_ms as u64))
    }
    pub fn join_by_link(&mut self, url: String, now_ms: f64) -> String {
        self.sh(|e| e.join_by_link(&url, now_ms as u64))
    }
    pub fn send_text(
        &mut self,
        group: &[u8],
        text: String,
        now_ms: f64,
    ) -> Result<String, JsValue> {
        self.try_sh(|e| Ok(e.send_text(gid(group)?, &text, now_ms as u64)))
    }
    pub fn send_location(
        &mut self,
        group: &[u8],
        lat: f64,
        lon: f64,
        accuracy_m: f32,
        breadcrumb: bool,
        now_ms: f64,
    ) -> Result<String, JsValue> {
        self.try_sh(|e| {
            Ok(e.send_location(gid(group)?, lat, lon, accuracy_m, breadcrumb, now_ms as u64))
        })
    }
    pub fn send_sos(
        &mut self,
        group: &[u8],
        lat: f64,
        lon: f64,
        note: String,
        cancelled: bool,
        now_ms: f64,
    ) -> Result<String, JsValue> {
        self.try_sh(|e| Ok(e.send_sos(gid(group)?, lat, lon, &note, cancelled, now_ms as u64)))
    }
    pub fn current_code(&self, group: &[u8], now_ms: f64) -> Option<String> {
        let g = <[u8; 16]>::try_from(group).ok()?;
        self.inner
            .current_code(&g, now_ms as u64)
            .map(|(c, s)| format!("{c}|{s}"))
    }
    pub fn deep_link(&self, group: &[u8], now_ms: f64, valid_ms: f64) -> Option<String> {
        let g = <[u8; 16]>::try_from(group).ok()?;
        self.inner.deep_link(&g, now_ms as u64, valid_ms as u64)
    }
    pub fn groups_json(&self) -> String {
        let me = self.inner.node_id();
        let items: Vec<JsGroup> = self
            .inner
            .groups
            .values()
            .map(|g| JsGroup {
                id: hex(&g.id),
                name: g.name.clone(),
                full_duplex: g.full_duplex,
                member_count: g.members.len() as u32,
                is_active: self.inner.active_group == Some(g.id),
                is_creator: g.creator == me,
                members: g
                    .members
                    .values()
                    .map(|m| JsMember {
                        node: hex(&m.node),
                        name: m.name.clone(),
                        hue: m.hue,
                    })
                    .collect(),
            })
            .collect();
        serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
    }
    pub fn restore_groups(&mut self, data: &[u8], now_ms: f64) -> bool {
        self.inner.restore_groups(data, now_ms as u64).is_ok()
    }
    pub fn is_talking(&self) -> bool {
        self.inner
            .active_group
            .and_then(|g| self.inner.groups.get(&g))
            .is_some_and(|g| g.floor.is_talking())
    }
    /// Concatenated 4-byte relay rendezvous hashes the host should join for `gid`.
    pub fn rendezvous_for_group(&self, group: &[u8], now_ms: f64) -> Vec<u8> {
        let Ok(g) = <[u8; 16]>::try_from(group) else {
            return vec![];
        };
        self.inner.rendezvous_for_group(&g, now_ms as u64).concat()
    }
}

/// Concatenated 4-byte relay rendezvous hashes for a typed code (slots −1,0,+1).
#[wasm_bindgen]
pub fn rendezvous_for_code(code: &str, now_ms: f64) -> Vec<u8> {
    Engine::rendezvous_for_code(code, now_ms as u64).concat()
}

#[wasm_bindgen]
pub fn parse_invite_code(text: &str) -> bool {
    titi_core::invite::Code::parse(text).is_ok()
}

#[wasm_bindgen]
pub fn group_hash(group: &[u8]) -> Result<Vec<u8>, JsValue> {
    Ok(titi_core::identity::group_hash(&gid(group)?).to_vec())
}

#[wasm_bindgen]
pub fn invite_wordlist() -> Vec<String> {
    titi_core::invite::wordlist::WORDS
        .iter()
        .map(|s| s.to_string())
        .collect()
}
