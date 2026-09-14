"use client";
import { AnimatePresence, motion } from "motion/react";
import { useEffect } from "react";

export function Sheet({ open, onClose, title, children }: { open: boolean; onClose: () => void; title?: string; children: React.ReactNode }) {
  useEffect(() => {
    if (!open) return;
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [open, onClose]);
  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div className="fixed inset-0 z-40 bg-black/60" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={onClose} />
          <motion.div
            role="dialog"
            aria-modal
            aria-label={title}
            className="fixed inset-x-0 bottom-0 z-50 mx-auto max-w-md rounded-t-[28px] bg-surface p-6 pb-[max(1.5rem,env(safe-area-inset-bottom))]"
            initial={{ y: "100%" }}
            animate={{ y: 0 }}
            exit={{ y: "100%" }}
            transition={{ type: "spring", stiffness: 380, damping: 34 }}
            drag="y"
            dragConstraints={{ top: 0 }}
            dragElastic={0.1}
            onDragEnd={(_, i) => { if (i.offset.y > 120 || i.velocity.y > 600) onClose(); }}
          >
            <div className="mx-auto mb-4 h-1 w-10 rounded-full bg-outline" />
            {title && <h2 className="mb-4 text-2xl font-bold">{title}</h2>}
            {children}
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
