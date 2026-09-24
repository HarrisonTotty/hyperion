//! Class placers: an orbit host's planets, from its architecture class, its disc and its stable
//! zone (plan 14, P14.T8; design notes 3, 4, 5, 7 and 9).
//!
//! [`place`] interprets the class's [`ClassTemplate`] inside the zone. Its inputs are plain
//! arguments, as plan 14's ordering note allows: the host's zero-age parameters and number
//! ([`PlacementHost`]), the zone's limits as a [`Truncation`], the host's [`Disc`] and the class
//! drawn for it (P14.T4.c). P14.T30.a adapts P14.T9's zones and P14.T1.d's context to them:
//! `zone.truncation()`, `ZoneDiscInputs::host()` and `zone.host_number()`.
//!
//! # What a host's planets are
//!
//! 1. **D5's second fallback** (ruling 55.3). A class with giants keeps them only if its disc grows
//!    a giant's core in time ([`giant_core`], P14.T7.c: 10 M⊕ of solids beyond the snow line, and
//!    a 10 M⊕ core grown there by pebble accretion within the disc's lifetime); otherwise it falls
//!    to its giant-free sibling, as D5's first fallback (P14.T4.c) does for a disc without the
//!    solids.
//! 2. **Groups, counts and slots.** Each group of the template is present with its presence share,
//!    and places a count drawn from its law (a chain takes its dynamically hot variant in 40% of
//!    systems). Its members take consecutive planet slots from `first_slot`, group by group inside
//!    out (design note 3), before anything is placed, so that their masses (P14.T7) and every other
//!    draw keyed by slot are fixed by the counts alone. A member that cannot be placed leaves its
//!    slot unused.
//! 3. **Positions** (P14.T8.a–c). A group whose template gives its first body a location (a period
//!    law, au × √L or snow-line radii) draws it inside the range its bounds allow, the law's shape
//!    kept (a range wholly inside the inner bound is held at it); a group placed "outward" or
//!    "flanking" starts from the planets before it. Each further member lies outward at a spacing
//!    drawn by P14.T6.b ([`draw_pair_spacing`]) against the floor of the pair's masses and the
//!    eccentricities it assumes. A member is placed only where it is admitted:
//!    - inside the zone's limits and the disc's edges, and inside the snow line for a group that
//!      stays inside it;
//!    - at least D7's circular floor from each neighbour already placed, and, towards a later
//!      group's first body, at D7's floor with that body's eccentricity and its own largest;
//!    - for a small planet, outside every giant's chaotic zone twice over, 2 × 1.3 (m ÷ M★)^(2⁄7)
//!      a either side (Wisdom 1980), and not between a migrated giant and where it formed, except
//!      in a flanking group, the template's exception;
//!    - for a hot Jupiter's companions, beyond the template's quiet period (100 days);
//!    - and for every planet, outside twice its host's Roche limit.
//!
//!    A group's walk ends at its count, the first member not admitted, or where
//!    [`next_semi_major_axis`] leaves no room.
//! 4. **Resonance** (P14.T8.a). A cold chain of three or more is resonant with probability 0.1
//!    (0.3 for hosts under 0.3 M☉). Inside out, each period ratio snaps to the nearest of 4:3, 3:2,
//!    5:3 and 2:1, wide of it by Fabrycky et al.'s (2014) excess, where the new position is
//!    admitted; otherwise the pair keeps its spacing.
//! 5. **Orbits** (P14.T8.d, [`orbits`]). Each planet's eccentricity follows its group's law
//!    truncated at its own limit, the largest whose periapsis stays outside the zone's inner limit
//!    and twice the Roche limit and whose apoapsis stays inside the zone's outer limit. Its
//!    inclination to its host's plane, node, periapsis and mean anomaly follow. Then D7 is
//!    checked on every adjacent pair with those eccentricities, and where it fails the outer
//!    body's eccentricity is scaled down until it holds (the inner body's too, should that not be
//!    enough), which is deterministic and needs no redraw.
//!
//! Tidal circularisation (P14.T8.e) is [`tides`]'s, for the fate transform (P14.T28).
//!
//! # Figures, re-checked
//!
//! - **Wisdom (1980, AJ 85, 1122, eq. 56):** resonances overlap within s ≃ 0.51 μ^(−2⁄7) of the
//!   planet, where a resonance lies at a ≃ 1 − 2 ÷ 3(s + 1), so the chaotic zone's half-width is
//!   δa ≈ 1.31 μ^(2⁄7) a; Chiang et al. (2009, ApJ 693, 734, eq. 1) print 1.3, as plan 14 does
//!   ([`CHAOTIC_ZONE_COEFFICIENT`]). Duncan, Quinn and Tremaine's (1989) 1.5 is the alternative
//!   Chiang et al. mention.
//! - **Ford and Rasio (2006, ApJ 638, L45, §1 and abstract):** hot Jupiters' inner edge lies
//!   "close to twice the Roche limit", a Roche limit of 2.16 R (M★ ÷ m)^⅓ for a planet of mass m
//!   and radius R ([`ROCHE_COEFFICIENT`]), their own definition, for R = 1.2 Jupiter radii. Here R
//!   is Chen and Kipping's median radius at the planet's mass (P14.T11.a), 1.23 Jupiter radii at a
//!   Jupiter mass, since the radius the derivation gives later depends on the age and the
//!   irradiation.
//! - **Fabrycky et al. (2014, ApJ 790, 146, §4, eq. 11):** Kepler's pairs show an excess at
//!   −0.2 < ζ₁ < −0.1 "just wide of first-order resonances", with the 3:2 pairs clustered at
//!   1.505–1.520. For a resonance (j + k) : j of order k, ζ = 3 (k ÷ (P₂ ÷ P₁ − 1) − j) gives an
//!   offset P₂ ÷ P₁ = (1 + ε) (j + k) ÷ j with ε = k|ζ| ÷ 3j(j + k): 0.28–0.56% at 4:3, 0.56–1.11%
//!   at 3:2, 0.44–0.89% at 5:3 and 1.67–3.33% at 2:1 ([`Commensurability::offset`]). Plan 14 had
//!   0.5–2% for all four; Fabrycky et al. find no excess near second-order resonances, and 5:3
//!   takes the same ζ.
//!
//! # Draws
//!
//! On [`tags::PLANET_COUNT`] (`System`), keyed by the system's ID: orbit host h reads words 64h
//! onwards ([`HostDraws`]): word 0 the hot variant's mark, word 1 the resonance's, and for group
//! g, words 8 + 8g onwards its presence's mark, its count (a mark, or its rank for a Poisson
//! law), its first body's location's rank and a flanking group's side; the rest are reserved. The
//! pair whose outer planet is in slot s reads its resonance offset at word 16,384 + 4s
//! ([`resonance_offset`]). The spacings are [`tags::PLANET_SPACING`]'s (P14.T6.b), keyed by the
//! pair's outer planet, and the masses [`tags::PLANET_MASS`]'s (P14.T7). Each planet's own orbit
//! is on [`tags::PLANET_ORBIT`] and its host's plane on [`tags::PLANET_PLANE`] ([`orbits`]).

pub mod orbits;
pub mod tides;

use core::f64::consts::{SQRT_2, TAU};

use orbits::{HostPlane, OrbitDraws, SystemPlane};

use crate::id::SystemId;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements};
use crate::planetary::architecture::template::{
    ClassTemplate, CountLaw, EccentricityLaw, GroupRole, Location, Origin, PeriodLaw, PlanetGroup,
    Reach, SpacingFamily, template,
};
use crate::planetary::architecture::{ArchitectureClass, ZoneLimit};
use crate::planetary::derive::composition::SnowLineSide;
use crate::planetary::derive::radius::radius_chen_kipping;
use crate::planetary::disc::{Disc, DiscHost, DiscProfile, Truncation};
use crate::planetary::index::{
    BLOCK_LEN, BodyIndex, BodySlot, BodySub, LAST_PLANET_SLOT, SECOND_GENERATION_SLOT_START,
};
use crate::planetary::params::SPACING_GIANT_MASS;
use crate::planetary::placement::masses::{GiantCore, giant_core, group_masses};
use crate::planetary::placement::spacing::{
    MAX_SPACING_STEP, Neighbour, SpacingDraws, SpacingKind, draw_pair_spacing, mutual_hill_factor,
    next_semi_major_axis, satisfies_floor, spacing_floor,
};
use crate::rng::{Mark, ObjectKey, Seed, Stream, Threshold, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::consts::{EARTH_MASS_KG, GM_EARTH, GM_SUN, SECONDS_PER_DAY, SOLAR_MASS_KG};
use crate::units::{Days, EarthMasses, GravitationalParameter, Metres, SolarMasses};

/// Words of the [`tags::PLANET_COUNT`] stream that one orbit host owns: host h reads words 64h to
/// 64h + 63. 256 hosts fill words 0 to 16,383.
pub const COUNT_WORDS_PER_HOST: u64 = 64;

/// The first word of group g's block in a host's words: 8 + 8g ([`COUNT_WORDS_PER_GROUP`]).
pub const COUNT_GROUP_WORDS_START: u64 = 8;

/// Words of one group's block: its presence's mark, its count, its location's rank and a
/// flanking group's side, then four reserved.
pub const COUNT_WORDS_PER_GROUP: u64 = 8;

/// The most groups a class template has: 4, so that every host's group blocks fit its 64 words.
pub const MAX_TEMPLATE_GROUPS: usize = 4;

/// The first word of the pairs' block of the [`tags::PLANET_COUNT`] stream: 16,384, the end of
/// the 256 hosts' words.
pub const COUNT_PAIR_WORDS_START: u64 = 256 * COUNT_WORDS_PER_HOST;

/// Words of the pairs' block that the pair whose outer planet is in slot s owns: 4, from word
/// 16,384 + 4s, of which the first is its resonance offset's rank.
pub const COUNT_WORDS_PER_PAIR: u64 = 4;

const _: () = {
    assert!(
        COUNT_GROUP_WORDS_START + COUNT_WORDS_PER_GROUP * (MAX_TEMPLATE_GROUPS as u64)
            <= COUNT_WORDS_PER_HOST,
        "every group's block fits its host's words"
    );
    let templates = &crate::planetary::architecture::template::TEMPLATES;
    let mut i = 0;
    while i < templates.len() {
        assert!(
            templates[i].groups().len() <= MAX_TEMPLATE_GROUPS,
            "no template has more groups than a host's words hold"
        );
        i += 1;
    }
};

/// The share of cold chains of three or more that are resonant: 0.1 (plan 14, P14.T8.a).
pub const RESONANT_CHAIN_PROBABILITY: f64 = 0.1;

/// The share of cold chains of three or more that are resonant around hosts under
/// [`LOW_MASS_RESONANCE_LIMIT`]: 0.3 (plan 14, P14.T8.a; TRAPPIST-1 is the limiting case).
pub const LOW_MASS_RESONANT_CHAIN_PROBABILITY: f64 = 0.3;

/// The host mass under which a chain is resonant with [`LOW_MASS_RESONANT_CHAIN_PROBABILITY`]:
/// 0.3 M☉ (plan 14, P14.T8.a).
pub const LOW_MASS_RESONANCE_LIMIT: SolarMasses = SolarMasses::new(0.3);

/// The fewest planets a chain needs to be resonant: 3 (plan 14, P14.T8.a).
pub const MIN_RESONANT_CHAIN: usize = 3;

/// The range of Fabrycky et al.'s (2014, §4) ζ over which a resonant pair's offset is drawn,
/// uniform: 0.1–0.2, their excess "just wide of first-order resonances".
pub const RESONANCE_OFFSET_ZETA: (f64, f64) = (0.1, 0.2);

/// The chaotic zone's half-width about a planet of mass ratio μ, in units of its semi-major axis:
/// 1.3 μ^(2⁄7) (Wisdom 1980, eq. 56; Chiang et al. 2009, eq. 1).
pub const CHAOTIC_ZONE_COEFFICIENT: f64 = 1.3;

/// How many chaotic-zone half-widths either side of a giant are kept free of small planets: 2
/// (plan 14, P14.T8.b).
pub const CHAOTIC_ZONE_GAP: f64 = 2.0;

/// The coefficient of a planet's Roche limit about its host, 2.16 R (M★ ÷ m)^⅓ for a planet of mass
/// m and radius R: Ford and Rasio's (2006, §1) R = 0.462 × (the limit) × (m ÷ M★)^⅓.
pub const ROCHE_COEFFICIENT: f64 = 2.16;

/// How many Roche limits a planet keeps from its host at periapsis: 2, the inner edge of the hot
/// Jupiters (Ford and Rasio 2006, abstract).
pub const ROCHE_CLEARANCE: f64 = 2.0;

/// The share of flanking groups whose first body lies inside the planet it flanks: 0.5, this
/// module's choice; the second, if any, goes to the other side (Huang et al. 2016: warm Jupiters
/// "closely flanked by small companions").
pub const FLANK_INSIDE_PROBABILITY: f64 = 0.5;

/// An orbit host as the placer takes it: its number in the system's draws and its zero-age
/// parameters, with its plane (P14.T8).
///
/// The zero-age parameters are the disc's host's ([`DiscHost`]: mass, \[Fe/H\], luminosity and
/// radius), which P14.T9's `ZoneDiscInputs::host` gives for every zone. The host number is the
/// zone's (`OrbitZone::host_number`): a single star is host 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementHost {
    number: u8,
    zams: DiscHost,
    plane: HostPlane,
}

