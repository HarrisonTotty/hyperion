//! The global list source's unit tests (P10.T9): the three test streams of `global_list::testing`
//! and a small dwarf core on a list of their own ([`GlobalList::of_parts`]), the streams' tubes
//! reduced from 64 tracers an arm, so that the sim's fast suite runs them on `wasm32-wasip1` too.
//! The brute-force comparisons, at the generator's 2,000 tracers, are slow and ignored.

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use super::*;
use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::Population;
use crate::galaxy::features::centre::NoCentreOrbitCache;
use crate::galaxy::global_list::members::expected_members;
use crate::galaxy::global_list::spec::{DwarfCoreSpec, SphericalOrbit, StreamSpec};
use crate::galaxy::global_list::spray::{Spray, SprayForce, SprayParams};
use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
use crate::galaxy::global_list::{DwarfCoreNumber, GlobalEntry};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{NoCache, SystemOrigin};
use crate::galaxy::potential::{MassModel, PotentialTables};
use crate::galaxy::query::{RangeQuery, range_query};
use crate::id::Layer;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::SolarMasses;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Years};

/// The galaxies' seed, the tubes' gap noise's and the sprays' too.
const SEED: Seed = Seed::new(0x0a10_9a00_0000_0000);

/// The stellar layers, every band a member can be in.
fn stellar() -> LayerSet {
    [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with)
}

/// The Milky Way fixture's full tables, shared.
fn tables() -> &'static PotentialTables {
    static TABLES: OnceLock<PotentialTables> = OnceLock::new();
    TABLES.get_or_init(|| PotentialTables::full(&MassModel::new(&GalaxyParams::milky_way_like())))
}

/// The fixture galaxy.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// The three test streams, numbered 0–2.
fn streams() -> Vec<StreamSpec> {
    let tables = tables();
    vec![
        pal5_like(tables),
        gd1_like(tables),
        sagittarius_like(tables),
    ]
}

/// A small dwarf core, number 0: 2,000 M☉ still bound of 6,000 at an accretion 3 Gyr ago, at
/// (30,000, 20,000, 5,000) ly on an orbit between 30,000 and 120,000 ly.
fn core() -> DwarfCoreSpec {
    let galaxy = galaxy();
    let orbit = SphericalOrbit::between(
        tables(),
        LightYears::new(30_000.0),
        LightYears::new(120_000.0),
    )
    .expect("turning points that bound an orbit");
    let mass = 2e3;
    DwarfCoreSpec {
        number: DwarfCoreNumber::new(0).expect("below the limit"),
        progenitor: 2,
        mass: SolarMasses::new(mass),
        systems: mass / galaxy.mean_system_mass(Population::Halo).value(),
        accreted: Years::new(3e9),
        accreted_mass: SolarMasses::new(3.0 * mass),
        position: GalacticDisplacement::new(
            [30_000.0, 20_000.0, 5_000.0].map(|x| x * METRES_PER_LIGHT_YEAR),
        ),
        velocity: GalacticVelocity::new([1.2e5, -8e4, 3e4]),
        orbit,
        stripping_fraction: 0.3,
        pericentres: 4,
    }
}

/// The tube of `spec` reduced from `per_arm` tracers an arm.
fn tube_of(spec: &StreamSpec, per_arm: u32) -> TubeTable {
    let tables = tables();
    let force = SprayForce::new(tables).expect("the full tables");
    let params = SprayParams::new(NonZeroU32::new(per_arm).expect("not zero"));
    let tracers = Spray::new(SEED, &force, tables, spec, &params)
        .expect("a finite stripping time")
        .tracers();
    let mean = galaxy().mean_system_mass(Population::Halo);
    TubeTable::from_tracers(SEED, mean, spec, &tracers).expect("enough tracers an arm")
}

/// A lookup of a list and its streams' tables, counting the tables it hands out; if `rebuild`, it
/// makes each table afresh on every call, as a cache that keeps none would.
struct Lookup {
    list: GlobalList,
    tubes: Vec<Arc<TubeTable>>,
    per_arm: u32,
    rebuild: bool,
    handed: AtomicUsize,
}

impl Lookup {
    fn new(list: GlobalList, per_arm: u32, rebuild: bool) -> Self {
        let tubes = list
            .debris()
            .streams()
            .iter()
            .map(|spec| Arc::new(tube_of(spec, per_arm)))
            .collect();
        Self {
            list,
            tubes,
            per_arm,
            rebuild,
            handed: AtomicUsize::new(0),
        }
    }

    fn handed(&self) -> usize {
        self.handed.load(Ordering::Relaxed)
    }
}

impl TubeLookup for Lookup {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        self.handed.fetch_add(1, Ordering::Relaxed);
        let index = usize::from(stream.get());
        if self.rebuild {
            let spec = &self.list.debris().streams()[index];
            Arc::new(tube_of(spec, self.per_arm))
        } else {
            Arc::clone(&self.tubes[index])
        }
    }
}

/// The fast fixture's lookup: the three test streams' 64-tracer tubes and the small core.
fn lookup() -> &'static Lookup {
    static LOOKUP: OnceLock<Lookup> = OnceLock::new();
    LOOKUP.get_or_init(|| {
        let list = GlobalList::of_parts(galaxy(), tables(), streams(), vec![core()]);
        Lookup::new(list, 64, false)
    })
}

/// The sphere of `radius` ly about `centre` at `years` from the epoch, padded as the range query
/// pads the sources' sphere.
fn sphere(centre: GalacticPosition, radius: f64, years: i64) -> QuerySphere {
    let t = UniverseTime::from_julian_years(years).expect("in the clock window");
    QuerySphere::new(
        centre,
        LightYears::new(radius),
        t,
        pad_for(t, UNBOUND_PAD_SPEED),
    )
    .expect("a positive radius")
}

/// The source's hits in `sphere`, sorted by ID.
fn hits(source: &GlobalListSource<'_>, galaxy: &Galaxy, sphere: &QuerySphere) -> Vec<SystemHit> {
    let mut out = Vec::new();
    source.systems_in_sphere(galaxy, sphere, stellar(), &mut out);
    out.sort_by_key(|hit| hit.id().raw());
    out
}

/// The hits' raw IDs, in order.
fn ids(hits: &[SystemHit]) -> Vec<u64> {
    hits.iter().map(|hit| hit.id().raw()).collect()
}

