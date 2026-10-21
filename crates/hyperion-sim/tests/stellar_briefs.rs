//! A range query's brief of a system (plan 06, P06.T38.e; plan 11, P11.T11.a): pinned briefs across
//! the five layers and the routes, over random systems the brief of each route is the full
//! system's, the summary's primary, bit for bit (rulings 89 and 90), and in now mode a row reads a
//! primary its pair has stripped, merged or fed as the summary does, before, during and after.

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell, resolve};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::stellar::binary::{Component, Segment, SegmentKind};
use hyperion_sim::stellar::brief::{BriefModel, BriefRoute};
use hyperion_sim::stellar::multiplicity::{
    MultiplicityContext, RedrawAttempt, StarIndex, draw_star_count,
};
use hyperion_sim::stellar::system::{StarSummary, StellarBrief, SystemStars, grid_multiplicity};
use hyperion_sim::stellar::{ObjectKind, Phase};
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::Years;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

const SEED: u64 = 0x0600_0038_e000_5eed;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The cell of `layer` near the point 26,000 ly out along +y, shifted by `step` cells along x and
/// `lift` along y.
fn solar_cell(layer: Layer, step: i32, lift: i32) -> CellKey {
    let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
    CellKey::new(layer, [step, 26_000 / size + lift, 0]).expect("a cell of the grid")
}

/// The first record of `layer` near the solar circle that `keep` accepts, `skip` of them passed
/// over.
fn first_of(
    galaxy: &Galaxy,
    layer: Layer,
    skip: usize,
    keep: impl Fn(&SystemRecord) -> bool,
) -> SystemRecord {
    let mut cell = Vec::new();
    let mut seen = 0;
    for step in 0..10_000 {
        generate_cell(galaxy, solar_cell(layer, step, 0), &mut cell);
        for record in cell.drain(..) {
            if keep(&record) {
                if seen == skip {
                    return record;
                }
                seen += 1;
            }
        }
    }
    panic!("no such record of layer {layer:?} near the Sun")
}

fn years(y: i64) -> UniverseTime {
    UniverseTime::from_julian_years(y).expect("inside the clock")
}

/// The kind of `record`'s primary at the epoch as its single-star self (the system of its primary
/// alone), which chose the pinned giant before P11.T11.a, so that the pinned systems stay the same.
fn kind_at_epoch(galaxy: &Galaxy, record: &SystemRecord) -> Option<ObjectKind> {
    SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle)
        .brief_at(UniverseTime::EPOCH)
        .map(|b| b.kind())
}

/// `route`'s place in a tally of the four.
fn route_index(route: BriefRoute) -> usize {
    match route {
        BriefRoute::MainSequence => 0,
        BriefRoute::Table => 1,
        BriefRoute::Exact => 2,
        BriefRoute::Paired => 3,
    }
}

/// The brief's kind, class, and the bits of its log L and `T_eff`: all of it but its star count.
fn state_of(brief: &StellarBrief) -> (ObjectKind, String, Option<u64>, u64) {
    (
        brief.kind(),
        brief.class().to_string(),
        brief.log_luminosity().map(|l| bits(l.value())),
        bits(brief.effective_temperature().value()),
    )
}

/// The pinned systems: two, two, two, two and two of layers A to E near the solar circle, another
/// object below 0.1 M☉, and a living giant or subgiant, which between them take both routes and every kind
/// of exact row (the cooling fits, a living evolved star and the dead).
fn pinned(galaxy: &Galaxy) -> Vec<(String, SystemRecord)> {
    let mut out = Vec::new();
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        for skip in 0..2 {
            out.push((
                format!("{layer:?}"),
                first_of(galaxy, layer, skip, |_| true),
            ));
        }
    }
    let taken: Vec<_> = out.iter().map(|(_, record)| record.id()).collect();
    out.push((
        "A below 0.1 M_sun".to_owned(),
        first_of(galaxy, Layer::A, 0, |r| {
            r.primary_initial_mass().value() < 0.1 && !taken.contains(&r.id())
        }),
    ));
    out.push((
        "C giant".to_owned(),
        first_of(galaxy, Layer::C, 0, |r| {
            matches!(
                kind_at_epoch(galaxy, r),
                Some(ObjectKind::Giant | ObjectKind::Subgiant)
            )
        }),
    ));
    out
}

