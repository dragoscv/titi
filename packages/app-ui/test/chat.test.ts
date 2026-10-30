import { beforeEach, describe, expect, it } from "vitest";
import { canDelete, canEdit, EDIT_WINDOW_MS, forwardedLabel, insertMention, MAX_EDITS, mentionQuery, mergeChat, previewOf, quoteText, reactionLabel, resolveMentions, textOf } from "../src/chat";
import { setLocale } from "../src/i18n";
import type { ChatItem, ChatReply } from "../src/types";

const ME = "aa".repeat(8);
const BOB = "bb".repeat(8);
const item = (id: string, p: Partial<ChatItem> = {}): ChatItem => ({
  group: "g", msg_uuid: id, from: ME, sent_ms: 1000, sort_ms: 1000, body: { kind: "text", text: "hi" }, deleted: false, edited: 0,
  reply: null, mentions: [], mention_all: false, mentions_me: false, forwarded: 0, reactions: [], acked: 0, ...p,
});

beforeEach(() => setLocale("en"));

describe("mergeChat", () => {
  it("returns the same list when nothing is merged", () => {
    const l = [item("a")];
    expect(mergeChat(l, [])).toBe(l);
  });

  it("upserts, sorts by sort_ms and honours the cap", () => {
    const out = mergeChat([item("a", { sort_ms: 1 }), item("b", { sort_ms: 3 })], [item("c", { sort_ms: 2 }), item("a", { sort_ms: 1, edited: 1 })], 2);
    expect(out.map((r) => r.msg_uuid)).toEqual(["c", "b"]);
    expect(mergeChat([item("b", { sort_ms: 1 })], [item("a", { sort_ms: 1 })]).map((r) => r.msg_uuid)).toEqual(["a", "b"]);
    expect(mergeChat([item("a", { sort_ms: 1 })], [item("b", { sort_ms: 1 })]).map((r) => r.msg_uuid)).toEqual(["a", "b"]);
  });
});

describe("edit / delete rules", () => {
  it("only my own, live text within 24 h and under 10 edits can be edited", () => {
    const now = 1000 + EDIT_WINDOW_MS - 1;
    expect(canEdit(item("a"), ME, now)).toBe(true);
    expect(canEdit(item("a"), ME, 1000 + EDIT_WINDOW_MS)).toBe(false);
    expect(canEdit(item("a", { from: BOB }), ME, now)).toBe(false);
    expect(canEdit(item("a", { edited: MAX_EDITS }), ME, now)).toBe(false);
    expect(canEdit(item("a", { deleted: true, body: null }), ME, now)).toBe(false);
    expect(canEdit(item("a", { body: { kind: "location", lat_e7: 0, lon_e7: 0, accuracy_m: 1, breadcrumb: false } }), ME, now)).toBe(false);
  });

  it("only my own, not yet deleted messages can be deleted", () => {
    expect(canDelete(item("a"), ME)).toBe(true);
    expect(canDelete(item("a", { from: BOB }), ME)).toBe(false);
    expect(canDelete(item("a", { deleted: true }), ME)).toBe(false);
  });
});

describe("labels", () => {
  it("previews every body kind and deleted rows", () => {
    expect(previewOf(item("a"), "Bob")).toBe("hi");
    expect(previewOf(item("a", { deleted: true, body: null }), "Bob")).toBe("This message was deleted");
    expect(previewOf(item("a", { body: { kind: "voiceNote", profile: "std", duration_ms: 1, opus_packets: [] } }), "Bob")).toBe("Voice note");
    expect(previewOf(item("a", { body: { kind: "location", lat_e7: 0, lon_e7: 0, accuracy_m: 1, breadcrumb: false } }), "Bob")).toBe("Bob shared a location");
    expect(previewOf(item("a", { body: { kind: "sos", lat_e7: 0, lon_e7: 0, note: "", cancelled: false } }), "Bob")).toBe("SOS from Bob");
    expect(previewOf(item("a", { body: { kind: "sos", lat_e7: 0, lon_e7: 0, note: "", cancelled: true } }), "Bob")).toBe("Bob cancelled the SOS");
    expect(textOf(null)).toBe("");
    expect(textOf({ kind: "voiceNote", profile: "std", duration_ms: 1, opus_packets: [] })).toBe("");
  });

  it("quotes show the deleted / missing state or a kind label", () => {
    const r = (p: Partial<ChatReply>): ChatReply => ({ msg_uuid: "x", sender: BOB, preview: "orig", kind: "text", state: "present", ...p });
    expect(quoteText(r({}))).toBe("orig");
    expect(quoteText(r({ state: "deleted" }))).toBe("Original message deleted");
    expect(quoteText(r({ state: "missing" }))).toBe("Message not on this device");
    expect(quoteText(r({ kind: "voice", preview: "" }))).toBe("Voice note");
    expect(quoteText(r({ kind: "location", preview: "near" }))).toBe("Location · near");
    expect(quoteText(r({ kind: "sos", preview: "" }))).toBe("SOS");
  });

  it("forwarded label switches to 'many times' at 5", () => {
    expect(forwardedLabel(0)).toBeNull();
    expect(forwardedLabel(1)).toBe("Forwarded");
    expect(forwardedLabel(5)).toBe("Forwarded many times");
  });

  it("reaction chips say whether I am included", () => {
    expect(reactionLabel({ emoji: "👍", count: 2, mine: true })).toBe("👍 2, including you");
    expect(reactionLabel({ emoji: "❤️", count: 1, mine: false })).toBe("❤️ 1");
  });
});

describe("mentions", () => {
  it("finds the @query before the caret only at a word start", () => {
    expect(mentionQuery("hi @Bo", 6)).toEqual({ query: "Bo", start: 3 });
    expect(mentionQuery("@", 1)).toEqual({ query: "", start: 0 });
    expect(mentionQuery("mail@x", 6)).toBeNull();
    expect(mentionQuery("hi @Bob there", 13)).toBeNull();
  });

  it("inserts the picked name with a trailing space and moves the caret", () => {
    expect(insertMention("hi @Bo!", 3, 6, "Bob")).toEqual({ text: "hi @Bob !", caret: 8 });
  });

  it("keeps only mentions still present in the text; @all only for the creator", () => {
    const picked = [{ node: BOB, name: "Bob" }, { node: "cc", name: "Ana (tel)" }, { node: BOB, name: "Bob" }];
    expect(resolveMentions("@Bob and @Ana (tel), @all!", picked, true)).toEqual({ mentions: [BOB, "cc"], mentionAll: true });
    expect(resolveMentions("@Bobby @all", picked, false)).toEqual({ mentions: [], mentionAll: false });
  });
});
