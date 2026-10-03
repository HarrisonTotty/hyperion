//! The sky's query and the census's plan: which cells of which layers it opens (rendering plan
//! R06, R06.T8.a; Design notes 9 and 10).

use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

use crate::coords::UnitVector;
use crate::galaxy::Galaxy;
use crate::galaxy::gas::modifiers::GasModifierSource;
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::placement::CellKey;
use crate::galaxy::query::{QuerySphere, SystemSource, cells_in_sphere, pad_for, pad_speed};
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
}

impl fmt::Display for BuildSkyQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cut => "the cut is not finite or is deeper than V 11",
            Self::NMax => "n_max is above 300,000",
            Self::ConeHalfAngle => "the cone's half-angle is not within 0 to 90 degrees",
            Self::ForcedCap => "a forced cap is not within 0 to the root cube's diagonal",
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

/// What a sky is asked for: an observer, a cut, the eye if asked, the count budget, an optional
/// cone and the observer's own system to leave out (Design notes 5, 10 and 11). Built through
/// [`SkyQuery::builder`].
#[derive(Debug, Clone, PartialEq)]
pub struct SkyQuery {
    observer: Observer,
    cut: Magnitudes,
    eye: Option<EyeObserver>,
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

    /// The eye, if the sky is asked for one: then a star is kept to the cut plus its colour offset.
    #[must_use]
    pub const fn eye(&self) -> Option<&EyeObserver> {
        self.eye.as_ref()
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
    /// Asks for the eye's limits, which keep each star to the cut plus its colour offset.
    #[must_use]
    pub fn eye(mut self, eye: EyeObserver) -> Self {
        self.query.eye = Some(eye);
        self
    }

    /// The most stars listed.
    #[must_use]
    pub fn n_max(mut self, n_max: NonZeroU32) -> Self {
        self.query.n_max = n_max;
        self
    }

    /// A narrow field.
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
    /// [`BuildSkyQueryError::Cut`] unless the cut is finite and at most [`MAX_CUT_V`], and
    /// [`BuildSkyQueryError::NMax`] if `n_max` is above [`MAX_N_MAX`].
    pub fn build(self) -> Result<SkyQuery, BuildSkyQueryError> {
        let cut = self.query.cut.value();
        if !(cut.is_finite() && cut <= MAX_CUT_V) {
            return Err(BuildSkyQueryError::Cut);
        }
        if self.query.n_max.get() > MAX_N_MAX {
            return Err(BuildSkyQueryError::NMax);
        }
        Ok(self.query)
    }
}

/// What a census job reads and keeps: the tables, the envelope, its own noise cache, the cell
/// cache and the sources it shares with the other jobs (Design notes 10 and 12).
///
/// A plain bundle of borrows, built by its caller per job, so its fields are public: it carries no
/// invariant a constructor could check, and the census reads the tables only through it, so their
/// source can change without touching the census.
pub struct SkyContext<'a> {
    /// The luminosity tables at the query's time.
    pub tables: &'a LuminosityTables,
    /// The brightness envelope.
    pub envelope: &'a BrightnessEnvelope,
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
#[derive(Debug, Clone, PartialEq)]
pub struct CensusPlan {
    caps: Vec<LayerCap>,
    cells: Vec<CellKey>,
}

impl CensusPlan {
    /// The caps, one per layer of [`CAPPED_LAYERS`].
    #[must_use]
    pub fn caps(&self) -> &[LayerCap] {
        &self.caps
    }

    /// The cells, in canonical order.
    #[must_use]
    pub fn cells(&self) -> &[CellKey] {
        &self.cells
    }
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
    let cells = plan_cells(query, &caps);
    CensusPlan { caps, cells }
}

/// The cells a census of `query` opens for `caps`, in canonical order: each layer's cells of
/// [`cells_in_sphere`] to its cap, padded as the range query pads at the earliest emitted time
/// the cap allows, and for a cone only those whose padded bounding ball meets it. A cap of no
/// radius opens nothing.
///
/// # Panics
///
/// If a cap's sphere cannot be built, which a positive finite cap never fails.
#[must_use]
pub fn plan_cells(query: &SkyQuery, caps: &[LayerCap]) -> Vec<CellKey> {
    let observer = query.observer();
    let centre = *observer.position();
    let apex = centre.to_light_years_f64();
    let t = observer.time();
    let mut cells = Vec::new();
    for cap in caps {
        let layer = cap.layer();
        let radius = cap.radius();
        if radius.value() <= 0.0 {
            continue;
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
        let sphere = QuerySphere::new(centre, radius, t, pad)
            .expect("a positive finite cap and pad make a sphere");
        let size = f64::from(layer.cell_size_ly());
        let half_diagonal = 0.5 * size * 3.0_f64.sqrt();
        for key in cells_in_sphere(layer, &sphere) {
            if let Some(cone) = query.cone() {
                let o = key.origin_ly();
                let offset = [
                    f64::from(o[0]) + 0.5 * size - apex[0],
                    f64::from(o[1]) + 0.5 * size - apex[1],
                    f64::from(o[2]) + 0.5 * size - apex[2],
                ];
                if !cone.meets_ball(offset, half_diagonal + pad.value()) {
                    continue;
                }
            }
            cells.push(key);
        }
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
        for error in [
            BuildSkyQueryError::Cut,
            BuildSkyQueryError::NMax,
            BuildSkyQueryError::ConeHalfAngle,
            BuildSkyQueryError::ForcedCap,
        ] {
            let text = error.to_string();
            let field = match error {
                BuildSkyQueryError::Cut => "cut",
                BuildSkyQueryError::NMax => "n_max",
                BuildSkyQueryError::ConeHalfAngle => "half-angle",
                BuildSkyQueryError::ForcedCap => "forced cap",
            };
            assert!(text.contains(field), "{text}");
        }
    }

    /// A plan with every cap forced, for the cell tests (no tables or rays are read).
    fn forced_plan(radius: f64, cone: Option<Cone>) -> CensusPlan {
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
        census_plan(galaxy, &tables, &envelope, &query, &mut cache)
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
        // Every cell whose box meets the sphere is there, and the layers come in order.
        let mut last_layer = 0;
        for key in plan.cells() {
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
                            assert!(plan.cells().contains(&key), "{key:?} is missing");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_cone_keeps_only_cells_whose_box_meets_it() {
        let all = forced_plan(400.0, None);
        let cone = Cone::new(UnitVector::NORTH, Degrees::new(10.0)).expect("a cone");
        let narrow = forced_plan(400.0, Some(cone));
        assert!(narrow.cells().len() < all.cells().len() / 5);
        let apex = observer().position().to_light_years_f64();
        for key in narrow.cells() {
            assert!(all.cells().contains(key));
        }
        // Every dropped cell's box misses the cone: its bounding ball does.
        for key in all.cells().iter().filter(|k| !narrow.cells().contains(k)) {
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
}
