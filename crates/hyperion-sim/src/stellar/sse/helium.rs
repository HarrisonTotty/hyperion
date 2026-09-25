//! Naked helium stars (plan 06, P06.T9; Hurley, Pols and Tout 2000, MNRAS 315, 543, "HPT",
//! section 6.1).
//!
//! HPT fit Pols's helium-star models (X = 0, Y = 0.98, Z = 0.02, from 0.32 to 10 M☉) with formulae
//! that do not depend on metallicity. The zero-age luminosity and radius and the helium
//! main-sequence lifetime are also what the zero-age horizontal branch of core helium burning
//! meets as the envelope vanishes (HPT section 5.3, equations 53, 54 and 57).
//!
//! [`HeliumStar`] follows one from its zero-age helium main sequence through the helium
//! Hertzsprung gap and giant branch. [`HeliumStar::new`] builds one at zero age, for plan 11's
//! stripped stars and for a star whose envelope goes in the Hertzsprung gap or on the giant branch;
//! `HeliumStar::from_core_helium_burning` and [`HeliumStar::from_early_agb`] take over a star
//! whose envelope a wind removes during core helium burning or on the early AGB, the Wolf-Rayet
//! route (HPT section 6).
//!
//! Units are HPT's, in unit newtypes: M☉, L☉, R☉ and Myr, the helium star's age counted from its
//! zero-age main sequence. Equation numbers are the journal's, which for this section are the
//! arXiv preprint's too.
//!
//! The comparison tests use the SSE build of [`sse`](super), run again on 2026-09-23 for these
//! phases: `hrdiag` at constant mass on finer age grids, for helium stars from 0.3 to 50 M☉, and at
//! the hand-over to a helium star, and a second time with the branch that applies the
//! small-envelope perturbation of HPT section 6.3 (equations 97–100) disabled, for rows where SSE
//! applies it; each test says which run it reads.

use crate::math;
use crate::stellar::Phase;
use crate::units::{Megayears, SolarLuminosities, SolarMasses, SolarRadii};

use super::PhasePoint;
use super::agb::{self, CHANDRASEKHAR_MSUN, CoreEnd, EarlyAgb};
#[cfg(test)]
use super::cheb::CoreHeliumBurning;
use super::gb::{GiantBranch, GiantTimes};

/// The zero-age luminosity of a helium star of mass `m` (HPT equation 77):
/// 15,262 M^10.25 ÷ (M⁹ + 29.54 M^7.5 + 31.18 M⁶ + 0.0469).
#[must_use]
pub(crate) fn zams_luminosity(m: SolarMasses) -> SolarLuminosities {
    let m = m.value();
    SolarLuminosities::new(
        15_262.0 * math::powf_positive(m, 10.25)
            / (math::powi(m, 9)
                + 29.54 * math::powf_positive(m, 7.5)
                + 31.18 * math::powi(m, 6)
                + 0.0469),
    )
}

/// The zero-age radius of a helium star of mass `m` (HPT equation 78):
/// 0.2391 M^4.6 ÷ (M⁴ + 0.162 M³ + 0.0065).
#[must_use]
pub(crate) fn zams_radius(m: SolarMasses) -> SolarRadii {
    let m = m.value();
    SolarRadii::new(
        0.2391 * math::powf_positive(m, 4.6)
            / (math::powi(m, 4) + 0.162 * math::powi(m, 3) + 0.0065),
    )
}

/// The central helium-burning lifetime of a helium star of mass `m`, its helium main sequence
/// (HPT equation 79): (0.4129 + 18.81 M⁴ + 1.853 M⁶) ÷ M^6.5 Myr.
#[must_use]
pub(crate) fn main_sequence_lifetime(m: SolarMasses) -> Megayears {
    let m = m.value();
    Megayears::new(
        (0.4129 + 18.81 * math::powi(m, 4) + 1.853 * math::powi(m, 6))
            / math::powf_positive(m, 6.5),
    )
}

/// The luminosity and radius of a helium main-sequence star of mass `m` at fractional age `tau`
/// (HPT equations 77, 78 and 80–83): what [`HeliumStar::at`] gives on the main sequence, without
/// building the rest of the star. Core helium burning's remnant (HPT section 6.3) reads it.
#[must_use]
pub(crate) fn main_sequence_point(m: SolarMasses, tau: f64) -> (SolarLuminosities, SolarRadii) {
    let mass = m.value();
    let alpha = (0.85 - 0.08 * mass).max(0.0);
    let beta = (0.4 - 0.22 * math::log10(mass)).max(0.0);
    (
        zams_luminosity(m) * (1.0 + 0.45 * tau + alpha * tau * tau),
        zams_radius(m) * (1.0 + beta * (tau - math::powi(tau, 6))),
    )
}

