//! The tests of P08.T12.c's conditional marks under every 26 hold (plan 08, P08.T12.c; Design
//! notes 17, 19 and 28): a displaced remnant dead its time since death ago through the stellar
//! stage, with its kick in its bin; a retained record's kick inside its bin; a living layer-E
//! record alive and unconstrained; the kick loop's attempts; the count of `lifetime` calls; and
//! layer E's living share and mass histogram at the Sun-like point against the class table.

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{assert_p_value, chi_square_gof, normal_cdf};

use super::candidate::{Pick, Picked, pick_of};
use super::marks::{self, LIFETIME_CALLS};
use super::{CandidateOutcome, generate_cell, generate_cell_where};
use super::{CellKey, STELLAR_LAYERS, SystemRecord, candidate_count, evaluate_candidate};
use crate::coords::GalacticPosition;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::displaced::class_table::{ClassKey, keeps_own_form};
use crate::galaxy::displaced::hold::{MILKY_WAY_SEED, displaced, held};
use crate::galaxy::displaced::marks::{StayCategory, age_bin_years, kind_index};
use crate::galaxy::displaced::site::{CarveOutRules, explosion_site, explosion_site_knowing};
use crate::galaxy::displaced::{DisplacedKind, PlacementClass, SPEED_EDGES};
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{Layer, SystemIdKind};
use crate::rng::{PiecewiseLinear, Seed};
use crate::stellar::brief::{BriefModel, BriefRoute};
use crate::stellar::multiplicity::{RedrawAttempt, draw_hierarchy_of_composition, draw_star_count};
use crate::stellar::sse::{Track, TrackOptions, lifetime_under};
use crate::stellar::system::{
    KICK_LOOP_CAP, StarModel, SystemStars, draw_metallicity, first_attempt_models,
    grid_multiplicity, primary_draws, primary_kept_stripped_mark, primary_model,
};
use crate::time::UniverseTime;
use crate::units::KilometresPerSecond;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// The Sun-like point.
fn sunlike() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube")
}

/// A point in the plane at 45° to both axes, `r` light-years from the centre.
fn diagonal(r: f64) -> GalacticPosition {
    let half = 0.5_f64.sqrt();
    GalacticPosition::from_light_years([r * half, r * half, 0.0]).expect("in the cube")
}

/// The candidate index of a grid record.
fn index_of(record: &SystemRecord) -> u32 {
    match record.id().kind() {
        SystemIdKind::Grid(grid) => grid.index(),
        other => panic!("a grid record has a grid ID, not {other:?}"),
    }
}

/// `n` cells of `layer` in a row along x from the one holding `at`.
fn row(layer: Layer, at: &GalacticPosition, n: i32) -> Vec<CellKey> {
    let first = CellKey::containing(layer, at).expect("in the cube");
    let [cell_x, cell_y, cell_z] = first.gen_cell().to_array();
    (0..n)
        .map(|dx| CellKey::new(layer, [cell_x + dx, cell_y, cell_z]).expect("in the cube"))
        .collect()
}

/// Every record of `cells` in `galaxy`, with its cell.
fn records(galaxy: &Galaxy, cells: &[CellKey]) -> Vec<(CellKey, SystemRecord)> {
    let mut out = Vec::new();
    let mut cell = Vec::new();
    for &key in cells {
        generate_cell(galaxy, key, &mut cell);
        out.extend(cell.iter().map(|r| (key, *r)));
    }
    out
}

/// The kick speed, km/s, of the primary of `record`'s system.
fn kick_of(stars: &SystemStars) -> Option<KilometresPerSecond> {
    stars
        .primary()
        .natal_kick()
        .map(|kick| KilometresPerSecond::from(kick.speed()))
}

/// A displaced remnant of layer E is dead at the epoch through `SystemStars::generate`, its death
/// the marks' time since death ago to a second (or, for the oldest, to sixteen ulps of its age,
/// whose own rounding is a minute at 10¹⁰ years), that time inside its class's age bin, and its
/// natal kick inside its kick constraint; the same record twice gives the same stars.
#[test]
fn a_displaced_remnant_died_its_time_since_death_ago_with_its_kick_in_its_bin() {
    let galaxy = displaced();
    let fields = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed");
    let table = fields.class_table();
    let cells = row(Layer::E, &sunlike(), 24);
    let mut checked = 0_u32;
    for (key, record) in records(galaxy, &cells) {
        let PlacementClass::Displaced {
            class,
            kind: DisplacedKind::Remnant,
        } = record.placement_class()
        else {
            continue;
        };
        let picked = pick_of(galaxy, key, index_of(&record)).expect("an accepted candidate");
        assert_eq!(picked, Picked::Displaced(class));
        let drawn = marks::draw(
            galaxy,
            fields,
            key,
            record.id(),
            record.epoch_position(),
            picked,
        )
        .expect("a displaced candidate draws marks");
        let since = record.age_at_epoch().value() - drawn.lifetime.expect("computed").value();
        let stars = SystemStars::generate(galaxy, &record);
        let dead_for = stars
            .primary()
            .remnant_age_at_epoch()
            .expect("a displaced remnant's primary is dead at the epoch")
            .value();
        let tolerance = (1.0 / SECONDS_PER_JULIAN_YEAR)
            .max(16.0 * f64::EPSILON * record.age_at_epoch().value());
        assert!(
            (dead_for - since).abs() <= tolerance,
            "{:?}: dead {dead_for} yr, the marks' {since} yr",
            record.id()
        );
        if let ClassKey::Thin { age, .. } = fields.class(class).key() {
            let (lo, hi) = age_bin_years(age, table.scales().tau_unit());
            assert!(since >= lo.value() * (1.0 - 1e-9), "{since} below {lo:?}");
            assert!(
                hi.is_none_or(|hi| since <= hi.value() * (1.0 + 1e-9)),
                "{since} above {hi:?}"
            );
        }
        let constraint = record
            .kick_constraint(galaxy)
            .expect("a displaced remnant has a kick constraint");
        let kick = kick_of(&stars).expect("a displaced remnant was kicked");
        assert!(
            constraint.holds(kick),
            "{:?}: kick {kick:?} outside {:?}",
            record.id(),
            constraint.speed()
        );
        assert_eq!(stars, SystemStars::generate(galaxy, &record));
        assert_eq!(stars.star_count(), 1, "a displaced remnant travels alone");
        checked += 1;
    }
    println!("{checked} displaced remnants checked");
    assert!(checked > 20, "only {checked} displaced remnants");
}

