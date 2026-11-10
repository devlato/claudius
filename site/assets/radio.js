// claudius radio: the studio demo's engine (claudius itself, in a Web Worker) writes and renders a song at a time,
// one genre after another, and this page plays them on two decks, mixing each into the next as a DJ would.
//
// The next song is asked for as soon as one goes on air. Most of the time its genre is one whose tempo range takes
// the playing song's tempo, or twice or half of it, and it is written at exactly that tempo and in a key with the same
// notes as the playing one's, so the two mix beat for beat and in key: it comes in under the outgoing song's outro on
// a bar line, its bass cut, the basses swap halfway through, and the outgoing song is thinned out and faded. A song in
// another tempo comes in after the outgoing one echoes out, or now and then after it stops like a record on a
// turntable. Every change is scheduled on the audio clock when the song arrives, so it lands on the beat even in a
// tab that is not being looked at.

const ENGINE = "/studio/claudius-engine_loader.js";
const RATE = 48000;
// what the station leaves out: no beat to mix on, or too harsh for a radio
const LEAVE_OUT = new Set(["ambient", "breakcore", "gabber", "hard_techno", "hardstyle", "idm"]);
// the genres the first song is drawn from
const OPENERS = ["house", "deep_house", "nu_disco", "hip_hop", "lofi", "afrobeats", "dnb", "synthwave", "trap", "funk"];
// how often the next song is one that mixes into the playing one beat for beat
const BLEND = 0.75;
// the bass of a song coming in is cut this far, and the outgoing one's once they swap
const BASS_CUT_DB = -26;
// for its tests: the first song starts 20 s before it gives way, so that the next comes in as soon as it is made
const SOON = new URLSearchParams(location.search).has("soon");
// the families whose songs are usually in a minor key
const MINOR = new Set(["hip hop", "drum and bass", "bass music", "techno and trance", "hard dance", "electronic", "house", "edm"]);
const NOTES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

const $ = (id) => document.getElementById(id);
const pick = (xs) => xs[Math.floor(Math.random() * xs.length)];
const said = (g) => (g === "dnb" ? "drum and bass" : g.replace(/_/g, " "));
const clock = (s) => `${Math.floor(Math.max(0, s) / 60)}:${String(Math.floor(Math.max(0, s) % 60)).padStart(2, "0")}`;

// ---- keys ----

