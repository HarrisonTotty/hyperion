//! The `sky` request (rendering plan R06, R06.T10): every star brighter than a view's limit as
//! seen from a point at a time, and the light of the rest as a band map, answered in bulk.
//!
//! The JSON response says what was computed: the cut, each layer's census and cap, the host
//! stars' discs and what is not modelled. The stars and the band travel as one bulk payload in
//! R03's binary frames (the crate docs lay out the frames), the stars first and then the band,
//! split by [`SkyResponse::stars_bytes`] and [`SkyResponse::band_bytes`], whose sum is the
//! manifest's `bytes`. The client decodes the payload only once it is complete.
//!
//! # Nearest first
//!
//! A sky arrives as several answers, nearest first (rendering plan R06, R06.T11.d; Design notes 11
//! and 13): the server censuses each layer shell by shell to fixed edges (125, 250 and 500 ly, then
//! 1,000 × 2^k ly; R06.T11.g) and sends a whole sky after each step, `partial_response`s, then the last as the terminal
//! `response`, [`SkyResponse::is_final`]. Each is the exact census to the radii it states, layer by
//! layer ([`SkyLayerCensusDto::complete_to_ly`] and its per-ray table), with the band holding the
//! light beyond them; each replaces the one before whole. Every machine sends the same answers, in
//! the same order, only at its own pace. A consumer of the sky's list reads `final` and
//! `complete_to_ly`: a star missing from an answer that is not final may lie beyond its radii.
//!
//! # The real tier
//!
//! Where an answer states a synthetic ceiling ([`SkyResponse::synthetic_ceiling_v`]; rendering plan
//! R13), layers C, D and E are censused only to their real boundary, at most
//! [`SkyResponse::real_limit_ly`], 2,000 ly, and the band holds the light of their stars beyond it,
//! in every answer, the final one included. Each layer states what lies beyond it: the stars
//! brighter than the cut ([`SkyLayerCensusDto::expected_beyond`]) and those brighter than the
//! ceiling ([`SkyLayerCensusDto::bright_beyond`]). RM3 draws none of them as points; rendering plan
//! R13.T7 will send them as synthetic stars.
//!
//! # The payload
//!
//! Every value is little-endian. A star is [`SKY_STAR_BYTES`], 24 (Design note 17):
//!
//! | Bytes | Field |
//! | ----- | ----- |
//! | 0–11  | the unit direction from the observer, galactic axes, three `f32` (0.012″ of rounding) |
//! | 12–15 | the distance, light-years, `f32` |
//! | 16–17 | the apparent V after extinction, millimagnitudes, `i16` |
//! | 18–21 | the chroma after reddening, two `u16` fractions of 65,535 |
//! | 22    | the eye offset, the star's own eye limit less its texel's, centimagnitudes, `i8` (−1.28 to +1.27) |
//! | 23    | the view camera's band term, −2.5 log₁₀(η ÷ η☉), units of 1/32 mag, `i8` (−4.0 to +3.97) |
//!
//! The chroma is the star's linear Rec. 709 chromaticity, r ÷ (r + g + b) and g ÷ (r + g + b),
//! each in [0, 1]; b's is one less the two. A client recovers the colour of unit luminance by
//! dividing (r, g, b) by 0.2126 r + 0.7152 g + 0.0722 b.
//!
//! The eye offset is what an eye view adds to the band texel's eye limit in the star's direction
//! to have the star's own limit (rendering plan R06, R06.T9.h; decided 2026-10-06,
//! `decision-r06-t9c-glare.md`): its colour offset against its texel's background, from its S/P
//! ratio after its own reddening, and its glare's self-exclusion, from the server's limit map
//! (R06.T11.c). Where the eye was not asked, the texels hold no eye limit, and it is the colour
//! offset alone, against a scotopic background, 2.5 log₁₀(ρ★ ÷ 2.297). It saturates at −1.28: a
//! cool star behind several magnitudes of dust can fall below that (a 2,300 K dwarf from
//! A<sub>V</sub> about 3.3, a red giant from about 7), and a view then keeps it where its own limit
//! would cull it.
//!
//! A band texel is [`SKY_TEXEL_BYTES`], 12, in the cube's face order (+X, −X, +Y, −Y, +Z, −Z on the
//! galactic axes, WebGPU's layer order), rows from the top, each face
//! [`BandSpecDto::face_texels`] square:
//!
//! | Bytes | Field |
//! | ----- | ----- |
//! | 0–3   | the luminance, cd/m², `f32` |
//! | 4–7   | the chroma, as a star's, two `u16` |
//! | 8–9   | the eye's limit at the request's field factor, millimagnitudes, `i16`; `i16::MIN` where the eye was not asked |
//! | 10–11 | the scotopic-to-photopic ratio ρ, units of 10⁻⁴, `u16` |

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::bulk::BulkManifestDto;
use crate::galaxy::MassLayer;
use crate::primitives::{GalacticPosition, SystemIdHex, UniverseIdHex, UniverseTime};

