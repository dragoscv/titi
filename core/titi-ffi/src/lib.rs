//! UniFFI surface for Kotlin and Swift. Coarse: bytes in, actions out.

use std::sync::Mutex;

use titi_core::engine::{Action, Config, Engine, MessageBody, UiEvent};
use titi_core::floor::Priority;
use titi_core::frame::Profile;
use titi_core::identity::Identity;
use titi_core::link::LinkClass;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum TitiError {
    #[error("{msg}")]
    Failed { msg: String },
}

impl From<titi_core::Error> for TitiError {
    fn from(e: titi_core::Error) -> Self {
        TitiError::Failed { msg: e.to_string() }
    }
}

#[derive(uniffi::Enum, Debug, Clone)]
pub enum FfiLinkClass {
    Lan,
    WifiAware,
    Nearby,
    Hotspot,
    BleL2cap,
    BleGatt,
    BtRfcomm,
    Internet,
    WebRtc,
}

impl From<FfiLinkClass> for LinkClass {
    fn from(c: FfiLinkClass) -> Self {
        match c {
            FfiLinkClass::Lan => LinkClass::Lan,
            FfiLinkClass::WifiAware => LinkClass::WifiAware,
            FfiLinkClass::Nearby => LinkClass::Nearby,
            FfiLinkClass::Hotspot => LinkClass::Hotspot,
            FfiLinkClass::BleL2cap => LinkClass::BleL2cap,
            FfiLinkClass::BleGatt => LinkClass::BleGatt,
            FfiLinkClass::BtRfcomm => LinkClass::BtRfcomm,
            FfiLinkClass::Internet => LinkClass::Internet,
            FfiLinkClass::WebRtc => LinkClass::WebRtc,
        }
    }
}

impl From<LinkClass> for FfiLinkClass {
    fn from(c: LinkClass) -> Self {
        match c {
            LinkClass::Lan => FfiLinkClass::Lan,
            LinkClass::WifiAware => FfiLinkClass::WifiAware,
            LinkClass::Nearby => FfiLinkClass::Nearby,
            LinkClass::Hotspot => FfiLinkClass::Hotspot,
            LinkClass::BleL2cap => FfiLinkClass::BleL2cap,
            LinkClass::BleGatt => FfiLinkClass::BleGatt,
            LinkClass::BtRfcomm => FfiLinkClass::BtRfcomm,
            LinkClass::Internet => FfiLinkClass::Internet,
            LinkClass::WebRtc => FfiLinkClass::WebRtc,
        }
    }
}

#[derive(uniffi::Enum, Debug, Clone)]
pub enum FfiProfile {
    Hq,
    Std,
    Low,
    Min,
}

impl From<Profile> for FfiProfile {
    fn from(p: Profile) -> Self {
        match p {
            Profile::Hq => FfiProfile::Hq,
            Profile::Std => FfiProfile::Std,
            Profile::Low => FfiProfile::Low,
            Profile::Min => FfiProfile::Min,
        }
    }
}

#[derive(uniffi::Enum, Debug, Clone)]
pub enum FfiPriority {
    Normal,
    Elevated,
    Emergency,
}

impl From<FfiPriority> for Priority {
    fn from(p: FfiPriority) -> Self {
        match p {
            FfiPriority::Normal => Priority::Normal,
            FfiPriority::Elevated => Priority::Elevated,
            FfiPriority::Emergency => Priority::Emergency,
        }
    }
}

#[derive(uniffi::Enum, Debug, Clone)]
pub enum FfiMessageBody {
    Text { text: String },
    VoiceNote { profile: FfiProfile, duration_ms: u32, opus_packets: Vec<u8> },
    Location { lat_e7: i64, lon_e7: i64, accuracy_m: u32, breadcrumb: bool },
    Sos { lat_e7: i64, lon_e7: i64, note: String, cancelled: bool },
}

