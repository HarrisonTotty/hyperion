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
//!
//! The engine starts where the first star arrives on its main sequence ([`arrival`], P11.T4.i).
//! A companion still contracting then, as a low-mass companion of a massive primary is through
//! the primary's whole life, is carried as its own zero-age main-sequence star until its own
//! arrival ([`engine_track_age_years`]), as Hurley, Tout and Pols (2002, section 2.8) start both
//! stars, and is shown on its own pre-main-sequence track until the pair touches it.
//!
//! [`engine_track_age_years`]: super::star::engine_track_age_years

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
    BinaryInput, BinaryTimeline, Context, FixedOrbit, OrbitPath, PooledIaEvent, Segment,
    SegmentKind, SupernovaRecord,
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
/// first star has arrived on the main sequence by then, and the periastron of its orbit is inside
/// the Roche-filling separation of either star's largest radius up to then, r ≥ `r_L(q)` a (1 − e)
/// with Eggleton's lobe (1983) and each star's [`Track::max_radius_until`]. A star that has not
/// arrived by then takes its zero-age main-sequence radius, as the engine carries it (P11.T4.i). A
/// star below 0.1 M☉ takes the largest radius of P06.T13's cooling fits, their first. Everything
/// else is two single stars on an orbit.
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

/// [`can_interact`] with the stars' own tracks where the caller holds them, as
/// [`evolve_with_tracks`] takes them: the same answer, without building them again.
#[must_use]
pub(crate) fn can_interact_with_tracks(
    input: &BinaryInput,
    until_age: Years,
    tracks: [Option<Arc<Track>>; 2],
) -> bool {
    let until = until_age.value().max(0.0);
    let members = own_members_with(input, until, tracks);
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
    evolve_with_tracks(input, until_age, [None, None])
}

/// [`evolve`] with the stars' own tracks, where the caller holds them: each `Some` must be the
/// track [`evolve`] would build for that star, [`Track::to_age`] of its mass, the pair's
/// composition and its draws at `until_age` (or past it, built in full), so that the timeline is
/// [`evolve`]'s bit for bit (a test holds it). A pinned primary's full track (design note 16) is
/// taken from its own only when that is built in full.
///
/// This is how plan 06's [`SystemStars`](crate::stellar::system::SystemStars) runs its pairs
/// (P11.T11, ruling 111.5): its `StarModel`s already hold the tracks, which saves their builds,
/// about 1 ms a pair.
#[must_use]
pub(crate) fn evolve_with_tracks(
    input: &BinaryInput,
    until_age: Years,
    tracks: [Option<Arc<Track>>; 2],
) -> BinaryTimeline {
    let until = until_age.value().max(0.0);
    let ctx = Arc::new(Context::of(input));
    let [own_primary, own_secondary] = tracks;
    let full_primary = own_primary
        .as_ref()
        .filter(|track| track.built_until().value().is_infinite())
        .map(Arc::clone);
    let pin = pinned_death(input, full_primary);
    let primary_track = pin.as_ref().map(|p| Arc::clone(&p.track)).or(own_primary);
    let members = own_members_with(input, until, [primary_track, own_secondary]);
    if !interacts(input, &members, until) {
        let segment = Segment::new(
            SegmentKind::Detached,
            0.0,
            until,
            members,
            Some(OrbitPath {
                fixed: Some(FixedOrbit::always(*input.orbit())),
                ..LiveOrbit::of(input, Years::ZERO).path()
            }),
            None,
        );
        return BinaryTimeline::new(vec![segment], None, Vec::new(), until, ctx, false);
    }
    // Before the start the stars are their own and the orbit the drawn one (`arrival`): the paths
    // the engine carries begin there, so that nothing before it depends on the steps after it.
    let start = arrival(&members, until);
    let members = members.map(|member| member.restarted(start));
    let orbit = LiveOrbit::of(input, Years::new(start));
    let mut engine = Engine::new(ctx, start, until, members, orbit, pin.map(|p| p.pin));
    engine.run();
    engine.finish()
}