/// P10.T9: a query whose sphere meets no stream's shell builds no table, and one on a stream's
/// axis builds its stream's alone (to version 25 the source visited every stream, since a member
/// could lie outside its shell then). The list here is `pal5_like`'s and `gd1_like`'s, since
/// `sagittarius_like`'s shell holds the whole cube.
#[test]
fn a_query_that_touches_no_shell_builds_no_table() {
    let streams = streams()[..2].to_vec();
    let galaxy = galaxy();
    let list = GlobalList::of_parts(galaxy, tables(), streams.clone(), Vec::new());
    let lookup = Lookup::new(list, 64, false);
    let source =
        GlobalListSource::new(galaxy, &lookup, &NoCentreOrbitCache).expect("no centre to build");
    // 2,000 ly from the centre, inside both shells' holes.
    let inner = GalacticPosition::from_light_years([2_000.0, 0.0, 0.0]).expect("in range");
    for years in [0, 1_000] {
        let s = sphere(inner, 50.0, years);
        let census = source.expected_in_sphere(galaxy, &s);
        assert_eq!(census, LayerCounts::ZERO, "{years}");
        assert_eq!(hits(&source, galaxy, &s), Vec::new(), "{years}");
    }
    assert_eq!(lookup.handed(), 0, "tables handed out");
    // A query on pal5's axis, inside its shell and not gd1's: below gd1's inner radius.
    let gd1_inner = lookup.list.entries()[1].shell().inner().value();
    let on_axis = (1..100)
        .map(|k| lookup.tubes[0].locate(f64::from(k) / 100.0, 0.0, 0.0))
        .find(|p| {
            let [x, y, z] = p.to_light_years_f64();
            (x * x + y * y + z * z).sqrt() < gd1_inner - 1_000.0
        })
        .expect("pal5's tube comes within gd1's hole");
    let s = sphere(on_axis, 50.0, 0);
    assert!(!lookup.list.entries()[1].shell().touches(&s));
    let before = lookup.handed();
    let census = source.expected_in_sphere(galaxy, &s);
    assert!(census.get(Layer::A) > 0.0, "{census:?}");
    let handed = lookup.handed() - before;
    assert_eq!(handed, 1);
}

/// P10.T10.b's registration: a range query reads only the streams [`GlobalListSource::streams_read`]
/// names, so a server that builds those tables first leaves none for the query to build. Through a
/// lookup that records the streams it hands out, on `pal5_like`'s axis at the epoch and a
/// millennium on and inside every shell's hole a millennium back.
#[test]
fn a_query_reads_only_the_streams_it_is_said_to() {
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    /// A lookup that records the streams whose tables it hands out.
    struct Recording<'a> {
        inner: &'a Lookup,
        read: Mutex<BTreeSet<u16>>,
    }

    impl TubeLookup for Recording<'_> {
        fn global_list(&self) -> &GlobalList {
            self.inner.global_list()
        }

        fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
            self.read.lock().expect("not poisoned").insert(stream.get());
            self.inner.tube(stream)
        }
    }

    let lookup = lookup();
    let on_axis = lookup.tubes[0].locate(0.5, 0.0, 0.0);
    let inner = GalacticPosition::from_light_years([2_000.0, 0.0, 0.0]).expect("in range");
    let galaxy = galaxy();
    for (centre, years) in [(on_axis, 0), (on_axis, 1_000), (inner, -1_000)] {
        let recording = Recording {
            inner: lookup,
            read: Mutex::default(),
        };
        let source = GlobalListSource::new(galaxy, &recording, &NoCentreOrbitCache)
            .expect("no centre to build");
        let query = RangeQuery::builder(centre, LightYears::new(50.0))
            .time(UniverseTime::from_julian_years(years).expect("in the clock window"))
            .build()
            .expect("a query in the cube");
        let _ = range_query(galaxy, &mut NoCache::new(), &[&source], &query);
        let said: BTreeSet<u16> = GlobalListSource::streams_read(galaxy, &lookup.list, &query)
            .iter()
            .map(|number| number.get())
            .collect();
        let read = recording.read.into_inner().expect("not poisoned");
        assert!(
            read.is_subset(&said),
            "{years}: read {read:?}, said {said:?}"
        );
    }
}

/// The streams whose tables a lookup handed out since it was last asked, and the list's models it
/// lends, each stream's envelope kept.
struct Enveloped<'a> {
    lookup: &'a Lookup,
    models: GlobalListModels,
    read: std::sync::Mutex<std::collections::BTreeSet<u16>>,
}

impl Enveloped<'_> {
    fn taken(&self) -> std::collections::BTreeSet<u16> {
        std::mem::take(&mut *self.read.lock().expect("not poisoned"))
    }
}

impl TubeLookup for Enveloped<'_> {
    fn global_list(&self) -> &GlobalList {
        &self.lookup.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        self.read.lock().expect("not poisoned").insert(stream.get());
        self.lookup.tube(stream)
    }

    fn models(&self) -> Option<&GlobalListModels> {
        Some(&self.models)
    }
}

/// Spheres on the three test streams' tubes and off them, from 1 ly to 10,000 ly, and anywhere in
/// the halo, each about a centre a query may have, in the root cube, which `sagittarius_like`'s
/// tube passes.
fn envelope_spheres(tubes: &[Arc<TubeTable>]) -> Vec<(GalacticPosition, f64)> {
    let mut draws = Stream::open(
        Seed::new(0x0a10_9a01),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy(),
    );
    let mut spheres = Vec::new();
    for k in 0..64 {
        let radius = crate::math::powf(10.0, 4.0 * draws.uniform());
        let centre = if k % 4 == 3 {
            let p = [0; 3].map(|_| 120_000.0 * draws.uniform() - 60_000.0);
            GalacticPosition::from_light_years(p).expect("in the cube")
        } else {
            let (u, a, b) = (
                draws.uniform(),
                16.0 * draws.uniform() - 8.0,
                16.0 * draws.uniform() - 8.0,
            );
            tubes[k % 4].locate(u, a, b)
        };
        if RangeQuery::builder(centre, LightYears::new(radius))
            .build()
            .is_ok()
        {
            spheres.push((centre, radius));
        }
    }
    assert!(spheres.len() > 40, "{} spheres in the cube", spheres.len());
    spheres
}

/// The census of `radius` ly about `centre` by hand, the old path: the streams of `lookup`'s list,
/// a list of streams alone, whose shells the sphere meets at the epoch, each one's cells' means
/// summed band by band from zero and added in list order; and how many streams' tubes it touches.
fn census_by_hand(
    galaxy: &Galaxy,
    lookup: &Lookup,
    (centre, radius): (GalacticPosition, f64),
) -> (LayerCounts, usize) {
    let (mut by_hand, mut touched) = (LayerCounts::ZERO, 0);
    let at_epoch = sphere(centre, radius, 0);
    for (n, tube) in lookup.tubes.iter().enumerate() {
        if !lookup.list.entries()[n].shell().touches(&at_epoch) {
            continue;
        }
        let shares = StreamBandShares::of(galaxy, &lookup.list.debris().streams()[n]);
        let cells: Vec<_> = tube
            .cells_touching(&centre, LightYears::new(radius))
            .collect();
        for band in BANDS_COARSEST_FIRST {
            let sum = cells
                .iter()
                .fold(0.0, |s, &c| s + expected_members(tube, &shares, c, band));
            by_hand.set(band.layer(), by_hand.get(band.layer()) + sum);
        }
        touched += usize::from(!cells.is_empty());
    }
    (by_hand, touched)
}

