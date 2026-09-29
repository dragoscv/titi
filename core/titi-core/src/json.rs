//! JSON view models shared by every host that renders a web UI (wasm PWA,
//! Tauri desktop, Tizen). One definition, so the TypeScript `UiEvent` /
//! `GroupJson` types in `packages/app-ui` match every platform byte-for-byte.

use serde::Serialize;

use crate::engine::{Engine, MessageBody, UiEvent};
use crate::frame::Profile;
use crate::link::LinkClass;

pub fn hex(b: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push(H[(x >> 4) as usize] as char);
        s.push(H[(x & 15) as usize] as char);
    }
    s
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

pub fn link_str(l: LinkClass) -> &'static str {
    match l {
        LinkClass::Lan => "lan",
        LinkClass::WifiAware => "wifiAware",
        LinkClass::Nearby => "nearby",
        LinkClass::Hotspot => "hotspot",
        LinkClass::BleL2cap => "bleL2cap",
        LinkClass::BleGatt => "bleGatt",
        LinkClass::BtRfcomm => "btRfcomm",
        LinkClass::Internet => "internet",
        LinkClass::WebRtc => "webRtc",
    }
}

pub fn profile_str(p: Profile) -> &'static str {
    match p {
        Profile::Hq => "hq",
        Profile::Std => "std",
        Profile::Low => "low",
        Profile::Min => "min",
    }
}

