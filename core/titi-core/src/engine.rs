//! Engine façade: wires identity, links, mesh, groups, floor, audio, store.
//!
//! Host contract (all platforms):
//! - call `on_link_up/down`, `on_peer_seen/lost` as transports report
//! - call `on_frame(link, peer_token, bytes, now)` for every received datagram
//! - call `on_audio_in(pcm_20ms, now)` from the capture thread at 20 ms cadence
//! - call `tick(now)` every 20 ms (or whenever `next_tick` says)
//! - pass every returned batch through [`Engine::shape`], then execute each [`Action`]
//!   (shape splits envelopes larger than the link MTU into paced `Fragment` frames)

use std::collections::{HashMap, HashSet, VecDeque};

use prost::Message;

use crate::audio::{self, JitterBuffer, Packet, Pop};
use crate::crypto::{self, GroupCipher, Pattern, ReplayWindow, Session};
use crate::floor::{Claim, Floor, FloorEvent, Priority};
use crate::frame::{
    self, flags, fragment, Codec, Envelope, FrameType, NodeId, Profile, VoiceHeader,
};
use crate::handover::{Handover, HoEvent};
use crate::identity::{self, Identity};
use crate::invite::{self, Code, DeepLink};
use crate::link::{Link, LinkClass, LinkId};
use crate::mesh::{self, Dedup, FloodScheduler, NeighbourTable, Rng, Route, Topology};
use crate::proto;
use crate::store::{Store, Stored};
use crate::time::{self, Ms};
use crate::{Error, Result};

/// Group identifier (16 bytes).
pub type GroupId = [u8; 16];

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Send bytes on a link to a peer token (host-specific), or broadcast when `peer` is None.
    Send {
        link: LinkId,
        peer: Option<String>,
        bytes: Vec<u8>,
    },
    /// Play 20 ms of 48 kHz mono PCM (mixed).
    Play {
        pcm: Vec<i16>,
    },
    /// Non-opus builds (wasm): one encoded Opus packet from `talker`, for the
    /// host to decode (WebCodecs) and mix. `lost` = concealment slot.
    PlayPacket {
        talker: NodeId,
        packet: Vec<u8>,
        frames: u8,
    },
    /// Host should (re)configure capture for this profile (rate/frames).
    Capture {
        active: bool,
        profile: Profile,
    },
    Ui(UiEvent),
    /// Persist identity seed / group table.
    Persist {
        key: String,
        value: Vec<u8>,
    },
    /// Ask host to be called back at this time even if idle.
    WakeAt(Ms),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    PeerDiscovered {
        node: NodeId,
        name: String,
        hue: u16,
        link: LinkClass,
        rssi_hint: i8,
        in_group: bool,
    },
    PeerLost {
        node: NodeId,
    },
    PeerLink {
        node: NodeId,
        link: LinkClass,
        bars: u8,
        hops: u8,
    },
    FloorGranted,
    FloorDenied {
        holder: NodeId,
    },
    FloorTaken {
        group: GroupId,
        holder: NodeId,
        name: String,
        prio: u8,
    },
    FloorIdle {
        group: GroupId,
    },
    TalkWarning,
    TalkTimeout,
    InviteOffered {
        group: GroupId,
        name: String,
        host: NodeId,
        host_name: String,
        members: u32,
    },
    Joined {
        group: GroupId,
        name: String,
    },
    JoinFailed {
        reason: String,
    },
    MemberJoined {
        group: GroupId,
        node: NodeId,
        name: String,
    },
    MemberLeft {
        group: GroupId,
        node: NodeId,
    },
    /// The creator deleted the group for everyone; it is already gone locally.
    GroupDissolved {
        group: GroupId,
        name: String,
    },
    Message {
        group: GroupId,
        from: NodeId,
        msg_uuid: [u8; 16],
        sent_ms: Ms,
        body: MessageBody,
    },
    MessageAcked {
        msg_uuid: [u8; 16],
        by: NodeId,
    },
    Handover {
        group: GroupId,
        state: String,
        link: Option<LinkClass>,
        profile: Profile,
    },
    Suspended {
        group: GroupId,
    },
    Resumed {
        group: GroupId,
    },
    ModeChanged {
        group: GroupId,
        full_duplex: bool,
    },
    Level {
        talker: Option<NodeId>,
        dbfs: f32,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageBody {
    Text(String),
    VoiceNote {
        profile: Profile,
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

pub struct Member {
    pub node: NodeId,
    pub name: String,
    pub hue: u16,
    pub pubkey: Option<[u8; 32]>,
    pub short: u16,
    pub last_seen: Ms,
}

pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub creator: NodeId,
    pub ikm: [u8; 32],
    pub epoch: u32,
    pub cipher: GroupCipher,
    pub k_invite: [u8; 32],
    pub full_duplex: bool,
    pub members: HashMap<NodeId, Member>,
    pub floor: Floor,
    pub handover: HashMap<NodeId, Handover>,
    pub my_short: u16,
    pub seq: u32,
    pub replay: HashMap<NodeId, ReplayWindow>,
    pub jitter: HashMap<u16, JitterBuffer>,
    #[cfg(feature = "opus")]
    pub decoders: HashMap<u16, audio::codec::Decoder>,
    pub short_to_node: HashMap<u16, NodeId>,
    pub joined_ms: Ms,
}

impl Group {
    fn hash(&self) -> [u8; 4] {
        identity::group_hash(&self.id)
    }
}

pub struct PeerSession {
    pub session: Session,
    pub link: LinkId,
    pub token: String,
    pub node: Option<NodeId>,
    pub psk_group: Option<GroupId>,
    pub created: Ms,
}

pub struct Config {
    pub display_name: String,
    pub avatar_hue: u16,
    pub kdf: crypto::KdfParams,
    pub relay_capable: bool,
    /// User quality setting: never encode better than this (None = automatic).
    pub max_profile: Option<Profile>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            display_name: "Titi".into(),
            avatar_hue: 40,
            kdf: crypto::KdfParams::SPEC,
            relay_capable: true,
            max_profile: None,
        }
    }
}

pub struct Engine {
    pub id: Identity,
    pub cfg: Config,
    pub links: HashMap<LinkId, Link>,
    pub neighbours: NeighbourTable,
    pub topology: Topology,
    pub dedup: Dedup,
    pub flood: FloodScheduler,
    pub sessions: HashMap<(LinkId, String), PeerSession>,
    pub groups: HashMap<GroupId, Group>,
    pub store: Store,
    pub rng: Rng,
    pub active_group: Option<GroupId>,
    pub ptt_prio: Priority,
    /// Buffered PCM during arbitration (≤ 300 ms).
    ptt_buffer: Vec<i16>,
    ptt_pending: bool,
    #[cfg(feature = "opus")]
    encoder: Option<audio::codec::Encoder>,
    pcm_accum: Vec<i16>,
    last_hello: HashMap<LinkId, Ms>,
    last_announce: Ms,
    next_hello_interval: Ms,
    pending_invites: HashMap<GroupId, (Ms, [u8; 32])>,
    msg_counter: u32,
    peer_names: HashMap<NodeId, (String, u16)>,
    last_level_ui: Ms,
    last_route_eval: Ms,
    last_peer_link: HashMap<NodeId, (LinkClass, u8, u8)>,
    /// Fragments waiting for link budget (see `shape` / `drain_fragments`).
    frag_out: VecDeque<(LinkId, Option<String>, Vec<u8>)>,
    frag_credit: HashMap<LinkId, i64>,
    frag_last_ms: Ms,
    reasm: fragment::Reassembler<(LinkId, NodeId)>,
}

const PTT_BUFFER_MAX: usize = audio::TICK_SAMPLES * 15; // 300 ms
/// Max bytes of queued outgoing fragments; a new oversize frame beyond it is dropped.
const FRAG_QUEUE_MAX: usize = 4 << 20;
/// Credit cap per link (bytes) so an idle link doesn't burst a whole note at once.
const FRAG_BURST: i64 = 4_000;

impl Engine {
    pub fn new(id: Identity, cfg: Config, seed: u64) -> Self {
        Engine {
            id,
            cfg,
            links: HashMap::new(),
            neighbours: NeighbourTable::default(),
            topology: Topology::default(),
            dedup: Dedup::default(),
            flood: FloodScheduler::default(),
            sessions: HashMap::new(),
            groups: HashMap::new(),
            store: Store::default(),
            rng: Rng::new(seed),
            active_group: None,
            ptt_prio: Priority::Normal,
            ptt_buffer: Vec::new(),
            ptt_pending: false,
            #[cfg(feature = "opus")]
            encoder: None,
            pcm_accum: Vec::new(),
            last_hello: HashMap::new(),
            last_announce: 0,
            next_hello_interval: mesh::HELLO_ALONE_MS,
            pending_invites: HashMap::new(),
            msg_counter: 0,
            peer_names: HashMap::new(),
            last_level_ui: 0,
            last_route_eval: 0,
            last_peer_link: HashMap::new(),
            frag_out: VecDeque::new(),
            frag_credit: HashMap::new(),
            frag_last_ms: 0,
            reasm: fragment::Reassembler::default(),
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.id.node_id()
    }

    /// Host-side post-processing of every action batch: a `Send` larger than its
    /// link's MTU is split into hop-local `Fragment` envelopes and queued; `tick`
    /// releases them at ≤ 1 per link per tick and ≤ half the link's estimated
    /// bandwidth, so they never starve voice or trip the relay's rate limit.
    pub fn shape(&mut self, acts: Vec<Action>) -> Vec<Action> {
        let mut out = Vec::with_capacity(acts.len());
        for a in acts {
            let Action::Send { link, peer, bytes } = a else {
                out.push(a);
                continue;
            };
            let mtu = self.links.get(&link).map(|l| l.mtu).unwrap_or(1200);
            if bytes.len() <= mtu {
                out.push(Action::Send { link, peer, bytes });
                continue;
            }
            let Ok(inner) = Envelope::decode(&bytes) else {
                continue;
            };
            let set = fragment::set_id(&inner.src, inner.msg_id);
            let chunk = mtu.saturating_sub(frame::ENVELOPE_MIN + fragment::HEADER);
            let queued: usize = self.frag_out.iter().map(|(_, _, b)| b.len()).sum();
            let Some(parts) = fragment::split(set, &bytes, chunk) else {
                continue;
            };
            if queued + bytes.len() > FRAG_QUEUE_MAX {
                continue;
            }
            for p in parts {
                let e = Envelope {
                    ftype: FrameType::Fragment,
                    ttl: 1,
                    hop_start: 1,
                    flags: 0,
                    msg_id: self.next_msg_id(),
                    src: self.node_id(),
                    dst: None,
                    payload: &p,
                };
                self.frag_out.push_back((link, peer.clone(), e.encode()));
            }
        }
        out
    }

    fn drain_fragments(&mut self, now: Ms) -> Vec<Action> {
        let dt = now.saturating_sub(self.frag_last_ms).min(200) as i64;
        self.frag_last_ms = now;
        self.frag_credit.retain(|id, _| self.links.contains_key(id));
        for (id, l) in &self.links {
            let c = self.frag_credit.entry(*id).or_insert(0);
            *c = (*c + l.stats.est_bps as i64 * dt / 16_000).min(FRAG_BURST);
        }
        if self.frag_out.is_empty() {
            return vec![];
        }
        let mut used = HashSet::new();
        let mut acts = vec![];
        let mut rest = VecDeque::with_capacity(self.frag_out.len());
        while let Some((link, peer, bytes)) = self.frag_out.pop_front() {
            let Some(c) = self.frag_credit.get_mut(&link) else {
                continue;
            }; // link gone
            if *c <= 0 || used.contains(&link) {
                rest.push_back((link, peer, bytes));
                continue;
            }
            used.insert(link);
            *c -= bytes.len() as i64;
            acts.push(Action::Send { link, peer, bytes });
        }
        self.frag_out = rest;
        acts
    }

    // ───────────────────────── links & peers ─────────────────────────

    pub fn on_link_up(
        &mut self,
        id: LinkId,
        class: LinkClass,
        mtu: Option<usize>,
        now: Ms,
    ) -> Vec<Action> {
        self.links.insert(id, Link::new(id, class, mtu));
        self.last_hello.insert(id, 0);
        self.hello_on(id, now)
    }

    pub fn on_link_down(&mut self, id: LinkId, now: Ms) -> Vec<Action> {
        self.links.remove(&id);
        self.last_hello.remove(&id);
        self.sessions.retain(|(l, _), _| *l != id);
        let lost = self.neighbours.link_down(id);
        let mut acts = vec![];
        for n in lost {
            acts.push(Action::Ui(UiEvent::PeerLost { node: n }));
        }
        acts.extend(self.recompute_routes(now));
        acts
    }

    pub fn on_link_stats(
        &mut self,
        id: LinkId,
        est_bps: u32,
        rtt_ms: u32,
        loss_pct: u8,
        now: Ms,
    ) -> Vec<Action> {
        if let Some(l) = self.links.get_mut(&id) {
            l.stats.est_bps = est_bps;
            l.stats.rtt_ms = rtt_ms;
            l.stats.loss_pct = loss_pct;
            for g in self.groups.values_mut() {
                g.floor.set_rtt(rtt_ms);
            }
        }
        self.recompute_routes(now)
    }

    /// Host discovered a peer token on a link (before any HELLO). Starts a HELLO.
    pub fn on_peer_seen(&mut self, link: LinkId, token: String, now: Ms) -> Vec<Action> {
        if !self.links.contains_key(&link) {
            return vec![];
        }
        vec![Action::Send {
            link,
            peer: Some(token),
            bytes: self.build_hello(link, now),
        }]
    }

    pub fn on_peer_lost(&mut self, link: LinkId, token: String, now: Ms) -> Vec<Action> {
        let mut acts = vec![];
        let node = self
            .sessions
            .remove(&(link, token.clone()))
            .and_then(|s| s.node);
        if let Some(l) = self.links.get_mut(&link) {
            l.peers.retain(|_, t| t != &token);
        }
        if let Some(n) = node {
            if self.neighbours.peer_down(link, n) {
                acts.push(Action::Ui(UiEvent::PeerLost { node: n }));
            }
        }
        acts.extend(self.recompute_routes(now));
        acts
    }

    // ───────────────────────── groups ─────────────────────────

    pub fn create_group(&mut self, name: &str, now: Ms) -> Result<(GroupId, Vec<Action>)> {
        let id: GroupId = identity::random_bytes();
        let secret: [u8; 32] = identity::random_bytes();
        let ikm = crypto::group_ikm(&secret, &id, crypto::KdfParams::LIGHT)?;
        let k_invite: [u8; 32] = identity::random_bytes();
        let g = self.make_group(id, name.to_string(), self.node_id(), ikm, 0, k_invite, now);
        self.groups.insert(id, g);
        self.active_group = Some(id);
        let mut acts = vec![Action::Ui(UiEvent::Joined {
            group: id,
            name: name.to_string(),
        })];
        acts.push(self.persist_groups());
        Ok((id, acts))
    }

    #[allow(clippy::too_many_arguments)]
    fn make_group(
        &self,
        id: GroupId,
        name: String,
        creator: NodeId,
        ikm: [u8; 32],
        epoch: u32,
        k_invite: [u8; 32],
        now: Ms,
    ) -> Group {
        let my_short = self.short_for(&self.node_id());
        let mut members = HashMap::new();
        members.insert(
            self.node_id(),
            Member {
                node: self.node_id(),
                name: self.cfg.display_name.clone(),
                hue: self.cfg.avatar_hue,
                pubkey: Some(self.id.ed25519_public()),
                short: my_short,
                last_seen: now,
            },
        );
        let mut short_to_node = HashMap::new();
        short_to_node.insert(my_short, self.node_id());
        Group {
            id,
            name,
            creator,
            ikm,
            epoch,
            cipher: GroupCipher::new(&crypto::epoch_key(&ikm, epoch), epoch),
            k_invite,
            full_duplex: false,
            members,
            floor: Floor::new(self.node_id(), my_short),
            handover: HashMap::new(),
            my_short,
            seq: 0,
            replay: HashMap::new(),
            jitter: HashMap::new(),
            #[cfg(feature = "opus")]
            decoders: HashMap::new(),
            short_to_node,
            joined_ms: now,
        }
    }

    fn short_for(&self, node: &NodeId) -> u16 {
        u16::from_be_bytes([node[0], node[1]])
    }

    /// Creator only: delete the group for every member (signed `Dissolve`
    /// flood), then forget it here. Members who are offline keep it until they
    /// next hear from someone who still relays the flood — or leave by hand.
    pub fn dissolve_group(&mut self, gid: GroupId, now: Ms) -> Result<Vec<Action>> {
        let me = self.node_id();
        let g = self
            .groups
            .get(&gid)
            .ok_or(Error::Invalid("unknown group"))?;
        if g.creator != me {
            return Err(Error::Invalid("only the creator can delete the group"));
        }
        let mut d = proto::Dissolve {
            group_uuid: gid.to_vec().into(),
            creator_pub: self.id.ed25519_public().to_vec().into(),
            ts_ms: now,
            signature: Default::default(),
        };
        d.signature = self.id.sign(&d.encode_to_vec()).to_vec().into();
        let mut acts = self.flood_control(ControlKind::Dissolve, d.encode_to_vec(), gid, now);
        self.groups.remove(&gid);
        if self.active_group == Some(gid) {
            self.active_group = self.groups.keys().next().copied();
        }
        acts.push(self.persist_groups());
        Ok(acts)
    }

    pub fn leave_group(&mut self, gid: GroupId, now: Ms) -> Vec<Action> {
        let mut acts = vec![];
        if self.groups.contains_key(&gid) {
            let leave = proto::Leave {
                group_uuid: gid.to_vec().into(),
                node_id: self.node_id().to_vec().into(),
            };
            acts.extend(self.flood_control(ControlKind::Leave, leave.encode_to_vec(), gid, now));
            self.groups.remove(&gid);
        }
        if self.active_group == Some(gid) {
            self.active_group = self.groups.keys().next().copied();
        }
        acts.push(self.persist_groups());
        acts
    }

    pub fn set_active_group(&mut self, gid: GroupId) {
        if self.groups.contains_key(&gid) {
            self.active_group = Some(gid);
        }
    }

    pub fn set_full_duplex(&mut self, gid: GroupId, on: bool, now: Ms) -> Vec<Action> {
        let Some(g) = self.groups.get_mut(&gid) else {
            return vec![];
        };
        g.full_duplex = on;
        let mc = proto::ModeChange {
            group_uuid: gid.to_vec().into(),
            full_duplex: on,
            by: self.node_id().to_vec().into(),
            ts_ms: now,
        };
        let mut acts = self.flood_control(ControlKind::ModeChange, mc.encode_to_vec(), gid, now);
        acts.push(Action::Ui(UiEvent::ModeChanged {
            group: gid,
            full_duplex: on,
        }));
        acts.push(Action::Capture {
            active: on,
            profile: Profile::Std,
        });
        acts
    }

    /// Current rotating code for a group (host shows it).
    pub fn current_code(&self, gid: &GroupId, now: Ms) -> Option<(String, u32)> {
        let g = self.groups.get(gid)?;
        Some((
            Code::for_slot(&g.k_invite, time::slot_index(now)).to_string(),
            crate::invite::seconds_until_rotation(now),
        ))
    }

    pub fn deep_link(&self, gid: &GroupId, now: Ms, valid_ms: u64) -> Option<String> {
        let g = self.groups.get(gid)?;
        // K_join = HKDF(ikm, "titi/v1/join-link") so the link never reveals ikm directly
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, &g.ikm);
        let mut k = [0u8; 32];
        hk.expand(b"titi/v1/join-link", &mut k).ok()?;
        Some(DeepLink::create(&self.id, g.id, k, now + valid_ms).to_url())
    }

