//! A binary's history as a timeline of segments (plan 11, design note 6): what the engine
//! returns, and the state at any age read from it.
//!
//! A [`BinaryTimeline`] is an ordered list of [`Segment`]s from zero age to the age the binary was
//! run to. Each segment holds the kind of phase ([`SegmentKind`]), its boundary ages, how each
//! member's state is evaluated inside it (`star.rs`), the orbit's semi-major axis and eccentricity
//! at the engine's steps, and, during stable transfer, the rate. [`BinaryTimeline::state_at`] is a
//! lookup and closed forms: the segment holding the age, each member's closed forms at its mass
//! and age there, the orbit's elements joined linearly between steps. So the state is continuous
//! in age inside a segment, is the same whatever was asked before, and needs no replay. The
//! timeline is derived data, which the caller may cache with the system.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::coords::SystemVelocity;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::{CompactRemnant, NatalKick};
use crate::stellar::sse::{self, ZCoeffs};
use crate::stellar::{Composition, StarState, substellar};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{
    GravitationalParameter, Metres, Radians, SolarMasses, SolarMassesPerYear, Years,
};

use super::params::BinaryParams;
use super::star::{Member, Path};

/// Which member of a pair: the one the input names first (the inner member's star, body index
/// order) or the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Component {
    /// The first star of [`BinaryInput`], the primary of a system's innermost pair.
    Primary,
    /// The second star.
    Secondary,
}

impl Component {
    /// Both components, primary first.
    pub const BOTH: [Self; 2] = [Self::Primary, Self::Secondary];

    /// 0 for the primary, 1 for the secondary.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
        }
    }

    /// The component of index `i` (0 or 1; any other value is the secondary).
    #[must_use]
    pub const fn of_index(i: usize) -> Self {
        if i == 0 {
            Self::Primary
        } else {
            Self::Secondary
        }
    }

    /// The other component.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Primary => Self::Secondary,
            Self::Secondary => Self::Primary,
        }
    }
}

/// What a segment of a binary's history is (plan 11's Provides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SegmentKind {
    /// Two stars on an orbit, each inside its Roche lobe: winds, tides, magnetic braking and
    /// gravitational radiation change the orbit (Hurley, Tout and Pols 2002, sections 2.1–2.4).
    Detached,
    /// Stable Roche-lobe overflow from `donor`, at the nuclear or thermal rate (their sections
    /// 2.6.2 and 2.6.3).
    StableTransfer {
        /// The star filling its Roche lobe.
        donor: Component,
    },
    /// A common envelope (their section 2.7.1), which is over on a dynamical timescale: a segment
    /// of no length that marks the event.
    CommonEnvelope,
    /// One star, or nothing: the pair has coalesced (their sections 2.6.4, 2.6.5, 2.7.2 and
    /// 2.7.3), and the other member is gone.
    Merged,
    /// Two single stars no longer bound, after `by`'s supernova (their section 2.5).
    Disrupted {
        /// The star whose supernova unbound the pair.
        by: Component,
    },
    /// Both stars fill their Roche lobes (their section 2.6.6), before the pair coalesces.
    Contact,
}

/// One segment of a timeline: its kind, its boundary ages, and what its members and orbit are
/// inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    kind: SegmentKind,
    start: f64,
    end: f64,
    members: [Member; 2],
    orbit: Option<OrbitPath>,
    /// The mean rate of transfer over each step, M☉ yr⁻¹, during stable transfer.
    rates: Option<Path>,
}

impl Segment {
    /// The kind of phase.
    #[must_use]
    pub const fn kind(&self) -> SegmentKind {
        self.kind
    }

    /// The age at which the segment starts, Julian years since the stars' onset of collapse.
    #[must_use]
    pub const fn start(&self) -> Years {
        Years::new(self.start)
    }

    /// The age at which it ends.
    #[must_use]
    pub const fn end(&self) -> Years {
        Years::new(self.end)
    }

    /// A segment, for the engine.
    #[must_use]
    pub(crate) fn new(
        kind: SegmentKind,
        start: f64,
        end: f64,
        members: [Member; 2],
        orbit: Option<OrbitPath>,
        rates: Option<Path>,
    ) -> Self {
        Self {
            kind,
            start,
            end,
            members,
            orbit,
            rates,
        }
    }

