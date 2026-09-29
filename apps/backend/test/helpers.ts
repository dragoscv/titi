import { create, fromBinary, toBinary, type MessageInitShape } from "@bufbuild/protobuf";
import { Flags, PROTOCOL_VERSION, SignalSchema, WsTag, type Signal } from "@titi/protocol";
import { tagged, type Sink } from "../src/relay.ts";

export class FakeSink implements Sink {
  out: Uint8Array[] = [];
  closed: { code: number; reason: string } | null = null;
  send(b: Uint8Array) {
    this.out.push(b);
  }
  close(code: number, reason: string) {
    this.closed = { code, reason };
  }
  signals(): Signal[] {
    return this.out.filter((b) => b[0] === WsTag.Signal).map((b) => fromBinary(SignalSchema, b.subarray(1)));
  }
  envelopes(): Uint8Array[] {
    return this.out.filter((b) => b[0] === WsTag.Envelope).map((b) => b.subarray(1));
  }
  errors(): { code: number; message: string }[] {
    return this.signals().flatMap((s) => (s.kind.case === "error" ? [{ code: s.kind.value.code, message: s.kind.value.message }] : []));
  }
  kinds(): (string | undefined)[] {
    return this.signals().map((s) => s.kind.case);
  }
  resumeToken(): Uint8Array {
    const j = this.signals().find((s) => s.kind.case === "roomJoined");
    if (!j || j.kind.case !== "roomJoined") throw new Error("no roomJoined");
    return j.kind.value.resumeToken;
  }
}

export const GH = new Uint8Array([1, 2, 3, 4]);
export const GH2 = new Uint8Array([5, 6, 7, 8]);
export const nodeA = new Uint8Array([0xa, 1, 2, 3, 4, 5, 6, 7]);
export const nodeB = new Uint8Array([0xb, 1, 2, 3, 4, 5, 6, 7]);
export const nodeC = new Uint8Array([0xc, 1, 2, 3, 4, 5, 6, 7]);

export function signal(kind: MessageInitShape<typeof SignalSchema>["kind"]): Uint8Array {
  return tagged(WsTag.Signal, toBinary(SignalSchema, create(SignalSchema, { kind })));
}

export function join(node: Uint8Array, opts: { resume?: Uint8Array; hash?: Uint8Array; rendezvous?: boolean } = {}): Uint8Array {
  return signal({
    case: "roomJoin",
    value: {
      groupHash: opts.hash ?? GH,
      node: { nodeId: node, displayName: "n" },
      resumeToken: opts.resume ?? new Uint8Array(),
      rendezvous: opts.rendezvous ?? false,
    },
  });
}

export function envelope(src: Uint8Array, opts: { dst?: Uint8Array; flags?: number; ftype?: number } = {}): Uint8Array {
  const { dst, flags = 0, ftype = 0x11 } = opts;
  const hdr = dst ? 24 : 16;
  const b = new Uint8Array(hdr + 3);
  b[0] = PROTOCOL_VERSION;
  b[1] = ftype;
  b[2] = 0x33;
  b[3] = flags | (dst ? Flags.UNICAST : 0);
  b.set(src, 8);
  if (dst) b.set(dst, 16);
  b.set([9, 9, 9], hdr);
  return tagged(WsTag.Envelope, b);
}
