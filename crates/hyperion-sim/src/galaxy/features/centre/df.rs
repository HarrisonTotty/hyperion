//! The isotropic distribution function of a tracer in the centre's potential, by Eddington
//! inversion, and the density that is its integral (plan 09, P09.T24.b and T24.c).
//!
//! # The inversion
//!
//! For a tracer of density `n(r)` in the relative potential `Ψ(r)` its members move in, the black
//! hole's, the cluster's and the rest of the galaxy's spherical average ([`CentreProfile::psi`];
//! ruling 144's joint revision), Eddington's formula (Binney and Tremaine 2008, eq. 4.46b) is
//!
//! `f(E) = (1 ÷ √8π²) [∫₀^E (d²n ÷ dΨ²) dΨ ÷ √(E − Ψ) + (dn ÷ dΨ)₀ ÷ √E]`.
//!
//! It is taken at the energies `E_j = Ψ(r_j)` of [`RADII`] = 256 radii even in `ln r` from
//! [`CORE_RADIUS`] (10⁻³ ly) to 10⁶ ly. `d²n ÷ dΨ²` is in closed form from the shape's slope and
//! curvature and the potential's first two derivatives, and the integral is taken over radius,
//! `∫_{r_j}^∞ … |dΨ ÷ d ln r| d ln r ÷ √(E_j − Ψ)`, in 16-node Gauss–Legendre panels between the
//! grid's radii out to 10⁷ ly. On the first panel, where `√(E_j − Ψ)` vanishes, `ln r = ln r_j +
//! t²` removes the singularity (the substitution of eq. 4.46's derivation), and beyond 10⁷ ly the
//! boundary term is taken there. f must be positive at every node: a profile that no isotropic
//! cluster can have is a [`BuildCentreError::NegativeDistribution`], not a panic.
//!
//! # The cut and the density
//!
//! f is zero above `E₀ = Ψ(r_c)` (the profile module's documentation), `r_c` the tracer's cut
//! radius ([`TracerProfile::cut_radius`]): the core radius of 10⁻³ ly for most tracers, 0.1 ly for
//! the young disc's inner edge, taken at the first grid radius at or outside it. No orbit is bound
//! more tightly than a circular one there. Outside that radius the cut changes nothing, and inside
//! it the density falls as `Ψ^½ ∝ r^−½`. f is also zero below `E_out = Ψ(r_out)`, the last grid
//! radius inside [`OUTER_CUT`] = 8,192 ly, 64 times the reach: no member's orbit reaches beyond
//! it. In the galaxy's potential the energies of orbits that stay near the cluster are a thin
//! slice just under Ψ at the centre, since the galaxy's well is several times deeper than the
//! cluster's, and the sampler proposes only over energies with members; the orbits the cut drops
//! (the stars' share beyond 8,192 ly under a slope of 5.5 from 100 ly is 10⁻⁶) move the density
//! inside the reach by some 10⁻⁵, through the normalisation; at 1,024 ly they moved it 5 × 10⁻⁴.
//! f is found at the grid's energies from `E₀` down to `E_out`, and only those must be positive:
//! the shape inside the cut radius does not enter them. **The
//! density used everywhere from here on is the integral of f**, `n_f(r) = 4π ∫_{E_out}^{min(Ψ,
//! E₀)} f(E) √(2(Ψ − E)) dE`, tabulated at the grid's radii to `r_out` and 64 more inside the core,
//! so positions and velocities agree by construction; it is zero from `r_out` out. f is scaled so
//! that `n_f` holds a total of one: what the cut keeps inside `r_c` in place of the profile's
//! share there (a few parts in 10⁸ for the stars), and the orbits the outer cut drops.
//! Between nodes `ln f` is a cubic Hermite interpolant in `ln E` with five-point slopes, since f
//! dips near a break of α = 10 and monotone slopes go flat there (they left 4.2 × 10⁻⁴ in the
//! density's integral), and `ln n_f` one in `ln r` with the profile's exact slopes from the cut
//! radius out and Fritsch and Butland's inside it.
//!
//! # The velocity sampler (P09.T24.c)
//!
//! At radius r the speed has the density `v² f(Ψ − v² ÷ 2) dv`, which in the energy `E = Ψ − v² ÷
//! 2` is `v f(E) dE`, `v = √(2(Ψ − E))`, over the energies with members, `E_out` to `min(Ψ, E₀)`.
//! The sampler proposes E from a piecewise-constant envelope of that density over the energy
//! grid's panels: on each, the interpolant's greatest f (tabulated per panel from its cubic, whose
//! extrema are the roots of a quadratic) times the greatest `v`, at the panel's lower energy. One
//! uniform picks the panel and the energy in it by inverse transform, and one mark accepts
//! against `v f(E)` over the panel's height. That is exact by the thinning theorem. The plan's
//! Beta proposal in `v² ÷ v_esc²`, `Beta(3⁄2, γ − ½)`, matches the Kepler regime but not the
//! galaxy's potential, in which the members' energies are a thin slice just under Ψ: at 30 ly it
//! accepted no proposal in 4,096. The direction is isotropic. Words: the direction's two,
//! then two per attempt (the proposal's uniform and the mark), at most [`MAX_ATTEMPTS`] attempts.

use crate::math;
use crate::rng::{Stream, Threshold};
use crate::stellar::draws::isotropic;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};

use super::profile::{BuildCentreError, CORE_RADIUS, CentreProfile, TracerProfile};
use crate::units::{KilometresPerSecond, LightYears};

/// The radii of the energy grid, from the core radius to 10⁶ ly (P09.T24.b).
pub const RADII: usize = 256;

