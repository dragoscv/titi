import { Hono } from "hono";
import { createNodeWebSocket } from "@hono/node-ws";
import type { Config } from "./config.ts";
import { log } from "./log.ts";
import { Relay, type Sink } from "./relay.ts";

export function createApp(cfg: Config, relay: Relay) {
  const app = new Hono();
  const nodeWs = createNodeWebSocket({ app });
  const injectWebSocket = (server: Parameters<typeof nodeWs.injectWebSocket>[0]) => nodeWs.injectWebSocket(server);

  app.get("/health", (c) =>
    c.json({ ok: true, service: "titi-relay", version: process.env.GIT_SHA ?? process.env.npm_package_version ?? "dev", ...relay.stats() }),
  );

  app.get(
    "/v1/ws",
    nodeWs.upgradeWebSocket(() => {
      let id = -1;
      return {
        onOpen(_evt, ws) {
          const sink: Sink = {
            send: (b) => {
              if (ws.readyState === 1) ws.send(b.slice().buffer);
            },
            close: (code, reason) => ws.close(code, reason),
          };
          id = relay.connect(sink);
          log.debug("ws.open", { id });
        },
        onMessage(evt) {
          const d = evt.data;
          if (d instanceof ArrayBuffer) relay.onMessage(id, new Uint8Array(d));
          else if (d instanceof Uint8Array) relay.onMessage(id, d);
          else if (typeof Blob !== "undefined" && d instanceof Blob) {
            void d.arrayBuffer().then((ab) => relay.onMessage(id, new Uint8Array(ab)));
          }
          // text frames are ignored: protocol is binary only
        },
        onClose() {
          relay.disconnect(id);
          log.debug("ws.close", { id });
        },
        onError(evt) {
          log.warn("ws.error", { id, err: "message" in evt && typeof evt.message === "string" ? evt.message : evt.type });
        },
      };
    }),
  );

  void cfg;
  return { app, injectWebSocket };
}
