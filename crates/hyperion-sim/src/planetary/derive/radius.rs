//! Radius from mass (plan 14, P14.T11.a and T11.b; design note 8).
//!
//! Two relations, used in turn. [`radius_chen_kipping`] places a body within the observed scatter
//! of radius at its mass, through the one quantile the body draws; [`radius_zeng`] is the radius of
//! a body of known iron, rock and water, which [`composition`](mod@super::composition) inverts to turn
//! that radius into a composition. Hydrogen and helium envelopes are
//! [`envelope`](super::envelope)'s.
//!
//! # Chen and Kipping (2017)
//!
//! Chen and Kipping (2017, ApJ 834, 17) fit one probabilistic broken power law to 316 bodies from
//! Solar System moons to late-type stars: log₁₀ R is normal about C + S log₁₀ M with a standard
//! deviation σ of its own on each of four segments, joined continuously at three fitted
//! transitions (their eqs. 2, 3, 5 and 6). Every constant below is the median of the posterior in
//! their Table 1, re-checked against the paper (arXiv:1603.08614, the published version).
//!
//! # Zeng, Sasselov and Jacobsen (2016) and Zeng et al. (2019)
//!
//! The rock-and-iron curves are the mass–radius table of Zeng, Sasselov and Jacobsen (2016, ApJ
//! 819, 127, Table 2): pure iron, 50%, 30%, 25% and 20% iron, and pure rock, from 0.125 to 32 M⊕,
//! computed from equations of state extrapolated from Earth's seismic model (PREM). The plan asked
//! for their fitted power law, R = (1.07 − 0.21 × CMF) M^(1/3.7) (their eq. 2), but they state it
//! only for 1–8 M⊕ and a core mass fraction of 0–0.4: it puts Mars 6% and Mercury 10% too large and
//! has no iron curve. The table it was fitted to covers every curve and Mercury's and Mars's masses
//! (after a short extrapolation), so the table is what is used, interpolated linearly in log M and
//! log R and linearly in the core mass fraction between curves; it agrees with the power law to
//! 0.025 R⊕ where the power law holds (tested).
//!
//! Water is Zeng et al.'s (2019, PNAS 116, 9723, Materials and Methods): an icy body of water mass
//! fraction x has R = f(x) M^(1/3.7) with f = 1 + 0.55 x − 0.14 x², so 1.41 for pure water, where a
//! rocky body has M^(1/3.7). Here the same factor blends a body's dry curve towards the table's
//! pure-water curve, R = `R_dry` + g(x) (`R_water` − `R_dry`) with g = (f − 1) ÷ 0.41, which is Zeng et
//! al.'s relation for an Earth-like core. Blended on pure rock it matches the 2016 table's 25% and
//! 50% water curves to 1.2% (tested), which suggests they sit on pure rock, though the paper does
//! not say.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::stellar::draws::UnitUniform;
use crate::units::consts::{GM_EARTH, GM_JUPITER, GM_SUN};
use crate::units::{Dex, EarthMasses, EarthRadii};

/// Chen and Kipping's radius at 1 M⊕ on the Terran segment, 10^C⁽¹⁾ = 1.008 R⊕ (Table 1:
/// 1.008 +0.046 −0.045).
pub const TERRAN_RADIUS_AT_ONE_EARTH_MASS: EarthRadii = EarthRadii::new(1.008);

/// The power-law index of Terran worlds, R ∝ M^0.2790 (Chen and Kipping 2017, Table 1: S⁽¹⁾ =
/// 0.2790 +0.0092 −0.0094).
pub const TERRAN_INDEX: f64 = 0.2790;

/// The power-law index of Neptunian worlds, R ∝ M^0.589 (Table 1: S⁽²⁾ = 0.589 +0.044 −0.031).
pub const NEPTUNIAN_INDEX: f64 = 0.589;

/// The power-law index of Jovian worlds, R ∝ M^−0.044 (Table 1: S⁽³⁾ = −0.044 +0.017 −0.019).
pub const JOVIAN_INDEX: f64 = -0.044;

/// The power-law index of stellar worlds, R ∝ M^0.881 (Table 1: S⁽⁴⁾ = 0.881 +0.025 −0.024).
pub const STELLAR_INDEX: f64 = 0.881;

