//! The mean dwarf sequence of Pecaut and Mamajek (2013, ApJS 208, 9, Table 5) in its maintained
//! extension to O3 and to the L, T and Y dwarfs (plan 06, P06.T23.a).
//!
//! **Source and version.** E. Mamajek, "A Modern Mean Dwarf Stellar Color and Effective Temperature
//! Sequence", <https://www.pas.rochester.edu/~emamajek/EEM_dwarf_UBVIJHK_colors_Teff.txt>,
//! **version 2022.04.16**, downloaded 2026-09-23 (SHA-256
//! `1de2edeec17bb3346e0e4e70b999de5ee29947df38474e64cddb7cfacc164b7f`). Its header asks that Pecaut
//! and Mamajek (2013) be cited for it until an updated version is published: their Table 5
//! (O9V–M9V; the CDS catalogue `J/ApJS/208/9`, `table5.dat`) is the table's published core, and the
//! file extends it to O3–O8.5 and to L0–Y4 and revises it since. The 2022 values differ from Table
//! 5's by up to 700 K at O9V (33,300 against 34,000 K) and 300 K at B1.5V and B4V, 200 K at B8V,
//! and by at most 100 K from B9V on; their bolometric corrections are on the IAU 2015 system
//! (BC<sub>V</sub> = −0.085 at the Sun's 5,772 K, with M<sub>bol</sub>☉ = 4.74), 0.015–0.05 mag
//! above Table 5's from F0 to K5, 0.18 mag at K9V and up to 0.23 mag apart among the M dwarfs. The
//! one table is used whole, so that the scale has no seam between the two.
//!
//! Four of the file's 32 columns are kept, with its numbers as printed: the spectral type,
//! T<sub>eff</sub>, the V-band bolometric correction BC<sub>V</sub> and B − V. The file gives
//! BC<sub>V</sub> down to L5V (1,710 K) and B − V down to M9V (2,380 K); below those a row has
//! none.

use super::SpectralLetter;
use crate::math;

/// One row of the mean dwarf sequence: a class V spectral type, its effective temperature and,
/// where the source gives them, its V-band bolometric correction and B − V colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DwarfRow {
    /// The spectral class letter.
    pub(crate) letter: SpectralLetter,
    /// The subtype within the class, 0–9.5 in the steps the source lists.
    pub(crate) subtype: f64,
    /// Effective temperature, K.
    pub(crate) teff_k: f64,
    /// V-band bolometric correction BC<sub>V</sub> = M<sub>bol</sub> − M<sub>V</sub>, mag, on the
    /// IAU 2015 bolometric scale.
    pub(crate) bc_v: Option<f64>,
    /// Intrinsic Johnson B − V colour, mag.
    pub(crate) colour_b_v: Option<f64>,
}

impl DwarfRow {
    /// The row's place on the spectral sequence as one number (see
    /// [`SpectralCode`](super::SpectralCode)): ten per class from O0 at 0, so G2 is 42.
    #[must_use]
    pub(crate) fn code(&self) -> f64 {
        10.0 * f64::from(self.letter.index()) + self.subtype
    }
}

/// A row of [`DWARF_SEQUENCE`], in the source's column order.
#[must_use]
const fn row(
    letter: SpectralLetter,
    subtype: f64,
    teff_k: f64,
    bc_v: Option<f64>,
    colour_b_v: Option<f64>,
) -> DwarfRow {
    DwarfRow {
        letter,
        subtype,
        teff_k,
        bc_v,
        colour_b_v,
    }
}

