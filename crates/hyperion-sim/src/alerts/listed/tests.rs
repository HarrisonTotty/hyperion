//! The tests of the lists' walks (P09.T36, P09.T43 and P11.T8.f in plan 12's walks).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::Seed;
use crate::alerts::local::LocalEventsCache;
use crate::alerts::recurrent::RecurrentProcess;
use crate::alerts::{
    AlertBand, AlertResult, AlertSource, BandLimits, ClassCensus, ClassSet, ClassStatus,
    DetectionPhase, SensorHorizon, SourceCensus, alerts_in, magnetar_scan_cell, one_shot_scan_cell,
};
use crate::events::TimeWindow;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::catalogue_classes::binary::feature::ListedBinary;
use crate::galaxy::catalogue_classes::binary::{NeutronStarMergers, StellarMergers};
use crate::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use crate::galaxy::catalogue_classes::supernova::explosion_event_of;
use crate::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use crate::galaxy::catalogue_classes::{
    CatalogueCellKey, cells_in_slab, entries_when, occupied_cells,
};
use crate::galaxy::features::catalogue::{FeatureCellContents, NoFeatureCache};
use crate::galaxy::features::centre::events::victims::victim_count;
use crate::galaxy::features::centre::recent::CentreRecent;
use crate::galaxy::features::members::recent::{FeatureClassesRules, RecentClass};
use crate::galaxy::gas::ccm::Band;
use crate::galaxy::gas::modifiers::NoModifiers;
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::{FeatureCell, SystemIdKind};
use crate::observe::NoStarsCache;
use crate::planetary::testing::in_shares;
use crate::units::WattsPerSquareMetre;

/// The shares a whole-cube walk runs in.
const SHARES: usize = 16;

/// The version's rules: the lists walked.
fn listed() -> GalaxyRules {
    GalaxyRules::of_version(crate::GENERATOR_VERSION)
}

/// The 26-like reference: the field's budget, which held every young star at 26, and no lists.
fn budget() -> GalaxyRules {
    GalaxyRules {
        field_share: FieldShareRules::None,
        feature_classes: FeatureClassesRules::Unlisted,
        ..listed()
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

/// A cell's lists, built once.
type BuiltOnce = Arc<OnceLock<Arc<ListedCell>>>;

/// A listed hosts cache that keeps every cell's lists and the centre's it is asked for, for one
/// galaxy and many threads, each built once while the threads that ask for it wait, counting the
/// lists it builds.
#[derive(Debug, Default)]
struct KeepLists {
    cells: Mutex<BTreeMap<[i32; 3], BuiltOnce>>,
    centre: OnceLock<Arc<ListedHosts>>,
    built: AtomicU64,
}

impl KeepLists {
    /// How many cells' lists and centres it has built.
    fn built(&self) -> u64 {
        self.built.load(Ordering::Relaxed)
    }

    /// The bytes its lists own and take themselves, as a byte-bounded cache would charge them
    /// without its entries' overhead: each kept cell's lists' and the centre's.
    fn bytes(&self) -> usize {
        let cells = self.cells.lock().unwrap_or_else(PoisonError::into_inner);
        let lists = cells
            .values()
            .filter_map(|slot| slot.get())
            .map(|cell| size_of::<ListedCell>() + cell.heap_bytes())
            .sum::<usize>();
        lists
            + self
                .centre
                .get()
                .map_or(0, |centre| size_of::<ListedHosts>() + centre.heap_bytes())
    }
}

impl ListedHostsCache for KeepLists {
    fn cell(
        &self,
        galaxy: &Galaxy,
        cell: FeatureCell,
        features: &dyn FeatureCellCache,
    ) -> Arc<ListedCell> {
        let slot = Arc::clone(
            self.cells
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(cell.to_array())
                .or_default(),
        );
        Arc::clone(slot.get_or_init(|| {
            self.built.fetch_add(1, Ordering::Relaxed);
            NoListedHostsCache.cell(galaxy, cell, features)
        }))
    }

    fn centre(&self, galaxy: &Galaxy) -> Arc<ListedHosts> {
        Arc::clone(self.centre.get_or_init(|| {
            self.built.fetch_add(1, Ordering::Relaxed);
            NoListedHostsCache.centre(galaxy)
        }))
    }
}

/// The Sun-like point.
fn sun() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap()
}

/// The query of `class` at `at` through one band's detection limit, W m⁻², resolving at a hundred
/// times it, over `[from, to)` years.
fn query(
    class: ClassId,
    at: GalacticPosition,
    (band, detect): (AlertBand, f64),
    (from, to): (i64, i64),
) -> AlertQuery {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(detect * 100.0)).unwrap();
    let sensor = SensorHorizon::new([(band, limits)]).unwrap();
    sensor_query(class, at, sensor, (from, to))
}

/// A class's process as a background walk builds it once for every slab (the server's
/// `BackgroundProcess`).
#[expect(
    clippy::large_enum_variant,
    reason = "one is built a walk and lent to every slab"
)]
enum CellProcess {
    CoreCollapse(CoreCollapseProcess),
    TypeIa(TypeIaProcess),
    StellarMergers(StellarMergers),
    NeutronStarMergers(NeutronStarMergers),
    Recurrent(RecurrentProcess),
}

