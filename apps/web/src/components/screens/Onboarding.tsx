"use client";
import { useState } from "react";
import { motion } from "motion/react";
import { Check, Mic } from "lucide-react";
import { host } from "@/lib/engine";
import { hueColor } from "@/lib/cn";
import { Avatar } from "../Avatar";
import { focusOnMount } from "../Sheet";

const HUES = [40, 15, 350, 290, 220, 170, 120, 80];

export function Onboarding({ onDone }: { onDone: () => void }) {
  const [name, setName] = useState("");
  const [hue, setHue] = useState(40);
  return (
    <div className="mx-auto flex min-h-dvh w-full max-w-md flex-col justify-between p-6">
      <div>
        <div className="mt-8 grid h-[72px] w-[72px] place-items-center rounded-[22px] bg-amber">
          <Mic size={36} className="text-graphite" />
        </div>
        <h1 className="mt-7 text-4xl font-extrabold leading-[1.05] tracking-tight">Talk anywhere.<br />No signal needed.</h1>
        <p className="mt-3 text-[17px] text-muted">Titi turns nearby phones into a private radio. The web version joins over the internet, or over Wi‑Fi to a phone running the app.</p>
        <h2 className="mt-9 text-base font-semibold">What should others call you?</h2>
        <div className="mt-2.5 flex items-center gap-3.5">
          <Avatar name={name || "?"} hue={hue} size={52} />
          <input className="flex-1 rounded-2xl border border-outline bg-surface px-4 py-3.5 text-lg outline-none focus:border-amber" placeholder="Your name" value={name} onChange={(e) => setName(e.target.value)} ref={focusOnMount} maxLength={24} />
        </div>
        <h2 className="mt-6 text-base font-semibold">Pick your colour</h2>
        <div className="mt-2.5 flex gap-2.5">
          {HUES.map((h) => (
            <button key={h} type="button" onClick={() => setHue(h)} className="grid h-10 w-10 place-items-center rounded-full" style={{ background: hueColor(h), outline: h === hue ? "3px solid var(--color-ink)" : "none" }} aria-label={`Colour ${h}`}>
              {h === hue && <Check size={18} className="text-graphite" />}
            </button>
          ))}
        </div>
      </div>
      <motion.button
        whileTap={{ scale: 0.98 }}
        disabled={name.trim().length < 2}
        onClick={() => { host.saveSettings({ name: name.trim(), hue, onboarded: true }); onDone(); }}
        className="h-14 w-full rounded-[18px] bg-amber text-lg font-semibold text-graphite disabled:opacity-40"
      >
        Continue
      </motion.button>
    </div>
  );
}
