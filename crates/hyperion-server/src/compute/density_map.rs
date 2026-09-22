//! Density maps: the service that computes them, the raw grid the cache holds, and its quantisation
//! for the wire.
//!
//! A map is computed once per galaxy, view, population and resolution as a [`RawDensityMap`] of
//! log₁₀ column densities, and that raw grid is what the cache keeps (plan 04, design note 12).
//! [`DensityMapService`] owns that cache: it renders a map's rows in bands of [`BAND_ROWS`] as bulk
//! jobs on the CPU pool, from plan 02's [`render_rows`], and hands out the assembled grid.
//! Each response quantises it to the bit depth its request asks for with [`quantise_map`], one
//! pass over the grid and cheap next to computing it, and base64-encodes the codes with
//! [`QuantisedMap::to_base64`]. The encoding, the orientation and the decode formula are those
//! documented on [`DensityMap`](hyperion_protocol::DensityMap), and
//! `packages/protocol/fixtures/density_map_4x2.json` pins them from this side and from the
//! TypeScript decoder's.

use std::error::Error;
use std::fmt;
use std::mem;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hyperion_protocol::{MapPopulation, MapView};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::map::{MapSelection, MapSpec, MapView as SpecView, render_rows};
use hyperion_sim::math;

use super::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyCache, GalaxyKey, JobError, Priority,
    SingleFlight,
};
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// How far the face-on floor lies below the ceiling, in dex.
///
/// The brainstorm's Visualiser gives face-on density a span of about four orders of magnitude;
/// plan 04's design note 12 puts the floor one decade lower still.
const FACE_ON_SPAN_DEX: f64 = 5.0;

/// How far the edge-on floor lies below the ceiling, in dex: the brainstorm's six orders of
/// magnitude edge-on, and one decade more, as face-on.
const EDGE_ON_SPAN_DEX: f64 = 7.0;

/// A density map as computed: log₁₀ of the column density of every pixel, before quantisation.
///
/// Pixels run row by row from the top and left to right within a row, in the orientation of the
/// map's view (see [`DensityMap`](hyperion_protocol::DensityMap)). Each value is log₁₀ of systems
/// per square light-year along the pixel's line of sight, held as `f32`, which is far finer than
/// the 16-bit codes it becomes; a pixel with no systems at all holds [`f32::NEG_INFINITY`].
#[derive(Debug, Clone, PartialEq)]
pub struct RawDensityMap {
    width_px: u16,
    height_px: u16,
    centre_ly: [f64; 2],
    ly_per_px: f64,
    log10: Vec<f32>,
}

impl RawDensityMap {
    /// A map of `width_px` × `height_px` pixels, each `ly_per_px` light-years square, centred on
    /// `centre_ly` (horizontal, vertical: (x, y) face-on, (x, z) edge-on, in light-years in the
    /// `GALACTIC` frame), with `log10` holding every pixel's log₁₀ column density in systems per
    /// square light-year.
    ///
    /// # Errors
    ///
    /// [`BuildRawMapError`] if either size is zero, if `log10` does not hold exactly one value per
    /// pixel, if the pixel size is not finite and positive or the centre not finite, or if a value
    /// is NaN or +∞. −∞ is the value of an empty pixel.
    pub fn new(
        width_px: u16,
        height_px: u16,
        centre_ly: [f64; 2],
        ly_per_px: f64,
        log10: Vec<f32>,
    ) -> Result<Self, BuildRawMapError> {
        if width_px == 0 || height_px == 0 {
            return Err(BuildRawMapError::Empty);
        }
        let pixels = usize::from(width_px) * usize::from(height_px);
        if log10.len() != pixels {
            return Err(BuildRawMapError::WrongPixelCount {
                expected: pixels,
                actual: log10.len(),
            });
        }
        let geometry_is_valid = ly_per_px.is_finite()
            && ly_per_px > 0.0
            && centre_ly.iter().all(|centre| centre.is_finite());
        if !geometry_is_valid {
            return Err(BuildRawMapError::InvalidGeometry);
        }
        if let Some(index) = log10
            .iter()
            .position(|value| value.is_nan() || (value.is_infinite() && value.is_sign_positive()))
        {
            return Err(BuildRawMapError::InvalidValue { index });
        }
        Ok(Self {
            width_px,
            height_px,
            centre_ly,
            ly_per_px,
            log10,
        })
    }

    /// Pixels per row.
    #[must_use]
    pub fn width_px(&self) -> u16 {
        self.width_px
    }

    /// Rows.
    #[must_use]
    pub fn height_px(&self) -> u16 {
        self.height_px
    }

    /// The centre of the raster in light-years, (horizontal, vertical): (x, y) face-on, (x, z)
    /// edge-on, in the `GALACTIC` frame.
    #[must_use]
    pub fn centre_ly(&self) -> [f64; 2] {
        self.centre_ly
    }

    /// The width and height of one pixel, in light-years.
    #[must_use]
    pub fn ly_per_px(&self) -> f64 {
        self.ly_per_px
    }

    /// log₁₀ of every pixel's column density in systems per square light-year, row by row from
    /// the top; [`f32::NEG_INFINITY`] for an empty pixel.
    #[must_use]
    pub fn log10(&self) -> &[f32] {
        &self.log10
    }
}