/// A retained record's kick: below a quarter of `v_c` for the thin disc, the thick disc and the
/// halo, and inside its `Retained { speed }` bin for the bulge, the bar and the nuclear disc, whose
/// classes keep their own form in every bin (Design note 13).
#[test]
fn a_retained_record_s_kick_lies_in_its_bin() {
    let galaxy = displaced();
    let table = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed")
        .class_table();
    let v_c = table.scales().v_c().value();
    let mut cells = row(Layer::E, &sunlike(), 8);
    cells.extend(row(Layer::E, &diagonal(3_200.0), 1));
    cells.extend(row(Layer::E, &diagonal(600.0), 1));
    let (mut disc, mut spheroid) = (0_u32, 0_u32);
    for (_, record) in records(galaxy, &cells) {
        let PlacementClass::Retained { speed } = record.placement_class() else {
            continue;
        };
        let source = table.component_source(record.component().expect("a grid record"));
        // A bulge cell holds thousands: a hundred and fifty of each kind of source say as much.
        if (if keeps_own_form(source) {
            spheroid
        } else {
            disc
        }) >= 150
        {
            continue;
        }
        let stars = SystemStars::generate(galaxy, &record);
        let kick =
            kick_of(&stars).expect("a retained remnant has a kick, of zero after a fallback");
        if keeps_own_form(source) {
            let v_ref = table.speed_scale(source).value();
            let lo = if speed.index() == 0 {
                0.0
            } else {
                SPEED_EDGES[speed.index() - 1] * v_ref
            };
            let hi = SPEED_EDGES
                .get(speed.index())
                .map_or(f64::INFINITY, |e| e * v_ref);
            assert!(
                (lo..hi).contains(&kick.value()),
                "{source:?}: {kick:?} outside bin {}",
                speed.index()
            );
            spheroid += 1;
        } else {
            assert_eq!(speed.index(), 0, "{source:?} retains only the slowest bin");
            assert!(kick.value() < 0.25 * v_c, "{source:?}: {kick:?}");
            disc += 1;
        }
    }
    println!("{disc} disc and halo, {spheroid} bulge, bar and nuclear disc retained records");
    assert!(disc > 0 && spheroid > 0);
}

