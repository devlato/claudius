//! Instruments from SFZ files: regions to zones, with the controller defaults, switches and release samples
//! an SFZ player would apply.

use super::*;

pub(super) fn cc_default(file: &SfzFile, cc: u32) -> f32 {
    if let Some(v) = file.cc.get(&cc) {
        return *v;
    }
    match cc {
        7 => 100.0 / 127.0,
        10 => 0.5,
        11 => 1.0,
        _ => 0.0,
    }
}

/// value of an opcode plus its `_onccN` modulations at the default controller values
pub(super) fn modded(r: &SfzRegion, file: &SfzFile, base: &str, default: f32) -> f32 {
    let mut v = r.f32(base).unwrap_or(default);
    let prefix = format!("{base}_oncc");
    for (k, val) in &r.opcodes {
        let Some(n) = k.strip_prefix(&prefix) else { continue };
        let (Ok(cc), Ok(depth)) = (n.parse::<u32>(), val.trim().parse::<f32>()) else { continue };
        let x = cc_default(file, cc);
        let curve = r.get(&format!("{base}_curvecc{cc}")).and_then(|c| c.trim().parse::<u32>().ok()).unwrap_or(0);
        let y = match curve {
            1 => 2.0 * x - 1.0,
            2 => 1.0 - x,
            3 => 1.0 - 2.0 * x,
            4 => x * x,
            5 => x.sqrt(),
            _ => x,
        };
        v += depth * y;
    }
    v
}

pub(super) fn region_plays(r: &SfzRegion, file: &SfzFile, sw_default: Option<i32>) -> bool {
    if r.get("sample").is_none() {
        return false;
    }
    if let Some(t) = r.get("trigger")
        && !matches!(t.trim(), "attack" | "first")
    {
        return false;
    }
    if let Some(sw) = r.key("sw_last")
        && let Some(d) = sw_default
        && sw != d
    {
        return false;
    }
    if r.get("sw_down").is_some() || r.get("sw_up").is_some() || r.get("sw_previous").is_some() {
        return false;
    }
    for (k, v) in &r.opcodes {
        if k.starts_with("on_locc")
            || k.starts_with("on_hicc")
            || k.starts_with("start_locc")
            || k.starts_with("start_hicc")
        {
            return false;
        }
        let (lo, n) = if let Some(n) = k.strip_prefix("locc") {
            (true, n)
        } else if let Some(n) = k.strip_prefix("hicc") {
            (false, n)
        } else {
            continue;
        };
        let (Ok(cc), Ok(lim)) = (n.parse::<u32>(), v.trim().parse::<f32>()) else { continue };
        let val = cc_default(file, cc) * 127.0;
        if (lo && val < lim) || (!lo && val > lim) {
            return false;
        }
    }
    true
}

