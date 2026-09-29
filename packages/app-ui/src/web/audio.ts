// Browser audio: WebCodecs Opus encode (20 ms @ 48 kHz mono) on capture,
// per-talker AudioDecoder + scheduled AudioBufferSource playout.
// Falls back to a clear error where WebCodecs Opus is unavailable (old Safari).

const SR = 48_000;
const FRAME = 960;

// Inlined so packaged hosts (Tizen .wgt, file:// origins) don't depend on a public path.
const WORKLET = `class P extends AudioWorkletProcessor{constructor(){super();this.b=new Float32Array(${FRAME});this.n=0}
process(i){const c=i[0]&&i[0][0];if(!c)return true;let k=0;while(k<c.length){const t=Math.min(c.length-k,${FRAME}-this.n);
this.b.set(c.subarray(k,k+t),this.n);this.n+=t;k+=t;if(this.n===${FRAME}){this.port.postMessage(this.b,[this.b.buffer]);this.b=new Float32Array(${FRAME});this.n=0}}return true}}
registerProcessor("titi-capture",P);`;
let workletUrl: string | null = null;
const getWorkletUrl = () => (workletUrl ??= URL.createObjectURL(new Blob([WORKLET], { type: "text/javascript" })));

export function webCodecsSupported(): boolean {
  return typeof AudioEncoder !== "undefined" && typeof AudioDecoder !== "undefined";
}

export class Capture {
  private ctx: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private node: AudioWorkletNode | null = null;
  private enc: AudioEncoder | null = null;
  muted = false;
  onLevel: ((dbfs: number) => void) | null = null;

  constructor(private onPacket: (packet: Uint8Array) => void) {}

  async start(profile: "hq" | "std" | "low" | "min" = "std") {
    if (this.enc) return;
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, sampleRate: SR, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      video: false,
    });
    this.ctx = new AudioContext({ sampleRate: SR, latencyHint: "interactive" });
    await this.ctx.audioWorklet.addModule(getWorkletUrl());
    const src = this.ctx.createMediaStreamSource(this.stream);
    this.node = new AudioWorkletNode(this.ctx, "titi-capture", { numberOfInputs: 1, numberOfOutputs: 0, channelCount: 1 });
    src.connect(this.node);

    const bitrate = { hq: 32_000, std: 24_000, low: 16_000, min: 8_000 }[profile];
    const enc = new AudioEncoder({
      output: (chunk) => {
        const buf = new Uint8Array(chunk.byteLength);
        chunk.copyTo(buf);
        this.onPacket(buf);
      },
      error: (e) => console.error("opus encoder", e),
    });
    enc.configure({ codec: "opus", sampleRate: SR, numberOfChannels: 1, bitrate, opus: { frameDuration: 20_000, application: "voip", useinbandfec: true } as never });
    this.enc = enc;

    let ts = 0;
    this.node.port.onmessage = (e: MessageEvent<Float32Array>) => {
      const f32 = e.data;
      if (this.muted) f32.fill(0);
      if (this.onLevel) {
        let s = 0;
        for (let i = 0; i < f32.length; i++) s += f32[i]! * f32[i]!;
        this.onLevel(20 * Math.log10(Math.sqrt(s / f32.length) + 1e-9));
      }
      const data = new AudioData({ format: "f32", sampleRate: SR, numberOfFrames: FRAME, numberOfChannels: 1, timestamp: ts, data: f32 as unknown as BufferSource });
      ts += 20_000;
      if (this.enc?.state === "configured") this.enc.encode(data);
      data.close();
    };
  }

  async stop() {
    this.node?.port.close();
    this.node?.disconnect();
    this.node = null;
    if (this.enc) { try { await this.enc.flush(); } catch { /* ignore */ } this.enc.close(); this.enc = null; }
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    await this.ctx?.close();
    this.ctx = null;
  }
}

/** Decodes per-talker Opus packets and schedules them back-to-back. */
export class Playout {
  private ctx: AudioContext | null = null;
  private decoders = new Map<string, { dec: AudioDecoder; nextTime: number; ts: number }>();
  private gain: GainNode | null = null;
  private vol = 1;

