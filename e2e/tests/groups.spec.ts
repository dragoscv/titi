import { createGroup, expect, readInviteCode, test } from "../fixtures.ts";

// Real multi-client flows: separate browser contexts = separate identities, talking through the relay.

test("@smoke host creates a group; it persists across reload", async ({ user }) => {
  const a = await user("Ana");
  await createGroup(a.page, "Cabana");
  await expect(a.page.getByText("1 member")).toBeVisible();
  await expect(a.page.getByText("Channel free")).toBeVisible();
  await a.page.reload();
  await expect(a.page.getByRole("button", { name: /Cabana/ })).toBeVisible();
});

test("create is disabled for names shorter than 2 characters", async ({ user }) => {
  const a = await user("Ana");
  await a.page.getByRole("button", { name: "New group" }).click();
  const create = a.page.getByRole("button", { name: "Create", exact: true });
  await a.page.getByPlaceholder(/Cabana/).fill("x");
  await expect(create).toBeDisabled();
  await a.page.keyboard.press("Escape");
  await expect(create).toBeHidden();
});

test("@smoke guest joins by spoken code over the relay; both see 2 members", async ({ user }) => {
  const host = await user("Ana");
  const guest = await user("Bogdan");
  await createGroup(host.page, "Trail team");
  const code = await readInviteCode(host.page);

  await guest.page.getByRole("button", { name: "Join" }).click();
  await guest.page.getByPlaceholder("word word word 00").fill(code);
  const join = guest.page.getByRole("button", { name: "Join", exact: true });
  await expect(join).toBeEnabled();
  await join.click();

  await expect(guest.page.getByRole("heading", { name: "Trail team" })).toBeVisible({ timeout: 20_000 });
  await expect(guest.page.getByText("2 members")).toBeVisible();
  await host.page.keyboard.press("Escape");
  await expect(host.page.getByText("2 members")).toBeVisible();
  await expect(host.page.getByText("Bogdan", { exact: true })).toBeVisible();
});

test("invalid code keeps Join disabled; typos in separators are tolerated", async ({ user }) => {
  const host = await user("Ana");
  const guest = await user("Bogdan");
  await createGroup(host.page, "Ski lift");
  const code = await readInviteCode(host.page);

  await guest.page.getByRole("button", { name: "Join" }).click();
  const input = guest.page.getByPlaceholder("word word word 00");
  const join = guest.page.getByRole("button", { name: "Join", exact: true });
  await input.fill("not a real code 99");
  await expect(join).toBeDisabled();
  await input.fill(code.toUpperCase().replaceAll(" ", "  "));
  await expect(join).toBeEnabled();
});

test("text chat is end-to-end between members", async ({ user }) => {
  const host = await user("Ana");
  const guest = await user("Bogdan");
  await createGroup(host.page, "Chat room");
  const code = await readInviteCode(host.page);
  await guest.page.getByRole("button", { name: "Join" }).click();
  await guest.page.getByPlaceholder("word word word 00").fill(code);
  await guest.page.getByRole("button", { name: "Join", exact: true }).click();
  await expect(guest.page.getByText("2 members")).toBeVisible({ timeout: 20_000 });
  await host.page.keyboard.press("Escape");

  await guest.page.getByRole("button", { name: "Chat" }).click();
  const send = guest.page.getByRole("button", { name: "Send" });
  await expect(send).toBeDisabled();
  await guest.page.getByPlaceholder("Message").fill("salut din pădure 🌲");
  await send.click();

  // the host is viewing this group, so no unread badge: the message shows up in the open chat
  await host.page.getByRole("button", { name: "Chat" }).click();
  await expect(host.page.getByText("salut din pădure 🌲")).toBeVisible();
});

test("push-to-talk: host holds the floor, guest sees who is talking", async ({ user }) => {
  const host = await user("Ana");
  const guest = await user("Bogdan");
  await createGroup(host.page, "Radio");
  const code = await readInviteCode(host.page);
  await guest.page.getByRole("button", { name: "Join" }).click();
  await guest.page.getByPlaceholder("word word word 00").fill(code);
  await guest.page.getByRole("button", { name: "Join", exact: true }).click();
  await expect(guest.page.getByText("2 members")).toBeVisible({ timeout: 20_000 });
  await host.page.keyboard.press("Escape");

  const talk = host.page.getByRole("button", { name: /Talk button/ });
  const box = (await talk.boundingBox())!;
  await host.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await host.page.mouse.down();
  // either the floor is granted or the host surfaces why not (mic/codec error toast): assert on the reason
  const outcome = host.page.getByText(/You are talking|Microphone unavailable.*/);
  await expect(outcome).toBeVisible();
  expect(await outcome.first().textContent()).toBe("You are talking");
  await expect(guest.page.getByText("Ana is talking")).toBeVisible();
  await host.page.mouse.up();
  await expect(host.page.getByText("Channel free")).toBeVisible();
  await expect(guest.page.getByText("Channel free")).toBeVisible();
});

test("switching to open mic and back", async ({ user }) => {
  const a = await user("Ana");
  await createGroup(a.page, "Duplex");
  await a.page.getByRole("button", { name: "Open mic" }).click();
  await expect(a.page.getByRole("button", { name: "Open mic" })).toHaveClass(/text-teal/);
  await a.page.getByRole("button", { name: "Push to talk" }).click();
  await expect(a.page.getByRole("button", { name: "Push to talk" })).toHaveClass(/text-amber/);
});

test("leave group asks for confirmation; cancel keeps it, accept removes it", async ({ user }) => {
  const a = await user("Ana");
  await createGroup(a.page, "Temp");
  a.page.once("dialog", (d) => void d.dismiss());
  await a.page.getByRole("button", { name: "More" }).click();
  await a.page.getByRole("button", { name: "Leave group" }).click();
  await expect(a.page.getByRole("heading", { name: "Temp" })).toBeVisible();

  a.page.once("dialog", (d) => void d.accept());
  await a.page.getByRole("button", { name: "More" }).click();
  await a.page.getByRole("button", { name: "Leave group" }).click();
  await expect(a.page.getByText("No groups yet")).toBeVisible();
});

test("deep link /j/<token> joins the group", async ({ user }) => {
  const host = await user("Ana");
  const guest = await user("Bogdan");
  await createGroup(host.page, "Link room");
  await host.page.getByRole("button", { name: "Invite" }).click();
  await expect(host.page.getByRole("img", { name: "Invite QR" })).toBeVisible();
  await host.page.getByRole("button", { name: "Share link" }).click();
  const url = await host.page.evaluate(() => navigator.clipboard.readText());
  const token = url.split("/j/")[1];
  expect(token).toBeTruthy();
  await host.page.keyboard.press("Escape");

  await guest.page.goto(`/j/${token}`);
  await expect(guest.page.getByRole("heading", { name: "Link room" })).toBeVisible({ timeout: 20_000 });
});