fn write_brief(w: &mut GoldenWriter, brief: &StellarBrief) {
    w.line(&format!(
        "  {:?} {} in {} stars",
        brief.kind(),
        brief.class(),
        brief.star_count()
    ));
    match brief.log_luminosity() {
        Some(l) => w.f64("  log L", l.value()),
        None => w.line("  log L none"),
    }
    w.f64("  Teff", brief.effective_temperature().value());
}

/// Golden briefs of the pinned systems at t = −500, 0 and +500 years, new at generator version 11
/// (P06.T38.e): the route each takes, and each brief, which the test checks against the full
/// system's first, the summary's primary since P11.T11.a.
#[test]
fn briefs_are_pinned() {
    let galaxy = milky_way();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (label, record) in pinned(&galaxy) {
        let model = BriefModel::new(&galaxy, &record);
        let stars = SystemStars::generate(&galaxy, &record);
        w.line("");
        w.u64_hex(&format!("{label} system"), record.id().raw());
        w.f64("initial mass", record.primary_initial_mass().value());
        w.f64("age at epoch", record.age_at_epoch().value());
        w.line(&format!("route {:?}", model.route()));
        for y in [-500, 0, 500] {
            let brief = model.brief_at(years(y));
            let exact = stars.brief_at(years(y));
            if model.route() == BriefRoute::Table {
                let (a, b) = (brief.expect("dead"), exact.expect("dead"));
                assert_eq!(
                    (a.kind(), a.class().to_string()),
                    (b.kind(), b.class().to_string()),
                    "{label}"
                );
            } else {
                assert_eq!(brief, exact, "{label} at {y} yr");
            }
            match brief {
                Some(brief) => {
                    w.line(&format!("t = {y} yr:"));
                    write_brief(&mut w, &brief);
                }
                None => w.line(&format!("t = {y} yr: not yet formed")),
            }
        }
    }
    golden!("stellar/briefs", w.as_str());
}

/// `n` records from random generation cells within 4,000 ly of the solar circle's point along x
/// and 2,000 ly along y, up to three of a cell, the layers in turn.
fn sample_records(galaxy: &Galaxy, seed: u64, n: usize) -> Vec<SystemRecord> {
    let mut rng = Lcg::new(seed);
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];
    let mut cell = Vec::new();
    let mut records = Vec::with_capacity(n + 3);
    let mut visited = 0;
    while records.len() < n {
        let layer = layers[visited % layers.len()];
        visited += 1;
        let size = u64::from(layer.cell_size_ly());
        let (along, across) = (4_000 / size, 2_000 / size);
        let step = i32::try_from(rng.next_u64() % (2 * along + 1)).expect("small")
            - i32::try_from(along).expect("small");
        let rise = i32::try_from(rng.next_u64() % (2 * across + 1)).expect("small")
            - i32::try_from(across).expect("small");
        generate_cell(galaxy, solar_cell(layer, step, rise), &mut cell);
        records.extend(cell.drain(..).take(3));
    }
    records
}

