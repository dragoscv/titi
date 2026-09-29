//! Two engines connected by a simulated LAN link: discovery, Noise session,
//! invite/accept, PTT floor, encrypted voice delivery, text message + ACK,
//! join-by-code, and a three-node relay.

use std::collections::VecDeque;

use titi_core::engine::{Action, Config, Engine, UiEvent};
use titi_core::floor::Priority;
use titi_core::identity::Identity;
use titi_core::link::LinkClass;

const LAN: u32 = 1;

struct Net {
    engines: Vec<Engine>,
    /// (to_idx, link, from_token, bytes)
    queue: VecDeque<(usize, u32, String, Vec<u8>)>,
    ui: Vec<Vec<UiEvent>>,
    plays: Vec<usize>,
    /// adjacency: who can hear whom on LAN
    adj: Vec<Vec<usize>>,
    /// largest frame put on the wire (must stay ≤ link MTU)
    max_frame: usize,
}

fn tok(i: usize) -> String {
    format!("peer{i}")
}

impl Net {
    fn new(n: usize, names: &[&str]) -> Self {
        let engines = (0..n)
            .map(|i| {
                let id = Identity::from_seed(&[(i + 1) as u8; 32]);
                let cfg = Config {
                    display_name: names[i].into(),
                    avatar_hue: 40,
                    kdf: titi_core::crypto::KdfParams::LIGHT,
                    relay_capable: true,
                    max_profile: None,
                };
                Engine::new(id, cfg, 1000 + i as u64)
            })
            .collect();
        let adj = (0..n)
            .map(|i| (0..n).filter(|j| *j != i).collect())
            .collect();
        Net {
            engines,
            queue: VecDeque::new(),
            ui: vec![vec![]; n],
            plays: vec![0; n],
            adj,
            max_frame: 0,
        }
    }

    fn apply(&mut self, from: usize, acts: Vec<Action>) {
        let acts = self.engines[from].shape(acts);
        for a in acts {
            match a {
                Action::Send { link, peer, bytes } => {
                    self.max_frame = self.max_frame.max(bytes.len());
                    let targets: Vec<usize> = match &peer {
                        Some(t) => {
                            let idx: usize = t
                                .trim_start_matches("peer")
                                .split('#')
                                .next()
                                .unwrap()
                                .parse()
                                .unwrap();
                            vec![idx]
                        }
                        None => self.adj[from].clone(),
                    };
                    for t in targets {
                        if self.adj[from].contains(&t) {
                            self.queue.push_back((t, link, tok(from), bytes.clone()));
                        }
                    }
                }
                Action::Play { .. } => self.plays[from] += 1,
                Action::Ui(e) => self.ui[from].push(e),
                _ => {}
            }
        }
    }

    fn pump(&mut self, now: u64) {
        let mut guard = 0;
        // deliver, then tick a few times so jittered flood relays (≤220 ms) fire
        for step in 0..4u64 {
            while let Some((to, link, from_tok, bytes)) = self.queue.pop_front() {
                let acts = self.engines[to].on_frame(link, from_tok, &bytes, now + step * 80);
                self.apply(to, acts);
                guard += 1;
                assert!(guard < 10_000, "message storm");
            }
            for i in 0..self.engines.len() {
                let acts = self.engines[i].tick(now + (step + 1) * 80);
                self.apply(i, acts);
            }
        }
        while let Some((to, link, from_tok, bytes)) = self.queue.pop_front() {
            let acts = self.engines[to].on_frame(link, from_tok, &bytes, now + 320);
            self.apply(to, acts);
        }
    }

    fn tick_all(&mut self, now: u64) {
        for i in 0..self.engines.len() {
            let acts = self.engines[i].tick(now);
            self.apply(i, acts);
        }
        let mut guard = 0;
        while let Some((to, link, from_tok, bytes)) = self.queue.pop_front() {
            let acts = self.engines[to].on_frame(link, from_tok, &bytes, now);
            self.apply(to, acts);
            guard += 1;
            assert!(guard < 10_000, "message storm");
        }
    }

    fn run(&mut self, from_ms: u64, to_ms: u64, step: u64) -> u64 {
        let mut t = from_ms;
        while t <= to_ms {
            self.tick_all(t);
            t += step;
        }
        t
    }

    fn has_ui(&self, i: usize, f: impl Fn(&UiEvent) -> bool) -> bool {
        self.ui[i].iter().any(f)
    }

