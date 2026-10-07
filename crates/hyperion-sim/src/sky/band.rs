//! The band: the light of every star a census does not list, integrated along rays from the
//! observer (rendering plan R06, R06.T9.b; Design notes 14 and 15).
//!
//! The band is a cube map on the galactic axes, [`BandSpec::face_texels`] a side, its faces in
//! WebGPU's layer order ([`CubeFace`]), one ray through each texel's centre. Each ray is marched
//! from [`FIRST_NODE_LY`] to the root cube's edge on distance nodes spaced geometrically,
//! [`BandSpec::nodes_per_decade`] a decade (twelve at [`BandSpec::STANDARD`]), with every layer's
//! complete-to radius inserted as a node of its own. Its extinction at every node comes from one
//! [`profile`] march ([`NoiseMode::Realised`], [`BAND_PROFILE_QUALITY`], with the modifiers near
//! the ray). At each node, for each density component and layer, the band takes the component's
//! density times the layer's share of its systems times their luminosity function
//! ([`LuminosityTables::get_at`], read at the node's light age through
//! [`LuminosityTables::age_for`]), dimmed by the node's extinction:
//!
//! - **within the layer's complete-to radius** ([`CompleteTo`]), the light of its stars fainter
//!   than the cut there, M<sub>V</sub> = cut − DM(d) − A<sub>V</sub>(d), since the census lists the
//!   brighter ones;
//! - **beyond it, all of the layer's light** (decided 2026-10-05, `decision-r06-census-cost.md`):
//!   so a sky that is still filling in is as bright as the final one, and the expected light of the
//!   stars beyond the caps is carried rather than dropped. The boundary is the census's
//!   (`decision-r06-census-cost-signoff.md`): until the census states its own (R06.T8.i), it is each
//!   layer's cap; until R06.T7.b, one radius a layer.
//!
//! The light along a ray is the trapezoid rule's in distance between nodes; the light nearer than
//! the first node, at most some 10⁻⁵ of a ray's, is left out. Then the census's overflow, the stars it kept
//! past `n_max`, is added to the texels its stars fall in, as points, each star's photopic
//! illuminance over its texel's solid angle. The listed stars are never in the band: a client that
//! culls one adds it to its own band layer (Design note 20), so that every star's light is counted
//! once. A census skips faint stars by mass without summing them, so the subtraction is exact in
//! expectation, not star by star. Two edges of that bookkeeping are the census's, not the band's
//! (R06's Risks, "The band's conservation"): a final census lists every star of the cells it opens,
//! some just beyond its caps, whose light the band holds too, as the caps' stated expected count
//! beyond allows; and where the eye is asked the census keeps each star to the cut plus its colour
//! offset, while the band subtracts at the cut alone, as Design note 15 states it (until R06.T8.k).
//!
//! The light is reddened by the dust in front of it (R06.T9.e; decided 2026-10-06,
//! `decision-r06-t9b-band.md`, item 3 and its addendum), through [`StarColour::reddened`]: each
//! node's light by the reddening curves of the colour table's solar point ([`solar_colour`]),
//! since the band's mix of spectra moves them by some ±0.03 a channel per magnitude of
//! A<sub>V</sub>, and each overflow star by its own. A ray keeps five sums: its photopic light
//! dimmed by the photopic transmission, its linear Rec. 709 red, green and blue light each by its
//! channel's (the solar row's signed channel through the dust over its own, the blue from unit
//! luminance before the dimming), and its scotopic light by the scotopic transmission. A texel's
//! luminance is the photopic sum, its chroma (linear Rec. 709 red and green at unit luminance, as
//! [`StarColour::chroma`]) the red and green sums over the Rec. 709 luminance of the three,
//! lifted into gamut by T3's rule where the dust has taken a channel below zero, and its S/P
//! ratio ρ the scotopic sum over the photopic, from the luminosity functions' colour of each
//! M<sub>V</sub> bin (Design note 15) and each overflow star's own reddened colour.
//!
//! The photopic luminance of a ray is K ∫ Σ ρ<sub>c</sub> s<sub>c,l</sub> ℓ<sub>c,l</sub>(d)
//! 10<sup>−0.4 k<sub>P</sub> A<sub>V</sub>(d)</sup> dd, with ρ<sub>c</sub> the component's systems per ly³,
//! s<sub>c,l</sub> the layer's share, ℓ<sub>c,l</sub> the photopic light per system (L☉,V ×
//! `lux_per_v0`) and K the photopic illuminance of one L☉,V at 10 pc times (10 pc)² in ly², so that
//! the integral is in cd m⁻² (lux per steradian): a system of light L at distance d gives L × K ÷
//! d², and a cone of solid angle Ω holds ρ Ω d² dd systems.
//!
//! Every texel is a function of its own ray and of the overflow stars that fall in it, added in the
//! census's order, so a face computed in any split of its rows gives the same bits.

use std::cmp::Ordering;
use std::ops::Range;

use core::num::NonZeroU32;

use crate::coords::{GalacticDisplacement, ROOT_HALF_WIDTH_LY, UnitVector};
use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::gas::extinction::{NoiseMode, Quality, profile};
use crate::galaxy::gas::modifiers::GasModifier;
use crate::galaxy::imf::MassBand;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::math;
use crate::time::Span;
use crate::units::consts::{
    METRES_PER_LIGHT_YEAR, RADIANS_PER_DEGREE, SECONDS_PER_JULIAN_YEAR, SOLAR_ABSOLUTE_MAGNITUDE_V,
};
use crate::units::{CandelasPerSquareMetre, LightYears, Magnitudes};

use super::caps::{CAPPED_LAYERS, LayerCap};
use super::census::{SkyCensus, SkyContext, SkyQuery};
use super::colour::{Reddened, Reddening, StarColour, lift_into_gamut, solar_colour};
use super::eye::{REFERENCE_SP_RATIO, illuminance_of_magnitude};
use super::luminosity::LuminosityTables;
use crate::tables::star_colour::LUMINANCE_RGB;

/// The quality of each ray's extinction profile (Design note 14): `Budget(256)`, the value of plan
/// 07's `SIGHTLINE_QUALITY` (P07.T10.c), as R06.T9.a fixes it.
pub const BAND_PROFILE_QUALITY: Quality =
    Quality::Budget(NonZeroU32::new(256).expect("256 is not zero"));

/// The nearest distance a ray's light is taken from, ly (Design note 14).
///
/// Nearer than it, a ray that holds all of the light would add the light of 0.01 ly of its systems:
/// about 10⁻⁶ of the ray's in the plane and 10⁻⁵ towards the poles, whose rays hold some 700 ly of
/// the disc. Within the census's radius it would add only the light of objects fainter than
/// M<sub>V</sub> 25 at a cut of V 8, beyond the tables' faintest bin.
pub const FIRST_NODE_LY: f64 = 0.01;

/// The layers a band holds light of: the census's ([`CAPPED_LAYERS`]); the rogue planets are dark.
const BAND_LAYERS: usize = CAPPED_LAYERS.len();

/// The largest face side a [`BandSpec`] takes, texels: 1,024² faces are 6.3 × 10⁶ rays, far past
/// any band a server marches.
pub const MAX_FACE_TEXELS: u16 = 1_024;

/// A face of the band's cube, on the galactic axes, in WebGPU's layer order (+X, −X, +Y, −Y, +Z,
/// −Z).
///
/// A direction maps to a face and its face coordinates as WebGPU's cube sampling maps it (WebGPU,
/// `GPUTextureViewDimension` `"cube"` and its "Cubemap faces" figure; the per-face coordinates
/// are Vulkan's Table 3, "Cube Map Face and Coordinate Selection"), as the client's `cubeTexelOf`
/// does (`view/sky/cube.ts`): row 0 is a face's top and column 0 its left. Ties between axes go
/// to x, then y, then z, as the client's do (Vulkan recommends the reverse; only a face's edge
/// can tell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CubeFace {
    /// +X, layer 0.
    PosX,
    /// −X, layer 1.
    NegX,
    /// +Y, layer 2.
    PosY,
    /// −Y, layer 3.
    NegY,
    /// +Z, galactic north, layer 4.
    PosZ,
    /// −Z, layer 5.
    NegZ,
}

impl CubeFace {
    /// Every face, in layer order.
    pub const ALL: [Self; 6] = [
        Self::PosX,
        Self::NegX,
        Self::PosY,
        Self::NegY,
        Self::PosZ,
        Self::NegZ,
    ];

    /// The face's layer in the cube, 0–5.
    #[must_use]
    pub const fn layer(self) -> u8 {
        match self {
            Self::PosX => 0,
            Self::NegX => 1,
            Self::PosY => 2,
            Self::NegY => 3,
            Self::PosZ => 4,
            Self::NegZ => 5,
        }
    }

    /// The direction through face coordinates `s` (along the columns, −1 at the left edge) and `t`
    /// (down the rows, −1 at the top), not normalised.
    #[must_use]
    const fn through(self, s: f64, t: f64) -> [f64; 3] {
        match self {
            Self::PosX => [1.0, -t, -s],
            Self::NegX => [-1.0, -t, s],
            Self::PosY => [s, 1.0, t],
            Self::NegY => [s, -1.0, -t],
            Self::PosZ => [s, -t, 1.0],
            Self::NegZ => [-s, -t, -1.0],
        }
    }

    /// The face `direction` falls on and its face coordinates `(s, t)`, each in [−1, 1]; `None`
    /// for a zero or non-finite direction. Ties between axes go to x, then y, then z, as the
    /// client's do.
    #[must_use]
    fn of_direction(direction: [f64; 3]) -> Option<(Self, f64, f64)> {
        if !direction.iter().all(|c| c.is_finite()) {
            return None;
        }
        let [x, y, z] = direction;
        let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
        let (face, across, down, major) = if ax >= ay && ax >= az {
            if x >= 0.0 {
                (Self::PosX, -z, -y, ax)
            } else {
                (Self::NegX, z, -y, ax)
            }
        } else if ay >= az {
            if y >= 0.0 {
                (Self::PosY, x, z, ay)
            } else {
                (Self::NegY, x, -z, ay)
            }
        } else if z >= 0.0 {
            (Self::PosZ, x, -y, az)
        } else {
            (Self::NegZ, -x, -y, az)
        };
        (major > 0.0).then(|| (face, across / major, down / major))
    }
}

/// The band's resolution (Design note 14): the face's side in texels and the distance nodes a
/// decade along each ray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BandSpec {
    face_texels: u16,
    nodes_per_decade: u16,
}

