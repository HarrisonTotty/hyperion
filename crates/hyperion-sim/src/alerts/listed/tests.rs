//! The tests of the lists' walks (P09.T36 in plan 12's walks), under the hold's new arm.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::Seed;
use crate::alerts::local::LocalEventsCache;
use crate::alerts::{
    AlertBand, AlertResult, AlertSource, BandLimits, ClassSet, ClassStatus, DetectionPhase,
    SensorHorizon, SourceCensus, SourceStatus, Unwalked, alerts_in, magnetar_scan_cell,
    one_shot_scan_cell,
};
use crate::events::TimeWindow;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use crate::galaxy::catalogue_classes::supernova::explosion_event_of;
use crate::galaxy::catalogue_classes::{cells_in_slab, entries_when, occupied_cells};
use crate::galaxy::features::catalogue::{FeatureCellContents, NoFeatureCache};
use crate::galaxy::features::members::recent::FeatureClassesRules;
use crate::galaxy::gas::ccm::Band;
use crate::galaxy::gas::modifiers::NoModifiers;
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::{FeatureCell, SystemIdKind};
use crate::observe::NoStarsCache;
use crate::planetary::testing::in_shares;
use crate::units::WattsPerSquareMetre;

/// The lists' walks are held until the 28 → 29 bump: the rules in force are version 27's and
/// before, and 29's turn them on.
#[test]
fn the_listed_alerts_are_held_only_until_the_29_bump() {
    use crate::GENERATOR_VERSION;
    assert!(
        GENERATOR_VERSION < LISTED_ALERTS_VERSION,
        "the 28 → 29 bump puts the lists' walks on the record (fix-p09-alert-walk-features): \
         delete `LISTED_ALERTS_VERSION`, `ListedAlertsRules` with its `FieldAlone` arm, \
         `GalaxyRules::listed_alerts`, the `match` in `ListedWalk::rules_read`, the held arm of \
         `ListedWalk::unwalked_by` and this test, and re-pin the alert tests' counts at 29"
    );
    assert_eq!(LISTED_ALERTS_VERSION, GeneratorVersion::new(29));
    assert_eq!(
        ListedAlertsRules::of_version(GENERATOR_VERSION),
        ListedAlertsRules::FieldAlone
    );
    assert_eq!(
        ListedAlertsRules::of_version(GeneratorVersion::new(28)),
        ListedAlertsRules::FieldAlone
    );
    assert_eq!(
        ListedAlertsRules::of_version(LISTED_ALERTS_VERSION),
        ListedAlertsRules::Lists
    );
}

/// The shares a whole-cube walk runs in.
const SHARES: usize = 16;

/// The rules of version `version`.
fn at(version: u32) -> GalaxyRules {
    GalaxyRules::of_version(GeneratorVersion::new(version))
}

/// The version's rules from 28 with this hold's new arm alone: the lists walked.
fn listed() -> GalaxyRules {
    GalaxyRules {
        listed_alerts: ListedAlertsRules::Lists,
        ..at(28)
    }
}

/// The 26-like reference: the field's budget, which held every young star at 26, and no lists.
fn budget() -> GalaxyRules {
    GalaxyRules {
        field_share: FieldShareRules::None,
        feature_classes: FeatureClassesRules::Unlisted,
        ..at(27)
    }
}

/// The galaxy of `seed` and `params` under `rules`, with its kinematic tables, which the lists
/// need, as the server's are.
fn galaxy(seed: u64, params: GalaxyParams, rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(Seed::new(seed), params, rules)
        .expect("the fixture builds")
        .with_full_potential()
}

/// A feature cell cache that keeps every cell it is asked for, for one galaxy and many threads.
#[derive(Debug, Default)]
struct KeepCells(Mutex<BTreeMap<[i32; 3], Arc<FeatureCellContents>>>);

impl FeatureCellCache for KeepCells {
    fn contents(&self, galaxy: &Galaxy, cell: FeatureCell) -> Arc<FeatureCellContents> {
        let key = cell.to_array();
        if let Some(found) = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Arc::clone(found);
        }
        let made = Arc::new(FeatureCatalogue::cell(galaxy, cell));
        let mut kept = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(kept.entry(key).or_insert(made))
    }
}

/// The Sun-like point.
fn sun() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap()
}

/// The query of the core collapses at `at` through one band's detection limit, W m⁻², resolving
/// at a hundred times it, over `[from, to)` years.
fn query(at: GalacticPosition, band: AlertBand, detect: f64, (from, to): (i64, i64)) -> AlertQuery {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(detect * 100.0)).unwrap();
    let sensor = SensorHorizon::new([(band, limits)]).unwrap();
    let window = TimeWindow::new(
        UniverseTime::from_julian_years(from).unwrap(),
        UniverseTime::from_julian_years(to).unwrap(),
    )
    .unwrap();
    AlertQuery::builder(at, window, sensor)
        .classes(ClassSet::of(ClassId::CORE_COLLAPSE))
        .build()
        .unwrap()
}

