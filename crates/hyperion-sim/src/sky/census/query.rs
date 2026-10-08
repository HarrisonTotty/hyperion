//! The sky's query and the census's plan: which cells of which layers it opens (rendering plan
//! R06, R06.T8.a; Design notes 9 and 10).

use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::coords::{GalacticDisplacement, UnitVector};
use crate::galaxy::Galaxy;
use crate::galaxy::gas::modifiers::GasModifierSource;
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::placement::CellKey;
use crate::galaxy::query::{
    QuerySphere, SystemSource, cells_in_sphere, cells_in_sphere_slab, count_cells_in_sphere,
    pad_for, pad_speed, sphere_slabs,
};
use crate::id::{Layer, SystemId};
use crate::math;
use crate::observe::Observer;
use crate::time::Span;
use crate::units::consts::{RADIANS_PER_DEGREE, SECONDS_PER_JULIAN_YEAR};
use crate::units::{Degrees, LightYears, Magnitudes, Radians};

use super::super::band::{BandSpec, CubeFace};
use super::super::caps::{CAPPED_LAYERS, LayerCap, layer_caps};
use super::super::dgl::Illumination;
use super::super::envelope::BrightnessEnvelope;
use super::super::eye::{EyeObserver, MAX_CUT_V};
use super::super::luminosity::LuminosityTables;
use super::cache::SkyCellCache;
use super::cell::CellOffsets;

/// The largest count of stars a sky lists, 3 × 10⁵ (Design note 11): 7.2 MB of payload.
pub const MAX_N_MAX: u32 = 300_000;

/// The largest forced cap, ly: the root cube's diagonal, 2¹⁷ × √3, beyond which nothing lies.
pub const MAX_FORCED_CAP_LY: f64 = 227_023.0;

/// [`MAX_N_MAX`] as a [`NonZeroU32`], the default.
const DEFAULT_N_MAX: NonZeroU32 = NonZeroU32::new(MAX_N_MAX).expect("300,000 is not zero");

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

    /// The query.
    ///
    /// # Errors
    ///
    /// [`BuildSkyQueryError::Cut`] unless the cut is finite and at most [`MAX_CUT_V`],
    /// [`BuildSkyQueryError::NMax`] if `n_max` is above [`MAX_N_MAX`],
    /// [`BuildSkyQueryError::EyeCut`] if the eye's cut is asked without the eye, or is not finite
    /// or is deeper than the cut, [`BuildSkyQueryError::ConeWithEye`] if the eye is asked with a
    /// cone, and [`BuildSkyQueryError::Illumination`] if the illumination was marched for another
    /// observer or at another time.
    pub fn build(mut self) -> Result<SkyQuery, BuildSkyQueryError> {
        let cut = self.query.cut.value();
        if !(cut.is_finite() && cut <= MAX_CUT_V) {
            return Err(BuildSkyQueryError::Cut);
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
    /// The per-cell cache of bright subsets ([`super::cache::NoSkyCellCache`] keeps none).
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

/// The census's plan: each layer's cap and the cells to open, in canonical order (layers as
/// [`CAPPED_LAYERS`], then each layer's cells as [`cells_in_sphere`] walks them).
///
/// It keeps each layer's padded sphere and streams the cells from it (R06.T8.f): near the Sun at
/// the eye's caps they number some 1.1 × 10⁸, which held as keys would take 2.2 GB. A server's
/// jobs take them slab by slab ([`slabs`](Self::slabs)), each an x slab of one layer's walk.
#[derive(Debug, Clone, PartialEq)]
pub struct CensusPlan {
    caps: Vec<LayerCap>,
    walks: Vec<LayerWalk>,
    region: Option<ConeRegion>,
    apex: [f64; 3],
}

impl CensusPlan {
    /// The caps, one per layer of [`CAPPED_LAYERS`].
    #[must_use]
    pub fn caps(&self) -> &[LayerCap] {
        &self.caps
    }

    /// The cells, in canonical order: [`plan_cells`]'s for the plan's query and caps, streamed.
    pub fn cells(&self) -> impl Iterator<Item = CellKey> + '_ {
        self.slabs().flat_map(|slab| slab.cells())
    }

    /// How many cells [`cells`](Self::cells) yields: counted column by column without visiting a
    /// cell, or, for a cone, by walking them.
    #[must_use]
    pub fn cell_count(&self) -> u64 {
        if self.region.is_some() {
            return self.cells().map(|_| 1_u64).sum();
        }
        self.walks
            .iter()
            .map(|walk| count_cells_in_sphere(walk.layer, &walk.sphere))
            .sum()
    }

    /// The plan's jobs: every x slab of each layer's walk, in canonical order, whose cells in turn
    /// are [`cells`](Self::cells). A slab may hold no cell.
    pub fn slabs(&self) -> impl Iterator<Item = CellSlab> + '_ {
        self.walks.iter().flat_map(move |&walk| {
            sphere_slabs(walk.layer, &walk.sphere).map(move |x| CellSlab {
                walk,
                x,
                region: self.region,
                apex: self.apex,
            })
        })
    }
}

/// One layer's walk in a [`CensusPlan`]: the sphere of its cap, padded, and the pad.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LayerWalk {
    layer: Layer,
    sphere: QuerySphere,
    pad: LightYears,
}

