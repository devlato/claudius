// claudius on a phone: pads, keys and a turntable that play the song's own sounds here (so a tap sounds at once,
// whatever the Wi-Fi does), follow the studio's transport, and send what was played back into the song as a take. The
// page reaches its claudius over HTTP when claudius serves it, or, with `?room=`, the studio demo in a browser tab,
// through a room of the site's relay (site/worker/room.js), where each request is a message with an answer back.
"use strict";

const $ = (id) => document.getElementById(id);

/** the seconds of the record one turn of the platter plays: 33⅓ turns a minute */
const TURN = 1.8;
/** how many bars of a track a record is, at most (a shorter section gives a shorter one) */
const RECORD_BARS = 2;

let ctx = null;
let song = null;
let mode = "pads";
const hits = new Map();

/** what is said of the studio being out of reach: cleared once it answers again */
const LOST = "no lost";

/** how a failure is said: as the studio out of reach (the link's own failures, and a network's), or as what it is */
function trouble(e) {
  return e.lost || e instanceof TypeError ? LOST : "no";
}

/** a failure of the link to the demo: none of the demo's own answers */
function unreachable(why) {
  return Object.assign(new Error(why), { lost: true });
}

/** clears what was said of the studio being out of reach */
function found() {
  if ($("said").classList.contains("lost")) say("");
}

function say(text, kind) {
  const f = $("said");
  f.textContent = text || "";
  f.className = kind || "";
}

/** says what is loading, unless what a take did is being said; the function it returns takes the words back, unless
 * something else has been said since */
function loading(text) {
  if (!$("said").className) say(text);
  return () => {
    if ($("said").textContent === text) say("");
  };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const ROOM = new URLSearchParams(location.search).get("room");
const link = ROOM
  ? relay(ROOM)
  : {
      get: (path) => fetch(path, { cache: "no-store" }),
      post: (path, body) =>
        fetch(path, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) }),
    };

/** an answer that came through the relay, read as fetch's are */
function answer(status, body) {
  return {
    ok: status < 400,
    status,
    json: async () => (typeof body === "string" ? JSON.parse(body) : body),
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    arrayBuffer: async () => body,
  };
}

/**
 * Requests to the demo in room `room`, as messages `{id, method, path, body}`; it answers each with `{id, status,
 * body}`, or a sound with the request's id (4 bytes, little-endian) before it. While the room says the demo is gone,
 * a request waits a few seconds, in case the tab comes back.
 */
