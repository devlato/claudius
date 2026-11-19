//! A recording's stems: its vocals, its drums, its bass and the rest, apart. With the ears, Spleeter separates them
//! (`earshot::separate`): once for a recording, kept for the session and in the cache folder by what the recording
//! is, for the three recordings separated last. Without the ears there are none, and what wants a recording's voice
//! or its drums hears them in the mix instead.

use std::sync::Arc;

use crate::audio::Stereo;

/// The four stems of a recording, each as long as it.
#[derive(Clone, Debug)]
pub struct Stems {
    pub vocals: Stereo,
    pub drums: Stereo,
    pub bass: Stereo,
    pub other: Stereo,
}

impl Stems {
    fn sum(parts: &[&Stereo]) -> Stereo {
        let mut out = Stereo::zeros(parts.iter().map(|p| p.len()).max().unwrap_or(0));
        for p in parts {
            out.add_at(p, 0, 1.0, 1.0);
        }
        out
    }

    /// what plays besides the drums: the vocals, the bass and the rest
    pub fn music(&self) -> Stereo {
        Self::sum(&[&self.vocals, &self.bass, &self.other])
    }

    /// what plays above the drums and the bass: the vocals and the rest
    pub fn upper(&self) -> Stereo {
        Self::sum(&[&self.vocals, &self.other])
    }

    /// what plays under the voice besides the drums: the bass and the rest
    pub fn backing(&self) -> Stereo {
        Self::sum(&[&self.bass, &self.other])
    }

    /// the share of the recording's power a stem holds, 0 to 1
    pub fn share(&self, stem: &Stereo) -> f64 {
        let power = |x: &Stereo| x.l.iter().chain(&x.r).map(|v| (*v as f64).powi(2)).sum::<f64>();
        let all = power(&self.vocals) + power(&self.drums) + power(&self.bass) + power(&self.other);
        if all > 0.0 { power(stem) / all } else { 0.0 }
    }

    /// whether a drum kit plays in it: its drums hold a twentieth of its power at least (a song with no drums leaves
    /// its drum stem a few percent at most, the strums and plucks the net hears as struck)
    pub fn drummed(&self) -> bool {
        self.share(&self.drums) >= 0.05
    }

    /// whether it sings: its vocals hold a twentieth of its power at least
    pub fn sung(&self) -> bool {
        self.share(&self.vocals) >= 0.05
    }
}

/// The stems of `x`, separated with the ears or kept from before; none without the ears, when their nets are neither
/// in the cache nor `may_download`, or when they cannot load. `progress` hears the share done, 0 to 1.
pub fn of(x: &Stereo, progress: &mut dyn FnMut(f64)) -> Option<Arc<Stems>> {
    separating::of(x, progress)
}

