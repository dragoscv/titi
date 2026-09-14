//! Store-and-forward queue for TEXT / VOICE_NOTE / LOCATION / SOS.
//! Every node keeps encrypted messages for its groups until TTL and syncs
//! inventories with neighbours; ACKs propagate back.

use std::collections::{HashMap, HashSet};

use crate::frame::NodeId;
use crate::time::Ms;

pub const DEFAULT_TTL_MS: u64 = 24 * 60 * 60 * 1000;
pub const MAX_STORE_BYTES: usize = 32 * 1024 * 1024;
pub const INVENTORY_INTERVAL_MS: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct Stored {
    pub msg_uuid: [u8; 16],
    pub group_uuid: [u8; 16],
    pub sender: NodeId,
    pub sent_ms: Ms,
    pub expires_ms: Ms,
    /// Encrypted frame payload as it travels (relays never decrypt).
    pub ciphertext: Vec<u8>,
    pub acked_by: HashSet<NodeId>,
    /// Neighbours we already forwarded to.
    pub forwarded_to: HashSet<NodeId>,
}

#[derive(Default)]
pub struct Store {
    pub items: HashMap<[u8; 16], Stored>,
    pub bytes: usize,
    last_inventory: HashMap<NodeId, Ms>,
}

impl Store {
    pub fn insert(&mut self, s: Stored) -> bool {
        if self.items.contains_key(&s.msg_uuid) {
            return false;
        }
        self.bytes += s.ciphertext.len();
        self.items.insert(s.msg_uuid, s);
        self.evict_if_needed();
        true
    }

    pub fn ack(&mut self, msg_uuid: &[u8; 16], by: NodeId) -> bool {
        if let Some(s) = self.items.get_mut(msg_uuid) {
            s.acked_by.insert(by)
        } else {
            false
        }
    }

    pub fn expire(&mut self, now: Ms) -> Vec<[u8; 16]> {
        let gone: Vec<[u8; 16]> = self.items.values().filter(|s| now >= s.expires_ms).map(|s| s.msg_uuid).collect();
        for g in &gone {
            if let Some(s) = self.items.remove(g) {
                self.bytes -= s.ciphertext.len();
            }
        }
        gone
    }

    fn evict_if_needed(&mut self) {
        while self.bytes > MAX_STORE_BYTES {
            let Some(oldest) = self.items.values().min_by_key(|s| s.sent_ms).map(|s| s.msg_uuid) else { break };
            if let Some(s) = self.items.remove(&oldest) {
                self.bytes -= s.ciphertext.len();
            }
        }
    }

    /// Message ids for a group (inventory payload).
    pub fn inventory(&self, group_uuid: &[u8; 16]) -> Vec<[u8; 16]> {
        self.items.values().filter(|s| &s.group_uuid == group_uuid).map(|s| s.msg_uuid).collect()
    }

    /// Given a peer's inventory, which of ours they lack.
    pub fn missing_for_peer(&self, group_uuid: &[u8; 16], peer_has: &[[u8; 16]]) -> Vec<&Stored> {
        let has: HashSet<&[u8; 16]> = peer_has.iter().collect();
        self.items
            .values()
            .filter(|s| &s.group_uuid == group_uuid && !has.contains(&s.msg_uuid))
            .collect()
    }

    /// Should we send an inventory to this neighbour now?
    pub fn inventory_due(&mut self, peer: NodeId, now: Ms) -> bool {
        let due = self.last_inventory.get(&peer).is_none_or(|t| now.saturating_sub(*t) >= INVENTORY_INTERVAL_MS);
        if due {
            self.last_inventory.insert(peer, now);
        }
        due
    }

    pub fn mark_forwarded(&mut self, msg_uuid: &[u8; 16], to: NodeId) {
        if let Some(s) = self.items.get_mut(msg_uuid) {
            s.forwarded_to.insert(to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: u8, group: u8, sent: Ms) -> Stored {
        Stored {
            msg_uuid: [id; 16],
            group_uuid: [group; 16],
            sender: [1; 8],
            sent_ms: sent,
            expires_ms: sent + DEFAULT_TTL_MS,
            ciphertext: vec![0; 10],
            acked_by: Default::default(),
            forwarded_to: Default::default(),
        }
    }

    #[test]
    fn insert_inventory_missing_expire() {
        let mut s = Store::default();
        assert!(s.insert(st(1, 7, 0)));
        assert!(!s.insert(st(1, 7, 0)));
        s.insert(st(2, 7, 5));
        s.insert(st(3, 8, 5));
        assert_eq!(s.inventory(&[7; 16]).len(), 2);
        let miss = s.missing_for_peer(&[7; 16], &[[1; 16]]);
        assert_eq!(miss.len(), 1);
        assert_eq!(miss[0].msg_uuid, [2; 16]);
        assert!(s.ack(&[2; 16], [9; 8]));
        assert_eq!(s.expire(DEFAULT_TTL_MS).len(), 1);
        assert_eq!(s.items.len(), 2);
    }

    #[test]
    fn inventory_due_throttles() {
        let mut s = Store::default();
        assert!(s.inventory_due([1; 8], 0));
        assert!(!s.inventory_due([1; 8], 100));
        assert!(s.inventory_due([1; 8], INVENTORY_INTERVAL_MS));
    }
}
