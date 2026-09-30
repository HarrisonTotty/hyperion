//! Bound open clusters: how long they live, what they keep, and the marks of an old one (plan 09,
//! P09.T2.a and P09.T4.a).
//!
//! # Dissolution (Lamers et al. 2005)
//!
//! A bound cluster in the Galactic tidal field loses mass to disruption on a timescale that rises
//! with its mass as `M^γ`, γ = 0.62, and to stellar evolution (Lamers, Gieles, Bastian et al. 2005,
//! A&A 441, 117, hereafter L05). Its **total disruption time** is
//! [`dissolution_time`]`(m₀) = t₄ (m₀ ÷ 10⁴ M☉)^γ` with `t₄ = 1.3 Gyr`, L05's value for the solar
//! neighbourhood (their abstract and eq. 11, "the disruption time of a 10⁴ M☉ cluster … 1.3 ± 0.5
//! Gyr", the time at which the cluster is completely disrupted). With `dM ÷ dt = −M^(1−γ) ÷ t₀`
//! the mass bound against disruption alone falls as
//!
//! `s(m₀, t) = (1 − t ÷ t_dis(m₀))^(1 ÷ γ)` ([`disruption_survival`]),
//!
//! which reaches zero at `t_dis`. That factor is what a population's budget gives up to its
//! clusters: stellar evolution takes the same share of the field's mass, and a dead star is still a
//! system, so the share of the living budget in a cluster is `s` alone ([`surviving_mass_fraction`]).
//! A cluster's present mass multiplies in L05's stellar-evolution survival `μ_ev(t)` as well
//! ([`present_mass`]): L05's eq. 6 subtracts the two terms' `γ`-th powers, which differs from the
//! product by a few per cent late in a cluster's life, and the product is kept so that the
//! cluster's lifetime is `t_dis` exactly, the figure the brainstorm and the plan quote.
//!
//! `μ_ev = 1 − q_ev`, `log q_ev = (log t − a_ev)^b_ev + c_ev` (L05 eq. 2–3, t in years) with
//! L05 Table 1's solar row, `a_ev = 7.00`, `b_ev = 0.255`, `c_ev = −1.805`. L05 fit it above 12.5
//! Myr and call the loss before that negligible; below `10^a_ev` = 10 Myr, where the fit's base
//! vanishes, `q_ev` falls linearly from its value there, `10^c_ev`, to 0 at t = 0, so that the mass
//! is continuous (ours).
//!
//! # The mass function
//!
//! Nurseries, and so bound clusters, are born on `dN ÷ dM ∝ M⁻²` over 10²–10⁵ M☉ (Design note 1).
//! [`NurseryMassFunction`] holds it with the closed forms the rates need.

use crate::galaxy::consts::YEARS_PER_GIGAYEAR;
use crate::galaxy::fields::metallicity::FehDistribution;
use crate::galaxy::quad::gl_log_panels;
use crate::math;
use crate::units::{Dex, LightYears, SolarMasses, Years};

/// L05's total disruption time of a 10⁴ M☉ cluster in the solar neighbourhood, `t₄`: 1.3 Gyr
/// (L05 abstract and eq. 11). A parameter of the generator version.
pub const DISSOLUTION_T4: Years = Years::new(1.3 * YEARS_PER_GIGAYEAR);

/// The mass at which [`DISSOLUTION_T4`] applies: 10⁴ M☉.
pub const DISSOLUTION_REFERENCE_MASS: SolarMasses = SolarMasses::new(1e4);

/// L05's disruption exponent γ: 0.62 (their eq. 4 and 5, "for clusters in a tidal field").
pub const DISRUPTION_GAMMA: f64 = 0.62;

/// L05 Table 1, solar metallicity (Z = 0.02): `a_ev`, `b_ev`, `c_ev` of eq. 2.
const EVOLUTION_FIT: (f64, f64, f64) = (7.00, 0.255, -1.805);

