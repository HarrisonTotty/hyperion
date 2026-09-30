//! The temperature scales of the giants (class III) and supergiants (Ib to Ia⁺) against which
//! [`luminosity`](super::luminosity) reads a non-dwarf's subtype (plan 06, P06.T23.b).
//!
//! Each scale is a list of nodes (spectral code, T<sub>eff</sub>), interpolated linearly in log₁₀
//! T<sub>eff</sub> between them, taken from the most recent published calibration for each range of
//! types and joined where they meet; a test checks that each falls strictly with the code, so a
//! subtype is monotone in temperature. The dwarf scale is [`pm13`](super::pm13), which the giant
//! scale meets where giants and dwarfs share one scale.
//!
//! **Giants (III).**
//!
//! - O3–O9.5: Martins, Schaerer and Hillier (2005, A&A 436, 1049), the observational scale of
//!   their Table 5. Pecaut and Mamajek's O dwarfs follow the same paper's Table 4 to within
//!   650 K (O5.5V: 40,500 against 39,865 K), so the two classes are on one system.
//! - B0–A1: the dwarf scale times the ratio of giant to dwarf temperature at the same type in
//!   Zorec et al. (2009, A&A 501, 297, Table 6), whose Balmer-jump calibration measures all the
//!   classes on one system but puts its dwarfs up to 2,300 K from Pecaut and Mamajek's: the
//!   difference between classes is taken from Zorec et al., the level from the dwarf scale.
//! - A2–F5: the dwarf scale itself. No modern calibration exists; Straižys and Kuriliene (1981,
//!   section 3, after Code et al. 1976) find one temperature scale for classes V to III from O to
//!   F5.
//! - F5–G5: interpolated in log₁₀ T<sub>eff</sub> between F5 and G5 III. No calibration covers the
//!   Hertzsprung gap, where giants are few: van Belle et al. (2021) have one star at G0 and one
//!   at G1, and their fit, extended to G0, would put G0 III at 5,219 K, 710 K below the Sun's
//!   class.
//! - G5–M5.5: van Belle et al. (2021, ApJ 922, 163), the piecewise-linear fit of their Table 8
//!   to the interferometric temperatures of 191 giants (Table 7's `TFIT`), at each subtype their
//!   Table 7 lists. It agrees with Richichi et al. (1999) and Ridgway et al. (1980) to about
//!   100 K from K0 to M5.
//! - M6–M9: Richichi et al. (1999, A&A 344, 511, Table 2, the cubic fit to their lunar
//!   occultations), because van Belle et al.'s fit is flat at 3,134 K beyond M6. M9.5 continues
//!   the M8–M9 interval in log₁₀ T<sub>eff</sub>: 2,755 × √(2,755 ÷ 2,940) K.
//!
//! **Supergiants (Ib, Iab, Ia and Ia⁺, one scale).**
//!
//! - O3–O9.5: Martins et al. (2005), Table 6.
//! - B0–B7: Markova and Puls (2008, A&A 478, 823), their equation 1, T<sub>eff</sub> = 27,800 −
//!   6,000x + 878x² − 45.9x³ K with x the B subtype, a fit to their own and four other non-LTE
//!   analyses of Galactic B supergiants (Crowther et al. 2006 among them). The IACOB fit of de
//!   Burgos et al. (2024, A&A 687, A228) is 1,000–1,300 K hotter at B1–B2, above Crowther et al.'s,
//!   Searle et al.'s (2008) and Markova and Puls's alike, and would put B1 and B2 supergiants above
//!   the giants of the same type.
//! - B8–A3: Firnstein and Przybilla (2012, A&A 543, A80), Table 6.
//! - F0–G9: Humphreys and McElroy (1984, ApJ 284, 565), Table 2, the only calibration of Galactic F
//!   and G supergiants; its A5 and A8 lie above Firnstein and Przybilla's A3 and are left out, so
//!   A3–F0 is interpolated. Its scatter is a subtype or two: against the temperatures of Kovtyukh,
//!   Gorlova and Belik (2012, MNRAS, arXiv:1204.4115), α Persei (F5 Ib, 6,541 K) reads F6.5, δ
//!   Canis Majoris (F8 Ia, 6,564 K) F6 and γ Cygni (F8 Iab, 6,188 K) F8, and Polaris (F7 Ib, 6,015
//!   K) reads F8.5.
//! - K1–M5: Levesque et al. (2005, ApJ 628, 973), Table 5, whose bins K1–1.5, K2–3, K5–M0 and
//!   M4–4.5 are placed at K1.25, K2.5, K7 and M4.25.
//! - M5.5–M9.5: no supergiant is calibrated this late, so the scale runs parallel in log₁₀
//!   T<sub>eff</sub> to the giants', at Levesque et al.'s M5 over van Belle et al.'s.
//!
//! A bright giant (II) is given the subtype halfway between the two scales' at its temperature,
//! and a subgiant (IV) the one halfway between the dwarfs' and the giants'.