/// Radii of the density table inside the core radius, at the grid's spacing: to about 5.5 × 10⁻⁶
/// ly.
const INNER_RADII: usize = 64;

/// The grid's outermost radius, ly, where f's table ends.
const TABLE_OUTER: f64 = 1e6;

/// Integration continues to about 10⁷ ly, this many panels past the table.
const EXTRA_PANELS: usize = 29;

/// The most attempts the velocity sampler makes before it gives up.
pub const MAX_ATTEMPTS: u32 = 4_096;

/// The outer energy cut's radius, ly: f is zero below the energy of the last grid radius inside
/// it (module documentation). Sixty-four times the reach.
pub const OUTER_CUT: LightYears = LightYears::new(8_192.0);

/// Out to this radius the realised density's logarithmic slope is the profile's exactly, ly: the
/// outer cut lowers the density beyond it (module documentation). Eight times the reach.
const EXACT_SLOPES_TO: f64 = 1_024.0;

/// `1 ÷ (√8 π²)`.
fn eddington_factor() -> f64 {
    1.0 / (8.0_f64.sqrt() * core::f64::consts::PI * core::f64::consts::PI)
}

/// A monotone cubic Hermite interpolant through increasing `x` (Fritsch and Butland's slopes),
/// linear beyond the ends.
#[derive(Debug, Clone, PartialEq)]
struct Hermite {
    x: Vec<f64>,
    y: Vec<f64>,
    d: Vec<f64>,
}

impl Hermite {
    fn new(x: Vec<f64>, y: Vec<f64>) -> Self {
        let n = x.len();
        let secant: Vec<f64> = (0..n - 1)
            .map(|k| (y[k + 1] - y[k]) / (x[k + 1] - x[k]))
            .collect();
        let mut d = vec![0.0; n];
        d[0] = secant[0];
        d[n - 1] = secant[n - 2];
        for k in 1..n - 1 {
            let (s0, s1) = (secant[k - 1], secant[k]);
            if s0 * s1 > 0.0 {
                let (h0, h1) = (x[k] - x[k - 1], x[k + 1] - x[k]);
                d[k] = 3.0 * (h0 + h1) / ((2.0 * h1 + h0) / s0 + (h1 + 2.0 * h0) / s1);
            }
        }
        Self { x, y, d }
    }

    /// A cubic Hermite interpolant through increasing `x` whose slopes are the derivatives of the
    /// Lagrange polynomial through the five nearest points (three at the ends, the parabolic
    /// estimate): fourth-order where the data turn, where Fritsch and Butland's slopes go flat,
    /// and not monotone.
    fn five_point(x: Vec<f64>, y: Vec<f64>) -> Self {
        let count = x.len();
        let slopes = (0..count)
            .map(|at| {
                let (lo, hi) = if at >= 2 && at + 2 < count {
                    (at - 2, at + 2)
                } else {
                    (at.saturating_sub(1), (at + 1).min(count - 1))
                };
                // The Lagrange basis's derivatives at x[at], for the points lo..=hi.
                let mut slope = 0.0;
                for point in lo..=hi {
                    let weight = if point == at {
                        (lo..=hi)
                            .filter(|&other| other != at)
                            .map(|other| 1.0 / (x[at] - x[other]))
                            .sum::<f64>()
                    } else {
                        let mut weight = 1.0 / (x[point] - x[at]);
                        for other in (lo..=hi).filter(|&other| other != point && other != at) {
                            weight *= (x[at] - x[other]) / (x[point] - x[other]);
                        }
                        weight
                    };
                    slope += weight * y[point];
                }
                slope
            })
            .collect();
        Self { x, y, d: slopes }
    }

    /// The greatest value of `eval(t) + c t` over the interval `[x[k], x[k + 1]]`: at its ends or
    /// where the cubic's derivative, a quadratic in the interval's position, vanishes inside it.
    fn interval_max(&self, k: usize, c: f64) -> f64 {
        let (start, width) = (self.x[k], self.x[k + 1] - self.x[k]);
        let (y0, y1) = (self.y[k], self.y[k + 1]);
        let (d0, d1) = (width * self.d[k], width * self.d[k + 1]);
        let at = |pos: f64| {
            let (pos2, pos3) = (pos * pos, pos * pos * pos);
            (2.0 * pos3 - 3.0 * pos2 + 1.0) * y0
                + (pos3 - 2.0 * pos2 + pos) * d0
                + (-2.0 * pos3 + 3.0 * pos2) * y1
                + (pos3 - pos2) * d1
                + c * (start + pos * width)
        };
        // The derivative in the position: quad pos² + lin pos + constant.
        let quad = 6.0 * y0 + 3.0 * d0 - 6.0 * y1 + 3.0 * d1;
        let lin = -6.0 * y0 - 4.0 * d0 + 6.0 * y1 - 2.0 * d1;
        let constant = d0 + c * width;
        let mut best = at(0.0).max(at(1.0));
        let mut consider = |pos: f64| {
            if pos > 0.0 && pos < 1.0 {
                best = best.max(at(pos));
            }
        };
        if quad.abs() > 1e-300 {
            let disc = lin * lin - 4.0 * quad * constant;
            if disc >= 0.0 {
                let root = disc.sqrt();
                consider((-lin + root) / (2.0 * quad));
                consider((-lin - root) / (2.0 * quad));
            }
        } else if lin.abs() > 1e-300 {
            consider(-constant / lin);
        }
        best
    }