/// [`HeliumStar::new`]`(m)`[`.at`](HeliumStar::at)`(t_HeMS × x)` and `t_HeMS`, bit for bit, building
/// only the main sequence where the age falls on it (x < 1): the helium main sequence evaluates its
/// star at the current mass (HPT section 7.1), so a track rebuilds it at every step of the phase,
/// and the rest of the star was most of that cost.
#[must_use]
pub(crate) fn main_sequence_at_fraction(m: SolarMasses, x: f64) -> (PhasePoint, Megayears) {
    let mass = m.value();
    let t_ms = main_sequence_lifetime(m);
    let t = t_ms * x;
    if t < t_ms {
        debug_assert!(
            t.value() >= -1e-9 * t_ms.value(),
            "a helium star lives from 0 to its end, not {t:?}"
        );
        // `HeliumStar::new`'s terms and `HeliumStar::point`'s main sequence, in their order.
        let l_zams = zams_luminosity(m);
        let alpha = (0.85 - 0.08 * mass).max(0.0);
        let r_zams = zams_radius(m);
        let beta = (0.4 - 0.22 * math::log10(mass)).max(0.0);
        let tau = t / t_ms;
        let point = PhasePoint {
            luminosity: l_zams * (1.0 + 0.45 * tau + alpha * tau * tau),
            radius: r_zams * (1.0 + beta * (tau - math::powi(tau, 6))),
            core_mass: SolarMasses::ZERO,
        };
        return (point, t_ms);
    }
    (HeliumStar::new(m).at(t), t_ms)
}

/// A naked helium star of one mass, from its helium zero-age main sequence through the helium
/// Hertzsprung gap and giant branch until its carbon–oxygen core reaches its limit (HPT section
/// 6.1).
///
/// On the helium main sequence, τ = t ÷ `t_HeMS` (equation 79), L = `L_ZHe` (1 + 0.45τ + ατ²) and
/// R = `R_ZHe` (1 + β(τ − τ⁶)) with α = max(0, 0.85 − 0.08 M) and β = max(0, 0.4 − 0.22 log M)
/// (80–83), and HPT define no core. After it the carbon–oxygen core grows along L = min(B Mc³,
/// D Mc⁵) with B = 4.1 × 10⁴ and D = 5.5 × 10⁴ ÷ (1 + 0.4 M⁴) (84), with `A_He` for the rate,
/// `t_HeMS` for `t_BGB` and `L_THe`, the luminosity at the end of the main sequence, for `L_BGB`.
/// The radius is min(R₁, R₂) (85): R₁ = `R_ZHe` (L ÷ `L_THe`)^0.2 + 0.02 (e^(L/λ) − e^(`L_THe`/λ))
/// with λ = 500 (2 + M⁵) ÷ M^2.5 (86, 87), the helium Hertzsprung gap, and R₂ = 0.08 L^0.75
/// (88), the helium giant branch. Time is counted from the helium zero-age main sequence.
///
/// The fits are HPT's to Pols's models from 0.32 to 10 M☉ at Z = 0.02, used at every metallicity
/// as HPT use them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HeliumStar {
    /// The helium star's (effective initial) mass, M☉.
    mass: SolarMasses,
    l_zams: SolarLuminosities,
    r_zams: SolarRadii,
    t_ms: Megayears,
    /// α and β of equations 82 and 83.
    alpha: f64,
    beta: f64,
    l_tms: SolarLuminosities,
    relation: GiantBranch,
    times: GiantTimes,
    /// λ of equation 87.
    lambda: SolarLuminosities,
    t_end: Megayears,
    end: CoreEnd,
}

