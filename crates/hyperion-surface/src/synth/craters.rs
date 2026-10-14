//! The small craters: every crater narrower than the field's boundary diameter `D_b`, drawn per
//! diameter octave and band-limited level by level (plan R09, T7.b; Design notes 12, 13 and 15).
//!
//! # The octaves
//!
//! Octave k holds the craters from `D_b` ÷ 2^(k + 1) up to twice that ([`CraterOctave`]), from
//! k = 0, just under `D_b`, down to the one that straddles [`SMALLEST_DIAMETER`], the 2 m band
//! limit, which draws its whole octave and keeps the craters of 2 m and wider: 16 octaves on an
//! Earth, whose last runs from 1.3 m and keeps 2 to 2.6 m. Narrower craters are R11's decoration
//! (Design note 12). Each octave is drawn on one quadtree level, the deepest whose narrowest cell
//! is at least the octave's reach: no crater of the octave reaches farther than 2.543 of its lower
//! diameters ([`MAX_EXTERIOR_WIDTH`]), and that must fit in 0.98 of the level's narrowest width,
//! S2's `kMinWidth` of 2√2 ÷ 3 × 2⁻ⁿ radians for the quadratic warp (`s2metrics.cc`), taken as a
//! chord on the spheroid's smallest radius. A
//! crater drawn in a cell then reaches no point outside the cell's 3 × 3 neighbourhood, its edge
//! and corner neighbours across face edges by the cube's own rule ([`PatchKey::edge_neighbour`],
//! [`PatchKey::corner_neighbours`]), so a query searches the 3 × 3 about its own cell and finds a
//! crater that straddles a face edge from both faces. The levels deepen by one an octave.
//!
//! # The draw
//!
//! The density is the body's, one function for the whole body
//! ([`crate::field::FieldHeader::craters`]), so the draw reads no coarse cell, and the octave
//! search's cells, which may be wider than the coarse cells, add nothing to the synthesis margin
//! (Design note 15). An octave's expected count
//! per km² is the production between its diameters, N(>D) − N(>2D) by
//! [`cumulative_density`] (screened, through the diameter map), capped at Trask's saturation over
//! the same octave, 0.079 (D⁻² − (2D)⁻²) km⁻² ([`saturation`]): min(production, saturation), each
//! octave on its own. A cell's count is a Poisson draw ([`hyperion_base::rng::Stream::poisson`])
//! whose mean is that density times the cell's true area on the spheroid
//! ([`cell_area`]), on `surface.crater` keyed by the cell (instance 0). Its craters follow
//! in the same stream, each:
//!
//! - its centre, uniform in the spheroid's area over the cell, by rejection: words for s and t
//!   within the cell and a third that accepts with the area element over its bound on the cell
//!   ([`MAX_PLACEMENT_ATTEMPTS`] at most, near one on the fine cells most octaves use);
//! - a word for its diameter within the octave by the body's size–frequency distribution
//!   ([`diameter_in_octave`]);
//! - a word for its age;
//!
//! and the last octave drops the craters it draws under 2 m, so that it keeps the production's
//! share above 2 m of its density ([`CraterOctave::density`]).
//!
//! Its shape is the fresh [`crater_shape`] of its diameter on the body, so its morphology follows
//! the transition zone, and its exterior width a is its own (at most 1.543).
//!
//! # Ages
//!
//! A crater's age is uniform in the cumulative production over its octave's window τ: where the
//! octave is under saturation the window is the whole surface age S, and where it is capped only
//! the youngest share `N_sat` ÷ `N_prod` of the craters formed survive, so τ is the age at which
//! the lunar chronology counted from the system's formation has accumulated that share,
//! τ = `chronology_age`(N(S; A) × `N_sat` ÷ `N_prod`, A), and a crater drawn with the uniform u is
//! `chronology_age`(u × N(τ; A), A) ([`chronology_age`]), A the contract's system age (Design
//! note 12; science-r09-craters.md item 5). Plan 14's belt scaling multiplies N alike, so it
//! cancels.
//!
//! # The profile
//!
//! A crater's height is its fresh profile ([`CraterShape::relief`]) with its rim filleted and its
//! volume restored ([`CraterProfile`]; Design note 13): on [1 − w, 1 + w] rim radii the profile is
//! the cubic Hermite through the fresh profile's heights and slopes at both ends, which removes
//! the sharp rim crest, and a compensating term c (1 − (r ÷ r₁)²)² out to the crater's reach
//! r₁ = 1 + a puts back the volume the fillet took, so every crater integrates to zero at every
//! level. Both volumes are closed forms: the fresh profile's piece by piece
//! (`CraterShape::moment`), the Hermite's from its basis's first two moments, and the term's
//! c r₁² ÷ 3. The fillet's half-width w is the level's band ([`band_half_width`]), held to
//! min(1, a) rim radii so that the fillet stays between the centre and the reach. The central
//! cone's apex and base are not filleted.
//!
//! # Degradation
//!
//! The band is in the fillet and the age in the relief factor: diffusion over a crater's age,
//! K = κt = L² ÷ 2 with L = √(2κt) ([`diffusion_length`]), scales its whole profile by
//! g = (b ÷ (K ÷ R² + b))² with b = 0.15 and R the rim radius, Minton et al. 2019's depth law
//! (eq. 8) over its fresh depth ([`relief_factor`]). The scaling keeps the profile's zero volume
//! and its reach. Eq. 8 is the whole outcome of diffusion, the rim's rounding included, so the
//! fillet takes the band alone (main's ruling on R09.T7.b, 2026-10-13). Eq. 8 is fitted over
//! K ÷ r² from 0 to 0.3 on CTEM's crater profile, so a 300 m crater at 3 Gyr (K ÷ R² = 0.73) is
//! extrapolated beyond Minton et al.'s fitted range: it keeps 2.9% of its relief, against about 7%
//! in Fassett and Thomson 2014 and 5.2% under exact diffusion of this bowl, and a 1 km crater 48%,
//! against about 52% and 64%.
//!
//! # The closed forms
//!
//! The craters are a Poisson field of zero-mean profiles, so octave k's variance is its density
//! times its craters' mean ∫h² dA (Campbell's theorem), over its diameters and its window's ages,
//! each at full resolution its fresh profile times its relief factor ([`CraterOctave::variance`]).
//!
//! What a level of band w leaves out of the finest level's ground is a Poisson field too
//! (`decision-r10-t5-closed-forms.md` §2; R09.T7.h). The ground holds every crater filleted at the
//! band limit's half-width, `w₂` = 1 m, as g F(`w₂`), F(w) its fresh shape filleted over w
//! ([`CraterProfile::filleted`]) and g its relief factor; the level holds the craters of the
//! octaves it keeps as g F(w), and none of the others. So the difference is two independent
//! shot noises, each of zero mean, since every profile integrates to zero at every fillet:
//!
//! - an octave the level keeps adds its sharpening, its density times its craters' mean
//!   g² ∫ (F(`w₂`) − F(w))² dA over its diameters and its window's ages, `Φ_k`(w): the coarse rims'
//!   form ([`rims`], R09.T7.c) over a Poisson population;
//! - an octave it leaves out adds its ground variance, the same mean of g² ∫ F(`w₂`)² dA, `G_k`:
//!   its craters at the ground's fillet, not the fresh profile.
//!
//! Both are sums at the octave's own eight nodes in diameter, as its variance is, with the same
//! mean of g² over the ages, and the radial integrals are Gauss–Legendre on the profiles'
//! polynomial pieces, exact to rounding.
//!
//! The wavelength-keyed forms ([`CraterOctaves::unresolved_variance`],
//! [`CraterOctaves::structure_function`]) place each octave on the wavelengths of its diameters:
//! finer than a wavelength λ, the share `s_k` = clamp(ln(λ ÷ D₀) ÷ ln(D₁ ÷ D₀), 0, 1) of octave k,
//! D₀ to D₁ the diameters it keeps (log₂(λ ÷ D) for a whole octave from D), is left out, at
//! `G_k`, and the rest is kept and sharpened at half the wavelength, `Φ_k`(max(½ λ, `w₂`)). The
//! level-keyed forms, R10.T5's
//! ([`Synthesiser::omitted_variance`](super::Synthesiser::omitted_variance)), take the octaves the
//! level keeps at its band and the others whole. Both are zero at the band limit and below.
//!
//! The structure function at a lag b adds 2 `s_k` `G_k` (1 − J₀(2π b ÷ `λ_k`)) for an octave's
//! share left out, `λ_k` = √(D₀ D₁) the geometric mean of its diameters, as R10's bands do; and for
//! its share kept, the rims' 2Φ (1 − exp(−S b² ÷ 2Φ)), with S its sharpening's slopes' mean square
//! along a random direction, the same sum over ½ ∫ |∇(F(`w₂`) − F(w))|² dA. The sharpening lies
//! mostly on an annulus about the fillet's width whatever the crater's size, so its spectrum is
//! near the band, not near D, though the term that restores the fillet's volume spreads part of it
//! over the whole reach; the form's small-lag limit, S b², and its long-lag one, 2Φ, are both
//! exact.
//!
//! # The coarse craters
//!
//! The craters of `D_b` and wider are the coarse field's list, smoothed into the coarse elevation
//! at the coarse cells' resolution; [`rims`] sharpens their rims at finer levels with the same
//! profile (R09.T7.c).
//!
//! # Determinism
//!
//! Every value is a function of the field's header, the detail seed, the direction and the level:
//! the craters of a cell come from its own stream, every sum runs in a fixed order (octaves from
//! the largest, the 3 × 3 cells by key, a cell's craters in draw order), and every transcendental
//! is `hyperion_base::math`'s. [`CraterCache`] is the caller's and moves no result.

