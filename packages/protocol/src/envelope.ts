// Titi radio envelope header (ADR-0003). Mirrors core/titi-core/src/frame.rs.
// The relay only reads the header to route; payloads are opaque (group AEAD).

export const PROTOCOL_VERSION = 1;
export const ENVELOPE_MIN = 16;
export const ENVELOPE_UNICAST = 24;

export const FrameType = {
  Hello: 0x01,
  Announce: 0x02,
  RouteProbe: 0x03,
  Handshake: 0x04,
  Control: 0x05,
  GroupControl: 0x06,
  VoiceRouted: 0x10,
  VoiceFlood: 0x11,
  Message: 0x20,
  Ack: 0x21,
  Inventory: 0x22,
} as const;
export type FrameType = (typeof FrameType)[keyof typeof FrameType];

export const Flags = {
  UNICAST: 0b0000_0001,
  SIGNED: 0b0000_0010,
  RELAYED: 0b0000_0100,
  URGENT: 0b0000_1000,
} as const;

export interface EnvelopeHeader {
  ftype: number;
  ttl: number;
  hopStart: number;
  flags: number;
  msgId: number;
  src: Uint8Array; // 8 bytes
  dst: Uint8Array | null; // 8 bytes when UNICAST
  headerLen: number;
}

export function decodeHeader(buf: Uint8Array): EnvelopeHeader | null {
  if (buf.length < ENVELOPE_MIN || buf[0] !== PROTOCOL_VERSION) return null;
  const flags = buf[3]!;
  const unicast = (flags & Flags.UNICAST) !== 0;
  const headerLen = unicast ? ENVELOPE_UNICAST : ENVELOPE_MIN;
  if (buf.length < headerLen) return null;
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  return {
    ftype: buf[1]!,
    ttl: buf[2]! >> 4,
    hopStart: buf[2]! & 0x0f,
    flags,
    msgId: view.getUint32(4),
    src: buf.subarray(8, 16),
    dst: unicast ? buf.subarray(16, 24) : null,
    headerLen,
  };
}

export function nodeIdHex(id: Uint8Array): string {
  let s = "";
  for (const b of id) s += b.toString(16).padStart(2, "0");
  return s;
}

export function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/** Wire tags for the backend WebSocket (signal.proto header comment). */
export const WsTag = { Signal: 0x00, Envelope: 0x01 } as const;
