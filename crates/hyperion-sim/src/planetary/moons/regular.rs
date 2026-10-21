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
//! [`derive_regular_moon`] runs P14.T16's [`derive_body`]
//! with the planet as the tidal primary and the star as the source of light: the moon orbits the
//! planet's mass, is lit by the planet's own glow at its own distance and by the star at the
//! planet's orbit, and its composition is set by which side of the circumplanetary ice line it
//! formed on ([`MoonNursery`]): 170 K in the planet's own early luminosity, its luminosity when its
//! star's disc disperses, when the last generation of satellites forms. Tidal heating
//! ([`tidal_heating`]) gives a surface heat flux, a volcanism level and the subsurface-ocean flag
//! ([`has_subsurface_ocean`]).
//!
//! # Tides (P14.T56.d)
//!
//! A moon's tides are its own, by its composition and state
//! ([`tides`](crate::planetary::moons::tides)): one complex Love number per moon, on the cold, ocean
//! (after Beuthe 2015's membrane, decision-p14-t56d-membrane) or heated branch its whole heat flow
//! puts it on, read at its own bulk ([`MoonBulk`]). Its tidal
//! heat reads its loss Im k₂, its ocean flag is its branch's, and its locking time its (k₂, Q),
//! which the stored rotation law reads too. They replace version 22's, which took Io's response
//! [`MOON_TIDAL_RESPONSE`] for every moon's heat, the flag at that tidal flux alone, and the class
//! pair k₂ = 0.3, Q = 100 for its locking, at its nursery's radius.
//!
//! # Volatiles (P14.T56.c)
//!
//! A moon's volatile inventory reads [`MoonVolatiles`]: its own ranks, words 0–5 of its
//! `planet.volatiles` stream keyed by its candidate as its radius rank is
//! ([`Satellite::volatiles`](crate::planetary::satellites::Satellite::volatiles); ruling 112.5),
//! and where its volatiles formed ([`VolatileSite`]). A regular moon's formed in the nursery at
//! its own distance, so its nitrogen takes the icy boost only where the circumplanetary disc was
//! at or below ammonia hydrate's 123 K, 170 K × √(`d_ice` ÷ a) (P14.T50.c's rule, which
//! [`derive_body`] reads in the nursery's disc), and its
//! water and carbon beyond the 170 K ice line. A captured or giant-impact moon's formed in its
//! parent's zone of the star's disc ([`MoonNursery::parent_zone`]), as the parent's own did, with
//! T50.c's rule there; its bulk is still its own (decision-p14-moons §2.2).
//!
//! The inventory and the atmosphere are read at the moon's own bulk ([`MoonBulk`]): its radius,
//! mass and so its surface gravity, which its Jeans escape and the weight of its air read, and its
//! mass fractions, whose ice is the inventory's water floor beyond the snow line. A regular moon's
//! is ruling 83.7's, [`moon_radius`] at its ice fraction; a giant-impact moon's and a capture's are
//! their own, as their records state them. Version 22 derived every moon at the median ranks in
//! its nursery at its own distance and read the composition solve that
//! [`derive_body`] gives a body of the moon's mass at its
//! radius rank, an error (P14.T56.c's follow-up).
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
use crate::planetary::derive::atmosphere::VolatileDraws;
use crate::planetary::derive::rotation::{
    SpinTides, SpinningBody, moment_of_inertia_factor, tidal_locking_time,
};
use crate::planetary::derive::{
    BodyHosts, BuildBodyHostsError, BuildFormationZoneError, BuildPlacedBodyError, DeriveBodyError,
    DerivedBody, FormationZone, GivenBulk, GivenFractions, HostLight, Illumination, MassFractions,
    PlacedBody, PlanetClass, SnowLineSide, derive_body,
};
use crate::planetary::disc::{self, DiscDraws, DiscHost, DiscProfile, Truncation};
use crate::planetary::hooks::figures::{HeatInputs, Subsurface, non_tidal_heat_flow};
use crate::planetary::moons::tides::{
    MoonTides, TidalBranch, TidalMaterial, TideInputs, mean_density, tidal_forcing,
};
use crate::planetary::moons::{MoonParent, decide, draw_rank, log_uniform, moon_orbit, rayleigh};
use crate::planetary::placement::spacing::{
    MAX_SPACING_REDRAWS, Neighbour, PAIR_SPACING_SIGMA, mutual_hill_factor, next_semi_major_axis,
    satisfies_floor, spacing_floor,
};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::draws::UnitUniform;
use crate::stellar::substellar::{GIANT_MIN_MASS, giant_cooling};
use crate::time::UniverseTime;
use crate::units::consts::{GM_EARTH, GRAVITATIONAL_CONSTANT, STEFAN_BOLTZMANN};
use crate::units::{
    Degrees, Dex, EarthMasses, JupiterMasses, Kelvin, Kilograms, KilogramsPerCubicMetre, Metres,
    MetresPerSecondSquared, Radians, Seconds, SolarLuminosities, SolarMasses, SolarRadii, Watts,
    WattsPerSquareMetre, Years,
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

// The moonlet count takes one word of `tags::MOON_COUNT` only below the Poisson sampler's switch
// to its two-word method; at or above it, the count would read the words after it.
const _: () = assert!(MOONLET_MEAN < crate::rng::POISSON_PTRS_MIN_MEAN);

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

/// The tidal response k₂ ÷ Q of a tidally heated rocky moon, its loss Im k₂: 0.015 (P14.T17.b;
/// P14.T56.d's heated branch).
///
/// Io's, measured from its astrometry: k₂ ÷ Q = 0.015 ± 0.003 (Lainey et al. 2009, Nature 459,
/// 957, abstract). Version 22 read it for every moon's tidal heat (ruling 83.1); P14.T56.d reads it
/// for the heated rocky branch only ([`tides`](crate::planetary::moons::tides), decision-p14-moons
/// §3.2), and the cold and ocean branches read their own. P14.T14.b's rocky body, k₂ = 0.3 and Q =
/// 100, is 0.003, which would give Io 0.45 W m⁻² against its observed 1.4–3.8; this value gives it
/// 2.2.
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

/// The decision for probability `p` of the word at `offset` of moon `ordinal`'s block of
/// `stream`, a [`tags::MOON_ORBIT`] stream ([`decide`]).
fn orbit_decides(stream: &Stream, ordinal: u8, offset: u64, p: f64) -> bool {
    let mut at = stream.clone();
    at.seek(orbit_block(ordinal) + offset);
    decide(&mut at, p)
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
        resonant_with_inner: ordinal > 1
            && orbit_decides(stream, ordinal, RESONANCE_WORD, RESONANCE_PROBABILITY),
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
    let titan = n > 1 && decide(&mut count, TITAN_PROBABILITY);
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
#[must_use]
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
///
/// It also holds where the planet itself formed in its star's disc
/// ([`parent_zone`](Self::parent_zone)), whose volatiles a captured or giant-impact moon takes
/// (P14.T56.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonNursery {
    early_luminosity: SolarLuminosities,
    disc: DiscProfile,
    parent_zone: FormationZone,
}

impl MoonNursery {
    /// The nursery of `parent`, of composition `composition`, formed at `formation_distance` from
    /// its star in the star's disc `star_disc`.
    ///
    /// # Errors
    ///
    /// [`DeriveMoonError::GiantCooling`] if plan 13's cooling refuses the planet's mass (over 13
    /// `M_J`); [`DeriveMoonError::Nursery`] if the circumplanetary disc could not be built; and
    /// [`DeriveMoonError::ParentZone`] if the formation distance is not positive and finite.
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
        let parent_zone = FormationZone::new(formation_distance, star_disc.snow_line())
            .map_err(DeriveMoonError::ParentZone)?;
        Ok(Self {
            early_luminosity,
            disc,
            parent_zone,
        })
    }

    /// The planet's luminosity when its satellites formed.
    #[must_use]
    pub const fn early_luminosity(&self) -> SolarLuminosities {
        self.early_luminosity
    }

    /// The circumplanetary ice line, or `None` for a planet formed inside its star's snow line,
    /// about which no ice condensed ([`parent_zone`](Self::parent_zone)'s side: for its positive,
    /// finite distances, "not inside" is "at or beyond").
    #[must_use]
    pub fn ice_line(&self) -> Option<Metres> {
        (self.parent_zone.side() == SnowLineSide::Beyond).then(|| self.disc.snow_line())
    }

    /// Where the planet formed in its star's disc: its formation distance and the star's disc's
    /// snow line, whose volatiles a captured or giant-impact moon takes (P14.T56.c,
    /// [`VolatileSite::ParentZone`]), as the planet's own inventory reads them.
    #[must_use]
    pub const fn parent_zone(&self) -> &FormationZone {
        &self.parent_zone
    }
}

/// Where a moon's volatiles formed, as its inventory reads them (P14.T56.c; decision-p14-moons
/// §2.2 (b)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VolatileSite {
    /// In its planet's circumplanetary disc at its own distance: a regular moon's, formed there.
    Nursery,
    /// In its parent's zone of the star's disc ([`MoonNursery::parent_zone`]): a captured moon's,
    /// which formed in the star's disc and not its planet's, and a giant-impact moon's, made of
    /// its parent's mantle and the impactor (Canup 2012, Science 338, 1052, abstract read by the
    /// science check; the zone takes the impactor to have formed near its target).
    ParentZone,
}

/// What a moon's volatile inventory reads (P14.T56.c): its ranks and where its volatiles formed.
///
/// The fields are plain values with no invariant between them, so they are public, as
/// [`VolatileDraws`]'s are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonVolatiles {
    /// The ranks of its water, carbon, nitrogen, sulphur, chlorine and ΔIW: a generated moon's
    /// own ([`Satellite::volatiles`](crate::planetary::satellites::Satellite::volatiles)).
    pub draws: VolatileDraws,
    /// Where its volatiles formed.
    pub site: VolatileSite,
}

impl MoonVolatiles {
    /// Every rank at its median, the volatiles formed in the nursery at the moon's own distance:
    /// the Solar System table's and tools' convention, as [`VolatileDraws::MEDIAN`] is a planet's
    /// (version 22's for every moon).
    pub const SHARED: Self = Self {
        draws: VolatileDraws::MEDIAN,
        site: VolatileSite::Nursery,
    };
}

/// A moon's own bulk, at which its volatile inventory, atmosphere and tides are read (P14.T56.c's
/// follow-up, P14.T56.d): its radius, its mass and so its surface gravity, and its mass fractions,
/// as its record's bulk section states them (P14.T22.a).
///
/// Version 22 read the air at the composition solve that
/// [`derive_body`] gives a body of the moon's mass at its
/// radius rank in the nursery: 2,281 km for a Ganymede at the median rank, against its own
/// 2,653 km by its ice fraction. Its water floor then read that solve's water, not its ice.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MoonBulk {
    /// As [`derive_moon`] derives it, ruling 83.7's: the radius [`moon_radius`] gives at its ice
    /// fraction ([`DerivedMoon::radius`]), and that ice over the solids P14.T16's derivation
    /// splits between iron and rock ([`DerivedMoon::fractions`]). A regular moon's.
    Derived,
    /// As its origin states it apart from its derivation: a giant-impact moon's sphere at its own
    /// density, with its parent's fractions or rock, and a capture's own radius, with the ice its
    /// density holds.
    Stated {
        /// Its mean radius, positive and finite ([`DeriveMoonError::StatedRadius`] otherwise).
        radius: Metres,
        /// Its mass fractions.
        fractions: MassFractions,
    },
}

/// What [`derive_moon`] reads of the moon itself (P14.T56.d gathers them): its mass, its orbit
/// about its planet, its radius rank, its volatiles and its own bulk.
///
/// The fields are plain values: [`derive_moon`] refuses a stated bulk whose radius is not
/// positive and finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonInputs {
    /// Its mass.
    pub mass: EarthMasses,
    /// Its orbit about its planet, with μ = G (`M_p` + m).
    pub orbit: KeplerElements,
    /// The radius rank its own `planet.radius` stream gives it.
    pub radius_rank: UnitUniform,
    /// Its volatile ranks and where they formed.
    pub volatiles: MoonVolatiles,
    /// Its own bulk, at which its air and its tides are read.
    pub bulk: MoonBulk,
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
    tide: TideState,
}

/// What a moon's tides give it at a time, built once from them ([`MoonTides`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct TideState {
    tidal_heating: Watts,
    heat_flux: WattsPerSquareMetre,
    volcanism: Volcanism,
    subsurface: Subsurface,
    ocean_heat_flow: WattsPerSquareMetre,
    locking_time: Seconds,
    tides: MoonTides,
}

impl DerivedMoon {
    /// The moon's volatile inventory at the time, from P14.T16's [`derive_body`] (P14.T13.a): the
    /// stored carriers of its [`element_inventory`](Self::element_inventory).
    #[must_use]
    pub const fn inventory(&self) -> &crate::planetary::derive::atmosphere::VolatileInventory {
        self.body.inventory()
    }

    /// The moon's element inventory at the time (P14.T50.a), from P14.T16's [`derive_body`], which
    /// P14.T23's [`BulkComposition`](crate::planetary::hooks::BulkComposition) holds: built at its
    /// own volatile ranks where its volatiles formed (P14.T56.c), in its planet's nursery or, for a
    /// captured or giant-impact moon, its parent's zone of the star's disc.
    #[must_use]
    pub const fn element_inventory(&self) -> &crate::planetary::chem::inventory::ElementInventory {
        self.body.element_inventory()
    }

    /// The flux the moon receives from its planet and the stars, from P14.T16's
    /// [`derive_body`].
    #[must_use]
    pub const fn flux(&self) -> crate::units::EarthFluxes {
        self.body.flux()
    }

    /// The moon's equilibrium temperature, from P14.T16's
    /// [`derive_body`].
    #[must_use]
    pub const fn equilibrium_temperature(&self) -> Kelvin {
        self.body.equilibrium_temperature()
    }

