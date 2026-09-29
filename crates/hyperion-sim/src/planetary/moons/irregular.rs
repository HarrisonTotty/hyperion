//! Captured moons: a giant's irregular population, an ice giant's Triton-like capture and a rocky
//! planet's small captures (plan 14, P14.T19).
//!
//! # A giant's population
//!
//! Every giant, ice or gas, captures a population of irregular moons (ruling 83.4). Its number
//! with diameters above 2.8 km is log-normal about a median of 90 with 0.25 dex of scatter
//! ([`POPULATION_MEDIAN`], [`POPULATION_SIGMA_DEX`]), whatever the giant and its belts, and is
//! recorded as that count with its size law ([`IrregularPopulation`]): its largest member's
//! diameter D₁ log-uniform in 150–350 km ([`LARGEST_DIAMETER`]), N(> D) = D₁ ÷ D (q = 2) down to a
//! break of 2.8–10 km, and a differential q = 3.5 below it ([`SIZE_SLOPE`]). Its largest members,
//! up to four, become bodies: the first is D₁ across, and by Rényi's representation of a power
//! law's order statistics the next sit where the law's count is 1 + Γₖ₋₁, Γ a running sum of
//! standard exponentials, down to 10 km across ([`BODY_MIN_RADIUS`]).
//!
//! Each captured body's orbit is referred to the parent's orbital plane: semi-major axis uniform in
//! 0.1–0.45 Hill radii ([`ORBIT_HILL_RADII`]), eccentricity uniform in 0.1–0.6
//! ([`ECCENTRICITY`]), retrograde with probability 0.8 ([`RETROGRADE_PROBABILITY`]), and an
//! inclination isotropic outside the Kozai gap of 60–120° ([`KOZAI_GAP`]). The axis is then held
//! so that it lies inside the stability limit of P14.T15 for the body's sense and eccentricity,
//! and its pericentre outside the parent's radius and fluid Roche limit; a body for which no axis
//! of 0.1 Hill radii or more fits is not kept. Domingos, Winter and Yokoyama's (2006, MNRAS 373,
//! 1227, §2) limit is a critical semi-major axis, fitted on the satellite's initial axis and
//! eccentricity, so the axis is tested against it: the fit's `e_sat` term already holds the
//! eccentricity, and an apocentre test would count it twice (ruling 133.2). Their grid stops at
//! `e_sat` = 0.5, so captures of 0.5–0.6 extrapolate its `e_sat` term, recorded.
//!
//! **Irregulars survive by orbit** (ruling 112.4). They are captured early, into the Hill sphere
//! the giant had where it formed, and none is captured after it migrates (Jewitt and Haghighipour
//! 2007: irregulars are "confined to the central 50% of their planets Hill spheres"; Spalding,
//! Batygin and Adams 2016: a Jupiter now at 1 AU "could lose moons if it formed beyond 5AU"). So
//! the capture orbits are drawn in Hill radii of the formation distance, and the count keeps the
//! share of that distribution that still fits the present limits ([`surviving_capture_share`]),
//! against the share that fitted where the giant formed, so that a giant that never moved keeps
//! ruling 83.4's count, which is the survivors'. A hot giant, whose present Hill sphere is a few
//! of its radii, keeps none.
//!
//! # An ice giant's large capture
//!
//! An ice giant has a Triton-like capture with probability 0.2 ([`LARGE_CAPTURE_PROBABILITY`]): a
//! moon of 10⁻⁴–10⁻³ of its planet's mass, log-uniform ([`LARGE_CAPTURE_MASS_RATIO`]), retrograde,
//! circularised close in at 10–20 planetary radii ([`LARGE_CAPTURE_RADII`]; Triton at 14.3). It
//! removes the planet's regular moons beyond its orbit, and those within 2√3 mutual Hill radii of
//! it ([`Captures::prune`]).
//!
//! # A rocky planet's captures
//!
//! A rocky planet next to a belt has one or two kilometre-scale captured moons with probability
//! 0.2 ([`ROCKY_CAPTURE_PROBABILITY`]), of 1–15 km radius ([`ROCKY_CAPTURE_RADIUS`]), on the capture
//! orbits above.
//! Those orbits are the lane's reading, provisional: Phobos and Deimos, whose origin is debated,
//! are at 0.003–0.007 Hill radii on nearly circular, nearly equatorial orbits.
//!
//! # Inputs
//!
//! The nearest belt comes in as a plain value, [`NearestBelt`]: its mass and whether the planet is
//! next to it, as P14.T22.a passes the belts' masses. Only a rocky planet's captures read it
//! (ruling 83.4). Nothing here reads P14.T21's belts.
//!
//! # Draws
//!
//! On the parent's own [`tags::MOON_CAPTURE`], whose word layout its doc comment gives.

use crate::Seed;
use crate::math;
use crate::orbit::KeplerElements;
use crate::planetary::derive::{OrbitSense, PlanetClass};
use crate::planetary::moons::regular::RegularMoons;
use crate::planetary::moons::{MoonParent, decide, draw_rank, log_uniform, moon_orbit};
use crate::planetary::params::HILL_STABLE_GAP;
use crate::planetary::placement::spacing::mutual_hill_radius;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::consts::EARTH_MASS_KG;
use crate::units::{Degrees, EarthMasses, KilogramsPerCubicMetre, Metres, Radians, SolarMasses};

/// The median count of a giant's irregular moons above [`POPULATION_MIN_DIAMETER`]: 90 (ruling
/// 83.4).
///
/// Ashton et al. (2021, PSJ 2, 158) put Saturn's at 150 ± 30 above 2.8 km across, and Jupiter's
/// is about 50; Jewitt and Haghighipour (2007, ARA&A 45, 261, §3) find the four giants'
/// populations "similar", so the count does not scale with the giant or its belts.
pub const POPULATION_MEDIAN: f64 = 90.0;

/// The scatter of log₁₀ of the count: 0.25 dex (ruling 83.4), spanning Jupiter's 50 to Saturn's
/// 150 in one σ.
pub const POPULATION_SIGMA_DEX: f64 = 0.25;

/// The diameter above which the population is counted: 2.8 km (Ashton et al. 2021).
pub const POPULATION_MIN_DIAMETER: Metres = Metres::new(2.8e3);

