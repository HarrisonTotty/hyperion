//! A giant's regular satellites, and their derivation (plan 14, P14.T17).
//!
//! # The system (P14.T17.a)
//!
//! A parent over [`REGULAR_MOON_MIN_PARENT_MASS`] (10 M⊕) with a hydrogen envelope, an ice giant
//! or a gas giant ([`hosts_regular_moons`]), has a satellite system whose total mass is
//! `M_p` × 10^N(−3.8, 0.3) ([`SATELLITE_MASS_RATIO_MEDIAN_DEX`], [`SATELLITE_MASS_RATIO_SIGMA_DEX`]):
//! Canup and Ward's (2006) common ratio near 10⁻⁴. It is split among 1–6 major moons, their count a
//! zero-truncated Poisson law of mean 3.5 ([`MAJOR_MOON_RATE`], [`MAJOR_MOON_MAX`]), by P14.T7's
//! correlated form with the moons' own step and spread (ruling 86): moon k of n, counted inside
//! out, takes the share 10^(`σ_w` εₖ + s (k − 1 − (n − 1) ÷ 2)) of the total, normalised, with
//! s = 0.30 dex per step outward and `σ_w` = 0.40 dex ([`MOON_OUTWARD_STEP_DEX`],
//! [`MOON_SCATTER_DEX`]), independent of the planets' fitted values. With probability 1⁄3
//! ([`TITAN_PROBABILITY`]) the system is Titan-like: its outermost moon takes 0.90–0.98 of the
//! total ([`TITAN_SHARE`]), and the others split the rest by the same law.
//!
//! The innermost moon sits uniformly at 3–8 planetary radii ([`INNERMOST_RADII`]), with its
//! pericentre outside the fluid Roche limit (the range moves out, keeping its ratio, about a
//! planet dense enough that the limit is beyond 3 radii). Each next moon follows at P14.T6's spacing about the planet, Δ = 15 + N(0, 3)
//! mutual Hill radii ([`MOON_SPACING_MEAN`], [`PAIR_SPACING_SIGMA`]), redrawn under design note 7's
//! floor at most [`MAX_SPACING_REDRAWS`] times and then the floor, or, with probability 0.5
//! ([`RESONANCE_PROBABILITY`]), exactly at twice its inner neighbour's period where the floor still
//! holds there, as the Galilean moons and those of TRAPPIST-1's analogues are. Eccentricities are
//! Rayleigh of scale 0.005 ([`FREE_ECCENTRICITY_SIGMA`]), raised in a resonance to a forced value
//! log-uniform in 0.004–0.04 ([`FORCED_ECCENTRICITY`]); inclinations to the planet's equator are
//! Rayleigh of scale 0.5° ([`INCLINATION_SIGMA`]); the node, periapsis and mean anomaly are uniform.
//! Where an eccentricity breaks the floor's gap of 2√3 mutual Hill radii, the outer moon's is
//! halved until it holds, as P14.T8.d does for planets.
//!
//! Every moon must lie inside a twentieth of the parent's Hill radius at its pericentre
//! ([`HILL_FRACTION`]), and the system inside the heaviest moon tides let survive
//! ([`MoonParent::maximum_moon_mass`], P14.T15): what does not fit is dropped from the outside in.
//! Small inner moonlets are a count only ([`RegularMoons::moonlets`]).
//!
//! # The derivation (P14.T17.b)
//!
//! [`derive_regular_moon`] runs P14.T16's [`derive_body`] with the planet as the tidal primary and
//! the star as the source of light: the moon orbits the planet's mass, is lit by the planet's own
//! glow at its own distance and by the star at the planet's orbit, and its composition is set by
//! which side of the circumplanetary ice line it formed on ([`MoonNursery`]): 170 K in the planet's
//! own early luminosity, its luminosity when its star's disc disperses, when the last generation of
//! satellites forms. Tidal heating ([`tidal_heating`]) gives a surface heat flux, a volcanism level
//! and the subsurface-ocean flag ([`has_subsurface_ocean`]).
//!
//! # Draws
//!
//! On the planet's own streams (design note 4), [`tags::MOON_COUNT`], [`tags::MOON_MASS`] and
//! [`tags::MOON_ORBIT`], whose word layouts their doc comments give. Moon k reads its own block of
//! words, so a system's draws never depend on how many moons an earlier step kept.

use std::error::Error;
use std::fmt;

use crate::Seed;
use crate::math;
use crate::orbit::KeplerElements;
use crate::planetary::derive::{
    BodyHosts, BuildBodyHostsError, BuildPlacedBodyError, DeriveBodyError, DerivedBody, HostLight,
    Illumination, PlacedBody, PlanetClass, derive_body,
};
use crate::planetary::disc::{self, DiscDraws, DiscHost, DiscProfile, Truncation};
use crate::planetary::moons::{MoonParent, draw_rank, log_uniform, moon_orbit, rayleigh};
use crate::planetary::params::{ROCKY_LOVE_NUMBER, ROCKY_TIDAL_Q};
use crate::planetary::placement::spacing::{
    MAX_SPACING_REDRAWS, Neighbour, PAIR_SPACING_SIGMA, mutual_hill_factor, next_semi_major_axis,
    satisfies_floor, spacing_floor,
};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::draws::UnitUniform;
use crate::stellar::substellar::{GIANT_MIN_MASS, giant_cooling};
use crate::time::UniverseTime;
use crate::units::consts::{GRAVITATIONAL_CONSTANT, STEFAN_BOLTZMANN};
use crate::units::{
    Degrees, EarthMasses, JupiterMasses, Kelvin, Kilograms, KilogramsPerCubicMetre, Metres,
    Radians, Seconds, SolarLuminosities, SolarMasses, SolarRadii, Watts, WattsPerSquareMetre,
    Years,
};

/// The least mass of a parent with regular moons: 10 M⊕, with an envelope (P14.T17.a), which the
/// classes [`PlanetClass::IceGiant`] and [`PlanetClass::GasGiant`] both imply.
pub const REGULAR_MOON_MIN_PARENT_MASS: EarthMasses = EarthMasses::new(10.0);

/// The median of log₁₀ of a satellite system's mass over its planet's: −3.8, 1.6 × 10⁻⁴
/// (P14.T17.a).
///
/// Canup and Ward (2006, Nature 441, 834, abstract): satellite systems formed in a gas-starved
/// circumplanetary disc hold "approximately 10⁻⁴" of their planet's mass, whatever its mass;
/// their review (Canup and Ward 2009, arXiv:0812.4995, §4.3) gives about 2.5 × 10⁻⁴ analytically.
/// The real systems hold 2.07 × 10⁻⁴ (the Galilean moons), 2.47 × 10⁻⁴ (Saturn's eight regular
/// moons) and 1.04 × 10⁻⁴ (Uranus's five major moons), from JPL's satellite GM values.
pub const SATELLITE_MASS_RATIO_MEDIAN_DEX: f64 = -3.8;

/// The scatter of log₁₀ of the satellite mass ratio: 0.3 dex (P14.T17.a), so that the middle two
/// thirds of systems hold 0.8–3.2 × 10⁻⁴, spanning the three real systems.
pub const SATELLITE_MASS_RATIO_SIGMA_DEX: f64 = 0.3;

/// The rate λ of the zero-truncated Poisson law of the count of major moons: 3.5, which with the
/// cap of [`MAJOR_MOON_MAX`] gives a mean count of 3.4995 (P14.T17.a's "mean 3.5").
pub const MAJOR_MOON_RATE: f64 = 3.5;

/// The most major moons a planet has: 6 (P14.T17.a).
pub const MAJOR_MOON_MAX: u8 = 6;

/// The step in log₁₀ mass per rank outward of a regular moon: 0.30 dex (ruling 86.1).
///
/// JPL's satellite GM table: 9 of the 13 adjacent pairs of major regular moons are outer-heavier
/// (0.69), with a step of +0.27 ± 0.24 dex; mid-sized moons grow outward from spreading rings
/// (Crida and Charnoz 2012, Science 338, 1196), Jupiter's do not. The moons' own law, independent
/// of P14.T7's fit for planets.
pub const MOON_OUTWARD_STEP_DEX: f64 = 0.30;

/// The within-system scatter of log₁₀ of a regular moon's mass: 0.40 dex (ruling 86.1).
pub const MOON_SCATTER_DEX: f64 = 0.40;

/// The probability that a regular system is Titan-like, one outer moon holding almost all its
/// mass: 1⁄3 (ruling 86.2), provisional.
///
/// Sasaki, Stewart and Ida (2010, ApJ 714, 1052): 70% of their Saturn-like runs leave one outer
/// body, and 31 of 67 hold over 95%; Titan holds 0.96 of Saturn's regular moons.
pub const TITAN_PROBABILITY: f64 = 1.0 / 3.0;

/// The share of a Titan-like system's total its outermost moon takes, uniform: 0.90–0.98 (ruling
/// 86.2).
pub const TITAN_SHARE: (f64, f64) = (0.90, 0.98);

/// The mean count of a regular system's small inner moonlets, Poisson: 6 (P14.T17.a's "a count
/// only").
///
/// This lane's choice, where the plan gives none: Jupiter has 4 inner moons inside Io (Metis,
/// Adrastea, Amalthea, Thebe), Neptune 6 inside Proteus and Uranus 13 inside Miranda, all under
/// 200 km. Provisional.
pub const MOONLET_MEAN: f64 = 6.0;

