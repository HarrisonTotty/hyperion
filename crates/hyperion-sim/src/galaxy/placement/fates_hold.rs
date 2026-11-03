//! The tests of P08.T12's hold at 27, `PLACEMENT_FATES_VERSION` (plan 08, P08.T12 at 27, the
//! redesign research-leans-19 §3 sketched): under every 27 hold a dead record's marks read its
//! progenitor's fate from the fitted fate table where it can stand for the track's, against the
//! same galaxy under the hold's held arm, the exact path.
//!
//! - The table read's premise, over a sweep (slow, over a finer one): the companion-stripped mark
//!   moves no iron core's or electron capture's fate.
//! - At the Sun-like point and about a globular's core, every dead record of layer E is dead at
//!   its age by its exact lifetime, its time since death in its class's age bin but for the
//!   bracket's width; the table reads most of the fates, and none whose route is not the track's.
//! - The two arms' layer-E remnants alike there: two-sample KS tests of their initial masses and
//!   times since death; slow, over more cells, of their remnants' masses and times since death as
//!   the stellar stage builds them.
//! - The same cells twice give the same records.
//!
//! The tripwire is here. The bump to 27 deletes the held arm, and these become tests of the
//! version's own galaxy.

use std::sync::OnceLock;

use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_two_sample};

use super::marks::{FateRead, PLACEMENT_FATES_VERSION, PlacementFateRules, draws_of, table_fate};
use super::{CellKey, SystemRecord, generate_cell};
use crate::GENERATOR_VERSION;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::displaced::class_table::ClassKey;
use crate::galaxy::displaced::marks::age_bin_years;
use crate::galaxy::displaced::{DisplacedKind, PlacementClass};
use crate::galaxy::features::FeatureProcess;
use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord};
use crate::galaxy::features::members::FeatureInterior;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::planetary::testing::in_shares;
use crate::rng::{Mark, Seed};
use crate::stellar::draws::{StandardNormal, StarDraws};
use crate::stellar::fates::{FateRoute, node_draws};
use crate::stellar::sse::{Fate, MAX_INITIAL_MASS, TrackOptions, fate_of, lifetime_under};
use crate::stellar::system::SystemStars;
use crate::stellar::{Composition, Phase};
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

/// The seed of the hold's Milky Way fixtures.
const SEED: u64 = 0x0812_f000_0000_0001;

/// The Milky Way fixture under every 27 hold, the fate table read, with its kinematic tables, which
/// a globular's members need.
fn fitted() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds")
        .with_full_potential()
    })
}

/// The Milky Way fixture under every 27 hold but this one, the exact path, with its kinematic
/// tables.
fn exact() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules {
                placement_fates: PlacementFateRules::Exact,
                ..GalaxyRules::every_27_hold()
            },
        )
        .expect("the Milky Way fixture builds")
        .with_full_potential()
    })
}

/// The tripwire of the hold: below 27 every dead record's fate is its track's.
#[test]
fn the_placement_fates_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < PLACEMENT_FATES_VERSION,
        "the 26 → 27 bump reads a dead record's fate from the fate table where it can stand for \
         the track's: the ages of most of layer E's dead records and of the hypervelocity \
         survivors, and so their cells' records, move. Delete `PLACEMENT_FATES_VERSION`, \
         `PlacementFateRules` with its `Exact` arm, `GalaxyRules::placement_fates`, `fate`'s \
         test of it and this test in the bump's own commit, and re-bless the goldens plan 08's \
         \"P08.T12 at 27, as built\" lists"
    );
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).placement_fates,
        PlacementFateRules::Exact
    );
    assert_eq!(fitted().rules().placement_fates, PlacementFateRules::Fitted);
}

/// The threads a slow test of the hold runs (its `threads-required` in `.config/nextest.toml`).
const SLOW_THREADS: usize = 4;

