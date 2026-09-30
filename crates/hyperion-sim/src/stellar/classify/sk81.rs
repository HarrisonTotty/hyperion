//! The calibration of MK types in temperature, bolometric magnitude and surface gravity of
//! Straižys and Kuriliene (1981, Astrophysics and Space Science 80, 353), from which the
//! luminosity classes' boundaries are drawn (plan 06, P06.T23.b).
//!
//! Three of the paper's tables, as printed (ADS scan `1981Ap&SS..80..353S`, pages 357, 358 and
//! 362), one row per spectral type from O5 to M6:
//!
//! - Table III, log₁₀ T<sub>eff</sub> for classes V, III and I–II. From O5 to F5 the paper prints
//!   one value for V and III (section 3: "the temperature scale to be the same for the ZAMS and
//!   luminosity V, IV, and III stars of spectral classes from O to F5"), which both columns here
//!   repeat; one scale serves every supergiant class and II.
//! - Table IV, the absolute bolometric magnitude M<sub>bol</sub> for classes V, IV, III, II, Ib,
//!   Iab and Ia, on the paper's zero point M<sub>bol</sub>☉ = +4.72 (section 3).
//! - Table VII, log₁₀ g (cm s⁻²) for the same seven classes, which the paper computes as log g =
//!   log 𝔐 + 4 log T<sub>eff</sub> + 0.4 M<sub>bol</sub> − 12.49 from the masses of evolutionary
//!   tracks (Table VI). A test recomputes every entry from Tables III, IV and VI and finds them
//!   within 0.045 dex, which checks the transcription of all three; the class IV temperatures that
//!   formula implies from F8 to K1 are those halfway, in log T<sub>eff</sub>, between V and III.
//!
//! The ZAMS columns are left out. A blank is `None`; the paper's parenthesised class III gravities
//! at M5 and M6 (0.76 and 0.52, uncertain) are kept.

use super::SpectralLetter;

/// A column of Tables IV and VII: the luminosity classes V to Ia, faintest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Column {
    /// Class V.
    Dwarf = 0,
    /// Class IV.
    Subgiant = 1,
    /// Class III.
    Giant = 2,
    /// Class II.
    BrightGiant = 3,
    /// Class Ib.
    SupergiantIb = 4,
    /// Class Iab.
    SupergiantIab = 5,
    /// Class Ia.
    SupergiantIa = 6,
}

impl Column {
    /// The column's index in [`Sk81Row::m_bol`] and [`Sk81Row::log_g`].
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

/// One spectral type of the paper's Tables III, IV and VII.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sk81Row {
    /// The spectral class letter.
    pub(crate) letter: SpectralLetter,
    /// The subtype.
    pub(crate) subtype: f64,
    /// Table III: log₁₀ T<sub>eff</sub> (K) for V, III and I–II.
    pub(crate) log_teff: [Option<f64>; 3],
    /// Table VII: log₁₀ g (cm s⁻²) for V, IV, III, II, Ib, Iab and Ia.
    pub(crate) log_g: [Option<f64>; 7],
    /// Table IV: M<sub>bol</sub> (mag, M<sub>bol</sub>☉ = +4.72) for V, IV, III, II, Ib, Iab and
    /// Ia.
    pub(crate) m_bol: [Option<f64>; 7],
}

impl Sk81Row {
    /// log₁₀ T<sub>eff</sub> of `column`'s stars at this type: Table III's V, III or I–II column,
    /// and for class IV the V value where V and III share one, otherwise the mean of the two in log
    /// T<sub>eff</sub> (which Tables IV, VI and VII imply). `None` where the table has none.
    #[must_use]
    pub(crate) fn column_log_teff(&self, column: Column) -> Option<f64> {
        let [v, iii, i] = self.log_teff;
        match column {
            Column::Dwarf => v,
            Column::Subgiant => Some(f64::midpoint(v?, iii?)),
            Column::Giant => iii,
            Column::BrightGiant
            | Column::SupergiantIb
            | Column::SupergiantIab
            | Column::SupergiantIa => i,
        }
    }
}

/// A row of [`SK81`], in the paper's column order.
#[must_use]
const fn sk(
    letter: SpectralLetter,
    subtype: f64,
    log_teff: [Option<f64>; 3],
    log_g: [Option<f64>; 7],
    m_bol: [Option<f64>; 7],
) -> Sk81Row {
    Sk81Row {
        letter,
        subtype,
        log_teff,
        log_g,
        m_bol,
    }
}

