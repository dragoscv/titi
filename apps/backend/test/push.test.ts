import { generateKeyPairSync, verify } from "node:crypto";
import { createServer, type Http2Server, type IncomingHttpHeaders } from "node:http2";
import type { AddressInfo } from "node:net";
import { FrameType } from "@titi/protocol";
import { beforeEach, describe, expect, it } from "vitest";
import { createApns, providerToken, pttPayload, type PushSender } from "../src/apns.ts";
import { loadConfig } from "../src/config.ts";
import { setLogLevel } from "../src/log.ts";
import { Relay } from "../src/relay.ts";
import { envelope, FakeSink, GH, GH2, join, nodeA, nodeB, nodeC, signal } from "./helpers.ts";

beforeEach(() => setLogLevel("error"));

function setup() {
  let t = 1_000_000;
  const sent: { token: string; sandbox: boolean; name: string }[] = [];
  let status = 200;
  const push: PushSender = { send: async (token, sandbox, name) => { sent.push({ token, sandbox, name }); return status; } };
  const relay = new Relay(loadConfig({}), () => t, push);
  const peer = (node: Uint8Array, hash = GH) => {
    const sink = new FakeSink();
    const id = relay.connect(sink);
    relay.onMessage(id, join(node, { hash }));
    return { sink, id };
  };
  return { relay, peer, sent, tick: (ms: number) => (t += ms), setStatus: (s: number) => (status = s) };
}

const register = (token: number[], hashes: Uint8Array[] = [GH], sandbox = false) =>
  signal({ case: "pushRegister", value: { token: new Uint8Array(token), groupHashes: hashes, sandbox } });
const voice = (src: Uint8Array) => envelope(src, { ftype: FrameType.VoiceFlood });

describe("push to talk wake-ups", () => {
  it("wakes a registered iPhone that left the room when a member starts talking, once per turn", async () => {
    const { relay, peer, sent, tick } = setup();
    const iphone = peer(nodeA);
    relay.onMessage(iphone.id, register([0xab, 0xcd], [GH], true));
    relay.disconnect(iphone.id); // suspended
    const talker = peer(nodeB);
    relay.onMessage(talker.id, voice(nodeB));
    relay.onMessage(talker.id, voice(nodeB)); // same turn: debounced
    await Promise.resolve();
    expect(sent).toEqual([{ token: "abcd", sandbox: true, name: "n" }]);
    tick(11_000);
    relay.onMessage(talker.id, voice(nodeB));
    expect(sent).toHaveLength(2);
    expect(relay.stats().push).toBe(1);
  });

  it("does not push while the iPhone is connected, for non-voice frames, or for other rooms", () => {
    const { relay, peer, sent } = setup();
    const iphone = peer(nodeA);
    relay.onMessage(iphone.id, register([1], [GH]));
    const talker = peer(nodeB);
    relay.onMessage(talker.id, voice(nodeB)); // iPhone online: delivered by socket
    expect(sent).toHaveLength(0);
    relay.disconnect(iphone.id);
    relay.onMessage(talker.id, envelope(nodeB, { ftype: FrameType.Message }));
    expect(sent).toHaveLength(0);
    const other = peer(nodeB, GH2);
    relay.onMessage(other.id, voice(nodeB));
    expect(sent).toHaveLength(0);
  });

  it("forgets a token on unregister, on 410 from APNs, after 12 h, and rejects bad registrations", async () => {
    const { relay, peer, sent, tick, setStatus } = setup();
    const a = peer(nodeA);
    relay.onMessage(a.id, register([1]));
    relay.onMessage(a.id, register([]));
    expect(relay.stats().push).toBe(0);
    relay.onMessage(a.id, signal({ case: "pushRegister", value: { token: new Uint8Array([1]), groupHashes: [new Uint8Array([1, 2])] } }));
    expect(a.sink.errors().at(-1)?.message).toBe("bad push_register");
    relay.onMessage(a.id, register([2]));
    relay.disconnect(a.id);
    setStatus(410);
    const b = peer(nodeB);
    relay.onMessage(b.id, voice(nodeB));
    await new Promise((r) => setTimeout(r, 0));
    expect(sent).toHaveLength(1);
    expect(relay.stats().push).toBe(0);
    const c = peer(nodeA);
    relay.onMessage(c.id, register([3]));
    tick(12 * 60 * 60_000 + 1);
    relay.sweep();
    expect(relay.stats().push).toBe(0);
  });

  it("requires a room join before registering", () => {
    const { relay } = setup();
    const sink = new FakeSink();
    const id = relay.connect(sink);
    relay.onMessage(id, register([1]));
    expect(sink.errors()[0]?.message).toBe("join first");
  });

  it("skips the talker's own registration, expired ones, and rendezvous-only overlap; falls back to 'Titi' for a nameless talker", async () => {
    const { relay, peer, sent, tick } = setup();
    // talker registered itself: never woken by its own voice
    const self = peer(nodeA);
    relay.onMessage(self.id, register([1]));
    relay.onMessage(self.id, voice(nodeA));
    expect(sent).toHaveLength(0);
    // a suspended iPhone whose registration expired (not swept yet) is skipped
    relay.disconnect(self.id);
    tick(12 * 60 * 60_000 + 1);
    const b = peer(nodeB);
    relay.onMessage(b.id, voice(nodeB));
    expect(sent).toHaveLength(0);
    // a talker that only shares a rendezvous room does not wake anyone
    const c = peer(nodeC);
    relay.onMessage(c.id, register([2], [GH2]));
    relay.disconnect(c.id);
    const sink = new FakeSink();
    const rid = relay.connect(sink);
    relay.onMessage(rid, signal({ case: "roomJoin", value: { groupHash: GH2, node: { nodeId: nodeA, displayName: "" }, resumeToken: new Uint8Array(), rendezvous: true } }));
    relay.onMessage(rid, voice(nodeA));
    expect(sent).toHaveLength(0);
    // a real group room, talker without a display name -> "Titi"
    relay.disconnect(rid);
    const rid2 = relay.connect(new FakeSink());
    relay.onMessage(rid2, signal({ case: "roomJoin", value: { groupHash: GH2, node: { nodeId: nodeB, displayName: "" }, resumeToken: new Uint8Array(), rendezvous: false } }));
    relay.onMessage(rid2, voice(nodeB));
    await Promise.resolve();
    expect(sent.at(-1)?.name).toBe("Titi");
  });
});