/// The mean dwarf sequence from O3V to Y4V, hottest first: every row of the source, 118 in all,
/// with T<sub>eff</sub> strictly falling and the spectral code strictly rising down the table.
#[expect(
    clippy::approx_constant,
    reason = "the source's colours, one of which (O7V's B − V, −0.318) happens to be near 1/π"
)]
pub(crate) const DWARF_SEQUENCE: [DwarfRow; 118] = [
    row(SpectralLetter::O, 3.0, 44_900.0, Some(-4.01), Some(-0.330)), // O3V
    row(SpectralLetter::O, 4.0, 42_900.0, Some(-3.89), Some(-0.326)), // O4V
    row(SpectralLetter::O, 5.0, 41_400.0, Some(-3.76), Some(-0.323)), // O5V
    row(SpectralLetter::O, 5.5, 40_500.0, Some(-3.67), Some(-0.322)), // O5.5V
    row(SpectralLetter::O, 6.0, 39_500.0, Some(-3.57), Some(-0.321)), // O6V
    row(SpectralLetter::O, 6.5, 38_300.0, Some(-3.49), Some(-0.319)), // O6.5V
    row(SpectralLetter::O, 7.0, 37_100.0, Some(-3.41), Some(-0.318)), // O7V
    row(SpectralLetter::O, 7.5, 36_100.0, Some(-3.33), Some(-0.317)), // O7.5V
    row(SpectralLetter::O, 8.0, 35_100.0, Some(-3.24), Some(-0.315)), // O8V
    row(SpectralLetter::O, 8.5, 34_300.0, Some(-3.18), Some(-0.314)), // O8.5V
    row(SpectralLetter::O, 9.0, 33_300.0, Some(-3.11), Some(-0.312)), // O9V
    row(SpectralLetter::O, 9.5, 31_900.0, Some(-3.01), Some(-0.307)), // O9.5V
    row(SpectralLetter::B, 0.0, 31_400.0, Some(-2.99), Some(-0.301)), // B0V
    row(SpectralLetter::B, 0.5, 29_000.0, Some(-2.83), Some(-0.289)), // B0.5V
    row(SpectralLetter::B, 1.0, 26_000.0, Some(-2.58), Some(-0.278)), // B1V
    row(SpectralLetter::B, 1.5, 24_500.0, Some(-2.44), Some(-0.252)), // B1.5V
    row(SpectralLetter::B, 2.0, 20_600.0, Some(-2.03), Some(-0.215)), // B2V
    row(SpectralLetter::B, 2.5, 18_500.0, Some(-1.77), Some(-0.198)), // B2.5V
    row(SpectralLetter::B, 3.0, 17_000.0, Some(-1.54), Some(-0.178)), // B3V
    row(SpectralLetter::B, 4.0, 16_400.0, Some(-1.49), Some(-0.165)), // B4V
    row(SpectralLetter::B, 5.0, 15_700.0, Some(-1.34), Some(-0.156)), // B5V
    row(SpectralLetter::B, 6.0, 14_500.0, Some(-1.13), Some(-0.140)), // B6V
    row(SpectralLetter::B, 7.0, 14_000.0, Some(-1.05), Some(-0.128)), // B7V
    row(SpectralLetter::B, 8.0, 12_300.0, Some(-0.73), Some(-0.109)), // B8V
    row(SpectralLetter::B, 9.0, 10_700.0, Some(-0.42), Some(-0.070)), // B9V
    row(SpectralLetter::B, 9.5, 10_400.0, Some(-0.36), Some(-0.050)), // B9.5V
    row(SpectralLetter::A, 0.0, 9700.0, Some(-0.21), Some(0.000)),    // A0V
    row(SpectralLetter::A, 1.0, 9300.0, Some(-0.14), Some(0.035)),    // A1V
    row(SpectralLetter::A, 2.0, 8800.0, Some(-0.07), Some(0.070)),    // A2V
    row(SpectralLetter::A, 3.0, 8600.0, Some(-0.04), Some(0.100)),    // A3V
    row(SpectralLetter::A, 4.0, 8250.0, Some(-0.02), Some(0.140)),    // A4V
    row(SpectralLetter::A, 5.0, 8100.0, Some(0.00), Some(0.160)),     // A5V
    row(SpectralLetter::A, 6.0, 7910.0, Some(0.005), Some(0.185)),    // A6V
    row(SpectralLetter::A, 7.0, 7760.0, Some(0.01), Some(0.210)),     // A7V
    row(SpectralLetter::A, 8.0, 7590.0, Some(0.02), Some(0.250)),     // A8V
    row(SpectralLetter::A, 9.0, 7400.0, Some(0.02), Some(0.270)),     // A9V
    row(SpectralLetter::F, 0.0, 7220.0, Some(0.01), Some(0.295)),     // F0V
    row(SpectralLetter::F, 1.0, 7020.0, Some(0.005), Some(0.330)),    // F1V
    row(SpectralLetter::F, 2.0, 6820.0, Some(-0.005), Some(0.370)),   // F2V
    row(SpectralLetter::F, 3.0, 6750.0, Some(-0.01), Some(0.390)),    // F3V
    row(SpectralLetter::F, 4.0, 6670.0, Some(-0.015), Some(0.410)),   // F4V
    row(SpectralLetter::F, 5.0, 6550.0, Some(-0.02), Some(0.440)),    // F5V
    row(SpectralLetter::F, 6.0, 6350.0, Some(-0.03), Some(0.486)),    // F6V
    row(SpectralLetter::F, 7.0, 6280.0, Some(-0.035), Some(0.500)),   // F7V
    row(SpectralLetter::F, 8.0, 6180.0, Some(-0.04), Some(0.530)),    // F8V
    row(SpectralLetter::F, 9.0, 6050.0, Some(-0.05), Some(0.560)),    // F9V
    row(SpectralLetter::F, 9.5, 5990.0, Some(-0.06), Some(0.580)),    // F9.5V
    row(SpectralLetter::G, 0.0, 5930.0, Some(-0.065), Some(0.595)),   // G0V
    row(SpectralLetter::G, 1.0, 5860.0, Some(-0.073), Some(0.622)),   // G1V
    row(SpectralLetter::G, 2.0, 5770.0, Some(-0.085), Some(0.650)),   // G2V
    row(SpectralLetter::G, 3.0, 5720.0, Some(-0.095), Some(0.660)),   // G3V
    row(SpectralLetter::G, 4.0, 5680.0, Some(-0.10), Some(0.670)),    // G4V
    row(SpectralLetter::G, 5.0, 5660.0, Some(-0.105), Some(0.680)),   // G5V
    row(SpectralLetter::G, 6.0, 5600.0, Some(-0.115), Some(0.700)),   // G6V
    row(SpectralLetter::G, 7.0, 5550.0, Some(-0.125), Some(0.710)),   // G7V
    row(SpectralLetter::G, 8.0, 5480.0, Some(-0.14), Some(0.730)),    // G8V
    row(SpectralLetter::G, 9.0, 5380.0, Some(-0.16), Some(0.775)),    // G9V
    row(SpectralLetter::K, 0.0, 5270.0, Some(-0.195), Some(0.816)),   // K0V
    row(SpectralLetter::K, 1.0, 5170.0, Some(-0.23), Some(0.857)),    // K1V
    row(SpectralLetter::K, 2.0, 5100.0, Some(-0.26), Some(0.884)),    // K2V
    row(SpectralLetter::K, 3.0, 4830.0, Some(-0.375), Some(0.990)),   // K3V
    row(SpectralLetter::K, 4.0, 4600.0, Some(-0.52), Some(1.090)),    // K4V
    row(SpectralLetter::K, 5.0, 4440.0, Some(-0.63), Some(1.150)),    // K5V
    row(SpectralLetter::K, 6.0, 4300.0, Some(-0.75), Some(1.240)),    // K6V
    row(SpectralLetter::K, 7.0, 4100.0, Some(-0.93), Some(1.340)),    // K7V
    row(SpectralLetter::K, 8.0, 3990.0, Some(-1.03), Some(1.363)),    // K8V
    row(SpectralLetter::K, 9.0, 3930.0, Some(-1.07), Some(1.400)),    // K9V
    row(SpectralLetter::M, 0.0, 3850.0, Some(-1.15), Some(1.420)),    // M0V
    row(SpectralLetter::M, 0.5, 3770.0, Some(-1.29), Some(1.445)),    // M0.5V
    row(SpectralLetter::M, 1.0, 3660.0, Some(-1.42), Some(1.485)),    // M1V
    row(SpectralLetter::M, 1.5, 3620.0, Some(-1.50), Some(1.495)),    // M1.5V
    row(SpectralLetter::M, 2.0, 3560.0, Some(-1.62), Some(1.505)),    // M2V
    row(SpectralLetter::M, 2.5, 3470.0, Some(-1.78), Some(1.522)),    // M2.5V
    row(SpectralLetter::M, 3.0, 3430.0, Some(-1.93), Some(1.53)),     // M3V
    row(SpectralLetter::M, 3.5, 3270.0, Some(-2.28), Some(1.60)),     // M3.5V
    row(SpectralLetter::M, 4.0, 3210.0, Some(-2.51), Some(1.65)),     // M4V
    row(SpectralLetter::M, 4.5, 3110.0, Some(-2.84), Some(1.69)),     // M4.5V
    row(SpectralLetter::M, 5.0, 3060.0, Some(-3.11), Some(1.83)),     // M5V
    row(SpectralLetter::M, 5.5, 2930.0, Some(-3.58), Some(1.94)),     // M5.5V
    row(SpectralLetter::M, 6.0, 2810.0, Some(-4.13), Some(2.01)),     // M6V
    row(SpectralLetter::M, 6.5, 2740.0, Some(-4.62), Some(2.07)),     // M6.5V
    row(SpectralLetter::M, 7.0, 2680.0, Some(-4.99), Some(2.12)),     // M7V
    row(SpectralLetter::M, 7.5, 2630.0, Some(-5.32), Some(2.14)),     // M7.5V
    row(SpectralLetter::M, 8.0, 2570.0, Some(-5.65), Some(2.15)),     // M8V
    row(SpectralLetter::M, 8.5, 2420.0, Some(-5.78), Some(2.16)),     // M8.5V
    row(SpectralLetter::M, 9.0, 2380.0, Some(-5.86), Some(2.17)),     // M9V
    row(SpectralLetter::M, 9.5, 2350.0, Some(-6.13), None),           // M9.5V
    row(SpectralLetter::L, 0.0, 2270.0, Some(-6.25), None),           // L0V
    row(SpectralLetter::L, 1.0, 2160.0, Some(-6.48), None),           // L1V
    row(SpectralLetter::L, 2.0, 2060.0, Some(-6.62), None),           // L2V
    row(SpectralLetter::L, 3.0, 1920.0, Some(-7.05), None),           // L3V
    row(SpectralLetter::L, 4.0, 1870.0, Some(-7.53), None),           // L4V
    row(SpectralLetter::L, 5.0, 1710.0, Some(-7.87), None),           // L5V
    row(SpectralLetter::L, 6.0, 1550.0, None, None),                  // L6V
    row(SpectralLetter::L, 7.0, 1530.0, None, None),                  // L7V
    row(SpectralLetter::L, 8.0, 1420.0, None, None),                  // L8V
    row(SpectralLetter::L, 9.0, 1370.0, None, None),                  // L9V
    row(SpectralLetter::T, 0.0, 1255.0, None, None),                  // T0V
    row(SpectralLetter::T, 1.0, 1240.0, None, None),                  // T1V
    row(SpectralLetter::T, 2.0, 1220.0, None, None),                  // T2V
    row(SpectralLetter::T, 3.0, 1200.0, None, None),                  // T3V
    row(SpectralLetter::T, 4.0, 1180.0, None, None),                  // T4V
    row(SpectralLetter::T, 4.5, 1170.0, None, None),                  // T4.5V
    row(SpectralLetter::T, 5.0, 1160.0, None, None),                  // T5V
    row(SpectralLetter::T, 5.5, 1040.0, None, None),                  // T5.5V
    row(SpectralLetter::T, 6.0, 950.0, None, None),                   // T6V
    row(SpectralLetter::T, 7.0, 825.0, None, None),                   // T7V
    row(SpectralLetter::T, 7.5, 750.0, None, None),                   // T7.5V
    row(SpectralLetter::T, 8.0, 680.0, None, None),                   // T8V
    row(SpectralLetter::T, 8.5, 600.0, None, None),                   // T8.5V
    row(SpectralLetter::T, 9.0, 560.0, None, None),                   // T9V
    row(SpectralLetter::T, 9.5, 510.0, None, None),                   // T9.5V
    row(SpectralLetter::Y, 0.0, 450.0, None, None),                   // Y0V
    row(SpectralLetter::Y, 0.5, 400.0, None, None),                   // Y0.5V
    row(SpectralLetter::Y, 1.0, 360.0, None, None),                   // Y1V
    row(SpectralLetter::Y, 1.5, 325.0, None, None),                   // Y1.5V
    row(SpectralLetter::Y, 2.0, 320.0, None, None),                   // Y2V
    row(SpectralLetter::Y, 4.0, 250.0, None, None),                   // Y4V
];