pub mod rims;

use std::collections::BTreeMap;

use hyperion_base::math;
use hyperion_base::rng::{DetailSeed, ObjectKey, Stream, Threshold};
use hyperion_base::units::{Gigayears, Metres, PerSquareKilometre, SquareMetres};

use super::{BandLevel, LevelSet};
use crate::craters::chronology::{EXPONENTIAL_COEFFICIENT, EXPONENTIAL_RATE, LINEAR_RATE};
use crate::craters::density::GAUSS_LEGENDRE_8;
use crate::craters::{
    CraterParams, CraterShape, DiameterOctave, SOLAR_SYSTEM_AGE, UnitUniform, chronology_age,
    crater_shape, cumulative_density, diameter_in_octave, lunar_chronology, saturation,
};
use crate::cube::{Edge, Face, FaceUv, MAX_LEVEL, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use crate::field::{FieldHeader, FieldView};
use crate::geometry::{BAND_LIMIT_M, min_rate, vertex_spacing};
use crate::height::HeightSample;
use crate::num;
use crate::spheroid::Spheroid;
use crate::tags::SURFACE_CRATER;

/// The smallest crater the synthesis keeps, metres: the 2 m band limit ([`BAND_LIMIT_M`]). The
/// last octave straddles it and keeps the craters it draws from it up; narrower craters are R11's
/// decoration, 1–2 m craters being typically 0.05–0.1 m deep (Design note 12).
pub const SMALLEST_DIAMETER: Metres = Metres::new(BAND_LIMIT_M);

/// The fillet half-width of the finest level's ground, `w₂`: ½ [`BAND_LIMIT_M`], 1 m, the
/// [`band_half_width`] of every level from the band limit's, against which the closed forms count
/// what a level leaves out (the module documentation).
const GROUND_FILLET: Metres = Metres::new(0.5 * BAND_LIMIT_M);

/// The widest exterior of any crater shape, in rim radii: 1.543, just above the fresh bowl's
/// 1.5426 (d ÷ h = 5.42), which bounds every shape's a (R09.T7.a and T7.e hold it from 10⁻⁴ to
/// 100 `D_t`, and airless, porous and larger craters' a are smaller), so that no crater reaches
/// beyond 2.543 rim radii (Design note 13).
pub const MAX_EXTERIOR_WIDTH: f64 = 1.543;

/// The topographic diffusivity κ that degrades small craters, m² Myr⁻¹: 5.5, Fassett and Thomson
/// 2014's for the lunar maria (JGR Planets 119(10), 2255–2271, doi:10.1002/2014JE004698; abstract:
/// "The average diffusivity we calculate over the past 3 Ga is ~5.5 m2/Myr", from craters of 800 m
/// to 5 km), in ∂h ÷ ∂t = κ∇²h (Minton et al. 2019, Icarus 326, 63, eq. 4).
///
/// It is taken beyond its calibration three ways: down to 2 m craters, 2.5 decades below it; held
/// constant over a system's whole age, where a rate that follows the impact flux would give a
/// 4 Gyr crater many times the degradation; and for every body, where weathering, wind and water
/// differ. Nor is it calibrated against the slopes of cratered ground. Low confidence (plan R09,
/// Risks, "Deferred findings (2026-10-13 rule)").
pub const DIFFUSIVITY_M2_PER_MYR: f64 = 5.5;

/// The share of a level's narrowest cell width that an octave's reach may take: 0.98, a safety
/// margin alone, since S2's `kMinWidth` bounds the cells' widths at every level (`s2metrics.h`:
/// "for a cell at level k, the distance between opposite edges is at least
/// kMinWidth.GetValue(k)"); the real narrowest widths exceed it, by about 0.4% at level 8 and
/// more at coarser levels (R09.T7.b's science check).
const SEARCH_MARGIN: f64 = 0.98;

/// The most candidates a crater's placement draws before it takes the last: 64. A candidate is
/// accepted with the area element over its bound: more than a quarter of the time on a level-1
/// cell, four in five from level 5 and nine in ten from level 12 on a figure of flattening 0.075
/// (tested), so the cap is never reached in practice; it bounds the words a cell can consume.
pub const MAX_PLACEMENT_ATTEMPTS: u32 = 64;

/// The most octaves a body has: 32, from just under `D_b` by halvings. `D_b` is at most a few
/// hundred kilometres, so the band limit is reached long before.
const MAX_OCTAVES: u8 = 32;

/// The most cells a [`CraterCache`] holds before it starts afresh: 16,384, a few bakes' worth.
const CACHE_CELLS: usize = 16_384;

/// The width over which a level filters a crater's rim, half the shortest wavelength the level
/// holds: ½ max(2 m, 4 s) with s the level's largest vertex spacing on the figure's equatorial
/// radius ([`vertex_spacing`]), R05's rule for the octaves a level evaluates (the test planet's
/// "lattice spacing at least four times the level's largest vertex spacing", never finer than the
/// 2 m band limit).
///
/// One metre at an Earth's finest level, 19; about 10 km at level 5.
#[must_use]
pub fn band_half_width(figure: &Spheroid, level: BandLevel) -> Metres {
    let spacing = vertex_spacing(figure.equatorial_radius_m, level.get()).max_m;
    Metres::new(0.5 * num::max(BAND_LIMIT_M, 4.0 * spacing))
}

/// The diffusion length L = √(2κt) of a crater `age` old under the diffusivity
/// [`DIFFUSIVITY_M2_PER_MYR`]: the standard deviation of the Gaussian that topographic diffusion
/// over t convolves the fresh profile with. About 200 m at 3.8 Gyr.
///
/// # Panics
///
/// If `age` is not finite and non-negative.
#[must_use]
pub fn diffusion_length(age: Gigayears) -> Metres {
    let t = age.value();
    assert!(
        t.is_finite() && t >= 0.0,
        "a crater's age must be finite and non-negative, got {t} Gyr"
    );
    Metres::new((2.0 * DIFFUSIVITY_M2_PER_MYR * (t * 1e3)).sqrt())
}

/// Minton et al. 2019's b in their degraded crater's depth, eq. 8: "We found that the value of d/D
/// as a function K could be approximated by: d/D = a²/(K/r² + b)², (8) where a = 0.07 and b =
/// 0.15" (Icarus 326, 63–87; arXiv:1902.07746, pp. 15–16), with K = ∫κ dt (their eq. 5) and r the
/// crater's radius, and "the coefficient b in this fit is equivalent to a(d/D)_initial^−1/2", the
/// fresh 0.218 (p. 16). The relief factor is the depth over the fresh one, (b ÷ (K ÷ r² + b))².
/// Fitted over K ÷ r² from 0 to 0.3 on a CTEM crater of Fassett and Thomson 2014's shape, and
/// extrapolated beyond that range (R09.T7.b's science check).
pub const DEGRADATION_B: f64 = 0.15;

/// The share of its fresh relief a crater of rim radius `rim_radius` keeps after diffusion over a
/// length `diffusion` ([`diffusion_length`]): g = (b ÷ (K ÷ R² + b))² with K = κt = L² ÷ 2 and
/// b = [`DEGRADATION_B`], Minton et al. 2019's depth law (eq. 8) over its fresh depth, so that g
/// is 1 for a fresh crater and falls as κt grows against R².
///
/// A rim fillet rounds a crater's crest but leaves its floor and, once held at the crater's own
/// size, most of its relief: a 300 m bowl kept 55% of its relief at 3 Gyr by a fillet over its
/// diffusion alone, against 5.2% under exact two-dimensional diffusion of the same bowl and about
/// 7% in Fassett and Thomson 2014's abstract ("a 300 m diameter crater is reduced to only ~7% of
/// its initial depth"). So the whole profile is scaled by g, which keeps its zero volume and its
/// reach, and the fillet takes the level's band alone (main's ruling on R09.T7.b, 2026-10-13). g
/// keeps 2.9% for that crater (K ÷ R² = 0.73, extrapolated beyond Minton et al.'s fitted range)
/// and 48% for a 1 km one (0.066; Fassett and Thomson's ~52%).
///
/// # Panics
///
/// If `rim_radius` is not finite and positive, or `diffusion` is not finite and non-negative.
#[must_use]
pub fn relief_factor(rim_radius: Metres, diffusion: Metres) -> f64 {
    let (r, l) = (rim_radius.value(), diffusion.value());
    assert!(
        r.is_finite() && r > 0.0 && l.is_finite() && l >= 0.0,
        "a rim radius must be finite and positive and a diffusion length finite and \
         non-negative, got {r} m and {l} m"
    );
    let k_over_r2 = 0.5 * (l / r) * (l / r);
    let g = DEGRADATION_B / (k_over_r2 + DEGRADATION_B);
    g * g
}

/// A crater's band-limited, degraded profile: its fresh shape with the rim filleted over a
/// half-width, the volume the fillet removes put back by a compensating term (Design note 13),
/// and the whole scaled by the relief its diffusive age leaves ([`relief_factor`]; the module
/// documentation).
///
/// It integrates to zero over the crater, as the fresh shape does, whatever the fillet and the
/// relief, and reaches no farther. Built once per crater, band and age, it is evaluated at any
/// distance by [`at`](Self::at).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterProfile {
    shape: CraterShape,
    /// The share of its relief the crater keeps, g, in [0, 1].
    relief: f64,
    /// The fillet's half-width in rim radii, at most min(1, a); zero for none.
    fillet: f64,
    /// The fresh height, metres, and slope, metres per rim radius, at 1 − w.
    inner: (f64, f64),
    /// The fresh height and slope at 1 + w.
    outer: (f64, f64),
    /// The compensating term's amplitude c, metres.
    compensation_m: f64,
}

