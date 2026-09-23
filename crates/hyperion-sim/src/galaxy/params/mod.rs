//! The galaxy's parameters: drawn from the seed, then derived and coupled (plan 02, P02.T5).
//!
//! The seed chooses the galaxy's gross properties from the ranges observed for large barred
//! spirals (brainstorm, "Galaxy parameters" and "Populations"). Each is drawn from a stream of
//! its own, `galaxy.params.<name>`, so that adding one moves no other. From those
//! [`GalaxyParamsBuilder::build`] derives the rest: the populations' shares of the
//! galaxy's systems and their age distributions, the mean present-day mass of a system in each
//! population, the system count and the populations' masses, then the sizes, coupled to the masses
//! they hold as mass^⅓, and the dark halo.
//!
//! Masses are in M☉, lengths in light-years and times in Julian years, each behind its
//! [`units`](crate::units) newtype. Times of past events are positive durations before the epoch.
//!
//! # Examples
//!
//! ```
//! use hyperion_sim::Seed;
//! use hyperion_sim::galaxy::Population;
//! use hyperion_sim::galaxy::imf::MassFunctionKind;
//! use hyperion_sim::galaxy::params::GalaxyParams;
//!
//! let params = GalaxyParams::from_seed(Seed::new(42), MassFunctionKind::default());
//! // The system count is derived, not drawn: 0.5–1.8 × 10¹¹ over the parameter ranges.
//! assert!((0.5e11..1.9e11).contains(&params.system_count()));
//! // Shares are of systems; masses follow and add up to the stellar mass.
//! let total: f64 = hyperion_sim::galaxy::POPULATIONS
//!     .iter()
//!     .map(|&p| params.population_mass(p).value())
//!     .sum();
//! assert!((total / params.stellar_mass().value() - 1.0).abs() < 1e-12);
//! assert!(params.population_share(Population::YoungThinDisc) < 0.005);
//! ```

mod accretion;
mod derive;
mod draws;
mod halo;
mod inputs;
mod milky_way;
mod validate;

use std::error::Error;
use std::fmt;

pub use accretion::{AccretionHistory, Orbit, Progenitor, ProgenitorKind};
pub use halo::{HaloBreak, HaloComponentKind, HaloComponentParams, HaloParams};
pub use inputs::{ArmCount, LesserProgenitorInput, RecentProgenitorInput};

use self::inputs::Inputs;
use super::Population;
use super::imf::MassFunctionKind;
use crate::Seed;
use crate::units::{
    Degrees, Dex, DexPerKiloparsec, KilometresPerSecond, LightYears, Radians, SolarMasses, Years,
};

/// A disc's scale length and height: exponential in radius, cored in height, with the height the
/// effective height `Σ ÷ 2ρ₀` of the vertical profile the fields solve for it (plan 02, Design
/// note 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscParams {
    length: LightYears,
    height: LightYears,
}

impl DiscParams {
    /// The radial scale length.
    #[must_use]
    pub fn length(&self) -> LightYears {
        self.length
    }

    /// The effective height `Σ ÷ 2ρ₀`.
    ///
    /// For the old thin disc it is that of the five sub-discs together, the harmonic mean of their
    /// own, which the fields meet by scaling their dispersions (plan 02, Design note 9).
    #[must_use]
    pub fn height(&self) -> LightYears {
        self.height
    }
}

/// The boxy bulge: `exp(−m)` with `m = {[(|x| ÷ a)² + (|y| ÷ b)²]^(c∥ ÷ 2) + (|z| ÷ c)^c∥}^(1 ÷ c∥)`
/// (brainstorm, "Populations"; Wegg and Gerhard 2013).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BulgeParams {
    scale_x: LightYears,
    scale_y: LightYears,
    scale_z: LightYears,
    boxiness: f64,
}

impl BulgeParams {
    /// `a`, the scale length along the bar, 1,700–3,000 ly.
    #[must_use]
    pub fn scale_x(&self) -> LightYears {
        self.scale_x
    }

    /// `b`, the scale length across the bar in the plane, 0.5–0.7 of `a`.
    #[must_use]
    pub fn scale_y(&self) -> LightYears {
        self.scale_y
    }

    /// `c`, the vertical scale length, 0.3–0.4 of `a`.
    #[must_use]
    pub fn scale_z(&self) -> LightYears {
        self.scale_z
    }

