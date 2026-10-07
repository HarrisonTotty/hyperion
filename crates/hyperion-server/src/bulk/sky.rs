//! The sky's bulk payload (rendering plan R06, R06.T10; Design note 17): the listed stars, then
//! the band's texels, little-endian, in the layouts `hyperion_protocol::sky` documents.
//!
//! The encoder takes plain values rather than the sim's types, so that the wire does not wait on
//! the census: R06.T11 converts each `SkyStar` and `BandTexel` to a [`SkyStarWire`] and a
//! [`SkyTexelWire`]. Every quantised field is rounded to nearest and clamped to its integer's
//! range, so an out-of-range value is carried as the nearest one the wire holds; a NaN is carried
//! as 0.

use axum::body::Bytes;
use hyperion_protocol::{SKY_STAR_BYTES, SKY_TEXEL_BYTES};

/// One listed star as the wire carries it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SkyStarWire {
    /// The unit direction from the observer, galactic axes.
    pub(crate) direction: [f32; 3],
    /// The distance, ly.
    pub(crate) distance_ly: f32,
    /// The apparent V after extinction, mag; carried to 0.001 mag within ±32.767.
    pub(crate) v_mag: f64,
    /// The chromaticity after reddening, r ÷ (r + g + b) and g ÷ (r + g + b), each in [0, 1].
    pub(crate) chroma: [f64; 2],
    /// The eye offset, mag: the star's own eye limit less its texel's (R06.T9.h), `eye_offsets`' from
    /// the limit map (R06.T11.c), or its colour offset alone against a scotopic background where
    /// the eye was not asked. Carried to 0.01 mag within −1.28 to +1.27, so a cool star behind
    /// several magnitudes of dust, whose offset can fall below −1.28, is kept by a view where its
    /// own limit would cull it.
    pub(crate) eye_offset_mag: f64,
    /// The view camera's band term, −2.5 log₁₀(η ÷ η☉), mag; carried in 1/32 mag within −4.0 to
    /// +3.97 (decision-camera-eta.md).
    pub(crate) camera_band_mag: f64,
}

/// One band texel as the wire carries it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SkyTexelWire {
    /// The luminance, cd/m².
    pub(crate) luminance_cd_m2: f32,
    /// The chromaticity, as a star's.
    pub(crate) chroma: [f64; 2],
    /// The eye's limit, mag, where the eye was asked; carried to 0.001 mag within ±32.767.
    pub(crate) eye_limit_mag: Option<f64>,
    /// The scotopic-to-photopic ratio ρ; carried to 10⁻⁴ within [0, 6.5535].
    pub(crate) sp_ratio: f64,
}

/// The payload and where it splits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedSky {
    /// The stars' bytes, then the band's.
    pub(crate) bytes: Bytes,
    /// The stars' bytes, first: the response's `stars_bytes`.
    pub(crate) stars_bytes: u64,
    /// The band's bytes, after them: the response's `band_bytes`.
    pub(crate) band_bytes: u64,
}

