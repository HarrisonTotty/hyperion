//! Density maps: the raw grid the map cache holds, and its quantisation for the wire.
//!
//! A map is computed once per galaxy, view, population and resolution as a [`RawDensityMap`] of
//! log₁₀ column densities, and that raw grid is what the cache keeps (plan 04, design note 12).
//! Each response quantises it to the bit depth its request asks for with [`quantise_map`], one
//! pass over the grid and cheap next to computing it, and base64-encodes the codes with
//! [`QuantisedMap::to_base64`]. The encoding, the orientation and the decode formula are those
//! documented on [`DensityMap`](hyperion_protocol::DensityMap), and
//! `packages/protocol/fixtures/density_map_4x2.json` pins them from this side and from the
//! TypeScript decoder's.

use std::error::Error;
use std::fmt;
use std::mem;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hyperion_protocol::MapView;

use crate::cache::HeapBytes;

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

#[cfg(test)]
mod tests {
    use hyperion_protocol::{DensityMap, MapPopulation, UniverseIdHex};
    use serde::Deserialize;

    use super::*;
    use crate::cache::{ByteLru, ENTRY_OVERHEAD_BYTES};

    const EMPTY: f32 = f32::NEG_INFINITY;

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
