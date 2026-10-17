//! Print the input / output names, types and shapes of ONNX models (dev tool).
//!
//!     cargo run --release --example inspect_onnx -- path/to/model.onnx [...]

use earshot as _;

fn ox<T, R>(r: Result<T, ort::Error<R>>) -> anyhow::Result<T> {
    r.map_err(|e| anyhow::anyhow!("{}", e.message()))
}

fn main() -> anyhow::Result<()> {
    let env = ox(ort::init().with_name("inspect").build())?;
    for p in std::env::args().skip(1) {
        let t = std::time::Instant::now();
        let b = ox(ort::session::Session::builder(&env))?;
        let mut b = ox(b.with_intra_threads(4))?;
        let s = ox(b.commit_from_file(&p))?;
        println!("{p} (loaded in {:.2} s)", t.elapsed().as_secs_f64());
        for i in s.inputs() {
            println!("  in  {:<24} {:?}", i.name(), i.dtype());
        }
        for o in s.outputs() {
            println!("  out {:<24} {:?}", o.name(), o.dtype());
        }
    }
    Ok(())
}
