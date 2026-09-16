//! Generates `N` split modules where every pair of splits shares one string that
//! nothing else references, and neighbouring pairs (index distance of at most two)
//! additionally share one small function. Each pair's symbols are reachable from
//! exactly two splits, so the split analysis creates one output module per pair:
//! `N * (N - 1) / 2` chunks for `N` splits.
//!
//! The strings all live in the read-only data segment. Relocating that segment
//! across so many modules pads it past its input length, so the emitter moves the
//! whole segment into the main module and the string-only pair chunks are left
//! defining nothing. The CLI skips those, leaving the `2 * N - 3` pair chunks with
//! a function plus the chunk shared by all splits: `2 * N - 2` chunk files.
//!
//! `N` is read from `SPARSE_SHARES_SPLITS` (default 12).

use std::{env, fmt::Write, fs, path::PathBuf};

fn count_upper(text: &str) -> usize {
    text.chars().filter(|c| c.is_ascii_uppercase()).count()
}

fn pair_text(i: usize, j: usize) -> String {
    format!(
        "Shared Only By Split {i} And Split {j}: {}",
        "x".repeat(i + j)
    )
}

fn has_pair_fn(i: usize, j: usize) -> bool {
    j - i <= 2
}

fn main() {
    println!("cargo:rerun-if-env-changed=SPARSE_SHARES_SPLITS");
    println!("cargo:rerun-if-changed=build.rs");

    let n: usize = env::var("SPARSE_SHARES_SPLITS")
        .ok()
        .map(|v| v.parse().expect("SPARSE_SHARES_SPLITS must be an integer"))
        .unwrap_or(12);
    assert!(n >= 2, "need at least two splits to share anything");

    let mut out = String::new();
    writeln!(out, "pub const SPLITS: usize = {n};").unwrap();

    for i in 0..n {
        for j in i + 1..n {
            writeln!(out, "static PAIR_{i}_{j}: &str = {:?};", pair_text(i, j)).unwrap();
            if has_pair_fn(i, j) {
                writeln!(
                    out,
                    "#[inline(never)]\nfn pair_fn_{i}_{j}(x: usize) -> usize {{ x.wrapping_add({}) }}",
                    i * n + j
                )
                .unwrap();
            }
        }
    }

    let mut expected = Vec::with_capacity(n);
    for i in 0..n {
        let mut body = Vec::new();
        let mut total = 0;
        for j in (0..n).filter(|&j| j != i) {
            let (a, b) = if i < j { (i, j) } else { (j, i) };
            body.push(format!("crate::count_upper(PAIR_{a}_{b})"));
            total += count_upper(&pair_text(a, b));
            if has_pair_fn(a, b) {
                body.push(format!("pair_fn_{a}_{b}(0)"));
                total += a * n + b;
            }
        }
        expected.push(total);
        writeln!(
            out,
            "#[wasm_split(split_{i})]\n#[allow(unused)]\nfn split_{i}() -> usize {{\n    {}\n}}",
            body.join("\n        + ")
        )
        .unwrap();
    }

    writeln!(out, "pub const EXPECTED: [usize; {n}] = {expected:?};").unwrap();
    let calls: Vec<_> = (0..n).map(|i| format!("split_{i}().await")).collect();
    writeln!(
        out,
        "#[allow(unused)]\nasync fn call_all() -> Vec<usize> {{\n    vec![{}]\n}}",
        calls.join(", ")
    )
    .unwrap();

    let dest = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("generated.rs");
    fs::write(dest, out).unwrap();
}