    /// The X-ray and ultraviolet flux the stars give the moon at its planet's orbit, from
    /// P14.T16's [`derive_body`] (P14.T24.a's radiation
    /// class reads it).
    #[must_use]
    pub const fn xuv_flux(&self) -> crate::units::WattsPerSquareMetre {
        self.body.xuv_flux()
    }

    /// The moon's atmosphere and surface at the time, from P14.T16's
    /// [`derive_body`] (P14.T13), which its photometry reads
    /// (P14.T47.b).
    #[must_use]
    pub const fn atmosphere(&self) -> &crate::planetary::derive::atmosphere::Atmosphere {
        self.body.atmosphere()
    }

    /// The Bond albedo the moon's equilibrium temperature was taken at, from P14.T16's
    /// [`derive_body`]: the one its photometry states
    /// (P14.T51.b).
    #[must_use]
    pub(crate) const fn albedo(&self) -> crate::planetary::derive::BondAlbedo {
        self.body.albedo()
    }

    /// The Bond albedo of its bare regolith, airless (P14.T51.i;
    /// [`DerivedBody::regolith_albedo`]).
    #[must_use]
    pub(crate) const fn regolith_albedo(&self) -> f64 {
        self.body.regolith_albedo()
    }

    /// How many passes its derivation took (P14.T51.b; [`DerivedBody::atmosphere_passes`]).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn atmosphere_passes(&self) -> u32 {
        self.body.atmosphere_passes()
    }

    /// Its derivation as a body (P14.T51.h's sample test).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn body(&self) -> &DerivedBody {
        &self.body
    }

    /// How its derivation's passes ended (P14.T51.h; [`DerivedBody::convergence`]).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn convergence(&self) -> crate::planetary::derive::passes::Convergence {
        self.body.convergence()
    }

    /// The moon's own Hill radius about its planet, from P14.T16's
    /// [`derive_body`].
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

    /// The moon's mass fractions: its ice as water, and the rest split between iron and rock as
    /// P14.T16's derivation splits the solids of a rocky body of its mass and radius rank
    /// (P14.T22.a's record of a moon).
    #[must_use]
    pub fn fractions(&self) -> MassFractions {
        self.body.fractions().with_water(self.ice_fraction)
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

    /// The tidal heat its planet raises in it, W ([`tidal_heating`]): at its own loss Im k₂
    /// ([`tides`](Self::tides)) and its own bulk's radius ([`MoonBulk`]).
    #[must_use]
    pub const fn tidal_heating(&self) -> Watts {
        self.tide.tidal_heating
    }

    /// Its tidal heat spread over its surface, W m⁻²: its own bulk's ([`MoonBulk`]), its
    /// record's.
    #[must_use]
    pub const fn heat_flux(&self) -> WattsPerSquareMetre {
        self.tide.heat_flux
    }

    /// The heat flow its ocean flag reads, W m⁻²: its branch's whole heat flow, P14.T24.b's
    /// non-tidal `F_nt` with its tide, `F_ocean` on the ocean branch ([`MoonTides::heat_flow`];
    /// decision-p14-moons §F-ocean F3).
    #[must_use]
    pub const fn ocean_heat_flow(&self) -> WattsPerSquareMetre {
        self.tide.ocean_heat_flow
    }

    /// Its tides by composition and state (P14.T56.d): its k₂, Q, loss, branch and whole heat
    /// flow.
    #[must_use]
    pub const fn tides(&self) -> MoonTides {
        self.tide.tides
    }

    /// The tides its spin's locking reads: its own pair, as its stored rotation law takes them
    /// (P14.T56.d).
    #[must_use]
    pub const fn spin_tides(&self) -> SpinTides {
        self.tide.tides.spin_tides()
    }

    /// Its volcanism level, from the heat flux.
    #[must_use]
    pub const fn volcanism(&self) -> Volcanism {
        self.tide.volcanism
    }

    /// Whether it holds liquid water under its ice: whether its tides are on the ocean branch,
    /// where [`has_subsurface_ocean`] holds at its whole heat flow
    /// ([`ocean_heat_flow`](Self::ocean_heat_flow)).
    #[must_use]
    pub const fn subsurface_ocean(&self) -> bool {
        matches!(self.tide.subsurface, Subsurface::Ocean(_))
    }

    /// Its subsurface ocean as P14.T24.b's figures read it ([`Subsurface::of_regular_moon`]),
    /// from the very layers its flag was decided on: the ice and water of the bulk its tides read
    /// under its surface at its equilibrium temperature, with the heat flow its flag reads; none
    /// where its flag holds none.
    #[must_use]
    pub(crate) const fn subsurface(&self) -> Subsurface {
        self.tide.subsurface
    }

    /// How long its planet's tides take to lock its spin ([`locking_time`]).
    #[must_use]
    pub const fn locking_time(&self) -> Seconds {
        self.tide.locking_time
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
    ocean_layers(radius, density, water, surface, flux)
        .is_some_and(|layers| layers.shell_thickness < layers.water_depth)
}

/// The two depths [`has_subsurface_ocean`] compares for a moon of radius `radius`, density
/// `density` and water mass fraction `water`, with a surface at `surface` and a heat flux `flux`
/// from below (P14.T17.b): the ice shell that conducts the flux, 651 ln(273 K ÷ `T_s`) ÷ F, and
/// the water layer, R (1 − (1 − x ρ ÷ `ρ_w`)^⅓). The moon holds an ocean where the shell is the
/// thinner, `water − shell` deep.
///
/// `None` where no ocean can be: a moon without water, with no flux from below, or with a surface
/// at or above the melting point.
#[must_use]
pub(crate) fn ocean_layers(
    radius: Metres,
    density: KilogramsPerCubicMetre,
    water: f64,
    surface: Kelvin,
    flux: WattsPerSquareMetre,
) -> Option<OceanLayers> {
    let (t_s, f) = (surface.value(), flux.value());
    if water <= 0.0 || f <= 0.0 || t_s >= ICE_MELTING_POINT.value() {
        return None;
    }
    let volume_share = (water * density.value() / WATER_DENSITY.value()).min(1.0);
    let layer = radius.value() * (1.0 - math::cbrt(1.0 - volume_share));
    let shell =
        ICE_CONDUCTIVITY_COEFFICIENT * math::ln(ICE_MELTING_POINT.value() / t_s.max(1.0)) / f;
    Some(OceanLayers {
        shell_thickness: Metres::new(shell),
        water_depth: Metres::new(layer),
    })
}

/// The depths of a moon's ice shell and of its water ([`ocean_layers`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OceanLayers {
    /// The ice shell that conducts the heat flux from the melting point to the surface.
    pub(crate) shell_thickness: Metres,
    /// The moon's water spread into a layer at [`WATER_DENSITY`] over its outer part.
    pub(crate) water_depth: Metres,
}

/// The time a planet of mass `planet_mass` takes to lock the spin of a moon of mass `mass`,
/// radius `radius`, class `class` and mass fractions `fractions` at semi-major axis `a`, from a
/// period of [`MOON_PRIMORDIAL_PERIOD_HOURS`]: P14.T14.b's
/// [`tidal_locking_time`], with the
/// moon's one moment of inertia,
/// [`moment_of_inertia_factor`]
/// (P14.T46.a, which retired the moons' own 0.35), and the tides `tides` say: a moon's own (k₂, Q)
/// by composition and state ([`MoonTides::spin_tides`], P14.T56.d), for P14.T17.b's "all regular
/// moons lock" as amended, or its class's ([`SpinTides::OfClass`]: for a rocky body P14.T14.b's
/// k₂ = 0.3 and Q = 100, version 22's for every moon; the despinning time is Gladman et al.
/// 1996's, Icarus 122, 166).
///
/// # Panics
///
/// If `mass` or `radius` is not positive and finite, or a given pair is not, which a derived
/// moon's never is.
#[must_use]
pub fn locking_time(
    planet_mass: Kilograms,
    mass: Kilograms,
    radius: Metres,
    a: Metres,
    class: PlanetClass,
    fractions: &MassFractions,
    tides: SpinTides,
) -> Seconds {
    let moon = SpinningBody::with_tides(mass, radius, class, fractions, tides)
        .expect("a moon's mass, radius and tides are positive and finite");
    tidal_locking_time(
        &moon,
        Seconds::new(MOON_PRIMORDIAL_PERIOD_HOURS * 3_600.0),
        a,
        planet_mass,
    )
}

/// The regular moon `moon` of `parent`, formed in `nursery` and lit by `sky`, derived at `t` in a
/// system aged `age` at the epoch, with the radius rank `radius_rank` its own `planet.radius`
/// stream gives it and the ranks `volatiles` of its volatile inventory (P14.T17.b, P14.T56.c; see
/// the [module](self) documentation).
///
/// The moon is a [`PlacedBody`] on its orbit about the planet, formed at its own distance from the
/// planet in the circumplanetary disc, or, about a planet with no ice line, at a point inside the
/// disc's line; [`derive_body`] reads it with the planet's
/// mass as its primary, the planet's glow as the host it orbits, and the stars at the planet's
/// orbit as the companions that light it. Its volatiles formed there too
/// ([`VolatileSite::Nursery`]). Its inventory, atmosphere and tides are read at its own radius,
/// mass and fractions ([`MoonBulk::Derived`]), not that derivation's.
///
/// # Errors
///
/// [`DeriveMoonError::Body`] where [`derive_body`]
/// refuses (a system not yet born at `t`), and [`DeriveMoonError::Placed`] or
/// [`DeriveMoonError::Hosts`] for inputs it cannot take.
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
/// use hyperion_sim::planetary::derive::atmosphere::VolatileDraws;
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
///     let (rank, volatiles) = (UnitUniform::HALF, VolatileDraws::MEDIAN);
///     let derived = derive_regular_moon(moon, &parent, &nursery, &sky, rank, volatiles, age, UniverseTime::EPOCH)?;
///     // Far from the Sun and locked to Jupiter.
///     assert!(derived.equilibrium_temperature().value() < 150.0);
///     assert!(derived.locking_time().value() < 1e16);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[expect(
    clippy::too_many_arguments,
    reason = "the moon, where it formed and what lights it, its two draws and the times"
)]
pub fn derive_regular_moon(
    moon: &RegularMoon,
    parent: &MoonParent,
    nursery: &MoonNursery,
    sky: &MoonSky,
    radius_rank: UnitUniform,
    volatiles: VolatileDraws,
    age: Years,
    t: UniverseTime,
) -> Result<DerivedMoon, DeriveMoonError> {
    derive_moon(
        &MoonInputs {
            mass: moon.mass,
            orbit: moon.orbit,
            radius_rank,
            volatiles: MoonVolatiles {
                draws: volatiles,
                site: VolatileSite::Nursery,
            },
            bulk: MoonBulk::Derived,
        },
        parent,
        nursery,
        sky,
        age,
        t,
    )
}

/// The moon `moon` of `parent`, derived as [`derive_regular_moon`] derives a regular one, with
/// its volatiles, and its inventory, atmosphere and tides read at its own bulk
/// ([`MoonInputs::bulk`]): what P14.T22.a's satellites share, whatever their origin (a
/// giant-impact moon's or a capture's flux and temperature come from here, beside their own radius
/// and density, and from P14.T56.c their inventory from their parent's zone,
/// [`VolatileSite::ParentZone`], its follow-up their air at that radius, [`MoonBulk::Stated`], and
/// P14.T56.d their tides by composition and state, [`MoonTides`]).
///
/// # Errors
///
/// As [`derive_regular_moon`], and [`DeriveMoonError::StatedRadius`] for a stated bulk whose
/// radius is not positive and finite.
pub fn derive_moon(
    moon: &MoonInputs,
    parent: &MoonParent,
    nursery: &MoonNursery,
    sky: &MoonSky,
    age: Years,
    t: UniverseTime,
) -> Result<DerivedMoon, DeriveMoonError> {
    let MoonInputs {
        mass,
        orbit,
        radius_rank,
        volatiles,
        bulk,
    } = *moon;
    if let MoonBulk::Stated { radius, .. } = bulk
        && !(radius.value().is_finite() && radius.value() > 0.0)
    {
        return Err(DeriveMoonError::StatedRadius);
    }
    let a = orbit.semi_major_axis();
    let icy = nursery.ice_line().is_some_and(|line| a >= line);
    let ice_fraction = if icy {
        let (lo, hi) = ICY_MOON_ICE_FRACTION;
        lo + radius_rank.value() * (hi - lo)
    } else {
        0.0
    };
    let radius = moon_radius(mass, ice_fraction);
    let formation = match nursery.ice_line() {
        Some(_) => a,
        None => Metres::new(a.value().min(0.5 * nursery.disc.snow_line().value())),
    };
    let placed = PlacedBody::new(mass, orbit, formation, radius_rank)
        .map_err(DeriveMoonError::Placed)?
        .with_volatiles(volatiles.draws);
    let placed = match volatiles.site {
        VolatileSite::Nursery => placed,
        VolatileSite::ParentZone => placed.with_volatiles_from(nursery.parent_zone),
    };
    let placed = match bulk {
        MoonBulk::Derived => {
            placed.with_bulk(GivenBulk::new(radius, GivenFractions::Water(ice_fraction)))
        }
        MoonBulk::Stated {
            radius: stated,
            fractions,
        } => placed.with_bulk(GivenBulk::new(stated, GivenFractions::Exactly(fractions))),
    };
    let hosts = BodyHosts::new(
        Kilograms::from(parent.mass()),
        sky.composition,
        &sky.planet,
        &sky.stars,
    )
    .map_err(DeriveMoonError::Hosts)?;
    let body =
        derive_body(&placed, &hosts, &nursery.disc, age, t).map_err(DeriveMoonError::Body)?;
    let density = mean_density(Kilograms::from(mass), radius);
    let at = TideOf {
        planet: Kilograms::from(parent.mass()),
        mass,
        orbit: &orbit,
        body: &body,
        radius,
        ice_fraction,
    };
    let age_now = Years::new(age.value() + t.since_epoch().as_julian_years_f64());
    let tide = own_tides(&at, bulk, sky.composition.fe_h(), age_now);
    Ok(DerivedMoon {
        body,
        ice_fraction,
        radius,
        density,
        tide,
    })
}