/// Every reader of a constrained record's primary sees the star the kick loop keeps (P08.T12.c):
/// for retained records and displaced remnants of layer E, `SystemStars`' primary is
/// `primary_model`'s, the range brief's is that star's brief (with the first attempt's count on
/// the paired route), the lens masses' primary is it, and plan 11's hierarchy reads its stripped
/// mark. The kept attempt is not the record's own for some of them, and its stripped mark not the
/// record's own for a few, so the test bites. The brief and the system's stars are the same in any
/// order of reading.
#[test]
fn every_reader_of_a_constrained_primary_sees_the_kept_attempt() {
    let galaxy = displaced();
    let mut cells = row(Layer::E, &sunlike(), 8);
    cells.extend(row(Layer::E, &diagonal(3_200.0), 1));
    let (mut retained, mut remnants, mut moved, mut marks) = (0_u32, 0_u32, 0_u32, 0_u32);
    let mut constrained = Vec::new();
    for (_, record) in records(galaxy, &cells) {
        let count = match record.placement_class() {
            PlacementClass::Retained { .. } => &mut retained,
            PlacementClass::Displaced {
                kind: DisplacedKind::Remnant,
                ..
            } => &mut remnants,
            PlacementClass::Alive | PlacementClass::Displaced { .. } => continue,
        };
        if *count >= 60 {
            continue;
        }
        *count += 1;
        assert!(record.kick_range(galaxy).is_some(), "{:?}", record.id());
        constrained.push(record);
        let composition = draw_metallicity(galaxy, &record);
        let (kept, attempts) = primary_model(galaxy, &record, composition);
        let stars = SystemStars::generate(galaxy, &record);
        assert_eq!(stars.primary(), &kept, "{:?}", record.id());
        assert_eq!(
            first_attempt_models(galaxy, &record).first(),
            Some(&kept),
            "the lens mass's primary, {:?}",
            record.id()
        );
        let brief = BriefModel::new(galaxy, &record);
        assert!(
            matches!(brief.route(), BriefRoute::Exact | BriefRoute::Paired),
            "{:?}: {:?}",
            record.id(),
            brief.route()
        );
        let ctx = grid_multiplicity(&record);
        let count = draw_star_count(galaxy, &record, ctx, RedrawAttempt::FIRST);
        for t in [
            UniverseTime::EPOCH,
            UniverseTime::from_julian_years(500_000).expect("inside the clock"),
        ] {
            assert_eq!(
                brief.brief_at(t),
                stars.brief_at(t).map(|b| b.with_star_count(count)),
                "{:?} at {t:?}",
                record.id()
            );
        }
        assert_eq!(
            primary_kept_stripped_mark(galaxy, &record, &composition),
            kept.draws().stripped(),
            "{:?}",
            record.id()
        );
        if stars.attempt().next().is_some() || stars.hierarchy().star_count() > 1 {
            assert_eq!(
                stars.hierarchy(),
                &draw_hierarchy_of_composition(galaxy, &record, &composition, ctx, stars.attempt()),
                "the hierarchy reads the kept mark, {:?}",
                record.id()
            );
        }
        let first = primary_draws(galaxy, &record);
        marks += u32::from(kept.draws().stripped() != first.stripped());
        if attempts > 1 {
            let own = StarModel::new_under(
                record.primary_initial_mass(),
                composition,
                first,
                record.age_at_epoch(),
                galaxy.rules().stellar,
            )
            .expect("a grid record's primary");
            moved += u32::from(own.natal_kick() != kept.natal_kick());
        }
    }
    println!(
        "{retained} retained and {remnants} displaced remnants, {moved} kept another kick, {marks} \
         another stripped mark"
    );
    assert!(retained > 0 && remnants > 0 && moved > 0 && marks > 0);
    let readers = |record: &SystemRecord| {
        (
            BriefModel::new(galaxy, record).brief_at(UniverseTime::EPOCH),
            SystemStars::generate(galaxy, record).primary().clone(),
        )
    };
    let sampled: Vec<SystemRecord> = constrained.iter().step_by(12).copied().collect();
    assert_order_independent(&sampled, readers);
}

/// A living layer-E record is alive at the epoch in the stellar stage, by construction (Design
/// note 19), and constrains no kick; so is every record of layers A to C, whose marks are plan
/// 03's. Some 0.1% of layer E's systems at the Sun-like point are alive, so the living records are
/// drawn from the marks of the component whose stay marks hold the largest living share, for the
/// candidates of a row of cells there.
#[test]
fn a_living_layer_e_record_is_alive_in_the_stellar_stage_and_unconstrained() {
    let galaxy = displaced();
    let fields = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed");
    let table = fields.class_table();
    let young = galaxy
        .fields()
        .component_ids()
        .max_by(|&a, &b| {
            let alive = |c| table.stay_marks(c).odds()[StayCategory::Alive.index()];
            alive(a).total_cmp(&alive(b))
        })
        .expect("the fixture has components");
    let key = CellKey::containing(Layer::E, &sunlike()).expect("in the cube");
    let position =
        GalacticPosition::from_light_years(key.origin_ly().map(f64::from)).expect("in the cube");
    let mut living = 0_u32;
    for index in 0..20_000 {
        let id = key.candidate_id(index).expect("under the capacity");
        let drawn = marks::draw(
            galaxy,
            fields,
            key,
            id,
            &position,
            Picked::Field(Pick::of(young)),
        )
        .expect("a layer-E field candidate draws marks");
        let record = SystemRecord::of_marks(galaxy, id, position, &drawn);
        if record.placement_class() != PlacementClass::Alive {
            continue;
        }
        assert_eq!(record.kick_constraint(galaxy), None);
        let stars = SystemStars::generate(galaxy, &record);
        let primary = stars.primary();
        assert!(
            primary
                .lifetime()
                .is_none_or(|life| life.value() > record.age_at_epoch().value()),
            "{:?} is dead at the epoch",
            record.id()
        );
        assert!(
            primary
                .state_at(UniverseTime::EPOCH)
                .is_none_or(|state| !state.phase().is_remnant()),
            "{:?} is a remnant at the epoch",
            record.id()
        );
        living += 1;
        if living == 200 {
            break;
        }
    }
    println!("{living} living records checked");
    assert!(living >= 50, "only {living} living records");
    for layer in [Layer::A, Layer::B, Layer::C] {
        for (_, record) in records(galaxy, &row(layer, &sunlike(), 2)) {
            assert_eq!(record.placement_class(), PlacementClass::Alive);
            assert_eq!(record.mark_attempt(), 0);
            assert_eq!(record.kick_constraint(galaxy), None);
        }
    }
}

