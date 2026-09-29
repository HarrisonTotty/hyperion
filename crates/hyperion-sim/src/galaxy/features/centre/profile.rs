//! The nuclear cluster's density profiles and the potential of the black hole plus the cluster
//! (plan 09, P09.T24.a).
//!
//! # The stellar profile
//!
//! The brainstorm's cluster ("Dense features") is a broken power law with an inner slope of 1.3, a
//! break near 10 ly and an outer slope of 3.5 (Schödel et al. 2014; Gallego-Cano et al. 2018),
//! continued inward to 10⁻³ ly and then falling as r^−½, cut at the grid's reach of 128 ly. Its
//! mass and break are plan 02's [`NuclearClusterParams`], which is also the mass the potential
//! tables hold, so the total is that of the whole, uncut law.
//!
//! As built (P09.T24.b, findings in the plan's Risks), two things differ from a literal reading,
//! both because the density must be the integral of an isotropic distribution function that is
//! nowhere negative:
//!
//! - **The break is smooth.** Where a density turns shallower inward at a sharp break, `dρ ÷ dΨ`
//!   drops there, and Eddington's formula takes a term `−J ÷ √(E − Ψ_b)` that is minus infinity
//!   just above the break's energy: no isotropic cluster has a sharp broken power law. The break
//!   here is `(1 + (r ÷ r_b)^α)^(−Δγ ÷ α)` with the sharpness α = [`BREAK_SHARPNESS`] = 4
//!   (provisional, ours; the inversion stays positive up to about α = 25 at Milky Way values).
//! - **The r^−½ core is the distribution function's, not the profile's.** A sharp turn to r^−½ at
//!   10⁻³ ly fails the same way, and a smooth one stays positive only if it is spread over more
//!   than a decade. Instead the profile keeps its 1.3 cusp to the centre and the distribution
//!   function is cut at the energy `Ψ(10⁻³ ly)` ([`CORE_RADIUS`]): outside that radius every
//!   orbit that reaches it is kept and the density is the profile's exactly, and inside it the
//!   density of the orbits that remain falls as `Ψ^½ ∝ r^−½`, which is the brainstorm's core and
//!   the marginal slope an isotropic cluster about a point mass can have
//!   ([`DistributionFunction`](super::df::DistributionFunction)).
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
use crate::galaxy::params::NuclearClusterParams;
use crate::math;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::units::{KilometresPerSecond, LightYears, SolarMasses};

/// The sharpness α of every break of the centre's profiles (module documentation; provisional,
/// ours).
pub const BREAK_SHARPNESS: f64 = 4.0;

/// The radius inside which the distribution function's cut makes the density fall as r^−½, ly
/// (the brainstorm, "Dense features").
pub const CORE_RADIUS: LightYears = LightYears::new(1e-3);

