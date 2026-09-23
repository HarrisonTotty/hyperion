//! Hill spacing, the stability floor between neighbouring planets, and the spacing draw (plan 14,
//! P14.T6; design note 7).
//!
//! Neighbours are spaced in mutual Hill radii, `R_H` = ((m₁ + m₂) ÷ 3M★)^⅓ (a₁ + a₂) ÷ 2, the unit
//! in which the brainstorm centres spacing "on the observed 14–20" and floors it "at about 10–12".
//! A pair satisfies the floor when both of design note 7's conditions hold: its spacing is at
//! least [`spacing_floor`] (10 rising to 12 with eccentricity for two small planets, 7 when one is
//! a giant), and the inner planet's apocentre and the outer's pericentre are at least 2√3 mutual
//! Hill radii apart (Gladman 1993), which makes "no overlapping orbits" a theorem of the generator.
//!
//! # The spacing draw (P14.T6.b)
//!
//! Each orbit host draws a mean spacing μ for each kind of neighbours ([`SpacingKind`],
//! [`SpacingDraws`]): normal about 17 with σ = 2.5, held to 13–24, for small planets; about 30 with
//! σ = 8 for the terrestrial groups of the `SolarLike` and `TerrestrialOnly` classes; and about 9
//! with σ = 2 for giant pairs. Each pair then draws Δ = μ + N(0, 3) ([`draw_pair_spacing`]). A
//! draw under the pair's floor is rejected and redrawn on the next draw number, at most
//! [`MAX_SPACING_REDRAWS`] (16) times, after which the floor itself is used, so the loop is bounded
//! and every pair reads a fixed block of words whatever happened to the others. Within a system
//! the spacings are correlated through μ, as Kepler's are (Weiss et al. 2018).
//!
//! All draws are on [`tags::PLANET_SPACING`], keyed by the system's ID, with the orbit host and
//! the planet's slot in the draw number (design note 4): host h's three mean spacings are words
//! 8h to 8h + 5 of the eight it owns ([`SPACING_WORDS_PER_HOST`]), and the pair whose outer
//! planet is in slot s reads
//! words 2,048 + 64s onwards ([`SPACING_PAIR_WORDS_START`], [`SPACING_WORDS_PER_PAIR`]), two per
//! attempt.

use crate::id::SystemId;
use crate::math;
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::params::{
    HILL_STABLE_GAP, SPACING_FLOOR_ECCENTRICITY_SLOPE, SPACING_FLOOR_GIANT,
    SPACING_FLOOR_SMALL_CIRCULAR, SPACING_FLOOR_SMALL_ECCENTRIC, SPACING_GIANT_MASS,
};
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::draws::StandardNormal;
use crate::units::consts::{EARTH_MASS_KG, SOLAR_MASS_KG};
use crate::units::{EarthMasses, Metres, SolarMasses};

/// The largest Δχ at which [`next_semi_major_axis`] still places a neighbour: 0.9. Beyond it the
/// next orbit would lie more than 19 times farther out, and there is no room for another planet.
pub const MAX_SPACING_STEP: f64 = 0.9;

/// A pair's Hill factor χ = ½ ((m₁ + m₂) ÷ 3M★)^⅓, dimensionless: the pair's mutual Hill radius is
/// χ (a₁ + a₂).
///
/// A type of its own, so that it cannot be passed where a spacing is meant.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct HillFactor(f64);

