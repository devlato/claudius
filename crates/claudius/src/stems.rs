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

/// Lets `of` download Spleeter's nets (about 160 MB) when they are not in the cache: the ears were asked for
/// (`Ears::load`), whose models are downloaded the same way. Until then only nets already there separate, so that
/// nothing a song does downloads anything unasked.
pub fn may_download() {
    MAY_DOWNLOAD.store(true, std::sync::atomic::Ordering::Relaxed);
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
        let mut h = blake3::Hasher::new();
        for (l, r) in x.l.iter().zip(&x.r) {
            h.update(&l.to_le_bytes());
            h.update(&r.to_le_bytes());
        }
        h.finalize().to_hex()[..20].to_string()
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
                s
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
