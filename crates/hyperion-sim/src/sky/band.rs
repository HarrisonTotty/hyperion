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
//!   (`decision-r06-census-cost-signoff.md`): each layer's complete-to radius ([`CompleteTo`]),
//!   its cap once its census is final and, before that, the edge of its shells done held within
//!   the cap (R06.T8.i). Since R06.T7.b a cap is one radius a ray of the caps' lattice, and each
//!   texel's ray reads the radius towards the texel's centre. A census not yet final in a layer
//!   lists its stars by that same radius towards each star's texel, so the two share it star by
//!   star (decided 2026-10-08, `decision-r06-t8i-listing.md`). The census's plan opens its cells
//!   by the rays' cones about each star's own direction, though, so where a cone's edge crosses a
//!   texel a star within the radius towards the texel's centre may lie in a cell left closed, and
//!   its light is in neither. That is R06.T7.b's gap, deferred by the owner: under one star a
//!   layer by the caps' count beyond, and some 10⁻² stars over the sky near the Sun (R06's Risks,
//!   "Deviations in T8.i, as built").
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
//! census keeps each star to the cut alone, with the eye or without it (R06.T8.k), and for a query
//! with a cone only in the cone's region, the band's texels about it, every texel that meets it
//! among them ([`ConeRegion`]; R06.T8.l, decided 2026-10-07, `decision-r06-t8k-cone.md`). The band
//! is complete in the region's texels and complete nowhere in the others, and the census lists
//! exactly the stars that [`BandSpec::texel_of`] places in the region's, so the cut and the cone's
//! region are boundaries the two share exactly.
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
//!
//! **The eye's light under a camera's cut** (R06.T9.j; decided 2026-10-06,
//! `decision-r06-t9c-glare.md`). The eye's limits are those of the eye's own request, whatever the
//! request's cut: their background is the light fainter than the eye's cut ([`SkyQuery::eye_cut`]),
//! not the band's. Where a camera's deeper cut sets the query's, the march also keeps, per layer and
//! radius, the five sums of the light fainter than the eye's cut within the radius plus all of the
//! light beyond it (the band's own sums beyond the radius), and [`sum_rows`] gives each texel that
//! light beside its own, with the overflow's stars an eye-only request would hold there (those at
//! or brighter than the eye's cut). A listed star fainter than the eye's cut adds no light to it:
//! its light is there already, as expected light. Each of those sums takes the arithmetic of a
//! march at the eye's cut, so at the same replies (the same census radius) a texel's eye light is,
//! bit for bit, the texel of the eye-only request's band. The band's own texels, as sent, are
//! unchanged. The eye's map under a camera's cut is so the eye-only request's at the camera's
//! census radii (the same replies and `n_max`). Beyond the eye-only request's own caps the
//! camera's census lists the real stars brighter than the eye's cut, under one expected a layer,
//! which the eye-only request holds as expected light: accepted as the truer sky, since those
//! stars take the place of their expected light in the eye's background, in expectation, not star
//! by star (decided 2026-10-07, `decision-r06-t9c-glare.md`, addendum 2).
//!
//! **The diffuse galactic light** (R06.T9.g; decided 2026-10-07, `decision-r06-t9g-dgl.md`). Where
//! the query states the request's illumination ([`SkyQuery::illumination`]), the observer's own sky
//! of all starlight, each ray also holds the starlight its dust scatters towards the observer
//! ([`super::dgl`]): five sums, in cd m⁻², from the ray's direction and its A<sub>V</sub> to the
//! root cube's edge alone, the profile's last node, whose bits no radius taken as a node moves.
//! [`march_rows`] keeps them beside the ray's slots, and [`sum_rows`] adds them to the texel's light
//! and to the eye's, before the overflow. So the diffuse light is the same in every reply and
//! under any cut, census or cone; the eye's light under a camera's cut keeps the eye-only
//! request's bits; and, being no star's light, it leaves the boundary the listing and the band
//! share where it is. [`BandTexel::diffuse_luminance`] gives a texel's part. A query with no
//! illumination gives the band as before, bit for bit.

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
use crate::observe::Observer;
use crate::time::Span;
use crate::units::consts::{
    METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SOLAR_ABSOLUTE_MAGNITUDE_V,
};
use crate::units::{CandelasPerSquareMetre, LightYears, Magnitudes, Radians};

use super::caps::{CAPPED_LAYERS, LayerCap, RayRadii};
use super::census::{ConeRegion, SkyCensus, SkyContext, SkyQuery};
use super::colour::{Reddened, Reddening, StarColour, lift_into_gamut, solar_colour};
use super::dgl::Illumination;
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

    /// The band's largest texel radius: the greatest angle from any texel's centre to its own
    /// corners, the farthest points of a texel from its centre (R06.T8.l; decided 2026-10-07,
    /// `decision-r06-t8k-cone.md`).
    ///
    /// For faces of n texels a side it is atan(√2 ÷ n). The cube's projection makes its texels
    /// largest at a face's centre, where the face meets the sphere face on, and there one end of a
    /// texel's half-diagonal, √2 ÷ n long on the face at unit distance, is the face's centre: the
    /// texel's own centre for an odd n, its corner for an even one. It is 1.266° at
    /// [`STANDARD`](Self::STANDARD)'s 64² and 5.05° at 16². Every point of a texel lies within it
    /// of the texel's centre, so a direction within α of a cone's axis lies in a texel whose
    /// centre is within α plus this.
    #[must_use]
    pub fn largest_texel_radius(&self) -> Radians {
        Radians::new(math::atan2(
            core::f64::consts::SQRT_2,
            f64::from(self.face_texels),
        ))
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
/// A census is complete to its plan's caps once final ([`CompleteTo::of_caps`]), and before that,
/// layer by layer, to the edge of its shells done, held within the caps (R06.T8.i; the
/// [`Completeness`](super::census::Completeness) a census of shells carries). Since R06.T7.b a
/// layer's cap is one radius a ray of the caps' lattice, and the band takes, towards each of its
/// texels, the radius [`LayerCap::radius_toward`] gives at the texel's centre: the largest radius
/// of the rays whose cones hold it. A census not yet final lists by that radius towards each
/// star's texel. Its plan opens cells by the cones about each star's own direction, so in a texel
/// a cone's edge crosses, the radius can exceed the one towards some of its stars, whose cells may
/// be left closed (R06.T7.b's gap, deferred; `decision-r06-t8i-listing.md`). A forced cap is one
/// radius in every direction. Each reply of a request states its own, and one [`march_rows`]
/// keeps every reply's radii (R06.T9.f).
///
/// Cloning shares a cap's radii per ray.
#[derive(Debug, Clone, PartialEq)]
pub struct CompleteTo {
    /// Each layer's boundary, in [`CAPPED_LAYERS`]' order.
    radii: [Boundary; BAND_LAYERS],
}

/// How far a census is complete in one layer: one radius in every direction (0 for nowhere, +∞ for
/// everywhere), or a radius a ray of the caps' lattice (R06.T7.b).
#[derive(Debug, Clone, PartialEq)]
enum Boundary {
    Uniform(LightYears),
    ByRay(RayRadii),
}

impl Boundary {
    /// The radius towards `direction`, ly, held at +0 at or below zero.
    #[must_use]
    fn toward(&self, direction: UnitVector) -> f64 {
        match self {
            Self::Uniform(radius) => radius.value(),
            Self::ByRay(rays) => above_zero_or_zero(rays.toward(direction).value()),
        }
    }

    /// The farthest radius in any direction, ly.
    #[must_use]
    fn farthest(&self) -> f64 {
        match self {
            Self::Uniform(radius) => radius.value(),
            Self::ByRay(rays) => rays.largest().value(),
        }
    }

    /// Whether the two are the same boundary: one radius of the same bits, or the same radii a ray.
    /// A march finds a reply's slots by it (R06.T9.f).
    #[must_use]
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Uniform(a), Self::Uniform(b)) => a.value().total_cmp(&b.value()).is_eq(),
            (Self::ByRay(a), Self::ByRay(b)) => a == b,
            (Self::Uniform(_), Self::ByRay(_)) | (Self::ByRay(_), Self::Uniform(_)) => false,
        }
    }
}

impl CompleteTo {
    /// Complete to each layer's cap in `caps` (a [`CensusPlan`](super::census::CensusPlan)'s):
    /// towards each direction, to the cap's radius towards it, and nowhere for a layer `caps` does
    /// not hold.
    #[must_use]
    pub fn of_caps(caps: &[LayerCap]) -> Self {
        Self::of_caps_within(caps, [None; BAND_LAYERS])
    }

    /// Complete to each layer's cap in `caps`, held within the layer's edge in `within_ly`, ly,
    /// where it has one.
    ///
    /// That is a census of some of its plan's shells (R06.T8.i). Towards each direction
    /// a layer with an edge is complete to the lesser of its edge and its cap's radius towards it,
    /// and one with none to its cap, as [`of_caps`](Self::of_caps). A cap one radius a ray all of
    /// whose rays reach the edge is complete to the edge in every direction.
    #[must_use]
    pub(crate) fn of_caps_within(caps: &[LayerCap], within_ly: [Option<f64>; BAND_LAYERS]) -> Self {
        Self {
            radii: std::array::from_fn(|l| {
                let (layer, edge) = (CAPPED_LAYERS[l], within_ly[l]);
                caps.iter().find(|cap| cap.layer() == layer).map_or(
                    Boundary::Uniform(LightYears::ZERO),
                    |cap| match (cap.rays(), edge) {
                        (Some(rays), None) => Boundary::ByRay(rays.clone()),
                        (Some(rays), Some(edge)) if rays.all_reach(edge) => {
                            Boundary::Uniform(LightYears::new(above_zero_or_zero(edge)))
                        }
                        (Some(rays), Some(edge)) => Boundary::ByRay(rays.within(edge)),
                        (None, edge) => {
                            let radius = cap.radius().value();
                            let radius = edge.map_or(radius, |edge| radius.min(edge));
                            Boundary::Uniform(LightYears::new(above_zero_or_zero(radius)))
                        }
                    },
                )
            }),
        }
    }

    /// Complete everywhere: the band holds only the light fainter than the cut, as a band computed
    /// with no census at all needs (the eye cut's pre-pass, R06.T9.d, and the band of the stars
    /// fainter than a limit).
    #[must_use]
    pub const fn everywhere() -> Self {
        Self::uniform(f64::INFINITY)
    }

    /// Complete nowhere: the band holds all of the light, as before a census's first reply.
    #[must_use]
    pub const fn nowhere() -> Self {
        Self::uniform(0.0)
    }

    /// Complete to `radius_ly` in every layer and direction.
    #[must_use]
    const fn uniform(radius_ly: f64) -> Self {
        let radius = LightYears::new(radius_ly);
        Self {
            radii: [
                Boundary::Uniform(radius),
                Boundary::Uniform(radius),
                Boundary::Uniform(radius),
                Boundary::Uniform(radius),
                Boundary::Uniform(radius),
                Boundary::Uniform(radius),
            ],
        }
    }

    /// The farthest radius to which `layer` is complete in any direction: 0 for nowhere (and for
    /// the rogue planets, which are dark), +∞ for everywhere. [`radius_toward`](Self::radius_toward)
    /// gives a direction's.
    #[must_use]
    pub fn radius(&self, layer: Layer) -> LightYears {
        CAPPED_LAYERS
            .iter()
            .position(|&l| l == layer)
            .map_or(LightYears::ZERO, |i| {
                LightYears::new(self.radii[i].farthest())
            })
    }

    /// The radius to which `layer` is complete along each ray of the caps' lattice, ly, in the
    /// lattice's order, or `None` where it is one radius in every direction,
    /// [`radius`](Self::radius) (R06.T11.d: the reply's per-ray table).
    ///
    /// Towards a direction the layer is complete to the largest of these radii over the rays whose
    /// cones hold it ([`RayRadii::toward`](super::caps::RayRadii::toward)).
    #[must_use]
    pub fn rays_ly(&self, layer: Layer) -> Option<&[f64]> {
        let i = CAPPED_LAYERS.iter().position(|&l| l == layer)?;
        match &self.radii[i] {
            Boundary::Uniform(_) => None,
            Boundary::ByRay(rays) => Some(rays.radii_ly()),
        }
    }

    /// Whether the two are the same radii in every layer, by their bits, as a march finds a reply's
    /// slots by them.
    #[must_use]
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.radii.iter().zip(&other.radii).all(|(a, b)| a.same(b))
    }

    /// The radius to which `layer` is complete towards `direction`: the band's boundary along a ray
    /// in that direction.
    #[must_use]
    pub fn radius_toward(&self, layer: Layer, direction: UnitVector) -> LightYears {
        CAPPED_LAYERS
            .iter()
            .position(|&l| l == layer)
            .map_or(LightYears::ZERO, |i| {
                LightYears::new(self.radii[i].toward(direction))
            })
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
    /// The diffuse galactic light's part of `luminance` (R06.T9.g): zero where the query states no
    /// illumination. Not on the wire, whose luminance holds it.
    diffuse: CandelasPerSquareMetre,
    eye: Option<EyeLimit>,
    /// The light fainter than the eye's cut, where a camera's deeper cut set the band's (R06.T9.j):
    /// the eye's background. `None` where the texel's own light is the eye's. Not on the wire.
    eye_light: Option<EyeLight>,
}

/// The light the eye's limit is taken against where it is not the band's own (R06.T9.j): the light
/// fainter than the eye's cut in a texel's direction, its photopic luminance and S/P ratio, and that
/// cut, which the limit map's glare must share.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EyeLight {
    luminance: CandelasPerSquareMetre,
    sp_ratio: f64,
    cut: Magnitudes,
}

