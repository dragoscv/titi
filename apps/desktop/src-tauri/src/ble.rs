//! BLE transport (DK-09): Windows has no L2CAP CoC API, so the desktop is a
//! GATT central against the Android peripheral (`BleTransport.kt`):
//! service 74697469-0001, RX 0004 (we write), TX 0005 (it notifies). Both
//! directions carry u16 BE length + frame, split into ATT-sized chunks; our
//! first frame is the 8-byte node id. Token = remote node id hex (from the
//! advert's service data), so a rotating MAC does not look like a new peer.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use btleplug::api::{
    Central, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::engine::{Cmd, EngineTx, TEvent};

pub const LINK_ID: u32 = 6;
pub const MTU: usize = 1000;
const SERVICE: Uuid = Uuid::from_u128(0x74697469_0001_4000_8000_746974690001);
const CH_RX: Uuid = Uuid::from_u128(0x74697469_0004_4000_8000_746974690001);
const CH_TX: Uuid = Uuid::from_u128(0x74697469_0005_4000_8000_746974690001);
const MAX_FRAME: usize = 4096;

pub struct BleHandle {
    tx: mpsc::Sender<(Option<String>, Vec<u8>)>,
    task: tokio::task::JoinHandle<()>,
}

impl BleHandle {
    pub fn send(&self, peer: Option<String>, bytes: Vec<u8>) {
        let _ = self.tx.try_send((peer, bytes));
    }
}

impl Drop for BleHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn start(rt: &tokio::runtime::Handle, ev: EngineTx, node_id: [u8; 8]) -> BleHandle {
    let (tx, rx) = mpsc::channel(128);
    let task = rt.spawn(run(ev, rx, node_id));
    BleHandle { tx, task }
}

/// u16 BE length prefix, as on the L2CAP socket.
pub fn frame(b: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(b.len() + 2);
    v.extend_from_slice(&(b.len() as u16).to_be_bytes());
    v.extend_from_slice(b);
    v
}

/// Stream reassembler: push chunks, get whole frames. Resets on a bogus length.
#[derive(Default)]
pub struct Deframer(Vec<u8>);

impl Deframer {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        self.0.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            if self.0.len() < 2 {
                break;
            }
            let len = u16::from_be_bytes([self.0[0], self.0[1]]) as usize;
            if len == 0 || len > MAX_FRAME {
                self.0.clear();
                break;
            }
            if self.0.len() < 2 + len {
                break;
            }
            out.push(self.0[2..2 + len].to_vec());
            self.0.drain(..2 + len);
        }
        out
    }
}

struct Conn {
    p: Peripheral,
    rx: Characteristic,
    chunk: usize,
}

type Conns = Arc<Mutex<HashMap<String, Conn>>>;

async fn adapter() -> Option<Adapter> {
    let m = Manager::new()
        .await
        .map_err(|e| log::warn!("ble manager: {e}"))
        .ok()?;
    m.adapters().await.ok()?.into_iter().next()
}