/// The least initial mass of a nursery and so of a bound cluster, M☉ (Design note 1).
pub const NURSERY_MASS_MIN: SolarMasses = SolarMasses::new(1e2);

/// The greatest initial mass of a nursery, M☉ (Design note 1).
pub const NURSERY_MASS_MAX: SolarMasses = SolarMasses::new(1e5);

/// The total disruption time of a bound cluster of initial mass `m0`: `t₄ (m₀ ÷ 10⁴ M☉)^0.62`,
/// the age at which nothing of it is left bound (L05; module documentation).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::kinds::open_cluster::dissolution_time;
/// use hyperion_sim::units::SolarMasses;
///
/// // A 10⁴ M☉ cluster lasts 1.3 Gyr; a hundredth of that mass, about 75 Myr.
/// assert!((dissolution_time(SolarMasses::new(1e4)).value() - 1.3e9).abs() < 1.0);
/// let small = dissolution_time(SolarMasses::new(100.0)).value();
/// assert!((7.0e7..8.0e7).contains(&small), "{small}");
/// ```
#[must_use]
pub fn dissolution_time(m0: SolarMasses) -> Years {
    let ratio = m0.value() / DISSOLUTION_REFERENCE_MASS.value();
    Years::new(DISSOLUTION_T4.value() * math::powf(ratio, DISRUPTION_GAMMA))
}

/// The initial mass whose [`dissolution_time`] is `age`: the least mass a cluster can have been
/// born with to be alive at `age`.
#[must_use]
pub fn least_surviving_mass(age: Years) -> SolarMasses {
    let ratio = age.value().max(0.0) / DISSOLUTION_T4.value();
    SolarMasses::new(DISSOLUTION_REFERENCE_MASS.value() * math::powf(ratio, 1.0 / DISRUPTION_GAMMA))
}

/// The fraction of a cluster's initial mass still bound against disruption at `age`,
/// `(1 − age ÷ t_dis)^(1 ÷ γ)`, 1 at birth and 0 from [`dissolution_time`] on; 1 before birth.
#[must_use]
pub fn disruption_survival(m0: SolarMasses, age: Years) -> f64 {
    let a = age.value();
    if a <= 0.0 {
        return 1.0;
    }
    let left = 1.0 - a / dissolution_time(m0).value();
    if left <= 0.0 {
        0.0
    } else {
        math::powf(left, 1.0 / DISRUPTION_GAMMA)
    }
}

/// L05's stellar-evolution survival `μ_ev(age)`: the fraction of a cluster's initial mass left
/// after stellar evolution alone (module documentation), 1 at birth.
#[must_use]
pub fn evolution_survival(age: Years) -> f64 {
    let (a_ev, b_ev, c_ev) = EVOLUTION_FIT;
    let t = age.value();
    if t <= 0.0 {
        return 1.0;
    }
    let knee = math::exp10(a_ev);
    let q = if t <= knee {
        math::exp10(c_ev) * t / knee
    } else {
        math::exp10(math::powf(math::log10(t) - a_ev, b_ev) + c_ev)
    };
    1.0 - q
}

/// A bound cluster's present mass at `age`: `m₀ μ_ev(age) s(m₀, age)` (module documentation), 0
/// once it has dissolved.
#[must_use]
pub fn present_mass(m0: SolarMasses, age: Years) -> SolarMasses {
    SolarMasses::new(m0.value() * evolution_survival(age) * disruption_survival(m0, age))
}

/// `m_b(age)`: the mass-weighted mean of [`disruption_survival`] over the bound clusters born
/// `age` ago on [`NurseryMassFunction`] (Design note 2). It is the share of their stars that a
/// cluster still holds, and falls from 1 at birth to 0 once the heaviest has dissolved.
///
/// By a fixed quadrature in `ln m₀` (four 16-node panels) from the least surviving mass to the
/// heaviest: the integrand vanishes at the lower end as a power 1.6, which the quadrature takes to
/// better than 10⁻⁸.
#[must_use]
pub fn surviving_mass_fraction(age: Years) -> f64 {
    NurseryMassFunction::STANDARD.mass_weighted(age, |m0| disruption_survival(m0, age))
}

