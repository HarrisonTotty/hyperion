//! What a readout may quote of the ground, given the survey the ship holds there (plan R10, T5;
//! Design notes 6 and 7).
//!
//! The client holds exact cells wherever it holds any, so it draws surveyed ground at whatever
//! detail its camera resolves. A number it states is another matter: it quotes the ground only as
//! well as the ship has measured it (the brainstorm's "Knowledge, and the surface seed").
//! [`surveyed_reading`] gives the elevation and the slope at a point, and [`slant_range`] the
//! distance along a line of sight to the ground it first meets, each from the survey resolution of
//! the cells it reads, which the view knows ([`SurveyView`], a client's [`PartialField`]).
//!
//! # The survey's level
//!
//! A cell surveyed at resolution code c is read at the deepest band level n whose shortest
//! wavelength `λ_n` (R09's [`shortest_wavelength`]) codes no finer than c: the level that adds
//! nothing the survey did not resolve, compared in the code's own steps of 9%, so that a pass at
//! 2 m, which codes as "2.15 m or finer", reaches the 2 m band limit. A survey that reaches the
//! band limit's level is resolved: its elevation is the authoritative ground's ([`ground_at`]),
//! and its uncertainties are zero.
//!
//! # Elevation
//!
//! The height above the datum along its normal at that level ([`Synthesiser::height_at`]), with
//! one standard deviation of every contribution the level leaves out of the finest level's ground
//! as its uncertainty (Design note 6: 1σ and not LE90, since the unresolved heights are skewed and
//! heavy-tailed, and current practice states RMS).
//!
//! That is R09's level-keyed closed form, [`Synthesiser::omitted_rms`] at the level, not its
//! [`Synthesiser::unresolved_rms`] at `λ_n`: a level evaluates each contribution by its own
//! scale, while the wavelength-keyed forms place each at wavelengths of their own, so a level
//! leaves out more than they count at `λ_n`, 2.6 to 3.3 times in the structural octaves' variance
//! on the synthetic worlds (`decision-r10-t5-closed-forms.md`). Each contribution's cut is at
//! least `λ_n`, so the uncertainty is at least R09's unresolved RMS at `λ_n`, the safe side.
//!
//! # Slope
//!
//! The adirectional slope, the gradient's magnitude from central differences over a baseline b
//! along local east and north, which is what tilts a lander; a two-point profile slope's RMS is
//! smaller by √2 on isotropic Gaussian terrain (Shepard et al. 2001, as Beyer, McEwen and Kirk
//! 2003, JGR 108, 8085, ¶25, cite it; found empirically by Kirk et al. 2003). The resolved part
//! g₀ is the central difference of the level's heights over b, whose limit for b short against
//! `λ_n` is the gradient's tangential part (R09's "Deviations in T4, as built"). The unresolved
//! part adds to each axis a variance v = D(b) ÷ b², D the structure function at lag b of what the
//! level leaves out, R09's level-keyed [`Synthesiser::omitted_structure_function`]: the sum over the
//! unresolved bands of 2 `σ_k`² (1 − J₀(k b)) and its craters' and channels' forms (Shepard et al.
//! 2001, JGR 106, 32777), about 2 `σ_k`² ÷ b² for a band much finer than b, and the band's gradient
//! variance for one much coarser. The slope's magnitude is then Rician about |g₀|, biased upward
//! while |g₀| is below about 2√v, so the reading quotes the RMS slope √(|g₀|² + 2v), never below
//! |g₀|, with the Rice distribution's standard deviation as its uncertainty: quoting |g₀| alone
//! would understate slopes from orbit, the unsafe direction. The Rice spread takes the unresolved
//! slope as Gaussian and alike on both axes, an approximation where channels run one way and
//! craters are heavy-tailed.
//!
//! # Slant range
//!
//! A march of the line of sight p(t) = o + t d against the ground at each cell's survey level. On
//! surveyed ground it steps by the ray's clearance over 3, a Lipschitz factor that holds for slopes
//! to 70.5°, never below a quarter of the level's shortest wavelength nor past half a coarse cell,
//! and bisects the first step that ends under the ground to a micrometre. Above the highest ground
//! the view's cells and the header allow (the highest cell it holds, plus the header's greatest
//! relief and the most any level's contributions add) it takes Newton's step to that ceiling,
//! which never overshoots, since a height above a convex datum is convex along a line.
//! A ray low enough to meet ground the ship has not surveyed, a survey's margin or a cell the view
//! does not hold, before it meets surveyed ground, has no range: the ship cannot say what stands
//! there ([`LineOfSight::NotSurveyed`]).
//!
//! Linearising the hit on z = h + δ gives dt ÷ dδ = cos `θ_s` ÷ sin `γ_s`, `θ_s` the resolved
//! surface's slope and `γ_s` the ray's grazing angle to it, so `σ_R` = `σ_h` cos `θ_s` ÷ sin `γ_s`.
//! Where unresolved relief can stand in front of the hit, sin `γ_s` under
//! [`GRAZING_FACTOR`] √v at b equal to `λ_n`, the linearisation fails, the true range is biased
//! short, and the range carries no σ ([`SlantRange::is_grazing`]).
//!
//! These readings are presentation: they move nothing generated and carry no generator version
//! (the plan's Generator version section). They read only through `hyperion_base::math`, as the
//! crate does.