    /// `c∥`, the vertical exponent that makes the bulge boxy, 3–4.
    #[must_use]
    pub fn boxiness(&self) -> f64 {
        self.boxiness
    }
}

/// The long bar along the x axis: level along most of its length with a Gaussian end, Gaussian
/// across, exponential in height (brainstorm, "Populations"; Wegg, Gerhard and Portail 2015).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarParams {
    half_length: LightYears,
    width: LightYears,
    height: LightYears,
    corotation_ratio: f64,
}

impl BarParams {
    /// The half-length, 10,000–18,000 ly: the arms start at its ends.
    #[must_use]
    pub fn half_length(&self) -> LightYears {
        self.half_length
    }

    /// The Gaussian width σ across the bar, 0.08–0.12 of the half-length.
    #[must_use]
    pub fn width(&self) -> LightYears {
        self.width
    }

    /// The exponential scale height, 500–700 ly.
    #[must_use]
    pub fn height(&self) -> LightYears {
        self.height
    }

    /// The corotation radius over the half-length, 1.0–1.4.
    #[must_use]
    pub fn corotation_ratio(&self) -> f64 {
        self.corotation_ratio
    }

    /// The corotation radius: the ratio times the half-length.
    #[must_use]
    pub fn corotation_radius(&self) -> LightYears {
        self.half_length * self.corotation_ratio
    }
}

/// The nuclear disc at the centre: exponential in radius, cored in height (plan 02, Design note 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NuclearDiscParams {
    length: LightYears,
    height: LightYears,
}

impl NuclearDiscParams {
    /// The radial scale length, 200–400 ly.
    #[must_use]
    pub fn length(&self) -> LightYears {
        self.length
    }

    /// The effective height `Σ ÷ 2ρ₀`, 0.3–0.5 of the length.
    #[must_use]
    pub fn height(&self) -> LightYears {
        self.height
    }
}

/// The nuclear star cluster, as the potential needs it: a broken power law outside the
/// populations' budgets (plan 02, Design note 15).
///
/// Its mass is 0.024 of the nuclear disc's with 0.2 dex of scatter, the Milky Way's 2.5 × 10⁷ M☉
/// (Schödel et al. 2014) against its nuclear disc's 1.05 × 10⁹ M☉ (Sormani et al. 2022); inner
/// slope 1.3, break 10 ly, outer slope 3.5 (brainstorm, "Dense features"). The slope lies between
/// Gallego-Cano et al.'s (2018) 1.43 ± 0.1 for the faint stars and Schödel et al.'s (2018) 1.13 ±
/// 0.05 for the diffuse light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NuclearClusterParams {
    mass: SolarMasses,
}

impl NuclearClusterParams {
    /// The inner logarithmic slope of the density.
    pub const INNER_SLOPE: f64 = 1.3;

    /// The break radius.
    pub const BREAK_RADIUS: LightYears = LightYears::new(10.0);

    /// The outer logarithmic slope of the density.
    pub const OUTER_SLOPE: f64 = 3.5;

    /// The cluster's mass.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The inner logarithmic slope, [`INNER_SLOPE`](Self::INNER_SLOPE).
    #[must_use]
    pub fn inner_slope(&self) -> f64 {
        Self::INNER_SLOPE
    }

    /// The break radius, [`BREAK_RADIUS`](Self::BREAK_RADIUS).
    #[must_use]
    pub fn break_radius(&self) -> LightYears {
        Self::BREAK_RADIUS
    }

    /// The outer logarithmic slope, [`OUTER_SLOPE`](Self::OUTER_SLOPE).
    #[must_use]
    pub fn outer_slope(&self) -> f64 {
        Self::OUTER_SLOPE
    }
}

/// The spiral arms: logarithmic spirals that modulate the discs' densities (plan 02, Design note
/// 10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmParams {
    count: ArmCount,
    pitch: Radians,
    young_width: LightYears,
    young_fraction: f64,
    old_amplitude: f64,
}

impl ArmParams {
    /// Two arms or four.
    #[must_use]
    pub fn count(&self) -> ArmCount {
        self.count
    }

    /// The pitch angle, 10–18°.
    #[must_use]
    pub fn pitch(&self) -> Radians {
        self.pitch
    }

    /// The young disc's arm width `σ_w`, 250–500 ly.
    #[must_use]
    pub fn young_width(&self) -> LightYears {
        self.young_width
    }

