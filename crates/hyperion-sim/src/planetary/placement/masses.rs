//! Planet masses: a characteristic mass per group, members correlated about it ("peas in a pod"),
//! and every group held to what its disc can supply (plan 14, P14.T7; design notes 4 and 5;
//! rulings 38, 55 and 60).
//!
//! # The model
//!
//! - **Characteristic mass** (P14.T7.a, as ruling 60 calibrates it). A group of the correlated law
//!   ([`MassLaw::Correlated`]) has one characteristic mass `m_c`, held to the template's range,
//!   whose median follows how the group grew ([`is_drift_fed`], [`characteristic_mass`]):
//!   - a *drift-fed* group, a compact chain, a warm giant's companions or a substellar chain:
//!     `m_c` = 4 M⊕ × (M★ ÷ M☉) × 10^(`σ_b` z), with `σ_b` = 0.5 dex
//!     ([`BETWEEN_SYSTEM_SCATTER_DEX`]). The host sets it; the disc's solids and metallicity do not;
//!   - an *in-situ* rocky group: `m_c` = 0.5 M⊕ × (`M_s` ÷ [`REFERENCE_SOLID_MASS`]), as P14.T7.a
//!     wrote it, `M_s` being the whole solid mass of the host's disc between its edges, already
//!     cut to the host's stable zone (P14.T9).
//! - **Members** (P14.T7.b). Member i of n, counted inside out, has
//!   log₁₀ mᵢ = log₁₀ `m_c` + `σ_w` εᵢ + s (i − (n − 1) ÷ 2), held to the template's range: the
//!   same scatter `σ_w` = 0.2 dex ([`WITHIN_SYSTEM_SCATTER_DEX`]) for every member, and s more per
//!   step outward, centred on the group so that `m_c` stays its typical mass whatever its count:
//!   s = 0.21 dex for a drift-fed group ([`OUTWARD_STEP_DEX`]) and 0 for a rocky one
//!   ([`ROCKY_OUTWARD_STEP_DEX`]).
//! - **The solid budget** (ruling 38, point 4; ruling 60). A group's members together hold at most
//!   [`solid_budget`], three times its disc's whole solid mass. A correlated group whose drawn
//!   members hold more forms only its innermost members that fit, each at its drawn mass, and
//!   none if even the first does not fit ([`GroupCap::Truncated`]), so that no member lies under
//!   its floor (ruling 66: a chain of 1 M⊕ × M★ planets cannot be built from less; Lambrechts et
//!   al. 2019, a poor pebble flux grows no super-Earths). A group drawn from a law is scaled down,
//!   every member by one factor. A starved disc makes fewer or smaller planets, never more mass
//!   than it had.
//! - **Giants** (P14.T7.c). A group whose range reaches giants ([`PlanetGroup::places_giants`])
//!   takes each mass from its template's law, Cumming et al.'s dN ÷ d ln M ∝ M^−0.31, and is held
//!   together to the disc's gas mass ([`gas_budget`]) in the same way. The ice-rich bodies, the
//!   ice giants and the survivor draw from their laws (log-uniform) and are held to the solid
//!   budget. Whether a host can make a giant at all is [`giant_core`]: 10 M⊕ of solids beyond the
//!   snow line, and a core of 10 M⊕ grown there within the disc's lifetime.
//!
//! Each group's limit is its own, so that changing one group's template never moves another
//! group's masses (P14.T7.b's test). A system's groups together can therefore hold more than its
//! budget: a `CompactWithColdGiant` chain may take up to all of it while its giants' cores also
//! need 10 M⊕ of solids.
//!
//! # Why a drift-fed group's mass follows its host (ruling 60)
//!
//! P14.T7.a scaled every `m_c` as the disc's solids, `M_s` ∝ M★ f 10^\[Fe/H\] with the gas
//! fraction f's 0.5 dex of scatter. Placed (P14.T10.b), that put a third of an M dwarf's chain
//! planets under 1 M⊕, halved the small planets at \[Fe/H\] = −0.8, and passed the disc's scatter
//! into the adjacent planets' correlation (0.78 about FGK hosts against Weiss et al.'s 0.65).
//! Three measurements say the masses of close-in planets do not follow their discs' solids:
//!
//! - Pascucci et al. (2018, ApJ 856, L28, §2 and Table 1) find the occurrence of Kepler's planets
//!   inside 100 days a broken power law in the planet-to-star mass ratio, breaking at 2.9, 2.8
//!   and 2.8 × 10⁻⁵ for M, K and G hosts, so that "the most common planetary mass inside the
//!   snowline is not fixed but rather increases linearly with stellar mass, from ∼3.5-4.5 M⊕
//!   around M dwarfs up to ∼8-9 M⊕ around G stars", while dust discs' masses scale more steeply
//!   than linearly (Pascucci et al. 2016, ApJ 831, 125, abstract: `M_dust` ∝ M★^1.3–1.9); their §4
//!   notes that the pebble isolation mass scales linearly with the star's mass.
//! - Zhu (2019, ApJ 873, 8, §2) finds the hosts of 1–2 R⊕ and of 2–4 R⊕ planets with "statistically
//!   the same" metallicities (KS p = 0.75), so a planet's size does not follow its disc's metals;
//!   the occurrence does, weakly (§4: from 25% of stars at \[Fe/H\] ≈ −0.2 to 36% at +0.2), and
//!   P14.T4.b's class weights carry that.
//! - Lambrechts et al. (2019, A&A 627, A83, abstract): where the pebble flux is high, embryos
//!   migrate to the disc's inner edge and form "closely spaced super-Earths in the 5 to 20
//!   Earth-mass range, bounded by the pebble isolation mass"; a poorer disc makes fewer of them,
//!   or a terrestrial system instead. So a disc short of its chain's solids forms fewer planets
//!   here, not lighter ones.
//!
//! An in-situ rocky group keeps P14.T7.a's law: "in the standard scenario of terrestrial planet
//! formation, the average planet mass scales almost linearly with the local surface density of
//! the planetesimal disk" (Pascucci et al. 2018, §4, after Kokubo et al. 2006).
//!
//! The 4 M⊕ median is plan 14's. Pascucci et al.'s G hosts' law (Table 1: indices 0.76 below the
//! break and −2.9 above) has its median at 0.55 of the break, 5.1 M⊕ at 1 M☉, if extended to the
//! smallest masses, and at 0.70 of it, 6.5 M⊕, inside the range they fitted, (0.5–8) × 10⁻⁵;
//! Mulders et al. (2021, §2.1) find a de-biased median of about 5 M⊕ for the radial-velocity
//! planets. The plan's 4 M⊕ lies 20–40% under those, at the edge of their errors, and is kept as
//! the plan's (ruling 55.4).
//!
//! # The solid budget (ruling 38, point 4; ruling 60)
//!
//! P14.T7.b capped each mass at 10 times the local isolation mass ([`DiscProfile::isolation_mass`],
//! Lissauer 1987). In plan 14's median solar disc that is 2 × 10⁻³ M⊕ at 0.1 au (the isolation
//! mass there is 2.3 × 10⁻⁴ M⊕), where the compact classes place planets of 1–20 M⊕: planets that
//! close in cannot be made from the solids of their own annulus. Chiang and Laughlin (2013, MNRAS
//! 431, 3444, eq. 4) turn the Kepler planets into a minimum-mass extrasolar nebula of solids,
//! 620 g cm⁻² (a ÷ 0.2 au)^−1.6, "a factor of 5 larger than the solid surface density of the MMSN"
//! (their §2.2); between 0.05 and 0.5 au it holds 12.7 M⊕ for a typical Kepler planet's host,
//! where the median solar disc here has 0.23 M⊕. The solids must be carried inward, as pebbles
//! drifting through the disc (Lambrechts et al. 2019), so the budget is drawn from the whole disc.
//!
//! Ruling 55 set its efficiency at 1, after Mulders et al. (2021, ApJ 920, 66, abstract), who find
//! the solid masses of the surveys' planetary systems about Sun-like stars matching those of Class
//! II discs "only when the planet formation efficiency is below 100%"; they conclude (§5) that
//! "the reservoir from which exoplanets formed was likely larger", as the embedded discs are
//! "significantly more massive". Tychoniec et al. (2020, A&A 640, A19, abstract) measure Class 0
//! discs at a median of 158 M⊕ of dust and Class I at 52 M⊕ (47 and 12 M⊕ as lower limits), "larger
//! by at least a factor of 10 and 3" than Class II discs, and find the observed planets' solids
//! explained "if planet formation starts in Class 0 phase with an efficiency of ∼15%". Plan 14's
//! median solar disc holds 32.2 M⊕, 3.2 times the ∼10 M⊕ of a Sun-like Class II disc (Mulders et
//! al. 2021, §1), which is a Class I disc; so the budget is the Class 0 reservoir, three times the
//! disc ([`SOLID_BUDGET_EFFICIENCY`]), of which the median Sun-like chain (3.5 planets of 4 M⊕)
//! takes 14%, Tychoniec et al.'s efficiency. At efficiency 1 the budget held M dwarfs to about 1.7
//! small planets inside 200 days, under Dressing and Charbonneau's window, however steep
//! P14.T4.b's compact exponent; Mulders, Pascucci and Apai (2015, ApJ 814, 130, abstract) find the
//! heavy-element mass of close-in planets rising "roughly inversely with stellar mass from 4 M⊕ in
//! F stars to 5 M⊕ in G and K stars to 7 M⊕ in M stars ... in stark contrast with observed
//! protoplanetary disk masses". It truncates 246 of 2,000 Sun-like chains (this module's tests).
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
//! difference in temperature, which they suggest photo-evaporation causes. The paper does not say
//! whether its correlation is of radii or of their logarithms; its figures are logarithmic, and
//! plan 14 reads it as log radii.
//!
//! A drift-fed group's mass no longer carries its disc's scatter, so `σ_b` is the whole of the
//! scatter between systems. With `σ_w` = 0.2 dex, `σ_b` = 0.5 dex and the step s = 0.21 dex were
//! fitted together on P14.T10.b's placed hosts, through P14.T11's Chen and Kipping radius with
//! each planet's own quantile: the adjacent log radii correlate at 0.633 about FGK primaries of
//! drawn \[Fe/H\] and 0.638 about single Suns, and the outer planet is the larger in 0.652 of
//! pairs, all inside Weiss et al.'s windows. Ruling 55.1 proposed the step ("a larger outward step
//! with the scatter re-fitted"); the plan's 0.1 dex gave 0.58. In this module's sample of 2,000
//! chains in solar discs the log radii correlate at 0.635, the log masses at 0.830, and the outer
//! planet is the heavier in 0.703 of pairs and the larger in 0.658. Chen and Kipping's scatter,
//! 0.146 dex in radius above 2.04 M⊕ and independent for each planet (design note 8), is what
//! separates the radius statistics from the mass ones; He, Ford and Ragozzine (2019, MNRAS 490,
//! 4575, §3.7) fit a within-system width of 0.31 ± 0.07 in ln R, 0.135 dex, which that scatter
//! alone more than fills.
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
use crate::planetary::architecture::template::{
    GroupRole, MassLaw, MassRange, PlanetGroup, ROCKY_MASS_FLOOR,
};
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

