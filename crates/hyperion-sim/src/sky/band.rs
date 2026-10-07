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
//!   than the cut there, M<sub>V</sub> = cut − DM(d) − v☉(A<sub>V</sub>) A<sub>V</sub>(d), since
//!   the census lists the brighter ones: the census cuts each star's own V, M<sub>V</sub> + DM plus
//!   its own V band's extinction, and the band takes the solar point's V band
//!   ([`Reddened::v_extinction`] of [`solar_colour`]'s curves; R06.T8.k, decided 2026-10-06,
//!   `decision-r06-t9b-band.md`, addendum item 4);
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
//! beyond allows; and the census's boundary in V is each star's own V extinction while the band's
//! is the solar point's, (v★ − v☉) A<sub>V</sub> apart, under 0.05 mag at A<sub>V</sub> 2 and some
//! 0.2 at 10, which moves a thin sliver of dimmed light between the two (addendum item 4). The
//! census keeps each star to the cut alone, with the eye or without it, and within a query's cone
//! only, so the eye's colour offset and the cone's edge are boundaries the two share (R06.T8.k).
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
//!
//! **The march and the sums** (R06.T9.f; decided 2026-10-06, `decision-r06-t9b-band.md`, item 7).
//! A request's band is marched once and summed for each of its replies, which differ only in the
//! radii to which their census is complete (R06.T8.i, T11.d). [`march_rows`] marches each ray once,
//! with every radius a reply can state (each reply's [`CompleteTo`]) a node of it, and keeps, per
//! layer and radius, the five sums of a reply complete to that radius: the light fainter than the
//! cut within the radius, run from the first node, plus all of the light beyond it, run from the
//! radius out. Both are summed node by node in distance order, so each radius's sums are those a
//! march of that radius alone takes over the same nodes, bit for bit, whatever other radii the
//! march keeps. A reply's texels therefore depend, within the quadrature, on the other radii of
//! its request, which are nodes of every ray: all of one request's replies share their nodes.
//! [`sum_rows`] then adds one reply's sums over the layers and the census's overflow points,
//! reading no profile and no luminosity table. [`band_rows`] is the march at its one reply's
//! radii, then its sum.

use std::ops::Range;

use core::num::NonZeroU32;

use crate::coords::{GalacticDisplacement, GalacticPosition, ROOT_HALF_WIDTH_LY, UnitVector};
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
    METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SOLAR_ABSOLUTE_MAGNITUDE_V,
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
/// ([`CompleteTo::of_caps`]); until R06.T7.b, one radius a layer. Each reply of a request states
/// its own, and one [`march_rows`] keeps every reply's radii (R06.T9.f).
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
                    .map_or(0.0, |cap| above_zero_or_zero(cap.radius().value()))
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

/// `radius` where it is above zero, and +0 otherwise, of either sign: a march and its sums find a
/// radius by its bits (R06.T9.f), and `f64::max` may keep −0.
#[must_use]
fn above_zero_or_zero(radius: f64) -> f64 {
    if radius > 0.0 { radius } else { 0.0 }
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

/// The five sums ([`Sums`]) of a star of colour `colour`, apparent V `v` behind a sightline of
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
    let reddened = colour.reddened(a_v);
    let light = unextinguished_lux(colour, v, &reddened);
    let [red, green] = colour.red_green();
    dimmed(
        [light, light * red, light * green, light * colour.sp_ratio()],
        &reddened,
    )
}

/// The photopic illuminance, lux, of a star of colour `colour` and apparent V `v` behind the dust
/// `reddened` (its colour's [`StarColour::reddened`] at its sightline's A<sub>V</sub>), were there
/// no dust: V less its own V band's extinction ([`Reddened::v_extinction`]), as the census measures
/// V (R06.T8.k), through its `lux_per_v0`. Its photopic illuminance is this times
/// [`Reddened::photopic_transmission`], as the band's overflow points and the limit map's glare
/// take it.
#[must_use]
pub(super) fn unextinguished_lux(colour: &StarColour, v: Magnitudes, reddened: &Reddened) -> f64 {
    illuminance_of_magnitude(v - reddened.v_extinction()).value() * colour.lux_per_v0()
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

/// The radii a march keeps a reply's sums at, and the radii its rays take as nodes.
#[derive(Debug, Clone, PartialEq)]
struct Edges {
    /// Per layer of [`CAPPED_LAYERS`], each radius to which a reply is complete, ly, ascending and
    /// each once: 0 for nowhere, +∞ for everywhere.
    kept: [Vec<f64>; BAND_LAYERS],
    /// The radii every ray inside the query's cone takes as nodes, ly, ascending and each once:
    /// every layer's kept radii (and, in the tests, more).
    nodes: Vec<f64>,
}

impl Edges {
    /// The radii of `replies`, each layer's and every layer's.
    ///
    /// # Panics
    ///
    /// If `replies` holds no reply.
    #[must_use]
    fn of_replies(replies: impl IntoIterator<Item = CompleteTo>) -> Self {
        let mut kept: [Vec<f64>; BAND_LAYERS] = Default::default();
        for reply in replies {
            for (radii, &radius) in kept.iter_mut().zip(&reply.radii_ly) {
                radii.push(radius);
            }
        }
        assert!(
            !kept[0].is_empty(),
            "a march keeps the radii of one reply at least"
        );
        for radii in &mut kept {
            ascending_once(radii);
        }
        let mut nodes: Vec<f64> = kept.iter().flatten().copied().collect();
        ascending_once(&mut nodes);
        Self { kept, nodes }
    }
}

/// Sorts `radii` ascending and keeps each once.
fn ascending_once(radii: &mut Vec<f64>) {
    radii.sort_by(f64::total_cmp);
    radii.dedup_by(|left, right| left.total_cmp(right).is_eq());
}

/// Each layer's first slot in a ray's sums, for each layer's `kept` radii in turn, then the slots
/// a ray holds.
#[must_use]
fn slot_starts<T: AsRef<[f64]>>(kept: &[T; BAND_LAYERS]) -> [usize; BAND_LAYERS + 1] {
    let mut starts = [0; BAND_LAYERS + 1];
    for (l, radii) in kept.iter().enumerate() {
        starts[l + 1] = starts[l] + radii.as_ref().len();
    }
    starts
}

/// How a ray's texel stands against its query's cone, the one place a ray's region is decided: a
/// query with a cone is complete only inside it, where its census lists stars (R06.T8.k).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// Inside the cone, or no cone: complete to each reply's radii.
    Inside,
    /// Outside the cone: complete nowhere, all of the light, whatever a reply's radii.
    Outside,
}