impl HillFactor {
    /// The factor χ.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// The Hill factor χ = ½ ((m₁ + m₂) ÷ 3M★)^⅓ of two planets of masses `m1` and `m2` about a host
/// of mass `m_host`.
///
/// # Panics
///
/// In debug builds, if the host's mass is not positive or a planet's is negative.
#[must_use]
pub fn mutual_hill_factor(m1: EarthMasses, m2: EarthMasses, m_host: SolarMasses) -> HillFactor {
    debug_assert!(m_host.value() > 0.0, "a host's mass is positive");
    debug_assert!(
        m1.value() >= 0.0 && m2.value() >= 0.0,
        "planet masses are not negative"
    );
    let ratio = (m1.value() + m2.value()) * EARTH_MASS_KG / (3.0 * m_host.value() * SOLAR_MASS_KG);
    HillFactor(0.5 * math::cbrt(ratio))
}

/// The mutual Hill radius of two planets of masses `m1` and `m2` at semi-major axes `a1` and `a2`
/// about a host of mass `m_host`: ((m₁ + m₂) ÷ 3M★)^⅓ × (a₁ + a₂) ÷ 2.
///
/// # Panics
///
/// In debug builds, if the host's mass is not positive or a planet's is negative.
///
/// # Examples
///
/// Jupiter and Saturn are 7.9 mutual Hill radii apart:
///
/// ```
/// use hyperion_sim::planetary::placement::mutual_hill_radius;
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
/// let (a1, a2) = (au(5.2029), au(9.5367));
/// let (jupiter, saturn) = (EarthMasses::new(317.8), EarthMasses::new(95.16));
/// let hill = mutual_hill_radius(jupiter, saturn, SolarMasses::new(1.0), a1, a2);
/// let spacing = (a2 - a1) / hill;
/// assert!((spacing - 7.9).abs() < 0.05);
/// ```
#[must_use]
pub fn mutual_hill_radius(
    m1: EarthMasses,
    m2: EarthMasses,
    m_host: SolarMasses,
    a1: Metres,
    a2: Metres,
) -> Metres {
    // ((m₁ + m₂) ÷ 3M★)^⅓ × (a₁ + a₂) ÷ 2 is 2χ × (a₁ + a₂) ÷ 2.
    Metres::new(mutual_hill_factor(m1, m2, m_host).value() * (a1.value() + a2.value()))
}

/// The semi-major axis of the neighbour outside `a1` at a spacing of `delta` mutual Hill radii,
/// for the pair's factor `chi` ([`mutual_hill_factor`]): a₁ (1 + Δχ) ÷ (1 − Δχ).
///
/// It inverts [`mutual_hill_radius`]: the spacing of the result is `delta`. `None` when Δχ is at
/// least [`MAX_SPACING_STEP`], where there is no room for another planet.
///
/// # Panics
///
/// In debug builds, if `delta` or χ is negative or not finite.
///
/// # Examples
///
/// Placing an Earth-mass neighbour 20 mutual Hill radii outside an Earth at 1 au:
///
/// ```
/// use hyperion_sim::planetary::placement::{
///     mutual_hill_factor, mutual_hill_radius, next_semi_major_axis,
/// };
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let (earth, sun) = (EarthMasses::new(1.0), SolarMasses::new(1.0));
/// let a1 = Metres::from(AstronomicalUnits::new(1.0));
/// let chi = mutual_hill_factor(earth, earth, sun);
/// let a2 = next_semi_major_axis(a1, 20.0, chi).expect("room for a neighbour");
/// let spacing = (a2 - a1) / mutual_hill_radius(earth, earth, sun, a1, a2);
/// assert!((spacing - 20.0).abs() < 1e-12);
/// assert!((AstronomicalUnits::from(a2).value() - 1.29).abs() < 0.01);
/// ```
#[must_use]
pub fn next_semi_major_axis(a1: Metres, delta: f64, chi: HillFactor) -> Option<Metres> {
    let chi = chi.value();
    debug_assert!(delta.is_finite() && delta >= 0.0, "a spacing, got {delta}");
    debug_assert!(chi.is_finite() && chi >= 0.0, "a Hill factor, got {chi}");
    let step = delta * chi;
    (step < MAX_SPACING_STEP).then(|| Metres::new(a1.value() * (1.0 + step) / (1.0 - step)))
}

/// The least spacing, in mutual Hill radii, at which two neighbours of masses `m1` and `m2` and
/// eccentricities `e1` and `e2` are placed (design note 7).
///
/// For two planets under [`SPACING_GIANT_MASS`] (0.1 `M_J`), the measured floor of Kepler's small
/// planets: 10 for circular orbits, rising by 80 per unit of mean eccentricity to at most 12 (Pu
/// and Wu 2015). For a pair with a giant, 7 (Chambers et al. 1996; Marzari and Weidenschilling
/// 2002), because the small planets' floor would forbid Jupiter and Saturn, 7.9 apart.
///
/// # Panics
///
/// In debug builds, if an eccentricity is outside 0–1.
#[must_use]
pub fn spacing_floor(m1: EarthMasses, m2: EarthMasses, e1: f64, e2: f64) -> f64 {
    debug_assert!((0.0..1.0).contains(&e1) && (0.0..1.0).contains(&e2));
    let giant = EarthMasses::from(SPACING_GIANT_MASS);
    if m1 < giant && m2 < giant {
        let mean_e = f64::midpoint(e1, e2);
        (SPACING_FLOOR_SMALL_CIRCULAR + SPACING_FLOOR_ECCENTRICITY_SLOPE * mean_e)
            .min(SPACING_FLOOR_SMALL_ECCENTRIC)
    } else {
        SPACING_FLOOR_GIANT
    }
}

/// A planet as the spacing floor sees it: its mass and the size and shape of its orbit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Neighbour {
    mass: EarthMasses,
    semi_major_axis: Metres,
    eccentricity: f64,
}

impl Neighbour {
    /// A planet of mass `mass` on an orbit of semi-major axis `semi_major_axis` and eccentricity
    /// `eccentricity`.
    ///
    /// # Panics
    ///
    /// In debug builds, if the mass or the semi-major axis is not positive, or the eccentricity
    /// is outside 0–1.
    #[must_use]
    pub fn new(mass: EarthMasses, semi_major_axis: Metres, eccentricity: f64) -> Self {
        debug_assert!(mass.value() > 0.0 && semi_major_axis.value() > 0.0);
        debug_assert!((0.0..1.0).contains(&eccentricity), "got e = {eccentricity}");
        Self {
            mass,
            semi_major_axis,
            eccentricity,
        }
    }

    /// The planet's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The orbit's semi-major axis.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Metres {
        self.semi_major_axis
    }

    /// The orbit's eccentricity.
    #[must_use]
    pub const fn eccentricity(&self) -> f64 {
        self.eccentricity
    }
}

