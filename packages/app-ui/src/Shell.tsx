"use client";
import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { host } from "./platform";
import { useStore } from "./store";
import { Home } from "./screens/Home";
import { Group } from "./screens/Group";
import { Join } from "./screens/Join";
import { Settings } from "./screens/Settings";
import { Onboarding } from "./screens/Onboarding";
import { Avatar } from "./Avatar";

export type Route = { name: "home" } | { name: "group"; id: string } | { name: "join" } | { name: "settings" };

export function Shell({ joinLink }: { joinLink?: string }) {
  const ready = useStore((s) => s.ready);
  const settings = useStore((s) => s.settings);
  const toast = useStore((s) => s.toast);
  const invites = useStore((s) => s.invites);
  const activeGroup = useStore((s) => s.activeGroup);
  const pendingLink = useStore((s) => s.pendingLink);
  const [route, setRoute] = useState<Route>({ name: "home" });
  const unsupported = !useStore((s) => s.voiceSupported);
  // once a join succeeds, jump to the group (adjusted during render instead of in an effect)
  if (route.name === "join" && activeGroup) setRoute({ name: "group", id: activeGroup });

  useEffect(() => {
    void host.init();
  }, []);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => useStore.getState().set({ toast: null }), 3000);
    return () => clearTimeout(t);
  }, [toast]);
  // deep link: join once the engine is running
  useEffect(() => {
    if (ready && joinLink && settings.onboarded) { void host.start().then(() => { host.joinByLink(joinLink); setRoute({ name: "join" }); }); }
  }, [ready, joinLink, settings.onboarded]);
  // OS-delivered deep link (desktop single-instance / TV intent): route during render…
  const takeLink = ready && !!pendingLink && settings.onboarded;
  if (takeLink && route.name !== "join") setRoute({ name: "join" });
  // …and hand it to the engine (external system) once
  useEffect(() => {
    if (!takeLink || !pendingLink) return;
    useStore.getState().set({ pendingLink: null });
    host.joinByLink(pendingLink);
  }, [takeLink, pendingLink]);

  if (!ready) return <Splash />;
  if (!settings.onboarded) return <Onboarding onDone={() => setRoute({ name: "home" })} />;

  return (
    <div className="mx-auto flex min-h-dvh w-full max-w-md flex-col">
      {unsupported && (
        <div className="m-3 rounded-2xl bg-danger/15 p-3 text-sm text-danger">This browser lacks WebCodecs Opus. Use Chrome, Edge or Safari 17+ for voice.</div>
      )}
      <AnimatePresence mode="wait" initial={false}>
        <motion.main key={route.name + ("id" in route ? route.id : "")} className="flex flex-1 flex-col" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} transition={{ duration: 0.18 }}>
          {route.name === "home" && <Home go={setRoute} />}
          {route.name === "group" && <Group id={route.id} go={setRoute} />}
          {route.name === "join" && <Join go={setRoute} />}
          {route.name === "settings" && <Settings go={setRoute} />}
        </motion.main>
      </AnimatePresence>

      <AnimatePresence>
        {invites[0] && (
          <motion.div key={invites[0].group} className="fixed inset-x-0 top-0 z-50 mx-auto max-w-md p-3" initial={{ y: -80, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: -80, opacity: 0 }} transition={{ type: "spring", stiffness: 380, damping: 30 }}>
            <div className="flex items-center gap-3 rounded-2xl bg-elevated p-3.5 shadow-2xl">
              <Avatar name={invites[0].hostName} hue={(parseInt(invites[0].host.slice(0, 2), 16) * 360) / 256} />
              <div className="flex-1 min-w-0">
                <div className="text-sm font-semibold">{invites[0].hostName} invites you</div>
                <div className="truncate text-xs text-muted">Join “{invites[0].name}” · {invites[0].members} members</div>
              </div>
              <button className="rounded-xl px-3 py-2 text-sm text-muted" onClick={() => host.declineInvite(invites[0]!.group, invites[0]!.host)}>Not now</button>
              <button className="rounded-xl bg-amber px-3.5 py-2 text-sm font-semibold text-graphite" onClick={() => { void host.start(); host.acceptInvite(invites[0]!.group, invites[0]!.host); }}>Join</button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {toast && (
          <motion.output key={toast} aria-live="polite" className="fixed inset-x-0 bottom-6 z-50 mx-auto block w-fit max-w-[90%] rounded-full bg-elevated px-4 py-2 text-sm shadow-xl" initial={{ y: 30, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: 30, opacity: 0 }}>
            {toast}
          </motion.output>
        )}
      </AnimatePresence>
    </div>
  );
}

function Splash() {
  return (
    <div className="grid min-h-dvh place-items-center">
      <motion.div className="grid h-20 w-20 place-items-center rounded-3xl bg-amber" animate={{ scale: [1, 1.06, 1] }} transition={{ duration: 1.2, repeat: Infinity }}>
        <span className="text-3xl font-extrabold text-graphite">t</span>
      </motion.div>
    </div>
  );
}
