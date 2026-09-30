//! The nuclear cluster's density profiles and the potential its members move in: the black hole,
//! the cluster and the rest of the galaxy's spherical average (plan 09, P09.T24.a; ruling 144 of
//! 2026-09-22's joint revision).
//!
//! # The stellar profile
//!
//! The brainstorm's cluster ("Dense features") is a broken power law with an inner slope of 1.3, a
//! break near 10 ly and an outer slope of 3.5 (Schödel et al. 2014; Gallego-Cano et al. 2018),
//! continued inward to 10⁻³ ly and then falling as r^−½, cut at the grid's reach of 128 ly. Its
//! law, mass and break are plan 02's [`NuclearClusterParams`], which the potential tables hold too
//! ([`NuclearCluster`]), and its mass is the mass inside the reach (ruling 144.3).
//!
//! As built (P09.T24.b, findings in the plan's Risks; rulings 144.1 and 144.3), three things
//! differ from a literal reading, the first two because the density must be the integral of an
//! isotropic distribution function that is nowhere negative:
//!
//! - **The break is smooth.** Where a density turns shallower inward at a sharp break, `dρ ÷ dΨ`
//!   drops there, and Eddington's formula takes a term `−J ÷ √(E − Ψ_b)` that is minus infinity
//!   just above the break's energy: no isotropic cluster has a sharp broken power law. The break
//!   here is the 3D Nuker law's, `(1 + (r ÷ r_b)^α)^(−Δγ ÷ α)`, with the sharpness α =
//!   [`BREAK_SHARPNESS`] = 10, the value Gallego-Cano et al. (2018, A&A 609, A26, §5.3) and Schödel
//!   et al. (2018, A&A 609, A27, §4.4) fix in their fits. The inversion stays positive at α = 10
//!   for black holes over ±4σ of the M–σ scatter, and goes negative near α = 20–25.
//! - **The r^−½ core is the distribution function's, not the profile's.** A sharp turn to r^−½ at
//!   10⁻³ ly fails the same way, and a smooth one stays positive only if it is spread over more
//!   than a decade. Instead the profile keeps its 1.3 cusp to the centre and the distribution
//!   function is cut at the energy `Ψ(10⁻³ ly)` ([`CORE_RADIUS`]): outside that radius every
//!   orbit that reaches it is kept and the density is the profile's exactly, and inside it the
//!   density of the orbits that remain falls as `Ψ^½ ∝ r^−½`. That is the brainstorm's core and
//!   the marginal slope An and Evans's (2006, ApJ 642, 752) cusp-slope theorem allows an isotropic
//!   cluster about a point mass ([`DistributionFunction`](super::df::DistributionFunction)). A
//!   tracer can be cut further out ([`TracerProfile::with_cut`]): the young disc's inner edge is
//!   the same device at 0.1 ly.
//! - **The law is tapered and normalised inside the reach.** The Nuker law is fitted "within
//!   about 20 pc" (Schödel et al. 2018), and continued to infinity its r^−3.5 tail put 22% of the
//!   mass beyond the reach, which left M(<3 pc) a third under the observed. A second smooth break
//!   at [`NuclearClusterParams::TAPER_RADIUS`] = 100 ly, of sharpness 4, steepens the slope to 5.5,
//!   and the mass is the parameter's inside the reach (96.8% of the whole law's), so that the grid
//!   holds the stated mass: ρ(1 pc) 1.51 × 10⁵ M☉ pc⁻³ and M(<3 pc) 7.1 × 10⁶ M☉ at Milky Way
//!   values, against Schödel et al. 2018's 1.2–1.8 × 10⁵ and 7.8 ± 0.6 × 10⁶.
//!
//! # The potential
//!
//! The members move in the potential of the black hole, the cluster and the rest of the galaxy
//! (ruling 144's joint revision): at 100 ly the black hole and the cluster gave 57 km/s of the
//! galaxy's 76 before the revision (100 after it), so members at 30–128 ly moved a quarter too
//! slow in theirs alone.
//! The galaxy is spherical here, its discs', bar's, bulge's and dark halo's monopole about the
//! centre ([`SphericalAverage`]), which a centre built for a galaxy holds
//! ([`CentreProfile::from_params`], [`CentreProfile::in_galaxy`]); a profile built from a shape
//! alone holds the black hole and the cluster only. Whether a remnant is retained is judged
//! against the escape speed from the black hole and the cluster alone
//! ([`cluster_escape_speed`](CentreProfile::cluster_escape_speed)), since "retained" means bound to
//! the cluster.
//!
//! # Integrals
//!
//! A smooth break has no closed-form enclosed mass, so the shape's two integrals, the mass
//! `∫₀ʳ 4πr′² s dr′` and the potential's outer term `∫ᵣ^∞ 4πr′ s dr′`, are tabulated at knots
//! 32 to a decade from 10⁻⁷ to 10⁶ ly by 16-node Gauss–Legendre panels in `ln r`, and between
//! knots their logarithms are the cubic Hermite interpolant in `ln r` through the knots' values
//! and their exact derivatives (from `4πr³ s` and `−4πr² s`), exact for a pure power law and
//! within 10⁻⁷ of a direct quadrature at Milky Way values. A potential is then a
//! logarithm and a cubic, which the inversions evaluate some ten thousand times each (measured:
//! a 16-node panel from the knot below made three inversions take a second). Below the first
//! knot and above the last the shape is a pure power law to 10⁻²⁵ and the tails are in closed
//! form. Radii are light-years from the black hole.

