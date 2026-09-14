"use client";
import { AnimatePresence, motion } from "motion/react";
import { Mic, MicOff, AudioLines } from "lucide-react";
import { useRef, useState } from "react";
import type { FloorState } from "@/lib/store";

const spring = { type: "spring" as const, stiffness: 380, damping: 30 };

/**
 * Morphing Talk button (ADR-0007): circle → squircle (armed) → wide bar
 * (transmitting) → mute pill (full-duplex). Pointer capture so a thumb that
 * drifts while talking never drops the floor.
 */
export function TalkButton(props: {
  floor: FloorState;
  fullDuplex: boolean;
  muted: boolean;
  talkerName: string | null;
  levelDbfs: number;
  onDown: () => void;
  onUp: () => void;
  onToggleMute: () => void;
}) {
  const { floor, fullDuplex, muted, talkerName, levelDbfs } = props;
  const [pressed, setPressed] = useState(false);
  const holding = useRef(false);
  const transmitting = floor === "talking";
  const busy = floor === "busy" || floor === "queued";
  const wide = transmitting || fullDuplex;
  const armed = pressed && !wide;
  const level = Math.min(1, Math.max(0, (levelDbfs + 50) / 50));

  const w = wide ? 320 : armed ? 196 : 176;
  const h = wide ? 96 : armed ? 196 : 176;
  const r = wide ? 48 : armed ? 56 : 88;
  const bg = fullDuplex && muted ? "var(--color-elevated)" : fullDuplex ? "var(--color-teal)" : transmitting ? "var(--color-amber)" : busy ? "var(--color-elevated)" : armed ? "var(--color-amber-deep)" : "var(--color-amber)";
  const fg = busy || (fullDuplex && muted) ? "var(--color-ink)" : "var(--color-graphite)";

  const down = (e: React.PointerEvent) => {
    if (fullDuplex) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    holding.current = true;
    setPressed(true);
    navigator.vibrate?.(15);
    props.onDown();
  };
  const up = () => {
    if (fullDuplex || !holding.current) return;
    holding.current = false;
    setPressed(false);
    props.onUp();
  };

  const label = fullDuplex ? (muted ? "Unmute" : "Open mic") : transmitting ? "You are talking" : busy ? talkerName ?? "" : "Hold to talk";
  const sub = fullDuplex ? (muted ? "Open mic" : "Tap to mute") : transmitting ? "Release to finish" : "";

  return (
    <motion.button
      type="button"
      className="no-select relative grid place-items-center overflow-hidden shadow-[0_18px_50px_-18px_rgba(255,176,32,0.55)] outline-none focus-visible:ring-4 focus-visible:ring-teal/60"
      style={{ color: fg }}
      animate={{ width: w, height: h, borderRadius: r, backgroundColor: bg, scale: pressed && !wide ? 0.96 : 1 }}
      transition={spring}
      onPointerDown={down}
      onPointerUp={up}
      onPointerCancel={up}
      onLostPointerCapture={up}
      onClick={() => fullDuplex && props.onToggleMute()}
      onKeyDown={(e) => { if ((e.key === " " || e.key === "Enter") && !e.repeat) { e.preventDefault(); if (fullDuplex) props.onToggleMute(); else { holding.current = true; setPressed(true); props.onDown(); } } }}
      onKeyUp={(e) => { if ((e.key === " " || e.key === "Enter") && !fullDuplex) up(); }}
      onContextMenu={(e) => e.preventDefault()}
      aria-label="Talk button. Press and hold to transmit."
      aria-pressed={transmitting}
    >
      {transmitting && (
        <motion.span
          className="pointer-events-none absolute inset-y-0 left-0 bg-white/25"
          animate={{ width: `${25 + 75 * level}%` }}
          transition={{ type: "spring", stiffness: 200, damping: 25 }}
        />
      )}
      <AnimatePresence mode="wait" initial={false}>
        {wide ? (
          <motion.span key="wide" className="relative flex w-full items-center gap-3.5 px-6" initial={{ opacity: 0, scale: 0.9 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0, scale: 1.1 }} transition={{ duration: 0.15 }}>
            {fullDuplex && muted ? <MicOff size={30} /> : transmitting ? <AudioLines size={30} /> : <Mic size={30} />}
            <span className="flex flex-col text-left">
              <span className="text-[17px] font-semibold leading-tight">{label}</span>
              <span className="text-xs opacity-70">{sub}</span>
            </span>
            {transmitting && (
              <span className="ml-auto flex items-end gap-[3px]">
                {[0, 1, 2, 3, 4, 5].map((i) => (
                  <span key={i} className="w-1 rounded-sm bg-current" style={{ height: 10 + i * 4, opacity: level >= ((i + 1) / 6) * 0.9 ? 1 : 0.25 }} />
                ))}
              </span>
            )}
          </motion.span>
        ) : (
          <motion.span key="round" className="relative flex flex-col items-center gap-1.5" initial={{ opacity: 0, scale: 0.9 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0, scale: 1.1 }} transition={{ duration: 0.15 }}>
            <Mic size={44} />
            <span className="text-sm font-semibold opacity-85">{label}</span>
          </motion.span>
        )}
      </AnimatePresence>
    </motion.button>
  );
}
