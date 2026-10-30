//! Message model v2 + durable store across real engines (CH-A1, CH-A2):
//! reply / react / edit / delete / forward / mention flow over a simulated LAN,
//! forged events are rejected, search and drafts work, and the store survives
//! a restart (SQLite on native, mirrored rows on web).

use std::collections::VecDeque;

use titi_core::chat::{Compose, ReplyState};
use titi_core::engine::{Action, Config, Engine, MessageBody, UiEvent};
use titi_core::identity::Identity;
use titi_core::link::LinkClass;

const LAN: u32 = 1;

struct Net {
    engines: Vec<Engine>,
    queue: VecDeque<(usize, String, Vec<u8>)>,
    ui: Vec<Vec<UiEvent>>,
    persisted: Vec<Vec<(String, Vec<u8>)>>,
}

fn tok(i: usize) -> String {
    format!("peer{i}")
}

impl Net {
    fn new(n: usize) -> Self {
        Net {
            engines: (0..n).map(engine).collect(),
            queue: VecDeque::new(),
            ui: vec![vec![]; n],
            persisted: vec![vec![]; n],
        }
    }
    fn apply(&mut self, from: usize, acts: Vec<Action>) {
        let acts = self.engines[from].shape(acts);
        for a in acts {
            match a {
                Action::Send { peer, bytes, .. } => {
                    let targets: Vec<usize> = match peer {
                        Some(t) => vec![t
                            .trim_start_matches("peer")
                            .split('#')
                            .next()
                            .unwrap()
                            .parse()
                            .unwrap()],
                        None => (0..self.engines.len()).filter(|j| *j != from).collect(),
                    };
                    for t in targets {
                        self.queue.push_back((t, tok(from), bytes.clone()));
                    }
                }
                Action::Ui(e) => self.ui[from].push(e),
                Action::Persist { key, value } => self.persisted[from].push((key, value)),
                _ => {}
            }
        }
    }
    fn settle(&mut self, now: &mut u64, ms: u64) {
        let end = *now + ms;
        let mut guard = 0;
        while *now <= end {
            while let Some((to, from, bytes)) = self.queue.pop_front() {
                let acts = self.engines[to].on_frame(LAN, from, &bytes, *now);
                self.apply(to, acts);
                guard += 1;
                assert!(guard < 50_000, "message storm");
            }
            for i in 0..self.engines.len() {
                let acts = self.engines[i].tick(*now);
                self.apply(i, acts);
            }
            *now += 20;
        }
    }
    fn item(&self, i: usize, gid: &[u8; 16], id: &[u8; 16]) -> titi_core::chat::ChatItem {
        self.engines[i]
            .history(gid, None, 500)
            .into_iter()
            .find(|x| &x.msg_uuid == id)
            .unwrap_or_else(|| panic!("engine {i} lacks message"))
    }
}

fn engine(i: usize) -> Engine {
    let cfg = Config {
        display_name: format!("N{i}"),
        kdf: titi_core::crypto::KdfParams::LIGHT,
        ..Default::default()
    };
    Engine::new(Identity::from_seed(&[(i + 1) as u8; 32]), cfg, 7 + i as u64)
}

fn last_sent_id(net: &Net, i: usize) -> [u8; 16] {
    net.ui[i]
        .iter()
        .rev()
        .find_map(|e| match e {
            UiEvent::ChatUpdated { item } if item.from == net.engines[i].node_id() => {
                Some(item.msg_uuid)
            }
            _ => None,
        })
        .unwrap()
}

fn setup(n: usize) -> (Net, [u8; 16], u64) {
    let mut net = Net::new(n);
    let mut now = 1_790_000_000_000u64;
    for i in 0..n {
        let acts = net.engines[i].on_link_up(LAN, LinkClass::Lan, None, now);
        net.apply(i, acts);
        for j in 0..n {
            if j != i {
                let acts = net.engines[i].on_peer_seen(LAN, tok(j), now);
                net.apply(i, acts);
            }
        }
    }
    net.settle(&mut now, 600);
    let (gid, acts) = net.engines[0].create_group("Munte", now).unwrap();
    net.apply(0, acts);
    let host = net.engines[0].node_id();
    for i in 1..n {
        let node = net.engines[i].node_id();
        let acts = net.engines[0].invite_peer(gid, node, now);
        net.apply(0, acts);
        net.settle(&mut now, 300);
        let acts = net.engines[i].accept_invite(gid, host, now);
        net.apply(i, acts);
        net.settle(&mut now, 600);
    }
    (net, gid, now)
}