pub(super) fn sfz_zone(r: &SfzRegion, file: &SfzFile, file_id: u32, keep_release: bool) -> Zone {
    let key = r.key("key");
    let lokey = r.key("lokey").or(key).unwrap_or(0).clamp(0, 127);
    let hikey = r.key("hikey").or(key).unwrap_or(127).clamp(0, 127);
    let pkc = r.key("pitch_keycenter").or(key).unwrap_or(60);
    let tune = modded(r, file, "tune", 0.0);
    let transpose = r.f32("transpose").unwrap_or(0.0);
    let vel = |k: &str, d: f32| r.f32(k).unwrap_or(d).clamp(0.0, 127.0) as u8;
    let xfin = (r.get("xfin_lovel").is_some() || r.get("xfin_hivel").is_some())
        .then(|| (vel("xfin_lovel", 0.0), vel("xfin_hivel", 0.0)));
    let xfout = (r.get("xfout_lovel").is_some() || r.get("xfout_hivel").is_some())
        .then(|| (vel("xfout_lovel", 127.0), vel("xfout_hivel", 127.0)));
    let amplitude = r.f32("amplitude").unwrap_or(100.0).max(0.001);
    let gain_db =
        r.f32("volume").unwrap_or(0.0) + r.f32("group_volume").unwrap_or(0.0) + 20.0 * (amplitude / 100.0).log10();
    let loop_start = r.f32("loop_start").unwrap_or(0.0).max(0.0) as u32;
    let loop_end = r.f32("loop_end").unwrap_or(0.0).max(0.0) as u32;
    let loop_mode = match r.get("loop_mode").map(|s| s.trim()) {
        Some("loop_continuous") => LoopMode::Continuous,
        Some("loop_sustain") => LoopMode::Sustain,
        Some("one_shot") => LoopMode::OneShot,
        Some(_) => LoopMode::NoLoop,
        None if loop_end > loop_start => LoopMode::Continuous,
        None => LoopMode::NoLoop,
    };
    let release = if keep_release && (r.get("ampeg_release").is_some()) {
        Some(modded(r, file, "ampeg_release", 0.0).max(0.01))
    } else {
        None
    };
    let attack = r.get("ampeg_attack").map(|_| modded(r, file, "ampeg_attack", 0.0).max(0.0));
    Zone {
        file: file_id,
        root: (pkc as f32 - transpose - tune / 100.0),
        lokey: lokey as u8,
        hikey: hikey as u8,
        lovel: vel("lovel", 0.0),
        hivel: vel("hivel", 127.0),
        xfin,
        xfout,
        seq_len: r.f32("seq_length").unwrap_or(1.0).clamp(1.0, 64.0) as u8,
        seq_pos: r.f32("seq_position").unwrap_or(1.0).clamp(1.0, 64.0) as u8,
        lorand: r.f32("lorand").unwrap_or(0.0),
        hirand: r.f32("hirand").unwrap_or(1.0),
        gain_db,
        pan: r.f32("pan").unwrap_or(0.0).clamp(-100.0, 100.0),
        offset: modded(r, file, "offset", 0.0).max(0.0) as u32,
        end: r.f32("end").unwrap_or(0.0).max(0.0) as u32,
        loop_mode,
        loop_start,
        loop_end,
        keytrack: r.f32("pitch_keytrack").unwrap_or(100.0) / 100.0,
        release,
        attack,
        hold: r.f32("ampeg_hold").unwrap_or(0.0).max(0.0),
        decay: r.f32("ampeg_decay").unwrap_or(0.0).max(0.0),
        sustain: (r.f32("ampeg_sustain").unwrap_or(100.0) / 100.0).clamp(0.0, 1.0),
    }
}

/// keep `n` velocity layers spread across the range and widen them so every velocity is covered
pub(crate) fn keep_layers(zones: &mut Vec<Zone>, n: usize) {
    let mut layers: Vec<(u8, u8)> = zones.iter().map(|z| (z.lovel, z.hivel)).collect();
    layers.sort();
    layers.dedup();
    if n == 0 || layers.len() <= n {
        return;
    }
    let l = layers.len();
    let mut idx: Vec<usize> = (0..n).map(|i| ((i * (l - 1)) as f64 / (n - 1).max(1) as f64).round() as usize).collect();
    idx.dedup();
    let kept: Vec<(u8, u8)> = idx.iter().map(|&i| layers[i]).collect();
    let ranges: Vec<(u8, u8)> = (0..kept.len())
        .map(|j| {
            let lo = if j == 0 { 0 } else { kept[j].0 };
            let hi = if j + 1 == kept.len() { 127 } else { kept[j + 1].0.saturating_sub(1) };
            (lo, hi)
        })
        .collect();
    zones.retain(|z| kept.contains(&(z.lovel, z.hivel)));
    for z in zones.iter_mut() {
        let j = kept.iter().position(|k| *k == (z.lovel, z.hivel)).expect("kept layer");
        z.lovel = ranges[j].0;
        z.hivel = ranges[j].1;
        z.xfin = None;
        z.xfout = None;
    }
}

/// re-root drum zones so pitch 60 plays the sample as recorded, on every key
pub(super) fn drumify(z: &mut Zone, center: f32) {
    z.root = 60.0 + (z.root - center);
    z.lokey = 0;
    z.hikey = 127;
}

