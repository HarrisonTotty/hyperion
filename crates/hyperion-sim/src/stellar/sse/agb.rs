//! The asymptotic giant branch: the early AGB, on which a carbon–oxygen core grows inside the
//! helium core, and the thermally pulsing AGB, on which both grow together under third dredge-up
//! (plan 06, P06.T8; Hurley, Pols and Tout 2000, MNRAS 315, 543, "HPT", sections 5.4 and 6).
//!
//! Both phases use the giant branch's core mass–luminosity relation of the star's mass (equation
//! 37) with the core's growth driven by helium burning on the early AGB (rate `A_He`) and by both
//! shells on the thermally pulsing AGB (`A_H,He`). The early AGB ends when the thermal pulses begin
//! at the second dredge-up or when its carbon–oxygen core reaches `Mc,SN` with the envelope still
//! on (a supernova); the thermally pulsing AGB when its core reaches `Mc,SN` or the whole mass, and
//! a white dwarf is left. With mass loss the last is the envelope's loss, which the track integrator of P06.T10
//! finds; the phases here hold the mass fixed.
//!
//! The `pub(crate)` items take and return unit newtypes in HPT's units (M☉, L☉, R☉, Myr from the
//! zero-age main sequence). Equation numbers are the journal's.
//!
//! The comparison tests use the SSE build of [`sse`](super), run again on 2026-09-23 for these
//! phases: `hrdiag` at constant mass on finer age grids, for helium stars from 0.3 to 50 M☉, and at
//! the hand-over to a helium star, and a second time with the branch that applies the
//! small-envelope perturbation of HPT section 6.3 (equations 97–100) disabled, for rows where SSE
//! applies it; each test says which run it reads.

// The track integrator of P06.T10 is the first caller outside tests.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the track integrator of P06.T10 is the first caller"
    )
)]

use crate::math;
use crate::units::{Megayears, SolarMasses, Years};

use super::PhasePoint;
use super::cheb;
use super::coeffs::ZCoeffs;
use super::gb::{self, GiantBranch, GiantTimes, RadiusLaw};

/// `A_He`, the rate constant of helium-shell burning, M☉ L☉⁻¹ Myr⁻¹, which sets how fast the
/// carbon–oxygen core grows on the early AGB and a helium giant's core (HPT section 5.4).
///
/// HPT's equation 68 derives 7.66 × 10⁻⁵ from the energy released per gram, 8.09 × 10¹⁷ erg g⁻¹,
/// and `X_He` = 0.98; the published SSE code uses 8.0 × 10⁻⁵. The early AGB lasts 1/`A_He`, so the
/// printed value makes it 4.4% longer, which puts the steep rise of L at its end up to 0.11 dex
/// fainter at a given age against the code (2.5 M☉ at Z = 0.02), beyond the 0.02 dex P06.T12.b
/// validates to, so the code's value is used, pending the owner's confirmation.
pub(crate) const HELIUM_RATE_MSUN_PER_LSUN_MYR: f64 = 8.0e-5;

/// `A_H,He` = `A_H` `A_He` ÷ (`A_H` + `A_He`) ≈ 1.27 × 10⁻⁵ M☉ L☉⁻¹ Myr⁻¹, the combined rate of the
/// two shells on the thermally pulsing AGB (HPT equation 71), as printed, and as the published SSE
/// code has it.
pub(crate) const COMBINED_RATE_MSUN_PER_LSUN_MYR: f64 = 1.27e-5;

/// The Chandrasekhar mass, 1.44 M☉, which HPT use at all times (section 6.2.1).
pub(crate) const CHANDRASEKHAR_MSUN: f64 = 1.44;

/// The core mass after the second dredge-up, `Mc,DU`, at which the thermal pulses begin (HPT
/// section 5.4): 0.44 `Mc,BAGB` + 0.448 for 0.8 ≤ `Mc,BAGB` < 2.25, and `Mc,BAGB` itself outside,
/// where there is no dredge-up (the two meet at 0.8; above 2.25 the star never reaches the thermal
/// pulses).
#[must_use]
pub(crate) fn mc_du(m: SolarMasses, c: &ZCoeffs) -> SolarMasses {
    let mc_bagb = gb::mc_bagb(m, c);
    if (0.8..2.25).contains(&mc_bagb.value()) {
        SolarMasses::new(0.44 * mc_bagb.value() + 0.448)
    } else {
        mc_bagb
    }
}

