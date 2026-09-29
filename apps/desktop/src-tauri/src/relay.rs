//! Internet link via the Cloud Run relay (ADR-0005), wire-compatible with the
//! web and Android `RelayTransport`: tag 0x00 = protobuf `Signal`, 0x01 =
//! opaque envelope. Rooms are 4-byte group hashes (+ rendezvous rooms).
//! Token = hex node id of the remote peer.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use prost::Message as _;
use titi_core::json::hex;
use titi_core::proto::{signal::Kind, NodeRef, RoomJoin, RoomLeave, Signal};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::engine::{Cmd, EngineTx, TEvent};

pub const LINK_ID: u32 = 8;
const TAG_SIGNAL: u8 = 0x00;
const TAG_ENVELOPE: u8 = 0x01;

pub struct Identity {
    pub node_id: Vec<u8>,
    pub name: String,
    pub hue: u32,
}

pub enum RelayCmd {
    Send(Vec<u8>),
    Rooms(Vec<([u8; 4], bool)>),
    Identity(Identity),
}

pub struct RelayHandle {
    tx: mpsc::Sender<RelayCmd>,
    task: tokio::task::JoinHandle<()>,
}

impl RelayHandle {
    pub fn send(&self, bytes: Vec<u8>) {
        let _ = self.tx.try_send(RelayCmd::Send(bytes));
    }
    pub fn set_rooms(&self, rooms: Vec<([u8; 4], bool)>) {
        let _ = self.tx.try_send(RelayCmd::Rooms(rooms));
    }
    pub fn set_identity(&self, id: Identity) {
        let _ = self.tx.try_send(RelayCmd::Identity(id));
    }
}

impl Drop for RelayHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn start(rt: &tokio::runtime::Handle, url: String, id: Identity, ev: EngineTx) -> RelayHandle {
    let (tx, rx) = mpsc::channel(256);
    let task = rt.spawn(run(url, id, ev, rx));
    RelayHandle { tx, task }
}

struct State {
    id: Identity,
    rooms: HashMap<[u8; 4], bool>,
    joined_rooms: HashSet<[u8; 4]>,
    resume: Vec<u8>,
    peers: HashSet<String>,
    joined: bool,
}

fn signal_bytes(k: Kind) -> Vec<u8> {
    let mut out = vec![TAG_SIGNAL];
    Signal { kind: Some(k) }.encode(&mut out).expect("vec grows");
    out
}

fn join_msgs(st: &mut State) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for (h, rdv) in &st.rooms {
        if st.joined_rooms.contains(h) {
            continue;
        }
        out.push(signal_bytes(Kind::RoomJoin(RoomJoin {
            group_hash: h.to_vec().into(),
            node: Some(NodeRef { node_id: st.id.node_id.clone().into(), pubkey: Default::default(), display_name: st.id.name.clone(), avatar_hue: st.id.hue }),
            resume_token: st.resume.clone().into(),
            rendezvous: *rdv,
        })));
        st.joined_rooms.insert(*h);
    }
    out
}