    /// Tap-to-invite: host offers a discovered peer membership.
    pub fn invite_peer(&mut self, gid: GroupId, node: NodeId, now: Ms) -> Vec<Action> {
        let Some(g) = self.groups.get(&gid) else {
            return vec![];
        };
        let k_join: [u8; 32] = identity::random_bytes();
        self.pending_invites.insert(gid, (now + 120_000, k_join));
        let offer = proto::InviteOffer {
            group_uuid: gid.to_vec().into(),
            group_name: g.name.clone(),
            host: Some(self.my_ref()),
            k_join: k_join.to_vec().into(),
            expires_ms: now + 120_000,
            member_count: g.members.len() as u32,
        };
        // carry the group secret material inside the Noise session as a JoinResponse follow-up
        self.send_control_to(node, ControlKind::InviteOffer, offer.encode_to_vec(), now)
    }

    /// Joiner accepts a pending offer shown in UI.
    pub fn accept_invite(&mut self, gid: GroupId, host: NodeId, now: Ms) -> Vec<Action> {
        let acc = proto::InviteAccept {
            group_uuid: gid.to_vec().into(),
            member: Some(self.my_ref()),
            accepted: true,
        };
        self.send_control_to(host, ControlKind::InviteAccept, acc.encode_to_vec(), now)
    }

    pub fn decline_invite(&mut self, gid: GroupId, host: NodeId, now: Ms) -> Vec<Action> {
        let acc = proto::InviteAccept {
            group_uuid: gid.to_vec().into(),
            member: Some(self.my_ref()),
            accepted: false,
        };
        self.send_control_to(host, ControlKind::InviteAccept, acc.encode_to_vec(), now)
    }

    /// Relay rendezvous rooms a *joiner* should sit in while joining by this
    /// code (slots −1, 0, +1), or empty if the code does not parse.
    pub fn rendezvous_for_code(code_text: &str, now: Ms) -> Vec<[u8; 4]> {
        let Ok(code) = Code::parse(code_text) else {
            return vec![];
        };
        let slot = time::slot_index(now);
        [0i64, -1, 1]
            .iter()
            .map(|d| invite::rendezvous_hash(&code, (slot as i64 + d).max(0) as u64))
            .collect()
    }

    /// Relay rendezvous rooms the *host* of `gid` should sit in so joiners
    /// typing its current code can reach it (same three slots).
    pub fn rendezvous_for_group(&self, gid: &GroupId, now: Ms) -> Vec<[u8; 4]> {
        let Some(g) = self.groups.get(gid) else {
            return vec![];
        };
        let slot = time::slot_index(now);
        [0i64, -1, 1]
            .iter()
            .map(|d| {
                let s = (slot as i64 + d).max(0) as u64;
                invite::rendezvous_hash(&Code::for_slot(&g.k_invite, s), s)
            })
            .collect()
    }

    /// Join by typed code: look for a neighbour advertising a group hash we
    /// don't have, open XXpsk3 with psk derived from the code, then JoinRequest.
    pub fn join_by_code(&mut self, code_text: &str, now: Ms) -> Vec<Action> {
        let code = match Code::parse(code_text) {
            Ok(c) => c,
            Err(_) => {
                return vec![Action::Ui(UiEvent::JoinFailed {
                    reason: "invalid_code".into(),
                })]
            }
        };
        // candidates: neighbours with a group hash we aren't a member of
        let mine: HashSet<[u8; 4]> = self.groups.values().map(|g| g.hash()).collect();
        let mut acts = vec![];
        let mut tried = false;
        let cands: Vec<(NodeId, [u8; 4])> = self
            .neighbours
            .map
            .values()
            .flat_map(|n| {
                n.group_hashes
                    .iter()
                    .filter(|h| !mine.contains(*h))
                    .map(move |h| (n.node, *h))
            })
            .collect();
        for (node, gh) in cands {
            tried = true;
            let Some((link, token)) = self.best_path_to(&node) else {
                continue;
            };
            let slot = time::slot_index(now);
            // We do not know k_invite; the psk is derived from the *code text + slot*
            // on both sides (host derives code from k_invite for slots −1..+1).
            for d in [0i64, -1, 1] {
                let s = (slot as i64 + d).max(0) as u64;
                let psk = psk_from_code(&code, s);
                let mut sess = match Session::initiator(
                    Pattern::XxPsk3,
                    self.id.x25519_secret(),
                    None,
                    Some(&psk),
                ) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let req = proto::JoinRequest {
                    group_hash: gh.to_vec().into(),
                    member: Some(self.my_ref()),
                    slot_delta: d as i32,
                };
                let Ok(Some(m1)) = sess.write_handshake(&[]) else {
                    continue;
                };
                let sid = (d + 2) as u8; // 1..3
                let key = (link, format!("{token}#{sid}"));
                self.sessions.insert(
                    key.clone(),
                    PeerSession {
                        session: sess,
                        link,
                        token: token.clone(),
                        node: Some(node),
                        psk_group: None,
                        created: now,
                    },
                );
                let env = Envelope {
                    ftype: FrameType::Handshake,
                    ttl: 1,
                    hop_start: 1,
                    flags: (Pattern::XxPsk3 as u8) << flags::HS_SHIFT | flags::UNICAST,
                    msg_id: self.next_msg_id(),
                    src: self.node_id(),
                    dst: Some(node),
                    payload: &[
                        &[(d + 2) as u8, sid][..],
                        &gh,
                        &req.encode_length_delimited_to_vec(),
                        &m1,
                    ]
                    .concat(),
                };
                acts.push(Action::Send {
                    link,
                    peer: Some(token.clone()),
                    bytes: env.encode(),
                });
            }
        }
        if !tried {
            acts.push(Action::Ui(UiEvent::JoinFailed {
                reason: "no_nearby_group".into(),
            }));
        }
        acts
    }

    /// Join via deep link (QR / URL). Works offline if the creator's link is reachable
    /// or online via relay: we derive the group key locally and announce membership.
    pub fn join_by_link(&mut self, url: &str, now: Ms) -> Vec<Action> {
        let dl = match DeepLink::parse(url).and_then(|d| d.verify(now).map(|_| d)) {
            Ok(d) => d,
            Err(_) => {
                return vec![Action::Ui(UiEvent::JoinFailed {
                    reason: "invalid_link".into(),
                })]
            }
        };
        // The link carries K_join = HKDF(ikm, "titi/v1/join-link"); members verify
        // by a JoinRequest signed with K_join; they answer with the real ikm.
        let gid = dl.group_uuid;
        let creator = identity::node_id_from_pubkey(&dl.creator_pub);
        let req = proto::JoinRequest {
            group_hash: identity::group_hash(&gid).to_vec().into(),
            member: Some(self.my_ref()),
            slot_delta: 0,
        };
        let mut acts = vec![];
        // psk = K_join → XXpsk3 to any reachable member (prefer creator)
        let mut targets: Vec<NodeId> = vec![creator];
        targets.extend(
            self.neighbours
                .map
                .values()
                .filter(|n| n.group_hashes.contains(&identity::group_hash(&gid)))
                .map(|n| n.node),
        );
        targets.dedup();
        for node in targets {
            let Some((link, token)) = self.best_path_to(&node) else {
                continue;
            };
            let Ok(mut sess) = Session::initiator(
                Pattern::XxPsk3,
                self.id.x25519_secret(),
                None,
                Some(&dl.k_join),
            ) else {
                continue;
            };
            let Ok(Some(m1)) = sess.write_handshake(&[]) else {
                continue;
            };
            let sid = 9u8;
            self.sessions.insert(
                (link, format!("{token}#{sid}")),
                PeerSession {
                    session: sess,
                    link,
                    token: token.clone(),
                    node: Some(node),
                    psk_group: Some(gid),
                    created: now,
                },
            );
            let env = Envelope {
                ftype: FrameType::Handshake,
                ttl: 1,
                hop_start: 1,
                flags: (Pattern::XxPsk3 as u8) << flags::HS_SHIFT | flags::UNICAST,
                msg_id: self.next_msg_id(),
                src: self.node_id(),
                dst: Some(node),
                payload: &[
                    &[1u8, sid][..],
                    &identity::group_hash(&gid),
                    &req.encode_length_delimited_to_vec(),
                    &m1,
                ]
                .concat(),
            };
            acts.push(Action::Send {
                link,
                peer: Some(token),
                bytes: env.encode(),
            });
            break;
        }
        if acts.is_empty() {
            acts.push(Action::Ui(UiEvent::JoinFailed {
                reason: "no_member_reachable".into(),
            }));
        }
        acts
    }

    // ───────────────────────── PTT / audio ─────────────────────────

