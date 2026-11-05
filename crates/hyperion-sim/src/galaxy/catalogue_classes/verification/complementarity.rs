//! P09.T38's complementarity: every recently dead or luminous-blue-variable layer-E system is held
//! by exactly one of the grid and the catalogue, and both sides decide it by the one shared test.
//!
//! A mark set is a grid record's: its primary's initial mass, its age at the epoch, its metallicity
//! draw, its primary's draws, its epoch velocity and its placement class. Each side keys its draws
//! on its own ID, so a class candidate has no grid twin (P09.T35's records): complementarity is per
//! mark set. For each, the grid's carve-out hook (`catalogue_claims`, with its two claims and its
//! site's gate) gives whether the grid keeps the system, and each class's own test, handed a
//! candidate carrying the same marks (`core_collapse::test_on_grid_marks`,
//! `lbv::test_on_grid_marks`), whether it keeps an entry. Exactly one of the three holds it, each
//! side's answer is the test's own statement of the one shared test (plan 08's predicate, then
//! `DeathMarks::of` and `claims` in the field, then the variables' prefilter and window, which
//! from 27 the grid asks of a field system alone, since that class draws no displaced one), the
//! class's site is the grid's, backed out along the record's velocity, and a kept entry is its
//! progenitor before its death and its supernova from it, at every evaluation time of the clock
//! window. Where a class keeps the marks, the class can draw them: its masses, its interval, its
//! component, and for a living runaway or walkaway its ejected term.
//!
//! The mark sets are drawn about the interval's ends, (−(`W_cap` + L + H), the class's thinning
//! interval's start, each set's own −(`W_eff` + L + H), +H) and across the clock window, about the
//! variables' prefilter's and horizon's edges, and over the whole interval, on young-field records
//! of the arm ridges and the nuclear disc, field stars and displaced alike, re-massed over band E
//! and re-aged; each is drawn on its own index's generator, so a run split into shares is the
//! whole run's. A quarter of them is decided again with the cheap tiers' brackets moved
//! (`with_forced_margin`): the predicate's reject path, which both sides take together. The grid's
//! own cells are walked too, through the production hook, with the lifetime the record's marks
//! computed: each record the carve-out claims is one class's entry and each it keeps is none.
//!
//! Inside features and at the centre the cells and the lists ask the same `claimed_by` and
//! `claimed_at_centre` on their own marks; their tests (`features::members::recent::tests`,
//! `features::centre::recent::tests`) run under every 27 hold beside these.

use std::collections::BTreeMap;

use hyperion_testkit::lcg::Lcg;

use super::{milky_way, ridge_cells};
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::core_collapse::{
    self, CoreCollapseEntry, CoreCollapseProcess, recent_death_claims,
};
use crate::galaxy::catalogue_classes::lbv::{
    self, LbvProcess, claims_its_placement, horizon_window, host_ages, lbv_claims, may_host,
};
use crate::galaxy::catalogue_classes::supernova::{
    DeathMarks, SupernovaInterval, SupernovaState, Surroundings, claims, effective_window,
};
use crate::galaxy::displaced::site::{CarveOutRules, with_forced_margin};
use crate::galaxy::displaced::{
    DisplacedKind, ExplosionSite, PlacementClass, death_in_interval_under, explosion_site,
};
use crate::galaxy::placement::{
    CandidateOutcome, CellKey, SystemRecord, candidate_count, catalogue_claims, evaluate_candidate,
    generate_cell,
};
use crate::galaxy::query::epoch_velocity;
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::snr::SHELL_WINDOW_CAP;
use crate::id::Layer;
use crate::math;
use crate::planetary::testing::in_shares;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::sse::lifetime_under;
use crate::stellar::system::{
    ClockDeath, SystemStars, draw_metallicity, primary_draws, primary_eta, track_mass_of,
};
use crate::tables::lbv::LBV_SAMPLER;
use crate::time::{CLOCK_WINDOW_H, ClockWindow, LIGHT_CROSSING_L, Span, UniverseTime};
use crate::units::{Seconds, SolarMasses, Years};

/// Whether the run is on WebAssembly, whose wasm32-wasip1 has no threads: a tenth of the mark sets
/// and an eighth of the ridges' cells, the checks' floors a tenth, there.
const WASM: bool = cfg!(target_family = "wasm");

/// The mark sets of a run.
const MARK_SETS: u64 = if WASM { 10_000 } else { 100_000 };

/// A floor of a native run's counts, a tenth of it on WebAssembly.
const fn floor(native: u64) -> u64 {
    if WASM { native / 10 } else { native }
}

