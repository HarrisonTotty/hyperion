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
    OneShotClass, SensorHorizon, TransientKind, kind_reach, magnetar_detections,
    magnetar_scan_cell, one_shot_detections, one_shot_scan_cell,
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
/// from its one core collapse of the 288 cells about the solar circle that goes off in the clock
/// window's future, a Type IIP 874 years after the epoch in cell [-3, 48, 0] (the server's pinned
/// supernova), through a V sensor that reaches 2,000 ly over the first 1,000 years, the cut walk
/// over each slab's occupied cells is the walk as built, and sees its onset.
#[test]
fn alerts_galaxy_scan_finds_a_supernova_nearby_as_the_walk_as_built() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = CoreCollapseProcess::new(&galaxy);
    let at = GalacticPosition::from_light_years([-1_320.4, 24_727.6, 145.3]).unwrap();
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
    assert!((arrival - 974.5).abs() < 0.1, "{arrival}");
}

/// The fast twin of the whole cube's magnetars: 1 ly from the one magnetar active at the epoch
/// within 3,000 ly of the Sun-like point, at [-7,210.6, 28,386.8, 27.0] ly, through an X-ray sensor
/// that reaches 500 ly over the first century, the cut walk over each slab's occupied cells is the
/// walk as built, and sees it burst.
#[test]
fn alerts_galaxy_scan_finds_a_magnetar_nearby_as_the_walk_as_built() {
    let galaxy = milky_way();
    let at = GalacticPosition::from_light_years([-7_209.6, 28_386.8, 27.0]).unwrap();
    let x = reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 500.0);
    let query = query(at, ClassId::CORE_COLLAPSE, x, 0, 100);
    let (built, cut) = magnetars_both(galaxy, &query);
    assert!(!cut.is_empty(), "the magnetar bursts");
    assert_eq!(cut.len(), built.len());
    assert!(cut == built, "the cut walk is the walk as built");
}

/// The whole cube's core collapses through a V limit of 10⁻¹² W m⁻² over the first 1,000 years from
/// the Sun-like point: the cut walk over each slab's occupied cells is the walk as built, four
/// onsets, arriving 364.2, 527.1, 531.8 and 629.3 years after the epoch (research's figures).
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
    assert_eq!(onsets.len(), 4, "{onsets:?}");
    for (onset, expected) in onsets.iter().zip([364.2, 527.1, 531.8, 629.3]) {
        assert!((onset - expected).abs() < 0.05, "{onsets:?}");
    }
}

/// The whole cube's magnetars through an X-ray limit of 10⁻¹⁴ W m⁻² over the first century from
/// the Sun-like point: the cut walk over each slab's occupied cells is the walk as built, 27,059
/// detections from 71 hosts (research's figures).
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
    assert_eq!(cut.len(), built.len());
    assert!(cut == built, "the cut walk is the walk as built");
    let mut hosts: Vec<_> = cut.iter().map(Detection::host).collect();
    hosts.sort_unstable();
    hosts.dedup();
    assert_eq!((cut.len(), hosts.len()), (27_059, 71));
}
