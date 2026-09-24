//! A system's stars as a hierarchy of pairs: the types, and the draw of its companions and their
//! orbits (plan 11, P11.T2.a–b, Design notes 1, 4, 5 and 9).
//!
//! [`draw_hierarchy`] documents the draw: its steps, the numbering of the stars and the keys of
//! their streams (Design note 5), where each companion goes, the laws it is drawn from, and the
//! draw numbers of each attempt (Design note 9).

use std::f64::consts::TAU;

use super::dist::{MIN_COMPANION_MASS, PeriodDistribution};
use super::model::{MAX_COMPANIONS, MultiplicityModel};
use super::stability::{Innermost, Limits, MAX_ECCENTRICITY, NECESSARY_AXIS_RATIO};
use crate::Seed;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{BodyId, SystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::rng::{DomainTag, Mark, ObjectKey, Stream, Threshold, Thresholds, tags};
use crate::stellar::draws::{ATTEMPT_WORDS, StarDraws};
use crate::units::consts::{GM_SUN, METRES_PER_AU};
use crate::units::{Days, GravitationalParameter, Metres, Radians, Seconds, SolarMasses};

/// The number of redraw attempts of a system's binaries: attempts 0 to 7 (plan 11, Design note
/// 9).
///
/// The grid redraws a system whose binary falls into a catalogue class or explodes as a Type Ia
/// (P11.T6–T8), and keeps the last attempt, with its innermost period moved out of the
/// interacting range, after this many.
pub const MAX_REDRAWS: u8 = 8;

/// The words of each stream that one redraw attempt owns: attempt n reads words 64n to 64n + 63.
///
/// Design note 9 sets it. It equals plan 06's block for a star's draws,
/// [`ATTEMPT_WORDS`](crate::stellar::draws::ATTEMPT_WORDS), so that a companion's own
/// [`StarDraws::for_attempt`] and its orbit are redrawn in step. Changing it is a
/// generator-version change.
pub const DRAWS_PER_ATTEMPT: u64 = 64;

const _: () = assert!(
    DRAWS_PER_ATTEMPT == ATTEMPT_WORDS,
    "an attempt's block is plan 06's"
);

/// The end of the stars' body indices: stars take indices 0 to 15, plan 14's slot `0x00`
/// (Design note 5), so that no planet, moon or ring ever shares an index with a star.
pub const STAR_BODY_INDEX_END: u16 = 16;

/// Redraws of a new orbit that fails the stability test or the tidal cut before its companion is
/// dropped (P11.T2.b): tries 0 to 16, on consecutive draw numbers of the attempt's block.
pub const MAX_STABILITY_REDRAWS: u64 = 16;

/// Words of `binary.orbit` and of `binary.orientation` that one try of an orbit reads.
const WORDS_PER_TRY: u64 = 3;

const _: () = assert!(
    (MAX_STABILITY_REDRAWS + 1) * WORDS_PER_TRY <= DRAWS_PER_ATTEMPT,
    "every try of an orbit fits its attempt's block"
);

const _: () = assert!(
    (MAX_COMPANIONS as u64) < STAR_BODY_INDEX_END as u64,
    "every star fits the stellar slot of body indices"
);

/// The relative margin by which a new orbit's period window is widened on each side, so that
/// rounding in the window's arithmetic never excludes a period the full test would admit. The
/// window only bounds a rejection loop, so a wider one costs nothing but a rare extra try.
const WINDOW_MARGIN: f64 = 1e-6;

/// The primary mass from which a star dies by core collapse and plan 06's companion-stripped
/// mark applies: 8 M☉ (plan 11, Design note 1; plan 06's kick law).
pub const STRIPPED_MARK_MIN_MASS: SolarMasses = SolarMasses::new(8.0);

/// The provisional share of primaries of [`STRIPPED_MARK_MIN_MASS`] and up whose envelope a
/// companion strips: 0.25, plan 06's provisional `KickLawParams::stripped_share` (its design note
/// 11), the probability the mark `star.stripped` is read against.
///
/// **Provisional** (P11.T2.c's first seam): P11.T1.d replaces it with
/// [`stripped_share`](super::stripped_share) at the system's composition, with a version bump.
pub const PROVISIONAL_STRIPPED_SHARE: f64 = 0.25;

/// The provisional interacting range: an innermost orbit interacts when its periastron is under
/// 10 au, P11.T1.c's test stand-in promoted and named.
///
/// **Provisional** (P11.T2.c's third seam): P11.T4.a replaces it with `can_interact`'s threshold,
/// the Roche-filling separation of each star's largest radius, with a version bump.
pub const PROVISIONAL_INTERACTING_PERIASTRON: Metres = Metres::new(10.0 * METRES_PER_AU);

/// One attempt of a conditional redraw of a system's binaries, 0 to [`MAX_REDRAWS`] − 1 (plan
/// 11, Design note 9): which block of [`DRAWS_PER_ATTEMPT`] words of each stream it reads.
///
/// Plan 06's [`StarDraws::for_attempt`] takes the same number for each companion's own draws.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::multiplicity::{MAX_REDRAWS, RedrawAttempt};
///
/// let second = RedrawAttempt::FIRST.next().expect("there are eight attempts");
/// assert_eq!(second.get(), 1);
/// assert_eq!(second.first_draw(), 64);
/// assert_eq!(RedrawAttempt::new(MAX_REDRAWS), None);
/// assert_eq!(RedrawAttempt::all().count(), usize::from(MAX_REDRAWS));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedrawAttempt(u8);

impl RedrawAttempt {
    /// Attempt 0, the only one a system that is never redrawn reads.
    pub const FIRST: Self = Self(0);

    /// Attempt `n`, or `None` unless `n` < [`MAX_REDRAWS`].
    #[must_use]
    pub const fn new(n: u8) -> Option<Self> {
        if n < MAX_REDRAWS { Some(Self(n)) } else { None }
    }

    /// The attempt's number, 0 to [`MAX_REDRAWS`] − 1.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The number of the first word the attempt reads on each stream: n × [`DRAWS_PER_ATTEMPT`].
    #[must_use]
    pub fn first_draw(self) -> u64 {
        u64::from(self.0) * DRAWS_PER_ATTEMPT
    }

    /// The attempt after this one, or `None` after the last.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        Self::new(self.0 + 1)
    }

    /// Every attempt, in order.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..MAX_REDRAWS).map(Self)
    }
}

/// What a system's context asks of its multiplicity (plan 11, P11.T2.a–b).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum MultiplicityContext {
    /// The model's own multiplicity: every grid system.
    #[default]
    Free,
    /// A single star whatever the model says: a system whose context rules companions out.
    ForcedSingle,
    /// At least one companion, the count drawn from the model given that there is one.
    ///
    /// With `max_separation`, no orbit's semi-major axis exceeds it: plan 09's cluster members,
    /// truncated at the cluster's hard–soft boundary, which P11.T8.f supplies. No caller passes
    /// this context before then. If every companion is dropped by the stability test the system
    /// comes out single; [`SystemHierarchy::dropped_companions`] says so.
    ForcedMultiple {
        /// The widest semi-major axis any orbit of the system may have, m.
        max_separation: Option<Metres>,
    },
}

/// The index of a star in a [`SystemHierarchy`], which is also its body index: 0 is the primary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StarIndex(u8);

impl StarIndex {
    /// The primary, star 0.
    pub const PRIMARY: Self = Self(0);

    /// The index, 0 to [`STAR_BODY_INDEX_END`] − 1.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The index of a node in a [`SystemHierarchy`]'s depth-first list: 0 is the root.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIndex(u8);

impl NodeIndex {
    /// The root, node 0: the whole system.
    pub const ROOT: Self = Self(0);

    /// The index.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// What kind of object a [`StarSlot`] holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SlotKind {
    /// A star, of at least [`MIN_COMPANION_MASS`](super::MIN_COMPANION_MASS).
    #[default]
    Star,
    /// A brown-dwarf companion, below that mass (P11.T2.d, Design note 15); none before T2.d.
    BrownDwarf,
}

/// One star of a hierarchy: its body, its initial mass and its kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarSlot {
    body: BodyId,
    initial_mass: SolarMasses,
    kind: SlotKind,
}

impl StarSlot {
    /// The star's body ID; its body index is its [`StarIndex`].
    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    /// The initial mass, M☉: the primary's is its record's
    /// [`primary_initial_mass`](SystemRecord::primary_initial_mass), bit for bit.
    #[must_use]
    pub const fn initial_mass(&self) -> SolarMasses {
        self.initial_mass
    }

    /// The kind of object.
    #[must_use]
    pub const fn kind(&self) -> SlotKind {
        self.kind
    }
}

/// A node of a hierarchy: one star, or a pair of nodes on a relative orbit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HierarchyNode {
    /// A star.
    Star(StarIndex),
    /// Two nodes on a relative orbit: the outer member's barycentre about the inner member's.
    Pair {
        /// The inner member, which holds the pair's lower-indexed stars.
        inner: NodeIndex,
        /// The outer member, whose first star is the one the orbit brought in.
        outer: NodeIndex,
        /// The relative orbit, with the pair's gravitational parameter `G (M_inner + M_outer)`.
        orbit: KeplerElements,
    },
}