#[test]
fn reply_react_edit_delete_between_engines() {
    let (mut net, gid, mut now) = setup(2);
    let acts = net.engines[0].send_text(gid, "Plecăm la 8 din tabără", now);
    net.apply(0, acts);
    net.settle(&mut now, 400);
    let orig = last_sent_id(&net, 0);
    assert_eq!(
        net.item(1, &gid, &orig).body,
        Some(MessageBody::Text("Plecăm la 8 din tabără".into()))
    );

    // B replies with a mention and reacts
    let a = net.engines[0].node_id();
    let c = Compose {
        reply_to: Some(orig),
        mentions: vec![a],
        mention_all: false,
    };
    let acts = net.engines[1].send_text_v2(gid, "OK, vin și eu", c, now);
    net.apply(1, acts);
    let acts = net.engines[1].react(gid, orig, "👍", now).unwrap();
    net.apply(1, acts);
    net.settle(&mut now, 400);
    let reply = last_sent_id(&net, 1);
    let r = net.item(0, &gid, &reply);
    assert!(r.mentions_me, "A is mentioned");
    let q = r.reply.unwrap();
    assert_eq!(q.state, ReplyState::Present);
    assert_eq!(q.msg_uuid, orig);
    let o = net.item(0, &gid, &orig);
    assert_eq!(o.reactions.len(), 1);
    assert_eq!(o.reactions[0].emoji, "👍");

    // A edits; both see it and the reply quote follows
    let acts = net.engines[0]
        .edit_message(gid, orig, "Plecăm la 9 din tabără", now)
        .unwrap();
    net.apply(0, acts);
    net.settle(&mut now, 400);
    for i in 0..2 {
        let o = net.item(i, &gid, &orig);
        assert_eq!(
            o.body,
            Some(MessageBody::Text("Plecăm la 9 din tabără".into()))
        );
        assert_eq!(o.edited, 1);
    }
    assert_eq!(net.engines[1].edit_history(&orig).len(), 2);
    assert_eq!(
        net.item(1, &gid, &reply).reply.unwrap().preview,
        "Plecăm la 9 din tabără"
    );

    // B cannot edit or delete A's message
    assert!(net.engines[1].edit_message(gid, orig, "x", now).is_err());
    assert!(net.engines[1].delete_message(gid, orig, now).is_err());

    // search finds it diacritic-blind on both
    for i in 0..2 {
        assert_eq!(net.engines[i].search(Some(&gid), "tabara", 10).len(), 1);
    }

    // A deletes for everyone → tombstone, search empty, quote cleared
    let before = net.ui[1].len();
    let acts = net.engines[0].delete_message(gid, orig, now).unwrap();
    net.apply(0, acts);
    net.settle(&mut now, 400);
    // the receiver is told the REPLY changed too (UIs replace rows only on ChatUpdated)
    assert!(
        net.ui[1][before..].iter().any(|e| matches!(e,
            UiEvent::ChatUpdated { item } if item.msg_uuid == reply
                && item.reply.as_ref().is_some_and(|q| q.state == ReplyState::Deleted))),
        "B got a refreshed reply row"
    );
    for i in 0..2 {
        let o = net.item(i, &gid, &orig);
        assert!(o.deleted && o.body.is_none() && o.reactions.is_empty());
        assert!(net.engines[i].search(None, "tabara", 10).is_empty());
        assert_eq!(
            net.item(i, &gid, &reply).reply.unwrap().state,
            ReplyState::Deleted
        );
    }
}

