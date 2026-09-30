//! Extinction maps: the service that computes them (plan 07, P07.T10.a).
//!
//! An extinction map is the visual extinction A(V) through the whole galaxy along each pixel's line
//! of sight, in magnitudes, from the mean gas field (plan 07, design note 19). It is computed,
//! cached and quantised exactly as a density map is ([`DensityMapService`](super::DensityMapService)):
//! once per galaxy, view and resolution as a [`RawDensityMap`] of log₁₀ values, whose type carries
//! no unit, rendered in bands of [`BAND_ROWS`](super::density_map::BAND_ROWS) as bulk jobs from the
//! sim's [`render_extinction_rows`], and quantised per request by
//! [`quantise_map_with_floor`](super::quantise_map_with_floor) above the fixed floor
//! [`EXTINCTION_FLOOR_LOG10_MAG`]. The encoding and the decode formula are those documented on
//! [`ExtinctionMap`](hyperion_protocol::ExtinctionMap), and
//! `packages/protocol/fixtures/extinction_map_4x2.json` pins them from both languages.

use std::fmt;
use std::sync::Arc;

use hyperion_protocol::MapView;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::map::render_extinction_rows;
use hyperion_sim::galaxy::map::{MapSelection, MapSpec};

use super::density_map::{render_bands, spec_view};
use super::{
    ComputeError, CpuPool, GalaxyCache, GalaxyKey, MapResolution, RawDensityMap, SingleFlight,
};
use crate::cache::{LruCounters, SharedByteLru};

/// log₁₀ of the extinction map's floor, 0.01 mag, in magnitudes (plan 07, P07.T10.a).
///
/// Face-on values run from about 0.01 mag at the rim to tens of magnitudes at the centre (design
/// note 19); below a hundredth of a magnitude no console could show the dimming.
pub const EXTINCTION_FLOOR_LOG10_MAG: f64 = -2.0;

/// What names an extinction map: the galaxy, the view and the resolution.
///
/// As for a density map, the bit depth is not part of it: the raw grid is what is cached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtinctionMapKey {
    galaxy: GalaxyKey,
    view: MapView,
    resolution: MapResolution,
}

impl ExtinctionMapKey {
    /// The map of `resolution` pixels over `galaxy`'s gas, seen in `view`.
    #[must_use]
    pub const fn new(galaxy: GalaxyKey, view: MapView, resolution: MapResolution) -> Self {
        Self {
            galaxy,
            view,
            resolution,
        }
    }

    /// The galaxy mapped.
    #[must_use]
    pub const fn galaxy(self) -> GalaxyKey {
        self.galaxy
    }

    /// The direction the map is seen from.
    #[must_use]
    pub const fn view(self) -> MapView {
        self.view
    }

    /// Its width in pixels.
    #[must_use]
    pub const fn resolution(self) -> MapResolution {
        self.resolution
    }

    /// The raster the sim renders for this key, over the root cube as a density map's is. The
    /// selection is plan 02's, which [`render_extinction_rows`] ignores.
    ///
    /// # Panics
    ///
    /// Never: M1's maps span the root cube exactly, which [`MapSpec::new`] allows.
    #[must_use]
    fn spec(self) -> MapSpec {
        MapSpec::new(
            spec_view(self.view),
            MapSelection::AllSystems,
            [
                u32::from(self.resolution.width_px()),
                u32::from(self.resolution.height_px(self.view)),
            ],
            [0.0, 0.0],
            self.resolution.ly_per_px(),
        )
        .expect("M1's maps are centred on the galactic centre and span the root cube exactly")
    }
}

/// The extinction maps the server has computed, in one byte budget of their own.
///
/// [`ExtinctionMapService::get`] is the whole interface, and behaves as
/// [`DensityMapService::get`](super::DensityMapService::get) does: one computation per key however
/// many ask at once, cancelled band by band once its last waiter has gone.
pub struct ExtinctionMapService {
    pool: Arc<CpuPool>,
    galaxies: Arc<GalaxyCache>,
    maps: Arc<SharedByteLru<ExtinctionMapKey, RawDensityMap>>,
    flights: SingleFlight<ExtinctionMapKey, RawDensityMap, ComputeError>,
}

impl ExtinctionMapService {
    /// A service that computes its maps on `pool` from the galaxies of `galaxies`, keeping at most
    /// `budget_bytes` of them. A budget of zero caches nothing.
    #[must_use]
    pub fn new(pool: Arc<CpuPool>, galaxies: Arc<GalaxyCache>, budget_bytes: usize) -> Self {
        Self {
            pool,
            galaxies,
            maps: Arc::new(SharedByteLru::new(budget_bytes)),
            flights: SingleFlight::new(),
        }
    }

