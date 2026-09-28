"use client";
import { useState } from "react";
import { motion } from "motion/react";
import { LogIn, Plus, Settings as SettingsIcon, Wifi, Cloud, Bluetooth } from "lucide-react";
import { host } from "@/lib/engine";
import { useStore } from "@/lib/store";
import { Avatar } from "../Avatar";
import { Sheet, focusOnMount } from "../Sheet";
import type { Route } from "../Shell";
import type { LinkClass } from "@/lib/types";

export const linkIcon = (l: LinkClass | null, size = 16) =>
  l === "internet" || l === "webRtc" ? <Cloud size={size} /> : l?.startsWith("ble") || l === "btRfcomm" ? <Bluetooth size={size} /> : <Wifi size={size} />;

export function Home({ go }: { go: (r: Route) => void }) {
  const { settings, groups, activeGroup, peers, running } = useStore();
  const [create, setCreate] = useState(false);
  const [name, setName] = useState("");
  const peerList = Object.values(peers);

  return (
    <div className="flex flex-1 flex-col gap-3 px-5 pb-28 pt-12">
      <header className="flex items-center gap-3">
        <Avatar name={settings.name || "T"} hue={settings.hue} />
        <div className="flex-1">
          <div className="font-semibold">{settings.name}</div>
          <div className="text-xs text-muted">{running ? `Radio on · ${peerList.length} nearby` : "Tap a group to switch the radio on"}</div>
        </div>
        <button className="rounded-full p-2 text-muted hover:text-ink" onClick={() => go({ name: "settings" })} aria-label="Settings"><SettingsIcon size={22} /></button>
      </header>

      <section className="mt-4">
        <h2 className="text-sm font-semibold text-muted">Nearby</h2>
        <div className="mt-2.5 rounded-[22px] bg-surface p-4">
          {peerList.length === 0 ? (
            <div className="flex items-center gap-3.5 text-sm text-muted">
              <Radar />
              {running ? "Looking for others in your room…" : "Web finds people through your groups (internet) or a phone running Titi on the same Wi‑Fi."}
            </div>
          ) : (
            <div className="flex gap-4 overflow-x-auto">
              {peerList.map((p) => (
                <div key={p.node} className="flex flex-col items-center gap-1.5 text-xs">
                  <Avatar name={p.name} hue={p.hue} size={52} />
                  <span className="max-w-16 truncate">{p.name}</span>
                  <span className={p.inGroup ? "text-teal" : "text-muted"}>{linkIcon(p.link, 14)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </section>

      <section className="mt-3">
        <h2 className="text-sm font-semibold text-muted">Your groups</h2>
        <div className="mt-2.5 flex flex-col gap-3">
          {groups.length === 0 && (
            <div className="rounded-[22px] bg-surface p-6 text-center">
              <div className="font-semibold">No groups yet</div>
              <div className="mt-1 text-sm text-muted">Create one, or join with a code from a friend.</div>
            </div>
          )}
          {groups.map((g) => {
            const talking = g.floor === "busy" || g.floor === "talking";
            return (
              <motion.button key={g.id} layoutId={`group-${g.id}`} whileTap={{ scale: 0.98 }} onClick={() => { void host.start(); host.setActiveGroup(g.id); go({ name: "group", id: g.id }); }} className={`flex items-center gap-3.5 rounded-[22px] p-[18px] text-left ${g.id === activeGroup ? "bg-elevated" : "bg-surface"}`}>
                <span className={`grid h-12 w-12 place-items-center rounded-2xl text-xl font-bold ${talking ? "bg-teal text-graphite" : "bg-amber/20 text-amber"}`}>{g.name[0]?.toUpperCase()}</span>
                <span className="flex-1 min-w-0">
                  <span className="block font-semibold">{g.name}</span>
                  <span className={`block text-xs ${talking ? "text-teal" : "text-muted"}`}>{talking && g.talkerName ? `${g.talkerName} is talking` : `${g.memberCount} member${g.memberCount === 1 ? "" : "s"}`}</span>
                </span>
                {g.unread > 0 && <span className="rounded-full bg-amber px-2 py-0.5 text-xs font-bold text-graphite">{g.unread}</span>}
                <span className="text-muted">{linkIcon(g.link)}</span>
              </motion.button>
            );
          })}
        </div>
      </section>

      <div className="fixed inset-x-0 bottom-0 mx-auto flex max-w-md gap-3 bg-gradient-to-t from-graphite via-graphite/95 to-transparent p-5 pt-8">
        <button className="flex h-14 flex-1 items-center justify-center gap-2 rounded-[18px] bg-amber font-semibold text-graphite" onClick={() => setCreate(true)}><Plus size={20} /> New group</button>
        <button className="flex h-14 flex-1 items-center justify-center gap-2 rounded-[18px] border border-outline font-semibold" onClick={() => go({ name: "join" })}><LogIn size={20} /> Join</button>
      </div>

      <Sheet open={create} onClose={() => setCreate(false)} title="Name your group">
        <input className="w-full rounded-2xl border border-outline bg-graphite px-4 py-3.5 text-lg outline-none focus:border-amber" placeholder="e.g. Cabana, Trail team, Ski lift" value={name} onChange={(e) => setName(e.target.value)} ref={focusOnMount} maxLength={32} />
        <button disabled={name.trim().length < 2} className="mt-4 h-14 w-full rounded-[18px] bg-amber font-semibold text-graphite disabled:opacity-40" onClick={() => { void host.start(); host.createGroup(name.trim()); setName(""); setCreate(false); }}>Create</button>
      </Sheet>
    </div>
  );
}

function Radar() {
  return (
    <span className="relative grid h-14 w-14 shrink-0 place-items-center">
      {[1, 2, 3].map((k) => <span key={k} className="absolute rounded-full border border-outline" style={{ width: `${k * 33}%`, height: `${k * 33}%` }} />)}
      <motion.span className="absolute left-1/2 top-1/2 h-1/2 w-[2px] origin-top bg-teal" animate={{ rotate: 360 }} transition={{ duration: 3.2, repeat: Infinity, ease: "linear" }} />
    </span>
  );
}