pub fn profile_from_str(s: &str) -> Profile {
    match s {
        "hq" => Profile::Hq,
        "low" => Profile::Low,
        "min" => Profile::Min,
        _ => Profile::Std,
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum JsBody {
    Text {
        text: String,
    },
    VoiceNote {
        profile: &'static str,
        duration_ms: u32,
        opus_packets: Vec<u8>,
    },
    Location {
        lat_e7: i64,
        lon_e7: i64,
        accuracy_m: u32,
        breadcrumb: bool,
    },
    Sos {
        lat_e7: i64,
        lon_e7: i64,
        note: String,
        cancelled: bool,
    },
}

impl From<MessageBody> for JsBody {
    fn from(b: MessageBody) -> Self {
        match b {
            MessageBody::Text(t) => JsBody::Text { text: t },
            MessageBody::VoiceNote {
                profile,
                duration_ms,
                opus_packets,
            } => JsBody::VoiceNote {
                profile: profile_str(profile),
                duration_ms,
                opus_packets,
            },
            MessageBody::Location {
                lat_e7,
                lon_e7,
                accuracy_m,
                breadcrumb,
            } => JsBody::Location {
                lat_e7,
                lon_e7,
                accuracy_m,
                breadcrumb,
            },
            MessageBody::Sos {
                lat_e7,
                lon_e7,
                note,
                cancelled,
            } => JsBody::Sos {
                lat_e7,
                lon_e7,
                note,
                cancelled,
            },
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JsUi {
    PeerDiscovered {
        node: String,
        name: String,
        hue: u16,
        link: &'static str,
        in_group: bool,
    },
    PeerLost {
        node: String,
    },
    PeerLink {
        node: String,
        link: &'static str,
        bars: u8,
        hops: u8,
    },
    FloorGranted,
    FloorDenied {
        holder: String,
    },
    FloorTaken {
        group: String,
        holder: String,
        name: String,
        prio: u8,
    },
    FloorIdle {
        group: String,
    },
    TalkWarning,
    TalkTimeout,
    InviteOffered {
        group: String,
        name: String,
        host: String,
        host_name: String,
        members: u32,
    },
    Joined {
        group: String,
        name: String,
    },
    JoinFailed {
        reason: String,
    },
    MemberJoined {
        group: String,
        node: String,
        name: String,
    },
    MemberLeft {
        group: String,
        node: String,
    },
    Message {
        group: String,
        from: String,
        msg_uuid: String,
        sent_ms: f64,
        body: JsBody,
    },
    MessageAcked {
        msg_uuid: String,
        by: String,
    },
    Handover {
        group: String,
        state: String,
        link: Option<&'static str>,
        profile: &'static str,
    },
    Suspended {
        group: String,
    },
    Resumed {
        group: String,
    },
    ModeChanged {
        group: String,
        full_duplex: bool,
    },
    Level {
        talker: Option<String>,
        dbfs: f32,
    },
    Error {
        message: String,
    },
}

impl From<UiEvent> for JsUi {
    fn from(e: UiEvent) -> Self {
        use JsUi as J;
        match e {
            UiEvent::PeerDiscovered {
                node,
                name,
                hue,
                link,
                in_group,
                ..
            } => J::PeerDiscovered {
                node: hex(&node),
                name,
                hue,
                link: link_str(link),
                in_group,
            },
            UiEvent::PeerLost { node } => J::PeerLost { node: hex(&node) },
            UiEvent::PeerLink {
                node,
                link,
                bars,
                hops,
            } => J::PeerLink {
                node: hex(&node),
                link: link_str(link),
                bars,
                hops,
            },
            UiEvent::FloorGranted => J::FloorGranted,
            UiEvent::FloorDenied { holder } => J::FloorDenied {
                holder: hex(&holder),
            },
            UiEvent::FloorTaken {
                group,
                holder,
                name,
                prio,
            } => J::FloorTaken {
                group: hex(&group),
                holder: hex(&holder),
                name,
                prio,
            },
            UiEvent::FloorIdle { group } => J::FloorIdle { group: hex(&group) },
            UiEvent::TalkWarning => J::TalkWarning,
            UiEvent::TalkTimeout => J::TalkTimeout,
            UiEvent::InviteOffered {
                group,
                name,
                host,
                host_name,
                members,
            } => J::InviteOffered {
                group: hex(&group),
                name,
                host: hex(&host),
                host_name,
                members,
            },
            UiEvent::Joined { group, name } => J::Joined {
                group: hex(&group),
                name,
            },
            UiEvent::JoinFailed { reason } => J::JoinFailed { reason },
            UiEvent::MemberJoined { group, node, name } => J::MemberJoined {
                group: hex(&group),
                node: hex(&node),
                name,
            },
            UiEvent::MemberLeft { group, node } => J::MemberLeft {
                group: hex(&group),
                node: hex(&node),
            },
            UiEvent::Message {
                group,
                from,
                msg_uuid,
                sent_ms,
                body,
            } => J::Message {
                group: hex(&group),
                from: hex(&from),
                msg_uuid: hex(&msg_uuid),
                sent_ms: sent_ms as f64,
                body: body.into(),
            },
            UiEvent::MessageAcked { msg_uuid, by } => J::MessageAcked {
                msg_uuid: hex(&msg_uuid),
                by: hex(&by),
            },
            UiEvent::Handover {
                group,
                state,
                link,
                profile,
            } => J::Handover {
                group: hex(&group),
                state,
                link: link.map(link_str),
                profile: profile_str(profile),
            },
            UiEvent::Suspended { group } => J::Suspended { group: hex(&group) },
            UiEvent::Resumed { group } => J::Resumed { group: hex(&group) },
            UiEvent::ModeChanged { group, full_duplex } => J::ModeChanged {
                group: hex(&group),
                full_duplex,
            },
            UiEvent::Level { talker, dbfs } => J::Level {
                talker: talker.map(|t| hex(&t)),
                dbfs,
            },
            UiEvent::Error { message } => J::Error { message },
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsGroup {
    pub id: String,
    pub name: String,
    pub full_duplex: bool,
    pub member_count: u32,
    pub is_active: bool,
    pub is_creator: bool,
    pub members: Vec<JsMember>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsMember {
    pub node: String,
    pub name: String,
    pub hue: u16,
}

/// Group table in UI shape, sorted by name so every host renders a stable order.
pub fn groups(e: &Engine) -> Vec<JsGroup> {
    let me = e.node_id();
    let mut out: Vec<JsGroup> = e
        .groups
        .values()
        .map(|g| {
            let mut members: Vec<JsMember> = g
                .members
                .values()
                .map(|m| JsMember {
                    node: hex(&m.node),
                    name: m.name.clone(),
                    hue: m.hue,
                })
                .collect();
            members.sort_by(|a, b| a.name.cmp(&b.name));
            JsGroup {
                id: hex(&g.id),
                name: g.name.clone(),
                full_duplex: g.full_duplex,
                member_count: g.members.len() as u32,
                is_active: e.active_group == Some(g.id),
                is_creator: g.creator == me,
                members,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let b = [0u8, 1, 0xab, 0xff];
        assert_eq!(hex(&b), "0001abff");
        assert_eq!(unhex("0001abff").unwrap(), b);
        assert!(unhex("abc").is_none());
        assert!(unhex("zz").is_none());
    }

    #[test]
    fn ui_event_shape_is_camel_tagged() {
        let j = serde_json::to_string(&JsUi::from(UiEvent::FloorIdle { group: [1; 16] })).unwrap();
        assert_eq!(
            j,
            r#"{"type":"floorIdle","group":"01010101010101010101010101010101"}"#
        );
        let j = serde_json::to_string(&JsUi::PeerDiscovered {
            node: "aa".into(),
            name: "A".into(),
            hue: 3,
            link: "lan",
            in_group: true,
        })
        .unwrap();
        assert!(j.contains(r#""type":"peerDiscovered""#) && j.contains(r#""in_group":true"#));
    }
}
