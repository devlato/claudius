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
    pub source: Option<f64>,
    pub take: Option<String>,
}

/// What the studio's Duplicate sends for section `i`, `s`: a section of its kind and length right after it, then all it
/// is set to (its progression and its build-up too) and the stretch of each take it plays, so that a chorus played twice
/// plays its own recording twice; as one change, which one undo takes back whole
pub fn duplicate(s: &claudius_api::SectionView, i: usize) -> Command {
    let mut commands = vec![
        Command::AddSection { after: i, kind: s.kind.clone(), bars: s.bars, like: Some(s.name.clone()) },
        SectionEdit {
            energy: Some(s.energy),
            layers: Some(s.layers.clone()),
            prog: Some(s.prog.clone()),
            variant: Some(s.variant.clone()),
            filter_in: Some(s.filter_in),
            build: Some(s.build),
            // a style named turns a build-up on: the copy's is named only when it has one
            build_style: s.build.then(|| s.build_style.clone()),
            build_amount: Some(s.build_amount),
            gap: Some(s.gap),
            impact: Some(s.impact),
            fills: Some(s.fills),
            ..Default::default()
        }
        .at(i + 1),
    ];
    commands.extend(
        s.takes.iter().map(|(take, at)| {
            SectionEdit { source: Some(*at), take: Some(take.clone()), ..Default::default() }.at(i + 1)
        }),
    );
    Command::Together { said: format!("Duplicate {}", s.name), commands }
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
            source,
            take,
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
            source,
            take,
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

#[cfg(test)]
mod tests {
    use claudius_api::{SectionView, SongView, TrackView};

    #[test]
    fn a_duplicate_is_the_section_again_under_its_own_name_playing_the_same_stretch_of_each_take() {
        let s = SectionView {
            name: "chorus".into(),
            kind: "hook".into(),
            bars: 8,
            energy: 0.8,
            layers: vec!["kick".into(), "vox".into()],
            prog: "B".into(),
            variant: "fill".into(),
            build: true,
            build_style: "roll".into(),
            build_amount: 0.7,
            fills: true,
            takes: [("vox".to_string(), 32.0)].into(),
            ..Default::default()
        };
        let vox = TrackView { name: "vox".into(), kind: "take".into(), ..Default::default() };
        let mut song = SongView { sections: vec![s.clone()], tracks: vec![vox], ..Default::default() };
        let together = super::duplicate(&s, 0);
        assert!(
            matches!(&together, claudius_api::Command::Together { said, .. } if said == "Duplicate chorus"),
            "one change"
        );
        crate::edits::apply(&mut song, &together).unwrap().unwrap();
        let copy = song.sections[1].clone();
        assert_eq!(copy.name, "hook");
        assert_eq!(SectionView { name: s.name.clone(), start_bar: 0, ..copy }, s);
    }
}
