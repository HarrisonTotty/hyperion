//! Fixed-node quadrature and a fixed-iteration bisection.
//!
//! Every integral the galaxy model takes numerically goes through these, so that each one is a
//! fixed sequence of IEEE operations: nodes and weights from
//! [`tables::gauss_legendre`](crate::tables::gauss_legendre), summed in index order, panel after
//! panel. Nothing adapts to the integrand and nothing stops on a tolerance test, because either
//! would let a change in the last bit of one value move the number of steps taken and, with it,
//! every value downstream. The node count and panel edges of each call are part of the generator
//! version.

use crate::math;
use crate::tables::gauss_legendre::{
    GL4_NODES, GL4_WEIGHTS, GL16_NODES, GL16_WEIGHTS, GL32_NODES, GL32_WEIGHTS,
};

/// `∫ₐᵇ f` by the rule with these nodes and weights on `[−1, 1]`, mapped linearly onto `[a, b]`.
#[inline]
fn rule(nodes: &[f64], weights: &[f64], mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    let half = 0.5 * (b - a);
    let mid = a + half;
    let mut sum = 0.0;
    for (&x, &w) in nodes.iter().zip(weights) {
        sum += w * f(mid + half * x);
    }
    sum * half
}

/// `∫ₐᵇ f(x) dx` by the 32-point Gauss–Legendre rule: exact for polynomials of degree up to 63.
///
/// `b < a` gives the negated integral. The 32 evaluations of `f` run from `a` towards `b`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::quad::gl32;
///
/// // The mass inside the half-light radius of an exponential profile, in units of its total.
/// let inside = gl32(|x| x * hyperion_sim::math::exp(-x), 0.0, 1.678_346_99);
/// assert!((inside - 0.5).abs() < 1e-8);
/// ```
#[must_use]
pub fn gl32(f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    rule(&GL32_NODES, &GL32_WEIGHTS, f, a, b)
}

/// `∫ₐᵇ f(x) dx` by the 16-point Gauss–Legendre rule: exact for polynomials of degree up to 31.
#[must_use]
pub fn gl16(f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    rule(&GL16_NODES, &GL16_WEIGHTS, f, a, b)
}

/// `∫ₐᵇ f(x) dx` by the 4-point Gauss–Legendre rule: exact for polynomials of degree up to 7.
///
/// This is the cheapest rule the galaxy model uses: the map averages the bulge's and the halo's
/// density across a pixel's height with it (plan 02, P02.T10.b). Four nodes hold a relative 10⁻³
/// while the panel spans at most some eight scale heights of the integrand — for `exp(−z ÷ c)` on
/// `[0, d]` the rule reads 0.998 of the integral at `d ÷ c = 8`, 0.943 at 16 and 0.054 at 80 — which
/// the maps plan 04 renders meet with room to spare (pixels of 128 to 1,024 ly against the bulge's
/// vertical scale of some 820 ly), and a much coarser raster does not: a pixel 8,192 ly tall resting
/// on the plane under-reads its column by 8 × 10⁻³ (`galaxy/map.rs`, unit tests).
#[must_use]
pub fn gl4(f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    rule(&GL4_NODES, &GL4_WEIGHTS, f, a, b)
}

/// `∫ f(x) dx` from `edges[0]` to the last edge, by [`gl32`] on each panel between consecutive
/// edges, summed in order.
///
/// The edges are the caller's to choose: put one at every kink or jump of `f`, and make panels
/// narrow where `f` changes fast. Fewer than two edges give 0.
#[must_use]
pub fn gl_panels(mut f: impl FnMut(f64) -> f64, edges: &[f64]) -> f64 {
    edges
        .windows(2)
        .fold(0.0, |sum, panel| sum + gl32(&mut f, panel[0], panel[1]))
}

/// `∫ₐᵇ f(x) dx` for `0 < a, b`, by [`gl32`] in `u = ln x`: `∫ f(eᵘ) eᵘ du` from `ln a` to `ln b`.
///
/// This is the form for integrands that span decades, such as a mass function or a radial
/// profile, where equal steps in `x` would waste nearly every node on the top decade.
#[must_use]
pub fn gl32_log(mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    gl32(
        |u| {
            let x = math::exp(u);
            f(x) * x
        },
        math::ln(a),
        math::ln(b),
    )
}

