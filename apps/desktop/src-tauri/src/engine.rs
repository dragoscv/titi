//! Engine host: one OS thread owns `titi_core::Engine` (deterministic, single
//! threaded like Android's `EngineHost`). Transports, audio callbacks, the
//! PTT hook and Tauri commands all talk to it through `Cmd` on one channel.
//! UI events leave as JSON (same shape as the wasm build) via `emit`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use titi_core::engine::{Action, Config, Engine, MessageBody, UiEvent};
use titi_core::floor::Priority;
use titi_core::identity::{group_hash, Identity};
use titi_core::json::{self, hex, unhex, JsUi};
use titi_core::link::LinkClass;

use crate::audio::{self, PlayQueue};
use crate::{lan, relay};

pub type EngineTx = mpsc::Sender<Cmd>;

pub enum TEvent {
    LinkUp(u32),
    LinkDown(u32),
    PeerSeen(u32, String),
    PeerLost(u32, String),
    Frame(u32, String, Vec<u8>),
}

pub type Reply<T> = mpsc::Sender<T>;

pub enum Cmd {
    T(TEvent),
    Audio(Vec<i16>),
    PttDown(u8),
    PttUp,
    Call(Box<dyn FnOnce(&mut Host) + Send>),
    Quit,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub name: String,
    pub hue: u16,
    pub onboarded: bool,
    pub relay_url: String,
    pub volume: f32,
    pub ptt_key: String,
    pub overlay: bool,
    pub autostart: bool,
    /// Window close button hides to the tray (radio stays on) instead of quitting.
    pub close_to_tray: bool,
    pub input_device: String,
    pub output_device: String,
    pub lan: bool,
    pub relay: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: String::new(),
            hue: 40,
            onboarded: false,
            relay_url: "wss://titi-relay-x3clqgvrdq-ew.a.run.app/v1/ws".into(),
            volume: 1.0,
            ptt_key: "F13".into(),
            overlay: true,
            autostart: false,
            close_to_tray: true,
            input_device: String::new(),
            output_device: String::new(),
            lan: true,
            relay: true,
        }
    }
}

/// Things worth a system notification (ids are hex).
pub enum Alert {
    Invite {
        group: String,
        host: String,
        host_name: String,
        name: String,
        members: u32,
    },
    Text {
        group: String,
        group_name: String,
        from_name: String,
        text: String,
    },
    Sos {
        group: String,
        from_name: String,
        note: String,
        cancelled: bool,
    },
}

/// Where the host sends things the UI / shell cares about.
pub trait Sink: Send + 'static {
    fn ui(&self, ev_json: String);
    fn groups(&self, groups_json: String);
    fn level(&self, dbfs: f32);
    fn floor(&self, talking: bool, talker: Option<String>);
    fn alert(&self, a: Alert);
}

pub struct Host {
    pub eng: Engine,
    pub settings: Settings,
    dir: PathBuf,
    rt: tokio::runtime::Runtime,
    tx: EngineTx,
    sink: Box<dyn Sink>,
    lan: Option<lan::LanHandle>,
    relay: Option<relay::RelayHandle>,
    capture: Option<audio::Capture>,
    _playback: Option<audio::Playback>,
    play: PlayQueue,
    level: Arc<AtomicU32>,
    /// (code, until_ms, groups that existed before the join)
    pending_code: Option<(String, u64)>,
    pending_before: Vec<[u8; 16]>,
    pub muted: bool,
    pub played: u64,
    last_rooms: Vec<([u8; 4], bool)>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn read(dir: &Path, f: &str) -> Option<Vec<u8>> {
    std::fs::read(dir.join(f)).ok()
}
fn write(dir: &Path, f: &str, b: &[u8]) {
    let tmp = dir.join(format!("{f}.tmp"));
    if std::fs::write(&tmp, b).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(f));
    }
}

impl Host {
    fn apply(&mut self, acts: Vec<Action>) {
        let acts = self.eng.shape(acts);
        for a in acts {
            match a {
                Action::Send { link, peer, bytes } => match link {
                    lan::LINK_ID => {
                        if let Some(l) = &self.lan {
                            l.send(peer, bytes)
                        }
                    }
                    relay::LINK_ID => {
                        if let Some(r) = &self.relay {
                            r.send(bytes)
                        }
                    }
                    _ => {}
                },
                Action::Play { pcm } => {
                    self.played += 1;
                    if self.played % 50 == 1 {
                        log::info!("play #{} ({} samples)", self.played, pcm.len())
                    }
                    self.play.push(&pcm)
                }
                Action::PlayPacket { .. } => {}
                Action::Capture { active, .. } => self.set_capture(active),
                Action::Persist { key, value } => {
                    if key == "groups" {
                        write(&self.dir, "groups.bin", &value)
                    }
                }
                Action::WakeAt(_) => {}
                Action::Ui(e) => self.on_ui(e),
            }
        }
    }