/// The run's shares (`in_shares`), and the slots it reserves (`.config/nextest.toml`).
const SHARES: usize = 16;

/// The seed of the mark sets' generators: mark set `i` draws on `Lcg::new(MARKS_SEED + i)`.
const MARKS_SEED: u64 = 0x0938_0000_0000_0000;

/// Every how many mark sets one is decided again with the reject path forced.
const FORCED_EVERY: u64 = 4;

/// The relative margin the forced reject path moves the cheap tiers' brackets by: late for the
/// deaths about +H, early for those about the interval's start.
const FORCED_MARGIN: f64 = 0.25;

/// Who holds a mark set's system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Holder {
    /// The grid keeps it.
    Grid,
    /// The core-collapse class keeps its death.
    CoreCollapse,
    /// The luminous blue variables' class keeps it.
    LuminousBlueVariable,
}

/// Where a mark set's death or age was drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    /// T uniform over the predicate's interval and a little beyond each end.
    Interval,
    /// T about the predicate's start, −(`W_cap` + L + H).
    PredicateStart,
    /// T about the core collapses' thinning interval's start, −(the field's cap + L + H).
    ClassStart,
    /// T about the set's own start, −(`W_eff` + L + H).
    ClaimStart,
    /// T about +H.
    End,
    /// T across the clock window.
    ClockWindow,
    /// The age about the variables' prefilter's edges.
    PrefilterEdge,
    /// The age about the edges of the ages at which the variable's window overlaps the source
    /// horizon.
    HorizonEdge,
}

impl Target {
    /// The targets in turn: mark set `i` takes `ALL[i % 8]`.
    const ALL: [Self; 8] = [
        Self::Interval,
        Self::PredicateStart,
        Self::ClassStart,
        Self::ClaimStart,
        Self::End,
        Self::ClockWindow,
        Self::PrefilterEdge,
        Self::HorizonEdge,
    ];

    /// The forced margin's sign for a death drawn here: late about +H and across the clock window,
    /// early about the starts and the variables' edges, both over the interval.
    fn forced_margin(self, i: u64) -> f64 {
        let late = match self {
            Self::End | Self::ClockWindow => true,
            Self::PredicateStart
            | Self::ClassStart
            | Self::ClaimStart
            | Self::PrefilterEdge
            | Self::HorizonEdge => false,
            Self::Interval => i.is_multiple_of(2),
        };
        if late { FORCED_MARGIN } else { -FORCED_MARGIN }
    }
}

/// Where a mark set the class keeps can be drawn from, or why it cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Coverage {
    /// Its class's candidates hold it.
    Covered,
    /// Its death lies outside the class's thinning interval.
    OutsideInterval,
    /// Its component is not one the class draws.
    NoComponent,
    /// A displaced system's death, where the core collapses have no ejected terms: at 26 they
    /// draw the field's deaths alone, at the field's share.
    DisplacedDeath,
    /// A living runaway or walkaway whose class no ejected term of the core collapses holds.
    NoEjectedTerm,
    /// A displaced system, which the luminous blue variables do not draw.
    DisplacedVariable,
}

/// What a run counts.
#[derive(Debug, Default)]
struct Tally {
    /// Mark sets by target and holder.
    holders: BTreeMap<(Target, Holder), u64>,
    /// The kept mark sets that the grid can place (its state consistent with its placement class
    /// and its age's density positive), by holder and coverage.
    coverage: BTreeMap<(Holder, Coverage), u64>,
    /// The deaths the exact tier puts in the interval, and those the cheap tiers refused: failed
    /// bounds.
    deaths_in_interval: u64,
    failed_bounds: u64,
    /// The mark sets decided again with the reject path forced, those whose death it refused, and
    /// those it handed to a class, each a failed bound that the exact tier now decides.
    forced: u64,
    forced_refused: u64,
    forced_kept: u64,
    /// The kept entries whose state was read at times on both sides of their death.
    straddled: u64,
    /// The displaced systems the variables' test would keep, which the grid keeps.
    displaced_variables: u64,
}

impl Tally {
    fn add(mut self, other: Self) -> Self {
        for (key, n) in other.holders {
            *self.holders.entry(key).or_default() += n;
        }
        for (key, n) in other.coverage {
            *self.coverage.entry(key).or_default() += n;
        }
        self.deaths_in_interval += other.deaths_in_interval;
        self.failed_bounds += other.failed_bounds;
        self.forced += other.forced;
        self.forced_refused += other.forced_refused;
        self.forced_kept += other.forced_kept;
        self.straddled += other.straddled;
        self.displaced_variables += other.displaced_variables;
        self
    }

