//! The explosion-site hook: whether a primary's death falls in the interval a supernova remnant can
//! be seen in, when, and for a grid record where (plan 08, P08.T13).
//!
//! The brainstorm's "One shared test" (under "Supernova remnants") makes a layer-E system a
//! catalogue entry, a supernova remnant, if and only if its primary's death falls between the shell
//! window before now and the end of the clock window, and has the grid resolve that system to "no
//! such system", so that nothing is counted twice. This module is the grid side's half of it:
//!
//! - [`death_in_interval`] decides, from a primary's own marks, whether its death falls in the
//!   interval (−(`W_cap` + L + H), +H], and when. It is the first half of the one shared test.
//!   Every side that keeps or claims a recently dead system calls it on its own candidate's
//!   primary, and plan 09's `claims` is the second half (P09.T17).
//! - [`explosion_site`] is that predicate on a grid record's marks, with the site the record's
//!   epoch position moved along its velocity to the death (Design note 22's x + v T).
//! - `RECENT_DEATH_CLAIM` is the gate: the claim plan 03's carve-out hook passes each site to,
//!   `None` until plan 09's P09.T35 gives it, with its version bump. While it is `None` the hook
//!   asks for no site, so placement reads no tier of the predicate, and no generated output depends
//!   on this module yet (P08.T13.b).
//!
//! `W_cap` is plan 09's [`SHELL_WINDOW_CAP`] (4 Myr), the ceiling of every shell window; L is
//! [`LIGHT_CROSSING_L`] (2¹⁸ Julian years) and H is [`CLOCK_WINDOW_H`] (1,000 Julian years). So
//! the interval opens 4,263,144 Julian years (134,534,593,094,400 s) before the epoch, exclusive,
//! and closes at +H, inclusive, as P09.T17's does. Plan 09's `claims` cuts each window at its
//! environment's cap, never above `W_cap`, so this interval holds every death plan 09 can claim.
//!
//! # The three tiers
//!
//! [`stellar::lifetime`](crate::stellar::lifetime) costs 0.5–1 ms for a layer-E star (plan 06
//! measured 0.95 ms at 20 M☉, and 0.49 ms after its speed rounds), not the 5 µs plan 08's Design
//! note 28 assumed: only the fate table meets that, at about 1 µs. So no death age is computed
//! exactly unless two cheaper brackets on it leave the verdict open (main's ruling, 2026-10-15).
//! Each tier brackets the exact death age, and the predicate answers `None` at the first whose
//! whole bracket puts the death outside the interval:
//!
//! 1. **The mass bracket**, [`LifetimeBracket::shared`]'s [`bracket`](LifetimeBracket::bracket)
//!    (P08.T9.d), which reads no draw: the least and greatest `lifetime` at the mass over Z
//!    10⁻⁴–0.03, widened by a tenth, which P08.T9.d's slow test holds over 10⁵ random stars with η
//!    within ±3 and either stripped mark. It answers for 2.5–150 M☉, and is skipped for a
//!    composition with a helium excess, which shortens a life and which the bracket is built
//!    without (no grid star has one; plan 09's members may). It decides nearly every layer-E
//!    record, since they are old and long dead.
//! 2. **The fate table** at the star's composition and η, [`FittedFates::fate_fitted`], where it
//!    answers and the companion-stripped mark cannot move the fate ([`stripped_mark_matters`],
//!    whose draws are read only inside [`STRIPPED_WINDOW`]). With d its death age and m the range
//!    brief's margin, max([`STATED_FACTOR`] × the age's bound, [`DEAD_MARGIN_FLOOR`]), the bracket
//!    is d ÷ (1 + m) to d ÷ (1 − m): the table's bound is relative to the exact value
//!    (`hyperion-fit`'s `stellar_fates`, its error's column 0), and that bracket is what it
//!    implies.
//! 3. **The exact lifetime**: `stellar::lifetime` at the track mass (held to 150 M☉, as
//!    [`StarModel`](crate::stellar::system::StarModel) holds it), the composition and the full
//!    draws. Its death time is [`SystemStars::death_time`]'s, bit for bit.
//!
//! The comparisons are in the clock's own [`UniverseTime`], never in `f64` years: each bracket end
//! is converted to a clock time as the death itself is, and an end the clock cannot hold lies
//! beyond the interval on its own side.
//!
//! # Their standing
//!
//! The brackets are validated, not proved: the fate table by plan 06's own record (P06.T38.d), and
//! the mass bracket by its 10⁵-star test. No proof is to be had at this cost, since the single-star
//! lifetime is piecewise, with changes of route and wind knots. So the carve-out's correctness does
//! not rest on them: the brainstorm's "nothing is counted twice" holds because every side decides
//! by this one function on the same marks, whatever the bounds' errors (main's ruling, 2026-10-15).
//! A bound that fails puts a recent death outside the interval. The system then stays a grid star
//! without a shell, and the catalogue drops the same candidate, so it costs a shell, never a double
//! count; the soundness test bounds that rate below 3 × 10⁻⁴ per death in the interval, at 95%.
//! From P09.T35's bump on, the predicate's inputs belong to the generator version: the fate
//! tables, [`LifetimeBracket`] and its widening, [`STATED_FACTOR`] and [`DEAD_MARGIN_FLOOR`].
//!
//! Nothing here draws or caches anything. The predicate reads one lazy value,
//! [`LifetimeBracket::shared`], whose bits are [`LifetimeBracket::new`]'s whoever reads it first
//! (ruling 128.5, under the sim-determinism skill's exception for lazy constants), and a site reads
//! only the record's existing draws, each a keyed stream opened fresh.

use std::cell::LazyCell;

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::displaced::marks::LifetimeBracket;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::epoch_velocity;
use crate::galaxy::snr::SHELL_WINDOW_CAP;
use crate::stellar::brief::{DEAD_MARGIN_FLOOR, STATED_FACTOR};
use crate::stellar::draws::{StandardNormal, StarDraws};
use crate::stellar::fates::{FittedFate, FittedFates, STRIPPED_WINDOW, stripped_mark_matters};
use crate::stellar::sse::MIN_INITIAL_MASS;
#[cfg(doc)]
use crate::stellar::system::SystemStars;
use crate::stellar::system::{
    clock_time_of_death, draw_metallicity, primary_draws, primary_eta, track_mass_of,
};
use crate::stellar::{Composition, lifetime};
use crate::time::{CLOCK_WINDOW_H, ClockWindow, LIGHT_CROSSING_L, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Seconds, SolarMasses, Years};