/// What a moon's tide reads (crate-private): its planet's mass, its own mass and orbit about the
/// planet, its derivation, and the radius and ice fraction its derivation gives it.
#[derive(Debug, Clone, Copy)]
struct TideOf<'a> {
    planet: Kilograms,
    mass: EarthMasses,
    orbit: &'a KeplerElements,
    body: &'a DerivedBody,
    radius: Metres,
    ice_fraction: f64,
}

/// A moon's orbit as its eccentricity tide reads it ([`TideOf::elements`]).
#[derive(Debug, Clone, Copy)]
struct TideElements {
    a: Metres,
    e: f64,
    mean_motion_rad_per_s: f64,
}

impl TideOf<'_> {
    /// The orbit's semi-major axis, eccentricity and mean motion.
    #[must_use]
    fn elements(&self) -> TideElements {
        TideElements {
            a: self.orbit.semi_major_axis(),
            e: self.orbit.eccentricity().value(),
            mean_motion_rad_per_s: core::f64::consts::TAU / self.orbit.period().value(),
        }
    }

    /// The fractions its derivation gives it, [`DerivedMoon::fractions`]'s.
    #[must_use]
    fn fractions(&self) -> MassFractions {
        self.body.fractions().with_water(self.ice_fraction)
    }
}

/// Its own tides by composition and state (P14.T56.d; decision-p14-moons §3 and §F-ocean), at its
/// own bulk `bulk` (a regular moon's its derivation's, a giant-impact moon's or a capture's its
/// record's) about a host of \[Fe/H\] `fe_h`, at the system age `age`: its branch's loss for its
/// heat over that bulk's surface, its branch's ocean, and its (k₂, Q) for its locking, at the
/// class and moment of inertia its record states.
///
/// `F_nt` is P14.T24.b's at its record's bulk with its own air and water
/// ([`non_tidal_heat_flow`]), with no tide and no ocean given, which the branch decides; the
/// shell's top is at its equilibrium temperature, as its ocean flag reads it. Its ocean's layers
/// are those its branch was decided on, at that bulk's mean density ([`mean_density`]), which is
/// its derivation's to the bit for a regular moon.
#[must_use]
fn own_tides(at: &TideOf<'_>, bulk: MoonBulk, fe_h: Dex, age: Years) -> TideState {
    let TideElements {
        a,
        e,
        mean_motion_rad_per_s: n,
    } = at.elements();
    let (radius, fractions) = match bulk {
        MoonBulk::Derived => (at.radius, at.fractions()),
        MoonBulk::Stated { radius, fractions } => (radius, fractions),
    };
    let class = PlanetClass::of(at.mass, &fractions);
    let kilograms = Kilograms::from(at.mass);
    let r = radius.value();
    let non_tidal = non_tidal_heat_flow(
        &HeatInputs {
            mass: at.mass,
            radius,
            surface_gravity: MetresPerSecondSquared::new(GM_EARTH * at.mass.value() / (r * r)),
            class,
            fractions,
            inventory_water: at.body.inventory().water(),
            fe_h,
            tidal_heat_flow: WattsPerSquareMetre::new(0.0),
            subsurface_ocean: Subsurface::NotComputed,
        },
        age,
        at.body.atmosphere().state(),
    )
    // A moon's class is rocky or icy, so it always has one; none is a body with no solid heat.
    .unwrap_or(WattsPerSquareMetre::new(0.0));
    let surface = at.body.equilibrium_temperature();
    let tides = MoonTides::of(&TideInputs {
        material: TidalMaterial::of_water(fractions.water()),
        moment_factor: moment_of_inertia_factor(class, &fractions),
        mass: kilograms,
        radius,
        water: fractions.water(),
        surface_temperature: surface,
        non_tidal_heat_flow: non_tidal,
        forcing: tidal_forcing(at.planet, radius, a, e, n),
        mean_motion_rad_per_s: n,
    });
    let heat = tidal_heating(at.planet, radius, tides.loss(), a, e, n);
    let area = 4.0 * core::f64::consts::PI * r * r;
    let flux = WattsPerSquareMetre::new(heat.value() / area);
    let ocean = tides.branch() == TidalBranch::Ocean;
    let subsurface = if ocean {
        Subsurface::of_regular_moon(
            radius,
            mean_density(kilograms, radius),
            fractions.water(),
            surface,
            tides.heat_flow(),
        )
    } else {
        Subsurface::NoOcean
    };
    debug_assert_eq!(
        matches!(subsurface, Subsurface::Ocean(_)),
        ocean,
        "an ocean-branch moon's layers are those its branch was decided on"
    );
    TideState {
        tidal_heating: heat,
        heat_flux: flux,
        volcanism: Volcanism::of(flux),
        subsurface,
        ocean_heat_flow: tides.heat_flow(),
        locking_time: locking_time(
            at.planet,
            kilograms,
            radius,
            a,
            class,
            &fractions,
            tides.spin_tides(),
        ),
        tides,
    }
}

/// A moon could not be derived.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveMoonError {
    /// Plan 13's giant cooling refused the planet (above 13 `M_J`).
    GiantCooling(crate::stellar::substellar::EvaluateGiantCoolingError),
    /// The circumplanetary disc could not be built.
    Nursery,
    /// Where the planet formed in its star's disc could not be stated (P14.T56.c): a formation
    /// distance or a snow line that is not positive and finite.
    ParentZone(BuildFormationZoneError),
    /// The planet's light could not be built.
    Sky,
    /// The moon could not be placed for the derivation.
    Placed(BuildPlacedBodyError),
    /// The radius of the bulk stated for it ([`MoonBulk::Stated`]) was not positive and finite.
    StatedRadius,
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
            Self::ParentZone(_) => "where the planet formed could not be stated",
            Self::Sky => "the planet's light could not be built",
            Self::Placed(_) => "the moon could not be placed",
            Self::StatedRadius => "the moon's stated radius was not positive and finite",
            Self::Hosts(_) => "the moon's hosts could not be built",
            Self::Body(_) => "the moon could not be derived",
        })
    }
}