/// The sky's members of each band within `radius` ly of `centre` over the window from the epoch to
/// `t`, in one part, each band's as a source `source` makes gives them.
fn sky_members<'a>(
    source: impl Fn() -> GlobalListSource<'a>,
    galaxy: &Galaxy,
    (centre, radius): (GalacticPosition, f64),
    t: UniverseTime,
) -> Vec<Vec<GlobalMemberRecord>> {
    BANDS_COARSEST_FIRST
        .into_iter()
        .map(|band| {
            let mut seen = Vec::new();
            let reach = (band, LightYears::new(radius));
            source().members_in_shell_over(
                galaxy,
                &centre,
                (UniverseTime::EPOCH, t),
                (reach, (0, NonZeroU32::MIN)),
                &mut |m| seen.push(*m),
            );
            seen
        })
        .collect()
}

/// The raw numbers of `streams`.
fn numbers(streams: &[StreamNumber]) -> std::collections::BTreeSet<u16> {
    streams.iter().map(|number| number.get()).collect()
}

/// Keeps every envelope of `tubes` in `models`: every other one from its parts, as a server reads
/// them from disk after a restart, each then its table's own (perf-range-envelopes-on-disk).
fn keep_every_envelope(models: &GlobalListModels, tubes: &[Arc<TubeTable>]) {
    for (k, tube) in tubes.iter().enumerate() {
        let own = TubeEnvelope::of(tube);
        if k % 2 == 0 {
            let read = TubeEnvelope::from_parts(*own.boxes_ly(), own.member_speed_bound())
                .expect("its own parts");
            models.keep_stored_envelope(tube.stream(), read);
        } else {
            models.keep_envelope(tube);
        }
        let kept = models.envelope(tube.stream()).expect("kept");
        assert_eq!(kept, &own, "stream {}", tube.stream().get());
    }
}

/// A stored envelope is kept only where none is kept already, and one of a number the list lacks
/// is passed by (perf-range-envelopes-on-disk).
#[test]
fn a_stored_envelope_replaces_none_and_one_past_the_list_is_passed_by() {
    let galaxy = galaxy();
    let tubes = &lookup().tubes;
    let list = GlobalList::of_parts(galaxy, tables(), streams(), Vec::new());
    let models = GlobalListModels::new(galaxy, &list);
    let (first, second) = (&tubes[0], &tubes[1]);
    models.keep_envelope(first);
    let other = TubeEnvelope::of(second);
    let other = TubeEnvelope::from_parts(*other.boxes_ly(), other.member_speed_bound())
        .expect("its own parts");
    assert_ne!(other, TubeEnvelope::of(first));
    models.keep_stored_envelope(first.stream(), other.clone());
    assert_eq!(
        models.envelope(first.stream()),
        Some(&TubeEnvelope::of(first))
    );
    let past = StreamNumber::new(u16::try_from(list.debris().streams().len()).expect("three"))
        .expect("below the limit");
    models.keep_stored_envelope(past, other);
    assert_eq!(models.envelope(past), None);
    assert_eq!(
        models.envelope(second.stream()),
        None,
        "none kept for the second"
    );
}

/// A source whose list's models keep every stream's envelope counts and finds what a source that
/// keeps none does, bit for bit, and its census is the cells' sums by hand, list order kept
/// (perf-range-census-streams); it reads only the tables of streams whose envelopes its spheres
/// meet, which [`GlobalListSource::streams_read_with`] names, and passes some by. Over
/// [`envelope_spheres`], at the epoch and a millennium on; below 300 ly, its hits and the sky's
/// members over each window too, the sky reading only the tables
/// [`GlobalListSource::streams_seen_with`] names (perf-range-cold-tables). Every other envelope
/// is kept from its parts, as a server reads them from disk after a restart, and is its table's
/// own (perf-range-envelopes-on-disk).
#[test]
fn kept_envelopes_pass_tables_by_and_move_no_count_or_hit() {
    let galaxy = galaxy();
    let kept = lookup();
    let streams = Lookup {
        list: GlobalList::of_parts(galaxy, tables(), streams(), Vec::new()),
        tubes: kept.tubes.clone(),
        per_arm: kept.per_arm,
        rebuild: false,
        handed: AtomicUsize::new(0),
    };
    let models = GlobalListModels::new(galaxy, &streams.list);
    keep_every_envelope(&models, &streams.tubes);
    let enveloped = Enveloped {
        lookup: &streams,
        models,
        read: std::sync::Mutex::default(),
    };
    let with = GlobalListSource::new(galaxy, &enveloped, &NoCentreOrbitCache).expect("no centre");
    let bits = |counts: LayerCounts| counts.to_array().map(hyperion_testkit::float::bits);
    // Made afresh for each call, so that it keeps no envelope and reads every table in reach.
    let without =
        || GlobalListSource::new(galaxy, &streams, &NoCentreOrbitCache).expect("no centre");
    let (mut passed, mut touched, mut sky_passed) = (0, 0, 0);
    let (mut hits_read, mut hits_read_without) = (0, 0);
    for (centre, radius) in envelope_spheres(&streams.tubes) {
        for years in [0, 1_000] {
            let s = sphere(centre, radius, years);
            let census = with.expected_in_sphere(galaxy, &s);
            assert_eq!(bits(census), bits(without().expected_in_sphere(galaxy, &s)));
            let (by_hand, touching) = census_by_hand(galaxy, &streams, (centre, radius));
            assert_eq!(bits(census), bits(by_hand), "{radius} ly, {years}");
            touched += touching;
            // `sagittarius_like`'s tube holds some 10⁸ members, so the hits and the sky's members
            // only in small spheres.
            let mut read = enveloped.taken();
            if radius <= 300.0 {
                let found = hits(&with, galaxy, &s);
                let mut hit_tables = enveloped.taken();
                hits_read += hit_tables.len();
                let handed = streams.handed();
                assert_eq!(found, hits(&without(), galaxy, &s), "{radius} ly, {years}");
                hits_read_without += streams.handed() - handed;
                read.append(&mut hit_tables);
                let sphere = (centre, radius);
                let seen = sky_members(|| with.clone(), galaxy, sphere, s.time());
                let sky_read = enveloped.taken();
                let unseen = sky_members(without, galaxy, sphere, s.time());
                assert_eq!(seen, unseen, "{radius} ly, {years}");
                // The sky's reads, padded by its own rule, are those `streams_seen_with` names
                // among the streams whose shells it meets (perf-range-cold-tables).
                let (window, outer) = ((UniverseTime::EPOCH, s.time()), LightYears::new(radius));
                let list = &streams.list;
                let named = numbers(&GlobalListSource::streams_seen_with(
                    galaxy,
                    list,
                    &enveloped.models,
                    &centre,
                    window,
                    outer,
                ));
                let shelled = numbers(&GlobalListSource::streams_seen(
                    galaxy, list, &centre, window, outer,
                ));
                assert!(
                    sky_read.is_subset(&named),
                    "the sky read {sky_read:?}, named {named:?}"
                );
                assert!(
                    named.is_subset(&shelled),
                    "named {named:?}, shelled {shelled:?}"
                );
                sky_passed += shelled.len() - named.len();
            }
            let query = RangeQuery::builder(centre, LightYears::new(radius))
                .time(UniverseTime::from_julian_years(years).expect("in the clock window"))
                .build()
                .expect("a query in the cube");
            let said = numbers(&GlobalListSource::streams_read_with(
                galaxy,
                &streams.list,
                &enveloped.models,
                &query,
            ));
            let shelled = numbers(&GlobalListSource::streams_read(
                galaxy,
                &streams.list,
                &query,
            ));
            assert!(read.is_subset(&said), "read {read:?}, said {said:?}");
            assert!(
                said.is_subset(&shelled),
                "said {said:?}, shelled {shelled:?}"
            );
            passed += shelled.len() - said.len();
        }
    }
    assert!(
        passed > 20 && touched > 20 && sky_passed > 0,
        "{passed} tables passed by, {touched} touched, {sky_passed} passed by the sky"
    );
    assert!(
        hits_read < hits_read_without,
        "the hits read {hits_read} tables, {hits_read_without} without the envelopes"
    );
}

