//! The hypervelocity row: ancient Type Ia survivors on straight lines (plan 15, P15.T6.f).
//!
//! A surviving donor leaves its supernova by the D6 mechanism (Shen et al. 2018, ApJ 865, 15):
//! slow ones from low-mass donors at 1,000–1,500 km/s, a quarter of the Type Ia rate, and fast
//! ones at 2,000–2,500 km/s, a few per cent (El-Badry et al. 2023, Open Journal of Astrophysics 6, §8.2; ruling 128.1),
//! the sim's `class_table::SURVIVOR_POPULATIONS`. Both are far above most escape speeds and
//! cross the cube in 10–20 Myr on straight lines. In a
//! steady state, objects launched isotropically at speed v from a source of rate density q(x′)
//! have the density `n(x) = ∫ q(x′) ÷ (4π |x − x′|² v) d³x′`, the sources convolved with
//! `1 ÷ (4π r² v)`. Only sources inside the cube are taken: survivors launched from beyond it can
//! cross it too, but the populations beyond hold under about 1% of the rate (the disc's mass beyond
//! 20 kpc is about 0.4%).
//!
//! The sources are the galaxy's populations weighted by their Type Ia rate per system: each field
//! component's density times the mean over its age distribution of the delay-time distribution
//! `t^−1.1` from 40 Myr (Maoz and Graur 2017, ApJ 848, 25), the ancient events' share of every
//! population, the old ones' most of all. The azimuthal integral is closed,
//! `∫ dφ ÷ (A − B cos φ) = 2π ÷ √(A² − B²)` with `A = R² + R′² + (z − z′)²` and `B = 2RR′`,
//! softened at a hundredth of the source's distance from the centre where the kernel's
//! logarithmic singularity meets a node; the sources are axisymmetrised over four azimuths.
//!
//! The form is then one `CoredPowerLaw`, fitted to `n`'s cell integrals by the misplaced share as
//! the other forms are; on straight lines in a steady state the shape does not depend on v, so
//! one form serves both populations. The number inside the cube is, for each population, the Type
//! Ia rate times its share times the mean path to the cube's face (the rate-weighted mean over the
//! sources along isotropic directions) over its `1 ÷ ⟨1 ÷ v⟩`. Where a slow survivor launches
//! below 1.5 times the local escape speed the straight line is poor; that share is reported.

use std::f64::consts::PI;
use std::num::NonZeroUsize;

use hyperion_sim::coords::ROOT_HALF_WIDTH_LY;
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::displaced::class_table::{
    MILKY_WAY_IA_RATE_PER_YEAR, SURVIVOR_POPULATIONS, survivors_inside,
};
use hyperion_sim::galaxy::displaced::forms::CoredPowerLawParams;
use hyperion_sim::galaxy::fields::{Fields, MAX_COMPONENTS};
use hyperion_sim::math;

use super::cells::{RzCells, map_in_order, normalise, total_variation};
use super::fit_disc::{EDGE_FRACTION, spheroid_shares};
use super::fit_old::{SPHEROID_BOXES, START};
use super::histogram::{R_CELLS, Z_CELLS};
use super::nelder_mead::{Stop, minimise};
use crate::parallel::BuildThreadPoolError;

/// The delay-time distribution's slope (Maoz and Graur 2017).
pub const DTD_SLOPE: f64 = -1.1;

/// The shortest delay, years (Maoz and Graur 2017).
pub const DTD_MIN_DELAY_YEARS: f64 = 4e7;

/// The age quantiles each component's delay average takes.
const AGE_QUANTILES: u32 = 256;

/// The azimuths, over a half-turn, the sources are averaged over (every density here is
/// symmetric under a half-turn).
const AZIMUTHS: [f64; 4] = [0.0, 0.25 * PI, 0.5 * PI, 0.75 * PI];

