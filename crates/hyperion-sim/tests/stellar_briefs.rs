//! A range query's brief of a system (plan 06, P06.T38.e): pinned briefs across the five layers
//! and both routes, and over random systems the brief of each route is the full system's, bit for
//! bit (rulings 89 and 90).

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::stellar::ObjectKind;
use hyperion_sim::stellar::brief::{BriefModel, BriefRoute};
use hyperion_sim::stellar::system::{StellarBrief, SystemStars};
use hyperion_sim::time::UniverseTime;
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

fn kind_at_epoch(galaxy: &Galaxy, record: &SystemRecord) -> Option<ObjectKind> {
    SystemStars::generate(galaxy, record)
        .brief_at(UniverseTime::EPOCH)
        .map(|b| b.kind())
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
/// system's first.
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
            assert_eq!(brief, stars.brief_at(years(y)), "{label} at {y} yr");
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

/// Every brief of `n` random systems at three times is the full system's, bit for bit, and the
/// main-sequence route is taken for some and the exact route for others.
fn check_briefs(seed: u64, n: usize) {
    let galaxy = milky_way();
    let mut routes = [0_usize; 2];
    let mut rng = Lcg::new(seed ^ 0x7469_6d65);
    for record in sample_records(&galaxy, seed, n) {
        let model = BriefModel::new(&galaxy, &record);
        let stars = SystemStars::generate(&galaxy, &record);
        routes[usize::from(model.route() == BriefRoute::Exact)] += 1;
        let y = i64::try_from(rng.next_u64() % 2_001).expect("small") - 1_000;
        for t in [years(y), UniverseTime::EPOCH, years(-200_000)] {
            let (a, b) = (model.brief_at(t), stars.brief_at(t));
            assert_eq!(a, b, "{record:?} at {t}");
            if let (Some(a), Some(b)) = (a, b) {
                assert_eq!(
                    (
                        bits(a.effective_temperature().value()),
                        a.log_luminosity().map(|l| bits(l.value()))
                    ),
                    (
                        bits(b.effective_temperature().value()),
                        b.log_luminosity().map(|l| bits(l.value()))
                    ),
                    "{record:?} at {t}"
                );
            }
        }
    }
    assert!(routes.iter().all(|&r| r > 0), "{routes:?}");
}

#[test]
fn briefs_are_the_full_systems_over_random_systems() {
    check_briefs(0x6272_6566, 300);
}

#[test]
#[ignore = "slow: 3 × 10⁴ random systems, each generated in full"]
fn briefs_are_the_full_systems_over_thirty_thousand_systems() {
    check_briefs(0x6272_6567, 30_000);
}