impl PlacementHost {
    /// The host numbered `number`, of zero-age parameters `zams`, whose planets orbit in `plane`:
    /// [`HostPlane::Isotropic`], or a close binary's plane for its circumbinary zone.
    #[must_use]
    pub const fn new(number: u8, zams: DiscHost, plane: HostPlane) -> Self {
        Self {
            number,
            zams,
            plane,
        }
    }

    /// The host's number in the system's draws.
    #[must_use]
    pub const fn number(&self) -> u8 {
        self.number
    }

    /// The host's zero-age parameters.
    #[must_use]
    pub const fn zams(&self) -> &DiscHost {
        &self.zams
    }

    /// The host's plane.
    #[must_use]
    pub const fn plane(&self) -> HostPlane {
        self.plane
    }
}

/// A first-order or second-order commensurability a resonant chain's pair can sit wide of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Commensurability {
    /// 4:3.
    FourToThree,
    /// 3:2.
    ThreeToTwo,
    /// 5:3.
    FiveToThree,
    /// 2:1.
    TwoToOne,
}

impl Commensurability {
    /// Every commensurability, in order of period ratio.
    pub const ALL: [Self; 4] = [
        Self::FourToThree,
        Self::ThreeToTwo,
        Self::FiveToThree,
        Self::TwoToOne,
    ];

    /// The outer and inner periods' integers, (j + k, j).
    #[must_use]
    pub const fn integers(self) -> (u8, u8) {
        match self {
            Self::FourToThree => (4, 3),
            Self::ThreeToTwo => (3, 2),
            Self::FiveToThree => (5, 3),
            Self::TwoToOne => (2, 1),
        }
    }

    /// The exact period ratio, (j + k) ÷ j.
    #[must_use]
    pub fn ratio(self) -> f64 {
        let (outer, inner) = self.integers();
        f64::from(outer) / f64::from(inner)
    }

    /// The fractional offset ε of a pair at Fabrycky et al.'s (2014, eq. 11) |ζ| wide of this
    /// commensurability: k|ζ| ÷ 3j(j + k), for a period ratio of (1 + ε) (j + k) ÷ j.
    #[must_use]
    pub fn offset(self, zeta: f64) -> f64 {
        let (outer, inner) = self.integers();
        let (p, j) = (f64::from(outer), f64::from(inner));
        (p - j) * zeta / (3.0 * j * p)
    }

    /// The commensurability nearest the period ratio `ratio`, in the logarithm.
    #[must_use]
    pub fn nearest(ratio: f64) -> Self {
        let distance = |c: Self| (math::ln(ratio) - math::ln(c.ratio())).abs();
        let mut best = Self::FourToThree;
        for c in Self::ALL {
            if distance(c) < distance(best) {
                best = c;
            }
        }
        best
    }
}

/// Where a resonant pair sits: wide of `commensurability` by the fraction `offset` of the period
/// ratio, measured against the planet inside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resonance {
    commensurability: Commensurability,
    offset: f64,
}

impl Resonance {
    /// The commensurability.
    #[must_use]
    pub const fn commensurability(&self) -> Commensurability {
        self.commensurability
    }

    /// The fractional offset ε: the pair's period ratio is (1 + ε) times the commensurability's.
    #[must_use]
    pub const fn offset(&self) -> f64 {
        self.offset
    }
}

/// One planet as placed (P14.T8): its index, where it came from in its template, its mass and its
/// primordial orbit about its host.
///
/// Its mass, orbit and formation distance are what the derivation reads (P14.T16.a's
/// `PlacedBody`, with the radius rank that P14.T30 draws on the planet's own `planet.radius`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedPlanet {
    index: BodyIndex,
    group: u8,
    role: GroupRole,
    mass: EarthMasses,
    orbit: KeplerElements,
    formation_distance: Metres,
    formed: SnowLineSide,
    origin: Origin,
    hot: bool,
    resonance: Option<Resonance>,
    drawn_eccentricity: f64,
    rescaled: bool,
}

impl PlacedPlanet {
    /// The planet's body index, in its slot.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// The position of the planet's group in its class template.
    #[must_use]
    pub const fn group(&self) -> u8 {
        self.group
    }

    /// The planet's group's role.
    #[must_use]
    pub const fn role(&self) -> GroupRole {
        self.role
    }

    /// The planet's mass (P14.T7).
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The planet's primordial orbit about its host, in the system frame, with μ = G (M★ + m).
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// How far from its host the planet formed: its semi-major axis, or for a giant placed inside
    /// the site where giants' cores grow ([`giant_core`]'s: the snow line, or the disc's inner
    /// edge beyond it), that site.
    #[must_use]
    pub const fn formation_distance(&self) -> Metres {
        self.formation_distance
    }

    /// Whether the planet formed beyond its disc's snow line (P14.T4.a's `formed_beyond_snow_line`),
    /// which fixes its composition however far it moved.
    #[must_use]
    pub const fn formed_beyond_snow_line(&self) -> bool {
        matches!(self.formed, SnowLineSide::Beyond)
    }

    /// The side of its disc's snow line the planet formed on, as the composition solve takes it
    /// (P14.T11.c).
    #[must_use]
    pub const fn formed(&self) -> SnowLineSide {
        self.formed
    }

    /// Where the planet's group formed, as its template marks it.
    #[must_use]
    pub const fn origin(&self) -> Origin {
        self.origin
    }

    /// Whether the planet's group is marked migrated in its template.
    #[must_use]
    pub const fn migrated(&self) -> bool {
        self.origin.is_migrated()
    }

    /// Whether the planet belongs to a chain's dynamically hot variant.
    #[must_use]
    pub const fn hot(&self) -> bool {
        self.hot
    }

    /// Where the planet sits against the planet inside it, if its chain is resonant and the pair
    /// was snapped.
    #[must_use]
    pub const fn resonance(&self) -> Option<Resonance> {
        self.resonance
    }

    /// The eccentricity as drawn from the group's law, truncated at the planet's own limit, before
    /// D7's re-check.
    #[must_use]
    pub const fn drawn_eccentricity(&self) -> f64 {
        self.drawn_eccentricity
    }

    /// Whether D7's re-check scaled the drawn eccentricity down.
    #[must_use]
    pub const fn rescaled(&self) -> bool {
        self.rescaled
    }
}

/// An orbit host's planets (P14.T8): its class as drawn and as kept, and its planets in slot
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct HostPlacement {
    drawn: ArchitectureClass,
    class: ArchitectureClass,
    core: Option<GiantCore>,
    planets: Vec<PlacedPlanet>,
    next_slot: u8,
}

impl HostPlacement {
    /// The class drawn for the host (P14.T4.c).
    #[must_use]
    pub const fn drawn_class(&self) -> ArchitectureClass {
        self.drawn
    }

    /// The class placed: the drawn one, or its giant-free sibling where the disc grows no giant's
    /// core in time (D5's second fallback), or `Barren` without a disc.
    #[must_use]
    pub const fn class(&self) -> ArchitectureClass {
        self.class
    }

    /// Whether the disc grows a giant's core, for a drawn class with giants.
    #[must_use]
    pub const fn core(&self) -> Option<GiantCore> {
        self.core
    }

    /// The planets placed, in slot order.
    #[must_use]
    pub fn planets(&self) -> &[PlacedPlanet] {
        &self.planets
    }

    /// The first slot after this host's: where the next host's planets begin.
    #[must_use]
    pub const fn next_slot(&self) -> u8 {
        self.next_slot
    }
}

/// One group's draws on [`tags::PLANET_COUNT`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupDraws {
    /// Whether the group is present, against its presence share. Word 8 + 8g of the host's.
    pub presence: Mark,
    /// Its count, by integer thresholds for a uniform law. Word 9 + 8g.
    pub count: Mark,
    /// The same word as a rank, for a Poisson law.
    pub count_rank: UnitUniform,
    /// The rank of its first body's location in its law. Word 10 + 8g.
    pub location: UnitUniform,
    /// A flanking group's side: inside the planet it flanks, below
    /// [`FLANK_INSIDE_PROBABILITY`]. Word 11 + 8g.
    pub side: Mark,
}