    /// The kept mark sets of `holder` with `coverage`.
    fn covered(&self, holder: Holder, coverage: Coverage) -> u64 {
        self.coverage.get(&(holder, coverage)).copied().unwrap_or(0)
    }

    /// The mark sets `holder` holds.
    fn held_by(&self, holder: Holder) -> u64 {
        self.holders
            .iter()
            .filter(|((_, h), _)| *h == holder)
            .map(|(_, n)| n)
            .sum()
    }
}

/// What the classes a mark set's coverage reads are in a galaxy.
struct Classes {
    core_collapse: CoreCollapseProcess,
    variables: LbvProcess,
}

impl Classes {
    fn of(galaxy: &Galaxy) -> Self {
        Self {
            core_collapse: CoreCollapseProcess::new(galaxy),
            variables: LbvProcess::new(galaxy),
        }
    }
}

/// Every side's answer on one mark set, checked against the others: the holder, and the kept
/// core-collapse entry and the grid's site.
struct Sides {
    holder: Holder,
    entry: Option<CoreCollapseEntry>,
    site: Option<ExplosionSite>,
    /// Whether the variables' test keeps the marks of a displaced system, which their class draws
    /// no candidate of: the grid's from 27.
    displaced_variable: bool,
}

/// Every side's answer on the mark set `record`, the grid's hook given the record's lifetime where
/// its marks computed one (`lifetime`), each checked against the others and against the test's own
/// statement of the one shared test ([`shared_test`]).
///
/// # Panics
///
/// Where two sides disagree, or two hold the system.
fn sides(galaxy: &Galaxy, record: &SystemRecord, lifetime: Option<Years>) -> Sides {
    let id = record.id();
    let site = explosion_site(galaxy, record);
    let grid_cc = site.is_some_and(|s| recent_death_claims(galaxy, &s, record));
    let grid_lbv = lbv_claims(galaxy, record);
    let grid = catalogue_claims(galaxy, record, lifetime);
    assert_eq!(
        grid,
        grid_cc || grid_lbv,
        "{id:?}: the hook asks the two claims"
    );
    let at = site.map_or(*record.epoch_position(), |s| *s.position());
    let entry = core_collapse::test_on_grid_marks(galaxy, record, at);
    let variable = lbv::test_on_grid_marks(galaxy, record);
    assert_eq!(
        entry.is_some(),
        grid_cc,
        "{id:?}: the core collapses' two sides"
    );
    if let (Some(entry), Some(site)) = (&entry, &site) {
        assert_eq!(
            entry.site(),
            site,
            "{id:?}: the class dies at the grid's site"
        );
    }
    // The class's test reads the marks alone. The grid's claim reads the record's placement too:
    // from 27 it claims a field system's alone, since the class draws no other
    // (`claims_its_placement`), so a displaced system the class's test would keep is the grid's.
    let field = claims_its_placement(galaxy, record);
    if !grid_cc {
        assert_eq!(
            grid_lbv,
            field && variable.is_some(),
            "{id:?}: the luminous blue variables' two sides"
        );
    }
    assert!(
        !(entry.is_some() && variable.is_some()),
        "{id:?}: both classes keep it"
    );
    let holder = if grid_cc {
        Holder::CoreCollapse
    } else if grid_lbv {
        Holder::LuminousBlueVariable
    } else {
        Holder::Grid
    };
    assert_eq!(
        shared_test(galaxy, record),
        holder,
        "{id:?}: the shared test"
    );
    Sides {
        holder,
        entry,
        site,
        displaced_variable: variable.is_some() && !field,
    }
}

