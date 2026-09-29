"use client";
import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowLeft, MessageSquare, UserPlus, MoreVertical, Send, Share2 } from "lucide-react";
import QRCode from "qrcode";
import { host } from "../platform";
import { toWebLink } from "../links";
import { useStore } from "../store";
import { Avatar, SignalBars } from "../Avatar";
import { TalkButton } from "../TalkButton";
import { Sheet } from "../Sheet";
import { linkIcon } from "./Home";
import type { Route } from "../Shell";

export function Group({ id, go }: { id: string; go: (r: Route) => void }) {
  const g = useStore((s) => s.groups.find((x) => x.id === id));
  const { peers, nodeId, levelDbfs, muted, messages } = useStore();
  const [invite, setInvite] = useState(false);
  const [chat, setChat] = useState(false);
  const [menu, setMenu] = useState(false);

  useEffect(() => { if (!g) go({ name: "home" }); }, [g, go]);
  useEffect(() => { void host.start(); host.setActiveGroup(id); }, [id]);
  if (!g) return null;

  const inGroupPeers = Object.values(peers).filter((p) => p.inGroup);
  const bars = Math.max(0, ...inGroupPeers.map((p) => p.bars));
  const hops = Math.max(0, ...inGroupPeers.map((p) => p.hops));
  const msgs = messages.filter((m) => m.group === id);

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex items-center gap-1 px-2 pt-2">
        <button className="p-3 text-muted hover:text-ink" onClick={() => go({ name: "home" })} aria-label="Back"><ArrowLeft /></button>
        <motion.div layoutId={`group-${g.id}`} className="flex-1 min-w-0">
          <h1 className="truncate text-xl font-semibold">{g.name}</h1>
          <div className="flex items-center gap-2 text-xs text-muted">
            <span>{g.memberCount} member{g.memberCount === 1 ? "" : "s"}</span>
            <span className="inline-flex items-center gap-1.5 rounded-full border border-outline px-2.5 py-1">
              {linkIcon(g.link, 13)}<span className="capitalize">{g.link ?? "searching…"}</span><SignalBars bars={bars} />{hops > 1 && <span>· {hops} hops</span>}
            </span>
          </div>
        </motion.div>
        <button className="relative p-3 text-muted hover:text-ink" onClick={() => setChat(true)} aria-label="Chat">
          <MessageSquare />
          {g.unread > 0 && <span className="absolute right-1.5 top-1.5 rounded-full bg-amber px-1.5 text-[10px] font-bold text-graphite">{g.unread}</span>}
        </button>
        <button className="p-3 text-muted hover:text-ink" onClick={() => setInvite(true)} aria-label="Invite"><UserPlus /></button>
        <div className="relative">
          <button className="p-3 text-muted hover:text-ink" onClick={() => setMenu((m) => !m)} aria-label="More"><MoreVertical /></button>
          {menu && (
            <div className="absolute right-2 top-12 z-30 w-56 overflow-hidden rounded-2xl bg-elevated shadow-2xl">
              <button className="block w-full px-4 py-3 text-left text-sm text-danger hover:bg-white/5" onClick={() => { setMenu(false); if (confirm(`Leave ${g.name}?`)) { host.leaveGroup(g.id); go({ name: "home" }); } }}>Leave group</button>
              {g.isCreator && <button className="block w-full px-4 py-3 text-left text-sm text-danger hover:bg-white/5" onClick={() => { setMenu(false); if (confirm(`Delete ${g.name} for everyone? Members' copies are removed too.`)) { host.dissolveGroup(g.id); go({ name: "home" }); } }}>Delete for everyone</button>}
            </div>
          )}
        </div>
      </header>

      <AnimatePresence>
        {(g.suspended || g.handover !== "stable") && (
          <motion.div className="mx-5 mt-2 rounded-xl bg-elevated px-3 py-2 text-center text-xs font-medium" initial={{ y: -10, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: -10, opacity: 0 }}>
            {g.suspended ? "Out of range — will resume" : g.handover === "switching" ? `Switching to ${g.link ?? ""}` : "Link weak — switching"}
          </motion.div>
        )}
      </AnimatePresence>

      <div className="mt-4 flex gap-4 overflow-x-auto px-5">
        {g.members.map((m) => {
          const p = peers[m.node];
          return (
            <div key={m.node} className="flex flex-col items-center gap-1.5">
              <Avatar name={m.name} hue={m.hue} size={56} talking={g.talkerNode === m.node} />
              <span className="max-w-16 truncate text-xs">{m.node === nodeId ? "You" : m.name}</span>
              {p && m.node !== nodeId && <span className="font-mono text-[11px] text-muted">{p.hops > 1 ? `${p.hops} hops` : p.link}</span>}
            </div>
          );
        })}
      </div>

      <div className="flex-1" />

      <AnimatePresence mode="wait">
        <motion.p key={g.floor + (g.talkerName ?? "")} className={`text-center text-base font-semibold ${g.floor === "talking" ? "text-amber" : g.floor === "busy" ? "text-teal" : "text-muted"}`} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
          {g.floor === "talking" ? "You are talking" : g.floor === "busy" ? `${g.talkerName ?? ""} is talking` : g.floor === "queued" ? "Waiting for the channel…" : "Channel free"}
        </motion.p>
      </AnimatePresence>

      <div className="mt-5 grid place-items-center px-6">
        <TalkButton floor={g.floor} fullDuplex={g.fullDuplex} muted={muted} talkerName={g.talkerName} levelDbfs={levelDbfs} onDown={() => host.pttDown()} onUp={() => host.pttUp()} onToggleMute={() => host.setMuted(!muted)} />
      </div>

      <div className="mx-6 mb-8 mt-7 flex items-center gap-3">
        <div className="flex flex-1 overflow-hidden rounded-full border border-outline text-sm font-medium">
          <button className={`flex-1 py-2.5 ${!g.fullDuplex ? "bg-amber/20 text-amber" : "text-muted"}`} onClick={() => host.setFullDuplex(g.id, false)}>Push to talk</button>
          <button className={`flex-1 py-2.5 ${g.fullDuplex ? "bg-teal/20 text-teal" : "text-muted"}`} onClick={() => host.setFullDuplex(g.id, true)}>Open mic</button>
        </div>
      </div>

      <InviteSheet open={invite} onClose={() => setInvite(false)} gid={g.id} gname={g.name} />

      <Sheet open={chat} onClose={() => setChat(false)} title={g.name}>
        <ChatPanel gid={g.id} msgs={msgs} />
      </Sheet>
    </div>
  );
}