    /// The young disc's arm amplitude A, 0.7–0.9: the fraction of young stars bound to the arms.
    #[must_use]
    pub fn young_fraction(&self) -> f64 {
        self.young_fraction
    }

    /// The old discs' cosine amplitude a, 0.10–0.30: the "10–30% ripple" of the brainstorm.
    #[must_use]
    pub fn old_amplitude(&self) -> f64 {
        self.old_amplitude
    }
}

/// The gas disc, as the potential needs it (plan 02, Design note 15; plan 07 owns the gas field).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasDiscParams {
    mass: SolarMasses,
    length: LightYears,
}

impl GasDiscParams {
    /// The gas disc's scale height, 700 ly.
    ///
    /// The brainstorm's 400 ly times 7 ÷ 4 (plan 02, ruling 1 of 2026-09-22): the column plan 07's
    /// gas field needs at the Sun's radius, McKee, Parravano and Hollenbach's (2015, ApJ 814, 13)
    /// 13.7 ± 1.6 M☉ pc⁻², is carried by thickening the neutral layer rather than densifying the
    /// plane, whose density the in-plane extinction reads. A real neutral layer carries its column
    /// well above 123 pc, which is what 400 ly is. With plan 07's warm ionised layer drawn by its
    /// own density (its ruling 19) the fixture's 24% of gas gives 13.8 M☉ pc⁻² on this height, a
    /// neutral mid-plane of 0.80 cm⁻³ and 1.06 mag per 3,000 ly; 700 ly (215 pc) is already 1.4
    /// times the measured atomic layer's effective height, 156 pc (McKee et al., Table 2: 10.9 M☉
    /// pc⁻² over a mid-plane 1.01 cm⁻³), so the height stays and the mass is what was tuned.
    pub const HEIGHT: LightYears = LightYears::new(700.0);

    /// Its mass, 17.5–35% of the thin disc's stellar mass ([`HEIGHT`](Self::HEIGHT)); 24% for the
    /// Milky Way fixture.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// Its scale length, 1.5–2 times the thin disc's.
    #[must_use]
    pub fn length(&self) -> LightYears {
        self.length
    }

    /// Its scale height, [`HEIGHT`](Self::HEIGHT).
    #[must_use]
    pub fn height(&self) -> LightYears {
        Self::HEIGHT
    }
}

/// The NFW dark halo (brainstorm, "Galaxy parameters").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DarkHaloParams {
    f_star: f64,
    m200: SolarMasses,
    concentration: f64,
    r200: LightYears,
}

impl DarkHaloParams {
    /// f★, the efficiency factor in `M₂₀₀ = M★ ÷ (0.157 f★)`, 0.12–0.45.
    #[must_use]
    pub fn f_star(&self) -> f64 {
        self.f_star
    }

    /// The mass inside `r₂₀₀`.
    #[must_use]
    pub fn m200(&self) -> SolarMasses {
        self.m200
    }

    /// The concentration `c₂₀₀ = r₂₀₀ ÷ r_s`.
    #[must_use]
    pub fn concentration(&self) -> f64 {
        self.concentration
    }

    /// The radius inside which the mean density is 200 times the critical density.
    #[must_use]
    pub fn r200(&self) -> LightYears {
        self.r200
    }

    /// The NFW scale radius, `r₂₀₀ ÷ c₂₀₀`.
    #[must_use]
    pub fn scale_radius(&self) -> LightYears {
        self.r200 / self.concentration
    }
}

/// The central black hole, whose mass follows the M–σ relation with scatter (brainstorm, "Galaxy
/// parameters"; plan 02, P02.T6.e and Design note 8).
///
/// The build is in two phases: the mass model without the black hole and the nuclear cluster
/// gives the bulge's dispersion ([`potential::sigma`](super::potential::sigma)), and McConnell
/// and Ma's (2013) relation with the drawn scatter gives the mass, which the final model then
/// holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlackHoleParams {
    scatter: Dex,
    bulge_dispersion: KilometresPerSecond,
    mass: SolarMasses,
}

impl BlackHoleParams {
    /// The black hole's offset from the M–σ relation, normal with 0.38 dex of scatter.
    #[must_use]
    pub fn scatter(&self) -> Dex {
        self.scatter
    }

    /// The bulge's projected velocity dispersion inside its effective radius, which the M–σ
    /// relation reads ([`bulge_dispersion`](super::potential::sigma::bulge_dispersion)).
    #[must_use]
    pub fn bulge_dispersion(&self) -> KilometresPerSecond {
        self.bulge_dispersion
    }