/// Where the innermost regular moon sits: uniformly between 3 and 8 planetary radii (P14.T17.a).
///
/// Mimas at 3.09 Saturn radii, Proteus at 4.75 Neptune radii, Miranda at 5.08 Uranus radii and Io
/// at 5.90 Jupiter radii (JPL's mean elements) all lie inside.
pub const INNERMOST_RADII: (f64, f64) = (3.0, 8.0);

/// The density at which the innermost moon is held outside its parent's fluid Roche limit: 500
/// kg m⁻³, below any moon's density, so that every moon lies outside the limit for its own
/// (P14.T17.a).
///
/// Saturn's least dense regular moons, Pan and Atlas inside its rings, are near 400–500 kg m⁻³ and
/// are rubble at their Roche limit (Porco et al. 2007, Science 318, 1602); the major moons are
/// 1,100–3,500. This lane's choice, provisional.
pub const PLACEMENT_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(500.0);

/// The mean spacing of neighbouring regular moons: 15 mutual Hill radii (P14.T17.a), with
/// P14.T6.b's scatter of [`PAIR_SPACING_SIGMA`] about it.
///
/// Io and Europa, at the 2:1 period ratio, are 15.7 mutual Hill radii apart.
pub const MOON_SPACING_MEAN: f64 = 15.0;

/// The probability that a pair of neighbouring regular moons sits at the 2:1 resonance: 0.5
/// (P14.T17.a).
///
/// Europa and Io are at 2.0000 and Ganymede and Europa at 2.0297 (JPL's periods).
pub const RESONANCE_PROBABILITY: f64 = 0.5;

/// The ratio of periods of a resonant pair: 2 (P14.T17.a's 2:1).
pub const RESONANT_PERIOD_RATIO: f64 = 2.0;

/// The scale of the Rayleigh law of a regular moon's free eccentricity: 0.005 (P14.T17.a).
pub const FREE_ECCENTRICITY_SIGMA: f64 = 0.005;

/// The range of a resonant moon's forced eccentricity, log-uniform: 0.004–0.04 (P14.T17.a). Io's
/// is 0.0041 and Europa's 0.0094.
pub const FORCED_ECCENTRICITY: (f64, f64) = (0.004, 0.04);

/// The scale of the Rayleigh law of a regular moon's inclination to its planet's equator: 0.5°
/// (P14.T17.a).
pub const INCLINATION_SIGMA: Degrees = Degrees::new(0.5);

/// The share of its parent's Hill radius at pericentre inside which every regular moon lies: a
/// twentieth (P14.T17.a). Callisto is at 0.035 of Jupiter's.
pub const HILL_FRACTION: f64 = 0.05;

/// The temperature of the circumplanetary ice line: 170 K (P14.T17.b), the same condensation
/// temperature as the star's snow line of P14.T3.b (Hayashi 1981, via Kennedy and Kenyon 2008).
pub const ICE_LINE_TEMPERATURE: Kelvin = Kelvin::new(170.0);

/// The tidal response k₂ ÷ Q of a regular moon, for its tidal heating: 0.015 (P14.T17.b).
///
/// Io's, measured from its astrometry: k₂ ÷ Q = 0.015 ± 0.003 (Lainey et al. 2009, Nature 459,
/// 957, abstract), for every moon (ruling 83.1). P14.T14.b's rocky body, k₂ = 0.3 and Q = 100, is
/// 0.003, which would give Io 0.45 W m⁻² against its observed 1.4–3.8; this value gives it 2.2.
/// Provisional for icy moons, whose own value is unmeasured.
pub const MOON_TIDAL_RESPONSE: f64 = 0.015;

/// The thermal conductivity of water ice times temperature: 651 W m⁻¹ (k = 651 ÷ T; Petrenko
/// and Whitworth 1999, as Hussmann et al. 2006, Icarus 185, 258 use it), so that a conductive ice
/// shell of thickness D between a surface at `T_s` and its melting point carries 651 ln(`T_m` ÷
/// `T_s`) ÷ D.
pub const ICE_CONDUCTIVITY_COEFFICIENT: f64 = 651.0;

/// The melting point at the base of an ice shell: 273 K.
pub const ICE_MELTING_POINT: Kelvin = Kelvin::new(273.0);

/// The density at which a moon's water fraction is spread into a layer: 1,000 kg m⁻³, liquid
/// water's (ice is 920).
pub const WATER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_000.0);

/// A regular moon's moment of inertia, in M R²: 0.35 (P14.T14.b's 0.33–0.4). The Galilean moons'
/// are 0.311–0.378 (Schubert et al. 2004, in "Jupiter", table 13.1).
pub const MOON_MOMENT_OF_INERTIA: f64 = 0.35;

/// A moon's primordial rotation period before tides despin it: 15 h, P14.T14.a's median for a
/// rocky body.
pub const MOON_PRIMORDIAL_PERIOD_HOURS: f64 = 15.0;

/// The mass fraction of water ice of a moon formed beyond its planet's ice line: 0.35–0.50,
/// uniform in its radius rank (ruling 83.7, as amended).
///
/// The sourced part is 0.35–0.41: Fortney, Marley and Barnes (2007, ApJ 659, 1661) give Titan
/// about 35% ices, and inverting their Eq. 7 gives 0.37–0.41 for Callisto, Ganymede and Titan.
/// The upper end, 0.41–0.50, is unsourced.
pub const ICY_MOON_ICE_FRACTION: (f64, f64) = (0.35, 0.5);

/// The least mass at which [`moon_radius`] reads Fortney et al.'s (2007) Eq. 7: 0.01 M⊕, the
/// bottom of the range it is fitted over (ruling 83.7).
pub const FORTNEY_MIN_MASS: EarthMasses = EarthMasses::new(0.01);

/// The uncompressed density of a small moon's rock: 3,300 kg m⁻³, a silicate mantle's (the Moon's
/// 3,344; ruling 83.7).
pub const MOON_ROCK_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(3_300.0);

/// The uncompressed density of a small moon's ice: 940 kg m⁻³, water ice Ih near 100 K
/// (Petrenko and Whitworth 1999; ruling 83.7).
pub const MOON_ICE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(940.0);

/// The radius of a moon of mass `mass` with a mass fraction `ice` of water ice over rock
/// (P14.T17.b, ruling 83.7).
///
/// From [`FORTNEY_MIN_MASS`] up, Fortney, Marley and Barnes's (2007, ApJ 659, 1661) Eq. 7 for
/// ice–rock bodies, R ÷ R⊕ = (0.0912 f + 0.1603) (log M)² + (0.3330 f + 0.7387) log M +
/// (0.4639 f + 1.1193), with M in M⊕ and f the ice fraction; below it an uncompressed sphere of
/// rock at [`MOON_ROCK_DENSITY`] and ice at [`MOON_ICE_DENSITY`], so that Zeng et al.'s curves
/// are never extrapolated below their range. The two meet 10% apart at 0.01 M⊕ for dry rock
/// (Eq. 7's 1,804 km against the sphere's 1,629), a step in mass only, since a moon's mass never
/// changes.
///
/// # Examples
///
/// Ganymede's mass at 0.4 ice is close to Ganymede:
///
/// ```
/// use hyperion_sim::planetary::moons::regular::moon_radius;
/// use hyperion_sim::units::EarthMasses;
///
/// let radius_km = moon_radius(EarthMasses::new(0.0248), 0.4).value() / 1e3;
/// assert!((radius_km / 2_631.0 - 1.0).abs() < 0.02);
/// ```
#[must_use]
pub fn moon_radius(mass: EarthMasses, ice: f64) -> Metres {
    let m = mass.value();
    if m >= FORTNEY_MIN_MASS.value() {
        let x = math::log10(m);
        let r =
            (0.0912 * ice + 0.1603) * x * x + (0.3330 * ice + 0.7387) * x + (0.4639 * ice + 1.1193);
        Metres::from(crate::units::EarthRadii::new(r))
    } else {
        let per_kg = ice / MOON_ICE_DENSITY.value() + (1.0 - ice) / MOON_ROCK_DENSITY.value();
        let volume = Kilograms::from(mass).value() * per_kg;
        Metres::new(math::cbrt(3.0 * volume / (4.0 * core::f64::consts::PI)))
    }
}

/// Whether `parent` has regular moons: a planet of [`REGULAR_MOON_MIN_PARENT_MASS`] or more with a
/// hydrogen envelope, an ice giant or a gas giant (P14.T17.a).
#[must_use]
pub fn hosts_regular_moons(parent: &MoonParent) -> bool {
    let enveloped = match parent.class() {
        PlanetClass::IceGiant | PlanetClass::GasGiant => true,
        PlanetClass::Rocky | PlanetClass::Icy | PlanetClass::SubNeptune => false,
    };
    enveloped && parent.mass() >= REGULAR_MOON_MIN_PARENT_MASS
}

/// Whether a regular moon sits at the 2:1 resonance with its inner neighbour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Resonance {
    /// Not resonant with its inner neighbour (or the innermost moon).
    None,
    /// At exactly twice its inner neighbour's period.
    TwoToOneWithInner,
}

/// One regular moon, primordial: its ordinal, mass and orbit about its planet, referred to the
/// planet's equator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegularMoon {
    ordinal: u8,
    mass: EarthMasses,
    orbit: KeplerElements,
    resonance: Resonance,
}

impl RegularMoon {
    /// The moon's place among its planet's regular moons, from 1 inside out, by which it draws.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    /// The moon's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The moon's orbit about its planet, referred to the planet's equator, with μ = G (`M_p` + m).
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// Whether it sits at the 2:1 resonance with its inner neighbour.
    #[must_use]
    pub const fn resonance(&self) -> Resonance {
        self.resonance
    }
}