/// The number share of bound clusters born `age` ago that are still alive: those born above
/// [`least_surviving_mass`].
#[must_use]
pub fn surviving_number_fraction(age: Years) -> f64 {
    NurseryMassFunction::STANDARD.number_above(least_surviving_mass(age))
}

/// The nurseries' birth mass function, `dN ÷ dM ∝ M⁻²` on `[lo, hi]` (Design note 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NurseryMassFunction {
    lo: f64,
    hi: f64,
}

impl NurseryMassFunction {
    /// The plan's function, 10²–10⁵ M☉.
    pub const STANDARD: Self = Self {
        lo: NURSERY_MASS_MIN.value(),
        hi: NURSERY_MASS_MAX.value(),
    };

    /// The least mass.
    #[must_use]
    pub const fn lo(&self) -> SolarMasses {
        SolarMasses::new(self.lo)
    }

    /// The greatest mass.
    #[must_use]
    pub const fn hi(&self) -> SolarMasses {
        SolarMasses::new(self.hi)
    }

    /// The mean mass, `ln(hi ÷ lo) ÷ (1 ÷ lo − 1 ÷ hi)`: 691 M☉ for the standard function.
    #[must_use]
    pub fn mean(&self) -> SolarMasses {
        SolarMasses::new(math::ln(self.hi / self.lo) / (1.0 / self.lo - 1.0 / self.hi))
    }

    /// The share of nurseries born above `m`.
    #[must_use]
    pub fn number_above(&self, m: SolarMasses) -> f64 {
        let m = m.value();
        if m <= self.lo {
            1.0
        } else if m >= self.hi {
            0.0
        } else {
            (1.0 / m - 1.0 / self.hi) / (1.0 / self.lo - 1.0 / self.hi)
        }
    }

    /// The share of the nurseries' mass born above `m`: `ln(hi ÷ m) ÷ ln(hi ÷ lo)`.
    #[must_use]
    pub fn mass_above(&self, m: SolarMasses) -> f64 {
        let m = m.value();
        if m <= self.lo {
            1.0
        } else if m >= self.hi {
            0.0
        } else {
            math::ln(self.hi / m) / math::ln(self.hi / self.lo)
        }
    }

    /// The mass `u` of the way up the number distribution above `floor`: the inverse transform
    /// `1 ÷ m = 1 ÷ m₁ − u (1 ÷ m₁ − 1 ÷ hi)` with `m₁ = max(lo, floor)`, for `u` in [0, 1).
    #[must_use]
    pub fn quantile_above(&self, floor: SolarMasses, u: f64) -> SolarMasses {
        let lo = self.lo.max(floor.value()).min(self.hi);
        let inverse = 1.0 / lo - u * (1.0 / lo - 1.0 / self.hi);
        SolarMasses::new((1.0 / inverse).clamp(lo, self.hi))
    }

    /// The mass-weighted mean of `f(m₀)` over the part of the function above
    /// [`least_surviving_mass`]`(age)`, taken over the whole function's mass: `∫ f dm ÷ m ÷ ln(hi
    /// ÷ lo)` by four 16-node panels in `ln m`.
    #[must_use]
    pub fn mass_weighted(&self, age: Years, f: impl Fn(SolarMasses) -> f64) -> f64 {
        let lo = self.lo.max(least_surviving_mass(age).value());
        if lo >= self.hi {
            return 0.0;
        }
        let (a, b) = (math::ln(lo), math::ln(self.hi));
        let step = (b - a) / 4.0;
        let edges = [
            lo,
            math::exp(a + step),
            math::exp(a + 2.0 * step),
            math::exp(a + 3.0 * step),
            self.hi,
        ];
        // `gl_log_panels` integrates `g(m) dm` with the substitution `m = e^x`, so dividing by
        // `m` leaves `∫ f d(ln m)`.
        gl_log_panels(|m| f(SolarMasses::new(m)) / m, &edges) / math::ln(self.hi / self.lo)
    }
}

