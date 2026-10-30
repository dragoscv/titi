"use client";
// Group chat (message model v2) for pointer/keyboard hosts: web + desktop.
// Reply, react, edit (24 h / 10 edits, visible history), delete for everyone,
// forward, copy, @mentions, drafts and search. Every action is reachable by
// keyboard: each bubble has an actions button; Escape closes the innermost layer.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Ban, Copy, CornerUpLeft, Forward, MoreHorizontal, Pencil, Search, Send, SmilePlus, Trash2, X } from "lucide-react";
import { host } from "../platform";
import { chatOf, useStore, type GroupState } from "../store";
import { mergeChatItems, nameOf } from "../reducer";
import { canDelete, canEdit, forwardedLabel, insertMention, mentionQuery, previewOf, QUICK_REACTIONS, quoteText, reactionLabel, resolveMentions, textOf } from "../chat";
import { locale, t } from "../i18n";
import { cn } from "../cn";
import { Avatar } from "../Avatar";
import type { ChatItem, EditVersion } from "../types";

const DRAFT_DEBOUNCE_MS = 600;
const SEARCH_DEBOUNCE_MS = 250;
const LONG_PRESS_MS = 500;
const FLASH_MS = 1600;

type Overlay =
  | { kind: "history"; item: ChatItem; versions: EditVersion[] | null }
  | { kind: "forward"; item: ChatItem }
  | { kind: "delete"; item: ChatItem };

const time = (ms: number) => new Date(ms).toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });
const domId = (uuid: string) => `msg-${uuid}`;