/// Over `n` random systems at the epoch, both ends of the window, a random time in it and 200,000
/// years before it: the exact, main-sequence and paired routes' briefs are the full system's, the
/// summary's primary, bit for bit, with the first attempt's star count; the table's have its kind
/// and class exactly, and a luminosity and temperature inside their stated error (ruling 90.4).
/// Every route is taken. Returns the table rows.
fn check_briefs(seed: u64, n: usize) -> usize {
    let galaxy = milky_way();
    let mut routes = [0_usize; 4];
    let mut table_rows = 0;
    let mut rng = Lcg::new(seed ^ 0x7469_6d65);
    for record in sample_records(&galaxy, seed, n) {
        let model = BriefModel::new(&galaxy, &record);
        let stars = SystemStars::generate(&galaxy, &record);
        routes[route_index(model.route())] += 1;
        let first = draw_star_count(
            &galaxy,
            &record,
            grid_multiplicity(&record),
            RedrawAttempt::FIRST,
        );
        assert_eq!(model.star_count(), first, "{record:?}");
        if stars.attempt() == RedrawAttempt::FIRST {
            assert_eq!(first, stars.star_count(), "{record:?}");
        }
        let y = i64::try_from(rng.next_u64() % 2_001).expect("small") - 1_000;
        for t in [
            years(y),
            UniverseTime::EPOCH,
            ClockWindow::START,
            ClockWindow::END,
            years(-200_000),
        ] {
            let (fitted, exact) = (model.brief_at(t), stars.brief_at(t));
            if model.route() == BriefRoute::Table {
                if t < ClockWindow::START {
                    continue;
                }
                let (fitted, exact) = (fitted.expect("dead, so formed"), exact.expect("formed"));
                assert_eq!(
                    (fitted.kind(), fitted.class().to_string()),
                    (exact.kind(), exact.class().to_string()),
                    "{record:?} at {t}"
                );
                table_rows += 1;
                if let Some(stated) = model.stated_error_at(t) {
                    let log_l = exact.log_luminosity().expect("lit").value();
                    let teff = exact.effective_temperature().value();
                    let (lo, hi) = stated.log_luminosity;
                    let (cold, hot) = stated.effective_temperature;
                    assert!(
                        (lo..=hi).contains(&log_l) && (cold..=hot).contains(&teff),
                        "{record:?} at {t}: log L {log_l}, T_eff {teff} outside {stated:?}"
                    );
                }
                continue;
            }
            assert_eq!(
                fitted.as_ref().map(state_of),
                exact.as_ref().map(state_of),
                "{record:?} at {t}"
            );
            assert_eq!(
                fitted.map(|b| b.star_count()),
                exact.map(|_| first),
                "{record:?} at {t}"
            );
        }
    }
    assert!(routes.iter().all(|&r| r > 0), "{routes:?}");
    table_rows
}

#[test]
fn briefs_are_the_full_systems_over_random_systems() {
    check_briefs(0x6272_6566, 300);
}

#[test]
#[ignore = "slow: 3 × 10⁴ random systems, each generated in full"]
fn briefs_are_the_full_systems_over_thirty_thousand_systems() {
    let rows = check_briefs(0x6272_6567, 30_000);
    println!("{rows} table-routed rows, every one inside its stated error");
}

/// What a pair the engine ran does to its system's primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// Its companion strips it: it donates by Roche-lobe overflow, or a common envelope ejects
    /// an envelope.
    Stripped,
    /// It accretes its companion's overflow on its main sequence, and is rejuvenated.
    Rejuvenated,
    /// It merges with its companion.
    Merged,
}

/// When the engine's pair makes `change` of `stars`' primary, the ages, years, from the start of
/// that segment of its timeline to the start of the next: its merger, at its age (both ages); the
/// first segment that strips it; or the first in which it accretes its companion's overflow on its
/// hydrogen or helium main sequence.
fn change_in(stars: &SystemStars, change: Change) -> Option<(f64, f64)> {
    let pair = stars
        .pairs()
        .iter()
        .find(|pair| pair.stars()[0] == StarIndex::PRIMARY)?;
    let timeline = pair.timeline();
    if change == Change::Merged {
        return timeline.merger_age().map(|t| (t.value(), t.value()));
    }
    let segments = timeline.segments();
    segments.iter().enumerate().find_map(|(i, segment)| {
        let onset = segment.start();
        let found = match (change, segment.kind()) {
            (
                Change::Stripped,
                SegmentKind::StableTransfer {
                    donor: Component::Primary,
                }
                | SegmentKind::CommonEnvelope,
            ) => true,
            (
                Change::Rejuvenated,
                SegmentKind::StableTransfer {
                    donor: Component::Secondary,
                },
            ) => matches!(
                timeline.state_at(Years::new(onset.value() + 1.0)).stars()[0].phase(),
                Phase::MainSequence | Phase::HeliumMainSequence
            ),
            _ => false,
        };
        let end = segments.get(i + 1).map_or(timeline.until(), Segment::start);
        found.then(|| (onset.value(), end.value()))
    })
}

