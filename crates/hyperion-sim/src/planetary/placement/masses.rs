//! Planet masses: a characteristic mass per group, members correlated about it ("peas in a pod"),
//! and every group held to what its disc holds (plan 14, P14.T7; design notes 4 and 5; ruling 38).
//!
//! # The model
//!
//! - **Characteristic mass** (P14.T7.a). A group of the correlated law ([`MassLaw::Correlated`]:
//!   a compact chain, a warm giant's companions, a rocky group) has one characteristic mass
//!   `m_c`, log-normal about a median that scales as the disc's solid mass to the power 1:
//!   `m_c` = `m_ref` × (`M_s` ÷ [`REFERENCE_SOLID_MASS`]) × 10^(`σ_b` z), held to the template's
//!   range. `M_s` is the whole solid mass of the host's disc between its edges, which is already
//!   cut to the host's stable zone (P14.T9), and `m_ref` the group's mass in the median solar disc
//!   ([`reference_mass`]): 4 M⊕ for a compact group (plan 14), 0.5 M⊕ for a rocky one.
//! - **Members** (P14.T7.b). Member i of n, counted inside out, has
//!   log₁₀ mᵢ = log₁₀ `m_c` + `σ_w` εᵢ + 0.1 (i − (n − 1) ÷ 2), held to the template's range: the
//!   same scatter about `m_c` for every member, and 0.1 dex more per step outward
//!   ([`OUTWARD_STEP_DEX`]), centred on the group so that `m_c` stays the group's typical mass
//!   whatever its count.
//! - **The solid budget** (ruling 38, point 4). A group's members together hold at most
//!   [`solid_budget`] of the disc, its whole solid mass; a group over it is scaled down, every
//!   member by the same factor, so that the correlation and the ordering survive. The budget
//!   outranks the template's range: a starved disc makes smaller planets, not more mass than it
//!   has. See below for why the budget is the whole disc and not the local annulus.
//! - **Giants** (P14.T7.c). A group whose range reaches giants ([`PlanetGroup::places_giants`])
//!   takes each mass from its template's law, Cumming et al.'s dN ÷ d ln M ∝ M^−0.31, and is held
//!   together to the disc's gas mass ([`gas_budget`]) in the same way. The ice-rich bodies, the
//!   ice giants and the survivor draw from their laws (log-uniform) and are held to the solid
//!   budget. Whether a host can make a giant at all is [`giant_core`]: 10 M⊕ of solids beyond the
//!   snow line, and a core of 10 M⊕ grown there within the disc's lifetime.
//!
//! Each group's limit is its own, so that changing one group's template never moves another
//! group's masses (P14.T7.b's test). A system's groups together can therefore hold more than its
//! disc: a `CompactWithColdGiant` chain may take up to the whole solid mass while its giants'
//! cores also need 10 M⊕ of it.
//!
//! # The solid budget (ruling 38, point 4)
//!
//! P14.T7.b capped each mass at 10 times the local isolation mass ([`DiscProfile::isolation_mass`],
//! Lissauer 1987). In plan 14's median solar disc that is 2 × 10⁻³ M⊕ at 0.1 au (the isolation
//! mass there is 2.3 × 10⁻⁴ M⊕), where the compact classes place planets of 1–20 M⊕: planets that
//! close in cannot be made from the solids of their own annulus. Chiang and Laughlin (2013, MNRAS
//! 431, 3444, eq. 4) turn the Kepler planets into a minimum-mass extrasolar nebula of solids,
//! 620 g cm⁻² (a ÷ 0.2 au)^−1.6, "a factor of 5 larger than the solid surface density of the MMSN"
//! (their §2.2); between 0.05 and 0.5 au it holds 12.7 M⊕ for a typical Kepler planet's host,
//! where the median solar disc here has 0.23 M⊕. The solids must be carried inward, as pebbles
//! drifting through the disc (Lambrechts et al. 2019, A&A 627, A83: super-Earth systems form when
//! the pebble flux through the inner disc is high), so the budget is drawn from the whole disc.
//!
//! Its efficiency is 1 ([`SOLID_BUDGET_EFFICIENCY`]): a group may hold the disc's whole solid
//! mass, and no more. Mulders et al. (2021, ApJ 920, 66, abstract and §2.4) compare the solid
//! masses of Kepler's and the radial-velocity surveys' planetary systems about Sun-like stars,
//! corrected for detection biases, with the dust masses of Class II discs: all three distributions
//! peak near 10 M⊕, and "we find a discrepancy only when the planet formation efficiency is below
//! 100%". Plan 14's discs are primordial, 0.2–0.7 dex heavier than Class II discs (P14.T3), so
//! against them a typical system needs less than all of it: the median compact group (3.5 planets
//! of a characteristic 4 M⊕) holds 43% of the median solar disc's 32.2 M⊕, and Kepler-11's five
//! inner planets, 35 M⊕ by the masses Chiang and Laughlin use (their §3.7.1), need a disc about
//! 1.1 times the median. The budget binds for a tenth of Sun-like chains, and for half of an M
//! dwarf's, whose chains are longer (P14.T5) while its disc's solids fall with its mass. The
//! pebble-accretion models' own efficiencies, about 50% in Lambrechts and Johansen (2014, A&A 572,
//! A107, abstract) and 15–20% into a single core (their §5.1 and §7), would bind for about half of
//! the Sun-like chains and override the plan's 4 M⊕ at the median disc, so they are not the
//! budget.
//!
//! The local isolation mass keeps what it describes: the core a giant starts from beyond the snow
//! line ([`giant_core`]).
//!
//! # The correlation (P14.T7.b)
//!
//! Weiss et al. (2018, AJ 155, 48) find in the California–Kepler Survey's multi-planet systems a
//! Pearson correlation of r = 0.65 between the sizes of adjacent planets (§3; their Fig. 2's
//! caption prints 0.62), 0.53 for planets over 1 R⊕, and the outer planet the larger in 65.4 ±
//! 0.4% of pairs (§5.3), the ratio of the outer to the inner size growing with the pair's
//! difference in temperature, which they suggest photo-evaporation causes. The paper does not say whether its correlation is of radii
//! or of their logarithms; its figures are logarithmic, and plan 14 reads it as log radii.
//!
//! Here the between-system scatter of `m_c` is supplied by the disc: its gas fraction's 0.5 dex
//! (P14.T3) passes into `m_c` at the power 1 on the solid mass, before the template's range. That
//! is more than the correlation needs, so the added scatter `σ_b` is 0
//! ([`BETWEEN_SYSTEM_SCATTER_DEX`]), and `σ_w` = 0.2 dex ([`WITHIN_SYSTEM_SCATTER_DEX`]) gives the
//! adjacent log radii of compact systems in solar discs, through P14.T11's Chen and Kipping radius
//! with each planet's own quantile, a correlation of 0.66. Their log masses then correlate at
//! 0.87, and the outer planet is the more massive in 0.58 of pairs and the larger in 0.58. No pair
//! of `σ_b` and `σ_w` meets both of Weiss et al.'s figures here: 65% needs a `σ_w` of about 0.13
//! dex, which with the disc's scatter puts the radius correlation near 0.71. Chen and Kipping's
//! scatter, 0.146 dex in radius above 2.04 M⊕ and independent for each planet (design note 8), is
//! what separates the radius correlation from the mass correlation; He, Ford and Ragozzine (2019,
//! MNRAS 490, 4575, §3.7) fit a within-system width of 0.31 ± 0.07 in ln R, 0.135 dex, which that
//! scatter alone more than fills.
//!
//! The 4 M⊕ median is plan 14's figure. Pascucci et al. (2018, ApJ 856, L28, abstract and §2)
//! find the occurrence of Kepler's planets inside 100 days a broken power law in the planet-to-star
//! mass ratio, with the same break for M, K and G hosts, so that "the most common planetary mass
//! inside the snowline ... increases linearly with stellar mass", from 3.5–4.5 M⊕ about M dwarfs
//! to 8–9 M⊕ about G stars (and 7.7 ± 1.7 M⊕ at 0.91 M☉ with EPOS, their §3). Their G hosts' law
//! (Table 1: indices 0.76 below a break of 2.8 × 10⁻⁵ and −2.9 above) has its median at 0.55 of
//! the break, 5.1 M⊕ at 1 M☉, if extended to the smallest masses, and at 0.70 of it, 6.5 M⊕,
//! inside the range they fitted, (0.5–8) × 10⁻⁵; Mulders et al. (2021, §2.1) find a de-biased
//! median of about 5 M⊕ for the radial-velocity planets. The plan's 4 M⊕ lies 20–40% under those,
//! at the edge of their errors, and is kept as the plan's; the scaling with the disc, whose solids
//! are proportional to the host's mass, is Pascucci et al.'s linear one.
//!
//! # Draws
//!
//! All on [`tags::PLANET_MASS`], keyed by the system's ID, with the planet's slot in the draw
//! number (design notes 3 and 4): the planet in slot s reads words 8s to 8s + 7
//! ([`MASS_WORDS_PER_SLOT`]). Words 0–1 are its own within-system scatter εᵢ, words 2–3 its
//! group's between-system scatter z, read at the group's first member, and word 4 the rank of the
//! group's mass law; 5–7 are reserved. A planet's masses depend on nothing but its slot, its
//! group's first slot, its place in the group and the group's count, so a placer assigns each
//! member its slot before asking for the masses and never renumbers it, and inserting a group
//! moves no other group's draws.