impl Reach {
    /// The reach of the ray along `direction` for a cone of axis and cosine `region`, if any:
    /// inside where its cosine with the axis is at least the cone's, the value
    /// [`Cone::holds`](super::census::Cone::holds) tests the census's stars against.
    #[must_use]
    fn of(region: Option<(UnitVector, f64)>, direction: UnitVector) -> Self {
        if region.is_some_and(|(axis, cos)| direction.dot(&axis) < cos) {
            Self::Outside
        } else {
            Self::Inside
        }
    }
}

/// The radius of a ray outside its query's cone: complete nowhere.
const NOWHERE_LY: [f64; 1] = [0.0];

/// A face's rows of a band, marched (R06.T9.f; see the [module](self) documentation): each ray's
/// five sums ([`Sums`]) of a reply complete to each radius the march keeps, per layer, from which
/// [`sum_rows`] gives any of those replies' texels.
///
/// Its heap is the rays' slots, 40 bytes for each layer's radius and ray
/// ([`heap_bytes`](Self::heap_bytes)): at 64² texels a face, with six layers' one radius each, some
/// 5.9 MB for the band.
#[derive(Debug, Clone, PartialEq)]
pub struct BandMarch {
    spec: BandSpec,
    face: CubeFace,
    rows: Range<u16>,
    /// The observer's position, from which the overflow's stars are placed in their texels.
    origin: GalacticPosition,
    /// Per layer of [`CAPPED_LAYERS`], the radii it keeps, ly, ascending and each once.
    kept: [Vec<f64>; BAND_LAYERS],
    /// Each ray's slots, in the band's order (row by row from the top, each row from its left):
    /// each layer's radii in turn, each slot the sums of a reply complete to that radius in that
    /// layer, in L☉,V × `lux_per_v0` per ly², before K.
    sums: Vec<Sums>,
}

impl BandMarch {
    /// The band's resolution.
    #[must_use]
    pub const fn spec(&self) -> BandSpec {
        self.spec
    }

    /// The face.
    #[must_use]
    pub const fn face(&self) -> CubeFace {
        self.face
    }

    /// The rows marched, from the top.
    #[must_use]
    pub fn rows(&self) -> Range<u16> {
        self.rows.clone()
    }

    /// Whether the march keeps every layer's radius of `complete_to`, so that [`sum_rows`] can sum
    /// a reply complete to it.
    #[must_use]
    pub fn holds(&self, complete_to: &CompleteTo) -> bool {
        self.kept
            .iter()
            .zip(&complete_to.radii_ly)
            .all(|(radii, radius)| radii.binary_search_by(|r| r.total_cmp(radius)).is_ok())
    }

    /// The bytes the march holds on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.sums.capacity() * size_of::<Sums>()
            + self
                .kept
                .iter()
                .map(|radii| radii.capacity() * size_of::<f64>())
                .sum::<usize>()
    }

    /// The slots a ray holds.
    #[must_use]
    fn slots(&self) -> usize {
        slot_starts(&self.kept)[BAND_LAYERS]
    }

    /// The slot of each layer's radius of `complete_to` in a ray's sums.
    ///
    /// # Panics
    ///
    /// If the march does not keep a layer's radius.
    #[must_use]
    fn slots_of(&self, complete_to: &CompleteTo) -> [usize; BAND_LAYERS] {
        let starts = slot_starts(&self.kept);
        std::array::from_fn(|l| {
            let radius = complete_to.radii_ly[l];
            let at = self.kept[l]
                .binary_search_by(|r| r.total_cmp(&radius))
                .unwrap_or_else(|_| {
                    panic!(
                        "complete to {radius} ly in layer {:?} is not a radius the march keeps: \
                         {:?}",
                        CAPPED_LAYERS[l], self.kept[l]
                    )
                });
            starts[l] + at
        })
    }
}

/// What a march holds for every ray: the components, each layer's share of their systems, the
/// reddening curves that dim every node, and the ray's scratch buffers.
struct Rays {
    components: Vec<ComponentId>,
    dust: Reddening,
    /// Per layer of [`CAPPED_LAYERS`], each component's share of its systems.
    shares: [[f64; MAX_COMPONENTS]; BAND_LAYERS],
    nodes: Vec<LightYears>,
    a_v: Vec<Magnitudes>,
    modifiers: Vec<GasModifier>,
    /// A ray's slots: the light fainter than the cut within each radius, from the first node.
    within: Vec<Sums>,
    /// A ray's slots: all of the light beyond each radius, from the radius out.
    beyond: Vec<Sums>,
}

/// One layer's photopic light sums at one node, per ly³: of its stars fainter than the cut, and of
/// all its stars, each zero where the node does not need it.
#[derive(Debug, Clone, Copy, Default)]
struct NodeLight {
    fainter: [f64; 4],
    all: [f64; 4],
}

/// Which of a layer's light a node needs: the light fainter than the cut, at the nodes out to the
/// layer's farthest radius, and all of it, at the nodes from its nearest radius on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Needs {
    fainter: bool,
    all: bool,
}

impl Needs {
    /// What a node `distance_ly` out needs of a layer that keeps the radii `radii_ly`, ascending
    /// and not empty.
    #[must_use]
    fn of(distance_ly: f64, radii_ly: &[f64]) -> Self {
        let nearest = radii_ly.first().expect("a layer keeps one radius at least");
        let farthest = radii_ly.last().expect("a layer keeps one radius at least");
        Self {
            fainter: distance_ly <= *farthest,
            all: distance_ly >= *nearest,
        }
    }
}

/// The trapezoid rule's sums over an interval of `width` between the sums `left` and `right`.
#[must_use]
fn trapezoid(left: Sums, right: Sums, width: f64) -> Sums {
    std::array::from_fn(|k| f64::midpoint(left[k], right[k]) * width)
}

