//! The ears: earshot (ported to Rust in crates/earshot) listening to what claudius renders. One CLAP embedding of the
//! master (earshot's windowing: at most six 10 s windows) answers every question the critic asks: does it read as
//! the genre rather than its named confusions, does it read as professionally produced rather than as one of the
//! usual flaws, what does it sound like, how typical is it of reference tracks. AudioSet tags of a few excerpts are
//! reported as held-out evidence and never optimised (earshot's own advice against Goodhart).
//!
//! Built without the `ears` feature (for platforms ONNX Runtime has no build for, such as Intel Macs), `Ears::load`
//! says so and the critic judges with meters and theory only.

pub const PRO: &[&str] = &["a professionally mixed and mastered music track", "a high quality studio recording"];
pub const FLAWS: &[&str] = &[
    "a cheap synthesized beep",
    "a low quality recording with hiss",
    "a distorted clipped recording",
    "a computer-generated electronic tone",
    "a low bitrate mp3 with artifacts",
    "a recording made on a phone",
];

#[cfg(feature = "ears")]
pub use listening::Ears;
#[cfg(not(feature = "ears"))]
pub use unavailable::Ears;

#[cfg(feature = "ears")]
mod listening {
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use super::{FLAWS, PRO};
    use crate::audio::{self, Stereo};
    use crate::genres::GenreProfile;
    use crate::judge::EarsReport;

    pub struct Ears {
        pub inner: earshot::Ears,
        with_ast: bool,
    }

    fn best(scores: &[earshot::Score]) -> (String, f64) {
        scores
            .iter()
            .map(|s| (s.query.clone(), s.z))
            .fold((String::new(), f64::MIN), |a, b| if b.1 > a.1 { b } else { a })
    }

    impl Ears {
        /// load CLAP (and the AudioSet tagger when `ast`); the first run downloads the ONNX models (~1.1 GB)
        pub fn load(ast: bool) -> Result<Ears> {
            let inner = earshot::Ears::load(earshot::EarsOptions { load_ast: ast, ..Default::default() })?;
            Ok(Ears { inner, with_ast: ast })
        }

        pub fn mono(x: &Stereo) -> Vec<f32> {
            earshot::audio::downmix(&x.l, &x.r)
        }

        /// embeddings of reference tracks (files or folders), for typicality
        pub fn embed_refs(&self, paths: &[PathBuf]) -> Result<Vec<Vec<f32>>> {
            let files = audio::list_audio(&paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>());
            files.iter().map(|f| self.inner.embed_file(Path::new(f))).collect()
        }

        /// what the ears hear in `x`, against the genre's query and confusions; without a genre, the best-fitting
        /// music genre of earshot's vocabulary is reported against the runner-up
        pub fn report(
            &self,
            x: &Stereo,
            genre: Option<&GenreProfile>,
            refs: Option<&[Vec<f32>]>,
            realness: bool,
        ) -> Result<EarsReport> {
            let mono = Self::mono(x);
            let emb = self.inner.embed(&mono)?;
            let (genre_query, genre_z, best_alt, best_alt_z) = match genre {
                Some(g) => {
                    let mut qs: Vec<&str> = vec![g.query.as_str()];
                    qs.extend(g.alts.iter().map(|s| s.as_str()));
                    let gs = self.inner.scores(&emb, &qs)?;
                    let (a, az) = best(&gs[1..]);
                    (g.query.clone(), gs[0].z, a, az)
                }
                None => {
                    let terms = self.inner.top_terms(
                        &emb,
                        2,
                        &[
                            "transitions",
                            "impacts",
                            "ui",
                            "keyboard",
                            "foley",
                            "water",
                            "nature",
                            "chimes",
                            "ambience",
                            "everyday",
                            "music_mood",
                            "instruments",
                            "voice",
                            "quality",
                            "production",
                        ],
                    );
                    let top = terms.first().map(|t| (t.term.clone(), t.z)).unwrap_or_default();
                    let second = terms.get(1).map(|t| (t.term.clone(), t.z)).unwrap_or_default();
                    (top.0, top.1, second.0, second.1)
                }
            };
            let (_, production_z) = best(&self.inner.scores(&emb, PRO)?);
            let flaws: Vec<&str> = FLAWS
                .iter()
                .copied()
                .filter(|f| !genre.is_some_and(|g| g.accepted_flaws.iter().any(|a| a == f)))
                .collect();
            let (worst_flaw, worst_flaw_z) = best(&self.inner.scores(&emb, &flaws)?);
            let sounds_like =
                self.inner.top_terms(&emb, 4, &["quality", "production"]).into_iter().map(|t| t.term).collect();
            let p_real = if realness { self.p_real(&mono, &emb).map(|p| (p * 1000.0).round() / 1000.0) } else { None };
            let audioset = if self.with_ast { self.held_out_tags(&mono).unwrap_or_default() } else { Vec::new() };
            Ok(EarsReport {
                genre_query,
                genre_z,
                best_alt: if best_alt.is_empty() { "-".into() } else { best_alt },
                best_alt_z: if best_alt_z == f64::MIN { 0.0 } else { best_alt_z },
                production_z,
                worst_flaw,
                worst_flaw_z,
                sounds_like,
                p_real,
                audioset,
                ref_typicality_z: refs.filter(|r| r.len() >= 2).map(|r| earshot::typicality(&emb, r, 5).z),
            })
        }