function InviteSheet({ open, onClose, gid, gname }: { open: boolean; onClose: () => void; gid: string; gname: string }) {
  return (
    <Sheet open={open} onClose={onClose} title={`Invite to ${gname}`}>
      <InviteBody gid={gid} gname={gname} />
    </Sheet>
  );
}

/** Mounted only while the sheet is open, so the first code/link come from lazy initialisers. */
function InviteBody({ gid, gname }: { gid: string; gname: string }) {
  const peersMap = useStore((s) => s.peers);
  const peers = Object.values(peersMap).filter((p) => !p.inGroup);
  const [code, setCode] = useState<[string, number] | null>(null);
  const [qr, setQr] = useState<string | null>(null);
  const [link, setLink] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const poll = () => void host.currentCode(gid).then((c) => { if (live) setCode(c); });
    poll();
    const t = setInterval(poll, 1000);
    void host.deepLink(gid).then(async (l) => {
      if (!live || !l) return;
      setLink(l);
      const url = await QRCode.toDataURL(toWebLink(l), { margin: 1, width: 400, color: { dark: "#0E1013", light: "#FFFFFF" } });
      if (live) setQr(url);
    });
    return () => { live = false; clearInterval(t); };
  }, [gid]);
  const secs = code?.[1] ?? 0;
  const share = () => {
    const url = (link ? toWebLink(link) : "");
    if (navigator.share) void navigator.share({ title: `Join ${gname} on Titi`, url }); else void navigator.clipboard.writeText(url);
  };
  return (
    <div className="flex flex-col items-center gap-2">
        {peers.length > 0 && (
          <>
            <div className="text-sm text-muted">Tap someone in your room</div>
            <div className="flex gap-4 overflow-x-auto py-2">
              {peers.map((p) => (
                <button key={p.node} className="flex flex-col items-center gap-1.5 text-xs" onClick={() => host.invitePeer(gid, p.node)}><Avatar name={p.name} hue={p.hue} size={56} />{p.name}</button>
              ))}
            </div>
          </>
        )}
        <div className="mt-2 text-sm text-muted">Say this code</div>
        <button className="font-mono text-[28px] font-medium tracking-wide text-amber" onClick={() => { if (code) void navigator.clipboard.writeText(code[0]); }}>{code?.[0].replaceAll("-", " ") ?? "…"}</button>
        <div className="font-mono text-xs text-muted">Changes in {Math.floor(secs / 60)}:{String(secs % 60).padStart(2, "0")}</div>
        {qr && (
          <>
            <div className="mt-3 text-sm text-muted">Or scan</div>
            {/* oxlint-disable-next-line next/no-img-element -- QR is a generated data: URL; next/image adds nothing here */}
            <img src={qr} alt="Invite QR" className="h-52 w-52 rounded-2xl bg-white p-2" />
            <button className="mt-3 inline-flex items-center gap-2 rounded-xl border border-outline px-4 py-2.5 text-sm" onClick={share}><Share2 size={16} /> Share link</button>
          </>
        )}
    </div>
  );
}