function parseKey(text) {
  const m = /^\s*([A-G])([#b]?)\s*(.*)$/.exec(text || "");
  if (!m) return null;
  const base = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 }[m[1]];
  const tonic = (base + (m[2] === "#" ? 1 : m[2] === "b" ? 11 : 0)) % 12;
  // the modes with a minor third, by the engine's names for them (`harmonic_minor`), or the `m` of a chord's
  const mode = m[3].trim().toLowerCase().replace(/[\s-]+/g, "_");
  return { tonic, minor: /^(m|min|minor|aeolian|dorian|phrygian|locrian|harmonic_minor|melodic_minor)$/.test(mode) };
}

const keyName = (k) => `${NOTES[k.tonic]} ${k.minor ? "minor" : "major"}`;

/** a key the next song mixes into the playing one in: the same notes in the mode its family is usually in, or now and
 *  then a fifth up or down (a step on the circle of fifths) */
function nextKey(now, family) {
  const minor = MINOR.has(family);
  let k = now;
  if (Math.random() < 0.3) k = { tonic: (k.tonic + (Math.random() < 0.5 ? 7 : 5)) % 12, minor: k.minor };
  if (k.minor === minor) return k;
  return { tonic: (k.tonic + (minor ? 9 : 3)) % 12, minor };
}

// ---- the engine ----

const policy = window.trustedTypes
  ? window.trustedTypes.createPolicy("claudius-radio", { createScriptURL: (u) => (u === ENGINE ? u : "") })
  : null;

class Engine {
  constructor() {
    this.worker = new Worker(policy ? policy.createScriptURL(ENGINE) : ENGINE);
    this.listening = false;
    this.held = [];
    this.waiting = null;
    this.genres = [];
    this.ready = new Promise((resolve, reject) => {
      this.onReady = resolve;
      this.onFailed = reject;
    });
    this.worker.onmessage = (m) => this.heard(m.data);
    this.worker.onerror = () => {
      if (!this.genres.length) this.onFailed(new Error("the engine did not start"));
      const w = this.waiting;
      this.waiting = null;
      if (w) w.reject(new Error("the engine stopped"));
    };
  }

  post(json) {
    if (!this.listening) {
      this.held.push(json);
      return;
    }
    this.worker.postMessage({ json: JSON.stringify(json) });
  }

  heard(data) {
    // a message sent before the worker first speaks would be lost: what was held goes now
    if (!this.listening) {
      this.listening = true;
      for (const j of this.held.splice(0)) this.worker.postMessage({ json: JSON.stringify(j) });
    }
    let v;
    try {
      v = JSON.parse(data.json);
    } catch {
      return;
    }
    if (v.state && v.state.genres && v.state.genres.length && !this.genres.length) {
      this.genres = v.state.genres.filter((g) => !LEAVE_OUT.has(g.name));
      this.onReady();
    }
    const w = this.waiting;
    if (!w || !v.state) return;
    if (v.what === "new_song" && v.reply && !v.reply.ok) {
      this.waiting = null;
      w.reject(new Error(v.reply.message));
      return;
    }
    const r = v.state.render;
    if (!data.audio || !r || !v.state.song || r.version !== v.state.song_version) return;
    if (r.url === "preview:") {
      if (w.preview) w.preview(v.state, data.audio);
      return;
    }
    this.waiting = null;
    w.resolve({ state: v.state, audio: data.audio });
  }

  /** the song `spec` asks for, written and rendered whole; `preview` hears its first bars before */
  song(spec, preview) {
    return new Promise((resolve, reject) => {
      this.waiting = { resolve, reject, preview };
      const command = { cmd: "new_song", genre: spec.genre, seed: spec.seed, bpm: spec.bpm, sampled: false };
      if (spec.key) command.key = keyName(spec.key);
      this.post({ what: "new_song", command });
    });
  }
}

// ---- the decks ----

let ctx = null;
let master = null;
let echo = null;
let engine = null;

function wire() {
  ctx = new AudioContext({ latencyHint: "playback" });
  master = ctx.createGain();
  master.gain.value = Number($("volume").value);
  master.connect(ctx.destination);
  // the echo a song goes out on: a delay feeding back on itself, darker each time round
  const into = ctx.createGain();
  const delay = ctx.createDelay(4);
  const back = ctx.createGain();
  const dark = ctx.createBiquadFilter();
  dark.type = "lowpass";
  dark.frequency.value = 3500;
  back.gain.value = 0.55;
  into.connect(delay);
  delay.connect(dark);
  dark.connect(back);
  back.connect(delay);
  dark.connect(master);
  echo = { into, delay };
}

function buffer(audio) {
  const frames = audio.length / 2;
  const b = ctx.createBuffer(2, frames, RATE);
  const [l, r] = [b.getChannelData(0), b.getChannelData(1)];
  for (let i = 0; i < frames; i++) {
    l[i] = audio[2 * i];
    r[i] = audio[2 * i + 1];
  }
  return b;
}

/** a song on a deck of its own: its sound through a low shelf (its bass), a high-pass and a fader, and a send to the
 *  echo */
function deck(state, audio) {
  const song = state.song;
  const src = ctx.createBufferSource();
  src.buffer = buffer(audio);
  const low = ctx.createBiquadFilter();
  low.type = "lowshelf";
  low.frequency.value = 200;
  const thin = ctx.createBiquadFilter();
  thin.type = "highpass";
  thin.frequency.value = 10;
  const fader = ctx.createGain();
  const send = ctx.createGain();
  send.gain.value = 0;
  src.connect(low);
  low.connect(thin);
  thin.connect(fader);
  fader.connect(master);
  fader.connect(send);
  send.connect(echo.into);
  return {
    src,
    low,
    thin,
    fader,
    send,
    title: song.title,
    genre: song.genre,
    bpm: song.bpm,
    key: parseKey(song.key) || { tonic: 9, minor: true },
    keyText: song.key,
    bars: song.bars,
    sections: song.sections,
    seconds: src.buffer.duration,
    t0: 0,
  };
}

const bar = (d) => 240 / d.bpm;

/** stops a song's source at `at`: one that has already played out (a song over before the next was made, a first
 *  song's opening bars before the rest of it) throws in some browsers when stopped, and is left as it is */
function halt(d, at) {
  try {
    d.src.stop(at);
  } catch {
    // it has ended
  }
}

function start(d, at, offset = 0) {
  d.t0 = at - offset;
  d.src.start(at, offset);
  d.src.onended = () => {
    for (const n of [d.src, d.low, d.thin, d.fader, d.send]) n.disconnect();
  };
}

/** where a song starts to give way to the next: its outro when it has one of 8 bars or more, else its last 16 bars,
 *  never before its middle; and how many bars of it are left from there */
function leaving(d) {
  const last = d.sections[d.sections.length - 1];
  let from = last && last.kind === "outro" && last.bars >= 8 ? last.start_bar : d.bars - 16;
  from = Math.max(from, Math.ceil(d.bars / 2));
  return { at: from * bar(d), bars: d.bars - from };
}

/** an equal-power fade of `param` from `from` to `to` over `secs` from `at` */
function fade(param, from, to, at, secs) {
  const n = 64;
  const curve = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const x = i / (n - 1);
    curve[i] = to > from ? from + (to - from) * Math.sin((x * Math.PI) / 2) : to + (from - to) * Math.cos((x * Math.PI) / 2);
  }
  param.setValueCurveAtTime(curve, at, Math.max(secs, 0.01));
}