static MAY_DOWNLOAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Lets `of` download Spleeter's nets (about 160 MB) when they are not in the cache, and `transcribe::notes` Basic
/// Pitch's model (0.2 MB): the ears were asked for (`Ears::load`), whose models are downloaded the same way. Until then
/// only models already there listen, so that nothing a song does downloads anything unasked.
pub fn may_download() {
    MAY_DOWNLOAD.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// whether `may_download` has let the stems' and the notes' models be downloaded
pub fn downloads() -> bool {
    MAY_DOWNLOAD.load(std::sync::atomic::Ordering::Relaxed)
}

/// How surely each of `spans` of `x` (start and end, in seconds) is a human voice, 0 to 1: the likeliest of AudioSet's
/// classes of one (speech, shouting, singing, rapping, humming, a choir) as CED-Tiny, an AudioSet tagger among the ears'
/// models (6.7 MB, `earshot::ced`), hears each stretch at its own length. None without it: no ears, or its model neither
/// in the cache nor `may_download`. A separation hears a riff, a lead or a pad as partly a voice, a distorted guitar most
/// of all, and leaves it in the vocals, where the tagger hears an instrument.
pub fn voiced(x: &Stereo, spans: &[(f64, f64)]) -> Option<Vec<f64>> {
    tagging::voiced(x, spans)
}

#[cfg(feature = "ears")]
mod tagging {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use crate::audio::{self, Stereo};
    use crate::config::{self, SRF};

    /// AudioSet's classes of a human voice, as its taggers name them (CED-Tiny names four of AST's more briefly)
    const VOICE: &[&str] = &[
        "Speech",
        "Male speech, man speaking",
        "Male speech",
        "Female speech, woman speaking",
        "Female speech",
        "Child speech, kid speaking",
        "Child speech",
        "Conversation",
        "Narration, monologue",
        "Narration",
        "Babbling",
        "Speech synthesizer",
        "Shout",
        "Bellow",
        "Whoop",
        "Yell",
        "Battle cry",
        "Children shouting",
        "Screaming",
        "Whispering",
        "Singing",
        "Choir",
        "Yodeling",
        "Chant",
        "Mantra",
        "Male singing",
        "Female singing",
        "Child singing",
        "Synthetic singing",
        "Rapping",
        "Humming",
        "Vocal music",
        "A capella",
    ];

    /// the tagger, loaded the first time it is asked for, with the places of its classes of a voice; none while its
    /// model is not in the cache and may not be downloaded, or when it cannot load, which is said once. Its absence is
    /// not kept: once the ears allow the download (`may_download`), the next song asks for it again and loads it, as
    /// the separator is loaded.
    fn tagger() -> Option<&'static (earshot::ced::Ced, Vec<usize>)> {
        static T: OnceLock<Option<(earshot::ced::Ced, Vec<usize>)>> = OnceLock::new();
        let cache = earshot::models::default_cache_dir();
        let offline = !super::downloads();
        if T.get().is_none() && offline && !earshot::ced::Ced::cached(&cache) {
            return None;
        }
        T.get_or_init(|| match earshot::ced::Ced::load(&cache, offline, earshot::models::default_threads()) {
            Ok(ced) => {
                let voice = VOICE.iter().filter_map(|v| ced.labels.iter().position(|l| l == v)).collect();
                Some((ced, voice))
            }
            Err(e) => {
                eprintln!("claudius: the voice cannot be told from the instruments: {e:#}");
                None
            }
        })
        .as_ref()
    }

    /// what the tagger heard in a voice's recording, kept by what the recording is and which tagger heard it: each
    /// stretch asked about, and how surely it heard a voice in it
    fn heard_before(x: &Stereo) -> PathBuf {
        config::cache_dir("voiced").join(format!("{}-ced.json", &audio::fingerprint(x).to_hex()[..20]))
    }

    pub fn voiced(x: &Stereo, spans: &[(f64, f64)]) -> Option<Vec<f64>> {
        // a unit test hears what it makes the same with and without the models on its computer
        if cfg!(test) {
            return None;
        }
        let path = config::FILES.then(|| heard_before(x));
        let mut known: Vec<(f64, f64, f64)> = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let find = |known: &[(f64, f64, f64)], s: &(f64, f64)| {
            known.iter().find(|k| (k.0 - s.0).abs() < 1e-6 && (k.1 - s.1).abs() < 1e-6).map(|k| k.2)
        };
        let missing: Vec<(f64, f64)> = spans.iter().copied().filter(|s| find(&known, s).is_none()).collect();
        if !missing.is_empty() {
            let (ced, voice) = tagger()?;
            let mono = x.mono();
            let at = |t: f64| ((t.max(0.0) * SRF) as usize).min(mono.len());
            let segs: Vec<&[f32]> = missing.iter().map(|&(a, b)| &mono[at(a)..at(b).max(at(a))]).collect();
            let probs = ced
                .probs(&segs)
                .map_err(|e| eprintln!("claudius: the voice could not be told from the instruments: {e:#}"))
                .ok()?;
            let sure = |p: &Vec<f32>| voice.iter().map(|&i| p[i] as f64).fold(0.0, f64::max);
            known.extend(missing.iter().zip(&probs).map(|(s, p)| (s.0, s.1, sure(p))));
            if let Some(p) = &path {
                let tmp = p.with_extension(format!("{}.tmp", config::process_id()));
                if serde_json::to_vec(&known).is_ok_and(|b| std::fs::write(&tmp, b).is_ok()) {
                    let _ = std::fs::rename(&tmp, p);
                }
            }
        }
        spans.iter().map(|s| find(&known, s)).collect()
    }
}

#[cfg(not(feature = "ears"))]
mod tagging {
    use crate::audio::Stereo;

    pub fn voiced(_: &Stereo, _: &[(f64, f64)]) -> Option<Vec<f64>> {
        None
    }
}

