//! The block function: Threefry2x64 with 20 rounds.
//!
//! Threefry is the counter-based generator of Salmon, Moraes, Dror and Shaw (2011), _Parallel
//! random numbers: as easy as 1, 2, 3_, SC '11, built from the Threefish block cipher of the Skein
//! hash family. A block maps a two-word key and a two-word counter to two output words, and for a
//! fixed key it is a bijection on the counter. It uses only 64-bit wrapping addition, rotation and
//! exclusive or, so it computes the same bits on every target. This implementation follows
//! Random123's `threefry.h` (D. E. Shaw Research) and is pinned against that distribution's
//! known-answer vectors.

/// The key-schedule parity constant, `SKEIN_KS_PARITY64` in Random123's `threefry.h`: the Skein
/// 1.3 specification's `C240`.
const KEY_SCHEDULE_PARITY: u64 = 0x1BD1_1BDA_A9FC_1A22;

/// The rotation constants of Threefry2x64, `R_64x2_0_0` to `R_64x2_7_0` in Random123's
/// `threefry.h`; round `r` rotates by `ROTATIONS[r mod 8]`.
const ROTATIONS: [u32; 8] = [16, 42, 12, 31, 16, 32, 24, 21];

/// Threefry2x64 with `ROUNDS` rounds, the Random123 construction.
///
/// The key schedule is `[k0, k1, k0 ^ k1 ^ parity]`. The counter words take `ks[0]` and `ks[1]`
/// first; each round is `x0 += x1; x1 = rotl(x1, R[r mod 8]); x1 ^= x0`; and after every fourth
/// round, with `s = (r + 1) ÷ 4`, `x0 += ks[s mod 3]` and `x1 += ks[(s + 1) mod 3] + s`. All
/// additions wrap.
///
/// The rounds run in groups of four, each group followed by its key injection, so that the
/// rotation amounts and schedule indices are constants of an unrolled loop: 10 ns a block,
/// against 23 ns for a flat loop over the twenty rounds (Criterion, `rng/threefry_block`, on the
/// development machine, x86-64).
#[inline]
#[must_use]
fn threefry2x64<const ROUNDS: usize>(key: [u64; 2], counter: [u64; 2]) -> [u64; 2] {
    let schedule = [key[0], key[1], KEY_SCHEDULE_PARITY ^ key[0] ^ key[1]];
    let mut x = [
        counter[0].wrapping_add(schedule[0]),
        counter[1].wrapping_add(schedule[1]),
    ];
    let mut injection = 0_u64;
    let mut s = 0_usize;
    for group in 0..ROUNDS / 4 {
        let rotations = &ROTATIONS[(group % 2) * 4..(group % 2) * 4 + 4];
        for &rotation in rotations {
            mix(&mut x, rotation);
        }
        injection += 1;
        s = if s == 2 { 0 } else { s + 1 };
        x[0] = x[0].wrapping_add(schedule[s]);
        x[1] = x[1]
            .wrapping_add(schedule[if s == 2 { 0 } else { s + 1 }])
            .wrapping_add(injection);
    }
    for round in ROUNDS / 4 * 4..ROUNDS {
        mix(&mut x, ROTATIONS[round % 8]);
    }
    x
}

/// One round: `x0 += x1; x1 = rotl(x1, rotation) ^ x0`.
#[inline]
fn mix(x: &mut [u64; 2], rotation: u32) {
    x[0] = x[0].wrapping_add(x[1]);
    x[1] = x[1].rotate_left(rotation) ^ x[0];
}