/// The cumulative size slope s of an irregular population, N(> D) ∝ D^−s: 2.5, the differential
/// index q = 3.5 of ruling 83.4 (Ashton et al. 2021 at Saturn; Jewitt and Haghighipour 2007, §4,
/// find it steepening to about 3.5 below 5 km at Jupiter).
pub const SIZE_SLOPE: f64 = 2.5;

/// The range of a giant's largest irregular's diameter D₁, log-uniform: 150–350 km (ruling 83.4,
/// as amended; JPL: Himalia 170, Phoebe 213, Nereid 340 km). It replaces P14.T19's 10–250 km.
pub const LARGEST_DIAMETER: (Metres, Metres) = (Metres::new(150e3), Metres::new(350e3));

/// The range to which the size law's break is clamped: 2.8–10 km (ruling 83.4, as amended).
pub const BREAK_DIAMETER: (Metres, Metres) = (Metres::new(2.8e3), Metres::new(10e3));

/// The least radius of a captured body a giant's population records as a body: 5 km, 10 km across
/// (P14.T19); its largest is D₁ ([`LARGEST_DIAMETER`]).
pub const BODY_MIN_RADIUS: Metres = Metres::new(5e3);

/// The most captured bodies recorded per giant: 4 (P14.T19).
pub const MAX_BODIES: u8 = 4;

/// The semi-major axes of captured orbits, in the parent's Hill radius: 0.1–0.45, uniform
/// (P14.T19).
///
/// Jewitt and Haghighipour (2007, §3): irregular orbits reach about half the Hill radius, with
/// medians of 0.44, 0.29, 0.17 and 0.19 at Jupiter, Saturn, Uranus and Neptune.
pub const ORBIT_HILL_RADII: (f64, f64) = (0.1, 0.45);

/// The eccentricities of captured orbits, uniform: 0.1–0.6 (P14.T19). The known irregulars'
/// 10th–90th percentiles are 0.14–0.50 (JPL's mean elements).
pub const ECCENTRICITY: (f64, f64) = (0.1, 0.6);

/// The probability that a captured orbit is retrograde: 0.8 (ruling 83.2, replacing P14.T19's
/// "about two thirds").
///
/// Jewitt and Haghighipour (2007, Table 2) count 88 of 107 irregulars retrograde and Ashton et al.
/// (2025) 100 of 122 at Saturn (ruling 83.2). Ashton et al. (2022, PSJ 3, 107) argue that surveys
/// find retrograde moons more easily than prograde ones, so the share may be biased high.
pub const RETROGRADE_PROBABILITY: f64 = 0.8;

/// The inclinations captured orbits avoid: 60–120° (P14.T19's Kozai gap).
///
/// Jewitt and Haghighipour (2007) give 60–130° (§3) and 55–130° (§5); Nesvorný et al. (2003, AJ
/// 126, 398) 50–140°, where the Kozai effect and the Sun's perturbations empty the orbits, with a
/// few Neptunian exceptions (JPL: Halimede 119.6°, Psamathe 127.8°, Neso 128.4°), which sit inside
/// the wider band, so it is not widened (ruling 83.3).
pub const KOZAI_GAP: (Degrees, Degrees) = (Degrees::new(60.0), Degrees::new(120.0));

/// The probability of an ice giant's Triton-like capture: 0.2 (P14.T19).
///
/// A design choice, with no source (ruling 83.5); Agnor and Hamilton (2006, Nature 441, 192) give
/// the capture's mechanism, by exchange from a binary, not its rate.
pub const LARGE_CAPTURE_PROBABILITY: f64 = 0.2;

/// The mass of a Triton-like capture over its planet's, log-uniform: 10⁻⁴–10⁻³ (P14.T19).
/// Triton's is 2.09 × 10⁻⁴.
pub const LARGE_CAPTURE_MASS_RATIO: (f64, f64) = (1e-4, 1e-3);

/// Where a Triton-like capture ends up after tides circularise it: 10–20 planetary radii,
/// log-uniform. Triton is at 14.3 Neptune radii with e = 0.00002 and i = 157°. This lane's
/// choice for P14.T19's "circularised close in", provisional.
pub const LARGE_CAPTURE_RADII: (f64, f64) = (10.0, 20.0);

/// The probability that a rocky planet next to a belt has small captured moons: 0.2 (P14.T19).
pub const ROCKY_CAPTURE_PROBABILITY: f64 = 0.2;

/// The radii of a rocky planet's captured moons, log-uniform: 1–15 km, P14.T19's
/// "kilometre-scale". Phobos's mean radius is 11.1 km and Deimos's 6.2. This lane's choice.
pub const ROCKY_CAPTURE_RADIUS: (Metres, Metres) = (Metres::new(1e3), Metres::new(15e3));

/// The density of a captured body: 1,700 kg m⁻³ (Phoebe 1,638, Porco et al. 2005, Science 307,
/// 1237; Phobos 1,861 and Deimos 1,465, Rosenblatt 2011, A&A Rev. 19, 44).
pub const CAPTURED_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_700.0);

/// The first word of captured body k's block of [`tags::MOON_CAPTURE`], k from 1.
const BODY_WORDS_START: u64 = 32;

/// The words of one captured body's block.
const BODY_WORDS: u64 = 16;

/// The words of a large capture.
const LARGE_WORDS: u64 = 16;

/// Whether a planet lies next to its nearest belt, with no planet between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BeltAdjacency {
    /// No planet lies between the planet and the belt.
    Neighbouring,
    /// Another planet lies between them.
    Separated,
}

/// The belt nearest a planet, as the captures read it: its mass and whether the planet is next to
/// it (P14.T22.a passes these from P14.T21's belts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestBelt {
    mass: EarthMasses,
    adjacency: BeltAdjacency,
}

impl NearestBelt {
    /// A belt of mass `mass`, next to the planet or not.
    ///
    /// Returns `None` for a mass that is negative or not finite.
    #[must_use]
    pub fn new(mass: EarthMasses, adjacency: BeltAdjacency) -> Option<Self> {
        (mass.value().is_finite() && mass.value() >= 0.0).then_some(Self { mass, adjacency })
    }