#[cfg(feature = "ears")]
mod separating {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, OnceLock};

    use super::Stems;
    use crate::audio::{self, Stereo};
    use crate::config;

    /// the recordings separated last, kept in memory, the latest last
    const KEPT: usize = 2;
    /// the recordings whose stems the cache folder keeps
    const ON_DISK: usize = 3;
    const NAMES: [&str; 4] = earshot::separate::STEMS;

    type Kept = Mutex<Vec<(String, Arc<Stems>)>>;

    fn kept() -> &'static Kept {
        static K: OnceLock<Kept> = OnceLock::new();
        K.get_or_init(|| Mutex::new(Vec::new()))
    }

    /// Spleeter, loaded the first time it is asked for (its nets downloaded first when they may be, unless offline);
    /// none while its nets are not in the cache and may not be downloaded, and none when it cannot load, which is said
    /// once
    fn separator() -> Option<&'static Mutex<Option<earshot::separate::Separator>>> {
        static S: OnceLock<Mutex<Option<earshot::separate::Separator>>> = OnceLock::new();
        let cache = earshot::models::default_cache_dir();
        let allowed = super::MAY_DOWNLOAD.load(std::sync::atomic::Ordering::Relaxed);
        if S.get().is_none() && !allowed && !earshot::separate::Separator::cached(&cache) {
            return None;
        }
        Some(S.get_or_init(|| {
            let loaded = earshot::separate::Separator::load(&cache, false, earshot::models::default_threads());
            if let Err(e) = &loaded {
                eprintln!("claudius: the stems cannot be separated: {e:#}");
            }
            Mutex::new(loaded.ok())
        }))
    }

    fn key(x: &Stereo) -> String {
        audio::fingerprint(x).to_hex()[..20].to_string()
    }

    fn folder(key: &str) -> PathBuf {
        config::cache_dir("stems").join(key)
    }

    fn from_disk(dir: &Path, len: usize) -> Option<Stems> {
        let load = |name: &str| -> Option<Stereo> {
            let mut s = audio::load(&dir.join(format!("{name}.flac"))).ok()?;
            s.ensure_len(len);
            s.truncate(len);
            Some(s)
        };
        Some(Stems { vocals: load(NAMES[0])?, drums: load(NAMES[1])?, bass: load(NAMES[2])?, other: load(NAMES[3])? })
    }

    /// the stems into the cache folder, and the folders of all but the latest few recordings out of it
    fn to_disk(dir: &Path, s: &Stems) {
        if !config::FILES {
            return;
        }
        let tmp = dir.with_extension(format!("{}.tmp", config::process_id()));
        let wrote = [&s.vocals, &s.drums, &s.bass, &s.other]
            .iter()
            .zip(NAMES)
            .all(|(x, name)| crate::take::store_as(x, &tmp, name).is_ok());
        if !wrote || std::fs::rename(&tmp, dir).is_err() {
            let _ = std::fs::remove_dir_all(&tmp);
            return;
        }
        let Some(parent) = dir.parent() else { return };
        let made =
            |e: &std::fs::DirEntry| e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok();
        let mut all: Vec<(std::time::Duration, PathBuf)> = std::fs::read_dir(parent)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| Some((made(&e)?, e.path())))
            .filter(|(_, p)| p.is_dir())
            .collect();
        all.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
        for (_, old) in all.iter().skip(ON_DISK) {
            let _ = std::fs::remove_dir_all(old);
        }
    }

    pub fn of(x: &Stereo, progress: &mut dyn FnMut(f64)) -> Option<Arc<Stems>> {
        if x.is_empty() {
            return None;
        }
        let k = key(x);
        if let Some((_, s)) = kept().lock().unwrap().iter().find(|(kk, _)| *kk == k) {
            progress(1.0);
            return Some(s.clone());
        }
        let dir = folder(&k);
        let stems = match from_disk(&dir, x.len()) {
            Some(s) => s,
            None => {
                let mut guard = separator()?.lock().unwrap_or_else(|e| e.into_inner());
                let sep = guard.as_mut()?;
                let [vocals, drums, bass, other] =
                    sep.separate(&(x.l.clone(), x.r.clone()), config::SR, progress).ok()?;
                let st = |(l, r): (Vec<f32>, Vec<f32>)| Stereo { l, r };
                let s = Stems { vocals: st(vocals), drums: st(drums), bass: st(bass), other: st(other) };
                to_disk(&dir, &s);
                // heard as the cache will give them back, at 24 bits, so that a recording is heard the same the first
                // time and every time after: a song's sections shifted on the last bits of its stems
                from_disk(&dir, x.len()).unwrap_or(s)
            }
        };
        progress(1.0);
        let s = Arc::new(stems);
        let mut k_ = kept().lock().unwrap();
        k_.push((k, s.clone()));
        let excess = k_.len().saturating_sub(KEPT);
        k_.drain(..excess);
        Some(s)
    }
}

#[cfg(not(feature = "ears"))]
mod separating {
    use std::sync::Arc;

    use super::Stems;
    use crate::audio::Stereo;

    pub fn of(_: &Stereo, _: &mut dyn FnMut(f64)) -> Option<Arc<Stems>> {
        None
    }
}