/// The carbon–oxygen core mass at which the AGB ends in a supernova if the envelope is still
/// there, `Mc,SN` = max(`M_Ch`, 0.773 `Mc,BAGB` − 0.35) (HPT equation 75).
///
/// Below `Mc,BAGB` = 1.6 M☉ carbon ignites degenerately at the Chandrasekhar mass and leaves no
/// remnant; from 1.6 to 2.25 an oxygen–neon core collapses by electron capture; above, carbon burns
/// in a non-degenerate core that goes on to collapse (HPT section 6).
///
/// The published SSE code also lets the core grow to at least 1.05 times the relation's
/// carbon–oxygen core at the base of the AGB. That binds only where that core already exceeds
/// `Mc,SN` (40–80 M☉), whose early AGB ends at once here and lasts at most 0.24% of the lifetime
/// longer in the code, within P06.T12.b's 1%, so the printed form is kept. At 60 M☉ and Z = 0.0001
/// or 0.001 the code's carbon–oxygen core at the base of the AGB exceeds its helium core, and it
/// goes on through a thermally pulsing AGB with third dredge-up for 0.41 and 0.31 Myr more (8.2%
/// and 6.4% of its lifetime): an artefact of the code, not followed, pending the owner's
/// confirmation.
#[must_use]
pub(crate) fn mc_sn(m: SolarMasses, c: &ZCoeffs) -> SolarMasses {
    SolarMasses::new(CHANDRASEKHAR_MSUN.max(0.773 * gb::mc_bagb(m, c).value() - 0.35))
}

/// How the early AGB ends at constant mass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EarlyAgbEnd {
    /// The carbon–oxygen core reaches `Mc,DU` and the thermal pulses begin
    /// ([`EarlyAgb::thermal_pulses`]).
    ThermalPulses,
    /// The carbon–oxygen core reaches `Mc,SN` ([`mc_sn`]) with the envelope still on.
    Supernova,
}

/// How a phase whose core grows until it stops the star ends at constant mass: the thermally
/// pulsing AGB and a naked helium star (P06.T9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoreEnd {
    /// The core reaches `Mc,SN` with the envelope still on.
    Supernova,
    /// The core reaches the whole mass (for a helium star, HPT's `Mc,max`): the envelope is gone
    /// and a white dwarf is left.
    WhiteDwarf,
}

/// The early AGB of a star of one mass, from the end of core helium burning at `t_BAGB` to the
/// second dredge-up at `t_DU`, or to `Mc,SN` first (HPT section 5.4).
///
/// The hydrogen-exhausted core stays at `Mc,BAGB` while the carbon–oxygen core inside it grows
/// along the giant branch's relation of the star's mass (equations 37 and 39), with `A_He` for the
/// rate, `t_BAGB` for `t_BGB` and `L_BAGB` for `L_BGB`; L follows the carbon–oxygen core and R is
/// `R_AGB`(L) (equation 74).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EarlyAgb {
    mass: SolarMasses,
    zeta: f64,
    t_bagb: Megayears,
    t_end: Megayears,
    end: EarlyAgbEnd,
    relation: GiantBranch,
    times: GiantTimes,
    mc_bagb: SolarMasses,
    mc_du: SolarMasses,
    mc_sn: SolarMasses,
    asymptotic: RadiusLaw,
}