    /// The belt's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// Whether the planet is next to it.
    #[must_use]
    pub const fn adjacency(&self) -> BeltAdjacency {
        self.adjacency
    }
}

/// A giant's irregular population: its count above [`POPULATION_MIN_DIAMETER`] and its broken
/// size law (ruling 83.4, as amended).
///
/// The cumulative count is N(> D) = D₁ ÷ D from the largest member's diameter D₁ down to the break
/// `D_b`, a differential q = 2 (Jewitt and Haghighipour 2007, §3; Ashton et al. 2021, after
/// Nicholson et al. 2008, for 20–200 km), and N(> D) = (D₁ ÷ `D_b`) (`D_b` ÷ D)^2.5, q = 3.5, below.
/// The break is `D_b` = (N × 2.8^2.5 ÷ D₁)^(2⁄3) km, which makes N(> 2.8 km) the drawn count,
/// clamped to 2.8–10 km ([`BREAK_DIAMETER`]); the fit is this plan's (ruling 83).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IrregularPopulation {
    count: u64,
    largest: Metres,
    break_diameter: Metres,
}

impl IrregularPopulation {
    /// The population of `count` members above [`POPULATION_MIN_DIAMETER`] whose largest is
    /// `largest` across.
    #[must_use]
    fn new(count: u64, largest: Metres) -> Self {
        let n = f64::from(u32::try_from(count).unwrap_or(u32::MAX));
        let km = |m: Metres| m.value() / 1e3;
        let min = km(POPULATION_MIN_DIAMETER);
        let raw = math::powf(n * math::powf(min, SIZE_SLOPE) / km(largest), 2.0 / 3.0);
        let (lo, hi) = (km(BREAK_DIAMETER.0), km(BREAK_DIAMETER.1));
        Self {
            count,
            largest,
            break_diameter: Metres::new(raw.clamp(lo, hi) * 1e3),
        }
    }

    /// How many members have diameters above [`POPULATION_MIN_DIAMETER`]: the drawn count.
    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// The largest member's diameter, D₁.
    #[must_use]
    pub const fn largest_diameter(&self) -> Metres {
        self.largest
    }

    /// The diameter of the break between the law's two slopes, `D_b`.
    #[must_use]
    pub const fn break_diameter(&self) -> Metres {
        self.break_diameter
    }

    /// The cumulative size slope s of N(> D) ∝ D^−s below the break: 2.5 ([`SIZE_SLOPE`]).
    #[must_use]
    pub const fn size_slope(&self) -> f64 {
        SIZE_SLOPE
    }

    /// The law's count of members wider than `diameter`: D₁ ÷ D above the break and the break's
    /// count times (`D_b` ÷ D)^2.5 below it. It equals [`count`](Self::count) at 2.8 km unless the
    /// break was clamped.
    #[must_use]
    pub fn count_above(&self, diameter: Metres) -> f64 {
        let (d1, db, d) = (
            self.largest.value(),
            self.break_diameter.value(),
            diameter.value(),
        );
        if d >= db {
            d1 / d
        } else {
            d1 / db * math::powf(db / d, SIZE_SLOPE)
        }
    }

    /// The diameter at which the law's count reaches `n` (at least 1): the inverse of
    /// [`count_above`](Self::count_above), D₁ at 1.
    #[must_use]
    pub fn diameter_at(&self, n: f64) -> Metres {
        let (d1, db) = (self.largest.value(), self.break_diameter.value());
        let at_break = d1 / db;
        Metres::new(if n <= at_break {
            d1 / n.max(1.0)
        } else {
            db * math::powf(at_break / n, 1.0 / SIZE_SLOPE)
        })
    }
}

/// What kind of capture a captured moon is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CaptureKind {
    /// One of a giant's largest irregulars.
    Irregular,
    /// An ice giant's Triton-like capture.
    Large,
    /// A rocky planet's kilometre-scale capture.
    Small,
}

/// A captured moon: its kind, ordinal, size and mass, and its orbit about its parent referred to
/// the parent's orbital plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapturedMoon {
    kind: CaptureKind,
    ordinal: u8,
    radius: Metres,
    mass: EarthMasses,
    sense: OrbitSense,
    orbit: KeplerElements,
}

impl CapturedMoon {
    /// What kind of capture it is.
    #[must_use]
    pub const fn kind(&self) -> CaptureKind {
        self.kind
    }

    /// Its candidate number within its kind, from 1, by which it draws: a large capture is 1, and
    /// a giant's irregular or a rocky planet's small capture is the number k of the block it drew
    /// from (words 32 + 16 (k − 1) onwards), kept or not the candidates before it were (ruling
    /// 112.5). With its [`kind`](Self::kind) it is the capture's identity, which its satellite's
    /// own draws are keyed by.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }

    /// Its radius.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }

    /// Its mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// Whether it goes round its parent prograde or retrograde.
    #[must_use]
    pub const fn sense(&self) -> OrbitSense {
        self.sense
    }

    /// Its orbit about its parent, referred to the parent's orbital plane: an inclination over 90°
    /// is retrograde.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }
}

/// A planet's captures ([`captures`]): its irregular population, if it is a giant,
/// and its captured bodies.
#[derive(Debug, Clone, PartialEq)]
pub struct Captures {
    population: Option<IrregularPopulation>,
    moons: Vec<CapturedMoon>,
}

impl Captures {
    /// No captures.
    pub const NONE: Self = Self {
        population: None,
        moons: Vec::new(),
    };

    /// The giant's irregular population, or `None` for a planet that is not a giant.
    #[must_use]
    pub const fn population(&self) -> Option<&IrregularPopulation> {
        self.population.as_ref()
    }

    /// The captured bodies.
    #[must_use]
    pub fn moons(&self) -> &[CapturedMoon] {
        &self.moons
    }

    /// The large, Triton-like capture, if there is one.
    #[must_use]
    pub fn large(&self) -> Option<&CapturedMoon> {
        self.moons
            .iter()
            .find(|moon| moon.kind == CaptureKind::Large)
    }