/// What a whole-cube walk of the core collapses found: the cells' detections and the lists', the
/// candidates the lists drew, and the seconds the shares spent on the cells and on the lists (a
/// measurement, logged).
#[derive(Debug, Default)]
struct Walked {
    cells: Vec<Detection>,
    lists: Vec<Detection>,
    drawn: u64,
    seconds: [f64; 2],
}

/// The whole cube's core collapses for `query` in `galaxy`, slab by slab of the class's sphere
/// as a background scan walks it, in [`SHARES`] shares: each slab's occupied cells cut to their
/// supernovae and magnetars, and its lists ([`listed_scan_slab`]).
fn whole_cube(galaxy: &Galaxy, query: &AlertQuery) -> Walked {
    let class = ClassId::CORE_COLLAPSE;
    let process = CoreCollapseProcess::new(galaxy);
    let reach = super::super::class_reach(class, query.sensor());
    let sphere = QuerySphere::new(
        *query.observer_position(),
        reach,
        query.window().start(),
        LightYears::ZERO,
    )
    .unwrap();
    let slabs: Vec<i32> = class_slabs(class, &sphere).collect();
    let supernovae = kind_reach(TransientKind::CoreCollapse, query.sensor()) > LightYears::ZERO;
    let magnetars = kind_reach(TransientKind::MagnetarBurst, query.sensor()) > LightYears::ZERO;
    let features = KeepCells::default();
    // Every feature cell made once, ahead of the slabs, eight of which read each column of cells.
    in_shares(SHARES, SHARES, |share| {
        for cell in every_feature_cell().skip(share).step_by(SHARES) {
            let _ = features.contents(galaxy, cell);
        }
    });
    let shares = in_shares(SHARES, SHARES, |share| {
        let mut noise = NoiseCache::with_capacity(4_096);
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_feature_cells(&features);
        let mut walked = Walked::default();
        for &x in slabs.iter().skip(share).step_by(SHARES) {
            let started = std::time::Instant::now();
            for (cell, _) in occupied_cells(galaxy, &process, cells_in_slab(class, &sphere, x)) {
                if supernovae {
                    one_shot_scan_cell(galaxy, &mut ctx, query, &process, cell, &mut walked.cells);
                }
                if magnetars {
                    magnetar_scan_cell(galaxy, &mut ctx, query, &process, cell, &mut walked.cells);
                }
            }
            let listing = std::time::Instant::now();
            walked.seconds[0] += (listing - started).as_secs_f64();
            walked.drawn += listed_scan_slab(
                galaxy,
                &mut ctx,
                query,
                (class, &sphere),
                x,
                &mut walked.lists,
            );
            walked.seconds[1] += listing.elapsed().as_secs_f64();
        }
        walked
    });
    let mut walked = Walked::default();
    for share in shares {
        walked.cells.extend(share.cells);
        walked.lists.extend(share.lists);
        walked.drawn += share.drawn;
        walked.seconds[0] += share.seconds[0];
        walked.seconds[1] += share.seconds[1];
    }
    for detections in [&mut walked.cells, &mut walked.lists] {
        sort_detections(detections);
    }
    walked
}

/// Every feature cell of the root cube, in x, y, z order.
fn every_feature_cell() -> impl Iterator<Item = FeatureCell> {
    (-16..16).flat_map(|i| {
        (-16..16).flat_map(move |j| {
            (-16..16).map(move |k| FeatureCell::new([i, j, k]).expect("in the cube"))
        })
    })
}

/// The hosts of `detections`, each once.
fn hosts(detections: &[Detection]) -> usize {
    detections
        .iter()
        .map(Detection::host)
        .collect::<BTreeSet<_>>()
        .len()
}

/// Whether `host` is a member of a catalogue feature's or the centre's list.
fn is_listed(host: SystemId) -> bool {
    matches!(
        host.kind(),
        SystemIdKind::FeatureMember(_) | SystemIdKind::Centre(_)
    )
}

