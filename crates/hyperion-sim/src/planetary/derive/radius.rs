//! Radius from mass (plan 14, P14.T11.a, T11.b and T11.d; design note 8).
//!
//! Two relations, used in turn. [`radius_chen_kipping`] places a body within the observed scatter
//! of radius at its mass, through the one quantile the body draws; [`radius_zeng`] is the radius of
//! a body of known iron, rock and water, which [`composition`](mod@super::composition) inverts to turn
//! that radius into a composition. Hydrogen and helium envelopes are
//! [`envelope`](super::envelope)'s. From 0.3 Jupiter masses a body is a giant, whose radius is
//! [`radius_giant`]'s.
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
//!
//! # Giants (P14.T11.d)
//!
//! A giant's radius is that of its interior, plan 13's
//! [`giant_cooling`](crate::stellar::substellar::giant_cooling) at the body's age, taken as the
//! [`CoolingState`] it returns, the one cooling type this step reads (ruling 34). That fit covers
//! 0.3–13 Jupiter masses, every giant plan 14 places. Three things are added to it.
//!
//! 1. **Inflation.** Giants receiving more than about 0.2 × 10⁹ erg s⁻¹ cm⁻², an equilibrium
//!    temperature near 1,000 K, are larger than cooling alone allows. Thorngren and Fortney (2018,
//!    AJ 155, 214; arXiv:1709.04539v2) fit the heating of the interior that 281 transiting giants
//!    above 0.5 Jupiter masses need, as a fraction ε of the flux F they receive, and favour a
//!    Gaussian in log F (their eq. 34): ε = 2.37% × exp[−(log₁₀ F − 0.14)² ÷ (2 × 0.37²)], F in
//!    10⁹ erg s⁻¹ cm⁻² ([`heating_efficiency`]). It peaks at 1,570 K (their abstract: "a
//!    maximum of ∼2.5% at `T_eq` ≈ 1500 K") and falls to 0.2% at 2,500 K. A giant whose interior
//!    has come to equilibrium radiates what it absorbs, so its own temperature is then ε^¼ `T_eq`
//!    (their eq. 35), with `T_eq` = (F ÷ 4σ)^¼, their black body with the heat spread over the
//!    whole planet. The radius at that equilibrium is their models' own: the radius of a 5 Gyr
//!    planet (their §5) of their mean composition and fitted heating, which their Fig. 2 draws at
//!    `T_eq` = 500, 1,000, 1,250, 1,500 and 2,000 K against mass
//!    ([`GiantRadius::inflated_radius`]). A giant takes the larger of its cooling
//!    radius and that one: young, it is still larger than its equilibrium, and it contracts onto it
//!    and stays (their Fig. 4). Between the figure's temperatures the radius is linear in `T_eq`,
//!    and beyond 2,000 K the 1,500–2,000 K line is carried on. That is an extrapolation. The heat
//!    ε F falls beyond 1,880 K, but their Gaussian model's radius, which their Fig. 12 draws for
//!    six bins of mass, still rises in every bin as far as it is drawn, about 2,500 K: at
//!    2,500 K the carried-on line is 0.2% above it at 0.9–1.2 Jupiter masses and 6–8% above it
//!    from 1.2 to 10.
//!
//!    Below [`GIANT_INFLATION_ONSET`], 1,000 K, the plan's threshold and theirs, the model radius
//!    is faded out, linearly in `T_eq`, to nothing at [`GIANT_INFLATION_FADE_START`], 500 K. There
//!    their 5 Gyr planets are still cooling, not held by their heating, and a 5 Gyr radius held at
//!    every later age would keep old giants up: at 500 K by up to 4.3%, at 13 Jupiter masses and
//!    13.8 Gyr, where plan 13's fit joins plan 06's brown dwarfs. With the fade no giant cooler
//!    than 900 K is held above its cooling radius at any age up to 13.8 Gyr (tested).
//! 2. **The cap**, [`GIANT_RADIUS_CAP`], at 2 Jupiter radii.
//! 3. **The blend.** Below 0.414 Jupiter masses, where Chen and Kipping's Jovian class ends, the
//!    radius is blended, in its logarithm and linearly in log mass, into the radius the body has
//!    without this step ([`GiantRadius::blended`]): its Chen and Kipping radius, through the
//!    composition solve and the envelope model. [`giant_share`] is the weight, 0 at 0.3 and 1 at
//!    0.414 Jupiter masses, so the radius is continuous at both. Plan 13's fit is coreless, and so
//!    12% too large at Saturn's mass (ruling 50), which the blend leaves to the envelope model.
//!
//! **Why the models' radius and not the cooling fit's.** The direct reading, the cooling track's
//! own state at the temperature ε^¼ `T_eq`, puts HD 209458 b at 1.47 Jupiter radii against its
//! 1.36, and, since the heat ε F peaks at 1,880 K, shrinks the planets beyond it as they are
//! heated more; against 650 transiting giants from the NASA Exoplanet Archive it is 11–13% large
//! at 1,000–1,500 K and 23% small beyond 2,500 K. The fit's objects are isolated and of solar
//! composition, and Thorngren and Fortney's have irradiated atmospheres and their heavy elements,
//! which set the radius at a given heating. With their models' radii, as used here, the same
//! planets' median comes out between 1.0% small and 5.9% large in each band of temperature below
//! 2,500 K (under 700 K, 700–1,000 K, 250 K wide to 2,000 K, and 2,000–2,500 K), and 9.9% large
//! for the eleven planets beyond.
//!
//! **Their Fig. 2, as read.** The five lines were read from the figure's vector paths in the
//! arXiv PDF, with the axes calibrated at their tick marks (10⁻¹, 1 and 10 Jupiter masses; 0 to 2
//! Jupiter radii). Each line is drawn through the same 50 masses spaced evenly in log mass from
//! 0.05 to 12 Jupiter masses, to 5 × 10⁻⁵ dex, and [`TF_RADII`] keeps them from 0.299 Jupiter
//! masses. The figure uses their Gaussian-process model of ε, which their DIC does not tell from
//! eq. 34's (−1,723 both). The 2,000 K line leaves the plot, at 2.46 Jupiter radii, below 0.586
//! Jupiter masses; its value at the next mass down is recovered from where its segment is cut, and
//! the lighter ones carry that segment on. They set where, between 1,500 and 2,000 K, planets of
//! 0.3–0.55 Jupiter masses reach the cap.
//!
//! The same models inflate planets below 0.5 Jupiter masses, which Thorngren and Fortney left out
//! of their sample, far more than such planets are seen to be: none is observed with a surface
//! gravity under about 3 m s⁻² (their §2 and Fig. 2). Below 0.414 Jupiter masses the blend holds
//! them back; between 0.414 and 0.5 only the cap does, and a giant of 0.42 Jupiter masses at the
//! cap has 2.6 m s⁻².

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::params::{
    GIANT_INFLATION_FADE_START, GIANT_INFLATION_ONSET, GIANT_RADIUS_CAP,
};
use crate::stellar::draws::UnitUniform;
use crate::stellar::substellar::{CoolingState, GIANT_MAX_MASS, GIANT_MIN_MASS};
use crate::units::consts::{GM_EARTH, GM_JUPITER, GM_SUN};
use crate::units::{
    Dex, EarthFluxes, EarthMasses, EarthRadii, JupiterMasses, JupiterRadii, Kelvin, Metres,
    SolarLuminosities, Watts, WattsPerSquareMetre,
};

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