impl From<MessageBody> for FfiMessageBody {
    fn from(b: MessageBody) -> Self {
        match b {
            MessageBody::Text(t) => FfiMessageBody::Text { text: t },
            MessageBody::VoiceNote { profile, duration_ms, opus_packets } => FfiMessageBody::VoiceNote { profile: profile.into(), duration_ms, opus_packets },
            MessageBody::Location { lat_e7, lon_e7, accuracy_m, breadcrumb } => FfiMessageBody::Location { lat_e7, lon_e7, accuracy_m, breadcrumb },
            MessageBody::Sos { lat_e7, lon_e7, note, cancelled } => FfiMessageBody::Sos { lat_e7, lon_e7, note, cancelled },
        }
    }
}

#[derive(uniffi::Enum, Debug, Clone)]
pub enum FfiUiEvent {
    PeerDiscovered { node: Vec<u8>, name: String, hue: u16, link: FfiLinkClass, in_group: bool },
    PeerLost { node: Vec<u8> },
    PeerLink { node: Vec<u8>, link: FfiLinkClass, bars: u8, hops: u8 },
    FloorGranted,
    FloorDenied { holder: Vec<u8> },
    FloorTaken { group: Vec<u8>, holder: Vec<u8>, name: String, prio: u8 },
    FloorIdle { group: Vec<u8> },
    TalkWarning,
    TalkTimeout,
    InviteOffered { group: Vec<u8>, name: String, host: Vec<u8>, host_name: String, members: u32 },
    Joined { group: Vec<u8>, name: String },
    JoinFailed { reason: String },
    MemberJoined { group: Vec<u8>, node: Vec<u8>, name: String },
    MemberLeft { group: Vec<u8>, node: Vec<u8> },
    Message { group: Vec<u8>, from: Vec<u8>, msg_uuid: Vec<u8>, sent_ms: u64, body: FfiMessageBody },
    MessageAcked { msg_uuid: Vec<u8>, by: Vec<u8> },
    Handover { group: Vec<u8>, state: String, link: Option<FfiLinkClass>, profile: FfiProfile },
    Suspended { group: Vec<u8> },
    Resumed { group: Vec<u8> },
    ModeChanged { group: Vec<u8>, full_duplex: bool },
    Level { talker: Option<Vec<u8>>, dbfs: f32 },
    Error { message: String },
}

impl From<UiEvent> for FfiUiEvent {
    fn from(e: UiEvent) -> Self {
        use FfiUiEvent as F;
        match e {
            UiEvent::PeerDiscovered { node, name, hue, link, in_group, .. } => F::PeerDiscovered { node: node.to_vec(), name, hue, link: link.into(), in_group },
            UiEvent::PeerLost { node } => F::PeerLost { node: node.to_vec() },
            UiEvent::PeerLink { node, link, bars, hops } => F::PeerLink { node: node.to_vec(), link: link.into(), bars, hops },
            UiEvent::FloorGranted => F::FloorGranted,
            UiEvent::FloorDenied { holder } => F::FloorDenied { holder: holder.to_vec() },
            UiEvent::FloorTaken { group, holder, name, prio } => F::FloorTaken { group: group.to_vec(), holder: holder.to_vec(), name, prio },
            UiEvent::FloorIdle { group } => F::FloorIdle { group: group.to_vec() },
            UiEvent::TalkWarning => F::TalkWarning,
            UiEvent::TalkTimeout => F::TalkTimeout,
            UiEvent::InviteOffered { group, name, host, host_name, members } => F::InviteOffered { group: group.to_vec(), name, host: host.to_vec(), host_name, members },
            UiEvent::Joined { group, name } => F::Joined { group: group.to_vec(), name },
            UiEvent::JoinFailed { reason } => F::JoinFailed { reason },
            UiEvent::MemberJoined { group, node, name } => F::MemberJoined { group: group.to_vec(), node: node.to_vec(), name },
            UiEvent::MemberLeft { group, node } => F::MemberLeft { group: group.to_vec(), node: node.to_vec() },
            UiEvent::Message { group, from, msg_uuid, sent_ms, body } => F::Message { group: group.to_vec(), from: from.to_vec(), msg_uuid: msg_uuid.to_vec(), sent_ms, body: body.into() },
            UiEvent::MessageAcked { msg_uuid, by } => F::MessageAcked { msg_uuid: msg_uuid.to_vec(), by: by.to_vec() },
            UiEvent::Handover { group, state, link, profile } => F::Handover { group: group.to_vec(), state, link: link.map(Into::into), profile: profile.into() },
            UiEvent::Suspended { group } => F::Suspended { group: group.to_vec() },
            UiEvent::Resumed { group } => F::Resumed { group: group.to_vec() },
            UiEvent::ModeChanged { group, full_duplex } => F::ModeChanged { group: group.to_vec(), full_duplex },
            UiEvent::Level { talker, dbfs } => F::Level { talker: talker.map(|t| t.to_vec()), dbfs },
            UiEvent::Error { message } => F::Error { message },
        }
    }
}