/// A system's stars and the orbits that hold them together (plan 11, P11.T2.a).
///
/// The nodes are listed depth first from the root, node 0, each pair's inner member before its
/// outer one; the stars are listed by body index, which is the order in which that list meets
/// them. A single star is one node.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::multiplicity::{
///     HierarchyNode, MultiplicityContext, RedrawAttempt, draw_hierarchy,
/// };
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let key = CellKey::new(Layer::C, [0, 812, 0])?;
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, key, &mut cell);
/// let record = cell.first().expect("layer C is not empty at the solar circle");
/// let context = MultiplicityContext::ForcedMultiple { max_separation: None };
/// let stars = draw_hierarchy(&galaxy, record, context, RedrawAttempt::FIRST);
/// // The primary is star 0 and no companion outweighs it.
/// let primary = stars.stars()[0].initial_mass();
/// assert_eq!(primary, record.primary_initial_mass());
/// assert!(stars.stars().iter().all(|s| s.initial_mass() <= primary));
/// if stars.star_count() > 1 {
///     assert!(matches!(stars.node(stars.root()), HierarchyNode::Pair { .. }));
/// }
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SystemHierarchy {
    nodes: Vec<HierarchyNode>,
    stars: Vec<StarSlot>,
    /// The total initial mass of each node, by node index: a star's own, a pair's its inner
    /// member's plus its outer member's.
    node_masses: Vec<SolarMasses>,
    dropped: u8,
}

impl SystemHierarchy {
    /// The root, the whole system: [`NodeIndex::ROOT`].
    #[must_use]
    pub const fn root(&self) -> NodeIndex {
        NodeIndex::ROOT
    }

    /// Every node, depth first from the root.
    #[must_use]
    pub fn nodes(&self) -> &[HierarchyNode] {
        &self.nodes
    }

    /// The node `index`.
    ///
    /// # Panics
    ///
    /// If `index` is not a node of this hierarchy.
    #[must_use]
    pub fn node(&self, index: NodeIndex) -> &HierarchyNode {
        &self.nodes[usize::from(index.0)]
    }

    /// Every star, by body index: the primary first.
    #[must_use]
    pub fn stars(&self) -> &[StarSlot] {
        &self.stars
    }

    /// The star `index`.
    ///
    /// # Panics
    ///
    /// If `index` is not a star of this hierarchy.
    #[must_use]
    pub fn star(&self, index: StarIndex) -> &StarSlot {
        &self.stars[usize::from(index.0)]
    }

    /// The number of stars, 1 to 1 + [`MAX_COMPANIONS`](super::MAX_COMPANIONS).
    ///
    /// # Panics
    ///
    /// Never: a hierarchy holds at most six stars.
    #[must_use]
    pub fn star_count(&self) -> u8 {
        u8::try_from(self.stars.len()).expect("a hierarchy holds at most six stars")
    }

    /// The total initial mass of the stars under `index`, M☉.
    ///
    /// # Panics
    ///
    /// If `index` is not a node of this hierarchy.
    #[must_use]
    pub fn node_mass(&self, index: NodeIndex) -> SolarMasses {
        self.node_masses[usize::from(index.0)]
    }

    /// The total initial mass of the system's stars, M☉: the root's
    /// [`node_mass`](Self::node_mass).
    ///
    /// It is summed down the tree, each pair's inner member and then its outer one, and it is the
    /// mass the tidal cut was tested at. Read it here rather than summing
    /// [`stars`](Self::stars) in body order, which can differ in the last bit.
    #[must_use]
    pub fn system_mass(&self) -> SolarMasses {
        self.node_masses[0]
    }

    /// Every pair with its orbit, depth first.
    pub fn pairs(&self) -> impl Iterator<Item = (NodeIndex, &KeplerElements)> {
        self.nodes
            .iter()
            .zip(0_u8..)
            .filter_map(|(node, i)| match node {
                HierarchyNode::Pair { orbit, .. } => Some((NodeIndex(i), orbit)),
                HierarchyNode::Star(_) => None,
            })
    }

    /// The first star of node `index` in depth-first order: the node's own primary, its
    /// lowest-indexed and heaviest star.
    ///
    /// # Panics
    ///
    /// If `index` is not a node of this hierarchy.
    #[must_use]
    pub fn first_star(&self, index: NodeIndex) -> StarIndex {
        let mut node = index;
        loop {
            match self.node(node) {
                HierarchyNode::Star(star) => return *star,
                HierarchyNode::Pair { inner, .. } => node = *inner,
            }
        }
    }

    /// The body whose ID keys the `binary.*` streams of pair `index`: the lowest-indexed star of
    /// its outer member, the star the orbit brought in (Design note 5); `None` for a star.
    ///
    /// # Panics
    ///
    /// If `index` is not a node of this hierarchy.
    #[must_use]
    pub fn pair_key(&self, index: NodeIndex) -> Option<BodyId> {
        match self.node(index) {
            HierarchyNode::Pair { outer, .. } => Some(self.star(self.first_star(*outer)).body),
            HierarchyNode::Star(_) => None,
        }
    }

    /// How many companions the multiplicity draw asked for that the stability test could not
    /// place (P11.T2.b): 0 for nearly every system.
    #[must_use]
    pub const fn dropped_companions(&self) -> u8 {
        self.dropped
    }

    /// The bytes the hierarchy owns on the heap, beyond `size_of::<SystemHierarchy>()`: its lists
    /// of nodes, stars and node masses, by capacity. For the server's byte-bounded system cache
    /// (plan 06, P06.T34); nothing generated reads it.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.nodes.capacity() * size_of::<HierarchyNode>()
            + self.stars.capacity() * size_of::<StarSlot>()
            + self.node_masses.capacity() * size_of::<SolarMasses>()
    }
}