/// `f` of each of `items`, in their order, on at most `threads` threads ([`in_shares`]; one runs
/// them on this thread): share k takes the items k, k + n, …
fn over<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = threads.max(1);
    let parts = in_shares(n, threads, |k| {
        items.iter().skip(k).step_by(n).map(&f).collect::<Vec<_>>()
    });
    let mut shares: Vec<_> = parts.into_iter().map(Vec::into_iter).collect();
    (0..items.len())
        .map(|i| shares[i % n].next().expect("each share holds its items"))
        .collect()
}

/// The \[Fe/H\] of `Z_FIT_MAX`, the most the formulae read.
const MOST_FE_H: f64 = 0.176;

/// For each of `metallicities`, `etas` and `n` masses over the stripped window, the single
/// star's fate and the companion-stripped one's: where the single star's collapses an iron core
/// or captures electrons, the two are the same, bit for bit. The fates compared there, and the
/// number of the others the mark moved.
fn stripped_mark_agrees(metallicities: &[f64], etas: &[f64], n: u32) -> (u32, u32) {
    let rules = GalaxyRules::every_27_hold().stellar;
    let options = TrackOptions::default().with_rules(rules);
    let (mut compared, mut moved) = (0, 0);
    for &fe_h in metallicities {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        for &eta in etas {
            let single = node_draws(StandardNormal::new(eta).expect("a finite η"));
            let stripped = single.clone().with_stripped(Mark::from_word(0));
            for i in 0..=n {
                let m = SolarMasses::new(5.5 + 5.5 * f64::from(i) / f64::from(n));
                let alone = fate_of(m, &composition, &single, options);
                let with = fate_of(m, &composition, &stripped, options);
                let same = format!("{alone:?}") == format!("{with:?}");
                if alone.iron_core.is_some() || alone.phase == Phase::NeutronStar {
                    assert!(same, "{m:?} at [Fe/H] {fe_h}, η {eta}: {alone:?}, {with:?}");
                    compared += 1;
                } else if !same {
                    moved += 1;
                }
            }
        }
    }
    (compared, moved)
}

/// The table read's premise: over the stripped window, 5.5–11 M☉, at four metallicities
/// and three η, the companion-stripped mark moves no iron core's or electron capture's fate, bit
/// for bit, and does move some white dwarfs', so that the sweep reaches the window.
#[test]
fn the_stripped_mark_moves_no_iron_core_or_electron_capture() {
    let (compared, moved) =
        stripped_mark_agrees(&[-2.3, -0.5, 0.086, MOST_FE_H], &[-3.0, 0.0, 3.0], 40);
    assert!(compared > 200, "{compared}");
    assert!(moved > 0, "{moved}");
}

/// The premise over a finer sweep: 400 masses, eight metallicities and five η.
#[test]
#[ignore = "slow: 16,000 fates, single and stripped"]
fn the_stripped_mark_moves_no_iron_core_or_electron_capture_over_a_finer_sweep() {
    let (compared, moved) = stripped_mark_agrees(
        &[-2.3, -1.5, -1.0, -0.5, -0.25, 0.0, 0.1, MOST_FE_H],
        &[-3.0, -1.5, 0.0, 1.5, 3.0],
        399,
    );
    println!("{compared} iron cores and electron captures the same; {moved} others moved");
    assert!(moved > 0);
}

/// The Sun-like point.
fn sun_like() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube")
}

/// The core of the globular with members nearest the Sun-like point, as the server's benches
/// choose it.
fn globular_core(galaxy: &Galaxy) -> GalacticPosition {
    let sun = sun_like();
    let mut globulars: Vec<FeatureRecord> =
        FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular).collect();
    globulars.sort_by(|a, b| {
        let away = |f: &FeatureRecord| f.position().distance_to(&sun).value();
        away(a).total_cmp(&away(b))
    });
    *globulars
        .iter()
        .find_map(|feature| FeatureInterior::of(galaxy, feature))
        .expect("the galaxy has a globular with members")
        .feature()
        .position()
}

