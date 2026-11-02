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
//! The file gives BC<sub>V</sub> down to L5V (1,710 K) and B − V down to M9V (2,380 K); below
//! those a star has none.
//!
//! **The fitted grids** ([`tables::dwarf_sequence`](crate::tables::dwarf_sequence), P06.T23.a's
//! licence fix): the spectral code, BC<sub>V</sub> and B − V on even grids in log₁₀
//! T<sub>eff</sub>, which `hyperion-fit`'s `dwarf_sequence` task resamples from the fetched file,
//! read linearly between nodes. The file's rows are not carried: its terms (none stated) do not let
//! the repository carry the raw table (decisions-r05 item 4, decision-r08-licences). The grids keep
//! the rows' values at their ends, O3V and each quantity's coolest row, and lie within 0.021 of a
//! subtype, 0.0034 mag in BC<sub>V</sub> and 0.0019 mag in B − V of the rows' interpolation
//! everywhere between (the table's acceptance). Every reader takes them through [`DwarfSequence::of`].

use crate::math;
use crate::stellar::rules::StellarRules;
use crate::tables::dwarf_sequence as reduced;

/// A photometric quantity of the sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Column {
    /// The V-band bolometric correction BC<sub>V</sub>, mag.
    BcV,
    /// The intrinsic B − V colour, mag.
    ColourBV,
}

/// The hottest point of the sequence, O3V, from which the photometry continues it to hotter
/// stars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hottest {
    /// Its effective temperature, K.
    pub(crate) teff_k: f64,
    /// Its BC<sub>V</sub>, mag.
    pub(crate) bc_v: f64,
    /// Its B − V, mag.
    pub(crate) colour_b_v: f64,
}

/// The dwarf sequence every reader takes: the fitted grids of
/// [`tables::dwarf_sequence`](crate::tables::dwarf_sequence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct DwarfSequence;

#[expect(
    clippy::unused_self,
    reason = "the value is the stellar rules' choice of sequence, the choke point a later hold of \
              it reads; one sequence is in force at every version"
)]
impl DwarfSequence {
    /// The sequence the stellar stage's `rules` read: the fitted grids at every version. It stays
    /// the one choke point every reader goes through, for a later hold of the sequence.
    #[must_use]
    pub(crate) const fn of(_rules: StellarRules) -> Self {
        Self
    }

    /// The dwarf scale's continuous spectral code at `teff_k`: linear in log₁₀ T<sub>eff</sub>
    /// between nodes, held at O3 (3) above the sequence and at Y4 (94) below it.
    ///
    /// The caller passes a finite, positive temperature (debug-asserted); a NaN reads as cooler
    /// than the sequence.
    #[must_use]
    pub(crate) fn code_at(self, teff_k: f64) -> f64 {
        debug_assert!(
            teff_k.is_finite() && teff_k > 0.0,
            "an effective temperature is finite and positive: {teff_k}"
        );
        if teff_k > reduced::TEFF_HOT_K {
            return reduced::CODE[0];
        }
        on_grid(
            teff_k,
            reduced::CODE_LOG_TEFF_COOL,
            reduced::CODE_STEP_DEX,
            &reduced::CODE,
        )
        .unwrap_or(reduced::CODE[reduced::CODE.len() - 1])
    }

    /// `column` at `teff_k`, linear in log₁₀ T<sub>eff</sub> between nodes; `None` where the
    /// sequence does not give it, and beyond either end.
    #[must_use]
    pub(crate) fn column_at(self, teff_k: f64, column: Column) -> Option<f64> {
        match column {
            Column::BcV => on_grid(
                teff_k,
                reduced::BC_V_LOG_TEFF_COOL,
                reduced::BC_V_STEP_DEX,
                &reduced::BC_V,
            ),
            Column::ColourBV => on_grid(
                teff_k,
                reduced::COLOUR_B_V_LOG_TEFF_COOL,
                reduced::COLOUR_B_V_STEP_DEX,
                &reduced::COLOUR_B_V,
            ),
        }
    }

    /// The hottest point, O3V: the grids' first nodes, the row's own values.
    #[must_use]
    pub(crate) fn hottest(self) -> Hottest {
        Hottest {
            teff_k: reduced::TEFF_HOT_K,
            bc_v: reduced::BC_V[0],
            colour_b_v: reduced::COLOUR_B_V[0],
        }
    }
}