export function ChatPanel({ g }: { g: GroupState }) {
  const gid = g.id;
  const me = useStore((s) => s.nodeId);
  const rows = useStore(chatOf(gid));
  const groups = useStore((s) => s.groups);
  const [text, setText] = useState("");
  const [caret, setCaret] = useState(0);
  const [replyTo, setReplyTo] = useState<ChatItem | null>(null);
  const [editing, setEditing] = useState<ChatItem | null>(null);
  const [menuFor, setMenuFor] = useState<string | null>(null);
  /** when the open menu was opened: decides whether Edit is still allowed (24 h window) */
  const [menuAt, setMenuAt] = useState(0);
  const [overlay, setOverlay] = useState<Overlay | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);
  const [picked, setPicked] = useState<{ node: string; name: string }[]>([]);
  const [mentionIdx, setMentionIdx] = useState(0);
  const [mentionOff, setMentionOff] = useState(false);
  const [olderDone, setOlderDone] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const stash = useRef("");
  const draftTimer = useRef<number | null>(null);
  const atBottom = useRef(true);
  const escape = useRef<(e: KeyboardEvent) => boolean>(() => false);

  // Escape closes the innermost chat layer before the sheet's own (bubble) listener sees it
  useEffect(() => {
    const k = (e: KeyboardEvent) => { if (e.key === "Escape" && escape.current(e)) { e.stopPropagation(); e.preventDefault(); } };
    window.addEventListener("keydown", k, true);
    return () => window.removeEventListener("keydown", k, true);
  }, []);

  // restore the saved draft when the chat opens (the group screen already loaded the newest page)
  useEffect(() => {
    let live = true;
    void host.draft(gid).then((d) => { if (live && d) setText((cur) => cur || d); });
    return () => { live = false; };
  }, [gid]);

  // keep the view pinned to the newest message unless the user scrolled up
  useLayoutEffect(() => {
    const el = list.current;
    if (el && atBottom.current && rows.length > 0) el.scrollTop = el.scrollHeight;
  }, [rows.length]);

  const pendingDraft = useRef<string | null>(null);
  const saveDraft = useCallback((v: string) => {
    if (draftTimer.current) clearTimeout(draftTimer.current);
    pendingDraft.current = v;
    draftTimer.current = window.setTimeout(() => { pendingDraft.current = null; host.setDraft(gid, v); }, DRAFT_DEBOUNCE_MS);
  }, [gid]);
  // closing the chat saves whatever was typed in the last debounce window
  useEffect(() => () => {
    if (draftTimer.current) clearTimeout(draftTimer.current);
    if (pendingDraft.current !== null) host.setDraft(gid, pendingDraft.current);
    pendingDraft.current = null;
  }, [gid]);

  const showFlash = useCallback((uuid: string) => {
    setFlash(uuid);
    requestAnimationFrame(() => document.getElementById(domId(uuid))?.scrollIntoView({ block: "center", behavior: "smooth" }));
    window.setTimeout(() => setFlash((f) => (f === uuid ? null : f)), FLASH_MS);
  }, []);

  // ---- composer -----------------------------------------------------------
  const mq = editing || mentionOff ? null : mentionQuery(text, caret);
  const candidates = useMemo(() => {
    if (!mq) return [];
    const q = mq.query.toLowerCase();
    const people = g.members.filter((m) => m.node !== me && m.name.toLowerCase().startsWith(q)).map((m) => ({ node: m.node, name: m.name, label: m.name }));
    const all = g.isCreator && "all".startsWith(q) ? [{ node: "", name: "all", label: t("chat.mentionAll") }] : [];
    return [...all, ...people].slice(0, 6);
  }, [mq, g.members, g.isCreator, me]);

  const onText = (v: string, pos: number) => {
    setText(v);
    setCaret(pos);
    setMentionIdx(0);
    setMentionOff(false);
    if (!editing) saveDraft(v);
  };

  const pickMention = (c: { node: string; name: string }) => {
    if (!mq) return;
    const r = insertMention(text, mq.start, caret, c.name);
    if (c.node) setPicked((p) => [...p.filter((x) => x.node !== c.node), { node: c.node, name: c.name }]);
    onText(r.text, r.caret);
    requestAnimationFrame(() => { input.current?.focus(); input.current?.setSelectionRange(r.caret, r.caret); });
  };

  const resetComposer = (next: string) => {
    setText(next);
    setCaret(next.length);
    setPicked([]);
  };

  const send = () => {
    const body = text.trim();
    if (!body) return;
    if (editing) {
      if (body !== textOf(editing.body)) host.editMessage(gid, editing.msg_uuid, body);
      setEditing(null);
      resetComposer(stash.current);
      return;
    }
    const { mentions, mentionAll } = resolveMentions(body, picked, g.isCreator);
    host.sendTextV2(gid, body, { replyTo: replyTo?.msg_uuid, mentions, mentionAll });
    setReplyTo(null);
    resetComposer("");
    if (draftTimer.current) clearTimeout(draftTimer.current);
    pendingDraft.current = null;
    host.setDraft(gid, "");
    atBottom.current = true;
  };

  const startEdit = (i: ChatItem) => {
    stash.current = editing ? stash.current : text;
    setReplyTo(null);
    setEditing(i);
    resetComposer(textOf(i.body));
    input.current?.focus();
  };
  const cancelEdit = () => { setEditing(null); resetComposer(stash.current); };

  const onComposerKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (candidates.length > 0) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); setMentionIdx((i) => (i + (e.key === "ArrowDown" ? 1 : candidates.length - 1)) % candidates.length); return; }
      if (e.key === "Enter" || e.key === "Tab") { e.preventDefault(); pickMention(candidates[Math.min(mentionIdx, candidates.length - 1)]!); return; }
    }
    if (e.key === "Enter") { e.preventDefault(); send(); }
  };

  // ---- message actions ------------------------------------------------------
  const toggleReaction = (i: ChatItem, emoji: string) => {
    const mine = i.reactions.find((r) => r.mine)?.emoji;
    host.react(gid, i.msg_uuid, mine === emoji ? "" : emoji);
    setMenuFor(null);
  };
  const openHistory = (i: ChatItem) => {
    setOverlay({ kind: "history", item: i, versions: null });
    void host.editHistory(i.msg_uuid).then((versions) => setOverlay((o) => (o?.kind === "history" && o.item.msg_uuid === i.msg_uuid ? { ...o, versions } : o)));
  };
  const copy = (i: ChatItem) => {
    void navigator.clipboard?.writeText(textOf(i.body)).then(() => useStore.getState().set({ toast: t("chat.copied") }), () => {});
    setMenuFor(null);
  };

  /** Escape closes the innermost open layer only (the sheet stays open). Returns whether it handled it. */
  const onEscape = () => {
    if (candidates.length > 0) setMentionOff(true);
    else if (overlay) setOverlay(null);
    else if (menuFor) { const id = menuFor; setMenuFor(null); requestAnimationFrame(() => document.getElementById(`${domId(id)}-more`)?.focus()); }
    else if (searchOpen) setSearchOpen(false);
    else if (editing) cancelEdit();
    else if (replyTo) setReplyTo(null);
    else return false;
    return true;
  };
  useLayoutEffect(() => { escape.current = onEscape; });

  const loadOlder = () => {
    const first = rows[0];
    if (!first) return;
    void host.history(gid, first.sort_ms).then((items) => {
      const known = new Set(useStore.getState().chat[gid]?.map((r) => r.msg_uuid));
      if (!items.some((i) => !known.has(i.msg_uuid))) setOlderDone(true);
      atBottom.current = false;
      mergeChatItems(items);
    });
  };

  const others = groups.filter((x) => x.id !== gid);
  const byId = useMemo(() => new Map(rows.map((r) => [r.msg_uuid, r])), [rows]);

  return (
    <div className="relative flex max-h-[60dvh] flex-col">
      <div className="mb-2 flex items-center justify-end gap-1">
        {searchOpen ? (
          <SearchBox gid={gid} onClose={() => setSearchOpen(false)} onPick={(i) => { mergeChatItems([i]); setSearchOpen(false); showFlash(i.msg_uuid); }} />
        ) : (
          <button type="button" className="rounded-full p-2 text-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-amber" onClick={() => setSearchOpen(true)} aria-label={t("chat.search")}><Search size={18} /></button>
        )}
      </div>

      <div
        ref={list}
        role="log"
        aria-label={t("chat.title")}
        className="flex flex-1 flex-col gap-2 overflow-y-auto pb-3"
        onScroll={(e) => { const el = e.currentTarget; atBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40; }}
      >
        {rows.length === 0 && <div className="py-8 text-center text-sm text-muted">{t("chat.empty")}</div>}
        {rows.length >= 20 && !olderDone && (
          <button type="button" className="mx-auto rounded-full px-3 py-1 text-xs text-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-amber" onClick={loadOlder}>{t("chat.loadOlder")}</button>
        )}
        {rows.map((i) => (
          <Row
            key={i.msg_uuid}
            i={i}
            me={me}
            hue={g.members.find((m) => m.node === i.from)?.hue ?? 200}
            flash={flash === i.msg_uuid}
            menuOpen={menuFor === i.msg_uuid}
            onMenu={(open) => { setMenuFor(open ? i.msg_uuid : null); if (open) setMenuAt(Date.now()); }}
            onQuote={(id) => { if (byId.has(id)) showFlash(id); }}
            onHistory={() => openHistory(i)}
            onReact={(emoji) => toggleReaction(i, emoji)}
            actions={{
              reply: () => { setMenuFor(null); if (editing) cancelEdit(); setReplyTo(i); input.current?.focus(); },
              edit: menuFor === i.msg_uuid && canEdit(i, me, menuAt) ? () => { setMenuFor(null); startEdit(i); } : undefined,
              del: canDelete(i, me) ? () => { setMenuFor(null); setOverlay({ kind: "delete", item: i }); } : undefined,
              forward: !i.deleted && i.body && others.length > 0 ? () => { setMenuFor(null); setOverlay({ kind: "forward", item: i }); } : undefined,
              copy: i.body?.kind === "text" ? () => copy(i) : undefined,
            }}
          />
        ))}
      </div>

      {(replyTo || editing) && (
        <div className="flex items-center gap-2 rounded-2xl bg-elevated px-3 py-2 text-sm">
          {editing ? <Pencil size={16} className="shrink-0 text-amber" /> : <CornerUpLeft size={16} className="shrink-0 text-amber" />}
          <div className="min-w-0 flex-1">
            <div className="text-xs font-semibold text-amber">{editing ? t("chat.editing") : t("chat.replyingTo", { name: replyTo!.from === me ? t("chat.you") : nameOf(replyTo!.from) })}</div>
            <div className="truncate text-muted">{previewOf((editing ?? replyTo)!, nameOf((editing ?? replyTo)!.from))}</div>
          </div>
          <button type="button" className="rounded-full p-1.5 text-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-amber" onClick={() => (editing ? cancelEdit() : setReplyTo(null))} aria-label={editing ? t("chat.cancelEdit") : t("chat.cancelReply")}><X size={16} /></button>
        </div>
      )}

      <div className="relative flex items-center gap-2 pt-2">
        {candidates.length > 0 && (
          <ul id={`mentions-${gid}`} aria-label={t("chat.mentions")} className="absolute bottom-full left-0 z-20 mb-2 w-64 overflow-hidden rounded-2xl bg-elevated shadow-2xl">
            {candidates.map((c, k) => (
              <li key={c.node || "all"}>
                <button type="button" tabIndex={-1} aria-current={k === mentionIdx} className={cn("block w-full px-4 py-2.5 text-left text-sm hover:bg-white/5", k === mentionIdx && "bg-white/10")} onMouseDown={(e) => e.preventDefault()} onClick={() => pickMention(c)}>
                  <b>@{c.name}</b>{c.node === "" && <span className="ml-2 text-muted">{c.label}</span>}
                </button>
              </li>
            ))}
          </ul>
        )}
        {/* screen readers hear the highlighted suggestion (↑/↓ move, Enter/Tab pick, Escape closes) */}
        <span className="sr-only" aria-live="polite">{candidates.length > 0 ? `@${candidates[Math.min(mentionIdx, candidates.length - 1)]!.name}` : ""}</span>
        <input
          ref={input}
          className="flex-1 rounded-full border border-outline bg-graphite px-4 py-3 outline-none focus:border-amber"
          placeholder={t("chat.placeholder")}
          aria-label={t("chat.placeholder")}
          aria-describedby={candidates.length > 0 ? `mentions-${gid}` : undefined}
          value={text}
          onChange={(e) => onText(e.target.value, e.target.selectionStart ?? e.target.value.length)}
          onSelect={(e) => setCaret(e.currentTarget.selectionStart ?? 0)}
          onKeyDown={onComposerKey}
        />
        <button type="button" className="grid h-12 w-12 place-items-center rounded-2xl bg-amber text-graphite outline-none focus-visible:ring-2 focus-visible:ring-ink disabled:opacity-40" disabled={!text.trim()} onClick={send} aria-label={editing ? t("chat.save") : t("chat.send")}><Send size={20} /></button>
      </div>

      {overlay && (
        <dialog open className="absolute inset-0 z-30 m-0 flex h-full w-full flex-col rounded-2xl border-0 bg-surface p-1 text-[inherit]" aria-modal aria-label={overlay.kind === "history" ? t("chat.editHistory") : overlay.kind === "forward" ? t("chat.forwardTo") : t("chat.deleteConfirm")}>
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-lg font-semibold">{overlay.kind === "history" ? t("chat.editHistory") : overlay.kind === "forward" ? t("chat.forwardTo") : t("chat.deleteConfirm")}</h3>
            <button type="button" className="rounded-full p-2 text-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-amber" onClick={() => setOverlay(null)} aria-label={t("chat.close")}><X size={18} /></button>
          </div>
          {overlay.kind === "history" && (
            <ol className="flex flex-col gap-2 overflow-y-auto">
              {(overlay.versions ?? []).map((v, k) => (
                <li key={k} className="rounded-2xl bg-elevated px-3.5 py-2.5"><div className="text-[15px]">{v.text}</div><div className="mt-0.5 font-mono text-[10px] text-muted">{time(v.sent_ms)}</div></li>
              ))}
              <li className="rounded-2xl bg-amber/20 px-3.5 py-2.5"><div className="text-[15px]">{textOf(overlay.item.body)}</div><div className="mt-0.5 font-mono text-[10px] text-muted">{t("chat.edited")}</div></li>
            </ol>
          )}
          {overlay.kind === "forward" && (
            <div className="flex flex-col gap-2 overflow-y-auto">
              {others.map((o, k) => (
                <button key={o.id} ref={k === 0 ? focusEl : undefined} type="button" className="rounded-2xl bg-elevated px-4 py-3 text-left outline-none hover:bg-white/5 focus-visible:ring-2 focus-visible:ring-amber" onClick={() => { host.forward(gid, overlay.item.msg_uuid, o.id); setOverlay(null); useStore.getState().set({ toast: t("chat.forwardDone", { name: o.name }) }); }}>{o.name}</button>
              ))}
            </div>
          )}
          {overlay.kind === "delete" && (
            <div className="flex flex-col gap-3">
              <div className="truncate rounded-2xl bg-elevated px-3.5 py-2.5 text-sm text-muted">{previewOf(overlay.item, nameOf(overlay.item.from))}</div>
              <div className="flex gap-2">
                <button type="button" className="flex-1 rounded-xl border border-outline py-2.5 outline-none focus-visible:ring-2 focus-visible:ring-amber" onClick={() => setOverlay(null)}>{t("chat.cancel")}</button>
                <button ref={focusEl} type="button" className="flex-1 rounded-xl bg-danger py-2.5 font-semibold text-graphite outline-none focus-visible:ring-2 focus-visible:ring-ink" onClick={() => { host.deleteMessage(gid, overlay.item.msg_uuid); setOverlay(null); }}>{t("chat.deleteYes")}</button>
              </div>
            </div>
          )}
        </dialog>
      )}
    </div>
  );
}