/// A planet's regular satellite system, primordial ([`regular_moons`]).
#[derive(Debug, Clone, PartialEq)]
pub struct RegularMoons {
    drawn_mass: EarthMasses,
    moons: Vec<RegularMoon>,
    moonlets: u32,
}

impl RegularMoons {
    /// No regular moons: the system of a parent without an envelope or under 10 M⊕.
    pub const NONE: Self = Self {
        drawn_mass: EarthMasses::ZERO,
        moons: Vec::new(),
        moonlets: 0,
    };

    /// The total satellite mass drawn, `M_p` × 10^N(−3.8, 0.3), before any moon was dropped.
    #[must_use]
    pub const fn drawn_mass(&self) -> EarthMasses {
        self.drawn_mass
    }

    /// The major moons kept, inside out.
    #[must_use]
    pub fn moons(&self) -> &[RegularMoon] {
        &self.moons
    }

    /// The mass of the major moons kept.
    #[must_use]
    pub fn total_mass(&self) -> EarthMasses {
        self.moons
            .iter()
            .fold(EarthMasses::ZERO, |sum, moon| sum + moon.mass)
    }

    /// The count of small inner moonlets, a count only; zero when no major moon is kept.
    #[must_use]
    pub const fn moonlets(&self) -> u32 {
        self.moonlets
    }

    /// The same system with only the moons `keep` accepts: what a large retrograde capture leaves
    /// (P14.T19), never a moon's ordinal changed.
    #[must_use]
    pub(crate) fn retaining(&self, keep: impl Fn(&RegularMoon) -> bool) -> Self {
        Self {
            drawn_mass: self.drawn_mass,
            moons: self
                .moons
                .iter()
                .filter(|moon| keep(moon))
                .copied()
                .collect(),
            moonlets: self.moonlets,
        }
    }
}

/// The first word of moon k's block of [`tags::MOON_ORBIT`], k from 1.
const ORBIT_WORDS_START: u64 = 16;

/// The words of one moon's block of [`tags::MOON_ORBIT`].
const ORBIT_WORDS_PER_MOON: u64 = 64;

/// The offsets of moon k's single-word draws in its [`tags::MOON_ORBIT`] block, after the
/// seventeen spacing attempts of two words each.
const RESONANCE_WORD: u64 = 34;
const FREE_ECCENTRICITY_WORD: u64 = 35;
const FORCED_ECCENTRICITY_WORD: u64 = 36;
const INCLINATION_WORD: u64 = 37;
const ANGLE_WORDS: u64 = 38;

/// The most times the outer moon's eccentricity is halved to clear the gap of the floor, after
/// which it is zero.
const ECCENTRICITY_HALVINGS: u8 = 8;

/// One moon's own draws.
#[derive(Debug, Clone, Copy)]
struct MoonDraws {
    eccentricity: f64,
    resonant_with_inner: bool,
    inclination: Radians,
    angles: [Radians; 3],
}

/// The regular satellite system of `parent` in the universe of `seed` (P14.T17.a; see the
/// [module](self) documentation).
///
/// A parent that does not host regular moons ([`hosts_regular_moons`]) has
/// [`RegularMoons::NONE`] and draws nothing. The result depends on `seed`, the parent's ID and the
/// parent alone, and the same inputs give the same bits.
///
/// # Examples
///
/// Jupiter's mass and orbit give a system of up to six moons, all inside a twentieth of its Hill
/// radius:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{CellSize, GenCell};
/// use hyperion_sim::id::{Layer, SystemId};
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::derive::PlanetClass;
/// use hyperion_sim::planetary::moons::{MoonParent, MoonParentParts, ParentKind, regular_moons};
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// use hyperion_sim::units::{EarthMasses, GravitationalParameter, Kilograms, Metres, Radians, SolarMasses};
///
/// let system = SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, [1, 2, 0])?, 3)?;
/// let id = BodyIndex::new(BodySlot::Planet(5), BodySub::Primary)?.body_id(system);
/// let orbit = KeplerElements::from_semi_major_axis(
///     Metres::new(5.2 * METRES_PER_AU),
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///     Eccentricity::new(0.048)?,
///     Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?,
///     Radians::ZERO,
/// )?;
/// let jupiter = MoonParent::new(MoonParentParts {
///     id,
///     kind: ParentKind::Planet,
///     mass: EarthMasses::new(317.8),
///     radius: Metres::new(6.99e7),
///     class: PlanetClass::GasGiant,
///     orbit,
///     host_mass: Kilograms::new(SOLAR_MASS_KG),
///     maximum_moon_mass: EarthMasses::new(1e3),
/// })?;
/// let system = regular_moons(Seed::new(7), &jupiter);
/// assert!((1..=6).contains(&system.moons().len()));
/// let bound = 0.05 * jupiter.hill_radius_at_pericentre().value();
/// assert!(system.moons().iter().all(|moon| moon.orbit().apoapsis().value() < bound));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn regular_moons(seed: Seed, parent: &MoonParent) -> RegularMoons {
    if !hosts_regular_moons(parent) {
        return RegularMoons::NONE;
    }
    let key = ObjectKey::from(parent.id());
    let mut count = Stream::open(seed, tags::MOON_COUNT, key);
    let z_total = count.standard_normal();
    let drawn_mass = parent.mass()
        * math::exp10(SATELLITE_MASS_RATIO_MEDIAN_DEX + SATELLITE_MASS_RATIO_SIGMA_DEX * z_total);
    let n = zero_truncated_poisson(MAJOR_MOON_RATE, MAJOR_MOON_MAX, draw_rank(&mut count));
    count.seek(3);
    let moonlets = count.poisson(MOONLET_MEAN);

    let (masses, _) = moon_masses(seed, parent, drawn_mass, n);
    let mut orbits = Stream::open(seed, tags::MOON_ORBIT, key);
    let innermost = draw_rank(&mut orbits);
    let draws: Vec<MoonDraws> = (1..=n).map(|k| moon_draws(&orbits, k)).collect();
    let eccentricities: Vec<f64> = (0..draws.len())
        .map(|i| {
            let forced = draws[i].resonant_with_inner
                || draws.get(i + 1).is_some_and(|d| d.resonant_with_inner);
            let free = draws[i].eccentricity;
            if forced {
                let rank = orbit_rank(&orbits, moon_ordinal(i), FORCED_ECCENTRICITY_WORD);
                free.max(log_uniform(
                    FORCED_ECCENTRICITY.0,
                    FORCED_ECCENTRICITY.1,
                    rank,
                ))
            } else {
                free
            }
        })
        .collect();

    let placed = place(seed, parent, &masses, &eccentricities, &draws, innermost);
    let moons = keep_inside(parent, placed);
    let moonlets = if moons.is_empty() {
        0
    } else {
        u32::try_from(moonlets).unwrap_or(u32::MAX)
    };
    RegularMoons {
        drawn_mass,
        moons,
        moonlets,
    }
}

/// The ordinal, from 1, of the moon at index `i` of a system.
fn moon_ordinal(i: usize) -> u8 {
    u8::try_from(i + 1).expect("a system has at most six major moons")
}

/// The rank at `offset` of moon `ordinal`'s block of `stream`, a [`tags::MOON_ORBIT`] stream.
fn orbit_rank(stream: &Stream, ordinal: u8, offset: u64) -> UnitUniform {
    let mut at = stream.clone();
    at.seek(orbit_block(ordinal) + offset);
    draw_rank(&mut at)
}

/// The first word of moon `ordinal`'s block of [`tags::MOON_ORBIT`].
fn orbit_block(ordinal: u8) -> u64 {
    ORBIT_WORDS_START + ORBIT_WORDS_PER_MOON * (u64::from(ordinal) - 1)
}

/// Moon `ordinal`'s own draws from `stream`, a [`tags::MOON_ORBIT`] stream.
fn moon_draws(stream: &Stream, ordinal: u8) -> MoonDraws {
    let rank = |offset| orbit_rank(stream, ordinal, offset);
    let angle = |offset| Radians::new(core::f64::consts::TAU * rank(offset).value());
    MoonDraws {
        eccentricity: rayleigh(FREE_ECCENTRICITY_SIGMA, rank(FREE_ECCENTRICITY_WORD)),
        resonant_with_inner: ordinal > 1 && rank(RESONANCE_WORD).value() < RESONANCE_PROBABILITY,
        inclination: Radians::new(rayleigh(
            Radians::from(INCLINATION_SIGMA).value(),
            rank(INCLINATION_WORD),
        )),
        angles: [
            angle(ANGLE_WORDS),
            angle(ANGLE_WORDS + 1),
            angle(ANGLE_WORDS + 2),
        ],
    }
}

/// The count at `rank` of a zero-truncated Poisson law of rate `lambda`, a draw above `max` held
/// at `max`, by inversion.
fn zero_truncated_poisson(lambda: f64, max: u8, rank: UnitUniform) -> u8 {
    let zero = math::exp(-lambda);
    let target = zero + rank.value() * (1.0 - zero);
    let (mut term, mut cumulative) = (zero, zero);
    for k in 1..max {
        term *= lambda / f64::from(k);
        cumulative += term;
        if cumulative >= target {
            return k;
        }
    }
    max
}