impl HeliumStar {
    /// A naked helium star of mass `m` at its zero-age main sequence: a stripped star (plan 11), or
    /// a star that loses its envelope in the Hertzsprung gap or on the first giant branch from
    /// `M_HeF` up (HPT section 6).
    ///
    /// Its evolution ends when the core reaches `Mc,max` = min(1.45 M − 0.31, M) (equation 89),
    /// leaving a white dwarf, or first `Mc,SN` = max(`M_Ch`, 0.773 M − 0.35) (equation 75 with the
    /// helium star's mass for `Mc,BAGB`), in a supernova. Below 0.214 M☉, where 1.45 M − 0.31 is
    /// not positive, `Mc,max` is M, as in the published SSE code.
    ///
    /// The fits start at 0.32 M☉. The SSE code also turns a helium main-sequence star lighter than
    /// the core at helium ignition of an `M_HeF` star (about 0.33 M☉) into a helium white dwarf at
    /// once, a rule HPT do not print, which is left to the hand-over of P06.T10.d.
    ///
    /// # Panics
    ///
    /// In debug builds, if `m` is not positive.
    #[must_use]
    pub(crate) fn new(m: SolarMasses) -> Self {
        debug_assert!(m.value() > 0.0, "a helium star of {m:?}");
        let mass = m.value();
        let l_zams = zams_luminosity(m);
        let alpha = (0.85 - 0.08 * mass).max(0.0);
        let l_tms = l_zams * (1.0 + 0.45 + alpha);
        let t_ms = main_sequence_lifetime(m);
        let relation = GiantBranch::helium_giant(m, agb::HELIUM_RATE_MSUN_PER_LSUN_MYR);
        let times = relation.times_from(t_ms, l_tms);
        let mc_max = shell_limit(mass);
        let mc_sn = CHANDRASEKHAR_MSUN.max(0.773 * mass - 0.35);
        let (limit, end) = if mc_max < mc_sn {
            (mc_max, CoreEnd::WhiteDwarf)
        } else {
            (mc_sn, CoreEnd::Supernova)
        };
        let t_limit =
            relation.time_of_luminosity(&times, relation.luminosity(SolarMasses::new(limit)));
        Self {
            mass: m,
            l_zams,
            r_zams: zams_radius(m),
            t_ms,
            alpha,
            beta: (0.4 - 0.22 * math::log10(mass)).max(0.0),
            l_tms,
            relation,
            times,
            lambda: shell_lambda(m),
            t_end: if t_limit < t_ms { t_ms } else { t_limit },
            end,
        }
    }