const focusEl = (el: HTMLElement | null) => { el?.focus(); };

/** Arrow / Home / End move between the menu items (the custom-emoji input keeps its arrows). */
function menuKeys(e: React.KeyboardEvent<HTMLDivElement>) {
  const keys = ["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Home", "End"];
  if (!keys.includes(e.key) || e.target instanceof HTMLInputElement) return;
  e.preventDefault();
  const items = Array.from(e.currentTarget.querySelectorAll<HTMLElement>("[role=menuitem]"));
  const k = items.findIndex((x) => x === document.activeElement);
  const next = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : (k + (e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : items.length - 1)) % items.length;
  items[next]?.focus();
}

interface RowActions { reply: () => void; edit?: () => void; del?: () => void; forward?: () => void; copy?: () => void }

function Row({ i, me, hue, flash, menuOpen, onMenu, onQuote, onHistory, onReact, actions }: {
  i: ChatItem; me: string; hue: number; flash: boolean; menuOpen: boolean;
  onMenu: (open: boolean) => void; onQuote: (id: string) => void; onHistory: () => void; onReact: (emoji: string) => void; actions: RowActions;
}) {
  const mine = i.from === me;
  const name = mine ? t("chat.you") : nameOf(i.from);
  const press = useRef<number | null>(null);
  const fwd = forwardedLabel(i.forwarded);
  const cancelPress = () => { if (press.current) { clearTimeout(press.current); press.current = null; } };
  return (
    <div id={domId(i.msg_uuid)} className={cn("group flex flex-col gap-1", mine ? "items-end" : "items-start")}>
      <div className={cn("flex max-w-full items-end gap-2", mine && "flex-row-reverse")}>
        {!mine && <Avatar name={name} hue={hue} size={28} />}
        <div
          className={cn(
            "max-w-[75%] rounded-[18px] px-3.5 py-2.5 transition-shadow duration-300",
            i.deleted ? "border border-dashed border-outline bg-transparent" : mine ? "bg-amber/20" : "bg-elevated",
            i.mentions_me && !i.deleted && "ring-2 ring-amber/70",
            flash && "ring-4 ring-teal",
          )}
          onContextMenu={(e) => { e.preventDefault(); onMenu(true); }}
          onPointerDown={(e) => { if (e.pointerType !== "mouse") press.current = window.setTimeout(() => onMenu(true), LONG_PRESS_MS); }}
          onPointerUp={cancelPress}
          onPointerLeave={cancelPress}
          onPointerCancel={cancelPress}
        >
          {!mine && <div className="text-[11px] text-muted">{name}</div>}
          {fwd && !i.deleted && <div className="flex items-center gap-1 text-[11px] italic text-muted"><Forward size={12} aria-hidden />{fwd}</div>}
          {i.reply && !i.deleted && (
            <button type="button" className="mb-1 block w-full rounded-lg border-l-2 border-amber bg-black/20 px-2 py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-amber" onClick={() => onQuote(i.reply!.msg_uuid)}>
              {i.reply.state === "present" && <span className="block text-[11px] font-semibold text-amber">{i.reply.sender === me ? t("chat.you") : nameOf(i.reply.sender)}</span>}
              <span className={cn("block truncate text-xs", i.reply.state === "present" ? "text-ink/80" : "italic text-muted")}>{quoteText(i.reply)}</span>
            </button>
          )}
          {i.deleted || !i.body ? (
            <div className="flex items-center gap-1.5 text-[15px] italic text-muted"><Ban size={14} aria-hidden />{t("chat.deleted")}</div>
          ) : (
            <div className="whitespace-pre-wrap break-words text-[15px]">{previewOf(i, name)}</div>
          )}
          <div className="mt-0.5 flex items-center gap-1.5 font-mono text-[10px] text-muted">
            <span>{time(i.sent_ms)}</span>
            {i.edited > 0 && !i.deleted && (
              <button type="button" className="underline decoration-dotted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-amber" onClick={onHistory} aria-label={`${i.edited > 1 ? t("chat.editedTimes", { n: i.edited }) : t("chat.edited")} · ${t("chat.editHistory")}`}>
                {i.edited > 1 ? t("chat.editedTimes", { n: i.edited }) : t("chat.edited")}
              </button>
            )}
            {mine && <span aria-label={i.acked > 0 ? t("chat.delivered") : t("chat.sent")}>{i.acked > 0 ? "✓✓" : "✓"}</span>}
          </div>
        </div>
        {!i.deleted && (
          <button
            id={`${domId(i.msg_uuid)}-more`}
            type="button"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-label={t("chat.actionsFor", { name })}
            className={cn("rounded-full p-1.5 text-muted outline-none hover:text-ink focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-amber group-hover:opacity-100", menuOpen ? "opacity-100" : "opacity-0")}
            onClick={() => onMenu(!menuOpen)}
          >
            <MoreHorizontal size={16} />
          </button>
        )}
      </div>

      {i.reactions.length > 0 && (
        <div className={cn("flex flex-wrap gap-1", mine ? "mr-9" : "ml-9")}>
          {i.reactions.map((r) => (
            <button key={r.emoji} type="button" aria-pressed={r.mine} aria-label={reactionLabel(r)} disabled={i.deleted} onClick={() => onReact(r.emoji)}
              className={cn("rounded-full border px-2 py-0.5 text-xs outline-none focus-visible:ring-2 focus-visible:ring-amber", r.mine ? "border-amber bg-amber/20" : "border-outline bg-elevated")}>
              {r.emoji} <span className="font-mono">{r.count}</span>
            </button>
          ))}
        </div>
      )}

      {menuOpen && <ActionsMenu mine={mine} onReact={onReact} actions={actions} onClose={() => onMenu(false)} />}
    </div>
  );
}