/// The deepest cut a request may ask, V 11.0, above which a narrow zoom would ask for some 10⁶
/// stars (rendering plan R06, Design note 5); a deeper exposure asks for a cone. The sim's
/// `sky::eye::MAX_CUT_V` (R06.T2) holds the same value, which the server's handler tests
/// (R06.T11.a).
pub const MAX_CUT_V: f64 = 11.0;

/// The most stars a request may list, 3 × 10⁵ (7.2 MB of payload; Design note 11), and the
/// default where a request names none.
pub const MAX_SKY_STARS: u32 = 300_000;

/// One star's bytes in the payload.
pub const SKY_STAR_BYTES: usize = 24;

/// One band texel's bytes in the payload.
pub const SKY_TEXEL_BYTES: usize = 12;

/// The bins of a host's bake spectrum, rendering plan R08's `BAKE_WAVELENGTHS_NM` (Design note 6).
pub const SKY_BAKE_BINS: usize = 15;

/// The sky as seen from a point at a time (`sky`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SkyRequest {
    /// The universe.
    pub universe: UniverseIdHex,
    /// The observer: the camera's galactic position.
    pub observer: GalacticPosition,
    /// The time, within the clock window.
    pub time: UniverseTime,
    /// The eye's parameters, where an eye view is open; then the server sets the eye's cut and
    /// returns each band texel's eye limit.
    pub eye: Option<EyeDto>,
    /// The deepest camera limit of the views open, V, finite and at most [`MAX_CUT_V`]. A request
    /// asks the eye, a camera's limit or both; one that asks neither is refused, naming this field.
    pub camera_limit_v: Option<f64>,
    /// The most stars to list, from 1 to [`MAX_SKY_STARS`]; [`MAX_SKY_STARS`] where absent.
    pub n_max: Option<u32>,
    /// An instrument's field stop, for a deep exposure of a narrow field: the census lists only the
    /// stars of the band's texels that meet the cone, and the band is complete only in them, so no
    /// star outside them glares inside it. The naked eye has no field stop, so a request with both
    /// `eye` and a cone is refused, `bad_request` naming `cone`; an instrument that wants both
    /// sends two requests (decided 2026-10-07, `decision-r06-t8k-cone.md`).
    pub cone: Option<ConeDto>,
    /// The observer's own system, whose stars are discs and are left out of the census.
    pub exclude_system: Option<SystemIdHex>,
}

/// The eye's parameters (the sim's `sky::eye::EyeObserver`, R06.T2; Design notes 2–4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EyeDto {
    /// Crumey's (2014) field factor F, 0.1 to 100, 1.4 by default (his real observers' 1.4–2.4);
    /// every limit moves by −2.5 log₁₀ F.
    pub field_factor: f64,
    /// The observer's age, years, finite and not negative, 25 by default (CIE 146:2002's glare,
    /// whose age term is fitted over about 20–80 years).
    pub age_years: f64,
    /// The eye's pigmentation p, 0 to 1.2, 0.5 by default (CIE 146:2002's glare: 0 for black eyes,
    /// 1 for light ones and 1.2 for very light blue-green).
    pub pigmentation: f64,
}