/// Where and when a primary died, or will die, inside the interval (−(`W_cap` + L + H), +H] (plan
/// 08, P08.T13): what plan 09's supernova test reads of an explosion (P09.T17).
///
/// The position is in the galactic frame. For a grid record it is the epoch position moved along
/// the record's epoch velocity to the death, by straight line ([`explosion_site`]). Plan 09's
/// catalogue candidates and feature members build their own with [`ExplosionSite::new`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::displaced::ExplosionSite;
/// use hyperion_sim::time::UniverseTime;
///
/// let place = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in the cube")?;
/// let died = UniverseTime::from_julian_years(-20_000).ok_or("on the clock")?;
/// let site = ExplosionSite::new(place, died);
/// assert_eq!(site.position(), &place);
/// assert!(site.death() < UniverseTime::EPOCH);
/// # Ok::<(), &str>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExplosionSite {
    position: GalacticPosition,
    death: UniverseTime,
}

impl ExplosionSite {
    /// The site at `position` of a death at `death`. Nothing is checked: a site is where and when a
    /// primary died, and the interval is the carve-out's, so a caller builds one for a death
    /// [`death_in_interval`] has found inside it, as [`explosion_site`] does.
    #[must_use]
    pub const fn new(position: GalacticPosition, death: UniverseTime) -> Self {
        Self { position, death }
    }

    /// Where the primary died or will die, in the galactic frame.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// When the primary died or will die: for a site of [`explosion_site`], or one built for a
    /// death [`death_in_interval`] found, a clock time in (−(`W_cap` + L + H), +H].
    #[must_use]
    pub const fn death(&self) -> UniverseTime {
        self.death
    }
}

/// The clock time of a primary's death if it falls in the interval (−(`W_cap` + L + H), +H], and
/// `None` otherwise: the first half of the brainstorm's one shared test (plan 08, P08.T13).
///
/// The primary is given by its marks: its initial mass `m0` (M☉), its `composition`, its Reimers η
/// `eta`, its age at the epoch `age_at_epoch` (Julian years since its onset of collapse) and its
/// full draws, which `draws` makes. The death time is the clock's EPOCH + (lifetime − age at the
/// epoch) × 31,557,600 s, floored to the nanosecond, the time
/// [`SystemStars::death_time`] gives for the same marks, bit for bit. `None` also below 0.1 M☉,
/// where nothing dies, and where the clock cannot hold the death.
///
/// It decides in the module's three tiers, cheapest first: the mass bracket, the fate table and
/// the exact lifetime. The brackets are validated, not proved, so a `None` can be wrong at a rate
/// the soundness test bounds below 3 × 10⁻⁴ per death in the interval; a `Some` never is. That is
/// why every side of the shared test calls this one function and none tests a death against the
/// interval by another route: the grid's carve-out through [`explosion_site`], and plan 09's field
/// candidates, runaways, walkaways and feature members on their own primaries' marks. A candidate
/// whose death was drawn inside the interval but which this puts outside is dropped by the
/// catalogue, because the grid keeps that system.
///
/// `draws` is called at most once, and only where the first two tiers leave the verdict open or
/// the stripped mark must be read (`m0` inside [`STRIPPED_WINDOW`] with the table answering). It
/// must give `eta` as its [`eta`](StarDraws::eta), bit for bit, as the stellar stage's
/// `primary_eta` gives a grid record's.
///
/// # Panics
///
/// In debug builds, if the draws' η is not `eta`.
///
/// # Examples
///
/// Whether a 20 M☉ star of solar composition is a recent supernova: dead two million years, it
/// is; dead a billion, it is not, and nothing but its mass and age is read to say so.
///
/// ```
/// use hyperion_sim::galaxy::displaced::death_in_interval;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::{Composition, lifetime};
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let (m0, solar, draws) = (SolarMasses::new(20.0), Composition::SOLAR, StarDraws::median());
/// let life = lifetime(m0, &solar, &draws).value();
/// let dead_two_myr = Years::new(life + 2e6);
/// let recent = death_in_interval(m0, &solar, draws.eta(), dead_two_myr, || draws.clone());
/// assert!(recent.is_some_and(|death| death < UniverseTime::EPOCH));
///
/// let long_dead = death_in_interval(m0, &solar, draws.eta(), Years::new(1e9), || {
///     unreachable!("the mass bracket decides a star a billion years old")
/// });
/// assert_eq!(long_dead, None);
/// ```
#[must_use]
pub fn death_in_interval(
    m0: SolarMasses,
    composition: &Composition,
    eta: StandardNormal,
    age_at_epoch: Years,
    draws: impl FnOnce() -> StarDraws,
) -> Option<UniverseTime> {
    decide(
        m0,
        age_at_epoch,
        &given_marks(composition, eta, draws),
        lifetime,
    )
}

/// Where and when the grid record `record`'s primary died or will die, if that falls in the
/// interval (−(`W_cap` + L + H), +H], and `None` otherwise (plan 08, P08.T13).
///
/// It is [`death_in_interval`]'s decision on the record's own marks: its initial mass, its
/// [`draw_metallicity`], its primary's η and draws (the stellar stage's seams `primary_eta` and
/// `primary_draws`, which P08.T12.c moves to the record's mark attempt) and its age at the epoch.
/// Each mark is read only when a tier needs it, so a record the mass bracket decides, nearly
/// every one, opens no stream: the metallicity and η cost three quarters of the hook for such a
/// record, where they are not needed (P08.T13's measurement). The grid's metallicity draw gives
/// no helium excess, so the mass bracket asks it nothing.
///
/// Where that gives a death at T, the site is the epoch position x moved by the epoch velocity v
/// ([`epoch_velocity`]) over T, by straight line: x + v T (Design note 22), which for a remnant
/// already dead backs it out to where it exploded. The velocity is drawn only then, and a galaxy
/// built without its kinematic tables has none, so its sites are the epoch positions.
///
/// Once plan 09 supplies the claim (P09.T35, which sets `RECENT_DEATH_CLAIM`), plan 03's carve-out
/// hook calls this for every accepted layer-E record and passes the site to the claim; until then
/// the hook calls it for none, and nothing generated reads a site (P08.T13.b). It is a pure
/// function of the galaxy and the record: `resolve`, `generate_cell` and plan 09 get the same
/// answer in any order.
///
/// # Panics
///
/// - In a galaxy with its kinematic tables, if `record` is not a grid record, as
///   [`epoch_velocity`] does; and as [`draw_metallicity`] does, for a record of another galaxy.
/// - In debug builds, as [`death_in_interval`] does, and if the metallicity draw, where it is read,
///   gives a helium excess.
///
/// # Examples
///
/// Nearly every layer-E star is long dead or far from death, so a cell at the solar circle has
/// few sites if any. Re-aged to have died about two million years ago, a record has one: the
/// metallicity draw reads the age, so the lifetime moves a little with it, but not out of the
/// interval.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::displaced::explosion_site;
/// use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::system::{ClockDeath, SystemStars};
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Years;
///
/// // Without its kinematic tables: a cheap galaxy, whose records do not move.
/// let galaxy = Galaxy::new(Seed::new(19));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::E, [0, 203, 0])?, &mut cell);
/// let record = cell.first().ok_or("the cell holds a system")?;
///
/// let ClockDeath::At(death, _) = SystemStars::generate(&galaxy, record).death_time() else {
///     return Err("a layer-E primary dies on the clock".into());
/// };
/// let lifetime = death.since_epoch().as_julian_years_f64() + record.age_at_epoch().value();
/// let dying = SystemRecord::from_parts(
///     record.id(),
///     *record.epoch_position(),
///     record.origin(),
///     record.population(),
///     record.primary_initial_mass(),
///     Years::new(lifetime + 2e6),
/// );
/// let site = explosion_site(&galaxy, &dying).ok_or("a death 2 Myr ago is recent")?;
/// assert!(site.death() < UniverseTime::EPOCH);
/// assert_eq!(site.position(), dying.epoch_position());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn explosion_site(galaxy: &Galaxy, record: &SystemRecord) -> Option<ExplosionSite> {
    let marks = Marks {
        composition: LazyCell::new(|| {
            let composition = draw_metallicity(galaxy, record);
            debug_assert!(
                composition.helium_excess().value() <= 0.0,
                "a grid record's metallicity draw gives no helium excess: {:?}",
                record.id()
            );
            composition
        }),
        eta: LazyCell::new(|| primary_eta(galaxy, record)),
        draws: LazyCell::new(|| primary_draws(galaxy, record)),
        helium_free: true,
    };
    let death = decide(
        record.primary_initial_mass(),
        record.age_at_epoch(),
        &marks,
        lifetime,
    )?;
    let elapsed = Seconds::new(death.since_epoch().as_seconds_f64());
    let position = record
        .epoch_position()
        .translated(epoch_velocity(galaxy, record).displacement_over(elapsed))
        .expect(
            "a grid record inside the root cube moving under 3,000 km/s for at most 4.27 Myr \
             travels under 43,000 ly, which leaves the addressable range nowhere",
        );
    Some(ExplosionSite::new(position, death))
}

