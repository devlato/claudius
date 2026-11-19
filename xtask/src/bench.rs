//! `cargo xtask bench compare BASE NEW`: an experiment's results on a benchmark compared with the baseline's, item by
//! item (docs/src/contributing/benchmarks.md).
//!
//! A results file has one JSON object a line for each item of the benchmark: its `item` (an id, the same in every run
//! of the benchmark), the `slice` it is counted in (`{"set": "round10"}`, `{"genre": "house", "decade": "2010s"}`), and
//! its `metrics`, each `true`/`false` (right or wrong) or a number (a score). `"better": {"wrong": "lower"}`, on an
//! item's line or a line of its own, names the metrics a lower value is better of; every other metric is better higher.
//! `"primary": ["right"]` names the metrics a change is made to improve; the others are guards it must not make worse.
//! Without it, every metric is primary. `"shown": ["total"]` names metrics compared and shown but never judged: a level
//! of which neither more nor less is better (a critic's total for a song as it is, which a critic rating every song
//! 100 would raise).
//!
//! Each metric is compared overall and in each slice: right/wrong items with McNemar's exact test on the items one run
//! gets right and the other wrong, scores with the Wilcoxon signed-rank test on the paired differences; the difference
//! of the means with a 95% interval from 10,000 paired bootstrap resamples; and the slices of each kind corrected
//! together with Holm's method. Which way a group went is the test's own: for a score, whether the ranks of the items
//! that got better outweigh those of the items that got worse; for right/wrong, whether more items are right only in
//! the experiment than only in the baseline. A change passes when each primary metric is better overall at p < 0.05,
//! with the mean better too, and no metric is worse overall at p < 0.05 or in a slice at p < 0.05 after the correction.
//! A comparison of nothing (no items, or a primary metric no item has) is an error, not a pass.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

/// how many paired resamples the intervals are drawn from
const RESAMPLES: usize = 10_000;

/// the significance level of every test
const ALPHA: f64 = 0.05;

/// One item's results in one run.
#[derive(Clone, Debug)]
struct Item {
    slice: BTreeMap<String, String>,
    metrics: BTreeMap<String, f64>,
    /// the metrics that are right or wrong rather than scores
    binary: BTreeSet<String>,
}

/// A run's results: each item by its id, and the metrics better lower.
#[derive(Debug, Default)]
struct Run {
    items: HashMap<String, Item>,
    lower: BTreeSet<String>,
    primary: BTreeSet<String>,
    shown: BTreeSet<String>,
}

fn read(path: &Path) -> Result<Run> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut run = Run::default();
    for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let at = || format!("{}:{}", path.display(), n + 1);
        let v: Value = serde_json::from_str(line).with_context(at)?;
        if let Some(better) = v.get("better").and_then(Value::as_object) {
            for (k, way) in better {
                match way.as_str() {
                    Some("lower") => run.lower.insert(k.clone()),
                    Some("higher") => run.lower.remove(k),
                    _ => bail!("{}: `better` says \"higher\" or \"lower\" of each metric", at()),
                };
            }
        }
        for m in v.get("primary").and_then(Value::as_array).into_iter().flatten() {
            run.primary
                .insert(m.as_str().with_context(|| format!("{}: `primary` lists metrics by name", at()))?.into());
        }
        for m in v.get("shown").and_then(Value::as_array).into_iter().flatten() {
            run.shown.insert(m.as_str().with_context(|| format!("{}: `shown` lists metrics by name", at()))?.into());
        }
        if v.get("item").is_none() {
            continue;
        }
        let id = v["item"].as_str().with_context(|| format!("{}: no `item`", at()))?.to_string();
        let slice = v["slice"]
            .as_object()
            .map(|m| {
                m.iter().map(|(k, v)| (k.clone(), v.as_str().map_or_else(|| v.to_string(), str::to_string))).collect()
            })
            .unwrap_or_default();
        let (mut metrics, mut binary) = (BTreeMap::new(), BTreeSet::new());
        for (k, m) in v["metrics"].as_object().with_context(|| format!("{}: no `metrics`", at()))? {
            let value = match m {
                Value::Bool(b) => {
                    binary.insert(k.clone());
                    f64::from(u8::from(*b))
                }
                _ => {
                    m.as_f64().with_context(|| format!("{}: metric `{k}` is neither true/false nor a number", at()))?
                }
            };
            metrics.insert(k.clone(), value);
        }
        ensure!(run.items.insert(id.clone(), Item { slice, metrics, binary }).is_none(), "{}: item {id} twice", at());
    }
    Ok(run)
}