/// The first row: O3V.
const HOTTEST: DwarfRow = DWARF_SEQUENCE[0];

/// Where an effective temperature falls on the dwarf sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Place {
    /// Hotter than the first row, O3V at 44,900 K.
    Hotter,
    /// At the temperature of the row with this index, exactly.
    At(usize),
    /// Strictly between row `hot` and the next, cooler row, `fraction` of the way from the first
    /// to the second in log₁₀ T<sub>eff</sub>, in (0, 1) up to rounding.
    Between {
        /// The index of the hotter row.
        hot: usize,
        /// The fraction of the interval in log₁₀ T<sub>eff</sub>.
        fraction: f64,
    },
    /// Cooler than the last row, Y4V at 250 K.
    Cooler,
}

/// Where `teff_k` falls on the dwarf sequence, interpolating in log₁₀ T<sub>eff</sub>.
///
/// The caller passes a finite, positive temperature (debug-asserted); a NaN reads as cooler than
/// every row.
#[must_use]
pub(crate) fn place(teff_k: f64) -> Place {
    debug_assert!(
        teff_k.is_finite() && teff_k > 0.0,
        "an effective temperature is finite and positive: {teff_k}"
    );
    if teff_k.is_nan() {
        return Place::Cooler;
    }
    // Rows `..hotter` are strictly hotter than `teff_k`; the next is at or below it.
    let hotter = DWARF_SEQUENCE.partition_point(|r| r.teff_k > teff_k);
    if DWARF_SEQUENCE
        .get(hotter)
        .is_some_and(|r| r.teff_k >= teff_k)
    {
        return Place::At(hotter);
    }
    if hotter == 0 {
        return Place::Hotter;
    }
    if hotter == DWARF_SEQUENCE.len() {
        return Place::Cooler;
    }
    let (hot, cold) = (&DWARF_SEQUENCE[hotter - 1], &DWARF_SEQUENCE[hotter]);
    let log_hot = math::log10(hot.teff_k);
    let fraction = (log_hot - math::log10(teff_k)) / (log_hot - math::log10(cold.teff_k));
    Place::Between {
        hot: hotter - 1,
        fraction,
    }
}

