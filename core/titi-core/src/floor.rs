//! MCPTT-lite floor control (TS 24.380 off-network, simplified).
//!
//! States: Idle · Pending (we asked, arbitration window running) · HasFloor ·
//! Taken (someone else) · Queued. Arbitration tuple `(prio desc, ts asc, id asc)`.

use crate::frame::NodeId;
use crate::time::Ms;

pub const T_ARB_MIN_MS: u64 = 80;
pub const T_ARB_MAX_MS: u64 = 250;
pub const T_TAKEN_REFRESH_MS: u64 = 1_000;
pub const T_TAKEN_EXPIRY_MS: u64 = 1_500;
pub const T_MAX_TALK_MS: u64 = 60_000;
pub const T_WARN_TALK_MS: u64 = 50_000;
pub const IDLE_REPEATS: u8 = 3;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Normal = 1,
    Elevated = 2,
    Emergency = 3,
}

impl Priority {
    pub fn from_u8(v: u8) -> Self {
        match v {
            3 => Self::Emergency,
            2 => Self::Elevated,
            _ => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    pub node: NodeId,
    pub prio: Priority,
    pub ts: Ms,
}

impl Claim {
    /// True if `self` beats `other` in arbitration.
    pub fn beats(&self, other: &Claim) -> bool {
        (
            self.prio,
            std::cmp::Reverse(self.ts),
            std::cmp::Reverse(self.node),
        ) > (
            other.prio,
            std::cmp::Reverse(other.ts),
            std::cmp::Reverse(other.node),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Idle,
    Pending {
        claim: Claim,
        deadline: Ms,
    },
    HasFloor {
        since: Ms,
        last_taken_sent: Ms,
        warned: bool,
        prio: Priority,
    },
    Taken {
        holder: Claim,
        last_heard: Ms,
        talker_short: u16,
    },
    Queued {
        claim: Claim,
        holder: Claim,
        last_heard: Ms,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloorEvent {
    /// Broadcast FLOOR_REQ (start local buffering).
    SendRequest(Claim),
    /// We won: broadcast FLOOR_TAKEN, release buffered audio.
    Granted { talker_short: u16 },
    /// Re-broadcast FLOOR_TAKEN keepalive.
    SendTaken,
    /// Broadcast FLOOR_IDLE (repeat count remaining).
    SendIdle,
    /// Lost arbitration / floor busy: play busy tone, drop buffer.
    Denied { holder: NodeId },
    /// Someone else has the floor (UI: who is talking).
    TakenBy {
        holder: NodeId,
        prio: Priority,
        talker_short: u16,
    },
    /// Floor became free.
    Idle,
    /// Our floor was pre-empted by higher priority.
    Revoked { by: NodeId },
    /// 10 s before auto-release.
    TalkWarning,
    /// Auto-released after 60 s.
    TalkTimeout,
    /// We were queued and the floor is now ours to request again.
    QueueReady,
}

pub struct Floor {
    pub me: NodeId,
    pub my_short: u16,
    pub state: State,
    pub t_arb_ms: u64,
    pub queue_enabled: bool,
    idle_repeats_left: u8,
}

impl Floor {
    pub fn new(me: NodeId, my_short: u16) -> Self {
        Floor {
            me,
            my_short,
            state: State::Idle,
            t_arb_ms: 150,
            queue_enabled: true,
            idle_repeats_left: 0,
        }
    }

    /// Adapt T_arb from measured max hop RTT.
    pub fn set_rtt(&mut self, max_hop_rtt_ms: u32) {
        self.t_arb_ms = (2 * max_hop_rtt_ms as u64).clamp(T_ARB_MIN_MS, T_ARB_MAX_MS);
    }

    pub fn is_talking(&self) -> bool {
        matches!(self.state, State::HasFloor { .. })
    }

    pub fn holder(&self) -> Option<NodeId> {
        match &self.state {
            State::HasFloor { .. } => Some(self.me),
            State::Taken { holder, .. } | State::Queued { holder, .. } => Some(holder.node),
            _ => None,
        }
    }

    pub fn ptt_down(&mut self, prio: Priority, now: Ms) -> Vec<FloorEvent> {
        let claim = Claim {
            node: self.me,
            prio,
            ts: now,
        };
        match &self.state {
            State::Idle => {
                self.state = State::Pending {
                    claim,
                    deadline: now + self.t_arb_ms,
                };
                vec![FloorEvent::SendRequest(claim)]
            }
            State::Taken { holder, .. } => {
                if claim.prio > holder.prio {
                    // pre-empt
                    self.state = State::Pending {
                        claim,
                        deadline: now + self.t_arb_ms,
                    };
                    vec![FloorEvent::SendRequest(claim)]
                } else if self.queue_enabled {
                    let h = *holder;
                    let lh = if let State::Taken { last_heard, .. } = &self.state {
                        *last_heard
                    } else {
                        now
                    };
                    self.state = State::Queued {
                        claim,
                        holder: h,
                        last_heard: lh,
                    };
                    vec![FloorEvent::Denied { holder: h.node }]
                } else {
                    vec![FloorEvent::Denied {
                        holder: holder.node,
                    }]
                }
            }
            _ => vec![],
        }
    }

    pub fn ptt_up(&mut self, now: Ms) -> Vec<FloorEvent> {
        match &self.state {
            State::HasFloor { .. } => {
                self.state = State::Idle;
                self.idle_repeats_left = IDLE_REPEATS - 1;
                let _ = now;
                vec![FloorEvent::SendIdle, FloorEvent::Idle]
            }
            State::Pending { .. } | State::Queued { .. } => {
                self.state = State::Idle;
                vec![]
            }
            _ => vec![],
        }
    }

    pub fn on_request(&mut self, other: Claim, now: Ms) -> Vec<FloorEvent> {
        match &self.state {
            State::Pending { claim, .. } => {
                if other.beats(claim) {
                    self.state = State::Idle;
                    vec![FloorEvent::Denied { holder: other.node }]
                } else {
                    vec![]
                }
            }
            State::HasFloor { prio, .. } => {
                if other.prio > *prio {
                    self.state = State::Idle;
                    vec![FloorEvent::Revoked { by: other.node }, FloorEvent::SendIdle]
                } else {
                    vec![FloorEvent::SendTaken]
                }
            }
            _ => {
                let _ = now;
                vec![]
            }
        }
    }

    pub fn on_taken(&mut self, holder: Claim, talker_short: u16, now: Ms) -> Vec<FloorEvent> {
        if holder.node == self.me {
            return vec![];
        }
        match &self.state {
            State::Pending { claim, .. } => {
                if holder.beats(claim) || holder.prio >= claim.prio {
                    self.state = State::Taken {
                        holder,
                        last_heard: now,
                        talker_short,
                    };
                    vec![
                        FloorEvent::Denied {
                            holder: holder.node,
                        },
                        FloorEvent::TakenBy {
                            holder: holder.node,
                            prio: holder.prio,
                            talker_short,
                        },
                    ]
                } else {
                    vec![]
                }
            }
            State::HasFloor { prio, .. } => {
                if holder.prio > *prio {
                    self.state = State::Taken {
                        holder,
                        last_heard: now,
                        talker_short,
                    };
                    vec![
                        FloorEvent::Revoked { by: holder.node },
                        FloorEvent::TakenBy {
                            holder: holder.node,
                            prio: holder.prio,
                            talker_short,
                        },
                    ]
                } else {
                    vec![FloorEvent::SendTaken]
                }
            }
            State::Taken { holder: h, .. } if h.node == holder.node => {
                self.state = State::Taken {
                    holder,
                    last_heard: now,
                    talker_short,
                };
                vec![]
            }
            State::Queued { claim, .. } => {
                let c = *claim;
                self.state = State::Queued {
                    claim: c,
                    holder,
                    last_heard: now,
                };
                vec![]
            }
            _ => {
                self.state = State::Taken {
                    holder,
                    last_heard: now,
                    talker_short,
                };
                vec![FloorEvent::TakenBy {
                    holder: holder.node,
                    prio: holder.prio,
                    talker_short,
                }]
            }
        }
    }

    /// Voice frames from a talker imply FLOOR_TAKEN.
    pub fn on_voice_from(&mut self, talker: NodeId, talker_short: u16, now: Ms) -> Vec<FloorEvent> {
        match &self.state {
            State::Taken { holder, .. } if holder.node == talker => {
                let h = *holder;
                self.state = State::Taken {
                    holder: h,
                    last_heard: now,
                    talker_short,
                };
                vec![]
            }
            State::Idle => {
                let holder = Claim {
                    node: talker,
                    prio: Priority::Normal,
                    ts: now,
                };
                self.state = State::Taken {
                    holder,
                    last_heard: now,
                    talker_short,
                };
                vec![FloorEvent::TakenBy {
                    holder: talker,
                    prio: Priority::Normal,
                    talker_short,
                }]
            }
            _ => vec![],
        }
    }

    pub fn on_idle(&mut self, from: NodeId, now: Ms) -> Vec<FloorEvent> {
        match &self.state {
            State::Taken { holder, .. } if holder.node == from => {
                self.state = State::Idle;
                vec![FloorEvent::Idle]
            }
            State::Queued { holder, .. } if holder.node == from => {
                self.state = State::Idle;
                let _ = now;
                vec![FloorEvent::Idle, FloorEvent::QueueReady]
            }
            _ => vec![],
        }
    }

    pub fn tick(&mut self, now: Ms) -> Vec<FloorEvent> {
        let mut ev = vec![];
        match &mut self.state {
            State::Pending { deadline, .. } if now >= *deadline => {
                let prio = if let State::Pending { claim, .. } = &self.state {
                    claim.prio
                } else {
                    Priority::Normal
                };
                self.state = State::HasFloor {
                    since: now,
                    last_taken_sent: now,
                    warned: false,
                    prio,
                };
                ev.push(FloorEvent::Granted {
                    talker_short: self.my_short,
                });
                ev.push(FloorEvent::SendTaken);
            }
            State::HasFloor {
                since,
                last_taken_sent,
                warned,
                ..
            } => {
                if now.saturating_sub(*since) >= T_MAX_TALK_MS {
                    self.state = State::Idle;
                    self.idle_repeats_left = IDLE_REPEATS - 1;
                    ev.push(FloorEvent::TalkTimeout);
                    ev.push(FloorEvent::SendIdle);
                    ev.push(FloorEvent::Idle);
                } else {
                    if !*warned && now.saturating_sub(*since) >= T_WARN_TALK_MS {
                        *warned = true;
                        ev.push(FloorEvent::TalkWarning);
                    }
                    if now.saturating_sub(*last_taken_sent) >= T_TAKEN_REFRESH_MS {
                        *last_taken_sent = now;
                        ev.push(FloorEvent::SendTaken);
                    }
                }
            }
            State::Taken { last_heard, .. }
                if now.saturating_sub(*last_heard) > T_TAKEN_EXPIRY_MS =>
            {
                self.state = State::Idle;
                ev.push(FloorEvent::Idle);
            }
            State::Queued { last_heard, .. }
                if now.saturating_sub(*last_heard) > T_TAKEN_EXPIRY_MS =>
            {
                self.state = State::Idle;
                ev.push(FloorEvent::Idle);
                ev.push(FloorEvent::QueueReady);
            }
            _ => {}
        }
        if matches!(self.state, State::Idle) && self.idle_repeats_left > 0 {
            self.idle_repeats_left -= 1;
            ev.push(FloorEvent::SendIdle);
        }
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> NodeId {
        [n; 8]
    }

    #[test]
    fn request_then_grant_after_arbitration() {
        let mut f = Floor::new(id(1), 11);
        let ev = f.ptt_down(Priority::Normal, 1000);
        assert!(matches!(ev[0], FloorEvent::SendRequest(_)));
        assert!(f.tick(1000 + f.t_arb_ms - 1).is_empty());
        let ev = f.tick(1000 + f.t_arb_ms);
        assert_eq!(ev[0], FloorEvent::Granted { talker_short: 11 });
        assert!(f.is_talking());
        let ev = f.ptt_up(2000);
        assert!(ev.contains(&FloorEvent::Idle));
        assert_eq!(f.state, State::Idle);
    }

    #[test]
    fn collision_lower_tuple_wins() {
        let mut a = Floor::new(id(1), 1);
        let mut b = Floor::new(id(2), 2);
        let ca = match a.ptt_down(Priority::Normal, 1000)[0] {
            FloorEvent::SendRequest(c) => c,
            _ => panic!(),
        };
        let cb = match b.ptt_down(Priority::Normal, 1005)[0] {
            FloorEvent::SendRequest(c) => c,
            _ => panic!(),
        };
        assert!(a.on_request(cb, 1010).is_empty()); // a is earlier, keeps pending
        let ev = b.on_request(ca, 1010);
        assert_eq!(ev, vec![FloorEvent::Denied { holder: id(1) }]);
        assert_eq!(b.state, State::Idle);
    }

    #[test]
    fn emergency_preempts() {
        let mut a = Floor::new(id(1), 1);
        a.ptt_down(Priority::Normal, 0);
        a.tick(1000);
        assert!(a.is_talking());
        let ev = a.on_taken(
            Claim {
                node: id(9),
                prio: Priority::Emergency,
                ts: 1500,
            },
            99,
            1500,
        );
        assert!(matches!(ev[0], FloorEvent::Revoked { .. }));
        assert_eq!(a.holder(), Some(id(9)));
    }

    #[test]
    fn taken_expires_without_voice() {
        let mut a = Floor::new(id(1), 1);
        a.on_voice_from(id(2), 22, 0);
        assert_eq!(a.holder(), Some(id(2)));
        assert!(a.tick(T_TAKEN_EXPIRY_MS).is_empty());
        assert_eq!(a.tick(T_TAKEN_EXPIRY_MS + 1), vec![FloorEvent::Idle]);
    }

    #[test]
    fn max_talk_timeout() {
        let mut a = Floor::new(id(1), 1);
        a.ptt_down(Priority::Normal, 0);
        a.tick(1000);
        let ev = a.tick(1000 + T_WARN_TALK_MS);
        assert!(ev.contains(&FloorEvent::TalkWarning));
        let ev = a.tick(1000 + T_MAX_TALK_MS);
        assert!(ev.contains(&FloorEvent::TalkTimeout));
        assert!(!a.is_talking());
    }

    #[test]
    fn queue_ready_after_holder_idle() {
        let mut a = Floor::new(id(1), 1);
        a.on_voice_from(id(2), 22, 0);
        let ev = a.ptt_down(Priority::Normal, 10);
        assert_eq!(ev, vec![FloorEvent::Denied { holder: id(2) }]);
        assert!(matches!(a.state, State::Queued { .. }));
        let ev = a.on_idle(id(2), 20);
        assert!(ev.contains(&FloorEvent::QueueReady));
    }
}