/// The brief's census, measured and logged, not asserted beyond its shape: the whole cube's
/// magnetars through an X-ray limit of 10⁻¹⁴ W m⁻² over the first century from the Sun-like point
/// in the Milky Way fixture of seed `0x0926_0000` (research's, P12.T8.b.2), and its supernovae
/// through a V limit of 10⁻¹² over the first 1,000 years, at 27 (the cells alone), at 29 (the cells
/// and the lists) and in the 26-like budget (the field holding every young star, no lists). The
/// cells' detections are 27's bit for bit at 29.
#[test]
#[ignore = "slow: every feature's list and the core collapses' cells of the cube, thrice"]
fn the_whole_cube_s_core_collapses_with_the_lists_measured() {
    let params = GalaxyParams::milky_way_like;
    let magnetars = query(sun(), AlertBand::XRay, 1e-14, (0, 100));
    let supernovae = query(sun(), AlertBand::Photometric(Band::V), 1e-12, (0, 1_000));
    let onsets = |detections: &[Detection]| {
        detections
            .iter()
            .filter(|d| d.phase() == DetectionPhase::Onset)
            .count()
    };
    let mut cells_at_27 = None;
    for (label, rules) in [("27", at(27)), ("29", listed()), ("budget", budget())] {
        let galaxy = galaxy(0x0926_0000, params(), rules);
        let bursts = whole_cube(&galaxy, &magnetars);
        let explosions = whole_cube(&galaxy, &supernovae);
        assert!(bursts.cells.iter().all(|d| !is_listed(d.host())));
        assert!(bursts.lists.iter().all(|d| is_listed(d.host())));
        eprintln!(
            "{label}: magnetars, cells {} detections from {} hosts, lists {} from {} hosts \
             ({} candidates drawn); supernovae, cells {} onsets ({} detections), lists {} onsets \
             ({} detections, {} candidates drawn); thread-seconds, cells {:.1} and {:.1}, lists \
             {:.1} and {:.1}",
            bursts.cells.len(),
            hosts(&bursts.cells),
            bursts.lists.len(),
            hosts(&bursts.lists),
            bursts.drawn,
            onsets(&explosions.cells),
            explosions.cells.len(),
            onsets(&explosions.lists),
            explosions.lists.len(),
            explosions.drawn,
            bursts.seconds[0],
            explosions.seconds[0],
            bursts.seconds[1],
            explosions.seconds[1],
        );
        for detection in bursts.lists.iter().chain(&explosions.lists).filter(|d| {
            d.phase() == DetectionPhase::Onset || d.kind() == TransientKind::MagnetarBurst
        }) {
            eprintln!(
                "  {label}: {:?} {:?} of {:#018x} arriving {:.3} yr, emitted {:.3} yr at {:?} ly",
                detection.kind(),
                detection.phase(),
                detection.host().raw(),
                detection.arrival().since_epoch().as_julian_years_f64(),
                detection
                    .retardation()
                    .emitted()
                    .since_epoch()
                    .as_julian_years_f64(),
                detection
                    .retardation()
                    .apparent_position()
                    .to_light_years_f64(),
            );
        }
        match label {
            "27" => {
                assert_eq!((bursts.lists.len(), explosions.lists.len()), (0, 0));
                cells_at_27 = Some((bursts.cells, explosions.cells));
            }
            "29" => {
                let (b, e) = cells_at_27.take().expect("27 walks first");
                assert_eq!(bursts.cells, b, "the cells' magnetars at 29 are 27's");
                assert_eq!(explosions.cells, e, "the cells' supernovae at 29 are 27's");
                assert!(bursts.drawn > 0 && explosions.drawn > 0);
            }
            _ => assert_eq!((bursts.lists.len(), explosions.lists.len()), (0, 0)),
        }
    }
}