/// The peak of Thorngren and Fortney's heating efficiency, 2.37% of the incident flux (2018, eq.
/// 34: 2.37 +1.3 −0.26 %).
pub const HEATING_EFFICIENCY_PEAK: f64 = 0.0237;

/// Where the heating efficiency peaks: log₁₀ F = 0.14, F in 10⁹ erg s⁻¹ cm⁻² (Thorngren and
/// Fortney 2018, eq. 34: 0.14 +0.060 −0.069), 1.38 × 10⁶ W m⁻², an equilibrium temperature of
/// 1,570 K.
pub const HEATING_PEAK_LOG_FLUX: f64 = 0.14;

/// The width of the heating efficiency's Gaussian in log₁₀ F, 0.37 dex (Thorngren and Fortney
/// 2018, eq. 34: 0.37 +0.038 −0.059).
pub const HEATING_LOG_FLUX_WIDTH: Dex = Dex::new(0.37);

/// Thorngren and Fortney's unit of flux, 10⁹ erg s⁻¹ cm⁻², in W m⁻².
const GIGA_ERG_PER_SECOND_PER_SQUARE_CENTIMETRE: f64 = 1e6;

/// The fraction of the flux `flux` arriving at a giant that heats its interior: Thorngren and
/// Fortney's (2018) eq. 34, ε = 2.37% × exp[−(log₁₀ F − 0.14)² ÷ (2 × 0.37²)], F in 10⁹ erg s⁻¹
/// cm⁻² (see the [module](self) documentation). Zero flux heats nothing.
///
/// Their fit is to giants above 0.5 Jupiter masses. It is used at every flux, and falls towards
/// zero on both sides of its peak.
///
/// # Panics
///
/// In debug builds, if `flux` is negative or not finite.
///
/// # Examples
///
/// The heating peaks near 1,570 K and is a tenth of that at 2,500 K:
///
/// ```
/// use hyperion_sim::planetary::derive::radius::heating_efficiency;
/// use hyperion_sim::units::{EarthFluxes, WattsPerSquareMetre};
///
/// // The flux whose black-body equilibrium temperature is t: 4σt⁴.
/// let at = |t: f64| {
///     let sigma = 5.670_374_419e-8;
///     EarthFluxes::from(WattsPerSquareMetre::new(4.0 * sigma * t * t * t * t))
/// };
/// assert!((heating_efficiency(at(1_570.0)) - 0.0237).abs() < 1e-5);
/// assert!((heating_efficiency(at(2_500.0)) - 0.0022).abs() < 1e-4);
/// ```
#[must_use]
pub fn heating_efficiency(flux: EarthFluxes) -> f64 {
    let s = WattsPerSquareMetre::from(flux).value();
    debug_assert!(s.is_finite() && s >= 0.0, "a flux is not negative, got {s}");
    if s <= 0.0 {
        return 0.0;
    }
    let x = math::log10(s / GIGA_ERG_PER_SECOND_PER_SQUARE_CENTIMETRE) - HEATING_PEAK_LOG_FLUX;
    let width = HEATING_LOG_FLUX_WIDTH.value();
    HEATING_EFFICIENCY_PEAK * math::exp(-x * x / (2.0 * width * width))
}

/// The equilibrium temperatures of Thorngren and Fortney's (2018) Fig. 2 lines, K, ascending.
pub const TF_TEMPERATURES: [f64; 5] = [500.0, 1_000.0, 1_250.0, 1_500.0, 2_000.0];

/// log₁₀ of the lightest mass of [`TF_RADII`], 0.299 Jupiter masses: the 17th of Thorngren and
/// Fortney's 50 masses spaced evenly in log mass from 0.05 to 12 Jupiter masses, log₁₀ 0.05 + 16 ×
/// [`TF_LOG_MASS_STEP`].
pub const TF_LOG_MASS_FIRST: f64 = -0.523_818_161_635_701_7;

/// The step between Thorngren and Fortney's masses: log₁₀(12 ÷ 0.05) ÷ 49 dex.
pub const TF_LOG_MASS_STEP: f64 = 0.048_575_739_626_767_47;

/// The number of masses in [`TF_RADII`], 0.299 to 12 Jupiter masses.
const TF_MASSES: u8 = 34;

