import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "motion/react";
import { AudioLines } from "lucide-react";

/** Always-on-top pill shown while someone talks and the main window is hidden. */
export function Overlay() {
  const [talker, setTalker] = useState<string | null>(null);
  const [level, setLevel] = useState(-60);
  useEffect(() => {
    const u1 = listen<{ talking: boolean; talker: string | null }>("titi://floor", (e) => setTalker(e.payload.talking ? e.payload.talker : null));
    const u2 = listen<number>("titi://level", (e) => setLevel(e.payload));
    return () => { void u1.then((f) => f()); void u2.then((f) => f()); };
  }, []);
  const lv = Math.min(1, Math.max(0, (level + 50) / 50));
  return (
    <AnimatePresence>
      {talker && (
        <motion.div
          className="mx-auto mt-2 flex h-12 w-fit max-w-[290px] items-center gap-3 rounded-full border border-white/10 bg-graphite/90 px-4 text-sm font-semibold text-ink shadow-2xl"
          initial={{ y: 20, opacity: 0, scale: 0.9 }}
          animate={{ y: 0, opacity: 1, scale: 1 }}
          exit={{ y: 20, opacity: 0, scale: 0.9 }}
          transition={{ type: "spring", stiffness: 380, damping: 30 }}
        >
          <motion.span className="grid h-7 w-7 place-items-center rounded-full bg-teal text-graphite" animate={{ scale: 1 + lv * 0.25 }}>
            <AudioLines size={16} />
          </motion.span>
          <span className="truncate">{talker} is talking</span>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