/// A recent-death claim: whether a catalogue class claims the grid record, whose primary dies at
/// the site inside the interval (plan 08, P08.T13.b).
pub(crate) type RecentDeathClaim = fn(&Galaxy, &ExplosionSite, &SystemRecord) -> bool;

/// The claim plan 03's carve-out hook passes each layer-E record's [`explosion_site`] to: `None`
/// until plan 09's P09.T35 gives it (plan 08, P08.T13.b).
///
/// P09.T35 writes `recent_death_claims(galaxy, &site, &record)`: the record's [`SystemStars`],
/// `DeathMarks::of`, then `claims(galaxy, &site, &marks, ShellEnvironment::Field)`. It sets this to
/// `Some(recent_death_claims)` in its bump commit. While it is `None` the hook asks for no site, so
/// placement reads no tier of the predicate: neither [`LifetimeBracket::shared`], nor the fate
/// table, nor `lifetime`.
///
/// It is one item on purpose: giving a claim and switching the hook on are the same edit (main's
/// ruling on P08.T13's cost, 2026-10-15). A flag beside a stub's body would be two edits, and a
/// claim written with the flag left off would keep on the grid every recent death the catalogue
/// claims, a silent double count.
pub(crate) const RECENT_DEATH_CLAIM: Option<RecentDeathClaim> = None;

/// The predicate's core: [`death_in_interval`] of a primary of initial mass `m0` and age at the
/// epoch `age_at_epoch`, whose other `marks` are read only where a tier needs them, with the exact
/// tier's death age given by `exact_death_age` of the track mass, the composition and the draws.
///
/// Every side of the shared test decides through this one function: [`death_in_interval`] with
/// the marks given and `stellar::lifetime`, [`explosion_site`] with a grid record's marks made
/// lazily, the soundness test with its own death age; and P08.T12.c will pass the lifetime it has
/// already computed for a layer-E record (Design note 19) rather than compute it again.
#[must_use]
fn decide<C, E, D>(
    m0: SolarMasses,
    age_at_epoch: Years,
    marks: &Marks<C, E, D>,
    exact_death_age: impl FnOnce(SolarMasses, &Composition, &StarDraws) -> Years,
) -> Option<UniverseTime>
where
    C: FnOnce() -> Composition,
    E: FnOnce() -> StandardNormal,
    D: FnOnce() -> StarDraws,
{
    if !(MIN_INITIAL_MASS.value()..).contains(&m0.value()) {
        count(Verdict::NoDeath);
        return None;
    }
    let start = interval_start();
    // 1. The mass bracket, built without a helium excess.
    if (marks.helium_free || LazyCell::force(&marks.composition).helium_excess().value() <= 0.0)
        && let Some(bracket) = LifetimeBracket::shared().bracket(m0)
        && outside(start, age_at_epoch, bracket)
    {
        count(Verdict::Bracket);
        return None;
    }
    // 2. The fate table, which holds unstripped stars.
    let eta = *LazyCell::force(&marks.eta);
    if let Some(fate) =
        FittedFates::generator().fate_fitted(m0, LazyCell::force(&marks.composition), eta)
    {
        let (lo, hi) = STRIPPED_WINDOW;
        let stripped = (lo..=hi).contains(&m0.value())
            && stripped_mark_matters(
                m0,
                LazyCell::force(&marks.composition),
                with_eta(LazyCell::force(&marks.draws), eta),
            );
        if !stripped && outside(start, age_at_epoch, table_bracket(&fate)) {
            count(Verdict::Table);
            return None;
        }
    }
    // 3. The exact death age.
    let death_age = exact_death_age(
        track_mass_of(m0),
        LazyCell::force(&marks.composition),
        with_eta(LazyCell::force(&marks.draws), eta),
    );
    let death = clock_time_of_death(age_at_epoch, death_age).filter(|&t| within(start, t));
    count(if death.is_some() {
        Verdict::ExactInside
    } else {
        Verdict::ExactOutside
    });
    death
}

/// `draws`, which must carry the primary's η `eta`, bit for bit: checked in debug builds.
#[must_use]
fn with_eta(draws: &StarDraws, eta: StandardNormal) -> &StarDraws {
    debug_assert!(
        draws.eta().value().total_cmp(&eta.value()).is_eq(),
        "the draws' η {:?} is not the primary's η, {eta:?}",
        draws.eta()
    );
    draws
}

/// The fate table's bracket on the exact death age, [d ÷ (1 + m), d ÷ (1 − m)] with d the table's
/// death age and m [`table_margin`]; its upper end is infinite for a margin of 1 or more.
#[must_use]
fn table_bracket(fate: &FittedFate) -> (Years, Years) {
    let margin = table_margin(fate);
    let d = fate.death_age.value();
    let hi = if margin < 1.0 {
        d / (1.0 - margin)
    } else {
        f64::INFINITY
    };
    (Years::new(d / (1.0 + margin)), Years::new(hi))
}

/// The range brief's margin on the fate table's death age, relative: max([`STATED_FACTOR`] × the
/// age's validated bound, [`DEAD_MARGIN_FLOOR`]).
#[must_use]
fn table_margin(fate: &FittedFate) -> f64 {
    (STATED_FACTOR * fate.bounds.0).max(DEAD_MARGIN_FLOOR)
}

