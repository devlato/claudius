//! The pattern functions and methods scripts call: constructors (`s`, `note`, `n`, `chord`, `seq`), time,
//! structure, randomness, per-event controls, and the settings a track takes from its pattern.

use std::sync::Arc;

use rhai::{Array, Dynamic, Engine, FnPtr, INT, NativeCallContext};

use crate::pattern::{CtrlKey, Kind, Node, Pattern, Value, fmt_num, mini, notes, quote};
use crate::rng::hash_str;

use super::{R, constant, fail, param};

fn source(kind: Kind, func: &str, d: &Dynamic) -> R<Pattern> {
    let (node, repr) = param(d)?;
    Ok(Pattern::new(node, kind, format!("{func}({repr})")))
}

/// call a closure once with the real pattern (the result) and once with a stand-in (to learn the closure's code)
fn apply_fn(ctx: &NativeCallContext, f: &FnPtr, p: &Pattern) -> R<(Pattern, String)> {
    let real: Pattern = f.call_within_context(ctx, (p.clone(),))?;
    let shown: Pattern = f.call_within_context(ctx, (Pattern::placeholder(),))?;
    Ok((real, format!("|x| {}", shown.repr)))
}

pub(super) fn register_patterns(engine: &mut Engine) {
    engine.register_type_with_name::<Pattern>("Pattern");
    for (name, kind) in [
        ("s", Kind::Sound),
        ("sound", Kind::Sound),
        ("note", Kind::Note),
        ("n", Kind::Degree),
        ("chord", Kind::Chord),
        ("seq", Kind::Plain),
    ] {
        engine.register_fn(name, move |d: Dynamic| source(kind, name, &d));
    }
    engine.register_fn("silence", || Pattern::new(Node::Silence, Kind::Plain, "silence()".into()));
    engine.register_fn("stack", |items: Array| -> R<Pattern> {
        let ps: Vec<Pattern> = items
            .into_iter()
            .map(|d| d.try_cast::<Pattern>().ok_or("stack() takes patterns"))
            .collect::<Result<_, _>>()?;
        let first = ps.first().cloned().ok_or("stack() needs at least one pattern")?;
        Ok(Pattern {
            node: Arc::new(Node::Stack(ps.iter().map(|p| p.node()).collect())),
            repr: format!("stack([{}])", ps.iter().map(|p| p.repr.clone()).collect::<Vec<_>>().join(", ")),
            ..first
        })
    });
    engine.register_fn("cat", |items: Array| -> R<Pattern> {
        let ps: Vec<Pattern> = items
            .into_iter()
            .map(|d| d.try_cast::<Pattern>().ok_or("cat() takes patterns"))
            .collect::<Result<_, _>>()?;
        let first = ps.first().cloned().ok_or("cat() needs at least one pattern")?;
        Ok(Pattern {
            node: Arc::new(Node::Alt(ps.iter().map(|p| p.node()).collect())),
            repr: format!("cat([{}])", ps.iter().map(|p| p.repr.clone()).collect::<Vec<_>>().join(", ")),
            ..first
        })
    });
    // time
    engine.register_fn("fast", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let f = crate::pattern::check_speed(constant(&x, "fast")?, "fast")?;
        Ok(p.with(Node::Fast(Box::new(p.node()), f), &format!(".fast({})", fmt_num(f))))
    });
    engine.register_fn("slow", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let f = crate::pattern::check_speed(constant(&x, "slow")?, "slow")?;
        Ok(p.with(Node::Fast(Box::new(p.node()), 1.0 / f), &format!(".slow({})", fmt_num(f))))
    });
    engine.register_fn("late", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let t = constant(&x, "late")?;
        Ok(p.with(Node::Late(Box::new(p.node()), t), &format!(".late({})", fmt_num(t))))
    });
    engine.register_fn("early", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let t = constant(&x, "early")?;
        Ok(p.with(Node::Late(Box::new(p.node()), -t), &format!(".early({})", fmt_num(t))))
    });
    engine.register_fn("rev", |p: &mut Pattern| p.with(Node::Rev(Box::new(p.node())), ".rev()"));
    engine.register_fn("ply", |_: &mut Pattern, x: rhai::FLOAT| -> R<Pattern> {
        fail(format!("ply takes a whole number from 1 to 256, not {}", fmt_num(x)))
    });
    engine.register_fn("ply", |p: &mut Pattern, n: INT| -> R<Pattern> {
        let k = crate::pattern::check_speed(n as f64, "ply")?;
        Ok(p.with(Node::Ply(Box::new(p.node()), k as u32), &format!(".ply({n})")))
    });
    // higher order
    engine.register_fn("every", |ctx: NativeCallContext, p: &mut Pattern, n: INT, f: FnPtr| -> R<Pattern> {
        let (t, code) = apply_fn(&ctx, &f, p)?;
        Ok(p.with(
            Node::Every(n.max(1) as u32, 0, Box::new(p.node()), Box::new(t.node())),
            &format!(".every({n}, {code})"),
        ))
    });
    engine.register_fn("lastOf", |ctx: NativeCallContext, p: &mut Pattern, n: INT, f: FnPtr| -> R<Pattern> {
        let (t, code) = apply_fn(&ctx, &f, p)?;
        let n = n.max(1) as u32;
        Ok(p.with(Node::Every(n, n - 1, Box::new(p.node()), Box::new(t.node())), &format!(".lastOf({n}, {code})")))
    });
    for (name, prob) in
        [("sometimes", 0.5), ("often", 0.75), ("rarely", 0.25), ("almostNever", 0.1), ("almostAlways", 0.9)]
    {
        engine.register_fn(name, move |ctx: NativeCallContext, p: &mut Pattern, f: FnPtr| -> R<Pattern> {
            let seed = hash_str(&p.repr) ^ hash_str(name);
            let chosen = p.with(Node::Keep(Box::new(p.node()), prob, seed), "");
            let (t, code) = apply_fn(&ctx, &f, &chosen)?;
            Ok(p.with(Node::Sometimes(Box::new(p.node()), Box::new(t.node()), prob, seed), &format!(".{name}({code})")))
        });
    }
    engine.register_fn(
        "sometimesBy",
        |ctx: NativeCallContext, p: &mut Pattern, prob: Dynamic, f: FnPtr| -> R<Pattern> {
            let pr = constant(&prob, "sometimesBy")?;
            let seed = hash_str(&p.repr) ^ 0x5b;
            let chosen = p.with(Node::Keep(Box::new(p.node()), pr, seed), "");
            let (t, code) = apply_fn(&ctx, &f, &chosen)?;
            Ok(p.with(
                Node::Sometimes(Box::new(p.node()), Box::new(t.node()), pr, seed),
                &format!(".sometimesBy({}, {code})", fmt_num(pr)),
            ))
        },
    );
    engine.register_fn("off", |ctx: NativeCallContext, p: &mut Pattern, t: Dynamic, f: FnPtr| -> R<Pattern> {
        let shift = constant(&t, "off")?;
        let (tp, code) = apply_fn(&ctx, &f, p)?;
        let node = Node::Stack(vec![p.node(), Node::Late(Box::new(tp.node()), shift)]);
        Ok(p.with(node, &format!(".off({}, {code})", fmt_num(shift))))
    });
    engine.register_fn("superimpose", |ctx: NativeCallContext, p: &mut Pattern, f: FnPtr| -> R<Pattern> {
        let (tp, code) = apply_fn(&ctx, &f, p)?;
        Ok(p.with(Node::Stack(vec![p.node(), tp.node()]), &format!(".superimpose({code})")))
    });
    engine.register_fn("degrade", |p: &mut Pattern| {
        p.with(Node::Degrade(Box::new(p.node()), 0.5, hash_str(&p.repr)), ".degrade()")
    });
    engine.register_fn("degradeBy", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let pr = constant(&x, "degradeBy")?;
        Ok(p.with(Node::Degrade(Box::new(p.node()), pr, hash_str(&p.repr)), &format!(".degradeBy({})", fmt_num(pr))))
    });
    // structure
    engine.register_fn("struct", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let (n, r) = param(&x)?;
        Ok(p.with(Node::Struct(Box::new(p.node()), Box::new(n)), &format!(".struct({r})")))
    });
    engine.register_fn("mask", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let (n, r) = param(&x)?;
        Ok(p.with(Node::Mask(Box::new(p.node()), Box::new(n)), &format!(".mask({r})")))
    });
    // the same limit as euclid in mini-notation
    fn euclid(k: INT, n: INT, r: INT) -> R<Node> {
        if !(1..=mini::MAX_STEPS as INT).contains(&n) {
            return fail(format!("a euclidean rhythm has 1 to {} steps, not {n}", mini::MAX_STEPS));
        }
        Ok(mini::euclid(Node::Atom(Value::Num(1.0)), k.clamp(0, n) as usize, n as usize, r.rem_euclid(n) as usize))
    }
    engine.register_fn("euclid", |p: &mut Pattern, k: INT, n: INT| -> R<Pattern> {
        Ok(p.with(Node::Struct(Box::new(p.node()), Box::new(euclid(k, n, 0)?)), &format!(".euclid({k}, {n})")))
    });
    engine.register_fn("euclidRot", |p: &mut Pattern, k: INT, n: INT, r: INT| -> R<Pattern> {
        let s = euclid(k, n, r)?;
        Ok(p.with(Node::Struct(Box::new(p.node()), Box::new(s)), &format!(".euclidRot({k}, {n}, {r})")))
    });
    // per-event controls
    for (name, key) in [
        ("gain", CtrlKey::Gain),
        ("velocity", CtrlKey::Velocity),
        ("legato", CtrlKey::Legato),
        ("add", CtrlKey::Add),
        ("transpose", CtrlKey::Transpose),
    ] {
        engine.register_fn(name, move |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
            let (n, r) = param(&x)?;
            Ok(p.with(Node::Ctrl(Box::new(p.node()), key, Box::new(n)), &format!(".{name}({r})")))
        });
    }
    // `n` on a sound pattern picks the sample index, like Strudel
    engine.register_fn("n", |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
        let (n, r) = param(&x)?;
        Ok(p.with(Node::Ctrl(Box::new(p.node()), CtrlKey::Index, Box::new(n)), &format!(".n({r})")))
    });
    // track-level settings
    fn opt<F: Fn(&mut crate::pattern::TrackOpts)>(p: &Pattern, suffix: String, f: F) -> Pattern {
        let mut q = p.clone();
        f(&mut q.opts);
        q.repr.push_str(&suffix);
        q
    }
    engine.register_fn("s", |p: &mut Pattern, name: &str| {
        opt(p, format!(".s({})", quote(name)), |o| o.sound = Some(name.to_string()))
    });
    engine.register_fn("sound", |p: &mut Pattern, name: &str| {
        opt(p, format!(".sound({})", quote(name)), |o| o.sound = Some(name.to_string()))
    });
    engine.register_fn("bank", |p: &mut Pattern, name: &str| {
        opt(p, format!(".bank({})", quote(name)), |o| o.bank = Some(name.to_string()))
    });
    engine.register_fn("scale", |p: &mut Pattern, name: &str| -> R<Pattern> {
        if notes::scale(name).is_none() {
            return fail(format!(
                "unknown scale '{name}' (try \"A:minor\", \"C:major\", \"D:dorian\", \"E:minor:pentatonic\")"
            ));
        }
        Ok(opt(p, format!(".scale({})", quote(name)), |o| o.scale = Some(name.to_string())))
    });
    engine.register_fn("octave", |p: &mut Pattern, n: INT| {
        opt(p, format!(".octave({n})"), |o| o.octave = Some(n as i32))
    });
    engine.register_fn("voicing", |p: &mut Pattern| {
        opt(p, ".voicing()".into(), |o| o.voicing = Some(o.voicing.unwrap_or((52, 74))))
    });
    engine.register_fn("voicing", |p: &mut Pattern, lo: INT, hi: INT| {
        opt(p, format!(".voicing({lo}, {hi})"), |o| o.voicing = Some((lo as i32, hi as i32)))
    });
    engine.register_fn("arp", |p: &mut Pattern, mode: &str| -> R<Pattern> {
        if !matches!(mode, "up" | "down" | "updown") {
            return fail(format!("unknown arp '{mode}' (\"up\", \"down\" or \"updown\")"));
        }
        Ok(opt(p, format!(".arp({})", quote(mode)), |o| o.arp = Some(mode.to_string())))
    });
    engine.register_fn("root", |p: &mut Pattern| opt(p, ".root()".into(), |o| o.root = true));
    engine.register_fn("role", |p: &mut Pattern, r: &str| {
        opt(p, format!(".role({})", quote(r)), |o| o.role = Some(r.to_string()))
    });
    macro_rules! num_opt {
        ($name:literal, $field:ident) => {
            engine.register_fn($name, |p: &mut Pattern, x: Dynamic| -> R<Pattern> {
                let v = constant(&x, $name)? as f32;
                Ok(opt(p, format!(".{}({})", $name, fmt_num(v as f64)), |o| o.$field = Some(v)))
            });
        };
    }
    num_opt!("level", level_db);
    num_opt!("pan", pan);
    num_opt!("room", reverb);
    num_opt!("reverb", reverb);
    num_opt!("delay", delay);
    num_opt!("duck", duck);
    num_opt!("lpf", lpf);
    num_opt!("cutoff", lpf);
    num_opt!("hpf", hpf);
    num_opt!("crush", crush);
    num_opt!("distort", drive);
}
