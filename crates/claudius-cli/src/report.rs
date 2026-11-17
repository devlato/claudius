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
    if let Some(a) = &e.aesthetics {
        println!(
            "        aesthetics (of 10): enjoyment {:.1}, production quality {:.1}; held out: usefulness {:.1}, complexity {:.1}",
            a.ce, a.pq, a.cu, a.pc
        );
    }
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
    for (i, line) in a.text().lines().enumerate() {
        let indent = if i == 0 { "" } else { "  " };
        println!("{indent}{line}");
    }
}