/// P10.T9: the census is the exact sum of the touched cells' means band by band, the same through
/// a lookup that keeps its tables and through one that rebuilds them on every call, and at any
/// time the query is asked at (the census is the epoch's); it is the cells' sum by hand.
#[test]
fn the_census_is_the_cells_sum_and_independent_of_the_cache() {
    let galaxy = galaxy();
    let kept = lookup();
    let rebuilt = Lookup::new(kept.list.clone(), 64, true);
    let a = GlobalListSource::new(galaxy, kept, &NoCentreOrbitCache).expect("no centre");
    let b = GlobalListSource::new(galaxy, &rebuilt, &NoCentreOrbitCache).expect("no centre");
    let gd1 = &kept.tubes[1];
    let centre = gd1.locate(0.3, 1.0, -0.5);
    let census = a.expected_in_sphere(galaxy, &sphere(centre, 200.0, 0));
    for years in [0, -500, 3_000] {
        let s = sphere(centre, 200.0, years);
        assert_eq!(a.expected_in_sphere(galaxy, &s), census, "{years}");
        assert_eq!(b.expected_in_sphere(galaxy, &s), census, "{years}");
    }
    assert!(rebuilt.handed() > 0);
    // By hand: the streams whose shells the sphere meets; the core's cells lie far away.
    let shares = |n: usize| StreamBandShares::of(galaxy, &kept.list.debris().streams()[n]);
    let mut by_hand = LayerCounts::ZERO;
    for n in [0, 1, 2] {
        let entry = kept.list.entries()[n];
        if !entry.shell().touches(&sphere(centre, 200.0, 0)) {
            continue;
        }
        let tube = &kept.tubes[n];
        let shares = shares(n);
        let cells: Vec<_> = tube
            .cells_touching(&centre, LightYears::new(200.0))
            .collect();
        for band in BANDS_COARSEST_FIRST {
            let sum = cells
                .iter()
                .fold(0.0, |s, &c| s + expected_members(tube, &shares, c, band));
            by_hand.set(band.layer(), by_hand.get(band.layer()) + sum);
        }
    }
    assert_eq!(census, by_hand);
    assert!(census.get(Layer::A) > 1.0, "{census:?}");
}

/// A lookup that lends one set of the list's models to every source made over it, as a server
/// keeps them for every query of a galaxy (P09.T37's bench).
struct Lending<'a> {
    lookup: &'a Lookup,
    models: GlobalListModels,
}

impl TubeLookup for Lending<'_> {
    fn global_list(&self) -> &GlobalList {
        &self.lookup.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        self.lookup.tube(stream)
    }

    fn models(&self) -> Option<&GlobalListModels> {
        Some(&self.models)
    }
}

/// P10.T9: the hits and the census of each sphere are the same whatever was asked before them,
/// through one source whose band shares fill as the queries reach their streams, and through
/// sources made for each query over models lent and filled across them (P09.T37's bench), and
/// equal a fresh source's (the sim-determinism skill's order check).
#[test]
fn the_source_s_answers_are_order_independent() {
    use hyperion_testkit::order::assert_order_independent;
    let galaxy = galaxy();
    let lookup = lookup();
    let core = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache)
        .expect("no centre")
        .core(DwarfCoreNumber::new(0).expect("0"))
        .expect("played")
        .clone();
    let mut members = Vec::new();
    let in_core = core
        .grid()
        .owned_cells()
        .find_map(|cell| {
            core.members_in_cell(galaxy, MassBand::A, cell, &mut members);
            members.first().map(|(m, _)| *m.record().epoch_position())
        })
        .expect("a core with members");
    // About a member of each stream near its axis, from `u` on, so that each sphere holds one
    // whatever the tube's centre line; a stream's members outside the root cube are dropped.
    let in_stream = |n: usize, u: f64| {
        let tube = &lookup.tubes[n];
        let shares = StreamBandShares::of(galaxy, &lookup.list.debris().streams()[n]);
        let mut members = Vec::new();
        (0..20)
            .map(|k| (u + 0.05 * f64::from(k)).fract())
            .find_map(|at| {
                tube.cells_touching(&tube.locate(at, 0.0, 0.0), LightYears::new(100.0))
                    .find_map(|cell| {
                        stream_cell_members(galaxy, tube, &shares, cell, MassBand::A, &mut members);
                        members.first().map(|m| *m.record().epoch_position())
                    })
            })
            .expect("a member near the axis")
    };
    let inner = GalacticPosition::from_light_years([2_000.0, 0.0, 0.0]).expect("in range");
    let spheres = [
        sphere(in_stream(0, 0.45), 300.0, 200),
        sphere(in_stream(1, 0.3), 200.0, 0),
        sphere(in_stream(2, 0.6), 50.0, -1_000),
        sphere(in_core, 30.0, 1_000),
        sphere(inner, 50.0, 0),
    ];
    let answer = |source: &GlobalListSource<'_>, s: &QuerySphere| {
        (
            hits(source, galaxy, s),
            source.expected_in_sphere(galaxy, s),
        )
    };
    let warm = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    assert_order_independent(&spheres, |s| answer(&warm, s));
    let fresh = |s: &QuerySphere| {
        let cold = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
        answer(&cold, s)
    };
    assert_order_independent(&spheres, fresh);
    let lending = Lending {
        lookup,
        models: GlobalListModels::new(galaxy, &lookup.list),
    };
    let lent = |s: &QuerySphere| {
        let source =
            GlobalListSource::new(galaxy, &lending, &NoCentreOrbitCache).expect("no centre");
        answer(&source, s)
    };
    assert_order_independent(&spheres, lent);
    let filled = lending
        .models
        .shares
        .iter()
        .filter(|shares| shares.get().is_some())
        .count();
    assert_eq!(
        filled, 3,
        "each stream's shares made once, by the first query to reach it"
    );
    for (k, s) in spheres.iter().enumerate() {
        assert_eq!(answer(&warm, s), fresh(s), "sphere {k}");
        assert_eq!(lent(s), fresh(s), "sphere {k}");
    }
    let counts: Vec<usize> = spheres
        .iter()
        .map(|s| hits(&warm, galaxy, s).len())
        .collect();
    assert!(counts[..4].iter().all(|&n| n > 0), "{counts:?}");
}