/// The characteristic mass of a drift-fed group about a host of 1 M☉: 4 M⊕ (plan 14, P14.T7.a).
///
/// Plan 14's figure, 20–40% under the medians of Pascucci et al. (2018) and Mulders et al. (2021),
/// 5–6.5 M⊕ at a solar mass, at the edge of their errors (see the [module documentation](self)).
/// It is the median of a compact chain's, a warm giant's companions' and a substellar chain's
/// characteristic mass about a solar-mass host, and scales in proportion to the host's mass
/// (Pascucci et al. 2018; ruling 60).
pub const COMPACT_CHARACTERISTIC_MASS: EarthMasses = EarthMasses::new(4.0);

/// The characteristic mass of a rocky group in the median solar disc: 0.5 M⊕.
///
/// This module's figure, where plan 14 gives none: the median solar disc makes a group like the
/// Solar System's, whose four terrestrial planets hold 1.98 M⊕, 0.5 M⊕ each on average. Kokubo and
/// Genda (2010, ApJ 714, L21, Table 1) grow 3.6 ± 0.8 planets from 2.3 M⊕ of protoplanets
/// between 0.5 and 1.5 au, the largest 1.18 M⊕ and the second 0.72 M⊕, a mean of 0.64 M⊕. It sets
/// how many rocky planets reach Earth's mass (ruling 48, point f). It is not what carried η⊕'s
/// miss (ruling 60): η⊕ counts 0.5–1.5 R⊕, 0.08–2.9 M⊕, and Kokubo and Genda's mean of 0.64 M⊕
/// raises it by 0.005; the rocky groups' reach was the carrier (P14.T5, [`CountLaw::Fill`]).
///
/// [`CountLaw::Fill`]: crate::planetary::architecture::template::CountLaw::Fill
pub const ROCKY_CHARACTERISTIC_MASS: EarthMasses = EarthMasses::new(0.5);