/// The one shared test on `record`'s marks, as the test states it: plan 08's predicate on the
/// primary's marks under the galaxy's stellar rules, the site the epoch position moved along the
/// epoch velocity to T, then [`DeathMarks::of`] and [`claims`] in the field (P09.T35); else, for a
/// field system, the variables' prefilter and window (P09.T43);
/// else the grid's.
fn shared_test(galaxy: &Galaxy, record: &SystemRecord) -> Holder {
    let composition = draw_metallicity(galaxy, record);
    let (mass, age) = (record.primary_initial_mass(), record.age_at_epoch());
    let death = death_in_interval_under(
        mass,
        &composition,
        primary_eta(galaxy, record),
        age,
        || primary_draws(galaxy, record),
        galaxy.rules().stellar,
    );
    let field = !matches!(record.placement_class(), PlacementClass::Displaced { .. });
    let prefiltered = field && may_host(mass, age);
    if death.is_none() && !prefiltered {
        return Holder::Grid;
    }
    let stars = SystemStars::generate_with(
        galaxy,
        record,
        &composition,
        MultiplicityContext::ForcedSingle,
    );
    if let Some(death) = death {
        let velocity = epoch_velocity(galaxy, record);
        let elapsed = Seconds::new(death.since_epoch().as_seconds_f64());
        let position = record
            .epoch_position()
            .translated(velocity.displacement_over(elapsed))
            .expect("a site in the cube");
        let site = ExplosionSite::new(position, death);
        if DeathMarks::of(galaxy, &stars, &site, velocity)
            .is_some_and(|marks| claims(galaxy, &site, &marks, Surroundings::Field))
        {
            return Holder::CoreCollapse;
        }
    }
    if prefiltered && horizon_window(&stars).is_some() {
        Holder::LuminousBlueVariable
    } else {
        Holder::Grid
    }
}

/// The exact death of `record`'s primary, the stellar stage's, bit for bit as the exact tier's.
fn exact_death(galaxy: &Galaxy, record: &SystemRecord) -> Option<UniverseTime> {
    match SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle).death_time() {
        ClockDeath::At(t, _) => Some(t),
        ClockDeath::BeyondClockRange | ClockDeath::AlreadyRemnantAtBirth => None,
    }
}

/// Whether `record`'s placement class is a living star's.
fn is_living_class(record: &SystemRecord) -> bool {
    match record.placement_class() {
        PlacementClass::Alive => true,
        PlacementClass::Retained { .. } => false,
        PlacementClass::Displaced { kind, .. } => match kind {
            DisplacedKind::Runaway | DisplacedKind::Walkaway => true,
            DisplacedKind::Remnant | DisplacedKind::HypervelocitySurvivor => false,
        },
    }
}

/// Whether `record` is a living runaway or walkaway.
fn is_ejected(record: &SystemRecord) -> bool {
    matches!(
        record.placement_class(),
        PlacementClass::Displaced {
            kind: DisplacedKind::Runaway | DisplacedKind::Walkaway,
            ..
        }
    )
}

/// Whether the grid can place `record` with its marks: a living star's class if and only if its
/// primary is alive at the epoch, and an age its component's field ages hold.
fn placeable(galaxy: &Galaxy, record: &SystemRecord, death: Option<UniverseTime>) -> bool {
    let alive = death.is_none_or(|t| t > UniverseTime::EPOCH);
    let component = record.component().expect("a grid record has its component");
    alive == is_living_class(record)
        && galaxy.field_ages(component).pdf(record.age_at_epoch()) > 0.0
}

/// Whether the class that keeps `record` (`holder`) draws its marks.
fn coverage(classes: &Classes, record: &SystemRecord, sides: &Sides) -> Coverage {
    let component = record.component().expect("a grid record has its component");
    match sides.holder {
        Holder::Grid => Coverage::Covered,
        Holder::CoreCollapse => {
            let process = &classes.core_collapse;
            let death = sides.site.expect("a kept death has its site").death();
            if !process.interval().contains(death) {
                Coverage::OutsideInterval
            } else if !process.components().iter().any(|c| c.id() == component) {
                Coverage::NoComponent
            } else if let PlacementClass::Displaced { class, .. } = record.placement_class() {
                if process.ejected_terms().is_empty() {
                    Coverage::DisplacedDeath
                } else if is_ejected(record)
                    && !process
                        .ejected_terms()
                        .iter()
                        .any(|term| term.component() == component && term.class() == class)
                {
                    Coverage::NoEjectedTerm
                } else {
                    Coverage::Covered
                }
            } else {
                Coverage::Covered
            }
        }
        Holder::LuminousBlueVariable => {
            if !classes
                .variables
                .components()
                .iter()
                .any(|c| c.id() == component)
            {
                Coverage::NoComponent
            } else if matches!(record.placement_class(), PlacementClass::Displaced { .. }) {
                Coverage::DisplacedVariable
            } else {
                Coverage::Covered
            }
        }
    }
}