/// P10.T9: the hits of a query about the small core's members are exactly the members a scan of
/// every owned cell finds in the sphere at its time, at the epoch and a millennium either way;
/// every core member lies inside its entry's shell; and the census counts the cells it touches.
#[test]
fn a_core_s_hits_are_every_member_inside_the_sphere() {
    let galaxy = galaxy();
    let lookup = lookup();
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    let model = source
        .core(DwarfCoreNumber::new(0).expect("0"))
        .expect("played");
    let shell = lookup.list.entries()[3].shell();
    let mut all = Vec::new();
    let mut cell_members = Vec::new();
    for band in BANDS_COARSEST_FIRST {
        for cell in model.grid().owned_cells() {
            model.members_in_cell(galaxy, band, cell, &mut cell_members);
            all.extend(cell_members.iter().map(|(m, _)| *m));
        }
    }
    assert!(all.len() > 1_000, "{} members", all.len());
    for member in &all {
        let at = GalacticPosition::ORIGIN.displacement_to(member.record().epoch_position());
        assert!(shell.contains(&at), "{shell:?} misses {at:?}");
    }
    let near = all[all.len() / 3].record().epoch_position();
    for years in [0, 1_000, -1_000] {
        let s = sphere(*near, 30.0, years);
        let elapsed = Seconds::new(s.time().since_epoch().as_seconds_f64());
        let mut want: Vec<SystemHit> = all
            .iter()
            .filter_map(|m| hit_at(m.record(), m.velocity(), &s, elapsed))
            .collect();
        want.sort_by_key(|hit| hit.id().raw());
        let got = hits(&source, galaxy, &s);
        // The streams' hits, if any, sort apart: their IDs are of another sub-kind.
        let cores: Vec<SystemHit> = got
            .into_iter()
            .filter(|hit| matches!(hit.id().kind(), crate::id::SystemIdKind::DwarfCore(_)))
            .collect();
        assert_eq!(cores, want, "{years}");
        assert_ne!(want.len(), 0, "{years}");
    }
    let census = source.expected_in_sphere(galaxy, &sphere(*near, 30.0, 0));
    assert!(census.get(Layer::A) > 0.0, "{census:?}");
}

/// P10.T9: every hit is a member of the global list in a requested layer, of the halo's
/// population and no density component (plan 10, "Members as `SystemRecord`s"), inside the
/// sphere at its time; a query asking for layer A alone gets layer A's hits alone.
#[test]
fn hits_are_global_list_members_of_the_layers_asked() {
    let galaxy = galaxy();
    let lookup = lookup();
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    let centre = lookup.tubes[0].locate(0.45, 0.0, 0.0);
    let s = sphere(centre, 300.0, 200);
    let all = hits(&source, galaxy, &s);
    assert!(all.len() > 10, "{} hits", all.len());
    let mut layers = std::collections::BTreeSet::new();
    for hit in &all {
        let record = hit.record();
        assert!(matches!(
            record.origin(),
            SystemOrigin::GlobalListMember { .. }
        ));
        assert_eq!(record.population(), Population::Halo);
        assert_eq!(record.component(), None);
        assert!(hit.distance().value() <= 300.0);
        layers.insert(record.layer());
    }
    assert!(layers.len() >= 2, "{layers:?}");
    let mut only_a = Vec::new();
    source.systems_in_sphere(galaxy, &s, LayerSet::EMPTY.with(Layer::A), &mut only_a);
    only_a.sort_by_key(|hit| hit.id().raw());
    let want: Vec<SystemHit> = all
        .into_iter()
        .filter(|hit| hit.record().layer() == Layer::A)
        .collect();
    assert_eq!(only_a, want);
}

/// P10.T9: merged with the grid by `range_query`, the source's hits keep their IDs apart from the
/// grid's, and the census counts its members.
#[test]
fn the_range_query_merges_the_list_s_members() {
    let galaxy = galaxy();
    let lookup = lookup();
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    // About a member of the small core, which lies dense.
    let model = source
        .core(DwarfCoreNumber::new(0).expect("0"))
        .expect("played");
    let mut members = Vec::new();
    let centre = model
        .grid()
        .owned_cells()
        .find_map(|cell| {
            model.members_in_cell(galaxy, MassBand::A, cell, &mut members);
            members.first().map(|(m, _)| *m.record().epoch_position())
        })
        .expect("a core with members");
    let query = RangeQuery::builder(centre, LightYears::new(20.0))
        .build()
        .expect("a valid query");
    let alone = range_query(galaxy, &mut NoCache::new(), &[], &query);
    let merged = range_query(galaxy, &mut NoCache::new(), &[&source], &query);
    let is_member =
        |hit: &&SystemHit| matches!(hit.record().origin(), SystemOrigin::GlobalListMember { .. });
    assert!(merged.systems().iter().any(|hit| is_member(&hit)));
    // The grid's hits are the query's alone, but for the layers the members' counts stopped.
    let grid: Vec<u64> = alone.systems().iter().map(|hit| hit.id().raw()).collect();
    for hit in merged.systems().iter().filter(|hit| !is_member(hit)) {
        assert!(grid.contains(&hit.id().raw()), "{hit:?}");
    }
    let a = merged.census().expected().get(Layer::A);
    assert!(a > alone.census().expected().get(Layer::A), "{a}");
}

/// Every member of every cell of `tube`, whose stream's band shares are `shares`, in every band.
fn every_member(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
) -> Vec<GlobalMemberRecord> {
    use crate::galaxy::global_list::tube::ACROSS_CELLS;
    use crate::id::StreamMemberId;
    let mut all = Vec::new();
    let mut members = Vec::new();
    for along in 0..tube.along_cells() {
        let along = u16::try_from(along).expect("at most 16,384 along cells");
        for a in 0..ACROSS_CELLS {
            for b in 0..ACROSS_CELLS {
                let cell = StreamMemberId::new(tube.stream().get(), Layer::A, along, [a, b], 0)
                    .expect("a tube's cell")
                    .into();
                for band in BANDS_COARSEST_FIRST {
                    stream_cell_members(galaxy, tube, shares, cell, band, &mut members);
                    all.extend_from_slice(&members);
                }
            }
        }
    }
    all
}

/// The hits a brute-force scan of `all` makes in `sphere`, those inside `shell`, sorted by ID.
fn brute_force(
    all: &[GlobalMemberRecord],
    shell: &BoundingShell,
    sphere: &QuerySphere,
) -> Vec<SystemHit> {
    let elapsed = Seconds::new(sphere.time().since_epoch().as_seconds_f64());
    let mut want: Vec<SystemHit> = all
        .iter()
        .filter(|m| in_stream_shell(shell, m.record().epoch_position()))
        .filter_map(|m| hit_at(m.record(), m.velocity(), sphere, elapsed))
        .collect();
    want.sort_by_key(|hit| hit.id().raw());
    want
}