impl BandSpec {
    /// The server's band: 64² texels a face, 24,576 rays, of texels 1.8° wide at the faces' centres
    /// (2 ÷ 64 rad) and 1.3° on average, and twelve nodes a decade.
    pub const STANDARD: Self = Self {
        face_texels: 64,
        nodes_per_decade: 12,
    };

    /// A band of `face_texels`² texels a face and `nodes_per_decade` nodes a decade, or `None` if
    /// either is zero or the face is wider than [`MAX_FACE_TEXELS`].
    #[must_use]
    pub const fn new(face_texels: u16, nodes_per_decade: u16) -> Option<Self> {
        if face_texels == 0 || face_texels > MAX_FACE_TEXELS || nodes_per_decade == 0 {
            None
        } else {
            Some(Self {
                face_texels,
                nodes_per_decade,
            })
        }
    }

    /// The face's side, texels.
    #[must_use]
    pub const fn face_texels(&self) -> u16 {
        self.face_texels
    }

    /// The distance nodes a decade along each ray.
    #[must_use]
    pub const fn nodes_per_decade(&self) -> u16 {
        self.nodes_per_decade
    }

    /// A face coordinate of texel `index`'s centre, in (−1, 1).
    #[must_use]
    fn centre(self, index: u16) -> f64 {
        (2.0 * f64::from(index) + 1.0) / f64::from(self.face_texels) - 1.0
    }

    /// Panics unless `row` and `column` are texels of a face.
    fn assert_texel(self, row: u16, column: u16) {
        let side = self.face_texels;
        assert!(
            row < side && column < side,
            "texel ({row}, {column}) is not on a face of {side}² texels"
        );
    }

    /// The direction through the centre of the texel at `row` (from the top) and `column` (from the
    /// left) of `face`, on the galactic axes.
    ///
    /// # Panics
    ///
    /// If `row` or `column` is not below [`face_texels`](Self::face_texels).
    #[must_use]
    pub fn texel_direction(&self, face: CubeFace, row: u16, column: u16) -> UnitVector {
        self.assert_texel(row, column);
        UnitVector::from_components(face.through(self.centre(column), self.centre(row)))
            .expect("a face's texel centre is a finite, non-zero direction")
    }

    /// The texel a direction falls in, as `(face, row, column)`.
    ///
    /// `direction` is on the galactic axes, of any length; `None` for a zero or non-finite one. A
    /// direction on a face's far edge is kept on its last texel.
    #[must_use]
    pub fn texel_of(&self, direction: [f64; 3]) -> Option<(CubeFace, u16, u16)> {
        let (face, s, t) = CubeFace::of_direction(direction)?;
        let along = |a: f64| -> u16 {
            let scaled = f64::midpoint(a, 1.0) * f64::from(self.face_texels);
            let last = f64::from(self.face_texels - 1);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped to 0 through face_texels − 1, at most 1,023"
            )]
            let index = scaled.floor().clamp(0.0, last) as u16;
            index
        };
        Some((face, along(t), along(s)))
    }

    /// The solid angle of the texel at `row` and `column`, steradians: the same on every face.
    ///
    /// The exact area of the texel's projection on the unit sphere: with A(a, b) = atan2(ab,
    /// √(a² + b² + 1)), the solid angle the face region from its centre to (a, b) subtends, a
    /// texel is A(x₀, y₀) − A(x₀, y₁) − A(x₁, y₀) + A(x₁, y₁), as the client's
    /// `texelSolidAnglesSr` computes it (`view/sky/cube.ts`). The six faces' sum is 4π.
    ///
    /// # Panics
    ///
    /// If `row` or `column` is not below [`face_texels`](Self::face_texels).
    #[must_use]
    pub fn texel_solid_angle_sr(&self, row: u16, column: u16) -> f64 {
        self.assert_texel(row, column);
        let n = f64::from(self.face_texels);
        let edge = |i: u32| 2.0 * f64::from(i) / n - 1.0;
        let corner = |a: f64, b: f64| math::atan2(a * b, (a * a + b * b + 1.0).sqrt());
        let (column, row) = (u32::from(column), u32::from(row));
        let (x0, x1) = (edge(column), edge(column + 1));
        let (y0, y1) = (edge(row), edge(row + 1));
        corner(x0, y0) - corner(x0, y1) - corner(x1, y0) + corner(x1, y1)
    }
}

impl Default for BandSpec {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// How far the census behind a band is complete, per layer (decided 2026-10-05,
/// `decision-r06-census-cost.md`): within a layer's radius the band holds the light of its stars
/// fainter than the cut, beyond it all of the layer's light.
///
/// Until the census states its own (R06.T8.i), a census is complete to its plan's caps
/// ([`CompleteTo::of_caps`]); until R06.T7.b, one radius a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompleteTo {
    /// Each layer's radius, ly, in [`CAPPED_LAYERS`]' order: 0 for nowhere, +∞ for everywhere.
    radii_ly: [f64; BAND_LAYERS],
}

impl CompleteTo {
    /// Complete to each layer's cap in `caps` (a [`CensusPlan`](super::census::CensusPlan)'s), and
    /// nowhere for a layer `caps` does not hold.
    #[must_use]
    pub fn of_caps(caps: &[LayerCap]) -> Self {
        Self {
            radii_ly: CAPPED_LAYERS.map(|layer| {
                caps.iter()
                    .find(|cap| cap.layer() == layer)
                    .map_or(0.0, |cap| cap.radius().value().max(0.0))
            }),
        }
    }

    /// Complete everywhere: the band holds only the light fainter than the cut, as a band computed
    /// with no census at all needs (the eye cut's pre-pass, R06.T9.d, and the band of the stars
    /// fainter than a limit).
    #[must_use]
    pub const fn everywhere() -> Self {
        Self {
            radii_ly: [f64::INFINITY; BAND_LAYERS],
        }
    }

    /// Complete nowhere: the band holds all of the light, as before a census's first reply.
    #[must_use]
    pub const fn nowhere() -> Self {
        Self {
            radii_ly: [0.0; BAND_LAYERS],
        }
    }

    /// The radius to which `layer` is complete: 0 for nowhere (and for the rogue planets, which are
    /// dark), +∞ for everywhere.
    #[must_use]
    pub fn radius(&self, layer: Layer) -> LightYears {
        CAPPED_LAYERS
            .iter()
            .position(|&l| l == layer)
            .map_or(LightYears::ZERO, |i| LightYears::new(self.radii_ly[i]))
    }
}

/// One texel of the band (Design note 15; the wire's 12 bytes, Design note 17, before encoding).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandTexel {
    luminance: CandelasPerSquareMetre,
    chroma: [f32; 2],
    sp_ratio: f64,
    eye: Option<EyeLimit>,
}

/// The eye's limit in a texel's direction and the veil it was taken against, as the limit map sets
/// them (R06.T9.h).
#[derive(Debug, Clone, Copy, PartialEq)]
struct EyeLimit {
    limit: Magnitudes,
    /// The listed stars' veiling glare over the texel, photopic and scotopic, cd m⁻².
    veil: [f64; 2],
}

impl BandTexel {
    /// The texel of the five light sums `sums`, cd m⁻² ([`Sums`]). A texel with no light takes
    /// white's chroma, (1, 1), and the reference star's ρ ([`REFERENCE_SP_RATIO`]); one whose red,
    /// green and blue sum to no luminance takes white's chroma. A colour out of gamut, as light
    /// whose blue the dust has taken below zero can be (R06.T9.e), is lifted into it by T3's rule
    /// ([`lift_into_gamut`]).
    #[must_use]
    fn of_sums(sums: Sums) -> Self {
        debug_assert!(
            sums.iter().all(|v| v.is_finite()),
            "a band texel's sums {sums:?}"
        );
        let [luminance, red, green, blue, scotopic] = sums;
        let [yr, yg, yb] = LUMINANCE_RGB;
        let rgb_luminance = yr * red + yg * green + yb * blue;
        let (chroma, sp_ratio) = if luminance > 0.0 {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the chroma is a display value of order one, and f32 is its declared type"
            )]
            let chroma = if rgb_luminance > 0.0 {
                let [red, green, _] =
                    lift_into_gamut([red, green, blue].map(|c| c / rgb_luminance));
                [red as f32, green as f32]
            } else {
                [1.0, 1.0]
            };
            (chroma, scotopic / luminance)
        } else {
            ([1.0, 1.0], REFERENCE_SP_RATIO)
        };
        Self {
            luminance: CandelasPerSquareMetre::new(luminance.max(0.0)),
            chroma,
            sp_ratio,
            eye: None,
        }
    }

    /// The texel's photopic luminance: the light of every star the census did not list.
    #[must_use]
    pub const fn luminance(&self) -> CandelasPerSquareMetre {
        self.luminance
    }

    /// The light's chroma, linear Rec. 709 red and green at unit luminance (blue follows from the
    /// Rec. 709 luminance weights), after reddening.
    #[must_use]
    pub const fn chroma(&self) -> [f32; 2] {
        self.chroma
    }

    /// The light's S/P ratio ρ after reddening, its scotopic light over its photopic (Design note
    /// 3): the photopic-weighted mean of its stars' reddened ρ.
    #[must_use]
    pub const fn sp_ratio(&self) -> f64 {
        self.sp_ratio
    }

    /// The eye's limit in the texel's direction, which the limit map sets
    /// ([`limit_rows`](super::limits::limit_rows), R06.T9.c); `None` until then and where the eye
    /// was not asked.
    #[must_use]
    pub const fn eye_limit(&self) -> Option<Magnitudes> {
        match self.eye {
            Some(eye) => Some(eye.limit),
            None => None,
        }
    }

    /// The listed stars' veiling glare over the texel that its eye limit was taken against,
    /// photopic and scotopic, cd m⁻² (R06.T9.h): `None` where the limit map has set no limit.
    #[must_use]
    pub(crate) const fn eye_veil(&self) -> Option<[f64; 2]> {
        match self.eye {
            Some(eye) => Some(eye.veil),
            None => None,
        }
    }

    /// Sets the eye's limit in the texel's direction and the veil `veil` (photopic and scotopic, cd
    /// m⁻²) it was taken against: the limit map's.
    pub(crate) const fn set_eye_limit(&mut self, limit: Magnitudes, veil: [f64; 2]) {
        self.eye = Some(EyeLimit { limit, veil });
    }
}

#[cfg(test)]
impl BandTexel {
    /// A white texel of photopic luminance `luminance`, cd m⁻², and S/P ratio `sp_ratio`, with no
    /// eye limit: for the limit map's tests.
    #[must_use]
    pub(crate) const fn of_light(luminance: f64, sp_ratio: f64) -> Self {
        Self {
            luminance: CandelasPerSquareMetre::new(luminance),
            chroma: [1.0, 1.0],
            sp_ratio,
            eye: None,
        }
    }
}