use crate::id::SystemId;
use crate::math;
use crate::planetary::architecture::template::{GroupRole, MassLaw, MassRange, PlanetGroup};
use crate::planetary::architecture::{ClassConstraints, DiscCapacity, HostMultiplicity, ZoneLimit};
use crate::planetary::disc::{Disc, DiscProfile};
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::draws::{StandardNormal, UnitUniform};
use crate::units::consts::METRES_PER_AU;
use crate::units::{EarthMasses, Megayears, Metres};

/// Words of the [`tags::PLANET_MASS`] stream that one planet slot owns: the planet in slot s reads
/// words 8s to 8s + 7.
///
/// Changing it moves every planet but the first slot's, which is a generator-version change.
pub const MASS_WORDS_PER_SLOT: u64 = 8;

/// The first word of a planet's within-system scatter in its slot's block.
const SCATTER_WORD: u64 = 0;

/// The first word of a group's between-system scatter in its first member's block.
const GROUP_WORD: u64 = 2;

/// The word of a group law's rank in a planet's block.
const RANK_WORD: u64 = 4;

/// The characteristic mass of a compact group in the median solar disc: 4 M⊕ (plan 14,
/// P14.T7.a).
///
/// Plan 14's figure, 20–40% under the medians of Pascucci et al. (2018) and Mulders et al. (2021),
/// 5–6.5 M⊕ at a solar mass, at the edge of their errors (see the [module documentation](self)).
/// It is the median of a compact chain's, a warm giant's companions' and a substellar chain's
/// characteristic mass when the disc holds [`REFERENCE_SOLID_MASS`].
pub const COMPACT_CHARACTERISTIC_MASS: EarthMasses = EarthMasses::new(4.0);

/// The characteristic mass of a rocky group in the median solar disc: 0.5 M⊕.
///
/// This module's figure, where plan 14 gives none: the median solar disc makes a group like the
/// Solar System's, whose four terrestrial planets hold 1.98 M⊕, 0.5 M⊕ each on average. Kokubo and
/// Genda (2010, ApJ 714, L21, Table 1) grow 3.6 ± 0.8 planets from 2.3 M⊕ of protoplanets
/// between 0.5 and 1.5 au, the largest 1.18 M⊕ and the second 0.72 M⊕, a mean of 0.64 M⊕. It sets
/// how many rocky planets reach Earth's mass, and so η⊕ (ruling 48, point f).
pub const ROCKY_CHARACTERISTIC_MASS: EarthMasses = EarthMasses::new(0.5);

/// The solid mass of the median solar disc, the disc of a 1 M☉ star of \[Fe/H\] = 0 at plan 06's
/// zero-age state with every variate at its median ([`DiscDraws::MEDIAN`]): 32.20 M⊕.
///
/// A group's median characteristic mass is its [`reference_mass`] in a disc of this much solid
/// mass, and scales in proportion to the solid mass (P14.T7.a). A test holds it to the disc model
/// it stands for.
///
/// [`DiscDraws::MEDIAN`]: crate::planetary::disc::DiscDraws::MEDIAN
pub const REFERENCE_SOLID_MASS: EarthMasses = EarthMasses::new(32.20);

/// The between-system scatter `σ_b` added to a group's characteristic mass, in dex: 0
/// (P14.T7.a–b).
///
/// Plan 14 leaves `σ_b` to be chosen, with `σ_w`, so that adjacent planets' log radii correlate at
/// 0.65 (Weiss et al. 2018). The disc already scatters the characteristic mass by its gas
/// fraction's 0.5 dex, which is more than that correlation needs, so any `σ_b` above 0 moves the
/// correlation away from Weiss et al.'s figure. The group's standard normal is still drawn, on
/// words 2–3 of its first member's block, so that a later `σ_b` changes a constant and no draw.
pub const BETWEEN_SYSTEM_SCATTER_DEX: f64 = 0.0;

/// The within-system scatter `σ_w` of a member about its group's characteristic mass, in dex: 0.2
/// (P14.T7.b).
///
/// Chosen so that the adjacent log radii of compact systems in solar discs, after P14.T11's Chen
/// and Kipping radius, correlate at Weiss et al.'s (2018) 0.65; see the [module
/// documentation](self) for what it gives and what it cannot.
pub const WITHIN_SYSTEM_SCATTER_DEX: f64 = 0.2;

/// How much heavier each member is than the one inside it, in dex: 0.1 (plan 14, P14.T7.b).
///
/// Plan 14's figure, for Weiss et al.'s (2018) outer planets being the larger in most pairs. It
/// is centred on the group's middle member.
pub const OUTWARD_STEP_DEX: f64 = 0.1;

/// The share of the disc's whole solid mass that one group's members may hold: 1 (ruling 38,
/// point 4).
///
/// Mulders et al. (2021, ApJ 920, 66): the solid masses of planetary systems about Sun-like stars
/// match those of their discs only for a formation efficiency near 100%. See the [module
/// documentation](self) for the budget's reasoning and its figures.
pub const SOLID_BUDGET_EFFICIENCY: f64 = 1.0;

/// The mass a giant's core must reach for runaway gas accretion: 10 M⊕ (P14.T7.c).
///
/// Lambrechts and Johansen (2014, A&A 572, A107, §3.2): the mass at which an embryo's envelope
/// can no longer be supported "is standardly identified as the critical core mass, which is
/// typically of the order of 10 ME" (after Mizuno 1980). It is the same figure as the solids a
/// giant needs beyond the snow line,
/// [`GIANT_SOLID_BUDGET`](crate::planetary::architecture::GIANT_SOLID_BUDGET).
pub const CRITICAL_CORE_MASS: EarthMasses = EarthMasses::new(10.0);

/// The core mass of Lambrechts and Johansen's (2014) eq. 35 at their reference: 11 M⊕ at 1 Myr and
/// 10 au, for a dust-to-gas ratio of 0.01, a solar-mass star and a gas surface density of
/// 500 g cm⁻² × (r ÷ 1 au)^−1.
const PEBBLE_CORE_MASS_AT_REFERENCE: EarthMasses = EarthMasses::new(11.0);

/// Lambrechts and Johansen's (2014) reference dust-to-gas ratio, Z₀ = 0.01.
const PEBBLE_REFERENCE_DUST_TO_GAS: f64 = 0.01;

/// Lambrechts and Johansen's (2014, eq. 1) reference gas normalisation β₀, in g cm⁻²:
/// `Σ_g` = β₀ (r ÷ 1 au)^−1 with β₀ = 500 g cm⁻².
const PEBBLE_REFERENCE_GAS_SCALE_G_CM2: f64 = 500.0;

/// The orbital radius of Lambrechts and Johansen's (2014) eq. 35 at its reference, in au.
const PEBBLE_REFERENCE_RADIUS_AU: f64 = 10.0;

/// A surface density in kg m⁻² in g cm⁻².
const G_CM2_PER_KG_M2: f64 = 0.1;

/// The random variates of one planet's mass, as drawn from [`tags::PLANET_MASS`] by
/// [`MassDraws::for_planet`], or given explicitly by a test or a tool.
///
/// The fields are plain variates with no invariant between them, so they are public, as the
/// disc's [`DiscDraws`](crate::planetary::disc::DiscDraws) are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassDraws {
    /// The planet's within-system scatter εᵢ about its group's characteristic mass. Words 0–1 of
    /// its slot's block.
    pub scatter: StandardNormal,
    /// Its group's between-system scatter z, read only when the planet is its group's first
    /// member. Words 2–3.
    pub group: StandardNormal,
    /// The rank of its group's mass law, for the groups that draw from a law. Word 4.
    pub rank: UnitUniform,
}

impl MassDraws {
    /// Every variate at its median.
    pub const MEDIAN: Self = Self {
        scatter: StandardNormal::ZERO,
        group: StandardNormal::ZERO,
        rank: UnitUniform::HALF,
    };

