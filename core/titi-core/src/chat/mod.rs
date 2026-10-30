//! Durable per-conversation store and message model v2 (ADR-0009: CH-A2, CH-A1).
//!
//! The core owns the schema. Native hosts open a SQLite + FTS5 database
//! ([`sqlite::SqliteBackend`], feature `sqlite`); web and Tizen use
//! [`MemBackend`], which mirrors every write as an `Action::Persist` row
//! (`chat/…` keys) that the host keeps in IndexedDB and replays with
//! `Engine::restore_chat_row` at start.
//!
//! Rules (decided 2026-10-30, research in the CH-A1 tracker row):
//! - reactions, edits and deletes are signed events; only the author of the
//!   target may edit or delete it; one reaction per member, last
//!   `(sent_ms, event id)` wins, empty emoji removes it;
//! - edits: text only, ≤ [`MAX_EDITS`], within [`EDIT_WINDOW_MS`] of the
//!   original on the author's clocks (± [`CLOCK_SKEW_MS`]); history is kept;
//! - delete for everyone: no time limit, leaves a tombstone, wipes body,
//!   quote, mentions, edits, reactions and the search row, beats everything
//!   whatever the arrival order (a delete that arrives first leaves a
//!   placeholder so the late original is never shown);
//! - an event whose target is unknown waits [`PENDING_TTL_MS`].

#[cfg(feature = "sqlite")]
pub mod sqlite;

use std::collections::HashMap;

use crate::engine::{GroupId, MessageBody};
use crate::frame::{NodeId, Profile};
use crate::proto::{self, chat_message::Body};
use crate::time::Ms;

pub const EDIT_WINDOW_MS: u64 = 24 * 60 * 60 * 1000;
pub const CLOCK_SKEW_MS: u64 = 5 * 60 * 1000;
pub const MAX_EDITS: usize = 10;
pub const PENDING_TTL_MS: u64 = 7 * 24 * 60 * 60 * 1000;
pub const PREVIEW_CHARS: usize = 80;
pub const MAX_REACTIONS_SHOWN: usize = 20;
pub const MAX_EMOJI_BYTES: usize = 32;
pub const FORWARDED_MANY: u32 = 5;
/// Quick reactions offered on every screen (watch / TV pick from these).
pub const QUICK_REACTIONS: [&str; 6] = ["👍", "❤️", "😂", "😮", "😢", "🙏"];
/// A `sent_ms` this far ahead of our clock sorts at receive time instead.
const FUTURE_SKEW_MS: u64 = 10 * 60 * 1000;

pub type MsgId = [u8; 16];

/// Storage backend. Infallible by contract: a failing write is dropped (the
/// message still shows live and stays in the store-and-forward queue).
pub trait ChatBackend: Send {
    fn get(&self, id: &MsgId) -> Option<proto::StoredMessage>;
    /// Upsert a row; `search` = text to index (None removes it from search).
    fn put(&mut self, conv: &GroupId, row: &proto::StoredMessage, search: Option<&str>);
    /// Up to `limit` rows of `conv` older than `before_ms`, oldest first.
    fn history(
        &self,
        conv: &GroupId,
        before_ms: Option<Ms>,
        limit: usize,
    ) -> Vec<proto::StoredMessage>;
    /// Newest first; `conv` None = every conversation.
    fn search(
        &self,
        conv: Option<&GroupId>,
        query: &str,
        limit: usize,
    ) -> Vec<(GroupId, proto::StoredMessage)>;
    fn wipe_conv(&mut self, conv: &GroupId);
    fn add_pending(&mut self, target: &MsgId, ev: &proto::PendingEvent);
    fn take_pending(&mut self, target: &MsgId) -> Vec<proto::PendingEvent>;
    /// Drops pending events received before `before_ms`; returns (target, event id).
    fn expire_pending(&mut self, before_ms: Ms) -> Vec<(MsgId, MsgId)>;
    fn set_draft(&mut self, conv: &GroupId, text: &str);
    fn draft(&self, conv: &GroupId) -> Option<String>;
    /// True when writes must be mirrored to the host as `chat/…` rows.
    fn mirror_rows(&self) -> bool {
        false
    }
}

