//! The displaced classes' conditional marks (plan 08, P08.T9.d and P08.T12.c): kind, initial mass,
//! birth component and time since death, drawn from per-class tables, and the lifetime bracket of
//! layer D.
//!
//! P08.T1 lays out one helper the marks draw with, `age_between`: an age on an interval of a
//! component's age distribution, by its closed-form CDF and quantile, so that nothing is added to
//! the distribution. The tables themselves are P08.T9.d's.

use crate::galaxy::ages::AgeDistribution;
use crate::units::Years;

/// The age on `[a, b]` of the distribution `ages` at the rank `u` in `[0, 1]`: `quantile(cdf(a) +
/// u (cdf(b) − cdf(a)))`, the distribution restricted to the interval (plan 08, Consumes).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "P08.T12.c's marks draw ages with it")
)]
pub(crate) fn age_between(ages: &AgeDistribution, a: Years, b: Years, u: f64) -> Years {
    let (lo, hi) = (ages.cdf(a), ages.cdf(b));
    ages.quantile(lo + u * (hi - lo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;

    /// P08.T1: for every component of the Milky Way fixture, 1,000 ranks on intervals inside and
    /// across its pieces land inside the interval, at the CDF the rank asks for, to 10⁻⁹.
    #[test]
    fn age_cdf_inverts_on_an_interval() {
        let galaxy = Galaxy::from_params(Seed::new(8), GalaxyParams::milky_way_like())
            .expect("the fixture's gas is mostly neutral");
        for component in galaxy.fields().components() {
            let ages = component.ages();
            let (min, max) = (ages.min().value(), ages.max().value());
            for (fa, fb) in [(0.0, 1.0), (0.1, 0.4), (0.35, 0.95), (0.02, 0.03)] {
                let a = Years::new(min + fa * (max - min));
                let b = Years::new(min + fb * (max - min));
                let (lo, hi) = (ages.cdf(a), ages.cdf(b));
                if hi - lo < 1e-9 {
                    continue;
                }
                for k in 0..1_000 {
                    let u = (f64::from(k) + 0.5) / 1_000.0;
                    let age = age_between(ages, a, b, u);
                    let span = (b.value() - a.value()).abs();
                    assert!(
                        age.value() >= a.value() - 1e-9 * span
                            && age.value() <= b.value() + 1e-9 * span,
                        "{:?}: {age:?} outside [{a:?}, {b:?}]",
                        component.population()
                    );
                    let target = lo + u * (hi - lo);
                    let got = ages.cdf(age);
                    assert!(
                        (got - target).abs() <= 1e-9 * target.max(1e-3),
                        "{:?}: cdf {got} against {target}",
                        component.population()
                    );
                }
            }
        }
    }
}