/// A kept entry's state at the clock window's ends, the epoch and a second each side of its death
/// where they fall in the window: its progenitor before the death and its supernova or bare
/// remnant from it. Returns whether the death fell in the window, so that both sides were read.
fn check_states(entry: &CoreCollapseEntry) -> bool {
    let death = entry.site().death();
    let second = Span::from_seconds(1);
    let times = [
        Some(ClockWindow::START),
        Some(UniverseTime::EPOCH),
        Some(ClockWindow::END),
        death.checked_sub(second),
        Some(death),
        death.checked_add(second),
    ];
    for t in times
        .into_iter()
        .flatten()
        .filter(|&t| ClockWindow::contains(t))
    {
        let state = entry.supernova().state_at(t);
        assert_eq!(
            matches!(state, SupernovaState::Progenitor { .. }),
            t < death,
            "{:?} at {t:?}, dying at {death:?}: {state:?}",
            entry.record().id()
        );
    }
    ClockWindow::contains(death)
}

/// Years after the epoch of the clock time `t`.
fn years_after_epoch(t: UniverseTime) -> f64 {
    t.since_epoch().as_julian_years_f64()
}

/// A displacement of 10⁻⁶ to 10⁴ years, log-uniform, of either sign.
fn about(lcg: &mut Lcg) -> f64 {
    let size = math::exp(math::ln(10.0) * (10.0 * lcg.next_f64() - 6.0));
    if lcg.next_below(2) == 0 { size } else { -size }
}

/// `base` re-massed to `mass` and re-aged so that its primary dies at `years` after the epoch:
/// the age is the lifetime less T, the lifetime read again at that age (the metallicity draw reads
/// it) until it settles.
fn dying_at(galaxy: &Galaxy, base: &SystemRecord, mass: SolarMasses, years: f64) -> SystemRecord {
    let rules = galaxy.rules().stellar;
    let mut record = base.with_primary_initial_mass(mass);
    for _ in 0..3 {
        let life = lifetime_under(
            track_mass_of(mass),
            &draw_metallicity(galaxy, &record),
            &primary_draws(galaxy, &record),
            rules,
        );
        record = record.with_age_at_epoch(Years::new((life.value() - years).max(1.0)));
    }
    record
}

/// One of `records`, by `lcg`.
fn pick<'a>(records: &'a [SystemRecord], lcg: &mut Lcg) -> &'a SystemRecord {
    &records[usize::try_from(lcg.next_below(u64::try_from(records.len()).unwrap())).unwrap()]
}

/// Mark set `i` on the base records `pool`: a base record (for a target whose star lives at the
/// epoch, half the time a living field star's, of which the young field's layer E has few), a
/// mass over band E (three in ten over the variables' masses) and an age at its target.
fn mark_set(galaxy: &Galaxy, classes: &Classes, pool: &Pool, i: u64) -> SystemRecord {
    let mut lcg = Lcg::new(MARKS_SEED + i);
    let target = Target::ALL[usize::try_from(i % 8).unwrap()];
    let living = matches!(
        target,
        Target::End | Target::ClockWindow | Target::PrefilterEdge | Target::HorizonEdge
    );
    let base = if living && !pool.living.is_empty() && lcg.next_below(2) == 0 {
        pick(&pool.living, &mut lcg)
    } else {
        pick(&pool.all, &mut lcg)
    };
    let (lo, hi) = if lcg.next_below(10) < 7 {
        (8.0, 150.0)
    } else {
        (32.0, 150.0)
    };
    let mass = SolarMasses::new(lo * math::exp(math::ln(hi / lo) * lcg.next_f64()));
    let (h, l) = (
        CLOCK_WINDOW_H.as_julian_years_f64(),
        LIGHT_CROSSING_L.as_julian_years_f64(),
    );
    let cap = SHELL_WINDOW_CAP.value();
    let at = |years: f64| dying_at(galaxy, base, mass, years);
    match target {
        Target::Interval => {
            let (start, end) = (-(cap + l + h) - 2.0e5, h + 100.0);
            at(start + (end - start) * lcg.next_f64())
        }
        Target::PredicateStart => at(-(cap + l + h) + about(&mut lcg)),
        Target::ClassStart => {
            at(years_after_epoch(classes.core_collapse.interval().start()) + about(&mut lcg))
        }
        Target::ClaimStart => {
            // The set's own window, from a probe dead two million years.
            let probe = at(-2.0e6);
            let start = explosion_site(galaxy, &probe).and_then(|site| {
                let stars =
                    SystemStars::generate_in(galaxy, &probe, MultiplicityContext::ForcedSingle);
                let velocity = epoch_velocity(galaxy, &probe);
                DeathMarks::of(galaxy, &stars, &site, velocity).map(|marks| {
                    let window = effective_window(galaxy, &site, &marks, Surroundings::Field);
                    years_after_epoch(SupernovaInterval::of_window(window.duration()).start())
                })
            });
            at(start.unwrap_or(-2.0e6) + about(&mut lcg))
        }
        Target::End => at(h + about(&mut lcg)),
        Target::ClockWindow => at(h * (2.0 * lcg.next_f64() - 1.0)),
        Target::PrefilterEdge => {
            let record = base.with_primary_initial_mass(mass);
            let Some(k) = LBV_SAMPLER.node_of(mass) else {
                return at(h + about(&mut lcg));
            };
            let (first, last) = host_ages(k);
            let edge = if lcg.next_below(2) == 0 { first } else { last };
            record.with_age_at_epoch(Years::new(edge.value() + about(&mut lcg)))
        }
        Target::HorizonEdge => {
            // The window's ages, from a probe long dead, whose track covers its whole life.
            let probe_age = 3.0e7;
            let probe = base
                .with_primary_initial_mass(mass)
                .with_age_at_epoch(Years::new(probe_age));
            let probed =
                SystemStars::generate_in(galaxy, &probe, MultiplicityContext::ForcedSingle);
            let Some((start, end)) = probed.lbv_window() else {
                return at(h + about(&mut lcg));
            };
            let age = if lcg.next_below(2) == 0 {
                probe_age + years_after_epoch(start) - h
            } else {
                probe_age + years_after_epoch(end) + h + l
            };
            probe.with_age_at_epoch(Years::new((age + about(&mut lcg)).max(1.0)))
        }
    }
}