/// The comparison of one metric in one group of items.
#[derive(Clone, Debug)]
struct Row {
    group: String,
    n: usize,
    base: f64,
    new: f64,
    /// the interval of `new - base`
    low: f64,
    high: f64,
    /// items better in the experiment, and worse
    better: usize,
    worse: usize,
    /// which way the test's statistic leans, above zero better: for a score, the summed ranks of the items better less
    /// those of the items worse; for right/wrong, the items better less the items worse
    lean: f64,
    p: f64,
    /// after Holm's correction among the slices of its kind (the whole set is not corrected)
    adjusted: f64,
}

/// `cargo xtask bench compare`: prints the comparison as Markdown for a pull request, and fails when a metric does not
/// pass (with `--report`, only prints).
pub fn compare(base: &Path, new: &Path, report: bool) -> Result<()> {
    let (b, n) = (read(base)?, read(new)?);
    let only_base = b.items.keys().filter(|k| !n.items.contains_key(*k)).count();
    let only_new = n.items.keys().filter(|k| !b.items.contains_key(*k)).count();
    ensure!(
        only_base == 0 && only_new == 0,
        "the runs are of different items: {only_base} only in {}, {only_new} only in {}",
        base.display(),
        new.display()
    );
    ensure!(!b.items.is_empty(), "no items in {} and {}: nothing to compare", base.display(), new.display());
    let lower: BTreeSet<String> = b.lower.union(&n.lower).cloned().collect();
    let metrics: BTreeSet<&String> = b.items.values().flat_map(|i| i.metrics.keys()).collect();
    let primary: BTreeSet<String> = b.primary.union(&n.primary).cloned().collect();
    let missing: Vec<&str> = primary.iter().filter(|m| !metrics.contains(m)).map(String::as_str).collect();
    ensure!(missing.is_empty(), "the primary metric {} is in no item", missing.join(", "));
    let shown: BTreeSet<String> = b.shown.union(&n.shown).cloned().collect();
    let missing: Vec<&str> = shown.iter().filter(|m| !metrics.contains(m)).map(String::as_str).collect();
    ensure!(missing.is_empty(), "the shown metric {} is in no item", missing.join(", "));
    let both: Vec<&str> = shown.intersection(&primary).map(String::as_str).collect();
    ensure!(both.is_empty(), "{} is primary and shown: a metric is judged or only shown", both.join(", "));
    let mut failed = Vec::new();
    for metric in metrics {
        let sign = if lower.contains(metric) { -1.0 } else { 1.0 };
        // each item's pair of values, and its slices
        let mut pairs: Vec<(&Item, f64, f64)> = Vec::new();
        let mut binary = true;
        let mut worse_items: Vec<&String> = Vec::new();
        let mut ids: Vec<&String> = b.items.keys().collect();
        ids.sort();
        for id in ids {
            let (bi, ni) = (&b.items[id], &n.items[id]);
            let (x, y) = match (bi.metrics.get(metric), ni.metrics.get(metric)) {
                (Some(x), Some(y)) => (*x, *y),
                // an item that measures it in neither run is not one of its pairs (a benchmark's items may measure
                // different things: the critic's songs as they are their total, their flawed copies how they rank)
                (None, None) => continue,
                _ => bail!("item {id} has `{metric}` in one run only"),
            };
            binary &= bi.binary.contains(metric) && ni.binary.contains(metric);
            if sign * (y - x) < 0.0 {
                worse_items.push(id);
            }
            pairs.push((bi, x, y));
        }
        let mut rows = vec![row("all", &pairs, binary, sign)];
        let kinds: BTreeSet<&String> = pairs.iter().flat_map(|(i, _, _)| i.slice.keys()).collect();
        for kind in kinds {
            let values: BTreeSet<&String> = pairs.iter().filter_map(|(i, _, _)| i.slice.get(kind)).collect();
            let mut slices: Vec<Row> = values
                .iter()
                .map(|v| {
                    let some: Vec<(&Item, f64, f64)> =
                        pairs.iter().filter(|(i, _, _)| i.slice.get(kind) == Some(v)).copied().collect();
                    row(&format!("{kind} = {v}"), &some, binary, sign)
                })
                .collect();
            let adjusted = holm(&slices.iter().map(|r| r.p).collect::<Vec<_>>());
            for (r, a) in slices.iter_mut().zip(adjusted) {
                r.adjusted = a;
            }
            rows.extend(slices);
        }
        // which way each group went is the test's own statistic; better overall needs the mean to agree with it as well,
        // so that a few items much better cannot carry a change most items got worse with, nor the other way round
        let all = &rows[0];
        let leans_better = all.p < ALPHA && all.lean > 0.0;
        let gained = leans_better && sign * (all.new - all.base) > 0.0;
        let worse: Vec<&Row> = rows.iter().filter(|r| r.adjusted < ALPHA && r.lean < 0.0).collect();
        let aim = (primary.is_empty() && !shown.contains(metric)) || primary.contains(metric);
        if shown.contains(metric) {
            print(metric, binary, sign, &rows, "shown, not judged", &worse_items);
            continue;
        }
        let verdict = match (aim, gained, worse.is_empty()) {
            (_, _, false) => format!("does not pass: worse in {}", names(&worse)),
            (true, true, true) => "passes: better overall, and worse in no slice".to_string(),
            (true, false, true) if leans_better => {
                "does not pass: most items better, but the mean is not (a few items much worse)".to_string()
            }
            (true, false, true) => "does not pass: not shown better overall".to_string(),
            (false, _, true) => "holds: worse in no slice (a guard)".to_string(),
        };
        if !worse.is_empty() || (aim && !gained) {
            failed.push(metric.clone());
        }
        print(metric, binary, sign, &rows, &verdict, &worse_items);
    }
    if !failed.is_empty() && !report {
        bail!("not an improvement in {}", failed.join(", "));
    }
    Ok(())
}

