//! The driver (P11.T4.a and T4.f): the pre-test [`can_interact`], and [`evolve`], which runs a
//! close pair forward once, event to event, into its timeline (plan 11, design notes 6 and 7).
//!
//! The engine follows Hurley, Tout and Pols (2002, "BSE") section 2.8. A detached pair is stepped
//! with the limits of their equations 88 and 89 (each star's own step, 1% of its mass, 2% of the
//! orbital angular momentum, 3% of a spin under magnetic braking) until a star fills its Roche
//! lobe, the stars collide at periastron, a star changes phase or dies, or the age is reached. At
//! the onset of Roche-lobe overflow the transfer is tested for dynamical stability (their section
//! 2.6.1) and runs as stable transfer (`rlof.rs`), a common envelope or a merger
//! (`common_envelope.rs`). Supernovae change the orbit and may unbind it (`supernova.rs`). Each of
//! these ends a segment of the timeline; a timeline holds at most [`MAX_SEGMENTS`], and a pair that
//! reaches the cap is left as it stands, which the tests count as a broken invariant.
//!
//! A primary of `m_cc(Z) − 1` M☉ or more whose single-star death is a collapse dies when plan 06
//! says, with plan 06's remnant and kick (design note 16): the engine takes that age as a fixed
//! boundary, holds the star at its last living state if its own binary history would end it
//! sooner, and explodes whatever the pair has made of it then.

use std::sync::Arc;

use crate::orbit::{Orientation, roche_lobe_radius};
use crate::stellar::multiplicity::stripped_mark_min_mass;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::{CompactRemnant, Death, NatalKick, StandardKickLaw};
use crate::stellar::sse::{self, Track};
use crate::stellar::substellar;
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{Metres, Radians, SolarMasses, Years};

use super::star::{Member, Path, positive, zams_spin};
use super::timeline::{
    BinaryInput, BinaryTimeline, Context, OrbitPath, PooledIaEvent, Segment, SegmentKind,
    SupernovaRecord,
};

/// The most segments a timeline holds (P11.T4.f): reaching it is a broken invariant, counted in
/// the tests.
pub const MAX_SEGMENTS: usize = 64;

/// The most events (phases the driver runs) a timeline takes, past which the engine stops as it
/// does at [`MAX_SEGMENTS`]: a guard against a loop of events that make no segments.
const MAX_EVENTS: u32 = 4_096;

/// The most steps one phase takes, past which the engine stops as it does at [`MAX_SEGMENTS`].
pub(super) const MAX_STEPS: u32 = 200_000;

/// Whether a pair can interact by `until_age`, the cheap pre-test (plan 11, design note 7): the
/// periastron of its orbit is inside the Roche-filling separation of either star's largest radius
/// up to then, r ≥ `r_L(q)` a (1 − e) with Eggleton's lobe (1983) and each star's
/// [`Track::max_radius_until`]. A star below 0.1 M☉ takes the largest radius of P06.T13's
/// cooling fits, their first. Everything else is two single stars on an orbit.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::binary::{BinaryInput, can_interact};
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::units::{GravitationalParameter, Radians, Seconds, SolarMasses, Years};
///
/// let days = |d: f64| Seconds::new(d * 86_400.0);
/// let orbit = |period| {
///     KeplerElements::from_period(
///         days(period),
///         GravitationalParameter::from_solar_masses(SolarMasses::new(2.0)),
///         Eccentricity::CIRCULAR,
///         Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))?,
///         Radians::new(0.0),
///     )
/// };
/// let pair = |period| {
///     BinaryInput::new(
///         SolarMasses::new(1.2),
///         SolarMasses::new(0.8),
///         Composition::SOLAR,
///         orbit(period).map_err(|e| e.to_string())?,
///         [StarDraws::median(), StarDraws::median()],
///         Years::new(5.0e9),
///     )
///     .map_err(|e| e.to_string())
/// };
/// // A 1.2 M☉ star's giant branch reaches a 100-day companion, but not one 10⁶ days out.
/// assert!(can_interact(&pair(100.0)?, Years::new(1.0e10)));
/// assert!(!can_interact(&pair(1.0e6)?, Years::new(1.0e10)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn can_interact(input: &BinaryInput, until_age: Years) -> bool {
    let until = until_age.value().max(0.0);
    let members = own_members(input, until, None);
    interacts(input, &members, until)
}