/// Two primaries of the test galaxy whose companions, grown past them after a case-A transfer,
/// overflow back onto them while they are still on their main sequences (24.5 and 19.3 M☉ then),
/// found among the 4,000 systems of 150 layer-E cells near the solar circle, two of them: the
/// engine feeds a main-sequence primary that rarely.
const REJUVENATED: [u64; 2] = [0x8211_b2e0_0000_0010, 0x826c_b2e0_0000_0013];

/// `record` with its age at the epoch `age` years and nothing else changed: a real record whose
/// history moves on the clock, so that one of its ages falls at the epoch.
fn re_aged(record: &SystemRecord, age: f64) -> SystemRecord {
    SystemRecord::from_parts(
        record.id(),
        *record.epoch_position(),
        record.origin(),
        record.population(),
        record.primary_initial_mass(),
        Years::new(age),
    )
}

/// The now row `row` is the summary's primary `primary`: the same kind and class, the same
/// luminosity and temperature to the bit, and the first attempt's star count `count`.
fn assert_row_is_the_summary(
    row: Option<StellarBrief>,
    primary: Option<&StarSummary>,
    count: u8,
    what: &str,
) {
    let (Some(row), Some(primary)) = (row, primary) else {
        assert!(
            row.is_none() && primary.is_none(),
            "{what}: a row {row:?} with a primary {primary:?}"
        );
        return;
    };
    assert_eq!(
        (row.kind(), row.class()),
        (primary.kind(), primary.classification()),
        "{what}"
    );
    let state = primary.state();
    assert_eq!(
        bits(row.effective_temperature().value()),
        bits(state.effective_temperature().value()),
        "{what}: T_eff"
    );
    let luminosity = state.luminosity().value();
    match row.log_luminosity() {
        Some(log) => assert_eq!(
            bits(log.value()),
            bits(math::log10(luminosity)),
            "{what}: log L"
        ),
        None => assert!(luminosity <= 0.0, "{what}: L = {luminosity} L☉ has no log"),
    }
    assert_eq!(row.star_count(), count, "{what}");
}