/// A cone of directions about an axis: an instrument's field stop (Design note 5; decided
/// 2026-10-07, `decision-r06-t8k-cone.md`).
///
/// Behind a field stop no light from outside the field reaches the detector. A cone's census lists
/// only the stars of the band's texels that meet it: those whose centres lie within its half-angle
/// plus the band's largest texel radius, 1.27° at 64² a face. So no star outside them glares inside
/// it, and its band holds all of the light outside them. The naked eye has no field stop, and its
/// glare reaches 90°, so it cannot ask a cone: a request with both [`SkyRequest::eye`] and a cone is
/// refused.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConeDto {
    /// The axis, a unit vector on the galactic axes.
    pub axis: [f64; 3],
    /// The half-angle, degrees, above 0 and at most 90.
    pub half_angle_deg: f64,
}

/// The sky computed: what was listed and to what limit, the band's shape, the host discs, and the
/// manifest of the payload that carries the stars and the band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SkyResponse {
    /// The universe.
    pub universe: UniverseIdHex,
    /// The time the sky is for.
    pub time: UniverseTime,
    /// The observer it is seen from.
    pub observer: GalacticPosition,
    /// The time after which the sky should be asked again: the least of one Julian year and the
    /// time at which the fastest near star would move a tenth of a pixel (Design note 13).
    pub valid_until: UniverseTime,
    /// The cut every listed star is brighter than, V: the deeper of the eye's and the camera's.
    pub cut_v: f64,
    /// The synthetic ceiling, V, at which the sky's real tier is held (rendering plan R13, Design
    /// notes 3 and 4): the server's constant V<sub>P</sub>, V 5.0 in RM3's interim, or the
    /// request's cut where the cut is brighter (decided 2026-10-09, `decision-r13-t2b-ceiling.md`).
    ///
    /// Layers C, D and E are then censused only to their real boundary, ray by ray the least of the
    /// radius beyond which under one star brighter than the ceiling is expected, the cut's own cap
    /// and [`real_limit_ly`](Self::real_limit_ly); beyond it their stars brighter than the cut are
    /// not listed, and their light is in the band. Each such layer's `cap_ly` is its real boundary at
    /// its farthest ray, the final reply's `complete_to_rays_ly` that boundary ray by ray, and its
    /// `expected_beyond` and `bright_beyond` count what lies beyond it. Absent where the reply has
    /// no real boundary: a request whose cut is at or brighter than V<sub>P</sub> and whose C, D and
    /// E caps at the cut lie within the real limit on every ray, where a ceiling would hold no ray,
    /// so that its census is R06's to the cut's own caps (rendering plan R13, R13.T2.b).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub synthetic_ceiling_v: Option<f64>,
    /// The real limit, ly: the farthest any ray of layers C, D and E is censused where the reply
    /// states a [`synthetic_ceiling_v`](Self::synthetic_ceiling_v), 2,000 ly (the sim's
    /// `sky::caps::REAL_LIMIT_LY`; decided by the owner on 2026-10-09). Absent with the ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub real_limit_ly: Option<f64>,
    /// Each layer's census, in the layers' order.
    pub census: Vec<SkyLayerCensusDto>,
    /// The stars in the payload.
    pub listed: u32,
    /// The stars brighter than the cut past the listed ones, whose light is in the band.
    pub overflow: u32,
    /// The band map's shape.
    pub band: BandSpecDto,
    /// The discs of the excluded system's stars, empty without one.
    pub hosts: Vec<HostDiscDto>,
    /// What the sky leaves out, each a label on the view.
    pub not_modelled: Vec<SkyGapDto>,
    /// The payload's manifest.
    pub bulk: BulkManifestDto,
    /// The payload's star bytes, first: `listed` × [`SKY_STAR_BYTES`].
    #[ts(type = "number")]
    pub stars_bytes: u64,
    /// The payload's band bytes, after the stars: six faces of texels × [`SKY_TEXEL_BYTES`].
    #[ts(type = "number")]
    pub band_bytes: u64,
    /// Whether this is the sky's last answer, every layer's census complete to its cap: the
    /// terminal `response` (rendering plan R06, R06.T11.d). A `partial_response`'s is false.
    #[serde(rename = "final")]
    pub is_final: bool,
}

