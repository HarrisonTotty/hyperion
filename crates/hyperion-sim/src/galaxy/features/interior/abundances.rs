//! Multiple populations: a massive globular's second population and its members' light-element
//! offsets (plan 09, P09.T9.h).
//!
//! Every globular born above 10⁵ M☉ splits its members by an independent mark into a first and a
//! second population (Milone and Marino 2022, Universe 8, 359). The first's share is `0.62 − 0.30
//! (log₁₀ M₀ − 5)` held to 0.1–0.7 ([`first_population_share`]), the plan's coefficients (ours, read
//! from their Fig. 8, which plots against the present mass; the plan reads the birth mass, a
//! question for the owner). A second-population member draws
//! an enrichment `e` in (0, 1], and its abundances are linear in it ([`MemberAbundances`]), with
//! maxima that rise with the cluster's birth mass, log-linearly from 10⁵ to 10^6.5 M☉ (ours):
//!
//! - nitrogen up 0.5–1.2 dex and sodium up 0.3–0.6, oxygen down 0.2–0.8 (the plan's ranges);
//! - carbon down 0.2–0.5 (ours: the plan says only "down");
//! - aluminium up 0.3–1.0 and magnesium down 0.1–0.4 only above 10⁶ M☉ and below [Fe/H] −1 (ours
//!   on the plan's condition; Carretta et al. 2009, A&A 505, 139, find the Mg–Al anticorrelation
//!   only in massive, metal-poor clusters);
//! - helium `Y = Y₀ + ΔY_max e²`, `ΔY_max` from 0.01 at 10⁵ M☉ to 0.18 at 10⁶ and above (Milone and
//!   Marino 2022);
//! - iron unchanged except in one cluster in six, whose members' [Fe/H] spreads by a normal of
//!   0.1 dex (the scatter is ours).
//!
//! The first population carries none of it ([`MemberAbundances::NONE`]).

use crate::math;
use crate::units::{Dex, HeliumExcess, SolarMasses};

/// Globulars born above this mass have a second population, M☉.
pub const MULTIPLE_POPULATION_MASS: f64 = 1e5;

/// The share of globulars with an iron spread (Milone and Marino 2022: one in six).
pub const IRON_SPREAD_SHARE: f64 = 1.0 / 6.0;

/// The iron spread's normal scatter, dex (ours, provisional).
pub const IRON_SPREAD_SIGMA: f64 = 0.1;

/// The greatest helium excess of the most massive clusters (Milone and Marino 2022).
pub const HELIUM_EXCESS_MAX: f64 = 0.18;

/// The first population's share of a globular born with `initial_mass`: `0.62 − 0.30 (log₁₀ M₀ −
/// 5)` held to 0.1–0.7, and 1 below 10⁵ M☉.
#[must_use]
pub fn first_population_share(initial_mass: SolarMasses) -> f64 {
    let m = initial_mass.value();
    if m <= MULTIPLE_POPULATION_MASS {
        return 1.0;
    }
    (0.62 - 0.30 * (math::log10(m) - 5.0)).clamp(0.1, 0.7)
}

/// The greatest helium excess of a cluster born with `initial_mass`: 0.01 at 10⁵ M☉ rising
/// log-linearly to 0.18 at 10⁶ and above.
#[must_use]
pub fn helium_excess_max(initial_mass: SolarMasses) -> f64 {
    let x = (math::log10(initial_mass.value()) - 5.0).clamp(0.0, 1.0);
    0.01 + (HELIUM_EXCESS_MAX - 0.01) * x
}