/// The paper's M<sub>bol</sub>☉, mag: its Table IV's zero point.
pub(crate) const SOLAR_M_BOL_MAG: f64 = 4.72;

/// Tables III, IV and VII of Straižys and Kuriliene (1981), O5 to M6, hottest first.
pub(crate) const SK81: [Sk81Row; 42] = [
    sk(
        SpectralLetter::O,
        5.0,
        [Some(4.626), Some(4.626), Some(4.618)],
        [
            Some(3.9),
            Some(3.86),
            Some(3.82),
            Some(3.76),
            Some(3.74),
            Some(3.69),
            None,
        ],
        [
            Some(-9.8),
            Some(-10.0),
            Some(-10.2),
            Some(-10.3),
            Some(-10.4),
            Some(-10.7),
            Some(-11.0),
        ],
    ), // O5
    sk(
        SpectralLetter::O,
        6.0,
        [Some(4.593), Some(4.593), Some(4.585)],
        [
            Some(3.86),
            Some(3.8),
            Some(3.76),
            Some(3.69),
            Some(3.64),
            Some(3.6),
            Some(3.53),
        ],
        [
            Some(-9.3),
            Some(-9.6),
            Some(-9.8),
            Some(-9.9),
            Some(-10.2),
            Some(-10.4),
            Some(-10.8),
        ],
    ), // O6
    sk(
        SpectralLetter::O,
        7.0,
        [Some(4.568), Some(4.568), Some(4.556)],
        [
            Some(3.85),
            Some(3.8),
            Some(3.74),
            Some(3.64),
            Some(3.57),
            Some(3.52),
            Some(3.45),
        ],
        [
            Some(-8.8),
            Some(-9.1),
            Some(-9.3),
            Some(-9.5),
            Some(-9.8),
            Some(-10.1),
            Some(-10.5),
        ],
    ), // O7
    sk(
        SpectralLetter::O,
        8.0,
        [Some(4.55), Some(4.55), Some(4.535)],
        [
            Some(3.87),
            Some(3.81),
            Some(3.75),
            Some(3.62),
            Some(3.53),
            Some(3.49),
            Some(3.39),
        ],
        [
            Some(-8.3),
            Some(-8.6),
            Some(-8.9),
            Some(-9.2),
            Some(-9.6),
            Some(-9.8),
            Some(-10.4),
        ],
    ), // O8
    sk(
        SpectralLetter::O,
        9.0,
        [Some(4.525), Some(4.525), Some(4.512)],
        [
            Some(3.95),
            Some(3.82),
            Some(3.74),
            Some(3.58),
            Some(3.5),
            Some(3.44),
            Some(3.31),
        ],
        [
            Some(-7.6),
            Some(-8.1),
            Some(-8.4),
            Some(-8.9),
            Some(-9.3),
            Some(-9.6),
            Some(-10.2),
        ],
    ), // O9
    sk(
        SpectralLetter::B,
        0.0,
        [Some(4.498), Some(4.498), Some(4.431)],
        [
            Some(4.0),
            Some(3.88),
            Some(3.74),
            Some(3.39),
            Some(3.27),
            Some(3.19),
            Some(3.05),
        ],
        [
            Some(-7.0),
            Some(-7.4),
            Some(-7.9),
            Some(-8.1),
            Some(-8.6),
            Some(-9.0),
            Some(-9.7),
        ],
    ), // B0
    sk(
        SpectralLetter::B,
        1.0,
        [Some(4.423), Some(4.423), Some(4.371)],
        [
            Some(4.0),
            Some(3.86),
            Some(3.71),
            Some(3.31),
            Some(3.17),
            Some(3.01),
            Some(2.87),
        ],
        [
            Some(-5.8),
            Some(-6.3),
            Some(-6.8),
            Some(-7.4),
            Some(-8.0),
            Some(-8.6),
            Some(-9.4),
        ],
    ), // B1
    sk(
        SpectralLetter::B,
        2.0,
        [Some(4.362), Some(4.362), Some(4.307)],
        [
            Some(4.06),
            Some(3.88),
            Some(3.68),
            Some(3.19),
            Some(3.0),
            Some(2.84),
            Some(2.68),
        ],
        [
            Some(-4.7),
            Some(-5.3),
            Some(-5.9),
            Some(-6.8),
            Some(-7.6),
            Some(-8.2),
            Some(-9.0),
        ],
    ), // B2
    sk(
        SpectralLetter::B,
        3.0,
        [Some(4.286), Some(4.286), Some(4.243)],
        [
            Some(4.06),
            Some(3.89),
            Some(3.71),
            Some(3.12),
            Some(2.79),
            Some(2.68),
            Some(2.49),
        ],
        [
            Some(-3.6),
            Some(-4.1),
            Some(-4.7),
            Some(-6.2),
            Some(-7.3),
            Some(-7.8),
            Some(-8.6),
        ],
    ), // B3
    sk(
        SpectralLetter::B,
        5.0,
        [Some(4.188), Some(4.188), Some(4.137)],
        [
            Some(4.1),
            Some(3.98),
            Some(3.81),
            Some(2.9),
            Some(2.52),
            Some(2.4),
            Some(2.22),
        ],
        [
            Some(-2.1),
            Some(-2.5),
            Some(-3.0),
            Some(-5.4),
            Some(-6.8),
            Some(-7.3),
            Some(-8.1),
        ],
    ), // B5
    sk(
        SpectralLetter::B,
        6.0,
        [Some(4.152), Some(4.152), Some(4.1)],
        [
            Some(4.09),
            Some(3.96),
            Some(3.84),
            Some(2.77),
            Some(2.42),
            Some(2.29),
            Some(2.13),
        ],
        [
            Some(-1.6),
            Some(-2.0),
            Some(-2.4),
            Some(-5.2),
            Some(-6.6),
            Some(-7.2),
            Some(-7.9),
        ],
    ), // B6
    sk(
        SpectralLetter::B,
        7.0,
        [Some(4.107), Some(4.107), Some(4.068)],
        [
            Some(4.07),
            Some(3.95),
            Some(3.82),
            Some(2.77),
            Some(2.33),
            Some(2.21),
            Some(2.02),
        ],
        [
            Some(-1.0),
            Some(-1.4),
            Some(-1.8),
            Some(-4.8),
            Some(-6.4),
            Some(-7.0),
            Some(-7.8),
        ],
    ), // B7
    sk(
        SpectralLetter::B,
        8.0,
        [Some(4.061), Some(4.061), Some(4.041)],
        [
            Some(4.07),
            Some(3.92),
            Some(3.79),
            Some(2.79),
            Some(2.27),
            Some(2.11),
            Some(1.97),
        ],
        [
            Some(-0.4),
            Some(-0.8),
            Some(-1.2),
            Some(-4.4),
            Some(-6.2),
            Some(-6.9),
            Some(-7.6),
        ],
    ), // B8
    sk(
        SpectralLetter::B,
        9.0,
        [Some(4.017), Some(4.017), Some(4.013)],
        [
            Some(4.03),
            Some(3.94),
            Some(3.75),
            Some(2.81),
            Some(2.2),
            Some(2.04),
            Some(1.88),
        ],
        [
            Some(0.1),
            Some(-0.2),
            Some(-0.8),
            Some(-4.0),
            Some(-6.0),
            Some(-6.8),
            Some(-7.5),
        ],
    ), // B9
    sk(
        SpectralLetter::A,
        0.0,
        [Some(3.982), Some(3.982), Some(3.991)],
        [
            Some(4.07),
            Some(3.91),
            Some(3.75),
            Some(2.85),
            Some(2.23),
            Some(2.01),
            Some(1.81),
        ],
        [
            Some(0.7),
            Some(0.2),
            Some(-0.3),
            Some(-3.6),
            Some(-5.7),
            Some(-6.6),
            Some(-7.4),
        ],
    ), // A0
    sk(
        SpectralLetter::A,
        1.0,
        [Some(3.973), Some(3.973), Some(3.978)],
        [
            Some(4.1),
            Some(3.96),
            Some(3.78),
            Some(2.88),
            Some(2.22),
            Some(1.96),
            Some(1.76),
        ],
        [
            Some(0.9),
            Some(0.5),
            Some(-0.1),
            Some(-3.3),
            Some(-5.5),
            Some(-6.6),
            Some(-7.4),
        ],
    ), // A1
    sk(
        SpectralLetter::A,
        2.0,
        [Some(3.961), Some(3.961), Some(3.964)],
        [
            Some(4.16),
            Some(3.98),
            Some(3.78),
            Some(2.87),
            Some(2.23),
            Some(1.92),
            Some(1.71),
        ],
        [
            Some(1.2),
            Some(0.7),
            Some(0.1),
            Some(-3.1),
            Some(-5.3),
            Some(-6.5),
            Some(-7.4),
        ],
    ), // A2
    sk(
        SpectralLetter::A,
        3.0,
        [Some(3.949), Some(3.949), Some(3.949)],
        [
            Some(4.2),
            Some(4.03),
            Some(3.83),
            Some(2.85),
            Some(2.2),
            Some(1.86),
            Some(1.65),
        ],
        [
            Some(1.5),
            Some(1.0),
            Some(0.4),
            Some(-3.0),
            Some(-5.2),
            Some(-6.4),
            Some(-7.4),
        ],
    ), // A3
    sk(
        SpectralLetter::A,
        5.0,
        [Some(3.924), Some(3.924), Some(3.919)],
        [
            Some(4.22),
            Some(4.06),
            Some(3.86),
            Some(2.81),
            Some(2.14),
            Some(1.74),
            Some(1.53),
        ],
        [
            Some(1.9),
            Some(1.4),
            Some(0.8),
            Some(-2.8),
            Some(-5.0),
            Some(-6.4),
            Some(-7.4),
        ],
    ), // A5
    sk(
        SpectralLetter::A,
        7.0,
        [Some(3.903), Some(3.903), Some(3.897)],
        [
            Some(4.26),
            Some(4.1),
            Some(3.86),
            Some(2.75),
            Some(2.08),
            Some(1.65),
            Some(1.38),
        ],
        [
            Some(2.3),
            Some(1.8),
            Some(1.1),
            Some(-2.7),
            Some(-4.9),
            Some(-6.5),
            Some(-7.6),
        ],
    ), // A7
    sk(
        SpectralLetter::F,
        0.0,
        [Some(3.863), Some(3.863), Some(3.869)],
        [
            Some(4.28),
            Some(4.05),
            Some(3.83),
            Some(2.67),
            Some(2.0),
            Some(1.51),
            Some(1.25),
        ],
        [
            Some(2.9),
            Some(2.2),
            Some(1.6),
            Some(-2.6),
            Some(-4.8),
            Some(-6.7),
            Some(-7.8),
        ],
    ), // F0
    sk(
        SpectralLetter::F,
        2.0,
        [Some(3.845), Some(3.845), Some(3.851)],
        [
            Some(4.26),
            Some(4.01),
            Some(3.81),
            Some(2.63),
            Some(1.92),
            Some(1.39),
            Some(1.15),
        ],
        [
            Some(3.1),
            Some(2.4),
            Some(1.8),
            Some(-2.5),
            Some(-4.8),
            Some(-6.8),
            Some(-7.9),
        ],
    ), // F2
    sk(
        SpectralLetter::F,
        5.0,
        [Some(3.813), Some(3.813), Some(3.813)],
        [
            Some(4.28),
            Some(3.93),
            Some(3.74),
            Some(2.48),
            Some(1.81),
            Some(1.22),
            Some(1.0),
        ],
        [
            Some(3.6),
            Some(2.6),
            Some(2.0),
            Some(-2.5),
            Some(-4.7),
            Some(-7.0),
            Some(-7.9),
        ],
    ), // F5
    sk(
        SpectralLetter::F,
        8.0,
        [Some(3.789), Some(3.782), Some(3.778)],
        [
            Some(4.35),
            Some(3.89),
            None,
            Some(2.38),
            Some(1.71),
            Some(1.06),
            Some(0.83),
        ],
        [
            Some(4.1),
            Some(2.8),
            None,
            Some(-2.4),
            Some(-4.6),
            Some(-7.1),
            Some(-8.0),
        ],
    ), // F8
    sk(
        SpectralLetter::G,
        0.0,
        [Some(3.774), Some(3.763), Some(3.756)],
        [
            Some(4.39),
            Some(3.84),
            None,
            Some(2.29),
            Some(1.62),
            Some(0.95),
            Some(0.72),
        ],
        [
            Some(4.4),
            Some(2.9),
            None,
            Some(-2.4),
            Some(-4.6),
            Some(-7.2),
            Some(-8.1),
        ],
    ), // G0
    sk(
        SpectralLetter::G,
        2.0,
        [Some(3.763), Some(3.74), Some(3.732)],
        [
            Some(4.4),
            Some(3.77),
            Some(3.2),
            Some(2.2),
            Some(1.53),
            Some(0.86),
            Some(0.61),
        ],
        [
            Some(4.6),
            Some(2.9),
            Some(1.0),
            Some(-2.4),
            Some(-4.6),
            Some(-7.2),
            Some(-8.2),
        ],
    ), // G2
    sk(
        SpectralLetter::G,
        5.0,
        [Some(3.74), Some(3.712), Some(3.699)],
        [
            Some(4.49),
            Some(3.71),
            Some(3.07),
            Some(2.04),
            Some(1.45),
            Some(0.71),
            Some(0.45),
        ],
        [
            Some(5.1),
            Some(3.0),
            Some(0.8),
            Some(-2.5),
            Some(-4.5),
            Some(-7.3),
            Some(-8.3),
        ],
    ), // G5
    sk(
        SpectralLetter::G,
        8.0,
        [Some(3.72), Some(3.695), Some(3.663)],
        [
            Some(4.55),
            Some(3.64),
            Some(2.95),
            Some(1.84),
            Some(1.3),
            Some(0.6),
            Some(0.3),
        ],
        [
            Some(5.5),
            Some(3.1),
            Some(0.6),
            Some(-2.7),
            Some(-4.5),
            Some(-7.2),
            Some(-8.3),
        ],
    ), // G8
    sk(
        SpectralLetter::K,
        0.0,
        [Some(3.703), Some(3.681), Some(3.643)],
        [
            Some(4.57),
            Some(3.57),
            Some(2.89),
            Some(1.74),
            Some(1.2),
            Some(0.54),
            Some(0.25),
        ],
        [
            Some(5.8),
            Some(3.0),
            Some(0.5),
            Some(-2.8),
            Some(-4.6),
            Some(-7.1),
            Some(-8.2),
        ],
    ), // K0
    sk(
        SpectralLetter::K,
        1.0,
        [Some(3.695), Some(3.663), Some(3.633)],
        [
            Some(4.55),
            Some(3.55),
            Some(2.78),
            Some(1.66),
            Some(1.16),
            Some(0.54),
            Some(0.25),
        ],
        [
            Some(5.9),
            Some(3.0),
            Some(0.4),
            Some(-2.9),
            Some(-4.6),
            Some(-7.1),
            Some(-8.1),
        ],
    ), // K1
    sk(
        SpectralLetter::K,
        2.0,
        [Some(3.686), Some(3.648), Some(3.623)],
        [
            Some(4.55),
            None,
            Some(2.63),
            Some(1.59),
            Some(1.1),
            Some(0.48),
            Some(0.23),
        ],
        [
            Some(6.0),
            None,
            Some(0.2),
            Some(-3.0),
            Some(-4.7),
            Some(-7.0),
            Some(-8.0),
        ],
    ), // K2
    sk(
        SpectralLetter::K,
        3.0,
        [Some(3.672), Some(3.628), Some(3.613)],
        [
            Some(4.56),
            None,
            Some(2.36),
            Some(1.52),
            Some(1.0),
            Some(0.46),
            Some(0.19),
        ],
        [
            Some(6.2),
            None,
            Some(-0.1),
            Some(-3.1),
            Some(-4.9),
            Some(-7.0),
            Some(-8.0),
        ],
    ), // K3
    sk(
        SpectralLetter::K,
        4.0,
        [Some(3.663), Some(3.613), None],
        [Some(4.57), None, Some(2.16), None, None, None, None],
        [Some(6.4), None, Some(-0.4), None, None, None, None],
    ), // K4
    sk(
        SpectralLetter::K,
        5.0,
        [Some(3.643), Some(3.602), Some(3.585)],
        [
            Some(4.57),
            None,
            Some(1.93),
            Some(1.2),
            Some(0.77),
            Some(0.35),
            Some(0.1),
        ],
        [
            Some(6.7),
            None,
            Some(-0.9),
            Some(-3.7),
            Some(-5.4),
            Some(-7.0),
            Some(-8.0),
        ],
    ), // K5
    sk(
        SpectralLetter::K,
        7.0,
        [Some(3.602), None, None],
        [Some(4.62), None, None, None, None, None, None],
        [Some(7.3), None, None, None, None, None, None],
    ), // K7
    sk(
        SpectralLetter::M,
        0.0,
        [Some(3.591), Some(3.591), Some(3.568)],
        [
            Some(4.61),
            None,
            Some(1.63),
            Some(1.01),
            Some(0.61),
            Some(0.3),
            Some(0.0),
        ],
        [
            Some(7.5),
            None,
            Some(-1.8),
            Some(-4.0),
            Some(-5.8),
            Some(-7.0),
            Some(-8.1),
        ],
    ), // M0
    sk(
        SpectralLetter::M,
        1.0,
        [Some(3.574), Some(3.58), Some(3.556)],
        [
            Some(4.67),
            None,
            Some(1.41),
            Some(0.84),
            Some(0.51),
            Some(0.19),
            Some(-0.07),
        ],
        [
            Some(7.9),
            None,
            Some(-2.4),
            Some(-4.3),
            Some(-6.0),
            Some(-7.2),
            Some(-8.2),
        ],
    ), // M1
    sk(
        SpectralLetter::M,
        2.0,
        [Some(3.55), Some(3.574), Some(3.544)],
        [
            Some(4.69),
            None,
            Some(1.31),
            Some(0.7),
            Some(0.39),
            Some(0.09),
            Some(-0.13),
        ],
        [
            Some(8.3),
            None,
            Some(-2.6),
            Some(-4.5),
            Some(-6.2),
            Some(-7.4),
            Some(-8.3),
        ],
    ), // M2
    sk(
        SpectralLetter::M,
        3.0,
        [Some(3.531), Some(3.562), Some(3.518)],
        [
            Some(4.71),
            None,
            Some(1.12),
            Some(0.38),
            Some(0.1),
            Some(-0.16),
            Some(-0.34),
        ],
        [
            Some(8.8),
            None,
            Some(-2.9),
            Some(-5.1),
            Some(-6.7),
            Some(-7.8),
            Some(-8.7),
        ],
    ), // M3
    sk(
        SpectralLetter::M,
        4.0,
        [Some(3.512), Some(3.55), Some(3.491)],
        [Some(4.77), None, Some(0.98), None, None, None, None],
        [
            Some(9.3),
            None,
            Some(-3.1),
            Some(-5.7),
            Some(-7.3),
            Some(-8.4),
            Some(-9.3),
        ],
    ), // M4
    sk(
        SpectralLetter::M,
        5.0,
        [Some(3.491), Some(3.531), Some(3.47)],
        [Some(5.06), None, Some(0.76), None, None, None, None],
        [
            Some(11.0),
            None,
            Some(-3.2),
            Some(-6.3),
            Some(-8.0),
            Some(-9.1),
            Some(-10.0),
        ],
    ), // M5
    sk(
        SpectralLetter::M,
        6.0,
        [None, Some(3.512), None],
        [None, None, Some(0.52), None, None, None, None],
        [None, None, Some(-3.6), None, None, None, None],
    ), // M6
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;

    /// Table VI, log₁₀ of the mass in M☉ for V, IV, III, II, Ib, Iab and Ia, which the paper
    /// derives from evolutionary tracks and from which it computes Table VII.
    const TABLE_VI: [[Option<f64>; 7]; 42] = [
        [
            Some(1.81),
            Some(1.85),
            Some(1.89),
            Some(1.9),
            Some(1.92),
            Some(1.99),
            None,
        ], // O5
        [
            Some(1.7),
            Some(1.76),
            Some(1.8),
            Some(1.8),
            Some(1.87),
            Some(1.91),
            Some(2.0),
        ], // O6
        [
            Some(1.59),
            Some(1.65),
            Some(1.68),
            Some(1.71),
            Some(1.76),
            Some(1.83),
            Some(1.92),
        ], // O7
        [
            Some(1.48),
            Some(1.54),
            Some(1.6),
            Some(1.65),
            Some(1.72),
            Some(1.76),
            Some(1.9),
        ], // O8
        [
            Some(1.38),
            Some(1.45),
            Some(1.49),
            Some(1.58),
            Some(1.66),
            Some(1.72),
            Some(1.83),
        ], // O9
        [
            Some(1.3),
            Some(1.34),
            Some(1.4),
            Some(1.4),
            Some(1.48),
            Some(1.56),
            Some(1.7),
        ], // B0
        [
            Some(1.11),
            Some(1.18),
            Some(1.23),
            Some(1.28),
            Some(1.38),
            Some(1.46),
            Some(1.64),
        ], // B1
        [
            Some(0.99),
            Some(1.04),
            Some(1.08),
            Some(1.18),
            Some(1.3),
            Some(1.38),
            Some(1.54),
        ], // B2
        [
            Some(0.84),
            Some(0.88),
            Some(0.94),
            Some(1.11),
            Some(1.23),
            Some(1.32),
            Some(1.45),
        ], // B3
        [
            Some(0.68),
            Some(0.72),
            Some(0.75),
            Some(1.0),
            Some(1.18),
            Some(1.26),
            Some(1.4),
        ], // B5
        [
            Some(0.61),
            Some(0.64),
            Some(0.68),
            Some(0.94),
            Some(1.15),
            Some(1.26),
            Some(1.38),
        ], // B6
        [
            Some(0.53),
            Some(0.57),
            Some(0.6),
            Some(0.91),
            Some(1.11),
            Some(1.23),
            Some(1.36),
        ], // B7
        [
            Some(0.48),
            Some(0.49),
            Some(0.52),
            Some(0.88),
            Some(1.08),
            Some(1.2),
            Some(1.34),
        ], // B8
        [
            Some(0.41),
            Some(0.45),
            Some(0.49),
            Some(0.85),
            Some(1.04),
            Some(1.2),
            Some(1.32),
        ], // B9
        [
            Some(0.35),
            Some(0.39),
            Some(0.43),
            Some(0.81),
            Some(1.04),
            Some(1.18),
            Some(1.3),
        ], // A0
        [
            Some(0.34),
            Some(0.36),
            Some(0.41),
            Some(0.78),
            Some(1.0),
            Some(1.18),
            Some(1.3),
        ], // A1
        [
            Some(0.32),
            Some(0.34),
            Some(0.39),
            Some(0.75),
            Some(0.98),
            Some(1.15),
            Some(1.3),
        ], // A2
        [
            Some(0.3),
            Some(0.32),
            Some(0.36),
            Some(0.75),
            Some(0.97),
            Some(1.11),
            Some(1.3),
        ], // A3
        [
            Some(0.26),
            Some(0.29),
            Some(0.33),
            Some(0.74),
            Some(0.95),
            Some(1.11),
            Some(1.3),
        ], // A5
        [
            Some(0.22),
            Some(0.26),
            Some(0.3),
            Some(0.73),
            Some(0.94),
            Some(1.15),
            Some(1.32),
        ], // A7
        [
            Some(0.16),
            Some(0.2),
            Some(0.23),
            Some(0.72),
            Some(0.93),
            Some(1.2),
            Some(1.38),
        ], // F0
        [
            Some(0.13),
            Some(0.16),
            Some(0.2),
            Some(0.72),
            Some(0.93),
            Some(1.2),
            Some(1.4),
        ], // F2
        [
            Some(0.08),
            Some(0.13),
            Some(0.18),
            Some(0.72),
            Some(0.93),
            Some(1.26),
            Some(1.4),
        ], // F5
        [
            Some(0.04),
            Some(0.11),
            None,
            Some(0.72),
            Some(0.93),
            Some(1.28),
            Some(1.41),
        ], // F8
        [
            Some(0.02),
            Some(0.1),
            None,
            Some(0.72),
            Some(0.93),
            Some(1.3),
            Some(1.43),
        ], // G0
        [
            Some(0.0),
            Some(0.1),
            Some(0.33),
            Some(0.72),
            Some(0.93),
            Some(1.3),
            Some(1.45),
        ], // G2
        [
            Some(-0.02),
            Some(0.08),
            Some(0.39),
            Some(0.73),
            Some(0.94),
            Some(1.32),
            Some(1.46),
        ], // G5
        [
            Some(-0.04),
            Some(0.08),
            Some(0.42),
            Some(0.76),
            Some(0.94),
            Some(1.32),
            Some(1.46),
        ], // G8
        [
            Some(-0.07),
            Some(0.11),
            Some(0.46),
            Some(0.78),
            Some(0.96),
            Some(1.3),
            Some(1.45),
        ], // K0
        [
            Some(-0.1),
            Some(0.13),
            Some(0.46),
            Some(0.78),
            Some(0.96),
            Some(1.3),
            Some(1.45),
        ], // K1
        [
            Some(-0.1),
            None,
            Some(0.45),
            Some(0.79),
            Some(0.98),
            Some(1.28),
            Some(1.43),
        ], // K2
        [
            Some(-0.12),
            None,
            Some(0.38),
            Some(0.8),
            Some(1.0),
            Some(1.3),
            Some(1.43),
        ], // K3
        [Some(-0.15), None, Some(0.36), None, None, None, None], // K4
        [
            Some(-0.19),
            None,
            Some(0.37),
            Some(0.83),
            Some(1.08),
            Some(1.3),
            Some(1.45),
        ], // K5
        [Some(-0.22), None, None, None, None, None, None],       // K7
        [
            Some(-0.26),
            None,
            Some(0.48),
            Some(0.83),
            Some(1.15),
            Some(1.32),
            Some(1.46),
        ], // M0
        [
            Some(-0.3),
            None,
            Some(0.54),
            Some(0.83),
            Some(1.18),
            Some(1.34),
            Some(1.48),
        ], // M1
        [
            Some(-0.35),
            None,
            Some(0.54),
            Some(0.81),
            Some(1.18),
            Some(1.36),
            Some(1.5),
        ], // M2
        [
            Some(-0.4),
            None,
            Some(0.52),
            Some(0.84),
            Some(1.2),
            Some(1.38),
            Some(1.56),
        ], // M3
        [Some(-0.52), None, Some(0.51), None, None, None, None], // M4
        [Some(-0.82), None, Some(0.41), None, None, None, None], // M5
        [None, None, Some(0.4), None, None, None, None],         // M6
    ];

    /// FNV-1a over every field's bits in row order, a missing value hashing as one `0xff` byte.
    fn checksum(rows: &[Sk81Row]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut eat = |bytes: &[u8]| {
            for &byte in bytes {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        for r in rows {
            eat(&[r.letter.index()]);
            eat(&bits(r.subtype).to_le_bytes());
            for value in r.log_teff.iter().chain(&r.log_g).chain(&r.m_bol) {
                match value {
                    Some(v) => eat(&bits(*v).to_le_bytes()),
                    None => eat(&[0xff]),
                }
            }
        }
        hash
    }

    #[test]
    fn the_calibration_is_pinned_by_its_checksum() {
        assert_eq!(checksum(&SK81), SK81_CHECKSUM);
    }

    const SK81_CHECKSUM: u64 = 6_448_109_106_400_513_586;

    /// Every gravity of Table VII is the paper's formula applied to Tables III, IV and VI, to
    /// within the tables' rounding: a slip in any of the three transcriptions shows here.
    #[test]
    fn the_gravities_follow_from_the_temperatures_magnitudes_and_masses() {
        let columns = [
            Column::Dwarf,
            Column::Subgiant,
            Column::Giant,
            Column::BrightGiant,
            Column::SupergiantIb,
            Column::SupergiantIab,
            Column::SupergiantIa,
        ];
        let mut checked = 0;
        for (row, masses) in SK81.iter().zip(TABLE_VI) {
            for column in columns {
                let k = column.index();
                let (Some(g), Some(m_bol), Some(log_m), Some(log_t)) = (
                    row.log_g[k],
                    row.m_bol[k],
                    masses[k],
                    row.column_log_teff(column),
                ) else {
                    continue;
                };
                let implied = log_m + 4.0 * log_t + 0.4 * m_bol - 12.49;
                assert!(
                    (g - implied).abs() < 0.045,
                    "{row:?} {column:?}: {g} against {implied}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 257);
    }

    #[test]
    fn temperatures_fall_down_each_column() {
        for k in 0..3 {
            let column: Vec<f64> = SK81.iter().filter_map(|r| r.log_teff[k]).collect();
            assert!(column.windows(2).all(|w| w[0] > w[1]), "column {k}");
        }
    }
}