    /// The members, for the crate's tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn members(&self) -> &[Member; 2] {
        &self.members
    }

    /// The bytes the segment owns on the heap.
    #[must_use]
    fn heap_bytes(&self) -> usize {
        let path = |p: &Path| std::mem::size_of_val(p.knots());
        self.members.iter().map(Member::heap_bytes).sum::<usize>()
            + self
                .orbit
                .as_ref()
                .map_or(0, |o| path(&o.axis) + path(&o.eccentricity))
            + self.rates.as_ref().map_or(0, path)
    }
}

/// An orbit through a segment: its orientation and mean anomaly at the epoch, and its
/// semi-major axis (R☉) and eccentricity at the engine's steps.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OrbitPath {
    pub(crate) orientation: Orientation,
    pub(crate) mean_anomaly: Radians,
    pub(crate) axis: Path,
    pub(crate) eccentricity: Path,
    /// The drawn orbit itself, for a pair that never interacts: two single stars on their orbit
    /// (plan 11, design note 7), whose elements do not change.
    pub(crate) fixed: Option<KeplerElements>,
}

/// The largest eccentricity a bound orbit is reported with: plan 14 carries bound orbits from
/// 0.9999 up as open ones (ruling 39 of 2026-09-22), which a secular binary never needs.
const MAX_REPORTED_ECCENTRICITY: f64 = 0.9998;

impl OrbitPath {
    /// The elements at `age` for a pair of total mass `total` M☉, or `None` if they cannot be
    /// built (a pair of no mass).
    #[must_use]
    fn elements_at(&self, age: f64, total: f64) -> Option<KeplerElements> {
        if let Some(fixed) = self.fixed {
            return Some(fixed);
        }
        let a = self.axis.at(age) * SOLAR_RADIUS_M;
        let e = self
            .eccentricity
            .at(age)
            .clamp(0.0, MAX_REPORTED_ECCENTRICITY);
        KeplerElements::from_semi_major_axis(
            Metres::new(a),
            GravitationalParameter::from_solar_masses(SolarMasses::new(total)),
            Eccentricity::new(e).ok()?,
            self.orientation,
            self.mean_anomaly,
        )
        .ok()
    }
}

/// The inputs of the binary engine: the two stars, their composition and draws, and their orbit
/// at zero age (plan 11's Provides).
///
/// Both stars are born together with the system's composition. The orbit is the relative orbit
/// of the second star about the first, as plan 11's hierarchy draws it; its orientation and mean
/// anomaly at the epoch place the stars at a supernova (their appendix A1). `age_at_epoch` is the
/// system's age at the epoch, which maps the engine's ages onto the universe clock there.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryInput {
    masses: [SolarMasses; 2],
    composition: Composition,
    orbit: KeplerElements,
    draws: [StarDraws; 2],
    age_at_epoch: Years,
    params: BinaryParams,
}

/// Why [`BinaryInput::new`] refused its inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildBinaryInputError {
    /// A star's initial mass is outside 0.01–150 M☉ or not a number.
    MassOutsideRange(SolarMasses),
    /// The age at the epoch is not finite.
    AgeNotFinite(Years),
}

impl fmt::Display for BuildBinaryInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassOutsideRange(m) => {
                write!(
                    f,
                    "a binary's star is 0.01-150 solar masses, not {}",
                    m.value()
                )
            }
            Self::AgeNotFinite(age) => {
                write!(f, "an age at the epoch is finite, not {}", age.value())
            }
        }
    }
}

impl Error for BuildBinaryInputError {}

impl BinaryInput {
    /// A pair of initial masses `m1` and `m2` of composition `comp` on `orbit` at zero age, with
    /// the stars' draws `draws` and the system's age at the epoch `age_at_epoch`, under the
    /// generator's [`BinaryParams`].
    ///
    /// # Errors
    ///
    /// [`BuildBinaryInputError::MassOutsideRange`] for a mass outside 0.01–150 M☉, and
    /// [`BuildBinaryInputError::AgeNotFinite`] for an age that is not finite.
    pub fn new(
        m1: SolarMasses,
        m2: SolarMasses,
        comp: Composition,
        orbit: KeplerElements,
        draws: [StarDraws; 2],
        age_at_epoch: Years,
    ) -> Result<Self, BuildBinaryInputError> {
        for m in [m1, m2] {
            if !(substellar::MIN_MASS.value()..=crate::stellar::system::MAX_STAR_MASS.value())
                .contains(&m.value())
            {
                return Err(BuildBinaryInputError::MassOutsideRange(m));
            }
        }
        if !age_at_epoch.value().is_finite() {
            return Err(BuildBinaryInputError::AgeNotFinite(age_at_epoch));
        }
        Ok(Self {
            masses: [m1, m2],
            composition: comp,
            orbit,
            draws,
            age_at_epoch,
            params: BinaryParams::default(),
        })
    }