/// `n` members of layer E's displaced classes in the galaxy as a whole, the remnants among them:
/// each class gives its share of `n` by its expected count in the band, its members' marks drawn
/// for consecutive candidates of the cells along x from the Sun-like point, from candidate `skip`
/// on (a remnant's marks read nothing of its position but its height, for a ballistic class's
/// vertical kick, and the cell's corner stands in for it).
fn displaced_remnants(galaxy: &Galaxy, n: u64, skip: u64) -> Vec<SystemRecord> {
    let fields = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed");
    let order = fields.order(MassBand::E);
    let total: f64 = order
        .iter()
        .map(|&k| fields.class(k).count(MassBand::E))
        .sum();
    let first = CellKey::containing(Layer::E, &sunlike()).expect("in the cube");
    let [cell_x, cell_y, cell_z] = first.gen_cell().to_array();
    let capacity = u64::from(first.index_capacity());
    let mut out = Vec::new();
    let mut i = skip;
    for &class in order {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a share of a test's count, rounded, far below 2⁵³"
        )]
        let members = (n as f64 * fields.class(class).count(MassBand::E) / total).round() as u64;
        for _ in 0..members {
            let along = i32::try_from(i / capacity).expect("a test's cells");
            let key =
                CellKey::new(Layer::E, [cell_x + along, cell_y, cell_z]).expect("in the cube");
            let id = key
                .candidate_id(u32::try_from(i % capacity).expect("under the capacity"))
                .expect("under the capacity");
            i += 1;
            let position = GalacticPosition::from_light_years(key.origin_ly().map(f64::from))
                .expect("in the cube");
            let drawn = marks::draw(galaxy, fields, key, id, &position, Picked::Displaced(class))
                .expect("a displaced class draws marks");
            let record = SystemRecord::of_marks(galaxy, id, position, &drawn);
            if matches!(
                record.placement_class(),
                PlacementClass::Displaced {
                    kind: DisplacedKind::Remnant,
                    ..
                }
            ) {
                out.push(record);
            }
        }
    }
    out
}

/// The kick loop's attempts for `records`: their sum, the most any made, and a description of
/// each that reached the cap.
fn kick_attempts(galaxy: &Galaxy, records: &[SystemRecord]) -> (u64, u32, Vec<String>) {
    let mut capped = Vec::new();
    let (sum, most) = records.iter().fold((0, 0), |(sum, most), record| {
        let (model, attempts) = primary_model(galaxy, record, draw_metallicity(galaxy, record));
        if attempts >= KICK_LOOP_CAP {
            capped.push(format!(
                "{:?} {:.3} M_sun {:?} {:?}, kick {:?} against {:?}",
                record.id(),
                record.primary_initial_mass().value(),
                record.placement_class(),
                model.death().map(|death| death.kind()),
                model
                    .natal_kick()
                    .map(|kick| KilometresPerSecond::from(kick.speed())),
                record.kick_constraint(galaxy).map(|c| c.speed()),
            ));
        }
        (sum + u64::from(attempts), most.max(attempts))
    });
    (sum, most, capped)
}

/// The kick loop's mean over layer E's displaced remnants, galaxy-wide by class, is about the
/// number of speed bins, within 6–10 attempts (Design note 17), and none of them reaches the cap.
///
/// From generator version 26, with the kick tables refitted at 26, both are findings, pinned as
/// they stand: one of the 1,997 reaches the cap, 8200B2E0000006A0, a 51.4 M☉ direct collapse
/// whose class asks its black hole for 392–493 km/s, and the others' mean is 10.77 attempts, so
/// the test holds it within 6–11.
#[test]
fn the_kick_loop_s_mean_over_displaced_remnants_is_six_to_ten_attempts() {
    let galaxy = displaced();
    let records = displaced_remnants(galaxy, 2_000, 0);
    let (sum, most, capped) = kick_attempts(galaxy, &records);
    assert_eq!(capped.len(), 1, "{capped:#?}");
    assert!(
        capped[0].starts_with("SystemId(0x8200b2e0000006a0) 51.428 M_sun"),
        "{capped:#?}"
    );
    let uncapped = sum - u64::from(KICK_LOOP_CAP);
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let mean = uncapped as f64 / (records.len() - 1) as f64;
    println!(
        "{} displaced remnants: {mean:.2} attempts on average but for the capped one, at most \
         {most}",
        records.len()
    );
    assert!(records.len() > 1_000, "only {}", records.len());
    assert!(most <= KICK_LOOP_CAP);
    assert!((6.0..=11.0).contains(&mean), "mean {mean}");
}

/// The kick loop over 10⁷ displaced remnants of layer E, galaxy-wide by class (Design note 17),
/// on every core the machine has: its mean is within 6–10 attempts, and the share that reaches the
/// cap is pinned where it stands, 2,081 of 9,996,081 (a finding against the plan's none: iron-core
/// progenitors at 8–14 and 25–42 M☉ whose own track, built with its stripped mark or between the
/// class table's mass nodes, gives their bin almost no weight; their kicks are scaled into it).
/// Natively only: under wasip1 it would take a day on one thread, and the loop's bits are the fast
/// suite's there.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: 10⁷ displaced remnants, each with its track and kick loop"]
fn the_kick_loop_over_ten_million_displaced_remnants() {
    const SYSTEMS: u64 = 10_000_000;
    const PARTS: u64 = 256;
    let galaxy = displaced();
    let workers = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);
    let next = std::sync::atomic::AtomicU64::new(0);
    let (sum, most, count, capped) = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let (mut sum, mut most, mut count) = (0_u64, 0_u32, 0_u64);
                    let mut capped = Vec::new();
                    loop {
                        let part = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if part >= PARTS {
                            return (sum, most, count, capped);
                        }
                        let records = displaced_remnants(
                            galaxy,
                            SYSTEMS / PARTS,
                            part * (SYSTEMS / PARTS) * 2,
                        );
                        let (s, m, c) = kick_attempts(galaxy, &records);
                        sum += s;
                        most = most.max(m);
                        capped.extend(c);
                        count += u64::try_from(records.len()).expect("a count");
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a worker"))
            .fold(
                (0, 0, 0, Vec::new()),
                |(s, m, c, mut k), (s2, m2, c2, k2)| {
                    k.extend(k2);
                    (s + s2, m.max(m2), c + c2, k)
                },
            )
    });
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let mean = sum as f64 / count as f64;
    println!(
        "{count} displaced remnants: {mean:.3} attempts on average, at most {most}; {} reached the \
         cap",
        capped.len()
    );
    for line in capped.iter().take(20) {
        println!("  {line}");
    }
    assert!((6.0..=10.0).contains(&mean), "mean {mean}");
    assert!(
        capped.len() <= 2_081,
        "{} reached the cap, more than the 2,081 pinned",
        capped.len()
    );
}