function ChatPanel({ gid, msgs }: { gid: string; msgs: ReturnType<typeof useStore.getState>["messages"] }) {
  const [text, setText] = useState("");
  const peers = useStore((s) => s.peers);
  const send = () => { if (text.trim()) { host.sendText(gid, text.trim()); setText(""); } };
  return (
    <div className="flex max-h-[60dvh] flex-col">
      <div className="flex flex-1 flex-col gap-2 overflow-y-auto pb-3">
        {msgs.length === 0 && <div className="py-8 text-center text-sm text-muted">No messages yet. Texts are stored and forwarded when a path exists.</div>}
        {msgs.map((m) => (
          <div key={m.id} className={`flex items-end gap-2 ${m.mine ? "justify-end" : ""}`}>
            {!m.mine && <Avatar name={m.fromName} hue={peers[m.from]?.hue ?? 200} size={28} />}
            <div className={`max-w-[75%] rounded-[18px] px-3.5 py-2.5 ${m.mine ? "bg-amber/20" : "bg-elevated"}`}>
              {!m.mine && <div className="text-[11px] text-muted">{m.fromName}</div>}
              <div className="text-[15px]">{m.body.kind === "text" ? m.body.text : m.body.kind === "location" ? `${m.fromName} shared a location` : m.body.kind === "sos" ? (m.body.cancelled ? `${m.fromName} cancelled the SOS` : `SOS from ${m.fromName}`) : "Voice note"}</div>
              <div className="mt-0.5 font-mono text-[10px] text-muted">{new Date(m.sentMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}{m.mine && (m.acked ? " ✓✓" : " ✓")}</div>
            </div>
          </div>
        ))}
      </div>
      <div className="flex items-center gap-2 pt-2">
        <input className="flex-1 rounded-full border border-outline bg-graphite px-4 py-3 outline-none focus:border-amber" placeholder="Message" value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && send()} />
        <button className="grid h-12 w-12 place-items-center rounded-2xl bg-amber text-graphite disabled:opacity-40" disabled={!text.trim()} onClick={send} aria-label="Send"><Send size={20} /></button>
      </div>
    </div>
  );
}