use core::f64::consts::FRAC_PI_2;

use hyperion_base::math;
use hyperion_base::units::{Metres, Radians, SquareMetres};

use crate::cube::{
    FaceUv, MAX_LEVEL, PATCH_QUADS, PatchKey, face_uv_to_xyz, unit_dir, xyz_to_face_uv,
};
use crate::field::{FieldView, PartialField, ResolutionCode, cell_at_index};
use crate::geometry::{BAND_LIMIT_M, vertex_spacing};
use crate::ground::{datum_foot, ground_at};
use crate::height::HeightSample;
use crate::num;
use crate::patch::normals::surface_normal;
use crate::spheroid::Spheroid;
use crate::synth::levels::shortest_wavelength;
use crate::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser};

/// The baselines a slope may be read over, metres: 2 m, the default, the band limit and a lander
/// pad's scale; 5 m; and 100 m (Design note 7). The first two are the lander-stability baselines
/// of `InSight`'s site selection, slopes at 2–5 m from `HiRISE` DEMs against a 15° limit
/// (Golombek et al. 2014, LPSC 45, abstract 1499), and the third stands near the same limit's
/// 84 m scale for the landing radar (Golombek et al. 2017, Space Sci. Rev. 211, 5, §2.7); the 2014
/// abstract also read slopes at about 300 m from MOLA. The readout states the one it uses after a
/// middle dot, `SLOPE 7° · BASELINE 2 m` (signoff-9).
pub const SLOPE_BASELINES: [Metres; 3] = [Metres::new(2.0), Metres::new(5.0), Metres::new(100.0)];

/// The factor of the grazing test: a line of sight is grazing where the sine of its grazing angle
/// to the resolved surface is under this many standard deviations of the unresolved slope per axis
/// over the survey's cell, sin `γ_s` < 2√v at b = `λ_n` (Design note 7). There unresolved relief
/// can stand in front of the hit, and the readout states the range without an uncertainty,
/// `SLANT RANGE ~41.2 km · GRAZING` (signoff-9).
pub const GRAZING_FACTOR: f64 = 2.0;

/// The march's Lipschitz factor: on surveyed ground it steps by the ray's clearance over 3, which
/// cannot pass under ground whose slope stays below atan √8, 70.5°, since the clearance then falls
/// by at most √(1 + 8) = 3 a metre of ray. A choice, not a bound: no slope of the synthesis has a
/// largest value, and a steeper wall thinner than the march's shortest step (a quarter of the
/// level's shortest wavelength) can be stepped past at a grazing angle.
pub(crate) const LIPSCHITZ: f64 = 3.0;

/// How far above the ceiling, metres, the march takes Newton's step to it rather than reading the
/// ground: each such step is then at least this long.
const FAR_MARGIN_M: f64 = 1.0;

/// The slack, metres, added to the ceiling for the rounding of its terms.
const CEILING_SLACK_M: f64 = 1.0;

/// The longest bracket, metres, the march's bisection leaves: a micrometre, or four ulps of the
/// range where that is coarser.
const RANGE_TOLERANCE_M: f64 = 1e-6;

/// Millimetres a metre, a cell's elevation's unit.
const MM_PER_M: f64 = 1_000.0;

/// From this x = ν² ÷ 2σ² on, [`rice_sigma`] takes the asymptotic series, whose first omitted term
/// is about 0.4 ÷ x⁴ of the variance, 1.6 × 10⁻⁷ here; below it, Kummer's series, of about x + 10√x
/// terms.
const RICE_ASYMPTOTIC_FROM: f64 = 40.0;

/// The most terms [`laguerre_half`] sums: enough for x up to [`RICE_ASYMPTOTIC_FROM`] with room.
const KUMMER_TERMS: u32 = 1_000;

/// A field view that knows how finely each cell it holds was surveyed: what a reading reads beyond
/// the heights.
pub trait SurveyView: FieldView {
    /// The resolution code of `cell`, a cell at the field's level: a survey's code for a surveyed
    /// cell, [`ResolutionCode::NONE`] for a cell held only as a survey's margin, and `None` for a
    /// cell the view does not hold. A code is `Some` only for a cell [`FieldView::cell`] holds; a
    /// view that breaks this has its readings refused as [`QueryHeightError::NotSurveyed`].
    fn resolution(&self, cell: PatchKey) -> Option<ResolutionCode>;
}

