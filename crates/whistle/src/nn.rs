//! Numerics shared by the encoder and the decoder: the reference's 8-bit
//! activation fake-quantisation and batched Cactus-Quants projections.

use needle_core::cq::CqWeight;
use needle_core::norm::zc_rms_norm_vec;

/// Rows per batched projection, the chunk needle-core's own prefill uses.
pub const CHUNK: usize = 64;

/// Which deployment numerics to run. Upstream's quantised reference
/// (`configure_deploy(act_bits=8, kv_bits=8)`) fake-quantises every matmul
/// input to int8 per row and stores attention keys and values at int8 per head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Numerics {
    pub act8: bool,
    pub kv8: bool,
}

impl Default for Numerics {
    fn default() -> Self {
        Self { act8: true, kv8: true }
    }
}

/// `fake_quant(x, x.len(), 8)`: symmetric int8 over the whole row. `jnp.round`
/// rounds half to even.
pub fn a8(x: &mut [f32]) {
    let mut absmax = 0.0f32;
    for &v in x.iter() {
        let a = v.abs();
        if a > absmax {
            absmax = a;
        }
    }
    let scale = if absmax > 0.0 { absmax / 127.0 } else { 1.0 };
    for v in x.iter_mut() {
        *v = (*v / scale).round_ties_even().clamp(-128.0, 127.0) * scale;
    }
}

/// [`a8`] over every `width`-long row of `x`, when `on`.
pub fn a8_rows(x: &mut [f32], width: usize, on: bool) {
    if on {
        for r in x.chunks_exact_mut(width) {
            a8(r);
        }
    }
}

/// ZCRMSNorm over every row of `x`, rows as wide as `scale`.
pub fn zc_rows(x: &mut [f32], scale: &[f32]) {
    for r in x.chunks_exact_mut(scale.len()) {
        zc_rms_norm_vec(r, scale);
    }
}

/// Rotate `rows` activations into `w`'s prepared domain, back to back. Every
/// weight with the same `in_feat` and group can reuse the result.
pub fn prepare(w: &CqWeight, x: &[f32], rows: usize) -> Vec<f32> {
    let p = w.prepared_len();
    let mut xh = vec![0.0f32; rows * p];
    for r in 0..rows {
        w.prepare_input(&x[r * w.in_feat..(r + 1) * w.in_feat], &mut xh[r * p..(r + 1) * p]);
    }
    xh
}

/// `out[r] = W[row_start .. row_start + n] · xh[r]` for every prepared row.
pub fn matmul(w: &CqWeight, xh: &[f32], rows: usize, row_start: usize, n: usize, out: &mut [f32]) {
    let p = w.prepared_len();
    let mut acc = vec![0.0f32; CHUNK];
    let mut r0 = 0;
    while r0 < rows {
        let b = (rows - r0).min(CHUNK);
        w.matmul_rows_prepared(&xh[r0 * p..(r0 + b) * p], b, row_start, n, &mut out[r0 * n..(r0 + b) * n], &mut acc);
        r0 += b;
    }
}

/// All of `W · xh[r]`, `(rows, out_feat)`.
pub fn project(w: &CqWeight, xh: &[f32], rows: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; rows * w.out_feat];
    matmul(w, xh, rows, 0, w.out_feat, &mut out);
    out
}

/// Index of the largest value; the first one on ties.
pub fn argmax(x: &[f32]) -> usize {
    let mut best = 0;
    for (i, &v) in x.iter().enumerate() {
        if v > x[best] {
            best = i;
        }
    }
    best
}

/// `log_softmax` of `x`, in f64 for the normaliser.
pub fn log_softmax(x: &[f32]) -> Vec<f32> {
    let max = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let sum: f64 = x.iter().map(|&v| ((v - max) as f64).exp()).sum();
    let lse = max as f64 + sum.ln();
    x.iter().map(|&v| (v as f64 - lse) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a8_keeps_the_maximum_and_rounds_half_to_even() {
        let mut x = [127.0f32, 0.5, 1.5, -2.5, -127.0];
        a8(&mut x);
        assert_eq!(x, [127.0, 0.0, 2.0, -2.0, -127.0]);
    }

    #[test]
    fn a8_leaves_zeros_alone() {
        let mut x = [0.0f32; 4];
        a8(&mut x);
        assert_eq!(x, [0.0; 4]);
    }
}
