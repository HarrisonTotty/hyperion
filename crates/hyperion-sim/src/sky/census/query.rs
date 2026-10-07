//! The sky's query and the census's plan: which cells of which layers it opens (rendering plan
//! R06, R06.T8.a; Design notes 9 and 10).

use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

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
use crate::units::{Degrees, LightYears, Magnitudes};

use super::super::caps::{CAPPED_LAYERS, LayerCap, layer_caps};
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
        })
    }
}

impl Error for BuildSkyQueryError {}

/// A cone of the sky about `axis`, for a deep exposure of a narrow field (Design note 5).
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

    /// The cosine of the half-angle: a direction lies inside the cone where its cosine with the
    /// axis is at least this. The census's stars ([`holds`](Self::holds)) and the band's rays
    /// (`band_rows`) test against the one value.
    #[must_use]
    pub fn cos_half_angle(&self) -> f64 {
        math::cos(self.half_angle.value() * RADIANS_PER_DEGREE)
    }

    /// Whether the direction of `displacement` from the apex lies inside the cone, its edge
    /// included: its cosine with the axis at least [`cos_half_angle`](Self::cos_half_angle),
    /// tested in metres as the displacement holds them. A zero displacement has no direction and is
    /// held, as a census with no cone keeps it; no star the census measures lies at its observer.
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

    /// Whether a ball of radius `radius_ly` whose centre lies `offset_ly` from the apex (both in
    /// light-years, on the galactic axes) meets the cone.
    #[must_use]
    pub(crate) fn meets_ball(&self, offset_ly: [f64; 3], radius_ly: f64) -> bool {
        let d = math::hypot(math::hypot(offset_ly[0], offset_ly[1]), offset_ly[2]);
        if d <= radius_ly {
            return true;
        }
        let u = self.axis.components();
        let cos = (offset_ly[0] * u[0] + offset_ly[1] * u[1] + offset_ly[2] * u[2]) / d;
        let angle = math::acos(cos.clamp(-1.0, 1.0));
        angle <= self.half_angle.value() * RADIANS_PER_DEGREE + math::asin(radius_ly / d)
    }
}

/// What a sky is asked for: an observer, a cut, the eye and its own cut if asked, the count
/// budget, an optional cone and the observer's own system to leave out (Design notes 5, 10 and 11).
/// Built through [`SkyQuery::builder`].
#[derive(Debug, Clone, PartialEq)]
pub struct SkyQuery {
    observer: Observer,
    cut: Magnitudes,
    eye: Option<EyeObserver>,
    /// The eye's cut: `Some` exactly when the eye is asked once built, and never deeper than the
    /// cut.
    eye_cut: Option<Magnitudes>,
    n_max: NonZeroU32,
    cone: Option<Cone>,
    exclude: Option<SystemId>,
    forced_caps: Option<Vec<LayerCap>>,
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
                cone: None,
                exclude: None,
                forced_caps: None,
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
    /// 2026-10-06, `decision-r06-t9c-glare.md`), so that at the same census radius the eye's limits
    /// are the eye-only request's, bit for bit. It is the query's cut unless a camera's deeper cut
    /// set that (Design note 5), and is never deeper than it. The band then keeps the light fainter
    /// than the eye's cut beside its own, as the eye's background
    /// ([`march_rows`](super::super::band::march_rows)), and only the listed stars at or brighter
    /// than it glare ([`Glare::of_listed`](super::super::limits::Glare::of_listed)).
    #[must_use]
    pub const fn eye_cut(&self) -> Option<Magnitudes> {
        self.eye_cut
    }

    /// The most stars listed.
    #[must_use]
    pub const fn n_max(&self) -> NonZeroU32 {
        self.n_max
    }

    /// The cone, if the sky is a narrow field.
    #[must_use]
    pub const fn cone(&self) -> Option<&Cone> {
        self.cone.as_ref()
    }

