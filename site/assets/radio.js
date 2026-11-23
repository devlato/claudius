// Infinite Radio: the studio demo's engine (claudius itself, in a Web Worker) writes and renders a song at a time,
// one genre after another, and this page plays them on two decks, mixing each into the next as a DJ would.
//
// The next song is asked for as soon as one goes on air. Most of the time its genre is one whose tempo range takes
// the playing song's tempo, or twice or half of it, and it is written at exactly that tempo and in a key with the same
// notes as the playing one's, so the two mix beat for beat and in key: it comes in under the outgoing song's outro on
// a bar line, its bass cut, the basses swap halfway through, and the outgoing song is thinned out and faded. A song in
// another tempo comes in after the outgoing one echoes out, or now and then after it stops like a record on a
// turntable. Every change is scheduled on the audio clock when the song arrives, so it lands on the beat even in a
// tab that is not being looked at.
//
// The station keeps to the genres chosen (`?genres=house,techno`, or the page's list), skips to the next song on the
// next beat, hands the song on air over as a file (encoded in the engine's worker, as the studio exports), and opens it
// in the studio demo, which writes the same song again from what the station asked for, on the same recordings when it
// samples. A song of a genre built on samples (trip hop, boom bap, lo-fi ...) is: before the engine writes it, the page
// finds a few recordings through the site's search, the engine chooses those worth fetching and cuts them, and the song
// on air credits those it plays.

const ENGINE = "/studio/claudius-engine_loader.js";
const RATE = 48000;
// what the station leaves out: no beat to mix on, or too harsh for a radio
const LEAVE_OUT = new Set(["ambient", "breakcore", "gabber", "hard_techno", "hardstyle", "idm"]);
// the genres the first song is drawn from
const OPENERS = ["house", "deep_house", "nu_disco", "hip_hop", "lofi", "afrobeats", "dnb", "synthwave", "trap", "funk"];
// how often the next song is one that mixes into the playing one beat for beat
const BLEND = 0.75;
// how many of the songs of a genre built on samples sample (the rest are written on its own sounds alone), and how
// often a station kept to two genres or more plays a song fusing two of them
const SAMPLED = 2 / 3;
const FUSED = 1 / 6;
// the bass of a song coming in is cut this far, and the outgoing one's once they swap
const BASS_CUT_DB = -26;
// a DJ mixer's middle band, round 1 kHz, cut this far while a song's mids wait for the swap or after it
const MIDS_CUT_DB = -12;
// the families DJs blend over long beat-only intros and outros, and how long a blend runs at most, in the outgoing
// song's bars: over the whole of the next song's intro between two of them, over a phrase of 8 for the others
const CLUB = new Set(["house", "techno and trance", "drum and bass", "edm", "bass music", "hard dance"]);
const CLUB_BARS = 32;
const SONG_BARS = 8;
// the families whose DJs stop a record to change songs, and how often a song that does not blend comes in so after one
const BRAKES = new Set(["hip hop", "r&b, soul and funk"]);
const BRAKE = 0.3;
/** the family of a genre, a fusion's first's */
const family = (genre) => (engine.genres.find((g) => g.name === genre.split("+")[0]) || {}).family || "";
// for its tests: the first song starts 20 s before it gives way, so that the next comes in as soon as it is made
const SOON = new URLSearchParams(location.search).has("soon");
// the families whose songs are usually in a minor key
const MINOR = new Set(["hip hop", "drum and bass", "bass music", "techno and trance", "hard dance", "electronic", "house", "edm"]);
const NOTES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
// the genres the station keeps to (none: every one it plays), from the address or the page's list
const keepTo = new Set(
  (new URLSearchParams(location.search).get("genres") || "")
    .split(",")
    .map((g) => g.trim())
    .filter(Boolean),
);

