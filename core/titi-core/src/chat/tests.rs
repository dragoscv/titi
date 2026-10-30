use super::*;

const G: GroupId = [7; 16];
const ALICE: NodeId = [1; 8];
const BOB: NodeId = [2; 8];

fn msg(id: u8, from: NodeId, sent: Ms, body: Body) -> proto::ChatMessage {
    proto::ChatMessage {
        msg_uuid: vec![id; 16].into(),
        group_uuid: G.to_vec().into(),
        sender_id: from.to_vec().into(),
        sent_ms: sent,
        body: Some(body),
        ..Default::default()
    }
}
fn text(id: u8, from: NodeId, sent: Ms, t: &str) -> proto::ChatMessage {
    msg(id, from, sent, Body::Text(t.into()))
}
fn edit(id: u8, from: NodeId, sent: Ms, target: u8, t: &str) -> proto::ChatMessage {
    msg(
        id,
        from,
        sent,
        Body::Edit(proto::Edit {
            target: vec![target; 16].into(),
            text: t.into(),
        }),
    )
}
fn del(id: u8, from: NodeId, sent: Ms, target: u8) -> proto::ChatMessage {
    msg(
        id,
        from,
        sent,
        Body::Delete(proto::Delete {
            target: vec![target; 16].into(),
        }),
    )
}
fn react(id: u8, from: NodeId, sent: Ms, target: u8, e: &str) -> proto::ChatMessage {
    msg(
        id,
        from,
        sent,
        Body::Reaction(proto::Reaction {
            target: vec![target; 16].into(),
            emoji: e.into(),
        }),
    )
}

fn backends() -> Vec<(&'static str, Chat)> {
    let mut v: Vec<(&'static str, Chat)> = vec![
        ("mem", Chat::default()),
        ("mem-mirror", Chat::new(Box::new(MemBackend::new(true)))),
    ];
    #[cfg(feature = "sqlite")]
    v.push((
        "sqlite",
        Chat::new(Box::new(sqlite::SqliteBackend::open(":memory:").unwrap())),
    ));
    v
}

fn item(c: &Chat, id: u8) -> ChatItem {
    c.view(&c.get(&[id; 16]).unwrap(), BOB, Some(ALICE))
        .unwrap()
}

#[test]
fn edits_follow_window_author_and_cap() {
    for (name, mut c) in backends() {
        c.ingest(G, text(1, ALICE, 1000, "salut"), Some(ALICE), 1000);
        // not the author
        assert!(c
            .ingest(G, edit(2, BOB, 2000, 1, "hack"), Some(BOB), 2000)
            .is_empty());
        // unsigned
        assert!(c
            .ingest(G, edit(3, ALICE, 2000, 1, "x"), None, 2000)
            .is_empty());
        // ok, then a later one wins regardless of arrival order
        c.ingest(G, edit(5, ALICE, 3000, 1, "salut v3"), Some(ALICE), 3000);
        c.ingest(G, edit(4, ALICE, 2500, 1, "salut v2"), Some(ALICE), 3100);
        let it = item(&c, 1);
        assert_eq!(
            it.body,
            Some(MessageBody::Text("salut v3".into())),
            "{name}"
        );
        assert_eq!(it.edited, 2, "{name}");
        assert_eq!(c.edit_history(&[1; 16]).len(), 3, "{name}");
        // replay of the same edit event is ignored
        assert!(c
            .ingest(G, edit(5, ALICE, 3000, 1, "salut v3"), Some(ALICE), 3200)
            .is_empty());
        // outside the 24 h window (+5 min skew)
        let late = 1000 + EDIT_WINDOW_MS + CLOCK_SKEW_MS + 1;
        assert!(c
            .ingest(G, edit(6, ALICE, late, 1, "late"), Some(ALICE), late)
            .is_empty());
        for i in 0..20u8 {
            c.ingest(
                G,
                edit(100 + i, ALICE, 4000 + i as u64, 1, "n"),
                Some(ALICE),
                5000,
            );
        }
        assert_eq!(item(&c, 1).edited as usize, MAX_EDITS, "{name}");
    }
}

