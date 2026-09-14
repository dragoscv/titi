//! Link abstraction: one instance per transport, reported by the host.

use crate::frame::NodeId;

pub type LinkId = u32;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LinkClass {
    Lan = 1,
    WifiAware = 2,
    Nearby = 3,
    Hotspot = 4,
    BleL2cap = 5,
    BleGatt = 6,
    BtRfcomm = 7,
    Internet = 8,
    WebRtc = 9,
}

impl LinkClass {
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => Self::Lan,
            2 => Self::WifiAware,
            3 => Self::Nearby,
            4 => Self::Hotspot,
            5 => Self::BleL2cap,
            6 => Self::BleGatt,
            7 => Self::BtRfcomm,
            8 => Self::Internet,
            9 => Self::WebRtc,
            _ => return None,
        })
    }

    /// Base cost (research 03 §8).
    pub fn base_cost(self) -> u32 {
        match self {
            Self::Lan => 1,
            Self::WifiAware | Self::Hotspot | Self::Nearby => 2,
            Self::WebRtc => 3,
            Self::BleL2cap => 5,
            Self::BtRfcomm => 6,
            Self::Internet => 8,
            Self::BleGatt => 12,
        }
    }

    /// Default bandwidth estimate before measurement (bps).
    pub fn default_bps(self) -> u32 {
        match self {
            Self::Lan | Self::WifiAware | Self::Hotspot | Self::WebRtc => 5_000_000,
            Self::Nearby => 40_000,
            Self::BleL2cap => 300_000,
            Self::BtRfcomm => 150_000,
            Self::Internet => 1_000_000,
            Self::BleGatt => 30_000,
        }
    }

    pub fn default_mtu(self) -> usize {
        match self {
            Self::BleGatt => 244,
            Self::BleL2cap => 1000,
            _ => 1200,
        }
    }

    /// Whether the link carries voice comfortably.
    pub fn is_control_only_preferred(self) -> bool {
        matches!(self, Self::BleGatt)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkStats {
    pub est_bps: u32,
    pub rtt_ms: u32,
    pub loss_pct: u8,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub id: LinkId,
    pub class: LinkClass,
    pub mtu: usize,
    pub stats: LinkStats,
    /// Peers reachable directly on this link (node_id → host-specific peer token).
    pub peers: std::collections::HashMap<NodeId, String>,
    /// True while the host reports the link usable.
    pub up: bool,
}

impl Link {
    pub fn new(id: LinkId, class: LinkClass, mtu: Option<usize>) -> Self {
        Link {
            id,
            class,
            mtu: mtu.unwrap_or(class.default_mtu()),
            stats: LinkStats { est_bps: class.default_bps(), rtt_ms: 30, loss_pct: 0 },
            peers: Default::default(),
            up: true,
        }
    }

    /// cost = base + 1000 / bps_norm + rtt  (bps_norm in units of 10 kbps)
    pub fn cost(&self) -> u32 {
        let bps_norm = (self.stats.est_bps / 10_000).max(1);
        let loss_penalty = self.stats.loss_pct as u32 * 2;
        self.class.base_cost() + 1000 / bps_norm + self.stats.rtt_ms / 10 + loss_penalty
    }

    /// Estimated one-way latency for the budget check.
    pub fn est_latency_ms(&self) -> u32 {
        let base = match self.class {
            LinkClass::Lan | LinkClass::WifiAware | LinkClass::Hotspot => 10,
            LinkClass::Nearby | LinkClass::WebRtc => 25,
            LinkClass::BleL2cap => 40,
            LinkClass::BtRfcomm => 50,
            LinkClass::Internet => 60,
            LinkClass::BleGatt => 90,
        };
        base + self.stats.rtt_ms / 2
    }
}
