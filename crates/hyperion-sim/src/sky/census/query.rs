//! The sky's query and the census's plan: which cells of which layers it opens, shell by shell
//! nearest first, and how far a census of some of its shells is complete (rendering plan R06,
//! R06.T8.a and R06.T8.i; Design notes 9, 10, 11 and 13).

use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::coords::{GalacticDisplacement, GalacticPosition, UnitVector};
use crate::galaxy::Galaxy;
use crate::galaxy::gas::modifiers::GasModifierSource;
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::placement::CellKey;
use crate::galaxy::query::{
    QuerySphere, SystemSource, cells_in_shell_slab, count_cells_in_sphere, pad_for, pad_speed,
    sphere_slabs,
};
use crate::id::{Layer, SystemId};
use crate::math;
use crate::observe::Observer;
use crate::time::Span;
use crate::units::consts::{RADIANS_PER_DEGREE, SECONDS_PER_JULIAN_YEAR};
use crate::units::{Degrees, LightYears, Magnitudes, Radians};

use super::super::band::{BandSpec, CompleteTo, CubeFace};
use super::super::caps::{
    CAPPED_LAYERS, LayerCap, REAL_BOUNDARY_LAYERS, RayCones, RayRadii, layer_caps,
    layer_caps_by_visibility, real_boundary_of,
};
use super::super::dgl::Illumination;
use super::super::envelope::BrightnessEnvelope;
use super::super::eye::{EyeObserver, MAX_CUT_V};
use super::super::limits::EyeVisibility;
use super::super::luminosity::LuminosityTables;
use super::cache::SkyCellCache;
use super::cell::{CellOffsets, SkyStar};

/// The largest count of stars a sky lists, 3 × 10⁵ (Design note 11): 7.2 MB of payload.
pub const MAX_N_MAX: u32 = 300_000;

/// The largest forced cap, ly: the root cube's diagonal, 2¹⁷ × √3, beyond which nothing lies.
pub const MAX_FORCED_CAP_LY: f64 = 227_023.0;

/// How much brighter than the cut a synthetic ceiling must be at least, mag: 0.5 (rendering plan
/// R13, R13.T2.a).
///
/// The server's ceilings, V 5.0 in RM3's interim and V 4.5 from R13.T7, lie 2.7 mag and more
/// brighter than the eye's cut near the Sun (V 7.77–8.18, R13.T1) and 0.8 mag and more in the
/// nuclear disc (V 5.81).
const MIN_CEILING_DEPTH_MAG: f64 = 0.5;

/// [`MAX_N_MAX`] as a [`NonZeroU32`], the default.
const DEFAULT_N_MAX: NonZeroU32 = NonZeroU32::new(MAX_N_MAX).expect("300,000 is not zero");

/// The edges of the census's distance shells, ly: 125, 250 and 500 ly, then 1,000 × 2<sup>k</sup>
/// ly, which is 1,000 × 2<sup>k</sup> for every k from −3.
///
/// They run to 128,000 ly, the last below [`MAX_FORCED_CAP_LY`], beyond which no cap reaches
/// (R06.T8.i; decided 2026-10-05, `decision-r06-census-cost.md`, with
/// `decision-r06-census-cost-signoff.md`'s 500 ly first shell). The two inner edges, 125 and
/// 250 ly, were added on 2026-10-08 (R06.T11.g, `decision-r06-t11d-first-sky.md` §1.4): a first
/// shell of 500 ly near the Sun generated about a quarter of C's records, half of D's and
/// five-sixths of E's, at 1–10 ms a system, some 760 CPU-s, against the first sky's 150 CPU-s; to
/// 125 ly it is estimated at 26–29.
///
/// A layer of [`SHELLED_LAYERS`] is censused in one shell for each edge below its cap's farthest
/// radius, nearest first, then one more to its cap. The edges are constants, never fitted to the
/// machine, so every machine and worker count gives the same sequence of replies, only at its own
/// pace.
pub const SHELL_EDGES_LY: [u32; 11] = [
    125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000, 32_000, 64_000, 128_000,
];

/// The layers censused shell by shell (R06.T8.i): C, D and E.
///
/// Layers A and B and the brown dwarfs, whose caps near the Sun are some 11, 68 and 1 ly, are one
/// shell each.
pub const SHELLED_LAYERS: [Layer; 3] = [Layer::C, Layer::D, Layer::E];

/// One distance shell of one layer of a [`CensusPlan`] (R06.T8.i).
///
/// Its cells are those of the layer's plan whose box first meets the sphere of the shell's edge,
/// padded as a walk to that edge pads, and not the sphere of the edge before it. A layer's last
/// shell holds the rest of its cells, to its cap.
///
/// Once a layer's shells to this one are done, its census is complete to the shell's edge, held
/// within the cap's radius towards each direction ([`CensusPlan::completeness`]). Shells order as
/// [`CensusPlan::shells`] gives them: by their place in their layer, nearest first, then by layer
/// as [`CAPPED_LAYERS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Shell {
    /// The shell's place among its layer's, nearest first, from 0.
    index: u8,
    layer: Layer,
    /// The edge, whole light-years, or `None` for its layer's last shell.
    edge_ly: Option<u32>,
}

impl Shell {
    /// The layer whose cells the shell holds.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The shell's place among its layer's shells, nearest first, from 0.
    #[must_use]
    pub const fn index(&self) -> u8 {
        self.index
    }

    /// The fixed edge the shell completes its layer to, held within the cap's radius towards each
    /// direction: one of [`SHELL_EDGES_LY`], or `None` for the layer's last shell, which completes
    /// it to its cap.
    #[must_use]
    pub fn edge(&self) -> Option<LightYears> {
        self.edge_ly.map(|edge| LightYears::new(f64::from(edge)))
    }

    /// Whether the shell is its layer's last, whose census lists every star of the cells it opens.
    #[must_use]
    pub const fn is_last(&self) -> bool {
        self.edge_ly.is_none()
    }
}

/// How far one layer's census has come through its shells.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Reached {
    /// Its shells done, nearest first, reach this edge, 0 before its first: it lists only the
    /// stars within its radius.
    Edge(LightYears),
    /// Its last shell is done: it lists every star of the cells it opens, as a one-shot census.
    Final,
}

/// How far a census of some of its plan's shells is complete, layer by layer, and so which of its
/// stars it lists (R06.T8.i).
///
/// Decided 2026-10-05 (`decision-r06-census-cost.md`, with the sign-off's condition 2,
/// `decision-r06-census-cost-signoff.md`) and 2026-10-08 (`decision-r06-t8i-listing.md`). Made by
/// [`CensusPlan::completeness`] and carried by the [`SkyCensus`](super::SkyCensus) that
/// [`merge_shells`](super::merge_shells) makes.
///
/// A layer whose shells are done to an edge is complete, towards each direction, to the lesser of
/// the edge and its cap's radius there ([`complete_to`](Self::complete_to)): its cap's rays each
/// held within the edge, or the edge in every direction where every ray reaches it. Until its last
/// shell is merged it lists only the stars within that radius towards their band texel
/// ([`lists`](Self::lists)), the radius the band's ray through the texel reads, bit for bit
/// (decided 2026-10-08, `decision-r06-t8i-listing.md`). A straddling cell's stars beyond it wait
/// for a later shell, neither listed nor overflow, so the listing and the band, which holds all of
/// the layer's light beyond the radius, share one boundary in every texel and no star's light is
/// counted twice (Design note 11). A layer whose last shell is merged lists every star of the cells
/// it opens, as the one-shot census does, in that census and every later one: beyond its cap
/// fewer than one star is expected, within its stated count beyond. A, B and the brown dwarfs, one
/// shell each, are final from the first.
///
/// Where a ray cone's edge crosses a texel, the radius towards the texel's centre can exceed the
/// radius towards one of its stars, and if that star's cell is unopened its light is in neither
/// the census nor the band: R06.T7.b's gap, deferred by the owner on 2026-10-08, under one star a
/// layer by the caps' count beyond (R06's Risks, "Deviations in T8.i, as built").
///
/// A query at a synthetic ceiling ([`SkyQuery::synthetic_ceiling`]; rendering plan R13, Design
/// note 3) lists C, D and E by the radius towards each star's texel at every reply, the final one
/// included: thousands of stars lie just beyond the real boundary R(u), in the cells that straddle
/// it, and the one-shot rule would count them twice, against the synthetic tier and the band. Its
/// plan opens their cells by cones widened by the texel's largest radius, so that no star within
/// R(u) of its texel lies in a cell left closed, and the gap is closed (fix (i), ruled
/// 2026-10-09).
#[derive(Debug, Clone, PartialEq)]
pub struct Completeness {
    /// The observer, from whom each star's direction is taken.
    observer: GalacticPosition,
    /// The band whose texels the radii are read at.
    band_spec: BandSpec,
    /// Per layer of [`CAPPED_LAYERS`], how far its shells are done.
    reached: [Reached; CAPPED_LAYERS.len()],
    /// Per layer, whether it lists by the radius towards each star's texel once final too: C, D
    /// and E at a synthetic ceiling.
    by_texel: [bool; CAPPED_LAYERS.len()],
    complete_to: CompleteTo,
}

impl Completeness {
    /// The radii to which the census is complete, per layer and towards each direction: the band's
    /// boundary for this census ([`sum_rows`](super::super::band::sum_rows)).
    #[must_use]
    pub const fn complete_to(&self) -> &CompleteTo {
        &self.complete_to
    }

    /// The observer, from whom each star's band texel is found.
    #[must_use]
    pub const fn observer(&self) -> &GalacticPosition {
        &self.observer
    }

    /// The band whose texels the census's radii are read towards: its query's
    /// [`band_spec`](SkyQuery::band_spec), at which its band is marched.
    #[must_use]
    pub const fn band_spec(&self) -> BandSpec {
        self.band_spec
    }

    /// Whether every layer's last shell is done: the census is the one-shot census of its plan.
    #[must_use]
    pub fn is_final(&self) -> bool {
        self.reached.iter().all(|r| matches!(r, Reached::Final))
    }

    /// The fixed shell edge `layer`'s shells done reach, 0 before its first (one of
    /// [`SHELL_EDGES_LY`] for a plan of [`census_plan`]), or `None` once its last is done and for a
    /// layer outside [`CAPPED_LAYERS`].
    #[must_use]
    pub fn edge(&self, layer: Layer) -> Option<LightYears> {
        let i = CAPPED_LAYERS.iter().position(|&l| l == layer)?;
        match self.reached[i] {
            Reached::Edge(edge) => Some(edge),
            Reached::Final => None,
        }
    }

    /// The least edge over the layers not yet final, or `None` for a final census: the figure of
    /// the view's stars-arriving note, which is a fixed edge and not a ray's radius (R06.T11.d).
    #[must_use]
    pub fn least_edge(&self) -> Option<LightYears> {
        self.reached
            .iter()
            .filter_map(|r| match r {
                Reached::Edge(edge) => Some(*edge),
                Reached::Final => None,
            })
            .min_by(|a, b| a.value().total_cmp(&b.value()))
    }

    /// Whether the census lists `star`: always where its layer is final, and otherwise only within
    /// its layer's complete-to radius ([`radius_for`](Self::radius_for)).
    ///
    /// At a synthetic ceiling C, D and E list only within it at every reply, the final one
    /// included (rendering plan R13, Design note 3).
    #[must_use]
    pub fn lists(&self, star: &SkyStar) -> bool {
        let Some(i) = CAPPED_LAYERS.iter().position(|&l| l == star.layer()) else {
            return true;
        };
        match self.reached[i] {
            Reached::Final if !self.by_texel[i] => true,
            Reached::Final | Reached::Edge(_) => {
                star.distance().value() < self.radius_for(star).value()
            }
        }
    }

    /// Whether some layer lists by the radius towards each star's band texel
    /// ([`lists`](Self::lists)): one not yet final, or C, D and E at a synthetic ceiling.
    ///
    /// Its band is then summed at the census's [`band_spec`](Self::band_spec) from its observer,
    /// so that the two share one boundary.
    #[must_use]
    pub fn lists_by_texel(&self) -> bool {
        self.reached
            .iter()
            .zip(&self.by_texel)
            .any(|(reached, &by_texel)| by_texel || matches!(reached, Reached::Edge(_)))
    }

    /// The radius `star`'s layer is complete to where the star lies, towards its band texel.
    ///
    /// That is towards the centre of the band's texel that [`BandSpec::texel_of`] places the star
    /// in, at the query's [`band_spec`](SkyQuery::band_spec), from the displacement the band places
    /// an overflow star by (R06.T8.l's lookup). It is the radius the band's ray through that texel
    /// reads, so the
    /// census and the band share it star by star; for uniform radii it is the radius in the star's
    /// own direction. A star at its observer, which no census lists, takes the layer's farthest
    /// radius.
    #[must_use]
    pub fn radius_for(&self, star: &SkyStar) -> LightYears {
        let d = self.observer.displacement_to(star.apparent()).metres();
        match self.band_spec.texel_of(d) {
            Some((face, row, column)) => self.complete_to.radius_toward(
                star.layer(),
                self.band_spec.texel_direction(face, row, column),
            ),
            None => self.complete_to.radius(star.layer()),
        }
    }
}

/// A [`SkyQuery`] or a [`Cone`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSkyQueryError {
    /// The cut is not finite or is deeper than [`MAX_CUT_V`].
    Cut,
    /// `n_max` is above [`MAX_N_MAX`].
    NMax,
    /// The cone's half-angle is not within (0°, 90°].
    ConeHalfAngle,
    /// A forced cap is not finite and non-negative, or reaches past the root cube's diagonal.
    ForcedCap,
    /// The eye's cut is asked without the eye, or is not finite, or is deeper than the cut.
    EyeCut,
    /// The eye is asked with a cone: the naked eye has no field stop, and its veil reaches 90°
    /// (R06.T8.l; decided 2026-10-07, `decision-r06-t8k-cone.md`, item 2).
    ConeWithEye,
    /// The illumination was marched for another observer, or at another time: the diffuse light
    /// is the observer's own sky scattered (R06.T9.g).
    Illumination,
    /// The eye's visibility is asked without the eye, or for a request whose cut is not the eye's
    /// own, or was taken for another observer, eye or cut than the request's (R06.T7.b): it caps
    /// an eye-only request.
    EyeVisibility,
    /// The synthetic ceiling is not finite, or is not brighter than the cut by at least 0.5 mag
    /// (rendering plan R13, R13.T2.a).
    SyntheticCeiling,
}

impl fmt::Display for BuildSkyQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cut => "the cut is not finite or is deeper than V 11",
            Self::NMax => "n_max is above 300,000",
            Self::ConeHalfAngle => "the cone's half-angle is not within 0 to 90 degrees",
            Self::ForcedCap => "a forced cap is not within 0 to the root cube's diagonal",
            Self::EyeCut => {
                "the eye's cut is asked without the eye, or is not finite or deeper than the cut"
            }
            Self::ConeWithEye => "the naked eye cannot ask a cone: it has no field stop",
            Self::Illumination => "the illumination was marched for another observer or time",
            Self::EyeVisibility => {
                "the eye's visibility caps an eye-only request of its observer, eye and cut"
            }
            Self::SyntheticCeiling => {
                "the synthetic ceiling is not finite or not brighter than the cut by 0.5 mag"
            }
        })
    }
}

impl Error for BuildSkyQueryError {}

/// A cone of the sky about `axis`: an instrument's field stop, for a deep exposure of a narrow
/// field (Design note 5; R06.T8.l, decided 2026-10-07, `decision-r06-t8k-cone.md`).
///
/// Behind a field stop no image-forming light from outside the field reaches the detector (the
/// light scattered into the field by optics in front of the stop is not modelled). A cone's census
/// lists only the stars of the band's texels about it ([`ConeRegion`]: those whose centres lie
/// within its half-angle plus the band's largest texel radius, every texel that meets the cone
/// among them), so none outside them glares inside it, and its band holds all of the light outside
/// them. The naked eye has no field stop narrower than its own field, and its veil reaches 90°
/// (Design note 4), so it cannot ask one ([`BuildSkyQueryError::ConeWithEye`]): an instrument
/// that wants both sends two requests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cone {
    axis: UnitVector,
    half_angle: Degrees,
}

impl Cone {
    /// The cone about `axis` of `half_angle`.
    ///
    /// # Errors
    ///
    /// [`BuildSkyQueryError::ConeHalfAngle`] unless the half-angle is within (0°, 90°].
    pub fn new(axis: UnitVector, half_angle: Degrees) -> Result<Self, BuildSkyQueryError> {
        let a = half_angle.value();
        if a > 0.0 && a <= 90.0 {
            Ok(Self { axis, half_angle })
        } else {
            Err(BuildSkyQueryError::ConeHalfAngle)
        }
    }

    /// The axis.
    #[must_use]
    pub const fn axis(&self) -> UnitVector {
        self.axis
    }

    /// The half-angle.
    #[must_use]
    pub const fn half_angle(&self) -> Degrees {
        self.half_angle
    }

    /// The half-angle, radians.
    #[must_use]
    fn half_angle_radians(&self) -> f64 {
        self.half_angle.value() * RADIANS_PER_DEGREE
    }

    /// The cosine of the half-angle: a direction lies inside the cone where its cosine with the
    /// axis is at least this ([`holds`](Self::holds)).
    #[must_use]
    pub fn cos_half_angle(&self) -> f64 {
        math::cos(self.half_angle_radians())
    }

    /// Whether the direction of `displacement` from the apex lies inside the cone itself, its edge
    /// included: its cosine with the axis at least [`cos_half_angle`](Self::cos_half_angle),
    /// tested in metres as the displacement holds them. A zero displacement has no direction and is
    /// held; no star the census measures lies at its observer.
    ///
    /// The census keeps a star by its band texel instead ([`ConeRegion::holds`]), so that its
    /// listing and the band share one boundary, and that region holds every direction this holds.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::coords::{GalacticDisplacement, UnitVector};
    /// use hyperion_sim::sky::census::{BuildSkyQueryError, Cone};
    /// use hyperion_sim::units::Degrees;
    /// use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
    ///
    /// let cone = Cone::new(UnitVector::X, Degrees::new(10.0))?;
    /// let ly = METRES_PER_LIGHT_YEAR;
    /// // A star a light-year along the axis and a tenth of one across it, 5.7° off the axis.
    /// assert!(cone.holds(&GalacticDisplacement::new([ly, 0.1 * ly, 0.0])));
    /// // One as far across as along, 45° off it.
    /// assert!(!cone.holds(&GalacticDisplacement::new([ly, ly, 0.0])));
    /// # Ok::<(), BuildSkyQueryError>(())
    /// ```
    #[must_use]
    pub fn holds(&self, displacement: &GalacticDisplacement) -> bool {
        let d = displacement.metres();
        let u = self.axis.components();
        let along = d[0] * u[0] + d[1] * u[1] + d[2] * u[2];
        let length = math::hypot(math::hypot(d[0], d[1]), d[2]);
        along >= self.cos_half_angle() * length
    }
}

/// How much wider than α + 2ρ a cone's plan opens its cells, radians: 10⁻⁹, against the rounding
/// of a texel's cosine, its lookup and the plan's arccosine (some 10⁻¹⁴ rad at a texel's corner),
/// so that no star of a region texel lies in a cell the plan leaves closed (R06.T8.l).
const PLAN_MARGIN_RAD: f64 = 1e-9;