/// The core collapses of every feature's list in `galaxy` that die in [0, 1,000) years, those of
/// them whose sites `in_slab` keeps, and the lists' magnetars active at the epoch, each logged
/// under `label`: the fast tests' fixtures.
fn listed_millennium(
    galaxy: &Galaxy,
    label: &str,
    millennium: impl Fn(UniverseTime) -> bool + Sync,
    in_slab: impl Fn(&GalacticPosition) -> bool + Sync,
) -> [u32; 3] {
    let class = ClassId::CORE_COLLAPSE;
    let active = |t: UniverseTime| {
        let years = t.since_epoch().as_julian_years_f64();
        (-MAGNETAR_ACTIVE_SPAN.value()..=0.0).contains(&years)
    };
    in_shares(SHARES, SHARES, |share| {
        let mut counts = [0; 3];
        let wanted = |t| millennium(t) || active(t);
        for cell in every_feature_cell().skip(share).step_by(SHARES) {
            for feature in FeatureCatalogue::cell(galaxy, cell).features() {
                let list = FeatureLevelList::of(galaxy, feature);
                for entry in list.entries().filter(|e| e.class() == class) {
                    let Some(kept) = list.recent_entry_when(galaxy, entry.index(), &wanted) else {
                        continue;
                    };
                    let site = kept.site().expect("a death has a site");
                    let death = Death::of_feature(galaxy, &kept).expect("a death");
                    let id = (feature.id().cell().to_array(), feature.id().index());
                    if millennium(site.death()) {
                        counts[0] += 1;
                        counts[1] += u32::from(in_slab(site.position()));
                        eprintln!(
                            "  {label}: {:#018x} of feature {id:?} dies {:.3} yr at {:?} ly",
                            death.host.raw(),
                            site.death().since_epoch().as_julian_years_f64(),
                            site.position().to_light_years_f64(),
                        );
                    }
                    let Some(magnetar) = &death.magnetar else {
                        continue;
                    };
                    let born = magnetar.birth().since_epoch().as_julian_years_f64();
                    if born <= 0.0 && born + magnetar.active_span().value() >= 0.0 {
                        counts[2] += 1;
                        eprintln!(
                            "  {label}: magnetar {:#018x} of feature {id:?} born {born:.1} yr, \
                             active {:.1} yr, at {:?} ly at the epoch",
                            death.host.raw(),
                            magnetar.active_span().value(),
                            death
                                .motion
                                .position_at(UniverseTime::EPOCH)
                                .to_light_years_f64(),
                        );
                    }
                }
            }
        }
        counts
    })
    .into_iter()
    .fold([0, 0, 0], |[a, b, c], [d, e, f]| [a + d, b + e, c + f])
}

/// The deaths of the core collapses in the first 1,000 years in seed 9's galaxy (the server's
/// alert fixture), counted, not walked: the cells' entries, each cell cut to the deaths in
/// [0, 1,000) years, and the lists' entries, every feature's list and the centre's, at 29 and in
/// the 26-like budget; galaxy-wide and in the server fixture's slab about the solar circle (within
/// 30,000 ly along x, −5,000 to 56,000 ly along y and 768 ly of the plane). Logged, not asserted
/// beyond its shape.
#[test]
#[ignore = "slow: every feature's list and every occupied core-collapse cell of seed 9, twice"]
fn the_first_millennium_s_core_collapses_with_the_lists_measured() {
    let class = ClassId::CORE_COLLAPSE;
    let millennium = |t: UniverseTime| {
        let years = t.since_epoch().as_julian_years_f64();
        (0.0..1_000.0).contains(&years)
    };
    let in_slab = |p: &GalacticPosition| {
        let [x, y, z] = p.to_light_years_f64();
        x.abs() <= 30_000.0 && (-5_000.0..=56_000.0).contains(&y) && z.abs() <= 768.0
    };
    for (label, rules) in [("29", listed()), ("budget", budget())] {
        let galaxy = galaxy(
            9,
            GalaxyParams::from_seed(
                Seed::new(9),
                crate::galaxy::imf::MassFunctionKind::default(),
            ),
            rules,
        );
        let process = CoreCollapseProcess::new(&galaxy);
        let whole = QuerySphere::new(
            sun(),
            crate::alerts::ROOT_CUBE_DIAGONAL,
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .unwrap();
        let slabs: Vec<i32> = class_slabs(class, &whole).collect();
        let cells: Vec<[u32; 2]> = in_shares(SHARES, SHARES, |share| {
            let mut counts = [0; 2];
            for &x in slabs.iter().skip(share).step_by(SHARES) {
                for (cell, _) in occupied_cells(&galaxy, &process, cells_in_slab(class, &whole, x))
                {
                    for entry in entries_when(&galaxy, &process, cell, millennium) {
                        let site = entry.value().site();
                        if millennium(site.death()) {
                            counts[0] += 1;
                            counts[1] += u32::from(in_slab(site.position()));
                        }
                    }
                }
            }
            counts
        });
        let lists = listed_millennium(&galaxy, label, millennium, in_slab);
        let cells = cells.iter().fold([0, 0], |[a, b], [c, d]| [a + c, b + d]);

        let mut centre = ListedWalk::default();
        let all = query(sun(), AlertBand::Photometric(Band::V), 1e-30, (0, 1_000));
        let reaches = Reaches::of(class, &all);
        centre.read_centre(&galaxy, &all, &reaches, class);
        let centre_deaths = centre
            .deaths
            .iter()
            .filter(|d| millennium(d.explosion))
            .count();
        eprintln!(
            "{label}: core collapses in [0, 1,000) yr: cells {} ({} in the slab), features' lists \
             {} ({} in the slab), the centre's {centre_deaths}; the lists' magnetars active at \
             the epoch {}",
            cells[0], cells[1], lists[0], lists[1], lists[2]
        );
        if label == "budget" {
            assert_eq!(lists, [0, 0, 0]);
        }
    }
}

/// Seed 9's galaxy (`Galaxy::new`'s parameters, the server's alert fixture) at 27, or with the
/// lists walked ([`listed`]), with its kinematic tables, built once each.
fn seed_9(lists: bool) -> &'static Galaxy {
    static AT_27: OnceLock<Galaxy> = OnceLock::new();
    static LISTED: OnceLock<Galaxy> = OnceLock::new();
    let (cell, rules) = if lists {
        (&LISTED, listed())
    } else {
        (&AT_27, at(27))
    };
    cell.get_or_init(|| {
        let params = GalaxyParams::from_seed(
            Seed::new(9),
            crate::galaxy::imf::MassFunctionKind::default(),
        );
        galaxy(9, params, rules)
    })
}

