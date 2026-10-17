//! The installed-pack index (`<packs>/<pack>/index.json`): metadata, the files, and every instrument's zones.

use serde::{Deserialize, Serialize};

pub const INDEX_FORMAT: u32 = 1;

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    (*v - 1.0).abs() < 1e-9
}

fn vel_max() -> u8 {
    127
}

fn is_vel_max(v: &u8) -> bool {
    *v == 127
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackIndex {
    pub format: u32,
    pub name: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub licence: String,
    pub licence_url: String,
    pub attribution: String,
    pub attribution_required: bool,
    #[serde(default)]
    pub licence_note: String,
    pub source: String,
    pub version: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub opt_in: bool,
    #[serde(default)]
    pub warning: Option<String>,
    /// added by the user (`add_sample_map`) rather than from the curated registry
    #[serde(default)]
    pub user: bool,
    /// samples are downloaded the first time they are played
    #[serde(default)]
    pub lazy: bool,
    #[serde(default)]
    pub installed_at: u64,
    /// bank names that select this pack (`tr909`, `909`, `RolandTR909` ...)
    #[serde(default)]
    pub aliases: Vec<String>,
    pub files: Vec<FileEntry>,
    pub instruments: Vec<InstrumentDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    /// path relative to the pack directory
    pub path: String,
    /// where it comes from (empty for files extracted from an archive)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// expected size in bytes of the download (0 = unknown)
    #[serde(default, skip_serializing_if = "is_default")]
    pub size: u64,
    /// convert to FLAC after download (lossless; WAV only)
    #[serde(default, skip_serializing_if = "is_default")]
    pub flac: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Pitched,
    Drum,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoopMode {
    #[default]
    NoLoop,
    OneShot,
    /// loop for as long as the voice lasts
    Continuous,
    /// loop while the note is held, then play on to the end
    Sustain,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Unison {
    pub voices: u32,
    pub detune_cents: f32,
    /// stereo spread 0..1
    pub spread: f32,
    /// maximum onset offset between voices in milliseconds
    pub delay_ms: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstrumentDef {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// play the whole sample regardless of the gate (drums, plucked / struck instruments left to ring)
    #[serde(default)]
    pub one_shot: bool,
    /// release time in seconds after the gate closes
    pub release: f32,
    #[serde(default)]
    pub attack: f32,
    #[serde(default)]
    pub gain_db: f32,
    /// amplitude velocity tracking 0..1 (1 = (v/127)^2, 0 = velocity only picks layers)
    #[serde(default = "one")]
    pub veltrack: f32,
    /// explicit velocity curve points (velocity, gain); overrides `veltrack`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vel_curve: Vec<(u8, f32)>,
    /// a new note cuts the previous one (classic drum machines, hi-hats)
    #[serde(default)]
    pub mono: bool,
    /// a repeated key fades the previous voice on that key (pianos)
    #[serde(default)]
    pub retrigger: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unison: Option<Unison>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Variant {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub zones: Vec<Zone>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Zone {
    /// index into `PackIndex::files`
    pub file: u32,
    /// MIDI pitch of the recording (fractional when the region is detuned)
    pub root: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub lokey: u8,
    #[serde(default = "vel_max", skip_serializing_if = "is_vel_max")]
    pub hikey: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub lovel: u8,
    #[serde(default = "vel_max", skip_serializing_if = "is_vel_max")]
    pub hivel: u8,
    /// velocity crossfade in (gain rises from lo to hi)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xfin: Option<(u8, u8)>,
    /// velocity crossfade out (gain falls from lo to hi)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xfout: Option<(u8, u8)>,
    /// round robin: sequence length and this zone's 1-based position
    #[serde(default, skip_serializing_if = "is_default")]
    pub seq_len: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub seq_pos: u8,
    /// random alternatives: plays when lorand <= r < hirand
    #[serde(default, skip_serializing_if = "is_default")]
    pub lorand: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub hirand: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub gain_db: f32,
    /// -100 (left) .. 100 (right), balance for stereo samples
    #[serde(default, skip_serializing_if = "is_default")]
    pub pan: f32,
    /// start offset in source samples
    #[serde(default, skip_serializing_if = "is_default")]
    pub offset: u32,
    /// last source sample to play (0 = end of file)
    #[serde(default, skip_serializing_if = "is_default")]
    pub end: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub loop_mode: LoopMode,
    #[serde(default, skip_serializing_if = "is_default")]
    pub loop_start: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub loop_end: u32,
    /// pitch tracking: 1 = follows the key, 0 = always at root pitch
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub keytrack: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack: Option<f32>,
    /// SFZ amplitude envelope after the attack: hold and decay seconds, sustain level 0..1
    #[serde(default, skip_serializing_if = "is_default")]
    pub hold: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub decay: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub sustain: f32,
}

impl Default for Zone {
    fn default() -> Self {
        Zone {
            file: 0,
            root: 60.0,
            lokey: 0,
            hikey: 127,
            lovel: 0,
            hivel: 127,
            xfin: None,
            xfout: None,
            seq_len: 0,
            seq_pos: 0,
            lorand: 0.0,
            hirand: 1.0,
            gain_db: 0.0,
            pan: 0.0,
            offset: 0,
            end: 0,
            loop_mode: LoopMode::NoLoop,
            loop_start: 0,
            loop_end: 0,
            keytrack: 1.0,
            release: None,
            attack: None,
            hold: 0.0,
            decay: 0.0,
            sustain: 1.0,
        }
    }
}

impl Zone {
    pub fn plays_key(&self, k: u8) -> bool {
        k >= self.lokey && k <= self.hikey
    }

    pub fn plays_vel(&self, v: u8) -> bool {
        v >= self.lovel && v <= self.hivel && self.xf_gain(v) > 0.0
    }

    /// velocity crossfade gain (SFZ "power" curve, so overlapping layers keep constant power)
    pub fn xf_gain(&self, v: u8) -> f32 {
        let v = v as f32;
        let mut g = 1.0f32;
        if let Some((lo, hi)) = self.xfin {
            let (lo, hi) = (lo as f32, hi as f32);
            if v < lo || (v <= lo && hi > lo) {
                g = 0.0;
            } else if v < hi {
                g *= ((v - lo) / (hi - lo)).sqrt();
            }
        }
        if let Some((lo, hi)) = self.xfout {
            let (lo, hi) = (lo as f32, hi as f32);
            if v > hi || (v >= hi && hi > lo) {
                g = 0.0;
            } else if v > lo {
                g *= ((hi - v) / (hi - lo)).sqrt();
            }
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfades_keep_power() {
        let soft = Zone { xfout: Some((0, 83)), ..Default::default() };
        let mid = Zone { xfin: Some((0, 83)), xfout: Some((84, 127)), ..Default::default() };
        let loud = Zone { xfin: Some((84, 127)), ..Default::default() };
        for v in 1..=127u8 {
            let p: f32 = [&soft, &mid, &loud].iter().map(|z| z.xf_gain(v).powi(2)).sum();
            assert!((p - 1.0).abs() < 0.05 || v == 83 || v == 84, "v={v} p={p}");
        }
        assert_eq!(loud.xf_gain(127), 1.0);
        assert_eq!(mid.xf_gain(127), 0.0);
        assert_eq!(soft.xf_gain(0), 1.0);
        let step = Zone { xfin: Some((64, 64)), ..Default::default() };
        assert_eq!(step.xf_gain(63), 0.0);
        assert_eq!(step.xf_gain(64), 1.0);
    }
}
