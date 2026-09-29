// Multi-group wall: every group at once, live talker per tile. Used by the PC
// app (true 21:9 at 3440×1440 when maximised) and the TV shell (16:9 grid).
// Pure view: focus/selection come from the caller so both pointer and D-pad work.
import { useSyncExternalStore } from "react";
import { AnimatePresence, motion } from "motion/react";
import { AudioLines, MessageSquare, Radio, Users } from "lucide-react";
import { useStore, type GroupState } from "./store";
import { Avatar } from "./Avatar";
import { cn, hueColor } from "./cn";
import { dashboardColumns } from "./dashboard-layout";

export interface DashboardProps {
  /** Index of the focused tile (D-pad), or -1. */
  focus?: number;
  /** Pointer hosts: tile click selects the group. */
  onPick?: (g: GroupState) => void;
  /** 10-foot sizing (TV). */
  tenFoot?: boolean;
  className?: string;
}

const onResize = (cb: () => void) => { window.addEventListener("resize", cb); return () => window.removeEventListener("resize", cb); };
const aspectNow = () => window.innerWidth / Math.max(1, window.innerHeight);

export function Dashboard({ focus = -1, onPick, tenFoot = false, className }: DashboardProps) {
  const groups = useStore((s) => s.groups);
  const aspect = useSyncExternalStore(onResize, aspectNow, () => 16 / 9);
  const cols = dashboardColumns(groups.length, aspect);
  if (groups.length === 0) {
    return (
      <div className={cn("grid flex-1 place-items-center rounded-[32px] bg-surface text-muted", className)}>
        <div className="flex flex-col items-center gap-4"><Radio size={tenFoot ? 80 : 48} className="text-amber" /><span className={tenFoot ? "text-4xl" : "text-lg"}>No groups yet</span></div>
      </div>
    );
  }
  return (
    <ul className={cn("grid flex-1 gap-5", className)} style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, gridAutoRows: "minmax(0, 1fr)" }} aria-label="All groups">
      {groups.map((g, i) => <li key={g.id} className="flex min-h-0"><Tile g={g} focused={focus === i} tenFoot={tenFoot} onPick={onPick} /></li>)}
    </ul>
  );
}

function Tile({ g, focused, tenFoot, onPick }: { g: GroupState; focused: boolean; tenFoot: boolean; onPick?: (g: GroupState) => void }) {
  const nodeId = useStore((s) => s.nodeId);
  const level = useStore((s) => s.levelDbfs);
  const msgs = useStore((s) => s.messages);
  const last = msgs.filter((m) => m.group === g.id).slice(tenFoot ? -2 : -3);
  const me = g.floor === "talking";
  const live = me || g.floor === "busy";
  const talker = g.members.find((m) => m.node === g.talkerNode);
  const hue = me ? 40 : talker?.hue ?? 170;
  const lv = Math.min(1, Math.max(0, (level + 50) / 50));
  const Tag = onPick ? "button" : "div";
  return (
    <Tag
      type={onPick ? "button" : undefined}
      onClick={onPick ? () => onPick(g) : undefined}
      aria-label={`${g.name}${live ? `, ${me ? "you are" : `${g.talkerName} is`} talking` : ""}`}
      className={cn(
        "relative flex min-h-0 w-full flex-1 flex-col overflow-hidden rounded-[32px] bg-surface text-left outline-none transition-transform duration-150",
        tenFoot ? "p-8" : "p-6 focus-visible:ring-4 focus-visible:ring-amber",
        focused && "scale-[1.03] ring-4 ring-amber",
        live && "bg-elevated",
      )}
    >
      {live && <motion.span aria-hidden className="pointer-events-none absolute inset-0" style={{ background: hueColor(hue), opacity: 0.1 }} animate={{ opacity: [0.06, 0.16, 0.06] }} transition={{ duration: 1.6, repeat: Infinity }} />}
      <div className="relative flex items-center gap-4">
        <span className={cn("grid shrink-0 place-items-center rounded-[18px] font-bold", tenFoot ? "h-16 w-16 text-3xl" : "h-12 w-12 text-2xl", live ? "bg-teal text-graphite" : "bg-amber/20 text-amber")}>{g.name[0]?.toUpperCase()}</span>
        <div className="min-w-0 flex-1">
          <div className={cn("truncate font-bold", tenFoot ? "text-[34px]" : "text-2xl")}>{g.name}</div>
          <div className={cn("flex items-center gap-2 text-muted", tenFoot ? "text-xl" : "text-sm")}><Users size={tenFoot ? 22 : 16} /> {g.memberCount} · {g.link ?? "connecting…"}{g.fullDuplex ? " · open mic" : ""}</div>
        </div>
        {g.unread > 0 && <span className={cn("rounded-full bg-amber px-3 py-1 font-bold text-graphite", tenFoot ? "text-xl" : "text-sm")}>{g.unread}</span>}
      </div>

      <div className="relative flex min-h-0 flex-1 items-center justify-center py-4">
        <AnimatePresence mode="wait" initial={false}>
          {live ? (
            <motion.div key={"t" + (g.talkerNode ?? "me")} className="flex flex-col items-center" initial={{ opacity: 0, scale: 0.9 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0 }}>
              <div className="relative">
                <motion.span className="absolute inset-[-14px] rounded-full" style={{ background: hueColor(hue), opacity: 0.25 }} animate={{ scale: 1 + (me ? lv : 0.6) * 0.3 }} transition={{ type: "spring", stiffness: 300, damping: 20 }} />
                <Avatar name={me ? "You" : g.talkerName ?? "?"} hue={hue} size={tenFoot ? 150 : 110} />
              </div>
              <div className={cn("mt-5 flex items-center gap-2 font-bold", me ? "text-amber" : "text-teal", tenFoot ? "text-3xl" : "text-xl")}><AudioLines size={tenFoot ? 32 : 22} /> {me ? "You" : g.talkerName}</div>
            </motion.div>
          ) : (
            <motion.div key="idle" className="flex -space-x-4" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
              {g.members.slice(0, 5).map((m) => <Avatar key={m.node} name={m.node === nodeId ? "Me" : m.name} hue={m.hue} size={tenFoot ? 84 : 64} className="ring-4 ring-surface" />)}
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {last.length > 0 && (
        <div className="relative flex flex-col gap-2">
          {last.map((m) => (
            <div key={m.id} className={cn("flex items-center gap-2 truncate rounded-2xl bg-graphite/50 px-4 py-2", tenFoot ? "text-xl" : "text-sm")}>
              <MessageSquare size={tenFoot ? 20 : 14} className="shrink-0 text-muted" /><b className="shrink-0">{m.fromName}</b>
              <span className="truncate">{m.body.kind === "text" ? m.body.text : m.body.kind === "sos" ? "SOS" : m.body.kind}</span>
            </div>
          ))}
        </div>
      )}
    </Tag>
  );
}