    /// The raw map of `key`: log₁₀ of A(V) in magnitudes per pixel, computed if it is not held
    /// already.
    ///
    /// # Errors
    ///
    /// Those of [`DensityMapService::get`](super::DensityMapService::get): the galaxy's, and
    /// [`ComputeError::Submit`] or [`ComputeError::Job`] for a band.
    pub async fn get(&self, key: ExtinctionMapKey) -> Result<Arc<RawDensityMap>, ComputeError> {
        if let Some(map) = self.maps.get(&key) {
            return Ok(map);
        }
        let pool = Arc::clone(&self.pool);
        let galaxies = Arc::clone(&self.galaxies);
        let maps = Arc::clone(&self.maps);
        self.flights
            .run(key, move || async move {
                // A flight that finished just before this one started has left its map behind
                // (as in `DensityMapService::get`).
                if let Some(map) = maps.get(&key) {
                    return Ok(map);
                }
                let galaxy = galaxies.get(key.galaxy()).await?;
                let map = Arc::new(render(&pool, &galaxy, key).await?);
                // A map larger than the whole budget is handed back and not stored.
                drop(maps.insert(key, Arc::clone(&map)));
                Ok(map)
            })
            .await
    }

    /// The maps held, the bytes they are charged, and the hits, misses, evictions and refusals so
    /// far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.maps.counters()
    }
}

impl fmt::Debug for ExtinctionMapService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtinctionMapService")
            .field("counters", &self.counters())
            .field("computing", &self.flights.len())
            .finish_non_exhaustive()
    }
}