/// The solid mass of the median solar disc, the disc of a 1 M☉ star of \[Fe/H\] = 0 at plan 06's
/// zero-age state with every variate at its median ([`DiscDraws::MEDIAN`]): 32.20 M⊕.
///
/// A rocky group's median characteristic mass is its [`reference_mass`] in a disc of this much
/// solid mass, and scales in proportion to the solid mass (P14.T7.a). A test holds it to the disc
/// model it stands for.
///
/// [`DiscDraws::MEDIAN`]: crate::planetary::disc::DiscDraws::MEDIAN
pub const REFERENCE_SOLID_MASS: EarthMasses = EarthMasses::new(32.20);

/// The between-system scatter `σ_b` of a drift-fed group's characteristic mass, in dex: 0.5
/// (P14.T7.a–b; ruling 60).
///
/// Plan 14 leaves `σ_b` to be chosen, with `σ_w`, so that adjacent planets' log radii correlate at
/// 0.65 (Weiss et al. 2018). A drift-fed group's mass no longer follows its disc, so this is the
/// whole of the scatter between systems, fitted with [`OUTWARD_STEP_DEX`] on P14.T10.b's placed
/// hosts (see the [module documentation](self)). It is drawn on words 2–3 of the group's first
/// member's block. A rocky group takes none: its disc's own scatter carries it.
pub const BETWEEN_SYSTEM_SCATTER_DEX: f64 = 0.5;