    /// The regular moons `regular` of `parent` that survive these captures: all of them, or with
    /// a large capture those inside its orbit and clear of it (P14.T19): a regular moon is kept
    /// when its apocentre lies at least 2√3 mutual Hill radii about the planet inside the
    /// capture's pericentre, design note 7's gap ([`HILL_STABLE_GAP`]), so that the moons that
    /// remain never cross the capture.
    #[must_use]
    pub fn prune(&self, parent: &MoonParent, regular: &RegularMoons) -> RegularMoons {
        let Some(large) = self.large() else {
            return regular.clone();
        };
        let host = SolarMasses::from(parent.mass());
        let (a_c, pericentre) = (large.orbit.semi_major_axis(), large.orbit.periapsis());
        regular.retaining(|moon| {
            let a = moon.orbit().semi_major_axis();
            let hill = mutual_hill_radius(moon.mass(), large.mass, host, a, a_c);
            a < a_c && pericentre - moon.orbit().apoapsis() >= hill * HILL_STABLE_GAP
        })
    }
}

/// The captured moons of `parent` in the universe of `seed`, beside its nearest belt `belt`
/// (P14.T19; see the [module](self) documentation).
///
/// A giant, ice or gas, has its population and largest members, and an ice giant may have a
/// Triton-like capture; a rocky planet next to a belt may have small captures; every other parent
/// has [`Captures::NONE`].
///
/// # Examples
///
/// Neptune beside a belt of the asteroid belt's mass:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{CellSize, GenCell};
/// use hyperion_sim::id::{Layer, SystemId};
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::derive::{OrbitSense, PlanetClass};
/// use hyperion_sim::planetary::moons::{
///     BeltAdjacency, MoonParent, MoonParentParts, NearestBelt, ParentKind, captures,
/// };
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// use hyperion_sim::units::{EarthMasses, GravitationalParameter, Kilograms, Metres, Radians, SolarMasses};
///
/// let system = SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, [1, 2, 0])?, 3)?;
/// let id = BodyIndex::new(BodySlot::Planet(8), BodySub::Primary)?.body_id(system);
/// let orbit = KeplerElements::from_semi_major_axis(
///     Metres::new(30.07 * METRES_PER_AU),
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///     Eccentricity::new(0.0086)?,
///     Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?,
///     Radians::ZERO,
/// )?;
/// let neptune = MoonParent::new(MoonParentParts {
///     id,
///     kind: ParentKind::Planet,
///     mass: EarthMasses::new(17.15),
///     radius: Metres::new(2.4622e7),
///     class: PlanetClass::IceGiant,
///     orbit,
///     host_mass: Kilograms::new(SOLAR_MASS_KG),
///     maximum_moon_mass: EarthMasses::new(1.0),
/// })?;
/// let belt = NearestBelt::new(EarthMasses::new(4e-4), BeltAdjacency::Neighbouring);
/// let caught = captures(Seed::new(3), &neptune, belt);
/// assert!(caught.population().is_some());
/// for moon in caught.moons() {
///     let e = moon.orbit().eccentricity().value();
///     let limit = neptune.stability_limit(e, moon.sense());
///     assert!(moon.orbit().semi_major_axis() < limit);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn captures(seed: Seed, parent: &MoonParent, belt: Option<NearestBelt>) -> Captures {
    let stream = Stream::open(seed, tags::MOON_CAPTURE, ObjectKey::from(parent.id()));
    match parent.class() {
        PlanetClass::GasGiant | PlanetClass::IceGiant => giant_captures(&stream, parent),
        PlanetClass::Rocky => match belt.map(|b| b.adjacency) {
            Some(BeltAdjacency::Neighbouring) => rocky_captures(&stream, parent),
            Some(BeltAdjacency::Separated) | None => Captures::NONE,
        },
        PlanetClass::Icy | PlanetClass::SubNeptune => Captures::NONE,
    }
}

/// The rank at word `word` of `stream`.
#[must_use]
fn rank_at(stream: &Stream, word: u64) -> UnitUniform {
    let mut at = stream.clone();
    at.seek(word);
    draw_rank(&mut at)
}

/// The decision for probability `p` of the word at `word` of `stream` ([`decide`]).
#[must_use]
fn decides_at(stream: &Stream, word: u64, p: f64) -> bool {
    let mut at = stream.clone();
    at.seek(word);
    decide(&mut at, p)
}

/// The first word of captured body `ordinal`'s block.
#[must_use]
fn body_block(ordinal: u8) -> u64 {
    BODY_WORDS_START + BODY_WORDS * (u64::from(ordinal) - 1)
}

/// The points of the midpoint rule over the capture eccentricities in
/// [`surviving_capture_share`]: 64, a fixed count.
const SHARE_POINTS: u32 = 64;

/// The share of the capture distribution drawn about `parent` in Hill radii `hill` (semi-major
/// axis uniform over [`ORBIT_HILL_RADII`] of it, eccentricity uniform over [`ECCENTRICITY`],
/// retrograde with [`RETROGRADE_PROBABILITY`]) whose orbits fit the parent's present limits: the
/// semi-major axis inside its stability limit for the orbit's sense and eccentricity (ruling
/// 133.2), the pericentre outside its radius and its fluid Roche limit for [`CAPTURED_DENSITY`]
/// (ruling 112.4).
///
/// The axis's share is exact at each eccentricity, and the mean over eccentricities a midpoint
/// rule of [`SHARE_POINTS`] points.
///
/// # Examples
///
/// About 95% of the distribution fits Jupiter where it is (ruling 133.2's 0.949), and none of it
/// fits a
/// Jupiter that formed at 5.2 au and now orbits at 0.05 au:
///
/// ```
/// use hyperion_sim::coords::{CellSize, GenCell};
/// use hyperion_sim::id::{Layer, SystemId};
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::derive::PlanetClass;
/// use hyperion_sim::planetary::moons::irregular::surviving_capture_share;
/// use hyperion_sim::planetary::moons::{MoonParent, MoonParentParts, ParentKind};
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// use hyperion_sim::units::{EarthMasses, GravitationalParameter, Kilograms, Metres, Radians, SolarMasses};
///
/// let system = SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, [1, 2, 0])?, 3)?;
/// let id = BodyIndex::new(BodySlot::Planet(5), BodySub::Primary)?.body_id(system);
/// let jupiter_at = |au: f64| -> Result<MoonParent, Box<dyn std::error::Error>> {
///     let orbit = KeplerElements::from_semi_major_axis(
///         Metres::new(au * METRES_PER_AU),
///         GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///         Eccentricity::new(0.048)?,
///         Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?,
///         Radians::ZERO,
///     )?;
///     Ok(MoonParent::new(MoonParentParts {
///         id,
///         kind: ParentKind::Planet,
///         mass: EarthMasses::new(317.8),
///         radius: Metres::new(6.9911e7),
///         class: PlanetClass::GasGiant,
///         orbit,
///         host_mass: Kilograms::new(SOLAR_MASS_KG),
///         maximum_moon_mass: EarthMasses::new(1.0),
///     })?)
/// };
/// let home = jupiter_at(5.2)?;
/// let share = surviving_capture_share(&home, home.formation_hill_radius());
/// assert!((0.94..0.96).contains(&share), "{share}");
/// let hot = jupiter_at(0.05)?.with_formation_distance(Metres::new(5.2 * METRES_PER_AU));
/// assert!(surviving_capture_share(&hot, hot.formation_hill_radius()) < f64::EPSILON);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn surviving_capture_share(parent: &MoonParent, hill: Metres) -> f64 {
    capture_share(parent, hill, 1.0)
}

