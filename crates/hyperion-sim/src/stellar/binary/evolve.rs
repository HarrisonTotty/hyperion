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
//! A pair the pre-test passes over is two single stars on its drawn orbit, and its stars' core
//! collapses change that orbit by BSE appendix A1 in closed form; the engine takes it over from a
//! collapse whose new orbit can interact (P11.T4.l, [`Engine::passed_over`]).
//!
//! A primary of `m_cc(Z) − 1` M☉ or more whose single-star death is a collapse dies when plan 06
//! says, with plan 06's remnant and kick (design note 16): the engine takes that age as a fixed
//! boundary, holds the star at its last living state if its own binary history would end it
//! sooner, and explodes whatever the pair has made of it then. Its own track's main sequence ends
//! at an age the engine stops at too, where a hydrogen main sequence the pair has prolonged goes on
//! along that track (P11.T4.n, [`Engine::end_pinned_main_sequence`]), so that nothing collapses
//! from a main sequence.
//!
//! The engine starts where the first star arrives on its main sequence ([`arrival`], P11.T4.i).
//! A companion still contracting then, as a low-mass companion of a massive primary is through
//! the primary's whole life, is carried as its own zero-age main-sequence star until its own
//! arrival ([`engine_track_age_years`]), as Hurley, Tout and Pols (2002, section 2.8) start both
//! stars, and is shown on its own pre-main-sequence track until the pair touches it.
//!
//! [`engine_track_age_years`]: super::star::engine_track_age_years

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::orbit::{Orientation, roche_lobe_radius};
use crate::stellar::multiplicity::stripped_mark_min_mass;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::{
    CompactRemnant, Death, NatalKick, StandardKickLaw, stripped_mark_applied_under,
};
use crate::stellar::sse::{self, Track};
use crate::stellar::{Phase, substellar};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{Megayears, Metres, Radians, SolarMasses, Years};

use super::detached::TestedOrbit;
use super::dwarf_limit::CoreSource;
use super::star::{Member, Path, positive, zams_spin};
use super::timeline::{
    BinaryInput, BinaryTimeline, Context, FixedOrbit, OrbitPath, PooledIaEvent, Segment,
    SegmentKind, SupernovaRecord,
};
use super::{collapses, wind_accretion};

/// The most segments a timeline holds (P11.T4.f): reaching it is a broken invariant, counted in
/// the tests. 64 until 27, when the sky-light fit's layer E (`sky_binary_light_e`) met four inner
/// pairs of 78–111 + 66–100 M☉ whose Hertzsprung-gap primary, after its first transfer, gives
/// mass in spells, detaching after each until its swelling refills its lobe a few years on (32
/// spells in cell 48's system 984), to its helium star in 71–87 segments: the cap stopped them at
/// about 3.2–3.6 Myr and stretched the last segment to the system's age, leaving a companion of
/// 59–94 M☉ on its main sequence for gigayears. The cap is a guard against a loop, which
/// `MAX_EVENTS` and `MAX_STEPS` bound too, so it is set well above such a history.
pub const MAX_SEGMENTS: usize = 256;

#[cfg(test)]
thread_local! {
    /// The cap on segments of this thread's runs, [`MAX_SEGMENTS`] unless a test raises it
    /// ([`evolve_with_segment_cap`]); the generator's builds have no such knob.
    static SEGMENT_CAP: core::cell::Cell<usize> = const { core::cell::Cell::new(MAX_SEGMENTS) };
}

/// The cap on a timeline's segments: [`MAX_SEGMENTS`], or in a test build the cap a test has set
/// ([`evolve_with_segment_cap`]).
#[must_use]
fn segment_cap() -> usize {
    #[cfg(test)]
    {
        SEGMENT_CAP.with(core::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        MAX_SEGMENTS
    }
}

/// The most events (phases the driver runs) a timeline takes, past which the engine stops as it
/// does at [`MAX_SEGMENTS`]: a guard against a loop of events that make no segments.
const MAX_EVENTS: u32 = 4_096;

/// The most steps one phase takes, past which the engine stops as it does at [`MAX_SEGMENTS`].
pub(super) const MAX_STEPS: u32 = 200_000;

/// Whether a pair can interact by `until_age`: the engine's cheap pre-test (plan 11, design note 7).
///
/// The first star has arrived on the main sequence by then, and either the periastron of the drawn
/// orbit is inside the Roche-filling separation of either star's largest radius up to then (the
/// lobe test: r ≥ `r_L(q)` a (1 − e) with Eggleton's lobe (1983) and each star's
/// [`Track::max_radius_until`]), or the engine's own sinks of orbital angular momentum can shrink
/// the orbit into that test by then (P11.T4.j, ruling p11-channels of 2026-10-06): magnetic braking
/// at the tidal equilibrium spin (BSE equation 50), gravitational radiation (equation 48) and the
/// spin the tides can take from the orbit (equations 34–35, with what a locked star's wind takes,
/// HPT equation 110, P11.T4.l), each at its largest. Those are the W Ursae Majoris channel and the
/// giants' tidal captures, which the lobe test alone passes over.
///
/// A star that has not arrived by then takes its zero-age main-sequence radius, as the engine
/// carries it (P11.T4.i). A star below 0.1 M☉ takes the largest radius of P06.T13's cooling fits,
/// their first. Under the 27 hold of P11.T12's fix (`wind_accretion`) a pair those tests pass over
/// can interact too where its main-sequence star takes 10⁻⁴ M☉ of its partner's wind by then, if
/// those tests pass it over, and neither star dies suddenly, up to the universe's age. Everything
/// else is two single stars on an orbit that their core collapses change (BSE appendix A1,
/// P11.T4.l): a pair this passes over at an age shows no interaction before that age and before
/// its first supernova in a run to any later age (P11.T4.j's wording). After a collapse a later
/// run may take the pair over where an earlier one keeps its new orbit fixed.
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
/// let orbit = |total: f64, period| {
///     KeplerElements::from_period(
///         days(period),
///         GravitationalParameter::from_solar_masses(SolarMasses::new(total)),
///         Eccentricity::CIRCULAR,
///         Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))?,
///         Radians::new(0.0),
///     )
/// };
/// let pair = |m1: f64, m2: f64, period| {
///     BinaryInput::new(
///         SolarMasses::new(m1),
///         SolarMasses::new(m2),
///         Composition::SOLAR,
///         orbit(m1 + m2, period).map_err(|e| e.to_string())?,
///         [StarDraws::median(), StarDraws::median()],
///         Years::new(5.0e9),
///     )
///     .map_err(|e| e.to_string())
/// };
/// // A 1.2 M☉ star's giant branch reaches a 100-day companion, but not one 10⁶ days out.
/// assert!(can_interact(&pair(1.2, 0.8, 100.0)?, Years::new(1.0e10)));
/// assert!(!can_interact(&pair(1.2, 0.8, 1.0e6)?, Years::new(1.0e10)));
/// // At 0.69 days a 1.04 and a 0.45 M☉ star stay inside their lobes on the drawn orbit, but
/// // magnetic braking brings them into transfer at 2.6 Gyr: the pair is run by 2 Gyr.
/// let braked = pair(1.04, 0.45, 0.69)?;
/// assert!(!can_interact(&braked, Years::new(1.0e8)));
/// assert!(can_interact(&braked, Years::new(2.0e9)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn can_interact(input: &BinaryInput, until_age: Years) -> bool {
    let until = until_age.value().max(0.0);
    let members = own_members(input, until, None);
    interacts(input, &members, until) || wind_accretion::onset(input, &members, until).is_some()
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
    interacts(input, &members, until) || wind_accretion::onset(input, &members, until).is_some()
}