/// The scatter of log₁₀ R about the Terran relation, 0.0403 dex (Table 1: σ⁽¹⁾ = 4.03 +0.94
/// −0.64 %).
///
/// Chen and Kipping's `σ_R` is the standard deviation of log₁₀ R about the relation (their eq. 3),
/// which Table 1 prints as a percentage: 4.03% is 0.0403 dex, a factor of 1.097 in radius.
pub const TERRAN_SCATTER: Dex = Dex::new(0.0403);

/// The scatter about the Neptunian relation, 0.146 dex (Table 1: σ⁽²⁾ = 14.6 +1.7 −1.3 %).
pub const NEPTUNIAN_SCATTER: Dex = Dex::new(0.146);

/// The scatter about the Jovian relation, 0.0737 dex (Table 1: σ⁽³⁾ = 7.37 +0.46 −0.45 %).
pub const JOVIAN_SCATTER: Dex = Dex::new(0.0737);

/// The scatter about the stellar relation, 0.0443 dex (Table 1: σ⁽⁴⁾ = 4.43 +0.64 −0.47 %).
pub const STELLAR_SCATTER: Dex = Dex::new(0.0443);

/// The Terran-to-Neptunian transition, 2.04 M⊕ (Table 1: 10^T⁽¹⁾ = 2.04 +0.66 −0.59 M⊕).
pub const TERRAN_NEPTUNIAN_TRANSITION: EarthMasses = EarthMasses::new(2.04);

/// The Neptunian-to-Jovian transition, 0.414 Jupiter masses, 131.6 M⊕ (Table 1: 10^T⁽²⁾ = 0.414
/// +0.057 −0.065 `M_J`), converted with the IAU 2015 nominal mass parameters.
pub const NEPTUNIAN_JOVIAN_TRANSITION: EarthMasses =
    EarthMasses::new(0.414 * GM_JUPITER / GM_EARTH);

/// The Jovian-to-stellar transition, 0.0800 solar masses, 26,636 M⊕ (Table 1: 10^T⁽³⁾ = 0.0800
/// +0.0081 −0.0072 M☉).
pub const JOVIAN_STELLAR_TRANSITION: EarthMasses = EarthMasses::new(0.0800 * GM_SUN / GM_EARTH);

/// Chen and Kipping's four classes of body, each a segment of their broken power law, named for a
/// typical member (their §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MassRadiusClass {
    /// Below 2.04 M⊕: rocky worlds such as Earth.
    Terran,
    /// From 2.04 M⊕ to 0.414 Jupiter masses: worlds with volatile envelopes, such as Neptune.
    Neptunian,
    /// From 0.414 Jupiter masses to 0.08 solar masses: gas giants and brown dwarfs, whose radius
    /// barely changes with mass.
    Jovian,
    /// From 0.08 solar masses: hydrogen-burning stars.
    Stellar,
}

impl MassRadiusClass {
    /// The class of a body of mass `mass`. A mass on a transition belongs to the heavier class.
    #[must_use]
    pub fn of(mass: EarthMasses) -> Self {
        let m = mass.value();
        if m < TERRAN_NEPTUNIAN_TRANSITION.value() {
            Self::Terran
        } else if m < NEPTUNIAN_JOVIAN_TRANSITION.value() {
            Self::Neptunian
        } else if m < JOVIAN_STELLAR_TRANSITION.value() {
            Self::Jovian
        } else {
            Self::Stellar
        }
    }

    /// The class's power-law index S, R ∝ M^S.
    #[must_use]
    pub const fn index(self) -> f64 {
        match self {
            Self::Terran => TERRAN_INDEX,
            Self::Neptunian => NEPTUNIAN_INDEX,
            Self::Jovian => JOVIAN_INDEX,
            Self::Stellar => STELLAR_INDEX,
        }
    }

    /// The class's scatter σ of log₁₀ R about its relation.
    #[must_use]
    pub const fn scatter(self) -> Dex {
        match self {
            Self::Terran => TERRAN_SCATTER,
            Self::Neptunian => NEPTUNIAN_SCATTER,
            Self::Jovian => JOVIAN_SCATTER,
            Self::Stellar => STELLAR_SCATTER,
        }
    }