    /// The draws of `planet` of `system`, in the universe of `seed`: words 8s onwards of
    /// `system`'s [`tags::PLANET_MASS`] stream, s being the planet's slot (the high byte of its
    /// index).
    ///
    /// # Panics
    ///
    /// In debug builds, if `planet` is not a planet: a primordial or second-generation planet's
    /// own index, not a moon's, a ring's, a belt's or a star's.
    #[must_use]
    pub fn for_planet(seed: Seed, system: SystemId, planet: BodyIndex) -> Self {
        debug_assert!(is_planet(planet), "a mass is a planet's, got {planet:?}");
        let mut stream = Stream::open(seed, tags::PLANET_MASS, ObjectKey::from(system));
        let start = u64::from(planet.get() >> 8) * MASS_WORDS_PER_SLOT;
        stream.seek(start + SCATTER_WORD);
        let scatter = draw_normal(&mut stream);
        stream.seek(start + GROUP_WORD);
        let group = draw_normal(&mut stream);
        stream.seek(start + RANK_WORD);
        let rank = UnitUniform::new(stream.uniform_open())
            .expect("an open uniform lies strictly between 0 and 1");
        Self {
            scatter,
            group,
            rank,
        }
    }
}

/// Whether `index` is a planet's own index.
#[must_use]
fn is_planet(index: BodyIndex) -> bool {
    matches!(
        (index.slot(), index.sub()),
        (
            BodySlot::Planet(_) | BodySlot::SecondGeneration(_),
            BodySub::Primary
        )
    )
}

/// The next two words of `stream` as a standard normal.
#[must_use]
fn draw_normal(stream: &mut Stream) -> StandardNormal {
    StandardNormal::new(stream.standard_normal()).expect("a Box–Muller variate is finite")
}

/// The characteristic mass a correlated group of `group`'s role has in the median solar disc,
/// one of [`REFERENCE_SOLID_MASS`]: its median characteristic mass there.
///
/// A compact chain, a warm giant's companions and a substellar chain take
/// [`COMPACT_CHARACTERISTIC_MASS`], and a rocky group [`ROCKY_CHARACTERISTIC_MASS`]. The other
/// roles draw from their laws in every template; should one take the correlated law, its reference
/// is the geometric middle of its range.
#[must_use]
pub fn reference_mass(group: &PlanetGroup) -> EarthMasses {
    match group.role() {
        GroupRole::Chain | GroupRole::Companions => COMPACT_CHARACTERISTIC_MASS,
        GroupRole::Rocky => ROCKY_CHARACTERISTIC_MASS,
        GroupRole::IceRich | GroupRole::Giant | GroupRole::IceGiant | GroupRole::Survivor => {
            let range = group.masses();
            EarthMasses::new((range.min().value() * range.max().value()).sqrt())
        }
    }
}

/// The characteristic mass of `group` in a disc of solid mass `solid_mass`, at the group's
/// between-system variate `between` (P14.T7.a): [`reference_mass`] × (`solid_mass` ÷
/// [`REFERENCE_SOLID_MASS`]) × 10^(`σ_b` z), held to the group's range.
///
/// # Panics
///
/// In debug builds, if `solid_mass` is negative or not finite.
#[must_use]
pub fn characteristic_mass(
    group: &PlanetGroup,
    solid_mass: EarthMasses,
    between: StandardNormal,
) -> EarthMasses {
    debug_assert!(
        solid_mass.value().is_finite() && solid_mass.value() >= 0.0,
        "a solid mass is finite and not negative, got {solid_mass:?}"
    );
    let median = reference_mass(group) * (solid_mass / REFERENCE_SOLID_MASS);
    let drawn = median * math::exp10(BETWEEN_SYSTEM_SCATTER_DEX * between.value());
    held(drawn, group.masses())
}

/// How many 0.1 dex steps member `position` of `count` lies outside the group's middle:
/// position − (count − 1) ÷ 2.
#[must_use]
fn steps_from_middle(position: usize, count: usize) -> f64 {
    let position = u32::try_from(position).expect("no group's members approach 2³² entries");
    let count = u32::try_from(count).expect("no group's members approach 2³² entries");
    (f64::from(position) * 2.0 + 1.0 - f64::from(count)) / 2.0
}

/// A member's mass about `characteristic`, at its scatter `scatter` and `steps` outside the
/// middle, held to `range` (P14.T7.b).
#[must_use]
fn member_mass(
    characteristic: EarthMasses,
    scatter: StandardNormal,
    steps: f64,
    range: MassRange,
) -> EarthMasses {
    let offset = WITHIN_SYSTEM_SCATTER_DEX * scatter.value() + OUTWARD_STEP_DEX * steps;
    held(characteristic * math::exp10(offset), range)
}

/// The mass at `rank` of a group law over `range`: the inverse of its cumulative distribution
/// (P14.T7.c for giants).
///
/// dN ÷ d ln M ∝ M^α gives M = (lo^α + u (hi^α − lo^α))^(1 ÷ α), and α = 0 or a log-uniform law
/// M = lo (hi ÷ lo)^u. The correlated law has no rank; it is not a law of one planet.
///
/// # Panics
///
/// If `range`'s law is [`MassLaw::Correlated`], in every build: a bug in the caller.
#[must_use]
fn law_mass(range: MassRange, rank: UnitUniform) -> EarthMasses {
    let (lo, hi, u) = (range.min().value(), range.max().value(), rank.value());
    let log_uniform = || math::exp(math::ln(lo) + u * (math::ln(hi) - math::ln(lo)));
    let mass = match range.law() {
        MassLaw::PowerLaw { index } if index.abs() > 1e-12 => {
            let (low, high) = (math::powf(lo, index), math::powf(hi, index));
            math::powf(low + u * (high - low), 1.0 / index)
        }
        MassLaw::PowerLaw { .. } | MassLaw::LogUniform => log_uniform(),
        MassLaw::Correlated => panic!("the correlated law draws a group, not one planet"),
    };
    held(EarthMasses::new(mass), range)
}

/// `mass` held to `range`.
#[must_use]
fn held(mass: EarthMasses, range: MassRange) -> EarthMasses {
    EarthMasses::new(mass.value().clamp(range.min().value(), range.max().value()))
}

/// The most that one group's members may hold of `disc`'s solids: [`SOLID_BUDGET_EFFICIENCY`] of
/// its whole solid mass between its edges (ruling 38, point 4).
#[must_use]
pub fn solid_budget(disc: &DiscProfile) -> EarthMasses {
    disc.solid_mass() * SOLID_BUDGET_EFFICIENCY
}

/// The most that one group of giants may hold: `disc`'s gas mass between its edges (P14.T7.c).
#[must_use]
pub fn gas_budget(disc: &DiscProfile) -> EarthMasses {
    EarthMasses::from(disc.gas_mass())
}

/// Which limit, if any, a group's masses were held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupCap {
    /// The masses as drawn fit inside the group's limit.
    AsDrawn,
    /// The drawn masses held more than [`solid_budget`], and were scaled down to it.
    SolidBudget,
    /// The drawn giants held more than [`gas_budget`], and were scaled down to it.
    GasMass,
}

/// The masses of one group's members, inside out, and what held them ([`group_masses`]).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupMasses {
    masses: Vec<EarthMasses>,
    characteristic: Option<EarthMasses>,
    cap: GroupCap,
}

impl GroupMasses {
    /// The members' masses, in the order of the members given.
    #[must_use]
    pub fn masses(&self) -> &[EarthMasses] {
        &self.masses
    }

    /// The group's characteristic mass, for a group of the correlated law with at least one
    /// member; `None` otherwise.
    #[must_use]
    pub const fn characteristic(&self) -> Option<EarthMasses> {
        self.characteristic
    }

    /// Which limit held the masses.
    #[must_use]
    pub const fn cap(&self) -> GroupCap {
        self.cap
    }

    /// The members' total mass, summed inside out.
    #[must_use]
    pub fn total(&self) -> EarthMasses {
        total(&self.masses)
    }
}

/// The sum of `masses` in their order.
#[must_use]
fn total(masses: &[EarthMasses]) -> EarthMasses {
    masses
        .iter()
        .fold(EarthMasses::ZERO, |sum, &mass| sum + mass)
}

