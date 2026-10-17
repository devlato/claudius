// claudius.pages.dev: demo players, the hero's step grid, install tabs, copy buttons and code colours.
// No dependencies; everything works without this file except the custom players (the page falls back to <audio>).
(function () {
  "use strict";

  const doc = document;
  const root = doc.documentElement;
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const wide = window.matchMedia("(min-width: 1040px)");
  const css = (name) => getComputedStyle(root).getPropertyValue(name).trim();

  // ---------------------------------------------------------------------------------------------------------------
  // formatting, as on the studio's LCD

  function clock(t) {
    t = Math.max(0, t || 0);
    const m = Math.floor(t / 60);
    const s = t - m * 60;
    return m + ":" + (s < 10 ? "0" : "") + s.toFixed(1);
  }

  function shortClock(t) {
    t = Math.max(0, Math.round(t || 0));
    return Math.floor(t / 60) + ":" + String(t % 60).padStart(2, "0");
  }

  // ---------------------------------------------------------------------------------------------------------------
  // players

  const players = new Map();
  let frame = 0;

  function player(slug) {
    return players.get(slug);
  }

  function setupPlayers() {
    doc.querySelectorAll("[data-track]").forEach((el) => {
      const slug = el.dataset.track;
      const audio = el.querySelector("[data-audio]");
      const wave = el.querySelector("[data-wave]");
      if (!audio || !wave) return;
      audio.removeAttribute("controls");
      const p = {
        slug,
        title: el.dataset.title || slug,
        el,
        audio,
        wave,
        canvas: wave.querySelector("canvas"),
        lcds: Array.from(doc.querySelectorAll('[data-lcd="' + slug + '"]')),
        buttons: Array.from(doc.querySelectorAll('[data-play="' + slug + '"]')),
        data: null,
        pending: null,
        hover: null,
        lastSecond: -1,
        status: "STOP",
        lcdText: "",
      };
      players.set(slug, p);

      p.buttons.forEach((b) => b.addEventListener("click", () => toggle(p)));
      audio.addEventListener("play", () => onState(p));
      audio.addEventListener("playing", () => onState(p));
      audio.addEventListener("pause", () => onState(p));
      audio.addEventListener("ended", () => onState(p));
      audio.addEventListener("waiting", () => onState(p, "LOADING"));
      audio.addEventListener("loadedmetadata", () => {
        if (p.pending != null) {
          audio.currentTime = p.pending;
          p.pending = null;
        }
      });
      audio.addEventListener("error", () => onState(p, "ERROR"));
      audio.addEventListener("seeked", () => paint(p));

      setupSeeking(p);
      fetch(wave.dataset.peaks)
        .then((r) => (r.ok ? r.json() : Promise.reject(new Error(r.status + " " + r.url))))
        .then((data) => {
          p.data = data;
          paint(p);
        })
        .catch(() => {
          wave.classList.add("is-empty");
        });
    });

    if ("ResizeObserver" in window) {
      const ro = new ResizeObserver((entries) => {
        entries.forEach((e) => {
          const p = Array.from(players.values()).find((x) => x.wave === e.target);
          if (p) drawWave(p);
        });
      });
      players.forEach((p) => ro.observe(p.wave));
    } else {
      window.addEventListener("resize", () => players.forEach(drawWave));
    }
    if (doc.fonts && doc.fonts.ready) doc.fonts.ready.then(() => players.forEach(drawWave));
  }

  function position(p) {
    if (p.pending != null) return p.pending;
    return p.audio.currentTime || 0;
  }

  function duration(p) {
    if (p.data) return p.data.duration;
    return isFinite(p.audio.duration) ? p.audio.duration : 0;
  }

  function playing(p) {
    return !p.audio.paused && !p.audio.ended;
  }

  function toggle(p) {
    if (playing(p)) {
      p.audio.pause();
      return;
    }
    players.forEach((other) => {
      if (other !== p && playing(other)) other.audio.pause();
    });
    if (p.audio.ended) p.audio.currentTime = 0;
    onState(p, "LOADING");
    const started = p.audio.play();
    if (started && started.catch) started.catch(() => onState(p, playing(p) ? null : "ERROR"));
    select(p.slug, false);
  }

  function onState(p, forced) {
    const on = playing(p);
    let status = forced || (on ? "PLAY" : "STOP");
    if (!forced && on && p.audio.readyState < 3) status = "LOADING";
    p.status = status;
    p.buttons.forEach((b) => {
      b.setAttribute("aria-pressed", on ? "true" : "false");
      b.setAttribute("aria-label", (on ? "Pause " : "Play ") + p.title);
    });
    p.el.classList.toggle("is-playing", on);
    if (on && "mediaSession" in navigator && window.MediaMetadata) {
      navigator.mediaSession.metadata = new MediaMetadata({
        title: p.title,
        artist: "claudius",
        album: "Demos",
        artwork: [{ src: "/icon-512.png", sizes: "512x512", type: "image/png" }],
      });
      navigator.mediaSession.setActionHandler("play", () => p.audio.play());
      navigator.mediaSession.setActionHandler("pause", () => p.audio.pause());
      try {
        navigator.mediaSession.setActionHandler("seekto", (d) => seek(p, d.seekTime));
      } catch (e) {
        /* not supported */
      }
    }
    paint(p);
    if (on) loop();
  }

  function loop() {
    if (frame) return;
    const tick = () => {
      frame = 0;
      let any = false;
      players.forEach((p) => {
        if (playing(p)) {
          any = true;
          paint(p);
        }
      });
      if (any) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
  }

  function seek(p, t) {
    const d = duration(p);
    t = Math.min(Math.max(0, t), Math.max(0, d - 0.05));
    if (p.audio.readyState >= 1) {
      p.audio.currentTime = t;
    } else {
      p.pending = t;
      if (p.audio.preload === "none") p.audio.preload = "metadata";
      p.audio.load();
    }
    paint(p);
  }

  function setupSeeking(p) {
    const wave = p.wave;
    const at = (e) => {
      const r = wave.getBoundingClientRect();
      return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)) * duration(p);
    };
    let dragging = false;
    wave.addEventListener("pointerdown", (e) => {
      if (!duration(p) || e.button !== 0) return;
      dragging = true;
      wave.setPointerCapture(e.pointerId);
      seek(p, at(e));
    });
    wave.addEventListener("pointermove", (e) => {
      if (dragging) seek(p, at(e));
      p.hover = e.pointerType === "mouse" ? at(e) : null;
      drawWave(p);
    });
    const end = () => {
      dragging = false;
    };
    wave.addEventListener("pointerup", end);
    wave.addEventListener("pointercancel", end);
    wave.addEventListener("pointerleave", () => {
      p.hover = null;
      drawWave(p);
    });
    wave.addEventListener("keydown", (e) => {
      const d = duration(p);
      const t = position(p);
      let next = null;
      switch (e.key) {
        case "ArrowRight":
        case "ArrowUp":
          next = t + (e.shiftKey ? 1 : 5);
          break;
        case "ArrowLeft":
        case "ArrowDown":
          next = t - (e.shiftKey ? 1 : 5);
          break;
        case "PageUp":
          next = t + 10;
          break;
        case "PageDown":
          next = t - 10;
          break;
        case "Home":
          next = 0;
          break;
        case "End":
          next = d;
          break;
        case " ":
        case "Enter":
        case "k":
          e.preventDefault();
          toggle(p);
          return;
        default:
          return;
      }
      e.preventDefault();
      seek(p, next);
    });
  }

  // the song's position at time t of the excerpt
  function songPlace(p, t) {
    const d = p.data;
    if (!d) return null;
    const barSec = 240 / d.bpm;
    const songBar = d.startBar + t / barSec;
    const bar = Math.floor(songBar + 1e-6);
    const beat = Math.floor((songBar - bar) * 4 + 1e-6);
    let sec = d.sections[0];
    for (const s of d.sections) if (t >= s.start - 1e-6) sec = s;
    return { songBar, bar, beat, section: sec };
  }

  function paint(p) {
    const t = position(p);
    const d = duration(p);
    const place = songPlace(p, t);
    const status = p.status;
    const key =
      clock(t) + "|" + status + "|" + (place ? place.bar + "." + place.beat + place.section.name : "");
    if (key !== p.lcdText) {
      p.lcdText = key;
      const pos = place
        ? "BAR " + String(place.bar + 1).padStart(3, "0") + "." + (place.beat + 1) + "\u00a0\u00a0\u00a0" + place.section.name.toUpperCase()
        : "";
      p.lcds.forEach((lcd) => {
        setText(lcd.querySelector(".lcd-time"), clock(t));
        setText(lcd.querySelector(".lcd-dur"), "/ " + clock(d));
        const st = lcd.querySelector(".lcd-status");
        setText(st, status);
        st.classList.toggle("is-play", status === "PLAY");
        if (pos) setText(lcd.querySelector(".lcd-pos"), pos);
      });
    }
    const second = Math.floor(t);
    if (second !== p.lastSecond) {
      p.lastSecond = second;
      p.wave.setAttribute("aria-valuenow", String(second));
      p.wave.setAttribute("aria-valuemax", String(Math.round(d)));
      p.wave.setAttribute("aria-valuetext", shortClock(t) + " of " + shortClock(d));
    }
    drawWave(p);
    paintUnit(p, t, place);
  }

  function setText(el, text) {
    if (el && el.textContent !== text) el.textContent = text;
  }

  // ---------------------------------------------------------------------------------------------------------------
  // waveforms: precomputed peaks, coloured by section like the studio's master lane

  function sectionColor(kind) {
    return css("--k-" + kind) || css("--k-outro");
  }

  function drawWave(p) {
    const c = p.canvas;
    const d = p.data;
    if (!c || !d) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 3);
    const w = c.clientWidth;
    const h = c.clientHeight;
    if (!w || !h) return;
    const W = Math.round(w * dpr);
    const H = Math.round(h * dpr);
    if (c.width !== W || c.height !== H) {
      c.width = W;
      c.height = H;
    }
    const g = c.getContext("2d");
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);

    const top = 17;
    const mid = top + (h - top - 3) / 2;
    const amp = (h - top - 5) / 2;
    const step = 3;
    const bars = Math.max(1, Math.floor(w / step));
    const max = Math.max(1, ...d.peaks);
    const progress = Math.min(1, position(p) / d.duration);
    const n = d.peaks.length;

    // section labels along the top
    g.font = "10px " + css("--mono");
    g.textBaseline = "top";
    d.sections.forEach((s) => {
      const x0 = (s.start / d.duration) * w;
      const x1 = (s.end / d.duration) * w;
      const col = sectionColor(s.kind);
      g.fillStyle = col;
      g.fillRect(x0, 12, Math.max(1, x1 - x0 - 2), 2);
      if (x1 - x0 > 44) {
        g.fillText(s.name.toUpperCase(), x0 + 3, 1);
      }
    });

    for (let i = 0; i < bars; i++) {
      const a = Math.floor((i / bars) * n);
      const b = Math.max(a + 1, Math.floor(((i + 1) / bars) * n));
      let pk = 0;
      let rm = 0;
      for (let j = a; j < b && j < n; j++) {
        if (d.peaks[j] > pk) pk = d.peaks[j];
        if (d.rms[j] > rm) rm = d.rms[j];
      }
      const x = i * step;
      const t = ((i + 0.5) / bars) * d.duration;
      let kind = d.sections[0].kind;
      for (const s of d.sections) if (t >= s.start) kind = s.kind;
      const played = (i + 0.5) / bars <= progress;
      const col = sectionColor(kind);
      const ph = Math.max(1, (pk / max) * amp);
      const rh = Math.max(1, Math.min(ph, (rm / max) * amp * 1.4));
      g.globalAlpha = played ? 0.5 : 0.22;
      g.fillStyle = col;
      g.fillRect(x, mid - ph, step - 1, ph * 2);
      g.globalAlpha = played ? 1 : 0.5;
      g.fillRect(x, mid - rh, step - 1, rh * 2);
    }
    g.globalAlpha = 1;

    if (p.hover != null) {
      g.fillStyle = "rgba(255,236,214,0.35)";
      g.fillRect(Math.round((p.hover / d.duration) * w), top, 1, h - top);
    }
    if (progress > 0 || playing(p)) {
      g.fillStyle = css("--playhead");
      g.fillRect(Math.round(progress * w) - 1, top - 2, 2, h - top + 2);
    }
  }

  // ---------------------------------------------------------------------------------------------------------------
  // the hero unit: the step grid and the song's form follow the player

  const units = new Map();

  function setupUnits() {
    doc.querySelectorAll("[data-steps-for]").forEach((steps) => {
      const slug = steps.dataset.stepsFor;
      const lanes = Array.from(steps.querySelectorAll(".lane")).map((lane) => ({
        el: lane,
        bars: lane.dataset.pattern.split("|"),
        cells: Array.from(lane.querySelectorAll(".cell")),
        in: (lane.dataset.in || "").split(/\s+/).filter(Boolean),
        bar: 0,
      }));
      const form = doc.querySelector('[data-form-for="' + slug + '"]');
      units.set(slug, {
        lanes,
        now: -1,
        head: form && form.querySelector(".form-head"),
        win: form && form.querySelector(".form-window"),
        placed: false,
      });
    });
  }

  const LEVEL = { X: "v3", x: "v2", o: "v1" };

  function paintUnit(p, t, place) {
    const u = units.get(p.slug);
    if (!u || !place) return;
    const d = p.data;
    if (!u.placed && u.win) {
      u.win.style.left = (d.startBar / d.songBars) * 100 + "%";
      u.win.style.width = (d.duration / (240 / d.bpm) / d.songBars) * 100 + "%";
      u.placed = true;
    }
    const started = playing(p) || t > 0;
    if (u.head) {
      u.head.style.left = (place.songBar / d.songBars) * 100 + "%";
      u.head.classList.toggle("is-on", started);
    }
    if (!started) return;
    const frac = place.songBar - place.bar;
    let step = Math.floor(frac * 16 + 1e-6);
    if (reduceMotion.matches) step -= step % 4;
    const patternBar = ((place.bar % 4) + 4) % 4;
    const kind = place.section.kind;
    const name = place.section.name;
    u.lanes.forEach((lane) => {
      const bar = lane.bars[patternBar % lane.bars.length];
      if (lane.bar !== bar) {
        lane.bar = bar;
        lane.cells.forEach((cell, i) => {
          cell.classList.remove("v1", "v2", "v3");
          const lvl = LEVEL[bar[i]];
          if (lvl) cell.classList.add(lvl);
        });
      }
      const on = !lane.in.length || lane.in.includes(kind) || lane.in.includes(name);
      lane.el.classList.toggle("is-out", !on);
      if (u.now !== step) {
        if (u.now >= 0 && lane.cells[u.now]) lane.cells[u.now].classList.remove("is-now");
        if (playing(p) && lane.cells[step]) lane.cells[step].classList.add("is-now");
      }
    });
    if (!playing(p)) {
      u.lanes.forEach((lane) => lane.cells.forEach((c) => c.classList.remove("is-now")));
      u.now = -1;
    } else {
      u.now = step;
    }
  }

  // ---------------------------------------------------------------------------------------------------------------
  // "How it was made": one panel beside the tracks on wide screens, under each track on narrow ones

  let selected = null;
  const open = new Set();

  function setupMade() {
    const first = doc.querySelector(".made.is-active");
    selected = first ? first.id.replace(/^made-/, "") : null;
    doc.querySelectorAll("[data-made]").forEach((b) => {
      b.addEventListener("click", () => {
        const slug = b.dataset.made;
        if (wide.matches) {
          selected = slug;
        } else if (open.has(slug)) {
          open.delete(slug);
        } else {
          open.add(slug);
        }
        renderMade();
      });
    });
    doc.querySelectorAll("[data-open-made]").forEach((a) => {
      a.addEventListener("click", (e) => {
        const slug = a.dataset.openMade;
        const panel = doc.getElementById("made-" + slug);
        if (!panel) return;
        e.preventDefault();
        if (wide.matches) selected = slug;
        else open.add(slug);
        renderMade();
        const target = wide.matches ? doc.getElementById(slug) || panel : panel;
        target.scrollIntoView({ behavior: reduceMotion.matches ? "auto" : "smooth", block: "start" });
      });
    });
    const onChange = () => renderMade();
    if (wide.addEventListener) wide.addEventListener("change", onChange);
    else wide.addListener(onChange);
    renderMade();
  }

  function select(slug, scroll) {
    if (!wide.matches || !doc.getElementById("made-" + slug)) return;
    selected = slug;
    renderMade();
    if (scroll) doc.getElementById("made-" + slug).scrollIntoView({ block: "nearest" });
  }

  function renderMade() {
    doc.querySelectorAll(".made").forEach((panel) => {
      const slug = panel.id.replace(/^made-/, "");
      panel.classList.toggle("is-active", slug === selected);
      panel.classList.toggle("is-open", open.has(slug));
    });
    doc.querySelectorAll("[data-made]").forEach((b) => {
      const slug = b.dataset.made;
      const shown = wide.matches ? slug === selected : open.has(slug);
      b.setAttribute("aria-expanded", shown ? "true" : "false");
      const track = doc.getElementById(slug);
      if (track) track.classList.toggle("is-selected", wide.matches && slug === selected);
    });
  }

  // ---------------------------------------------------------------------------------------------------------------
  // install tabs (WAI-ARIA tabs with automatic activation)

  function setupTabs() {
    doc.querySelectorAll("[data-tabs]").forEach((box) => {
      const tabs = Array.from(box.querySelectorAll('[role="tab"]'));
      const activate = (tab, focus) => {
        tabs.forEach((t) => {
          const on = t === tab;
          t.setAttribute("aria-selected", on ? "true" : "false");
          t.tabIndex = on ? 0 : -1;
          const panel = doc.getElementById(t.getAttribute("aria-controls"));
          if (panel) panel.hidden = !on;
        });
        if (focus) tab.focus();
      };
      tabs.forEach((tab, i) => {
        tab.addEventListener("click", () => activate(tab, false));
        tab.addEventListener("keydown", (e) => {
          let j = null;
          if (e.key === "ArrowRight") j = (i + 1) % tabs.length;
          else if (e.key === "ArrowLeft") j = (i - 1 + tabs.length) % tabs.length;
          else if (e.key === "Home") j = 0;
          else if (e.key === "End") j = tabs.length - 1;
          if (j == null) return;
          e.preventDefault();
          activate(tabs[j], true);
        });
      });
      const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
      if (/win/i.test(platform)) {
        const win = tabs.find((t) => t.id === "tab-win");
        if (win) activate(win, false);
      }
    });
  }

  // ---------------------------------------------------------------------------------------------------------------
  // copy buttons

  function setupCopy() {
    doc.querySelectorAll("[data-copy]").forEach((b) => {
      b.addEventListener("click", async () => {
        const text = b.dataset.copy;
        let ok = false;
        try {
          await navigator.clipboard.writeText(text);
          ok = true;
        } catch (e) {
          const code = b.parentElement && b.parentElement.querySelector("code");
          if (code) {
            const range = doc.createRange();
            range.selectNodeContents(code);
            const sel = window.getSelection();
            sel.removeAllRanges();
            sel.addRange(range);
          }
        }
        b.textContent = ok ? "Copied" : "Press Ctrl+C";
        b.classList.toggle("is-done", ok);
        clearTimeout(b._reset);
        b._reset = setTimeout(() => {
          b.textContent = "Copy";
          b.classList.remove("is-done");
        }, 1800);
      });
    });
  }

  // ---------------------------------------------------------------------------------------------------------------
  // code colours: the tokenizer of the studio's script editor (crates/claudius-ui/src/panels/script.rs)

  const KEYWORDS = new Set(["let", "const", "fn", "if", "else", "for", "in", "while", "loop", "return", "true", "false", "switch", "break", "continue", "this"]);
  const PUNCT = "()[]{}<>,.;:=+*|!?&%^~";
  const digit = (c) => c >= "0" && c <= "9";
  const alpha = (c) => /[A-Za-z_$]/.test(c);
  const alnum = (c) => /[A-Za-z0-9_$]/.test(c);

  function rhaiTokens(src) {
    const out = [];
    let i = 0;
    const n = src.length;
    while (i < n) {
      const c = src[i];
      const start = i;
      let tok = null;
      if (c === "/" && src[i + 1] === "/") {
        while (i < n && src[i] !== "\n") i++;
        tok = "c";
      } else if (c === "/" && src[i + 1] === "*") {
        i += 2;
        while (i + 1 < n && !(src[i] === "*" && src[i + 1] === "/")) i++;
        i = Math.min(i + 2, n);
        tok = "c";
      } else if (c === '"' || c === "'" || c === "`") {
        i++;
        while (i < n && src[i] !== c && src[i] !== "\n") {
          if (src[i] === "\\") i++;
          i++;
        }
        i = Math.min(i + 1, n);
        tok = "s";
      } else if (digit(c) || (c === "." && digit(src[i + 1] || "")) || (c === "-" && digit(src[i + 1] || "") && (i === 0 || !/[A-Za-z0-9]/.test(src[i - 1])))) {
        i++;
        while (i < n && /[A-Za-z0-9._]/.test(src[i])) i++;
        tok = "n";
      } else if (alpha(c)) {
        while (i < n && alnum(src[i])) i++;
        const word = src.slice(start, i);
        let j = i;
        while (j < n && src[j] === " ") j++;
        const call = src[j] === "(";
        const afterDot = start > 0 && src[start - 1] === ".";
        tok = KEYWORDS.has(word) ? "k" : call && afterDot ? "m" : call ? "f" : null;
      } else if (PUNCT.includes(c)) {
        i++;
        tok = "p";
      } else {
        i++;
      }
      out.push([src.slice(start, i), tok]);
    }
    return out;
  }

  function shTokens(src) {
    const out = [];
    let i = 0;
    const n = src.length;
    let command = true;
    while (i < n) {
      const c = src[i];
      const start = i;
      let tok = null;
      const wordStart = i === 0 || /\s|\(/.test(src[i - 1]);
      if (c === "\n") {
        i++;
        command = true;
      } else if (c === "#" && wordStart) {
        while (i < n && src[i] !== "\n") i++;
        tok = "c";
      } else if (c === '"' || c === "'") {
        i++;
        while (i < n && src[i] !== c) i++;
        i = Math.min(i + 1, n);
        tok = "s";
      } else if (/\s/.test(c)) {
        while (i < n && /[ \t]/.test(src[i])) i++;
      } else if ("|&;()<>$=".includes(c)) {
        i++;
        tok = "p";
        if (c !== "$" && c !== "=") command = true;
      } else {
        while (i < n && !/[\s|&;()<>"'=]/.test(src[i])) i++;
        const word = src.slice(start, i);
        if (command) {
          tok = /^[A-Z_]+$/.test(word) && src[i] === "=" ? null : "f";
          command = /^[A-Z_]+$/.test(word) && src[i] === "=";
        } else if (word[0] === "-") {
          tok = "m";
        }
      }
      out.push([src.slice(start, i), tok]);
    }
    return out;
  }

  function jsonTokens(src) {
    const out = [];
    const re = /("(?:[^"\\]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?)|\b(true|false|null)\b|([{}[\],:])|(\s+|[^"\s{}[\],:]+)/g;
    let m;
    while ((m = re.exec(src))) {
      if (m[1]) {
        out.push([m[1], m[2] ? "m" : "s"]);
        if (m[2]) out.push([m[2], "p"]);
      } else if (m[3]) out.push([m[3], "n"]);
      else if (m[4]) out.push([m[4], "k"]);
      else if (m[5]) out.push([m[5], "p"]);
      else out.push([m[6], null]);
    }
    return out;
  }

  function highlight() {
    const kinds = { "language-rhai": rhaiTokens, "language-sh": shTokens, "language-json": jsonTokens };
    Object.keys(kinds).forEach((cls) => {
      doc.querySelectorAll("code." + cls).forEach((code) => {
        const toks = kinds[cls](code.textContent);
        const frag = doc.createDocumentFragment();
        toks.forEach(([text, tok]) => {
          if (!tok) {
            frag.appendChild(doc.createTextNode(text));
            return;
          }
          const span = doc.createElement("span");
          span.className = "t-" + tok;
          span.textContent = text;
          frag.appendChild(span);
        });
        code.textContent = "";
        code.appendChild(frag);
      });
    });
  }

  // ---------------------------------------------------------------------------------------------------------------
  // the section you are reading, marked in the nav

  function setupNav() {
    const links = Array.from(doc.querySelectorAll('.nav a[href^="#"]'));
    if (!links.length || !("IntersectionObserver" in window)) return;
    const byId = new Map(links.map((a) => [a.getAttribute("href").slice(1), a]));
    const io = new IntersectionObserver(
      (entries) => {
        entries.forEach((e) => {
          if (!e.isIntersecting) return;
          links.forEach((l) => l.removeAttribute("aria-current"));
          const a = byId.get(e.target.id);
          if (a) a.setAttribute("aria-current", "true");
        });
      },
      { rootMargin: "-45% 0px -50% 0px" }
    );
    // every section, so the hero clears the mark
    doc.querySelectorAll("main > section").forEach((s) => io.observe(s));
  }

  // code blocks and panels that scroll take keyboard focus, so they can be scrolled without a mouse
  function setupScrollers() {
    const boxes = Array.from(doc.querySelectorAll("pre, .made, .table-wrap"));
    const check = () => {
      boxes.forEach((el) => {
        const scrolls = el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1;
        if (scrolls && !el.hasAttribute("tabindex")) {
          el.tabIndex = 0;
          el.dataset.scrollFocus = "";
        } else if (!scrolls && el.dataset.scrollFocus !== undefined) {
          el.removeAttribute("tabindex");
          delete el.dataset.scrollFocus;
        }
      });
    };
    check();
    let pending = 0;
    const later = () => {
      cancelAnimationFrame(pending);
      pending = requestAnimationFrame(check);
    };
    window.addEventListener("resize", later);
    doc.addEventListener("click", (e) => {
      if (e.target.closest && e.target.closest("[data-made], [data-open-made]")) later();
    });
  }

  function init() {
    highlight();
    setupUnits();
    setupPlayers();
    setupMade();
    setupTabs();
    setupCopy();
    setupNav();
    setupScrollers();
  }

  if (doc.readyState === "loading") doc.addEventListener("DOMContentLoaded", init);
  else init();
})();