/// Runs the pair of `input` forward once from zero age to `until_age`, into its timeline (plan
/// 11, design note 6).
///
/// A pair that cannot interact ([`can_interact`]) is two single stars on their drawn orbit, whose
/// states are plan 06's own, unless a star of it dies suddenly by `until_age`: then the engine runs
/// it from the start, and its collapse acts on its orbit (P11.T10's fix, `collapses.rs`). A pair
/// whose stars only take each other's wind (from the 27 hold of P11.T12's fix) is the closed form
/// of `wind_accretion`. Otherwise the engine runs event to event (see the module documentation).
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
    run_pair(input, until_age, tracks, PreTest::Gate)
}

/// Whether [`run_pair`] reads the pre-test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreTest {
    /// A pair that cannot interact is two single stars on the drawn orbit, and one whose orbit a
    /// collapse leaves cannot interact on goes on on it (the generator's).
    Gate,
    /// Every pair is run through the engine, for the tests that check the pre-test against it.
    #[cfg(test)]
    Past,
    /// The pre-test is read on the drawn orbit, and every pair a bound collapse leaves is run
    /// through the engine from it, for the tests that check the pre-test asked again after a
    /// collapse (P11.T4.l).
    #[cfg(test)]
    PastAfterCollapse,
}

/// [`evolve`] with the stars' own tracks, the pre-test read as `pre_test` says.
#[must_use]
fn run_pair(
    input: &BinaryInput,
    until_age: Years,
    tracks: [Option<Arc<Track>>; 2],
    pre_test: PreTest,
) -> BinaryTimeline {
    #[cfg(any(test, feature = "testing"))]
    super::scan::count_engine_call();
    let until = until_age.value().max(0.0);
    let ctx = Arc::new(Context::of(input));
    let [own_primary, own_secondary] = tracks;
    let full_primary = own_primary
        .as_ref()
        .filter(|track| track.built_until().value().is_infinite())
        .map(Arc::clone);
    let pin = pinned_death(input, full_primary);
    let own_end = pin.as_ref().and_then(PinnedTrack::main_sequence_end);
    let primary_track = pin.as_ref().map(|p| Arc::clone(&p.track)).or(own_primary);
    let members = own_members_with(input, until, [primary_track, own_secondary]);
    let gated = match pre_test {
        PreTest::Gate => true,
        #[cfg(test)]
        PreTest::PastAfterCollapse => true,
        #[cfg(test)]
        PreTest::Past => false,
    };
    // A star's collapse by `until` runs the pair, so that it acts on the orbit (P11.T10's fix,
    // `collapses.rs`).
    let collapses = || {
        let pinned = pin.as_ref().map(|p| p.pin.death.age().value());
        collapses::runs_pair(first_sudden_death(&members, pinned), until)
    };
    if gated && !interacts(input, &members, until) && !collapses() {
        if let Some(onset) = wind_accretion::onset(input, &members, until) {
            return wind_accretion::wind_fed_timeline(input, members, until, ctx, onset);
        }
        let pin = pin.map(|p| p.pin);
        return Engine::passed_over(input, ctx, until, members, pin, own_end, pre_test);
    }
    // Before the start the stars are their own and the orbit the drawn one (`arrival`): the paths
    // the engine carries begin there, so that nothing before it depends on the steps after it.
    let start = arrival(&members, until);
    let members = members.map(|member| member.restarted(start));
    let orbit = LiveOrbit::of(input, Years::new(start));
    let mut engine = Engine::new(ctx, start, until, members, orbit, pin.map(|p| p.pin))
        .with_own_main_sequence_end(own_end);
    engine.run();
    engine.finish()
}