/// A client's field: each cell's code as its payload's cover carried it.
impl SurveyView for PartialField {
    fn resolution(&self, cell: PatchKey) -> Option<ResolutionCode> {
        Self::resolution(self, cell)
    }
}

/// What a readout may quote of the ground at a point: its elevation and slope, each with its
/// uncertainty, given the survey there.
///
/// The view's terrain readout shows them under `GROUND AT LINE OF SIGHT` as `ELEVATION` and
/// `SLOPE` with its `· BASELINE 2 m` part, Estimated with `~` and `± …` until the point is
/// resolved, each value rounded to its uncertainty (`~2140 m ± 180 m`, `SLOPE ~12° ± 4° ·
/// BASELINE 2 m`), and `NOT SURVEYED` once in place of the readings where the ground has not been
/// surveyed (signoff-9; R10.T12 formats them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurveyedReading {
    elevation: Metres,
    elevation_sigma: Metres,
    slope_rms: Radians,
    slope_sigma: Radians,
    slope_baseline: Metres,
    resolution: Metres,
    resolved: bool,
}

impl SurveyedReading {
    /// The elevation, metres above the datum along its normal: the height at the survey's level,
    /// or the authoritative ground's where the survey is resolved.
    #[must_use]
    pub const fn elevation(&self) -> Metres {
        self.elevation
    }

    /// The elevation's standard uncertainty, metres: one standard deviation of the ground the
    /// survey's level leaves out; zero where resolved.
    #[must_use]
    pub const fn elevation_sigma(&self) -> Metres {
        self.elevation_sigma
    }

    /// The RMS adirectional slope over [`slope_baseline`](Self::slope_baseline), radians from the
    /// local horizontal: the angle whose tangent is √(|g₀|² + 2v), never below the resolved
    /// slope's.
    #[must_use]
    pub const fn slope_rms(&self) -> Radians {
        self.slope_rms
    }

    /// The slope's standard uncertainty, radians: the Rice distribution's standard deviation of
    /// the gradient's magnitude, taken through the arctangent's derivative at the RMS slope; zero
    /// where resolved.
    #[must_use]
    pub const fn slope_sigma(&self) -> Radians {
        self.slope_sigma
    }

    /// The baseline the slope is read over, metres ([`SLOPE_BASELINES`]).
    #[must_use]
    pub const fn slope_baseline(&self) -> Metres {
        self.slope_baseline
    }

    /// The survey resolution the reading was taken at, metres: the coarsest its cell's code admits
    /// ([`ResolutionCode::resolution`]), which the readout states as `RESOLUTION` (signoff-9).
    #[must_use]
    pub const fn resolution(&self) -> Metres {
        self.resolution
    }

    /// Whether the survey reaches the band limit there, so that both uncertainties are zero and the
    /// readout drops its `~` and `± …`.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        self.resolved
    }
}

/// The reading at `at` on `synth`'s body, its slope over `slope_baseline`, or `None` where the
/// ground there has not been surveyed: a cell held only as a survey's margin, or not held (see the
/// [module documentation](self)).
///
/// `cache` is the caller's, and no result depends on what it holds.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] where a height the slope reads strays past the cells the view
/// holds, which a surveyed cell's margin makes all but impossible.
///
/// # Panics
///
/// If `slope_baseline` is not finite and positive, `at` names no direction, or a height is not
/// finite (a bug).
pub fn surveyed_reading<F: SurveyView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    at: FaceUv,
    slope_baseline: Metres,
) -> Result<Option<SurveyedReading>, QueryHeightError> {
    reading_over(synth, cache, unit_dir(face_uv_to_xyz(at)), slope_baseline)
}

/// Where a line of sight meets the ground, as far as the survey can say.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineOfSight {
    /// It meets surveyed ground, at this slant range.
    Ground(SlantRange),
    /// It meets no ground before it rises above every ground the body may have, or before it has
    /// gone the reach it was given. The readout's `NO GROUND` is the first alone, a line of sight
    /// that passes the body (signoff-9), so a caller that states it gives a reach past the body's
    /// far side.
    NoGround,
    /// It passes low enough to meet ground the ship has not surveyed before it meets any it has, so
    /// the survey cannot say where it meets the ground: the readout's `NOT SURVEYED`.
    NotSurveyed,
}

/// The distance along a line of sight to the ground it first meets, with its uncertainty.
///
/// The readout shows it as `SLANT RANGE`, Estimated with `~` and `± …` until resolved
/// (`~41.2 km ± 0.3 km`), or with `GRAZING` after a middle dot in place of the uncertainty
/// (`SLANT RANGE ~41.2 km · GRAZING`), and never `GRAZING` on a resolved range (signoff-9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlantRange {
    range: Metres,
    sigma: Option<Metres>,
    resolved: bool,
    hit_m: [f64; 3],
    at: FaceUv,
}

