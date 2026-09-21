//! The boxy bulge (brainstorm, "Populations"; plan 02, P02.T7.c).

use super::{BuildFieldError, Site};
use crate::galaxy::PointLy;
use crate::galaxy::params::BulgeParams;
use crate::math;
use crate::units::LightYears;

/// Below this `x = t^c∥`, `(1 + x)^(1 ÷ c∥)` is summed as its binomial series to `x⁶`: every
/// coefficient is below 1 in magnitude, so the remainder is under `x⁷ = 2⁻⁵⁶`, a sixteenth of the
/// last bit of 1.
const SERIES_LIMIT: f64 = 1.0 / 256.0;

/// The boxy triaxial bulge along the bar, `n0 exp(−m)` systems per cubic light-year, with
/// `m = {[(|x| ÷ a)² + (|y| ÷ b)²]^(c∥ ÷ 2) + (|z| ÷ c)^c∥}^(1 ÷ c∥)`.
///
/// The measured density falls exponentially along all three axes (Wegg and Gerhard 2013, MNRAS
/// 435, 1874); combining the vertical term with an exponent `c∥` of 3–4 makes the body boxy, and
/// the in-plane exponent is `c⊥ = 2`. `m` is a norm of `(x ÷ a, y ÷ b, z ÷ c)`, so the density
/// never rises with |x|, |y| or |z| and changes across a cell by no more than `m` of the cell's
/// diagonal. Since `m` is homogeneous of degree 1, `∫ exp(−m) dV = 3! V(c∥) a b c`, where `V` is
/// the volume of the unit body `m ≤ 1` ([`unit_volume`](Self::unit_volume)), and
/// `n0 = count ÷ (6 a b c V)` normalises it over all space in closed form (plan 02, Design note
/// 11).
///
/// `m` is evaluated as `M (1 + t^c∥)^(1 ÷ c∥)` with `M` the larger of the in-plane and vertical
/// terms and `t ≤ 1` their ratio, through `exp`, `ln` and `ln_1p`: the same function as the
/// formula's three powers, at about a third of their cost. Where `t^c∥` is below 1/256, which is
/// most of the disc, the bracket is its binomial series, exact to a sixteenth of the last bit, and
/// saves two of the five transcendental calls.
///
/// In floating point the density is non-increasing only to rounding: when the coordinate that
/// sets `M` grows by one unit in the last place, `M` rises by less than the rounding of the
/// bracket can take away, so the computed `exp(−m)` can rise by a few units in the last place of
/// `m` times `m`, under 10⁻¹⁴ of the density (measured stepping through `M = vertical` and
/// `t^c∥ = 1/256`). The nearest-corner bound of plan 02's P02.T8 therefore needs a relative margin
/// of at least that (plan 02, Risks, R16).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::fields::bulge::BoxyBulge;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let bulge = BoxyBulge::of(2.6e10, GalaxyParams::milky_way_like().bulge())?;
/// // A hundred times the solar neighbourhood's density at the centre (brainstorm, "Populations").
/// assert!((0.2..0.35).contains(&bulge.n0()));
/// // One scale length along each axis alone is m = 1.
/// let a = bulge.scale_x().value();
/// assert!((bulge.radius(a, 0.0, 0.0) - 1.0).abs() < 1e-15);
/// let above = bulge.density(&PointLy::new(0.0, 0.0, -bulge.scale_z().value()));
/// assert!((above / bulge.n0() - hyperion_sim::math::exp(-1.0)).abs() < 1e-15);
/// # Ok::<(), hyperion_sim::galaxy::fields::BuildFieldError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxyBulge {
    n0: f64,
    scale_x: LightYears,
    scale_y: LightYears,
    scale_z: LightYears,
    boxiness: f64,
    inv_x: f64,
    inv_y: f64,
    inv_z: f64,
    inv_boxiness: f64,
    /// The binomial coefficients `C(1 ÷ c∥, k)` for `k` = 1 to 6.
    series: [f64; 6],
}