/// What [`every_kept_stripped_mark_builds_the_track_built`] counts of some records: those whose
/// track's build read the companion-stripped mark, and those whose kept attempt's mark is not the
/// record's own. Each such record's track is built again with the kept mark, and its death must be
/// the same, its kind and age bit for bit.
fn kept_marks_against_their_tracks(galaxy: &Galaxy, records: &[SystemRecord]) -> (u32, u32) {
    let (mut read, mut moved) = (0_u32, 0_u32);
    let rules = galaxy.rules().stellar;
    let options = TrackOptions::default().with_rules(rules);
    for record in records {
        let composition = draw_metallicity(galaxy, record);
        let (kept, _) = primary_model(galaxy, record, composition);
        let first = primary_draws(galaxy, record);
        let Some(track) = kept.track() else { continue };
        if track.companion_mark_read().is_some() {
            read += 1;
            assert_eq!(
                kept.draws().stripped(),
                first.stripped(),
                "{:?}: in the electron-capture band the kept mark is the track's",
                record.id()
            );
        }
        if kept.draws().stripped() == first.stripped() {
            continue;
        }
        moved += 1;
        let m0 = track.initial_mass();
        let own = Track::full_with(m0, &composition, &first, options);
        let other = Track::full_with(
            m0,
            &composition,
            &first.clone().with_stripped(kept.draws().stripped()),
            options,
        );
        let (a, b) = (own.death(), other.death());
        assert_eq!(
            a.map(|d| d.kind()),
            b.map(|d| d.kind()),
            "{:?} at {m0:?}",
            record.id()
        );
        if let (Some(a), Some(b)) = (a, b) {
            assert_same_bits(a.age().value(), b.age().value());
        }
        assert!(
            own == other,
            "{:?}: the whole track, not just its death",
            record.id()
        );
    }
    (read, moved)
}

/// Ruling 93.2 as amended (2026-11-01): the kick loop redraws the companion-stripped mark with the
/// remnant's and the kick's draws, except where the built track read it, in the electron-capture
/// band of 5.5–11 M☉ (`fates::STRIPPED_WINDOW`) where the single star's and the companion-stripped
/// star's windows disagree (`Builder::by_companion_mark`): there the kept mark is the track's.
/// Over 200,000 displaced remnants galaxy-wide by class and the retained records of nine layer-E
/// cells at the Sun-like point and in the bulge, every kept attempt's mark builds the same track as
/// the one built, its death's kind and age bit for bit, on every core the machine has.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: 200,000 displaced remnants and some 500 retained records, each with its track and kick loop"]
fn every_kept_stripped_mark_builds_the_track_built() {
    const SYSTEMS: u64 = 200_000;
    const PARTS: u64 = 100;
    let galaxy = displaced();
    let workers = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);
    let next = std::sync::atomic::AtomicU64::new(0);
    let (mut read, mut moved) = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let (mut read, mut moved) = (0_u32, 0_u32);
                    loop {
                        let part = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if part >= PARTS {
                            return (read, moved);
                        }
                        let records = displaced_remnants(
                            galaxy,
                            SYSTEMS / PARTS,
                            part * (SYSTEMS / PARTS) * 2,
                        );
                        let (r, m) = kept_marks_against_their_tracks(galaxy, &records);
                        read += r;
                        moved += m;
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a worker"))
            .fold((0, 0), |(r, m), (r2, m2)| (r + r2, m + m2))
    });
    let remnants = (read, moved);
    let mut cells = row(Layer::E, &sunlike(), 8);
    cells.extend(row(Layer::E, &diagonal(3_200.0), 1));
    let retained: Vec<SystemRecord> = records(galaxy, &cells)
        .into_iter()
        .map(|(_, record)| record)
        .filter(|r| matches!(r.placement_class(), PlacementClass::Retained { .. }))
        .collect();
    let (r, m) = kept_marks_against_their_tracks(galaxy, &retained);
    read += r;
    moved += m;
    println!(
        "{} retained records and the remnants: {read} tracks read the mark ({} of them \
         remnants), {moved} kept another mark ({} remnants), each the same track",
        retained.len(),
        remnants.0,
        remnants.1
    );
    assert!(read > 0, "the electron-capture band is reached");
    assert!(moved > 0, "some kept mark is not the record's own");
}