/// Draws the stars of the system `record` and the orbits that bind them (plan 11, P11.T2.a–b).
///
/// The primary is the record's own star, body 0, with its
/// [`primary_initial_mass`](SystemRecord::primary_initial_mass), placement's `system.primary_mass`
/// draw: plan 06's per-star draws ([`StarDraws`]) hold no mass, and its star model takes the same
/// accessor. Companions, their masses and every orbit are drawn as the sections below describe,
/// on the words of `attempt`. The model is
/// [`MultiplicityModel::default_v1`]; the tidal radius is plan 02's
/// [`tidal_radius`](crate::galaxy::potential::PotentialTables::tidal_radius) at the record's
/// epoch position; the context is `ctx`.
///
/// For a primary of [`STRIPPED_MARK_MIN_MASS`] or more (except under
/// [`ForcedSingle`](MultiplicityContext::ForcedSingle)), plan 06's companion-stripped mark,
/// [`StarDraws::stripped`] of the primary at attempt 0, is read against
/// [`PROVISIONAL_STRIPPED_SHARE`] first (Design note 1). When it is set the system is multiple and
/// the primary's own orbit has its periastron inside [`PROVISIONAL_INTERACTING_PERIASTRON`]; when
/// it is not, the system is multiple with probability `(MF − s) ÷ (1 − s)` and that orbit's
/// periastron lies outside. Both are exact conditional draws, the orbit's by rejection among its
/// tries, and they keep the share of stripped primaries at the mark's. The mark is read at
/// attempt 0 until plan 08's `SystemRecord::mark_attempt` exists (P11.T2.c's second seam).
///
/// P11.T2.c wires it into the system stage:
/// [`SystemStars::generate`](crate::stellar::system::SystemStars::generate) calls
/// `draw_hierarchy(galaxy, record, MultiplicityContext::Free, RedrawAttempt::FIRST)` for grid
/// systems, and [`SystemStars::generate_in`](crate::stellar::system::SystemStars::generate_in)
/// passes any other context.
///
/// A system at the galactic centre itself has a tidal radius of zero, so it has no room for any
/// companion and comes out single.
///
/// # The draw
///
/// [`draw_hierarchy`] makes a system's stars from its record, as a pure function of the seed, the
/// system's ID, the context and the redraw attempt:
///
/// 1. **Multiplicity.** One mark on `system.multiplicity` decides whether the primary has
///    companions, with probability [`MultiplicityModel::multiple_fraction`], and one more picks
///    how many from [`MultiplicityModel::companion_count_pmf`] given that it has.
/// 2. **Companions, one at a time.** Each companion joins one node of the hierarchy's outer spine:
///    the whole system (a new outermost orbit), or the outer member at any level down to its last
///    star (a new orbit inside that member). Each try of its orbit picks the node afresh, together
///    with the period (see "Where a companion goes").
/// 3. **The orbit.** Its node and period, mass ratio and eccentricity come from the model's
///    distributions on `binary.orbit`, its isotropic orientation on `binary.orientation` and its
///    mean anomaly at the epoch on `binary.phase`, all three keyed by the companion's own body ID.
/// 4. **Stability.** The whole hierarchy with the new orbit must pass the whole test: Mardling and
///    Aarseth's criterion for every pair ([`mardling_aarseth_limit`](super::mardling_aarseth_limit)),
///    every apocentre inside half the tidal radius ([`TIDAL_CUT_SHARE`](super::TIDAL_CUT_SHARE)),
///    every semi-major axis inside a forced multiple's widest separation, and the primary's
///    companion-stripped mark. If it does not, the new orbit alone is redrawn, on the next draw
///    numbers of the same attempt, up to [`MAX_STABILITY_REDRAWS`] times; then the companion, and
///    every later one, is dropped. No orbit already placed is ever redrawn.
///
/// # Numbering and keys (Design note 5)
///
/// The primary is star 0. A companion only ever joins the outer spine, and the new star becomes
/// the outer member of the pair it forms, so it comes last in the hierarchy's depth-first order
/// (inner member before outer). The order of drawing is therefore the depth-first order, and
/// companion k is star k, body index k, known when its orbit is drawn. That orbit's pair has the
/// new star as its outer member, and later companions only join the new star or pairs outside
/// it, so the new star stays the lowest-indexed star of the pair's outer member: "the star the
/// orbit brings in", whose [`BodyId`] keys the pair's `binary.*` streams. No two pairs share a
/// key, and no pair is keyed by the primary.
///
/// # Where a companion goes
///
/// Every hierarchy shape can be built this way, in some order of drawing: a triple with its
/// inner pair about the primary, one with a pair as its outer member (Tokovinin 2014, AJ 147, 87,
/// finds these "almost as frequent" among solar-type stars), 3 + 1 and 2 + 2 quadruples, and so
/// on. For each candidate node the necessary conditions of the stability test bound the new
/// orbit's semi-major axis, and with the range of its mass, its period: the criterion's smallest
/// axis ratio, 1.96 (C = 2.8 times the smallest inclination factor, 0.7), against the orbit the
/// new one encloses; against the orbit that encloses it, the criterion with that orbit's own
/// eccentricity, which is known; and the tidal cut for a new outermost orbit. Each node's window
/// has a weight, its period distribution's share inside the window. One mark then picks both the
/// node, by integer thresholds on the weights, and the period, by inverse transform inside that
/// node's window of the mark's residual, and the whole test accepts or rejects the try.
///
/// That is rejection sampling of the pair (node, orbit), so it is exact: a companion joins a node
/// with probability proportional to the chance that an orbit drawn for that node from the model's
/// distributions passes the test, and its orbit is the model's, conditioned on passing. The
/// windows only make it cheap, because every period outside them would fail. The one inexact
/// case is the cap of 17 tries, after which the companion is dropped.
///
/// # The laws a companion is drawn from
///
/// Moe and Di Stefano (2017, ApJS 230, 15, §2 and §5) define a tertiary's period and mass ratio
/// "with respect to the solar-type primary": "We do not define the mass ratio of the wide system
/// to be q = `M_B ÷ (M_Aa + M_Ab)` as done in Raghavan et al. (2010)". Tokovinin (2014, §4.3)
/// draws the periods of outer and inner pairs "recursively from the same log-normal
/// distribution". So a companion's period and mass-ratio laws are the model's at the mass of the
/// first star of the node it joins, the node's own primary (the system's primary for the whole
/// system), and its mass is that star's mass times the ratio. Every star is therefore at most as
/// heavy as the primary, and at least [`MIN_COMPANION_MASS`].
///
/// # Orbits
///
/// A pair's period, eccentricity, orientation and phase are drawn when it forms; its orbit is an
/// [`orbit::KeplerElements`](KeplerElements) built from the period with the pair's own
/// gravitational parameter, `G (M_inner + M_outer)`, of its members' total initial masses. A
/// companion that later joins one of its members changes that μ and so the pair's semi-major
/// axis, at the same period, which is why every test runs on the whole hierarchy as it stands.
///
/// # Draw numbers (Design note 9)
///
/// Attempt n reads words 64n to 64n + 63 of every stream here ([`DRAWS_PER_ATTEMPT`]):
///
/// | Stream                | Key          | Words of attempt n                                    |
/// | --------------------- | ------------ | ----------------------------------------------------- |
/// | `system.multiplicity` | the system   | 64n: multiple or not; 64n + 1: the companion count    |
/// | `binary.orbit`        | companion k  | 64n + 3r: try r's node and period (one mark); + 1, + 2: its mass ratio and eccentricity |
/// | `binary.orientation`  | companion k  | 64n + 3r, + 1, + 2: try r's cos i, ascending node, argument of periapsis |
/// | `binary.phase`        | companion k  | 64n + r: try r's mean anomaly at the epoch            |
///
/// Try r runs from 0 to [`MAX_STABILITY_REDRAWS`], so the orbit streams use 51 of their 64 words.
/// Every word is read by its number, so an attempt drawn alone equals the same attempt drawn after
/// any others, and a redraw never touches another attempt's words. The primary's own draws
/// (plan 06's `StarDraws`) are never redrawn here.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::multiplicity::{MultiplicityContext, RedrawAttempt, draw_hierarchy};
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let record = cell.first().expect("layer C is not empty at the solar circle");
/// let stars = draw_hierarchy(&galaxy, record, MultiplicityContext::Free, RedrawAttempt::FIRST);
/// // A pure function of the ID and the attempt.
/// assert_eq!(stars, draw_hierarchy(&galaxy, record, MultiplicityContext::Free, RedrawAttempt::FIRST));
/// let single = draw_hierarchy(&galaxy, record, MultiplicityContext::ForcedSingle, RedrawAttempt::FIRST);
/// assert_eq!(single.star_count(), 1);
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[must_use]
pub fn draw_hierarchy(
    galaxy: &Galaxy,
    record: &SystemRecord,
    ctx: MultiplicityContext,
    attempt: RedrawAttempt,
) -> SystemHierarchy {
    let model = MultiplicityModel::default_v1();
    let draw = Draw {
        model: &model,
        streams: Streams::new(galaxy, record.id(), attempt),
        limits: Limits::new(
            galaxy.potential(),
            PointLy::from(record.epoch_position()),
            match ctx {
                MultiplicityContext::ForcedMultiple { max_separation } => max_separation,
                MultiplicityContext::Free | MultiplicityContext::ForcedSingle => None,
            },
            innermost(galaxy, record, ctx),
        ),
    };
    let m0 = record.primary_initial_mass();
    let count = draw.companion_count(m0, ctx);
    let mut draft = Draft::single(m0);
    let mut placed = 0;
    for k in 1..=count {
        match draw.place(&draft, k) {
            Some(next) => {
                draft = next;
                placed = k;
            }
            None => break,
        }
    }
    draft.build(record.id(), count - placed)
}

/// What the primary's stripped mark asks of its orbit under `ctx` (Design note 1).
#[must_use]
fn innermost(galaxy: &Galaxy, record: &SystemRecord, ctx: MultiplicityContext) -> Innermost {
    match ctx {
        MultiplicityContext::ForcedSingle => return Innermost::Free,
        MultiplicityContext::Free | MultiplicityContext::ForcedMultiple { .. } => {}
    }
    if record.primary_initial_mass() < STRIPPED_MARK_MIN_MASS {
        return Innermost::Free;
    }
    let primary = BodyId::new(record.id(), 0);
    let mark = StarDraws::for_star(galaxy.seed(), primary).stripped();
    if mark.is_below(Threshold::from_probability(PROVISIONAL_STRIPPED_SHARE)) {
        Innermost::Interacting(PROVISIONAL_INTERACTING_PERIASTRON)
    } else {
        Innermost::Wide(PROVISIONAL_INTERACTING_PERIASTRON)
    }
}

/// The streams of one system's attempt: where each draw is read.
#[derive(Debug, Clone, Copy)]
struct Streams {
    seed: Seed,
    system: SystemId,
    base: u64,
}

impl Streams {
    #[must_use]
    fn new(galaxy: &Galaxy, system: SystemId, attempt: RedrawAttempt) -> Self {
        Self {
            seed: galaxy.seed(),
            system,
            base: attempt.first_draw(),
        }
    }

    /// Word `offset` of the attempt's block of the system's stream `tag`, as a mark.
    #[must_use]
    fn system_mark(&self, tag: DomainTag, offset: u64) -> Mark {
        let stream = Stream::open(self.seed, tag, ObjectKey::from(self.system));
        Mark::from_word(stream.word_at(self.base + offset))
    }

    /// Companion `body`'s stream `tag`.
    #[must_use]
    fn body_stream(&self, tag: DomainTag, body: u8) -> Stream {
        let key = ObjectKey::from(BodyId::new(self.system, u16::from(body)));
        Stream::open(self.seed, tag, key)
    }
}

/// One system's draw: the model, the streams and the limits.
struct Draw<'a> {
    model: &'a MultiplicityModel,
    streams: Streams,
    limits: Limits<'a>,
}