    /// The interval `k` with `x[k] ≤ t ≤ x[k + 1]`, clamped to the ends, by bisection.
    fn interval(&self, t: f64) -> usize {
        let (mut lo, mut hi) = (0, self.x.len() - 1);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.x[mid] <= t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    #[expect(
        clippy::many_single_char_names,
        reason = "the Hermite basis in its usual symbols"
    )]
    fn eval(&self, t: f64) -> f64 {
        let n = self.x.len();
        if t <= self.x[0] {
            return self.y[0] + self.d[0] * (t - self.x[0]);
        }
        if t >= self.x[n - 1] {
            return self.y[n - 1] + self.d[n - 1] * (t - self.x[n - 1]);
        }
        let k = self.interval(t);
        let h = self.x[k + 1] - self.x[k];
        let s = (t - self.x[k]) / h;
        let (s2, s3) = (s * s, s * s * s);
        (2.0 * s3 - 3.0 * s2 + 1.0) * self.y[k]
            + (s3 - 2.0 * s2 + s) * h * self.d[k]
            + (-2.0 * s3 + 3.0 * s2) * self.y[k + 1]
            + (s3 - s2) * h * self.d[k + 1]
    }
}

/// One quadrature node of the energy grid: its radius, `Ψ`, `dΨ ÷ dr`, `d²Ψ ÷ dr²` and its
/// weight for `d ln r` (or, on a singular panel, for `dt`, the substitution's `2t` included).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Node {
    r: f64,
    psi: f64,
    slope: f64,
    curvature: f64,
    weight: f64,
}

impl Node {
    fn at(profile: &CentreProfile, r: f64, weight: f64) -> Self {
        Self {
            r,
            psi: profile.psi(r),
            slope: profile.psi_slope(r),
            curvature: profile.psi_curvature(r),
            weight,
        }
    }

    /// `d²n ÷ dΨ² × |dΨ ÷ d ln r|` for `tracer` here: the integrand of Eddington's formula over
    /// `ln r`, from the shape's slope and curvature and the potential's two derivatives.
    fn integrand(&self, tracer: &TracerProfile) -> f64 {
        let r = self.r;
        let (s, ls, curv) = tracer.shape().derivatives(r);
        let n = s / tracer.shape_total();
        let d1 = n * ls / r;
        let d2 = n * (ls * ls - ls + curv) / (r * r);
        let (p1, p2) = (self.slope, self.curvature);
        let f2 = (d2 * p1 - d1 * p2) / (p1 * p1 * p1);
        f2 * (-p1 * r)
    }
}

/// The radii, energies and quadrature nodes of the inversion, which depend on the potential
/// alone: the three tracers of Design note 14 share one (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyGrid {
    radii: Vec<f64>,
    psi: Vec<f64>,
    step: f64,
    /// The regular panels between consecutive radii.
    regular: Vec<[Node; 16]>,
    /// The singular first panel above each of the first [`RADII`] radii.
    singular: Vec<[Node; 16]>,
    /// The outermost radius, its `Ψ` and its `dΨ ÷ dr`, for the boundary term.
    outer: (f64, f64, f64),
}

impl EnergyGrid {
    /// The first of the [`RADII`] radii at or outside `r` ly (within 10⁻⁹ of a step), and never
    /// past the last but one.
    fn first_at_or_outside(&self, r: f64) -> usize {
        let ln_r0 = math::ln(self.radii[0]);
        let at = ((math::ln(r) - ln_r0) / self.step - 1e-9)
            .ceil()
            .clamp(0.0, 254.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is clamped to [0, 254] and whole"
        )]
        let j = at as usize;
        j.min(RADII - 2)
    }

    /// The last of the [`RADII`] radii at or inside `r` ly (within 10⁻⁹ of a step), and never
    /// before the second.
    fn last_at_or_inside(&self, r: f64) -> usize {
        let ln_r0 = math::ln(self.radii[0]);
        let at = ((math::ln(r) - ln_r0) / self.step + 1e-9)
            .floor()
            .clamp(1.0, 255.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is clamped to [1, 255] and whole"
        )]
        let j = at as usize;
        j.min(RADII - 1)
    }

    /// The grid of `profile`'s potential.
    #[must_use]
    pub fn new(profile: &CentreProfile) -> Self {
        let total = RADII + EXTRA_PANELS;
        #[expect(clippy::cast_precision_loss, reason = "255 is exact")]
        let step = math::ln(TABLE_OUTER / CORE_RADIUS.value()) / (RADII - 1) as f64;
        let ln_r0 = math::ln(CORE_RADIUS.value());
        #[expect(clippy::cast_precision_loss, reason = "indices below 400 are exact")]
        let radii: Vec<f64> = (0..total)
            .map(|j| math::exp(ln_r0 + step * j as f64))
            .collect();
        let psi: Vec<f64> = radii.iter().map(|&r| profile.psi(r)).collect();
        let half = 0.5 * step;
        let regular = (0..total - 1)
            .map(|j| {
                let mid = math::ln(radii[j]) + half;
                std::array::from_fn(|n| {
                    Node::at(
                        profile,
                        math::exp(mid + half * GL16_NODES[n]),
                        GL16_WEIGHTS[n] * half,
                    )
                })
            })
            .collect();
        // ln r = ln r_j + t², t on [0, √Δ].
        let th = 0.5 * step.sqrt();
        let singular = (0..RADII)
            .map(|j| {
                std::array::from_fn(|n| {
                    let t = th + th * GL16_NODES[n];
                    Node::at(
                        profile,
                        radii[j] * math::exp(t * t),
                        GL16_WEIGHTS[n] * th * 2.0 * t,
                    )
                })
            })
            .collect();
        let r_out = radii[total - 1];
        Self {
            outer: (r_out, psi[total - 1], profile.psi_slope(r_out)),
            radii,
            psi,
            step,
            regular,
            singular,
        }
    }
}

