//! Real FFTs with numpy's conventions (unnormalised forward, 1/n inverse), plans cached per thread.

use std::cell::RefCell;
use std::sync::Arc;

pub use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

pub type C64 = Complex<f64>;

thread_local! {
    static PLANNER: RefCell<RealFftPlanner<f64>> = RefCell::new(RealFftPlanner::new());
}

/// a reusable forward real FFT of fixed length (input buffer is clobbered)
pub struct Rfft {
    plan: Arc<dyn RealToComplex<f64>>,
    scratch: Vec<C64>,
    pub n: usize,
}

impl Rfft {
    pub fn new(n: usize) -> Rfft {
        let plan = PLANNER.with(|p| p.borrow_mut().plan_fft_forward(n));
        let scratch = plan.make_scratch_vec();
        Rfft { plan, scratch, n }
    }

    pub fn bins(&self) -> usize {
        self.n / 2 + 1
    }

    /// `input` (length n) is used as scratch space; `out` has n / 2 + 1 bins
    pub fn process(&mut self, input: &mut [f64], out: &mut [C64]) {
        self.plan.process_with_scratch(input, out, &mut self.scratch).expect("rfft length mismatch");
    }
}

/// a reusable inverse real FFT (numpy.fft.irfft: 1/n scaling, imaginary parts of DC / Nyquist ignored)
pub struct Irfft {
    plan: Arc<dyn ComplexToReal<f64>>,
    scratch: Vec<C64>,
    buf: Vec<C64>,
    pub n: usize,
}

impl Irfft {
    pub fn new(n: usize) -> Irfft {
        let plan = PLANNER.with(|p| p.borrow_mut().plan_fft_inverse(n));
        let scratch = plan.make_scratch_vec();
        let buf = plan.make_input_vec();
        Irfft { plan, scratch, buf, n }
    }

    pub fn process(&mut self, spec: &[C64], out: &mut [f64]) {
        let m = spec.len().min(self.buf.len());
        self.buf[..m].copy_from_slice(&spec[..m]);
        for v in &mut self.buf[m..] {
            *v = C64::new(0.0, 0.0);
        }
        self.buf[0].im = 0.0;
        if self.n.is_multiple_of(2) {
            let l = self.buf.len();
            self.buf[l - 1].im = 0.0;
        }
        self.plan.process_with_scratch(&mut self.buf, out, &mut self.scratch).expect("irfft length mismatch");
        let s = 1.0 / self.n as f64;
        for v in out.iter_mut() {
            *v *= s;
        }
    }
}

/// numpy.fft.rfft(x, n)
pub fn rfft(x: &[f64], n: usize) -> Vec<C64> {
    let mut f = Rfft::new(n);
    let mut buf = vec![0.0; n];
    let m = x.len().min(n);
    buf[..m].copy_from_slice(&x[..m]);
    let mut out = vec![C64::new(0.0, 0.0); f.bins()];
    f.process(&mut buf, &mut out);
    out
}

/// numpy.fft.irfft(spec, n)
pub fn irfft(spec: &[C64], n: usize) -> Vec<f64> {
    let mut f = Irfft::new(n);
    let mut out = vec![0.0; n];
    f.process(spec, &mut out);
    out
}

/// scipy.fft.next_fast_len(n, real=True): smallest 2^a 3^b 5^c >= n
pub fn next_fast_len(n: usize) -> usize {
    if n <= 6 {
        return n.max(1);
    }
    let mut best = usize::MAX;
    let mut p5 = 1usize;
    while p5 < best {
        let mut p35 = p5;
        while p35 < best {
            let mut p = p35;
            while p < n {
                p *= 2;
            }
            if p < best {
                best = p;
            }
            if p35 >= n {
                break;
            }
            p35 *= 3;
        }
        if p5 >= n {
            break;
        }
        p5 *= 5;
    }
    best
}