/// A node the next companion may join, with its period window and the window's weight.
#[derive(Debug, Clone)]
struct Host {
    /// The node, in the draft's arena.
    node: usize,
    /// The initial mass of the node's first star, at which the companion's laws are taken.
    first_mass: SolarMasses,
    periods: PeriodDistribution,
    /// The window's limits.
    lo: Days,
    hi: Days,
    /// The period distribution's share inside the window.
    weight: f64,
}

impl Draw<'_> {
    /// How many companions the primary of initial mass `m0` has under `ctx`, before stability:
    /// 0 to [`MAX_COMPANIONS`].
    #[must_use]
    fn companion_count(&self, m0: SolarMasses, ctx: MultiplicityContext) -> u8 {
        let pmf = self.model.companion_count_pmf(m0);
        let multiples = &pmf[1..];
        let multiple_share = multiples.iter().fold(0.0, |sum, &p| sum + p);
        let multiple = match (ctx, self.limits.innermost()) {
            (MultiplicityContext::ForcedSingle, _) => false,
            (MultiplicityContext::ForcedMultiple { .. }, _)
            | (MultiplicityContext::Free, Innermost::Interacting(_)) => true,
            (MultiplicityContext::Free, Innermost::Free) => self
                .streams
                .system_mark(tags::SYSTEM_MULTIPLICITY, 0)
                .is_below(Threshold::from_probability(multiple_share.clamp(0.0, 1.0))),
            (MultiplicityContext::Free, Innermost::Wide(_)) => {
                let s = PROVISIONAL_STRIPPED_SHARE;
                let p = ((multiple_share - s) / (1.0 - s)).clamp(0.0, 1.0);
                self.streams
                    .system_mark(tags::SYSTEM_MULTIPLICITY, 0)
                    .is_below(Threshold::from_probability(p))
            }
        };
        if !multiple {
            return 0;
        }
        let extra = self
            .streams
            .system_mark(tags::SYSTEM_MULTIPLICITY, 1)
            .pick_weighted(multiples, multiple_share)
            .expect("the last count's threshold is the whole share, which every mark lies below");
        u8::try_from(1 + extra).expect("at most five companions")
    }

    /// The draft with companion `k` placed, or `None` if it cannot be.
    #[must_use]
    fn place(&self, draft: &Draft, k: u8) -> Option<Draft> {
        let hosts = self.hosts(draft);
        let weights: Vec<f64> = hosts.iter().map(|h| h.weight).collect();
        let total = weights.iter().fold(0.0, |sum, &w| sum + w);
        if total <= 0.0 {
            return None;
        }
        let windows = Thresholds::from_weights(&weights, total);
        let mut orbits = self.streams.body_stream(tags::BINARY_ORBIT, k);
        let mut orientations = self.streams.body_stream(tags::BINARY_ORIENTATION, k);
        let mut phases = self.streams.body_stream(tags::BINARY_PHASE, k);
        let base = self.streams.base;
        (0..=MAX_STABILITY_REDRAWS).find_map(|r| {
            orbits.seek(base + WORDS_PER_TRY * r);
            orientations.seek(base + WORDS_PER_TRY * r);
            phases.seek(base + r);
            let (index, u) = pick_window(&windows, orbits.mark());
            let host = &hosts[index];
            let (mass, orbit) =
                self.try_orbit(host, u, &mut orbits, &mut orientations, &mut phases)?;
            let next = draft.joined(host.node, mass, orbit);
            self.limits
                .admits(&next.build(self.streams.system, 0))
                .then_some(next)
        })
    }

    /// One try of a companion's initial mass and orbit about `host`, its period the share `u` of
    /// the way through the host's window, or `None` for an eccentricity an open orbit would have
    /// to carry.
    #[must_use]
    fn try_orbit(
        &self,
        host: &Host,
        u: f64,
        orbits: &mut Stream,
        orientations: &mut Stream,
        phases: &mut Stream,
    ) -> Option<(SolarMasses, DraftOrbit)> {
        let period = host.periods.quantile_in(u, host.lo, host.hi);
        let q = self
            .model
            .mass_ratio_distribution(host.first_mass, period)
            .sample(orbits);
        let e = self.model.eccentricity_distribution(period).sample(orbits);
        let cos_i = 1.0 - 2.0 * orientations.uniform();
        let node = TAU * orientations.uniform();
        let argument = TAU * orientations.uniform();
        let mean_anomaly = TAU * phases.uniform();
        if e >= MAX_ECCENTRICITY {
            return None;
        }
        let orbit = DraftOrbit {
            period: Seconds::from(period),
            eccentricity: Eccentricity::new(e)
                .expect("an eccentricity drawn in [0, e_max) is bound"),
            orientation: Orientation::new(
                Radians::new(math::acos(cos_i)),
                Radians::new(node),
                Radians::new(argument),
            )
            .expect("an inclination from acos lies in [0, π]"),
            mean_anomaly: Radians::new(mean_anomaly),
        };
        Some((host.first_mass * q, orbit))
    }

    /// Every node of the draft's outer spine, from the root to its last star, with the window
    /// the necessary conditions leave for a new orbit about it ([`draw_hierarchy`], "Where a
    /// companion goes").
    #[must_use]
    fn hosts(&self, draft: &Draft) -> Vec<Host> {
        let spine = draft.spine();
        let system_mass = draft.mass(draft.root);
        spine
            .iter()
            .enumerate()
            .map(|(level, &node)| {
                let first_mass = draft.masses[usize::from(draft.first_star(node))];
                let node_mass = draft.mass(node);
                let lightest = SolarMasses::new(MIN_COMPANION_MASS.value().min(first_mass.value()));
                // The new orbit encloses this node's own orbit, if it has one.
                let a_lo = match draft.nodes[node] {
                    DraftNode::Pair { orbit, .. } => {
                        semi_major_axis(orbit.period, node_mass) * NECESSARY_AXIS_RATIO
                    }
                    DraftNode::Star(_) => Metres::ZERO,
                };
                // A new outermost orbit must fit the tidal cut; any other must fit inside the
                // orbit of the pair whose outer member it joins, with the new mass added.
                let a_hi = match level.checked_sub(1).map(|up| spine[up]) {
                    None => self.limits.widest_axis(system_mass + first_mass),
                    Some(parent) => widest_inside(draft, parent, node_mass + first_mass),
                };
                let p_lo = period_of(a_lo, node_mass + first_mass) * (1.0 - WINDOW_MARGIN);
                let p_hi = period_of(a_hi, node_mass + lightest) * (1.0 + WINDOW_MARGIN);
                let periods = self.model.period_distribution(first_mass);
                let shortest = math::exp10(periods.support().0);
                let lo = Days::new(Days::from(p_lo).value().max(shortest));
                let hi = Days::from(p_hi);
                let weight = if hi > lo {
                    periods.share_in(lo, hi)
                } else {
                    0.0
                };
                Host {
                    node,
                    first_mass,
                    periods,
                    lo,
                    hi,
                    weight,
                }
            })
            .collect()
    }
}

/// The widest semi-major axis that a new pair can have as the outer member of the pair `parent`,
/// with a total initial mass of at most `heaviest`, by the necessary conditions of Mardling and
/// Aarseth's test of `parent`'s orbit.
///
/// That orbit's eccentricity is known, its semi-major axis grows with the new mass at its fixed
/// period, the inclination factor is at least 0.7 and the mass ratio of the other member to the
/// new pair is at least the other's mass over `heaviest`. If the other member is itself a pair,
/// the new pair may be as wide as it, for then the other member is the inner binary of the test.
#[must_use]
fn widest_inside(draft: &Draft, parent: usize, heaviest: SolarMasses) -> Metres {
    let DraftNode::Pair { inner, orbit, .. } = draft.nodes[parent] else {
        unreachable!("a spine node's parent is a pair");
    };
    let other_mass = draft.mass(inner);
    let axis = semi_major_axis(orbit.period, other_mass + heaviest);
    let e = orbit.eccentricity.value();
    let bracket = (1.0 + other_mass / heaviest) * (1.0 + e) / (1.0 - e).sqrt();
    let inside = axis * (1.0 - e) / (NECESSARY_AXIS_RATIO * math::powf(bracket, 0.4));
    match draft.nodes[inner] {
        DraftNode::Pair { orbit: other, .. } => {
            let other_axis = semi_major_axis(other.period, other_mass);
            if other_axis > inside {
                other_axis
            } else {
                inside
            }
        }
        DraftNode::Star(_) => inside,
    }
}

/// The semi-major axis of an orbit of `period` about a total mass of `mass`: the form
/// [`KeplerElements::from_period`] uses, ∛(μ (P ÷ 2π)²).
#[must_use]
fn semi_major_axis(period: Seconds, mass: SolarMasses) -> Metres {
    let per_radian = period.value() / TAU;
    Metres::new(math::cbrt(GM_SUN * mass.value() * per_radian * per_radian))
}

/// The period of an orbit of semi-major axis `a` about a total mass of `mass`: 2π a √(a ÷ μ).
#[must_use]
fn period_of(a: Metres, mass: SolarMasses) -> Seconds {
    let a = a.value();
    Seconds::new(TAU * (a * (a / (GM_SUN * mass.value())).sqrt()))
}