pub(super) fn build_sfz(ctx: &mut Ctx, src_key: &str, spec: &InstSpec) -> Result<Vec<InstrumentDef>> {
    let sfz_path = spec.sfz.as_deref().expect("sfz");
    let base_dir = match sfz_path.rfind('/') {
        Some(i) => sfz_path[..i].to_string(),
        None => String::new(),
    };
    let text = ctx.sources[src_key].read_text(sfz_path).with_context(|| format!("reading {sfz_path}"))?;
    let file = {
        let src = &ctx.sources[src_key];
        // mappings include the same small files many times (Salamander: once per velocity layer)
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut inc = |p: &str| -> Result<String> {
            let full = if base_dir.is_empty() { p.to_string() } else { format!("{base_dir}/{p}") };
            let full = sfz::normalize_path(&full);
            if let Some(t) = seen.get(&full) {
                return Ok(t.clone());
            }
            let t = src.read_text(&full)?;
            seen.insert(full, t.clone());
            Ok(t)
        };
        sfz::parse(&text, &mut inc).with_context(|| format!("parsing {sfz_path}"))?
    };
    let sw_default = file.regions.iter().find_map(|r| r.key("sw_default"));
    let exclude = spec.exclude.as_deref().map(Regex::new).transpose()?;
    let kind = spec_kind(spec);
    let mut zones: Vec<Zone> = Vec::new();
    let mut centers: Vec<f32> = Vec::new();
    let mut veltrack: Option<f32> = None;
    let mut all_one_shot = true;
    for r in &file.regions {
        if !region_plays(r, &file, sw_default) {
            continue;
        }
        let path = sfz::join_path(&base_dir, &r.default_path, r.get("sample").unwrap_or_default());
        if path.starts_with('*') {
            // built-in oscillators (*sine, *noise) are not samples
            continue;
        }
        if exclude.as_ref().is_some_and(|re| re.is_match(&path)) {
            continue;
        }
        let z0 = sfz_zone(r, &file, 0, spec.keep_zone_release);
        if let Some([lo, hi]) = spec.keys
            && (z0.lokey < lo || z0.hikey > hi)
        {
            continue;
        }
        let id = ctx.add_source_file(src_key, &path);
        let mut z = z0;
        z.file = id;
        if veltrack.is_none() {
            veltrack = Some(modded(r, &file, "amp_veltrack", 100.0) / 100.0);
        }
        if z.loop_mode != LoopMode::OneShot {
            all_one_shot = false;
        }
        centers.push(r.key("pitch_keycenter").or(r.key("key")).unwrap_or(60) as f32);
        zones.push(z);
    }
    if zones.is_empty() {
        bail!("{sfz_path}: no playable regions");
    }
    let make = |name: &str, variants: Vec<Variant>| {
        let mut d = base_def(spec, name, kind, variants);
        if spec.veltrack.is_none() {
            d.veltrack = veltrack.unwrap_or(1.0).clamp(0.0, 1.0);
        }
        if spec.one_shot.is_none() && all_one_shot {
            d.one_shot = true;
        }
        d
    };
    if let Some(kit) = &spec.kit {
        let mut out = Vec::new();
        for (name, keys) in kit {
            let mut variants = Vec::new();
            for &k in keys {
                let mut vz: Vec<Zone> = Vec::new();
                for (z, &c) in zones.iter().zip(&centers) {
                    if z.plays_key(k) {
                        let mut z = z.clone();
                        drumify(&mut z, c);
                        vz.push(z);
                    }
                }
                if !vz.is_empty() {
                    variants.push(Variant { name: notes::note_name(k as i32), zones: vz });
                }
            }
            if !variants.is_empty() {
                let mut d = make(name, variants);
                d.mono = matches!(name.as_str(), "hh" | "oh") || d.mono;
                out.push(d);
            }
        }
        return Ok(out);
    }
    if spec.kit_auto {
        let mut ranges: Vec<(u8, u8)> = zones.iter().map(|z| (z.lokey, z.hikey)).collect();
        ranges.sort();
        ranges.dedup();
        let variants = ranges
            .iter()
            .map(|&(lo, hi)| {
                let vz: Vec<Zone> = zones
                    .iter()
                    .zip(&centers)
                    .filter(|(z, _)| (z.lokey, z.hikey) == (lo, hi))
                    .map(|(z, &c)| {
                        let mut z = z.clone();
                        drumify(&mut z, c);
                        z
                    })
                    .collect();
                Variant { name: notes::note_name(lo as i32), zones: vz }
            })
            .collect();
        return Ok(vec![make(&spec.name, variants)]);
    }
    if let Some(n) = spec.keep_layers {
        keep_layers(&mut zones, n);
    }
    if let Some(o) = spec.octave.filter(|o| *o != 0) {
        let d = 12 * o;
        for z in &mut zones {
            z.root += d as f32;
            z.lokey = (z.lokey as i32 + d).clamp(0, 127) as u8;
            z.hikey = (z.hikey as i32 + d).clamp(0, 127) as u8;
        }
    }
    if kind == Kind::Drum {
        for (z, &c) in zones.iter_mut().zip(&centers) {
            drumify(z, c);
        }
    }
    Ok(vec![make(&spec.name, vec![Variant { name: String::new(), zones }])])
}