impl BoxyBulge {
    /// A bulge of `count` systems over all space with the scale lengths `a`, `b` and `c` along
    /// x, y and z and the vertical exponent `boxiness` (c∥).
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative, a scale length is not positive, the
    /// boxiness is below 1 (where `m` is no longer a norm), or any value is not finite.
    pub fn new(
        count: f64,
        a: LightYears,
        b: LightYears,
        c: LightYears,
        boxiness: f64,
    ) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_non_negative("count", count)?;
        for (name, length) in [("scale x", a), ("scale y", b), ("scale z", c)] {
            BuildFieldError::check_positive(name, length.value())?;
        }
        if !(boxiness.is_finite() && boxiness >= 1.0) {
            return Err(BuildFieldError::new("boxiness", boxiness));
        }
        let volume = 6.0 * Self::unit_volume(boxiness) * a.value() * b.value() * c.value();
        Ok(Self {
            n0: count / volume,
            scale_x: a,
            scale_y: b,
            scale_z: c,
            boxiness,
            inv_x: 1.0 / a.value(),
            inv_y: 1.0 / b.value(),
            inv_z: 1.0 / c.value(),
            inv_boxiness: 1.0 / boxiness,
            series: binomial_series(1.0 / boxiness),
        })
    }

    /// The bulge of a galaxy's parameters holding `count` systems.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative or not finite.
    pub fn of(count: f64, bulge: &BulgeParams) -> Result<Self, BuildFieldError> {
        Self::new(
            count,
            bulge.scale_x(),
            bulge.scale_y(),
            bulge.scale_z(),
            bulge.boxiness(),
        )
    }

    /// The volume of the unit body `m ≤ 1` of boxiness `p`, in units of `a b c`:
    /// `V(p) = 4π B(2 ÷ p, 1 ÷ p + 1) ÷ p`, which is `4π ÷ 3` at `p = 2`.
    ///
    /// With `ϖ` the in-plane radius of the unit body, `V = 4π ∫₀¹ ϖ (1 − ϖ^p)^(1 ÷ p) dϖ`, and
    /// `w = ϖ^p` turns that into the Beta function.
    #[must_use]
    pub fn unit_volume(p: f64) -> f64 {
        let ln_beta =
            |x: f64, y: f64| math::ln_gamma(x) + math::ln_gamma(y) - math::ln_gamma(x + y);
        4.0 * core::f64::consts::PI * math::exp(ln_beta(2.0 / p, 1.0 / p + 1.0)) / p
    }

    /// The central density `n0`, systems per cubic light-year.
    #[must_use]
    pub fn n0(&self) -> f64 {
        self.n0
    }

    /// `a`, the scale length along the bar.
    #[must_use]
    pub fn scale_x(&self) -> LightYears {
        self.scale_x
    }

    /// `b`, the scale length across the bar in the plane.
    #[must_use]
    pub fn scale_y(&self) -> LightYears {
        self.scale_y
    }

    /// `c`, the vertical scale length.
    #[must_use]
    pub fn scale_z(&self) -> LightYears {
        self.scale_z
    }

    /// `c∥`, the vertical exponent.
    #[must_use]
    pub fn boxiness(&self) -> f64 {
        self.boxiness
    }

    /// The number of systems over all space, `6 a b c V(c∥) n0`.
    #[must_use]
    pub fn count(&self) -> f64 {
        6.0 * Self::unit_volume(self.boxiness)
            * self.scale_x.value()
            * self.scale_y.value()
            * self.scale_z.value()
            * self.n0
    }

    /// The dimensionless radius `m` at `(x, y, z)` (ly); only the magnitudes matter.
    #[must_use]
    pub fn radius(&self, x: f64, y: f64, z: f64) -> f64 {
        let (along, across) = (x * self.inv_x, y * self.inv_y);
        let planar = (along * along + across * across).sqrt();
        let vertical = z.abs() * self.inv_z;
        let (big, small) = if planar >= vertical {
            (planar, vertical)
        } else {
            (vertical, planar)
        };
        if big == 0.0 {
            return 0.0;
        }
        // t^p = exp(p ln t), which is 0 for t = 0.
        let power = math::exp(self.boxiness * math::ln(small / big));
        let bracket = if power < SERIES_LIMIT {
            let [c1, c2, c3, c4, c5, c6] = self.series;
            1.0 + power
                * (c1 + power * (c2 + power * (c3 + power * (c4 + power * (c5 + power * c6)))))
        } else {
            math::exp(math::ln_1p(power) * self.inv_boxiness)
        };
        big * bracket
    }

    /// The density `n0 exp(−m)` at `(x, y, z)` (ly), systems per cubic light-year.
    #[must_use]
    pub fn envelope(&self, x: f64, y: f64, z: f64) -> f64 {
        self.n0 * math::exp(-self.radius(x, y, z))
    }

    /// The density at `p`, systems per cubic light-year.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        self.envelope(p.x, p.y, p.z)
    }

    /// The density at `site`.
    pub(crate) fn density_at(&self, site: &Site) -> f64 {
        self.envelope(site.abs_x, site.abs_y, site.abs_z)
    }
}

