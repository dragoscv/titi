import { expect, test } from "../fixtures.ts";

test("settings: rename, colour, volume and relay URL persist", async ({ user }) => {
  const a = await user("Ana");
  await a.page.getByRole("button", { name: "Settings" }).click();
  await expect(a.page.getByRole("heading", { name: "Settings" })).toBeVisible();

  await a.page.getByLabel("Name").fill("Ana Maria");
  await a.page.getByRole("button", { name: "Colour 290" }).click();
  await a.page.getByLabel(/Speaker volume/).fill("0.5");
  await expect(a.page.getByText("Speaker volume · 50%")).toBeVisible();

  const relay = a.page.getByLabel("Relay URL");
  const original = await relay.inputValue();
  await relay.fill("wss://example.invalid/v1/ws");
  await a.page.getByRole("button", { name: "Reset to default" }).click();
  await expect(relay).not.toHaveValue("wss://example.invalid/v1/ws");

  await a.page.reload();
  const s = await a.page.evaluate(() => JSON.parse(localStorage.getItem("titi.settings") ?? "{}") as Record<string, unknown>);
  expect(s).toMatchObject({ name: "Ana Maria", hue: 290, volume: 0.5 });
  expect(original).toBeTruthy();
});

test("settings back button returns home", async ({ user }) => {
  const a = await user("Ana");
  await a.page.getByRole("button", { name: "Settings" }).click();
  await a.page.getByRole("button", { name: "Back" }).click();
  await expect(a.page.getByText("No groups yet")).toBeVisible();
});
