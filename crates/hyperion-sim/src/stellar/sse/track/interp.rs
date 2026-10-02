//! Interpolation between a segment's knots: cubic Hermite with the knots' own derivatives, held
//! monotone (Fritsch and Carlson 1980, SIAM J. Numer. Anal. 17, 238).
//!
//! A knot carries each quantity's exact derivative from the integration, so the cubic through two
//! knots is accurate to the fourth order in their spacing. Fritsch and Carlson's limit keeps it
//! monotone wherever the two values are, so a mass that only falls between knots never rises
//! between them: where a slope has the wrong sign it is set to zero, and where the two slopes are
//! too steep for the secant (α² + β² > 9, with α and β the slopes over the secant) both are scaled
//! back onto that circle. The limit acts on each interval alone, so the curve stays continuous at
//! the knots, where it takes the knots' values exactly.

/// One interval of a quantity between two knots: its ends `x0` < `x1`, the values `y0` and `y1`
/// there, and the slopes `d0` and `d1` (y per x).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Interval {
    pub(super) x0: f64,
    pub(super) x1: f64,
    pub(super) y0: f64,
    pub(super) y1: f64,
    pub(super) d0: f64,
    pub(super) d1: f64,
}

impl Interval {
    /// The monotone cubic Hermite interpolant at `x`.
    ///
    /// At `x0` it returns `y0` and at `x1` `y1`, bit for bit; the arithmetic form is output.
    #[must_use]
    pub(super) fn at(&self, x: f64) -> f64 {
        self.at_offset(x - self.x0)
    }

    /// The interpolant at `dx` past `x0`: [`Interval::at`] of `x0 + dx`, bit for bit where `dx`
    /// is `x − x0` rounded, and finer where the caller forms `dx` more precisely than an `x` far
    /// from zero can hold it (plan 14, P14.T45.d).
    #[must_use]
    pub(super) fn at_offset(&self, dx: f64) -> f64 {
        let Self { x0, x1, y0, y1, .. } = *self;
        let h = x1 - x0;
        let delta = (y1 - y0) / h;
        if delta.abs() > 0.0 {
            self.cubic(dx, h, delta)
        } else {
            y0
        }
    }

    /// [`Interval::at_offset`] for a non-zero secant `delta` over the width `h`.
    #[must_use]
    fn cubic(&self, dx: f64, h: f64, delta: f64) -> f64 {
        let Self { y0, y1, d0, d1, .. } = *self;
        // Positive ratios only, so that a slope of the wrong sign, or NaN, is flattened.
        let flatten = |ratio: f64| if ratio > 0.0 { ratio } else { 0.0 };
        let (mut alpha, mut beta) = (flatten(d0 / delta), flatten(d1 / delta));
        let norm = alpha * alpha + beta * beta;
        if norm > 9.0 {
            let scale = 3.0 / norm.sqrt();
            alpha *= scale;
            beta *= scale;
        }
        let (m0, m1) = (alpha * delta * h, beta * delta * h);
        let u = dx / h;
        let u2 = u * u;
        let u3 = u2 * u;
        let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
        let h10 = u3 - 2.0 * u2 + u;
        let h01 = 3.0 * u2 - 2.0 * u3;
        let h11 = u3 - u2;
        h00 * y0 + h10 * m0 + h01 * y1 + h11 * m1
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::math;

    fn monotone_hermite(x: f64, x0: f64, x1: f64, y0: f64, y1: f64, d0: f64, d1: f64) -> f64 {
        Interval {
            x0,
            x1,
            y0,
            y1,
            d0,
            d1,
        }
        .at(x)
    }

    #[test]
    fn the_interpolant_takes_the_knots_values_exactly() {
        let (x0, x1, y0, y1) = (1.25e9, 1.31e9, 0.873_4, 0.851_2);
        assert_same_bits(monotone_hermite(x0, x0, x1, y0, y1, -3e-10, -5e-10), y0);
        assert_same_bits(monotone_hermite(x1, x0, x1, y0, y1, -3e-10, -5e-10), y1);
    }

    /// With exact slopes a smooth function is reproduced to the fourth order: halving the
    /// interval cuts the error about sixteenfold.
    #[test]
    fn a_smooth_function_is_reproduced_to_the_fourth_order() {
        let f = |x: f64| math::exp(-x);
        let error = |h: f64| {
            let (x0, x1) = (0.3, 0.3 + h);
            (0..=64)
                .map(|i| {
                    let x = x0 + h * f64::from(i) / 64.0;
                    (monotone_hermite(x, x0, x1, f(x0), f(x1), -f(x0), -f(x1)) - f(x)).abs()
                })
                .fold(0.0, f64::max)
        };
        let ratio = error(0.2) / error(0.1);
        assert!((12.0..20.0).contains(&ratio), "{ratio}");
    }

    /// A slope far too steep for the secant would overshoot; the limit keeps the curve between the
    /// knots' values, and a slope of the wrong sign is flattened.
    #[test]
    fn the_interpolant_never_leaves_monotone_data() {
        for (d0, d1) in [
            (-50.0, -0.1),
            (-0.1, -50.0),
            (0.5, -1.0),
            (-1.0, 0.5),
            (-8.0, -8.0),
        ] {
            let mut last = f64::INFINITY;
            for i in 0..=1_000 {
                let x = f64::from(i) / 1_000.0;
                let y = monotone_hermite(x, 0.0, 1.0, 1.0, 0.0, d0, d1);
                assert!(
                    y <= last && (0.0..=1.0).contains(&y),
                    "{d0}, {d1} at {x}: {y}"
                );
                last = y;
            }
        }
    }

    #[test]
    fn equal_values_give_a_constant() {
        for i in 0..=10 {
            let x = f64::from(i) / 10.0;
            assert_same_bits(monotone_hermite(x, 0.0, 1.0, 2.5, 2.5, 1.0, -1.0), 2.5);
        }
    }

    /// P14.T45.d: `at` is `at_offset` of `x − x0` to the bit, and an offset finer than one unit
    /// in the last place of an age of gigayears moves the value smoothly.
    #[test]
    fn the_offset_form_is_the_interpolant_and_resolves_fine_offsets() {
        let interval = Interval {
            x0: 1.502_68e9,
            x1: 1.502_68e9 + 2_000.0,
            y0: 1.3,
            y1: 1.25,
            d0: -2e-5,
            d1: -3e-5,
        };
        for k in 0..=20 {
            let x = interval.x0 + 100.0 * f64::from(k) + 0.123;
            assert_same_bits(interval.at(x), interval.at_offset(x - interval.x0));
        }
        // A tenth of a second apart, far below the 2.4 × 10⁻⁷ yr an age of 1.5 Gyr resolves.
        let tenth = 0.1 / 31_557_600.0;
        let mut last = interval.at_offset(500.0);
        for k in 1..=10 {
            let next = interval.at_offset(500.0 + tenth * f64::from(k));
            assert!(next < last, "{k}: {next} after {last}");
            last = next;
        }
    }
}
