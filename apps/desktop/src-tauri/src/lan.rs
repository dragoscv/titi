//! Same-LAN transport, wire-compatible with android `LanTransport`: UDP unicast
//! and IPv4 multicast 239.77.84.84:41414, TTL 1. Discovery is the engine's own
//! HELLO multicast. Token = "ip:port".
//!
//! Windows has many virtual adapters (Hyper-V, WSL, VPN) — we join the group
//! on every IPv4 interface and send broadcasts out of each one, re-checking
//! the interface set every few seconds (Wi-Fi changes, laptop resume).

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, SockRef, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::engine::{Cmd, EngineTx, TEvent};

pub const LINK_ID: u32 = 1;
pub const PORT: u16 = 41414;
pub const GROUP: Ipv4Addr = Ipv4Addr::new(239, 77, 84, 84);
const PEER_EXPIRY: Duration = Duration::from_secs(40);

pub struct LanHandle {
    tx: mpsc::Sender<(Option<String>, Vec<u8>)>,
    task: tokio::task::JoinHandle<()>,
}

impl LanHandle {
    /// Drops under pressure: stale voice is worthless.
    pub fn send(&self, peer: Option<String>, bytes: Vec<u8>) {
        let _ = self.tx.try_send((peer, bytes));
    }
}

impl Drop for LanHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn start(rt: &tokio::runtime::Handle, ev: EngineTx) -> LanHandle {
    let (tx, rx) = mpsc::channel(128);
    let task = rt.spawn(run(ev, rx));
    LanHandle { tx, task }
}

fn local_ipv4s() -> Vec<Ipv4Addr> {
    let mut v: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.ip() {
            IpAddr::V4(a) if !a.is_loopback() && !a.is_link_local() && !a.is_unspecified() => Some(a),
            _ => None,
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

fn bind() -> std::io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    s.set_nonblocking(true)?;
    s.set_recv_buffer_size(256 * 1024)?;
    s.set_multicast_ttl_v4(1)?;
    s.set_multicast_loop_v4(false)?;
    s.bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, PORT)).into())?;
    UdpSocket::from_std(s.into())
}

async fn run(ev: EngineTx, mut out: mpsc::Receiver<(Option<String>, Vec<u8>)>) {
    loop {
        let sock = match bind() {
            Ok(s) => s,
            Err(e) => {
                log::warn!("lan bind failed: {e}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        let mut joined: HashSet<Ipv4Addr> = HashSet::new();
        let mut ifaces = local_ipv4s();
        rejoin(&sock, &ifaces, &mut joined);
        log::info!("lan bound :{PORT} on {} interface(s): {:?}", joined.len(), joined);
        let mut up = !joined.is_empty();
        if up {
            let _ = ev.send(Cmd::T(TEvent::LinkUp(LINK_ID)));
        }
        let mut peers: HashMap<String, Instant> = HashMap::new();
        let mut buf = vec![0u8; 2048];
        let mut housekeeping = tokio::time::interval(Duration::from_secs(4));
        let failed = loop {
            tokio::select! {
                r = sock.recv_from(&mut buf) => match r {
                    Ok((n, SocketAddr::V4(from))) => {
                        if from.port() == PORT && ifaces.contains(from.ip()) { continue; } // our own echo
                        let token = from.to_string();
                        if peers.insert(token.clone(), Instant::now()).is_none() {
                            let _ = ev.send(Cmd::T(TEvent::PeerSeen(LINK_ID, token.clone())));
                        }
                        let _ = ev.send(Cmd::T(TEvent::Frame(LINK_ID, token, buf[..n].to_vec())));
                    }
                    Ok(_) => {}
                    // Windows reports ICMP port-unreachable of a previous send as a recv error
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                    Err(e) => { log::warn!("lan rx: {e}"); break true; }
                },
                m = out.recv() => {
                    let Some((peer, bytes)) = m else { return };
                    send(&sock, &ifaces, peer, &bytes).await;
                }
                _ = housekeeping.tick() => {
                    let now = Instant::now();
                    peers.retain(|tok, seen| {
                        let keep = now.duration_since(*seen) < PEER_EXPIRY;
                        if !keep { let _ = ev.send(Cmd::T(TEvent::PeerLost(LINK_ID, tok.clone()))); }
                        keep
                    });
                    let cur = local_ipv4s();
                    if cur != ifaces {
                        log::info!("lan interfaces changed: {cur:?}");
                        ifaces = cur;
                        joined.retain(|a| ifaces.contains(a));
                        rejoin(&sock, &ifaces, &mut joined);
                        let now_up = !joined.is_empty();
                        if now_up != up {
                            up = now_up;
                            let _ = ev.send(Cmd::T(if up { TEvent::LinkUp(LINK_ID) } else { TEvent::LinkDown(LINK_ID) }));
                        }
                    }
                }
            }
        };
        if up {
            for tok in peers.keys() {
                let _ = ev.send(Cmd::T(TEvent::PeerLost(LINK_ID, tok.clone())));
            }
            let _ = ev.send(Cmd::T(TEvent::LinkDown(LINK_ID)));
        }
        if failed {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

fn rejoin(sock: &UdpSocket, ifaces: &[Ipv4Addr], joined: &mut HashSet<Ipv4Addr>) {
    let s = SockRef::from(sock);
    for ip in ifaces {
        if joined.contains(ip) {
            continue;
        }
        match s.join_multicast_v4(&GROUP, ip) {
            Ok(()) => {
                joined.insert(*ip);
            }
            Err(e) => log::debug!("join {GROUP} on {ip}: {e}"),
        }
    }
}

async fn send(sock: &UdpSocket, ifaces: &[Ipv4Addr], peer: Option<String>, bytes: &[u8]) {
    match peer {
        Some(tok) => {
            if let Ok(addr) = tok.parse::<SocketAddr>() {
                if let Err(e) = sock.send_to(bytes, addr).await {
                    log::debug!("lan tx {addr}: {e}");
                }
            }
        }
        None => {
            let dst = SocketAddr::V4(SocketAddrV4::new(GROUP, PORT));
            let s = SockRef::from(sock);
            for ip in ifaces {
                if s.set_multicast_if_v4(ip).is_ok() {
                    let _ = sock.send_to(bytes, dst).await;
                }
            }
        }
    }
}