/// The hypervelocity row's inputs, from the manifest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HypervelocitySettings {
    /// Gauss–Legendre nodes per doubling panel of the source grid.
    pub panel_nodes: usize,
    /// Isotropic directions of the residence time's average.
    pub directions: usize,
    /// The most evaluations of the fit.
    pub evaluations: usize,
    /// The simplex's tolerance.
    pub tolerance: f64,
}

/// The row and its figures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HypervelocityFit {
    /// The form, weight 1.
    pub form: CoredPowerLawParams,
    /// Its misplaced share against the convolution.
    pub misplaced: f64,
    /// Parameters on a box edge.
    pub at_edge: usize,
    /// The rate-weighted mean path from launch to the cube's face, ly.
    pub mean_exit_ly: f64,
    /// The survivors inside the cube at any time, slow and fast ([`SURVIVOR_POPULATIONS`]).
    pub inside: [f64; 2],
    /// The rate-weighted share of the slow population's launches below 1.5 times the local
    /// escape speed, where a straight line is a poor path (ruling 128.1's finding).
    pub slow_near_escape: f64,
}

/// Each component's Type Ia rate per system, up to one constant: the mean of `t^−1.1` over its
/// age distribution from 40 Myr, by quantile midpoints.
fn rate_per_system(fields: &Fields) -> Vec<f64> {
    fields
        .components()
        .iter()
        .map(|c| {
            let ages = c.ages();
            let sum: f64 = (0..AGE_QUANTILES)
                .map(|k| {
                    let u = (f64::from(k) + 0.5) / f64::from(AGE_QUANTILES);
                    let t = ages.quantile(u).value();
                    if t < DTD_MIN_DELAY_YEARS {
                        0.0
                    } else {
                        math::exp(DTD_SLOPE * math::ln(t / 1e9))
                    }
                })
                .sum();
            sum / f64::from(AGE_QUANTILES)
        })
        .collect()
}

/// The nodes of `[0, end]` on doubling panels down to `first`, with a first panel from 0: `(x,
/// weight)`.
fn doubling_nodes(first: f64, end: f64, rule: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut edges = vec![0.0, first];
    while *edges.last().expect("two edges") < end {
        let next = (2.0 * edges.last().expect("two edges")).min(end);
        edges.push(next);
    }
    let mut out = Vec::new();
    for w in edges.windows(2) {
        let (half, mid) = (0.5 * (w[1] - w[0]), f64::midpoint(w[0], w[1]));
        out.extend(rule.iter().map(|&(x, wt)| (mid + half * x, wt * half)));
    }
    out
}

/// The Gauss–Legendre rule of `n` nodes: 4, or 8 for any other (the manifest admits only those).
fn rule(n: usize) -> Vec<(f64, f64)> {
    use hyperion_sim::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS, GL8_NODES, GL8_WEIGHTS};
    if n <= 4 {
        GL4_NODES.iter().copied().zip(GL4_WEIGHTS).collect()
    } else {
        GL8_NODES.iter().copied().zip(GL8_WEIGHTS).collect()
    }
}

/// The distance from `x` to the cube's face along the unit vector `u`, ly.
fn exit_distance(x: [f64; 3], u: [f64; 3], l: f64) -> f64 {
    let mut d = f64::INFINITY;
    for i in 0..3 {
        if u[i] > 0.0 {
            d = d.min((l - x[i]) / u[i]);
        } else if u[i] < 0.0 {
            d = d.min((-l - x[i]) / u[i]);
        }
    }
    d.max(0.0)
}

/// A source node: `(R, |z|, weight)`, ly, its weight the rate density times its volume element
/// in `R′ dR′ dz′`.
type SourceNode = (f64, f64, f64);