    /// The same input under `params`.
    #[must_use]
    pub const fn with_params(self, params: BinaryParams) -> Self {
        Self { params, ..self }
    }

    /// The initial masses, M☉, primary first.
    #[must_use]
    pub const fn masses(&self) -> [SolarMasses; 2] {
        self.masses
    }

    /// The composition.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The orbit at zero age.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// The stars' draws, primary first.
    #[must_use]
    pub const fn draws(&self) -> &[StarDraws; 2] {
        &self.draws
    }

    /// The system's age at the epoch.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// The engine's parameters.
    #[must_use]
    pub const fn params(&self) -> &BinaryParams {
        &self.params
    }
}

/// What every segment of one timeline shares: the pair's composition, coefficients, draws and
/// parameters.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Context {
    composition: Composition,
    coeffs: ZCoeffs,
    draws: [StarDraws; 2],
    params: BinaryParams,
    age_at_epoch: Years,
    /// The lightest helium main-sequence star that burns helium, M☉.
    lightest_helium_star: f64,
    /// x of HPT equation 47, for BSE equation 57.
    giant_exponent: f64,
}

impl Context {
    /// The context of `input`.
    #[must_use]
    pub(crate) fn of(input: &BinaryInput) -> Self {
        let coeffs = ZCoeffs::new(input.composition.z_fit());
        Self {
            composition: input.composition,
            lightest_helium_star: sse::lightest_helium_star(&coeffs),
            giant_exponent: sse::giant_radius_exponent(&input.composition),
            coeffs,
            draws: input.draws.clone(),
            params: input.params,
            age_at_epoch: input.age_at_epoch,
        }
    }

    #[must_use]
    pub(crate) const fn composition(&self) -> &Composition {
        &self.composition
    }

    #[must_use]
    pub(crate) const fn coeffs(&self) -> &ZCoeffs {
        &self.coeffs
    }

    #[must_use]
    pub(crate) const fn draws(&self, slot: usize) -> &StarDraws {
        &self.draws[if slot == 0 { 0 } else { 1 }]
    }

    #[must_use]
    pub(crate) const fn params(&self) -> &BinaryParams {
        &self.params
    }

    #[must_use]
    pub(crate) const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    #[must_use]
    pub(crate) const fn lightest_helium_star(&self) -> f64 {
        self.lightest_helium_star
    }

    #[must_use]
    pub(crate) const fn giant_exponent(&self) -> f64 {
        self.giant_exponent
    }
}

/// Which channel a pooled Type Ia candidate came by (plan 11, P11.T6's `IaPoolChannel`):
/// sub-Chandrasekhar cases included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IaPoolChannel {
    /// Two white dwarfs merge.
    Merger,
    /// An accreting white dwarf reaches ignition: the Chandrasekhar mass, 0.15 M☉ of accreted
    /// helium on a carbon–oxygen dwarf (an edge-lit detonation), or 0.7 M☉ for a helium dwarf
    /// (Hurley, Tout and Pols 2002, section 2.6.6).
    Accretion,
}

/// The first event of a binary that could have been a Type Ia supernova: plan 11's pool, whose
/// explosion P11.T6's mark decides. The engine continues from it as if it did not explode (an
/// unexploded pooled event "stays what the engine made it").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PooledIaEvent {
    age: Years,
    channel: IaPoolChannel,
    masses: [SolarMasses; 2],
}

impl PooledIaEvent {
    /// A pooled event at `age` by `channel`, of the two stars' masses then.
    #[must_use]
    pub(crate) const fn new(age: Years, channel: IaPoolChannel, masses: [SolarMasses; 2]) -> Self {
        Self {
            age,
            channel,
            masses,
        }
    }

    /// When, Julian years since the stars' onset of collapse.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// The channel.
    #[must_use]
    pub const fn channel(&self) -> IaPoolChannel {
        self.channel
    }

    /// The two white dwarfs' masses for a merger; the accreting dwarf's and its donor's for
    /// accretion. M☉.
    #[must_use]
    pub const fn masses(&self) -> [SolarMasses; 2] {
        self.masses
    }
}