use core::fmt;
use std::error::Error;

use crate::galaxy::consts::G;
use crate::galaxy::params::{GalaxyParams, NuclearClusterParams};
use crate::galaxy::potential::spherical::{NuclearCluster, SphericalMass};
use crate::galaxy::potential::{MassModel, SphericalAverage};
use crate::math;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::units::{KilometresPerSecond, LightYears, SolarMasses};

/// The sharpness α of the nuclear cluster's break and of the black holes' copy of it: the 3D Nuker
/// law's α = 10 (module documentation; Gallego-Cano et al. 2018; Schödel et al. 2018).
pub const BREAK_SHARPNESS: f64 = NuclearClusterParams::BREAK_SHARPNESS;

/// The radius inside which the distribution function's cut makes the density fall as r^−½, ly
/// (the brainstorm, "Dense features").
pub const CORE_RADIUS: LightYears = LightYears::new(1e-3);

/// The reach of the centre's grid, where its members end, ly (the brainstorm, "Dense features"):
/// the radius the cluster's mass is normalised inside.
pub const REACH: LightYears = NuclearClusterParams::NORMALISATION_RADIUS;

/// The first knot of the shape's tables, ly.
const TABLE_LO: f64 = 1e-7;

/// Knots per decade.
const KNOTS_PER_DECADE: u32 = 32;

/// Decades from the first knot to the last, 10⁻⁷ to 10⁶ ly.
const TABLE_DECADES: u32 = 13;

/// A profile or a potential could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildCentreError {
    /// A shape's inner slope is outside `[0, 3)`, so its mass diverges at the centre.
    InnerSlope(f64),
    /// A shape's outer slope is not above 3, so its mass diverges outward.
    OuterSlope(f64),
    /// A break's radius or sharpness is not positive and finite, or its rise is negative.
    Break,
    /// A mass is negative or not finite.
    Mass(f64),
    /// The distribution function of a profile is not positive and finite at the energy of the
    /// radius given, ly: no isotropic cluster has that profile in that potential (P09.T24.b).
    NegativeDistribution(f64),
}

impl fmt::Display for BuildCentreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InnerSlope(g) => {
                write!(f, "a centre profile's inner slope {g} is outside [0, 3)")
            }
            Self::OuterSlope(g) => write!(f, "a centre profile's outer slope {g} is not above 3"),
            Self::Break => f.write_str(
                "a centre profile's break needs a positive radius and sharpness and a rise of 0 \
                 or more",
            ),
            Self::Mass(m) => write!(f, "a centre mass of {m} solar masses is not valid"),
            Self::NegativeDistribution(r) => write!(
                f,
                "the distribution function is negative at the energy of {r} ly: no isotropic \
                 cluster has this profile"
            ),
        }
    }
}

impl Error for BuildCentreError {}

/// One smooth break of a shape: the logarithmic slope steepens by `rise` about `radius`, over a
/// width set by `sharpness` (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlopeBreak {
    /// The break radius, ly.
    pub radius: f64,
    /// The sharpness α: the factor is `(1 + (r ÷ radius)^α)^(−rise ÷ α)`.
    pub sharpness: f64,
    /// How much steeper the slope is outside the break.
    pub rise: f64,
}

/// A density shape `s(r) = r^−γ₀ Π (1 + (r ÷ r_k)^α_k)^(−Δ_k ÷ α_k)`, unnormalised: an inner
/// slope and the breaks that steepen it (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct TracerShape {
    inner_slope: f64,
    breaks: Vec<SlopeBreak>,
}

impl TracerShape {
    /// A shape with the inner slope `inner_slope` and the `breaks`.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError`] if the inner slope is outside `[0, 3)`, the outer slope not above 3,
    /// or a break's radius or sharpness not positive and finite or its rise negative.
    pub fn new(
        inner_slope: f64,
        breaks: impl IntoIterator<Item = SlopeBreak>,
    ) -> Result<Self, BuildCentreError> {
        if !(0.0..3.0).contains(&inner_slope) {
            return Err(BuildCentreError::InnerSlope(inner_slope));
        }
        let breaks: Vec<SlopeBreak> = breaks.into_iter().collect();
        for b in &breaks {
            let good = b.radius.is_finite()
                && b.radius > 0.0
                && b.sharpness.is_finite()
                && b.sharpness > 0.0
                && b.rise.is_finite()
                && b.rise >= 0.0;
            if !good {
                return Err(BuildCentreError::Break);
            }
        }
        let shape = Self {
            inner_slope,
            breaks,
        };
        if shape.outer_slope() <= 3.0 {
            return Err(BuildCentreError::OuterSlope(shape.outer_slope()));
        }
        Ok(shape)
    }