/// The slow fixture's lookup: `pal5_like`'s and `gd1_like`'s tubes at the generator's 2,000
/// tracers, on a list of the two.
fn generator_lookup() -> &'static Lookup {
    static LOOKUP: OnceLock<Lookup> = OnceLock::new();
    LOOKUP.get_or_init(|| {
        let list = GlobalList::of_parts(galaxy(), tables(), streams()[..2].to_vec(), Vec::new());
        Lookup::new(list, 1_000, false)
    })
}

/// P10.T9: a query on the axis of `gd1_like`, and on `pal5_like`'s, at the generator's 2,000
/// tracers, equals a brute-force enumeration of every member of its tube, at the epoch and a
/// millennium either way, the members inside the shell; every member of `gd1_like` lies inside
/// its shell, and of `pal5_like`'s some lie outside, which the source drops (printed); and no member moves faster than its tube's speed bound.
#[test]
#[ignore = "slow: sprays two test streams of 2,000 tracers and enumerates every member of their tubes"]
fn a_query_on_a_test_stream_s_axis_equals_every_member_of_its_tube() {
    let lookup = generator_lookup();
    for (n, name) in [(1, "gd1"), (0, "pal5")] {
        let tube = &lookup.tubes[n];
        let shell = lookup.list.entries()[n].shell();
        let galaxy = galaxy();
        let shares = StreamBandShares::of(galaxy, &lookup.list.debris().streams()[n]);
        let all = every_member(galaxy, tube, &shares);
        let outside = all
            .iter()
            .filter(|m| {
                !shell.contains(
                    &GalacticPosition::ORIGIN.displacement_to(m.record().epoch_position()),
                )
            })
            .count();
        let bound = tube.member_speed_bound().value();
        let fastest = all
            .iter()
            .map(|m| crate::units::KilometresPerSecond::from(m.velocity().speed()).value())
            .fold(0.0, f64::max);
        eprintln!(
            "{name}: {} members, {outside} outside the shell; fastest {fastest:.1} km/s against \
             the bound's {bound:.1}",
            all.len()
        );
        assert!(fastest <= bound, "{name}: {fastest} against {bound}");
        if name == "gd1" {
            assert_eq!(outside, 0, "{name}");
        }
        let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
        for (u, radius, occupied) in [(0.2, 60.0, false), (0.5, 300.0, true), (0.9, 150.0, false)] {
            for years in [0, 1_000, -1_000] {
                let s = sphere(tube.locate(u, 0.0, 0.0), radius, years);
                let got = hits(&source, galaxy, &s);
                let mine: Vec<SystemHit> = got
                    .into_iter()
                    .filter(|hit| {
                        matches!(
                            hit.id().kind(),
                            crate::id::SystemIdKind::Stream(id) if usize::from(id.number()) == n
                        )
                    })
                    .collect();
                let want = brute_force(&all, &shell, &s);
                assert_eq!(ids(&mine), ids(&want), "{name} {u} {years}");
                assert_eq!(mine, want, "{name} {u} {years}");
                if occupied {
                    assert_ne!(want.len(), 0, "{name}: no member on the axis");
                }
            }
        }
    }
}

/// The most of `pal5_like`'s members that lie outside its shell under any of the eight seeds of
/// [`over_eight_seeds_the_test_tubes_lie_within_their_tracers_and_shells`], as measured
/// (2026-11-05, x86-64, at generator version 27) with its tips fitted in the orbital plane
/// (P10.T6.a): 5, at the third seed, of 6 over the eight. When the fit was new (2026-10-31, at 24)
/// it was 2 of 3, against 147 at the first seed when the tips were fitted in x, y and z.
///
/// No 27 hold moved it. On the tree before the folds, every 27 hold and 26's rules alike gave
/// 67,580 members and 5 outside at the third seed, and so did each hold turned back to its 26 arm
/// alone. So it moved between 24 and 26, which this slow test was not run on.
const PAL5_OUTSIDE_PINNED: usize = 5;

/// P10.T6.a's tips, fitted in the orbital plane: over eight seeds of the generator's 2,000
/// tracers, every knot of `pal5_like` and `gd1_like` lies within its arm's tracers' radial range,
/// and of their members outside the list's shell `gd1_like` has none and `pal5_like` at most
/// [`PAL5_OUTSIDE_PINNED`] a seed (each seed printed, with the knots that kept the Cartesian fit,
/// none as measured).
#[test]
#[ignore = "slow: sprays two test streams under eight seeds and enumerates their tubes"]
fn over_eight_seeds_the_test_tubes_lie_within_their_tracers_and_shells() {
    use crate::galaxy::global_list::spray::Arm;
    let galaxy = galaxy();
    let list = GlobalList::of_parts(galaxy, tables(), streams()[..2].to_vec(), Vec::new());
    let force = SprayForce::new(tables()).expect("the full tables");
    let params = SprayParams::new(NonZeroU32::new(1_000).expect("not zero"));
    let mean = galaxy.mean_system_mass(Population::Halo);
    let radius = |x: GalacticDisplacement| {
        let [a, b, c] = x.metres();
        (a * a + b * b + c * c).sqrt() / METRES_PER_LIGHT_YEAR
    };
    for (n, name) in [(0, "pal5"), (1, "gd1")] {
        let spec = &list.debris().streams()[n];
        let shell = list.entries()[n].shell();
        let shares = StreamBandShares::of(galaxy, spec);
        for k in 0..8 {
            let seed = Seed::new(SEED.get() + k);
            let tracers = Spray::new(seed, &force, tables(), spec, &params)
                .expect("a finite stripping time")
                .tracers();
            let before = crate::galaxy::global_list::tube::cartesian_knots();
            let tube = TubeTable::from_tracers(seed, mean, spec, &tracers).expect("enough");
            let fallbacks = crate::galaxy::global_list::tube::cartesian_knots() - before;
            let range = |arm: Arm| {
                tracers
                    .iter()
                    .filter(|t| t.energy_arm() == arm)
                    .map(|t| radius(t.position()))
                    .fold((f64::INFINITY, 0.0_f64), |(a, b), r| (a.min(r), b.max(r)))
            };
            for arm in [Arm::Trailing, Arm::Leading] {
                let (low, high) = range(arm);
                for (j, knot) in tube.arm(arm).knots().iter().enumerate() {
                    let r = radius(GalacticPosition::ORIGIN.displacement_to(&knot.centre()));
                    assert!(
                        (low..=high).contains(&r),
                        "{name} +{k} {arm:?} knot {j}: {r:.0} ly, its arm's tracers \
                         {low:.0}–{high:.0}"
                    );
                }
            }
            let all = every_member(galaxy, &tube, &shares);
            let outside = all
                .iter()
                .filter(|m| {
                    !shell.contains(
                        &GalacticPosition::ORIGIN.displacement_to(m.record().epoch_position()),
                    )
                })
                .count();
            eprintln!(
                "{name} +{k}: {} members, {outside} outside the shell; {fallbacks} knots kept the \
                 Cartesian fit",
                all.len()
            );
            let most = if n == 0 { PAL5_OUTSIDE_PINNED } else { 0 };
            assert!(
                outside <= most,
                "{name} +{k}: {outside} outside, against {most}"
            );
        }
    }
}