/** The next song `b` mixed into the playing one `a` as `how` says; when it comes in, in the audio clock's seconds. A
 *  song that came too late to blend on the outgoing one's outro comes in on its next bar line, after an echo. */
function mix(a, b, how) {
  const out = leaving(a);
  const soonest = Math.max(a.t0 + out.at, ctx.currentTime + 1);
  // on the outgoing song's bar lines, so that its last beat and the next song's first fall together
  const at = a.t0 + Math.ceil((soonest - a.t0) / bar(a) - 1e-6) * bar(a);
  const left = a.bars - Math.round((at - a.t0) / bar(a));
  if (how === "blend" && left >= 4) {
    // the next song's own bars, counted in the outgoing one's (twice as many when it is at twice the tempo)
    const intro = b.sections.length ? b.sections[0].bars : 16;
    const ratio = bar(b) / bar(a);
    let bars = Math.min(left, Math.floor(intro * ratio), 16);
    bars -= bars % 2;
    bars = Math.max(bars, 4);
    const span = bars * bar(a);
    const half = (bars / 2) * bar(a);
    const beat = bar(a) / 4;
    start(b, at);
    fade(b.fader.gain, 0, 1, at, half);
    b.low.gain.setValueAtTime(BASS_CUT_DB, at);
    b.low.gain.setValueAtTime(BASS_CUT_DB, at + half - beat);
    b.low.gain.linearRampToValueAtTime(0, at + half);
    a.low.gain.setValueAtTime(0, at + half - beat);
    a.low.gain.linearRampToValueAtTime(BASS_CUT_DB, at + half);
    fade(a.fader.gain, 1, 0, at + half, span - half);
    a.thin.frequency.setValueAtTime(10, at + half);
    a.thin.frequency.exponentialRampToValueAtTime(500, at + span);
    halt(a, at + span + 0.05);
    return { at, label: `mixed in beat for beat over ${bars} bars, in key`, blend: true };
  }
  if (how === "brake") {
    // the outgoing song stops like a record when its turntable is switched off, and the next drops in
    a.src.playbackRate.setValueAtTime(1, at);
    a.src.playbackRate.linearRampToValueAtTime(0.05, at + 0.9);
    a.fader.gain.setValueAtTime(1, at + 0.5);
    a.fader.gain.linearRampToValueAtTime(0, at + 0.9);
    halt(a, at + 1);
    start(b, at + 1);
    return { at: at + 1, label: "dropped in as the last record stopped" };
  }
  // an echo out: the outgoing song's last beat fed into the echo, the song cut on the bar, and the next coming in on
  // its own downbeat a bar later while the echoes die away
  const beat = bar(a) / 4;
  echo.delay.delayTime.setValueAtTime(0.75 * beat, at - beat);
  a.send.gain.setValueAtTime(0, at - beat);
  a.send.gain.linearRampToValueAtTime(1, at);
  a.fader.gain.setValueAtTime(1, at);
  a.fader.gain.linearRampToValueAtTime(0, at + 0.05);
  halt(a, at + 0.1);
  const next = at + bar(a);
  start(b, next);
  b.fader.gain.setValueAtTime(0, next);
  b.fader.gain.linearRampToValueAtTime(1, next + bar(b));
  b.low.gain.setValueAtTime(BASS_CUT_DB, next);
  b.low.gain.setValueAtTime(BASS_CUT_DB, next + 2 * bar(b) - 0.05);
  b.low.gain.linearRampToValueAtTime(0, next + 2 * bar(b));
  return { at: next, label: "in as the last song echoed out" };
}