/// `∫ₐᵇ f(x) dx` for `0 < a, b`, by [`gl16`] in `u = ln x`, as [`gl32_log`] does with 32 points.
#[must_use]
pub fn gl16_log(mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    gl16(
        |u| {
            let x = math::exp(u);
            f(x) * x
        },
        math::ln(a),
        math::ln(b),
    )
}

/// `∫ f(x) dx` from `edges[0]` to the last edge, all positive, by [`gl32_log`] on each panel
/// between consecutive edges, summed in order. Fewer than two edges give 0.
#[must_use]
pub fn gl_log_panels(mut f: impl FnMut(f64) -> f64, edges: &[f64]) -> f64 {
    edges
        .windows(2)
        .fold(0.0, |sum, panel| sum + gl32_log(&mut f, panel[0], panel[1]))
}

/// `LEGENDRE_AT_GL16[k][i]` is the Legendre polynomial `P_k` at node `i` of the 16-point rule, by
/// the recurrence `(k + 1) P_{k+1} = (2k + 1) x P_k − k P_{k−1}`, evaluated at compile time.
const LEGENDRE_AT_GL16: [[f64; 16]; 16] = {
    let mut table = [[0.0; 16]; 16];
    let mut i = 0;
    while i < 16 {
        let x = GL16_NODES[i];
        table[0][i] = 1.0;
        table[1][i] = x;
        let mut k = 1;
        let mut kf = 1.0;
        while k < 15 {
            table[k + 1][i] =
                ((2.0 * kf + 1.0) * x * table[k][i] - kf * table[k - 1][i]) / (kf + 1.0);
            k += 1;
            kf += 1.0;
        }
        i += 1;
    }
    table
};

/// A function on a panel `[a, b]`, known by its values at the panel's 16 Gauss–Legendre nodes, and
/// integrated from `a` to any point of the panel through the polynomial of degree 15 that takes
/// those values.
///
/// This is how a quadrature takes many partial integrals of one integrand without evaluating it
/// again: the 16 values fix the polynomial's Legendre coefficients, `c_k = (2k + 1) ÷ 2 ×
/// Σᵢ wᵢ fᵢ P_k(xᵢ)` (exact, because the rule integrates `P_k P_j` exactly), and `∫₋₁ᵗ P_k =
/// (P_{k+1}(t) − P_{k−1}(t)) ÷ (2k + 1)` gives any partial integral. Over the whole panel it is
/// the 16-point rule itself, and it is exact for polynomials of degree up to 15.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::quad::Gl16Panel;
///
/// // ∫₀ˣ eᵗ dt = eˣ − 1, from 16 values of eᵗ on [0, 1].
/// let values = Gl16Panel::nodes(0.0, 1.0).map(hyperion_sim::math::exp);
/// let panel = Gl16Panel::new(0.0, 1.0, &values);
/// let x = 0.3;
/// assert!((panel.integral_to(x) - (hyperion_sim::math::exp(x) - 1.0)).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gl16Panel {
    mid: f64,
    half: f64,
    coefficients: [f64; 16],
}

impl Gl16Panel {
    /// The 16 points of `[a, b]` at which [`new`](Self::new) takes its values, ascending.
    #[must_use]
    pub fn nodes(a: f64, b: f64) -> [f64; 16] {
        let half = 0.5 * (b - a);
        let mid = a + half;
        GL16_NODES.map(|x| mid + half * x)
    }

    /// The panel `[a, b]` of the function whose values at [`nodes(a, b)`](Self::nodes) are
    /// `values`.
    #[must_use]
    pub fn new(a: f64, b: f64, values: &[f64; 16]) -> Self {
        let half = 0.5 * (b - a);
        let mut coefficients = [0.0; 16];
        let mut order = 0.0;
        for (coefficient, legendre) in coefficients.iter_mut().zip(&LEGENDRE_AT_GL16) {
            let mut sum = 0.0;
            for ((&w, &f), &p) in GL16_WEIGHTS.iter().zip(values).zip(legendre) {
                sum += w * f * p;
            }
            *coefficient = (order + 0.5) * sum;
            order += 1.0;
        }
        Self {
            mid: a + half,
            half,
            coefficients,
        }
    }