#[test]
fn delete_is_a_tombstone_that_beats_everything() {
    for (name, mut c) in backends() {
        c.ingest(G, text(1, ALICE, 1000, "secret plan"), Some(ALICE), 1000);
        c.ingest(G, react(2, BOB, 1100, 1, "👍"), Some(BOB), 1100);
        assert_eq!(c.backend.search(None, "secret", 10).len(), 1, "{name}");
        assert!(
            c.ingest(G, del(3, BOB, 1200, 1), Some(BOB), 1200)
                .is_empty(),
            "{name}"
        );
        c.ingest(G, del(4, ALICE, 1300, 1), Some(ALICE), 1300);
        let it = item(&c, 1);
        assert!(
            it.deleted && it.body.is_none() && it.reactions.is_empty(),
            "{name}"
        );
        assert!(c.backend.search(None, "secret", 10).is_empty(), "{name}");
        // later edit / reaction are ignored
        assert!(c
            .ingest(G, edit(5, ALICE, 1400, 1, "x"), Some(ALICE), 1400)
            .is_empty());
        assert!(c
            .ingest(G, react(6, BOB, 1500, 1, "❤️"), Some(BOB), 1500)
            .is_empty());
    }
}

#[test]
fn delete_before_original_hides_the_late_original() {
    for (name, mut c) in backends() {
        c.ingest(G, del(9, ALICE, 2000, 1), Some(ALICE), 2000);
        assert!(c.backend.history(&G, None, 10).is_empty(), "{name}");
        // a forger cannot use the placeholder to suppress Bob's message
        c.ingest(G, text(1, ALICE, 1000, "oops"), Some(ALICE), 2100);
        let it = item(&c, 1);
        assert!(it.deleted && it.body.is_none(), "{name}");
        assert!(c.backend.search(None, "oops", 10).is_empty(), "{name}");
        assert_eq!(c.backend.history(&G, None, 10).len(), 1, "{name}");
    }
}

#[test]
fn events_before_target_wait_then_apply() {
    for (name, mut c) in backends() {
        c.ingest(G, edit(2, ALICE, 2000, 1, "fixed"), Some(ALICE), 2000);
        c.ingest(G, react(3, BOB, 2100, 1, "😂"), Some(BOB), 2100);
        let out = c.ingest(G, text(1, ALICE, 1000, "fixd"), Some(ALICE), 2200);
        assert_eq!(out, vec![([1; 16], true)], "{name}");
        let it = item(&c, 1);
        assert_eq!(it.body, Some(MessageBody::Text("fixed".into())), "{name}");
        assert_eq!(it.reactions.len(), 1, "{name}");
        assert!(it.reactions[0].mine, "{name}");
        // expiry drops stale pending events
        c.ingest(G, react(4, BOB, 3000, 8, "👍"), Some(BOB), 3000);
        c.expire_pending(3000 + PENDING_TTL_MS + 1);
        c.ingest(
            G,
            text(8, ALICE, 2900, "late"),
            Some(ALICE),
            3000 + PENDING_TTL_MS + 2,
        );
        assert!(item(&c, 8).reactions.is_empty(), "{name}");
    }
}

#[test]
fn reactions_one_per_member_last_wins_empty_removes() {
    for (name, mut c) in backends() {
        c.ingest(G, text(1, ALICE, 1000, "hi"), Some(ALICE), 1000);
        c.ingest(G, react(2, BOB, 1100, 1, "👍"), Some(BOB), 1100);
        c.ingest(G, react(3, ALICE, 1150, 1, "👍"), Some(ALICE), 1150);
        c.ingest(G, react(4, BOB, 1200, 1, "❤️"), Some(BOB), 1200);
        c.ingest(G, react(5, BOB, 1050, 1, "😢"), Some(BOB), 1250); // older, loses
        let it = item(&c, 1);
        assert_eq!(it.reactions.len(), 2, "{name}");
        assert!(
            it.reactions.iter().any(|r| r.emoji == "❤️" && r.mine),
            "{name}"
        );
        c.ingest(G, react(6, BOB, 1300, 1, ""), Some(BOB), 1300);
        let it = item(&c, 1);
        assert_eq!(it.reactions.len(), 1, "{name}");
        assert_eq!(it.reactions[0].emoji, "👍", "{name}");
    }
}