describe("apns helpers", () => {
  it("signs an ES256 provider token APNs can verify and builds a name-only payload", () => {
    const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
    const tok = providerToken(privateKey, "KEYID12345", "TEAM123456", 1_700_000_000);
    const [h, p, s] = tok.split(".");
    expect(JSON.parse(Buffer.from(h!, "base64url").toString())).toEqual({ alg: "ES256", kid: "KEYID12345" });
    expect(JSON.parse(Buffer.from(p!, "base64url").toString())).toEqual({ iss: "TEAM123456", iat: 1_700_000_000 });
    expect(verify("sha256", Buffer.from(`${h}.${p}`), { key: publicKey, dsaEncoding: "ieee-p1363" }, Buffer.from(s!, "base64url"))).toBe(true);
    expect(JSON.parse(pttPayload("x".repeat(100)))).toEqual({ aps: {}, activeSpeaker: "x".repeat(64) });
  });

  it("is disabled without a key", () => {
    expect(createApns({ topic: "t" })).toBeNull();
    expect(createApns({ keyP8: "k", topic: "t" })).toBeNull();
  });

  it("posts a pushtotalk request with the provider token and reports the APNs status", async () => {
    const { privateKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
    const seen: { headers: IncomingHttpHeaders; body: string }[] = [];
    const server: Http2Server = createServer((req, res) => {
      let body = "";
      req.on("data", (c: Buffer) => (body += c.toString()));
      req.on("end", () => {
        seen.push({ headers: req.headers, body });
        res.statusCode = req.url.endsWith("/gone") ? 410 : 200;
        res.end();
      });
    });
    await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
    const origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
    const apns = createApns({
      keyP8: privateKey.export({ type: "pkcs8", format: "pem" }).toString(),
      keyId: "KEYID12345",
      teamId: "TEAM123456",
      topic: "ro.titi.app.voip-ptt",
      origins: { production: origin, sandbox: origin },
    })!;
    expect(await apns.send("abcd", false, "Ana")).toBe(200);
    expect(await apns.send("gone", true, "Ana")).toBe(410);
    server.close();
    expect(seen[0]!.headers[":path"]).toBe("/3/device/abcd");
    expect(seen[0]!.headers["apns-push-type"]).toBe("pushtotalk");
    expect(seen[0]!.headers["apns-topic"]).toBe("ro.titi.app.voip-ptt");
    expect(seen[0]!.headers["apns-priority"]).toBe("10");
    expect(seen[0]!.headers.authorization).toMatch(/^bearer [\w-]+\.[\w-]+\.[\w-]+$/);
    expect(JSON.parse(seen[0]!.body)).toEqual({ aps: {}, activeSpeaker: "Ana" });
  });

  it("resolves 0 when APNs is unreachable", async () => {
    const { privateKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
    const apns = createApns({
      keyP8: privateKey.export({ type: "pkcs8", format: "pem" }).toString(),
      keyId: "K",
      teamId: "T",
      topic: "t",
      origins: { production: "http://127.0.0.1:1", sandbox: "http://127.0.0.1:1" },
    })!;
    expect(await apns.send("x", false, "n")).toBe(0);
  });
});
