//! Whether the calling thread's floating-point mode flushes subnormals to zero.
//!
//! Rust never turns flushing on, on either target, but a native library can: it may write the
//! control register itself, or, built with fast-math by a toolchain older than GCC 13 (or by Clang
//! with `-mdaz-ftz`), link the startup code `crtfastmath.o` that sets the flags when the library
//! loads, and a thread inherits the mode of the thread that created it (C11 §7.6). Under such a
//! mode generation would silently differ from every other machine's, so the server checks rather
//! than trusts (the rendering brainstorm's "Determinism hazards specific to terrain", bullet
//! "Flush-to-zero from outside"; plan R04, design note 15).
//!
//! Reading the control register (x86-64's MXCSR, `AArch64`'s FPCR) needs `unsafe`, which the
//! workspace forbids, so [`probe_flush_to_zero`] watches what the arithmetic does instead. It
//! detects:
//!
//! - **Output flushing**: MXCSR's FTZ, bit 15 (Intel SDM vol. 1 §10.2.3.3), and the FPCR's FZ, bit
//!   24 (Arm ARM, FPCR). A subnormal result is replaced by zero.
//! - **Input flushing**: MXCSR's DAZ, bit 6 (Intel SDM vol. 1 §10.2.3.4); on `AArch64`, FZ again
//!   while the FPCR's AH (bit 1) is clear, or FIZ (bit 0) under `FEAT_AFP`. A subnormal operand is
//!   read as zero.

use std::fmt;
use std::hint::black_box;

/// What [`probe_flush_to_zero`] saw the thread's floating-point mode do with subnormals.
///
/// Only [`FlushProbe::KEEPS_SUBNORMALS`] is fit to generate on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlushProbe {
    flushes_outputs: bool,
    flushes_inputs: bool,
}

impl FlushProbe {
    /// IEEE 754 arithmetic, as Rust and WebAssembly require: subnormals are kept.
    pub const KEEPS_SUBNORMALS: Self = Self {
        flushes_outputs: false,
        flushes_inputs: false,
    };
    /// Subnormal results become zero (x86-64's FTZ).
    pub const FLUSHES_OUTPUTS: Self = Self {
        flushes_outputs: true,
        flushes_inputs: false,
    };
    /// Subnormal operands are read as zero (x86-64's DAZ).
    pub const FLUSHES_INPUTS: Self = Self {
        flushes_outputs: false,
        flushes_inputs: true,
    };
    /// Both (x86-64's FTZ and DAZ together, or `AArch64`'s FZ with AH clear).
    pub const FLUSHES_BOTH: Self = Self {
        flushes_outputs: true,
        flushes_inputs: true,
    };

    /// Whether a subnormal result was replaced by zero.
    #[must_use]
    pub const fn flushes_outputs(self) -> bool {
        self.flushes_outputs
    }

    /// Whether a subnormal operand was read as zero.
    #[must_use]
    pub const fn flushes_inputs(self) -> bool {
        self.flushes_inputs
    }

    /// Whether the mode departs from IEEE 754 in either way, so that the thread must not generate.
    #[must_use]
    pub const fn flushes(self) -> bool {
        self.flushes_outputs || self.flushes_inputs
    }
}

impl fmt::Display for FlushProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match (self.flushes_outputs, self.flushes_inputs) {
            (false, false) => "keeps subnormals",
            (true, false) => "flushes subnormal results to zero",
            (false, true) => "reads subnormal operands as zero",
            (true, true) => {
                "flushes subnormal results to zero and reads subnormal operands as zero"
            }
        })
    }
}

/// 2⁵², which takes the smallest positive subnormal `f64` (2⁻¹⁰⁷⁴) to the smallest normal one.
const F64_SUBNORMAL_TO_NORMAL: f64 = 4_503_599_627_370_496.0;
/// 2²³, which takes the smallest positive subnormal `f32` (2⁻¹⁴⁹) to the smallest normal one.
const F32_SUBNORMAL_TO_NORMAL: f32 = 8_388_608.0;