/// The masses of `n` moons sharing `total`, inside out: by the moons' correlated law, or in a
/// Titan-like system (words 4 and 5 of [`tags::MOON_COUNT`]) with the outermost at 0.90–0.98 of
/// it and the rest shared by the law; and whether the system is Titan-like.
fn moon_masses(
    seed: Seed,
    parent: &MoonParent,
    total: EarthMasses,
    n: u8,
) -> (Vec<EarthMasses>, bool) {
    let key = ObjectKey::from(parent.id());
    let mut count = Stream::open(seed, tags::MOON_COUNT, key);
    count.seek(4);
    let titan = n > 1 && draw_rank(&mut count).value() < TITAN_PROBABILITY;
    let dominant = if titan {
        let (lo, hi) = TITAN_SHARE;
        lo + draw_rank(&mut count).value() * (hi - lo)
    } else {
        0.0
    };
    let shared = if titan { n - 1 } else { n };
    let mut stream = Stream::open(seed, tags::MOON_MASS, key);
    let centre = (f64::from(shared) - 1.0) / 2.0;
    let weights: Vec<f64> = (0..shared)
        .map(|k| {
            stream.seek(2 * u64::from(k));
            let epsilon = stream.standard_normal();
            math::exp10(
                MOON_SCATTER_DEX * epsilon + MOON_OUTWARD_STEP_DEX * (f64::from(k) - centre),
            )
        })
        .collect();
    let sum: f64 = weights.iter().sum();
    let rest = total * (1.0 - dominant);
    let mut masses: Vec<EarthMasses> = weights.iter().map(|w| rest * (w / sum)).collect();
    if titan {
        masses.push(total * dominant);
    }
    (masses, titan)
}

/// A moon placed at an axis with its eccentricity, before the Hill and mass cut.
#[derive(Debug, Clone, Copy)]
struct Placed {
    mass: EarthMasses,
    a: Metres,
    e: f64,
    resonance: Resonance,
}

/// The walk outward from the innermost moon (P14.T17.a), stopping where there is no room for
/// another moon.
fn place(
    seed: Seed,
    parent: &MoonParent,
    masses: &[EarthMasses],
    eccentricities: &[f64],
    draws: &[MoonDraws],
    innermost: UnitUniform,
) -> Vec<(Placed, MoonDraws)> {
    let r = parent.radius().value();
    let roche = parent.roche_limit_fluid(PLACEMENT_DENSITY).value();
    let first_e = eccentricities.first().copied().unwrap_or(0.0);
    let lo = (INNERMOST_RADII.0 * r).max(roche / (1.0 - first_e) * (1.0 + 1e-9));
    let hi = (INNERMOST_RADII.1 * r).max(lo * INNERMOST_RADII.1 / INNERMOST_RADII.0);
    let host = SolarMasses::from(parent.mass());
    let mut spacing = Stream::open(seed, tags::MOON_ORBIT, ObjectKey::from(parent.id()));

    let mut placed: Vec<(Placed, MoonDraws)> = Vec::with_capacity(masses.len());
    for (i, (&mass, &e)) in masses.iter().zip(eccentricities).enumerate() {
        let Some(&(inner, _)) = placed.last() else {
            let a = Metres::new(lo + innermost.value() * (hi - lo));
            let first = Placed {
                mass,
                a,
                e,
                resonance: Resonance::None,
            };
            placed.push((first, draws[i]));
            continue;
        };
        let inner_n = Neighbour::new(inner.mass, inner.a, inner.e);
        let resonant = draws[i].resonant_with_inner.then(|| {
            let ratio = (parent.mu_with(mass) / parent.mu_with(inner.mass))
                * RESONANT_PERIOD_RATIO
                * RESONANT_PERIOD_RATIO;
            Metres::new(inner.a.value() * math::cbrt(ratio))
        });
        let snapped =
            resonant.filter(|&a| satisfies_floor(&inner_n, &Neighbour::new(mass, a, e), host));
        let (a, resonance) = if let Some(a) = snapped {
            (a, Resonance::TwoToOneWithInner)
        } else {
            let floor = spacing_floor(inner.mass, mass, inner.e, e);
            spacing.seek(orbit_block(moon_ordinal(i)));
            let delta = (0..=MAX_SPACING_REDRAWS)
                .map(|_| spacing.normal(MOON_SPACING_MEAN, PAIR_SPACING_SIGMA))
                .find(|&delta| delta >= floor)
                .unwrap_or(floor);
            let chi = mutual_hill_factor(inner.mass, mass, host);
            let Some(a) = next_semi_major_axis(inner.a, delta, chi) else {
                break;
            };
            (a, Resonance::None)
        };
        let mut e = e;
        let mut halvings = 0;
        while !satisfies_floor(&inner_n, &Neighbour::new(mass, a, e), host) {
            if halvings == ECCENTRICITY_HALVINGS {
                e = 0.0;
                break;
            }
            e *= 0.5;
            halvings += 1;
        }
        placed.push((
            Placed {
                mass,
                a,
                e,
                resonance,
            },
            draws[i],
        ));
    }
    placed
}

/// The longest inner run of `placed` that lies inside a twentieth of the parent's Hill radius at
/// pericentre and whose total mass tides let survive, as moons: what does not fit is dropped from
/// the outside in.
fn keep_inside(parent: &MoonParent, placed: Vec<(Placed, MoonDraws)>) -> Vec<RegularMoon> {
    let bound = HILL_FRACTION * parent.hill_radius_at_pericentre().value();
    let limit = parent.maximum_moon_mass().value();
    let mut total = 0.0;
    let mut moons = Vec::with_capacity(placed.len());
    for (i, (moon, draws)) in placed.into_iter().enumerate() {
        total += moon.mass.value();
        if moon.a.value() * (1.0 + moon.e) >= bound || total > limit {
            break;
        }
        let orbit = moon_orbit(
            parent,
            moon.mass,
            moon.a,
            moon.e,
            draws.inclination,
            draws.angles,
        );
        moons.push(RegularMoon {
            ordinal: moon_ordinal(i),
            mass: moon.mass,
            orbit,
            resonance: moon.resonance,
        });
    }
    moons
}

/// Where a planet's regular moons formed: its circumplanetary disc and ice line (P14.T17.b).
///
/// The ice line is where the planet's own early luminosity, the luminosity at which its last
/// generation of satellites formed, heats a blackbody grain to [`ICE_LINE_TEMPERATURE`]: d =
/// 2.7 au × √(`L_p` ÷ L☉), the star's snow line of P14.T3.b at the planet's luminosity. The early
/// luminosity is plan 13's giant cooling at the age at which the star's disc disperses: Canup and
/// Ward (2006) find that the satellites that survive are the last generation, formed as the inflow
/// from the dispersing disc wanes. Baraffe et al.'s (2003) 1 `M_J` planet is at log L ÷ L☉ =
/// −4.72 at 1 Myr, −5.52 at 5 Myr and −5.87 at 10 Myr, putting the line at about 25, 9.7 and 6.5
/// Jupiter radii. Below plan 13's 0.3 `M_J` the luminosity follows the cooling's own power law in
/// mass from 0.3 to 0.6 `M_J` at that age (provisional: no cooling model covers ice giants).
///
/// A planet that formed inside its star's snow line, where the star alone keeps grains above 170
/// K, has no ice line: every moon forms dry. Beyond it, the star's heat, under 170 K, is left out
/// (for Jupiter it would move the line out by 11%).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonNursery {
    early_luminosity: SolarLuminosities,
    disc: DiscProfile,
    ice: bool,
}

impl MoonNursery {
    /// The nursery of `parent`, of composition `composition`, formed at `formation_distance` from
    /// its star in the star's disc `star_disc`.
    ///
    /// # Errors
    ///
    /// [`DeriveMoonError::GiantCooling`] if plan 13's cooling refuses the planet's mass (over 13
    /// `M_J`); [`DeriveMoonError::Nursery`] if the circumplanetary disc could not be built.
    pub fn new(
        parent: &MoonParent,
        composition: &Composition,
        star_disc: &DiscProfile,
        formation_distance: Metres,
    ) -> Result<Self, DeriveMoonError> {
        let age = Years::from(star_disc.lifetime());
        let mass = JupiterMasses::from(parent.mass());
        let early_luminosity = if mass >= GIANT_MIN_MASS {
            giant_cooling(mass, age, composition)
                .map_err(DeriveMoonError::GiantCooling)?
                .luminosity()
        } else {
            let at = |m: f64| {
                giant_cooling(JupiterMasses::new(m), age, composition)
                    .map(|state| state.luminosity().value())
                    .map_err(DeriveMoonError::GiantCooling)
            };
            let (low, high) = (
                at(GIANT_MIN_MASS.value())?,
                at(2.0 * GIANT_MIN_MASS.value())?,
            );
            let slope = math::log2(high / low);
            SolarLuminosities::new(low * math::powf(mass / GIANT_MIN_MASS, slope))
        };
        let host = DiscHost::new(
            SolarMasses::from(parent.mass()),
            composition.fe_h(),
            early_luminosity,
            SolarRadii::from(parent.radius()),
        )
        .map_err(|_| DeriveMoonError::Nursery)?;
        let disc = disc::derive(
            &host,
            star_disc.lifetime(),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        );
        let disc = *disc.profile().ok_or(DeriveMoonError::Nursery)?;
        Ok(Self {
            early_luminosity,
            disc,
            ice: formation_distance >= star_disc.snow_line(),
        })
    }

    /// The planet's luminosity when its satellites formed.
    #[must_use]
    pub const fn early_luminosity(&self) -> SolarLuminosities {
        self.early_luminosity
    }