#[derive(uniffi::Enum, Debug, Clone)]
pub enum FfiAction {
    Send { link: u32, peer: Option<String>, bytes: Vec<u8> },
    Play { pcm: Vec<i16> },
    Capture { active: bool, profile: FfiProfile },
    Ui { event: FfiUiEvent },
    Persist { key: String, value: Vec<u8> },
    WakeAt { at_ms: u64 },
}

impl From<Action> for FfiAction {
    fn from(a: Action) -> Self {
        match a {
            Action::Send { link, peer, bytes } => FfiAction::Send { link, peer, bytes },
            Action::Play { pcm } => FfiAction::Play { pcm },
            Action::Capture { active, profile } => FfiAction::Capture { active, profile: profile.into() },
            Action::Ui(e) => FfiAction::Ui { event: e.into() },
            Action::Persist { key, value } => FfiAction::Persist { key, value },
            Action::WakeAt(t) => FfiAction::WakeAt { at_ms: t },
        }
    }
}

fn conv(v: Vec<Action>) -> Vec<FfiAction> {
    v.into_iter().map(Into::into).collect()
}

fn to16(v: &[u8]) -> Result<[u8; 16], TitiError> {
    v.try_into().map_err(|_| TitiError::Failed { msg: "group id must be 16 bytes".into() })
}
fn to8(v: &[u8]) -> Result<[u8; 8], TitiError> {
    v.try_into().map_err(|_| TitiError::Failed { msg: "node id must be 8 bytes".into() })
}

#[derive(uniffi::Record, Debug, Clone)]
pub struct GroupSummary {
    pub id: Vec<u8>,
    pub name: String,
    pub full_duplex: bool,
    pub member_count: u32,
    pub is_active: bool,
    pub is_creator: bool,
}

#[derive(uniffi::Record, Debug, Clone)]
pub struct MemberSummary {
    pub node: Vec<u8>,
    pub name: String,
    pub hue: u16,
    pub last_seen_ms: u64,
}

#[derive(uniffi::Object)]
pub struct TitiEngine {
    inner: Mutex<Engine>,
}

#[uniffi::export]
impl TitiEngine {
    /// `seed` = 32 bytes persisted identity seed, or empty to generate.
    #[uniffi::constructor]
    pub fn new(seed: Vec<u8>, display_name: String, avatar_hue: u16, rng_seed: u64) -> Result<Self, TitiError> {
        let id = if seed.len() == 32 {
            let mut s = [0u8; 32];
            s.copy_from_slice(&seed);
            Identity::from_seed(&s)
        } else {
            Identity::generate()
        };
        let cfg = Config { display_name, avatar_hue, ..Default::default() };
        Ok(TitiEngine { inner: Mutex::new(Engine::new(id, cfg, rng_seed)) })
    }

    pub fn identity_seed(&self) -> Vec<u8> {
        self.inner.lock().unwrap().id.seed().to_vec()
    }
    pub fn node_id(&self) -> Vec<u8> {
        self.inner.lock().unwrap().node_id().to_vec()
    }
    pub fn set_display_name(&self, name: String, hue: u16) {
        let mut e = self.inner.lock().unwrap();
        e.cfg.display_name = name;
        e.cfg.avatar_hue = hue;
    }