/// The member of the nursery 49 of feature cell [−4, 6, 0] in seed 9's galaxy whose core collapse
/// goes off 234.797 years after the epoch at [−12,727.7, 26,883.1, 204.0] ly, some 12,750 ly
/// spinward of the Sun-like point (the first-millennium census's): the lists' supernova.
const LISTED_SUPERNOVA: u64 = 0xe65a_0018_f000_0003;

/// The member of the cluster 571 of feature cell [3, 2, −1] in seed 9's galaxy whose Type Ia, a
/// merger's, goes off 742.870 years after the epoch at [14,360.3, 9,057.3, 283.7] ly: the one
/// listed Type Ia of the galaxy's lists exploding in [−1,500, 1,000) years.
const LISTED_TYPE_IA: u64 = 0xe9c9_e11d_f000_0001;

/// The member of the nursery 775 of feature cell [0, 6, −1] in seed 9's galaxy whose magnetar, born
/// 119,354.4 years before the epoch, is active for 136,272.7 years, at [200.9, 25,409.4, −165.5] ly
/// at the epoch, some 650 ly from the Sun-like point (the census's): the lists' magnetar.
const LISTED_MAGNETAR: u64 = 0xe859_e183_f000_000c;

/// The sensor of one band through which `kind` reaches `ly` light-years, resolving at a hundred
/// times its detection limit: a reach goes as the limit's inverse square root.
fn reaching(band: AlertBand, kind: TransientKind, ly: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let sensor = |detect: f64| {
        let limits = BandLimits::new(w(detect), w(detect * 100.0)).unwrap();
        SensorHorizon::new([(band, limits)]).unwrap()
    };
    let probe = 1e-6;
    let ratio = kind_reach(kind, &sensor(probe)).value() / ly;
    sensor(probe * ratio * ratio)
}

/// The query of `class` alone through `sensor` at `at` over `[from, to)` Julian years.
fn sensor_query(
    class: ClassId,
    at: GalacticPosition,
    sensor: SensorHorizon,
    (from, to): (i64, i64),
) -> AlertQuery {
    let window = TimeWindow::new(
        UniverseTime::from_julian_years(from).unwrap(),
        UniverseTime::from_julian_years(to).unwrap(),
    )
    .unwrap();
    AlertQuery::builder(at, window, sensor)
        .classes(ClassSet::of(class))
        .build()
        .unwrap()
}

/// What `alerts_in` finds for `query` in `galaxy`, the feature cells generated for each lookup.
fn alerts(galaxy: &Galaxy, query: &AlertQuery) -> AlertResult {
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    alerts_in(
        galaxy,
        &mut ctx,
        query,
        &[],
        &mut NoStarsCache,
        &mut LocalEventsCache::default(),
    )
}

/// The class `query` asks about alone.
fn class_of(query: &AlertQuery) -> ClassId {
    let mut classes = query.classes().iter();
    let class = classes.next().expect("a class");
    assert!(classes.next().is_none(), "one class");
    class
}

/// The class's sphere of a background walk for `query` and its slabs.
fn class_sphere(query: &AlertQuery) -> (QuerySphere, Vec<i32>) {
    let class = class_of(query);
    let reach = super::super::class_reach(class, query.sensor());
    let sphere = QuerySphere::new(
        *query.observer_position(),
        reach,
        query.window().start(),
        LightYears::ZERO,
    )
    .unwrap();
    let slabs = class_slabs(class, &sphere).collect();
    (sphere, slabs)
}

/// The lists' detections of the slab `x` of the class's sphere for `query` in `galaxy`, the feature
/// cells kept in `features`, and the candidates they drew ([`listed_scan_slab`]).
fn slab(
    galaxy: &Galaxy,
    query: &AlertQuery,
    (sphere, x): (&QuerySphere, i32),
    features: &KeepCells,
) -> (Vec<Detection>, u64) {
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_feature_cells(features);
    let mut out = Vec::new();
    let drawn = listed_scan_slab(
        galaxy,
        &mut ctx,
        query,
        (class_of(query), sphere),
        x,
        &mut out,
    );
    (out, drawn)
}

