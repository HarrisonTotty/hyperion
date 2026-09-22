//! Order independence of whole-cell generation, and the cache interface it is lent through (plan
//! 03, P03.T6).
//!
//! The brainstorm's testing item: "Generating A then B equals generating B then A equals generating
//! B alone." Placement is a pure function of the galaxy and the cell, so what these tests really
//! guard is that it stays one: that no cell peeks at a neighbour, that a cache changes nothing it
//! lends, and that eviction is always safe.

use std::cell::RefCell;
use std::collections::BTreeMap;

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellCache, CellKey, NoCache, STELLAR_LAYERS, SystemRecord, cell_heap_bytes, generate_cell,
};
use hyperion_sim::id::Layer;
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

/// The seed of the galaxy these tests place cells of.
const SEED: u64 = 0x0300_04de_0000_0000;

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
}

/// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
fn sunlike() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the root cube")
}

/// A point in the bulge, some 460 ly from the centre and just above the plane, where cells hold
/// many times what they do at the Sun.
fn in_bulge() -> GalacticPosition {
    GalacticPosition::from_light_years([300.0, 350.0, 100.0]).expect("in the root cube")
}

/// The cells these tests generate: neighbours of each other in every layer, at the Sun-like point
/// and in the bulge, so that a cell that read its neighbour's stream would show up.
fn cells() -> Vec<CellKey> {
    let mut keys = Vec::new();
    for spec in STELLAR_LAYERS {
        let at_sun = CellKey::containing(spec.layer(), &sunlike()).expect("a stellar layer");
        let [x, y, z] = at_sun.gen_cell().to_array();
        for step in 0..3 {
            keys.push(CellKey::new(spec.layer(), [x + step, y, z]).expect("inside the cube"));
        }
        keys.push(CellKey::new(spec.layer(), [x, y + 1, z - 1]).expect("inside the cube"));
    }
    // Two layer-A neighbours in the bulge, where a cell holds about a hundred candidates.
    let bulge = CellKey::containing(Layer::A, &in_bulge()).expect("a stellar layer");
    let [x, y, z] = bulge.gen_cell().to_array();
    keys.push(bulge);
    keys.push(CellKey::new(Layer::A, [x + 1, y, z]).expect("inside the cube"));
    keys.push(CellKey::new(Layer::A, [x, y, z + 1]).expect("inside the cube"));
    keys
}

/// A cache that keeps every cell it has been asked for, by key.
#[derive(Debug, Default)]
struct BTreeCache {
    store: BTreeMap<CellKey, Vec<SystemRecord>>,
}

impl BTreeCache {
    /// How many cells it has generated: it never evicts, so this is its miss count.
    fn cells(&self) -> usize {
        self.store.len()
    }

    /// What it holds, as a byte-bounded cache would count it.
    fn bytes(&self) -> usize {
        self.store
            .values()
            .map(|systems| cell_heap_bytes(systems))
            .sum()
    }
}

impl CellCache for BTreeCache {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let systems = self.store.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            // `cell_heap_bytes` counts records, so a cache that keeps the buffer shrinks it.
            systems.shrink_to_fit();
            systems
        });
        f(systems)
    }
}

/// A cache that stores every cell and then evicts it at once, so every call is a miss.
#[derive(Debug, Default)]
struct EvictingCache {
    store: BTreeMap<CellKey, Vec<SystemRecord>>,
    calls: u32,
}

impl CellCache for EvictingCache {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let mut systems = Vec::new();
        generate_cell(galaxy, key, &mut systems);
        self.store.insert(key, systems);
        self.calls += 1;
        let result = f(&self.store[&key]);
        // Eviction is always safe, at any moment, including in the middle of a walk.
        self.store.clear();
        result
    }
}

fn generated(galaxy: &Galaxy, key: CellKey) -> Vec<SystemRecord> {
    let mut systems = Vec::new();
    generate_cell(galaxy, key, &mut systems);
    systems
}

#[test]
fn generating_a_cell_does_not_depend_on_what_was_generated_before_it() {
    let galaxy = galaxy();
    let keys = cells();
    // Cold every time: the function itself is pure.
    assert_order_independent(&keys, |&key| generated(&galaxy, key));
    // Behind one warm cache kept across all four passes.
    let cache = RefCell::new(BTreeCache::default());
    assert_order_independent(&keys, |&key| {
        cache
            .borrow_mut()
            .with_cell(&galaxy, key, <[SystemRecord]>::to_vec)
    });
    assert_eq!(
        cache.borrow().cells(),
        keys.len(),
        "a warm cache should have generated each cell once"
    );
    assert!(cache.borrow().bytes() > 0);
}

