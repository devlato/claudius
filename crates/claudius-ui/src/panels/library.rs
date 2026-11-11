//! The sound library: the packs claudius knows (installed or not, with their licences and sizes) and every sound a
//! part or a script may use: sampled instruments by pack, their short names, synth presets and drum voices. Click
//! an instrument or a preset to put it on one of the song's parts.

use claudius_api::{Command, PackInfo, SongView, SoundList, words};
use eframe::egui::{self, Color32, RichText, Ui};

use super::{Cx, empty, layer_color, page_frame, small};
use crate::cmds::PartEdit;
use crate::theme::{self, LINE_SOFT, RAISED, TEXT, TEXT_DIM, TEXT_FAINT};
use crate::widgets;

/// green for licences that ask nothing, amber for attribution, grey for the rest
fn licence_color(licence: &str) -> Color32 {
    let l = licence.to_lowercase();
    if l.contains("cc0") || l.contains("public domain") || l.contains("unlicense") {
        theme::GOOD
    } else if l.contains("cc-by") {
        theme::WARN
    } else {
        TEXT_DIM
    }
    .into()
}

/// a pack's card; true when its install button was clicked (only packs that are not installed, nor opt-in, have one)
fn pack_card(ui: &mut Ui, p: &PackInfo, instruments: usize, can_install: bool) -> bool {
    let mut install = false;
    let frame = egui::Frame::new()
        .fill(if p.installed { RAISED.into() } else { theme::mix(RAISED, theme::BG, 0.5) })
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .stroke(egui::Stroke::new(1.0, LINE_SOFT));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::row(ui, ("pack", &p.name), |ui, side| {
            if side == widgets::Side::Right {
                if instruments > 0 {
                    ui.label(small(format!("{instruments} sounds ·")));
                }
                ui.label(small(format!("{:.0} MB", p.size_mb)));
                if !p.installed && !p.opt_in {
                    install = ui.add_enabled(can_install, egui::Button::new("install").small()).clicked();
                }
                return;
            }
            ui.label(RichText::new(&p.name).strong().size(13.0).color(if p.installed { TEXT } else { TEXT_DIM }));
            if p.installed {
                widgets::pill(ui, "installed", theme::GOOD);
            } else {
                widgets::pill(ui, "not installed", TEXT_FAINT);
            }
        });
        ui.add(egui::Label::new(RichText::new(&p.description).size(11.5).color(TEXT_DIM)).wrap());
        ui.horizontal_wrapped(|ui| {
            ui.label(small("licence"));
            widgets::pill(ui, &p.license, licence_color(&p.license));
        });
    });
    ui.add_space(4.0);
    install
}

/// a sound chip, `label` the sound's name as it is shown; clicking it offers the song's parts to put it on
fn assignable(
    ui: &mut Ui,
    label: &str,
    sound: &str,
    list: &SoundList,
    song: Option<&SongView>,
    out: &mut Vec<claudius_api::Command>,
) {
    // a part may name a pack's instrument by its short name
    let short = sound.split_once(':').map_or(sound, |(_, n)| n);
    let current: Vec<&str> = song
        .map(|s| s.parts.iter().filter(|p| p.preset == sound || p.preset == short).map(|p| p.name.as_str()).collect())
        .unwrap_or_default();
    let resp = if current.is_empty() {
        ui.small_button(label)
    } else {
        ui.add(egui::Button::new(RichText::new(label).color(Color32::BLACK)).small().fill(theme::ACCENT))
    };
    let about = super::named(super::sound_about(sound, Some(list)).as_deref(), sound);
    let resp = resp.on_hover_text(if current.is_empty() {
        format!("{about}\nclick to use it on a part")
    } else {
        format!("{about}\nplays {}", current.join(", "))
    });
    egui::Popup::menu(&resp).show(|ui| {
        ui.label(RichText::new(label).strong());
        let Some(song) = song.filter(|s| !s.parts.is_empty()) else {
            ui.label(small("the song has no parts"));
            return;
        };
        ui.label(small("use it as the sound of"));
        for p in &song.parts {
            let on = p.preset == sound;
            let now = words::name_in_words(&p.preset);
            let text = RichText::new(format!("{} · {} (now {now})", p.name, p.role)).color(if on {
                theme::ACCENT.into()
            } else {
                layer_color(song, &p.name)
            });
            if ui.add_enabled(!on, egui::Button::new(text)).clicked() {
                out.push(PartEdit { preset: Some(sound.to_string()), ..Default::default() }.on(&p.name));
                ui.close();
            }
        }
    });
}

