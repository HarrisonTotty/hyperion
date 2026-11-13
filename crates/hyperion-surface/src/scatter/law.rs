//! The rock size–frequency law: Golombek and Rapp 1997's exponential model of the area rocks cover,
//! and the number of rocks it gives (plan R11, Design note 4).
//!
//! # The model
//!
//! At a site of rock abundance k, the fraction of the ground covered by rocks of diameter D or
//! larger is
//!
//! > F_k(D) = k exp(−q(k) D), q(k) = 1.79 + 0.152 ÷ k per metre
//!
//! (Golombek and Rapp 1997, JGR 102(E2), 4117, eqs. 1–2, as Golombek et al. 2012, _Mars_ 7, 1–22,
//! quote them), k being the fraction covered by all rocks. A rock of diameter D covers π D² ÷ 4,
//! so the number of rocks per square metre between D and D + dD is n(D) dD = −F′(D) ÷ (π D² ÷ 4)
//! dD = (4 k q ÷ π) e^(−qD) ÷ D² dD, and the cumulative number is its integral from D up:
//!
//! > N(>D) = (4 k q ÷ π) [e^(−qD) ÷ D − q E₁(qD)]
//!
//! with E₁ the exponential integral ([`exponential_integral_e1`]). It reproduces Golombek et al.
//! 2012's chance of meeting a rock over 1.1 m in 4 m², 0.078% at k = 5% and 0.75% at 10%.
//!
//! k is clamped to [0, [`MAX_ROCK_ABUNDANCE`]], the range the model family was fitted over.
//!
//! # Arithmetic
//!
//! Everything is `f64`, every transcendental goes through `hyperion_base::math`, and E₁ is a
//! series below 1 and a continued fraction from 1 up, each summed in a fixed order to a fixed
//! stop, so that the law gives the same bits on every target. N(>D) is taken as
//! (4kq ÷ π) e^(−x) (1 ÷ D − q e^x E₁(x)) at x = qD, the continued fraction giving e^x E₁(x)
//! without the exponential, so that neither term underflows before the other.

use hyperion_base::math;
use hyperion_base::units::Metres;

use crate::num;

/// The largest rock abundance the law is used at: 0.40, the top of the range Golombek and Rapp
/// 1997's model family spans (Design note 4).
pub const MAX_ROCK_ABUNDANCE: f64 = 0.40;

/// q(k)'s constant term, per metre (Golombek and Rapp 1997, eq. 2).
const Q_CONSTANT_PER_M: f64 = 1.79;

/// q(k)'s coefficient of 1 ÷ k, per metre (Golombek and Rapp 1997, eq. 2).
const Q_INVERSE_PER_M: f64 = 0.152;

/// The most terms [`exponential_integral_e1`]'s series sums: below x = 1 its terms fall faster
/// than xⁿ ÷ n!, under 10⁻¹⁷ of the sum by the 20th.
const SERIES_TERMS: u32 = 64;

/// The most steps [`scaled_e1`]'s continued fraction takes: at x = 1, its slowest, it meets
/// [`CF_TOLERANCE`] in under 200.
const CF_STEPS: u32 = 1_000;

/// The step below which the continued fraction stops: a few ulps of 1.
const CF_TOLERANCE: f64 = 4.0 * f64::EPSILON;

/// The most steps [`RockAbundance::diameter_at`] takes: Newton's method from the bracket's mean
/// meets its tolerance in a handful, and the bisection that guards it halves the bracket each
/// time it is taken.
const INVERSION_STEPS: u32 = 64;

