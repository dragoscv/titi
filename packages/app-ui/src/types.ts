// Mirrors core/titi-wasm JSON shapes.

export type LinkClass = "lan" | "wifiAware" | "nearby" | "hotspot" | "bleL2cap" | "bleGatt" | "btRfcomm" | "internet" | "webRtc";
export type Profile = "hq" | "std" | "low" | "min";

export type MsgBody =
  | { kind: "text"; text: string }
  | { kind: "voiceNote"; profile: Profile; duration_ms: number; opus_packets: number[] }
  | { kind: "location"; lat_e7: number; lon_e7: number; accuracy_m: number; breadcrumb: boolean }
  | { kind: "sos"; lat_e7: number; lon_e7: number; note: string; cancelled: boolean };

export interface ChatReply {
  msg_uuid: string;
  sender: string;
  preview: string;
  kind: "text" | "voice" | "location" | "sos";
  state: "present" | "deleted" | "missing";
}
export interface ChatReaction { emoji: string; count: number; mine: boolean }
/** One timeline row (message model v2). `body === null` = deleted for everyone. */
export interface ChatItem {
  group: string;
  msg_uuid: string;
  from: string;
  sent_ms: number;
  sort_ms: number;
  body: MsgBody | null;
  deleted: boolean;
  /** number of edits */
  edited: number;
  reply: ChatReply | null;
  mentions: string[];
  mention_all: boolean;
  mentions_me: boolean;
  /** forward count (>= 5 = "forwarded many times") */
  forwarded: number;
  reactions: ChatReaction[];
  /** acks received (0 = not yet delivered) */
  acked: number;
}
export interface EditVersion { text: string; sent_ms: number }

export type UiEvent =
  | { type: "peerDiscovered"; node: string; name: string; hue: number; link: LinkClass; in_group: boolean }
  | { type: "peerLost"; node: string }
  | { type: "peerLink"; node: string; link: LinkClass; bars: number; hops: number }
  | { type: "floorGranted" }
  | { type: "floorDenied"; holder: string }
  | { type: "floorTaken"; group: string; holder: string; name: string; prio: number }
  | { type: "floorIdle"; group: string }
  | { type: "talkWarning" }
  | { type: "talkTimeout" }
  | { type: "inviteOffered"; group: string; name: string; host: string; host_name: string; members: number }
  | { type: "joined"; group: string; name: string }
  | { type: "joinFailed"; reason: string }
  | { type: "memberJoined"; group: string; node: string; name: string }
  | { type: "memberLeft"; group: string; node: string }
  | { type: "groupDissolved"; group: string; name: string }
  | { type: "message"; group: string; from: string; msg_uuid: string; sent_ms: number; body: MsgBody }
  | { type: "chatUpdated"; item: ChatItem }
  | { type: "messageAcked"; msg_uuid: string; by: string }
  | { type: "handover"; group: string; state: string; link: LinkClass | null; profile: Profile }
  | { type: "suspended"; group: string }
  | { type: "resumed"; group: string }
  | { type: "modeChanged"; group: string; full_duplex: boolean }
  | { type: "level"; talker: string | null; dbfs: number }
  | { type: "error"; message: string };

export type Action =
  | { t: "send"; link: number; peer: string | null; bytes: number[] }
  | { t: "playPacket"; talker: string; packet: number[]; frames: number }
  | { t: "capture"; active: boolean; profile: Profile }
  | { t: "ui"; event: UiEvent }
  | { t: "persist"; key: string; value: number[] }
  | { t: "wakeAt"; at_ms: number };

export interface GroupJson {
  id: string;
  name: string;
  fullDuplex: boolean;
  memberCount: number;
  isActive: boolean;
  isCreator: boolean;
  members: { node: string; name: string; hue: number }[];
}

export const LinkIds = { LAN: 1, BLE_GATT: 6, INTERNET: 8, WEBRTC: 9 } as const;
/** LinkClass numeric codes as in core/titi-core/src/link.rs `from_u8`. */
export const LinkClassCode = { internet: 8, webRtc: 9 } as const;
