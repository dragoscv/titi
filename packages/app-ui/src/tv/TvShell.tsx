// 10-foot UI for TVs / smart monitors (Tizen, and any `tenFoot` host).
// Remote model: ↑/↓ move between groups, ←/→ between panes, OK hold = talk
// (keydown → floor, keyup → release, watchdog if keyup is lost), Back = up a
// level / exit. Everything is D-pad reachable; no pointer, no text entry
// needed (join by code shown on-screen; phone scans the QR).
import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { AudioLines, Mic, Radio, Users, MessageSquare, Plus, QrCode } from "lucide-react";
import QRCode from "qrcode";
import { host } from "../platform";
import { useStore, type GroupState } from "../store";
import { Avatar } from "../Avatar";
import { hueColor } from "../cn";

const KEY = { LEFT: 37, UP: 38, RIGHT: 39, DOWN: 40, OK: 13, BACK: 10009, ESC: 27, PLAY_PAUSE: 10252, RED: 403 } as const;
/** Some remotes stop repeating without a keyup; release after this gap. */
const HOLD_WATCHDOG_MS = 650;
const MAX_TALK_MS = 60_000;

type Pane = "groups" | "actions";

export function TvShell() {
  const ready = useStore((s) => s.ready);
  const settings = useStore((s) => s.settings);
  const groups = useStore((s) => s.groups);
  const activeId = useStore((s) => s.activeGroup);
  const toast = useStore((s) => s.toast);
  const invites = useStore((s) => s.invites);
  const [focus, setFocus] = useState(0);
  const [pane, setPane] = useState<Pane>("groups");
  const [action, setAction] = useState(0);
  const [sheet, setSheet] = useState<"none" | "invite" | "create" | "join">("none");
  const talking = useRef(false);
  const lastRepeat = useRef(0);
  const talkStart = useRef(0);
  /** true once this remote has shown it auto-repeats while held (then a repeat gap = released) */
  const repeats = useRef(false);

  useEffect(() => { void host.init().then(() => host.start()); }, []);
  useEffect(() => {
    if (!ready || settings.onboarded) return;
    // TVs have no comfortable keyboard: name the device after the room once
    host.saveSettings({ name: "Living room TV", hue: 200, onboarded: true });
  }, [ready, settings.onboarded]);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => useStore.getState().set({ toast: null }), 3500);
    return () => clearTimeout(t);
  }, [toast]);

  const sel = groups[Math.min(focus, Math.max(0, groups.length - 1))];
  useEffect(() => { if (sel && sel.id !== activeId) host.setActiveGroup(sel.id); }, [sel, activeId]);

  const down = useCallback(() => { if (talking.current) return; talking.current = true; talkStart.current = Date.now(); host.pttDown(); }, []);
  const up = useCallback(() => { if (!talking.current) return; talking.current = false; host.pttUp(); }, []);

  // keyup releases; if a repeating remote goes quiet without keyup, release; hard cap 60 s
  useEffect(() => {
    const t = setInterval(() => {
      if (!talking.current) return;
      const now = Date.now();
      if ((repeats.current && now - lastRepeat.current > HOLD_WATCHDOG_MS) || now - talkStart.current > MAX_TALK_MS) up();
    }, 200);
    return () => clearInterval(t);
  }, [up]);

  const actions = sel ? [
    { id: "talk", label: sel.fullDuplex ? "Open mic" : "Hold OK to talk", icon: <Mic size={34} /> },
    { id: "invite", label: "Invite", icon: <QrCode size={34} /> },
    { id: "mode", label: sel.fullDuplex ? "Push to talk" : "Open mic", icon: <Radio size={34} /> },
  ] : [];

  useEffect(() => {
    const onDown = (e: KeyboardEvent) => {
      if (sheet !== "none") {
        if (e.keyCode === KEY.BACK || e.keyCode === KEY.ESC) { e.preventDefault(); setSheet("none"); }
        return;
      }
      const k = e.keyCode;
      if (invites[0] && k === KEY.OK) { e.preventDefault(); host.acceptInvite(invites[0].group, invites[0].host); return; }
      if (invites[0] && k === KEY.BACK) { e.preventDefault(); host.declineInvite(invites[0].group, invites[0].host); return; }
      const isTalkKey = k === KEY.PLAY_PAUSE || k === KEY.RED || (k === KEY.OK && pane === "actions" && action === 0) || (k === KEY.OK && pane === "groups" && groups.length > 0);
      if (isTalkKey && sel) {
        e.preventDefault();
        if (sel.fullDuplex) { if (!e.repeat) host.setMuted(!useStore.getState().muted); return; }
        lastRepeat.current = Date.now();
        if (e.repeat) repeats.current = true;
        else down();
        return;
      }
      switch (k) {
        case KEY.UP: e.preventDefault(); if (pane === "groups") setFocus((f) => Math.max(0, f - 1)); else setAction((a) => Math.max(0, a - 1)); break;
        case KEY.DOWN: e.preventDefault(); if (pane === "groups") setFocus((f) => Math.min(groups.length, f + 1)); else setAction((a) => Math.min(actions.length - 1, a + 1)); break;
        case KEY.RIGHT: e.preventDefault(); if (sel) setPane("actions"); break;
        case KEY.LEFT: e.preventDefault(); setPane("groups"); break;
        case KEY.OK:
          e.preventDefault();
          if (pane === "groups" && focus >= groups.length) setSheet("join");
          else if (pane === "actions" && sel) {
            const id = actions[action]?.id;
            if (id === "invite") setSheet("invite");
            if (id === "mode") host.setFullDuplex(sel.id, !sel.fullDuplex);
          }
          break;
        case KEY.BACK:
          if (pane === "actions") { e.preventDefault(); setPane("groups"); }
          else { try { (window as unknown as { tizen?: { application: { getCurrentApplication(): { exit(): void } } } }).tizen?.application.getCurrentApplication().exit(); } catch { /* not tizen */ } }
          break;
      }
    };
    const onUp = (e: KeyboardEvent) => {
      if (e.keyCode === KEY.OK || e.keyCode === KEY.PLAY_PAUSE || e.keyCode === KEY.RED) up();
    };
    window.addEventListener("keydown", onDown);
    window.addEventListener("keyup", onUp);
    return () => { window.removeEventListener("keydown", onDown); window.removeEventListener("keyup", onUp); };
  }, [sheet, pane, action, actions, groups.length, sel, focus, invites, down, up]);

  if (!ready) return <div className="grid h-[1080px] place-items-center text-4xl text-muted">Titi</div>;

  return (
    <div className="tv relative flex h-[1080px] w-[1920px] overflow-hidden bg-graphite text-ink">
      {/* rail */}
      <aside className="flex w-[520px] shrink-0 flex-col gap-4 py-[56px] pl-[80px] pr-8">
        <div className="mb-6 flex items-center gap-4">
          <span className="grid h-16 w-16 place-items-center rounded-[22px] bg-amber text-graphite"><Mic size={34} /></span>
          <div>
            <div className="text-[40px] font-extrabold leading-none">Titi</div>
            <div className="mt-1 text-xl text-muted">{settings.name}</div>
          </div>
        </div>
        {groups.map((g, i) => <GroupRow key={g.id} g={g} focused={pane === "groups" && focus === i} selected={sel?.id === g.id} />)}
        <div className={`flex items-center gap-4 rounded-[26px] px-6 py-5 text-2xl transition-transform duration-150 ${pane === "groups" && focus >= groups.length ? "scale-[1.04] bg-elevated ring-4 ring-amber" : "bg-surface text-muted"}`}>
          <Plus size={32} /> Join a group
        </div>
        <div className="mt-auto text-lg text-muted">↑↓ choose · → actions · hold OK to talk · Back exits</div>
      </aside>

      {/* stage */}
      <main className="relative flex flex-1 flex-col py-[56px] pr-[80px]">
        {sel ? <Stage g={sel} pane={pane} action={action} actions={actions} /> : <Empty />}
      </main>

      <AnimatePresence>
        {sheet !== "none" && (
          <motion.div className="absolute inset-0 z-40 grid place-items-center bg-black/70" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
            <motion.div className="w-[900px] rounded-[40px] bg-surface p-14" initial={{ scale: 0.92, y: 30 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, opacity: 0 }} transition={{ type: "spring", stiffness: 320, damping: 30 }}>
              {sheet === "invite" && sel && <InvitePanel gid={sel.id} name={sel.name} />}
              {sheet === "join" && <JoinPanel />}
              <div className="mt-10 text-center text-xl text-muted">Press Back to close</div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {invites[0] && (
          <motion.div key={invites[0].group} className="absolute left-1/2 top-12 z-50 flex -translate-x-1/2 items-center gap-6 rounded-[32px] bg-elevated px-10 py-7 shadow-2xl" initial={{ y: -120, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: -120, opacity: 0 }}>
            <Avatar name={invites[0].hostName} hue={200} size={72} />
            <div>
              <div className="text-3xl font-bold">{invites[0].hostName} invites this TV</div>
              <div className="text-2xl text-muted">to “{invites[0].name}” · OK to join · Back to ignore</div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {toast && (
          <motion.output key={toast} className="absolute bottom-12 left-1/2 z-50 -translate-x-1/2 rounded-full bg-elevated px-8 py-4 text-2xl shadow-xl" initial={{ y: 40, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: 40, opacity: 0 }}>{toast}</motion.output>
        )}
      </AnimatePresence>
    </div>
  );
}

function GroupRow({ g, focused, selected }: { g: GroupState; focused: boolean; selected: boolean }) {
  const live = g.floor === "busy" || g.floor === "talking";
  return (
    <div className={`flex items-center gap-5 rounded-[26px] px-6 py-5 transition-transform duration-150 ${focused ? "scale-[1.04] bg-elevated ring-4 ring-amber" : selected ? "bg-elevated" : "bg-surface"}`}>
      <span className={`grid h-16 w-16 shrink-0 place-items-center rounded-[20px] text-3xl font-bold ${live ? "bg-teal text-graphite" : "bg-amber/20 text-amber"}`}>{g.name[0]?.toUpperCase()}</span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[28px] font-semibold">{g.name}</span>
        <span className={`block truncate text-xl ${live ? "text-teal" : "text-muted"}`}>{live && g.talkerName ? `${g.talkerName} is talking` : `${g.memberCount} members`}</span>
      </span>
      {g.unread > 0 && <span className="rounded-full bg-amber px-3 py-1 text-xl font-bold text-graphite">{g.unread}</span>}
    </div>
  );
}

function Stage({ g, pane, action, actions }: { g: GroupState; pane: Pane; action: number; actions: { id: string; label: string; icon: React.ReactNode }[] }) {
  const nodeId = useStore((s) => s.nodeId);
  const level = useStore((s) => s.levelDbfs);
  const muted = useStore((s) => s.muted);
  const msgs = useStore((s) => s.messages);
  const recent = msgs.filter((m) => m.group === g.id).slice(-4);
  const talker = g.members.find((m) => m.node === g.talkerNode);
  const me = g.floor === "talking";
  const lv = Math.min(1, Math.max(0, (level + 50) / 50));
  return (
    <>
      <header className="flex items-end justify-between">
        <div>
          <div className="text-xl font-semibold uppercase tracking-widest text-muted">Channel</div>
          <h1 className="text-[64px] font-extrabold leading-tight">{g.name}</h1>
        </div>
        <div className="flex items-center gap-3 rounded-full bg-surface px-6 py-3 text-2xl text-muted"><Users size={28} /> {g.memberCount} · {g.link ?? "connecting…"}</div>
      </header>

      <div className="relative mt-6 flex flex-1 items-center justify-center rounded-[48px] bg-surface">
        <AnimatePresence mode="wait">
          {g.floor === "busy" || me ? (
            <motion.div key={"t" + (g.talkerNode ?? "")} className="flex flex-col items-center" initial={{ opacity: 0, scale: 0.9 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0, scale: 1.05 }} transition={{ type: "spring", stiffness: 260, damping: 26 }}>
              <div className="relative">
                <motion.span className="absolute inset-[-28px] rounded-full" style={{ background: me ? "var(--color-amber)" : hueColor(talker?.hue ?? 170), opacity: 0.18 }} animate={{ scale: 1 + lv * 0.35 }} transition={{ type: "spring", stiffness: 300, damping: 20 }} />
                <Avatar name={me ? "You" : g.talkerName ?? "?"} hue={me ? 40 : talker?.hue ?? 170} size={260} />
              </div>
              <div className={`mt-12 flex items-center gap-4 text-[56px] font-bold ${me ? "text-amber" : "text-teal"}`}><AudioLines size={56} /> {me ? "You are talking" : `${g.talkerName} is talking`}</div>
            </motion.div>
          ) : (
            <motion.div key="idle" className="flex flex-col items-center text-muted" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
              <div className="flex -space-x-6">{g.members.slice(0, 6).map((m) => <Avatar key={m.node} name={m.node === nodeId ? "TV" : m.name} hue={m.hue} size={140} className="ring-8 ring-surface" />)}</div>
              <div className="mt-12 text-[44px] font-semibold">{g.fullDuplex ? (muted ? "Open mic · muted" : "Open mic · live") : "Channel free"}</div>
            </motion.div>
          )}
        </AnimatePresence>
        {recent.length > 0 && (
          <div className="absolute bottom-8 left-8 flex max-w-[640px] flex-col gap-3">
            {recent.map((m) => (
              <div key={m.id} className="flex items-center gap-3 rounded-[22px] bg-elevated px-5 py-3 text-2xl"><MessageSquare size={24} className="text-muted" /><b>{m.fromName}</b><span className="truncate">{m.body.kind === "text" ? m.body.text : m.body.kind === "sos" ? "SOS" : m.body.kind}</span></div>
            ))}
          </div>
        )}
      </div>

      <div className="mt-8 flex gap-6">
        {actions.map((a, i) => (
          <div key={a.id} className={`flex flex-1 items-center justify-center gap-4 rounded-[28px] py-7 text-3xl font-semibold transition-transform duration-150 ${pane === "actions" && action === i ? "scale-[1.05] ring-4 ring-amber" : ""} ${i === 0 ? (me ? "bg-amber text-graphite" : "bg-amber/20 text-amber") : "bg-elevated"}`}>
            {a.icon} {a.label}
          </div>
        ))}
      </div>
    </>
  );
}

function Empty() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center rounded-[48px] bg-surface text-center">
      <Radio size={96} className="text-amber" />
      <h1 className="mt-8 text-[56px] font-extrabold">Turn this screen into a room radio</h1>
      <p className="mt-4 max-w-[900px] text-3xl text-muted">On your phone open Titi → a group → Invite, then choose “Join a group” here and type the code, or invite this TV from the nearby list.</p>
    </div>
  );
}

function InvitePanel({ gid, name }: { gid: string; name: string }) {
  const [code, setCode] = useState<[string, number] | null>(null);
  const [qr, setQr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const poll = () => void host.currentCode(gid).then((c) => { if (live) setCode(c); });
    poll();
    const t = setInterval(poll, 1000);
    void host.deepLink(gid).then(async (l) => { if (l && live) setQr(await QRCode.toDataURL(l.replace("titi://j/", "https://titi.app/j/"), { margin: 1, width: 560 })); });
    return () => { live = false; clearInterval(t); };
  }, [gid]);
  return (
    <div className="flex items-center gap-12">
      {/* oxlint-disable-next-line next/no-img-element -- not a Next app (Tizen .wgt); QR is a generated data: URL */}
      {qr ? <img src={qr} alt="" className="h-[340px] w-[340px] rounded-[28px] bg-white p-3" /> : <div className="h-[340px] w-[340px] rounded-[28px] bg-elevated" />}
      <div>
        <div className="text-2xl text-muted">Join “{name}”</div>
        <div className="mt-3 font-mono text-[56px] font-medium text-amber">{code?.[0].replaceAll("-", " ") ?? "…"}</div>
        <div className="mt-2 text-2xl text-muted">Scan with a phone camera or say the code · changes in {Math.floor((code?.[1] ?? 0) / 60)}:{String((code?.[1] ?? 0) % 60).padStart(2, "0")}</div>
      </div>
    </div>
  );
}