    pub fn on_link_up(&self, link: u32, class: FfiLinkClass, mtu: Option<u32>, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().on_link_up(link, class.into(), mtu.map(|m| m as usize), now_ms))
    }
    pub fn on_link_down(&self, link: u32, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().on_link_down(link, now_ms))
    }
    pub fn on_link_stats(&self, link: u32, est_bps: u32, rtt_ms: u32, loss_pct: u8, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().on_link_stats(link, est_bps, rtt_ms, loss_pct, now_ms))
    }
    pub fn on_peer_seen(&self, link: u32, token: String, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().on_peer_seen(link, token, now_ms))
    }
    pub fn on_peer_lost(&self, link: u32, token: String, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().on_peer_lost(link, token, now_ms))
    }
    pub fn on_frame(&self, link: u32, token: String, bytes: Vec<u8>, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().on_frame(link, token, &bytes, now_ms))
    }
    pub fn on_audio_in(&self, pcm: Vec<i16>, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().on_audio_in(&pcm, now_ms))
    }
    pub fn tick(&self, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().tick(now_ms))
    }

    pub fn ptt_down(&self, prio: FfiPriority, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().ptt_down(prio.into(), now_ms))
    }
    pub fn ptt_up(&self, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().ptt_up(now_ms))
    }

    pub fn create_group(&self, name: String, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        let (_, acts) = self.inner.lock().unwrap().create_group(&name, now_ms)?;
        Ok(conv(acts))
    }
    pub fn leave_group(&self, group: Vec<u8>, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().leave_group(to16(&group)?, now_ms)))
    }
    pub fn set_active_group(&self, group: Vec<u8>) -> Result<(), TitiError> {
        self.inner.lock().unwrap().set_active_group(to16(&group)?);
        Ok(())
    }
    pub fn set_full_duplex(&self, group: Vec<u8>, on: bool, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().set_full_duplex(to16(&group)?, on, now_ms)))
    }
    pub fn current_code(&self, group: Vec<u8>, now_ms: u64) -> Result<Option<String>, TitiError> {
        Ok(self.inner.lock().unwrap().current_code(&to16(&group)?, now_ms).map(|(c, secs)| format!("{c}|{secs}")))
    }
    pub fn deep_link(&self, group: Vec<u8>, now_ms: u64, valid_ms: u64) -> Result<Option<String>, TitiError> {
        Ok(self.inner.lock().unwrap().deep_link(&to16(&group)?, now_ms, valid_ms))
    }
    pub fn invite_peer(&self, group: Vec<u8>, node: Vec<u8>, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().invite_peer(to16(&group)?, to8(&node)?, now_ms)))
    }
    pub fn accept_invite(&self, group: Vec<u8>, host: Vec<u8>, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().accept_invite(to16(&group)?, to8(&host)?, now_ms)))
    }
    pub fn decline_invite(&self, group: Vec<u8>, host: Vec<u8>, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().decline_invite(to16(&group)?, to8(&host)?, now_ms)))
    }
    pub fn join_by_code(&self, code: String, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().join_by_code(&code, now_ms))
    }
    pub fn join_by_link(&self, url: String, now_ms: u64) -> Vec<FfiAction> {
        conv(self.inner.lock().unwrap().join_by_link(&url, now_ms))
    }

    pub fn send_text(&self, group: Vec<u8>, text: String, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().send_text(to16(&group)?, &text, now_ms)))
    }
    pub fn send_voice_note(&self, group: Vec<u8>, profile: FfiProfile, duration_ms: u32, opus_packets: Vec<u8>, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        let p = match profile { FfiProfile::Hq => Profile::Hq, FfiProfile::Std => Profile::Std, FfiProfile::Low => Profile::Low, FfiProfile::Min => Profile::Min };
        Ok(conv(self.inner.lock().unwrap().send_voice_note(to16(&group)?, p, duration_ms, opus_packets, now_ms)))
    }
    pub fn send_location(&self, group: Vec<u8>, lat: f64, lon: f64, accuracy_m: f32, breadcrumb: bool, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().send_location(to16(&group)?, lat, lon, accuracy_m, breadcrumb, now_ms)))
    }
    pub fn send_sos(&self, group: Vec<u8>, lat: f64, lon: f64, note: String, cancelled: bool, now_ms: u64) -> Result<Vec<FfiAction>, TitiError> {
        Ok(conv(self.inner.lock().unwrap().send_sos(to16(&group)?, lat, lon, &note, cancelled, now_ms)))
    }

    pub fn restore_groups(&self, data: Vec<u8>, now_ms: u64) -> Result<(), TitiError> {
        Ok(self.inner.lock().unwrap().restore_groups(&data, now_ms)?)
    }

    pub fn groups(&self) -> Vec<GroupSummary> {
        let e = self.inner.lock().unwrap();
        let me = e.node_id();
        e.groups
            .values()
            .map(|g| GroupSummary { id: g.id.to_vec(), name: g.name.clone(), full_duplex: g.full_duplex, member_count: g.members.len() as u32, is_active: e.active_group == Some(g.id), is_creator: g.creator == me })
            .collect()
    }
    pub fn members(&self, group: Vec<u8>) -> Result<Vec<MemberSummary>, TitiError> {
        let e = self.inner.lock().unwrap();
        let g = e.groups.get(&to16(&group)?).ok_or(TitiError::Failed { msg: "unknown group".into() })?;
        Ok(g.members.values().map(|m| MemberSummary { node: m.node.to_vec(), name: m.name.clone(), hue: m.hue, last_seen_ms: m.last_seen }).collect())
    }
    pub fn is_talking(&self) -> bool {
        let e = self.inner.lock().unwrap();
        e.active_group.and_then(|g| e.groups.get(&g)).is_some_and(|g| g.floor.is_talking())
    }
}