    fn bring_up(&mut self, now: u64) {
        for i in 0..self.engines.len() {
            let acts = self.engines[i].on_link_up(LAN, LinkClass::Lan, None, now);
            self.apply(i, acts);
        }
        for i in 0..self.engines.len() {
            for j in self.adj[i].clone() {
                let acts = self.engines[i].on_peer_seen(LAN, tok(j), now);
                self.apply(i, acts);
            }
        }
        self.pump(now);
    }
}

fn pcm_tone(n: usize, f: f32) -> Vec<i16> {
    (0..n)
        .map(|i| ((i as f32 * f).sin() * 8000.0) as i16)
        .collect()
}

/// Invites every other node into a fresh group hosted by node 0 (direct neighbours only).
fn group_of_all(net: &mut Net, now: u64) -> [u8; 16] {
    let (gid, acts) = net.engines[0].create_group("Note", now).unwrap();
    net.apply(0, acts);
    let host = net.engines[0].node_id();
    for i in 1..net.engines.len() {
        let n = net.engines[i].node_id();
        let acts = net.engines[0].invite_peer(gid, n, now);
        net.apply(0, acts);
        net.pump(now);
        let acts = net.engines[i].accept_invite(gid, host, now);
        net.apply(i, acts);
        net.pump(now);
    }
    gid
}

fn voice_note_bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}

#[test]
fn voice_note_larger_than_mtu_is_fragmented_and_delivered() {
    let mut net = Net::new(2, &["Ana", "Ceas"]);
    let mut now = 1_789_403_000_000u64;
    net.bring_up(now);
    net.run(now, now + 500, 20);
    now += 520;
    let gid = group_of_all(&mut net, now);
    // ~10 s of 16 kbps opus ≈ 20 KB: 17+ fragments at LAN MTU 1200
    let note = voice_note_bytes(20_000);
    let acts = net.engines[1].send_voice_note(
        gid,
        titi_core::frame::Profile::Std,
        10_000,
        note.clone(),
        now,
    );
    net.apply(1, acts);
    // fragments are paced: one per link per tick
    net.run(now, now + 2_000, 20);
    assert!(
        net.max_frame <= 1200,
        "a frame exceeded the MTU: {}",
        net.max_frame
    );
    assert!(
        net.has_ui(0, |e| matches!(e, UiEvent::Message { body: titi_core::engine::MessageBody::VoiceNote { opus_packets, duration_ms: 10_000, .. }, .. } if *opus_packets == note)),
        "Ana got the voice note intact"
    );
    assert!(
        net.has_ui(1, |e| matches!(e, UiEvent::MessageAcked { .. })),
        "sender got the ACK"
    );
}

#[test]
fn voice_note_is_relayed_in_fragments_over_a_middle_node() {
    // A — B — C: the relayed Message is re-fragmented hop by hop
    let mut net = Net::new(3, &["A", "B", "C"]);
    let mut now = 1_789_404_000_000u64;
    net.bring_up(now);
    net.run(now, now + 600, 20);
    now += 620;
    let gid = group_of_all(&mut net, now); // all three still adjacent while inviting
    net.adj = vec![vec![1], vec![0, 2], vec![1]];
    let note = voice_note_bytes(6_000);
    let acts = net.engines[0].send_voice_note(
        gid,
        titi_core::frame::Profile::Std,
        3_000,
        note.clone(),
        now,
    );
    net.apply(0, acts);
    net.run(now, now + 3_000, 20);
    assert!(net.max_frame <= 1200, "max frame {}", net.max_frame);
    assert!(
        net.has_ui(2, |e| matches!(e, UiEvent::Message { body: titi_core::engine::MessageBody::VoiceNote { opus_packets, .. }, .. } if *opus_packets == note)),
        "C got A's note through B"
    );
}