/// Whether every death age in `bracket` (least, greatest) puts the death of a star whose age at
/// the epoch is `age_at_epoch` outside the interval that opens at `start` ([`interval_start`]):
/// all at or before `start`, or all after +H.
///
/// The clock time is non-decreasing in the death age, so the ends decide.
#[must_use]
fn outside(start: UniverseTime, age_at_epoch: Years, (least, greatest): (Years, Years)) -> bool {
    let all_before = match clock_time_of_death(age_at_epoch, greatest) {
        Some(t) => t <= start,
        // Off the clock: before it if the death age is below the age at the epoch.
        None => greatest.value() < age_at_epoch.value(),
    };
    all_before
        || match clock_time_of_death(age_at_epoch, least) {
            Some(t) => t > INTERVAL_END,
            None => least.value() > age_at_epoch.value(),
        }
}

/// The interval's start, −(`W_cap` + L + H), which it excludes: 134,534,593,094,400 s before the
/// epoch, 4,263,144 Julian years.
#[must_use]
fn interval_start() -> UniverseTime {
    // 4 × 10⁶ × 31,557,600 s = 1.262304 × 10¹⁴ s, an integer an f64 holds exactly.
    let cap = Span::from_seconds_f64(SHELL_WINDOW_CAP.value() * SECONDS_PER_JULIAN_YEAR)
        .expect("the shell window's cap is a few million years");
    cap.checked_add(LIGHT_CROSSING_L)
        .and_then(|span| span.checked_add(CLOCK_WINDOW_H))
        .and_then(|span| UniverseTime::EPOCH.checked_sub(span))
        .expect("4.3 Myr of seconds is far inside the clock's range")
}

/// The interval's end, +H, which it includes.
const INTERVAL_END: UniverseTime = ClockWindow::END;

/// Whether the clock time `t` lies in the interval (−(`W_cap` + L + H), +H]: the tests' predicate
/// on T alone.
#[cfg(test)]
#[must_use]
fn in_interval(t: UniverseTime) -> bool {
    within(interval_start(), t)
}

/// Whether the clock time `t` lies in the interval that opens at `start`, [`interval_start`].
#[must_use]
fn within(start: UniverseTime, t: UniverseTime) -> bool {
    start < t && t <= INTERVAL_END
}

/// What the predicate reads of a primary besides its initial mass and age, each mark made only
/// when a tier first reads it.
struct Marks<C, E, D> {
    /// The composition.
    composition: LazyCell<Composition, C>,
    /// Reimers η: [`StarDraws::eta`] of the draws, bit for bit.
    eta: LazyCell<StandardNormal, E>,
    /// The full draws.
    draws: LazyCell<StarDraws, D>,
    /// Whether the composition is known to carry no helium excess before it is read, as a grid
    /// record's metallicity draw gives none; otherwise the mass bracket reads the composition.
    helium_free: bool,
}

/// The marks of [`death_in_interval`]: the composition and η given, the draws made by `draws`.
#[must_use]
fn given_marks(
    composition: &Composition,
    eta: StandardNormal,
    draws: impl FnOnce() -> StarDraws,
) -> Marks<impl FnOnce() -> Composition, impl FnOnce() -> StandardNormal, impl FnOnce() -> StarDraws>
{
    let composition = *composition;
    Marks {
        composition: LazyCell::new(move || composition),
        eta: LazyCell::new(move || eta),
        draws: LazyCell::new(draws),
        helium_free: false,
    }
}

/// What decided a verdict of [`death_in_interval`], counted in a test build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Below 0.1 M☉: nothing dies.
    NoDeath,
    /// Tier 1, the mass bracket: outside.
    Bracket,
    /// Tier 2, the fate table: outside.
    Table,
    /// Tier 3, the exact death age: inside.
    ExactInside,
    /// Tier 3, the exact death age: outside, or off the clock.
    ExactOutside,
}

/// How many verdicts each tier gave on this thread, in a test build (plan 08, P08.T13's counts).
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Verdicts {
    /// Below 0.1 M☉.
    pub(crate) no_death: u64,
    /// Tier 1, outside.
    pub(crate) bracket: u64,
    /// Tier 2, outside.
    pub(crate) table: u64,
    /// Tier 3, inside.
    pub(crate) exact_inside: u64,
    /// Tier 3, outside or off the clock.
    pub(crate) exact_outside: u64,
}

#[cfg(test)]
impl Verdicts {
    /// Every verdict counted.
    #[must_use]
    pub(crate) fn total(&self) -> u64 {
        self.no_death + self.bracket + self.table + self.exact_inside + self.exact_outside
    }

    /// The exact tier's verdicts.
    #[must_use]
    pub(crate) fn exact(&self) -> u64 {
        self.exact_inside + self.exact_outside
    }

    /// The shares of the three tiers, in order, as one line for a test's report.
    #[must_use]
    pub(crate) fn shares(&self) -> String {
        let total = self.total().max(1);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a test's counts are far below 2⁵³"
        )]
        let share = |n: u64| n as f64 / total as f64;
        format!(
            "{} verdicts: mass bracket {} ({:.4}), fate table {} ({:.4}), exact {} ({:.4}, {} \
             inside)",
            self.total(),
            self.bracket,
            share(self.bracket),
            self.table,
            share(self.table),
            self.exact(),
            share(self.exact()),
            self.exact_inside,
        )
    }
}

#[cfg(test)]
thread_local! {
    /// The verdicts counted on this thread: a test's counter, never a process-wide one, since tests
    /// run on threads of one process.
    static VERDICTS: std::cell::Cell<Verdicts> = const {
        std::cell::Cell::new(Verdicts {
            no_death: 0,
            bracket: 0,
            table: 0,
            exact_inside: 0,
            exact_outside: 0,
        })
    };
}

/// The verdicts counted on this thread since the last [`take_verdicts`], which it resets.
#[cfg(test)]
pub(crate) fn take_verdicts() -> Verdicts {
    VERDICTS.with(|cell| cell.replace(Verdicts::default()))
}

/// Counts one verdict on this thread.
#[cfg(test)]
fn count(verdict: Verdict) {
    VERDICTS.with(|cell| {
        let mut v = cell.get();
        match verdict {
            Verdict::NoDeath => v.no_death += 1,
            Verdict::Bracket => v.bracket += 1,
            Verdict::Table => v.table += 1,
            Verdict::ExactInside => v.exact_inside += 1,
            Verdict::ExactOutside => v.exact_outside += 1,
        }
        cell.set(v);
    });
}