/// The reach of the centre's grid, where its members end, ly (the brainstorm, "Dense features").
pub const REACH: LightYears = LightYears::new(128.0);

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

    /// The nuclear cluster's stellar shape: inner slope 1.3, a break at 10 ly of sharpness
    /// [`BREAK_SHARPNESS`] and outer slope 3.5 (plan 02's [`NuclearClusterParams`]).
    ///
    /// # Panics
    ///
    /// Never: the parameters' slopes and break are constants inside the ranges.
    #[must_use]
    pub fn nuclear_cluster(params: &NuclearClusterParams) -> Self {
        Self::new(
            params.inner_slope(),
            [SlopeBreak {
                radius: params.break_radius().value(),
                sharpness: BREAK_SHARPNESS,
                rise: params.outer_slope() - params.inner_slope(),
            }],
        )
        .expect("the nuclear cluster's slopes are 1.3 and 3.5")
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
/// documentation): a tracer's number density per member, per cubic light-year.
#[derive(Debug, Clone, PartialEq)]
pub struct TracerProfile {
    shape: TracerShape,
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
    /// The profile of `shape`, normalised to one.
    ///
    /// # Panics
    ///
    /// Never: the knots' count is a constant, 417.
    #[must_use]
    pub fn new(shape: TracerShape) -> Self {
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
            ln_knots,
            inner,
            outer,
            total,
        }
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

/// The nuclear cluster's stellar profile about the central black hole, and the potential of the
/// two (module documentation).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::centre::CentreProfile;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let profile = CentreProfile::from_params(&GalaxyParams::milky_way_like())?;
/// // The escape speed is some 1,100 km/s at 0.1 ly and some 210 at 10 ly (the brainstorm).
/// let near = profile.escape_speed(0.1).value();
/// let far = profile.escape_speed(10.0).value();
/// assert!((1_000.0..1_250.0).contains(&near) && (180.0..240.0).contains(&far));
/// # Ok::<(), hyperion_sim::galaxy::features::centre::BuildCentreError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CentreProfile {
    stars: TracerProfile,
    mass: SolarMasses,
    black_hole: SolarMasses,
}

impl CentreProfile {
    /// The profile of a cluster of `cluster`'s mass and shape about a black hole of `black_hole`.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError::Mass`] if either mass is negative or not finite.
    pub fn new(
        cluster: &NuclearClusterParams,
        black_hole: SolarMasses,
    ) -> Result<Self, BuildCentreError> {
        for m in [cluster.mass().value(), black_hole.value()] {
            if !(m.is_finite() && m >= 0.0) {
                return Err(BuildCentreError::Mass(m));
            }
        }
        Ok(Self {
            stars: TracerProfile::new(TracerShape::nuclear_cluster(cluster)),
            mass: cluster.mass(),
            black_hole,
        })
    }

    /// The profile of a galaxy's parameters: its nuclear cluster and its black hole.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn from_params(
        params: &crate::galaxy::params::GalaxyParams,
    ) -> Result<Self, BuildCentreError> {
        Self::new(params.nuclear_cluster(), params.black_hole().mass())
    }

    /// The stars' profile, normalised to one.
    #[must_use]
    pub fn stars(&self) -> &TracerProfile {
        &self.stars
    }

    /// The cluster's total mass, the whole uncut law's.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
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

    /// The relative potential `Ψ(r) = −Φ(r)` at `r` ly, (km/s)², zero at infinity:
    /// `G (M_bh + M(<r)) ÷ r + 4πG ∫ᵣ^∞ ρ r′ dr′`.
    #[must_use]
    pub fn psi(&self, r: f64) -> f64 {
        G * self.enclosed_mass(r).value() / r + G * self.mass.value() * self.stars.outer_moment(r)
    }

    /// `dΨ ÷ dr` at `r` ly, (km/s)² per light-year: `−G M(<r) ÷ r²`.
    #[must_use]
    pub fn psi_slope(&self, r: f64) -> f64 {
        -G * self.enclosed_mass(r).value() / (r * r)
    }

    /// `d²Ψ ÷ dr²` at `r` ly: `2 G M(<r) ÷ r³ − 4πG ρ(r)`.
    #[must_use]
    pub fn psi_curvature(&self, r: f64) -> f64 {
        2.0 * G * self.enclosed_mass(r).value() / (r * r * r)
            - 4.0 * core::f64::consts::PI * G * self.density(r)
    }

    /// The escape speed from `r` ly out of the black hole and the cluster, `√(2Ψ)`.
    #[must_use]
    pub fn escape_speed(&self, r: f64) -> KilometresPerSecond {
        KilometresPerSecond::new((2.0 * self.psi(r)).sqrt())
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
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::{GalaxyParams, GalaxyParamsBuilder};
    use hyperion_testkit::float::assert_same_bits;

    fn milky_way() -> (GalaxyParams, CentreProfile) {
        let params = GalaxyParams::milky_way_like();
        let profile = CentreProfile::from_params(&params).unwrap();
        (params, profile)
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
        // An r^−3.5 tail leaves 2.4 × 10⁻⁶ of the mass beyond 10¹² ly.
        assert!((p.stars.fraction_within(1e12) - 1.0).abs() < 1e-5);
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
        assert!((s.log_slope(1e5) + 3.5).abs() < 1e-6);
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

    /// P09.T24.a at Milky Way values. The windows are the brainstorm's ("Dense features"): about
    /// 7,800 systems per cubic light-year at 3 ly (9,000 under Kroupa's), 4–5 × 10⁷ systems in
    /// the cluster, fewer than three inside 10⁻³ ly, and the stars outweighing the black hole
    /// near 10 ly. The density figure depends on the mean system mass (ruling 138 moves it), so
    /// the measured values are printed; its miss is provisional (Risks).
    #[test]
    fn the_milky_way_s_cluster_has_the_brainstorm_s_figures() {
        for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
            let params = GalaxyParamsBuilder::new()
                .mass_function(kind)
                .build()
                .unwrap();
            let p = CentreProfile::from_params(&params).unwrap();
            let mean = params.mean_system_mass(Population::NuclearDisc);
            let at_3 = p.systems_per_cubic_ly(3.0, mean);
            let systems = p.mass().value() / mean.value();
            let inside_reach = systems * p.stars.fraction_within(REACH.value());
            let core = systems * p.stars.fraction_within(CORE_RADIUS.value());
            let influence = p.influence_radius().value();
            eprintln!(
                "{kind:?}: mean {:.4} M☉, {at_3:.0} systems/ly³ at 3 ly ({:.0} M☉/ly³), \
                 {systems:.4e} systems ({inside_reach:.4e} inside 128 ly), {core:.3} inside \
                 10⁻³ ly, M(<10) {:.4e} against M_bh {:.4e}, influence radius {influence:.2} ly, \
                 v_esc {:.0} at 0.1 ly and {:.0} at 10 ly",
                mean.value(),
                p.density(3.0),
                p.stellar_mass_within(10.0).value(),
                p.black_hole().value(),
                p.escape_speed(0.1).value(),
                p.escape_speed(10.0).value(),
            );
            // Provisional window (Risks): the brainstorm's 7,800 (9,000) came from the measured
            // 1.5 × 10⁵ M☉ pc⁻³, and the fixture's law gives 15% less at 3 ly.
            let (lo, hi) = match kind {
                MassFunctionKind::Chabrier => (7_800.0 * 0.75, 7_800.0 * 1.25),
                MassFunctionKind::Kroupa => (9_000.0 * 0.75, 9_000.0 * 1.25),
            };
            assert!((lo..hi).contains(&at_3), "{kind:?}: {at_3} per ly³ at 3 ly");
            if kind == MassFunctionKind::Chabrier {
                assert!((4e7..5e7).contains(&systems), "{systems} systems");
            }
            assert!(core < 3.0, "{core} systems inside 10⁻³ ly");
            assert!(p.stellar_mass_within(10.0) > p.black_hole());
            assert!((5.0..15.0).contains(&influence), "{influence}");
        }
    }
}