fn instruments_by_pack(list: &SoundList) -> Vec<(String, Vec<String>)> {
    let mut packs: Vec<(String, Vec<String>)> = Vec::new();
    for full in list.instruments.iter().filter(|n| n.contains(':')) {
        let pack = full.split(':').next().unwrap_or("").to_string();
        match packs.iter_mut().find(|(p, _)| *p == pack) {
            Some((_, v)) => v.push(full.clone()),
            None => packs.push((pack, vec![full.clone()])),
        }
    }
    packs
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let Some(list) = cx.sounds else {
        empty(ui, "The sound library appears here once claudius has listed it.");
        return;
    };
    super::missing_packs(ui, cx);
    let downloading = cx.state.busy.as_deref().is_some_and(|b| b.starts_with("downloading"));
    let song = cx.state.song.as_ref();
    let by_pack = instruments_by_pack(list);
    let mut out = Vec::new();
    page_frame().show(ui, |ui| {
        widgets::row(ui, "sounds_head", |ui, side| {
            if side == widgets::Side::Right {
                let installed = list.packs.iter().filter(|p| p.installed).count();
                ui.label(small(format!(
                    "{} drum voices · {} synth presets · {} sampled instruments · {installed} of {} packs installed",
                    list.drums.len(),
                    list.synths.len(),
                    by_pack.iter().map(|(_, v)| v.len()).sum::<usize>(),
                    list.packs.len()
                )));
                return;
            }
            widgets::heading(ui, "Sounds");
            ui.add(egui::TextEdit::singleline(&mut cx.view.library_filter).hint_text("search").desired_width(220.0));
        });
        ui.add_space(4.0);
        let f = cx.view.library_filter.to_lowercase();
        let hit = |n: &str| f.is_empty() || n.to_lowercase().contains(&f);
        let found = |n: &&String| super::finds_sound(&f, n, Some(list));
        // the packs take at most a share of the dock, so that the sounds keep room beside them in a narrow window
        let wide = ui.available_width();
        egui::Panel::left("packs")
            .resizable(true)
            .default_size((wide * 0.42).clamp(220.0, 380.0))
            .size_range(220.0..=(wide * 0.6).clamp(260.0, 620.0))
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 10, top: 0, bottom: 0 }))
            .show(ui, |ui| {
                widgets::heading(ui, "Packs");
                egui::ScrollArea::vertical().id_salt("packs").auto_shrink([false, false]).show(ui, |ui| {
                    let mut packs: Vec<&PackInfo> =
                        list.packs.iter().filter(|p| hit(&p.name) || hit(&p.description) || hit(&p.license)).collect();
                    packs.sort_by_key(|p| !p.installed);
                    for p in packs {
                        let n = by_pack.iter().find(|(name, _)| *name == p.name).map_or(0, |(_, v)| v.len());
                        if pack_card(ui, p, n, !downloading) {
                            out.push(Command::InstallPacks { names: vec![p.name.clone()] });
                        }
                    }
                    if list.packs.is_empty() {
                        ui.label(small("no sample packs installed"));
                    }
                });
            });
        egui::ScrollArea::vertical().id_salt("sounds").auto_shrink([false, false]).show(ui, |ui| {
            let filtering = !f.is_empty();
            widgets::heading(ui, "Sampled instruments");
            ui.label(small("multi-sampled recordings from the installed packs; click one to put it on a part"));
            for (pack, names) in &by_pack {
                let names: Vec<&String> = names.iter().filter(found).collect();
                if names.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(RichText::new(format!("{pack} · {}", names.len())).size(12.0))
                    .id_salt(("lib_pack", pack))
                    .default_open(names.len() <= 20)
                    .open(filtering.then_some(true))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                            for full in names {
                                let name = full.split_once(':').map_or(full.as_str(), |(_, n)| n);
                                assignable(ui, &words::name_in_words(name), full, list, song, &mut out);
                            }
                        });
                    });
            }
            let aliases: Vec<&String> = list.instruments.iter().filter(|n| !n.contains(':') && found(n)).collect();
            if !aliases.is_empty() {
                egui::CollapsingHeader::new(RichText::new(format!("Short names · {}", aliases.len())).size(12.0))
                    .id_salt("lib_aliases")
                    .default_open(false)
                    .open(filtering.then_some(true))
                    .show(ui, |ui| {
                        ui.label(small("aliases a part or a script may use instead of pack:instrument"));
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                            for n in aliases {
                                assignable(ui, &words::name_in_words(n), n, list, song, &mut out);
                            }
                        });
                    });
            }
            ui.add_space(6.0);
            widgets::heading(ui, "Synth presets");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for n in list.synths.iter().filter(found) {
                    assignable(ui, &words::name_in_words(n), n, list, song, &mut out);
                }
            });
            ui.add_space(6.0);
            widgets::heading(ui, "Drum voices");
            ui.label(small("procedural drums for patterns: s(\"bd*4\"), or a real kit with .bank(\"tr909\")"));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for n in list.drums.iter().filter(found) {
                    let voice = list.sound_about(n).unwrap_or("a drum voice");
                    ui.label(RichText::new(n).monospace().size(11.5).color(theme::role_color(n)))
                        .on_hover_text(format!("{voice}, synthesized\nin a pattern: s(\"{n}\")"));
                }
            });
        });
    });
    cx.out.extend(out);
}