impl CraterProfile {
    /// The profile of `shape` at a level of band half-width `band` ([`band_half_width`]) for a
    /// crater of diffusion length `diffusion` ([`diffusion_length`]): the band in the fillet and
    /// the age in the relief, its rim filleted over `band`, held to min(1, a) rim radii, and its
    /// whole relief scaled by [`relief_factor`] of its rim radius and the diffusion. The
    /// synthesis's craters (R09.T7.b) and the coarse craters' sharpening (T7.c) take it.
    ///
    /// # Panics
    ///
    /// If `band` or `diffusion` is not finite and non-negative.
    #[must_use]
    pub fn new(shape: CraterShape, band: Metres, diffusion: Metres) -> Self {
        let rim = shape.diameter() * 0.5;
        Self {
            relief: relief_factor(rim, diffusion),
            ..Self::filleted(shape, band)
        }
    }

    /// The profile of `shape` with its rim filleted over the half-width `fillet_half_width`,
    /// metres, held to min(1, a) rim radii, and its whole relief kept: the fillet alone.
    ///
    /// # Panics
    ///
    /// If `fillet_half_width` is not finite and non-negative.
    #[must_use]
    pub fn filleted(shape: CraterShape, fillet_half_width: Metres) -> Self {
        let w_m = fillet_half_width.value();
        assert!(
            w_m.is_finite() && w_m >= 0.0,
            "a fillet's half-width must be finite and non-negative, got {w_m} m"
        );
        let a = shape.exterior_width();
        let held = num::min(1.0, a);
        let fillet = num::min(w_m / (0.5 * shape.diameter().value()), held);
        if fillet <= 0.0 {
            return Self {
                shape,
                relief: 1.0,
                fillet: 0.0,
                inner: (0.0, 0.0),
                outer: (0.0, 0.0),
                compensation_m: 0.0,
            };
        }
        let (x0, x1) = (1.0 - fillet, 1.0 + fillet);
        let inner = shape.relief_and_slope(x0);
        let outer = shape.relief_and_slope(x1);
        let change = hermite_moment(x0, x1 - x0, inner, outer) - shape.moment(x0, x1);
        let reach = 1.0 + a;
        Self {
            shape,
            relief: 1.0,
            fillet,
            inner,
            outer,
            compensation_m: -3.0 * change / (reach * reach),
        }
    }

    /// The fresh shape.
    #[must_use]
    pub const fn shape(&self) -> &CraterShape {
        &self.shape
    }

    /// The fillet's half-width as held, metres: at most min(1, a) rim radii.
    #[must_use]
    pub fn fillet_half_width(&self) -> Metres {
        self.shape.diameter() * (0.5 * self.fillet)
    }

    /// The compensating term's amplitude c, metres, before the relief factor: its height at the
    /// centre, positive where the fillet took volume from the rim.
    #[must_use]
    pub const fn compensation(&self) -> Metres {
        Metres::new(self.compensation_m)
    }

    /// The share of its relief the crater keeps, g ([`relief_factor`]): 1 for the fillet alone.
    #[must_use]
    pub const fn relief_factor(&self) -> f64 {
        self.relief
    }

    /// The profile's height and its slope along the radius at `r_over_radius` rim radii from the
    /// centre: zero at and beyond the reach.
    ///
    /// # Panics
    ///
    /// If `r_over_radius` is not finite and non-negative.
    #[must_use]
    pub fn at(&self, r_over_radius: f64) -> ProfileSample {
        let (height_m, slope_m_per_rim_radius) = self.height_and_slope(r_over_radius);
        ProfileSample {
            height_m,
            slope_m_per_rim_radius,
        }
    }

    /// [`at`](Self::at) as a pair: the height, metres, and the slope, metres per rim radius.
    #[must_use]
    fn height_and_slope(&self, r_over_radius: f64) -> (f64, f64) {
        let x = r_over_radius;
        assert!(
            x.is_finite() && x >= 0.0,
            "a distance from a crater's centre must be finite and non-negative, got {x}"
        );
        if self.fillet <= 0.0 {
            let (height, slope) = self.shape.relief_and_slope(x);
            return (self.relief * height, self.relief * slope);
        }
        let (x0, x1) = (1.0 - self.fillet, 1.0 + self.fillet);
        let (mut height, mut slope) = if x > x0 && x < x1 {
            hermite(x0, x1 - x0, self.inner, self.outer, x)
        } else {
            self.shape.relief_and_slope(x)
        };
        let reach = 1.0 + self.shape.exterior_width();
        if x < reach {
            let q = x / reach;
            let b = 1.0 - q * q;
            height += self.compensation_m * b * b;
            slope += -4.0 * self.compensation_m * q * b / reach;
        }
        (self.relief * height, self.relief * slope)
    }

    /// The breakpoints of the profile's polynomial pieces in rim radii, ascending from 0 to the
    /// reach, some perhaps repeated: within each consecutive pair the height is one polynomial of
    /// degree at most 4.
    #[must_use]
    fn breakpoints(&self) -> [f64; 7] {
        let reach = 1.0 + self.shape.exterior_width();
        let clip = |x: f64| num::min(num::max(x, 0.0), reach);
        let mut points = [
            0.0,
            clip(self.shape.peak_fraction()),
            clip(self.shape.floor_fraction()),
            clip(1.0 - self.fillet),
            clip(1.0),
            clip(1.0 + self.fillet),
            reach,
        ];
        points.sort_by(f64::total_cmp);
        points
    }

    /// ∫ h(x)² 2x dx over the crater, square metres: its ∫h² dA over π R², by eight-point
    /// Gauss–Legendre on each polynomial piece, exact to rounding for pieces of degree at most 15
    /// (h² x is at most 9).
    #[must_use]
    fn square_moment(&self) -> f64 {
        let points = self.breakpoints();
        let mut total = 0.0;
        for pair in points.windows(2) {
            let (lo, hi) = (pair[0], pair[1]);
            if hi <= lo {
                continue;
            }
            let half = 0.5 * (hi - lo);
            let mid = f64::midpoint(lo, hi);
            let mut sum = 0.0;
            for (node, weight) in GAUSS_LEGENDRE_8 {
                let x = mid + half * node;
                let h = self.height_and_slope(x).0;
                sum += weight * h * h * 2.0 * x;
            }
            total += half * sum;
        }
        total
    }
}

/// A band-limited profile's height and slope at a distance from the crater's centre
/// ([`CraterProfile::at`], [`profile`]).
///
/// Plain data with public fields, as [`HeightSample`] is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfileSample {
    /// The height above the surrounding surface, metres.
    pub height_m: f64,
    /// The height's derivative along the radius, metres per rim radius: divide by the rim radius
    /// in metres for metres per metre.
    pub slope_m_per_rim_radius: f64,
}

/// The band-limited, degraded profile of `crater` at a level of band half-width `band` for a
/// crater of diffusion length `diffusion`, at `r_over_radius` rim radii from the centre: its
/// height and slope ([`CraterProfile::new`]; Provides' `profile`, with the band given as its
/// half-width and the crater's age as its diffusion length, since a band level alone names no
/// width in metres).
///
/// # Panics
///
/// If `r_over_radius`, `band` or `diffusion` is not finite and non-negative.
#[must_use]
pub fn profile(
    r_over_radius: f64,
    crater: &CraterShape,
    band: Metres,
    diffusion: Metres,
) -> ProfileSample {
    CraterProfile::new(*crater, band, diffusion).at(r_over_radius)
}

/// The cubic Hermite on [`x0`, `x0` + `span`] through the heights and slopes `from` and `to` at its
/// ends, at `x`: its height and slope.
#[must_use]
fn hermite(x0: f64, span: f64, from: (f64, f64), to: (f64, f64), x: f64) -> (f64, f64) {
    let t = (x - x0) / span;
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    let height = from.0 * h00 + span * from.1 * h10 + to.0 * h01 + span * to.1 * h11;
    let d00 = 6.0 * t2 - 6.0 * t;
    let d10 = 3.0 * t2 - 4.0 * t + 1.0;
    let d11 = 3.0 * t2 - 2.0 * t;
    let slope = (from.0 - to.0) * d00 / span + from.1 * d10 + to.1 * d11;
    (height, slope)
}

/// ∫ H(x) 2x dx of the cubic Hermite of [`hermite`] over its span: with x = x₀ + Δt, the basis's
/// moments ∫ h = (½, 1/12, ½, −1/12) and ∫ t h = (3/20, 1/30, 7/20, −1/20) give 2Δ [x₀ (f₀ ÷ 2 +
/// Δ m₀ ÷ 12 + f₁ ÷ 2 − Δ m₁ ÷ 12) + Δ (3f₀ ÷ 20 + Δ m₀ ÷ 30 + 7f₁ ÷ 20 − Δ m₁ ÷ 20)].
#[must_use]
fn hermite_moment(x0: f64, span: f64, from: (f64, f64), to: (f64, f64)) -> f64 {
    let (f0, m0) = from;
    let (f1, m1) = to;
    let zeroth = 0.5 * f0 + span * m0 / 12.0 + 0.5 * f1 - span * m1 / 12.0;
    let first = 0.15 * f0 + span * m0 / 30.0 + 0.35 * f1 - span * m1 / 20.0;
    2.0 * span * (x0 * zeroth + span * first)
}