#[test]
fn discovery_invite_ptt_voice_text() {
    let mut net = Net::new(2, &["Ana", "Bogdan"]);
    let mut now = 1_789_400_000_000u64;
    net.bring_up(now);
    net.run(now, now + 500, 20);
    now += 520;

    // both discovered each other and have Noise transport sessions
    assert!(net.has_ui(
        0,
        |e| matches!(e, UiEvent::PeerDiscovered { name, .. } if name == "Bogdan")
    ));
    assert!(net.has_ui(
        1,
        |e| matches!(e, UiEvent::PeerDiscovered { name, .. } if name == "Ana")
    ));
    assert!(
        net.engines[0]
            .sessions
            .values()
            .any(|s| s.session.is_transport()),
        "A has transport session"
    );
    assert!(
        net.engines[1]
            .sessions
            .values()
            .any(|s| s.session.is_transport()),
        "B has transport session"
    );

    // Ana creates a group and invites Bogdan by tap
    let (gid, acts) = net.engines[0].create_group("Munte", now).unwrap();
    net.apply(0, acts);
    let b_id = net.engines[1].node_id();
    let acts = net.engines[0].invite_peer(gid, b_id, now);
    net.apply(0, acts);
    net.pump(now);
    assert!(
        net.has_ui(
            1,
            |e| matches!(e, UiEvent::InviteOffered { name, .. } if name == "Munte")
        ),
        "B saw invite: {:?}",
        net.ui[1]
    );
    let a_id = net.engines[0].node_id();
    let acts = net.engines[1].accept_invite(gid, a_id, now);
    net.apply(1, acts);
    net.pump(now);
    assert!(
        net.has_ui(
            1,
            |e| matches!(e, UiEvent::Joined { name, .. } if name == "Munte")
        ),
        "B joined: {:?}",
        net.ui[1]
    );
    assert!(net.has_ui(0, |e| matches!(e, UiEvent::MemberJoined { .. })));
    assert_eq!(net.engines[1].groups[&gid].members.len(), 2);
    assert_eq!(net.engines[0].groups[&gid].members.len(), 2);
    assert_eq!(
        net.engines[0].groups[&gid].ikm, net.engines[1].groups[&gid].ikm,
        "same group key"
    );

    // let routes compute
    net.run(now, now + 100, 20);
    now += 120;

    // Ana presses PTT
    let acts = net.engines[0].ptt_down(Priority::Normal, now);
    net.apply(0, acts);
    net.pump(now);
    // feed audio during arbitration (buffered)
    for _ in 0..5 {
        now += 20;
        let acts = net.engines[0].on_audio_in(&pcm_tone(960, 0.05), now);
        net.apply(0, acts);
        net.tick_all(now);
    }
    // arbitration window elapses → granted
    now += 300;
    net.tick_all(now);
    assert!(
        net.has_ui(0, |e| matches!(e, UiEvent::FloorGranted)),
        "granted: {:?}",
        net.ui[0]
    );
    assert!(
        net.has_ui(
            1,
            |e| matches!(e, UiEvent::FloorTaken { name, .. } if name == "Ana")
        ),
        "B sees Ana talking: {:?}",
        net.ui[1]
    );

    // stream 1 s of voice
    for _ in 0..50 {
        now += 20;
        let acts = net.engines[0].on_audio_in(&pcm_tone(960, 0.05), now);
        net.apply(0, acts);
        net.tick_all(now);
    }
    assert!(net.plays[1] > 30, "B played {} frames", net.plays[1]);
    assert_eq!(net.plays[0], 0, "A does not hear itself");

    // B tries to talk while A holds → denied
    let acts = net.engines[1].ptt_down(Priority::Normal, now);
    net.apply(1, acts);
    assert!(
        net.has_ui(1, |e| matches!(e, UiEvent::FloorDenied { .. })),
        "B denied: {:?}",
        &net.ui[1][net.ui[1].len().saturating_sub(5)..]
    );
    let acts = net.engines[1].ptt_up(now);
    net.apply(1, acts);

    // A releases → B (which is idle after its own ptt_up) times out A's floor
    let acts = net.engines[0].ptt_up(now);
    net.apply(0, acts);
    net.pump(now);
    assert!(net.has_ui(0, |e| matches!(e, UiEvent::FloorIdle { .. })));
    assert!(
        net.engines[1].groups[&gid].floor.holder().is_none(),
        "B sees floor free"
    );

    // text message + ack
    let acts = net.engines[1].send_text(gid, "Salut!", now);
    net.apply(1, acts);
    net.pump(now);
    assert!(net.has_ui(0, |e| matches!(e, UiEvent::Message { body: titi_core::engine::MessageBody::Text(t), .. } if t == "Salut!")), "A got text: {:?}", net.ui[0]);
    assert!(
        net.has_ui(1, |e| matches!(e, UiEvent::MessageAcked { .. })),
        "B got ack"
    );

    // full-duplex toggle propagates
    let acts = net.engines[0].set_full_duplex(gid, true, now);
    net.apply(0, acts);
    net.pump(now);
    assert!(net.engines[1].groups[&gid].full_duplex);
}

