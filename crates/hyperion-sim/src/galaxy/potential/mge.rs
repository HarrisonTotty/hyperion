//! Axisymmetric Gaussian components: the discs, the bar and the bulge as the potential sees them
//! (plan 02, P02.T6.b).
//!
//! A Gaussian of mass M, width σ and axis ratio q has the density `ρ(R, z) = M ÷ ((2π)^(3÷2) σ³ q)
//! × exp(−(R² + z² ÷ q²) ÷ 2σ²)`. With `ε = 1 − q²`, its potential and forces are one-dimensional
//! integrals over `T` from 0 to 1 (Binney and Tremaine 2008, §2.5.2, the homoeoid form of
//! eq. 2.140 with `T² = 1 ÷ (1 + τ)`; Emsellem, Monnet and Bacon 1994, A&A 285, 723; Cappellari
//! 2002, MNRAS 333, 400, eq. 12):
//!
//! - `Φ(R, z) = −G M √(2 ÷ π) ÷ σ × ∫ exp(−T² (R² + z² ÷ (1 − εT²)) ÷ 2σ²) ÷ √(1 − εT²) dT`;
//! - `v_c²(R) = R ∂Φ ÷ ∂R = G M √(2 ÷ π) R² ÷ σ³ × ∫ T² exp(−T² R² ÷ 2σ²) ÷ √(1 − εT²) dT` in
//!   the plane, and its derivatives the same form with polynomials in `T²`;
//! - `K_z(R, z) = ∂Φ ÷ ∂z = G M √(2 ÷ π) z ÷ σ³ × ∫ T² exp(…) ÷ (1 − εT²)^(3÷2) dT`.
//!
//! They hold for prolate Gaussians too (`ε < 0`), which the nuclear disc's tall, narrow terms
//! produce. The profiles of the mass model are expanded into such Gaussians with the fitted
//! tables [`MGE_EXP`] and [`MGE_BAR`] by [`double_exponential`], [`spheroidal_exponential`] and
//! [`bar_disc`].
//!
//! # Quadrature
//!
//! Each integral is taken by the 32-point Gauss–Legendre rule on fixed panels, after a change of
//! variable that keeps the integrand smooth, because the expansions reach axis ratios from 10⁻⁴
//! to about 100 and a single rule in `T` fails at both ends:
//!
//! - The integrand is cut where the Gaussian factor falls below `e^(−40.5)`: `T` stops at
//!   `min(1, 9σ ÷ ρ)` with `ρ² = R² + z² ÷ max(1, q²)`, which the exponent always exceeds. Far
//!   from the Gaussian all 32 nodes then fall where the integrand lives.
//! - Near spherical (`|ε| ≤ ½`) the rule runs in `T` itself.
//! - Prolate, it runs in `v` with `T = sinh v ÷ √−ε`, which absorbs `1 ÷ √(1 − εT²)` into `dv`.
//! - Oblate in the plane, it runs in `v` with `T = sin v ÷ √ε`, which removes the endpoint
//!   singularity `1 ÷ √(1 − T²)` that a flat Gaussian has at `T = 1`.
//! - Oblate off the plane, it runs in `u` with `w = tan v = sinh u`, which is linear near 0 and
//!   logarithmic far out, from 0 to the Gaussian's cut in `w`, `9σ √ε ÷ |z|`, or to `√ε ÷ q`. In
//!   `w` the vertical factor `exp(−z² w² ÷ 2σ²ε)` is a plain Gaussian, and a flat Gaussian's
//!   integrand, which in `T` crowds into the last `q²` before 1, spreads over decades.
//!
//! Every choice depends only on the Gaussian and the point, so each value is a fixed sequence of
//! IEEE operations.

use super::BuildComponentError;
use crate::galaxy::consts::G;
use crate::math;
use crate::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
use crate::tables::mge::{MGE_BAR, MGE_EXP};
use crate::units::{LightYears, SolarMasses};

/// Where the integrands are cut, in standard deviations of their Gaussian factor: `e^(−9²÷2)`
/// is 2.6 × 10⁻¹⁸.
const CUT: f64 = 9.0;

/// At or below this `|ε|` the rule runs in `T`: `1 − εT²` then stays within `[½, 3÷2]`.
const NEAR_SPHERICAL: f64 = 0.5;

/// How a Gaussian's integrals change variable (module documentation, "Quadrature").
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    /// `|ε| ≤ ½`: the rule runs in `T`.
    NearSpherical { eps: f64 },
    /// `ε > ½`: in `v` in the plane, in `w` off it. `root` is `√ε`.
    Oblate { eps: f64, root: f64 },
    /// `ε < −½`: in `v` with `T = sinh v ÷ √−ε`. `root` is `√−ε`.
    Prolate { minus_eps: f64, root: f64 },
}

