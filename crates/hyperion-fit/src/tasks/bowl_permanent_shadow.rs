//! `run bowl_permanent_shadow`: a spherical bowl's permanent shadow by exact ray geometry, the
//! table that `hyperion_surface::cold_trap` reads in place of Hayne et al. 2021's eq. 26
//! (rendering plan R09, R09.T7.f; `science-r09-drainage.md` §8.12 and `decision-r09-t7f.md`).
//!
//! # The geometry
//!
//! The bowl is a spherical cap of rim radius 1 (diameter 2) and depth d = 2γ, γ = d/D in
//! (0, 0.5]: a sphere of radius R = (1 + d²) ÷ 2d whose centre stands h = R − d above the rim's
//! plane. Axes are east, north and up, at a north latitude of colatitude e₀; the celestial pole is
//! n = (0, sin e₀, cos e₀). Below, Δ stands for δ + α, the star's greatest declination plus its
//! angular radius.
//!
//! A point of the floor is lit when its chord toward some part of the star's disk leaves through the
//! rim (`science-r09-drainage.md` §8.10's test). The star's centre keeps to declinations within δ,
//! each day at every hour angle, and its disk reaches α beyond, so over a year its disk covers
//! exactly the band of declinations |dec| ≤ Δ. A point is therefore in permanent shadow when no
//! direction of that band reaches the sky through the opening.
//!
//! The directions that reach the sky from a point inside the bowl are those toward the opening, a
//! convex disk, so they form a connected, convex cone that holds the zenith: straight up from any
//! point of the floor is the opening. The zenith's declination is the latitude, at least 0 ≥ −Δ,
//! so the cone meets the band exactly when its least declination is at most Δ. Declination, linear
//! in the direction, has no stationary point on the cone but the south celestial pole, which lies
//! below the horizon, so its least value is on the cone's boundary, the directions toward the rim.
//! So the point is in permanent shadow exactly when every direction toward the rim has a
//! declination above Δ:
//!
//! > min over the rim of n · (X − P) ÷ |X − P| > sin Δ,
//!
//! which is the chord test at the band's upper edge, the upper limb at its greatest declination.
//! For each floor point this least sine is found once, by a scan of the rim and a golden-section
//! refinement, and serves every Δ at once.
//!
//! At the pole the shadow is a disk, and its share of the opening is Hayne et al.'s eq. 6,
//! x₀″² with x₀″ = cos 2Δ − (β ÷ 2) sin 2Δ and β = 1 ÷ 2γ − 2γ (their eq. 3 at the elevation Δ).
//! Since β ÷ 2 = cot θ, θ the rim's slope below, that is (sin(θ − 2Δ) ÷ sin θ)² ([`polar_shadow`]),
//! and the pole's row of the table is that closed form.
//!
//! # The axes
//!
//! The rim's slope θ = 2 atan(2γ) sets the scale of every angle that matters: a bowl casts no
//! shadow at all once the noon sun is above θ, and eq. 6 leaves no polar shadow once 2Δ ≥ θ. So the
//! table's axes are ln γ over 0.02–0.5, the colatitude over θ on [0, 1] and Δ over θ on [0, ½],
//! and the shadow is exactly zero where (e₀ + Δ) ÷ θ ≥ 1 or Δ ÷ θ ≥ ½. In these variables the
//! shadow hardly depends on γ: small slopes are similar, so that below d/D 0.04 the share at a
//! fixed point moves by about 0.002, and the edge of the shadow moves by under 0.01 of θ from d/D
//! 0.02 to 0.5 (`decision-r09-t7f.md` §1). `cold_trap` reads a bowl shallower than 0.02 from the
//! 0.02 row at its own θ, and one deeper than a hemisphere as one.
//!
//! The plan's axes were the colatitude over 0–45° and Δ over 0–30°, in degrees, withdrawn by
//! `decision-r09-t7f.md`: a shallow bowl's shadow lies within 4.6° of the pole at d/D 0.02, so
//! degree-uniform nodes fine enough for 0.01 would have run past the 500 kB limit on a committed
//! file, and deep bowls keep shadow beyond both ranges.
//!
//! # The estimate
//!
//! Each node's share of the opening is the count of a square lattice's points in shadow, `grid`
//! to the rim's radius, over the half disk east of the meridian, which mirrors the west. Counts
//! are integers, so the share is exact for its lattice; at 256 it is within about 2 × 10⁻⁴ of the
//! continuum (against 1,024). Shares off the pole are rounded to `decimals` places, below the
//! lattice's own error, which keeps the committed file small. Every node is a pure function of its
//! index and every transcendental goes through [`hyperion_sim::math`], so the table is the same
//! for any thread count and on every platform.

use std::f64::consts::PI;
use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::math;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::optimise::golden_section;
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TableCrate, TaskClass, TaskOutput};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The greatest (colatitude + Δ) over the rim's slope at which any shadow survives: beyond it the
/// noon sun is above every part of the wall.
pub const COLATITUDE_RANGE: f64 = 1.0;

/// The greatest Δ = δ + α over the rim's slope at which any shadow survives: eq. 6's polar limit,
/// where 2Δ reaches the rim's slope.
pub const LIMB_RANGE: f64 = 0.5;

/// The depth ratios at which a run checks its trilinear table against exact geometry every degree
/// from the pole to 60°: T7.f's (decision-r09-t7f.md §3 (a)).
pub const CHECK_DEPTH_RATIOS: [f64; 4] = [0.06, 0.1, 0.2, 0.3];

/// The values of Δ = δ + α, degrees, at which a run checks its table every degree from the pole to
/// 60°: the Sun as a point and as a disk at the Moon (1.54° and 1.54° + 0.27°), Mercury's
/// 0.034° + 0.87° at perihelion (`science-r09-drainage.md` §8.4), and wider skies to 30°.
pub const CHECK_LIMBS_DEGREES: [f64; 10] =
    [0.0, 0.27, 0.90, 1.54, 1.81, 3.0, 5.0, 10.0, 20.0, 30.0];

/// One of R09.T7.f's test points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TracedPoint {
    /// The bowl's d/D.
    pub depth_ratio: f64,
    /// The latitude, degrees.
    pub latitude_deg: f64,
    /// Δ = δ + α, degrees.
    pub limb_deg: f64,
}

