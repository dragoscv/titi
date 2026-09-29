//! Link handover state machine (per group session, per talker).
//!
//! STABLE → DEGRADED (bicast ≤1 s) → SWITCHING (profile renegotiate) → STABLE
//! any → SUSPENDED after 3 s without a route.

use crate::frame::Profile;
use crate::mesh::Route;
use crate::time::Ms;

pub const BICAST_MAX_MS: u64 = 1_000;
pub const SUSPEND_AFTER_MS: u64 = 3_000;
pub const SWITCH_CONFIRM_FRAMES: u8 = 3;
pub const COST_JUMP_FACTOR: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoState {
    Stable {
        route: Route,
    },
    Degraded {
        old: Option<Route>,
        alt: Route,
        since: Ms,
    },
    Switching {
        route: Route,
        confirmed: u8,
    },
    Suspended {
        since: Ms,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoEvent {
    /// Send on both routes.
    Bicast {
        old: Option<Route>,
        alt: Route,
    },
    /// Only new route; renegotiated profile.
    Switched {
        route: Route,
        profile: Profile,
    },
    Stable {
        route: Route,
    },
    Suspended,
    Resumed {
        route: Route,
    },
}

pub struct Handover {
    pub state: HoState,
    pub profile: Profile,
    last_route_seen: Ms,
}

impl Handover {
    pub fn new(now: Ms) -> Self {
        Handover {
            state: HoState::Suspended { since: now },
            profile: Profile::Std,
            last_route_seen: now,
        }
    }

    pub fn current_route(&self) -> Option<&Route> {
        match &self.state {
            HoState::Stable { route } | HoState::Switching { route, .. } => Some(route),
            HoState::Degraded { alt, .. } => Some(alt),
            HoState::Suspended { .. } => None,
        }
    }

    pub fn is_suspended(&self) -> bool {
        matches!(self.state, HoState::Suspended { .. })
    }

    /// Feed the best route currently computed (None if unreachable).
    pub fn on_route(&mut self, best: Option<Route>, now: Ms) -> Vec<HoEvent> {
        let mut ev = vec![];
        match (&self.state, best) {
            (HoState::Suspended { .. }, Some(r)) => {
                self.profile = Profile::for_bandwidth(r.min_bps);
                ev.push(HoEvent::Resumed { route: r.clone() });
                ev.push(HoEvent::Switched {
                    route: r.clone(),
                    profile: self.profile,
                });
                self.state = HoState::Switching {
                    route: r,
                    confirmed: 0,
                };
                self.last_route_seen = now;
            }
            (HoState::Suspended { .. }, None) => {}
            (_, None) => {
                if now.saturating_sub(self.last_route_seen) >= SUSPEND_AFTER_MS {
                    self.state = HoState::Suspended { since: now };
                    ev.push(HoEvent::Suspended);
                } else if let HoState::Stable { route } = &self.state {
                    // lost route: degrade with no alternative yet
                    let r = route.clone();
                    self.state = HoState::Degraded {
                        old: Some(r.clone()),
                        alt: r,
                        since: now,
                    };
                }
            }
            (HoState::Stable { route }, Some(r)) => {
                self.last_route_seen = now;
                let changed = r.path != route.path;
                let jump = r.cost >= route.cost.saturating_mul(COST_JUMP_FACTOR)
                    || route.cost >= r.cost.saturating_mul(COST_JUMP_FACTOR);
                if changed || jump {
                    let old = route.clone();
                    ev.push(HoEvent::Bicast {
                        old: Some(old.clone()),
                        alt: r.clone(),
                    });
                    self.state = HoState::Degraded {
                        old: Some(old),
                        alt: r,
                        since: now,
                    };
                }
            }
            (HoState::Degraded { old, since, .. }, Some(r)) => {
                self.last_route_seen = now;
                if now - *since >= BICAST_MAX_MS {
                    self.profile = Profile::for_bandwidth(r.min_bps);
                    ev.push(HoEvent::Switched {
                        route: r.clone(),
                        profile: self.profile,
                    });
                    self.state = HoState::Switching {
                        route: r,
                        confirmed: 0,
                    };
                } else {
                    let o = old.clone();
                    self.state = HoState::Degraded {
                        old: o,
                        alt: r,
                        since: *since,
                    };
                }
            }
            (HoState::Switching { route, confirmed }, Some(r)) => {
                self.last_route_seen = now;
                if r.path != route.path {
                    self.state = HoState::Switching {
                        route: r,
                        confirmed: 0,
                    };
                } else {
                    // A route that keeps being re-computed identically is as good a
                    // confirmation as an ACK for a member who never transmits.
                    let c = confirmed + 1;
                    if c >= SWITCH_CONFIRM_FRAMES {
                        ev.push(HoEvent::Stable { route: r.clone() });
                        self.state = HoState::Stable { route: r };
                    } else {
                        self.state = HoState::Switching {
                            route: r,
                            confirmed: c,
                        };
                    }
                }
            }
        }
        ev
    }

    /// Listener acknowledged a frame on the current route (RECV_ACK or voice
    /// heard back). Advances DEGRADED → SWITCHING → STABLE.
    pub fn on_ack(&mut self, now: Ms) -> Vec<HoEvent> {
        let mut ev = vec![];
        match &self.state {
            HoState::Degraded { alt, .. } => {
                self.profile = Profile::for_bandwidth(alt.min_bps);
                ev.push(HoEvent::Switched {
                    route: alt.clone(),
                    profile: self.profile,
                });
                self.state = HoState::Switching {
                    route: alt.clone(),
                    confirmed: 1,
                };
            }
            HoState::Switching { route, confirmed } => {
                let c = confirmed + 1;
                if c >= SWITCH_CONFIRM_FRAMES {
                    ev.push(HoEvent::Stable {
                        route: route.clone(),
                    });
                    self.state = HoState::Stable {
                        route: route.clone(),
                    };
                } else {
                    self.state = HoState::Switching {
                        route: route.clone(),
                        confirmed: c,
                    };
                }
            }
            _ => {}
        }
        let _ = now;
        ev
    }

    pub fn tick(&mut self, now: Ms) -> Vec<HoEvent> {
        match &self.state {
            HoState::Degraded { alt, since, .. } if now - *since >= BICAST_MAX_MS => {
                self.profile = Profile::for_bandwidth(alt.min_bps);
                let r = alt.clone();
                self.state = HoState::Switching {
                    route: r.clone(),
                    confirmed: 0,
                };
                vec![HoEvent::Switched {
                    route: r,
                    profile: self.profile,
                }]
            }
            HoState::Stable { .. } | HoState::Switching { .. } | HoState::Degraded { .. }
                if now.saturating_sub(self.last_route_seen) >= SUSPEND_AFTER_MS =>
            {
                self.state = HoState::Suspended { since: now };
                vec![HoEvent::Suspended]
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(n: u8, cost: u32, bps: u32) -> Route {
        Route {
            path: vec![[n; 8]],
            cost,
            latency_ms: 10,
            min_bps: bps,
        }
    }

    #[test]
    fn full_cycle_lan_to_ble() {
        let mut h = Handover::new(0);
        let ev = h.on_route(Some(route(2, 2, 5_000_000)), 0);
        assert!(matches!(ev[0], HoEvent::Resumed { .. }));
        assert_eq!(h.profile, Profile::Hq);
        for _ in 0..3 {
            h.on_ack(10);
        }
        assert!(matches!(h.state, HoState::Stable { .. }));
        // LAN degrades → BLE alternative
        let ev = h.on_route(Some(route(2, 40, 300_000)), 100);
        assert!(matches!(ev[0], HoEvent::Bicast { .. }));
        let ev = h.on_ack(200);
        match &ev[0] {
            HoEvent::Switched { profile, .. } => assert_eq!(*profile, Profile::Hq), // 300 kbps still fits HQ
            _ => panic!(),
        }
        h.on_ack(220);
        let ev = h.on_ack(240);
        assert!(matches!(ev[0], HoEvent::Stable { .. }));
        // route vanishes at t=300: degrade, keep trying old route (bicast window), then
        // suspend 3 s after the last time a route was seen (t=100).
        assert!(h.on_route(None, 300).is_empty());
        assert!(matches!(h.state, HoState::Degraded { .. }));
        let ev = h.tick(300 + BICAST_MAX_MS);
        assert!(matches!(ev[0], HoEvent::Switched { .. }));
        assert!(h.tick(100 + SUSPEND_AFTER_MS - 1).is_empty());
        assert_eq!(h.tick(100 + SUSPEND_AFTER_MS), vec![HoEvent::Suspended]);
        assert!(h.is_suspended());
    }

    #[test]
    fn bicast_times_out_into_switching() {
        let mut h = Handover::new(0);
        h.on_route(Some(route(2, 2, 5_000_000)), 0);
        for _ in 0..3 {
            h.on_ack(1);
        }
        h.on_route(Some(route(3, 30, 30_000)), 10);
        let ev = h.tick(10 + BICAST_MAX_MS);
        match &ev[0] {
            HoEvent::Switched { profile, .. } => assert_eq!(*profile, Profile::Std),
            _ => panic!(),
        }
    }
}