/// The stars, then the texels, as one payload.
#[must_use]
pub(crate) fn encode_sky_payload(stars: &[SkyStarWire], texels: &[SkyTexelWire]) -> EncodedSky {
    let stars_len = stars.len() * SKY_STAR_BYTES;
    let band_len = texels.len() * SKY_TEXEL_BYTES;
    let mut bytes = Vec::with_capacity(stars_len + band_len);
    for star in stars {
        for component in star.direction {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
        bytes.extend_from_slice(&star.distance_ly.to_le_bytes());
        bytes.extend_from_slice(&millimagnitudes(star.v_mag).to_le_bytes());
        for fraction in star.chroma {
            bytes.extend_from_slice(&unit_fraction(fraction).to_le_bytes());
        }
        bytes.extend_from_slice(&centimagnitudes(star.eye_offset_mag).to_le_bytes());
        bytes.extend_from_slice(&thirty_seconds(star.camera_band_mag).to_le_bytes());
    }
    for texel in texels {
        bytes.extend_from_slice(&texel.luminance_cd_m2.to_le_bytes());
        for fraction in texel.chroma {
            bytes.extend_from_slice(&unit_fraction(fraction).to_le_bytes());
        }
        let limit = texel
            .eye_limit_mag
            .map_or(i16::MIN, |limit| millimagnitudes(limit).max(i16::MIN + 1));
        bytes.extend_from_slice(&limit.to_le_bytes());
        bytes.extend_from_slice(&ratio(texel.sp_ratio).to_le_bytes());
    }
    debug_assert_eq!(bytes.len(), stars_len + band_len);
    EncodedSky {
        bytes: Bytes::from(bytes),
        stars_bytes: u64::try_from(stars_len).expect("a usize fits a u64 on every target"),
        band_bytes: u64::try_from(band_len).expect("a usize fits a u64 on every target"),
    }
}

/// `value` × `scale`, rounded to nearest and clamped to [`lo`, `hi`], NaN as 0.
#[must_use]
fn quantise(value: f64, scale: f64, lo: f64, hi: f64) -> f64 {
    let scaled = (value * scale).round();
    if scaled.is_nan() {
        0.0
    } else {
        scaled.clamp(lo, hi)
    }
}

/// A magnitude in thousandths, as an `i16`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "quantise clamps to the i16 range and rounds to a whole number first"
)]
#[must_use]
fn millimagnitudes(mag: f64) -> i16 {
    quantise(mag, 1_000.0, f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

/// A magnitude in hundredths, as an `i8`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "quantise clamps to the i8 range and rounds to a whole number first"
)]
#[must_use]
fn centimagnitudes(mag: f64) -> i8 {
    quantise(mag, 100.0, f64::from(i8::MIN), f64::from(i8::MAX)) as i8
}

/// A magnitude in units of 1/32, as an `i8`: −4.0 to +3.97, every step exact.
#[expect(
    clippy::cast_possible_truncation,
    reason = "quantise clamps to the i8 range and rounds to a whole number first"
)]
#[must_use]
fn thirty_seconds(mag: f64) -> i8 {
    quantise(mag, 32.0, f64::from(i8::MIN), f64::from(i8::MAX)) as i8
}

/// A fraction in [0, 1] in units of 1 ÷ 65,535, as a `u16`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "quantise clamps to the u16 range and rounds to a whole number first"
)]
#[must_use]
fn unit_fraction(fraction: f64) -> u16 {
    quantise(fraction, f64::from(u16::MAX), 0.0, f64::from(u16::MAX)) as u16
}

