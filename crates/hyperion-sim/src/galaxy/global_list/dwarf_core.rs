//! A dwarf core's model: its profile, grid, member classes, ages, metallicity and motion (plan 10,
//! P10.T7.a).
//!
//! A core is what is still bound of a recent dwarf progenitor inside the root cube (Design note
//! 11; [`DwarfCoreSpec`]). [`DwarfCoreModel::from_spec`] turns its specification into what plan
//! 09's feature machinery reads to place its members (P10.T7.b): nothing is placed here, and
//! everything but the mean metallicity's offset is a closed form or a fixed quadrature of the
//! specification and the galaxy's in-plane tables.
//!
//! - **The profile** is a Plummer sphere flattened along the grid's z by [`FLATTENING`], shared by
//!   every class (no segregation: q = 1, Design note 9 of plan 09), tapered to zero at the core's
//!   extent as plan 09's profiles are. Its projected half-light radius `R_e`, which is a Plummer
//!   sphere's scale `a`, follows the dwarfs' size–mass relation at the core's present stellar mass
//!   ([`half_light_radius`]); its three-dimensional half-mass radius is `1.305 a`, held to 8,500 ly
//!   ([`HALF_MASS_RADIUS_LIMIT_LY`]).
//! - **The extent** is the core's Jacobi radius `r_J`, where `r_J³ = G m(< r_J) ÷ (4Ω² − κ²)` at
//!   its present galactocentric radius, the denominator floored as plan 02's tidal radius floors
//!   it, and held under the brainstorm's 20,000 ly ([`EXTENT_LIMIT_LY`]). The bound mass is the
//!   greater of the stars' and an isothermal halo's of the core's observed dispersion σ,
//!   `m(< r) = 2 σ² r ÷ G`, which gives `r_J = σ √(2 ÷ (4Ω² − κ²))`. A dwarf is dark-matter
//!   dominated, so below a few 10⁸ M☉ the halo's radius is the larger: the stars' alone would be
//!   2.5 times smaller at 10⁶ M☉, inside the core's own half-light radius.
//! - **The grid** is `NestedGrid::new(w, 16, 8)` by plan 09's
//!   [`grid_width`](MemberClassTable::grid_width) rule from a floor of 16 ly
//!   ([`CORE_GRID_WIDTH_MIN_LOG2`]) in place of 1 ⁄ 64 ly: the least power of two whose reach, 1,024 w,
//!   holds the extent, since a core has no tails. So w is 16 ly up to an extent of 16,384 ly and
//!   32 ly above it.
//! - **The classes**, by plan 09's device ([`MemberClassTable::from_classes`]), with no remnant
//!   lost: each band's living stars, its white dwarfs, and band E's neutron stars and black holes,
//!   per system formed over the core's [`StarFormationHistory`], each age with its own turn-off,
//!   scaled together to the specification's expected systems.
//! - **Ages** follow the core's own [`StarFormationHistory`], set by the progenitor's stellar mass
//!   at infall: an old burst 10–13 Gyr ago and a tail of constant rate from 10 Gyr ago to a
//!   quenching time after the accretion ([`quenching_time`]), which holds more of the stars the
//!   more massive the dwarf.
//! - **Metallicity**: the core's mean is Kirby et al. 2013's eq. 4 at the progenitor's stellar mass
//!   at infall ([`mass_metallicity`]) plus one Gaussian offset of [`MEAN_METALLICITY_SCATTER_DEX`],
//!   on `dwarf.core.metallicity`; its members spread about that mean by
//!   [`metallicity_spread`] (the desktop's ruling on plan 10's re-validation, issue #123, point
//!   4).
//! - **Motion**: the bulk motion is the core's epoch state on its orbit, from which the leapfrog
//!   integrates it (P10.T4); its members add the measured dispersion of [`velocity_dispersion`],
//!   flat in radius ([`DwarfCoreModel::dispersion_at`]).
//!
//! Which mass each figure reads: the size, the extent and the dispersion the core's present
//! stellar mass ([`DwarfCoreSpec::mass`], after the share's scaling), since they describe what is
//! bound now; the ages and metallicities the progenitor's at infall
//! ([`DwarfCoreSpec::accreted_mass`]), since its stars formed before it was stripped.

use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::consts::{G, LIGHT_YEARS_PER_PARSEC, LIGHT_YEARS_PER_YEAR_PER_KM_S};
use crate::galaxy::features::cluster::MEAN_BLACK_HOLE_MASS;
use crate::galaxy::features::interior::counts::{
    ClassCount, NEUTRON_STAR_MASS, WHITE_DWARF_PROGENITOR_MAX, moments, white_dwarf_mass,
};
use crate::galaxy::features::interior::retention::retention_tabulated;
use crate::galaxy::features::interior::{
    ClassKind, Generation, MemberClass, MemberClassTable, Multiplicity, ProfileShape,
};
use crate::galaxy::features::nested::NestedGrid;
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::params::ProgenitorKind;
use crate::galaxy::potential::TIDAL_FLOOR;
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::sse::turn_off_mass;
use crate::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Dex, HeliumExcess, KilometresPerSecond, LightYears, SolarMasses, Years};

use super::DwarfCoreNumber;
use super::spec::DwarfCoreSpec;

mod members;

pub use members::{ExceedDwarfCoreIndexError, resolve_dwarf_core_member};

/// The dwarfs' size–mass relation, `log₁₀(R_e ÷ pc) = a + b log₁₀(M★ ÷ M☉)`, as `(a, b)`.
///
/// A least-squares fit (ours, 2026-10-28) to the exponential scale lengths of Woo, Courteau and
/// Dekel's (2008, MNRAS 390, 1453) Table 1, 30 Local Group dwarfs of 10⁵·⁶–10⁹·² M☉ (M33 left out),
/// each turned into a half-light radius by `R_e = 1.678 R★` (Wolf et al. 2010, MNRAS 406, 1220,
/// appendix B), with an rms of 0.21 dex, which is not drawn. Their own slope is 0.33 ± 0.02 (§4.2).
/// It gives 155, 286, 530 and 980 pc at 10⁵, 10⁶, 10⁷ and 10⁸ M☉; McConnachie (2012, AJ 144, 4,
/// Table 3) fits a flatter 0.15 to semi-major axes. Below 10⁵·⁶ M☉ it is extrapolated: 55 pc at
/// 2 × 10³ M☉, as the ultra-faint dwarfs are.
pub const SIZE_MASS: (f64, f64) = (0.855, 0.267);