/// An orbit host's group-level draws on [`tags::PLANET_COUNT`] (P14.T8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostDraws {
    /// Whether a chain takes its dynamically hot variant. Word 0 of the host's.
    pub hot_variant: Mark,
    /// Whether a cold chain of three or more is resonant. Word 1.
    pub resonance: Mark,
    /// Each group's draws, in template order.
    pub groups: [GroupDraws; MAX_TEMPLATE_GROUPS],
}

impl HostDraws {
    /// The draws of orbit host number `host` of `system`, in the universe of `seed`: words
    /// [`COUNT_WORDS_PER_HOST`] × `host` onwards of `system`'s [`tags::PLANET_COUNT`] stream.
    ///
    /// # Panics
    ///
    /// Never: a group's position, at most [`MAX_TEMPLATE_GROUPS`], fits a `u64`.
    #[must_use]
    pub fn for_host(seed: Seed, system: SystemId, host: u8) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_COUNT, ObjectKey::from(system));
        let start = u64::from(host) * COUNT_WORDS_PER_HOST;
        let mark = |n: u64| Mark::from_word(stream.word_at(start + n));
        let hot_variant = mark(0);
        let resonance = mark(1);
        let groups = core::array::from_fn(|g| {
            let g = u64::try_from(g).expect("a group's position fits a u64");
            let block = COUNT_GROUP_WORDS_START + COUNT_WORDS_PER_GROUP * g;
            stream.seek(start + block + 1);
            let count_rank = rank(stream.uniform_open());
            let location = rank(stream.uniform_open());
            GroupDraws {
                presence: Mark::from_word(stream.word_at(start + block)),
                count: Mark::from_word(stream.word_at(start + block + 1)),
                count_rank,
                location,
                side: Mark::from_word(stream.word_at(start + block + 3)),
            }
        });
        Self {
            hot_variant,
            resonance,
            groups,
        }
    }
}

/// An open uniform as a rank.
#[must_use]
fn rank(u: f64) -> UnitUniform {
    UnitUniform::new(u).expect("an open uniform lies strictly between 0 and 1")
}

/// The rank of the resonance offset of the pair whose outer planet is `outer`: word 16,384 + 4s
/// of [`tags::PLANET_COUNT`], s being its slot.
#[must_use]
pub fn resonance_offset(seed: Seed, system: SystemId, outer: BodyIndex) -> UnitUniform {
    let mut stream = Stream::open(seed, tags::PLANET_COUNT, ObjectKey::from(system));
    stream.seek(COUNT_PAIR_WORDS_START + u64::from(outer.get() >> 8) * COUNT_WORDS_PER_PAIR);
    rank(stream.uniform_open())
}

/// D5's second fallback (ruling 55.3): `class`, if its disc `disc` in a zone ending at `zone`
/// grows a giant's core in time or the class has no giants, and otherwise its giant-free sibling;
/// with the core's verdict for a class with giants.
///
/// # Examples
///
/// A Sun-like host whose disc is cut inside its snow line keeps no giant:
///
/// ```
/// use hyperion_sim::planetary::architecture::{ArchitectureClass, ZoneLimit};
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::classes::core_fallback;
/// use hyperion_sim::units::{
///     AstronomicalUnits, Dex, Megayears, Metres, SolarLuminosities, SolarMasses, SolarRadii,
/// };
///
/// let sun = DiscHost::new(
///     SolarMasses::new(1.0),
///     Dex::new(0.0),
///     SolarLuminosities::new(0.70),
///     SolarRadii::new(0.89),
/// )?;
/// let median = disc::derive(&sun, Megayears::new(2.5), &DiscDraws::MEDIAN, Truncation::NONE);
/// let (kept, core) = core_fallback(ArchitectureClass::SolarLike, &median, ZoneLimit::Unbounded);
/// assert_eq!(kept, ArchitectureClass::SolarLike);
/// assert!(core.is_some_and(|c| c.forms()));
///
/// let two_au = ZoneLimit::Outer(Metres::from(AstronomicalUnits::new(2.0)));
/// let (fallen, _) = core_fallback(ArchitectureClass::SolarLike, &median, two_au);
/// assert_eq!(fallen, ArchitectureClass::TerrestrialOnly);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn core_fallback(
    class: ArchitectureClass,
    disc: &Disc,
    zone: ZoneLimit,
) -> (ArchitectureClass, Option<GiantCore>) {
    if !class.has_giants() {
        return (class, None);
    }
    let core = giant_core(disc, zone);
    if core.forms() {
        (class, Some(core))
    } else {
        (class.giant_free_sibling(), Some(core))
    }
}

/// The planets of orbit host `host` of `system`, in the universe of `seed`, drawn class `class`,
/// with disc `disc` inside the zone limits `limits`, in planet slots from `first_slot` (P14.T8;
/// see the [module documentation](self)).
///
/// `first_slot` is the host's first planet slot: 1 for the first host of a system, and the
/// previous host's [`HostPlacement::next_slot`] after it, so that slots run host by host in
/// hierarchy order (design note 3); a second-generation host (P14.T28.e) starts at `0xC0`. No
/// planet is placed past its block's last slot, 191 or `0xCF`.
///
/// A host without a disc places nothing and is `Barren`.
///
/// # Panics
///
/// In debug builds, if `first_slot` is 0.
///
/// # Examples
///
/// A Sun-like star whose median disc lives 2.5 Myr, drawn `SolarLike`: rocky planets inside the
/// snow line, and giants beyond it, every pair spaced beyond D7's floor.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::architecture::ArchitectureClass;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::classes::orbits::HostPlane;
/// use hyperion_sim::planetary::placement::classes::{PlacementHost, place};
/// use hyperion_sim::planetary::placement::{Neighbour, satisfies_floor};
/// use hyperion_sim::units::{Dex, Megayears, SolarLuminosities, SolarMasses, SolarRadii};
///
/// let (seed, system) = (Seed::new(7), SystemId::from_raw(0x0200_0800_2000_0000)?);
/// let sun = DiscHost::new(
///     SolarMasses::new(1.0),
///     Dex::new(0.0),
///     SolarLuminosities::new(0.70),
///     SolarRadii::new(0.89),
/// )?;
/// let disc = disc::derive(&sun, Megayears::new(2.5), &DiscDraws::MEDIAN, Truncation::NONE);
/// let host = PlacementHost::new(0, sun, HostPlane::Isotropic);
/// let placed = place(seed, system, &host, Truncation::NONE, &disc, ArchitectureClass::SolarLike, 1);
/// assert_eq!(placed.class(), ArchitectureClass::SolarLike);
///
/// let mut planets = placed.planets().to_vec();
/// planets.sort_by(|a, b| a.orbit().semi_major_axis().total_cmp(&b.orbit().semi_major_axis()));
/// let snow = disc.profile().expect("a disc").snow_line();
/// assert!(planets.iter().any(|p| p.orbit().semi_major_axis() > snow));
/// let neighbour = |p: &hyperion_sim::planetary::placement::classes::PlacedPlanet| {
///     Neighbour::new(p.mass(), p.orbit().semi_major_axis(), p.orbit().eccentricity().value())
/// };
/// for pair in planets.windows(2) {
///     assert!(satisfies_floor(&neighbour(&pair[0]), &neighbour(&pair[1]), sun.mass()));
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn place(
    seed: Seed,
    system: SystemId,
    host: &PlacementHost,
    limits: Truncation,
    disc: &Disc,
    class: ArchitectureClass,
    first_slot: u8,
) -> HostPlacement {
    debug_assert!(first_slot > 0, "slot 0 is the stellar level's");
    let zone = limits
        .outer()
        .map_or(ZoneLimit::Unbounded, ZoneLimit::Outer);
    let Some(profile) = disc.profile() else {
        return HostPlacement {
            drawn: class,
            class: ArchitectureClass::Barren,
            core: None,
            planets: Vec::new(),
            next_slot: first_slot,
        };
    };
    let (kept, core) = core_fallback(class, disc, zone);
    let mut placer = Placer::new(seed, system, host, limits, profile, template(kept));
    let next_slot = placer.reserve(first_slot);
    placer.walk();
    let planets = placer.finish();
    HostPlacement {
        drawn: class,
        class: kept,
        core,
        planets,
        next_slot,
    }
}

/// The last slot of the block `first` lies in, or `None` past the planets' blocks.
#[must_use]
fn last_slot(first: u8) -> Option<u8> {
    if first <= LAST_PLANET_SLOT {
        Some(LAST_PLANET_SLOT)
    } else if first < SECOND_GENERATION_SLOT_START + BLOCK_LEN {
        (first >= SECOND_GENERATION_SLOT_START)
            .then_some(SECOND_GENERATION_SLOT_START + BLOCK_LEN - 1)
    } else {
        None
    }
}

/// The planet's own index in slot `slot`.
#[must_use]
fn planet_index(slot: u8) -> BodyIndex {
    let slot = if slot <= LAST_PLANET_SLOT {
        BodySlot::Planet(slot)
    } else {
        BodySlot::SecondGeneration(slot - SECOND_GENERATION_SLOT_START)
    };
    BodyIndex::new(slot, BodySub::Primary).expect("the slot lies in a planets' block")
}

/// The spacing law's kind for a group's family.
#[must_use]
const fn spacing_kind(family: SpacingFamily) -> SpacingKind {
    match family {
        SpacingFamily::SmallPlanets => SpacingKind::SmallPlanets,
        SpacingFamily::Terrestrial => SpacingKind::TerrestrialGroup,
        SpacingFamily::Giants => SpacingKind::GiantPair,
    }
}