    fn on_ui(&mut self, e: UiEvent) {
        match &e {
            UiEvent::Level { talker, dbfs } if talker.as_ref() != Some(&self.eng.node_id()) => {
                self.sink.level(*dbfs)
            }
            UiEvent::FloorGranted => self.sink.floor(true, None),
            UiEvent::FloorTaken { holder, name, .. } if *holder != self.eng.node_id() => {
                self.sink.floor(true, Some(name.clone()))
            }
            UiEvent::FloorIdle { .. } => self.sink.floor(false, None),
            UiEvent::InviteOffered {
                group,
                name,
                host,
                host_name,
                members,
            } => self.sink.alert(Alert::Invite {
                group: hex(group),
                host: hex(host),
                host_name: host_name.clone(),
                name: name.clone(),
                members: *members,
            }),
            UiEvent::Message {
                group, from, body, ..
            } if *from != self.eng.node_id() => {
                let g = self.eng.groups.get(group);
                let from_name = g
                    .and_then(|g| g.members.get(from))
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| hex(from)[..6].to_string());
                let group_name = g.map(|g| g.name.clone()).unwrap_or_default();
                match body {
                    MessageBody::Text(t) => self.sink.alert(Alert::Text {
                        group: hex(group),
                        group_name,
                        from_name,
                        text: t.clone(),
                    }),
                    MessageBody::Sos {
                        note, cancelled, ..
                    } => self.sink.alert(Alert::Sos {
                        group: hex(group),
                        from_name,
                        note: note.clone(),
                        cancelled: *cancelled,
                    }),
                    _ => {}
                }
            }
            _ => {}
        }
        let refresh = matches!(
            e,
            UiEvent::Joined { .. } | UiEvent::MemberJoined { .. } | UiEvent::MemberLeft { .. }
        );
        if let Ok(j) = serde_json::to_string(&JsUi::from(e)) {
            self.sink.ui(j);
        }
        if refresh {
            self.push_groups();
            self.sync_rooms();
        }
    }

    fn set_capture(&mut self, on: bool) {
        if on && self.capture.is_none() {
            match audio::start_capture(
                &self.settings.input_device,
                self.tx.clone(),
                self.level.clone(),
            ) {
                Ok(c) => {
                    c.muted.store(self.muted, Ordering::Relaxed);
                    self.capture = Some(c)
                }
                Err(e) => {
                    log::warn!("capture failed: {e}");
                    let acts = self.eng.ptt_up(now_ms());
                    self.apply(acts);
                    self.on_ui(UiEvent::Error {
                        message: format!("Microphone unavailable: {e}"),
                    });
                }
            }
        } else if !on {
            self.capture = None;
        }
    }

    pub fn set_muted(&mut self, m: bool) {
        self.muted = m;
        if let Some(c) = &self.capture {
            c.muted.store(m, Ordering::Relaxed)
        }
    }

    pub fn groups_json(&self) -> String {
        serde_json::to_string(&json::groups(&self.eng)).unwrap_or_else(|_| "[]".into())
    }
    pub fn push_groups(&self) {
        self.sink.groups(self.groups_json());
    }

    pub fn sync_rooms(&mut self) {
        let now = now_ms();
        let mut rooms: Vec<([u8; 4], bool)> = Vec::new();
        for gid in self.eng.groups.keys() {
            rooms.push((group_hash(gid), false));
            for h in self.eng.rendezvous_for_group(gid, now) {
                rooms.push((h, true));
            }
        }
        if let Some((code, until)) = &self.pending_code {
            if now < *until {
                for h in Engine::rendezvous_for_code(code, now) {
                    rooms.push((h, true));
                }
            }
        }
        rooms.sort();
        rooms.dedup();
        if rooms != self.last_rooms {
            self.last_rooms = rooms.clone();
            if let Some(r) = &self.relay {
                r.set_rooms(rooms)
            }
        }
    }

    /// (Re)apply transport/audio settings.
    pub fn apply_settings(&mut self, s: Settings) {
        let old = std::mem::replace(&mut self.settings, s);
        write(
            &self.dir,
            "settings.json",
            &serde_json::to_vec_pretty(&self.settings).unwrap_or_default(),
        );
        self.eng.cfg.display_name = if self.settings.name.is_empty() {
            "Titi".into()
        } else {
            self.settings.name.clone()
        };
        self.eng.cfg.avatar_hue = self.settings.hue;
        self.play.set_volume(self.settings.volume);
        let rth = self.rt.handle().clone();
        if self.settings.lan != (self.lan.is_some()) {
            if self.settings.lan {
                self.lan = Some(lan::start(&rth, self.tx.clone()))
            } else {
                self.lan = None;
                let a = self.eng.on_link_down(lan::LINK_ID, now_ms());
                self.apply(a)
            }
        }
        let relay_changed = old.relay_url != self.settings.relay_url;
        if self.settings.relay != self.relay.is_some() || relay_changed {
            if self.relay.take().is_some() {
                let a = self.eng.on_link_down(relay::LINK_ID, now_ms());
                self.apply(a)
            }
            if self.settings.relay {
                self.relay = Some(relay::start(
                    &rth,
                    self.settings.relay_url.clone(),
                    self.relay_identity(),
                    self.tx.clone(),
                ));
                self.last_rooms.clear();
                self.sync_rooms();
            }
        } else if let Some(r) = &self.relay {
            if old.name != self.settings.name || old.hue != self.settings.hue {
                r.set_identity(self.relay_identity())
            }
        }
        if old.output_device != self.settings.output_device {
            self._playback = audio::start_playback(&self.settings.output_device, self.play.clone())
                .map_err(|e| log::warn!("playback: {e}"))
                .ok();
        }
        if old.input_device != self.settings.input_device && self.capture.is_some() {
            self.capture = None;
            self.set_capture(true);
        }
    }

    fn relay_identity(&self) -> relay::Identity {
        relay::Identity {
            node_id: self.eng.node_id().to_vec(),
            name: self.eng.cfg.display_name.clone(),
            hue: self.settings.hue as u32,
        }
    }

    pub fn cue(&self, kind: &str) {
        let t: &[(f32, f32, f32)] = match kind {
            "granted" => &[(1760.0, 0.045, 0.5), (2350.0, 0.07, 0.45)],
            "released" => &[(1320.0, 0.05, 0.4), (880.0, 0.07, 0.35)],
            "denied" => &[(330.0, 0.06, 0.35), (0.0, 0.04, 0.0), (330.0, 0.06, 0.35)],
            "incoming" => &[(1568.0, 0.04, 0.3)],
            "warning" => &[(988.0, 0.08, 0.3), (0.0, 0.06, 0.0), (988.0, 0.08, 0.3)],
            "sos" => &[
                (1760.0, 0.12, 0.5),
                (1175.0, 0.12, 0.5),
                (1760.0, 0.12, 0.5),
                (1175.0, 0.12, 0.5),
            ],
            _ => &[],
        };
        self.play.cue(t);
    }

    // ---- intents (called via Cmd::Call) -----------------------------------
    pub fn create_group(&mut self, name: &str) -> Result<(), String> {
        let (_, a) = self
            .eng
            .create_group(name, now_ms())
            .map_err(|e| e.to_string())?;
        self.apply(a);
        self.push_groups();
        self.sync_rooms();
        Ok(())
    }
    pub fn gid(s: &str) -> Result<[u8; 16], String> {
        unhex(s)
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| "bad group id".into())
    }
    pub fn nid(s: &str) -> Result<[u8; 8], String> {
        unhex(s)
            .and_then(|v| v.try_into().ok())
            .ok_or_else(|| "bad node id".into())
    }
    pub fn run(&mut self, f: impl FnOnce(&mut Engine, u64) -> Vec<Action>) {
        let a = f(&mut self.eng, now_ms());
        self.apply(a);
    }
    pub fn join_by_code(&mut self, code: &str) {
        self.pending_code = Some((code.to_string(), now_ms() + 120_000));
        self.pending_before = self.eng.groups.keys().copied().collect();
        self.sync_rooms();
        self.run(|e, n| e.join_by_code(code, n));
    }
    pub fn node_hex(&self) -> String {
        hex(&self.eng.node_id())
    }
}