/// A grid of [`tables::dwarf_sequence`](crate::tables::dwarf_sequence) at `teff_k`: its `values`
/// at nodes `step` dex apart in log₁₀ T<sub>eff</sub> from `LOG_TEFF_HOT` down to `cool`, linear
/// between them, as the table's task measures it; `None` outside the ends and for a NaN.
#[must_use]
fn on_grid(teff_k: f64, cool: f64, step: f64, values: &[f64]) -> Option<f64> {
    let x = math::log10(teff_k);
    if !(x >= cool && x <= reduced::LOG_TEFF_HOT) {
        return None;
    }
    let position = (reduced::LOG_TEFF_HOT - x) / step;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative number of steps, held below the grid's few thousand cells"
    )]
    let i = (position.floor().clamp(0.0, 1.0e5) as usize).min(values.len() - 2);
    let fraction = position - f64::from(u32::try_from(i).expect("a grid has a few thousand nodes"));
    Some(values[i] + fraction * (values[i + 1] - values[i]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stellar::classify::tests::{star, star_g};
    use crate::stellar::classify::{
        ClassExtras, LuminosityClass, classify_under, subtype_from_teff_under,
    };
    use crate::stellar::draws::StarDraws;
    use crate::stellar::photometry::{absolute_magnitude_v_under, colour_b_v_under};
    use crate::stellar::{Composition, Phase, StarState};
    use crate::units::Kelvin;

    /// The generator version's stellar rules.
    const VERSION: StellarRules = StellarRules::of_version(crate::GENERATOR_VERSION);

    /// The grids' ends are the rows': O3V's code 3 at 44,900 K, Y4V's 94, BC<sub>V</sub> ending at
    /// L5V (1,710 K) and B − V at M9V (2,380 K), the source's last values of each.
    #[test]
    fn the_grids_end_where_the_source_does() {
        let sequence = DwarfSequence::of(VERSION);
        assert!((sequence.code_at(44_900.0) - 3.0).abs() < 1e-12);
        assert!((sequence.code_at(250.0) - 94.0).abs() < 1e-9);
        assert!((sequence.code_at(100.0) - 94.0).abs() < 1e-12);
        for (column, coolest) in [(Column::BcV, 1_710.0), (Column::ColourBV, 2_380.0)] {
            assert!(sequence.column_at(coolest * 1.000_001, column).is_some());
            assert_eq!(sequence.column_at(coolest * 0.999_99, column), None);
        }
        assert_eq!(sequence.column_at(45_000.0, Column::BcV), None);
    }

    /// The plan's figures (P06.T23's tests, restated on the grids): the Sun is G2V with
    /// M<sub>V</sub> = 4.81 ± 0.05 and B − V 0.65; a Vega-like star (9,600 K, log g 4.0) is A0V;
    /// a 4,300 K, log g 1.7 star is K III; 3,600 K and 10⁵ L☉ is M Iab or Ia; the dwarf subtype is
    /// monotone in temperature; an L4 dwarf has a V magnitude, one below 1,710 K none.
    #[test]
    fn the_plans_figures_hold_on_the_grids() {
        let sun = star(Phase::MainSequence, 1.0, 1.0, 5_772.0);
        let class = |state: &StarState| {
            classify_under(
                state,
                &Composition::SOLAR,
                &StarDraws::median(),
                ClassExtras::NONE,
                VERSION,
            )
        };
        assert_eq!(class(&sun).to_string(), "G2V");
        let m_v = absolute_magnitude_v_under(&sun, VERSION).unwrap().value();
        assert!((m_v - 4.81).abs() < 0.05, "{m_v}");
        let b_v = colour_b_v_under(Kelvin::new(5_772.0), VERSION)
            .unwrap()
            .value();
        assert!((b_v - 0.65).abs() < 0.005, "{b_v}");
        let vega = star_g(Phase::MainSequence, 2.1, 4.0, 9_600.0);
        assert_eq!(class(&vega).to_string(), "A0V");
        let arcturus = star_g(Phase::FirstGiantBranch, 1.1, 1.7, 4_300.0);
        let written = class(&arcturus).to_string();
        assert!(
            written.starts_with('K') && written.ends_with("III"),
            "{written}"
        );
        let red = class(&star(Phase::CoreHeliumBurning, 15.0, 1e5, 3_600.0));
        assert!(red.to_string().starts_with('M'), "{red}");
        assert!(
            matches!(
                red.luminosity_class(),
                Some(LuminosityClass::Supergiant | LuminosityClass::LuminousSupergiant)
            ),
            "{red}"
        );
        let mut previous = 0.0;
        let mut t = 60_000.0_f64;
        while t > 100.0 {
            let code = subtype_from_teff_under(Kelvin::new(t), VERSION)
                .unwrap()
                .value();
            assert!(code >= previous, "{t} K: {code} after {previous}");
            previous = code;
            t *= 0.9995;
        }
        let l4 = star(Phase::Substellar, 0.07, 2.0e-4, 1_870.0);
        assert!(absolute_magnitude_v_under(&l4, VERSION).is_some());
        let cooler = star(Phase::Substellar, 0.06, 1e-4, 1_650.0);
        assert_eq!(absolute_magnitude_v_under(&cooler, VERSION), None);
    }
}