/// An axisymmetric Gaussian mass component, centred on the origin with its short (or long) axis
/// along z.
///
/// Potentials are in (km/s)², zero at infinity; forces in (km/s)² per light-year; radii in
/// light-years in the galactic frame (plan 02, Design note 1).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::potential::mge::Gaussian;
/// use hyperion_sim::units::{LightYears, SolarMasses};
///
/// # fn main() -> Result<(), hyperion_sim::galaxy::potential::BuildComponentError> {
/// // A flattened Gaussian's potential and circular speed, far outside it and near its centre.
/// let g = Gaussian::new(SolarMasses::new(1e10), LightYears::new(3_000.0), 0.3)?;
/// let far = LightYears::new(1e7);
/// let kepler = hyperion_sim::galaxy::consts::G * 1e10 / 1e7;
/// assert!((g.v_circ_sq(far) / kepler - 1.0).abs() < 1e-6);
/// assert!(g.potential(LightYears::ZERO, LightYears::ZERO) < g.potential(far, LightYears::ZERO));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Gaussian {
    mass: SolarMasses,
    sigma: LightYears,
    q: f64,
    shape: Shape,
    /// `G M √(2 ÷ π) ÷ σ`, (km/s)².
    amplitude: f64,
    /// `1 ÷ 2σ²`, ly⁻².
    a: f64,
    /// `1 ÷ max(1, q²)`: the weight of `z²` in the radius that sets the cut.
    z_weight: f64,
    /// The nodes of the whole range, which most points use.
    whole: Box<WholeRange>,
}

/// One node of a quadrature: `T²`, `1 ÷ (1 − εT²)` and the weight of `f(T) ÷ √(1 − εT²)`.
type Node = [f64; 3];

/// The nodes for a point within the cut, `T_c = 1`, generated once: in the plane (the oblate
/// rule in `v`; other shapes use the same rule in and off the plane) and off it without the
/// vertical cut. They are the very nodes [`Gaussian::generate`] gives for those arguments, so
/// using them changes no bit.
#[derive(Debug, Clone, PartialEq)]
struct WholeRange {
    /// Filled for an oblate Gaussian only; other shapes use `off_plane` in the plane.
    in_plane: [Node; 32],
    off_plane: [Node; 32],
    /// The oblate rule's end in `w` for `T_c = 1`, `√ε ÷ q`; 0 for other shapes.
    w_end: f64,
}

/// A Gaussian's in-plane quantities at one radius, for the potential tables.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct InPlane {
    /// `v_c²`, (km/s)².
    pub v_circ_sq: f64,
    /// `d v_c² ÷ d ln R`, (km/s)².
    pub slope: f64,
    /// `d² v_c² ÷ d (ln R)²`, (km/s)².
    pub curvature: f64,
    /// `Φ(R, 0)`, (km/s)².
    pub potential: f64,
}

impl std::ops::AddAssign for InPlane {
    fn add_assign(&mut self, rhs: Self) {
        self.v_circ_sq += rhs.v_circ_sq;
        self.slope += rhs.slope;
        self.curvature += rhs.curvature;
        self.potential += rhs.potential;
    }
}

/// A Gaussian's potential and its first logarithmic derivatives at one point, for the (R, z)
/// grid: `Φ`, `R ∂Φ ÷ ∂R`, `z ∂Φ ÷ ∂z` and `R z ∂²Φ ÷ ∂R ∂z`, all (km/s)².
pub(crate) type GridPoint = [f64; 4];

impl Gaussian {
    /// A Gaussian of `mass`, width `sigma` along R and axis ratio `q` (its width along z is
    /// `q σ`; `q > 1` is prolate).
    ///
    /// # Errors
    ///
    /// [`BuildComponentError`] if the mass is negative or any value is not finite, or the width
    /// or the axis ratio is not positive.
    pub fn new(mass: SolarMasses, sigma: LightYears, q: f64) -> Result<Self, BuildComponentError> {
        BuildComponentError::check_non_negative("mass", mass.value())?;
        BuildComponentError::check_positive("width", sigma.value())?;
        BuildComponentError::check_positive("axis ratio", q)?;
        let eps = (1.0 - q) * (1.0 + q);
        let shape = if eps.abs() <= NEAR_SPHERICAL {
            Shape::NearSpherical { eps }
        } else if eps > 0.0 {
            Shape::Oblate {
                eps,
                root: eps.sqrt(),
            }
        } else {
            Shape::Prolate {
                minus_eps: -eps,
                root: (-eps).sqrt(),
            }
        };
        let s = sigma.value();
        let mut gaussian = Self {
            mass,
            sigma,
            q,
            shape,
            amplitude: G * mass.value() * (2.0 / core::f64::consts::PI).sqrt() / s,
            a: 0.5 / (s * s),
            z_weight: 1.0 / (q * q).max(1.0),
            whole: Box::new(WholeRange {
                in_plane: [[0.0; 3]; 32],
                off_plane: [[0.0; 3]; 32],
                w_end: 0.0,
            }),
        };
        let mut whole = WholeRange {
            in_plane: [[0.0; 3]; 32],
            off_plane: [[0.0; 3]; 32],
            w_end: gaussian.oblate_w_end(1.0, 0.0),
        };
        let mut i = 0;
        gaussian.generate(1.0, 0.0, false, |t2, inv_d, jac| {
            whole.off_plane[i] = [t2, inv_d, jac];
            i += 1;
        });
        // Only an oblate Gaussian has a rule of its own in the plane; the others read
        // `off_plane` there too, which is the same rule.
        if matches!(shape, Shape::Oblate { .. }) {
            i = 0;
            gaussian.generate(1.0, 0.0, true, |t2, inv_d, jac| {
                whole.in_plane[i] = [t2, inv_d, jac];
                i += 1;
            });
        }
        *gaussian.whole = whole;
        Ok(gaussian)
    }