/// One diameter octave of a body's small craters: its diameters, its search level, its density and
/// its window of ages (the module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterOctave {
    index: u8,
    diameters: DiameterOctave,
    smallest: Metres,
    kept: f64,
    level: u8,
    production: PerSquareKilometre,
    saturation: PerSquareKilometre,
    window: Gigayears,
    window_density: PerSquareKilometre,
    variance: SquareMetres,
    /// Its variance at the ground's fillet, `G_k`, square metres, by the same nodes as the
    /// variance (R09.T7.h).
    ground_variance: SquareMetres,
    /// The mean area its kept craters' reaches cover, π ((1 + a) R)², square metres, by the same
    /// nodes as the variance (R09.T8's level bound).
    reach_area_m2: f64,
}

impl CraterOctave {
    /// The octave's index k, 0 for the largest, just under `D_b`.
    #[must_use]
    pub const fn index(&self) -> u8 {
        self.index
    }

    /// Its diameters, from `D_b` ÷ 2^(k + 1) up to twice that, which its cells draw.
    #[must_use]
    pub const fn diameters(&self) -> DiameterOctave {
        self.diameters
    }

    /// The smallest diameter it keeps: its lower diameter, but [`SMALLEST_DIAMETER`] for the last
    /// octave, which straddles it and keeps only the craters of 2 m and wider that it draws.
    #[must_use]
    pub const fn smallest(&self) -> Metres {
        self.smallest
    }

    /// The quadtree level its craters are drawn on: the deepest whose narrowest cell holds its
    /// reach.
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }

    /// The body's production between its diameters, N(>D) − N(>2D) per km², screened and through
    /// the diameter map ([`cumulative_density`]).
    #[must_use]
    pub const fn production(&self) -> PerSquareKilometre {
        self.production
    }

    /// Trask's saturation between its diameters, 0.079 (D⁻² − (2D)⁻²) per km² ([`saturation`]).
    #[must_use]
    pub const fn saturation(&self) -> PerSquareKilometre {
        self.saturation
    }

    /// The craters per km² it draws, min(production, saturation): its cells' Poisson mean per
    /// area.
    #[must_use]
    pub fn drawn_density(&self) -> PerSquareKilometre {
        PerSquareKilometre::new(num::min(self.production.value(), self.saturation.value()))
    }

    /// The craters per km² it keeps: [`drawn_density`](Self::drawn_density), times the
    /// production's share from [`smallest`](Self::smallest) up for the last octave,
    /// (N(>2 m) − N(>2D)) ÷ (N(>D) − N(>2D)).
    #[must_use]
    pub fn density(&self) -> PerSquareKilometre {
        self.drawn_density() * self.kept
    }

    /// Whether saturation caps it.
    #[must_use]
    pub fn is_saturated(&self) -> bool {
        self.production.value() > self.saturation.value()
    }

    /// The window τ its craters' ages are drawn from: the surface age, or under saturation the age
    /// at which the chronology has accumulated the surviving share.
    #[must_use]
    pub const fn window(&self) -> Gigayears {
        self.window
    }

    /// Its closed-form variance, square metres: its kept density times its kept craters' mean
    /// ∫h² dA at full resolution, each its fresh profile times its relief factor, over its kept
    /// diameters, by the octave's own inversion, and its window's ages, uniform in the production
    /// (the module documentation).
    #[must_use]
    pub const fn variance(&self) -> SquareMetres {
        self.variance
    }

    /// Its variance at the ground's fillet, `G_k`, square metres: [`variance`](Self::variance) with
    /// each crater filleted over [`GROUND_FILLET`], 1 m, in place of its fresh profile; what an
    /// octave a level leaves out adds to what the level leaves out (the module documentation).
    #[must_use]
    pub(crate) const fn ground_variance(&self) -> SquareMetres {
        self.ground_variance
    }
}

/// One of an octave's eight nodes in diameter, at which its closed forms are summed (the module
/// documentation): the crater of that diameter at the ground's fillet and its weight.
#[derive(Debug, Clone, Copy, PartialEq)]
struct OctaveNode {
    /// The node's crater, its fresh shape filleted over [`GROUND_FILLET`].
    ground: CraterProfile,
    /// The node's Gauss–Legendre weight in the octave's mean over its diameters, ½ ω, times the
    /// crater's disc of rim radius R, π R², square metres.
    disc_m2: f64,
    /// The mean of g² over the octave's window for the node's rim radius
    /// ([`mean_square_relief`]).
    relief2: f64,
}

/// An octave's sharpening at a level's band (the module documentation): what the octave adds to
/// what the level leaves out of the ground where the level keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Sharpening {
    /// Its variance, `Φ_k`, square metres.
    pub(crate) variance_m2: f64,
    /// Its slopes' mean square along a random direction, `S_k`, square metres per square metre.
    pub(crate) slope2: f64,
}

/// One crater's sharpening from a fillet of half-width `band` to the ground's ([`GROUND_FILLET`]),
/// before its relief factor: ∫ (F(`w₂`) − F(w))² 2x dx, square metres, and
/// ∫ (F′(`w₂`) − F′(w))² 2x dx, square metres per square rim radius, over its reach in rim radii,
/// with F(w) its fresh shape filleted over w ([`CraterProfile::filleted`]) and `ground` that shape
/// filleted over the ground's, F(`w₂`).
///
/// Both are zero, to the bit, where the two fillets are one: a band no wider than the ground's,
/// or both held at min(1, a) rim radii. The integrals are the coarse rims' ([`rims`]), exact to
/// rounding on the profiles' polynomial pieces.
///
/// # Panics
///
/// If `band` is not finite and non-negative.
#[must_use]
fn sharpening_moments(ground: &CraterProfile, band: Metres) -> (f64, f64) {
    let level = CraterProfile::filleted(ground.shape, band);
    // Neither fillet is a NaN, since both bands are finite.
    if level.fillet <= ground.fillet {
        return (0.0, 0.0);
    }
    rims::difference_moments(ground, &level)
}

/// What a body's small craters are drawn from, beyond the detail seed: what a cache's craters
/// were made by.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Inputs {
    params: CraterParams,
    figure: Spheroid,
    boundary_diameter: Metres,
    surface_age: Gigayears,
}

/// A body's crater octaves, a function of its crater contract, figure, boundary diameter and
/// surface age alone (the module documentation): what the draw and the closed forms read.
#[derive(Debug, Clone, PartialEq)]
pub struct CraterOctaves {
    inputs: Inputs,
    octaves: Vec<CraterOctave>,
    /// Each octave's nodes, by index: none for an octave of no density.
    nodes: Vec<Vec<OctaveNode>>,
}

impl CraterOctaves {
    /// The octaves of the body of `header`: [`for_body`](Self::for_body) of its crater contract,
    /// figure, boundary diameter and surface age.
    ///
    /// # Panics
    ///
    /// As [`for_body`](Self::for_body).
    #[must_use]
    pub fn new(header: &FieldHeader) -> Self {
        Self::for_body(
            *header.craters(),
            header.figure(),
            header.boundary_diameter(),
            header.surface_age(),
        )
    }

    /// The octaves of a body of crater contract `params` and figure `figure`, whose craters of
    /// `boundary_diameter` and wider are the coarse field's, on a surface `surface_age` old: what
    /// the coarse pass reads before it has a header (R09.T12.e books the craters' variance).
    ///
    /// # Panics
    ///
    /// If `boundary_diameter` is not finite and positive, the surface age is not finite and
    /// non-negative or is older than the contract's system ([`lunar_chronology`]), an octave's
    /// reach exceeds even a whole face's
    /// narrowest width, which no figure of flattening under about 0.8 reaches, or two octaves
    /// would share the deepest level, 24, which only a body of a radius beyond about 2 × 10⁸ m
    /// reaches.
    #[must_use]
    pub fn for_body(
        params: CraterParams,
        figure: Spheroid,
        boundary_diameter: Metres,
        surface_age: Gigayears,
    ) -> Self {
        let d_b = boundary_diameter.value();
        assert!(
            d_b.is_finite() && d_b > 0.0,
            "a boundary diameter must be finite and positive, got {d_b} m"
        );
        let inputs = Inputs {
            params,
            figure,
            boundary_diameter,
            surface_age,
        };
        let system = params.system_age();
        let surface_density = lunar_chronology(surface_age, system);
        let smallest_radius_m = num::min(figure.equatorial_radius_m, figure.polar_radius_m);
        let mut octaves: Vec<CraterOctave> = Vec::new();
        let mut nodes: Vec<Vec<OctaveNode>> = Vec::new();
        let mut lower_m = d_b;
        for index in 0..MAX_OCTAVES {
            lower_m *= 0.5;
            // Down to the octave that straddles the band limit, which keeps its craters from it up.
            if 2.0 * lower_m <= SMALLEST_DIAMETER.value() {
                break;
            }
            let diameters = DiameterOctave::new(Metres::new(lower_m))
                .expect("an octave under a boundary diameter is a valid octave");
            let (lower, upper) = (diameters.lower(), diameters.upper());
            let smallest = if lower.value() < SMALLEST_DIAMETER.value() {
                SMALLEST_DIAMETER
            } else {
                lower
            };
            let above = |d: Metres| cumulative_density(&params, d).value();
            let (above_lower, above_smallest, above_upper) =
                (above(lower), above(smallest), above(upper));
            let production = above_lower - above_upper;
            let kept = if production > 0.0 {
                (above_smallest - above_upper) / production
            } else {
                1.0
            };
            let capped = saturation(lower).value() - saturation(upper).value();
            let (window, window_density) = if production > capped {
                let survivors = surface_density * (capped / production);
                (chronology_age(survivors, system), survivors)
            } else {
                (surface_age, surface_density)
            };
            let level = search_level((1.0 + MAX_EXTERIOR_WIDTH) * lower_m, smallest_radius_m);
            // The level names the octave in its cells' keys and streams, so no two may share one.
            assert!(
                octaves.last().is_none_or(|previous| previous.level < level),
                "two crater octaves on one quadtree level, {level}"
            );
            let mut octave = CraterOctave {
                index,
                diameters,
                smallest,
                kept,
                level,
                production: PerSquareKilometre::new(production),
                saturation: PerSquareKilometre::new(capped),
                window,
                window_density,
                variance: SquareMetres::ZERO,
                ground_variance: SquareMetres::ZERO,
                reach_area_m2: 0.0,
            };
            let moments = octave_moments(&params, &octave);
            octave.variance = moments.variance;
            octave.ground_variance = moments.ground_variance;
            octave.reach_area_m2 = moments.reach_area_m2;
            octaves.push(octave);
            nodes.push(moments.nodes);
        }
        Self {
            inputs,
            octaves,
            nodes,
        }
    }