/// The masses of `group`'s members in `disc`, from each member's variates `draws`, inside out
/// (P14.T7): the pure function behind [`group_masses`].
///
/// Member i of the group is `draws[i]`, the group's count is `draws.len()`, and the group's
/// between-system variate is the first member's. A correlated group scatters its members about
/// [`characteristic_mass`]; any other draws each mass from its law at the member's rank. The
/// members are then held together to [`solid_budget`], or for a group that places giants to
/// [`gas_budget`], by one factor for every member.
///
/// # Examples
///
/// Every variate at its median shows the law's shape: a chain of five in the median solar disc
/// centred on its characteristic mass, each planet 0.1 dex heavier than the one inside it; and in a
/// disc with a tenth of the metals, the same chain held to that disc's solids.
///
/// ```
/// use hyperion_sim::planetary::architecture::ArchitectureClass;
/// use hyperion_sim::planetary::architecture::template::template;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{
///     GroupCap, MassDraws, group_masses_from, solid_budget,
/// };
/// use hyperion_sim::units::{Dex, Megayears, SolarLuminosities, SolarMasses, SolarRadii};
///
/// let disc_at = |fe_h: f64| -> Result<_, Box<dyn std::error::Error>> {
///     let host = DiscHost::new(
///         SolarMasses::new(1.0),
///         Dex::new(fe_h),
///         SolarLuminosities::new(0.70),
///         SolarRadii::new(0.89),
///     )?;
///     let disc = disc::derive(&host, Megayears::new(2.0), &DiscDraws::MEDIAN, Truncation::NONE);
///     Ok(*disc.profile().expect("an untruncated disc"))
/// };
/// let chain = &template(ArchitectureClass::CompactMulti).groups()[0];
/// let draws = [MassDraws::MEDIAN; 5];
///
/// let solar = group_masses_from(chain, &disc_at(0.0)?, &draws);
/// let m = solar.masses();
/// assert_eq!(solar.cap(), GroupCap::AsDrawn);
/// assert!((m[2].value() - 4.0).abs() < 0.01);
/// assert!(m[0] < m[1] && m[3] < m[4]);
///
/// let poor = disc_at(-1.0)?;
/// let held = group_masses_from(chain, &poor, &draws);
/// assert_eq!(held.cap(), GroupCap::SolidBudget);
/// assert!((held.total() / solid_budget(&poor) - 1.0).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn group_masses_from(
    group: &PlanetGroup,
    disc: &DiscProfile,
    draws: &[MassDraws],
) -> GroupMasses {
    let range = group.masses();
    let (mut masses, characteristic): (Vec<EarthMasses>, _) = match range.law() {
        MassLaw::Correlated => match draws.first() {
            None => (Vec::new(), None),
            Some(first) => {
                let characteristic = characteristic_mass(group, disc.solid_mass(), first.group);
                let masses = draws
                    .iter()
                    .enumerate()
                    .map(|(i, d)| {
                        let steps = steps_from_middle(i, draws.len());
                        member_mass(characteristic, d.scatter, steps, range)
                    })
                    .collect();
                (masses, Some(characteristic))
            }
        },
        MassLaw::PowerLaw { .. } | MassLaw::LogUniform => (
            draws.iter().map(|d| law_mass(range, d.rank)).collect(),
            None,
        ),
    };
    let (limit, reached) = if group.places_giants() {
        (gas_budget(disc), GroupCap::GasMass)
    } else {
        (solid_budget(disc), GroupCap::SolidBudget)
    };
    let unscaled = total(&masses);
    let cap = if unscaled > limit {
        let scale = limit / unscaled;
        for mass in &mut masses {
            *mass = *mass * scale;
        }
        reached
    } else {
        GroupCap::AsDrawn
    };
    GroupMasses {
        masses,
        characteristic,
        cap,
    }
}

/// The masses of `group`'s members `members`, inside out, about a host whose disc is `disc`, in
/// system `system` of the universe of `seed` (P14.T7).
///
/// `members` are the planets' indices, in the order the placer (P14.T8) sets them down from the
/// host outward, for the count it drew; a placer that stops early keeps the masses of those it
/// placed. Each member's variates are its own slot's ([`MassDraws::for_planet`]), so the masses
/// of one group never depend on another's. See [`group_masses_from`] for the laws and the limits.
///
/// # Panics
///
/// In debug builds, if a member is not a planet's own index, or two members share a slot.
///
/// # Examples
///
/// A compact chain of four planets about a Sun-like star: similar masses, rising outward, and
/// never more than the disc's solids.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::architecture::ArchitectureClass;
/// use hyperion_sim::planetary::architecture::template::template;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{group_masses, solid_budget};
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::sse::{ZCoeffs, zams};
/// use hyperion_sim::units::{Megayears, SolarMasses};
///
/// let (seed, system) = (Seed::new(7), SystemId::from_raw(0x0200_0800_2000_0000)?);
/// let (mass, composition) = (SolarMasses::new(1.0), Composition::SOLAR);
/// let coeffs = ZCoeffs::new(composition.z_fit());
/// let host = DiscHost::new(
///     mass,
///     composition.fe_h(),
///     zams::luminosity(mass, &coeffs),
///     zams::radius(mass, &coeffs),
/// )?;
/// let disc = disc::derive(&host, Megayears::new(2.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// let profile = disc.profile().expect("an untruncated disc");
///
/// let chain = &template(ArchitectureClass::CompactMulti).groups()[0];
/// let members = (1..=4)
///     .map(|n| BodyIndex::new(BodySlot::Planet(n), BodySub::Primary))
///     .collect::<Result<Vec<_>, _>>()?;
/// let group = group_masses(seed, system, chain, profile, &members);
/// assert_eq!(group.masses().len(), 4);
/// // The median solar disc's chains are about 4 M⊕.
/// let typical = group.characteristic().expect("a correlated group").value();
/// assert!((typical - 4.0).abs() < 0.01);
/// assert!(group.total() <= solid_budget(profile));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn group_masses(
    seed: Seed,
    system: SystemId,
    group: &PlanetGroup,
    disc: &DiscProfile,
    members: &[BodyIndex],
) -> GroupMasses {
    debug_assert!(
        members
            .iter()
            .enumerate()
            .all(|(i, a)| members[..i].iter().all(|b| a.slot() != b.slot())),
        "a group's members are in distinct slots, got {members:?}"
    );
    let draws: Vec<MassDraws> = members
        .iter()
        .map(|&member| MassDraws::for_planet(seed, system, member))
        .collect();
    group_masses_from(group, disc, &draws)
}

/// Whether a host's disc can make a giant, and if so by what age its core reaches
/// [`CRITICAL_CORE_MASS`] (P14.T7.c; design note 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GiantCore {
    /// There is no disc.
    NoDisc,
    /// The snow line lies beyond the disc's reach, or there are under 10 M⊕ of solids between
    /// them: D5's condition, as P14.T4.c applies it
    /// ([`ClassConstraints::capacity`] is [`DiscCapacity::SmallPlanets`]).
    TooFewSolids,
    /// The core would reach 10 M⊕ only at age `by`, after the disc has gone.
    TooSlow {
        /// The time from the disc's formation at which the core would reach the critical mass,
        /// beyond the disc's lifetime; infinite where no pebbles reach the core's site.
        by: Megayears,
    },
    /// The core reaches 10 M⊕ at age `by`, within the disc's lifetime.
    Forms {
        /// The time from the disc's formation at which the core reaches the critical mass, from
        /// zero (a seed already that heavy) to the disc's lifetime.
        by: Megayears,
    },
}

impl GiantCore {
    /// Whether the disc can make a giant.
    #[must_use]
    pub const fn forms(&self) -> bool {
        match self {
            Self::Forms { .. } => true,
            Self::NoDisc | Self::TooFewSolids | Self::TooSlow { .. } => false,
        }
    }
}

