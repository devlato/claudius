//! Effects in scripts: a track's insert chain (`fx`), a bus's (`bus`), the master's (`master(#{ fx })`), the sends'
//! effects (`send_fx`), and values by section (`sections`). An effect is written as the blueprint keeps it, a map
//! tagged by `fx` (`#{ fx: "reverb", decay: 2.4, mix: 0.3 }`), so a script reaches every effect and every setting the
//! engine has; a setting that can move is a number, a curve (`[[beat, value], ...]`) or `sections(...)`.

use std::cell::RefCell;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, Map};

use crate::fx::{EFFECT_NAMES, Effect};

use super::form::section_kind;
use super::mix::mix_of;
use super::song::{check_keys, with_bp};
use super::{Ctx, R, fail, num, text};

/// what a setting that can move is, for a message about one that is not
const MOVES: &str =
    "a setting that can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })";

pub(super) fn register_effects(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    let c = ctx.clone();
    engine.register_fn("fx", move |track: &str, chain: Array| -> R<()> {
        let chain = chain_of(&chain, &format!("fx(\"{track}\")"))?;
        with_bp(&c, |bp, _| {
            mix_of(bp, track).ok_or_else(|| format!("no track '{track}'"))?.fx = chain;
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("bus", move |name: &str, chain: Array| -> R<()> {
        if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            return fail(format!("a bus is named with letters, digits, - and _, not '{name}'"));
        }
        let chain = chain_of(&chain, &format!("bus(\"{name}\")"))?;
        with_bp(&c, |bp, _| {
            bp.mix.buses.insert(name.to_string(), chain);
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("send_fx", move |m: Map| -> R<()> {
        check_keys(&m, "send_fx()", &["reverb", "delay"])?;
        let get = |k: &str| m.get(k).map(|d| effect_of(d, &format!("send_fx()'s {k}"))).transpose();
        let (reverb, delay) = (get("reverb")?, get("delay")?);
        with_bp(&c, |bp, _| {
            if let Some(r) = reverb {
                bp.mix.reverb = r;
            }
            if let Some(d) = delay {
                bp.mix.delay = d;
            }
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("sections", move |rest: Dynamic, m: Map| -> R<Map> {
        let rest = num(&rest).ok_or("sections(): the value for the rest of the song is a number")?;
        let names = with_bp(&c, |bp, _| Ok(bp.form.iter().map(|s| s.name.clone()).collect::<Vec<_>>()))?;
        let mut sections = Map::new();
        for (k, v) in m {
            // a section of the form by its name, or every section of a kind (by the kind's own name)
            let key = if names.iter().any(|n| *n == k.as_str()) {
                k.to_string()
            } else {
                match section_kind(&k) {
                    Ok(kind) => kind.name().to_string(),
                    Err(_) => {
                        return fail(format!(
                            "sections(): no section or kind '{k}' (sections: {}; kinds: intro verse build drop break \
                             hook groove outro)",
                            names.join(" ")
                        ));
                    }
                }
            };
            let values: Array = match num(&v) {
                Some(x) => vec![Dynamic::from_float(x)],
                None => v.clone().try_cast::<Array>().unwrap_or_default(),
            };
            if values.is_empty() || values.iter().any(|x| num(x).is_none()) {
                return fail(format!("sections(): '{k}' is a number, or the numbers it moves through ([0.1, 0.6])"));
            }
            sections.insert(key.into(), values.into());
        }
        let mut out = Map::new();
        out.insert("rest".into(), Dynamic::from_float(rest));
        out.insert("sections".into(), sections.into());
        Ok(out)
    });
}

/// a chain of effects as a script writes it
pub(super) fn chain_of(chain: &Array, what: &str) -> R<Vec<Effect>> {
    chain.iter().enumerate().map(|(i, d)| effect_of(d, &format!("{what}'s effect {}", i + 1))).collect()
}

/// an effect as a script writes it: a map tagged by `fx`
pub(super) fn effect_of(d: &Dynamic, what: &str) -> R<Effect> {
    let v = json_of(d).map_err(|e| format!("{what}: {e}"))?;
    if !v.get("fx").is_some_and(|f| f.is_string()) {
        return fail(format!(
            "{what} is a map such as #{{ fx: \"reverb\", ... }} (effects: {})",
            EFFECT_NAMES.join(" ")
        ));
    }
    serde_json::from_value::<Effect>(v).map_err(|e| {
        let e = e.to_string();
        let moves = if e.contains("untagged enum Param") { format!(": {MOVES}") } else { String::new() };
        format!("{what}: {e}{moves}").into()
    })
}

/// a script's value as JSON: numbers, strings, true and false, arrays and maps
fn json_of(d: &Dynamic) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    if let Ok(b) = d.as_bool() {
        return Ok(Value::Bool(b));
    }
    if let Ok(i) = d.as_int() {
        return Ok(i.into());
    }
    if let Ok(f) = d.as_float() {
        return serde_json::Number::from_f64(f).map(Value::Number).ok_or_else(|| format!("{f} is not a number"));
    }
    if let Some(s) = text(d) {
        return Ok(Value::String(s));
    }
    if let Some(a) = d.clone().try_cast::<Array>() {
        return a.iter().map(json_of).collect::<Result<_, _>>().map(Value::Array);
    }
    if let Some(m) = d.clone().try_cast::<Map>() {
        return m
            .iter()
            .map(|(k, v)| Ok((k.to_string(), json_of(v)?)))
            .collect::<Result<_, String>>()
            .map(Value::Object);
    }
    Err(format!("a {} is not a number, a string, an array or a map", d.type_name()))
}