    /// The octaves, from the largest.
    #[must_use]
    pub fn octaves(&self) -> &[CraterOctave] {
        &self.octaves
    }

    /// The craters' variance in wavelengths shorter than `finer_than`, square metres (the module
    /// documentation): over the octaves from the largest, Σ [`s_k` `G_k` + (1 − `s_k`) `Φ_k`(w)],
    /// the share `s_k` = clamp(ln(λ ÷ D₀) ÷ ln(D₁ ÷ D₀), 0, 1) of octave k's kept diameters D₀ to
    /// D₁ below λ counted at its ground variance `G_k`, and the rest at its sharpening from
    /// w = max(½ λ, 1 m) to the ground's 1 m fillet. Zero at the 2 m band limit and below, and
    /// non-decreasing in λ (tested).
    ///
    /// # Panics
    ///
    /// If `finer_than` is not finite and positive.
    #[must_use]
    pub fn unresolved_variance(&self, finer_than: Metres) -> SquareMetres {
        let w = wavelength(finer_than);
        self.variance_with(wavelength_band(w), |octave| band_share(w, octave))
    }

    /// The craters' structure function at `lag` in wavelengths shorter than `finer_than`, square
    /// metres (the module documentation): over the octaves, Σ [`s_k` 2 `G_k` (1 − J₀(2π b ÷
    /// `λ_k`)) + (1 − `s_k`) 2 `Φ_k` (1 − exp(−`S_k` b² ÷ 2 `Φ_k`))], with `s_k`, `G_k` and `Φ_k` as in [`unresolved_variance`](Self::unresolved_variance), `λ_k`
    /// the geometric mean of the diameters octave k keeps, √2 times its lower for a whole octave,
    /// and `S_k` its sharpening's slopes' mean square along a random direction. Zero at no lag, and
    /// twice the unresolved variance far beyond the octaves' diameters.
    ///
    /// # Panics
    ///
    /// If `finer_than` is not finite and positive, or `lag` is not finite and non-negative.
    #[must_use]
    pub fn structure_function(&self, finer_than: Metres, lag: Metres) -> SquareMetres {
        let w = wavelength(finer_than);
        assert_lag(lag);
        self.structure_with(wavelength_band(w), lag, |octave| band_share(w, octave))
    }

    /// The craters' part of what band level `set.level()` leaves out of the finest level's ground,
    /// square metres: R10.T5's level-keyed form (`decision-r10-t5-closed-forms.md` §2; R09.T7.h),
    /// Σ `Φ_k`(`w_n`) over the octaves the level keeps, through [`LevelSet::craters_through`],
    /// sharpened from its band `w_n` ([`band_half_width`]) to the ground's, plus Σ `G_k` over the
    /// octaves it leaves out, whole at the ground's fillet. Zero from the band limit's level.
    #[must_use]
    pub(crate) fn omitted_variance(&self, set: &LevelSet) -> SquareMetres {
        let band = band_half_width(&self.inputs.figure, set.level());
        self.variance_with(band, |octave| left_out_by(set, octave))
    }

    /// The structure function at `lag` of [`omitted_variance`](Self::omitted_variance)'s ground,
    /// square metres: 2 `Φ_k` (1 − exp(−`S_k` b² ÷ 2 `Φ_k`)) for each octave the level keeps and
    /// 2 `G_k` (1 − J₀(2π b ÷ `λ_k`)) for each it leaves out, summed from the largest.
    ///
    /// # Panics
    ///
    /// If `lag` is not finite and non-negative.
    #[must_use]
    pub(crate) fn omitted_structure_function(&self, set: &LevelSet, lag: Metres) -> SquareMetres {
        assert_lag(lag);
        let band = band_half_width(&self.inputs.figure, set.level());
        self.structure_with(band, lag, |octave| left_out_by(set, octave))
    }

    /// Octave `k`'s sharpening at a level of band half-width `band` (the module documentation):
    /// its density per m² times, over its nodes, ½ ω π R² E[g²] times the crater's
    /// [`sharpening_moments`], the slopes' moment per square metre and halved for a random
    /// direction. Zero for an octave of no density, and at a band no wider than the ground's.
    ///
    /// # Panics
    ///
    /// If the body has no octave `k`; or, for an octave of any density, if `band` is not finite and
    /// non-negative.
    #[must_use]
    pub(crate) fn sharpening(&self, k: usize, band: Metres) -> Sharpening {
        let density_per_m2 = self.octaves[k].density().value() * 1e-6;
        let (mut square, mut slope) = (0.0, 0.0);
        for node in &self.nodes[k] {
            let (heights, slopes) = sharpening_moments(&node.ground, band);
            let rim_m = 0.5 * node.ground.shape.diameter().value();
            square += node.disc_m2 * heights * node.relief2;
            slope += node.disc_m2 * (0.5 * slopes / (rim_m * rim_m)) * node.relief2;
        }
        Sharpening {
            variance_m2: num::assert_finite(density_per_m2 * square),
            slope2: num::assert_finite(density_per_m2 * slope),
        }
    }

    /// The variance of the craters' ground that a band half-width `band` leaves out, with the
    /// share `left_out(octave)` of each octave left out, at its ground variance, and the rest
    /// kept, at its sharpening at `band`: the octaves' terms summed from the largest, each share
    /// left out before the share kept.
    #[must_use]
    fn variance_with(&self, band: Metres, left_out: impl Fn(&CraterOctave) -> f64) -> SquareMetres {
        let mut variance = 0.0;
        for (k, octave) in self.octaves.iter().enumerate() {
            let share = left_out(octave);
            if share > 0.0 {
                variance += share * octave.ground_variance().value();
            }
            if share < 1.0 {
                variance += (1.0 - share) * self.sharpening(k, band).variance_m2;
            }
        }
        SquareMetres::new(variance)
    }

    /// The structure function at `lag` of [`variance_with`](Self::variance_with)'s ground, summed
    /// in its order: each share left out on its octave's J₀ ring, each share kept by the rims'
    /// form.
    #[must_use]
    fn structure_with(
        &self,
        band: Metres,
        lag: Metres,
        left_out: impl Fn(&CraterOctave) -> f64,
    ) -> SquareMetres {
        let b = lag.value();
        let mut d = 0.0;
        for (k, octave) in self.octaves.iter().enumerate() {
            let share = left_out(octave);
            if share > 0.0 {
                let wavenumber = 2.0 * core::f64::consts::PI / octave_wavelength(octave);
                d += share
                    * 2.0
                    * octave.ground_variance().value()
                    * (1.0 - math::j0(wavenumber * b));
            }
            if share < 1.0 {
                let sharpening = self.sharpening(k, band);
                if sharpening.variance_m2 > 0.0 {
                    let saturation = 2.0 * sharpening.variance_m2;
                    d += (1.0 - share)
                        * saturation
                        * -math::exp_m1(-sharpening.slope2 * b * b / saturation);
                }
            }
        }
        SquareMetres::new(d)
    }
}

/// The share of `octave` that the level of `set` leaves out: none of an octave it keeps, all of
/// one it does not ([`LevelSet::keeps_crater_octave`], by the octave's index).
#[must_use]
fn left_out_by(set: &LevelSet, octave: &CraterOctave) -> f64 {
    if set.keeps_crater_octave(octave.index()) {
        0.0
    } else {
        1.0
    }
}

/// The fillet the wavelength-keyed forms sharpen the octaves they keep from: half the wavelength
/// `w_m` metres, never finer than the ground's ([`GROUND_FILLET`]).
#[must_use]
fn wavelength_band(w_m: f64) -> Metres {
    Metres::new(num::max(0.5 * w_m, GROUND_FILLET.value()))
}