/// The sources' nodes in the galaxy of `fields` (module documentation), axisymmetrised.
fn source_nodes(fields: &Fields, panel_nodes: usize) -> Vec<SourceNode> {
    let half_width = f64::from(ROOT_HALF_WIDTH_LY);
    let per_system = rate_per_system(fields);
    let rule = rule(panel_nodes);
    let r_nodes = doubling_nodes(1.0, half_width, &rule);
    let z_nodes = doubling_nodes(0.1, half_width, &rule);
    let mut densities = [0.0; MAX_COMPONENTS];
    let mut out = Vec::with_capacity(r_nodes.len() * z_nodes.len());
    for &(radius, r_weight) in &r_nodes {
        for &(height, z_weight) in &z_nodes {
            let mut rate = 0.0;
            for &phi in &AZIMUTHS {
                let (sin, cos) = math::sin_cos(phi);
                fields.densities(
                    &PointLy::new(radius * cos, radius * sin, height),
                    &mut densities,
                );
                rate += densities
                    .iter()
                    .zip(&per_system)
                    .map(|(d, w)| d * w)
                    .sum::<f64>();
            }
            #[expect(clippy::cast_precision_loss, reason = "four azimuths")]
            let rate = rate / AZIMUTHS.len() as f64;
            if rate > 0.0 {
                out.push((radius, height, rate * radius * r_weight * z_weight));
            }
        }
    }
    out
}

/// The survivors' density, up to one constant, at `(radius, height)` ly: the sources convolved
/// with `1 ÷ r²`, both signs of the sources' heights, azimuth in closed form.
fn convolved(sources: &[SourceNode], radius: f64, height: f64) -> f64 {
    sources
        .iter()
        .map(|&(source_r, source_z, weight)| {
            let soft = 0.01 * (source_r + source_z + 1.0);
            let cross = 2.0 * radius * source_r;
            let kernel = |dz: f64| {
                let sum = radius * radius + source_r * source_r + dz * dz;
                let gap = (source_r - radius) * (source_r - radius) + dz * dz + soft * soft;
                2.0 * PI / (gap * (sum + cross)).sqrt()
            };
            weight * (kernel(height - source_z) + kernel(height + source_z))
        })
        .sum()
}

/// The rate-weighted mean distance from the sources inside the cube to its face along `count`
/// isotropic (Fibonacci) directions, ly.
fn mean_exit_distance(sources: &[SourceNode], count: usize) -> f64 {
    let half_width = f64::from(ROOT_HALF_WIDTH_LY);
    let golden = PI * (3.0 - 5.0_f64.sqrt());
    let count = count.max(1);
    #[expect(clippy::cast_precision_loss, reason = "a few hundred directions")]
    let directions: Vec<[f64; 3]> = (0..count)
        .map(|i| {
            let up = 1.0 - (2.0 * i as f64 + 1.0) / count as f64;
            let across = (1.0 - up * up).max(0.0).sqrt();
            let (sin, cos) = math::sin_cos(golden * i as f64);
            [across * cos, across * sin, up]
        })
        .collect();
    let (mut weight, mut distance) = (0.0, 0.0);
    for &(radius, height, rate) in sources {
        for &phi in &AZIMUTHS {
            let (sin, cos) = math::sin_cos(phi);
            let at = [radius * cos, radius * sin, height];
            if at.iter().any(|c| c.abs() > half_width) {
                continue;
            }
            #[expect(clippy::cast_precision_loss, reason = "a few hundred directions")]
            let mean = directions
                .iter()
                .map(|&u| exit_distance(at, u, half_width))
                .sum::<f64>()
                / directions.len() as f64;
            weight += rate;
            distance += rate * mean;
        }
    }
    if weight > 0.0 { distance / weight } else { 0.0 }
}

/// The rate-weighted share of the slow survivors' launches, uniform in speed over their band, that
/// leave slower than 1.5 times the escape speed at the source (`escape` km/s at `(R, |z|)` ly).
fn slow_near_escape(sources: &[SourceNode], escape: impl Fn(f64, f64) -> f64) -> f64 {
    let slow = SURVIVOR_POPULATIONS[0];
    let (mut weight, mut near) = (0.0, 0.0);
    for &(radius, height, rate) in sources {
        let limit = 1.5 * escape(radius, height);
        let share = ((limit - slow.min_km_s) / (slow.max_km_s - slow.min_km_s)).clamp(0.0, 1.0);
        weight += rate;
        near += rate * share;
    }
    if weight > 0.0 { near / weight } else { 0.0 }
}