    /// The helium star that core helium burning leaves at `t` when its envelope is gone (HPT
    /// section 6, equation 76), and its age: a star of the core's mass, whose relative age on the
    /// helium main sequence is the relative age of core helium burning, t = ((t′ − `t′_HeI`) ÷
    /// `t′_He`) `t_HeMS`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside core helium burning by more than rounding.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn from_core_helium_burning(
        phase: &CoreHeliumBurning,
        t: Megayears,
    ) -> (Self, Megayears) {
        let tau = (t - phase.t_start()) / (phase.t_end() - phase.t_start());
        let star = Self::new(phase.at(t).core_mass);
        let age = star.t_ms * tau;
        (star, age)
    }

    /// The helium giant that the early AGB leaves at `t` when its envelope is gone, and its age
    /// (HPT section 6): a star of the helium core's mass, `Mc,BAGB`, whose age puts its core at the
    /// early AGB's carbon–oxygen core on equation 84's relation, and no earlier than the end of
    /// its main sequence.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside the early AGB by more than rounding.
    #[must_use]
    pub(crate) fn from_early_agb(phase: &EarlyAgb, t: Megayears) -> (Self, Megayears) {
        let star = Self::new(phase.at(t).core_mass);
        let core = phase.co_core_mass(t);
        let age = star
            .relation
            .time_of_luminosity(&star.times, star.relation.luminosity(core));
        let age = if age < star.t_ms { star.t_ms } else { age };
        (star, age)
    }

    /// The end of the helium main sequence, `t_HeMS`, from the helium zero-age main sequence.
    #[must_use]
    pub(crate) const fn t_ms(&self) -> Megayears {
        self.t_ms
    }

    /// When the core reaches `Mc,max` or `Mc,SN` at constant mass.
    #[must_use]
    pub(crate) const fn t_end(&self) -> Megayears {
        self.t_end
    }

    /// How the star ends at constant mass: [`CoreEnd::WhiteDwarf`] when `Mc,max` < `Mc,SN`,
    /// otherwise [`CoreEnd::Supernova`].
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn end(&self) -> CoreEnd {
        self.end
    }

    /// The phase at age `t`: the helium main sequence before `t_HeMS`, then the helium
    /// Hertzsprung gap while R₁ < R₂ and the helium giant branch once R₂ ≤ R₁ (HPT section 6.1).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn phase_at(&self, t: Megayears) -> Phase {
        self.phase_at_mass(t, self.mass)
    }

    /// `HeliumStar::phase_at` for a star of current mass `mt`, whose radii decide it (see
    /// [`HeliumStar::at_mass`]).
    #[must_use]
    pub(crate) fn phase_at_mass(&self, t: Megayears, mt: SolarMasses) -> Phase {
        if t < self.t_ms {
            return Phase::HeliumMainSequence;
        }
        let l = self.relation.luminosity(self.core_mass_at(t));
        let (r1, r2) = self.giant_radii(l, zams_radius(mt), shell_lambda(mt));
        if r2 <= r1 {
            Phase::HeliumGiantBranch
        } else {
            Phase::HeliumHertzsprungGap
        }
    }

    /// The helium star's mass, M☉.
    #[must_use]
    pub(crate) const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The largest mass the carbon–oxygen core of a star of this helium star's (initial) mass
    /// reaches when its current mass is `mt`: min(`Mc,max`(`mt`), `Mc,SN`), with `Mc,max` =
    /// min(1.45 `mt` − 0.31, `mt`) (HPT equation 89, which the published SSE code evaluates at the
    /// current mass; below 0.214 M☉ it is `mt`) and `Mc,SN` = max(`M_Ch`, 0.773 M − 0.35) with
    /// the initial mass (equation 75). The star ends when its core reaches it.
    #[must_use]
    pub(crate) fn core_limit(&self, mt: SolarMasses) -> SolarMasses {
        let mc_sn = CHANDRASEKHAR_MSUN.max(0.773 * self.mass.value() - 0.35);
        SolarMasses::new(shell_limit(mt.value()).min(mc_sn))
    }

    /// The luminosity at the end of the helium main sequence, `L_THe`.
    #[must_use]
    pub(crate) const fn l_tms(&self) -> SolarLuminosities {
        self.l_tms
    }

    /// The helium giants' core mass–luminosity relation of this star (HPT equation 84), for the
    /// remnant of the early AGB (section 6.3).
    #[must_use]
    pub(crate) const fn relation(&self) -> &GiantBranch {
        &self.relation
    }

    /// The radius of this star at luminosity `l` after its main sequence, min(R₁, R₂) (HPT
    /// equation 85) at its own mass.
    #[must_use]
    pub(crate) fn shell_radius(&self, l: SolarLuminosities) -> SolarRadii {
        let (r1, r2) = self.giant_radii(l, self.r_zams, self.lambda);
        if r2 < r1 { r2 } else { r1 }
    }

    /// Luminosity, radius and core mass at age `t`, 0 ≤ t ≤ [`HeliumStar::t_end`] (HPT equations
    /// 80–88); the core is zero on the helium main sequence.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside the star's life by more than rounding.
    #[must_use]
    pub(crate) fn at(&self, t: Megayears) -> PhasePoint {
        self.point(t, self.r_zams, self.lambda)
    }

    /// [`HeliumStar::at`] for a star whose current mass `mt` has fallen below the mass it was
    /// built for, after its main sequence: HPT section 7.1 evaluate the radius at the current
    /// mass, so R₁ takes `R_ZHe`(`mt`) and λ(`mt`) (equations 78, 86 and 87), as the published SSE
    /// code does (`rzhef(mt)`, `rhehgf(mt, …)`); the luminosity and core keep the initial mass. On
    /// the helium main sequence the initial mass is the current one (HPT section 7.1), so a helium
    /// main-sequence star is rebuilt at its current mass rather than evaluated here. Equal, bit for
    /// bit, to [`HeliumStar::at`] when `mt` is the star's own mass.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`HeliumStar::at`].
    #[must_use]
    pub(crate) fn at_mass(&self, t: Megayears, mt: SolarMasses) -> PhasePoint {
        self.point(t, zams_radius(mt), shell_lambda(mt))
    }

    /// L, R and core at `t` with `r_zams` and `lambda` in equation 86.
    #[must_use]
    fn point(&self, t: Megayears, r_zams: SolarRadii, lambda: SolarLuminosities) -> PhasePoint {
        debug_assert!(
            t.value() >= -1e-9 * self.t_ms.value()
                && t.value() <= self.t_end.value() * (1.0 + 1e-12),
            "a helium star lives from 0 to its end, not {t:?}"
        );
        if t < self.t_ms {
            let tau = t / self.t_ms;
            return PhasePoint {
                luminosity: self.l_zams * (1.0 + 0.45 * tau + self.alpha * tau * tau),
                radius: self.r_zams * (1.0 + self.beta * (tau - math::powi(tau, 6))),
                core_mass: SolarMasses::ZERO,
            };
        }
        let core_mass = self.core_mass_at(t);
        let luminosity = self.relation.luminosity(core_mass);
        let (r1, r2) = self.giant_radii(luminosity, r_zams, lambda);
        PhasePoint {
            luminosity,
            radius: if r2 < r1 { r2 } else { r1 },
            core_mass,
        }
    }

    /// The carbon–oxygen core after the main sequence (HPT equation 39 with equation 84's
    /// relation).
    #[must_use]
    pub(crate) fn core_mass_at(&self, t: Megayears) -> SolarMasses {
        self.relation.core_mass_at(&self.times, t)
    }

    /// R₁ and R₂ of HPT equations 86 and 88 at luminosity `l`, with `r_zams` and `lambda` the
    /// zero-age radius and λ of equation 87 at the mass the radius is evaluated for.
    #[must_use]
    fn giant_radii(
        &self,
        l: SolarLuminosities,
        r_zams: SolarRadii,
        lambda: SolarLuminosities,
    ) -> (SolarRadii, SolarRadii) {
        let (l, l_tms, lambda) = (l.value(), self.l_tms.value(), lambda.value());
        let r1 = r_zams * math::powf_positive(l / l_tms, 0.2)
            + SolarRadii::new(0.02 * (math::exp(l / lambda) - math::exp(l_tms / lambda)));
        (r1, SolarRadii::new(0.08 * math::powf_positive(l, 0.75)))
    }
}