/// The dwarf scale's continuous spectral code at `teff_k`: linear in log₁₀ T<sub>eff</sub> between
/// rows, held at O3 (3) above the table and at Y4 (94) below it.
#[must_use]
pub(crate) fn code_at(teff_k: f64) -> f64 {
    match place(teff_k) {
        Place::Hotter => HOTTEST.code(),
        Place::At(i) => DWARF_SEQUENCE[i].code(),
        Place::Between { hot, fraction } => {
            let (a, b) = (DWARF_SEQUENCE[hot].code(), DWARF_SEQUENCE[hot + 1].code());
            a + fraction * (b - a)
        }
        Place::Cooler => DWARF_SEQUENCE[DWARF_SEQUENCE.len() - 1].code(),
    }
}

/// A column of the table at `teff_k`: a row's own value at its temperature, linear in log₁₀
/// T<sub>eff</sub> between two rows that both give it; `None` where either does not, and beyond
/// either end.
#[must_use]
pub(crate) fn column_at(teff_k: f64, column: impl Fn(&DwarfRow) -> Option<f64>) -> Option<f64> {
    match place(teff_k) {
        Place::At(i) => column(&DWARF_SEQUENCE[i]),
        Place::Between { hot, fraction } => {
            let a = column(&DWARF_SEQUENCE[hot])?;
            let b = column(&DWARF_SEQUENCE[hot + 1])?;
            Some(a + fraction * (b - a))
        }
        Place::Hotter | Place::Cooler => None,
    }
}