pub struct Started {
    pub tx: EngineTx,
    pub node_id: String,
    pub settings: Settings,
    pub level: Arc<AtomicU32>,
}

pub fn spawn(dir: PathBuf, sink: Box<dyn Sink>) -> Started {
    std::fs::create_dir_all(&dir).ok();
    let settings: Settings = read(&dir, "settings.json")
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let id = match read(&dir, "identity.seed") {
        Some(s) if s.len() == 32 => Identity::from_seed(&s.try_into().unwrap()),
        _ => {
            let id = Identity::generate();
            write(&dir, "identity.seed", &id.seed());
            id
        }
    };
    let cfg = Config {
        display_name: if settings.name.is_empty() {
            "Titi".into()
        } else {
            settings.name.clone()
        },
        avatar_hue: settings.hue,
        ..Default::default()
    };
    let mut eng = Engine::new(id, cfg, rand::random());
    if let Some(g) = read(&dir, "groups.bin") {
        if let Err(e) = eng.restore_groups(&g, now_ms()) {
            log::warn!("restore groups: {e}")
        }
    }
    let node_id = hex(&eng.node_id());
    let (tx, rx) = mpsc::channel::<Cmd>();
    let level = Arc::new(AtomicU32::new((-60f32).to_bits()));
    let out = Started {
        tx: tx.clone(),
        node_id,
        settings: settings.clone(),
        level: level.clone(),
    };
    let s0 = settings.clone();
    std::thread::Builder::new()
        .name("titi-engine".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("titi-net")
                .build()
                .expect("tokio");
            let play = PlayQueue::default();
            play.set_volume(s0.volume);
            let playback = audio::start_playback(&s0.output_device, play.clone())
                .map_err(|e| log::warn!("playback: {e}"))
                .ok();
            let mut h = Host {
                eng,
                settings: Settings {
                    lan: false,
                    relay: false,
                    ..s0.clone()
                },
                dir,
                rt,
                tx: tx.clone(),
                sink,
                lan: None,
                relay: None,
                capture: None,
                _playback: playback,
                play,
                level,
                pending_code: None,
                pending_before: Vec::new(),
                muted: false,
                played: 0,
                last_rooms: Vec::new(),
            };
            h.apply_settings(s0);
            h.push_groups();
            run_loop(h, rx);
        })
        .expect("engine thread");
    out
}