/// The lists' detections of every slab of the class's sphere for `query` in `galaxy`, as a
/// background scan walks them, merged, and the candidates they drew; each slab's the same in any
/// order of the slabs, through one cache of feature cells.
fn slabs(galaxy: &Galaxy, query: &AlertQuery) -> (Vec<Detection>, u64) {
    let (sphere, slabs) = class_sphere(query);
    let features = KeepCells::default();
    assert_order_independent(&slabs, |&x| slab(galaxy, query, (&sphere, x), &features));
    let mut merged = Vec::new();
    let mut drawn = 0;
    for &x in &slabs {
        let (out, n) = slab(galaxy, query, (&sphere, x), &features);
        merged.extend(out);
        drawn += n;
    }
    sort_detections(&mut merged);
    (merged, drawn)
}

/// The lists' detections for `query` in `galaxy` without the walk's lookup, pre-tests or cut:
/// every death of the class on the list of every feature whose centre lies within the class's
/// kinds' reaches and the cap's [`LIFETIME_REACH`] of a site and of a remnant of its reach, and on
/// the centre's, each list's entries built whole. Each entry built under a time read first is the
/// whole entry's, bit for bit, where it is kept.
fn brute_force(galaxy: &Galaxy, query: &AlertQuery) -> Vec<Detection> {
    let class = class_of(query);
    let reaches = Reaches::of(class, query);
    let radius = reaches
        .supernovae
        .map_or(0.0, |(_, r)| r.value() + LIFETIME_REACH.value())
        .max(
            reaches
                .magnetars
                .map_or(0.0, |r| r.value() + 2.0 * LIFETIME_REACH.value()),
        );
    let before = |t: UniverseTime| t < UniverseTime::EPOCH;
    let observer = query.observer_position();
    let mut walk = ListedWalk::default();
    let near = FeatureCatalogue::near(galaxy, observer, LightYears::new(radius), &NoFeatureCache);
    for feature in near {
        let list = FeatureLevelList::of(galaxy, &feature);
        for entry in list.entries().filter(|entry| entry.class() == class) {
            let whole = list.recent_entry(galaxy, entry.index());
            assert_eq!(
                list.recent_entry_when(galaxy, entry.index(), &before),
                whole
                    .clone()
                    .filter(|e| before(e.site().expect("a death").death())),
                "the entry {} of {:?}",
                entry.index(),
                feature.id()
            );
            walk.deaths
                .extend(whole.and_then(|e| Death::of_feature(galaxy, &e)));
        }
    }
    let every = Reaches {
        supernovae: Some((
            TransientKind::CoreCollapse,
            crate::alerts::ROOT_CUBE_DIAGONAL,
        )),
        magnetars: Some(crate::alerts::ROOT_CUBE_DIAGONAL),
    };
    let all = sensor_query(
        class,
        GalacticPosition::ORIGIN,
        reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 1.0),
        (-1_000, 1_000),
    );
    let mut centre = ListedWalk::default();
    centre.read_centre(galaxy, &all, &every, class);
    walk.deaths.extend(centre.deaths);
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut out = Vec::new();
    walk.detections(galaxy, &mut ctx, query, class, &mut out);
    out
}