/// One layer's census: its cap, what the census opened and kept, how far it is complete, and what it
/// could not model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SkyLayerCensusDto {
    /// The mass layer.
    pub layer: MassLayer,
    /// The radius the census searches to, ly (Design note 9): since rendering plan R06.T7.b, its
    /// farthest ray's, the cap being one radius a ray of the caps' lattice. Where the reply states
    /// a [`synthetic_ceiling_v`](SkyResponse::synthetic_ceiling_v), layers C, D and E's cap is
    /// their real boundary (rendering plan R13, Design note 3), at most
    /// [`real_limit_ly`](SkyResponse::real_limit_ly), and the final reply's
    /// [`complete_to_rays_ly`](Self::complete_to_rays_ly) is that boundary ray by ray.
    pub cap_ly: f64,
    /// The rule's bound on that radius, ly: no star of the layer beyond it can pass the cut.
    pub rule_bound_ly: f64,
    /// The stars expected brighter than the cut beyond the cap: the cap's approximation, stated.
    /// Beyond a real boundary that is the count the band holds the light of, near the Sun some
    /// thousands in each of layers C, D and E at the eye's cut (rendering plan R13, R13.T1.b).
    pub expected_beyond: f64,
    /// The stars expected brighter than the synthetic ceiling beyond the cap, stated for every
    /// layer where the reply states a ceiling ([`synthetic_ceiling_v`](SkyResponse::synthetic_ceiling_v);
    /// rendering plan R13, Design note 3): for layers C, D and E beyond their real boundary, under
    /// one where that boundary is the radius at the ceiling and more where
    /// [`real_limit_ly`](SkyResponse::real_limit_ly) holds it, near the Sun some 172 C to E at the
    /// ceiling V 5.0 (R13.T1.b); for A, B and the brown dwarfs beyond the cut's own cap, which
    /// they keep. Absent without a ceiling, and for a cap a test forces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bright_beyond: Option<f64>,
    /// The cells searched.
    pub cells: u32,
    /// The candidates whose systems were generated.
    #[ts(type = "number")]
    pub candidates_opened: u64,
    /// The stars brighter than the cut.
    #[ts(type = "number")]
    pub accepted: u64,
    /// Of those, the stars listed.
    pub listed: u32,
    /// The stars left out for want of a V magnitude (white dwarfs until plan 06's A4).
    pub without_photometry: u32,
    /// Whether the layer's feature members (clusters, the galactic centre) are absent.
    pub feature_members_absent: bool,
    /// How far this answer's census of the layer is complete, ly, in its farthest direction
    /// (rendering plan R06, R06.T11.d): the fixed shell edge its shells reach, 125, 250 or 500 ly
    /// or 1,000 × 2^k ly (R06.T11.g), while it is not [`final`](Self::is_final), and `cap_ly`
    /// once it is. Towards
    /// each ray of the caps' lattice it is complete to that ray's radius in
    /// [`complete_to_rays_ly`](Self::complete_to_rays_ly), the edge held within the cap. It lists
    /// a star, until it is final, only where the star lies nearer than that radius towards the
    /// centre of its band texel, whose ray the band holds all of the layer's light beyond; where
    /// the reply states a [`synthetic_ceiling_v`](SkyResponse::synthetic_ceiling_v), layers C, D
    /// and E list so in the final reply too (rendering plan R13, Design note 3).
    pub complete_to_ly: f64,
    /// The radius to which the layer is complete along each ray of the caps' lattice, ly, in the
    /// lattice's order: the sim's `sky::caps::CapLattice` of as many rays as the table holds
    /// (1,536), each ray's cap radius held within the shell edge reached. Empty where the layer is
    /// complete to one radius in every direction, `complete_to_ly`. Towards a direction it is
    /// complete to the largest of these over the rays within the lattice's spacing of it.
    ///
    /// Where the reply states a [`synthetic_ceiling_v`](SkyResponse::synthetic_ceiling_v), layers
    /// C, D and E's radii are their real boundary held within the edge reached. In the final reply
    /// that is the real boundary itself, ray by ray. In a reply not yet final, each ray below
    /// [`complete_to_ly`](Self::complete_to_ly) is at its real boundary, while a ray at that edge,
    /// or an empty table, leaves the real boundary between the edge and [`cap_ly`](Self::cap_ly).
    /// No table states it sooner: each reply's band holds the light beyond that reply's own radii,
    /// so a client draws by those alone (rendering plan R13, R13.T2.b).
    pub complete_to_rays_ly: Vec<f64>,
    /// Whether the layer's last shell is censused: it is complete to its cap, and lists every star
    /// of the cells it opens, as a one-shot census does, but for layers C, D and E where the reply
    /// states a [`synthetic_ceiling_v`](SkyResponse::synthetic_ceiling_v), which list only the
    /// stars within their real boundary towards each star's band texel. Layers A and B and the
    /// brown dwarfs, one shell each, are final from the first answer.
    #[serde(rename = "final")]
    pub is_final: bool,
}