/// The layer-E cells within `reach` cells of the one holding `at` on each axis.
fn block(at: &GalacticPosition, reach: i32) -> Vec<CellKey> {
    let [x, y, z] = CellKey::containing(Layer::E, at)
        .expect("in the cube")
        .gen_cell()
        .to_array();
    let span = -reach..=reach;
    span.clone()
        .flat_map(|dx| {
            let span = span.clone();
            span.clone().flat_map(move |dy| {
                span.clone()
                    .filter_map(move |dz| CellKey::new(Layer::E, [x + dx, y + dy, z + dz]).ok())
            })
        })
        .collect()
}

/// Whether `record` is dead by its marks: retained, or a displaced remnant.
fn is_dead(record: &SystemRecord) -> bool {
    matches!(
        record.placement_class(),
        PlacementClass::Retained { .. }
            | PlacementClass::Displaced {
                kind: DisplacedKind::Remnant,
                ..
            }
    )
}

/// The dead records of `cells` in `galaxy`.
fn dead_records(galaxy: &Galaxy, cells: &[CellKey]) -> Vec<SystemRecord> {
    let mut out = Vec::new();
    let mut cell = Vec::new();
    for &key in cells {
        generate_cell(galaxy, key, &mut cell);
        out.extend(cell.iter().filter(|r| is_dead(r)).copied());
    }
    out
}

/// A dead record's progenitor as its marks read it: its mass held to the tracks' heaviest, its
/// source's reference composition and its draws at attempt 0.
fn progenitor(galaxy: &Galaxy, record: &SystemRecord) -> (SolarMasses, Composition, StarDraws) {
    let m = record.primary_initial_mass();
    let m = if m > MAX_INITIAL_MASS {
        MAX_INITIAL_MASS
    } else {
        m
    };
    let composition = record
        .progenitor_composition(galaxy)
        .expect("a dead record has its progenitor's composition");
    (m, composition, draws_of(galaxy, record.id()))
}

/// A fate's route as the marks read it: the white dwarf it leaves, if any, whether an iron core
/// collapsed, and whether anything is left.
fn route_of(fate: &Fate) -> (Option<Phase>, bool, bool) {
    let white_dwarf = matches!(
        fate.phase,
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
    );
    (
        white_dwarf.then_some(fate.phase),
        fate.iron_core.is_some(),
        fate.remnant.kind() != crate::stellar::remnant::RemnantKind::None,
    )
}

/// The time since death on which a dead record of `galaxy` drew its age: its class's age bin for a
/// thin-disc class, any for an old source's or a retained record.
fn since_death_bin(galaxy: &Galaxy, record: &SystemRecord) -> (f64, Option<f64>) {
    let displaced = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed");
    match record.placement_class() {
        PlacementClass::Displaced { class, .. } => match displaced.class(class).key() {
            ClassKey::Thin { age, .. } => {
                let (lo, hi) = age_bin_years(age, displaced.class_table().scales().tau_unit());
                (lo.value(), hi.map(Years::value))
            }
            ClassKey::Old { .. } | ClassKey::Hypervelocity => (0.0, None),
        },
        PlacementClass::Retained { .. } | PlacementClass::Alive => (0.0, None),
    }
}

/// What [`check_dead_records`] counts.
#[derive(Debug, Clone, Copy, Default)]
struct DeadRecords {
    /// The dead records.
    records: u32,
    /// Those whose fate the table read.
    table: u32,
    /// Those whose fate the table read with a route other than the track's.
    other_route: u32,
    /// The largest share of its lifetime by which a record's time since death passes its bin's.
    worst_excess: f64,
    /// The largest share of its lifetime by which the table's bracket moved a record's death
    /// earlier than its track's.
    worst_shift: f64,
}

impl DeadRecords {
    /// The counts of both.
    fn and(self, other: Self) -> Self {
        Self {
            records: self.records + other.records,
            table: self.table + other.table,
            other_route: self.other_route + other.other_route,
            worst_excess: self.worst_excess.max(other.worst_excess),
            worst_shift: self.worst_shift.max(other.worst_shift),
        }
    }
}