/// The exponential integral E₁(x) = ∫ₓ^∞ e^(−t) ÷ t dt, for `x` positive.
///
/// Below 1 it is the series −γ − ln x − Σ (−x)ⁿ ÷ (n n!), from n = 1; from 1 up, e^(−x) times the
/// continued fraction 1 ÷ (x + 1 − 1² ÷ (x + 3 − 2² ÷ (x + 5 − …))) by the modified Lentz method
/// (Press et al., _Numerical Recipes_, 3rd ed., §6.3, `expint` for n = 1). Both are held to
/// tabulated values (Abramowitz and Stegun 1964, Table 5.1) to 10⁻¹² relative over x from 0.5 to
/// 700, past every argument the law takes. It underflows to 0 above about 745.
///
/// # Panics
///
/// If `x` is not positive, or is a NaN.
#[must_use]
pub fn exponential_integral_e1(x: f64) -> f64 {
    assert!(x > 0.0, "E1 needs a positive argument, got {x}");
    if x < 1.0 {
        e1_series(x)
    } else {
        math::exp(-x) * scaled_e1(x)
    }
}

/// E₁(x) by its series, for x in (0, 1).
#[must_use]
fn e1_series(x: f64) -> f64 {
    let mut sum = 0.0;
    // Term n is (−1)^(n+1) xⁿ ÷ (n n!); `power` holds (−1)^(n+1) xⁿ ÷ n!.
    let mut power = 1.0;
    for n in 1..=SERIES_TERMS {
        let n = f64::from(n);
        power *= -x / n;
        let term = -power / n;
        sum += term;
        if term.abs() <= f64::EPSILON * sum.abs() {
            break;
        }
    }
    -core::f64::consts::EULER_GAMMA - math::ln(x) + sum
}

/// e^x E₁(x) by its continued fraction, for x of at least 1: the modified Lentz method of
/// _Numerical Recipes_' `expint`.
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "Numerical Recipes' names of the Lentz method's terms, b, c, d and h"
)]
fn scaled_e1(x: f64) -> f64 {
    // The smallest denominator the method lets stand: a tiny value in place of zero.
    const TINY: f64 = 1e-300;
    let mut b = x + 1.0;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..CF_STEPS {
        let a = -f64::from(i) * f64::from(i);
        b += 2.0;
        d = 1.0 / (a * d + b);
        c = b + a / c;
        let step = c * d;
        h *= step;
        if (step - 1.0).abs() <= CF_TOLERANCE {
            break;
        }
    }
    h
}

/// e^x E₁(x), for any positive x: the continued fraction from 1 up, the series' value times e^x
/// below.
#[must_use]
fn scaled_e1_any(x: f64) -> f64 {
    if x < 1.0 {
        math::exp(x) * e1_series(x)
    } else {
        scaled_e1(x)
    }
}

/// Golombek and Rapp 1997's rock abundance k at a site: the fraction of the ground that rocks of
/// every size cover, clamped to [0, [`MAX_ROCK_ABUNDANCE`]] (see the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct RockAbundance {
    k: f64,
}

impl RockAbundance {
    /// No rocks: k = 0.
    pub const NONE: Self = Self { k: 0.0 };

    /// The abundance `k`, clamped to [0, [`MAX_ROCK_ABUNDANCE`]]; a −0 is taken as +0, so that no
    /// abundance carries a sign bit.
    ///
    /// # Panics
    ///
    /// If `k` is a NaN.
    #[must_use]
    pub fn new(k: f64) -> Self {
        assert!(!k.is_nan(), "a rock abundance must not be a NaN");
        let k = num::min(num::max(k, 0.0), MAX_ROCK_ABUNDANCE);
        Self { k }
    }

    /// k, the fraction of the ground covered by rocks, 0 to [`MAX_ROCK_ABUNDANCE`].
    #[must_use]
    pub const fn k(&self) -> f64 {
        self.k
    }

