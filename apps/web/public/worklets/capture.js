// Accumulates 48 kHz mono float samples into 960-sample (20 ms) frames and
// posts them to the main thread as Float32Array.
class CaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(960);
    this.n = 0;
  }
  process(inputs) {
    const ch = inputs[0] && inputs[0][0];
    if (!ch) return true;
    let i = 0;
    while (i < ch.length) {
      const take = Math.min(ch.length - i, 960 - this.n);
      this.buf.set(ch.subarray(i, i + take), this.n);
      this.n += take;
      i += take;
      if (this.n === 960) {
        this.port.postMessage(this.buf, [this.buf.buffer]);
        this.buf = new Float32Array(960);
        this.n = 0;
      }
    }
    return true;
  }
}
registerProcessor("titi-capture", CaptureProcessor);