use super::SpectralLetter;

/// A node of a temperature scale: a spectral code and its effective temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Node {
    /// The spectral code (see [`SpectralCode`](super::SpectralCode)).
    pub(crate) code: f64,
    /// Effective temperature, K.
    pub(crate) teff_k: f64,
}

/// The code of `letter` and `subtype`, as a `const` (`f64::from` is not one).
#[must_use]
const fn code(letter: SpectralLetter, subtype: f64) -> f64 {
    let tens = match letter {
        SpectralLetter::O => 0.0,
        SpectralLetter::B => 10.0,
        SpectralLetter::A => 20.0,
        SpectralLetter::F => 30.0,
        SpectralLetter::G => 40.0,
        SpectralLetter::K => 50.0,
        SpectralLetter::M => 60.0,
        SpectralLetter::L => 70.0,
        SpectralLetter::T => 80.0,
        SpectralLetter::Y => 90.0,
    };
    tens + subtype
}

/// A node at `letter` `subtype` of temperature `teff_k`.
#[must_use]
const fn node(letter: SpectralLetter, subtype: f64, teff_k: f64) -> Node {
    Node {
        code: code(letter, subtype),
        teff_k,
    }
}

/// A B or A giant: the dwarf scale's `dwarf_k` times Zorec et al.'s (2009, Table 6) giant over
/// dwarf temperature at the same type, `zorec_iii_k ÷ zorec_v_k`.
#[must_use]
const fn zorec_giant(
    letter: SpectralLetter,
    subtype: f64,
    dwarf_k: f64,
    zorec_v_k: f64,
    zorec_iii_k: f64,
) -> Node {
    node(letter, subtype, dwarf_k * zorec_iii_k / zorec_v_k)
}

/// A B supergiant from Markova and Puls's (2008) equation 1 in the subtype `x`.
#[must_use]
const fn markova(x: f64) -> Node {
    node(
        SpectralLetter::B,
        x,
        27_800.0 - 6_000.0 * x + 878.0 * x * x - 45.9 * x * x * x,
    )
}

/// van Belle et al.'s (2021, Table 8) M5 III, K: the point from which the supergiant scale runs
/// parallel to the giants'.
const VAN_BELLE_M5_K: f64 = 3_386.0;

/// Levesque et al.'s (2005, Table 5) M5 I, K.
const LEVESQUE_M5_K: f64 = 3_450.0;

/// A late M supergiant: the giant scale's `giant_k` at the same type, times Levesque et al.'s M5
/// over van Belle et al.'s.
#[must_use]
const fn parallel(subtype: f64, giant_k: f64) -> Node {
    node(
        SpectralLetter::M,
        subtype,
        giant_k * LEVESQUE_M5_K / VAN_BELLE_M5_K,
    )
}

/// The giant scale's M9.5, K: M9's 2,755 K times √(2,755 ÷ 2,940), the M8–M9 interval of
/// Richichi et al. (1999) continued in log₁₀ T<sub>eff</sub> (a test checks the arithmetic).
const GIANT_M9_5_K: f64 = 2_666.9;