/// The isotropic distribution function of one tracer in the centre's potential, with the density
/// that is its integral (module documentation).
///
/// f is per cubic light-year per (km/s)³ for a tracer of total one: a member's phase-space
/// density.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::centre::{CentreProfile, DistributionFunction};
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let profile = CentreProfile::from_params(&GalaxyParams::milky_way_like())?;
/// let df = DistributionFunction::invert(profile.stars(), &profile)?;
/// // Outside the core the density is the profile's.
/// let (n, want) = (df.density(3.0), profile.stars().density(3.0));
/// assert!((n / want - 1.0).abs() < 1e-3);
/// // Near the black hole the stars move at some 500 km/s along each axis.
/// let sigma = df.dispersion(&profile, 0.1);
/// assert!((450.0..560.0).contains(&sigma), "{sigma}");
/// # Ok::<(), hyperion_sim::galaxy::features::centre::BuildCentreError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DistributionFunction {
    /// `E_j`, falling with j, (km/s)².
    energies: Vec<f64>,
    /// `f_j`, positive.
    values: Vec<f64>,
    /// `ln f` against `ln E`, increasing in `ln E`.
    log_f: Hermite,
    /// Per energy panel `q` (between `E_{q+1}` and `E_q`), its 16 nodes' energies and weights for
    /// `dE`, and f there.
    panel_energy: Vec<[f64; 16]>,
    panel_weight: Vec<[f64; 16]>,
    panel_f: Vec<[f64; 16]>,
    /// `ln n_f` against `ln r`.
    log_density: Hermite,
    /// `∫ f dE` over the energies with members.
    energy_integral: f64,
    /// The factor f was scaled by for a total of one.
    scale: f64,
    /// The innermost tabulated radius and its density, for the `r^−½` below.
    inner: (f64, f64),
    /// The outermost tabulated radius: the density is zero beyond it.
    outer: f64,
    /// Whether the tracer is cut at the core radius, f positive at every energy up to `Ψ(10⁻³
    /// ly)`.
    cut_at_core: bool,
    /// The share of the realised density inside each tabulated radius.
    cumulative: Hermite,
    /// Per energy panel `q`, the interpolant's greatest f over it (with a margin for rounding):
    /// the sampler's envelope.
    panel_max: Vec<f64>,
}

impl DistributionFunction {
    /// The distribution function of `tracer` in `profile`'s potential (module documentation).
    ///
    /// # Errors
    ///
    /// [`BuildCentreError::NegativeDistribution`] with the radius whose energy it fails at, if f
    /// is not positive at every node: no isotropic cluster has `tracer`'s profile.
    pub fn invert(
        tracer: &TracerProfile,
        profile: &CentreProfile,
    ) -> Result<Self, BuildCentreError> {
        Self::invert_on(&EnergyGrid::new(profile), tracer, profile)
    }