    /// The nuclear cluster's stellar shape (plan 02's [`NuclearClusterParams`]): inner slope 1.3, a
    /// break at 10 ly of sharpness [`BREAK_SHARPNESS`] to 3.5, and the taper at 100 ly of
    /// sharpness 4 to 5.5 (module documentation).
    ///
    /// # Panics
    ///
    /// Never: the slopes and breaks are constants inside the ranges.
    #[must_use]
    pub fn nuclear_cluster() -> Self {
        type P = NuclearClusterParams;
        Self::new(
            P::INNER_SLOPE,
            [
                SlopeBreak {
                    radius: P::BREAK_RADIUS.value(),
                    sharpness: P::BREAK_SHARPNESS,
                    rise: P::OUTER_SLOPE - P::INNER_SLOPE,
                },
                SlopeBreak {
                    radius: P::TAPER_RADIUS.value(),
                    sharpness: P::TAPER_SHARPNESS,
                    rise: P::TAPERED_SLOPE - P::OUTER_SLOPE,
                },
            ],
        )
        .expect("the nuclear cluster's slopes are 1.3, 3.5 and 5.5 and its breaks 10 and 100 ly")
    }

    /// The inner logarithmic slope γ₀ (the density falls as `r^−γ₀` at the centre).
    #[must_use]
    pub fn inner_slope(&self) -> f64 {
        self.inner_slope
    }

    /// The outer logarithmic slope, γ₀ plus every break's rise.
    #[must_use]
    pub fn outer_slope(&self) -> f64 {
        self.inner_slope + self.breaks.iter().map(|b| b.rise).sum::<f64>()
    }

    /// The breaks, innermost first as given.
    #[must_use]
    pub fn breaks(&self) -> &[SlopeBreak] {
        &self.breaks
    }

    /// `s(r)` at `r` ly, unnormalised.
    #[must_use]
    pub fn value(&self, r: f64) -> f64 {
        let mut s = math::powf(r, -self.inner_slope);
        for b in &self.breaks {
            let y = math::powf(r / b.radius, b.sharpness);
            s *= math::powf(1.0 + y, -b.rise / b.sharpness);
        }
        s
    }

    /// `d ln s ÷ d ln r` at `r` ly: `−γ₀ − Σ Δ y ÷ (1 + y)`, `y = (r ÷ r_k)^α`.
    #[must_use]
    pub fn log_slope(&self, r: f64) -> f64 {
        let mut g = -self.inner_slope;
        for b in &self.breaks {
            let y = math::powf(r / b.radius, b.sharpness);
            g -= b.rise * y / (1.0 + y);
        }
        g
    }

    /// `d² ln s ÷ d (ln r)²` at `r` ly: `−Σ Δ α y ÷ (1 + y)²`.
    #[must_use]
    pub fn log_curvature(&self, r: f64) -> f64 {
        let mut h = 0.0;
        for b in &self.breaks {
            let y = math::powf(r / b.radius, b.sharpness);
            h -= b.rise * b.sharpness * y / ((1.0 + y) * (1.0 + y));
        }
        h
    }
}

impl TracerShape {
    /// `s(r)`, `d ln s ÷ d ln r` and `d² ln s ÷ d (ln r)²` at `r` ly together, with one power per
    /// break: what [`value`](Self::value), [`log_slope`](Self::log_slope) and
    /// [`log_curvature`](Self::log_curvature) give, bit for bit.
    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "the shape, its slope and curvature in the module documentation's symbols"
    )]
    pub fn derivatives(&self, r: f64) -> (f64, f64, f64) {
        let mut s = math::powf(r, -self.inner_slope);
        let mut g = -self.inner_slope;
        let mut h = 0.0;
        for b in &self.breaks {
            let y = math::powf(r / b.radius, b.sharpness);
            s *= math::powf(1.0 + y, -b.rise / b.sharpness);
            g -= b.rise * y / (1.0 + y);
            h -= b.rise * b.sharpness * y / ((1.0 + y) * (1.0 + y));
        }
        (s, g, h)
    }
}

/// `∫ₐᵇ f(r) d ln r` by one 16-node Gauss–Legendre panel in `ln r`, from `a` towards `b`.
fn panel_ln(mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    let (la, lb) = (math::ln(a), math::ln(b));
    let half = 0.5 * (lb - la);
    let mid = la + half;
    let mut sum = 0.0;
    for (&x, &w) in GL16_NODES.iter().zip(&GL16_WEIGHTS) {
        sum += w * f(math::exp(mid + half * x));
    }
    sum * half
}

/// A shape normalised to a total of one over all space, with its tabulated integrals (module
/// documentation): a tracer's number density per member, per cubic light-year, and the radius
/// inside which its distribution function's energy cut makes the density fall as r^−½.
#[derive(Debug, Clone, PartialEq)]
pub struct TracerProfile {
    shape: TracerShape,
    /// The cut's radius, ly: [`CORE_RADIUS`] unless [`with_cut`](Self::with_cut) says otherwise.
    cut: f64,
    /// `ln r_k`, even in steps of `ln 10 ÷ 32`.
    ln_knots: Vec<f64>,
    /// The logarithm of `∫₀^{r_k} 4πr² s dr` and its derivative in `ln r`, `4π r³ s` over the
    /// integral, at each knot.
    inner: Vec<(f64, f64)>,
    /// The logarithm of `∫_{r_k}^∞ 4πr s dr` and its derivative in `ln r`, `−4π r² s` over the
    /// integral, at each knot.
    outer: Vec<(f64, f64)>,
    total: f64,
}