/// The base records the mark sets are drawn on: all of them, and the living field stars among
/// them.
struct Pool {
    all: Vec<SystemRecord>,
    living: Vec<SystemRecord>,
}

/// The base records of `galaxy`: its layer-E records along the arm ridges from 8,000 to 40,000 ly,
/// whole cells, and across the nuclear disc, each cell's first candidates (a cell there holds
/// hundreds of thousands); every one of the young field's components and a few hundred of the
/// rest, whose ages no recent death reaches. The cells are read in [`SHARES`] shares.
fn base_records(galaxy: &Galaxy, classes: &Classes) -> Pool {
    const NUCLEAR_CANDIDATES: u32 = 4_096;
    let young = |r: &SystemRecord| {
        r.component().is_some_and(|c| {
            classes
                .core_collapse
                .components()
                .iter()
                .any(|y| y.id() == c)
        })
    };
    let ridges = ridge_cells(
        galaxy,
        &[
            8_000.0, 12_000.0, 16_000.0, 20_000.0, 26_000.0, 32_000.0, 40_000.0,
        ],
        8,
    );
    let nuclear: Vec<CellKey> = [-448.0, -192.0, 192.0, 448.0]
        .iter()
        .map(|&x| {
            let at = GalacticPosition::from_light_years([x, 64.0, 0.0]).expect("in the cube");
            CellKey::containing(Layer::E, &at).expect("a cell of the cube")
        })
        .collect();
    let shares = in_shares(SHARES, SHARES, |share| {
        let (mut pool, mut rest, mut cell) = (Vec::new(), Vec::new(), Vec::new());
        let mut keep = |record: &SystemRecord| {
            if young(record) {
                pool.push(*record);
            } else if rest.len() < 400 / SHARES {
                rest.push(*record);
            }
        };
        for &key in ridges.iter().skip(share).step_by(SHARES) {
            generate_cell(galaxy, key, &mut cell);
            cell.iter().for_each(&mut keep);
        }
        for &key in nuclear.iter().skip(share).step_by(SHARES) {
            for index in 0..NUCLEAR_CANDIDATES.min(candidate_count(galaxy, key)) {
                if let CandidateOutcome::Accepted(record) = evaluate_candidate(galaxy, key, index) {
                    keep(&record);
                }
            }
        }
        pool.extend(rest);
        pool
    });
    let all: Vec<SystemRecord> = shares.into_iter().flatten().collect();
    let living = all
        .iter()
        .filter(|r| r.placement_class() == PlacementClass::Alive)
        .copied()
        .collect();
    Pool { all, living }
}