// ---- the station ----

let onAir = null;
let next = null;
let planned = null;
const recent = [];

/** what to play after `now`: most of the time a genre that mixes into it beat for beat, at its tempo (or twice or half
 *  of it) and in a key of the same notes; else another tempo and key, which comes in after an echo or a stop */
function plan(now) {
  const pool = engine.genres.filter((g) => g.name !== now.genre && !recent.includes(g.name));
  const seed = 1 + Math.floor(Math.random() * 999_999);
  const fits = (g, bpm) => bpm >= g.bpm[0] * 0.98 && bpm <= g.bpm[1] * 1.02;
  const mixable = [];
  for (const g of pool) {
    for (const f of [1, 2, 0.5]) if (fits(g, now.bpm * f)) mixable.push({ g, bpm: Math.round(now.bpm * f * 10) / 10 });
  }
  if (mixable.length && Math.random() < BLEND) {
    const m = pick(mixable);
    return { genre: m.g.name, bpm: m.bpm, key: nextKey(now.key, m.g.family), seed, how: "blend" };
  }
  const g = pick(pool.length ? pool : engine.genres);
  const bpm = Math.round(g.bpm[0] + Math.random() * (g.bpm[1] - g.bpm[0]));
  const key = { tonic: Math.floor(Math.random() * 12), minor: MINOR.has(g.family) };
  return { genre: g.name, bpm, key, seed, how: Math.random() < 0.2 ? "brake" : "echo" };
}

function remember(genre) {
  recent.push(genre);
  if (recent.length > 4) recent.shift();
}

function status(text) {
  $("status").textContent = text.charAt(0).toUpperCase() + text.slice(1);
}

/** `text` in the element `id`, set only when it changes, so that the song's title is announced when a new one is on
 *  and the clock never is */
function show(id, text) {
  const el = $(id);
  if (el.textContent !== text) el.textContent = text;
}

async function queue() {
  const spec = plan(onAir);
  planned = spec;
  remember(spec.genre);
  try {
    const song = await engine.song(spec);
    const b = deck(song.state, song.audio);
    const m = mix(onAir, b, spec.how);
    next = { deck: b, at: m.at, label: `${said(b.genre)}, ${m.label}`, blend: m.blend };
    planned = null;
  } catch (e) {
    status(`The next song could not be made (${e.message}); trying another.`);
    planned = null;
    setTimeout(queue, 1000);
  }
}

