#!/usr/bin/env python
"""Dev-only parity fixtures for crates/earshot: run the Python earshot (the oracle) on a few inputs and dump its
results as JSON, which tests/parity.rs compares against the Rust port.

    .venv/bin/python crates/earshot/parity/make_fixtures.py   # from the repository root

Writes small synthetic WAV / FLAC files into parity/media/ and one JSON per input into parity/fixtures/. Real Apple
Loops are referenced by absolute path (skipped when missing; never copied into the repo).
"""
import json
import math
import os
import sys

import numpy as np
import soundfile as sf

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
MEDIA = os.path.join(HERE, 'media')
FIX = os.path.join(HERE, 'fixtures')
SR = 48000

APPLE = [
    '/Library/Audio/Apple Loops/Apple/06 Deep House/4th Dimension Beat 01.caf',
    '/Library/Audio/Apple Loops/Apple/03 Dubstep/Phantom Pulse Bass.caf',
    '/Library/Audio/Apple Loops/Apple/13 Drummer/Anders - Verse.caf',
    '/Library/Audio/Apple Loops/Apple/11 Blues Garage/Dusky Blues Guitar Chords.caf',
]
QUERIES = ['a kick drum', 'a sine wave test tone', 'white noise', 'a click', 'a fast air whoosh', 'house music',
           'techno music', 'a professionally mixed and mastered music track', 'a bass synthesizer', 'a drum loop']
PHRASES = ['a kick drum', 'house music', 'a professionally mixed and mastered music track', 'white noise',
           'a deep thocky mechanical keyboard press']


# ------------------------------------------------------------------------------------------------ synthetic media
def _env(n, a, d):
    t = np.arange(n) / SR
    return np.minimum(1.0, t / max(a, 1e-4)) * np.exp(-t / d)


def kick(n=int(0.6 * SR)):
    t = np.arange(n) / SR
    f = 45 + 120 * np.exp(-t / 0.03)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return 0.9 * np.sin(ph) * _env(n, 0.002, 0.15)


def snare(rng, n=int(0.25 * SR)):
    t = np.arange(n) / SR
    body = 0.4 * np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.04)
    noise = rng.standard_normal(n) * np.exp(-t / 0.06) * 0.5
    return body + noise


def hat(rng, n=int(0.06 * SR)):
    x = rng.standard_normal(n)
    x = np.diff(np.concatenate([[0], x]))  # brighten
    return 0.15 * x * np.exp(-np.arange(n) / SR / 0.015)