/// One block of Threefry2x64-20: two output words for a key and a counter.
///
/// This is the random function every stream is built on (Salmon et al. 2011, Random123
/// `threefry2x64_R(20, ctr, key)`). For a fixed key it is a bijection on the counter, so distinct
/// counters never share an output block. [`Stream`](super::Stream) fixes what the key and counter
/// words hold.
///
/// # Examples
///
/// The first of Random123's known-answer vectors for this variant:
///
/// ```
/// use hyperion_sim::rng::threefry2x64_20;
///
/// assert_eq!(
///     threefry2x64_20([0, 0], [0, 0]),
///     [0xc2b6_e3a8_c2c6_9865, 0x6f81_ed42_f350_084d],
/// );
/// ```
#[inline]
#[must_use]
pub fn threefry2x64_20(key: [u64; 2], counter: [u64; 2]) -> [u64; 2] {
    threefry2x64::<20>(key, counter)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const ONES: u64 = u64::MAX;
    /// The digits of π that Random123 uses for its third vector of each variant.
    const PI_COUNTER: [u64; 2] = [0x243f_6a88_85a3_08d3, 0x1319_8a2e_0370_7344];
    const PI_KEY: [u64; 2] = [0xa409_3822_299f_31d0, 0x082e_fa98_ec4e_6c89];

    /// `(counter, key, expected output)`, the `threefry2x64` lines of `tests/kat_vectors` in the
    /// Random123 distribution (D. E. Shaw Research,
    /// <https://github.com/DEShawResearch/random123>), whose columns are
    /// `name rounds counter key expected`.
    type Vector = ([u64; 2], [u64; 2], [u64; 2]);

    const KAT_13: [Vector; 3] = [
        (
            [0, 0],
            [0, 0],
            [0xf167_b032_c3b4_80bd, 0xe91f_9fee_4b7a_6fb5],
        ),
        (
            [ONES, ONES],
            [ONES, ONES],
            [0xccde_c5c9_17a8_74b1, 0x4df5_3abc_a26c_eb01],
        ),
        (
            PI_COUNTER,
            PI_KEY,
            [0xc3aa_c715_6104_2993, 0x3fe7_ae88_01af_f316],
        ),
    ];

    const KAT_20: [Vector; 3] = [
        (
            [0, 0],
            [0, 0],
            [0xc2b6_e3a8_c2c6_9865, 0x6f81_ed42_f350_084d],
        ),
        (
            [ONES, ONES],
            [ONES, ONES],
            [0xe02c_b7c4_d95d_277a, 0xd066_33d0_893b_8b68],
        ),
        (
            PI_COUNTER,
            PI_KEY,
            [0x263c_7d30_bb0f_0af1, 0x56be_8361_d331_1526],
        ),
    ];

    const KAT_32: [Vector; 3] = [
        (
            [0, 0],
            [0, 0],
            [0x38ba_854d_7f13_cfb3, 0xd02f_ca72_9d54_fadc],
        ),
        (
            [ONES, ONES],
            [ONES, ONES],
            [0x6b53_2f4f_6e28_8646, 0x0388_f1ec_135e_e18e],
        ),
        (
            PI_COUNTER,
            PI_KEY,
            [0xdad4_92f3_2efb_d0c4, 0xb6d7_d0cd_1f19_3e84],
        ),
    ];

    #[test]
    fn matches_random123_known_answers_at_20_rounds() {
        for (counter, key, expected) in KAT_20 {
            assert_eq!(
                threefry2x64_20(key, counter),
                expected,
                "counter {counter:x?}, key {key:x?}"
            );
        }
    }

    /// The 13- and 32-round vectors exercise the same rotation table and key schedule at other
    /// depths, so a wrong injection after round 20 or a wrong constant cannot hide.
    #[test]
    fn matches_random123_known_answers_at_13_and_32_rounds() {
        for (counter, key, expected) in KAT_13 {
            assert_eq!(threefry2x64::<13>(key, counter), expected, "13 rounds");
        }
        for (counter, key, expected) in KAT_32 {
            assert_eq!(threefry2x64::<32>(key, counter), expected, "32 rounds");
        }
    }

    /// A spot check of the bijection: consecutive counters under one key give distinct blocks,
    /// and distinct first words too.
    #[test]
    fn consecutive_counters_give_distinct_outputs() {
        let key = [0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210];
        let mut blocks = BTreeSet::new();
        let mut words = BTreeSet::new();
        for n in 0..100_000_u64 {
            let out = threefry2x64_20(key, [n, 0]);
            assert!(blocks.insert(out), "counter {n} repeats a block");
            assert!(words.insert(out[0]), "counter {n} repeats a first word");
        }
    }
}
