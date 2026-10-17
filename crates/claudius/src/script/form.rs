//! The form: `sec`, `form`, `section`, `layer` and the section methods, and the checks that a section only names
//! tracks and progressions the song has.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, INT, Map};

use crate::model::*;

use super::song::{check_keys, genre_of, map_f32, map_str, with_bp};
use super::{Ctx, R, constant, fail, num, text};

fn layers_from(d: &Dynamic, all: &[String]) -> R<Vec<String>> {
    let names: Vec<String> = if let Some(s) = text(d) {
        s.split_whitespace().map(String::from).collect()
    } else if let Ok(a) = d.clone().into_typed_array::<String>() {
        a
    } else if let Some(a) = d.clone().try_cast::<Array>() {
        a.into_iter().filter_map(|x| text(&x)).collect()
    } else {
        return fail("layers are a string (\"kick hats\") or an array of names");
    };
    if names.iter().any(|n| n == "*") {
        return Ok(all.to_vec());
    }
    Ok(names)
}

fn section_kind(k: &str) -> R<SectionKind> {
    Ok(match k {
        "intro" => SectionKind::Intro,
        "verse" => SectionKind::Verse,
        "build" => SectionKind::Build,
        "drop" => SectionKind::Drop,
        "break" | "breakdown" => SectionKind::Break,
        "hook" | "chorus" => SectionKind::Hook,
        "groove" | "loop" => SectionKind::Groove,
        "outro" => SectionKind::Outro,
        other => {
            return fail(format!("unknown section kind '{other}' (intro verse build drop break hook groove outro)"));
        }
    })
}

pub(super) fn default_energy(k: SectionKind) -> f32 {
    match k {
        SectionKind::Intro | SectionKind::Outro => 0.4,
        SectionKind::Break => 0.35,
        SectionKind::Build => 0.6,
        SectionKind::Verse | SectionKind::Groove => 0.65,
        SectionKind::Drop | SectionKind::Hook => 0.95,
    }
}

/// a section being built by sec(...).build().impact() ...
#[derive(Clone, Debug)]
pub struct Sec {
    spec: SectionSpec,
    all_layers: bool,
}

