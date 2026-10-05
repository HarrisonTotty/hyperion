//! The sky census's oracle (rendering plan R06, R06.T8.e; the plan's Test helpers): every system
//! of every cell the census opens, generated whole and measured with no skip, beside the census
//! itself with the same forced caps, and the two places the identity tests stand.
//!
//! Both open the cells of the census's own plan for the forced caps (`census_plan`, whose cover of
//! each cap's padded sphere R06.T8.a's tests check), so the two differ only in the skips: the
//! census takes each cell's bright subset above its mass floor and bounds each record's light
//! before generating it, while the oracle generates every record of every cell and measures each
//! with `census_record(…, Bound::Ignored, …)`.
//!
//! The cells run on up to [`THREADS`] threads, each with its own context, and their parts are
//! merged by `merge_census`, whose order is total and whose tallies are sums, so neither the
//! threads' count nor their timing changes a bit. Both read tables for no component
//! (`LuminosityTables::dark`): a census with forced caps reads no table, and the skips read only
//! the envelope.

use std::sync::atomic::{AtomicUsize, Ordering};

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::caps::CAPPED_LAYERS;
use hyperion_sim::sky::census::{
    Bound, CensusTallies, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery, SkyStar, census_cell,
    census_plan, census_record, merge_census,
};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;
use hyperion_testkit::float;

/// The most threads the oracle and its census run on: eight, a share of a machine other test
/// processes use too; one on WebAssembly, which has no threads.
#[cfg(not(target_family = "wasm"))]
pub const THREADS: usize = 8;

/// The most threads the oracle and its census run on: eight, a share of a machine other test
/// processes use too; one on WebAssembly, which has no threads.
#[cfg(target_family = "wasm")]
pub const THREADS: usize = 1;

/// Noise-cache slots per thread: 1 MiB of the sightlines' lattice words.
const NOISE_SLOTS: usize = 1 << 16;

/// The place near the Sun the identity tests stand, ly, as the sim's own sky tests stand: 26,000 ly
/// from the centre on the +y axis (the fixture's solar circle is 3.8 disc lengths, about 26,600 ly;
/// R₀ = 8.178 kpc, GRAVITY Collaboration 2019, A&A 625, L10) and 68 ly above the plane
/// (z☉ = 20.8 ± 0.3 pc, Bennett and Bovy 2019, MNRAS 482, 1417).
pub const SUN_LY: [f64; 3] = [0.0, 26_000.0, 68.0];

/// The place in the nuclear disc the identity tests stand, ly: 150 ly from Sgr A* in the plane.
pub const NUCLEAR_DISC_LY: [f64; 3] = [0.0, 150.0, 0.0];

/// One cell's census: its stars and its tallies, as `merge_census` takes them.
pub type Part = (Vec<SkyStar>, CensusTallies);

/// An observer near the Sun at the epoch ([`SUN_LY`]).
///
/// # Panics
///
/// If `galaxy`'s bar reaches the place, which is then not like the Sun's.
#[must_use]
pub fn observer_near_sun(galaxy: &Galaxy) -> Observer {
    let bar = galaxy.params().bar().half_length().value();
    assert!(bar < SUN_LY[1], "the bar reaches {bar} ly, past the Sun");
    observer_at(SUN_LY)
}

/// An observer in the nuclear disc at the epoch ([`NUCLEAR_DISC_LY`]).
///
/// # Panics
///
/// If the place lies beyond `galaxy`'s nuclear disc's scale length (200–400 ly).
#[must_use]
pub fn observer_in_nuclear_disc(galaxy: &Galaxy) -> Observer {
    let length = galaxy.params().nuclear_disc().length().value();
    assert!(
        NUCLEAR_DISC_LY[1] < length,
        "the nuclear disc's scale length is {length} ly, inside the place"
    );
    observer_at(NUCLEAR_DISC_LY)
}

fn observer_at(ly: [f64; 3]) -> Observer {
    Observer::new(
        GalacticPosition::from_light_years(ly).expect("in the root cube"),
        UniverseTime::EPOCH,
    )
    .expect("the epoch is within the clock window")
}

/// Every capped layer at `radius`.
#[must_use]
pub fn every_layer(radius: LightYears) -> Vec<(Layer, LightYears)> {
    CAPPED_LAYERS.iter().map(|&layer| (layer, radius)).collect()
}

/// The census's oracle: every system of every cell the census of `query` opens with every cap
/// forced to `radius`, generated whole and measured with no skip, merged at the query's `n_max`.
///
/// # Panics
///
/// If `radius` is not a valid forced cap (finite, non-negative, within the root cube's diagonal).
#[must_use]
pub fn brute_force_sky(galaxy: &Galaxy, query: SkyQuery, radius: LightYears) -> SkyCensus {
    let n_max = query.n_max();
    merge_census(
        brute_force_parts(galaxy, query, &every_layer(radius)),
        n_max,
    )
}

/// The oracle's parts: [`brute_force_sky`] for each listed layer within its radius and no other
/// layer, one part per cell in the plan's order.
///
/// # Panics
///
/// If a radius is not a valid forced cap.
#[must_use]
pub fn brute_force_parts(
    galaxy: &Galaxy,
    query: SkyQuery,
    radii: &[(Layer, LightYears)],
) -> Vec<Part> {
    over_cells(galaxy, query, radii, &|ctx, key, query, out| {
        oracle_cell(galaxy, ctx, key, query, out)
    })
}