/// The photopic luminance and S/P ratio of light of the five sums `sums` ([`Sums`]), as a texel
/// takes them: the photopic sum, held at zero or more, and the scotopic sum over it, or the
/// reference star's ρ ([`REFERENCE_SP_RATIO`]) where there is no light.
#[must_use]
fn luminance_and_ratio(sums: &Sums) -> (CandelasPerSquareMetre, f64) {
    let [luminance, .., scotopic] = *sums;
    let sp_ratio = if luminance > 0.0 {
        scotopic / luminance
    } else {
        REFERENCE_SP_RATIO
    };
    (CandelasPerSquareMetre::new(luminance.max(0.0)), sp_ratio)
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
        let [luminance, red, green, blue, _] = sums;
        let [yr, yg, yb] = LUMINANCE_RGB;
        let rgb_luminance = yr * red + yg * green + yb * blue;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the chroma is a display value of order one, and f32 is its declared type"
        )]
        let chroma = if luminance > 0.0 && rgb_luminance > 0.0 {
            let [red, green, _] = lift_into_gamut([red, green, blue].map(|c| c / rgb_luminance));
            [red as f32, green as f32]
        } else {
            [1.0, 1.0]
        };
        let (luminance, sp_ratio) = luminance_and_ratio(&sums);
        Self {
            luminance,
            chroma,
            sp_ratio,
            diffuse: CandelasPerSquareMetre::ZERO,
            eye: None,
            eye_light: None,
        }
    }

    /// The texel with `diffuse`, the photopic part of its light that is the diffuse galactic light
    /// (R06.T9.g), as [`diffuse_luminance`](Self::diffuse_luminance) gives it.
    #[must_use]
    const fn with_diffuse(mut self, diffuse: CandelasPerSquareMetre) -> Self {
        self.diffuse = diffuse;
        self
    }

    /// The texel with the light fainter than the eye's cut `cut` of the five sums `eye` ([`Sums`])
    /// as the eye's background, where the march kept it (R06.T9.j).
    #[must_use]
    fn with_eye_light(mut self, eye: Option<(Sums, Magnitudes)>) -> Self {
        self.eye_light = eye.map(|(sums, cut)| {
            debug_assert!(
                sums.iter().all(|v| v.is_finite()),
                "a band texel's eye sums {sums:?}"
            );
            let (luminance, sp_ratio) = luminance_and_ratio(&sums);
            EyeLight {
                luminance,
                sp_ratio,
                cut,
            }
        });
        self
    }

    /// The texel's photopic luminance: the light of every star the census did not list, and the
    /// diffuse galactic light where the query states the request's illumination (R06.T9.g).
    #[must_use]
    pub const fn luminance(&self) -> CandelasPerSquareMetre {
        self.luminance
    }

    /// The diffuse galactic light's part of the texel's [`luminance`](Self::luminance), cd m⁻².
    ///
    /// It is the starlight the dust along the texel's ray scatters towards the observer
    /// (R06.T9.g; [`super::dgl`]), zero where the query states no illumination. It is the same in
    /// every reply and under any cut, census or cone, and is no star's light: the rest of the
    /// luminance is the band's starlight and its overflow, as before.
    #[must_use]
    pub const fn diffuse_luminance(&self) -> CandelasPerSquareMetre {
        self.diffuse
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

    /// The light the eye's limit in the texel's direction is taken against, its photopic luminance
    /// and S/P ratio: the light fainter than the eye's cut (R06.T9.j). That is the texel's own light
    /// unless a camera's deeper cut set the band's, when the march kept the eye's beside it.
    #[must_use]
    pub(crate) const fn eye_background(&self) -> (CandelasPerSquareMetre, f64) {
        match self.eye_light {
            Some(light) => (light.luminance, light.sp_ratio),
            None => (self.luminance, self.sp_ratio),
        }
    }

    /// The eye's cut of the light [`eye_background`](Self::eye_background) gives, where the march
    /// kept it beside the texel's own (R06.T9.j); `None` where the texel's own light is the eye's.
    #[must_use]
    pub(crate) const fn eye_light_cut(&self) -> Option<Magnitudes> {
        match self.eye_light {
            Some(light) => Some(light.cut),
            None => None,
        }
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
            diffuse: CandelasPerSquareMetre::ZERO,
            eye: None,
            eye_light: None,
        }
    }

    /// The texel without the eye's light, its own light as the eye's background: the texel as
    /// R06.T9.i read it under a camera's cut, for the tests that measure what R06.T9.j changes.
    #[must_use]
    pub(crate) const fn without_eye_light(self) -> Self {
        Self {
            eye_light: None,
            ..self
        }
    }
}

/// A texel's or a ray's five light sums, in order: the photopic light, the linear Rec. 709 red,
/// green and blue light, and the scotopic light (the photopic light that ρ weights), each after
/// the dust in front of it (R06.T9.e).
pub(crate) type Sums = [f64; 5];

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

/// The boundaries a march keeps a reply's sums at, and the radii its rays take as nodes.
#[derive(Debug, Clone, PartialEq)]
struct Edges {
    /// Per layer of [`CAPPED_LAYERS`], each boundary to which a reply is complete, each once: the
    /// radii in every direction ascending (0 for nowhere, +∞ for everywhere), then those a ray
    /// in the replies' order. Each ray keeps each boundary at its radius towards the ray.
    kept: [Vec<Boundary>; BAND_LAYERS],
    /// The radii every ray in the query's cone's region (or of a query with no cone) takes as
    /// nodes, ly, ascending and each once, beside its own radii of the boundaries a ray: every
    /// layer's radii in every direction (and, in the tests, more).
    nodes: Vec<f64>,
}

impl Edges {
    /// The boundaries of `replies`, each layer's, and every layer's radii in every direction.
    ///
    /// # Panics
    ///
    /// If `replies` holds no reply.
    #[must_use]
    fn of_replies(replies: impl IntoIterator<Item = CompleteTo>) -> Self {
        let mut uniform: [Vec<f64>; BAND_LAYERS] = Default::default();
        let mut by_ray: [Vec<Boundary>; BAND_LAYERS] = Default::default();
        let mut any = false;
        for reply in replies {
            any = true;
            for ((radii, rays), boundary) in uniform.iter_mut().zip(&mut by_ray).zip(reply.radii) {
                match boundary {
                    Boundary::Uniform(radius) => radii.push(radius.value()),
                    Boundary::ByRay(_) => {
                        if !rays.iter().any(|b| b.same(&boundary)) {
                            rays.push(boundary);
                        }
                    }
                }
            }
        }
        assert!(any, "a march keeps the radii of one reply at least");
        for radii in &mut uniform {
            ascending_once(radii);
        }
        let mut nodes: Vec<f64> = uniform.iter().flatten().copied().collect();
        ascending_once(&mut nodes);
        let kept = std::array::from_fn(|l| {
            uniform[l]
                .iter()
                .map(|&radius| Boundary::Uniform(LightYears::new(radius)))
                .chain(by_ray[l].iter().cloned())
                .collect()
        });
        Self { kept, nodes }
    }
}

/// Sorts `radii` ascending and keeps each once.
fn ascending_once(radii: &mut Vec<f64>) {
    radii.sort_by(f64::total_cmp);
    radii.dedup_by(|left, right| left.total_cmp(right).is_eq());
}

/// Each layer's first slot in a ray's sums, for each layer's count of radii kept, `kept`, in turn,
/// then the slots a ray holds.
#[must_use]
fn slot_starts(kept: [usize; BAND_LAYERS]) -> [usize; BAND_LAYERS + 1] {
    let mut starts = [0; BAND_LAYERS + 1];
    for (l, &radii) in kept.iter().enumerate() {
        starts[l + 1] = starts[l] + radii;
    }
    starts
}

/// How a ray's texel stands against its query's cone, the one place a ray's region is decided: a
/// query with a cone is complete only in the cone's region, the band's texels about it, where its
/// census lists stars (R06.T8.l).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// In the cone's region, or no cone: complete to each reply's radii.
    Inside,
    /// Outside the cone's region: complete nowhere, all of the light, whatever a reply's radii.
    Outside,
}

impl Reach {
    /// The reach of the ray through the texel at `row` and `column` of `face` for the cone's
    /// `region`, if any: inside where [`ConeRegion::holds_texel`]
    /// holds it, the test the census keeps each star's texel by.
    #[must_use]
    fn of(region: Option<&ConeRegion>, face: CubeFace, row: u16, column: u16) -> Self {
        if region.is_some_and(|region| !region.holds_texel(face, row, column)) {
            Self::Outside
        } else {
            Self::Inside
        }
    }
}

/// The radius of a ray outside its query's cone's region: complete nowhere.
const NOWHERE_LY: [f64; 1] = [0.0];

/// A face's rows of a band, marched (R06.T9.f; see the [module](self) documentation): each ray's
/// five sums ([`Sums`]) of a reply complete to each radius the march keeps, per layer, from which
/// [`sum_rows`] gives any of those replies' texels; and, where a camera's deeper cut set the
/// query's, the same of the light fainter than the eye's cut, the eye's background (R06.T9.j).
///
/// Its heap is the rays' slots, 40 bytes for each layer's radius and ray, twice that where it keeps
/// the eye's light, and each ray's A<sub>V</sub> to the edge, 8 bytes, and diffuse sums, 40 bytes
/// where the query states an illumination (R06.T9.g; [`heap_bytes`](Self::heap_bytes)): at 64²
/// texels a face, with six layers' one radius each, some 5.9 MB for the band and 1.2 MB of the
/// diffuse light.
#[derive(Debug, Clone, PartialEq)]
pub struct BandMarch {
    spec: BandSpec,
    face: CubeFace,
    rows: Range<u16>,
    /// The observer's position, from which the overflow's stars are placed in their texels.
    origin: GalacticPosition,
    /// Per layer of [`CAPPED_LAYERS`], the boundaries it keeps, each once ([`Edges::kept`]).
    kept: [Vec<Boundary>; BAND_LAYERS],
    /// The eye's cut, where it is shallower than the query's and the march keeps the light fainter
    /// than it beside the band's (R06.T9.j).
    eye_cut: Option<Magnitudes>,
    /// Each ray's slots, in the band's order (row by row from the top, each row from its left):
    /// each layer's radii in turn, each slot the sums of a reply complete to that radius in that
    /// layer, in L☉,V × `lux_per_v0` per ly², before K; then, where the march keeps the eye's
    /// light, the same slots of the light fainter than the eye's cut.
    sums: Vec<Sums>,
    /// Each ray's A<sub>V</sub> to the root cube's edge, A<sub>∞</sub>, in the band's order: its
    /// profile's last node, 0 for a ray that holds no light.
    edge_a_v: Vec<Magnitudes>,
    /// Each ray's five sums of the diffuse galactic light, cd m⁻², in the band's order, where the
    /// march has an illumination (R06.T9.g); empty otherwise.
    diffuse: Vec<Sums>,
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

    /// The eye's cut whose light the march keeps beside the band's, the eye's background
    /// (R06.T9.j): the query's eye's cut where it is shallower than the query's, and `None` where
    /// the query asks no eye or asks it at its own cut, whose light is the band's.
    #[must_use]
    pub const fn eye_cut(&self) -> Option<Magnitudes> {
        self.eye_cut
    }

    /// Whether the march keeps every layer's radius of `complete_to`, so that [`sum_rows`] can sum
    /// a reply complete to it.
    #[must_use]
    pub fn holds(&self, complete_to: &CompleteTo) -> bool {
        self.kept
            .iter()
            .zip(&complete_to.radii)
            .all(|(kept, boundary)| kept.iter().any(|b| b.same(boundary)))
    }

    /// The bytes the march holds on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.sums.capacity() * size_of::<Sums>()
            + self.edge_a_v.capacity() * size_of::<Magnitudes>()
            + self.diffuse.capacity() * size_of::<Sums>()
            + self
                .kept
                .iter()
                .map(|kept| kept.capacity() * size_of::<Boundary>())
                .sum::<usize>()
    }

    /// Each ray's A<sub>V</sub> to the root cube's edge, A<sub>∞</sub>, in the band's order: its
    /// extinction profile's last node, whose bits no other node moves (R06.T9.g).
    #[must_use]
    pub(crate) fn edge_a_v(&self) -> &[Magnitudes] {
        &self.edge_a_v
    }

    /// Each ray's five sums of the diffuse galactic light, cd m⁻², in the band's order: empty
    /// where the march had no illumination (R06.T9.g).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn diffuse(&self) -> &[Sums] {
        &self.diffuse
    }

    /// Each ray's five sums of the band's light, cd m⁻², of a reply complete to `complete_to`, in
    /// the band's order: its slots over the layers, as [`sum_rows`] takes them before the overflow
    /// and the diffuse light, each ray's at the reply's radius towards its own direction
    /// ([`CompleteTo::radius_toward`]; R06.T7.b). The illumination is the march complete nowhere
    /// (R06.T9.g).
    ///
    /// # Panics
    ///
    /// If the march does not keep a layer's radius of `complete_to`.
    #[must_use]
    pub(crate) fn ray_light(&self, complete_to: &CompleteTo) -> Vec<Sums> {
        let slots = self.slots_of(complete_to);
        let to_luminance = light_to_lux_ly2();
        if self.sums.is_empty() {
            return Vec::new();
        }
        self.sums
            .chunks_exact(self.ray_len())
            .map(|ray| summed_light(ray, &slots, to_luminance))
            .collect()
    }

    /// The slots a ray holds of one light: the band's, and as many of the eye's where it keeps
    /// them.
    #[must_use]
    fn slots(&self) -> usize {
        slot_starts(self.kept.each_ref().map(Vec::len))[BAND_LAYERS]
    }

    /// The sums a ray holds: its band's slots, then the eye's where the march keeps them.
    #[must_use]
    fn ray_len(&self) -> usize {
        self.slots() * lights(self.eye_cut)
    }

    /// The slot of each layer's radius of `complete_to` in a ray's sums.
    ///
    /// # Panics
    ///
    /// If the march does not keep a layer's radius.
    #[must_use]
    fn slots_of(&self, complete_to: &CompleteTo) -> [usize; BAND_LAYERS] {
        let starts = slot_starts(self.kept.each_ref().map(Vec::len));
        std::array::from_fn(|l| {
            let boundary = &complete_to.radii[l];
            let at = self.kept[l]
                .iter()
                .position(|b| b.same(boundary))
                .unwrap_or_else(|| {
                    panic!(
                        "complete to {} ly in layer {:?} is not a radius the march keeps: {:?}",
                        boundary.farthest(),
                        CAPPED_LAYERS[l],
                        self.kept[l]
                            .iter()
                            .map(Boundary::farthest)
                            .collect::<Vec<_>>()
                    )
                });
            starts[l] + at
        })
    }
}

/// One ray's light of a reply, cd m⁻²: its slots `slots` of `ray` summed over the layers, in their
/// order, then times K (`to_luminance`), as [`sum_rows`] takes each texel's before its overflow.
#[must_use]
fn summed_light(ray: &[Sums], slots: &[usize; BAND_LAYERS], to_luminance: f64) -> Sums {
    let mut light = [0.0; 5];
    for &slot in slots {
        add_to(&mut light, ray[slot]);
    }
    light.map(|v| v * to_luminance)
}