/// The age of the earliest sudden death (a collapse, `DeathKind::is_sudden`) of the stars of
/// `members` on their own tracks, years, or `None`: the primary's at its pin, `pinned_years`,
/// where design note 16 pins it, and each other star's own track's death where the track is built
/// to it. A white dwarf's birth is no sudden death.
#[must_use]
pub(super) fn first_sudden_death(members: &[Member; 2], pinned_years: Option<f64>) -> Option<f64> {
    let own = |i: usize| match &members[i] {
        Member::Track { track, offset } => track
            .death()
            .filter(|death| death.kind().is_sudden())
            .map(|death| death.age().value() + offset),
        Member::Shaped { .. }
        | Member::MainSequence { .. }
        | Member::Cooling { .. }
        | Member::Frozen { .. }
        | Member::Remnant { .. }
        | Member::Gone => None,
    };
    let primary = pinned_years.or_else(|| own(0));
    [primary, own(1)].into_iter().flatten().reduce(f64::min)
}

/// Whether [`evolve`] runs the pair of `input` through the engine to `until_age`: where the lobe
/// and decay tests of [`can_interact`] say it can interact by then, or a star of it dies suddenly
/// by then (P11.T10's fix, `run_pair`'s pre-test), for the build-age
/// tests. A pair that [`can_interact`] only by its wind (P11.T12's fix) is not run: it is
/// `wind_accretion`'s closed form.
#[cfg(test)]
#[must_use]
pub(super) fn engine_runs(input: &BinaryInput, until_age: Years) -> bool {
    let until = until_age.value().max(0.0);
    let members = own_members(input, until, None);
    let pinned = pinned_death(input, None).map(|p| p.pin.death.age().value());
    interacts(input, &members, until)
        || collapses::runs_pair(first_sudden_death(&members, pinned), until)
}

/// Whether the lobe and decay tests of [`can_interact`] say the pair of `input` can interact by
/// `until_age`, or a star of it dies suddenly by then (the primary at
/// its pin where design note 16 pins it): the pairs `wind_accretion`'s closed form, which records
/// no supernova, leaves their own models, so that none the engine runs at a later age is fed there.
#[must_use]
pub(super) fn runs_or_collapses_by(input: &BinaryInput, until_age: Years) -> bool {
    let until = until_age.value().max(0.0);
    let members = own_members(input, until, None);
    let pinned = pinned_death(input, None).map(|p| p.pin.death.age().value());
    interacts(input, &members, until)
        || first_sudden_death(&members, pinned).is_some_and(|age| age <= until)
}

/// The drawn orbit of `input` at every age: a passed-over pair's, two single stars on their orbit.
#[must_use]
pub(super) fn drawn_orbit_path(input: &BinaryInput) -> OrbitPath {
    OrbitPath {
        fixed: Some(FixedOrbit::always(*input.orbit())),
        ..LiveOrbit::of(input, Years::ZERO).path()
    }
}

/// [`evolve`] past the pre-test: the engine runs every pair, whether [`can_interact`] passes it or
/// not, so that the tests can check the pre-test against the engine itself (P11.T4.j).
#[cfg(test)]
#[must_use]
pub(super) fn evolve_past_the_pre_test(input: &BinaryInput, until_age: Years) -> BinaryTimeline {
    run_pair(input, until_age, [None, None], PreTest::Past)
}

/// [`evolve`] past the pre-test asked again after a passed-over pair's bound collapse: the engine
/// runs every such pair from the collapse, whatever that pre-test says, so that the tests can check
/// it against the engine (P11.T4.l).
#[cfg(test)]
#[must_use]
pub(super) fn evolve_past_the_second_pre_test(
    input: &BinaryInput,
    until_age: Years,
) -> BinaryTimeline {
    run_pair(input, until_age, [None, None], PreTest::PastAfterCollapse)
}