/// Fits the hypervelocity row in the galaxy of `fields`, lengths in `R_d` of `r_d` ly, on the
/// cells `cells`, on `threads` threads; `escape` gives the escape speed, km/s, at `(R, |z|)` ly.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// Never: the density is computed at every node of the cells, in the order they are integrated.
pub fn fit_hypervelocity(
    fields: &Fields,
    r_d: f64,
    cells: &RzCells,
    settings: HypervelocitySettings,
    escape: impl Fn(f64, f64) -> f64,
    threads: NonZeroUsize,
) -> Result<HypervelocityFit, BuildThreadPoolError> {
    let sources = source_nodes(fields, settings.panel_nodes);
    let points: Vec<(f64, f64)> = cells
        .r
        .iter()
        .flat_map(|&(_, r, _)| cells.z.iter().map(move |&(_, z, _)| (r * r_d, z * r_d)))
        .collect();
    let at_nodes = map_in_order(points.len(), threads, |i| {
        convolved(&sources, points[i].0, points[i].1)
    })?;
    let mut observed = vec![0.0; R_CELLS * Z_CELLS];
    let mut next = at_nodes.iter();
    cells.integrate(
        |_, _| {
            *next
                .next()
                .expect("one value per node, in the cells' order")
        },
        &mut observed,
    );
    normalise(&mut observed);
    let mut model = vec![0.0; R_CELLS * Z_CELLS];
    let found = minimise(
        |p| {
            spheroid_shares(cells, p[0], p[1], p[2], &mut model);
            total_variation(&model, &observed)
        },
        &SPHEROID_BOXES,
        &START[..3],
        0.3,
        Stop {
            evaluations: settings.evaluations,
            tolerance: settings.tolerance,
        },
    );
    let fitted = &found.values;
    spheroid_shares(cells, fitted[0], fitted[1], fitted[2], &mut model);
    let mean_exit_ly = mean_exit_distance(&sources, settings.directions);
    Ok(HypervelocityFit {
        form: CoredPowerLawParams {
            weight: 1.0,
            a: fitted[0],
            q: fitted[1],
            gamma: fitted[2],
        },
        misplaced: total_variation(&model, &observed),
        at_edge: (0..3)
            .filter(|&i| SPHEROID_BOXES[i].at_edge(fitted[i], EDGE_FRACTION))
            .count(),
        mean_exit_ly,
        inside: survivors_inside(MILKY_WAY_IA_RATE_PER_YEAR, mean_exit_ly),
        slow_near_escape: slow_near_escape(&sources, escape),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_survivor_at_the_centre_crosses_half_the_cube() {
        let l = 10.0;
        assert!((exit_distance([0.0; 3], [1.0, 0.0, 0.0], l) - 10.0).abs() < 1e-12);
        let d = exit_distance([5.0, 0.0, 0.0], [-1.0, 0.0, 0.0], l);
        assert!((d - 15.0).abs() < 1e-12);
        // One source in the plane: the density is even in z and falls away from it as 1 ÷ d².
        let source = [(1_000.0, 0.0, 1.0)];
        let (up, down) = (
            convolved(&source, 0.0, 500.0),
            convolved(&source, 0.0, -500.0),
        );
        assert!((up / down - 1.0).abs() < 1e-12);
        let (near, far) = (
            convolved(&source, 0.0, 10_000.0),
            convolved(&source, 0.0, 20_000.0),
        );
        assert!((near / far - 4.0).abs() < 0.05, "{near} {far}");
        let nodes = doubling_nodes(1.0, 64.0, &rule(4));
        let length: f64 = nodes.iter().map(|&(_, w)| w).sum();
        assert!((length - 64.0).abs() < 1e-12);
    }
}