    /// The mass.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The width σ along R.
    #[must_use]
    pub fn sigma(&self) -> LightYears {
        self.sigma
    }

    /// The axis ratio q: the width along z over the width along R.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        self.q
    }

    /// The bytes each Gaussian owns on the heap: its nodes for the whole range.
    pub(crate) const HEAP_BYTES: usize = size_of::<WholeRange>();

    /// The density at `(R, z)`, M☉ per cubic light-year.
    #[must_use]
    pub fn density(&self, r_cyl: LightYears, z: LightYears) -> f64 {
        let s = self.sigma.value();
        let (r, z) = (r_cyl.value(), z.value() / self.q);
        let two_pi = 2.0 * core::f64::consts::PI;
        let norm = two_pi * two_pi.sqrt() * s * s * s * self.q;
        self.mass.value() / norm * math::exp(-self.a * (r * r + z * z))
    }

    /// The upper end of the integral in `T` at a point: where the Gaussian factor has fallen by
    /// `e^(−CUT²÷2)`.
    fn t_cut(&self, r_cyl: f64, z: f64) -> f64 {
        let rho = (r_cyl * r_cyl + z * z * self.z_weight).sqrt();
        (CUT * self.sigma.value() / rho).min(1.0)
    }

    /// Calls `visit(t², 1 ÷ (1 − εT²), jac)` at each node of the quadrature for the point `(R, z)`,
    /// where `Σ jac f(T) ≈ ∫₀^Tc f(T) ÷ √(1 − εT²) dT`. `in_plane` selects the oblate
    /// Gaussian's rule for `z = 0`.
    fn for_each_node(
        &self,
        r_cyl: f64,
        z: f64,
        in_plane: bool,
        mut visit: impl FnMut(f64, f64, f64),
    ) {
        let t_cut = self.t_cut(r_cyl, z);
        let oblate = matches!(self.shape, Shape::Oblate { .. });
        let cached = if t_cut < 1.0 {
            None
        } else if oblate && in_plane {
            Some(&self.whole.in_plane)
        } else if !oblate || self.oblate_w_end(1.0, z) >= self.whole.w_end {
            Some(&self.whole.off_plane)
        } else {
            None
        };
        match cached {
            Some(nodes) => {
                for &[t2, inv_d, jac] in nodes {
                    visit(t2, inv_d, jac);
                }
            }
            None => self.generate(t_cut, z, in_plane, visit),
        }
    }

    /// The oblate rule's end in `w = tan v` for the cut `t_cut` in `T` and height `z`: `√ε T_c ÷
    /// √(1 − εT_c²)`, or the vertical factor's cut `9σ √ε ÷ |z|` if that is nearer. 0 for other
    /// shapes.
    fn oblate_w_end(&self, t_cut: f64, z: f64) -> f64 {
        let Shape::Oblate { root, .. } = self.shape else {
            return 0.0;
        };
        // 1 − εT_c² = (1 − T_c)(1 + T_c) + q² T_c², without cancellation.
        let d_cut = (1.0 - t_cut) * (1.0 + t_cut) + self.q * self.q * t_cut * t_cut;
        let w_end = root * t_cut / d_cut.sqrt();
        if z == 0.0 {
            w_end
        } else {
            w_end.min(CUT * self.sigma.value() * root / z.abs())
        }
    }

    /// The nodes of the rule for the cut `t_cut` in `T` and height `z`, as
    /// [`for_each_node`](Self::for_each_node) describes.
    fn generate(&self, t_cut: f64, z: f64, in_plane: bool, mut visit: impl FnMut(f64, f64, f64)) {
        match self.shape {
            Shape::NearSpherical { eps } => {
                let half = 0.5 * t_cut;
                for (&x, &w) in GL32_NODES.iter().zip(&GL32_WEIGHTS) {
                    let t = half + half * x;
                    let t2 = t * t;
                    let d = 1.0 - eps * t2;
                    visit(t2, 1.0 / d, w * half / d.sqrt());
                }
            }
            Shape::Prolate { minus_eps, root } => {
                let half = 0.5 * math::asinh(root * t_cut);
                for (&x, &w) in GL32_NODES.iter().zip(&GL32_WEIGHTS) {
                    let s = math::sinh(half + half * x);
                    let s2 = s * s;
                    visit(s2 / minus_eps, 1.0 / (1.0 + s2), w * half / root);
                }
            }
            Shape::Oblate { eps, root } if in_plane => {
                let v_cut = if t_cut < 1.0 {
                    math::asin(root * t_cut)
                } else {
                    math::atan2(root, self.q)
                };
                let half = 0.5 * v_cut;
                for (&x, &w) in GL32_NODES.iter().zip(&GL32_WEIGHTS) {
                    let (sin, cos) = math::sin_cos(half + half * x);
                    visit(sin * sin / eps, 1.0 / (cos * cos), w * half / root);
                }
            }
            Shape::Oblate { eps, root } => {
                let w_end = self.oblate_w_end(t_cut, z);
                let mut emit = |w: f64, jac: f64| {
                    let w2 = w * w;
                    let one_w2 = 1.0 + w2;
                    visit(w2 / (eps * one_w2), one_w2, jac / (root * one_w2));
                };
                // w = sinh u: linear near 0, logarithmic far out, with dw = cosh u du.
                let half = 0.5 * math::asinh(w_end);
                for (&x, &weight) in GL32_NODES.iter().zip(&GL32_WEIGHTS) {
                    let w = math::sinh(half + half * x);
                    emit(w, weight * half * (1.0 + w * w).sqrt());
                }
            }
        }
    }

    /// The potential at `(R, z)`, (km/s)², zero at infinity.
    #[must_use]
    pub fn potential(&self, r_cyl: LightYears, z: LightYears) -> f64 {
        let (r, z) = (r_cyl.value(), z.value());
        let (r2, z2) = (r * r, z * z);
        let mut sum = 0.0;
        self.for_each_node(r, z, z == 0.0, |t2, inv_d, jac| {
            sum += jac * math::exp(-self.a * t2 * (r2 + z2 * inv_d));
        });
        -self.amplitude * sum
    }

    /// The vertical force `K_z = ∂Φ ÷ ∂z` at `(R, z)`, (km/s)² per light-year: positive above the
    /// plane, where it pulls towards it, and odd in z.
    #[must_use]
    pub fn vertical_force(&self, r_cyl: LightYears, z: LightYears) -> f64 {
        let (r, z) = (r_cyl.value(), z.value());
        if z == 0.0 {
            return 0.0;
        }
        let (r2, z2) = (r * r, z * z);
        let mut sum = 0.0;
        self.for_each_node(r, z, false, |t2, inv_d, jac| {
            sum += jac * t2 * inv_d * math::exp(-self.a * t2 * (r2 + z2 * inv_d));
        });
        self.amplitude * 2.0 * self.a * z * sum
    }

    /// The in-plane circular speed squared `v_c² = R ∂Φ ÷ ∂R` at radius `r`, (km/s)².
    #[must_use]
    pub fn v_circ_sq(&self, r: LightYears) -> f64 {
        self.in_plane(r.value()).v_circ_sq
    }

    /// `d v_c² ÷ dR` at radius `r`, (km/s)² per light-year.
    #[must_use]
    pub fn v_circ_sq_derivative(&self, r: LightYears) -> f64 {
        self.in_plane(r.value()).slope / r.value()
    }

    /// `v_c²`, its first two derivatives in `ln R` and `Φ(R, 0)` at radius `r` (ly), from one set
    /// of exponentials: with `Jₙ = ∫ T²ⁿ exp(−aR²T²) ÷ √(1 − εT²) dT` and `a = 1 ÷ 2σ²`,
    /// `v_c² = A 2aR² J₁`, `d v_c² ÷ d ln R = A 2aR² (2J₁ − 2aR²J₂)`, `d² v_c² ÷ d(ln R)² = A 2aR²
    /// (4J₁ − 12aR²J₂ + 4a²R⁴J₃)` and `Φ = −A J₀`, where `A = G M √(2 ÷ π) ÷ σ`.
    pub(crate) fn in_plane(&self, r: f64) -> InPlane {
        let x = self.a * r * r;
        let mut j = [0.0; 4];
        self.for_each_node(r, 0.0, true, |t2, _, jac| {
            let e = jac * math::exp(-x * t2);
            j[0] += e;
            j[1] += e * t2;
            j[2] += e * t2 * t2;
            j[3] += e * t2 * t2 * t2;
        });
        let scale = self.amplitude * 2.0 * x;
        InPlane {
            v_circ_sq: scale * j[1],
            slope: scale * (2.0 * j[1] - 2.0 * x * j[2]),
            curvature: scale * (4.0 * j[1] - 12.0 * x * j[2] + 4.0 * x * x * j[3]),
            potential: -self.amplitude * j[0],
        }
    }

    /// `Φ`, `R ∂Φ ÷ ∂R`, `z ∂Φ ÷ ∂z` and `R z ∂²Φ ÷ ∂R ∂z` at `(R, z)` (ly), for bicubic
    /// interpolation in the logarithms. With `E = exp(−aT²(R² + z² ÷ D))`, `D = 1 − εT²`:
    /// `Φ = −A ∫E`, `R ∂Φ ÷ ∂R = A 2aR² ∫T²E`, `z ∂Φ ÷ ∂z = A 2az² ∫T²E ÷ D` and the cross term
    /// `−A 4a²R²z² ∫T⁴E ÷ D`, each `∫ … ÷ √D dT`.
    pub(crate) fn grid_point(&self, r: f64, z: f64) -> GridPoint {
        let (r2, z2) = (r * r, z * z);
        let mut m = [0.0; 4];
        self.for_each_node(r, z, z == 0.0, |t2, inv_d, jac| {
            let e = jac * math::exp(-self.a * t2 * (r2 + z2 * inv_d));
            m[0] += e;
            m[1] += e * t2;
            m[2] += e * t2 * inv_d;
            m[3] += e * t2 * t2 * inv_d;
        });
        let two_a = 2.0 * self.a;
        [
            -self.amplitude * m[0],
            self.amplitude * two_a * r2 * m[1],
            self.amplitude * two_a * z2 * m[2],
            -self.amplitude * two_a * two_a * r2 * z2 * m[3],
        ]
    }

    /// The mass inside the sphere of radius `r`.
    ///
    /// With `g(x) = ∫₀ˣ u² e^(−u²÷2) du` and `k = 1 + μ² ε ÷ q²` along the direction of cosine `μ`
    /// to the z axis, `M(<r) = M √(2 ÷ π) ÷ q × ∫₀¹ k^(−3÷2) g(r√k ÷ σ) dμ`. Near spherical the
    /// rule runs in `μ`; oblate, in `ψ` with `tan ψ = μ √ε ÷ q`, which gives `M √(2 ÷ π) ÷ √ε ×
    /// ∫₀^acos q cos ψ g(r ÷ σ cos ψ) dψ`; prolate, in `t` with `sin ψ = μ √−ε ÷ q` and `t = tan
    /// ψ`, which gives `M √(2 ÷ π) ÷ √−ε × ∫₀^√−ε g(r ÷ σ√(1 + t²)) dt`, on `[0, 1]` and in `ln t`
    /// beyond.
    #[must_use]
    pub fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        let x = r.value() / self.sigma.value();
        let m = self.mass.value();
        let root_two_over_pi = (2.0 / core::f64::consts::PI).sqrt();
        let fraction = match self.shape {
            Shape::NearSpherical { eps } => {
                let q2 = self.q * self.q;
                crate::galaxy::quad::gl32(
                    |mu| {
                        let k = 1.0 + mu * mu * eps / q2;
                        g_integral(x * k.sqrt()) / (k * k.sqrt())
                    },
                    0.0,
                    1.0,
                ) * root_two_over_pi
                    / self.q
            }
            Shape::Oblate { root, .. } => {
                crate::galaxy::quad::gl32(
                    |psi| {
                        let cos = math::cos(psi);
                        cos * g_integral(x / cos)
                    },
                    0.0,
                    math::atan2(root, self.q),
                ) * root_two_over_pi
                    / root
            }
            Shape::Prolate { root, .. } => {
                let f = |t: f64| g_integral(x / (1.0 + t * t).sqrt());
                let inner = crate::galaxy::quad::gl32(f, 0.0, root.min(1.0));
                let outer = if root > 1.0 {
                    crate::galaxy::quad::gl32_log(f, 1.0, root)
                } else {
                    0.0
                };
                (inner + outer) * root_two_over_pi / root
            }
        };
        SolarMasses::new(m * fraction)
    }
}