def make_media():
    os.makedirs(MEDIA, exist_ok=True)
    rng = np.random.default_rng(7)
    files = {}
    # 1 kHz sine, 2 s at -6 dBFS (mono file)
    t = np.arange(2 * SR) / SR
    files['sine.wav'] = (0.5 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)
    # white-noise burst with a short attack and decay, 1.5 s, decorrelated stereo
    n = int(1.5 * SR)
    e = _env(n, 0.01, 0.35)[:, None]
    files['noise.wav'] = (0.4 * rng.standard_normal((n, 2)) * e).astype(np.float32)
    # a single click in 0.5 s of silence
    x = np.zeros(int(0.5 * SR))
    x[2400:2400 + 48] = np.hanning(48) * 0.8
    files['click.wav'] = x.astype(np.float32)
    # log sine sweep 50 Hz -> 15 kHz, 4 s, stereo with a small level offset
    t = np.arange(4 * SR) / SR
    f0, f1, T = 50.0, 15000.0, 4.0
    ph = 2 * np.pi * f0 * T / math.log(f1 / f0) * (np.exp(t / T * math.log(f1 / f0)) - 1)
    s = 0.3 * np.sin(ph) * np.minimum(1, t / 0.05) * np.minimum(1, (T - t) / 0.05)
    files['sweep.wav'] = np.stack([s, 0.8 * s], 1).astype(np.float32)
    # synthetic kick drum
    files['kick.wav'] = kick().astype(np.float32)
    # harmonic tone with vibrato (sawtooth-like, 8 partials), 3 s: tonal, f0 ~ 220 Hz
    t = np.arange(3 * SR) / SR
    f = 220 * (1 + 0.004 * np.sin(2 * np.pi * 5 * t))
    ph = 2 * np.pi * np.cumsum(f) / SR
    tone = sum(np.sin(k * ph) / k for k in range(1, 9)) * 0.25 * np.minimum(1, t / 0.02) * np.exp(-t / 2.5)
    files['tone.wav'] = np.stack([tone, tone], 1).astype(np.float32)
    # 15 s drum loop at 120 BPM: kick on 1 and 3, snare on 2 and 4, hats on eighths (FLAC, stereo)
    n = 15 * SR
    loop = np.zeros((n, 2))
    beat = SR // 2
    for b in range(30):
        p = b * beat
        k = kick() if b % 2 == 0 else snare(rng)
        m = min(len(k), n - p)
        loop[p:p + m, :] += k[:m, None]
        for h in (0, beat // 2):
            q = p + h
            hh = hat(rng)
            m = min(len(hh), n - q)
            if m > 0:
                loop[q:q + m, 0] += 0.8 * hh[:m]
                loop[q:q + m, 1] += 1.0 * hh[:m]
    loop *= 0.7 / np.abs(loop).max()
    files['loop.wav'] = loop.astype(np.float32)
    out = {}
    for name, x in files.items():
        # short ones as WAV, the longer ones as FLAC (smaller; also exercises a second decoder)
        if name in ('noise.wav', 'sweep.wav', 'tone.wav', 'loop.wav'):
            name = name.replace('.wav', '.flac')
        p = os.path.join(MEDIA, name)
        sf.write(p, x, SR, subtype='PCM_16')
        out[name] = p
    for stale in ('noise.wav', 'sweep.wav', 'tone.wav'):
        if os.path.exists(os.path.join(MEDIA, stale)):
            os.remove(os.path.join(MEDIA, stale))
    return out


# ------------------------------------------------------------------------------------------------ helpers
def clean(o):
    if isinstance(o, dict):
        return {str(k): clean(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [clean(v) for v in o]
    if isinstance(o, np.ndarray):
        return clean(o.tolist())
    if isinstance(o, (np.floating,)):
        o = float(o)
    if isinstance(o, (np.integer,)):
        return int(o)
    if isinstance(o, (np.bool_,)):
        return bool(o)
    if isinstance(o, float) and (math.isnan(o) or math.isinf(o)):
        return None
    return o


def arr_stats(x):
    x = np.asarray(x, np.float64).ravel()
    return {'n': int(x.size), 'sum': float(x.sum()), 'sumsq': float((x * x).sum()), 'min': float(x.min()),
            'max': float(x.max()), 'head': x[:8].tolist(), 'mid': x[x.size // 2: x.size // 2 + 8].tolist()}


def rel(p):
    return os.path.relpath(p, CRATE) if p.startswith(CRATE) else p


# ------------------------------------------------------------------------------------------------ main
def main():
    from earshot import audio, compare, describe, features, match, realness, search, timeline, vocab
    from earshot.models import ast, clap

    media = make_media()
    inputs = [(k, v) for k, v in media.items()]
    for p in APPLE:
        if os.path.exists(p):
            inputs.append((os.path.basename(p), p))
        else:
            print(f'skipping missing Apple Loop: {p}', file=sys.stderr)
    os.makedirs(FIX, exist_ok=True)
    c = clap.get()
    v = vocab.get()
    t = ast.get()
    print(f'CLAP precision: {c.precision}, AST precision: {t.precision}', file=sys.stderr)
    common = {'clap_precision': c.precision, 'ast_precision': t.precision, 'logit_scale': vocab.logit_scale(),
              'phrases': {}}
    for ph in PHRASES:
        common['phrases'][ph] = {
            'tokens': c.proc.tokenizer(ph)['input_ids'],
            'raw': c.embed_text([ph])[0].tolist(),
            'templated': v.embed_text([ph])[0].tolist(),
            'kind': search.query_kind(v, ph),
        }
    # soxr resampling of the first 2 s of two decodes both sides can reproduce (resampler parity)
    common['resample'] = {}
    for src in ('loop.flac', 'noise.flac'):
        xr = audio.load(media[src], mono=True, cache=False)[:2 * SR]
        out = {}
        for sr in (16000, 22050, 32000):
            y = audio.resample(xr, SR, sr)
            h = len(y) // 2
            out[str(sr)] = {'stats': arr_stats(y), 'head': y[:256].tolist(), 'mid': y[h:h + 256].tolist(),
                            'tail': y[-256:].tolist()}
        common['resample'][src] = out
    with open(os.path.join(FIX, '_common.json'), 'w') as fh:
        json.dump(clean(common), fh)

    embs, names = [], []
    for name, path in inputs:
        print(f'{name} ...', file=sys.stderr)
        r = {'name': name, 'path': rel(path)}
        xs = audio.load(path, cache=False)
        xm = audio.load(path, mono=True, cache=False)
        r['decode'] = {'stereo': arr_stats(xs), 'mono': arr_stats(xm)}
        dur = len(xm) / SR
        # CLAP front end of the first window (embed_files semantics: first span, repeat-padded)
        sp = vocab.spans_for(0.0, max(dur, 0.01))
        seg = xm[int(round(sp[0][0] * SR)):int(round(sp[0][1] * SR))]
        feat = c.proc(audio=[c._prep(seg, 'repeatpad')], sampling_rate=SR, return_tensors='np')['input_features'][0, 0]
        r['clap_mel'] = {'shape': list(feat.shape), 'stats': arr_stats(feat), 'row0': feat[0].tolist(),
                         'row500': feat[500].tolist(), 'row1000': feat[-1].tolist(), 'col10': feat[::50, 10].tolist()}
        a = vocab.embed_files([path])[0]
        r['embedding'] = a.tolist()
        r['n_spans'] = len(sp)
        r['scores'] = v.scores(a[None], QUERIES)[0]
        r['match'] = match.score_queries(a, ['a kick drum'], ['a snare drum hit', 'a hi-hat'], v)
        r['match_multi'] = match.score_queries(a, ['house music', 'techno music'], None, v)
        r['top_terms'] = v.top_terms(a, k=8, exclude=match.SKIP_CATS)
        r['nearest_category'] = v.nearest_category(v.embed_text(['a kick drum']))[0]
        # realness (+ the per-clip signal features behind it)
        r['realness'] = realness.realness(path, cache=False)
        x, gain, rdb, pdb = realness.lift_quiet(xm)
        r['realness_clips'] = []
        for t0, t1, clip in realness._clips(x):
            f = realness.signal_features(clip) if realness.audible(clip) else None
            r['realness_clips'].append({'t0': t0, 't1': t1, 'features': f})
        r['realness_gain'] = gain
        # AudioSet
        p = t.probs([xm])[0]
        r['audioset'] = p.tolist()
        r['audioset_top10'] = t.top(p, 10)
        # measurements + beat grid
        r['measure'] = features.measure(xs, music=True)
        r['measure_light'] = features.measure(xs, light=True)
        r['onsets_mono'] = features.onsets(xm)
        if dur >= 5:
            r['grid'] = timeline.music_grid(audio.to_mono(xs))
            r['event_candidates'] = [list(cc) for cc in search.event_candidates(xm, n=None)]
        r['feats'] = compare.measure_path(path)
        if name in ('loop.flac', 'Anders - Verse.caf', 'kick.wav', 'click.wav'):
            q = 'a kick drum' if name != 'click.wav' else 'a click'
            r['find'] = search.find(path, q, top=5)
            r['find_query'] = q
        if name in ('kick.wav', 'loop.flac', 'sweep.flac'):
            r['describe'] = describe.describe(path)
        with open(os.path.join(FIX, os.path.splitext(name)[0].replace(' ', '_') + '.json'), 'w') as fh:
            json.dump(clean(r), fh)
        embs.append(a)
        names.append(name)

    # cross-file results
    E = np.stack(embs)
    cross = {'names': names, 'typicality': {}, 'differences': {}}
    for i, n in enumerate(names):
        refs = np.delete(E, i, axis=0)
        sx, z, pct = compare._typicality(E[i], refs, 3)
        cross['typicality'][n] = {'mean_cos_knn': sx, 'z': z, 'percentile': pct}
    synth = [p for _, p in inputs if p.startswith(MEDIA)]
    cross['rank'] = search.rank(synth, 'a kick drum')
    fa, fb = compare.measure_path(media['kick.wav']), compare.measure_path(media['click.wav'])
    cross['differences']['kick_vs_click'] = {'a': fa, 'b': fb, 'result': compare.differences(fa, fb)}
    fa, fb = compare.measure_path(media['tone.flac']), compare.measure_path(media['sweep.flac'])
    cross['differences']['tone_vs_sweep'] = {'a': fa, 'b': fb, 'result': compare.differences(fa, fb)}
    with open(os.path.join(FIX, '_cross.json'), 'w') as fh:
        json.dump(clean(cross), fh)
    print(f'wrote {len(inputs)} fixtures to {FIX}', file=sys.stderr)


if __name__ == '__main__':
    main()