/// The longest three-dimensional half-mass radius a core takes, ly (P10.T7.a).
///
/// The relation passes it near 10⁹·¹⁶ M☉; at 10⁹·⁵ its median is about 10,500 ly, so the most
/// massive progenitors would be held, but a core's present mass rarely reaches 10⁸·⁵ M☉.
pub const HALF_MASS_RADIUS_LIMIT_LY: f64 = 8_500.0;

/// A Plummer sphere's three-dimensional half-mass radius over its scale `a`, `(2^(2⁄3) − 1)^(−½)`
/// = 1.3048.
///
/// Its projected half-light radius is `a` itself (Wolf et al. 2010, appendix B).
pub const PLUMMER_HALF_MASS_OVER_SCALE: f64 = 1.304_766_03;

/// A core's axis ratio along its grid's z: 0.67, one less the median ellipticity of the Local
/// Group's dwarfs.
///
/// The median is 0.33 (interquartile 0.20–0.41) over the 48 dwarfs of at least 10⁵ M☉ in
/// McConnachie (2012, AJ 144, 4, Table 3). That is a projected figure taken as the intrinsic one;
/// an oblate body seen from random directions looks rounder than it is, so the cores are rounder
/// than the dwarfs.
pub const FLATTENING: f64 = 0.67;

/// The farthest a core's members reach from its centre, ly: the brainstorm's "up to 20,000 ly"
/// ("Dense features", the feature table).
pub const EXTENT_LIMIT_LY: f64 = 20_000.0;

/// The width a core's nested grid is tried from, as a power of two: 16 ly (P10.T7.a, "that
/// floor"), in place of plan 09's 1 ⁄ 64 ly.
pub const CORE_GRID_WIDTH_MIN_LOG2: i32 = 4;

/// The line-of-sight velocity dispersion of a dwarf, `log₁₀(σ ÷ km/s) = a + b log₁₀(M★ ÷ M☉)`, as
/// `(a, b)`.
///
/// A least-squares fit (ours, 2026-10-28) to McConnachie's (2012, AJ 144, 4) Table 4, 28 Local
/// Group dwarfs of 10⁵·⁴–10⁸·⁷ M☉ at M★ ÷ `L_V` = 1.6, rms 0.14 dex. It gives 5.6, 8.1, 11.7, 16.9
/// and 24.3 km/s at 10⁵–10⁹ M☉ (Fornax, 2 × 10⁷ M☉: 13 against 11.7 measured), extrapolated
/// outside the fitted range. A Plummer sphere of the stars alone falls 1.8 (NGC 205) to 15 (Canes
/// Venatici I) times short.
pub const VELOCITY_DISPERSION: (f64, f64) = (-0.045, 0.159);

/// Kirby et al.'s (2013, ApJ 779, 102) eq. 4, the mean \[Fe/H\] of a dwarf's stars,
/// `⟨[Fe/H]⟩ = a + b log₁₀(M★ ÷ 10⁶ M☉)`, as `(a, b)`: (−1.69 ± 0.04, 0.30 ± 0.02).
pub const MASS_METALLICITY: (f64, f64) = (-1.69, 0.30);

/// The rms of the dwarfs' mean \[Fe/H\] about [`MASS_METALLICITY`], dex: 0.17.
///
/// "The rms about the best-fit line is 0.17" (Kirby et al. 2013, §5.1): a scatter between
/// galaxies, of which about 0.05 dex is measurement error.
pub const MEAN_METALLICITY_SCATTER_DEX: f64 = 0.17;

/// Kirby et al.'s (2011, ApJ 727, 78) eq. 13, the error-corrected spread of \[Fe/H\] within a
/// dwarf spheroidal, `σ = a + b log₁₀(L_V ÷ 10⁵ L☉)`, as `(a, b)`: (0.45 ± 0.03, −0.06 ± 0.02).
///
/// It is fitted to the 15 dwarf spheroidals of their Table 2, 10³·⁶ L☉ (Ursa Major II and Coma
/// Berenices) to 10⁷·³ L☉ (Fornax), 10³·⁸–10⁷·⁵ M☉ at [`MASS_TO_LIGHT_V`]; outside that it is
/// extrapolated.
pub const METALLICITY_SPREAD: (f64, f64) = (0.45, -0.06);

/// The stellar mass per solar luminosity in V of Kirby et al. 2013's stellar masses, 1.6.
///
/// It is the median of Woo, Courteau and Dekel's (2008) Table 2 for dwarf ellipticals.
pub const MASS_TO_LIGHT_V: f64 = 1.6;

/// The least spread of \[Fe/H\] within a core, dex: 0.32.
///
/// [`METALLICITY_SPREAD`] is held where it reaches 0.32, near 10⁷·³⁷ M☉, the most luminous dwarf
/// spheroidals of Kirby et al. 2011's sample (Fornax, 10⁷·³ L☉). Above it the hold is an
/// extrapolation.
pub const METALLICITY_SPREAD_FLOOR_DEX: f64 = 0.32;

/// The old burst's ages, years: 10–13 Gyr ago (P10.T7.a).
///
/// Its younger end is the z ≈ 2 at which Weisz et al. (2014, ApJ 789, 147) count a dwarf's early
/// star formation.
pub const OLD_BURST_AGES_YR: (f64, f64) = (10e9, 13e9);

/// The old burst's share of a dwarf's stars, `clamp(a + b (log₁₀(M★ ÷ M☉) − 5), lo, hi)`, as
/// `(a, b, lo, hi)`.
///
/// Weisz et al. (2014, ApJ 789, 147, §4.2) find that dwarfs below 10⁵ M☉ formed "≳80%" of their
/// stars before z ≈ 2 and those above 10⁷ M☉ "only ~30%"; linear in log M★ between (ours).
pub const OLD_SHARE: (f64, f64, f64, f64) = (0.8, -0.25, 0.3, 0.8);

/// The least initial mass of a black hole's progenitor in the class's range: plan 09's clusters'
/// (`interior::counts`), which the class only labels.
const BLACK_HOLE_PROGENITOR_MIN_MSUN: f64 = 20.0;