/// The giant (class III) scale, O3 to M9.5, hottest first (see the [module](self)
/// documentation for each range's source).
pub(crate) const GIANT: [Node; 62] = {
    use SpectralLetter::{A, B, F, G, K, M, O};
    [
        // Martins, Schaerer and Hillier (2005), Table 5.
        node(O, 3.0, 44_537.0),
        node(O, 4.0, 42_422.0),
        node(O, 5.0, 40_307.0),
        node(O, 5.5, 39_249.0),
        node(O, 6.0, 38_192.0),
        node(O, 6.5, 37_134.0),
        node(O, 7.0, 36_077.0),
        node(O, 7.5, 35_019.0),
        node(O, 8.0, 33_961.0),
        node(O, 8.5, 32_904.0),
        node(O, 9.0, 31_846.0),
        node(O, 9.5, 30_789.0),
        // The dwarf scale times Zorec et al. (2009), Table 6: III ÷ V.
        zorec_giant(B, 0.0, 31_400.0, 30_000.0, 29_070.0),
        zorec_giant(B, 1.0, 26_000.0, 27_080.0, 24_880.0),
        zorec_giant(B, 2.0, 20_600.0, 22_620.0, 21_740.0),
        zorec_giant(B, 3.0, 17_000.0, 19_270.0, 18_900.0),
        zorec_giant(B, 4.0, 16_400.0, 17_220.0, 17_310.0),
        zorec_giant(B, 5.0, 15_700.0, 15_380.0, 15_890.0),
        zorec_giant(B, 6.0, 14_500.0, 14_100.0, 14_530.0),
        zorec_giant(B, 7.0, 14_000.0, 13_000.0, 13_460.0),
        zorec_giant(B, 8.0, 12_300.0, 12_190.0, 12_380.0),
        zorec_giant(B, 9.0, 10_700.0, 11_340.0, 11_240.0),
        zorec_giant(A, 0.0, 9_700.0, 10_470.0, 10_350.0),
        zorec_giant(A, 1.0, 9_300.0, 9_860.0, 9_820.0),
        // The dwarf scale, Pecaut and Mamajek (2013) in version 2022.04.16.
        node(A, 2.0, 8_800.0),
        node(A, 3.0, 8_600.0),
        node(A, 4.0, 8_250.0),
        node(A, 5.0, 8_100.0),
        node(A, 6.0, 7_910.0),
        node(A, 7.0, 7_760.0),
        node(A, 8.0, 7_590.0),
        node(A, 9.0, 7_400.0),
        node(F, 0.0, 7_220.0),
        node(F, 1.0, 7_020.0),
        node(F, 2.0, 6_820.0),
        node(F, 3.0, 6_750.0),
        node(F, 4.0, 6_670.0),
        node(F, 5.0, 6_550.0),
        // van Belle et al. (2021), Table 7's TFIT.
        node(G, 5.0, 4_955.0),
        node(G, 8.0, 4_797.0),
        node(K, 0.0, 4_692.0),
        node(K, 1.0, 4_639.0),
        node(K, 1.25, 4_537.0),
        node(K, 1.5, 4_487.0),
        node(K, 2.0, 4_387.0),
        node(K, 3.0, 4_188.0),
        node(K, 3.25, 4_138.0),
        node(K, 3.5, 4_088.0),
        node(K, 4.0, 3_988.0),
        node(K, 5.0, 3_902.0),
        node(M, 0.0, 3_816.0),
        node(M, 1.0, 3_730.0),
        node(M, 2.0, 3_644.0),
        node(M, 3.0, 3_558.0),
        node(M, 4.0, 3_472.0),
        node(M, 5.0, VAN_BELLE_M5_K),
        node(M, 5.5, 3_343.0),
        // Richichi et al. (1999), Table 2, cubic fit; M9.5 continued from M8–M9.
        node(M, 6.0, 3_240.0),
        node(M, 7.0, 3_100.0),
        node(M, 8.0, 2_940.0),
        node(M, 9.0, 2_755.0),
        node(M, 9.5, GIANT_M9_5_K),
    ]
};