/// Asserts `lag` is finite and non-negative.
fn assert_lag(lag: Metres) {
    let b = lag.value();
    assert!(
        b.is_finite() && b >= 0.0,
        "a lag must be finite and non-negative, got {b} m"
    );
}

/// `finer_than` in metres, asserted finite and positive ([`assert_wavelength`]).
#[must_use]
fn wavelength(finer_than: Metres) -> f64 {
    assert_wavelength(finer_than);
    finer_than.value()
}

/// Asserts `finer_than` is finite and positive.
fn assert_wavelength(finer_than: Metres) {
    let w = finer_than.value();
    assert!(
        w.is_finite() && w > 0.0,
        "a wavelength must be finite and positive, got {w} m"
    );
}

/// The share of `octave`'s variance in wavelengths shorter than `w_m` metres:
/// clamp(ln(w ÷ D₀) ÷ ln(D₁ ÷ D₀), 0, 1), D₀ to D₁ the diameters it keeps.
#[must_use]
fn band_share(w_m: f64, octave: &CraterOctave) -> f64 {
    let (smallest, upper) = kept_span(octave);
    let x = math::ln(w_m / smallest) / math::ln(upper / smallest);
    num::min(num::max(x, 0.0), 1.0)
}

/// The diameters `octave` keeps, from its smallest to its upper, metres.
#[must_use]
fn kept_span(octave: &CraterOctave) -> (f64, f64) {
    (octave.smallest.value(), octave.diameters.upper().value())
}

/// The wavelength `octave`'s variance is placed at, metres: the geometric mean of the diameters it
/// keeps, √2 times its lower for a whole octave.
#[must_use]
fn octave_wavelength(octave: &CraterOctave) -> f64 {
    let (smallest, upper) = kept_span(octave);
    (smallest * upper).sqrt()
}

/// Octave `octave`'s moments on the body of `params` (the module documentation), first its
/// closed-form variance, square metres: its kept density per m² times its kept craters' mean
/// ∫h² dA over their diameters and its window's ages, at full resolution.
///
/// At full resolution a crater is its fresh profile times its relief factor g, so its ∫h² dA is
/// the fresh one's, π R² ∫f² 2x dx, times g². The mean over the diameters is eight-point
/// Gauss–Legendre in the diameter's share, by the octave's own inversion ([`diameter_in_octave`])
/// over the shares it keeps, from 1 − kept to 1; a power law fitted to the octave's ends at the
/// same nodes lay 12% high on Mars (R09.T7.b's build). The mean of g² over the ages is
/// [`mean_square_relief`]'s, in closed form where the production is linear: g² falls by orders
/// of magnitude across a window, so a few young craters carry an octave's variance, which nodes in
/// the age's share would miss (R09.T7.b's build found them 120 times short).
///
/// Beside it, at the same nodes: its ground variance, the same sum with each crater filleted over
/// [`GROUND_FILLET`] in place of its fresh profile (R09.T7.h); each node's crater at that fillet
/// and its weight, from which its sharpening at any band is summed; and the mean area its kept
/// craters' reaches cover, π ((1 + a) R)² with each node's own exterior width a, square metres:
/// what R09.T8's level bound counts the craters reaching a point by (a marked Poisson field
/// covers a point a Poisson number of times, of mean the density times that area). All are zero,
/// and there are no nodes, for an octave of no density.
#[must_use]
fn octave_moments(params: &CraterParams, octave: &CraterOctave) -> OctaveMoments {
    let density_per_m2 = octave.density().value() * 1e-6;
    if density_per_m2 <= 0.0 {
        return OctaveMoments {
            variance: SquareMetres::ZERO,
            ground_variance: SquareMetres::ZERO,
            reach_area_m2: 0.0,
            nodes: Vec::new(),
        };
    }
    let dropped = 1.0 - octave.kept;
    let mut mean = 0.0;
    let mut ground_mean = 0.0;
    let mut reach_area = 0.0;
    let mut nodes = Vec::with_capacity(GAUSS_LEGENDRE_8.len());
    for (node, weight) in GAUSS_LEGENDRE_8 {
        let node_share = dropped + octave.kept * f64::midpoint(1.0, node);
        let size = UnitUniform::new(node_share).expect("a node's share lies in [0, 1)");
        let diameter = diameter_in_octave(params, octave.diameters, size);
        let shape = crater_shape(params, diameter);
        let rim_m = 0.5 * diameter.value();
        let fresh = CraterProfile::filleted(shape, Metres::ZERO).square_moment();
        let ground = CraterProfile::filleted(shape, GROUND_FILLET);
        let relief = mean_square_relief(rim_m, octave, params.system_age());
        // ½ ω π R², the leading factors of the variance's term in their order, so that the
        // variance keeps its bits.
        let disc_m2 = 0.5 * weight * core::f64::consts::PI * rim_m * rim_m;
        mean += disc_m2 * fresh * relief;
        ground_mean += disc_m2 * ground.square_moment() * relief;
        let reach_m = shape.reach().value();
        reach_area += 0.5 * weight * core::f64::consts::PI * reach_m * reach_m;
        nodes.push(OctaveNode {
            ground,
            disc_m2,
            relief2: relief,
        });
    }
    OctaveMoments {
        variance: SquareMetres::new(density_per_m2 * mean),
        ground_variance: SquareMetres::new(density_per_m2 * ground_mean),
        reach_area_m2: reach_area,
        nodes,
    }
}

/// An octave's moments at its nodes ([`octave_moments`]).
struct OctaveMoments {
    /// Its variance at full resolution, `σ_k`².
    variance: SquareMetres,
    /// Its variance at the ground's fillet, `G_k`.
    ground_variance: SquareMetres,
    /// The mean area its kept craters' reaches cover, square metres.
    reach_area_m2: f64,
    /// Its nodes.
    nodes: Vec<OctaveNode>,
}

/// The mean of g² ([`relief_factor`]) over `octave`'s window of ages for craters of rim radius
/// `rim_m` metres, in a system `system` old: E[g²] = ∫ G(t) dN(t) ÷ N(τ) over [0, τ], with
/// G(t) = (b ÷ (α t + b))⁴, α = κ ÷ R² per Gyr and dN ÷ dt = L + E λ e^(λ (o + t)), the
/// chronology's rate ([`lunar_chronology`]'s L, E, λ and o).
///
/// The linear part is closed: L ∫ G dt = L τ (1 − (1 + x)⁻³) ÷ (3x) with x = α τ ÷ b. The
/// exponential part is eight-point Gauss–Legendre on eight equal panels of the window, where it
/// is smooth: it matters only in a window's oldest Gyr, far from G's fall near t = 0. One for a
/// window of no age or no craters.
#[must_use]
fn mean_square_relief(rim_m: f64, octave: &CraterOctave, system: Gigayears) -> f64 {
    let tau = octave.window.value();
    let total = octave.window_density.value();
    if tau <= 0.0 || total <= 0.0 {
        return 1.0;
    }
    let alpha = DIFFUSIVITY_M2_PER_MYR * 1e3 / (rim_m * rim_m);
    let b = DEGRADATION_B;
    let x = alpha * tau / b;
    let linear = if x > 0.0 {
        LINEAR_RATE * tau * -math::exp_m1(-3.0 * math::ln_1p(x)) / (3.0 * x)
    } else {
        LINEAR_RATE * tau
    };
    let offset = SOLAR_SYSTEM_AGE.value() - system.value();
    let panels = 8_u8;
    let width = tau / f64::from(panels);
    let mut exponential = 0.0;
    for panel in 0..panels {
        let mid = width * (f64::from(panel) + 0.5);
        let mut sum = 0.0;
        for (node, weight) in GAUSS_LEGENDRE_8 {
            let t = mid + 0.5 * width * node;
            let fall = b / (alpha * t + b);
            let g2 = fall * fall * fall * fall;
            sum += weight * g2 * math::exp(EXPONENTIAL_RATE * (offset + t));
        }
        exponential += 0.5 * width * sum;
    }
    (linear + EXPONENTIAL_COEFFICIENT * EXPONENTIAL_RATE * exponential) / total
}

/// The deepest quadtree level whose narrowest cell, as a chord on a sphere of the figure's smallest
/// radius `smallest_radius_m`, holds `reach_m` within [`SEARCH_MARGIN`] (the module documentation).
///
/// A chord between two spheroid points M·d₁ and M·d₂ is at least the smallest radius times
/// |d₁ − d₂| = 2 sin(θ ÷ 2), and every point outside a cell's 3 × 3 neighbourhood is at least the
/// level's narrowest width θ from it.
///
/// # Panics
///
/// If no level's cells hold the reach.
#[must_use]
fn search_level(reach_m: f64, smallest_radius_m: f64) -> u8 {
    (0..=MAX_LEVEL)
        .rev()
        .find(|&level| {
            let width = min_rate() / f64::from(1_u32 << level);
            reach_m <= SEARCH_MARGIN * 2.0 * smallest_radius_m * math::sin(0.5 * width)
        })
        .expect("an octave's reach fits a face's narrowest width")
}

/// S2's quadratic warp's rate du ÷ ds = (8 ÷ 3) max(s, 1 − s) at the cell coordinate `s`.
#[must_use]
fn warp_rate(s: f64) -> f64 {
    8.0 / 3.0 * if s >= 0.5 { s } else { 1.0 - s }
}