    /// The black hole's mass: `10^(8.32 + 5.64 log₁₀(σ ÷ 200 km/s) + scatter)` M☉.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }
}

/// A [`GalaxyParams`] could not be built: a parameter lies outside its range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildGalaxyParamsError {
    /// `parameter` is `value`, outside `[min, max]` (or NaN).
    OutOfRange {
        /// The parameter, named as its tag without the `galaxy.params.` prefix, such as
        /// `share.thick`; a fixed size is named by its length, such as `thin.length`.
        parameter: &'static str,
        /// The value given.
        value: f64,
        /// The lowest value allowed.
        min: f64,
        /// The highest value allowed.
        max: f64,
    },
    /// The number of lesser old progenitors is not 2–5.
    LesserProgenitorCount {
        /// The number given.
        count: usize,
    },
    /// The lesser progenitors' weights sum to zero.
    LesserWeightsZero,
}

impl fmt::Display for BuildGalaxyParamsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange {
                parameter,
                value,
                min,
                max,
            } => write!(f, "{parameter} = {value} lies outside [{min}, {max}]"),
            Self::LesserProgenitorCount { count } => {
                write!(f, "a halo has 2–5 lesser progenitors, not {count}")
            }
            Self::LesserWeightsZero => f.write_str("the lesser progenitors' weights sum to zero"),
        }
    }
}

impl Error for BuildGalaxyParamsError {}

/// Everything the seed decides about a galaxy, drawn and derived.
///
/// Build one from a seed ([`from_seed`](Self::from_seed)), take the Milky Way fixture
/// ([`milky_way_like`](Self::milky_way_like)), or set values by hand with
/// [`GalaxyParamsBuilder`]. It is immutable, and a pure function of its inputs and the generator
/// version.
#[derive(Debug, Clone, PartialEq)]
pub struct GalaxyParams {
    mass_function: MassFunctionKind,
    stellar_mass: SolarMasses,
    sfh_timescale: Years,
    bar_of_bulge: f64,
    shares: [f64; 7],
    mean_masses: [SolarMasses; 7],
    masses: [SolarMasses; 7],
    system_count: f64,
    mean_formed_mass: SolarMasses,
    thin_disc: DiscParams,
    young_disc: DiscParams,
    thick_disc: DiscParams,
    bulge: BulgeParams,
    bar: BarParams,
    nuclear_disc: NuclearDiscParams,
    nuclear_cluster: NuclearClusterParams,
    arms: ArmParams,
    gas_disc: GasDiscParams,
    dark_halo: DarkHaloParams,
    black_hole: BlackHoleParams,
    metallicity_gradient: DexPerKiloparsec,
    halo: HaloParams,
    accretion: AccretionHistory,
}

impl GalaxyParams {
    /// The parameters the seed draws, with `mass_function` for the mean masses.
    ///
    /// Every parameter comes from its own stream, `Stream::open(seed, tag, key)` with the tag
    /// `galaxy.params.<name>` and `ObjectKey::galaxy()`, or `ObjectKey::galaxy_item(n)` for a
    /// list (plan 02, Design note 2).
    ///
    /// # Panics
    ///
    /// Never: every value is drawn inside the range the builder checks.
    #[must_use]
    pub fn from_seed(seed: Seed, mass_function: MassFunctionKind) -> Self {
        derive::build(&draws::draw_inputs(seed, mass_function))
            .expect("every drawn value lies inside the range it is drawn from")
    }

