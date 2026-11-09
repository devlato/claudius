//! Builders for the commands that change a subset of fields (`SectionEdit { bars: Some(8), ..Default::default() }.at(2)`).

use claudius_api::Command;

#[derive(Default)]
pub struct SectionEdit {
    pub name: Option<String>,
    pub bars: Option<u32>,
    pub energy: Option<f32>,
    pub layers: Option<Vec<String>>,
    pub variant: Option<String>,
    pub filter_in: Option<bool>,
    pub build: Option<bool>,
    pub build_style: Option<String>,
    pub build_amount: Option<f32>,
    pub gap: Option<bool>,
    pub impact: Option<bool>,
    pub fills: Option<bool>,
    pub prog: Option<String>,
}

impl SectionEdit {
    pub fn at(self, index: usize) -> Command {
        let SectionEdit {
            name,
            bars,
            energy,
            layers,
            variant,
            filter_in,
            build,
            build_style,
            build_amount,
            gap,
            impact,
            fills,
            prog,
        } = self;
        Command::SetSection {
            index,
            name,
            bars,
            energy,
            layers,
            prog,
            variant,
            filter_in,
            build,
            build_style,
            build_amount,
            gap,
            impact,
            fills,
        }
    }
}

#[derive(Default)]
pub struct MixEdit {
    pub level_db: Option<f32>,
    pub pan: Option<f32>,
    pub reverb: Option<f32>,
    pub delay: Option<f32>,
    pub duck: Option<f32>,
    pub drive_db: Option<f32>,
    pub muted: Option<bool>,
    pub solo: Option<bool>,
    pub bus: Option<String>,
}

impl MixEdit {
    pub fn on(self, track: &str) -> Command {
        let MixEdit { level_db, pan, reverb, delay, duck, drive_db, muted, solo, bus } = self;
        Command::SetMix { track: track.to_string(), level_db, pan, reverb, delay, duck, drive_db, muted, solo, bus }
    }
}

#[derive(Default)]
pub struct PartEdit {
    pub style: Option<String>,
    pub preset: Option<String>,
    pub register: Option<(i32, i32)>,
    pub density: Option<f32>,
    pub reseed: bool,
    pub late_ms: Option<f32>,
}

impl PartEdit {
    pub fn on(self, name: &str) -> Command {
        let PartEdit { style, preset, register, density, reseed, late_ms } = self;
        Command::SetPart { name: name.to_string(), style, preset, register, density, reseed, late_ms }
    }
}