/// The unit direction at (`s`, `t`) of `face`, and the spheroid's area there per unit of s and t,
/// square metres: (du ÷ ds)(dv ÷ dt) ÷ (1 + u² + v²)^(3/2), the gnomonic face's solid angle
/// element through the warp, times a² c |M⁻¹d|, the area element of the map d ↦ M·d from the
/// unit sphere to the spheroid.
#[must_use]
fn area_density(face: Face, s: f64, t: f64, figure: &Spheroid) -> ([f64; 3], f64) {
    let (u, v) = (st_to_uv(s), st_to_uv(t));
    let rho2 = 1.0 + u * u + v * v;
    let solid = warp_rate(s) * warp_rate(t) / (rho2 * rho2.sqrt());
    let dir = unit_dir(face_uv_to_xyz(FaceUv { face, u, v }));
    let (equatorial_m, polar_m) = (figure.equatorial_radius_m, figure.polar_radius_m);
    let inverse = [
        dir[0] / equatorial_m,
        dir[1] / equatorial_m,
        dir[2] / polar_m,
    ];
    let norm = (inverse[0] * inverse[0] + inverse[1] * inverse[1] + inverse[2] * inverse[2]).sqrt();
    (dir, solid * (equatorial_m * equatorial_m * polar_m * norm))
}

/// The cell coordinates `cell` spans along one axis, from its index `index`: [i ÷ 2ⁿ,
/// (i + 1) ÷ 2ⁿ], exact binary fractions.
#[must_use]
fn span(cell: PatchKey, index: u32) -> (f64, f64) {
    let side = f64::from(1_u32 << cell.level());
    (f64::from(index) / side, f64::from(index + 1) / side)
}

/// The true area of `cell` on `figure`'s spheroid, square metres: the area element integrated by
/// eight-point Gauss–Legendre in s and t over each part of the cell on one side of the warp's kink
/// at ½ (only a whole face straddles it), to under 10⁻⁹ of it at every level.
#[must_use]
pub fn cell_area(cell: PatchKey, figure: &Spheroid) -> SquareMetres {
    let parts = |(lo, hi): (f64, f64)| -> [(f64, f64); 2] {
        if lo < 0.5 && hi > 0.5 {
            [(lo, 0.5), (0.5, hi)]
        } else {
            [(lo, hi), (hi, hi)]
        }
    };
    let mut area = 0.0;
    for (s0, s1) in parts(span(cell, cell.i())) {
        for (t0, t1) in parts(span(cell, cell.j())) {
            if s1 <= s0 || t1 <= t0 {
                continue;
            }
            let (hs, ht) = (0.5 * (s1 - s0), 0.5 * (t1 - t0));
            let (ms, mt) = (f64::midpoint(s0, s1), f64::midpoint(t0, t1));
            let mut sum = 0.0;
            for (ns, ws) in GAUSS_LEGENDRE_8 {
                let mut row = 0.0;
                for (nt, wt) in GAUSS_LEGENDRE_8 {
                    row += wt * area_density(cell.face(), ms + hs * ns, mt + ht * nt, figure).1;
                }
                sum += ws * row;
            }
            area += hs * ht * sum;
        }
    }
    SquareMetres::new(area)
}

/// A bound on [`area_density`] over `cell`: the warp's largest rates over its spans, times the
/// gnomonic factor at its smallest |u| and |v|, times a² c ÷ min(a, c), since |M⁻¹d| ≤ 1 ÷ min(a,
/// c).
#[must_use]
fn area_density_bound(cell: PatchKey, figure: &Spheroid) -> f64 {
    let rate = |(lo, hi): (f64, f64)| 8.0 / 3.0 * num::max(hi, 1.0 - lo);
    let nearest = |(lo, hi): (f64, f64)| {
        if lo <= 0.5 && hi >= 0.5 {
            0.0
        } else {
            num::min(st_to_uv(lo).abs(), st_to_uv(hi).abs())
        }
    };
    let (along_i, along_j) = (span(cell, cell.i()), span(cell, cell.j()));
    let (u, v) = (nearest(along_i), nearest(along_j));
    let rho2 = 1.0 + u * u + v * v;
    let (equatorial_m, polar_m) = (figure.equatorial_radius_m, figure.polar_radius_m);
    let stretch = equatorial_m * equatorial_m * polar_m / num::min(equatorial_m, polar_m);
    rate(along_i) * rate(along_j) / (rho2 * rho2.sqrt()) * stretch
}

/// A crater narrower than the boundary diameter, as the synthesis draws it: its octave, centre,
/// fresh shape and age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmallCrater {
    octave: u8,
    centre: [f64; 3],
    point_m: [f64; 3],
    shape: CraterShape,
    age: Gigayears,
    diffusion: Metres,
}

impl SmallCrater {
    /// Its octave's index.
    #[must_use]
    pub const fn octave(&self) -> u8 {
        self.octave
    }

    /// Its centre, a unit direction in body-fixed axes.
    #[must_use]
    pub const fn centre(&self) -> [f64; 3] {
        self.centre
    }

    /// Its centre's spheroid point M·d, metres, from which distances are chords.
    #[must_use]
    pub const fn point_m(&self) -> [f64; 3] {
        self.point_m
    }

    /// Its rim-to-rim diameter.
    #[must_use]
    pub const fn diameter(&self) -> Metres {
        self.shape.diameter()
    }

    /// Its fresh shape ([`crater_shape`] of its diameter on the body).
    #[must_use]
    pub const fn shape(&self) -> &CraterShape {
        &self.shape
    }

    /// Its age.
    #[must_use]
    pub const fn age(&self) -> Gigayears {
        self.age
    }

    /// Its diffusion length ([`diffusion_length`] of its age).
    #[must_use]
    pub const fn diffusion(&self) -> Metres {
        self.diffusion
    }
}

/// The caller's cache of a body's drawn craters, by cell: one per worker or per bake, as
/// [`SynthCache`](super::SynthCache) is (R09.T8 holds it there).
///
/// No height depends on what it holds, only the time a query takes: it holds the craters of one
/// body's draw, starting afresh when it is used for another (another seed or header), and when it
/// has grown to 16,384 cells.
#[derive(Debug, Clone, Default)]
pub struct CraterCache {
    owner: Option<(DetailSeed, Inputs)>,
    cells: BTreeMap<u64, Vec<SmallCrater>>,
}

impl CraterCache {
    /// An empty cache.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            owner: None,
            cells: BTreeMap::new(),
        }
    }

    /// The number of cells it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether it holds no cell.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// A body's small craters, drawn from its detail seed: the octaves of its field's header, the
/// craters of any octave's cell, and their heights at a level (the module documentation).
///
/// It is plain data, a function of the header and the seed alone, and reads no cell of the field:
/// the craters' density is the body's.
#[derive(Debug, Clone, PartialEq)]
pub struct SmallCraters {
    seed: DetailSeed,
    octaves: CraterOctaves,
}

impl SmallCraters {
    /// The small craters of the body of `header`, drawn from its detail seed `seed`.
    ///
    /// # Panics
    ///
    /// As [`CraterOctaves::new`].
    #[must_use]
    pub fn new(header: &FieldHeader, seed: DetailSeed) -> Self {
        Self {
            seed,
            octaves: CraterOctaves::new(header),
        }
    }

    /// The body's detail seed.
    #[must_use]
    pub const fn seed(&self) -> DetailSeed {
        self.seed
    }

    /// The body's octaves.
    #[must_use]
    pub const fn octaves(&self) -> &CraterOctaves {
        &self.octaves
    }

