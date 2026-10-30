import { describe, expect, it } from "vitest";
import { chatOf, useStore } from "../src/store";
import { LinkClassCode, LinkIds, type ChatItem } from "../src/types";

const row = (id: string): ChatItem => ({
  group: "g1", msg_uuid: id, from: "aa", sent_ms: 1, sort_ms: 1, body: { kind: "text", text: "x" }, deleted: false, edited: 0,
  reply: null, mentions: [], mention_all: false, mentions_me: false, forwarded: 0, reactions: [], acked: 0,
});

describe("chatOf selector", () => {
  it("returns one stable empty array for unknown groups (no re-render loop)", () => {
    const s = useStore.getState();
    expect(chatOf("nope")(s)).toBe(chatOf("other")(s));
    expect(chatOf("nope")(s)).toEqual([]);
  });

  it("returns the group's rows when present", () => {
    useStore.getState().set({ chat: { g1: [row("a")] } });
    expect(chatOf("g1")(useStore.getState()).map((r) => r.msg_uuid)).toEqual(["a"]);
    useStore.getState().set({ chat: {} });
  });
});

describe("wire constants", () => {
  it("link ids and class codes match core/titi-core/src/link.rs", () => {
    expect(LinkIds).toEqual({ LAN: 1, BLE_GATT: 6, INTERNET: 8, WEBRTC: 9 });
    expect(LinkClassCode).toEqual({ internet: 8, webRtc: 9 });
  });
});