impl CellProcess {
    /// `class`'s process in `galaxy`.
    fn of(galaxy: &Galaxy, class: ClassId) -> Self {
        match class {
            ClassId::CORE_COLLAPSE => Self::CoreCollapse(CoreCollapseProcess::new(galaxy)),
            ClassId::TYPE_IA => Self::TypeIa(TypeIaProcess::new(galaxy)),
            ClassId::STELLAR_MERGER => Self::StellarMergers(StellarMergers::new(galaxy)),
            ClassId::NEUTRON_STAR_MERGER => {
                Self::NeutronStarMergers(NeutronStarMergers::new(galaxy))
            }
            _ => Self::Recurrent(RecurrentProcess::of(galaxy, class).expect("a recurrent class")),
        }
    }

    /// The occupied cells of `cells`.
    fn occupied(
        &self,
        galaxy: &Galaxy,
        cells: impl IntoIterator<Item = CatalogueCellKey>,
    ) -> Vec<(CatalogueCellKey, u32)> {
        match self {
            Self::CoreCollapse(p) => occupied_cells(galaxy, p, cells),
            Self::TypeIa(p) => occupied_cells(galaxy, p, cells),
            Self::StellarMergers(p) => occupied_cells(galaxy, p, cells),
            Self::NeutronStarMergers(p) => occupied_cells(galaxy, p, cells),
            Self::Recurrent(p) => p.occupied_cells(galaxy, cells),
        }
    }

    /// The detections of the class's cell `cell` for `query`, as a background scan walks it.
    fn scan(
        &self,
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        query: &AlertQuery,
        cell: CatalogueCellKey,
        out: &mut Vec<Detection>,
    ) {
        let sees = |kind| kind_reach(kind, query.sensor()) > LightYears::ZERO;
        match self {
            Self::CoreCollapse(p) => {
                if sees(TransientKind::CoreCollapse) {
                    one_shot_scan_cell(galaxy, ctx, query, p, cell, out);
                }
                if sees(TransientKind::MagnetarBurst) {
                    magnetar_scan_cell(galaxy, ctx, query, p, cell, out);
                }
            }
            Self::TypeIa(p) => one_shot_scan_cell(galaxy, ctx, query, p, cell, out),
            Self::StellarMergers(p) => one_shot_scan_cell(galaxy, ctx, query, p, cell, out),
            Self::NeutronStarMergers(p) => one_shot_scan_cell(galaxy, ctx, query, p, cell, out),
            Self::Recurrent(p) => p.scan_cell(galaxy, ctx, cell, query, out),
        }
    }
}

/// What a whole-cube walk of a class found: the cells' detections and the lists', the candidates
/// the lists drew, and the seconds the shares spent on the cells and on the lists (a measurement,
/// logged).
#[derive(Debug, Default)]
struct CubeWalk {
    cells: Vec<Detection>,
    lists: Vec<Detection>,
    drawn: u64,
    seconds: [f64; 2],
}

/// Every feature cell of `galaxy` made in `features` once, in [`SHARES`] shares split across the
/// plane, ahead of a walk whose slabs each read some of them.
fn every_feature_cell_made(galaxy: &Galaxy, features: &KeepCells) {
    let shares = i32::try_from(SHARES).unwrap();
    in_shares(SHARES, SHARES, |share| {
        let share = i32::try_from(share).unwrap();
        for cell in every_feature_cell()
            .filter(|cell| (cell.to_array()[0] + cell.to_array()[1]).rem_euclid(shares) == share)
        {
            let _ = features.contents(galaxy, cell);
        }
    });
}