    /// `∫ₐᵇ`: the 16-point rule on the values given.
    #[must_use]
    pub fn integral(&self) -> f64 {
        2.0 * self.coefficients[0] * self.half
    }

    /// The interpolating polynomial's value at `x` in `[a, b]`; `x` outside is clamped to the
    /// panel. At the nodes it returns the values given, to rounding.
    ///
    /// This is how a function that is costly to evaluate is read at many points from 16 values:
    /// the sub-discs' Jeans integrals read the vertical force this way (plan 02, Design note 9).
    #[must_use]
    pub fn value(&self, x: f64) -> f64 {
        let t = ((x - self.mid) / self.half).clamp(-1.0, 1.0);
        // P_{k−1} and P_k at t, stepping k up from 1.
        let mut below = 1.0;
        let mut at = t;
        let mut sum = self.coefficients[0] + self.coefficients[1] * t;
        let mut k = 1.0;
        for &coefficient in &self.coefficients[2..] {
            let above = ((2.0 * k + 1.0) * t * at - k * below) / (k + 1.0);
            sum += coefficient * above;
            below = at;
            at = above;
            k += 1.0;
        }
        sum
    }

    /// `∫ₐˣ` of the interpolating polynomial, for `x` in `[a, b]`; `x` outside is clamped to the
    /// panel.
    #[must_use]
    pub fn integral_to(&self, x: f64) -> f64 {
        let t = ((x - self.mid) / self.half).clamp(-1.0, 1.0);
        // P_{k−1}, P_k and P_{k+1} at t, stepping k up from 1.
        let mut below = 1.0;
        let mut at = t;
        let mut sum = self.coefficients[0] * (t + 1.0);
        let mut k = 1.0;
        for &coefficient in &self.coefficients[1..] {
            let above = ((2.0 * k + 1.0) * t * at - k * below) / (k + 1.0);
            sum += coefficient * (above - below) / (2.0 * k + 1.0);
            below = at;
            at = above;
            k += 1.0;
        }
        sum * self.half
    }
}