#[test]
fn a_then_b_equals_b_then_a_equals_b_alone() {
    let galaxy = galaxy();
    let keys = cells();
    for pair in keys.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let b_alone = generated(&galaxy, b);

        // One buffer reused, which is how a walk uses it: A then B.
        let mut buffer = Vec::new();
        generate_cell(&galaxy, a, &mut buffer);
        let a_first = buffer.clone();
        generate_cell(&galaxy, b, &mut buffer);
        assert_eq!(buffer, b_alone, "B after A differs from B alone");

        // B then A.
        generate_cell(&galaxy, b, &mut buffer);
        assert_eq!(buffer, b_alone, "B after B differs from B alone");
        generate_cell(&galaxy, a, &mut buffer);
        assert_eq!(buffer, a_first, "A after B differs from A after nothing");

        // Record for record, and not only as a whole.
        for (after, alone) in generated(&galaxy, b).iter().zip(&b_alone) {
            assert_same_records(after, alone);
        }
    }
}

/// Two records of the same system, compared field by field and float by float: a derived `==` would
/// take `+0.0` for `−0.0`.
#[track_caller]
fn assert_same_records(after: &SystemRecord, alone: &SystemRecord) {
    assert_eq!(after.id(), alone.id());
    assert_eq!(
        after.epoch_position().cell().to_array(),
        alone.epoch_position().cell().to_array()
    );
    for (a, b) in after
        .epoch_position()
        .offset_metres()
        .into_iter()
        .zip(alone.epoch_position().offset_metres())
    {
        assert_same_bits(a, b);
    }
    assert_eq!(after.origin(), alone.origin());
    assert_eq!(after.population(), alone.population());
    assert_same_bits(
        after.primary_initial_mass().value(),
        alone.primary_initial_mass().value(),
    );
    assert_same_bits(after.age_at_epoch().value(), alone.age_at_epoch().value());
}

/// The same seed, built twice, places the same systems: what the server does on every start, when
/// it rebuilds the galaxy from `(seed, generator_version)` and resolves saved IDs against it.
#[test]
fn the_same_seed_built_twice_places_the_same_systems() {
    let (first, second) = (galaxy(), galaxy());
    for key in cells() {
        let (a, b) = (generated(&first, key), generated(&second, key));
        assert_eq!(a.len(), b.len(), "cell {:?}", key.gen_cell().to_array());
        for (x, y) in a.iter().zip(&b) {
            assert_same_records(x, y);
        }
    }
}

#[test]
fn a_toy_cache_lends_the_same_systems_as_no_cache() {
    let galaxy = galaxy();
    let keys = cells();
    let mut no_cache = NoCache::new();
    let mut warm = BTreeCache::default();
    // Every key twice, so the second visit is a hit.
    for &key in keys.iter().chain(keys.iter()) {
        let expected = no_cache.with_cell(&galaxy, key, <[SystemRecord]>::to_vec);
        let lent = warm.with_cell(&galaxy, key, <[SystemRecord]>::to_vec);
        assert_eq!(lent, expected, "cell {:?}", key.gen_cell().to_array());
        assert_eq!(expected, generated(&galaxy, key));
    }
    assert_eq!(warm.cells(), keys.len());
}

#[test]
fn a_cache_that_evicts_on_every_call_changes_nothing() {
    let galaxy = galaxy();
    let keys = cells();
    let mut evicting = EvictingCache::default();
    let mut no_cache = NoCache::new();
    for &key in &keys {
        let expected = no_cache.with_cell(&galaxy, key, <[SystemRecord]>::to_vec);
        assert_eq!(
            evicting.with_cell(&galaxy, key, <[SystemRecord]>::to_vec),
            expected
        );
    }
    assert!(evicting.store.is_empty(), "eviction left something behind");
    assert_eq!(evicting.calls, u32::try_from(keys.len()).unwrap());
}

#[test]
fn a_cells_weight_is_the_bytes_of_its_records() {
    let galaxy = galaxy();
    let bulge_cell = CellKey::containing(Layer::A, &in_bulge()).expect("a stellar layer");
    let in_bulge = generated(&galaxy, bulge_cell);
    assert!(
        in_bulge.len() > 8,
        "a layer-A bulge cell holds {} systems",
        in_bulge.len()
    );
    let record_bytes = cell_heap_bytes(&in_bulge[..1]);
    assert!(
        (1..=80).contains(&record_bytes),
        "{record_bytes} bytes a record"
    );
    assert_eq!(cell_heap_bytes(&in_bulge), in_bulge.len() * record_bytes);
    // A cache bounded by bytes pays far more for a bulge cell than for one at the rim.
    let rim = generated(
        &galaxy,
        CellKey::new(Layer::A, [0, 7_000, 0]).expect("in the cube"),
    );
    assert!(cell_heap_bytes(&rim) < cell_heap_bytes(&in_bulge));
    assert_eq!(cell_heap_bytes(&[]), 0);
}