    /// The class's offset C, log₁₀ R⊕ at 1 M⊕ on its segment's line: C⁽¹⁾ from the Terran radius
    /// and each next one from the continuity of the relation at the transitions, C⁽ʲ⁺¹⁾ = C⁽ʲ⁾ +
    /// (S⁽ʲ⁾ − S⁽ʲ⁺¹⁾) T⁽ʲ⁾ (Chen and Kipping 2017, eq. 6), in that order.
    #[must_use]
    fn offset(self) -> f64 {
        let terran = math::log10(TERRAN_RADIUS_AT_ONE_EARTH_MASS.value());
        let neptunian = terran
            + (TERRAN_INDEX - NEPTUNIAN_INDEX) * math::log10(TERRAN_NEPTUNIAN_TRANSITION.value());
        let jovian = neptunian
            + (NEPTUNIAN_INDEX - JOVIAN_INDEX) * math::log10(NEPTUNIAN_JOVIAN_TRANSITION.value());
        let stellar = jovian
            + (JOVIAN_INDEX - STELLAR_INDEX) * math::log10(JOVIAN_STELLAR_TRANSITION.value());
        match self {
            Self::Terran => terran,
            Self::Neptunian => neptunian,
            Self::Jovian => jovian,
            Self::Stellar => stellar,
        }
    }
}

/// The radius of a body of mass `mass` at rank `quantile` of Chen and Kipping's (2017) scatter at
/// that mass: log₁₀ R = C + S log₁₀ M + σ Φ⁻¹(q), on the segment of [`MassRadiusClass::of`].
///
/// This is the first step of design note 8: a body's one drawn number, its `planet.radius`
/// quantile, places it within the observed spread of radii at its mass, and stands for the
/// composition its class does not fix. The median ([`UnitUniform::HALF`]) is continuous across
/// the transitions; other quantiles step there, since each segment has its own scatter, as in the
/// paper's model.
///
/// The relation was fitted from 2 × 10²¹ kg (3 × 10⁻⁴ M⊕, their cut between Iapetus, the heaviest
/// body known not to be in hydrostatic equilibrium, and Rhea, the lightest known to be) to 0.87 M☉
/// (the heaviest star on the main sequence within a Hubble time; their §2.2 and §5.1), and is
/// extrapolated outside that range.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
///
/// # Examples
///
/// Earth's mass gives an Earth at the median, and a body at the 84th percentile of Terran
/// scatter is 10^0.0403 = 1.097 times larger:
///
/// ```
/// use hyperion_sim::planetary::derive::radius_chen_kipping;
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::units::EarthMasses;
///
/// let earth = EarthMasses::new(1.0);
/// let median = radius_chen_kipping(earth, UnitUniform::HALF).value();
/// assert!((median - 1.008).abs() < 1e-12);
/// let high = UnitUniform::new(0.841_344_746).expect("inside (0, 1)");
/// let ratio = radius_chen_kipping(earth, high).value() / median;
/// assert!((ratio - 1.097).abs() < 1e-3);
/// ```
#[must_use]
pub fn radius_chen_kipping(mass: EarthMasses, quantile: UnitUniform) -> EarthRadii {
    debug_assert!(
        mass.value().is_finite() && mass.value() > 0.0,
        "a mass is positive and finite, got {mass:?}"
    );
    let class = MassRadiusClass::of(mass);
    let log_r = class.offset()
        + class.index() * math::log10(mass.value())
        + class.scatter().value() * math::normal_quantile(quantile.value());
    EarthRadii::new(math::exp10(log_r))
}

/// Zeng, Sasselov and Jacobsen's (2016) Table 2: radius (R⊕ of 6,371 km) at masses of 0.125 × 2ⁱ
/// M⊕, i = 0 to 8, for (by column) 100%, 50%, 30%, 25% and 20% iron, pure rock, and 25%, 50% and
/// 100% water. Their iron is the core of Earth's seismic model extrapolated in pressure, their rock
/// its mantle.
const ZENG_TABLE: [[f64; 9]; 9] = [
    [0.445, 0.523, 0.547, 0.553, 0.558, 0.58, 0.649, 0.697, 0.776],
    [0.55, 0.645, 0.672, 0.679, 0.685, 0.711, 0.793, 0.851, 0.952],
    [0.676, 0.789, 0.823, 0.832, 0.84, 0.872, 0.969, 1.039, 1.163],
    [0.823, 0.961, 1.005, 1.016, 1.026, 1.067, 1.182, 1.27, 1.41],
    [0.99, 1.164, 1.22, 1.23, 1.25, 1.3, 1.44, 1.54, 1.71],
    [1.176, 1.4, 1.47, 1.49, 1.5, 1.57, 1.74, 1.85, 2.05],
    [1.38, 1.66, 1.75, 1.77, 1.79, 1.88, 2.07, 2.21, 2.45],
    [1.59, 1.94, 2.06, 2.08, 2.11, 2.22, 2.45, 2.61, 2.9],
    [1.82, 2.25, 2.38, 2.42, 2.45, 2.58, 2.85, 3.04, 3.36],
];