/// The time a dwarf of stellar mass `mass` at infall goes on forming stars after its accretion,
/// `τ_q`: 1 Gyr at and below 10⁷ M☉ (ours: a reading of "< 2 Gyr" and "more rapid at lower
/// `M_star`", below), 2 Gyr at 10⁸ M☉ and 9.5 Gyr at 10⁹ M☉ and above, linear in log₁₀ M★
/// between ([`QUENCHING_TIME_YR`]).
///
/// Wetzel, Tollerud and Weisz 2015 (ApJL 808, L27, abstract) find quenching after infall "< 2 Gyr"
/// below 10⁸ M☉ and "more rapid at lower `M_star`", the timescale then increasing "rapidly …
/// peaking at ~9.5 Gyr for `M_star` ~ 10⁹ `M_sun`"; Fillingham et al. 2015 (MNRAS 454, 2039,
/// abstract) find about 2 Gyr at 10⁶–10⁸ M☉. Their "< 5 Gyr" above 5 × 10⁹ M☉ lies beyond the
/// recent progenitors' 10⁹·⁵ M☉. Main's ruling of 2026-10-30, after the research lane
/// `research-p10` (its advice §4): the points are read from the abstracts, not from Wetzel et
/// al.'s Fig. 3, a parameter of the generator version.
#[must_use]
pub fn quenching_time(mass: SolarMasses) -> Years {
    let x = math::log10(mass.value());
    let [(x0, t0), (x1, t1), (x2, t2)] = QUENCHING_TIME_YR;
    Years::new(if x <= x0 {
        t0
    } else if x <= x1 {
        t0 + (x - x0) / (x1 - x0) * (t1 - t0)
    } else if x <= x2 {
        t1 + (x - x1) / (x2 - x1) * (t2 - t1)
    } else {
        t2
    })
}

/// [`quenching_time`]'s points, `(log₁₀(M★ ÷ M☉), τ_q ÷ yr)`: 1 Gyr at 10⁷, 2 Gyr at 10⁸ and
/// 9.5 Gyr at 10⁹, held beyond the ends.
pub const QUENCHING_TIME_YR: [(f64, f64); 3] = [(7.0, 1e9), (8.0, 2e9), (9.0, 9.5e9)];

/// A dwarf core's star formation history (module documentation).
///
/// It is an old burst of constant rate over [`OLD_BURST_AGES_YR`] holding
/// [`old_share`](Self::old_share) of the stars, and a tail of constant rate from the burst's
/// younger end to [`quenching_time`] after the accretion, or to the epoch where that has not come,
/// holding the rest. So both the tail's share of the stars and its length grow with the dwarf's
/// mass (plan 10, P10.T7.a: "a tail that lasts longer the more massive the dwarf"): a dwarf of
/// 10⁹ M☉ accreted within 6 Gyr still forms stars at the epoch, as the Magellanic Clouds do.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::global_list::dwarf_core::StarFormationHistory;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // A Sagittarius-sized dwarf accreted 5 Gyr ago forms most of its stars in the tail, which
/// // goes on 5.6 Gyr after its accretion, to the epoch.
/// let history = StarFormationHistory::of(SolarMasses::new(3e8), Years::new(5e9));
/// assert!(history.old_share() < 0.5);
/// let (youngest, oldest) = (history.age_quantile(0.0), history.age_quantile(1.0));
/// assert!(youngest.value() < 1.0 && (oldest.value() - 13e9).abs() < 1.0);
/// // One of 10⁸ M☉ accreted as long ago stopped 2 Gyr after it.
/// let small = StarFormationHistory::of(SolarMasses::new(1e8), Years::new(5e9));
/// assert!((small.age_quantile(0.0).value() - 3e9).abs() < 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarFormationHistory {
    old_share: f64,
    /// The tail's ages, years: its end's, [`quenching_time`] after the accretion, to the burst's
    /// younger end.
    tail: (f64, f64),
}

impl StarFormationHistory {
    /// The history of a dwarf of stellar mass `mass` at infall, accreted `accreted` ago (0–6 Gyr;
    /// an end beyond the burst's younger end leaves no tail).
    #[must_use]
    pub fn of(mass: SolarMasses, accreted: Years) -> Self {
        let (a, b, lo, hi) = OLD_SHARE;
        let old_share = (a + b * (math::log10(mass.value()) - 5.0)).clamp(lo, hi);
        let end = accreted.value() - quenching_time(mass).value();
        let young = end.clamp(0.0, OLD_BURST_AGES_YR.0);
        let tail = (young, OLD_BURST_AGES_YR.0);
        let old_share = if tail.1 > tail.0 { old_share } else { 1.0 };
        Self { old_share, tail }
    }

    /// The share of the stars in the old burst.
    #[must_use]
    pub const fn old_share(&self) -> f64 {
        self.old_share
    }

    /// The history of the stars formed at least `age` ago: the tail cut there, its share of the
    /// stars cut with it, for a dwarf's tube, which holds only stars formed before its last
    /// stripping (plan 10, P10.T8.a; research-p10's advice §4).
    #[must_use]
    pub(crate) fn no_younger_than(self, age: Years) -> Self {
        let (young, old) = self.tail;
        if age.value() <= young {
            return self;
        }
        let cut = age.value().min(old);
        // The tail's rate is constant, so the share left is the length left.
        let tail = (1.0 - self.old_share) * (old - cut) / (old - young);
        let old_share = if old > cut {
            self.old_share / (self.old_share + tail)
        } else {
            1.0
        };
        Self {
            old_share,
            tail: (cut, old),
        }
    }

    /// The tail's youngest and oldest ages.
    #[must_use]
    pub fn tail(&self) -> (Years, Years) {
        (Years::new(self.tail.0), Years::new(self.tail.1))
    }

    /// The age below which a share `u` of the stars formed, `u` in [0, 1]: the inverse transform
    /// of the history's distribution in age, through the tail first.
    #[must_use]
    pub fn age_quantile(&self, u: f64) -> Years {
        let u = u.clamp(0.0, 1.0);
        let tail_share = 1.0 - self.old_share;
        let (young, old) = self.tail;
        let (burst_young, burst_old) = OLD_BURST_AGES_YR;
        Years::new(if u < tail_share {
            young + u / tail_share * (old - young)
        } else {
            burst_young + (u - tail_share) / self.old_share * (burst_old - burst_young)
        })
    }

    /// The history's quadrature nodes, `(age, weight)`, the weights summing to 1: the 32-point
    /// Gauss–Legendre rule on the burst, then on the tail (none if it is empty), each weighted by
    /// its share.
    #[must_use]
    fn nodes(&self) -> Vec<(f64, f64)> {
        let mut nodes = Vec::with_capacity(2 * GL32_NODES.len());
        let mut piece = |(young, old): (f64, f64), share: f64| {
            let half = 0.5 * (old - young);
            let mid = young + half;
            for (&x, &w) in GL32_NODES.iter().zip(&GL32_WEIGHTS) {
                nodes.push((mid + half * x, 0.5 * w * share));
            }
        };
        piece(OLD_BURST_AGES_YR, self.old_share);
        if self.tail.1 > self.tail.0 {
            piece(self.tail, 1.0 - self.old_share);
        }
        nodes
    }
}