/// Counts nothing outside a test build.
#[cfg(not(test))]
fn count(_: Verdict) {}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::Seed;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellCache, CellKey, generate_cell, resolve};
    use crate::galaxy::query::{RangeQuery, position_at, range_query};
    use crate::id::{BodyId, Layer, SystemIdKind};
    use crate::math;
    use crate::rng::{ObjectKey, Stream, tags};
    use crate::stellar::draws::StarDrawsParts;
    use crate::stellar::system::{ClockDeath, SystemStars};
    use crate::units::{Dex, HeliumExcess, LightYears};

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).unwrap()
    }

    fn span_of_years(years: f64) -> Span {
        Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR).unwrap()
    }

    // --- The interval ---

    /// The interval's ends, as the clock holds them: −(`W_cap` + L + H) out and a second later in,
    /// +H in and a second later out; and its start is the Type Ia class's `SHELL_WINDOW_CAP + L +
    /// H` before the epoch.
    #[test]
    fn the_interval_is_open_below_and_closed_above() {
        let start = interval_start();
        assert_eq!(start, UniverseTime::new(-134_534_593_094_400, 0).unwrap());
        let (second, nanosecond) = (Span::from_seconds(1), Span::new(0, 1).unwrap());
        assert!(!in_interval(start));
        assert!(in_interval(start.checked_add(nanosecond).unwrap()));
        assert!(in_interval(start.checked_add(second).unwrap()));
        assert!(in_interval(UniverseTime::EPOCH));
        assert!(in_interval(ClockWindow::END));
        assert!(!in_interval(
            ClockWindow::END.checked_add(nanosecond).unwrap()
        ));
        assert!(!in_interval(ClockWindow::END.checked_add(second).unwrap()));
        // `catalogue_classes::type_ia`'s `before_interval`, which this agrees with.
        let before = SHELL_WINDOW_CAP.value()
            + LIGHT_CROSSING_L.as_julian_years_f64()
            + CLOCK_WINDOW_H.as_julian_years_f64();
        assert_same_bits(-before, start.since_epoch().as_julian_years_f64());
    }

    /// A bracket decides only when the whole of it is on one side of the interval.
    #[test]
    fn a_bracket_decides_only_from_one_side() {
        let age = Years::new(2e7);
        let life = |years: f64| Years::new(age.value() + years);
        // Dead 5 Myr at its longest: before the interval.
        assert!(outside(interval_start(), age, (life(-6e6), life(-5e6))));
        // Dying in 1,500 years at its soonest: after it.
        assert!(outside(interval_start(), age, (life(1_500.0), life(1e6))));
        // Straddling either end, or inside: open.
        assert!(!outside(interval_start(), age, (life(-5e6), life(-4e6))));
        assert!(!outside(
            interval_start(),
            age,
            (life(500.0), life(2_000.0))
        ));
        assert!(!outside(interval_start(), age, (life(-1e3), life(1e3))));
        // An infinite upper end lies after the interval, so only the lower end can decide.
        assert!(!outside(
            interval_start(),
            age,
            (life(-1e3), Years::new(f64::INFINITY))
        ));
        assert!(outside(
            interval_start(),
            age,
            (life(2e3), Years::new(f64::INFINITY))
        ));
    }

    // --- The soundness test (main's ruling, 2026-10-15) ---

    /// The soundness test's seed.
    const SOUNDNESS_SEED: u64 = 0x0813_5000;

    /// The fixed η a few of the soundness test's primaries take: the fate table's top panel answers
    /// to ±2.4, the mass bracket's test holds ±3, and ±4 is beyond both.
    const FIXED_ETAS: [f64; 6] = [2.4, -2.4, 3.0, -3.0, 4.0, -4.0];

    /// The ages at which the soundness test puts each primary's death.
    const SOUNDNESS_AGES: u64 = 11;

    /// The soundness test's ages that put a death inside the interval.
    const SOUNDNESS_AGES_INSIDE: u64 = 3;

    /// A synthetic primary of the soundness test.
    struct Primary {
        m0: SolarMasses,
        composition: Composition,
        draws: StarDraws,
    }

    /// `n` synthetic primaries on the soundness test's stream: m₀ log-uniform on 8–150 M☉, every
    /// third on 8–11 M☉, the stripped window; \[Fe/H\] uniform over Z 10⁻⁴–0.03 as
    /// `tests/galaxy_class_table.rs`'s `bracket_holds` draws it, but one in thirty below that and
    /// one in thirty above; and the draws of a synthetic ID, so that η and the stripped mark are
    /// random, but for one in fifty at a fixed η. No helium excess.
    fn primaries(n: u32) -> Vec<Primary> {
        let seed = Seed::new(SOUNDNESS_SEED);
        let mut stream = Stream::open(seed, tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let key = CellKey::new(Layer::E, [0, 0, 0]).unwrap();
        let (fe_lo, fe_hi) = (math::log10(0.005), math::log10(1.5));
        (0..n)
            .map(|i| {
                let top = if i % 3 == 0 { 11.0 } else { 150.0 };
                let m0 = 8.0 * math::exp(stream.uniform() * math::ln(top / 8.0));
                let u = stream.uniform();
                let fe_h = match i % 30 {
                    7 => fe_lo - 1.5 * (1.0 - u),
                    22 => fe_hi + 0.5 * (1.0 - u),
                    _ => fe_lo + u * (fe_hi - fe_lo),
                };
                let id = key.candidate_id(i).unwrap();
                let mut draws = StarDraws::for_star(seed, BodyId::new(id, 0));
                if i % 50 == 11 {
                    let eta = FIXED_ETAS[usize::try_from(i / 50).unwrap() % FIXED_ETAS.len()];
                    draws = StarDraws::from_parts(StarDrawsParts {
                        eta: StandardNormal::new(eta).unwrap(),
                        ..draws.parts().clone()
                    });
                }
                Primary {
                    m0: SolarMasses::new(m0),
                    composition: Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
                    draws,
                }
            })
            .collect()
    }

    /// The least age at the epoch at which a star of death age `death_age` has died by `target`:
    /// its death's clock time is the latest the ages' `f64` steps reach that is not after `target`,
    /// within a quarter of a second of it for a layer-E star.
    fn age_dying_by(death_age: Years, target: UniverseTime) -> Years {
        let t = |age: f64| clock_time_of_death(Years::new(age), death_age).unwrap();
        let mut age =
            death_age.value() - target.since_epoch().as_seconds_f64() / SECONDS_PER_JULIAN_YEAR;
        while t(age) > target {
            age = age.next_up();
        }
        while t(age.next_down()) <= target {
            age = age.next_down();
        }
        Years::new(age)
    }

    /// The soundness test over `n` primaries. At ages that put each death at the interval's ends
    /// and a second inside them, at its middle, and outside it by half and twice the fate table's
    /// margin and by twice the mass bracket's width, the predicate's core answers `Some` exactly
    /// inside, with the conversion of the exact death age as its time. The exact age is passed to
    /// the core, so that no lifetime is computed twice. Returns the tiers' verdicts.
    fn soundness(n: u32) -> Verdicts {
        let start = interval_start();
        let middle =
            UniverseTime::new(start.seconds().midpoint(INTERVAL_END.seconds()), 0).unwrap();
        let second = Span::from_seconds(1);
        let mut dropped = Vec::new();
        take_verdicts();
        for p in primaries(n) {
            let death_age = lifetime(track_mass_of(p.m0), &p.composition, &p.draws);
            let eta = p.draws.eta();
            let margin = FittedFates::generator()
                .fate_fitted(p.m0, &p.composition, eta)
                .map_or(DEAD_MARGIN_FLOOR, |fate| table_margin(&fate));
            let md = margin * death_age.value();
            let (lo, hi) = LifetimeBracket::shared().bracket(p.m0).unwrap();
            let width = hi.value() - lo.value();
            let before = |years: f64| start.checked_sub(span_of_years(years)).unwrap();
            let after = |years: f64| INTERVAL_END.checked_add(span_of_years(years)).unwrap();
            let targets = [
                (start, false),
                (start.checked_add(second).unwrap(), true),
                (middle, true),
                (INTERVAL_END, true),
                (INTERVAL_END.checked_add(second).unwrap(), false),
                (before(0.5 * md), false),
                (before(2.0 * md), false),
                (after(0.5 * md), false),
                (after(2.0 * md), false),
                (before(2.0 * width), false),
                (after(2.0 * width), false),
            ];
            for (target, inside) in targets {
                let age = age_dying_by(death_age, target);
                let t = clock_time_of_death(age, death_age).unwrap();
                assert_eq!(in_interval(t), inside, "the ages put {t} against {target}");
                let expected = inside.then_some(t);
                let got = decide(
                    p.m0,
                    age,
                    &given_marks(&p.composition, eta, || p.draws.clone()),
                    |_, _, _| death_age,
                );
                if expected.is_some() && got.is_none() {
                    dropped.push((p.m0, p.composition.fe_h(), eta, death_age, t));
                } else {
                    assert_eq!(got, expected, "{:?} at {:?}: {t}", p.m0, p.composition);
                }
            }
        }
        let verdicts = take_verdicts();
        println!(
            "soundness over {n} primaries, {SOUNDNESS_AGES} ages each: {}; {} deaths in the \
             interval dropped",
            verdicts.shares(),
            dropped.len()
        );
        assert!(
            dropped.is_empty(),
            "deaths in the interval dropped: {dropped:?}"
        );
        assert_eq!(verdicts.total(), u64::from(n) * SOUNDNESS_AGES);
        assert_eq!(verdicts.exact_inside, u64::from(n) * SOUNDNESS_AGES_INSIDE);
        verdicts
    }

    #[test]
    fn death_in_interval_keeps_every_death_inside_the_interval() {
        let verdicts = soundness(300);
        // The ages twice the bracket's width away are the mass bracket's, and the table's margins
        // are the table's where it answers.
        assert!(verdicts.bracket > 0 && verdicts.table > 0, "{verdicts:?}");
    }

    #[test]
    #[ignore = "slow: 10⁴ lifetimes, about 12 s"]
    fn death_in_interval_keeps_every_death_inside_the_interval_over_ten_thousand_primaries() {
        soundness(10_000);
    }

    /// A composition with a helium excess, which shortens a life and which the mass bracket is
    /// built without, skips the bracket; the fate table refuses it too, so the exact tier decides,
    /// from the one read of the draws.
    #[test]
    fn a_helium_excess_skips_the_mass_bracket() {
        let m0 = SolarMasses::new(20.0);
        let rich = Composition::from_fe_h(Dex::new(0.0), HeliumExcess::new(0.05));
        let draws = StarDraws::median();
        let life = lifetime(m0, &rich, &draws);
        let calls = std::cell::Cell::new(0);
        let make = || {
            calls.set(calls.get() + 1);
            draws.clone()
        };
        take_verdicts();
        let long_dead = Years::new(life.value() + 1e9);
        assert_eq!(
            death_in_interval(m0, &rich, draws.eta(), long_dead, make),
            None
        );
        assert_eq!(calls.get(), 1);
        let recent = Years::new(life.value() + 1e3);
        let t = death_in_interval(m0, &rich, draws.eta(), recent, make);
        assert!(t.is_some());
        assert_eq!(t, clock_time_of_death(recent, life));
        assert_eq!(calls.get(), 2);
        let verdicts = take_verdicts();
        assert_eq!(
            (verdicts.bracket, verdicts.table, verdicts.exact()),
            (0, 0, 2)
        );
    }

    /// The draws are made at most once, and not at all where the mass bracket decides.
    #[test]
    fn the_draws_are_made_once_and_only_where_needed() {
        let (m0, solar, draws) = (
            SolarMasses::new(20.0),
            Composition::SOLAR,
            StarDraws::median(),
        );
        let life = lifetime(m0, &solar, &draws);
        let calls = std::cell::Cell::new(0);
        let make = || {
            calls.set(calls.get() + 1);
            draws.clone()
        };
        let long_dead = Years::new(life.value() + 1e9);
        assert_eq!(
            death_in_interval(m0, &solar, draws.eta(), long_dead, make),
            None
        );
        assert_eq!(calls.get(), 0);
        let recent = Years::new(life.value() + 1e3);
        let t = death_in_interval(m0, &solar, draws.eta(), recent, make);
        assert_eq!(calls.get(), 1);
        assert_eq!(t, clock_time_of_death(recent, life));
        // Below 0.1 M☉ nothing dies, and nothing is read.
        let light = SolarMasses::new(0.08);
        assert_eq!(
            death_in_interval(light, &solar, draws.eta(), recent, make),
            None
        );
        assert_eq!(calls.get(), 1);
    }

    // --- On the Milky Way fixture with its kinematic tables ---

    /// The Milky Way fixture with its kinematic tables, built once per process as
    /// `observe::system`'s is: the build takes some 5 s in a test build and nextest runs each test
    /// in a process of its own, so the properties that read it run in one test,
    /// [`explosion_sites_are_where_the_records_move_and_agree_with_the_stellar_stage`].
    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x0813_0000), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
                .with_full_potential()
        })
    }

    /// The layer-E cells of the agreement test: six along x at the solar circle, and six on the
    /// arm ridge plan 03's tests use, at (−17,000, −19,700, 4) ly.
    fn sample_cells() -> Vec<CellKey> {
        let arm = GalacticPosition::from_light_years([-17_000.0, -19_700.0, 4.0]).unwrap();
        let arm = CellKey::containing(Layer::E, &arm)
            .unwrap()
            .gen_cell()
            .to_array();
        [[0, 203, 0], arm]
            .into_iter()
            .flat_map(|[x, y, z]| (0..6).map(move |dx| CellKey::new(Layer::E, [x + dx, y, z])))
            .map(Result::unwrap)
            .collect()
    }

    /// `record` with its age at the epoch replaced by `age`.
    fn re_aged(record: &SystemRecord, age: Years) -> SystemRecord {
        SystemRecord::from_parts(
            record.id(),
            *record.epoch_position(),
            record.origin(),
            record.population(),
            record.primary_initial_mass(),
            age,
        )
    }

    /// The death time the stellar stage gives `record`'s primary.
    fn death_time(record: &SystemRecord) -> ClockDeath {
        SystemStars::generate(galaxy(), record).death_time()
    }

    /// `record` re-aged so that its primary dies within a year of `target`, and that death.
    ///
    /// The metallicity draw reads the age, so a re-aged record's lifetime moves a little: the
    /// re-aging is repeated until the death is close enough. Each round reads the primary's
    /// lifetime alone, the record path's own seams with no companion and no track kept, which is
    /// a few times cheaper than its `SystemStars`; the tests compare with `death_time()` after.
    fn dying_at(record: &SystemRecord, target: UniverseTime) -> (SystemRecord, UniverseTime) {
        let galaxy = galaxy();
        let mut current = *record;
        for _ in 0..10 {
            let death_age = lifetime(
                track_mass_of(current.primary_initial_mass()),
                &draw_metallicity(galaxy, &current),
                &primary_draws(galaxy, &current),
            );
            let t = clock_time_of_death(current.age_at_epoch(), death_age)
                .expect("a layer-E primary dies on the clock");
            if t.checked_since(target).unwrap().as_julian_years_f64().abs() < 1.0 {
                return (current, t);
            }
            let lifetime = t.since_epoch().as_julian_years_f64() + current.age_at_epoch().value();
            let age = lifetime - target.since_epoch().as_julian_years_f64();
            current = re_aged(record, Years::new(age));
        }
        panic!(
            "{:?} does not settle within a year of {target}",
            record.id()
        );
    }

    /// The fixture's properties, each on its one build, in turn.
    #[test]
    fn explosion_sites_are_where_the_records_move_and_agree_with_the_stellar_stage() {
        let mut cell = Vec::new();
        generate_cell(
            galaxy(),
            CellKey::new(Layer::E, [0, 203, 0]).unwrap(),
            &mut cell,
        );
        let record = cell[0];
        a_death_in_five_centuries_explodes_where_the_record_has_moved_to(&record);
        a_record_dead_ten_million_years_has_no_site(&record);
        let records = explosion_sites_agree_with_the_stellar_stage();
        // The agreement test's real records, and its records re-aged to die in the interval's
        // middle, each with a site: the same sites in any order of calls.
        assert_order_independent(&records, |record| explosion_site(galaxy(), record));
        a_site_is_the_same_by_every_route();
    }

    /// A field record dying at T = +500 yr: its site is x + v T, and its death `death_time()`'s T.
    fn a_death_in_five_centuries_explodes_where_the_record_has_moved_to(record: &SystemRecord) {
        let galaxy = galaxy();
        let (dying, _) = dying_at(record, years(500));
        let ClockDeath::At(t, _) = death_time(&dying) else {
            panic!("a death at +500 yr is on the clock")
        };
        assert!(
            t.checked_since(years(500))
                .unwrap()
                .as_julian_years_f64()
                .abs()
                < 1.0
        );
        let site = explosion_site(galaxy, &dying).expect("a death at +500 yr is in the interval");
        assert_eq!(site.death(), t);
        let velocity = epoch_velocity(galaxy, &dying).metres_per_second();
        assert!(
            velocity.iter().any(|v| v.abs() > 1e3),
            "the fixture's records move: {velocity:?} m/s"
        );
        // x + v T recomputed from the velocity's components and T's seconds. 4 m is a few units in
        // the last place of an offset inside one light-year (2 m near its top), against a drift of
        // some 10¹⁴ m over the five centuries.
        let seconds = t.since_epoch().as_seconds_f64();
        let moved = dying
            .epoch_position()
            .displacement_to(site.position())
            .metres();
        for axis in 0..3 {
            let expected = velocity[axis] * seconds;
            assert!(
                (moved[axis] - expected).abs() <= 4.0,
                "axis {axis}: moved {} m against v T = {expected} m",
                moved[axis]
            );
        }
        // Inside the clock window the site is where the range query draws the record then, bit
        // for bit: the same drift by the same arithmetic.
        assert_eq!(*site.position(), position_at(galaxy, &dying, t));
    }

    /// A record dead 10 Myr has no site.
    fn a_record_dead_ten_million_years_has_no_site(record: &SystemRecord) {
        let (dead, _) = dying_at(record, years(-10_000_000));
        let ClockDeath::At(t, _) = death_time(&dead) else {
            panic!("a death 10 Myr ago is on the clock")
        };
        assert!(t < interval_start());
        assert_eq!(explosion_site(galaxy(), &dead), None);
    }

    /// Checks `record`'s site against the stellar stage's death time, bit for bit where it has
    /// one, and returns whether it has one.
    fn agrees(record: &SystemRecord) -> bool {
        match (explosion_site(galaxy(), record), death_time(record)) {
            (Some(site), ClockDeath::At(t, _)) => {
                assert_eq!(site.death(), t, "{:?}", record.id());
                true
            }
            (None, ClockDeath::At(t, _)) => {
                assert!(
                    !in_interval(t),
                    "{:?} dies at {t}, in the interval",
                    record.id()
                );
                false
            }
            (None, ClockDeath::BeyondClockRange) => false,
            (site, death) => panic!("{:?}: site {site:?}, death {death:?}", record.id()),
        }
    }

    /// The record path (metallicity, η, draws, track mass) agrees with
    /// `SystemStars::death_time()` on real records, and on records re-aged across both ends of
    /// the interval and across the tiers' margins, so that every tier decides some. Returns the
    /// real records and those re-aged to die in the interval's middle.
    fn explosion_sites_agree_with_the_stellar_stage() -> Vec<SystemRecord> {
        let galaxy = galaxy();
        let mut sample = Vec::new();
        let mut cell = Vec::new();
        for key in sample_cells() {
            generate_cell(galaxy, key, &mut cell);
            sample.extend_from_slice(&cell);
        }
        sample.truncate(200);
        // `explosion_site` asks the mass bracket without reading the metallicity, on the premise
        // that a grid record's draw has no helium excess.
        for record in &sample {
            let helium = draw_metallicity(galaxy, record).helium_excess().value();
            assert_same_bits(helium, 0.0);
        }
        take_verdicts();
        let sites = sample.iter().filter(|record| agrees(record)).count();
        let real = take_verdicts();
        assert_eq!(real.total(), u64::try_from(sample.len()).unwrap());
        let start = interval_start();
        let middle =
            UniverseTime::new(start.seconds().midpoint(INTERVAL_END.seconds()), 0).unwrap();
        let mut re_aged_sites = 0;
        let (mut re_aged_records, mut middles) = (0, Vec::new());
        for record in sample.iter().step_by(sample.len().div_ceil(12)) {
            let ClockDeath::At(t, _) = death_time(record) else {
                panic!("a layer-E primary dies on the clock")
            };
            let life = t.since_epoch().as_julian_years_f64() + record.age_at_epoch().value();
            let m0 = record.primary_initial_mass();
            let composition = draw_metallicity(galaxy, record);
            let margin = FittedFates::generator()
                .fate_fitted(m0, &composition, primary_eta(galaxy, record))
                .map_or(DEAD_MARGIN_FLOOR, |fate| table_margin(&fate));
            let (lo, hi) = LifetimeBracket::shared().bracket(m0).unwrap();
            let (md, width) = (margin * life, hi.value() - lo.value());
            let before = |years: f64| start.checked_sub(span_of_years(years)).unwrap();
            let after = |years: f64| INTERVAL_END.checked_add(span_of_years(years)).unwrap();
            let targets = [
                before(1_000.0),
                start.checked_add(span_of_years(1_000.0)).unwrap(),
                middle,
                INTERVAL_END.checked_sub(span_of_years(100.0)).unwrap(),
                after(100.0),
                before(0.5 * md),
                before(2.0 * md),
                after(0.5 * md),
                after(2.0 * md),
                before(2.0 * width),
                after(2.0 * width),
            ];
            for target in targets {
                let (aged, _) = dying_at(record, target);
                re_aged_sites += usize::from(agrees(&aged));
                re_aged_records += 1;
                if target == middle {
                    middles.push(aged);
                }
            }
        }
        let re_aged = take_verdicts();
        println!(
            "agreement: {} real records, {sites} with a site: {}; {} re-aged records, \
             {re_aged_sites} with a site: {}",
            sample.len(),
            real.shares(),
            re_aged_records,
            re_aged.shares()
        );
        assert!(
            re_aged.bracket > 0
                && re_aged.table > 0
                && re_aged.exact_inside > 0
                && re_aged.exact_outside > 0,
            "every tier decides some re-aged record: {re_aged:?}"
        );
        sample.extend(middles);
        sample
    }

    /// The first record with a site in a fixed scan of layer-E cells along x at the solar circle,
    /// where about one cell in 28 holds a death in the interval, with its cell and its site.
    fn a_record_with_a_site(galaxy: &Galaxy) -> (CellKey, SystemRecord, ExplosionSite) {
        let mut cell = Vec::new();
        for x in 0..600 {
            let key = CellKey::new(Layer::E, [x, 203, 0]).unwrap();
            generate_cell(galaxy, key, &mut cell);
            if let Some((record, site)) = cell
                .iter()
                .find_map(|record| explosion_site(galaxy, record).map(|site| (*record, site)))
            {
                return (key, record, site);
            }
        }
        panic!("no death in the interval in 600 layer-E cells at the solar circle");
    }

    /// A site is the same through `resolve`, `generate_cell` and a direct call, and on a second
    /// call.
    fn a_site_is_the_same_by_every_route() {
        let galaxy = galaxy();
        let (key, record, site) = a_record_with_a_site(galaxy);
        let resolved = resolve(galaxy, record.id()).unwrap();
        assert_eq!(resolved, record);
        assert_eq!(explosion_site(galaxy, &resolved), Some(site));
        assert_eq!(explosion_site(galaxy, &record), Some(site));
        let SystemIdKind::Grid(grid) = record.id().kind() else {
            unreachable!("a grid record has a grid ID")
        };
        println!(
            "a site in layer-E cell {:?}, candidate {}: {:.2} M☉ dying at {} ({:.1} yr) at {:?} \
             ly, {:?} ly at the epoch",
            key.gen_cell().to_array(),
            grid.index(),
            record.primary_initial_mass().value(),
            site.death(),
            site.death().since_epoch().as_julian_years_f64(),
            site.position().to_light_years_f64(),
            record.epoch_position().to_light_years_f64(),
        );
    }

    // --- The benches' centre ---

    /// A cell cache that generates every cell it is asked for and keeps the layer-E records.
    #[derive(Debug, Default)]
    struct RecordingCache {
        layer_e: Vec<SystemRecord>,
    }

    impl CellCache for RecordingCache {
        fn with_cell<R>(
            &mut self,
            galaxy: &Galaxy,
            key: CellKey,
            f: impl FnOnce(&[SystemRecord]) -> R,
        ) -> R {
            let mut cell = Vec::new();
            generate_cell(galaxy, key, &mut cell);
            if key.layer() == Layer::E {
                self.layer_e.extend_from_slice(&cell);
            }
            f(&cell)
        }
    }

    /// Each tier's share of the verdicts at the benches' centre: the layer-E cell at the Sun-like
    /// point of `benches/placement.rs`'s `cell_by_layer`, and the 50 ly query of
    /// `benches/range_query.rs`'s `range_50ly_cold`, each on its bench's seed, from
    /// [`explosion_site`] called on each placed layer-E record: one verdict per record. Placing
    /// them, the production hook gives one verdict per layer-E record once a claim is set, and none
    /// while [`RECENT_DEATH_CLAIM`] is `None` (P08.T13.b); another layer gives none either way.
    #[test]
    fn the_benches_centre_gives_one_verdict_per_layer_e_record_and_the_hook_needs_a_claim() {
        // One per placed record: every verdict here is the bracket's, so no record has a site a
        // claim could take, and the placed records are the accepted candidates.
        let per_record = |records: usize| {
            let records = u64::try_from(records).unwrap();
            if RECENT_DEATH_CLAIM.is_some() {
                records
            } else {
                0
            }
        };
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let milky_way = |seed: u64| {
            Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
        };
        let galaxy = milky_way(0x0311_0000_0000_0000);
        let mut cell = Vec::new();
        take_verdicts();
        generate_cell(
            &galaxy,
            CellKey::containing(Layer::E, &sun).unwrap(),
            &mut cell,
        );
        assert_eq!(take_verdicts().total(), per_record(cell.len()));
        let cell_sites = cell
            .iter()
            .filter_map(|record| explosion_site(&galaxy, record))
            .count();
        let one_cell = take_verdicts();
        assert_eq!(one_cell.total(), u64::try_from(cell.len()).unwrap());
        let mut other = Vec::new();
        for layer in [Layer::A, Layer::B, Layer::C, Layer::D] {
            generate_cell(
                &galaxy,
                CellKey::containing(layer, &sun).unwrap(),
                &mut other,
            );
        }
        assert_eq!(take_verdicts().total(), 0);

        let galaxy = milky_way(0x0311_1000_0000_0000);
        let query = RangeQuery::builder(sun, LightYears::new(50.0))
            .build()
            .unwrap();
        let mut cache = RecordingCache::default();
        let result = range_query(&galaxy, &mut cache, &[], &query);
        assert_eq!(take_verdicts().total(), per_record(cache.layer_e.len()));
        let query_sites = cache
            .layer_e
            .iter()
            .filter_map(|record| explosion_site(&galaxy, record))
            .count();
        let queried = take_verdicts();
        assert_eq!(queried.total(), u64::try_from(cache.layer_e.len()).unwrap());
        println!(
            "cell_by_layer (E, Sun-like point): {}, {cell_sites} sites; range_50ly_cold ({} \
             cells): {}, {query_sites} sites",
            one_cell.shares(),
            result.stats().cells_visited(),
            queried.shares()
        );
    }
}