/// `C(α, k) = α (α − 1) ⋯ (α − k + 1) ÷ k!` for `k` = 1 to 6.
fn binomial_series(alpha: f64) -> [f64; 6] {
    let mut coefficients = [0.0; 6];
    let mut c = 1.0;
    let mut k = 0.0;
    for coefficient in &mut coefficients {
        c *= (alpha - k) / (k + 1.0);
        *coefficient = c;
        k += 1.0;
    }
    coefficients
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::quad::gl_panels;

    /// `V(p) = 2π ∫₀¹ (1 − z^p)^(2 ÷ p) dz`, the unit body as a stack of discs, by panels that
    /// narrow towards `z = 1`, where the integrand's slope is infinite.
    #[test]
    fn the_unit_volume_matches_a_quadrature() {
        for p in [1.0, 2.0, 3.0, 3.5, 4.0] {
            let edges = [0.0, 0.5, 0.9, 0.99, 0.999, 0.999_9, 0.999_99, 1.0];
            let stack = 2.0
                * core::f64::consts::PI
                * gl_panels(
                    |z| math::exp(2.0 / p * math::ln_1p(-math::exp(p * math::ln(z)))),
                    &edges,
                );
            let closed = BoxyBulge::unit_volume(p);
            assert!(
                (stack / closed - 1.0).abs() < 1e-6,
                "p {p}: {stack} against {closed}"
            );
        }
        assert!((BoxyBulge::unit_volume(2.0) - 4.0 / 3.0 * core::f64::consts::PI).abs() < 1e-14);
        // p = 1 is the double cone ϖ + |z| ≤ 1, of volume 2π ÷ 3.
        assert!((BoxyBulge::unit_volume(1.0) - 2.0 / 3.0 * core::f64::consts::PI).abs() < 1e-14);
    }

    /// The evaluated `m` against the formula's three powers.
    #[test]
    fn the_radius_is_the_formula() {
        let bulge = BoxyBulge::new(
            1.0,
            LightYears::new(2_280.0),
            LightYears::new(1_440.0),
            LightYears::new(820.0),
            3.5,
        )
        .unwrap();
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0xb0);
        for _ in 0..10_000 {
            let x = 20_000.0 * (lcg.next_f64() - 0.5);
            let y = 20_000.0 * (lcg.next_f64() - 0.5);
            let z = 10_000.0 * (lcg.next_f64() - 0.5);
            let s = (x / 2_280.0) * (x / 2_280.0) + (y / 1_440.0) * (y / 1_440.0);
            let formula = math::powf(
                math::powf(s, 1.75) + math::powf(z.abs() / 820.0, 3.5),
                1.0 / 3.5,
            );
            let ours = bulge.radius(x, y, z);
            assert!(
                (ours / formula - 1.0).abs() < 1e-14,
                "{ours} against {formula}"
            );
        }
        assert!(bulge.radius(0.0, 0.0, 0.0).abs() < f64::MIN_POSITIVE);
        assert!((bulge.radius(0.0, 1_440.0, 0.0) - 1.0).abs() < 1e-15);
        assert!((bulge.radius(0.0, 0.0, 820.0) - 1.0).abs() < 1e-15);
    }

    /// The binomial series and the closed form meet at the switch, and the series is exact to the
    /// last bits below it.
    #[test]
    fn the_series_agrees_with_the_closed_form() {
        for p in [3.0, 3.5, 4.0] {
            let series = binomial_series(1.0 / p);
            for x in [1e-12, 1e-6, 1e-3, SERIES_LIMIT] {
                let [c1, c2, c3, c4, c5, c6] = series;
                let sum = 1.0 + x * (c1 + x * (c2 + x * (c3 + x * (c4 + x * (c5 + x * c6)))));
                let closed = math::exp(math::ln_1p(x) / p);
                assert!(
                    (sum / closed - 1.0).abs() < 4e-16,
                    "p {p}, x {x}: {sum} against {closed}"
                );
            }
        }
    }

    #[test]
    fn invalid_bulges_are_rejected() {
        let l = LightYears::new(1.0);
        assert_eq!(
            BoxyBulge::new(1.0, l, l, l, 0.5).unwrap_err().quantity(),
            "boxiness"
        );
        assert_eq!(
            BoxyBulge::new(1.0, l, LightYears::new(0.0), l, 3.0)
                .unwrap_err()
                .quantity(),
            "scale y"
        );
    }
}