/// Renders every band of `key`'s raster on the pool from the galaxy's gas.
async fn render(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    key: ExtinctionMapKey,
) -> Result<RawDensityMap, ComputeError> {
    render_bands(
        pool,
        galaxy,
        key.spec(),
        key.resolution(),
        key.view(),
        |galaxy, spec, rows, values| render_extinction_rows(galaxy.gas(), spec, rows, values),
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::OnceLock;
    use std::time::Duration;

    use hyperion_protocol::{ExtinctionMap, ExtinctionMapRequest, UniverseIdHex};
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use serde::Deserialize;
    use tokio::time::timeout;

    use super::*;
    use crate::compute::density_map::to_log10;
    use crate::compute::{CodeDepth, QuantisedMap, quantise_map_with_floor};
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};

    /// The seed whose galaxy every map of these tests is of: the density maps' seed.
    const SEED: u64 = 0x4d2;

    /// A budget that holds every map these tests compute.
    const ROOMY: usize = 64 << 20;

    /// Upper bound on any wait, as the density map's tests have it.
    const WAIT: Duration = Duration::from_secs(300);

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
    }

    fn service(workers: usize) -> (Arc<CpuPool>, ExtinctionMapService) {
        let pool = Arc::new(
            CpuPool::new(
                NonZeroUsize::new(workers).expect("a test asks for at least one worker"),
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .expect("the pool starts"),
        );
        let galaxies = Arc::new(GalaxyCache::with_builder(Arc::clone(&pool), |_| {
            galaxy().clone()
        }));
        let service = ExtinctionMapService::new(Arc::clone(&pool), galaxies, ROOMY);
        (pool, service)
    }

    fn key(view: MapView) -> ExtinctionMapKey {
        ExtinctionMapKey::new(
            GalaxyKey::new(SEED, GENERATOR_VERSION),
            view,
            MapResolution::Px128,
        )
    }

    async fn get(service: &ExtinctionMapService, key: ExtinctionMapKey) -> Arc<RawDensityMap> {
        timeout(WAIT, service.get(key))
            .await
            .expect("timed out computing a map")
            .expect("the pool computes the map")
    }

    /// The codes of a quantised map, read back from its bytes.
    fn codes(map: &QuantisedMap) -> Vec<u16> {
        match map.depth() {
            CodeDepth::Eight => map.bytes().iter().copied().map(u16::from).collect(),
            CodeDepth::Sixteen => map
                .bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| u16::from_le_bytes(pair))
                .collect(),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_map_built_on_one_worker_equals_one_built_on_four_and_the_sims_unbanded_raster() {
        for view in [MapView::FaceOn, MapView::EdgeOn] {
            let key = key(view);
            let mut values = Vec::new();
            render_extinction_rows(
                galaxy().gas(),
                &key.spec(),
                0..u32::from(MapResolution::Px128.height_px(view)),
                &mut values,
            );
            let unbanded = to_log10(&values);
            let (one_pool, one) = service(1);
            let (four_pool, four) = service(4);
            let (on_one, on_four) = (get(&one, key).await, get(&four, key).await);
            assert_eq!(on_one.log10(), on_four.log10(), "{view:?}");
            assert_eq!(on_one.log10(), unbanded.as_slice(), "{view:?}");
            assert_eq!(
                (on_one.width_px(), on_one.height_px()),
                (128, MapResolution::Px128.height_px(view))
            );
            one_pool.shutdown().await.unwrap();
            four_pool.shutdown().await.unwrap();
        }
    }

    #[tokio::test]
    async fn the_second_get_is_a_cache_hit_and_runs_no_job() {
        let (pool, service) = service(2);
        let face_on = key(MapView::FaceOn);
        let first = get(&service, face_on).await;
        let after_build = pool.counters().completed();
        assert_eq!(
            (service.counters().entries(), service.counters().hits()),
            (1, 0)
        );
        let again = get(&service, face_on).await;
        assert!(Arc::ptr_eq(&first, &again), "the same map, not a copy");
        assert_eq!(service.counters().hits(), 1);
        assert_eq!(
            pool.counters().completed(),
            after_build,
            "a hit runs no job"
        );
        pool.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_face_on_map_of_a_fixed_seed_matches_the_golden_codes() {
        let (pool, service) = service(4);
        let map = get(&service, key(MapView::FaceOn)).await;
        let quantised = quantise_map_with_floor(&map, EXTINCTION_FLOOR_LOG10_MAG, CodeDepth::Eight);
        let codes = codes(&quantised);
        let mut golden = GoldenWriter::new();
        golden.header(GENERATOR_VERSION.get());
        golden.line(&format!("seed = 0x{SEED:016x}"));
        golden.line(&format!(
            "size = {} x {} px, {} ly per px",
            map.width_px(),
            map.height_px(),
            map.ly_per_px()
        ));
        golden.f64("floor_log10_mag", quantised.floor_log10());
        golden.f64("ceiling_log10_mag", quantised.ceiling_log10());
        golden.line(&format!(
            "code_sum = {}",
            codes.iter().map(|&code| u64::from(code)).sum::<u64>()
        ));
        // The density map's eight pixels: the centre, the bar's ends, the arms and the rim.
        for (column, row) in [
            (64_u16, 64_u16),
            (64, 32),
            (32, 64),
            (96, 64),
            (64, 96),
            (80, 80),
            (16, 16),
            (0, 0),
        ] {
            let index = usize::from(row) * usize::from(map.width_px()) + usize::from(column);
            golden.line(&format!("code[{column},{row}] = {}", codes[index]));
        }
        golden!("extinction_map_face_on_128", &golden.finish());
        pool.shutdown().await.unwrap();
    }

    /// `packages/protocol/fixtures/extinction_map_4x2.json`, which the TypeScript decoder's tests
    /// read too.
    #[derive(Debug, Deserialize)]
    struct Fixture {
        log10_mag: Vec<Option<f32>>,
        codes: Vec<u16>,
        map: ExtinctionMap,
    }

    #[test]
    fn the_four_by_two_fixture_is_reproduced_byte_for_byte() {
        let fixture: Fixture = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/protocol/fixtures/extinction_map_4x2.json"
        )))
        .unwrap();
        let expected = fixture.map;
        let values = fixture
            .log10_mag
            .iter()
            .map(|value| value.unwrap_or(f32::NEG_INFINITY))
            .collect();
        let raw = RawDensityMap::new(
            expected.width_px,
            expected.height_px,
            expected.centre_ly,
            expected.ly_per_px,
            values,
        )
        .unwrap();
        let depth = CodeDepth::try_from(expected.bits).unwrap();
        let quantised = quantise_map_with_floor(&raw, EXTINCTION_FLOOR_LOG10_MAG, depth);
        assert_eq!(codes(&quantised), fixture.codes);
        let request = ExtinctionMapRequest {
            universe: UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            view: expected.view,
            resolution: 128,
            bits: expected.bits,
        };
        let map = crate::convert::extinction_map(request, &raw, &quantised);
        assert_eq!(map, expected);
    }

    #[test]
    fn a_map_with_nothing_above_the_floor_has_its_ceiling_at_the_floor_and_every_code_zero() {
        let raw = RawDensityMap::new(2, 1, [0.0, 0.0], 1.0, vec![f32::NEG_INFINITY, -2.5]).unwrap();
        for depth in [CodeDepth::Eight, CodeDepth::Sixteen] {
            let quantised = quantise_map_with_floor(&raw, EXTINCTION_FLOOR_LOG10_MAG, depth);
            assert_eq!(quantised.floor_log10().to_bits(), (-2.0_f64).to_bits());
            assert_eq!(quantised.ceiling_log10().to_bits(), (-2.0_f64).to_bits());
            assert_eq!(codes(&quantised), [0, 0]);
        }
    }
}