#[test]
fn edit_window_is_enforced_by_sender() {
    let (mut net, gid, mut now) = setup(2);
    let acts = net.engines[0].send_text(gid, "old", now);
    net.apply(0, acts);
    net.settle(&mut now, 200);
    let id = last_sent_id(&net, 0);
    let late = now + titi_core::chat::EDIT_WINDOW_MS + 1;
    assert!(net.engines[0].edit_message(gid, id, "new", late).is_err());
    // delete has no time limit
    let acts = net.engines[0].delete_message(gid, id, late).unwrap();
    net.apply(0, acts);
    net.settle(&mut now, 200);
    assert!(net.item(1, &gid, &id).deleted);
}

#[test]
fn forward_counts_and_mention_all_is_creator_only() {
    let (mut net, gid, mut now) = setup(3);
    let acts = net.engines[1].send_text(gid, "notă", now);
    net.apply(1, acts);
    net.settle(&mut now, 300);
    let id = last_sent_id(&net, 1);
    // second group hosted by node 2 with node 0
    let (g2, acts) = net.engines[2].create_group("Acasă", now).unwrap();
    net.apply(2, acts);
    let n0 = net.engines[0].node_id();
    let host = net.engines[2].node_id();
    let acts = net.engines[2].invite_peer(g2, n0, now);
    net.apply(2, acts);
    net.settle(&mut now, 300);
    let acts = net.engines[0].accept_invite(g2, host, now);
    net.apply(0, acts);
    net.settle(&mut now, 600);
    let acts = net.engines[0].forward(gid, id, g2, now).unwrap();
    net.apply(0, acts);
    net.settle(&mut now, 300);
    let f = last_sent_id(&net, 0);
    let it = net.item(2, &g2, &f);
    assert_eq!(it.forwarded, 1);
    assert_eq!(it.body, Some(MessageBody::Text("notă".into())));

    // @all from a non-creator (node 1) does not ping; from the creator it does
    let c = Compose {
        mention_all: true,
        ..Default::default()
    };
    let acts = net.engines[1].send_text_v2(gid, "toți", c.clone(), now);
    net.apply(1, acts);
    let acts = net.engines[0].send_text_v2(gid, "toți, real", c, now);
    net.apply(0, acts);
    net.settle(&mut now, 300);
    let a1 = last_sent_id(&net, 1);
    let a0 = last_sent_id(&net, 0);
    assert!(!net.item(2, &gid, &a1).mentions_me);
    assert!(net.item(2, &gid, &a0).mentions_me);
}

#[test]
fn drafts_clear_on_send_and_leave_wipes_history() {
    let (mut net, gid, mut now) = setup(2);
    net.engines[1].set_draft(gid, "jumătate");
    assert_eq!(net.engines[1].draft(&gid).as_deref(), Some("jumătate"));
    let acts = net.engines[1].send_text(gid, "gata", now);
    net.apply(1, acts);
    net.settle(&mut now, 300);
    assert_eq!(net.engines[1].draft(&gid), None);
    assert_eq!(net.engines[0].history(&gid, None, 10).len(), 1);
    let acts = net.engines[0].leave_group(gid, now);
    net.apply(0, acts);
    assert!(net.engines[0].history(&gid, None, 10).is_empty());
    assert!(net.engines[0].search(None, "gata", 10).is_empty());
}