fn names(rows: &[&Row]) -> String {
    rows.iter().map(|r| r.group.as_str()).collect::<Vec<_>>().join(", ")
}

fn row(group: &str, pairs: &[(&Item, f64, f64)], binary: bool, sign: f64) -> Row {
    let n = pairs.len();
    let mean = |f: &dyn Fn(&(&Item, f64, f64)) -> f64| pairs.iter().map(f).sum::<f64>() / n.max(1) as f64;
    let (base, new) = (mean(&|p| p.1), mean(&|p| p.2));
    let diffs: Vec<f64> = pairs.iter().map(|p| p.2 - p.1).collect();
    let better = diffs.iter().filter(|d| sign * **d > 0.0).count();
    let worse = diffs.iter().filter(|d| sign * **d < 0.0).count();
    let (p, lean) = if binary {
        (mcnemar(better, worse), better as f64 - worse as f64)
    } else {
        let (p, lean) = wilcoxon(&diffs);
        (p, sign * lean)
    };
    let (low, high) = interval(&diffs);
    Row { group: group.to_string(), n, base, new, low, high, better, worse, lean, p, adjusted: p }
}

/// McNemar's exact test, two-sided: of the `b + c` items the runs disagree on, how unlikely a split this uneven is when
/// either run is as likely as the other to get each right
fn mcnemar(b: usize, c: usize) -> f64 {
    let n = b + c;
    if n == 0 {
        return 1.0;
    }
    // the binomial tail with p = 1/2, summed in logarithms so that a thousand items do not overflow
    let k = b.min(c);
    let mut ln_choose = 0.0; // ln C(n, 0)
    let mut tail = 0.0;
    for i in 0..=k {
        if i > 0 {
            ln_choose += ((n - i + 1) as f64).ln() - (i as f64).ln();
        }
        tail += (ln_choose - n as f64 * std::f64::consts::LN_2).exp();
    }
    (2.0 * tail).min(1.0)
}