/// The within-system scatter `σ_w` of a member about its group's characteristic mass, in dex: 0.2
/// (P14.T7.b).
///
/// Chosen so that the adjacent log radii of compact systems in solar discs, after P14.T11's Chen
/// and Kipping radius, correlate at Weiss et al.'s (2018) 0.65; see the [module
/// documentation](self) for what it gives and what it cannot.
pub const WITHIN_SYSTEM_SCATTER_DEX: f64 = 0.2;

/// How much heavier each member of a drift-fed group is than the one inside it, in dex: 0.21
/// (P14.T7.b; ruling 60).
///
/// Plan 14 had 0.1, for Weiss et al.'s (2018) outer planets being the larger in most pairs; 0.21
/// is the step that, with [`WITHIN_SYSTEM_SCATTER_DEX`] and Chen and Kipping's scatter, makes the
/// outer planet the larger in their 65.4% of pairs on P14.T10.b's placed hosts (ruling 55.1
/// proposed it: "a larger outward step with the scatter re-fitted"). It is centred on the group's
/// middle member.
pub const OUTWARD_STEP_DEX: f64 = 0.21;

/// How much heavier each member of a rocky group is than the one inside it, in dex: 0 (ruling 60).
///
/// Weiss et al.'s (2018) step is a property of Kepler's compact multis. An in-situ terrestrial
/// group has none: the Solar System's are Mercury 0.055, Venus 0.815, Earth 1 and Mars 0.107 M⊕,
/// and Kokubo and Genda (2010, ApJ 714, L21, §3.2 and Table 1) grow the largest planet near the
/// middle of their protoplanets' region. Since a rocky group reserves [`ROCKY_MAX_COUNT`]
/// members and places as many as reach its snow line, a step centred on the reserved count would
/// also lighten every group that stops early.
///
/// [`ROCKY_MAX_COUNT`]: crate::planetary::architecture::template::ROCKY_MAX_COUNT
pub const ROCKY_OUTWARD_STEP_DEX: f64 = 0.0;

/// How many times the disc's whole solid mass one group's members may hold: 3 (ruling 38, point
/// 4; ruling 60).
///
/// Planet formation starts in the embedded, Class 0 phase, whose discs hold at least ten times the
/// dust of Class II discs and Class I discs at least three times (Tychoniec et al. 2020, A&A 640,
/// A19, abstract). Plan 14's disc, 32.2 M⊕ of solids about the Sun, is 3.2 times the ∼10 M⊕ of a
/// Sun-like Class II disc (Mulders et al. 2021, ApJ 920, 66, §1), so it stands for a Class I disc
/// and the Class 0 reservoir is three times it. See the [module documentation](self).
pub const SOLID_BUDGET_EFFICIENCY: f64 = 3.0;

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