        /// earshot's p_real: the mean over 4 s windows, each one CLAP pass and a set of signal measurements. A song
        /// has sixty of them, which took most of the critic's time for a reading the score does not use, so the mean
        /// is taken over eight windows spread across the song.
        fn p_real(&self, mono: &[f32], emb: &[f32]) -> Option<f64> {
            const WINDOWS: usize = 8;
            let sr = earshot::SR as usize;
            let win = 4 * sr;
            if mono.len() <= 6 * sr {
                return self.inner.realness(mono, Some(emb)).ok()?.p_real;
            }
            let spread = mono.len() - win;
            let ps: Vec<f64> = (0..WINDOWS)
                .filter_map(|i| {
                    let start = spread * i / (WINDOWS - 1);
                    self.inner.realness(&mono[start..start + win], None).ok()?.p_real
                })
                .collect();
            (!ps.is_empty()).then(|| ps.iter().sum::<f64>() / ps.len() as f64)
        }

        /// AudioSet top labels over three 10 s excerpts (start, middle, three quarters): held-out evidence only
        fn held_out_tags(&self, mono: &[f32]) -> Result<Vec<(String, f32)>> {
            let w = (10.24 * earshot::SR as f64) as usize;
            let labels = self.inner.audioset_labels();
            let mut probs = vec![0.0f32; labels.len()];
            let n = mono.len();
            for f in [0.1, 0.5, 0.75] {
                let a = ((n as f64 * f) as usize).min(n.saturating_sub(w));
                let p = self.inner.audioset(&mono[a..(a + w).min(n)])?;
                for (acc, v) in probs.iter_mut().zip(p) {
                    *acc += v / 3.0;
                }
            }
            let mut idx: Vec<usize> = (0..probs.len()).collect();
            idx.sort_by(|a, b| probs[*b].partial_cmp(&probs[*a]).unwrap());
            Ok(idx.into_iter().take(6).map(|i| (labels[i].clone(), (probs[i] * 1000.0).round() / 1000.0)).collect())
        }
    }
}

#[cfg(not(feature = "ears"))]
mod unavailable {
    use std::path::PathBuf;

    use anyhow::{Result, bail};

    use crate::audio::Stereo;
    use crate::genres::GenreProfile;
    use crate::judge::EarsReport;

    enum Never {}

    /// a build without earshot: `load` explains, and nothing else can be reached
    pub struct Ears {
        never: Never,
    }

    impl Ears {
        pub fn load(_ast: bool) -> Result<Ears> {
            bail!(
                "this claudius was built without the ears (earshot needs ONNX Runtime, which has no build for this platform)"
            )
        }

        pub fn mono(x: &Stereo) -> Vec<f32> {
            x.l.iter().zip(&x.r).map(|(l, r)| (l + r) * std::f32::consts::FRAC_1_SQRT_2).collect()
        }

        pub fn embed_refs(&self, _paths: &[PathBuf]) -> Result<Vec<Vec<f32>>> {
            match self.never {}
        }

        pub fn report(
            &self,
            _x: &Stereo,
            _genre: Option<&GenreProfile>,
            _refs: Option<&[Vec<f32>]>,
            _realness: bool,
        ) -> Result<EarsReport> {
            match self.never {}
        }
    }
}