/// The marks of an old open cluster: one of the bound clusters older than the young disc, drawn on
/// a process per sub-disc of the old thin disc (P09.T4.a; Design note 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenClusterMarks {
    age_at_epoch: Years,
    initial_mass: SolarMasses,
    half_mass_radius: LightYears,
    concentration: f64,
    fe_h: Dex,
}

/// The median half-mass radius of a 10⁴ M☉ open cluster, 11.08 ly: Brown and Gnedin's (2021,
/// MNRAS 508, 5935, Table 2) full LEGUS fit of the projected radius, 2.548 pc × (M ÷ 10⁴ M☉)^0.242,
/// times 4/3, the half-mass radius over the projected half-light radius (Spitzer 1987, *Dynamical
/// Evolution of Globular Clusters*, §1.2; 1.305 for a Plummer sphere) (ruling 126.6). The fit is
/// to the masses of clusters younger than about 1 Gyr as seen today, over about 10^2.6–10^5.4 M☉;
/// we read it at the initial mass, and below 10^2.6 M☉ it is extrapolated.
pub const HALF_MASS_RADIUS_AT_1E4: LightYears = LightYears::new(11.08);

/// The half-mass radius's slope with mass, 0.242 (Brown and Gnedin 2021).
pub const HALF_MASS_RADIUS_SLOPE: f64 = 0.242;

/// The half-mass radius's log-normal scatter, dex: Brown and Gnedin 2021's, Table 2 (ruling 126.6).
pub const HALF_MASS_RADIUS_SIGMA_DEX: f64 = 0.25;

/// An open cluster's half-mass radius at initial mass `m0` and standard normal `n`:
/// `11.08 ly × (m0 ÷ 10⁴ M☉)^0.242 × 10^(0.25 n)` (P09.T4.a; ruling 126.6).
#[must_use]
pub fn half_mass_radius(m0: SolarMasses, n: f64) -> LightYears {
    LightYears::new(
        HALF_MASS_RADIUS_AT_1E4.value()
            * math::powf(m0.value() / 1e4, HALF_MASS_RADIUS_SLOPE)
            * math::exp10(HALF_MASS_RADIUS_SIGMA_DEX * n),
    )
}

/// The King concentration `c = log₁₀(r_t ÷ r_c)` of an open cluster is drawn uniform on this
/// range. Provisional: our round figures for the range open clusters show (Piskunov et al. 2008,
/// A&A 477, 165, fit King profiles to Galactic open clusters, not read here).
pub const CONCENTRATION_RANGE: (f64, f64) = (0.5, 1.5);

impl OpenClusterMarks {
    /// Marks from their parts, as the catalogue draws them.
    #[must_use]
    pub(crate) fn new(
        age_at_epoch: Years,
        initial_mass: SolarMasses,
        (half_mass_radius, concentration): (LightYears, f64),
        metallicity: &FehDistribution,
        metallicity_normal: f64,
    ) -> Self {
        Self {
            age_at_epoch,
            initial_mass,
            half_mass_radius,
            concentration,
            fe_h: Dex::new(
                metallicity.mean().value() + metallicity.sigma().value() * metallicity_normal,
            ),
        }
    }

    /// The cluster's age at the epoch.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// Its initial mass, conditional on its being alive at the epoch.
    #[must_use]
    pub const fn initial_mass(&self) -> SolarMasses {
        self.initial_mass
    }

    /// Its present mass at `t` years after the epoch ([`present_mass`]).
    #[must_use]
    pub fn present_mass(&self, t: Years) -> SolarMasses {
        present_mass(self.initial_mass, self.age_at_epoch + t)
    }