/// R09.T7.f's test points, at which the committed table read trilinearly matches direct ray
/// tracing to 0.01 (decision-r09-t7f.md §3 (a)): d/D 0.06, 0.1, 0.2 and 0.3 every 5° from the
/// pole to 60° at the Moon's 1.54°, deep bowls past 45° colatitude, and wider skies to a
/// hemisphere at the pole under 40°.
#[must_use]
pub fn traced_points() -> Vec<TracedPoint> {
    let point = |depth_ratio, latitude_deg, limb_deg| TracedPoint {
        depth_ratio,
        latitude_deg,
        limb_deg,
    };
    let mut points = Vec::with_capacity(37);
    for ratio in CHECK_DEPTH_RATIOS {
        for step in 0..=6_u32 {
            points.push(point(ratio, 90.0 - 5.0 * f64::from(step), 1.54));
        }
    }
    points.extend([
        point(0.5, 45.0, 1.54),
        point(0.5, 30.0, 1.54),
        point(0.4, 50.0, 1.54),
        point(0.2, 85.0, 4.0),
        point(0.3, 75.0, 4.0),
        point(0.3, 80.0, 20.0),
        point(0.5, 60.0, 20.0),
        point(0.5, 90.0, 30.0),
        point(0.5, 90.0, 40.0),
    ]);
    points
}

/// The grid's parameters, from the manifest's `[params]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// The shallowest tabulated d/D.
    pub depth_ratio_min: f64,
    /// The deepest tabulated d/D, at most a hemisphere's 0.5.
    pub depth_ratio_max: f64,
    /// Nodes in ln(d/D), even from the shallowest to the deepest.
    pub depth_ratio_nodes: usize,
    /// Nodes in the colatitude over the rim's slope, even on [0, 1].
    pub colatitude_nodes: usize,
    /// Nodes in Δ over the rim's slope, even on [0, ½].
    pub limb_nodes: usize,
    /// Lattice points to the rim's radius.
    pub grid: u32,
    /// Points of the rim's first scan for its least declination.
    pub scan: u32,
    /// Golden-section steps that refine the scan's least point.
    pub refine: u32,
    /// Decimal places the shares off the pole are rounded to.
    pub decimals: i32,
}

/// The largest count a manifest may give.
const MOST: u64 = 100_000;

impl Params {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] for a parameter that is missing or out of range:
    /// - the depth ratios must satisfy 0 < min < max ≤ 0.5;
    /// - each axis needs 2 to 100,000 nodes, the lattice and the scan 4 to 100,000 points, and the
    ///   refinement 1 to 100,000 steps;
    /// - the places must be 1 to 12.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let count = |key: &str, least: u64| -> Result<u64, ManifestParamError> {
            let n = manifest.u64(key)?;
            if n < least || n > MOST {
                return Err(ManifestParamError::new(
                    key,
                    &format!("an integer from {least} to 100,000"),
                ));
            }
            Ok(n)
        };
        let nodes = |key: &str| -> Result<usize, ManifestParamError> {
            usize::try_from(count(key, 2)?).map_err(|_| ManifestParamError::new(key, "a usize"))
        };
        let small = |key: &str, least: u64| -> Result<u32, ManifestParamError> {
            u32::try_from(count(key, least)?).map_err(|_| ManifestParamError::new(key, "a u32"))
        };
        let depth_ratio_min = manifest.f64("depth_ratio_min")?;
        let depth_ratio_max = manifest.f64("depth_ratio_max")?;
        if !(depth_ratio_min > 0.0 && depth_ratio_min < depth_ratio_max && depth_ratio_max <= 0.5) {
            return Err(ManifestParamError::new(
                "depth_ratio_min and depth_ratio_max",
                "0 < min < max ≤ 0.5",
            ));
        }
        let decimals = small("decimals", 1)?;
        if decimals > 12 {
            return Err(ManifestParamError::new("decimals", "from 1 to 12"));
        }
        Ok(Self {
            depth_ratio_min,
            depth_ratio_max,
            depth_ratio_nodes: nodes("depth_ratio_nodes")?,
            colatitude_nodes: nodes("colatitude_nodes")?,
            limb_nodes: nodes("limb_nodes")?,
            grid: small("grid", 4)?,
            scan: small("scan", 4)?,
            refine: small("refine", 1)?,
            decimals: i32::try_from(decimals)
                .map_err(|_| ManifestParamError::new("decimals", "from 1 to 12"))?,
        })
    }
}

/// The rim's slope of a spherical bowl of depth ratio `depth_ratio`, radians: 2 atan(2γ), whose
/// tangent 4γ ÷ (1 − 4γ²) is the wall's steepest slope, 90° for a hemisphere.
#[must_use]
pub fn rim_slope(depth_ratio: f64) -> f64 {
    2.0 * math::atan(2.0 * depth_ratio)
}

/// A spherical bowl of rim radius 1.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bowl {
    /// Its depth d = 2γ.
    depth: f64,
    /// Its sphere's radius R = (1 + d²) ÷ 2d.
    sphere: f64,
    /// Its sphere's centre's height above the rim's plane, h = R − d, never negative.
    centre: f64,
}

impl Bowl {
    /// The bowl of depth ratio `depth_ratio` in (0, 0.5].
    ///
    /// # Panics
    ///
    /// If the depth ratio is not in (0, 0.5].
    #[must_use]
    fn new(depth_ratio: f64) -> Self {
        assert!(
            depth_ratio > 0.0 && depth_ratio <= 0.5,
            "a bowl's depth ratio must be in (0, 0.5], got {depth_ratio}"
        );
        let depth = 2.0 * depth_ratio;
        let sphere = (1.0 + depth * depth) / (2.0 * depth);
        Self {
            depth,
            sphere,
            centre: sphere - depth,
        }
    }

    /// The floor's height at a horizontal distance² `rho2` < 1 from the axis: negative, −d at the
    /// axis and 0 at the rim, written R − √(R² − ρ²) = ρ² ÷ (R + √(R² − ρ²)) so that a shallow bowl
    /// keeps its digits.
    #[must_use]
    fn floor(&self, rho2: f64) -> f64 {
        -self.depth + rho2 / (self.sphere + (self.sphere * self.sphere - rho2).sqrt())
    }
}

/// The rim's directions sampled for its least declination: (sin ψ, cos ψ) at ψ = 2πk ÷ `scan`.
#[must_use]
fn rim_scan(scan: u32) -> Vec<(f64, f64)> {
    (0..scan)
        .map(|k| math::sin_cos(2.0 * PI * f64::from(k) / f64::from(scan)))
        .collect()
}

