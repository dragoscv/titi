// Pure chat helpers shared by the reducer, hosts and screens (message model v2).
import { t } from "./i18n";
import type { ChatItem, MsgBody } from "./types";

/** Rows kept in memory per group (older ones are paged in from the host on demand). */
export const CHAT_CAP = 500;
export const EDIT_WINDOW_MS = 86_400_000;
export const MAX_EDITS = 10;
export const FORWARDED_MANY = 5;
export const QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🙏"] as const;

const byOrder = (a: ChatItem, b: ChatItem) => a.sort_ms - b.sort_ms || (a.msg_uuid < b.msg_uuid ? -1 : a.msg_uuid > b.msg_uuid ? 1 : 0);

/** Upsert rows by msg_uuid, keep sort_ms order, keep the newest `cap`. */
export function mergeChat(list: ChatItem[], items: readonly ChatItem[], cap = CHAT_CAP): ChatItem[] {
  if (items.length === 0) return list;
  const map = new Map(list.map((i) => [i.msg_uuid, i]));
  for (const i of items) map.set(i.msg_uuid, i);
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts a fresh copy; toSorted() is missing on older Tizen Chromium
  return [...map.values()].sort(byOrder).slice(-cap);
}

/** Own text message that may still be edited (24 h window, at most 10 edits). */
export function canEdit(i: ChatItem, me: string, now: number): boolean {
  return i.from === me && !i.deleted && i.body?.kind === "text" && now - i.sent_ms < EDIT_WINDOW_MS && i.edited < MAX_EDITS;
}

export const canDelete = (i: ChatItem, me: string) => i.from === me && !i.deleted;

export const textOf = (b: MsgBody | null) => (b?.kind === "text" ? b.text : "");

/** One-line summary of a row for lists, tiles and the TV stage. */
export function previewOf(i: ChatItem, fromName: string): string {
  const b = i.body;
  if (i.deleted || !b) return t("chat.deleted");
  switch (b.kind) {
    case "text": return b.text;
    case "voiceNote": return t("chat.kind.voice");
    case "location": return t("chat.location", { name: fromName });
    case "sos": return t(b.cancelled ? "chat.sosCancelled" : "chat.sos", { name: fromName });
    default: return "";
  }
}

/** Label of a quoted message (kind prefix, or the deleted / missing state). */
export function quoteText(r: NonNullable<ChatItem["reply"]>): string {
  if (r.state === "deleted") return t("chat.replyDeleted");
  if (r.state === "missing") return t("chat.replyMissing");
  if (r.kind === "text") return r.preview;
  const label = t(r.kind === "voice" ? "chat.kind.voice" : r.kind === "location" ? "chat.kind.location" : "chat.kind.sos");
  return r.preview ? `${label} · ${r.preview}` : label;
}

export const forwardedLabel = (n: number) => (n >= FORWARDED_MANY ? t("chat.forwardedMany") : n > 0 ? t("chat.forwarded") : null);

/** The `@word` being typed right before the caret, or null. */
export function mentionQuery(text: string, caret: number): { query: string; start: number } | null {
  const m = /(^|\s)@([^\s@]*)$/.exec(text.slice(0, caret));
  if (!m) return null;
  return { query: m[2]!, start: caret - m[2]!.length - 1 };
}

/** Replace the `@query` at `start` with `@name ` and return the new text + caret. */
export function insertMention(text: string, start: number, caret: number, name: string): { text: string; caret: number } {
  const ins = `@${name} `;
  return { text: text.slice(0, start) + ins + text.slice(caret), caret: start + ins.length };
}

/** Mentions whose `@name` is still in the text when sending; `@all` only counts for the creator. */
export function resolveMentions(text: string, picked: readonly { node: string; name: string }[], allowAll: boolean): { mentions: string[]; mentionAll: boolean } {
  const has = (name: string) => new RegExp(`(^|\\s)@${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?=$|[\\s.,!?;:])`, "u").test(text);
  const mentions = [...new Set(picked.filter((p) => has(p.name)).map((p) => p.node))];
  return { mentions, mentionAll: allowAll && has("all") };
}

/** Accessible label of a reaction chip. */
export function reactionLabel(r: { emoji: string; count: number; mine: boolean }) {
  return t(r.mine ? "chat.reactionMine" : "chat.reaction", { emoji: r.emoji, count: r.count });
}
