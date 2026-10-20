//! What the commands print: a song's summary, the ears' report and an analysis.

use claudius::judge::{EarsReport, Verdict};
use claudius::model::Blueprint;
use claudius::render::Rendered;
use claudius::song;

pub(crate) fn print_ears(e: &EarsReport) {
    println!(
        "  ears: '{}' z {:+.1} (closest confusion '{}' {:+.1}); production z {:+.1} vs '{}' {:+.1}{}",
        e.genre_query,
        e.genre_z,
        e.best_alt,
        e.best_alt_z,
        e.production_z,
        e.worst_flaw,
        e.worst_flaw_z,
        e.p_real.map(|p| format!("; p_real {p:.2}")).unwrap_or_default()
    );
    println!("        sounds like: {}", e.sounds_like.join(", "));
    if let Some(z) = e.ref_typicality_z {
        println!("        typicality vs the reference tracks: z {z:+.1}");
    }
    if !e.audioset.is_empty() {
        println!(
            "        AudioSet (held out): {}",
            e.audioset.iter().map(|(l, p)| format!("{l} {p:.2}")).collect::<Vec<_>>().join(", ")
        );
    }
}

pub(crate) fn print_summary(bp: &Blueprint, r: &Rendered, w: &song::Written, v: &Verdict, secs: f64) {
    println!(
        "{} — {} at {:.0} BPM in {}, {} bars ({:.0}:{:02.0})",
        bp.title,
        bp.genre,
        bp.bpm,
        bp.key.name(),
        bp.bars(),
        (bp.secs() / 60.0).floor(),
        bp.secs() % 60.0
    );
    for (k, v) in &bp.harmony.progressions {
        println!("  progression {k}: {}", v.join(" "));
    }
    println!("  form: {}", bp.form.iter().map(|s| format!("{} {}", s.name, s.bars)).collect::<Vec<_>>().join(" | "));
    println!(
        "  tracks: {}",
        r.tracks.iter().map(|t| format!("{} ({})", t.name, t.sound)).collect::<Vec<_>>().join(", ")
    );
    println!(
        "  master: {:.1} LUFS, true peak {:.1} dBTP (pre-master {:.1} LUFS){}",
        r.lufs,
        r.true_peak_db,
        r.premaster_lufs,
        if secs > 0.0 { format!("; done in {secs:.1} s") } else { String::new() }
    );
    if let Some(e) = &v.ears {
        print_ears(e);
    }
    println!("  verdict: {}", v.summary);
    println!("  wrote {}", w.mp3.display());
}

pub(crate) fn print_analysis(a: &claudius::analysis::song::Analysis) {
    println!(
        "{:.1} BPM (pulse clarity {:.2}), {} (confidence {:.2}), swing {:.2}, {} bars from {:.2} s",
        a.bpm,
        a.pulse_clarity,
        a.key.name(),
        a.key_confidence,
        a.swing,
        a.bars,
        a.first_downbeat
    );
    println!(
        "  progression A: {}  |  B: {}  ({} bar(s) per chord)",
        a.progression_a.join(" "),
        a.progression_b.join(" "),
        a.bars_per_chord
    );
    for s in &a.sections {
        let mut l = Vec::new();
        for (on, n) in [(s.kick, "kick"), (s.bass, "bass"), (s.hats, "hats")] {
            if on {
                l.push(n);
            }
        }
        println!("  bar {:3} +{:2}  {:7} energy {:.2}  {}", s.start_bar, s.bars, s.kind.name(), s.energy, l.join(" "));
    }
    let grid = claudius::analysis::song::grid;
    println!(
        "  kick  {}\n  snare {}\n  hats  {}\n  bass style: {}",
        grid(&a.kick_steps),
        grid(&a.snare_steps),
        grid(&a.hat_steps),
        a.bass_style
    );
}