/// A member's abundance offsets from its cluster's composition (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberAbundances {
    /// The enrichment `e`: 0 for the first population, (0, 1] for the second.
    pub enrichment: f64,
    /// [N/Fe] offset.
    pub nitrogen: Dex,
    /// [Na/Fe] offset.
    pub sodium: Dex,
    /// [O/Fe] offset.
    pub oxygen: Dex,
    /// [C/Fe] offset.
    pub carbon: Dex,
    /// [Al/Fe] offset.
    pub aluminium: Dex,
    /// [Mg/Fe] offset.
    pub magnesium: Dex,
    /// [Fe/H] offset from the cluster's, in a cluster with an iron spread.
    pub iron: Dex,
    /// The helium mass fraction's excess ΔY.
    pub helium_excess: HeliumExcess,
}

impl MemberAbundances {
    /// No offset at all: a first-population member of a cluster without an iron spread.
    pub const NONE: Self = Self {
        enrichment: 0.0,
        nitrogen: Dex::ZERO,
        sodium: Dex::ZERO,
        oxygen: Dex::ZERO,
        carbon: Dex::ZERO,
        aluminium: Dex::ZERO,
        magnesium: Dex::ZERO,
        iron: Dex::ZERO,
        helium_excess: HeliumExcess::ZERO,
    };

    /// A member's offsets: `enrichment` 0 for the first population or in (0, 1] for the second,
    /// in a cluster born with `initial_mass` at `fe_h`, with `iron` its [Fe/H] offset (0 unless
    /// the cluster has an iron spread).
    #[must_use]
    pub fn of(enrichment: f64, initial_mass: SolarMasses, fe_h: Dex, iron: Dex) -> Self {
        let e = enrichment.clamp(0.0, 1.0);
        let x = ((math::log10(initial_mass.value()) - 5.0) / 1.5).clamp(0.0, 1.0);
        let span = |lo: f64, hi: f64| lo + (hi - lo) * x;
        let massive_metal_poor = initial_mass.value() > 1e6 && fe_h.value() < -1.0;
        let (al, mg) = if massive_metal_poor {
            (span(0.3, 1.0), span(0.1, 0.4))
        } else {
            (0.0, 0.0)
        };
        Self {
            enrichment: e,
            nitrogen: Dex::new(e * span(0.5, 1.2)),
            sodium: Dex::new(e * span(0.3, 0.6)),
            oxygen: Dex::new(-e * span(0.2, 0.8)),
            carbon: Dex::new(-e * span(0.2, 0.5)),
            aluminium: Dex::new(e * al),
            magnesium: Dex::new(-e * mg),
            iron,
            helium_excess: HeliumExcess::new(helium_excess_max(initial_mass) * e * e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_population_s_share_follows_milone_and_marino() {
        let at = |m: f64| first_population_share(SolarMasses::new(m));
        assert!((at(9e4) - 1.0).abs() < 1e-15);
        assert!((at(1e5 * (1.0 + 1e-12)) - 0.62).abs() < 1e-9);
        assert!((at(1e6) - 0.32).abs() < 1e-12);
        assert!((at(math::exp10(6.5)) - 0.17).abs() < 1e-12);
        assert!((at(1e8) - 0.1).abs() < 1e-15);
    }

    #[test]
    fn helium_never_exceeds_its_maximum() {
        for m in [1e5, 3e5, 1e6, 1e7] {
            for e in [0.0, 0.3, 0.7, 1.0] {
                let a = MemberAbundances::of(e, SolarMasses::new(m), Dex::new(-1.5), Dex::ZERO);
                assert!(a.helium_excess.value() <= HELIUM_EXCESS_MAX + 1e-15);
                assert!(a.nitrogen.value() >= 0.0 && a.oxygen.value() <= 0.0);
            }
        }
        let top = MemberAbundances::of(1.0, SolarMasses::new(3e6), Dex::new(-1.5), Dex::ZERO);
        assert!((top.helium_excess.value() - 0.18).abs() < 1e-12);
        assert!(top.aluminium.value() > 0.0);
        let rich = MemberAbundances::of(1.0, SolarMasses::new(3e6), Dex::new(-0.5), Dex::ZERO);
        assert!(rich.aluminium.value().abs() < 1e-15);
    }
}