/// The window of `windows` that `mark` falls in, and where in it, as a share strictly inside
/// (0, 1).
///
/// This is the integer-threshold pick of a mixture's component, whose residual is a uniform on
/// the component's own range. The window is the first whose threshold lies above the mark, so it
/// is picked with its weight's share of the total. The residual is `(offset + ½) ÷ width`, the
/// offset of the mark above the window's lower threshold and the window's width counted in
/// marks. A window wider than 2⁵² marks is counted in pairs of marks, as
/// [`Stream::uniform_open`] counts a word's top 52 bits, so that `offset + ½` stays exact and the
/// share stays below 1.
#[must_use]
fn pick_window(windows: &Thresholds, mark: Mark) -> (usize, f64) {
    let index = mark
        .pick(windows)
        .expect("the last threshold is 2⁵³, above every mark");
    let upper = windows.as_slice()[index].get();
    let lower = index
        .checked_sub(1)
        .map_or(0, |below| windows.as_slice()[below].get());
    let (mut offset, mut width) = (mark.get() - lower, upper - lower);
    if width > 1 << 52 {
        offset >>= 1;
        width = width.div_ceil(2);
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "both are integers of at most 2⁵², all of which an f64 represents exactly"
    )]
    let (offset, width) = (offset as f64, width as f64);
    (index, (offset + 0.5) / width)
}

/// An orbit as drawn, before its semi-major axis is fixed by its members' masses.
#[derive(Debug, Clone, Copy)]
struct DraftOrbit {
    period: Seconds,
    eccentricity: Eccentricity,
    orientation: Orientation,
    mean_anomaly: Radians,
}

/// A node of a draft, in an arena.
#[derive(Debug, Clone, Copy)]
enum DraftNode {
    /// A star, by body index.
    Star(u8),
    /// A pair of arena nodes.
    Pair {
        inner: usize,
        outer: usize,
        orbit: DraftOrbit,
    },
}

/// A hierarchy while it is drawn: star initial masses by body index and nodes in an arena, the
/// root wherever the last companion put it.
#[derive(Debug, Clone)]
struct Draft {
    masses: Vec<SolarMasses>,
    nodes: Vec<DraftNode>,
    root: usize,
}

impl Draft {
    /// The primary alone, of initial mass `m0`.
    #[must_use]
    fn single(m0: SolarMasses) -> Self {
        Self {
            masses: vec![m0],
            nodes: vec![DraftNode::Star(0)],
            root: 0,
        }
    }

    /// The outer spine: the root, its outer member, that member's outer member and so on, down to
    /// a star.
    #[must_use]
    fn spine(&self) -> Vec<usize> {
        let mut spine = Vec::with_capacity(MAX_COMPANIONS + 1);
        let mut node = self.root;
        loop {
            spine.push(node);
            match self.nodes[node] {
                DraftNode::Star(_) => return spine,
                DraftNode::Pair { outer, .. } => node = outer,
            }
        }
    }

    /// The body index of `node`'s first star in depth-first order.
    #[must_use]
    fn first_star(&self, node: usize) -> u8 {
        let mut node = node;
        loop {
            match self.nodes[node] {
                DraftNode::Star(body) => return body,
                DraftNode::Pair { inner, .. } => node = inner,
            }
        }
    }

    /// The total initial mass of `node`'s stars, summed as the finished hierarchy sums it: inner
    /// member, then outer.
    #[must_use]
    fn mass(&self, node: usize) -> SolarMasses {
        match self.nodes[node] {
            DraftNode::Star(body) => self.masses[usize::from(body)],
            DraftNode::Pair { inner, outer, .. } => self.mass(inner) + self.mass(outer),
        }
    }

    /// This draft with a new star of initial mass `mass` as the outer member of a new pair whose
    /// inner member is `host`, on `orbit`. `host` must be on the outer spine.
    #[must_use]
    fn joined(&self, host: usize, mass: SolarMasses, orbit: DraftOrbit) -> Self {
        let mut next = self.clone();
        let body = u8::try_from(next.masses.len()).expect("at most six stars");
        next.masses.push(mass);
        let star = next.nodes.len();
        next.nodes.push(DraftNode::Star(body));
        let pair = next.nodes.len();
        next.nodes.push(DraftNode::Pair {
            inner: host,
            outer: star,
            orbit,
        });
        if host == self.root {
            next.root = pair;
        } else {
            let parent = self
                .nodes
                .iter()
                .position(|n| matches!(n, DraftNode::Pair { outer, .. } if *outer == host))
                .expect("a spine node other than the root is its parent's outer member");
            if let DraftNode::Pair { outer, .. } = &mut next.nodes[parent] {
                *outer = pair;
            }
        }
        next
    }

    /// The finished hierarchy of system `system`, with `dropped` companions left out: nodes
    /// depth first from the root, each pair's orbit built from its period with its members'
    /// total mass.
    #[must_use]
    fn build(&self, system: SystemId, dropped: u8) -> SystemHierarchy {
        let mut hierarchy = SystemHierarchy {
            nodes: Vec::with_capacity(self.nodes.len()),
            stars: (0_u16..)
                .zip(&self.masses)
                .map(|(index, &initial_mass)| StarSlot {
                    body: BodyId::new(system, index),
                    initial_mass,
                    kind: SlotKind::Star,
                })
                .collect(),
            node_masses: Vec::with_capacity(self.nodes.len()),
            dropped,
        };
        self.emit(self.root, &mut hierarchy);
        hierarchy
    }

    /// Appends `node` and its subtree to `out` depth first; its index and total initial mass.
    fn emit(&self, node: usize, out: &mut SystemHierarchy) -> (NodeIndex, SolarMasses) {
        let index = NodeIndex(u8::try_from(out.nodes.len()).expect("at most eleven nodes"));
        match self.nodes[node] {
            DraftNode::Star(body) => {
                let mass = self.masses[usize::from(body)];
                out.nodes.push(HierarchyNode::Star(StarIndex(body)));
                out.node_masses.push(mass);
                (index, mass)
            }
            DraftNode::Pair {
                inner,
                outer,
                orbit,
            } => {
                out.nodes.push(HierarchyNode::Star(StarIndex::PRIMARY));
                out.node_masses.push(SolarMasses::ZERO);
                let (inner, inner_mass) = self.emit(inner, out);
                let (outer, outer_mass) = self.emit(outer, out);
                let mass = inner_mass + outer_mass;
                let orbit = KeplerElements::from_period(
                    orbit.period,
                    GravitationalParameter::from_solar_masses(mass),
                    orbit.eccentricity,
                    orbit.orientation,
                    orbit.mean_anomaly,
                )
                .expect("a period of 0.1 d to 10¹¹ d about a stellar mass gives a finite orbit");
                let slot = usize::from(index.0);
                out.nodes[slot] = HierarchyNode::Pair {
                    inner,
                    outer,
                    orbit,
                };
                out.node_masses[slot] = mass;
                (index, mass)
            }
        }
    }
}

// Plan 14's synthetic hosts (P14.T1.d): hierarchies of one and two stars built from their parts,
// for `planetary::context`'s builder.
impl SystemHierarchy {
    /// One star of initial mass `mass`, M☉, and kind `kind`, body 0 of `system`: the hierarchy a
    /// draw gives a single star.
    ///
    /// A single star passes every test of the draw, so the hierarchy is one a draw could give.
    #[must_use]
    pub(crate) fn single(system: SystemId, mass: SolarMasses, kind: SlotKind) -> Self {
        Self {
            nodes: vec![HierarchyNode::Star(StarIndex::PRIMARY)],
            stars: vec![StarSlot {
                body: BodyId::new(system, 0),
                initial_mass: mass,
                kind,
            }],
            node_masses: vec![mass],
            dropped: 0,
        }
    }

    /// Two stars of `system` on a relative orbit: the primary, star 0, of initial mass `m0` M☉, and
    /// the companion, star 1, of `m1` M☉ and kind `kind`, with semi-major axis `a` and
    /// eccentricity `e` about their total mass, in the reference plane and at periapsis at the
    /// epoch, as the tests' `hand_built` makes its pairs.
    ///
    /// The nodes are laid out as the draw lays out a binary: the pair, then the primary, then the
    /// companion. A pair with no third body passes Mardling and Aarseth's criterion whatever its
    /// orbit, so the draw's other tests are left to the caller, which holds them: the companion
    /// no heavier than the primary, the period inside the draw's range, the eccentricity inside
    /// its envelope at that period, and the apocentre inside
    /// [`TIDAL_CUT_SHARE`](super::TIDAL_CUT_SHARE) of the system's tidal radius. A brown-dwarf
    /// companion is one the draw makes only from P11.T2.d.
    ///
    /// # Errors
    ///
    /// What [`KeplerElements::from_semi_major_axis`] refuses: a semi-major axis that is not
    /// positive and finite, masses whose parameter is not, or a period that overflows or
    /// underflows.
    pub(crate) fn binary(
        system: SystemId,
        m0: SolarMasses,
        (m1, kind): (SolarMasses, SlotKind),
        a: Metres,
        e: Eccentricity,
    ) -> Result<Self, crate::orbit::BuildOrbitError> {
        let mass = m0 + m1;
        let flat = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)
            .expect("zero angles are an orientation");
        let orbit = KeplerElements::from_semi_major_axis(
            a,
            GravitationalParameter::from_solar_masses(mass),
            e,
            flat,
            Radians::ZERO,
        )?;
        Ok(Self {
            nodes: vec![
                HierarchyNode::Pair {
                    inner: NodeIndex(1),
                    outer: NodeIndex(2),
                    orbit,
                },
                HierarchyNode::Star(StarIndex::PRIMARY),
                HierarchyNode::Star(StarIndex(1)),
            ],
            stars: vec![
                StarSlot {
                    body: BodyId::new(system, 0),
                    initial_mass: m0,
                    kind: SlotKind::Star,
                },
                StarSlot {
                    body: BodyId::new(system, 1),
                    initial_mass: m1,
                    kind,
                },
            ],
            node_masses: vec![mass, m0, m1],
            dropped: 0,
        })
    }
}