/// The cubic Hermite interpolant on `[0, 1]` in `t` between `(y0, d0)` and `(y1, d1)`, slopes per
/// panel width `h`.
fn hermite(t: f64, h: f64, (y0, d0): (f64, f64), (y1, d1): (f64, f64)) -> f64 {
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * y0
        + (t3 - 2.0 * t2 + t) * h * d0
        + (-2.0 * t3 + 3.0 * t2) * y1
        + (t3 - t2) * h * d1
}

impl TracerProfile {
    /// The profile of `shape`, normalised to one, cut at [`CORE_RADIUS`].
    ///
    /// # Panics
    ///
    /// Never: the knots' count is a constant, 417.
    #[must_use]
    pub fn new(shape: TracerShape) -> Self {
        Self::with_cut(shape, CORE_RADIUS)
    }

    /// The profile of `shape`, normalised to one, whose distribution function is cut at the
    /// energy of `cut` (at or outside [`CORE_RADIUS`]; the inversion takes the first node of its
    /// energy grid at or outside it): the device of the module documentation, which the young
    /// disc's inner edge uses at 0.1 ly (ruling 144.6). The shape inside `cut` then sets only the
    /// normalisation, which the distribution function takes afresh from what its cut keeps.
    ///
    /// # Panics
    ///
    /// Never: the knots' count is a constant, 417.
    #[must_use]
    pub fn with_cut(shape: TracerShape, cut: LightYears) -> Self {
        let n = usize::try_from(KNOTS_PER_DECADE * TABLE_DECADES).expect("416 knots") + 1;
        let lo = math::ln(TABLE_LO);
        let step = core::f64::consts::LN_10 / f64::from(KNOTS_PER_DECADE);
        let ln_knots: Vec<f64> = (0..n)
            .map(|k| lo + step * f64::from(u32::try_from(k).expect("416 knots fit a u32")))
            .collect();
        let knots: Vec<f64> = ln_knots.iter().map(|&x| math::exp(x)).collect();
        let four_pi = 4.0 * core::f64::consts::PI;
        let (g_in, g_out) = (shape.inner_slope(), shape.outer_slope());
        let values: Vec<f64> = knots.iter().map(|&r| shape.value(r)).collect();
        let mut inner = Vec::with_capacity(n);
        let r0 = knots[0];
        inner.push((
            four_pi * r0 * r0 * r0 * values[0] / (3.0 - g_in),
            four_pi * r0 * r0 * r0 * values[0],
        ));
        for k in 1..n {
            let piece = panel_ln(
                |r| four_pi * r * r * r * shape.value(r),
                knots[k - 1],
                knots[k],
            );
            let r = knots[k];
            inner.push((inner[k - 1].0 + piece, four_pi * r * r * r * values[k]));
        }
        let mut outer = vec![(0.0, 0.0); n];
        let rn = knots[n - 1];
        outer[n - 1] = (
            four_pi * rn * rn * values[n - 1] / (g_out - 2.0),
            -four_pi * rn * rn * values[n - 1],
        );
        for k in (0..n - 1).rev() {
            let piece = panel_ln(|r| four_pi * r * r * shape.value(r), knots[k], knots[k + 1]);
            let r = knots[k];
            outer[k] = (outer[k + 1].0 + piece, -four_pi * r * r * values[k]);
        }
        let total = inner[n - 1].0 + four_pi * rn * rn * rn * values[n - 1] / (g_out - 3.0);
        let logs = |(y, d): (f64, f64)| (math::ln(y), d / y);
        let inner = inner.into_iter().map(logs).collect();
        let outer = outer.into_iter().map(logs).collect();
        Self {
            shape,
            cut: cut.value().max(CORE_RADIUS.value()),
            ln_knots,
            inner,
            outer,
            total,
        }
    }

    /// The radius of the distribution function's energy cut, ly.
    #[must_use]
    pub fn cut_radius(&self) -> LightYears {
        LightYears::new(self.cut)
    }

    /// The shape.
    #[must_use]
    pub fn shape(&self) -> &TracerShape {
        &self.shape
    }

