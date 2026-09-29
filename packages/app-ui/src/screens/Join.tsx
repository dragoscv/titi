"use client";
import { useEffect, useRef, useState } from "react";
import { ArrowLeft, QrCode } from "lucide-react";
import { host } from "../platform";
import { isInviteUrl } from "../links";
import { useStore } from "../store";
import type { Route } from "../Shell";

export function Join({ go }: { go: (r: Route) => void }) {
  const [code, setCode] = useState("");
  const [searching, setSearching] = useState(false);
  const [scan, setScan] = useState(false);
  const peers = useStore((s) => Object.keys(s.peers).length);
  const parse = useStore((s) => s.ready);
  const norm = code.trim().toLowerCase().replace(/\s+/g, "-");
  // async validation (core parser may live behind IPC): remember the verdict for the code it was computed for
  const [checked, setChecked] = useState<{ code: string; ok: boolean }>({ code: "", ok: false });
  useEffect(() => {
    if (!parse || norm.length === 0) return;
    let live = true;
    void host.parseCode(norm).then((ok) => { if (live) setChecked({ code: norm, ok }); });
    return () => { live = false; };
  }, [parse, norm]);
  const valid = parse && norm.length > 0 && checked.code === norm && checked.ok;

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex items-center gap-1 px-2 pt-2">
        <button className="p-3 text-muted hover:text-ink" onClick={() => go({ name: "home" })} aria-label="Back"><ArrowLeft /></button>
        <h1 className="text-xl font-semibold">Join a group</h1>
      </header>
      {scan ? (
        <Scanner onResult={(t) => { setScan(false); if (isInviteUrl(t)) { void host.start(); host.joinByLink(t.startsWith("http") ? "titi://j/" + t.split("/j/")[1] : t); setSearching(true); } else setCode(t); }} onCancel={() => setScan(false)} />
      ) : (
        <div className="p-6">
          <p className="text-[17px] text-muted">Ask the host for the three words and two digits shown on their screen. Both of you need to be in the same room — online, or a phone on your Wi‑Fi.</p>
          <input className="mt-6 w-full rounded-[18px] border border-outline bg-surface px-4 py-4 font-mono text-[26px] outline-none placeholder:text-outline focus:border-amber" placeholder="word word word 00" value={code} onChange={(e) => setCode(e.target.value)} autoCapitalize="none" autoCorrect="off" spellCheck={false} />
          <div className="mt-4 flex gap-3">
            <button className="flex h-14 flex-1 items-center justify-center gap-2 rounded-[18px] border border-outline font-semibold" onClick={() => setScan(true)}><QrCode size={20} /> Scan QR</button>
            <button className="h-14 flex-1 rounded-[18px] bg-amber font-semibold text-graphite disabled:opacity-40" disabled={!valid || searching} onClick={() => { void host.start(); host.joinByCode(norm); setSearching(true); }}>Join</button>
          </div>
          {searching && (
            <div className="mt-7 text-sm">
              <div className="flex items-center gap-3"><span className="h-5 w-5 animate-spin rounded-full border-2 border-outline border-t-amber" /> Looking for the group…</div>
              {peers === 0 && <p className="mt-3 text-muted">Nobody reachable yet. The host must be online in the same group room, or run the Titi app on a phone on your Wi‑Fi.</p>}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

type BarcodeDetectorCtor = new (o: { formats: string[] }) => { detect: (v: HTMLVideoElement) => Promise<{ rawValue: string }[]> };
const getDetector = () => (window as unknown as { BarcodeDetector?: BarcodeDetectorCtor }).BarcodeDetector;

function Scanner({ onResult, onCancel }: { onResult: (t: string) => void; onCancel: () => void }) {
  const video = useRef<HTMLVideoElement>(null);
  // Scanner mounts only on the client (after a tap), so window is available for the lazy initialiser
  const [err, setErr] = useState<string | null>(() => (getDetector() ? null : "QR scanning needs Chrome/Edge/Safari 17+. Type the code instead."));
  useEffect(() => {
    let stream: MediaStream | null = null;
    let raf = 0;
    const Detector = getDetector();
    if (!Detector) return;
    const det = new Detector({ formats: ["qr_code"] });
    void (async () => {
      try {
        stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: "environment" } });
        if (video.current) { video.current.srcObject = stream; await video.current.play(); }
        const loop = async () => {
          if (video.current && video.current.readyState >= 2) {
            const r = await det.detect(video.current).catch(() => []);
            if (r[0]) { onResult(r[0].rawValue); return; }
          }
          raf = requestAnimationFrame(() => void loop());
        };
        void loop();
      } catch (e) { setErr(String((e as Error).message)); }
    })();
    return () => { cancelAnimationFrame(raf); stream?.getTracks().forEach((t) => t.stop()); };
  }, [onResult]);
  return (
    <div className="flex flex-1 flex-col p-5">
      <div className="relative flex-1 overflow-hidden rounded-3xl bg-black">
        <video ref={video} className="h-full w-full object-cover" muted playsInline />
        {err && <div className="absolute inset-0 grid place-items-center p-6 text-center text-sm text-danger">{err}</div>}
      </div>
      <button className="mt-4 h-12 rounded-2xl border border-outline" onClick={onCancel}>Cancel</button>
    </div>
  );
}