/// The lattice of `grid` points to the rim's radius over the half disk east of the meridian, in
/// its fixed order: rows of x, each down y.
fn half_disk(grid: u32) -> impl Iterator<Item = (f64, f64)> {
    let spacing = 1.0 / f64::from(grid);
    (0..grid).flat_map(move |i| {
        let x = (f64::from(i) + 0.5) * spacing;
        (0..2 * grid).filter_map(move |j| {
            let y = -1.0 + (f64::from(j) + 0.5) * spacing;
            (x * x + y * y < 1.0).then_some((x, y))
        })
    })
}

/// The rim seen from one floor point: n · (X − P) ÷ |X − P| for the rim point X at azimuth ψ.
#[derive(Debug, Clone, Copy)]
struct RimView {
    x: f64,
    y: f64,
    sin_colatitude: f64,
    /// −n · P.
    offset: f64,
    /// 1 + |P|², so that |X − P|² = `square` − 2x cos ψ − 2y sin ψ.
    square: f64,
}

impl RimView {
    /// The rim seen from the floor point (`x`, `y`) of `bowl` at `colatitude_rad`.
    #[must_use]
    fn new(bowl: &Bowl, colatitude_rad: f64, x: f64, y: f64) -> Self {
        let (sin_colatitude, cos_colatitude) = math::sin_cos(colatitude_rad);
        let z = bowl.floor(x * x + y * y);
        Self {
            x,
            y,
            sin_colatitude,
            offset: -(y * sin_colatitude + z * cos_colatitude),
            square: 1.0 + x * x + y * y + z * z,
        }
    }

    /// The sine of the declination of the rim point at (sin ψ, cos ψ).
    #[must_use]
    fn at(&self, sin_psi: f64, cos_psi: f64) -> f64 {
        let reach = self.square - 2.0 * self.x * cos_psi - 2.0 * self.y * sin_psi;
        (self.sin_colatitude * sin_psi + self.offset) / reach.sqrt()
    }

    /// The least sine over the rim: the least of the `scan`, refined by `refine` golden-section
    /// steps over the two scan intervals about it. A point whose scan already finds a negative
    /// sine is lit under any band, so that sine is returned unrefined.
    #[must_use]
    fn least(&self, scan: &[(f64, f64)], refine: u32) -> f64 {
        let mut best = (f64::INFINITY, 0_u32);
        let mut k = 0_u32;
        for &(s, c) in scan {
            let v = self.at(s, c);
            if v < best.0 {
                best = (v, k);
            }
            k += 1;
        }
        if best.0 < 0.0 {
            return best.0;
        }
        let step = 2.0 * PI / f64::from(k);
        let at = f64::from(best.1);
        let f = |psi: f64| {
            let (s, c) = math::sin_cos(psi);
            self.at(s, c)
        };
        let psi = golden_section(f, step * (at - 1.0), step * (at + 1.0), refine);
        best.0.min(f(psi))
    }
}

/// The shares of a bowl's opening in permanent shadow at `colatitude_rad`, one for each Δ of
/// `limbs_rad` (each in [0, π/2)), by the rim's least declination on a lattice of `grid` points to
/// the rim's radius, the rim scanned at `scan` points and refined by `refine` golden-section steps.
///
/// # Panics
///
/// If the depth ratio is not in (0, 0.5], or `grid` or `scan` is zero.
#[must_use]
pub fn shadow_shares(
    depth_ratio: f64,
    colatitude_rad: f64,
    limbs_rad: &[f64],
    grid: u32,
    scan: u32,
    refine: u32,
) -> Vec<f64> {
    assert!(grid > 0 && scan > 0, "the lattice and the scan need points");
    let bowl = Bowl::new(depth_ratio);
    let rim = rim_scan(scan);
    let thresholds: Vec<f64> = limbs_rad.iter().map(|&l| math::sin(l)).collect();
    let mut counts = vec![0_u64; limbs_rad.len()];
    let mut total = 0_u64;
    for (x, y) in half_disk(grid) {
        total += 1;
        let least = RimView::new(&bowl, colatitude_rad, x, y).least(&rim, refine);
        for (count, &threshold) in counts.iter_mut().zip(&thresholds) {
            if least > threshold {
                *count += 1;
            }
        }
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "lattice counts below 2⁵³ are exact in an f64"
    )]
    let shares = counts.iter().map(|&c| c as f64 / total as f64).collect();
    shares
}

/// Hayne et al. 2021's eq. 6, a bowl's permanent shadow at the pole as a share of its opening,
/// under Δ = `limb_rad`: x₀″² with x₀″ = cos 2Δ − (β ÷ 2) sin 2Δ (their eq. 3 at the elevation
/// Δ), zero where x₀″ ≤ 0. It is written (sin(θ − 2Δ) ÷ sin θ)², θ the rim's slope, since
/// β ÷ 2 = (1 − 4γ²) ÷ 4γ = cot θ; that form is exactly zero at 2Δ = θ.
///
/// # Panics
///
/// If the depth ratio is not in (0, 0.5].
#[must_use]
pub fn polar_shadow(depth_ratio: f64, limb_rad: f64) -> f64 {
    assert!(
        depth_ratio > 0.0 && depth_ratio <= 0.5,
        "a bowl's depth ratio must be in (0, 0.5], got {depth_ratio}"
    );
    let slope = rim_slope(depth_ratio);
    if 2.0 * limb_rad >= slope {
        return 0.0;
    }
    let edge = math::sin(slope - 2.0 * limb_rad) / math::sin(slope);
    edge * edge
}

/// `n` ≥ 2 even nodes from `lo` to `hi`, both included exactly.
#[must_use]
fn even_nodes(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let last = u32::try_from(n - 1).expect("a manifest's node count is at most 100,000");
    (0..=last)
        .map(|i| {
            if i == last {
                hi
            } else {
                lo + (hi - lo) * f64::from(i) / f64::from(last)
            }
        })
        .collect()
}

