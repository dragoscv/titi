import { expect, test } from "@playwright/test";
import { RELAY_WS } from "../playwright.config.ts";

// Relay service contract, black-box over HTTP/WS (runs against local or E2E_RELAY_URL).
const httpBase = RELAY_WS.replace(/^ws/, "http").replace(/\/v1\/ws$/, "");

test("@smoke relay /health reports ok", async ({ request }) => {
  const r = await request.get(`${httpBase}/health`);
  expect(r.ok()).toBe(true);
  expect(await r.json()).toMatchObject({ ok: true, service: "titi-relay" });
});

test("relay rejects a non-upgrade GET on the websocket path", async ({ request }) => {
  const r = await request.get(`${httpBase}/v1/ws`);
  expect(r.status()).toBeGreaterThanOrEqual(400);
});

test("relay closes on garbage binary frames without crashing", async ({ page, request }) => {
  const closed = await page.evaluate(async (url) => {
    return await new Promise<number>((resolve) => {
      const ws = new WebSocket(url);
      ws.binaryType = "arraybuffer";
      ws.onopen = () => ws.send(new Uint8Array(8192).fill(0xff));
      ws.onclose = (e) => resolve(e.code);
      setTimeout(() => resolve(-1), 5000);
    });
  }, RELAY_WS);
  expect(closed).not.toBe(0);
  const h = await request.get(`${httpBase}/health`);
  expect(h.ok(), "relay still healthy after abuse").toBe(true);
});
