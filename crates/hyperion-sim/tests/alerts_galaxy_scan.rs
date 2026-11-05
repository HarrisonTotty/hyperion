//! A background scan that reaches the whole galaxy (plan 12, P12.T8.b.2): each candidate's event
//! time read first, the cells' entries whose time can matter, and the cut walks over the occupied
//! cells of each x-slab, which give the walks as built, `Detection` for `Detection`.
//!
//! The fixture is the Milky Way-like galaxy of seed `0x0926_0000` and the Sun-like point
//! [0, 26,000, 20] ly, as research's measurements were (research-alerts-galaxy-scan, 2026-11-01).

use std::sync::OnceLock;

use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, ClassSet, Detection, DetectionPhase,
    OneShotClass, RecurrentProcess, SensorHorizon, TransientKind, class_reach, kind_reach,
    magnetar_detections, magnetar_scan_cell, one_shot_detections, one_shot_scan_cell,
    recurrent_detections,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, ClassId, ClassProcess, cells_in_slab, cells_in_sphere,
    class_slabs, entries_when, occupied_cells,
};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::query::QuerySphere;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{LightYears, WattsPerSquareMetre};

/// The Milky Way-like fixture.
fn milky_way() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like()).unwrap()
    })
}

/// The Sun-like point.
fn sun() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap()
}

fn years(years: i64) -> UniverseTime {
    UniverseTime::from_julian_years(years).unwrap()
}

/// A sensor of one band's limits, W m⁻², resolving at a hundred times the detection limit.
fn sensor(band: AlertBand, detect: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(detect * 100.0)).unwrap();
    SensorHorizon::new([(band, limits)]).unwrap()
}

/// The query of `class` alone from `at`, over `[from, to)` years.
fn query(
    at: GalacticPosition,
    class: ClassId,
    sensor: SensorHorizon,
    from: i64,
    to: i64,
) -> AlertQuery {
    let window = TimeWindow::new(years(from), years(to)).unwrap();
    AlertQuery::builder(at, window, sensor)
        .classes(ClassSet::of(class))
        .build()
        .unwrap()
}

/// The sphere of `kind`'s reach for `query`, about its observer at its window's start.
fn kind_sphere(query: &AlertQuery, kind: TransientKind) -> QuerySphere {
    QuerySphere::new(
        *query.observer_position(),
        kind_reach(kind, query.sensor()),
        query.window().start(),
        LightYears::ZERO,
    )
    .unwrap()
}

/// `detections` in the walks' order: by arrival, then by event.
fn sorted(mut detections: Vec<Detection>) -> Vec<Detection> {
    detections.sort_by(|a, b| {
        a.arrival()
            .cmp(&b.arrival())
            .then(a.event().cmp(&b.event()))
    });
    detections
}

/// Where two lists of detections first differ: the index and the two there, or the lengths when
/// one is the other's start; `None` when they are equal. Lists of tens of thousands are compared
/// without printing them whole.
fn first_difference(
    cut: &[Detection],
    built: &[Detection],
) -> Option<(usize, Option<Detection>, Option<Detection>)> {
    (0..cut.len().max(built.len()))
        .find(|&at| cut.get(at) != built.get(at))
        .map(|at| (at, cut.get(at).cloned(), built.get(at).cloned()))
}