/// Probes the calling thread's floating-point mode for flushed subnormals, in `f64` and `f32`.
///
/// Four multiplications, each exact under IEEE 754 and each operand behind [`black_box`] so that
/// it is computed at run time, on this thread, rather than folded by the compiler:
///
/// - `f64::MIN_POSITIVE * 0.5` and `f32::MIN_POSITIVE * 0.5` have subnormal results, which output
///   flushing replaces by zero.
/// - The smallest subnormal times 2⁵² (`f64`) or 2²³ (`f32`) has a normal result from a subnormal
///   operand, which input flushing turns into zero.
///
/// Each result's bits are compared with zero rather than the result with `0.0`: under input
/// flushing alone the comparison would read the subnormal result of the first pair as zero, and
/// the two modes could not be told apart (design note 15). The two widths share their control bits
/// on both architectures, so each pair is redundant with the other; both are kept because the
/// brainstorm asks for both and they cost nothing.
///
/// Not inlined, so that each call is a fresh run of the same instructions on the calling thread.
/// It takes about 1.6 µs without flushing (measured under shared load, provisional), on the slow
/// path the hardware takes for subnormals.
#[inline(never)]
#[must_use]
pub fn probe_flush_to_zero() -> FlushProbe {
    let f64_output = black_box(f64::MIN_POSITIVE) * black_box(0.5);
    let f32_output = black_box(f32::MIN_POSITIVE) * black_box(0.5);
    let f64_input = black_box(f64::from_bits(1)) * black_box(F64_SUBNORMAL_TO_NORMAL);
    let f32_input = black_box(f32::from_bits(1)) * black_box(F32_SUBNORMAL_TO_NORMAL);
    FlushProbe {
        flushes_outputs: f64_output.to_bits() == 0 || f32_output.to_bits() == 0,
        flushes_inputs: f64_input.to_bits() == 0 || f32_input.to_bits() == 0,
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn the_probe_passes_on_a_default_thread() {
        assert_eq!(probe_flush_to_zero(), FlushProbe::KEEPS_SUBNORMALS);
    }

    #[test]
    fn the_probe_passes_on_a_spawned_thread() {
        let probe = thread::spawn(probe_flush_to_zero).join().unwrap();
        assert_eq!(probe, FlushProbe::KEEPS_SUBNORMALS);
    }

    #[test]
    fn the_scale_factors_take_the_smallest_subnormal_to_the_smallest_normal() {
        // Exact powers of two, checked by their bits: the product is MIN_POSITIVE.
        assert_eq!(
            (f64::from_bits(1) * F64_SUBNORMAL_TO_NORMAL).to_bits(),
            f64::MIN_POSITIVE.to_bits()
        );
        assert_eq!(
            (f32::from_bits(1) * F32_SUBNORMAL_TO_NORMAL).to_bits(),
            f32::MIN_POSITIVE.to_bits()
        );
        assert_eq!(F64_SUBNORMAL_TO_NORMAL.to_bits(), 0x4330_0000_0000_0000);
        assert_eq!(F32_SUBNORMAL_TO_NORMAL.to_bits(), 0x4B00_0000);
    }

    #[test]
    fn each_mode_says_what_it_flushes() {
        let modes = [
            FlushProbe::KEEPS_SUBNORMALS,
            FlushProbe::FLUSHES_OUTPUTS,
            FlushProbe::FLUSHES_INPUTS,
            FlushProbe::FLUSHES_BOTH,
        ];
        assert_eq!(
            modes.map(|mode| (
                mode.flushes_outputs(),
                mode.flushes_inputs(),
                mode.flushes()
            )),
            [
                (false, false, false),
                (true, false, true),
                (false, true, true),
                (true, true, true)
            ]
        );
        assert_eq!(
            modes.map(|mode| mode.to_string()),
            [
                "keeps subnormals",
                "flushes subnormal results to zero",
                "reads subnormal operands as zero",
                "flushes subnormal results to zero and reads subnormal operands as zero",
            ]
        );
        assert_eq!(FlushProbe::default(), FlushProbe::KEEPS_SUBNORMALS);
    }
}