/// A ratio in units of 10⁻⁴, as a `u16`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "quantise clamps to the u16 range and rounds to a whole number first"
)]
#[must_use]
fn ratio(value: f64) -> u16 {
    quantise(value, 10_000.0, 0.0, f64::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The star whose bytes `PINNED_STAR` holds; `packages/protocol/src/sky.test.ts` decodes the
    /// same bytes.
    fn pinned_star() -> SkyStarWire {
        SkyStarWire {
            direction: [0.6, -0.8, 0.0],
            distance_ly: 8.6,
            v_mag: -1.46,
            chroma: [0.25, 0.4],
            eye_offset_mag: 0.12,
            camera_band_mag: -0.25,
        }
    }

    /// The texels whose bytes `PINNED_TEXELS` holds: one with the eye's limit, one without.
    fn pinned_texels() -> [SkyTexelWire; 2] {
        [
            SkyTexelWire {
                luminance_cd_m2: 2.5e-4,
                chroma: [0.32, 0.33],
                eye_limit_mag: Some(6.6),
                sp_ratio: 2.26,
            },
            SkyTexelWire {
                luminance_cd_m2: 1.0e-5,
                chroma: [0.0, 1.0],
                eye_limit_mag: None,
                sp_ratio: 0.0,
            },
        ]
    }

    const PINNED_STAR: [u8; 24] = [
        0x9a, 0x99, 0x19, 0x3f, // 0.6
        0xcd, 0xcc, 0x4c, 0xbf, // −0.8
        0x00, 0x00, 0x00, 0x00, // 0
        0x9a, 0x99, 0x09, 0x41, // 8.6
        0x4c, 0xfa, // −1,460 mmag
        0x00, 0x40, // 16,384 = round(0.25 × 65,535)
        0x66, 0x66, // 26,214 = 0.4 × 65,535
        0x0c, // 12 cmag
        0xf8, // −8 × 1/32 mag
    ];

    const PINNED_TEXELS: [u8; 24] = [
        0x6f, 0x12, 0x83, 0x39, // 2.5 × 10⁻⁴
        0xeb, 0x51, // 20,971 = round(0.32 × 65,535)
        0x7b, 0x54, // 21,627 = round(0.33 × 65,535)
        0xc8, 0x19, // 6,600 mmag
        0x48, 0x58, // 22,600 = 2.26 × 10⁴
        0xac, 0xc5, 0x27, 0x37, // 10⁻⁵
        0x00, 0x00, // 0
        0xff, 0xff, // 65,535
        0x00, 0x80, // i16::MIN: no eye
        0x00, 0x00, // 0
    ];

    #[test]
    fn a_star_and_texels_encode_to_the_pinned_bytes() {
        let encoded = encode_sky_payload(&[pinned_star()], &pinned_texels());
        assert_eq!(encoded.stars_bytes, 24);
        assert_eq!(encoded.band_bytes, 24);
        assert_eq!(&encoded.bytes[..24], &PINNED_STAR[..]);
        assert_eq!(&encoded.bytes[24..], &PINNED_TEXELS[..]);
    }

    #[test]
    fn the_split_sums_to_the_payload() {
        let stars = vec![pinned_star(); 5];
        let texels = vec![pinned_texels()[0]; 6 * 4 * 4];
        let encoded = encode_sky_payload(&stars, &texels);
        assert_eq!(encoded.stars_bytes, 5 * 24);
        assert_eq!(encoded.band_bytes, 6 * 16 * 12);
        assert_eq!(
            encoded.stars_bytes + encoded.band_bytes,
            u64::try_from(encoded.bytes.len()).unwrap()
        );
    }

    #[test]
    fn out_of_range_values_clamp_to_the_wire_s_range() {
        assert_eq!(millimagnitudes(40.0), i16::MAX);
        assert_eq!(millimagnitudes(-40.0), i16::MIN);
        assert_eq!(millimagnitudes(f64::NAN), 0);
        assert_eq!(centimagnitudes(2.0), i8::MAX);
        // The camera band term: −3.1 mag is −99.2 thirty-seconds, so −99, which decodes to
        // −3.09375 exactly; −4.5 saturates at −128.
        assert_eq!(thirty_seconds(-3.1), -99);
        assert!((f64::from(thirty_seconds(-3.1)) / 32.0 - (-3.093_75)).abs() < f64::EPSILON);
        assert_eq!(thirty_seconds(-4.5), i8::MIN);
        assert_eq!(thirty_seconds(4.5), i8::MAX);
        assert_eq!(unit_fraction(1.5), u16::MAX);
        assert_eq!(unit_fraction(-0.1), 0);
        assert_eq!(ratio(7.0), u16::MAX);
        // An eye limit never collides with the no-eye sentinel.
        let texel = SkyTexelWire {
            eye_limit_mag: Some(-40.0),
            ..pinned_texels()[0]
        };
        let encoded = encode_sky_payload(&[], &[texel]);
        assert_eq!(
            i16::from_le_bytes([encoded.bytes[8], encoded.bytes[9]]),
            i16::MIN + 1
        );
    }
}