/// Runs the pair of `input` forward once from zero age to `until_age`, into its timeline (plan
/// 11, design note 6).
///
/// A pair that cannot interact ([`can_interact`]) is one detached segment of two single stars on
/// their orbit, whose states are plan 06's own. Otherwise the engine runs event to event (see the
/// module documentation).
///
/// # Examples
///
/// A close pair of a 2.9 and a 0.9 M☉ star (Hurley, Tout and Pols 2002, section 3.1) becomes an
/// Algol: the primary fills its Roche lobe on the Hertzsprung gap and gives most of its mass to
/// its companion.
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::binary::{BinaryInput, BinaryParams, SegmentKind, evolve};
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::units::{GravitationalParameter, Radians, Seconds, SolarMasses, Years};
///
/// let orbit = KeplerElements::from_period(
///     Seconds::new(3.0 * 86_400.0),
///     GravitationalParameter::from_solar_masses(SolarMasses::new(3.8)),
///     Eccentricity::CIRCULAR,
///     Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))?,
///     Radians::new(0.0),
/// )?;
/// let input = BinaryInput::new(
///     SolarMasses::new(2.9),
///     SolarMasses::new(0.9),
///     Composition::SOLAR,
///     orbit,
///     [StarDraws::median(), StarDraws::median()],
///     Years::new(1.0e9),
/// )?;
/// let timeline = evolve(&input, Years::new(5.0e8));
/// assert!(timeline
///     .segments()
///     .iter()
///     .any(|s| matches!(s.kind(), SegmentKind::StableTransfer { .. })));
/// let after = timeline.state_at(Years::new(5.0e8));
/// assert!(after.stars()[1].mass().value() > 2.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn evolve(input: &BinaryInput, until_age: Years) -> BinaryTimeline {
    let until = until_age.value().max(0.0);
    let ctx = Arc::new(Context::of(input));
    let pin = pinned_death(input);
    let primary_track = pin.as_ref().map(|p| Arc::clone(&p.track));
    let members = own_members(input, until, primary_track);
    let orbit = LiveOrbit::of(input);
    if !interacts(input, &members, until) {
        let segment = Segment::new(
            SegmentKind::Detached,
            0.0,
            until,
            members,
            Some(OrbitPath {
                fixed: Some(*input.orbit()),
                ..orbit.path()
            }),
            None,
        );
        return BinaryTimeline::new(vec![segment], None, Vec::new(), until, ctx, false);
    }
    let mut engine = Engine::new(ctx, until, members, orbit, pin.map(|p| p.pin));
    engine.run();
    engine.finish()
}

/// Each star on its own single-star form from zero age: its track built to `until` (the
/// primary's may be given, built in full), or the cooling fits below 0.1 M☉.
#[must_use]
fn own_members(input: &BinaryInput, until: f64, primary: Option<Arc<Track>>) -> [Member; 2] {
    let mut primary = primary;
    core::array::from_fn(|i| {
        let m = input.masses()[i];
        if m < sse::MIN_INITIAL_MASS {
            return Member::Cooling {
                offset: 0.0,
                mass: Path::starting(0.0, m.value()),
            };
        }
        let track = match (i, primary.take()) {
            (0, Some(track)) => track,
            _ => Arc::new(Track::to_age(
                track_mass(m),
                input.composition(),
                &input.draws()[i],
                Years::new(until),
            )),
        };
        Member::Track { track, offset: 0.0 }
    })
}

/// The initial mass a track is built for: the star's own, held to the formulae's 100 M☉ as plan
/// 06's `StarModel` holds it.
#[must_use]
pub(super) fn track_mass(m: SolarMasses) -> SolarMasses {
    if m > sse::MAX_INITIAL_MASS {
        sse::MAX_INITIAL_MASS
    } else {
        m
    }
}

/// Whether the pair can interact by `until` on its members' own tracks ([`can_interact`]).
#[must_use]
fn interacts(input: &BinaryInput, members: &[Member; 2], until: f64) -> bool {
    let [m1, m2] = input.masses().map(SolarMasses::value);
    let periastron = input.orbit().periapsis();
    (0..2).any(|i| {
        let (m, other) = if i == 0 { (m1, m2) } else { (m2, m1) };
        let largest = match &members[i] {
            Member::Track { track, .. } => track.max_radius_until(Years::new(until)).value(),
            Member::Cooling { .. } => {
                substellar::cooling(SolarMasses::new(m), Years::ZERO, input.composition())
                    .map_or(0.0, |s| s.radius().value())
            }
            Member::Shaped { .. }
            | Member::MainSequence { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => 0.0,
        };
        let lobe = roche_lobe_radius(m / other, periastron).value() / SOLAR_RADIUS_M;
        largest >= lobe
    })
}

/// The primary's single-star death, which the engine may not move (plan 11, design note 16), with
/// the full track it was read from.
struct PinnedTrack {
    track: Arc<Track>,
    pin: Pin,
}

/// A primary's death as plan 06 gives it: when, what it leaves, its kick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Pin {
    pub(super) death: Death,
    pub(super) remnant: CompactRemnant,
    pub(super) kick: Option<NatalKick>,
}