impl EarlyAgb {
    /// The early AGB of a star of mass `m` at the metallicity of `c`.
    #[must_use]
    pub(crate) fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        let t_bagb = cheb::t_hei(m, c) + cheb::t_he(m, c);
        let relation = GiantBranch::new(m, c).with_rate(HELIUM_RATE_MSUN_PER_LSUN_MYR);
        let times = relation.times_from(t_bagb, cheb::l_bagb(m, c));
        let (mc_du, mc_sn) = (mc_du(m, c), mc_sn(m, c));
        let (target, end) = if mc_sn.value() <= mc_du.value() {
            (mc_sn, EarlyAgbEnd::Supernova)
        } else {
            (mc_du, EarlyAgbEnd::ThermalPulses)
        };
        let t_target = relation.time_of_luminosity(&times, relation.luminosity(target));
        Self {
            mass: m,
            zeta: c.zeta(),
            t_bagb,
            t_end: if t_target < t_bagb { t_bagb } else { t_target },
            end,
            relation,
            times,
            mc_bagb: gb::mc_bagb(m, c),
            mc_du,
            mc_sn,
            asymptotic: RadiusLaw::asymptotic(m, c),
        }
    }

    /// When the early AGB starts, `t_BAGB` = `t_HeI` + `t_He`.
    #[must_use]
    pub(crate) const fn t_start(&self) -> Megayears {
        self.t_bagb
    }

    /// When it ends: `t_DU`, or where the carbon–oxygen core reaches `Mc,SN` (no earlier than
    /// `t_BAGB`).
    #[must_use]
    pub(crate) const fn t_end(&self) -> Megayears {
        self.t_end
    }

    /// How it ends.
    #[must_use]
    pub(crate) const fn end(&self) -> EarlyAgbEnd {
        self.end
    }

    /// The thermally pulsing AGB that follows, if the early AGB ends in the thermal pulses.
    #[must_use]
    pub(crate) fn thermal_pulses(&self) -> Option<ThermallyPulsingAgb> {
        (self.end == EarlyAgbEnd::ThermalPulses).then(|| ThermallyPulsingAgb::after(self))
    }

    /// The carbon–oxygen core at `t` (HPT equation 39 with `A_He`).
    #[must_use]
    pub(crate) fn co_core_mass(&self, t: Megayears) -> SolarMasses {
        self.relation.core_mass_at(&self.times, t)
    }

    /// Luminosity, radius and core mass at `t`, `t_BAGB` ≤ t ≤ [`EarlyAgb::t_end`]; the core mass
    /// is the hydrogen-exhausted core's, `Mc,BAGB` (the carbon–oxygen core is
    /// [`EarlyAgb::co_core_mass`]).
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside the phase by more than rounding.
    #[must_use]
    pub(crate) fn at(&self, t: Megayears) -> PhasePoint {
        debug_assert!(
            t.value() >= self.t_bagb.value() * (1.0 - 1e-12)
                && t.value() <= self.t_end.value() * (1.0 + 1e-12),
            "the early AGB runs from t_BAGB to its end, not {t:?}"
        );
        let luminosity = self.relation.luminosity(self.co_core_mass(t));
        PhasePoint {
            luminosity,
            radius: self.asymptotic.at(luminosity),
            core_mass: self.mc_bagb,
        }
    }
}

/// The thermally pulsing AGB of a star of one mass, from the second dredge-up at `t_DU` until its
/// core reaches `Mc,SN` or the whole mass (HPT section 5.4), built by
/// [`EarlyAgb::thermal_pulses`].
///
/// The carbon–oxygen and helium cores grow together along the giant branch's relation of the
/// star's mass with `A_H,He` for the rate, `t_DU` for `t_BGB` and `L_DU` = L(`Mc,DU`) for `L_BGB`
/// (equations 70–72), which gives L as it would be without dredge-up. Third dredge-up returns a
/// fraction λ = min(0.9, 0.3 + 0.001 M⁵) of the growth (equation 73), so the core is
/// `Mc,DU` + (1 − λ)(Mc′ − `Mc,DU`) with Mc′ the relation's core. R is `R_AGB`(L).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThermallyPulsingAgb {
    t_du: Megayears,
    t_end: Megayears,
    end: CoreEnd,
    relation: GiantBranch,
    times: GiantTimes,
    mc_du: SolarMasses,
    /// λ of equation 73.
    dredge_up: f64,
    asymptotic: RadiusLaw,
    zeta: f64,
}

impl ThermallyPulsingAgb {
    /// The thermally pulsing AGB that starts where `early` ends.
    #[must_use]
    fn after(early: &EarlyAgb) -> Self {
        let t_du = early.t_end;
        let relation = early.relation.with_rate(COMBINED_RATE_MSUN_PER_LSUN_MYR);
        let times = relation.times_from(t_du, relation.luminosity(early.mc_du));
        let (limit, end) = if early.mass <= early.mc_sn {
            (early.mass, CoreEnd::WhiteDwarf)
        } else {
            (early.mc_sn, CoreEnd::Supernova)
        };
        let mut phase = Self {
            t_du,
            t_end: t_du,
            end,
            relation,
            times,
            mc_du: early.mc_du,
            dredge_up: 0.9_f64.min(0.3 + 0.001 * math::powi(early.mass.value(), 5)),
            asymptotic: early.asymptotic,
            zeta: early.zeta,
        };
        phase.t_end = phase.time_of_core_mass(limit);
        phase
    }

    /// When the thermal pulses start, `t_DU`.
    #[must_use]
    pub(crate) const fn t_start(&self) -> Megayears {
        self.t_du
    }

    /// When the core reaches `Mc,SN` or the whole mass at constant mass.
    #[must_use]
    pub(crate) const fn t_end(&self) -> Megayears {
        self.t_end
    }

    /// How the phase ends at constant mass: [`CoreEnd::Supernova`] if `Mc,SN` is below the star's
    /// mass, [`CoreEnd::WhiteDwarf`] otherwise.
    #[must_use]
    pub(crate) const fn end(&self) -> CoreEnd {
        self.end
    }