// ───────────────────────── view model ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteKind {
    Text,
    Voice,
    Location,
    Sos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyState {
    /// The original is on this device (preview reflects its latest edit).
    Present,
    /// The original was deleted for everyone: no preview.
    Deleted,
    /// The original never reached this device: preview is the embedded quote.
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyView {
    pub msg_uuid: MsgId,
    pub sender: NodeId,
    pub preview: String,
    pub kind: QuoteKind,
    pub state: ReplyState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionView {
    pub emoji: String,
    pub count: u32,
    pub mine: bool,
}

/// One timeline row as every UI renders it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatItem {
    pub group: GroupId,
    pub msg_uuid: MsgId,
    pub from: NodeId,
    pub sent_ms: Ms,
    /// Display order key (`sent_ms`, clamped to receive time + 10 min).
    pub sort_ms: Ms,
    /// None when deleted for everyone.
    pub body: Option<MessageBody>,
    pub deleted: bool,
    /// Number of edits (0 = never edited).
    pub edited: u32,
    pub reply: Option<ReplyView>,
    pub mentions: Vec<NodeId>,
    pub mention_all: bool,
    /// This message mentions the local user (or @all from the creator).
    pub mentions_me: bool,
    pub forwarded: u32,
    pub reactions: Vec<ReactionView>,
    pub acked: u32,
}

/// Options for composing a message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Compose {
    pub reply_to: Option<MsgId>,
    pub mentions: Vec<NodeId>,
    pub mention_all: bool,
}

// ───────────────────────── helpers ─────────────────────────

pub(crate) fn id16(b: &[u8]) -> Option<[u8; 16]> {
    b.try_into().ok()
}
pub(crate) fn id8(b: &[u8]) -> Option<[u8; 8]> {
    b.try_into().ok()
}

pub fn sort_ms(row: &proto::StoredMessage) -> Ms {
    let sent = row.msg.as_ref().map(|m| m.sent_ms).unwrap_or(0);
    sent.min(row.received_ms.saturating_add(FUTURE_SKEW_MS))
}

/// Romanian cedilla letters → comma-below (ş→ș, ţ→ț), so search and quotes
/// agree whichever keyboard typed them.
pub fn ro_comma(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ş' => 'ș',
            'Ş' => 'Ș',
            'ţ' => 'ț',
            'Ţ' => 'Ț',
            c => c,
        })
        .collect()
}

/// Lowercase + strip diacritics, the same folding SQLite's
/// `unicode61 remove_diacritics 2` applies (Latin ranges used in Europe).
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        if ('\u{300}'..='\u{36f}').contains(&c) {
            continue; // combining marks (NFD input)
        }
        out.push(match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
            'ď' | 'đ' => 'd',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
            'ĥ' | 'ħ' => 'h',
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
            'ĵ' => 'j',
            'ķ' => 'k',
            'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
            'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
            'ŕ' | 'ŗ' | 'ř' => 'r',
            'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => 's',
            'ţ' | 'ť' | 'ŧ' | 'ț' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
            'ŵ' => 'w',
            'ý' | 'ÿ' | 'ŷ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            c => c,
        });
    }
    out
}