/// Thorngren and Fortney's (2018) Fig. 2: the radius, in Jupiter radii (71,492 km), of a 5 Gyr
/// giant of their mean composition and fitted heating, at each temperature of
/// [`TF_TEMPERATURES`] (rows) and each of their masses from 0.299 to 12 Jupiter masses (columns;
/// [`TF_LOG_MASS_FIRST`] and [`TF_LOG_MASS_STEP`]), read from the figure's vector paths (see the
/// [module](self) documentation). The first six entries at 2,000 K lie above the figure's top: the
/// sixth is where the line's last segment, cut at the top, comes from, and the five before it
/// carry that segment on, a lower bound on a line that steepens there.
pub const TF_RADII: [[f64; 34]; 5] = [
    [
        0.8592, 0.8707, 0.8816, 0.8920, 0.9026, 0.9124, 0.9219, 0.9308, 0.9394, 0.9479, 0.9560,
        0.9642, 0.9721, 0.9798, 0.9871, 0.9942, 1.0007, 1.0067, 1.0120, 1.0167, 1.0207, 1.0238,
        1.0260, 1.0273, 1.0276, 1.0268, 1.0251, 1.0222, 1.0183, 1.0134, 1.0074, 1.0004, 0.9924,
        0.9835,
    ],
    [
        1.0575, 1.0504, 1.0445, 1.0395, 1.0365, 1.0341, 1.0325, 1.0316, 1.0317, 1.0325, 1.0337,
        1.0358, 1.0382, 1.0408, 1.0432, 1.0458, 1.0479, 1.0497, 1.0511, 1.0519, 1.0525, 1.0524,
        1.0517, 1.0504, 1.0485, 1.0457, 1.0422, 1.0378, 1.0327, 1.0267, 1.0198, 1.0119, 1.0033,
        0.9939,
    ],
    [
        1.4457, 1.3466, 1.2977, 1.2594, 1.2312, 1.2083, 1.1908, 1.1771, 1.1669, 1.1595, 1.1541,
        1.1507, 1.1484, 1.1469, 1.1458, 1.1453, 1.1447, 1.1437, 1.1425, 1.1409, 1.1389, 1.1360,
        1.1322, 1.1274, 1.1215, 1.1143, 1.1060, 1.0966, 1.0862, 1.0748, 1.0624, 1.0493, 1.0357,
        1.0217,
    ],
    [
        2.3864, 2.0006, 1.7921, 1.6553, 1.5634, 1.4920, 1.4351, 1.3918, 1.3593, 1.3344, 1.3147,
        1.2998, 1.2892, 1.2798, 1.2718, 1.2648, 1.2589, 1.2538, 1.2498, 1.2432, 1.2390, 1.2343,
        1.2291, 1.2234, 1.2117, 1.2022, 1.1915, 1.1794, 1.1662, 1.1517, 1.1361, 1.1194, 1.1018,
        1.0834,
    ],
    [
        4.9336, 4.4810, 4.0284, 3.5758, 3.1232, 2.6706, 2.2180, 1.9962, 1.8430, 1.7310, 1.6471,
        1.5884, 1.5451, 1.5088, 1.4804, 1.4604, 1.4459, 1.4323, 1.4230, 1.4122, 1.4044, 1.3970,
        1.3892, 1.3776, 1.3632, 1.3477, 1.3334, 1.3153, 1.2957, 1.2754, 1.2539, 1.2316, 1.2091,
        1.1856,
    ],
];

/// The radius of Thorngren and Fortney's 5 Gyr model of a giant of mass `mass` at equilibrium
/// temperature `t_eq`: [`TF_RADII`], linear in log mass and in `T_eq`, held at its lightest and
/// heaviest masses and below 500 K, and carried on from its last two temperatures beyond 2,000 K.
#[must_use]
fn thorngren_fortney_radius(mass: JupiterMasses, t_eq: Kelvin) -> JupiterRadii {
    let last = TF_MASSES - 1;
    let position = ((math::log10(mass.value()) - TF_LOG_MASS_FIRST) / TF_LOG_MASS_STEP)
        .clamp(0.0, f64::from(last));
    let mut node: u8 = 0;
    while node + 1 < last && position >= f64::from(node + 1) {
        node += 1;
    }
    let past = position - f64::from(node);
    let at = |row: usize| {
        let (below, above) = (
            TF_RADII[row][usize::from(node)],
            TF_RADII[row][usize::from(node) + 1],
        );
        below + past * (above - below)
    };
    let t = t_eq.value();
    if t <= TF_TEMPERATURES[0] {
        return JupiterRadii::new(at(0));
    }
    let mut upper = 1;
    while upper < TF_TEMPERATURES.len() - 1 && t > TF_TEMPERATURES[upper] {
        upper += 1;
    }
    let (cooler, hotter) = (TF_TEMPERATURES[upper - 1], TF_TEMPERATURES[upper]);
    let share = (t - cooler) / (hotter - cooler);
    let (low, high) = (at(upper - 1), at(upper));
    JupiterRadii::new(low + share * (high - low))
}

/// How much of Thorngren and Fortney's model radius holds a giant at equilibrium temperature
/// `t_eq`, 0–1: none up to [`GIANT_INFLATION_FADE_START`], all from [`GIANT_INFLATION_ONSET`], and
/// linear in `T_eq` between (see the [module](self) documentation).
#[must_use]
fn inflation_weight(t_eq: Kelvin) -> f64 {
    let (start, onset) = (
        GIANT_INFLATION_FADE_START.value(),
        GIANT_INFLATION_ONSET.value(),
    );
    ((t_eq.value() - start) / (onset - start)).clamp(0.0, 1.0)
}

/// How much of a body of mass `mass` is a giant, 0–1: 0 up to 0.3 Jupiter masses
/// ([`GIANT_MIN_MASS`]), 1 from 0.414 ([`NEPTUNIAN_JOVIAN_TRANSITION`]), and linear in log mass
/// between, the weight of [`GiantRadius::blended`] and of the composition's
/// [`GiantComposition::blended`](super::composition::GiantComposition::blended).
///
/// # Examples
///
/// Saturn, 0.2994 Jupiter masses, is below the giants, and a body of 0.35 Jupiter masses half a
/// giant:
///
/// ```
/// use hyperion_sim::planetary::derive::radius::giant_share;
/// use hyperion_sim::units::{EarthMasses, JupiterMasses};
///
/// assert!(giant_share(EarthMasses::new(95.16)).abs() < 1e-15);
/// let share = giant_share(EarthMasses::from(JupiterMasses::new(0.352)));
/// assert!((share - 0.5).abs() < 0.01);
/// assert!((giant_share(EarthMasses::new(318.0)) - 1.0).abs() < 1e-15);
/// ```
#[must_use]
pub fn giant_share(mass: EarthMasses) -> f64 {
    let m = mass.value();
    let low = EarthMasses::from(GIANT_MIN_MASS).value();
    let high = NEPTUNIAN_JOVIAN_TRANSITION.value();
    if m >= high {
        1.0
    } else if m <= low {
        0.0
    } else {
        math::ln(m / low) / math::ln(high / low)
    }
}