    /// The system left out: the observer's own, whose stars are discs.
    #[must_use]
    pub const fn exclude(&self) -> Option<SystemId> {
        self.exclude
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

    /// A narrow field: the census opens only the cells whose padded ball meets the cone, and keeps
    /// only the stars inside it ([`Cone::holds`]; R06.T8.k).
    #[must_use]
    pub fn cone(mut self, cone: Cone) -> Self {
        self.query.cone = Some(cone);
        self
    }

    /// Leaves out the observer's own system.
    #[must_use]
    pub fn exclude(mut self, system: SystemId) -> Self {
        self.query.exclude = Some(system);
        self
    }

    /// The query.
    ///
    /// # Errors
    ///
    /// [`BuildSkyQueryError::Cut`] unless the cut is finite and at most [`MAX_CUT_V`],
    /// [`BuildSkyQueryError::NMax`] if `n_max` is above [`MAX_N_MAX`], and
    /// [`BuildSkyQueryError::EyeCut`] if the eye's cut is asked without the eye, or is not finite
    /// or is deeper than the cut.
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
    cone: Option<Cone>,
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
        if self.cone.is_some() {
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
                cone: self.cone,
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
    cone: Option<Cone>,
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
            .filter(move |&key| meets_cone(slab.cone.as_ref(), slab.apex, key, slab.walk.pad))
    }
}

/// Whether `key`'s padded bounding ball meets `cone` about `apex` (ly), or there is no cone.
#[must_use]
fn meets_cone(cone: Option<&Cone>, apex: [f64; 3], key: CellKey, pad: LightYears) -> bool {
    let Some(cone) = cone else {
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
    cone.meets_ball(offset, half_diagonal + pad.value())
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
/// cap allows, and, for a cone, only the cells whose padded bounding ball meets it.
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
    let caps = match &query.forced_caps {
        Some(caps) => caps.clone(),
        None => layer_caps(galaxy, tables, envelope, observer, query.cut(), cache),
    };
    let walks = caps
        .iter()
        .filter_map(|cap| layer_walk(query, cap))
        .collect();
    CensusPlan {
        caps,
        walks,
        cone: query.cone().copied(),
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
/// the cap allows, and for a cone only those whose padded bounding ball meets it. A cap of no
/// radius opens nothing. Held, for a caller whose cells are few: a [`CensusPlan`] streams the same
/// cells ([`CensusPlan::cells`]).
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
                .filter(|&key| meets_cone(query.cone(), apex, key, walk.pad)),
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
        ] {
            let text = error.to_string();
            let field = match error {
                BuildSkyQueryError::Cut => "cut",
                BuildSkyQueryError::NMax => "n_max",
                BuildSkyQueryError::ConeHalfAngle => "half-angle",
                BuildSkyQueryError::ForcedCap => "forced cap",
                BuildSkyQueryError::EyeCut => "eye's cut",
            };
            assert!(text.contains(field), "{text}");
        }
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

    #[test]
    fn a_cone_keeps_only_cells_whose_box_meets_it() {
        let all: Vec<CellKey> = forced_plan(400.0, None).cells().collect();
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(10.0)).expect("a cone");
        let narrow: Vec<CellKey> = forced_plan(400.0, Some(cone)).cells().collect();
        assert!(narrow.len() < all.len() / 5);
        let apex = observer().position().to_light_years_f64();
        for key in &narrow {
            assert!(all.contains(key));
        }
        // Every dropped cell's box misses the cone: its bounding ball does.
        for key in all.iter().filter(|k| !narrow.contains(k)) {
            let size = f64::from(key.layer().cell_size_ly());
            let o = key.origin_ly();
            let offset = [
                f64::from(o[0]) + 0.5 * size - apex[0],
                f64::from(o[1]) + 0.5 * size - apex[1],
                f64::from(o[2]) + 0.5 * size - apex[2],
            ];
            assert!(!cone.meets_ball(offset, 0.5 * size * 3.0_f64.sqrt()));
        }
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