/// From 29 `query`'s class's walk sees `host`'s detections beside the cells' (module
/// documentation), each named on the list's member, and the cells' are 27's bit for bit; the
/// census adds the lists' expected candidates to the cells' and names what the lists hold that no
/// walk reads; below 29 no list is read. The walk's lists find what every list near enough finds
/// whole ([`brute_force`]). A background scan's slabs give the lists' detections in any order and
/// count the candidates the interactive walk's lists drew. Returns 29's detections of `host`,
/// pinned in the golden `name`.
fn listed_beside_the_cells(query: &AlertQuery, host: SystemId, name: &str) -> Vec<Detection> {
    let class = class_of(query);
    let (old, new) = (seed_9(false), seed_9(true));
    let (before, after) = (alerts(old, query), alerts(new, query));
    let (listed, cells): (Vec<Detection>, Vec<Detection>) = after
        .detections()
        .iter()
        .cloned()
        .partition(|d| is_listed(d.host()));
    assert_eq!(cells, before.detections(), "the cells' walk is 27's");
    assert!(before.detections().iter().all(|d| !is_listed(d.host())));
    let lists = ListedWalk::find(new, &NoFeatureCache, query, class).expect("read from 29");
    assert!(ListedWalk::find(old, &NoFeatureCache, query, class).is_none());
    let census = |result: &AlertResult| {
        let census = result.class_census(class).expect("asked");
        assert_eq!(census.status(), ClassStatus::Complete);
        census.expected_hosts().expect("counted")
    };
    assert_same_bits(census(&after), census(&before) + lists.expected());
    assert!(lists.expected() > 0.0);
    let unwalked = |result: &AlertResult| {
        result
            .sources()
            .iter()
            .find(|source| source.source() == AlertSource::FeatureLists)
            .map(SourceCensus::status)
    };
    let not_built = |task| Some(SourceStatus::Unwalked(Unwalked::NotBuilt(task)));
    assert_eq!(unwalked(&before), not_built("P09.T36"));
    assert_eq!(unwalked(&after), not_built(ListedWalk::unwalked_by(new)));
    let mut whole = brute_force(new, query);
    sort_detections(&mut whole);
    assert_eq!(whole, listed, "the lists' walk finds every list's whole");
    let (by_slab, drawn) = slabs(new, query);
    assert_eq!(
        by_slab, listed,
        "the slabs give the interactive walk's lists"
    );
    assert_eq!(drawn, lists.drawn());
    assert_eq!(slabs(old, query), (Vec::new(), 0));
    let mine: Vec<Detection> = listed
        .into_iter()
        .filter(|detection| detection.host() == host)
        .collect();
    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    for detection in &mine {
        let arrival = detection.arrival();
        w.line(&format!(
            "{:?} {:?} {:?} in {:?}",
            detection.kind(),
            detection.class(),
            detection.phase(),
            detection.band()
        ));
        w.line(&format!("event {}", detection.event()));
        w.u64_hex("host", detection.host().raw());
        w.line(&format!(
            "arrival {} s {} ns",
            arrival.seconds(),
            arrival.subsec_nanos()
        ));
        w.f64("flux", detection.flux().value());
    }
    hyperion_testkit::golden!(name, &w.finish());
    mine
}

/// The lists' supernova, 0.2 ly spinward of its site, through a V sensor that reaches 700 ly over
/// [200, 300) years: from 29 its onset is seen, on its list's member, beside the cells', 0.2 years
/// after its explosion, which its explosion event names.
#[test]
fn from_29_a_listed_supernova_is_seen_beside_the_cells() {
    let host = SystemId::from_raw(LISTED_SUPERNOVA).unwrap();
    let at = GalacticPosition::from_light_years([-12_727.473, 26_883.128, 204.046]).unwrap();
    let v = reaching(
        AlertBand::Photometric(Band::V),
        TransientKind::CoreCollapse,
        700.0,
    );
    let query = sensor_query(ClassId::CORE_COLLAPSE, at, v, (200, 300));
    let seen = listed_beside_the_cells(&query, host, "alerts/listed_supernova");
    let [onset] = seen.as_slice() else {
        panic!("one onset of the listed supernova: {seen:?}");
    };
    assert_eq!(onset.phase(), DetectionPhase::Onset);
    assert_eq!(onset.kind(), TransientKind::CoreCollapse);
    assert_eq!(onset.class(), Some(ClassId::CORE_COLLAPSE));
    assert_eq!(onset.event(), explosion_event_of(host));
    let arrival = onset.arrival().since_epoch().as_julian_years_f64();
    assert!((arrival - 234.997).abs() < 0.01, "{arrival}");
}

/// The lists' Type Ia, 0.2 ly spinward of its site, through a V sensor that reaches 700 ly over
/// [700, 800) years: from 29 its onset is seen, a merger's Type Ia on its list's member, beside the
/// cells', 0.2 years after its explosion, which its explosion event names.
#[test]
fn from_29_a_listed_type_ia_is_seen_beside_the_cells() {
    let host = SystemId::from_raw(LISTED_TYPE_IA).unwrap();
    let at = GalacticPosition::from_light_years([14_360.493, 9_057.254, 283.739]).unwrap();
    let v = reaching(
        AlertBand::Photometric(Band::V),
        TransientKind::TypeIa,
        700.0,
    );
    let query = sensor_query(ClassId::TYPE_IA, at, v, (700, 800));
    let seen = listed_beside_the_cells(&query, host, "alerts/listed_type_ia");
    let [onset] = seen.as_slice() else {
        panic!("one onset of the listed Type Ia: {seen:?}");
    };
    assert_eq!(onset.phase(), DetectionPhase::Onset);
    assert_eq!(onset.kind(), TransientKind::TypeIa);
    assert_eq!(onset.class(), Some(ClassId::TYPE_IA));
    assert_eq!(onset.event(), explosion_event_of(host));
    let arrival = onset.arrival().since_epoch().as_julian_years_f64();
    assert!((arrival - 743.070).abs() < 0.01, "{arrival}");
}