/// The characteristic mass a correlated group of `group`'s role has about a host of 1 M☉ in the
/// median solar disc, one of [`REFERENCE_SOLID_MASS`]: its median characteristic mass there.
///
/// A drift-fed group ([`is_drift_fed`]), a compact chain, a warm giant's companions or a
/// substellar chain, takes [`COMPACT_CHARACTERISTIC_MASS`], and a rocky group
/// [`ROCKY_CHARACTERISTIC_MASS`]. The other roles draw from their laws in every template; should
/// one take the correlated law, its reference is the geometric middle of its range.
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

/// Whether a group of `role` is drift-fed: grown from solids carried inward to the disc's inner
/// edge up to a mass its host sets (a compact chain, a warm giant's companions, a substellar
/// chain), rather than assembled in situ from the local solids (every other role).
///
/// Lambrechts et al. (2019, A&A 627, A83, abstract): where the pebble flux is high, embryos
/// migrate, "concentrate at the inner edge of the disc", and form "closely spaced super-Earths in
/// the 5 to 20 Earth-mass range, bounded by the pebble isolation mass"; where it is low, a few
/// terrestrial planets grow by collisions in place.
#[must_use]
pub const fn is_drift_fed(role: GroupRole) -> bool {
    match role {
        GroupRole::Chain | GroupRole::Companions => true,
        GroupRole::Rocky
        | GroupRole::IceRich
        | GroupRole::Giant
        | GroupRole::IceGiant
        | GroupRole::Survivor => false,
    }
}

/// The characteristic mass of `group` about the host of `disc`, at the group's between-system
/// variate `between` (P14.T7.a, as ruling 60 calibrates it), held to the group's range.
///
/// - A drift-fed group ([`is_drift_fed`]): [`reference_mass`] × (M★ ÷ M☉) × 10^(`σ_b` z), with
///   [`BETWEEN_SYSTEM_SCATTER_DEX`], M★ being the disc's host mass (a pair's total, for a
///   circumbinary disc). Neither the disc's solids nor its metallicity enter.
/// - Any other: [`reference_mass`] × (`M_s` ÷ [`REFERENCE_SOLID_MASS`]), `M_s` being the disc's
///   whole solid mass between its edges, as P14.T7.a wrote it, with no added scatter.
///
/// See the [module documentation](self) for the sources of each.
#[must_use]
pub fn characteristic_mass(
    group: &PlanetGroup,
    disc: &DiscProfile,
    between: StandardNormal,
) -> EarthMasses {
    let drawn = if is_drift_fed(group.role()) {
        let median = reference_mass(group) * disc.host_mass().value();
        median * math::exp10(BETWEEN_SYSTEM_SCATTER_DEX * between.value())
    } else {
        reference_mass(group) * (disc.solid_mass() / REFERENCE_SOLID_MASS)
    };
    held_between(drawn, mass_floor(group, disc), group.masses().max())
}

/// The least mass a member of `group` about the host of `disc` is held to (ruling 66).
///
/// A drift-fed group's floor follows the same law as its masses, the template's floor times
/// M★ ÷ M☉, held no lower than the smaller of the template's floor and a rocky planet's,
/// [`ROCKY_MASS_FLOOR`]: 1 M⊕ about a Sun, 0.3 M⊕ about a 0.3 M☉ host. A floor fixed at 1 M⊕
/// held 39% of the chain planets about hosts of 0.2–0.4 M☉ at exactly 1 M⊕. Any other group's is
/// the template's.
///
/// [`ROCKY_MASS_FLOOR`]: crate::planetary::architecture::template::ROCKY_MASS_FLOOR
#[must_use]
pub fn mass_floor(group: &PlanetGroup, disc: &DiscProfile) -> EarthMasses {
    let floor = group.masses().min();
    if is_drift_fed(group.role()) {
        let lowest = floor.value().min(ROCKY_MASS_FLOOR.value());
        EarthMasses::new((floor.value() * disc.host_mass().value()).max(lowest))
    } else {
        floor
    }
}

/// How many 0.1 dex steps member `position` of `count` lies outside the group's middle:
/// position − (count − 1) ÷ 2.
#[must_use]
fn steps_from_middle(position: usize, count: usize) -> f64 {
    let position = u32::try_from(position).expect("no group's members approach 2³² entries");
    let count = u32::try_from(count).expect("no group's members approach 2³² entries");
    (f64::from(position) * 2.0 + 1.0 - f64::from(count)) / 2.0
}

/// How much heavier each member of `group` is than the one inside it, in dex:
/// [`OUTWARD_STEP_DEX`] for a drift-fed group, [`ROCKY_OUTWARD_STEP_DEX`] for any other.
#[must_use]
pub const fn outward_step(group: &PlanetGroup) -> f64 {
    if is_drift_fed(group.role()) {
        OUTWARD_STEP_DEX
    } else {
        ROCKY_OUTWARD_STEP_DEX
    }
}

