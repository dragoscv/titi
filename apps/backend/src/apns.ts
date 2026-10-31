// Apple Push to Talk wake-ups (ADR-0011). An iPhone in a system PTT channel registers its ephemeral
// APNs token for the groups it listens to; when it is suspended (socket gone) and voice starts in one
// of those rooms, the relay sends one `pushtotalk` push carrying only the talker's display name.
// No key configured -> every call is a no-op. Tokens live in memory only and are never logged.

import { connect, type ClientHttp2Session } from "node:http2";
import { createPrivateKey, sign, type KeyObject } from "node:crypto";
import { log } from "./log.ts";

export interface ApnsConfig {
  keyP8?: string | undefined;
  keyId?: string | undefined;
  teamId?: string | undefined;
  topic: string; // <bundle id>.voip-ptt
  /** Test seam: override the APNs origins (e.g. a local h2c server). */
  origins?: { production: string; sandbox: string } | undefined;
}

export interface PushSender {
  send(token: string, sandbox: boolean, name: string): Promise<number>;
}

const b64url = (b: Buffer | string) => Buffer.from(b).toString("base64url");

/** ES256 provider token (APNs accepts one for up to 60 min; we refresh every 40). */
export function providerToken(key: KeyObject, keyId: string, teamId: string, nowS: number): string {
  const h = b64url(JSON.stringify({ alg: "ES256", kid: keyId }));
  const p = b64url(JSON.stringify({ iss: teamId, iat: nowS }));
  const sig = sign("sha256", Buffer.from(`${h}.${p}`), { key, dsaEncoding: "ieee-p1363" });
  return `${h}.${p}.${b64url(sig)}`;
}

export function pttPayload(name: string): string {
  return JSON.stringify({ aps: {}, activeSpeaker: name.slice(0, 64) });
}

export function createApns(cfg: ApnsConfig): PushSender | null {
  if (!cfg.keyP8 || !cfg.keyId || !cfg.teamId) return null;
  const key = createPrivateKey(cfg.keyP8);
  const keyId = cfg.keyId, teamId = cfg.teamId;
  let token = "", tokenAt = 0;
  const sessions = new Map<string, ClientHttp2Session>();
  const origins = cfg.origins ?? { production: "https://api.push.apple.com", sandbox: "https://api.sandbox.push.apple.com" };
  const session = (sandbox: boolean) => {
    const host = sandbox ? origins.sandbox : origins.production;
    let s = sessions.get(host);
    if (!s || s.closed || s.destroyed) {
      s = connect(host);
      s.on("error", (e) => log.warn("apns.session", { err: e.message }));
      s.on("close", () => sessions.delete(host));
      s.unref();
      sessions.set(host, s);
    }
    return s;
  };
  return {
    send(deviceToken, sandbox, name) {
      const now = Math.floor(Date.now() / 1000);
      if (!token || now - tokenAt > 40 * 60) { token = providerToken(key, keyId, teamId, now); tokenAt = now; }
      return new Promise((resolve) => {
        const req = session(sandbox).request({
          ":method": "POST",
          ":path": `/3/device/${deviceToken}`,
          authorization: `bearer ${token}`,
          "apns-push-type": "pushtotalk",
          "apns-topic": cfg.topic,
          "apns-priority": "10",
          "apns-expiration": "0",
        });
        req.setTimeout(5000, () => { req.close(); resolve(0); });
        req.on("response", (h) => {
          const status = Number(h[":status"] ?? 0);
          if (status !== 200) log.warn("apns.push", { status });
          resolve(status);
        });
        req.on("error", () => resolve(0));
        req.end(pttPayload(name));
      });
    },
  };
}
