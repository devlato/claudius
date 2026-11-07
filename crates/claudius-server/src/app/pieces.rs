//! The auto-sampler's pieces (`sample_pieces`): phrases of a recording offered before any goes into the song, each as
//! it was cut and as treated, to hear side by side (`/api/pieces/<file>`), drawn again for another take
//! (`redraw_pieces`), and kept as chosen (`keep_pieces`). Only the last offer is held.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use claudius::audio::{self, Stereo};
use claudius::model::{Blueprint, Sound};
use claudius::{config, sampler, take};
use claudius_api::{KeptPiece, PieceView, PiecesView, Reply};

use super::{App, no, ok};

/// what the auto-sampler offered last: the recording, what it credits, how it was asked, and the pieces
#[derive(Clone)]
pub(super) struct Offer {
    name: String,
    src: Arc<Stereo>,
    sources: take::Sources,
    opt: sampler::Options,
    seed: u64,
    pieces: Vec<sampler::Piece>,
}

/// the files of the phrases sampled into `bp`
pub(super) fn sampled(bp: Option<&Blueprint>) -> BTreeSet<String> {
    let loops = bp.map(|bp| bp.loops.iter().filter(|l| l.sampled)).into_iter().flatten();
    loops
        .filter_map(|l| match &l.sound {
            Sound::Loop { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect()
}

impl App {
    /// where the pieces offered are kept to hear
    fn pieces_dir(&self) -> PathBuf {
        self.cache.join("pieces")
    }

    /// a piece's file to hear, by its name (`3-1-raw.wav`): none for any other name
    pub fn pieces_file(&self, name: &str) -> Option<PathBuf> {
        let plain = name.strip_suffix(".wav")?.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        let path = self.pieces_dir().join(name);
        (plain && path.is_file()).then_some(path)
    }

    /// The pieces of recording `x`, called `name`, the auto-sampler offers for take `seed`, held until another offer
    /// replaces them; `sources`: what the recording credits, which the pieces kept credit too.
    pub(super) fn offer_pieces(
        self: &Arc<Self>,
        x: Arc<Stereo>,
        name: &str,
        sources: take::Sources,
        opt: sampler::Options,
        seed: u64,
    ) -> Reply {
        let v = self.version();
        if self.blueprint().is_none() {
            return no("no song yet: start one with new_song, then sample a recording into it", v);
        }
        let ears = self.ears.get().and_then(|e| e.as_ref());
        let pieces = match sampler::pieces(&x, ears, &opt, seed) {
            Ok(p) => p,
            Err(e) => return no(e, v),
        };
        // the last offer's files go with it
        let dir = self.pieces_dir();
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return no(format!("cannot keep the pieces to hear in {}: {e}", dir.display()), v);
        }
        let mut views = Vec::new();
        for (i, p) in pieces.iter().enumerate() {
            let id = i as u32 + 1;
            let file = |kind: &str| format!("{v}-{id}-{kind}.wav");
            for (kind, sound) in [("raw", &p.raw), ("treated", &p.treated)] {
                if let Err(e) = audio::save_wav(&dir.join(file(kind)), sound, false) {
                    return no(format!("cannot keep the pieces to hear: {e:#}"), v);
                }
            }
            views.push(PieceView {
                id,
                place: p.place(),
                bars: p.phrase.bars,
                sung: p.phrase.sung,
                raw: format!("/api/pieces/{}", file("raw")),
                treated: format!("/api/pieces/{}", file("treated")),
                treatment: p.treatment.clone(),
            });
        }
        let lines: Vec<String> = views
            .iter()
            .map(|p| format!("{}: {}, {:.0}% sung; treated, {}", p.id, p.place, p.sung * 100.0, p.treatment))
            .collect();
        let n = views.len();
        let said = format!("{n} piece{} of {name}, take {seed}:\n{}", if n == 1 { "" } else { "s" }, lines.join("\n"));
        let view = PiecesView { recording: name.to_string(), seed, pieces: views };
        let offer = Offer { name: name.to_string(), src: x, sources, opt, seed, pieces };
        let v = self.update(|s| {
            s.pieces = Some((offer, view));
            s.say(format!("offered {n} pieces of {name} (take {seed})"));
            s.version + 1
        });
        ok(said, v)
    }

    /// the last recording offered, drawn again for take `seed` (the take after the last one when none)
    pub(super) fn redraw_pieces(self: &Arc<Self>, seed: Option<u64>) -> Reply {
        let held = self.lock().pieces.as_ref().map(|(o, _)| o.clone());
        let Some(o) = held else {
            return no("no pieces to draw again: sample_pieces offers some first", self.version());
        };
        self.offer_pieces(o.src, &o.name, o.sources, o.opt, seed.unwrap_or(o.seed + 1))
    }

    /// the pieces offered that `keep` names, each as it was cut or treated, sampled into the song in `sections` (those
    /// that suit each, when none), crediting what the recording credits; the offer goes once they are in
    pub(super) fn keep_pieces(self: &Arc<Self>, keep: &[KeptPiece], sections: Vec<String>) -> Reply {
        let v = self.version();
        let held = self.lock().pieces.as_ref().map(|(o, _)| o.clone());
        let Some(o) = held else {
            return no("no pieces to keep: sample_pieces offers some first", v);
        };
        let mut chosen = Vec::new();
        for k in keep {
            match o.pieces.get((k.id as usize).wrapping_sub(1)) {
                Some(p) => chosen.push((p, k.treated)),
                None => return no(format!("no piece {} (pieces 1 to {})", k.id, o.pieces.len()), v),
            }
        }
        let dir = take::dir(self.lock().folder.as_deref());
        if !config::write_allowed(&dir, self.roots.as_deref()) {
            return no(format!("{} is outside the folders this claudius may write", dir.display()), v);
        }
        let opt = sampler::Options { sections, ..o.opt.clone() };
        let before = sampled(self.blueprint().as_ref());
        let mut r =
            self.edit_with(|bp| sampler::keep(bp, &o.src, &o.name, &chosen, &opt, &dir).map(|s| s.said(&o.name)));
        if r.ok {
            // the pieces are in the song by now: a credit not kept is said, not taken for a failure
            if !o.sources.is_empty() {
                for file in sampled(self.blueprint().as_ref()).difference(&before) {
                    if let Err(e) = take::keep_sources(Path::new(file), &o.sources) {
                        r.message.push_str(&format!("\ncannot keep what {file} credits: {e:#}"));
                    }
                }
            }
            let v = self.update(|s| {
                s.pieces = None;
                s.version + 1
            });
            return Reply { version: v, ..r };
        }
        r
    }
}