/// The cut walk of `class` over `sphere`: each x-slab's occupied cells, each walked by `walk`.
fn cut_walk(
    galaxy: &Galaxy,
    process: &impl ClassProcess,
    sphere: &QuerySphere,
    mut walk: impl FnMut(&mut AlertContext<'_>, CatalogueCellKey, &mut Vec<Detection>),
) -> Vec<Detection> {
    let class = process.class();
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut out = Vec::new();
    for x in class_slabs(class, sphere) {
        for (cell, _) in occupied_cells(galaxy, process, cells_in_slab(class, sphere, x)) {
            walk(&mut ctx, cell, &mut out);
        }
    }
    sorted(out)
}

/// The one-shot walk as built, every cell whole, and the cut walk of the same query.
fn one_shot_both<P: OneShotClass>(
    galaxy: &Galaxy,
    process: &P,
    query: &AlertQuery,
) -> (Vec<Detection>, Vec<Detection>) {
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut built = Vec::new();
    one_shot_detections(galaxy, &mut ctx, query, process, &mut built);
    let sphere = kind_sphere(query, process.kind());
    let cut = cut_walk(galaxy, process, &sphere, |ctx, cell, out| {
        one_shot_scan_cell(galaxy, ctx, query, process, cell, out);
    });
    (built, cut)
}

/// The magnetar walk as built, every cell whole, and the cut walk of the same query.
fn magnetars_both(galaxy: &Galaxy, query: &AlertQuery) -> (Vec<Detection>, Vec<Detection>) {
    let process = CoreCollapseProcess::new(galaxy);
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut built = Vec::new();
    magnetar_detections(galaxy, &mut ctx, query, &process, &mut built);
    let sphere = kind_sphere(query, TransientKind::MagnetarBurst);
    let cut = cut_walk(galaxy, &process, &sphere, |ctx, cell, out| {
        magnetar_scan_cell(galaxy, ctx, query, &process, cell, out);
    });
    (built, cut)
}

/// The first `count` occupied cells of `process`'s class about the Sun-like point, in key order.
fn occupied_about_the_sun(process: &impl ClassProcess, count: usize) -> Vec<CatalogueCellKey> {
    let sphere = QuerySphere::new(
        sun(),
        LightYears::new(20_000.0),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .unwrap();
    let cells = cells_in_sphere(process.class(), &sphere);
    let occupied = occupied_cells(milky_way(), process, cells);
    assert!(occupied.len() >= count, "{} occupied cells", occupied.len());
    occupied
        .into_iter()
        .take(count)
        .map(|(cell, _)| cell)
        .collect()
}

/// On `cells` of `process`'s class: every entry's event time is within a day of the time its
/// candidate reads first; `entries_when` with every time wanted is the whole cell's entries; and
/// with a window wanted, it keeps exactly the entries whose time read first is in the window,
/// among them every one whose own time is a day or more inside it. Returns the entries checked.
fn check_read_first<P>(process: &P, cells: &[CatalogueCellKey]) -> usize
where
    P: OneShotClass,
    P::Entry: Clone + PartialEq + std::fmt::Debug,
{
    let galaxy = milky_way();
    let day = Span::from_seconds(86_400);
    let window = (years(-400_000), years(0));
    let inside = |t: UniverseTime| window.0 <= t && t < window.1;
    let well_inside = |t: UniverseTime| {
        window.0.checked_add(day).unwrap() <= t && t < window.1.checked_sub(day).unwrap()
    };
    let mut entries = 0;
    let mut largest = Span::ZERO;
    for &key in cells {
        let whole = CatalogueClassCell::generate(galaxy, process, key);
        assert_eq!(
            entries_when(galaxy, process, key, |_| true),
            whole.entries(),
            "{key:?}"
        );
        let kept = entries_when(galaxy, process, key, inside);
        let mut expected = Vec::new();
        for entry in whole.entries() {
            let read = process
                .event_time(galaxy, entry.id())
                .expect("a supernova reads T first");
            let event = process
                .event(galaxy, entry.value())
                .expect("a supernova makes an event");
            let apart = event
                .emitted()
                .checked_since(read)
                .unwrap()
                .checked_abs()
                .unwrap();
            largest = largest.max(apart);
            assert!(
                apart <= day,
                "{key:?}: {apart:?} between the time read first and the entry's"
            );
            if inside(read) {
                expected.push(entry.clone());
            } else {
                assert!(
                    !well_inside(event.emitted()),
                    "{key:?}: an entry inside the window is cut"
                );
            }
        }
        assert_eq!(kept, expected, "{key:?}");
        entries += whole.entries().len();
    }
    eprintln!(
        "{} entries of {} cells, the read-first time at most {largest:?} off",
        entries,
        cells.len()
    );
    entries
}

/// The read-first time on 200 occupied cells of each supernova class about the Sun-like point:
/// within a day of every entry's, and the cut cell the whole cell's entries in its window.
#[test]
fn alerts_galaxy_scan_reads_each_candidates_time_first() {
    let galaxy = milky_way();
    let core_collapses = CoreCollapseProcess::new(galaxy);
    let cells = occupied_about_the_sun(&core_collapses, 200);
    assert!(check_read_first(&core_collapses, &cells) > 0);
    let type_ia = TypeIaProcess::new(galaxy);
    let cells = occupied_about_the_sun(&type_ia, 200);
    assert!(check_read_first(&type_ia, &cells) > 0);
}

/// The read-first time on 2,000 occupied cells of each supernova class about the Sun-like point,
/// as the fast test checks it on 200.
#[test]
#[ignore = "slow: generates 2,000 occupied core-collapse cells twice, some 10 s"]
fn alerts_galaxy_scan_reads_each_candidates_time_first_over_two_thousand_cells() {
    let galaxy = milky_way();
    let core_collapses = CoreCollapseProcess::new(galaxy);
    let cells = occupied_about_the_sun(&core_collapses, 2_000);
    assert!(check_read_first(&core_collapses, &cells) > 0);
    let type_ia = TypeIaProcess::new(galaxy);
    let cells = occupied_about_the_sun(&type_ia, 2_000);
    assert!(check_read_first(&type_ia, &cells) > 0);
}

/// The sensor of one band through which `kind` reaches `ly` light-years, resolving at a hundred
/// times its detection limit: a reach goes as the limit's inverse square root.
fn reaching(band: AlertBand, kind: TransientKind, ly: f64) -> SensorHorizon {
    let probe = 1e-6;
    let ratio = kind_reach(kind, &sensor(band, probe)).value() / ly;
    sensor(band, probe * ratio * ratio)
}

/// The fast twin of the whole cube's supernovae, in the galaxy of seed 9 (`Galaxy::new`): 100 ly
/// from a core collapse that goes off in the clock window's future, 179 years after the epoch in
/// cell [-2, 30, 0] (the server's pinned supernova, re-pinned at 27: the 288 cells about the solar
/// circle that held one at 26 hold none once the young field gives most of its young stars to the
/// features, P09.T2.c), through a V sensor that reaches 2,000 ly over the first 1,000 years, the
/// cut walk over each slab's occupied cells is the walk as built, and sees its onset.
#[test]
fn alerts_galaxy_scan_finds_a_supernova_nearby_as_the_walk_as_built() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = CoreCollapseProcess::new(&galaxy);
    let at = GalacticPosition::from_light_years([-584.1, 15_392.2, 108.4]).unwrap();
    let v = reaching(
        AlertBand::Photometric(Band::V),
        TransientKind::CoreCollapse,
        2_000.0,
    );
    let query = query(at, ClassId::CORE_COLLAPSE, v, 0, 1_000);
    let (built, cut) = one_shot_both(&galaxy, &process, &query);
    assert_eq!(cut, built);
    let onset = cut
        .iter()
        .find(|detection| detection.phase() == DetectionPhase::Onset)
        .expect("the pinned supernova goes off");
    let arrival = onset.arrival().since_epoch().as_julian_years_f64();
    assert!((arrival - 279.4).abs() < 0.1, "{arrival}");
}

/// The fast twin of the whole cube's magnetars: 1 ly from the nearest of the cube's 17 magnetars
/// that bursts over the first century near itself, at [7,161.9, 16,283.3, 217.7] ly, 12,072 ly
/// from the Sun-like point, through an X-ray sensor that reaches 500 ly over the first century,
/// the cut walk over each slab's occupied cells is the walk as built, and sees it burst eleven
/// times. (Re-pinned at 27: the field's young stars, most given to the features from 27,
/// P09.T2.c, hold no magnetar active at the epoch within 3,000 ly, as at 26 at
/// [-7,210.6, 28,386.8, 27.0] ly.)
#[test]
fn alerts_galaxy_scan_finds_a_magnetar_nearby_as_the_walk_as_built() {
    let galaxy = milky_way();
    let at = GalacticPosition::from_light_years([7_162.9, 16_283.3, 217.7]).unwrap();
    let x = reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 500.0);
    let query = query(at, ClassId::CORE_COLLAPSE, x, 0, 100);
    let (built, cut) = magnetars_both(galaxy, &query);
    assert!(!cut.is_empty(), "the magnetar bursts");
    assert_eq!(first_difference(&cut, &built), None);
}