/// The count of a law at its draws, for a host of `mass`.
#[must_use]
fn draw_count(law: CountLaw, mass: SolarMasses, draws: &GroupDraws) -> u8 {
    match law {
        CountLaw::Uniform { min, max } => {
            let span = u128::from(max - min) + 1;
            let step = (u128::from(draws.count.get()) * span) >> 53;
            min + u8::try_from(step).expect("the step lies below the span, at most 256")
        }
        CountLaw::ZeroTruncatedPoisson { max, .. } => {
            let lambda = law.poisson_rate(mass).expect("a Poisson law has a rate");
            let zero = math::exp(-lambda);
            let target = zero + draws.count_rank.value() * (1.0 - zero);
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
    }
}

/// One reserved member of a group: its slot, mass and draws.
#[derive(Debug, Clone, Copy)]
struct Member {
    index: BodyIndex,
    group: usize,
    mass: EarthMasses,
    draws: OrbitDraws,
    law: EccentricityLaw,
    cold_chain: bool,
    hot: bool,
}

impl Member {
    fn is_giant(&self) -> bool {
        self.mass >= EarthMasses::from(SPACING_GIANT_MASS)
    }
}

/// A member at a semi-major axis, with the eccentricity it has there.
#[derive(Debug, Clone, Copy)]
struct Planet {
    member: usize,
    a: Metres,
    e: f64,
    resonance: Option<Resonance>,
}

/// One group as reserved.
#[derive(Debug, Clone)]
struct Group {
    template: PlanetGroup,
    members: Vec<usize>,
    anchor: Option<Metres>,
}

/// The state of one host's placement.
struct Placer<'a> {
    seed: Seed,
    system: SystemId,
    host: &'a PlacementHost,
    disc: &'a DiscProfile,
    template: &'static ClassTemplate,
    draws: HostDraws,
    spacing: SpacingDraws,
    zone_inner: Option<Metres>,
    zone_outer: Option<Metres>,
    lo: Metres,
    hi: Metres,
    core_site: Metres,
    members: Vec<Member>,
    groups: Vec<Group>,
    placed: Vec<Planet>,
}

impl<'a> Placer<'a> {
    fn new(
        seed: Seed,
        system: SystemId,
        host: &'a PlacementHost,
        limits: Truncation,
        disc: &'a DiscProfile,
        template: &'static ClassTemplate,
    ) -> Self {
        let lo = limits
            .inner()
            .map_or(disc.inner_edge(), |r| r.max_of(disc.inner_edge()));
        let hi = limits
            .outer()
            .map_or(disc.outer_edge(), |r| r.min_of(disc.outer_edge()));
        let core_site = disc.snow_line().max_of(disc.inner_edge());
        Self {
            seed,
            system,
            host,
            disc,
            template,
            draws: HostDraws::for_host(seed, system, host.number()),
            spacing: SpacingDraws::for_host(seed, system, host.number()),
            zone_inner: limits.inner(),
            zone_outer: limits.outer(),
            lo,
            hi,
            core_site,
            members: Vec::new(),
            groups: Vec::new(),
            placed: Vec::new(),
        }
    }

    fn host_mass(&self) -> SolarMasses {
        self.host.zams().mass()
    }

    /// The groups' presence, counts and slots from `first_slot`, their masses and draws, and the
    /// first bodies' anchors; returns the first slot after them.
    fn reserve(&mut self, first_slot: u8) -> u8 {
        let mass = self.host_mass();
        let mut slot = first_slot;
        let last = last_slot(first_slot);
        for (g, group) in self.template.groups().iter().enumerate() {
            let draws = self.draws.groups[g];
            let present = draws
                .presence
                .is_below(Threshold::from_probability(group.presence()));
            let hot = group.hot_variant().filter(|variant| {
                self.draws
                    .hot_variant
                    .is_below(Threshold::from_probability(variant.probability()))
            });
            let (count, law) = match hot {
                Some(variant) => (variant.count(), variant.eccentricity()),
                None => (group.count(), group.eccentricity()),
            };
            let wanted = if present {
                draw_count(count, mass, &draws)
            } else {
                0
            };
            let room = last.map_or(0, |last| {
                u16::from(last)
                    .saturating_add(1)
                    .saturating_sub(u16::from(slot))
            });
            let n = u8::try_from(u16::from(wanted).min(room)).expect("at most a count");
            let indices: Vec<BodyIndex> = (slot..slot + n).map(planet_index).collect();
            slot += n;
            let masses = if indices.is_empty() {
                Vec::new()
            } else {
                group_masses(self.seed, self.system, group, self.disc, &indices)
                    .masses()
                    .to_vec()
            };
            let cold_chain = group.role() == GroupRole::Chain && hot.is_none();
            let members = indices
                .iter()
                .zip(masses)
                .map(|(&index, mass)| {
                    self.members.push(Member {
                        index,
                        group: g,
                        mass,
                        draws: OrbitDraws::for_planet(self.seed, self.system, index),
                        law,
                        cold_chain,
                        hot: hot.is_some(),
                    });
                    self.members.len() - 1
                })
                .collect::<Vec<_>>();
            self.groups.push(Group {
                template: *group,
                members,
                anchor: None,
            });
        }
        // Last group first, so that a small group's first body is drawn inside the gaps of the
        // giants anchored beyond it.
        for g in (0..self.groups.len()).rev() {
            let anchor = self.groups[g].members.first().and_then(|&first| {
                let (lo, hi) = self.bounds(g, first);
                let hi = if self.members[first].is_giant() {
                    hi
                } else {
                    self.groups[g + 1..]
                        .iter()
                        .filter_map(|later| {
                            let giant = &self.members[*later.members.first()?];
                            let a = later.anchor?;
                            giant.is_giant().then(|| {
                                a * (1.0 - CHAOTIC_ZONE_GAP * self.chaotic_zone(giant.mass))
                            })
                        })
                        .fold(hi, Extremes::min_of)
                };
                self.anchor(g, first, lo, hi)
            });
            self.groups[g].anchor = anchor;
        }
        slot
    }

    /// The gravitational parameter of a member's orbit, G (M★ + m).
    fn mu(&self, member: usize) -> GravitationalParameter {
        GravitationalParameter::new(
            GM_SUN * self.host_mass().value() + GM_EARTH * self.members[member].mass.value(),
        )
    }

    /// A member's period at semi-major axis `a`.
    fn period(&self, member: usize, a: Metres) -> Days {
        let a = a.value();
        Days::new(TAU * (a * (a / self.mu(member).value()).sqrt()) / SECONDS_PER_DAY)
    }

    /// A member's semi-major axis at period `period`.
    fn axis(&self, member: usize, period: Days) -> Metres {
        let per_radian = period.value() * SECONDS_PER_DAY / TAU;
        Metres::new(math::cbrt(
            self.mu(member).value() * per_radian * per_radian,
        ))
    }

    /// Twice a member's Roche limit about the host, from Chen and Kipping's median radius at its
    /// mass (Ford and Rasio 2006).
    fn roche_floor(&self, member: usize) -> Metres {
        let mass = self.members[member].mass;
        let radius = Metres::from(radius_chen_kipping(mass, UnitUniform::HALF));
        let ratio = self.host_mass().value() * SOLAR_MASS_KG / (mass.value() * EARTH_MASS_KG);
        radius * (ROCHE_CLEARANCE * ROCHE_COEFFICIENT * math::cbrt(ratio))
    }

    /// The bounds of a member's semi-major axis in group `g`.
    fn bounds(&self, g: usize, member: usize) -> (Metres, Metres) {
        let group = &self.groups[g].template;
        // A giant that migrated to an orbit drawn by period, the hot and warm Jupiters, arrives
        // by high-eccentricity migration and tidal circularisation, whose inner edge is twice the
        // Roche limit (Ford and Rasio 2006), inside the gas disc's magnetospheric cavity: the
        // disc's inner edge does not hold it.
        let inside_cavity = group.places_giants()
            && group.origin().is_migrated()
            && matches!(group.location(), Location::Period(_));
        let floor = if inside_cavity {
            self.zone_inner.unwrap_or(Metres::ZERO)
        } else {
            self.lo
        };
        let mut lo = floor.max_of(self.roche_floor(member));
        if g > 0
            && let Some(quiet) = self.template.quiet_inside()
        {
            lo = lo.max_of(self.axis(member, quiet));
        }
        let hi = match group.reach() {
            Reach::InsideSnowLine => self.hi.min_of(self.disc.snow_line()),
            Reach::Open => self.hi,
        };
        (lo, hi)
    }

    /// The largest eccentricity of a member at `a`: its periapsis outside the zone's inner limit
    /// and twice its Roche limit, its apoapsis inside the zone's outer limit.
    fn eccentricity_limit(&self, member: usize, a: Metres) -> f64 {
        let q = self.zone_inner.map_or(self.roche_floor(member), |r| {
            r.max_of(self.roche_floor(member))
        });
        let mut limit = 1.0 - q / a;
        if let Some(outer) = self.zone_outer {
            limit = limit.min(outer / a - 1.0);
        }
        limit.max(0.0)
    }

    /// A member's eccentricity at `a`.
    fn eccentricity_at(&self, member: usize, a: Metres) -> f64 {
        let m = &self.members[member];
        orbits::eccentricity(
            m.law,
            m.draws.eccentricity,
            self.period(member, a),
            self.eccentricity_limit(member, a),
        )
    }

    /// The first body of group `g`, its `first` member, from its location's law inside `lo`–`hi`,
    /// or `None` for a group placed from the planets before it or with no room.
    fn anchor(&self, g: usize, first: usize, lo: Metres, hi: Metres) -> Option<Metres> {
        if lo >= hi {
            return None;
        }
        let u = self.draws.groups[g].location.value();
        let group = &self.groups[g].template;
        let root_l = self.host.zams().zams_luminosity().value().sqrt();
        let au = |x: f64| Metres::new(x * crate::units::consts::METRES_PER_AU);
        let log_uniform = |x0: Metres, x1: Metres| -> Option<Metres> {
            if x1 <= lo {
                return Some(lo);
            }
            if x0 >= hi {
                return None;
            }
            let (a, b) = (x0.max_of(lo), x1.min_of(hi));
            let (la, lb) = (math::ln(a.value()), math::ln(b.value()));
            Some(Metres::new(math::exp(la + u * (lb - la))))
        };
        match group.location() {
            Location::ScaledAu { inner, outer } => {
                log_uniform(au(inner * root_l), au(outer * root_l))
            }
            Location::SnowLines { inner, outer } => {
                let snow = self.disc.snow_line();
                log_uniform(snow * inner, snow * outer)
            }
            Location::Period(law) => {
                let (p_lo, p_hi) = (self.period(first, lo), self.period(first, hi));
                let (min, max) = law.range();
                if max <= p_lo {
                    return Some(lo);
                }
                if min >= p_hi {
                    return None;
                }
                let (a, b) = (min.max_of(p_lo), max.min_of(p_hi));
                let period = truncated_period(law, a, b, u);
                Some(self.axis(first, period).max_of(lo).min_of(hi))
            }
            Location::Outward | Location::Flanking => None,
        }
    }