/// One x slab of one layer's walk in a [`CensusPlan`]: a census job's share of the plan, which
/// streams its own cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellSlab {
    walk: LayerWalk,
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

    /// The slab's x coordinate on its layer's grid.
    #[must_use]
    pub const fn x(&self) -> i32 {
        self.x
    }

    /// The slab's cells, in canonical order.
    pub fn cells(&self) -> impl Iterator<Item = CellKey> + use<> {
        let slab = *self;
        cells_in_sphere_slab(slab.walk.layer, &slab.walk.sphere, slab.x)
            .filter(move |&key| meets_cone(slab.region.as_ref(), slab.apex, key, slab.walk.pad))
    }
}

/// Whether `key`'s padded bounding ball meets the cone a plan opens cells by for `region`, about
/// `apex` (ly), or there is no cone: the cone widened to hold every texel of its region
/// ([`ConeRegion::meets_ball`]).
#[must_use]
fn meets_cone(region: Option<&ConeRegion>, apex: [f64; 3], key: CellKey, pad: LightYears) -> bool {
    let Some(region) = region else {
        return true;
    };
    let size = f64::from(key.layer().cell_size_ly());
    let half_diagonal = 0.5 * size * 3.0_f64.sqrt();
    let o = key.origin_ly();
    let offset = [
        f64::from(o[0]) + 0.5 * size - apex[0],
        f64::from(o[1]) + 0.5 * size - apex[1],
        f64::from(o[2]) + 0.5 * size - apex[2],
    ];
    region.meets_ball(offset, half_diagonal + pad.value())
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

/// The census's plan for `query` (Design notes 9 and 10): each layer's cap, then the cells of
/// [`cells_in_sphere`] to the cap, padded as the range query pads at the earliest emitted time the
/// cap allows, and, for a cone, only the cells whose padded bounding ball meets the cone widened to
/// hold every texel of its region, α + 2ρ ([`ConeRegion`]; R06.T8.l).
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
    let caps = match &query.forced_caps {
        Some(caps) => caps.clone(),
        None => layer_caps(
            galaxy,
            tables,
            envelope,
            query.observer(),
            query.cut(),
            cache,
        ),
    };
    census_plan_of(query, caps)
}

/// The census's plan for `query` to `caps`, one per layer of [`CAPPED_LAYERS`], which the caller
/// has computed: [`census_plan`] after its caps, for a server that runs the caps' rays as jobs of
/// its own ([`layer_caps_over`](crate::sky::caps::layer_caps_over); R06.T11.c). The query's own
/// forced caps, if it has them, are not read: the plan takes `caps`.
///
/// # Panics
///
/// If a cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
pub fn census_plan_of(query: &SkyQuery, caps: Vec<LayerCap>) -> CensusPlan {
    let observer = query.observer();
    let walks = caps
        .iter()
        .filter_map(|cap| layer_walk(query, cap))
        .collect();
    CensusPlan {
        caps,
        walks,
        region: query.cone_region().copied(),
        apex: observer.position().to_light_years_f64(),
    }
}

/// The walk of `cap`'s layer for `query`: [`cells_in_sphere`] to the cap, padded as the range query
/// pads at the earliest emitted time the cap allows; `None` for a cap of no radius.
///
/// # Panics
///
/// If the cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
fn layer_walk(query: &SkyQuery, cap: &LayerCap) -> Option<LayerWalk> {
    let observer = query.observer();
    let t = observer.time();
    let layer = cap.layer();
    let radius = cap.radius();
    if radius.value() <= 0.0 {
        return None;
    }
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
    Some(LayerWalk { layer, sphere, pad })
}

/// The cells a census of `query` opens for `caps`, in canonical order: each layer's cells of
/// [`cells_in_sphere`] to its cap, padded as the range query pads at the earliest emitted time
/// the cap allows, and for a cone only those whose padded bounding ball meets the cone widened to
/// hold every texel of its region (R06.T8.l). A cap of no radius opens nothing. Held, for a caller
/// whose cells are few: a [`CensusPlan`] streams the same cells ([`CensusPlan::cells`]).
///
/// # Panics
///
/// If a cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
pub fn plan_cells(query: &SkyQuery, caps: &[LayerCap]) -> Vec<CellKey> {
    let apex = query.observer().position().to_light_years_f64();
    let mut cells = Vec::new();
    for walk in caps.iter().filter_map(|cap| layer_walk(query, cap)) {
        cells.extend(
            cells_in_sphere(walk.layer, &walk.sphere)
                .filter(|&key| meets_cone(query.cone_region(), apex, key, walk.pad)),
        );
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
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
            };
            assert!(text.contains(field), "{text}");
        }
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
        // Every cell whose box meets the sphere is there, and the layers come in order.
        let mut last_layer = 0;
        for &key in &cells {
            let rank = CAPPED_LAYERS
                .iter()
                .position(|&l| l == key.layer())
                .expect("a capped layer");
            assert!(rank >= last_layer);
            last_layer = rank;
        }
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

    /// The plan's streamed cells are [`plan_cells`]' for the queries of the tests above, in the
    /// same order, slab by slab, and its count is theirs (R06.T8.f).
    #[test]
    fn streamed_cells_are_plan_cells() {
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(10.0)).expect("a cone");
        for (radius, cone) in [(300.0, None), (400.0, None), (400.0, Some(cone))] {
            let (query, plan) = forced_query_and_plan(radius, cone);
            let streamed: Vec<CellKey> = plan.cells().collect();
            let held = plan_cells(&query, plan.caps());
            assert!(!held.is_empty());
            assert_eq!(streamed, held, "{radius} ly, cone {cone:?}");
            assert_eq!(
                plan.cell_count(),
                u64::try_from(held.len()).expect("few"),
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
            assert_eq!(by_slab, held);
        }
    }
}