    /// The circumplanetary ice line, or `None` for a planet formed inside its star's snow line,
    /// about which no ice condensed.
    #[must_use]
    pub fn ice_line(&self) -> Option<Metres> {
        self.ice.then(|| self.disc.snow_line())
    }
}

/// What lights a moon at a time: its planet's own glow at the moon's distance, and the planet's
/// hosts at the planet's orbit (P14.T17.b, "the star as the source of light").
#[derive(Debug, Clone, PartialEq)]
pub struct MoonSky {
    planet: [HostLight; 1],
    stars: Vec<Illumination>,
    composition: Composition,
}

impl MoonSky {
    /// The sky of a moon of the planet `planet_now`, derived at the time, on `planet_orbit_now`
    /// about `planet_hosts`, also at the time.
    ///
    /// # Errors
    ///
    /// [`DeriveMoonError::Sky`] if the planet's light could not be built, which its derivation
    /// rules out.
    pub fn new(
        planet_now: &DerivedBody,
        planet_orbit_now: &KeplerElements,
        planet_hosts: &BodyHosts<'_>,
    ) -> Result<Self, DeriveMoonError> {
        let luminosity = SolarLuminosities::from(planet_now.internal_luminosity());
        let radius = Metres::from(planet_now.radius());
        let temperature = if luminosity.value() > 0.0 {
            let area = 4.0 * core::f64::consts::PI * radius.value() * radius.value();
            Kelvin::new(math::powf(
                planet_now.internal_luminosity().value() / (area * STEFAN_BOLTZMANN),
                0.25,
            ))
        } else {
            Kelvin::ZERO
        };
        let planet = HostLight::new(luminosity, temperature, SolarRadii::from(radius))
            .map_err(|_| DeriveMoonError::Sky)?;
        let a = planet_orbit_now.semi_major_axis();
        let e = planet_orbit_now.eccentricity().value();
        let mut stars: Vec<Illumination> = planet_hosts
            .orbited()
            .iter()
            .map(|host| Illumination::new(*host, a, e).ok_or(DeriveMoonError::Sky))
            .collect::<Result<_, _>>()?;
        stars.extend_from_slice(planet_hosts.companions());
        Ok(Self {
            planet: [planet],
            stars,
            composition: *planet_hosts.composition(),
        })
    }
}

/// A regular moon derived at a time (P14.T17.b): P14.T16's derivation with the planet as its
/// primary, and its tidal heat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DerivedMoon {
    body: DerivedBody,
    ice_fraction: f64,
    radius: Metres,
    density: KilogramsPerCubicMetre,
    tidal_heating: Watts,
    heat_flux: WattsPerSquareMetre,
    volcanism: Volcanism,
    subsurface_ocean: bool,
    locking_time: Seconds,
}

impl DerivedMoon {
    /// The flux the moon receives from its planet and the stars, from P14.T16's [`derive_body`].
    #[must_use]
    pub const fn flux(&self) -> crate::units::EarthFluxes {
        self.body.flux()
    }

    /// The moon's equilibrium temperature, from P14.T16's [`derive_body`].
    #[must_use]
    pub const fn equilibrium_temperature(&self) -> Kelvin {
        self.body.equilibrium_temperature()
    }

    /// The moon's own Hill radius about its planet, from P14.T16's [`derive_body`].
    #[must_use]
    pub const fn hill_radius(&self) -> Metres {
        self.body.hill_radius()
    }

    /// The moon's mass fraction of water ice: 0.35–0.50 beyond the circumplanetary ice line, by its
    /// radius rank, and none inside it ([`ICY_MOON_ICE_FRACTION`], ruling 83.7).
    #[must_use]
    pub const fn ice_fraction(&self) -> f64 {
        self.ice_fraction
    }

    /// The moon's radius ([`moon_radius`], ruling 83.7).
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }

    /// The moon's mean density, from its mass and [`radius`](Self::radius).
    #[must_use]
    pub const fn density(&self) -> KilogramsPerCubicMetre {
        self.density
    }

    /// The moon's class: [`PlanetClass::Icy`] with ice, [`PlanetClass::Rocky`] without.
    #[must_use]
    pub const fn class(&self) -> PlanetClass {
        if self.ice_fraction > 0.0 {
            PlanetClass::Icy
        } else {
            PlanetClass::Rocky
        }
    }

    /// The tidal heat its planet raises in it, W ([`tidal_heating`]).
    #[must_use]
    pub const fn tidal_heating(&self) -> Watts {
        self.tidal_heating
    }

    /// Its tidal heat spread over its surface, W m⁻².
    #[must_use]
    pub const fn heat_flux(&self) -> WattsPerSquareMetre {
        self.heat_flux
    }

    /// Its volcanism level, from the heat flux.
    #[must_use]
    pub const fn volcanism(&self) -> Volcanism {
        self.volcanism
    }

    /// Whether it holds liquid water under its ice ([`has_subsurface_ocean`]).
    #[must_use]
    pub const fn subsurface_ocean(&self) -> bool {
        self.subsurface_ocean
    }

    /// How long its planet's tides take to lock its spin ([`locking_time`]).
    #[must_use]
    pub const fn locking_time(&self) -> Seconds {
        self.locking_time
    }
}

/// How volcanically active a moon is, from its tidal heat flux (P14.T17.b).
///
/// The levels are this lane's, where the plan names none: Earth's mean heat flow is 0.09 W m⁻²
/// (Davies and Davies 2010, Solid Earth 1, 5: 47 TW), Enceladus's about 0.02 with active plumes,
/// Io's 1.4–3.8. Provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Volcanism {
    /// Under 0.01 W m⁻²: no tidally driven activity (Callisto, Ganymede today).
    Dormant,
    /// 0.01–0.1 W m⁻²: local activity, plumes or cryovolcanism (Enceladus, Europa).
    Minor,
    /// 0.1–1 W m⁻²: widespread activity, above Earth's heat flow.
    Major,
    /// 1 W m⁻² and over: global resurfacing (Io).
    Extreme,
}

impl Volcanism {
    /// The level of a surface heat flux `flux`.
    #[must_use]
    pub fn of(flux: WattsPerSquareMetre) -> Self {
        let f = flux.value();
        if f >= 1.0 {
            Self::Extreme
        } else if f >= 0.1 {
            Self::Major
        } else if f >= 0.01 {
            Self::Minor
        } else {
            Self::Dormant
        }
    }
}

/// The tidal heat a planet of mass `planet_mass` raises in a synchronous moon of radius `radius`
/// and tidal response `k2_over_q` on an orbit of semi-major axis `a`, eccentricity `e` and mean
/// motion `n` (rad s⁻¹): H = (21 ÷ 2) (k₂ ÷ Q) G `M_p`² R⁵ n e² ÷ a⁶ (P14.T17.b).
///
/// The eccentricity tide of a synchronous satellite, as the plan cites it after Peale, Cassen and
/// Reynolds (1979, Science 203, 892), in the form of Segatz et al. (1988, Icarus 75, 187), who
/// write it with k₂ ÷ Q. For Io, at Lainey et al.'s (2009) k₂ ÷ Q = 0.015, it gives 9.3 × 10¹³ W,
/// 2.2 W m⁻², against the observed 0.6–1.6 × 10¹⁴ W.
///
/// # Examples
///
/// Io:
///
/// ```
/// use hyperion_sim::planetary::moons::regular::tidal_heating;
/// use hyperion_sim::units::{Kilograms, Metres};
///
/// let n = core::f64::consts::TAU / (1.769_138 * 86_400.0);
/// let heat = tidal_heating(
///     Kilograms::new(1.898_19e27),
///     Metres::new(1.8216e6),
///     0.015,
///     Metres::new(4.218e8),
///     0.0041,
///     n,
/// );
/// assert!((heat.value() / 9.3e13 - 1.0).abs() < 0.05);
/// ```
#[must_use]
pub fn tidal_heating(
    planet_mass: Kilograms,
    radius: Metres,
    k2_over_q: f64,
    a: Metres,
    e: f64,
    mean_motion: f64,
) -> Watts {
    let (r, a) = (radius.value(), a.value());
    let m = planet_mass.value();
    Watts::new(
        21.0 / 2.0
            * k2_over_q
            * GRAVITATIONAL_CONSTANT
            * m
            * m
            * math::powi(r, 5)
            * mean_motion
            * e
            * e
            / math::powi(a, 6),
    )
}

/// Whether a moon of radius `radius`, density `density` and water mass fraction `water`, with a
/// surface at `surface` and a heat flux `flux` from below, keeps liquid water under its ice
/// (P14.T17.b).
///
/// An ice shell conducting `flux` from its melting point to the surface is D = 651 ln(273 K ÷
/// `T_s`) ÷ F thick ([`ICE_CONDUCTIVITY_COEFFICIENT`]). The moon's water, spread at
/// [`WATER_DENSITY`] over its outer layer, is R (1 − (1 − x ρ ÷ `ρ_w`)^⅓) deep. The ocean exists
/// when the shell the heat allows is thinner than the water: the flux exceeds what keeps a water
/// layer liquid under the ice. A surface at or above the melting point has no subsurface ocean,
/// and neither has a moon without water.
///
/// # Examples
///
/// Europa, about 8% water under a surface near 100 K, keeps its ocean with a flux of 0.05 W m⁻²:
///
/// ```
/// use hyperion_sim::planetary::moons::regular::has_subsurface_ocean;
/// use hyperion_sim::units::{Kelvin, KilogramsPerCubicMetre, Metres, WattsPerSquareMetre};
///
/// let europa = |flux| has_subsurface_ocean(
///     Metres::new(1.5608e6),
///     KilogramsPerCubicMetre::new(3_013.0),
///     0.08,
///     Kelvin::new(102.0),
///     WattsPerSquareMetre::new(flux),
/// );
/// assert!(europa(0.05));
/// assert!(!europa(1e-4));
/// ```
#[must_use]
pub fn has_subsurface_ocean(
    radius: Metres,
    density: KilogramsPerCubicMetre,
    water: f64,
    surface: Kelvin,
    flux: WattsPerSquareMetre,
) -> bool {
    let (t_s, f) = (surface.value(), flux.value());
    if water <= 0.0 || f <= 0.0 || t_s >= ICE_MELTING_POINT.value() {
        return false;
    }
    let volume_share = (water * density.value() / WATER_DENSITY.value()).min(1.0);
    let layer = radius.value() * (1.0 - math::cbrt(1.0 - volume_share));
    let shell =
        ICE_CONDUCTIVITY_COEFFICIENT * math::ln(ICE_MELTING_POINT.value() / t_s.max(1.0)) / f;
    shell < layer
}