const $ = (id) => document.getElementById(id);
const pick = (xs) => xs[Math.floor(Math.random() * xs.length)];
// a genre as people say it, a fusion as its genres: "trip hop + idm"
const said = (g) => g.split("+").map((x) => (x === "dnb" ? "drum and bass" : x.replace(/_/g, " "))).join(" + ");
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
    // what the page asked the engine (`ask`) and is waiting to hear, by its id
    this.asked = new Map();
    this.asks = 0;
    // files asked for (`encode`), by their ids
    this.encoding = new Map();
    this.encodes = 0;
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

  /** `json` to the engine, with `audio` (samples) or `bytes` (a file) handed over rather than copied */
  post(json, audio, bytes) {
    if (!this.listening) {
      this.held.push([json, audio, bytes]);
      return;
    }
    if (audio) this.worker.postMessage({ json: JSON.stringify(json), audio }, [audio.buffer]);
    else if (bytes) this.worker.postMessage({ json: JSON.stringify(json), bytes }, [bytes.buffer]);
    else this.worker.postMessage({ json: JSON.stringify(json) });
  }

  /** The engine asked `kind` of `body` and its answer: `choose` (which of a search's finds are worth fetching), a
   *  `recording` to cut (`bytes`, the file) or `sampled_from` (the recordings the song plays). */
  ask(kind, body, bytes) {
    return new Promise((resolve) => {
      const id = ++this.asks;
      this.asked.set(id, resolve);
      if (kind === "recording") this.post({ recording: body, id }, null, bytes);
      else this.post({ [kind]: { ...body, id } });
    });
  }

  heard(data) {
    // a message sent before the worker first speaks would be lost: what was held goes now
    if (!this.listening) {
      this.listening = true;
      for (const [j, audio, bytes] of this.held.splice(0)) this.post(j, audio, bytes);
    }
    let v;
    try {
      v = JSON.parse(data.json);
    } catch {
      return;
    }
    // an answer to `ask`: a recording's cut comes with the state, which says nothing new of a song being made
    for (const kind of ["chosen", "sampled_from", "sampled"]) {
      const answer = v[kind];
      if (answer && this.asked.has(answer.id)) {
        const done = this.asked.get(answer.id);
        this.asked.delete(answer.id);
        done(answer);
        return;
      }
    }
    if (v.encoded) {
      const asked = this.encoding.get(v.encoded.id);
      this.encoding.delete(v.encoded.id);
      if (!asked) return;
      if (v.encoded.error || !data.bytes) asked.reject(new Error(v.encoded.error || "the engine sent no file"));
      else asked.resolve({ bytes: data.bytes, extension: v.encoded.extension, mime: v.encoded.mime });
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
    const rendered = r && r.version === v.state.song_version && r.url !== "preview:";
    // a render that failed comes without audio and with nothing more to come until the song changes
    if (!data.audio && v.state.song && !v.state.busy && !rendered) {
      this.waiting = null;
      w.reject(new Error((v.state.log || []).at(-1) || "the song could not be rendered"));
      return;
    }
    if (!data.audio || !r || !v.state.song || r.version !== v.state.song_version) return;
    if (r.url === "preview:") {
      if (w.preview) w.preview(v.state, data.audio);
      return;
    }
    this.waiting = null;
    w.resolve({ state: v.state, audio: data.audio });
  }

  /** `audio` (interleaved stereo at 48 kHz) as a file in `format` (`mp3`), tagged with `title`: its bytes, the
   *  extension of its name and its media type */
  encode(format, title, audio) {
    return new Promise((resolve, reject) => {
      const id = ++this.encodes;
      this.encoding.set(id, { resolve, reject });
      this.post({ encode: { id, spec: format, title } }, audio);
    });
  }

  /** the song `spec` asks for, written and rendered whole; `preview` hears its first bars before */
  song(spec, preview) {
    return new Promise((resolve, reject) => {
      this.waiting = { resolve, reject, preview };
      // built on samples as its genre is: on the recordings `gather` had cut, else on what stands in for them
      const command = { cmd: "new_song", genre: spec.genre, seed: spec.seed, bpm: spec.bpm };
      if (spec.key) command.key = keyName(spec.key);
      // a song of a genre built on samples the station writes on the genre's own sounds alone
      if (spec.sampled === false) command.sampled = false;
      this.post({ what: "new_song", command });
    });
  }
}

// ---- the samples a song built on them plays ----

// how long the libraries are searched for a song's samples: the first song is waited for, the others are written while
// one plays
const GATHER_FIRST = 20_000;
const GATHER = 45_000;