/// The primary's single-star death if design note 16 pins it: a primary of `m_cc(Z) − 1` M☉ or
/// more (plan 11's `stripped_mark_min_mass`, ruling 93.3) whose death is a collapse, read as plan
/// 06's `StarModel` reads it (`Track::fate_with` its remnant draws, the companion-stripped mark,
/// and the kick law).
#[must_use]
fn pinned_death(input: &BinaryInput) -> Option<PinnedTrack> {
    let m1 = input.masses()[0];
    if m1 < stripped_mark_min_mass(input.composition()) {
        return None;
    }
    let draws = &input.draws()[0];
    let track = Arc::new(Track::full(track_mass(m1), input.composition(), draws));
    let fate = track.fate_with(RemnantDraws::of(draws))?;
    if !fate.death.kind().is_sudden() {
        return None;
    }
    let law = StandardKickLaw::default();
    let death = law.with_stripped_mark(fate.death, draws);
    let kick = law.natal_kick(&death, &fate.remnant, draws);
    Some(PinnedTrack {
        track,
        pin: Pin {
            death,
            remnant: fate.remnant,
            kick,
        },
    })
}

/// The orbit as the engine carries it: its semi-major axis (R☉) and eccentricity now, its
/// orientation and mean anomaly at the epoch, and their paths through the current segment.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LiveOrbit {
    pub(super) a: f64,
    pub(super) e: f64,
    pub(super) orientation: Orientation,
    pub(super) mean_anomaly: Radians,
    pub(super) axis: Path,
    pub(super) ecc: Path,
}

impl LiveOrbit {
    /// The input's orbit at zero age.
    #[must_use]
    fn of(input: &BinaryInput) -> Self {
        let k = input.orbit();
        let a = k.semi_major_axis().value() / SOLAR_RADIUS_M;
        let e = k.eccentricity().value();
        Self {
            a,
            e,
            orientation: *k.orientation(),
            mean_anomaly: k.mean_anomaly_at_epoch(),
            axis: Path::starting(0.0, a),
            ecc: Path::starting(0.0, e),
        }
    }

    /// A new orbit of `a` (R☉) and `e` at `age`.
    #[must_use]
    pub(super) fn new(
        age: f64,
        a: f64,
        e: f64,
        orientation: Orientation,
        mean_anomaly: Radians,
    ) -> Self {
        Self {
            a,
            e,
            orientation,
            mean_anomaly,
            axis: Path::starting(age, a),
            ecc: Path::starting(age, e),
        }
    }

    /// Sets the orbit at `age`, recording it.
    pub(super) fn set(&mut self, age: f64, a: f64, e: f64) {
        self.a = a;
        self.e = e;
        self.axis.push(age, a);
        self.ecc.push(age, e);
    }

    /// The orbit's path through the segment so far.
    #[must_use]
    pub(super) fn path(&self) -> OrbitPath {
        OrbitPath {
            orientation: self.orientation,
            mean_anomaly: self.mean_anomaly,
            axis: self.axis.clone(),
            eccentricity: self.ecc.clone(),
            fixed: None,
        }
    }

    /// The same orbit with its paths cut back to its value at `age`.
    #[must_use]
    fn restarted(&self, age: f64) -> Self {
        Self::new(age, self.a, self.e, self.orientation, self.mean_anomaly)
    }

    /// The semi-major axis in metres.
    #[must_use]
    pub(super) fn axis_metres(&self) -> Metres {
        Metres::new(self.a * SOLAR_RADIUS_M)
    }
}