async fn run(url: String, id: Identity, ev: EngineTx, mut rx: mpsc::Receiver<RelayCmd>) {
    let mut st = State { id, rooms: HashMap::new(), joined_rooms: HashSet::new(), resume: Vec::new(), peers: HashSet::new(), joined: false };
    let mut attempt: u32 = 0;
    loop {
        // drain commands that arrived while disconnected (keep rooms/identity, drop voice)
        while let Ok(c) = rx.try_recv() {
            match c {
                RelayCmd::Rooms(r) => st.rooms = r.into_iter().collect(),
                RelayCmd::Identity(i) => st.id = i,
                RelayCmd::Send(_) => {}
            }
        }
        let conn = tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::connect_async(url.as_str())).await;
        let ws = match conn {
            Ok(Ok((ws, _))) => ws,
            other => {
                if let Ok(Err(e)) = other { log::info!("relay connect: {e}"); }
                attempt += 1;
                let backoff = (500u64 << attempt.min(6)).min(30_000);
                // stay responsive to room/identity updates while waiting
                let sleep = tokio::time::sleep(Duration::from_millis(backoff));
                tokio::pin!(sleep);
                loop {
                    tokio::select! {
                        _ = &mut sleep => break,
                        c = rx.recv() => match c {
                            None => return,
                            Some(RelayCmd::Rooms(r)) => st.rooms = r.into_iter().collect(),
                            Some(RelayCmd::Identity(i)) => st.id = i,
                            Some(RelayCmd::Send(_)) => {}
                        }
                    }
                }
                continue;
            }
        };
        attempt = 0;
        log::info!("relay connected {url}");
        let (mut sink, mut stream) = ws.split();
        st.joined_rooms.clear();
        for m in join_msgs(&mut st) {
            let _ = sink.send(Message::Binary(m.into())).await;
        }
        let mut ping = tokio::time::interval(Duration::from_secs(25));
        ping.tick().await;
        loop {
            tokio::select! {
                m = stream.next() => {
                    let Some(Ok(m)) = m else { break };
                    let Message::Binary(b) = m else { continue };
                    if b.is_empty() { continue; }
                    match b[0] {
                        TAG_ENVELOPE => {
                            if b.len() < 1 + 16 { continue; }
                            let src = hex(&b[9..17]);
                            if st.peers.insert(src.clone()) { let _ = ev.send(Cmd::T(TEvent::PeerSeen(LINK_ID, src.clone()))); }
                            let _ = ev.send(Cmd::T(TEvent::Frame(LINK_ID, src, b[1..].to_vec())));
                        }
                        TAG_SIGNAL => {
                            let Ok(sig) = Signal::decode(&b[1..]) else { continue };
                            match sig.kind {
                                Some(Kind::RoomJoined(j)) => {
                                    st.resume = j.resume_token.to_vec();
                                    for p in j.peers {
                                        let id = hex(&p.node_id);
                                        if st.peers.insert(id.clone()) { let _ = ev.send(Cmd::T(TEvent::PeerSeen(LINK_ID, id))); }
                                    }
                                    if !st.joined {
                                        st.joined = true;
                                        let _ = ev.send(Cmd::T(TEvent::LinkUp(LINK_ID)));
                                    }
                                }
                                Some(Kind::PeerEvent(pe)) => {
                                    let Some(n) = pe.node else { continue };
                                    let id = hex(&n.node_id);
                                    if pe.joined {
                                        if st.peers.insert(id.clone()) { let _ = ev.send(Cmd::T(TEvent::PeerSeen(LINK_ID, id))); }
                                    } else if st.peers.remove(&id) {
                                        let _ = ev.send(Cmd::T(TEvent::PeerLost(LINK_ID, id)));
                                    }
                                }
                                Some(Kind::Error(e)) => {
                                    log::warn!("relay error {}: {}", e.code, e.message);
                                    if e.code == 5 {
                                        st.resume.clear();
                                        st.joined_rooms.clear();
                                        for m in join_msgs(&mut st) { let _ = sink.send(Message::Binary(m.into())).await; }
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                c = rx.recv() => {
                    let Some(c) = c else { return };
                    match c {
                        RelayCmd::Send(bytes) => {
                            if !st.joined { continue; }
                            let mut out = Vec::with_capacity(bytes.len() + 1);
                            out.push(TAG_ENVELOPE);
                            out.extend_from_slice(&bytes);
                            if sink.send(Message::Binary(out.into())).await.is_err() { break; }
                        }
                        RelayCmd::Rooms(r) => {
                            let want: HashMap<[u8; 4], bool> = r.into_iter().collect();
                            if want == st.rooms { continue; }
                            let gone: Vec<[u8; 4]> = st.rooms.keys().filter(|k| !want.contains_key(*k)).copied().collect();
                            for h in gone {
                                st.joined_rooms.remove(&h);
                                let _ = sink.send(Message::Binary(signal_bytes(Kind::RoomLeave(RoomLeave { group_hash: h.to_vec().into() })).into())).await;
                            }
                            // rendezvous flag flips need a re-join
                            for (k, v) in &want { if st.rooms.get(k) != Some(v) { st.joined_rooms.remove(k); } }
                            st.rooms = want;
                            for m in join_msgs(&mut st) { let _ = sink.send(Message::Binary(m.into())).await; }
                        }
                        RelayCmd::Identity(i) => st.id = i,
                    }
                }
                _ = ping.tick() => {
                    if sink.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
                }
            }
        }
        log::info!("relay disconnected");
        if st.joined {
            st.joined = false;
            for p in st.peers.drain() {
                let _ = ev.send(Cmd::T(TEvent::PeerLost(LINK_ID, p)));
            }
            let _ = ev.send(Cmd::T(TEvent::LinkDown(LINK_ID)));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