/// The fitted table.
#[derive(Debug, Clone, PartialEq)]
pub struct BowlShadowFit {
    /// ln(d/D) at the nodes, ascending.
    pub ln_depth_ratios: Vec<f64>,
    /// The colatitude over the rim's slope at the nodes, from 0 to 1.
    pub colatitudes: Vec<f64>,
    /// Δ over the rim's slope at the nodes, from 0 to ½.
    pub limbs: Vec<f64>,
    /// The shares in shadow: a row for each depth ratio and colatitude, depth ratio major, one
    /// share for each limb.
    pub shares: Vec<Vec<f64>>,
    /// The parameters it was made with.
    pub params: Params,
    /// The lattice's points in the half disk.
    pub lattice_points: u64,
}

impl BowlShadowFit {
    /// The share in shadow, trilinear in ln γ, e₀ ÷ θ and Δ ÷ θ, for a bowl of depth ratio
    /// `depth_ratio` at `colatitude_rad` under Δ = `limb_rad`, as `hyperion_surface::cold_trap`
    /// reads the committed table, without its cap. A ratio below the shallowest node takes that
    /// node's row at its own rim slope, one above the deepest the deepest's, and the share is zero
    /// where (e₀ + Δ) ÷ θ ≥ 1 or Δ ÷ θ ≥ ½.
    ///
    /// # Panics
    ///
    /// If the depth ratio is not finite and positive.
    #[must_use]
    pub fn interpolate(&self, depth_ratio: f64, colatitude_rad: f64, limb_rad: f64) -> f64 {
        assert!(
            depth_ratio.is_finite() && depth_ratio > 0.0,
            "a bowl's depth ratio must be finite and positive, got {depth_ratio}"
        );
        let ratio = depth_ratio.min(self.params.depth_ratio_max);
        let slope = rim_slope(ratio);
        let over = |angle: f64| if angle > 0.0 { angle / slope } else { 0.0 };
        let (colatitude, limb) = (over(colatitude_rad), over(limb_rad));
        if colatitude + limb >= COLATITUDE_RANGE || limb >= LIMB_RANGE {
            return 0.0;
        }
        let (depth_cell, depth_at) = cell(&self.ln_depth_ratios, math::ln(ratio));
        let (colatitude_cell, colatitude_at) = cell(&self.colatitudes, colatitude);
        let (limb_cell, limb_at) = cell(&self.limbs, limb);
        let width = self.colatitudes.len();
        let mut share = 0.0;
        for (depth_step, depth_weight) in [(0, 1.0 - depth_at), (1, depth_at)] {
            for (colatitude_step, colatitude_weight) in
                [(0, 1.0 - colatitude_at), (1, colatitude_at)]
            {
                let row = &self.shares
                    [(depth_cell + depth_step) * width + colatitude_cell + colatitude_step];
                share += depth_weight
                    * colatitude_weight
                    * ((1.0 - limb_at) * row[limb_cell] + limb_at * row[limb_cell + 1]);
            }
        }
        share
    }
}

/// The cell of the ascending `nodes` that holds `x`, and `x`'s place in it in [0, 1], clamped to
/// the end cells.
#[must_use]
fn cell(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 1;
    if x <= nodes[0] {
        return (0, 0.0);
    }
    if x >= nodes[last] {
        return (last - 1, 1.0);
    }
    let i = nodes.partition_point(|&node| node <= x) - 1;
    (i, (x - nodes[i]) / (nodes[i + 1] - nodes[i]))
}

/// `value` rounded to `decimals` places.
#[must_use]
fn round_to(value: f64, decimals: i32) -> f64 {
    let scale = math::powi(10.0, decimals);
    (value * scale).round() / scale
}

/// The table on `threads` threads: one row, a depth ratio and a colatitude, a chunk, each a pure
/// function of its index, in index order.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `params` has fewer than two nodes on an axis, more than 100,000, or a deepest ratio outside
/// (0, 0.5]: none of which [`Params::of`] accepts.
pub fn fit(params: &Params, threads: NonZeroUsize) -> Result<BowlShadowFit, BuildThreadPoolError> {
    let ln_depth_ratios = even_nodes(
        math::ln(params.depth_ratio_min),
        math::ln(params.depth_ratio_max),
        params.depth_ratio_nodes,
    );
    let colatitudes = even_nodes(0.0, COLATITUDE_RANGE, params.colatitude_nodes);
    let limbs = even_nodes(0.0, LIMB_RANGE, params.limb_nodes);
    let width = colatitudes.len();
    let rows = u64::try_from(ln_depth_ratios.len() * width).expect("under 10¹⁰ rows fit a u64");
    let mut shares = Vec::with_capacity(ln_depth_ratios.len() * width);
    map_reduce_chunks(
        rows,
        1,
        threads,
        |range| {
            range
                .map(|row| {
                    let row = usize::try_from(row).expect("a row of a Vec fits a usize");
                    // exp(ln γ) may round a hair past the deepest ratio.
                    let ratio = math::exp(ln_depth_ratios[row / width]).min(params.depth_ratio_max);
                    let slope = rim_slope(ratio);
                    let limbs_rad: Vec<f64> = limbs.iter().map(|&b| b * slope).collect();
                    if row % width == 0 {
                        // The pole: eq. 6, in full.
                        limbs_rad.iter().map(|&l| polar_shadow(ratio, l)).collect()
                    } else {
                        shadow_shares(
                            ratio,
                            colatitudes[row % width] * slope,
                            &limbs_rad,
                            params.grid,
                            params.scan,
                            params.refine,
                        )
                        .into_iter()
                        .map(|s| round_to(s, params.decimals))
                        .collect::<Vec<f64>>()
                    }
                })
                .collect::<Vec<Vec<f64>>>()
        },
        |part| shares.extend(part),
    )?;
    let lattice_points = u64::try_from(half_disk(params.grid).count())
        .expect("a lattice of at most 100,000² points fits a u64");
    Ok(BowlShadowFit {
        ln_depth_ratios,
        colatitudes,
        limbs,
        shares,
        params: *params,
        lattice_points,
    })
}

/// What a run measured, for the header's acceptance line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figures {
    /// The largest difference at the pole between the rim's least declination on the run's
    /// lattice and eq. 6, over the depth ratios 0.02, 0.1 and 0.5 and Δ of 0.1, 0.25 and 0.4 of
    /// the rim's slope.
    pub polar_lattice_error: f64,
    /// The largest difference between the trilinear table and exact geometry on the run's lattice
    /// at [`traced_points`], every degree from the pole to 60° for [`CHECK_DEPTH_RATIOS`] at
    /// [`CHECK_LIMBS_DEGREES`], and the d/D 0.005 bowl at 0.2 and 0.05 of its rim slope.
    pub worst_check: Worst,
    /// The mean difference over the same points.
    pub mean_check: f64,
    /// The largest difference at the centres of the cells near the pole (the colatitude under 0.1
    /// of the rim's slope) for d/D above 0.37, where the read's error peaks: there the rim's slope
    /// changes fastest with ln(d/D).
    pub worst_corner: Worst,
    /// The largest difference between the pole's row read between its depth-ratio nodes and eq. 6
    /// itself, at the limb's nodes and its cells' centres.
    pub worst_pole_read: f64,
}