/// Whether the dead `record` of the fitted fixture is dead at its age by its exact lifetime, its
/// time since death at least its bin's least and at most its greatest and the bracket's width
/// more, and, where the table read its fate, whether the route is the track's: its counts.
fn check_dead_record(record: &SystemRecord) -> DeadRecords {
    let galaxy = fitted();
    let rules = galaxy.rules().stellar;
    let (m, composition, draws) = progenitor(galaxy, record);
    let life = lifetime_under(m, &composition, &draws, rules).value();
    let since = record.age_at_epoch().value() - life;
    let (lo, hi) = since_death_bin(galaxy, record);
    assert!(
        since >= lo * (1.0 - 1e-9),
        "{:?}: {since} below {lo}",
        record.id()
    );
    let read = table_fate(m, &composition, &draws);
    let excess = read.map_or(0.0, |read| read.death_bound().0.value() - life);
    let mut counts = DeadRecords {
        records: 1,
        ..DeadRecords::default()
    };
    if let Some(hi) = hi {
        assert!(
            since <= hi * (1.0 + 1e-9) + excess,
            "{:?}: {since} above {hi} and {excess}",
            record.id()
        );
        counts.worst_excess = (since - hi).max(0.0) / life;
    }
    if let Some(FateRead::Table { route, remnant, .. }) = read {
        let track = fate_of(
            m,
            &composition,
            &draws,
            TrackOptions::default().with_rules(rules),
        );
        let table_route = (
            route.white_dwarf_phase(),
            route == FateRoute::IronCore,
            remnant,
        );
        counts.table = 1;
        counts.worst_shift = excess / life;
        counts.other_route = u32::from(table_route != route_of(&track));
    }
    counts
}

/// [`check_dead_record`] of every dead record of `cells` in the fitted fixture, on `threads`.
fn check_dead_records(cells: &[CellKey], threads: usize) -> DeadRecords {
    over(&dead_records(fitted(), cells), threads, check_dead_record)
        .into_iter()
        .fold(DeadRecords::default(), DeadRecords::and)
}

/// At the Sun-like point and about the globular's core, every dead record of layer E is dead at
/// its age by its exact lifetime and its time since death in its class's age bin, but for the
/// bracket's width above it; the table reads most of the fates, and every route it reads is the
/// track's.
#[test]
fn every_dead_record_is_dead_at_its_age_and_every_route_read_is_the_track_s() {
    for (name, at) in [("Sun-like", sun_like()), ("core", globular_core(fitted()))] {
        let counts = check_dead_records(&block(&at, 1), 1);
        println!(
            "{name}: {} dead records, {} read from the table, {} with another route; a death at \
             most {:.2e} of its lifetime earlier, and a time since death at most {:.2e} of it past \
             its bin",
            counts.records,
            counts.table,
            counts.other_route,
            counts.worst_shift,
            counts.worst_excess
        );
        assert!(counts.records > 50, "{name}: {counts:?}");
        assert!(2 * counts.table > counts.records, "{name}: {counts:?}");
        assert_eq!(counts.other_route, 0, "{name}: {counts:?}");
    }
}

/// The initial masses and times since death, by their exact lifetimes, of the dead records of
/// `cells` in `galaxy`.
fn masses_and_deaths(galaxy: &Galaxy, cells: &[CellKey]) -> (Vec<f64>, Vec<f64>) {
    let rules = galaxy.rules().stellar;
    dead_records(galaxy, cells)
        .iter()
        .map(|record| {
            let (m, composition, draws) = progenitor(galaxy, record);
            let life = lifetime_under(m, &composition, &draws, rules).value();
            (
                record.primary_initial_mass().value(),
                record.age_at_epoch().value() - life,
            )
        })
        .unzip()
}