/// `g(x) = ∫₀ˣ u² e^(−u²÷2) du = √(π ÷ 2) erf(x ÷ √2) − x e^(−x²÷2)`, by its series below
/// `x = ½`, where the closed form cancels.
fn g_integral(x: f64) -> f64 {
    if x < 0.5 {
        // Σₙ (−1)ⁿ x^(2n+3) ÷ (2ⁿ n! (2n + 3)): each term is x² ÷ 2n times the last, so twelve
        // terms reach 10⁻¹⁹ of the first at x = ½.
        let x2 = x * x;
        let mut power = x * x2;
        let mut sum = 0.0;
        let mut n = 0.0;
        for _ in 0..12 {
            sum += power / (2.0 * n + 3.0);
            n += 1.0;
            power *= -x2 / (2.0 * n);
        }
        sum
    } else {
        (0.5 * core::f64::consts::PI).sqrt() * math::erf(x * core::f64::consts::FRAC_1_SQRT_2)
            - x * math::exp(-0.5 * x * x)
    }
}

/// The Gaussians of an expansion whose weights and widths are `terms`, with masses in proportion
/// to `weight × (their other factors)` and scaled so that they sum to `mass` exactly: the total
/// mass is the component's, whatever the expansion's own small mass error.
fn normalised(
    mass: SolarMasses,
    terms: impl Iterator<Item = (f64, LightYears, f64)> + Clone,
) -> Result<Vec<Gaussian>, BuildComponentError> {
    let total = terms.clone().fold(0.0, |sum, (m, _, _)| sum + m);
    terms
        .map(|(m, sigma, q)| Gaussian::new(mass * (m / total), sigma, q))
        .collect()
}