#[test]
fn replies_render_present_deleted_and_missing() {
    for (name, mut c) in backends() {
        c.ingest(G, text(1, ALICE, 1000, "original"), Some(ALICE), 1000);
        let q = reply_ref(&c.get(&[1; 16]).unwrap()).unwrap();
        let mut r = text(2, BOB, 1100, "answer");
        r.reply_to = Some(q.clone());
        c.ingest(G, r, Some(BOB), 1100);
        c.ingest(
            G,
            edit(3, ALICE, 1200, 1, "original (edited)"),
            Some(ALICE),
            1200,
        );
        let rv = item(&c, 2).reply.unwrap();
        assert_eq!(rv.state, ReplyState::Present, "{name}");
        assert_eq!(rv.preview, "original (edited)", "{name}");
        c.ingest(G, del(4, ALICE, 1300, 1), Some(ALICE), 1300);
        let rv = item(&c, 2).reply.unwrap();
        assert_eq!(rv.state, ReplyState::Deleted, "{name}");
        assert!(rv.preview.is_empty(), "{name}");
        let mut m = text(5, BOB, 1400, "re: never seen");
        m.reply_to = Some(proto::ReplyRef {
            msg_uuid: vec![99; 16].into(),
            sender_id: ALICE.to_vec().into(),
            preview: "x".repeat(200),
            kind: proto::ReplyKind::Text as i32,
        });
        c.ingest(G, m, Some(BOB), 1400);
        let rv = item(&c, 5).reply.unwrap();
        assert_eq!(rv.state, ReplyState::Missing, "{name}");
        assert_eq!(rv.preview.chars().count(), PREVIEW_CHARS, "{name}");
    }
}

#[test]
fn mention_all_only_from_creator() {
    let mut c = Chat::default();
    let mut a = text(1, ALICE, 1000, "@all meet");
    a.mention_all = true;
    c.ingest(G, a, Some(ALICE), 1000);
    let mut b = text(2, [3; 8], 1000, "@all spam");
    b.mention_all = true;
    c.ingest(G, b, Some([3; 8]), 1000);
    let mut d = text(3, [3; 8], 1000, "@bob");
    d.mentions = vec![BOB.to_vec().into()];
    c.ingest(G, d, Some([3; 8]), 1000);
    assert!(item(&c, 1).mentions_me);
    assert!(!item(&c, 2).mentions_me && !item(&c, 2).mention_all);
    assert!(item(&c, 3).mentions_me);
}

#[test]
fn search_folds_romanian_diacritics_and_prefixes() {
    for (name, mut c) in backends() {
        c.ingest(
            G,
            text(1, ALICE, 1000, "Ştiinţă și tehnică"),
            Some(ALICE),
            1000,
        );
        c.ingest(G, text(2, ALICE, 2000, "altceva"), Some(ALICE), 2000);
        c.ingest(
            G,
            text(3, ALICE, 3000, "știința e frumoasă"),
            Some(ALICE),
            3000,
        );
        for q in ["stiinta", "știință", "ştiinţă", "STIIN", "si teh"] {
            let r = c.backend.search(Some(&G), q, 10);
            assert!(!r.is_empty(), "{name}: {q}");
        }
        let r = c.backend.search(None, "stiint", 10);
        assert_eq!(r.len(), 2, "{name}");
        // newest first
        assert_eq!(
            &r[0].1.msg.as_ref().unwrap().msg_uuid[..],
            &[3; 16],
            "{name}"
        );
        assert!(c.backend.search(None, "", 10).is_empty(), "{name}");
        assert!(
            c.backend.search(Some(&[8; 16]), "stiinta", 10).is_empty(),
            "{name}"
        );
        // edits re-index
        c.ingest(G, edit(4, ALICE, 2100, 2, "nou cuvant"), Some(ALICE), 2100);
        assert!(c.backend.search(None, "altceva", 10).is_empty(), "{name}");
        assert_eq!(c.backend.search(None, "cuvânt", 10).len(), 1, "{name}");
    }
}