function ActionsMenu({ mine, onReact, actions, onClose }: { mine: boolean; onReact: (emoji: string) => void; actions: RowActions; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [custom, setCustom] = useState("");
  useEffect(() => { ref.current?.querySelector<HTMLElement>("[role=menuitem]")?.focus(); }, []);
  // close on outside pointer
  useEffect(() => {
    const away = (e: PointerEvent) => {
      const el = e.target instanceof Element ? e.target : null;
      if (ref.current && el && !ref.current.contains(el) && !el.closest("[aria-haspopup=menu]")) onClose();
    };
    window.addEventListener("pointerdown", away);
    return () => window.removeEventListener("pointerdown", away);
  }, [onClose]);
  const item = "flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm outline-none hover:bg-white/5 focus-visible:bg-white/10";
  const addCustom = () => { const v = custom.trim(); if (v) { onReact(v); setCustom(""); } };
  return (
    <div ref={ref} role="menu" tabIndex={-1} aria-label={t("chat.actions")} onKeyDown={menuKeys} className={cn("z-20 w-64 overflow-hidden rounded-2xl bg-elevated shadow-2xl", mine ? "mr-9" : "ml-9")}>
      <div className="flex justify-between px-2 pt-2">
        {QUICK_REACTIONS.map((e) => (
          <button key={e} type="button" role="menuitem" aria-label={t("chat.reactWith", { emoji: e })} className="rounded-full p-1.5 text-xl outline-none hover:bg-white/10 focus-visible:ring-2 focus-visible:ring-amber" onClick={() => onReact(e)}>{e}</button>
        ))}
      </div>
      <div className="flex items-center gap-2 px-3 py-2">
        <input className="min-w-0 flex-1 rounded-full border border-outline bg-graphite px-3 py-1.5 text-sm outline-none focus:border-amber" placeholder={t("chat.reactOther")} aria-label={t("chat.reactOther")} value={custom} maxLength={16} onChange={(e) => setCustom(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addCustom(); } }} />
        <button type="button" className="rounded-full p-1.5 text-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-amber disabled:opacity-40" disabled={!custom.trim()} onClick={addCustom} aria-label={t("chat.reactAdd")}><SmilePlus size={18} /></button>
      </div>
      <div className="border-t border-outline py-1">
        <button type="button" role="menuitem" className={item} onClick={actions.reply}><CornerUpLeft size={16} aria-hidden />{t("chat.reply")}</button>
        {actions.edit && <button type="button" role="menuitem" className={item} onClick={actions.edit}><Pencil size={16} aria-hidden />{t("chat.edit")}</button>}
        {actions.forward && <button type="button" role="menuitem" className={item} onClick={actions.forward}><Forward size={16} aria-hidden />{t("chat.forward")}</button>}
        {actions.copy && <button type="button" role="menuitem" className={item} onClick={actions.copy}><Copy size={16} aria-hidden />{t("chat.copy")}</button>}
        {actions.del && <button type="button" role="menuitem" className={cn(item, "text-danger")} onClick={actions.del}><Trash2 size={16} aria-hidden />{t("chat.delete")}</button>}
      </div>
    </div>
  );
}