/// Kirby et al. 2013's eq. 4 at stellar mass `mass`: the mean \[Fe/H\] of a dwarf's stars
/// ([`MASS_METALLICITY`]). Fitted over the dwarfs of 10³·⁷–10⁸·⁰ M☉ of their Table 4 (Coma
/// Berenices to IC 1613), it is "unbroken … up to M★ = 5 × 10⁸ M☉ (NGC 205)", which the fit does
/// not include; above that it is extrapolated, −0.64 at 10⁹·⁵ M☉.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::global_list::dwarf_core::mass_metallicity;
/// use hyperion_sim::units::SolarMasses;
///
/// // Fornax, 2 × 10⁷ M☉: about −1.3.
/// let fe_h = mass_metallicity(SolarMasses::new(2e7)).value();
/// assert!((fe_h + 1.30).abs() < 0.01, "{fe_h}");
/// ```
#[must_use]
pub fn mass_metallicity(mass: SolarMasses) -> Dex {
    let (a, b) = MASS_METALLICITY;
    Dex::new(a + b * math::log10(mass.value() / 1e6))
}

/// The spread of \[Fe/H\] among a core's members about its mean for a dwarf of stellar mass `mass`:
/// Kirby et al. 2011's eq. 13 at `L_V = M★ ÷ 1.6` ([`MASS_TO_LIGHT_V`]), held at
/// [`METALLICITY_SPREAD_FLOOR_DEX`]: 0.46 dex at 10⁵ M☉, 0.40 at 10⁶ and 0.34 at 10⁷.
#[must_use]
pub fn metallicity_spread(mass: SolarMasses) -> Dex {
    let (a, b) = METALLICITY_SPREAD;
    let luminosity = mass.value() / MASS_TO_LIGHT_V;
    Dex::new((a + b * math::log10(luminosity / 1e5)).max(METALLICITY_SPREAD_FLOOR_DEX))
}

/// The projected half-light radius of a dwarf of stellar mass `mass` by [`SIZE_MASS`]: a Plummer
/// sphere's scale.
#[must_use]
pub fn half_light_radius(mass: SolarMasses) -> LightYears {
    let (a, b) = SIZE_MASS;
    LightYears::new(math::exp10(a + b * math::log10(mass.value())) * LIGHT_YEARS_PER_PARSEC)
}

/// The line-of-sight velocity dispersion of a dwarf of stellar mass `mass` by
/// [`VELOCITY_DISPERSION`].
#[must_use]
pub fn velocity_dispersion(mass: SolarMasses) -> KilometresPerSecond {
    let (a, b) = VELOCITY_DISPERSION;
    KilometresPerSecond::new(math::exp10(a + b * math::log10(mass.value())))
}

/// A dwarf core's model (module documentation): pure functions of its [`DwarfCoreSpec`] and the
/// galaxy.
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::Debris;
/// use hyperion_sim::galaxy::global_list::dwarf_core::DwarfCoreModel;
/// use hyperion_sim::galaxy::imf::MassBand;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// for spec in Debris::generate(&galaxy).cores() {
///     let core = DwarfCoreModel::from_spec(&galaxy, spec);
///     // Its members are not coeval, and its grid holds them all.
///     let (youngest, oldest) = core.history().tail();
///     assert!(oldest.value() > youngest.value());
///     assert!(core.grid().reach().value() >= core.extent().value());
///     let members: f64 = MassBand::ALL.iter().map(|&b| core.table().expected(b)).sum();
///     assert!((members / spec.systems() - 1.0).abs() < 1e-9);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DwarfCoreModel {
    number: DwarfCoreNumber,
    systems: f64,
    position: GalacticDisplacement,
    velocity: GalacticVelocity,
    scale: f64,
    extent: f64,
    dispersion: f64,
    history: StarFormationHistory,
    fe_h: Dex,
    fe_h_spread: Dex,
    table: MemberClassTable,
    grid: NestedGrid,
}

impl DwarfCoreModel {
    /// The model of the core `spec` of `galaxy` (module documentation).
    ///
    /// It draws the mean metallicity's offset, two words of `dwarf.core.metallicity` keyed by the
    /// progenitor's number as its other draws are, and reads the galaxy's in-plane tables and mass
    /// function.
    ///
    /// # Panics
    ///
    /// If the specification's present stellar mass is not positive and finite, which no
    /// [`Debris`](super::Debris) core's is: it drops a core the share scales under 10³ systems.
    #[must_use]
    pub fn from_spec(galaxy: &Galaxy, spec: &DwarfCoreSpec) -> Self {
        let mass = spec.mass();
        assert!(
            mass.value() > 0.0 && mass.value().is_finite(),
            "a dwarf core of {} M☉",
            mass.value()
        );
        let infall = spec.accreted_mass();
        let scale = half_light_radius(mass)
            .value()
            .min(HALF_MASS_RADIUS_LIMIT_LY / PLUMMER_HALF_MASS_OVER_SCALE);
        let dispersion = velocity_dispersion(mass).value();
        let extent = extent_of(galaxy, spec).value();
        let history = StarFormationHistory::of(infall, spec.accreted());
        let fe_h = dwarf_mean_fe_h(galaxy, spec.progenitor(), infall);
        let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
        let shape = ProfileShape::Plummer { scale };
        let classes = classes(
            galaxy.mass_function(),
            &composition,
            &history,
            shape,
            spec.systems(),
        );
        let table = MemberClassTable::from_classes(&classes, LightYears::new(extent), FLATTENING);
        let grid = table.grid_from(CORE_GRID_WIDTH_MIN_LOG2);
        Self {
            number: spec.number(),
            systems: spec.systems(),
            position: spec.position(),
            velocity: spec.velocity(),
            scale,
            extent,
            dispersion,
            history,
            fe_h,
            fe_h_spread: metallicity_spread(infall),
            table,
            grid,
        }
    }

    /// The core's number on the global list.
    #[must_use]
    pub const fn number(&self) -> DwarfCoreNumber {
        self.number
    }

    /// The expected systems it holds, the specification's: what its classes sum to.
    #[must_use]
    pub const fn systems(&self) -> f64 {
        self.systems
    }

    /// Its centre about the galactic centre at the epoch, m.
    #[must_use]
    pub const fn position(&self) -> GalacticDisplacement {
        self.position
    }

    /// Its bulk velocity in the galactic frame at the epoch: its orbit's epoch state, from which
    /// the leapfrog integrates it.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The Plummer scale `a`, which is its projected half-light radius.
    #[must_use]
    pub fn plummer_scale(&self) -> LightYears {
        LightYears::new(self.scale)
    }

    /// Its three-dimensional half-mass radius before the taper, `1.305 a`, at most 8,500 ly.
    #[must_use]
    pub fn half_mass_radius(&self) -> LightYears {
        LightYears::new(self.scale * PLUMMER_HALF_MASS_OVER_SCALE)
    }