/// A giant's radius and internal heat (P14.T11.d), from [`radius_giant`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GiantRadius {
    radius: EarthRadii,
    cooling_radius: EarthRadii,
    inflated_radius: EarthRadii,
    internal_luminosity: SolarLuminosities,
    heating_efficiency: f64,
    share: f64,
}

impl GiantRadius {
    /// The giant's radius: the larger of its cooling radius and its
    /// [`inflated_radius`](Self::inflated_radius), capped at [`GIANT_RADIUS_CAP`]. It is the
    /// body's radius from 0.414 Jupiter masses; below that, [`blended`](Self::blended) is.
    #[must_use]
    pub const fn radius(&self) -> EarthRadii {
        self.radius
    }

    /// The radius of the giant's interior as it cooled, plan 13's, uncapped.
    #[must_use]
    pub const fn cooling_radius(&self) -> EarthRadii {
        self.cooling_radius
    }

    /// The radius at which the heating of the giant's interior balances its cooling: Thorngren
    /// and Fortney's (2018) model radius at its mass and equilibrium temperature, faded out below
    /// 1,000 K (see the [module](self) documentation). Uncapped.
    #[must_use]
    pub const fn inflated_radius(&self) -> EarthRadii {
        self.inflated_radius
    }

    /// Whether the heating holds the giant above its cooling radius.
    #[must_use]
    pub fn is_inflated(&self) -> bool {
        self.inflated_radius.value() > self.cooling_radius.value()
    }

    /// The fraction ε of the incident flux that heats the interior ([`heating_efficiency`]).
    #[must_use]
    pub const fn heating_efficiency(&self) -> f64 {
        self.heating_efficiency
    }

    /// How much of the body is a giant ([`giant_share`]).
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }

    /// The luminosity the body radiates of its own, for P14.T12.a's
    /// [`with_internal_heat`](super::irradiation::with_internal_heat): the larger of its cooling
    /// luminosity and the heat deposited in it, ε π R² F, which it radiates at equilibrium
    /// (Thorngren and Fortney 2018, eqs. 4 and 35), times [`share`](Self::share), so that it
    /// vanishes at 0.3 Jupiter masses with the model below, which carries none.
    #[must_use]
    pub fn internal_luminosity(&self) -> SolarLuminosities {
        self.internal_luminosity * self.share
    }

    /// The radius of a body whose radius without this step, its Chen and Kipping radius through
    /// the composition solve and the envelope model at its age, is `below`: the two blended in
    /// their logarithms by [`share`](Self::share), so `below` at 0.3 Jupiter masses and
    /// [`radius`](Self::radius) from 0.414. The cap is not applied to `below`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `below` is not positive and finite.
    #[must_use]
    pub fn blended(&self, below: EarthRadii) -> EarthRadii {
        debug_assert!(
            below.value().is_finite() && below.value() > 0.0,
            "a radius is positive and finite, got {below:?}"
        );
        if self.share >= 1.0 {
            self.radius
        } else if self.share <= 0.0 {
            below
        } else {
            let ln_r = (1.0 - self.share) * math::ln(below.value())
                + self.share * math::ln(self.radius.value());
            EarthRadii::new(math::exp(ln_r))
        }
    }
}

/// A giant's radius could not be derived.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveGiantError {
    /// The mass is outside plan 13's cooling fit, 0.3–13 Jupiter masses ([`GIANT_MIN_MASS`] and
    /// [`GIANT_MAX_MASS`]), or not a number.
    MassOutsideGiants(EarthMasses),
    /// The flux is negative or not finite.
    FluxNotValid(EarthFluxes),
}

impl fmt::Display for DeriveGiantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassOutsideGiants(m) => write!(
                f,
                "a mass of {} earth masses is outside the giant planets' {} to {} Jupiter masses",
                m.value(),
                GIANT_MIN_MASS.value(),
                GIANT_MAX_MASS.value()
            ),
            Self::FluxNotValid(s) => write!(
                f,
                "a flux of {} earth fluxes is negative or not finite",
                s.value()
            ),
        }
    }
}

impl Error for DeriveGiantError {}

/// Checks that `mass` lies in plan 13's cooling fit, 0.3–13 Jupiter masses, and returns it in
/// Jupiter masses.
pub(crate) fn giant_mass(mass: EarthMasses) -> Result<JupiterMasses, DeriveGiantError> {
    let jupiters = JupiterMasses::from(mass);
    if (GIANT_MIN_MASS.value()..=GIANT_MAX_MASS.value()).contains(&jupiters.value()) {
        Ok(jupiters)
    } else {
        Err(DeriveGiantError::MassOutsideGiants(mass))
    }
}