/// A member's mass about `characteristic`, at its scatter `scatter` and `steps` outside the
/// middle of a group whose members step outward by `step` dex, held to `floor`–`ceiling`
/// (P14.T7.b; ruling 66).
#[must_use]
fn member_mass(
    characteristic: EarthMasses,
    scatter: StandardNormal,
    steps: f64,
    step: f64,
    (floor, ceiling): (EarthMasses, EarthMasses),
) -> EarthMasses {
    let offset = WITHIN_SYSTEM_SCATTER_DEX * scatter.value() + step * steps;
    held_between(characteristic * math::exp10(offset), floor, ceiling)
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
    held_between(mass, range.min(), range.max())
}

/// `mass` held to `floor`–`ceiling`.
#[must_use]
fn held_between(mass: EarthMasses, floor: EarthMasses, ceiling: EarthMasses) -> EarthMasses {
    EarthMasses::new(mass.value().clamp(floor.value(), ceiling.value()))
}

/// The most that one group's members may hold of `disc`'s solids: [`SOLID_BUDGET_EFFICIENCY`]
/// times its whole solid mass between its edges (ruling 38, point 4; ruling 60).
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
    /// The drawn masses of a group drawn from a law held more than [`solid_budget`], and were
    /// scaled down to it.
    SolidBudget,
    /// The drawn masses of a correlated group held more than [`solid_budget`], so only its
    /// innermost members that fit formed, each at its drawn mass; where even the first did not
    /// fit, none formed (ruling 66).
    Truncated,
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
    /// The masses of the members that formed, in the order of the members given: every member,
    /// unless the group was [`Truncated`](GroupCap::Truncated), when they are its innermost.
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

/// How many of `masses`, from the first, fit inside `limit` together, summed in their order.
#[must_use]
fn formed_within(masses: &[EarthMasses], limit: EarthMasses) -> usize {
    let mut sum = EarthMasses::ZERO;
    let mut formed = 0;
    for &mass in masses {
        sum = sum + mass;
        if sum > limit {
            break;
        }
        formed += 1;
    }
    formed
}

