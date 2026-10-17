#!/usr/bin/env python3
"""Cut a site demo from a claudius song folder (needs ffmpeg and numpy).

    excerpt.py analyze SONG_DIR
    excerpt.py cut SONG_DIR SLUG START_BAR BARS OUT_DIR [--title T] [--key K]

`analyze` prints the loudness of every bar with the section it belongs to, to find the drop or the groove.
`cut` writes OUT_DIR/SLUG.mp3 (160 kbps, a short fade in and a 3 s fade out) and OUT_DIR/SLUG.json: 600 peak and
RMS values (0..255) read back from the MP3, the sections inside the excerpt in seconds, and where the excerpt sits in
the song (START_BAR counts from 0), which the players use for their bar readout.
"""
import json
import subprocess
import sys

import numpy as np

SR = 44100
POINTS = 600


def load(song_dir, src="song.wav", start=None, dur=None):
    cmd = ["ffmpeg", "-v", "error"]
    if start is not None:
        cmd += ["-ss", f"{start:.6f}", "-t", f"{dur:.6f}"]
    cmd += ["-i", f"{song_dir}/{src}", "-f", "f32le", "-ac", "2", "-ar", str(SR), "-"]
    raw = subprocess.run(cmd, check=True, capture_output=True).stdout
    return np.frombuffer(raw, dtype=np.float32).reshape(-1, 2)


def project(song_dir):
    return json.load(open(f"{song_dir}/project.json"))


def section_at(p, bar):
    for s in p["sections"]:
        if s["start_bar"] <= bar < s["start_bar"] + s["bars"]:
            return s
    return None


def analyze(song_dir):
    p = project(song_dir)
    bar_s = 240.0 / p["bpm"]
    x = load(song_dir)
    n = int(len(x) / (bar_s * SR))
    for b in range(n):
        seg = x[int(b * bar_s * SR) : int((b + 1) * bar_s * SR)]
        rms = float(np.sqrt(np.mean(seg**2)) + 1e-9)
        db = 20 * np.log10(rms)
        s = section_at(p, b)
        bar = "#" * max(0, int((db + 40) * 1.5))
        print(f"bar {b + 1:3d} {b * bar_s:7.1f}s {s['name'] if s else '-':8s} {db:6.1f} dB {bar}")


def cut(song_dir, slug, start_bar, bars, out_dir, title=None, key=None):
    p = project(song_dir)
    bar_s = 240.0 / p["bpm"]
    start = start_bar * bar_s
    dur = bars * bar_s
    fade_in, fade_out = 0.4, 3.0
    mp3 = f"{out_dir}/{slug}.mp3"
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-ss", f"{start:.6f}", "-t", f"{dur:.6f}", "-i", f"{song_dir}/song.wav",
            "-af", f"afade=t=in:st=0:d={fade_in},afade=t=out:st={dur - fade_out:.3f}:d={fade_out}",
            "-ar", str(SR), "-ac", "2", "-c:a", "libmp3lame", "-b:a", "160k",
            "-map_metadata", "-1", "-id3v2_version", "3",
            "-metadata", f"title={title or p['title']}", "-metadata", "artist=claudius",
            "-metadata", "comment=Made with claudius",
            mp3,
        ],
        check=True,
    )
    x = load(out_dir, src=f"{slug}.mp3")
    duration = len(x) / SR
    mono = np.max(np.abs(x), axis=1)
    sq = np.mean(x**2, axis=1)
    edges = np.linspace(0, len(x), POINTS + 1).astype(int)
    peaks, rms = [], []
    for a, b in zip(edges[:-1], edges[1:]):
        peaks.append(float(mono[a:b].max()) if b > a else 0.0)
        rms.append(float(np.sqrt(sq[a:b].mean())) if b > a else 0.0)
    q = lambda v: [min(255, int(round(min(1.0, t) * 255))) for t in v]
    sections = []
    end_bar = start_bar + bars
    for s in p["sections"]:
        a, b = s["start_bar"], s["start_bar"] + s["bars"]
        lo, hi = max(a, start_bar), min(b, end_bar)
        if hi > lo:
            sections.append({
                "name": s["name"], "kind": s["kind"],
                "start": round((lo - start_bar) * bar_s, 3), "end": round(min(duration, (hi - start_bar) * bar_s), 3),
            })
    meta = {
        "title": title or p["title"],
        "genre": p["genre"],
        "bpm": p["bpm"],
        "key": key,
        "songSeconds": round(p["bars"] * bar_s, 2),
        "songBars": p["bars"],
        "startBar": start_bar,
        "startSeconds": round(start, 3),
        "duration": round(duration, 3),
        "points": POINTS,
        "scale": 255,
        "peaks": q(peaks),
        "rms": q(rms),
        "sections": sections,
    }
    with open(f"{out_dir}/{slug}.json", "w") as f:
        json.dump(meta, f, separators=(",", ":"))
    print(f"{slug}: {start:.1f}s + {dur:.1f}s -> {duration:.2f}s, sections {[s['name'] for s in sections]}")


if __name__ == "__main__":
    if sys.argv[1] == "analyze":
        analyze(sys.argv[2])
    else:
        args = sys.argv[2:]
        opts = {}
        while "--title" in args or "--key" in args:
            i = next(i for i, a in enumerate(args) if a.startswith("--"))
            opts[args[i][2:]] = args[i + 1]
            del args[i : i + 2]
        cut(args[0], args[1], float(args[2]), float(args[3]), args[4], **opts)