/// The binary engine's state while it runs.
#[derive(Debug)]
pub(super) struct Engine {
    pub(super) ctx: Arc<Context>,
    pub(super) until: f64,
    pub(super) age: f64,
    pub(super) members: [Member; 2],
    /// Each star's spin angular momentum, M☉ R☉² yr⁻¹.
    pub(super) spins: [f64; 2],
    pub(super) orbit: Option<LiveOrbit>,
    pub(super) kind: SegmentKind,
    pub(super) segment_start: f64,
    /// The transfer rate at each step of a stable-transfer segment.
    pub(super) rates: Option<Path>,
    pub(super) segments: Vec<Segment>,
    pub(super) pooled: Option<PooledIaEvent>,
    pub(super) supernovae: Vec<SupernovaRecord>,
    /// Whether a companion or a common envelope removed each star's envelope.
    pub(super) stripped: [bool; 2],
    /// The helium each white dwarf has accreted, M☉ (BSE section 2.6.6).
    pub(super) accreted_helium: [f64; 2],
    /// The primary's pinned death, until it happens.
    pub(super) pin: Option<Pin>,
    pub(super) capped: bool,
    /// The last step's length, years, from which the next one starts.
    pub(super) dt_hint: f64,
    /// When a contact pair coalesces.
    pub(super) contact_until: f64,
}

impl Engine {
    /// An engine for a pair of `members` on `orbit`, from zero age.
    #[must_use]
    pub(super) fn new(
        ctx: Arc<Context>,
        until: f64,
        members: [Member; 2],
        orbit: LiveOrbit,
        pin: Option<Pin>,
    ) -> Self {
        let mut engine = Self {
            ctx,
            until,
            age: 0.0,
            members,
            spins: [0.0; 2],
            orbit: Some(orbit),
            kind: SegmentKind::Detached,
            segment_start: 0.0,
            rates: None,
            segments: Vec::new(),
            pooled: None,
            supernovae: Vec::new(),
            stripped: [false; 2],
            accreted_helium: [0.0; 2],
            pin,
            capped: false,
            dt_hint: 0.0,
            contact_until: 0.0,
        };
        for i in 0..2 {
            let (mass, tau) = engine.current(i);
            if let Some(s) = engine.structure(i, 0.0, mass, tau) {
                let r = s.state.radius().value();
                engine.spins[i] = super::star::moment_of_inertia(&s) * zams_spin(mass, r);
            }
        }
        engine
    }

    /// Runs the pair to its age, or to the cap.
    pub(super) fn run(&mut self) {
        let mut events = 0_u32;
        while self.age < self.until && !self.capped {
            events += 1;
            if events > MAX_EVENTS {
                self.capped = true;
                break;
            }
            match self.kind {
                SegmentKind::StableTransfer { donor } => self.transfer_phase(donor.index()),
                SegmentKind::Contact => self.contact_phase(),
                SegmentKind::Detached
                | SegmentKind::CommonEnvelope
                | SegmentKind::Merged
                | SegmentKind::Disrupted { .. } => self.detached_phase(),
            }
        }
    }

    /// The timeline, with the last segment closed at the pair's age.
    #[must_use]
    pub(super) fn finish(mut self) -> BinaryTimeline {
        let end = self.until;
        self.push_segment(end);
        BinaryTimeline::new(
            self.segments,
            self.pooled,
            self.supernovae,
            self.until,
            self.ctx,
            self.capped,
        )
    }

    /// Member `i`'s mass and τ now.
    #[must_use]
    pub(super) fn current(&self, i: usize) -> (f64, f64) {
        match &self.members[i] {
            Member::Shaped { mass, .. }
            | Member::Cooling { mass, .. }
            | Member::Remnant { mass, .. } => (mass.last(), 0.0),
            Member::MainSequence { mass, tau, .. } => (mass.last(), tau.last()),
            member @ (Member::Track { .. } | Member::Frozen { .. } | Member::Gone) => {
                (member.mass_at(self.age), 0.0)
            }
        }
    }

    /// Member `i`'s structure at `age` with `mass` and `tau` (see [`Member::evaluate`]).
    #[must_use]
    pub(super) fn structure(
        &self,
        i: usize,
        age: f64,
        mass: f64,
        tau: f64,
    ) -> Option<sse::Structure> {
        self.members[i].evaluate(&self.ctx, i, age, mass, tau)
    }

    /// Records the segment so far, ending at `end`, and starts the next one there of the same
    /// kind. A segment of no length is kept only where it marks an event (a common envelope).
    pub(super) fn close_segment(&mut self) {
        let end = self.age;
        self.push_segment(end);
        self.segment_start = end;
        self.members = [
            self.members[0].restarted(end),
            self.members[1].restarted(end),
        ];
        if let Some(orbit) = &self.orbit {
            self.orbit = Some(orbit.restarted(end));
        }
        self.rates = match self.kind {
            SegmentKind::StableTransfer { .. } => Some(Path::default()),
            SegmentKind::Detached
            | SegmentKind::CommonEnvelope
            | SegmentKind::Merged
            | SegmentKind::Disrupted { .. }
            | SegmentKind::Contact => None,
        };
        if self.segments.len() + 1 >= MAX_SEGMENTS {
            self.capped = true;
        }
    }