/** Code entry with the remote: pick 3 words from the wordlist via on-screen field + TV keyboard, 2 digits. */
function JoinPanel() {
  const [text, setText] = useState("");
  const [ok, setOk] = useState(false);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => { ref.current?.focus(); }, []);
  const norm = text.trim().toLowerCase().replace(/\s+/g, "-");
  useEffect(() => { let l = true; void host.parseCode(norm).then((v) => { if (l) setOk(v); }); return () => { l = false; }; }, [norm]);
  return (
    <div>
      <h2 className="text-[44px] font-bold">Join with a code</h2>
      <p className="mt-2 text-2xl text-muted">Three words and two digits from the host’s screen.</p>
      <input
        ref={ref}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.keyCode === 13 && ok) { host.joinByCode(norm); useStore.getState().set({ toast: "Looking for the group…" }); } }}
        className="mt-8 w-full rounded-[24px] border-4 border-outline bg-graphite px-8 py-6 font-mono text-5xl outline-none focus:border-amber"
        placeholder="word word word 00"
        aria-label="Invite code"
      />
      <div className={`mt-4 text-2xl ${ok ? "text-teal" : "text-muted"}`}>{ok ? "Press OK to join" : "Type the code with the on-screen keyboard"}</div>
    </div>
  );
}