    /// How far its members reach from its centre (an ellipsoidal radius along the grid's z):
    /// its Jacobi radius, at most 20,000 ly.
    #[must_use]
    pub fn extent(&self) -> LightYears {
        LightYears::new(self.extent)
    }

    /// Its line-of-sight velocity dispersion, the whole core's ([`velocity_dispersion`]).
    #[must_use]
    pub fn dispersion(&self) -> KilometresPerSecond {
        KilometresPerSecond::new(self.dispersion)
    }

    /// The one-dimensional velocity dispersion of its members at ellipsoidal radius `r`: the
    /// core's [`dispersion`](Self::dispersion) at every radius.
    ///
    /// Walker et al. 2007 (ApJ 667, L53, abstract) find in each of seven dwarf spheroidals a
    /// dispersion "of order 10 km/s that remains approximately constant with distance from the
    /// dSph center, out to and in some cases beyond the radius at which the mean surface
    /// brightness falls to the background level", where a Plummer sphere's shape falls to
    /// 0.56 σ₀ at three scales (main's ruling of 2026-10-30, after the research lane
    /// `research-p10`, its advice §2). Their dwarf spheroidals are of about 10⁵·⁵–10⁷·³ M☉: above
    /// about 2 × 10⁷ M☉, where the Magellanic Clouds and Sagittarius rotate, the flat profile is an
    /// extrapolation.
    #[must_use]
    pub fn dispersion_at(&self, _r: LightYears) -> KilometresPerSecond {
        KilometresPerSecond::new(self.dispersion)
    }

    /// Its star formation history, which its members' ages follow.
    #[must_use]
    pub const fn history(&self) -> &StarFormationHistory {
        &self.history
    }

    /// Its members' mean \[Fe/H\].
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// The spread of its members' \[Fe/H\] about the mean ([`metallicity_spread`]).
    #[must_use]
    pub const fn fe_h_spread(&self) -> Dex {
        self.fe_h_spread
    }

    /// Its member classes with their counts and profiles.
    #[must_use]
    pub const fn table(&self) -> &MemberClassTable {
        &self.table
    }

    /// Its nested grid: 16 cells and eight levels at a width of 16 or 32 ly.
    #[must_use]
    pub const fn grid(&self) -> &NestedGrid {
        &self.grid
    }
}

/// The youngest age, years, a member's turn-off is read at: 10⁴, past the zero of a history still
/// forming stars, as plan 09's centre reads it.
pub(crate) const YOUNGEST_TURN_OFF_AGE_YR: f64 = 1e4;

/// The mean \[Fe/H\] of recent progenitor `progenitor`'s stars, of stellar mass `infall` at
/// infall: Kirby et al. 2013's eq. 4 plus its offset, two words of `dwarf.core.metallicity`
/// keyed by the progenitor's number, which its core and its tube share (P10.T7.a, P10.T8.a).
#[must_use]
pub(crate) fn dwarf_mean_fe_h(galaxy: &Galaxy, progenitor: u32, infall: SolarMasses) -> Dex {
    let key = ObjectKey::galaxy_item(ProgenitorKind::Recent(progenitor).number());
    let offset = Stream::open(galaxy.seed(), tags::DWARF_CORE_METALLICITY, key)
        .normal(0.0, MEAN_METALLICITY_SCATTER_DEX);
    Dex::new(mass_metallicity(infall).value() + offset)
}

/// How far the members of `spec`'s core reach from its centre, what
/// [`DwarfCoreModel::extent`] gives, without building the model: its Jacobi radius at its present
/// mass and dispersion, held to [`EXTENT_LIMIT_LY`]. The global list widens the core's bounding
/// shell by it (P10.T4).
#[must_use]
pub(crate) fn extent_of(galaxy: &Galaxy, spec: &DwarfCoreSpec) -> LightYears {
    let mass = spec.mass();
    let dispersion = velocity_dispersion(mass).value();
    LightYears::new(
        jacobi_radius(galaxy, &spec.position(), mass.value(), dispersion).min(EXTENT_LIMIT_LY),
    )
}

/// The Jacobi radius, ly, of a core of `stars` M☉ at `position` (m, about the galactic centre)
/// with dispersion `sigma` (km/s) (module documentation).
///
/// With `D = 4Ω² − κ²`, floored at plan 02's 0.05 Ω² as its tidal radius is, the bound mass
/// `max(M★, 2 σ² r ÷ G)` gives the greater of the stars' `(G M★ ÷ D)^⅓` and the isothermal halo's
/// `σ √(2 ÷ D)`.
#[must_use]
fn jacobi_radius(galaxy: &Galaxy, position: &GalacticDisplacement, stars: f64, sigma: f64) -> f64 {
    let m = position.metres();
    let r = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt() / METRES_PER_LIGHT_YEAR;
    let tables = galaxy.potential();
    let omega = tables.omega(LightYears::new(r)).value();
    let kappa = tables.kappa(LightYears::new(r)).value();
    // (rad/yr)² to (km/s ÷ ly)², the working units of `G`.
    let per_year = LIGHT_YEARS_PER_YEAR_PER_KM_S * LIGHT_YEARS_PER_YEAR_PER_KM_S;
    let denominator =
        (4.0 * omega * omega - kappa * kappa).max(TIDAL_FLOOR * omega * omega) / per_year;
    let halo = sigma * (2.0 / denominator).sqrt();
    let own = math::cbrt(G * stars / denominator);
    halo.max(own)
}