/// Whether a ball of radius `radius_ly` whose centre lies `offset_ly` from the apex (both in
/// light-years, on the galactic axes) meets the cone about `axis` of half-angle `opening`, radians
/// (any opening: one of π or more holds every direction).
#[must_use]
fn ball_meets_cone(axis: UnitVector, opening: f64, offset_ly: [f64; 3], radius_ly: f64) -> bool {
    let d = math::hypot(math::hypot(offset_ly[0], offset_ly[1]), offset_ly[2]);
    if d <= radius_ly {
        return true;
    }
    let u = axis.components();
    let cos = (offset_ly[0] * u[0] + offset_ly[1] * u[1] + offset_ly[2] * u[2]) / d;
    let angle = math::acos(cos.clamp(-1.0, 1.0));
    angle <= opening + math::asin(radius_ly / d)
}

/// The band's texels a cone's census and band share (R06.T8.l; decided 2026-10-07,
/// `decision-r06-t8k-cone.md`, item 1): those whose centre lies within the cone's half-angle α
/// plus the band's largest texel radius ρ ([`BandSpec::largest_texel_radius`]) of its axis, at
/// the [`BandSpec`] the query's band uses ([`SkyQuery::band_spec`]). Every texel that meets the
/// cone is among them, and some that do not.
///
/// In a texel of the region the band is complete, to each reply's radii, and the census lists
/// exactly the stars [`BandSpec::texel_of`] places in it, the lookup the band places its
/// overflow's stars by. Every other texel is complete nowhere and the census lists none of its
/// stars. So each texel holds either the full sky's light fainter than the cut with its own
/// stars listed, or all of its light with none listed: no star's light is counted twice or lost,
/// bit for bit (Design note 11). By the triangle inequality every direction within α of the axis
/// lies in a texel whose centre is within α + ρ, so the field is complete however narrow the
/// cone. Over cones of every axis the region's area is on average that of the cap its texels'
/// centres lie in, about (1 + ρ ÷ α)² the cone's: 2.3 times at 2.5° and 27 times at 0.3° at 64²,
/// where a sub-degree field needs a finer band for its own background in any case (R06's Risks,
/// "A narrow cone's region"). That is a mean, not a bound: the texels reach to α + 2ρ, so a
/// region holds at most the cap of α + 2ρ, 4.1 times the cone at 2.5° and 89 times at 0.3° (29
/// times at 0.3° about the census's brightest star near the Sun, in R06.T8.l's test).
///
/// # Examples
///
/// A cone of 1° about the centre of a face, where four 16² texels meet 5.05° from their centres,
/// holds no texel's centre, yet its region holds those four texels, and so every direction in it:
///
/// ```
/// use hyperion_sim::coords::{GalacticDisplacement, UnitVector};
/// use hyperion_sim::sky::band::{BandSpec, CubeFace};
/// use hyperion_sim::sky::census::{Cone, ConeRegion};
/// use hyperion_sim::units::Degrees;
///
/// let spec = BandSpec::new(16, 12).ok_or("a spec")?;
/// let cone = Cone::new(UnitVector::X, Degrees::new(1.0))?;
/// let region = ConeRegion::new(cone, spec);
/// // α + ρ: 1° and the texels' largest radius at 16², 5.05°.
/// assert!((Degrees::from(region.reach()).value() - 6.05).abs() < 0.01);
/// for (row, column) in [(7, 7), (7, 8), (8, 7), (8, 8)] {
///     assert!(region.holds_texel(CubeFace::PosX, row, column));
///     let centre = spec.texel_direction(CubeFace::PosX, row, column);
///     assert!(!cone.holds(&GalacticDisplacement::new(centre.components())));
/// }
/// assert!(region.holds(&GalacticDisplacement::new([1.0, 0.01, -0.01])));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConeRegion {
    cone: Cone,
    spec: BandSpec,
    /// cos(α + ρ): a texel is in the region where its centre's cosine with the axis is at least
    /// this.
    cos_reach: f64,
}

impl ConeRegion {
    /// The region of `cone` in a band of `spec`.
    #[must_use]
    pub fn new(cone: Cone, spec: BandSpec) -> Self {
        let reach = cone.half_angle_radians() + spec.largest_texel_radius().value();
        Self {
            cone,
            spec,
            cos_reach: math::cos(reach),
        }
    }

    /// The cone.
    #[must_use]
    pub const fn cone(&self) -> &Cone {
        &self.cone
    }

    /// The band's resolution, whose texels the region is made of.
    #[must_use]
    pub const fn spec(&self) -> BandSpec {
        self.spec
    }

    /// How far from the axis a texel's centre may lie in the region, α + ρ.
    #[must_use]
    pub fn reach(&self) -> Radians {
        Radians::new(self.cone.half_angle_radians() + self.spec.largest_texel_radius().value())
    }

    /// The cosine of [`reach`](Self::reach), the one value the census's stars and the band's rays
    /// are tested against: a texel is in the region where its centre's cosine with the axis is at
    /// least this.
    #[must_use]
    pub const fn cos_reach(&self) -> f64 {
        self.cos_reach
    }

    /// Whether the texel at `row` (from the top) and `column` (from the left) of `face` lies in
    /// the region: its centre ([`BandSpec::texel_direction`]) has a cosine with the axis at least
    /// [`cos_reach`](Self::cos_reach). The band's march tests each ray so, and the census each
    /// star's texel.
    ///
    /// # Panics
    ///
    /// If `row` or `column` is not below the band's [`BandSpec::face_texels`].
    #[must_use]
    pub fn holds_texel(&self, face: CubeFace, row: u16, column: u16) -> bool {
        self.spec
            .texel_direction(face, row, column)
            .dot(&self.cone.axis())
            >= self.cos_reach
    }

    /// Whether the direction of `displacement` from the apex lies in a texel of the region: the
    /// texel [`BandSpec::texel_of`] places it in, as the band places its overflow's stars, tested
    /// by [`holds_texel`](Self::holds_texel). So a star and its texel agree bit for bit. A zero
    /// displacement has no direction and is held, as [`Cone::holds`] holds it; no star the
    /// census measures lies at its observer. A displacement that is not finite, which no star
    /// has, is not held.
    #[must_use]
    pub fn holds(&self, displacement: &GalacticDisplacement) -> bool {
        let d = displacement.metres();
        match self.spec.texel_of(d) {
            Some((face, row, column)) => self.holds_texel(face, row, column),
            // `texel_of` places every finite direction but the zero one.
            None => d.iter().all(|c| c.is_finite()),
        }
    }

    /// Whether a ball of radius `radius_ly` whose centre lies `offset_ly` from the apex (both in
    /// light-years, on the galactic axes) meets the cone a census plan opens its cells by: the cone
    /// widened to α + 2ρ, and [`PLAN_MARGIN_RAD`] more. A region texel's centre lies within α + ρ
    /// of the axis and each of its points within ρ of its centre, so every star a region texel
    /// holds lies within α + 2ρ; the margin covers the rounding of the tests on both sides. A
    /// wider plan only opens more cells, whose stars the region keeps or leaves out as before.
    #[must_use]
    fn meets_ball(&self, offset_ly: [f64; 3], radius_ly: f64) -> bool {
        let opening = self.cone.half_angle_radians()
            + 2.0 * self.spec.largest_texel_radius().value()
            + PLAN_MARGIN_RAD;
        ball_meets_cone(self.cone.axis(), opening, offset_ly, radius_ly)
    }
}

/// What a sky is asked for: an observer, a cut, the eye and its own cut if asked, the count
/// budget, an optional cone with the band whose texels make its region, the observer's own
/// system to leave out, and the request's illumination, whose scattered light the band holds
/// (Design notes 5, 10, 11 and 15; R06.T9.g). Built through [`SkyQuery::builder`].
#[derive(Debug, Clone, PartialEq)]
pub struct SkyQuery {
    observer: Observer,
    cut: Magnitudes,
    eye: Option<EyeObserver>,
    /// The eye's cut: `Some` exactly when the eye is asked once built, and never deeper than the
    /// cut.
    eye_cut: Option<Magnitudes>,
    n_max: NonZeroU32,
    /// The band's resolution, whose texels make a cone's region.
    band_spec: BandSpec,
    /// The cone's region at `band_spec`, if the sky is a narrow field: computed once, when the cone
    /// or the band is set, and read by the census and the band alike.
    region: Option<ConeRegion>,
    exclude: Option<SystemId>,
    forced_caps: Option<Vec<LayerCap>>,
    /// The request's illumination, of the query's observer once built (R06.T9.g).
    illumination: Option<Arc<Illumination>>,
    /// The eye's visibility, if an eye-only request asks its caps by it (R06.T7.b).
    eye_visibility: Option<Arc<EyeVisibility>>,
    /// The synthetic ceiling V<sub>P</sub>, if the request states one (rendering plan R13): at
    /// least [`MIN_CEILING_DEPTH_MAG`] brighter than the cut once built.
    synthetic_ceiling: Option<Magnitudes>,
}

/// Builds a [`SkyQuery`].
#[derive(Debug, Clone, PartialEq)]
pub struct SkyQueryBuilder {
    query: SkyQuery,
}

impl SkyQuery {
    /// A query for `observer` to apparent V `cut`, with `n_max` of [`MAX_N_MAX`], no eye, no cone
    /// and nothing left out, to be refined and checked by the builder.
    ///
    /// # Examples
    ///
    /// A deep exposure of a narrow field asks for a cone, the eye's sky for the eye:
    ///
    /// ```
    /// use hyperion_sim::coords::{GalacticPosition, UnitVector};
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::EyeObserver;
    /// use hyperion_sim::sky::census::{BuildSkyQueryError, Cone, SkyQuery};
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::{Degrees, Magnitudes};
    ///
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in the cube")?;
    /// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
    /// let eye = SkyQuery::builder(observer.clone(), Magnitudes::new(7.95))
    ///     .eye(EyeObserver::default())
    ///     .build()?;
    /// assert!(eye.eye().is_some());
    /// let narrow = SkyQuery::builder(observer.clone(), Magnitudes::new(11.0))
    ///     .cone(Cone::new(UnitVector::NORTH, Degrees::new(1.0))?)
    ///     .build()?;
    /// assert!(narrow.cone().is_some());
    /// // Deeper than V 11 a narrow zoom would ask for a million stars: a cone is the way.
    /// let deep = SkyQuery::builder(observer, Magnitudes::new(12.0)).build();
    /// assert_eq!(deep, Err(BuildSkyQueryError::Cut));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn builder(observer: Observer, cut: Magnitudes) -> SkyQueryBuilder {
        SkyQueryBuilder {
            query: Self {
                observer,
                cut,
                eye: None,
                eye_cut: None,
                n_max: DEFAULT_N_MAX,
                band_spec: BandSpec::STANDARD,
                region: None,
                exclude: None,
                forced_caps: None,
                illumination: None,
                eye_visibility: None,
                synthetic_ceiling: None,
            },
        }
    }

    /// The observer.
    #[must_use]
    pub const fn observer(&self) -> &Observer {
        &self.observer
    }

    /// The cut, apparent V.
    #[must_use]
    pub const fn cut(&self) -> Magnitudes {
        self.cut
    }

    /// The eye, if the sky is asked for one. It sets the cut (R06.T9.d) and each view's cull, but
    /// not the census: a star is kept to the cut alone, with the eye or without it, and each view
    /// applies the star's colour offset (decided 2026-10-06, `decision-r06-t9b-band.md`;
    /// R06.T8.k).
    #[must_use]
    pub const fn eye(&self) -> Option<&EyeObserver> {
        self.eye.as_ref()
    }

    /// The eye's cut, apparent V, if the sky is asked for the eye: the cut of the eye's own
    /// request, at which its limit map is taken whatever the query's cut (R06.T9.j; decided
    /// 2026-10-06, `decision-r06-t9c-glare.md`). It is the query's cut unless a camera's deeper cut
    /// set that (Design note 5), and is never deeper than it. The band then keeps the light fainter
    /// than the eye's cut beside its own, as the eye's background
    /// ([`march_rows`](super::super::band::march_rows)), and only the listed stars at or brighter
    /// than it glare ([`Glare::of_listed`](super::super::limits::Glare::of_listed)).
    ///
    /// So the eye's map under a camera's cut is the eye-only request's at the camera's census
    /// radii (the same replies and `n_max`), bit for bit. Beyond the eye-only request's own caps, which follow its shallower cut,
    /// the camera's census lists the real stars brighter than the eye's cut, under one expected a
    /// layer, which the eye-only request holds as expected light: accepted as the truer sky
    /// (decided 2026-10-07, `decision-r06-t9c-glare.md`, addendum 2).
    #[must_use]
    pub const fn eye_cut(&self) -> Option<Magnitudes> {
        self.eye_cut
    }

    /// The most stars listed.
    #[must_use]
    pub const fn n_max(&self) -> NonZeroU32 {
        self.n_max
    }

    /// The cone, if the sky is a narrow field: an instrument's field stop.
    #[must_use]
    pub const fn cone(&self) -> Option<&Cone> {
        match &self.region {
            Some(region) => Some(region.cone()),
            None => None,
        }
    }

    /// The resolution of the band the query's sky is marched at: [`BandSpec::STANDARD`], the
    /// server's, unless [`SkyQueryBuilder::band_spec`] set another. Only a cone's region reads it,
    /// and a cone's band must be marched at it ([`march_rows`](super::super::band::march_rows)).
    #[must_use]
    pub const fn band_spec(&self) -> BandSpec {
        self.band_spec
    }

    /// The cone's region, the band's texels about it, if the sky is a narrow field
    /// (R06.T8.l): the census lists only its texels' stars and the band is complete only in them.
    #[must_use]
    pub const fn cone_region(&self) -> Option<&ConeRegion> {
        self.region.as_ref()
    }

    /// The system left out: the observer's own, whose stars are discs.
    #[must_use]
    pub const fn exclude(&self) -> Option<SystemId> {
        self.exclude
    }

    /// Each layer's forced cap, where the query forces its caps
    /// ([`with_caps_forced`](Self::with_caps_forced)): `None` where [`census_plan`] derives them.
    #[must_use]
    pub fn forced_caps(&self) -> Option<&[LayerCap]> {
        self.forced_caps.as_deref()
    }

    /// The request's illumination, if stated (R06.T9.g; decided 2026-10-07,
    /// `decision-r06-t9g-dgl.md`).
    ///
    /// It is the observer's own sky of all starlight, whose light the band's dust scatters into
    /// each of its rays ([`super::super::dgl`]). The band then holds that diffuse galactic light in
    /// every texel, the same in every reply and under any cut, census or cone; without one it holds
    /// the starlight alone, as before. The census does not read it.
    #[must_use]
    pub fn illumination(&self) -> Option<&Illumination> {
        self.illumination.as_deref()
    }

    /// The eye's visibility, if the request caps its census by it
    /// ([`SkyQueryBuilder::eye_visibility`]; R06.T7.b).
    #[must_use]
    pub fn eye_visibility(&self) -> Option<&EyeVisibility> {
        self.eye_visibility.as_deref()
    }

    /// The synthetic ceiling V<sub>P</sub>, apparent V, if the request states one
    /// ([`SkyQueryBuilder::synthetic_ceiling`]; rendering plan R13, Design notes 3 and 4): then C,
    /// D and E are censused to their real boundary only, and listed by the radius towards each
    /// star's band texel at every reply.
    #[must_use]
    pub const fn synthetic_ceiling(&self) -> Option<Magnitudes> {
        self.synthetic_ceiling
    }

    /// The same query with every layer's cap forced to `radius`: the census the brute force is
    /// compared with (R06.T8.e). A forced cap states nothing beyond it, and the plan reads no
    /// luminosity table.
    ///
    /// # Errors
    ///
    /// [`BuildSkyQueryError::ForcedCap`] unless `radius` is finite, non-negative and within the
    /// root cube's diagonal ([`MAX_FORCED_CAP_LY`]).
    pub fn with_caps_forced(self, radius: LightYears) -> Result<Self, BuildSkyQueryError> {
        let radii: Vec<(Layer, LightYears)> =
            CAPPED_LAYERS.iter().map(|&layer| (layer, radius)).collect();
        self.with_caps_forced_per_layer(&radii)
    }

    /// The same query with each listed layer's cap forced to its radius and every other layer's
    /// to nothing: a brute-force comparison of the layers whose cells a test can afford to
    /// generate whole.
    ///
    /// # Errors
    ///
    /// [`BuildSkyQueryError::ForcedCap`] unless every radius is finite, non-negative and within
    /// the root cube's diagonal ([`MAX_FORCED_CAP_LY`]).
    pub fn with_caps_forced_per_layer(
        mut self,
        radii: &[(Layer, LightYears)],
    ) -> Result<Self, BuildSkyQueryError> {
        if !radii
            .iter()
            .all(|(_, r)| (0.0..=MAX_FORCED_CAP_LY).contains(&r.value()))
        {
            return Err(BuildSkyQueryError::ForcedCap);
        }
        self.forced_caps = Some(
            CAPPED_LAYERS
                .iter()
                .map(|&layer| {
                    let radius = radii
                        .iter()
                        .find(|(l, _)| *l == layer)
                        .map_or(LightYears::ZERO, |&(_, r)| r);
                    LayerCap::forced(layer, radius)
                })
                .collect(),
        );
        Ok(self)
    }
}

impl SkyQueryBuilder {
    /// Asks for the eye's limits, at the query's cut unless [`eye_cut`](Self::eye_cut) sets the
    /// eye's own. The census still keeps each star to the cut alone (R06.T8.k).
    #[must_use]
    pub fn eye(mut self, eye: EyeObserver) -> Self {
        self.query.eye = Some(eye);
        self
    }