    /// Whether the site has no rocks, k = 0.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.k <= 0.0
    }

    /// q(k) = 1.79 + 0.152 ÷ k, per metre: the rate at which the area covered falls with diameter.
    /// Infinite at k = 0.
    #[must_use]
    pub fn q(&self) -> f64 {
        Q_CONSTANT_PER_M + Q_INVERSE_PER_M / self.k
    }

    /// k q, per metre, as 1.79 k + 0.152, which stays finite at k = 0.
    #[must_use]
    fn kq(self) -> f64 {
        Q_CONSTANT_PER_M * self.k + Q_INVERSE_PER_M
    }

    /// `F_k(D)` = k exp(−qD): the fraction of the ground covered by rocks of `diameter` or larger.
    ///
    /// # Panics
    ///
    /// If `diameter` is not positive and finite.
    #[must_use]
    pub fn cumulative_area(&self, diameter: Metres) -> f64 {
        let d = assert_diameter(diameter);
        if self.is_none() {
            return 0.0;
        }
        self.k * math::exp(-self.q() * d)
    }

    /// N(>D), the number of rocks of `diameter` or larger per square metre, in closed form (see
    /// the [module documentation](self)); 0 at k = 0, and never negative.
    ///
    /// # Panics
    ///
    /// If `diameter` is not positive and finite.
    #[must_use]
    pub fn cumulative_number_per_m2(&self, diameter: Metres) -> f64 {
        let d = assert_diameter(diameter);
        if self.is_none() {
            return 0.0;
        }
        let q = self.q();
        let x = q * d;
        let bracket = 1.0 / d - q * scaled_e1_any(x);
        num::max(
            4.0 * self.kq() / core::f64::consts::PI * math::exp(-x) * bracket,
            0.0,
        )
    }

    /// n(D), the number of rocks per square metre per metre of diameter at `diameter`:
    /// (4 k q ÷ π) e^(−qD) ÷ D², the slope of N(>D) with its sign turned.
    ///
    /// # Panics
    ///
    /// If `diameter` is not positive and finite.
    #[must_use]
    pub fn number_density(&self, diameter: Metres) -> f64 {
        let d = assert_diameter(diameter);
        if self.is_none() {
            return 0.0;
        }
        4.0 * self.kq() / core::f64::consts::PI * math::exp(-self.q() * d) / (d * d)
    }

    /// The number of rocks per square metre with diameters from `low` up to `high`:
    /// N(>`low`) − N(>`high`), never negative.
    ///
    /// # Panics
    ///
    /// If either diameter is not positive and finite, or `high` is below `low`.
    #[must_use]
    pub fn number_between_per_m2(&self, low: Metres, high: Metres) -> f64 {
        assert!(
            high >= low,
            "a diameter range must not be reversed: {low:?} to {high:?}"
        );
        num::max(
            self.cumulative_number_per_m2(low) - self.cumulative_number_per_m2(high),
            0.0,
        )
    }

    /// The diameter in [`low`, `high`] below which the share `u` of the rocks between them lies:
    /// the law inverted within the range, the D with N(>`low`) − N(>D) = `u` (N(>`low`) −
    /// N(>`high`)). Newton's method on N, from the range's mean, guarded by bisection of the
    /// bracket, to a step of 10⁻¹⁴ of the diameter, a few nanometres. `low` itself at k = 0, where
    /// the range holds no rocks.
    ///
    /// # Panics
    ///
    /// If either diameter is not positive and finite, `high` is below `low`, or `u` is not in
    /// [0, 1).
    #[must_use]
    pub fn diameter_at(&self, low: Metres, high: Metres, u: f64) -> Metres {
        assert!(
            (0.0..1.0).contains(&u),
            "a share of the rocks must lie in [0, 1), got {u}"
        );
        let between = self.number_between_per_m2(low, high);
        if self.is_none() || between <= 0.0 {
            return low;
        }
        let target = self.cumulative_number_per_m2(low) - u * between;
        let (mut lo, mut hi) = (low.value(), high.value());
        let mut d = f64::midpoint(lo, hi);
        for _ in 0..INVERSION_STEPS {
            // N falls with D, so a count above the target is a diameter too small.
            let excess = self.cumulative_number_per_m2(Metres::new(d)) - target;
            if excess > 0.0 {
                lo = d;
            } else {
                hi = d;
            }
            let slope = self.number_density(Metres::new(d));
            let newton = d + excess / slope;
            let next = if newton > lo && newton < hi && newton.is_finite() {
                newton
            } else {
                f64::midpoint(lo, hi)
            };
            let step = (next - d).abs();
            d = next;
            if step <= 1e-14 * d {
                break;
            }
        }
        Metres::new(num::min(num::max(d, low.value()), high.value()))
    }
}