/// A root of `f` between `lo` and `hi` by bisection, in exactly `iterations` halvings.
///
/// `f(lo)` and `f(hi)` should differ in sign. The sign of `f(lo)` is taken once; each step
/// evaluates `f` at the midpoint and keeps the half whose ends still differ in sign, a zero
/// counting as positive. The result is the midpoint of the final bracket, within
/// `|hi − lo| × 2^−(iterations + 1)` of a sign change of `f`. Without a sign change it drifts to
/// `hi`. The count is fixed so that the answer does not rest on a tolerance test: 60 iterations
/// narrow any bracket of `f64`s to its last bits, and further steps change nothing.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::quad::bisect;
///
/// let root_two = bisect(|x| x * x - 2.0, 1.0, 2.0, 60);
/// assert!((root_two - std::f64::consts::SQRT_2).abs() < 1e-15);
/// ```
#[must_use]
pub fn bisect(mut f: impl FnMut(f64) -> f64, lo: f64, hi: f64, iterations: u32) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    let lo_negative = f(lo) < 0.0;
    for _ in 0..iterations {
        let mid = lo + 0.5 * (hi - lo);
        if (f(mid) < 0.0) == lo_negative {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo + 0.5 * (hi - lo)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn assert_relative(actual: f64, expected: f64, tolerance: f64) {
        let error = ((actual - expected) / expected).abs();
        assert!(
            error <= tolerance,
            "{actual} is {error:e} from {expected}, over {tolerance:e}"
        );
    }

    #[test]
    fn gl32_integrates_x_to_the_62_exactly() {
        let integral = gl32(|x| math::powi(x, 62), -1.0, 1.0);
        assert_relative(integral, 2.0 / 63.0, 1e-13);
    }

    #[test]
    fn gl16_integrates_x_to_the_30_exactly() {
        let integral = gl16(|x| math::powi(x, 30), -1.0, 1.0);
        assert_relative(integral, 2.0 / 31.0, 1e-13);
    }

    #[test]
    fn gl4_integrates_x_to_the_6_exactly() {
        assert_relative(gl4(|x| math::powi(x, 6), -1.0, 1.0), 2.0 / 7.0, 1e-15);
        assert_relative(gl4(|x| math::powi(x, 7) + 1.0, -1.0, 1.0), 2.0, 1e-15);
        // An exponential over a fifth of an e-fold, as a pixel's height spans: eight digits.
        assert_relative(
            gl4(|x| math::exp(-x), 0.0, 0.2),
            1.0 - math::exp(-0.2),
            1e-12,
        );
        assert_same_bits(gl4(|_| 1.0, 3.0, 3.0), 0.0);
    }

    #[test]
    fn rules_map_onto_any_interval_and_reverse_sign() {
        let cubic = |x: f64| 3.0 * x * x * x - x + 2.0;
        let exact = |x: f64| 0.75 * x * x * x * x - 0.5 * x * x + 2.0 * x;
        for (a, b) in [(0.0, 1.0), (-3.0, 5.0), (2.0, 2.5)] {
            assert_relative(gl32(cubic, a, b), exact(b) - exact(a), 1e-14);
            assert_relative(gl16(cubic, a, b), exact(b) - exact(a), 1e-14);
            assert_relative(gl32(cubic, b, a), exact(a) - exact(b), 1e-14);
        }
    }

    #[test]
    fn panels_add_up_in_order() {
        let f = |x: f64| math::exp(-x);
        let edges = [0.0, 1.0, 3.0, 10.0];
        let expected = gl32(f, 0.0, 1.0) + gl32(f, 1.0, 3.0) + gl32(f, 3.0, 10.0);
        assert_same_bits(gl_panels(f, &edges), expected);
        assert_relative(gl_panels(f, &edges), 1.0 - math::exp(-10.0), 1e-14);
        assert_same_bits(gl_panels(f, &[1.0]), 0.0);
        assert_same_bits(gl_panels(f, &[]), 0.0);
    }

    /// A power law across six decades, where a linear rule would fail.
    #[test]
    fn log_substitution_handles_integrands_over_decades() {
        let f = |x: f64| math::powf(x, -2.3);
        let exact = (math::powf(1e-3, -1.3) - math::powf(1e3, -1.3)) / 1.3;
        assert_relative(gl32_log(f, 1e-3, 1e3), exact, 1e-10);
        let edges = [1e-3, 1e-1, 1e1, 1e3];
        assert_relative(gl_log_panels(f, &edges), exact, 1e-13);
        assert_relative(gl16_log(|x| 1.0 / x, 1.0, 1e4), math::ln(1e4), 1e-15);
    }

    /// The Legendre table against closed forms of P₂, P₃ and P₁₅'s value at 1.
    #[test]
    fn the_legendre_table_holds_the_polynomials() {
        for (i, &x) in GL16_NODES.iter().enumerate() {
            assert!((LEGENDRE_AT_GL16[2][i] - 0.5 * (3.0 * x * x - 1.0)).abs() < 1e-15);
            assert!((LEGENDRE_AT_GL16[3][i] - 0.5 * (5.0 * x * x * x - 3.0 * x)).abs() < 1e-15);
            // P₁₆ vanishes at the nodes: (16 P₁₆ = 31 x P₁₅ − 15 P₁₄).
            let p16 = (31.0 * x * LEGENDRE_AT_GL16[15][i] - 15.0 * LEGENDRE_AT_GL16[14][i]) / 16.0;
            assert!(p16.abs() < 1e-14, "P₁₆ at node {i}: {p16}");
        }
    }

    /// Partial integrals are exact for a polynomial of degree 15, and the whole panel is the
    /// 16-point rule.
    #[test]
    fn a_panel_integrates_its_interpolant_exactly() {
        let (a, b) = (-0.7, 2.3);
        let poly = |x: f64| math::powi(x, 15) - 3.0 * math::powi(x, 8) + x - 2.0;
        let antiderivative =
            |x: f64| math::powi(x, 16) / 16.0 - math::powi(x, 9) / 3.0 + 0.5 * x * x - 2.0 * x;
        let panel = Gl16Panel::new(a, b, &Gl16Panel::nodes(a, b).map(poly));
        for i in 0..=20 {
            let x = a + (b - a) * f64::from(i) / 20.0;
            let exact = antiderivative(x) - antiderivative(a);
            let scale = antiderivative(b) - antiderivative(a);
            assert!(
                ((panel.integral_to(x) - exact) / scale).abs() < 1e-14,
                "at {x}: {} against {exact}",
                panel.integral_to(x)
            );
        }
        assert_same_bits(panel.integral_to(a), 0.0);
        assert_same_bits(panel.integral_to(a - 1.0), 0.0);
        assert_relative(panel.integral(), gl16(poly, a, b), 1e-14);
        assert_relative(panel.integral_to(b), panel.integral(), 1e-14);
    }

    /// The interpolant reproduces a polynomial of degree 15 everywhere on the panel, returns the
    /// values given at the nodes, and clamps outside.
    #[test]
    fn a_panel_evaluates_its_interpolant() {
        let (a, b) = (-0.7, 2.3);
        let poly = |x: f64| math::powi(x, 15) - 3.0 * math::powi(x, 8) + x - 2.0;
        let nodes = Gl16Panel::nodes(a, b);
        let values = nodes.map(poly);
        let panel = Gl16Panel::new(a, b, &values);
        for i in 0..=40 {
            let x = a + (b - a) * f64::from(i) / 40.0;
            let scale = poly(b).abs();
            assert!(
                ((panel.value(x) - poly(x)) / scale).abs() < 1e-13,
                "at {x}: {} against {}",
                panel.value(x),
                poly(x)
            );
        }
        for (&x, &v) in nodes.iter().zip(&values) {
            assert!(
                (panel.value(x) - v).abs() < 1e-13 * poly(b).abs(),
                "node {x}"
            );
        }
        assert_same_bits(panel.value(b + 1.0), panel.value(b + 2.0));
        assert_same_bits(panel.value(a - 1.0), panel.value(a - 2.0));
        assert_relative(panel.value(b + 1.0), panel.value(b), 1e-14);
        assert_relative(panel.value(a - 1.0), panel.value(a), 1e-14);
        let smooth = |u: f64| math::exp(u) * (1.0 + 0.3 * math::sin(4.0 * u));
        let panel = Gl16Panel::new(0.0, 1.0, &Gl16Panel::nodes(0.0, 1.0).map(smooth));
        for i in 0..=20 {
            let x = f64::from(i) / 20.0;
            assert!((panel.value(x) / smooth(x) - 1.0).abs() < 1e-12, "at {x}");
        }
    }

    /// A smooth function that is not a polynomial, over a panel as wide as the mass quadrature's.
    #[test]
    fn a_panel_integrates_a_smooth_function_to_the_rule_accuracy() {
        let (a, b) = (math::ln(0.08), math::ln(0.08) + 0.5);
        let f = |u: f64| math::exp(u) * (1.0 + 0.3 * math::sin(4.0 * u));
        let panel = Gl16Panel::new(a, b, &Gl16Panel::nodes(a, b).map(f));
        for i in 1..=10 {
            let x = a + (b - a) * f64::from(i) / 10.0;
            assert_relative(panel.integral_to(x), gl32(f, a, x), 1e-13);
        }
    }

    #[test]
    fn bisection_takes_a_fixed_number_of_steps() {
        let mut calls = 0;
        let root = bisect(
            |x| {
                calls += 1;
                x - 0.3
            },
            0.0,
            1.0,
            40,
        );
        assert_eq!(calls, 41, "one call at lo, then one per iteration");
        assert!((root - 0.3).abs() < 1e-12);
    }

    #[test]
    fn bisection_finds_roots_with_either_orientation() {
        let rising = bisect(math::ln, 0.1, 5.0, 60);
        let falling = bisect(|x| -math::ln(x), 0.1, 5.0, 60);
        assert!((rising - 1.0).abs() < 1e-15);
        assert!((falling - 1.0).abs() < 1e-15);
        let reversed = bisect(math::ln, 5.0, 0.1, 60);
        assert!((reversed - 1.0).abs() < 1e-15);
    }
}
