//! scipy.signal lfilter / sosfilt and the scipy.ndimage 1-D filters earshot and librosa use.

/// scipy.signal.lfilter(b, a, x) (direct form II transposed, zero initial state)
pub fn lfilter(b: &[f64], a: &[f64], x: &[f64]) -> Vec<f64> {
    let n = b.len().max(a.len());
    let a0 = a[0];
    let mut bb = vec![0.0; n];
    let mut aa = vec![0.0; n];
    for (i, v) in b.iter().enumerate() {
        bb[i] = v / a0;
    }
    for (i, v) in a.iter().enumerate() {
        aa[i] = v / a0;
    }
    let mut z = vec![0.0; n];
    let mut y = Vec::with_capacity(x.len());
    for &xi in x {
        let yi = bb[0] * xi + z[0];
        for k in 1..n {
            z[k - 1] = bb[k] * xi + if k < n - 1 { z[k] } else { 0.0 } - aa[k] * yi;
        }
        y.push(yi);
    }
    y
}

/// scipy.signal.sosfilt(sos, x): cascaded biquads (b0, b1, b2, a0, a1, a2), each direct form II transposed
pub fn sosfilt(sos: &[[f64; 6]], x: &[f64]) -> Vec<f64> {
    let mut y = x.to_vec();
    for s in sos {
        let (b0, b1, b2, a1, a2) = (s[0] / s[3], s[1] / s[3], s[2] / s[3], s[4] / s[3], s[5] / s[3]);
        let (mut z0, mut z1) = (0.0, 0.0);
        for v in y.iter_mut() {
            let xi = *v;
            let yi = b0 * xi + z0;
            z0 = b1 * xi - a1 * yi + z1;
            z1 = b2 * xi - a2 * yi;
            *v = yi;
        }
    }
    y
}

/// index into [0, n) for scipy.ndimage mode 'reflect' (d c b a | a b c d | d c b a)
pub fn reflect_index(i: i64, n: usize) -> usize {
    let n = n as i64;
    if n == 1 {
        return 0;
    }
    let p = 2 * n;
    let mut j = i.rem_euclid(p);
    if j >= n {
        j = p - 1 - j;
    }
    j as usize
}

/// the median (upper middle for even counts) of a window, reordering it
fn median_window(vals: &mut [f64]) -> f64 {
    let n = vals.len();
    let k = n / 2;
    vals.select_nth_unstable_by(k, |a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    vals[k]
}

/// scipy.signal.medfilt(x, size): median with zero padding (rank filter, mode 'constant')
pub fn medfilt(x: &[f64], size: usize) -> Vec<f64> {
    let n = x.len() as i64;
    let h = (size / 2) as i64;
    let mut buf = Vec::with_capacity(size);
    (0..n)
        .map(|i| {
            buf.clear();
            for j in i - h..=i + h {
                buf.push(if j >= 0 && j < n { x[j as usize] } else { 0.0 });
            }
            median_window(&mut buf)
        })
        .collect()
}

/// scipy.ndimage.median_filter(x, size, mode='reflect') on a 1-D sequence (odd size)
pub fn median_filter_reflect(x: &[f64], size: usize) -> Vec<f64> {
    let n = x.len();
    let h = (size / 2) as i64;
    let mut buf = Vec::with_capacity(size);
    (0..n as i64)
        .map(|i| {
            buf.clear();
            for j in i - h..=i + h {
                buf.push(x[reflect_index(j, n)]);
            }
            median_window(&mut buf)
        })
        .collect()
}

/// scipy.ndimage.median_filter(x, size, mode='nearest') on a 1-D sequence (odd size)
pub fn median_filter_nearest(x: &[f64], size: usize) -> Vec<f64> {
    let n = x.len() as i64;
    let h = (size / 2) as i64;
    let mut buf = Vec::with_capacity(size);
    (0..n)
        .map(|i| {
            buf.clear();
            for j in i - h..=i + h {
                buf.push(x[j.clamp(0, n - 1) as usize]);
            }
            median_window(&mut buf)
        })
        .collect()
}

/// scipy.ndimage.maximum_filter1d(x, size, origin=origin, mode='nearest')
pub fn maximum_filter1d_nearest(x: &[f64], size: usize, origin: i64) -> Vec<f64> {
    let n = x.len() as i64;
    let start = -((size / 2) as i64) - origin;
    (0..n)
        .map(|i| {
            let mut m = f64::NEG_INFINITY;
            for k in 0..size as i64 {
                let j = (i + start + k).clamp(0, n - 1);
                m = m.max(x[j as usize]);
            }
            m
        })
        .collect()
}

/// scipy.ndimage.uniform_filter1d(x, size) with the default mode 'reflect'
pub fn uniform_filter1d_reflect(x: &[f64], size: usize) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return vec![];
    }
    let h = (size / 2) as i64;
    // window of output i: [i - h, i - h + size)
    let get = |j: i64| x[reflect_index(j, n)];
    let mut out = Vec::with_capacity(n);
    let mut acc: f64 = (0..size as i64).map(|k| get(k - h)).sum();
    out.push(acc / size as f64);
    for i in 1..n as i64 {
        acc += get(i - h + size as i64 - 1) - get(i - h - 1);
        out.push(acc / size as f64);
    }
    out
}