    pub fn ptt_down(&mut self, prio: Priority, now: Ms) -> Vec<Action> {
        let Some(gid) = self.active_group else {
            return vec![];
        };
        self.ptt_prio = prio;
        let Some(g) = self.groups.get_mut(&gid) else {
            return vec![];
        };
        if g.full_duplex {
            return vec![Action::Capture {
                active: true,
                profile: self.current_profile(&gid),
            }];
        }
        let evs = g.floor.ptt_down(prio, now);
        self.ptt_buffer.clear();
        self.ptt_pending = true;
        let mut acts = vec![Action::Capture {
            active: true,
            profile: self.current_profile(&gid),
        }];
        acts.extend(self.apply_floor_events(gid, evs, now));
        acts
    }

    pub fn ptt_up(&mut self, now: Ms) -> Vec<Action> {
        let Some(gid) = self.active_group else {
            return vec![];
        };
        let Some(g) = self.groups.get_mut(&gid) else {
            return vec![];
        };
        self.ptt_pending = false;
        self.ptt_buffer.clear();
        self.pcm_accum.clear();
        if g.full_duplex {
            return vec![Action::Capture {
                active: false,
                profile: Profile::Std,
            }];
        }
        let evs = g.floor.ptt_up(now);
        let mut acts = vec![Action::Capture {
            active: false,
            profile: Profile::Std,
        }];
        acts.extend(self.apply_floor_events(gid, evs, now));
        acts
    }

    /// 20 ms of 48 kHz mono PCM from the mic.
    pub fn on_audio_in(&mut self, pcm: &[i16], now: Ms) -> Vec<Action> {
        let Some(gid) = self.active_group else {
            return vec![];
        };
        let mut acts = vec![];
        if now.saturating_sub(self.last_level_ui) >= 100 {
            self.last_level_ui = now;
            acts.push(Action::Ui(UiEvent::Level {
                talker: Some(self.node_id()),
                dbfs: audio::energy_dbfs(pcm),
            }));
        }
        let Some(g) = self.groups.get(&gid) else {
            return acts;
        };
        let talking = g.full_duplex || g.floor.is_talking();
        if !talking {
            if self.ptt_pending {
                // buffer during arbitration
                self.ptt_buffer.extend_from_slice(pcm);
                if self.ptt_buffer.len() > PTT_BUFFER_MAX {
                    let excess = self.ptt_buffer.len() - PTT_BUFFER_MAX;
                    self.ptt_buffer.drain(..excess);
                }
            }
            return acts;
        }
        if g.full_duplex && audio::is_silence(pcm) && self.pcm_accum.is_empty() {
            return acts; // DTX
        }
        self.pcm_accum.extend_from_slice(pcm);
        let profile = self.current_profile(&gid);
        let need = audio::TICK_SAMPLES * (profile.frame_ms() as usize / time::TICK_MS as usize);
        while self.pcm_accum.len() >= need {
            let frame: Vec<i16> = self.pcm_accum.drain(..need).collect();
            acts.extend(self.encode_and_send(gid, &frame, profile, now));
        }
        acts
    }

    fn encode_and_send(
        &mut self,
        gid: GroupId,
        pcm: &[i16],
        profile: Profile,
        now: Ms,
    ) -> Vec<Action> {
        #[cfg(feature = "opus")]
        let payload = {
            let enc = match &mut self.encoder {
                Some(e) if e.profile == profile => e,
                _ => match audio::codec::Encoder::new(profile) {
                    Ok(e) => self.encoder.insert(e),
                    Err(e) => {
                        return vec![Action::Ui(UiEvent::Error {
                            message: e.to_string(),
                        })]
                    }
                },
            };
            match enc.encode(pcm) {
                Ok(p) => p,
                Err(e) => {
                    return vec![Action::Ui(UiEvent::Error {
                        message: e.to_string(),
                    })]
                }
            }
        };
        #[cfg(not(feature = "opus"))]
        let payload: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.send_voice_payload(gid, payload, profile, now)
    }

    /// Host-encoded Opus packet (web/WebCodecs). Applies the same floor gating
    /// as `on_audio_in` but skips PCM accumulation and the core encoder.
    pub fn on_opus_in(&mut self, packet: &[u8], now: Ms) -> Vec<Action> {
        let Some(gid) = self.active_group else {
            return vec![];
        };
        let Some(g) = self.groups.get(&gid) else {
            return vec![];
        };
        if !(g.full_duplex || g.floor.is_talking()) {
            return vec![];
        }
        let profile = self.current_profile(&gid);
        self.send_voice_payload(gid, packet.to_vec(), profile, now)
    }

    fn send_voice_payload(
        &mut self,
        gid: GroupId,
        payload: Vec<u8>,
        profile: Profile,
        now: Ms,
    ) -> Vec<Action> {
        let me = self.node_id();
        let Some(g) = self.groups.get_mut(&gid) else {
            return vec![];
        };
        let seq = g.seq;
        g.seq = g.seq.wrapping_add(1);
        let marker = seq == 0;
        let hdr = VoiceHeader {
            codec: Codec::Opus,
            frames: 1,
            marker,
            profile,
            talker: g.my_short,
            seq: seq as u16,
            ts: time::to_tick16(now),
        };
        let plain = hdr.encode(&payload);
        let nonce = crypto::voice_nonce(&me, g.epoch, seq);
        let aad = [gid.as_slice(), &g.epoch.to_be_bytes()].concat();
        let ct = g.cipher.seal(&nonce, &aad, &plain);
        // frame: epoch(4) ‖ seq32(4) ‖ ct
        let mut body = Vec::with_capacity(8 + ct.len());
        body.extend_from_slice(&g.epoch.to_be_bytes());
        body.extend_from_slice(&seq.to_be_bytes());
        body.extend_from_slice(&ct);
        self.deliver_voice(gid, body, now)
    }

    /// Route voice: unicast source-routed to each member with a confirmed
    /// path, flood TTL 3 for the rest, and the relay link if present.
    fn deliver_voice(&mut self, gid: GroupId, body: Vec<u8>, _now: Ms) -> Vec<Action> {
        let Some(g) = self.groups.get(&gid) else {
            return vec![];
        };
        let me = self.node_id();
        let members: Vec<NodeId> = g.members.keys().copied().filter(|n| *n != me).collect();
        let gh = g.hash();
        let mut acts = vec![];
        let mut flood_needed = false;
        let mut sent_links: HashSet<(LinkId, Option<String>)> = HashSet::new();
        for m in &members {
            let route = self
                .topology
                .route(&me, &self.neighbours, m, mesh::MAX_RELAYS);
            match route {
                Some(r) if r.hops() == 1 => {
                    if let Some((link, token)) = self.best_path_to(m) {
                        let key = (link, Some(token.clone()));
                        if sent_links.insert(key) {
                            let env = self.voice_env(
                                FrameType::VoiceRouted,
                                &r.path,
                                &gh,
                                &body,
                                Some(*m),
                                1,
                            );
                            acts.push(Action::Send {
                                link,
                                peer: Some(token),
                                bytes: env,
                            });
                        }
                    }
                }
                Some(r) => {
                    let first = r.path[0];
                    if let Some((link, token)) = self.best_path_to(&first) {
                        let env = self.voice_env(
                            FrameType::VoiceRouted,
                            &r.path,
                            &gh,
                            &body,
                            Some(*m),
                            r.hops() as u8,
                        );
                        acts.push(Action::Send {
                            link,
                            peer: Some(token),
                            bytes: env,
                        });
                    }
                }
                None => flood_needed = true,
            }
        }
        if flood_needed || members.is_empty() {
            let env = self.voice_env(
                FrameType::VoiceFlood,
                &[],
                &gh,
                &body,
                None,
                frame::VOICE_FLOOD_TTL,
            );
            let links: Vec<_> = self.links.values().cloned().collect();
            for l in &links {
                if l.class.is_control_only_preferred() && !flood_needed {
                    continue;
                }
                if l.class == LinkClass::Internet || l.peers.is_empty() {
                    acts.push(Action::Send {
                        link: l.id,
                        peer: None,
                        bytes: env.clone(),
                    });
                } else {
                    for token in l.peers.values() {
                        let key = (l.id, Some(token.clone()));
                        if sent_links.insert(key) {
                            acts.push(Action::Send {
                                link: l.id,
                                peer: Some(token.clone()),
                                bytes: env.clone(),
                            });
                        }
                    }
                }
            }
        }
        // always mirror to relay link so online members hear it
        let inet: Vec<LinkId> = self
            .links
            .values()
            .filter(|l| l.class == LinkClass::Internet)
            .map(|l| l.id)
            .collect();
        for lid in inet {
            if !acts
                .iter()
                .any(|a| matches!(a, Action::Send { link, .. } if *link == lid))
            {
                let env = self.voice_env(FrameType::VoiceFlood, &[], &gh, &body, None, 1);
                acts.push(Action::Send {
                    link: lid,
                    peer: None,
                    bytes: env,
                });
            }
        }
        acts
    }

    fn voice_env(
        &mut self,
        ft: FrameType,
        route: &[NodeId],
        gh: &[u8; 4],
        body: &[u8],
        dst: Option<NodeId>,
        ttl: u8,
    ) -> Vec<u8> {
        let mut payload = Vec::with_capacity(4 + 1 + route.len() * 8 + body.len());
        payload.extend_from_slice(gh);
        if ft == FrameType::VoiceRouted {
            frame::encode_route(route, &mut payload);
        }
        payload.extend_from_slice(body);
        let msg_id = self.next_msg_id();
        let env = Envelope {
            ftype: ft,
            ttl,
            hop_start: ttl,
            flags: flags::URGENT,
            msg_id,
            src: self.node_id(),
            dst,
            payload: &payload,
        };
        env.encode()
    }

    fn current_profile(&self, gid: &GroupId) -> Profile {
        let auto = self.auto_profile(gid);
        // Profile orders best→worst (Hq < Min): the ceiling is the max of the two
        self.cfg.max_profile.map_or(auto, |cap| auto.max(cap))
    }

    fn auto_profile(&self, gid: &GroupId) -> Profile {
        let Some(g) = self.groups.get(gid) else {
            return Profile::Std;
        };
        let me = self.node_id();
        let mut worst = Profile::Hq;
        let mut any = false;
        for m in g.members.keys().filter(|n| **n != me) {
            if let Some(r) = self
                .topology
                .route(&me, &self.neighbours, m, mesh::MAX_RELAYS)
            {
                any = true;
                worst = worst.max(Profile::for_bandwidth(r.min_bps));
            }
        }
        if !any {
            // only relay/internet or nobody: STD
            if self.links.values().any(|l| l.class == LinkClass::Internet) {
                return Profile::Std;
            }
            return Profile::Low;
        }
        worst
    }

    // ───────────────────────── messages ─────────────────────────

    pub fn send_text(&mut self, gid: GroupId, text: &str, now: Ms) -> Vec<Action> {
        self.send_message(gid, proto::chat_message::Body::Text(text.to_string()), now)
    }

    pub fn send_voice_note(
        &mut self,
        gid: GroupId,
        profile: Profile,
        duration_ms: u32,
        opus_packets: Vec<u8>,
        now: Ms,
    ) -> Vec<Action> {
        let vn = proto::VoiceNote {
            profile: profile as i32,
            duration_ms,
            opus_packets: opus_packets.into(),
        };
        self.send_message(gid, proto::chat_message::Body::Voice(vn), now)
    }

    pub fn send_location(
        &mut self,
        gid: GroupId,
        lat: f64,
        lon: f64,
        accuracy_m: f32,
        breadcrumb: bool,
        now: Ms,
    ) -> Vec<Action> {
        let loc = proto::Location {
            lat,
            lon,
            accuracy_m,
            breadcrumb,
            ..Default::default()
        };
        self.send_message(gid, proto::chat_message::Body::Location(loc), now)
    }

    pub fn send_sos(
        &mut self,
        gid: GroupId,
        lat: f64,
        lon: f64,
        note: &str,
        cancelled: bool,
        now: Ms,
    ) -> Vec<Action> {
        let sos = proto::Sos {
            location: Some(proto::Location {
                lat,
                lon,
                ..Default::default()
            }),
            note: note.to_string(),
            cancelled,
        };
        let mut acts = self.send_message(gid, proto::chat_message::Body::Sos(sos), now);
        if !cancelled {
            acts.extend(self.ptt_down(Priority::Emergency, now));
        }
        acts
    }

    fn send_message(
        &mut self,
        gid: GroupId,
        body: proto::chat_message::Body,
        now: Ms,
    ) -> Vec<Action> {
        let Some(g) = self.groups.get(&gid) else {
            return vec![];
        };
        let msg_uuid: [u8; 16] = identity::random_bytes();
        let cm = proto::ChatMessage {
            msg_uuid: msg_uuid.to_vec().into(),
            group_uuid: gid.to_vec().into(),
            sender_id: self.node_id().to_vec().into(),
            sent_ms: now,
            ttl_ms: crate::store::DEFAULT_TTL_MS,
            body: Some(body),
        };
        let plain = cm.encode_to_vec();
        let nonce = crypto::random_nonce();
        let aad = [gid.as_slice(), &g.epoch.to_be_bytes()].concat();
        let ct = g.cipher.seal(&nonce, &aad, &plain);
        // stored/forwarded envelope payload: gh(4) ‖ msg_uuid(16) ‖ sender(8) ‖ sent(8) ‖ exp(8) ‖ epoch(4) ‖ nonce(24) ‖ ct
        let mut p = Vec::new();
        p.extend_from_slice(&g.hash());
        p.extend_from_slice(&msg_uuid);
        p.extend_from_slice(&self.node_id());
        p.extend_from_slice(&now.to_be_bytes());
        p.extend_from_slice(&(now + crate::store::DEFAULT_TTL_MS).to_be_bytes());
        p.extend_from_slice(&g.epoch.to_be_bytes());
        p.extend_from_slice(&nonce);
        p.extend_from_slice(&ct);
        self.store.insert(Stored {
            msg_uuid,
            group_uuid: gid,
            sender: self.node_id(),
            sent_ms: now,
            expires_ms: now + crate::store::DEFAULT_TTL_MS,
            ciphertext: p.clone(),
            acked_by: Default::default(),
            forwarded_to: Default::default(),
        });
        let mut acts = self.broadcast_all(FrameType::Message, &p, self.neighbours.ttl(), now);
        if let Some(b) = self.decode_message_body(&cm) {
            acts.push(Action::Ui(UiEvent::Message {
                group: gid,
                from: self.node_id(),
                msg_uuid,
                sent_ms: now,
                body: b,
            }));
        }
        acts
    }