/// The Wilcoxon signed-rank test, two-sided, on paired differences: the ranks of their sizes, zeros left out and ties
/// given their mean rank, with the normal approximation corrected for ties and continuity (exact below 20 differences
/// without ties); and which way the differences lean, the summed ranks of the positive ones less those of the negative
fn wilcoxon(diffs: &[f64]) -> (f64, f64) {
    let mut d: Vec<f64> = diffs.iter().copied().filter(|x| x.abs() > 1e-12).collect();
    let n = d.len();
    if n == 0 {
        return (1.0, 0.0);
    }
    d.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
    let (mut ranks, mut ties) = (vec![0.0; n], 0.0);
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && (d[j + 1].abs() - d[i].abs()).abs() < 1e-12 {
            j += 1;
        }
        let t = (j - i + 1) as f64;
        ties += t * t * t - t;
        for r in &mut ranks[i..=j] {
            *r = (i + j) as f64 / 2.0 + 1.0;
        }
        i = j + 1;
    }
    let plus: f64 = d.iter().zip(&ranks).filter(|(x, _)| **x > 0.0).map(|(_, r)| r).sum();
    let nf = n as f64;
    let mean = nf * (nf + 1.0) / 4.0;
    // the negative ranks sum to n(n + 1)/2 less the positive ones
    let lean = 2.0 * (plus - mean);
    if n < 20 && ties == 0.0 {
        // the exact distribution of the sum of the positive ranks: how many of the 2^n sign patterns give each sum
        let most = n * (n + 1) / 2;
        let mut ways = vec![0.0f64; most + 1];
        ways[0] = 1.0;
        for r in 1..=n {
            for s in (r..=most).rev() {
                ways[s] += ways[s - r];
            }
        }
        let total = 2f64.powi(n as i32);
        let w = plus.min(nf * (nf + 1.0) / 2.0 - plus).round() as usize;
        let tail: f64 = ways[..=w].iter().sum::<f64>() / total;
        return ((2.0 * tail).min(1.0), lean);
    }
    let var = nf * (nf + 1.0) * (2.0 * nf + 1.0) / 24.0 - ties / 48.0;
    let z = ((plus - mean).abs() - 0.5).max(0.0) / var.sqrt();
    ((2.0 * (1.0 - normal_cdf(z))).min(1.0), lean)
}

/// the standard normal distribution below `z` (Abramowitz and Stegun 7.1.26, within 1.5e-7)
fn normal_cdf(z: f64) -> f64 {
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly =
        t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    if z >= 0.0 { 0.5 * (1.0 + erf) } else { 0.5 * (1.0 - erf) }
}

/// Holm's correction of p-values tested together: the smallest times their number, the next times one fewer, each at
/// least the one before it
fn holm(p: &[f64]) -> Vec<f64> {
    let m = p.len();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|a, b| p[*a].total_cmp(&p[*b]));
    let mut out = vec![1.0; m];
    let mut most: f64 = 0.0;
    for (k, &i) in order.iter().enumerate() {
        most = most.max(((m - k) as f64 * p[i]).min(1.0));
        out[i] = most;
    }
    out
}

/// a 95% interval of the mean difference: the 2.5th and 97.5th percentiles of the means of `RESAMPLES` resamples of the
/// items with replacement, drawn the same way every run
fn interval(diffs: &[f64]) -> (f64, f64) {
    let n = diffs.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        // xorshift64*
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    };
    let mut means: Vec<f64> =
        (0..RESAMPLES).map(|_| (0..n).map(|_| diffs[(next() % n as u64) as usize]).sum::<f64>() / n as f64).collect();
    means.sort_by(f64::total_cmp);
    (means[RESAMPLES * 25 / 1000], means[RESAMPLES * 975 / 1000 - 1])
}

