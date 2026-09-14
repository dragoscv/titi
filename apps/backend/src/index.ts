import { serve } from "@hono/node-server";
import { createApp } from "./app.ts";
import { loadConfig } from "./config.ts";
import { log, setLogLevel } from "./log.ts";
import { Relay } from "./relay.ts";

const cfg = loadConfig();
setLogLevel(cfg.LOG_LEVEL);
const relay = new Relay(cfg);
const { app, injectWebSocket } = createApp(cfg, relay);

const server = serve({ fetch: app.fetch, port: cfg.PORT, hostname: "0.0.0.0" }, (info) => {
  log.info("listening", { port: info.port });
});
injectWebSocket(server);

const sweeper = setInterval(() => relay.sweep(), 30_000);

function shutdown(signal: string) {
  log.info("shutdown", { signal });
  clearInterval(sweeper);
  server.close(() => process.exit(0));
  setTimeout(() => process.exit(0), 5_000).unref();
}
process.on("SIGTERM", () => shutdown("SIGTERM"));
process.on("SIGINT", () => shutdown("SIGINT"));