/// The radius of a giant planet of mass `mass` whose interior is `interior`, receiving `flux`
/// (P14.T11.d): the larger of the interior's radius and Thorngren and Fortney's equilibrium
/// radius at that flux, faded out below 1,000 K, capped at [`GIANT_RADIUS_CAP`] (see the
/// [module](self) documentation).
///
/// `interior` is plan 13's
/// [`giant_cooling`](crate::stellar::substellar::giant_cooling) of the same mass at the body's
/// age (ruling 34: this step reads a giant's [`CoolingState`] and no
/// [`StarState`](crate::stellar::StarState)), and `flux` the total the body receives from its
/// hosts, averaged over its orbit, as P14.T12.a's
/// [`total_flux`](super::irradiation::total_flux) gives it. The equilibrium temperature read here
/// is the flux's own, (F ÷ 4σ)^¼, with no albedo, as Thorngren and Fortney define it.
///
/// # Errors
///
/// [`DeriveGiantError::MassOutsideGiants`] outside 0.3–13 Jupiter masses, and
/// [`DeriveGiantError::FluxNotValid`] for a negative or non-finite flux.
///
/// # Examples
///
/// HD 209458 b, of 0.685 Jupiter masses at 0.047 au from a star of 1.62 L☉ (Torres, Winn and Holman
/// 2008), is 1.36 Jupiter radii across, where cooling alone would leave it under 1:
///
/// ```
/// use hyperion_sim::planetary::derive::radius::radius_giant;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::substellar::giant_cooling;
/// use hyperion_sim::units::{EarthFluxes, EarthMasses, JupiterMasses, JupiterRadii, Years};
///
/// let mass = JupiterMasses::new(0.685);
/// let interior = giant_cooling(mass, Years::new(3.1e9), &Composition::SOLAR)?;
/// let flux = EarthFluxes::new(1.622 / (0.047_07 * 0.047_07));
/// let giant = radius_giant(EarthMasses::from(mass), &interior, flux)?;
/// let r = JupiterRadii::from(giant.radius()).value();
/// assert!((r / 1.359 - 1.0).abs() < 0.05);
/// assert!(JupiterRadii::from(interior.radius()).value() < 1.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn radius_giant(
    mass: EarthMasses,
    interior: &CoolingState,
    flux: EarthFluxes,
) -> Result<GiantRadius, DeriveGiantError> {
    let jupiters = giant_mass(mass)?;
    if !(flux.value().is_finite() && flux.value() >= 0.0) {
        return Err(DeriveGiantError::FluxNotValid(flux));
    }
    let t_eq = equilibrium_temperature(flux, BondAlbedo::from_fraction(0.0));
    let inflated =
        EarthRadii::from(thorngren_fortney_radius(jupiters, t_eq)) * inflation_weight(t_eq);
    let cooled = EarthRadii::from(interior.radius());
    let cap = EarthRadii::from(GIANT_RADIUS_CAP);
    let radius = EarthRadii::new(cooled.value().max(inflated.value()).min(cap.value()));
    let heating_efficiency = heating_efficiency(flux);
    let r = Metres::from(radius).value();
    let deposited = Watts::new(
        heating_efficiency
            * core::f64::consts::PI
            * r
            * r
            * WattsPerSquareMetre::from(flux).value(),
    );
    let internal = interior
        .luminosity()
        .value()
        .max(SolarLuminosities::from(deposited).value());
    Ok(GiantRadius {
        radius,
        cooling_radius: cooled,
        inflated_radius: inflated,
        internal_luminosity: SolarLuminosities::new(internal),
        heating_efficiency,
        share: giant_share(mass),
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::derive::composition::{SnowLineSide, composition};
    use crate::planetary::derive::envelope::radius_with_envelope;
    use crate::planetary::params::OUTER_WATER_CAP;
    use crate::stellar::Composition;
    use crate::stellar::substellar::giant_cooling;
    use crate::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M, STEFAN_BOLTZMANN};
    use crate::units::{Gigayears, HeliumExcess, Years};

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

    // P14.T11.d: giants.

    fn interior(m_j: f64, gyr: f64) -> CoolingState {
        giant_cooling(
            JupiterMasses::new(m_j),
            Years::new(gyr * 1e9),
            &Composition::SOLAR,
        )
        .unwrap()
    }

    fn giant(m_j: f64, gyr: f64, flux: EarthFluxes) -> GiantRadius {
        let mass = EarthMasses::from(JupiterMasses::new(m_j));
        radius_giant(mass, &interior(m_j, gyr), flux).unwrap()
    }

    fn in_jupiters(r: EarthRadii) -> f64 {
        JupiterRadii::from(r).value()
    }

    /// The flux whose black-body equilibrium temperature, (F ÷ 4σ)^¼, is `t_eq`.
    fn flux_at(t_eq: f64) -> EarthFluxes {
        let s = 4.0 * STEFAN_BOLTZMANN * math::powi(t_eq, 4);
        EarthFluxes::from(WattsPerSquareMetre::new(s))
    }

    fn black_body(flux: EarthFluxes) -> f64 {
        equilibrium_temperature(flux, BondAlbedo::from_fraction(0.0)).value()
    }

    /// Plan 14's T11 test (d), with each bracket on the model it rests on. Masses, mean radii and
    /// semi-major axes from NASA's planetary fact sheets (fetched 2026-09-23); Jupiter's radius
    /// is also given at its 1 bar equator, the IAU's 71,492 km.
    #[test]
    fn the_giant_planets_radii_follow_from_their_masses() {
        // Jupiter rests on the cooling fit: at 4.57 Gyr and 5.2 au (T_eq 122 K) it is not
        // inflated, and its radius is plan 13's, 71,478 km.
        let jupiter = giant(1.0, 4.57, EarthFluxes::new(1.0 / (5.203_4 * 5.203_4)));
        assert!(!jupiter.is_inflated());
        assert_same_bits(jupiter.radius().value(), jupiter.cooling_radius().value());
        let km = Metres::from(jupiter.radius()).value() / 1e3;
        for actual in [71_492.0, 69_911.0] {
            assert!((km / actual - 1.0).abs() < 0.1, "Jupiter: {km} km");
        }
        assert!((km / 71_492.0 - 1.0).abs() < 0.001, "{km} km");

        // Saturn, 0.2994 Jupiter masses, is below the giants: T11.d gives it no share, and its
        // radius is the sub-giant model's, which with Thorngren et al.'s (2016, Table 1) 27 M⊕ of
        // heavy elements on the icy core of the composition solve gives 58,330 km against 58,232
        // (+0.2%). Neither Chen and Kipping's median, 11.82 R⊕ (+29%, Saturn lying 0.77σ below it,
        // at its 22nd percentile), nor the cooling fit's coreless 65,270 km at 0.3 Jupiter masses
        // (+12%, ruling 50) would hold it to 10%.
        let saturn = EarthMasses::new(568.32e24 / EARTH_MASS_KG);
        assert!(giant_share(saturn).abs() < f64::MIN_POSITIVE);
        let icy = CoreComposition::new(EARTH_CORE_MASS_FRACTION, OUTER_WATER_CAP).unwrap();
        let envelope = 1.0 - 27.0 / saturn.value();
        let flux = EarthFluxes::new(1.0 / (9.537 * 9.537));
        let r = radius_with_envelope(saturn, icy, envelope, flux, Gigayears::new(4.57)).value();
        let actual = 58_232e3 / EARTH_RADIUS_M;
        assert!(
            (r / actual - 1.0).abs() < 0.1,
            "Saturn: {r} R⊕ against {actual}"
        );
        let median = radius_chen_kipping(saturn, UnitUniform::HALF).value();
        let sigmas = math::log10(actual / median) / NEPTUNIAN_SCATTER.value();
        assert!((-0.8..-0.7).contains(&sigmas), "Saturn at {sigmas} σ");

        // Uranus and Neptune rest on Chen and Kipping. Uranus's median is 1.8% small. Neptune's
        // is 4.31 R⊕, 11.5% above its 3.865: Neptune lies 0.32σ below the relation, at its 37th
        // percentile, so the plan's 10% at the median is not the paper's (as P14.T11.a found for
        // Jupiter). Asserted for Neptune: the paper's median and Neptune inside one σ of it.
        let uranus = EarthMasses::new(86.811e24 / EARTH_MASS_KG);
        let r = radius_chen_kipping(uranus, UnitUniform::HALF).value();
        let actual = 25_362e3 / EARTH_RADIUS_M;
        assert!(
            (r / actual - 1.0).abs() < 0.1,
            "Uranus: {r} R⊕ against {actual}"
        );
        let neptune = EarthMasses::new(102.409e24 / EARTH_MASS_KG);
        let r = radius_chen_kipping(neptune, UnitUniform::HALF).value();
        let actual = 24_622e3 / EARTH_RADIUS_M;
        assert!((r - 4.31).abs() < 0.005, "Neptune's median: {r} R⊕");
        let sigmas = math::log10(actual / r) / NEPTUNIAN_SCATTER.value();
        assert!((-1.0..0.0).contains(&sigmas), "Neptune at {sigmas} σ");
        for body in [uranus, neptune] {
            assert!(giant_share(body).abs() < f64::MIN_POSITIVE);
        }
    }

    /// The radius a body has without P14.T11.d: Chen and Kipping's at rank `q`, through the
    /// composition solve beyond the snow line, and the envelope model at `gyr`.
    fn without_giants(mass: EarthMasses, q: f64, flux: EarthFluxes, gyr: f64) -> EarthRadii {
        let r = radius_chen_kipping(mass, rank(q));
        let solved = composition(mass, r, SnowLineSide::Beyond, flux).unwrap();
        let (core, fraction) = (solved.core(), solved.envelope_fraction());
        radius_with_envelope(mass, core, fraction, flux, Gigayears::new(gyr))
    }

    /// A body's radius as `derive_body` is to join the pieces: the model below up to 0.3 Jupiter
    /// masses, the giant's from 0.414, and the two blended between.
    fn body_radius(mass: EarthMasses, q: f64, flux: EarthFluxes, gyr: f64) -> f64 {
        if giant_share(mass) <= 0.0 {
            return without_giants(mass, q, flux, gyr).value();
        }
        let cooling = giant_cooling(
            JupiterMasses::from(mass),
            Years::new(gyr * 1e9),
            &Composition::SOLAR,
        )
        .unwrap();
        let giant = radius_giant(mass, &cooling, flux).unwrap();
        if giant_share(mass) >= 1.0 {
            giant.radius().value()
        } else {
            giant.blended(without_giants(mass, q, flux, gyr)).value()
        }
    }

    #[test]
    fn the_radius_is_continuous_in_mass_where_the_giants_begin() {
        let edges = [
            EarthMasses::from(GIANT_MIN_MASS).value(),
            NEPTUNIAN_JOVIAN_TRANSITION.value(),
        ];
        let mut worst: f64 = 0.0;
        for gyr in [0.01, 0.1, 1.0, 4.6, 10.0] {
            for s in [0.01, 1.0, 100.0, 1_000.0, 3_000.0] {
                let flux = EarthFluxes::new(s);
                for q in [0.1, 0.5, 0.9] {
                    for edge in edges {
                        let below =
                            body_radius(EarthMasses::new(edge * (1.0 - 1e-9)), q, flux, gyr);
                        let above =
                            body_radius(EarthMasses::new(edge * (1.0 + 1e-9)), q, flux, gyr);
                        let jump = (above / below - 1.0).abs();
                        // Plan 14 asks for 5%; the blend's weights make it exact but for the
                        // change of 2 × 10⁻⁹ in mass.
                        assert!(jump < 0.05, "{edge} M⊕, {gyr} Gyr, {s} F⊕, {q}: {jump}");
                        worst = worst.max(jump);
                    }
                    // And no step inside the blend.
                    let steps = 200_u32;
                    let mut last = body_radius(EarthMasses::new(edges[0]), q, flux, gyr);
                    for i in 1..=steps {
                        let m = edges[0]
                            * math::powf(edges[1] / edges[0], f64::from(i) / f64::from(steps));
                        let r = body_radius(EarthMasses::new(m), q, flux, gyr);
                        assert!(
                            (r / last - 1.0).abs() < 0.01,
                            "{m} M⊕, {gyr} Gyr, {s} F⊕, {q}"
                        );
                        last = r;
                    }
                }
            }
        }
        assert!(worst < 1e-6, "{worst}");
    }

    /// A transiting planet as Torres, Winn and Holman (2008, ApJ 677, 1324) give it in their
    /// Tables 1, 3 and 5: its mass (`M_J`), radius (`R_J`) and semi-major axis (au), its host's
    /// luminosity (L☉), age (Gyr) and \[Fe/H\], and their zero-albedo equilibrium temperature,
    /// T★ (R★ ÷ 2a)^½ (K).
    struct Transiting {
        name: &'static str,
        mass: f64,
        radius: f64,
        semi_major_axis: f64,
        luminosity: f64,
        age: f64,
        fe_h: f64,
        t_eq: f64,
    }

    const HOT_JUPITERS: [Transiting; 2] = [
        Transiting {
            name: "HD 209458 b",
            mass: 0.685,
            radius: 1.359,
            semi_major_axis: 0.047_07,
            luminosity: 1.622,
            age: 3.1,
            fe_h: 0.00,
            t_eq: 1_449.0,
        },
        Transiting {
            name: "HD 189733 b",
            mass: 1.144,
            radius: 1.138,
            semi_major_axis: 0.030_99,
            luminosity: 0.331,
            age: 6.8,
            fe_h: -0.03,
            t_eq: 1_201.0,
        },
    ];

    #[test]
    fn hot_jupiters_are_inflated_to_their_observed_radii() {
        for planet in HOT_JUPITERS {
            let name = planet.name;
            let comp = Composition::from_fe_h(Dex::new(planet.fe_h), HeliumExcess::ZERO);
            let mass = JupiterMasses::new(planet.mass);
            let cooling = giant_cooling(mass, Years::new(planet.age * 1e9), &comp).unwrap();
            let a = planet.semi_major_axis;
            let flux = EarthFluxes::new(planet.luminosity / (a * a));
            let t = black_body(flux);
            assert!((t - planet.t_eq).abs() < 5.0, "{name}: {t} K");
            let giant = radius_giant(EarthMasses::from(mass), &cooling, flux).unwrap();
            let ours = in_jupiters(giant.radius());
            let cooled = in_jupiters(giant.cooling_radius());
            let observed = planet.radius;
            eprintln!("{name}: {ours:.4} R_J against {observed}, cooling alone {cooled:.4}");
            assert!(giant.is_inflated(), "{name}");
            assert!((ours / observed - 1.0).abs() < 0.05, "{name}: {ours} R_J");
            assert!(
                cooled < 0.9 * observed,
                "{name}: {cooled} R_J cooling alone"
            );
            // At equilibrium the interior radiates what it absorbs: its own temperature is
            // ε^¼ T_eq (Thorngren and Fortney 2018, eq. 35).
            let r = Metres::from(giant.radius()).value();
            let area = 4.0 * core::f64::consts::PI * r * r;
            let own = Watts::from(giant.internal_luminosity()).value() / (area * STEFAN_BOLTZMANN);
            let expected = math::powf(giant.heating_efficiency(), 0.25) * black_body(flux);
            assert!((own.sqrt().sqrt() / expected - 1.0).abs() < 1e-12, "{name}");
        }
    }

    #[test]
    fn the_heating_efficiency_is_thorngren_and_fortney_s() {
        // Eq. 34 peaks at 2.37% where log₁₀ F = 0.14 (F in 10⁹ erg s⁻¹ cm⁻²), 1,570 K.
        let peak = EarthFluxes::from(WattsPerSquareMetre::new(math::exp10(0.14) * 1e6));
        assert!((heating_efficiency(peak) - 0.0237).abs() < 1e-15);
        assert!((black_body(peak) - 1_570.0).abs() < 1.0);
        // One width either side, e^(−½) of the peak.
        for dex in [-0.37, 0.37] {
            let s = EarthFluxes::from(WattsPerSquareMetre::new(math::exp10(0.14 + dex) * 1e6));
            let e = heating_efficiency(s) / 0.0237;
            assert!((e - math::exp(-0.5)).abs() < 1e-12);
        }
        // Their abstract: "falling to ∼0.2% at 2,500 K"; and their threshold of inflation, 0.2 ×
        // 10⁹ erg s⁻¹ cm⁻², about 1,000 K (§1), where ε is 0.25%.
        assert!((heating_efficiency(flux_at(2_500.0)) - 0.002_2).abs() < 1e-4);
        assert!((heating_efficiency(flux_at(1_000.0)) - 0.002_5).abs() < 1e-4);
        let threshold = EarthFluxes::from(WattsPerSquareMetre::new(0.2e6));
        assert!((black_body(threshold) - 1_000.0).abs() < 50.0);
        // At the peak the interior's own temperature, ε^¼ T_eq, is 0.39 T_eq.
        assert!((math::powf(HEATING_EFFICIENCY_PEAK, 0.25) - 0.39).abs() < 0.003);
        assert!(heating_efficiency(EarthFluxes::ZERO).abs() < f64::MIN_POSITIVE);
    }

    #[test]
    fn the_equilibrium_radius_reads_thorngren_and_fortney_s_figure() {
        // The table's masses are theirs: 0.299 to 12 Jupiter masses.
        let mass_at = |column: u8| {
            JupiterMasses::new(math::exp10(
                TF_LOG_MASS_FIRST + f64::from(column) * TF_LOG_MASS_STEP,
            ))
        };
        assert!((mass_at(0).value() - 0.299_35).abs() < 1e-5);
        assert!((mass_at(TF_MASSES - 1).value() - 12.0).abs() < 1e-9);
        for (row, &t) in TF_TEMPERATURES.iter().enumerate() {
            for column in [0, 7, 11, 20, 33] {
                let r = thorngren_fortney_radius(mass_at(column), Kelvin::new(t)).value();
                assert!(
                    (r - TF_RADII[row][usize::from(column)]).abs() < 1e-9,
                    "{t} K, {column}"
                );
            }
        }
        // A Jupiter at 1,500 K is 1.30 Jupiter radii (and at 2,000 K 1.60).
        let jupiter = JupiterMasses::new(1.0);
        let r = thorngren_fortney_radius(jupiter, Kelvin::new(1_500.0)).value();
        assert!((r - 1.30).abs() < 0.005, "{r}");
        // It rises with the temperature at every mass, and beyond 2,000 K too, and it is
        // continuous across the figure's nodes.
        for i in 0..=200_u32 {
            let m = JupiterMasses::new(0.3 * math::powf(13.0 / 0.3, f64::from(i) / 200.0));
            let mut last = 0.0;
            for j in 0..=400_u32 {
                let t = 100.0 + 10.0 * f64::from(j);
                let r = thorngren_fortney_radius(m, Kelvin::new(t)).value();
                assert!(r >= last, "{m:?} at {t} K");
                last = r;
            }
            for t in TF_TEMPERATURES {
                let below = thorngren_fortney_radius(m, Kelvin::new(t - 1e-9)).value();
                let above = thorngren_fortney_radius(m, Kelvin::new(t + 1e-9)).value();
                assert!((above - below).abs() < 1e-9);
            }
        }
        // Unfaded, the 500 K line would hold old giants up, most at 13 Jupiter masses and 13.8
        // Gyr; faded, no giant up to 900 K is held above its cooling radius at any age to 13.8
        // Gyr, so a cool giant is plan 13's alone.
        let mut held: f64 = 0.0;
        for i in 0..=200_u32 {
            let m = 0.3 * math::powf(13.0 / 0.3, f64::from(i) / 200.0);
            let old = in_jupiters(EarthRadii::from(interior(m, 13.8).radius()));
            let at_500 = thorngren_fortney_radius(JupiterMasses::new(m), Kelvin::new(500.0));
            held = held.max(at_500.value() / old - 1.0);
            for t in [0.0, 122.0, 500.0, 750.0, 900.0] {
                for gyr in [0.001, 0.1, 1.0, 4.6, 9.0, 13.8] {
                    let g = giant(m, gyr, flux_at(t));
                    assert!(!g.is_inflated(), "{m} MJ at {t} K and {gyr} Gyr");
                    let cap = EarthRadii::from(GIANT_RADIUS_CAP).value();
                    let cooled = g.cooling_radius().value().min(cap);
                    assert_same_bits(g.radius().value(), cooled);
                }
            }
        }
        assert!((held - 0.043).abs() < 0.001, "{held}");
    }

    #[test]
    fn the_radius_is_continuous_in_time_and_flux_and_capped() {
        for m in [0.3, 0.5, 1.0, 3.0, 12.0] {
            for s in [0.01, 100.0, 700.0, 3_000.0, 30_000.0] {
                let flux = EarthFluxes::new(s);
                // Over the clock window of ±1,000 years (plan 01's H), at any age.
                for gyr in [0.001_1, 0.01, 0.1, 1.0, 4.6, 13.8] {
                    let now = giant(m, gyr, flux).radius().value();
                    for dt in [-1e-6, 1e-6] {
                        let then = giant(m, gyr + dt, flux).radius().value();
                        assert!((then / now - 1.0).abs() < 1e-3, "{m} MJ, {s} F⊕, {gyr} Gyr");
                    }
                }
                // Never growing with age.
                let mut last = f64::INFINITY;
                for i in 0..=100_u32 {
                    let gyr = 1e-3 * math::exp10(f64::from(i) * 4.2 / 100.0);
                    let r = giant(m, gyr, flux).radius().value();
                    assert!(r <= last * (1.0 + 1e-12), "{m} MJ, {s} F⊕, {gyr} Gyr");
                    last = r;
                }
            }
            // Never shrinking as the flux grows, and continuous in it; never above the cap.
            let cap = EarthRadii::from(GIANT_RADIUS_CAP).value();
            for gyr in [0.01, 5.0] {
                let mut last = 0.0;
                for i in 0..=2_000_u32 {
                    let s = 1e-2 * math::exp10(f64::from(i) * 7.0 / 2_000.0);
                    let r = giant(m, gyr, EarthFluxes::new(s)).radius().value();
                    assert!(r >= last && r <= cap, "{m} MJ, {s} F⊕, {gyr} Gyr");
                    assert!(i == 0 || r / last - 1.0 < 0.01, "{m} MJ, {s} F⊕: a step");
                    last = r;
                }
            }
        }
        // A light giant at 2,500 K is held at the cap.
        let capped = giant(0.42, 5.0, flux_at(2_500.0));
        assert!((in_jupiters(capped.radius()) - 2.0).abs() < 1e-12);
        assert!(in_jupiters(capped.inflated_radius()) > 2.0);
    }

    #[test]
    fn the_blend_runs_from_the_model_below_to_the_giant() {
        let at = |m_j: f64| giant(m_j, 4.6, EarthFluxes::new(300.0));
        let below = EarthRadii::new(9.0);
        let edge = at(0.3);
        assert!(edge.share().abs() < f64::MIN_POSITIVE);
        assert_same_bits(edge.blended(below).value(), below.value());
        assert!(edge.internal_luminosity().value().abs() < f64::MIN_POSITIVE);
        let jovian = at(0.5);
        assert_same_bits(jovian.share(), 1.0);
        assert_same_bits(jovian.blended(below).value(), jovian.radius().value());
        // Halfway in log mass, halfway in the logarithm of the radius.
        let high = NEPTUNIAN_JOVIAN_TRANSITION.value();
        let low = EarthMasses::from(GIANT_MIN_MASS).value();
        let middle = JupiterMasses::from(EarthMasses::new((low * high).sqrt())).value();
        let half = at(middle);
        assert!((half.share() - 0.5).abs() < 1e-12);
        let expected = (below.value() * half.radius().value()).sqrt();
        assert!((half.blended(below).value() / expected - 1.0).abs() < 1e-12);
        let own = interior(middle, 4.6).luminosity().value();
        assert!(half.internal_luminosity().value() >= 0.5 * own * (1.0 - 1e-12));
    }

    #[test]
    fn giants_outside_the_cooling_fit_are_refused() {
        let jupiter = interior(1.0, 1.0);
        let one = EarthFluxes::new(1.0);
        for m in [95.0, 5_000.0, f64::NAN] {
            let mass = EarthMasses::new(m);
            match radius_giant(mass, &jupiter, one) {
                Err(DeriveGiantError::MassOutsideGiants(got)) => {
                    assert!(got.value().total_cmp(&m).is_eq());
                }
                other => panic!("{m} M⊕: {other:?}"),
            }
        }
        let mass = EarthMasses::from(JupiterMasses::new(1.0));
        for s in [-1.0, f64::INFINITY, f64::NAN] {
            assert!(matches!(
                radius_giant(mass, &jupiter, EarthFluxes::new(s)),
                Err(DeriveGiantError::FluxNotValid(_))
            ));
        }
        let text = DeriveGiantError::MassOutsideGiants(EarthMasses::new(95.0)).to_string();
        assert_eq!(
            text,
            "a mass of 95 earth masses is outside the giant planets' 0.3 to 13 Jupiter masses"
        );
        let text = DeriveGiantError::FluxNotValid(EarthFluxes::new(-1.0)).to_string();
        assert_eq!(text, "a flux of -1 earth fluxes is negative or not finite");
    }

    #[test]
    fn a_giant_is_the_same_twice() {
        let flux = EarthFluxes::new(640.0);
        let (a, b) = (giant(0.8, 2.0, flux), giant(0.8, 2.0, flux));
        assert_eq!(a, b);
        assert_same_bits(a.radius().value(), b.radius().value());
        assert_same_bits(
            a.internal_luminosity().value(),
            b.internal_luminosity().value(),
        );
    }
}