    /// The Milky Way fixture: the galaxy's measured values, without scatter but for the black
    /// hole's, for the comparisons of plan 02's P02.T11.
    ///
    /// The default mass function, Chabrier's system function with its branch above 1 M☉ scaled;
    /// M★ 6.0 × 10¹⁰ M☉ (Licquia and Newman 2015); shares thick 10%, bulge and bar 31% with the
    /// bar 30% of that (Bland-Hawthorn and Gerhard 2016; Portail et al. 2017), nuclear disc 1.75%
    /// (Launhardt et al. 2002; Sormani et al. 2022), halo 1%; timescale 7 Gyr; thin disc 7,000 ly
    /// long with an effective height of 1,100 ly (Bovy and Rix 2013; Bland-Hawthorn and Gerhard
    /// 2016; plan 02, ruling 8), thick disc 0.9 and 2.7 times that, young disc 285 ly (ruling 3),
    /// gas 24% of the thin disc's mass (rulings 1 and 19); bulge 2,280 × 1,440 × 820 ly, boxiness 3.5 (Wegg and Gerhard 2013); bar half-length 16,000 ly, height 590 ly
    /// (Wegg, Gerhard and Portail 2015), corotation ratio 1.24 (Portail et al. 2017); nuclear disc 290 ly by 93 ly
    /// (Sormani et al. 2022); four arms at 12°; f★ 0.32, so M₂₀₀ lies near the 1.3 × 10¹² M☉ of
    /// McMillan (2017); the black hole 0.512 dex below the M–σ relation, which makes it the 4.30 ×
    /// 10⁶ M☉ of Sgr A* (GRAVITY Collaboration 2022; McConnell and Ma 2013); the halo's inner
    /// slopes 2.5, and the dominant merger's break at 58,700 ly (18 kpc), steepening by 2.0
    /// (Pila-Díez et al. 2015; Medina et al. 2024). Values without a measurement take the middle
    /// of their ranges.
    ///
    /// # Panics
    ///
    /// Never: the fixture's values lie inside their ranges, which a test checks.
    #[must_use]
    pub fn milky_way_like() -> Self {
        derive::build(&milky_way::inputs()).expect("the fixture's values lie inside their ranges")
    }

    /// Which mass function the mean masses use.
    #[must_use]
    pub fn mass_function(&self) -> MassFunctionKind {
        self.mass_function
    }

    /// The galaxy's stellar mass, 3–10 × 10¹⁰ M☉, log-uniform.
    #[must_use]
    pub fn stellar_mass(&self) -> SolarMasses {
        self.stellar_mass
    }

    /// The timescale τ of the thin disc's declining formation rate, 5–9 Gyr.
    #[must_use]
    pub fn sfh_timescale(&self) -> Years {
        self.sfh_timescale
    }

    /// The long bar's part of the combined bulge-and-bar share, 30–40% (plan 02, Risks, R1).
    #[must_use]
    pub fn bar_share_of_bulge(&self) -> f64 {
        self.bar_of_bulge
    }

    /// The population's share of the galaxy's systems (born at the epoch). The shares sum to 1.
    ///
    /// The thick disc, the bulge with its bar, the nuclear disc and the halo are drawn; the thin
    /// disc takes the rest, and its young part is the share of the declining history in the last
    /// 100 Myr (plan 02, Design note 3).
    #[must_use]
    pub fn population_share(&self, population: Population) -> f64 {
        self.shares[population.index()]
    }

    /// The mean present-day mass of a system of the population, living stars, remnants and
    /// companions together ([`fates::mean_present_mass`](super::fates::mean_present_mass)).
    #[must_use]
    pub fn mean_system_mass(&self, population: Population) -> SolarMasses {
        self.mean_masses[population.index()]
    }

    /// The population's stellar mass: `N × share × mean mass`. The masses sum to the stellar mass.
    #[must_use]
    pub fn population_mass(&self, population: Population) -> SolarMasses {
        self.masses[population.index()]
    }

    /// The expected number of systems born at the epoch: the stellar mass over
    /// `Σ share × mean mass`.
    #[must_use]
    pub fn system_count(&self) -> f64 {
        self.system_count
    }

    /// The initial mass formed per system, with nothing dead
    /// ([`fates::mean_formed_mass`](super::fates::mean_formed_mass)).
    #[must_use]
    pub fn mean_formed_mass(&self) -> SolarMasses {
        self.mean_formed_mass
    }

    /// The old thin disc: its scale length and its effective height, 850–1,150 ly, the sub-discs'
    /// harmonic mean.
    #[must_use]
    pub fn thin_disc(&self) -> &DiscParams {
        &self.thin_disc
    }

    /// The young thin disc: the thin disc's scale length and an effective height of 225–345 ly.
    #[must_use]
    pub fn young_disc(&self) -> &DiscParams {
        &self.young_disc
    }

    /// The thick disc: 0.7–0.9 of the thin disc's length and 2.7–3.3 of its effective height.
    #[must_use]
    pub fn thick_disc(&self) -> &DiscParams {
        &self.thick_disc
    }

    /// The bulge.
    #[must_use]
    pub fn bulge(&self) -> &BulgeParams {
        &self.bulge
    }

    /// The long bar.
    #[must_use]
    pub fn bar(&self) -> &BarParams {
        &self.bar
    }