/// Where a check's largest difference fell, and the table's and the geometry's shares there.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Worst {
    /// The difference.
    pub error: f64,
    /// The bowl's d/D.
    pub depth_ratio: f64,
    /// The latitude, degrees.
    pub latitude_deg: f64,
    /// Δ = δ + α, degrees.
    pub limb_deg: f64,
    /// The table's share.
    pub table: f64,
    /// Exact geometry's share.
    pub exact: f64,
}

impl Worst {
    /// Keeps the larger of `self` and the difference at a point.
    fn update(&mut self, check: &CheckBowl, limb_deg: f64, table: f64, exact: f64) {
        let error = (table - exact).abs();
        if error > self.error {
            *self = Self {
                error,
                depth_ratio: check.depth_ratio,
                latitude_deg: check.latitude_deg,
                limb_deg,
                table,
                exact,
            };
        }
    }
}

impl std::fmt::Display for Worst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.4} (d/D {:.3}, {:.2}°, Δ {:.2}°: {:.4} against {:.4})",
            self.error, self.depth_ratio, self.latitude_deg, self.limb_deg, self.table, self.exact
        )
    }
}

/// One exact evaluation of the checks: a bowl at a latitude, at its values of Δ.
struct CheckBowl {
    depth_ratio: f64,
    latitude_deg: f64,
    limbs_deg: Vec<f64>,
}

/// The largest difference at the pole between the rim's least declination on `params`'s lattice
/// and eq. 6.
#[must_use]
fn polar_lattice_error(params: &Params) -> f64 {
    let mut worst = 0.0_f64;
    for ratio in [0.02, 0.1, 0.5] {
        let slope = rim_slope(ratio);
        let limbs = [0.1 * slope, 0.25 * slope, 0.4 * slope];
        let lattice = shadow_shares(ratio, 0.0, &limbs, params.grid, params.scan, params.refine);
        for (share, &limb) in lattice.iter().zip(&limbs) {
            worst = worst.max((share - polar_shadow(ratio, limb)).abs());
        }
    }
    worst
}

/// The checks' bowls: T7.f's points, every degree from the pole to 60° and the d/D 0.005 bowl,
/// then the centres of the cells near the pole for d/D above 0.37; and how many are the former.
#[must_use]
fn check_bowls(fit: &BowlShadowFit) -> (Vec<CheckBowl>, usize) {
    let degree = PI / 180.0;
    let mut checks: Vec<CheckBowl> = Vec::new();
    for ratio in CHECK_DEPTH_RATIOS {
        for latitude in 60..=90_u32 {
            checks.push(CheckBowl {
                depth_ratio: ratio,
                latitude_deg: f64::from(latitude),
                limbs_deg: CHECK_LIMBS_DEGREES.to_vec(),
            });
        }
    }
    for point in traced_points() {
        checks.push(CheckBowl {
            depth_ratio: point.depth_ratio,
            latitude_deg: point.latitude_deg,
            limbs_deg: vec![point.limb_deg],
        });
    }
    // The shallow bowl read from the shallowest row at its own slope.
    let shallow = rim_slope(0.005) / degree;
    checks.push(CheckBowl {
        depth_ratio: 0.005,
        latitude_deg: 90.0 - 0.2 * shallow,
        limbs_deg: vec![0.05 * shallow],
    });
    let points = checks.len();
    for cell in fit
        .ln_depth_ratios
        .windows(2)
        .filter(|w| math::exp(w[1]) > 0.37)
    {
        let ratio = math::exp(f64::midpoint(cell[0], cell[1]));
        let slope_deg = rim_slope(ratio) / degree;
        for column in fit.colatitudes.windows(2).filter(|w| w[1] <= 0.1 + 1e-12) {
            checks.push(CheckBowl {
                depth_ratio: ratio,
                latitude_deg: 90.0 - f64::midpoint(column[0], column[1]) * slope_deg,
                limbs_deg: fit
                    .limbs
                    .windows(2)
                    .map(|limb| f64::midpoint(limb[0], limb[1]) * slope_deg)
                    .collect(),
            });
        }
    }
    (checks, points)
}

/// The largest difference between the pole's row read between its depth-ratio nodes and eq. 6:
/// eight points a cell in ln(d/D), at every limb node and every limb cell's centre.
#[must_use]
fn pole_read_error(fit: &BowlShadowFit) -> f64 {
    let mut limbs: Vec<f64> = fit.limbs.clone();
    limbs.extend(fit.limbs.windows(2).map(|b| f64::midpoint(b[0], b[1])));
    let mut worst = 0.0_f64;
    for cell in fit.ln_depth_ratios.windows(2) {
        for k in 0..8_u32 {
            let ratio = math::exp(cell[0] + (cell[1] - cell[0]) * (f64::from(k) + 0.5) / 8.0);
            let slope = rim_slope(ratio);
            for &b in &limbs {
                let limb = b * slope;
                let error = (fit.interpolate(ratio, 0.0, limb) - polar_shadow(ratio, limb)).abs();
                worst = worst.max(error);
            }
        }
    }
    worst
}