    /// Every group, inside out.
    fn walk(&mut self) {
        for g in 0..self.groups.len() {
            let Some(&first) = self.groups[g].members.first() else {
                continue;
            };
            let group = self.groups[g].template;
            let start = match group.location() {
                Location::Outward => {
                    let Some(outermost) = self.outermost() else {
                        continue;
                    };
                    self.spaced(outermost, first, Side::Outside)
                }
                Location::Flanking => {
                    let Some(flanked) = self.flanked(g) else {
                        continue;
                    };
                    let side = if self.draws.groups[g]
                        .side
                        .is_below(Threshold::from_probability(FLANK_INSIDE_PROBABILITY))
                    {
                        Side::Inside
                    } else {
                        Side::Outside
                    };
                    self.spaced(flanked, first, side)
                }
                Location::Period(_) | Location::ScaledAu { .. } | Location::SnowLines { .. } => {
                    self.groups[g].anchor
                }
            };
            let Some(a) = start else {
                continue;
            };
            if !self.admits(g, first, a) {
                continue;
            }
            self.push(first, a);
            let members = self.groups[g].members.clone();
            let mut previous = self.placed.len() - 1;
            let mut first_side = None;
            if matches!(group.location(), Location::Flanking) {
                let flanked = self.flanked(g).expect("the first member flanked it");
                first_side = Some(if self.placed[previous].a < self.placed[flanked].a {
                    Side::Inside
                } else {
                    Side::Outside
                });
            }
            for &member in &members[1..] {
                let next = match (first_side, group.location()) {
                    (Some(side), Location::Flanking) => {
                        let flanked = self.flanked(g).expect("the group flanks a planet");
                        self.spaced(flanked, member, side.other())
                    }
                    _ => self.spaced(previous, member, Side::Outside),
                };
                match next {
                    Some(a) if self.admits(g, member, a) => {
                        self.push(member, a);
                        previous = self.placed.len() - 1;
                    }
                    _ => break,
                }
            }
            if group.role() == GroupRole::Chain {
                self.resonate(g);
            }
        }
    }

    /// Places `member` at `a`.
    fn push(&mut self, member: usize, a: Metres) {
        let e = self.eccentricity_at(member, a);
        self.placed.push(Planet {
            member,
            a,
            e,
            resonance: None,
        });
    }

    /// The outermost planet placed so far.
    fn outermost(&self) -> Option<usize> {
        (0..self.placed.len()).max_by(|&i, &j| self.placed[i].a.total_cmp(&self.placed[j].a))
    }

    /// The planet group `g` flanks: the previous group's first planet placed.
    fn flanked(&self, g: usize) -> Option<usize> {
        let previous = g.checked_sub(1)?;
        self.placed
            .iter()
            .position(|p| self.members[p.member].group == previous)
    }

    /// Where `member` goes at a drawn spacing on `side` of placed planet `reference`.
    fn spaced(&self, reference: usize, member: usize, side: Side) -> Option<Metres> {
        let r = self.placed[reference];
        let (m_ref, m_new) = (self.members[r.member].mass, self.members[member].mass);
        let e_new = orbits::upper_eccentricity(
            self.members[member].law,
            self.members[member].draws.eccentricity,
        );
        let floor = spacing_floor(m_ref, m_new, r.e, e_new);
        let kind = spacing_kind(self.groups[self.members[member].group].template.spacing());
        let mean = self.spacing.mean_spacing(kind);
        let outer = match side {
            Side::Outside => self.members[member].index,
            Side::Inside => self.members[r.member].index,
        };
        let delta = draw_pair_spacing(self.seed, self.system, outer, mean, floor).spacing();
        let chi = mutual_hill_factor(m_ref, m_new, self.host_mass());
        match side {
            Side::Outside => next_semi_major_axis(r.a, delta, chi),
            Side::Inside => {
                let step = delta * chi.value();
                (step < MAX_SPACING_STEP).then(|| r.a * ((1.0 - step) / (1.0 + step)))
            }
        }
    }

    /// The later groups' first bodies not yet placed, as planets at their anchors.
    fn barriers(&self, after: usize) -> Vec<Planet> {
        self.groups
            .iter()
            .enumerate()
            .skip(after + 1)
            .filter_map(|(_, group)| {
                let first = *group.members.first()?;
                let a = group.anchor?;
                Some(Planet {
                    member: first,
                    a,
                    e: self.eccentricity_at(first, a),
                    resonance: None,
                })
            })
            .collect()
    }

    /// Whether `member` of group `g` may be placed at `a`, among the planets placed and the later
    /// groups' first bodies (see the [module documentation](self)).
    fn admits(&self, g: usize, member: usize, a: Metres) -> bool {
        self.admits_among(g, member, a, &self.placed)
    }

    fn admits_among(&self, g: usize, member: usize, a: Metres, placed: &[Planet]) -> bool {
        let (lo, hi) = self.bounds(g, member);
        if !(a >= lo && a <= hi) {
            return false;
        }
        let m = &self.members[member];
        let host = self.host_mass();
        let barriers = self.barriers(g);
        let others = placed
            .iter()
            .map(|p| (p, false))
            .chain(barriers.iter().map(|p| (p, true)));
        let mut inner: Option<(&Planet, bool)> = None;
        let mut outer: Option<(&Planet, bool)> = None;
        let flanking = matches!(self.groups[g].template.location(), Location::Flanking);
        for (other, is_barrier) in others {
            let o = &self.members[other.member];
            // A small planet outside every giant's chaotic zone twice over, and a giant with no
            // small planet inside its own.
            if m.is_giant() != o.is_giant() {
                let (giant, giant_a, small_a) = if m.is_giant() {
                    (m, a, other.a)
                } else {
                    (o, other.a, a)
                };
                let half = self.chaotic_zone(giant.mass);
                if (small_a.value() - giant_a.value()).abs()
                    <= CHAOTIC_ZONE_GAP * half * giant_a.value()
                {
                    return false;
                }
            }
            // No small planet between a migrated giant and where it formed, but in a flanking
            // group.
            let other_flanking =
                matches!(self.groups[o.group].template.location(), Location::Flanking);
            let site = self.core_site;
            let in_corridor = |giant_a: Metres, small_a: Metres| {
                giant_a < site && small_a > giant_a && small_a < site
            };
            if !m.is_giant() && !flanking && o.is_giant() && in_corridor(other.a, a) {
                return false;
            }
            if m.is_giant() && !o.is_giant() && !other_flanking && in_corridor(a, other.a) {
                return false;
            }
            if other.a < a {
                if inner.is_none_or(|(p, _)| other.a > p.a) {
                    inner = Some((other, is_barrier));
                }
            } else if outer.is_none_or(|(p, _)| other.a < p.a) {
                outer = Some((other, is_barrier));
            }
        }
        let upper = orbits::upper_eccentricity(m.law, m.draws.eccentricity);
        let fits = |low: (&Planet, bool, bool), high: (&Planet, bool, bool)| {
            let neighbour = |p: &Planet, own: bool, eccentric: bool| {
                let e = if !eccentric {
                    0.0
                } else if own {
                    upper
                } else {
                    p.e
                };
                Neighbour::new(self.members[p.member].mass, p.a, e)
            };
            satisfies_floor(
                &neighbour(low.0, low.1, low.2),
                &neighbour(high.0, high.1, high.2),
                host,
            )
        };
        let me = Planet {
            member,
            a,
            e: 0.0,
            resonance: None,
        };
        if let Some((p, is_barrier)) = inner
            && !fits((p, false, is_barrier), (&me, true, is_barrier))
        {
            return false;
        }
        if let Some((p, is_barrier)) = outer
            && !fits((&me, true, is_barrier), (p, false, is_barrier))
        {
            return false;
        }
        true
    }

    /// The chaotic zone's half-width about a giant of `mass`, in its semi-major axis.
    fn chaotic_zone(&self, mass: EarthMasses) -> f64 {
        let mu = mass.value() * EARTH_MASS_KG / (self.host_mass().value() * SOLAR_MASS_KG);
        CHAOTIC_ZONE_COEFFICIENT * math::powf(mu, 2.0 / 7.0)
    }

    /// Snaps a resonant cold chain's period ratios, inside out (P14.T8.a).
    ///
    /// Each outer planet of a pair goes, in order of preference, where the pair's period ratio is
    /// the nearest commensurability's widened by its offset, where it was placed, or where the
    /// pair keeps its placed ratio from the inner planet as it now lies; the first of these that
    /// is admitted is kept, and a chain with none left ends there.
    fn resonate(&mut self, g: usize) {
        let chain: Vec<Planet> = self
            .placed
            .iter()
            .filter(|p| self.members[p.member].group == g)
            .copied()
            .collect();
        let Some(&first) = chain.first() else {
            return;
        };
        if !self.members[first.member].cold_chain || chain.len() < MIN_RESONANT_CHAIN {
            return;
        }
        let share = if self.host_mass() < LOW_MASS_RESONANCE_LIMIT {
            LOW_MASS_RESONANT_CHAIN_PROBABILITY
        } else {
            RESONANT_CHAIN_PROBABILITY
        };
        if !self
            .draws
            .resonance
            .is_below(Threshold::from_probability(share))
        {
            return;
        }
        let mut kept: Vec<Planet> = self
            .placed
            .iter()
            .filter(|p| self.members[p.member].group != g)
            .copied()
            .collect();
        kept.push(first);
        let mut inner = first;
        for pair in chain.windows(2) {
            let (before, planet) = (pair[0], pair[1]);
            let member = planet.member;
            let placed_ratio = self.period(member, planet.a).value()
                / self.period(before.member, before.a).value();
            let inner_period = self.period(inner.member, inner.a).value();
            let at_ratio = |ratio: f64| self.axis(member, Days::new(inner_period * ratio));
            let nearest = Commensurability::nearest(placed_ratio);
            let u = resonance_offset(self.seed, self.system, self.members[member].index).value();
            let (zeta_lo, zeta_hi) = RESONANCE_OFFSET_ZETA;
            let offset = nearest.offset(zeta_lo + u * (zeta_hi - zeta_lo));
            let candidates = [
                (at_ratio(nearest.ratio() * (1.0 + offset)), true),
                (planet.a, false),
                (at_ratio(placed_ratio), false),
            ];
            let chosen = candidates
                .into_iter()
                .find(|&(a, _)| a > inner.a && self.admits_among(g, member, a, &kept));
            let Some((a, resonant)) = chosen else {
                break;
            };
            let next = Planet {
                member,
                a,
                e: self.eccentricity_at(member, a),
                resonance: resonant.then_some(Resonance {
                    commensurability: nearest,
                    offset,
                }),
            };
            kept.push(next);
            inner = next;
        }
        self.placed = kept;
    }