/// The masses of `group`'s members in `disc`, from each member's variates `draws`, inside out
/// (P14.T7): the pure function behind [`group_masses`].
///
/// Member i of the group is `draws[i]`, the group's count is `draws.len()`, and the group's
/// between-system variate is the first member's. A correlated group scatters its members about
/// [`characteristic_mass`]; any other draws each mass from its law at the member's rank. The
/// members are then held together to [`solid_budget`], or for a group that places giants to
/// [`gas_budget`]: a correlated group over it forms its innermost members that fit
/// ([`GroupCap::Truncated`]), any other is scaled down by one factor for every member.
///
/// # Examples
///
/// Every variate at its median shows the law's shape: a chain of five about a Sun centred on its
/// characteristic mass, each planet 0.21 dex heavier than the one inside it; and in a disc with a
/// tenth of the metals, the same chain's three inner planets, all that its budget can build.
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
/// assert_eq!(held.cap(), GroupCap::Truncated);
/// assert_eq!(held.masses(), &m[..3]);
/// assert!(held.total() <= solid_budget(&poor));
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
                let characteristic = characteristic_mass(group, disc, first.group);
                let step = outward_step(group);
                let limits = (mass_floor(group, disc), range.max());
                let masses = draws
                    .iter()
                    .enumerate()
                    .map(|(i, d)| {
                        let steps = steps_from_middle(i, draws.len());
                        member_mass(characteristic, d.scatter, steps, step, limits)
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
    let cap = if unscaled <= limit {
        GroupCap::AsDrawn
    } else if matches!(range.law(), MassLaw::Correlated) {
        masses.truncate(formed_within(&masses, limit));
        GroupCap::Truncated
    } else {
        let scale = limit / unscaled;
        for mass in &mut masses {
            *mass = *mass * scale;
        }
        reached
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
/// A compact chain of four planets about a Sun-like star: similar masses about a characteristic
/// mass drawn about 4 M⊕, and never more than the disc's budget.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::architecture::ArchitectureClass;
/// use hyperion_sim::planetary::architecture::template::template;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{
///     BETWEEN_SYSTEM_SCATTER_DEX, MassDraws, group_masses, solid_budget,
/// };
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
/// // A Sun's chains are 4 M⊕ times the group's own between-system scatter.
/// let z = MassDraws::for_planet(seed, system, members[0]).group.value();
/// let typical = group.characteristic().expect("a correlated group").value();
/// let expected = (4.0 * hyperion_sim::math::exp10(BETWEEN_SYSTEM_SCATTER_DEX * z)).clamp(1.0, 20.0);
/// assert!((typical / expected - 1.0).abs() < 1e-9);
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
        // So its rocky groups are 0.5 M⊕, exactly as far as the constant's four figures allow,
        // and its compact groups, which follow the host, 4 M⊕.
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        for (group, expected) in [(chain(), 4.0), (rocky, 0.5)] {
            let m = characteristic_mass(group, &disc, StandardNormal::ZERO);
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

    /// P14.T7.a, as ruling 60 calibrates it: the median characteristic mass of 10⁴ compact groups
    /// about Suns is 3.6–4.4 M⊕. It does not move when the discs' solids double, and halves with
    /// the host's mass (Pascucci et al. 2018); a rocky group's still doubles with its disc's
    /// solids.
    #[test]
    fn compact_groups_about_suns_have_a_median_of_four_earth_masses() {
        let sun = zams_host(1.0, 0.0);
        // The Sun's zero-age luminosity and radius at every mass and [Fe/H], so that only the
        // mass or the metals change.
        let host = |mass: f64, fe_h: f64| {
            DiscHost::new(
                SolarMasses::new(mass),
                Dex::new(fe_h),
                sun.zams_luminosity(),
                sun.zams_radius(),
            )
            .unwrap()
        };
        let median_of = |mass: f64, fe_h: f64| {
            let host = host(mass, fe_h);
            let mut masses: Vec<f64> = (0..10_000)
                .map(|i| {
                    let disc = drawn_disc(&host, i);
                    let group = MassDraws::for_planet(SEED, system(i), planet(1)).group;
                    characteristic_mass(chain(), &disc, group).value()
                })
                .collect();
            median(&mut masses)
        };
        let solar = median_of(1.0, 0.0);
        assert!((3.6..4.4).contains(&solar), "{solar} M⊕");
        // [Fe/H] = log₁₀ 2 doubles every disc's solids, and the gas does not change.
        let doubled = median_of(1.0, core::f64::consts::LOG10_2);
        assert_same_bits(doubled, solar);
        let half = median_of(0.5, 0.0);
        assert!((half / solar - 0.5).abs() < 1e-12, "{half} against {solar}");
        // A rocky group is proportional to its disc's solid mass.
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        let one = characteristic_mass(
            rocky,
            &median_disc_with(&sun, 0.0, 0.3),
            StandardNormal::ZERO,
        );
        let two = characteristic_mass(
            rocky,
            &median_disc_with(&sun, core::f64::consts::LOG10_2, 0.3),
            StandardNormal::ZERO,
        );
        assert!((two / one - 2.0).abs() < 1e-12, "{one:?} {two:?}");
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
        // Weiss et al.'s 65.4%, to three binomial standard errors of this sample's 4,546 pairs
        // (ruling 60's step); P14.T10.b holds its placed hosts to their 65.0–65.8%.
        assert!((0.633..0.675).contains(&outer_larger), "{outer_larger}");
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

    /// P14.T7.b, with ruling 60's budget: no group exceeds its budget, three times its disc's
    /// solids.
    #[test]
    fn no_group_exceeds_its_budget() {
        let sample = compact_sample();
        let mut truncated = 0;
        for (group, disc) in &sample.groups {
            let budget = solid_budget(disc).value();
            assert!(group.total().value() <= budget * (1.0 + 1e-12));
            assert!((budget / disc.solid_mass().value() - 3.0).abs() < 1e-15);
            if group.cap() == GroupCap::Truncated {
                truncated += 1;
            }
        }
        // An eighth of Sun-like chains are short of their reservoir (246 of 2,000): the heavy
        // chains, and the poorest discs.
        assert!(
            (180..320).contains(&truncated),
            "{truncated} of 2,000 truncated to the budget"
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

    /// Ruling 60: a correlated group over its budget forms its innermost members, each at its
    /// drawn mass, and where even the first does not fit it forms none, so that no member lies under
    /// its floor (ruling 66). A group drawn from a law is still scaled by one factor.
    #[test]
    fn a_group_over_its_budget_forms_its_innermost_members() {
        let poor = |gas: f64| {
            profile(disc::derive(
                &zams_host(1.0, -1.0),
                Megayears::new(2.5),
                &DiscDraws {
                    gas_fraction: StandardNormal::new(gas).unwrap(),
                    ..DiscDraws::MEDIAN
                },
                Truncation::NONE,
            ))
        };
        let draws = [MassDraws::MEDIAN; 6];
        // The median solar disc forms all six; a chain's masses do not depend on its disc.
        let rich = median_disc(&zams_host(1.0, 0.0), 2.0);
        let whole = group_masses_from(chain(), &rich, &draws);
        assert_eq!(whole.cap(), GroupCap::AsDrawn);
        assert_eq!(whole.masses().len(), 6);
        // A metal-poor disc with a thirtieth of the solids forms the inner ones, bit for bit.
        let some = group_masses_from(chain(), &poor(-1.0), &draws);
        assert_eq!(some.cap(), GroupCap::Truncated);
        let formed = some.masses().len();
        assert!((1..6).contains(&formed), "{formed}");
        for (a, b) in some.masses().iter().zip(whole.masses()) {
            assert_same_bits(a.value(), b.value());
        }
        assert!(some.total() <= solid_budget(&poor(-1.0)));
        let next = some.total() + whole.masses()[formed];
        assert!(next > solid_budget(&poor(-1.0)));
        // One still poorer forms none: its budget is under even the innermost member.
        let none = group_masses_from(chain(), &poor(-3.0), &draws);
        assert_eq!(none.cap(), GroupCap::Truncated);
        assert!(none.masses().is_empty());
        assert!(solid_budget(&poor(-3.0)) < whole.masses()[0]);
        // An ice giant group, drawn from its law, is scaled down whole.
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        let scaled = group_masses_from(ice, &poor(-3.0), &draws[..2]);
        assert_eq!(scaled.cap(), GroupCap::SolidBudget);
        assert_eq!(scaled.masses().len(), 2);
        assert!((scaled.total() / solid_budget(&poor(-3.0)) - 1.0).abs() < 1e-12);
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

    /// Ruling 66: a drift-fed group's floor follows its host's mass, never under a rocky planet's,
    /// and a rocky group's is its template's.
    #[test]
    fn a_chain_s_floor_follows_its_host() {
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        let substellar = &template(ArchitectureClass::SubstellarCompact).groups()[0];
        for (mass, chain_floor) in [(1.0, 1.0), (0.3, 0.3), (0.02, 0.05)] {
            let disc = median_disc(&zams_host(mass, 0.0), 2.0);
            assert!((mass_floor(chain(), &disc).value() - chain_floor).abs() < 1e-15);
            assert_same_bits(mass_floor(rocky, &disc).value(), 0.05);
            assert!(mass_floor(substellar, &disc).value() >= 0.01);
            // No member of a far-scattered chain is held at 1 M⊕ about an M dwarf.
            let low = [MassDraws {
                scatter: StandardNormal::new(-8.0).unwrap(),
                ..MassDraws::MEDIAN
            }];
            let held = group_masses_from(chain(), &disc, &low).masses()[0].value();
            assert_same_bits(held, mass_floor(chain(), &disc).value());
        }
    }

    #[test]
    fn members_step_outward_about_the_characteristic_mass() {
        assert_same_bits(steps_from_middle(0, 1), 0.0);
        assert_same_bits(steps_from_middle(0, 4), -1.5);
        assert_same_bits(steps_from_middle(3, 4), 1.5);
        assert_same_bits(steps_from_middle(2, 5), 0.0);
        // With no scatter, each chain member is 0.21 dex heavier than the one inside it, and the
        // middle one is the characteristic mass; a rocky group's members are all alike.
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        let draws = [MassDraws::MEDIAN; 5];
        let group = group_masses_from(chain(), &disc, &draws);
        let m_c = group.characteristic().unwrap().value();
        let masses = group.masses();
        assert!((masses[2].value() / m_c - 1.0).abs() < 1e-15);
        for pair in masses.windows(2) {
            assert!((pair[1] / pair[0] - math::exp10(OUTWARD_STEP_DEX)).abs() < 1e-12);
        }
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        let flat = group_masses_from(rocky, &disc, &draws);
        let m_c = flat.characteristic().unwrap().value();
        assert!(
            flat.masses()
                .iter()
                .all(|m| (m.value() / m_c - 1.0).abs() < 1e-15)
        );
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