impl SlantRange {
    /// The distance from the line of sight's origin to the ground, metres.
    #[must_use]
    pub const fn range(&self) -> Metres {
        self.range
    }

    /// The range's standard uncertainty, metres, `σ_h` cos `θ_s` ÷ sin `γ_s`: zero where resolved,
    /// and `None` where the line of sight is [grazing](Self::is_grazing).
    #[must_use]
    pub const fn sigma(&self) -> Option<Metres> {
        self.sigma
    }

    /// Whether the line of sight meets the ground so obliquely that relief finer than the survey
    /// could stand in front of the point: sin `γ_s` < [`GRAZING_FACTOR`] √v. Never on a resolved
    /// range.
    #[must_use]
    pub const fn is_grazing(&self) -> bool {
        self.sigma.is_none()
    }

    /// Whether the survey reaches the band limit at the hit, so that the range is the ground's.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// The point the line of sight meets the ground, body-fixed, metres.
    #[must_use]
    pub const fn hit_m(&self) -> [f64; 3] {
        self.hit_m
    }

    /// The hit's foot on the datum, where [`surveyed_reading`] reads the ground under it.
    #[must_use]
    pub const fn at(&self) -> FaceUv {
        self.at
    }
}

/// Where the line of sight from `origin_m` along `direction`, body-fixed, metres, meets `synth`'s
/// ground within `max` metres of it, as far as the survey can say (see the
/// [module documentation](self)).
///
/// `direction` need not be a unit vector. `cache` is the caller's, and no result depends on what
/// it holds. The march reads every cell the view holds once a call, for its ceiling (about a
/// millisecond on a level-8 field).
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] where a height on surveyed ground strays past the cells the
/// view holds, which a surveyed cell's margin makes all but impossible.
///
/// # Panics
///
/// If `origin_m` is not finite, `direction` is zero or not finite, `max` is not finite and
/// non-negative, or a point of the march is the body's centre or a height is not finite (a bug).
pub fn slant_range<F: SurveyView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    origin_m: &[f64; 3],
    direction: &[f64; 3],
    max: Metres,
) -> Result<LineOfSight, QueryHeightError> {
    march(synth, cache, origin_m, direction, max)
}

/// What a view knows of a cell's survey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Survey {
    /// Surveyed at `code`.
    Surveyed {
        /// The cell, at the field's level.
        cell: PatchKey,
        /// Its survey's resolution code.
        code: ResolutionCode,
    },
    /// Held only as a survey's margin: its heights may be known, but not surveyed.
    Margin,
    /// Not held.
    NotHeld,
}

/// The ground a reading reads: R09's synthesis over a [`SurveyView`], or a test's analytic ground.
pub(crate) trait SurveyedGround {
    /// The caller's cache.
    type Cache;
    /// Why a height could not be had.
    type Error;

    /// The datum.
    fn figure(&self) -> Spheroid;

    /// The coarse field's level, whose cells the survey is kept by.
    fn field_level(&self) -> u8;

    /// What the view knows of the survey of the cell holding the unit direction `dir`.
    fn survey(&self, dir: [f64; 3]) -> Survey;