/// The lightest mass of [`ZENG_TABLE`], M⊕; each row doubles it.
const ZENG_FIRST_MASS: f64 = 0.125;

/// The core mass fractions of the table's dry curves, in increasing order, with their columns: the
/// radius at a mass falls as the fraction rises.
pub(crate) const DRY_CURVES: [(f64, usize); 6] =
    [(0.0, 5), (0.2, 4), (0.25, 3), (0.3, 2), (0.5, 1), (1.0, 0)];

/// The table's column of pure water.
const WATER_COLUMN: usize = 8;

/// Earth's core mass fraction, 0.325 ± 0.001 (Zeng, Sasselov and Jacobsen 2016, Table 1): the
/// "Earth-like" composition of 32.5% iron.
pub const EARTH_CORE_MASS_FRACTION: f64 = 0.325;

/// Where `mass` falls in [`ZENG_TABLE`]: the row below it and the fraction of a doubling past that
/// row, outside the table measured from the first or the last interval, which extrapolate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TablePosition {
    row: usize,
    past: f64,
}

impl TablePosition {
    /// The position of `mass` (M⊕, positive).
    #[must_use]
    pub(crate) fn of(mass: EarthMasses) -> Self {
        let doublings = math::log2(mass.value() / ZENG_FIRST_MASS);
        let mut row: u8 = 7;
        while row > 0 && doublings < f64::from(row) {
            row -= 1;
        }
        Self {
            row: usize::from(row),
            past: doublings - f64::from(row),
        }
    }

    /// The radius of the table's column `column` at this position, R⊕: log R linear in log M.
    #[must_use]
    pub(crate) fn radius(self, column: usize) -> f64 {
        let below = ZENG_TABLE[self.row][column];
        let above = ZENG_TABLE[self.row + 1][column];
        below * math::powf(above / below, self.past)
    }

    /// The radii of the dry curves at this position, R⊕, in the order of [`DRY_CURVES`].
    #[must_use]
    pub(crate) fn dry_radii(self) -> [f64; 6] {
        DRY_CURVES.map(|(_, column)| self.radius(column))
    }

    /// The radius of pure water at this position, R⊕.
    #[must_use]
    pub(crate) fn water_radius(self) -> f64 {
        self.radius(WATER_COLUMN)
    }
}

/// The radius of a dry body of core mass fraction `cmf` from the dry curves' radii `radii` at its
/// mass: linear in the fraction between the two curves that bracket it.
#[must_use]
pub(crate) fn dry_radius(radii: &[f64; 6], cmf: f64) -> f64 {
    let mut upper = 1;
    while upper < DRY_CURVES.len() - 1 && cmf > DRY_CURVES[upper].0 {
        upper += 1;
    }
    let (low, high) = (DRY_CURVES[upper - 1].0, DRY_CURVES[upper].0);
    let share = (cmf - low) / (high - low);
    radii[upper - 1] + share * (radii[upper] - radii[upper - 1])
}

/// How far a water mass fraction `x` moves a body from its dry curve to pure water: g(x) = (0.55 x
/// − 0.14 x²) ÷ 0.41, Zeng et al.'s (2019) f(x) − 1 scaled to reach 1 at pure water.
#[must_use]
pub(crate) fn water_blend(x: f64) -> f64 {
    (0.55 * x - 0.14 * x * x) / 0.41
}