/// A texel's or a ray's five light sums, in order: the photopic light, the linear Rec. 709 red,
/// green and blue light, and the scotopic light (the photopic light that ρ weights), each after
/// the dust in front of it (R06.T9.e).
type Sums = [f64; 5];

/// The five sums ([`Sums`]) of light whose four undimmed sums are `sums` (the luminosity
/// functions' order: the photopic light, then it times each chroma channel, then it times ρ),
/// through `dust`: the photopic light dimmed by its transmission, each channel by its own (the
/// solar row's signed channel through the dust over its own, so a channel past its change of sign
/// subtracts), the blue taken from unit luminance before the dimming, b = (1 − Y<sub>r</sub> r −
/// Y<sub>g</sub> g) ÷ Y<sub>b</sub>, and the scotopic light by the scotopic transmission.
#[must_use]
fn dimmed(sums: [f64; 4], dust: &Reddened) -> Sums {
    let [light, red, green, scotopic] = sums;
    let [yr, yg, yb] = LUMINANCE_RGB;
    let blue = (light - yr * red - yg * green) / yb;
    let [t_red, t_green, t_blue] = dust.transmission();
    [
        light * dust.photopic_transmission(),
        red * t_red,
        green * t_green,
        blue * t_blue,
        scotopic * dust.scotopic_transmission(),
    ]
}

/// The five sums ([`Sums`]) of a star of colour `colour`, apparent V `v` after an extinction of
/// `a_v`, lux: its photopic illuminance unextinguished ([`unextinguished_lux`]) times its colour,
/// reddened by its own [`StarColour::reddened`] at `a_v`: each channel by its own transmission,
/// the photopic and the scotopic light by theirs.
///
/// The channels are the star's signed light, added to the texel before any lift into gamut, since
/// light adds linearly: a channel's transmission times its unreddened colour is its reddened light
/// exactly, because no row of the colour table is out of gamut unreddened. The texel lifts the
/// total.
#[must_use]
fn point_lux(colour: &StarColour, v: Magnitudes, a_v: Magnitudes) -> Sums {
    let light = unextinguished_lux(colour, v, a_v);
    let [red, green] = colour.red_green();
    dimmed(
        [light, light * red, light * green, light * colour.sp_ratio()],
        &colour.reddened(a_v),
    )
}

/// The photopic illuminance, lux, of a star of colour `colour` and apparent V `v` after an
/// extinction of `a_v`, were there no dust: V less the extinction, through its `lux_per_v0`. Its
/// photopic illuminance is this times its [`Reddened::photopic_transmission`] at `a_v`, as the
/// band's overflow points and the limit map's glare take it.
///
/// The census's V is M<sub>V</sub> + DM + `A_V` until R06.T8.k, which makes it the star's own V
/// extinction ([`Reddened::v_extinction`]); this subtraction follows it there.
#[must_use]
pub(super) fn unextinguished_lux(colour: &StarColour, v: Magnitudes, a_v: Magnitudes) -> f64 {
    illuminance_of_magnitude(v - a_v).value() * colour.lux_per_v0()
}

/// The photopic illuminance of one L☉,V at 10 pc times (10 pc)² in ly², lux ly²: the band's K, so
/// that a system of photopic light L at d ly gives L K ÷ d² lux.
#[must_use]
fn light_to_lux_ly2() -> f64 {
    let ten_parsecs = 10.0 * LIGHT_YEARS_PER_PARSEC;
    illuminance_of_magnitude(Magnitudes::new(SOLAR_ABSOLUTE_MAGNITUDE_V)).value()
        * ten_parsecs
        * ten_parsecs
}

/// The distance modulus at `d_ly` light-years: 5 log₁₀(d ÷ 10 pc).
#[must_use]
fn distance_modulus(d_ly: f64) -> f64 {
    5.0 * math::log10(d_ly / (10.0 * LIGHT_YEARS_PER_PARSEC))
}

/// The distance from `origin_ly` along the unit vector `along` to the root cube's face, ly; 0 if
/// the ray never enters the cube ahead.
#[must_use]
fn distance_to_edge_ly(origin_ly: [f64; 3], along: [f64; 3]) -> f64 {
    let half = f64::from(ROOT_HALF_WIDTH_LY);
    let mut exit = f64::INFINITY;
    for (&from, &step) in origin_ly.iter().zip(&along) {
        if step > 0.0 {
            exit = exit.min((half - from) / step);
        } else if step < 0.0 {
            exit = exit.min((-half - from) / step);
        }
    }
    if exit.is_finite() { exit.max(0.0) } else { 0.0 }
}

/// What a band call holds for every ray: the components, each layer's share of their systems, the
/// reddening curves that dim every node, and the ray's scratch buffers.
struct Rays {
    components: Vec<ComponentId>,
    dust: Reddening,
    /// Per layer of [`CAPPED_LAYERS`], each component's share of its systems.
    shares: [[f64; MAX_COMPONENTS]; BAND_LAYERS],
    nodes: Vec<LightYears>,
    a_v: Vec<Magnitudes>,
    modifiers: Vec<GasModifier>,
}

/// One layer's photopic light sums at one node, per ly³: of its stars fainter than the cut, and of
/// all its stars, each zero where the node does not need it.
#[derive(Debug, Clone, Copy, Default)]
struct NodeLight {
    fainter: [f64; 4],
    all: [f64; 4],
}

/// Where a node lies against a layer's complete-to radius, which is a node of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// Within it: the intervals on both sides take the light fainter than the cut.
    Within,
    /// At it: the interval ending here takes the fainter light, the one starting here all of it.
    At,
    /// Beyond it: the intervals on both sides take all of the light.
    Beyond,
}

impl Side {
    /// The side of a node `distance_ly` out against a radius of `radius_ly` (+∞ for everywhere).
    #[must_use]
    fn of(distance_ly: f64, radius_ly: f64) -> Self {
        match distance_ly.total_cmp(&radius_ly) {
            Ordering::Less => Self::Within,
            Ordering::Equal => Self::At,
            Ordering::Greater => Self::Beyond,
        }
    }
}

impl Rays {
    #[must_use]
    fn new(galaxy: &Galaxy, dust: &Reddening) -> Self {
        let fields = galaxy.fields();
        let components: Vec<ComponentId> = fields.component_ids().collect();
        let mut shares = [[0.0; MAX_COMPONENTS]; BAND_LAYERS];
        for (layer_shares, &layer) in shares.iter_mut().zip(&CAPPED_LAYERS) {
            let band = MassBand::from(layer);
            for &id in &components {
                layer_shares[id.index()] =
                    galaxy.shares().component_share(band, fields.component(id));
            }
        }
        Self {
            components,
            dust: *dust,
            shares,
            nodes: Vec::new(),
            a_v: Vec::new(),
            modifiers: Vec::new(),
        }
    }

    /// A ray's distance nodes to `edge_ly`: geometric from [`FIRST_NODE_LY`], `per_decade` a
    /// decade, then the edge, and every complete-to radius between, ascending and each once.
    fn place_nodes(&mut self, edge_ly: f64, radii_ly: &[f64; BAND_LAYERS], per_decade: u16) {
        self.nodes.clear();
        let per_decade = f64::from(per_decade);
        for step in 0_u32.. {
            let distance = FIRST_NODE_LY * math::exp10(f64::from(step) / per_decade);
            if distance >= edge_ly {
                break;
            }
            self.nodes.push(LightYears::new(distance));
        }
        self.nodes.push(LightYears::new(edge_ly));
        for &radius in radii_ly {
            if radius > FIRST_NODE_LY && radius < edge_ly {
                self.nodes.push(LightYears::new(radius));
            }
        }
        self.nodes
            .sort_by(|left, right| left.value().total_cmp(&right.value()));
        self.nodes
            .dedup_by(|left, right| left.value().total_cmp(&right.value()).is_eq());
    }

    /// Layer `l`'s light at `point`, whose components' densities are `densities`, for light `ago`
    /// old: the light fainter than `limit` unless the node is beyond the layer's radius, and all of
    /// it unless the node is within it.
    #[expect(
        clippy::too_many_arguments,
        reason = "one node's inputs: the tables, the layer, the point and its densities, the limit, \
                  the light's age and the node's side of the layer's radius"
    )]
    #[must_use]
    fn layer_light(
        &self,
        tables: &LuminosityTables,
        l: usize,
        point: &PointLy,
        densities: &[f64; MAX_COMPONENTS],
        limit: Magnitudes,
        ago: Span,
        side: Side,
    ) -> NodeLight {
        let layer = CAPPED_LAYERS[l];
        let (fainter, all) = (side != Side::Beyond, side != Side::Within);
        let mut light = NodeLight::default();
        for &id in &self.components {
            let systems = densities[id.index()] * self.shares[l][id.index()];
            if systems <= 0.0 {
                continue;
            }
            let function = tables.get_at(id, layer, point);
            if fainter {
                let sums = function.colour_sums_fainter_than(limit, ago);
                for (sum, value) in light.fainter.iter_mut().zip(sums) {
                    *sum += systems * value;
                }
            }
            if all {
                let sums =
                    function.colour_sums_fainter_than(Magnitudes::new(f64::NEG_INFINITY), ago);
                for (sum, value) in light.all.iter_mut().zip(sums) {
                    *sum += systems * value;
                }
            }
        }
        light
    }

    /// The five light sums along `direction` ([`Sums`], in L☉,V × `lux_per_v0` per ly², before
    /// K), each node's light reddened by the call's dust.
    fn light_along(
        &mut self,
        galaxy: &Galaxy,
        ctx: &mut SkyContext<'_>,
        query: &SkyQuery,
        direction: UnitVector,
        radii_ly: &[f64; BAND_LAYERS],
        spec: BandSpec,
    ) -> Sums {
        let observer = query.observer();
        let origin = observer.position();
        let from = origin.to_light_years_f64();
        let along = direction.components();
        let edge_ly = distance_to_edge_ly(from, along);
        if edge_ly <= FIRST_NODE_LY {
            return [0.0; 5];
        }
        self.place_nodes(edge_ly, radii_ly, spec.nodes_per_decade());
        let end = origin
            .translated(GalacticDisplacement::new(
                along.map(|c| c * edge_ly * METRES_PER_LIGHT_YEAR),
            ))
            .expect("the ray's end lies on a face of the root cube, within the grid's range");
        self.modifiers.clear();
        ctx.modifiers
            .modifiers_near_segment(origin, &end, &mut self.modifiers);
        profile(
            galaxy.gas(),
            origin,
            direction,
            &self.nodes,
            NoiseMode::Realised,
            BAND_PROFILE_QUALITY,
            &self.modifiers,
            &mut ctx.noise,
            &mut self.a_v,
        );
        let tables = ctx.tables;
        let cut = query.cut().value();
        let mut densities = [0.0; MAX_COMPONENTS];
        let mut sums = [0.0; 5];
        // Each layer's dimmed light at the previous node, on the side its next interval takes.
        let mut before = [[0.0; 5]; BAND_LAYERS];
        let mut previous: Option<f64> = None;
        for (node, extinction) in self.nodes.iter().zip(&self.a_v) {
            let distance = node.value();
            let through = self.dust.through(*extinction);
            let point = PointLy::new(
                from[0] + distance * along[0],
                from[1] + distance * along[1],
                from[2] + distance * along[2],
            );
            galaxy.fields().densities(&point, &mut densities);
            let ago = tables.age_for(
                observer.time(),
                Span::from_seconds_f64(distance * SECONDS_PER_JULIAN_YEAR)
                    .expect("a light time within the root cube's diagonal is a span"),
            );
            let limit = Magnitudes::new(cut - distance_modulus(distance) - extinction.value());
            for (l, &radius) in radii_ly.iter().enumerate() {
                let side = Side::of(distance, radius);
                let light = self.layer_light(tables, l, &point, &densities, limit, ago, side);
                let (ending, starting) = match side {
                    Side::Within => (light.fainter, None),
                    Side::At => (light.fainter, Some(light.all)),
                    Side::Beyond => (light.all, None),
                };
                let ending = dimmed(ending, &through);
                if let Some(last) = previous {
                    let width = distance - last;
                    for ((sum, left), right) in sums.iter_mut().zip(before[l]).zip(ending) {
                        *sum += f64::midpoint(left, right) * width;
                    }
                }
                before[l] = starting.map_or(ending, |all| dimmed(all, &through));
            }
            previous = Some(distance);
        }
        sums
    }
}