    /// Pushes the segment from its start to `end`, unless it is empty and marks nothing.
    fn push_segment(&mut self, end: f64) {
        let empty = end <= self.segment_start;
        if empty && self.kind != SegmentKind::CommonEnvelope && !self.segments.is_empty() {
            return;
        }
        let rates = self.rates.take().filter(|r| !r.knots().is_empty());
        self.segments.push(Segment::new(
            self.kind,
            self.segment_start,
            end,
            self.members.clone(),
            self.orbit.as_ref().map(LiveOrbit::path),
            rates,
        ));
    }

    /// Ends the current segment and starts one of `kind`.
    pub(super) fn begin(&mut self, kind: SegmentKind) {
        self.close_segment();
        self.kind = kind;
        self.rates = match kind {
            SegmentKind::StableTransfer { .. } => Some(Path::default()),
            SegmentKind::Detached
            | SegmentKind::CommonEnvelope
            | SegmentKind::Merged
            | SegmentKind::Disrupted { .. }
            | SegmentKind::Contact => None,
        };
    }

    /// Replaces the members from now, the current segment having been closed: a change of form
    /// inside a phase.
    pub(super) fn set_member(&mut self, i: usize, member: Member) {
        self.members[i] = member;
    }

    /// The kind a pair without Roche-lobe overflow is in: detached while bound, merged when one
    /// member is gone, disrupted otherwise.
    #[must_use]
    pub(super) fn quiet_kind(&self) -> SegmentKind {
        if self.orbit.is_some() {
            SegmentKind::Detached
        } else if self.members.iter().any(Member::is_gone) {
            SegmentKind::Merged
        } else {
            match self.kind {
                kind @ SegmentKind::Disrupted { .. } => kind,
                SegmentKind::Detached
                | SegmentKind::StableTransfer { .. }
                | SegmentKind::CommonEnvelope
                | SegmentKind::Merged
                | SegmentKind::Contact => SegmentKind::Merged,
            }
        }
    }

    /// The Roche-lobe radius of member `i` in the current orbit, R☉, with masses `m`.
    #[must_use]
    pub(super) fn roche_lobe(&self, i: usize, m: [f64; 2]) -> f64 {
        let Some(orbit) = &self.orbit else {
            return f64::INFINITY;
        };
        roche_lobe(m[i], m[1 - i], orbit.a)
    }

    /// Records a pooled Type Ia candidate if it is the first.
    pub(super) fn pool(&mut self, event: PooledIaEvent) {
        if self.pooled.is_none() {
            self.pooled = Some(event);
        }
    }
}

/// The span, in track age, of the phase of `track` (placed at `offset`) that the engine's age `age`
/// is in or about to enter: the segment holding the track's age, or the next one where the age is
/// already at that segment's end to the resolution of the engine's clock, so that a step never
/// stalls on a boundary it cannot reach in floating point.
#[must_use]
pub(super) fn phase_ahead(track: &Track, offset: f64, age: f64) -> (f64, f64, f64) {
    let track_age = (age - offset).max(0.0);
    let (start, end) = track.phase_span(track_age);
    let resolution = 4.0 * f64::EPSILON * age.abs().max(1.0);
    if end.is_finite() && end + offset - age <= resolution {
        let (next_start, next_end) = track.phase_span(end);
        return (next_start, next_end, track_age.max(end));
    }
    (start, end, track_age)
}

/// The offset that puts a track's age `at` at the engine's age `age`, rounded so that the track's
/// age there, age − offset, is never below `at` (a phase starting at `at` holds it).
#[must_use]
pub(super) fn offset_for(age: f64, at: f64) -> f64 {
    let mut offset = age - at;
    for _ in 0..8 {
        if age - offset >= at {
            break;
        }
        offset = offset.next_down();
    }
    offset
}

/// The Roche-lobe radius of a star of `m` with a companion of `other` at separation `a`, all in
/// the engine's units (Eggleton 1983, BSE equation 53).
#[must_use]
pub(super) fn roche_lobe(m: f64, other: f64, a: f64) -> f64 {
    if !(positive(m) && positive(other) && positive(a)) {
        return f64::INFINITY;
    }
    roche_lobe_radius(m / other, Metres::new(a)).value()
}