    /// The height and gradient at `dir` at `level` (R09's `height_at`).
    ///
    /// # Errors
    ///
    /// Where a read strays past the cells the view holds.
    fn height(
        &self,
        cache: &mut Self::Cache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<HeightSample, Self::Error>;

    /// [`height`](Self::height), or `None` where a read strays past the cells the view holds: for a
    /// survey's margin, whose outer cells cannot all be synthesised.
    fn held_height(
        &self,
        cache: &mut Self::Cache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Option<HeightSample>;

    /// The authoritative ground's height under the body-fixed point `p`, metres ([`ground_at`]).
    ///
    /// # Errors
    ///
    /// Where a read strays past the cells the view holds.
    fn ground(&self, cache: &mut Self::Cache, p: &[f64; 3]) -> Result<Metres, Self::Error>;

    /// The RMS, metres, of what `level` leaves out of the finest level's ground in surveyed `cell`.
    ///
    /// # Errors
    ///
    /// Where the view does not hold the cell, though its survey said it did.
    fn omitted_rms(&self, cell: PatchKey, level: BandLevel) -> Result<Metres, Self::Error>;

    /// The structure function, square metres, at a lag of `lag`, of what `level` leaves out of the
    /// finest level's ground in surveyed `cell`.
    ///
    /// # Errors
    ///
    /// Where the view does not hold the cell, though its survey said it did.
    fn omitted_structure(
        &self,
        cell: PatchKey,
        level: BandLevel,
        lag: Metres,
    ) -> Result<SquareMetres, Self::Error>;

    /// A height above the datum, metres, that no ground of the body reaches, at any level, held or
    /// not; +∞ where the view can bound nothing.
    fn ceiling_m(&self) -> f64;
}

/// R09's synthesis over a view that knows its survey.
impl<F: SurveyView> SurveyedGround for Synthesiser<'_, F> {
    type Cache = SynthCache;
    type Error = QueryHeightError;

    fn figure(&self) -> Spheroid {
        self.field().header().figure()
    }

    fn field_level(&self) -> u8 {
        self.field().header().level().get()
    }

    fn survey(&self, dir: [f64; 3]) -> Survey {
        let cell = PatchKey::containing(self.field_level(), dir)
            .expect("a coarse field's level is a level");
        match SurveyView::resolution(self.field(), cell) {
            None => Survey::NotHeld,
            Some(code) if code.is_surveyed() => Survey::Surveyed { cell, code },
            Some(_) => Survey::Margin,
        }
    }

    fn height(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<HeightSample, QueryHeightError> {
        self.height_at(cache, dir, level)
    }

    fn held_height(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Option<HeightSample> {
        // `NotSurveyed`, the one error, is the answer asked for: a cell the view does not hold.
        self.height_at(cache, dir, level).ok()
    }

    fn ground(&self, cache: &mut SynthCache, p: &[f64; 3]) -> Result<Metres, QueryHeightError> {
        ground_at(self, cache, p).map(|g| g.height())
    }

    fn omitted_rms(&self, cell: PatchKey, level: BandLevel) -> Result<Metres, QueryHeightError> {
        Synthesiser::omitted_rms(self, cell, level).ok_or(QueryHeightError::NotSurveyed(cell))
    }

    fn omitted_structure(
        &self,
        cell: PatchKey,
        level: BandLevel,
        lag: Metres,
    ) -> Result<SquareMetres, QueryHeightError> {
        self.omitted_structure_function(cell, level, lag)
            .ok_or(QueryHeightError::NotSurveyed(cell))
    }

    /// The highest cell elevation the view holds, plus the header's greatest relief, plus the most
    /// any level's contributions add ([`crate::synth::LevelBounds::contribution_range_m`]), plus a
    /// metre: the base elevation is a convex blend of the cells it reads, and no cell of the body
    /// lies above its lowest by more than the relief, nor its lowest above any cell held. The
    /// craters' share of the contributions is R09's 1 − 10⁻⁹ quantile. +∞ where the view holds no
    /// cell.
    fn ceiling_m(&self) -> f64 {
        let field = self.field();
        let header = field.header();
        let level = header.level();
        let highest = (0..level.cell_count())
            .filter_map(|index| cell_at_index(level.get(), index))
            .filter_map(|cell| field.cell(cell))
            .map(|record| record.elevation_mm)
            .max();
        let Some(highest) = highest else {
            return f64::INFINITY;
        };
        let reach = (0..=MAX_LEVEL)
            .map(|n| {
                let band = BandLevel::new(n).expect("a level to the deepest is a band level");
                self.bounds().contribution_range_m(band).1
            })
            .fold(0.0, num::max);
        num::assert_finite(
            f64::from(highest) / MM_PER_M
                + header.realised_relief().value()
                + reach
                + CEILING_SLACK_M,
        )
    }
}

/// The band level a survey is read at (see the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SurveyLevel {
    /// The level.
    pub(crate) band: BandLevel,
    /// Its shortest wavelength, `λ_n`.
    pub(crate) wavelength: Metres,
    /// Whether it is the band limit's level, the shallowest whose `λ_n` is 2 m.
    pub(crate) resolved: bool,
}

/// The level a survey of resolution code `code` is read at on `figure`: the deepest to the band
/// limit's whose shortest wavelength codes no finer than `code`, or level 0 for a survey coarser
/// than every level.
///
/// # Panics
///
/// If `code` is [`ResolutionCode::NONE`], which no survey has.
#[must_use]
pub(crate) fn survey_level(figure: &Spheroid, code: ResolutionCode) -> SurveyLevel {
    assert!(code.is_surveyed(), "a survey's level needs a survey's code");
    let band = |n: u8| BandLevel::new(n).expect("a level to the deepest is a band level");
    let limit = (0..=MAX_LEVEL)
        .find(|&n| shortest_wavelength(figure, band(n)).value() <= BAND_LIMIT_M)
        .unwrap_or(MAX_LEVEL);
    // A wavelength too coarse for any code is coarser than every survey.
    let resolves_no_more = |n: u8| {
        ResolutionCode::from_resolution(shortest_wavelength(figure, band(n)))
            .map_or(true, |c| c >= code)
    };
    let level = (0..=limit)
        .rev()
        .find(|&n| resolves_no_more(n))
        .unwrap_or(0);
    SurveyLevel {
        band: band(level),
        wavelength: shortest_wavelength(figure, band(level)),
        resolved: level == limit,
    }
}

/// [`surveyed_reading`] over any [`SurveyedGround`], at the unit direction `dir`.
pub(crate) fn reading_over<S: SurveyedGround>(
    source: &S,
    cache: &mut S::Cache,
    dir: [f64; 3],
    slope_baseline: Metres,
) -> Result<Option<SurveyedReading>, S::Error> {
    let b = slope_baseline.value();
    assert!(
        b.is_finite() && b > 0.0,
        "a slope's baseline must be finite and positive, got {b} m"
    );
    let Survey::Surveyed { cell, code } = source.survey(dir) else {
        return Ok(None);
    };
    let figure = source.figure();
    let level = survey_level(&figure, code);
    let (elevation, sigma) = if level.resolved {
        (source.ground(cache, &figure.point(dir))?.value(), 0.0)
    } else {
        (
            source.height(cache, dir, level.band)?.height_m,
            source.omitted_rms(cell, level.band)?.value(),
        )
    };
    let slope = slope_terms(source, cache, dir, elevation, cell, level, slope_baseline)?;
    let (rms, sd) = slope.rms_and_sigma();
    Ok(Some(SurveyedReading {
        elevation: Metres::new(num::assert_finite(elevation)),
        elevation_sigma: Metres::new(num::assert_finite(sigma)),
        slope_rms: Radians::new(num::assert_finite(rms)),
        slope_sigma: Radians::new(num::assert_finite(sd)),
        slope_baseline,
        resolution: code.resolution().expect("a survey's code has a resolution"),
        resolved: level.resolved,
    }))
}

/// A slope's two parts over a baseline: the resolved gradient's local east and north components,
/// metres per metre, and the unresolved variance each adds, v.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SlopeTerms {
    /// g₀, east then north, metres per metre.
    pub(crate) resolved: [f64; 2],
    /// v = D(b) ÷ b², per axis.
    pub(crate) axis_variance: f64,
}

impl SlopeTerms {
    /// |g₀|.
    #[must_use]
    pub(crate) fn resolved_magnitude(&self) -> f64 {
        let [e, n] = self.resolved;
        (e * e + n * n).sqrt()
    }