/// [`surviving_capture_share`] with the stability limits scaled by `limit_scale`: they are
/// proportional to the Hill radius, so a scale of `R_H,f` ÷ `R_H` gives the limits where the parent
/// formed, on an orbit of the same eccentricity.
#[must_use]
fn capture_share(parent: &MoonParent, hill: Metres, limit_scale: f64) -> f64 {
    let (lo_share, hi_share) = ORBIT_HILL_RADII;
    let (e_lo, e_hi) = ECCENTRICITY;
    let (from, to) = (lo_share * hill.value(), hi_share * hill.value());
    let span = to - from;
    if span <= 0.0 {
        return 0.0;
    }
    let floor = parent
        .radius()
        .value()
        .max(parent.roche_limit_fluid(CAPTURED_DENSITY).value());
    let n = f64::from(SHARE_POINTS);
    let fits = |sense: OrbitSense| {
        (0..SHARE_POINTS)
            .map(|k| {
                let e = e_lo + (e_hi - e_lo) * (f64::from(k) + 0.5) / n;
                let lo = from.max(floor / (1.0 - e));
                let limit = parent.stability_limit(e, sense).value() * limit_scale;
                let hi = to.min(limit);
                ((hi - lo) / span).clamp(0.0, 1.0)
            })
            .sum::<f64>()
            / n
    };
    RETROGRADE_PROBABILITY * fits(OrbitSense::Retrograde)
        + (1.0 - RETROGRADE_PROBABILITY) * fits(OrbitSense::Prograde)
}

/// The share of a giant's captured population that survives its migration (ruling 112.4): the
/// capture distribution in Hill radii of its formation distance, [`surviving_capture_share`] of
/// the present limits over that of the limits it had where it formed. Exactly 1 for a giant that
/// formed on its orbit, and 0 where nothing fits now.
#[must_use]
fn migration_share(parent: &MoonParent) -> f64 {
    if parent.formation_distance().value() <= parent.orbit().semi_major_axis().value() {
        return 1.0;
    }
    let (formed, present) = (parent.formation_hill_radius(), parent.hill_radius());
    let now = capture_share(parent, formed, 1.0);
    if now <= 0.0 {
        return 0.0;
    }
    let then = capture_share(parent, formed, formed.value() / present.value());
    if then > 0.0 {
        (now / then).min(1.0)
    } else {
        0.0
    }
}

/// A giant's population, its largest members and an ice giant's large capture.
fn giant_captures(stream: &Stream, parent: &MoonParent) -> Captures {
    let mut moons = Vec::new();
    let large = (parent.class() == PlanetClass::IceGiant
        && decides_at(stream, 8, LARGE_CAPTURE_PROBABILITY))
    .then(|| large_capture(stream, parent));
    if let Some(large) = large {
        moons.push(large);
    }
    let mut count = stream.clone();
    count.seek(0);
    let z = count.standard_normal();
    // Ruling 112.4: the drawn count is the survivors' of a giant that formed where it is, and a
    // migrated giant keeps the share of it that still fits.
    let n = (POPULATION_MEDIAN * math::exp10(POPULATION_SIGMA_DEX * z) * migration_share(parent))
        .round();
    let (lo, hi) = LARGEST_DIAMETER;
    let largest = Metres::new(log_uniform(lo.value(), hi.value(), rank_at(stream, 2)));
    let population = IrregularPopulation::new(whole_count(n), largest);
    // The largest member is D₁; by Rényi's representation the next ones sit where the law's count
    // is 1 + Γₖ₋₁, Γ a running sum of standard exponentials.
    let mut gamma = 0.0;
    for k in 1..=MAX_BODIES {
        if f64::from(k) > n {
            break;
        }
        let block = body_block(k);
        if k > 1 {
            gamma -= math::ln(rank_at(stream, block).value());
        }
        let radius = population.diameter_at(1.0 + gamma) * 0.5;
        if radius < BODY_MIN_RADIUS {
            break;
        }
        if let Some(moon) = captured_body(stream, parent, block, CaptureKind::Irregular, radius) {
            moons.push(CapturedMoon { ordinal: k, ..moon });
        }
    }
    let population = Some(population);
    Captures { population, moons }
}

/// The count `n`, a whole non-negative number far below 2⁵³, as an integer.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rounded log-normal count about 90 is whole, positive and far below 2^53"
)]
fn whole_count(n: f64) -> u64 {
    n.max(0.0) as u64
}

/// An ice giant's Triton-like capture, on words 16–21.
fn large_capture(stream: &Stream, parent: &MoonParent) -> CapturedMoon {
    let rank = |offset: u64| rank_at(stream, LARGE_WORDS + offset);
    let (lo, hi) = LARGE_CAPTURE_MASS_RATIO;
    let mass = parent.mass() * log_uniform(lo, hi, rank(0));
    let a = parent.radius() * log_uniform(LARGE_CAPTURE_RADII.0, LARGE_CAPTURE_RADII.1, rank(1));
    let inclination = inclination(OrbitSense::Retrograde, rank(2));
    let angles = [angle(rank(3)), angle(rank(4)), angle(rank(5))];
    let volume = crate::units::Kilograms::from(mass).value() / CAPTURED_DENSITY.value();
    let radius = Metres::new(math::cbrt(3.0 * volume / (4.0 * core::f64::consts::PI)));
    CapturedMoon {
        kind: CaptureKind::Large,
        ordinal: 1,
        radius,
        mass,
        sense: OrbitSense::Retrograde,
        orbit: moon_orbit(parent, mass, a, 0.0, inclination, angles),
    }
}