/// [`evolve`] with its cap on segments at `cap` in place of [`MAX_SEGMENTS`], on this thread
/// alone and for this run alone, so that the tests can run a capped timeline on and compare its
/// end (P11's held floor, dropped in version 25's wiring; `decision-galaxy-25-train.md`, "The
/// held floor"). The generator's path is untouched: [`segment_cap`] is [`MAX_SEGMENTS`] outside
/// test builds, and here outside this call.
#[cfg(test)]
#[must_use]
pub(super) fn evolve_with_segment_cap(
    input: &BinaryInput,
    until_age: Years,
    cap: usize,
) -> BinaryTimeline {
    /// Puts the thread's cap back when the run ends, a panicking one included.
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SEGMENT_CAP.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(SEGMENT_CAP.with(|c| c.replace(cap)));
    evolve(input, until_age)
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
pub(super) fn own_members(
    input: &BinaryInput,
    until: f64,
    primary: Option<Arc<Track>>,
) -> [Member; 2] {
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
            Arc::new(Track::to_age_with(
                track_mass(m),
                input.composition(),
                &input.draws()[i],
                input.track_options(),
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
/// radii would be their contracting ones; then if the drawn orbit reaches the lobe test
/// ([`reaches_lobe`]), or if the decay the engine's own sinks can make by `until` brings it there
/// ([`decay_reaches`](super::detached::decay_reaches), P11.T4.j). A star that has not arrived by
/// `until` takes its largest radius at its arrival, its zero-age main-sequence radius, as the
/// engine reads it ([`engine_track_age_years`](super::star::engine_track_age_years)).
#[must_use]
pub(super) fn interacts(input: &BinaryInput, members: &[Member; 2], until: f64) -> bool {
    let start = arrival(members, until);
    if start >= until {
        return false;
    }
    let radii = largest_radii_rsun(input, members, until);
    reaches_lobe(input, radii)
        || super::detached::decay_reaches(
            input.params(),
            &TestedOrbit::drawn(input, members, until),
            members,
            start,
            until,
            radii,
        )
}

/// Each star's largest radius up to the engine's age `until_years`, R☉, as the lobe test reads
/// it: a star on its own track its [`Track::max_radius_until`] at its engine age, held to no less
/// than its arrival ([`engine_track_age_years`](super::star::engine_track_age_years)); a star
/// below 0.1 M☉ the first of P06.T13's cooling fits; anything else none.
#[must_use]
pub(super) fn largest_radii_rsun(
    input: &BinaryInput,
    members: &[Member; 2],
    until_years: f64,
) -> [f64; 2] {
    let masses = input.masses();
    core::array::from_fn(|i| match &members[i] {
        Member::Track { track, offset } => {
            let at = super::star::engine_track_age_years(track, *offset, until_years);
            track.max_radius_until(Years::new(at)).value()
        }
        Member::Cooling { .. } => substellar::cooling(masses[i], Years::ZERO, input.composition())
            .map_or(0.0, |s| s.radius().value()),
        Member::Shaped { .. }
        | Member::MainSequence { .. }
        | Member::Frozen { .. }
        | Member::Remnant { .. }
        | Member::Gone => 0.0,
    })
}

/// The lobe test on the drawn orbit (design note 7): whether either star of the largest radii
/// `radii_rsun` ([`largest_radii_rsun`]) reaches its Roche lobe at the drawn orbit's periastron.
#[must_use]
pub(super) fn reaches_lobe(input: &BinaryInput, radii_rsun: [f64; 2]) -> bool {
    let [m1, m2] = input.masses().map(SolarMasses::value);
    lobe_reached_at(input.orbit().periapsis(), [m1 / m2, m2 / m1], radii_rsun)
}

/// The lobe test on `tested` (P11.T4.l, the orbit a passed-over pair's collapse leaves): whether
/// either star of the largest radii `radii_rsun` reaches its Roche lobe at the orbit's periastron,
/// at its lobe ratio ([`TestedOrbit::lobe_ratios`]).
#[must_use]
pub(super) fn reaches_lobe_on(tested: &TestedOrbit, radii_rsun: [f64; 2]) -> bool {
    lobe_reached_at(tested.elements.periapsis(), tested.lobe_ratios, radii_rsun)
}

/// Whether either star of the largest radii `radii_rsun` (R☉) reaches its Roche lobe at
/// `periastron`, star i at the mass ratio `ratios[i]`, its mass ÷ its companion's (Eggleton 1983).
#[must_use]
fn lobe_reached_at(periastron: Metres, ratios: [f64; 2], radii_rsun: [f64; 2]) -> bool {
    (0..2).any(|i| {
        let lobe = roche_lobe_radius(ratios[i], periastron).value() / SOLAR_RADIUS_M;
        radii_rsun[i] >= lobe
    })
}

/// The lobe part of [`can_interact`] alone: whether the first star has arrived by `until_age` and
/// the drawn orbit reaches the lobe test then, without the decay the engine's sinks can make
/// (plan 08's `interacting_periastron` is this test's boundary).
#[cfg(test)]
#[must_use]
pub(crate) fn lobe_reached(input: &BinaryInput, until_age: Years) -> bool {
    let until = until_age.value().max(0.0);
    let members = own_members(input, until, None);
    arrival(&members, until) < until
        && reaches_lobe(input, largest_radii_rsun(input, &members, until))
}

/// The primary's single-star death, which the engine may not move (plan 11, design note 16), with
/// the full track it was read from.
struct PinnedTrack {
    track: Arc<Track>,
    pin: Pin,
}

impl PinnedTrack {
    /// Where the pin's own track's main sequence ends, if it has one: the engine's stop of
    /// P11.T4.n ([`Engine::end_pinned_main_sequence`]).
    #[must_use]
    fn main_sequence_end(&self) -> Option<OwnMainSequenceEnd> {
        self.track
            .main_sequence_end()
            .map(|end| OwnMainSequenceEnd {
                track: Arc::clone(&self.track),
                age_years: end.value(),
            })
    }
}

/// A primary's death as plan 06 gives it: when, what it leaves, its kick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Pin {
    pub(super) death: Death,
    pub(super) remnant: CompactRemnant,
    pub(super) kick: Option<NatalKick>,
}

/// The pinned primary's own track (design note 16's, [`PinnedTrack`]'s) and the age its main
/// sequence ends on it, which the engine stops at (P11.T4.n, decision-observed-class-findings
/// §B.3): [`Engine::end_pinned_main_sequence`].
#[derive(Debug, Clone, PartialEq)]
pub(super) struct OwnMainSequenceEnd {
    track: Arc<Track>,
    /// The track's age at the end of its last main-sequence segment
    /// ([`Track::main_sequence_end`]), years: the engine's age there, the track placed at zero.
    age_years: f64,
}

/// The primary's single-star death if design note 16 pins it: a primary of `m_cc(Z) − 1` M☉ or
/// more (plan 11's `stripped_mark_min_mass`, ruling 93.3) whose death is a collapse, read as plan
/// 06's `StarModel` reads it (`Track::fate_with` its remnant draws, the companion-stripped mark,
/// and the kick law under the input's stellar rules).
#[must_use]
fn pinned_death(input: &BinaryInput, full: Option<Arc<Track>>) -> Option<PinnedTrack> {
    let m1 = input.masses()[0];
    if m1 < stripped_mark_min_mass(input.composition()) {
        return None;
    }
    let draws = &input.draws()[0];
    let track = full.unwrap_or_else(|| {
        Arc::new(Track::full_with(
            track_mass(m1),
            input.composition(),
            draws,
            input.track_options(),
        ))
    });
    let fate = track.fate_with(RemnantDraws::of(draws))?;
    if !fate.death.kind().is_sudden() {
        return None;
    }
    let law = StandardKickLaw::under(input.rules());
    let death = stripped_mark_applied_under(
        fate.death,
        draws,
        (track.initial_mass(), track.composition()),
        input.rules(),
    );
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

/// The age of the primary's pinned death, years, if design note 16 pins it ([`pinned_death`]),
/// for the tests' invariants.
#[cfg(test)]
#[must_use]
pub(super) fn pinned_death_age_years(input: &BinaryInput) -> Option<f64> {
    pinned_death(input, None).map(|pinned| pinned.pin.death.age().value())
}

/// The primary's pin ([`pinned_death`]) and the full track it is read from, for the tests that
/// start an engine at the pin.
#[cfg(test)]
#[must_use]
pub(super) fn pinned_track_and_pin(input: &BinaryInput) -> Option<(Arc<Track>, Pin)> {
    pinned_death(input, None).map(|pinned| (pinned.track, pinned.pin))
}

/// The pinned primary's own main-sequence end ([`PinnedTrack::main_sequence_end`]), for the tests
/// that start an engine before it.
#[cfg(test)]
#[must_use]
pub(super) fn own_main_sequence_end_of(input: &BinaryInput) -> Option<OwnMainSequenceEnd> {
    pinned_death(input, None).and_then(|pinned| pinned.main_sequence_end())
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
    pub(super) fn of(input: &BinaryInput, start: Years) -> Self {
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
    /// The pinned primary's own main sequence's end, until the engine reaches it (P11.T4.n).
    own_main_sequence_end: Option<OwnMainSequenceEnd>,
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
    /// The white dwarf each member's core is ([`CoreSource`]), where the member is a star whose
    /// core is a carbon–oxygen or oxygen–neon dwarf's: the giant such a dwarf
    /// swelled into (P11.T4's swollen dwarf), a merger product placed around one (P11.T4's dwarf
    /// limit, [`Product::Member`](super::common_envelope::Product::Member)), or what a strip of
    /// either leaves living. Its strip, its own death and a merger product it makes take that
    /// kind. Any other member's is [`CoreSource::Other`], and [`Engine::set_member`] resets it:
    /// the callers that keep the star (a merger, a strip that leaves a star, the swell) set it
    /// again.
    pub(super) dwarf_cores: [CoreSource; 2],
    /// The main-sequence lifetimes read last ([`Engine::main_sequence_lifetime`]).
    lifetimes: Lifetimes,
    /// The members' structures evaluated last ([`Engine::structure`]).
    structures: KeptStructures,
}

/// The main-sequence lifetimes the engine read last, by the star's kind and its mass, as
/// [`sse::main_sequence_lifetime`] gives them at the pair's composition.
///
/// A lifetime is a pure function of those, and the engine asks for it again at the same mass: a
/// detached step of a main-sequence member reads it for its rates and its limit at the step's
/// start, at its middle, and for its τ at its end, which is the next step's start; a step of
/// transfer for the donor's and the accretor's τ and their ends (some five reads at three masses a
/// detached step). So each is computed once and read back, bit for bit: 1.4% of the instructions
/// of the field census near the Sun (callgrind of 639 systems' cells, 2026-11-02, R06.T16.a's
/// cost).
#[derive(Debug, Default)]
struct Lifetimes {
    // A cell each, not one for the array, so that a read copies one entry and not all of them.
    kept: [Cell<Option<(bool, f64, f64)>>; LIFETIMES_KEPT],
    next: Cell<usize>,
}

/// The lifetimes [`Lifetimes`] keeps: the masses a step reads, a member's at its start, middle and
/// end, for both members.
const LIFETIMES_KEPT: usize = 6;

impl Lifetimes {
    /// [`sse::main_sequence_lifetime`], years, of a star of mass `m` (M☉), a helium one if
    /// `helium`, at the composition of `coeffs`, which must be the same at every call: the one
    /// kept, or computed and kept in place of the oldest.
    #[must_use]
    fn of(&self, coeffs: &sse::ZCoeffs, helium: bool, m: f64) -> f64 {
        if let Some((_, _, lifetime)) = self
            .kept
            .iter()
            .filter_map(Cell::get)
            .find(|&(h, kept, _)| h == helium && same(kept, m))
        {
            // The tests read every kept lifetime against its computation afresh.
            #[cfg(test)]
            assert!(
                same(sse::main_sequence_lifetime(coeffs, helium, m), lifetime),
                "a kept lifetime is its computation's"
            );
            return lifetime;
        }
        let lifetime = sse::main_sequence_lifetime(coeffs, helium, m);
        let next = self.next.get();
        self.kept[next].set(Some((helium, m, lifetime)));
        self.next.set((next + 1) % LIFETIMES_KEPT);
        lifetime
    }
}

/// Whether `a` and `b` are the same number: `total_cmp` is equal exactly where the bits are, so
/// a kept value is read back only for the very arguments it was computed at, not near ones.
#[must_use]
fn same(a: f64, b: f64) -> bool {
    a.total_cmp(&b).is_eq()
}

/// The members' structures the engine evaluated last, one a slot, each with what its evaluation
/// read ([`Engine::structure`]).
///
/// A member's structure is a pure function of its form and of the age, mass and τ asked of it, and
/// a step of stable transfer asks for the same ones again: the donor's after the step for its spin,
/// both for the accretor's events and for the check for a bare core, and both again at the next
/// step's start, some six evaluations a step of two structures (`Engine::run` spent 6% of the
/// field census near the Sun on them, 2026-11-02). So each is evaluated once and read back, bit for
/// bit, while the member keeps its form.
#[derive(Debug, Default)]
struct KeptStructures {
    kept: [RefCell<Option<KeptStructure>>; 2],
}

/// A structure [`KeptStructures`] keeps: what its evaluation read of the member, the age, mass and
/// τ it was asked at, and the structure.
#[derive(Debug)]
struct KeptStructure {
    form: EvaluatedForm,
    asked: [f64; 3],
    structure: Option<sse::Structure>,
}

/// What [`Member::evaluate`] reads of a member besides the age, mass and τ it is asked at, for the
/// forms worth keeping.
///
/// A kept track is held, so that no other can take its place at its address. Every field of a
/// member is named below, so that one added must be keyed or set aside here.
#[derive(Debug)]
enum EvaluatedForm {
    Track(Arc<Track>, f64),
    Shaped(Arc<Track>, f64),
    MainSequence(bool),
    Cooling(f64),
    Remnant(Phase, f64, Megayears),
}

impl EvaluatedForm {
    /// The form of `member`, or `None` for one whose structure costs nothing to evaluate again (a
    /// frozen star's, nothing's).
    #[must_use]
    fn of(member: &Member) -> Option<Self> {
        // The mass and τ paths are the engine's record, which `Member::evaluate` does not read.
        match member {
            Member::Track { track, offset } => Some(Self::Track(Arc::clone(track), *offset)),
            Member::Shaped {
                track,
                offset,
                mass: _,
            } => Some(Self::Shaped(Arc::clone(track), *offset)),
            Member::MainSequence {
                helium,
                mass: _,
                tau: _,
            } => Some(Self::MainSequence(*helium)),
            Member::Cooling { offset, mass: _ } => Some(Self::Cooling(*offset)),
            Member::Remnant {
                phase,
                birth,
                origin,
                mass: _,
            } => Some(Self::Remnant(*phase, *birth, *origin)),
            Member::Frozen {
                state: _,
                core_radius: _,
            }
            | Member::Gone => None,
        }
    }

    /// Whether this is the form of `member`.
    #[must_use]
    fn is_of(&self, member: &Member) -> bool {
        match (self, member) {
            (Self::Track(kept, kept_offset), Member::Track { track, offset })
            | (
                Self::Shaped(kept, kept_offset),
                Member::Shaped {
                    track,
                    offset,
                    mass: _,
                },
            ) => Arc::ptr_eq(kept, track) && same(*kept_offset, *offset),
            (
                Self::MainSequence(kept),
                Member::MainSequence {
                    helium,
                    mass: _,
                    tau: _,
                },
            ) => kept == helium,
            (Self::Cooling(kept), Member::Cooling { offset, mass: _ }) => same(*kept, *offset),
            (
                Self::Remnant(kept, kept_birth, kept_origin),
                Member::Remnant {
                    phase,
                    birth,
                    origin,
                    mass: _,
                },
            ) => {
                kept == phase
                    && same(*kept_birth, *birth)
                    && same(kept_origin.value(), origin.value())
            }
            (
                Self::Track(..)
                | Self::Shaped(..)
                | Self::MainSequence(_)
                | Self::Cooling(_)
                | Self::Remnant(..),
                Member::Track { .. }
                | Member::Shaped { .. }
                | Member::MainSequence { .. }
                | Member::Cooling { .. }
                | Member::Frozen { .. }
                | Member::Remnant { .. }
                | Member::Gone,
            ) => false,
        }
    }
}

impl KeptStructures {
    /// The structure of `member`, in `slot`, at `age` (years on the engine's clock) with `mass`
    /// (M☉) and `tau`: the one kept, or `evaluate`'s, kept in place of the slot's last.
    #[must_use]
    fn of(
        &self,
        slot: usize,
        member: &Member,
        (age, mass, tau): (f64, f64, f64),
        evaluate: impl FnOnce() -> Option<sse::Structure>,
    ) -> Option<sse::Structure> {
        let asked = [age, mass, tau];
        let cell = &self.kept[slot];
        let kept = cell.take();
        if let Some(hit) = &kept
            && hit.asked.iter().zip(asked).all(|(&a, b)| same(a, b))
            && hit.form.is_of(member)
        {
            let structure = hit.structure;
            cell.replace(kept);
            // The tests read every kept structure against its evaluation afresh, so that a field
            // `Member::evaluate` came to read that the form does not key could not go unseen.
            #[cfg(test)]
            assert_eq!(
                evaluate(),
                structure,
                "a kept structure is its evaluation's"
            );
            return structure;
        }
        let structure = evaluate();
        cell.replace(
            EvaluatedForm::of(member)
                .map(|form| KeptStructure {
                    form,
                    asked,
                    structure,
                })
                .or(kept),
        );
        structure
    }
}

impl Engine {
    /// The main-sequence lifetime, years, of a star of mass `m` (M☉), a helium one if `helium`, at
    /// the pair's composition: [`sse::main_sequence_lifetime`]'s, bit for bit, from [`Lifetimes`].
    #[must_use]
    pub(super) fn main_sequence_lifetime(&self, helium: bool, m: f64) -> f64 {
        self.lifetimes.of(self.ctx.coeffs(), helium, m)
    }

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
            own_main_sequence_end: None,
            capped: false,
            dt_hint: 0.0,
            contact_until: 0.0,
            overflow_onset: (0.0, 0.0),
            first_contact: None,
            in_common_envelope: false,
            lifetimes: Lifetimes::default(),
            structures: KeptStructures::default(),
            dwarf_cores: [CoreSource::Other; 2],
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
    ///
    /// Before each phase a pin still pending at the engine's age, to the resolution of its clock
    /// ([`clock_resolution_years`]), is acted on (P11.T4.k, ruling p11-t4k-faults of 2026-10-06):
    /// a backstop for any exit that a step landing on the pin takes before its stop, such as
    /// T4.g's strip, which wins that step ([`Engine::integrate`]) and leaves the pin no longer
    /// ahead. No phase passes a pin it has not acted on (debug-asserted). The pinned primary's own
    /// main-sequence end has the same backstop (P11.T4.n,
    /// [`Engine::own_main_sequence_end_due_now`]).
    pub(super) fn run(&mut self) {
        let mut events = 0_u32;
        while self.age < self.until && !self.capped {
            events += 1;
            if events > MAX_EVENTS {
                self.capped = true;
                break;
            }
            self.own_main_sequence_end_due_now();
            self.pin_due_now();
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

    /// Acts on the primary's pin if it is due at the engine's age, to the resolution of its clock
    /// ([`clock_resolution_years`], as [`Engine::own_death_now`] reads it): see [`Engine::run`].
    fn pin_due_now(&mut self) {
        let Some(pin) = &self.pin else {
            return;
        };
        let at = pin.death.age().value();
        let resolution = clock_resolution_years(self.age);
        debug_assert!(
            at >= self.age - resolution,
            "the primary's pin at {at} yr lies behind the engine at {} yr ({:?}; the primary a {:?} \
             of {:?}, drawn {:?})",
            self.age,
            self.kind,
            self.members[0].state_at(&self.ctx, 0, self.age).phase(),
            self.ctx.composition(),
            self.ctx.draws(0)
        );
        if (at - self.age).abs() <= resolution {
            self.pinned_collapse();
            if let SegmentKind::StableTransfer { donor } = self.kind {
                self.end_without_a_living_donor(donor.index());
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

    /// Member `i`'s structure at `age` with `mass` and `tau` (see [`Member::evaluate`]), bit for
    /// bit, from [`KeptStructures`].
    #[must_use]
    pub(super) fn structure(
        &self,
        i: usize,
        age: f64,
        mass: f64,
        tau: f64,
    ) -> Option<sse::Structure> {
        let member = &self.members[i];
        self.structures.of(i, member, (age, mass, tau), || {
            member.evaluate(&self.ctx, i, age, mass, tau)
        })
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
        if self.segments.len() + 1 >= segment_cap() {
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
    /// inside a phase. The new member's core is no dwarf's ([`Engine::dwarf_cores`]) unless the
    /// caller says so.
    pub(super) fn set_member(&mut self, i: usize, member: Member) {
        self.members[i] = member;
        self.dwarf_cores[i] = CoreSource::Other;
    }

    /// The kind a pair without Roche-lobe overflow is in: detached while bound, disrupted once
    /// disrupted, and merged otherwise.
    ///
    /// An orbit a death unbinds is labelled by the death itself,
    /// [`SegmentKind::Disrupted`] by the star that died (`Engine::explode`, and `Engine::die` for a
    /// white dwarf's birth, P11.T4.j's F1), and keeps that label whatever dies after, a member
    /// that leaves nothing included (a pinned primary's pair instability, which is no merger); with
    /// no orbit and no disruption before, the kind is [`SegmentKind::Merged`].
    #[must_use]
    pub(super) fn quiet_kind(&self) -> SegmentKind {
        if self.orbit.is_some() {
            SegmentKind::Detached
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

/// P11.T4.n: a pinned main sequence ends at its own (decision-observed-class-findings §B, which
/// supersedes ruling p11-t4k-faults' "recorded, not routed").
impl Engine {
    /// The engine with the pinned primary's own main-sequence end `end` as a stop, if it lies
    /// after the engine's age ([`Engine::end_pinned_main_sequence`]).
    #[must_use]
    pub(super) fn with_own_main_sequence_end(mut self, end: Option<OwnMainSequenceEnd>) -> Self {
        self.own_main_sequence_end = end.filter(|end| end.age_years > self.age);
        self
    }

    /// The age of the pinned primary's own main-sequence end, years, if it lies after `age` and
    /// member 0, of `phase` at `age`, holds a hydrogen main sequence that outlasts it: an event of
    /// the step from `age` ([`Stop::PinnedMainSequenceEnd`]).
    ///
    /// [`Stop::PinnedMainSequenceEnd`]: super::detached::Stop::PinnedMainSequenceEnd
    #[must_use]
    pub(super) fn own_main_sequence_end_after(
        &self,
        age: f64,
        phase: Option<Phase>,
    ) -> Option<f64> {
        let end = self.own_main_sequence_end.as_ref()?;
        (end.age_years > age && self.outlasts_its_own(end, age, phase)).then_some(end.age_years)
    }

    /// Whether member 0, of `phase` at the engine's age `age`, is a hydrogen main-sequence star
    /// whose main sequence outlasts the pin's own, which ends at `end`.
    ///
    /// It is one the binary carries ([`Member::MainSequence`]: a case-A donor, a rejuvenated
    /// accretor, a contact pair's star), or a star on a track whose main-sequence segment ends
    /// later (a merger's product, `common_envelope.rs`'s `main_sequence_star`). The segment is the
    /// one [`phase_ahead`] reads at `age`, which a track holds whole however far it is built, so
    /// the answer does not depend on the age the pair is run to. A primary on its own track ends
    /// its main sequence there by itself, at its own boundary.
    #[must_use]
    fn outlasts_its_own(&self, end: &OwnMainSequenceEnd, age: f64, phase: Option<Phase>) -> bool {
        phase == Some(Phase::MainSequence)
            && self.members[0].track().is_none_or(|(track, offset)| {
                let (_, segment_end, _) = phase_ahead(track, offset, age);
                offset + segment_end > end.age_years + clock_resolution_years(end.age_years)
            })
    }

    /// The pinned primary's own main sequence ends now (P11.T4.n, decision-observed-class-findings
    /// §B.3).
    ///
    /// If member 0 still holds a hydrogen main sequence that outlasts it
    /// ([`Engine::own_main_sequence_end_after`]), the segment closes here, an event, and the star
    /// goes on along the pin's own track from that end, on its closed forms at the mass the binary
    /// gives it ([`Member::Shaped`] at offset 0, as [`offset_for`] places it): the form a case-B
    /// donor already takes. A contact pair coalesces then, as a contact does when a star leaves its
    /// main sequence ([`Engine::contact`]; Hurley, Tout and Pols 2002 section 2.6.6, which
    /// coalesces every contact; Menon et al. 2021, MNRAS 507, 5013, section 2.2.3, whose massive
    /// contact pairs merge as a star leaves its main sequence).
    ///
    /// What follows is the engine's own: the radius at the star's mass (HPT section 7.1), the
    /// small-envelope perturbation (HPT section 6.3), a naked helium star once M ≤ Mc, and case AB
    /// in a transfer. The pin falls at the own track's death through the carried tie
    /// ([`Engine::pinned_collapse`]), with plan 06's age, remnant and kick unchanged.
    ///
    /// Without it, a main sequence the binary prolonged outlasted the pin, and the star collapsed
    /// from it, a hot, non-degenerate hydrogen core (decision-observed-class-findings §B.1). The
    /// prolonging is the τ-preserving rule of HPT section 7.1, which BSE (Hurley, Tout and Pols
    /// 2002, section 2.6) applies to a donor that loses mass (`rlof.rs`'s `aged_tau`), or for a
    /// merger's product BSE's equation 80, which starts it young. By the ruling's science check,
    /// real case-A main sequences extend by only about 5–10%, and the τ rule overstates it by an
    /// order of magnitude because real donors are overluminous for their mass (Sen et al. 2022,
    /// A&A 659, A98: about 7 times). The departures (plan 11's Risks, "P11.T4.n as built"): L steps
    /// at the switch, up by 0.77 dex for record 822C72E00000008C's donor; the core is the own
    /// track's, case-B-like, about twice a real case-A donor's; such donors most likely end as
    /// white dwarfs or in an electron-capture supernova after the pin (Wellstein, Langer and Braun
    /// 2001, A&A 369, 939, system 31: an oxygen–neon–magnesium white dwarf), design note 16's departure; and a
    /// case-A-stripped remnant is finding F13's.
    pub(super) fn end_pinned_main_sequence(&mut self) {
        let Some(end) = self.own_main_sequence_end.take() else {
            return;
        };
        let (mass, tau) = self.current(0);
        let phase = self
            .structure(0, self.age, mass, tau)
            .map(|s| s.state.phase());
        if !self.outlasts_its_own(&end, self.age, phase) {
            return;
        }
        self.close_segment();
        let offset = offset_for(self.age, end.age_years);
        self.set_member(
            0,
            Member::Shaped {
                track: end.track,
                offset,
                mass: Path::starting(self.age, mass),
            },
        );
        if self.kind == SegmentKind::Contact {
            self.contact_until = 0.0;
            self.mix();
        }
    }

    /// Acts on the pinned primary's own main-sequence end if it is due at the engine's age, to the
    /// resolution of its clock ([`clock_resolution_years`]), as [`Engine::pin_due_now`] acts on
    /// the pin.
    ///
    /// It is a backstop for a step that lands on the end and leaves its phase by another exit (a
    /// transfer that detaches, say). One the engine has passed is dropped: no phase passes it while
    /// member 0 holds a main sequence that outlasts it (debug-asserted).
    fn own_main_sequence_end_due_now(&mut self) {
        let Some(end) = &self.own_main_sequence_end else {
            return;
        };
        let at = end.age_years;
        let resolution = clock_resolution_years(self.age);
        if at > self.age + resolution {
            return;
        }
        if at >= self.age - resolution {
            self.end_pinned_main_sequence();
            return;
        }
        if cfg!(debug_assertions) {
            let (mass, tau) = self.current(0);
            let phase = self
                .structure(0, self.age, mass, tau)
                .map(|s| s.state.phase());
            debug_assert!(
                !self.outlasts_its_own(end, self.age, phase),
                "the pinned primary's own main sequence ended at {at} yr, behind the engine at {} \
                 yr ({:?}), with member 0 a {phase:?} of {mass} M☉ whose main sequence outlasts it",
                self.age,
                self.kind
            );
        }
        self.own_main_sequence_end = None;
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
    let resolution = clock_resolution_years(age);
    if end.is_finite() && end + offset - age <= resolution {
        let (next_start, next_end) = track.phase_span(end);
        return (next_start, next_end, track_age.max(end));
    }
    (start, end, track_age)
}

/// The resolution of the engine's clock at the engine's age `age_years`, years: 4 ε of the age (of
/// a year below one), within which two ages are the same instant to the engine. A step that starts
/// so near a boundary is at it ([`phase_ahead`]), and a death or the pin that falls so near the age
/// is now ([`Engine::own_death_now`], [`Engine::run`]'s backstop).
#[must_use]
pub(super) fn clock_resolution_years(age_years: f64) -> f64 {
    4.0 * f64::EPSILON * age_years.abs().max(1.0)
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