/// The core's member classes (module documentation), scaled to `systems` in all: in each band its
/// living stars and white dwarfs, then band E's neutron stars and black holes, every one single,
/// first population and of profile `shape`, per system formed over `history` on the mass function
/// `f` with the turn-off of `composition` at each age.
///
/// A star dies at the end of its main sequence, and every remnant stays. The neutron stars and
/// black holes are plan 08's per primary formed (through plan 09's retention table, which reads
/// them whatever the escape speed), counted from the ages at which an 8 M☉ star has died: a core
/// still forming stars leaves out the remnants of its last 40 Myr.
#[must_use]
pub(crate) fn classes(
    f: &dyn MassFunction,
    composition: &Composition,
    history: &StarFormationHistory,
    shape: ProfileShape,
    systems: f64,
) -> Vec<ClassCount> {
    // The turn-off at each of the history's quadrature nodes, computed once.
    let nodes: Vec<(f64, f64, f64)> = history
        .nodes()
        .into_iter()
        .map(|(age, weight)| {
            (
                age,
                weight,
                turn_off_mass(Years::new(age), composition).value(),
            )
        })
        .collect();
    let youngest = turn_off_mass(Years::new(history.tail.0), composition).value();
    let oldest = turn_off_mass(Years::new(OLD_BURST_AGES_YR.1), composition).value();
    let remnants = retention_tabulated(f, composition, KilometresPerSecond::new(100.0));
    let dead_massive: f64 = nodes
        .iter()
        .filter(|&&(_, _, m_to)| m_to <= WHITE_DWARF_PROGENITOR_MAX)
        .fold(0.0, |s, &(_, w, _)| s + w);
    let class = |kind, band, range: (f64, f64), mean_mass: f64| MemberClass {
        kind,
        band,
        generation: Generation::First,
        multiplicity: Multiplicity::Single,
        initial_mass_range: (SolarMasses::new(range.0), SolarMasses::new(range.1)),
        mean_mass: SolarMasses::new(mean_mass),
    };
    let mut out = Vec::new();
    for band in MassBand::ALL {
        let (lo, hi) = (
            MASS_BAND_EDGES[band.index()],
            MASS_BAND_EDGES[band.index() + 1],
        );
        // Per system formed, over the history: the count and mass of the living, and of the white
        // dwarfs' progenitors.
        let (mut live, mut live_mass, mut wd, mut wd_initial) = (0.0, 0.0, 0.0, 0.0);
        for &(_, w, m_to) in &nodes {
            let (n, m) = moments(f, lo, hi.min(m_to), |_| 1.0);
            let (n_wd, m_wd) =
                moments(f, lo.max(m_to), hi.min(WHITE_DWARF_PROGENITOR_MAX), |_| 1.0);
            live += w * n;
            live_mass += w * m;
            wd += w * n_wd;
            wd_initial += w * m_wd;
        }
        if live > 0.0 {
            out.push(ClassCount {
                class: class(
                    ClassKind::Living,
                    band,
                    (lo, hi.min(youngest)),
                    live_mass / live,
                ),
                expected: live,
                shape,
            });
        }
        if wd > 0.0 {
            out.push(ClassCount {
                class: class(
                    ClassKind::WhiteDwarf,
                    band,
                    (lo.max(oldest), hi.min(WHITE_DWARF_PROGENITOR_MAX)),
                    white_dwarf_mass(wd_initial / wd),
                ),
                expected: wd,
                shape,
            });
        }
        if band == MassBand::E {
            for (kind, per_primary, range, mass) in [
                (
                    ClassKind::NeutronStar,
                    remnants.neutron_stars_per_primary,
                    (WHITE_DWARF_PROGENITOR_MAX, hi),
                    NEUTRON_STAR_MASS,
                ),
                (
                    ClassKind::BlackHole,
                    remnants.black_holes_per_primary,
                    (BLACK_HOLE_PROGENITOR_MIN_MSUN, hi),
                    MEAN_BLACK_HOLE_MASS,
                ),
            ] {
                let expected = per_primary * dead_massive;
                if expected > 0.0 {
                    out.push(ClassCount {
                        class: class(kind, band, range, mass),
                        expected,
                        shape,
                    });
                }
            }
        }
    }
    let total = out.iter().fold(0.0, |s, c| s + c.expected);
    let k = if total > 0.0 { systems / total } else { 0.0 };
    for c in &mut out {
        c.expected *= k;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::spec::SphericalOrbit;
    use super::*;
    use crate::galaxy::PointLy;
    use crate::galaxy::Population;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::quad::gl16;
    use crate::rng::Seed;

    fn milky_way() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0a10_7a00), GalaxyParams::milky_way_like()).unwrap()
    }

    /// A core of `mass` M☉ still bound, `infall` at accretion `accreted` years ago, at `at` ly
    /// about the centre, on an orbit of 30,000–120,000 ly: the specification's own figures, as
    /// `dwarfs` would give them.
    fn spec(galaxy: &Galaxy, mass: f64, infall: f64, accreted: f64, at: [f64; 3]) -> DwarfCoreSpec {
        let orbit = SphericalOrbit::between(
            galaxy.potential(),
            LightYears::new(30_000.0),
            LightYears::new(120_000.0),
        )
        .unwrap();
        DwarfCoreSpec {
            number: DwarfCoreNumber::new(1).unwrap(),
            progenitor: 2,
            mass: SolarMasses::new(mass),
            systems: mass / galaxy.mean_system_mass(Population::Halo).value(),
            accreted: Years::new(accreted),
            accreted_mass: SolarMasses::new(infall),
            position: GalacticDisplacement::new(at.map(|x| x * METRES_PER_LIGHT_YEAR)),
            velocity: GalacticVelocity::new([1.2e5, -8e4, 3e4]),
            orbit,
            stripping_fraction: 0.3,
            pericentres: 4,
        }
    }

    #[test]
    fn the_metallicity_laws_are_kirby_s() {
        let fe_h = |m: f64| mass_metallicity(SolarMasses::new(m)).value();
        assert!((fe_h(1e6) + 1.69).abs() < 1e-12);
        assert!((fe_h(math::exp10(9.5)) + 0.64).abs() < 1e-12);
        let spread = |m: f64| metallicity_spread(SolarMasses::new(m)).value();
        for (m, want) in [
            (1e5, 0.462),
            (1e6, 0.402),
            (1e7, 0.342),
            (1e8, 0.32),
            (1e9, 0.32),
        ] {
            assert!((spread(m) - want).abs() < 5e-4, "{m}: {}", spread(m));
        }
        // Held from 10^7.37 M☉.
        assert!(
            spread(math::exp10(7.36)) > 0.32 && (spread(math::exp10(7.38)) - 0.32).abs() < 1e-15
        );
    }

    #[test]
    fn the_size_and_dispersion_laws_give_the_fits_figures() {
        for (m, pc, km_s) in [
            (1e5, 155.0, 5.6),
            (1e6, 286.0, 8.1),
            (1e7, 530.0, 11.7),
            (1e8, 980.0, 16.9),
            (1e9, 1_810.0, 24.3),
        ] {
            let r = half_light_radius(SolarMasses::new(m)).value() / LIGHT_YEARS_PER_PARSEC;
            assert!((r / pc - 1.0).abs() < 0.01, "{m}: {r} pc");
            let s = velocity_dispersion(SolarMasses::new(m)).value();
            assert!((s / km_s - 1.0).abs() < 0.01, "{m}: {s} km/s");
        }
        // A Plummer sphere's half-mass radius: (2^(2/3) − 1)^(−1/2).
        let exact = 1.0 / (math::cbrt(4.0) - 1.0).sqrt();
        assert!((PLUMMER_HALF_MASS_OVER_SCALE - exact).abs() < 1e-7);
    }

    /// P10.T7.a: the age distribution of a 10⁸ M☉ core spreads over more than 2 Gyr, however
    /// recently it was accreted; the history's nodes integrate its shares; its tail ends 2 Gyr
    /// after the accretion, or at the epoch.
    #[test]
    fn a_10_8_core_s_ages_spread_over_more_than_two_gyr() {
        for accreted in [0.0, 1e9, 3e9, 6e9] {
            let history = StarFormationHistory::of(SolarMasses::new(1e8), Years::new(accreted));
            let end = (accreted - 2e9).max(0.0);
            assert!((history.old_share() - 0.3).abs() < 1e-12);
            let spread = history.age_quantile(0.95).value() - history.age_quantile(0.05).value();
            assert!(spread > 2e9, "accreted {accreted}: {spread}");
            let nodes = history.nodes();
            let total = nodes.iter().fold(0.0, |s, &(_, w)| s + w);
            assert!((total - 1.0).abs() < 1e-13);
            // The mean age by the nodes against the pieces' midpoints.
            let mean = nodes.iter().fold(0.0, |s, &(a, w)| s + a * w);
            let want = 0.3 * 11.5e9 + 0.7 * f64::midpoint(end, 10e9);
            assert!((mean / want - 1.0).abs() < 1e-12, "{mean} {want}");
            assert!((history.age_quantile(0.0).value() - end).abs() < 1e-3);
            assert!((history.age_quantile(1.0).value() / 13e9 - 1.0).abs() < 1e-12);
        }
        // A small dwarf formed most of its stars in the burst, a large one in the tail.
        let small = StarFormationHistory::of(SolarMasses::new(3e4), Years::new(2e9));
        let large = StarFormationHistory::of(SolarMasses::new(3e9), Years::new(2e9));
        assert!((small.old_share() - 0.8).abs() < 1e-12 && (large.old_share() - 0.3).abs() < 1e-12);
    }

    /// A dwarf's tube's history (P10.T8.a): cut at an age, the tail's share falls with its length
    /// and the shares still sum to 1; cut past the tail, only the burst is left.
    #[test]
    fn a_history_cut_at_an_age_keeps_the_older_stars_only() {
        let full = StarFormationHistory::of(SolarMasses::new(1e8), Years::new(5e9));
        assert!((full.tail().0.value() - 3e9).abs() < 1e-3);
        let cut = full.no_younger_than(Years::new(6.5e9));
        assert!((cut.tail().0.value() - 6.5e9).abs() < 1e-3);
        // The tail held 0.7 over 7 Gyr; 3.5 Gyr of it is left, 0.35 against the burst's 0.3.
        assert!(
            (cut.old_share() - 0.3 / 0.65).abs() < 1e-12,
            "{}",
            cut.old_share()
        );
        assert!(cut.age_quantile(0.0).value() >= 6.5e9 - 1e-3);
        assert_eq!(full.no_younger_than(Years::new(1e9)), full);
        let burst = full.no_younger_than(Years::new(11e9));
        assert!((burst.old_share() - 1.0).abs() < 1e-15);
        let nodes = burst.nodes();
        assert_eq!(nodes.len(), GL32_NODES.len());
    }

    /// The tail ends `τ_q(M★)` after the accretion (main's ruling of 2026-10-30): 1 Gyr up to
    /// 10⁷ M☉, 2 Gyr at 10⁸ and 9.5 Gyr from 10⁹, linear in log₁₀ M★ between, so that a dwarf of
    /// 10⁹ M☉ accreted within 6 Gyr still forms stars at the epoch.
    #[test]
    fn the_tail_ends_a_mass_dependent_quenching_time_after_the_accretion() {
        let tau = |m: f64| quenching_time(SolarMasses::new(m)).value();
        for (m, want) in [
            (1e5, 1e9),
            (1e7, 1e9),
            (math::exp10(7.5), 1.5e9),
            (1e8, 2e9),
            (math::exp10(8.5), 5.75e9),
            (1e9, 9.5e9),
            (math::exp10(9.5), 9.5e9),
        ] {
            assert!((tau(m) / want - 1.0).abs() < 1e-12, "{m:e}: {}", tau(m));
        }
        let history = StarFormationHistory::of(SolarMasses::new(1e9), Years::new(6e9));
        assert!(history.tail().0.value().abs() < 1e-3);
        let early = StarFormationHistory::of(SolarMasses::new(1e6), Years::new(6e9));
        assert!((early.tail().0.value() - 5e9).abs() < 1e-3);
    }

    /// P10.T7.a: the profile integrates to the specification's system count, the extent stays
    /// under the brainstorm's 20,000 ly, and the grid is plan 09's rule from 16 ly.
    #[test]
    fn the_profile_integrates_to_the_spec_s_systems_inside_its_extent() {
        let galaxy = milky_way();
        for (mass, at) in [
            (2e3, [30_000.0, 20_000.0, 5_000.0]),
            (1e6, [-40_000.0, 10_000.0, 20_000.0]),
            (1e8, [55_000.0, -30_000.0, -10_000.0]),
            (3e9, [20_000.0, 0.0, 15_000.0]),
        ] {
            let spec = spec(&galaxy, mass, 3.0 * mass, 3e9, at);
            let core = DwarfCoreModel::from_spec(&galaxy, &spec);
            let label = format!("{mass:e} M☉");
            let extent = core.extent().value();
            eprintln!(
                "{label}: a {:.0} ly, r_h {:.0} ly, extent {extent:.0} ly, grid {} ly, σ {:.1} \
                 km/s, [Fe/H] {:.2} ± {:.2}, old share {:.2}, {:.3e} systems",
                core.plummer_scale().value(),
                core.half_mass_radius().value(),
                core.grid().width().value(),
                core.dispersion().value(),
                core.fe_h().value(),
                core.fe_h_spread().value(),
                core.history().old_share(),
                core.systems()
            );
            assert!(
                extent > 0.0 && extent <= EXTENT_LIMIT_LY,
                "{label}: {extent}"
            );
            assert!(core.half_mass_radius().value() <= HALF_MASS_RADIUS_LIMIT_LY * (1.0 + 1e-12));
            let w = core.grid().width().value();
            assert!(
                [16.0, 32.0].iter().any(|x| (x - w).abs() < 1e-12),
                "{label}: {w}"
            );
            assert!(core.grid().reach().value() >= extent);
            assert!(
                w <= 16.0 || 1_024.0 * 0.5 * w < extent,
                "{label}: {w} for {extent}"
            );
            // The classes sum to the systems.
            let classes: f64 = MassBand::ALL
                .iter()
                .map(|&b| core.table().expected(b))
                .sum();
            assert!((classes / spec.systems() - 1.0).abs() < 1e-12, "{label}");
            // And so does the density over space, by quadrature in spherical coordinates (not
            // the profile's own ellipsoidal ones): 64 panels in ln(1 + r ÷ a) out to the extent,
            // 32 in the polar angle, 16 points each.
            let a = core.plummer_scale().value();
            let t_max = math::ln_1p(extent / a);
            let integral: f64 = (0..64)
                .map(|i| {
                    let (t0, t1) = (t_max * f64::from(i) / 64.0, t_max * f64::from(i + 1) / 64.0);
                    gl16(
                        |t| {
                            let r = a * math::exp_m1(t);
                            let shell: f64 = (0..32)
                                .map(|j| {
                                    let (c0, c1) = (
                                        core::f64::consts::PI * f64::from(j) / 32.0,
                                        core::f64::consts::PI * f64::from(j + 1) / 32.0,
                                    );
                                    gl16(
                                        |theta| {
                                            let (sin, cos) = math::sin_cos(theta);
                                            let p = PointLy::new(r * sin, 0.0, r * cos);
                                            core.table().density(&p) * sin
                                        },
                                        c0,
                                        c1,
                                    )
                                })
                                .sum();
                            2.0 * core::f64::consts::PI * r * r * shell * (r + a)
                        },
                        t0,
                        t1,
                    )
                })
                .sum();
            assert!(
                (integral / spec.systems() - 1.0).abs() < 1e-3,
                "{label}: {integral} against {}",
                spec.systems()
            );
        }
    }

    #[test]
    fn a_core_s_classes_hold_every_remnant_and_its_dispersion_is_flat() {
        let galaxy = milky_way();
        let spec = spec(&galaxy, 1e7, 4e7, 4e9, [40_000.0, 25_000.0, -8_000.0]);
        let core = DwarfCoreModel::from_spec(&galaxy, &spec);
        let kinds = |kind: ClassKind| core.table().expected_of(kind);
        assert!(kinds(ClassKind::Living) > 0.0 && kinds(ClassKind::WhiteDwarf) > 0.0);
        assert!(kinds(ClassKind::NeutronStar) > 0.0 && kinds(ClassKind::BlackHole) > 0.0);
        assert!(core.table().tail(MassBand::A).is_none());
        // Old: its tail ended 2.4 Gyr ago, τ_q of 1.6 Gyr after its accretion 4 Gyr ago, and
        // nothing above that age's turn-off, about 1.5 M☉, lives, so bands D and E hold only
        // remnants.
        for band in [MassBand::D, MassBand::E] {
            assert!(
                core.table()
                    .classes(band)
                    .all(|(c, _)| c.kind != ClassKind::Living),
                "{band:?}"
            );
        }
        // Every class has the one profile: no segregation.
        let p = PointLy::new(700.0, -300.0, 200.0);
        let reference = core.table().profile(MassBand::A, 0).unwrap().density(&p);
        for band in MassBand::ALL {
            for i in 0..core.table().classes(band).count() {
                let d = core.table().profile(band, i).unwrap().density(&p);
                assert!((d / reference - 1.0).abs() < 1e-12, "{band:?} {i}");
            }
        }
        // Flat in radius (Walker et al. 2007): the observed dispersion everywhere.
        let observed = core.dispersion().value();
        for r in [
            0.0,
            100.0,
            core.plummer_scale().value(),
            core.extent().value(),
        ] {
            assert_eq!(
                core.dispersion_at(LightYears::new(r)).value(),
                observed,
                "{r}"
            );
        }
    }

    /// Every core the dwarfs of six drawn galaxies leave, as drawn and unscaled, has a model whose
    /// classes sum to its systems inside an extent under 20,000 ly.
    #[test]
    fn every_core_of_drawn_galaxies_has_a_model() {
        use super::super::debris::in_list_order;
        use super::super::dwarfs::dwarfs;

        let mut cores = 0;
        for n in 0..6_u64 {
            let galaxy = Galaxy::new(Seed::new(0x0a10_7a10 | n));
            let mut debris = dwarfs(galaxy.seed(), galaxy.params(), galaxy.potential());
            in_list_order(&mut debris);
            for spec in debris.iter().filter_map(|d| d.core.as_ref()) {
                let core = DwarfCoreModel::from_spec(&galaxy, spec);
                let classes: f64 = MassBand::ALL
                    .iter()
                    .map(|&b| core.table().expected(b))
                    .sum();
                assert!((classes / spec.systems() - 1.0).abs() < 1e-12);
                assert!(core.extent().value() <= EXTENT_LIMIT_LY);
                assert!(core.grid().reach().value() >= core.extent().value());
                assert!(core.fe_h().value().is_finite() && core.dispersion().value() > 0.0);
                cores += 1;
            }
        }
        assert!(cores > 0, "no cores in six galaxies");
    }

    #[test]
    fn a_core_is_a_pure_function_of_its_spec_and_draws_its_own_offset() {
        let galaxy = milky_way();
        let first = spec(&galaxy, 5e6, 2e7, 4e9, [30_000.0, -30_000.0, 12_000.0]);
        let core = DwarfCoreModel::from_spec(&galaxy, &first);
        assert_eq!(core, DwarfCoreModel::from_spec(&galaxy, &first));
        let mean = mass_metallicity(SolarMasses::new(2e7)).value();
        let offset = core.fe_h().value() - mean;
        assert!(
            offset.abs() < 5.0 * MEAN_METALLICITY_SCATTER_DEX,
            "{offset}"
        );
        let want = Stream::open(
            galaxy.seed(),
            tags::DWARF_CORE_METALLICITY,
            ObjectKey::galaxy_item(8),
        )
        .normal(0.0, MEAN_METALLICITY_SCATTER_DEX);
        let exact = mass_metallicity(SolarMasses::new(2e7)).value() + want;
        assert!(
            core.fe_h().value().total_cmp(&exact).is_eq(),
            "{offset} {want}"
        );
        // Cores modelled in any order are the same cores (T7.b reads them behind a cache).
        let specs: Vec<DwarfCoreSpec> = (0..4)
            .map(|j| DwarfCoreSpec {
                progenitor: j,
                ..first.clone()
            })
            .collect();
        hyperion_testkit::order::assert_order_independent(&specs, |spec| {
            DwarfCoreModel::from_spec(&galaxy, spec)
        });
        // Another progenitor draws its own.
        let other = DwarfCoreSpec {
            progenitor: 3,
            ..first
        };
        let moved = DwarfCoreModel::from_spec(&galaxy, &other).fe_h().value() - mean;
        assert!((moved - offset).abs() > 1e-9);
        assert!(
            (core.fe_h_spread().value() - metallicity_spread(SolarMasses::new(2e7)).value()).abs()
                < 1e-15
        );
    }
}