/// `diameter`'s metres, once it is known to be positive and finite.
#[must_use]
#[track_caller]
fn assert_diameter(diameter: Metres) -> f64 {
    let d = diameter.value();
    assert!(
        d.is_finite() && d > 0.0,
        "a rock diameter must be positive and finite, got {d}"
    );
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// E₁ at tabulated points: Abramowitz and Stegun 1964, Table 5.1 (x = 0.5, 1, 2, 5 and 10, to
    /// their 15 figures), the rest computed with mpmath's `e1` at 50 digits (2026-11-02) and
    /// rounded to `f64`. The range covers both branches, the switch at 1 and the law's
    /// arguments, qD from 0.87 (k = 0.40, D = 0.4 m) to several hundred.
    const E1_TABLE: [(f64, f64); 17] = [
        (0.5, 0.559_773_594_776_160_8),
        (0.8, 0.310_596_578_545_543),
        (0.868, 0.275_142_481_740_288_16),
        (0.95, 0.238_737_523_653_734_66),
        (0.999, 0.219_752_182_022_944_55),
        (1.0, 0.219_383_934_395_520_29),
        (1.001, 0.219_016_422_527_468_86),
        (1.5, 0.100_019_582_406_632_65),
        (2.0, 0.048_900_510_708_061_12),
        (3.0, 0.013_048_381_094_197_037),
        (5.0, 0.001_148_295_591_275_325_7),
        (10.0, 4.156_968_929_685_325e-6),
        (20.0, 9.835_525_290_649_882e-11),
        (50.0, 3.783_264_029_550_459e-24),
        (100.0, 3.683_597_761_682_032e-46),
        (300.0, 1.710_384_276_804_51e-133),
        (700.0, 1.406_518_766_234_033e-307),
    ];

    #[test]
    fn e1_meets_its_table_to_a_part_in_a_million_million() {
        for (x, expected) in E1_TABLE {
            let got = exponential_integral_e1(x);
            let relative = (got - expected).abs() / expected;
            assert!(
                relative <= 1e-12,
                "E1({x}) = {got:e}, tabulated {expected:e}: {relative:e} relative"
            );
        }
    }

    /// The two branches agree at the switch, 1, so that the law has no step there.
    #[test]
    fn e1_is_continuous_across_its_branches() {
        let series = e1_series(1.0);
        let fraction = math::exp(-1.0) * scaled_e1(1.0);
        assert!(
            (series - fraction).abs() <= 1e-13 * fraction,
            "{series} and {fraction}"
        );
    }

    /// Golombek et al. 2012's oracle: the chance of meeting a rock over 1.1 m in 4 m² is 0.078% at
    /// k = 5% and 0.75% at 10%.
    #[test]
    fn the_law_meets_golombek_2012s_chance_of_a_rock_over_one_metre() {
        for (k, expected) in [(0.05, 0.000_78), (0.10, 0.007_5)] {
            let n = RockAbundance::new(k).cumulative_number_per_m2(Metres::new(1.1)) * 4.0;
            let chance = 1.0 - math::exp(-n);
            assert!(
                (chance - expected).abs() <= 0.01 * expected,
                "k = {k}: {chance} against {expected}"
            );
        }
    }

    /// N(>0.4 m) over 1,024 m², a 32 m square, at the plan's six abundances (Design note 4), to
    /// 1%; k = 0.069 is the Viking 1 lander's.
    #[test]
    fn the_authoritative_rocks_of_a_32_m_square_follow_the_law() {
        for (k, expected) in [
            (0.02, 2.6),
            (0.05, 32.0),
            (0.069, 58.0),
            (0.10, 101.0),
            (0.18, 212.0),
            (0.30, 377.0),
        ] {
            let n = RockAbundance::new(k).cumulative_number_per_m2(Metres::new(0.4)) * 1_024.0;
            assert!(
                (n - expected).abs() <= 0.01 * expected,
                "k = {k}: {n} rocks against {expected}"
            );
        }
    }

    /// N(>D) is the integral of n(D) from D up: its difference over a short interval matches the
    /// density's Simpson rule there.
    #[test]
    fn the_number_density_is_the_slope_of_the_cumulative_number() {
        let law = RockAbundance::new(0.18);
        for d in [0.4, 0.8, 1.6, 3.2] {
            let (a, b) = (d, d * 1.001);
            let difference = law.number_between_per_m2(Metres::new(a), Metres::new(b));
            let mid = f64::midpoint(a, b);
            let simpson = (b - a) / 6.0
                * (law.number_density(Metres::new(a))
                    + 4.0 * law.number_density(Metres::new(mid))
                    + law.number_density(Metres::new(b)));
            assert!(
                (difference - simpson).abs() <= 1e-9 * simpson,
                "at {d} m: {difference} against {simpson}"
            );
        }
    }

    /// The area law is k at no diameter and falls with D; nothing is rocky at k = 0; k clamps to
    /// [0, 0.40]; and the cumulative number falls with D.
    #[test]
    fn the_law_is_bounded_and_falls_with_diameter() {
        assert_eq!(RockAbundance::new(0.55).k(), MAX_ROCK_ABUNDANCE);
        assert_eq!(RockAbundance::new(-0.1).k(), 0.0);
        assert!(RockAbundance::new(-0.0).k().is_sign_positive());
        let none = RockAbundance::NONE;
        assert_eq!(none.cumulative_number_per_m2(Metres::new(0.4)), 0.0);
        assert_eq!(none.cumulative_area(Metres::new(0.4)), 0.0);
        let law = RockAbundance::new(0.1);
        let tiny = law.cumulative_area(Metres::new(1e-9));
        assert!((tiny - 0.1).abs() < 1e-8, "{tiny}");
        let mut last = f64::INFINITY;
        for d in [0.4, 0.8, 1.6, 3.2, 6.4, 12.8] {
            let n = law.cumulative_number_per_m2(Metres::new(d));
            assert!(n >= 0.0 && n < last, "N(>{d}) = {n} after {last}");
            last = n;
        }
        // A tiny abundance has a huge q, and its rocks are none, not a NaN.
        let sparse = RockAbundance::new(1e-6);
        assert_eq!(sparse.cumulative_number_per_m2(Metres::new(0.4)), 0.0);
    }

    /// The inversion takes a share back to its diameter: N(>low) − N(>D) is the share of the range's
    /// count, and the ends map to the range's ends.
    #[test]
    fn the_diameter_inversion_returns_the_share_it_was_given() {
        for k in [0.02, 0.069, 0.18, 0.40] {
            let law = RockAbundance::new(k);
            for (low, high) in [(0.4, 0.8), (0.8, 1.6), (1.6, 3.2), (3.2, 6.4)] {
                let (low, high) = (Metres::new(low), Metres::new(high));
                let between = law.number_between_per_m2(low, high);
                let first = law.diameter_at(low, high, 0.0);
                assert!(first >= low && first.value() - low.value() <= 1e-14 * low.value());
                for u in [0.001, 0.25, 0.5, 0.75, 0.999] {
                    let d = law.diameter_at(low, high, u);
                    assert!(d >= low && d <= high, "{d:?} outside {low:?} to {high:?}");
                    let share = law.number_between_per_m2(low, d) / between;
                    assert!(
                        (share - u).abs() <= 1e-9,
                        "k = {k}, {low:?}: share {share} against {u}"
                    );
                }
            }
        }
        let none = RockAbundance::NONE;
        assert_eq!(
            none.diameter_at(Metres::new(0.4), Metres::new(0.8), 0.5),
            Metres::new(0.4)
        );
    }
}