    /// Its total disruption time ([`dissolution_time`]).
    #[must_use]
    pub fn dissolution_time(&self) -> Years {
        dissolution_time(self.initial_mass)
    }

    /// Its half-mass radius, light-years.
    #[must_use]
    pub const fn half_mass_radius(&self) -> LightYears {
        self.half_mass_radius
    }

    /// Its King concentration `log₁₀(r_t ÷ r_c)`.
    #[must_use]
    pub const fn concentration(&self) -> f64 {
        self.concentration
    }

    /// Its [Fe/H], drawn once from the population's law at its position and age: a cluster is
    /// coeval and of one metallicity.
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::consts::YEARS_PER_MEGAYEAR;

    fn myr(value: f64) -> Years {
        Years::new(value * YEARS_PER_MEGAYEAR)
    }

    #[test]
    fn survival_falls_from_one_to_zero_at_the_dissolution_time() {
        let m = SolarMasses::new(3e3);
        let t = dissolution_time(m);
        assert!((disruption_survival(m, Years::ZERO) - 1.0).abs() < 1e-15);
        assert!(disruption_survival(m, t * 0.5) > 0.0);
        assert!(disruption_survival(m, t).abs() < 1e-15);
        assert!(disruption_survival(m, t * 1.01).abs() < 1e-15);
        let back = dissolution_time(least_surviving_mass(t));
        assert!((back.value() / t.value() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn stellar_evolution_follows_lamers_table_1_and_is_continuous_at_the_bridge() {
        // L05 eq. 2 at 1 Gyr with the solar row: log q = 2^0.255 − 1.805.
        let expected = 1.0 - math::exp10(math::powf(2.0, 0.255) - 1.805);
        assert!((evolution_survival(Years::new(1e9)) - expected).abs() < 1e-12);
        let knee = Years::new(1e7);
        let below = evolution_survival(Years::new(1e7 * (1.0 - 1e-9)));
        assert!((below - evolution_survival(knee)).abs() < 1e-6);
        assert!((evolution_survival(Years::ZERO) - 1.0).abs() < 1e-15);
        // Monotone falling.
        let mut last = 1.0;
        for k in 1..400 {
            let s = evolution_survival(Years::new(1e5 * math::powf(1.03, f64::from(k))));
            assert!(s <= last, "{k}");
            last = s;
        }
    }

    #[test]
    fn the_mass_function_mean_and_shares_are_closed_forms() {
        let f = NurseryMassFunction::STANDARD;
        assert!((f.mean().value() - 691.5).abs() < 1.0, "{:?}", f.mean());
        assert!(
            (f.number_above(SolarMasses::new(1e3)) - (1e-3 - 1e-5) / (1e-2 - 1e-5)).abs() < 1e-15
        );
        assert!((f.mass_above(SolarMasses::new(1e3)) - 2.0 / 3.0).abs() < 1e-12);
        let q = f.quantile_above(SolarMasses::new(1e2), 0.0);
        assert!((q.value() - 1e2).abs() < 1e-9);
    }

    #[test]
    fn surviving_mass_fraction_matches_a_fine_midpoint_sum() {
        for age in [myr(5.0), myr(100.0), myr(700.0), myr(3_000.0)] {
            let n = 200_000;
            let (a, b) = (math::ln(1e2), math::ln(1e5));
            let mut sum = 0.0;
            for i in 0..n {
                let x = a + (b - a) * (f64::from(i) + 0.5) / f64::from(n);
                sum += disruption_survival(SolarMasses::new(math::exp(x)), age);
            }
            let midpoint = sum / f64::from(n);
            let quad = surviving_mass_fraction(age);
            assert!(
                (quad - midpoint).abs() < 1e-7,
                "{age:?}: {quad} vs {midpoint}"
            );
        }
        assert!(surviving_mass_fraction(myr(9_000.0)).abs() < 1e-15);
    }
}