    fn decode_message_body(&self, cm: &proto::ChatMessage) -> Option<MessageBody> {
        Some(match cm.body.as_ref()? {
            proto::chat_message::Body::Text(t) => MessageBody::Text(t.clone()),
            proto::chat_message::Body::Voice(v) => MessageBody::VoiceNote {
                profile: Profile::from_u8(v.profile as u8).unwrap_or(Profile::Std),
                duration_ms: v.duration_ms,
                opus_packets: v.opus_packets.to_vec(),
            },
            proto::chat_message::Body::Location(l) => MessageBody::Location {
                lat_e7: (l.lat * 1e7) as i64,
                lon_e7: (l.lon * 1e7) as i64,
                accuracy_m: l.accuracy_m as u32,
                breadcrumb: l.breadcrumb,
            },
            proto::chat_message::Body::Sos(s) => {
                let l = s.location.unwrap_or_default();
                MessageBody::Sos {
                    lat_e7: (l.lat * 1e7) as i64,
                    lon_e7: (l.lon * 1e7) as i64,
                    note: s.note.clone(),
                    cancelled: s.cancelled,
                }
            }
        })
    }

    // ───────────────────────── inbound ─────────────────────────

    pub fn on_frame(&mut self, link: LinkId, token: String, bytes: &[u8], now: Ms) -> Vec<Action> {
        let env = match Envelope::decode(bytes) {
            Ok(e) => e,
            Err(_) => return vec![],
        };
        if env.src == self.node_id() {
            return vec![];
        }
        let key = env.dedup_key();
        let is_dup = !matches!(
            env.ftype,
            FrameType::Hello | FrameType::Handshake | FrameType::Control | FrameType::Inventory
        ) && self.dedup.seen(key, now);
        if is_dup {
            self.flood.cancel(&key);
            return vec![];
        }
        // remember token ↔ node on this link — only for frames that came
        // straight from their origin; a relayed frame's src is not our peer.
        if env.flags & flags::RELAYED == 0 {
            if let Some(l) = self.links.get_mut(&link) {
                l.peers.insert(env.src, token.clone());
            }
        }
        match env.ftype {
            FrameType::Hello => self.handle_hello(link, token, &env, now),
            FrameType::Announce => self.handle_announce(link, &env, now),
            FrameType::Handshake => self.handle_handshake(link, token, &env, now),
            FrameType::Control => self.handle_control(link, token, &env, now),
            FrameType::GroupControl => self.handle_group_control_flood(link, &env, now),
            FrameType::VoiceRouted | FrameType::VoiceFlood => self.handle_voice(link, &env, now),
            FrameType::Message => self.handle_message(link, &env, now),
            FrameType::Ack => self.handle_ack(link, &env, now),
            FrameType::Inventory => self.handle_inventory(link, token, &env, now),
            FrameType::Fragment => match self.reasm.push((link, env.src), env.payload, now) {
                // hop-local: never relayed; a fragment inside a fragment is refused
                Ok(Some(inner)) if inner.get(1) != Some(&(FrameType::Fragment as u8)) => {
                    self.on_frame(link, token, &inner, now)
                }
                _ => vec![],
            },
            FrameType::RouteProbe => vec![],
        }
    }

    fn handle_hello(
        &mut self,
        link: LinkId,
        token: String,
        env: &Envelope,
        now: Ms,
    ) -> Vec<Action> {
        let Ok(h) = proto::Hello::decode(env.payload) else {
            return vec![];
        };
        let Some(l) = self.links.get(&link).cloned() else {
            return vec![];
        };
        let node = env.src;
        let known = self.neighbours.map.contains_key(&node);
        let n = self.neighbours.observe(node, &l, now);
        if let Some(r) = &h.node {
            n.display_name = r.display_name.clone();
            n.avatar_hue = r.avatar_hue as u16;
            if r.pubkey.len() == 32 {
                let mut pk = [0u8; 32];
                pk.copy_from_slice(&r.pubkey);
                n.pubkey = Some(pk);
            }
        }
        n.battery_class = h.battery_class as u8;
        n.caps = h.caps;
        n.group_hashes = h
            .group_hashes
            .iter()
            .filter(|g| g.len() == 4)
            .map(|g| {
                let mut a = [0u8; 4];
                a.copy_from_slice(g);
                a
            })
            .collect();
        n.reported_neighbours = h
            .neighbour_ids
            .iter()
            .filter(|x| x.len() == 8)
            .map(|x| {
                let mut a = [0u8; 8];
                a.copy_from_slice(x);
                a
            })
            .collect();
        let name = n.display_name.clone();
        let hue = n.avatar_hue;
        let ghs = n.group_hashes.clone();
        self.peer_names.insert(node, (name.clone(), hue));
        let in_group = self.groups.values().any(|g| ghs.contains(&g.hash()));
        let mut acts = vec![Action::Ui(UiEvent::PeerDiscovered {
            node,
            name,
            hue,
            link: l.class,
            rssi_hint: 0,
            in_group,
        })];
        if !known {
            // answer with our HELLO so both sides learn each other quickly
            acts.push(Action::Send {
                link,
                peer: Some(token.clone()),
                bytes: self.build_hello(link, now),
            });
        }
        // open a Noise session if none (initiator = lower node id to avoid glare)
        if !self.sessions.contains_key(&(link, token.clone())) && self.node_id() < node {
            let remote_x = self
                .neighbours
                .map
                .get(&node)
                .and_then(|n| n.pubkey)
                .and_then(|pk| identity::ed25519_pub_to_x25519(&pk).ok());
            let pattern = if remote_x.is_some() {
                Pattern::Ik
            } else {
                Pattern::Xx
            };
            if let Ok(mut s) =
                Session::initiator(pattern, self.id.x25519_secret(), remote_x.as_ref(), None)
            {
                if let Ok(Some(m1)) = s.write_handshake(&[]) {
                    self.sessions.insert(
                        (link, token.clone()),
                        PeerSession {
                            session: s,
                            link,
                            token: token.clone(),
                            node: Some(node),
                            psk_group: None,
                            created: now,
                        },
                    );
                    let e = Envelope {
                        ftype: FrameType::Handshake,
                        ttl: 1,
                        hop_start: 1,
                        flags: (pattern as u8) << flags::HS_SHIFT | flags::UNICAST,
                        msg_id: self.next_msg_id(),
                        src: self.node_id(),
                        dst: Some(node),
                        payload: &[&[0u8, 0u8][..], &[0u8; 4], &m1].concat(),
                    };
                    acts.push(Action::Send {
                        link,
                        peer: Some(token.clone()),
                        bytes: e.encode(),
                    });
                }
            }
        }
        // members of our groups: refresh presence
        for g in self.groups.values_mut() {
            if let Some(m) = g.members.get_mut(&node) {
                m.last_seen = now;
            }
        }
        acts.extend(self.recompute_routes(now));
        acts.extend(self.maybe_sync_store(link, token, node, now));
        acts
    }