/// Whether `disc`, in a stable zone ending at `zone`, can make a giant (P14.T7.c): 10 M⊕ of solids
/// beyond the snow line, and a core of [`CRITICAL_CORE_MASS`] grown there within the disc's
/// lifetime.
///
/// The solids are P14.T4.c's condition ([`ClassConstraints::new`]), unchanged. The core starts as
/// the local isolation mass at the snow line, or at the inner edge where that lies beyond it
/// ([`DiscProfile::isolation_mass`], Lissauer 1987): ruling 38 keeps the isolation mass for this,
/// the core a giant starts from. In plan 14's median solar disc it is 0.07 M⊕ there, and
/// planetesimals alone grow no 10 M⊕ core anywhere in the disc (0.2 M⊕ at 5 au). The core then
/// grows by pebble accretion, Lambrechts and Johansen's (2014, A&A 572, A107) eq. 35,
///
/// `M_c`(t)^⅓ = `M_c,0`^⅓ + [11 M⊕ (Z ÷ 0.01)^(25/6) (M★ ÷ M☉)^(−11/12) (β ÷ 500 g cm⁻²)³
/// (r ÷ 10 au)^(−5/4)]^⅓ (t ÷ 1 Myr)^(13/18),
///
/// with the disc's own dust-to-gas ratio Z and gas surface density `Σ_g` = β (r ÷ 1 au)^−1 at the
/// site. Their disc is untapered and their model runs from 5 to 100 au, so applying it at a snow
/// line of 2–3 au in a disc with a 30 au taper is an extrapolation; there the growth is fastest.
/// It reproduces their summary, a core "within about 1 Myr at 5 AU", in the median solar disc
/// (0.50 Myr; 0.32 Myr at its snow line).
///
/// # Panics
///
/// In debug builds, if the zone's end is not positive.
///
/// # Examples
///
/// A Sun-like star's median disc grows a giant's core in a third of a Myr, unless a companion
/// cuts its disc inside the snow line, or the disc is gone before the core is grown.
///
/// ```
/// use hyperion_sim::planetary::architecture::ZoneLimit;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{GiantCore, giant_core};
/// use hyperion_sim::stellar::draws::StandardNormal;
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
/// let median = disc::derive(&sun, Megayears::new(1.7), &DiscDraws::MEDIAN, Truncation::NONE);
/// let GiantCore::Forms { by } = giant_core(&median, ZoneLimit::Unbounded) else {
///     panic!("the median solar disc grows a core");
/// };
/// assert!((0.3..0.35).contains(&by.value()));
///
/// let two_au = Metres::from(AstronomicalUnits::new(2.0));
/// let cut = giant_core(&median, ZoneLimit::Outer(two_au));
/// assert_eq!(cut, GiantCore::TooFewSolids);
///
/// // A lighter disc that lasts only 0.3 Myr has the solids, but not the time.
/// let light = DiscDraws {
///     gas_fraction: StandardNormal::new(-0.5).expect("finite"),
///     ..DiscDraws::MEDIAN
/// };
/// let brief = disc::derive(&sun, Megayears::new(0.3), &light, Truncation::NONE);
/// assert!(matches!(giant_core(&brief, ZoneLimit::Unbounded), GiantCore::TooSlow { .. }));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn giant_core(disc: &Disc, zone: ZoneLimit) -> GiantCore {
    let capacity = ClassConstraints::new(disc, zone, HostMultiplicity::SingleOrWide).capacity();
    let profile = match (capacity, disc) {
        (DiscCapacity::None, _) | (_, Disc::None) => return GiantCore::NoDisc,
        (DiscCapacity::SmallPlanets, _) => return GiantCore::TooFewSolids,
        (DiscCapacity::Giants, Disc::Present(profile)) => profile,
    };
    let site = if profile.snow_line() > profile.inner_edge() {
        profile.snow_line()
    } else {
        profile.inner_edge()
    };
    let by = critical_core_age(profile, site);
    if by <= profile.lifetime() {
        GiantCore::Forms { by }
    } else {
        GiantCore::TooSlow { by }
    }
}

/// The pebble-accretion growth rate of Lambrechts and Johansen's (2014) eq. 35 at `at` in `disc`:
/// the cube root of the core mass gained in the first Myr, in M⊕^⅓, so that `M_c`(t)^⅓ =
/// `M_c,0`^⅓ + this × (t ÷ 1 Myr)^(13/18).
#[must_use]
fn pebble_growth(disc: &DiscProfile, at: Metres) -> f64 {
    let gas = disc.gas_surface_density(at).value();
    if gas <= 0.0 {
        return 0.0;
    }
    let dust_to_gas = disc.surface_density(at).value() / gas;
    let r_au = at.value() / METRES_PER_AU;
    let beta = gas * r_au * G_CM2_PER_KG_M2;
    let ratio = beta / PEBBLE_REFERENCE_GAS_SCALE_G_CM2;
    let at_one_myr = PEBBLE_CORE_MASS_AT_REFERENCE.value()
        * math::powf(dust_to_gas / PEBBLE_REFERENCE_DUST_TO_GAS, 25.0 / 6.0)
        * math::powf(disc.host_mass().value(), -11.0 / 12.0)
        * (ratio * ratio * ratio)
        * math::powf(r_au / PEBBLE_REFERENCE_RADIUS_AU, -5.0 / 4.0);
    math::cbrt(at_one_myr)
}