#[test]
fn history_pages_and_drafts_and_wipe() {
    for (name, mut c) in backends() {
        for i in 1..=30u8 {
            c.ingest(G, text(i, ALICE, i as u64 * 10, "m"), Some(ALICE), 500);
        }
        let last = c.backend.history(&G, None, 10);
        assert_eq!(last.len(), 10, "{name}");
        assert_eq!(last[9].msg.as_ref().unwrap().sent_ms, 300, "{name}");
        let older = c.backend.history(&G, Some(210), 10);
        assert_eq!(
            older.last().unwrap().msg.as_ref().unwrap().sent_ms,
            200,
            "{name}"
        );
        c.set_draft(&G, "ciornă");
        assert_eq!(c.draft(&G).as_deref(), Some("ciornă"), "{name}");
        c.set_draft(&G, "");
        assert_eq!(c.draft(&G), None, "{name}");
        c.set_draft(&G, "again");
        c.wipe_conv(&G);
        assert!(c.backend.history(&G, None, 10).is_empty(), "{name}");
        assert_eq!(c.draft(&G), None, "{name}");
    }
}

#[test]
fn future_timestamps_sort_at_receive_time() {
    let mut c = Chat::default();
    c.ingest(
        G,
        text(1, ALICE, 10_000_000_000, "from the future"),
        Some(ALICE),
        1000,
    );
    c.ingest(G, text(2, ALICE, 2000, "now"), Some(ALICE), 2000);
    let h = c.backend.history(&G, None, 10);
    assert_eq!(&h[1].msg.as_ref().unwrap().msg_uuid[..], &[1; 16]);
    assert_eq!(sort_ms(&h[1]), 1000 + FUTURE_SKEW_MS);
}

#[test]
fn mirror_rows_restore_into_a_fresh_store() {
    let mut a = Chat::new(Box::new(MemBackend::new(true)));
    a.ingest(G, text(1, ALICE, 1000, "persist me"), Some(ALICE), 1000);
    a.ingest(G, react(2, BOB, 1100, 1, "🙏"), Some(BOB), 1100);
    a.ingest(G, react(3, BOB, 1200, 9, "👍"), Some(BOB), 1200); // pending
    a.set_draft(&G, "half typed");
    let rows = std::mem::take(&mut a.mirror_out);
    // host keeps last value per key, drops empty values / prefixes
    let mut kv: std::collections::BTreeMap<String, Vec<u8>> = Default::default();
    for (k, v) in rows {
        if k.ends_with('/') {
            kv.retain(|x, _| !x.starts_with(&k));
        } else if v.is_empty() {
            kv.remove(&k);
        } else {
            kv.insert(k, v);
        }
    }
    let mut b = Chat::new(Box::new(MemBackend::new(true)));
    for (k, v) in &kv {
        assert!(b.restore_row(k, v), "{k}");
    }
    assert!(b.mirror_out.is_empty());
    assert_eq!(b.draft(&G).as_deref(), Some("half typed"));
    assert_eq!(b.backend.search(None, "persist", 5).len(), 1);
    let it = b.view(&b.get(&[1; 16]).unwrap(), BOB, None).unwrap();
    assert_eq!(it.reactions[0].emoji, "🙏");
    b.ingest(G, text(9, ALICE, 1300, "target"), Some(ALICE), 1300);
    let it = b.view(&b.get(&[9; 16]).unwrap(), BOB, None).unwrap();
    assert_eq!(it.reactions.len(), 1);
}

#[test]
fn fold_matches_known_romanian_forms() {
    assert_eq!(fold("ĂÂÎȘȚăâîșțŞŢşţ"), "aaistaaiststst");
    assert_eq!(
        tokens("Ştiinţă, și-o pâine!"),
        vec!["stiinta", "si", "o", "paine"]
    );
}