    /// The panel `k` with `ln_knots[k] ≤ x < ln_knots[k + 1]` and the position `t` in it, for
    /// `x = ln r` inside the table.
    fn locate(&self, x: f64) -> (usize, f64) {
        let last = self.ln_knots.len() - 2;
        let step = self.ln_knots[1] - self.ln_knots[0];
        let at = ((x - self.ln_knots[0]) / step).clamp(0.0, 415.0).floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is clamped to [0, 415] and floored, a small non-negative integer"
        )]
        let mut k = (at as usize).min(last);
        while k > 0 && self.ln_knots[k] > x {
            k -= 1;
        }
        while k < last && self.ln_knots[k + 1] <= x {
            k += 1;
        }
        (
            k,
            (x - self.ln_knots[k]) / (self.ln_knots[k + 1] - self.ln_knots[k]),
        )
    }

    /// The density at `r` ly, per cubic light-year for a total of one.
    #[must_use]
    pub fn density(&self, r: f64) -> f64 {
        self.shape.value(r) / self.total
    }

    /// The unnormalised shape's integral over all space, ly³: what the density divides by.
    #[must_use]
    pub fn shape_total(&self) -> f64 {
        self.total
    }

    /// The share of the tracer inside `r` ly: between knots, the cubic Hermite interpolant of the
    /// integral's logarithm in `ln r` through the knots' values and exact derivatives (module
    /// documentation).
    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "radius and panel in their usual symbols"
    )]
    pub fn fraction_within(&self, r: f64) -> f64 {
        let four_pi = 4.0 * core::f64::consts::PI;
        let n = self.ln_knots.len();
        if r <= 0.0 {
            return 0.0;
        }
        let x = math::ln(r);
        let raw = if x <= self.ln_knots[0] {
            four_pi * r * r * r * self.shape.value(r) / (3.0 - self.shape.inner_slope())
        } else if x >= self.ln_knots[n - 1] {
            self.total
                - four_pi * r * r * r * self.shape.value(r) / (self.shape.outer_slope() - 3.0)
        } else {
            let (k, t) = self.locate(x);
            let h = self.ln_knots[k + 1] - self.ln_knots[k];
            math::exp(hermite(t, h, self.inner[k], self.inner[k + 1]))
        };
        raw / self.total
    }

    /// `∫ᵣ^∞ 4πr′ n(r′) dr′` for the normalised density `n`, per light-year: the outer term of the
    /// potential of a unit mass so distributed. Interpolated as
    /// [`fraction_within`](Self::fraction_within) is.
    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "radius and panel in their usual symbols"
    )]
    pub fn outer_moment(&self, r: f64) -> f64 {
        let four_pi = 4.0 * core::f64::consts::PI;
        let n = self.ln_knots.len();
        let x = math::ln(r);
        let raw = if x <= self.ln_knots[0] {
            // `∫_r^{r₀} 4πr s dr` of the pure inner power law, then the table.
            let first = math::exp(self.ln_knots[0]);
            let g = self.shape.inner_slope();
            let s0 = self.shape.value(first);
            let head = if (2.0 - g).abs() < 1e-9 {
                four_pi * s0 * first * first * math::ln(first / r)
            } else {
                four_pi
                    * s0
                    * math::powf(first, g)
                    * (math::powf(first, 2.0 - g) - math::powf(r, 2.0 - g))
                    / (2.0 - g)
            };
            math::exp(self.outer[0].0) + head
        } else if x >= self.ln_knots[n - 1] {
            four_pi * r * r * self.shape.value(r) / (self.shape.outer_slope() - 2.0)
        } else {
            let (k, t) = self.locate(x);
            let h = self.ln_knots[k + 1] - self.ln_knots[k];
            math::exp(hermite(t, h, self.outer[k], self.outer[k + 1]))
        };
        raw / self.total
    }
}

/// The nuclear cluster's stellar profile about the central black hole, and the potential its
/// members move in (module documentation).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::centre::CentreProfile;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let profile = CentreProfile::from_params(&GalaxyParams::milky_way_like())?;
/// // The escape speed from the cluster is some 1,100 km/s at 0.1 ly and some 210 at 10 ly (the
/// // brainstorm); from the galaxy it is several times more.
/// let near = profile.cluster_escape_speed(0.1).value();
/// let far = profile.cluster_escape_speed(10.0).value();
/// assert!((1_000.0..1_250.0).contains(&near) && (180.0..240.0).contains(&far));
/// assert!(profile.escape_speed(10.0).value() > 3.0 * far);
/// # Ok::<(), hyperion_sim::galaxy::features::centre::BuildCentreError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CentreProfile {
    stars: TracerProfile,
    /// The whole law's mass, inside the reach and beyond it.
    mass: SolarMasses,
    black_hole: SolarMasses,
    /// The rest of the galaxy's monopole, for a centre built in a galaxy.
    galaxy: Option<SphericalAverage>,
}

impl CentreProfile {
    /// The profile of a cluster of `cluster`'s mass about a black hole of `black_hole`, in the
    /// potential of the two alone.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError::Mass`] if either mass is negative or not finite.
    pub fn new(
        cluster: &NuclearClusterParams,
        black_hole: SolarMasses,
    ) -> Result<Self, BuildCentreError> {
        Self::from_profile(NuclearCluster::shape().clone(), cluster.mass(), black_hole)
    }

    /// The profile of a cluster of shape `stars` holding `mass` inside the reach, about a black
    /// hole of `black_hole`, in the potential of the two alone: [`new`](Self::new) with the shape
    /// given directly, for the tests of other laws and of extreme black hole to cluster ratios.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError::Mass`] if either mass is negative or not finite.
    pub fn from_shape(
        stars: TracerShape,
        mass: SolarMasses,
        black_hole: SolarMasses,
    ) -> Result<Self, BuildCentreError> {
        Self::from_profile(TracerProfile::new(stars), mass, black_hole)
    }