/// The two arms' dead records of layer E are alike at the Sun-like point and about the
/// globular's core: two-sample KS tests of their initial masses and of their times since death.
#[test]
fn the_two_arms_dead_records_have_alike_masses_and_deaths() {
    for (name, at) in [("Sun-like", sun_like()), ("core", globular_core(fitted()))] {
        let cells = block(&at, 1);
        let (mut fitted_masses, mut fitted_deaths) = masses_and_deaths(fitted(), &cells);
        let (mut exact_masses, mut exact_deaths) = masses_and_deaths(exact(), &cells);
        let masses = ks_two_sample(&mut fitted_masses, &mut exact_masses);
        let deaths = ks_two_sample(&mut fitted_deaths, &mut exact_deaths);
        println!(
            "{name}: {} and {} dead records; masses D {:.4} (p {:.3}), deaths D {:.4} (p {:.3})",
            fitted_masses.len(),
            exact_masses.len(),
            masses.statistic,
            masses.p_value,
            deaths.statistic,
            deaths.p_value
        );
        assert_p_value(&format!("{name} initial masses"), masses.p_value, ALPHA);
        assert_p_value(&format!("{name} times since death"), deaths.p_value, ALPHA);
    }
}

/// The remnants' masses and times since death of the dead records of `cells` in `galaxy`, as the
/// stellar stage builds their stars, on [`SLOW_THREADS`].
fn remnants(galaxy: &Galaxy, cells: &[CellKey]) -> (Vec<f64>, Vec<f64>) {
    let records = dead_records(galaxy, cells);
    over(&records, SLOW_THREADS, |record| {
        let stars = SystemStars::generate(galaxy, record);
        let primary = stars.primary();
        (
            primary
                .remnant()
                .expect("a dead record's primary is a remnant")
                .mass()
                .value(),
            primary
                .remnant_age_at_epoch()
                .expect("a dead record's primary is dead at the epoch")
                .value(),
        )
    })
    .into_iter()
    .unzip()
}

/// The two arms' layer-E remnants alike through the stellar stage, over five cells a side at the
/// Sun-like point and three about the globular's core: two-sample KS tests of their remnants'
/// masses and of their times since death, and every dead record's star a remnant at the epoch.
#[test]
#[ignore = "slow: some 3,200 dead records' stars, twice"]
fn the_two_arms_remnants_have_alike_masses_and_deaths_through_the_stellar_stage() {
    for (name, at, reach) in [
        ("Sun-like", sun_like(), 2),
        ("core", globular_core(fitted()), 1),
    ] {
        let cells = block(&at, reach);
        let (mut fitted_masses, mut fitted_deaths) = remnants(fitted(), &cells);
        let (mut exact_masses, mut exact_deaths) = remnants(exact(), &cells);
        let masses = ks_two_sample(&mut fitted_masses, &mut exact_masses);
        let deaths = ks_two_sample(&mut fitted_deaths, &mut exact_deaths);
        println!(
            "{name}: {} and {} remnants; masses D {:.4} (p {:.3}), deaths D {:.4} (p {:.3})",
            fitted_masses.len(),
            exact_masses.len(),
            masses.statistic,
            masses.p_value,
            deaths.statistic,
            deaths.p_value
        );
        assert_p_value(&format!("{name} remnant masses"), masses.p_value, ALPHA);
        assert_p_value(&format!("{name} times since death"), deaths.p_value, ALPHA);
        let counts = check_dead_records(&cells, SLOW_THREADS);
        println!("{name}: {counts:?}");
        assert_eq!(counts.other_route, 0, "{name}: {counts:?}");
    }
}

/// The same cells twice give the same records under the fitted arm.
#[test]
fn the_fitted_arm_gives_the_same_records_twice() {
    let (mut first, mut again) = (Vec::new(), Vec::new());
    for key in block(&sun_like(), 0) {
        generate_cell(fitted(), key, &mut first);
        generate_cell(fitted(), key, &mut again);
        assert_ne!(first.len(), 0);
        assert_eq!(format!("{first:?}"), format!("{again:?}"));
    }
}