/// Whether `inner` and `outer`, neighbours about a host of mass `m_host`, satisfy both of design
/// note 7's conditions: a spacing of at least [`spacing_floor`] mutual Hill radii, and at least
/// [`HILL_STABLE_GAP`] (2√3) mutual Hill radii between the inner apocentre and the outer
/// pericentre (Gladman 1993). False if `outer` is not outside `inner`.
///
/// # Panics
///
/// In debug builds, if the host's mass is not positive.
///
/// # Examples
///
/// Jupiter and Saturn clear the giants' floor of 7; two Jupiters 1.3 au apart do not:
///
/// ```
/// use hyperion_sim::planetary::placement::{Neighbour, satisfies_floor};
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
/// let sun = SolarMasses::new(1.0);
/// let jupiter = Neighbour::new(EarthMasses::new(317.8), au(5.203), 0.048);
/// let saturn = Neighbour::new(EarthMasses::new(95.16), au(9.537), 0.054);
/// assert!(satisfies_floor(&jupiter, &saturn, sun));
/// let twin = Neighbour::new(EarthMasses::new(317.8), au(6.5), 0.0);
/// assert!(!satisfies_floor(&jupiter, &twin, sun));
/// ```
#[must_use]
pub fn satisfies_floor(inner: &Neighbour, outer: &Neighbour, m_host: SolarMasses) -> bool {
    let (a1, a2) = (inner.semi_major_axis, outer.semi_major_axis);
    if a2 <= a1 {
        return false;
    }
    let hill = mutual_hill_radius(inner.mass, outer.mass, m_host, a1, a2).value();
    let spacing = (a2.value() - a1.value()) / hill;
    let floor = spacing_floor(
        inner.mass,
        outer.mass,
        inner.eccentricity,
        outer.eccentricity,
    );
    let apocentre = a1.value() * (1.0 + inner.eccentricity);
    let pericentre = a2.value() * (1.0 - outer.eccentricity);
    spacing >= floor && pericentre - apocentre >= HILL_STABLE_GAP * hill
}

/// Words of the [`tags::PLANET_SPACING`] stream that one orbit host's mean spacings own: host h
/// reads words 8h to 8h + 7, a standard normal for each [`SpacingKind`] in declaration order and
/// two words reserved. 256 hosts fill words 0 to 2,047. Changing it moves every host after the
/// first, which is a generator-version change.
pub const SPACING_WORDS_PER_HOST: u64 = 8;

/// The first word of the pairs' block of the [`tags::PLANET_SPACING`] stream, 2,048: the end of
/// the 256 hosts' mean spacings.
pub const SPACING_PAIR_WORDS_START: u64 = 256 * SPACING_WORDS_PER_HOST;

/// Words of the [`tags::PLANET_SPACING`] stream that one pair owns: the pair whose outer planet
/// is in slot s reads words 2,048 + 64s onwards, two for each of its at most 17 attempts, and the
/// rest are reserved.
pub const SPACING_WORDS_PER_PAIR: u64 = 64;

/// How many times a pair's spacing is redrawn when it falls under the pair's floor: 16. After the
/// sixteenth redraw, the seventeenth attempt, the floor itself is taken (P14.T6.b).
pub const MAX_SPACING_REDRAWS: u8 = 16;

/// The scatter of a pair's spacing about its host's mean: σ = 3 mutual Hill radii (P14.T6.b).
pub const PAIR_SPACING_SIGMA: f64 = 3.0;

// `as` widens a u8 to a u64 here, losslessly: `u64::from` is not callable in a constant.
const _: () = assert!(
    2 * (MAX_SPACING_REDRAWS as u64 + 1) <= SPACING_WORDS_PER_PAIR,
    "every attempt of a pair fits its block"
);

/// The kind of neighbours a spacing is drawn for, each with its own law for a host's mean
/// ([`SpacingKind::law`], P14.T6.b).
///
/// Which kind a pair is, is the class template's (P14.T5) and the placer's (P14.T8): the pair's
/// floor is [`spacing_floor`] in every case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpacingKind {
    /// Neighbours in a compact group of small planets, Kepler's multis.
    SmallPlanets,
    /// Neighbours in the terrestrial group of a `SolarLike` or `TerrestrialOnly` system.
    TerrestrialGroup,
    /// Two neighbouring giants.
    GiantPair,
}

impl SpacingKind {
    /// Every kind, in declaration order, which is the order of their words in a host's block.
    pub const ALL: [Self; 3] = [Self::SmallPlanets, Self::TerrestrialGroup, Self::GiantPair];

    /// The law of a host's mean spacing for neighbours of this kind, in mutual Hill radii.
    ///
    /// - Small planets: normal about 17 with σ = 2.5, held to 13–24, plan 14's reading of the
    ///   brainstorm's "centred on the observed 14–20". Weiss et al. (2018, AJ 155, 48, §5.2 and
    ///   Fig. 14) find 93% of the California–Kepler Survey's adjacent pairs at least 10 mutual Hill
    ///   radii apart and the distribution peaking at about 20, the systems of four or more
    ///   transiting planets the tightest, and note that a pair seen 20 apart may hide a planet
    ///   that does not transit. Pu and Wu (2015, ApJ 807, 44, abstract) find the pairs of systems
    ///   with four or more transiting planets "tightly clustered around 12 mutual Hill radii" once
    ///   transit geometry and sensitivity are accounted for.
    /// - Terrestrial groups: normal about 30 with σ = 8, plan 14's figure, held to 14–46, two
    ///   standard deviations, so that the mean clears the small planets' highest floor of 12.
    ///   Computed from JPL's masses, the Solar System's Venus and Earth are 26.3 apart and Earth
    ///   and Mars 40.1.
    /// - Giant pairs: normal about 9 with σ = 2, plan 14's figure, held to 7–13: the giants' floor
    ///   of 7 below and two standard deviations above. Jupiter and Saturn are 7.9 apart, Saturn
    ///   and Uranus 14.0.
    ///
    /// The holds of the second and third laws are this module's, since the plan gives none; a
    /// mean under the floor would leave every pair at the floor.
    #[must_use]
    pub const fn law(self) -> MeanSpacingLaw {
        match self {
            Self::SmallPlanets => MeanSpacingLaw {
                centre: 17.0,
                sigma: 2.5,
                min: 13.0,
                max: 24.0,
            },
            Self::TerrestrialGroup => MeanSpacingLaw {
                centre: 30.0,
                sigma: 8.0,
                min: 14.0,
                max: 46.0,
            },
            Self::GiantPair => MeanSpacingLaw {
                centre: 9.0,
                sigma: 2.0,
                min: 7.0,
                max: 13.0,
            },
        }
    }