fn print(metric: &str, binary: bool, sign: f64, rows: &[Row], verdict: &str, worse_items: &[&String]) {
    let way = if sign > 0.0 { "higher" } else { "lower" };
    let kind = if binary { "right or wrong; McNemar's exact test" } else { "a score; Wilcoxon signed-rank test" };
    println!("### {metric} ({way} is better; {kind})\n");
    println!("| | items | base | experiment | difference (95% interval) | better | worse | p | p, Holm |");
    println!("|---|---:|---:|---:|---|---:|---:|---:|---:|");
    let show = |v: f64| if binary { format!("{:.1}%", v * 100.0) } else { format!("{v:.4}") };
    let diff = |v: f64| if binary { format!("{:+.1}", v * 100.0) } else { format!("{v:+.4}") };
    for r in rows {
        let adjusted = if r.group == "all" { "".to_string() } else { format!("{:.3}", r.adjusted) };
        println!(
            "| {} | {} | {} | {} | {} ({} to {}) | {} | {} | {:.4} | {adjusted} |",
            r.group,
            r.n,
            show(r.base),
            show(r.new),
            diff(r.new - r.base),
            diff(r.low),
            diff(r.high),
            r.better,
            r.worse,
            r.p,
        );
    }
    println!("\n**{metric}: {verdict}.**\n");
    if !worse_items.is_empty() {
        let shown: Vec<&str> = worse_items.iter().take(50).map(|s| s.as_str()).collect();
        let more = worse_items.len().saturating_sub(shown.len());
        let more = if more > 0 { format!(" and {more} more") } else { String::new() };
        println!("Worse in the experiment: {}{more}.\n", shown.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcnemars_test_matches_the_binomial_tail() {
        assert_eq!(mcnemar(0, 0), 1.0);
        // 10 against 2 of 12 disagreements: 2 * (1 + 12 + 66) / 4096
        assert!((mcnemar(10, 2) - 2.0 * 79.0 / 4096.0).abs() < 1e-12);
        assert!((mcnemar(2, 10) - mcnemar(10, 2)).abs() < 1e-12, "two-sided");
        assert_eq!(mcnemar(5, 5), 1.0);
        assert!(mcnemar(600, 400) < 1e-9 && mcnemar(600, 400) > 0.0, "a thousand disagreements do not overflow");
    }

    #[test]
    fn the_signed_rank_test_is_exact_for_few_and_approximate_for_many() {
        // eight differences all positive: 2 / 2^8
        assert!((wilcoxon(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]).0 - 2.0 / 256.0).abs() < 1e-12);
        assert_eq!(wilcoxon(&[0.0, 0.0]), (1.0, 0.0), "no difference");
        let balanced: Vec<f64> = (1..=40).map(|i| if i % 2 == 0 { f64::from(i) } else { -f64::from(i) }).collect();
        assert!(wilcoxon(&balanced).0 > 0.5);
        let shifted: Vec<f64> = (1..=40).map(|i| f64::from(i % 7) + 0.5).collect();
        assert!(wilcoxon(&shifted).0 < 1e-6);
    }

    #[test]
    fn the_signed_rank_test_leans_the_way_the_ranks_do_not_the_mean() {
        // all eight better: the positive ranks sum to 36 and the negative to none
        assert_eq!(wilcoxon(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]).1, 36.0);
        // most a little worse, a few much better: the mean goes up, the ranks down
        let d = mostly_worse();
        assert!(d.iter().sum::<f64>() > 0.0);
        let (p, lean) = wilcoxon(&d);
        assert!(p < ALPHA && lean < 0.0, "p {p}, lean {lean}");
    }

    /// 34 items a point worse and 6 twenty points better
    fn mostly_worse() -> Vec<f64> {
        (0..40).map(|i| if i < 34 { -1.0 } else { 20.0 }).collect()
    }

    /// a run's results file in the system's temporary folder, an item a line with its score
    fn results(name: &str, header: &str, scores: &[f64]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("claudius-bench-{}-{name}.jsonl", std::process::id()));
        let mut text = format!("{header}\n");
        for (i, s) in scores.iter().enumerate() {
            text += &format!(
                "{{\"item\": \"i{i}\", \"slice\": {{\"half\": \"{}\"}}, \"metrics\": {{\"score\": {s}}}}}\n",
                i % 2
            );
        }
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn a_few_items_much_better_do_not_carry_a_change_most_items_got_worse_with() {
        let base = results("mostly-worse-base", "", &[10.0; 40]);
        let new = results("mostly-worse-new", "", &mostly_worse().iter().map(|d| 10.0 + d).collect::<Vec<_>>());
        let failed = compare(&base, &new, false).unwrap_err().to_string();
        assert!(failed.contains("score"), "{failed}");
        // and the other way round: most items a little better, a few much worse, is not shown better either
        let new = results("mostly-better-new", "", &mostly_worse().iter().map(|d| 10.0 - d).collect::<Vec<_>>());
        assert!(compare(&base, &new, false).is_err());
        // a change every item is better with passes
        let new = results("all-better-new", "", &[11.0; 40]);
        compare(&base, &new, false).unwrap();
    }

    #[test]
    fn a_comparison_of_nothing_is_an_error_not_a_pass() {
        let empty = results("empty", "", &[]);
        assert!(compare(&empty, &empty, true).is_err(), "no items");
        let base = results("primary-base", r#"{"primary": ["right"]}"#, &[10.0; 4]);
        let new = results("primary-new", r#"{"primary": ["right"]}"#, &[11.0; 4]);
        let missing = compare(&base, &new, true).unwrap_err().to_string();
        assert!(missing.contains("right"), "a primary metric no item has: {missing}");
    }

    #[test]
    fn a_shown_metric_is_compared_and_never_judged() {
        // 30 items: right in 10 more of them, and every total 20 lower, which only shows
        let run = |name: &str, right: usize, total: f64| {
            let path = std::env::temp_dir().join(format!("claudius-bench-{}-{name}.jsonl", std::process::id()));
            let mut text = String::from("{\"primary\": [\"right\"], \"shown\": [\"total\"]}\n");
            for i in 0..30 {
                text +=
                    &format!("{{\"item\": \"i{i}\", \"metrics\": {{\"right\": {}, \"total\": {total}}}}}\n", i < right);
            }
            std::fs::write(&path, text).unwrap();
            path
        };
        let (base, new) = (run("shown-base", 15, 80.0), run("shown-new", 25, 60.0));
        compare(&base, &new, false).unwrap();
        // a metric is judged or shown, not both
        let both = std::env::temp_dir().join(format!("claudius-bench-{}-shown-both.jsonl", std::process::id()));
        std::fs::write(
            &both,
            "{\"primary\": [\"total\"], \"shown\": [\"total\"]}\n{\"item\": \"a\", \"metrics\": {\"total\": 1}}\n",
        )
        .unwrap();
        assert!(compare(&both, &both, true).is_err());
    }

    #[test]
    fn a_metric_an_item_measures_in_neither_run_is_left_out_of_its_pairs() {
        // the songs as they are measure their total, and their flawed copies how they rank: each metric is compared
        // on the items that measure it
        let run = |name: &str, extra: &str| {
            let path = std::env::temp_dir().join(format!("claudius-bench-{}-{name}.jsonl", std::process::id()));
            let mut text = String::from("{\"primary\": [\"ranked\"], \"shown\": [\"total\"]}\n");
            for i in 0..12 {
                text += &format!("{{\"item\": \"s{i}\", \"metrics\": {{\"total\": {}{extra}}}}}\n", 50 + i);
                text += &format!("{{\"item\": \"s{i}/noise\", \"metrics\": {{\"ranked\": {}}}}}\n", i % 3 == 0);
            }
            std::fs::write(&path, text).unwrap();
            path
        };
        let mixed = run("mixed", "");
        compare(&mixed, &mixed, true).unwrap();
        // a metric an item has in one run only is still refused
        let more = run("mixed-more", ", \"ranked\": true");
        let refused = compare(&mixed, &more, true).unwrap_err().to_string();
        assert!(refused.contains("one run only"), "{refused}");
    }

    #[test]
    fn holms_correction_scales_and_keeps_the_order() {
        let a = holm(&[0.01, 0.04, 0.03]);
        assert!((a[0] - 0.03).abs() < 1e-12 && (a[2] - 0.06).abs() < 1e-12 && (a[1] - 0.06).abs() < 1e-12);
    }

    #[test]
    fn the_interval_holds_the_mean_and_is_the_same_every_run() {
        let d: Vec<f64> = (0..200).map(|i| f64::from(i % 5) - 1.0).collect();
        let (lo, hi) = interval(&d);
        assert!(lo < 1.0 && 1.0 < hi && hi - lo < 0.5, "{lo} {hi}");
        assert_eq!(interval(&d), (lo, hi));
    }
}