/// The time a planet of mass `planet_mass` takes to lock the spin of a moon of mass `mass` and
/// radius `radius` at semi-major axis `a`, from a period of [`MOON_PRIMORDIAL_PERIOD_HOURS`]:
/// τ = ω a⁶ I Q ÷ (3 G `M_p`² k₂ R⁵), with I = 0.35 M R² and a rocky body's k₂ = 0.3 and Q = 100
/// (P14.T14.b, after Gladman et al. 1996, Icarus 122, 166).
///
/// P14.T14.b's locking time, which T14 will own; here for P14.T17.b's "all regular moons lock".
#[must_use]
pub fn locking_time(planet_mass: Kilograms, mass: Kilograms, radius: Metres, a: Metres) -> Seconds {
    let omega = core::f64::consts::TAU / (MOON_PRIMORDIAL_PERIOD_HOURS * 3_600.0);
    let (r, a, m_p) = (radius.value(), a.value(), planet_mass.value());
    let inertia = MOON_MOMENT_OF_INERTIA * mass.value() * r * r;
    Seconds::new(
        omega * math::powi(a, 6) * inertia * ROCKY_TIDAL_Q
            / (3.0 * GRAVITATIONAL_CONSTANT * m_p * m_p * ROCKY_LOVE_NUMBER * math::powi(r, 5)),
    )
}

/// The regular moon `moon` of `parent`, formed in `nursery` and lit by `sky`, derived at `t` in a
/// system aged `age` at the epoch, with the radius rank `radius_rank` its own `planet.radius`
/// stream gives it (P14.T17.b; see the [module](self) documentation).
///
/// The moon is a [`PlacedBody`] on its orbit about the planet, formed at its own distance from the
/// planet in the circumplanetary disc, or, about a planet with no ice line, at a point inside the
/// disc's line; [`derive_body`] reads it with the planet's mass as its primary, the planet's glow
/// as the host it orbits, and the stars at the planet's orbit as the companions that light it.
///
/// # Errors
///
/// [`DeriveMoonError::Body`] where [`derive_body`] refuses (a system not yet born at `t`), and
/// [`DeriveMoonError::Placed`] or [`DeriveMoonError::Hosts`] for inputs it cannot take.
///
/// # Examples
///
/// Jupiter's moons, derived about Jupiter as P14.T16 derives it at the epoch about the present
/// Sun, in the zero-age Sun's disc:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{CellSize, GenCell};
/// use hyperion_sim::id::{Layer, SystemId};
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::moons::regular::{MoonNursery, MoonSky, derive_regular_moon};
/// use hyperion_sim::planetary::moons::{MoonParent, ParentKind, regular_moons};
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// use hyperion_sim::units::{
///     Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres, Radians,
///     SolarLuminosities, SolarMasses, SolarRadii, Years,
/// };
///
/// let zams = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
/// let disc = disc::derive(&zams, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// let disc = disc.profile().expect("a median disc exists");
/// let a = Metres::new(5.2 * METRES_PER_AU);
/// let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
/// let flat = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
/// let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.048)?, flat, Radians::ZERO)?;
/// let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
/// let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
/// let age = Years::new(4.57e9);
/// let placed = PlacedBody::new(EarthMasses::new(317.8), orbit, a, UnitUniform::HALF)?;
/// let jupiter = derive_body(&placed, &hosts, disc, age, UniverseTime::EPOCH)?;
///
/// let system = SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, [1, 2, 0])?, 3)?;
/// let id = BodyIndex::new(BodySlot::Planet(5), BodySub::Primary)?.body_id(system);
/// let host = Kilograms::new(SOLAR_MASS_KG);
/// let parent = MoonParent::from_derived(id, ParentKind::Planet, &jupiter, orbit, host)?;
/// let nursery = MoonNursery::new(&parent, &Composition::SOLAR, disc, a)?;
/// let sky = MoonSky::new(&jupiter, &orbit, &hosts)?;
/// for moon in regular_moons(Seed::new(7), &parent).moons() {
///     let rank = UnitUniform::HALF;
///     let derived = derive_regular_moon(moon, &parent, &nursery, &sky, rank, age, UniverseTime::EPOCH)?;
///     // Far from the Sun and locked to Jupiter.
///     assert!(derived.equilibrium_temperature().value() < 150.0);
///     assert!(derived.locking_time().value() < 1e16);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn derive_regular_moon(
    moon: &RegularMoon,
    parent: &MoonParent,
    nursery: &MoonNursery,
    sky: &MoonSky,
    radius_rank: UnitUniform,
    age: Years,
    t: UniverseTime,
) -> Result<DerivedMoon, DeriveMoonError> {
    let a = moon.orbit.semi_major_axis();
    let formation = match nursery.ice_line() {
        Some(_) => a,
        None => Metres::new(a.value().min(0.5 * nursery.disc.snow_line().value())),
    };
    let placed = PlacedBody::new(moon.mass, moon.orbit, formation, radius_rank)
        .map_err(DeriveMoonError::Placed)?;
    let hosts = BodyHosts::new(
        Kilograms::from(parent.mass()),
        sky.composition,
        &sky.planet,
        &sky.stars,
    )
    .map_err(DeriveMoonError::Hosts)?;
    let body =
        derive_body(&placed, &hosts, &nursery.disc, age, t).map_err(DeriveMoonError::Body)?;
    let icy = nursery.ice_line().is_some_and(|line| a >= line);
    let ice_fraction = if icy {
        let (lo, hi) = ICY_MOON_ICE_FRACTION;
        lo + radius_rank.value() * (hi - lo)
    } else {
        0.0
    };
    let radius = moon_radius(moon.mass, ice_fraction);
    let volume = 4.0 / 3.0 * core::f64::consts::PI * math::powi(radius.value(), 3);
    let density = KilogramsPerCubicMetre::new(Kilograms::from(moon.mass).value() / volume);
    let e = moon.orbit.eccentricity().value();
    let n = core::f64::consts::TAU / moon.orbit.period().value();
    let heat = tidal_heating(
        Kilograms::from(parent.mass()),
        radius,
        MOON_TIDAL_RESPONSE,
        a,
        e,
        n,
    );
    let area = 4.0 * core::f64::consts::PI * radius.value() * radius.value();
    let flux = WattsPerSquareMetre::new(heat.value() / area);
    let ocean = has_subsurface_ocean(
        radius,
        density,
        ice_fraction,
        body.equilibrium_temperature(),
        flux,
    );
    Ok(DerivedMoon {
        body,
        ice_fraction,
        radius,
        density,
        tidal_heating: heat,
        heat_flux: flux,
        volcanism: Volcanism::of(flux),
        subsurface_ocean: ocean,
        locking_time: locking_time(
            Kilograms::from(parent.mass()),
            Kilograms::from(moon.mass),
            radius,
            a,
        ),
    })
}

/// A moon could not be derived.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveMoonError {
    /// Plan 13's giant cooling refused the planet (above 13 `M_J`).
    GiantCooling(crate::stellar::substellar::EvaluateGiantCoolingError),
    /// The circumplanetary disc could not be built.
    Nursery,
    /// The planet's light could not be built.
    Sky,
    /// The moon could not be placed for the derivation.
    Placed(BuildPlacedBodyError),
    /// The moon's hosts could not be built.
    Hosts(BuildBodyHostsError),
    /// P14.T16's derivation refused the moon.
    Body(DeriveBodyError),
}

impl fmt::Display for DeriveMoonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::GiantCooling(_) => "the planet's early luminosity could not be evaluated",
            Self::Nursery => "the circumplanetary disc could not be built",
            Self::Sky => "the planet's light could not be built",
            Self::Placed(_) => "the moon could not be placed",
            Self::Hosts(_) => "the moon's hosts could not be built",
            Self::Body(_) => "the moon could not be derived",
        })
    }
}