/// The band's texels of rows `rows` (from the top) of `face`, appended to `out` row by row, each
/// row from its left (Design notes 14 and 15; see the [module](self) documentation).
///
/// `census` is the census the band completes and `complete_to` the radii to which it is complete:
/// within them the band holds the light fainter than `query`'s cut, beyond them all of the light,
/// and the census's overflow as points. A query with a cone is complete only within it: every ray
/// outside the cone holds all of the light. The light is reddened by the dust in front of it, each
/// node's by the solar point's ratios and each overflow star's by its own (R06.T9.e). `ctx`
/// supplies the luminosity tables, the gas modifiers and the noise cache of the rays' profiles;
/// its other fields are not read.
///
/// Each texel is a function of its own ray and of the overflow, so the texels of any split of a
/// face's rows, appended in order, are one call's over the face, bit for bit; the noise cache
/// changes the cost, never a value.
///
/// # Panics
///
/// If `rows` reaches past the face's last row.
///
/// # Examples
///
/// The band of a sky near the Sun with no census, holding the light of the stars fainter than V
/// 6.5, as a dark sky's background is measured (`no_run`: the tables take a minute or more to
/// build):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::band::{BandSpec, CompleteTo, CubeFace, band_rows};
/// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery};
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::eye::surface_brightness;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise: NoiseCache::with_capacity(1 << 16),
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let query = SkyQuery::builder(Observer::new(sun, UniverseTime::EPOCH)?, Magnitudes::new(6.5))
///     .build()?;
/// let spec = BandSpec::STANDARD;
/// let mut face = Vec::new();
/// // The face towards galactic north: a server runs a face's rows as several jobs.
/// for rows in [0..32, 32..64] {
///     band_rows(&galaxy, &mut ctx, &query, &SkyCensus::empty(), &CompleteTo::everywhere(),
///         &spec, CubeFace::PosZ, rows, &mut face);
/// }
/// let pole = surface_brightness(face[32 * 64 + 32].luminance()).ok_or("a luminance")?;
/// assert!(pole.value() > 22.0, "the pole is darker than the band");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[expect(
    clippy::too_many_arguments,
    reason = "Design note 15's inputs: the galaxy, the job's context, the query, the census and how \
              far it is complete, the band's resolution, the face, the rows and the output"
)]
pub fn band_rows(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    census: &SkyCensus,
    complete_to: &CompleteTo,
    spec: &BandSpec,
    face: CubeFace,
    rows: Range<u16>,
    out: &mut Vec<BandTexel>,
) {
    band_rows_through(
        galaxy,
        ctx,
        query,
        census,
        complete_to,
        *spec,
        face,
        rows,
        &solar_colour().reddening(),
        out,
    );
}