#[test]
fn join_by_rotating_code() {
    let mut net = Net::new(2, &["Host", "Guest"]);
    let mut now = 1_789_400_500_000u64;
    net.bring_up(now);
    net.run(now, now + 400, 20);
    now += 420;
    let (gid, acts) = net.engines[0].create_group("Cabana", now).unwrap();
    net.apply(0, acts);
    // host's HELLO must now advertise the group hash → send hellos
    net.run(now, now + 5000, 100);
    now += 5100;
    let (code, secs) = net.engines[0].current_code(&gid, now).unwrap();
    assert!(secs <= 600);
    let acts = net.engines[1].join_by_code(&code.to_uppercase(), now);
    assert!(!acts.is_empty(), "guest found a candidate group");
    net.apply(1, acts);
    net.pump(now);
    assert!(
        net.has_ui(
            1,
            |e| matches!(e, UiEvent::Joined { name, .. } if name == "Cabana")
        ),
        "guest joined: {:?}",
        net.ui[1]
    );
    assert_eq!(net.engines[0].groups[&gid].members.len(), 2);

    // wrong code fails
    let acts = net.engines[1].join_by_code("acid-acid-acid-00", now);
    net.apply(1, acts);
    net.pump(now);
    assert_eq!(net.engines[1].groups.len(), 1);
}

#[test]
fn join_by_deep_link() {
    let mut net = Net::new(2, &["Host", "Web"]);
    let mut now = 1_789_401_000_000u64;
    net.bring_up(now);
    net.run(now, now + 400, 20);
    now += 420;
    let (gid, acts) = net.engines[0].create_group("Link", now).unwrap();
    net.apply(0, acts);
    net.run(now, now + 5000, 100);
    now += 5100;
    let url = net.engines[0].deep_link(&gid, now, 60_000).unwrap();
    assert!(url.starts_with("titi://j/"));
    let acts = net.engines[1].join_by_link(&url, now);
    net.apply(1, acts);
    net.pump(now);
    assert!(
        net.has_ui(1, |e| matches!(e, UiEvent::Joined { .. })),
        "{:?}",
        net.ui[1]
    );
}

#[test]
fn three_nodes_relay_voice_over_middle() {
    // A — B — C  (A and C cannot hear each other)
    let mut net = Net::new(3, &["A", "B", "C"]);
    net.adj = vec![vec![1], vec![0, 2], vec![1]];
    let mut now = 1_789_402_000_000u64;
    net.bring_up(now);
    net.run(now, now + 600, 20);
    now += 620;
    let (gid, acts) = net.engines[0].create_group("Relay", now).unwrap();
    net.apply(0, acts);
    // invite B, then B invites C
    let b = net.engines[1].node_id();
    let c = net.engines[2].node_id();
    let a = net.engines[0].node_id();
    let acts = net.engines[0].invite_peer(gid, b, now);
    net.apply(0, acts);
    net.pump(now);
    let acts = net.engines[1].accept_invite(gid, a, now);
    net.apply(1, acts);
    net.pump(now);
    let acts = net.engines[1].invite_peer(gid, c, now);
    net.apply(1, acts);
    net.pump(now);
    let acts = net.engines[2].accept_invite(gid, b, now);
    net.apply(2, acts);
    net.pump(now);
    assert_eq!(
        net.engines[2].groups[&gid].members.len(),
        3,
        "C sees 3 members"
    );
    // A must learn about C via MemberJoin flood relayed through B
    assert_eq!(
        net.engines[0].groups[&gid].members.len(),
        3,
        "A sees 3 members: {:?}",
        net.ui[0]
    );

    // announces so A learns topology (A-B, B-C) → route A→C via B
    net.run(now, now + 65_000, 500);
    now += 65_500;
    let route = net.engines[0]
        .topology
        .route(&a, &net.engines[0].neighbours, &c, 3);
    assert!(
        route.is_some(),
        "A has a route to C: topo={:?}",
        net.engines[0].topology.adj.keys().collect::<Vec<_>>()
    );
    assert_eq!(route.unwrap().path, vec![b, c]);

    // A talks; C must hear
    let acts = net.engines[0].ptt_down(Priority::Normal, now);
    net.apply(0, acts);
    net.pump(now);
    now += 300;
    net.tick_all(now);
    assert!(net.has_ui(0, |e| matches!(e, UiEvent::FloorGranted)));
    for _ in 0..50 {
        now += 20;
        let acts = net.engines[0].on_audio_in(&pcm_tone(960, 0.05), now);
        net.apply(0, acts);
        net.tick_all(now);
    }
    assert!(
        net.plays[2] > 30,
        "C heard A through B ({} frames)",
        net.plays[2]
    );
    assert!(
        net.plays[1] > 30,
        "B heard A directly ({} frames)",
        net.plays[1]
    );
    assert!(net.has_ui(
        2,
        |e| matches!(e, UiEvent::FloorTaken { name, .. } if name == "A")
    ));
}