/// Opus encode helper for voice notes (host records PCM, we produce packets).
#[uniffi::export]
pub fn encode_voice_note(pcm_48k_mono: Vec<i16>, profile: FfiProfile) -> Result<Vec<u8>, TitiError> {
    let p = match profile { FfiProfile::Hq => Profile::Hq, FfiProfile::Std => Profile::Std, FfiProfile::Low => Profile::Low, FfiProfile::Min => Profile::Min };
    let mut enc = titi_core::audio::codec::Encoder::new(p)?;
    let n = enc.frame_samples();
    let mut out = Vec::new();
    for chunk in pcm_48k_mono.chunks(n) {
        let mut frame = chunk.to_vec();
        frame.resize(n, 0);
        let pkt = enc.encode(&frame)?;
        out.extend_from_slice(&(pkt.len() as u16).to_be_bytes());
        out.extend_from_slice(&pkt);
    }
    Ok(out)
}

#[uniffi::export]
pub fn decode_voice_note(packets: Vec<u8>, profile: FfiProfile) -> Result<Vec<i16>, TitiError> {
    let p = match profile { FfiProfile::Hq => Profile::Hq, FfiProfile::Std => Profile::Std, FfiProfile::Low => Profile::Low, FfiProfile::Min => Profile::Min };
    let mut dec = titi_core::audio::codec::Decoder::new()?;
    let n = titi_core::audio::TICK_SAMPLES * (p.frame_ms() as usize / 20);
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 <= packets.len() {
        let len = u16::from_be_bytes([packets[i], packets[i + 1]]) as usize;
        i += 2;
        if i + len > packets.len() { break; }
        out.extend(dec.decode(&packets[i..i + len], n, false)?);
        i += len;
    }
    Ok(out)
}

#[uniffi::export]
pub fn parse_invite_code(text: String) -> bool {
    titi_core::invite::Code::parse(&text).is_ok()
}

#[uniffi::export]
pub fn invite_wordlist() -> Vec<String> {
    titi_core::invite::wordlist::WORDS.iter().map(|s| s.to_string()).collect()
}