function SearchBox({ gid, onClose, onPick }: { gid: string; onClose: () => void; onPick: (i: ChatItem) => void }) {
  const [q, setQ] = useState("");
  const [found, setFound] = useState<{ q: string; hits: ChatItem[] } | null>(null);
  const query = q.trim();
  /** results only for the current query (stale ones are hidden, never shown under a new query) */
  const hits = query && found?.q === query ? found.hits : null;
  useEffect(() => {
    if (!query) return undefined;
    let live = true;
    const tm = setTimeout(() => void host.search(gid, query).then((r) => { if (live) setFound({ q: query, hits: r }); }), SEARCH_DEBOUNCE_MS);
    return () => { live = false; clearTimeout(tm); };
  }, [gid, query]);
  return (
    <div className="relative flex flex-1 items-center gap-1">
      <Search size={16} className="absolute left-3 text-muted" aria-hidden />
      <input ref={focusEl} type="search" className="flex-1 rounded-full border border-outline bg-graphite py-2 pl-9 pr-3 text-sm outline-none focus:border-amber" placeholder={t("chat.searchPlaceholder")} aria-label={t("chat.search")} value={q} onChange={(e) => setQ(e.target.value)} />
      <button type="button" className="rounded-full p-2 text-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-amber" onClick={onClose} aria-label={t("chat.searchClose")}><X size={18} /></button>
      {hits && (
        <ul className="absolute left-0 right-0 top-full z-30 mt-2 max-h-72 overflow-y-auto rounded-2xl bg-elevated shadow-2xl" aria-label={t("chat.search")}>
          {hits.length === 0 && <li className="px-4 py-3 text-sm text-muted">{t("chat.searchNone")}</li>}
          {hits.map((h) => (
            <li key={h.msg_uuid}>
              <button type="button" className="block w-full px-4 py-2.5 text-left outline-none hover:bg-white/5 focus-visible:bg-white/10" onClick={() => onPick(h)}>
                <span className="flex justify-between text-[11px] text-muted"><b>{nameOf(h.from)}</b><span className="font-mono">{time(h.sent_ms)}</span></span>
                <span className="block truncate text-sm">{previewOf(h, nameOf(h.from))}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
