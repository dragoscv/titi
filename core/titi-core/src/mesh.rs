//! Mesh: dedup, neighbour table, topology map, route computation, flood scheduler.

use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};

use lru::LruCache;

use crate::frame::{NodeId, DEFAULT_TTL, DENSE_TTL};
use crate::link::{Link, LinkClass, LinkId};
use crate::time::Ms;

pub const DEDUP_CAPACITY: usize = 2048;
pub const DEDUP_TTL_MS: u64 = 5 * 60 * 1000;
pub const HELLO_ALONE_MS: u64 = 4_000;
pub const HELLO_CONNECTED_MIN_MS: u64 = 15_000;
pub const HELLO_CONNECTED_MAX_MS: u64 = 30_000;
pub const ANNOUNCE_MIN_MS: u64 = 30_000;
pub const ANNOUNCE_MAX_MS: u64 = 60_000;
pub const NEIGHBOUR_EXPIRY_MS: u64 = 60_000;
pub const TOPOLOGY_EXPIRY_MS: u64 = 120_000;
pub const FLOOD_JITTER_MIN_MS: u64 = 10;
pub const FLOOD_JITTER_MAX_MS: u64 = 220;
pub const MAX_RELAYS: usize = 3;
pub const MAX_ROUTE_LATENCY_MS: u32 = 400;
pub const PER_HOP_BUDGET_MS: u32 = 60;
pub const DENSE_LINK_THRESHOLD: usize = 6;

/// Deterministic-jitter PRNG (xorshift) so tests are reproducible.
#[derive(Debug, Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next_u64() % (hi - lo + 1)
    }
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        (self.next_u64() % den as u64) < num as u64
    }
}

pub struct Dedup {
    cache: LruCache<[u8; 12], Ms>,
}

impl Default for Dedup {
    fn default() -> Self {
        Dedup { cache: LruCache::new(std::num::NonZeroUsize::new(DEDUP_CAPACITY).unwrap()) }
    }
}

impl Dedup {
    /// Returns true if the key was already seen (within TTL).
    pub fn seen(&mut self, key: [u8; 12], now: Ms) -> bool {
        if let Some(t) = self.cache.get(&key) {
            if now.saturating_sub(*t) < DEDUP_TTL_MS {
                return true;
            }
        }
        self.cache.put(key, now);
        false
    }
}

#[derive(Debug, Clone)]
pub struct Neighbour {
    pub node: NodeId,
    pub links: BTreeMap<LinkId, NeighbourLink>,
    pub last_seen: Ms,
    pub display_name: String,
    pub avatar_hue: u16,
    pub pubkey: Option<[u8; 32]>,
    pub battery_class: u8,
    pub caps: u32,
    pub group_hashes: Vec<[u8; 4]>,
    pub reported_neighbours: Vec<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeighbourLink {
    pub class: LinkClass,
    pub cost: u32,
    pub latency_ms: u32,
    pub bps: u32,
    pub last_seen: Ms,
}

#[derive(Default)]
pub struct NeighbourTable {
    pub map: HashMap<NodeId, Neighbour>,
}

impl NeighbourTable {
    pub fn observe(&mut self, node: NodeId, link: &Link, now: Ms) -> &mut Neighbour {
        let n = self.map.entry(node).or_insert_with(|| Neighbour {
            node,
            links: BTreeMap::new(),
            last_seen: now,
            display_name: String::new(),
            avatar_hue: 0,
            pubkey: None,
            battery_class: 0,
            caps: 0,
            group_hashes: vec![],
            reported_neighbours: vec![],
        });
        n.last_seen = now;
        n.links.insert(
            link.id,
            NeighbourLink {
                class: link.class,
                cost: link.cost(),
                latency_ms: link.est_latency_ms(),
                bps: link.stats.est_bps,
                last_seen: now,
            },
        );
        n
    }

    pub fn link_down(&mut self, link_id: LinkId) -> Vec<NodeId> {
        let mut lost = vec![];
        for n in self.map.values_mut() {
            n.links.remove(&link_id);
            if n.links.is_empty() {
                lost.push(n.node);
            }
        }
        for id in &lost {
            self.map.remove(id);
        }
        lost
    }

    pub fn peer_down(&mut self, link_id: LinkId, node: NodeId) -> bool {
        if let Some(n) = self.map.get_mut(&node) {
            n.links.remove(&link_id);
            if n.links.is_empty() {
                self.map.remove(&node);
                return true;
            }
        }
        false
    }

    /// Expire stale neighbours; returns the ones removed.
    pub fn expire(&mut self, now: Ms) -> Vec<NodeId> {
        let mut gone = vec![];
        for n in self.map.values_mut() {
            n.links.retain(|_, l| now.saturating_sub(l.last_seen) < NEIGHBOUR_EXPIRY_MS);
            if n.links.is_empty() {
                gone.push(n.node);
            }
        }
        for id in &gone {
            self.map.remove(id);
        }
        gone
    }