/// Search tokens: alphanumeric runs, folded.
pub fn tokens(s: &str) -> Vec<String> {
    fold(&ro_comma(s))
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every query token is a prefix of some text token (AND, prefix search).
pub fn matches(text_tokens: &[String], query_tokens: &[String]) -> bool {
    !query_tokens.is_empty()
        && query_tokens
            .iter()
            .all(|q| text_tokens.iter().any(|t| t.starts_with(q.as_str())))
}

fn preview_of(text: &str) -> String {
    let t: String = text.chars().take(PREVIEW_CHARS).collect();
    ro_comma(&t)
}

pub fn body_from_proto(b: &Body) -> Option<MessageBody> {
    Some(match b {
        Body::Text(t) => MessageBody::Text(t.clone()),
        Body::Voice(v) => MessageBody::VoiceNote {
            profile: Profile::from_u8(v.profile as u8).unwrap_or(Profile::Std),
            duration_ms: v.duration_ms,
            opus_packets: v.opus_packets.to_vec(),
        },
        Body::Location(l) => MessageBody::Location {
            lat_e7: (l.lat * 1e7) as i64,
            lon_e7: (l.lon * 1e7) as i64,
            accuracy_m: l.accuracy_m as u32,
            breadcrumb: l.breadcrumb,
        },
        Body::Sos(s) => {
            let l = s.location.unwrap_or_default();
            MessageBody::Sos {
                lat_e7: (l.lat * 1e7) as i64,
                lon_e7: (l.lon * 1e7) as i64,
                note: s.note.clone(),
                cancelled: s.cancelled,
            }
        }
        Body::Reaction(_) | Body::Edit(_) | Body::Delete(_) => return None,
    })
}

fn is_event(b: &Body) -> bool {
    matches!(b, Body::Reaction(_) | Body::Edit(_) | Body::Delete(_))
}

fn event_target(b: &Body) -> Option<MsgId> {
    match b {
        Body::Reaction(r) => id16(&r.target),
        Body::Edit(e) => id16(&e.target),
        Body::Delete(d) => id16(&d.target),
        _ => None,
    }
}

/// Current text of a text message (latest edit wins by `(sent_ms, event id)`).
pub fn current_text(row: &proto::StoredMessage) -> Option<String> {
    let m = row.msg.as_ref()?;
    let Some(Body::Text(orig)) = &m.body else {
        return None;
    };
    Some(
        row.edits
            .iter()
            .max_by(|a, b| (a.sent_ms, &a.event_id).cmp(&(b.sent_ms, &b.event_id)))
            .map(|r| r.text.clone())
            .unwrap_or_else(|| orig.clone()),
    )
}

fn search_text(row: &proto::StoredMessage) -> Option<String> {
    if row.deleted {
        return None;
    }
    match row.msg.as_ref()?.body.as_ref()? {
        Body::Text(_) => current_text(row).map(|t| ro_comma(&t)),
        Body::Sos(s) if !s.note.is_empty() => Some(ro_comma(&s.note)),
        _ => None,
    }
}

fn tombstone(row: &mut proto::StoredMessage, now: Ms) {
    row.deleted = true;
    row.deleted_ms = now;
    row.edits.clear();
    row.reactions.clear();
    if let Some(m) = row.msg.as_mut() {
        m.body = None;
        m.reply_to = None;
        m.mentions.clear();
        m.mention_all = false;
        m.forwarded = 0;
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Build the quote embedded in a reply.
pub fn reply_ref(row: &proto::StoredMessage) -> Option<proto::ReplyRef> {
    let m = row.msg.as_ref()?;
    if row.deleted {
        return None;
    }
    let (kind, preview) = match m.body.as_ref()? {
        Body::Text(_) => (
            proto::ReplyKind::Text,
            preview_of(&current_text(row).unwrap_or_default()),
        ),
        Body::Voice(_) => (proto::ReplyKind::Voice, String::new()),
        Body::Location(_) => (proto::ReplyKind::Location, String::new()),
        Body::Sos(s) => (proto::ReplyKind::Sos, preview_of(&s.note)),
        _ => return None,
    };
    Some(proto::ReplyRef {
        msg_uuid: m.msg_uuid.clone(),
        sender_id: m.sender_id.clone(),
        preview,
        kind: kind as i32,
    })
}

// ───────────────────────── the store ─────────────────────────

/// Applies the v2 rules on top of a backend and records host mirror rows.
pub struct Chat {
    pub backend: Box<dyn ChatBackend>,
    /// `chat/…` rows to hand to the host (empty value = delete; key ending in
    /// `/` = delete every key with that prefix).
    pub mirror_out: Vec<(String, Vec<u8>)>,
}

impl Default for Chat {
    fn default() -> Self {
        Chat::new(Box::new(MemBackend::default()))
    }
}

impl Chat {
    pub fn new(backend: Box<dyn ChatBackend>) -> Self {
        Chat {
            backend,
            mirror_out: vec![],
        }
    }

    fn put(&mut self, conv: &GroupId, row: &proto::StoredMessage) {
        let text = search_text(row);
        self.backend.put(conv, row, text.as_deref());
        if self.backend.mirror_rows() {
            if let Some(m) = row.msg.as_ref() {
                use prost::Message;
                self.mirror_out.push((
                    format!("chat/m/{}/{}", hex(conv), hex(&m.msg_uuid)),
                    row.encode_to_vec(),
                ));
            }
        }
    }

    pub fn get(&self, id: &MsgId) -> Option<proto::StoredMessage> {
        self.backend.get(id)
    }

    pub fn wipe_conv(&mut self, conv: &GroupId) {
        self.backend.wipe_conv(conv);
        if self.backend.mirror_rows() {
            self.mirror_out
                .push((format!("chat/m/{}/", hex(conv)), vec![]));
            self.mirror_out
                .push((format!("chat/d/{}", hex(conv)), vec![]));
        }
    }

    pub fn set_draft(&mut self, conv: &GroupId, text: &str) {
        self.backend.set_draft(conv, text);
        if self.backend.mirror_rows() {
            self.mirror_out
                .push((format!("chat/d/{}", hex(conv)), text.as_bytes().to_vec()));
        }
    }

    pub fn draft(&self, conv: &GroupId) -> Option<String> {
        self.backend.draft(conv)
    }

    pub fn expire_pending(&mut self, now: Ms) {
        let gone = self
            .backend
            .expire_pending(now.saturating_sub(PENDING_TTL_MS));
        if self.backend.mirror_rows() {
            for (t, id) in gone {
                self.mirror_out
                    .push((format!("chat/p/{}/{}", hex(&t), hex(&id)), vec![]));
            }
        }
    }

    /// Ingest a decoded message (own or received). `signed_by` = node id whose
    /// signature verified, None if unsigned (legacy content is accepted,
    /// events are not). Returns changed rows: (id, fresh) — fresh = a new
    /// content message (notify), else an update to an existing one.
    pub fn ingest(
        &mut self,
        conv: GroupId,
        mut cm: proto::ChatMessage,
        signed_by: Option<NodeId>,
        now: Ms,
    ) -> Vec<(MsgId, bool)> {
        let (Some(id), Some(sender)) = (id16(&cm.msg_uuid), id8(&cm.sender_id)) else {
            return vec![];
        };
        cm.signature = Default::default();
        let Some(body) = cm.body.clone() else {
            return vec![];
        };
        if is_event(&body) {
            if signed_by != Some(sender) {
                return vec![];
            }
            let Some(target) = event_target(&body) else {
                return vec![];
            };
            return match self.backend.get(&target) {
                Some(row) if row.placeholder => vec![],
                Some(row) => {
                    if self.apply_event(&conv, row, &cm, now) {
                        vec![(target, false)]
                    } else {
                        vec![]
                    }
                }
                None if matches!(body, Body::Delete(_)) => {
                    let row = proto::StoredMessage {
                        msg: Some(proto::ChatMessage {
                            msg_uuid: target.to_vec().into(),
                            group_uuid: conv.to_vec().into(),
                            sender_id: sender.to_vec().into(),
                            sent_ms: cm.sent_ms,
                            ..Default::default()
                        }),
                        received_ms: now,
                        deleted: true,
                        deleted_ms: now,
                        placeholder: true,
                        ..Default::default()
                    };
                    self.put(&conv, &row);
                    vec![]
                }
                None => {
                    let ev = proto::PendingEvent {
                        msg: Some(cm),
                        received_ms: now,
                    };
                    self.backend.add_pending(&target, &ev);
                    if self.backend.mirror_rows() {
                        use prost::Message;
                        self.mirror_out.push((
                            format!("chat/p/{}/{}", hex(&target), hex(&id)),
                            ev.encode_to_vec(),
                        ));
                    }
                    vec![]
                }
            };
        }
        // content message
        if let Some(existing) = self.backend.get(&id) {
            if !existing.placeholder {
                return vec![];
            }
            let deleter = existing.msg.as_ref().map(|m| m.sender_id.clone());
            if deleter.as_deref() == Some(&cm.sender_id[..]) {
                let mut row = existing;
                row.placeholder = false;
                row.received_ms = now;
                cm.body = None;
                cm.reply_to = None;
                cm.mentions.clear();
                row.msg = Some(cm);
                self.put(&conv, &row);
                return vec![(id, false)];
            }
            // a forged pre-delete: ignore it and keep the real message
        }
        let row = proto::StoredMessage {
            msg: Some(cm),
            received_ms: now,
            ..Default::default()
        };
        self.put(&conv, &row);
        let mut pend = self.backend.take_pending(&id);
        if !pend.is_empty() && self.backend.mirror_rows() {
            self.mirror_out
                .push((format!("chat/p/{}/", hex(&id)), vec![]));
        }
        pend.sort_by_key(|p| p.msg.as_ref().map(|m| m.sent_ms).unwrap_or(0));
        for p in pend {
            let (Some(ev), Some(row)) = (p.msg, self.backend.get(&id)) else {
                continue;
            };
            self.apply_event(&conv, row, &ev, now);
        }
        vec![(id, true)]
    }

    fn apply_event(
        &mut self,
        conv: &GroupId,
        mut row: proto::StoredMessage,
        ev: &proto::ChatMessage,
        now: Ms,
    ) -> bool {
        let Some(orig) = row.msg.clone() else {
            return false;
        };
        let author_ok = orig.sender_id == ev.sender_id;
        match ev.body.as_ref() {
            Some(Body::Delete(_)) => {
                if !author_ok || row.deleted {
                    return false;
                }
                tombstone(&mut row, now);
            }
            Some(Body::Edit(e)) => {
                if row.deleted
                    || !author_ok
                    || !matches!(orig.body, Some(Body::Text(_)))
                    || ev.sent_ms > orig.sent_ms + EDIT_WINDOW_MS + CLOCK_SKEW_MS
                    || ev.sent_ms + CLOCK_SKEW_MS < orig.sent_ms
                    || row.edits.len() >= MAX_EDITS
                    || row.edits.iter().any(|r| r.event_id == ev.msg_uuid)
                {
                    return false;
                }
                row.edits.push(proto::Revision {
                    text: e.text.clone(),
                    sent_ms: ev.sent_ms,
                    event_id: ev.msg_uuid.clone(),
                });
            }
            Some(Body::Reaction(r)) => {
                if row.deleted || r.emoji.len() > MAX_EMOJI_BYTES {
                    return false;
                }
                let key = (ev.sent_ms, ev.msg_uuid.clone());
                match row.reactions.iter_mut().find(|x| x.node == ev.sender_id) {
                    Some(x) => {
                        if key <= (x.sent_ms, x.event_id.clone()) {
                            return false;
                        }
                        x.emoji = r.emoji.clone();
                        x.sent_ms = ev.sent_ms;
                        x.event_id = ev.msg_uuid.clone();
                    }
                    None => row.reactions.push(proto::StoredReaction {
                        node: ev.sender_id.clone(),
                        emoji: r.emoji.clone(),
                        sent_ms: ev.sent_ms,
                        event_id: ev.msg_uuid.clone(),
                    }),
                }
            }
            _ => return false,
        }
        self.put(conv, &row);
        true
    }

    pub fn ack(&mut self, id: &MsgId, by: NodeId) -> bool {
        let Some(mut row) = self.backend.get(id) else {
            return false;
        };
        if row.acked_by.iter().any(|b| b[..] == by[..]) {
            return false;
        }
        row.acked_by.push(by.to_vec().into());
        let Some(conv) = row.msg.as_ref().and_then(|m| id16(&m.group_uuid)) else {
            return false;
        };
        self.put(&conv, &row);
        true
    }

    /// Edit history, oldest first: (text, sent_ms). Empty for non-text/deleted.
    pub fn edit_history(&self, id: &MsgId) -> Vec<(String, Ms)> {
        let Some(row) = self.backend.get(id) else {
            return vec![];
        };
        let Some(m) = row.msg.as_ref() else {
            return vec![];
        };
        let Some(Body::Text(orig)) = &m.body else {
            return vec![];
        };
        let mut revs: Vec<&proto::Revision> = row.edits.iter().collect();
        revs.sort_by(|a, b| (a.sent_ms, &a.event_id).cmp(&(b.sent_ms, &b.event_id)));
        std::iter::once((orig.clone(), m.sent_ms))
            .chain(revs.into_iter().map(|r| (r.text.clone(), r.sent_ms)))
            .collect()
    }

    /// Render a stored row. `creator` = the group's creator (for @all).
    pub fn view(
        &self,
        row: &proto::StoredMessage,
        me: NodeId,
        creator: Option<NodeId>,
    ) -> Option<ChatItem> {
        if row.placeholder {
            return None;
        }
        let m = row.msg.as_ref()?;
        let group = id16(&m.group_uuid)?;
        let from = id8(&m.sender_id)?;
        let body = if row.deleted {
            None
        } else {
            let b = body_from_proto(m.body.as_ref()?)?;
            Some(match b {
                MessageBody::Text(_) => MessageBody::Text(current_text(row).unwrap_or_default()),
                b => b,
            })
        };
        let reply = m.reply_to.as_ref().and_then(|r| {
            let rid = id16(&r.msg_uuid)?;
            let kind = match proto::ReplyKind::try_from(r.kind).unwrap_or(proto::ReplyKind::Text) {
                proto::ReplyKind::Voice => QuoteKind::Voice,
                proto::ReplyKind::Location => QuoteKind::Location,
                proto::ReplyKind::Sos => QuoteKind::Sos,
                _ => QuoteKind::Text,
            };
            let (state, preview) = match self.backend.get(&rid) {
                Some(t) if t.deleted => (ReplyState::Deleted, String::new()),
                Some(t) => (
                    ReplyState::Present,
                    reply_ref(&t)
                        .map(|q| q.preview)
                        .unwrap_or_else(|| r.preview.clone()),
                ),
                None => (ReplyState::Missing, preview_of(&r.preview)),
            };
            Some(ReplyView {
                msg_uuid: rid,
                sender: id8(&r.sender_id).unwrap_or_default(),
                preview,
                kind,
                state,
            })
        });
        let mentions: Vec<NodeId> = m.mentions.iter().filter_map(|b| id8(b)).collect();
        let mention_all = m.mention_all && creator == Some(from);
        let mentions_me = from != me && (mentions.contains(&me) || mention_all);
        let mut counts: HashMap<&str, (u32, bool)> = HashMap::new();
        let mut order: Vec<&str> = vec![];
        let mut reacts: Vec<&proto::StoredReaction> = row
            .reactions
            .iter()
            .filter(|r| !r.emoji.is_empty())
            .collect();
        reacts.sort_by(|a, b| (a.sent_ms, &a.event_id).cmp(&(b.sent_ms, &b.event_id)));
        for r in reacts {
            let e = counts.entry(r.emoji.as_str()).or_insert_with(|| {
                order.push(r.emoji.as_str());
                (0, false)
            });
            e.0 += 1;
            e.1 |= r.node[..] == me[..];
        }
        let mut reactions: Vec<ReactionView> = order
            .into_iter()
            .map(|e| ReactionView {
                emoji: e.to_string(),
                count: counts[e].0,
                mine: counts[e].1,
            })
            .collect();
        reactions.sort_by_key(|r| std::cmp::Reverse(r.count));
        reactions.truncate(MAX_REACTIONS_SHOWN);
        Some(ChatItem {
            group,
            msg_uuid: id16(&m.msg_uuid)?,
            from,
            sent_ms: m.sent_ms,
            sort_ms: sort_ms(row),
            body,
            deleted: row.deleted,
            edited: row.edits.len() as u32,
            reply,
            mentions,
            mention_all,
            mentions_me,
            forwarded: m.forwarded,
            reactions,
            acked: row.acked_by.len() as u32,
        })
    }

    /// Replay one host row (`chat/m|p|d/…`) into a mirroring backend at start.
    pub fn restore_row(&mut self, key: &str, value: &[u8]) -> bool {
        use prost::Message;
        let parts: Vec<&str> = key.split('/').collect();
        let unhex = |s: &str| -> Option<[u8; 16]> {
            if s.len() != 32 {
                return None;
            }
            let mut o = [0u8; 16];
            for (i, b) in o.iter_mut().enumerate() {
                *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
            }
            Some(o)
        };
        match parts.as_slice() {
            ["chat", "m", conv, _] => {
                let (Some(conv), Ok(row)) = (unhex(conv), proto::StoredMessage::decode(value))
                else {
                    return false;
                };
                let text = search_text(&row);
                self.backend.put(&conv, &row, text.as_deref());
                true
            }
            ["chat", "p", target, _] => {
                let (Some(t), Ok(ev)) = (unhex(target), proto::PendingEvent::decode(value)) else {
                    return false;
                };
                self.backend.add_pending(&t, &ev);
                true
            }
            ["chat", "d", conv] => {
                let Some(conv) = unhex(conv) else {
                    return false;
                };
                self.backend
                    .set_draft(&conv, &String::from_utf8_lossy(value));
                true
            }
            _ => false,
        }
    }
}

// ───────────────────────── in-memory backend ─────────────────────────

struct MemRow {
    conv: GroupId,
    row: proto::StoredMessage,
    tokens: Vec<String>,
}

/// Backend for web / Tizen (mirrored to IndexedDB by the host) and tests.
pub struct MemBackend {
    rows: HashMap<MsgId, MemRow>,
    pending: HashMap<MsgId, Vec<(MsgId, proto::PendingEvent)>>,
    drafts: HashMap<GroupId, String>,
    mirror: bool,
}

impl Default for MemBackend {
    fn default() -> Self {
        MemBackend::new(false)
    }
}

impl MemBackend {
    pub fn new(mirror: bool) -> Self {
        MemBackend {
            rows: HashMap::new(),
            pending: HashMap::new(),
            drafts: HashMap::new(),
            mirror,
        }
    }
}

impl ChatBackend for MemBackend {
    fn get(&self, id: &MsgId) -> Option<proto::StoredMessage> {
        self.rows.get(id).map(|r| r.row.clone())
    }
    fn put(&mut self, conv: &GroupId, row: &proto::StoredMessage, search: Option<&str>) {
        let Some(id) = row.msg.as_ref().and_then(|m| id16(&m.msg_uuid)) else {
            return;
        };
        self.rows.insert(
            id,
            MemRow {
                conv: *conv,
                row: row.clone(),
                tokens: search.map(tokens).unwrap_or_default(),
            },
        );
    }
    fn history(
        &self,
        conv: &GroupId,
        before_ms: Option<Ms>,
        limit: usize,
    ) -> Vec<proto::StoredMessage> {
        let mut v: Vec<&MemRow> = self
            .rows
            .values()
            .filter(|r| &r.conv == conv && !r.row.placeholder)
            .filter(|r| before_ms.is_none_or(|b| sort_ms(&r.row) < b))
            .collect();
        v.sort_by_key(|r| {
            (
                sort_ms(&r.row),
                r.row.msg.as_ref().map(|m| m.msg_uuid.clone()),
            )
        });
        let skip = v.len().saturating_sub(limit);
        v.into_iter().skip(skip).map(|r| r.row.clone()).collect()
    }
    fn search(
        &self,
        conv: Option<&GroupId>,
        query: &str,
        limit: usize,
    ) -> Vec<(GroupId, proto::StoredMessage)> {
        let q = tokens(query);
        let mut v: Vec<&MemRow> = self
            .rows
            .values()
            .filter(|r| conv.is_none_or(|c| &r.conv == c) && matches(&r.tokens, &q))
            .collect();
        v.sort_by_key(|r| std::cmp::Reverse(sort_ms(&r.row)));
        v.into_iter()
            .take(limit)
            .map(|r| (r.conv, r.row.clone()))
            .collect()
    }
    fn wipe_conv(&mut self, conv: &GroupId) {
        self.rows.retain(|_, r| &r.conv != conv);
        self.drafts.remove(conv);
        for list in self.pending.values_mut() {
            list.retain(|(_, e)| e.msg.as_ref().map(|m| &m.group_uuid[..]) != Some(&conv[..]));
        }
        self.pending.retain(|_, l| !l.is_empty());
    }
    fn add_pending(&mut self, target: &MsgId, ev: &proto::PendingEvent) {
        let Some(id) = ev.msg.as_ref().and_then(|m| id16(&m.msg_uuid)) else {
            return;
        };
        let l = self.pending.entry(*target).or_default();
        if !l.iter().any(|(i, _)| *i == id) {
            l.push((id, ev.clone()));
        }
    }
    fn take_pending(&mut self, target: &MsgId) -> Vec<proto::PendingEvent> {
        self.pending
            .remove(target)
            .map(|l| l.into_iter().map(|(_, e)| e).collect())
            .unwrap_or_default()
    }
    fn expire_pending(&mut self, before_ms: Ms) -> Vec<(MsgId, MsgId)> {
        let mut gone = vec![];
        for (t, l) in self.pending.iter_mut() {
            l.retain(|(id, e)| {
                let keep = e.received_ms >= before_ms;
                if !keep {
                    gone.push((*t, *id));
                }
                keep
            });
        }
        self.pending.retain(|_, l| !l.is_empty());
        gone
    }
    fn set_draft(&mut self, conv: &GroupId, text: &str) {
        if text.is_empty() {
            self.drafts.remove(conv);
        } else {
            self.drafts.insert(*conv, text.to_string());
        }
    }
    fn draft(&self, conv: &GroupId) -> Option<String> {
        self.drafts.get(conv).cloned()
    }
    fn mirror_rows(&self) -> bool {
        self.mirror
    }
}

#[cfg(test)]
mod tests;