/// The water mass fraction whose [`water_blend`] is `g` (0–1): the smaller root of 0.14 x² −
/// 0.55 x + 0.41 g = 0.
#[must_use]
pub(crate) fn water_fraction_of_blend(g: f64) -> f64 {
    (0.55 - (0.55 * 0.55 - 4.0 * 0.14 * 0.41 * g).sqrt()) / (2.0 * 0.14)
}

/// What a body without a hydrogen envelope is made of: its water mass fraction and the core mass
/// fraction of the rest, the share of iron in its rock and iron (Zeng et al.'s "CMF").
///
/// The named compositions are the curves plan 14 names: [`IRON`](Self::IRON),
/// [`EARTH_LIKE`](Self::EARTH_LIKE), [`ROCK`](Self::ROCK), [`HALF_WATER`](Self::HALF_WATER) and
/// [`WATER`](Self::WATER); water sits on an Earth-like core, as in Zeng et al. (2019).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CoreComposition {
    core_mass_fraction: f64,
    water_fraction: f64,
}

impl CoreComposition {
    /// Pure iron.
    pub const IRON: Self = Self {
        core_mass_fraction: 1.0,
        water_fraction: 0.0,
    };

    /// Earth's composition, 32.5% iron and 67.5% rock ([`EARTH_CORE_MASS_FRACTION`]).
    pub const EARTH_LIKE: Self = Self {
        core_mass_fraction: EARTH_CORE_MASS_FRACTION,
        water_fraction: 0.0,
    };

    /// Pure rock, with no iron.
    pub const ROCK: Self = Self {
        core_mass_fraction: 0.0,
        water_fraction: 0.0,
    };

    /// Half water by mass on an Earth-like core.
    pub const HALF_WATER: Self = Self {
        core_mass_fraction: EARTH_CORE_MASS_FRACTION,
        water_fraction: 0.5,
    };

    /// Pure water.
    pub const WATER: Self = Self {
        core_mass_fraction: EARTH_CORE_MASS_FRACTION,
        water_fraction: 1.0,
    };

    /// A composition from fractions the caller has already confined to 0–1.
    #[must_use]
    pub(crate) const fn from_fractions(core_mass_fraction: f64, water_fraction: f64) -> Self {
        Self {
            core_mass_fraction,
            water_fraction,
        }
    }

    /// A body whose water mass fraction is `water_fraction` and whose rock and iron are
    /// `core_mass_fraction` iron, both in 0–1.
    ///
    /// # Errors
    ///
    /// The [`BuildCoreCompositionError`] naming the first fraction outside 0–1 (NaN included).
    pub fn new(
        core_mass_fraction: f64,
        water_fraction: f64,
    ) -> Result<Self, BuildCoreCompositionError> {
        if !(0.0..=1.0).contains(&core_mass_fraction) {
            return Err(BuildCoreCompositionError::CoreMassFractionOutOfRange);
        }
        if !(0.0..=1.0).contains(&water_fraction) {
            return Err(BuildCoreCompositionError::WaterFractionOutOfRange);
        }
        Ok(Self {
            core_mass_fraction,
            water_fraction,
        })
    }

    /// The share of iron in the rock and iron, 0–1.
    #[must_use]
    pub const fn core_mass_fraction(self) -> f64 {
        self.core_mass_fraction
    }

    /// The mass fraction of water, 0–1.
    #[must_use]
    pub const fn water_fraction(self) -> f64 {
        self.water_fraction
    }

    /// The mass fraction of iron, 0–1.
    #[must_use]
    pub fn iron_fraction(self) -> f64 {
        self.core_mass_fraction * (1.0 - self.water_fraction)
    }

    /// The mass fraction of rock, 0–1.
    #[must_use]
    pub fn rock_fraction(self) -> f64 {
        (1.0 - self.core_mass_fraction) * (1.0 - self.water_fraction)
    }
}

/// A [`CoreComposition`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildCoreCompositionError {
    /// The core mass fraction was outside 0–1.
    CoreMassFractionOutOfRange,
    /// The water mass fraction was outside 0–1.
    WaterFractionOutOfRange,
}

impl fmt::Display for BuildCoreCompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CoreMassFractionOutOfRange => "a core mass fraction must lie in 0 to 1",
            Self::WaterFractionOutOfRange => "a water mass fraction must lie in 0 to 1",
        })
    }
}

impl Error for BuildCoreCompositionError {}