#[test]
fn member_that_missed_memberjoin_is_learned_from_voice() {
    // A hosts, B joins while C is offline (never hears B's MemberJoin flood).
    // Later C hears B talk: authenticated group voice must add B to C's roster.
    let mut net = Net::new(3, &["A", "B", "C"]);
    let mut now = 1_789_404_000_000u64;
    net.bring_up(now);
    net.run(now, now + 600, 20);
    now += 620;
    let (gid, acts) = net.engines[0].create_group("Late", now).unwrap();
    net.apply(0, acts);
    let a = net.engines[0].node_id();
    let b = net.engines[1].node_id();
    let c = net.engines[2].node_id();
    // C joins first
    let acts = net.engines[0].invite_peer(gid, c, now);
    net.apply(0, acts);
    net.pump(now);
    let acts = net.engines[2].accept_invite(gid, a, now);
    net.apply(2, acts);
    net.pump(now);
    // C goes deaf; B joins
    net.adj = vec![vec![1], vec![0], vec![]];
    let acts = net.engines[0].invite_peer(gid, b, now);
    net.apply(0, acts);
    net.pump(now);
    let acts = net.engines[1].accept_invite(gid, a, now);
    net.apply(1, acts);
    net.pump(now);
    assert!(
        !net.engines[2].groups[&gid].members.contains_key(&b),
        "precondition: C missed B"
    );
    // C is back in range of B only; B talks
    net.adj = vec![vec![1], vec![0, 2], vec![1]];
    net.run(now, now + 5_000, 100);
    now += 5_200;
    let acts = net.engines[1].ptt_down(Priority::Normal, now);
    net.apply(1, acts);
    net.pump(now);
    now += 300;
    net.tick_all(now);
    for _ in 0..25 {
        now += 20;
        let acts = net.engines[1].on_audio_in(&pcm_tone(960, 0.05), now);
        net.apply(1, acts);
        net.tick_all(now);
    }
    assert!(
        net.engines[2].groups[&gid].members.contains_key(&b),
        "C learned B from its voice: {:?}",
        net.ui[2]
    );
    assert!(net.has_ui(
        2,
        |e| matches!(e, UiEvent::MemberJoined { node, .. } if *node == b)
    ));
}

fn capture_profile(acts: &[Action]) -> Option<titi_core::frame::Profile> {
    acts.iter().find_map(|a| match a {
        Action::Capture {
            active: true,
            profile,
        } => Some(*profile),
        _ => None,
    })
}

#[test]
fn quality_setting_caps_the_capture_profile() {
    use titi_core::frame::Profile;
    let mut net = Net::new(2, &["Ana", "Ceas"]);
    let mut now = 1_789_405_000_000u64;
    net.bring_up(now);
    net.run(now, now + 500, 20);
    now += 520;
    let _gid = group_of_all(&mut net, now);
    let auto =
        capture_profile(&net.engines[0].ptt_down(Priority::Normal, now)).expect("capture on");
    let _ = net.engines[0].ptt_up(now + 100);
    assert!(
        auto <= Profile::Std,
        "automatic profile on LAN is Std or better, got {auto:?}"
    );
    net.engines[0].cfg.max_profile = Some(Profile::Low);
    let capped = capture_profile(&net.engines[0].ptt_down(Priority::Normal, now + 2_000))
        .expect("capture on");
    assert_eq!(
        capped,
        Profile::Low,
        "user cap Low wins over a better automatic profile"
    );
    let _ = net.engines[0].ptt_up(now + 2_100);
    net.engines[0].cfg.max_profile = Some(Profile::Hq);
    let loose = capture_profile(&net.engines[0].ptt_down(Priority::Normal, now + 4_000))
        .expect("capture on");
    assert_eq!(
        loose, auto,
        "a cap above the automatic profile changes nothing"
    );
}