/// The whole cube's core collapses through a V limit of 10⁻¹² W m⁻² over the first 1,000 years from
/// the Sun-like point: the cut walk over each slab's occupied cells is the walk as built, one
/// onset, arriving 360.6 years after the epoch (re-measured at 27, P12.T13: the features hold most
/// young stars from 27, P09.T2.c, and the field's walk does not read their deaths, P09.T36; four at
/// 26, arriving 364.2, 527.1, 531.8 and 629.3 years after it, research's figures).
#[test]
#[ignore = "slow: walks every core-collapse cell of the cube whole, some 100 CPU-s"]
fn alerts_galaxy_scan_finds_the_whole_cubes_supernovae_as_the_walk_as_built() {
    let galaxy = milky_way();
    let process = CoreCollapseProcess::new(galaxy);
    let v = AlertBand::Photometric(Band::V);
    let query = query(sun(), ClassId::CORE_COLLAPSE, sensor(v, 1e-12), 0, 1_000);
    let sphere = kind_sphere(&query, TransientKind::CoreCollapse);
    assert_eq!(
        class_slabs(ClassId::CORE_COLLAPSE, &sphere).len(),
        256,
        "the whole cube"
    );
    let (built, cut) = one_shot_both(galaxy, &process, &query);
    for detection in &cut {
        eprintln!(
            "{:?} at {:.1} yr from {:?}",
            detection.phase(),
            detection.arrival().since_epoch().as_julian_years_f64(),
            detection
                .retardation()
                .apparent_position()
                .to_light_years_f64()
        );
    }
    assert_eq!(cut, built);
    let onsets: Vec<f64> = cut
        .iter()
        .filter(|detection| detection.phase() == DetectionPhase::Onset)
        .map(|detection| detection.arrival().since_epoch().as_julian_years_f64())
        .collect();
    assert_eq!(onsets.len(), 1, "{onsets:?}");
    for (onset, expected) in onsets.iter().zip([360.6]) {
        assert!((onset - expected).abs() < 0.05, "{onsets:?}");
    }
}