    /// The nuclear disc.
    #[must_use]
    pub fn nuclear_disc(&self) -> &NuclearDiscParams {
        &self.nuclear_disc
    }

    /// The nuclear star cluster.
    #[must_use]
    pub fn nuclear_cluster(&self) -> &NuclearClusterParams {
        &self.nuclear_cluster
    }

    /// The spiral arms.
    #[must_use]
    pub fn arms(&self) -> &ArmParams {
        &self.arms
    }

    /// The gas disc.
    #[must_use]
    pub fn gas_disc(&self) -> &GasDiscParams {
        &self.gas_disc
    }

    /// The dark halo.
    #[must_use]
    pub fn dark_halo(&self) -> &DarkHaloParams {
        &self.dark_halo
    }

    /// The central black hole.
    #[must_use]
    pub fn black_hole(&self) -> &BlackHoleParams {
        &self.black_hole
    }

    /// The discs' radial \[Fe/H\] gradient, −0.07 to −0.04 dex per kpc (the brainstorm's "about
    /// −0.05 dex per kpc in the Milky Way disc").
    #[must_use]
    pub fn metallicity_gradient(&self) -> DexPerKiloparsec {
        self.metallicity_gradient
    }

    /// The stellar halo's components.
    #[must_use]
    pub fn halo(&self) -> &HaloParams {
        &self.halo
    }

    /// The accretion history.
    #[must_use]
    pub fn accretion(&self) -> &AccretionHistory {
        &self.accretion
    }

    /// The bytes the parameters own on the heap: the halo's components with their ages, and the
    /// progenitors.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        let halo = &self.halo.components;
        halo.capacity() * size_of::<HaloComponentParams>()
            + halo.iter().map(|c| c.ages.heap_bytes()).sum::<usize>()
            + self.accretion.progenitors.capacity() * size_of::<Progenitor>()
    }
}

/// Builds [`GalaxyParams`] from values set by hand, checking every one against its range.
///
/// A builder starts from the Milky Way fixture's values ([`GalaxyParams::milky_way_like`]), so a
/// test sets only what it varies. The sizes that are coupled to masses can be fixed (as the
/// fixture fixes them) or coupled with a scatter in dex (as a drawn galaxy couples them); the
/// last setter called wins.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::params::{BuildGalaxyParamsError, GalaxyParamsBuilder};
/// use hyperion_sim::units::{Dex, SolarMasses};
///
/// let heavier = GalaxyParamsBuilder::new()
///     .stellar_mass(SolarMasses::new(9e10))
///     .thin_length_scatter(Dex::new(0.0))
///     .build()?;
/// assert!(heavier.thin_disc().length().value() > 8_480.0);
///
/// let error = GalaxyParamsBuilder::new().thick_share(0.2).build().unwrap_err();
/// assert!(matches!(
///     error,
///     BuildGalaxyParamsError::OutOfRange { parameter: "share.thick", .. }
/// ));
/// # Ok::<(), BuildGalaxyParamsError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GalaxyParamsBuilder {
    inputs: Inputs,
}