    /// The first word of this kind's standard normal in a host's block.
    #[must_use]
    const fn word(self) -> u64 {
        match self {
            Self::SmallPlanets => 0,
            Self::TerrestrialGroup => 2,
            Self::GiantPair => 4,
        }
    }
}

/// The law of a host's mean spacing: normal about [`centre`](Self::centre) with standard
/// deviation [`sigma`](Self::sigma), held to [`min`](Self::min)–[`max`](Self::max), all in mutual
/// Hill radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeanSpacingLaw {
    centre: f64,
    sigma: f64,
    min: f64,
    max: f64,
}

impl MeanSpacingLaw {
    /// The normal's mean and median.
    #[must_use]
    pub const fn centre(self) -> f64 {
        self.centre
    }

    /// The normal's standard deviation.
    #[must_use]
    pub const fn sigma(self) -> f64 {
        self.sigma
    }

    /// The least mean spacing.
    #[must_use]
    pub const fn min(self) -> f64 {
        self.min
    }

    /// The greatest mean spacing.
    #[must_use]
    pub const fn max(self) -> f64 {
        self.max
    }

    /// The mean spacing at the standard normal `z`: `centre + sigma × z`, held to `min`–`max`.
    #[must_use]
    pub fn mean(self, z: StandardNormal) -> f64 {
        (self.centre + self.sigma * z.value()).clamp(self.min, self.max)
    }
}

/// The random variates of one orbit host's mean spacings, as drawn from [`tags::PLANET_SPACING`]
/// by [`SpacingDraws::for_host`], or given explicitly by a test or a tool.
///
/// The fields are plain variates with no invariant between them, so they are public, as the
/// disc's [`DiscDraws`](crate::planetary::disc::DiscDraws) are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingDraws {
    /// The small planets' mean spacing's standard normal. Words 0–1 of the host's block.
    pub small_planets: StandardNormal,
    /// The terrestrial group's. Words 2–3.
    pub terrestrial_group: StandardNormal,
    /// The giant pairs'. Words 4–5.
    pub giant_pair: StandardNormal,
}

impl SpacingDraws {
    /// Every variate at its median: every mean spacing at its law's centre.
    pub const MEDIAN: Self = Self {
        small_planets: StandardNormal::ZERO,
        terrestrial_group: StandardNormal::ZERO,
        giant_pair: StandardNormal::ZERO,
    };

    /// The draws of orbit host number `host` of `system`, in the universe of `seed`: words 8 ×
    /// host onwards of `system`'s [`tags::PLANET_SPACING`] stream.
    ///
    /// Hosts are numbered as for the disc's draws: a single star is host 0, and the zones of a
    /// multiple system take the numbers of
    /// [`OrbitZone::host_number`](super::zones::OrbitZone::host_number).
    #[must_use]
    pub fn for_host(seed: Seed, system: SystemId, host: u8) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_SPACING, ObjectKey::from(system));
        let start = u64::from(host) * SPACING_WORDS_PER_HOST;
        Self {
            small_planets: normal_at(&mut stream, start + SpacingKind::SmallPlanets.word()),
            terrestrial_group: normal_at(&mut stream, start + SpacingKind::TerrestrialGroup.word()),
            giant_pair: normal_at(&mut stream, start + SpacingKind::GiantPair.word()),
        }
    }

    /// This host's standard normal for neighbours of kind `kind`.
    #[must_use]
    pub const fn normal(&self, kind: SpacingKind) -> StandardNormal {
        match kind {
            SpacingKind::SmallPlanets => self.small_planets,
            SpacingKind::TerrestrialGroup => self.terrestrial_group,
            SpacingKind::GiantPair => self.giant_pair,
        }
    }

    /// This host's mean spacing μ for neighbours of kind `kind`, in mutual Hill radii: the kind's
    /// [`law`](SpacingKind::law) at this host's variate.
    #[must_use]
    pub fn mean_spacing(&self, kind: SpacingKind) -> f64 {
        kind.law().mean(self.normal(kind))
    }
}

/// The standard normal on words `word` and `word + 1` of `stream`.
#[must_use]
fn normal_at(stream: &mut Stream, word: u64) -> StandardNormal {
    stream.seek(word);
    StandardNormal::new(stream.standard_normal()).expect("a Box–Muller variate is finite")
}

/// How a pair's spacing was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpacingOutcome {
    /// A draw at or above the floor, after `redraws` rejected ones (0–16).
    Drawn {
        /// How many draws before it fell under the floor.
        redraws: u8,
    },
    /// All seventeen attempts fell under the floor, and the floor was taken.
    Floor,
}

/// A pair's spacing, in mutual Hill radii, and how it was reached ([`draw_pair_spacing`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairSpacing {
    spacing: f64,
    outcome: SpacingOutcome,
}