/// Adds `more` to `sums`, sum by sum.
fn add_to(sums: &mut Sums, more: Sums) {
    for (sum, value) in sums.iter_mut().zip(more) {
        *sum += value;
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
            within: Vec::new(),
            beyond: Vec::new(),
        }
    }

    /// A ray's distance nodes to `edge_ly`: geometric from [`FIRST_NODE_LY`], `per_decade` a
    /// decade, then the edge, and every radius of `radii_ly` between, ascending and each once.
    fn place_nodes(&mut self, edge_ly: f64, radii_ly: &[f64], per_decade: u16) {
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
    /// old: the light fainter than `limit` and all of it, each where `needs` asks it.
    #[expect(
        clippy::too_many_arguments,
        reason = "one node's inputs: the tables, the layer, the point and its densities, the limit, \
                  the light's age and what the node needs of the layer"
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
        needs: Needs,
    ) -> NodeLight {
        let layer = CAPPED_LAYERS[l];
        let mut light = NodeLight::default();
        for &id in &self.components {
            let systems = densities[id.index()] * self.shares[l][id.index()];
            if systems <= 0.0 {
                continue;
            }
            let function = tables.get_at(id, layer, point);
            if needs.fainter {
                let sums = function.colour_sums_fainter_than(limit, ago);
                for (sum, value) in light.fainter.iter_mut().zip(sums) {
                    *sum += systems * value;
                }
            }
            if needs.all {
                let sums =
                    function.colour_sums_fainter_than(Magnitudes::new(f64::NEG_INFINITY), ago);
                for (sum, value) in light.all.iter_mut().zip(sums) {
                    *sum += systems * value;
                }
            }
        }
        light
    }

    /// Marches the ray along `direction` into `out`, its slots in `edges`' order (each layer's kept
    /// radii in turn): for each, the five sums ([`Sums`], in L☉,V × `lux_per_v0` per ly², before
    /// K) of the light fainter than the cut out to the radius plus all of the light beyond it, each
    /// node's light reddened by the call's dust. A ray outside the cone (`reach`) is complete
    /// nowhere, so each of its slots holds all of its layer's light.
    ///
    /// Within a layer the light fainter than the cut is summed from the first node, and each
    /// radius's light beyond it from the radius out, interval by interval in distance order, so a
    /// radius's slot is the bits a march keeping it alone takes over the same nodes.
    #[expect(
        clippy::too_many_arguments,
        reason = "one ray's inputs: the galaxy, the job's context, the query, the direction, the \
                  radii, the ray's reach, the band's resolution and its slots"
    )]
    fn march(
        &mut self,
        galaxy: &Galaxy,
        ctx: &mut SkyContext<'_>,
        query: &SkyQuery,
        direction: UnitVector,
        edges: &Edges,
        reach: Reach,
        spec: BandSpec,
        out: &mut [Sums],
    ) {
        // The radii this ray keeps per layer, and the radii its nodes take.
        let (kept, node_radii): ([&[f64]; BAND_LAYERS], &[f64]) = match reach {
            Reach::Inside => (
                std::array::from_fn(|l| edges.kept[l].as_slice()),
                &edges.nodes,
            ),
            Reach::Outside => ([NOWHERE_LY.as_slice(); BAND_LAYERS], &[]),
        };
        let starts = slot_starts(&kept);
        self.within.clear();
        self.within.resize(starts[BAND_LAYERS], [0.0; 5]);
        self.beyond.clear();
        self.beyond.resize(starts[BAND_LAYERS], [0.0; 5]);
        self.march_slots(galaxy, ctx, query, direction, &kept, node_radii, spec);
        let ours = slot_starts(&edges.kept);
        for l in 0..BAND_LAYERS {
            let theirs = &mut out[ours[l]..ours[l + 1]];
            let slots = starts[l]..starts[l + 1];
            let mut totals = self.within[slots.clone()]
                .iter()
                .zip(&self.beyond[slots])
                .map(|(within, beyond)| std::array::from_fn(|k| within[k] + beyond[k]));
            match reach {
                Reach::Inside => {
                    for (slot, total) in theirs.iter_mut().zip(totals) {
                        *slot = total;
                    }
                }
                Reach::Outside => {
                    let nowhere: Sums = totals.next().expect("a layer keeps one radius");
                    theirs.fill(nowhere);
                }
            }
        }
    }

    /// The node-by-node march of one ray into [`Rays::within`] and [`Rays::beyond`], whose slots
    /// are each layer's `kept` radii in turn and start at zero; `node_radii` are the radii taken
    /// as nodes. Each interval lies within or beyond each radius, since a radius is a node, or
    /// nearer than the first, or past the last. A radius's slot stops taking the fainter light,
    /// and starts taking all of it, at the first interval beyond it.
    #[expect(
        clippy::too_many_arguments,
        reason = "one ray's inputs: the galaxy, the job's context, the query, the direction, the \
                  radii kept and taken as nodes, and the band's resolution"
    )]
    fn march_slots(
        &mut self,
        galaxy: &Galaxy,
        ctx: &mut SkyContext<'_>,
        query: &SkyQuery,
        direction: UnitVector,
        kept: &[&[f64]; BAND_LAYERS],
        node_radii: &[f64],
        spec: BandSpec,
    ) {
        let observer = query.observer();
        let origin = observer.position();
        let from = origin.to_light_years_f64();
        let along = direction.components();
        let edge_ly = distance_to_edge_ly(from, along);
        if edge_ly <= FIRST_NODE_LY {
            return;
        }
        self.place_nodes(edge_ly, node_radii, spec.nodes_per_decade());
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
        let starts = slot_starts(kept);
        let mut densities = [0.0; MAX_COMPONENTS];
        // Per layer: the fainter light run from the first node, how many of its radii the intervals
        // so far have passed (each slot holding the fainter light as it stood there), and the
        // dimmed light at the previous node.
        let mut fainter = [[0.0; 5]; BAND_LAYERS];
        let mut passed = [0_usize; BAND_LAYERS];
        let mut before = [([0.0; 5], [0.0; 5]); BAND_LAYERS];
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
            // The census cuts each star's own V; the band, the solar point's (R06.T8.k).
            let limit =
                Magnitudes::new(cut - distance_modulus(distance) - through.v_extinction().value());
            for (l, radii) in kept.iter().enumerate() {
                let needs = Needs::of(distance, radii);
                let light = self.layer_light(tables, l, &point, &densities, limit, ago, needs);
                let here = (
                    if needs.fainter {
                        dimmed(light.fainter, &through)
                    } else {
                        [0.0; 5]
                    },
                    if needs.all {
                        dimmed(light.all, &through)
                    } else {
                        [0.0; 5]
                    },
                );
                if let Some(last) = previous {
                    let width = distance - last;
                    let slots = starts[l]..starts[l + 1];
                    // The radii this interval lies beyond take the fainter light as it stands.
                    while passed[l] < radii.len() && radii[passed[l]] < distance {
                        self.within[slots.start + passed[l]] = fainter[l];
                        passed[l] += 1;
                    }
                    if needs.fainter {
                        add_to(&mut fainter[l], trapezoid(before[l].0, here.0, width));
                    }
                    if passed[l] > 0 {
                        let all = trapezoid(before[l].1, here.1, width);
                        for beyond in &mut self.beyond[slots.start..slots.start + passed[l]] {
                            add_to(beyond, all);
                        }
                    }
                }
                before[l] = here;
            }
            previous = Some(distance);
        }
        // The radii at or past the ray's last node take all of the fainter light.
        for (l, radii) in kept.iter().enumerate() {
            for slot in &mut self.within[starts[l] + passed[l]..starts[l] + radii.len()] {
                *slot = fainter[l];
            }
        }
    }
}