/// The table's terms with a positive weight.
fn nonzero(table: &[(f64, f64)]) -> impl Iterator<Item = (f64, f64)> + Clone + '_ {
    table.iter().copied().filter(|&(w, _)| w > 0.0)
}

/// Checks a component's mass and two lengths.
fn check(
    mass: SolarMasses,
    lengths: [(&'static str, LightYears); 2],
) -> Result<(), BuildComponentError> {
    BuildComponentError::check_non_negative("mass", mass.value())?;
    for (name, length) in lengths {
        BuildComponentError::check_positive(name, length.value())?;
    }
    Ok(())
}

/// A double-exponential disc, `ρ ∝ e^(−R ÷ length) e^(−|z| ÷ height)`, as Gaussians.
///
/// Both factors are expanded with [`MGE_EXP`], and the product of the radial term `(wᵢ, sᵢ)` and
/// the vertical term `(wⱼ, sⱼ)` is a Gaussian of width `sᵢ length` and axis ratio `sⱼ height ÷
/// sᵢ length`, whose mass is proportional to `wᵢ sᵢ² wⱼ sⱼ`. Radial terms are the outer loop.
///
/// # Errors
///
/// [`BuildComponentError`] if the mass is negative or a length is not positive.
pub fn double_exponential(
    mass: SolarMasses,
    length: LightYears,
    height: LightYears,
) -> Result<Vec<Gaussian>, BuildComponentError> {
    check(mass, [("length", length), ("height", height)])?;
    let terms = nonzero(&MGE_EXP).flat_map(move |(wi, si)| {
        nonzero(&MGE_EXP).map(move |(wj, sj)| {
            (
                wi * si * si * wj * sj,
                length * si,
                (height * sj) / (length * si),
            )
        })
    });
    normalised(mass, terms)
}

/// A spheroidal exponential, `ρ ∝ exp(−√(R² ÷ a_r² + z² ÷ a_z²))`, as Gaussians: [`MGE_EXP`] in
/// the spheroidal radius, each term a Gaussian of width `s a_r` and axis ratio `a_z ÷ a_r`, of
/// mass proportional to `w s³`.
///
/// # Errors
///
/// [`BuildComponentError`] if the mass is negative or a length is not positive.
pub fn spheroidal_exponential(
    mass: SolarMasses,
    a_r: LightYears,
    a_z: LightYears,
) -> Result<Vec<Gaussian>, BuildComponentError> {
    check(mass, [("radial scale", a_r), ("vertical scale", a_z)])?;
    let q = a_z / a_r;
    let terms = nonzero(&MGE_EXP).map(move |(w, s)| (w * s * s * s, a_r * s, q));
    normalised(mass, terms)
}

/// The long bar as an axisymmetric disc: its azimuthally averaged surface density ([`MGE_BAR`],
/// in half-lengths) times `e^(−|z| ÷ height)` ([`MGE_EXP`]). The product of the radial term
/// `(bᵢ, βᵢ)` and the vertical term `(wⱼ, sⱼ)` is a Gaussian of width `βᵢ half_length` and axis
/// ratio `sⱼ height ÷ βᵢ half_length`, of mass proportional to `bᵢ βᵢ² wⱼ sⱼ`.
///
/// # Errors
///
/// [`BuildComponentError`] if the mass is negative or a length is not positive.
pub fn bar_disc(
    mass: SolarMasses,
    half_length: LightYears,
    height: LightYears,
) -> Result<Vec<Gaussian>, BuildComponentError> {
    check(mass, [("half-length", half_length), ("height", height)])?;
    let terms = nonzero(&MGE_BAR).flat_map(move |(bi, beta)| {
        nonzero(&MGE_EXP).map(move |(wj, sj)| {
            (
                bi * beta * beta * wj * sj,
                half_length * beta,
                (height * sj) / (half_length * beta),
            )
        })
    });
    normalised(mass, terms)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Composite Simpson's rule for `∫ₐᵇ f` with `n` (even) intervals.
    fn simpson(integrand: impl Fn(f64) -> f64, lo: f64, hi: f64, intervals: u32) -> f64 {
        let step = (hi - lo) / f64::from(intervals);
        let mut sum = integrand(lo) + integrand(hi);
        for i in 1..intervals {
            sum += integrand(lo + step * f64::from(i)) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        sum * step / 3.0
    }

    /// `∫₀¹ f(T) ÷ √(1 − εT²) dT` by brute force, knowing nothing of the scheme under test:
    /// `T = cos φ`, and Simpson's rule with 40,000 intervals in `ln φ` from 10⁻¹² to π ÷ 2,
    /// which resolves both a flat Gaussian's end at `T → 1` and a distant point's Gaussian at
    /// `T → 0`.
    fn reference(
        gaussian: &Gaussian,
        r_cyl: f64,
        height: f64,
        moment: impl Fn(f64, f64) -> f64,
    ) -> f64 {
        let (lo, hi) = (math::ln(1e-12), math::ln(0.5 * core::f64::consts::PI));
        let integrand = |log_phi: f64| {
            let phi = math::exp(log_phi);
            let (sin, cos) = math::sin_cos(phi);
            let t2 = cos * cos;
            // 1 − εT² = sin²φ + q² cos²φ, without cancellation.
            let q2 = gaussian.q * gaussian.q;
            let denominator = sin * sin + q2 * t2;
            let exponent = gaussian.a * t2 * (r_cyl * r_cyl + height * height / denominator);
            phi * sin * math::exp(-exponent) / denominator.sqrt() * moment(t2, denominator)
        };
        simpson(integrand, lo, hi, 40_000)
    }

    fn gaussians() -> Vec<Gaussian> {
        [1e-4, 0.01, 0.3, 0.72, 1.0, 1.2, 3.0, 40.0]
            .iter()
            .map(|&q| Gaussian::new(SolarMasses::new(1e9), LightYears::new(1_000.0), q).unwrap())
            .collect()
    }

    /// Every change of variable against the brute-force reference, at points inside, near and
    /// far from the Gaussian, in and off the plane.
    #[test]
    fn the_quadrature_matches_a_brute_force_integral() {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (2_000.0, 0.0),
            (30_000.0, 0.0),
            (500.0, 0.05),
            (500.0, 3.0),
            (1_500.0, 40.0),
            (0.0, 700.0),
            (8_000.0, 9_000.0),
            (100.0, 1e-3),
            (3_000.0, 0.5),
            (50.0, 5_000.0),
            (20_000.0, 100.0),
        ];
        for g in gaussians() {
            for &(r, z) in &points {
                let what = format!("q {} at ({r}, {z})", g.q);
                let phi = -g.amplitude * reference(&g, r, z, |_, _| 1.0);
                let ours = g.potential(LightYears::new(r), LightYears::new(z));
                assert!(
                    (ours / phi - 1.0).abs() < 1e-7,
                    "Φ {what}: {ours} against {phi}"
                );
                if z > 0.0 {
                    let kz = g.amplitude * 2.0 * g.a * z * reference(&g, r, z, |t2, d| t2 / d);
                    let ours = g.vertical_force(LightYears::new(r), LightYears::new(z));
                    assert!(
                        (ours / kz - 1.0).abs() < 1e-6,
                        "K_z {what}: {ours} against {kz}"
                    );
                } else if r > 0.0 {
                    let v2 = g.amplitude * 2.0 * g.a * r * r * reference(&g, r, 0.0, |t2, _| t2);
                    let ours = g.v_circ_sq(LightYears::new(r));
                    assert!(
                        (ours / v2 - 1.0).abs() < 1e-7,
                        "v² {what}: {ours} against {v2}"
                    );
                }
            }
        }
    }

    /// The in-plane slope and curvature and the grid's derivatives against finite differences.
    #[test]
    fn derivatives_match_finite_differences() {
        let h = 1e-4;
        for g in gaussians() {
            for r in [200.0, 1_000.0, 4_000.0] {
                let at = g.in_plane(r);
                let up = g.in_plane(r * math::exp(h));
                let down = g.in_plane(r * math::exp(-h));
                let slope = (up.v_circ_sq - down.v_circ_sq) / (2.0 * h);
                let curvature = (up.slope - down.slope) / (2.0 * h);
                let phi_slope = (up.potential - down.potential) / (2.0 * h);
                let scale = at.v_circ_sq.abs() + at.slope.abs();
                assert!((at.slope - slope).abs() < 1e-6 * scale, "q {} slope", g.q);
                assert!(
                    (at.curvature - curvature).abs() < 1e-5 * scale,
                    "q {} curvature",
                    g.q
                );
                assert!(
                    (at.v_circ_sq - phi_slope).abs() < 1e-6 * scale,
                    "q {} dΦ",
                    g.q
                );
                for z in [30.0, 800.0] {
                    let p = g.grid_point(r, z);
                    let dr = (g.grid_point(r * math::exp(h), z)[0]
                        - g.grid_point(r * math::exp(-h), z)[0])
                        / (2.0 * h);
                    let dz = (g.grid_point(r, z * math::exp(h))[0]
                        - g.grid_point(r, z * math::exp(-h))[0])
                        / (2.0 * h);
                    let drz = (g.grid_point(r, z * math::exp(h))[1]
                        - g.grid_point(r, z * math::exp(-h))[1])
                        / (2.0 * h);
                    let scale = p[0].abs();
                    assert!((p[1] - dr).abs() < 1e-7 * scale, "q {} ∂R at z {z}", g.q);
                    assert!((p[2] - dz).abs() < 1e-7 * scale, "q {} ∂z at z {z}", g.q);
                    assert!((p[3] - drz).abs() < 1e-6 * scale, "q {} ∂R∂z at z {z}", g.q);
                    let kz = g.vertical_force(LightYears::new(r), LightYears::new(z));
                    assert!((p[2] - kz * z).abs() < 1e-9 * scale);
                }
            }
        }
    }

    /// The mass inside a sphere against a brute-force integral of the density, for every shape
    /// thick enough for the brute force to resolve, and a nearly razor-thin one against the
    /// thin limit `M (1 − e^(−r² ÷ 2σ²))`.
    #[test]
    fn the_enclosed_mass_integrates_the_density() {
        let thin = &gaussians()[0];
        for r in [100.0, 1_000.0, 5_000.0] {
            let x = r / thin.sigma.value();
            let limit = 1e9 * (1.0 - math::exp(-0.5 * x * x));
            let ours = thin.enclosed_mass(LightYears::new(r)).value();
            assert!(
                (ours / limit - 1.0).abs() < 1e-3,
                "thin, r {r}: {ours} against {limit}"
            );
        }
        for g in gaussians().into_iter().skip(1) {
            for r in [100.0, 1_000.0, 5_000.0] {
                // M(<r) = 4π ∫₀^r ∫₀¹ ρ(r′ √(1 − μ²), r′ μ) dμ r′² dr′, Simpson in both.
                let shell = |rr: f64| {
                    let ring = |mu: f64| {
                        g.density(
                            LightYears::new(rr * (1.0 - mu * mu).sqrt()),
                            LightYears::new(rr * mu),
                        )
                    };
                    simpson(ring, 0.0, 1.0, 2_000) * rr * rr
                };
                let brute = 4.0 * core::f64::consts::PI * simpson(shell, 0.0, r, 200);
                let ours = g.enclosed_mass(LightYears::new(r)).value();
                assert!(
                    (ours / brute - 1.0).abs() < 2e-3,
                    "q {} r {r}: {ours} against {brute}",
                    g.q
                );
            }
        }
    }

    #[test]
    fn the_series_of_g_meets_the_closed_form() {
        let below = g_integral(0.5_f64.next_down());
        let above = g_integral(0.5);
        assert!(
            ((below - above) / above).abs() < 1e-13,
            "{below} against {above}"
        );
    }

    #[test]
    fn invalid_components_are_rejected() {
        let m = SolarMasses::new(1.0);
        let l = LightYears::new(1.0);
        let quantity = |result: Result<_, BuildComponentError>| {
            result.map(|_: Gaussian| ()).unwrap_err().quantity
        };
        assert_eq!(
            quantity(Gaussian::new(SolarMasses::new(-1.0), l, 1.0)),
            "mass"
        );
        assert_eq!(
            quantity(Gaussian::new(m, LightYears::new(0.0), 1.0)),
            "width"
        );
        assert_eq!(quantity(Gaussian::new(m, l, f64::NAN)), "axis ratio");
        assert_eq!(
            double_exponential(m, l, LightYears::new(-1.0))
                .unwrap_err()
                .quantity,
            "height"
        );
        let error = Gaussian::new(m, l, 0.0).unwrap_err();
        assert_eq!(error.quantity, "axis ratio");
        assert_eq!(
            error.to_string(),
            "a mass component's axis ratio is 0, outside its range"
        );
    }
}
