import { test as base, expect, type BrowserContext, type Page } from "@playwright/test";
import { RELAY_WS } from "./playwright.config.ts";

/**
 * Every page opened through these fixtures is watched: console errors, uncaught exceptions and
 * failed same-origin requests are collected and FAIL the test at teardown (with the log attached),
 * so a regression that only shows up in the console is still caught.
 */
export type Watched = { page: Page; errors: string[]; name: string };

const IGNORED = [
  /Download the React DevTools/,
  /\[Fast Refresh\]/,
  // fake media device noise in headless chromium
  /Requested device not found/,
];

export function watch(page: Page, name: string): Watched {
  const errors: string[] = [];
  const push = (s: string) => { if (!IGNORED.some((r) => r.test(s))) errors.push(s); };
  page.on("console", (m) => { if (m.type() === "error") push(`[console] ${m.text()}`); });
  page.on("pageerror", (e) => push(`[pageerror] ${e.message}\n${e.stack ?? ""}`));
  page.on("requestfailed", (r) => {
    const u = new URL(r.url());
    // websocket reconnects and aborted prefetches are expected; same-origin document/asset failures are not
    if (u.protocol.startsWith("ws") || r.failure()?.errorText === "net::ERR_ABORTED") return;
    push(`[requestfailed] ${r.method()} ${r.url()} ${r.failure()?.errorText ?? ""}`);
  });
  page.on("response", (r) => {
    if (r.status() >= 500) push(`[http ${r.status()}] ${r.url()}`);
  });
  return { page, errors, name };
}

/** Seed an onboarded identity so tests start on Home (the onboarding flow has its own spec). */
export async function seedUser(ctx: BrowserContext, name: string, hue = 40) {
  await ctx.addInitScript(
    ([n, h, relay]) => {
      if (localStorage.getItem("titi.settings")) return;
      localStorage.setItem("titi.settings", JSON.stringify({ name: n, hue: h, onboarded: true, relayUrl: relay }));
    },
    [name, hue, RELAY_WS] as const,
  );
}

type Fixtures = {
  /** A fresh, onboarded user in its own browser context (own identity, own storage). */
  user: (name: string) => Promise<Watched>;
};

export const test = base.extend<Fixtures>({
  user: async ({ browser }, use, info) => {
    const opened: { ctx: BrowserContext; w: Watched }[] = [];
    await use(async (name) => {
      const ctx = await browser.newContext({
        permissions: ["microphone", "clipboard-read", "clipboard-write"],
        baseURL: info.project.use.baseURL,
      });
      await seedUser(ctx, name);
      const page = await ctx.newPage();
      const w = watch(page, name);
      opened.push({ ctx, w });
      await page.goto("/");
      await expect(page.getByText(name, { exact: true }).first()).toBeVisible();
      return w;
    });
    const all = opened.flatMap(({ w }) => w.errors.map((e) => `${w.name}: ${e}`));
    if (all.length) await info.attach("browser-errors.txt", { body: all.join("\n"), contentType: "text/plain" });
    await Promise.all(opened.map(({ ctx }) => ctx.close()));
    expect(all, "browser console/page errors").toEqual([]);
  },
});

export { expect };

/** Create a group from Home and land on its screen. */
export async function createGroup(p: Page, name: string) {
  await p.getByRole("button", { name: "New group" }).click();
  await p.getByPlaceholder(/Cabana/).fill(name);
  await p.getByRole("button", { name: "Create", exact: true }).click();
  await p.getByRole("button", { name }).first().click();
  await expect(p.getByRole("heading", { name })).toBeVisible();
}

/** Open the invite sheet and read the spoken code ("word word word 00"). */
export async function readInviteCode(p: Page): Promise<string> {
  await p.getByRole("button", { name: "Invite" }).click();
  const code = p.getByText(/^[a-z]+ [a-z]+ [a-z]+ \d{2}$/);
  await expect(code).toBeVisible();
  return (await code.textContent())!.trim();
}
