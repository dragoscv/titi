"use client";
import { motion } from "motion/react";
import { cn, hueColor, initials } from "./cn";

export function Avatar({ name, hue, size = 40, talking = false, className }: { name: string; hue: number; size?: number; talking?: boolean; className?: string }) {
  return (
    <div className={cn("relative grid place-items-center rounded-full font-bold text-graphite", className)} style={{ width: size, height: size, background: hueColor(hue), fontSize: size * 0.38 }} aria-hidden>
      {talking && (
        <motion.span
          className="absolute inset-[-4px] rounded-full border-2 border-teal"
          animate={{ scale: [1, 1.18, 1], opacity: [0.9, 0.4, 0.9] }}
          transition={{ duration: 0.9, repeat: Infinity, ease: "easeInOut" }}
        />
      )}
      {initials(name)}
    </div>
  );
}

export function SignalBars({ bars, className }: { bars: number; className?: string }) {
  return (
    <span className={cn("inline-flex items-end gap-[2px]", className)} aria-label={`Signal ${bars} of 4`}>
      {[0, 1, 2, 3].map((i) => (
        <span key={i} className={cn("w-[3px] rounded-sm", i < bars ? "bg-teal" : "bg-outline")} style={{ height: 5 + i * 3 }} />
      ))}
    </span>
  );
}