/// P11.T11.a: in now mode a range row reads the primary its pair has made, as the summary does.
/// Two systems of each change, a primary its companion strips and one that merges (the first of
/// layer E near the solar circle whose change comes after 25,000 years) and the two primaries
/// their companions feed ([`REJUVENATED`]), are each re-aged three times, a real record with only
/// its age at the epoch changed: so that the epoch falls 5,000 years before the change, in the
/// middle of it, and 20,000 years after it ends. At both ends of the clock window and at the
/// epoch, each row's model's brief is the summary's primary then, bit for bit, with the first
/// attempt's star count, and the model takes the paired route during and after. Some rows are not
/// the class their primary's single-star track has then, which they read before P11.T11.a.
#[test]
fn now_rows_read_the_pair_before_during_and_after_it_changes_the_primary() {
    let galaxy = milky_way();
    let mut found: Vec<(Change, SystemRecord, (f64, f64))> = Vec::new();
    for id in REJUVENATED {
        let record = resolve(&galaxy, SystemId::from_raw(id).expect("a system ID"))
            .expect("a grid system of the test galaxy");
        let ages = change_in(
            &SystemStars::generate(&galaxy, &record),
            Change::Rejuvenated,
        )
        .unwrap_or_else(|| panic!("{id:#x}: its companion feeds it on its main sequence"));
        found.push((Change::Rejuvenated, record, ages));
    }
    let mut cell = Vec::new();
    'search: for step in 0..100 {
        generate_cell(&galaxy, solar_cell(Layer::E, step, 0), &mut cell);
        for record in cell.drain(..) {
            let stars = SystemStars::generate(&galaxy, &record);
            for change in [Change::Stripped, Change::Merged] {
                if found.iter().filter(|(c, ..)| *c == change).count() == 2 {
                    continue;
                }
                if let Some(ages) = change_in(&stars, change).filter(|&(onset, _)| onset > 25_000.0)
                {
                    found.push((change, record, ages));
                    break;
                }
            }
            if found.len() == 6 {
                break 'search;
            }
        }
    }
    assert_eq!(found.len(), 6, "two systems of each change: {found:?}");
    let mut reclassed = 0;
    for (change, grown, (onset, end)) in &found {
        for (when, age) in [
            ("before", onset - 5_000.0),
            ("during", f64::midpoint(*onset, *end)),
            ("after", end + 20_000.0),
        ] {
            let record = re_aged(grown, age);
            let model = BriefModel::new(&galaxy, &record);
            let stars = SystemStars::generate(&galaxy, &record);
            let count = draw_star_count(
                &galaxy,
                &record,
                grid_multiplicity(&record),
                RedrawAttempt::FIRST,
            );
            if when != "before" {
                assert_eq!(
                    model.route(),
                    BriefRoute::Paired,
                    "{:?} {change:?} {when}",
                    record.id()
                );
            }
            for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
                let row = model.brief_at(t);
                let summary = stars.summary_at(t);
                let what = format!(
                    "{:?} {change:?} {when}, at age {age:.0} yr, at {t}",
                    record.id()
                );
                assert_row_is_the_summary(row, summary.stars().first(), count, &what);
                let single =
                    SystemStars::generate_in(&galaxy, &record, MultiplicityContext::ForcedSingle)
                        .brief_at(t);
                reclassed += usize::from(
                    row.map(|b| (b.kind(), b.class())) != single.map(|b| (b.kind(), b.class())),
                );
            }
        }
    }
    assert!(
        reclassed > 0,
        "no row reads a class its single-star track does not have"
    );
}

/// P11.T11.a: a row the gate certifies is its single-star primary's brief, bit for bit, as every
/// row was before it. Over the random systems of [`check_briefs`], each row on the exact or
/// main-sequence route at both ends of the window and the epoch is the brief of the system of
/// its primary alone (plan 06's single-star system), with the first attempt's star count; a
/// table row has that brief's kind and class.
#[test]
fn certified_rows_are_their_single_star_briefs() {
    let galaxy = milky_way();
    let mut certified = 0;
    for record in sample_records(&galaxy, 0x6365_7274, 300) {
        let model = BriefModel::new(&galaxy, &record);
        if model.route() == BriefRoute::Paired {
            continue;
        }
        certified += 1;
        let single = SystemStars::generate_in(&galaxy, &record, MultiplicityContext::ForcedSingle);
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            let (row, own) = (model.brief_at(t), single.brief_at(t));
            if model.route() == BriefRoute::Table {
                assert_eq!(
                    row.map(|b| (b.kind(), b.class().to_string())),
                    own.map(|b| (b.kind(), b.class().to_string())),
                    "{record:?} at {t}"
                );
                continue;
            }
            assert_eq!(
                row.as_ref().map(state_of),
                own.as_ref().map(state_of),
                "{record:?} at {t}"
            );
            assert_eq!(
                row.map(|b| b.star_count()),
                own.map(|_| model.star_count()),
                "{record:?} at {t}"
            );
        }
    }
    assert!(certified > 150, "{certified} certified rows of 300");
}
