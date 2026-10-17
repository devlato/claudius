//! Drum step strings (`"X...x...x..ox..."`): reading them into hits and printing hits back.

use crate::theory::rhythm::Hit;

pub(super) fn cells(steps: &str) -> String {
    steps.chars().filter(|c| !c.is_whitespace() && *c != '|').collect()
}

pub(super) fn tile_hits(hits: &[Hit], bars: u32, to_bars: u32) -> Vec<Hit> {
    let (period, end) = (bars as f64 * 4.0, to_bars as f64 * 4.0);
    let mut out = Vec::new();
    let mut off = 0.0;
    while off < end - 1e-9 {
        out.extend(hits.iter().map(|h| Hit { t: h.t + off, v: h.v }).filter(|h| h.t < end - 1e-9));
        off += period;
    }
    out
}

/// a step string with repeated bars printed once
pub(super) fn shortest_steps(steps: &str) -> String {
    let bars: Vec<&str> = steps.split('|').collect();
    if bars.len() > 1 && bars.iter().all(|b| *b == bars[0]) { bars[0].to_string() } else { steps.to_string() }
}

pub fn steps_to_hits(steps: &str, per_bar: u32) -> (Vec<Hit>, u32) {
    let cells: Vec<char> = cells(steps).chars().collect();
    let per_bar = per_bar.max(1);
    let step = 4.0 / per_bar as f64;
    let hits = cells
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let v = match c {
                'X' => 1.0,
                'x' => 0.8,
                'o' => 0.45,
                d if d.is_ascii_digit() && *d != '0' => d.to_digit(10).unwrap() as f32 / 10.0,
                _ => return None,
            };
            Some(Hit { t: i as f64 * step, v })
        })
        .collect();
    (hits, (cells.len() as u32).div_ceil(per_bar).clamp(1, 16))
}

/// a lane variant as a step string (16, 32 or 48 cells per bar, '|' between bars) and its resolution
/// velocities a step string writes as `X` (from here) and as `o` (below `STEP_GHOST`); the studio names them the same way
pub const STEP_ACCENT: f32 = 0.85;
pub const STEP_GHOST: f32 = 0.5;

pub fn hits_to_steps(hits: &[Hit], bars: u32) -> (String, u32) {
    let res = [16u32, 32, 48]
        .into_iter()
        .find(|r| {
            hits.iter().all(|h| {
                let x = h.t * *r as f64 / 4.0;
                (x - x.round()).abs() < 1e-6
            })
        })
        .unwrap_or(48);
    let cells = (res * bars.max(1)) as usize;
    let mut s = vec!['.'; cells];
    for h in hits {
        let i = (h.t * res as f64 / 4.0).round() as usize;
        if i < cells {
            s[i] = if h.v >= STEP_ACCENT {
                'X'
            } else if h.v >= STEP_GHOST {
                'x'
            } else {
                'o'
            };
        }
    }
    let mut out = String::new();
    for (i, c) in s.iter().enumerate() {
        if i > 0 && i % res as usize == 0 {
            out.push('|');
        }
        out.push(*c);
    }
    (out, res)
}