impl HeapBytes for RawDensityMap {
    fn heap_bytes(&self) -> usize {
        // A `Vec`'s allocation never exceeds `isize::MAX` bytes, so this cannot overflow.
        self.log10.capacity() * mem::size_of::<f32>()
    }
}

/// A [`RawDensityMap`] could not be built from what it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildRawMapError {
    /// The width or the height is zero.
    Empty,
    /// The grid does not hold one value per pixel.
    WrongPixelCount {
        /// Width times height.
        expected: usize,
        /// The values given.
        actual: usize,
    },
    /// The pixel size is not finite and positive, or the centre is not finite.
    InvalidGeometry,
    /// A value is NaN or +∞.
    InvalidValue {
        /// The index of the first such value.
        index: usize,
    },
}

impl fmt::Display for BuildRawMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a density map needs at least one pixel"),
            Self::WrongPixelCount { expected, actual } => {
                write!(
                    f,
                    "a density map of {expected} pixels was given {actual} values"
                )
            }
            Self::InvalidGeometry => f.write_str(
                "a density map's pixel size must be finite and positive and its centre finite",
            ),
            Self::InvalidValue { index } => {
                write!(f, "density map pixel {index} is neither finite nor empty")
            }
        }
    }
}

impl Error for BuildRawMapError {}

/// How many bits each pixel code of a quantised map takes: the two depths the protocol offers.
///
/// Quantising is cheap and the raw grid is what is cached, so each request chooses (plan 04,
/// design note 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CodeDepth {
    /// One byte per code: codes 0 to 255.
    Eight,
    /// Two bytes per code, little-endian: codes 0 to 65,535.
    Sixteen,
}

impl CodeDepth {
    /// Bits per code, as the wire's `bits` gives them.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Eight => 8,
            Self::Sixteen => 16,
        }
    }

    /// The largest code, `2^bits − 1`, which stands for the map's ceiling.
    #[must_use]
    pub const fn max_code(self) -> u16 {
        match self {
            Self::Eight => 0xff,
            Self::Sixteen => 0xffff,
        }
    }

    /// Bytes per code.
    #[must_use]
    const fn bytes_per_code(self) -> usize {
        match self {
            Self::Eight => 1,
            Self::Sixteen => 2,
        }
    }
}

impl TryFrom<u8> for CodeDepth {
    type Error = ParseCodeDepthError;

    /// Reads the wire's `bits`: 8 or 16.
    fn try_from(bits: u8) -> Result<Self, Self::Error> {
        match bits {
            8 => Ok(Self::Eight),
            16 => Ok(Self::Sixteen),
            _ => Err(ParseCodeDepthError { bits }),
        }
    }
}

/// A bit depth other than 8 or 16 was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParseCodeDepthError {
    bits: u8,
}

impl ParseCodeDepthError {
    /// The depth asked for.
    #[must_use]
    pub fn bits(&self) -> u8 {
        self.bits
    }
}

impl fmt::Display for ParseCodeDepthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "codes are 8 or 16 bits, not {}", self.bits)
    }
}

impl Error for ParseCodeDepthError {}

/// A density map quantised for the wire: its codes, and the floor and ceiling they span.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantisedMap {
    depth: CodeDepth,
    floor_log10_per_ly2: f64,
    ceiling_log10_per_ly2: f64,
    bytes: Vec<u8>,
}

impl QuantisedMap {
    /// The bit depth of the codes.
    #[must_use]
    pub fn depth(&self) -> CodeDepth {
        self.depth
    }

    /// log₁₀ of the column density, in systems per square light-year, that code 1 stands for.
    #[must_use]
    pub fn floor_log10_per_ly2(&self) -> f64 {
        self.floor_log10_per_ly2
    }

    /// log₁₀ of the column density, in systems per square light-year, that the largest code
    /// stands for: the map's maximum.
    #[must_use]
    pub fn ceiling_log10_per_ly2(&self) -> f64 {
        self.ceiling_log10_per_ly2
    }

    /// The codes as bytes: one per code at 8 bits, two little-endian at 16, row by row from the
    /// top.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The codes in the standard base64 alphabet with padding, as the wire's `data_base64`.
    #[must_use]
    pub fn to_base64(&self) -> String {
        STANDARD.encode(&self.bytes)
    }
}

