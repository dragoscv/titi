"use client";
import { ArrowLeft, Check } from "lucide-react";
import { host } from "@/lib/engine";
import { DEFAULT_RELAY, useStore } from "@/lib/store";
import { hueColor } from "@/lib/cn";
import type { Route } from "../Shell";

const HUES = [40, 15, 350, 290, 220, 170, 120, 80];

export function Settings({ go }: { go: (r: Route) => void }) {
  const { settings, nodeId } = useStore();
  return (
    <div className="flex flex-1 flex-col">
      <header className="flex items-center gap-1 px-2 pt-2">
        <button className="p-3 text-muted hover:text-ink" onClick={() => go({ name: "home" })} aria-label="Back"><ArrowLeft /></button>
        <h1 className="text-xl font-semibold">Settings</h1>
      </header>
      <div className="flex flex-col gap-3 p-5">
        <Section title="Profile">
          <label htmlFor="settings-name" className="text-sm text-muted">Name</label>
          <input id="settings-name" className="mt-1 w-full rounded-xl border border-outline bg-graphite px-3.5 py-3 outline-none focus:border-amber" value={settings.name} onChange={(e) => host.saveSettings({ name: e.target.value })} maxLength={24} />
          <div className="mt-3 text-sm text-muted">Colour</div>
          <div className="mt-2 flex gap-2.5">
            {HUES.map((h) => (
              <button key={h} className="grid h-9 w-9 place-items-center rounded-full" style={{ background: hueColor(h), outline: h === settings.hue ? "3px solid var(--color-ink)" : "none" }} onClick={() => host.saveSettings({ hue: h })} aria-label={`Colour ${h}`}>
                {h === settings.hue && <Check size={16} className="text-graphite" />}
              </button>
            ))}
          </div>
        </Section>
        <Section title="Connection">
          <label htmlFor="settings-relay" className="text-sm text-muted">Relay URL</label>
          <input id="settings-relay" className="mt-1 w-full rounded-xl border border-outline bg-graphite px-3.5 py-3 font-mono text-xs outline-none focus:border-amber" value={settings.relayUrl} onChange={(e) => host.saveSettings({ relayUrl: e.target.value })} />
          <button className="mt-2 text-xs text-muted underline" onClick={() => host.saveSettings({ relayUrl: DEFAULT_RELAY })}>Reset to default</button>
        </Section>
        <Section title="About">
          <div className="text-sm">Titi web 0.1.0</div>
          <div className="mt-1 text-xs text-muted">Device ID</div>
          <div className="font-mono text-xs">{nodeId}</div>
          <p className="mt-3 text-xs text-muted">Voice and messages are end-to-end encrypted with the group key. The relay only sees encrypted frames and a 4-byte group hash.</p>
        </Section>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-[22px] bg-surface p-[18px]">
      <h2 className="mb-3 text-sm font-semibold text-amber">{title}</h2>
      {children}
    </section>
  );
}