/// The lists' magnetar, 1 ly spinward of its remnant at the epoch, through an X-ray sensor that
/// reaches 500 ly over the first century: from 29 it bursts, eighteen times, on its list's member,
/// beside the cells', whose magnetars hold none active within 3,000 ly of the Sun-like point.
#[test]
fn from_29_a_listed_magnetar_bursts_beside_the_cells() {
    let host = SystemId::from_raw(LISTED_MAGNETAR).unwrap();
    let at = GalacticPosition::from_light_years([201.853, 25_409.364, -165.497]).unwrap();
    let x = reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 500.0);
    let query = sensor_query(ClassId::CORE_COLLAPSE, at, x, (0, 100));
    let seen = listed_beside_the_cells(&query, host, "alerts/listed_magnetar");
    assert_eq!(seen.len(), 18, "the listed magnetar bursts: {seen:?}");
    assert!(
        seen.iter().all(|d| d.kind() == TransientKind::MagnetarBurst
            && d.class() == Some(ClassId::CORE_COLLAPSE))
    );
}

/// The galactic centre's list is read for a walk whose sphere it can reach: its expected
/// candidates and its draws of the class are its list's, and every death read is one of its
/// members, dying in the walk's cut.
#[test]
fn the_centre_s_list_is_read_as_its_own() {
    let galaxy = seed_9(true);
    let x = reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 10.0);
    let class = ClassId::CORE_COLLAPSE;
    let query = sensor_query(class, GalacticPosition::ORIGIN, x, (0, 100));
    let mut walk = ListedWalk::default();
    walk.read_centre(galaxy, &query, &Reaches::of(class, &query), class);
    let model = galaxy.centre().unwrap();
    let recent = CentreRecent::new(galaxy, model, victim_count(galaxy).unwrap());
    let expected: f64 = recent
        .expected()
        .iter()
        .filter(|(c, _)| *c == RecentClass::CoreCollapse)
        .map(|(_, e)| e)
        .sum();
    assert_same_bits(walk.expected(), expected);
    assert_eq!(walk.drawn(), u64::from(recent.list(galaxy).count(class)));
    assert!(walk.drawn() > 0);
    let balls = Balls::of_centre(&query, &Reaches::of(class, &query)).expect("in reach");
    let wanted = balls.wanted(&query);
    for death in &walk.deaths {
        assert!(
            matches!(death.host.kind(), SystemIdKind::Centre(_)),
            "{death:?}"
        );
        assert!(wanted(death.explosion));
    }
}

/// A list's site pad grows with its distance and its speed and never passes the cap's
/// [`LIFETIME_REACH`]; the lookup's pad at the lookup's radius and the pad speed bounds the pad of
/// any list it keeps; and a slab's bounds are its cells', the first and the last open.
#[test]
fn the_pads_bound_the_lists_sites_and_the_slabs_part_the_features() {
    let x = reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 500.0);
    let query = sensor_query(ClassId::CORE_COLLAPSE, sun(), x, (0, 100));
    let beta = MetresPerSecond::from(UNBOUND_PAD_SPEED).value() / SPEED_OF_LIGHT;
    let near = site_pad(&query, SUPERNOVA_LIGHT_YEARS, 100.0, 50.0, 1e-4);
    assert!(near > 0.0 && near < site_pad(&query, SUPERNOVA_LIGHT_YEARS, 1e4, 50.0, 1e-4));
    assert!(near < site_pad(&query, SUPERNOVA_LIGHT_YEARS, 100.0, 50.0, 1e-3));
    assert_same_bits(
        site_pad(&query, magnetar_span_years(), 3e5, 4_096.0, beta),
        LIFETIME_REACH.value(),
    );
    for radius in [10.0, 1_000.0, 30_000.0] {
        let bound = lookup_pad(&query, SUPERNOVA_LIGHT_YEARS, radius, 4_096.0, beta);
        let farthest = radius + 4_096.0 + bound;
        assert!(site_pad(&query, SUPERNOVA_LIGHT_YEARS, farthest, 4_096.0, beta) <= bound);
    }
    let slabs = -3..4;
    let bounds = |x| slab_bounds(&slabs, x, 512.0);
    for (x, (lo, hi)) in [
        (-3, (f64::NEG_INFINITY, -1_024.0)),
        (0, (0.0, 512.0)),
        (3, (1_536.0, f64::INFINITY)),
    ] {
        assert_same_bits(bounds(x).0, lo);
        assert_same_bits(bounds(x).1, hi);
    }
}