/// Quantises a raw map to codes of `depth` bits for `view`, by plan 04's design note 12.
///
/// The ceiling is the map's largest value and the floor lies 5 dex below it face-on and 7 dex
/// edge-on. With `max = 2^bits − 1`, a pixel at or below the floor, an empty one included, gets
/// code 0, and any other pixel of log density `v` gets
///
/// ```text
/// code = 1 + round((v − floor) ÷ (ceiling − floor) × (max − 1))
/// ```
///
/// clamped to 1..=`max`, so the ceiling's pixel gets `max`. That is the inverse of the decode
/// formula on [`DensityMap`](hyperion_protocol::DensityMap), to within half a step. A map with no
/// systems at all has floor and ceiling 0 and every code 0.
#[must_use]
pub fn quantise_map(map: &RawDensityMap, view: MapView, depth: CodeDepth) -> QuantisedMap {
    let len = map.log10.len() * depth.bytes_per_code();
    let Some(ceiling) = map
        .log10
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .max_by(f32::total_cmp)
    else {
        return QuantisedMap {
            depth,
            floor_log10_per_ly2: 0.0,
            ceiling_log10_per_ly2: 0.0,
            bytes: vec![0; len],
        };
    };
    let ceiling = f64::from(ceiling);
    let floor = ceiling - span_dex(view);
    let span = ceiling - floor;
    let max = depth.max_code();
    let mut bytes = Vec::with_capacity(len);
    for &value in &map.log10 {
        let code = code_of(f64::from(value), floor, span, max).to_le_bytes();
        match depth {
            // An 8-bit map's codes are at most 255, so the low byte is the whole code.
            CodeDepth::Eight => bytes.push(code[0]),
            CodeDepth::Sixteen => bytes.extend_from_slice(&code),
        }
    }
    QuantisedMap {
        depth,
        floor_log10_per_ly2: floor,
        ceiling_log10_per_ly2: ceiling,
        bytes,
    }
}

/// How far the floor lies below the ceiling in `view`, in dex.
#[must_use]
fn span_dex(view: MapView) -> f64 {
    match view {
        MapView::FaceOn => FACE_ON_SPAN_DEX,
        MapView::EdgeOn => EDGE_ON_SPAN_DEX,
    }
}

/// The code of one pixel of log density `value`: 0 at or below `floor` (−∞ included), else
/// 1 to `max` linearly over `span` dex above it.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the code is rounded and then clamped to 1..=max, so it is a whole number that a u16 \
              holds exactly"
)]
fn code_of(value: f64, floor: f64, span: f64, max: u16) -> u16 {
    if value <= floor {
        return 0;
    }
    let step = ((value - floor) / span * f64::from(max - 1)).round();
    (1.0 + step).clamp(1.0, f64::from(max)) as u16
}

/// The width of every M1 map in light-years: the root cube, face-on and edge-on alike.
///
/// Plan 04's design note 12 fixes the extents: face-on a square of 131,072 ly centred on the
/// galactic centre, edge-on the same width and half the height, so that both views span everything
/// the generator places.
pub const MAP_WIDTH_LY: f64 = 131_072.0;

/// How many rows of a map one band renders (plan 04, P04.T11.c).
///
/// A band is the unit of bulk work, so it is what bounds how long an interactive job can wait behind
/// a map (design note 21) and how much of a cancelled map runs on. Sixteen rows of a 1,024-pixel
/// edge-on map are some seconds; see the plan's Risks for the measured cost.
const BAND_ROWS: u16 = 16;

/// How wide a map is, in pixels: the four resolutions a request may ask for.
///
/// The height follows from the view (design note 12): a face-on map is square, an edge-on one half
/// as tall as it is wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapResolution {
    /// 128 pixels wide, each 1,024 ly.
    Px128,
    /// 256 pixels wide, each 512 ly.
    Px256,
    /// 512 pixels wide, each 256 ly.
    Px512,
    /// 1,024 pixels wide, each 128 ly.
    Px1024,
}

impl MapResolution {
    /// Every resolution, from the smallest.
    pub const ALL: [Self; 4] = [Self::Px128, Self::Px256, Self::Px512, Self::Px1024];

    /// The width in pixels, as the wire's `resolution` gives it.
    #[must_use]
    pub const fn width_px(self) -> u16 {
        match self {
            Self::Px128 => 128,
            Self::Px256 => 256,
            Self::Px512 => 512,
            Self::Px1024 => 1_024,
        }
    }

    /// The height in pixels: the width face-on, half of it edge-on.
    #[must_use]
    pub const fn height_px(self, view: MapView) -> u16 {
        match view {
            MapView::FaceOn => self.width_px(),
            MapView::EdgeOn => self.width_px() / 2,
        }
    }

    /// The size of a pixel in light-years, square: [`MAP_WIDTH_LY`] over the width.
    #[must_use]
    pub fn ly_per_px(self) -> f64 {
        MAP_WIDTH_LY / f64::from(self.width_px())
    }
}

impl TryFrom<u16> for MapResolution {
    type Error = ParseMapResolutionError;

    /// Reads the wire's `resolution`: 128, 256, 512 or 1,024.
    fn try_from(resolution: u16) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.width_px() == resolution)
            .ok_or(ParseMapResolutionError { resolution })
    }
}

/// A map width other than 128, 256, 512 or 1,024 pixels was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParseMapResolutionError {
    resolution: u16,
}

impl ParseMapResolutionError {
    /// The width asked for.
    #[must_use]
    pub fn resolution(&self) -> u16 {
        self.resolution
    }
}

impl fmt::Display for ParseMapResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a map is 128, 256, 512 or 1024 pixels wide, not {}",
            self.resolution
        )
    }
}

impl Error for ParseMapResolutionError {}

/// What names a density map: the galaxy, the view, the systems counted and the resolution.
///
/// The bit depth is not part of it, because the raw grid is what is cached and every depth is
/// quantised from the same grid (design note 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapKey {
    galaxy: GalaxyKey,
    view: MapView,
    population: MapPopulation,
    resolution: MapResolution,
}