/// The hottest row, from which the photometry continues the table to hotter stars.
#[must_use]
pub(crate) const fn hottest() -> DwarfRow {
    HOTTEST
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;

    /// FNV-1a over every field's bits in row order, a missing value hashing as one `0xff` byte, so
    /// that an accidental edit of any digit fails.
    fn checksum(rows: &[DwarfRow]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut eat = |bytes: &[u8]| {
            for &byte in bytes {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        for r in rows {
            eat(&[r.letter.index()]);
            for value in [Some(r.subtype), Some(r.teff_k), r.bc_v, r.colour_b_v] {
                match value {
                    Some(v) => eat(&bits(v).to_le_bytes()),
                    None => eat(&[0xff]),
                }
            }
        }
        hash
    }

    #[test]
    fn the_dwarf_table_is_pinned_by_its_checksum() {
        assert_eq!(checksum(&DWARF_SEQUENCE), DWARF_SEQUENCE_CHECKSUM);
    }

    const DWARF_SEQUENCE_CHECKSUM: u64 = 10_053_943_438_955_035_134;

    #[test]
    fn temperature_falls_and_the_code_rises_down_the_table() {
        for pair in DWARF_SEQUENCE.windows(2) {
            assert!(pair[0].teff_k > pair[1].teff_k, "{pair:?}");
            assert!(pair[0].code() < pair[1].code(), "{pair:?}");
        }
        assert!((DWARF_SEQUENCE[0].code() - 3.0).abs() < 1e-12, "O3V first");
        assert!(
            (DWARF_SEQUENCE[117].code() - 94.0).abs() < 1e-12,
            "Y4V last"
        );
    }

    /// The source's columns end where it says: BC<sub>V</sub> at L5V, B − V at M9V, each without a
    /// gap above.
    #[test]
    fn the_photometric_columns_end_where_the_source_does() {
        let last_correction = DWARF_SEQUENCE
            .iter()
            .rposition(|r| r.bc_v.is_some())
            .unwrap();
        let last_colour = DWARF_SEQUENCE
            .iter()
            .rposition(|r| r.colour_b_v.is_some())
            .unwrap();
        let rows = &DWARF_SEQUENCE;
        assert!(rows[..=last_correction].iter().all(|r| r.bc_v.is_some()));
        assert!(rows[..=last_colour].iter().all(|r| r.colour_b_v.is_some()));
        assert!(rows[last_correction + 1..].iter().all(|r| r.bc_v.is_none()));
        assert!(
            rows[last_colour + 1..]
                .iter()
                .all(|r| r.colour_b_v.is_none())
        );
        let (l5, m9) = (&rows[last_correction], &rows[last_colour]);
        assert_eq!(
            (l5.letter, bits(l5.subtype), bits(l5.teff_k)),
            (SpectralLetter::L, bits(5.0), bits(1_710.0))
        );
        assert_eq!(
            (m9.letter, bits(m9.subtype), bits(m9.teff_k)),
            (SpectralLetter::M, bits(9.0), bits(2_380.0))
        );
    }

    /// Spot checks against the file as printed: the Sun's type, and one row of each class.
    #[test]
    fn rows_read_as_the_source_prints_them() {
        let find = |letter: SpectralLetter, subtype: f64| {
            *DWARF_SEQUENCE
                .iter()
                .find(|r| r.letter == letter && bits(r.subtype) == bits(subtype))
                .unwrap()
        };
        let g2 = find(SpectralLetter::G, 2.0);
        assert_eq!(
            (bits(g2.teff_k), g2.bc_v.map(bits), g2.colour_b_v.map(bits)),
            (bits(5_770.0), Some(bits(-0.085)), Some(bits(0.650)))
        );
        let a0 = find(SpectralLetter::A, 0.0);
        assert_eq!(
            (bits(a0.teff_k), a0.bc_v.map(bits), a0.colour_b_v.map(bits)),
            (bits(9_700.0), Some(bits(-0.21)), Some(bits(0.0)))
        );
        assert_eq!(bits(find(SpectralLetter::O, 9.5).teff_k), bits(31_900.0));
        assert_eq!(bits(find(SpectralLetter::B, 9.5).teff_k), bits(10_400.0));
        assert_eq!(bits(find(SpectralLetter::F, 9.5).teff_k), bits(5_990.0));
        assert_eq!(bits(find(SpectralLetter::K, 9.0).teff_k), bits(3_930.0));
        assert_eq!(bits(find(SpectralLetter::M, 5.0).teff_k), bits(3_060.0));
        assert_eq!(bits(find(SpectralLetter::L, 0.0).teff_k), bits(2_270.0));
        assert_eq!(bits(find(SpectralLetter::T, 6.0).teff_k), bits(950.0));
        assert_eq!(bits(find(SpectralLetter::Y, 0.0).teff_k), bits(450.0));
    }
}