/// The lights a march keeps for each ray: the band's, and the eye's where a camera's deeper cut
/// set the query's (`eye_cut`).
#[must_use]
const fn lights(eye_cut: Option<Magnitudes>) -> usize {
    if eye_cut.is_some() { 2 } else { 1 }
}

/// What a march holds for every ray: the components, each layer's share of their systems, the
/// reddening curves that dim every node, the eye's cut where the march keeps its light, and the
/// ray's scratch buffers.
struct Rays {
    components: Vec<ComponentId>,
    dust: Reddening,
    /// The eye's cut, V, where the march keeps the light fainter than it (R06.T9.j).
    eye_cut: Option<f64>,
    /// Per layer of [`CAPPED_LAYERS`], each component's share of its systems.
    shares: [[f64; MAX_COMPONENTS]; BAND_LAYERS],
    nodes: Vec<LightYears>,
    a_v: Vec<Magnitudes>,
    /// The last ray's A<sub>V</sub> to the root cube's edge: its profile's last node, or 0 where
    /// the ray ends before its first node (R06.T9.g).
    edge_a_v: Magnitudes,
    modifiers: Vec<GasModifier>,
    /// A ray's slots: the light fainter than the cut within each radius, from the first node.
    within: Vec<Sums>,
    /// A ray's slots: all of the light beyond each radius, from the radius out.
    beyond: Vec<Sums>,
    /// A ray's slots: the light fainter than the eye's cut within each radius, from the first
    /// node, where the march keeps it; empty otherwise.
    eye_within: Vec<Sums>,
    /// A ray's radii per layer: its march's boundaries towards its direction, ascending and each
    /// once (R06.T7.b).
    ray_kept: [Vec<f64>; BAND_LAYERS],
    /// Per layer, each of the march's boundaries' place among the ray's radii.
    ray_places: [Vec<usize>; BAND_LAYERS],
    /// A ray's radii taken as nodes: the march's, then its own.
    ray_nodes: Vec<f64>,
}

/// One layer's photopic light sums at one node, per ly³: of its stars fainter than the cut, of all
/// its stars, and of its stars fainter than the eye's cut, each zero where the node does not need
/// it.
#[derive(Debug, Clone, Copy, Default)]
struct NodeLight {
    fainter: [f64; 4],
    all: [f64; 4],
    eye_fainter: [f64; 4],
}