/// The radius of a body of mass `mass` and composition `core`, with no hydrogen envelope, from
/// Zeng et al.'s curves (see the [module](self) documentation).
///
/// The dry radius is interpolated between the table's iron, rock and mixed curves in the core
/// mass fraction, and water moves it towards the pure-water curve by Zeng et al.'s (2019) factor.
/// Masses outside 0.125–32 M⊕ extrapolate each curve along its last interval in log M and log R.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
///
/// # Examples
///
/// Mercury, 70% iron by mass (Santerne et al. 2018), is far denser than an Earth-like body of its
/// mass would be:
///
/// ```
/// use hyperion_sim::planetary::derive::radius::{CoreComposition, radius_zeng};
/// use hyperion_sim::units::EarthMasses;
///
/// let mass = EarthMasses::new(0.0553);
/// let mercury = radius_zeng(mass, CoreComposition::new(0.7, 0.0)?).value();
/// assert!((mercury - 0.383).abs() < 0.005);
/// assert!(radius_zeng(mass, CoreComposition::EARTH_LIKE).value() > 1.1 * mercury);
/// # Ok::<(), hyperion_sim::planetary::derive::radius::BuildCoreCompositionError>(())
/// ```
#[must_use]
pub fn radius_zeng(mass: EarthMasses, core: CoreComposition) -> EarthRadii {
    debug_assert!(
        mass.value().is_finite() && mass.value() > 0.0,
        "a mass is positive and finite, got {mass:?}"
    );
    let position = TablePosition::of(mass);
    let dry = dry_radius(&position.dry_radii(), core.core_mass_fraction);
    let water = position.water_radius();
    EarthRadii::new(dry + water_blend(core.water_fraction) * (water - dry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M};
    use crate::units::{JupiterMasses, JupiterRadii};

    fn rank(q: f64) -> UnitUniform {
        UnitUniform::new(q).unwrap()
    }

    fn median(mass: f64) -> f64 {
        radius_chen_kipping(EarthMasses::new(mass), UnitUniform::HALF).value()
    }

    #[test]
    fn the_median_is_continuous_across_every_transition() {
        for transition in [
            TERRAN_NEPTUNIAN_TRANSITION,
            NEPTUNIAN_JOVIAN_TRANSITION,
            JOVIAN_STELLAR_TRANSITION,
        ] {
            let m = transition.value();
            let (below, above) = (median(m * (1.0 - 1e-12)), median(m));
            assert!(
                (above / below - 1.0).abs() < 1e-10,
                "{below} R⊕ below {m} M⊕ and {above} R⊕ at it"
            );
        }
        assert_eq!(
            MassRadiusClass::of(TERRAN_NEPTUNIAN_TRANSITION),
            MassRadiusClass::Neptunian
        );
        assert!((NEPTUNIAN_JOVIAN_TRANSITION.value() - 131.58).abs() < 0.01);
        assert!((JOVIAN_STELLAR_TRANSITION.value() - 26_635.7).abs() < 0.1);
    }

    #[test]
    fn radius_rises_with_the_quantile_by_the_segment_s_scatter() {
        let masses = [0.01, 1.0, 5.0, 80.0, 300.0, 1e4, 1e5];
        for m in masses {
            let mut last = 0.0;
            for i in 1..100_u32 {
                let r = radius_chen_kipping(EarthMasses::new(m), rank(f64::from(i) / 100.0));
                assert!(r.value() > last, "{m} M⊕ at the {i}th percentile");
                last = r.value();
            }
            // One standard deviation up is 10^σ of the segment.
            let class = MassRadiusClass::of(EarthMasses::new(m));
            let up = radius_chen_kipping(EarthMasses::new(m), rank(0.841_344_746_068_542_9));
            let expected = math::exp10(class.scatter().value());
            assert!((up.value() / median(m) / expected - 1.0).abs() < 1e-8);
        }
    }

    #[test]
    fn the_median_earth_is_earth_and_the_median_jupiter_is_an_inflated_one() {
        let earth = median(1.0);
        assert!((earth - 1.0).abs() < 0.1, "{earth} R⊕");
        assert!((earth - 1.008).abs() < 1e-12);
        // Plan 14 asks for 1.0 `R_J` to 10% at 1 `M_J`. Chen and Kipping's Table 1 gives 13.77 R⊕,
        // 1.23 `R_J`: their Jovian segment is fitted mostly to irradiated hot Jupiters, and Jupiter
        // itself (11.2 R⊕, their data table) lies 0.09 dex, 1.2σ, below it. Jupiter's own radius
        // is P14.T11.d's, from plan 13's cooling fit.
        let jupiter = EarthMasses::from(JupiterMasses::new(1.0)).value();
        let r = median(jupiter);
        assert!((r - 13.77).abs() < 0.01, "{r} R⊕");
        let in_jupiters = JupiterRadii::from(EarthRadii::new(r)).value();
        assert!((in_jupiters - 1.227).abs() < 0.002, "{in_jupiters} R_J");
    }

    #[test]
    fn the_segments_follow_table_1_of_chen_and_kipping() {
        // On each segment R ∝ M^S: the median at twice a mass is 2^S of it.
        for (m, s) in [
            (0.1, TERRAN_INDEX),
            (10.0, NEPTUNIAN_INDEX),
            (1_000.0, JOVIAN_INDEX),
            (50_000.0, STELLAR_INDEX),
        ] {
            let ratio = median(2.0 * m) / median(m);
            assert!((ratio - math::powf(2.0, s)).abs() < 1e-12, "{m} M⊕");
        }
        // A 0.5 M☉ star at the median: 0.52 R☉, about right for an early M dwarf.
        let star = median(0.5 * GM_SUN / GM_EARTH) * EARTH_RADIUS_M / 6.957e8;
        assert!((0.4..0.55).contains(&star), "{star} R☉");
    }

    // Mass (kg), core mass fraction and volumetric mean radius (km) of the four rocky planets:
    // masses and radii from NASA's planetary fact sheets (Williams, fetched 2026-09-23), core mass
    // fractions Earth's 0.325 and Venus's 0.31 from Zeng et al. (2016, Table 1), Mars's 0.2 from
    // McSween (2003) as Zeng et al. (2016, §3.2) quote it, and Mercury's "about 70% metallic core"
    // from Santerne et al. (2018, Nature Astronomy 2, 393, abstract).
    const ROCKY_PLANETS: [(&str, f64, f64, f64); 4] = [
        ("Mercury", 0.330_10e24, 0.70, 2_439.7),
        ("Venus", 4.867_3e24, 0.31, 6_051.8),
        ("Earth", 5.972_2e24, EARTH_CORE_MASS_FRACTION, 6_371.0),
        ("Mars", 0.641_69e24, 0.2, 3_389.5),
    ];

    #[test]
    fn the_rocky_planets_radii_follow_from_their_masses_and_compositions() {
        for (name, kg, cmf, km) in ROCKY_PLANETS {
            let mass = EarthMasses::new(kg / EARTH_MASS_KG);
            let r = radius_zeng(mass, CoreComposition::new(cmf, 0.0).unwrap());
            let actual = km * 1e3 / EARTH_RADIUS_M;
            let error = r.value() / actual - 1.0;
            assert!(
                error.abs() < 0.05,
                "{name}: {} R⊕ against {actual}",
                r.value()
            );
            assert!(error.abs() < 0.006, "{name}: {error:+.4}");
        }
    }

    #[test]
    fn the_table_agrees_with_zeng_s_power_law_where_it_holds() {
        // Their eq. 2, for 1–8 M⊕ and core mass fractions 0–0.4: within 0.03 R⊕. They state 0.01;
        // their own 20–30% curves at 8 M⊕ lie 0.014–0.017 below it, and 40% iron, between the 30%
        // and 50% curves, 0.025.
        for m in [1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0] {
            for cmf in [0.0, 0.1, 0.2, 0.25, EARTH_CORE_MASS_FRACTION, 0.4] {
                let table =
                    radius_zeng(EarthMasses::new(m), CoreComposition::new(cmf, 0.0).unwrap());
                let fit = (1.07 - 0.21 * cmf) * math::powf(m, 1.0 / 3.7);
                assert!(
                    (table.value() - fit).abs() < 0.03,
                    "{m} M⊕, CMF {cmf}: {} against {fit}",
                    table.value()
                );
            }
        }
    }

    #[test]
    fn water_follows_zeng_s_growth_model_factor() {
        // On an Earth-like core, R = (1 + 0.55 x − 0.14 x²) `R_Earth`-like (Zeng et al. 2019), to the
        // 1.2% by which the table's pure-water curve is not exactly 1.41 Earth-like radii (1.426 at
        // 0.125 M⊕, 1.411 at 1 M⊕).
        for m in [0.2, 1.0, 5.0, 20.0] {
            let mass = EarthMasses::new(m);
            let earth_like = radius_zeng(mass, CoreComposition::EARTH_LIKE).value();
            for x in [0.1, 0.25, 0.5, 2.0 / 3.0, 1.0] {
                let r = radius_zeng(
                    mass,
                    CoreComposition::new(EARTH_CORE_MASS_FRACTION, x).unwrap(),
                );
                let factor = 1.0 + 0.55 * x - 0.14 * x * x;
                assert!(
                    (r.value() / (factor * earth_like) - 1.0).abs() < 0.012,
                    "{m} M⊕, {x}"
                );
            }
        }
        // Zeng et al. (2016)'s own 25% and 50% water curves, on pure rock, to 1.2% (1.15% for 25%
        // water at 0.125 M⊕).
        for (row, masses) in ZENG_TABLE.iter().enumerate() {
            let mass =
                EarthMasses::new(ZENG_FIRST_MASS * math::powi(2.0, i32::try_from(row).unwrap()));
            for (x, column) in [(0.25, 6), (0.5, 7)] {
                let r = radius_zeng(mass, CoreComposition::new(0.0, x).unwrap()).value();
                assert!((r / masses[column] - 1.0).abs() < 0.012, "{mass:?}, {x}");
            }
        }
        // g reaches 1 at pure water and inverts.
        assert!((water_blend(1.0) - 1.0).abs() < 1e-15);
        for x in [0.0, 0.001, 0.3, 0.539, 1.0] {
            assert!((water_fraction_of_blend(water_blend(x)) - x).abs() < 1e-12);
        }
    }

    #[test]
    fn the_named_curves_are_ordered_and_reproduce_the_table_at_its_rows() {
        for (row, radii) in ZENG_TABLE.iter().enumerate() {
            let mass =
                EarthMasses::new(ZENG_FIRST_MASS * math::powi(2.0, i32::try_from(row).unwrap()));
            let iron = radius_zeng(mass, CoreComposition::IRON).value();
            assert!((iron - radii[0]).abs() < 1e-12, "{mass:?}");
            let rock = radius_zeng(mass, CoreComposition::ROCK).value();
            assert!((rock - radii[5]).abs() < 1e-12);
            let water = radius_zeng(mass, CoreComposition::WATER).value();
            assert!((water - radii[8]).abs() < 1e-12);
            let earth = radius_zeng(mass, CoreComposition::EARTH_LIKE).value();
            let half = radius_zeng(mass, CoreComposition::HALF_WATER).value();
            assert!(iron < earth && earth < rock && rock < half && half < water);
        }
        // Extrapolated curves stay ordered from Ceres's mass to a 130 M⊕ core.
        for m in [1.6e-4, 0.01, 0.06, 50.0, 130.0] {
            let mass = EarthMasses::new(m);
            let radii = TablePosition::of(mass).dry_radii();
            assert!(radii.windows(2).all(|w| w[0] > w[1]), "{m} M⊕: {radii:?}");
            assert!(TablePosition::of(mass).water_radius() > radii[0]);
        }
    }

    #[test]
    fn compositions_are_validated_once() {
        assert!(CoreComposition::new(0.5, 0.5).is_ok());
        assert_eq!(
            CoreComposition::new(1.1, 0.0),
            Err(BuildCoreCompositionError::CoreMassFractionOutOfRange)
        );
        assert_eq!(
            CoreComposition::new(0.3, f64::NAN),
            Err(BuildCoreCompositionError::WaterFractionOutOfRange)
        );
        let c = CoreComposition::HALF_WATER;
        let sum = c.iron_fraction() + c.rock_fraction() + c.water_fraction();
        assert!((sum - 1.0).abs() < 1e-15);
        let text = BuildCoreCompositionError::WaterFractionOutOfRange.to_string();
        assert!(text.starts_with('a') && !text.ends_with('.'));
    }
}