/// The band map's shape: a cube on the galactic axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BandSpecDto {
    /// Each face's side, texels (64).
    pub face_texels: u16,
}

/// A part of the sky not modelled yet, each shown on the view's label block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SkyGapDto {
    /// Members of star clusters and other features (until R06.T16.a): `CLUSTERS: NOT YET
    /// MODELLED`.
    FeatureMembers,
    /// Members of the galactic centre, whose orbits are not propagated (until P09.T28): also
    /// `CLUSTERS: NOT YET MODELLED`.
    CentreMembers,
    /// White dwarfs, which have no V magnitude yet (plan 06's A4): `WHITE DWARFS: NOT YET
    /// MODELLED`.
    WhiteDwarfs,
}

/// The power-2 limb-darkening law I(μ) ÷ I(1) = 1 − c (1 − μ^α) (Maxted 2018; Design note 16).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PowerTwoDto {
    /// The law's c.
    pub c: f64,
    /// The law's α.
    pub alpha: f64,
}

/// A host star drawn as a disc (Design note 16): its size, its surface luminance per channel and
/// its limb darkening, and its colour for rendering plans R07 and R08. Every array is in the order
/// B, V, R, for the display's blue, green and red.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HostDiscDto {
    /// The star's index in its system.
    pub star: u8,
    /// The star's radius, m.
    pub radius_m: f64,
    /// The effective temperature, K.
    pub teff_k: f64,
    /// The surface gravity, log₁₀ of cm s⁻².
    pub log_g: f64,
    /// The disc's mean luminance at the surface, cd/m², per channel: R07's illuminance is
    /// π L̄ sin² ρ.
    pub mean_luminance_cd_m2: [f64; 3],
    /// The centre's luminance I(1), cd/m², per channel: the mean over the law's disc average.
    pub central_luminance_cd_m2: [f64; 3],
    /// The limb-darkening law per channel.
    pub limb: [PowerTwoDto; 3],
    /// The star's chroma, linear Rec. 709 r and g of unit luminance (`StarColour::chroma`).
    pub chroma: [f32; 2],
    /// The photopic illuminance of a V = 0 star of this spectrum over 2.54 µlx.
    pub lux_per_v0: f64,
    /// The spectrum at R08's bake wavelengths, normalised to 1 lx (Design note 6).
    pub bake_spectrum: [f64; SKY_BAKE_BINS],
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::testing::{assert_wire_form, assert_wire_strings};

    fn request() -> SkyRequest {
        SkyRequest {
            universe: UniverseIdHex::from_u64(42),
            observer: GalacticPosition {
                cell_ly: [-26_000, 0, 20],
                offset_m: [0.5, 0.25, 0.0],
            },
            time: UniverseTime {
                seconds: 3_600,
                nanos: 0,
            },
            eye: Some(EyeDto {
                field_factor: 1.4,
                age_years: 25.0,
                pigmentation: 0.5,
            }),
            camera_limit_v: Some(9.5),
            n_max: None,
            cone: Some(ConeDto {
                axis: [0.0, 0.0, 1.0],
                half_angle_deg: 2.5,
            }),
            exclude_system: Some(SystemIdHex::from_u64(0x0200_0800_2000_0000)),
        }
    }

    #[test]
    fn sky_request_wire_form() {
        assert_wire_form(
            &request(),
            json!({
                "universe": "000000000000002a",
                "observer": { "cell_ly": [-26_000, 0, 20], "offset_m": [0.5, 0.25, 0.0] },
                "time": { "seconds": 3_600, "nanos": 0 },
                "eye": { "field_factor": 1.4, "age_years": 25.0, "pigmentation": 0.5 },
                "camera_limit_v": 9.5,
                "n_max": null,
                "cone": { "axis": [0.0, 0.0, 1.0], "half_angle_deg": 2.5 },
                "exclude_system": "0200080020000000",
            }),
        );
    }

    fn host() -> HostDiscDto {
        HostDiscDto {
            star: 0,
            radius_m: 6.957e8,
            teff_k: 5_772.0,
            log_g: 4.438,
            mean_luminance_cd_m2: [1.8e9, 2.0e9, 2.1e9],
            central_luminance_cd_m2: [2.4e9, 2.5e9, 2.5e9],
            limb: [PowerTwoDto {
                c: 0.7837,
                alpha: 0.6893,
            }; 3],
            chroma: [1.0, 1.0],
            lux_per_v0: 1.0,
            bake_spectrum: [0.25; SKY_BAKE_BINS],
        }
    }

    /// A response near the Sun with no ceiling, its one layer E not yet final.
    fn response() -> SkyResponse {
        SkyResponse {
            universe: UniverseIdHex::from_u64(42),
            time: UniverseTime::default(),
            observer: GalacticPosition::default(),
            valid_until: UniverseTime {
                seconds: 31_557_600,
                nanos: 0,
            },
            cut_v: 7.96,
            synthetic_ceiling_v: None,
            real_limit_ly: None,
            census: vec![SkyLayerCensusDto {
                layer: MassLayer::E,
                cap_ly: 10_000.0,
                rule_bound_ly: 12_000.0,
                expected_beyond: 0.4,
                bright_beyond: None,
                cells: 9,
                candidates_opened: 120,
                accepted: 80,
                listed: 79,
                without_photometry: 0,
                feature_members_absent: true,
                complete_to_ly: 4_000.0,
                complete_to_rays_ly: vec![3_500.0, 4_000.0],
                is_final: false,
            }],
            listed: 2,
            overflow: 1,
            band: BandSpecDto { face_texels: 64 },
            hosts: vec![host()],
            not_modelled: vec![SkyGapDto::FeatureMembers, SkyGapDto::WhiteDwarfs],
            bulk: BulkManifestDto {
                chunks: 2,
                bytes: 294_960,
            },
            stars_bytes: 48,
            band_bytes: 294_912,
            is_final: false,
        }
    }

    /// [`response`]'s wire form, with no ceiling: no key for the ceiling, the limit or a layer's
    /// count beyond it.
    fn response_wire() -> serde_json::Value {
        let host = json!({
            "star": 0,
            "radius_m": 6.957e8,
            "teff_k": 5_772.0,
            "log_g": 4.438,
            "mean_luminance_cd_m2": [1.8e9, 2.0e9, 2.1e9],
            "central_luminance_cd_m2": [2.4e9, 2.5e9, 2.5e9],
            "limb": [
                { "c": 0.7837, "alpha": 0.6893 },
                { "c": 0.7837, "alpha": 0.6893 },
                { "c": 0.7837, "alpha": 0.6893 },
            ],
            "chroma": [1.0, 1.0],
            "lux_per_v0": 1.0,
            "bake_spectrum": vec![0.25; SKY_BAKE_BINS],
        });
        json!({
            "universe": "000000000000002a",
            "time": { "seconds": 0, "nanos": 0 },
            "observer": { "cell_ly": [0, 0, 0], "offset_m": [0.0, 0.0, 0.0] },
            "valid_until": { "seconds": 31_557_600, "nanos": 0 },
            "cut_v": 7.96,
            "census": [{
                "layer": "e",
                "cap_ly": 10_000.0,
                "rule_bound_ly": 12_000.0,
                "expected_beyond": 0.4,
                "cells": 9,
                "candidates_opened": 120,
                "accepted": 80,
                "listed": 79,
                "without_photometry": 0,
                "feature_members_absent": true,
                "complete_to_ly": 4_000.0,
                "complete_to_rays_ly": [3_500.0, 4_000.0],
                "final": false,
            }],
            "listed": 2,
            "overflow": 1,
            "band": { "face_texels": 64 },
            "hosts": [host],
            "not_modelled": ["feature_members", "white_dwarfs"],
            "bulk": { "chunks": 2, "bytes": 294_960 },
            "stars_bytes": 48,
            "band_bytes": 294_912,
            "final": false,
        })
    }

    #[test]
    fn sky_response_wire_form() {
        let response = response();
        assert_eq!(
            response.stars_bytes + response.band_bytes,
            response.bulk.bytes
        );
        assert_eq!(
            response.band_bytes,
            6 * 64 * 64 * u64::try_from(SKY_TEXEL_BYTES).unwrap()
        );
        assert_wire_form(&response, response_wire());
    }

    /// A reply at a synthetic ceiling states it, the real limit and each layer's count beyond its
    /// boundary brighter than the ceiling (rendering plan R13, R13.T2.b); one without states none
    /// of them, with no key, as a server before R13.T2.b wrote it, so that both readers keep their
    /// meaning and `PROTOCOL_VERSION` stays (R03 Design note 12; `decision-p14-t35e-wire.md`).
    #[test]
    fn a_ceiling_and_its_counts_are_optional_keys() {
        let mut at_ceiling = response();
        at_ceiling.synthetic_ceiling_v = Some(5.0);
        at_ceiling.real_limit_ly = Some(2_000.0);
        at_ceiling.census[0].bright_beyond = Some(162.4);
        let mut wire = response_wire();
        wire["synthetic_ceiling_v"] = json!(5.0);
        wire["real_limit_ly"] = json!(2_000.0);
        wire["census"][0]["bright_beyond"] = json!(162.4);
        assert_wire_form(&at_ceiling, wire);
        // Absent keys read as no ceiling, and no ceiling writes none.
        let without: SkyResponse = serde_json::from_value(response_wire()).unwrap();
        assert_eq!(
            (
                without.synthetic_ceiling_v,
                without.real_limit_ly,
                without.census[0].bright_beyond
            ),
            (None, None, None)
        );
        let written = serde_json::to_value(&without).unwrap();
        for key in ["synthetic_ceiling_v", "real_limit_ly"] {
            assert!(written.get(key).is_none(), "{key}");
        }
        assert!(written["census"][0].get("bright_beyond").is_none());
    }

    #[test]
    fn sky_gap_wire_strings() {
        assert_wire_strings(&[
            (SkyGapDto::FeatureMembers, "feature_members"),
            (SkyGapDto::CentreMembers, "centre_members"),
            (SkyGapDto::WhiteDwarfs, "white_dwarfs"),
        ]);
    }

    #[test]
    fn the_payload_sizes_are_design_note_17_s() {
        assert_eq!(SKY_STAR_BYTES, 24);
        assert_eq!(SKY_TEXEL_BYTES, 12);
        // N_max's 3 × 10⁵ stars are 7.2 MB.
        assert_eq!(
            usize::try_from(MAX_SKY_STARS).unwrap() * SKY_STAR_BYTES,
            7_200_000
        );
    }
}