#[test]
fn mirrored_store_survives_restart() {
    let (mut net, gid, mut now) = setup(2);
    net.engines[1].set_chat_backend(Box::new(titi_core::chat::MemBackend::new(true)));
    let acts = net.engines[0].send_text(gid, "rămâne după restart", now);
    net.apply(0, acts);
    net.settle(&mut now, 300);
    let id = last_sent_id(&net, 0);
    let acts = net.engines[1].react(gid, id, "❤️", now).unwrap();
    net.apply(1, acts);
    net.engines[1].set_draft(gid, "ciornă");
    net.apply(1, vec![]);
    let rows = net.persisted[1].clone();
    assert!(rows.iter().any(|(k, _)| k.starts_with("chat/m/")));
    // fresh engine + host replays rows (last value per key)
    let mut fresh = engine(1);
    fresh.set_chat_backend(Box::new(titi_core::chat::MemBackend::new(true)));
    let groups = rows
        .iter()
        .rev()
        .find(|(k, _)| k == "groups")
        .unwrap()
        .1
        .clone();
    fresh.restore_groups(&groups, now).unwrap();
    let mut kv = std::collections::BTreeMap::new();
    for (k, v) in rows.into_iter().filter(|(k, _)| k.starts_with("chat/")) {
        if k.ends_with('/') {
            kv.retain(|x: &String, _| !x.starts_with(&k));
        } else if v.is_empty() {
            kv.remove(&k);
        } else {
            kv.insert(k, v);
        }
    }
    for (k, v) in &kv {
        assert!(fresh.restore_chat_row(k, v));
    }
    let h = fresh.history(&gid, None, 10);
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].reactions[0].emoji, "❤️");
    assert_eq!(fresh.draft(&gid).as_deref(), Some("ciornă"));
    assert_eq!(fresh.search(None, "ramane", 5).len(), 1);
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_store_survives_restart() {
    let dir = std::env::temp_dir().join(format!("titi-chat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("chat.db");
    let _ = std::fs::remove_file(&path);
    let p = path.to_str().unwrap().to_string();
    let (mut net, gid, mut now) = setup(2);
    net.engines[1].set_chat_backend(Box::new(
        titi_core::chat::sqlite::SqliteBackend::open(&p).unwrap(),
    ));
    let acts = net.engines[0].send_text(gid, "Ştiinţă pe disc", now);
    net.apply(0, acts);
    net.settle(&mut now, 300);
    net.engines[1].set_draft(gid, "draft");
    drop(net);
    let mut fresh = engine(1);
    fresh.set_chat_backend(Box::new(
        titi_core::chat::sqlite::SqliteBackend::open(&p).unwrap(),
    ));
    assert_eq!(fresh.history(&gid, None, 10).len(), 1);
    assert_eq!(fresh.search(None, "stiinta", 5).len(), 1);
    assert_eq!(fresh.draft(&gid).as_deref(), Some("draft"));
}

#[test]
fn forged_event_with_wrong_key_is_ignored() {
    // engine 1 crafts an edit of A's message signed with its own key but
    // claiming A's sender id: the receiver must reject it.
    use prost::Message;
    use titi_core::proto;
    let (mut net, gid, mut now) = setup(2);
    let acts = net.engines[0].send_text(gid, "adevărat", now);
    net.apply(0, acts);
    net.settle(&mut now, 300);
    let id = last_sent_id(&net, 0);
    let a = net.engines[0].node_id();
    let mut cm = proto::ChatMessage {
        msg_uuid: vec![0x42; 16].into(),
        group_uuid: gid.to_vec().into(),
        sender_id: a.to_vec().into(),
        sent_ms: now,
        sender_pub: net.engines[1].id.ed25519_public().to_vec().into(),
        body: Some(proto::chat_message::Body::Edit(proto::Edit {
            target: id.to_vec().into(),
            text: "fals".into(),
        })),
        ..Default::default()
    };
    let unsigned = cm.encode_to_vec();
    let sig = net.engines[1]
        .id
        .sign(&titi_core::engine::chat_sig_input(&unsigned));
    cm.signature = sig.to_vec().into();
    let mut plain = unsigned.clone();
    plain.extend_from_slice(&titi_core::engine::chat_sig_trailer(&sig));
    let decoded = proto::ChatMessage::decode(plain.as_slice()).unwrap();
    assert!(!titi_core::engine::verify_chat_signature(&decoded, &plain));
    // the genuine path verifies
    let mut ok = decoded.clone();
    ok.sender_id = net.engines[1].node_id().to_vec().into();
    ok.signature = Default::default();
    let u2 = ok.encode_to_vec();
    let s2 = net.engines[1]
        .id
        .sign(&titi_core::engine::chat_sig_input(&u2));
    let mut p2 = u2;
    p2.extend_from_slice(&titi_core::engine::chat_sig_trailer(&s2));
    let d2 = proto::ChatMessage::decode(p2.as_slice()).unwrap();
    assert!(titi_core::engine::verify_chat_signature(&d2, &p2));
    // and the store ignores an unsigned edit claiming to be A
    assert!(net.engines[0]
        .chat
        .ingest(gid, decoded, None, now)
        .is_empty());
    assert_eq!(
        net.item(0, &gid, &id).body,
        Some(MessageBody::Text("adevărat".into()))
    );
}
