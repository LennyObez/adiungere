//! A program that carries one symbol from `tools/forbidden-encoder-symbols.txt`, and nothing else.
//!
//! `scripts/check-encoder-symbols.sh` compiles this file and reads its symbols before it reads the
//! product's. A check that has not been seen to find anything is a check that may find nothing, so the
//! fixture has to be refused, and refused for this one function, before the real binary is trusted.
//!
//! The function is kept out of line and its result is observed, so the optimiser cannot fold it away and
//! take the symbol with it.

/// The name of a video encoder's entry point, carried by an ordinary Rust function.
#[inline(never)]
pub fn x264_encoder_open() -> u32 {
    std::hint::black_box(7)
}

fn main() {
    std::process::exit(i32::try_from(x264_encoder_open()).unwrap_or(1));
}
