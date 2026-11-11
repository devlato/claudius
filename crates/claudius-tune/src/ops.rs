//! The attention softmax as one fused, threaded op with its own backward, and a projection by frozen weights whose
//! backward gives only the input's gradient.
//!
//! candle's CPU elementwise ops run on one thread and its fused softmax has no backward, so the scaled, causally
//! masked softmax over a `[.., rows, keys]` score tensor is done here in one pass per row on the thread pool.

use candle_core::{CpuStorage, CustomOp1, CustomOp2, Device, Layout, Shape, Tensor, WithDType};
use num_traits::Float;
use rayon::prelude::*;

/// `x · w` for a frozen `w` (`[in, out]`), whose backward yields the gradient of `x` only.
///
/// candle's matmul backward also computes, and keeps until the end of `backward`, a full gradient for the weight
/// even when nothing tracks it: for the five projections of every block and the 768×8192 head, that is as much
/// arithmetic as the forward and a weight-sized buffer per projection per example.
pub struct Frozen(pub Tensor);

impl CustomOp1 for Frozen {
    fn name(&self) -> &'static str {
        "frozen-linear"
    }

    fn cpu_fwd(&self, st: &CpuStorage, l: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        let (n, k) = l.shape().dims2()?;
        let x = match st {
            CpuStorage::F32(_) => Tensor::from_slice(contiguous::<f32>(st, l)?, (n, k), &Device::Cpu)?,
            CpuStorage::F64(_) => Tensor::from_slice(contiguous::<f64>(st, l)?, (n, k), &Device::Cpu)?,
            _ => candle_core::bail!("frozen linear: f32 or f64 only"),
        };
        let y = x.matmul(&self.0)?.flatten_all()?;
        let out = match st {
            CpuStorage::F32(_) => CpuStorage::F32(y.to_vec1()?),
            _ => CpuStorage::F64(y.to_vec1()?),
        };
        Ok((out, Shape::from((n, self.0.dim(1)?))))
    }

    fn bwd(&self, _arg: &Tensor, _res: &Tensor, grad: &Tensor) -> candle_core::Result<Option<Tensor>> {
        Ok(Some(grad.matmul(&self.0.t()?)?))
    }
}

/// `softmax(scale * x)` over the last axis, masked to the keys a query may see.
///
/// The columns are absolute positions `0..start + s`. Row `r` is the query at absolute position `start + r % s`, so
/// it sees the columns `lo..=start + r % s`, where `lo` is set by the sliding window when there is one.
#[derive(Clone, Copy, Debug)]
pub struct CausalSoftmax {
    pub s: usize,
    pub start: usize,
    pub window: Option<usize>,
    pub scale: f64,
}

fn contiguous<'a, T: WithDType>(st: &'a CpuStorage, l: &Layout) -> candle_core::Result<&'a [T]> {
    let (a, b) = l
        .contiguous_offsets()
        .ok_or_else(|| candle_core::Error::Msg("causal softmax needs a contiguous input".into()))?;
    Ok(&st.as_slice::<T>()?[a..b])
}

impl CausalSoftmax {
    fn run<T: Float + Send + Sync>(&self, x: &[T], k: usize) -> Vec<T> {
        let scale = T::from(self.scale).unwrap_or_else(T::one);
        let mut out = vec![T::zero(); x.len()];
        out.par_chunks_mut(k).zip(x.par_chunks(k)).enumerate().for_each(|(r, (o, x))| {
            let hi = self.start + r % self.s;
            let lo = self.window.map_or(0, |w| (hi + 1).saturating_sub(w));
            let mut max = T::neg_infinity();
            for &v in &x[lo..=hi] {
                let v = v * scale;
                if v > max {
                    max = v;
                }
            }
            let mut sum = T::zero();
            for c in lo..=hi {
                let e = (x[c] * scale - max).exp();
                o[c] = e;
                sum = sum + e;
            }
            let inv = T::one() / sum;
            for v in &mut o[lo..=hi] {
                *v = *v * inv;
            }
        });
        out
    }
}

impl CustomOp1 for CausalSoftmax {
    fn name(&self) -> &'static str {
        "causal-softmax"
    }

    fn cpu_fwd(&self, st: &CpuStorage, l: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        let k = l.dims().last().copied().unwrap_or(0);
        let out = match st {
            CpuStorage::F32(_) => CpuStorage::F32(self.run(contiguous::<f32>(st, l)?, k)),
            CpuStorage::F64(_) => CpuStorage::F64(self.run(contiguous::<f64>(st, l)?, k)),
            _ => candle_core::bail!("causal softmax: f32 or f64 only"),
        };
        Ok((out, l.shape().clone()))
    }

    fn bwd(&self, _arg: &Tensor, res: &Tensor, grad: &Tensor) -> candle_core::Result<Option<Tensor>> {
        let g = res.contiguous()?.apply_op2_no_bwd(&grad.contiguous()?, &SoftmaxBwd { scale: self.scale })?;
        Ok(Some(g))
    }
}

/// `scale * y * (g - <y, g>)` per row: the gradient of `softmax(scale * x)` given its output `y`. Masked keys have
/// `y = 0`, so they get no gradient without the mask being repeated here.
struct SoftmaxBwd {
    scale: f64,
}

impl SoftmaxBwd {
    fn run<T: Float + Send + Sync>(&self, y: &[T], g: &[T], k: usize) -> Vec<T> {
        let scale = T::from(self.scale).unwrap_or_else(T::one);
        let mut out = vec![T::zero(); y.len()];
        out.par_chunks_mut(k).zip(y.par_chunks(k).zip(g.par_chunks(k))).for_each(|(o, (y, g))| {
            let mut dot = T::zero();
            for c in 0..k {
                dot = dot + y[c] * g[c];
            }
            for c in 0..k {
                o[c] = scale * y[c] * (g[c] - dot);
            }
        });
        out
    }
}

impl CustomOp2 for SoftmaxBwd {
    fn name(&self) -> &'static str {
        "causal-softmax-bwd"
    }

    fn cpu_fwd(
        &self,
        s1: &CpuStorage,
        l1: &Layout,
        s2: &CpuStorage,
        l2: &Layout,
    ) -> candle_core::Result<(CpuStorage, Shape)> {
        let k = l1.dims().last().copied().unwrap_or(0);
        let out = match (s1, s2) {
            (CpuStorage::F32(_), CpuStorage::F32(_)) => {
                CpuStorage::F32(self.run(contiguous::<f32>(s1, l1)?, contiguous::<f32>(s2, l2)?, k))
            }
            (CpuStorage::F64(_), CpuStorage::F64(_)) => {
                CpuStorage::F64(self.run(contiguous::<f64>(s1, l1)?, contiguous::<f64>(s2, l2)?, k))
            }
            _ => candle_core::bail!("causal softmax backward: f32 or f64 only"),
        };
        Ok((out, l1.shape().clone()))
    }
}