impl Default for GalaxyParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Setters that store one value, converted to working units, into the builder's inputs.
macro_rules! setters {
    ($( $(#[$meta:meta])* $name:ident($unit:ty) => $field:ident $(= $convert:expr)?; )*) => {
        $(
            $(#[$meta])*
            #[must_use]
            pub fn $name(mut self, value: $unit) -> Self {
                self.inputs.$field = setters!(@convert value $(, $convert)?);
                self
            }
        )*
    };
    (@convert $value:ident) => { $value };
    (@convert $value:ident, $convert:expr) => { $convert($value) };
}

impl GalaxyParamsBuilder {
    /// A builder holding the Milky Way fixture's values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inputs: milky_way::inputs(),
        }
    }

    setters! {
        /// The mass function the mean masses use.
        mass_function(MassFunctionKind) => mass_function;
        /// The stellar mass, 3–10 × 10¹⁰ M☉.
        stellar_mass(SolarMasses) => stellar_mass = SolarMasses::value;
        /// The thick disc's share of systems, 8–14%.
        thick_share(f64) => share_thick;
        /// The bulge's and the long bar's share together, 20–35%.
        bulge_bar_share(f64) => share_bulge_bar;
        /// The long bar's part of the bulge-and-bar share, 30–40%.
        bar_share_of_bulge(f64) => share_bar_of_bulge;
        /// The nuclear disc's share of systems, 1–2.5%.
        nuclear_disc_share(f64) => share_nuclear_disc;
        /// The halo's share of systems, 0.7–1.4%.
        halo_share(f64) => share_halo;
        /// The thin disc's formation timescale, 5–9 Gyr.
        sfh_timescale(Years) => sfh_timescale = Years::value;
        /// The thin disc's scale length, fixed, 7,000–11,500 ly.
        thin_length(LightYears) => thin_length = Inputs::fixed;
        /// The thin disc's scale length coupled to its mass, with this scatter (within ±9 × 0.05
        /// dex).
        thin_length_scatter(Dex) => thin_length = Inputs::coupled;
        /// The old thin disc's effective height, 850–1,150 ly.
        thin_mean_height(LightYears) => thin_mean_height = LightYears::value;
        /// The young disc's effective height, 225–345 ly.
        young_height(LightYears) => young_height = LightYears::value;
        /// The thick disc's length over the thin disc's, 0.7–0.9.
        thick_length_ratio(f64) => thick_length_ratio;
        /// The thick disc's height over the thin disc's mean, 2.7–3.3.
        thick_height_ratio(f64) => thick_height_ratio;
        /// The bulge's long scale length, fixed, 1,700–3,000 ly.
        bulge_length(LightYears) => bulge_length = Inputs::fixed;
        /// The bulge's long scale length coupled to its mass, with this scatter (within ±9 × 0.06
        /// dex).
        bulge_length_scatter(Dex) => bulge_length = Inputs::coupled;
        /// The bulge's middle axis over its long axis, 0.5–0.7.
        bulge_b_over_a(f64) => bulge_b_over_a;
        /// The bulge's short axis over its long axis, 0.3–0.4.
        bulge_c_over_a(f64) => bulge_c_over_a;
        /// The bulge's vertical exponent, 3–4.
        bulge_boxiness(f64) => bulge_boxiness;
        /// The long bar's half-length, fixed, 10,000–18,000 ly.
        bar_half_length(LightYears) => bar_length = Inputs::fixed;
        /// The long bar's half-length coupled to its mass, with this scatter (within ±9 × 0.05
        /// dex).
        bar_length_scatter(Dex) => bar_length = Inputs::coupled;
        /// The long bar's width over its half-length, 0.08–0.12.
        bar_width_ratio(f64) => bar_width_ratio;
        /// The long bar's scale height, 500–700 ly, of its exponential profile in height.
        bar_height(LightYears) => bar_height = LightYears::value;
        /// The bar's corotation radius over its half-length, 1.0–1.4.
        bar_corotation_ratio(f64) => bar_corotation_ratio;
        /// The nuclear disc's scale length, fixed, 200–400 ly.
        nuclear_length(LightYears) => nuclear_length = Inputs::fixed;
        /// The nuclear disc's scale length coupled to its mass, with this scatter (within ±9 ×
        /// 0.04 dex).
        nuclear_length_scatter(Dex) => nuclear_length = Inputs::coupled;
        /// The nuclear disc's height over its length, 0.3–0.5.
        nuclear_height_ratio(f64) => nuclear_height_ratio;
        /// The nuclear cluster's mass scatter (within ±9 × 0.2 dex).
        nuclear_cluster_mass_scatter(Dex) => nuclear_cluster_mass_scatter = Dex::value;
        /// Two arms or four.
        arm_count(ArmCount) => arm_count;
        /// The arms' pitch angle, 10–18°.
        arm_pitch(Degrees) => arm_pitch = Degrees::value;
        /// The young disc's arm width `σ_w`, 250–500 ly.
        arm_young_width(LightYears) => arm_young_width = LightYears::value;
        /// The young disc's arm amplitude A, 0.7–0.9.
        arm_young_fraction(f64) => arm_young_fraction;
        /// The old discs' arm amplitude a, 0.10–0.30.
        arm_old_amplitude(f64) => arm_old_amplitude;
        /// The gas disc's mass over the thin disc's, 0.175–0.35.
        gas_mass_fraction(f64) => gas_mass_fraction;
        /// The gas disc's length over the thin disc's, 1.5–2.0.
        gas_length_ratio(f64) => gas_length_ratio;
        /// The dark halo's f★, 0.12–0.45.
        dark_f_star(f64) => dark_f_star;
        /// The dark halo's concentration scatter (within ±9 × 0.11 dex).
        dark_concentration_scatter(Dex) => dark_concentration_scatter = Dex::value;
        /// The black hole's scatter about M–σ (within ±9 × 0.38 dex).
        black_hole_scatter(Dex) => bh_scatter = Dex::value;
        /// The discs' metallicity gradient, −0.07 to −0.04 dex per kpc.
        metallicity_gradient(DexPerKiloparsec) => metallicity_gradient = DexPerKiloparsec::value;
        /// The dominant merger's break radius, 52,000–91,000 ly.
        halo_dominant_break_radius(LightYears) => halo_dominant_break_radius = LightYears::value;
        /// How much the dominant merger's slope steepens beyond its break, 1.5–2.5.
        halo_dominant_break_steepening(f64) => halo_dominant_break_steepening;
        /// The lesser progenitors' combined share of the halo before renormalising, 10–25%.
        halo_lesser_share_total(f64) => halo_lesser_share_total;
        /// The lesser old progenitors, 2–5 of them.
        halo_lesser_progenitors(Vec<LesserProgenitorInput>) => halo_lesser;
        /// The halo's discrete share, 2–15%.
        halo_discrete_share(f64) => halo_discrete_share;
        /// The last major merger, 6–11 Gyr before the epoch. The in-situ and dominant halo
        /// components' stars must be at least as old.
        last_major_merger(Years) => last_major_merger = Years::value;
        /// The dominant merger's orbit, with an eccentricity of 0.85–0.95.
        dominant_orbit(Orbit) => dominant_orbit;
        /// The recent progenitors.
        recent_progenitors(Vec<RecentProgenitorInput>) => recent;
        /// The globular cluster count's scatter (within ±9 × 0.2 dex).
        globular_count_scatter(Dex) => globular_count_scatter = Dex::value;
    }

    /// The in-situ halo: its share before renormalising (15–30%), axis ratio (0.45–0.55), core
    /// (1,500–3,000 ly), slope (2.2–2.8) and the centre of its one-gigayear age range
    /// (10.5–12.5 Gyr, and at least half a gigayear before the last major merger, which heated
    /// it).
    #[must_use]
    pub fn halo_in_situ(
        mut self,
        share: f64,
        flattening: f64,
        core: LightYears,
        slope: f64,
        age_centre: Years,
    ) -> Self {
        self.inputs.halo_in_situ = inputs::HaloComponentInput {
            share,
            flattening,
            core: core.value(),
            slope,
            age_centre: age_centre.value(),
        };
        self
    }

    /// The dominant merger's halo component: share before renormalising (35–60%), axis ratio
    /// (0.6–0.8), core (2,000–5,000 ly), slope (2.2–2.8) and age centre (10.5–12.5 Gyr, and at
    /// least half a gigayear before the last major merger, when its star formation stopped).
    #[must_use]
    pub fn halo_dominant(
        mut self,
        share: f64,
        flattening: f64,
        core: LightYears,
        slope: f64,
        age_centre: Years,
    ) -> Self {
        self.inputs.halo_dominant = inputs::HaloComponentInput {
            share,
            flattening,
            core: core.value(),
            slope,
            age_centre: age_centre.value(),
        };
        self
    }

    /// The globular-born debris: share before renormalising (8–15%), core (3,000–5,000 ly),
    /// slope (4.0–4.5) and age centre (10.5–12.5 Gyr). It is spherical.
    #[must_use]
    pub fn halo_debris(
        mut self,
        share: f64,
        core: LightYears,
        slope: f64,
        age_centre: Years,
    ) -> Self {
        self.inputs.halo_debris = inputs::HaloComponentInput {
            share,
            flattening: draws::DEBRIS_FLATTENING,
            core: core.value(),
            slope,
            age_centre: age_centre.value(),
        };
        self
    }

    /// Checks every value against its range and derives the rest.
    ///
    /// # Errors
    ///
    /// [`BuildGalaxyParamsError::OutOfRange`] naming the first value outside its range, in the
    /// order of plan 02's table but for the last major merger, which is checked before the halo
    /// whose ages depend on it; [`BuildGalaxyParamsError::LesserProgenitorCount`] unless there
    /// are 2–5 lesser progenitors; [`BuildGalaxyParamsError::LesserWeightsZero`] if their weights
    /// sum to zero.
    pub fn build(self) -> Result<GalaxyParams, BuildGalaxyParamsError> {
        derive::build(&self.inputs)
    }
}