/// P10.T9, the bench's figure: a 50 ly query on `pal5_like`'s axis generates every member of the
/// cells it touches and returns those in the sphere (measured 2026-10-31, the tips fitted in the
/// orbital plane: 135 generated and 22 returned over the 19 queries, about 6 a hit, where the plan
/// estimated about 160 to find two).
#[test]
#[ignore = "slow: sprays a test stream of 2,000 tracers"]
fn a_query_on_a_globular_stream_s_axis_generates_its_cells_members() {
    let lookup = generator_lookup();
    let galaxy = galaxy();
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    let tube = &lookup.tubes[0];
    let shares = StreamBandShares::of(galaxy, &lookup.list.debris().streams()[0]);
    let (mut generated, mut returned) = (0, 0);
    for k in 1..20 {
        let centre = tube.locate(f64::from(k) / 20.0, 0.0, 0.0);
        let s = sphere(centre, 50.0, 0);
        let got = hits(&source, galaxy, &s);
        returned += got.len();
        let mut members = Vec::new();
        for cell in tube.cells_touching(&centre, LightYears::new(50.0)) {
            for band in BANDS_COARSEST_FIRST {
                stream_cell_members(galaxy, tube, &shares, cell, band, &mut members);
                generated += members.len();
            }
        }
    }
    eprintln!(
        "pal5: {generated} members generated, {returned} returned over 19 queries of 50 ly: {:.0} \
         a hit",
        f64::from(u32::try_from(generated).expect("small"))
            / f64::from(u32::try_from(returned.max(1)).expect("small"))
    );
    assert!(returned > 0 && generated > returned);
}

/// A lookup of a list and one table, which it hands out for every stream.
struct One(GlobalList, Arc<TubeTable>);

impl TubeLookup for One {
    fn global_list(&self) -> &GlobalList {
        &self.0
    }

    fn tube(&self, _: StreamNumber) -> Arc<TubeTable> {
        Arc::clone(&self.1)
    }
}

/// P10.T9: `resolve_with` refuses a drawn stream's member outside its entry's shell, as the source
/// leaves it out, and resolves it with the list's own shell (to version 25 it resolved both). The
/// shell is the empty one here, so that every member is outside.
#[test]
#[ignore = "slow: builds a Milky Way galaxy's kinematic tables, its list and a tube"]
fn resolve_with_refuses_a_stream_member_outside_its_shell_from_26() {
    use crate::galaxy::placement::resolve_with;
    let galaxy = Galaxy::from_params(Seed::new(0x0a10_8b01), GalaxyParams::milky_way_like())
        .expect("builds")
        .with_full_potential();
    let list = GlobalList::build(&galaxy);
    let spec = &list.debris().streams()[0];
    let tube = Arc::new(TubeTable::build(&galaxy, spec).expect("the full potential"));
    let shares = StreamBandShares::of(&galaxy, spec);
    let shell = list.stream_entry(spec.number()).expect("listed").shell();
    let centre = tube.locate(0.5, 0.0, 0.0);
    let mut members = Vec::new();
    let mut found = None;
    for cell in tube.cells_touching(&centre, LightYears::new(100.0)) {
        stream_cell_members(&galaxy, &tube, &shares, cell, MassBand::A, &mut members);
        if let Some(m) = members.iter().find(|m| {
            shell.contains(&GalacticPosition::ORIGIN.displacement_to(m.record().epoch_position()))
        }) {
            found = Some(*m);
            break;
        }
    }
    let member = found.expect("a member inside the shell on the axis");
    let id = member.record().id();
    let own = One(list.clone(), Arc::clone(&tube));
    assert_eq!(resolve_with(&galaxy, id, &own), Ok(*member.record()));
    assert_eq!(
        crate::galaxy::placement::resolve_list_member_with(&galaxy, id, &own),
        Ok(member)
    );
    let mut emptied = list.clone();
    let index = emptied
        .entries
        .iter()
        .position(|e| e.kind() == GlobalEntryKind::Stream(spec.number()))
        .expect("listed");
    emptied.entries[index] = GlobalEntry::new(
        GlobalEntryKind::Stream(spec.number()),
        spec.mass(),
        BoundingShell::new(LightYears::new(1.0), LightYears::ZERO),
    );
    let empty = One(emptied, tube);
    assert_eq!(
        resolve_with(&galaxy, id, &empty),
        Err(crate::galaxy::placement::ResolveSystemError::NoSuchSystem)
    );
}

/// A lookup that builds each of a drawn galaxy's tables on first use and keeps it.
struct Kept<'g> {
    galaxy: &'g Galaxy,
    list: GlobalList,
    tubes: std::sync::Mutex<std::collections::BTreeMap<StreamNumber, Arc<TubeTable>>>,
}

impl TubeLookup for Kept<'_> {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        let spec = self.list.stream(stream).expect("a stream of the list");
        let mut tubes = self.tubes.lock().expect("not poisoned");
        Arc::clone(tubes.entry(stream).or_insert_with(|| {
            Arc::new(TubeTable::build(self.galaxy, spec).expect("the full potential"))
        }))
    }
}

/// P10.T9, from version 26: in a drawn galaxy's list, cut to its centre and its first stream so
/// that one tube is built, the source's members of the centre are the centre's own source's, and
/// its census holds the centre's; and each stream member it returns about a member inside the
/// stream's shell resolves through `resolve_with` to the record it returned.
#[test]
#[ignore = "slow: builds a Milky Way galaxy's kinematic tables, its list, its centre and a tube"]
fn a_drawn_galaxy_s_source_holds_the_centre_s_and_resolves_its_members() {
    use crate::galaxy::features::centre::CentreMemberSource;
    use crate::galaxy::placement::resolve_with;
    use crate::id::SystemIdKind;
    let galaxy = Galaxy::from_params(Seed::new(0x0a10_8b01), GalaxyParams::milky_way_like())
        .expect("builds")
        .with_full_potential();
    let full = GlobalList::build(&galaxy);
    let spec = full.debris().streams()[0].clone();
    let entry = *full.stream_entry(spec.number()).expect("listed");
    assert_eq!(full.entries()[0].kind(), GlobalEntryKind::Centre);
    let list = GlobalList {
        entries: vec![full.entries()[0], entry],
        debris: full.debris().clone(),
    };
    let lookup = Kept {
        galaxy: &galaxy,
        list,
        tubes: std::sync::Mutex::default(),
    };
    let source = GlobalListSource::new(&galaxy, &lookup, &NoCentreOrbitCache).expect("a centre");
    let centre = CentreMemberSource::new(&galaxy, &NoCentreOrbitCache).expect("a centre");
    let s = sphere(GalacticPosition::ORIGIN, 0.05, 100);
    let mut want = Vec::new();
    centre.systems_in_sphere(&galaxy, &s, stellar(), &mut want);
    want.sort_by_key(|hit| hit.id().raw());
    let got: Vec<SystemHit> = hits(&source, &galaxy, &s)
        .into_iter()
        .filter(|hit| matches!(hit.id().kind(), SystemIdKind::Centre(_)))
        .collect();
    assert!(!want.is_empty(), "no member of the centre within 0.05 ly");
    assert_eq!(got, want);
    let (all, own) = (
        source.expected_in_sphere(&galaxy, &s),
        centre.expected_in_sphere(&galaxy, &s),
    );
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        assert!(
            all.get(layer) >= own.get(layer),
            "{layer:?}: {all:?} {own:?}"
        );
    }
    // A member inside the shell, found as `resolve_with_refuses_…` finds one.
    let tube = lookup.tube(spec.number());
    let shares = StreamBandShares::of(&galaxy, &spec);
    let shell = entry.shell();
    let mut members = Vec::new();
    let mut found = None;
    for cell in tube.cells_touching(&tube.locate(0.5, 0.0, 0.0), LightYears::new(100.0)) {
        stream_cell_members(&galaxy, &tube, &shares, cell, MassBand::A, &mut members);
        found = members.iter().find_map(|m| {
            let at = m.record().epoch_position();
            shell
                .contains(&GalacticPosition::ORIGIN.displacement_to(at))
                .then_some(*at)
        });
        if found.is_some() {
            break;
        }
    }
    let near = found.expect("a member inside the shell on the axis");
    let returned: Vec<SystemHit> = hits(&source, &galaxy, &sphere(near, 20.0, 0))
        .into_iter()
        .filter(|hit| matches!(hit.id().kind(), SystemIdKind::Stream(_)))
        .collect();
    assert!(!returned.is_empty(), "no member about one of stream 0's");
    for hit in returned.iter().take(20) {
        assert_eq!(resolve_with(&galaxy, hit.id(), &lookup), Ok(*hit.record()));
    }
}