/// The whole cube's hosts of `query`'s class in `galaxy`, slab by slab of the class's sphere as a
/// background scan walks it, in [`SHARES`] shares: each slab's occupied cells, cut as the scan cuts
/// them, and with `cells` walked, its lists ([`listed_scan_slab`]) through `lists`.
fn whole_cube(
    galaxy: &Galaxy,
    query: &AlertQuery,
    (features, lists): (&KeepCells, &KeepLists),
    cells: bool,
) -> CubeWalk {
    let class = class_of(query);
    let process = CellProcess::of(galaxy, class);
    let (sphere, slabs) = class_sphere(query);
    let shares = in_shares(SHARES, SHARES, |share| {
        let mut noise = NoiseCache::with_capacity(4_096);
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers)
            .with_feature_cells(features)
            .with_listed_hosts(lists);
        let mut walked = CubeWalk::default();
        for &x in slabs.iter().skip(share).step_by(SHARES) {
            let started = std::time::Instant::now();
            if cells {
                for (cell, _) in process.occupied(galaxy, cells_in_slab(class, &sphere, x)) {
                    process.scan(galaxy, &mut ctx, query, cell, &mut walked.cells);
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
    let mut walked = CubeWalk::default();
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

/// The onsets of `detections`.
fn onsets(detections: &[Detection]) -> usize {
    detections
        .iter()
        .filter(|d| d.phase() == DetectionPhase::Onset)
        .count()
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
/// through a V limit of 10⁻¹² over the first 1,000 years, under the version's rules (the cells and
/// the lists) and in the 26-like budget (the field holding every young star, no lists). The
/// magnetars are walked twice through one cache of the lists, cold and then warm, and the second walk builds no list and finds the
/// first's detections, each to the bit: the lists' cost before the cache and after it
/// (fix-p09-alert-walk-features-2), logged.
#[test]
#[ignore = "slow: every feature's list and the core collapses' cells of the cube, twice"]
fn the_whole_cube_s_core_collapses_with_the_lists_measured() {
    let params = GalaxyParams::milky_way_like;
    let class = ClassId::CORE_COLLAPSE;
    let magnetars = query(class, sun(), (AlertBand::XRay, 1e-14), (0, 100));
    let supernovae = query(
        class,
        sun(),
        (AlertBand::Photometric(Band::V), 1e-12),
        (0, 1_000),
    );
    for (label, rules) in [("29", listed()), ("budget", budget())] {
        let galaxy = galaxy(0x0926_0000, params(), rules);
        let (features, lists) = (KeepCells::default(), KeepLists::default());
        every_feature_cell_made(&galaxy, &features);
        let bursts = whole_cube(&galaxy, &magnetars, (&features, &lists), true);
        let cold = lists.built();
        let explosions = whole_cube(&galaxy, &supernovae, (&features, &lists), true);
        let again = whole_cube(&galaxy, &magnetars, (&features, &lists), false);
        assert_eq!(again.lists, bursts.lists, "a warm walk finds the cold's");
        assert_eq!(again.drawn, bursts.drawn);
        assert!(bursts.cells.iter().all(|d| !is_listed(d.host())));
        assert!(bursts.lists.iter().all(|d| is_listed(d.host())));
        eprintln!(
            "{label}: magnetars, cells {} detections from {} hosts, lists {} from {} hosts \
             ({} candidates drawn); supernovae, cells {} onsets ({} detections), lists {} onsets \
             ({} detections, {} candidates drawn); thread-seconds, cells {:.1} and {:.1}, lists \
             {:.1} cold and {:.1} warm, and {:.1}; {cold} lists' cells and centres built, {} \
             after, {:.1} MB of lists kept",
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
            again.seconds[1],
            explosions.seconds[1],
            lists.built(),
            1e-6 * f64::from(u32::try_from(lists.bytes()).unwrap_or(u32::MAX)),
        );
        assert_eq!(lists.built(), cold, "the warm walks build no list");
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
        if label == "29" {
            assert!(bursts.drawn > 0 && explosions.drawn > 0);
        } else {
            assert_eq!((bursts.lists.len(), explosions.lists.len()), (0, 0));
        }
    }
}

/// The census of each class the lists hold beside its cells, measured and logged as the brief's
/// count table, not asserted beyond its shape: the whole cube's hosts and detections of each from
/// the Sun-like point in the Milky Way fixture of seed `0x0926_0000`, under the version's rules (the
/// cells and the lists) and in the 26-like budget (the field holding every young star, no lists).
/// The sensors: X-rays at 10⁻¹⁴ W m⁻² and V
/// at 10⁻¹² (some twelfth magnitude), over windows that see each class's events: the core
/// collapses' as [`the_whole_cube_s_core_collapses_with_the_lists_measured`]'s, the novae' and the
/// X-ray binaries' over a year, the rest over a millennium.
#[test]
#[ignore = "slow: every class's cells and lists of the cube, twice"]
fn every_listed_class_s_whole_cube_hosts_counted() {
    let params = GalaxyParams::milky_way_like;
    let (x, v) = (
        (AlertBand::XRay, 1e-14),
        (AlertBand::Photometric(Band::V), 1e-12),
    );
    let classes = [
        (ClassId::CORE_COLLAPSE, x, (0, 100)),
        (ClassId::CORE_COLLAPSE, v, (0, 1_000)),
        (ClassId::TYPE_IA, v, (0, 1_000)),
        (ClassId::LUMINOUS_BLUE_VARIABLE, v, (0, 1_000)),
        (ClassId::ACCRETING_WHITE_DWARF, v, (0, 1)),
        (ClassId::ACCRETING_WHITE_DWARF_SLOW, v, (0, 1_000)),
        (ClassId::XRAY_BINARY, x, (0, 1)),
        (ClassId::STELLAR_MERGER, v, (0, 1_000)),
        (ClassId::NEUTRON_STAR_MERGER, v, (0, 1_000)),
    ];
    for (label, rules) in [("29", listed()), ("budget", budget())] {
        let galaxy = galaxy(0x0926_0000, params(), rules);
        let (features, lists) = (KeepCells::default(), KeepLists::default());
        every_feature_cell_made(&galaxy, &features);
        for &(class, (band, detect), window) in &classes {
            let query = query(class, sun(), (band, detect), window);
            let walked = whole_cube(&galaxy, &query, (&features, &lists), true);
            eprintln!(
                "COUNT {label} {class} {band:?} {detect:e} {window:?}: cells {} hosts, {} \
                 detections ({} onsets); lists {} hosts, {} detections ({} onsets), {} drawn; \
                 thread-seconds cells {:.1}, lists {:.1}",
                hosts(&walked.cells),
                walked.cells.len(),
                onsets(&walked.cells),
                hosts(&walked.lists),
                walked.lists.len(),
                onsets(&walked.lists),
                walked.drawn,
                walked.seconds[0],
                walked.seconds[1],
            );
            assert!(walked.cells.iter().all(|d| !is_listed(d.host())));
            assert!(walked.lists.iter().all(|d| is_listed(d.host())));
            if label == "budget" {
                assert_eq!(walked.lists.len(), 0);
            }
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

/// The query of `class` from the galactic centre over [−1,000, 1,000) years through a sensor that
/// sees each of its kinds: what reads the whole of the centre's list of it.
fn from_the_centre(class: ClassId) -> (AlertQuery, Reaches) {
    let (walked, kinds) = Walked::of(class).expect("a listed class");
    let mut every = [None; MAX_KINDS];
    for (slot, &kind) in every.iter_mut().zip(kinds) {
        *slot = Some((kind, crate::alerts::ROOT_CUBE_DIAGONAL));
    }
    let all = sensor_query(
        class,
        GalacticPosition::ORIGIN,
        reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 1.0),
        (-1_000, 1_000),
    );
    (
        all,
        Reaches {
            walked,
            kinds: every,
        },
    )
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
        let all = query(
            class,
            sun(),
            (AlertBand::Photometric(Band::V), 1e-30),
            (0, 1_000),
        );
        let reaches = Reaches::of(class, &all).expect("seen");
        centre.read_centre(&galaxy, &NoListedHostsCache, (&all, &reaches, class));
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

/// Seed 9's galaxy (`Galaxy::new`'s parameters, the server's alert fixture) under the version's
/// rules ([`listed`]), with its kinematic tables, built once.
fn seed_9() -> &'static Galaxy {
    static LISTED: OnceLock<Galaxy> = OnceLock::new();
    LISTED.get_or_init(|| {
        let params = GalaxyParams::from_seed(
            Seed::new(9),
            crate::galaxy::imf::MassFunctionKind::default(),
        );
        galaxy(9, params, listed())
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

/// What `alerts_in` finds for `query` in `galaxy`, the feature cells generated for each lookup and
/// the lists read through `lists`.
fn alerts(galaxy: &Galaxy, query: &AlertQuery, lists: &dyn ListedHostsCache) -> AlertResult {
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_listed_hosts(lists);
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
/// cells kept in `features` and the lists in `lists`, and the candidates they drew
/// ([`listed_scan_slab`]).
fn slab(
    galaxy: &Galaxy,
    query: &AlertQuery,
    (sphere, x): (&QuerySphere, i32),
    (features, lists): (&KeepCells, &KeepLists),
) -> (Vec<Detection>, u64) {
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers)
        .with_feature_cells(features)
        .with_listed_hosts(lists);
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
/// order of the slabs, through one cache of feature cells and one of lists.
fn slabs(galaxy: &Galaxy, query: &AlertQuery) -> (Vec<Detection>, u64) {
    let (sphere, slabs) = class_sphere(query);
    let caches = (&KeepCells::default(), &KeepLists::default());
    assert_order_independent(&slabs, |&x| slab(galaxy, query, (&sphere, x), caches));
    let mut merged = Vec::new();
    let mut drawn = 0;
    for &x in &slabs {
        let (out, n) = slab(galaxy, query, (&sphere, x), caches);
        merged.extend(out);
        drawn += n;
    }
    sort_detections(&mut merged);
    (merged, drawn)
}

/// The lists' detections for `query` in `galaxy` without the walk's lookup, pre-tests, cut or kept
/// lists: every host of the class on the list of every feature whose centre lies within the
/// class's kinds' reaches, what lies beyond a site and the cap's [`LIFETIME_REACH`], each list's
/// entries built whole, and every host of the centre's ([`from_the_centre`]). Each death built
/// under a time read first is the whole entry's, bit for bit, where it is kept.
fn brute_force(galaxy: &Galaxy, query: &AlertQuery) -> Vec<Detection> {
    let class = class_of(query);
    let reaches = Reaches::of(class, query).expect("the sensor sees the class");
    let radius = reaches
        .seen()
        .map(|(kind, r)| r.value() + beyond_ly(kind) + LIFETIME_REACH.value())
        .fold(0.0, f64::max);
    let before = |t: UniverseTime| t < UniverseTime::EPOCH;
    let observer = query.observer_position();
    let mut walk = ListedWalk::default();
    let near = FeatureCatalogue::near(galaxy, observer, LightYears::new(radius), &NoFeatureCache);
    for feature in near {
        let list = FeatureLevelList::of(galaxy, &feature);
        for entry in list.entries().filter(|entry| entry.class() == class) {
            let index = entry.index();
            let binary = || list.binary_entry(galaxy, index);
            match reaches.walked {
                Walked::Deaths => {
                    let whole = list.recent_entry(galaxy, index);
                    assert_eq!(
                        list.recent_entry_when(galaxy, index, &before),
                        whole
                            .clone()
                            .filter(|e| before(e.site().expect("a death").death())),
                        "the entry {index} of {:?}",
                        feature.id()
                    );
                    walk.deaths
                        .extend(whole.and_then(|e| Death::of_feature(galaxy, &e)));
                }
                Walked::Eruptions => {
                    walk.eruptions
                        .extend(list.recent_entry(galaxy, index).map(|entry| {
                            let member = entry.member();
                            let star = StarRecipe::of_record(
                                galaxy,
                                member.record(),
                                *member.composition(),
                            );
                            let window = star.to_star().lbv_window().expect("a variable's window");
                            EruptionHost::new(member.record().id(), window, *entry.motion(), star)
                                .unwrap()
                        }));
                }
                Walked::Novae => walk.novae.extend(binary().map(|entry| match entry {
                    ListedBinary::AccretingWd(entry) => *entry.scan(),
                    other => panic!("a white dwarf's entry: {other:?}"),
                })),
                Walked::XrayBinaries => {
                    walk.xray_binaries.extend(binary().map(|entry| match entry {
                        ListedBinary::XrayBinary(entry) => *entry.scan(),
                        other => panic!("an X-ray binary's entry: {other:?}"),
                    }));
                }
                Walked::Mergers => {
                    walk.mergers.extend(binary().and_then(|entry| {
                        super::super::oneshot::listed_merger_event(galaxy, &entry)
                    }));
                }
            }
        }
    }
    let (all, every) = from_the_centre(class);
    let mut centre = ListedWalk::default();
    centre.read_centre(galaxy, &NoListedHostsCache, (&all, &every, class));
    walk.deaths.extend(centre.deaths);
    walk.eruptions.extend(centre.eruptions);
    walk.novae.extend(centre.novae);
    walk.xray_binaries.extend(centre.xray_binaries);
    walk.mergers.extend(centre.mergers);
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut out = Vec::new();
    walk.detections(galaxy, &mut ctx, query, class, &mut out);
    out
}

/// `query`'s class's walk sees `host`'s detections beside the cells' (module documentation), each
/// named on the list's member; the census counts the lists' expected candidates and finds the
/// lists' source complete. The walk's lists find what every list near enough finds whole
/// ([`brute_force`]), and the same with the lists kept between walks as without, the second walk
/// building none. A background scan's slabs give the lists' detections in any order and count the
/// candidates the interactive walk's lists drew. Returns 29's detections of `host`, pinned in the
/// golden `name`.
fn listed_beside_the_cells(query: &AlertQuery, host: SystemId, name: &str) -> Vec<Detection> {
    let class = class_of(query);
    let new = seed_9();
    let after = alerts(new, query, &NoListedHostsCache);
    let kept = KeepLists::default();
    assert_eq!(
        alerts(new, query, &kept),
        after,
        "the kept lists' walk is the built's"
    );
    let built = kept.built();
    assert!(built > 0);
    assert_eq!(alerts(new, query, &kept), after, "a walk of the kept lists");
    assert_eq!(kept.built(), built, "the second walk builds no list");
    let listed: Vec<Detection> = after
        .detections()
        .iter()
        .filter(|d| is_listed(d.host()))
        .cloned()
        .collect();
    let none = (
        &NoFeatureCache as &dyn FeatureCellCache,
        &NoListedHostsCache as &dyn ListedHostsCache,
    );
    let lists = ListedWalk::find(new, none, query, class).expect("read");
    let census = after.class_census(class).expect("asked");
    assert_eq!(census.status(), ClassStatus::Complete);
    assert!(census.expected_hosts().expect("counted") >= lists.expected());
    assert!(lists.expected() > 0.0);
    let source = after
        .sources()
        .iter()
        .find(|source| source.source() == AlertSource::FeatureLists)
        .map(SourceCensus::status);
    assert_eq!(source, Some(SourceStatus::Complete));
    let mut whole = brute_force(new, query);
    sort_detections(&mut whole);
    assert_eq!(whole, listed, "the lists' walk finds every list's whole");
    let (by_slab, drawn) = slabs(new, query);
    assert_eq!(
        by_slab, listed,
        "the slabs give the interactive walk's lists"
    );
    assert_eq!(drawn, lists.drawn());
    pinned(listed, host, name)
}

/// The detections of `host` among `listed`, pinned in the golden `name`.
fn pinned(listed: Vec<Detection>, host: SystemId, name: &str) -> Vec<Detection> {
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

/// A background scan of `query`'s class, whose census sends it to the background whatever its
/// reach, sees `host`'s detections on the lists slab by slab (module documentation), each named on
/// the list's member; the slabs' lists find what every list near enough finds whole
/// ([`brute_force`]), in any order of the slabs. Returns the detections of `host`, pinned in the
/// golden `name`.
fn listed_in_the_background(query: &AlertQuery, host: SystemId, name: &str) -> Vec<Detection> {
    let class = class_of(query);
    let new = seed_9();
    let census = alerts(new, query, &NoListedHostsCache);
    let status = census.class_census(class).map(ClassCensus::status);
    assert!(
        matches!(
            status,
            Some(ClassStatus::OverLimit | ClassStatus::NeedsBackgroundScan)
        ),
        "{class} waits for the background: {status:?}"
    );
    let (listed, drawn) = slabs(new, query);
    assert!(drawn > 0);
    assert!(listed.iter().all(|d| is_listed(d.host())));
    let mut whole = brute_force(new, query);
    sort_detections(&mut whole);
    assert_eq!(whole, listed, "the slabs' lists find every list's whole");
    pinned(listed, host, name)
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
    let galaxy = seed_9();
    let x = reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 10.0);
    let class = ClassId::CORE_COLLAPSE;
    let query = sensor_query(class, GalacticPosition::ORIGIN, x, (0, 100));
    let reaches = Reaches::of(class, &query).expect("seen");
    let mut walk = ListedWalk::default();
    walk.read_centre(galaxy, &NoListedHostsCache, (&query, &reaches, class));
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
    let balls = Balls::of_centre(&query, &reaches, 0.0).expect("in reach");
    let wanted = balls.wanted(&query);
    for death in &walk.deaths {
        assert!(
            matches!(death.host.kind(), SystemIdKind::Centre(_)),
            "{death:?}"
        );
        assert!(wanted(death.explosion));
    }
}

/// A list's site pad grows with its distance and its speed and, for hosts that die, never passes
/// the cap's [`LIFETIME_REACH`], which a living host's passes; the lookup's pad at the lookup's
/// radius and the pad speed bounds the pad of any list it keeps, capped or not; and a slab's bounds
/// are its cells', the first and the last open.
#[test]
fn the_pads_bound_the_lists_sites_and_the_slabs_part_the_features() {
    let x = reaching(AlertBand::XRay, TransientKind::MagnetarBurst, 500.0);
    let query = sensor_query(ClassId::CORE_COLLAPSE, sun(), x, (0, 100));
    let beta = MetresPerSecond::from(UNBOUND_PAD_SPEED).value() / SPEED_OF_LIGHT;
    for walked in [Walked::Deaths, Walked::Novae] {
        let pad = |span, d, reach, beta| site_pad(&query, span, d, (reach, beta), walked);
        let near = pad(SUPERNOVA_LIGHT_YEARS, 100.0, 50.0, 1e-4);
        assert!(near > 0.0 && near < pad(SUPERNOVA_LIGHT_YEARS, 1e4, 50.0, 1e-4));
        assert!(near < pad(SUPERNOVA_LIGHT_YEARS, 100.0, 50.0, 1e-3));
        for radius in [10.0, 1_000.0, 30_000.0] {
            let bound = lookup_pad(
                &query,
                SUPERNOVA_LIGHT_YEARS,
                (radius, 4_096.0),
                beta,
                walked,
            );
            let farthest = radius + 4_096.0 + bound;
            assert!(pad(SUPERNOVA_LIGHT_YEARS, farthest, 4_096.0, beta) <= bound);
        }
    }
    let far = |walked| site_pad(&query, magnetar_span_years(), 3e5, (4_096.0, beta), walked);
    assert_same_bits(far(Walked::Deaths), LIFETIME_REACH.value());
    assert!(far(Walked::Eruptions) > LIFETIME_REACH.value());
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

/// Every class the lists hold is walked for each kind the class's own walk sees, in its order, and
/// a class no catalogue feature's list holds, the centre's victims, for none; a query that does not
/// ask about a class, or a sensor that sees none of its kinds, reads no list of it.
#[test]
fn every_listed_class_is_walked_for_its_own_kinds() {
    use TransientKind as K;
    let listed = [
        (
            ClassId::CORE_COLLAPSE,
            vec![K::CoreCollapse, K::MagnetarBurst],
        ),
        (ClassId::TYPE_IA, vec![K::TypeIa]),
        (ClassId::LUMINOUS_BLUE_VARIABLE, vec![K::GiantEruption]),
        (ClassId::ACCRETING_WHITE_DWARF, vec![K::Nova]),
        (ClassId::ACCRETING_WHITE_DWARF_SLOW, vec![K::Nova]),
        (
            ClassId::XRAY_BINARY,
            vec![K::XrayTransient, K::XrayBurst, K::BeXOutburst],
        ),
        (ClassId::STELLAR_MERGER, vec![K::LuminousRedNova]),
        (ClassId::NEUTRON_STAR_MERGER, vec![K::Kilonova]),
    ];
    for (class, kinds) in &listed {
        let (_, walked) = Walked::of(*class).expect("a listed class");
        assert_eq!(walked, kinds.as_slice(), "{class}");
        for kind in kinds {
            assert!(
                kind.row().classes().contains(*class),
                "{class} makes {kind:?}"
            );
        }
    }
    assert_eq!(Walked::of(ClassId::TIDAL_DISRUPTION_VICTIM), None);
    let radio = query(
        ClassId::XRAY_BINARY,
        sun(),
        (AlertBand::Photometric(Band::Radio), 1e-30),
        (0, 1),
    );
    assert_eq!(Reaches::of(ClassId::XRAY_BINARY, &radio), None);
    let x = query(
        ClassId::XRAY_BINARY,
        sun(),
        (AlertBand::XRay, 1e-14),
        (0, 1),
    );
    assert_eq!(
        Reaches::of(ClassId::ACCRETING_WHITE_DWARF, &x),
        None,
        "not asked"
    );
    let seen: Vec<_> = Reaches::of(ClassId::XRAY_BINARY, &x)
        .expect("seen")
        .seen()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(seen, [K::XrayTransient, K::XrayBurst, K::BeXOutburst]);
}

/// A cell's lists are built of its whole contents: one made through some of its processes is
/// refused.
#[test]
#[should_panic(expected = "a cell's lists are of its whole contents")]
fn a_cell_s_lists_are_of_its_whole_contents() {
    let galaxy = seed_9();
    let cell = FeatureCell::new([0, 6, -1]).unwrap();
    let partial = FeatureCatalogue::cell_through(
        galaxy,
        cell,
        crate::galaxy::features::FeatureProcess::ALL[0],
    );
    let _ = ListedCell::of(galaxy, &partial);
}

/// A cell's lists, kept, are the lists of its features as each is built: each feature that
/// registers a class with its classes' expected candidates and counts, its list's bit for bit; a
/// cell's hosts found by a walk change none of them, nor what the cell charges a cache.
#[test]
fn a_cell_s_lists_are_its_features_lists() {
    let galaxy = seed_9();
    let cell = FeatureCell::new([0, 6, -1]).unwrap();
    let contents = FeatureCatalogue::cell(galaxy, cell);
    let listed = ListedCell::of(galaxy, &contents);
    let charged = listed.heap_bytes();
    let mut lists = 0;
    for feature in contents.features() {
        let list = FeatureLevelList::of(galaxy, feature);
        let hosts = listed.hosts(feature);
        let classes: Vec<_> = list.class_counts().collect();
        assert_eq!(hosts.is_some(), !classes.is_empty(), "{:?}", feature.id());
        let Some(hosts) = hosts else {
            continue;
        };
        lists += 1;
        for (class, expected, count) in classes {
            let (e, n) = hosts.class(class);
            assert_same_bits(e, expected);
            assert_eq!(n, count, "{class} of {:?}", feature.id());
            let found = hosts.feature_hosts(galaxy, feature, class);
            assert_eq!(found.is_some(), count > 0, "{class} of {:?}", feature.id());
        }
    }
    assert!(lists > 10, "the cell lists {lists}");
    assert_eq!(
        listed.heap_bytes(),
        charged,
        "a cell charges the same whatever has read it"
    );
    assert_eq!(listed, ListedCell::of(galaxy, &contents));
}

/// The member of the star-forming region 1584 of feature cell [0, 2, −1] in seed 9's galaxy, a
/// luminous blue variable whose giant eruption begins 176.863 years after the epoch at
/// [1,058.7, 9,229.2, 12.0] ly: the lists' variable.
const LISTED_VARIABLE: u64 = 0xe849_e318_7000_0001;

/// The member of the globular 2 of feature cell [1, −1, −1] in seed 9's galaxy whose stars merge
/// 427.430 years before the epoch at [5,897.7, −2,698.4, −121.1] ly: the lists' stellar merger.
const LISTED_MERGER: u64 = 0xe8bd_e001_7000_015d;

/// The member of the open cluster 15 of feature cell [−6, 3, 0] in seed 9's galaxy, a fast
/// accreting white dwarf whose nova begins 82.172 years after the epoch at
/// [−23,339.4, 16,014.8, 486.6] ly: the lists' fast nova.
const LISTED_FAST_NOVA: u64 = 0xe54e_0007_f000_0001;

/// The member of the globular 0 of feature cell [−3, −2, −5] in seed 9's galaxy, a slow accreting
/// white dwarf whose nova begins 764.574 years after the epoch at [−9,792.8, −5,763.1,
/// −17,139.2] ly, in the halo: the lists' slow nova.
const LISTED_SLOW_NOVA: u64 = 0xe6b9_6000_7000_0057;

/// The member of the globular 0 of feature cell [−3, 1, −1] in seed 9's galaxy, a neutron-star
/// transient whose one outburst of the clock window begins 129.399 years before the epoch at
/// [−11,565.9, 7,330.7, −232.6] ly: the lists' X-ray binary.
const LISTED_TRANSIENT: u64 = 0xe6c5_e000_7000_0014;

/// The point `ahead` light-years above `site` along +z.
fn above(site: [f64; 3], ahead: f64) -> GalacticPosition {
    GalacticPosition::from_light_years([site[0], site[1], site[2] + ahead]).unwrap()
}

/// From 29 `query`'s walk sees one onset of `kind` of the listed `host`, on its list's member,
/// beside the cells', arriving at `arrival` years, which the golden `name` pins: in the
/// interactive walk ([`listed_beside_the_cells`]), or for a class its census sends to the
/// background in a background scan's ([`listed_in_the_background`]).
fn one_listed_onset(
    query: &AlertQuery,
    (host, kind): (u64, TransientKind),
    arrival: f64,
    name: &str,
) {
    let class = class_of(query);
    let host = SystemId::from_raw(host).unwrap();
    let seen = if class == ClassId::STELLAR_MERGER {
        listed_in_the_background(query, host, name)
    } else {
        listed_beside_the_cells(query, host, name)
    };
    let [onset] = seen.as_slice() else {
        panic!("one onset of the listed {kind:?}: {seen:?}");
    };
    assert_eq!(onset.phase(), DetectionPhase::Onset);
    assert_eq!(onset.kind(), kind);
    assert_eq!(onset.class(), Some(class));
    let at = onset.arrival().since_epoch().as_julian_years_f64();
    assert!((at - arrival).abs() < 0.01, "{at}");
}

/// The lists' variable, 1 ly above where its giant eruption begins, through a V sensor that reaches
/// 100 ly over [100, 200) years: from 29 its onset is seen, on its list's member, beside the
/// cells', a year after it begins.
#[test]
fn from_29_a_listed_variable_erupts_beside_the_cells() {
    let at = above([1_058.690, 9_229.188, 11.969], 1.0);
    let v = reaching(
        AlertBand::Photometric(Band::V),
        TransientKind::GiantEruption,
        100.0,
    );
    let query = sensor_query(ClassId::LUMINOUS_BLUE_VARIABLE, at, v, (100, 200));
    let host = (LISTED_VARIABLE, TransientKind::GiantEruption);
    one_listed_onset(&query, host, 177.863, "alerts/listed_variable");
}

/// The lists' stellar merger, 1 ly above where its stars merge, through a V sensor that reaches
/// 1,000 ly over [−500, −400) years, the brightest red nova's reach (its own V peak is 4 × 10⁻⁵ of
/// the kind's bound): from 29 its luminous red nova is seen, on its list's member, beside the
/// cells', a year after the merger, in a background scan, since the class's census, some 4 × 10⁷
/// candidates in a cell of its grid, is over the host limit at any reach.
#[test]
fn from_29_a_listed_merger_is_seen_beside_the_cells() {
    let at = above([5_897.660, -2_698.375, -121.097], 1.0);
    let v = reaching(
        AlertBand::Photometric(Band::V),
        TransientKind::LuminousRedNova,
        1_000.0,
    );
    let query = sensor_query(ClassId::STELLAR_MERGER, at, v, (-500, -400));
    let host = (LISTED_MERGER, TransientKind::LuminousRedNova);
    one_listed_onset(&query, host, -426.430, "alerts/listed_merger");
}

/// The lists' fast nova, 1 ly above it, through a V sensor that reaches 50 ly over [80, 90) years:
/// from 29 its onset is seen, on its list's member, beside the cells', a year after it begins.
#[test]
fn from_29_a_listed_fast_nova_is_seen_beside_the_cells() {
    let at = above([-23_339.431, 16_014.774, 486.560], 1.0);
    let v = reaching(AlertBand::Photometric(Band::V), TransientKind::Nova, 50.0);
    let query = sensor_query(ClassId::ACCRETING_WHITE_DWARF, at, v, (80, 90));
    let host = (LISTED_FAST_NOVA, TransientKind::Nova);
    one_listed_onset(&query, host, 83.172, "alerts/listed_fast_nova");
}

/// The lists' slow nova, 1 ly above it, through a V sensor that reaches 50 ly over [700, 800)
/// years: from 29 its onset is seen, on its list's member, beside the cells', a year after it
/// begins.
#[test]
fn from_29_a_listed_slow_nova_is_seen_beside_the_cells() {
    let at = above([-9_792.817, -5_763.148, -17_139.163], 1.0);
    let v = reaching(AlertBand::Photometric(Band::V), TransientKind::Nova, 50.0);
    let query = sensor_query(ClassId::ACCRETING_WHITE_DWARF_SLOW, at, v, (700, 800));
    let host = (LISTED_SLOW_NOVA, TransientKind::Nova);
    one_listed_onset(&query, host, 765.574, "alerts/listed_slow_nova");
}

/// The lists' X-ray transient, 1 ly above it, through an X-ray sensor that reaches 50 ly over
/// [−200, −100) years: from 29 its outburst's onset is seen, on its list's member, beside the
/// cells', a year after it begins.
#[test]
fn from_29_a_listed_x_ray_transient_is_seen_beside_the_cells() {
    let at = above([-11_565.893, 7_330.728, -232.559], 1.0);
    let x = reaching(AlertBand::XRay, TransientKind::XrayTransient, 50.0);
    let query = sensor_query(ClassId::XRAY_BINARY, at, x, (-200, -100));
    let host = (LISTED_TRANSIENT, TransientKind::XrayTransient);
    one_listed_onset(&query, host, -128.399, "alerts/listed_x_ray_transient");
}