/// Design note 28's count: placement calls `lifetime` for no record of layers A to C, at most
/// twice for each accepted layer-E record and never for a candidate the pick thins, and for under
/// a tenth of layer D's records at the Sun-like point. A candidate dropped after its marks, by the
/// field's share φ (P09.T2.c), a catalogue class's claim (P09.T35) or the loss cone, made its
/// marks' calls, at most two as well.
#[test]
fn placement_calls_lifetime_twice_per_layer_e_record_at_most_and_for_few_of_layer_d() {
    let galaxy = displaced();
    let calls = || LIFETIME_CALLS.with(std::cell::Cell::get);
    for spec in STELLAR_LAYERS {
        let layer = spec.layer();
        let keys = row(layer, &sunlike(), if layer == Layer::E { 8 } else { 2 });
        let (mut records, mut with_calls, mut most) = (0_u32, 0_u32, 0_u64);
        let mut dropped = 0_u32;
        for key in keys {
            for index in 0..candidate_count(galaxy, key) {
                let before = calls();
                let outcome = evaluate_candidate(galaxy, key, index);
                let made = calls() - before;
                match outcome {
                    CandidateOutcome::Accepted(_) => {
                        records += 1;
                        with_calls += u32::from(made > 0);
                        most = most.max(made);
                    }
                    CandidateOutcome::Thinned
                    | CandidateOutcome::ClaimedByCatalogue
                    | CandidateOutcome::InLossCone => {
                        if pick_of(galaxy, key, index).is_none() {
                            assert_eq!(made, 0);
                        } else {
                            dropped += u32::from(made > 0);
                            most = most.max(made);
                        }
                    }
                }
            }
        }
        if dropped > 0 {
            println!(
                "layer {}: {dropped} candidates dropped after their marks called lifetime",
                layer.letter()
            );
        }
        println!(
            "layer {}: {with_calls} of {records} records call lifetime, at most {most} each",
            layer.letter()
        );
        match layer {
            Layer::A | Layer::B | Layer::C => assert_eq!(with_calls + dropped, 0),
            Layer::D => assert!(with_calls * 10 < records, "{with_calls} of {records}"),
            Layer::E => assert!(most <= 2, "{most}"),
            Layer::BrownDwarf | Layer::RoguePlanet => unreachable!("stellar layers only"),
        }
        assert!(records > 0);
    }
}

/// The mass bins of the living-share and histogram test: edges, M☉, over layer E's band.
const MASS_EDGES: [f64; 6] = [8.0, 10.0, 13.0, 18.0, 28.0, 50.0];

/// The mass bin of `m`, M☉: 0 to [`MASS_EDGES`]'s length, the last above 50.
fn mass_bin(m: f64) -> usize {
    MASS_EDGES.iter().filter(|&&edge| m >= edge).count() - 1
}