/// Checks `fit` against exact geometry on its own lattice, on `threads` threads.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// Never on a target whose `usize` fits a `u64`: the checks' count and indices are converted.
pub fn figures(
    fit: &BowlShadowFit,
    threads: NonZeroUsize,
) -> Result<Figures, BuildThreadPoolError> {
    let p = fit.params;
    let degree = PI / 180.0;
    let (checks, points) = check_bowls(fit);
    let mut worst_check = Worst::default();
    let mut worst_corner = Worst::default();
    let (mut sum, mut count) = (0.0, 0_u32);
    let mut index = 0_usize;
    map_reduce_chunks(
        u64::try_from(checks.len()).expect("a few hundred checks fit a u64"),
        1,
        threads,
        |range| {
            range
                .map(|i| {
                    let check = &checks[usize::try_from(i).expect("a check's index fits a usize")];
                    let colatitude = (90.0 - check.latitude_deg) * degree;
                    let limbs: Vec<f64> = check.limbs_deg.iter().map(|d| d * degree).collect();
                    let exact = shadow_shares(
                        check.depth_ratio,
                        colatitude,
                        &limbs,
                        p.grid,
                        p.scan,
                        p.refine,
                    );
                    limbs
                        .iter()
                        .map(|&limb| fit.interpolate(check.depth_ratio, colatitude, limb))
                        .zip(exact)
                        .collect::<Vec<(f64, f64)>>()
                })
                .collect::<Vec<_>>()
        },
        |part| {
            for pairs in part {
                let check = &checks[index];
                for ((table, exact), &limb) in pairs.into_iter().zip(&check.limbs_deg) {
                    if index < points {
                        worst_check.update(check, limb, table, exact);
                        sum += (table - exact).abs();
                        count += 1;
                    } else {
                        worst_corner.update(check, limb, table, exact);
                    }
                }
                index += 1;
            }
        },
    )?;
    Ok(Figures {
        polar_lattice_error: polar_lattice_error(&p),
        worst_check,
        mean_check: sum / f64::from(count),
        worst_corner,
        worst_pole_read: pole_read_error(fit),
    })
}

/// The shares as a `pub static`, one row a line in the emitter's literals: a `static` rather than
/// the emitter's `const`, since Clippy refuses so large a `const` array (`large_const_arrays`),
/// as `stellar_fates`'s and `binary_reach`'s are.
#[must_use]
fn shadow_item(rows: &[Vec<f64>]) -> TableItem {
    let width = rows.first().map_or(0, Vec::len);
    let mut out = String::from(
        "/// The share of the opening in permanent shadow, a row for each depth ratio and\n\
         /// colatitude (depth ratio major) and a share for each limb.\n#[rustfmt::skip]\n",
    );
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "pub static SHADOW: [[f64; {width}]; {}] = [",
        rows.len()
    );
    for row in rows {
        let values: Vec<String> = row.iter().map(|&v| crate::emit::literal(v)).collect();
        // Writing to a String cannot fail.
        let _ = writeln!(out, "    [{}],", values.join(", "));
    }
    out.push_str("];\n");
    TableItem::Source(out)
}

/// The table's contents.
#[must_use]
pub fn render(fit: &BowlShadowFit) -> RustTable {
    let p = &fit.params;
    let notes = format!(
        "\
Axes: ln(d/D) at {nt} even nodes from ln {lo} to ln {hi}; the colatitude e₀ over the rim's
slope θ = 2 atan(2 d/D) at {na} even nodes on [0, 1]; Δ = δ + α, the star's greatest
declination plus its angular radius, over θ at {nb} even nodes on [0, ½]. `SHADOW` has a
row for each depth ratio and colatitude, depth ratio major, and a share for each limb. Read
it trilinear in the three axes; it is zero where (e₀ + Δ) ÷ θ ≥ 1 or Δ ÷ θ ≥ ½, a bowl
shallower than the first node takes that node's row at its own θ, and one deeper than the
last the last's.

Each share off the pole is the count of a lattice of {grid} points to the rim's radius over
the half disk, a point in shadow when the least declination of its rim directions is above
Δ (the rim scanned at {scan} points and refined by {refine} golden-section steps), rounded
to {decimals} places. The pole's row is Hayne et al.'s eq. 6 itself, in full.",
        nt = fit.ln_depth_ratios.len(),
        lo = p.depth_ratio_min,
        hi = p.depth_ratio_max,
        na = fit.colatitudes.len(),
        nb = fit.limbs.len(),
        grid = p.grid,
        scan = p.scan,
        refine = p.refine,
        decimals = p.decimals,
    );
    RustTable {
        summary: vec![
            "A spherical bowl's permanent shadow by exact ray geometry (rendering plan R09,"
                .to_owned(),
            "R09.T7.f): the share of its opening that no part of the star's disk ever reaches,"
                .to_owned(),
            "which `cold_trap` reads in place of Hayne et al. 2021's eq. 26.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![
            TableItem::Array {
                name: "LN_DEPTH_RATIO".to_owned(),
                doc: vec!["ln(d/D) at the nodes, ascending.".to_owned()],
                values: fit.ln_depth_ratios.clone(),
            },
            TableItem::Array {
                name: "COLATITUDE_PER_RIM_SLOPE".to_owned(),
                doc: vec![
                    "The colatitude over the rim's slope at the nodes, from 0 to 1, beyond which"
                        .to_owned(),
                    "the noon sun is above every part of the wall.".to_owned(),
                ],
                values: fit.colatitudes.clone(),
            },
            TableItem::Array {
                name: "LIMB_PER_RIM_SLOPE".to_owned(),
                doc: vec![
                    "The star's greatest declination plus its angular radius, over the rim's"
                        .to_owned(),
                    "slope, at the nodes, from 0 to ½, where eq. 6 leaves no polar shadow."
                        .to_owned(),
                ],
                values: fit.limbs.clone(),
            },
            shadow_item(&fit.shares),
        ],
    }
}

/// The task (R09.T7.f): fast, about 45 s on one thread, so that `check --rerun-fast` reruns it
/// and holds the committed bytes to a fresh run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BowlPermanentShadowTask;

impl FitTask for BowlPermanentShadowTask {
    fn name(&self) -> &'static str {
        "bowl_permanent_shadow"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_crate(&self) -> TableCrate {
        TableCrate::Surface
    }