  set volume(v: number) {
    this.vol = Math.min(1, Math.max(0, v));
    if (this.gain) this.gain.gain.value = this.vol;
  }

  async ensure() {
    if (this.ctx) { if (this.ctx.state === "suspended") await this.ctx.resume(); return; }
    this.ctx = new AudioContext({ sampleRate: SR, latencyHint: "interactive" });
    this.gain = this.ctx.createGain();
    this.gain.gain.value = this.vol;
    this.gain.connect(this.ctx.destination);
  }

  /** Empty packet = concealment slot (we just let the schedule advance). */
  play(talker: string, packet: Uint8Array, frames = 1) {
    const ctx = this.ctx;
    if (!ctx || !this.gain) return;
    let d = this.decoders.get(talker);
    if (!d) {
      const dec = new AudioDecoder({
        output: (ad) => this.schedule(talker, ad),
        error: (e) => console.warn("opus decoder", e),
      });
      dec.configure({ codec: "opus", sampleRate: SR, numberOfChannels: 1 });
      d = { dec, nextTime: 0, ts: 0 };
      this.decoders.set(talker, d);
    }
    if (packet.length === 0) { d.ts += 20_000 * frames; return; }
    if (d.dec.state !== "configured") return;
    d.dec.decode(new EncodedAudioChunk({ type: "key", timestamp: d.ts, data: packet }));
    d.ts += 20_000 * frames;
  }

  private schedule(talker: string, ad: AudioData) {
    const ctx = this.ctx!;
    const d = this.decoders.get(talker)!;
    const buf = ctx.createBuffer(1, ad.numberOfFrames, SR);
    const f32 = new Float32Array(ad.numberOfFrames);
    ad.copyTo(f32, { planeIndex: 0, format: "f32-planar" });
    buf.copyToChannel(f32, 0);
    ad.close();
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.connect(this.gain!);
    const now = ctx.currentTime;
    // keep ~40 ms of lead; resync if we fell behind or drifted far ahead
    if (d.nextTime < now + 0.02 || d.nextTime > now + 0.3) d.nextTime = now + 0.04;
    src.start(d.nextTime);
    d.nextTime += buf.duration;
  }

  dropIdle() {
    for (const [k, d] of this.decoders) if (this.ctx && d.nextTime < this.ctx.currentTime - 2) { d.dec.close(); this.decoders.delete(k); }
  }

  /** Short synthesized cue (Bird pack) */
  cue(kind: "granted" | "released" | "denied" | "incoming" | "warning" | "sos") {
    const ctx = this.ctx;
    if (!ctx) return;
    const tones: [number, number, number][] = {
      granted: [[1760, 0.045, 0.5], [2350, 0.07, 0.45]],
      released: [[1320, 0.05, 0.4], [880, 0.07, 0.35]],
      denied: [[330, 0.06, 0.35], [0, 0.04, 0], [330, 0.06, 0.35]],
      incoming: [[1568, 0.04, 0.3]],
      warning: [[988, 0.08, 0.3], [0, 0.06, 0], [988, 0.08, 0.3]],
      sos: [[1760, 0.12, 0.5], [1175, 0.12, 0.5], [1760, 0.12, 0.5], [1175, 0.12, 0.5]],
    }[kind] as [number, number, number][];
    let t = ctx.currentTime + 0.01;
    for (const [hz, dur, gain] of tones) {
      if (hz > 0) {
        const o = ctx.createOscillator();
        const g = ctx.createGain();
        o.frequency.value = hz;
        g.gain.setValueAtTime(0, t);
        g.gain.linearRampToValueAtTime(gain, t + dur * 0.1);
        g.gain.linearRampToValueAtTime(0, t + dur);
        o.connect(g).connect(ctx.destination);
        o.start(t);
        o.stop(t + dur + 0.01);
      }
      t += dur;
    }
  }
}