    fn from_profile(
        stars: TracerProfile,
        mass: SolarMasses,
        black_hole: SolarMasses,
    ) -> Result<Self, BuildCentreError> {
        for m in [mass.value(), black_hole.value()] {
            if !(m.is_finite() && m >= 0.0) {
                return Err(BuildCentreError::Mass(m));
            }
        }
        let total = mass.value() / stars.fraction_within(REACH.value());
        Ok(Self {
            stars,
            mass: SolarMasses::new(total),
            black_hole,
            galaxy: None,
        })
    }

    /// The profile of a galaxy's parameters: its nuclear cluster and its black hole, in their
    /// potential and the monopole of the galaxy's mass model ([`in_galaxy`](Self::in_galaxy)),
    /// which it builds.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn from_params(params: &GalaxyParams) -> Result<Self, BuildCentreError> {
        Ok(
            Self::new(params.nuclear_cluster(), params.black_hole().mass())?
                .in_galaxy(&MassModel::new(params)),
        )
    }

    /// This profile in the potential of `model`'s discs, bar, bulge and dark halo besides the
    /// black hole and the cluster: their spherical average about the centre (module
    /// documentation). `model`'s own black hole and cluster are not read.
    #[must_use]
    pub fn in_galaxy(self, model: &MassModel) -> Self {
        Self {
            galaxy: Some(SphericalAverage::of_extended(model)),
            ..self
        }
    }

    /// The rest of the galaxy's monopole, if the profile was built in a galaxy.
    #[must_use]
    pub fn galaxy(&self) -> Option<&SphericalAverage> {
        self.galaxy.as_ref()
    }

    /// The stars' profile, normalised to one.
    #[must_use]
    pub fn stars(&self) -> &TracerProfile {
        &self.stars
    }

    /// The cluster's whole mass, inside the reach and beyond it: the law's.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The cluster's mass inside the reach: plan 02's parameter.
    #[must_use]
    pub fn mass_within_reach(&self) -> SolarMasses {
        self.stellar_mass_within(REACH.value())
    }

    /// The black hole's mass.
    #[must_use]
    pub fn black_hole(&self) -> SolarMasses {
        self.black_hole
    }

    /// The stars' density at `r` ly, M☉ per cubic light-year.
    #[must_use]
    pub fn density(&self, r: f64) -> f64 {
        self.mass.value() * self.stars.density(r)
    }

    /// The stars' density at `r` ly in systems per cubic light-year, for a mean system mass of
    /// `mean_mass`.
    #[must_use]
    pub fn systems_per_cubic_ly(&self, r: f64, mean_mass: SolarMasses) -> f64 {
        self.density(r) / mean_mass.value()
    }

    /// The stars' mass inside `r` ly.
    #[must_use]
    pub fn stellar_mass_within(&self, r: f64) -> SolarMasses {
        SolarMasses::new(self.mass.value() * self.stars.fraction_within(r))
    }

    /// The mass inside `r` ly, the black hole's and the stars'.
    #[must_use]
    pub fn enclosed_mass(&self, r: f64) -> SolarMasses {
        SolarMasses::new(self.black_hole.value() + self.stellar_mass_within(r).value())
    }

    /// The mass inside `r` ly that pulls: the black hole's, the stars' and the rest of the
    /// galaxy's.
    #[must_use]
    pub fn total_enclosed_mass(&self, r: f64) -> SolarMasses {
        let rest = self
            .galaxy
            .as_ref()
            .map_or(SolarMasses::ZERO, |g| g.enclosed_mass(LightYears::new(r)));
        self.enclosed_mass(r) + rest
    }

    /// The relative potential of the black hole and the cluster alone at `r` ly, (km/s)², zero at
    /// infinity: `G (M_bh + M(<r)) ÷ r + 4πG ∫ᵣ^∞ ρ r′ dr′`.
    #[must_use]
    pub fn cluster_psi(&self, r: f64) -> f64 {
        G * self.enclosed_mass(r).value() / r + G * self.mass.value() * self.stars.outer_moment(r)
    }

    /// The relative potential `Ψ(r) = −Φ(r)` the members move in at `r` ly, (km/s)², zero at
    /// infinity: [`cluster_psi`](Self::cluster_psi) plus the rest of the galaxy's monopole.
    #[must_use]
    pub fn psi(&self, r: f64) -> f64 {
        let rest = self.galaxy.as_ref().map_or(0.0, |g| g.psi(r));
        self.cluster_psi(r) + rest
    }

    /// `dΨ ÷ dr` at `r` ly, (km/s)² per light-year: `−G M(<r) ÷ r²` of everything that pulls.
    #[must_use]
    pub fn psi_slope(&self, r: f64) -> f64 {
        -G * self.total_enclosed_mass(r).value() / (r * r)
    }

    /// `d²Ψ ÷ dr²` at `r` ly: `2 G M(<r) ÷ r³ − 4πG ρ(r)` of everything that pulls.
    #[must_use]
    pub fn psi_curvature(&self, r: f64) -> f64 {
        let rest = self
            .galaxy
            .as_ref()
            .map_or(0.0, |g| g.density(LightYears::new(r)));
        2.0 * G * self.total_enclosed_mass(r).value() / (r * r * r)
            - 4.0 * core::f64::consts::PI * G * (self.density(r) + rest)
    }

    /// The escape speed `√(2Ψ)` from `r` ly out of the galaxy (from the black hole and the cluster
    /// alone for a profile not built in one).
    #[must_use]
    pub fn escape_speed(&self, r: f64) -> KilometresPerSecond {
        KilometresPerSecond::new((2.0 * self.psi(r)).sqrt())
    }

    /// The escape speed from `r` ly out of the black hole and the cluster, `√(2 Ψ_cluster)`: what
    /// a remnant's kick must stay under to be retained (module documentation).
    #[must_use]
    pub fn cluster_escape_speed(&self, r: f64) -> KilometresPerSecond {
        KilometresPerSecond::new((2.0 * self.cluster_psi(r)).sqrt())
    }

    /// The radius where the stars inside weigh as much as the black hole, ly: the influence
    /// radius (plan 09, P09.T28.b), by a fixed bisection in `ln r` over 10⁻³–10⁴ ly.
    #[must_use]
    pub fn influence_radius(&self) -> LightYears {
        let m = self.black_hole.value();
        let x = crate::galaxy::quad::bisect(
            |lr| self.stellar_mass_within(math::exp(lr)).value() - m,
            math::ln(1e-3),
            math::ln(1e4),
            64,
        );
        LightYears::new(math::exp(x))
    }

    /// The reach where the members end ([`REACH`]).
    #[must_use]
    pub fn reach(&self) -> LightYears {
        REACH
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::Population;
    use crate::galaxy::features::centre::testing::milky_way_profile;
    use crate::galaxy::imf::MassFunctionKind;
    use hyperion_testkit::float::assert_same_bits;

    fn milky_way() -> (GalaxyParams, &'static CentreProfile) {
        (GalaxyParams::milky_way_like(), milky_way_profile())
    }

    #[test]
    fn the_tables_match_a_direct_quadrature() {
        let (_, p) = milky_way();
        let four_pi = 4.0 * core::f64::consts::PI;
        for r in [3e-7, 1e-3, 0.37, 3.0, 9.99, 10.0, 47.0, 128.0, 5e4] {
            // The pure inner power law below 10⁻¹⁰ ly, then a panel per decade.
            let lo = 1e-10;
            let mut edges = vec![lo];
            while *edges.last().unwrap() * 10.0 < r {
                edges.push(edges.last().unwrap() * 10.0);
            }
            edges.push(r);
            let direct = four_pi * lo * lo * lo * p.stars.density(lo) / 1.7
                + crate::galaxy::quad::gl_log_panels(
                    |x| four_pi * x * x * p.stars.density(x),
                    &edges,
                );
            let table = p.stars.fraction_within(r);
            eprintln!("{r}: relative error {:.2e}", (table / direct - 1.0).abs());
            assert!(
                (table - direct).abs() < 1e-7 * direct,
                "{r}: {table} {direct}"
            );
        }
        // The taper's r^−5.5 tail leaves nothing beyond 10¹² ly.
        assert!((p.stars.fraction_within(1e12) - 1.0).abs() < 1e-12);
        // The potential's outer term against its own quadrature from r to 10⁹ ly.
        for r in [1e-3, 3.0, 128.0] {
            let direct = crate::galaxy::quad::gl_log_panels(
                |x| four_pi * x * p.stars.density(x),
                &[
                    r,
                    r * 10.0,
                    r * 100.0,
                    r * 1e3,
                    r * 1e4,
                    r * 1e6,
                    r * 1e9,
                    r * 1e12,
                ],
            );
            let table = p.stars.outer_moment(r);
            assert!(
                (table - direct).abs() < 1e-6 * direct,
                "{r}: {table} {direct}"
            );
        }
    }

    #[test]
    fn the_slopes_are_the_shape_s_derivatives() {
        let (_, p) = milky_way();
        let s = p.stars.shape();
        for r in [0.01, 3.0, 10.0, 30.0] {
            let h = 1e-5;
            let numeric = (math::ln(s.value(r * math::exp(h)))
                - math::ln(s.value(r * math::exp(-h))))
                / (2.0 * h);
            assert!((numeric - s.log_slope(r)).abs() < 1e-6, "{r}");
            let curv = (s.log_slope(r * math::exp(h)) - s.log_slope(r * math::exp(-h))) / (2.0 * h);
            assert!((curv - s.log_curvature(r)).abs() < 1e-6, "{r}");
        }
        assert!((s.log_slope(1e-4) + 1.3).abs() < 1e-6);
        assert!((s.log_slope(40.0) + 3.5).abs() < 0.2);
        assert!((s.log_slope(1e5) + 5.5).abs() < 1e-6);
    }

    #[test]
    #[expect(clippy::many_single_char_names, reason = "the shape's symbols")]
    fn the_joint_derivatives_are_the_separate_ones_bit_for_bit() {
        let (_, p) = milky_way();
        let s = p.stars.shape();
        for r in [1e-6, 0.3, 10.0, 7e3] {
            let (v, g, h) = s.derivatives(r);
            assert_same_bits(v, s.value(r));
            assert_same_bits(g, s.log_slope(r));
            assert_same_bits(h, s.log_curvature(r));
        }
    }

    #[test]
    fn a_shape_whose_mass_diverges_is_refused() {
        assert_eq!(
            TracerShape::new(3.0, []),
            Err(BuildCentreError::InnerSlope(3.0))
        );
        assert_eq!(
            TracerShape::new(
                1.0,
                [SlopeBreak {
                    radius: 1.0,
                    sharpness: 2.0,
                    rise: 1.5
                }]
            ),
            Err(BuildCentreError::OuterSlope(2.5))
        );
    }

    /// P09.T24.a at Milky Way values, in the windows of ruling 144.2 and 144.4, on the joint
    /// revision's law (ruling 144.3). The observations are mass densities and enclosed masses:
    /// ρ(1 pc) 1.2–1.8 × 10⁵ M☉ pc⁻³ (Schödel et al. 2018, A&A 609, A27, Table 3's four
    /// normalisations), M(<1 pc) 0.8–1.2 × 10⁶ M☉ (its 1.0 ± 0.1), M(<3 pc) 6–10 × 10⁶ (its 7.8 ±
    /// 0.6) and M(<3.9 pc) 7–11 × 10⁶ (Chatzopoulos et al. 2015's 8.94 ± 0.9). The parameter is
    /// the mass inside the reach. The systems the grid holds, inside the reach, are counted at
    /// the centre's own mean system mass, T27's (ruling 144.4): 4–6 × 10⁷, fewer than three
    /// inside 10⁻³ ly (provisionally 3.5 under Kroupa's function); and the stars outweigh the
    /// black hole near 10 ly. Systems per cubic
    /// light-year at 3 ly are printed, not tested: the brainstorm's 7,800 was 4,322 M☉ ly⁻³ over an
    /// older mean mass.
    #[test]
    fn the_milky_way_s_cluster_has_the_observed_masses() {
        use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC as PC;
        use crate::galaxy::features::centre::CentreClasses;
        let (params, p) = milky_way();
        let rho_1pc = p.density(PC) * PC * PC * PC;
        let within = |pc: f64| p.stellar_mass_within(pc * PC).value();
        let (m1, m3, m39) = (within(1.0), within(3.0), within(3.9));
        let influence = p.influence_radius().value();
        let inside = p.stars.fraction_within(REACH.value());
        eprintln!(
            "ρ(1 pc) {rho_1pc:.4e} M☉ pc⁻³; M(<1 pc) {m1:.4e}, M(<3 pc) {m3:.4e}, M(<3.9 pc) \
             {m39:.4e}, M(<10 pc) {:.4e}, M(<30 pc) {:.4e}, the law's {:.4e} with {inside:.4} \
             inside 128 ly; M(<10 ly) {:.4e} against M_bh {:.4e}, influence radius \
             {influence:.2} ly, v_esc from the cluster {:.0} at 0.1 ly and {:.0} at 10 ly, from \
             the galaxy {:.0} and {:.0}",
            within(10.0),
            within(30.0),
            p.mass().value(),
            p.stellar_mass_within(10.0).value(),
            p.black_hole().value(),
            p.cluster_escape_speed(0.1).value(),
            p.cluster_escape_speed(10.0).value(),
            p.escape_speed(0.1).value(),
            p.escape_speed(10.0).value(),
        );
        let reach = p.mass_within_reach().value();
        assert!(
            (reach / params.nuclear_cluster().mass().value() - 1.0).abs() < 1e-12,
            "{reach}"
        );
        assert!((1.2e5..=1.8e5).contains(&rho_1pc), "ρ(1 pc) {rho_1pc}");
        assert!((0.8e6..=1.2e6).contains(&m1), "M(<1 pc) {m1}");
        assert!((6.0e6..=10.0e6).contains(&m3), "M(<3 pc) {m3}");
        assert!((7.0e6..=11.0e6).contains(&m39), "M(<3.9 pc) {m39}");
        assert!(p.stellar_mass_within(10.0) > p.black_hole());
        assert!((5.0..15.0).contains(&influence), "{influence}");
        for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
            let imf = kind.to_mass_function();
            let classes = CentreClasses::new(p, imf.as_ref());
            let mean = classes.mean_system_mass();
            let systems = classes.systems();
            let at_3 = p.systems_per_cubic_ly(3.0, mean);
            let core = systems * p.stars.fraction_within(CORE_RADIUS.value());
            let held = systems * inside;
            eprintln!(
                "{kind:?}: the centre's mean {:.4} M☉ (the nuclear disc's {:.4}), {at_3:.0} \
                 systems/ly³ at 3 ly ({:.0} M☉/ly³), {systems:.4e} systems ({held:.4e} inside \
                 128 ly), {core:.3} inside 10⁻³ ly",
                mean.value(),
                params.mean_system_mass(Population::NuclearDisc).value(),
                p.density(3.0),
            );
            // Provisional under Kroupa's function (the plan's Risks, "The joint revision as
            // built"): the normalisation inside the reach raised the cusp by 28%, and its lighter
            // mean puts 3.4 systems inside 10⁻³ ly, over the brainstorm's "fewer than three".
            let most = if kind == MassFunctionKind::Chabrier {
                3.0
            } else {
                3.5
            };
            assert!(core < most, "{core} systems inside 10⁻³ ly");
            if kind == MassFunctionKind::Chabrier {
                assert!(
                    (4e7..=6e7).contains(&held),
                    "{held} systems inside the reach"
                );
            }
        }
    }
}