    /// The RMS slope, atan √(|g₀|² + 2v), and its uncertainty, the Rice standard deviation through
    /// the arctangent's derivative there, radians.
    #[must_use]
    pub(crate) fn rms_and_sigma(&self) -> (f64, f64) {
        let g0 = self.resolved_magnitude();
        let mean_square = g0 * g0 + 2.0 * self.axis_variance;
        let sigma = rice_sigma(g0, self.axis_variance);
        (math::atan(mean_square.sqrt()), sigma / (1.0 + mean_square))
    }
}

/// The slope's terms at the unit direction `dir`, whose ground is `elevation` metres, in surveyed
/// `cell` at `level`, over `baseline`: central differences of the level's heights between the
/// feet of the points half the baseline either way along local east and north from the ground
/// point, in the tangent plane there, so that the datum's curvature cancels between them.
fn slope_terms<S: SurveyedGround>(
    source: &S,
    cache: &mut S::Cache,
    dir: [f64; 3],
    elevation: f64,
    cell: PatchKey,
    level: SurveyLevel,
    baseline: Metres,
) -> Result<SlopeTerms, S::Error> {
    let figure = source.figure();
    let b = baseline.value();
    let centre = figure.surface_point(dir, elevation);
    let up = figure.normal(dir);
    let (east, north) = east_north(up);
    let mut resolved = [0.0; 2];
    for (component, axis) in resolved.iter_mut().zip([east, north]) {
        let mut heights = [0.0; 2];
        for (height, sign) in heights.iter_mut().zip([0.5, -0.5]) {
            let q = [0, 1, 2].map(|k| centre[k] + sign * b * axis[k]);
            let (foot, _) = datum_foot(&figure, &q);
            *height = source.height(cache, foot, level.band)?.height_m;
        }
        *component = (heights[0] - heights[1]) / b;
    }
    let axis_variance = if level.resolved {
        0.0
    } else {
        source
            .omitted_structure(cell, level.band, baseline)?
            .value()
            / (b * b)
    };
    Ok(SlopeTerms {
        resolved,
        axis_variance,
    })
}

/// Local east and north at the outward normal `up`: east = ẑ × ν ÷ |ẑ × ν|, x̂ at a pole, and
/// north = ν × east.
#[must_use]
fn east_north(up: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let across = (up[0] * up[0] + up[1] * up[1]).sqrt();
    let east = if across > 0.0 {
        [-up[1] / across, up[0] / across, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let north = cross(up, east);
    (east, north)
}

/// The standard deviation of |g₀ + δ| for δ of variance `variance` on each of two axes, `nu` = |g₀|:
/// the Rice distribution's, √(ν² + 2σ² − μ²) with its mean μ = σ √(π ÷ 2) `L_½`(−ν² ÷ 2σ²)
/// (Rice 1944–45, Bell Syst. Tech. J. 23, 282 and 24, 46); zero for no variance.
///
/// With x = ν² ÷ 2σ², the variance is σ² (2 + 2x − (π ÷ 2) `L_½`(−x)²), from Kummer's series
/// ([`laguerre_half`]) below x = 40, and the asymptotic σ² (1 − 1 ÷ 4x − 1 ÷ 8x² − 11 ÷ 64x³) from
/// it, the square of `L_½`(−x) ~ (2 ÷ √π) √x (1 + 1 ÷ 4x + 1 ÷ 32x² + 3 ÷ 128x³ + 75 ÷ 2048x⁴) (the
/// expansion of M(a, b, z) for large −z, Abramowitz and Stegun 13.5.1, at a = −½, b = 1). It is
/// σ √(2 − π ÷ 2), Rayleigh's, at ν = 0, and tends to σ as ν grows.
#[must_use]
pub(crate) fn rice_sigma(nu: f64, variance: f64) -> f64 {
    if variance <= 0.0 {
        return 0.0;
    }
    let x = nu * nu / (2.0 * variance);
    let ratio = if x < RICE_ASYMPTOTIC_FROM {
        let l = laguerre_half(x);
        2.0 + 2.0 * x - FRAC_PI_2 * l * l
    } else {
        let r = 1.0 / x;
        1.0 - r * (0.25 + r * (0.125 + r * (11.0 / 64.0)))
    };
    (variance * num::max(ratio, 0.0)).sqrt()
}

/// The Laguerre function `L_½`(−x) = M(−½, 1, −x) = e^−x M(3/2, 1, x) for x ≥ 0, by Kummer's
/// transformation (Abramowitz and Stegun 13.1.27) and the series M(3/2, 1, x) =
/// Σ (3/2)ₖ xᵏ ÷ (k!)², whose terms are all positive, summed until a term no longer moves the sum
/// past the largest term.
#[must_use]
pub(crate) fn laguerre_half(x: f64) -> f64 {
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..=KUMMER_TERMS {
        let k = f64::from(k);
        term *= (k + 0.5) * x / (k * k);
        sum += term;
        if k > x && term <= f64::EPSILON * sum {
            break;
        }
    }
    math::exp(-x) * sum
}

/// [`slant_range`] over any [`SurveyedGround`].
pub(crate) fn march<S: SurveyedGround>(
    source: &S,
    cache: &mut S::Cache,
    origin: &[f64; 3],
    direction: &[f64; 3],
    max: Metres,
) -> Result<LineOfSight, S::Error> {
    assert!(
        origin.iter().all(|x| x.is_finite()),
        "a line of sight's origin must be finite, got {origin:?}"
    );
    let reach = max.value();
    assert!(
        reach.is_finite() && reach >= 0.0,
        "a line of sight's reach must be finite and non-negative, got {reach} m"
    );
    let figure = source.figure();
    let marcher = Marcher {
        source,
        figure,
        origin: *origin,
        direction: unit_dir(*direction),
        ceiling_m: source.ceiling_m(),
        cell_step_m: 0.5
            * f64::from(PATCH_QUADS)
            * vertex_spacing(figure.polar_radius_m, source.field_level()).min_m,
        band_limit: survey_level(&figure, ResolutionCode::FINEST),
    };
    let mut t = 0.0;
    let mut probe = marcher.probe(cache, t)?;
    loop {
        match probe {
            Probe::Clear(step) => {
                if t >= reach {
                    return Ok(LineOfSight::NoGround);
                }
                let next = num::min(t + step, reach);
                let ahead = marcher.probe(cache, next)?;
                if let Probe::Below(contact) = ahead {
                    return marcher.refine(cache, t, next, contact);
                }
                t = next;
                probe = ahead;
            }
            Probe::Below(contact) => {
                return Ok(LineOfSight::Ground(marcher.range_at(t, contact)?));
            }
            Probe::Unsurveyed => return Ok(LineOfSight::NotSurveyed),
            Probe::Leaving => return Ok(LineOfSight::NoGround),
        }
    }
}

/// A line of sight's march against a [`SurveyedGround`].
struct Marcher<'s, S> {
    source: &'s S,
    figure: Spheroid,
    origin: [f64; 3],
    direction: [f64; 3],
    ceiling_m: f64,
    cell_step_m: f64,
    band_limit: SurveyLevel,
}

/// What the march finds at a point of the line of sight.
enum Probe {
    /// Clear of the ground, and safe to step on by this much, metres.
    Clear(f64),
    /// At or under surveyed ground.
    Below(Contact),
    /// Low enough to meet ground the ship has not surveyed.
    Unsurveyed,
    /// Above every ground and rising, so that it meets none.
    Leaving,
}

/// Where a point of the line of sight is under surveyed ground: its foot, the cell, the survey's
/// level and the ground's height and gradient there.
#[derive(Clone, Copy)]
struct Contact {
    dir: [f64; 3],
    cell: PatchKey,
    level: SurveyLevel,
    ground: HeightSample,
}

impl<S: SurveyedGround> Marcher<'_, S> {
    /// The point `t` metres along the line of sight.
    #[must_use]
    fn point(&self, t: f64) -> [f64; 3] {
        [0, 1, 2].map(|k| self.origin[k] + t * self.direction[k])
    }

    /// What the march finds `t` metres along the line of sight.
    fn probe(&self, cache: &mut S::Cache, t: f64) -> Result<Probe, S::Error> {
        let (dir, height) = datum_foot(&self.figure, &self.point(t));
        if height > self.ceiling_m + FAR_MARGIN_M {
            // dh/dt; the height is convex along the line, so Newton's step lands at or above the
            // ceiling, and a rising line never comes down to it.
            let rate = dot(self.direction, self.figure.normal(dir));
            return Ok(if rate >= 0.0 {
                Probe::Leaving
            } else {
                Probe::Clear((height - self.ceiling_m) / -rate)
            });
        }
        match self.source.survey(dir) {
            Survey::NotHeld => Ok(Probe::Unsurveyed),
            Survey::Margin => Ok(
                match self.source.held_height(cache, dir, self.band_limit.band) {
                    Some(ground) if height > ground.height_m => Probe::Clear(
                        self.step(height - ground.height_m, self.band_limit.wavelength),
                    ),
                    _ => Probe::Unsurveyed,
                },
            ),
            Survey::Surveyed { cell, code } => {
                let level = survey_level(&self.figure, code);
                let ground = self.source.height(cache, dir, level.band)?;
                let clearance = height - ground.height_m;
                Ok(if clearance > 0.0 {
                    Probe::Clear(self.step(clearance, level.wavelength))
                } else {
                    Probe::Below(Contact {
                        dir,
                        cell,
                        level,
                        ground,
                    })
                })
            }
        }
    }

    /// The step over ground cleared by `clearance` metres at a level of shortest wavelength
    /// `wavelength`: the clearance over [`LIPSCHITZ`], at least a quarter of the wavelength and at
    /// most half a coarse cell.
    #[must_use]
    fn step(&self, clearance: f64, wavelength: Metres) -> f64 {
        num::min(
            num::max(clearance / LIPSCHITZ, 0.25 * wavelength.value()),
            self.cell_step_m,
        )
    }

    /// The first crossing between `above`, clear, and `below`, under the ground at `contact`, by
    /// bisection to [`RANGE_TOLERANCE_M`].
    fn refine(
        &self,
        cache: &mut S::Cache,
        mut above: f64,
        mut below: f64,
        mut contact: Contact,
    ) -> Result<LineOfSight, S::Error> {
        loop {
            let tolerance = num::max(RANGE_TOLERANCE_M, 4.0 * f64::EPSILON * below);
            let mid = f64::midpoint(above, below);
            if below - above <= tolerance || mid <= above || mid >= below {
                break;
            }
            match self.probe(cache, mid)? {
                Probe::Clear(_) | Probe::Leaving => above = mid,
                Probe::Below(c) => {
                    below = mid;
                    contact = c;
                }
                Probe::Unsurveyed => return Ok(LineOfSight::NotSurveyed),
            }
        }
        Ok(LineOfSight::Ground(self.range_at(below, contact)?))
    }

    /// The slant range `t` metres along the line of sight, under the ground at `contact`.
    fn range_at(&self, t: f64, contact: Contact) -> Result<SlantRange, S::Error> {
        let Contact {
            dir,
            cell,
            level,
            ground,
        } = contact;
        let normal = surface_normal(&self.figure, dir, ground.height_m, ground.gradient);
        let cos_slope = dot(normal, self.figure.normal(dir));
        let sin_grazing = num::max(-dot(self.direction, normal), 0.0);
        let sigma = if level.resolved {
            Some(Metres::ZERO)
        } else {
            let lambda = level.wavelength;
            let sigma_h = self.source.omitted_rms(cell, level.band)?.value();
            let lag = lambda.value();
            let v = self
                .source
                .omitted_structure(cell, level.band, lambda)?
                .value()
                / (lag * lag);
            // Grazing: no σ. A ray along the resolved surface grazes whatever its unresolved v.
            if sin_grazing < GRAZING_FACTOR * v.sqrt() || (sigma_h > 0.0 && sin_grazing <= 0.0) {
                None
            } else if sigma_h > 0.0 {
                Some(Metres::new(num::assert_finite(
                    sigma_h * cos_slope / sin_grazing,
                )))
            } else {
                Some(Metres::ZERO)
            }
        };
        Ok(SlantRange {
            range: Metres::new(num::assert_finite(t)),
            sigma,
            resolved: level.resolved,
            hit_m: self.point(t),
            at: xyz_to_face_uv(dir),
        })
    }
}

/// `a · b`.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `a × b`.
#[must_use]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests;
