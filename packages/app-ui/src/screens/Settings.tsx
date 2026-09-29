"use client";
import { useEffect, useState } from "react";
import { ArrowLeft, Check, Power } from "lucide-react";
import { host, type AudioDevice } from "../platform";
import { DEFAULT_RELAY, useStore } from "../store";
import { hueColor } from "../cn";
import type { Route } from "../Shell";

const HUES = [40, 15, 350, 290, 220, 170, 120, 80];

export function Settings({ go }: { go: (r: Route) => void }) {
  const { settings, nodeId, platform } = useStore();
  const caps = host.caps;
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
        <Section title="Audio">
          <label htmlFor="settings-vol" className="text-sm text-muted">Speaker volume · {Math.round(settings.volume * 100)}%</label>
          <input id="settings-vol" type="range" min={0} max={1} step={0.05} value={settings.volume} onChange={(e) => host.saveSettings({ volume: Number(e.target.value) })} className="mt-2 w-full accent-amber" />
          {host.audioDevices && <Devices />}
        </Section>
        {caps.globalPtt && (
          <Section title="Push to talk">
            <PttKey />
            <Toggle label="Floating talker bubble" hint="Shows who is talking when Titi is in the tray" on={settings.overlay} set={(v) => host.saveSettings({ overlay: v })} />
          </Section>
        )}
        {host.quit && (
          <Section title="App">
            {caps.globalPtt && <Toggle label="Start with the computer" hint="Launch minimised to the tray so the radio is always on" on={settings.autostart} set={(v) => host.saveSettings({ autostart: v })} />}
            {caps.globalPtt && <Toggle label="Run in the background" hint="Closing the window keeps Titi in the tray. Off: closing the window quits Titi." on={settings.closeToTray} set={(v) => host.saveSettings({ closeToTray: v })} />}
            <button type="button" onClick={() => host.quit?.()} className="mt-2 flex h-12 w-full items-center justify-center gap-2 rounded-2xl border border-outline font-semibold hover:border-danger hover:text-danger">
              <Power size={18} /> Quit Titi
            </button>
            <p className="mt-2 text-xs text-muted">Stops the radio and closes Titi completely until you open it again.</p>
          </Section>
        )}
        <Section title="Connection">
          {caps.lan && <Toggle label="Same Wi‑Fi (LAN)" hint="Talk to phones on this network with no internet" on={settings.lan} set={(v) => host.saveSettings({ lan: v })} />}
          {caps.lan && <Toggle label="Internet relay" hint="Reach group members anywhere" on={settings.relay} set={(v) => host.saveSettings({ relay: v })} />}
          <label htmlFor="settings-relay" className="text-sm text-muted">Relay URL</label>
          <input id="settings-relay" className="mt-1 w-full rounded-xl border border-outline bg-graphite px-3.5 py-3 font-mono text-xs outline-none focus:border-amber" value={settings.relayUrl} onChange={(e) => host.saveSettings({ relayUrl: e.target.value })} />
          <button className="mt-2 text-xs text-muted underline" onClick={() => host.saveSettings({ relayUrl: DEFAULT_RELAY })}>Reset to default</button>
        </Section>
        <Section title="About">
          <div className="text-sm">Titi {platform} 0.1.0</div>
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

function Toggle({ label, hint, on, set }: { label: string; hint?: string; on: boolean; set: (v: boolean) => void }) {
  return (
    <button type="button" role="switch" aria-checked={on} onClick={() => set(!on)} className="flex w-full items-center gap-3 py-2.5 text-left">
      <span className="flex-1">
        <span className="block text-[15px]">{label}</span>
        {hint && <span className="block text-xs text-muted">{hint}</span>}
      </span>
      <span className={`relative h-7 w-12 shrink-0 rounded-full transition-colors ${on ? "bg-amber" : "bg-outline"}`}>
        <span className={`absolute top-1 h-5 w-5 rounded-full bg-ink transition-transform ${on ? "translate-x-6" : "translate-x-1"}`} />
      </span>
    </button>
  );
}

function PttKey() {
  const key = useStore((s) => s.settings.pttKey);
  const held = useStore((s) => s.pttKeyDown);
  const [learning, setLearning] = useState(false);
  const learn = async () => {
    if (!host.capturePttKey) return;
    setLearning(true);
    const k = await host.capturePttKey();
    setLearning(false);
    if (k) host.saveSettings({ pttKey: k });
  };
  return (
    <div className="flex items-center gap-3 pb-2">
      <span className="flex-1">
        <span className="block text-[15px]">Talk key</span>
        <span className="block text-xs text-muted">Hold anywhere — even in games. Keys like F13, CapsLock or a mouse side button work best.</span>
      </span>
      <button type="button" onClick={() => void learn()} className={`min-w-24 rounded-xl border px-3 py-2 font-mono text-sm ${held ? "border-amber bg-amber/20 text-amber" : "border-outline"}`}>
        {learning ? "Press a key…" : key || "Off"}
      </button>
      {key && <button type="button" className="text-xs text-muted underline" onClick={() => host.saveSettings({ pttKey: "" })}>Clear</button>}
    </div>
  );
}

function Devices() {
  const settings = useStore((s) => s.settings);
  const [devs, setDevs] = useState<{ inputs: AudioDevice[]; outputs: AudioDevice[] } | null>(null);
  useEffect(() => { void host.audioDevices?.().then(setDevs); }, []);
  if (!devs) return null;
  const sel = (id: string, label: string, list: AudioDevice[], value: string, on: (v: string) => void) => (
    <div className="mt-3">
      <label htmlFor={id} className="text-sm text-muted">{label}</label>
      <select id={id} value={value} onChange={(e) => on(e.target.value)} className="mt-1 w-full rounded-xl border border-outline bg-graphite px-3 py-2.5 text-sm outline-none focus:border-amber">
        <option value="">System default</option>
        {list.map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
      </select>
    </div>
  );
  return (
    <>
      {sel("settings-in", "Microphone", devs.inputs, settings.inputDevice, (v) => host.saveSettings({ inputDevice: v }))}
      {sel("settings-out", "Speaker", devs.outputs, settings.outputDevice, (v) => host.saveSettings({ outputDevice: v }))}
    </>
  );
}