/// [`band_rows`], with each node's light reddened by the curves `dust`: the solar point's for the
/// band, a grey dust's for the tests' unreddened march.
#[expect(
    clippy::too_many_arguments,
    reason = "band_rows' inputs and the dust's ratios"
)]
fn band_rows_through(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    census: &SkyCensus,
    complete_to: &CompleteTo,
    spec: BandSpec,
    face: CubeFace,
    rows: Range<u16>,
    dust: &Reddening,
    out: &mut Vec<BandTexel>,
) {
    let side = spec.face_texels();
    assert!(
        rows.end <= side,
        "rows {rows:?} reach past a face of {side} rows"
    );
    if rows.is_empty() {
        return;
    }
    let width = usize::from(side);
    let to_luminance = light_to_lux_ly2();
    let cone = query.cone().map(|cone| {
        (
            cone.axis(),
            math::cos(cone.half_angle().value() * RADIANS_PER_DEGREE),
        )
    });
    let mut rays = Rays::new(galaxy, dust);
    let mut texels: Vec<Sums> = Vec::with_capacity(rows.len() * width);
    for row in rows.clone() {
        for column in 0..side {
            let direction = spec.texel_direction(face, row, column);
            let outside_cone = cone.is_some_and(|(axis, cos)| direction.dot(&axis) < cos);
            let radii = if outside_cone {
                CompleteTo::nowhere().radii_ly
            } else {
                complete_to.radii_ly
            };
            let light = rays.light_along(galaxy, ctx, query, direction, &radii, spec);
            texels.push(light.map(|v| v * to_luminance));
        }
    }
    // The overflow, as points: each star's five sums over its texel's solid angle, reddened by its
    // own colour, added in the census's order, whatever the split of the rows.
    let origin = query.observer().position();
    for star in census.overflow() {
        let Some((on, row, column)) =
            spec.texel_of(origin.displacement_to(star.apparent()).metres())
        else {
            continue;
        };
        if on != face || !rows.contains(&row) {
            continue;
        }
        let omega = spec.texel_solid_angle_sr(row, column);
        let at = usize::from(row - rows.start) * width + usize::from(column);
        let light = point_lux(star.colour(), star.v(), star.a_v());
        for (sum, lux) in texels[at].iter_mut().zip(light) {
            *sum += lux / omega;
        }
    }
    out.extend(texels.into_iter().map(BandTexel::of_sums));
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_testkit::float::{bits, bits_f32};

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::CENTIMETRES_PER_LIGHT_YEAR;
    use crate::galaxy::gas::ccm::HYDROGEN_COLUMN_PER_MAG;
    use crate::galaxy::gas::extinction::sightline;
    use crate::galaxy::gas::modifiers::{GasModifierSource, NoModifiers};
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::observe::Observer;
    use crate::sky::census::{
        CensusTallies, Cone, MAX_N_MAX, NoSkyCellCache, SkyStar, census_cell, census_plan,
        merge_census,
    };
    use crate::sky::eye::surface_brightness;
    use crate::sky::testing::{milky_way_envelope, milky_way_offsets, milky_way_tables};
    use crate::time::UniverseTime;
    use crate::units::{Degrees, HydrogenPerCm3, MagnitudesPerArcsec2};

    /// The Sun's place in the fixture, ly, as the sim's other sky tests stand.
    const SUN: [f64; 3] = [0.0, 26_000.0, 68.0];

    /// 2,000 ly above the Sun, as the caps' tests stand above it.
    const ABOVE_THE_SUN: [f64; 3] = [0.0, 26_000.0, 2_000.0];

    /// The deepest cut and the widest radius of the conservation tests' census.
    const CENSUS_CUT: f64 = 8.0;
    const CENSUS_RADIUS_LY: f64 = 200.0;

    fn context() -> SkyContext<'static> {
        SkyContext {
            tables: milky_way_tables(),
            envelope: milky_way_envelope(),
            offsets: milky_way_offsets(),
            noise: NoiseCache::with_capacity(1 << 16),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        }
    }

    fn observer_at(ly: [f64; 3]) -> Observer {
        let at = GalacticPosition::from_light_years(ly).expect("in the cube");
        Observer::new(at, UniverseTime::EPOCH).expect("an observer")
    }

    fn query_at(ly: [f64; 3], cut: f64) -> SkyQuery {
        SkyQuery::builder(observer_at(ly), Magnitudes::new(cut))
            .build()
            .expect("a valid query")
    }

    fn n(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).expect("not zero")
    }

    fn spec(face_texels: u16) -> BandSpec {
        BandSpec::new(face_texels, BandSpec::STANDARD.nodes_per_decade()).expect("a spec")
    }

    /// Complete to `radius` in every layer, as a census with every cap forced to it is.
    fn complete_within(radius: f64) -> CompleteTo {
        let caps: Vec<LayerCap> = CAPPED_LAYERS
            .iter()
            .map(|&layer| LayerCap::forced(layer, LightYears::new(radius)))
            .collect();
        CompleteTo::of_caps(&caps)
    }

    /// Every face of the band, in layer order.
    fn whole_band(
        query: &SkyQuery,
        census: &SkyCensus,
        complete_to: &CompleteTo,
        spec: BandSpec,
    ) -> Vec<BandTexel> {
        whole_band_through(
            query,
            census,
            complete_to,
            spec,
            &solar_colour().reddening(),
        )
    }

    /// Every face of the band, each node's light reddened by the curves `dust`.
    fn whole_band_through(
        query: &SkyQuery,
        census: &SkyCensus,
        complete_to: &CompleteTo,
        spec: BandSpec,
        dust: &Reddening,
    ) -> Vec<BandTexel> {
        let mut ctx = context();
        let mut out = Vec::new();
        for face in CubeFace::ALL {
            band_rows_through(
                milky_way_galaxy(),
                &mut ctx,
                query,
                census,
                complete_to,
                spec,
                face,
                0..spec.face_texels(),
                dust,
                &mut out,
            );
        }
        out
    }

    /// How a test band's nodes are dimmed.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Dust {
        /// By the solar point's ratios, as [`band_rows`] dims them.
        Reddening,
        /// Every band as V is, as the band was before R06.T9.e.
        Grey,
    }

    impl Dust {
        /// The curves that dim the nodes.
        fn curves(self) -> Reddening {
            match self {
                Self::Reddening => solar_colour().reddening(),
                Self::Grey => solar_colour().reddening().with_grey_dust(),
            }
        }
    }

    /// The band near the Sun of the stars fainter than V 6.5 (no census, complete everywhere) at
    /// 16² faces, behind `dust`: built once for the tests that read it.
    fn near_the_sun(dust: Dust) -> &'static [BandTexel] {
        static REDDENED: OnceLock<Vec<BandTexel>> = OnceLock::new();
        static GREY: OnceLock<Vec<BandTexel>> = OnceLock::new();
        let build = || {
            whole_band_through(
                &query_at(SUN, 6.5),
                &SkyCensus::empty(),
                &CompleteTo::everywhere(),
                spec(16),
                &dust.curves(),
            )
        };
        match dust {
            Dust::Reddening => REDDENED.get_or_init(build),
            Dust::Grey => GREY.get_or_init(build),
        }
    }

    /// Each texel's direction and solid angle, in the band's order.
    fn texel_geometry(spec: BandSpec) -> Vec<(UnitVector, f64)> {
        let n = spec.face_texels();
        let mut out = Vec::new();
        for face in CubeFace::ALL {
            for row in 0..n {
                for column in 0..n {
                    out.push((
                        spec.texel_direction(face, row, column),
                        spec.texel_solid_angle_sr(row, column),
                    ));
                }
            }
        }
        out
    }

    /// The band's light at the observer, lux: each texel's luminance times its solid angle.
    fn band_lux(band: &[BandTexel], spec: BandSpec) -> f64 {
        band.iter()
            .zip(texel_geometry(spec))
            .map(|(t, (_, w))| t.luminance().value() * w)
            .sum()
    }

    /// The photopic illuminance of `stars` at the observer, each reddened by its own colour, lux.
    fn stars_lux(stars: &[SkyStar]) -> f64 {
        stars
            .iter()
            .map(|s| point_lux(s.colour(), s.v(), s.a_v())[0])
            .sum()
    }

    /// The mean surface brightness of the texels whose galactic latitude lies in `latitudes`
    /// (degrees, by |b|), from their mean luminance.
    fn mean_surface_brightness(
        band: &[BandTexel],
        spec: BandSpec,
        latitudes: Range<f64>,
    ) -> MagnitudesPerArcsec2 {
        let (mut light, mut omega) = (0.0, 0.0);
        for (texel, (u, w)) in band.iter().zip(texel_geometry(spec)) {
            let b = math::asin(u.components()[2]).abs() / RADIANS_PER_DEGREE;
            if latitudes.contains(&b) {
                light += texel.luminance().value() * w;
                omega += w;
            }
        }
        surface_brightness(CandelasPerSquareMetre::new(light / omega)).expect("a luminance")
    }

    /// The S/P ratio of the light of the texels whose galactic latitude lies in `latitudes`
    /// (degrees, by |b|): their scotopic light over their photopic.
    fn mean_sp_ratio(band: &[BandTexel], spec: BandSpec, latitudes: Range<f64>) -> f64 {
        let (mut scotopic, mut photopic) = (0.0, 0.0);
        for (texel, (u, w)) in band.iter().zip(texel_geometry(spec)) {
            let b = math::asin(u.components()[2]).abs() / RADIANS_PER_DEGREE;
            if latitudes.contains(&b) {
                photopic += texel.luminance().value() * w;
                scotopic += texel.luminance().value() * texel.sp_ratio() * w;
            }
        }
        scotopic / photopic
    }

    /// A texel's linear Rec. 709 blue over its green, at unit luminance.
    fn blue_over_green(texel: &BandTexel) -> f64 {
        let [r, g] = texel.chroma().map(f64::from);
        let [yr, yg, yb] = LUMINANCE_RGB;
        (1.0 - yr * r - yg * g) / yb / g
    }

    /// Every float of `texels` as bits.
    fn texel_bits(texels: &[BandTexel]) -> Vec<(u64, [u32; 2], u64, Option<u64>)> {
        texels
            .iter()
            .map(|t| {
                (
                    bits(t.luminance().value()),
                    t.chroma().map(bits_f32),
                    bits(t.sp_ratio()),
                    t.eye_limit().map(|m| bits(m.value())),
                )
            })
            .collect()
    }

    /// The stars a census near the Sun lists to V [`CENSUS_CUT`] within [`CENSUS_RADIUS_LY`],
    /// every cap forced to it, with no eye, so that the census keeps each star to the cut alone as
    /// the band subtracts it: built once for the tests that read it.
    fn census_stars() -> &'static [SkyStar] {
        static STARS: OnceLock<Vec<SkyStar>> = OnceLock::new();
        STARS.get_or_init(|| {
            let galaxy = milky_way_galaxy();
            let query = query_at(SUN, CENSUS_CUT)
                .with_caps_forced(LightYears::new(CENSUS_RADIUS_LY))
                .expect("a forced cap");
            let mut ctx = context();
            let plan = census_plan(galaxy, ctx.tables, ctx.envelope, &query, &mut ctx.noise);
            let mut stars = Vec::new();
            for key in plan.cells() {
                census_cell(galaxy, &mut ctx, key, &query, &mut stars);
            }
            merge_census([(stars, CensusTallies::default())], n(MAX_N_MAX))
                .listed()
                .to_vec()
        })
    }

    /// The census to `cut` complete to `radius` (at most the census's own): its stars brighter
    /// than the cut within the radius, as a partial reply lists them (R06.T8.i), so that listing
    /// and band share one boundary, merged at `n_max`. A census of a brighter cut keeps a subset
    /// of a deeper one's stars, each with the same V, since no star's V depends on the cut.
    fn census_to(cut: f64, radius: f64, n_max: NonZeroU32) -> SkyCensus {
        assert!(cut <= CENSUS_CUT && radius <= CENSUS_RADIUS_LY);
        let stars: Vec<SkyStar> = census_stars()
            .iter()
            .filter(|s| s.v().value() < cut && s.distance().value() <= radius)
            .copied()
            .collect();
        merge_census([(stars, CensusTallies::default())], n_max)
    }

    /// The listed, overflow and band light near the Sun at `cut`, complete to `radius`, lux: the
    /// listed stars' light and the band's (which holds the overflow).
    fn total_light(cut: f64, radius: f64, n_max: NonZeroU32, spec: BandSpec) -> (f64, f64) {
        let census = census_to(cut, radius, n_max);
        let band = whole_band(&query_at(SUN, cut), &census, &complete_within(radius), spec);
        (stars_lux(census.listed()), band_lux(&band, spec))
    }

    /// The model's own integral along `u` for `query` with no census, complete everywhere,
    /// independently of the band's quadrature: 96 nodes a decade, each node's extinction from a
    /// full-quality sightline to it, and the luminosity functions' light and colour read through
    /// their public functions. Photopic luminance, cd m⁻².
    fn reference_luminance(query: &SkyQuery, direction: UnitVector) -> f64 {
        let galaxy = milky_way_galaxy();
        let tables = milky_way_tables();
        let origin = query.observer().position();
        let from = origin.to_light_years_f64();
        let along = direction.components();
        let edge = distance_to_edge_ly(from, along);
        let mut cache = NoiseCache::with_capacity(1 << 16);
        let components: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        let mut densities = [0.0; MAX_COMPONENTS];
        let mut nodes: Vec<f64> = (0..2_000_u32)
            .map(|step| FIRST_NODE_LY * math::exp10(f64::from(step) / 96.0))
            .take_while(|&distance| distance < edge)
            .collect();
        nodes.push(edge);
        let values: Vec<f64> = nodes
            .iter()
            .map(|&distance| {
                let end = origin
                    .translated(GalacticDisplacement::new(
                        along.map(|c| c * distance * METRES_PER_LIGHT_YEAR),
                    ))
                    .expect("in the cube");
                let extinction = sightline(
                    galaxy.gas(),
                    origin,
                    &end,
                    NoiseMode::Realised,
                    Quality::Full,
                    &[],
                    &mut cache,
                )
                .a_v()
                .value();
                let point = PointLy::new(
                    from[0] + distance * along[0],
                    from[1] + distance * along[1],
                    from[2] + distance * along[2],
                );
                galaxy.fields().densities(&point, &mut densities);
                let ago = tables.age_for(
                    query.observer().time(),
                    Span::from_seconds_f64(distance * SECONDS_PER_JULIAN_YEAR).expect("a span"),
                );
                let limit =
                    Magnitudes::new(query.cut().value() - distance_modulus(distance) - extinction);
                let mut light = 0.0;
                for &layer in &CAPPED_LAYERS {
                    let band = MassBand::from(layer);
                    for &id in &components {
                        let share = galaxy
                            .shares()
                            .component_share(band, galaxy.fields().component(id));
                        let function = tables.get_at(id, layer, &point);
                        if let Some(colour) = function.colour_fainter_than(limit, ago) {
                            light += densities[id.index()]
                                * share
                                * function.light_fainter_than(limit, ago).value()
                                * colour.lux_per_v0();
                        }
                    }
                }
                light * math::exp10(-0.4 * extinction)
            })
            .collect();
        let sum: f64 = (1..nodes.len())
            .map(|k| f64::midpoint(values[k - 1], values[k]) * (nodes[k] - nodes[k - 1]))
            .sum();
        sum * light_to_lux_ly2()
    }

    #[test]
    fn a_band_spec_holds_a_face_and_nodes() {
        assert_eq!(BandSpec::new(64, 12), Some(BandSpec::STANDARD));
        assert_eq!(BandSpec::default(), BandSpec::STANDARD);
        assert_eq!(BandSpec::new(0, 12), None);
        assert_eq!(BandSpec::new(64, 0), None);
        assert_eq!(BandSpec::new(MAX_FACE_TEXELS + 1, 12), None);
        assert!(BandSpec::new(MAX_FACE_TEXELS, 1).is_some());
        let layers: Vec<u8> = CubeFace::ALL.iter().map(|f| f.layer()).collect();
        assert_eq!(layers, [0, 1, 2, 3, 4, 5]);
    }

    /// The client's own cases (`view/sky/splatCpu.test.ts`, `cubeTexelOf`), and every texel's
    /// centre back to its texel.
    #[test]
    fn faces_map_directions_as_webgpus_cube_sampling_does() {
        let four = spec(4);
        let axes = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        for (face, axis) in CubeFace::ALL.into_iter().zip(axes) {
            assert_eq!(four.texel_of(axis), Some((face, 2, 2)), "{face:?}");
        }
        // (direction, face, column, row), as the client's test writes them.
        for (direction, face, column, row) in [
            ([1.0, 0.9, -0.9], CubeFace::PosX, 3, 0),
            ([0.9, 1.0, 0.9], CubeFace::PosY, 3, 3),
            ([0.9, -0.9, -1.0], CubeFace::NegZ, 0, 3),
            ([-1.0, 0.9, 0.9], CubeFace::NegX, 3, 0),
            ([0.9, -1.0, 0.9], CubeFace::NegY, 3, 0),
            ([-0.9, 0.9, 1.0], CubeFace::PosZ, 0, 0),
        ] {
            assert_eq!(four.texel_of(direction), Some((face, row, column)));
        }
        assert_eq!(four.texel_of([0.0; 3]), None);
        assert_eq!(four.texel_of([f64::NAN, 1.0, 0.0]), None);
        for size in [1, 3, 8, 64] {
            let spec = spec(size);
            for face in CubeFace::ALL {
                for row in 0..size {
                    for column in 0..size {
                        let u = spec.texel_direction(face, row, column);
                        assert_eq!(spec.texel_of(u.components()), Some((face, row, column)));
                    }
                }
            }
        }
    }

    #[test]
    fn the_texels_solid_angles_cover_the_sphere() {
        for size in [1, 2, 7, 64] {
            let spec = spec(size);
            let mut face = 0.0;
            for row in 0..size {
                for column in 0..size {
                    let w = spec.texel_solid_angle_sr(row, column);
                    assert!(w > 0.0, "{size}: ({row}, {column}) {w} sr");
                    // The face is symmetric under its diagonal and its mirrors.
                    let mirrored = spec.texel_solid_angle_sr(size - 1 - row, column);
                    let transposed = spec.texel_solid_angle_sr(column, row);
                    assert!(
                        (w - transposed).abs() < 1e-15,
                        "{size}: ({row}, {column}) {w} against {transposed}"
                    );
                    assert!(
                        (w - mirrored).abs() < 1e-15,
                        "{size}: ({row}, {column}) {w} against {mirrored}"
                    );
                    face += w;
                }
            }
            let sphere = 6.0 * face;
            assert!(
                (sphere / (4.0 * core::f64::consts::PI) - 1.0).abs() < 1e-12,
                "{size}: {sphere}"
            );
        }
    }

    #[test]
    fn complete_to_reads_each_layers_cap() {
        let caps = [
            LayerCap::forced(Layer::C, LightYears::new(8_193.0)),
            LayerCap::forced(Layer::A, LightYears::new(11.0)),
        ];
        let complete = CompleteTo::of_caps(&caps);
        assert_eq!(complete.radius(Layer::A), LightYears::new(11.0));
        assert_eq!(complete.radius(Layer::C), LightYears::new(8_193.0));
        for layer in [
            Layer::B,
            Layer::D,
            Layer::E,
            Layer::BrownDwarf,
            Layer::RoguePlanet,
        ] {
            assert_eq!(complete.radius(layer), LightYears::ZERO, "{layer:?}");
        }
        assert_eq!(CompleteTo::of_caps(&[]), CompleteTo::nowhere());
        for layer in CAPPED_LAYERS {
            assert!(CompleteTo::everywhere().radius(layer).value().is_infinite());
        }
        assert_eq!(
            CompleteTo::everywhere().radius(Layer::RoguePlanet),
            LightYears::ZERO
        );
    }

    /// Rows computed in any split, in any order and through a warm or a cold noise cache give
    /// one call's texels over the face, bit for bit, the overflow's points split with them.
    #[test]
    fn the_sum_over_rows_is_one_call_over_the_face() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let query = query_at(SUN, CENSUS_CUT);
        // The census within 50 ly, its overflow past the 20 brightest, to land in the faces.
        let census = census_to(CENSUS_CUT, 50.0, n(20));
        let complete = complete_within(50.0);
        assert!(census.overflow().len() > 50, "{}", census.overflow().len());
        let mut ctx = context();
        for face in CubeFace::ALL {
            let mut whole = Vec::new();
            band_rows(
                galaxy,
                &mut ctx,
                &query,
                &census,
                &complete,
                &spec,
                face,
                0..8,
                &mut whole,
            );
            assert_eq!(whole.len(), 64);
            let splits: [&[Range<u16>]; 4] = [
                &[0..3, 3..5, 5..8],
                &[0..1, 1..2, 2..3, 3..4, 4..5, 5..6, 6..7, 7..8],
                &[0..0, 0..7, 7..7, 7..8],
                &[0..4, 4..8],
            ];
            for (k, split) in splits.iter().enumerate() {
                // Every other split through a fresh cache, and its rows run last first.
                let mut fresh = context();
                let ctx = if k % 2 == 0 { &mut ctx } else { &mut fresh };
                let mut parts: Vec<(u16, Vec<BandTexel>)> = split
                    .iter()
                    .rev()
                    .map(|rows| {
                        let mut out = Vec::new();
                        band_rows(
                            galaxy,
                            ctx,
                            &query,
                            &census,
                            &complete,
                            &spec,
                            face,
                            rows.clone(),
                            &mut out,
                        );
                        assert_eq!(out.len(), rows.len() * 8);
                        (rows.start, out)
                    })
                    .collect();
                parts.sort_by_key(|(start, _)| *start);
                let joined: Vec<BandTexel> = parts.into_iter().flat_map(|(_, p)| p).collect();
                assert_eq!(
                    texel_bits(&joined),
                    texel_bits(&whole),
                    "{face:?}, split {split:?}"
                );
            }
        }
        // The overflow is in the band: the faces hold its light, and the empty census's do not.
        let with = band_lux(&whole_band(&query, &census, &complete, spec), spec);
        let without = band_lux(
            &whole_band(&query, &SkyCensus::empty(), &complete, spec),
            spec,
        );
        let overflow = stars_lux(census.overflow());
        assert!(
            ((with - without) / overflow - 1.0).abs() < 1e-9,
            "{with} − {without} against {overflow}"
        );
    }

    #[test]
    #[should_panic(expected = "reach past a face of 8 rows")]
    fn rows_past_the_face_are_refused() {
        let mut out = Vec::new();
        band_rows(
            milky_way_galaxy(),
            &mut context(),
            &query_at(SUN, 6.5),
            &SkyCensus::empty(),
            &CompleteTo::everywhere(),
            &spec(8),
            CubeFace::PosX,
            7..9,
            &mut out,
        );
    }

    /// From 2,000 ly above the Sun the disc lies below: the band is brighter towards the plane
    /// than towards the pole, and each ray, and so their ratio, is the model's own integral to
    /// within the quadrature and the profile's budget (12 nodes a decade against 96, and
    /// `Budget(256)` against full-quality sightlines: 3%).
    #[test]
    fn above_the_disc_the_band_is_brighter_towards_the_plane_by_the_models_own_integral() {
        let query = query_at(ABOVE_THE_SUN, 6.5);
        let spec = BandSpec::STANDARD;
        let mut ctx = context();
        // The texels beside each pole, 1.3° from it.
        let mut at = |face: CubeFace| {
            let mut out = Vec::new();
            band_rows(
                milky_way_galaxy(),
                &mut ctx,
                &query,
                &SkyCensus::empty(),
                &CompleteTo::everywhere(),
                &spec,
                face,
                31..32,
                &mut out,
            );
            let band = out[31].luminance().value();
            let reference = reference_luminance(&query, spec.texel_direction(face, 31, 31));
            (band, reference)
        };
        let (up, up_reference) = at(CubeFace::PosZ);
        let (down, down_reference) = at(CubeFace::NegZ);
        let mu = |b: f64| {
            surface_brightness(CandelasPerSquareMetre::new(b))
                .expect("a luminance")
                .value()
        };
        eprintln!(
            "above the Sun: towards the pole μ {:.3} (the integral's {:.3}), towards the plane μ \
             {:.3} ({:.3})",
            mu(up),
            mu(up_reference),
            mu(down),
            mu(down_reference)
        );
        assert!(down > up, "the plane is the brighter: {down} against {up}");
        for (band, reference) in [(up, up_reference), (down, down_reference)] {
            assert!(
                (band / reference - 1.0).abs() < 0.03,
                "{band} against the integral's {reference}"
            );
        }
        let (ratio, reference_ratio) = (down / up, down_reference / up_reference);
        assert!(
            (ratio / reference_ratio - 1.0).abs() < 0.03,
            "{ratio} against the integral's {reference_ratio}"
        );
    }

    /// Near the Sun the band of the stars fainter than V 6.5 is within 0.5 mag of μ 22.05 in the
    /// plane (|b| under 5°) and 24.3 at the poles (|b| over 80°), each region's mean luminance:
    /// Gaia DR3's flux sums of the stars fainter than V 6.5 by these definitions, 22.06 and 24.28,
    /// with V from G by Riello et al. 2021, Table C.2 (decided 2026-10-06,
    /// `decision-r06-t9b-band.md`, items 1 and 2). The fixture's poles are some 0.3 mag faint, its
    /// column light being 21% low (R06's Risks, "The galaxy's local light is low").
    #[test]
    fn near_the_sun_the_band_is_as_bright_as_the_stars_fainter_than_v_6_5() {
        let spec = spec(16);
        let band = near_the_sun(Dust::Reddening);
        let plane = mean_surface_brightness(band, spec, 0.0..5.0).value();
        let poles = mean_surface_brightness(band, spec, 80.0..90.1).value();
        eprintln!("near the Sun, fainter than V 6.5: plane μ {plane:.3}, poles μ {poles:.3}");
        assert!((plane - 22.05).abs() < 0.5, "the plane's μ {plane}");
        assert!((poles - 24.3).abs() < 0.5, "the poles' μ {poles}");
        assert!(band.iter().all(|t| t.eye_limit().is_none()));
        let sp: Vec<f64> = band.iter().map(BandTexel::sp_ratio).collect();
        assert!(
            sp.iter().all(|&r| (1.5..3.5).contains(&r)),
            "starlight's ρ: {:?}",
            sp.iter()
                .copied()
                .fold((f64::INFINITY, 0.0_f64), |(lo, hi), r| (
                    lo.min(r),
                    hi.max(r)
                ))
        );
    }

    /// Near the Sun the dust reddens the plane: against grey dust (every band dimmed as V is, the
    /// band before R06.T9.e), the plane's S/P ratio falls 5–20% and the poles' under 1.5%, and the
    /// plane's μ moves under 0.05 mag (`decision-r06-t9b-band.md`, item 3: the plane's
    /// unreddened ρ is 5–20% high, the poles' 1% or less).
    #[test]
    fn near_the_sun_the_dust_reddens_the_plane_and_hardly_the_poles() {
        let spec = spec(16);
        let (reddened, grey) = (near_the_sun(Dust::Reddening), near_the_sun(Dust::Grey));
        let fall = |latitudes: Range<f64>| {
            let (red, unreddened) = (
                mean_sp_ratio(reddened, spec, latitudes.clone()),
                mean_sp_ratio(grey, spec, latitudes),
            );
            (red, unreddened, 1.0 - red / unreddened)
        };
        let (plane_rho, plane_grey_rho, plane_fall) = fall(0.0..5.0);
        let (poles_rho, poles_grey_rho, poles_fall) = fall(80.0..90.1);
        let mu = |band: &[BandTexel]| mean_surface_brightness(band, spec, 0.0..5.0).value();
        let (plane_mu, plane_grey_mu) = (mu(reddened), mu(grey));
        eprintln!(
            "near the Sun, fainter than V 6.5, reddened against grey dust: the plane's ρ \
             {plane_rho:.4} against {plane_grey_rho:.4} ({:.2}% lower), μ {plane_mu:.4} against \
             {plane_grey_mu:.4}; the poles' ρ {poles_rho:.4} against {poles_grey_rho:.4} ({:.2}% \
             lower)",
            100.0 * plane_fall,
            100.0 * poles_fall
        );
        assert!(
            (0.05..0.20).contains(&plane_fall),
            "the plane's ρ falls {plane_fall}"
        );
        assert!(
            (0.0..0.015).contains(&poles_fall),
            "the poles' ρ falls {poles_fall}"
        );
        assert!(
            (plane_mu - plane_grey_mu).abs() < 0.05,
            "the plane's μ {plane_mu} against {plane_grey_mu}"
        );
        // The plane is redder too: the light-weighted linear Rec. 709 colour of its texels, at
        // unit luminance.
        let colour = |band: &[BandTexel]| {
            let [yr, yg, yb] = LUMINANCE_RGB;
            let (mut sums, mut light) = ([0.0; 3], 0.0);
            for (texel, (u, w)) in band.iter().zip(texel_geometry(spec)) {
                if u.components()[2].abs() < math::sin(5.0 * RADIANS_PER_DEGREE) {
                    let [r, g] = texel.chroma().map(f64::from);
                    let lux = texel.luminance().value() * w;
                    for (sum, c) in sums.iter_mut().zip([r, g, (1.0 - yr * r - yg * g) / yb]) {
                        *sum += lux * c;
                    }
                    light += lux;
                }
            }
            sums.map(|c| c / light)
        };
        let (red_rgb, grey_rgb) = (colour(reddened), colour(grey));
        eprintln!(
            "the plane's colour at unit luminance, reddened against grey dust: r {:.4} against \
             {:.4}, g {:.4} against {:.4}, b {:.4} against {:.4} ({:+.1}% blue)",
            red_rgb[0],
            grey_rgb[0],
            red_rgb[1],
            grey_rgb[1],
            red_rgb[2],
            grey_rgb[2],
            100.0 * (red_rgb[2] / grey_rgb[2] - 1.0)
        );
        assert!(
            red_rgb[2] < grey_rgb[2] && red_rgb[0] > grey_rgb[0],
            "{red_rgb:?} against {grey_rgb:?}"
        );
    }

    /// A star's five sums are its colour's, reddened by its own extinction: the photopic light its
    /// unextinguished illuminance times the photopic transmission, each channel its unextinguished
    /// light times its own transmission, and the scotopic its reddened ρ times the photopic; a
    /// texel holding only it has its reddened colour, lifted into gamut where the dust has taken a
    /// channel below zero (`A_V` 12); and the band adds an overflow star's sums to the faces,
    /// photopic and scotopic light alike.
    #[test]
    fn an_overflow_stars_sums_are_its_reddened_colours() {
        let census = census_to(CENSUS_CUT, 50.0, n(20));
        let [yr, yg, yb] = LUMINANCE_RGB;
        for star in census.overflow().iter().take(5) {
            let colour = star.colour();
            let unextinguished = star.v() - star.a_v();
            for a_v in [star.a_v().value(), 0.5, 2.0, 12.0] {
                // The same star behind a_v of dust.
                let a_v = Magnitudes::new(a_v);
                let v = unextinguished + a_v;
                let reddened = colour.reddened(a_v);
                let light = illuminance_of_magnitude(unextinguished).value() * colour.lux_per_v0();
                let [red, green] = colour.red_green();
                let [t_red, t_green, t_blue] = reddened.transmission();
                let photopic = light * reddened.photopic_transmission();
                let expected = [
                    photopic,
                    light * red * t_red,
                    light * green * t_green,
                    light * (1.0 - yr * red - yg * green) / yb * t_blue,
                    photopic * reddened.sp_ratio(),
                ];
                let sums = point_lux(colour, v, a_v);
                for (got, want) in sums.iter().zip(expected) {
                    // A channel past its change of sign is near zero: compared on the light's scale.
                    assert!(
                        (got - want).abs() <= 1e-9 * light,
                        "A_V {}: {sums:?} against {expected:?}",
                        a_v.value()
                    );
                }
                // A texel holding only it has its reddened ρ and colour.
                let texel = BandTexel::of_sums(sums);
                assert!((texel.sp_ratio() / reddened.sp_ratio() - 1.0).abs() < 1e-12);
                for (got, want) in texel.chroma().iter().zip(reddened.red_green()) {
                    assert!(
                        (f64::from(*got) - want).abs() < 2e-6,
                        "{got} against {want}"
                    );
                }
            }
        }
        // The band holds the overflow's photopic and scotopic light.
        let spec = spec(4);
        let query = query_at(SUN, CENSUS_CUT);
        let complete = complete_within(50.0);
        let light = |band: &[BandTexel]| {
            band.iter().zip(texel_geometry(spec)).fold(
                [0.0, 0.0],
                |[photopic, scotopic], (t, (_, w))| {
                    let lux = t.luminance().value() * w;
                    [photopic + lux, scotopic + lux * t.sp_ratio()]
                },
            )
        };
        let with = light(&whole_band(&query, &census, &complete, spec));
        let without = light(&whole_band(&query, &SkyCensus::empty(), &complete, spec));
        let overflow = census
            .overflow()
            .iter()
            .map(|s| point_lux(s.colour(), s.v(), s.a_v()))
            .fold([0.0, 0.0], |[photopic, scotopic], sums| {
                [photopic + sums[0], scotopic + sums[4]]
            });
        for k in 0..2 {
            assert!(
                ((with[k] - without[k]) / overflow[k] - 1.0).abs() < 1e-9,
                "{with:?} − {without:?} against {overflow:?}"
            );
        }
    }

    /// Lowering the cut moves the light of the stars between the cuts from the listed stars and
    /// the overflow into the band, and raising it moves it back: the listed, overflow and band
    /// light together is kept within 1% either way, as it is when `n_max` moves stars from the
    /// list into the overflow (exactly, but for rounding).
    #[test]
    fn lowering_or_raising_the_cut_conserves_the_light() {
        let spec = spec(8);
        let all = n(MAX_N_MAX);
        let r = CENSUS_RADIUS_LY;
        let mut totals = Vec::new();
        for cut in [CENSUS_CUT, 7.0, 6.0] {
            let (listed, band) = total_light(cut, r, all, spec);
            eprintln!(
                "cut {cut}, complete to {r} ly: listed {listed:.5e} lx, band {band:.5e} lx, \
                 together {:.5e} lx",
                listed + band
            );
            totals.push((cut, listed, band));
        }
        for pair in totals.windows(2) {
            let ((deep, listed_deep, band_deep), (shallow, listed_shallow, band_shallow)) =
                (pair[0], pair[1]);
            assert!(
                listed_shallow < listed_deep,
                "V {deep} → {shallow}: the list's light {listed_deep} lx → {listed_shallow} lx"
            );
            assert!(
                band_shallow > band_deep,
                "V {deep} → {shallow}: the band's light {band_deep} lx → {band_shallow} lx"
            );
            eprintln!(
                "V {deep} → {shallow}: the list lost {:.4e} lx, the band gained {:.4e} lx",
                listed_deep - listed_shallow,
                band_shallow - band_deep
            );
            let (a, b) = (listed_deep + band_deep, listed_shallow + band_shallow);
            assert!(
                (b / a - 1.0).abs() < 0.01,
                "V {deep} → {shallow}: {a} lx → {b} lx"
            );
        }
        // n_max keeps the 100 brightest: the rest overflow into the band, with no light lost.
        for (cut, listed, band) in totals {
            let (listed_100, band_100) = total_light(cut, r, n(100), spec);
            let (moved_out, moved_in) = (listed - listed_100, band_100 - band);
            assert!(moved_out > 0.0, "cut {cut}");
            assert!(
                (moved_in / moved_out - 1.0).abs() < 1e-9,
                "cut {cut}: {moved_out} lx left the list, {moved_in} lx reached the band"
            );
        }
    }

    /// The listed, overflow and band light together does not depend on how far the census is
    /// complete, within 1%: a census complete to 100 ly and one complete to 200 ly (as a partial
    /// and a later reply, R06.T8.i) give the same sky's light.
    ///
    /// Against the band of no census at all, complete nowhere, the listed stars' light falls
    /// short of the tables' expectation of it by more (printed, not asserted; R06's Risks, "The
    /// band's conservation"). The light of the stars near the Sun is a sum over few stars, whose
    /// mean the rare bright ones and the nearest ones carry: about r₁ ÷ R of each type's light
    /// within R lies within the radius r₁ that holds one expected star of it, so most
    /// realisations hold less than the mean. T5.c's realised-against-table residuals at the
    /// solar circle (C −4.4%, D −20.4%) add a systematic of the same sign.
    #[test]
    fn the_light_does_not_depend_on_the_complete_to_radius() {
        let spec = spec(8);
        let all = n(MAX_N_MAX);
        let query = query_at(SUN, CENSUS_CUT);
        let nowhere = band_lux(
            &whole_band(&query, &SkyCensus::empty(), &CompleteTo::nowhere(), spec),
            spec,
        );
        let mut totals = Vec::new();
        for radius in [100.0, CENSUS_RADIUS_LY] {
            let (listed, band) = total_light(CENSUS_CUT, radius, all, spec);
            eprintln!(
                "complete to {radius} ly: listed {listed:.5e} lx, band {band:.5e} lx, together \
                 {:.5e} lx, {:+.2}% against the band complete nowhere ({nowhere:.5e} lx); the \
                 listed hold {:.1}% of the light the band took out",
                listed + band,
                100.0 * ((listed + band) / nowhere - 1.0),
                100.0 * listed / (nowhere - band)
            );
            totals.push(listed + band);
        }
        assert!(
            (totals[1] / totals[0] - 1.0).abs() < 0.01,
            "{} lx against {} lx",
            totals[1],
            totals[0]
        );
    }

    /// A census of a cone lists stars only inside it: inside the band is the full sky's, complete
    /// as asked, and outside it holds all of the light, bit for bit.
    #[test]
    fn a_cone_is_complete_only_inside_it() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let observer = observer_at(SUN);
        let cone = Cone::new(UnitVector::X, Degrees::new(30.0)).expect("a cone");
        let narrow = SkyQuery::builder(observer, Magnitudes::new(9.0))
            .cone(cone)
            .build()
            .expect("a valid query");
        let wide = SkyQuery::builder(observer, Magnitudes::new(9.0))
            .build()
            .expect("a valid query");
        let face = |query: &SkyQuery, complete: &CompleteTo| {
            let mut out = Vec::new();
            band_rows(
                galaxy,
                &mut context(),
                query,
                &SkyCensus::empty(),
                complete,
                &spec,
                CubeFace::PosX,
                0..8,
                &mut out,
            );
            texel_bits(&out)
        };
        let coned = face(&narrow, &CompleteTo::everywhere());
        let fainter = face(&wide, &CompleteTo::everywhere());
        let everything = face(&wide, &CompleteTo::nowhere());
        let (mut inside, mut outside) = (0, 0);
        for (k, texel) in coned.iter().enumerate() {
            let row = u16::try_from(k / 8).expect("a row");
            let column = u16::try_from(k % 8).expect("a column");
            let u = spec.texel_direction(CubeFace::PosX, row, column);
            if u.dot(&UnitVector::X) >= math::cos(30.0 * RADIANS_PER_DEGREE) {
                assert_eq!(texel, &fainter[k]);
                inside += 1;
            } else {
                assert_eq!(texel, &everything[k]);
                outside += 1;
            }
        }
        assert!(
            inside > 0 && outside > 0,
            "{inside} inside, {outside} outside"
        );
        assert_ne!(fainter, everything);
    }

    /// A cloud on the +X axis from the Sun, given to the segments that head its way and pass
    /// within six of its core radii.
    struct OneCloud(GasModifier);

    impl OneCloud {
        /// A Plummer ball `distance_ly` from the Sun, of core `core_ly` and `density` cm⁻³ at its
        /// centre.
        fn at(distance_ly: f64, core_ly: f64, density: f64) -> Self {
            let centre = GalacticPosition::from_light_years([SUN[0] + distance_ly, SUN[1], SUN[2]])
                .expect("in the cube");
            Self(GasModifier::Cloud {
                centre,
                core_radius: LightYears::new(core_ly),
                central_density: HydrogenPerCm3::new(density),
                dust_per_hydrogen: 1.0,
            })
        }

        /// 300 ly away, core 50 ly and 100 cm⁻³ at its centre.
        fn near_the_sun() -> Self {
            Self::at(300.0, 50.0, 100.0)
        }
    }

    impl GasModifierSource for OneCloud {
        fn modifiers_near_segment(
            &self,
            from: &GalacticPosition,
            to: &GalacticPosition,
            out: &mut Vec<GasModifier>,
        ) {
            let GasModifier::Cloud {
                centre,
                core_radius,
                ..
            } = self.0
            else {
                return;
            };
            let ly = |v: [f64; 3]| v.map(|m| m / METRES_PER_LIGHT_YEAR);
            let (along, towards) = (
                ly(from.displacement_to(to).metres()),
                ly(from.displacement_to(&centre).metres()),
            );
            let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
            let share = (dot(towards, along) / dot(along, along)).clamp(0.0, 1.0);
            let off = [
                towards[0] - share * along[0],
                towards[1] - share * along[1],
                towards[2] - share * along[2],
            ];
            let reach = 6.0 * core_radius.value();
            if share > 0.0 && dot(off, off) < reach * reach {
                out.push(self.0);
            }
        }
    }

    /// The modifiers near each ray dim it: a cloud ahead on +X darkens that face's central texels,
    /// and the rays that pass nowhere near it keep their bits.
    #[test]
    fn the_modifiers_near_a_ray_dim_it() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let query = query_at(SUN, 6.5);
        let cloud = OneCloud::near_the_sun();
        let face = |modifiers: &dyn GasModifierSource, face: CubeFace| {
            let mut ctx = context();
            ctx.modifiers = modifiers;
            let mut out = Vec::new();
            band_rows(
                galaxy,
                &mut ctx,
                &query,
                &SkyCensus::empty(),
                &CompleteTo::everywhere(),
                &spec,
                face,
                0..8,
                &mut out,
            );
            out
        };
        let (clear, clouded) = (
            face(&NoModifiers, CubeFace::PosX),
            face(&cloud, CubeFace::PosX),
        );
        for (row, column) in [(3_usize, 3_usize), (3, 4), (4, 3), (4, 4)] {
            let k = row * 8 + column;
            let (before, after) = (clear[k].luminance().value(), clouded[k].luminance().value());
            assert!(
                after < 0.9 * before,
                "({row}, {column}): {after} behind the cloud against {before}"
            );
        }
        assert_eq!(
            texel_bits(&face(&cloud, CubeFace::NegX)),
            texel_bits(&face(&NoModifiers, CubeFace::NegX)),
            "the rays away from the cloud"
        );
    }

    /// A cloud of `A_V` 1 close ahead on +X (10 ly away, core 5 ly), with nearly all of its rays'
    /// light behind it, lowers the texels behind it in ρ and in blue by its own ratios, to 1%: the
    /// reddened march's ratio of clouded to clear, over the same of an unreddened march (grey
    /// dust, whose cloud dims every band alike), is the solar curves' t<sub>S</sub> ÷
    /// t<sub>P</sub> for ρ and t<sub>b</sub> ÷ t<sub>g</sub> for blue over green, at A each ray's
    /// own column through the cloud. The rays heading away keep their bits.
    #[test]
    fn a_cloud_reddens_the_texels_behind_it_by_its_ratios() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let query = query_at(SUN, 6.5);
        // A_V 1 through its centre: a Plummer ball's column along a whole line through its centre
        // is 4/3 of its central density times its core. From the observer, two cores from the
        // centre, a ray holds 0.992 of it.
        let core_ly = 5.0;
        let density = HYDROGEN_COLUMN_PER_MAG / (4.0 / 3.0 * core_ly * CENTIMETRES_PER_LIGHT_YEAR);
        let cloud = OneCloud::at(10.0, core_ly, density);
        let solar = solar_colour().reddening();
        let grey = solar.with_grey_dust();
        let face = |modifiers: &dyn GasModifierSource, face: CubeFace, dust: &Reddening| {
            let mut ctx = context();
            ctx.modifiers = modifiers;
            let mut out = Vec::new();
            band_rows_through(
                galaxy,
                &mut ctx,
                &query,
                &SkyCensus::empty(),
                &CompleteTo::everywhere(),
                spec,
                face,
                0..8,
                dust,
                &mut out,
            );
            out
        };
        let reddened = (
            face(&NoModifiers, CubeFace::PosX, &solar),
            face(&cloud, CubeFace::PosX, &solar),
        );
        let unreddened = (
            face(&NoModifiers, CubeFace::PosX, &grey),
            face(&cloud, CubeFace::PosX, &grey),
        );
        // A ray's own column through the cloud, as the band's profile ends: its sightline to the
        // root cube's edge with the cloud, less without it.
        let column = |u: UnitVector| {
            let origin = query.observer().position();
            let edge = distance_to_edge_ly(origin.to_light_years_f64(), u.components());
            let end = origin
                .translated(GalacticDisplacement::new(
                    u.components().map(|c| c * edge * METRES_PER_LIGHT_YEAR),
                ))
                .expect("in the cube");
            let mut cache = NoiseCache::with_capacity(1 << 16);
            let mut a_v = |modifiers: &[GasModifier]| {
                sightline(
                    galaxy.gas(),
                    origin,
                    &end,
                    NoiseMode::Realised,
                    BAND_PROFILE_QUALITY,
                    modifiers,
                    &mut cache,
                )
                .a_v()
                .value()
            };
            a_v(&[cloud.0]) - a_v(&[])
        };
        let axis = column(UnitVector::X);
        assert!(
            (axis - 1.0).abs() < 0.02,
            "the cloud's A_V on its axis {axis}"
        );
        for (row, column_index) in [(3_u16, 3_u16), (3, 4), (4, 3), (4, 4)] {
            let k = usize::from(row) * 8 + usize::from(column_index);
            let a = column(spec.texel_direction(CubeFace::PosX, row, column_index));
            let double = |f: fn(&BandTexel) -> f64| {
                (f(&reddened.1[k]) / f(&reddened.0[k]))
                    / (f(&unreddened.1[k]) / f(&unreddened.0[k]))
            };
            let (rho, blue) = (double(BandTexel::sp_ratio), double(blue_over_green));
            let behind = solar.through(Magnitudes::new(a));
            let [_, t_green, t_blue] = behind.transmission();
            let (rho_expected, blue_expected) = (
                behind.scotopic_transmission() / behind.photopic_transmission(),
                t_blue / t_green,
            );
            eprintln!(
                "({row}, {column_index}) behind A_V {a:.4}: ρ {rho:.5} (its ratios' {rho_expected:.5}), \
                 blue over green {blue:.5} ({blue_expected:.5})"
            );
            assert!(
                rho < 1.0 && blue < 1.0,
                "the cloud reddens ({row}, {column_index})"
            );
            assert!(
                (rho / rho_expected - 1.0).abs() < 0.01,
                "({row}, {column_index}): ρ {rho} against {rho_expected}"
            );
            assert!(
                (blue / blue_expected - 1.0).abs() < 0.01,
                "({row}, {column_index}): blue {blue} against {blue_expected}"
            );
        }
        assert_eq!(
            texel_bits(&face(&cloud, CubeFace::NegX, &solar)),
            texel_bits(&face(&NoModifiers, CubeFace::NegX, &solar)),
            "the rays away from the cloud"
        );
    }

    #[test]
    fn a_texel_with_no_light_is_dark_and_white() {
        let texel = BandTexel::of_sums([0.0; 5]);
        assert_eq!(bits(texel.luminance().value()), bits(0.0));
        assert_eq!(texel.chroma().map(bits_f32), [1.0_f32, 1.0].map(bits_f32));
        assert_eq!(bits(texel.sp_ratio()), bits(REFERENCE_SP_RATIO));
        assert_eq!(texel.eye_limit(), None);
    }

    #[test]
    #[should_panic(expected = "is not on a face of 8² texels")]
    fn a_texel_off_the_face_is_refused() {
        let _ = spec(8).texel_solid_angle_sr(8, 0);
    }
}