async fn run(ev: EngineTx, mut out: mpsc::Receiver<(Option<String>, Vec<u8>)>, node_id: [u8; 8]) {
    let Some(a) = adapter().await else {
        log::info!("ble: no adapter");
        while out.recv().await.is_some() {}
        return;
    };
    let mut events = match a.events().await {
        Ok(e) => e,
        Err(e) => {
            log::warn!("ble events: {e}");
            return;
        }
    };
    if let Err(e) = a
        .start_scan(ScanFilter {
            services: vec![SERVICE],
        })
        .await
    {
        log::warn!("ble scan: {e}");
        return;
    }
    log::info!("ble scanning for titi peripherals");
    let _ = ev.send(Cmd::T(TEvent::LinkUp(LINK_ID)));
    let conns: Conns = Arc::default();
    let connecting: Arc<Mutex<HashSet<String>>> = Arc::default();
    let by_id: Arc<Mutex<HashMap<PeripheralId, String>>> = Arc::default();
    let mut rescan = tokio::time::interval(Duration::from_secs(20));
    loop {
        tokio::select! {
            e = events.next() => match e {
                Some(CentralEvent::ServiceDataAdvertisement { id, service_data }) => {
                    let Some(nid) = service_data.get(&SERVICE).filter(|v| v.len() == 8) else { continue };
                    let token = titi_core::json::hex(nid);
                    if conns.lock().unwrap().contains_key(&token) || !connecting.lock().unwrap().insert(token.clone()) { continue }
                    let Ok(p) = a.peripheral(&id).await else { connecting.lock().unwrap().remove(&token); continue };
                    by_id.lock().unwrap().insert(id, token.clone());
                    let (ev, conns, connecting) = (ev.clone(), conns.clone(), connecting.clone());
                    tokio::spawn(async move {
                        match tokio::time::timeout(Duration::from_secs(20), connect(p, token.clone(), node_id, ev.clone(), conns.clone())).await {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => log::warn!("ble connect {token}: {e}"),
                            Err(_) => log::warn!("ble connect {token}: timeout"),
                        }
                        connecting.lock().unwrap().remove(&token);
                    });
                }
                Some(CentralEvent::DeviceDisconnected(id)) => {
                    let tok = by_id.lock().unwrap().get(&id).cloned();
                    if let Some(tok) = tok {
                        if conns.lock().unwrap().remove(&tok).is_some() {
                            log::info!("ble peer lost {tok}");
                            let _ = ev.send(Cmd::T(TEvent::PeerLost(LINK_ID, tok)));
                        }
                    }
                }
                Some(_) => {}
                None => break,
            },
            m = out.recv() => {
                let Some((peer, bytes)) = m else { break };
                let targets: Vec<(Peripheral, Characteristic, usize)> = {
                    let c = conns.lock().unwrap();
                    match &peer {
                        Some(t) => c.get(t).map(|c| vec![(c.p.clone(), c.rx.clone(), c.chunk)]).unwrap_or_default(),
                        None => c.values().map(|c| (c.p.clone(), c.rx.clone(), c.chunk)).collect(),
                    }
                };
                let data = frame(&bytes);
                for (p, rx, chunk) in targets {
                    for part in data.chunks(chunk) {
                        if let Err(e) = p.write(&rx, part, WriteType::WithoutResponse).await { log::debug!("ble write: {e}"); break }
                    }
                }
            }
            _ = rescan.tick() => {
                // WinRT watchers occasionally stop reporting; restart the scan
                let _ = a.stop_scan().await;
                let _ = a.start_scan(ScanFilter { services: vec![SERVICE] }).await;
            }
        }
    }
    let _ = a.stop_scan().await;
    let peers: Vec<(String, Conn)> = conns.lock().unwrap().drain().collect();
    for (tok, c) in peers {
        let _ = c.p.disconnect().await;
        let _ = ev.send(Cmd::T(TEvent::PeerLost(LINK_ID, tok)));
    }
    let _ = ev.send(Cmd::T(TEvent::LinkDown(LINK_ID)));
}

async fn connect(
    p: Peripheral,
    token: String,
    node_id: [u8; 8],
    ev: EngineTx,
    conns: Conns,
) -> btleplug::Result<()> {
    if !p.is_connected().await? {
        p.connect().await?
    }
    p.discover_services().await?;
    let chars = p.characteristics();
    let find = |u: Uuid| {
        chars
            .iter()
            .find(|c| c.uuid == u && c.service_uuid == SERVICE)
            .cloned()
    };
    let (Some(rx), Some(tx)) = (find(CH_RX), find(CH_TX)) else {
        let _ = p.disconnect().await;
        log::info!("ble {token}: peripheral has no data channel (old build)");
        return Ok(());
    };
    let mut notes = p.notifications().await?;
    p.subscribe(&tx).await?;
    let chunk = (p.mtu() as usize).saturating_sub(3).clamp(20, 512);
    for part in frame(&node_id).chunks(chunk) {
        p.write(&rx, part, WriteType::WithResponse).await?;
    }
    log::info!("ble peer attached {token} (att payload {chunk})");
    conns.lock().unwrap().insert(
        token.clone(),
        Conn {
            p: p.clone(),
            rx,
            chunk,
        },
    );
    let _ = ev.send(Cmd::T(TEvent::PeerSeen(LINK_ID, token.clone())));
    tokio::spawn(async move {
        let mut d = Deframer::default();
        while let Some(n) = notes.next().await {
            if n.uuid != CH_TX {
                continue;
            }
            for f in d.push(&n.value) {
                let _ = ev.send(Cmd::T(TEvent::Frame(LINK_ID, token.clone(), f)));
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deframer_reassembles_chunked_stream() {
        let a = vec![1u8; 300];
        let b = vec![2u8; 5];
        let mut s = frame(&a);
        s.extend(frame(&b));
        let mut d = Deframer::default();
        let mut got = Vec::new();
        for c in s.chunks(17) {
            got.extend(d.push(c))
        }
        assert_eq!(got, vec![a, b]);
    }

    #[test]
    fn deframer_resets_on_bogus_length() {
        let mut d = Deframer::default();
        assert!(d.push(&[0xFF, 0xFF, 1, 2]).is_empty());
        assert_eq!(d.push(&frame(&[9])), vec![vec![9u8]]);
    }
}
