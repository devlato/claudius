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
/// classes of one (speech, shouting, singing, rapping, humming, a choir) as the ears' AudioSet tagger (AST) hears it.
/// None without the tagger: no ears, or its model (347 MB, the one the verdict's `describe` reads with) neither in the
/// cache nor `may_download`. A separation hears a riff, a lead or a pad as partly a voice, a distorted guitar most of
/// all, and leaves it in the vocals, where the tagger hears an instrument.
pub fn voiced(x: &Stereo, spans: &[(f64, f64)]) -> Option<Vec<f64>> {
    tagging::voiced(x, spans)
}

#[cfg(feature = "ears")]
mod tagging {
    use std::sync::OnceLock;

    use crate::audio::Stereo;
    use crate::config::SRF;

    /// AudioSet's classes of a human voice, as its tagger names them
    const VOICE: &[&str] = &[
        "Speech",
        "Male speech, man speaking",
        "Female speech, woman speaking",
        "Child speech, kid speaking",
        "Conversation",
        "Narration, monologue",
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
    /// model is not in the cache and may not be downloaded, or when it cannot load, which is said once
    fn tagger() -> Option<&'static (earshot::ast::Ast, Vec<usize>)> {
        static T: OnceLock<Option<(earshot::ast::Ast, Vec<usize>)>> = OnceLock::new();
        T.get_or_init(|| {
            use earshot::models::*;
            let cache = default_cache_dir();
            let offline = !super::downloads();
            let cached = [AST_MODEL, AST_CONFIG].iter().all(|f| model_path(&cache, AST_REPO, AST_REV, f).is_file());
            if !cached && offline {
                return None;
            }
            let load = || -> anyhow::Result<earshot::ast::Ast> {
                let model = fetch(&cache, AST_REPO, AST_REV, AST_MODEL, offline)?;
                let config = fetch(&cache, AST_REPO, AST_REV, AST_CONFIG, offline)?;
                earshot::ast::Ast::new(session(&model, default_threads())?, &config)
            };
            match load() {
                Ok(ast) => {
                    let voice = VOICE.iter().filter_map(|v| ast.labels.iter().position(|l| l == v)).collect();
                    Some((ast, voice))
                }
                Err(e) => {
                    eprintln!("claudius: the voice cannot be told from the instruments: {e:#}");
                    None
                }
            }
        })
        .as_ref()
    }

    pub fn voiced(x: &Stereo, spans: &[(f64, f64)]) -> Option<Vec<f64>> {
        let (ast, voice) = tagger()?;
        let mono = x.mono();
        let at = |t: f64| ((t.max(0.0) * SRF) as usize).min(mono.len());
        let segs: Vec<&[f32]> = spans.iter().map(|&(a, b)| &mono[at(a)..at(b).max(at(a))]).collect();
        let probs = ast.probs(&segs, earshot::ast::Reduce::Max).ok()?;
        Some(probs.iter().map(|p| voice.iter().map(|&i| p[i] as f64).fold(0.0, f64::max)).collect())
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