/** numbers in [0, 1) that `seed` fixes (mulberry32): each song tries its genre's searches in an order of its own */
function seeded(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// every recording the station has had cut, by its id: the role it was cut for and the search's record of it (its licence
// in full, what the studio is handed to cut it again). The engine keeps what it has cut, so a later song can play one
// gathered for an earlier one, and is credited for it as for its own.
const gathered = new Map();

/** A few recordings for the song `spec` asks for, when its genre `g` is built on samples (`g.sampling`, as the engine
 *  says it), for as long as `ms`: for each role, two of its searches in an order the song's seed picks, the engine
 *  choosing of each search's finds those worth fetching (`choose`: a length the role takes, not cut from someone's
 *  record), and two of those fetched and cut, until one is. Those it cut, each with its role and the search's record of
 *  it (what the studio is handed to cut again). A role nothing is found for goes without; with no search at all, the
 *  song plays what stands in for samples in its genre. */
async function gather(g, spec, ms) {
  const sampling = g && g.sampling;
  if (!sampling || !sampling.by_default || spec.sampled === false) return [];
  const until = performance.now() + ms;
  const r = seeded(spec.seed);
  const kept = [];
  for (const role of sampling.roles) {
    const terms = [...role.search];
    for (let i = terms.length - 1; i > 0; i--) {
      const j = Math.floor(r() * (i + 1));
      [terms[i], terms[j]] = [terms[j], terms[i]];
    }
    let cut = false;
    for (const q of terms.slice(0, 2)) {
      if (cut || performance.now() > until) break;
      let results;
      try {
        const res = await fetch(`/studio/samples/search?q=${encodeURIComponent(q)}`);
        if (!res.ok) continue;
        results = (await res.json()).results || [];
      } catch {
        continue;
      }
      const chosen = (await engine.ask("choose", { genre: g.name, role: role.role, results })).recordings || [];
      for (const rec of chosen.slice(0, 2)) {
        if (performance.now() > until) break;
        try {
          const res = await fetch(`/studio/samples/${rec.found.audio}`);
          if (!res.ok) continue;
          const answer = await engine.ask("recording", rec, new Uint8Array(await res.arrayBuffer()));
          if (answer.ok) {
            const sample = { role: role.role, found: results.find((x) => x.id === rec.found.id) || rec.found };
            kept.push(sample);
            gathered.set(rec.found.id, sample);
            cut = true;
            break;
          }
        } catch {
          // a recording that does not come is passed over for the next
        }
      }
    }
  }
  return kept;
}

/** The song `spec` asks for, written, with the recordings it plays: gathered first when its genre samples (`ms` at
 *  most), then written and rendered (`preview` hears its first bars before); `{ state, audio, samples }`, `samples`
 *  being every recording the song plays (`sampled_from`), this round's or an earlier one's, each with its role and its
 *  record. */
async function written(spec, ms, preview) {
  // a fusion samples as its first genre does
  const g = engine.genres.find((x) => x.name === spec.genre.split("+")[0]);
  await gather(g, spec, ms);
  const song = await engine.song(spec, preview);
  // what the song plays, as the engine says it: what was gathered for it, and anything gathered for an earlier song that
  // it plays too, each credited with the role it was cut for and the search's record of it
  const playing = (await engine.ask("sampled_from", {})).found || [];
  const samples = playing.filter((f) => gathered.has(f.id)).map((f) => gathered.get(f.id));
  return { ...song, samples };
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

/** a song on a deck of its own: its sound (`audio`, or a buffer of it) through a low shelf (its bass), a high-pass and a
 *  fader, and a send to the echo; with the song the station asked the engine for (`spec`) */
function deck(state, audio, spec) {
  const song = state.song;
  const src = ctx.createBufferSource();
  src.buffer = audio instanceof AudioBuffer ? audio : buffer(audio);
  const low = ctx.createBiquadFilter();
  low.type = "lowshelf";
  low.frequency.value = 200;
  // a DJ mixer's mid knob: a broad bell round 1 kHz, at 0 dB until a blend turns it
  const mid = ctx.createBiquadFilter();
  mid.type = "peaking";
  mid.frequency.value = 1000;
  mid.Q.value = 0.7;
  mid.gain.value = 0;
  const thin = ctx.createBiquadFilter();
  thin.type = "highpass";
  thin.frequency.value = 10;
  const fader = ctx.createGain();
  const send = ctx.createGain();
  send.gain.value = 0;
  src.connect(low);
  low.connect(mid);
  mid.connect(thin);
  thin.connect(fader);
  fader.connect(master);
  fader.connect(send);
  send.connect(echo.into);
  return {
    src,
    low,
    mid,
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
    state,
    spec,
  };
}

/** song `d` on a fresh deck (a skip's), with what it samples, so that its credits and its studio link stay */
const again = (d) => {
  const b = deck(d.state, d.src.buffer, d.spec);
  b.samples = d.samples;
  return b;
};

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

/** the layers a song's beat is (`radio::BEAT` in the app): a section plays it when one of them plays there (a hat or
 *  a shaker alone keeps time over an intro, and is not the beat a DJ hands over) */
const BEAT = new Set(["kick", "snare", "clap", "break"]);
/** how many of the outgoing song's bars the next one's intro runs under, at most and at least, when the beat is handed
 *  over on a downbeat (`handOver`) */
const HAND_OVER_MOST = 8;
const HAND_OVER_LEAST = 2;
const beatIn = (s) => (s.layers || []).some((l) => BEAT.has(l));

/** the bar where a song's beat stops for good: the end of its last section that plays it (its end when none does) */
function beatEnds(d) {
  const on = d.sections.filter(beatIn);
  if (!on.length) return d.bars;
  const last = on[on.length - 1];
  return last.start_bar + last.bars;
}

/** how many bars a song plays before its beat first comes in: its intro without drums */
function beatStarts(d) {
  const first = d.sections.find(beatIn);
  return first ? first.start_bar : 0;
}

/** where a song starts to give way to the next: its outro when it has one of 8 bars or more, else its last 16 bars, or
 *  8 bars before its beat stops for good where that is sooner (the most the next song's intro runs under it), never
 *  before its middle; and how many bars of it are left from there */
function leaving(d) {
  const last = d.sections[d.sections.length - 1];
  let from = last && last.kind === "outro" && last.bars >= 8 ? last.start_bar : d.bars - 16;
  const end = beatEnds(d);
  if (end < d.bars) from = Math.min(from, end - HAND_OVER_MOST);
  from = Math.max(from, Math.ceil(d.bars / 2));
  return { at: from * bar(d), bars: d.bars - from };
}

/** When the playing song's beat stops before its end (an outro without drums) or the next one's starts after its
 *  beginning (an intro without them), a blend of the one over the other would leave the music without a beat while it
 *  lasts: instead the next comes in under the playing one's last bars with a beat, so that its beat lands on the
 *  downbeat where the other's ends, as a DJ lets the next record's intro run under the outgoing song's last bars and
 *  drops its beat on the one (the app's `hand_over`). Null when both beats run on through the change, or too late. */
function handOver(a, b) {
  const end = beatEnds(a);
  const begins = beatStarts(b);
  if (end >= a.bars && begins === 0) return null;
  // the next song's bars counted in the playing one's, and its intro so
  const ratio = bar(b) / bar(a);
  const intro = begins * ratio;
  const lead = Math.min(HAND_OVER_MOST, Math.max(HAND_OVER_LEAST, Math.round(intro)));
  const atBar = end - lead;
  const at = a.t0 + atBar * bar(a);
  if (atBar < Math.ceil(a.bars / 2) || at < ctx.currentTime + 1) return null;
  const span = lead * bar(a);
  const beat = bar(a) / 4;
  const last = at + span - beat;
  // its intro's start left out, so that its beat lands on the downbeat; its fader up over the first half, its bass
  // held back until the downbeat and its mids until just before it
  start(b, at, Math.max(intro - lead, 0) * bar(a));
  fade(b.fader.gain, 0, 1, at, span / 2);
  b.low.gain.setValueAtTime(BASS_CUT_DB, at);
  b.low.gain.setValueAtTime(BASS_CUT_DB, last);
  b.low.gain.linearRampToValueAtTime(0, at + span);
  b.mid.gain.setValueAtTime(MIDS_CUT_DB, at);
  b.mid.gain.setValueAtTime(MIDS_CUT_DB, at + span - 2 * beat);
  b.mid.gain.linearRampToValueAtTime(0, last);
  // the playing song whole until its last beat, thinned and faded over it; what it would have played after (its
  // outro) is left out
  fade(a.fader.gain, 1, 0, last, beat);
  a.thin.frequency.setValueAtTime(10, last);
  a.thin.frequency.exponentialRampToValueAtTime(300, at + span);
  halt(a, at + span + 0.05);
  return { at, label: `its intro under the last ${lead} bars, its beat in on the downbeat where the other's ended`, blend: true };
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
  if (how === "blend") {
    const handed = handOver(a, b);
    if (handed) return handed;
  }
  const out = leaving(a);
  const soonest = Math.max(a.t0 + out.at, ctx.currentTime + 1);
  // on the outgoing song's bar lines, so that its last beat and the next song's first fall together
  const at = a.t0 + Math.ceil((soonest - a.t0) / bar(a) - 1e-6) * bar(a);
  const left = a.bars - Math.round((at - a.t0) / bar(a));
  if (how === "blend" && left >= 4) {
    // the next song's own bars, counted in the outgoing one's (twice as many when it is at twice the tempo): its whole
    // beat-only intro between two club songs, a phrase of 8 between others
    const intro = b.sections.length ? b.sections[0].bars : 16;
    const ratio = bar(b) / bar(a);
    const most = CLUB.has(family(a.genre)) && CLUB.has(family(b.genre)) ? CLUB_BARS : SONG_BARS;
    let bars = Math.min(left, Math.floor(intro * ratio), most);
    bars -= bars % 2;
    bars = Math.max(bars, 4);
    const span = bars * bar(a);
    const half = (bars / 2) * bar(a);
    const beat = bar(a) / 4;
    // as on a three-band mixer: the next song's fader up over the first quarter, its bass and mids held back, so that its
    // highs come in first; the basses swapping on the half, its mids coming in round it and the outgoing one's leaving
    start(b, at);
    fade(b.fader.gain, 0, 1, at, span / 4);
    b.low.gain.setValueAtTime(BASS_CUT_DB, at);
    b.low.gain.setValueAtTime(BASS_CUT_DB, at + half - beat);
    b.low.gain.linearRampToValueAtTime(0, at + half);
    b.mid.gain.setValueAtTime(MIDS_CUT_DB, at);
    b.mid.gain.setValueAtTime(MIDS_CUT_DB, at + half - beat);
    b.mid.gain.linearRampToValueAtTime(0, at + half + beat);
    a.low.gain.setValueAtTime(0, at + half - beat);
    a.low.gain.linearRampToValueAtTime(BASS_CUT_DB, at + half);
    a.mid.gain.setValueAtTime(0, at + half);
    a.mid.gain.linearRampToValueAtTime(MIDS_CUT_DB, at + half + 2 * beat);
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

/** The playing song `a` let go of from its next beat: `b`, when it was to blend in (`how`: at `a`'s tempo, in a key of
 *  its notes), mixed in over two bars as a DJ cuts a song short, its bass held back until the second bar, where the
 *  basses swap, `a` thinned and faded out; else `a`'s last beat into the echo and cut, as an echo out does, `b` from its
 *  start. What was scheduled for `a` is taken back first. */
function cut(a, b, how) {
  const now = ctx.currentTime;
  for (const p of [a.fader.gain, a.low.gain, a.mid.gain, a.thin.frequency, a.send.gain, a.src.playbackRate]) {
    p.cancelScheduledValues(now);
    p.setValueAtTime(p.value, now);
  }
  const beat = bar(a) / 4;
  const at = a.t0 + Math.ceil((now + 0.15 - a.t0) / beat) * beat;
  if (how === "blend") {
    const span = 2 * bar(a);
    // from two bars before the next song's beat comes in (the end of a quiet intro, or its start), so that the skip
    // lands on its beat
    start(b, at, Math.max(beatStarts(b) * bar(b) - span, 0));
    fade(b.fader.gain, 0, 1, at, beat);
    b.low.gain.setValueAtTime(BASS_CUT_DB, at);
    b.low.gain.setValueAtTime(BASS_CUT_DB, at + bar(a) - beat);
    b.low.gain.linearRampToValueAtTime(0, at + bar(a));
    a.low.gain.setValueAtTime(0, at + bar(a) - beat);
    a.low.gain.linearRampToValueAtTime(BASS_CUT_DB, at + bar(a));
    fade(a.fader.gain, 1, 0, at, span);
    a.thin.frequency.setValueAtTime(10, at);
    a.thin.frequency.exponentialRampToValueAtTime(800, at + span);
    halt(a, at + span + 0.05);
    return { at, label: "skipped to, mixed in over two bars", blend: true };
  }
  echo.delay.delayTime.setValueAtTime(0.75 * beat, now);
  a.send.gain.linearRampToValueAtTime(1, at);
  a.fader.gain.setValueAtTime(a.fader.gain.value, at);
  a.fader.gain.linearRampToValueAtTime(0, at + 0.05);
  halt(a, at + 0.1);
  start(b, at);
  return { at, label: "skipped to, in on the beat" };
}

// ---- the station ----

let onAir = null;
let next = null;
let planned = null;
const recent = [];
// a skip asked for while the next song is still being written: it comes in as soon as it is
let skipping = false;

/** the genres the station plays: those chosen, or every one */
function station() {
  const chosen = engine.genres.filter((g) => keepTo.has(g.name));
  return chosen.length ? chosen : engine.genres;
}

/** what to play after `now`: most of the time a genre that mixes into it beat for beat, at its tempo (or twice or half
 *  of it) and in a key of the same notes; else another tempo and key, which comes in after an echo or a stop */
function plan(now) {
  const all = station();
  // a fused song (`house+techno`) was each of its genres: none of them follows it while another can, and where none other
  // can (a station of those two), the one that led it does not
  const played = now.genre.split("+");
  const other = all.filter((g) => !played.includes(g.name));
  // a station of a few genres plays them again sooner than every fifth song, and one of a single genre plays only it
  const fresh = other.filter((g) => !recent.includes(g.name));
  const notLead = all.filter((g) => g.name !== played[0]);
  const pool = fresh.length ? fresh : other.length ? other : notLead.length ? notLead : all;
  const seed = 1 + Math.floor(Math.random() * 999_999);
  const fits = (g, bpm) => bpm >= g.bpm[0] * 0.98 && bpm <= g.bpm[1] * 1.02;
  const mixable = [];
  for (const g of pool) {
    for (const f of [1, 2, 0.5]) if (fits(g, now.bpm * f)) mixable.push({ g, bpm: Math.round(now.bpm * f * 10) / 10 });
  }
  let spec;
  if (mixable.length && Math.random() < BLEND) {
    const m = pick(mixable);
    spec = { genre: m.g.name, bpm: m.bpm, key: nextKey(now.key, m.g.family), seed, how: "blend" };
  } else {
    const g = pick(pool);
    const bpm = Math.round(g.bpm[0] + Math.random() * (g.bpm[1] - g.bpm[0]));
    const key = { tonic: Math.floor(Math.random() * 12), minor: MINOR.has(g.family) };
    // a record stop after a song of a family whose DJs do it, an echo out after any other
    const brakes = BRAKES.has(family(now.genre));
    spec = { genre: g.name, bpm, key, seed, how: brakes && Math.random() < BRAKE ? "brake" : "echo" };
  }
  return touched(spec);
}

/** `spec` as the station plays it: a third of the songs of a genre built on samples written on its own sounds alone,
 *  and now and then, on a station kept to two genres or more, its genre fused with another of them */
function touched(spec) {
  const g = engine.genres.find((x) => x.name === spec.genre);
  if (g && g.sampling && g.sampling.by_default && Math.random() >= SAMPLED) spec.sampled = false;
  const kept = engine.genres.filter((x) => keepTo.has(x.name)).map((x) => x.name);
  const others = kept.filter((n) => n !== spec.genre);
  if (kept.length >= 2 && others.length && Math.random() < FUSED) spec.genre = `${spec.genre}+${pick(others)}`;
  return spec;
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
  // a station that failed to start, and was let go of, has nothing to follow
  if (!onAir) return;
  const spec = plan(onAir);
  planned = spec;
  remember(spec.genre.split("+")[0]);
  try {
    const song = await written(spec, GATHER);
    const b = deck(song.state, song.audio, spec);
    b.samples = song.samples;
    const m = skipping ? cut(onAir, b, spec.how) : mix(onAir, b, spec.how);
    skipping = false;
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

/** the next song on air once its time `t` has come, as it has from the moment its first bars sound: the clock looks
 * every quarter second (less often in a tab out of sight), and a skip looks first */
function promote(t) {
  if (next && t >= next.at) {
    addPlayed(onAir);
    onAir = next.deck;
    status(next.label);
    next = null;
    queue();
  }
}

function tick() {
  if (!ctx) return;
  const t = ctx.currentTime;
  promote(t);
  if (!onAir) return;
  const at = t - onAir.t0;
  const link = studio(onAir);
  if ($("open").getAttribute("href") !== link) $("open").setAttribute("href", link);
  show("now-title", onAir.title);
  show("now-meta", `${said(onAir.genre)} · ${Math.round(onAir.bpm * 10) / 10} BPM · ${onAir.keyText}`);
  $("now-bar").style.width = `${Math.min(100, (100 * at) / onAir.seconds)}%`;
  show("now-time", `${clock(at)} / ${clock(onAir.seconds)}`);
  const sampled = credits(onAir);
  show("now-samples", sampled ? `Samples ${sampled}` : "");
  if (next) {
    const d = next.deck;
    const how = next.blend ? "mixes in" : "comes in";
    show("next", `${d.title}: ${said(d.genre)}, ${Math.round(d.bpm)} BPM, ${d.keyText}; ${how} ${clock(next.at - t)} from now`);
  } else if (planned) {
    const when = skipping ? ", to come in as soon as it is written" : "";
    show("next", `writing a ${said(planned.genre)} song at ${planned.bpm} BPM in ${keyName(planned.key)}${when}…`);
  }
}

/** where the studio demo writes the song on deck `d` again, as the station asked the engine for it: on the recordings it
 *  sampled, which the studio fetches and cuts again, else built on none */
function studio(d) {
  const samples = d.samples || [];
  const sampled = samples.length ? "1" : "0";
  const q = new URLSearchParams({ genre: d.spec.genre, seed: String(d.spec.seed), bpm: String(d.spec.bpm), sampled });
  if (d.spec.key) q.set("key", keyName(d.spec.key));
  if (samples.length) q.set("recordings", JSON.stringify(samples));
  return `/studio/?mock&${q}`;
}

/** what the song on deck `d` samples, as its credits say it: "title" by creator, licence */
function credits(d) {
  const licence = (f) => (f.license === "cc0" ? "CC0 1.0" : `CC BY ${f.license_version || ""}`.trim());
  return (d.samples || []).map(({ found: f }) => `"${f.title}" by ${f.creator}, ${licence(f)}`).join("; ");
}

/** the next song in now, on the playing one's next beat; or, while it is still being written, as soon as it is */
function skip() {
  if (!onAir || !ctx) return;
  // a song already mixing in is on air: the skip is to the one after it, not back to its own top
  promote(ctx.currentTime);
  if (!next) {
    skipping = true;
    status("Skipping: the next song comes in as soon as it is written");
    tick();
    return;
  }
  // its source was scheduled to start later: it never starts, and a fresh deck plays the song
  halt(next.deck, ctx.currentTime);
  const b = again(next.deck);
  // a song that was to blend in mixes in over two bars; one that was not echoes in on the beat
  const m = cut(onAir, b, next.deck.spec.how);
  next = { deck: b, at: m.at, label: `${said(b.genre)}, ${m.label}`, blend: !!m.blend };
}

/** the song on air as a file to keep: its audio, as it plays, encoded in the engine's worker */
async function download() {
  const d = onAir;
  if (!d || !engine) return;
  const button = $("download");
  button.disabled = true;
  status(`Making a file of ${d.title}…`);
  const b = d.src.buffer;
  const [l, r] = [b.getChannelData(0), b.getChannelData(1)];
  const audio = new Float32Array(2 * b.length);
  for (let i = 0; i < b.length; i++) {
    audio[2 * i] = l[i];
    audio[2 * i + 1] = r[i];
  }
  try {
    const file = await engine.encode("mp3", d.title, audio);
    const url = URL.createObjectURL(new Blob([file.bytes], { type: file.mime }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `${d.title.replace(/[^\w\s-]/g, "").trim().replace(/\s+/g, " ") || "Infinite Radio"}.${file.extension}`;
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
    status(`${d.title} is in your downloads`);
  } catch (e) {
    status(`${d.title} could not be made a file: ${e.message}`);
  } finally {
    button.disabled = false;
  }
}

// ---- the genres ----

/** the list of genres to keep to, by family, once the engine says what it plays; a choice holds from the next song
 *  written, and the address keeps it, to share the station */
function genreList() {
  const list = $("genre-list");
  if (list.childElementCount || !engine) return;
  const families = new Map();
  for (const g of engine.genres) {
    const f = g.family || "other";
    if (!families.has(f)) families.set(f, []);
    families.get(f).push(g);
  }
  for (const [family, genres] of [...families].sort(([a], [b]) => a.localeCompare(b))) {
    const set = document.createElement("fieldset");
    const legend = document.createElement("legend");
    legend.textContent = family;
    set.append(legend);
    for (const g of genres) {
      const label = document.createElement("label");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.value = g.name;
      box.checked = keepTo.has(g.name);
      box.addEventListener("change", () => choose(g.name, box.checked));
      label.append(box, ` ${said(g.name)}`);
      set.append(label);
    }
    list.append(set);
  }
  sayGenres();
}

function choose(genre, kept) {
  if (kept) keepTo.add(genre);
  else keepTo.delete(genre);
  const q = new URLSearchParams(location.search);
  if (keepTo.size) q.set("genres", [...keepTo].join(","));
  else q.delete("genres");
  const query = q.toString().replace(/%2C/g, ",");
  history.replaceState(null, "", `${location.pathname}${query ? `?${query}` : ""}`);
  sayGenres();
}

function sayGenres() {
  const n = engine ? station().length : keepTo.size;
  const words = !keepTo.size ? "every one" : keepTo.size <= 3 ? [...keepTo].map(said).join(", ") : `${n} chosen`;
  show("genres-said", words);
}

/** the engine started for the list of genres before the station is tuned in */
async function genresWanted() {
  if (!$("genres").open) return;
  if (!engine) {
    engine = new Engine();
    show("genres-said", "loading the list…");
  }
  const starting = engine;
  try {
    await starting.ready;
    genreList();
  } catch (e) {
    // an engine that did not start is let go of, so that Tune in, or the list opened again, starts another
    if (engine === starting) {
      starting.worker.terminate();
      engine = null;
    }
    sayGenres();
    status(`The list of genres could not be had: ${e.message}`);
  }
}

async function tuneIn() {
  const button = $("tune");
  if (ctx) {
    // a station on air pauses and plays; one the browser will not pause or play again stays as it was
    const playing = ctx.state === "running";
    try {
      await (playing ? ctx.suspend() : ctx.resume());
      button.textContent = playing ? "Play" : "Pause";
      button.dataset.playing = String(!playing);
    } catch (e) {
      status(`The radio could not ${playing ? "pause" : "play"}: ${e.message}`);
    }
    return;
  }
  button.disabled = true;
  wire();
  engine = engine || new Engine();
  status("Starting the engine…");
  await engine.ready;
  genreList();
  const openers = station().filter((x) => OPENERS.includes(x.name));
  const g = pick(openers.length ? openers : station());
  const spec = { genre: g.name, bpm: Math.round((g.bpm[0] + g.bpm[1]) / 2), seed: 1 + Math.floor(Math.random() * 999_999) };
  remember(spec.genre);
  const samples = g.sampling && g.sampling.by_default;
  status(`${samples ? "Finding recordings for" : "Writing"} a ${said(spec.genre)} song at ${spec.bpm} BPM…`);
  let early = null;
  const song = await written(spec, GATHER_FIRST, (state, audio) => {
    // the first bars, to play while the rest renders
    if (early || SOON) return;
    early = deck(state, audio, spec);
    start(early, ctx.currentTime + 0.1);
    onAir = early;
    tick();
    status("On air while the rest of it renders");
  });
  const whole = deck(song.state, song.audio, spec);
  whole.samples = song.samples;
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
  // the deck shows the song as it goes on air, not at the clock's next tick
  tick();
  status("On air");
  button.disabled = false;
  button.textContent = "Pause";
  button.dataset.playing = "true";
  for (const id of ["skip", "download"]) $(id).disabled = false;
  $("open").removeAttribute("aria-disabled");
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
    $("tune").dataset.playing = "false";
  });
});
$("skip").addEventListener("click", skip);
$("download").addEventListener("click", () => download());
$("genres").addEventListener("toggle", () => genresWanted());
sayGenres();
$("volume").addEventListener("input", (e) => {
  if (master) master.gain.setTargetAtTime(Number(e.target.value), ctx.currentTime, 0.05);
});
setInterval(tick, 250);