/// A supernova (or accretion-induced collapse) in a binary: who, when, what it left, its kick,
/// and what it did to the pair (Hurley, Tout and Pols 2002, section 2.5 and appendix A1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupernovaRecord {
    age: Years,
    component: Component,
    remnant: CompactRemnant,
    kick: Option<NatalKick>,
    bound: bool,
    velocities: [SystemVelocity; 2],
}

impl SupernovaRecord {
    /// A record, for the engine.
    #[must_use]
    pub(crate) const fn new(
        age: Years,
        component: Component,
        remnant: CompactRemnant,
        kick: Option<NatalKick>,
        bound: bool,
        velocities: [SystemVelocity; 2],
    ) -> Self {
        Self {
            age,
            component,
            remnant,
            kick,
            bound,
            velocities,
        }
    }

    /// When, Julian years since the stars' onset of collapse.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// Which star exploded.
    #[must_use]
    pub const fn component(&self) -> Component {
        self.component
    }

    /// What it left.
    #[must_use]
    pub const fn remnant(&self) -> CompactRemnant {
        self.remnant
    }

    /// Its natal kick, if it had one.
    #[must_use]
    pub const fn kick(&self) -> Option<NatalKick> {
        self.kick
    }

    /// Whether the pair stayed bound.
    #[must_use]
    pub const fn bound(&self) -> bool {
        self.bound
    }

    /// Each member's velocity after the explosion in the frame of the pair's barycentre before it,
    /// primary first, along the system frame's axes: their barycentre's, the pair's recoil, for a
    /// pair that stays bound (their equation A14), and each star's own for one unbound.
    #[must_use]
    pub const fn velocities(&self) -> [SystemVelocity; 2] {
        self.velocities
    }

    /// The pair's recoil, the velocity of its barycentre after the explosion in the frame of the
    /// barycentre before it (their equation A14), for a pair that stays bound.
    #[must_use]
    pub fn recoil(&self) -> Option<SystemVelocity> {
        self.bound.then_some(self.velocities[0])
    }
}

/// A binary's history (plan 11's Provides): its segments from zero age, the first pooled Type Ia
/// candidate, and its supernovae.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryTimeline {
    segments: Vec<Segment>,
    pooled_ia: Option<PooledIaEvent>,
    supernovae: Vec<SupernovaRecord>,
    until: f64,
    context: Arc<Context>,
    capped: bool,
}

impl BinaryTimeline {
    /// A timeline, for the engine.
    #[must_use]
    pub(crate) fn new(
        segments: Vec<Segment>,
        pooled_ia: Option<PooledIaEvent>,
        supernovae: Vec<SupernovaRecord>,
        until: f64,
        context: Arc<Context>,
        capped: bool,
    ) -> Self {
        debug_assert!(!segments.is_empty(), "a timeline has a segment");
        Self {
            segments,
            pooled_ia,
            supernovae,
            until,
            context,
            capped,
        }
    }

    /// The pair's state at `age`, Julian years since the stars' onset of collapse, from zero to the
    /// age the binary was run to: continuous inside a segment (plan 11, design note 6).
    ///
    /// # Panics
    ///
    /// In debug builds, if `age` is not finite or lies outside the timeline; release builds clamp
    /// it into range.
    #[must_use]
    pub fn state_at(&self, age: Years) -> BinaryState {
        let age = age.value();
        debug_assert!(
            age.is_finite() && age >= 0.0 && age <= self.until * (1.0 + 1e-12) + 1e-9,
            "a timeline runs from 0 to {} years, not {age}",
            self.until
        );
        let age = if age > 0.0 { age.min(self.until) } else { 0.0 };
        let index = self
            .segments
            .partition_point(|s| s.start <= age)
            .saturating_sub(1);
        let segment = &self.segments[index];
        let stars = [
            segment.members[0].state_at(&self.context, 0, age),
            segment.members[1].state_at(&self.context, 1, age),
        ];
        let total = stars[0].mass().value() + stars[1].mass().value();
        let orbit = segment
            .orbit
            .as_ref()
            .and_then(|o| o.elements_at(age, total));
        let transfer_rate = match segment.kind {
            SegmentKind::StableTransfer { .. } => segment
                .rates
                .as_ref()
                .map(|r| SolarMassesPerYear::new(step_value(r, age))),
            SegmentKind::Detached
            | SegmentKind::CommonEnvelope
            | SegmentKind::Merged
            | SegmentKind::Disrupted { .. }
            | SegmentKind::Contact => None,
        };
        BinaryState {
            age: Years::new(age),
            stars,
            orbit,
            transfer_rate,
            kind: segment.kind,
        }
    }

