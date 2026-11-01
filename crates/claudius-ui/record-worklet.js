// claudius' recorder, in the browser's audio thread (`audio/web_record.rs` loads it). It hands the page what its input
// hears, about 43 ms at a time, with the context frame its first sample reached the worklet in, so that the page can
// line a take up with what played on the same clock. Told "stop", it hands over what it still holds, then says
// `{ done: true }`. It plays nothing.
class ClaudiusRecorder extends AudioWorkletProcessor {
  constructor() {
    super();
    this.size = 2048;
    this.planes = [];
    this.held = 0;
    this.first = 0;
    this.done = false;
    this.port.onmessage = (e) => {
      if (e.data === "stop" && !this.done) {
        this.flush();
        this.done = true;
        this.port.postMessage({ done: true });
      }
    };
  }

  process(inputs) {
    if (this.done) return false;
    const input = inputs[0];
    // no input yet (or it ended): nothing to hand over, and the next chunk says where it starts
    if (!input || input.length === 0) {
      this.flush();
      return true;
    }
    const n = input[0].length;
    // a chunk holds frames one after another, all with the same channels: a quantum that does not fit, has other
    // channels or is not next to the last starts another
    const room = this.planes.length === input.length ? this.planes[0].length - this.held : 0;
    if (room < n || (this.held > 0 && currentFrame !== this.first + this.held)) this.flush();
    if (this.planes.length !== input.length || this.planes[0].length < n) {
      this.planes = input.map(() => new Float32Array(Math.max(this.size, n)));
    }
    if (this.held === 0) this.first = currentFrame;
    for (let c = 0; c < input.length; c++) this.planes[c].set(input[c], this.held);
    this.held += n;
    if (this.held >= this.size) this.flush();
    return true;
  }

  flush() {
    if (this.held === 0) return;
    const channels = this.planes.map((p) => p.slice(0, this.held));
    this.port.postMessage({ frame: this.first, channels }, channels.map((c) => c.buffer));
    this.held = 0;
  }
}

registerProcessor("claudius-recorder", ClaudiusRecorder);