/// Marches rows `rows` (from the top) of `face` once, keeping each reply of `replies` (R06.T9.f;
/// see the [module](self) documentation): every radius to which a reply is complete, in any layer,
/// is a node of each ray, and each layer's sums are kept at each of its radii, so that
/// [`sum_rows`] gives any of the replies' texels from the march alone.
///
/// The light is that of [`band_rows`]: within a layer's radius, the light fainter than `query`'s
/// cut at M<sub>V</sub> = cut − DM − v☉ A<sub>V</sub> (R06.T8.k), beyond it all of the layer's
/// light, each node's light reddened by the solar point's curves (R06.T9.e). A query with a cone is
/// complete only within it: every ray outside the cone holds all of the light, whatever a reply's
/// radii. `ctx` supplies the luminosity tables, the gas modifiers and the noise cache of the rays'
/// profiles; its other fields are not read.
///
/// Each ray is a function of its own direction, so the marches of any split of a face's rows, each
/// summed, give the texels of one march over the face, bit for bit; the noise cache changes the
/// cost, never a value.
///
/// # Panics
///
/// If `replies` is empty, or `rows` reaches past the face's last row.
///
/// # Examples
///
/// A request's replies, nearest first, are each complete to a further radius: one march serves
/// them all (`no_run`: the tables take a minute or more to build).
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::band::{BandSpec, CompleteTo, CubeFace, march_rows, sum_rows};
/// use hyperion_sim::sky::caps::{CAPPED_LAYERS, LayerCap};
/// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery};
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{LightYears, Magnitudes};
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
/// let query = SkyQuery::builder(Observer::new(sun, UniverseTime::EPOCH)?, Magnitudes::new(8.0))
///     .build()?;
/// // Two replies: complete to 500 ly, then to 1,000 ly, in every layer.
/// let within = |ly: f64| {
///     let caps: Vec<LayerCap> =
///         CAPPED_LAYERS.iter().map(|&l| LayerCap::forced(l, LightYears::new(ly))).collect();
///     CompleteTo::of_caps(&caps)
/// };
/// let (first, last) = (within(500.0), within(1_000.0));
/// let spec = BandSpec::STANDARD;
/// let march = march_rows(&galaxy, &mut ctx, &query, [first, last], &spec, CubeFace::PosZ, 0..64);
/// assert!(march.holds(&first) && march.holds(&last));
/// // Each reply's band from the one march, with that reply's census (empty here).
/// for reply in [first, last] {
///     let mut face = Vec::new();
///     sum_rows(&march, &SkyCensus::empty(), &reply, &mut face);
///     assert_eq!(face.len(), 64 * 64);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn march_rows(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    replies: impl IntoIterator<Item = CompleteTo>,
    spec: &BandSpec,
    face: CubeFace,
    rows: Range<u16>,
) -> BandMarch {
    march_rows_through(
        galaxy,
        ctx,
        query,
        Edges::of_replies(replies),
        *spec,
        face,
        rows,
        &solar_colour().reddening(),
    )
}

/// [`march_rows`] at `edges`, with each node's light reddened by the curves `dust`: the solar
/// point's for the band, a grey dust's for the tests' unreddened march.
#[expect(
    clippy::too_many_arguments,
    reason = "march_rows' inputs, its radii resolved, and the dust's ratios"
)]
fn march_rows_through(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    edges: Edges,
    spec: BandSpec,
    face: CubeFace,
    rows: Range<u16>,
    dust: &Reddening,
) -> BandMarch {
    let side = spec.face_texels();
    assert!(
        rows.end <= side,
        "rows {rows:?} reach past a face of {side} rows"
    );
    debug_assert!(
        edges.kept.iter().flatten().all(|radius| edges
            .nodes
            .binary_search_by(|r| r.total_cmp(radius))
            .is_ok()),
        "every radius a march keeps is a node of its rays: {edges:?}"
    );
    let slots = slot_starts(&edges.kept)[BAND_LAYERS];
    let mut sums = vec![[0.0; 5]; rows.len() * usize::from(side) * slots];
    if !sums.is_empty() {
        let region = query
            .cone()
            .map(|cone| (cone.axis(), cone.cos_half_angle()));
        let mut rays = Rays::new(galaxy, dust);
        let texels = rows
            .clone()
            .flat_map(|row| (0..side).map(move |column| (row, column)));
        for ((row, column), out) in texels.zip(sums.chunks_exact_mut(slots)) {
            let direction = spec.texel_direction(face, row, column);
            let reach = Reach::of(region, direction);
            rays.march(galaxy, ctx, query, direction, &edges, reach, spec, out);
        }
    }
    BandMarch {
        spec,
        face,
        rows,
        origin: *query.observer().position(),
        kept: edges.kept,
        sums,
    }
}