/// A rocky planet's one or two small captures.
fn rocky_captures(stream: &Stream, parent: &MoonParent) -> Captures {
    if !decides_at(stream, 8, ROCKY_CAPTURE_PROBABILITY) {
        return Captures::NONE;
    }
    let count: u8 = if decides_at(stream, 9, 0.5) { 1 } else { 2 };
    let mut moons = Vec::new();
    for k in 1..=count {
        let block = body_block(k);
        let (lo, hi) = ROCKY_CAPTURE_RADIUS;
        let radius = Metres::new(log_uniform(lo.value(), hi.value(), rank_at(stream, block)));
        if let Some(moon) = captured_body(stream, parent, block, CaptureKind::Small, radius) {
            moons.push(CapturedMoon { ordinal: k, ..moon });
        }
    }
    Captures {
        population: None,
        moons,
    }
}

/// A uniform angle in [0, 2π) at `rank`.
fn angle(rank: UnitUniform) -> Radians {
    Radians::new(core::f64::consts::TAU * rank.value())
}

/// An inclination at `rank`, isotropic in the half of the sphere of `sense` outside the Kozai
/// gap: cos i uniform in (cos 60°, 1] prograde and [−1, cos 120°) retrograde.
fn inclination(sense: OrbitSense, rank: UnitUniform) -> Radians {
    let (gap_lo, gap_hi) = (Radians::from(KOZAI_GAP.0), Radians::from(KOZAI_GAP.1));
    let cos_i = match sense {
        OrbitSense::Prograde => {
            let edge = math::cos(gap_lo.value());
            1.0 - rank.value() * (1.0 - edge)
        }
        OrbitSense::Retrograde => {
            let edge = math::cos(gap_hi.value());
            -1.0 + rank.value() * (edge + 1.0)
        }
    };
    Radians::new(math::acos(cos_i.clamp(-1.0, 1.0)))
}