impl MapKey {
    /// The map of `resolution` pixels over `galaxy`, seen in `view`, counting `population`.
    #[must_use]
    pub const fn new(
        galaxy: GalaxyKey,
        view: MapView,
        population: MapPopulation,
        resolution: MapResolution,
    ) -> Self {
        Self {
            galaxy,
            view,
            population,
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

    /// Which systems it counts.
    #[must_use]
    pub const fn population(self) -> MapPopulation {
        self.population
    }

    /// Its width in pixels.
    #[must_use]
    pub const fn resolution(self) -> MapResolution {
        self.resolution
    }

    /// The raster the sim renders for this key: one [`MapSpec`] for the whole map, which every band
    /// renders part of, so that the result does not depend on how the rows were split.
    ///
    /// # Panics
    ///
    /// Never: M1's maps span the root cube exactly, which [`MapSpec::new`] allows.
    #[must_use]
    fn spec(self) -> MapSpec {
        MapSpec::new(
            spec_view(self.view),
            spec_selection(self.population),
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

/// The view the sim renders, from the view the wire names.
#[must_use]
fn spec_view(view: MapView) -> SpecView {
    match view {
        MapView::FaceOn => SpecView::FaceOn,
        MapView::EdgeOn => SpecView::EdgeOn,
    }
}

/// The selection the sim renders, from the population the wire names.
#[must_use]
fn spec_selection(population: MapPopulation) -> MapSelection {
    match population {
        MapPopulation::All => MapSelection::AllSystems,
        MapPopulation::Young => MapSelection::YoungOnly,
    }
}

/// The density maps the server has computed, in one byte budget.
///
/// [`DensityMapService::get`] is the whole interface: it hands back a key's raw map, computing it on
/// the CPU pool if no one has yet, and taking the galaxy it needs from the [`GalaxyCache`]. Callers
/// that ask for one key at once share one computation through a [`SingleFlight`], and if every one of
/// them gives up, the bands still queued are skipped: a map in progress stops only when its last
/// waiter has gone (plan 04, design note 5).
pub struct DensityMapService {
    pool: Arc<CpuPool>,
    galaxies: Arc<GalaxyCache>,
    maps: Arc<SharedByteLru<MapKey, RawDensityMap>>,
    flights: SingleFlight<MapKey, RawDensityMap, ComputeError>,
}

impl DensityMapService {
    /// A service that computes its maps on `pool` from the galaxies of `galaxies`, keeping at most
    /// `budget_bytes` of them.
    ///
    /// A budget of zero caches nothing: every request recomputes its map, which is measured in
    /// seconds, so it is a setting for a memory-starved host and not a default.
    #[must_use]
    pub fn new(pool: Arc<CpuPool>, galaxies: Arc<GalaxyCache>, budget_bytes: usize) -> Self {
        Self {
            pool,
            galaxies,
            maps: Arc::new(SharedByteLru::new(budget_bytes)),
            flights: SingleFlight::new(),
        }
    }

    /// The raw map of `key`, computed if it is not held already.
    ///
    /// # Errors
    ///
    /// [`ComputeError`] as [`GalaxyCache::get`] gives it for the galaxy the map is of, and for each
    /// band: [`ComputeError::Submit`] if the pool is shutting down, and [`ComputeError::Job`] if a
    /// band was cancelled, panicked, or was still queued when the pool stopped.
    pub async fn get(&self, key: MapKey) -> Result<Arc<RawDensityMap>, ComputeError> {
        if let Some(map) = self.maps.get(&key) {
            return Ok(map);
        }
        let pool = Arc::clone(&self.pool);
        let galaxies = Arc::clone(&self.galaxies);
        let maps = Arc::clone(&self.maps);
        self.flights
            .run(key, move || async move {
                // As in `GalaxyCache::get`: a caller can arrive just after another flight has
                // finished and left the registry, and the map it computed is worth more than a
                // second computation of it. This look does not count as a lookup.
                if let Some(map) = maps.get(&key) {
                    return Ok(map);
                }
                let galaxy = galaxies.get(key.galaxy()).await?;
                let map = Arc::new(render(&pool, &galaxy, key).await?);
                // A map larger than the whole budget is handed back and not stored; the caller gets
                // it either way.
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

impl fmt::Debug for DensityMapService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DensityMapService")
            .field("counters", &self.counters())
            .field("computing", &self.flights.len())
            .finish_non_exhaustive()
    }
}

/// Renders every band of `key`'s raster on the pool and assembles them in row order.
///
/// Each band is one bulk job over [`BAND_ROWS`] rows of the one [`MapSpec`], so the values do not
/// depend on how the rows were split or on how many workers ran them (plan 02, P02.T10: the rasters
/// of band sizes 1 to 13 are identical bit for bit).
async fn render(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    key: MapKey,
) -> Result<RawDensityMap, ComputeError> {
    let spec = key.spec();
    let (width_px, height_px) = (
        key.resolution().width_px(),
        key.resolution().height_px(key.view()),
    );
    let token = CancelToken::new();
    // Cancels the bands still queued once every waiter on this map has gone.
    let _cancel_on_drop = CancelOnDrop::new(token.clone());
    let mut bands = Vec::with_capacity(usize::from(height_px).div_ceil(usize::from(BAND_ROWS)));
    let mut first_row = 0;
    while first_row < u32::from(height_px) {
        let rows = first_row..(first_row + u32::from(BAND_ROWS)).min(u32::from(height_px));
        first_row = rows.end;
        let galaxy = Arc::clone(galaxy);
        bands.push(
            pool.submit(Priority::Bulk, token.clone(), move |token| {
                if token.is_cancelled() {
                    return None;
                }
                let mut values = Vec::new();
                render_rows(galaxy.fields(), &spec, rows, &mut values);
                Some(to_log10(&values))
            })
            .await?,
        );
    }
    let mut log10 = Vec::with_capacity(usize::from(width_px) * usize::from(height_px));
    for band in bands {
        let values = band
            .await
            .unwrap_or_else(|closed| Err(JobError::from(closed)))?
            .ok_or(JobError::Cancelled)?;
        log10.extend_from_slice(&values);
    }
    Ok(RawDensityMap::new(
        width_px,
        height_px,
        [0.0, 0.0],
        key.resolution().ly_per_px(),
        log10,
    )
    .expect("a raster of the key's size, whose values are finite or −∞"))
}

/// log₁₀ of every pixel's column density, as `f32`: `−∞` where there are no systems at all.
///
/// The logarithm is the sim's ([`math::log10`]), not the platform's, so that a map's codes are the
/// same on every machine, as the golden file that pins them assumes.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "a column density's logarithm lies well inside f32's range, and f32 is far finer than \
              the 16-bit codes it becomes (see RawDensityMap)"
)]
fn to_log10(values: &[f64]) -> Vec<f32> {
    values
        .iter()
        .map(|&density| {
            if density > 0.0 {
                math::log10(density) as f32
            } else {
                // An empty pixel, and the guard that keeps a NaN out of the map.
                f32::NEG_INFINITY
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::OnceLock;
    use std::time::Duration;

    use hyperion_protocol::{DensityMap, UniverseIdHex};
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use serde::Deserialize;
    use tokio::time::timeout;

    use super::*;
    use crate::cache::{ByteLru, ENTRY_OVERHEAD_BYTES};
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};

    const EMPTY: f32 = f32::NEG_INFINITY;

    /// The seed whose galaxy every map of these tests is of.
    const SEED: u64 = 0x4d2;

    /// A budget that holds every map these tests compute.
    const ROOMY: usize = 64 << 20;

    /// Upper bound on any wait: a 128-pixel edge-on raster is seconds of work, and more on a loaded
    /// machine (see the plan's Risks for the measured cost).
    const WAIT: Duration = Duration::from_secs(300);

    /// One real galaxy of [`SEED`], built once for the whole test binary: every service hands out
    /// clones of it, so the maps are the maps of one galaxy and nothing pays for a rebuild.
    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
    }

    /// A pool of `workers` workers and a service over it, with room for every map.
    fn service(workers: usize) -> (Arc<CpuPool>, DensityMapService) {
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
        let service = DensityMapService::new(Arc::clone(&pool), galaxies, ROOMY);
        (pool, service)
    }

    /// The key of a 128-pixel map of the test galaxy.
    fn key(view: MapView, population: MapPopulation) -> MapKey {
        MapKey::new(
            GalaxyKey::new(SEED, GENERATOR_VERSION),
            view,
            population,
            MapResolution::Px128,
        )
    }

    async fn get(service: &DensityMapService, key: MapKey) -> Arc<RawDensityMap> {
        timeout(WAIT, service.get(key))
            .await
            .expect("timed out computing a map")
            .expect("the pool computes the map")
    }

    /// The value at column `column` and row `row`.
    fn pixel(map: &RawDensityMap, column: u16, row: u16) -> f32 {
        map.log10()[usize::from(row) * usize::from(map.width_px()) + usize::from(column)]
    }

    /// Asserts that two pixels of one map agree: exactly, which two empty pixels do, or to a
    /// relative 10⁻⁹, which is as close as two sums of the same components in a different order come.
    #[track_caller]
    fn assert_mirrored(here: f32, there: f32, at: &str) {
        if here.total_cmp(&there) == std::cmp::Ordering::Equal {
            return;
        }
        let error = (f64::from(here) - f64::from(there)).abs() / f64::from(here).abs().max(1.0);
        assert!(error < 1e-9, "{at}: {here} against {there}");
    }

    #[test]
    fn a_resolution_is_one_of_the_four_widths_and_carries_its_geometry() {
        for resolution in MapResolution::ALL {
            assert_eq!(
                MapResolution::try_from(resolution.width_px()),
                Ok(resolution)
            );
            assert_same_bits(
                resolution.ly_per_px() * f64::from(resolution.width_px()),
                MAP_WIDTH_LY,
            );
            assert_eq!(resolution.height_px(MapView::FaceOn), resolution.width_px());
            assert_eq!(
                resolution.height_px(MapView::EdgeOn),
                resolution.width_px() / 2
            );
            // The edge-on map is half the root cube tall, as design note 12 fixes it.
            assert_same_bits(
                resolution.ly_per_px() * f64::from(resolution.height_px(MapView::EdgeOn)),
                0.5 * MAP_WIDTH_LY,
            );
        }
        for width in [0, 1, 127, 129, 500, 2_048, u16::MAX] {
            let error = MapResolution::try_from(width).unwrap_err();
            assert_eq!(error.resolution(), width);
            assert_eq!(
                error.to_string(),
                format!("a map is 128, 256, 512 or 1024 pixels wide, not {width}")
            );
        }
    }

    #[test]
    fn the_map_spans_the_sims_root_cube() {
        // `MapKey::spec` cannot fail only because the picture fits the cube exactly; plan 01 owns
        // the cube's half-width and this is where plan 04's extent meets it.
        assert_same_bits(
            MAP_WIDTH_LY,
            2.0 * f64::from(hyperion_sim::coords::ROOT_HALF_WIDTH_LY),
        );
        for view in [MapView::FaceOn, MapView::EdgeOn] {
            for resolution in MapResolution::ALL {
                let key = MapKey::new(
                    GalaxyKey::new(SEED, GENERATOR_VERSION),
                    view,
                    MapPopulation::All,
                    resolution,
                );
                let spec = key.spec();
                assert_same_bits(spec.extent()[0], MAP_WIDTH_LY);
                assert_eq!(spec.width_px(), u32::from(resolution.width_px()));
                assert_eq!(spec.height_px(), u32::from(resolution.height_px(view)));
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn one_worker_and_four_build_the_same_map() {
        let key = key(MapView::FaceOn, MapPopulation::All);
        let (one_pool, one) = service(1);
        let (four_pool, four) = service(4);
        let (on_one, on_four) = tokio::join!(get(&one, key), get(&four, key));
        // Bit for bit: every band renders from one `MapSpec`, so neither the band size nor the
        // number of workers that ran them changes a pixel (plan 02, P02.T10).
        assert_eq!(on_one, on_four);
        assert_eq!(on_one.log10().len(), 128 * 128);
        one_pool.shutdown().await.unwrap();
        four_pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn the_second_get_is_a_cache_hit_and_runs_no_job() {
        let (pool, service) = service(2);
        let all = key(MapView::FaceOn, MapPopulation::All);
        let first = get(&service, all).await;
        let after_build = pool.counters().completed();
        let counters = service.counters();
        assert_eq!((counters.entries(), counters.hits()), (1, 0));
        assert_eq!(
            counters.bytes(),
            ByteLru::<MapKey, RawDensityMap>::charge(&first)
        );

        let again = get(&service, all).await;
        assert!(Arc::ptr_eq(&first, &again), "the same map, not a copy");
        assert_eq!(service.counters().hits(), 1);
        assert_eq!(
            pool.counters().completed(),
            after_build,
            "a hit runs no job at all"
        );
        // Another population is another key, and is computed.
        let young = get(&service, key(MapView::FaceOn, MapPopulation::Young)).await;
        assert_ne!(young.log10(), first.log10());
        assert_eq!(service.counters().entries(), 2);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_concurrent_gets_compute_once() {
        let (pool, service) = service(2);
        let key = key(MapView::FaceOn, MapPopulation::All);
        let (first, second) = tokio::join!(get(&service, key), get(&service, key));
        assert!(Arc::ptr_eq(&first, &second));
        let counters = service.counters();
        assert_eq!(
            (counters.entries(), counters.misses(), counters.hits()),
            (1, 3, 0),
            "both callers missed the cache, as did the flight's own look, and one map was computed"
        );
        // One raster's bands, and one galaxy build, and nothing twice.
        let bands = u64::from(128_u32.div_ceil(u32::from(BAND_ROWS)));
        assert_eq!(pool.counters().completed(), bands + 1);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_face_on_map_is_unchanged_by_a_half_turn_and_peaks_at_the_centre() {
        let (pool, service) = service(4);
        let map = get(&service, key(MapView::FaceOn, MapPopulation::All)).await;
        let (width, height) = (map.width_px(), map.height_px());
        // The bar, the discs and evenly spaced arms all have a half-turn's symmetry, so the picture
        // is its own 180° rotation to the last few bits of the fields' sums.
        for row in 0..height {
            for column in 0..width {
                assert_mirrored(
                    pixel(&map, column, row),
                    pixel(&map, width - 1 - column, height - 1 - row),
                    &format!("({column}, {row}) against its opposite"),
                );
            }
        }
        // The four pixels at the centre hold the galaxy's nucleus, the densest column there is.
        let centre = pixel(&map, width / 2, height / 2);
        let largest = map
            .log10()
            .iter()
            .copied()
            .max_by(f32::total_cmp)
            .expect("the map has pixels");
        assert_eq!(
            centre.total_cmp(&largest),
            std::cmp::Ordering::Equal,
            "the centre pixel is the map's ceiling, {centre} against {largest}"
        );
        pool.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn an_edge_on_map_is_symmetric_in_z() {
        let (pool, service) = service(4);
        let map = get(&service, key(MapView::EdgeOn, MapPopulation::All)).await;
        let (width, height) = (map.width_px(), map.height_px());
        assert_eq!(height, width / 2, "edge-on is half as tall as it is wide");
        for row in 0..height / 2 {
            for column in 0..width {
                assert_mirrored(
                    pixel(&map, column, row),
                    pixel(&map, column, height - 1 - row),
                    &format!("({column}, {row}) against its reflection in the plane"),
                );
            }
        }
        pool.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_face_on_map_of_a_fixed_seed_matches_the_golden_codes() {
        let (pool, service) = service(4);
        let map = get(&service, key(MapView::FaceOn, MapPopulation::All)).await;
        let quantised = quantise_map(&map, MapView::FaceOn, CodeDepth::Eight);
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
        golden.f64("floor_log10_per_ly2", quantised.floor_log10_per_ly2());
        golden.f64("ceiling_log10_per_ly2", quantised.ceiling_log10_per_ly2());
        golden.line(&format!(
            "code_sum = {}",
            codes.iter().map(|&code| u64::from(code)).sum::<u64>()
        ));
        // Eight pixels across the picture: the centre, the bar's ends, the arms and the rim.
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
        golden!("density_map_face_on_128", &golden.finish());
        pool.shutdown().await.unwrap();
    }

    /// A map `width_px` wide of `values`, centred on the origin at 1,024 ly per pixel.
    fn raw(width_px: u16, values: &[f32]) -> RawDensityMap {
        let height_px = u16::try_from(values.len() / usize::from(width_px)).unwrap();
        RawDensityMap::new(width_px, height_px, [0.0, 0.0], 1_024.0, values.to_vec()).unwrap()
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

    /// Asserts that two floats are the same bits: the quantiser's floor and ceiling are exact.
    #[track_caller]
    fn assert_same_bits(actual: f64, expected: f64) {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} is not {expected:?}"
        );
    }

    #[test]
    fn an_eight_bit_face_on_map_quantises_as_computed_by_hand() {
        // Ceiling 0, floor −5; codes 1 + round((v + 5) ÷ 5 × 254).
        let map = raw(3, &[0.0, -2.5, -1.0, -4.0, -6.0, EMPTY]);
        let quantised = quantise_map(&map, MapView::FaceOn, CodeDepth::Eight);
        assert_same_bits(quantised.ceiling_log10_per_ly2(), 0.0);
        assert_same_bits(quantised.floor_log10_per_ly2(), -5.0);
        // 254 × 0.5 = 127; 254 × 0.8 = 203.2; 254 × 0.2 = 50.8.
        assert_eq!(quantised.bytes(), [255, 128, 204, 52, 0, 0]);
    }

    #[test]
    fn a_sixteen_bit_edge_on_map_quantises_as_computed_by_hand() {
        // Ceiling 3, floor −4; codes 1 + round((v + 4) ÷ 7 × 65,534).
        let map = raw(2, &[3.0, -0.5, -4.0, 1.0, EMPTY, -3.0]);
        let quantised = quantise_map(&map, MapView::EdgeOn, CodeDepth::Sixteen);
        assert_same_bits(quantised.ceiling_log10_per_ly2(), 3.0);
        assert_same_bits(quantised.floor_log10_per_ly2(), -4.0);
        // 65,534 × 3.5 ÷ 7 = 32,767; × 5 ÷ 7 = 46,810; × 1 ÷ 7 = 9,362.
        assert_eq!(codes(&quantised), [65_535, 32_768, 0, 46_811, 0, 9_363]);
    }

    #[test]
    fn the_ceiling_pixel_gets_the_largest_code() {
        let ceiling = 0.123_f32;
        let map = raw(2, &[-1.7, ceiling, EMPTY, -0.4]);
        for view in [MapView::FaceOn, MapView::EdgeOn] {
            for depth in [CodeDepth::Eight, CodeDepth::Sixteen] {
                let quantised = quantise_map(&map, view, depth);
                assert_same_bits(quantised.ceiling_log10_per_ly2(), f64::from(ceiling));
                let codes = codes(&quantised);
                assert_eq!(codes[1], depth.max_code(), "{view:?} at {depth:?}");
                assert!(codes[0] < codes[3] && codes[3] < codes[1], "{codes:?}");
            }
        }
    }

    #[test]
    fn a_pixel_at_the_floor_gets_zero_and_one_just_above_it_gets_one() {
        for (view, ceiling, floor) in [
            (MapView::FaceOn, 1.5_f32, -3.5_f32),
            (MapView::EdgeOn, 2.0, -5.0),
        ] {
            let map = raw(3, &[ceiling, floor, floor.next_up()]);
            for depth in [CodeDepth::Eight, CodeDepth::Sixteen] {
                let quantised = quantise_map(&map, view, depth);
                assert_same_bits(quantised.floor_log10_per_ly2(), f64::from(floor));
                assert_eq!(
                    codes(&quantised),
                    [depth.max_code(), 0, 1],
                    "{view:?} at {depth:?}"
                );
            }
        }
    }

    #[test]
    fn sixteen_bit_codes_are_little_endian() {
        // Codes 65,535, 32,768 = 0x8000, 46,811 = 0xb6db and 9,363 = 0x2493.
        let map = raw(4, &[3.0, -0.5, 1.0, -3.0]);
        let quantised = quantise_map(&map, MapView::EdgeOn, CodeDepth::Sixteen);
        assert_eq!(
            quantised.bytes(),
            [0xff, 0xff, 0x00, 0x80, 0xdb, 0xb6, 0x93, 0x24]
        );
    }

    #[test]
    fn an_empty_map_has_floor_and_ceiling_zero_and_every_code_zero() {
        let map = raw(2, &[EMPTY; 4]);
        for (depth, len) in [(CodeDepth::Eight, 4), (CodeDepth::Sixteen, 8)] {
            let quantised = quantise_map(&map, MapView::FaceOn, depth);
            assert_same_bits(quantised.floor_log10_per_ly2(), 0.0);
            assert_same_bits(quantised.ceiling_log10_per_ly2(), 0.0);
            assert_eq!(quantised.bytes(), vec![0; len]);
        }
    }

    #[test]
    fn codes_encode_as_standard_base64_with_padding() {
        let quantised = QuantisedMap {
            depth: CodeDepth::Eight,
            floor_log10_per_ly2: 0.0,
            ceiling_log10_per_ly2: 0.0,
            bytes: vec![0, 1, 2, 0xff],
        };
        assert_eq!(quantised.to_base64(), "AAEC/w==");
    }

    /// `packages/protocol/fixtures/density_map_4x2.json`, which the TypeScript decoder's tests
    /// read too.
    #[derive(Debug, Deserialize)]
    struct Fixture {
        log10_per_ly2: Vec<Option<f32>>,
        codes: Vec<u16>,
        map: DensityMap,
    }

    #[test]
    fn the_four_by_two_fixture_is_reproduced_byte_for_byte() {
        let fixture: Fixture = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/protocol/fixtures/density_map_4x2.json"
        )))
        .unwrap();
        let expected = fixture.map;
        let values = fixture
            .log10_per_ly2
            .iter()
            .map(|value| value.unwrap_or(EMPTY))
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
        let quantised = quantise_map(&raw, expected.view, depth);
        assert_eq!(codes(&quantised), fixture.codes);
        let map = DensityMap {
            universe: UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            view: expected.view,
            population: MapPopulation::All,
            width_px: raw.width_px(),
            height_px: raw.height_px(),
            centre_ly: raw.centre_ly(),
            ly_per_px: raw.ly_per_px(),
            bits: quantised.depth().bits(),
            floor_log10_per_ly2: quantised.floor_log10_per_ly2(),
            ceiling_log10_per_ly2: quantised.ceiling_log10_per_ly2(),
            data_base64: quantised.to_base64(),
        };
        assert_eq!(map, expected);
    }

    #[test]
    fn a_raw_map_refuses_what_is_not_a_map() {
        let build = |width, height, centre, ly_per_px, values: &[f32]| {
            RawDensityMap::new(width, height, centre, ly_per_px, values.to_vec())
        };
        let origin = [0.0, 0.0];
        assert_eq!(build(0, 2, origin, 1.0, &[]), Err(BuildRawMapError::Empty));
        assert_eq!(
            build(2, 2, origin, 1.0, &[0.0; 3]),
            Err(BuildRawMapError::WrongPixelCount {
                expected: 4,
                actual: 3
            })
        );
        for (centre, ly_per_px) in [
            (origin, 0.0),
            (origin, -1.0),
            (origin, f64::INFINITY),
            ([f64::NAN, 0.0], 1.0),
        ] {
            assert_eq!(
                build(1, 1, centre, ly_per_px, &[0.0]),
                Err(BuildRawMapError::InvalidGeometry)
            );
        }
        for bad in [f32::NAN, f32::INFINITY] {
            assert_eq!(
                build(3, 1, origin, 1.0, &[EMPTY, 0.0, bad]),
                Err(BuildRawMapError::InvalidValue { index: 2 })
            );
        }
        assert!(build(2, 1, [-3.0, 4.0], 0.5, &[EMPTY, -12.0]).is_ok());
    }

    #[test]
    fn a_raw_map_is_charged_for_its_grid() {
        let map = raw(4, &[0.0; 8]);
        assert_eq!(map.heap_bytes(), 8 * mem::size_of::<f32>());
        assert_eq!(
            ByteLru::<u8, RawDensityMap>::charge(&map),
            8 * mem::size_of::<f32>() + mem::size_of::<RawDensityMap>() + ENTRY_OVERHEAD_BYTES
        );
    }

    #[test]
    fn code_depth_reads_eight_and_sixteen_bits_only() {
        assert_eq!(CodeDepth::try_from(8), Ok(CodeDepth::Eight));
        assert_eq!(CodeDepth::try_from(16), Ok(CodeDepth::Sixteen));
        for bits in [0, 1, 12, 24, 32, 255] {
            let error = CodeDepth::try_from(bits).unwrap_err();
            assert_eq!(error.bits(), bits);
            assert_eq!(
                error.to_string(),
                format!("codes are 8 or 16 bits, not {bits}")
            );
        }
        for depth in [CodeDepth::Eight, CodeDepth::Sixteen] {
            assert_eq!(CodeDepth::try_from(depth.bits()), Ok(depth));
            assert_eq!(u32::from(depth.max_code()), (1_u32 << depth.bits()) - 1);
        }
    }
}