/// perf-range-census-streams: over the server tests' galaxy's 321 streams, the census of each
/// sphere train 27 measured, about the Sun-like point out to the widest radius a request may ask,
/// 131,072 ly, and 58,000 ly above the centre, is the old path's bit for bit: each stream's cells'
/// means summed by hand ([`expected_members`] over [`TubeTable::cells_touching`]) and added in list
/// order. So it is through a source that keeps no envelope and through one whose models keep every
/// envelope, rebuilt from its parts as a server stores them (perf-range-envelopes-on-disk), which reads 10 of the
/// 121 tables whose shells the Sun-like 50 ly sphere meets.
#[test]
#[ignore = "slow: builds a drawn galaxy's 321 tube tables and sums the widest query's 377 million \
            cells by hand, some 10 CPU-minutes"]
fn a_drawn_galaxy_s_census_is_the_old_path_s_bit_for_bit() {
    use crate::planetary::testing::in_shares;
    /// The slots it takes (`.config/nextest.toml`'s slow profile).
    const THREADS: usize = 8;
    let galaxy = Galaxy::new(Seed::new(0x4d2)).with_full_potential();
    let streams = GlobalList::build(&galaxy).debris().streams().to_vec();
    assert_eq!(streams.len(), 321);
    let tubes = in_shares(streams.len(), THREADS, |k| {
        Arc::new(TubeTable::build(&galaxy, &streams[k]).expect("the full potential"))
    });
    let lookup = Lookup {
        list: GlobalList::of_parts(&galaxy, galaxy.potential(), streams, Vec::new()),
        tubes,
        per_arm: 0,
        rebuild: false,
        handed: AtomicUsize::new(0),
    };
    // Each envelope kept from its parts, as a server reads them from disk after a restart, is its
    // table's own, bit for bit (perf-range-envelopes-on-disk).
    let models = GlobalListModels::new(&galaxy, &lookup.list);
    let spelt = |envelope: &TubeEnvelope| -> Vec<u64> {
        let boxes = envelope.boxes_ly().iter().flatten().flatten().copied();
        boxes
            .chain([envelope.member_speed_bound().value()])
            .map(hyperion_testkit::float::bits)
            .collect()
    };
    for tube in &lookup.tubes {
        let own = TubeEnvelope::of(tube);
        let read = TubeEnvelope::from_parts(*own.boxes_ly(), own.member_speed_bound())
            .expect("its own parts");
        models.keep_stored_envelope(tube.stream(), read);
        let kept = models.envelope(tube.stream()).expect("kept");
        assert_eq!(spelt(kept), spelt(&own), "{}", tube.stream().get());
    }
    let enveloped = Enveloped {
        lookup: &lookup,
        models,
        read: std::sync::Mutex::default(),
    };
    let kept = GlobalListSource::new(&galaxy, &enveloped, &NoCentreOrbitCache).expect("no centre");
    let at = |p: [f64; 3]| GalacticPosition::from_light_years(p).expect("in the cube");
    let sun = at([0.0, 26_000.0, 0.0]);
    let spheres = [
        (sun, 50.0, 10),
        (sun, 1_000.0, 14),
        (sun, 5_000.0, 26),
        (sun, 20_000.0, 104),
        (sun, 131_072.0, 238),
        (at([0.0, 0.0, 58_000.0]), 200.0, 0),
        (at([0.0, 0.0, 58_000.0]), 5_000.0, 4),
        (at([0.0, 0.0, 30_000.0]), 5_000.0, 18),
    ];
    for (centre, radius, reads) in spheres {
        let s = sphere(centre, radius, 0);
        let entries = lookup.list.entries();
        let by_stream = in_shares(entries.len(), THREADS, |n| {
            let number =
                StreamNumber::new(u16::try_from(n).expect("321 streams")).expect("a number");
            assert_eq!(entries[n].kind(), GlobalEntryKind::Stream(number));
            if !entries[n].shell().touches(&s) {
                return None;
            }
            let (tube, spec) = (&lookup.tubes[n], &lookup.list.debris().streams()[n]);
            let shares = StreamBandShares::of(&galaxy, spec);
            let cells: Vec<_> = tube
                .cells_touching(&centre, LightYears::new(radius))
                .collect();
            Some(BANDS_COARSEST_FIRST.map(|band| {
                cells.iter().fold(0.0, |sum, &c| {
                    sum + expected_members(tube, &shares, c, band)
                })
            }))
        });
        let mut old = LayerCounts::ZERO;
        for sums in by_stream.into_iter().flatten() {
            for (band, sum) in BANDS_COARSEST_FIRST.into_iter().zip(sums) {
                old.set(band.layer(), old.get(band.layer()) + sum);
            }
        }
        let bits = |counts: LayerCounts| counts.to_array().map(hyperion_testkit::float::bits);
        let fresh =
            GlobalListSource::new(&galaxy, &lookup, &NoCentreOrbitCache).expect("no centre");
        assert_eq!(
            bits(fresh.expected_in_sphere(&galaxy, &s)),
            bits(old),
            "{radius} ly"
        );
        assert_eq!(
            bits(kept.expected_in_sphere(&galaxy, &s)),
            bits(old),
            "{radius} ly"
        );
        assert_eq!(
            enveloped.taken().len(),
            reads,
            "{radius} ly: the tables read"
        );
        eprintln!("{radius} ly about {centre:?}: {old:?}");
    }
}