/// A captured body of `kind` and `radius` on a capture orbit drawn from its block at `block`, in
/// Hill radii of the parent's formation distance (ruling 112.4), or `None` if no axis of 0.1 of
/// them or more keeps it inside the present stability limit (its semi-major axis, ruling 133.2)
/// and its pericentre outside
/// the parent's radius and fluid Roche limit for [`CAPTURED_DENSITY`].
fn captured_body(
    stream: &Stream,
    parent: &MoonParent,
    block: u64,
    kind: CaptureKind,
    radius: Metres,
) -> Option<CapturedMoon> {
    let rank = |offset: u64| rank_at(stream, block + offset);
    let sense = if decides_at(stream, block + 1, RETROGRADE_PROBABILITY) {
        OrbitSense::Retrograde
    } else {
        OrbitSense::Prograde
    };
    let inclination = inclination(sense, rank(2));
    // Captured where the parent formed (ruling 112.4), and held to the present limits.
    let hill = parent.formation_hill_radius().value();
    let e = ECCENTRICITY.0 + rank(4).value() * (ECCENTRICITY.1 - ECCENTRICITY.0);
    let limit = parent.stability_limit(e, sense).value();
    let floor = parent
        .radius()
        .value()
        .max(parent.roche_limit_fluid(CAPTURED_DENSITY).value());
    let lo = (ORBIT_HILL_RADII.0 * hill).max(floor / (1.0 - e) * (1.0 + 1e-9));
    let hi = (ORBIT_HILL_RADII.1 * hill).min(limit * (1.0 - 1e-9));
    if hi <= lo {
        return None;
    }
    let a = Metres::new(lo + rank(3).value() * (hi - lo));
    let angles = [angle(rank(5)), angle(rank(6)), angle(rank(7))];
    let volume = 4.0 / 3.0 * core::f64::consts::PI * math::powi(radius.value(), 3);
    let mass = EarthMasses::new(CAPTURED_DENSITY.value() * volume / EARTH_MASS_KG);
    Some(CapturedMoon {
        kind,
        ordinal: 0,
        radius,
        mass,
        sense,
        orbit: moon_orbit(parent, mass, a, e, inclination, angles),
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, ks_one_sample};

    use super::*;
    use crate::planetary::moons::ParentKind;
    use crate::planetary::moons::regular::regular_moons;
    use crate::planetary::moons::testing::{jupiter, neptune, parent, planet_id, saturn};

    /// The asteroid belt beside a planet.
    fn belt(mass: f64) -> Option<NearestBelt> {
        NearestBelt::new(EarthMasses::new(mass), BeltAdjacency::Neighbouring)
    }

    /// Giants of every kind: the Solar System's three, and a Jupiter on an eccentric and on a warm
    /// orbit, each as `n` bodies.
    fn giants(n: u32) -> Vec<MoonParent> {
        let mut all = Vec::new();
        for i in 0..n {
            all.push(jupiter(planet_id(i, 5)));
            all.push(saturn(planet_id(i, 6)));
            all.push(neptune(planet_id(i, 8)));
            all.push(parent(
                planet_id(i, 3),
                ParentKind::Planet,
                317.8,
                69_911.0,
                PlanetClass::GasGiant,
                2.0,
                0.4,
            ));
            all.push(parent(
                planet_id(i, 2),
                ParentKind::Planet,
                30.0,
                25_000.0,
                PlanetClass::IceGiant,
                0.3,
                0.1,
            ));
        }
        all
    }

    /// Rocky planets beside a belt, as `n` bodies.
    fn rocky(n: u32) -> Vec<MoonParent> {
        (0..n)
            .map(|i| {
                parent(
                    planet_id(i, 4),
                    ParentKind::Planet,
                    0.107,
                    3_389.5,
                    PlanetClass::Rocky,
                    1.524,
                    0.093,
                )
            })
            .collect()
    }

    fn all_captures(parents: &[MoonParent], mass: f64) -> Vec<(MoonParent, Captures)> {
        parents
            .iter()
            .map(|p| (*p, captures(Seed::new(19), p, belt(mass))))
            .collect()
    }

    #[test]
    fn every_capture_lies_inside_its_stability_limit_and_its_hill_sphere() {
        let mut parents = giants(400);
        parents.extend(rocky(2_000));
        let mut checked = 0;
        for (parent, caught) in all_captures(&parents, 4e-4) {
            for moon in caught.moons() {
                let e = moon.orbit().eccentricity().value();
                let limit = parent.stability_limit(e, moon.sense());
                // Ruling 133.2: Domingos et al.'s limit is on the semi-major axis.
                assert!(moon.orbit().semi_major_axis() < limit, "{moon:?}");
                assert!(moon.orbit().periapsis() > parent.radius());
                assert!(moon.orbit().periapsis() > parent.roche_limit_fluid(CAPTURED_DENSITY));
                let apocentre_bound = parent.hill_radius_at_pericentre();
                assert!(moon.orbit().apoapsis() < apocentre_bound);
                checked += 1;
            }
        }
        assert!(checked > 1_000, "{checked}");
    }

    #[test]
    fn no_inclination_falls_in_the_kozai_gap() {
        let (lo, hi) = (
            Radians::from(KOZAI_GAP.0).value(),
            Radians::from(KOZAI_GAP.1).value(),
        );
        let mut retrograde = 0u64;
        let mut total = 0u64;
        for (_, caught) in all_captures(&giants(400), 4e-4) {
            for moon in caught.moons() {
                let i = moon.orbit().inclination().value();
                assert!(!(lo..=hi).contains(&i), "{} degrees", i.to_degrees());
                let retro = i > core::f64::consts::FRAC_PI_2;
                assert_eq!(retro, moon.sense() == OrbitSense::Retrograde);
                if moon.kind() == CaptureKind::Irregular {
                    total += 1;
                    retrograde += u64::from(retro);
                }
            }
        }
        assert!(total > 1_000);
        let share = f64::from(u32::try_from(retrograde).unwrap())
            / f64::from(u32::try_from(total).unwrap());
        assert!((share - RETROGRADE_PROBABILITY).abs() < 0.05, "{share}");
    }

    #[test]
    fn a_triton_like_capture_leaves_no_regular_moon_outside_it() {
        let mut large = 0u64;
        let (mut kept_moons, mut dropped_moons) = (0usize, 0usize);
        let tries = 2_000;
        for i in 0..tries {
            let ice = neptune(planet_id(i, 8));
            let caught = captures(Seed::new(19), &ice, belt(0.05));
            let Some(triton) = caught.large() else {
                continue;
            };
            large += 1;
            let ratio = triton.mass() / ice.mass();
            assert!((1e-4..1e-3).contains(&ratio));
            assert_eq!(triton.sense(), OrbitSense::Retrograde);
            assert!(triton.orbit().eccentricity().value() < 1e-12);
            let radii = triton.orbit().semi_major_axis() / ice.radius();
            assert!((10.0..20.0).contains(&radii), "{radii}");
            let regular = regular_moons(Seed::new(19), &ice);
            let kept = caught.prune(&ice, &regular);
            let a = triton.orbit().semi_major_axis();
            assert!(kept.moons().iter().all(|m| m.orbit().semi_major_axis() < a));
            let pericentre = triton.orbit().periapsis();
            assert!(
                kept.moons()
                    .iter()
                    .all(|m| m.orbit().apoapsis() < pericentre)
            );
            // The inner moons survive, as a run from the planet out, and every one dropped
            // lies at or beyond the capture's orbit or too near its pericentre.
            let n = kept.moons().len();
            assert_eq!(kept.moons(), &regular.moons()[..n]);
            for dropped in &regular.moons()[n..] {
                let hill = mutual_hill_radius(
                    dropped.mass(),
                    triton.mass(),
                    SolarMasses::from(ice.mass()),
                    dropped.orbit().semi_major_axis(),
                    a,
                );
                assert!(
                    dropped.orbit().semi_major_axis() >= a
                        || pericentre - dropped.orbit().apoapsis() < hill * HILL_STABLE_GAP,
                    "{dropped:?}"
                );
            }
            kept_moons += n;
            dropped_moons += regular.moons().len() - n;
        }
        assert!(kept_moons > 100, "{kept_moons} kept");
        assert!(dropped_moons > 10, "{dropped_moons} dropped");
        assert_poisson_count(
            "large captures",
            large,
            LARGE_CAPTURE_PROBABILITY * f64::from(tries),
            ALPHA,
        );
        // A gas giant never has one.
        assert!(
            all_captures(
                &(0..500)
                    .map(|i| jupiter(planet_id(i, 5)))
                    .collect::<Vec<_>>(),
                4e-4
            )
            .iter()
            .all(|(_, caught)| caught.large().is_none())
        );
    }

    #[test]
    fn the_population_is_log_normal_about_ninety_whatever_the_giant_or_belt() {
        let mut logs: Vec<f64> = Vec::new();
        let (mut largest, mut first, mut unclamped): (Vec<f64>, Vec<f64>, usize) =
            (Vec::new(), Vec::new(), 0);
        for (p, caught) in all_captures(&giants(1_000), 4e-4) {
            let population = caught.population().expect("every giant has a population");
            assert_same_bits(population.size_slope(), SIZE_SLOPE);
            logs.push(math::log10(f64::from(
                u32::try_from(population.count()).unwrap(),
            )));
            // The count depends on neither the belt nor its absence.
            for other in [belt(0.1), None] {
                assert_eq!(
                    captures(Seed::new(19), &p, other).population(),
                    caught.population()
                );
            }
            // The largest members, at most four, are 10 km to D₁ across and in size order, the
            // first D₁ itself when its orbit fits.
            let d1 = population.largest_diameter().value();
            let sizes: Vec<f64> = caught
                .moons()
                .iter()
                .filter(|m| m.kind() == CaptureKind::Irregular)
                .map(|m| 2.0 * m.radius().value())
                .collect();
            assert!(sizes.len() <= usize::from(MAX_BODIES));
            assert!(sizes.windows(2).all(|w| w[0] >= w[1]));
            assert!(
                sizes
                    .iter()
                    .all(|&d| (1e4..=d1 * (1.0 + 1e-12)).contains(&d))
            );
            largest.push(d1);
            first.extend(
                sizes
                    .first()
                    .copied()
                    .filter(|&d| (d / d1 - 1.0).abs() < 1e-12),
            );
            // N(> 2.8 km) is the drawn count wherever the break is not clamped.
            let db = population.break_diameter();
            if db > BREAK_DIAMETER.0 && db < BREAK_DIAMETER.1 {
                let at = population.count_above(POPULATION_MIN_DIAMETER);
                let drawn = f64::from(u32::try_from(population.count()).unwrap());
                assert!((at / drawn - 1.0).abs() < 1e-9, "{at} against {drawn}");
                unclamped += 1;
            }
            // Above the break the law is D₁ ÷ D, q = 2.
            let d = Metres::new(50e3);
            assert!((population.count_above(d) - d1 / 50e3).abs() < 1e-9);
            assert!((population.diameter_at(population.count_above(d)) / d - 1.0).abs() < 1e-9);
        }
        // D₁ is log-uniform over 150–350 km.
        let (lo, hi) = (LARGEST_DIAMETER.0.value(), LARGEST_DIAMETER.1.value());
        let ks = ks_one_sample(&mut largest, |d| {
            (math::ln(d.clamp(lo, hi) / lo) / math::ln(hi / lo)).clamp(0.0, 1.0)
        });
        assert_p_value("the largest irregular's diameter", ks.p_value, ALPHA);
        assert!(
            first.len() * 2 > largest.len(),
            "{} of {}",
            first.len(),
            largest.len()
        );
        assert!(
            unclamped * 2 > largest.len(),
            "{unclamped} of {}",
            largest.len()
        );
        logs.sort_by(f64::total_cmp);
        let n = logs.len();
        let median = logs[n / 2];
        let sigma = (logs[n * 841 / 1000] - logs[n * 159 / 1000]) / 2.0;
        assert!(
            (median - math::log10(90.0)).abs() < 0.02,
            "median 10^{median}"
        );
        assert!((sigma - 0.25).abs() < 0.02, "{sigma} dex");
    }

    #[test]
    fn a_rocky_planet_next_to_a_belt_has_one_or_two_small_captures_one_time_in_five() {
        let parents = rocky(5_000);
        let with = all_captures(&parents, 4e-4)
            .into_iter()
            .filter(|(_, caught)| !caught.moons().is_empty())
            .collect::<Vec<_>>();
        assert_poisson_count(
            "small captures",
            u64::try_from(with.len()).unwrap(),
            ROCKY_CAPTURE_PROBABILITY * 5_000.0,
            5.0 * ALPHA,
        );
        for (_, caught) in &with {
            assert!((1..=2).contains(&caught.moons().len()));
            assert!(caught.population().is_none());
            for moon in caught.moons() {
                assert_eq!(moon.kind(), CaptureKind::Small);
                assert!((1e3..15e3).contains(&moon.radius().value()));
            }
        }
        let separated = NearestBelt::new(EarthMasses::new(4e-4), BeltAdjacency::Separated);
        assert!(
            parents
                .iter()
                .all(|p| captures(Seed::new(19), p, separated) == Captures::NONE)
        );
        assert!(
            parents
                .iter()
                .all(|p| captures(Seed::new(19), p, None) == Captures::NONE)
        );
    }

    #[test]
    fn two_calls_agree_bit_for_bit() {
        for p in giants(50) {
            assert_eq!(
                captures(Seed::new(2), &p, belt(1e-3)),
                captures(Seed::new(2), &p, belt(1e-3))
            );
        }
        assert!(NearestBelt::new(EarthMasses::new(-1.0), BeltAdjacency::Neighbouring).is_none());
    }

    /// Ruling 112.4: irregulars survive by orbit. A giant that formed where it is keeps ruling
    /// 83.4's count bit for bit; one that migrated in keeps the share of its capture distribution,
    /// drawn in its Hill radii of then, that fits now, and every body it keeps fits; a hot giant
    /// keeps none. Before the fix a hot Jupiter kept its ~90.
    #[test]
    fn irregulars_survive_by_orbit() {
        let au = |x: f64| Metres::new(x * crate::units::consts::METRES_PER_AU);
        let at = |i: u32, a: f64| {
            parent(
                planet_id(i, 3),
                ParentKind::Planet,
                317.8,
                69_911.0,
                PlanetClass::GasGiant,
                a,
                0.02,
            )
        };
        let (mut home_total, mut warm_total) = (0_u64, 0_u64);
        for i in 0..300 {
            let seed = Seed::new(23);
            let home = at(i, 5.2);
            let formed_here = home.with_formation_distance(au(5.2));
            assert_same_bits(migration_share(&formed_here), 1.0);
            assert_eq!(
                captures(seed, &home, None),
                captures(seed, &formed_here, None)
            );
            let hot = at(i, 0.05).with_formation_distance(au(5.2));
            let caught = captures(seed, &hot, None);
            assert_eq!(caught.population().map(IrregularPopulation::count), Some(0));
            assert!(caught.moons().is_empty(), "{:?}", caught.moons());
            let warm = at(i, 3.0).with_formation_distance(au(5.2));
            let share = migration_share(&warm);
            assert!(share > 0.0 && share < 1.0, "{share}");
            let kept = captures(seed, &warm, None);
            for moon in kept.moons() {
                let e = moon.orbit().eccentricity().value();
                assert!(moon.orbit().semi_major_axis() < warm.stability_limit(e, moon.sense()));
                assert!(moon.orbit().periapsis() > warm.roche_limit_fluid(CAPTURED_DENSITY));
            }
            home_total += captures(seed, &home, None)
                .population()
                .map_or(0, IrregularPopulation::count);
            warm_total += kept.population().map_or(0, IrregularPopulation::count);
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of a few tens of thousands"
        )]
        let kept = warm_total as f64 / home_total as f64;
        // research/r-pfix14's share.py: 0.503 at 0.5 of the formation distance and 0.661 at 0.6,
        // on the semi-major axis (ruling 133.2); 3 ÷ 5.2 = 0.577 lies between.
        assert!((0.55..0.70).contains(&kept), "{warm_total} of {home_total}");
    }
}