    /// The time at which the core, after third dredge-up, reaches `mc` (`t_DU` if it starts there
    /// or above).
    #[must_use]
    pub(crate) fn time_of_core_mass(&self, mc: SolarMasses) -> Megayears {
        if mc <= self.mc_du {
            return self.t_du;
        }
        let undredged = self.mc_du + (mc - self.mc_du) * (1.0 / (1.0 - self.dredge_up));
        let t = self
            .relation
            .time_of_luminosity(&self.times, self.relation.luminosity(undredged));
        if t < self.t_du { self.t_du } else { t }
    }

    /// Luminosity, radius and core mass at `t`, `t_DU` ≤ t (HPT equations 37, 39, 73 and 74).
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies before `t_DU` by more than rounding.
    #[must_use]
    pub(crate) fn at(&self, t: Megayears) -> PhasePoint {
        debug_assert!(
            t.value() >= self.t_du.value() * (1.0 - 1e-12),
            "the thermally pulsing AGB starts at t_DU, not {t:?}"
        );
        let undredged = self.relation.core_mass_at(&self.times, t);
        let luminosity = self.relation.luminosity(undredged);
        PhasePoint {
            luminosity,
            radius: self.asymptotic.at(luminosity),
            core_mass: self.mc_du + (undredged - self.mc_du) * (1.0 - self.dredge_up),
        }
    }

    /// The interpulse period at core mass `mc` with `envelope` M☉ above it (see
    /// [`interpulse_period`]), with this phase's first pulse at `Mc,DU`.
    #[must_use]
    pub(crate) fn interpulse_period(&self, mc: SolarMasses, envelope: SolarMasses) -> Years {
        interpulse_period_at(mc, self.mc_du, envelope, self.zeta)
    }
}

/// The interpulse period of a thermally pulsing AGB star with core mass `mc`, whose first pulse
/// came at core mass `mc_first` and which has `envelope` M☉ above its core, at the metallicity of
/// `c` (Wagenhuber and Groenewegen 1998, A&A 340, 183, equation 11).
///
/// lg `τ_ip` = (−3.628 + 0.1337 z)(Mc − 1.9454) − 10^(−2.080 − 0.353 z + 0.200 (`M_env` + α − 1.5))
/// − 10^(−0.626 − 70.30 (`Mc,0` − z) `ΔMc`), `τ_ip` in years, with z = log₁₀(Z ÷ 0.02), `ΔMc` =
/// Mc − `Mc,0` ≥ 0 and the mixing-length parameter α at their fit's 1.5. The first term is the
/// asymptotic relation, the second hot-bottom burning's shortening and the third the turn-on of the
/// first pulses, which shortens `τ_ip` by almost half at the first. HPT model no pulses and give no
/// interpulse period. The relation was fitted to 0.8–7 M☉ at Z = 0.0001–0.02 and core masses up to
/// about 1 M☉; beyond those it is extrapolated.
#[must_use]
pub(crate) fn interpulse_period(
    mc: SolarMasses,
    mc_first: SolarMasses,
    envelope: SolarMasses,
    c: &ZCoeffs,
) -> Years {
    interpulse_period_at(mc, mc_first, envelope, c.zeta())
}