    /// The eye's own cut, where a camera's deeper cut sets the query's (Design note 5; R06.T9.j):
    /// the eye's limits are then those of a request at the eye's cut alone, its background the
    /// light fainter than it and its glare the stars at or brighter than it
    /// ([`SkyQuery::eye_cut`]). The server gives the eye's cut [`eye_cut`](super::super::limits::eye_cut)'s
    /// value and the query the deeper of the two. Asked without [`eye`](Self::eye), it is refused.
    ///
    /// # Examples
    ///
    /// The cockpit eye beside a camera that sees to V 10.06:
    ///
    /// ```
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::EyeObserver;
    /// use hyperion_sim::sky::census::{BuildSkyQueryError, SkyQuery};
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::Magnitudes;
    ///
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
    /// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
    /// let (eye_cut, camera) = (Magnitudes::new(8.28), Magnitudes::new(10.06));
    /// let query = SkyQuery::builder(observer.clone(), camera)
    ///     .eye(EyeObserver::default())
    ///     .eye_cut(eye_cut)
    ///     .build()?;
    /// assert_eq!((query.cut(), query.eye_cut()), (camera, Some(eye_cut)));
    /// // An eye-only request takes its cut as the eye's.
    /// let eye_only = SkyQuery::builder(observer.clone(), eye_cut).eye(EyeObserver::default()).build()?;
    /// assert_eq!(eye_only.eye_cut(), Some(eye_cut));
    /// // The eye's cut is never deeper than the cut the census lists to.
    /// let deeper = SkyQuery::builder(observer, eye_cut)
    ///     .eye(EyeObserver::default())
    ///     .eye_cut(camera)
    ///     .build();
    /// assert_eq!(deeper, Err(BuildSkyQueryError::EyeCut));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn eye_cut(mut self, eye_cut: Magnitudes) -> Self {
        self.query.eye_cut = Some(eye_cut);
        self
    }

    /// The most stars listed.
    #[must_use]
    pub fn n_max(mut self, n_max: NonZeroU32) -> Self {
        self.query.n_max = n_max;
        self
    }

    /// A narrow field, behind an instrument's field stop (R06.T8.l; decided 2026-10-07,
    /// `decision-r06-t8k-cone.md`): the census keeps only the stars of the band's texels about the
    /// cone, its region at the query's [`band_spec`](Self::band_spec) ([`ConeRegion`]), and the
    /// band is complete only in them, so the two share one boundary exactly. The plan opens the
    /// cells whose padded ball meets the cone widened to hold every region texel. None of the
    /// stars outside the region is listed, so none of them glares inside it: the naked eye, which
    /// has no field stop, cannot ask one ([`BuildSkyQueryError::ConeWithEye`]).
    #[must_use]
    pub fn cone(mut self, cone: Cone) -> Self {
        self.query.region = Some(ConeRegion::new(cone, self.query.band_spec));
        self
    }

    /// The resolution of the band the query's sky is marched at, whose texels make a cone's region
    /// ([`SkyQuery::band_spec`]); [`BandSpec::STANDARD`] unless set.
    #[must_use]
    pub fn band_spec(mut self, spec: BandSpec) -> Self {
        self.query.band_spec = spec;
        self.query.region = self
            .query
            .region
            .map(|region| ConeRegion::new(*region.cone(), spec));
        self
    }

    /// Leaves out the observer's own system.
    #[must_use]
    pub fn exclude(mut self, system: SystemId) -> Self {
        self.query.exclude = Some(system);
        self
    }

    /// States the request's illumination, so that the band holds the diffuse galactic light
    /// (R06.T9.g; [`SkyQuery::illumination`]).
    ///
    /// It is marched once a request for its observer and time ([`Illumination::march`]) and shared
    /// by every query of the request. One marched for another observer or time is refused.
    #[must_use]
    pub fn illumination(mut self, illumination: Arc<Illumination>) -> Self {
        self.query.illumination = Some(illumination);
        self
    }

    /// Caps an eye-only request's census by the eye's visibility (R06.T7.b).
    ///
    /// The visibility is the limit across the sky that the eye cut's pre-pass gives at the eye's
    /// cut, for the request's observer and eye
    /// ([`eye_visibility`](super::super::limits::eye_visibility)). Each ray of the caps counts the
    /// stars brighter than the eye's deepest limit about its cone, rather than the uniform cut, so
    /// the caps reach less far where the sky is brighter and the eye sees less
    /// ([`layer_caps_by_visibility`], whose example shows the flow). The census still lists every
    /// star of the cells it opens to the cut. A request with a camera's deeper cut asks none: its
    /// caps are the camera's.
    #[must_use]
    pub fn eye_visibility(mut self, visibility: EyeVisibility) -> Self {
        self.query.eye_visibility = Some(Arc::new(visibility));
        self
    }

    /// States the synthetic ceiling V<sub>P</sub>, apparent V (rendering plan R13, Design notes 3
    /// and 4): the hybrid sky's real tier, the server's constant on every request.
    ///
    /// Layers C, D and E are then censused only to their real boundary R(u), ray by ray the least
    /// of their cap at the ceiling, their cap at the cut and
    /// [`REAL_LIMIT_LY`](super::super::caps::REAL_LIMIT_LY) ([`real_boundary`](super::super::caps::real_boundary)):
    /// [`census_plan`] counts the caps at both cuts and draws it, and [`census_plan_of`] takes the
    /// caps its caller drew so. Either way the plan opens their cells by the rays' cones widened by
    /// the query's band texel's largest radius, and lists them by the radius towards each star's
    /// texel at every reply, the final one included ([`Completeness`]), so that the listing, the
    /// band and the synthetic tier beyond R(u) share one boundary with no gap. A, B and the brown
    /// dwarfs are censused as without it. With no ceiling every bit is R06's. A ceiling not
    /// finite, or not brighter than the cut by at least 0.5 mag, is refused
    /// ([`BuildSkyQueryError::SyntheticCeiling`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::census::{BuildSkyQueryError, SkyQuery};
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::Magnitudes;
    ///
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
    /// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
    /// let query = |ceiling| {
    ///     SkyQuery::builder(observer.clone(), Magnitudes::new(7.95))
    ///         .synthetic_ceiling(Magnitudes::new(ceiling))
    ///         .build()
    /// };
    /// // RM3's interim states V 5.0, R13's synthetic tier V 4.5.
    /// assert_eq!(query(5.0)?.synthetic_ceiling(), Some(Magnitudes::new(5.0)));
    /// // A ceiling within half a magnitude of the cut is refused.
    /// assert_eq!(query(7.6), Err(BuildSkyQueryError::SyntheticCeiling));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn synthetic_ceiling(mut self, v: Magnitudes) -> Self {
        self.query.synthetic_ceiling = Some(v);
        self
    }

    /// The query.
    ///
    /// # Errors
    ///
    /// [`BuildSkyQueryError::Cut`] unless the cut is finite and at most [`MAX_CUT_V`],
    /// [`BuildSkyQueryError::NMax`] if `n_max` is above [`MAX_N_MAX`],
    /// [`BuildSkyQueryError::EyeCut`] if the eye's cut is asked without the eye, or is not finite
    /// or is deeper than the cut, [`BuildSkyQueryError::ConeWithEye`] if the eye is asked with a
    /// cone, [`BuildSkyQueryError::Illumination`] if the illumination was marched for another
    /// observer or at another time, and [`BuildSkyQueryError::EyeVisibility`] if the eye's
    /// visibility is asked without the eye, with an eye's cut shallower than the cut, or was taken
    /// for another observer, eye or cut, and [`BuildSkyQueryError::SyntheticCeiling`] if the
    /// synthetic ceiling is not finite or not brighter than the cut by at least 0.5 mag.
    pub fn build(mut self) -> Result<SkyQuery, BuildSkyQueryError> {
        let cut = self.query.cut.value();
        if !(cut.is_finite() && cut <= MAX_CUT_V) {
            return Err(BuildSkyQueryError::Cut);
        }
        if self
            .query
            .synthetic_ceiling
            .is_some_and(|v| !(v.value().is_finite() && cut - v.value() >= MIN_CEILING_DEPTH_MAG))
        {
            return Err(BuildSkyQueryError::SyntheticCeiling);
        }
        if self.query.n_max.get() > MAX_N_MAX {
            return Err(BuildSkyQueryError::NMax);
        }
        self.query.eye_cut = match (self.query.eye, self.query.eye_cut) {
            (None, None) => None,
            (Some(_), None) => Some(self.query.cut),
            (Some(_), Some(eye_cut)) if eye_cut.value().is_finite() && eye_cut.value() <= cut => {
                Some(eye_cut)
            }
            (None | Some(_), Some(_)) => return Err(BuildSkyQueryError::EyeCut),
        };
        if self.query.eye.is_some() && self.query.region.is_some() {
            return Err(BuildSkyQueryError::ConeWithEye);
        }
        if self
            .query
            .illumination
            .as_ref()
            .is_some_and(|light| light.observer() != &self.query.observer)
        {
            return Err(BuildSkyQueryError::Illumination);
        }
        if let Some(visibility) = &self.query.eye_visibility {
            let at = visibility.cut().value();
            let eye_only = self
                .query
                .eye_cut
                .is_some_and(|eye| eye.value().total_cmp(&cut).is_eq());
            let theirs = visibility.observer() == &self.query.observer
                && self.query.eye.as_ref() == Some(visibility.eye());
            if !(eye_only && theirs && at.total_cmp(&cut).is_eq()) {
                return Err(BuildSkyQueryError::EyeVisibility);
            }
        }
        Ok(self.query)
    }
}

/// What a census job reads and keeps: the tables, the envelope, the cells' offset bounds, its own
/// noise cache, the cell cache and the sources it shares with the other jobs (Design notes 10 and
/// 12).
///
/// A plain bundle of borrows, built by its caller per job, so its fields are public: it carries no
/// invariant a constructor could check, and the census reads the tables only through it, so their
/// source can change without touching the census.
pub struct SkyContext<'a> {
    /// The luminosity tables at the query's time.
    pub tables: &'a LuminosityTables,
    /// The brightness envelope.
    pub envelope: &'a BrightnessEnvelope,
    /// The galaxy's bounds on how far a cell's stars lie from their barycentres (R06.T8.f).
    pub offsets: &'a CellOffsets,
    /// The job's own noise cache, for the caps' rays and the stars' sightlines.
    pub noise: NoiseCache,
    /// The cell cache, its entries blocks of cells keyed by magnitude (R06.T8.h;
    /// [`super::cache::NoSkyCellCache`] keeps none).
    pub cells: &'a dyn SkyCellCache,
    /// The sources of systems beyond the grid (plan 09's feature members, once P09.T40 supplies
    /// them; none until then).
    pub sources: &'a [&'a dyn SystemSource],
    /// The gas's modifiers for the sightlines (plan 09's feature gas, once supplied).
    pub modifiers: &'a dyn GasModifierSource,
}

impl fmt::Debug for SkyContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkyContext")
            .field("sources", &self.sources.len())
            .finish_non_exhaustive()
    }
}

/// The census's plan: each layer's cap and the cells to open, shell by shell, nearest first
/// (R06.T8.i): its [`shells`](Self::shells) in their order, then each shell's cells as
/// [`cells_in_sphere`](crate::galaxy::query::cells_in_sphere) walks them.
///
/// It keeps each layer's padded sphere and streams the cells from it (R06.T8.f): near the Sun at
/// the eye's caps they number some 1.1 × 10⁸, which held as keys would take 2.2 GB. A server's
/// jobs take them slab by slab ([`slabs`](Self::slabs), or one shell's,
/// [`shell_slabs`](Self::shell_slabs)), each an x slab of one shell of one layer's walk. Where a
/// layer's cap is one radius a ray (R06.T7.b), its sphere is its farthest ray's, and the walk keeps
/// only the cells whose padded ball meets some ray's cone within that ray's radius, the region of
/// [`LayerCap::radius_toward`].
///
/// Each layer of [`SHELLED_LAYERS`] has one shell for each of [`SHELL_EDGES_LY`] below its cap's
/// farthest radius, then one to its cap; each other layer has one. A cell belongs to the first
/// shell whose sphere, the edge padded as a walk to that edge pads, its box meets, and to its
/// layer's last if none: every cell of the plan is in exactly one shell. The shells change which
/// cells the census opens not at all, so the census of every shell is the one-shot census, star
/// for star.
///
/// At a synthetic ceiling (rendering plan R13; [`census_plan_of`]) C's, D's and E's walks open
/// cells by their rays' cones widened by the band texel's largest radius, and their real boundary
/// held at [`REAL_LIMIT_LY`](super::super::caps::REAL_LIMIT_LY), itself one of the shell edges,
/// ends their last shell on it: near the Sun their shells are to 125, 250, 500 and 1,000 ly, then
/// to R(u). The census of every shell is then the one-shot census of the plan's cells less its
/// stars at or beyond the radius towards their texel.
#[derive(Debug, Clone, PartialEq)]
pub struct CensusPlan {
    caps: Vec<LayerCap>,
    walks: Vec<LayerWalk>,
    region: Option<ConeRegion>,
    apex: [f64; 3],
    observer: GalacticPosition,
    band_spec: BandSpec,
    /// Per layer of [`CAPPED_LAYERS`], whether it lists by the radius towards each star's texel at
    /// every reply: C, D and E at a synthetic ceiling ([`Completeness`]).
    by_texel: [bool; CAPPED_LAYERS.len()],
}

impl CensusPlan {
    /// The caps, one per layer of [`CAPPED_LAYERS`].
    #[must_use]
    pub fn caps(&self) -> &[LayerCap] {
        &self.caps
    }

    /// The cells, in canonical order: shell by shell as [`shells`](Self::shells) gives them, each
    /// shell's slab by slab, streamed.
    pub fn cells(&self) -> impl Iterator<Item = CellKey> + '_ {
        self.slabs().flat_map(|slab| slab.cells())
    }

    /// How many cells [`cells`](Self::cells) yields: counted column by column without visiting a
    /// cell for a layer of one radius, and by walking them for a cone or a layer of one radius a
    /// ray.
    #[must_use]
    pub fn cell_count(&self) -> u64 {
        self.walks
            .iter()
            .map(|walk| {
                if self.region.is_none() && walk.rays.is_none() {
                    // The layer's shells partition its sphere's cells.
                    count_cells_in_sphere(walk.layer, &walk.sphere)
                } else {
                    (0..walk.shell_count())
                        .flat_map(|index| self.slabs_of(walk, index))
                        .map(|slab| slab.cells().map(|_| 1_u64).sum::<u64>())
                        .sum()
                }
            })
            .sum()
    }

    /// The plan's jobs: every x slab of each shell's walk, shell by shell in
    /// [`shells`](Self::shells)' order, whose cells in turn are [`cells`](Self::cells). A slab may
    /// hold no cell.
    pub fn slabs(&self) -> impl Iterator<Item = CellSlab> + '_ {
        self.shells().flat_map(move |shell| self.shell_slabs(shell))
    }

    /// The plan's shells, nearest first.
    ///
    /// They are every layer's first, in [`CAPPED_LAYERS`]' order, then every layer's second, and
    /// so on, each layer's last completing it to its cap. A layer whose
    /// cap has no radius has none. A server may run them in another order, nearest first within
    /// each layer (R06.T11.d): the census of a set of shells does not depend on their order.
    pub fn shells(&self) -> impl Iterator<Item = Shell> + '_ {
        let ranks = self
            .walks
            .iter()
            .map(LayerWalk::shell_count)
            .max()
            .unwrap_or(0);
        (0..ranks).flat_map(move |index| {
            self.walks
                .iter()
                .filter(move |walk| index < walk.shell_count())
                .map(move |walk| walk.shell(index))
        })
    }

    /// The jobs of one of the plan's shells: every x slab of its walk.
    ///
    /// Each slab's cells are the shell's, in order. A slab may hold no cell.
    ///
    /// # Panics
    ///
    /// If `shell` is not one of the plan's ([`shells`](Self::shells)).
    pub fn shell_slabs(&self, shell: Shell) -> impl Iterator<Item = CellSlab> + '_ {
        let walk = self
            .walks
            .iter()
            .find(|walk| {
                walk.layer == shell.layer
                    && usize::from(shell.index) < walk.shell_count()
                    && walk.shell(usize::from(shell.index)) == shell
            })
            .unwrap_or_else(|| panic!("{shell:?} is not a shell of the plan"));
        self.slabs_of(walk, usize::from(shell.index))
    }

    /// The slabs of shell `index` of `walk`, one of the plan's.
    fn slabs_of<'a>(
        &'a self,
        walk: &'a LayerWalk,
        index: usize,
    ) -> impl Iterator<Item = CellSlab> + 'a {
        sphere_slabs(walk.layer, walk.shell_spheres(index).0).map(move |x| CellSlab {
            walk: walk.clone(),
            shell: index,
            x,
            region: self.region,
            apex: self.apex,
        })
    }

    /// How far a census of the shells `done` is complete, and so what it lists (R06.T8.i).
    ///
    /// Each layer is complete to the edge of its last shell of an unbroken run from its first in
    /// `done`, or to its cap once all of its shells are. A shell of `done` after a gap in its
    /// layer's run adds
    /// nothing until the gap is filled; one not of the plan is not read. A layer whose cap has no
    /// radius is final from the start.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::caps::{CAPPED_LAYERS, LayerCap};
    /// use hyperion_sim::sky::census::{SkyQuery, census_plan_of};
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::{LightYears, Magnitudes};
    ///
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
    /// let query = SkyQuery::builder(Observer::new(sun, UniverseTime::EPOCH)?, Magnitudes::new(7.95))
    ///     .build()?;
    /// // Caps as the caps near the Sun put them, uniform here: C has shells to 125, 250, 500,
    /// // 1,000, 2,000 and 4,000 ly, then its cap.
    /// let caps = [11.0, 68.0, 8_193.0, 9_925.0, 21_369.0, 1.0]
    ///     .iter()
    ///     .zip(CAPPED_LAYERS)
    ///     .map(|(&r, layer)| LayerCap::forced(layer, LightYears::new(r)))
    ///     .collect();
    /// let plan = census_plan_of(&query, caps);
    /// // The first reply: every layer's first shell, so A, B and the brown dwarfs are final.
    /// let first = plan.completeness(plan.shells().filter(|shell| shell.index() == 0));
    /// assert_eq!(first.edge(Layer::C), Some(LightYears::new(125.0)));
    /// assert_eq!(first.edge(Layer::A), None);
    /// assert_eq!(first.least_edge(), Some(LightYears::new(125.0)));
    /// assert!(!first.is_final() && plan.completeness(plan.shells()).is_final());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn completeness(&self, done: impl IntoIterator<Item = Shell>) -> Completeness {
        let mut marked: Vec<Vec<bool>> = self
            .walks
            .iter()
            .map(|walk| vec![false; walk.shell_count()])
            .collect();
        for shell in done {
            if let Some((w, walk)) = self
                .walks
                .iter()
                .enumerate()
                .find(|(_, walk)| walk.layer == shell.layer)
            {
                let index = usize::from(shell.index);
                if index < walk.shell_count() && walk.shell(index) == shell {
                    marked[w][index] = true;
                }
            }
        }
        let reached = CAPPED_LAYERS.map(|layer| {
            let Some(w) = self.walks.iter().position(|walk| walk.layer == layer) else {
                return Reached::Final;
            };
            let walk = &self.walks[w];
            let run = marked[w].iter().take_while(|&&done| done).count();
            if run == walk.shell_count() {
                Reached::Final
            } else {
                Reached::Edge(
                    run.checked_sub(1)
                        .map_or(LightYears::ZERO, |last| walk.edges[last].1.radius()),
                )
            }
        });
        let within = reached.map(|r| match r {
            Reached::Edge(edge) => Some(edge.value()),
            Reached::Final => None,
        });
        Completeness {
            observer: self.observer,
            band_spec: self.band_spec,
            reached,
            by_texel: self.by_texel,
            complete_to: CompleteTo::of_caps_within(&self.caps, within),
        }
    }

    /// The census's completeness once every shell is done: final, complete to the caps
    /// ([`CompleteTo::of_caps`]).
    #[must_use]
    pub fn complete(&self) -> Completeness {
        self.completeness(self.shells())
    }

    /// One reply for each place a shell can hold in its layer, nearest first, for the band's march.
    ///
    /// Reply k is complete in each layer to its shells to the k-th, or to its cap where it has
    /// fewer. Each layer's every
    /// radius a census of the plan can state at its first shell or after is among them, so a band
    /// marched for them all ([`march_rows`](super::super::band::march_rows)) sums any such census
    /// (R06.T9.f: one march a request, every reply's radii its nodes). A census in which some layer
    /// has no shell done is complete nowhere in it ([`CompleteTo::nowhere`]), which the march must
    /// then keep too.
    ///
    /// # Examples
    ///
    /// One march a request keeps every radius its census of shells can state, whichever shells are
    /// done when a reply is summed:
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
    /// use hyperion_sim::galaxy::gas::noise::NoiseCache;
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::band::{CubeFace, march_rows};
    /// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext, SkyQuery, census_plan};
    /// use hyperion_sim::sky::envelope::BrightnessEnvelope;
    /// use hyperion_sim::sky::luminosity::LuminosityTables;
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::Magnitudes;
    ///
    /// let galaxy = Galaxy::new(Seed::new(11));
    /// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
    /// let offsets = CellOffsets::build(&galaxy);
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
    /// let query = SkyQuery::builder(Observer::new(sun, UniverseTime::EPOCH)?, Magnitudes::new(7.95))
    ///     .build()?;
    /// let mut ctx = SkyContext {
    ///     tables: &tables,
    ///     envelope: &envelope,
    ///     offsets: &offsets,
    ///     noise: NoiseCache::with_capacity(1 << 16),
    ///     cells: &NoSkyCellCache,
    ///     sources: &[],
    ///     modifiers: &NoModifiers,
    /// };
    /// let plan = census_plan(&galaxy, &tables, &envelope, &query, &mut ctx.noise);
    /// let spec = query.band_spec();
    /// let march = march_rows(&galaxy, &mut ctx, &query, plan.replies(), &spec, CubeFace::PosZ, 0..64);
    /// // Every layer's first shell, and C's second: the march holds that census's radii.
    /// let done = plan
    ///     .shells()
    ///     .filter(|shell| shell.index() == 0 || (shell.layer() == Layer::C && shell.index() == 1));
    /// assert!(march.holds(plan.completeness(done).complete_to()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn replies(&self) -> Vec<CompleteTo> {
        let ranks = self
            .walks
            .iter()
            .map(LayerWalk::shell_count)
            .max()
            .unwrap_or(0)
            .max(1);
        (0..ranks)
            .map(|rank| {
                let done = self
                    .shells()
                    .filter(|shell| usize::from(shell.index) <= rank);
                self.completeness(done).complete_to
            })
            .collect()
    }
}