/// The supergiant (Ib to Ia⁺) scale, O3 to M9.5, hottest first (see the [module](self)
/// documentation for each range's source).
pub(crate) const SUPERGIANT: [Node; 53] = {
    use SpectralLetter::{A, B, F, G, K, M, O};
    [
        // Martins, Schaerer and Hillier (2005), Table 6.
        node(O, 3.0, 42_233.0),
        node(O, 4.0, 40_422.0),
        node(O, 5.0, 38_612.0),
        node(O, 5.5, 37_706.0),
        node(O, 6.0, 36_801.0),
        node(O, 6.5, 35_895.0),
        node(O, 7.0, 34_990.0),
        node(O, 7.5, 34_084.0),
        node(O, 8.0, 33_179.0),
        node(O, 8.5, 32_274.0),
        node(O, 9.0, 31_368.0),
        node(O, 9.5, 30_463.0),
        // Markova and Puls (2008), equation 1.
        markova(0.0),
        markova(1.0),
        markova(2.0),
        markova(3.0),
        markova(4.0),
        markova(5.0),
        markova(6.0),
        markova(7.0),
        // Firnstein and Przybilla (2012), Table 6.
        node(B, 8.0, 12_200.0),
        node(B, 9.0, 10_920.0),
        node(A, 0.0, 9_840.0),
        node(A, 1.0, 9_240.0),
        node(A, 2.0, 8_960.0),
        node(A, 3.0, 8_430.0),
        // Humphreys and McElroy (1984), Table 2.
        node(F, 0.0, 7_800.0),
        node(F, 5.0, 7_000.0),
        node(F, 6.0, 6_600.0),
        node(F, 8.0, 6_200.0),
        node(G, 0.0, 5_500.0),
        node(G, 2.0, 5_100.0),
        node(G, 3.0, 5_000.0),
        node(G, 6.0, 4_750.0),
        node(G, 9.0, 4_500.0),
        // Levesque et al. (2005), Table 5.
        node(K, 1.25, 4_100.0),
        node(K, 2.5, 4_015.0),
        node(K, 7.0, 3_840.0),
        node(M, 0.0, 3_790.0),
        node(M, 1.0, 3_745.0),
        node(M, 1.5, 3_710.0),
        node(M, 2.0, 3_660.0),
        node(M, 2.5, 3_615.0),
        node(M, 3.0, 3_605.0),
        node(M, 3.5, 3_550.0),
        node(M, 4.25, 3_535.0),
        node(M, 5.0, LEVESQUE_M5_K),
        // Parallel to the giants beyond M5.
        parallel(5.5, 3_343.0),
        parallel(6.0, 3_240.0),
        parallel(7.0, 3_100.0),
        parallel(8.0, 2_940.0),
        parallel(9.0, 2_755.0),
        parallel(9.5, GIANT_M9_5_K),
    ]
};

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::super::pm13::DWARF_SEQUENCE;
    use super::*;

    /// The node of `nodes` at `code`.
    fn at(nodes: &[Node], code: f64) -> f64 {
        nodes
            .iter()
            .find(|n| bits(n.code) == bits(code))
            .map(|n| n.teff_k)
            .unwrap()
    }

    /// The dwarf table's temperature at `code`.
    fn dwarf(code: f64) -> f64 {
        DWARF_SEQUENCE
            .iter()
            .find(|r| bits(r.code()) == bits(code))
            .unwrap()
            .teff_k
    }

    /// Temperature falls strictly and the code rises strictly along each scale, from O3 to M9.5.
    #[test]
    fn each_scale_falls_strictly_from_o3_to_m9_5() {
        for nodes in [GIANT.as_slice(), SUPERGIANT.as_slice()] {
            for pair in nodes.windows(2) {
                assert!(pair[0].code < pair[1].code, "{pair:?}");
                assert!(pair[0].teff_k > pair[1].teff_k, "{pair:?}");
            }
            assert_eq!(bits(nodes[0].code), bits(3.0));
            assert_eq!(bits(nodes[nodes.len() - 1].code), bits(69.5));
        }
    }

    /// The A2–F5 giants are the dwarf table's rows, digit for digit, and the B0–A1 giants take
    /// the dwarf table's rows as their level.
    #[test]
    fn the_a_and_f_giants_are_on_the_dwarf_scale() {
        let shared: Vec<&Node> = GIANT
            .iter()
            .filter(|n| (22.0..=35.0).contains(&n.code))
            .collect();
        assert_eq!(shared.len(), 14);
        for n in shared {
            assert_eq!(bits(dwarf(n.code)), bits(n.teff_k), "{n:?}");
        }
        for n in GIANT.iter().filter(|n| (10.0..=21.0).contains(&n.code)) {
            let ratio = n.teff_k / dwarf(n.code);
            assert!((0.91..1.04).contains(&ratio), "{n:?}");
        }
    }

    /// The computed nodes are the published formulae's values.
    #[test]
    fn the_fitted_nodes_are_the_papers_numbers() {
        // Markova and Puls's equation 1, as computed by hand.
        let markova = [
            27_800.0, 22_632.1, 18_944.8, 16_462.7, 14_910.4, 14_012.5, 13_493.6, 13_078.3,
        ];
        for (x, expected) in (0..8).map(f64::from).zip(markova) {
            let t = at(&SUPERGIANT, 10.0 + x);
            assert!((t - expected).abs() < 1e-6, "B{x}: {t}");
        }
        // Zorec's B0 giant: 31,400 × 29,070 ÷ 30,000.
        assert!((at(&GIANT, 10.0) - 30_426.6).abs() < 1e-6);
        let m9_5 = 2_755.0 * (2_755.0_f64 / 2_940.0).sqrt();
        assert!((GIANT_M9_5_K - m9_5).abs() < 0.05, "{m9_5}");
        let ratio = LEVESQUE_M5_K / VAN_BELLE_M5_K;
        for code in [65.5, 66.0, 67.0, 68.0, 69.0, 69.5] {
            let expected = at(&GIANT, code) * ratio;
            assert!((at(&SUPERGIANT, code) - expected).abs() < 1e-9, "{code}");
        }
    }

    /// Giants are cooler than dwarfs of the same type from G to early K and hotter from M1 on,
    /// and B and early M supergiants cooler than giants, as the calibrations find (A–F and M3
    /// supergiants are the hotter, in them too).
    #[test]
    fn the_scales_order_as_the_calibrations_do() {
        for code in [45.0, 48.0, 50.0, 52.0, 55.0] {
            assert!(at(&GIANT, code) < dwarf(code), "giant at {code}");
        }
        for code in [61.0, 62.0, 63.0, 64.0, 65.0] {
            assert!(at(&GIANT, code) > dwarf(code), "giant at {code}");
        }
        for code in [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 60.0] {
            assert!(
                at(&SUPERGIANT, code) < at(&GIANT, code),
                "supergiant at {code}"
            );
        }
    }
}