impl PairSpacing {
    /// The spacing Δ, in mutual Hill radii: never under the floor it was drawn against.
    #[must_use]
    pub const fn spacing(&self) -> f64 {
        self.spacing
    }

    /// Whether it was drawn, after how many redraws, or is the floor.
    #[must_use]
    pub const fn outcome(&self) -> SpacingOutcome {
        self.outcome
    }
}

/// The spacing of the pair of `system` whose outer planet is `outer`, in the universe of `seed`,
/// about the host's mean spacing `mean` and never under `floor`, both in mutual Hill radii
/// (P14.T6.b).
///
/// Attempt k, from 0, is Δ = `mean` + N(0, 3) on words 2,048 + 64s + 2k of
/// [`tags::PLANET_SPACING`], s being the slot of `outer` (the high byte of its index). The first
/// attempt at or above `floor` is taken; after [`MAX_SPACING_REDRAWS`] redraws the floor is. Each
/// pair reads its own block, so a pair's spacing depends on nothing but its slot, its mean and its
/// floor. The words follow the slot, so a placer assigns the outer planet its slot before drawing
/// its spacing, and never renumbers it afterwards.
///
/// The caller supplies the host's `mean` ([`SpacingDraws::mean_spacing`]) and the pair's `floor`,
/// [`spacing_floor`] of its masses and of the eccentricities it will be placed at.
///
/// # Panics
///
/// In debug builds, if `mean` is not finite, `floor` is not positive and finite, or `outer` is not
/// a planet: a primordial or second-generation planet's own index, not a moon's, a ring's, a
/// belt's or a star's.
///
/// # Examples
///
/// The second planet of a compact chain, outside the first, about host 0's mean spacing:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::placement::spacing::{
///     SpacingDraws, SpacingKind, draw_pair_spacing, spacing_floor,
/// };
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::units::EarthMasses;
///
/// let (seed, system) = (Seed::new(7), SystemId::from_raw(0x0200_0800_2000_0000)?);
/// let mean = SpacingDraws::for_host(seed, system, 0).mean_spacing(SpacingKind::SmallPlanets);
/// assert!((13.0..=24.0).contains(&mean));
///
/// let (m1, m2) = (EarthMasses::new(4.0), EarthMasses::new(6.0));
/// let floor = spacing_floor(m1, m2, 0.0, 0.0);
/// let outer = BodyIndex::new(BodySlot::Planet(2), BodySub::Primary)?;
/// let pair = draw_pair_spacing(seed, system, outer, mean, floor);
/// assert!(pair.spacing() >= 10.0);
/// // The same pair, asked again, is the same to the bit.
/// assert_eq!(pair, draw_pair_spacing(seed, system, outer, mean, floor));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn draw_pair_spacing(
    seed: Seed,
    system: SystemId,
    outer: BodyIndex,
    mean: f64,
    floor: f64,
) -> PairSpacing {
    debug_assert!(mean.is_finite(), "a mean spacing, got {mean}");
    debug_assert!(
        floor.is_finite() && floor > 0.0,
        "a spacing floor, got {floor}"
    );
    debug_assert!(
        matches!(
            (outer.slot(), outer.sub()),
            (
                BodySlot::Planet(_) | BodySlot::SecondGeneration(_),
                BodySub::Primary
            )
        ),
        "a pair's outer body is a planet, got {outer:?}"
    );
    let slot = u64::from(outer.get() >> 8);
    let mut stream = Stream::open(seed, tags::PLANET_SPACING, ObjectKey::from(system));
    stream.seek(SPACING_PAIR_WORDS_START + slot * SPACING_WORDS_PER_PAIR);
    for redraws in 0..=MAX_SPACING_REDRAWS {
        let spacing = stream.normal(mean, PAIR_SPACING_SIGMA);
        if spacing >= floor {
            return PairSpacing {
                spacing,
                outcome: SpacingOutcome::Drawn { redraws },
            };
        }
    }
    PairSpacing {
        spacing: floor,
        outcome: SpacingOutcome::Floor,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample, normal_cdf};

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::Layer;
    use crate::units::AstronomicalUnits;
    use crate::units::consts::{GM_EARTH, GM_JUPITER, GM_SUN, GRAVITATIONAL_CONSTANT};

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn m_earth_of_gm(gm: f64) -> EarthMasses {
        EarthMasses::new(gm / GM_EARTH)
    }

    /// The eight planets: semi-major axis (au), eccentricity (JPL's approximate elements,
    /// Standish) and GM (m³ s⁻²).
    fn planets() -> [(&'static str, Neighbour); 8] {
        let p = |a: f64, e: f64, gm: f64| Neighbour::new(m_earth_of_gm(gm), au(a), e);
        [
            ("Mercury", p(0.387_099_27, 0.205_635_93, 2.203_2e13)),
            ("Venus", p(0.723_335_66, 0.006_776_72, 3.248_59e14)),
            ("Earth", p(1.000_002_61, 0.016_711_23, GM_EARTH)),
            ("Mars", p(1.523_710_34, 0.093_394_10, 4.282_837e13)),
            ("Jupiter", p(5.202_887, 0.048_386_24, GM_JUPITER)),
            ("Saturn", p(9.536_675_94, 0.053_861_79, 3.793_118_7e16)),
            ("Uranus", p(19.189_164_64, 0.047_257_44, 5.793_939e15)),
            ("Neptune", p(30.069_922_76, 0.008_590_48, 6.836_529e15)),
        ]
    }

    fn sun() -> SolarMasses {
        SolarMasses::new(GM_SUN / GRAVITATIONAL_CONSTANT / SOLAR_MASS_KG)
    }

    fn spacing(inner: &Neighbour, outer: &Neighbour, host: SolarMasses) -> f64 {
        let (a1, a2) = (inner.semi_major_axis(), outer.semi_major_axis());
        (a2 - a1) / mutual_hill_radius(inner.mass(), outer.mass(), host, a1, a2)
    }

    #[test]
    fn the_solar_system_satisfies_the_floor_pairwise() {
        let planets = planets();
        for (i, (inner_name, inner)) in planets.iter().enumerate() {
            for (outer_name, outer) in &planets[i + 1..] {
                assert!(
                    satisfies_floor(inner, outer, sun()),
                    "{inner_name} and {outer_name}: {:.2} mutual Hill radii",
                    spacing(inner, outer, sun())
                );
            }
        }
        // Jupiter and Saturn at 7.9, against the giants' floor of 7.
        let (jupiter, saturn) = (&planets[4].1, &planets[5].1);
        let js = spacing(jupiter, saturn, sun());
        assert!((js - 7.9).abs() < 0.05, "{js}");
        assert!((spacing_floor(jupiter.mass(), saturn.mass(), 0.05, 0.05) - 7.0).abs() < 1e-15);
        // Uranus and Neptune are both under 0.1 M_J, so the small planets' floor applies.
        let (uranus, neptune) = (&planets[6].1, &planets[7].1);
        assert!(spacing_floor(uranus.mass(), neptune.mass(), 0.0, 0.0) >= 10.0);
    }

    #[test]
    fn two_jupiters_at_five_point_two_and_six_point_five_au_fail() {
        let jupiter = m_earth_of_gm(GM_JUPITER);
        let inner = Neighbour::new(jupiter, au(5.2), 0.0);
        let outer = Neighbour::new(jupiter, au(6.5), 0.0);
        assert!(!satisfies_floor(&inner, &outer, sun()));
        assert!(spacing(&inner, &outer, sun()) < 3.0);
    }

    #[test]
    fn kepler_11_b_and_c_fail_narrowly() {
        // Lissauer et al. (2013, ApJ 770, 131, Tables 3 and 4): M★ = 0.961 M☉; b 1.9 M⊕ at 0.091 au,
        // e = 0.045; c 2.9 M⊕ at 0.107 au, e = 0.026.
        let host = SolarMasses::new(0.961);
        let b = Neighbour::new(EarthMasses::new(1.9), au(0.091), 0.045);
        let c = Neighbour::new(EarthMasses::new(2.9), au(0.107), 0.026);
        let bc = spacing(&b, &c, host);
        assert!((bc - 9.4).abs() < 0.1, "{bc}");
        assert!(!satisfies_floor(&b, &c, host));
        // Short of even the circular floor of 10.
        let circular_c = Neighbour::new(c.mass(), c.semi_major_axis(), 0.0);
        let circular_b = Neighbour::new(b.mass(), b.semi_major_axis(), 0.0);
        assert!(!satisfies_floor(&circular_b, &circular_c, host));
    }

    #[test]
    fn next_semi_major_axis_inverts_the_mutual_hill_radius() {
        let mut rng = Lcg::new(0x5eed_0014_0006);
        for _ in 0..10_000 {
            let a1 = au(0.01 + 50.0 * rng.next_f64());
            let m1 = EarthMasses::new(0.01 + 3_000.0 * rng.next_f64());
            let m2 = EarthMasses::new(0.01 + 3_000.0 * rng.next_f64());
            let host = SolarMasses::new(0.08 + 3.0 * rng.next_f64());
            let delta = 2.0 + 40.0 * rng.next_f64();
            let chi = mutual_hill_factor(m1, m2, host);
            let Some(a2) = next_semi_major_axis(a1, delta, chi) else {
                assert!(delta * chi.value() >= MAX_SPACING_STEP);
                continue;
            };
            let back = (a2 - a1) / mutual_hill_radius(m1, m2, host, a1, a2);
            assert!(
                (back / delta - 1.0).abs() < 1e-12,
                "{delta} came back as {back}"
            );
        }
    }

    #[test]
    fn there_is_no_room_past_the_largest_step() {
        assert_eq!(
            next_semi_major_axis(au(1.0), 10.0, HillFactor(0.0901)),
            None
        );
        assert_eq!(
            next_semi_major_axis(au(1.0), 1.0, HillFactor(MAX_SPACING_STEP)),
            None
        );
        let a = next_semi_major_axis(au(1.0), 10.0, HillFactor(0.0899)).unwrap();
        assert!((AstronomicalUnits::from(a).value() - 1.899 / 0.101).abs() < 1e-9);
    }

    #[test]
    fn the_small_planets_floor_rises_with_eccentricity_to_twelve() {
        let small = EarthMasses::new(5.0);
        assert!((spacing_floor(small, small, 0.0, 0.0) - 10.0).abs() < 1e-15);
        assert!((spacing_floor(small, small, 0.01, 0.01) - 10.8).abs() < 1e-12);
        assert!((spacing_floor(small, small, 0.02, 0.02) - 11.6).abs() < 1e-12);
        // Pu and Wu's 12 at σₑ = 0.02, a mean eccentricity of 0.025.
        assert!((spacing_floor(small, small, 0.025, 0.025) - 12.0).abs() < 1e-12);
        assert!((spacing_floor(small, small, 0.3, 0.1) - 12.0).abs() < 1e-15);
        // 0.1 M_J is the boundary: a pair with a planet at or above it takes the giants' floor.
        let giant = EarthMasses::from(SPACING_GIANT_MASS);
        assert!((giant.value() - 31.78).abs() < 0.01, "{}", giant.value());
        assert!((spacing_floor(small, giant, 0.0, 0.0) - 7.0).abs() < 1e-15);
        assert!((spacing_floor(giant, small, 0.3, 0.3) - 7.0).abs() < 1e-15);
    }

    #[test]
    fn overlapping_orbits_never_satisfy_the_floor() {
        // Widely spaced in Hill radii but crossing: the inner apocentre is at 1.7 au.
        let inner = Neighbour::new(EarthMasses::new(1.0), au(1.0), 0.7);
        let outer = Neighbour::new(EarthMasses::new(1.0), au(1.6), 0.0);
        assert!(spacing(&inner, &outer, sun()) > 12.0);
        assert!(!satisfies_floor(&inner, &outer, sun()));
        // The same orbits in the wrong order, or the same orbit twice.
        assert!(!satisfies_floor(&outer, &inner, sun()));
        assert!(!satisfies_floor(&inner, &inner, sun()));
    }

    #[test]
    fn the_gap_at_closest_approach_is_two_root_three_mutual_hill_radii() {
        assert!((HILL_STABLE_GAP - 2.0 * 3.0_f64.sqrt()).abs() < 1e-15);
        // Two giants at a spacing of 8, just clear of the floor of 7, with eccentricities that
        // bring them to 2√3 mutual Hill radii at closest approach, and just past it.
        let jupiter = m_earth_of_gm(GM_JUPITER);
        let a1 = au(5.0);
        let chi = mutual_hill_factor(jupiter, jupiter, sun());
        let a2 = next_semi_major_axis(a1, 8.0, chi).unwrap();
        let hill = mutual_hill_radius(jupiter, jupiter, sun(), a1, a2).value();
        let e_at_gap =
            (a2.value() - a1.value() - HILL_STABLE_GAP * hill) / (a1.value() + a2.value());
        let pair = |e: f64| {
            satisfies_floor(
                &Neighbour::new(jupiter, a1, e),
                &Neighbour::new(jupiter, a2, e),
                sun(),
            )
        };
        assert!(pair(e_at_gap * (1.0 - 1e-9)));
        assert!(!pair(e_at_gap * (1.0 + 1e-9)));
    }

    // P14.T6.b, the spacing draw.

    const SEED: Seed = Seed::new(0x5eed_0014_0006_000b);

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [5, -9, 2]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn planet(slot: u8) -> BodyIndex {
        BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap()
    }

    #[test]
    fn accepted_small_planet_spacings_have_no_mass_below_ten_and_a_median_in_fourteen_to_twenty() {
        // Two small planets on circular orbits: the brainstorm's floor of 10.
        let floor = spacing_floor(EarthMasses::new(3.0), EarthMasses::new(5.0), 0.0, 0.0);
        assert_same_bits(floor, 10.0);
        let mut spacings = Vec::with_capacity(10_000);
        for index in 0..2_000 {
            let id = system(index);
            let mean = SpacingDraws::for_host(SEED, id, 0).mean_spacing(SpacingKind::SmallPlanets);
            for slot in 2..=6 {
                let pair = draw_pair_spacing(SEED, id, planet(slot), mean, floor);
                // Seventeen draws under 10 about a mean of at least 13 has odds of 10⁻¹³.
                assert_ne!(pair.outcome(), SpacingOutcome::Floor);
                spacings.push(pair.spacing());
            }
        }
        spacings.sort_by(f64::total_cmp);
        assert!(spacings[0] >= 10.0, "{}", spacings[0]);
        let median = spacings[spacings.len() / 2];
        assert!((14.0..=20.0).contains(&median), "median {median}");
    }

    #[test]
    fn a_pair_s_spacing_is_its_host_s_normal_truncated_at_the_floor() {
        // About a mean of 13 against the eccentric floor of 12, 37% of first draws are rejected;
        // the accepted ones are the normal N(13, 3) cut at 12.
        let (mean, floor) = (13.0, 12.0);
        let mut spacings: Vec<f64> = (0..10_000)
            .map(|i| draw_pair_spacing(SEED, system(i), planet(3), mean, floor).spacing())
            .collect();
        let below = normal_cdf((floor - mean) / PAIR_SPACING_SIGMA);
        let ks = ks_one_sample(&mut spacings, |x| {
            ((normal_cdf((x - mean) / PAIR_SPACING_SIGMA) - below) / (1.0 - below)).max(0.0)
        });
        assert_p_value("truncated pair spacings", ks.p_value, ALPHA);
    }

    #[test]
    fn attempt_k_of_the_pair_in_slot_s_is_on_words_2048_plus_64s_plus_2k() {
        for (index, slot, mean, floor) in
            [(0, 2, 17.0, 10.0), (1, 9, 13.0, 12.0), (2, 191, 9.0, 7.0)]
        {
            let id = system(index);
            let pair = draw_pair_spacing(SEED, id, planet(slot), mean, floor);
            let mut stream = Stream::open(SEED, tags::PLANET_SPACING, ObjectKey::from(id));
            stream.seek(2_048 + 64 * u64::from(slot));
            let mut redraws = 0;
            let spacing = loop {
                let delta = stream.normal(mean, 3.0);
                if delta >= floor {
                    break delta;
                }
                redraws += 1;
            };
            assert_same_bits(pair.spacing(), spacing);
            assert_eq!(pair.outcome(), SpacingOutcome::Drawn { redraws });
        }
        // The last pair's block ends inside the stream's first 2¹⁵ words.
        const { assert!(SPACING_PAIR_WORDS_START + 256 * SPACING_WORDS_PER_PAIR <= 1 << 15) };
    }

    #[test]
    fn after_sixteen_redraws_the_floor_is_taken() {
        // A mean of 0 against a floor of 30 is ten standard deviations short every time.
        let pair = draw_pair_spacing(SEED, system(0), planet(1), 0.0, 30.0);
        assert_eq!(pair.outcome(), SpacingOutcome::Floor);
        assert_same_bits(pair.spacing(), 30.0);
        // Redraws are counted: some pairs about a mean at the floor need several.
        let most = (0..2_000)
            .filter_map(|i| {
                match draw_pair_spacing(SEED, system(i), planet(2), 12.0, 12.0).outcome() {
                    SpacingOutcome::Drawn { redraws } => Some(redraws),
                    SpacingOutcome::Floor => None,
                }
            })
            .max()
            .unwrap();
        assert!((5..=MAX_SPACING_REDRAWS).contains(&most), "{most}");
    }

    #[test]
    fn two_runs_of_the_redraw_loop_agree() {
        for index in 0..500 {
            let id = system(index);
            let mean = SpacingDraws::for_host(SEED, id, 3).mean_spacing(SpacingKind::GiantPair);
            let first = draw_pair_spacing(SEED, id, planet(4), mean, 7.0);
            let second = draw_pair_spacing(SEED, id, planet(4), mean, 7.0);
            assert_same_bits(first.spacing(), second.spacing());
            assert_eq!(first.outcome(), second.outcome());
        }
        let keys: Vec<(u32, u8)> = (0..40).flat_map(|i| (1..=5).map(move |s| (i, s))).collect();
        assert_order_independent(&keys, |&(i, s)| {
            hyperion_testkit::float::bits(
                draw_pair_spacing(SEED, system(i), planet(s), 17.0, 10.0).spacing(),
            )
        });
    }

    #[test]
    fn host_h_draws_its_mean_spacings_from_words_8h_onwards() {
        for host in [0_u8, 1, 17, 31, 255] {
            let id = system(u32::from(host));
            let draws = SpacingDraws::for_host(SEED, id, host);
            let mut stream = Stream::open(SEED, tags::PLANET_SPACING, ObjectKey::from(id));
            stream.seek(8 * u64::from(host));
            for kind in SpacingKind::ALL {
                assert_same_bits(draws.normal(kind).value(), stream.standard_normal());
            }
            assert!(stream.position() <= 8 * u64::from(host) + SPACING_WORDS_PER_HOST);
        }
    }

    #[test]
    fn mean_spacings_follow_their_laws_inside_their_holds() {
        let n = 20_000;
        let mut means: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for index in 0..n {
            let draws = SpacingDraws::for_host(SEED, system(index), 0);
            for (kind, out) in SpacingKind::ALL.into_iter().zip(&mut means) {
                out.push(draws.mean_spacing(kind));
            }
        }
        // A count against its binomial expectation, two-sided at α: the normal approximation
        // with a continuity correction, sound here since every expected count is over 50.
        let assert_binomial = |name: String, values: &[f64], f: &dyn Fn(f64) -> bool, p: f64| {
            let hits = values.iter().filter(|&&x| f(x)).count();
            let (k, n) = (f64::from(u32::try_from(hits).unwrap()), f64::from(n));
            let z = (((k - n * p).abs() - 0.5).max(0.0)) / (n * p * (1.0 - p)).sqrt();
            assert_p_value(&name, 2.0 * (1.0 - normal_cdf(z)), ALPHA);
        };
        for (kind, values) in SpacingKind::ALL.into_iter().zip(&mut means) {
            let law = kind.law();
            values.sort_by(f64::total_cmp);
            assert!(values[0] >= law.min() && values[values.len() - 1] <= law.max());
            // Half the means lie below the law's centre, and the share held at each end is the
            // normal's tail beyond it.
            let below = |x: f64| x < law.centre();
            assert_binomial(format!("{kind:?} below centre"), values, &below, 0.5);
            let low = normal_cdf((law.min() - law.centre()) / law.sigma());
            let high = 1.0 - normal_cdf((law.max() - law.centre()) / law.sigma());
            let at_min = |x: f64| x <= law.min();
            let at_max = |x: f64| x >= law.max();
            assert_binomial(format!("{kind:?} held low"), values, &at_min, low);
            assert_binomial(format!("{kind:?} held high"), values, &at_max, high);
        }
        // Small planets: 17 ± 2.5 held to 13–24; the median host is at the centre exactly.
        assert_same_bits(
            SpacingDraws::MEDIAN.mean_spacing(SpacingKind::SmallPlanets),
            17.0,
        );
        assert_same_bits(
            SpacingDraws::MEDIAN.mean_spacing(SpacingKind::TerrestrialGroup),
            30.0,
        );
        assert_same_bits(
            SpacingDraws::MEDIAN.mean_spacing(SpacingKind::GiantPair),
            9.0,
        );
    }
}