/// [`interpulse_period`] at z = log₁₀(Z ÷ 0.02) = `zeta`.
#[must_use]
fn interpulse_period_at(
    mc: SolarMasses,
    mc_first: SolarMasses,
    envelope: SolarMasses,
    zeta: f64,
) -> Years {
    const ALPHA_MLT: f64 = 1.5;
    let (mc, mc_first) = (mc.value(), mc_first.value());
    let growth = (mc - mc_first).max(0.0);
    let asymptotic = (-3.628 + 0.1337 * zeta) * (mc - 1.9454);
    let hot_bottom =
        math::exp10(-2.080 - 0.353 * zeta + 0.200 * (envelope.value() + ALPHA_MLT - 1.5));
    let turn_on = math::exp10(-0.626 - 70.30 * (mc_first - zeta) * growth);
    Years::new(math::exp10(asymptotic - hot_bottom - turn_on))
}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;

    use super::super::cheb::CoreHeliumBurning;
    use super::super::continuity::assert_no_jump;
    use super::*;
    use crate::units::MetalFraction;

    const REFERENCE_Z: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    fn coeffs(z: f64) -> ZCoeffs {
        ZCoeffs::new(MetalFraction::new(z))
    }

    fn mass(m: f64) -> SolarMasses {
        SolarMasses::new(m)
    }

    /// Masses from 0.8 to 100 M☉, evenly in log mass.
    fn masses(n: u32) -> Vec<f64> {
        let (lo, hi) = (math::log10(0.8), 2.0);
        (0..n)
            .map(|i| math::exp10(lo + (hi - lo) * f64::from(i) / f64::from(n - 1)))
            .collect()
    }

    #[track_caller]
    fn assert_close(what: &str, a: f64, b: f64, tolerance: f64) {
        assert!((a / b - 1.0).abs() < tolerance, "{what}: {a} against {b}");
    }

    /// The early AGB starts where core helium burning ends, in L, R and core mass, and an age sweep
    /// across `t_BAGB` finds no jump in L or R.
    #[test]
    fn the_early_agb_starts_where_core_helium_burning_ends() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(60) {
                let m = mass(m);
                let (cheb, early) = (CoreHeliumBurning::new(m, &c), EarlyAgb::new(m, &c));
                let what = format!("{m:?}, Z = {z}");
                assert_close(
                    &format!("t_BAGB at {what}"),
                    early.t_start().value(),
                    cheb.t_end().value(),
                    1e-15,
                );
                let (ending, starting) = (cheb.at(cheb.t_end()), early.at(early.t_start()));
                assert_close(
                    &format!("L at {what}"),
                    ending.luminosity.value(),
                    starting.luminosity.value(),
                    1e-9,
                );
                assert_close(
                    &format!("R at {what}"),
                    ending.radius.value(),
                    starting.radius.value(),
                    1e-9,
                );
                assert_close(
                    &format!("Mc at {what}"),
                    ending.core_mass.value(),
                    starting.core_mass.value(),
                    1e-12,
                );
                if early.t_end() <= early.t_start() {
                    continue;
                }
                let t0 = early.t_start().value();
                let dt = 0.1 * (t0 - cheb.t_start().value()).min(early.t_end().value() - t0);
                let point = |t: f64| {
                    let t = Megayears::new(t);
                    if t < early.t_start() {
                        cheb.at(t)
                    } else {
                        early.at(t)
                    }
                };
                let log_l = |t: f64| math::log10(point(t).luminosity.value());
                let log_r = |t: f64| math::log10(point(t).radius.value());
                assert_no_jump(
                    &format!("L across t_BAGB at {what}"),
                    log_l,
                    t0 - dt,
                    t0 + dt,
                    1e-6,
                );
                assert_no_jump(
                    &format!("R across t_BAGB at {what}"),
                    log_r,
                    t0 - dt,
                    t0 + dt,
                    1e-6,
                );
            }
        }
    }

    /// The thermal pulses start where the early AGB ends, continuous in L and R; the carbon–oxygen
    /// core reaches `Mc,DU`, which is where the pulses start, while the hydrogen-exhausted core
    /// falls from `Mc,BAGB` to `Mc,DU` in the second dredge-up for 0.8 ≤ `Mc,BAGB` < 2.25, a jump
    /// HPT declare (section 5.4).
    #[test]
    fn the_thermal_pulses_start_where_the_early_agb_ends() {
        let mut dredged = 0;
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(60) {
                let m = mass(m);
                let early = EarlyAgb::new(m, &c);
                let Some(pulsing) = early.thermal_pulses() else {
                    continue;
                };
                let what = format!("{m:?}, Z = {z}");
                let t_du = early.t_end();
                assert_eq!(
                    pulsing.t_start().total_cmp(&t_du),
                    Ordering::Equal,
                    "{what}"
                );
                let (ending, starting) = (early.at(t_du), pulsing.at(t_du));
                assert_close(
                    &format!("L at {what}"),
                    ending.luminosity.value(),
                    starting.luminosity.value(),
                    1e-9,
                );
                assert_close(
                    &format!("R at {what}"),
                    ending.radius.value(),
                    starting.radius.value(),
                    1e-9,
                );
                let mc_du = mc_du(m, &c).value();
                assert_close(
                    &format!("CO core at {what}"),
                    early.co_core_mass(t_du).value(),
                    mc_du,
                    1e-9,
                );
                assert_close(
                    &format!("Mc at {what}"),
                    starting.core_mass.value(),
                    mc_du,
                    1e-9,
                );
                let mc_bagb = gb::mc_bagb(m, &c).value();
                if (0.8..2.25).contains(&mc_bagb) {
                    dredged += 1;
                    assert!(
                        ending.core_mass.value() > starting.core_mass.value() + 1e-3,
                        "no dredge-up at {what}"
                    );
                } else {
                    assert_close(
                        &format!("Mc,BAGB at {what}"),
                        ending.core_mass.value(),
                        mc_du,
                        1e-12,
                    );
                }
                let dt = 0.1
                    * (t_du - early.t_start())
                        .value()
                        .min((pulsing.t_end() - t_du).value());
                let t0 = t_du.value();
                let point = |t: f64| {
                    let t = Megayears::new(t);
                    if t < t_du { early.at(t) } else { pulsing.at(t) }
                };
                let log_l = |t: f64| math::log10(point(t).luminosity.value());
                let log_r = |t: f64| math::log10(point(t).radius.value());
                assert_no_jump(
                    &format!("L across t_DU at {what}"),
                    log_l,
                    t0 - dt,
                    t0 + dt,
                    1e-6,
                );
                assert_no_jump(
                    &format!("R across t_DU at {what}"),
                    log_r,
                    t0 - dt,
                    t0 + dt,
                    1e-6,
                );
            }
        }
        assert!(dredged > 20, "only {dredged} stars had a second dredge-up");
    }

    /// At constant mass a 1 M☉ star's core grows monotonically through the AGB at every
    /// metallicity: the carbon–oxygen core on the early AGB (the helium core stays at `Mc,BAGB`,
    /// below 0.8 M☉, so there is no second dredge-up), then the common core of the thermal pulses,
    /// until it reaches the whole mass and a white dwarf is left.
    #[test]
    fn a_solar_mass_core_grows_monotonically_at_constant_mass() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            let early = EarlyAgb::new(mass(1.0), &c);
            let pulsing = early
                .thermal_pulses()
                .expect("a solar-mass star reaches the pulses");
            assert_eq!(pulsing.end(), CoreEnd::WhiteDwarf, "Z = {z}");
            let (t0, t1, t2) = (
                early.t_start().value(),
                early.t_end().value(),
                pulsing.t_end().value(),
            );
            let mut last = 0.0;
            for i in 0..=2_000 {
                let f = f64::from(i) / 2_000.0;
                let core = if f <= 0.5 {
                    early.co_core_mass(Megayears::new(t0 + (t1 - t0) * 2.0 * f))
                } else {
                    pulsing
                        .at(Megayears::new(t1 + (t2 - t1) * (2.0 * f - 1.0)))
                        .core_mass
                };
                assert!(
                    core.value() >= last,
                    "the core shrinks at step {i}, Z = {z}"
                );
                last = core.value();
            }
            assert_close(&format!("final core at Z = {z}"), last, 1.0, 1e-9);
        }
    }

    /// `m_c_bagb` (HPT equation 66) reaches 1.6 M☉ at 6.31 M☉ and 2.25 M☉ at 8.20 M☉ at
    /// Z = 0.02, the `M_up` and `M_ec` that HPT section 6 defines by inverting it (the plan's "near
    /// 6.5 and 8"); the published SSE code's `mcagbf` agrees to 10⁻¹².
    #[test]
    fn the_core_at_the_base_of_the_agb_reaches_1_6_and_2_25_solar_masses_near_6_3_and_8_2() {
        let c = coeffs(0.02);
        let crossing = |target: f64| {
            let (mut lo, mut hi) = (1.0, 20.0);
            for _ in 0..60 {
                let mid = f64::midpoint(lo, hi);
                if super::super::m_c_bagb(mass(mid), &c).value() < target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            lo
        };
        let (m_up, m_ec) = (crossing(1.6), crossing(2.25));
        assert!((m_up - 6.307).abs() < 1e-3, "M_up = {m_up}");
        assert!((m_ec - 8.203).abs() < 1e-3, "M_ec = {m_ec}");
        for (m, sse) in [
            (6.0, 1.500_436_845_699_506_3),
            (7.0, 1.831_100_944_988_399_7),
            (8.0, 2.177_756_448_874_952),
            (10.0, 2.911_824_016_544_468),
        ] {
            assert_close(
                &format!("Mc,BAGB at {m}"),
                gb::mc_bagb(mass(m), &c).value(),
                sse,
                1e-12,
            );
        }
    }

    /// The landmarks against the published SSE code's `star` (see [`sse`](super::super) for the
    /// run), to 10⁻¹²: `t_DU` (its `tscls(13)`) and `L_DU` (`lums(8)`) for stars that reach the
    /// thermal pulses, and the time the core reaches `Mc,SN` at constant mass (`tscls(14)`).
    #[test]
    fn landmarks_match_the_published_sse_code() {
        for &(z, m, t_du_sse, t_sn_sse, l_du_sse) in SSE_AGB_LANDMARKS {
            let (c, m) = (coeffs(z), mass(m));
            let what = format!("Z = {z}, {m:?}");
            let early = EarlyAgb::new(m, &c);
            let relation = GiantBranch::new(m, &c);
            let l_du = relation.luminosity(mc_du(m, &c)).value();
            assert_close(&format!("L_DU at {what}"), l_du, l_du_sse, 1e-12);
            assert_eq!(early.end(), EarlyAgbEnd::ThermalPulses, "{what}");
            assert_close(
                &format!("t_DU at {what}"),
                early.t_end().value(),
                t_du_sse,
                1e-12,
            );
            let pulsing = early.thermal_pulses().expect("the pulses follow");
            assert_eq!(pulsing.end(), CoreEnd::Supernova, "{what}");
            assert_close(
                &format!("t_SN at {what}"),
                pulsing.t_end().value(),
                t_sn_sse,
                1e-12,
            );
        }
    }

    /// (Z, M, `t_DU` Myr, `t(Mc,SN)` Myr, `L_DU`) from SSE's `star`.
    const SSE_AGB_LANDMARKS: &[(f64, f64, f64, f64, f64)] = &[
        (
            0.0001,
            6.0,
            68.970_006_251_695,
            69.941_907_258_661_32,
            26_900.624_801_687_89,
        ),
        (
            0.001,
            5.0,
            103.235_937_981_118_2,
            104.521_244_253_460_07,
            27_063.434_655_106_943,
        ),
        (
            0.004,
            3.0,
            383.267_305_236_098_3,
            385.315_861_242_738_3,
            12_196.886_699_308_769,
        ),
        (
            0.02,
            7.0,
            55.422_110_254_505_824,
            56.392_238_508_478_85,
            29_288.095_805_185_836,
        ),
    ];

    /// How the AGB ends at constant mass, by the core at its base (HPT section 6): up to 2.25 M☉
    /// the thermal pulses begin; above, the early AGB ends in a supernova when the carbon–oxygen
    /// core reaches `Mc,SN`. A star lighter than `Mc,SN` ends its pulses as a white dwarf of its
    /// whole mass.
    #[test]
    fn the_agb_ends_by_the_core_at_its_base() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(60) {
                let m = mass(m);
                let early = EarlyAgb::new(m, &c);
                let mc_bagb = gb::mc_bagb(m, &c).value();
                let what = format!("{m:?}, Z = {z}");
                if mc_bagb < 2.25 {
                    assert_eq!(early.end(), EarlyAgbEnd::ThermalPulses, "{what}");
                    let pulsing = early.thermal_pulses().expect("the pulses follow");
                    let expected = if m.value() <= CHANDRASEKHAR_MSUN {
                        CoreEnd::WhiteDwarf
                    } else {
                        CoreEnd::Supernova
                    };
                    assert_eq!(pulsing.end(), expected, "{what}");
                    let end = pulsing.at(pulsing.t_end()).core_mass.value();
                    let limit = m.value().min(mc_sn(m, &c).value());
                    assert_close(&format!("final core at {what}"), end, limit, 1e-9);
                } else {
                    assert_eq!(early.end(), EarlyAgbEnd::Supernova, "{what}");
                    assert!(early.thermal_pulses().is_none(), "{what}");
                }
            }
        }
    }

    /// L, R and core mass against the published SSE code's `hrdiag` with no mass loss, sampled
    /// finely over the AGB (see [`sse`](super::super) for the run), at points where its
    /// small-envelope perturbation does not act (μ ≥ 1), to 10⁻⁹. The whole run of 5 Z × 28 masses
    /// agrees to 10⁻¹¹ on such points.
    #[test]
    fn matches_the_published_sse_code() {
        for &(z, m, t, l_sse, r_sse, mc_sse) in SSE_EARLY_AGB {
            let point = EarlyAgb::new(mass(m), &coeffs(z)).at(Megayears::new(t));
            let what = format!("early AGB at Z = {z}, M = {m}, t = {t}");
            assert_close(
                &format!("L of {what}"),
                point.luminosity.value(),
                l_sse,
                1e-9,
            );
            assert_close(&format!("R of {what}"), point.radius.value(), r_sse, 1e-9);
            assert_close(
                &format!("Mc of {what}"),
                point.core_mass.value(),
                mc_sse,
                1e-9,
            );
        }
        for &(z, m, t, l_sse, r_sse, mc_sse) in SSE_PULSING_AGB {
            let c = coeffs(z);
            let early = EarlyAgb::new(mass(m), &c);
            let pulsing = early.thermal_pulses().expect("the pulses follow");
            let point = pulsing.at(Megayears::new(t));
            let what = format!("thermal pulses at Z = {z}, M = {m}, t = {t}");
            assert_close(
                &format!("L of {what}"),
                point.luminosity.value(),
                l_sse,
                1e-9,
            );
            assert_close(&format!("R of {what}"), point.radius.value(), r_sse, 1e-9);
            assert_close(
                &format!("Mc of {what}"),
                point.core_mass.value(),
                mc_sse,
                1e-9,
            );
        }
    }

    /// (Z, M, t Myr, L, R, Mc) from SSE's `hrdiag` on the early AGB.
    const SSE_EARLY_AGB: &[(f64, f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            12_457.976_465_261_814,
            247.726_665_223_298_7,
            31.898_679_381_926_453,
            0.512_228_320_142_355_4,
        ),
        (
            0.001,
            5.0,
            103.107_890_958_810_07,
            11_520.981_775_269_734,
            190.787_727_977_236_9,
            1.552_945_132_471_57,
        ),
        (
            0.03,
            2.5,
            842.978_233_618_701_9,
            3_976.490_015_174_501,
            176.682_618_014_906_15,
            0.576_175_923_862_159_1,
        ),
    ];

    /// (Z, M, t Myr, L, R, Mc) from SSE's `hrdiag` on the thermally pulsing AGB; the 1 M☉ star's
    /// pulses start above `M_x` (equation 72).
    const SSE_PULSING_AGB: &[(f64, f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            12_461.997_215_757_694,
            5_855.994_084_782_481,
            284.739_766_296_269_8,
            0.559_661_529_923_040_4,
        ),
        (
            0.004,
            3.0,
            383.611_139_007_865_63,
            18_087.219_835_302_752,
            357.198_664_899_122_66,
            0.817_478_087_833_718_4,
        ),
        (
            0.0001,
            6.0,
            69.074_296_611_919_15,
            31_086.982_871_781_307,
            314.586_849_419_010_3,
            1.308_126_950_289_926,
        ),
    ];

    /// The interpulse period of Wagenhuber and Groenewegen (1998, eq. 11): the asymptotic relation
    /// gives 7.6 × 10⁴ yr at a 0.6 M☉ core at Z = 0.02 (their Fig. 6), shortened by the turn-on
    /// effect to 0.58 of that at the first pulse and by under 3% for a small envelope; the period
    /// falls with core mass and lengthens at low metallicity.
    #[test]
    fn the_interpulse_period_follows_wagenhuber_and_groenewegen() {
        let c = coeffs(0.02);
        let asymptotic = math::exp10(-3.628 * (0.6 - 1.945_4));
        assert!((asymptotic - 7.61e4).abs() < 100.0, "{asymptotic}");
        let tau = |mc: f64, first: f64, envelope: f64, c: &ZCoeffs| {
            super::super::interpulse_period(mass(mc), mass(first), mass(envelope), c).value()
        };
        let first = tau(0.6, 0.6, 0.1, &c);
        let turn_on = math::exp10(-math::exp10(-0.626));
        assert!(
            (first / (asymptotic * turn_on) - 1.0).abs() < 0.03,
            "{first}"
        );
        let later = tau(0.65, 0.6, 0.1, &c);
        let later_asymptotic = math::exp10(-3.628 * (0.65 - 1.945_4));
        assert!((later / later_asymptotic - 1.0).abs() < 0.03, "{later}");
        assert!(
            tau(0.57, 0.55, 0.5, &c) > tau(0.55, 0.55, 0.5, &c),
            "no turn-on"
        );
        let mut last = f64::INFINITY;
        for i in 0..=80 {
            let mc = 0.62 + 0.01 * f64::from(i);
            let t = tau(mc, 0.55, 0.5, &c);
            assert!(t < last, "τ_ip rises at Mc = {mc}");
            last = t;
        }
        assert!(
            tau(0.7, 0.6, 0.5, &coeffs(1e-4)) > 1.5 * tau(0.7, 0.6, 0.5, &c),
            "τ_ip is not longer at low metallicity"
        );
        // A pulsing star's own period takes its first pulse at `Mc,DU`.
        let early = EarlyAgb::new(mass(1.0), &c);
        let pulsing = early
            .thermal_pulses()
            .expect("a solar-mass star reaches the pulses");
        let own = pulsing.interpulse_period(mass(0.56), mass(0.4)).value();
        let free = tau(0.56, mc_du(mass(1.0), &c).value(), 0.4, &c);
        assert!((own - free).abs() < 1e-9 * own, "{own} against {free}");
    }
}