    pub fn best_link(&self, node: &NodeId) -> Option<(LinkId, NeighbourLink)> {
        self.map
            .get(node)?
            .links
            .iter()
            .min_by_key(|(_, l)| l.cost)
            .map(|(id, l)| (*id, *l))
    }

    pub fn count(&self) -> usize {
        self.map.len()
    }

    pub fn total_links(&self) -> usize {
        self.map.values().map(|n| n.links.len()).sum()
    }

    pub fn ttl(&self) -> u8 {
        if self.total_links() >= DENSE_LINK_THRESHOLD { DENSE_TTL } else { DEFAULT_TTL }
    }
}

/// 2-hop topology built from ANNOUNCEs (plus our own neighbours).
#[derive(Default)]
pub struct Topology {
    /// node → (neighbour → (class, cost)), with freshness
    pub adj: HashMap<NodeId, (Ms, HashMap<NodeId, (LinkClass, u32)>)>,
}

impl Topology {
    pub fn update(&mut self, node: NodeId, neighbours: &[(NodeId, LinkClass, u32)], now: Ms) {
        let entry = self.adj.entry(node).or_insert_with(|| (now, HashMap::new()));
        entry.0 = now;
        entry.1.clear();
        for (n, c, cost) in neighbours {
            entry.1.insert(*n, (*c, *cost));
        }
    }

    pub fn expire(&mut self, now: Ms) {
        self.adj.retain(|_, (t, _)| now.saturating_sub(*t) < TOPOLOGY_EXPIRY_MS);
    }

    /// Dijkstra on summed cost. Bidirectional confirmation: an edge counts
    /// only if both ends report it (or one end is `self_id`, whose local view
    /// is authoritative). Returns full path excluding `from`, including `to`.
    pub fn route(
        &self,
        self_id: &NodeId,
        local: &NeighbourTable,
        to: &NodeId,
        max_relays: usize,
    ) -> Option<Route> {
        if let Some((_, l)) = local.best_link(to) {
            return Some(Route { path: vec![*to], cost: l.cost, latency_ms: l.latency_ms, min_bps: l.bps });
        }
        #[derive(PartialEq, Eq)]
        struct St(u32, NodeId, Vec<NodeId>, u32, u32); // cost, node, path, latency, min_bps
        impl Ord for St {
            fn cmp(&self, o: &Self) -> std::cmp::Ordering {
                o.0.cmp(&self.0)
            }
        }
        impl PartialOrd for St {
            fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(o))
            }
        }
        let mut heap = BinaryHeap::new();
        let mut best: HashMap<NodeId, u32> = HashMap::new();
        for (n, nb) in &local.map {
            if let Some((_, l)) = nb.links.iter().min_by_key(|(_, l)| l.cost) {
                heap.push(St(l.cost, *n, vec![*n], l.latency_ms, l.bps));
            }
        }
        while let Some(St(cost, node, path, lat, minbps)) = heap.pop() {
            if &node == to {
                return Some(Route { path, cost, latency_ms: lat, min_bps: minbps });
            }
            if best.get(&node).is_some_and(|c| *c <= cost) {
                continue;
            }
            best.insert(node, cost);
            if path.len() > max_relays {
                continue;
            }
            let Some((_, edges)) = self.adj.get(&node) else { continue };
            for (next, (class, ecost)) in edges {
                if next == self_id || path.contains(next) {
                    continue;
                }
                // bidirectional confirmation
                let confirmed = self
                    .adj
                    .get(next)
                    .map(|(_, e)| e.contains_key(&node))
                    .unwrap_or(false)
                    || local.map.contains_key(next);
                if !confirmed {
                    continue;
                }
                let est_lat = class_latency(*class);
                let est_bps = class_default_bps(*class);
                let nl = lat + est_lat;
                if nl > MAX_ROUTE_LATENCY_MS {
                    continue;
                }
                let mut p = path.clone();
                p.push(*next);
                heap.push(St(cost + ecost, *next, p, nl, minbps.min(est_bps)));
            }
        }
        None
    }

    /// Nodes known anywhere in the map (for UI constellation).
    pub fn known_nodes(&self) -> HashSet<NodeId> {
        let mut s = HashSet::new();
        for (n, (_, e)) in &self.adj {
            s.insert(*n);
            s.extend(e.keys());
        }
        s
    }
}

fn class_latency(c: LinkClass) -> u32 {
    Link::new(0, c, None).est_latency_ms()
}
fn class_default_bps(c: LinkClass) -> u32 {
    c.default_bps()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// Path excluding self, ending at destination.
    pub path: Vec<NodeId>,
    pub cost: u32,
    pub latency_ms: u32,
    pub min_bps: u32,
}

impl Route {
    pub fn hops(&self) -> usize {
        self.path.len()
    }
}