    /// The age at which the pair coalesced, if it did by the age it was run to.
    #[must_use]
    pub fn merger_age(&self) -> Option<Years> {
        self.segments
            .iter()
            .find(|s| s.kind == SegmentKind::Merged)
            .map(|s| Years::new(s.start))
    }

    /// The age of each component's supernova (or collapse), primary first, if it had one by the
    /// age the binary was run to; a merger's product explodes as the primary.
    #[must_use]
    pub fn supernova_ages(&self) -> [Option<Years>; 2] {
        Component::BOTH.map(|c| {
            self.supernovae
                .iter()
                .find(|s| s.component == c)
                .map(|s| s.age)
        })
    }

    /// The segments, in age order.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The first pooled Type Ia candidate, if there was one.
    #[must_use]
    pub const fn pooled_ia(&self) -> Option<PooledIaEvent> {
        self.pooled_ia
    }

    /// The supernovae, in age order.
    #[must_use]
    pub fn supernovae(&self) -> &[SupernovaRecord] {
        &self.supernovae
    }

    /// The pair's total recoil after its supernovae, the sum of each one's (their equation A14),
    /// if the pair had a supernova and is still bound.
    #[must_use]
    pub fn recoil(&self) -> Option<SystemVelocity> {
        if self.supernovae.is_empty() || self.supernovae.iter().any(|s| !s.bound) {
            return None;
        }
        Some(
            self.supernovae
                .iter()
                .fold(SystemVelocity::ZERO, |sum, s| sum + s.velocities[0]),
        )
    }

    /// The age the binary was run to.
    #[must_use]
    pub const fn until(&self) -> Years {
        Years::new(self.until)
    }

    /// Whether the engine stopped at its cap on segments ([`MAX_SEGMENTS`](super::MAX_SEGMENTS)),
    /// a broken invariant the tests count.
    #[must_use]
    pub const fn hit_segment_cap(&self) -> bool {
        self.capped
    }

    /// The bytes the timeline owns on the heap, beyond `size_of::<BinaryTimeline>()`, for the
    /// server's byte-bounded caches: its segments and their paths (the shared tracks and context
    /// are counted once each by their owners).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.segments.capacity() * size_of::<Segment>()
            + self.segments.iter().map(Segment::heap_bytes).sum::<usize>()
            + self.supernovae.capacity() * size_of::<SupernovaRecord>()
    }
}

/// The value of a piecewise-constant path at `age`: the value recorded at the end of the step
/// that holds it.
#[must_use]
fn step_value(path: &Path, age: f64) -> f64 {
    let knots = path.knots();
    let i = knots
        .partition_point(|k| k[0] < age)
        .min(knots.len().saturating_sub(1));
    knots.get(i).map_or(0.0, |k| k[1])
}

/// A binary's state at one age (plan 11's Provides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinaryState {
    age: Years,
    stars: [StarState; 2],
    orbit: Option<KeplerElements>,
    transfer_rate: Option<SolarMassesPerYear>,
    kind: SegmentKind,
}

impl BinaryState {
    /// The age, Julian years since the stars' onset of collapse.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// Each star's state, primary first: plan 06's `NoRemnant` for a star merged into the other or
    /// destroyed. A star whose mass the binary changed reports its effective age (HPT section 7.1).
    #[must_use]
    pub const fn stars(&self) -> &[StarState; 2] {
        &self.stars
    }

    /// The relative orbit of the secondary about the primary, while they are bound.
    #[must_use]
    pub const fn orbit(&self) -> Option<&KeplerElements> {
        self.orbit.as_ref()
    }

    /// The rate of transfer from the donor, M☉ yr⁻¹, during stable transfer: the mean over the
    /// engine's step that holds the age.
    #[must_use]
    pub const fn transfer_rate(&self) -> Option<SolarMassesPerYear> {
        self.transfer_rate
    }

    /// The kind of phase.
    #[must_use]
    pub const fn kind(&self) -> SegmentKind {
        self.kind
    }

    /// The pair's total mass, M☉.
    #[must_use]
    pub fn total_mass(&self) -> SolarMasses {
        SolarMasses::new(self.stars[0].mass().value() + self.stars[1].mass().value())
    }
}