impl Error for DeriveMoonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GiantCooling(e) => Some(e),
            Self::ParentZone(e) => Some(e),
            Self::Placed(e) => Some(e),
            Self::Hosts(e) => Some(e),
            Self::Body(e) => Some(e),
            Self::Nursery | Self::Sky | Self::StatedRadius => None,
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
                // 3–8 planetary radii, the range shifted out (keeping its 8 : 3) where the
                // pericentre would fall inside the fluid Roche limit.
                let r = p.radius().value();
                let e = first.orbit().eccentricity().value();
                let roche = p.roche_limit_fluid(PLACEMENT_DENSITY).value();
                let lo = (3.0 * r).max(roche / (1.0 - e));
                let hi = (8.0 * r).max(lo * 8.0 / 3.0);
                let a = first.orbit().semi_major_axis().value();
                assert!(
                    (lo * (1.0 - 1e-6)..hi * (1.0 + 1e-6)).contains(&a),
                    "{lo} {a} {hi}"
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
            assert_eq!(system.moons(), []);
            assert_eq!(system.moonlets(), 0);
            for p in [&rocky, &small, &light] {
                assert_eq!(
                    regular_moons(Seed::new(9), &renamed(p, i, 2)),
                    RegularMoons::NONE
                );
            }
        }
    }

    /// The tidal limit of P14.T15 drops moons from the outside in: under a limit between the
    /// unlimited system's partial sums, the moons kept are exactly its innermost ones whose
    /// running total fits, and the drawn total is untouched.
    #[test]
    fn the_moon_mass_limit_drops_moons_from_the_outside_in() {
        let (mut cut, mut kept_some) = (0u32, 0u32);
        for p in giants(100) {
            let free = regular_moons(Seed::new(4), &p);
            if free.moons().len() < 2 {
                continue;
            }
            // Between the first moon's mass and the whole system's.
            let first = free.moons()[0].mass().value();
            let limit = EarthMasses::new(f64::midpoint(first, free.total_mass().value()));
            let limited = MoonParent::new(MoonParentParts {
                maximum_moon_mass: limit,
                ..parts(&p)
            })
            .unwrap();
            let system = regular_moons(Seed::new(4), &limited);
            let n = system.moons().len();
            assert_eq!(system.moons(), &free.moons()[..n]);
            assert!(system.total_mass() <= limit);
            assert!(n < free.moons().len());
            let next: f64 = free.moons()[..=n].iter().map(|m| m.mass().value()).sum();
            assert!(next > limit.value(), "the next moon would have fitted");
            assert_same_bits(system.drawn_mass().value(), free.drawn_mass().value());
            cut += 1;
            kept_some += u32::from(n >= 1);
        }
        assert!(cut > 100, "{cut}");
        assert_eq!(kept_some, cut, "the first moon always fits under the limit");
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
                    VolatileDraws::MEDIAN,
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

    /// P14.T17.b: every generated moon of the table's Jupiter is at least as dense as its
    /// placement assumed, outside its fluid Roche limit, and cold. T17.b's test (b), that the
    /// regular moons lock, is the amended one
    /// (`moons::tides::tests::the_measured_moons_lock_as_t17b_amended` and
    /// `the_share_of_generated_regular_moons_unlocked_at_the_epoch_is_recorded`).
    #[test]
    fn generated_moons_of_jupiter_are_dense_cold_and_outside_their_roche_limits() {
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
                    VolatileDraws::MEDIAN,
                    SOLAR_AGE,
                    UniverseTime::EPOCH,
                )
                .unwrap();
                assert!(d.density() >= PLACEMENT_DENSITY, "{:?}", d.density());
                assert!(moon.orbit().periapsis() > p.roche_limit_fluid(d.density()));
                assert!(d.equilibrium_temperature().value() < 200.0);
                derived_moons += 1;
            }
        }
        assert!(derived_moons > 500);
    }

    /// The table's giant `name` in slot `slot` as its moons read it (P14.T56.c): the parent,
    /// derived at the epoch about the present Sun, its nursery, formed at `formed_au` in the
    /// zero-age Sun's disc (its own distance unless a test moves it), and its moons' sky.
    fn table_giant(
        name: &str,
        slot: u8,
        formed_au: Option<f64>,
    ) -> (MoonParent, MoonNursery, MoonSky) {
        let disc = solar::solar_disc();
        let (parent, derived) = derived_parent(name, planet_id(0, slot));
        let lights = [solar::sun()];
        let hosts = BodyHosts::new(
            Kilograms::new(SOLAR_MASS_KG),
            Composition::SOLAR,
            &lights,
            &[],
        )
        .unwrap();
        let sky = MoonSky::new(&derived, parent.orbit(), &hosts).unwrap();
        let formed = formed_au.map_or(parent.orbit().semi_major_axis(), |au| {
            Metres::new(au * crate::units::consts::METRES_PER_AU)
        });
        let nursery = MoonNursery::new(&parent, &Composition::SOLAR, &disc, formed).unwrap();
        (parent, nursery, sky)
    }

    /// An orbit of semi-major axis `a_km` and eccentricity `e` about `parent`, in its reference
    /// plane.
    fn about(parent: &MoonParent, a_km: f64, e: f64) -> KeplerElements {
        use crate::orbit::{Eccentricity, Orientation};
        let flat = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap();
        KeplerElements::from_semi_major_axis(
            Metres::new(a_km * 1e3),
            crate::units::GravitationalParameter::from_earth_masses(parent.mass()),
            Eccentricity::new(e).unwrap(),
            flat,
            Radians::ZERO,
        )
        .unwrap()
    }

    /// A moon of `kg` on `orbit` about `parent`, from `nursery` under `sky`, derived at the epoch
    /// at the radius rank `rank` with `volatiles`, its inventory, atmosphere and tides read at its
    /// own bulk `bulk` (P14.T56.c's follow-up, P14.T56.d).
    fn own_moon(
        mass: (f64, UnitUniform),
        orbit: &KeplerElements,
        giant: &(MoonParent, MoonNursery, MoonSky),
        volatiles: &MoonVolatiles,
        bulk: MoonBulk,
    ) -> DerivedMoon {
        let ((kg, rank), (parent, nursery, sky)) = (mass, giant);
        let inputs = MoonInputs {
            mass: EarthMasses::new(kg / crate::units::consts::EARTH_MASS_KG),
            orbit: *orbit,
            radius_rank: rank,
            volatiles: *volatiles,
            bulk,
        };
        derive_moon(
            &inputs,
            parent,
            nursery,
            sky,
            SOLAR_AGE,
            UniverseTime::EPOCH,
        )
        .unwrap()
    }

    /// The table's moon `(kg, km, a_km, e)` about the giant `giant`: its mass and orbit, and the
    /// radius rank at which [`derive_body`] keeps its
    /// measured mean radius `km` in the giant's nursery, the table's convention
    /// ([`solar::rank_for`]; `table_bodies`, "each at the rank that keeps its radius").
    fn table_moon(
        (kg, km, a_km, e): (f64, f64, f64, f64),
        giant: &(MoonParent, MoonNursery, MoonSky),
    ) -> ((f64, UnitUniform), KeplerElements) {
        let orbit = about(&giant.0, a_km, e);
        let a_au = orbit.semi_major_axis().value() / crate::units::consts::METRES_PER_AU;
        let rank = solar::rank_for(&giant.1.disc, kg, km, a_au, e);
        ((kg, UnitUniform::new(rank).unwrap()), orbit)
    }

    /// A moon's nitrogen, carbon (as CO₂) and water per unit of its mass over Earth's.
    fn per_earth(moon: &DerivedMoon, kg: f64) -> (f64, f64, f64) {
        use crate::planetary::derive::atmosphere::{
            EARTH_CARBON_DIOXIDE_PER_MASS, EARTH_NITROGEN_PER_MASS, EARTH_WATER_PER_MASS,
        };
        let inventory = moon.inventory();
        (
            inventory.nitrogen().value() / (kg * EARTH_NITROGEN_PER_MASS),
            inventory.carbon_dioxide().value() / (kg * EARTH_CARBON_DIOXIDE_PER_MASS),
            inventory.water().value() / (kg * EARTH_WATER_PER_MASS),
        )
    }

    /// Ganymede's mass (kg), mean radius (km), semi-major axis (km) and eccentricity: JPL's
    /// "Planetary Satellite Physical Parameters" GM (JUP365) over G and mean radius (Archinal et
    /// al. 2018), and its "Planetary Satellite Mean Elements" (JUP365, to the local Laplace
    /// plane), both read 2026-10-15.
    const GANYMEDE: (f64, f64, f64, f64) = (
        9_887.832_75e9 / GRAVITATIONAL_CONSTANT,
        2_631.20,
        1_070_400.0,
        0.001,
    );
    /// Callisto's (the same tables, JUP365).
    const CALLISTO: (f64, f64, f64, f64) = (
        7_179.283_40e9 / GRAVITATIONAL_CONSTANT,
        2_410.30,
        1_882_700.0,
        0.007,
    );
    /// Titan's (the same tables, SAT441).
    const TITAN: (f64, f64, f64, f64) = (
        8_978.137_10e9 / GRAVITATIONAL_CONSTANT,
        2_574.76,
        1_221_900.0,
        0.029,
    );
    /// Io's (the same tables, JUP365, as `hooks::figures`'s tests' `MOONS` read them on
    /// 2026-10-12).
    const IO: (f64, f64, f64, f64) = (
        5_959.915_47e9 / GRAVITATIONAL_CONSTANT,
        1_821.49,
        421_800.0,
        0.004,
    );
    /// Europa's (the same tables, JUP365).
    const EUROPA: (f64, f64, f64, f64) = (
        3_202.712_10e9 / GRAVITATIONAL_CONSTANT,
        1_560.80,
        671_100.0,
        0.009,
    );
    /// Triton's (the same tables, NEP097, the mean elements read 2026-10-16), on its circular
    /// orbit.
    const TRITON: (f64, f64, f64, f64) = (
        1_428.495_46e9 / GRAVITATIONAL_CONSTANT,
        1_352.6,
        354_800.0,
        0.0,
    );

    /// The radius ranks the table's moons are read at (P14.T56.c's follow-up): the one at which
    /// [`derive_body`] keeps each one's measured radius
    /// (`keep`, the table's convention), the median and the quartiles, and the range's two ends.
    fn every_rank(keep: UnitUniform) -> [UnitUniform; 6] {
        let rank = |u: f64| UnitUniform::new(u).unwrap();
        [
            keep,
            rank(0.001),
            rank(0.25),
            rank(0.5),
            rank(0.75),
            rank(0.999),
        ]
    }

    /// A capture's own bulk, as its record states it (`PlanetarySystem::moon_bulk_of`): its mean
    /// radius `km` at its mass `kg`, with the ice its density holds over ruling 83.7's rock at
    /// [`MOON_ROCK_DENSITY`] and ice at [`MOON_ICE_DENSITY`].
    fn captured_bulk(kg: f64, km: f64) -> (MoonBulk, f64) {
        let r = km * 1e3;
        let density = kg / (4.0 / 3.0 * core::f64::consts::PI * r * r * r);
        let (rock, ice) = (MOON_ROCK_DENSITY.value(), MOON_ICE_DENSITY.value());
        let ice = ((1.0 / density - 1.0 / rock) / (1.0 / ice - 1.0 / rock)).clamp(0.0, 1.0);
        let fractions = MassFractions::solid(0.0, 1.0 - ice, ice);
        let bulk = MoonBulk::Stated {
            radius: Metres::new(r),
            fractions,
        };
        (bulk, ice)
    }

    /// P14.T56.c: about the generated Jupiter the circumplanetary ammonia line lies at
    /// (170 ÷ 123)² = 1.910 ice lines, 22.7 Jupiter radii (decision-ammonia-line), between
    /// Ganymede, which at 15.0 radii formed at about 151.6 K and takes Earth's nitrogen per unit
    /// mass, and Callisto, which at 26.3 formed at about 114.3 K and takes a hundred times it.
    /// The ruling's temperatures are at an ice line of 11.9 radii as rounded, so they hold within
    /// the 0.35 K that its last digit spans (the generated line is 11.86 radii). Both, formed
    /// beyond the ice line, take carbon's boost.
    #[test]
    fn the_generated_jupiter_s_ammonia_line_lies_between_ganymede_and_callisto() {
        use crate::planetary::derive::atmosphere::{
            ammonia_line, formation_temperature, nitrogen_boosted,
        };
        let jupiter = table_giant("Jupiter", 5, None);
        let line = jupiter
            .1
            .ice_line()
            .expect("Jupiter formed beyond the snow line");
        // In Jupiter's nominal equatorial radius, as decision-ammonia-line states the line.
        let radii = |d: Metres| d.value() / crate::units::consts::JUPITER_RADIUS_M;
        let ammonia = ammonia_line(line);
        assert!((ammonia.value() / line.value() - 1.910).abs() < 5e-4);
        assert!((radii(line) - 11.9).abs() < 0.05, "{} R_J", radii(line));
        assert!(
            (radii(ammonia) - 22.7).abs() < 0.05,
            "{} R_J",
            radii(ammonia)
        );
        let shared = MoonVolatiles::SHARED;
        for (moon, t_f, boost) in [(GANYMEDE, 151.6, 1.0), (CALLISTO, 114.3, 100.0)] {
            let (mass, orbit) = table_moon(moon, &jupiter);
            let (kg, a_km) = (moon.0, moon.2);
            let formed = formation_temperature(orbit.semi_major_axis(), line).value();
            assert!((formed - t_f).abs() < 0.35, "{a_km} km: {formed} K");
            assert_eq!(nitrogen_boosted(Kelvin::new(formed)), boost > 1.0);
            let derived = own_moon(mass, &orbit, &jupiter, &shared, MoonBulk::Derived);
            let (n, c, _) = per_earth(&derived, kg);
            assert!(
                (n / boost - 1.0).abs() < 1e-12,
                "{a_km} km: {n}× Earth's nitrogen"
            );
            assert!((c / 100.0 - 1.0).abs() < 1e-12, "{c}");
        }
    }

    /// P14.T56.c: the table's Titan, at the median volatile ranks about the generated Saturn,
    /// formed at or below 123 K, so it keeps the nitrogen boost and at least a bar of N₂; that
    /// needs the generated Saturn's ice line within (123 ÷ 170)² of Titan's distance, 10.6 of
    /// Saturn's 60,268 km equatorial radii (decision-ammonia-line; NASA's Saturn fact sheet). The
    /// generated line lies at 4.5 of them, so Titan formed near 80 K. Its air is read at its own
    /// bulk (P14.T56.c's follow-up, in force since P14.T54.a deleted the moons' hold).
    #[test]
    fn the_table_s_titan_forms_below_123_k_and_keeps_a_bar_of_nitrogen() {
        use crate::planetary::derive::atmosphere::{
            AMMONIA_HYDRATE_CONDENSATION, Gas, formation_temperature,
        };
        let saturn = table_giant("Saturn", 6, None);
        let line = saturn
            .1
            .ice_line()
            .expect("Saturn formed beyond the snow line");
        let (mass, orbit) = table_moon(TITAN, &saturn);
        let formed = formation_temperature(orbit.semi_major_axis(), line);
        assert!(
            formed <= AMMONIA_HYDRATE_CONDENSATION,
            "Titan formed at {} K, the ice line at {} equatorial radii",
            formed.value(),
            line.value() / 6.0268e7
        );
        let (shared, own) = (MoonVolatiles::SHARED, MoonBulk::Derived);
        let titan = own_moon(mass, &orbit, &saturn, &shared, own);
        let nitrogen = titan.atmosphere().partial_pressures().of(Gas::Nitrogen);
        assert!(nitrogen.value() >= 1e5, "{nitrogen:?}");
    }

    /// P14.T56.c and its follow-up: the table's Ganymede and Callisto stay airless at every radius
    /// rank, read at their own bulk, and their passes converge: Jeans escape removes what nitrogen
    /// and methane they hold (decision-p14-moons §2.1, Ganymede's λ near the threshold).
    ///
    /// Read at the composition solve's bulk, version 22's, they were airless only near the rank
    /// that keeps their measured radii. At the median rank that solve gives Ganymede 2,281 km
    /// against its own 2,653 km, so it held 3.6 bar (2.8 of N₂, 0.8 of CH₄) before the ammonia line
    /// and 0.31 bar, mostly CH₄, under it; at the lowest ranks Ganymede held 9.4 bar (0.39 under
    /// the line) and Callisto 7.4. Read at their own radii, 2,574–2,731 km and 2,387–2,540 km
    /// across the ranks, Ganymede's N₂ is at λ 22.3–23.6 and Callisto's at 17.4–18.5 at their
    /// 548 K exobase, under the threshold of 25, whatever state a pass reads.
    ///
    /// The exobase reads the irradiation, not the albedo a pass reads (decision-p14-t54a-ganymede
    /// §1.4). Read at the pass's albedo, it read P14.T51.b's frost, brighter than T13.c's ice: the
    /// passes left Ganymede a 133–166 mbar snowball of N₂ and CH₄ at every rank, and Callisto a
    /// bracketed jump at λ = 25 on three.
    #[test]
    fn the_table_s_ganymede_and_callisto_stay_airless() {
        use crate::planetary::derive::atmosphere::{
            Gas, JEANS_RETENTION_THRESHOLD, SurfaceState, jeans_parameter,
        };
        use crate::planetary::derive::passes::Convergence;
        let jupiter = table_giant("Jupiter", 5, None);
        for moon in [GANYMEDE, CALLISTO] {
            let ((kg, keep), orbit) = table_moon(moon, &jupiter);
            for rank in every_rank(keep) {
                let derived = own_moon(
                    (kg, rank),
                    &orbit,
                    &jupiter,
                    &MoonVolatiles::SHARED,
                    MoonBulk::Derived,
                );
                let air = derived.atmosphere();
                let lambda = jeans_parameter(
                    Kilograms::new(kg),
                    derived.radius(),
                    Gas::Nitrogen,
                    air.exobase_temperature(),
                );
                let at = format!(
                    "{} km at {rank:?}: λ(N₂) {lambda:.3} at {:.1} km, exobase {:.1} K, {air:?}",
                    moon.1,
                    derived.radius().value() / 1e3,
                    air.exobase_temperature().value()
                );
                eprintln!("{at}");
                assert_eq!(air.state(), SurfaceState::Airless, "{at}");
                assert_eq!(derived.convergence(), Convergence::Converged, "{at}");
                assert!(lambda < JEANS_RETENTION_THRESHOLD, "{at}");
            }
        }
    }

    /// Decision-p14-t54a-ganymede §1.4: over the fixture's radius ranks the window of
    /// [`EXOBASE_MULTIPLE`](crate::planetary::derive::atmosphere::EXOBASE_MULTIPLE) is 4.24–5.04
    /// times the irradiation temperature, and holds it: at every rank the table's Ganymede loses
    /// its N₂ and Titan keeps its own. The window's edges are the multiples at which Ganymede's
    /// most bound rank and Titan's least bound sit on the threshold.
    #[test]
    fn the_exobase_window_holds_over_the_fixture_s_ranks() {
        use crate::planetary::derive::atmosphere::{
            EXOBASE_MULTIPLE, Gas, JEANS_RETENTION_THRESHOLD, jeans_parameter,
        };
        let jupiter = table_giant("Jupiter", 5, None);
        let saturn = table_giant("Saturn", 6, None);
        let lambdas = |moon: (f64, f64, f64, f64), giant: &(MoonParent, MoonNursery, MoonSky)| {
            let ((kg, keep), orbit) = table_moon(moon, giant);
            every_rank(keep).map(|rank| {
                let shared = &MoonVolatiles::SHARED;
                let derived = own_moon((kg, rank), &orbit, giant, shared, MoonBulk::Derived);
                let air = derived.atmosphere();
                let exobase = air.exobase_temperature();
                let lambda =
                    jeans_parameter(Kilograms::new(kg), derived.radius(), Gas::Nitrogen, exobase);
                (lambda, air.retention().retains(Gas::Nitrogen))
            })
        };
        let (ganymede, titan) = (lambdas(GANYMEDE, &jupiter), lambdas(TITAN, &saturn));
        eprintln!("λ(N₂): Ganymede {ganymede:?}, Titan {titan:?}");
        assert!(
            ganymede
                .iter()
                .all(|&(lambda, kept)| !kept && lambda < 25.0)
        );
        assert!(titan.iter().all(|&(lambda, kept)| kept && lambda >= 25.0));
        let edge = |lambda: f64| EXOBASE_MULTIPLE * lambda / JEANS_RETENTION_THRESHOLD;
        let lower = edge(ganymede.iter().map(|&(l, _)| l).fold(0.0, f64::max));
        let upper = edge(titan.iter().map(|&(l, _)| l).fold(f64::INFINITY, f64::min));
        eprintln!("the window over the ranks: {lower:.4}–{upper:.4} T₀");
        assert!((lower - 4.24).abs() < 0.005 && (upper - 5.04).abs() < 0.005);
    }

    /// P14.T56.c's follow-up: every moon of the table, read at its own bulk at every radius rank
    /// with the median volatile ranks.
    ///
    /// - Its escape is judged at its own radius and mass: a regular moon's [`moon_radius`] at its
    ///   ice fraction, and Triton's, a capture's, its measured mean radius (JPL).
    /// - Its water floor is its own ice: the inventory holds the larger of the drawn water and its
    ///   bulk's.
    /// - Io and Europa, formed dry inside Jupiter's ice line, Ganymede, Callisto and Triton are
    ///   airless, under 1 mbar. Io's is the vapour of its CO₂ frost, some 7 Pa at 119 K;
    ///   Ganymede's and Callisto's are radiogenic argon, 1.2–17 Pa and up to 12 Pa across the ranks
    ///   (Callisto's argon sits at λ ≈ 25 and is lost at two of them), where Ganymede's measured
    ///   air is an O₂ and H₂O exosphere under 1 µPa (Hall et al. 1998, ApJ 499, 475; Roth et al.
    ///   2021, Nat. Astron. 5, 1043); Europa's holds nothing. Triton's measured N₂ is some 14 µbar
    ///   (1.4 Pa): Voyager 2's radio occultation as Gurrola 1995 (doctoral thesis, Stanford
    ///   University) re-analysed it, 14 ± 2 µbar as Marques Oliveira et al. 2022 (A&A 659, A136,
    ///   §1) quote it, where Tyler et al. 1989 (Science 246, 1466) gave a preliminary 1.6 ± 0.3 Pa.
    ///   That is thin and under the line too. At its own radius and the 228 K exobase of
    ///   [`EXOBASE_MULTIPLE`](crate::planetary::derive::atmosphere::EXOBASE_MULTIPLE), Triton's
    ///   N₂ fails Jeans's test at λ ≈ 15.6, but its ice holds it (decision-p14-t54a-ganymede, C0
    ///   follow-up, rule (b)): the ice's vapour at its equilibrium temperature, under a pascal, is
    ///   its N₂'s partial pressure. Its thin air is P14.T53.c's (the plan's recorded limitation,
    ///   "Pluto and Triton are `Airless` until T53.c").
    /// - The ice holds an ice crust's water and nitrogen where Jeans's test fails them, and no rock
    ///   bulk's: an Io keeps no water.
    /// - Titan keeps more than a bar of N₂ at every rank, and its air lies within 15% of
    ///   Huygens's 1,467 hPa and 2 K of its 93.65 K at the surface (HASI, Fulchignoni et al. 2005,
    ///   Nature 438, 785): 1,438 hPa at the median rank, of which N₂ weighs 1,387 and CH₄ 51
    ///   (about 6% by mole, against Huygens's 5.65 ± 0.18%, Niemann et al. 2010, JGR 115, E12006),
    ///   and 93.8 K, against the 2,655 hPa and 98.4 K version 22 gave it at the solve's 2,220 km.
    #[test]
    fn the_table_s_moons_take_their_air_at_their_own_bulk() {
        use crate::planetary::derive::atmosphere::{
            AIRLESS_PRESSURE, Gas, Retention, SurfaceState,
        };
        use crate::planetary::params::ICY_WATER_FRACTION;
        use crate::units::Pascals;
        let jupiter = table_giant("Jupiter", 5, None);
        let saturn = table_giant("Saturn", 6, None);
        let neptune = table_giant("Neptune", 8, None);
        for (name, moon, giant) in [
            ("Io", IO, &jupiter),
            ("Europa", EUROPA, &jupiter),
            ("Ganymede", GANYMEDE, &jupiter),
            ("Callisto", CALLISTO, &jupiter),
            ("Titan", TITAN, &saturn),
            ("Triton", TRITON, &neptune),
        ] {
            let ((kg, keep), orbit) = table_moon(moon, giant);
            let captured = name == "Triton";
            let (volatiles, stated) = if captured {
                let site = VolatileSite::ParentZone;
                let draws = VolatileDraws::MEDIAN;
                (
                    MoonVolatiles { draws, site },
                    Some(captured_bulk(kg, moon.1)),
                )
            } else {
                (MoonVolatiles::SHARED, None)
            };
            for rank in every_rank(keep) {
                let bulk = stated.map_or(MoonBulk::Derived, |(bulk, _)| bulk);
                let derived = own_moon((kg, rank), &orbit, giant, &volatiles, bulk);
                let air = derived.atmosphere();
                let at = format!("{name} at {rank:?}: {air:?}");
                let (radius, ice) = match bulk {
                    MoonBulk::Stated { radius, fractions } => (radius, fractions.water()),
                    MoonBulk::Derived => (derived.radius(), derived.ice_fraction()),
                };
                let placed =
                    Kilograms::from(EarthMasses::new(kg / crate::units::consts::EARTH_MASS_KG));
                let escape = Retention::of(placed, radius, air.exobase_temperature());
                let held = air.ice_held();
                assert_eq!(
                    air.retention(),
                    escape.with(held).with_methane_as_nitrogen(),
                    "{at}"
                );
                // The ice holds an ice crust's water and nitrogen alone, where Jeans's test fails
                // them, and nothing of a rock bulk's (decision-p14-t54a-ganymede, C0 follow-up).
                for gas in Gas::ALL {
                    if held.retains(gas) {
                        assert!(matches!(gas, Gas::Water | Gas::Nitrogen), "{at}");
                        assert!(!escape.retains(gas), "{at}");
                    }
                }
                if ice < ICY_WATER_FRACTION {
                    assert_eq!(held, Retention::default(), "{at}");
                }
                let water = derived.inventory().water().value();
                assert!(
                    water >= ice * kg * (1.0 - 1e-12),
                    "{at}: {water} kg of water"
                );
                let pressure = air.surface_pressure().unwrap();
                if name == "Titan" {
                    assert_eq!(air.state(), SurfaceState::Snowball, "{at}");
                    let nitrogen = air.partial_pressures().of(Gas::Nitrogen);
                    assert!(nitrogen.value() >= 1e5, "{at}");
                    assert!((pressure.value() / 1.467e5 - 1.0).abs() < 0.15, "{at}");
                    assert!(
                        (air.surface_temperature().value() - 93.65).abs() < 2.0,
                        "{at}"
                    );
                } else {
                    assert_eq!(air.state(), SurfaceState::Airless, "{at}");
                    assert!(pressure < AIRLESS_PRESSURE, "{at}");
                }
                // An Io keeps no water: its bulk is rock, which the ice-held rule does not reach,
                // and its water fails Jeans's test.
                if name == "Io" {
                    assert!(water > 0.0, "{at}");
                    assert!(!air.retention().retains(Gas::Water), "{at}");
                    assert_same_bits(air.inventory_weights().of(Gas::Water).value(), 0.0);
                }
                // Triton's ice holds its N₂, whose vapour stays far under the airless line.
                if captured {
                    assert!(held.retains(Gas::Nitrogen), "{at}");
                    let nitrogen = air.partial_pressures().of(Gas::Nitrogen);
                    assert!(
                        nitrogen > Pascals::ZERO && nitrogen < AIRLESS_PRESSURE,
                        "{at}"
                    );
                }
            }
        }
    }

    /// The condensate partition of the moon `derived`'s air over the passes' stand-in surface: an
    /// isothermal climate at its surface temperature, on basins not known, its condensed water all
    /// at the surface.
    fn passes_partition(derived: &DerivedMoon) -> crate::planetary::chem::partition::Partition {
        use crate::planetary::chem::partition::{Basins, ClimateSpan, Ground, partition};
        let air = derived.atmosphere();
        let climate = ClimateSpan::isothermal(air.surface_temperature());
        let ground = Ground::new(derived.body().surface_gravity(), Basins::UNKNOWN, 1.0);
        partition(&climate, &ground, &air.column_inventory())
    }

    /// Decision-p14-t54a-ganymede, C0 follow-up, rule (b): at every radius rank the measured
    /// Ganymede, Callisto and Titan, ice crusts whose water fails Jeans's test at the exobase
    /// (λ(H₂O) = 0.643 λ(N₂), under 20 on all three), hold their water as ice: their ice's vapour
    /// at their equilibrium temperatures, 74.2–78.2 K, is some 10⁻²⁴–10⁻²² Pa, so it loses under
    /// 10⁻²⁴ kg over the age, the air's column holds the whole inventory and the partition lists it
    /// as a solid. Ganymede's and Callisto's N₂ stays lost: its saturated vapour at their 78.2 K is
    /// 1.1 bar (N₂ boils at 77.36 K under one atmosphere, Span et al. 2000), a bulk air, which
    /// Jeans's test judges and loses (§1.4 of the ruling stands), where Titan keeps its own by that
    /// test.
    #[test]
    fn the_measured_icy_moons_hold_their_water_ice_and_ganymede_s_nitrogen_stays_lost() {
        use crate::planetary::chem::partition::CondensatePhase;
        use crate::planetary::derive::atmosphere::{
            AIRLESS_PRESSURE, Gas, Retention, ice_vapour_escape,
        };
        use crate::substance::saturation_pressure;
        let jupiter = table_giant("Jupiter", 5, None);
        let saturn = table_giant("Saturn", 6, None);
        for (name, moon, giant) in [
            ("Ganymede", GANYMEDE, &jupiter),
            ("Callisto", CALLISTO, &jupiter),
            ("Titan", TITAN, &saturn),
        ] {
            let ((kg, keep), orbit) = table_moon(moon, giant);
            for rank in every_rank(keep) {
                let shared = &MoonVolatiles::SHARED;
                let derived = own_moon((kg, rank), &orbit, giant, shared, MoonBulk::Derived);
                let air = derived.atmosphere();
                let t = derived.equilibrium_temperature();
                let placed =
                    Kilograms::from(EarthMasses::new(kg / crate::units::consts::EARTH_MASS_KG));
                let escape = Retention::of(placed, derived.radius(), air.exobase_temperature());
                let water = derived.inventory().water();
                let loss = ice_vapour_escape(placed, derived.radius(), Gas::Water, t, SOLAR_AGE)
                    .expect("water ice's vapour at an icy moon's temperature is no air");
                let partition = passes_partition(&derived);
                let ice = partition.condensate(Gas::Water.substance());
                let nitrogen = saturation_pressure(Gas::Nitrogen.substance(), t).unwrap();
                let at = format!(
                    "{name} at {rank:?}, T_eq {:.2} K: water's loss {:.3e} of {:.3e} kg, N₂'s \
                     vapour {:.3e} Pa, water {ice:?}",
                    t.value(),
                    loss.value(),
                    water.value(),
                    nitrogen.value()
                );
                eprintln!("{at}");
                assert!(!escape.retains(Gas::Water), "{at}");
                assert!(air.ice_held().retains(Gas::Water), "{at}");
                assert!(air.retention().retains(Gas::Water), "{at}");
                assert!(loss.value() < 1e-12 * water.value(), "{at}");
                assert!(air.inventory_weights().of(Gas::Water).value() > 0.0, "{at}");
                let ice = ice.unwrap_or_else(|| panic!("{at}: no water condensate"));
                assert_eq!(ice.phase(), CondensatePhase::Solid, "{at}");
                if name == "Titan" {
                    assert!(escape.retains(Gas::Nitrogen), "{at}");
                    assert!(!air.ice_held().retains(Gas::Nitrogen), "{at}");
                } else {
                    assert!(nitrogen > AIRLESS_PRESSURE, "{at}");
                    assert!(!air.retention().retains(Gas::Nitrogen), "{at}");
                    assert_same_bits(air.inventory_weights().of(Gas::Nitrogen).value(), 0.0);
                    assert_same_bits(air.partial_pressures().of(Gas::Nitrogen).value(), 0.0);
                }
            }
        }
    }

    /// The figures of the moon `derived` of `kg`, under its spin `spin`, at its own bulk (its
    /// radius, gravity, class, fractions and water, as its record reads them), about the present
    /// Sun at its age with no tide; its crater contract a planet's on a circular orbit at 1 au
    /// under the Solar System's sources, as `table_figures` gives the table's bodies theirs.
    fn own_figures(
        derived: &DerivedMoon,
        kg: f64,
        spin: &crate::planetary::hooks::surface::SeasonalSpin,
    ) -> crate::planetary::hooks::figures::GlobalFigures {
        use crate::planetary::hooks::figures::relief::EARTH_CONTINENTAL_FRACTION;
        use crate::planetary::hooks::figures::tests::solar_sources;
        use crate::planetary::hooks::figures::{
            Continents, CraterInputs, FiguresInputs, GlobalFigures, HostOrbit, HostOrbiter,
            ImpactorOrbit,
        };
        let r = derived.radius().value();
        let mass = EarthMasses::new(kg / crate::units::consts::EARTH_MASS_KG);
        GlobalFigures::derive(&FiguresInputs {
            atmosphere: derived.atmosphere(),
            mass,
            radius: derived.radius(),
            surface_gravity: MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * kg / (r * r)),
            class: derived.class(),
            fractions: derived.fractions(),
            inventory_water: derived.inventory().water(),
            rotation_period: spin.rotation_period,
            fe_h: Dex::new(0.0),
            age: SOLAR_AGE,
            tidal_heat_flow: WattsPerSquareMetre::new(0.0),
            subsurface_ocean: Subsurface::NotComputed,
            craters: CraterInputs {
                orbit: ImpactorOrbit::Host(HostOrbiter {
                    orbit: HostOrbit {
                        semi_major_axis: Metres::new(crate::units::consts::METRES_PER_AU),
                        eccentricity: 0.0,
                        inclination: Radians::ZERO,
                    },
                    mass,
                    belt: None,
                }),
                density: derived.density(),
                sources: solar_sources(),
                molten_until: None,
                last_giant_impact: None,
            },
            continents: Continents::Given(EARTH_CONTINENTAL_FRACTION),
        })
        .expect("the measured moons are in range")
        .expect("an icy moon has a surface")
    }

    /// Decision-p14-t54a-ganymede, C1 follow-up §C1.4, the record's side of rule (b): on the
    /// record's path (the moon's figures and conditions at its own bulk, and `condensates_under`),
    /// the measured Ganymede, Callisto and Titan, at every radius rank, ice bulks whose ice holds
    /// the water Jeans's test fails, publish an `H2O` solid entry that is the partition's whole
    /// frozen water, its layer at ice Ih's density at the mean surface temperature, over the
    /// figures' T24.b ice area to the bit, which the figures' ices list as it. Their surface water,
    /// the water free to pool, stays on water's Jeans verdict (P14.T13.b): none, so that Titan's
    /// surface water, potential ocean and ocean weight are C0's (7015ccf8), zero to the bit, and no
    /// water sea is listed. Titan's crust is not a sea.
    #[test]
    fn the_measured_icy_moons_publish_their_crust_s_water_at_the_figures_ice_area() {
        use crate::planetary::chem::partition::CondensatePhase;
        use crate::planetary::derive::atmosphere::{Gas, SurfaceMaterial};
        use crate::planetary::hooks::cold_traps::{ColdTrapRules, RecordColdTraps, lunar_sky};
        use crate::planetary::hooks::figures::{VolatileHistory, cover};
        use crate::planetary::hooks::surface::tests::spin_of;
        use crate::planetary::hooks::surface::{SurfaceConditions, SurfaceInputs};
        let water = Gas::Water.substance();
        let sky = lunar_sky();
        let jupiter = table_giant("Jupiter", 5, None);
        let saturn = table_giant("Saturn", 6, None);
        for (name, moon, giant) in [
            ("Ganymede", GANYMEDE, &jupiter),
            ("Callisto", CALLISTO, &jupiter),
            ("Titan", TITAN, &saturn),
        ] {
            let ((kg, keep), orbit) = table_moon(moon, giant);
            let spin = spin_of(name);
            for rank in every_rank(keep) {
                let shared = &MoonVolatiles::SHARED;
                let derived = own_moon((kg, rank), &orbit, giant, shared, MoonBulk::Derived);
                let air = derived.atmosphere();
                let figures = own_figures(&derived, kg, &spin);
                let r = derived.radius().value();
                let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * kg / (r * r));
                let inputs = SurfaceInputs {
                    atmosphere: air,
                    equilibrium_temperature: derived.equilibrium_temperature(),
                    surface_gravity: gravity,
                    radius: derived.radius(),
                    material: SurfaceMaterial::of(&derived.fractions()),
                    stellar_flux: WattsPerSquareMetre::from(derived.flux()),
                    xuv_flux: derived.xuv_flux(),
                    spin,
                    heat_flow: figures.heat_flow(),
                    ocean_weight: figures.ocean_weight(),
                };
                assert_eq!(inputs.material, SurfaceMaterial::Ice, "{name} at {rank:?}");
                let conditions = SurfaceConditions::derive(&inputs).expect("it has a surface");
                let table = conditions.zonal_insolation();
                let traps = RecordColdTraps::at_the_albedo_of(ColdTrapRules::Held, &sky, air);
                let (partition, published) =
                    figures.condensates_under(air, &conditions, gravity, &traps, &table);
                let ice = cover::ice_fraction(&cover::IceInputs {
                    conditions: &conditions,
                    state: air.state(),
                    crust: figures.impactors().crust(),
                    surface_water: figures.surface_water(),
                });
                let at = format!(
                    "{name} at {rank:?}, a {:?} at {:.2} K: T24.b's ice {ice}, {:?}",
                    air.state(),
                    conditions.mean_surface_temperature().value(),
                    published.entries()
                );
                eprintln!("{at}");
                // The ice holds the water Jeans's test fails.
                assert!(air.retention().retains(Gas::Water), "{at}");
                assert!(!air.retains_against_escape(Gas::Water), "{at}");
                let frozen = partition
                    .condensate(water)
                    .unwrap_or_else(|| panic!("{at}: the partition holds the crust's water"));
                assert_eq!(frozen.phase(), CondensatePhase::Solid, "{at}");
                let entry = published
                    .entry(water, CondensatePhase::Solid)
                    .unwrap_or_else(|| panic!("{at}: no H2O solid entry"));
                assert_same_bits(entry.area_fraction(), ice);
                assert_same_bits(entry.reservoir().value(), frozen.reservoir().value());
                let rho = crate::substance::ice_ih_density(conditions.mean_surface_temperature());
                let layer = entry.reservoir_gel().expect("ice Ih's density is read");
                assert_same_bits(layer.value(), frozen.reservoir().value() / rho.value());
                assert!(
                    published.entry(water, CondensatePhase::Liquid).is_none(),
                    "{at}"
                );
                // T24.b's surface water stays on water's Jeans verdict: C0's none.
                assert_same_bits(figures.surface_water().value(), 0.0);
                assert_same_bits(figures.potential_ocean(), 0.0);
                assert_same_bits(figures.ocean_weight().get(), 0.0);
                // The figures' ices list it as the cover's ice, divided with the others where they
                // exceed the surface (W2).
                let total = published
                    .surface_ices()
                    .iter()
                    .fold(0.0, |sum, &(_, f)| sum + f);
                let second = figures.with_surface_under(
                    &conditions,
                    VolatileHistory::NONE,
                    Some(&published),
                    &table,
                );
                let &(_, listed) = second
                    .surface_ices()
                    .iter()
                    .find(|&&(id, _)| id == water)
                    .unwrap_or_else(|| panic!("{at}: its ices list its water"));
                assert_same_bits(listed, if total > 1.0 { ice / total } else { ice });
                assert!(
                    second.surface_liquids().iter().all(|&(id, _)| id != water),
                    "{at}: {:?}",
                    second.surface_liquids()
                );
            }
        }
    }

    /// P14.T56.c's follow-up, the pure check: a regular moon's own bulk, [`MoonBulk::Derived`], is
    /// its [`DerivedMoon::radius`] and [`DerivedMoon::fractions`] exactly, so that its inventory,
    /// atmosphere and tides are those of a moon stated at them, bit for bit.
    #[test]
    fn a_regular_moon_s_own_bulk_is_its_radius_and_fractions() {
        let jupiter = table_giant("Jupiter", 5, None);
        let saturn = table_giant("Saturn", 6, None);
        for (moon, giant) in [
            (IO, &jupiter),
            (GANYMEDE, &jupiter),
            (CALLISTO, &jupiter),
            (TITAN, &saturn),
        ] {
            let ((kg, keep), orbit) = table_moon(moon, giant);
            for rank in every_rank(keep) {
                let mass = (kg, rank);
                let shared = &MoonVolatiles::SHARED;
                let own = own_moon(mass, &orbit, giant, shared, MoonBulk::Derived);
                let stated = MoonBulk::Stated {
                    radius: own.radius(),
                    fractions: own.fractions(),
                };
                assert_eq!(own_moon(mass, &orbit, giant, shared, stated), own);
            }
        }
    }

    /// P14.T56.c's follow-up: a stated bulk whose radius is not positive and finite is refused
    /// with its own error, rather than read as an airless moon.
    #[test]
    fn a_stated_radius_that_is_not_a_radius_is_refused() {
        let jupiter = table_giant("Jupiter", 5, None);
        let ((kg, keep), orbit) = table_moon(GANYMEDE, &jupiter);
        let fractions = MassFractions::solid(0.0, 0.6, 0.4);
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let bulk = MoonBulk::Stated {
                radius: Metres::new(bad),
                fractions,
            };
            let inputs = MoonInputs {
                mass: EarthMasses::new(kg / crate::units::consts::EARTH_MASS_KG),
                orbit,
                radius_rank: keep,
                volatiles: MoonVolatiles::SHARED,
                bulk,
            };
            let refused = derive_moon(
                &inputs,
                &jupiter.0,
                &jupiter.1,
                &jupiter.2,
                SOLAR_AGE,
                UniverseTime::EPOCH,
            )
            .unwrap_err();
            assert_eq!(refused, DeriveMoonError::StatedRadius, "{bad}");
            let text = refused.to_string();
            assert_eq!(text, text.to_lowercase());
            assert!(refused.source().is_none());
        }
    }

    /// P14.T56.c's follow-up: a Triton-like capture inside the generated Jupiter's circumplanetary
    /// ice line (T56.c's case, at 10 Jupiter radii) takes carbon's and nitrogen's boosts from
    /// Jupiter's zone, and its water from its own icy bulk, Triton's measured radius and the ice
    /// its density holds (0.24), where a moon of its mass formed in the nursery there, dry, holds
    /// the drawn inventory's water.
    #[test]
    fn a_capture_s_water_floor_is_its_own_ice() {
        use crate::planetary::derive::atmosphere::EARTH_WATER_PER_MASS;
        let home = table_giant("Jupiter", 5, None);
        let (kg, km) = (TRITON.0, TRITON.1);
        let orbit = about(&home.0, 714_920.0, 0.0);
        assert!(home.1.ice_line().unwrap() > orbit.semi_major_axis());
        let volatiles = MoonVolatiles {
            draws: VolatileDraws::MEDIAN,
            site: VolatileSite::ParentZone,
        };
        let (bulk, ice) = captured_bulk(kg, km);
        assert!((ice - 0.238).abs() < 0.005, "{ice}");
        let mass = (kg, UnitUniform::HALF);
        let own = own_moon(mass, &orbit, &home, &volatiles, bulk);
        let dry = own_moon(mass, &orbit, &home, &volatiles, MoonBulk::Derived);
        assert_same_bits(dry.ice_fraction(), 0.0);
        let water = own.inventory().water().value();
        assert!((water / (ice * kg) - 1.0).abs() < 1e-12, "{water} kg");
        let drawn = dry.inventory().water().value() / (kg * EARTH_WATER_PER_MASS);
        assert!((drawn - 1.0).abs() < 1e-12, "{drawn}× Earth's water");
        assert_eq!(own.inventory().nitrogen(), dry.inventory().nitrogen());
        assert_eq!(
            own.inventory().carbon_dioxide(),
            dry.inventory().carbon_dioxide()
        );
    }

    /// P14.T56.c: a captured moon's volatiles are those of a body formed at its parent's formation
    /// distance in the star's disc, the ammonia line's rule included, read at its own bulk, which
    /// does not move with them.
    ///
    /// - Himalia (JPL's GM, JUP344, and ecliptic mean elements, JUP347: 11,439,000 km, e = 0.160,
    ///   of which only a and e are read) about a Jupiter formed at 3 au, beyond the snow line and
    ///   inside the ammonia line, keeps its icy carbon but loses the nitrogen boost the cold outer
    ///   nursery gave it.
    /// - A Triton-like capture of Triton's mass (JPL's GM, NEP097) at 10 Jupiter radii, inside
    ///   the generated Jupiter's circumplanetary ice line, formed in the nursery would be dry of
    ///   icy carbon and nitrogen; from Jupiter's own zone, at 5.2 au and 112 K, it takes both
    ///   boosts.
    #[test]
    fn a_captured_moon_takes_the_volatiles_of_its_parent_s_zone() {
        use crate::planetary::derive::atmosphere::volatile_inventory;
        let himalia = (0.151_55e9 / GRAVITATIONAL_CONSTANT, 11_439_000.0, 0.160);
        let triton = (1_428.495_46e9 / GRAVITATIONAL_CONSTANT, 714_920.0, 0.0);
        let at_3_au = table_giant("Jupiter", 5, Some(3.0));
        let home = table_giant("Jupiter", 5, None);
        assert!(home.1.ice_line().unwrap().value() > triton.1 * 1e3);
        let draws = VolatileDraws {
            nitrogen: UnitUniform::new(0.7).unwrap(),
            carbon: UnitUniform::new(0.2).unwrap(),
            ..VolatileDraws::MEDIAN
        };
        let site = |site| MoonVolatiles { draws, site };
        for ((kg, a_km, e), giant, ratios) in [
            (himalia, &at_3_au, (0.01, 1.0)),
            (triton, &home, (100.0, 100.0)),
        ] {
            let orbit = about(&giant.0, a_km, e);
            let mass = (kg, UnitUniform::HALF);
            let derived = MoonBulk::Derived;
            let nursery = own_moon(mass, &orbit, giant, &site(VolatileSite::Nursery), derived);
            let zone = own_moon(
                mass,
                &orbit,
                giant,
                &site(VolatileSite::ParentZone),
                derived,
            );
            let (n, c, _) = per_earth(&zone, kg);
            let (n_nursery, c_nursery, _) = per_earth(&nursery, kg);
            assert!(
                (n / n_nursery / ratios.0 - 1.0).abs() < 1e-12,
                "{a_km} km: {n} {n_nursery}"
            );
            assert!(
                (c / c_nursery / ratios.1 - 1.0).abs() < 1e-12,
                "{a_km} km: {c} {c_nursery}"
            );
            // The inventory is the parent's zone's, at the moon's own mass and fractions.
            let parent_zone = giant.1.parent_zone();
            let expected = volatile_inventory(
                zone.body.mass(),
                &zone.fractions(),
                parent_zone.side(),
                parent_zone.temperature(),
                &draws,
                SOLAR_AGE,
            );
            assert_eq!(*zone.inventory(), expected, "{a_km} km");
            // Its bulk does not move.
            assert_same_bits(zone.radius().value(), nursery.radius().value());
            assert_same_bits(zone.density().value(), nursery.density().value());
            assert_same_bits(zone.ice_fraction(), nursery.ice_fraction());
            assert_eq!(zone.fractions(), nursery.fractions());
        }
    }

    /// P14.T56.c: a giant-impact moon takes its parent's zone at every distance, so its nitrogen
    /// does not step as it recedes. Its parent, half an Earth mass formed at 3 au, between the
    /// snow and ammonia lines, gives it carbon's boost and not nitrogen's; in the parent's tiny
    /// nursery it formed far below 123 K at either distance and took both.
    #[test]
    fn a_giant_impact_moon_takes_its_parent_s_zone_as_it_recedes() {
        use crate::units::consts::{EARTH_MASS_KG, METRES_PER_AU};
        let disc = solar::solar_disc();
        let (kg, a_au) = (0.5 * EARTH_MASS_KG, 3.0);
        let derived = solar::derive(&solar::placed(kg, a_au, 0.02, 0.5), &disc).unwrap();
        let orbit = solar::orbit(a_au, 0.02);
        let host = Kilograms::new(SOLAR_MASS_KG);
        let parent =
            MoonParent::from_derived(planet_id(0, 3), ParentKind::Planet, &derived, orbit, host)
                .unwrap();
        let lights = [solar::sun()];
        let hosts = BodyHosts::new(host, Composition::SOLAR, &lights, &[]).unwrap();
        let sky = MoonSky::new(&derived, &orbit, &hosts).unwrap();
        let nursery = MoonNursery::new(
            &parent,
            &Composition::SOLAR,
            &disc,
            Metres::new(a_au * METRES_PER_AU),
        )
        .unwrap();
        let rocky = (parent, nursery, sky);
        let moon_kg = (0.012 * kg, UnitUniform::HALF);
        let r = rocky.0.radius().value() / 1e3;
        let mut steps = Vec::new();
        for radii in [8.0, 40.0] {
            let orbit = about(&rocky.0, radii * r, 0.0);
            let draws = VolatileDraws::MEDIAN;
            let site = |site| MoonVolatiles { draws, site };
            let derived = MoonBulk::Derived;
            let zone = own_moon(
                moon_kg,
                &orbit,
                &rocky,
                &site(VolatileSite::ParentZone),
                derived,
            );
            let nursery = own_moon(
                moon_kg,
                &orbit,
                &rocky,
                &site(VolatileSite::Nursery),
                derived,
            );
            let (n, c, _) = per_earth(&zone, moon_kg.0);
            assert!(
                (n - 1.0).abs() < 1e-12 && (c / 100.0 - 1.0).abs() < 1e-12,
                "{n} {c}"
            );
            let (n_nursery, ..) = per_earth(&nursery, moon_kg.0);
            assert!(
                (n_nursery / 100.0 - 1.0).abs() < 1e-12,
                "{radii} radii: {n_nursery}"
            );
            steps.push(n);
        }
        assert_same_bits(steps[0], steps[1]);
    }

    /// P14.T56.c: a nursery holds where its planet formed in the star's disc, refuses a
    /// formation distance that is not a place with its own error, and has an ice line exactly
    /// where its planet formed at or beyond the star's snow line, as before.
    #[test]
    fn a_nursery_holds_where_its_planet_formed() {
        use crate::planetary::derive::BuildFormationZoneError;
        let disc = solar::solar_disc();
        let (jupiter, _) = derived_parent("Jupiter", planet_id(0, 5));
        let nursery =
            |d: f64| MoonNursery::new(&jupiter, &Composition::SOLAR, &disc, Metres::new(d));
        for bad in [f64::NAN, 0.0, -1.0, f64::INFINITY] {
            let refused = nursery(bad).unwrap_err();
            assert_eq!(
                refused,
                DeriveMoonError::ParentZone(BuildFormationZoneError::DistanceNotPositive)
            );
            assert!(refused.source().is_some());
            let text = refused.to_string();
            assert_eq!(text, text.to_lowercase());
        }
        let line = disc.snow_line().value();
        let at = nursery(line).unwrap();
        assert!(at.ice_line().is_some());
        assert_same_bits(at.parent_zone().distance().value(), line);
        assert_same_bits(at.parent_zone().snow_line().value(), line);
        assert_eq!(nursery(line * (1.0 - 1e-12)).unwrap().ice_line(), None);
    }

    // P14.T56.d: moons' Love numbers and dissipation by state.

    /// A spread of radius ranks for the `i`th moon of a sample: the golden ratio's sequence.
    fn spread_rank(i: u32) -> UnitUniform {
        let x = (0.5 + f64::from(i) * 0.618_033_988_749_895).fract();
        UnitUniform::new(x.clamp(1e-6, 1.0 - 1e-6)).unwrap()
    }

    /// The regular moon `moon` of the giant `giant`'s system `p`, derived at the radius rank
    /// `rank` with the median volatiles in a system aged `age`.
    fn regular_derived(
        moon: &RegularMoon,
        p: &MoonParent,
        (_, nursery, sky): &(MoonParent, MoonNursery, MoonSky),
        rank: UnitUniform,
        age: Years,
    ) -> DerivedMoon {
        let inputs = MoonInputs {
            mass: moon.mass(),
            orbit: *moon.orbit(),
            radius_rank: rank,
            volatiles: MoonVolatiles::SHARED,
            bulk: MoonBulk::Derived,
        };
        derive_moon(&inputs, p, nursery, sky, age, UniverseTime::EPOCH).unwrap()
    }

    /// The table's Jupiter, Saturn, Uranus and Neptune as their moons read them: the giants of
    /// the tide checks, which each test renames over its own systems.
    fn table_giants() -> [(MoonParent, MoonNursery, MoonSky); 4] {
        [
            table_giant("Jupiter", 5, None),
            table_giant("Saturn", 6, None),
            table_giant("Uranus", 7, None),
            table_giant("Neptune", 8, None),
        ]
    }

    /// P14.T56.d: a regular moon's heat is its own loss's, its flag its branch's and its locking
    /// its own pair's, at its own radius; its published ocean is the very one its flag was decided
    /// on.
    #[test]
    fn a_regular_moon_s_heat_flag_and_locking_are_its_own_tides_s() {
        let jupiter = table_giant("Jupiter", 5, None);
        let mut moons = 0;
        for i in 0..40 {
            let p = renamed(&jupiter.0, i, 5);
            for moon in regular_moons(Seed::new(6), &p).moons() {
                let rank = spread_rank(i);
                let (a, e) = (
                    moon.orbit().semi_major_axis(),
                    moon.orbit().eccentricity().value(),
                );
                let n = core::f64::consts::TAU / moon.orbit().period().value();
                let planet = Kilograms::from(p.mass());
                let own = regular_derived(moon, &p, &jupiter, rank, SOLAR_AGE);
                let tides = own.tides();
                let ocean = matches!(own.subsurface(), Subsurface::Ocean(_));
                assert_eq!(ocean, own.subsurface_ocean());
                let heat = tidal_heating(planet, own.radius(), tides.loss(), a, e, n);
                assert_same_bits(own.tidal_heating().value(), heat.value());
                assert_eq!(own.subsurface_ocean(), tides.branch() == TidalBranch::Ocean);
                assert_eq!(own.spin_tides(), tides.spin_tides());
                let fractions = own.fractions();
                let lock = locking_time(
                    planet,
                    Kilograms::from(moon.mass()),
                    own.radius(),
                    a,
                    PlanetClass::of(moon.mass(), &fractions),
                    &fractions,
                    tides.spin_tides(),
                );
                assert_same_bits(own.locking_time().value(), lock.value());
                moons += 1;
            }
        }
        assert!(moons > 100, "{moons}");
    }

    /// The heat flux, W m⁻², that version 22 gave the regular moon `moon` of `p` at the radius
    /// `radius`: Io's response [`MOON_TIDAL_RESPONSE`] for every moon, over that radius's sphere,
    /// as its tides spread their own.
    fn io_response_flux(moon: &RegularMoon, p: &MoonParent, radius: Metres) -> f64 {
        let orbit = moon.orbit();
        let motion = core::f64::consts::TAU / orbit.period().value();
        let (axis, e) = (orbit.semi_major_axis(), orbit.eccentricity().value());
        let planet = Kilograms::from(p.mass());
        let heat = tidal_heating(planet, radius, MOON_TIDAL_RESPONSE, axis, e, motion);
        let metres = radius.value();
        heat.value() / (4.0 * core::f64::consts::PI * metres * metres)
    }

    /// P14.T56.d: a cold rocky moon's heat flux is k₂ ÷ (38 × 0.015) of version 22's, which read
    /// Io's response [`MOON_TIDAL_RESPONSE`] for every moon at the same radius; a heated one's is
    /// version 22's, bit for bit (generated moons of the table's Jupiter inside its ice line).
    #[test]
    fn a_cold_rocky_moon_s_heat_is_its_loss_over_io_s_and_a_heated_one_s_is_unchanged() {
        use crate::planetary::moons::tides::TidalBranch;
        use crate::planetary::params::COLD_ROCK_TIDAL_Q;
        let jupiter = table_giant("Jupiter", 5, None);
        let (mut cold, mut heated) = (0, 0);
        let (mut least, mut most) = (f64::INFINITY, 0.0_f64);
        for i in 0..300 {
            let p = renamed(&jupiter.0, i, 5);
            for moon in regular_moons(Seed::new(6), &p).moons() {
                let rank = spread_rank(i);
                let own = regular_derived(moon, &p, &jupiter, rank, SOLAR_AGE);
                if own.class() != PlanetClass::Rocky {
                    continue;
                }
                let v22 = io_response_flux(moon, &p, own.radius());
                let tides = own.tides();
                match tides.branch() {
                    TidalBranch::Cold => {
                        let ratio = tides.k2() / (COLD_ROCK_TIDAL_Q * MOON_TIDAL_RESPONSE);
                        let moved = own.heat_flux().value() / v22;
                        assert!((moved / ratio - 1.0).abs() < 1e-12, "{moved} {ratio}");
                        assert!(ratio > 0.0 && ratio < 0.1, "{ratio}");
                        (least, most) = (least.min(ratio), most.max(ratio));
                        cold += 1;
                    }
                    TidalBranch::Heated => {
                        assert_same_bits(own.heat_flux().value(), v22);
                        heated += 1;
                    }
                    TidalBranch::Ocean => panic!("a dry moon holds no ocean"),
                }
            }
        }
        eprintln!(
            "{cold} cold and {heated} heated rocky moons; the cold ones' heat × {least:.4}–{most:.4} \
             of version 22's"
        );
        assert!(cold > 20 && heated > 20, "{cold} cold, {heated} heated");
    }

    /// P14.T56.d: a regular moon's ocean is its branch's, and an ocean once frozen never returns
    /// in time: over 0.5–13.5 Gyr of its system's age, a moon's flag never goes from ocean to none
    /// and back (generated moons of the table's giants, their skies held at the epoch's).
    #[test]
    fn an_ocean_once_frozen_never_returns() {
        let ages: Vec<Years> = (0..=52)
            .map(|k| Years::new(0.5e9 + 0.25e9 * f64::from(k)))
            .collect();
        let (mut moons, mut frozen, mut always) = (0, 0, 0);
        for giant in &table_giants() {
            let slot = giant.0.id().body_index().to_be_bytes()[0];
            for i in 0..25 {
                let p = renamed(&giant.0, i, slot);
                for moon in regular_moons(Seed::new(11), &p).moons() {
                    let rank = spread_rank(i);
                    let flags: Vec<bool> = ages
                        .iter()
                        .map(|&age| regular_derived(moon, &p, giant, rank, age).subsurface_ocean())
                        .collect();
                    let lost = flags.windows(2).position(|w| w[0] && !w[1]);
                    if let Some(at) = lost {
                        assert!(
                            !flags[at + 1..].contains(&true),
                            "{:?} moon {}: {flags:?}",
                            p.id(),
                            moon.ordinal()
                        );
                        frozen += 1;
                    }
                    always += usize::from(flags.iter().all(|&ocean| ocean));
                    moons += 1;
                }
            }
        }
        eprintln!("{moons} moons: {always} keep an ocean throughout, {frozen} freeze once");
        assert!(moons > 200, "{moons}");
    }

    /// P14.T56.d: over 10⁴ generated moons every moon's tides are finite and positive, every
    /// ocean-branch heat flow lies under its small-Λ root, and the largest of them, those at 1 W m⁻²
    /// or more, the shares of the three branches and of the thin-shell branch are recorded, with no
    /// bound in W m⁻² (decision-p14-t56d-membrane §7.1: physical for a held eccentricity; its limit,
    /// the eccentricity's damping and a resonance's budget, is P14.T55.j's).
    #[test]
    fn the_largest_ocean_branch_flux_of_ten_thousand_generated_moons_is_recorded() {
        use crate::planetary::moons::tides::TidalBranch;
        let mut counts = [0_u32; 3];
        let mut largest: Option<(f64, f64, f64)> = None;
        let (mut moons, mut past_one, mut beyond) = (0_u32, Vec::new(), Vec::new());
        'giants: for i in 0..2_000 {
            for giant in &table_giants() {
                let slot = giant.0.id().body_index().to_be_bytes()[0];
                let p = renamed(&giant.0, i, slot);
                for moon in regular_moons(Seed::new(12), &p).moons() {
                    let own = regular_derived(moon, &p, giant, spread_rank(moons), SOLAR_AGE);
                    let tides = own.tides();
                    assert!(tides.k2().is_finite() && tides.k2() > 0.0, "{tides:?}");
                    assert!(tides.q().is_finite() && tides.q() > 0.0, "{tides:?}");
                    assert!(own.locking_time().value().is_finite());
                    counts[match tides.branch() {
                        TidalBranch::Cold => 0,
                        TidalBranch::Ocean => 1,
                        TidalBranch::Heated => 2,
                    }] += 1;
                    if tides.branch() == TidalBranch::Ocean {
                        let f = tides.heat_flow().value();
                        let state = ocean_state(moon, &p, &own);
                        // Every ocean-branch flux is finite and under its small-Λ root
                        // (decision-p14-t56d-membrane §7.1), and none is bounded in W m⁻²: a moon
                        // at 1 W m⁻² or more is recorded (P14.T55.j's eccentricity damping).
                        assert!(f.is_finite(), "{:?}: {f} W m⁻²", p.id());
                        assert!(f <= state.small * (1.0 + 1e-12), "{:?}: {f}", p.id());
                        let orbit = moon.orbit();
                        let e = orbit.eccentricity().value();
                        if state.stiff && state.gain >= 1.0 {
                            past_one.push(e);
                        }
                        let km = own.radius().value() / 1e3;
                        if f >= 1.0 {
                            let parent_radii = orbit.semi_major_axis().value() / p.radius().value();
                            beyond.push((f, km, e, parent_radii));
                        }
                        if largest.is_none_or(|(most, ..)| f > most) {
                            largest = Some((f, km, e));
                        }
                    }
                    moons += 1;
                    if moons >= 10_000 {
                        break 'giants;
                    }
                }
            }
        }
        past_one.sort_by(f64::total_cmp);
        eprintln!(
            "{moons} moons: {} cold, {} on the ocean branch, {} heated; the largest ocean-branch \
             heat flow {largest:?} (W m⁻², km, e); {} on the thin-shell branch, stiff with X ≥ 1 \
             ({:.4} of the ocean branch), at e {:?}–{:?}",
            counts[0],
            counts[1],
            counts[2],
            past_one.len(),
            f64::from(u32::try_from(past_one.len()).unwrap()) / f64::from(counts[1].max(1)),
            past_one.first(),
            past_one.last()
        );
        eprintln!(
            "{} ocean-branch moons at 1 W m⁻² or more (W m⁻², km, e, parent radii): {beyond:?}",
            beyond.len()
        );
        assert_eq!(moons, 10_000);
        assert!(counts.iter().all(|&n| n > 0), "{counts:?}");
    }

    /// What [`the_largest_ocean_branch_flux_of_ten_thousand_generated_moons_is_recorded`] reads
    /// of an ocean-branch moon: its stiff shell's gain X, whether its shell is stiff at its rock's
    /// heat alone, h°|K| ÷ `F_nt` > 1 (`F_nt` its whole heat less its tide), and its small-Λ root.
    struct OceanState {
        gain: f64,
        stiff: bool,
        small: f64,
    }

    /// [`OceanState`] of the regular moon `moon` of `p`, derived under its own tides as `own`.
    fn ocean_state(moon: &RegularMoon, p: &MoonParent, own: &DerivedMoon) -> OceanState {
        use crate::planetary::moons::tides::{
            ocean_love_number, shell_modulus, shell_stiffness, stiff_shell_gain, tidal_forcing,
        };
        use crate::planetary::params::ICE_POISSON_RATIO;
        let tides = own.tides();
        let orbit = moon.orbit();
        let radius = own.radius();
        let motion = core::f64::consts::TAU / orbit.period().value();
        let surface = own.equilibrium_temperature();
        let gravity = crate::units::MetresPerSecondSquared::new(
            GM_EARTH * moon.mass().value() / (radius.value() * radius.value()),
        );
        let stiffness = shell_stiffness(
            shell_modulus(surface, motion),
            ICE_POISSON_RATIO,
            surface,
            gravity,
            radius,
        );
        let forcing = tidal_forcing(
            Kilograms::from(p.mass()),
            radius,
            orbit.semi_major_axis(),
            orbit.eccentricity().value(),
            motion,
        );
        let ocean = ocean_love_number(own.density());
        let non_tidal = tides.heat_flow().value() - forcing.value() * tides.loss();
        let shell = 1.0 + ocean;
        let size = crate::math::hypot(stiffness.real.value(), stiffness.imaginary.value());
        let drive = forcing.value() * ocean * shell * stiffness.imaginary.value();
        OceanState {
            gain: stiff_shell_gain(forcing, ocean, stiffness),
            stiff: shell * size > non_tidal,
            small: f64::midpoint(non_tidal, (non_tidal * non_tidal + 4.0 * drive).sqrt()),
        }
    }

    /// P14.T17.b's test (b) as amended, its second half (decision-p14-moons §3.4 (a)): the share
    /// of generated regular moons unlocked at the epoch under their own tides is recorded; version
    /// 22's class pair locked them all.
    #[test]
    fn the_share_of_generated_regular_moons_unlocked_at_the_epoch_is_recorded() {
        let seconds = SOLAR_AGE.value() * crate::units::consts::SECONDS_PER_JULIAN_YEAR;
        let (mut moons, mut unlocked) = (0_u32, 0_u32);
        for giant in &table_giants() {
            let slot = giant.0.id().body_index().to_be_bytes()[0];
            for i in 0..150 {
                let p = renamed(&giant.0, i, slot);
                for moon in regular_moons(Seed::new(13), &p).moons() {
                    let rank = spread_rank(moons);
                    let own = regular_derived(moon, &p, giant, rank, SOLAR_AGE);
                    unlocked += u32::from(own.locking_time().value() > seconds);
                    moons += 1;
                }
            }
        }
        let share = f64::from(unlocked) / f64::from(moons);
        eprintln!(
            "{unlocked} of {moons} generated regular moons unlocked at the Sun's age ({share:.4})"
        );
        assert!(moons > 1_000, "{moons}");
        assert!(share < 0.5, "{share}");
    }

    /// P14.T56.c's follow-up's finding, built by P14.T56.d: a capture's tides read its own bulk,
    /// its record's, not the nursery's radius. The table's Triton, about the table's Neptune at
    /// its own 1,352.6 km and the ice its density holds, against the nursery's 1,474 km.
    #[test]
    fn a_capture_s_tides_read_its_own_bulk() {
        let neptune = table_giant("Neptune", 8, None);
        let ((kg, rank), orbit) = table_moon(TRITON, &neptune);
        let (bulk, ice) = captured_bulk(kg, TRITON.1);
        let volatiles = MoonVolatiles {
            draws: VolatileDraws::MEDIAN,
            site: VolatileSite::ParentZone,
        };
        let own = own_moon((kg, rank), &orbit, &neptune, &volatiles, bulk);
        // Its derivation's radius, which version 22's tides read, is the nursery's, some 1,474 km,
        // not its own.
        assert!(
            (own.radius().value() - 1.3526e6).abs() > 5e4,
            "{:?}",
            own.radius()
        );
        let tides = own.tides();
        let fractions = MassFractions::solid(0.0, 1.0 - ice, ice);
        let class = PlanetClass::of(
            EarthMasses::new(kg / crate::units::consts::EARTH_MASS_KG),
            &fractions,
        );
        let mass = Kilograms::from(EarthMasses::new(kg / crate::units::consts::EARTH_MASS_KG));
        let lock = locking_time(
            Kilograms::from(neptune.0.mass()),
            mass,
            Metres::new(TRITON.1 * 1e3),
            orbit.semi_major_axis(),
            class,
            &fractions,
            tides.spin_tides(),
        );
        assert_same_bits(own.locking_time().value(), lock.value());
        // On its circular orbit it feels no eccentricity tide.
        assert_same_bits(own.tidal_heating().value(), 0.0);
        eprintln!(
            "Triton: {:?}, k2 {:.4}, Q {:.1}, F {:.2} mW m⁻², locks in {:.3e} yr",
            tides.branch(),
            tides.k2(),
            tides.q(),
            1e3 * tides.heat_flow().value(),
            own.locking_time().value() / crate::units::consts::SECONDS_PER_JULIAN_YEAR
        );
    }

    /// P14.T51.b: a capture whose density makes it icy
    /// (Triton's mass and measured radius about the generated Jupiter, 0.24 of ice) and a
    /// giant-impact moon of a rocky planet (a sphere of the Moon's 3,344 kg m⁻³ of rock about half
    /// an Earth at 3 au) take the albedo, and so the equilibrium temperature, of their bulk
    /// section's material, which their air reads (`MoonBulk::Stated`): the capture its fresh frost
    /// (with its ices), above 0.5, and the giant-impact moon's rock, its lid not known, the Moon's
    /// 0.11 by the space-weathering stand-in.
    #[test]
    fn a_capture_and_a_giant_impact_moon_take_their_bulk_s_albedo() {
        use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
        use crate::planetary::derive::equilibrium_temperature;
        use crate::planetary::derive::photometry::area_mix::{Regolith, sun_bands};
        use crate::units::consts::{EARTH_MASS_KG, METRES_PER_AU};
        let volatiles = MoonVolatiles {
            draws: VolatileDraws::MEDIAN,
            site: VolatileSite::ParentZone,
        };
        let home = table_giant("Jupiter", 5, None);
        let (kg, km) = (TRITON.0, TRITON.1);
        let orbit = about(&home.0, 714_920.0, 0.0);
        let (captured, ice) = captured_bulk(kg, km);
        assert!(ice >= crate::planetary::params::ICY_WATER_FRACTION, "{ice}");
        let mass = (kg, UnitUniform::HALF);
        let capture = own_moon(mass, &orbit, &home, &volatiles, captured);
        assert!(capture.albedo().value() > 0.5, "{:?}", capture.albedo());
        let disc = solar::solar_disc();
        let (parent_kg, a_au) = (0.5 * EARTH_MASS_KG, 3.0);
        let derived = solar::derive(&solar::placed(parent_kg, a_au, 0.02, 0.5), &disc).unwrap();
        let parent_orbit = solar::orbit(a_au, 0.02);
        let host = Kilograms::new(SOLAR_MASS_KG);
        let parent = MoonParent::from_derived(
            planet_id(0, 3),
            ParentKind::Planet,
            &derived,
            parent_orbit,
            host,
        )
        .unwrap();
        assert_eq!(parent.class(), PlanetClass::Rocky);
        let lights = [solar::sun()];
        let hosts = BodyHosts::new(host, Composition::SOLAR, &lights, &[]).unwrap();
        let sky = MoonSky::new(&derived, &parent_orbit, &hosts).unwrap();
        let nursery = MoonNursery::new(
            &parent,
            &Composition::SOLAR,
            &disc,
            Metres::new(a_au * METRES_PER_AU),
        )
        .unwrap();
        let rocky = (parent, nursery, sky);
        let moon_kg = 0.012 * parent_kg;
        let density = 3_344.0;
        let radius = math::cbrt(3.0 * moon_kg / (4.0 * core::f64::consts::PI * density));
        let impact = MoonBulk::Stated {
            radius: Metres::new(radius),
            fractions: MassFractions::solid(0.0, 1.0, 0.0),
        };
        let orbit = about(&rocky.0, 10.0 * rocky.0.radius().value() / 1e3, 0.0);
        let mass = (moon_kg, UnitUniform::HALF);
        let moon = own_moon(mass, &orbit, &rocky, &volatiles, impact);
        let rock = Regolith::of_material(SurfaceMaterial::Rock)
            .bond(SurfaceState::Airless, &sun_bands())
            .bond;
        assert_eq!(moon.atmosphere().state(), SurfaceState::Airless);
        assert_same_bits(moon.albedo().value(), rock);
        for derived in [&capture, &moon] {
            assert_same_bits(
                derived.equilibrium_temperature().value(),
                equilibrium_temperature(derived.flux(), derived.albedo()).value(),
            );
        }
        eprintln!(
            "a Triton-like capture {:.5} at {:.2} K; a giant-impact moon of rock {:.5} at {:.2} K",
            capture.albedo().value(),
            capture.equilibrium_temperature().value(),
            moon.albedo().value(),
            moon.equilibrium_temperature().value(),
        );
    }

    /// The table's Triton at its own inventory, about the table's Neptune, N₂-covered: its N₂ fails
    /// Jeans's test at the exobase (λ ≈ 15.6) but its ice holds it (decision-p14-t54a-ganymede, C0
    /// follow-up, rule (b): its vapour at Triton's equilibrium temperature is a few hundredths of a
    /// pascal at λ ≈ 110 at the surface), so its partition lists N₂ ice, with the methane that
    /// follows it, and it stays airless, its thin air P14.T53.c's. Its Bond albedo lies in E2's
    /// band, 0.72–0.92 (Hillier et al. 1990, Science 250, 419: 0.82 ± 0.05; decision-p14-t51-batch
    /// E2), and its passes converge within the plain passes (`Converged`, P14.T51.h).
    #[test]
    fn the_table_s_triton_holds_its_nitrogen_ice_and_stays_airless() {
        use crate::planetary::chem::partition::CondensatePhase;
        use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
        let volatiles = MoonVolatiles {
            draws: VolatileDraws::MEDIAN,
            site: VolatileSite::ParentZone,
        };
        let neptune = table_giant("Neptune", 8, None);
        let ((kg, rank), orbit) = table_moon(TRITON, &neptune);
        let (own, _) = captured_bulk(kg, TRITON.1);
        let triton = own_moon((kg, rank), &orbit, &neptune, &volatiles, own);
        let air = triton.atmosphere();
        let partition = passes_partition(&triton);
        let listed: Vec<_> = partition
            .condensates()
            .iter()
            .map(|c| (c.substance().substance().key(), c.phase()))
            .collect();
        let albedo = triton.albedo().value();
        eprintln!(
            "the table's Triton at its own inventory: {:?}, Bond albedo {albedo:.5}, T_eq {:.2} \
             K, N2 {:.3e} Pa, its condensates {listed:?}",
            air.state(),
            triton.equilibrium_temperature().value(),
            air.partial_pressures().of(Gas::Nitrogen).value(),
        );
        assert_eq!(air.state(), SurfaceState::Airless);
        assert!(air.ice_held().retains(Gas::Nitrogen), "{air:?}");
        assert!(air.retention().retains(Gas::Methane), "{air:?}");
        let nitrogen = partition
            .condensate(Gas::Nitrogen.substance())
            .expect("Triton's ice holds its nitrogen");
        assert_eq!(nitrogen.phase(), CondensatePhase::Solid);
        assert!((0.72..=0.92).contains(&albedo), "{albedo}");
        assert_eq!(
            triton.convergence(),
            crate::planetary::derive::passes::Convergence::Converged,
            "{} passes",
            triton.atmosphere_passes()
        );
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