/// One layer's walk in a [`CensusPlan`]: the sphere of its cap, padded, the pad, the cap's radius
/// per ray, if it has one a ray, with the cones about its rays that the walk opens cells by, and
/// its shells' spheres.
#[derive(Debug, Clone, PartialEq)]
struct LayerWalk {
    layer: Layer,
    sphere: QuerySphere,
    pad: LightYears,
    rays: Option<(RayRadii, RayCones)>,
    /// Each shell's but the last's edge, whole light-years, and its sphere, nearest first: the edge,
    /// padded as a walk to that edge pads. The last shell walks `sphere`.
    edges: Arc<[(u32, QuerySphere)]>,
}

impl LayerWalk {
    /// The layer's shells: one for each edge, then its last.
    #[must_use]
    fn shell_count(&self) -> usize {
        self.edges.len() + 1
    }

    /// Shell `index` of the layer, below [`shell_count`](Self::shell_count).
    ///
    /// # Panics
    ///
    /// Never for a walk of the fixed edges, which gives a layer at most twelve shells, nor of a
    /// test's few.
    #[must_use]
    fn shell(&self, index: usize) -> Shell {
        let edge_ly = self.edges.get(index).map(|&(edge, _)| edge);
        Shell {
            index: u8::try_from(index).expect("a layer has far fewer than 256 shells"),
            layer: self.layer,
            edge_ly,
        }
    }

    /// The sphere shell `index` walks, and the one inside it whose cells are the shells' before it.
    #[must_use]
    fn shell_spheres(&self, index: usize) -> (&QuerySphere, Option<&QuerySphere>) {
        let outer = self
            .edges
            .get(index)
            .map_or(&self.sphere, |(_, sphere)| sphere);
        let inner = index.checked_sub(1).map(|before| &self.edges[before].1);
        (outer, inner)
    }
}

/// One x slab of one shell of one layer's walk in a [`CensusPlan`]: a census job's share of the
/// plan, which streams its own cells. Cloning shares the cap's radii per ray.
#[derive(Debug, Clone, PartialEq)]
pub struct CellSlab {
    walk: LayerWalk,
    shell: usize,
    x: i32,
    region: Option<ConeRegion>,
    apex: [f64; 3],
}

impl CellSlab {
    /// The layer whose cells the slab holds.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.walk.layer
    }

    /// The shell whose cells the slab holds (R06.T8.i).
    #[must_use]
    pub fn shell(&self) -> Shell {
        self.walk.shell(self.shell)
    }

    /// The slab's x coordinate on its layer's grid.
    #[must_use]
    pub const fn x(&self) -> i32 {
        self.x
    }

    /// The slab's cells, in canonical order.
    pub fn cells(&self) -> impl Iterator<Item = CellKey> + use<> {
        let slab = self.clone();
        let (outer, inner) = slab.walk.shell_spheres(slab.shell);
        let (outer, inner) = (*outer, inner.copied());
        cells_in_shell_slab(slab.walk.layer, &outer, inner.as_ref(), slab.x).filter(move |&key| {
            opens(
                slab.region.as_ref(),
                slab.walk.rays.as_ref(),
                slab.apex,
                key,
                slab.walk.pad,
            )
        })
    }
}

/// The offset of `key`'s centre from `apex` and the radius of its bounding ball padded by `pad`,
/// both in light-years.
#[must_use]
fn padded_ball(apex: [f64; 3], key: CellKey, pad: LightYears) -> ([f64; 3], f64) {
    let size = f64::from(key.layer().cell_size_ly());
    let half_diagonal = 0.5 * size * 3.0_f64.sqrt();
    let o = key.origin_ly();
    let offset = [
        f64::from(o[0]) + 0.5 * size - apex[0],
        f64::from(o[1]) + 0.5 * size - apex[1],
        f64::from(o[2]) + 0.5 * size - apex[2],
    ];
    (offset, half_diagonal + pad.value())
}

/// Whether a plan opens `key`, a cell of its layer's sphere, about `apex` (ly): its bounding ball,
/// padded by `pad`, meets the cone it opens cells by for `region`, if any (the cone widened to hold
/// every texel of its region, [`ConeRegion::meets_ball`]), and, for a cap of one radius a ray
/// (`rays`), some ray's cone of `rays`' cones nearer than the ray's radius (R06.T7.b; widened by
/// the band texel's largest radius at a synthetic ceiling, rendering plan R13, fix (i)).
#[must_use]
fn opens(
    region: Option<&ConeRegion>,
    rays: Option<&(RayRadii, RayCones)>,
    apex: [f64; 3],
    key: CellKey,
    pad: LightYears,
) -> bool {
    if region.is_none() && rays.is_none() {
        return true;
    }
    let (offset, radius) = padded_ball(apex, key, pad);
    region.is_none_or(|region| region.meets_ball(offset, radius))
        && rays.is_none_or(|(rays, cones)| rays.meets_ball(cones, offset, radius))
}

/// The years light takes to cross `d` ly, as a span.
///
/// # Panics
///
/// If `d` is not finite, which no cap is: a cap is the caps' own radius, under the root cube's
/// diagonal, or a forced one, checked against it.
fn light_time(d: f64) -> Span {
    Span::from_seconds_f64(d * SECONDS_PER_JULIAN_YEAR)
        .expect("a cap under the root cube's diagonal is a representable span")
}

/// The census's plan for `query` (Design notes 9 and 10): each layer's cap, one radius a ray
/// ([`layer_caps`], or [`layer_caps_by_visibility`] for an eye-only request that asks it; R06.T7.b)
/// unless forced, then the cells of [`cells_in_sphere`](crate::galaxy::query::cells_in_sphere) to
/// the cap's farthest radius, padded as the range query pads at the earliest emitted time that
/// radius allows; for a cap of one radius a ray, only the cells whose padded bounding ball meets
/// some ray's cone within that ray's radius; and, for a cone, only the cells whose padded bounding
/// ball meets the cone widened to hold every texel of its region, α + 2ρ ([`ConeRegion`];
/// R06.T8.l). The cells are taken shell by shell, nearest first, to [`SHELL_EDGES_LY`]
/// ([`CensusPlan`]; R06.T8.i).
///
/// A query at a synthetic ceiling ([`SkyQuery::synthetic_ceiling`]; rendering plan R13) that forces
/// no cap takes C's, D's and E's real boundary instead: the caps' rays measured once and counted at
/// the request's cut (by the eye's visibility where it asks it) and at the ceiling, and
/// [`real_boundary`](super::super::caps::real_boundary) of the two; A's, B's and the brown dwarfs'
/// caps are the cut's, bit for bit. Its plan is then [`census_plan_of`]'s at a ceiling.
///
/// # Panics
///
/// If a cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
pub fn census_plan(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
    query: &SkyQuery,
    cache: &mut NoiseCache,
) -> CensusPlan {
    let observer = query.observer();
    let caps = match (
        &query.forced_caps,
        query.synthetic_ceiling,
        query.eye_visibility(),
    ) {
        (Some(caps), _, _) => caps.clone(),
        (None, Some(ceiling), _) => {
            real_boundary_of(galaxy, tables, envelope, query, ceiling, cache)
        }
        (None, None, Some(visibility)) => {
            layer_caps_by_visibility(galaxy, tables, envelope, observer, visibility, cache)
        }
        (None, None, None) => layer_caps(galaxy, tables, envelope, observer, query.cut(), cache),
    };
    census_plan_of(query, caps)
}

/// The census's plan for `query` to `caps`, one per layer of [`CAPPED_LAYERS`], which the caller
/// has computed: [`census_plan`] after its caps, for a server that runs the caps' rays as jobs of
/// its own ([`layer_caps_over`](crate::sky::caps::layer_caps_over); R06.T11.c). The query's own
/// forced caps, if it has them, are not read: the plan takes `caps`.
///
/// Where the query states a synthetic ceiling (rendering plan R13, Design note 3), `caps` are C's,
/// D's and E's real boundary R(u) ([`real_boundary`](super::super::caps::real_boundary)), and the
/// plan censuses them in shells to it with their cones widened by the query's band texel's largest
/// radius ρ (fix (i)), each listed by the radius towards its stars' band texel at every reply, the
/// final one included ([`Completeness`]). With no ceiling every bit is R06's.
///
/// # Panics
///
/// If a cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
pub fn census_plan_of(query: &SkyQuery, caps: Vec<LayerCap>) -> CensusPlan {
    plan_with_edges(query, caps, &SHELL_EDGES_LY)
}

/// [`census_plan_of`] with the shells' edges `edges_ly`, ascending, in place of
/// [`SHELL_EDGES_LY`]: a test's nearer edges, so that a census a test can afford still has several
/// shells. The server's tests reach it through their own seam (R06.T11.d); no sky a client asks
/// takes other edges than [`SHELL_EDGES_LY`], which are constants.
///
/// # Panics
///
/// Unless every edge is above nought, they ascend and there are fewer than 255, and as
/// [`census_plan_of`].
#[doc(hidden)]
#[must_use]
pub fn census_plan_with_edges(
    query: &SkyQuery,
    caps: Vec<LayerCap>,
    edges_ly: &[u32],
) -> CensusPlan {
    assert!(
        edges_ly.first().is_none_or(|&first| first > 0)
            && edges_ly.windows(2).all(|pair| pair[0] < pair[1])
            && edges_ly.len() < usize::from(u8::MAX),
        "shell edges are above nought and ascend, fewer than 255 of them: {edges_ly:?}"
    );
    plan_with_edges(query, caps, edges_ly)
}

/// [`census_plan_of`] with the shells' edges `edges_ly`, ascending: [`SHELL_EDGES_LY`], or a test's
/// nearer ones, so that a test's census can afford shells.
#[must_use]
pub(crate) fn plan_with_edges(
    query: &SkyQuery,
    caps: Vec<LayerCap>,
    edges_ly: &[u32],
) -> CensusPlan {
    debug_assert!(
        edges_ly.windows(2).all(|pair| pair[0] < pair[1]),
        "the shells' edges ascend"
    );
    let observer = *query.observer().position();
    let walks = caps
        .iter()
        .filter_map(|cap| layer_walk(query, cap, edges_ly))
        .collect();
    let by_texel = CAPPED_LAYERS
        .map(|layer| query.synthetic_ceiling.is_some() && REAL_BOUNDARY_LAYERS.contains(&layer));
    CensusPlan {
        caps,
        walks,
        region: query.cone_region().copied(),
        apex: observer.to_light_years_f64(),
        observer,
        band_spec: query.band_spec(),
        by_texel,
    }
}

/// The sphere of `radius` about `query`'s observer for `layer`, padded as the range query pads at
/// the earliest emitted time that radius allows, and the pad.
///
/// # Panics
///
/// If the sphere cannot be built, which a positive finite radius never fails.
#[must_use]
fn padded_sphere(query: &SkyQuery, layer: Layer, radius: LightYears) -> (QuerySphere, LightYears) {
    let observer = query.observer();
    let t = observer.time();
    // An observer's time is within ±1,000 years and a cap's light time within 2¹⁸ years, so
    // the subtraction never leaves the clock's range.
    let earliest = t
        .checked_sub(light_time(radius.value()))
        .expect("an observer's time less a cap's light time is on the clock");
    let speed = pad_speed(layer);
    let pad = LightYears::new(
        pad_for(earliest, speed)
            .value()
            .max(pad_for(t, speed).value()),
    );
    let sphere = QuerySphere::new(*observer.position(), radius, t, pad)
        .expect("a positive finite cap and pad make a sphere");
    (sphere, pad)
}

/// The walk of `cap`'s layer for `query`: [`cells_in_sphere`](crate::galaxy::query::cells_in_sphere)
/// to the cap's farthest radius, padded as the range query pads at the earliest emitted time that
/// radius allows, with the cap's radius per ray if it has one, and, for a layer of
/// [`SHELLED_LAYERS`], the spheres of the edges of `edges_ly` below that radius, each padded so;
/// `None` for a cap of no radius.
///
/// # Panics
///
/// If the cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
fn layer_walk(query: &SkyQuery, cap: &LayerCap, edges_ly: &[u32]) -> Option<LayerWalk> {
    let layer = cap.layer();
    let radius = cap.radius();
    if radius.value() <= 0.0 {
        return None;
    }
    let (sphere, pad) = padded_sphere(query, layer, radius);
    let edges: Arc<[(u32, QuerySphere)]> = if SHELLED_LAYERS.contains(&layer) {
        edges_ly
            .iter()
            .take_while(|&&edge| f64::from(edge) < radius.value())
            .map(|&edge| {
                let at = LightYears::new(f64::from(edge));
                (edge, padded_sphere(query, layer, at).0)
            })
            .collect()
    } else {
        Arc::new([])
    };
    // At a synthetic ceiling C's, D's and E's cones are widened by the band texel's largest
    // radius, so that every star nearer than the radius towards its texel lies in an opened cell
    // (rendering plan R13, Design note 3; fix (i)).
    let widening = if query.synthetic_ceiling.is_some() && REAL_BOUNDARY_LAYERS.contains(&layer) {
        query.band_spec().largest_texel_radius()
    } else {
        Radians::new(0.0)
    };
    Some(LayerWalk {
        layer,
        sphere,
        pad,
        rays: cap
            .rays()
            .map(|rays| (rays.clone(), rays.cones_widened_by(widening))),
        edges,
    })
}

