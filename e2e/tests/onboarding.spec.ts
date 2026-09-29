import { expect, test } from "@playwright/test";
import { watch } from "../fixtures.ts";

test.describe("onboarding", () => {
  test("@smoke @mobile first run → name + colour → home, persisted across reload", async ({ page }) => {
    const w = watch(page, "new-user");
    await page.goto("/");
    const cont = page.getByRole("button", { name: "Continue" });
    const name = page.getByPlaceholder("Your name");

    await expect(name).toBeFocused();
    await expect(cont).toBeDisabled();
    await name.fill("A"); // one char is too short
    await expect(cont).toBeDisabled();
    await name.fill("  "); // whitespace does not count
    await expect(cont).toBeDisabled();

    await name.fill("Bogdan");
    await page.getByRole("button", { name: "Colour 220" }).click();
    await cont.click();
    await expect(page.getByText("No groups yet")).toBeVisible();
    await expect(page.getByText("Bogdan", { exact: true })).toBeVisible();

    await page.reload();
    await expect(page.getByText("No groups yet")).toBeVisible();
    const saved = await page.evaluate(() => JSON.parse(localStorage.getItem("titi.settings") ?? "{}") as { name: string; hue: number; onboarded: boolean });
    expect(saved).toMatchObject({ name: "Bogdan", hue: 220, onboarded: true });
    expect(w.errors).toEqual([]);
  });

  test("name is capped at 24 characters", async ({ page }) => {
    await page.goto("/");
    const name = page.getByPlaceholder("Your name");
    await name.fill("x".repeat(40));
    await expect(name).toHaveValue("x".repeat(24));
  });

  test("identity seed survives reload (same node id)", async ({ page }) => {
    await page.goto("/");
    await page.getByPlaceholder("Your name").fill("Seed");
    await page.getByRole("button", { name: "Continue" }).click();
    const seed1 = await page.evaluate(() => localStorage.getItem("titi.seed"));
    expect(seed1).toMatch(/^[0-9a-f]{64}$/);
    await page.reload();
    await expect(page.getByText("No groups yet")).toBeVisible();
    expect(await page.evaluate(() => localStorage.getItem("titi.seed"))).toBe(seed1);
  });
});