/// The grid's own cells of `on` against `off`, the same rules but the carve-out's held arm: each
/// record `on` claims through the production hook, with the lifetime its marks computed, is
/// `off`'s and one class's entry by every side's answer; each `on` keeps is `off`'s and every
/// side's grid system. Each cell is read to the first candidates its limit gives. Returns the records claimed
/// and kept.
fn walk_cells(on: &Galaxy, off: &Galaxy, keys: &[(CellKey, u32)]) -> (u64, u64) {
    let rows = in_shares(SHARES, SHARES, |share| {
        let (mut claimed, mut kept) = (0, 0);
        for &(key, limit) in keys.iter().skip(share).step_by(SHARES) {
            assert_eq!(candidate_count(off, key), candidate_count(on, key));
            for index in 0..candidate_count(off, key).min(limit) {
                match (
                    evaluate_candidate(off, key, index),
                    evaluate_candidate(on, key, index),
                ) {
                    (CandidateOutcome::Accepted(record), CandidateOutcome::ClaimedByCatalogue) => {
                        assert_ne!(sides(on, &record, None).holder, Holder::Grid);
                        claimed += 1;
                    }
                    (CandidateOutcome::Accepted(before), CandidateOutcome::Accepted(after)) => {
                        assert_eq!(before, after);
                        if after.layer() == Layer::E {
                            assert_eq!(sides(on, &after, None).holder, Holder::Grid);
                            kept += 1;
                        }
                    }
                    (before, after) => assert_eq!(before, after, "{key:?} candidate {index}"),
                }
            }
        }
        (claimed, kept)
    });
    rows.into_iter()
        .fold((0, 0), |(a, b), (c, d)| (a + c, b + d))
}

/// The mark sets of `galaxy` (module documentation), in [`SHARES`] shares by index.
fn sample(galaxy: &Galaxy) -> Tally {
    let classes = Classes::of(galaxy);
    let pool = base_records(galaxy, &classes);
    eprintln!(
        "{} base records, {} of them living field stars",
        pool.all.len(),
        pool.living.len()
    );
    assert!(pool.all.len() > 1_000 && pool.living.len() > 10);
    let interval = SupernovaInterval::of_window(SHELL_WINDOW_CAP);
    let shares = in_shares(SHARES, SHARES, |share| {
        let classes = Classes::of(galaxy);
        let mut tally = Tally::default();
        for i in (0..MARK_SETS).skip(share).step_by(SHARES) {
            let target = Target::ALL[usize::try_from(i % 8).unwrap()];
            let record = mark_set(galaxy, &classes, &pool, i);
            let sides = sides(galaxy, &record, None);
            *tally.holders.entry((target, sides.holder)).or_default() += 1;
            let death = exact_death(galaxy, &record);
            if death.is_some_and(|t| interval.contains(t)) {
                tally.deaths_in_interval += 1;
                tally.failed_bounds += u64::from(sides.site.is_none());
            }
            if sides.holder != Holder::Grid && placeable(galaxy, &record, death) {
                *tally
                    .coverage
                    .entry((sides.holder, coverage(&classes, &record, &sides)))
                    .or_default() += 1;
            }
            if let Some(entry) = &sides.entry {
                tally.straddled += u64::from(check_states(entry));
            }
            tally.displaced_variables += u64::from(sides.displaced_variable);
            if i % FORCED_EVERY == 0 {
                let forced =
                    with_forced_margin(target.forced_margin(i), || sides_holder(galaxy, &record));
                tally.forced += 1;
                match (sides.holder, forced) {
                    (Holder::CoreCollapse, Holder::Grid | Holder::LuminousBlueVariable) => {
                        tally.forced_refused += 1;
                    }
                    (Holder::Grid | Holder::LuminousBlueVariable, Holder::CoreCollapse) => {
                        // Only a failed bound can be undone by moving the brackets: the exact
                        // tier then decides it.
                        assert!(sides.site.is_none(), "{:?}", record.id());
                        assert!(
                            death.is_some_and(|t| interval.contains(t)),
                            "{:?}",
                            record.id()
                        );
                        tally.forced_kept += 1;
                    }
                    _ => {}
                }
            }
        }
        tally
    });
    shares.into_iter().fold(Tally::default(), Tally::add)
}

/// [`sides`]'s holder, every side checked.
fn sides_holder(galaxy: &Galaxy, record: &SystemRecord) -> Holder {
    sides(galaxy, record, None).holder
}