    fn table_path(&self) -> &'static str {
        "bowl_permanent_shadow.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "LN_DEPTH_RATIO",
            "COLATITUDE_PER_RIM_SLOPE",
            "LIMB_PER_RIM_SLOPE",
            "SHADOW",
        ]
    }

    /// None: the fit reads no constant of the sim, only `math`.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let params = Params::of(manifest)?;
        let fit = fit(&params, threads)?;
        let figures = figures(&fit, threads)?;
        let mut acceptance = String::new();
        // Writing to a String cannot fail.
        let _ = write!(
            acceptance,
            "{nt} × {na} × {nb} nodes from {rows} bowls of {points} lattice points each (half \
             disk), Δ standing for δ + α; at the pole the lattice gives eq. 6 within {polar:.5}; \
             read trilinearly against exact geometry on the lattice, within {check} at R09.T7.f's \
             points, every degree from the pole to 60° for d/D 0.06, 0.1, 0.2 and 0.3 at Δ of \
             0–30° and a d/D 0.005 bowl (mean {mean:.5}), within {corner} at the cell centres \
             near the pole for d/D above 0.37, where the error peaks, and within {pole:.4} of \
             eq. 6 on the pole's row between its depth-ratio nodes",
            nt = fit.ln_depth_ratios.len(),
            na = fit.colatitudes.len(),
            nb = fit.limbs.len(),
            rows = fit.shares.len() - fit.ln_depth_ratios.len(),
            points = fit.lattice_points,
            polar = figures.polar_lattice_error,
            check = figures.worst_check,
            mean = figures.mean_check,
            corner = figures.worst_corner,
            pole = figures.worst_pole_read,
        );
        Ok(TaskOutput {
            table: render(&fit),
            source: "Hayne, Aharonson and Schörghofer (2021, Nat. Astron. 5, 169), eqs. 3 and 6 \
                     and Methods A (equation numbers of arXiv:2005.05369v1); the chord test of \
                     science-r09-drainage.md §8.10"
                .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_sim::units::{Degrees, Radians};
    use hyperion_surface::cold_trap;
    use std::f64::consts::FRAC_PI_2;

    fn radians(degrees: f64) -> f64 {
        Radians::from(Degrees::new(degrees)).value()
    }

    /// A bowl's permanent shadow at `colatitude_rad` under the band |dec| ≤ `limb_rad` by direct
    /// ray tracing, as `science-r09-drainage.md` §8.10's script does it: a point of a `grid` ×
    /// `grid` lattice across the opening is lit when its chord toward a sampled star direction
    /// leaves through the rim, the directions `hours` hour angles at each of `declinations`
    /// declinations spread evenly over [−Δ, Δ] (one: the upper edge alone).
    ///
    /// The chord from a floor point P along a direction s meets the sphere again at
    /// P − 2((P − C) · s) s, which is above the rim's plane exactly when the chord leaves through
    /// the opening.
    fn traced_shadow(
        depth_ratio: f64,
        colatitude_rad: f64,
        limb_rad: f64,
        grid: u32,
        hours: u32,
        declinations: u32,
    ) -> f64 {
        let bowl = Bowl::new(depth_ratio);
        let (sin_lat, cos_lat) = math::sin_cos(FRAC_PI_2 - colatitude_rad);
        let mut suns = Vec::new();
        for k in 0..declinations {
            let dec = if declinations == 1 {
                limb_rad
            } else {
                limb_rad * (2.0 * f64::from(k) / f64::from(declinations - 1) - 1.0)
            };
            let (sin_dec, cos_dec) = math::sin_cos(dec);
            for j in 0..hours {
                let (sin_h, cos_h) = math::sin_cos(2.0 * PI * f64::from(j) / f64::from(hours));
                let up = sin_lat * sin_dec + cos_lat * cos_dec * cos_h;
                if up > 0.0 {
                    suns.push([
                        -cos_dec * sin_h,
                        cos_lat * sin_dec - sin_lat * cos_dec * cos_h,
                        up,
                    ]);
                }
            }
        }
        // The highest first, which light most points soonest.
        suns.sort_by(|a, b| b[2].total_cmp(&a[2]));
        let (mut shadowed, mut total) = (0_u32, 0_u32);
        for i in 0..grid {
            let x = -1.0 + 2.0 * (f64::from(i) + 0.5) / f64::from(grid);
            for j in 0..grid {
                let y = -1.0 + 2.0 * (f64::from(j) + 0.5) / f64::from(grid);
                if x * x + y * y >= 1.0 {
                    continue;
                }
                total += 1;
                let z = bowl.floor(x * x + y * y);
                let lit = suns.iter().any(|s| {
                    // (C − P) · s, positive when the chord runs inward.
                    let inward = -x * s[0] - y * s[1] + (bowl.centre - z) * s[2];
                    inward > 0.0 && z + 2.0 * inward * s[2] > 0.0
                });
                if !lit {
                    shadowed += 1;
                }
            }
        }
        f64::from(shadowed) / f64::from(total)
    }

    /// Hayne et al.'s eq. 6 as printed: (cos 2Δ − (β ÷ 2) sin 2Δ)², held at zero where the base
    /// is not positive.
    fn hayne_eq6(ratio: f64, limb_rad: f64) -> f64 {
        let beta = 0.5 / ratio - 2.0 * ratio;
        let (s, c) = math::sin_cos(2.0 * limb_rad);
        let edge = c - 0.5 * beta * s;
        if edge > 0.0 { edge * edge } else { 0.0 }
    }

    #[test]
    fn the_pole_is_haynes_eq_6() {
        for ratio in [0.02, 0.06, 0.1, 0.2, 0.3, 0.45, 0.5] {
            let slope = rim_slope(ratio);
            for i in 0..=60_u32 {
                let limb = slope * f64::from(i) / 100.0;
                let ours = polar_shadow(ratio, limb);
                let hayne = hayne_eq6(ratio, limb);
                assert!(
                    (ours - hayne).abs() < 1e-12,
                    "d/D {ratio}, Δ {limb}: {ours} against eq. 6's {hayne}"
                );
            }
        }
        // The rim's least declination on a lattice gives the same at the pole.
        for (ratio, b) in [(0.06, 0.1), (0.1, 0.2), (0.3, 0.05), (0.5, 0.3)] {
            let limb = b * rim_slope(ratio);
            let lattice = shadow_shares(ratio, 0.0, &[limb], 200, 48, 24)[0];
            let closed = polar_shadow(ratio, limb);
            assert!(
                (lattice - closed).abs() < 2e-3,
                "d/D {ratio}: {lattice} against eq. 6's {closed}"
            );
        }
    }

    #[test]
    fn the_rims_least_declination_matches_direct_ray_tracing() {
        // science-r09-drainage.md §8.12 and decision-r09-t7f.md §1's exact figures at δ 1.54°,
        // against the chord test with the star at three declinations and 720 hour angles.
        let delta = radians(1.54);
        for (ratio, latitude, exact) in [
            (0.06, 85.0, 0.2344),
            (0.1, 80.0, 0.2240),
            (0.2, 70.0, 0.2657),
            (0.25, 60.0, 0.1738),
            (0.3, 60.0, 0.2735),
        ] {
            let colatitude = radians(90.0 - latitude);
            let rim = shadow_shares(ratio, colatitude, &[delta], 200, 48, 24)[0];
            let rays = traced_shadow(ratio, colatitude, delta, 201, 720, 3);
            assert!(
                (rim - rays).abs() < 2e-3,
                "d/D {ratio} at {latitude}°: rim {rim}, rays {rays}"
            );
            assert!(
                (rim - exact).abs() < 2e-3,
                "d/D {ratio} at {latitude}°: {rim} against {exact}"
            );
        }
        // The refined least sine against a scan of 20,000 rim points, wherever the point is not
        // already lit: never more than 10⁻⁹ above it, and never far below it.
        let bowl = Bowl::new(0.2);
        let rim = rim_scan(48);
        let fine = rim_scan(20_000);
        for colatitude in [0.0, 5.0, 10.0, 20.0] {
            for (x, y) in half_disk(16) {
                let view = RimView::new(&bowl, radians(colatitude), x, y);
                let least = view.least(&rim, 24);
                if least < 0.0 {
                    continue;
                }
                let brute = fine
                    .iter()
                    .map(|&(s, c)| view.at(s, c))
                    .fold(f64::INFINITY, f64::min);
                assert!(
                    least - brute <= 1e-9 && brute - least <= 1e-6,
                    "{colatitude}°, ({x}, {y}): {least} against {brute}"
                );
            }
        }
    }

    /// The committed table read trilinearly, cap and all, against direct ray tracing (R09.T7.f's
    /// acceptance): within 0.01 at its points.
    #[test]
    fn the_committed_table_matches_direct_ray_tracing() {
        for point in traced_points() {
            let colatitude = radians(90.0 - point.latitude_deg);
            let limb = radians(point.limb_deg);
            let table = cold_trap::bowl_permanent_shadow(
                point.depth_ratio,
                Radians::new(colatitude),
                Radians::new(limb),
            );
            let rays = traced_shadow(point.depth_ratio, colatitude, limb, 181, 720, 3);
            assert!(
                (table - rays).abs() <= 0.01,
                "{point:?}: table {table}, rays {rays}"
            );
        }
    }

    #[test]
    fn ray_tracing_finds_no_shadow_at_the_axes_ends() {
        // At Δ of half the rim's slope, and where the highest sun reaches the rim's slope.
        for ratio in [0.02, 0.1, 0.5] {
            let slope = rim_slope(ratio);
            for a in [0.1, 0.3, 0.5] {
                for limb in [0.5 * slope, (1.0 - a) * slope] {
                    let rays = traced_shadow(ratio, a * slope, limb, 121, 720, 3);
                    assert!(
                        rays.abs() < 1e-12,
                        "d/D {ratio}, e₀ {a} θ, Δ {limb}: {rays}"
                    );
                    let rim = shadow_shares(ratio, a * slope, &[limb], 64, 48, 24)[0];
                    assert!(rim.abs() < 1e-12, "d/D {ratio}, e₀ {a} θ, Δ {limb}: {rim}");
                }
            }
        }
    }

    #[test]
    fn a_shallow_bowl_reads_the_shallowest_row_at_its_own_slope() {
        // Small slopes are similar: a d/D 0.005 bowl at colatitude 0.2 θ and Δ 0.05 θ.
        let ratio = 0.005;
        let slope = rim_slope(ratio);
        let table = cold_trap::bowl_permanent_shadow(
            ratio,
            Radians::new(0.2 * slope),
            Radians::new(0.05 * slope),
        );
        let rays = traced_shadow(ratio, 0.2 * slope, 0.05 * slope, 301, 1440, 3);
        assert!((table - rays).abs() <= 2e-3, "table {table}, rays {rays}");
    }

    /// The smoke manifest.
    fn smoke() -> Manifest {
        Manifest::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("manifests/bowl_permanent_shadow.smoke.toml"),
        )
        .unwrap()
    }

    #[test]
    fn the_table_is_the_same_on_any_thread_count() {
        let params = Params::of(&smoke()).unwrap();
        let one = fit(&params, NonZeroUsize::MIN).unwrap();
        let four = fit(&params, NonZeroUsize::new(4).unwrap()).unwrap();
        assert_eq!(one.shares.len(), four.shares.len());
        for (a, b) in one
            .shares
            .iter()
            .flatten()
            .zip(four.shares.iter().flatten())
        {
            assert!(a.total_cmp(b).is_eq(), "{a} against {b}");
        }
        assert_eq!(render(&one), render(&four));
        // Every share in [0, 1], and none rises with Δ.
        for row in &one.shares {
            assert!(row.iter().all(|s| (0.0..=1.0).contains(s)), "{row:?}");
            assert!(row.windows(2).all(|w| w[1] <= w[0]), "{row:?}");
        }
        assert_eq!(
            one.lattice_points,
            u64::try_from(half_disk(params.grid).count()).unwrap()
        );
    }

    #[test]
    fn the_task_runs_its_smoke_manifest() {
        let task = BowlPermanentShadowTask;
        let output = task.run(&smoke(), NonZeroUsize::new(4).unwrap()).unwrap();
        let body = output.table.body().unwrap();
        let declared: Vec<&str> = body
            .lines()
            .filter_map(|l| {
                l.strip_prefix("pub const ")
                    .or_else(|| l.strip_prefix("pub static "))
            })
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(declared, task.items());
        assert!(
            output
                .acceptance
                .contains("at the pole the lattice gives eq. 6")
        );
        assert!(output.acceptance.contains("for d/D above 0.37"));
        let bad = Manifest::from_bytes(
            std::path::Path::new("bad.toml"),
            b"task = \"bowl_permanent_shadow\"\n[params]\ndepth_ratio_min = 0.3\n\
              depth_ratio_max = 0.2\ndepth_ratio_nodes = 2\ncolatitude_nodes = 2\n\
              limb_nodes = 2\ngrid = 8\nscan = 8\nrefine = 4\ndecimals = 4\n"
                .to_vec(),
        )
        .unwrap();
        assert!(matches!(
            task.run(&bad, NonZeroUsize::MIN),
            Err(RunTaskError::Param(_))
        ));
        assert!(task.fingerprint().is_empty());
        assert_eq!(task.table_crate(), TableCrate::Surface);
    }
}