function addPlayed(d) {
  const li = document.createElement("li");
  li.textContent = `${d.title}: ${said(d.genre)}, ${Math.round(d.bpm)} BPM, ${d.keyText}`;
  $("played").prepend(li);
  while ($("played").children.length > 12) $("played").lastElementChild.remove();
}

function tick() {
  if (!ctx) return;
  const t = ctx.currentTime;
  if (next && t >= next.at) {
    addPlayed(onAir);
    onAir = next.deck;
    status(next.label);
    next = null;
    queue();
  }
  if (!onAir) return;
  const at = t - onAir.t0;
  show("now-title", onAir.title);
  show("now-meta", `${said(onAir.genre)} · ${Math.round(onAir.bpm * 10) / 10} BPM · ${onAir.keyText}`);
  $("now-bar").style.width = `${Math.min(100, (100 * at) / onAir.seconds)}%`;
  show("now-time", `${clock(at)} / ${clock(onAir.seconds)}`);
  if (next) {
    const d = next.deck;
    const how = next.blend ? "mixes in" : "comes in";
    show("next", `${d.title}: ${said(d.genre)}, ${Math.round(d.bpm)} BPM, ${d.keyText}; ${how} ${clock(next.at - t)} from now`);
  } else if (planned) {
    show("next", `writing a ${said(planned.genre)} song at ${planned.bpm} BPM in ${keyName(planned.key)}…`);
  }
}

async function tuneIn() {
  const button = $("tune");
  if (ctx) {
    if (ctx.state === "running") {
      await ctx.suspend();
      button.textContent = "Play";
    } else {
      await ctx.resume();
      button.textContent = "Pause";
    }
    return;
  }
  button.disabled = true;
  wire();
  engine = new Engine();
  status("Starting the engine…");
  await engine.ready;
  const g = pick(engine.genres.filter((x) => OPENERS.includes(x.name)));
  const spec = { genre: g.name, bpm: Math.round((g.bpm[0] + g.bpm[1]) / 2), seed: 1 + Math.floor(Math.random() * 999_999) };
  remember(spec.genre);
  status(`Writing a ${said(spec.genre)} song at ${spec.bpm} BPM…`);
  let early = null;
  const song = await engine.song(spec, (state, audio) => {
    // the first bars, to play while the rest renders
    if (early || SOON) return;
    early = deck(state, audio);
    start(early, ctx.currentTime + 0.1);
    onAir = early;
    status("On air while the rest of it renders");
  });
  const whole = deck(song.state, song.audio);
  if (early) {
    // the whole song takes over where its first bars are (or where they ended, when they ran out before it came),
    // with a crossfade too short to hear
    const at = ctx.currentTime + 0.15;
    start(whole, at, Math.min(at - early.t0, early.seconds - 0.05, whole.seconds - 1));
    whole.fader.gain.setValueAtTime(0, at);
    whole.fader.gain.linearRampToValueAtTime(1, at + 0.05);
    early.fader.gain.setValueAtTime(1, at);
    early.fader.gain.linearRampToValueAtTime(0, at + 0.05);
    halt(early, at + 0.1);
  } else {
    start(whole, ctx.currentTime + 0.1, SOON ? Math.max(0, leaving(whole).at - 20) : 0);
  }
  onAir = whole;
  status("On air");
  button.disabled = false;
  button.textContent = "Pause";
  queue();
}

/** what a start that failed set up, let go of, so that Tune in starts the station afresh rather than pausing silence */
function reset() {
  if (engine) engine.worker.terminate();
  if (ctx) ctx.close();
  engine = ctx = master = echo = null;
  onAir = next = planned = null;
}

$("tune").addEventListener("click", () => {
  tuneIn().catch((e) => {
    reset();
    status(`The radio could not start: ${e.message}`);
    $("tune").disabled = false;
    $("tune").textContent = "Tune in";
  });
});
$("volume").addEventListener("input", (e) => {
  if (master) master.gain.setTargetAtTime(Number(e.target.value), ctx.currentTime, 0.05);
});
setInterval(tick, 250);