function relay(room) {
  const waiting = new Map();
  /** what failed waiting for the demo, by id: a take the demo answers after all is still news */
  const failed = new Map();
  let ws = null;
  let opening = null;
  let next = 1;
  let gone = false;
  const rearm = (id, ms) => {
    const w = waiting.get(id);
    clearTimeout(w.timer);
    w.short = ms < 120_000;
    w.timer = setTimeout(() => fail(id, gone ? "the demo is not open" : "the demo did not answer"), ms);
  };
  const fail = (id, why) => {
    const w = waiting.get(id);
    if (!w) return;
    waiting.delete(id);
    clearTimeout(w.timer);
    failed.set(id, w.path);
    if (failed.size > 64) failed.delete(failed.keys().next().value);
    w.no(unreachable(why));
  };
  const settle = (id, status, body) => {
    const w = waiting.get(id);
    if (!w) {
      // the demo kept its answer while it was away: a take it made after all is said
      if (failed.get(id) === "api/take" && body && typeof body.message === "string") say(`the demo got the take: ${body.message}`, body.ok ? "ok" : "no");
      failed.delete(id);
      return;
    }
    waiting.delete(id);
    clearTimeout(w.timer);
    if (gone) {
      // what was asked while it was away waits as long as anything now
      gone = false;
      for (const [other, x] of waiting) if (x.short) rearm(other, 120_000);
      back();
    }
    w.ok(answer(status, body));
  };
  const open = () =>
    new Promise((ok, no) => {
      const s = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/relay/${room}?as=phone`);
      s.binaryType = "arraybuffer";
      s.onopen = () => {
        ws = s;
        ok();
      };
      s.onerror = () => no(unreachable("the site's relay cannot be reached"));
      s.onclose = () => {
        ws = null;
        for (const id of [...waiting.keys()]) fail(id, "the connection to the demo closed");
      };
      s.onmessage = (e) => {
        if (typeof e.data !== "string") return settle(new DataView(e.data).getUint32(0, true), 200, e.data.slice(4));
        const m = JSON.parse(e.data);
        if (m.event === "gone") {
          // nothing that waits is answered by a demo that is not there: each asks again, and is answered once it is
          gone = true;
          for (const id of [...waiting.keys()]) fail(id, "the demo is not open");
          say("the demo is not open: open it again and scan its code", LOST);
        } else if (m.id) settle(m.id, m.status, m.body);
      };
    });
  async function ask(method, path, body) {
    if (!ws) {
      opening ??= open().finally(() => (opening = null));
      await opening;
    }
    const id = next++;
    return new Promise((ok, no) => {
      waiting.set(id, { ok, no, path: path.split("?")[0], short: gone });
      ws.send(JSON.stringify({ id, method, path, body }));
      // the demo's engine renders a whole song at a time, and what a phone asks for meanwhile waits for it; while the
      // demo is away, the room drops what is asked
      rearm(id, gone ? 5000 : 120_000);
    });
  }
  return { get: (path) => ask("GET", path), post: (path, body) => ask("POST", path, body) };
}

/** the demo answers again: what did not load while it was away is asked for again */
function back() {
  found();
  if (!ctx || !song) return;
  if (!keys.buf) keys.name = "";
  if (!deck.buf) deck.name = "";
  loadSounds();
}

async function sound(url) {
  const r = await link.get(url);
  if (!r.ok) throw new Error(await r.text());
  return ctx.decodeAudioData(await r.arrayBuffer());
}

/** plays `buf` (from `from` seconds, for `secs`) at `v`, `rate` times as fast; returns what stops it */
function play(buf, v, rate = 1, from = 0, secs = undefined) {
  const src = ctx.createBufferSource();
  src.buffer = buf;
  src.playbackRate.value = rate;
  const g = ctx.createGain();
  g.gain.value = v;
  src.connect(g).connect(ctx.destination);
  src.start(0, from, secs);
  return { src, g };
}

// The clock. The server's time in this page's (performance.now()), from the quickest of the last few round trips, and
// the studio's transport: where the song was at a time on the server's clock.

const trips = [];
let offset = 0;
let transport = { playing: false, at: 0, since: 0 };
/** since when the clock has not been reached (the page's time), while it has not */
let lost = null;
/** when the clock last answered (the page's time) */
let reached = 0;

async function tick() {
  const t0 = performance.now();
  try {
    const r = await link.get("api/clock");
    if (!r.ok) throw new Error(await r.text());
    const c = await r.json();
    const t1 = performance.now();
    trips.push({ rtt: t1 - t0, offset: c.now - (t0 + t1) / 2 });
    if (trips.length > 12) trips.shift();
    offset = trips.reduce((a, b) => (b.rtt < a.rtt ? b : a)).offset;
    transport = c;
    lost = null;
    reached = t1;
    found();
  } catch (e) {
    // what failed after an answer came for something asked since says nothing of the studio now
    if (t0 < reached) return;
    // a blip on the Wi-Fi keeps the song going as it was; only a studio gone for a while stops it here
    lost ??= performance.now();
    if (performance.now() - lost > 5000) transport = { playing: false, at: 0, since: 0 };
    say(`cannot reach the studio: ${e.message}`, LOST);
  }
}

/** where the song is, in seconds, at `t` on this page's clock; null while the studio is stopped */
function songAt(t) {
  if (!transport.playing) return null;
  return transport.at + (t + offset - transport.since) / 1000;
}

/** how long after a tap its sound is heard, in milliseconds */
function latency() {
  return ctx ? ((ctx.baseLatency || 0) + (ctx.outputLatency || 0)) * 1000 : 0;
}

/** an event's time on this page's clock */
function when(e) {
  return e.timeStamp > 0 && e.timeStamp < 1e11 ? e.timeStamp : performance.now();
}

/** the section the song is in at `secs` */
function sectionAt(secs) {
  const bar = Math.floor((secs * song.bpm) / 240);
  let at = song.sections[0];
  for (const s of song.sections) if (s.start_bar <= bar) at = s;
  return at;
}

// The song: what it has to play, and its sounds, fetched again whenever it changes.

async function watch() {
  for (;;) {
    try {
      const r = await link.get(`api/song?after=${song ? song.version : 0}`);
      if (r.status === 404) {
        song = null;
        $("title").textContent = "No song yet";
        $("where").textContent = "start one in the studio";
        $("rec").disabled = true;
        await sleep(2000);
        continue;
      }
      if (!r.ok) throw new Error(await r.text());
      const s = await r.json();
      if (!song || s.version !== song.version) open(s);
    } catch (e) {
      say(`cannot reach the studio: ${e.message}`, LOST);
      await sleep(3000);
    }
  }
}

function open(s) {
  song = s;
  $("title").textContent = s.title;
  $("rec").disabled = false;
  buildPads();
  fillSounds(s);
  fill($("track"), s.tracks, (x) => [x, x], s.tracks.filter((t) => !s.lanes.includes(t)));
  fillFrom();
  hits.clear();
  keys.name = "";
  deck.name = "";
  if (ctx) loadSounds();
}

/** the sounds the keys can play: the song's parts' own first, then the synths and the instruments */
function fillSounds(s) {
  const select = $("sound");
  const had = select.value;
  select.textContent = "";
  const group = (label, items) => {
    const g = document.createElement("optgroup");
    g.label = label;
    for (const [value, text] of items) {
      const o = document.createElement("option");
      o.value = value;
      o.textContent = text;
      g.append(o);
    }
    select.append(g);
  };
  group("In the song", s.parts.map(([part, sound]) => [sound, `${part}: ${sound}`]));
  group("Synths", s.synths.map((x) => [x, x]));
  group("Instruments", s.instruments.map((x) => [x, x]));
  const all = [...s.parts.map(([, x]) => x), ...s.synths, ...s.instruments];
  const keysPart = s.parts.find(([part]) => /keys|piano|chords/.test(part));
  const pick = [had, keysPart && keysPart[1], "epiano", all[0]].find((x) => x && all.includes(x));
  if (pick) select.value = pick;
}

/** the sections a record can be cut from: those the chosen track plays in */
function fillFrom() {
  const track = $("track").value;
  const playing = song.sections.filter((x) => x.layers.includes(track));
  fill($("from"), playing.length ? playing : song.sections, (x) => [x.start_bar, `${x.name}, bar ${x.start_bar + 1}`], []);
}

/** `select` offering `items` (as [value, label]), keeping what it had, or else the first of `liked` there is */
function fill(select, items, entry, liked) {
  const had = select.value;
  select.textContent = "";
  for (const x of items) {
    const [value, label] = entry(x);
    const o = document.createElement("option");
    o.value = value;
    o.textContent = label;
    select.append(o);
  }
  const values = items.map((x) => String(entry(x)[0]));
  const pick = values.includes(had) ? had : liked.map(String).find((v) => values.includes(v));
  if (pick !== undefined) select.value = pick;
}

async function loadSounds() {
  if (!song) return;
  const v = song.version;
  await Promise.all(
    song.lanes
      .filter((lane) => !hits.has(lane))
      .map(async (lane) => {
        try {
          const b = await sound(`api/hits/${encodeURIComponent(lane)}`);
          if (song && song.version === v) hits.set(lane, b);
        } catch (e) {
          say(`no sound for ${lane}: ${e.message}`, trouble(e));
        }
      }),
  );
  if (mode === "keys") loadKeys();
  if (mode === "deck") loadRecord();
}

// Pads: a lane each, struck harder nearer the top.

const HUES = [
  ["kick", 8],
  ["808", 8],
  ["snare", 205],
  ["clap", 285],
  ["rim", 250],
  ["hat", 48],
  ["hh", 48],
  ["oh", 60],
  ["ride", 70],
  ["crash", 80],
  ["tom", 160],
  ["perc", 130],
  ["shaker", 100],
];

function buildPads() {
  const box = $("pads");
  box.textContent = "";
  const n = song.lanes.length;
  box.style.setProperty("--cols", n <= 4 ? 2 : n <= 9 ? 3 : 4);
  for (const lane of song.lanes) {
    const pad = document.createElement("div");
    pad.className = "pad";
    pad.textContent = lane;
    const hue = HUES.find(([k]) => lane.toLowerCase().includes(k));
    pad.style.setProperty("--hue", hue ? hue[1] : 30);
    pad.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      strike(lane, pad, e);
    });
    box.append(pad);
  }
}

function strike(lane, pad, e) {
  if (!ctx) return;
  const r = pad.getBoundingClientRect();
  const v = Math.min(1, Math.max(0.3, 1 - (0.7 * (e.clientY - r.top)) / r.height));
  const b = hits.get(lane);
  if (b) play(b, v);
  pad.classList.add("hit");
  setTimeout(() => pad.classList.remove("hit"), 90);
  const at = songAt(when(e) + latency());
  if (take && take.mode === "pads" && at !== null) take.hits.push({ lane, at, v });
}

// Keys: two octaves of the sound chosen, rendered a note every three semitones and pitched to the keys between.

let octave = 3;
const keys = { name: "", low: 0, buf: null };
const held = new Map();
const BLACK = [1, 3, 6, 8, 10];

function buildKeys() {
  const kb = $("keyboard");
  kb.textContent = "";
  const low = 12 * (octave + 1);
  const whites = [];
  for (let p = low; p <= low + 24; p++) if (!BLACK.includes(p % 12)) whites.push(p);
  const w = 100 / whites.length;
  whites.forEach((p, i) => {
    const k = document.createElement("div");
    k.className = "white";
    k.dataset.p = p;
    k.textContent = p % 12 === 0 ? `C${p / 12 - 1}` : "";
    kb.append(k);
    if (i < whites.length - 1 && BLACK.includes((p + 1) % 12)) {
      const b = document.createElement("div");
      b.className = "black";
      b.dataset.p = p + 1;
      b.style.left = `${(i + 0.68) * w}%`;
      b.style.width = `${w * 0.64}%`;
      kb.append(b);
    }
  });
  $("octave").textContent = `C${octave}`;
}

async function loadKeys() {
  const s = $("sound").value;
  if (!s || !song || !ctx) return;
  const low = 12 * (octave + 1) - 1;
  const name = `${s}:${low}:${song.version}`;
  if (keys.name === name) return;
  Object.assign(keys, { name, low, buf: null });
  const done = loading(`getting ${s} ready…`);
  try {
    const b = await sound(`api/keys?sound=${encodeURIComponent(s)}&low=${low}&high=${low + 27}&step=3`);
    if (keys.name === name) keys.buf = b;
    done();
  } catch (e) {
    say(e.message, trouble(e));
  }
}

/** the pointer's moves go to `el` until it lets go, where the browser allows it */
function capture(el, e) {
  try {
    el.setPointerCapture(e.pointerId);
  } catch {
    // a pointer the browser no longer tracks: its moves go where they fall
  }
}

function keyAt(e) {
  const el = document.elementFromPoint(e.clientX, e.clientY);
  return el && el.dataset && el.dataset.p ? el : null;
}

function noteOn(id, el, e) {
  if (!keys.buf || !ctx) return;
  const p = Number(el.dataset.p);
  const r = el.getBoundingClientRect();
  const v = Math.min(1, 0.45 + (0.55 * (e.clientY - r.top)) / r.height);
  const i = Math.max(0, Math.round((p - keys.low) / 3));
  const voice = play(keys.buf, v, Math.pow(2, (p - (keys.low + 3 * i)) / 12), i * song.slot, song.slot);
  const at = songAt(when(e) + latency());
  const note = take && take.mode === "keys" && at !== null ? { at, d: null, p, v } : null;
  if (note) take.notes.push(note);
  held.set(id, { el, voice, note });
  el.classList.add("down");
}

function noteOff(id, t) {
  const n = held.get(id);
  if (!n) return;
  held.delete(id);
  n.el.classList.remove("down");
  const now = ctx.currentTime;
  n.voice.g.gain.setTargetAtTime(0, now, 0.05);
  n.voice.src.stop(now + 0.4);
  if (n.note) {
    const end = songAt(t + latency());
    n.note.d = end === null ? 0.25 : Math.max(0.03, end - n.note.at);
  }
}

function keyboard() {
  const kb = $("keyboard");
  kb.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    capture(kb, e);
    const k = keyAt(e);
    if (k) noteOn(e.pointerId, k, e);
  });
  kb.addEventListener("pointermove", (e) => {
    const n = held.get(e.pointerId);
    const k = keyAt(e);
    if (n && k && k !== n.el) {
      noteOff(e.pointerId, when(e));
      noteOn(e.pointerId, k, e);
    }
  });
  for (const up of ["pointerup", "pointercancel"]) kb.addEventListener(up, (e) => noteOff(e.pointerId, when(e)));
  $("down").onclick = () => shift(-1);
  $("up").onclick = () => shift(1);
  $("sound").onchange = loadKeys;
}

function shift(by) {
  octave = Math.min(6, Math.max(1, octave + by));
  buildKeys();
  loadKeys();
}

// The turntable: a record of two bars of a track, moved by the finger (one turn is 1.8 s of it), let go to run on with
// the motor (or come to rest without it), heard through the crossfader. The sound is read along the record as it
// moves, as `claudius::scratch::render` reads it again for the take.

const deck = {
  name: "",
  buf: null,
  track: "",
  bar: 0,
  bars: RECORD_BARS,
  pos: 0,
  vel: 0,
  target: 0,
  touching: false,
  angle: 0,
  turned: 0,
  start: 0,
  gain: 1,
  dc: [0, 0, 0, 0],
  node: null,
};

async function loadRecord() {
  const track = $("track").value;
  const bar = Number($("from").value || 0);
  if (!track || !song || !ctx) return;
  const section = song.sections.find((s) => s.start_bar === bar);
  const bars = Math.min(RECORD_BARS, section ? section.bars : RECORD_BARS);
  const name = `${track}:${bar}:${bars}:${song.version}`;
  if (deck.name === name) return;
  Object.assign(deck, { name, buf: null, pos: 0, vel: 0 });
  const done = loading(`cutting a record of ${track}…`);
  try {
    const b = await sound(`api/record?track=${encodeURIComponent(track)}&bar=${bar}&bars=${bars}`);
    if (deck.name === name) Object.assign(deck, { buf: b, track, bar, bars });
    done();
  } catch (e) {
    say(e.message, trouble(e));
  }
  if (!deck.node) {
    deck.node = ctx.createScriptProcessor(1024, 0, 2);
    deck.node.onaudioprocess = spin;
    deck.node.connect(ctx.destination);
  }
}

/** an audio block of the turntable: the record read from where it was to where it is now */
function spin(e) {
  const L = e.outputBuffer.getChannelData(0);
  const R = e.outputBuffer.getChannelData(1);
  const b = deck.buf;
  if (!b) {
    L.fill(0);
    R.fill(0);
    return;
  }
  const n = L.length;
  const dt = n / ctx.sampleRate;
  const from = deck.pos;
  let to;
  if (deck.touching) {
    to = deck.target;
    deck.vel = (to - from) / dt;
  } else {
    deck.vel += (($("motor").checked ? 1 : 0) - deck.vel) * Math.min(1, dt / 0.2);
    to = from + deck.vel * dt;
  }
  const fader = Number($("fader").value);
  const l = b.getChannelData(0);
  const r = b.numberOfChannels > 1 ? b.getChannelData(1) : l;
  const len = l.length;
  const rate = b.sampleRate;
  const dc = deck.dc;
  for (let i = 0; i < n; i++) {
    const w = i / n;
    const at = (from + (to - from) * w) * rate;
    const j = ((Math.floor(at) % len) + len) % len;
    const k = (j + 1) % len;
    const f = at - Math.floor(at);
    const g = deck.gain + (fader - deck.gain) * w;
    const x = (l[j] + (l[k] - l[j]) * f) * g;
    const y = (r[j] + (r[k] - r[j]) * f) * g;
    // a still record is silent: no direct current from a sample held
    L[i] = dc[1] = x - dc[0] + 0.9987 * dc[1];
    dc[0] = x;
    R[i] = dc[3] = y - dc[2] + 0.9987 * dc[3];
    dc[2] = y;
  }
  if (take && take.mode === "deck") {
    const at = songAt(heardAt(e.playbackTime));
    const last = take.moves[take.moves.length - 1];
    if (at !== null && (!last || at > last.at)) take.moves.push({ at, pos: from, fader: deck.gain });
  }
  deck.gain = fader;
  deck.pos = to;
}

/** the time on this page's clock at which the audio context's `t` is heard */
function heardAt(t) {
  if (ctx.getOutputTimestamp) {
    const s = ctx.getOutputTimestamp();
    if (s.performanceTime > 0) return s.performanceTime + (t - s.contextTime) * 1000;
  }
  return performance.now() + (t - ctx.currentTime) * 1000 + latency();
}

function angleOf(e, c) {
  const r = c.getBoundingClientRect();
  return Math.atan2(e.clientY - r.top - r.height / 2, e.clientX - r.left - r.width / 2);
}

function platter() {
  const c = $("platter");
  c.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    capture(c, e);
    Object.assign(deck, { touching: true, angle: angleOf(e, c), turned: 0, start: deck.pos, target: deck.pos });
  });
  c.addEventListener("pointermove", (e) => {
    if (!deck.touching) return;
    const a = angleOf(e, c);
    let d = a - deck.angle;
    if (d > Math.PI) d -= 2 * Math.PI;
    if (d < -Math.PI) d += 2 * Math.PI;
    deck.angle = a;
    deck.turned += d;
    deck.target = deck.start + (deck.turned / (2 * Math.PI)) * TURN;
  });
  for (const up of ["pointerup", "pointercancel"]) c.addEventListener(up, () => (deck.touching = false));
  $("track").onchange = () => {
    fillFrom();
    loadRecord();
  };
  $("from").onchange = loadRecord;
}

function drawPlatter() {
  const c = $("platter");
  const w = c.clientWidth * devicePixelRatio;
  const h = c.clientHeight * devicePixelRatio;
  if (c.width !== w || c.height !== h) Object.assign(c, { width: w, height: h });
  const g = c.getContext("2d");
  g.clearRect(0, 0, w, h);
  const rad = Math.min(w, h) / 2 - 6;
  g.save();
  g.translate(w / 2, h / 2);
  g.rotate((deck.pos / TURN) * 2 * Math.PI);
  g.fillStyle = "#08090b";
  g.beginPath();
  g.arc(0, 0, rad, 0, 2 * Math.PI);
  g.fill();
  g.strokeStyle = "#1d2027";
  for (let k = 0.42; k < 0.98; k += 0.045) {
    g.beginPath();
    g.arc(0, 0, rad * k, 0, 2 * Math.PI);
    g.stroke();
  }
  g.fillStyle = "#f0a83a";
  g.beginPath();
  g.arc(0, 0, rad * 0.34, 0, 2 * Math.PI);
  g.fill();
  g.fillStyle = "#101217";
  g.font = `600 ${Math.round(rad * 0.09)}px -apple-system, sans-serif`;
  g.textAlign = "center";
  g.fillText(deck.track || "no record", 0, -rad * 0.12);
  g.fillRect(-2, rad * 0.42, 4, rad * 0.5);
  g.restore();
}

// Takes: ● starts one along with the song, and stops it and sends it as the command it makes.

let take = null;

function record() {
  if (take) return send();
  const at = songAt(performance.now() + latency());
  if (!song || at === null) return say("play the song in the studio first: a take records along with it", "no");
  take = { mode, from: at, hits: [], notes: [], moves: [], track: deck.track, bar: deck.bar, bars: deck.bars };
  take.sound = $("sound").value;
  take.role = $("role").value;
  $("rec").classList.add("taking");
  say("recording: tap ● again to send the take");
}

async function send() {
  const t = take;
  take = null;
  $("rec").classList.remove("taking");
  for (const [id] of held) noteOff(id, performance.now());
  const end = songAt(performance.now() + latency());
  const sec = sectionAt(t.from);
  const start = (sec.start_bar * 240) / song.bpm;
  const beats = (s) => ((s - start) * song.bpm) / 60;
  const round = (x, by = 1e4) => Math.round(x * by) / by;
  let cmd;
  if (t.mode === "pads") {
    const played = t.hits.filter((h) => h.at >= start).map((h) => ({ lane: h.lane, t: round(beats(h.at)), v: round(h.v, 100) }));
    if (!played.length) return say("nothing was played", "no");
    const [first, last] = [played[0].t, played[played.length - 1].t];
    const until = round(Math.max(end === null ? 0 : beats(end), last + 0.25));
    cmd = { cmd: "add_hits", from: sec.name, hits: played, start: round(Math.min(Math.max(0, beats(t.from)), first)), beats: until };
  } else if (t.mode === "keys") {
    const notes = t.notes
      .filter((n) => n.at >= start)
      .map((n) => ({ t: round(beats(n.at)), d: round(Math.max(1 / 16, (n.d * song.bpm) / 60)), p: n.p, v: round(n.v, 100) }));
    if (!notes.length) return say("nothing was played", "no");
    cmd = { cmd: "add_notes", from: sec.name, notes, sounds: [t.sound], role: t.role };
  } else {
    const moves = t.moves
      .filter((m) => m.at >= start)
      .map((m) => ({ t: round(m.at - start), at: round(m.pos), fader: round(m.fader, 1000) }));
    if (moves.length < 2) return say("the record did not move", "no");
    if (moves[0].t > 0) moves.unshift({ t: 0, at: moves[0].at, fader: 0 });
    cmd = { cmd: "add_scratch", from: sec.name, track: t.track, bar: t.bar, bars: t.bars, moves };
  }
  say("sending the take…");
  try {
    const r = await link.post("api/take", cmd);
    const reply = r.ok ? await r.json() : { ok: false, message: await r.text() };
    say(reply.message, reply.ok ? "ok" : "no");
  } catch (e) {
    say(`the take did not get to the studio: ${e.message}`, "no");
  }
}

// What the page shows as the song goes: the section and bar, the beat, the record turning.

function frame() {
  if (song) {
    const at = songAt(performance.now() + latency());
    const dots = $("beat").children;
    if (at === null) {
      $("where").textContent = "stopped: play the song in the studio";
      for (const d of dots) d.classList.remove("on");
    } else {
      const beat = (at * song.bpm) / 60;
      $("where").textContent = `${sectionAt(at).name}, bar ${Math.floor(beat / 4) + 1}`;
      [...dots].forEach((d, i) => d.classList.toggle("on", i === ((Math.floor(beat) % 4) + 4) % 4));
    }
  }
  if (mode === "deck") drawPlatter();
  requestAnimationFrame(frame);
}

function choose(m) {
  mode = m;
  for (const b of document.querySelectorAll("nav button")) b.classList.toggle("on", b.dataset.mode === m);
  for (const s of document.querySelectorAll(".mode")) s.classList.toggle("on", s.id === m);
  if (m === "keys") {
    buildKeys();
    loadKeys();
  }
  if (m === "deck") loadRecord();
}

async function start() {
  const Ctx = window.AudioContext || window.webkitAudioContext;
  // the sound goes on with the ring switch on silent (Safari)
  if (navigator.audioSession) navigator.audioSession.type = "playback";
  ctx = new Ctx({ latencyHint: "interactive" });
  await ctx.resume();
  $("start").classList.add("gone");
  loadSounds();
  if (mode !== "pads") choose(mode);
}

addEventListener("DOMContentLoaded", () => {
  for (const b of document.querySelectorAll("nav button")) b.onclick = () => choose(b.dataset.mode);
  $("rec").onclick = record;
  $("go").onclick = start;
  keyboard();
  platter();
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden && ctx) ctx.resume();
  });
  // a few quick round trips to set the clock, then one a second
  (async () => {
    for (let i = 0; i < 5; i++) {
      await tick();
      await sleep(150);
    }
    setInterval(tick, 1000);
  })();
  watch();
  requestAnimationFrame(frame);
});