/// The cells the walk takes, each with the candidates it reads: the ridges' cells whole, and the
/// nuclear disc's first candidates, as [`base_records`] reads them.
fn walked_cells(galaxy: &Galaxy) -> Vec<(CellKey, u32)> {
    let mut keys: Vec<(CellKey, u32)> =
        ridge_cells(galaxy, &[12_000.0, 20_000.0, 26_000.0, 32_000.0], 48)
            .into_iter()
            .step_by(if WASM { 8 } else { 1 })
            .map(|key| (key, u32::MAX))
            .collect();
    let edge = i32::try_from(Layer::E.cell_size_ly()).unwrap();
    for x in (-512..=512).step_by(usize::try_from(edge).unwrap()) {
        let at =
            GalacticPosition::from_light_years([f64::from(x), 320.0, 0.0]).expect("in the cube");
        keys.push((
            CellKey::containing(Layer::E, &at).expect("a cell of the cube"),
            65_536,
        ));
    }
    keys
}

/// Runs the whole check under `rules` and prints its tally.
fn run(rules: GalaxyRules, name: &str) -> (Tally, (u64, u64)) {
    let on = milky_way(rules);
    eprintln!("{name}: built");
    let off = milky_way(GalaxyRules {
        carve_out: CarveOutRules::Unclaimed,
        ..rules
    });
    eprintln!("{name}: built without the carve-out");
    let walked = walk_cells(&on, &off, &walked_cells(&on));
    eprintln!("{name}: cells walked");
    let tally = sample(&on);
    eprintln!("{name}: mark sets sampled");
    eprintln!(
        "{name}: the grid's cells, {} claimed and {} kept",
        walked.0, walked.1
    );
    eprintln!("{name}: mark sets by target and holder {:?}", tally.holders);
    eprintln!(
        "{name}: kept and placeable, by holder and coverage {:?}",
        tally.coverage
    );
    eprintln!(
        "{name}: {} deaths in the interval, {} refused by a cheap tier (failed bounds); {} forced, \
         {} refused by the forced tiers, {} failed bounds undone; {} entries read on both sides of \
         their death; {} displaced variables kept by the grid",
        tally.deaths_in_interval,
        tally.failed_bounds,
        tally.forced,
        tally.forced_refused,
        tally.forced_kept,
        tally.straddled,
        tally.displaced_variables
    );
    (tally, walked)
}

/// P09.T38's complementarity under every 27 hold (module documentation): every mark set and every
/// record of the cells held by exactly one side, every side the shared test's; every class keeping
/// only what its candidates draw; the reject path, forced, taken by both sides together.
#[test]
#[ignore = "slow: 10⁵ mark sets and the ridges' cells under every 27 hold (10⁴ on wasm)"]
fn the_grid_and_the_catalogue_are_complementary_under_every_27_hold() {
    let (tally, (claimed, kept)) = run(
        GalaxyRules::of_version(crate::GENERATOR_VERSION),
        "every 27 hold",
    );
    assert!(claimed > 0 && kept > floor(10_000), "{claimed} {kept}");
    assert_eq!(tally.holders.values().sum::<u64>(), MARK_SETS);
    for holder in [
        Holder::Grid,
        Holder::CoreCollapse,
        Holder::LuminousBlueVariable,
    ] {
        assert!(
            tally.held_by(holder) > floor(1_000),
            "{holder:?}: {tally:?}"
        );
    }
    for holder in [Holder::CoreCollapse, Holder::LuminousBlueVariable] {
        for coverage in [
            Coverage::OutsideInterval,
            Coverage::NoComponent,
            Coverage::DisplacedDeath,
            Coverage::NoEjectedTerm,
            Coverage::DisplacedVariable,
        ] {
            assert_eq!(
                tally.covered(holder, coverage),
                0,
                "{holder:?} {coverage:?}"
            );
        }
        assert!(
            tally.covered(holder, Coverage::Covered) > floor(100),
            "{holder:?}"
        );
    }
    assert!(tally.forced_refused > floor(100), "{tally:?}");
    assert!(tally.straddled > floor(100), "{tally:?}");
    assert!(tally.displaced_variables > 0, "{tally:?}");
}

/// The same under the 26 arm, for comparison: every side agrees on every mark set; the classes'
/// coverage at 26 is printed, not asserted, since the runaways' and walkaways' deaths join the
/// class only at 27 (P09.T33.b).
#[test]
#[ignore = "slow: 10⁵ mark sets and the ridges' cells under every 26 hold (10⁴ on wasm)"]
fn the_grid_and_the_catalogue_are_complementary_under_every_26_hold() {
    let (tally, (claimed, _)) = run(
        GalaxyRules::of_version(crate::GENERATOR_VERSION),
        "every 26 hold with its retargets",
    );
    assert!(claimed > 0, "{claimed}");
    assert_eq!(tally.holders.values().sum::<u64>(), MARK_SETS);
    assert!(tally.forced_refused > floor(100), "{tally:?}");
}