    fn handle_announce(&mut self, link: LinkId, env: &Envelope, now: Ms) -> Vec<Action> {
        let Ok(a) = proto::Announce::decode(env.payload) else {
            return vec![];
        };
        let Some(node_ref) = &a.node else {
            return vec![];
        };
        if node_ref.pubkey.len() != 32 || node_ref.node_id.len() != 8 {
            return vec![];
        }
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&node_ref.pubkey);
        let mut nid = [0u8; 8];
        nid.copy_from_slice(&node_ref.node_id);
        if identity::node_id_from_pubkey(&pk) != nid {
            return vec![];
        }
        let mut unsigned = a.clone();
        unsigned.signature = Default::default();
        if identity::verify(&pk, &unsigned.encode_to_vec(), &a.signature).is_err() {
            return vec![];
        }
        let neigh: Vec<(NodeId, LinkClass, u32)> = a
            .neighbours
            .iter()
            .filter(|n| n.node_id.len() == 8)
            .map(|n| {
                let mut id = [0u8; 8];
                id.copy_from_slice(&n.node_id);
                (
                    id,
                    LinkClass::from_u8(n.class as u8).unwrap_or(LinkClass::BleGatt),
                    n.cost,
                )
            })
            .collect();
        self.topology.update(nid, &neigh, now);
        self.peer_names.insert(
            nid,
            (node_ref.display_name.clone(), node_ref.avatar_hue as u16),
        );
        let mut acts = self.relay_flood(link, env, now, false);
        acts.extend(self.recompute_routes(now));
        acts
    }

    fn handle_handshake(
        &mut self,
        link: LinkId,
        token: String,
        env: &Envelope,
        now: Ms,
    ) -> Vec<Action> {
        let pattern = Pattern::from_u8((env.flags & flags::HS_MASK) >> flags::HS_SHIFT)
            .unwrap_or(Pattern::Xx);
        // payload: tag(1) ‖ sid(1) ‖ group_hash(4) ‖ [len-delimited JoinRequest if psk] ‖ noise msg
        // tag: 0 plain, 1 deep-link psk, 2..4 code psk (slot delta -0/-1/+1). sid: session id echoed in replies.
        if env.payload.len() < 6 {
            return vec![];
        }
        let tag = env.payload[0];
        let sid = env.payload[1];
        let mut gh = [0u8; 4];
        gh.copy_from_slice(&env.payload[2..6]);
        let body = &env.payload[6..];
        let mut acts = vec![];
        let key = if sid == 0 {
            (link, token.clone())
        } else {
            (link, format!("{token}#{sid}"))
        };
        let me = self.node_id();
        if let Some(ps) = self.sessions.get_mut(&key) {
            // continuing an existing handshake (we're initiator or responder mid-way)
            if ps.session.is_transport() {
                // stale: peer restarted; drop and treat as new
                self.sessions.remove(&key);
            } else {
                if ps.session.read_handshake(body).is_err() {
                    self.sessions.remove(&key);
                    return vec![];
                }
                if let Ok(Some(m)) = ps.session.write_handshake(&[]) {
                    let e = Envelope {
                        ftype: FrameType::Handshake,
                        ttl: 1,
                        hop_start: 1,
                        flags: (pattern as u8) << flags::HS_SHIFT | flags::UNICAST,
                        msg_id: 0,
                        src: me,
                        dst: Some(env.src),
                        payload: &[&[0u8, sid][..], &gh, &m].concat(),
                    };
                    acts.push(Action::Send {
                        link,
                        peer: Some(token.clone()),
                        bytes: e.encode(),
                    });
                }
                if ps.session.is_transport() {
                    ps.node = Some(env.src);
                    let psk_group = ps.psk_group;
                    let sess_token = key.1.clone();
                    acts.extend(
                        self.on_session_established(link, sess_token, env.src, psk_group, now),
                    );
                }
                return acts;
            }
        }
        // new responder session
        let (psk, join_req, noise_msg) = if pattern == Pattern::XxPsk3 {
            // find the group by hash and the JoinRequest to derive psk
            let rest = body;
            let Ok(req) = proto::JoinRequest::decode_length_delimited_or_whole(rest) else {
                return vec![];
            };
            let (req, consumed) = req;
            let noise_msg = &rest[consumed..];
            let Some(g) = self.groups.values().find(|g| g.hash() == gh) else {
                return vec![];
            };
            let psk = if tag == 1 {
                // deep-link join: psk = K_join = HKDF(ikm, join-link)
                let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, &g.ikm);
                let mut k = [0u8; 32];
                hk.expand(b"titi/v1/join-link", &mut k).ok();
                k
            } else {
                // tag 2,3,4 → slot delta 0,-1,+1 (joiner's view; symmetric so host uses the same)
                let d = tag as i64 - 2;
                let slot = (time::slot_index(now) as i64 + d).max(0) as u64;
                let code = Code::for_slot(&g.k_invite, slot);
                psk_from_code(&code, slot)
            };
            (Some(psk), Some(req), noise_msg)
        } else {
            (None, None, body)
        };
        let Ok(mut s) = Session::responder(pattern, self.id.x25519_secret(), psk.as_ref()) else {
            return vec![];
        };
        if s.read_handshake(noise_msg).is_err() {
            return vec![];
        }
        let gid_for_join = if psk.is_some() {
            self.groups.values().find(|g| g.hash() == gh).map(|g| g.id)
        } else {
            None
        };
        if let Ok(Some(m)) = s.write_handshake(&[]) {
            let e = Envelope {
                ftype: FrameType::Handshake,
                ttl: 1,
                hop_start: 1,
                flags: (pattern as u8) << flags::HS_SHIFT | flags::UNICAST,
                msg_id: 0,
                src: self.node_id(),
                dst: Some(env.src),
                payload: &[&[0u8, sid][..], &gh, &m].concat(),
            };
            acts.push(Action::Send {
                link,
                peer: Some(token.clone()),
                bytes: e.encode(),
            });
        }
        let established = s.is_transport();
        let sess_token = key.1.clone();
        self.sessions.insert(
            key,
            PeerSession {
                session: s,
                link,
                token: token.clone(),
                node: Some(env.src),
                psk_group: gid_for_join,
                created: now,
            },
        );
        if let (Some(req), Some(_gid)) = (join_req, gid_for_join) {
            if let Some(m) = req.member {
                self.remember_member_ref(&m, now);
            }
        }
        if established {
            acts.extend(self.on_session_established(link, sess_token, env.src, gid_for_join, now));
        }
        acts
    }

    fn remember_member_ref(&mut self, r: &proto::NodeRef, _now: Ms) {
        if r.node_id.len() == 8 {
            let mut id = [0u8; 8];
            id.copy_from_slice(&r.node_id);
            self.peer_names
                .insert(id, (r.display_name.clone(), r.avatar_hue as u16));
        }
    }

    fn on_session_established(
        &mut self,
        link: LinkId,
        token: String,
        node: NodeId,
        psk_group: Option<GroupId>,
        now: Ms,
    ) -> Vec<Action> {
        let mut acts = vec![];
        if let Some(gid) = psk_group {
            // Responder side of a join: the joiner proved the code → send JoinResponse + add member.
            if let Some(g) = self.groups.get_mut(&gid) {
                let (name, hue) = self.peer_names.get(&node).cloned().unwrap_or_default();
                let short = u16::from_be_bytes([node[0], node[1]]);
                g.members.insert(
                    node,
                    Member {
                        node,
                        name: name.clone(),
                        hue,
                        pubkey: None,
                        short,
                        last_seen: now,
                    },
                );
                g.short_to_node.insert(short, node);
                let resp = proto::JoinResponse {
                    ok: true,
                    group_uuid: gid.to_vec().into(),
                    group_name: g.name.clone(),
                    k_epoch: g.ikm.to_vec().into(), // ikm; joiner derives epoch keys
                    epoch: g.epoch,
                    members: g
                        .members
                        .values()
                        .map(|m| proto::NodeRef {
                            node_id: m.node.to_vec().into(),
                            pubkey: m.pubkey.map(|p| p.to_vec()).unwrap_or_default().into(),
                            display_name: m.name.clone(),
                            avatar_hue: m.hue as u32,
                        })
                        .collect(),
                    reason: String::new(),
                };
                let full = g.full_duplex;
                let creator = g.creator;
                let mut body = resp.encode_to_vec();
                // append k_invite + creator + full_duplex as trailer (JoinResponse has no fields for them; keep proto stable)
                body.extend_from_slice(&g.k_invite);
                body.extend_from_slice(&creator);
                body.push(full as u8);
                acts.extend(self.send_control_raw(
                    link,
                    token.clone(),
                    ControlKind::JoinResponse,
                    body,
                ));
                acts.push(Action::Ui(UiEvent::MemberJoined {
                    group: gid,
                    node,
                    name,
                }));
                acts.push(self.persist_groups());
            }
        }
        // sync store with the new session peer
        acts.extend(self.maybe_sync_store(link, token, node, now));
        acts
    }

    fn handle_control(
        &mut self,
        link: LinkId,
        token: String,
        env: &Envelope,
        now: Ms,
    ) -> Vec<Action> {
        let Some((&sid, ct)) = env.payload.split_first() else {
            return vec![];
        };
        let key = if sid == 0 {
            (link, token.clone())
        } else {
            (link, format!("{token}#{sid}"))
        };
        let plain = {
            let Some(ps) = self.sessions.get_mut(&key) else {
                return vec![];
            };
            match ps.session.decrypt(ct) {
                Ok(p) => p,
                Err(_) => return vec![],
            }
        };
        let token = key.1.clone();
        if plain.is_empty() {
            return vec![];
        }
        let kind = ControlKind::from_u8(plain[0]);
        let body = &plain[1..];
        match kind {
            Some(ControlKind::InviteOffer) => {
                let Ok(o) = proto::InviteOffer::decode(body) else {
                    return vec![];
                };
                let mut gid = [0u8; 16];
                if o.group_uuid.len() != 16 {
                    return vec![];
                }
                gid.copy_from_slice(&o.group_uuid);
                let host = o.host.clone().unwrap_or_default();
                let mut hid = [0u8; 8];
                if host.node_id.len() == 8 {
                    hid.copy_from_slice(&host.node_id)
                } else {
                    hid = env.src
                }
                vec![Action::Ui(UiEvent::InviteOffered {
                    group: gid,
                    name: o.group_name,
                    host: hid,
                    host_name: host.display_name,
                    members: o.member_count,
                })]
            }
            Some(ControlKind::InviteAccept) => {
                let Ok(a) = proto::InviteAccept::decode(body) else {
                    return vec![];
                };
                if !a.accepted || a.group_uuid.len() != 16 {
                    return vec![];
                }
                let mut gid = [0u8; 16];
                gid.copy_from_slice(&a.group_uuid);
                if let Some(m) = &a.member {
                    self.remember_member_ref(m, now);
                }
                if self.pending_invites.remove(&gid).is_none() {
                    return vec![];
                }
                self.on_session_established(link, token, env.src, Some(gid), now)
            }
            Some(ControlKind::JoinResponse) => {
                // body = JoinResponse ‖ k_invite(32) ‖ creator(8) ‖ full(1)
                if body.len() < 41 {
                    return vec![];
                }
                let (pb, trailer) = body.split_at(body.len() - 41);
                let Ok(r) = proto::JoinResponse::decode(pb) else {
                    return vec![];
                };
                if !r.ok || r.group_uuid.len() != 16 || r.k_epoch.len() != 32 {
                    return vec![Action::Ui(UiEvent::JoinFailed { reason: r.reason })];
                }
                let mut gid = [0u8; 16];
                gid.copy_from_slice(&r.group_uuid);
                let mut ikm = [0u8; 32];
                ikm.copy_from_slice(&r.k_epoch);
                let mut k_invite = [0u8; 32];
                k_invite.copy_from_slice(&trailer[..32]);
                let mut creator = [0u8; 8];
                creator.copy_from_slice(&trailer[32..40]);
                let full = trailer[40] == 1;
                let mut g = self.make_group(
                    gid,
                    r.group_name.clone(),
                    creator,
                    ikm,
                    r.epoch,
                    k_invite,
                    now,
                );
                g.full_duplex = full;
                for m in &r.members {
                    if m.node_id.len() == 8 {
                        let mut id = [0u8; 8];
                        id.copy_from_slice(&m.node_id);
                        let short = u16::from_be_bytes([id[0], id[1]]);
                        let pk = if m.pubkey.len() == 32 {
                            let mut p = [0u8; 32];
                            p.copy_from_slice(&m.pubkey);
                            Some(p)
                        } else {
                            None
                        };
                        g.members.insert(
                            id,
                            Member {
                                node: id,
                                name: m.display_name.clone(),
                                hue: m.avatar_hue as u16,
                                pubkey: pk,
                                short,
                                last_seen: now,
                            },
                        );
                        g.short_to_node.insert(short, id);
                    }
                }
                let name = g.name.clone();
                self.groups.insert(gid, g);
                self.active_group = Some(gid);
                // remove temp psk join sessions
                self.sessions.retain(|(_, t), _| !t.contains('#'));
                let mut acts = vec![
                    Action::Ui(UiEvent::Joined { group: gid, name }),
                    self.persist_groups(),
                ];
                // announce to all: JOIN flood so members add us
                let jr = proto::InviteAccept {
                    group_uuid: gid.to_vec().into(),
                    member: Some(self.my_ref()),
                    accepted: true,
                };
                acts.extend(self.flood_control(
                    ControlKind::MemberJoin,
                    jr.encode_to_vec(),
                    gid,
                    now,
                ));
                acts
            }
            Some(
                k @ (ControlKind::FloorReq
                | ControlKind::FloorTaken
                | ControlKind::FloorIdle
                | ControlKind::Leave
                | ControlKind::ModeChange
                | ControlKind::MemberJoin
                | ControlKind::KeyRotate
                | ControlKind::Dissolve),
            ) => self.handle_group_control(k, body, env.src, now),
            _ => vec![],
        }
    }

    /// gh(4) ‖ kind(1) ‖ epoch(4) ‖ nonce(24) ‖ ct
    fn handle_group_control_flood(&mut self, link: LinkId, env: &Envelope, now: Ms) -> Vec<Action> {
        const HDR: usize = 4 + 1 + 4 + 24;
        if env.payload.len() < HDR + crypto::TAG_LEN {
            return vec![];
        }
        let p = env.payload;
        let mut gh = [0u8; 4];
        gh.copy_from_slice(&p[..4]);
        let kind_u8 = p[4];
        let epoch = u32::from_be_bytes(p[5..9].try_into().unwrap());
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&p[9..33]);
        let ct = &p[33..];
        let urgent = env.flags & flags::URGENT != 0;
        let mut acts = self.relay_flood(link, env, now, urgent);
        let Some(kind) = ControlKind::from_u8(kind_u8) else {
            return acts;
        };
        let Some(gid) = self.groups.values().find(|g| g.hash() == gh).map(|g| g.id) else {
            return acts;
        };
        let g = &self.groups[&gid];
        if epoch != g.epoch {
            return acts;
        }
        let aad = [gid.as_slice(), &epoch.to_be_bytes(), &[kind_u8]].concat();
        let Ok(body) = g.cipher.open(&nonce, &aad, ct) else {
            return acts;
        };
        if kind != ControlKind::Leave
            && kind != ControlKind::Dissolve
            && kind != ControlKind::MemberJoin
            && env.src != self.node_id()
            && !self.groups[&gid].members.contains_key(&env.src)
        {
            let (name, hue) = self
                .peer_names
                .get(&env.src)
                .cloned()
                .unwrap_or_else(|| (String::new(), 200));
            let short = u16::from_be_bytes([env.src[0], env.src[1]]);
            let g = self.groups.get_mut(&gid).unwrap();
            g.members.insert(
                env.src,
                Member {
                    node: env.src,
                    name: name.clone(),
                    hue,
                    pubkey: None,
                    short,
                    last_seen: now,
                },
            );
            g.short_to_node.insert(short, env.src);
            acts.push(Action::Ui(UiEvent::MemberJoined {
                group: gid,
                node: env.src,
                name,
            }));
            acts.push(self.persist_groups());
        }
        acts.extend(self.handle_group_control(kind, &body, env.src, now));
        acts
    }

    /// Group-scoped control carried in flood frames: gh(4) ‖ kind(1) ‖ protobuf.
    fn handle_group_control(
        &mut self,
        kind: ControlKind,
        body: &[u8],
        from: NodeId,
        now: Ms,
    ) -> Vec<Action> {
        match kind {
            ControlKind::FloorReq => {
                let Ok(r) = proto::FloorRequest::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&r.group_uuid) else {
                    return vec![];
                };
                let claim = Claim {
                    node: from,
                    prio: Priority::from_u8(r.priority as u8),
                    ts: r.ts_ms,
                };
                let Some(g) = self.groups.get_mut(&gid) else {
                    return vec![];
                };
                let evs = g.floor.on_request(claim, now);
                self.apply_floor_events(gid, evs, now)
            }
            ControlKind::FloorTaken => {
                let Ok(t) = proto::FloorTaken::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&t.group_uuid) else {
                    return vec![];
                };
                let claim = Claim {
                    node: from,
                    prio: Priority::from_u8(t.priority as u8),
                    ts: t.since_ms,
                };
                let Some(g) = self.groups.get_mut(&gid) else {
                    return vec![];
                };
                g.short_to_node.insert(t.talker_short as u16, from);
                let evs = g.floor.on_taken(claim, t.talker_short as u16, now);
                self.apply_floor_events(gid, evs, now)
            }
            ControlKind::FloorIdle => {
                let Ok(i) = proto::FloorIdle::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&i.group_uuid) else {
                    return vec![];
                };
                let Some(g) = self.groups.get_mut(&gid) else {
                    return vec![];
                };
                let evs = g.floor.on_idle(from, now);
                self.apply_floor_events(gid, evs, now)
            }
            ControlKind::Leave => {
                let Ok(l) = proto::Leave::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&l.group_uuid) else {
                    return vec![];
                };
                let me = self.node_id();
                let Some(g) = self.groups.get_mut(&gid) else {
                    return vec![];
                };
                if g.members.remove(&from).is_some() {
                    let is_creator = g.creator == me;
                    let mut acts = vec![Action::Ui(UiEvent::MemberLeft {
                        group: gid,
                        node: from,
                    })];
                    // creator rotates the key
                    if is_creator {
                        acts.extend(self.rotate_key(gid, now));
                    }
                    acts.push(self.persist_groups());
                    return acts;
                }
                vec![]
            }
            ControlKind::ModeChange => {
                let Ok(m) = proto::ModeChange::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&m.group_uuid) else {
                    return vec![];
                };
                let Some(g) = self.groups.get_mut(&gid) else {
                    return vec![];
                };
                if g.full_duplex != m.full_duplex {
                    g.full_duplex = m.full_duplex;
                    return vec![Action::Ui(UiEvent::ModeChanged {
                        group: gid,
                        full_duplex: m.full_duplex,
                    })];
                }
                vec![]
            }
            ControlKind::MemberJoin => {
                let Ok(a) = proto::InviteAccept::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&a.group_uuid) else {
                    return vec![];
                };
                let Some(m) = a.member else { return vec![] };
                let Some(g) = self.groups.get_mut(&gid) else {
                    return vec![];
                };
                if g.members.contains_key(&from) {
                    return vec![];
                }
                let short = u16::from_be_bytes([from[0], from[1]]);
                let pk = if m.pubkey.len() == 32 {
                    let mut p = [0u8; 32];
                    p.copy_from_slice(&m.pubkey);
                    Some(p)
                } else {
                    None
                };
                g.members.insert(
                    from,
                    Member {
                        node: from,
                        name: m.display_name.clone(),
                        hue: m.avatar_hue as u16,
                        pubkey: pk,
                        short,
                        last_seen: now,
                    },
                );
                g.short_to_node.insert(short, from);
                vec![
                    Action::Ui(UiEvent::MemberJoined {
                        group: gid,
                        node: from,
                        name: m.display_name,
                    }),
                    self.persist_groups(),
                ]
            }
            ControlKind::KeyRotate => {
                let Ok(k) = proto::KeyRotate::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&k.group_uuid) else {
                    return vec![];
                };
                let Some(g) = self.groups.get_mut(&gid) else {
                    return vec![];
                };
                if from != g.creator || k.k_epoch.len() != 32 || k.epoch <= g.epoch {
                    return vec![];
                }
                // verify creator signature
                let Some(pk) = g.members.get(&g.creator).and_then(|m| m.pubkey) else {
                    return vec![];
                };
                let mut unsigned = k.clone();
                unsigned.signature = Default::default();
                if identity::verify(&pk, &unsigned.encode_to_vec(), &k.signature).is_err() {
                    return vec![];
                }
                let mut ikm = [0u8; 32];
                ikm.copy_from_slice(&k.k_epoch);
                g.ikm = ikm;
                g.epoch = k.epoch;
                g.cipher = GroupCipher::new(&crypto::epoch_key(&ikm, k.epoch), k.epoch);
                g.replay.clear();
                vec![self.persist_groups()]
            }
            ControlKind::Dissolve => {
                let Ok(d) = proto::Dissolve::decode(body) else {
                    return vec![];
                };
                let Some(gid) = gid_of(&d.group_uuid) else {
                    return vec![];
                };
                let Some(g) = self.groups.get(&gid) else {
                    return vec![];
                };
                let Ok(pk) = <[u8; 32]>::try_from(d.creator_pub.as_ref()) else {
                    return vec![];
                };
                // self-certifying: the key must hash to the creator's node id
                if from != g.creator || identity::node_id_from_pubkey(&pk) != g.creator {
                    return vec![];
                }
                let mut unsigned = d.clone();
                unsigned.signature = Default::default();
                if identity::verify(&pk, &unsigned.encode_to_vec(), &d.signature).is_err() {
                    return vec![];
                }
                let name = g.name.clone();
                self.groups.remove(&gid);
                if self.active_group == Some(gid) {
                    self.active_group = self.groups.keys().next().copied();
                }
                vec![
                    Action::Ui(UiEvent::GroupDissolved { group: gid, name }),
                    self.persist_groups(),
                ]
            }
            _ => vec![],
        }
    }

    fn rotate_key(&mut self, gid: GroupId, now: Ms) -> Vec<Action> {
        let me = self.node_id();
        let Some(g) = self.groups.get_mut(&gid) else {
            return vec![];
        };
        let new_ikm: [u8; 32] = identity::random_bytes();
        let epoch = g.epoch + 1;
        g.ikm = new_ikm;
        g.epoch = epoch;
        g.cipher = GroupCipher::new(&crypto::epoch_key(&new_ikm, epoch), epoch);
        g.replay.clear();
        let mut kr = proto::KeyRotate {
            group_uuid: gid.to_vec().into(),
            epoch,
            k_epoch: new_ikm.to_vec().into(),
            ts_ms: now,
            signature: Default::default(),
        };
        let sig = self.id.sign(&kr.encode_to_vec());
        kr.signature = sig.to_vec().into();
        // deliver per member over Noise sessions (never flood the key)
        let members: Vec<NodeId> = g.members.keys().copied().filter(|n| *n != me).collect();
        let body = kr.encode_to_vec();
        let mut acts = vec![];
        for m in members {
            acts.extend(self.send_control_to(m, ControlKind::KeyRotate, body.clone(), now));
        }
        acts
    }

    fn handle_voice(&mut self, link: LinkId, env: &Envelope, now: Ms) -> Vec<Action> {
        if env.payload.len() < 4 {
            return vec![];
        }
        let mut gh = [0u8; 4];
        gh.copy_from_slice(&env.payload[..4]);
        let rest = &env.payload[4..];
        let (route, body) = if env.ftype == FrameType::VoiceRouted {
            match frame::decode_route(rest) {
                Ok(x) => x,
                Err(_) => return vec![],
            }
        } else {
            (vec![], rest)
        };
        let me = self.node_id();
        let mut acts = vec![];
        // Relay?
        let for_me = env.dst.is_none_or(|d| d == me);
        if env.ftype == FrameType::VoiceRouted && !for_me {
            // forward to next hop after me
            if let Some(pos) = route.iter().position(|n| *n == me) {
                if let Some(next) = route.get(pos + 1) {
                    if let Some(r) = env.relayed() {
                        if let Some((l, tok)) = self.best_path_to(next) {
                            acts.push(Action::Send {
                                link: l,
                                peer: Some(tok),
                                bytes: r.encode(),
                            });
                        }
                    }
                }
            }
            if env.dst != Some(me) {
                return acts;
            }
        }
        if env.ftype == FrameType::VoiceFlood && self.cfg.relay_capable {
            acts.extend(self.relay_flood(link, env, now, true));
        }
        // Is this one of our groups?
        let Some(gid) = self.groups.values().find(|g| g.hash() == gh).map(|g| g.id) else {
            return acts;
        };
        if body.len() < 8 + crypto::TAG_LEN {
            return acts;
        }
        let epoch = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
        let seq32 = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
        let ct = &body[8..];
        let g = self.groups.get_mut(&gid).unwrap();
        if epoch != g.epoch {
            return acts;
        }
        let nonce = crypto::voice_nonce(&env.src, epoch, seq32);
        let aad = [gid.as_slice(), &epoch.to_be_bytes()].concat();
        let Ok(plain) = g.cipher.open(&nonce, &aad, ct) else {
            return acts;
        };
        if g.replay
            .entry(env.src)
            .or_default()
            .check_and_update(seq32)
            .is_err()
        {
            return acts;
        }
        let Ok((hdr, payload)) = VoiceHeader::decode(&plain) else {
            return acts;
        };
        g.short_to_node.insert(hdr.talker, env.src);
        // Membership convergence: a node that can produce group-AEAD voice holds the
        // group key, so it is a member even if we missed its MemberJoin flood (offline).
        let mut learned = None;
        if !g.members.contains_key(&env.src) && env.src != me {
            let (name, hue) = self
                .peer_names
                .get(&env.src)
                .cloned()
                .unwrap_or_else(|| (String::new(), 200));
            g.members.insert(
                env.src,
                Member {
                    node: env.src,
                    name: name.clone(),
                    hue,
                    pubkey: None,
                    short: hdr.talker,
                    last_seen: now,
                },
            );
            learned = Some(name);
        }
        if let Some(name) = learned {
            acts.push(Action::Ui(UiEvent::MemberJoined {
                group: gid,
                node: env.src,
                name,
            }));
            acts.push(self.persist_groups());
        }
        let g = self.groups.get_mut(&gid).unwrap();
        // floor inference
        let evs = g.floor.on_voice_from(env.src, hdr.talker, now);
        // presence
        if let Some(m) = g.members.get_mut(&env.src) {
            m.last_seen = now;
        }
        let jb = g.jitter.entry(hdr.talker).or_default();
        if hdr.marker {
            jb.reset();
        }
        // unwrap in TICK units (last_ts_abs is ms) then scale; mixing the two
        // overflowed u64 with real wall-clock timestamps.
        let ts_abs =
            time::unwrap16(jb.last_ts_abs / time::TICK_MS, hdr.ts).saturating_mul(time::TICK_MS);
        jb.push(
            Packet {
                seq: seq32 as u64,
                ts_abs,
                arrived: now,
                payload: payload.to_vec(),
                frames: hdr.frames,
                profile: hdr.profile,
            },
            now,
        );
        acts.extend(self.apply_floor_events(gid, evs, now));
        // handover ack: hearing voice back from a member confirms our route
        if let Some(h) = self
            .groups
            .get_mut(&gid)
            .unwrap()
            .handover
            .get_mut(&env.src)
        {
            for e in h.on_ack(now) {
                acts.extend(self.ho_event_to_actions(gid, e));
            }
        }
        acts
    }

    fn handle_message(&mut self, link: LinkId, env: &Envelope, now: Ms) -> Vec<Action> {
        // gh(4) ‖ msg_uuid(16) ‖ sender(8) ‖ sent(8) ‖ exp(8) ‖ epoch(4) ‖ nonce(24) ‖ ct
        const HDR: usize = 4 + 16 + 8 + 8 + 8 + 4 + 24;
        if env.payload.len() < HDR + crypto::TAG_LEN {
            return vec![];
        }
        let p = env.payload;
        let mut gh = [0u8; 4];
        gh.copy_from_slice(&p[..4]);
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&p[4..20]);
        let mut sender = [0u8; 8];
        sender.copy_from_slice(&p[20..28]);
        let sent = u64::from_be_bytes(p[28..36].try_into().unwrap());
        let exp = u64::from_be_bytes(p[36..44].try_into().unwrap());
        let epoch = u32::from_be_bytes(p[44..48].try_into().unwrap());
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&p[48..72]);
        let ct = &p[72..];
        if now >= exp {
            return vec![];
        }
        let mut acts = vec![];
        // store for relay (any group we know about the hash of, or all if relay-capable)
        let gid_opt = self.groups.values().find(|g| g.hash() == gh).map(|g| g.id);
        let store_gid = gid_opt.unwrap_or({
            let mut z = [0u8; 16];
            z[..4].copy_from_slice(&gh);
            z
        });
        let is_new = self.store.insert(Stored {
            msg_uuid: uuid,
            group_uuid: store_gid,
            sender,
            sent_ms: sent,
            expires_ms: exp,
            ciphertext: p.to_vec(),
            acked_by: Default::default(),
            forwarded_to: HashSet::from([env.src]),
        });
        if !is_new {
            return vec![];
        }
        acts.extend(self.relay_flood(link, env, now, false));
        if let Some(gid) = gid_opt {
            let g = self.groups.get(&gid).unwrap();
            if epoch == g.epoch {
                let aad = [gid.as_slice(), &epoch.to_be_bytes()].concat();
                if let Ok(plain) = g.cipher.open(&nonce, &aad, ct) {
                    if let Ok(cm) = proto::ChatMessage::decode(plain.as_slice()) {
                        if let Some(body) = self.decode_message_body(&cm) {
                            acts.push(Action::Ui(UiEvent::Message {
                                group: gid,
                                from: sender,
                                msg_uuid: uuid,
                                sent_ms: sent,
                                body,
                            }));
                            // ACK back (flood, small)
                            let ack = proto::Ack {
                                msg_uuid: uuid.to_vec().into(),
                                by: self.node_id().to_vec().into(),
                                ts_ms: now,
                            };
                            let mut ap = gh.to_vec();
                            ap.extend_from_slice(&ack.encode_to_vec());
                            acts.extend(self.broadcast_all(
                                FrameType::Ack,
                                &ap,
                                self.neighbours.ttl(),
                                now,
                            ));
                        }
                    }
                }
            }
        }
        acts
    }

    fn handle_ack(&mut self, link: LinkId, env: &Envelope, now: Ms) -> Vec<Action> {
        if env.payload.len() < 4 {
            return vec![];
        }
        let Ok(a) = proto::Ack::decode(&env.payload[4..]) else {
            return vec![];
        };
        if a.msg_uuid.len() != 16 || a.by.len() != 8 {
            return vec![];
        }
        let mut u = [0u8; 16];
        u.copy_from_slice(&a.msg_uuid);
        let mut by = [0u8; 8];
        by.copy_from_slice(&a.by);
        let mut acts = self.relay_flood(link, env, now, false);
        if self.store.ack(&u, by) {
            acts.push(Action::Ui(UiEvent::MessageAcked { msg_uuid: u, by }));
        }
        acts
    }

    fn handle_inventory(
        &mut self,
        link: LinkId,
        token: String,
        env: &Envelope,
        _now: Ms,
    ) -> Vec<Action> {
        if env.payload.len() < 4 {
            return vec![];
        }
        let Ok(inv) = proto::Inventory::decode(&env.payload[4..]) else {
            return vec![];
        };
        let has: Vec<[u8; 16]> = inv
            .msg_uuids
            .iter()
            .filter(|u| u.len() == 16)
            .map(|u| {
                let mut a = [0u8; 16];
                a.copy_from_slice(u);
                a
            })
            .collect();
        let mut gh = [0u8; 4];
        gh.copy_from_slice(&env.payload[..4]);
        // find messages for that group-hash the peer lacks
        let missing: Vec<Vec<u8>> = self
            .store
            .items
            .values()
            .filter(|s| identity::group_hash(&s.group_uuid) == gh || s.group_uuid[..4] == gh)
            .filter(|s| !has.contains(&s.msg_uuid) && !s.forwarded_to.contains(&env.src))
            .map(|s| s.ciphertext.clone())
            .collect();
        let uuids: Vec<[u8; 16]> = self
            .store
            .items
            .values()
            .filter(|s| !has.contains(&s.msg_uuid))
            .map(|s| s.msg_uuid)
            .collect();
        let mut acts = vec![];
        for c in missing {
            let e = Envelope {
                ftype: FrameType::Message,
                ttl: 1,
                hop_start: 1,
                flags: flags::RELAYED,
                msg_id: self.next_msg_id(),
                src: self.node_id(),
                dst: None,
                payload: &c,
            };
            acts.push(Action::Send {
                link,
                peer: Some(token.clone()),
                bytes: e.encode(),
            });
        }
        for u in uuids {
            self.store.mark_forwarded(&u, env.src);
        }
        acts
    }

    fn maybe_sync_store(
        &mut self,
        link: LinkId,
        token: String,
        node: NodeId,
        now: Ms,
    ) -> Vec<Action> {
        if !self.store.inventory_due(node, now) {
            return vec![];
        }
        let mut acts = vec![];
        let groups: Vec<(GroupId, [u8; 4])> =
            self.groups.values().map(|g| (g.id, g.hash())).collect();
        for (gid, gh) in groups {
            let inv = proto::Inventory {
                group_uuid: Default::default(),
                msg_uuids: self
                    .store
                    .inventory(&gid)
                    .iter()
                    .map(|u| u.to_vec().into())
                    .collect(),
            };
            let mut p = gh.to_vec();
            p.extend_from_slice(&inv.encode_to_vec());
            let e = Envelope {
                ftype: FrameType::Inventory,
                ttl: 1,
                hop_start: 1,
                flags: 0,
                msg_id: self.next_msg_id(),
                src: self.node_id(),
                dst: Some(node),
                payload: &p,
            };
            acts.push(Action::Send {
                link,
                peer: Some(token.clone()),
                bytes: e.encode(),
            });
        }
        acts
    }

    // ───────────────────────── tick ─────────────────────────

    pub fn tick(&mut self, now: Ms) -> Vec<Action> {
        let mut acts = vec![];
        // hellos
        let links: Vec<LinkId> = self.links.keys().copied().collect();
        let interval = if self.neighbours.count() == 0 {
            mesh::HELLO_ALONE_MS
        } else {
            self.next_hello_interval
        };
        for l in links {
            let last = *self.last_hello.get(&l).unwrap_or(&0);
            if now.saturating_sub(last) >= interval {
                acts.extend(self.hello_on(l, now));
            }
        }
        // announce
        if !self.groups.is_empty() || self.neighbours.count() > 0 {
            let ann_interval = self.rng.range(mesh::ANNOUNCE_MIN_MS, mesh::ANNOUNCE_MAX_MS);
            if now.saturating_sub(self.last_announce) >= ann_interval {
                self.last_announce = now;
                acts.extend(self.send_announce(now));
            }
        }
        // expiry
        for n in self.neighbours.expire(now) {
            acts.push(Action::Ui(UiEvent::PeerLost { node: n }));
        }
        self.topology.expire(now);
        self.store.expire(now);
        self.pending_invites.retain(|_, (exp, _)| *exp > now);
        self.sessions
            .retain(|_, s| s.session.is_transport() || now.saturating_sub(s.created) < 10_000);
        // flood relays due
        for p in self.flood.due(now) {
            acts.extend(self.fanout_relay(&p, true));
        }
        // routes: handover's SUSPEND timer is fed by `on_route`, which only ran on
        // hello/announce (15–30 s apart) → false "out of range" after 3 s. Re-evaluate
        // once a second; it is a Dijkstra over a handful of nodes.
        if now.saturating_sub(self.last_route_eval) >= 1_000 {
            self.last_route_eval = now;
            acts.extend(self.recompute_routes(now));
        }
        // floors & handover & playout
        let gids: Vec<GroupId> = self.groups.keys().copied().collect();
        for gid in gids {
            let evs = self.groups.get_mut(&gid).unwrap().floor.tick(now);
            acts.extend(self.apply_floor_events(gid, evs, now));
            let hos: Vec<(NodeId, Vec<HoEvent>)> = self
                .groups
                .get_mut(&gid)
                .unwrap()
                .handover
                .iter_mut()
                .map(|(n, h)| (*n, h.tick(now)))
                .collect();
            for (_, evs) in hos {
                for e in evs {
                    acts.extend(self.ho_event_to_actions(gid, e));
                }
            }
            acts.extend(self.playout(gid, now));
        }
        acts.extend(self.drain_fragments(now));
        if let Some(t) = self.flood.next_fire() {
            acts.push(Action::WakeAt(t));
        }
        acts
    }

    /// Produce one 20 ms mixed frame if any talker has audio.
    fn playout(&mut self, gid: GroupId, now: Ms) -> Vec<Action> {
        let Some(g) = self.groups.get_mut(&gid) else {
            return vec![];
        };
        #[cfg(not(feature = "opus"))]
        {
            // Host decodes: hand over packets in playout order, one per talker per tick.
            let mut acts = vec![];
            let talkers: Vec<u16> = g.jitter.keys().copied().collect();
            for t in talkers {
                let jb = g.jitter.get_mut(&t).unwrap();
                let node = g.short_to_node.get(&t).copied().unwrap_or([0u8; 8]);
                match jb.pop(now) {
                    Pop::Wait => {}
                    Pop::Packet(p) => acts.push(Action::PlayPacket {
                        talker: node,
                        packet: p.payload,
                        frames: p.frames,
                    }),
                    Pop::Lost {
                        fec_from_next: Some(nx),
                        ..
                    } => acts.push(Action::PlayPacket {
                        talker: node,
                        packet: nx.payload,
                        frames: nx.frames,
                    }),
                    Pop::Lost { .. } => acts.push(Action::PlayPacket {
                        talker: node,
                        packet: vec![],
                        frames: 1,
                    }),
                }
            }
            g.jitter.retain(|_, jb| {
                jb.buffered_ms() > 0
                    || now.saturating_sub(jb.last_ts_abs) < 2_000
                    || jb.last_ts_abs == 0
            });
            return acts;
        }
        #[cfg(feature = "opus")]
        {
            let mut streams: Vec<Vec<i16>> = vec![];
            let mut ui_level: Option<(NodeId, f32)> = None;
            let talkers: Vec<u16> = g.jitter.keys().copied().collect();
            for t in talkers {
                let jb = g.jitter.get_mut(&t).unwrap();
                match jb.pop(now) {
                    Pop::Wait => {}
                    Pop::Packet(p) => {
                        let n = audio::TICK_SAMPLES
                            * (p.profile.frame_ms() as usize / time::TICK_MS as usize);
                        let pcm = {
                            let dec = match g.decoders.entry(t) {
                                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                                std::collections::hash_map::Entry::Vacant(v) => {
                                    match audio::codec::Decoder::new() {
                                        Ok(d) => v.insert(d),
                                        Err(_) => continue,
                                    }
                                }
                            };
                            dec.decode(&p.payload, n, false)
                                .unwrap_or_else(|_| vec![0; n])
                        };
                        if let Some(node) = g.short_to_node.get(&t) {
                            ui_level = Some((*node, audio::energy_dbfs(&pcm)));
                        }
                        streams.push(pcm);
                    }
                    Pop::Lost {
                        fade,
                        fec_from_next,
                    } => {
                        if let Some(dec) = g.decoders.get_mut(&t) {
                            let n = audio::TICK_SAMPLES;
                            let mut pcm = match fec_from_next {
                                Some(nx) => dec
                                    .decode(&nx.payload, n, true)
                                    .unwrap_or_else(|_| vec![0; n]),
                                None => dec.conceal(n).unwrap_or_else(|_| vec![0; n]),
                            };
                            if fade {
                                audio::fade_out(&mut pcm, 0.5, 0.0);
                            }
                            streams.push(pcm);
                        }
                    }
                }
            }
            // drop idle jitter buffers (no packets for 2 s)
            g.jitter.retain(|_, jb| {
                jb.buffered_ms() > 0
                    || now.saturating_sub(jb.last_ts_abs) < 2_000
                    || jb.last_ts_abs == 0
            });
            if streams.is_empty() {
                return vec![];
            }
            // full-duplex: cap simultaneous talkers to MAX_TALKERS loudest
            if streams.len() > audio::MAX_TALKERS {
                streams.sort_by(|a, b| {
                    audio::energy_dbfs(b)
                        .partial_cmp(&audio::energy_dbfs(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                streams.truncate(audio::MAX_TALKERS);
            }
            let mut out = vec![
                0i16;
                streams
                    .iter()
                    .map(|s| s.len())
                    .max()
                    .unwrap_or(audio::TICK_SAMPLES)
            ];
            let refs: Vec<&[i16]> = streams.iter().map(|s| s.as_slice()).collect();
            audio::mix(&refs, &mut out);
            let mut acts = vec![Action::Play { pcm: out }];
            if let Some((node, db)) = ui_level {
                if now.saturating_sub(self.last_level_ui) >= 100 {
                    self.last_level_ui = now;
                    acts.push(Action::Ui(UiEvent::Level {
                        talker: Some(node),
                        dbfs: db,
                    }));
                }
            }
            acts
        }
    }

    // ───────────────────────── helpers ─────────────────────────

    fn my_ref(&self) -> proto::NodeRef {
        proto::NodeRef {
            node_id: self.node_id().to_vec().into(),
            pubkey: self.id.ed25519_public().to_vec().into(),
            display_name: self.cfg.display_name.clone(),
            avatar_hue: self.cfg.avatar_hue as u32,
        }
    }

    fn next_msg_id(&mut self) -> u32 {
        self.msg_counter = self.msg_counter.wrapping_add(1);
        (self.rng.next_u64() as u32) ^ self.msg_counter
    }

    fn build_hello(&mut self, link: LinkId, now: Ms) -> Vec<u8> {
        let l = self.links.get(&link).cloned();
        let h = proto::Hello {
            node: Some(self.my_ref()),
            group_hashes: self
                .groups
                .values()
                .map(|g| g.hash().to_vec().into())
                .collect(),
            link: l.as_ref().map(|l| proto::LinkInfo {
                class: l.class as i32,
                est_bps: l.stats.est_bps,
                rtt_ms: l.stats.rtt_ms,
                loss_pct: l.stats.loss_pct as u32,
            }),
            battery_class: 3,
            neighbour_ids: self
                .neighbours
                .map
                .keys()
                .take(10)
                .map(|n| n.to_vec().into())
                .collect(),
            caps: (self.cfg.relay_capable as u32)
                | 2
                | if self.links.values().any(|l| l.class == LinkClass::Internet) {
                    4
                } else {
                    0
                },
            ts_ms: now,
        };
        let env = Envelope {
            ftype: FrameType::Hello,
            ttl: 1,
            hop_start: 1,
            flags: 0,
            msg_id: self.next_msg_id(),
            src: self.node_id(),
            dst: None,
            payload: &h.encode_to_vec(),
        };
        env.encode()
    }

    fn hello_on(&mut self, link: LinkId, now: Ms) -> Vec<Action> {
        self.last_hello.insert(link, now);
        if self.neighbours.count() > 0 {
            self.next_hello_interval = self
                .rng
                .range(mesh::HELLO_CONNECTED_MIN_MS, mesh::HELLO_CONNECTED_MAX_MS);
        }
        let bytes = self.build_hello(link, now);
        vec![Action::Send {
            link,
            peer: None,
            bytes,
        }]
    }

    fn send_announce(&mut self, now: Ms) -> Vec<Action> {
        let neighbours: Vec<proto::NeighbourLink> = self
            .neighbours
            .map
            .values()
            .take(10)
            .filter_map(|n| {
                n.links
                    .values()
                    .min_by_key(|l| l.cost)
                    .map(|l| proto::NeighbourLink {
                        node_id: n.node.to_vec().into(),
                        class: l.class as i32,
                        cost: l.cost,
                    })
            })
            .collect();
        let mut a = proto::Announce {
            node: Some(self.my_ref()),
            neighbours,
            ts_ms: now,
            signature: Default::default(),
        };
        let sig = self.id.sign(&a.encode_to_vec());
        a.signature = sig.to_vec().into();
        let ttl = self.neighbours.ttl();
        let p = a.encode_to_vec();
        self.broadcast_all(FrameType::Announce, &p, ttl, now)
    }

    fn broadcast_all(&mut self, ft: FrameType, payload: &[u8], ttl: u8, _now: Ms) -> Vec<Action> {
        let env = Envelope {
            ftype: ft,
            ttl,
            hop_start: ttl,
            flags: 0,
            msg_id: self.next_msg_id(),
            src: self.node_id(),
            dst: None,
            payload,
        };
        let bytes = env.encode();
        // mark our own as seen so echoes are dropped
        self.dedup.seen(env.dedup_key(), _now);
        let mut acts = vec![];
        for l in self.links.values() {
            if l.peers.is_empty() || l.class == LinkClass::Internet {
                acts.push(Action::Send {
                    link: l.id,
                    peer: None,
                    bytes: bytes.clone(),
                });
            } else {
                for tok in l.peers.values() {
                    acts.push(Action::Send {
                        link: l.id,
                        peer: Some(tok.clone()),
                        bytes: bytes.clone(),
                    });
                }
            }
        }
        acts
    }

    /// Group control as flood: Control frames are per-link encrypted, so for
    /// flooding we use a group-AEAD'd payload inside a Message-like envelope:
    /// gh(4) ‖ kind(1) ‖ epoch(4) ‖ nonce(24) ‖ ct(kind-specific protobuf).
    fn flood_control(
        &mut self,
        kind: ControlKind,
        body: Vec<u8>,
        gid: GroupId,
        now: Ms,
    ) -> Vec<Action> {
        let Some(g) = self.groups.get(&gid) else {
            return vec![];
        };
        let nonce = crypto::random_nonce();
        let aad = [g.id.as_slice(), &g.epoch.to_be_bytes(), &[kind as u8]].concat();
        let ct = g.cipher.seal(&nonce, &aad, &body);
        let mut p = Vec::with_capacity(33 + ct.len());
        p.extend_from_slice(&g.hash());
        p.push(kind as u8);
        p.extend_from_slice(&g.epoch.to_be_bytes());
        p.extend_from_slice(&nonce);
        p.extend_from_slice(&ct);
        let ttl = self.neighbours.ttl();
        let urgent = matches!(
            kind,
            ControlKind::FloorReq | ControlKind::FloorTaken | ControlKind::FloorIdle
        );
        let env = Envelope {
            ftype: FrameType::GroupControl,
            ttl,
            hop_start: ttl,
            flags: if urgent { flags::URGENT } else { 0 },
            msg_id: self.next_msg_id(),
            src: self.node_id(),
            dst: None,
            payload: &p,
        };
        let bytes = env.encode();
        self.dedup.seen(env.dedup_key(), now);
        let mut acts = vec![];
        for l in self.links.values() {
            if l.peers.is_empty() || l.class == LinkClass::Internet {
                acts.push(Action::Send {
                    link: l.id,
                    peer: None,
                    bytes: bytes.clone(),
                });
            } else {
                for tok in l.peers.values() {
                    acts.push(Action::Send {
                        link: l.id,
                        peer: Some(tok.clone()),
                        bytes: bytes.clone(),
                    });
                }
            }
        }
        acts
    }

    /// Control frame to a specific node over its Noise session.
    fn send_control_to(
        &mut self,
        node: NodeId,
        kind: ControlKind,
        body: Vec<u8>,
        _now: Ms,
    ) -> Vec<Action> {
        let Some((link, token)) = self.best_path_to(&node) else {
            return vec![];
        };
        self.send_control_raw(link, token, kind, body)
    }

    fn send_control_raw(
        &mut self,
        link: LinkId,
        token: String,
        kind: ControlKind,
        body: Vec<u8>,
    ) -> Vec<Action> {
        let key = (link, token.clone());
        let Some(ps) = self.sessions.get_mut(&key) else {
            return vec![];
        };
        let mut plain = Vec::with_capacity(1 + body.len());
        plain.push(kind as u8);
        plain.extend_from_slice(&body);
        let Ok(ct) = ps.session.encrypt(&plain) else {
            return vec![];
        };
        let dst = ps.node;
        let wire_token = ps.token.clone();
        // sid so the receiver can pick the same session for decryption
        let sid: u8 = token
            .rsplit_once('#')
            .and_then(|(_, s)| s.parse().ok())
            .unwrap_or(0);
        let mut payload = Vec::with_capacity(1 + ct.len());
        payload.push(sid);
        payload.extend_from_slice(&ct);
        let env = Envelope {
            ftype: FrameType::Control,
            ttl: 1,
            hop_start: 1,
            flags: 0,
            msg_id: self.next_msg_id(),
            src: self.node_id(),
            dst,
            payload: &payload,
        };
        vec![Action::Send {
            link,
            peer: Some(wire_token),
            bytes: env.encode(),
        }]
    }

    fn relay_flood(
        &mut self,
        from_link: LinkId,
        env: &Envelope,
        now: Ms,
        urgent: bool,
    ) -> Vec<Action> {
        if !self.cfg.relay_capable {
            return vec![];
        }
        let Some(r) = env.relayed() else {
            return vec![];
        };
        let from_peer = self
            .links
            .get(&from_link)
            .and_then(|l| l.peers.get(&env.src).cloned());
        let mut rng = self.rng.clone();
        self.flood.schedule(
            env.dedup_key(),
            r.encode(),
            from_link,
            from_peer,
            now,
            &mut rng,
            urgent,
        );
        self.rng = rng;
        if urgent {
            // fire now
            let due = self.flood.due(now);
            let mut acts = vec![];
            for p in due {
                acts.extend(self.fanout_relay(&p, false));
            }
            return acts;
        }
        vec![]
    }

    /// Fan a relayed frame out to every link. On the ingress link, skip only
    /// the peer it came from (a LAN/BLE link with several peers must still
    /// forward to the others); a peerless broadcast ingress link is skipped
    /// entirely, except Internet which always mirrors.
    fn fanout_relay(&self, p: &mesh::PendingRelay, limit_fanout: bool) -> Vec<Action> {
        let mut acts = vec![];
        for l in self.links.values() {
            let ingress = l.id == p.exclude_link;
            if l.peers.is_empty() || l.class == LinkClass::Internet {
                if ingress && l.class != LinkClass::Internet {
                    continue;
                }
                acts.push(Action::Send {
                    link: l.id,
                    peer: None,
                    bytes: p.bytes.clone(),
                });
                continue;
            }
            let toks = l
                .peers
                .values()
                .filter(|t| !(ingress && p.exclude_peer.as_ref() == Some(*t)));
            let n = if limit_fanout {
                mesh::broadcast_fanout(l.peers.len())
            } else {
                usize::MAX
            };
            for tok in toks.take(n) {
                acts.push(Action::Send {
                    link: l.id,
                    peer: Some(tok.clone()),
                    bytes: p.bytes.clone(),
                });
            }
        }
        acts
    }

    /// Best (link, token) to reach a direct neighbour.
    fn best_path_to(&self, node: &NodeId) -> Option<(LinkId, String)> {
        let (link, _) = self.neighbours.best_link(node)?;
        let tok = self.links.get(&link)?.peers.get(node)?.clone();
        Some((link, tok))
    }

    fn recompute_routes(&mut self, now: Ms) -> Vec<Action> {
        let me = self.node_id();
        let mut acts = vec![];
        let gids: Vec<GroupId> = self.groups.keys().copied().collect();
        for gid in gids {
            let members: Vec<NodeId> = self.groups[&gid]
                .members
                .keys()
                .copied()
                .filter(|n| *n != me)
                .collect();
            for m in members {
                let r: Option<Route> =
                    self.topology
                        .route(&me, &self.neighbours, &m, mesh::MAX_RELAYS);
                let bars = r.as_ref().map(bars_for).unwrap_or(0);
                let hops = r.as_ref().map(|r| r.hops() as u8).unwrap_or(0);
                let class = r
                    .as_ref()
                    .and_then(|r| self.neighbours.best_link(&r.path[0]))
                    .map(|(_, l)| l.class);
                let g = self.groups.get_mut(&gid).unwrap();
                let h = g.handover.entry(m).or_insert_with(|| Handover::new(now));
                let evs = h.on_route(r, now);
                if let Some(c) = class {
                    // only surface changes: this runs every second
                    if self.last_peer_link.insert(m, (c, bars, hops)) != Some((c, bars, hops)) {
                        acts.push(Action::Ui(UiEvent::PeerLink {
                            node: m,
                            link: c,
                            bars,
                            hops,
                        }));
                    }
                }
                for e in evs {
                    acts.extend(self.ho_event_to_actions(gid, e));
                }
            }
        }
        acts
    }

    fn ho_event_to_actions(&self, gid: GroupId, e: HoEvent) -> Vec<Action> {
        let link_of = |r: &Route| self.neighbours.best_link(&r.path[0]).map(|(_, l)| l.class);
        match e {
            HoEvent::Bicast { alt, .. } => vec![Action::Ui(UiEvent::Handover {
                group: gid,
                state: "degraded".into(),
                link: link_of(&alt),
                profile: Profile::for_bandwidth(alt.min_bps),
            })],
            HoEvent::Switched { route, profile } => vec![Action::Ui(UiEvent::Handover {
                group: gid,
                state: "switching".into(),
                link: link_of(&route),
                profile,
            })],
            HoEvent::Stable { route } => vec![Action::Ui(UiEvent::Handover {
                group: gid,
                state: "stable".into(),
                link: link_of(&route),
                profile: Profile::for_bandwidth(route.min_bps),
            })],
            HoEvent::Suspended => vec![Action::Ui(UiEvent::Suspended { group: gid })],
            HoEvent::Resumed { .. } => vec![Action::Ui(UiEvent::Resumed { group: gid })],
        }
    }

    fn apply_floor_events(&mut self, gid: GroupId, evs: Vec<FloorEvent>, now: Ms) -> Vec<Action> {
        let mut acts = vec![];
        for e in evs {
            match e {
                FloorEvent::SendRequest(c) => {
                    let nonce = self.rng.next_u64() as u32;
                    let r = proto::FloorRequest {
                        group_uuid: gid.to_vec().into(),
                        talker_id: c.node.to_vec().into(),
                        priority: c.prio as i32,
                        ts_ms: c.ts,
                        nonce,
                    };
                    acts.extend(self.flood_control(
                        ControlKind::FloorReq,
                        r.encode_to_vec(),
                        gid,
                        now,
                    ));
                }
                FloorEvent::Granted { .. } => {
                    acts.push(Action::Ui(UiEvent::FloorGranted));
                    // flush buffered PTT audio
                    self.ptt_pending = false;
                    let buf = std::mem::take(&mut self.ptt_buffer);
                    if !buf.is_empty() {
                        let profile = self.current_profile(&gid);
                        let need = audio::TICK_SAMPLES
                            * (profile.frame_ms() as usize / time::TICK_MS as usize);
                        // mark first frame
                        let mut first = true;
                        for chunk in buf.chunks(need) {
                            if chunk.len() < need {
                                self.pcm_accum.extend_from_slice(chunk);
                                break;
                            }
                            if first {
                                first = false;
                                if let Some(g) = self.groups.get_mut(&gid) {
                                    g.seq = 0;
                                }
                            }
                            acts.extend(self.encode_and_send(gid, chunk, profile, now));
                        }
                    } else if let Some(g) = self.groups.get_mut(&gid) {
                        g.seq = 0;
                    }
                }
                FloorEvent::SendTaken => {
                    let Some(g) = self.groups.get(&gid) else {
                        continue;
                    };
                    let prio = match &g.floor.state {
                        crate::floor::State::HasFloor { prio, .. } => *prio,
                        _ => Priority::Normal,
                    };
                    let my_short = g.my_short;
                    let t = proto::FloorTaken {
                        group_uuid: gid.to_vec().into(),
                        talker_id: self.node_id().to_vec().into(),
                        talker_short: my_short as u32,
                        priority: prio as i32,
                        since_ms: now,
                        profile: self.current_profile(&gid) as i32,
                    };
                    acts.extend(self.flood_control(
                        ControlKind::FloorTaken,
                        t.encode_to_vec(),
                        gid,
                        now,
                    ));
                }
                FloorEvent::SendIdle => {
                    let i = proto::FloorIdle {
                        group_uuid: gid.to_vec().into(),
                        talker_id: self.node_id().to_vec().into(),
                        ts_ms: now,
                    };
                    acts.extend(self.flood_control(
                        ControlKind::FloorIdle,
                        i.encode_to_vec(),
                        gid,
                        now,
                    ));
                }
                FloorEvent::Denied { holder } => {
                    self.ptt_pending = false;
                    self.ptt_buffer.clear();
                    acts.push(Action::Ui(UiEvent::FloorDenied { holder }));
                }
                FloorEvent::TakenBy { holder, prio, .. } => {
                    let name = self
                        .peer_names
                        .get(&holder)
                        .map(|(n, _)| n.clone())
                        .unwrap_or_default();
                    acts.push(Action::Ui(UiEvent::FloorTaken {
                        group: gid,
                        holder,
                        name,
                        prio: prio as u8,
                    }));
                }
                FloorEvent::Idle => acts.push(Action::Ui(UiEvent::FloorIdle { group: gid })),
                FloorEvent::Revoked { by } => {
                    self.ptt_pending = false;
                    acts.push(Action::Capture {
                        active: false,
                        profile: Profile::Std,
                    });
                    acts.push(Action::Ui(UiEvent::FloorDenied { holder: by }));
                }
                FloorEvent::TalkWarning => acts.push(Action::Ui(UiEvent::TalkWarning)),
                FloorEvent::TalkTimeout => {
                    acts.push(Action::Capture {
                        active: false,
                        profile: Profile::Std,
                    });
                    acts.push(Action::Ui(UiEvent::TalkTimeout));
                }
                FloorEvent::QueueReady => {
                    // auto re-request if the user still holds the button
                    if self.ptt_pending {
                        let Some(g) = self.groups.get_mut(&gid) else {
                            continue;
                        };
                        let evs = g.floor.ptt_down(self.ptt_prio, now);
                        acts.extend(self.apply_floor_events(gid, evs, now));
                    }
                }
            }
        }
        acts
    }

    fn persist_groups(&self) -> Action {
        // compact binary: count ‖ per group: id16 ‖ ikm32 ‖ k_invite32 ‖ epoch4 ‖ creator8 ‖ full1 ‖ name_len1 ‖ name ‖ members_count1 ‖ (node8 ‖ hue2 ‖ name_len1 ‖ name)*
        let mut v = vec![self.groups.len() as u8];
        for g in self.groups.values() {
            v.extend_from_slice(&g.id);
            v.extend_from_slice(&g.ikm);
            v.extend_from_slice(&g.k_invite);
            v.extend_from_slice(&g.epoch.to_be_bytes());
            v.extend_from_slice(&g.creator);
            v.push(g.full_duplex as u8);
            let nb = g.name.as_bytes();
            v.push(nb.len().min(255) as u8);
            v.extend_from_slice(&nb[..nb.len().min(255)]);
            v.push(g.members.len().min(255) as u8);
            for m in g.members.values().take(255) {
                v.extend_from_slice(&m.node);
                v.extend_from_slice(&m.hue.to_be_bytes());
                let mb = m.name.as_bytes();
                v.push(mb.len().min(255) as u8);
                v.extend_from_slice(&mb[..mb.len().min(255)]);
            }
        }
        Action::Persist {
            key: "groups".into(),
            value: v,
        }
    }

    pub fn restore_groups(&mut self, data: &[u8], now: Ms) -> Result<()> {
        let mut i = 0;
        let rd = |i: &mut usize, n: usize| -> Result<&[u8]> {
            if *i + n > data.len() {
                return Err(Error::Truncated {
                    need: *i + n,
                    got: data.len(),
                });
            }
            let s = &data[*i..*i + n];
            *i += n;
            Ok(s)
        };
        let count = rd(&mut i, 1)?[0];
        for _ in 0..count {
            let mut id = [0u8; 16];
            id.copy_from_slice(rd(&mut i, 16)?);
            let mut ikm = [0u8; 32];
            ikm.copy_from_slice(rd(&mut i, 32)?);
            let mut kinv = [0u8; 32];
            kinv.copy_from_slice(rd(&mut i, 32)?);
            let epoch = u32::from_be_bytes(rd(&mut i, 4)?.try_into().unwrap());
            let mut creator = [0u8; 8];
            creator.copy_from_slice(rd(&mut i, 8)?);
            let full = rd(&mut i, 1)?[0] == 1;
            let nl = rd(&mut i, 1)?[0] as usize;
            let name = String::from_utf8_lossy(rd(&mut i, nl)?).to_string();
            let mc = rd(&mut i, 1)?[0];
            let mut g = self.make_group(id, name, creator, ikm, epoch, kinv, now);
            g.full_duplex = full;
            for _ in 0..mc {
                let mut node = [0u8; 8];
                node.copy_from_slice(rd(&mut i, 8)?);
                let hue = u16::from_be_bytes(rd(&mut i, 2)?.try_into().unwrap());
                let ml = rd(&mut i, 1)?[0] as usize;
                let mname = String::from_utf8_lossy(rd(&mut i, ml)?).to_string();
                let short = u16::from_be_bytes([node[0], node[1]]);
                g.members.insert(
                    node,
                    Member {
                        node,
                        name: mname.clone(),
                        hue,
                        pubkey: None,
                        short,
                        last_seen: 0,
                    },
                );
                g.short_to_node.insert(short, node);
                self.peer_names.insert(node, (mname, hue));
            }
            self.groups.insert(id, g);
        }
        if self.active_group.is_none() {
            self.active_group = self.groups.keys().next().copied();
        }
        Ok(())
    }
}

fn gid_of(b: &[u8]) -> Option<GroupId> {
    if b.len() != 16 {
        return None;
    }
    let mut g = [0u8; 16];
    g.copy_from_slice(b);
    Some(g)
}

fn bars_for(r: &Route) -> u8 {
    match r.cost {
        0..=5 => 4,
        6..=15 => 3,
        16..=40 => 2,
        _ => 1,
    }
}

/// PSK for code joins: Argon2id-free (fast path) HKDF over code text + slot;
/// the code entropy is what it is — the psk only gates the handshake, the
/// group key comes from the responder afterwards.
fn psk_from_code(code: &Code, slot: u64) -> [u8; 32] {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(b"titi/v1/code-psk"), &code.secret_bytes(slot));
    let mut k = [0u8; 32];
    hk.expand(b"psk", &mut k).expect("hkdf");
    k
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    InviteOffer = 1,
    InviteAccept = 2,
    JoinResponse = 3,
    FloorReq = 10,
    FloorTaken = 11,
    FloorIdle = 12,
    Leave = 20,
    ModeChange = 21,
    MemberJoin = 22,
    KeyRotate = 23,
    Dissolve = 24,
}

impl ControlKind {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => Self::InviteOffer,
            2 => Self::InviteAccept,
            3 => Self::JoinResponse,
            10 => Self::FloorReq,
            11 => Self::FloorTaken,
            12 => Self::FloorIdle,
            20 => Self::Leave,
            21 => Self::ModeChange,
            22 => Self::MemberJoin,
            23 => Self::KeyRotate,
            24 => Self::Dissolve,
            _ => return None,
        })
    }
}

/// Helper: decode a length-delimited protobuf at the head of a buffer and
/// return (message, bytes consumed).
trait DecodeHead: Sized {
    fn decode_length_delimited_or_whole(buf: &[u8]) -> Result<(Self, usize)>;
}

impl DecodeHead for proto::JoinRequest {
    fn decode_length_delimited_or_whole(buf: &[u8]) -> Result<(Self, usize)> {
        let mut b = buf;
        let len = prost::encoding::decode_varint(&mut b).map_err(|e| Error::Proto(e.to_string()))?
            as usize;
        let hdr = buf.len() - b.len();
        if b.len() < len {
            return Err(Error::Truncated {
                need: hdr + len,
                got: buf.len(),
            });
        }
        let m = proto::JoinRequest::decode(&b[..len])?;
        Ok((m, hdr + len))
    }
}