/// Hierarchies built by hand, for the tests of what reads a hierarchy (plan 14's stable zones):
/// named masses and orbits, and brown-dwarf slots, which no draw makes before P11.T2.d.
///
/// Nothing here checks stability or the tidal cut, so a hand-built hierarchy need not be one the
/// draw could give. That is why it is test-only: [`SystemHierarchy`] is stable by construction
/// everywhere else.
#[cfg(test)]
pub(crate) mod hand_built {
    use super::{
        BodyId, HierarchyNode, NodeIndex, STAR_BODY_INDEX_END, SlotKind, SolarMasses, StarIndex,
        StarSlot, SystemHierarchy, SystemId,
    };
    use crate::orbit::{Eccentricity, KeplerElements, Orientation};
    use crate::units::{GravitationalParameter, Metres, Radians};

    /// A node of a hand-built hierarchy.
    #[derive(Debug, Clone)]
    pub(crate) enum Node {
        /// A star, or a brown dwarf, of initial mass `mass`.
        Star { mass: SolarMasses, kind: SlotKind },
        /// Two nodes on a relative orbit of semi-major axis `a` and eccentricity `e` about their
        /// total mass, in the reference plane, at periapsis at the epoch.
        Pair {
            inner: Box<Node>,
            outer: Box<Node>,
            a: Metres,
            e: Eccentricity,
        },
    }

    /// The hierarchy of `system` under `root`, its stars numbered depth first, inner member
    /// before outer, as the draw numbers them; each orbit is built from its semi-major axis, so
    /// that [`KeplerElements::semi_major_axis`] returns `a` bit for bit.
    ///
    /// # Panics
    ///
    /// For an orbit [`KeplerElements::from_semi_major_axis`] rejects, or more than 16 stars.
    pub(crate) fn build(system: SystemId, root: &Node) -> SystemHierarchy {
        let mut h = SystemHierarchy {
            nodes: Vec::new(),
            stars: Vec::new(),
            node_masses: Vec::new(),
            dropped: 0,
        };
        emit(system, root, &mut h);
        h
    }