/// The cells a census of `query` opens for `caps`, in [`CensusPlan::cells`]' order, shell by shell:
/// each layer's cells of [`cells_in_sphere`](crate::galaxy::query::cells_in_sphere) to its cap's
/// farthest radius, padded as the range query pads at the earliest emitted time that radius allows;
/// for a cap of one radius a ray only those whose padded bounding ball meets some ray's cone, of
/// half-angle the lattice's spacing, nearer than the ray's radius (R06.T7.b), so that the census is
/// complete towards each direction to the cap's radius towards it ([`LayerCap::radius_toward`]);
/// and for a cone only those whose padded bounding ball meets the cone widened to hold every texel
/// of its region (R06.T8.l). A cap of no radius opens nothing. Held, for a caller whose cells are
/// few: a [`CensusPlan`] streams the same cells ([`CensusPlan::cells`]).
///
/// # Panics
///
/// If a cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
pub fn plan_cells(query: &SkyQuery, caps: Vec<LayerCap>) -> Vec<CellKey> {
    census_plan_of(query, caps).cells().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use hyperion_testkit::float::bits;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::sky::caps::{CAP_RAYS, CapLattice};
    use crate::sky::census::cache::NoSkyCellCache;
    use crate::sky::census::cell::{SkyStar, census_cell};
    use crate::sky::census::merge::{SkyCensus, merge_census, merge_shells};
    use crate::sky::testing::{milky_way_dark_tables, milky_way_envelope, milky_way_offsets};
    use crate::time::UniverseTime;

    fn observer() -> Observer {
        Observer::new(
            GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer")
    }

    #[test]
    fn every_refusal_names_its_field() {
        let build = |cut: f64| SkyQuery::builder(observer(), Magnitudes::new(cut)).build();
        assert_eq!(build(f64::NAN), Err(BuildSkyQueryError::Cut));
        assert_eq!(build(11.01), Err(BuildSkyQueryError::Cut));
        assert!(build(11.0).is_ok());
        let n = SkyQuery::builder(observer(), Magnitudes::new(7.0))
            .n_max(NonZeroU32::new(MAX_N_MAX + 1).expect("not zero"))
            .build();
        assert_eq!(n, Err(BuildSkyQueryError::NMax));
        assert_eq!(
            Cone::new(UnitVector::NORTH, Degrees::new(0.0)),
            Err(BuildSkyQueryError::ConeHalfAngle)
        );
        assert_eq!(
            Cone::new(UnitVector::NORTH, Degrees::new(90.5)),
            Err(BuildSkyQueryError::ConeHalfAngle)
        );
        assert!(Cone::new(UnitVector::NORTH, Degrees::new(90.0)).is_ok());
        let query = || {
            SkyQuery::builder(observer(), Magnitudes::new(7.0))
                .build()
                .expect("valid")
        };
        for bad in [f64::NAN, -1.0, f64::INFINITY, 300_000.0] {
            assert_eq!(
                query().with_caps_forced(LightYears::new(bad)),
                Err(BuildSkyQueryError::ForcedCap),
                "{bad}"
            );
        }
        assert!(query().with_caps_forced(LightYears::ZERO).is_ok());
        let eye = |cut: f64, eye_cut: Option<f64>, asked: bool| {
            let builder = SkyQuery::builder(observer(), Magnitudes::new(cut));
            let builder = if asked {
                builder.eye(EyeObserver::default())
            } else {
                builder
            };
            match eye_cut {
                Some(eye_cut) => builder.eye_cut(Magnitudes::new(eye_cut)),
                None => builder,
            }
            .build()
            .map(|query| query.eye_cut().map(Magnitudes::value))
        };
        assert_eq!(eye(10.06, Some(8.15), true), Ok(Some(8.15)));
        assert_eq!(eye(8.15, Some(8.15), true), Ok(Some(8.15)));
        assert_eq!(eye(8.15, None, true), Ok(Some(8.15)), "an eye-only request");
        assert_eq!(eye(8.15, None, false), Ok(None), "no eye");
        for (cut, eye_cut, asked) in [
            (8.15, 10.06, true),
            (8.15, f64::NAN, true),
            (8.15, f64::NEG_INFINITY, true),
            (10.06, 8.15, false),
        ] {
            assert_eq!(
                eye(cut, Some(eye_cut), asked),
                Err(BuildSkyQueryError::EyeCut),
                "cut {cut}, the eye's {eye_cut}, the eye asked: {asked}"
            );
        }
        let forced = SkyQuery::builder(observer(), Magnitudes::new(10.06))
            .eye(EyeObserver::default())
            .eye_cut(Magnitudes::new(8.15))
            .build()
            .and_then(|query| query.with_caps_forced(LightYears::new(100.0)));
        assert_eq!(
            forced.map(|query| query.eye_cut()),
            Ok(Some(Magnitudes::new(8.15))),
            "a forced cap keeps the eye's cut"
        );
        for error in [
            BuildSkyQueryError::Cut,
            BuildSkyQueryError::NMax,
            BuildSkyQueryError::ConeHalfAngle,
            BuildSkyQueryError::ForcedCap,
            BuildSkyQueryError::EyeCut,
            BuildSkyQueryError::ConeWithEye,
            BuildSkyQueryError::Illumination,
            BuildSkyQueryError::EyeVisibility,
            BuildSkyQueryError::SyntheticCeiling,
        ] {
            let text = error.to_string();
            let field = match error {
                BuildSkyQueryError::Cut => "cut",
                BuildSkyQueryError::NMax => "n_max",
                BuildSkyQueryError::ConeHalfAngle => "half-angle",
                BuildSkyQueryError::ForcedCap => "forced cap",
                BuildSkyQueryError::EyeCut => "eye's cut",
                BuildSkyQueryError::ConeWithEye => "cone",
                BuildSkyQueryError::Illumination => "illumination",
                BuildSkyQueryError::EyeVisibility => "visibility",
                BuildSkyQueryError::SyntheticCeiling => "synthetic ceiling",
            };
            assert!(text.contains(field), "{text}");
        }
    }

    /// A synthetic ceiling is finite and brighter than the cut by half a magnitude at least
    /// (rendering plan R13, R13.T2.a), with the eye or without it; a forced cap keeps it, and a
    /// query states none unless asked.
    #[test]
    fn a_synthetic_ceiling_is_half_a_magnitude_brighter_than_the_cut_at_least() {
        let ceiling = |cut: f64, ceiling: f64, eye: bool| {
            let builder = SkyQuery::builder(observer(), Magnitudes::new(cut));
            let builder = if eye {
                builder.eye(EyeObserver::default())
            } else {
                builder
            };
            builder
                .synthetic_ceiling(Magnitudes::new(ceiling))
                .build()
                .map(|query| query.synthetic_ceiling().map(Magnitudes::value))
        };
        for eye in [false, true] {
            assert_eq!(ceiling(7.95, 5.0, eye), Ok(Some(5.0)));
            assert_eq!(ceiling(7.0, 6.5, eye), Ok(Some(6.5)), "half a magnitude");
            assert_eq!(ceiling(7.0, -30.0, eye), Ok(Some(-30.0)));
            for (cut, v) in [
                (7.0, 6.6),
                (7.0, 7.0),
                (7.0, 9.0),
                (7.0, f64::NAN),
                (7.0, f64::NEG_INFINITY),
                (7.0, f64::INFINITY),
            ] {
                assert_eq!(
                    ceiling(cut, v, eye),
                    Err(BuildSkyQueryError::SyntheticCeiling),
                    "cut {cut}, ceiling {v}, the eye asked: {eye}"
                );
            }
        }
        let at_ceiling = SkyQuery::builder(observer(), Magnitudes::new(7.95))
            .synthetic_ceiling(Magnitudes::new(5.0))
            .build()
            .and_then(|query| query.with_caps_forced(LightYears::new(100.0)));
        assert_eq!(
            at_ceiling.map(|query| query.synthetic_ceiling()),
            Ok(Some(Magnitudes::new(5.0))),
            "a forced cap keeps the ceiling"
        );
        let none = SkyQuery::builder(observer(), Magnitudes::new(7.0)).build();
        assert_eq!(
            none.map(|query| query.synthetic_ceiling()),
            Ok(None),
            "no ceiling unless stated"
        );
    }

    /// The naked eye cannot ask a cone (R06.T8.l; decided 2026-10-07, `decision-r06-t8k-cone.md`,
    /// item 2): a cone is an instrument's field stop, and the eye's veil reaches 90°, so an eye
    /// with a cone would take its limits without the glare of every star outside the field. Either
    /// alone is a query, whichever is set first and whatever the eye's cut.
    #[test]
    fn an_eye_with_a_cone_is_refused() {
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(2.5)).expect("a cone");
        let builder = || SkyQuery::builder(observer(), Magnitudes::new(10.06));
        let eye = EyeObserver::default();
        for refused in [
            builder().eye(eye).cone(cone),
            builder().cone(cone).eye(eye),
            builder().eye(eye).eye_cut(Magnitudes::new(8.15)).cone(cone),
            builder()
                .cone(cone)
                .band_spec(BandSpec::new(16, 12).expect("a spec"))
                .eye(eye),
        ] {
            assert_eq!(refused.build(), Err(BuildSkyQueryError::ConeWithEye));
        }
        let narrow = builder().cone(cone).build().expect("a cone alone");
        assert_eq!(narrow.cone(), Some(&cone));
        assert_eq!(narrow.eye(), None);
        let eye_only = builder().eye(eye).build().expect("the eye alone");
        assert_eq!(eye_only.cone_region(), None);
        assert_eq!(
            BuildSkyQueryError::ConeWithEye.to_string(),
            "the naked eye cannot ask a cone: it has no field stop"
        );
    }

    /// The eye's visibility caps an eye-only request of its observer and eye at the cut it was
    /// taken at (R06.T7.b): asked without the eye, with a camera's deeper cut, at another cut, or
    /// for another observer or eye, it is refused.
    #[test]
    fn the_eyes_visibility_is_an_eye_only_requests_at_its_cut() {
        let at = |cut: f64| {
            EyeVisibility::uniform(observer(), EyeObserver::default(), Magnitudes::new(cut))
        };
        let builder = |cut: f64| SkyQuery::builder(observer(), Magnitudes::new(cut));
        let eye = EyeObserver::default();
        let eye_only = builder(8.28)
            .eye(eye)
            .eye_visibility(at(8.28))
            .build()
            .expect("an eye-only request at its cut");
        assert_eq!(eye_only.eye_visibility(), Some(&at(8.28)));
        assert_eq!(
            builder(8.28)
                .eye(eye)
                .eye_cut(Magnitudes::new(8.28))
                .eye_visibility(at(8.28))
                .build()
                .map(|query| query.eye_visibility().is_some()),
            Ok(true)
        );
        for refused in [
            builder(8.28).eye_visibility(at(8.28)),
            builder(10.06)
                .eye(eye)
                .eye_cut(Magnitudes::new(8.28))
                .eye_visibility(at(8.28)),
            builder(10.06)
                .eye(eye)
                .eye_cut(Magnitudes::new(8.28))
                .eye_visibility(at(10.06)),
            builder(8.28).eye(eye).eye_visibility(at(8.15)),
            // Another observer's, or another eye's.
            builder(8.28)
                .eye(eye)
                .eye_visibility(EyeVisibility::uniform(
                    Observer::new(
                        GalacticPosition::from_light_years([0.0, 26_001.0, 0.0])
                            .expect("in the cube"),
                        UniverseTime::EPOCH,
                    )
                    .expect("an observer"),
                    eye,
                    Magnitudes::new(8.28),
                )),
            builder(8.28)
                .eye(eye)
                .eye_visibility(EyeVisibility::uniform(
                    observer(),
                    EyeObserver::new(EyeObserver::DEFAULT_FIELD_FACTOR, 70.0, 0.5).expect("an eye"),
                    Magnitudes::new(8.28),
                )),
        ] {
            assert_eq!(refused.build(), Err(BuildSkyQueryError::EyeVisibility));
        }
        assert_eq!(
            builder(8.28)
                .eye(eye)
                .build()
                .map(|q| q.eye_visibility().is_none()),
            Ok(true)
        );
    }

    /// A cone's region is made of the query's band's texels, set before or after the cone, and
    /// widens the cone by that band's largest texel radius, 1.266° at the server's 64² and 5.05°
    /// at 16² (R06.T8.l).
    #[test]
    fn a_cones_region_is_of_the_querys_band() {
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(1.0)).expect("a cone");
        let coarse = BandSpec::new(16, 12).expect("a spec");
        let builder = || SkyQuery::builder(observer(), Magnitudes::new(10.0));
        let standard = builder().cone(cone).build().expect("valid");
        let after = builder()
            .cone(cone)
            .band_spec(coarse)
            .build()
            .expect("valid");
        let before = builder()
            .band_spec(coarse)
            .cone(cone)
            .build()
            .expect("valid");
        assert_eq!(standard.band_spec(), BandSpec::STANDARD);
        assert_eq!(after, before);
        let reach = |query: &SkyQuery| {
            let region = query.cone_region().expect("a region");
            assert_eq!(region.spec(), query.band_spec());
            assert_eq!(region.cone(), &cone);
            Degrees::from(region.reach()).value()
        };
        assert!(
            (reach(&standard) - 2.266).abs() < 0.001,
            "{}",
            reach(&standard)
        );
        assert!((reach(&after) - 6.051).abs() < 0.001, "{}", reach(&after));
        let region = after.cone_region().expect("a region");
        assert_eq!(
            hyperion_testkit::float::bits(region.cos_reach()),
            hyperion_testkit::float::bits(math::cos(region.reach().value()))
        );
    }

    /// A plan with every cap forced, for the cell tests (no tables or rays are read).
    fn forced_plan(radius: f64, cone: Option<Cone>) -> CensusPlan {
        forced_query_and_plan(radius, cone).1
    }

    /// [`forced_plan`]'s query and plan.
    fn forced_query_and_plan(radius: f64, cone: Option<Cone>) -> (SkyQuery, CensusPlan) {
        let galaxy = milky_way_galaxy();
        let tables = LuminosityTables::build_with(
            galaxy,
            UniverseTime::EPOCH,
            &[],
            crate::sky::luminosity::BuildOptions::STANDARD,
        );
        let envelope = BrightnessEnvelope::build_with(&[0.0], 2);
        let mut builder = SkyQuery::builder(observer(), Magnitudes::new(7.0));
        if let Some(cone) = cone {
            builder = builder.cone(cone);
        }
        let query = builder
            .build()
            .expect("a valid query")
            .with_caps_forced(LightYears::new(radius))
            .expect("a valid cap");
        let mut cache = NoiseCache::with_capacity(16);
        let plan = census_plan(galaxy, &tables, &envelope, &query, &mut cache);
        (query, plan)
    }

    /// The plan to caps a caller computed is the plan of the query that forces them: the plan the
    /// server makes after its caps' ray jobs (R06.T11.c).
    #[test]
    fn a_plan_to_given_caps_is_the_plan_that_forces_them() {
        let (forced, plan) = forced_query_and_plan(300.0, None);
        let unforced = SkyQuery::builder(observer(), Magnitudes::new(7.0))
            .build()
            .expect("a valid query");
        assert_eq!(census_plan_of(&unforced, plan.caps().to_vec()), plan);
        assert_eq!(census_plan_of(&forced, plan.caps().to_vec()), plan);
    }

    #[test]
    fn forced_caps_replace_every_layers_and_the_cells_cover_each_sphere_in_order() {
        let plan = forced_plan(300.0, None);
        assert!(
            plan.caps()
                .iter()
                .all(|c| (c.radius().value() - 300.0).abs() < 1e-12)
        );
        assert_eq!(plan.caps().len(), CAPPED_LAYERS.len());
        let apex = observer().position().to_light_years_f64();
        let cells: Vec<CellKey> = plan.cells().collect();
        // Every cell whose box meets the sphere is there, and the layers come in order within
        // each rank of shells, each shell's cells its layer's: at 300 ly C, D and E have three
        // shells, to 125, 250 and 300 ly (R06.T8.i at R06.T11.g's edges).
        let mut last = (0_u8, 0_usize);
        let mut in_shells = Vec::with_capacity(cells.len());
        for shell in plan.shells() {
            let rank = CAPPED_LAYERS
                .iter()
                .position(|&l| l == shell.layer())
                .expect("a capped layer");
            assert!((shell.index(), rank) >= last, "{shell:?} after {last:?}");
            last = (shell.index(), rank);
            for key in plan.shell_slabs(shell).flat_map(|slab| slab.cells()) {
                assert_eq!(key.layer(), shell.layer(), "{key:?} in {shell:?}");
                in_shells.push(key);
            }
        }
        assert_eq!(in_shells, cells);
        assert_eq!(
            plan.shells()
                .filter(|shell| shell.layer() == Layer::C)
                .count(),
            3
        );
        for &layer in &CAPPED_LAYERS {
            let size = i32::try_from(layer.cell_size_ly()).expect("small");
            let r = 300.0;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "cell indices near the Sun-like point, far inside i32"
            )]
            let lo = |a: f64| ((a - r) / f64::from(size)).floor() as i32;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "cell indices near the Sun-like point, far inside i32"
            )]
            let hi = |a: f64| ((a + r) / f64::from(size)).floor() as i32;
            for x in lo(apex[0])..=hi(apex[0]) {
                for y in lo(apex[1])..=hi(apex[1]) {
                    for z in lo(apex[2])..=hi(apex[2]) {
                        let key = CellKey::new(layer, [x, y, z]).expect("in the cube");
                        let o = key.origin_ly();
                        // The nearest point of the box to the observer.
                        let near = |k: usize| {
                            let a = f64::from(o[k]);
                            apex[k].clamp(a, a + f64::from(size)) - apex[k]
                        };
                        let d = (near(0) * near(0) + near(1) * near(1) + near(2) * near(2)).sqrt();
                        if d <= r {
                            assert!(cells.contains(&key), "{key:?} is missing");
                        }
                    }
                }
            }
        }
    }

    /// A cone's plan keeps only the cells whose padded bounding ball meets the cone widened to hold
    /// every texel of its region, α + 2ρ (R06.T8.l): 10° and twice 1.266° at the server's 64².
    #[test]
    fn a_cone_keeps_only_cells_whose_box_meets_it() {
        let all: Vec<CellKey> = forced_plan(400.0, None).cells().collect();
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(10.0)).expect("a cone");
        let (query, plan) = forced_query_and_plan(400.0, Some(cone));
        let narrow: Vec<CellKey> = plan.cells().collect();
        assert!(narrow.len() < all.len() / 5);
        let region = query.cone_region().expect("a region");
        let apex = observer().position().to_light_years_f64();
        for key in &narrow {
            assert!(all.contains(key));
        }
        // The cone the plan opens by: wider than the cone, by twice the band's texel radius.
        let opening = 10.0 * RADIANS_PER_DEGREE
            + 2.0 * BandSpec::STANDARD.largest_texel_radius().value()
            + PLAN_MARGIN_RAD;
        let mut widened = 0_usize;
        // Every dropped cell's box misses that cone: its bounding ball does.
        for key in all.iter().filter(|k| !narrow.contains(k)) {
            let size = f64::from(key.layer().cell_size_ly());
            let o = key.origin_ly();
            let offset = [
                f64::from(o[0]) + 0.5 * size - apex[0],
                f64::from(o[1]) + 0.5 * size - apex[1],
                f64::from(o[2]) + 0.5 * size - apex[2],
            ];
            let radius = 0.5 * size * 3.0_f64.sqrt();
            assert!(!region.meets_ball(offset, radius));
            assert!(!ball_meets_cone(cone.axis(), opening, offset, radius));
        }
        // And the plan opens cells whose padded ball the cone alone misses.
        for key in &narrow {
            let size = f64::from(key.layer().cell_size_ly());
            let o = key.origin_ly();
            let offset = [
                f64::from(o[0]) + 0.5 * size - apex[0],
                f64::from(o[1]) + 0.5 * size - apex[1],
                f64::from(o[2]) + 0.5 * size - apex[2],
            ];
            let pad = plan
                .walks
                .iter()
                .find(|walk| walk.layer == key.layer())
                .expect("a walk of the cell's layer")
                .pad
                .value();
            let radius = 0.5 * size * 3.0_f64.sqrt() + pad;
            if !ball_meets_cone(cone.axis(), 10.0 * RADIANS_PER_DEGREE, offset, radius) {
                widened += 1;
            }
        }
        assert!(widened > 0, "the widened cone opens more cells");
    }

    /// The plan's streamed cells, slab by slab, are its shells' in order, for the queries of the
    /// tests above: each shell's the cells of `cells_in_sphere` to its sphere that the sphere
    /// inside it lacks, opened as the plan opens them, in the walk's order; and its count is theirs
    /// (R06.T8.f, R06.T8.i). The held [`plan_cells`] are the same cells.
    #[test]
    fn streamed_cells_are_plan_cells() {
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(10.0)).expect("a cone");
        let cases: [(f64, Option<Cone>, &[u32]); 3] = [
            (300.0, None, &SHELL_EDGES_LY),
            (400.0, None, &[100, 200]),
            (400.0, Some(cone), &[100, 200]),
        ];
        for (radius, cone, edges) in cases {
            let (query, plan) = forced_query_and_plan(radius, cone);
            let plan = plan_with_edges(&query, plan.caps().to_vec(), edges);
            let mut expected = Vec::new();
            for shell in plan.shells() {
                let walk = plan
                    .walks
                    .iter()
                    .find(|walk| walk.layer == shell.layer())
                    .expect("the shell's walk");
                let (outer, inner) = walk.shell_spheres(usize::from(shell.index()));
                let held: BTreeSet<CellKey> = inner
                    .map(|inner| crate::galaxy::query::cells_in_sphere(walk.layer, inner).collect())
                    .unwrap_or_default();
                expected.extend(
                    crate::galaxy::query::cells_in_sphere(walk.layer, outer).filter(|&key| {
                        !held.contains(&key)
                            && opens(
                                query.cone_region(),
                                walk.rays.as_ref(),
                                plan.apex,
                                key,
                                walk.pad,
                            )
                    }),
                );
            }
            let streamed: Vec<CellKey> = plan.cells().collect();
            assert!(!streamed.is_empty());
            assert_eq!(
                streamed, expected,
                "{radius} ly, cone {cone:?}, edges {edges:?}"
            );
            assert_eq!(
                plan.cell_count(),
                u64::try_from(expected.len()).expect("few"),
                "{radius} ly, cone {cone:?}"
            );
            let mut by_slab = Vec::new();
            for slab in plan.slabs() {
                for key in slab.cells() {
                    assert_eq!(key.layer(), slab.layer());
                    let size = i32::try_from(key.size_ly()).expect("small");
                    assert_eq!(key.origin_ly()[0], slab.x() * size);
                    by_slab.push(key);
                }
            }
            assert_eq!(by_slab, expected);
            let held: BTreeSet<CellKey> = plan_cells(&query, plan.caps().to_vec())
                .into_iter()
                .collect();
            assert_eq!(held, expected.iter().copied().collect::<BTreeSet<_>>());
        }
    }

    /// Radii a ray of the standard lattice between `lo` and `hi` ly, varying from ray to ray and
    /// over the sky: a jagged region, whose cones each hold a different reach.
    fn jagged_radii(seed: u64, lo: f64, hi: f64) -> RayRadii {
        let lattice = Arc::new(CapLattice::new(CAP_RAYS));
        let radii = lattice
            .directions()
            .iter()
            .zip(crate::sky::testing::uniforms(seed))
            .map(|(d, u)| {
                let [x, y, z] = d.components();
                let wave = 0.5 + 0.25 * math::sin(3.0 * x + 2.0 * y) + 0.25 * math::cos(5.0 * z);
                lo + (hi - lo) * (0.5 * wave + 0.5 * u)
            })
            .collect();
        RayRadii::new(lattice, radii)
    }

    /// A query near the Sun-like point at `cut` whose every layer's cap is one radius a ray,
    /// jagged between `lo` and `hi` ly, each layer's of its own seed.
    fn query_by_ray(cut: f64, lo: f64, hi: f64) -> SkyQuery {
        let mut query = SkyQuery::builder(observer(), Magnitudes::new(cut))
            .build()
            .expect("a valid query");
        query.forced_caps = Some(
            CAPPED_LAYERS
                .iter()
                .zip(0_u64..)
                .map(|(&layer, k)| {
                    LayerCap::forced_by_ray(layer, jagged_radii(0x7b_0100 + k, lo, hi))
                })
                .collect(),
        );
        query
    }

    /// A plan whose caps are one radius a ray (R06.T7.b) opens, in each layer, every cell that
    /// holds a point within the cap's radius towards it, at 2 × 10⁴ random points, and only cells
    /// of the sphere of its farthest ray; streamed, slab by slab, and counted, its cells are
    /// [`plan_cells`]'.
    #[test]
    fn a_plan_by_ray_opens_every_cell_within_each_directions_radius() {
        let query = query_by_ray(7.0, 60.0, 400.0);
        let caps = query.forced_caps.clone().expect("forced caps");
        let held = plan_cells(&query, caps.clone());
        let opened: BTreeSet<CellKey> = held.iter().copied().collect();
        let spheres: Vec<LayerCap> = caps
            .iter()
            .map(|cap| LayerCap::forced(cap.layer(), cap.radius()))
            .collect();
        let round: BTreeSet<CellKey> = plan_cells(&query, spheres).into_iter().collect();
        assert!(opened.is_subset(&round));
        assert!(
            opened.len() < round.len() / 2,
            "{} of {}",
            opened.len(),
            round.len()
        );
        let apex = observer().position().to_light_years_f64();
        let mut uniforms = crate::sky::testing::uniforms(0x7b_0200);
        let (mut inside, mut outside) = (0_u32, 0_u32);
        for _ in 0..20_000 {
            let mut next = || uniforms.next().expect("endless");
            let u = UnitVector::from_components(std::array::from_fn(|_| 2.0 * next() - 1.0))
                .expect("a direction");
            let d = 450.0 * next();
            let p = u.components().map(|c| c * d);
            for cap in &caps {
                let size = f64::from(cap.layer().cell_size_ly());
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "cell indices near the Sun-like point, far inside i32"
                )]
                let index = |k: usize| ((apex[k] + p[k]) / size).floor() as i32;
                let key =
                    CellKey::new(cap.layer(), [index(0), index(1), index(2)]).expect("in the cube");
                if d < cap.radius_toward(u).value() {
                    inside += 1;
                    assert!(
                        opened.contains(&key),
                        "{key:?} holds {p:?}, within its radius"
                    );
                } else {
                    outside += 1;
                }
            }
        }
        assert!(inside > 10_000 && outside > 10_000, "{inside}, {outside}");
        let plan = census_plan_of(&query, caps.clone());
        let streamed: Vec<CellKey> = plan.cells().collect();
        assert_eq!(streamed, held);
        let by_slab: Vec<CellKey> = plan.slabs().flat_map(|slab| slab.cells()).collect();
        assert_eq!(by_slab, held);
        assert_eq!(plan.cell_count(), u64::try_from(held.len()).expect("few"));
    }

    /// The census by ray (R06.T7.b) near the Sun to V 8, every layer's cap jagged between 40 and
    /// 150 ly: it lists every star of a census with every cap forced to 160 ly that lies within
    /// its layer's radius towards it, so no direction is left without cells; each star it lists
    /// is that census's, bit for bit; and no star it lists lies beyond its own rays' radii by more
    /// than the cells' overshoot the final reply already allows, a cell's diagonal and twice its
    /// pad and 10 ly of the stars' offsets from their barycentres, in distance and in the angle
    /// that subtends.
    #[test]
    fn a_census_by_ray_lists_every_star_within_its_radii_and_none_beyond_its_cells() {
        let galaxy = milky_way_galaxy();
        let by_ray = query_by_ray(8.0, 40.0, 150.0);
        let full = by_ray
            .clone()
            .with_caps_forced(LightYears::new(160.0))
            .expect("a forced cap");
        let census = |query: &SkyQuery| {
            let mut ctx = SkyContext {
                tables: milky_way_dark_tables(),
                envelope: milky_way_envelope(),
                offsets: milky_way_offsets(),
                noise: NoiseCache::with_capacity(1 << 16),
                cells: &NoSkyCellCache,
                sources: &[],
                modifiers: &NoModifiers,
            };
            let plan = census_plan(galaxy, ctx.tables, ctx.envelope, query, &mut ctx.noise);
            let mut stars = Vec::new();
            for key in plan.cells() {
                census_cell(galaxy, &mut ctx, key, query, &mut stars);
            }
            let walks = plan.walks;
            (stars, walks)
        };
        let key = |s: &SkyStar| (s.system(), s.star());
        let (theirs, _) = census(&full);
        let (ours, walks) = census(&by_ray);
        let all: BTreeMap<_, &SkyStar> = theirs.iter().map(|s| (key(s), s)).collect();
        let listed: BTreeMap<_, &SkyStar> = ours.iter().map(|s| (key(s), s)).collect();
        let caps = by_ray.forced_caps.as_ref().expect("forced caps");
        let cap_of = |layer: Layer| caps.iter().find(|c| c.layer() == layer).expect("capped");
        let origin = observer().position().to_light_years_f64();
        let direction = |s: &SkyStar| {
            let at = s.apparent().to_light_years_f64();
            UnitVector::from_components(std::array::from_fn(|k| at[k] - origin[k]))
                .expect("a star away from its observer")
        };
        let mut within = 0_u32;
        for (k, star) in &all {
            if star.distance() < cap_of(star.layer()).radius_toward(direction(star)) {
                within += 1;
                assert!(
                    listed.contains_key(k),
                    "{star:?} lies within its radius, unlisted"
                );
            }
        }
        let mut beyond = 0_u32;
        for (k, star) in &listed {
            let theirs = all.get(k).expect("a star of the full census");
            assert_eq!(
                (bits(star.v().value()), bits(star.distance().value())),
                (bits(theirs.v().value()), bits(theirs.distance().value()))
            );
            let cap = cap_of(star.layer());
            let walk = walks
                .iter()
                .find(|w| w.layer == star.layer())
                .expect("a walk of the star's layer");
            let rays = cap.rays().expect("one radius a ray");
            let size = f64::from(star.layer().cell_size_ly());
            let overshoot = 3.0_f64.sqrt() * size + 2.0 * walk.pad.value() + 10.0;
            let d = star.distance().value();
            let u = direction(star);
            if d >= cap.radius_toward(u).value() {
                beyond += 1;
            }
            let opening = rays.lattice().spacing().value() + math::asin((overshoot / d).min(1.0));
            let reached =
                rays.lattice()
                    .directions()
                    .iter()
                    .zip(rays.radii_ly())
                    .any(|(ray, &r)| {
                        r > d - overshoot && math::acos(ray.dot(&u).clamp(-1.0, 1.0)) <= opening
                    });
            assert!(
                reached,
                "{star:?} lies beyond its rays' radii and the cells' overshoot"
            );
        }
        eprintln!(
            "by ray: {} listed of the full census's {}; {within} of those within their radii, all \
             listed; {beyond} listed beyond their radii, within the cells' overshoot",
            listed.len(),
            all.len()
        );
        assert!(
            within > 200 && listed.len() < all.len(),
            "{within}, {}",
            listed.len()
        );
    }

    /// Caps forced near the Sun as the caps put them at the eye's cut (R06.T7's spheres), and
    /// layer by layer the shells' edges below them.
    fn caps_near_the_sun() -> Vec<LayerCap> {
        [11.0, 68.0, 8_193.0, 9_925.0, 21_369.0, 1.0]
            .iter()
            .zip(CAPPED_LAYERS)
            .map(|(&radius, layer)| LayerCap::forced(layer, LightYears::new(radius)))
            .collect()
    }

    /// The shells take the fixed edges below each layer's farthest radius, then the cap: near the
    /// Sun C and D have shells to 125, 250, 500, 1,000, 2,000, 4,000 and 8,000 ly, and E to
    /// 16,000 ly too; A, B and the brown dwarfs one each. They come nearest first across the
    /// layers, which is their order as values, and each layer's last is final (R06.T8.i, at
    /// R06.T11.g's edges).
    #[test]
    fn the_shells_take_the_fixed_edges_below_each_cap() {
        let query = SkyQuery::builder(observer(), Magnitudes::new(7.95))
            .build()
            .expect("a valid query");
        let plan = census_plan_of(&query, caps_near_the_sun());
        let shells: Vec<Shell> = plan.shells().collect();
        let edges = |layer: Layer| -> Vec<Option<LightYears>> {
            shells
                .iter()
                .filter(|shell| shell.layer() == layer)
                .map(Shell::edge)
                .collect()
        };
        let with_last = |n: usize| -> Vec<Option<LightYears>> {
            SHELL_EDGES_LY[..n]
                .iter()
                .map(|&edge| Some(LightYears::new(f64::from(edge))))
                .chain([None])
                .collect()
        };
        assert_eq!(edges(Layer::A), [None]);
        assert_eq!(edges(Layer::B), [None]);
        assert_eq!(edges(Layer::BrownDwarf), [None]);
        assert_eq!(edges(Layer::C), with_last(7));
        assert_eq!(edges(Layer::D), with_last(7));
        assert_eq!(edges(Layer::E), with_last(8));
        assert_eq!(
            SHELL_EDGES_LY[..3],
            [125, 250, 500],
            "the first reply is complete to 125 ly (R06.T11.g)"
        );
        for shell in &shells {
            let mine: Vec<&Shell> = shells
                .iter()
                .filter(|s| s.layer() == shell.layer())
                .collect();
            assert_eq!(
                usize::from(shell.index()),
                mine.iter().position(|s| *s == shell).expect("its own")
            );
            assert_eq!(
                shell.is_last(),
                usize::from(shell.index()) + 1 == mine.len()
            );
        }
        // Nearest first: every layer's first shell, then every second, and so on.
        let mut sorted = shells.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, shells);
        assert_eq!(
            shells.iter().take(6).map(Shell::layer).collect::<Vec<_>>(),
            CAPPED_LAYERS
        );
        // A cap on an edge takes no shell to it, and a cap of no radius has none.
        let mut caps = caps_near_the_sun();
        caps[2] = LayerCap::forced(Layer::C, LightYears::new(1_000.0));
        caps[3] = LayerCap::forced(Layer::D, LightYears::ZERO);
        let plan = census_plan_of(&query, caps);
        let c: Vec<Option<LightYears>> = plan
            .shells()
            .filter(|s| s.layer() == Layer::C)
            .map(|s| s.edge())
            .collect();
        assert_eq!(
            c,
            [125.0, 250.0, 500.0]
                .map(|edge| Some(LightYears::new(edge)))
                .into_iter()
                .chain([None])
                .collect::<Vec<_>>()
        );
        assert_eq!(plan.shells().filter(|s| s.layer() == Layer::D).count(), 0);
        let done = plan.completeness(std::iter::empty());
        assert_eq!(done.edge(Layer::D), None, "a layer of no radius is final");
        assert_eq!(done.edge(Layer::C), Some(LightYears::ZERO));
        assert_eq!(done.complete_to().radius(Layer::C), LightYears::ZERO);
    }

    /// A census of some shells is complete to each layer's edge reached, held within its cap, and
    /// final in a layer once its last shell is done; a gap in a layer's run holds it at the shell
    /// before the gap. The plan's replies hold every layer's every edge, rank by rank, and the last
    /// is complete to the caps (R06.T8.i).
    #[test]
    fn a_census_of_some_shells_is_complete_to_the_edges_reached() {
        let query = SkyQuery::builder(observer(), Magnitudes::new(7.95))
            .build()
            .expect("a valid query");
        let plan = census_plan_of(&query, caps_near_the_sun());
        let shells: Vec<Shell> = plan.shells().collect();
        let of = |layer: Layer, index: u8| {
            *shells
                .iter()
                .find(|s| s.layer() == layer && s.index() == index)
                .expect("a shell")
        };
        // C's first three (to 500 ly), D's first and third, E's last alone, and all of A's, B's
        // and the brown dwarfs'.
        let done = [
            of(Layer::A, 0),
            of(Layer::B, 0),
            of(Layer::BrownDwarf, 0),
            of(Layer::C, 2),
            of(Layer::C, 0),
            of(Layer::C, 1),
            of(Layer::D, 0),
            of(Layer::D, 2),
            of(Layer::E, 8),
        ];
        let reached = plan.completeness(done);
        let ly = |r: f64| Some(LightYears::new(r));
        assert_eq!(reached.edge(Layer::C), ly(500.0));
        assert_eq!(reached.edge(Layer::D), ly(125.0));
        assert_eq!(reached.edge(Layer::E), ly(0.0));
        for layer in [Layer::A, Layer::B, Layer::BrownDwarf] {
            assert_eq!(reached.edge(layer), None, "{layer:?}");
        }
        assert_eq!(reached.least_edge(), ly(0.0));
        assert!(!reached.is_final());
        let radii = reached.complete_to();
        for (layer, radius) in [
            (Layer::A, 11.0),
            (Layer::B, 68.0),
            (Layer::C, 500.0),
            (Layer::D, 125.0),
            (Layer::E, 0.0),
            (Layer::BrownDwarf, 1.0),
        ] {
            assert_eq!(bits(radii.radius(layer).value()), bits(radius), "{layer:?}");
        }
        let replies = plan.replies();
        assert_eq!(replies.len(), 9, "E's eight edges and its cap");
        for (rank, reply) in replies.iter().enumerate() {
            for layer in SHELLED_LAYERS {
                let edges: Vec<Shell> = shells
                    .iter()
                    .copied()
                    .filter(|s| s.layer() == layer)
                    .collect();
                let at = edges[rank.min(edges.len() - 1)];
                let expected = at.edge().unwrap_or_else(|| {
                    caps_near_the_sun()
                        .iter()
                        .find(|c| c.layer() == layer)
                        .expect("a cap")
                        .radius()
                });
                assert_eq!(
                    bits(reply.radius(layer).value()),
                    bits(expected.value()),
                    "{layer:?} at {rank}"
                );
            }
        }
        assert_eq!(replies.last(), Some(&CompleteTo::of_caps(plan.caps())));
        assert_eq!(
            plan.complete().complete_to(),
            &CompleteTo::of_caps(plan.caps())
        );
        assert!(plan.complete().is_final());
        assert_eq!(plan.complete().least_edge(), None);
    }

    /// For a cap of one radius a ray, a layer's census of its shells to an edge is complete towards
    /// each direction to the lesser of the edge and the cap's radius there, bit for bit, and to the
    /// edge alone in every direction where every ray reaches past it (R06.T8.i on R06.T7.b).
    #[test]
    fn by_ray_a_shells_radius_is_held_within_each_rays_cap() {
        let query = query_by_ray(7.0, 60.0, 400.0);
        let caps = query.forced_caps.clone().expect("forced caps");
        let plan = plan_with_edges(&query, caps.clone(), &[50, 100, 200, 300]);
        let shells: Vec<Shell> = plan.shells().collect();
        let replies = plan.replies();
        assert_eq!(
            replies.len(),
            5,
            "four edges below every shelled layer's farthest ray"
        );
        for (rank, reply) in replies.iter().enumerate() {
            let mut uniforms =
                crate::sky::testing::uniforms(0x7b_0900 + u64::try_from(rank).expect("few"));
            for _ in 0..2_000 {
                let mut next = || uniforms.next().expect("endless");
                let u = UnitVector::from_components(std::array::from_fn(|_| 2.0 * next() - 1.0))
                    .expect("a direction");
                for cap in &caps {
                    let towards = cap.radius_toward(u).value();
                    let mine: Vec<&Shell> =
                        shells.iter().filter(|s| s.layer() == cap.layer()).collect();
                    let expected = mine[rank.min(mine.len() - 1)]
                        .edge()
                        .map_or(towards, |edge| towards.min(edge.value()));
                    assert_eq!(
                        bits(reply.radius_toward(cap.layer(), u).value()),
                        bits(expected),
                        "{:?} at rank {rank}",
                        cap.layer()
                    );
                }
            }
        }
        // Every ray reaches past 50 ly, so the first reply is complete to it in every direction;
        // the last is complete to the caps.
        for layer in SHELLED_LAYERS {
            assert_eq!(replies[0].radius(layer), LightYears::new(50.0), "{layer:?}");
        }
        assert_eq!(replies.last(), Some(&CompleteTo::of_caps(&caps)));
    }

    /// The cells of the plan's shells in order are its cells, each once, and they are the cells of
    /// each layer's walk that the plan opens; each lies in the first shell whose sphere, the edge
    /// padded as a walk to it pads, its box meets, and in its layer's last if none; and each slab
    /// names its shell (R06.T8.i). Checked at the fixed edges with caps of one radius past them, at
    /// a test's edges with caps of one radius a ray, and in a cone.
    #[test]
    fn every_cell_is_in_exactly_one_shell() {
        let query = SkyQuery::builder(observer(), Magnitudes::new(7.95))
            .build()
            .expect("a valid query");
        let uniform: Vec<LayerCap> = [30.0, 60.0, 1_100.0, 2_100.0, 1_100.0, 40.0]
            .iter()
            .zip(CAPPED_LAYERS)
            .map(|(&radius, layer)| LayerCap::forced(layer, LightYears::new(radius)))
            .collect();
        let by_ray = query_by_ray(7.0, 60.0, 400.0);
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(10.0)).expect("a cone");
        let coned = SkyQuery::builder(observer(), Magnitudes::new(7.0))
            .cone(cone)
            .build()
            .expect("a valid query");
        let forced = |radius: f64| -> Vec<LayerCap> {
            CAPPED_LAYERS
                .iter()
                .map(|&layer| LayerCap::forced(layer, LightYears::new(radius)))
                .collect()
        };
        let cases: [(&SkyQuery, Vec<LayerCap>, &[u32]); 3] = [
            (&query, uniform, &SHELL_EDGES_LY),
            (
                &by_ray,
                by_ray.forced_caps.clone().expect("forced caps"),
                &[50, 100, 200, 300],
            ),
            (&coned, forced(400.0), &[100, 200]),
        ];
        for (query, caps, edges) in cases {
            let plan = plan_with_edges(query, caps, edges);
            let mut by_shell = Vec::new();
            for shell in plan.shells() {
                for slab in plan.shell_slabs(shell) {
                    assert_eq!(slab.shell(), shell);
                    assert_eq!(slab.layer(), shell.layer());
                    by_shell.extend(slab.cells().map(|key| (key, shell)));
                }
            }
            let cells: Vec<CellKey> = by_shell.iter().map(|&(key, _)| key).collect();
            assert_eq!(
                plan.cells().collect::<Vec<_>>(),
                cells,
                "the shells in order"
            );
            assert_eq!(plan.cell_count(), u64::try_from(cells.len()).expect("few"));
            let once: BTreeSet<CellKey> = cells.iter().copied().collect();
            assert_eq!(once.len(), cells.len(), "no cell is in two shells");
            // The plan's cells as they were before the shells: each layer's walk, opened.
            let walked: BTreeSet<CellKey> = plan
                .walks
                .iter()
                .flat_map(|walk| {
                    crate::galaxy::query::cells_in_sphere(walk.layer, &walk.sphere).filter(|&key| {
                        opens(
                            query.cone_region(),
                            walk.rays.as_ref(),
                            plan.apex,
                            key,
                            walk.pad,
                        )
                    })
                })
                .collect();
            assert_eq!(once, walked);
            for walk in &plan.walks {
                let met: Vec<BTreeSet<CellKey>> = walk
                    .edges
                    .iter()
                    .map(|(_, sphere)| {
                        crate::galaxy::query::cells_in_sphere(walk.layer, sphere).collect()
                    })
                    .collect();
                for &(key, shell) in by_shell.iter().filter(|(key, _)| key.layer() == walk.layer) {
                    let first = met
                        .iter()
                        .position(|cells| cells.contains(&key))
                        .unwrap_or(met.len());
                    assert_eq!(usize::from(shell.index()), first, "{key:?} in {shell:?}");
                }
                if walk.layer == Layer::C || walk.layer == Layer::D {
                    assert!(walk.edges.len() >= 2, "{:?} has shells", walk.layer);
                }
            }
            eprintln!("{} cells in {} shells", cells.len(), plan.shells().count());
        }
    }

    /// A cell's census: its stars and tallies.
    type Part = (Vec<SkyStar>, crate::sky::census::CensusTallies);

    /// The census of each of `cells` for `query`, on up to four threads (one on WebAssembly, which
    /// has none), each with a context of its own, keyed by cell: a cell's census is a function of
    /// the cell and the query alone, so the threads' timing changes no bit.
    fn census_by_cell(query: &SkyQuery, cells: &[CellKey]) -> BTreeMap<CellKey, Part> {
        by_cell(query, cells, &|ctx, key, query, out| {
            census_cell(milky_way_galaxy(), ctx, key, query, out)
        })
    }

    /// The census's oracle for each of `cells`, as [`census_by_cell`] runs the census: every
    /// record of each cell generated whole and measured for `query` with no skip, as the
    /// integration tests' `brute_force_sky` measures them (R06.T8.e).
    fn oracle_by_cell(query: &SkyQuery, cells: &[CellKey]) -> BTreeMap<CellKey, Part> {
        by_cell(query, cells, &|ctx, key, query, out| {
            let galaxy = milky_way_galaxy();
            let mut records = Vec::new();
            crate::galaxy::placement::generate_cell(galaxy, key, &mut records);
            let mut tallies = crate::sky::census::CensusTallies::default();
            for record in &records {
                crate::sky::census::census_record(
                    galaxy,
                    ctx,
                    record,
                    query,
                    crate::sky::census::Bound::Ignored,
                    &mut tallies,
                    out,
                );
            }
            tallies
        })
    }

    /// What one cell's job measures: `key`'s stars for the query into `out`, and its tallies.
    type CellMeasure = dyn Fn(
            &mut SkyContext<'_>,
            CellKey,
            &SkyQuery,
            &mut Vec<SkyStar>,
        ) -> crate::sky::census::CensusTallies
        + Sync;

    /// `each` of `cells` for `query`, on up to four threads (one on WebAssembly), each with a
    /// context of its own, keyed by cell.
    fn by_cell(query: &SkyQuery, cells: &[CellKey], each: &CellMeasure) -> BTreeMap<CellKey, Part> {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let job = || {
            let mut ctx = SkyContext {
                tables: milky_way_dark_tables(),
                envelope: milky_way_envelope(),
                offsets: milky_way_offsets(),
                noise: NoiseCache::with_capacity(1 << 16),
                cells: &NoSkyCellCache,
                sources: &[],
                modifiers: &NoModifiers,
            };
            let mut parts = Vec::new();
            loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&key) = cells.get(k) else {
                    return parts;
                };
                let mut stars = Vec::new();
                let tallies = each(&mut ctx, key, query, &mut stars);
                parts.push((key, (stars, tallies)));
            }
        };
        let parts: Vec<(CellKey, Part)> = if cfg!(target_family = "wasm") {
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
        parts.into_iter().collect()
    }

    /// Every float of `stars` that a census measures, with its system and star, in order:
    /// `PartialEq` holds 0.0 and −0.0 equal.
    fn star_bits(stars: &[SkyStar]) -> Vec<(SystemId, u64, [u64; 6])> {
        stars
            .iter()
            .map(|s| {
                let [x, y, z] = s.apparent().offset_metres();
                (
                    s.system(),
                    u64::from(s.star().get()),
                    [
                        s.v().value(),
                        s.a_v().value(),
                        s.distance().value(),
                        x,
                        y,
                        z,
                    ]
                    .map(bits),
                )
            })
            .collect()
    }

    /// The shells' edges of the census tests, ly: R06.T8.i's 1,000 × 2<sup>k</sup> ly scaled down
    /// twelvefold and more, so that a unit test can afford a census to its last.
    const TEST_EDGES_LY: [u32; 2] = [40, 80];

    /// The census tests' plan near the Sun, its shells and every cell's census.
    struct ShellCensus {
        query: SkyQuery,
        plan: CensusPlan,
        shells: Vec<Shell>,
        parts: BTreeMap<CellKey, Part>,
    }

    impl ShellCensus {
        /// Near the Sun to V 8, every cap forced to 160 ly, and C, D and E in shells to
        /// [`TEST_EDGES_LY`].
        fn near_the_sun() -> Self {
            let query =
                SkyQuery::builder(crate::sky::testing::sun_observer(), Magnitudes::new(8.0))
                    .build()
                    .expect("a valid query")
                    .with_caps_forced(LightYears::new(160.0))
                    .expect("a forced cap");
            let caps = query.forced_caps.clone().expect("forced caps");
            let plan = plan_with_edges(&query, caps, &TEST_EDGES_LY);
            let cells: Vec<CellKey> = plan.cells().collect();
            let parts = census_by_cell(&query, &cells);
            let shells = plan.shells().collect();
            Self {
                query,
                plan,
                shells,
                parts,
            }
        }

        /// The parts of the cells of the shells `done`.
        fn parts_of(&self, done: &[Shell]) -> Vec<Part> {
            done.iter()
                .flat_map(|&shell| self.plan.shell_slabs(shell))
                .flat_map(|slab| slab.cells())
                .map(|key| self.parts[&key].clone())
                .collect()
        }
    }

    /// The listed stars and then the overflow of `census`.
    fn whole(census: &SkyCensus) -> Vec<SkyStar> {
        let mut stars = census.listed().to_vec();
        stars.extend_from_slice(census.overflow());
        stars
    }

    /// Whether `star`'s layer is censused shell by shell.
    fn shelled(star: &SkyStar) -> bool {
        SHELLED_LAYERS.contains(&star.layer())
    }

    /// Checks the census of the shells of `fixture` to its k-th edge against the census with C's,
    /// D's and E's caps forced to that edge and against the `last` census, of every shell, and
    /// returns how many stars of its shells it holds back.
    fn check_the_census_to_an_edge(fixture: &ShellCensus, k: usize, last: &SkyCensus) -> usize {
        let unbounded = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let edge = f64::from(TEST_EDGES_LY[k]);
        let done: Vec<Shell> = fixture
            .shells
            .iter()
            .copied()
            .filter(|s| usize::from(s.index()) <= k)
            .collect();
        let reached = fixture.plan.completeness(done.iter().copied());
        for layer in SHELLED_LAYERS {
            assert_eq!(
                reached.edge(layer),
                Some(LightYears::new(edge)),
                "{layer:?}"
            );
        }
        let census = merge_shells(fixture.parts_of(&done), unbounded, reached.clone());
        assert_eq!(census.completeness(), Some(&reached));
        assert!(census.overflow().is_empty());
        // The census with C's, D's and E's caps forced to the edge, less its stars beyond it.
        let forced = fixture
            .query
            .clone()
            .with_caps_forced_per_layer(&SHELLED_LAYERS.map(|layer| (layer, LightYears::new(edge))))
            .expect("a forced cap");
        let caps = forced.forced_caps().expect("forced caps").to_vec();
        let theirs_cells: Vec<CellKey> = census_plan_of(&forced, caps).cells().collect();
        let theirs = merge_census(
            census_by_cell(&forced, &theirs_cells).into_values(),
            unbounded,
        );
        let within: Vec<SkyStar> = theirs
            .listed()
            .iter()
            .filter(|s| s.distance().value() < edge)
            .copied()
            .collect();
        let ours: Vec<SkyStar> = census
            .listed()
            .iter()
            .filter(|s| shelled(s))
            .copied()
            .collect();
        assert_eq!(
            star_bits(&ours),
            star_bits(&within),
            "C, D and E to {edge} ly"
        );
        assert_eq!(ours, within);
        // A's, B's and the brown dwarfs' one shell is done: they are listed whole.
        let others: Vec<SkyStar> = census
            .listed()
            .iter()
            .filter(|s| !shelled(s))
            .copied()
            .collect();
        let finals: Vec<SkyStar> = last
            .listed()
            .iter()
            .filter(|s| !shelled(s))
            .copied()
            .collect();
        assert_eq!(
            star_bits(&others),
            star_bits(&finals),
            "A, B and the brown dwarfs"
        );
        let waiting = check_the_stars_beyond_wait(fixture.parts_of(&done), edge, &census, last);
        // No star of the partial census lies beyond its layer's radius, at any n_max.
        let few = NonZeroU32::new(60).expect("not zero");
        let cut = merge_shells(fixture.parts_of(&done), few, reached.clone());
        assert_eq!(
            whole(&cut),
            census.listed(),
            "the brightest 60 listed, the rest past n_max"
        );
        for star in whole(&cut).iter().filter(|s| shelled(s)) {
            let radius = reached.radius_for(star).value();
            assert!(
                star.distance().value() < radius,
                "{star:?} beyond {radius} ly"
            );
            assert_eq!(bits(radius), bits(edge));
        }
        eprintln!(
            "to {edge} ly: C, D and E list {} stars, and {} of their shells' stars wait",
            ours.len(),
            waiting
        );
        waiting
    }

    /// Checks that the stars of `parts`, a census of shells to `edge`, that lie at or beyond the
    /// edge are not listed by its `census` and are by the `last`, and returns how many there are.
    fn check_the_stars_beyond_wait(
        parts: Vec<Part>,
        edge: f64,
        census: &SkyCensus,
        last: &SkyCensus,
    ) -> usize {
        let kept: Vec<SkyStar> = parts.into_iter().flat_map(|(stars, _)| stars).collect();
        let waiting: Vec<&SkyStar> = kept
            .iter()
            .filter(|s| shelled(s) && s.distance().value() >= edge)
            .collect();
        assert!(
            !waiting.is_empty(),
            "a straddling cell holds stars beyond {edge} ly"
        );
        for &star in &waiting {
            assert!(!census.listed().contains(star));
            assert!(
                last.listed().contains(star),
                "{star:?} is listed by the last census"
            );
        }
        waiting.len()
    }

    /// Near the Sun to V 8, every cap forced to 160 ly and C, D and E in shells to 40 and 80 ly
    /// (R06.T8.i's tests):
    ///
    /// - shells 1–k merged, at any `n_max`, are the census with C's, D's and E's caps forced to
    ///   shell k's edge, less that census's stars beyond the edge, bit for bit, and A's, B's and
    ///   the brown dwarfs' are their whole census, their one shell done;
    /// - no star of a partial census, listed or past `n_max`, lies beyond its layer's complete-to
    ///   radius, and the stars a straddling cell holds beyond it are listed by a later census;
    /// - the last, every shell merged, is [`census_plan`]'s census, star for star.
    #[test]
    fn shells_merged_are_the_census_to_each_edge_and_the_last_is_the_plans() {
        let fixture = ShellCensus::near_the_sun();
        let unbounded = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let last = merge_shells(
            fixture.parts_of(&fixture.shells),
            unbounded,
            fixture.plan.complete(),
        );
        let held_back: usize = (0..TEST_EDGES_LY.len())
            .map(|k| check_the_census_to_an_edge(&fixture, k, &last))
            .sum();
        assert!(held_back > 0);
        // The last: every shell, the one-shot census of the plan's cells, star for star.
        let one_shot = census_plan(
            milky_way_galaxy(),
            milky_way_dark_tables(),
            milky_way_envelope(),
            &fixture.query,
            &mut NoiseCache::with_capacity(16),
        );
        let few = NonZeroU32::new(60).expect("not zero");
        let one_shot = merge_census(one_shot.cells().map(|key| fixture.parts[&key].clone()), few);
        let final_few = merge_shells(
            fixture.parts_of(&fixture.shells),
            few,
            fixture.plan.complete(),
        );
        assert_eq!(final_few.listed(), one_shot.listed());
        assert_eq!(final_few.overflow(), one_shot.overflow());
        assert_eq!(final_few.tallies(), one_shot.tallies());
        assert_eq!(star_bits(&whole(&final_few)), star_bits(&whole(&one_shot)));
        assert!(final_few.completeness().is_some_and(Completeness::is_final));
        assert_eq!(one_shot.completeness(), None);
        eprintln!(
            "the last lists {} and passes {} past n_max, of {}; {} cells in {} shells",
            final_few.listed().len(),
            final_few.overflow().len(),
            last.listed().len(),
            fixture.parts.len(),
            fixture.shells.len()
        );
    }

    /// C's, D's and E's `caps` held within `edge`, ly, each ray's radius the lesser of its own and
    /// the edge, or the edge in every direction where every ray reaches it; A's, B's and the brown
    /// dwarfs' forced to nothing.
    fn caps_within(caps: &[LayerCap], edge: f64) -> Vec<LayerCap> {
        caps.iter()
            .map(
                |cap| match (SHELLED_LAYERS.contains(&cap.layer()), cap.rays()) {
                    (false, _) => LayerCap::forced(cap.layer(), LightYears::ZERO),
                    (true, Some(rays)) if !rays.all_reach(edge) => {
                        LayerCap::forced_by_ray(cap.layer(), rays.within(edge))
                    }
                    (true, _) => LayerCap::forced(cap.layer(), LightYears::new(edge)),
                },
            )
            .collect()
    }

    /// A star's direction from the observer of the tests' queries by ray.
    fn direction_of(star: &SkyStar) -> UnitVector {
        UnitVector::from_components(
            observer()
                .position()
                .displacement_to(star.apparent())
                .metres(),
        )
        .expect("a star away from its observer")
    }

    /// Near the Sun to V 8 with every layer's cap one radius a ray, jagged between 40 and 150 ly,
    /// and C, D and E in shells to 50 and 100 ly (R06.T8.i's tests, as ruled for caps by ray):
    ///
    /// - shells 1–k merged hold, bit for bit, every star of the census with caps forced to shell
    ///   k's radii (each ray's held within edge k) that lies within its layer's radius towards its
    ///   band texel;
    /// - any other star they list lies within that radius towards its texel and at or beyond it
    ///   towards its own direction: one of R06.T7.b's gap, whose cell the one-shot plan's padded
    ///   ball opens and the forced census's smaller one does not;
    /// - in a layer not yet final no listed star lies at or beyond its radius towards its texel,
    ///   which is the radius the band's ray through that texel reads.
    #[test]
    fn by_ray_shells_merged_hold_the_census_to_each_shells_radii() {
        let query = query_by_ray(8.0, 40.0, 150.0);
        let caps = query.forced_caps.clone().expect("forced caps");
        let edges = [50, 100];
        let plan = plan_with_edges(&query, caps.clone(), &edges);
        let cells: Vec<CellKey> = plan.cells().collect();
        let parts = census_by_cell(&query, &cells);
        let shells: Vec<Shell> = plan.shells().collect();
        let unbounded = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let spec = query.band_spec();
        for (k, &edge) in edges.iter().enumerate() {
            let edge = f64::from(edge);
            let done: Vec<Shell> = shells
                .iter()
                .copied()
                .filter(|s| usize::from(s.index()) <= k)
                .collect();
            let reached = plan.completeness(done.iter().copied());
            let mine: Vec<Part> = done
                .iter()
                .flat_map(|&shell| plan.shell_slabs(shell))
                .flat_map(|slab| slab.cells())
                .map(|key| parts[&key].clone())
                .collect();
            let census = merge_shells(mine, unbounded, reached.clone());
            let ours: BTreeMap<(SystemId, u8), &SkyStar> = census
                .listed()
                .iter()
                .filter(|s| shelled(s))
                .map(|s| ((s.system(), s.star().get()), s))
                .collect();
            // The census with C's, D's and E's caps forced to shell k's radii, A's, B's and the
            // brown dwarfs' to nothing.
            let mut forced = query.clone();
            let forced_caps = caps_within(&caps, edge);
            forced.forced_caps = Some(forced_caps.clone());
            let theirs_cells: Vec<CellKey> = census_plan_of(&forced, forced_caps).cells().collect();
            let theirs = merge_census(
                census_by_cell(&forced, &theirs_cells).into_values(),
                unbounded,
            );
            let mut within = 0_u32;
            for star in theirs.listed() {
                if star.distance() < reached.radius_for(star) {
                    within += 1;
                    let mine = ours
                        .get(&(star.system(), star.star().get()))
                        .unwrap_or_else(|| {
                            panic!("{star:?} lies within its texel's radius, unlisted")
                        });
                    assert_eq!(star_bits(&[**mine]), star_bits(&[*star]));
                }
            }
            let mut gap = 0_u32;
            for star in ours.values() {
                let radius = reached.radius_for(star);
                assert!(
                    star.distance() < radius,
                    "{star:?} lies beyond its texel's radius"
                );
                let d = observer()
                    .position()
                    .displacement_to(star.apparent())
                    .metres();
                let (face, row, column) = spec.texel_of(d).expect("a star away from its observer");
                let band = reached
                    .complete_to()
                    .radius_toward(star.layer(), spec.texel_direction(face, row, column));
                assert_eq!(
                    bits(radius.value()),
                    bits(band.value()),
                    "the band's radius"
                );
                let theirs_too = theirs
                    .listed()
                    .iter()
                    .any(|s| (s.system(), s.star()) == (star.system(), star.star()));
                if !theirs_too {
                    gap += 1;
                    let own = reached
                        .complete_to()
                        .radius_toward(star.layer(), direction_of(star));
                    assert!(
                        star.distance() >= own,
                        "{star:?}: within its own direction's radius"
                    );
                }
            }
            assert!(within > 50, "{within} stars within shell {k}'s radii");
            eprintln!(
                "by ray to {edge} ly: C, D and E list {} stars, {within} of the forced census's, and \
                 {gap} of the gap",
                ours.len()
            );
        }
    }

    /// The ceiling of the tests at a synthetic ceiling, V: RM3's interim's (rendering plan R13,
    /// Design note 15).
    const TEST_CEILING_V: f64 = 5.0;

    /// [`query_by_ray`] at the synthetic ceiling [`TEST_CEILING_V`], with C's, D's and E's rays held
    /// within `limit_ly`, as [`real_boundary`](super::super::caps::real_boundary) holds them within
    /// the real limit: a real boundary R(u) scaled down so that a unit test can afford its census,
    /// uniform at the limit on some rays and varying on the rest.
    fn query_at_ceiling(cut: f64, lo: f64, hi: f64, limit_ly: f64) -> SkyQuery {
        let mut query = SkyQuery::builder(observer(), Magnitudes::new(cut))
            .synthetic_ceiling(Magnitudes::new(TEST_CEILING_V))
            .build()
            .expect("a valid query");
        query.forced_caps = Some(
            CAPPED_LAYERS
                .iter()
                .zip(0_u64..)
                .map(|(&layer, k)| {
                    let rays = jagged_radii(0x7b_0100 + k, lo, hi);
                    let rays = if REAL_BOUNDARY_LAYERS.contains(&layer) {
                        rays.within(limit_ly)
                    } else {
                        rays
                    };
                    LayerCap::forced_by_ray(layer, rays)
                })
                .collect(),
        );
        query
    }

    /// How many of `cap`'s rays lie exactly at `edge_ly`, bit for bit, and how many within it.
    fn rays_at_and_within(cap: &LayerCap, edge_ly: f64) -> (usize, usize) {
        let radii = cap.rays().expect("one radius a ray").radii_ly();
        let at = radii.iter().filter(|&&r| bits(r) == bits(edge_ly)).count();
        let within = radii.iter().filter(|&&r| r < edge_ly).count();
        (at, within)
    }

    /// A query at a synthetic ceiling is the same query's plan, shells and caps, but lists C, D
    /// and E by the radius towards each star's texel at every reply and opens their cells by cones
    /// widened by the band texel's largest radius (rendering plan R13, R13.T2.a). With C's, D's
    /// and E's rays jagged between 400 and 1,500 ly and held within 1,000 ly, its plan opens every
    /// cell the plan without the ceiling opens, and more of C's and D's, whose cells are narrow
    /// there; A's, B's and the brown dwarfs' walks are the same. With no ceiling the plan is
    /// R06's, and only a census not yet final lists by texel.
    #[test]
    fn a_plan_at_a_ceiling_widens_c_to_e_and_lists_them_by_texel_at_every_reply() {
        let at_ceiling = query_at_ceiling(8.0, 400.0, 1_500.0, 1_000.0);
        let caps = at_ceiling.forced_caps.clone().expect("forced caps");
        let mut without = at_ceiling.clone();
        without.synthetic_ceiling = None;
        let edges = [250, 500, 1_000];
        let (ours, theirs) = (
            plan_with_edges(&at_ceiling, caps.clone(), &edges),
            plan_with_edges(&without, caps.clone(), &edges),
        );
        assert_eq!(ours.caps(), theirs.caps());
        assert_eq!(
            ours.shells().collect::<Vec<_>>(),
            theirs.shells().collect::<Vec<_>>()
        );
        for (a, b) in ours.walks.iter().zip(&theirs.walks) {
            assert_eq!(a.layer, b.layer);
            assert_eq!((a.sphere, a.pad, &a.edges), (b.sphere, b.pad, &b.edges));
            let (cells_a, cells_b): (BTreeSet<CellKey>, BTreeSet<CellKey>) = (
                (0..a.shell_count())
                    .flat_map(|i| ours.slabs_of(a, i))
                    .flat_map(|slab| slab.cells())
                    .collect(),
                (0..b.shell_count())
                    .flat_map(|i| theirs.slabs_of(b, i))
                    .flat_map(|slab| slab.cells())
                    .collect(),
            );
            if REAL_BOUNDARY_LAYERS.contains(&a.layer) {
                assert_ne!(a.rays, b.rays, "{:?}'s cones are widened", a.layer);
                assert!(cells_b.is_subset(&cells_a), "{:?}", a.layer);
                if a.layer != Layer::E {
                    assert!(
                        cells_a.len() > cells_b.len(),
                        "{:?}: {} cells widened against {}",
                        a.layer,
                        cells_a.len(),
                        cells_b.len()
                    );
                }
                eprintln!(
                    "{:?}: {} cells with the cones widened, {} without",
                    a.layer,
                    cells_a.len(),
                    cells_b.len()
                );
            } else {
                assert_eq!(a, b, "{:?}'s walk", a.layer);
            }
        }
        let replies: Vec<Vec<Shell>> = (0..3)
            .map(|k| {
                ours.shells()
                    .filter(|shell| usize::from(shell.index()) <= k)
                    .collect()
            })
            .collect();
        for done in &replies {
            assert!(ours.completeness(done.iter().copied()).lists_by_texel());
        }
        assert!(ours.complete().is_final() && ours.complete().lists_by_texel());
        assert!(theirs.complete().is_final() && !theirs.complete().lists_by_texel());
        assert!(
            theirs
                .completeness(replies[0].iter().copied())
                .lists_by_texel()
        );
        assert_eq!(
            ours.complete().complete_to(),
            theirs.complete().complete_to(),
            "one boundary, the caps'"
        );
    }

    /// Asserts that no C, D or E star of `census`, listed or past `n_max`, lies at or beyond its
    /// texel's radius in `reached` or `limit_ly`.
    fn within_its_boundary(census: &SkyCensus, reached: &Completeness, limit_ly: f64) {
        for star in whole(census).iter().filter(|s| shelled(s)) {
            let radius = reached.radius_for(star);
            assert!(
                star.distance() < radius && star.distance().value() < limit_ly,
                "{star:?} at or beyond its texel's {} ly",
                radius.value()
            );
        }
    }

    /// Checks the census at a ceiling of `fixture`'s shells done, its shells to `edge_ly`, against
    /// the census with C's, D's and E's caps forced to those shells' radii under the same rule,
    /// and that none of its stars lies at or beyond its texel's radius or `limit_ly`.
    fn check_the_ceilings_census_to_an_edge(fixture: &ShellCensus, edge_ly: f64, limit_ly: f64) {
        let ShellCensus {
            query,
            plan,
            shells: done,
            parts,
        } = fixture;
        let unbounded = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let mine = |n_max| {
            let cells = done
                .iter()
                .flat_map(|&shell| plan.shell_slabs(shell))
                .flat_map(|slab| slab.cells());
            let reached = plan.completeness(done.iter().copied());
            merge_shells(cells.map(|key| parts[&key].clone()), n_max, reached)
        };
        let reached = plan.completeness(done.iter().copied());
        assert_eq!(reached.edge(Layer::C), Some(LightYears::new(edge_ly)));
        let census = mine(unbounded);
        within_its_boundary(&census, &reached, limit_ly);
        // The census with C's, D's and E's caps forced to the shells' radii, under the same rule.
        let caps = query.forced_caps.clone().expect("forced caps");
        let mut forced = query.clone();
        let forced_caps = caps_within(&caps, edge_ly);
        forced.forced_caps = Some(forced_caps.clone());
        let theirs_plan = census_plan_of(&forced, forced_caps);
        let theirs_cells: Vec<CellKey> = theirs_plan.cells().collect();
        let theirs = merge_shells(
            census_by_cell(&forced, &theirs_cells).into_values(),
            unbounded,
            theirs_plan.complete(),
        );
        let ours: Vec<SkyStar> = census
            .listed()
            .iter()
            .filter(|s| shelled(s))
            .copied()
            .collect();
        assert!(ours.len() > 50, "{} stars to {edge_ly} ly", ours.len());
        assert_eq!(
            star_bits(&ours),
            star_bits(theirs.listed()),
            "to {edge_ly} ly"
        );
        for star in &ours {
            assert_eq!(
                bits(reached.radius_for(star).value()),
                bits(theirs_plan.complete().radius_for(star).value()),
                "one boundary"
            );
        }
        // At any n_max: the brightest 60 listed, the rest past it, none beyond its radius.
        let cut = mine(NonZeroU32::new(60).expect("not zero"));
        assert_eq!(whole(&cut), census.listed());
        within_its_boundary(&cut, &reached, limit_ly);
        eprintln!(
            "at the ceiling to {edge_ly} ly: C, D and E list {} stars, as the forced census does",
            ours.len()
        );
    }

    /// A real boundary at the real limit ends C's, D's and E's last shell on it (rendering plan
    /// R13, R13.T2.a; R13.T1.b's determinism audit), at a ceiling near the Sun with
    /// [`SHELL_EDGES_LY`]: their rays lie between 1,000 and 2,500 ly, some exactly at 1,000 ly,
    /// held within `REAL_LIMIT_LY`, some exactly at it. The walk and the completeness decide at
    /// equality, so their shells are to 125, 250, 500 and 1,000 ly and then to R(u), with no shell
    /// of their own at 2,000 ly; done to 1,000 ly, which every ray reaches, each is complete to it
    /// in every direction; and at every reply, final or not, each is complete within the limit
    /// towards 10⁴ random directions, and to R(u)'s rays, bit for bit, once final. So too where
    /// every ray is held at the limit.
    #[test]
    fn a_real_boundary_at_the_limit_ends_its_last_shell_on_it() {
        use super::super::super::caps::REAL_LIMIT_LY;
        let lattice = Arc::new(CapLattice::new(CAP_RAYS));
        for (lo, hi) in [(800.0, 2_500.0), (2_100.0, 2_600.0)] {
            let mut query = SkyQuery::builder(observer(), Magnitudes::new(7.95))
                .synthetic_ceiling(Magnitudes::new(TEST_CEILING_V))
                .build()
                .expect("a query at the ceiling");
            let caps: Vec<LayerCap> = CAPPED_LAYERS
                .iter()
                .zip(0_u64..)
                .map(|(&layer, k)| {
                    if !REAL_BOUNDARY_LAYERS.contains(&layer) {
                        return LayerCap::forced(layer, LightYears::new(50.0));
                    }
                    let radii = crate::sky::testing::uniforms(0x7b_0500 + k)
                        .take(CAP_RAYS)
                        .map(|u| (lo + (hi - lo) * u).max(1_000.0))
                        .collect();
                    let rays = RayRadii::new(Arc::clone(&lattice), radii).within(REAL_LIMIT_LY);
                    LayerCap::forced_by_ray(layer, rays)
                })
                .collect();
            query.forced_caps = Some(caps.clone());
            let plan = census_plan_of(&query, caps.clone());
            let all_held = lo > REAL_LIMIT_LY;
            for layer in SHELLED_LAYERS {
                let cap = caps.iter().find(|c| c.layer() == layer).expect("capped");
                let (at, within) = rays_at_and_within(cap, REAL_LIMIT_LY);
                let (at_floor, _) = rays_at_and_within(cap, 1_000.0);
                assert!(
                    at > 0 && (all_held || (within > 0 && at_floor > 0)),
                    "{layer:?}, {lo}–{hi} ly: {at} at the limit, {within} within it, {at_floor} \
                     at 1,000 ly"
                );
                assert_eq!(all_held, within == 0, "{layer:?}");
                assert_eq!(bits(cap.radius().value()), bits(REAL_LIMIT_LY));
                let ends: Vec<Option<LightYears>> = plan
                    .shells()
                    .filter(|shell| shell.layer() == layer)
                    .map(|shell| shell.edge())
                    .collect();
                let expected: Vec<Option<LightYears>> = [125.0, 250.0, 500.0, 1_000.0]
                    .map(|edge| Some(LightYears::new(edge)))
                    .into_iter()
                    .chain([None])
                    .collect();
                assert_eq!(ends, expected, "{layer:?}");
            }
            let mut uniforms = crate::sky::testing::uniforms(0x7b_0600);
            let directions: Vec<UnitVector> = (0..10_000)
                .map(|_| {
                    let mut next = || uniforms.next().expect("endless");
                    UnitVector::from_components(std::array::from_fn(|_| 2.0 * next() - 1.0))
                        .expect("a direction")
                })
                .collect();
            for rank in 0..5_u8 {
                let done = plan.shells().filter(|shell| shell.index() <= rank);
                let reached = plan.completeness(done);
                assert!(reached.lists_by_texel());
                for layer in SHELLED_LAYERS {
                    let complete_to = reached.complete_to();
                    if rank == 3 {
                        assert_eq!(reached.edge(layer), Some(LightYears::new(1_000.0)));
                        assert_eq!(
                            complete_to.rays_ly(layer),
                            None,
                            "every ray reaches 1,000 ly"
                        );
                        assert_eq!(bits(complete_to.radius(layer).value()), bits(1_000.0));
                    }
                    if rank == 4 {
                        assert!(reached.is_final());
                        let cap = caps.iter().find(|c| c.layer() == layer).expect("capped");
                        let rays = cap.rays().expect("by ray").radii_ly();
                        assert_eq!(
                            complete_to
                                .rays_ly(layer)
                                .map(|r| r.iter().map(|&v| bits(v)).collect::<Vec<_>>()),
                            Some(rays.iter().map(|&v| bits(v)).collect()),
                            "{layer:?}"
                        );
                    }
                    for &u in &directions {
                        let radius = complete_to.radius_toward(layer, u).value();
                        assert!(
                            radius <= REAL_LIMIT_LY,
                            "{layer:?} towards {u:?}: {radius} ly"
                        );
                        if all_held && rank == 4 {
                            assert_eq!(bits(radius), bits(REAL_LIMIT_LY));
                        }
                    }
                }
            }
        }
    }

    /// Near the Sun to V 8 at a synthetic ceiling, C's, D's and E's rays jagged between 40 and
    /// 150 ly and held within a limit of 100 ly, the last of the shells' edges 40, 70 and 100 ly,
    /// as `REAL_LIMIT_LY` is one of [`SHELL_EDGES_LY`] (rendering plan R13, R13.T2.a):
    ///
    /// - the limit holds rays exactly at that edge, which is no shell's but the last's: C's, D's
    ///   and E's shells are to 40 and 70 ly, then to R(u), as the census walks them at equality;
    /// - shells 1–k merged, at any reply, list C's, D's and E's stars exactly as the census with
    ///   their caps forced to shell k's radii, under the same rule, bit for bit and in order;
    /// - the last, every shell merged, is the one-shot census of the plan's cells less its C, D and
    ///   E stars at or beyond their texel's radius, which straddling cells hold, at any `n_max`;
    /// - no listed C, D or E star, nor any past `n_max`, lies at or beyond its texel's R(u), nor at
    ///   or beyond the limit, in any reply.
    #[test]
    fn at_a_ceiling_shells_merged_are_the_census_forced_to_the_real_boundary() {
        let limit = 100.0;
        let query = query_at_ceiling(8.0, 40.0, 150.0, limit);
        let caps = query.forced_caps.clone().expect("forced caps");
        let edges = [40, 70, 100];
        let plan = plan_with_edges(&query, caps.clone(), &edges);
        let shells: Vec<Shell> = plan.shells().collect();
        for layer in SHELLED_LAYERS {
            let cap = caps.iter().find(|c| c.layer() == layer).expect("capped");
            let (at, within) = rays_at_and_within(cap, limit);
            assert!(
                at > 100 && within > 100,
                "{layer:?}: {at} at, {within} within"
            );
            assert_eq!(bits(cap.radius().value()), bits(limit), "{layer:?}");
            let ends: Vec<Option<LightYears>> = shells
                .iter()
                .filter(|shell| shell.layer() == layer)
                .map(Shell::edge)
                .collect();
            assert_eq!(
                ends,
                [
                    Some(LightYears::new(40.0)),
                    Some(LightYears::new(70.0)),
                    None
                ],
                "{layer:?}"
            );
        }
        let cells: Vec<CellKey> = plan.cells().collect();
        let parts = census_by_cell(&query, &cells);
        let parts_of = |done: &[Shell]| -> Vec<Part> {
            done.iter()
                .flat_map(|&shell| plan.shell_slabs(shell))
                .flat_map(|slab| slab.cells())
                .map(|key| parts[&key].clone())
                .collect()
        };
        let unbounded = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let within_its_boundary = |census: &SkyCensus, reached: &Completeness| {
            within_its_boundary(census, reached, limit);
        };
        for (k, &edge) in edges[..2].iter().enumerate() {
            let done: Vec<Shell> = shells
                .iter()
                .copied()
                .filter(|s| usize::from(s.index()) <= k)
                .collect();
            let fixture = ShellCensus {
                query: query.clone(),
                plan: plan.clone(),
                shells: done,
                parts: parts.clone(),
            };
            check_the_ceilings_census_to_an_edge(&fixture, f64::from(edge), limit);
        }
        // The last: the one-shot census less its C, D and E stars at or beyond their radius.
        let complete = plan.complete();
        assert!(complete.is_final());
        let last = merge_shells(parts_of(&shells), unbounded, complete.clone());
        within_its_boundary(&last, &complete);
        let one_shot = merge_census(cells.iter().map(|key| parts[key].clone()), unbounded);
        let (kept, held): (Vec<SkyStar>, Vec<SkyStar>) = one_shot
            .listed()
            .iter()
            .partition(|star| complete.lists(star));
        assert_eq!(star_bits(last.listed()), star_bits(&kept));
        assert!(
            !held.is_empty()
                && held
                    .iter()
                    .all(|s| shelled(s) && s.distance() >= complete.radius_for(s)),
            "the straddling cells' stars beyond R(u): {}",
            held.len()
        );
        let few = NonZeroU32::new(60).expect("not zero");
        let last_few = merge_shells(parts_of(&shells), few, complete.clone());
        assert_eq!(last_few.listed(), &kept[..60]);
        assert_eq!(last_few.overflow(), &kept[60..]);
        within_its_boundary(&last_few, &complete);
        eprintln!(
            "at the ceiling, the last lists {} stars of the one-shot census's {}, holding {} at or \
             beyond their texel's R(u)",
            last.listed().len(),
            one_shot.listed().len(),
            held.len()
        );
    }

    /// No gap at a synthetic ceiling (rendering plan R13, R13.T2.a; fix (i) of
    /// `decision-r06-t8i-listing.md`): near the Sun to V 11, every C, D and E star the oracle finds
    /// within 150 ly that lies within its texel's R(u) is listed by the final reply, bit for bit,
    /// over the limit's uniform rays and the varying ones alike. The oracle generates every system
    /// of every cell whole and measures it with no skip, as the integration tests'
    /// `brute_force_sky` does.
    ///
    /// R(u) is a real boundary scaled down: C's, D's and E's rays on one side of a tilted plane
    /// through the sky reach 150 ly, held at a limit of 140 ly, and the others 40–60 ly, varying
    /// from ray to ray. The query's band is of 2² texels a face, of up to 35° from their centres,
    /// scaled with the radii: R06.T7.b's gap needs a texel wider than a cell's ball, and at 140 ly a
    /// cell of C's 32 ly spans some 11° about its centre, where at the real limit's 2,000 ly it
    /// spans 0.8° against the 64² band's 1.27°. The same caps censused by R06's cones, unwidened,
    /// leave some of those stars in cells left closed, across the limit's edge: the gap, which the
    /// widening closes.
    #[test]
    #[cfg_attr(
        target_family = "wasm",
        ignore = "slow: on wasm32-wasip1, which has no threads, the oracle runs on one"
    )]
    fn at_a_ceiling_every_star_within_its_texels_real_boundary_is_listed() {
        let limit = 140.0;
        let mut query = query_at_ceiling(11.0, 50.0, 150.0, limit);
        query.band_spec =
            BandSpec::new(2, BandSpec::STANDARD.nodes_per_decade()).expect("a band of 2² a face");
        let lattice = Arc::new(CapLattice::new(CAP_RAYS));
        let edged: Vec<f64> = lattice
            .directions()
            .iter()
            .zip(crate::sky::testing::uniforms(0x7b_0400))
            .map(|(d, u)| {
                // The edge is tilted, so that it crosses the texels rather than run along their
                // edges, which lie in the axis planes.
                let [x, y, z] = d.components();
                if 0.8 * x + 0.5 * y + 0.33 * z > 0.3 {
                    150.0
                } else {
                    40.0 + 20.0 * u
                }
            })
            .collect();
        let real = RayRadii::new(lattice, edged).within(limit);
        for cap in query.forced_caps.as_mut().expect("forced caps") {
            if REAL_BOUNDARY_LAYERS.contains(&cap.layer()) {
                *cap = LayerCap::forced_by_ray(cap.layer(), real.clone());
            }
        }
        let caps = query.forced_caps.clone().expect("forced caps");
        let plan = census_plan_of(&query, caps.clone());
        let cells: Vec<CellKey> = plan.cells().collect();
        let unbounded = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let complete = plan.complete();
        let census = merge_shells(
            census_by_cell(&query, &cells).into_values(),
            unbounded,
            complete.clone(),
        );
        let listed: BTreeMap<(SystemId, u8), &SkyStar> = census
            .listed()
            .iter()
            .map(|s| ((s.system(), s.star().get()), s))
            .collect();
        // The same caps' census by R06's cones, unwidened, and its stars.
        let mut unwidened = query.clone();
        unwidened.synthetic_ceiling = None;
        let r06_cells: Vec<CellKey> = census_plan_of(&unwidened, caps).cells().collect();
        let r06: BTreeSet<(SystemId, u8)> = census_by_cell(&unwidened, &r06_cells)
            .into_values()
            .flat_map(|(stars, _)| stars)
            .map(|s| (s.system(), s.star().get()))
            .collect();
        // The oracle of C, D and E within 150 ly.
        let oracle = query
            .clone()
            .with_caps_forced_per_layer(&SHELLED_LAYERS.map(|l| (l, LightYears::new(150.0))))
            .expect("a forced cap");
        let oracle_caps = oracle.forced_caps.clone().expect("forced caps");
        let oracle_cells: Vec<CellKey> = census_plan_of(&oracle, oracle_caps).cells().collect();
        let (mut at_limit, mut varying, mut gap) = (0_u32, 0_u32, 0_u32);
        for star in oracle_by_cell(&oracle, &oracle_cells)
            .into_values()
            .flat_map(|(stars, _)| stars)
        {
            let radius = complete.radius_for(&star);
            if star.distance() >= radius {
                continue;
            }
            if bits(radius.value()) == bits(limit) {
                at_limit += 1;
            } else {
                varying += 1;
            }
            let ours = listed
                .get(&(star.system(), star.star().get()))
                .unwrap_or_else(|| panic!("{star:?} lies within its texel's R(u), unlisted"));
            assert_eq!(star_bits(&[**ours]), star_bits(&[star]));
            if !r06.contains(&(star.system(), star.star().get())) {
                gap += 1;
            }
        }
        eprintln!(
            "at the ceiling: {at_limit} stars within R(u) towards texels at the limit, {varying} \
             towards texels below it, all listed; R06's cones would leave {gap} of them in cells \
             left closed"
        );
        assert!(at_limit > 50 && varying > 50, "{at_limit}, {varying}");
        assert!(gap > 0, "the test meets R06.T7.b's gap");
    }
}