/// Design note 19's rescaling: at the Sun-like point, layer E's records alive at the epoch by
/// their own lifetimes (at the composition and draws the stellar stage reads), against the class
/// table's living share there (the field's stay marks' alive weight and the displaced classes'
/// runaways and walkaways), and the living records' masses against the table's densities, each by
/// chi-square at the 1% level. Some 0.1% of layer E's systems
/// there are alive, so it reads 10,000 cells, a block of 100 × 100 in the plane about it, each
/// record's expectation taken at its own position.
///
/// The marks are the budget's, so the field share φ (P09.T2.c), which thins the young field after
/// them, and the carve-out (P09.T35), whose luminous blue variables take living hosts, are held
/// back ([`marks_rules`]); `the_field_share_thins_layer_e_s_living_field_records_by_age` holds
/// φ's composition.
#[test]
#[ignore = "slow: 10,000 layer-E cells at the Sun-like point, some 300,000 records"]
fn layer_e_s_living_share_and_masses_at_the_sun_match_the_class_table() {
    let budget = milky_way_under(marks_rules());
    let galaxy = &budget;
    let fields = galaxy.fields();
    let classes = fields.displaced().expect("the classes are installed");
    let table = classes.class_table();
    let shares = galaxy.shares();
    let (mut alive, mut expected_alive) = (0_u64, 0.0);
    let mut observed = [0_u64; MASS_EDGES.len()];
    let mut expected = [0.0; MASS_EDGES.len()];
    let block = sun_block();
    let mut sampled = 0_usize;
    for (_, record) in records(galaxy, &block) {
        sampled += 1;
        let p = PointLy::from(record.epoch_position());
        // Each source of the record's pick, weighted as the thinning weighted it.
        let mut parts: Vec<(f64, f64, [f64; MASS_EDGES.len()])> = Vec::new();
        for (c, component) in fields.component_ids().zip(fields.components()) {
            let weight = shares.component_share(MassBand::E, component) * component.density(&p);
            if weight <= 0.0 {
                continue;
            }
            let marks = table.stay_marks(c);
            let odds = marks.odds()[StayCategory::Alive.index()];
            let hist = marks
                .mass_density(StayCategory::Alive)
                .map_or([0.0; MASS_EDGES.len()], |density| histogram(&density));
            parts.push((weight, odds, hist));
        }
        for &id in classes.order(MassBand::E) {
            let weight = shares.displaced_share(MassBand::E, id) * classes.density(id, &p);
            if weight <= 0.0 {
                continue;
            }
            let marks = table.marks(MassBand::E, id);
            let kinds = marks.kind_odds();
            let mut hist = [0.0; MASS_EDGES.len()];
            let mut odds = 0.0;
            for kind in [DisplacedKind::Runaway, DisplacedKind::Walkaway] {
                let kind_odds = kinds[kind_index(kind)];
                if kind_odds <= 0.0 {
                    continue;
                }
                odds += kind_odds;
                if let Ok(density) = marks.mass_density(kind) {
                    for (share, part) in hist.iter_mut().zip(histogram(&density)) {
                        *share += kind_odds * part;
                    }
                }
            }
            if odds > 0.0 {
                for share in &mut hist {
                    *share /= odds;
                }
            }
            parts.push((weight, odds, hist));
        }
        let total: f64 = parts.iter().map(|&(weight, _, _)| weight).sum();
        let p_alive: f64 = parts
            .iter()
            .map(|&(weight, alive, _)| weight * alive)
            .sum::<f64>()
            / total;
        expected_alive += p_alive;
        let life = lifetime_under(
            record.primary_initial_mass(),
            &draw_metallicity(galaxy, &record),
            &primary_draws(galaxy, &record),
            galaxy.rules().stellar,
        );
        let is_alive = life.value() > record.age_at_epoch().value();
        if is_alive {
            alive += 1;
            observed[mass_bin(record.primary_initial_mass().value())] += 1;
            let living: f64 = parts.iter().map(|&(weight, alive, _)| weight * alive).sum();
            for &(weight, alive, hist) in &parts {
                for (count, share) in expected.iter_mut().zip(hist) {
                    *count += weight * alive * share / living;
                }
            }
        }
    }
    println!(
        "{alive} alive against {expected_alive:.1}; masses {observed:?} against {expected:.1?}"
    );
    let total = u64::try_from(sampled).expect("a count");
    #[expect(clippy::cast_precision_loss, reason = "a count far below 2⁵³")]
    let share = chi_square_gof(
        &[alive, total - alive],
        &[expected_alive, total as f64 - expected_alive],
    );
    assert_p_value("layer E's living share at the Sun", share.p_value, 0.01);
    let fit = chi_square_gof(&observed, &expected);
    assert_p_value("layer E's living masses at the Sun", fit.p_value, 0.01);
}

/// Every 26 hold with its retargets but the field share and the carve-out, which thin and claim
/// layer E's records after their marks: the rules under which the marks are the budget's.
const fn marks_rules() -> GalaxyRules {
    GalaxyRules {
        field_share: FieldShareRules::None,
        carve_out: CarveOutRules::Unclaimed,
        ..GalaxyRules::every_26_hold_with_its_retargets()
    }
}

/// Layer E's 10,000 cells about the Sun-like point, a block of 100 × 100 in the plane.
fn sun_block() -> Vec<CellKey> {
    let sun = CellKey::containing(Layer::E, &sunlike()).expect("in the cube");
    let [cell_x, cell_y, cell_z] = sun.gen_cell().to_array();
    (-50..50)
        .flat_map(|dx| (-50..50).map(move |dy| (dx, dy)))
        .map(|(dx, dy)| {
            CellKey::new(Layer::E, [cell_x + dx, cell_y + dy, cell_z]).expect("in the cube")
        })
        .collect()
}

/// P09.T2.c's composition with the marks: under the field share a living field record of layer E
/// is kept with its field's share at its own age, the lowest slice of its acceptance mark, and a
/// displaced one always, so over every fourth cell of [`sun_block`] the living field records
/// number the sum of those shares, by a normal test at the 1% level.
#[test]
#[ignore = "slow: 2,500 layer-E cells at the Sun-like point, twice"]
fn the_field_share_thins_layer_e_s_living_field_records_by_age() {
    let budget = milky_way_under(marks_rules());
    let phi = milky_way_under(GalaxyRules {
        field_share: FieldShareRules::Phi,
        ..marks_rules()
    });
    let (kept, mean, variance) = living_field_under_phi(&budget, &phi, &sun_block());
    let z = (f64::from(kept) - mean) / variance.sqrt();
    println!("under the field share: {kept} living field records against {mean:.1}, z = {z:.2}");
    assert!(kept > 0, "the field keeps some living records");
    assert_p_value(
        "the living field records under the field share",
        2.0 * normal_cdf(-z.abs()),
        0.01,
    );
}

/// The Milky Way fixture of the displaced classes' tests under `rules`.
fn milky_way_under(rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(
        Seed::new(MILKY_WAY_SEED),
        GalaxyParams::milky_way_like(),
        rules,
    )
    .expect("the Milky Way fixture builds its displaced classes")
}