/// The texels of a reply complete to `complete_to`, from `march`, appended to `out` row by row,
/// each row from its left (R06.T9.f; see the [module](self) documentation): each ray's sums at
/// each layer's radius, over the layers, then `census`'s overflow, the stars it kept past `n_max`,
/// as points in their texels, each reddened by its own colour (R06.T9.e). It reads no profile and
/// no luminosity table.
///
/// `census` is the reply's census, of the march's query, and `complete_to` the radii to which it is
/// complete. A march of any split of a face's rows, each summed, gives one march's texels over the
/// face, bit for bit, and a reply's texels are those of a march that keeps that reply alone over
/// the same nodes.
///
/// # Panics
///
/// If the march does not keep a layer's radius of `complete_to` ([`BandMarch::holds`]).
pub fn sum_rows(
    march: &BandMarch,
    census: &SkyCensus,
    complete_to: &CompleteTo,
    out: &mut Vec<BandTexel>,
) {
    let slots = march.slots_of(complete_to);
    if march.sums.is_empty() {
        return;
    }
    let (spec, face, rows) = (march.spec, march.face, &march.rows);
    let width = usize::from(spec.face_texels());
    let to_luminance = light_to_lux_ly2();
    let mut texels: Vec<Sums> = march
        .sums
        .chunks_exact(march.slots())
        .map(|ray| {
            let mut light = [0.0; 5];
            for &slot in &slots {
                add_to(&mut light, ray[slot]);
            }
            light.map(|v| v * to_luminance)
        })
        .collect();
    // The overflow, as points: each star's five sums over its texel's solid angle, reddened by its
    // own colour, added in the census's order, whatever the split of the rows.
    for star in census.overflow() {
        let Some((on, row, column)) =
            spec.texel_of(march.origin.displacement_to(star.apparent()).metres())
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

/// The band's texels of rows `rows` (from the top) of `face`, appended to `out` row by row, each
/// row from its left (Design notes 14 and 15; see the [module](self) documentation): the march of
/// those rows at `complete_to`'s radii ([`march_rows`]), then its sum ([`sum_rows`]).
///
/// `census` is the census the band completes and `complete_to` the radii to which it is complete:
/// within them the band holds the light fainter than `query`'s cut, beyond them all of the light,
/// and the census's overflow as points. A query with a cone is complete only within it: every ray
/// outside the cone holds all of the light, and the census lists only the stars inside it
/// (R06.T8.k). The light fainter than the cut is taken below M<sub>V</sub> = cut − DM − v☉
/// A<sub>V</sub>, the solar point's V extinction (R06.T8.k). The light is reddened by the dust in
/// front of it, each node's by the solar point's ratios and each overflow star's by its own
/// (R06.T9.e). `ctx` supplies the luminosity tables, the gas modifiers and the noise cache of the
/// rays' profiles; its other fields are not read.
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
    let march = march_rows_through(
        galaxy,
        ctx,
        query,
        Edges::of_replies([*complete_to]),
        spec,
        face,
        rows,
        dust,
    );
    sum_rows(&march, census, complete_to, out);
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
    use crate::units::consts::RADIANS_PER_DEGREE;
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

    /// Whether a test's queries ask for the eye, which since R06.T8.k moves no star of the census.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Eye {
        /// The eye is asked, as the cockpit's sky asks it.
        Asked,
        /// No eye: a camera's sky.
        NotAsked,
    }

    /// [`query_at`] near the Sun to `cut`, with the eye if `eye` asks it.
    fn query_near_the_sun(cut: f64, eye: Eye) -> SkyQuery {
        let builder = SkyQuery::builder(observer_at(SUN), Magnitudes::new(cut));
        match eye {
            Eye::Asked => builder.eye(crate::sky::eye::EyeObserver::default()),
            Eye::NotAsked => builder,
        }
        .build()
        .expect("a valid query")
    }

    /// The listed stars of `query`'s census with every cap forced to `radius`, merged at
    /// [`MAX_N_MAX`].
    fn listed_with_caps_forced(query: &SkyQuery, radius: f64) -> Vec<SkyStar> {
        let galaxy = milky_way_galaxy();
        let query = query
            .clone()
            .with_caps_forced(LightYears::new(radius))
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
    }

    /// The stars a census near the Sun lists to V [`CENSUS_CUT`] within [`CENSUS_RADIUS_LY`],
    /// every cap forced to it, with the eye or without it as `eye` says: the census keeps each
    /// star to the cut alone either way, as the band subtracts it (R06.T8.k). Built once for each.
    fn census_stars(eye: Eye) -> &'static [SkyStar] {
        static ASKED: OnceLock<Vec<SkyStar>> = OnceLock::new();
        static NOT_ASKED: OnceLock<Vec<SkyStar>> = OnceLock::new();
        let build =
            || listed_with_caps_forced(&query_near_the_sun(CENSUS_CUT, eye), CENSUS_RADIUS_LY);
        match eye {
            Eye::Asked => ASKED.get_or_init(build),
            Eye::NotAsked => NOT_ASKED.get_or_init(build),
        }
    }

    /// The census to `cut` complete to `radius` (at most the census's own): its stars brighter
    /// than the cut within the radius, as a partial reply lists them (R06.T8.i), so that listing
    /// and band share one boundary, merged at `n_max`. A census of a brighter cut keeps a subset
    /// of a deeper one's stars, each with the same V, since no star's V depends on the cut.
    fn census_to(cut: f64, radius: f64, n_max: NonZeroU32, eye: Eye) -> SkyCensus {
        assert!(cut <= CENSUS_CUT && radius <= CENSUS_RADIUS_LY);
        let stars: Vec<SkyStar> = census_stars(eye)
            .iter()
            .filter(|s| s.v().value() < cut && s.distance().value() <= radius)
            .copied()
            .collect();
        merge_census([(stars, CensusTallies::default())], n_max)
    }

    /// The listed, overflow and band light near the Sun at `cut`, complete to `radius`, lux: the
    /// listed stars' light and the band's (which holds the overflow), with the eye as `eye` says.
    fn total_light(
        cut: f64,
        radius: f64,
        n_max: NonZeroU32,
        spec: BandSpec,
        eye: Eye,
    ) -> (f64, f64) {
        let census = census_to(cut, radius, n_max, eye);
        let query = query_near_the_sun(cut, eye);
        let band = whole_band(&query, &census, &complete_within(radius), spec);
        (stars_lux(census.listed()), band_lux(&band, spec))
    }

    /// [`total_light`] with the eye asked, held to its bits without the eye: the eye moves no star
    /// between the list and the band (R06.T8.k).
    fn total_light_with_the_eye(
        cut: f64,
        radius: f64,
        n_max: NonZeroU32,
        spec: BandSpec,
    ) -> (f64, f64) {
        let (listed, band) = total_light(cut, radius, n_max, spec, Eye::Asked);
        let (listed_no_eye, band_no_eye) = total_light(cut, radius, n_max, spec, Eye::NotAsked);
        assert_eq!(
            [bits(listed), bits(band)],
            [bits(listed_no_eye), bits(band_no_eye)],
            "cut {cut}, complete to {radius} ly: {listed} + {band} lx with the eye, \
             {listed_no_eye} + {band_no_eye} lx without"
        );
        (listed, band)
    }

    /// The model's own integral along `u` for `query` with no census, complete everywhere,
    /// independently of the band's quadrature: 96 nodes a decade, each node's extinction from a
    /// full-quality sightline to it, and the luminosity functions' light and colour read through
    /// their public functions, the light cut at the solar point's V extinction and dimmed by its
    /// photopic transmission, as the band's is (R06.T9.e, R06.T8.k). Photopic luminance, cd m⁻².
    fn reference_luminance(query: &SkyQuery, direction: UnitVector) -> f64 {
        let galaxy = milky_way_galaxy();
        let tables = milky_way_tables();
        let origin = query.observer().position();
        let from = origin.to_light_years_f64();
        let along = direction.components();
        let edge = distance_to_edge_ly(from, along);
        let mut cache = NoiseCache::with_capacity(1 << 16);
        let components: Vec<ComponentId> = galaxy.fields().component_ids().collect();
        let solar = solar_colour().reddening();
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
                let through = solar.through(Magnitudes::new(extinction));
                let limit = Magnitudes::new(
                    query.cut().value()
                        - distance_modulus(distance)
                        - through.v_extinction().value(),
                );
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
                light * through.photopic_transmission()
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
        // A radius at zero of either sign is +0, so a march finds it by its bits (R06.T9.f).
        let signed = CompleteTo::of_caps(&[LayerCap::forced(Layer::B, LightYears::new(-0.0))]);
        assert_eq!(bits(signed.radius(Layer::B).value()), bits(0.0));
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
        let census = census_to(CENSUS_CUT, 50.0, n(20), Eye::NotAsked);
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
        let census = census_to(CENSUS_CUT, 50.0, n(20), Eye::NotAsked);
        let [yr, yg, yb] = LUMINANCE_RGB;
        for star in census.overflow().iter().take(5) {
            let colour = star.colour();
            // The census's V holds the star's own V extinction (R06.T8.k).
            let unextinguished = star.v() - colour.reddened(star.a_v()).v_extinction();
            for a_v in [star.a_v().value(), 0.5, 2.0, 12.0] {
                // The same star behind a_v of dust.
                let a_v = Magnitudes::new(a_v);
                let reddened = colour.reddened(a_v);
                let v = unextinguished + reddened.v_extinction();
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
    /// list into the overflow (exactly, but for rounding). The eye is asked, and every total is
    /// the bits it is without it (R06.T8.k).
    #[test]
    fn lowering_or_raising_the_cut_conserves_the_light() {
        let spec = spec(8);
        let all = n(MAX_N_MAX);
        let r = CENSUS_RADIUS_LY;
        let mut totals = Vec::new();
        for cut in [CENSUS_CUT, 7.0, 6.0] {
            let (listed, band) = total_light_with_the_eye(cut, r, all, spec);
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
            let (listed_100, band_100) = total_light_with_the_eye(cut, r, n(100), spec);
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
    /// and a later reply, R06.T8.i) give the same sky's light. The eye is asked, and every total
    /// is the bits it is without it (R06.T8.k).
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
        let query = query_near_the_sun(CENSUS_CUT, Eye::Asked);
        let nowhere = band_lux(
            &whole_band(&query, &SkyCensus::empty(), &CompleteTo::nowhere(), spec),
            spec,
        );
        let mut totals = Vec::new();
        for radius in [100.0, CENSUS_RADIUS_LY] {
            let (listed, band) = total_light_with_the_eye(CENSUS_CUT, radius, all, spec);
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

    /// Replies a march near the Sun keeps: complete nowhere; to 50, 100 and 400 ly in every layer;
    /// to a final reply's caps, each layer its own (A's, B's and the brown dwarfs' within 400 ly,
    /// and E's 50,000 ly beyond the root cube's edge on the rays towards +Y, which end 39,536 ly
    /// out); and everywhere.
    fn replies_near_the_sun() -> [CompleteTo; 6] {
        let caps: Vec<LayerCap> = [
            (Layer::A, 70.0),
            (Layer::B, 150.0),
            (Layer::C, 3_000.0),
            (Layer::D, 4_300.0),
            (Layer::E, 50_000.0),
            (Layer::BrownDwarf, 30.0),
        ]
        .into_iter()
        .map(|(layer, radius)| LayerCap::forced(layer, LightYears::new(radius)))
        .collect();
        [
            CompleteTo::nowhere(),
            complete_within(50.0),
            complete_within(100.0),
            complete_within(400.0),
            CompleteTo::of_caps(&caps),
            CompleteTo::everywhere(),
        ]
    }

    /// `sum_rows` at every radius a march keeps equals `band_rows` with the march's radii as nodes,
    /// bit for bit (R06.T9.f): each reply's texels, with a census's overflow among them, are those
    /// of a march that keeps that reply alone over the same nodes. The replies' light falls as the
    /// census is complete further out, and the march refuses no reply it keeps. Summed from the
    /// request's march, a reply differs from its own `band_rows`, whose rays lack the other
    /// replies' nodes, by the quadrature alone: at twelve nodes a decade by up to 2.0% in a texel
    /// and 0.14% over the band, on two rays 5–7° below the plane towards the inner Galaxy (the −Y
    /// face), nearly all of it from the node at 4,300 ly that splits the 3,831–4,642 ly interval
    /// and the rest from 3,000 ly (at 48 a decade 0.05% and 0.003%). Twelve a decade lie within
    /// 1.1–2.4% of 48 in a texel here, and 0.5–0.7% over the band.
    #[test]
    fn a_march_sums_each_reply_as_band_rows_with_the_same_nodes() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let query = query_at(SUN, CENSUS_CUT);
        let census = census_to(CENSUS_CUT, 50.0, n(20), Eye::NotAsked);
        assert!(!census.overflow().is_empty());
        let replies = replies_near_the_sun();
        let nodes = Edges::of_replies(replies).nodes;
        let dust = solar_colour().reddening();
        let mut ctx = context();
        let mut bands: Vec<Vec<BandTexel>> = vec![Vec::new(); replies.len()];
        let mut owns: Vec<Vec<BandTexel>> = vec![Vec::new(); replies.len()];
        let mut quadrature = 0.0_f64;
        for face in CubeFace::ALL {
            let march = march_rows(galaxy, &mut ctx, &query, replies, &spec, face, 0..8);
            assert_eq!(
                (march.spec(), march.face(), march.rows()),
                (spec, face, 0..8)
            );
            assert!(!march.holds(&complete_within(200.0)));
            // Six slots a ray for the radii every layer shares, one more for each layer's cap.
            assert_eq!(march.slots(), 6 * 5 + 6);
            assert!(march.heap_bytes() >= 64 * march.slots() * size_of::<Sums>());
            for (k, reply) in replies.iter().enumerate() {
                assert!(march.holds(reply), "reply {k}");
                let mut kept = Vec::new();
                sum_rows(&march, &census, reply, &mut kept);
                // `band_rows` of the reply, with the march's radii as nodes.
                let alone = Edges {
                    kept: reply.radii_ly.map(|radius| vec![radius]),
                    nodes: nodes.clone(),
                };
                let mut fresh = Vec::new();
                sum_rows(
                    &march_rows_through(galaxy, &mut ctx, &query, alone, spec, face, 0..8, &dust),
                    &census,
                    reply,
                    &mut fresh,
                );
                assert_eq!(texel_bits(&kept), texel_bits(&fresh), "{face:?}, reply {k}");
                let mut own = Vec::new();
                band_rows(
                    galaxy,
                    &mut ctx,
                    &query,
                    &census,
                    reply,
                    &spec,
                    face,
                    0..8,
                    &mut own,
                );
                for (from_march, from_own) in kept.iter().zip(&own) {
                    let (a, b) = (from_march.luminance().value(), from_own.luminance().value());
                    quadrature = quadrature.max((a / b - 1.0).abs());
                }
                bands[k].extend(kept);
                owns[k].extend(own);
            }
        }
        let light: Vec<f64> = bands.iter().map(|band| band_lux(band, spec)).collect();
        let whole = light
            .iter()
            .zip(&owns)
            .map(|(&l, own)| (l / band_lux(own, spec) - 1.0).abs())
            .fold(0.0, f64::max);
        let printed: Vec<String> = light.iter().map(|l| format!("{l:.5e}")).collect();
        eprintln!(
            "one march, six replies: band light {} lx; a reply from the march against its own \
             band_rows, at most {quadrature:.2e} relative in a texel's luminance and {whole:.2e} \
             over the band",
            printed.join(", ")
        );
        for pair in light[..4].windows(2) {
            assert!(pair[1] < pair[0], "{light:?}");
        }
        assert!(light[..5].iter().all(|&l| light[5] < l), "{light:?}");
        assert!(quadrature < 0.03 && whole < 0.002, "{quadrature}, {whole}");
    }

    /// Rows marched in any split, in any order and through a warm or a cold noise cache, each
    /// summed, give one march's texels over the face for every reply it keeps, bit for bit, the
    /// overflow's points split with them (R06.T9.f); and so does a march of the replies in reverse
    /// order with one of them twice.
    #[test]
    fn a_march_in_any_split_of_the_rows_sums_to_the_same_bits() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let query = query_at(SUN, CENSUS_CUT);
        let census = census_to(CENSUS_CUT, 50.0, n(20), Eye::NotAsked);
        let replies = replies_near_the_sun();
        let mut ctx = context();
        let sums = |march: &BandMarch, reply: &CompleteTo| {
            let mut out = Vec::new();
            sum_rows(march, &census, reply, &mut out);
            out
        };
        let mut reordered = replies.to_vec();
        reordered.reverse();
        reordered.push(replies[2]);
        for face in CubeFace::ALL {
            let whole = march_rows(galaxy, &mut ctx, &query, replies, &spec, face, 0..8);
            let shuffled = march_rows(
                galaxy,
                &mut ctx,
                &query,
                reordered.clone(),
                &spec,
                face,
                0..8,
            );
            assert_eq!(shuffled.slots(), whole.slots());
            for reply in &replies {
                assert_eq!(
                    texel_bits(&sums(&shuffled, reply)),
                    texel_bits(&sums(&whole, reply)),
                    "{face:?}, the replies reordered"
                );
            }
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
                let mut parts: Vec<BandMarch> = split
                    .iter()
                    .rev()
                    .map(|rows| march_rows(galaxy, ctx, &query, replies, &spec, face, rows.clone()))
                    .collect();
                parts.sort_by_key(|part| part.rows().start);
                for reply in &replies {
                    let joined: Vec<BandTexel> =
                        parts.iter().flat_map(|part| sums(part, reply)).collect();
                    assert_eq!(
                        texel_bits(&joined),
                        texel_bits(&sums(&whole, reply)),
                        "{face:?}, split {split:?}"
                    );
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "is not a radius the march keeps")]
    fn a_reply_the_march_does_not_keep_is_refused() {
        let march = march_rows(
            milky_way_galaxy(),
            &mut context(),
            &query_at(SUN, 6.5),
            [complete_within(100.0), complete_within(400.0)],
            &spec(2),
            CubeFace::PosZ,
            0..1,
        );
        sum_rows(
            &march,
            &SkyCensus::empty(),
            &complete_within(200.0),
            &mut Vec::new(),
        );
    }

    /// The stars a census near the Sun lists to V [`CENSUS_CUT`] within 400 ly, every cap forced to
    /// it, with no eye: its cells on up to four threads (one on WebAssembly, which has none), each
    /// with its own context, merged at [`MAX_N_MAX`], whose order is total, so the threads'
    /// timing changes no bit.
    fn census_within_400_ly() -> Vec<SkyStar> {
        let galaxy = milky_way_galaxy();
        let query = query_near_the_sun(CENSUS_CUT, Eye::NotAsked)
            .with_caps_forced(LightYears::new(400.0))
            .expect("a forced cap");
        let mut ctx = context();
        let keys: Vec<crate::galaxy::placement::CellKey> =
            census_plan(galaxy, ctx.tables, ctx.envelope, &query, &mut ctx.noise)
                .cells()
                .collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let job = || {
            let mut ctx = context();
            let mut stars = Vec::new();
            loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&key) = keys.get(k) else {
                    return stars;
                };
                census_cell(galaxy, &mut ctx, key, &query, &mut stars);
            }
        };
        let stars: Vec<SkyStar> = if cfg!(target_family = "wasm") {
            job()
        } else {
            std::thread::scope(|scope| {
                let threads: Vec<_> = (0..4).map(|_| scope.spawn(job)).collect();
                threads
                    .into_iter()
                    .flat_map(|thread| thread.join().expect("a census thread completes"))
                    .collect()
            })
        };
        let census = merge_census([(stars, CensusTallies::default())], n(MAX_N_MAX));
        assert!(census.overflow().is_empty());
        census.listed().to_vec()
    }

    /// From one march near the Sun, the listed, overflow and band light of replies complete to 100,
    /// 200 and 400 ly (every cap forced, the census listing only the stars within each radius, its
    /// brightest 500 listed and the rest overflowing into the band) agree within 1% (R06.T9.f,
    /// the band ruling's item 5).
    #[test]
    fn one_marchs_light_at_100_200_and_400_ly_agrees_within_1_percent() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let stars = census_within_400_ly();
        let query = query_near_the_sun(CENSUS_CUT, Eye::NotAsked);
        let radii = [100.0, 200.0, 400.0];
        let replies = radii.map(complete_within);
        let mut ctx = context();
        let marches: Vec<BandMarch> = CubeFace::ALL
            .iter()
            .map(|&face| march_rows(galaxy, &mut ctx, &query, replies, &spec, face, 0..8))
            .collect();
        let mut totals = Vec::new();
        for (radius, reply) in radii.iter().zip(&replies) {
            let within: Vec<SkyStar> = stars
                .iter()
                .filter(|s| s.distance().value() <= *radius)
                .copied()
                .collect();
            let count = within.len();
            let census = merge_census([(within, CensusTallies::default())], n(500));
            assert!(!census.overflow().is_empty(), "{radius} ly, {count} stars");
            let mut band = Vec::new();
            for march in &marches {
                sum_rows(march, &census, reply, &mut band);
            }
            let (listed, band) = (stars_lux(census.listed()), band_lux(&band, spec));
            eprintln!(
                "complete to {radius} ly, {count} stars: listed {listed:.5e} lx, band with the \
                 overflow {band:.5e} lx, together {:.5e} lx",
                listed + band
            );
            totals.push(listed + band);
        }
        for (k, a) in totals.iter().enumerate() {
            for b in &totals[k + 1..] {
                assert!((b / a - 1.0).abs() < 0.01, "{totals:?}");
            }
        }
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

    /// For a cone, the listed and band light together are the full sky's inside the cone, within
    /// 1% (R06.T8.k): a 30° cone's census near the Sun to V [`CENSUS_CUT`] within
    /// [`CENSUS_RADIUS_LY`], with the eye asked, lists exactly the full census's stars inside the
    /// cone, star for star and bit for bit, and its band's texels inside the cone are the full
    /// band's. Before R06.T8.k the cone's census also listed the stars of its cells outside the
    /// cone, whose light its band, complete nowhere there, held too: printed here.
    #[test]
    fn a_cones_listed_and_band_light_are_the_full_skys_inside_it() {
        let spec = spec(16);
        let cone = Cone::new(UnitVector::X, Degrees::new(30.0)).expect("a cone");
        let narrow = SkyQuery::builder(observer_at(SUN), Magnitudes::new(CENSUS_CUT))
            .eye(crate::sky::eye::EyeObserver::default())
            .cone(cone)
            .build()
            .expect("a valid query");
        let origin = *observer_at(SUN).position();
        let inside = |star: &SkyStar| cone.holds(&origin.displacement_to(star.apparent()));
        let coned = listed_with_caps_forced(&narrow, CENSUS_RADIUS_LY);
        let full = census_stars(Eye::NotAsked);
        let full_inside: Vec<SkyStar> = full.iter().filter(|s| inside(s)).copied().collect();
        assert!(
            coned
                .iter()
                .all(|s| inside(s) && s.v().value() <= CENSUS_CUT)
        );
        assert_eq!(
            coned, full_inside,
            "the cone's stars are the full sky's inside it"
        );
        let star_bits = |stars: &[SkyStar]| {
            stars
                .iter()
                .flat_map(|s| {
                    [
                        bits(s.v().value()),
                        bits(s.a_v().value()),
                        bits(s.distance().value()),
                    ]
                })
                .collect::<Vec<u64>>()
        };
        assert_eq!(star_bits(&coned), star_bits(&full_inside));
        // The texels whose centres lie inside the cone, as the band tests a ray.
        let complete = complete_within(CENSUS_RADIUS_LY);
        let band_inside = |query: &SkyQuery, census: &SkyCensus| {
            whole_band(query, census, &complete, spec)
                .iter()
                .zip(texel_geometry(spec))
                .filter(|(_, (u, _))| u.dot(&cone.axis()) >= cone.cos_half_angle())
                .map(|(t, (_, w))| t.luminance().value() * w)
                .sum::<f64>()
        };
        let all = n(MAX_N_MAX);
        let cone_sky = merge_census([(coned.clone(), CensusTallies::default())], all);
        let full_sky = census_to(CENSUS_CUT, CENSUS_RADIUS_LY, all, Eye::NotAsked);
        let (cone_band, full_band) = (
            band_inside(&narrow, &cone_sky),
            band_inside(&query_near_the_sun(CENSUS_CUT, Eye::NotAsked), &full_sky),
        );
        assert_eq!(bits(cone_band), bits(full_band), "the band inside the cone");
        let cone_light = stars_lux(cone_sky.listed()) + cone_band;
        let full_light = stars_lux(&full_inside) + full_band;
        // What the cone's census listed before R06.T8.k beyond these: its cells' stars outside it.
        let mut ctx = context();
        let forced = narrow
            .clone()
            .with_caps_forced(LightYears::new(CENSUS_RADIUS_LY))
            .expect("a forced cap");
        let cells: std::collections::BTreeSet<crate::galaxy::placement::CellKey> = census_plan(
            milky_way_galaxy(),
            ctx.tables,
            ctx.envelope,
            &forced,
            &mut ctx.noise,
        )
        .cells()
        .collect();
        let outside: Vec<SkyStar> = full
            .iter()
            .filter(|s| {
                !inside(s)
                    && crate::galaxy::placement::CellKey::of(s.system())
                        .is_ok_and(|key| cells.contains(&key))
            })
            .copied()
            .collect();
        eprintln!(
            "a 30° cone within {CENSUS_RADIUS_LY} ly to V {CENSUS_CUT}: {} stars listed, \
             {cone_light:.5e} lx with the band inside it, the full sky's {full_light:.5e} lx; its \
             cells' {} stars outside it, {:.2}% of that light, were listed before R06.T8.k",
            coned.len(),
            outside.len(),
            100.0 * stars_lux(&outside) / cone_light
        );
        assert!(!coned.is_empty() && !outside.is_empty());
        assert!(
            (cone_light / full_light - 1.0).abs() < 0.01,
            "{cone_light} lx against {full_light} lx"
        );
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
