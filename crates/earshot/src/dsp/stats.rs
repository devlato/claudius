//! numpy-compatible reductions: percentile (linear), median, mean, std, argmax / argsort helpers.

/// numpy.percentile(v, q) with the default 'linear' method (NaN-free input)
pub fn percentile(v: &[f64], q: f64) -> f64 {
    let mut s: Vec<f64> = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    percentile_sorted(&s, q)
}

pub fn percentile_sorted(s: &[f64], q: f64) -> f64 {
    let n = s.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 {
        return s[0];
    }
    let idx = q / 100.0 * (n - 1) as f64;
    let lo = idx.floor().max(0.0) as usize;
    let hi = (lo + 1).min(n - 1);
    let t = idx - lo as f64;
    let (a, b) = (s[lo], s[hi]);
    let d = b - a;
    // numpy's _lerp: a + d * t, computed from the upper end when t >= 0.5
    if t >= 0.5 { b - d * (1.0 - t) } else { a + d * t }
}

/// numpy.median
pub fn median(v: &[f64]) -> f64 {
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    let mut s: Vec<f64> = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 }
}

/// numpy's pairwise summation (the order np.sum / np.mean use for contiguous float arrays)
pub fn pairwise_sum(v: &[f64]) -> f64 {
    const BLOCK: usize = 128;
    let n = v.len();
    if n < 8 {
        let mut r = 0.0;
        for &x in v {
            r += x;
        }
        r
    } else if n <= BLOCK {
        let mut r = [0.0f64; 8];
        r.copy_from_slice(&v[..8]);
        let mut i = 8;
        while i + 8 <= n {
            for k in 0..8 {
                r[k] += v[i + k];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += v[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_sum(&v[..n2]) + pairwise_sum(&v[n2..])
    }
}

/// np.mean with pairwise summation
pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    pairwise_sum(v) / v.len() as f64
}

/// np.std (population, ddof 0)
pub fn std(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let m = mean(v);
    let d: Vec<f64> = v.iter().map(|x| (x - m) * (x - m)).collect();
    (pairwise_sum(&d) / v.len() as f64).sqrt()
}

/// np.std(ddof=1)
pub fn std_ddof1(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = mean(v);
    let d: Vec<f64> = v.iter().map(|x| (x - m) * (x - m)).collect();
    (pairwise_sum(&d) / (v.len() - 1) as f64).sqrt()
}

/// index of the first maximum (np.argmax); NaN-free input
pub fn argmax(v: &[f64]) -> usize {
    let mut best = 0;
    for i in 1..v.len() {
        if v[i] > v[best] {
            best = i;
        }
    }
    best
}

pub fn argmin(v: &[f64]) -> usize {
    let mut best = 0;
    for i in 1..v.len() {
        if v[i] < v[best] {
            best = i;
        }
    }
    best
}

pub fn max(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

pub fn min(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::INFINITY, f64::min)
}

/// np.argsort(-v) with a stable sort (ties keep index order, like numpy's default quicksort usually does for
/// the small arrays involved; exact ties practically never occur in model scores)
pub fn argsort_desc(v: &[f64]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|&a, &b| v[b].partial_cmp(&v[a]).unwrap_or(std::cmp::Ordering::Equal));
    idx
}

/// np.corrcoef(a, b)[0, 1]
pub fn corrcoef(a: &[f64], b: &[f64]) -> f64 {
    let ma = mean(a);
    let mb = mean(b);
    let da: Vec<f64> = a.iter().map(|x| x - ma).collect();
    let db: Vec<f64> = b.iter().map(|x| x - mb).collect();
    let sab: Vec<f64> = da.iter().zip(&db).map(|(x, y)| x * y).collect();
    let saa: Vec<f64> = da.iter().map(|x| x * x).collect();
    let sbb: Vec<f64> = db.iter().map(|x| x * x).collect();
    let c = pairwise_sum(&sab) / (pairwise_sum(&saa).sqrt() * pairwise_sum(&sbb).sqrt());
    c.clamp(-1.0, 1.0)
}
