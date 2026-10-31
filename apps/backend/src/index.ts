import { serve } from "@hono/node-server";
import { createApns } from "./apns.ts";
import { createApp } from "./app.ts";
import { loadConfig } from "./config.ts";
import { startDemoBot } from "./demo-bot.ts";
import { log, setLogLevel } from "./log.ts";
import { Relay } from "./relay.ts";

const cfg = loadConfig();
setLogLevel(cfg.LOG_LEVEL);
const push = createApns({ keyP8: cfg.APNS_KEY_P8, keyId: cfg.APNS_KEY_ID, teamId: cfg.APNS_TEAM_ID, topic: cfg.APNS_TOPIC });
log.info("apns", { enabled: push !== null });
const relay = new Relay(cfg, Date.now, push);
const demoBot = startDemoBot(relay, { seedHex: cfg.DEMO_BOT_SEED, groupsHex: cfg.DEMO_BOT_GROUPS });
log.info("demo_bot", { enabled: demoBot !== null });
const { app, injectWebSocket } = createApp(cfg, relay);

const server = serve({ fetch: app.fetch, port: cfg.PORT, hostname: "0.0.0.0" }, (info) => {
  log.info("listening", { port: info.port });
});
injectWebSocket(server);

const sweeper = setInterval(() => relay.sweep(), 30_000);

function shutdown(signal: string) {
  log.info("shutdown", { signal });
  clearInterval(sweeper);
  demoBot?.stop();
  server.close(() => process.exit(0));
  setTimeout(() => process.exit(0), 5_000).unref();
}
process.on("SIGTERM", () => shutdown("SIGTERM"));
process.on("SIGINT", () => shutdown("SIGINT"));