/// Over every fourth cell of `block`: the living field records of `phi`, the field share's
/// galaxy, and the mean and variance of their count from `budget`'s, the same galaxy without the
/// field share, each kept with its field's share at its own age.
fn living_field_under_phi(budget: &Galaxy, phi: &Galaxy, block: &[CellKey]) -> (u32, f64, f64) {
    let field = phi.field_share().expect("the field share is installed");
    let living = |cell: &[SystemRecord]| {
        cell.iter()
            .filter(|r| r.placement_class() == PlacementClass::Alive)
            .copied()
            .collect::<Vec<_>>()
    };
    let (mut kept, mut mean, mut variance) = (0_u32, 0.0, 0.0);
    let mut cell = Vec::new();
    for &key in block.iter().step_by(4) {
        generate_cell(budget, key, &mut cell);
        for record in living(&cell) {
            let component = record.component().expect("a grid record");
            let f = field.keeps(component, MassBand::E, record.age_at_epoch());
            mean += f;
            variance += f * (1.0 - f);
        }
        generate_cell(phi, key, &mut cell);
        kept += u32::try_from(living(&cell).len()).expect("a count");
    }
    (kept, mean, variance)
}

/// The share of a mass density in each of [`MASS_EDGES`]' bins.
fn histogram(density: &PiecewiseLinear) -> [f64; MASS_EDGES.len()] {
    let total = density.integral(density.lo(), density.hi());
    core::array::from_fn(|i| {
        let lo = MASS_EDGES[i];
        let hi = MASS_EDGES.get(i + 1).copied().unwrap_or(density.hi());
        density.integral(lo.max(density.lo()), hi.min(density.hi()).max(lo)) / total
    })
}

/// Under the held arm the version's galaxy draws plan 03's marks: every record is alive, at
/// attempt 0, with no kick constraint, and its metallicity is read at its epoch position and age.
#[test]
fn the_version_s_records_keep_plan_03_s_marks() {
    let galaxy = held();
    for layer in [Layer::D, Layer::E] {
        for (_, record) in records(galaxy, &row(layer, &sunlike(), 2)) {
            assert_eq!(record.placement_class(), PlacementClass::Alive);
            assert_eq!(record.mark_attempt(), 0);
            assert_eq!(record.kick_constraint(galaxy), None);
            assert_eq!(record.metallicity_age(), None);
            assert_eq!(
                record.formation_site(galaxy),
                PointLy::from(record.epoch_position())
            );
        }
    }
}

/// Under every 26 hold `generate_cell_where` is `generate_cell` filtered by mass in layers D and E
/// too, where the conditional marks draw the mass after the pick and so every candidate is
/// evaluated whole (P08.T12.c), in the same order.
#[test]
fn generate_cell_where_is_generate_cell_filtered_under_every_26_hold() {
    let galaxy = displaced();
    let mut whole = Vec::new();
    let mut kept = Vec::new();
    for layer in [Layer::D, Layer::E, Layer::C] {
        for key in row(layer, &sunlike(), 3) {
            for floor in [0.0, 3.0, 9.0, 20.0, 200.0] {
                generate_cell(galaxy, key, &mut whole);
                whole.retain(|r| r.primary_initial_mass().value() >= floor);
                generate_cell_where(galaxy, key, |m| m.value() >= floor, &mut kept);
                assert_eq!(kept, whole, "{key:?} above {floor} M_sun");
            }
        }
    }
}

/// P08.T13's inheritance 3: the lifetime a layer-E record's marks computed and pass to the
/// carve-out hook is the one the hook's exact step would compute, so the site it gives is the
/// one `explosion_site` gives: for the records of a row of cells at the Sun-like point, and for
/// members of the youngest thin classes, dead under 0.1 `R_d ÷ v_c`, whose deaths fall in the
/// interval.
#[test]
fn the_hook_s_site_from_the_marks_lifetime_is_the_exact_step_s() {
    let galaxy = displaced();
    let fields = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed");
    let mut cases: Vec<(CellKey, SystemRecord, Picked)> = Vec::new();
    for (key, record) in records(galaxy, &row(Layer::E, &sunlike(), 6)) {
        let picked = pick_of(galaxy, key, index_of(&record)).expect("an accepted candidate");
        cases.push((key, record, picked));
    }
    let key = CellKey::containing(Layer::E, &sunlike()).expect("in the cube");
    let corner =
        GalacticPosition::from_light_years(key.origin_ly().map(f64::from)).expect("in the cube");
    let mut index = 2_000_000;
    for &class in fields.order(MassBand::E) {
        if !matches!(fields.class(class).key(), ClassKey::Thin { age, .. } if age.index() == 0) {
            continue;
        }
        for _ in 0..40 {
            let id = key.candidate_id(index).expect("under the capacity");
            index += 1;
            let drawn = marks::draw(galaxy, fields, key, id, &corner, Picked::Displaced(class))
                .expect("a displaced class draws marks");
            cases.push((
                key,
                SystemRecord::of_marks(galaxy, id, corner, &drawn),
                Picked::Displaced(class),
            ));
        }
    }
    let (mut sites, mut compared) = (0, 0);
    for (key, record, picked) in cases {
        let drawn = marks::draw(
            galaxy,
            fields,
            key,
            record.id(),
            record.epoch_position(),
            picked,
        )
        .expect("a layer-E candidate draws marks");
        let given = explosion_site_knowing(galaxy, &record, drawn.lifetime);
        assert_eq!(given, explosion_site(galaxy, &record), "{:?}", record.id());
        sites += usize::from(given.is_some());
        compared += 1;
    }
    println!("{compared} records compared, {sites} with a site in the interval");
    assert!(sites > 0, "the youngest remnants die in the interval");
}