/// Scheduled relay of a flood frame with jitter; cancelled if a duplicate is
/// heard before firing (bitchat).
#[derive(Debug, Clone)]
pub struct PendingRelay {
    pub key: [u8; 12],
    pub fire_at: Ms,
    pub bytes: Vec<u8>,
    pub exclude_link: LinkId,
    /// Peer token the frame arrived from; never echo back to it.
    pub exclude_peer: Option<String>,
}

#[derive(Default)]
pub struct FloodScheduler {
    pub pending: Vec<PendingRelay>,
}

impl FloodScheduler {
    pub fn schedule(&mut self, key: [u8; 12], bytes: Vec<u8>, from_link: LinkId, from_peer: Option<String>, now: Ms, rng: &mut Rng, urgent: bool) {
        let jitter = if urgent { 0 } else { rng.range(FLOOD_JITTER_MIN_MS, FLOOD_JITTER_MAX_MS) };
        self.pending.push(PendingRelay { key, fire_at: now + jitter, bytes, exclude_link: from_link, exclude_peer: from_peer });
    }

    /// Cancel a pending relay because we heard the same frame again.
    pub fn cancel(&mut self, key: &[u8; 12]) -> bool {
        let before = self.pending.len();
        self.pending.retain(|p| &p.key != key);
        before != self.pending.len()
    }

    pub fn due(&mut self, now: Ms) -> Vec<PendingRelay> {
        let (due, keep): (Vec<_>, Vec<_>) = self.pending.drain(..).partition(|p| p.fire_at <= now);
        self.pending = keep;
        due
    }

    pub fn next_fire(&self) -> Option<Ms> {
        self.pending.iter().map(|p| p.fire_at).min()
    }
}

/// Fanout for broadcast relays: log2(degree), min 1; announces use full fanout.
pub fn broadcast_fanout(degree: usize) -> usize {
    if degree <= 2 { degree.max(1) } else { (usize::BITS - degree.leading_zeros()) as usize }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> NodeId {
        [n; 8]
    }

    #[test]
    fn dedup_basic() {
        let mut d = Dedup::default();
        assert!(!d.seen([1; 12], 0));
        assert!(d.seen([1; 12], 10));
        assert!(!d.seen([1; 12], DEDUP_TTL_MS + 11));
    }

    #[test]
    fn neighbours_expire_and_ttl() {
        let mut t = NeighbourTable::default();
        let l = Link::new(1, LinkClass::Lan, None);
        t.observe(id(2), &l, 0);
        assert_eq!(t.count(), 1);
        assert_eq!(t.ttl(), DEFAULT_TTL);
        assert_eq!(t.expire(NEIGHBOUR_EXPIRY_MS + 1), vec![id(2)]);
        assert_eq!(t.count(), 0);
    }

    #[test]
    fn route_direct_and_two_hop() {
        let me = id(1);
        let mut local = NeighbourTable::default();
        let lan = Link::new(1, LinkClass::Lan, None);
        local.observe(id(2), &lan, 0);
        let mut topo = Topology::default();
        // 2 says it sees 1 and 3; 3 says it sees 2
        topo.update(id(2), &[(id(1), LinkClass::Lan, 1), (id(3), LinkClass::BleL2cap, 5)], 0);
        topo.update(id(3), &[(id(2), LinkClass::BleL2cap, 5)], 0);
        let r = topo.route(&me, &local, &id(2), MAX_RELAYS).unwrap();
        assert_eq!(r.path, vec![id(2)]);
        let r = topo.route(&me, &local, &id(3), MAX_RELAYS).unwrap();
        assert_eq!(r.path, vec![id(2), id(3)]);
        assert_eq!(r.min_bps, LinkClass::BleL2cap.default_bps());
        // unconfirmed edge (4 only claimed by 3) is not used
        topo.update(id(3), &[(id(2), LinkClass::BleL2cap, 5), (id(4), LinkClass::Lan, 1)], 0);
        assert!(topo.route(&me, &local, &id(4), MAX_RELAYS).is_none());
        topo.update(id(4), &[(id(3), LinkClass::Lan, 1)], 0);
        assert_eq!(topo.route(&me, &local, &id(4), MAX_RELAYS).unwrap().path, vec![id(2), id(3), id(4)]);
    }

    #[test]
    fn flood_scheduler_cancel_and_due() {
        let mut f = FloodScheduler::default();
        let mut rng = Rng::new(42);
        f.schedule([1; 12], vec![1], 1, None, 0, &mut rng, false);
        f.schedule([2; 12], vec![2], 1, None, 0, &mut rng, true);
        assert!(f.cancel(&[1; 12]));
        let due = f.due(0);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].bytes, vec![2]);
    }

    #[test]
    fn fanout() {
        assert_eq!(broadcast_fanout(1), 1);
        assert_eq!(broadcast_fanout(2), 2);
        assert_eq!(broadcast_fanout(8), 4);
        assert_eq!(broadcast_fanout(16), 5);
    }
}