impl Error for DeriveMoonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GiantCooling(e) => Some(e),
            Self::Placed(e) => Some(e),
            Self::Hosts(e) => Some(e),
            Self::Body(e) => Some(e),
            Self::Nursery | Self::Sky => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::derive::OrbitSense;
    use crate::planetary::derive::solar::{self, SOLAR_AGE};
    use crate::planetary::moons::testing::{jupiter, neptune, parent, planet_id, saturn};
    use crate::planetary::moons::{MoonParentParts, ParentKind};
    use crate::units::consts::SOLAR_MASS_KG;

    /// The Solar System's Jupiter or Saturn, derived at the epoch, as a parent.
    fn derived_parent(name: &str, id: crate::id::BodyId) -> (MoonParent, DerivedBody) {
        let planets = solar::solar_system();
        let derived = *solar::found(&planets, name);
        let &(_, _, _, a, e) = solar::PLANETS.iter().find(|p| p.0 == name).unwrap();
        let parent = MoonParent::from_derived(
            id,
            ParentKind::Planet,
            &derived,
            solar::orbit(a, e),
            Kilograms::new(SOLAR_MASS_KG),
        )
        .unwrap();
        (parent, derived)
    }

    /// The same parent as `p` under the ID of planet `slot` of system `index`.
    fn renamed(p: &MoonParent, index: u32, slot: u8) -> MoonParent {
        MoonParent::new(MoonParentParts {
            id: planet_id(index, slot),
            ..parts(p)
        })
        .unwrap()
    }

    /// Giants of many kinds, `n` of each: Jupiter, Saturn, Neptune, a 13 Jupiter-mass giant at 1
    /// au, a warm Jupiter at 0.2 au and an eccentric one.
    fn giants(n: u32) -> Vec<MoonParent> {
        let kinds = [
            jupiter(planet_id(0, 5)),
            saturn(planet_id(0, 6)),
            neptune(planet_id(0, 8)),
            parent(
                planet_id(0, 2),
                ParentKind::Planet,
                4_131.0,
                75_000.0,
                PlanetClass::GasGiant,
                1.0,
                0.05,
            ),
            parent(
                planet_id(0, 1),
                ParentKind::Planet,
                317.8,
                80_000.0,
                PlanetClass::GasGiant,
                0.2,
                0.0,
            ),
            parent(
                planet_id(0, 3),
                ParentKind::Planet,
                317.8,
                69_911.0,
                PlanetClass::GasGiant,
                3.0,
                0.5,
            ),
        ];
        (0..n)
            .flat_map(|i| {
                kinds
                    .iter()
                    .map(move |p| renamed(p, i, p.id().body_index().to_be_bytes()[0]))
            })
            .collect()
    }

    #[test]
    fn jupiter_s_and_saturn_s_masses_give_satellite_totals_within_a_factor_of_three_in_the_median()
    {
        for (name, slot, real) in [("Jupiter", 5, 2.07e-4), ("Saturn", 6, 2.47e-4)] {
            let (p, _) = derived_parent(name, planet_id(0, slot));
            let mut totals: Vec<f64> = (0..2_001)
                .map(|i| {
                    regular_moons(Seed::new(1), &renamed(&p, i, slot))
                        .total_mass()
                        .value()
                })
                .collect();
            totals.sort_by(f64::total_cmp);
            let ratio = totals[1_000] / (real * p.mass().value());
            assert!((1.0 / 3.0..3.0).contains(&ratio), "{name}: {ratio}");
        }
    }

    #[test]
    fn every_moon_lies_inside_a_twentieth_of_the_hill_radius_and_outside_the_fluid_roche_limit() {
        let mut moons = 0;
        for p in giants(300) {
            let system = regular_moons(Seed::new(3), &p);
            let bound = HILL_FRACTION * p.hill_radius_at_pericentre().value();
            let roche = p.roche_limit_fluid(PLACEMENT_DENSITY);
            for moon in system.moons() {
                assert!(moon.orbit().apoapsis().value() < bound, "{moon:?}");
                assert!(moon.orbit().periapsis() > roche);
                assert!(moon.orbit().periapsis() > p.radius());
                let limit =
                    p.stability_limit(moon.orbit().eccentricity().value(), OrbitSense::Prograde);
                assert!(moon.orbit().apoapsis() < limit);
                moons += 1;
            }
            assert!(system.total_mass() <= p.maximum_moon_mass());
        }
        assert!(moons > 3_000, "{moons}");
    }

    #[test]
    fn neighbouring_moons_satisfy_the_floor_about_their_planet_and_resonant_ones_sit_at_two_to_one()
    {
        let (mut resonant, mut pairs) = (0u32, 0u32);
        for p in giants(300) {
            let host = SolarMasses::from(p.mass());
            let system = regular_moons(Seed::new(4), &p);
            for pair in system.moons().windows(2) {
                let neighbour = |m: &RegularMoon| {
                    Neighbour::new(
                        m.mass(),
                        m.orbit().semi_major_axis(),
                        m.orbit().eccentricity().value(),
                    )
                };
                assert!(satisfies_floor(
                    &neighbour(&pair[0]),
                    &neighbour(&pair[1]),
                    host
                ));
                assert_eq!(pair[1].ordinal(), pair[0].ordinal() + 1);
                pairs += 1;
                if pair[1].resonance() == Resonance::TwoToOneWithInner {
                    resonant += 1;
                    let ratio = pair[1].orbit().period().value() / pair[0].orbit().period().value();
                    assert!((ratio - 2.0).abs() < 1e-12, "{ratio}");
                    for moon in pair {
                        let e = moon.orbit().eccentricity().value();
                        assert!(e >= FORCED_ECCENTRICITY.0 / 256.0 || e <= 0.0);
                    }
                }
            }
            if let Some(first) = system.moons().first() {
                assert_eq!(first.resonance(), Resonance::None);
                let radii = first.orbit().semi_major_axis() / p.radius();
                assert!(
                    (3.0..8.0 * 1.000_001).contains(&radii)
                        || first.orbit().semi_major_axis() > p.roche_limit_fluid(PLACEMENT_DENSITY)
                );
            }
        }
        let share = f64::from(resonant) / f64::from(pairs);
        assert!((0.3..0.55).contains(&share), "{resonant} of {pairs}");
    }

    #[test]
    fn the_count_of_major_moons_is_a_zero_truncated_poisson_law_of_mean_three_and_a_half() {
        let n = 100_000;
        let mut sum = 0u32;
        for i in 0..n {
            let rank = UnitUniform::new((f64::from(i) + 0.5) / f64::from(n)).unwrap();
            let k = zero_truncated_poisson(MAJOR_MOON_RATE, MAJOR_MOON_MAX, rank);
            assert!((1..=MAJOR_MOON_MAX).contains(&k));
            sum += u32::from(k);
        }
        let mean = f64::from(sum) / f64::from(n);
        assert!((mean - 3.5).abs() < 0.01, "{mean}");
    }

    #[test]
    fn the_moons_split_their_mass_by_their_own_law() {
        let jupiter = jupiter(planet_id(0, 5));
        let total = EarthMasses::new(0.05);
        let (mut outer_larger, mut pairs) = (0u32, 0u32);
        let (mut spreads, mut largest) = (Vec::new(), Vec::new());
        let (mut titans, mut systems) = (0u32, 0u32);
        for i in 0..15_000u32 {
            let p = renamed(&jupiter, i, 5);
            let n = 4 + u8::try_from(i % 3).unwrap();
            let (masses, titan) = moon_masses(Seed::new(8), &p, total, n);
            assert_eq!(masses.len(), usize::from(n));
            let sum: f64 = masses.iter().map(|m| m.value()).sum();
            assert!((sum / total.value() - 1.0).abs() < 1e-12);
            systems += 1;
            if titan {
                titans += 1;
                let share = masses.last().unwrap().value() / sum;
                assert!((0.90..=0.98).contains(&share), "{share}");
                continue;
            }
            for pair in masses.windows(2) {
                pairs += 1;
                outer_larger += u32::from(pair[1] > pair[0]);
            }
            let logs: Vec<f64> = masses.iter().map(|m| math::log10(m.value())).collect();
            let mean = logs.iter().sum::<f64>() / f64::from(n);
            let var = logs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / f64::from(n - 1);
            spreads.push(var.sqrt());
            largest.push(masses.iter().map(|m| m.value()).fold(0.0, f64::max) / sum);
        }
        let median = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        assert!(
            spreads.len() >= 10_000,
            "{} main-branch systems",
            spreads.len()
        );
        let share = f64::from(outer_larger) / f64::from(pairs);
        assert!((0.65..0.75).contains(&share), "outer-heavier {share}");
        let spread = median(&mut spreads);
        assert!((0.45..0.70).contains(&spread), "spread {spread} dex");
        let big = median(&mut largest);
        assert!((0.45..0.65).contains(&big), "largest share {big}");
        let titan_share = f64::from(titans) / f64::from(systems);
        assert!(
            (titan_share - TITAN_PROBABILITY).abs() < 0.02,
            "{titan_share}"
        );
    }

    #[test]
    fn a_generated_system_s_totals_are_inside_the_drawn_total() {
        for i in 0..2_000 {
            let p = renamed(&jupiter(planet_id(0, 5)), i, 5);
            let system = regular_moons(Seed::new(8), &p);
            assert!(system.total_mass() / system.drawn_mass() <= 1.0 + 1e-12);
            let ratio = system.drawn_mass() / p.mass();
            assert!((1e-6..1e-1).contains(&ratio));
        }
    }

    #[test]
    fn a_hot_jupiter_and_planets_without_an_envelope_keep_no_regular_moons() {
        let hot = parent(
            planet_id(0, 1),
            ParentKind::Planet,
            317.8,
            90_000.0,
            PlanetClass::GasGiant,
            0.05,
            0.0,
        );
        let rocky = parent(
            planet_id(0, 2),
            ParentKind::Planet,
            12.0,
            14_000.0,
            PlanetClass::Rocky,
            1.0,
            0.0,
        );
        let small = parent(
            planet_id(0, 3),
            ParentKind::Planet,
            8.0,
            18_000.0,
            PlanetClass::SubNeptune,
            1.0,
            0.0,
        );
        let light = parent(
            planet_id(0, 4),
            ParentKind::Planet,
            9.9,
            20_000.0,
            PlanetClass::IceGiant,
            10.0,
            0.0,
        );
        for i in 0..500 {
            let system = regular_moons(Seed::new(9), &renamed(&hot, i, 1));
            assert!(system.moons().is_empty());
            assert_eq!(system.moonlets(), 0);
            for p in [&rocky, &small, &light] {
                assert_eq!(
                    regular_moons(Seed::new(9), &renamed(p, i, 2)),
                    RegularMoons::NONE
                );
            }
        }
    }

    #[test]
    fn two_calls_agree_bit_for_bit_and_pruning_keeps_the_inner_moons() {
        for p in giants(50) {
            let system = regular_moons(Seed::new(2), &p);
            assert_eq!(system, regular_moons(Seed::new(2), &p));
            if let Some(second) = system.moons().get(1) {
                let a = second.orbit().semi_major_axis();
                let pruned = system.retaining(|moon| moon.orbit().semi_major_axis() <= a);
                assert_eq!(pruned.moons(), &system.moons()[..2]);
                assert_same_bits(pruned.drawn_mass().value(), system.drawn_mass().value());
            }
        }
    }

    #[test]
    fn io_s_elements_give_a_heat_flux_of_one_to_four_watts_per_square_metre() {
        let radius = Metres::new(1.8216e6);
        let n = core::f64::consts::TAU / (1.769_138 * 86_400.0);
        let heat = tidal_heating(
            Kilograms::new(1.898_13e27),
            radius,
            MOON_TIDAL_RESPONSE,
            Metres::new(4.218e8),
            0.0041,
            n,
        );
        let flux = heat.value() / (4.0 * core::f64::consts::PI * radius.value() * radius.value());
        assert!((1.0..4.0).contains(&flux), "{flux} W/m²");
        assert_eq!(
            Volcanism::of(WattsPerSquareMetre::new(flux)),
            Volcanism::Extreme
        );
    }

    #[test]
    fn europa_s_elements_give_the_subsurface_ocean_flag() {
        let radius = Metres::new(1.5608e6);
        let n = core::f64::consts::TAU / (3.551_181 * 86_400.0);
        let heat = tidal_heating(
            Kilograms::new(1.898_13e27),
            radius,
            MOON_TIDAL_RESPONSE,
            Metres::new(6.711e8),
            0.0094,
            n,
        );
        let flux = WattsPerSquareMetre::new(
            heat.value() / (4.0 * core::f64::consts::PI * radius.value() * radius.value()),
        );
        let density = KilogramsPerCubicMetre::new(3_013.0);
        assert!(has_subsurface_ocean(
            radius,
            density,
            0.08,
            Kelvin::new(102.0),
            flux
        ));
        // Without water, or with a surface above the melting point, there is none.
        assert!(!has_subsurface_ocean(
            radius,
            density,
            0.0,
            Kelvin::new(102.0),
            flux
        ));
        assert!(!has_subsurface_ocean(
            radius,
            density,
            0.08,
            Kelvin::new(300.0),
            flux
        ));
    }

    #[test]
    fn ganymede_and_callisto_like_moons_have_icy_densities() {
        let density = |kg: f64, ice: f64| {
            let r = moon_radius(EarthMasses::new(kg / 5.972_2e24), ice).value();
            kg / (4.0 / 3.0 * core::f64::consts::PI * r * r * r) / 1e3
        };
        // Ruling 83.7 as amended: each moon's real density lies between the model's at the
        // range's two ends, for its own mass (JPL's masses and densities).
        let (lo, hi) = ICY_MOON_ICE_FRACTION;
        for (name, kg, real) in [
            ("Ganymede", 1.4819e23, 1.942),
            ("Callisto", 1.0759e23, 1.834),
            ("Titan", 1.3452e23, 1.881),
        ] {
            let (dense, light) = (density(kg, lo), density(kg, hi));
            assert!(
                light < real && real < dense,
                "{name}: {light}–{dense} against {real}"
            );
        }
        // Below 0.01 M⊕ the uncompressed sphere: Europa's mass as dry rock is Europa's size.
        let europa = moon_radius(EarthMasses::new(4.7998e22 / 5.972_2e24), 0.0).value();
        assert!((europa / 1.5608e6 - 1.0).abs() < 0.04, "{europa}");
        let icy_small = moon_radius(EarthMasses::new(1e-4), 0.45).value();
        let rock_small = moon_radius(EarthMasses::new(1e-4), 0.0).value();
        assert!(icy_small > rock_small);
    }

    #[test]
    fn generated_moons_of_jupiter_are_dry_inside_the_ice_line_and_icy_beyond() {
        let disc = solar::solar_disc();
        let (jup, derived) = derived_parent("Jupiter", planet_id(0, 5));
        let lights = [solar::sun()];
        let hosts = BodyHosts::new(
            Kilograms::new(SOLAR_MASS_KG),
            Composition::SOLAR,
            &lights,
            &[],
        )
        .unwrap();
        let sky = MoonSky::new(&derived, jup.orbit(), &hosts).unwrap();
        let nursery = MoonNursery::new(
            &jup,
            &Composition::SOLAR,
            &disc,
            jup.orbit().semi_major_axis(),
        )
        .unwrap();
        let line = nursery.ice_line().unwrap();
        let (mut icy, mut dry) = (0, 0);
        for i in 0..300 {
            let p = renamed(&jup, i, 5);
            for moon in regular_moons(Seed::new(6), &p).moons() {
                let rank = UnitUniform::new(0.3).unwrap();
                let d = derive_regular_moon(
                    moon,
                    &p,
                    &nursery,
                    &sky,
                    rank,
                    SOLAR_AGE,
                    UniverseTime::EPOCH,
                )
                .unwrap();
                if moon.orbit().semi_major_axis() >= line {
                    icy += 1;
                    assert!((d.ice_fraction() - 0.395).abs() < 1e-12);
                    assert_eq!(d.class(), PlanetClass::Icy);
                } else {
                    dry += 1;
                    assert_same_bits(d.ice_fraction(), 0.0);
                    assert_eq!(d.class(), PlanetClass::Rocky);
                    // Fortney et al.'s rock at 0.01 M⊕ is 2,430 kg m⁻³, the least dry density.
                    assert!(d.density() > KilogramsPerCubicMetre::new(2_400.0));
                }
            }
        }
        assert!(icy > 100 && dry > 100, "{icy} icy, {dry} dry");
    }

    #[test]
    fn the_solar_system_s_regular_moons_lock_and_so_do_the_generated_ones() {
        let gm_mass = |gm: f64| Kilograms::new(gm / GRAVITATIONAL_CONSTANT);
        let jupiter_kg = Kilograms::new(1.898_13e27);
        let saturn_kg = Kilograms::new(5.683_2e26);
        let uranus_kg = Kilograms::new(8.681_1e25);
        for (planet, name, gm, radius_km, a_km) in [
            (jupiter_kg, "Io", 5_959.9, 1_821.6, 421_800.0),
            (jupiter_kg, "Callisto", 7_179.3, 2_410.3, 1_882_700.0),
            (saturn_kg, "Titan", 8_978.1, 2_574.7, 1_221_900.0),
            (saturn_kg, "Iapetus", 120.5, 734.5, 3_561_700.0),
            (uranus_kg, "Oberon", 205.3, 761.4, 583_500.0),
        ] {
            let t = locking_time(
                planet,
                gm_mass(gm * 1e9),
                Metres::new(radius_km * 1e3),
                Metres::new(a_km * 1e3),
            );
            assert!(
                t.value() < 1e8 * 3.156e7,
                "{name}: {} Myr",
                t.value() / 3.156e13
            );
        }
        let disc = solar::solar_disc();
        let (jup, derived) = derived_parent("Jupiter", planet_id(0, 5));
        let lights = [solar::sun()];
        let hosts = BodyHosts::new(
            Kilograms::new(SOLAR_MASS_KG),
            Composition::SOLAR,
            &lights,
            &[],
        )
        .unwrap();
        let sky = MoonSky::new(&derived, jup.orbit(), &hosts).unwrap();
        let nursery = MoonNursery::new(
            &jup,
            &Composition::SOLAR,
            &disc,
            jup.orbit().semi_major_axis(),
        )
        .unwrap();
        let mut derived_moons = 0;
        for i in 0..300 {
            let p = renamed(&jup, i, 5);
            for moon in regular_moons(Seed::new(6), &p).moons() {
                let d = derive_regular_moon(
                    moon,
                    &p,
                    &nursery,
                    &sky,
                    UnitUniform::HALF,
                    SOLAR_AGE,
                    UniverseTime::EPOCH,
                )
                .unwrap();
                assert!(d.locking_time().value() < SOLAR_AGE.value() * 3.156e7);
                assert!(d.density() >= PLACEMENT_DENSITY, "{:?}", d.density());
                assert!(moon.orbit().periapsis() > p.roche_limit_fluid(d.density()));
                assert!(d.equilibrium_temperature().value() < 200.0);
                derived_moons += 1;
            }
        }
        assert!(derived_moons > 500);
    }

    fn parts(p: &MoonParent) -> MoonParentParts {
        MoonParentParts {
            id: p.id(),
            kind: p.kind(),
            mass: p.mass(),
            radius: p.radius(),
            class: p.class(),
            orbit: *p.orbit(),
            host_mass: p.host_mass(),
            maximum_moon_mass: p.maximum_moon_mass(),
        }
    }
}