/// λ of HPT equation 87 for a helium star of mass `m`: 500 (2 + M⁵) ÷ M^2.5 L☉.
#[must_use]
fn shell_lambda(m: SolarMasses) -> SolarLuminosities {
    let mass = m.value();
    SolarLuminosities::new(500.0 * (2.0 + math::powi(mass, 5)) / math::powf_positive(mass, 2.5))
}

/// `Mc,max` of HPT equation 89 for a helium star of current mass `mt`: min(1.45 M − 0.31, M) at
/// the current mass, as the published SSE code evaluates it for equation 98's µ, and M itself below
/// 0.214 M☉, where the first is not positive.
#[must_use]
pub(crate) fn shell_limit_at(mt: SolarMasses) -> SolarMasses {
    SolarMasses::new(shell_limit(mt.value()))
}

/// `Mc,max` of HPT equation 89 for a helium star of current mass `m` (M☉): min(1.45 m − 0.31, m),
/// and m below 0.214 M☉, where the first is not positive (as in the published SSE code).
#[must_use]
fn shell_limit(m: f64) -> f64 {
    let limit = 1.45 * m - 0.31;
    if limit > 0.0 { limit.min(m) } else { m }
}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;

    use super::super::coeffs::ZCoeffs;
    use super::super::continuity::assert_continuous_over;
    use super::*;
    use crate::units::MetalFraction;

    /// Helium-star masses from 0.32 to 50 M☉, evenly in log mass.
    fn helium_masses(n: u32) -> Vec<f64> {
        let (lo, hi) = (math::log10(0.32), math::log10(50.0));
        (0..n)
            .map(|i| math::exp10(lo + (hi - lo) * f64::from(i) / f64::from(n - 1)))
            .collect()
    }

    /// The main sequence built alone is the whole star's, bit for bit, at every fractional age
    /// including its end, with its lifetime.
    #[test]
    fn the_main_sequence_alone_is_the_whole_stars_bit_for_bit() {
        for m in helium_masses(60) {
            let m = SolarMasses::new(m);
            let star = HeliumStar::new(m);
            for i in 0..=64 {
                let x = f64::from(i) / 64.0;
                let (point, t_ms) = main_sequence_at_fraction(m, x);
                assert_eq!(t_ms, star.t_ms());
                assert_eq!(point, star.at(star.t_ms() * x), "{m:?} at x = {x}");
            }
        }
    }

    fn mass(m: f64) -> SolarMasses {
        SolarMasses::new(m)
    }

    #[track_caller]
    fn assert_close(what: &str, a: f64, b: f64, tolerance: f64) {
        assert!((a / b - 1.0).abs() < tolerance, "{what}: {a} against {b}");
    }

    /// Equation 79 gives a 4 M☉ helium star 1.514 Myr on its main sequence (the plan's "about 1
    /// Myr"), as SSE's `themsf` does.
    #[test]
    fn a_four_solar_mass_helium_star_burns_helium_for_one_and_a_half_million_years() {
        let t = HeliumStar::new(mass(4.0)).t_ms().value();
        assert!((t - 1.514_362_902_832_031_1).abs() < 1e-12, "{t}");
    }

    /// L and R are continuous across the helium main sequence, Hertzsprung gap and giant branch
    /// for every mass, and meet at the end of the main sequence to 10⁻⁹; the core appears there, as
    /// HPT define none on the helium main sequence.
    #[test]
    fn luminosity_and_radius_are_continuous_through_a_helium_stars_life() {
        for m in helium_masses(60) {
            let star = HeliumStar::new(mass(m));
            let t_ms = star.t_ms();
            let (before, after) = (star.at(t_ms * (1.0 - 1e-15)), star.at(t_ms));
            let what = format!("M = {m}");
            assert_close(
                &format!("L at {what}"),
                before.luminosity.value(),
                after.luminosity.value(),
                1e-9,
            );
            assert_close(
                &format!("R at {what}"),
                before.radius.value(),
                after.radius.value(),
                1e-9,
            );
            let ages: Vec<f64> = (0..=2_000)
                .map(|i| star.t_end().value() * f64::from(i) / 2_000.0)
                .collect();
            let log_l = |t: f64| math::log10(star.at(Megayears::new(t)).luminosity.value());
            let log_r = |t: f64| math::log10(star.at(Megayears::new(t)).radius.value());
            assert_continuous_over(&format!("L at {what}"), log_l, &ages, 2e-3, 1e-6);
            assert_continuous_over(&format!("R at {what}"), log_r, &ages, 2e-3, 1e-6);
        }
    }

    /// At constant mass a helium star from about 1 to 2 M☉ reaches the helium giant branch (R₂ ≤ R₁,
    /// HPT section 6.1) before its core stops it; lighter ones stay in the Hertzsprung gap to the
    /// end, and heavier ones explode first. The published SSE code agrees at the masses of its run
    /// (type 9 only at 1.0, 1.2, 1.5 and 2.0 M☉ of 22 from 0.3 to 50).
    #[test]
    fn helium_giants_come_from_helium_stars_of_one_to_two_solar_masses() {
        let reaches_giant_branch = |m: f64| {
            let star = HeliumStar::new(mass(m));
            star.phase_at(star.t_end()) == Phase::HeliumGiantBranch
        };
        for m in [1.0, 1.2, 1.5, 2.0] {
            assert!(reaches_giant_branch(m), "{m} M☉");
        }
        for m in [0.5, 0.8, 2.5, 4.0, 10.0] {
            assert!(!reaches_giant_branch(m), "{m} M☉");
        }
        let star = HeliumStar::new(mass(1.5));
        assert_eq!(star.phase_at(Megayears::ZERO), Phase::HeliumMainSequence);
        assert_eq!(star.phase_at(star.t_ms()), Phase::HeliumHertzsprungGap);
    }

    /// A helium star ends when its core reaches `Mc,max` (a white dwarf, below `Mc,SN`) or
    /// `Mc,SN`, at the times the published SSE code's `star` gives (`tscls(14)`), to 10⁻¹².
    #[test]
    fn a_helium_star_ends_at_its_core_limit() {
        for &(m, t_end_sse, end) in &[
            (0.5, 162.752_347_088_322_18, CoreEnd::WhiteDwarf),
            (1.0, 23.420_932_746_346_367, CoreEnd::WhiteDwarf),
            (2.0, 5.113_785_919_314_66, CoreEnd::Supernova),
            (4.0, 1.626_627_343_258_939_5, CoreEnd::Supernova),
            (10.0, 0.672_139_588_448_242_5, CoreEnd::Supernova),
        ] {
            let star = HeliumStar::new(mass(m));
            assert_eq!(star.end(), end, "{m} M☉");
            assert_close(
                &format!("t_end at {m} M☉"),
                star.t_end().value(),
                t_end_sse,
                1e-12,
            );
            let core = star.at(star.t_end()).core_mass.value();
            let limit = (1.45 * m - 0.31)
                .min(m)
                .min(CHANDRASEKHAR_MSUN.max(0.773 * m - 0.35));
            assert_close(&format!("final core at {m} M☉"), core, limit, 1e-9);
        }
    }

    /// L, R and core mass against the published SSE code's `hrdiag` for helium stars at constant
    /// mass (see [`sse`](super::super) for the run; types 7–9 at 401 ages), to 10⁻⁹, where its
    /// small-envelope perturbation (HPT section 6.3, µ = 5 (`Mc,max` − Mc) ÷ `Mc,max` < 1 for
    /// helium giants) does not act.
    #[test]
    fn matches_the_published_sse_code() {
        for &(m, t, l_sse, r_sse, mc_sse) in SSE_HELIUM_STARS {
            let point = HeliumStar::new(mass(m)).at(Megayears::new(t));
            let what = format!("M = {m}, t = {t}");
            assert_close(
                &format!("L at {what}"),
                point.luminosity.value(),
                l_sse,
                1e-9,
            );
            assert_close(&format!("R at {what}"), point.radius.value(), r_sse, 1e-9);
            if mc_sse > 0.0 {
                assert_close(
                    &format!("Mc at {what}"),
                    point.core_mass.value(),
                    mc_sse,
                    1e-9,
                );
            } else {
                assert_eq!(
                    point.core_mass.total_cmp(&SolarMasses::ZERO),
                    Ordering::Equal,
                    "{what}"
                );
            }
        }
    }

    /// The helium star core helium burning leaves when its envelope is gone (HPT equation 76): its
    /// mass is the core's and its age the same fraction of `t_HeMS`, so its luminosity is the one
    /// P06.T10.d's small-envelope perturbation (HPT section 6.3) takes the star to as the envelope
    /// vanishes. The published SSE code's own entries agree to 10⁻⁹ (`hrdiag` of a core helium
    /// burning star whose mass is set just below its core, at τ = 0.1–0.9). A low-mass star with no
    /// envelope left meets it on the zero-age horizontal branch without the perturbation (HPT
    /// section 5.3), in L and R to 10⁻⁹ (the plan's 1%). A star that still has its envelope does
    /// not: the unperturbed formulae make a 1 M☉ star at Z = 0.02 0.49 dex brighter halfway through
    /// than the helium star of its core (0.13–1.75 dex over 0.8–20 M☉ and the phase), which is the
    /// jump T10.d's perturbation closes as the envelope thins.
    #[test]
    fn the_helium_star_left_by_core_helium_burning_continues_it() {
        for &(z, m, tau, l_sse, r_sse) in SSE_ENTRY_FROM_CHEB {
            let c = ZCoeffs::new(MetalFraction::new(z));
            let phase = CoreHeliumBurning::new(mass(m), &c);
            let t = phase.t_start() + (phase.t_end() - phase.t_start()) * tau;
            let (star, age) = HeliumStar::from_core_helium_burning(&phase, t);
            let point = star.at(age);
            let what = format!("Z = {z}, M = {m}, τ = {tau}");
            assert_close(
                &format!("L at {what}"),
                point.luminosity.value(),
                l_sse,
                1e-9,
            );
            assert_close(&format!("R at {what}"), point.radius.value(), r_sse, 1e-9);
            assert_close(&format!("age at {what}"), age / star.t_ms(), tau, 1e-12);
        }
        for z in [1e-4, 1e-3, 4e-3, 0.02, 0.03] {
            let c = ZCoeffs::new(MetalFraction::new(z));
            // The mass whose envelope at helium ignition is zero: M = Mc,HeI(M).
            let (mut lo, mut hi) = (0.3, 1.0);
            for _ in 0..80 {
                let mid = f64::midpoint(lo, hi);
                if mid < super::super::gb::mc_hei(mass(mid), &c).value() {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let phase = CoreHeliumBurning::new(mass(hi), &c);
            let start = phase.at(phase.t_start());
            let (star, age) = HeliumStar::from_core_helium_burning(&phase, phase.t_start());
            let point = star.at(age);
            let what = format!("M = {hi}, Z = {z}");
            assert_close(
                &format!("L at {what}"),
                start.luminosity.value(),
                point.luminosity.value(),
                1e-9,
            );
            assert_close(
                &format!("R at {what}"),
                start.radius.value(),
                point.radius.value(),
                1e-9,
            );
        }
        let c = ZCoeffs::new(MetalFraction::new(0.02));
        let phase = CoreHeliumBurning::new(mass(1.0), &c);
        let half = phase.t_start() + (phase.t_end() - phase.t_start()) * 0.5;
        let (star, age) = HeliumStar::from_core_helium_burning(&phase, half);
        let jump = math::log10(phase.at(half).luminosity / star.at(age).luminosity);
        assert!((0.48..0.5).contains(&jump), "{jump} dex");
    }

    /// The helium giant the early AGB leaves when the envelope is gone (HPT section 6) against the
    /// published SSE code's own entries (`hrdiag` of an early-AGB star whose mass is set to its
    /// helium core, at 10–90% of the phase), to 10⁻⁹.
    #[test]
    fn the_helium_giant_left_by_the_early_agb_continues_it() {
        for &(z, m, fraction, l_sse, r_sse) in SSE_ENTRY_FROM_EARLY_AGB {
            let c = ZCoeffs::new(MetalFraction::new(z));
            let phase = EarlyAgb::new(mass(m), &c);
            let t = phase.t_start() + (phase.t_end() - phase.t_start()) * fraction;
            let (star, age) = HeliumStar::from_early_agb(&phase, t);
            let point = star.at(age);
            let what = format!("Z = {z}, M = {m}, at {fraction}");
            assert_close(
                &format!("L at {what}"),
                point.luminosity.value(),
                l_sse,
                1e-9,
            );
            assert_close(&format!("R at {what}"), point.radius.value(), r_sse, 1e-9);
            assert!(age >= star.t_ms(), "{what}");
        }
    }

    /// L, R and `t_HeMS` against the published SSE code's `lzhef`, `rzhef` and `themsf` (see
    /// [`sse`](super::super) for the run), to 10⁻¹².
    #[test]
    fn zero_age_values_match_the_published_sse_code() {
        for &(m, l_sse, r_sse, t_sse) in SSE_HELIUM_ZAMS {
            let m = SolarMasses::new(m);
            let pairs = [
                ("L", zams_luminosity(m).value(), l_sse),
                ("R", zams_radius(m).value(), r_sse),
                ("t_HeMS", main_sequence_lifetime(m).value(), t_sse),
            ];
            for (name, ours, theirs) in pairs {
                assert!(
                    (ours / theirs - 1.0).abs() < 1e-12,
                    "{name} at {m:?}: {ours} against {theirs}"
                );
            }
        }
    }

    /// (M, `L_ZHe`, `R_ZHe`, `t_HeMS` Myr) from SSE's `lzhef`, `rzhef` and `themsf`.
    const SSE_HELIUM_ZAMS: &[(f64, f64, f64, f64)] = &[
        (
            0.5,
            17.924_020_164_166_762,
            0.110_467_206_395_587_92,
            146.397_408_077_875_47,
        ),
        (
            0.8,
            111.532_739_991_288_51,
            0.171_654_296_329_753_01,
            36.692_437_161_136_86,
        ),
        (
            1.0,
            247.090_270_031_359_84,
            0.204_621_309_370_988_46,
            21.0759,
        ),
        (
            4.0,
            16_667.975_929_763_48,
            0.527_913_646_614_270_4,
            1.514_362_902_832_031_1,
        ),
        (
            25.0,
            688_977.666_059_645_5,
            1.638_847_054_095_924,
            0.376_619_200_338_247_64,
        ),
    ];

    /// (M, t Myr, L, R, Mc) from SSE's `hrdiag` for helium stars: on the main sequence at 0.5 and 4
    /// M☉, in the Hertzsprung gap at 1 and 4 M☉, and on the giant branch at 1.5 and 2 M☉.
    const SSE_HELIUM_STARS: &[(f64, f64, f64, f64, f64)] = &[
        (
            0.5,
            72.628_234_888_163_77,
            25.498_754_765_620_333,
            0.135_250_063_388_145_55,
            0.0,
        ),
        (
            4.0,
            1.127_252_748_878_445_2,
            27_146.094_220_534_425,
            0.609_022_645_578_966_6,
            0.0,
        ),
        (
            1.0,
            21.886_861_651_460_68,
            912.543_212_990_631_1,
            0.234_465_906_797_382_58,
            0.471_198_345_776_879_1,
        ),
        (
            4.0,
            1.567_052_116_812_080_8,
            46_793.950_470_512_915,
            0.779_816_955_135_875_6,
            2.448_327_507_859_656_5,
        ),
        (
            1.5,
            9.198_996_485_586_544,
            24_270.814_300_035_83,
            155.561_903_102_518_53,
            1.059_471_814_179_908_9,
        ),
        (
            2.0,
            5.087_577_766_478_173,
            34_200.760_824_307_35,
            201.194_828_082_084_9,
            1.357_003_474_157_819_4,
        ),
    ];

    /// (Z, M, τ, L, R) from SSE's `hrdiag` at the helium star core helium burning leaves.
    const SSE_ENTRY_FROM_CHEB: &[(f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            0.5,
            24.391_740_674_563_45,
            0.133_788_461_851_429_73,
        ),
        (
            0.02,
            1.0,
            0.9,
            39.748_227_682_458_76,
            0.131_873_928_168_779_66,
        ),
        (
            0.0001,
            0.8,
            0.3,
            25.358_009_221_566_633,
            0.131_018_888_299_381_02,
        ),
    ];

    /// (Z, M, fraction of the early AGB, L, R) from SSE's `hrdiag` at the helium giant the early
    /// AGB leaves.
    const SSE_ENTRY_FROM_EARLY_AGB: &[(f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            0.5,
            234.132_605_963_847_34,
            0.158_590_407_939_403_93,
        ),
        (
            0.02,
            2.0,
            0.1,
            127.380_687_698_869_54,
            0.141_805_112_167_883_05,
        ),
        (
            0.004,
            3.0,
            0.5,
            1_435.545_833_271_666_4,
            0.260_603_678_090_427_34,
        ),
        (
            0.001,
            5.0,
            0.9,
            21_662.331_963_252_884,
            142.846_265_911_930_03,
        ),
    ];
}