    /// The planets as placed: eccentricities re-checked against D7, then their orbits.
    fn finish(self) -> Vec<PlacedPlanet> {
        let host = self.host_mass();
        let mut order: Vec<usize> = (0..self.placed.len()).collect();
        order.sort_by(|&i, &j| self.placed[i].a.total_cmp(&self.placed[j].a));
        let mut e: Vec<f64> = self.placed.iter().map(|p| p.e).collect();
        let mut rescaled = vec![false; e.len()];
        let neighbour = |i: usize, e: f64| {
            let p = &self.placed[i];
            Neighbour::new(self.members[p.member].mass, p.a, e)
        };
        for pair in order.windows(2) {
            let (i, j) = (pair[0], pair[1]);
            if satisfies_floor(&neighbour(i, e[i]), &neighbour(j, e[j]), host) {
                continue;
            }
            let outer = largest_scale(|s| {
                satisfies_floor(&neighbour(i, e[i]), &neighbour(j, s * e[j]), host)
            });
            e[j] *= outer;
            rescaled[j] = true;
            if !satisfies_floor(&neighbour(i, e[i]), &neighbour(j, e[j]), host) {
                let inner = largest_scale(|s| {
                    satisfies_floor(&neighbour(i, s * e[i]), &neighbour(j, e[j]), host)
                });
                e[i] *= inner;
                rescaled[i] = true;
            }
            debug_assert!(
                satisfies_floor(&neighbour(i, e[i]), &neighbour(j, e[j]), host),
                "every placed pair clears D7's circular floor"
            );
        }
        let plane: SystemPlane = orbits::host_plane(
            self.seed,
            self.system,
            self.host.number(),
            self.host.plane(),
        );
        let snow = self.disc.snow_line();
        let mut planets: Vec<PlacedPlanet> = self
            .placed
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let m = &self.members[p.member];
                let group = &self.groups[m.group].template;
                let period = self.period(p.member, p.a);
                let width = orbits::inclination_width(m.law, period, m.cold_chain);
                let inclination = orbits::mutual_inclination(width, m.draws.inclination);
                let orientation = orbits::orientation_in_plane(
                    &plane,
                    inclination,
                    m.draws.node,
                    m.draws.periapsis,
                )
                .expect("the angles are finite and the inclination an arccosine");
                let orbit = KeplerElements::from_semi_major_axis(
                    p.a,
                    self.mu(p.member),
                    Eccentricity::new(e[i]).expect("an eccentricity held below 0.99"),
                    orientation,
                    m.draws.mean_anomaly,
                )
                .expect("a positive, finite axis about a positive mass");
                let formation_distance = if m.is_giant() {
                    p.a.max_of(self.core_site)
                } else {
                    p.a
                };
                PlacedPlanet {
                    index: m.index,
                    group: u8::try_from(m.group).expect("at most four groups"),
                    role: group.role(),
                    mass: m.mass,
                    orbit,
                    formation_distance,
                    formed: if formation_distance < snow {
                        SnowLineSide::Inside
                    } else {
                        SnowLineSide::Beyond
                    },
                    origin: group.origin(),
                    hot: m.hot,
                    resonance: p.resonance,
                    drawn_eccentricity: p.e,
                    rescaled: rescaled[i],
                }
            })
            .collect();
        planets.sort_by_key(PlacedPlanet::index);
        planets
    }
}

/// Which side of a planet another is placed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Inside,
    Outside,
}

impl Side {
    const fn other(self) -> Self {
        match self {
            Self::Inside => Self::Outside,
            Self::Outside => Self::Inside,
        }
    }
}