impl NodeLight {
    /// Its light dimmed by the dust `through` in front of the node ([`dimmed`]): of the stars
    /// fainter than the cut, of all of them, and of those fainter than the eye's cut, each zero
    /// where `needs` does not ask it. The eye's is zero where the march keeps none, as summed.
    #[must_use]
    fn dimmed(self, needs: Needs, through: &Reddened) -> (Sums, Sums, Sums) {
        let fainter = |light: [f64; 4]| {
            if needs.fainter {
                dimmed(light, through)
            } else {
                [0.0; 5]
            }
        };
        let all = if needs.all {
            dimmed(self.all, through)
        } else {
            [0.0; 5]
        };
        (fainter(self.fainter), all, fainter(self.eye_fainter))
    }
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

/// Writes one light's slots of a ray into `out`, in the march's order (`ours`, the first slot of
/// each layer's boundaries in [`BandMarch`]): each slot the light fainter than a cut within its
/// radius, `within`, plus all of the light beyond it, `beyond`, both in the ray's own order
/// (`starts`, each layer's radii towards the ray ascending), the march's boundary at `places[l][j]`
/// among them. A ray outside the cone's region (`reach`) has one radius a layer, whose sums fill
/// every slot of its layer.
fn fill_slots(
    (within, beyond): (&[Sums], &[Sums]),
    starts: &[usize; BAND_LAYERS + 1],
    ours: &[usize; BAND_LAYERS + 1],
    places: &[Vec<usize>; BAND_LAYERS],
    reach: Reach,
    out: &mut [Sums],
) {
    for l in 0..BAND_LAYERS {
        let theirs = &mut out[ours[l]..ours[l + 1]];
        let total =
            |slot: usize| -> Sums { std::array::from_fn(|k| within[slot][k] + beyond[slot][k]) };
        match reach {
            Reach::Inside => {
                for (slot, &place) in theirs.iter_mut().zip(&places[l]) {
                    *slot = total(starts[l] + place);
                }
            }
            Reach::Outside => {
                debug_assert_eq!(
                    starts[l + 1] - starts[l],
                    1,
                    "a ray outside keeps one radius"
                );
                theirs.fill(total(starts[l]));
            }
        }
    }
}

impl Rays {
    #[must_use]
    fn new(galaxy: &Galaxy, dust: &Reddening, eye_cut: Option<Magnitudes>) -> Self {
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
            eye_cut: eye_cut.map(Magnitudes::value),
            shares,
            nodes: Vec::new(),
            a_v: Vec::new(),
            edge_a_v: Magnitudes::ZERO,
            modifiers: Vec::new(),
            within: Vec::new(),
            beyond: Vec::new(),
            eye_within: Vec::new(),
            ray_kept: Default::default(),
            ray_places: Default::default(),
            ray_nodes: Vec::new(),
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
    /// old: the light fainter than `limit` and all of it, each where `needs` asks it, and the light
    /// fainter than `eye_limit`, where there is one, wherever it asks the light fainter than
    /// `limit`. Each is summed over the components in one order, so the light fainter than an
    /// eye's limit has the bits of the light fainter than the same limit.
    #[expect(
        clippy::too_many_arguments,
        reason = "one node's inputs: the tables, the layer, the point and its densities, the two \
                  limits, the light's age and what the node needs of the layer"
    )]
    #[must_use]
    fn layer_light(
        &self,
        tables: &LuminosityTables,
        l: usize,
        point: &PointLy,
        densities: &[f64; MAX_COMPONENTS],
        (limit, eye_limit): (Magnitudes, Option<Magnitudes>),
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
                if let Some(eye_limit) = eye_limit {
                    let sums = function.colour_sums_fainter_than(eye_limit, ago);
                    for (sum, value) in light.eye_fainter.iter_mut().zip(sums) {
                        *sum += systems * value;
                    }
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
    /// boundaries in turn, each taken at its radius towards `direction`): for each, the five sums ([`Sums`], in L☉,V × `lux_per_v0` per ly², before
    /// K) of the light fainter than the cut out to the radius plus all of the light beyond it, each
    /// node's light reddened by the call's dust; then, where the march keeps the eye's light, the
    /// same slots of the light fainter than the eye's cut out to the radius plus all of the light
    /// beyond it (R06.T9.j). A ray outside the cone's region (`reach`) is complete nowhere, so
    /// each of its slots holds all of its layer's light.
    ///
    /// Within a layer the light fainter than the cut is summed from the first node, and each
    /// radius's light beyond it from the radius out, interval by interval in distance order, so a
    /// radius's slot is the bits a march keeping it alone takes over the same nodes. The eye's
    /// slots share the light beyond each radius, and so are the bits of a march at the eye's cut.
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
        // The radii this ray keeps per layer, each of the march's boundaries towards its direction,
        // ascending and each once, with each boundary's place among them; and the radii its nodes
        // take, the march's and its own.
        let mut kept_ly = std::mem::take(&mut self.ray_kept);
        let mut places = std::mem::take(&mut self.ray_places);
        let mut node_radii = std::mem::take(&mut self.ray_nodes);
        node_radii.clear();
        for ((radii, places), boundaries) in kept_ly.iter_mut().zip(&mut places).zip(&edges.kept) {
            radii.clear();
            places.clear();
            match reach {
                Reach::Inside => {
                    // The boundaries' radii in the march's order go to the ray's nodes, and from
                    // there, once sorted, give each boundary its place.
                    let from = node_radii.len();
                    node_radii.extend(boundaries.iter().map(|b| b.toward(direction)));
                    radii.extend_from_slice(&node_radii[from..]);
                    ascending_once(radii);
                    places.extend(node_radii[from..].iter().map(|radius| {
                        radii
                            .binary_search_by(|r| r.total_cmp(radius))
                            .expect("a boundary's radius is among the ray's")
                    }));
                }
                Reach::Outside => radii.extend_from_slice(&NOWHERE_LY),
            }
        }
        if reach == Reach::Inside {
            node_radii.extend_from_slice(&edges.nodes);
        }
        let kept: [&[f64]; BAND_LAYERS] = std::array::from_fn(|l| kept_ly[l].as_slice());
        let starts = slot_starts(kept.map(<[f64]>::len));
        let eye_slots = if self.eye_cut.is_some() {
            starts[BAND_LAYERS]
        } else {
            0
        };
        self.within.clear();
        self.within.resize(starts[BAND_LAYERS], [0.0; 5]);
        self.beyond.clear();
        self.beyond.resize(starts[BAND_LAYERS], [0.0; 5]);
        self.eye_within.clear();
        self.eye_within.resize(eye_slots, [0.0; 5]);
        self.march_slots(galaxy, ctx, query, direction, &kept, &node_radii, spec);
        let ours = slot_starts(edges.kept.each_ref().map(Vec::len));
        let (band, eye) = out.split_at_mut(ours[BAND_LAYERS]);
        fill_slots(
            (&self.within, &self.beyond),
            &starts,
            &ours,
            &places,
            reach,
            band,
        );
        if self.eye_cut.is_some() {
            fill_slots(
                (&self.eye_within, &self.beyond),
                &starts,
                &ours,
                &places,
                reach,
                eye,
            );
        }
        self.ray_kept = kept_ly;
        self.ray_places = places;
        self.ray_nodes = node_radii;
    }

    /// Places the nodes of the ray from `observer` along `direction` to the root cube's edge, with
    /// `node_radii` among them, and their extinction profile, keeping its last node as the ray's
    /// A<sub>V</sub> to the edge; `false` for a ray that ends before its first node, which holds no
    /// light and no dust.
    ///
    /// The last node's bits are the profile's to the edge alone: [`profile`] plans its steps on the
    /// line to its last node and takes each cloud's column to it in closed form, so the radii taken
    /// as nodes, which a reply, a cone or a census moves, never move it (R06.T9.g).
    fn trace(
        &mut self,
        galaxy: &Galaxy,
        ctx: &mut SkyContext<'_>,
        observer: &Observer,
        direction: UnitVector,
        node_radii: &[f64],
        spec: BandSpec,
    ) -> bool {
        let origin = observer.position();
        let along = direction.components();
        let edge_ly = distance_to_edge_ly(origin.to_light_years_f64(), along);
        self.edge_a_v = Magnitudes::ZERO;
        if edge_ly <= FIRST_NODE_LY {
            return false;
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
        self.edge_a_v = self.a_v.last().copied().unwrap_or(Magnitudes::ZERO);
        true
    }

    /// The node-by-node march of one ray into [`Rays::within`] and [`Rays::beyond`], and
    /// [`Rays::eye_within`] where the march keeps the eye's light, whose slots are each layer's
    /// `kept` radii in turn and start at zero; `node_radii` are the radii taken as nodes. Each
    /// interval lies within or beyond each radius, since a radius is a node, or nearer than the
    /// first, or past the last. A radius's slot stops taking the fainter light, and starts taking
    /// all of it, at the first interval beyond it. The light fainter than the eye's cut is run as
    /// the light fainter than the cut is, at the eye's cut: the same nodes, limits and order.
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
        let from = observer.position().to_light_years_f64();
        let along = direction.components();
        if !self.trace(galaxy, ctx, observer, direction, node_radii, spec) {
            return;
        }
        let tables = ctx.tables;
        let cut = query.cut().value();
        let eye_cut = self.eye_cut;
        let starts = slot_starts(kept.map(<[f64]>::len));
        let mut densities = [0.0; MAX_COMPONENTS];
        // Per layer: the fainter light run from the first node, and the light fainter than the
        // eye's cut, how many of its radii the intervals so far have passed (each slot holding the
        // fainter light as it stood there), and the dimmed light at the previous node.
        let mut fainter = [[0.0; 5]; BAND_LAYERS];
        let mut eye_fainter = [[0.0; 5]; BAND_LAYERS];
        let mut passed = [0_usize; BAND_LAYERS];
        let mut before = [([0.0; 5], [0.0; 5], [0.0; 5]); BAND_LAYERS];
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
            // The census cuts each star's own V; the band, the solar point's (R06.T8.k). The eye's
            // limit is the same expression at the eye's cut, so its bits are a march's there.
            let (modulus, v_extinction) = (distance_modulus(distance), through.v_extinction());
            let limit = Magnitudes::new(cut - modulus - v_extinction.value());
            let eye_limit =
                eye_cut.map(|eye| Magnitudes::new(eye - modulus - v_extinction.value()));
            for (l, radii) in kept.iter().enumerate() {
                let needs = Needs::of(distance, radii);
                let light = self.layer_light(
                    tables,
                    l,
                    &point,
                    &densities,
                    (limit, eye_limit),
                    ago,
                    needs,
                );
                let here = light.dimmed(needs, &through);
                if let Some(last) = previous {
                    let width = distance - last;
                    let slots = starts[l]..starts[l + 1];
                    // The radii this interval lies beyond take the fainter light as it stands.
                    while passed[l] < radii.len() && radii[passed[l]] < distance {
                        self.within[slots.start + passed[l]] = fainter[l];
                        if eye_cut.is_some() {
                            self.eye_within[slots.start + passed[l]] = eye_fainter[l];
                        }
                        passed[l] += 1;
                    }
                    if needs.fainter {
                        add_to(&mut fainter[l], trapezoid(before[l].0, here.0, width));
                        if eye_cut.is_some() {
                            add_to(&mut eye_fainter[l], trapezoid(before[l].2, here.2, width));
                        }
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
            let rest = starts[l] + passed[l]..starts[l] + radii.len();
            for slot in &mut self.within[rest.clone()] {
                *slot = fainter[l];
            }
            if eye_cut.is_some() {
                for slot in &mut self.eye_within[rest] {
                    *slot = eye_fainter[l];
                }
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
/// complete only in the cone's region, the band's texels about it ([`SkyQuery::cone_region`];
/// R06.T8.l), whose stars alone its census lists: every ray of a texel
/// outside the region holds all of the light, whatever a reply's radii. `ctx` supplies the
/// luminosity tables, the gas modifiers and the noise cache of the rays' profiles; its other fields
/// are not read.
///
/// Where the query states the request's illumination ([`SkyQuery::illumination`]), the march also
/// keeps each ray's diffuse galactic light, from its direction and its A<sub>V</sub> to the edge
/// alone (R06.T9.g; [`super::dgl`]): the same whatever the replies, the cut, the eye or the cone.
///
/// Where the query asks the eye at a cut shallower than its own ([`SkyQuery::eye_cut`], a camera's
/// deeper cut setting the query's), the march also keeps, per layer and radius, the same sums of
/// the light fainter than the eye's cut, the eye's background (R06.T9.j): a second set of slots,
/// which doubles its heap. They take the arithmetic of a march at the eye's cut over the same
/// nodes, so [`sum_rows`] gives each texel the eye-only request's light beside its own, bit for
/// bit, at the same replies: the eye's map under a camera's cut is the eye-only request's at the
/// camera's census radii, its replies and `n_max` (decided 2026-10-07,
/// `decision-r06-t9c-glare.md`, addendum 2).
///
/// Each ray is a function of its own direction, so the marches of any split of a face's rows, each
/// summed, give the texels of one march over the face, bit for bit; the noise cache changes the
/// cost, never a value.
///
/// # Panics
///
/// If `replies` is empty, `rows` reaches past the face's last row, the query has a cone and
/// `spec` is not its [`SkyQuery::band_spec`], whose texels make the cone's region, the rays'
/// slots number more than the address space holds, or the query's illumination was marched for
/// another observer (which its builder refuses).
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
/// let replies = [first.clone(), last.clone()];
/// let march = march_rows(&galaxy, &mut ctx, &query, replies, &spec, CubeFace::PosZ, 0..64);
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
        query.illumination(),
    )
}

/// [`march_rows`] at `edges`, with each node's light reddened by the curves `dust`: the solar
/// point's for the band, a grey dust's for the tests' unreddened march; and each ray's diffuse
/// galactic light from `illumination`, if any (R06.T9.g), which must be of the query's observer.
#[expect(
    clippy::too_many_arguments,
    reason = "march_rows' inputs, its radii resolved, the dust's ratios and the illumination"
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
    illumination: Option<&Illumination>,
) -> BandMarch {
    let side = spec.face_texels();
    assert!(
        rows.end <= side,
        "rows {rows:?} reach past a face of {side} rows"
    );
    let region = query.cone_region();
    assert!(
        region.is_none_or(|region| region.spec() == spec),
        "a cone's band is marched at its query's band, {:?}, not at {spec:?}: its region is made \
         of that band's texels",
        query.band_spec()
    );
    debug_assert!(
        edges.kept.iter().flatten().all(|boundary| match boundary {
            Boundary::Uniform(radius) => edges
                .nodes
                .binary_search_by(|r| r.total_cmp(&radius.value()))
                .is_ok(),
            Boundary::ByRay(_) => true,
        }),
        "every radius a march keeps in every direction is a node of its rays: {edges:?}"
    );
    assert!(
        illumination.is_none_or(|light| light.observer() == query.observer()),
        "an illumination marched for {:?} lights a band for {:?}: the diffuse light is each \
         observer's own sky scattered",
        illumination.map(Illumination::observer),
        query.observer()
    );
    // The eye's light is kept only where a camera's deeper cut set the query's: at the eye's own
    // cut the band's light is the eye's (R06.T9.j).
    let eye_cut = query
        .eye_cut()
        .filter(|eye| eye.value() < query.cut().value());
    let ray_len = slot_starts(edges.kept.each_ref().map(Vec::len))[BAND_LAYERS] * lights(eye_cut);
    // Checked: on wasm32 a `usize` is 32 bits, and a band of many replies at a fine face could
    // pass it before its allocation is refused.
    let len = rows
        .len()
        .checked_mul(usize::from(side))
        .and_then(|texels| texels.checked_mul(ray_len))
        .expect("a march's slots number fewer than the address space holds");
    let mut sums = vec![[0.0; 5]; len];
    let rays_count = rows.len() * usize::from(side);
    let mut edge_a_v = Vec::with_capacity(if sums.is_empty() { 0 } else { rays_count });
    let mut diffuse = Vec::with_capacity(if sums.is_empty() || illumination.is_none() {
        0
    } else {
        rays_count
    });
    if !sums.is_empty() {
        let mut rays = Rays::new(galaxy, dust, eye_cut);
        let texels = rows
            .clone()
            .flat_map(|row| (0..side).map(move |column| (row, column)));
        for ((row, column), out) in texels.zip(sums.chunks_exact_mut(ray_len)) {
            let direction = spec.texel_direction(face, row, column);
            let reach = Reach::of(region, face, row, column);
            rays.march(galaxy, ctx, query, direction, &edges, reach, spec, out);
            edge_a_v.push(rays.edge_a_v);
            // The diffuse light reads only the ray's direction and its dust to the edge, so it is
            // the same whatever the cut, the replies, the cone or the census (R06.T9.g).
            if let Some(light) = illumination {
                diffuse.push(light.diffuse_toward(&direction, rays.edge_a_v));
            }
        }
    }
    BandMarch {
        spec,
        face,
        rows,
        origin: *query.observer().position(),
        kept: edges.kept,
        eye_cut,
        sums,
        edge_a_v,
        diffuse,
    }
}

/// The texels of a reply complete to `complete_to`, from `march`, appended to `out` row by row,
/// each row from its left (R06.T9.f; see the [module](self) documentation): each ray's sums at
/// each layer's radius, over the layers, then `census`'s overflow, the stars it kept past `n_max`,
/// as points in their texels, each reddened by its own colour (R06.T9.e). Where the march holds the
/// diffuse galactic light (R06.T9.g), each texel's and the eye's light take its ray's diffuse
/// sums, before the overflow, and [`BandTexel::diffuse_luminance`] gives the part. It reads no
/// profile and no luminosity table.
///
/// `census` is the reply's census, of the march's query, and `complete_to` the radii to which it is
/// complete. A march of any split of a face's rows, each summed, gives one march's texels over the
/// face, bit for bit, and a reply's texels are those of a march that keeps that reply alone over
/// the same nodes.
///
/// Where the march keeps the eye's light (a camera's deeper cut, R06.T9.j), each texel also takes
/// the light fainter than the eye's cut, as the eye's background: the eye's slots at the reply's
/// radii, and the overflow's stars at or brighter than the eye's cut, those an eye-only request
/// lists or overflows too, each placed and reddened as in the band. A listed or overflowing star
/// fainter than the eye's cut adds nothing to it: its light is there as expected light. The
/// census lists by V, so an eye-only census of the same radius is this one's stars at or brighter
/// than the eye's cut, in the same order, and the eye's light is, bit for bit, its texels'. The
/// texels' own light, chroma and ρ are as without it. The contract is the eye-only request's light
/// at the camera's census radii (the same replies and `n_max`): beyond the eye-only request's own caps, the camera's census lists
/// the real stars brighter than the eye's cut, under one expected a layer, in place of the expected
/// light the eye-only request holds there (decided 2026-10-07, `decision-r06-t9c-glare.md`,
/// addendum 2).
///
/// A census of some of its plan's shells ([`merge_shells`](super::census::merge_shells); R06.T8.i)
/// lists a layer not yet final by the radius towards each star's texel at its query's
/// [`band_spec`](SkyQuery::band_spec), the radius this band's ray through that texel reads: its
/// band is summed at that spec and at its own radii
/// ([`Completeness`](super::census::Completeness)), so that the two share one
/// boundary bit for bit (decided 2026-10-08, `decision-r06-t8i-listing.md`).
///
/// # Panics
///
/// - If the march does not keep a layer's radius of `complete_to` ([`BandMarch::holds`]).
/// - For a census of shells, if `complete_to` is not the radii its completeness states, or if some
///   layer is not yet final and the march is not at the census's band or from its observer.
pub fn sum_rows(
    march: &BandMarch,
    census: &SkyCensus,
    complete_to: &CompleteTo,
    out: &mut Vec<BandTexel>,
) {
    if let Some(completeness) = census.completeness() {
        assert!(
            completeness.complete_to().same(complete_to),
            "a census of shells is summed at the radii it is complete to"
        );
        assert!(
            completeness.is_final()
                || (completeness.band_spec() == march.spec
                    && completeness.observer().is_same_point(&march.origin)),
            "a census of shells not yet final lists by its query's band, {:?}, from its observer, \
             so its band is marched at it from there, not at {:?}",
            completeness.band_spec(),
            march.spec
        );
    }
    let slots = march.slots_of(complete_to);
    if march.sums.is_empty() {
        return;
    }
    let (spec, face, rows) = (march.spec, march.face, &march.rows);
    let width = usize::from(spec.face_texels());
    let to_luminance = light_to_lux_ly2();
    let one_light = march.slots();
    // Each texel's light, and the eye's where the march keeps it, each with the ray's diffuse
    // galactic light where the march has it (R06.T9.g): the same sums in both, before the
    // overflow, so the eye's light keeps the eye-only request's bits.
    let mut texels: Vec<(Sums, Option<Sums>, CandelasPerSquareMetre)> = march
        .sums
        .chunks_exact(march.ray_len())
        .enumerate()
        .map(|(i, ray)| {
            let (band, eye) = ray.split_at(one_light);
            let mut light = summed_light(band, &slots, to_luminance);
            let mut eye = march
                .eye_cut
                .map(|_| summed_light(eye, &slots, to_luminance));
            let mut diffuse = CandelasPerSquareMetre::ZERO;
            if let Some(scattered) = march.diffuse.get(i) {
                add_to(&mut light, *scattered);
                if let Some(eye) = &mut eye {
                    add_to(eye, *scattered);
                }
                diffuse = CandelasPerSquareMetre::new(scattered[0]);
            }
            (light, eye, diffuse)
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
        let (band, eye, _) = &mut texels[at];
        for (sum, lux) in band.iter_mut().zip(light) {
            *sum += lux / omega;
        }
        // An eye-only census keeps the stars at or brighter than its cut (R06.T8.k).
        if let (Some(eye), Some(eye_cut)) = (eye, march.eye_cut)
            && star.v() <= eye_cut
        {
            for (sum, lux) in eye.iter_mut().zip(light) {
                *sum += lux / omega;
            }
        }
    }
    out.extend(texels.into_iter().map(|(band, eye, diffuse)| {
        BandTexel::of_sums(band)
            .with_diffuse(diffuse)
            .with_eye_light(eye.zip(march.eye_cut))
    }));
}

/// The band's texels of rows `rows` (from the top) of `face`, appended to `out` row by row, each
/// row from its left (Design notes 14 and 15; see the [module](self) documentation): the march of
/// those rows at `complete_to`'s radii ([`march_rows`]), then its sum ([`sum_rows`]).
///
/// `census` is the census the band completes and `complete_to` the radii to which it is complete:
/// within them the band holds the light fainter than `query`'s cut, beyond them all of the light,
/// and the census's overflow as points. A query with a cone is complete only in the cone's region,
/// the band's texels about it: every ray of a texel outside it holds all of the light, and the
/// census lists only the stars of the region's texels (R06.T8.l). The light fainter than the cut
/// is taken below M<sub>V</sub> = cut − DM − v☉ A<sub>V</sub>, the solar point's V extinction
/// (R06.T8.k). The light is reddened by the dust in
/// front of it, each node's by the solar point's ratios and each overflow star's by its own
/// (R06.T9.e). Where the query states an illumination, each texel holds the diffuse galactic light
/// too (R06.T9.g). Where the query asks the eye at a cut shallower than its own, each texel also
/// holds the light fainter than the eye's cut, as the eye's background ([`march_rows`]; R06.T9.j).
/// `ctx` supplies the luminosity tables, the gas modifiers and the noise cache of the rays'
/// profiles; its other fields are not read.
///
/// Each texel is a function of its own ray and of the overflow, so the texels of any split of a
/// face's rows, appended in order, are one call's over the face, bit for bit; the noise cache
/// changes the cost, never a value.
///
/// # Panics
///
/// If `rows` reaches past the face's last row, or the query has a cone and `spec` is not its
/// [`SkyQuery::band_spec`].
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
        query.illumination(),
        out,
    );
}

/// [`band_rows`] with the diffuse galactic light of `illumination` (R06.T9.g), whatever the query
/// states: the eye cut's pre-pass, which holds the request's illumination by reference
/// ([`eye_cut`](super::limits::eye_cut)).
///
/// # Panics
///
/// As [`band_rows`], and if `illumination` was marched for another observer than the query's.
#[expect(
    clippy::too_many_arguments,
    reason = "band_rows' inputs and the request's illumination"
)]
pub(crate) fn band_rows_lit(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    census: &SkyCensus,
    complete_to: &CompleteTo,
    spec: BandSpec,
    face: CubeFace,
    rows: Range<u16>,
    illumination: Option<&Illumination>,
    out: &mut Vec<BandTexel>,
) {
    band_rows_through(
        galaxy,
        ctx,
        query,
        census,
        complete_to,
        spec,
        face,
        rows,
        &solar_colour().reddening(),
        illumination,
        out,
    );
}

/// [`band_rows`], with each node's light reddened by the curves `dust`: the solar point's for the
/// band, a grey dust's for the tests' unreddened march; and the diffuse light of `illumination`.
#[expect(
    clippy::too_many_arguments,
    reason = "band_rows' inputs, the dust's ratios and the illumination"
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
    illumination: Option<&Illumination>,
    out: &mut Vec<BandTexel>,
) {
    let march = march_rows_through(
        galaxy,
        ctx,
        query,
        Edges::of_replies([complete_to.clone()]),
        spec,
        face,
        rows,
        dust,
        illumination,
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
    use crate::sky::caps::{CAP_RAYS, CapLattice};
    use crate::sky::census::{
        CensusTallies, Cone, MAX_N_MAX, NoSkyCellCache, SkyStar, census_cell, census_plan,
        census_plan_of, merge_census, merge_shells,
    };
    use crate::sky::eye::surface_brightness;
    use crate::sky::testing::{
        milky_way_dark_tables, milky_way_envelope, milky_way_offsets, milky_way_tables,
        sun_illumination,
    };
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
                query.illumination(),
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

    /// Radii a ray of the standard lattice for each capped layer, 20 to 2,000 ly, even in ln r.
    fn caps_by_ray(seed: u64) -> Vec<LayerCap> {
        let lattice = std::sync::Arc::new(CapLattice::new(CAP_RAYS));
        CAPPED_LAYERS
            .iter()
            .zip(0_u64..)
            .map(|(&layer, k)| {
                let radii = crate::sky::testing::uniforms(seed + k)
                    .take(CAP_RAYS)
                    .map(|u| 20.0 * math::exp10(2.0 * u))
                    .collect();
                LayerCap::forced_by_ray(
                    layer,
                    RayRadii::new(std::sync::Arc::clone(&lattice), radii),
                )
            })
            .collect()
    }

    /// A census's caps of one radius a ray (R06.T7.b) are complete towards each direction to the
    /// caps' radius towards it, the radius their plan opened its cells by, and in any direction to
    /// their farthest ray's at most.
    #[test]
    fn complete_to_reads_each_layers_radius_towards_a_direction() {
        let caps = caps_by_ray(0x7b_0300);
        let complete = CompleteTo::of_caps(&caps);
        assert_eq!(complete, CompleteTo::of_caps(&caps.clone()));
        assert_ne!(complete, CompleteTo::of_caps(&caps_by_ray(0x7b_0400)));
        let spec = spec(16);
        for cap in &caps {
            assert_eq!(complete.radius(cap.layer()), cap.radius());
            for face in CubeFace::ALL {
                for (row, column) in [(0, 0), (7, 9), (15, 3)] {
                    let u = spec.texel_direction(face, row, column);
                    let towards = complete.radius_toward(cap.layer(), u);
                    assert_eq!(bits(towards.value()), bits(cap.radius_toward(u).value()));
                    assert!(towards <= cap.radius());
                }
            }
        }
        assert_eq!(
            complete.radius_toward(Layer::RoguePlanet, UnitVector::NORTH),
            LightYears::ZERO
        );
    }

    /// The per-ray table a reply states (R06.T11.d) is each cap's rays as they are, then held
    /// within a shell's edge, and none where the layer is one radius in every direction.
    #[test]
    fn a_layers_rays_are_its_caps_held_within_its_edge() {
        let caps = caps_by_ray(0x7b_0500);
        let complete = CompleteTo::of_caps(&caps);
        for cap in &caps {
            let rays = cap.rays().expect("a cap by ray").radii_ly();
            assert_eq!(
                complete.rays_ly(cap.layer()).map(<[f64]>::len),
                Some(CAP_RAYS)
            );
            let stated = complete.rays_ly(cap.layer()).expect("by ray");
            assert!(stated.iter().zip(rays).all(|(a, b)| bits(*a) == bits(*b)));
        }
        // Held within 100 ly: each ray the lesser of its radius and the edge, and B's, its rays
        // all reaching 15 ly, one radius.
        let mut within = [None; BAND_LAYERS];
        within[1] = Some(15.0);
        within[2] = Some(100.0);
        let held = CompleteTo::of_caps_within(&caps, within);
        assert_eq!(held.rays_ly(Layer::B), None);
        assert_eq!(held.radius(Layer::B), LightYears::new(15.0));
        let rays = caps[2].rays().expect("C by ray").radii_ly();
        let stated = held.rays_ly(Layer::C).expect("C by ray, some under 100 ly");
        assert!(
            stated
                .iter()
                .zip(rays)
                .all(|(a, b)| bits(*a) == bits(b.min(100.0)))
        );
        assert_eq!(CompleteTo::nowhere().rays_ly(Layer::C), None);
        assert_eq!(complete.rays_ly(Layer::RoguePlanet), None);
    }

    /// A reply complete to caps of one radius a ray (R06.T7.b) gives each texel the band of a
    /// reply complete in every direction to the caps' radius towards the texel's centre, bit for
    /// bit, and each ray that reply's light ([`BandMarch::ray_light`], the illumination's): the
    /// band reads the radius the census's plan opened its cells by, so that the band and the
    /// listing share one boundary. A march that keeps it beside a reply complete nowhere sums it
    /// to the same bits.
    #[test]
    fn a_reply_by_ray_takes_each_texels_radius_towards_it() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let query = query_at(SUN, CENSUS_CUT);
        let by_ray = CompleteTo::of_caps(&caps_by_ray(0x7b_0500));
        let census = SkyCensus::empty();
        let mut ctx = context();
        for face in [CubeFace::PosX, CubeFace::PosZ, CubeFace::NegY] {
            let mut texels = Vec::new();
            band_rows(
                galaxy,
                &mut ctx,
                &query,
                &census,
                &by_ray,
                &spec,
                face,
                0..2,
                &mut texels,
            );
            let march = march_rows(
                galaxy,
                &mut ctx,
                &query,
                [by_ray.clone(), CompleteTo::nowhere()],
                &spec,
                face,
                0..2,
            );
            assert!(march.holds(&by_ray) && march.holds(&CompleteTo::nowhere()));
            let mut summed = Vec::new();
            sum_rows(&march, &census, &by_ray, &mut summed);
            assert_eq!(texel_bits(&summed), texel_bits(&texels), "{face:?}");
            for row in 0..2 {
                for column in 0..8 {
                    let u = spec.texel_direction(face, row, column);
                    let uniform = CompleteTo {
                        radii: std::array::from_fn(|l| {
                            Boundary::Uniform(LightYears::new(by_ray.radii[l].toward(u)))
                        }),
                    };
                    let one = march_rows(
                        galaxy,
                        &mut ctx,
                        &query,
                        [uniform.clone()],
                        &spec,
                        face,
                        row..row + 1,
                    );
                    let mut alone = Vec::new();
                    sum_rows(&one, &census, &uniform, &mut alone);
                    let (at, column) = (
                        usize::from(row) * 8 + usize::from(column),
                        usize::from(column),
                    );
                    assert_eq!(
                        texel_bits(&texels[at..=at]),
                        texel_bits(&alone[column..=column]),
                        "{face:?} ({row}, {column})"
                    );
                    // The illumination's light per ray reads the same radius (R06.T9.g).
                    let (ours, theirs) = (march.ray_light(&by_ray), one.ray_light(&uniform));
                    assert_eq!(
                        ours[at].map(bits),
                        theirs[column].map(bits),
                        "{face:?} ({row}, {column})"
                    );
                }
            }
        }
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
        let nodes = Edges::of_replies(replies.clone()).nodes;
        let dust = solar_colour().reddening();
        let mut ctx = context();
        let mut bands: Vec<Vec<BandTexel>> = vec![Vec::new(); replies.len()];
        let mut owns: Vec<Vec<BandTexel>> = vec![Vec::new(); replies.len()];
        let mut quadrature = 0.0_f64;
        for face in CubeFace::ALL {
            let march = march_rows(galaxy, &mut ctx, &query, replies.clone(), &spec, face, 0..8);
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
                    kept: reply.radii.clone().map(|boundary| vec![boundary]),
                    nodes: nodes.clone(),
                };
                let mut fresh = Vec::new();
                sum_rows(
                    &march_rows_through(
                        galaxy,
                        &mut ctx,
                        &query,
                        alone,
                        spec,
                        face,
                        0..8,
                        &dust,
                        None,
                    ),
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

    /// Every float of `texels` as bits, with the eye's light ([`BandTexel::eye_background`]).
    fn texel_and_eye_bits(texels: &[BandTexel]) -> Vec<(u64, [u32; 2], u64, [u64; 2])> {
        texels
            .iter()
            .zip(texel_bits(texels))
            .map(|(t, (luminance, chroma, sp_ratio, _))| {
                let (eye, eye_ratio) = t.eye_background();
                (
                    luminance,
                    chroma,
                    sp_ratio,
                    [bits(eye.value()), bits(eye_ratio)],
                )
            })
            .collect()
    }

    /// Rows marched in any split, in any order and through a warm or a cold noise cache, each
    /// summed, give one march's texels over the face for every reply it keeps, bit for bit, the
    /// overflow's points split with them (R06.T9.f); and so does a march of the replies in reverse
    /// order with one of them twice. So does the eye's light beside each texel, of a request whose
    /// eye's cut, 6.5, is shallower than its own (R06.T9.j). Two of the replies are of caps by ray
    /// (R06.T7.b).
    #[test]
    fn a_march_in_any_split_of_the_rows_sums_to_the_same_bits() {
        let eye = SkyQuery::builder(observer_at(SUN), Magnitudes::new(CENSUS_CUT))
            .eye(crate::sky::eye::EyeObserver::default())
            .eye_cut(Magnitudes::new(6.5))
            .build()
            .expect("a valid query");
        for query in [query_at(SUN, CENSUS_CUT), eye] {
            any_split_of_the_rows_sums_to_the_same_bits(&query);
        }
    }

    /// [`a_march_in_any_split_of_the_rows_sums_to_the_same_bits`] for `query`.
    fn any_split_of_the_rows_sums_to_the_same_bits(query: &SkyQuery) {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let census = census_to(CENSUS_CUT, 50.0, n(20), Eye::NotAsked);
        // The near-Sun replies, and two of caps by ray (R06.T7.b), whose boundaries a march keeps
        // in the order they first come and each ray reads towards its own direction.
        let mut replies = replies_near_the_sun().to_vec();
        replies.extend([0x7b_0600, 0x7b_0700].map(|seed| CompleteTo::of_caps(&caps_by_ray(seed))));
        let mut ctx = context();
        let sums = |march: &BandMarch, reply: &CompleteTo| {
            let mut out = Vec::new();
            sum_rows(march, &census, reply, &mut out);
            texel_and_eye_bits(&out)
        };
        let mut reordered = replies.clone();
        reordered.reverse();
        reordered.push(replies[2].clone());
        for face in CubeFace::ALL {
            let whole = march_rows(galaxy, &mut ctx, query, replies.clone(), &spec, face, 0..8);
            let shallower = query.eye_cut().filter(|eye| *eye < query.cut());
            assert_eq!(
                whole.eye_cut(),
                shallower,
                "the eye's light kept where its cut is shallower"
            );
            let shuffled = march_rows(
                galaxy,
                &mut ctx,
                query,
                reordered.clone(),
                &spec,
                face,
                0..8,
            );
            assert_eq!(shuffled.slots(), whole.slots());
            for reply in &replies {
                assert_eq!(
                    sums(&shuffled, reply),
                    sums(&whole, reply),
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
                    .map(|rows| {
                        march_rows(
                            galaxy,
                            ctx,
                            query,
                            replies.clone(),
                            &spec,
                            face,
                            rows.clone(),
                        )
                    })
                    .collect();
                parts.sort_by_key(|part| part.rows().start);
                for reply in &replies {
                    let joined: Vec<_> = parts.iter().flat_map(|part| sums(part, reply)).collect();
                    assert_eq!(joined, sums(&whole, reply), "{face:?}, split {split:?}");
                }
            }
        }
    }

    /// The eye's light from the march of a request whose camera's cut is deeper than the eye's
    /// (R06.T9.j), on two pairs of cuts: a camera at V 10.06 beside the eye's 8.15 with no census,
    /// and 8.0 beside 6.5 with a census within 50 ly whose brightest 20 are listed, so that both
    /// requests overflow and the deeper one's overflow holds stars between the cuts too. For every
    /// reply of [`replies_near_the_sun`]:
    ///
    /// - every texel's eye light, `sum_rows` at the eye's cut from the deeper march, is the texel
    ///   of the march at the eye's cut, luminance and ρ bit for bit, its census the deeper one's
    ///   stars at or brighter than the eye's cut;
    /// - the band's texels, as sent, are those of the deeper march without the eye's light, bit
    ///   for bit;
    /// - the march keeps a second set of slots, and a request whose eye's cut is its own keeps
    ///   none.
    ///
    /// With no census, the eye's light is brighter than the band's own in every texel complete
    /// everywhere, by the light between the cuts.
    #[test]
    fn every_sum_at_the_eyes_cut_from_a_deeper_march_is_the_march_at_the_eyes_cut() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let replies = replies_near_the_sun();
        let eye = crate::sky::eye::EyeObserver::default();
        let asked = |cut: f64, eye_cut: f64| {
            SkyQuery::builder(observer_at(SUN), Magnitudes::new(cut))
                .eye(eye)
                .eye_cut(Magnitudes::new(eye_cut))
                .build()
                .expect("a valid query")
        };
        let census_at = |cut: f64| {
            let stars: Vec<SkyStar> = census_stars(Eye::NotAsked)
                .iter()
                .filter(|s| s.v().value() <= cut && s.distance().value() <= 50.0)
                .copied()
                .collect();
            merge_census([(stars, CensusTallies::default())], n(20))
        };
        let (deep_census, shallow_census) = (census_at(CENSUS_CUT), census_at(6.5));
        assert!(
            !shallow_census.overflow().is_empty()
                && deep_census.overflow().iter().any(|s| s.v().value() > 6.5),
            "both overflow, the deeper one with stars between the cuts too"
        );
        let mut ctx = context();
        for (camera, eye_cut, deep_census, shallow_census) in [
            (10.06, 8.15, &SkyCensus::empty(), &SkyCensus::empty()),
            (CENSUS_CUT, 6.5, &deep_census, &shallow_census),
        ] {
            let mut brighter = (0_usize, f64::INFINITY);
            for face in CubeFace::ALL {
                let march = |query: &SkyQuery, ctx: &mut SkyContext<'_>| {
                    march_rows(galaxy, ctx, query, replies.clone(), &spec, face, 0..8)
                };
                let deep = march(&asked(camera, eye_cut), &mut ctx);
                let plain = march(&query_at(SUN, camera), &mut ctx);
                let shallow = march(&asked(eye_cut, eye_cut), &mut ctx);
                assert_eq!(deep.eye_cut, Some(Magnitudes::new(eye_cut)));
                assert_eq!((plain.eye_cut, shallow.eye_cut), (None, None));
                assert_eq!(deep.ray_len(), 2 * plain.ray_len());
                assert_eq!(shallow.ray_len(), shallow.slots());
                assert!(deep.heap_bytes() >= 2 * plain.sums.len() * size_of::<Sums>());
                for (k, reply) in replies.iter().enumerate() {
                    let sum = |march: &BandMarch, census: &SkyCensus| {
                        let mut out = Vec::new();
                        sum_rows(march, census, reply, &mut out);
                        out
                    };
                    let (texels, alone) = (sum(&deep, deep_census), sum(&plain, deep_census));
                    let at_the_eyes_cut = sum(&shallow, shallow_census);
                    let what = format!("cuts {camera} and {eye_cut}, {face:?}, reply {k}");
                    assert_eq!(texel_bits(&texels), texel_bits(&alone), "{what}: as sent");
                    assert!(alone.iter().all(|t| t.eye_light.is_none()), "{what}");
                    let eye_light = |texels: &[BandTexel]| -> Vec<[u64; 2]> {
                        texels
                            .iter()
                            .map(|t| {
                                let (luminance, sp_ratio) = t.eye_background();
                                [bits(luminance.value()), bits(sp_ratio)]
                            })
                            .collect()
                    };
                    assert!(texels.iter().all(|t| t.eye_light.is_some()), "{what}");
                    assert_eq!(
                        eye_light(&texels),
                        eye_light(&at_the_eyes_cut),
                        "{what}: the eye's light"
                    );
                    assert!(at_the_eyes_cut.iter().all(|t| t.eye_light.is_none()));
                    if deep_census.overflow().is_empty() && *reply == CompleteTo::everywhere() {
                        for texel in &texels {
                            let ratio =
                                texel.eye_background().0.value() / texel.luminance().value();
                            assert!(ratio > 1.0, "{what}: {ratio}");
                            brighter = (brighter.0 + 1, brighter.1.min(ratio));
                        }
                    }
                }
            }
            if deep_census.overflow().is_empty() {
                eprintln!(
                    "a camera to V {camera} beside the eye's {eye_cut}: complete everywhere, the \
                     eye's light is brighter than the band's in all {} texels, by {:.3} times at \
                     least",
                    brighter.0, brighter.1
                );
                assert_eq!(brighter.0, CubeFace::ALL.len() * 64);
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
            .map(|&face| march_rows(galaxy, &mut ctx, &query, replies.clone(), &spec, face, 0..8))
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

    /// A census of a cone lists stars only in its region, the band's texels about it: in
    /// them the band is the full sky's, complete as asked, and outside them it holds all of the
    /// light, bit for bit (R06.T8.k, R06.T8.l).
    #[test]
    fn a_cone_is_complete_only_in_its_region() {
        let galaxy = milky_way_galaxy();
        let spec = spec(8);
        let observer = observer_at(SUN);
        let cone = Cone::new(UnitVector::X, Degrees::new(30.0)).expect("a cone");
        let narrow = SkyQuery::builder(observer, Magnitudes::new(9.0))
            .cone(cone)
            .band_spec(spec)
            .build()
            .expect("a valid query");
        let region = *narrow.cone_region().expect("a region");
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
            let within = math::cos(30.0 * RADIANS_PER_DEGREE + spec.largest_texel_radius().value());
            assert_eq!(
                region.holds_texel(CubeFace::PosX, row, column),
                u.dot(&UnitVector::X) >= within
            );
            if region.holds_texel(CubeFace::PosX, row, column) {
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

    /// Every float of a star that the band or a view reads, as bits.
    fn star_bits(stars: &[SkyStar]) -> Vec<u64> {
        stars
            .iter()
            .flat_map(|s| {
                let at = s.apparent().to_light_years_f64();
                [
                    bits(s.v().value()),
                    bits(s.a_v().value()),
                    bits(s.distance().value()),
                    bits(at[0]),
                    bits(at[1]),
                    bits(at[2]),
                ]
            })
            .collect()
    }

    /// The query near the Sun to V [`CENSUS_CUT`] with `cone`, its region of a band of `spec`.
    fn cone_query(cone: Cone, spec: BandSpec) -> SkyQuery {
        SkyQuery::builder(observer_at(SUN), Magnitudes::new(CENSUS_CUT))
            .cone(cone)
            .band_spec(spec)
            .build()
            .expect("a valid query")
    }

    /// A band's texel beside its face, row and column.
    type Placed = ((CubeFace, u16, u16), BandTexel);

    /// The band of `query` and `census` complete to `complete_to` at `spec`, over each face's rows
    /// of `rows` in turn, with each texel's face, row and column beside it.
    fn band_over(
        query: &SkyQuery,
        census: &SkyCensus,
        complete_to: &CompleteTo,
        spec: BandSpec,
        rows: &[(CubeFace, Range<u16>)],
    ) -> Vec<Placed> {
        let mut ctx = context();
        let mut band = Vec::new();
        let mut places = Vec::new();
        for (face, rows) in rows {
            band_rows(
                milky_way_galaxy(),
                &mut ctx,
                query,
                census,
                complete_to,
                &spec,
                *face,
                rows.clone(),
                &mut band,
            );
            for row in rows.clone() {
                places.extend((0..spec.face_texels()).map(|column| (*face, row, column)));
            }
        }
        assert_eq!(band.len(), places.len());
        places.into_iter().zip(band).collect()
    }

    /// The stars of `census` the band would hold within [`CENSUS_RADIUS_LY`], every one listed:
    /// those of `stars` within the radius.
    fn listed_within_the_radius(stars: &[SkyStar]) -> SkyCensus {
        let within: Vec<SkyStar> = stars
            .iter()
            .filter(|s| s.distance().value() <= CENSUS_RADIUS_LY)
            .copied()
            .collect();
        let census = merge_census([(within, CensusTallies::default())], n(MAX_N_MAX));
        assert!(census.overflow().is_empty());
        census
    }

    /// The full sky near the Sun at 16²: its census to V [`CENSUS_CUT`] within
    /// [`CENSUS_RADIUS_LY`], complete to it, with no cone; and the band complete nowhere, which
    /// holds all of the light. Built once.
    fn full_and_nowhere_at_16() -> &'static [Vec<Placed>; 2] {
        static BANDS: OnceLock<[Vec<Placed>; 2]> = OnceLock::new();
        BANDS.get_or_init(|| {
            let spec = spec(16);
            let rows = every_row(spec);
            let query = query_near_the_sun(CENSUS_CUT, Eye::NotAsked);
            let full = listed_within_the_radius(census_stars(Eye::NotAsked));
            [
                band_over(
                    &query,
                    &full,
                    &complete_within(CENSUS_RADIUS_LY),
                    spec,
                    &rows,
                ),
                band_over(
                    &query,
                    &SkyCensus::empty(),
                    &CompleteTo::nowhere(),
                    spec,
                    &rows,
                ),
            ]
        })
    }

    /// Every row of every face of a band of `spec`.
    fn every_row(spec: BandSpec) -> Vec<(CubeFace, Range<u16>)> {
        CubeFace::ALL
            .iter()
            .map(|&face| (face, 0..spec.face_texels()))
            .collect()
    }

    /// The rows of each face holding a texel of `region`, from its first such row to its last.
    fn rows_meeting(region: &ConeRegion) -> Vec<(CubeFace, Range<u16>)> {
        let side = region.spec().face_texels();
        CubeFace::ALL
            .iter()
            .filter_map(|&face| {
                let met: Vec<u16> = (0..side)
                    .filter(|&row| (0..side).any(|column| region.holds_texel(face, row, column)))
                    .collect();
                Some((face, *met.first()?..*met.last()? + 1))
            })
            .collect()
    }

    /// 10⁴ directions inside `cone`: 100 rings at (k + ½) hundredths of its half-angle from the
    /// axis, of 100 directions each, as displacements of a metre.
    fn lattice_inside(cone: &Cone) -> Vec<GalacticDisplacement> {
        let axis = cone.axis();
        let other = if axis.components()[0].abs() > 0.9 {
            UnitVector::NORTH
        } else {
            UnitVector::X
        };
        let across = UnitVector::from_components(axis.cross(&other)).expect("not along the other");
        let third = UnitVector::from_components(axis.cross(&across)).expect("perpendicular");
        let (u, a, b) = (axis.components(), across.components(), third.components());
        let half_angle = cone.half_angle().value() * RADIANS_PER_DEGREE;
        let mut lattice = Vec::with_capacity(10_000);
        for ring in 0..100_u32 {
            let (sin_t, cos_t) = math::sin_cos(half_angle * (f64::from(ring) + 0.5) / 100.0);
            for step in 0..100_u32 {
                let (sin_p, cos_p) =
                    math::sin_cos(2.0 * core::f64::consts::PI * f64::from(step) / 100.0);
                lattice.push(GalacticDisplacement::new(std::array::from_fn(|k| {
                    cos_t * u[k] + sin_t * (cos_p * a[k] + sin_p * b[k])
                })));
            }
        }
        lattice
    }

    /// What a cone's sky shares with the full sky's near the Sun, texel by texel.
    #[derive(Debug)]
    struct ConeRecord {
        /// The region's texels and their solid angle, sr.
        texels: usize,
        region_sr: f64,
        /// The cone's own solid angle, sr; the ruling's (1 + ρ ÷ α)², the region's mean over
        /// cones; and the bound on the region's, the cap of α + 2ρ over the cone, which its
        /// texels reach.
        cone_sr: f64,
        estimate: f64,
        bound: f64,
        /// The stars the cone's census lists, and those of them inside the cone itself.
        listed: usize,
        inside: usize,
        /// The cells the cone's plan opens, and the full sky's.
        cells: u64,
        full_cells: u64,
    }

    /// A cone's census and band share the band's texels exactly (R06.T8.l; decided 2026-10-07,
    /// `decision-r06-t8k-cone.md`, item 1): near the Sun to V [`CENSUS_CUT`] within
    /// [`CENSUS_RADIUS_LY`], every cap forced to it, for `cone` at `spec` over `rows`, against the
    /// full sky's band `full` and the band complete nowhere `nowhere` over the same rows:
    ///
    /// 1. the cone's census lists exactly the full census's stars whose texels
    ///    ([`BandSpec::texel_of`]) are in the region, star for star and bit for bit, each at or
    ///    brighter than the cut;
    /// 2. each region texel's band is the full sky's texel, bit for bit, and each other texel the
    ///    band's complete nowhere, bit for bit;
    /// 3. so, texel by texel, the listed and band light are the full sky's in the region and all
    ///    of the light outside it, to 10⁻¹² relative;
    /// 4. every direction of a lattice of 10⁴ inside the cone lies in a region texel;
    /// 5. the full census's stars inside the cone itself are all listed.
    fn assert_a_cone_shares_the_bands_texels(
        cone: Cone,
        spec: BandSpec,
        rows: &[(CubeFace, Range<u16>)],
        [full, nowhere]: [&[Placed]; 2],
    ) -> ConeRecord {
        let query = cone_query(cone, spec);
        let region = *query.cone_region().expect("a region");
        let origin = *observer_at(SUN).position();
        let toward = |star: &SkyStar| origin.displacement_to(star.apparent());
        let what = format!(
            "a {}° cone at {}²",
            cone.half_angle().value(),
            spec.face_texels()
        );
        // 1. The cone's stars: the full census's of the region's texels.
        let coned = listed_with_caps_forced(&query, CENSUS_RADIUS_LY);
        let all = census_stars(Eye::NotAsked);
        let in_region: Vec<SkyStar> = all
            .iter()
            .filter(|s| region.holds(&toward(s)))
            .copied()
            .collect();
        assert!(coned.iter().all(|s| s.v().value() <= CENSUS_CUT), "{what}");
        assert_eq!(
            coned, in_region,
            "{what}: the full census's stars of the region"
        );
        assert_eq!(star_bits(&coned), star_bits(&in_region), "{what}");
        // 2 and 3. The band and the light, texel by texel.
        let (cone_sky, full_sky) = (
            listed_within_the_radius(&coned),
            listed_within_the_radius(all),
        );
        let band = band_over(
            &query,
            &cone_sky,
            &complete_within(CENSUS_RADIUS_LY),
            spec,
            rows,
        );
        let (texels, region_sr) = assert_texel_by_texel(
            &what,
            &region,
            &band,
            [full, nowhere],
            [&cone_sky, &full_sky],
        );
        let inside = assert_the_field_is_complete(&what, &region, all, &coned);
        let cells = |query: &SkyQuery| {
            let mut ctx = context();
            let query = query
                .clone()
                .with_caps_forced(LightYears::new(CENSUS_RADIUS_LY))
                .expect("a forced cap");
            census_plan(
                milky_way_galaxy(),
                ctx.tables,
                ctx.envelope,
                &query,
                &mut ctx.noise,
            )
            .cell_count()
        };
        let alpha = cone.half_angle().value() * RADIANS_PER_DEGREE;
        let rho = spec.largest_texel_radius().value();
        let cap = |angle: f64| 2.0 * core::f64::consts::PI * (1.0 - math::cos(angle));
        let record = ConeRecord {
            texels,
            region_sr,
            cone_sr: cap(alpha),
            estimate: math::powi(1.0 + rho / alpha, 2),
            bound: cap(alpha + 2.0 * rho) / cap(alpha),
            listed: coned.len(),
            inside,
            cells: cells(&query),
            full_cells: cells(&query_near_the_sun(CENSUS_CUT, Eye::NotAsked)),
        };
        eprintln!(
            "{what}: its region is {} texels, {:.4e} sr, {:.2} times the cone's {:.4e} sr (the \
             ruling's (1 + ρ ÷ α)² {:.2}, at most {:.2}); it lists {} stars, the full census's of \
             those texels bit for bit, {} of them inside the cone; its plan opens {} cells of the \
             full sky's {}",
            record.texels,
            record.region_sr,
            record.region_sr / record.cone_sr,
            record.cone_sr,
            record.estimate,
            record.bound,
            record.listed,
            record.inside,
            record.cells,
            record.full_cells
        );
        assert!(record.texels > 0, "{record:?}");
        assert!(
            record.region_sr <= record.bound * record.cone_sr,
            "{record:?}"
        );
        record
    }

    /// Items 4 and 5 of [`assert_a_cone_shares_the_bands_texels`] for the cone's `region`, the full
    /// census's stars `all` and the cone's `coned`: the field is complete, every direction of the
    /// lattice inside the cone lying in a region texel, and the full census's stars inside the
    /// cone itself are listed, as many as are returned.
    fn assert_the_field_is_complete(
        what: &str,
        region: &ConeRegion,
        all: &[SkyStar],
        coned: &[SkyStar],
    ) -> usize {
        let cone = region.cone();
        for direction in lattice_inside(cone) {
            assert!(
                cone.holds(&direction),
                "{what}: {direction:?} inside the cone"
            );
            assert!(
                region.holds(&direction),
                "{what}: {direction:?} in the region"
            );
        }
        let origin = *observer_at(SUN).position();
        let inside: Vec<&SkyStar> = all
            .iter()
            .filter(|s| cone.holds(&origin.displacement_to(s.apparent())))
            .collect();
        assert!(
            inside.iter().all(|s| coned.contains(s)),
            "{what}: the stars inside the cone"
        );
        inside.len()
    }

    /// Items 2 and 3 of [`assert_a_cone_shares_the_bands_texels`] for the cone's `region` and
    /// `band`, against the full sky's band and the band complete nowhere over the same texels,
    /// with the cone's census and the full one, each within the radius: the region's texels and
    /// their solid angle, sr.
    fn assert_texel_by_texel(
        what: &str,
        region: &ConeRegion,
        band: &[Placed],
        [full, nowhere]: [&[Placed]; 2],
        [cone_sky, full_sky]: [&SkyCensus; 2],
    ) -> (usize, f64) {
        let spec = region.spec();
        let origin = *observer_at(SUN).position();
        // Each texel's listed light, lux, its stars placed as the band places its overflow's.
        let starlight = |census: &SkyCensus| {
            let mut light = std::collections::BTreeMap::new();
            for star in census.listed() {
                let toward = origin.displacement_to(star.apparent());
                let place = spec.texel_of(toward.metres()).expect("a direction");
                *light.entry(place).or_insert(0.0) += stars_lux(std::slice::from_ref(star));
            }
            light
        };
        let (cone_stars, full_stars) = (starlight(cone_sky), starlight(full_sky));
        assert!(
            cone_stars
                .keys()
                .all(|&(face, row, column)| region.holds_texel(face, row, column)),
            "{what}: a listed star outside the region"
        );
        let (mut texels, mut region_sr) = (0_usize, 0.0);
        for (k, (((place, texel), (at_full, full_texel)), (at_nowhere, nowhere_texel))) in
            band.iter().zip(full).zip(nowhere).enumerate()
        {
            assert_eq!(
                (place, place),
                (at_full, at_nowhere),
                "{what}: the same texels"
            );
            let &(face, row, column) = place;
            let omega = spec.texel_solid_angle_sr(row, column);
            let held = region.holds_texel(face, row, column);
            let (expected, stars) = if held {
                texels += 1;
                region_sr += omega;
                (full_texel, full_stars.get(place).copied().unwrap_or(0.0))
            } else {
                (nowhere_texel, 0.0)
            };
            assert_eq!(
                texel_bits(std::slice::from_ref(texel)),
                texel_bits(std::slice::from_ref(expected)),
                "{what}: texel {k} {place:?}, in the region: {held}"
            );
            let light =
                texel.luminance().value() * omega + cone_stars.get(place).copied().unwrap_or(0.0);
            let theirs = expected.luminance().value() * omega + stars;
            assert!(
                (light / theirs - 1.0).abs() <= 1e-12,
                "{what}: texel {place:?}, {light} lx against {theirs} lx"
            );
        }
        (texels, region_sr)
    }

    /// The brightest star the full census near the Sun lists within [`CENSUS_RADIUS_LY`] whose
    /// direction lies more than `degrees` from every texel centre of a band of `spec`: the axis of
    /// a cone of that half-angle holding no texel centre and a star.
    fn a_star_between_the_texel_centres(spec: BandSpec, degrees: f64) -> UnitVector {
        let origin = *observer_at(SUN).position();
        let side = spec.face_texels();
        let centres: Vec<UnitVector> = CubeFace::ALL
            .iter()
            .flat_map(|&face| {
                (0..side).flat_map(move |row| {
                    (0..side).map(move |column| spec.texel_direction(face, row, column))
                })
            })
            .collect();
        let cos = math::cos(degrees * RADIANS_PER_DEGREE);
        census_stars(Eye::NotAsked)
            .iter()
            .filter(|s| s.distance().value() <= CENSUS_RADIUS_LY)
            .map(|s| {
                UnitVector::from_components(origin.displacement_to(s.apparent()).metres())
                    .expect("a star away from the observer")
            })
            .find(|u| centres.iter().all(|c| c.dot(u) < cos))
            .expect("a star between the texel centres")
    }

    /// A 1° cone at 16², narrower than a texel and holding no texel's centre, about a star near
    /// the Sun: its region is the texels about it, which the census lists and the band completes
    /// exactly (R06.T8.l). Keeping a star by its texel's centre would have listed nothing.
    #[test]
    fn a_1_degree_cone_holding_no_texel_centre_shares_the_bands_texels_exactly() {
        let spec = spec(16);
        let cone = Cone::new(
            a_star_between_the_texel_centres(spec, 2.0),
            Degrees::new(1.0),
        )
        .expect("a cone");
        let [full, nowhere] = full_and_nowhere_at_16();
        let record = assert_a_cone_shares_the_bands_texels(
            cone,
            spec,
            &every_row(spec),
            [full.as_slice(), nowhere.as_slice()],
        );
        let side = spec.face_texels();
        for face in CubeFace::ALL {
            for row in 0..side {
                for column in 0..side {
                    let centre = spec.texel_direction(face, row, column);
                    assert!(
                        !cone.holds(&GalacticDisplacement::new(centre.components())),
                        "a texel centre inside the cone"
                    );
                }
            }
        }
        assert!(record.inside > 0, "{record:?}");
    }

    /// A 10° cone at 16² about a corner of the cube, where three faces meet (R06.T8.l).
    #[test]
    fn a_10_degree_cone_shares_the_bands_texels_exactly() {
        let spec = spec(16);
        let corner = UnitVector::from_components([1.0, 1.0, 1.0]).expect("a direction");
        let cone = Cone::new(corner, Degrees::new(10.0)).expect("a cone");
        let [full, nowhere] = full_and_nowhere_at_16();
        let record = assert_a_cone_shares_the_bands_texels(
            cone,
            spec,
            &every_row(spec),
            [full.as_slice(), nowhere.as_slice()],
        );
        let region = *cone_query(cone, spec).cone_region().expect("a region");
        let faces: Vec<CubeFace> = rows_meeting(&region)
            .iter()
            .map(|(face, _)| *face)
            .collect();
        assert_eq!(faces, [CubeFace::PosX, CubeFace::PosY, CubeFace::PosZ]);
        assert!(record.inside > 0, "{record:?}");
    }

    /// A cone's overflow falls in its region (R06.T8.l; the ruling's "the overflow's points read
    /// the same region"): the 10° cone at 16² about the cube's corner, its census within
    /// [`CENSUS_RADIUS_LY`] merged at an `n_max` of 10, so that most of its stars are the band's
    /// points. Every overflowing star's texel, as the band places it, is in the region; every
    /// texel outside it is the band complete nowhere, bit for bit; and each region texel's listed
    /// and band light, its points among it, is the full sky's to 10⁻¹² relative.
    #[test]
    fn a_cones_overflow_falls_in_its_region() {
        let spec = spec(16);
        let corner = UnitVector::from_components([1.0, 1.0, 1.0]).expect("a direction");
        let cone = Cone::new(corner, Degrees::new(10.0)).expect("a cone");
        let query = cone_query(cone, spec);
        let region = *query.cone_region().expect("a region");
        let origin = *observer_at(SUN).position();
        let place_of = |star: &SkyStar| {
            spec.texel_of(origin.displacement_to(star.apparent()).metres())
                .expect("a direction")
        };
        let within: Vec<SkyStar> =
            listed_within_the_radius(&listed_with_caps_forced(&query, CENSUS_RADIUS_LY))
                .listed()
                .to_vec();
        let census = merge_census([(within, CensusTallies::default())], n(10));
        assert!(
            census.overflow().len() > census.listed().len(),
            "most overflow"
        );
        for star in census.overflow() {
            let (face, row, column) = place_of(star);
            assert!(
                region.holds_texel(face, row, column),
                "{star:?} overflows outside"
            );
        }
        let band = band_over(
            &query,
            &census,
            &complete_within(CENSUS_RADIUS_LY),
            spec,
            &every_row(spec),
        );
        let [full, nowhere] = full_and_nowhere_at_16();
        let full_sky = listed_within_the_radius(census_stars(Eye::NotAsked));
        let lux_in = |stars: &[SkyStar], at: (CubeFace, u16, u16)| -> f64 {
            stars
                .iter()
                .filter(|s| place_of(s) == at)
                .map(|s| stars_lux(std::slice::from_ref(s)))
                .sum()
        };
        let mut points = 0_usize;
        for (((place, texel), (_, full_texel)), (_, nowhere_texel)) in
            band.iter().zip(full).zip(nowhere)
        {
            let &(face, row, column) = place;
            if !region.holds_texel(face, row, column) {
                assert_eq!(
                    texel_bits(std::slice::from_ref(texel)),
                    texel_bits(std::slice::from_ref(nowhere_texel)),
                    "{place:?} outside the region"
                );
                continue;
            }
            let omega = spec.texel_solid_angle_sr(row, column);
            let light = texel.luminance().value() * omega + lux_in(census.listed(), *place);
            let theirs = full_texel.luminance().value() * omega + lux_in(full_sky.listed(), *place);
            assert!(
                (light / theirs - 1.0).abs() <= 1e-12,
                "{place:?}: {light} lx against {theirs} lx"
            );
            if texel_bits(std::slice::from_ref(texel))
                != texel_bits(std::slice::from_ref(full_texel))
            {
                points += 1;
            }
        }
        eprintln!(
            "a 10° cone at 16² listing 10 of its {} stars: {} texels hold its points, each with \
             the full sky's light",
            census.listed().len() + census.overflow().len(),
            points
        );
        assert!(points > 0, "the overflow's points are in the band");
    }

    /// A 30° cone at 16² along +X, T8.k's, now shared by the band's texels exactly (R06.T8.l):
    /// T8.k's test held its listed and band light to the full sky's within 1%.
    #[test]
    fn a_30_degree_cone_shares_the_bands_texels_exactly() {
        let spec = spec(16);
        let cone = Cone::new(UnitVector::X, Degrees::new(30.0)).expect("a cone");
        let [full, nowhere] = full_and_nowhere_at_16();
        let record = assert_a_cone_shares_the_bands_texels(
            cone,
            spec,
            &every_row(spec),
            [full.as_slice(), nowhere.as_slice()],
        );
        assert!(record.inside > 0, "{record:?}");
    }

    /// A 0.3° cone at the server's 64², about the brightest star the census lists near the Sun,
    /// on the rows its region meets (R06.T8.l): about 27 times the cone's area on average over
    /// axes (29 here), and at most the cap of α + 2ρ, 89 times, the cost of a sub-degree field in
    /// a band of 1.3° texels.
    #[test]
    fn a_0_3_degree_cone_at_64_shares_the_bands_texels_exactly() {
        let spec = BandSpec::STANDARD;
        let origin = *observer_at(SUN).position();
        let brightest = census_stars(Eye::NotAsked)
            .iter()
            .find(|s| s.distance().value() <= CENSUS_RADIUS_LY)
            .expect("a star");
        let axis =
            UnitVector::from_components(origin.displacement_to(brightest.apparent()).metres())
                .expect("a direction");
        let cone = Cone::new(axis, Degrees::new(0.3)).expect("a cone");
        let region = *cone_query(cone, spec).cone_region().expect("a region");
        let rows = rows_meeting(&region);
        let query = query_near_the_sun(CENSUS_CUT, Eye::NotAsked);
        let full = band_over(
            &query,
            &listed_within_the_radius(census_stars(Eye::NotAsked)),
            &complete_within(CENSUS_RADIUS_LY),
            spec,
            &rows,
        );
        let nowhere = band_over(
            &query,
            &SkyCensus::empty(),
            &CompleteTo::nowhere(),
            spec,
            &rows,
        );
        let record = assert_a_cone_shares_the_bands_texels(
            cone,
            spec,
            &rows,
            [full.as_slice(), nowhere.as_slice()],
        );
        assert!(record.inside > 0, "{record:?}");
    }

    /// A cone's band marched at a band other than its query's is refused: its region is made of the
    /// query's band's texels, which the census keeps its stars by (R06.T8.l).
    #[test]
    #[should_panic(expected = "a cone's band is marched at its query's band")]
    fn a_cones_band_at_another_resolution_is_refused() {
        let cone = Cone::new(UnitVector::X, Degrees::new(10.0)).expect("a cone");
        let _ = march_rows(
            milky_way_galaxy(),
            &mut context(),
            &cone_query(cone, spec(16)),
            [CompleteTo::everywhere()],
            &spec(8),
            CubeFace::PosX,
            0..1,
        );
    }

    /// The band's largest texel radius, atan(√2 ÷ n), is the greatest angle from any texel's centre
    /// to a point of its edge, as a scan of every texel's corners and of points along its edges
    /// finds it on faces of n texels a side: 1.266° at 64² and 5.05° at 16² (R06.T8.l).
    #[test]
    fn the_largest_texel_radius_is_the_farthest_point_of_any_texel_from_its_centre() {
        let angle = |a: &UnitVector, b: [f64; 3]| {
            let b = UnitVector::from_components(b).expect("a direction");
            let sine = a.cross(&b).iter().map(|c| c * c).sum::<f64>().sqrt();
            math::atan2(sine, a.dot(&b))
        };
        for side in [1_u16, 2, 3, 8, 15, 16, 63, 64] {
            let spec = spec(side);
            let n = f64::from(side);
            let edge = |i: u16| 2.0 * f64::from(i) / n - 1.0;
            // Every face is the same up to its axes, so one face's texels are every face's.
            let face = CubeFace::NegY;
            let mut largest = 0.0_f64;
            for row in 0..side {
                for column in 0..side {
                    let centre = spec.texel_direction(face, row, column);
                    let (s0, s1) = (edge(column), edge(column + 1));
                    let (t0, t1) = (edge(row), edge(row + 1));
                    for k in 0..=8_u32 {
                        let f = f64::from(k) / 8.0;
                        let (s, t) = (s0 + f * (s1 - s0), t0 + f * (t1 - t0));
                        for (s, t) in [(s, t0), (s, t1), (s0, t), (s1, t)] {
                            largest = largest.max(angle(&centre, face.through(s, t)));
                        }
                    }
                }
            }
            let radius = spec.largest_texel_radius().value();
            assert!(
                (largest / radius - 1.0).abs() < 1e-12,
                "{side}²: the scan's {largest} rad against {radius} rad"
            );
        }
        let degrees = |side: u16| spec(side).largest_texel_radius().value() / RADIANS_PER_DEGREE;
        assert!((degrees(64) - 1.266).abs() < 0.001, "{}", degrees(64));
        assert!((degrees(16) - 5.051).abs() < 0.001, "{}", degrees(16));
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
                None,
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

    /// A near-Sun query to `cut` lit by the Sun's 16² illumination (R06.T9.g), with the eye at
    /// `eye_cut` if given, and `cone` with the band it is marched at if given.
    fn lit_query(cut: f64, eye_cut: Option<f64>, cone: Option<(Cone, BandSpec)>) -> SkyQuery {
        let mut builder = SkyQuery::builder(observer_at(SUN), Magnitudes::new(cut))
            .illumination(std::sync::Arc::clone(sun_illumination()));
        if let Some(eye_cut) = eye_cut {
            builder = builder
                .eye(crate::sky::eye::EyeObserver::default())
                .eye_cut(Magnitudes::new(eye_cut));
        }
        if let Some((cone, spec)) = cone {
            builder = builder.cone(cone).band_spec(spec);
        }
        builder.build().expect("a valid query")
    }

    /// Each ray's diffuse sums and A<sub>∞</sub> as bits, over every face of the 8² band, for
    /// `query` marched keeping `replies`, each face's rows in the parts `split`, on `ctx`.
    fn diffuse_bits(
        ctx: &mut SkyContext<'_>,
        query: &SkyQuery,
        replies: &[CompleteTo],
        split: &[Range<u16>],
    ) -> Vec<[u64; 6]> {
        let mut out = Vec::new();
        for face in CubeFace::ALL {
            for rows in split {
                let march = march_rows(
                    milky_way_galaxy(),
                    ctx,
                    query,
                    replies.iter().cloned(),
                    &spec(8),
                    face,
                    rows.clone(),
                );
                assert_eq!(march.diffuse().len(), march.edge_a_v().len());
                out.extend(
                    march
                        .diffuse()
                        .iter()
                        .zip(march.edge_a_v())
                        .map(|(sums, edge)| {
                            let [photopic, red, green, blue, scotopic] = sums.map(bits);
                            [photopic, red, green, blue, scotopic, bits(edge.value())]
                        }),
                );
            }
        }
        out
    }

    /// One case of the diffuse light's independence: what it varies, its query, its replies and
    /// its split of each face's rows.
    type DiffuseCase = (&'static str, SkyQuery, Vec<CompleteTo>, Vec<Range<u16>>);

    /// Every face's texels' diffuse luminance as bits, from `query`'s march near the Sun at 8²
    /// complete within 50 ly, summed with no census and with `census`, in that order.
    fn texel_diffuse_bits(query: &SkyQuery, census: &SkyCensus) -> [Vec<u64>; 2] {
        let within = complete_within(50.0);
        let mut out: [Vec<u64>; 2] = Default::default();
        for face in CubeFace::ALL {
            let march = march_rows(
                milky_way_galaxy(),
                &mut context(),
                query,
                [within.clone()],
                &spec(8),
                face,
                0..8,
            );
            for (bits_of, census) in out.iter_mut().zip([&SkyCensus::empty(), census]) {
                let mut texels = Vec::new();
                sum_rows(&march, census, &within, &mut texels);
                bits_of.extend(texels.iter().map(|t| bits(t.diffuse_luminance().value())));
            }
        }
        out
    }

    /// The cases of the diffuse light's independence (R06.T9.g, test 7) beside its reference, lit
    /// and complete `everywhere`, its rows `whole`.
    fn diffuse_cases(everywhere: &[CompleteTo], whole: &[Range<u16>]) -> [DiffuseCase; 7] {
        let (everywhere, whole) = (everywhere.to_vec(), whole.to_vec());
        let replies = replies_near_the_sun();
        let cone = Cone::new(UnitVector::X, Degrees::new(30.0)).expect("a cone");
        [
            (
                "cut 6.5",
                lit_query(6.5, None, None),
                everywhere.clone(),
                whole.clone(),
            ),
            (
                "cut 10.06 with the eye at 8.15",
                lit_query(10.06, Some(8.15), None),
                everywhere.clone(),
                whole.clone(),
            ),
            (
                "the eye at 8.15, complete nowhere",
                lit_query(8.15, Some(8.15), None),
                vec![CompleteTo::nowhere()],
                whole.clone(),
            ),
            (
                "to the caps",
                lit_query(8.15, None, None),
                vec![replies[4].clone()],
                whole.clone(),
            ),
            (
                "T9.f's six replies",
                lit_query(8.15, None, None),
                replies.to_vec(),
                whole.clone(),
            ),
            (
                "a 30° cone",
                lit_query(8.15, None, Some((cone, spec(8)))),
                replies.to_vec(),
                whole.clone(),
            ),
            (
                "the rows split",
                lit_query(8.15, None, None),
                vec![complete_within(100.0)],
                vec![0..3, 3..5, 5..8],
            ),
        ]
    }

    /// The diffuse light is the same, bit for bit, whatever the cut (6.5, 8.15, and 10.06 with the
    /// eye at 8.15), the replies (complete nowhere, everywhere, to the caps and T9.f's six), the
    /// census (none, and one within 50 ly with an overflow), a 30° cone, the split of the rows, a
    /// warm or a cold noise cache and the order of the illumination's jobs: at 8², near the Sun,
    /// lit by the 16² illumination (R06.T9.g, test 7). It reads only each ray's direction and its
    /// dust to the edge, whose bits no reply's, cone's or census's node moves.
    #[test]
    fn the_diffuse_light_is_the_same_whatever_the_cut_the_census_the_replies_and_the_cone() {
        let whole: Vec<Range<u16>> = std::iter::once(0..8).collect();
        let everywhere = vec![CompleteTo::everywhere()];
        let cold = |query: &SkyQuery, replies: &[CompleteTo], split: &[Range<u16>]| {
            diffuse_bits(&mut context(), query, replies, split)
        };
        let reference = cold(&lit_query(8.15, None, None), &everywhere, &whole);
        let lit = |sums: &[u64; 6]| sums[0] != bits(0.0);
        assert!(
            reference.iter().all(lit),
            "every ray of the band holds diffuse light near the Sun"
        );
        let cases = diffuse_cases(&everywhere, &whole);
        let mut warm = context();
        for (what, query, replies, split) in &cases {
            assert_eq!(&cold(query, replies, split), &reference, "{what}");
            assert_eq!(
                &diffuse_bits(&mut warm, query, replies, split),
                &reference,
                "{what}, on a warm noise cache"
            );
        }
        // The census moves no diffuse light: each texel's part, with no census and with one
        // within 50 ly whose overflow adds points, is its ray's photopic diffuse sum.
        let query = lit_query(8.15, None, None);
        let stars: Vec<SkyStar> = listed_with_caps_forced(&query, 50.0)
            .into_iter()
            .filter(|s| s.distance().value() <= 50.0)
            .collect();
        let census = merge_census([(stars, CensusTallies::default())], n(20));
        assert!(
            !census.overflow().is_empty(),
            "the census overflows into the band"
        );
        let rays: Vec<u64> = reference.iter().map(|sums| sums[0]).collect();
        let [empty, listed] = texel_diffuse_bits(&query, &census);
        assert_eq!(empty, rays, "the texels' diffuse light with no census");
        assert_eq!(
            listed, rays,
            "the texels' diffuse light with a census and its overflow"
        );
        // The illumination's jobs in another order and another split: the same illumination, and
        // so the same diffuse light.
        let observer = observer_at(SUN);
        let mut ctx = context();
        let mut jobs = Vec::new();
        for face in CubeFace::ALL.into_iter().rev() {
            for rows in [9..16, 0..4, 4..9] {
                jobs.push(Illumination::march_rows(
                    milky_way_galaxy(),
                    &mut ctx,
                    &observer,
                    face,
                    rows,
                ));
            }
        }
        let assembled = std::sync::Arc::new(Illumination::assemble(&observer, jobs));
        assert_eq!(
            assembled.bits(),
            sun_illumination().bits(),
            "the illumination is the same in any order"
        );
        let again = SkyQuery::builder(observer, Magnitudes::new(8.15))
            .illumination(assembled)
            .build()
            .expect("a valid query");
        assert_eq!(cold(&again, &everywhere, &whole), reference);
    }

    /// A texel is its starlight and its diffuse light: its luminance is the unlit march's plus its
    /// [`BandTexel::diffuse_luminance`], to 10⁻¹⁵ relative, its chroma and ρ are those of the
    /// summed sums, the eye's light under a camera's cut takes the same diffuse sums as the eye-only
    /// request's, bit for bit, and a query with no illumination gives texels with none, of the
    /// starlight's sums alone (R06.T9.g, test 8). At 8², near the Sun.
    #[test]
    fn the_texel_is_its_starlight_and_its_diffuse_light() {
        let within = complete_within(100.0);
        let unlit = query_near_the_sun(8.15, Eye::NotAsked);
        let lit = lit_query(8.15, None, None);
        let stars: Vec<SkyStar> = listed_with_caps_forced(&unlit, 50.0)
            .into_iter()
            .filter(|s| s.distance().value() <= 50.0)
            .collect();
        let census = merge_census([(stars, CensusTallies::default())], n(20));
        let (mut worst, mut texels) = (0.0_f64, 0_usize);
        for face in CubeFace::ALL {
            let march_of = |query: &SkyQuery| {
                march_rows(
                    milky_way_galaxy(),
                    &mut context(),
                    query,
                    [within.clone()],
                    &spec(8),
                    face,
                    0..8,
                )
            };
            let (dark, light) = (march_of(&unlit), march_of(&lit));
            assert!(
                dark.diffuse().is_empty(),
                "no illumination, no diffuse light"
            );
            assert_eq!(light.diffuse().len(), 64);
            let sum = |march: &BandMarch, census: &SkyCensus| {
                let mut out = Vec::new();
                sum_rows(march, census, &within, &mut out);
                out
            };
            for (starlight, both) in sum(&dark, &census).iter().zip(sum(&light, &census)) {
                let expected = starlight.luminance().value() + both.diffuse_luminance().value();
                worst = worst.max((both.luminance().value() / expected - 1.0).abs());
                assert_eq!(bits(starlight.diffuse_luminance().value()), bits(0.0));
                texels += 1;
            }
            // With no census, each texel is the texel of its summed sums, bit for bit.
            let rays = light.ray_light(&within);
            for (i, texel) in sum(&light, &SkyCensus::empty()).iter().enumerate() {
                let mut sums = rays[i];
                add_to(&mut sums, light.diffuse()[i]);
                let expected = BandTexel::of_sums(sums)
                    .with_diffuse(CandelasPerSquareMetre::new(light.diffuse()[i][0]));
                assert_eq!(
                    texel_bits(&[*texel]),
                    texel_bits(&[expected]),
                    "{face:?} {i}"
                );
                assert_eq!(
                    bits(texel.diffuse_luminance().value()),
                    bits(light.diffuse()[i][0])
                );
            }
            for (i, texel) in sum(&dark, &SkyCensus::empty()).iter().enumerate() {
                assert_eq!(
                    texel_bits(&[*texel]),
                    texel_bits(&[BandTexel::of_sums(dark.ray_light(&within)[i])]),
                    "{face:?} {i}: no illumination, the starlight alone"
                );
            }
            // The eye's light under a camera's cut, against the eye-only request's.
            let camera = march_of(&lit_query(10.06, Some(8.15), None));
            let eye_only = march_of(&lit_query(8.15, Some(8.15), None));
            assert_eq!(
                camera
                    .diffuse()
                    .iter()
                    .map(|d| d.map(bits))
                    .collect::<Vec<_>>(),
                eye_only
                    .diffuse()
                    .iter()
                    .map(|d| d.map(bits))
                    .collect::<Vec<_>>(),
            );
            for (under, alone) in sum(&camera, &SkyCensus::empty())
                .iter()
                .zip(sum(&eye_only, &SkyCensus::empty()))
            {
                let (light, rho) = under.eye_background();
                assert_eq!(
                    [bits(light.value()), bits(rho)],
                    [bits(alone.luminance().value()), bits(alone.sp_ratio())],
                    "{face:?}: the eye's light under a camera's cut"
                );
            }
        }
        eprintln!("{texels} texels: each its starlight and its diffuse light within {worst:.2e}");
        assert!(worst <= 1e-15, "{worst}");
    }

    /// A census near the Sun with every cap forced to 1,000 ly, so C, D and E in shells to 125, 250
    /// and 500 ly and then 1,000, its query's band at `face_texels`: the census of every layer's
    /// first shell, of no stars, and the first face row of a march of the plan's replies at
    /// `march_texels`, on tables of no component, whose rays hold no light.
    fn first_shell_and_march(face_texels: u16, march_texels: u16) -> (SkyCensus, BandMarch) {
        let galaxy = milky_way_galaxy();
        let query = SkyQuery::builder(observer_at(SUN), Magnitudes::new(CENSUS_CUT))
            .band_spec(spec(face_texels))
            .build()
            .expect("a valid query")
            .with_caps_forced(LightYears::new(1_000.0))
            .expect("a forced cap");
        let plan = census_plan_of(&query, query.forced_caps().expect("forced caps").to_vec());
        let first = plan.completeness(plan.shells().filter(|shell| shell.index() == 0));
        assert!(!first.is_final());
        let census = merge_shells(Vec::new(), n(MAX_N_MAX), first);
        let mut ctx = SkyContext {
            tables: milky_way_dark_tables(),
            envelope: milky_way_envelope(),
            offsets: milky_way_offsets(),
            noise: NoiseCache::with_capacity(1 << 12),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let replies = plan.replies();
        let march = march_rows(
            galaxy,
            &mut ctx,
            &query,
            replies,
            &spec(march_texels),
            CubeFace::PosZ,
            0..1,
        );
        (census, march)
    }

    /// A census of some of its plan's shells is summed at the radii it is complete to and at its
    /// query's band, whose texels it lists by (R06.T8.i; decided 2026-10-08,
    /// `decision-r06-t8i-listing.md`).
    #[test]
    fn a_census_of_shells_is_summed_at_its_querys_band_and_radii() {
        let (census, march) = first_shell_and_march(8, 8);
        let completeness = census.completeness().expect("a census of shells");
        assert!(march.holds(completeness.complete_to()));
        let mut texels = Vec::new();
        sum_rows(&march, &census, completeness.complete_to(), &mut texels);
        assert_eq!(texels.len(), 8);
    }

    #[test]
    #[should_panic(expected = "a census of shells not yet final lists by its query's band")]
    fn a_census_of_shells_summed_at_another_band_is_refused() {
        let (census, march) = first_shell_and_march(16, 8);
        let completeness = census.completeness().expect("a census of shells");
        sum_rows(&march, &census, completeness.complete_to(), &mut Vec::new());
    }

    #[test]
    #[should_panic(expected = "a census of shells is summed at the radii it is complete to")]
    fn a_census_of_shells_summed_at_other_radii_is_refused() {
        let (census, march) = first_shell_and_march(8, 8);
        sum_rows(&march, &census, &complete_within(1_000.0), &mut Vec::new());
    }
}