    /// The craters drawn in `cell` of octave `octave`'s level, in draw order, from `cache` or
    /// drawn into it.
    ///
    /// # Panics
    ///
    /// If the body has no octave `octave`, or `cell` is not at its level; or if a crater drawn has
    /// an exterior wider than [`MAX_EXTERIOR_WIDTH`] (a bug).
    pub fn craters_in<'c>(
        &self,
        octave: u8,
        cell: PatchKey,
        cache: &'c mut CraterCache,
    ) -> &'c [SmallCrater] {
        let index = usize::from(octave);
        assert!(
            index < self.octaves.octaves.len(),
            "the body has no crater octave {octave}"
        );
        assert_eq!(
            cell.level(),
            self.octaves.octaves[index].level,
            "the cell is not at the octave's level"
        );
        let owner = (self.seed, self.octaves.inputs);
        if cache.owner != Some(owner) {
            cache.cells.clear();
            cache.owner = Some(owner);
        }
        let key = (u64::from(octave) << 56) | cell.to_u64();
        if cache.cells.len() >= CACHE_CELLS && !cache.cells.contains_key(&key) {
            cache.cells.clear();
        }
        cache
            .cells
            .entry(key)
            .or_insert_with(|| self.draw(octave, cell))
    }

    /// The craters of `cell` of octave `octave`, drawn from the cell's own stream (the module
    /// documentation).
    #[must_use]
    fn draw(&self, octave: u8, cell: PatchKey) -> Vec<SmallCrater> {
        let inputs = &self.octaves.inputs;
        let figure = inputs.figure;
        let params = &inputs.params;
        let band = &self.octaves.octaves[usize::from(octave)];
        let km2 = cell_area(cell, &figure).value() * 1e-6;
        let mean = band.drawn_density().value() * km2;
        let mut stream = self.seed.stream(
            SURFACE_CRATER,
            ObjectKey::surface_cell(cell.face().index(), cell.level(), cell.i(), cell.j(), 0)
                .expect("a patch key is a surface cell"),
        );
        let count = stream.poisson(mean);
        let bound = area_density_bound(cell, &figure);
        // A capacity hint alone, which a count beyond usize's range (never reached: the mean is a
        // few craters a cell) leaves at none; the loop draws every crater whatever it is.
        let mut craters = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for _ in 0..count {
            let centre = place(cell, &figure, bound, &mut stream);
            let size = UnitUniform::new(stream.uniform()).expect("a uniform is in [0, 1)");
            let age_share = stream.uniform();
            let diameter = diameter_in_octave(params, band.diameters, size);
            // The last octave draws below the band limit too, and keeps what is at or above it.
            if diameter.value() < band.smallest.value() {
                continue;
            }
            let age = chronology_age(band.window_density * age_share, params.system_age());
            let shape = crater_shape(params, diameter);
            assert!(
                shape.exterior_width() <= MAX_EXTERIOR_WIDTH,
                "a crater's exterior width {} exceeds the search's bound",
                shape.exterior_width()
            );
            craters.push(SmallCrater {
                octave,
                centre,
                point_m: figure.point(centre),
                shape,
                age,
                diffusion: diffusion_length(age),
            });
        }
        craters
    }

    /// The craters of octaves 0 to `through` (inclusive, and never past the body's last) whose
    /// reach holds the unit direction `dir`, octave by octave from the largest, each octave's
    /// cells by key and each cell's in draw order, with each one's chord from its centre, metres,
    /// and the chord's vector from the centre's spheroid point.
    fn each_reaching(
        &self,
        dir: [f64; 3],
        through: u8,
        cache: &mut CraterCache,
        mut emit: impl FnMut(&SmallCrater, [f64; 3], f64),
    ) {
        let p = self.octaves.inputs.figure.point(dir);
        let last = num_octaves_through(self.octaves.octaves.len(), through);
        for octave in 0..last {
            let level = self.octaves.octaves[usize::from(octave)].level;
            let home = PatchKey::containing(level, dir).expect("an octave's level is a level");
            let (cells, n) = neighbourhood(home);
            for &cell in &cells[..n] {
                for crater in self.craters_in(octave, cell, cache) {
                    let delta = [0, 1, 2].map(|k| p[k] - crater.point_m[k]);
                    let d2 = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
                    let reach = crater.shape.reach().value();
                    if d2 < reach * reach {
                        emit(crater, delta, d2.sqrt());
                    }
                }
            }
        }
    }

    /// The craters of octaves 0 to `through` whose reach holds the unit direction `dir`, in the
    /// order [`at`](Self::at) sums them.
    ///
    /// # Panics
    ///
    /// When an octave is searched: if `dir` is zero or has a component that is not finite; and if
    /// a crater drawn has an exterior wider than [`MAX_EXTERIOR_WIDTH`] (a bug).
    #[must_use]
    pub fn craters_reaching(
        &self,
        dir: [f64; 3],
        through: u8,
        cache: &mut CraterCache,
    ) -> Vec<SmallCrater> {
        let mut found = Vec::new();
        self.each_reaching(dir, through, cache, |crater, _, _| found.push(*crater));
        found
    }

    /// The small craters of octaves 0 to `through` (inclusive, and never past the body's last) at
    /// the unit direction `dir`, band-limited at `level`: their height above the base elevation,
    /// metres, and its gradient in body-fixed space at the spheroid point, metres per metre, as
    /// R09.T4's base elevation returns its own (the gradient of the height as a function of
    /// direction).
    ///
    /// Each crater is [`CraterProfile::new`] of the level's [`band_half_width`] and its own
    /// [`diffusion_length`]: its rim filleted over the band and its relief scaled by its age.
    /// `cache` is the caller's; the result is independent of what it holds.
    ///
    /// # Panics
    ///
    /// When an octave is searched: if `dir` is zero or has a component that is not finite; and if a
    /// crater drawn has an exterior wider than [`MAX_EXTERIOR_WIDTH`] or a height is not finite
    /// (bugs).
    pub fn at(
        &self,
        dir: [f64; 3],
        level: BandLevel,
        through: u8,
        cache: &mut CraterCache,
    ) -> HeightSample {
        let figure = self.octaves.inputs.figure;
        let band = band_half_width(&figure, level);
        let mut height = 0.0;
        let mut gradient = [0.0; 3];
        self.each_reaching(dir, through, cache, |crater, delta, distance| {
            let profile = CraterProfile::new(crater.shape, band, crater.diffusion);
            let rim_m = 0.5 * crater.shape.diameter().value();
            let (h, slope) = profile.height_and_slope(distance / rim_m);
            height += h;
            if distance > 0.0 {
                let along = slope / (rim_m * distance);
                for (g, d) in gradient.iter_mut().zip(delta) {
                    *g += along * d;
                }
            }
        });
        let gradient = direction_gradient(&figure, dir, gradient);
        HeightSample {
            height_m: num::assert_finite(height),
            gradient: gradient.map(num::assert_finite),
        }
    }
}

/// The number of octaves 0 to `through` among a body's `count`.
#[must_use]
fn num_octaves_through(count: usize, through: u8) -> u8 {
    let count = u8::try_from(count).expect("a body has at most 32 crater octaves");
    count.min(through.saturating_add(1))
}

/// The 3 × 3 neighbourhood of `cell`: itself, its four edge neighbours and its corner neighbours
/// (three at a cube corner), across face edges by the cube's rule, sorted by key without repeats;
/// the array and how many of it are the neighbourhood.
#[must_use]
fn neighbourhood(cell: PatchKey) -> ([PatchKey; 9], usize) {
    let mut cells = [cell; 9];
    let mut n = 1;
    for edge in Edge::ALL {
        cells[n] = cell.edge_neighbour(edge);
        n += 1;
    }
    for corner in cell.corner_neighbours().into_iter().flatten() {
        cells[n] = corner;
        n += 1;
    }
    cells[..n].sort_unstable();
    let mut kept = 1;
    for k in 1..n {
        let next = cells[k];
        if next != cells[kept - 1] {
            cells[kept] = next;
            kept += 1;
        }
    }
    (cells, kept)
}

/// A crater's centre in `cell`, uniform in the spheroid's area over it, by rejection against
/// `bound` ([`area_density_bound`]): each candidate draws s and t within the cell and a third word,
/// a [`Mark`](hyperion_base::rng::Mark) that accepts it below the area element over the bound
/// ([`Threshold::from_ratio`], whose debug assertion holds the bound), and the last of
/// [`MAX_PLACEMENT_ATTEMPTS`] is taken whatever it draws.
#[must_use]
fn place(cell: PatchKey, figure: &Spheroid, bound: f64, stream: &mut Stream) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    let (i, j) = (f64::from(cell.i()), f64::from(cell.j()));
    let mut attempt = 1;
    loop {
        let s = (i + stream.uniform()) / side;
        let t = (j + stream.uniform()) / side;
        let accept = stream.mark();
        let (dir, density) = area_density(cell.face(), s, t, figure);
        if accept.is_below(Threshold::from_ratio(density, bound))
            || attempt == MAX_PLACEMENT_ATTEMPTS
        {
            return dir;
        }
        attempt += 1;
    }
}

/// The gradient `g` of a function of the spheroid point P, at the spheroid point of the unit
/// direction `dir`, as the gradient of the same values as a function of direction alone,
/// H(P) = h(M⁻¹P ÷ |M⁻¹P|): M⁻¹ (I − d dᵀ) M g, the convention of R09.T4's base elevation (the
/// relief's `as_function_of_direction`).
#[must_use]
fn direction_gradient(figure: &Spheroid, dir: [f64; 3], g: [f64; 3]) -> [f64; 3] {
    let radii = [
        figure.equatorial_radius_m,
        figure.equatorial_radius_m,
        figure.polar_radius_m,
    ];
    let u = [0, 1, 2].map(|k| radii[k] * g[k]);
    let along = u[0] * dir[0] + u[1] * dir[1] + u[2] * dir[2];
    [0, 1, 2].map(|k| (u[k] - along * dir[k]) / radii[k])
}

/// The craters' variance in wavelengths shorter than `finer_than` at `cell`, square metres, or
/// `None` where `field` does not hold the cell (Design note 18): the body's, the same in every
/// cell ([`CraterOctaves::unresolved_variance`]).
///
/// # Panics
///
/// If `finer_than` is not finite and positive, or `cell` is not of the field's level.
#[must_use]
pub fn unresolved_variance(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
) -> Option<SquareMetres> {
    assert_wavelength(finer_than);
    let header = field.header();
    assert_eq!(
        cell.level(),
        header.level().get(),
        "the cell is not of the field's level"
    );
    field.cell(cell)?;
    Some(CraterOctaves::new(header).unresolved_variance(finer_than))
}

/// The craters' structure function at `lag` in wavelengths shorter than `finer_than` at `cell`,
/// square metres, or `None` where `field` does not hold the cell: the body's, the same in every
/// cell ([`CraterOctaves::structure_function`]), R10's slope readouts' share of the craters.
///
/// # Panics
///
/// If `finer_than` is not finite and positive, `lag` is not finite and non-negative, or `cell` is
/// not of the field's level.
#[must_use]
pub fn structure_function(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
    lag: Metres,
) -> Option<SquareMetres> {
    assert_wavelength(finer_than);
    let header = field.header();
    assert_eq!(
        cell.level(),
        header.level().get(),
        "the cell is not of the field's level"
    );
    field.cell(cell)?;
    Some(CraterOctaves::new(header).structure_function(finer_than, lag))
}

pub(crate) mod bound;

#[cfg(test)]
mod tests;