/// the form: `sec`, `form`, `section`, `layer` and the section methods
pub(super) fn register_form(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    engine.register_type_with_name::<Sec>("Section");
    let c = ctx.clone();
    let mk = move |kind: &str, bars: INT, layers: Option<Dynamic>| -> R<Sec> {
        let k = section_kind(kind)?;
        let all = c.borrow().bp.as_ref().map(|b| b.layer_names()).unwrap_or_default();
        let (layers, all_layers) = match layers {
            Some(d) => {
                let l = layers_from(&d, &all)?;
                let star = text(&d).is_some_and(|s| s.trim() == "*");
                (l, star)
            }
            None => (all.clone(), true),
        };
        Ok(Sec {
            spec: SectionSpec {
                name: kind.to_string(),
                kind: k,
                bars: bars.clamp(1, 256) as u32,
                energy: default_energy(k),
                prog: if matches!(k, SectionKind::Break) { "B".into() } else { "A".into() },
                layers,
                variant: if matches!(k, SectionKind::Intro | SectionKind::Outro) {
                    "sparse".into()
                } else {
                    "main".into()
                },
                filter_in: false,
                build: k == SectionKind::Build,
                gap: false,
                impact: false,
                fills: matches!(k, SectionKind::Drop | SectionKind::Groove | SectionKind::Hook),
            },
            all_layers,
        })
    };
    let mk = Rc::new(mk);
    let m1 = mk.clone();
    engine.register_fn("sec", move |kind: &str, bars: INT| m1(kind, bars, None));
    let m1 = mk.clone();
    engine.register_fn("sec", move |kind: &str, bars: INT, layers: Dynamic| m1(kind, bars, Some(layers)));
    macro_rules! flag {
        ($name:literal, $field:ident) => {
            engine.register_fn($name, |s: &mut Sec| {
                let mut t = s.clone();
                t.spec.$field = true;
                t
            });
            engine.register_fn($name, |s: &mut Sec, on: bool| {
                let mut t = s.clone();
                t.spec.$field = on;
                t
            });
        };
    }
    flag!("build", build);
    flag!("gap", gap);
    flag!("impact", impact);
    flag!("fills", fills);
    flag!("filter_in", filter_in);
    engine.register_fn("energy", |s: &mut Sec, x: Dynamic| -> R<Sec> {
        let mut t = s.clone();
        t.spec.energy = constant(&x, "energy")?.clamp(0.0, 1.0) as f32;
        Ok(t)
    });
    engine.register_fn("name", |s: &mut Sec, n: &str| {
        let mut t = s.clone();
        t.spec.name = n.to_string();
        t
    });
    engine.register_fn("prog", |s: &mut Sec, n: &str| {
        let mut t = s.clone();
        t.spec.prog = n.to_string();
        t
    });
    engine.register_fn("variant", |s: &mut Sec, n: &str| {
        let mut t = s.clone();
        t.spec.variant = n.to_string();
        t
    });
    let c = ctx.clone();
    engine.register_fn("form", move |items: Array| -> R<()> {
        let secs: Vec<Sec> = items
            .into_iter()
            .map(|d| d.try_cast::<Sec>().ok_or("form() takes sec(...) items"))
            .collect::<Result<_, _>>()?;
        with_bp(&c, |bp, _| {
            let all = bp.layer_names();
            let mut used: BTreeMap<String, usize> = BTreeMap::new();
            bp.form = secs
                .into_iter()
                .map(|s| {
                    let mut spec = s.spec;
                    if s.all_layers {
                        spec.layers = all.clone();
                    }
                    let n = used.entry(spec.name.clone()).or_insert(0);
                    *n += 1;
                    if *n > 1 {
                        spec.name = format!("{}{}", spec.name, n);
                    }
                    spec
                })
                .collect();
            let optional = optional_layers(bp);
            for s in &bp.form {
                check_section(bp, s, &optional)?;
            }
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("section", move |which: Dynamic, m: Map| -> R<()> {
        check_keys(
            &m,
            "section()",
            &["bars", "energy", "prog", "variant", "name", "layers", "build", "gap", "impact", "fills", "filter_in"],
        )?;
        with_bp(&c, |bp, _| {
            let all = bp.layer_names();
            let optional = optional_layers(bp);
            let idx = match (which.as_int().ok(), text(&which)) {
                (Some(i), _) => i as usize,
                (None, Some(n)) => {
                    bp.form.iter().position(|s| s.name == n).ok_or_else(|| format!("no section '{n}'"))?
                }
                _ => return fail("section() takes a name or an index"),
            };
            let s = bp.form.get_mut(idx).ok_or_else(|| format!("no section {idx}"))?;
            if let Some(v) = m.get("bars").and_then(num) {
                s.bars = (v as u32).clamp(1, 256);
            }
            if let Some(v) = map_f32(&m, "energy") {
                s.energy = v.clamp(0.0, 1.0);
            }
            if let Some(v) = map_str(&m, "prog") {
                s.prog = v;
            }
            if let Some(v) = map_str(&m, "variant") {
                s.variant = v;
            }
            if let Some(v) = map_str(&m, "name") {
                s.name = v;
            }
            if let Some(d) = m.get("layers") {
                s.layers = layers_from(d, &all)?;
            }
            for (k, field) in [
                ("build", &mut s.build),
                ("gap", &mut s.gap),
                ("impact", &mut s.impact),
                ("fills", &mut s.fills),
                ("filter_in", &mut s.filter_in),
            ] {
                if let Some(v) = m.get(k).and_then(|d| d.as_bool().ok()) {
                    *field = v;
                }
            }
            let s = s.clone();
            check_section(bp, &s, &optional)
        })
    });
    let c = ctx.clone();
    engine.register_fn("layer", move |section: Dynamic, layer: &str, on: bool| -> R<()> {
        with_bp(&c, |bp, _| {
            let idx = match (section.as_int().ok(), text(&section)) {
                (Some(i), _) => i as usize,
                (None, Some(n)) => {
                    bp.form.iter().position(|s| s.name == n).ok_or_else(|| format!("no section '{n}'"))?
                }
                _ => return fail("layer() takes a section name or index"),
            };
            let all = bp.layer_names();
            if !all.iter().any(|l| l == layer) && !optional_layers(bp).iter().any(|l| l == layer) {
                return fail(format!("no track '{layer}' (tracks: {})", all.join(" ")));
            }
            let s = bp.form.get_mut(idx).ok_or_else(|| format!("no section {idx}"))?;
            s.layers.retain(|l| l != layer);
            if on {
                s.layers.push(layer.to_string());
            }
            Ok(())
        })
    });
}

/// the genre's optional loops and parts: a form may list them before they have a sound (no library), and then they
/// stay silent
fn optional_layers(bp: &Blueprint) -> Vec<String> {
    genre_of(bp)
        .map(|g| g.loops.iter().map(|l| l.name.clone()).chain(g.parts.iter().map(|p| p.name.clone())).collect())
        .unwrap_or_default()
}

/// a section only names tracks the song has and a progression it has
fn check_section(bp: &Blueprint, s: &SectionSpec, optional: &[String]) -> R<()> {
    let all = bp.layer_names();
    if let Some(bad) = s.layers.iter().find(|l| !all.contains(l) && !optional.contains(l)) {
        return fail(format!("section '{}' uses '{bad}', which is not a track (tracks: {})", s.name, all.join(" ")));
    }
    let has = |name: &str| bp.harmony.progressions.contains_key(name);
    // a break plays B, or A when the song has no B
    if !(has(&s.prog) || (s.prog == "B" && has("A"))) {
        let names: Vec<&str> = bp.harmony.progressions.keys().map(String::as_str).collect();
        return fail(format!(
            "section '{}' plays progression '{}', which the song does not have (progressions: {}; prog() adds one)",
            s.name,
            s.prog,
            names.join(" ")
        ));
    }
    Ok(())
}
