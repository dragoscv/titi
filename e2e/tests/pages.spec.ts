import AxeBuilder from "@axe-core/playwright";
import { test as base } from "@playwright/test";
import { expect, test, watch } from "../fixtures.ts";

// Every public route: renders, no console errors, security headers, WCAG 2.2 AA (axe).
const ROUTES = [
  { path: "/", ready: /Talk anywhere/ },
  { path: "/privacy", ready: /Privacy policy/ },
  { path: "/~offline", ready: /offline/i },
];

for (const r of ROUTES) {
  base(`@smoke ${r.path} renders clean and passes axe`, async ({ page }, info) => {
    const w = watch(page, r.path);
    const res = await page.goto(r.path);
    expect(res?.status(), "HTTP status").toBeLessThan(400);
    await expect(page.getByText(r.ready).first()).toBeVisible();

    const h = res!.headers();
    expect(h["x-content-type-options"]).toBe("nosniff");
    expect(h["x-frame-options"]).toBe("DENY");
    expect(h["referrer-policy"]).toBe("strict-origin-when-cross-origin");

    const axe = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();
    const serious = axe.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
    if (serious.length) await info.attach("axe.json", { body: JSON.stringify(serious, null, 2), contentType: "application/json" });
    expect(serious.map((v) => `${v.id}: ${v.nodes.length} node(s)`)).toEqual([]);
    expect(w.errors).toEqual([]);
  });
}

base("@smoke manifest and service worker are served", async ({ request }) => {
  const m = await request.get("/manifest.webmanifest");
  expect(m.ok()).toBe(true);
  const json = (await m.json()) as { name?: string; icons?: unknown[] };
  expect(json.name).toBeTruthy();
  expect(json.icons?.length).toBeGreaterThan(0);
  const sw = await request.get("/serwist/sw.js");
  expect(sw.ok()).toBe(true);
  expect(sw.headers()["content-type"]).toMatch(/javascript/);
});

base("unknown route returns 404, not a crash", async ({ request }) => {
  const r = await request.get("/definitely-not-a-page");
  expect(r.status()).toBe(404);
});

test("@smoke home shows the empty state for a new user", async ({ user }) => {
  const a = await user("Ana");
  await expect(a.page.getByText("No groups yet")).toBeVisible();
  await expect(a.page.getByRole("button", { name: "New group" })).toBeEnabled();
  await expect(a.page.getByRole("button", { name: "Join" })).toBeEnabled();
});