/// The oracle's parts for the cells `keys` alone: every system of each generated whole and
/// measured for `query` with no skip, one part per cell in `keys`' order. A cell's measure reads
/// no cap, so `query`'s are not forced.
#[must_use]
pub fn brute_force_parts_of(galaxy: &Galaxy, query: &SkyQuery, keys: &[CellKey]) -> Vec<Part> {
    let (tables, envelope) = (
        LuminosityTables::dark(galaxy),
        BrightnessEnvelope::build(galaxy),
    );
    over_keys(&tables, &envelope, query, keys, &|ctx, key, query, out| {
        oracle_cell(galaxy, ctx, key, query, out)
    })
}

/// The census's parts for the cells `keys` alone, skips and all: `census_cell` of each for
/// `query`, one part per cell in `keys`' order.
#[must_use]
pub fn census_parts_of(galaxy: &Galaxy, query: &SkyQuery, keys: &[CellKey]) -> Vec<Part> {
    let (tables, envelope) = (
        LuminosityTables::dark(galaxy),
        BrightnessEnvelope::build(galaxy),
    );
    over_keys(&tables, &envelope, query, keys, &|ctx, key, query, out| {
        census_cell(galaxy, ctx, key, query, out)
    })
}

/// Every record of `key`, generated whole and measured for `query` with no skip, into `out`.
fn oracle_cell(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    key: CellKey,
    query: &SkyQuery,
    out: &mut Vec<SkyStar>,
) -> CensusTallies {
    let mut tally = CensusTallies::default();
    let mut records: Vec<SystemRecord> = Vec::new();
    generate_cell(galaxy, key, &mut records);
    for record in &records {
        census_record(galaxy, ctx, record, query, Bound::Ignored, &mut tally, out);
    }
    tally
}

/// The census's parts, skips and all, with each listed layer's cap forced to its radius and no
/// other layer opened: `census_cell` over the plan's cells, one part per cell in the plan's order.
///
/// # Panics
///
/// If a radius is not a valid forced cap.
#[must_use]
pub fn census_parts(galaxy: &Galaxy, query: SkyQuery, radii: &[(Layer, LightYears)]) -> Vec<Part> {
    over_cells(galaxy, query, radii, &|ctx, key, query, out| {
        census_cell(galaxy, ctx, key, query, out)
    })
}

/// What one cell's job does: measure `key` for the forced query into `out`, returning its tallies.
type CellJob<'a> =
    dyn Fn(&mut SkyContext<'_>, CellKey, &SkyQuery, &mut Vec<SkyStar>) -> CensusTallies + Sync + 'a;

/// Runs `each` over the cells of `query`'s plan with `radii` forced, on up to [`THREADS`] threads
/// that take the cells in turn, each thread with its own context, and returns the parts in the
/// plan's order.
fn over_cells(
    galaxy: &Galaxy,
    query: SkyQuery,
    radii: &[(Layer, LightYears)],
    each: &CellJob<'_>,
) -> Vec<Part> {
    let forced = query
        .with_caps_forced_per_layer(radii)
        .expect("a valid forced cap");
    let tables = LuminosityTables::dark(galaxy);
    let envelope = BrightnessEnvelope::build(galaxy);
    // A forced plan reads neither the tables nor the noise.
    let plan = census_plan(
        galaxy,
        &tables,
        &envelope,
        &forced,
        &mut NoiseCache::with_capacity(0),
    );
    over_keys(&tables, &envelope, &forced, plan.cells(), each)
}

/// Runs `each` over the cells `keys` for `query`, on up to [`THREADS`] threads that take the
/// cells in turn, each thread with its own context, and returns the parts in `keys`' order.
fn over_keys(
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    query: &SkyQuery,
    keys: &[CellKey],
    each: &CellJob<'_>,
) -> Vec<Part> {
    let next = AtomicUsize::new(0);
    let job = || {
        let mut ctx = SkyContext {
            tables,
            envelope,
            noise: NoiseCache::with_capacity(NOISE_SLOTS),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let mut done: Vec<(usize, Part)> = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(&key) = keys.get(i) else {
                return done;
            };
            let mut stars = Vec::new();
            let tallies = each(&mut ctx, key, query, &mut stars);
            done.push((i, (stars, tallies)));
        }
    };
    let threads = THREADS.min(keys.len()).max(1);
    let mut done: Vec<(usize, Part)> = if threads == 1 {
        job()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads).map(|_| scope.spawn(job)).collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("a census thread panicked"))
                .collect()
        })
    };
    done.sort_unstable_by_key(|(i, _)| *i);
    assert_eq!(done.len(), keys.len(), "every cell measured once");
    done.into_iter().map(|(_, part)| part).collect()
}

/// Every float of `stars`, as bits: `PartialEq` holds 0.0 and −0.0 equal. The apparent position
/// is its offset in metres within its cell, as it is held; its cell is compared by `PartialEq`.
#[must_use]
pub fn float_bits(stars: &[SkyStar]) -> Vec<u64> {
    let mut bits = Vec::new();
    for star in stars {
        let c = star.colour();
        let floats = [
            star.distance().value(),
            star.v().value(),
            star.a_v().value(),
            c.lux_per_v0(),
            c.sp_ratio(),
            c.camera_band_mag(),
        ];
        let arrays = star
            .apparent()
            .offset_metres()
            .into_iter()
            .chain(c.red_green())
            .chain(c.extinction_ratio())
            .chain(c.bake_spectrum());
        bits.extend(floats.into_iter().chain(arrays).map(float::bits));
    }
    bits
}

/// Asserts `got` and `expected` the same stars in the same order, star for star and bit for bit.
#[track_caller]
pub fn assert_same_stars(got: &[SkyStar], expected: &[SkyStar], what: &str) {
    assert_eq!(got.len(), expected.len(), "{what}: the count of stars");
    for (g, e) in got.iter().zip(expected) {
        assert_eq!(g, e, "{what}");
    }
    assert_eq!(float_bits(got), float_bits(expected), "{what}: the bits");
}