/// Where the engine starts stepping for a pair of `members` run to `until`: where the first star
/// arrives on the main sequence (P06.T15.b; P11.T4.i, ruling p11-channels of 2026-10-06), and no
/// later than `until`.
///
/// A convention, as in the codes plan 11 follows: binary population codes evolve both stars from
/// the zero-age main sequence (Hurley, Tout and Pols 2002, section 2.8), and the orbits plan 11
/// draws are those observed about main-sequence primaries, a zero-age population (Raghavan et al.
/// 2010; Duchêne and Kraus 2013; Sana et al. 2012; Moe and Di Stefano 2017). What the pre-main
/// sequence does to a pair is already in them: close pairs form wider and are brought in then
/// (Bate, Bonnell and Bromm 2002; Moe and Kratter 2018), and the pairs that merge while embedded
/// (Stahler 2010; Tokovinin and Moe 2020) are counted as single stars. Merging the drawn pairs
/// whose contracting stars overfill their orbits would count those mergers twice. So nothing
/// interacts before the first arrival, which is never before accretion ends
/// ([`PROTOSTAR_YEARS`](crate::stellar::premain::PROTOSTAR_YEARS)): each star is its own
/// protostar or contraction on the drawn orbit,
/// detached, in the timeline's first segment. Shown before then, such stars overlap their orbit,
/// and the drawn orbit holds their final masses while they accrete.
///
/// From the first arrival a star still contracting is its own zero-age main-sequence star to the
/// engine, its clock held at τ = 0 until its own arrival
/// ([`engine_track_age_years`](super::star::engine_track_age_years)), so that a massive primary
/// meets a low-mass companion that arrives after the primary's main sequence ends, and its
/// supernova acts on the pair. At \[Fe/H\] 0 such companions are those below about 0.3, 1.0 and
/// 1.8 M☉ beside 4, 8 and 20 M☉; the last rests on the arrival law's Kelvin–Helmholtz extension,
/// and would be nearer 2 M☉ in MIST (`premain`). Between the two arrivals the later star fills its
/// lobe only where its zero-age radius overfills the drawn orbit, never by its contracting
/// radius.
///
/// Each arrival is [`Track::main_sequence_arrival`], which does not depend on how far the track is
/// built, so neither does the start: a pair run to an age before a star's arrival is the same pair,
/// to that age, as one run past it (P11's protostar mergers, 2026-10-05: read from the built
/// segments alone, a star built short of its main sequence had no arrival, and the pair was
/// stepped from zero age as protostars, which merged at once into a 0.01 M☉ cooling star).
#[must_use]
pub(super) fn arrival(members: &[Member; 2], until: f64) -> f64 {
    members
        .iter()
        .filter_map(|member| match member {
            Member::Track { track, .. } => track.main_sequence_arrival().map(Years::value),
            Member::Shaped { .. }
            | Member::MainSequence { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => None,
        })
        .reduce(f64::min)
        .unwrap_or(0.0)
        .min(until)
}

/// Each star on its own single-star form from zero age: its track built to `until`, or to its
/// own main sequence's start if that is later (the primary's may be given, built in full), or the
/// cooling fits below 0.1 M☉.
#[must_use]
fn own_members(input: &BinaryInput, until: f64, primary: Option<Arc<Track>>) -> [Member; 2] {
    own_members_with(input, until, [primary, None])
}

/// [`own_members`] with any of the stars' tracks given.
///
/// Every track is built at least to its own star's arrival on the main sequence
/// ([`sse::main_sequence_start`], which is [`Track::main_sequence_arrival`] bit for bit): a star
/// that has not arrived by `until` is read there, as its zero-age main-sequence star
/// ([`engine_track_age_years`](super::star::engine_track_age_years)). A track's segments are the
/// same however far it is built, so this changes no state the pair had before.
#[must_use]
fn own_members_with(
    input: &BinaryInput,
    until: f64,
    tracks: [Option<Arc<Track>>; 2],
) -> [Member; 2] {
    let mut tracks = tracks;
    core::array::from_fn(|i| {
        let m = input.masses()[i];
        if m < sse::MIN_INITIAL_MASS {
            return Member::Cooling {
                offset: 0.0,
                mass: Path::starting(0.0, m.value()),
            };
        }
        let build = |reach: f64| {
            Arc::new(Track::to_age(
                track_mass(m),
                input.composition(),
                &input.draws()[i],
                Years::new(reach),
            ))
        };
        let need = until.max(sse::main_sequence_start(track_mass(m), input.composition()));
        let track = tracks[i].take().unwrap_or_else(|| build(need));
        // A build that ends within the margin past `need` is built on (`reach_margin_years`).
        let margin = reach_margin_years(need);
        let track = if track.built_until().value() > need + margin {
            track
        } else {
            build(need + margin)
        };
        Member::Track { track, offset: 0.0 }
    })
}

/// How far past the pair's age `until` (years) the engine builds the tracks it reads, years:
/// 16 ε `until`, past the resolution of [`phase_ahead`]'s look-ahead there (4 ε of the age).
///
/// A step that starts within that resolution below a track's last boundary looks for the next
/// segment, which a track built just short of it does not have: it would stop on the boundary
/// again and again, to its cap. The age the pair is run to once cut that step and must not
/// (`detached::StepLimit`), so every track is built past it by this margin, which changes no
/// state, since a track's segments are the same however far it is built.
#[must_use]
pub(super) fn reach_margin_years(until: f64) -> f64 {
    16.0 * f64::EPSILON * until.abs().max(1.0)
}

/// The initial mass a track is built for: the star's own, held to the formulae's 150 M☉ as plan
/// 06's `StarModel` holds it.
#[must_use]
pub(super) fn track_mass(m: SolarMasses) -> SolarMasses {
    if m > sse::MAX_INITIAL_MASS {
        sse::MAX_INITIAL_MASS
    } else {
        m
    }
}

/// Whether the pair can interact by `until` on its members' own tracks ([`can_interact`]): never
/// before the first star has arrived on the main sequence (`arrival`), where the stars' largest
/// radii would be their contracting ones. A star that has not arrived by `until` takes its
/// largest radius at its arrival, its zero-age main-sequence radius, as the engine reads it
/// ([`engine_track_age_years`](super::star::engine_track_age_years)).
#[must_use]
fn interacts(input: &BinaryInput, members: &[Member; 2], until: f64) -> bool {
    if arrival(members, until) >= until {
        return false;
    }
    let [m1, m2] = input.masses().map(SolarMasses::value);
    let periastron = input.orbit().periapsis();
    (0..2).any(|i| {
        let (m, other) = if i == 0 { (m1, m2) } else { (m2, m1) };
        let largest = match &members[i] {
            Member::Track { track, offset } => {
                let at = super::star::engine_track_age_years(track, *offset, until);
                track.max_radius_until(Years::new(at)).value()
            }
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
fn pinned_death(input: &BinaryInput, full: Option<Arc<Track>>) -> Option<PinnedTrack> {
    let m1 = input.masses()[0];
    if m1 < stripped_mark_min_mass(input.composition()) {
        return None;
    }
    let draws = &input.draws()[0];
    let track =
        full.unwrap_or_else(|| Arc::new(Track::full(track_mass(m1), input.composition(), draws)));
    let fate = track.fate_with(RemnantDraws::of(draws))?;
    if !fate.death.kind().is_sudden() {
        return None;
    }
    let law = StandardKickLaw::default();
    let death =
        law.with_stripped_mark(fate.death, draws, track.initial_mass(), track.composition());
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
    /// The drawn orbit up to the engine's start, in the first segment only (`arrival`).
    drawn: Option<FixedOrbit>,
}

impl LiveOrbit {
    /// The input's orbit, drawn, up to the engine's start at `start`, where its paths begin.
    #[must_use]
    fn of(input: &BinaryInput, start: Years) -> Self {
        let k = input.orbit();
        let a = k.semi_major_axis().value() / SOLAR_RADIUS_M;
        let e = k.eccentricity().value();
        Self {
            a,
            e,
            orientation: *k.orientation(),
            mean_anomaly: k.mean_anomaly_at_epoch(),
            axis: Path::starting(start.value(), a),
            ecc: Path::starting(start.value(), e),
            drawn: Some(FixedOrbit {
                elements: *k,
                until: start,
            }),
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
            drawn: None,
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
            fixed: self.drawn,
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
    /// When the last Roche-lobe overflow began, years, and the donor's main-sequence lifetime
    /// then, years (0 for any other donor, and before the first onset): Nelson and Eggleton's
    /// `t_RLOF` and `t_MS` (ruling 114.2).
    pub(super) overflow_onset: (f64, f64),
    /// When the accretor first filled its lobe in the current transfer, years (Nelson and
    /// Eggleton's `t_contact`; ruling 114.2).
    pub(super) first_contact: Option<f64>,
    /// Whether a common envelope is being resolved: a merger it leads to never starts another
    /// (the guard against the recursion of a held bare core, ruling 129.4c).
    pub(super) in_common_envelope: bool,
}

impl Engine {
    /// An engine for a pair of `members` on `orbit`, stepping from `start` (years; zero age, or
    /// where the first star has arrived on the main sequence), its first segment from zero age.
    #[must_use]
    pub(super) fn new(
        ctx: Arc<Context>,
        start: f64,
        until: f64,
        members: [Member; 2],
        orbit: LiveOrbit,
        pin: Option<Pin>,
    ) -> Self {
        let mut engine = Self {
            ctx,
            until,
            age: start,
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
            overflow_onset: (0.0, 0.0),
            first_contact: None,
            in_common_envelope: false,
        };
        for i in 0..2 {
            let (mass, tau) = engine.current(i);
            if let Some(s) = engine.structure(i, start, mass, tau) {
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

    /// The span, years, from the engine's age `age` to past the pair's age by
    /// [`reach_margin_years`], which a track the engine builds at `age` must hold.
    #[must_use]
    pub(super) fn reach_span_years(&self, age: f64) -> f64 {
        (self.until - age).max(0.0) + reach_margin_years(self.until)
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

/// A track for a star the engine places on it now at `at(track)`, a track age, built far enough to
/// hold the star `span_years` on, past the pair's age ([`Engine::reach_span_years`]); and that
/// age, if found.
///
/// `build` builds the track to a track age, and `guess_years` is where the caller expects the
/// star to be placed. A track is built to the segment that holds its reach, and a guess short of
/// the placement (the closed-form main-sequence lifetime against the track's own, short by up to
/// 0.15% at 3–8 M☉) left the pair's age past the track's end, or the phase sought not built at
/// all. The engine then stopped on the track's last boundary again and again, to its cap, and the
/// star froze in a pair run to that age but not in one run past the next segment (P11's build-age
/// dependence, 2026-10-05). So the track is built again: past its end while the placement is not
/// in it, then from the placement. A track's segments are the same however far it is built, so a
/// placement found stays the same.
#[must_use]
pub(super) fn track_reaching(
    guess_years: f64,
    span_years: f64,
    build: impl Fn(f64) -> Track,
    at: impl Fn(&Track) -> Option<f64>,
) -> (Track, Option<f64>) {
    let mut track = build(guess_years + span_years);
    for _ in 0..REBUILDS {
        let end = track.built_until().value();
        match at(&track) {
            Some(placed) if end > placed + span_years => return (track, Some(placed)),
            Some(placed) => track = build(placed + span_years),
            None if end.is_finite() => track = build(end + span_years),
            None => return (track, None),
        }
    }
    let placed = at(&track);
    (track, placed)
}

/// The most rebuilds [`track_reaching`] makes: one to find the phase, one to reach past the
/// placement, and two to spare.
const REBUILDS: u32 = 4;

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