    /// [`invert`](Self::invert) on a grid already built for `profile`'s potential.
    ///
    /// # Errors
    ///
    /// As [`invert`](Self::invert).
    #[expect(
        clippy::too_many_lines,
        reason = "the inversion, the energy panels and the density table in the documented order"
    )]
    pub fn invert_on(
        grid: &EnergyGrid,
        tracer: &TracerProfile,
        profile: &CentreProfile,
    ) -> Result<Self, BuildCentreError> {
        let (radii, psi, step) = (&grid.radii, &grid.psi, grid.step);
        let ln_r0 = math::ln(CORE_RADIUS.value());
        // The cut's node: f is found from its energy down, to the outer cut's.
        let first = grid.first_at_or_outside(tracer.cut_radius().value());
        let last = grid.last_at_or_inside(OUTER_CUT.value());
        let nodes = last + 1 - first;
        let regular: Vec<[(f64, f64); 16]> = grid
            .regular
            .iter()
            .map(|panel| panel.map(|n| (n.psi, n.weight * n.integrand(tracer))))
            .collect();
        let (outer_r, outer_psi, outer_p1) = grid.outer;
        let outer_slope = {
            let n = tracer.density(outer_r);
            let d1 = n * tracer.shape().log_slope(outer_r) / outer_r;
            d1 / outer_p1
        };
        let mut values = Vec::with_capacity(nodes);
        for j in first..=last {
            let e = psi[j];
            let mut sum = 0.0;
            for node in &grid.singular[j] {
                sum += node.weight * node.integrand(tracer) / (e - node.psi).sqrt();
            }
            for panel in &regular[j + 1..] {
                for &(p, w) in panel {
                    sum += w / (e - p).sqrt();
                }
            }
            sum += outer_slope / (e - outer_psi).sqrt();
            let f = eddington_factor() * sum;
            if !(f > 0.0 && f.is_finite()) {
                return Err(BuildCentreError::NegativeDistribution(radii[j]));
            }
            values.push(f);
        }
        let energies: Vec<f64> = psi[first..=last].to_vec();
        // Five-point slopes: f dips near the break at α = 10, and Fritsch and Butland's flat
        // slopes there left 4.2 × 10⁻⁴ in the density's integral, parabolic ones 1.9 × 10⁻⁴.
        let log_f = Hermite::five_point(
            energies.iter().rev().map(|&e| math::ln(e)).collect(),
            values.iter().rev().map(|&f| math::ln(f)).collect(),
        );
        let f_at = |e: f64| math::exp(log_f.eval(math::ln(e)));

        // Energy panels q = 0..nodes − 2, between E_{q+1} and E_q, 16 nodes in ln E each.
        let mut panel_energy = Vec::with_capacity(nodes - 1);
        let mut panel_weight = Vec::with_capacity(nodes - 1);
        let mut panel_f = Vec::with_capacity(nodes - 1);
        for q in 0..nodes - 1 {
            let (a, b) = (math::ln(energies[q + 1]), math::ln(energies[q]));
            let h = 0.5 * (b - a);
            let es: [f64; 16] = std::array::from_fn(|n| math::exp(a + h + h * GL16_NODES[n]));
            panel_weight.push(std::array::from_fn(|n| GL16_WEIGHTS[n] * h * es[n]));
            panel_f.push(es.map(f_at));
            panel_energy.push(es);
        }

        let mut df = Self {
            energies,
            values,
            log_f,
            panel_energy,
            panel_weight,
            panel_f,
            log_density: Hermite::new(vec![0.0, 1.0], vec![0.0, 0.0]),
            energy_integral: 0.0,
            scale: 1.0,
            inner: (1.0, 1.0),
            outer: radii[last - 1],
            cut_at_core: first == 0,
            cumulative: Hermite::new(vec![0.0, 1.0], vec![0.0, 0.0]),
            panel_max: Vec::new(),
        };

        // The density table, from 64 radii inside the core to the outer cut's radius but one (at
        // the cut, the integral has no energies left).
        #[expect(clippy::cast_precision_loss, reason = "indices below 400 are exact")]
        let table_r: Vec<f64> = (0..INNER_RADII + last)
            .map(|i| math::exp(ln_r0 + step * (i as f64 - INNER_RADII as f64)))
            .collect();
        let table_n: Vec<f64> = table_r
            .iter()
            .enumerate()
            .map(|(i, &r)| {
                let psi_r = if i >= INNER_RADII {
                    psi[i - INNER_RADII]
                } else {
                    profile.psi(r)
                };
                df.speed_integral(psi_r, |_| 1.0)
            })
            .collect();
        df.inner = (table_r[0], table_n[0]);
        df.log_density = Hermite::new(
            table_r.iter().map(|&r| math::ln(r)).collect(),
            table_n.iter().map(|&n| math::ln(n)).collect(),
        );
        // From the cut radius out the realised density is the profile, whose logarithmic slope
        // is exact: Fritsch and Butland's estimates there left 5 × 10⁻⁴ at the break of α = 10.
        // Near the outer cut it falls below the profile, and keeps their slopes.
        for (i, &r) in table_r.iter().enumerate().skip(INNER_RADII + first) {
            if r > EXACT_SLOPES_TO {
                break;
            }
            df.log_density.d[i] = tracer.shape().log_slope(r);
        }
        // The realised share inside each radius: the r^−½ core below the table, then panels.
        let four_pi = 4.0 * core::f64::consts::PI;
        let mut within = Vec::with_capacity(table_r.len());
        within.push(four_pi * table_r[0] * table_r[0] * table_r[0] * table_n[0] / 2.5);
        for i in 1..table_r.len() {
            let (a, b) = (math::ln(table_r[i - 1]), math::ln(table_r[i]));
            let h = 0.5 * (b - a);
            let mut piece = 0.0;
            for n in 0..16 {
                let r = math::exp(a + h + h * GL16_NODES[n]);
                piece += GL16_WEIGHTS[n] * h * four_pi * r * r * r * df.density(r);
            }
            within.push(within[i - 1] + piece);
        }
        // A total of one: the realised density is nothing beyond the table.
        let scale = 1.0 / within[within.len() - 1];
        df.rescale(scale);
        for w in &mut within {
            *w *= scale;
        }
        df.cumulative = Hermite::new(table_r.iter().map(|&r| math::ln(r)).collect(), within);
        df.energy_integral = df
            .panel_weight
            .iter()
            .zip(&df.panel_f)
            .map(|(w, f)| w.iter().zip(f).map(|(w, f)| w * f).sum::<f64>())
            .sum();

        // The sampler's envelope: the interpolant's greatest `ln f` over each energy panel, `ln
        // f`'s interval `nodes − 2 − q` being energy panel q, with a margin for rounding.
        df.panel_max = (0..nodes - 1)
            .map(|q| math::exp(df.log_f.interval_max(nodes - 2 - q, 0.0)) * (1.0 + 1e-9))
            .collect();
        Ok(df)
    }

    /// Multiplies f and every table built on it by `scale`: the Hermite interpolants of `ln f` and
    /// `ln n_f` shift by `ln scale`, which leaves their slopes as they were.
    fn rescale(&mut self, scale: f64) {
        let shift = math::ln(scale);
        self.scale *= scale;
        for f in &mut self.values {
            *f *= scale;
        }
        for y in &mut self.log_f.y {
            *y += shift;
        }
        for panel in &mut self.panel_f {
            for f in panel {
                *f *= scale;
            }
        }
        for y in &mut self.log_density.y {
            *y += shift;
        }
        self.inner.1 *= scale;
    }

    /// An envelope `coefficient × r^−slope` of the realised density at every radius up to
    /// `radius` ly, as `(coefficient, slope)`: what the cells beside the black hole propose under
    /// (P09.T27). Of two bounds it takes the one with the smaller integral over the ball of
    /// `radius`:
    ///
    /// - the shape's cusp continued inward, `r^−γ₀` over the shape's integral, times f's scale,
    ///   for a tracer cut at the core radius, whose f is positive at every energy, so that the cut
    ///   only lowers the profile;
    /// - `4π √2 √(R Ψ(R)) ∫ f dE × r^−½` for any tracer, because `n_f(r) ≤ 4π √(2Ψ(r)) ∫ f dE`
    ///   and `r Ψ(r)` does not fall outward (its derivative is the potential's outer term).
    #[must_use]
    pub fn inner_envelope(
        &self,
        tracer: &TracerProfile,
        profile: &CentreProfile,
        radius: f64,
    ) -> (f64, f64) {
        let ball = |(c, g): (f64, f64)| c * math::powf(radius, 3.0 - g) / (3.0 - g);
        let half = (
            4.0 * core::f64::consts::PI
                * 2.0_f64.sqrt()
                * (radius * profile.psi(radius)).sqrt()
                * self.energy_integral,
            0.5,
        );
        if !self.cut_at_core {
            return half;
        }
        let cusp = (
            self.scale / tracer.shape_total(),
            tracer.shape().inner_slope(),
        );
        if ball(cusp) <= ball(half) { cusp } else { half }
    }

    /// The energies of the grid, (km/s)², falling: `E_j = Ψ(r_j)`.
    #[must_use]
    pub fn energies(&self) -> &[f64] {
        &self.energies
    }

    /// f at the grid's energies.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// The cut `E₀ = Ψ(10⁻³ ly)`, the most bound energy with members, (km/s)².
    #[must_use]
    pub fn cut_energy(&self) -> f64 {
        self.energies[0]
    }

    /// f at energy `e`, (km/s)²: zero above the cut and below the grid's least energy.
    #[must_use]
    pub fn value(&self, e: f64) -> f64 {
        let last = self.energies.len() - 1;
        if e.is_nan() || e <= 0.0 || e > self.energies[0] || e < self.energies[last] {
            return 0.0;
        }
        math::exp(self.log_f.eval(math::ln(e)))
    }

    /// `4π ∫ v² f(Ψ − v² ÷ 2) g(v) dv` at relative potential `psi_r`: as an integral over energy,
    /// `4π ∫₀^{min(Ψ, E₀)} f(E) v g(v) dE` with `v = √(2(Ψ − E))`.
    #[expect(
        clippy::many_single_char_names,
        reason = "energy, speed and the panel's symbols as in the module documentation"
    )]
    fn speed_integral(&self, psi_r: f64, g: impl Fn(f64) -> f64) -> f64 {
        let last = self.energies.len() - 1;
        let top = psi_r.min(self.energies[0]);
        if top.is_nan() || top <= self.energies[last] {
            return 0.0;
        }
        // The panel p with E_{p+1} < top ≤ E_p.
        let (mut lo, mut hi) = (0, last);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.energies[mid] >= top {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let p = lo;
        let mut sum = 0.0;
        // The partial top panel, E = top − D t², t on [0, 1].
        let d = top - self.energies[p + 1];
        for n in 0..16 {
            let t = 0.5 + 0.5 * GL16_NODES[n];
            let e = top - d * t * t;
            let v = (2.0 * (psi_r - e)).max(0.0).sqrt();
            sum += GL16_WEIGHTS[n] * 0.5 * self.value(e) * v * g(v) * 2.0 * d * t;
        }
        for q in p + 1..last {
            for n in 0..16 {
                let v = (2.0 * (psi_r - self.panel_energy[q][n])).sqrt();
                sum += self.panel_weight[q][n] * self.panel_f[q][n] * v * g(v);
            }
        }
        4.0 * core::f64::consts::PI * sum
    }

    /// The realised density at `r` ly, the integral of f (per cubic light-year for a total of
    /// one), from its table: `r^−½` inside the table's innermost radius, and zero beyond its
    /// outermost.
    #[must_use]
    pub fn density(&self, r: f64) -> f64 {
        let (r0, n0) = self.inner;
        if r < r0 {
            return n0 * (r0 / r).sqrt();
        }
        if r > self.outer {
            return 0.0;
        }
        math::exp(self.log_density.eval(math::ln(r)))
    }

    /// The lowest energy with members, `E_out`, (km/s)²: the outer cut's (module documentation).
    #[must_use]
    pub fn floor_energy(&self) -> f64 {
        self.energies[self.energies.len() - 1]
    }

    /// The greatest speed at `r` ly, `√(2(Ψ − E_out))`: the members' orbits stay inside the outer
    /// cut, so none is faster.
    #[must_use]
    pub fn max_speed(&self, profile: &CentreProfile, r: f64) -> KilometresPerSecond {
        KilometresPerSecond::new(
            (2.0 * (profile.psi(r) - self.floor_energy()))
                .max(0.0)
                .sqrt(),
        )
    }

    /// The realised density at `r` ly, integrated afresh rather than read from its table.
    #[must_use]
    pub fn density_direct(&self, profile: &CentreProfile, r: f64) -> f64 {
        self.speed_integral(profile.psi(r), |_| 1.0)
    }

    /// The share of the realised density inside `r` ly.
    #[must_use]
    pub fn fraction_within(&self, r: f64) -> f64 {
        let (r0, n0) = self.inner;
        if r <= 0.0 {
            return 0.0;
        }
        if r < r0 {
            return 4.0 * core::f64::consts::PI * r * r * r * n0 * (r0 / r).sqrt() / 2.5;
        }
        if r >= self.outer {
            return 1.0;
        }
        self.cumulative.eval(math::ln(r))
    }

    /// The one-dimensional velocity dispersion at `r` ly, km/s: `√(⟨v²⟩ ÷ 3)`.
    #[must_use]
    pub fn dispersion(&self, profile: &CentreProfile, r: f64) -> f64 {
        let psi = profile.psi(r);
        let n = self.speed_integral(psi, |_| 1.0);
        let v2 = self.speed_integral(psi, |v| v * v);
        (v2 / (3.0 * n)).sqrt()
    }

    /// `4π ∫ v² f g(v) dv` at `r` ly, per cubic light-year for a tracer of total one: the
    /// density of the members there weighted by `g` of their speed.
    #[must_use]
    pub fn speed_moment(&self, profile: &CentreProfile, r: f64, g: impl Fn(f64) -> f64) -> f64 {
        self.speed_integral(profile.psi(r), g)
    }

    /// `4π ∫ v² f g(v) dv ÷ n_f` at `r` ly: the mean of `g` over the speeds there.
    #[must_use]
    pub fn mean_over_speeds(&self, profile: &CentreProfile, r: f64, g: impl Fn(f64) -> f64) -> f64 {
        let psi = profile.psi(r);
        let n = self.speed_integral(psi, |_| 1.0);
        if n > 0.0 {
            self.speed_integral(psi, g) / n
        } else {
            0.0
        }
    }

    /// The share of speeds at `r` ly below `v`: what the Kolmogorov–Smirnov tests compare with.
    #[must_use]
    pub fn speed_cdf(&self, profile: &CentreProfile, r: f64, v: f64) -> f64 {
        self.mean_over_speeds(profile, r, |u| if u < v { 1.0 } else { 0.0 })
    }

    /// A velocity at `position` (ly from the black hole, galactic axes) from the next words of
    /// `stream`, km/s (module documentation), or `None` if the position lies beyond the outer cut
    /// ([`OUTER_CUT`]) or if [`MAX_ATTEMPTS`] attempts all fail.
    ///
    /// # Panics
    ///
    /// In debug builds, if the attempts all fail, which a bound that holds makes vanishingly
    /// unlikely at any radius inside the reach.
    ///
    /// The speed never exceeds [`max_speed`](Self::max_speed), under the escape speed `√(2Ψ)`.
    #[expect(
        clippy::many_single_char_names,
        reason = "the sampler's symbols as in the module documentation"
    )]
    pub fn draw_velocity(
        &self,
        profile: &CentreProfile,
        position: [f64; 3],
        stream: &mut Stream,
    ) -> Option<[f64; 3]> {
        let direction = isotropic(stream).components();
        let r = (position[0] * position[0] + position[1] * position[1] + position[2] * position[2])
            .sqrt();
        let psi = profile.psi(r);
        let last = self.energies.len() - 1;
        let top = psi.min(self.energies[0]);
        if top.is_nan() || top <= self.energies[last] {
            return None;
        }
        // The panel p with E_{p+1} < top ≤ E_p.
        let (mut lo, mut hi) = (0, last);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.energies[mid] >= top {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        // The envelope's pieces, from the partial panel under `top` down: each panel's greatest f
        // times the greatest `√(2(Ψ − E))` on it, at its lower energy, over its width.
        let mut cumulative = [0.0; RADII];
        let mut total = 0.0;
        for (k, q) in (lo..last).enumerate() {
            let (below, above) = (self.energies[q + 1], self.energies[q].min(top));
            let height = self.panel_max[q] * (2.0 * (psi - below)).sqrt();
            total += height * (above - below);
            cumulative[k] = total;
        }
        let pieces = last - lo;
        for _ in 0..MAX_ATTEMPTS {
            let u = stream.uniform_open();
            let mark = stream.mark();
            let at = u * total;
            let k = cumulative[..pieces]
                .partition_point(|&c| c < at)
                .min(pieces - 1);
            let q = lo + k;
            let (below, above) = (self.energies[q + 1], self.energies[q].min(top));
            let start = if k == 0 { 0.0 } else { cumulative[k - 1] };
            let piece = cumulative[k] - start;
            let t = if piece > 0.0 {
                ((at - start) / piece).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let e = below + t * (above - below);
            let height = self.panel_max[q] * (2.0 * (psi - below)).sqrt();
            let v = (2.0 * (psi - e)).max(0.0).sqrt();
            if mark.is_below(Threshold::from_ratio(v * self.value(e), height)) {
                return Some(direction.map(|c| c * v));
            }
        }
        debug_assert!(false, "{MAX_ATTEMPTS} velocity attempts failed at {r} ly");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::features::centre::profile::{SlopeBreak, TracerShape};
    use crate::rng::{ObjectKey, tags};
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    fn fixture() -> (CentreProfile, DistributionFunction) {
        let profile = super::super::testing::milky_way_profile().clone();
        let df = DistributionFunction::invert(profile.stars(), &profile).unwrap();
        (profile, df)
    }

    /// P09.T24.b: the integral of f returns the profile to 10⁻⁴ from the core radius to the reach,
    /// integrated afresh and from its table.
    #[test]
    fn the_integral_of_f_returns_the_profile() {
        let (profile, df) = fixture();
        let (mut direct_worst, mut table_worst) = ((0.0_f64, 0.0), (0.0_f64, 0.0));
        for i in 0..160 {
            let r = CORE_RADIUS.value() * math::exp(f64::from(i) * 0.074);
            if r > 128.0 {
                break;
            }
            let want = profile.stars().density(r);
            let direct = (df.density_direct(&profile, r) / want - 1.0).abs();
            let table = (df.density(r) / want - 1.0).abs();
            if direct > direct_worst.0 {
                direct_worst = (direct, r);
            }
            if table > table_worst.0 {
                table_worst = (table, r);
            }
        }
        eprintln!(
            "worst relative error {:.3e} at {:.4} ly integrated, {:.3e} at {:.4} ly tabulated",
            direct_worst.0, direct_worst.1, table_worst.0, table_worst.1
        );
        assert!(direct_worst.0 < 1e-4, "{direct_worst:?}");
        assert!(table_worst.0 < 1e-4, "{table_worst:?}");
    }

    /// Inside the core the density falls as r^−½, and fewer than three of the Milky Way's systems
    /// lie there.
    #[test]
    fn the_core_falls_as_the_inverse_square_root() {
        let (_, df) = fixture();
        let (a, b) = (1e-5, 1e-4);
        let slope = math::ln(df.density(b) / df.density(a)) / math::ln(b / a);
        assert!((slope + 0.5).abs() < 0.03, "{slope}");
        let systems = 2.5e7 / 0.56 * df.fraction_within(CORE_RADIUS.value());
        assert!(systems < 3.0, "{systems}");
        let (profile, _) = fixture();
        let (got, want) = (
            df.fraction_within(128.0),
            profile.stars().fraction_within(128.0),
        );
        assert!((got / want - 1.0).abs() < 1e-4, "{got} {want}");
    }

    /// P09.T24.b: a profile with a 0.03 ly core has no isotropic distribution function.
    #[test]
    fn a_cored_profile_is_rejected() {
        let profile = super::super::testing::milky_way_profile().clone();
        let cored = TracerProfile::new(
            TracerShape::new(
                0.0,
                [
                    SlopeBreak {
                        radius: 0.03,
                        sharpness: 2.0,
                        rise: 1.3,
                    },
                    SlopeBreak {
                        radius: 10.0,
                        sharpness: 4.0,
                        rise: 2.2,
                    },
                ],
            )
            .unwrap(),
        );
        match DistributionFunction::invert(&cored, &profile) {
            Err(BuildCentreError::NegativeDistribution(r)) => {
                eprintln!("negative at {r} ly");
                assert!(r < 0.1, "{r}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// P09.T24.b: 500 km/s (±10%) at 0.1 ly, rising as r^−½ inside 3 ly (Kepler about the black
    /// hole: `σ² = G M ÷ ((1 + γ) r)` for a cusp of slope γ).
    #[test]
    fn the_dispersion_is_keplerian_near_the_black_hole() {
        let (profile, df) = fixture();
        let at = |r: f64| df.dispersion(&profile, r);
        let s01 = at(0.1);
        let slope_in = math::ln(at(0.3) / at(0.01)) / math::ln(30.0);
        let slope_out = math::ln(at(3.0) / at(0.3)) / math::ln(10.0);
        let kepler = (crate::galaxy::consts::G * profile.black_hole().value() / (2.3 * 0.1)).sqrt();
        eprintln!(
            "σ(0.1 ly) {s01:.1} km/s (Kepler cusp {kepler:.1}); slopes {slope_in:.4} over \
             0.01–0.3 ly, {slope_out:.4} over 0.3–3 ly; σ(1) {:.1}, σ(3) {:.1}, σ(10) {:.1}",
            at(1.0),
            at(3.0),
            at(10.0)
        );
        assert!((450.0..=550.0).contains(&s01), "{s01}");
        assert!((slope_in + 0.5).abs() < 0.03, "{slope_in}");
        assert!((-0.5..-0.35).contains(&slope_out), "{slope_out}");
    }

    /// P09.T24.c: speeds at five radii against the numerical density in v (Kolmogorov–Smirnov),
    /// none above escape.
    #[test]
    fn sampled_speeds_follow_the_distribution_function() {
        let (profile, df) = fixture();
        for (k, r) in [3e-4, 0.01, 0.3, 3.0, 60.0].into_iter().enumerate() {
            let v_esc = profile.escape_speed(r).value();
            let mut speeds = Vec::with_capacity(4_000);
            for n in 0..4_000_u64 {
                let key = ObjectKey::cell((u64::try_from(k).unwrap() << 32) | n);
                let mut stream = Stream::open(Seed::new(0x0924), tags::SELFTEST_STREAM, key);
                let v = df
                    .draw_velocity(&profile, [0.0, r, 0.0], &mut stream)
                    .unwrap();
                let s = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                assert!(s <= v_esc * (1.0 + 1e-12), "{s} above {v_esc} at {r}");
                speeds.push(s);
            }
            // The CDF on a table of 256 speeds, linear between.
            let grid: Vec<(f64, f64)> = (0..=256)
                .map(|i| {
                    let v = v_esc * f64::from(i) / 256.0;
                    (v, df.speed_cdf(&profile, r, v))
                })
                .collect();
            let cdf = |v: f64| {
                let i = grid
                    .partition_point(|&(x, _)| x <= v)
                    .clamp(1, grid.len() - 1);
                let ((x0, c0), (x1, c1)) = (grid[i - 1], grid[i]);
                (c0 + (c1 - c0) * (v - x0) / (x1 - x0)).clamp(0.0, 1.0)
            };
            let ks = ks_one_sample(&mut speeds, cdf);
            eprintln!("r {r}: D {:.4}, p {:.3}", ks.statistic, ks.p_value);
            assert_p_value("centre speeds", ks.p_value, ALPHA);
        }
    }

    #[test]
    fn the_same_stream_gives_the_same_velocity() {
        let (profile, df) = fixture();
        let draw = || {
            let mut s = Stream::open(Seed::new(7), tags::SELFTEST_STREAM, ObjectKey::cell(3));
            df.draw_velocity(&profile, [1.0, 0.5, -0.2], &mut s)
        };
        assert_eq!(draw(), draw());
    }
}