/// The whole cube's magnetars through an X-ray limit of 10⁻¹⁴ W m⁻² over the first century from
/// the Sun-like point: the cut walk over each slab's occupied cells is the walk as built, 6,628
/// detections from 17 hosts (re-measured at 27, whose field share gives most young stars to the
/// features, P09.T2.c; 27,059 from 71 at 26, research's figures).
#[test]
#[ignore = "slow: walks every core-collapse cell of the cube's magnetars whole, some 100 CPU-s"]
fn alerts_galaxy_scan_finds_the_whole_cubes_magnetars_as_the_walk_as_built() {
    let galaxy = milky_way();
    let query = query(
        sun(),
        ClassId::CORE_COLLAPSE,
        sensor(AlertBand::XRay, 1e-14),
        0,
        100,
    );
    let (built, cut) = magnetars_both(galaxy, &query);
    assert_eq!(first_difference(&cut, &built), None);
    let mut hosts: Vec<_> = cut.iter().map(Detection::host).collect();
    hosts.sort_unstable();
    hosts.dedup();
    assert_eq!((cut.len(), hosts.len()), (6_628, 17));
}

/// The whole cube's Type Ia through a V limit of 10⁻¹² W m⁻² over [−1,000, 1,000) years from the
/// Sun-like point: the cut walk over each slab's occupied cells is the walk as built, two onsets,
/// arriving 198.9 and 747.6 years after the epoch from 27,977 and 17,653 ly. No Type Ia nearer
/// lights the clock window, and every sensor that sees one of these reaches across the cube, so
/// there is no fast twin: a Type Ia cell is cheap, and the sweep of bounds is the cost.
#[test]
#[ignore = "slow: sweeps every Type Ia cell of the cube twice, some 30 s"]
fn alerts_galaxy_scan_finds_the_whole_cubes_type_ia_as_the_walk_as_built() {
    let galaxy = milky_way();
    let process = TypeIaProcess::new(galaxy);
    let v = AlertBand::Photometric(Band::V);
    let query = query(sun(), ClassId::TYPE_IA, sensor(v, 1e-12), -1_000, 1_000);
    let (built, cut) = one_shot_both(galaxy, &process, &query);
    assert_eq!(first_difference(&cut, &built), None);
    let onsets: Vec<f64> = cut
        .iter()
        .filter(|detection| detection.phase() == DetectionPhase::Onset)
        .map(|detection| detection.arrival().since_epoch().as_julian_years_f64())
        .collect();
    assert_eq!(onsets.len(), 2, "{onsets:?}");
    for (onset, expected) in onsets.iter().zip([198.9, 747.6]) {
        assert!((onset - expected).abs() < 0.05, "{onsets:?}");
    }
}