/// The age at which a core seeded with the local isolation mass at `at` reaches
/// [`CRITICAL_CORE_MASS`] by pebble accretion (Lambrechts and Johansen 2014, eq. 35): zero if the
/// seed is already that heavy, and infinite if no pebbles reach it.
#[must_use]
fn critical_core_age(disc: &DiscProfile, at: Metres) -> Megayears {
    let seed = math::cbrt(disc.isolation_mass(at).value());
    let needed = math::cbrt(CRITICAL_CORE_MASS.value()) - seed;
    if needed <= 0.0 {
        return Megayears::ZERO;
    }
    let growth = pebble_growth(disc, at);
    if growth <= 0.0 {
        return Megayears::new(f64::INFINITY);
    }
    Megayears::new(math::powf(needed / growth, 18.0 / 13.0))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::BodyId;
    use crate::id::Layer;
    use crate::planetary::architecture::template::{
        CHAIN_COUNT, ClassTemplate, CountLaw, TEMPLATES, template,
    };
    use crate::planetary::architecture::{ArchitectureClass, GIANT_SOLID_BUDGET, class_weights};
    use crate::planetary::derive::radius::radius_chen_kipping;
    use crate::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::premain::disc_lifetime;
    use crate::stellar::sse::{ZCoeffs, zams};
    use crate::units::{AstronomicalUnits, Dex, HeliumExcess, SolarMasses};

    const SEED: Seed = Seed::new(0x5eed_0000_0014_0007);

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [-7, 31, 2]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn planet(slot: u8) -> BodyIndex {
        BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap()
    }

    fn slots(first: u8, count: u8) -> Vec<BodyIndex> {
        (first..first + count).map(planet).collect()
    }

    /// A host of plan 06's zero-age state at `mass` and `fe_h`.
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

    fn profile(disc: Disc) -> DiscProfile {
        *disc.profile().expect("a disc")
    }

    /// The disc of `host` at the median variates, living `lifetime` Myr.
    fn median_disc(host: &DiscHost, lifetime: f64) -> DiscProfile {
        profile(disc::derive(
            host,
            Megayears::new(lifetime),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        ))
    }

    /// The disc of `host` at \[Fe/H\] `fe_h` (its zero-age luminosity and radius kept) and gas
    /// fraction variate `gas`, the others at their medians, living 2 Myr.
    fn median_disc_with(host: &DiscHost, fe_h: f64, gas: f64) -> DiscProfile {
        let host = DiscHost::new(
            host.mass(),
            Dex::new(fe_h),
            host.zams_luminosity(),
            host.zams_radius(),
        )
        .unwrap();
        let draws = DiscDraws {
            gas_fraction: StandardNormal::new(gas).unwrap(),
            ..DiscDraws::MEDIAN
        };
        profile(disc::derive(
            &host,
            Megayears::new(2.0),
            &draws,
            Truncation::NONE,
        ))
    }

    /// Host 0's disc of system `index`, as drawn, living 2.5 Myr.
    fn drawn_disc(host: &DiscHost, index: u32) -> DiscProfile {
        let draws = DiscDraws::for_host(SEED, system(index), 0);
        profile(disc::derive(
            host,
            Megayears::new(2.5),
            &draws,
            Truncation::NONE,
        ))
    }

    fn chain() -> &'static PlanetGroup {
        &template(ArchitectureClass::CompactMulti).groups()[0]
    }

    fn median(values: &mut [f64]) -> f64 {
        values.sort_by(f64::total_cmp);
        values[values.len() / 2]
    }

    fn pearson(pairs: &[(f64, f64)]) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a sample of pairs is far below 2⁵³"
        )]
        let n = pairs.len() as f64;
        let (mx, my) = pairs
            .iter()
            .fold((0.0, 0.0), |(x, y), &(a, b)| (x + a / n, y + b / n));
        let (sxy, sxx, syy) = pairs.iter().fold((0.0, 0.0, 0.0), |(xy, xx, yy), &(a, b)| {
            let (dx, dy) = (a - mx, b - my);
            (xy + dx * dy, xx + dx * dx, yy + dy * dy)
        });
        sxy / (sxx * syy).sqrt()
    }

    /// A zero-truncated Poisson count of `law` for a host of `mass`, held at its cap, by
    /// inversion on `rng`: the placer's count (P14.T8), which this module does not draw.
    fn chain_count(law: CountLaw, mass: f64, rng: &mut Lcg) -> u8 {
        let lambda = law.poisson_rate(SolarMasses::new(mass)).unwrap();
        let (_, max) = law.range();
        loop {
            let u = rng.next_f64();
            let (mut k, mut p) = (0_u8, math::exp(-lambda));
            let mut cumulative = p;
            while u > cumulative && k < max {
                k += 1;
                p *= lambda / f64::from(k);
                cumulative += p;
            }
            if k >= 1 {
                return k;
            }
        }
    }

    #[test]
    fn draws_are_words_eight_s_onwards_of_the_system_stream() {
        let id = system(5);
        for slot in [1_u8, 2, 9, 191] {
            let mut stream = Stream::open(SEED, tags::PLANET_MASS, ObjectKey::from(id));
            stream.seek(8 * u64::from(slot));
            let draws = MassDraws::for_planet(SEED, id, planet(slot));
            assert_same_bits(draws.scatter.value(), stream.standard_normal());
            assert_same_bits(draws.group.value(), stream.standard_normal());
            assert_same_bits(draws.rank.value(), stream.uniform_open());
            assert!(stream.position() <= 8 * u64::from(slot) + MASS_WORDS_PER_SLOT);
        }
        let second = BodyIndex::new(BodySlot::SecondGeneration(0), BodySub::Primary).unwrap();
        let mut stream = Stream::open(SEED, tags::PLANET_MASS, ObjectKey::from(id));
        stream.seek(8 * 0xC0);
        assert_same_bits(
            MassDraws::for_planet(SEED, id, second).scatter.value(),
            stream.standard_normal(),
        );
    }

    #[test]
    fn a_planet_s_draws_do_not_depend_on_what_was_drawn_before() {
        let keys: Vec<(u32, u8)> = (0..5)
            .flat_map(|s| [1_u8, 2, 3, 40].map(move |p| (s, p)))
            .collect();
        assert_order_independent(&keys, |&(s, p)| {
            MassDraws::for_planet(SEED, system(s), planet(p))
        });
        assert_ne!(
            MassDraws::for_planet(SEED, system(0), planet(1)),
            MassDraws::for_planet(SEED, system(0), planet(2))
        );
        assert_ne!(
            MassDraws::for_planet(SEED, system(0), planet(1)),
            MassDraws::for_planet(SEED, system(1), planet(1))
        );
    }

    #[test]
    fn the_same_inputs_give_the_same_masses_twice() {
        let disc = drawn_disc(&zams_host(0.9, -0.1), 3);
        for group in TEMPLATES.iter().flat_map(ClassTemplate::groups) {
            let run = || group_masses(SEED, system(3), group, &disc, &slots(2, 5));
            assert_eq!(run(), run());
        }
    }

    #[test]
    fn the_reference_solid_mass_is_the_median_solar_disc_s() {
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        let solids = disc.solid_mass() / REFERENCE_SOLID_MASS;
        assert!((solids - 1.0).abs() < 2e-4, "{}", disc.solid_mass().value());
        // So its compact groups are 4 M⊕ and its rocky groups 0.5 M⊕, exactly as far as the
        // constant's four figures allow.
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        for (group, expected) in [(chain(), 4.0), (rocky, 0.5)] {
            let m = characteristic_mass(group, disc.solid_mass(), StandardNormal::ZERO);
            assert!((m.value() / expected - 1.0).abs() < 2e-4, "{m:?}");
        }
    }

    /// Ruling 38, point 4: the local isolation mass cannot make the compact classes' planets, and
    /// the whole disc holds Chiang and Laughlin's minimum-mass extrasolar nebula.
    #[test]
    fn close_in_planets_need_the_whole_disc_s_solids() {
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        // Ten times the isolation mass at 0.1 au is 2.3 × 10⁻³ M⊕, against a chain's 1–20 M⊕.
        let local = 10.0 * disc.isolation_mass(au(0.1)).value();
        assert!((1e-3..1e-2).contains(&local), "{local} M⊕");
        // Chiang and Laughlin's (2013) eq. 4, 620 g cm⁻² (a ÷ 0.2 au)^−1.6, holds 12.7 M⊕ between
        // 0.05 and 0.5 au; the median solar disc has a fiftieth of that there.
        let sigma0 = 620.0 / G_CM2_PER_KG_M2;
        let r0 = au(0.2).value();
        let ring = |a: f64| math::powf(au(a).value() / r0, 0.4);
        let mmen = core::f64::consts::TAU * sigma0 * r0 * r0 / 0.4 * (ring(0.5) - ring(0.05))
            / crate::units::consts::EARTH_MASS_KG;
        assert!((mmen - 12.7).abs() < 0.1, "{mmen} M⊕");
        let in_situ = disc.solid_mass_between(au(0.05), au(0.5)).value();
        assert!(in_situ < mmen / 30.0, "{in_situ} M⊕ in situ");
        assert!(solid_budget(&disc).value() > 2.0 * mmen);
    }

    /// P14.T7.a: the median characteristic mass of 10⁴ compact groups in solar discs is 3.6–4.4
    /// M⊕, and doubles with the solid mass.
    #[test]
    fn compact_groups_in_solar_discs_have_a_median_of_four_earth_masses() {
        let sun = zams_host(1.0, 0.0);
        let median_of = |fe_h: f64| {
            // The Sun's zero-age luminosity and radius at every [Fe/H], so that only the metals
            // change.
            let host = DiscHost::new(
                sun.mass(),
                Dex::new(fe_h),
                sun.zams_luminosity(),
                sun.zams_radius(),
            )
            .unwrap();
            let mut masses: Vec<f64> = (0..10_000)
                .map(|i| {
                    let disc = drawn_disc(&host, i);
                    let group = MassDraws::for_planet(SEED, system(i), planet(1)).group;
                    characteristic_mass(chain(), disc.solid_mass(), group).value()
                })
                .collect();
            median(&mut masses)
        };
        let solar = median_of(0.0);
        assert!((3.6..4.4).contains(&solar), "{solar} M⊕");
        // [Fe/H] = log₁₀ 2 doubles every disc's solids, and the gas does not change.
        let doubled = median_of(core::f64::consts::LOG10_2);
        assert!(
            (doubled / solar - 2.0).abs() < 1e-9,
            "{doubled} against {solar}"
        );
        // It is proportional to the solid mass in any one disc, inside the template's range.
        let disc = drawn_disc(&zams_host(1.0, 0.0), 11);
        let one = characteristic_mass(chain(), disc.solid_mass(), StandardNormal::ZERO);
        let two = characteristic_mass(chain(), disc.solid_mass() * 2.0, StandardNormal::ZERO);
        assert!((two / one - 2.0).abs() < 1e-12);
    }

    /// The compact sample of P14.T7.b: 2,000 systems of solar discs, each a cold chain of the
    /// template's count at slots 1 onwards; for each adjacent pair, the log masses and the log
    /// radii of P14.T11 (each planet's own quantile, here from an LCG, as P14.T16 will draw it on
    /// `planet.radius`).
    struct CompactSample {
        masses: Vec<(f64, f64)>,
        radii: Vec<(f64, f64)>,
        groups: Vec<(GroupMasses, DiscProfile)>,
    }

    fn compact_sample() -> CompactSample {
        let host = zams_host(1.0, 0.0);
        let mut rng = Lcg::new(0x5eed_0014_0007);
        let mut sample = CompactSample {
            masses: Vec::new(),
            radii: Vec::new(),
            groups: Vec::new(),
        };
        for i in 0..2_000 {
            let disc = drawn_disc(&host, i);
            let count = chain_count(CHAIN_COUNT, 1.0, &mut rng);
            let group = group_masses(SEED, system(i), chain(), &disc, &slots(1, count));
            let radii: Vec<f64> = group
                .masses()
                .iter()
                .map(|&m| {
                    let quantile = loop {
                        if let Some(q) = UnitUniform::new(rng.next_f64()) {
                            break q;
                        }
                    };
                    math::log10(radius_chen_kipping(m, quantile).value())
                })
                .collect();
            for (pair, radius) in group.masses().windows(2).zip(radii.windows(2)) {
                sample
                    .masses
                    .push((math::log10(pair[0].value()), math::log10(pair[1].value())));
                sample.radii.push((radius[0], radius[1]));
            }
            sample.groups.push((group, disc));
        }
        sample
    }

    /// P14.T7.b's correlation, calibrated on the radii and measured on the masses.
    #[test]
    fn adjacent_planets_are_peas_in_a_pod() {
        let sample = compact_sample();
        assert!(sample.masses.len() > 4_000, "{} pairs", sample.masses.len());
        let radius_r = pearson(&sample.radii);
        let mass_r = pearson(&sample.masses);
        let outer_heavier = share(&sample.masses);
        let outer_larger = share(&sample.radii);
        // Weiss et al.'s (2018) 0.65 in log radius, the calibration's target.
        assert!((0.60..0.70).contains(&radius_r), "{radius_r}");
        // P14.T7.b's window for the log masses, corrected for Chen and Kipping's scatter.
        assert!((0.80..0.90).contains(&mass_r), "{mass_r}");
        assert!((0.55..0.75).contains(&outer_heavier), "{outer_heavier}");
        // Short of Weiss et al.'s 65.4%, which they tie to photo-evaporation (P14.T13).
        assert!((0.55..0.65).contains(&outer_larger), "{outer_larger}");
    }

    /// The share of pairs whose outer member is the larger.
    fn share(pairs: &[(f64, f64)]) -> f64 {
        let outer = pairs.iter().filter(|(inner, outer)| outer > inner).count();
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of pairs are far below 2⁵³"
        )]
        let share = outer as f64 / pairs.len() as f64;
        share
    }

    /// P14.T7.b: no group exceeds its disc's solids.
    #[test]
    fn no_group_exceeds_its_disc_s_solids() {
        let sample = compact_sample();
        let mut capped = 0;
        for (group, disc) in &sample.groups {
            let budget = solid_budget(disc).value();
            assert!(group.total().value() <= budget * (1.0 + 1e-12));
            if group.cap() == GroupCap::SolidBudget {
                capped += 1;
                assert!((group.total().value() / budget - 1.0).abs() < 1e-12);
            }
        }
        // About a tenth of Sun-like chains take the whole disc (197 of 2,000).
        assert!(
            (150..250).contains(&capped),
            "{capped} of 2,000 held to the budget"
        );
        // Every template's groups in an M dwarf's poor disc, where the budget binds.
        let poor = profile(disc::derive(
            &zams_host(0.3, -0.5),
            Megayears::new(2.5),
            &DiscDraws {
                gas_fraction: StandardNormal::new(-1.5).unwrap(),
                ..DiscDraws::MEDIAN
            },
            Truncation::NONE,
        ));
        for group in TEMPLATES.iter().flat_map(ClassTemplate::groups) {
            let masses = group_masses(SEED, system(9), group, &poor, &slots(1, 10));
            let limit = if group.places_giants() {
                gas_budget(&poor)
            } else {
                solid_budget(&poor)
            };
            assert!(masses.total().value() <= limit.value() * (1.0 + 1e-12));
            assert!(masses.masses().iter().all(|m| m.value() > 0.0));
        }
    }

    /// The budget scales a group's members by one factor, so their ratios hold, and it outranks
    /// the template's floor.
    #[test]
    fn a_group_held_to_its_budget_keeps_its_members_ratios() {
        // A metal-poor disc with a tenth of the solids, where six planets at the template's floor
        // of 1 M⊕ would already be too many.
        let poor = profile(disc::derive(
            &zams_host(1.0, -1.0),
            Megayears::new(2.5),
            &DiscDraws {
                gas_fraction: StandardNormal::new(-1.0).unwrap(),
                ..DiscDraws::MEDIAN
            },
            Truncation::NONE,
        ));
        assert!(
            solid_budget(&poor).value() < 6.0,
            "{:?}",
            solid_budget(&poor)
        );
        let draws: Vec<MassDraws> = slots(1, 6)
            .iter()
            .map(|&m| MassDraws::for_planet(SEED, system(0), m))
            .collect();
        let held_down = group_masses_from(chain(), &poor, &draws);
        assert_eq!(held_down.cap(), GroupCap::SolidBudget);
        assert!(held_down.masses().iter().any(|m| m.value() < 1.0));
        // Each member is its drawn mass times one factor.
        let m_c = characteristic_mass(chain(), poor.solid_mass(), draws[0].group);
        let factors: Vec<f64> = draws
            .iter()
            .enumerate()
            .zip(held_down.masses())
            .map(|((i, d), held)| {
                let unscaled =
                    member_mass(m_c, d.scatter, steps_from_middle(i, 6), chain().masses());
                *held / unscaled
            })
            .collect();
        for factor in &factors {
            assert!((factor / factors[0] - 1.0).abs() < 1e-14, "{factors:?}");
        }
        assert!(factors[0] < 1.0);
        // The median solar disc does not hold the same draws down.
        let rich = median_disc(&zams_host(1.0, 0.0), 2.0);
        assert_eq!(
            group_masses_from(chain(), &rich, &draws).cap(),
            GroupCap::AsDrawn
        );
    }

    /// P14.T7.b: changing the template of one group leaves the masses of the others
    /// bit-identical.
    #[test]
    fn changing_one_group_s_template_leaves_the_others_bit_identical() {
        let disc = drawn_disc(&zams_host(1.1, 0.2), 4);
        let cold = template(ArchitectureClass::CompactWithColdGiant).groups();
        let (chain_group, giants) = (&cold[0], &cold[1]);
        let (chain_slots, giant_slots) = (slots(1, 5), slots(6, 2));
        let chain_masses = group_masses(SEED, system(4), chain_group, &disc, &chain_slots);
        let giant_masses = group_masses(SEED, system(4), giants, &disc, &giant_slots);
        // The giants' group becomes a Solar-like system's ice giants, and then a rocky group:
        // the chain does not move.
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        let rocky = &template(ArchitectureClass::SolarLike).groups()[0];
        for other in [ice, rocky] {
            let _ = group_masses(SEED, system(4), other, &disc, &giant_slots);
            let again = group_masses(SEED, system(4), chain_group, &disc, &chain_slots);
            for (a, b) in again.masses().iter().zip(chain_masses.masses()) {
                assert_same_bits(a.value(), b.value());
            }
        }
        // The chain becomes a substellar chain or a rocky group: the giants do not move.
        let substellar = &template(ArchitectureClass::SubstellarCompact).groups()[0];
        for other in [substellar, rocky] {
            let changed = group_masses(SEED, system(4), other, &disc, &chain_slots);
            assert_ne!(changed.masses(), chain_masses.masses());
            let again = group_masses(SEED, system(4), giants, &disc, &giant_slots);
            for (a, b) in again.masses().iter().zip(giant_masses.masses()) {
                assert_same_bits(a.value(), b.value());
            }
        }
    }

    #[test]
    fn members_step_outward_about_the_characteristic_mass() {
        assert_same_bits(steps_from_middle(0, 1), 0.0);
        assert_same_bits(steps_from_middle(0, 4), -1.5);
        assert_same_bits(steps_from_middle(3, 4), 1.5);
        assert_same_bits(steps_from_middle(2, 5), 0.0);
        // With no scatter, each member is 0.1 dex heavier than the one inside it, and the
        // middle one is the characteristic mass.
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        let draws = [MassDraws::MEDIAN; 5];
        let group = group_masses_from(chain(), &disc, &draws);
        let m_c = group.characteristic().unwrap().value();
        let masses = group.masses();
        assert!((masses[2].value() / m_c - 1.0).abs() < 1e-15);
        for pair in masses.windows(2) {
            assert!((pair[1] / pair[0] - math::exp10(0.1)).abs() < 1e-12);
        }
        // Held to the template's range at both ends.
        let far = [MassDraws {
            scatter: StandardNormal::new(8.0).unwrap(),
            ..MassDraws::MEDIAN
        }];
        assert_same_bits(
            group_masses_from(chain(), &disc, &far).masses()[0].value(),
            20.0,
        );
        let near = [MassDraws {
            scatter: StandardNormal::new(-8.0).unwrap(),
            ..MassDraws::MEDIAN
        }];
        assert_same_bits(
            group_masses_from(chain(), &disc, &near).masses()[0].value(),
            1.0,
        );
        assert!(group_masses_from(chain(), &disc, &[]).masses().is_empty());
        assert_eq!(
            group_masses_from(chain(), &disc, &[]).characteristic(),
            None
        );
    }

    #[test]
    fn every_template_group_has_a_reference_mass_inside_its_range() {
        let substellar = &template(ArchitectureClass::SubstellarCompact).groups()[0];
        for group in TEMPLATES.iter().flat_map(ClassTemplate::groups) {
            let range = group.masses();
            // A substellar chain's hosts have discs of a few hundredths of the solar one: 4 M⊕
            // at 0.08 M☉ is 0.32 M⊕, inside its range.
            let reference = if group == substellar {
                reference_mass(group) * 0.08
            } else {
                reference_mass(group)
            };
            assert!(
                range.min() <= reference && reference <= range.max(),
                "{group:?}"
            );
        }
        let rocky = &template(ArchitectureClass::SolarLike).groups()[0];
        assert_same_bits(reference_mass(rocky).value(), 0.5);
        let companions = &template(ArchitectureClass::WarmGiant).groups()[1];
        assert_same_bits(reference_mass(companions).value(), 4.0);
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        assert!((reference_mass(ice).value() - 300.0_f64.sqrt()).abs() < 1e-12);
    }

    /// P14.T7.c and the other laws: each mass is its law's quantile, so the masses of many
    /// planets follow the law.
    #[test]
    fn law_masses_follow_their_laws() {
        // A disc rich enough that no limit holds one planet of either law.
        let disc = profile(disc::derive(
            &zams_host(1.0, 0.0),
            Megayears::new(2.0),
            &DiscDraws {
                gas_fraction: StandardNormal::new(1.0).unwrap(),
                ..DiscDraws::MEDIAN
            },
            Truncation::NONE,
        ));
        let giants = &template(ArchitectureClass::EccentricGiant).groups()[1];
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        assert_eq!(giants.masses().law(), MassLaw::PowerLaw { index: -0.31 });
        assert_eq!(ice.masses().law(), MassLaw::LogUniform);
        for group in [giants, ice] {
            let range = group.masses();
            let (lo, hi) = (range.min().value(), range.max().value());
            let law = range.law();
            let cdf = |m: f64| {
                match law {
                    MassLaw::PowerLaw { index } => {
                        (math::powf(m, index) - math::powf(lo, index))
                            / (math::powf(hi, index) - math::powf(lo, index))
                    }
                    MassLaw::LogUniform => {
                        (math::ln(m) - math::ln(lo)) / (math::ln(hi) - math::ln(lo))
                    }
                    MassLaw::Correlated => unreachable!("both groups draw from a law"),
                }
                .clamp(0.0, 1.0)
            };
            let mut masses: Vec<f64> = (0..10_000)
                .map(|i| {
                    let one = group_masses(SEED, system(i), group, &disc, &slots(1, 1));
                    assert_eq!(one.cap(), GroupCap::AsDrawn);
                    assert!(one.characteristic().is_none());
                    one.masses()[0].value()
                })
                .collect();
            assert!(masses.iter().all(|&m| (lo..=hi).contains(&m)));
            let ks = ks_one_sample(&mut masses, cdf);
            assert_p_value(&format!("{:?}", group.role()), ks.p_value, ALPHA);
        }
        // The ranks' ends map to the range's ends.
        let range = giants.masses();
        let low = UnitUniform::new(1e-15).unwrap();
        let high = UnitUniform::new(1.0 - 1e-15).unwrap();
        assert!((law_mass(range, low) / range.min() - 1.0).abs() < 1e-12);
        assert!((law_mass(range, high) / range.max() - 1.0).abs() < 1e-12);
    }

    /// P14.T7.c: no giant exceeds its disc's gas mass.
    #[test]
    fn no_giant_exceeds_its_disc_s_gas_mass() {
        let mut capped = 0;
        for (mass, fe_h) in [(1.0, 0.0), (0.5, 0.0), (0.3, -0.3), (2.0, 0.2)] {
            let host = zams_host(mass, fe_h);
            for i in 0..500 {
                let disc = drawn_disc(&host, i);
                for class in ArchitectureClass::ALL {
                    for group in template(class)
                        .groups()
                        .iter()
                        .filter(|g| g.places_giants())
                    {
                        let giants = group_masses(SEED, system(i), group, &disc, &slots(1, 3));
                        let gas = gas_budget(&disc).value();
                        assert!(giants.total().value() <= gas * (1.0 + 1e-12));
                        assert!(giants.masses().iter().all(|m| m.value() <= gas));
                        if giants.cap() == GroupCap::GasMass {
                            capped += 1;
                        }
                    }
                }
            }
        }
        assert!(capped > 0, "some poor discs hold their giants to their gas");
    }

    /// P14.T7.c: a disc with under 10 M⊕ of solids beyond the snow line yields D5's fallback.
    #[test]
    fn a_disc_with_too_few_solids_beyond_the_snow_line_falls_back() {
        // An M dwarf's median disc has just under 10 M⊕ beyond its snow line.
        let m_dwarf = median_disc(&zams_host(0.3, 0.0), 2.5);
        let beyond = m_dwarf.solid_mass_between(m_dwarf.snow_line(), m_dwarf.outer_edge());
        assert!(beyond < GIANT_SOLID_BUDGET, "{beyond:?}");
        let disc = Disc::Present(m_dwarf);
        assert_eq!(
            giant_core(&disc, ZoneLimit::Unbounded),
            GiantCore::TooFewSolids
        );
        let constraints =
            ClassConstraints::new(&disc, ZoneLimit::Unbounded, HostMultiplicity::SingleOrWide);
        let weights = class_weights(SolarMasses::new(0.3), Dex::new(0.0));
        assert_same_bits(weights.constrained(&constraints).giant_total(), 0.0);
        // A solar disc cut inside its snow line likewise, and no disc at all.
        let sun = disc::derive(
            &zams_host(1.0, 0.0),
            Megayears::new(2.5),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        );
        assert_eq!(
            giant_core(&sun, ZoneLimit::Outer(au(2.0))),
            GiantCore::TooFewSolids
        );
        assert_eq!(
            giant_core(&Disc::None, ZoneLimit::Unbounded),
            GiantCore::NoDisc
        );
        // The median solar disc grows its core in a third of a Myr.
        let core = giant_core(&sun, ZoneLimit::Unbounded);
        assert!(core.forms(), "{core:?}");
    }

    /// Lambrechts and Johansen's (2014) eq. 35 at their own reference, and their summary: a core
    /// within about 1 Myr at 5 au. A planetesimal core alone never reaches 10 M⊕ in the median
    /// solar disc.
    #[test]
    fn pebble_cores_grow_as_lambrechts_and_johansen_find() {
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        // The disc's own figures at 10 au, scaled to their reference by eq. 35's powers.
        let at = au(10.0);
        let gas = disc.gas_surface_density(at).value() * 10.0 * G_CM2_PER_KG_M2;
        let z = disc.surface_density(at).value() / disc.gas_surface_density(at).value();
        let growth = pebble_growth(&disc, at);
        let reference = growth * growth * growth
            / (math::powf(z / 0.01, 25.0 / 6.0) * math::powf(gas / 500.0, 3.0));
        assert!((reference - 11.0).abs() < 1e-9, "{reference} M⊕");
        let at_five = critical_core_age(&disc, au(5.0)).value();
        assert!((0.3..1.0).contains(&at_five), "{at_five} Myr at 5 au");
        // The planetesimal isolation mass peaks near r_c, under 1 M⊕.
        let most = (1..=90)
            .map(|a| disc.isolation_mass(au(f64::from(a))).value())
            .fold(0.0, f64::max);
        assert!(most < 1.0, "{most} M⊕");
        assert!((0.1..0.3).contains(&disc.isolation_mass(au(5.0)).value()));
        // Outside the disc there is no gas and no growth.
        assert_same_bits(critical_core_age(&disc, au(200.0)).value(), f64::INFINITY);
        // In the heaviest, most metal-rich disc a planetesimal core is already critical near r_c.
        let heavy = median_disc_with(&zams_host(1.0, 0.0), 0.5, 2.0);
        assert!(heavy.isolation_mass(au(30.0)) > CRITICAL_CORE_MASS);
        assert_same_bits(critical_core_age(&heavy, au(30.0)).value(), 0.0);
        let core = giant_core(&Disc::Present(disc), ZoneLimit::Unbounded);
        match core {
            GiantCore::Forms { by } => assert!((0.3..0.35).contains(&by.value()), "{by:?}"),
            other => panic!("{other:?}"),
        }
    }

    /// The share of discs that can make a giant, by the solids alone (P14.T4.c) and with the core
    /// grown within the disc's lifetime (P14.T7.c), each disc living its star's own lifetime: the
    /// figures the orchestrator rules on, pinned.
    #[test]
    fn the_core_s_lifetime_condition_removes_some_giant_capable_discs() {
        let cases = [
            (1.0, 0.0, 0.80..0.87, 0.66..0.76),
            (1.0, -0.5, 0.44..0.53, 0.25..0.33),
            (1.0, 0.3, 0.92..0.96, 0.85..0.92),
            (0.5, 0.0, 0.62..0.70, 0.60..0.68),
            (1.5, 0.0, 0.85..0.92, 0.58..0.66),
        ];
        for (mass, fe_h, solids_window, core_window) in cases {
            let host = zams_host(mass, fe_h);
            let (mut solids, mut both) = (0_u32, 0_u32);
            for i in 0..4_000 {
                let draws = DiscDraws::for_host(SEED, system(i), 0);
                let star = StarDraws::for_star(SEED, BodyId::new(system(i), 0));
                let lifetime = disc_lifetime(host.mass(), star.disc_lifetime());
                let disc = disc::derive(&host, lifetime, &draws, Truncation::NONE);
                match giant_core(&disc, ZoneLimit::Unbounded) {
                    GiantCore::Forms { .. } => (solids, both) = (solids + 1, both + 1),
                    GiantCore::TooSlow { .. } => solids += 1,
                    GiantCore::NoDisc | GiantCore::TooFewSolids => {}
                }
            }
            let (solids, both) = (f64::from(solids) / 4_000.0, f64::from(both) / 4_000.0);
            assert!(
                solids_window.contains(&solids) && core_window.contains(&both),
                "{mass} M☉, [Fe/H] {fe_h}: solids {solids}, core in time {both}"
            );
        }
    }
}