    /// Appends `node` and its subtree depth first, as `Draft::emit` does; its total mass.
    fn emit(system: SystemId, node: &Node, out: &mut SystemHierarchy) -> SolarMasses {
        let slot = out.nodes.len();
        match node {
            Node::Star { mass, kind } => {
                assert!(
                    out.stars.len() < usize::from(STAR_BODY_INDEX_END),
                    "a hierarchy holds at most 16 stars"
                );
                let index = u8::try_from(out.stars.len()).expect("fewer than 16 stars");
                out.stars.push(StarSlot {
                    body: BodyId::new(system, u16::from(index)),
                    initial_mass: *mass,
                    kind: *kind,
                });
                out.nodes.push(HierarchyNode::Star(StarIndex(index)));
                out.node_masses.push(*mass);
                *mass
            }
            Node::Pair { inner, outer, a, e } => {
                out.nodes.push(HierarchyNode::Star(StarIndex::PRIMARY));
                out.node_masses.push(SolarMasses::ZERO);
                let inner_index = NodeIndex(u8::try_from(out.nodes.len()).expect("few nodes"));
                let inner_mass = emit(system, inner, out);
                let outer_index = NodeIndex(u8::try_from(out.nodes.len()).expect("few nodes"));
                let outer_mass = emit(system, outer, out);
                let mass = inner_mass + outer_mass;
                let flat = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)
                    .expect("zero angles are an orientation");
                let orbit = KeplerElements::from_semi_major_axis(
                    *a,
                    GravitationalParameter::from_solar_masses(mass),
                    *e,
                    flat,
                    Radians::ZERO,
                )
                .expect("a hand-built orbit is valid");
                out.nodes[slot] = HierarchyNode::Pair {
                    inner: inner_index,
                    outer: outer_index,
                    orbit,
                };
                out.node_masses[slot] = mass;
                mass
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::f64::consts::PI;

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::super::testing::{
        SAMPLE, galaxy, imf_records, inner_disc, log_uniform_records, records_of_mass, sunlike,
    };
    use super::*;
    use crate::galaxy::imf::MassBand;

    fn free(galaxy: &Galaxy, record: &SystemRecord) -> SystemHierarchy {
        draw_hierarchy(
            galaxy,
            record,
            MultiplicityContext::Free,
            RedrawAttempt::FIRST,
        )
    }

    /// Mardling and Aarseth's (2001) eq. 90 and their two factors, written out again from the
    /// paper with the inclination in degrees, independently of the module's own code: whether the
    /// pair `pair` of `h` is stable, to a relative margin of 10⁻¹².
    fn satisfies_mardling_aarseth(h: &SystemHierarchy, pair: NodeIndex) -> bool {
        let HierarchyNode::Pair {
            inner,
            outer,
            orbit,
        } = *h.node(pair)
        else {
            unreachable!("a pair");
        };
        let orbit_of = |n: NodeIndex| match *h.node(n) {
            HierarchyNode::Pair { orbit, .. } => Some(orbit),
            HierarchyNode::Star(_) => None,
        };
        // (inner binary, its orbit, the third body, f₁)
        let (binary, binary_orbit, third, f1) = match (orbit_of(inner), orbit_of(outer)) {
            (None, None) => return true,
            (Some(o), None) => (inner, o, outer, 1.0),
            (None, Some(o)) => (outer, o, inner, 1.0),
            (Some(a), Some(b)) => {
                let (wide, tight) = (a.semi_major_axis().value(), b.semi_major_axis().value());
                let f1 = 1.0 + 0.1 * (wide / tight).min(tight / wide);
                if wide >= tight {
                    (inner, a, outer, f1)
                } else {
                    (outer, b, inner, f1)
                }
            }
        };
        let q_out = h.node_mass(third).value() / h.node_mass(binary).value();
        let e_out = orbit.eccentricity().value();
        let [ax, ay, az] = binary_orbit.orientation().normal();
        let [bx, by, bz] = orbit.orientation().normal();
        let degrees = math::acos((ax * bx + ay * by + az * bz).clamp(-1.0, 1.0)) * 180.0 / PI;
        let f = 1.0 - 0.3 * degrees / 180.0;
        let bracket = (1.0 + q_out) * (1.0 + e_out) / math::powf(1.0 - e_out, 0.5);
        let critical =
            f1 * f * 2.8 * math::powf(bracket, 0.4) * binary_orbit.semi_major_axis().value();
        let periastron = orbit.semi_major_axis().value() * (1.0 - e_out);
        periastron > critical * (1.0 - 1e-12)
    }

    /// The pairs of `h` with the node indices of their members.
    fn pairs(h: &SystemHierarchy) -> Vec<(NodeIndex, NodeIndex, NodeIndex, KeplerElements)> {
        h.nodes()
            .iter()
            .zip(0_u8..)
            .filter_map(|(node, i)| match *node {
                HierarchyNode::Pair {
                    inner,
                    outer,
                    orbit,
                } => Some((NodeIndex(i), inner, outer, orbit)),
                HierarchyNode::Star(_) => None,
            })
            .collect()
    }

    /// The stars under `node` in depth-first order, inner members first.
    fn depth_first(h: &SystemHierarchy, node: NodeIndex, out: &mut Vec<u8>) {
        match *h.node(node) {
            HierarchyNode::Star(star) => out.push(star.get()),
            HierarchyNode::Pair { inner, outer, .. } => {
                depth_first(h, inner, out);
                depth_first(h, outer, out);
            }
        }
    }

    #[test]
    fn the_same_system_draws_the_same_hierarchy_twice() {
        let galaxy = galaxy();
        for record in imf_records(&galaxy, 500, &sunlike(), 1) {
            assert_eq!(free(&galaxy, &record), free(&galaxy, &record));
        }
    }

    /// P11.T2.a: attempt n drawn alone equals attempt n drawn after attempts 0 to n − 1, and the
    /// attempts differ from one another.
    #[test]
    fn an_attempt_drawn_alone_equals_it_drawn_after_the_earlier_ones() {
        let galaxy = galaxy();
        let records = imf_records(&galaxy, 400, &sunlike(), 2);
        let draw = |record: &SystemRecord, attempt| {
            draw_hierarchy(&galaxy, record, MultiplicityContext::Free, attempt)
        };
        let mut differ = 0;
        for record in &records {
            let in_turn: Vec<SystemHierarchy> =
                RedrawAttempt::all().map(|a| draw(record, a)).collect();
            for attempt in RedrawAttempt::all().collect::<Vec<_>>().into_iter().rev() {
                assert_eq!(draw(record, attempt), in_turn[usize::from(attempt.get())]);
            }
            if in_turn[1..].iter().any(|h| *h != in_turn[0]) {
                differ += 1;
            }
        }
        // About half of these primaries are multiple, and a multiple's attempts all differ.
        assert!(
            differ > 100,
            "only {differ} of 400 systems changed between attempts"
        );
    }

    /// P11.T2: the draw is order independent, through the testkit's helper, over the mass
    /// function and massive primaries (whose stripped mark is a second stream read), every
    /// context and two attempts.
    #[test]
    fn a_hierarchy_does_not_depend_on_what_was_drawn_before() {
        let galaxy = galaxy();
        let mut records = imf_records(&galaxy, 150, &sunlike(), 3);
        records.extend(log_uniform_records(
            &galaxy,
            150,
            &sunlike(),
            (8.0, 120.0),
            15,
        ));
        let contexts = [
            MultiplicityContext::Free,
            MultiplicityContext::ForcedSingle,
            MultiplicityContext::ForcedMultiple {
                max_separation: None,
            },
            MultiplicityContext::ForcedMultiple {
                max_separation: Some(Metres::from(crate::units::AstronomicalUnits::new(300.0))),
            },
        ];
        let second = RedrawAttempt::FIRST.next().expect("eight attempts");
        let keys: Vec<(usize, MultiplicityContext, RedrawAttempt)> = (0..records.len())
            .flat_map(|i| {
                contexts
                    .iter()
                    .flat_map(move |&c| [(i, c, RedrawAttempt::FIRST), (i, c, second)])
            })
            .collect();
        assert_order_independent(&keys, |&(i, context, attempt)| {
            draw_hierarchy(&galaxy, &records[i], context, attempt)
        });
    }

    /// The node and period pick: the window is the one `Mark::pick` gives, and the share inside it
    /// stays strictly inside (0, 1) at both ends of every window, the widest included.
    #[test]
    fn a_window_pick_agrees_with_the_thresholds_and_stays_inside_the_window() {
        let cases: [&[f64]; 4] = [&[1.0], &[0.3, 0.0, 0.7], &[0.999_999, 1e-6], &[0.2; 5]];
        for weights in cases {
            let total = weights.iter().fold(0.0, |sum, &w| sum + w);
            let windows = Thresholds::from_weights(weights, total);
            let edges: Vec<u64> = windows.as_slice().iter().map(|t| t.get()).collect();
            let mut marks = vec![0, 1, (1 << 53) - 2, (1 << 53) - 1];
            for &edge in &edges {
                marks.extend([edge.saturating_sub(1), edge.min((1 << 53) - 1)]);
            }
            for word in marks {
                let mark = Mark::from_word(word << 11);
                let (index, u) = pick_window(&windows, mark);
                assert_eq!(Some(index), mark.pick(&windows));
                assert!(
                    u > 0.0 && u < 1.0,
                    "a share of {u} for mark {word} in {weights:?}"
                );
                assert!(weights[index] > 0.0, "an empty window picked");
            }
        }
    }

    /// A system at the galactic centre has a tidal radius of zero, so every companion it asks for
    /// is dropped and it comes out single.
    #[test]
    fn a_system_at_the_galactic_centre_comes_out_single() {
        let galaxy = galaxy();
        let centre = crate::coords::GalacticPosition::from_light_years([0.0; 3])
            .expect("the centre is inside the root cube");
        let context = MultiplicityContext::ForcedMultiple {
            max_separation: None,
        };
        for record in log_uniform_records(&galaxy, 200, &centre, (0.08, 150.0), 16) {
            let h = draw_hierarchy(&galaxy, &record, context, RedrawAttempt::FIRST);
            assert_eq!(h.star_count(), 1);
            assert!(h.dropped_companions() >= 1);
        }
    }

    /// P11.T2.a (Design note 5): every pair of 10⁴ hierarchies has a key of its own, the lowest
    /// star of its outer member, and no pair is keyed by the primary.
    #[test]
    fn every_pair_of_ten_thousand_hierarchies_has_a_key_of_its_own() {
        let galaxy = galaxy();
        let mut keys = BTreeSet::new();
        let mut count = 0;
        for record in imf_records(&galaxy, SAMPLE, &sunlike(), 4) {
            let h = free(&galaxy, &record);
            for (pair, _, outer, _) in pairs(&h) {
                let key = h.pair_key(pair).expect("a pair has a key");
                let mut under = Vec::new();
                depth_first(&h, outer, &mut under);
                assert_eq!(key.body_index(), u16::from(under[0]));
                assert_ne!(key.body_index(), 0, "a pair keyed by the primary");
                assert_eq!(key.system(), record.id());
                keys.insert(key);
                count += 1;
            }
        }
        assert!(count > 4_000, "only {count} pairs");
        assert_eq!(keys.len(), count, "two pairs share a key");
    }

    /// Design note 5: the primary is star 0 and the stars are numbered depth first, inner member
    /// before outer, each with its body ID; nodes are listed depth first from the root.
    #[test]
    fn stars_are_numbered_depth_first_from_the_primary() {
        let galaxy = galaxy();
        for record in imf_records(&galaxy, 2_000, &sunlike(), 5) {
            let h = free(&galaxy, &record);
            let mut order = Vec::new();
            depth_first(&h, h.root(), &mut order);
            let expected: Vec<u8> = (0..h.star_count()).collect();
            assert_eq!(order, expected);
            for (slot, index) in h.stars().iter().zip(0_u16..) {
                assert_eq!(slot.body(), BodyId::new(record.id(), index));
                assert_eq!(slot.kind(), SlotKind::Star);
            }
            assert_eq!(h.nodes().len(), 2 * h.stars().len() - 1);
            for (pair, inner, outer, _) in pairs(&h) {
                assert_eq!(
                    inner.get(),
                    pair.get() + 1,
                    "pre-order puts the inner member next"
                );
                assert!(outer > inner);
            }
            assert!(u16::from(h.star_count()) <= STAR_BODY_INDEX_END);
        }
    }

    /// P11.T2.a: every orbit is built with its own pair's gravitational parameter, the members'
    /// total initial mass, and keeps its drawn period.
    #[test]
    fn every_orbit_carries_its_pairs_gravitational_parameter() {
        let galaxy = galaxy();
        for record in imf_records(&galaxy, 2_000, &sunlike(), 6) {
            let h = free(&galaxy, &record);
            for (pair, inner, outer, orbit) in pairs(&h) {
                let mass = h.node_mass(inner).value() + h.node_mass(outer).value();
                assert_same_bits(h.node_mass(pair).value(), mass);
                let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(mass));
                assert_same_bits(orbit.gravitational_parameter().value(), mu.value());
                let days = Days::from(orbit.period()).value();
                assert!((0.099..1.01e11).contains(&days), "a period of {days} d");
            }
        }
    }

    /// P11.T2.b's properties over 10⁴ hierarchies: every pair satisfies Mardling and Aarseth's
    /// criterion, every apocentre lies inside half the tidal radius, no star outweighs the primary
    /// or falls below the stellar floor, and the primary is its record's.
    #[test]
    fn ten_thousand_hierarchies_are_stable_inside_the_cut_and_under_the_primary() {
        let galaxy = galaxy();
        for (at, salt) in [(sunlike(), 7), (inner_disc(), 8)] {
            let point = PointLy::from(&at);
            let (mut triples, mut higher) = (0, 0);
            for record in imf_records(&galaxy, SAMPLE, &at, salt) {
                let h = free(&galaxy, &record);
                let m0 = record.primary_initial_mass();
                assert_same_bits(h.stars()[0].initial_mass().value(), m0.value());
                assert_eq!(h.stars()[0].body(), BodyId::new(record.id(), 0));
                for star in h.stars() {
                    let m = star.initial_mass();
                    assert!(m <= m0, "a star of {m:?} outweighs its primary of {m0:?}");
                    assert!(m.value() >= MIN_COMPANION_MASS.value() * (1.0 - 1e-15));
                }
                let total = h
                    .stars()
                    .iter()
                    .fold(0.0, |sum, s| sum + s.initial_mass().value());
                let tidal = galaxy
                    .potential()
                    .tidal_radius(SolarMasses::new(total), &point);
                for (pair, _, _, orbit) in pairs(&h) {
                    assert!(
                        orbit.apoapsis().value() <= 0.5 * tidal.value() * (1.0 + 1e-12),
                        "an apocentre of {:?} outside half of {tidal:?}",
                        orbit.apoapsis()
                    );
                    assert!(
                        satisfies_mardling_aarseth(&h, pair),
                        "an unstable pair in {h:#?}"
                    );
                    assert!(orbit.eccentricity().value() < MAX_ECCENTRICITY);
                }
                match h.star_count() {
                    3 => triples += 1,
                    4.. => higher += 1,
                    _ => {}
                }
            }
            println!("at {at:?}: {triples} triples and {higher} higher multiples in {SAMPLE}");
            assert!(triples > 100 && higher > 10);
        }
    }

    /// P11.T2.b: fewer than 1% of companions are dropped after 16 redraws, over the galaxy's mass
    /// function at the Sun-like point and in the inner disc, and over every mass range equally.
    #[test]
    fn fewer_than_one_companion_in_a_hundred_is_dropped() {
        let galaxy = galaxy();
        let samples = [
            (
                "mass function, Sun-like point",
                imf_records(&galaxy, SAMPLE, &sunlike(), 9),
            ),
            (
                "mass function, inner disc",
                imf_records(&galaxy, SAMPLE, &inner_disc(), 10),
            ),
            (
                "log-uniform 0.08–150 M☉, Sun-like point",
                log_uniform_records(&galaxy, SAMPLE, &sunlike(), (0.08, 150.0), 11),
            ),
        ];
        for (what, records) in samples {
            let mut kept = [0_u32; 5];
            let mut dropped = [0_u32; 5];
            for record in &records {
                let h = free(&galaxy, record);
                let band = MassBand::ALL
                    .iter()
                    .position(|b| record.primary_initial_mass().value() <= b.hi())
                    .expect("inside the stellar range");
                kept[band] += u32::from(h.star_count() - 1);
                dropped[band] += u32::from(h.dropped_companions());
            }
            let (all_kept, all_dropped) = (kept.iter().sum::<u32>(), dropped.iter().sum::<u32>());
            let share = f64::from(all_dropped) / f64::from(all_kept + all_dropped);
            println!(
                "{what}: {all_dropped} of {} companions dropped ({:.3}%); by band A–E kept \
                 {kept:?}, dropped {dropped:?}",
                all_kept + all_dropped,
                100.0 * share
            );
            assert!(share < 0.01, "{what}: {:.3}% dropped", 100.0 * share);
        }
    }

    /// The decisions follow the model: at 1 M☉ the multiple share is 44% and the companions
    /// asked for average 0.62 per system, each within 3.29 standard errors (α = 10⁻³).
    #[test]
    fn multiplicity_follows_the_model() {
        let galaxy = galaxy();
        let model = MultiplicityModel::default_v1();
        for mass in [0.3, 1.0, 3.0] {
            let m1 = SolarMasses::new(mass);
            let records = records_of_mass(&galaxy, SAMPLE, &sunlike(), mass);
            let (mut multiples, mut asked, mut asked_sq) = (0_u32, 0.0, 0.0);
            for record in &records {
                let h = free(&galaxy, record);
                let n = f64::from(h.star_count() - 1 + h.dropped_companions());
                if n > 0.0 {
                    multiples += 1;
                }
                asked += n;
                asked_sq += n * n;
            }
            let n = f64::from(SAMPLE);
            let share = f64::from(multiples) / n;
            let mf = model.multiple_fraction(m1);
            let sigma = (mf * (1.0 - mf) / n).sqrt();
            let mean = asked / n;
            let cf = model.companion_frequency(m1);
            let sigma_mean = ((asked_sq / n - mean * mean) / n).sqrt();
            println!(
                "{mass} M☉: multiple {share:.4} (MF {mf:.4}), companions {mean:.4} (CF {cf:.4})"
            );
            assert!((share - mf).abs() < 3.29 * sigma, "{share} against {mf}");
            assert!((mean - cf).abs() < 3.29 * sigma_mean, "{mean} against {cf}");
        }
    }

    #[test]
    fn forced_single_gives_one_star() {
        let galaxy = galaxy();
        for record in imf_records(&galaxy, 2_000, &sunlike(), 12) {
            let h = draw_hierarchy(
                &galaxy,
                &record,
                MultiplicityContext::ForcedSingle,
                RedrawAttempt::FIRST,
            );
            assert_eq!(h.star_count(), 1);
            assert_eq!(h.nodes(), &[HierarchyNode::Star(StarIndex::PRIMARY)]);
            assert_same_bits(
                h.system_mass().value(),
                record.primary_initial_mass().value(),
            );
            assert_eq!(h.dropped_companions(), 0);
        }
    }

    /// P11.T2.b's _Slice_: `ForcedMultiple` with an explicit widest separation gives at least one
    /// companion to every system that keeps one, and no orbit wider than the separation.
    #[test]
    fn forced_multiple_keeps_every_orbit_inside_its_separation() {
        let galaxy = galaxy();
        let widest = Metres::from(crate::units::AstronomicalUnits::new(1_000.0));
        let context = MultiplicityContext::ForcedMultiple {
            max_separation: Some(widest),
        };
        let (mut singles, mut dropped, mut kept) = (0, 0_u32, 0_u32);
        for record in imf_records(&galaxy, 5_000, &sunlike(), 13) {
            let h = draw_hierarchy(&galaxy, &record, context, RedrawAttempt::FIRST);
            if h.star_count() == 1 {
                singles += 1;
                assert!(
                    h.dropped_companions() > 0,
                    "a forced multiple drew no companion"
                );
            }
            for (_, _, _, orbit) in pairs(&h) {
                assert!(orbit.semi_major_axis() <= widest);
            }
            kept += u32::from(h.star_count() - 1);
            dropped += u32::from(h.dropped_companions());
            let unbounded = draw_hierarchy(
                &galaxy,
                &record,
                MultiplicityContext::ForcedMultiple {
                    max_separation: None,
                },
                RedrawAttempt::FIRST,
            );
            assert!(unbounded.star_count() + unbounded.dropped_companions() > 1);
        }
        println!(
            "forced multiples inside 1,000 au: {singles} of 5000 came out single; {dropped} of \
             {} companions dropped",
            kept + dropped
        );
        assert!(f64::from(dropped) < 0.05 * f64::from(kept + dropped));
    }

    /// Design note 1 on the provisional seams: a massive primary's stripped mark decides its own
    /// orbit, set for a quarter of them.
    #[test]
    fn a_massive_primarys_stripped_mark_decides_its_orbit() {
        let galaxy = galaxy();
        let records = log_uniform_records(&galaxy, 4_000, &sunlike(), (8.0, 120.0), 14);
        let (mut stripped, mut lost) = (0_u32, 0_u32);
        for record in &records {
            let h = free(&galaxy, record);
            let mark = StarDraws::for_star(galaxy.seed(), BodyId::new(record.id(), 0)).stripped();
            let orbit = super::super::stability::primary_orbit(&h);
            if mark.is_below(Threshold::from_probability(PROVISIONAL_STRIPPED_SHARE)) {
                stripped += 1;
                if let Some(o) = orbit {
                    assert!(o.periapsis() < PROVISIONAL_INTERACTING_PERIASTRON);
                } else {
                    assert!(h.dropped_companions() > 0);
                    lost += 1;
                }
            } else if let Some(o) = orbit {
                assert!(o.periapsis() >= PROVISIONAL_INTERACTING_PERIASTRON);
            }
        }
        let share = f64::from(stripped) / 4_000.0;
        let sigma = (0.25 * 0.75 / 4_000.0_f64).sqrt();
        println!(
            "{stripped} of 4000 massive primaries stripped ({share:.4}); {lost} lost their companion"
        );
        assert!((share - PROVISIONAL_STRIPPED_SHARE).abs() < 3.29 * sigma);
        assert!(lost < 4, "{lost} stripped primaries have no companion");
    }

    /// The shapes the draw makes for Sun-like primaries, printed for the record: which member of
    /// a triple holds the inner pair, and how many quadruples are 2 + 2.
    #[test]
    fn sun_like_hierarchies_take_every_shape() {
        let galaxy = galaxy();
        let (mut primary_side, mut secondary_side, mut two_two, mut three_one) = (0, 0, 0, 0);
        for record in records_of_mass(&galaxy, 3 * SAMPLE, &sunlike(), 1.0) {
            let h = free(&galaxy, &record);
            let HierarchyNode::Pair { inner, outer, .. } = *h.node(h.root()) else {
                continue;
            };
            let is_pair = |n| matches!(h.node(n), HierarchyNode::Pair { .. });
            match (h.star_count(), is_pair(inner), is_pair(outer)) {
                (3, true, false) => primary_side += 1,
                (3, false, true) => secondary_side += 1,
                (4, true, true) => two_two += 1,
                (4, _, _) => three_one += 1,
                _ => {}
            }
        }
        println!(
            "Sun-like triples: inner pair about the primary {primary_side}, in the outer member \
             {secondary_side}; quadruples 2 + 2 {two_two}, 3 + 1 {three_one}"
        );
        assert!(primary_side > 0 && secondary_side > 0 && two_two > 0 && three_one > 0);
    }
}