/// The largest s in [0, 1] for which `holds(s)`, by 60 halvings, for a condition that holds at 0
/// and, once it fails, fails for every larger s; 0 if it fails at 0.
fn largest_scale(holds: impl Fn(f64) -> bool) -> f64 {
    if holds(1.0) {
        return 1.0;
    }
    if !holds(0.0) {
        return 0.0;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if holds(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The period at rank `u` of `law` truncated to `a`–`b`, the law's shape kept.
#[must_use]
fn truncated_period(law: PeriodLaw, a: Days, b: Days, u: f64) -> Days {
    let (la, lb) = (math::ln(a.value()), math::ln(b.value()));
    let ln_period = match law {
        PeriodLaw::LogUniform { .. } => la + u * (lb - la),
        PeriodLaw::LogNormal {
            median, sigma_dex, ..
        } => {
            let scale = sigma_dex * core::f64::consts::LN_10;
            let centre = math::ln(median.value());
            let cdf = |x: f64| 0.5 * math::erfc(-(x - centre) / (scale * SQRT_2));
            let p = cdf(la) + u * (cdf(lb) - cdf(la));
            centre + scale * math::normal_quantile(p.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON))
        }
        PeriodLaw::BrokenPowerLaw {
            break_period,
            rising,
            falling,
            ..
        } => {
            let xb = math::ln(break_period.value());
            let mass = |alpha: f64, x1: f64, x2: f64| {
                if x2 <= x1 {
                    0.0
                } else if alpha.abs() < 1e-12 {
                    x2 - x1
                } else {
                    (math::exp(alpha * (x2 - xb)) - math::exp(alpha * (x1 - xb))) / alpha
                }
            };
            let invert = |alpha: f64, x1: f64, target: f64| {
                if alpha.abs() < 1e-12 {
                    x1 + target
                } else {
                    xb + math::ln(math::exp(alpha * (x1 - xb)) + alpha * target) / alpha
                }
            };
            let below = mass(rising, la, lb.min(xb));
            let above = mass(falling, la.max(xb), lb);
            let target = u * (below + above);
            if target <= below {
                invert(rising, la, target).min(lb.min(xb))
            } else {
                invert(falling, la.max(xb), target - below).min(lb)
            }
        }
    };
    Days::new(math::exp(ln_period.clamp(la, lb)))
}

/// `max` and `min` on the units, for the bounds.
trait Extremes: Sized + PartialOrd {
    fn max_of(self, other: Self) -> Self {
        if other > self { other } else { self }
    }
    fn min_of(self, other: Self) -> Self {
        if other < self { other } else { self }
    }
}

impl Extremes for Metres {}
impl Extremes for Days {}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::tides::{
        GIANT_TIDAL_Q_PRIME, ROCKY_TIDAL_Q_PRIME, circularisation_time, circularise,
    };
    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer};
    use crate::orbit::Orientation;
    use crate::planetary::disc::{self, DiscDraws};
    use crate::planetary::placement::masses::MassDraws;
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::premain::disc_lifetime;
    use crate::stellar::sse::{ZCoeffs, zams};
    use crate::time::CLOCK_WINDOW_H;
    use crate::units::consts::METRES_PER_AU;
    use crate::units::{Dex, HeliumExcess, Megayears, Radians, Seconds, Years};

    const SEED: Seed = Seed::new(0x5eed_0000_0014_0008);

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [3, -5, 1]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn au(x: f64) -> Metres {
        Metres::new(x * METRES_PER_AU)
    }

    fn zams_host(mass: f64, fe_h: f64) -> DiscHost {
        let mass = SolarMasses::new(mass);
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let coeffs = ZCoeffs::new(composition.z_fit());
        DiscHost::new(
            mass,
            composition.fe_h(),
            zams::luminosity(mass, &coeffs),
            zams::radius(mass, &coeffs),
        )
        .unwrap()
    }

    /// One sampled host: its placement host, disc, limits and planets.
    struct Sample {
        host: PlacementHost,
        disc: Disc,
        limits: Truncation,
        placed: HostPlacement,
    }

    impl Sample {
        fn mass(&self) -> SolarMasses {
            self.host.zams().mass()
        }

        /// The planets in order of semi-major axis.
        fn sorted(&self) -> Vec<PlacedPlanet> {
            let mut planets = self.placed.planets().to_vec();
            planets.sort_by(|a, b| {
                a.orbit()
                    .semi_major_axis()
                    .total_cmp(&b.orbit().semi_major_axis())
            });
            planets
        }
    }

    /// Host 0 of `n` systems drawn class `class`: masses 0.2–1.4 M☉, \[Fe/H\] −0.3 to +0.3,
    /// discs and their stars' lifetimes as drawn, one in four inside an outer limit and one in
    /// seven outside an inner one, as a zone would set them.
    fn sample(class: ArchitectureClass, n: u32) -> Vec<Sample> {
        (0..n)
            .map(|i| {
                let mass = [0.2, 0.5, 0.8, 1.0, 1.4][(i % 5) as usize];
                let fe_h = [-0.3, 0.0, 0.3][(i % 3) as usize];
                let zams = zams_host(mass, fe_h);
                let id = system(i);
                let mut limits = Truncation::NONE;
                if i % 4 == 1 {
                    limits = limits.with_outer(au(4.0 + 3.0 * mass));
                }
                if i % 7 == 3 {
                    limits = limits.with_inner(au(0.15 * mass));
                }
                let star = StarDraws::for_star(SEED, BodyId::new(id, 0));
                let lifetime = disc_lifetime(zams.mass(), star.disc_lifetime());
                let disc = disc::derive(&zams, lifetime, &DiscDraws::for_host(SEED, id, 0), limits);
                let host = PlacementHost::new(0, zams, HostPlane::Isotropic);
                let placed = place(SEED, id, &host, limits, &disc, class, 1);
                Sample {
                    host,
                    disc,
                    limits,
                    placed,
                }
            })
            .collect()
    }

    fn neighbour(p: &PlacedPlanet) -> Neighbour {
        Neighbour::new(
            p.mass(),
            p.orbit().semi_major_axis(),
            p.orbit().eccentricity().value(),
        )
    }

    fn is_giant(p: &PlacedPlanet) -> bool {
        p.mass() >= EarthMasses::from(SPACING_GIANT_MASS)
    }

    fn chaotic_zone(p: &PlacedPlanet, host: SolarMasses) -> f64 {
        let mu = p.mass().value() * EARTH_MASS_KG / (host.value() * SOLAR_MASS_KG);
        CHAOTIC_ZONE_COEFFICIENT * math::powf(mu, 2.0 / 7.0)
    }

    /// P14.T8 (a), (b), (c) and (d): every placed planet lies inside the limits passed in and
    /// outside its disc's inner edge (a hot or warm Jupiter, which migrates inside the disc's
    /// cavity, outside twice its Roche limit instead), its whole orbit inside the zone, and every
    /// adjacent pair satisfies both of D7's conditions at the epoch.
    #[test]
    fn every_planet_lies_inside_its_limits_and_every_pair_clears_d7() {
        let mut planets = 0;
        for class in ArchitectureClass::ALL {
            for s in sample(class, 600) {
                let profile = s.disc.profile().expect("the sample's discs exist");
                let sorted = s.sorted();
                for p in &sorted {
                    planets += 1;
                    let orbit = p.orbit();
                    let a = orbit.semi_major_axis();
                    let cavity = is_giant(p)
                        && p.migrated()
                        && matches!(
                            class,
                            ArchitectureClass::HotJupiter | ArchitectureClass::WarmGiant
                        )
                        && p.group() == 0;
                    assert!(cavity || a >= profile.inner_edge(), "{class:?}");
                    assert!(a <= profile.outer_edge(), "{class:?}");
                    if let Some(inner) = s.limits.inner() {
                        assert!(orbit.periapsis() >= inner * (1.0 - 1e-12), "{class:?}");
                    }
                    if let Some(outer) = s.limits.outer() {
                        assert!(orbit.apoapsis() <= outer * (1.0 + 1e-12), "{class:?}");
                    }
                }
                for pair in sorted.windows(2) {
                    assert!(
                        satisfies_floor(&neighbour(&pair[0]), &neighbour(&pair[1]), s.mass()),
                        "{class:?}: {:?} and {:?}",
                        pair[0].index(),
                        pair[1].index()
                    );
                }
            }
        }
        assert!(planets > 10_000, "{planets}");
    }

    /// P14.T8.a: a resonant pair's period ratio is its commensurability's widened by Fabrycky et
    /// al.'s offset, 0.1–0.2 in ζ, where the floor held; resonant chains are commoner about hosts
    /// under 0.3 M☉.
    #[test]
    fn resonant_pairs_sit_just_wide_of_commensurability() {
        let (mut pairs, mut low, mut high) = (0, 0, 0);
        for s in sample(ArchitectureClass::CompactMulti, 3_000) {
            let sorted = s.sorted();
            let resonant = sorted.iter().any(|p| p.resonance().is_some());
            if resonant {
                if s.mass() < LOW_MASS_RESONANCE_LIMIT {
                    low += 1;
                } else {
                    high += 1;
                }
            }
            for pair in sorted.windows(2) {
                let Some(resonance) = pair[1].resonance() else {
                    continue;
                };
                pairs += 1;
                let ratio = pair[1].orbit().period() / pair[0].orbit().period();
                let c = resonance.commensurability();
                let wide = ratio / c.ratio() - 1.0;
                assert!(
                    (wide - resonance.offset()).abs() < 1e-9,
                    "{wide} {resonance:?}"
                );
                let (lo, hi) = RESONANCE_OFFSET_ZETA;
                assert!(
                    wide >= c.offset(lo) * (1.0 - 1e-9) && wide <= c.offset(hi) * (1.0 + 1e-9),
                    "{c:?} {wide}"
                );
            }
        }
        assert!(pairs > 50 && low > 0 && high > 0, "{pairs} {low} {high}");
        // The offsets are Fabrycky et al.'s: 0.56–1.11% at 3:2, 1.67–3.33% at 2:1.
        let three_two = Commensurability::ThreeToTwo;
        assert!((three_two.offset(0.1) - 0.1 / 18.0).abs() < 1e-15);
        assert!((Commensurability::TwoToOne.offset(0.2) - 0.2 / 6.0).abs() < 1e-15);
        assert!((Commensurability::FourToThree.offset(0.2) - 0.2 / 36.0).abs() < 1e-15);
        assert!((Commensurability::FiveToThree.offset(0.1) - 0.2 / 45.0).abs() < 1e-15);
        assert_eq!(Commensurability::nearest(1.52), three_two);
        assert_eq!(Commensurability::nearest(2.9), Commensurability::TwoToOne);
        assert_eq!(
            Commensurability::nearest(1.61),
            Commensurability::FiveToThree
        );
    }

    /// P14.T8.b: no small planet lies in a giant's chaotic zone (Wisdom 1980), and outside the
    /// flanking groups none lies within two of its widths.
    #[test]
    fn no_small_planet_lies_in_a_giant_s_chaotic_zone() {
        let mut checked = 0;
        for class in ArchitectureClass::ALL
            .into_iter()
            .filter(|c| c.has_giants())
        {
            for s in sample(class, 800) {
                let planets = s.placed.planets();
                for giant in planets.iter().filter(|p| is_giant(p)) {
                    let ag = giant.orbit().semi_major_axis().value();
                    let zone = chaotic_zone(giant, s.mass());
                    for small in planets.iter().filter(|p| !is_giant(p)) {
                        checked += 1;
                        let gap = (small.orbit().semi_major_axis().value() - ag).abs() / ag;
                        let flanking = small.role() == GroupRole::Companions;
                        let width = if flanking { 1.0 } else { CHAOTIC_ZONE_GAP };
                        assert!(gap > width * zone, "{class:?}: {gap} against {zone}");
                    }
                }
            }
        }
        assert!(checked > 1_000, "{checked}");
    }

    /// P14.T8.c: every hot Jupiter's periapsis lies outside twice its Roche limit, and it has no
    /// companion inside 100 days.
    #[test]
    fn hot_jupiters_clear_twice_their_roche_limit_and_their_quiet_zone() {
        let mut hot = 0;
        for s in sample(ArchitectureClass::HotJupiter, 1_500) {
            if s.placed.class() != ArchitectureClass::HotJupiter {
                continue;
            }
            let planets = s.placed.planets();
            let Some(jupiter) = planets.iter().find(|p| p.group() == 0) else {
                continue;
            };
            hot += 1;
            let radius = Metres::from(radius_chen_kipping(jupiter.mass(), UnitUniform::HALF));
            let ratio = s.mass().value() * SOLAR_MASS_KG / (jupiter.mass().value() * EARTH_MASS_KG);
            let roche = radius * (ROCHE_COEFFICIENT * math::cbrt(ratio));
            assert!(jupiter.orbit().periapsis() >= roche * (2.0 * (1.0 - 1e-12)));
            // Inside 10 days, or held at its lower bound where the zone begins beyond 10 days; the
            // disc's inner edge does not hold it (it migrates inside the cavity).
            let period = Days::from(jupiter.orbit().period());
            let roche_bound = roche * 2.0;
            let bound = s
                .limits
                .inner()
                .map_or(roche_bound, |r| r.max_of(roche_bound));
            let a = jupiter.orbit().semi_major_axis();
            assert!(
                period.value() < 10.0 + 1e-9 || (a / bound - 1.0).abs() < 1e-12,
                "{period:?}"
            );
            assert!(
                jupiter.origin() == Origin::BeyondSnowLine && jupiter.formed_beyond_snow_line()
            );
            for other in planets.iter().filter(|p| p.group() != 0) {
                assert!(Days::from(other.orbit().period()).value() >= 100.0 * (1.0 - 1e-12));
            }
        }
        assert!(hot > 500, "{hot}");
    }

    /// P14.T8.d: the drawn eccentricities of cold compact planets are Rayleigh with σ = 0.04, and
    /// a planet keeps its drawn eccentricity unless D7's re-check scaled it down.
    #[test]
    fn cold_chain_eccentricities_are_rayleigh() {
        let mut drawn = Vec::new();
        let (mut all, mut rescaled) = (0_u32, 0_u32);
        for s in sample(ArchitectureClass::CompactMulti, 6_000) {
            if s.limits != Truncation::NONE {
                continue;
            }
            for p in s.placed.planets() {
                all += 1;
                if p.rescaled() {
                    rescaled += 1;
                    assert!(p.orbit().eccentricity().value() < p.drawn_eccentricity());
                } else {
                    assert_same_bits(p.orbit().eccentricity().value(), p.drawn_eccentricity());
                }
                if !p.hot() && p.role() == GroupRole::Chain {
                    drawn.push(p.drawn_eccentricity());
                }
            }
        }
        assert!(drawn.len() > 10_000, "{}", drawn.len());
        let sigma: f64 = 0.04;
        let ks = ks_one_sample(&mut drawn, |e| {
            -math::exp_m1(-(e * e) / (2.0 * sigma * sigma))
        });
        assert_p_value("cold chains' eccentricities", ks.p_value, ALPHA);
        // Few are scaled down: the spacing is drawn against the floor of the eccentricities
        // assumed.
        assert!(
            f64::from(rescaled) < 0.1 * f64::from(all),
            "{rescaled} of {all}"
        );
    }

    /// One Jupiter mass, in Earth masses.
    const EARTH_MASSES_PER_JUPITER: f64 =
        crate::planetary::architecture::template::EARTH_MASSES_PER_JUPITER_MASS;

    /// The age of the placed systems in (e): 5 Gyr.
    const AGE: Years = Years::new(5e9);

    fn damping(p: &PlacedPlanet, host: SolarMasses) -> Years {
        let radius = Metres::from(radius_chen_kipping(p.mass(), UnitUniform::HALF));
        let q = if is_giant(p) {
            GIANT_TIDAL_Q_PRIME
        } else {
            ROCKY_TIDAL_Q_PRIME
        };
        let orbit = p.orbit();
        circularisation_time(
            p.mass(),
            radius,
            host,
            orbit.semi_major_axis(),
            orbit.period(),
            q,
        )
    }

    /// P14.T8.e: hot Jupiters of up to 1.5 Jupiter masses within 5 days of the Sun-like hosts are
    /// circular to 0.01 at 5 Gyr, a circularising orbit stays between its primordial periapsis and
    /// semi-major axis, and every adjacent pair still clears D7 at 5 Gyr ± H.
    ///
    /// Heavier hot Jupiters circularise more slowly, τ growing with the planet's mass: at Q′ = 10⁶
    /// 95% of those of 1.5–2 Jupiter masses, 85% of 2–4 and 65% of heavier ones reach e < 0.01 by
    /// 5 Gyr, the rest keeping up to about 0.17, as massive hot Jupiters do (HAT-P-2 b, XO-3 b,
    /// WASP-14 b); so plan 14's "e < 0.01" is asserted up to 1.5 Jupiter masses.
    #[test]
    fn circularisation_keeps_pairs_apart_and_hot_jupiters_circular() {
        let window = Years::new(CLOCK_WINDOW_H.as_julian_years_f64());
        let mut close = 0;
        for class in [
            ArchitectureClass::HotJupiter,
            ArchitectureClass::CompactMulti,
        ] {
            for s in sample(class, 3_000) {
                let at = |t: Years| -> Vec<Neighbour> {
                    s.sorted()
                        .iter()
                        .map(|p| {
                            let orbit = p.orbit();
                            let (a, e) = circularise(
                                orbit.semi_major_axis(),
                                orbit.eccentricity().value(),
                                damping(p, s.mass()),
                                t,
                            );
                            assert!(a <= orbit.semi_major_axis() && a >= orbit.periapsis());
                            Neighbour::new(p.mass(), a, e)
                        })
                        .collect()
                };
                for t in [AGE - window, AGE, AGE + window] {
                    for pair in at(t).windows(2) {
                        assert!(satisfies_floor(&pair[0], &pair[1], s.mass()), "{class:?}");
                    }
                }
                if class == ArchitectureClass::HotJupiter
                    && s.placed.class() == class
                    && (0.8..1.2).contains(&s.mass().value())
                {
                    let jupiters = s.placed.planets().iter().filter(|p| {
                        p.group() == 0 && p.mass().value() <= 1.5 * EARTH_MASSES_PER_JUPITER
                    });
                    for p in jupiters {
                        if Days::from(p.orbit().period()).value() < 5.0 {
                            close += 1;
                            let (_, e) = circularise(
                                p.orbit().semi_major_axis(),
                                p.orbit().eccentricity().value(),
                                damping(p, s.mass()),
                                AGE,
                            );
                            assert!(
                                e < 0.01,
                                "{:?} M⊕ at {:?}: e {e}",
                                p.mass(),
                                p.orbit().period()
                            );
                        }
                    }
                }
            }
        }
        assert!(close > 30, "{close}");
    }

    /// D5's second fallback: a disc that grows no giant's core in its lifetime keeps no giant, and
    /// a host without a disc places nothing.
    #[test]
    fn a_disc_that_grows_no_core_in_time_keeps_no_giant() {
        let sun = zams_host(1.0, 0.0);
        let light = DiscDraws {
            gas_fraction: crate::stellar::draws::StandardNormal::new(-0.5).unwrap(),
            ..DiscDraws::MEDIAN
        };
        let brief = disc::derive(&sun, Megayears::new(0.3), &light, Truncation::NONE);
        let host = PlacementHost::new(0, sun, HostPlane::Isotropic);
        for class in ArchitectureClass::ALL
            .into_iter()
            .filter(|c| c.has_giants())
        {
            let placed = place(SEED, system(1), &host, Truncation::NONE, &brief, class, 1);
            assert_eq!(placed.drawn_class(), class);
            assert_eq!(placed.class(), class.giant_free_sibling());
            assert!(matches!(placed.core(), Some(GiantCore::TooSlow { .. })));
            assert!(placed.planets().iter().all(|p| !is_giant(p)));
        }
        let none = place(
            SEED,
            system(1),
            &host,
            Truncation::NONE,
            &Disc::None,
            ArchitectureClass::SolarLike,
            5,
        );
        assert_eq!(none.class(), ArchitectureClass::Barren);
        assert!(none.planets().is_empty());
        assert_eq!(none.next_slot(), 5);
    }

    /// Design note 3: a host's planets take consecutive slots from its first, drawn counts
    /// reserved before placing, and none beyond its block.
    #[test]
    fn planets_take_their_slots_from_the_host_s_first() {
        let sun = zams_host(1.0, 0.1);
        let disc = disc::derive(
            &sun,
            Megayears::new(3.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        );
        let host = PlacementHost::new(0, sun, HostPlane::Isotropic);
        let class = ArchitectureClass::SolarLike;
        let from_one = place(SEED, system(2), &host, Truncation::NONE, &disc, class, 1);
        let from_ten = place(SEED, system(2), &host, Truncation::NONE, &disc, class, 10);
        let reserved = from_one.next_slot() - 1;
        assert_eq!(from_ten.next_slot(), 10 + reserved);
        for p in from_one.planets() {
            let BodySlot::Planet(n) = p.index().slot() else {
                panic!("a primordial planet");
            };
            assert!(n >= 1 && n < from_one.next_slot());
        }
        // Slots are keys: the planets move with them.
        assert_ne!(from_one.planets(), from_ten.planets());
        // Near the block's end the counts are cut.
        let last = place(
            SEED,
            system(2),
            &host,
            Truncation::NONE,
            &disc,
            class,
            LAST_PLANET_SLOT,
        );
        assert!(last.planets().len() <= 1 && last.next_slot() == LAST_PLANET_SLOT + 1);
        // A second-generation host's planets take the second-generation slots.
        let second = place(
            SEED,
            system(2),
            &host,
            Truncation::NONE,
            &disc,
            class,
            SECOND_GENERATION_SLOT_START,
        );
        assert!(
            second
                .planets()
                .iter()
                .all(|p| matches!(p.index().slot(), BodySlot::SecondGeneration(_)))
        );
        assert!(second.next_slot() <= SECOND_GENERATION_SLOT_START + BLOCK_LEN);
        // Masses are read at the members' own slots.
        for p in from_one.planets() {
            let draws = MassDraws::for_planet(SEED, system(2), p.index());
            assert!(draws.scatter.value().is_finite());
        }
    }

    #[test]
    fn the_same_host_places_the_same_planets_twice_in_any_order() {
        let samples = |i: &u32| {
            let s = &sample(ArchitectureClass::CompactWithColdGiant, *i + 1)
                [usize::try_from(*i).unwrap()];
            s.placed.clone()
        };
        assert_order_independent(&[3, 11, 7], samples);
    }

    /// P14.T8.d: a close binary's circumbinary planets share its plane, to their inclinations.
    #[test]
    fn an_aligned_host_s_planets_share_its_plane() {
        let binary = Orientation::new(Radians::new(0.9), Radians::new(2.0), Radians::ZERO).unwrap();
        let plane = SystemPlane::of_orbit(&binary);
        let pair = zams_host(1.6, 0.0);
        let limits = Truncation::NONE.with_inner(au(0.4));
        let disc = disc::derive(&pair, Megayears::new(3.0), &DiscDraws::MEDIAN, limits);
        let host = PlacementHost::new(17, pair, HostPlane::Aligned(plane));
        let mut planets = 0;
        for i in 0..40 {
            let placed = place(
                SEED,
                system(i),
                &host,
                limits,
                &disc,
                ArchitectureClass::CompactMulti,
                1,
            );
            for p in placed.planets() {
                planets += 1;
                let (n, pole) = (p.orbit().orientation().normal(), binary.normal());
                let cos = n[0] * pole[0] + n[1] * pole[1] + n[2] * pole[2];
                assert!(math::acos(cos.clamp(-1.0, 1.0)) < 0.5, "{cos}");
            }
        }
        assert!(planets > 30, "{planets}");
    }

    #[test]
    fn a_host_s_group_draws_are_its_own_words() {
        let draws = HostDraws::for_host(SEED, system(4), 3);
        let stream = Stream::open(SEED, tags::PLANET_COUNT, ObjectKey::from(system(4)));
        let start = 3 * COUNT_WORDS_PER_HOST;
        assert_eq!(draws.hot_variant, Mark::from_word(stream.word_at(start)));
        assert_eq!(draws.resonance, Mark::from_word(stream.word_at(start + 1)));
        let g = 2;
        let block = start + COUNT_GROUP_WORDS_START + COUNT_WORDS_PER_GROUP * g;
        let group = draws.groups[usize::try_from(g).unwrap()];
        assert_eq!(group.presence, Mark::from_word(stream.word_at(block)));
        assert_eq!(group.count, Mark::from_word(stream.word_at(block + 1)));
        assert_eq!(group.side, Mark::from_word(stream.word_at(block + 3)));
        assert_ne!(draws, HostDraws::for_host(SEED, system(4), 4));
        let outer = BodyIndex::new(BodySlot::Planet(9), BodySub::Primary).unwrap();
        let mut stream = Stream::open(SEED, tags::PLANET_COUNT, ObjectKey::from(system(4)));
        stream.seek(COUNT_PAIR_WORDS_START + 9 * COUNT_WORDS_PER_PAIR);
        assert_same_bits(
            resonance_offset(SEED, system(4), outer).value(),
            stream.uniform_open(),
        );
    }

    #[test]
    fn counts_follow_their_laws() {
        let uniform = CountLaw::Uniform { min: 1, max: 3 };
        let mut tally = [0_u32; 4];
        let chain = crate::planetary::architecture::template::CHAIN_COUNT;
        let (mut sum, n) = (0.0, 20_000_u32);
        for i in 0..n {
            let draws = HostDraws::for_host(SEED, system(i), 0).groups[0];
            tally[usize::from(draw_count(uniform, SolarMasses::new(1.0), &draws))] += 1;
            sum += f64::from(draw_count(chain, SolarMasses::new(1.0), &draws));
        }
        assert_eq!(tally[0], 0);
        for count in &tally[1..] {
            assert!((6_300..7_000).contains(count), "{tally:?}");
        }
        let mean = sum / f64::from(n);
        assert!(
            (mean - chain.mean(SolarMasses::new(1.0))).abs() < 0.05,
            "{mean}"
        );
    }

    #[test]
    fn a_truncated_period_law_keeps_its_shape_inside_its_range() {
        let hot = PeriodLaw::LogNormal {
            median: Days::new(3.5),
            sigma_dex: 0.15,
            min: Days::new(1.0),
            max: Days::new(10.0),
        };
        let first = PeriodLaw::BrokenPowerLaw {
            break_period: Days::new(12.0),
            rising: 1.6,
            falling: -0.9,
            min: Days::new(1.0),
            max: Days::new(50.0),
        };
        for law in [hot, first] {
            let (a, b) = (Days::new(2.0), Days::new(30.0).min_of(law.range().1));
            let mut periods: Vec<f64> = (0..20_000)
                .map(|i| {
                    let u = HostDraws::for_host(SEED, system(i), 1).groups[0]
                        .location
                        .value();
                    truncated_period(law, a, b, u).value()
                })
                .collect();
            assert!(
                periods
                    .iter()
                    .all(|&p| p >= a.value() * (1.0 - 1e-12) && p <= b.value() * (1.0 + 1e-12))
            );
            let density = |p: f64| match law {
                PeriodLaw::LogNormal {
                    median, sigma_dex, ..
                } => {
                    let z = (math::log10(p) - math::log10(median.value())) / sigma_dex;
                    math::exp(-0.5 * z * z) / p
                }
                PeriodLaw::BrokenPowerLaw { .. } => {
                    let x = p / 12.0;
                    (if p < 12.0 {
                        math::powf(x, 1.6)
                    } else {
                        math::powf(x, -0.9)
                    }) / p
                }
                PeriodLaw::LogUniform { .. } => 1.0 / p,
            };
            // The cumulative distribution by Simpson's rule in ln P.
            let cdf = |p: f64| {
                let integral = |x1: f64, x2: f64| {
                    let steps = 400_u32;
                    let h = (x2 - x1) / f64::from(steps);
                    let g = |x: f64| density(math::exp(x)) * math::exp(x);
                    let mut sum = g(x1) + g(x2);
                    for k in 1..steps {
                        sum += if k % 2 == 1 { 4.0 } else { 2.0 } * g(x1 + h * f64::from(k));
                    }
                    sum * h / 3.0
                };
                let (la, lb) = (math::ln(a.value()), math::ln(b.value()));
                integral(la, math::ln(p).clamp(la, lb)) / integral(la, lb)
            };
            let ks = ks_one_sample(&mut periods, cdf);
            assert_p_value(&format!("{law:?}"), ks.p_value, ALPHA);
        }
        let _ = Seconds::ZERO;
    }
}
