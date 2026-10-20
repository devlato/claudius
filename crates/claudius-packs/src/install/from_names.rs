//! Instruments from folders of samples, read from their file names: the note, the velocity layer and the round robin.

use super::*;

/// shell-style glob -> anchored regex (`*` stays inside a directory, `**` crosses them)
pub(crate) fn glob_regex(glob: &str) -> Result<Regex> {
    let mut re = String::from("^");
    let mut chars = glob.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                re.push_str(".*");
            }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push('$');
    Ok(Regex::new(&re)?)
}

pub(super) fn junk(path: &str) -> bool {
    path.contains("__MACOSX/")
        || path.ends_with(".DS_Store")
        || path.rsplit('/').next().is_some_and(|f| f.starts_with("._"))
}

pub(super) fn is_audio(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    [".wav", ".flac", ".mp3", ".ogg", ".aif", ".aiff", ".m4a", ".opus"].iter().any(|e| l.ends_with(e))
}

pub(super) fn stem(path: &str) -> &str {
    let f = path.rsplit('/').next().unwrap_or(path);
    match f.rfind('.') {
        Some(i) => &f[..i],
        None => f,
    }
}

/// assign key ranges between neighbouring roots (per velocity layer), extending the ends to the keyboard edges
pub(crate) fn spread_keys(zones: &mut [Zone]) {
    let mut layers: Vec<(u8, u8)> = zones.iter().map(|z| (z.lovel, z.hivel)).collect();
    layers.sort();
    layers.dedup();
    for layer in layers {
        let mut roots: Vec<i32> =
            zones.iter().filter(|z| (z.lovel, z.hivel) == layer).map(|z| z.root.round() as i32).collect();
        roots.sort();
        roots.dedup();
        for z in zones.iter_mut().filter(|z| (z.lovel, z.hivel) == layer) {
            let r = z.root.round() as i32;
            let i = roots.iter().position(|&x| x == r).expect("root");
            let lo = if i == 0 { 0 } else { (roots[i - 1] + r) / 2 + 1 };
            let hi = if i + 1 == roots.len() { 127 } else { (r + roots[i + 1]) / 2 };
            z.lokey = lo.clamp(0, 127) as u8;
            z.hikey = hi.clamp(0, 127) as u8;
        }
    }
}

/// velocity ranges for `n` layers spread evenly over 1..127
pub(super) fn layer_range(i: usize, n: usize) -> (u8, u8) {
    let b = |k: usize| ((127 * k) as f64 / n as f64).round() as u8;
    (if i == 0 { 0 } else { b(i) + 1 }, b(i + 1))
}

pub(super) fn zones_from_names(paths: &[(u32, String)], vel_re: Option<&Regex>, kind: Kind) -> Vec<Zone> {
    struct F {
        id: u32,
        note: Option<i32>,
        vel: Option<f32>,
        rr: Option<u32>,
    }
    let fs: Vec<F> = paths
        .iter()
        .map(|(id, p)| {
            let st = stem(p);
            let info = notes::analyze_name(st);
            let vel = vel_re
                .and_then(|re| re.captures(st))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse::<f32>().ok())
                .or(info.vel);
            F { id: *id, note: info.note, vel, rr: info.rr }
        })
        .collect();
    let mut vels: Vec<f32> = fs.iter().filter_map(|f| f.vel).collect();
    vels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    vels.dedup();
    let pitched = kind == Kind::Pitched && fs.iter().any(|f| f.note.is_some());
    let mut zones: Vec<Zone> = Vec::new();
    for f in &fs {
        let layer = f.vel.and_then(|v| vels.iter().position(|x| *x == v)).unwrap_or(0);
        let (lovel, hivel) = if vels.len() > 1 { layer_range(layer, vels.len()) } else { (0, 127) };
        let root = if pitched { f.note.unwrap_or(60) as f32 } else { 60.0 };
        zones.push(Zone { file: f.id, root, lovel, hivel, ..Default::default() });
    }
    // round robins: files sharing root and layer
    let mut groups: BTreeMap<(i32, u8), Vec<usize>> = BTreeMap::new();
    for (i, z) in zones.iter().enumerate() {
        groups.entry((z.root.round() as i32, z.lovel)).or_default().push(i);
    }
    for (_, mut idx) in groups {
        if idx.len() < 2 {
            continue;
        }
        idx.sort_by_key(|&i| (fs[i].rr.unwrap_or(0), fs[i].id));
        let n = idx.len().min(64) as u8;
        for (pos, &i) in idx.iter().enumerate() {
            zones[i].seq_len = n;
            zones[i].seq_pos = (pos as u8 % n) + 1;
        }
    }
    if pitched {
        spread_keys(&mut zones);
    }
    zones
}

/// the audio files of a listing that the glob matches and `exclude` (a regex) does not, sorted
pub(super) fn matching(listing: &[(String, u64)], glob: &str, exclude: Option<&str>) -> Result<Vec<String>> {
    let re = glob_regex(glob)?;
    let exclude = exclude.map(Regex::new).transpose()?;
    let mut matched: Vec<String> = listing
        .iter()
        .map(|(p, _)| p.clone())
        .filter(|p| re.is_match(p) && !junk(p) && is_audio(p))
        .filter(|p| !exclude.as_ref().is_some_and(|x| x.is_match(p)))
        .collect();
    matched.sort();
    Ok(matched)
}

pub(super) fn build_files(ctx: &mut Ctx, src_key: &str, spec: &InstSpec) -> Result<InstrumentDef> {
    let kind = spec_kind(spec);
    let listing = ctx.sources.get_mut(src_key).expect("source").list()?;
    let parts: Vec<&InstSpec> = match &spec.variants {
        Some(v) => v.iter().collect(),
        None => vec![spec],
    };
    let mut variants: Vec<Variant> = Vec::new();
    for part in parts {
        let glob = part.files.as_deref().ok_or_else(|| anyhow!("{}: variant without files", spec.name))?;
        let mut matched = matching(&listing, glob, part.exclude.as_deref().or(spec.exclude.as_deref()))?;
        if matched.is_empty() {
            bail!("{}: no files match {glob}", spec.name);
        }
        if part.each {
            if let Some(first) = &part.first
                && let Some(i) = matched.iter().position(|p| p.rsplit('/').next() == Some(first.as_str()))
            {
                let f = matched.remove(i);
                matched.insert(0, f);
            }
            for p in matched {
                let id = ctx.add_source_file(src_key, &p);
                let root =
                    if kind == Kind::Pitched { notes::analyze_name(stem(&p)).note.unwrap_or(60) as f32 } else { 60.0 };
                variants.push(Variant {
                    name: stem(&p).to_string(),
                    zones: vec![Zone { file: id, root, ..Default::default() }],
                });
            }
        } else {
            let vel_re = part.vel.as_deref().map(Regex::new).transpose()?;
            let ids: Vec<(u32, String)> =
                matched.iter().map(|p| (ctx.add_source_file(src_key, p), p.clone())).collect();
            let zones = zones_from_names(&ids, vel_re.as_ref(), kind);
            variants.push(Variant { name: part.name.clone(), zones });
        }
    }
    Ok(base_def(spec, &spec.name, kind, variants))
}