/// A recurrent class's walk as built for `query`, its plan's cells each scanned with a process
/// built for it, and the walk over each slab's occupied cells of the class sphere with one process
/// built once (`RecurrentProcess`), in seed 9's galaxy.
fn recurrent_both(class: ClassId, query: &AlertQuery) -> (Vec<Detection>, Vec<Detection>) {
    let galaxy = Galaxy::new(Seed::new(9));
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut built = Vec::new();
    recurrent_detections(&galaxy, &mut ctx, class, query, &mut built);
    let process = RecurrentProcess::of(&galaxy, class).expect("a recurrent class");
    let sphere = QuerySphere::new(
        sun(),
        class_reach(class, query.sensor()),
        query.window().start(),
        LightYears::ZERO,
    )
    .unwrap();
    let mut cut = Vec::new();
    for x in class_slabs(class, &sphere) {
        for (cell, _) in process.occupied_cells(&galaxy, cells_in_slab(class, &sphere, x)) {
            process.scan_cell(&galaxy, &mut ctx, cell, query, &mut cut);
        }
    }
    (built, sorted(cut))
}

/// The X-ray binaries' walk over each slab's occupied cells, one process lent to every cell, is
/// their walk as built, through an X-ray sensor reaching 30,000 ly over the first year from the
/// Sun-like point of seed 9's galaxy: what the server's background walk of the class reads.
#[test]
fn alerts_galaxy_scan_walks_the_x_ray_binaries_occupied_cells_as_built() {
    let xray = reaching(AlertBand::XRay, TransientKind::XrayTransient, 30_000.0);
    let class = ClassId::XRAY_BINARY;
    let (built, cut) = recurrent_both(class, &query(sun(), class, xray, 0, 1));
    assert!(!built.is_empty(), "the X-ray binaries outburst and burst");
    assert_eq!(first_difference(&cut, &built), None);
}

/// As the X-ray binaries' fast test, for both accreting white dwarf classes' novae and the
/// luminous blue variables' giant eruptions, over the whole cube: through a V limit of twelfth
/// magnitude over the first decade (the sim's own example of the novae), 63 fast novae, 7 slow
/// and no eruption (re-measured at 27, P12.T13; 91, 11 and one at 26, measured 2026-11-01).
#[test]
#[ignore = "slow: scans every cell of the three classes across the cube twice, some 6 minutes"]
fn alerts_galaxy_scan_walks_the_novae_and_eruptions_occupied_cells_as_built() {
    // V = 12: Vega's 3.631 × 10⁻⁸ W m⁻² µm⁻¹ through the V band's 0.084 µm, 10^4.8 fainter.
    let twelfth = sensor(
        AlertBand::Photometric(Band::V),
        3.631e-8 * 0.084 * 1.584_893e-5,
    );
    let seen = [
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
        ClassId::LUMINOUS_BLUE_VARIABLE,
    ]
    .map(|class| {
        let (built, cut) = recurrent_both(class, &query(sun(), class, twelfth.clone(), 0, 10));
        assert_eq!(first_difference(&cut, &built), None, "{class}");
        built.len()
    });
    assert_eq!(seen, [63, 7, 0]);
}