fn run_loop(mut h: Host, rx: mpsc::Receiver<Cmd>) {
    let tick = Duration::from_millis(20);
    let mut next = Instant::now() + tick;
    let mut n: u64 = 0;
    loop {
        let wait = next.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(Cmd::Quit) | Err(RecvTimeoutError::Disconnected) => {
                let a = h.eng.ptt_up(now_ms());
                h.apply(a);
                return;
            }
            Ok(c) => handle(&mut h, c),
            Err(RecvTimeoutError::Timeout) => {}
        }
        if Instant::now() >= next {
            next += tick;
            if Instant::now() > next + tick * 5 {
                next = Instant::now() + tick
            } // slept (laptop suspend)
            let a = h.eng.tick(now_ms());
            h.apply(a);
            n += 1;
            if n.is_multiple_of(50) {
                let db = f32::from_bits(h.level.load(Ordering::Relaxed));
                if h.capture.is_some() {
                    h.sink.level(db)
                }
            }
            if n.is_multiple_of(150) {
                if let Some((code, until)) = h.pending_code.clone() {
                    let joined = h.eng.groups.keys().any(|g| !h.pending_before.contains(g));
                    if joined || now_ms() > until {
                        h.pending_code = None;
                        h.sync_rooms();
                    } else {
                        h.run(|e, t| e.join_by_code(&code, t));
                    }
                }
            }
            if n.is_multiple_of(500) {
                h.sync_rooms();
            }
        }
    }
}

fn handle(h: &mut Host, c: Cmd) {
    let now = now_ms();
    match c {
        Cmd::T(t) => {
            let a = match t {
                TEvent::LinkUp(id) => {
                    let (class, bps, rtt, mtu) = if id == lan::LINK_ID {
                        (LinkClass::Lan, 5_000_000, 8, Some(1200))
                    } else {
                        (LinkClass::Internet, 200_000, 120, None)
                    };
                    let mut a = h.eng.on_link_up(id, class, mtu, now);
                    a.extend(h.eng.on_link_stats(id, bps, rtt, 0, now));
                    a
                }
                TEvent::LinkDown(id) => h.eng.on_link_down(id, now),
                TEvent::PeerSeen(id, t) => h.eng.on_peer_seen(id, t, now),
                TEvent::PeerLost(id, t) => h.eng.on_peer_lost(id, t, now),
                TEvent::Frame(id, t, b) => h.eng.on_frame(id, t, &b, now),
            };
            h.apply(a);
        }
        Cmd::Audio(pcm) => {
            let a = h.eng.on_audio_in(&pcm, now);
            h.apply(a)
        }
        Cmd::PttDown(p) => {
            let a = h.eng.ptt_down(Priority::from_u8(p), now);
            h.apply(a)
        }
        Cmd::PttUp => {
            let a = h.eng.ptt_up(now);
            h.apply(a)
        }
        Cmd::Call(f) => f(h),
        Cmd::Quit => {}
    }
}
