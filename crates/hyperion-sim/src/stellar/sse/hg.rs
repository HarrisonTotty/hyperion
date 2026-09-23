//! The Hertzsprung gap: from core hydrogen exhaustion to the base of the giant branch, or to
//! helium ignition from `M_FGB` up (plan 06, P06.T6.b; Hurley, Pols and Tout 2000, MNRAS 315, 543,
//! "HPT", section 5.1.2).
//!
//! Units are HPT's, in unit newtypes: M☉, L☉, R☉ and Myr from the zero-age main sequence.

// The track integrator of P06.T10 is the first caller outside tests.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the track integrator of P06.T10 is the first caller"
    )
)]

use crate::math;
use crate::units::{Megayears, SolarLuminosities, SolarMasses, SolarRadii};

use super::PhasePoint;
use super::coeffs::ZCoeffs;
use super::gb::{self, GiantBranch};
use super::ms;

/// A star of one mass crossing the Hertzsprung gap: its end points and core, evaluated once.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HertzsprungGap {
    t_ms: Megayears,
    t_bgb: Megayears,
    l_tms: SolarLuminosities,
    r_tms: SolarRadii,
    l_ehg: SolarLuminosities,
    r_ehg: SolarRadii,
    mc_ehg: SolarMasses,
    rho: f64,
}

impl HertzsprungGap {
    /// The gap of a star of mass `m` at the metallicity of `c`.
    ///
    /// The gap ends (HPT section 5.1) at the base of the giant branch below `M_FGB`, where
    /// `L_EHG` = `L_BGB` and `R_EHG` = `R_GB`(`L_BGB`), and at helium ignition from `M_FGB` up,
    /// with `L_HeI` and `R_HeI`. The core mass at its end (equation 28) is the giant branch's at
    /// `L_BGB` below `M_HeF`, `Mc,BGB` (equation 44) up to `M_FGB`, and `Mc,HeI` from there up.
    #[must_use]
    pub(crate) fn new(m: SolarMasses, c: &ZCoeffs) -> Self {
        let (m_hef, m_fgb) = (c.m_hef().value(), c.m_fgb().value());
        let l_bgb = ms::l_bgb(m, c);
        let (l_ehg, r_ehg) = if m.value() < m_fgb {
            (l_bgb, gb::radius(m, l_bgb, c))
        } else {
            (gb::l_hei(m, c), gb::r_hei(m, c))
        };
        let mc_ehg = if m.value() < m_hef {
            GiantBranch::new(m, c).core_mass(l_bgb)
        } else if m.value() < m_fgb {
            gb::mc_bgb(m, c)
        } else {
            gb::mc_hei(m, c)
        };
        let m525 = math::powf(m.value(), 5.25);
        Self {
            t_ms: ms::t_ms(m, c),
            t_bgb: ms::t_bgb(m, c),
            l_tms: ms::l_tms(m, c),
            r_tms: ms::r_tms(m, c),
            l_ehg,
            r_ehg,
            mc_ehg,
            // HPT equation 29.
            rho: (1.586 + m525) / (2.434 + 1.02 * m525),
        }
    }

    /// When the gap starts, `t_MS`.
    #[must_use]
    pub(crate) const fn t_start(&self) -> Megayears {
        self.t_ms
    }

    /// When the gap ends, `t_BGB` (for stars from `M_FGB` up, helium ignition).
    #[must_use]
    pub(crate) const fn t_end(&self) -> Megayears {
        self.t_bgb
    }

    /// Luminosity, radius and core mass at `t`, `t_MS` ≤ t ≤ `t_BGB` (HPT equations 25–30).
    ///
    /// L and R interpolate geometrically in τ = (t − `t_MS`) ÷ (`t_BGB` − `t_MS`) from the terminal
    /// main sequence to the end of the gap, and the core grows linearly from ρ `Mc,EHG` to
    /// `Mc,EHG`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` lies outside `t_MS` ≤ t ≤ `t_BGB` by more than rounding.
    #[must_use]
    pub(crate) fn at(&self, t: Megayears) -> PhasePoint {
        let tau = (t - self.t_ms) / (self.t_bgb - self.t_ms);
        debug_assert!(
            (-1e-9..=1.0 + 1e-9).contains(&tau),
            "the gap runs from t_MS to t_BGB, not τ = {tau}"
        );
        let l = self.l_tms.value() * math::powf(self.l_ehg / self.l_tms, tau);
        let r = self.r_tms.value() * math::powf(self.r_ehg / self.r_tms, tau);
        PhasePoint {
            luminosity: SolarLuminosities::new(l),
            radius: SolarRadii::new(r),
            core_mass: self.mc_ehg * ((1.0 - tau) * self.rho + tau),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::MetalFraction;

    const REFERENCE_Z: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    fn coeffs(z: f64) -> ZCoeffs {
        ZCoeffs::new(MetalFraction::new(z))
    }

    fn masses(n: u32) -> Vec<SolarMasses> {
        (0..n)
            .map(|i| SolarMasses::new(math::exp10(-0.3 + 2.3 * f64::from(i) / f64::from(n - 1))))
            .collect()
    }

    #[track_caller]
    fn assert_close(what: &str, a: f64, b: f64) {
        assert!((a / b - 1.0).abs() < 1e-9, "{what}: {a} against {b}");
    }

    /// The gap starts where the main sequence ends, in L and R. (Its core starts at ρ `Mc,EHG`:
    /// HPT define no core on the main sequence.)
    #[test]
    fn the_gap_starts_at_the_terminal_main_sequence() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(100) {
                let gap = HertzsprungGap::new(m, &c);
                let start = gap.at(gap.t_start());
                let end_of_ms = ms::MainSequence::new(m, &c).at(gap.t_start());
                let what = format!("{m:?}, Z = {z}");
                let (l0, l1) = (start.luminosity.value(), end_of_ms.luminosity.value());
                assert_close(&format!("L at {what}"), l0, l1);
                let (r0, r1) = (start.radius.value(), end_of_ms.radius.value());
                assert_close(&format!("R at {what}"), r0, r1);
            }
        }
    }

    /// Below `M_FGB` the gap ends where the giant branch starts, in L, R and core mass.
    #[test]
    fn the_gap_ends_at_the_base_of_the_giant_branch() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(100)
                .into_iter()
                .filter(|m| m.value() < c.m_fgb().value())
            {
                let what = format!("{m:?}, Z = {z}");
                let gap = HertzsprungGap::new(m, &c);
                let t_bgb = ms::t_bgb(m, &c).value();
                assert_close(&format!("t_end at {what}"), gap.t_end().value(), t_bgb);
                let end = gap.at(gap.t_end());
                let giant = gb::FirstGiantBranch::new(m, &c).at(gap.t_end());
                let pairs = [
                    ("L", end.luminosity.value(), giant.luminosity.value()),
                    ("R", end.radius.value(), giant.radius.value()),
                    ("Mc", end.core_mass.value(), giant.core_mass.value()),
                ];
                for (name, a, b) in pairs {
                    assert_close(&format!("{name} at {what}"), a, b);
                }
            }
        }
    }

    /// From `M_FGB` up the gap ends at helium ignition, at all five metallicities (below
    /// Z = 0.004 `M_FGB` lies under 12 M☉, where equation 50 interpolates).
    #[test]
    fn massive_stars_leave_the_gap_at_helium_ignition() {
        for z in REFERENCE_Z {
            let c = coeffs(z);
            for m in masses(100)
                .into_iter()
                .filter(|m| m.value() >= c.m_fgb().value())
            {
                let what = format!("{m:?}, Z = {z}");
                let end = HertzsprungGap::new(m, &c).at(ms::t_bgb(m, &c));
                let pairs = [
                    ("L", end.luminosity.value(), gb::l_hei(m, &c).value()),
                    ("R", end.radius.value(), gb::r_hei(m, &c).value()),
                    ("Mc", end.core_mass.value(), gb::mc_hei(m, &c).value()),
                ];
                for (name, a, b) in pairs {
                    assert_close(&format!("{name} at {what}"), a, b);
                }
            }
        }
    }

    /// Luminosity, radius and core mass in the gap from the published SSE code (see
    /// [`sse`](super::super) for the run; `hrdiag` with no mass loss), where its small-envelope
    /// perturbation (HPT section 6.3) does not act: L and R to 10⁻⁹, `Mc` to 10⁻⁷ (see
    /// [`gb`]'s test).
    #[test]
    fn matches_the_published_sse_code() {
        for &(z, m, t, l_sse, r_sse, mc_sse) in SSE_HG {
            let c = coeffs(z);
            let point = HertzsprungGap::new(SolarMasses::new(m), &c).at(Megayears::new(t));
            let (l, r, mc) = (
                point.luminosity.value(),
                point.radius.value(),
                point.core_mass.value(),
            );
            assert_close(&format!("L at Z = {z}, M = {m}"), l, l_sse);
            assert_close(&format!("R at Z = {z}, M = {m}"), r, r_sse);
            assert!(
                (mc / mc_sse - 1.0).abs() < 1e-7,
                "Mc at Z = {z}, M = {m}: {mc} against {mc_sse}"
            );
        }
    }

    /// The gap above `M_FGB` against the published SSE code, at every row of its run where the
    /// small-envelope perturbation does not act (only Z = 10⁻⁴, 5–10 M☉): L to 10⁻⁹ and `Mc` to
    /// 10⁻⁷, as below `M_FGB`. R is held to 0.4% only: below 12 M☉ the radius at ignition
    /// interpolates with µ = log(M ÷ 12) ÷ log(`M_FGB` ÷ 12) (equation 50), and `M_FGB`'s rounded
    /// constants (see [`ZCoeffs::m_fgb`]) move it by up to 0.3% (1.3 × 10⁻³ dex) at 5 M☉; with the
    /// code's own `M_FGB` these rows agree to 10⁻¹³.
    #[test]
    fn matches_the_published_sse_code_above_m_fgb() {
        for &(z, m, t, l_sse, r_sse, mc_sse) in SSE_HG_ABOVE_M_FGB {
            let c = coeffs(z);
            assert!(m > c.m_fgb().value());
            let point = HertzsprungGap::new(SolarMasses::new(m), &c).at(Megayears::new(t));
            let (l, r, mc) = (
                point.luminosity.value(),
                point.radius.value(),
                point.core_mass.value(),
            );
            assert_close(&format!("L at Z = {z}, M = {m}, t = {t}"), l, l_sse);
            assert!(
                (r / r_sse - 1.0).abs() < 4e-3,
                "R at Z = {z}, M = {m}, t = {t}: {r} against {r_sse}"
            );
            assert!(
                (mc / mc_sse - 1.0).abs() < 1e-7,
                "Mc at Z = {z}, M = {m}, t = {t}: {mc} against {mc_sse}"
            );
        }
    }

    /// (Z, M, t Myr, L, R, Mc) from SSE's `hrdiag` in the gap above `M_FGB`.
    const SSE_HG_ABOVE_M_FGB: &[(f64, f64, f64, f64, f64, f64)] = &[
        (
            1e-4,
            5.0,
            87.991_595_580_914_16,
            2_347.858_621_181_019,
            11.668_355_551_309_693,
            0.854_992_880_604_563_9,
        ),
        (
            1e-4,
            5.0,
            88.255_834_606_682_68,
            2_782.447_802_185_429_7,
            53.103_070_436_717_1,
            0.863_475_573_419_436,
        ),
        (
            1e-4,
            6.0,
            61.505_164_695_585_3,
            4_239.167_265_045_708,
            10.185_298_933_891_888,
            1.089_330_828_225_27,
        ),
        (
            1e-4,
            6.0,
            61.688_762_202_139_28,
            5_130.181_827_145_344,
            50.932_527_819_357_77,
            1.101_753_969_035_898_6,
        ),
        (
            1e-4,
            7.0,
            46.186_352_081_538_864,
            6_547.225_362_167_329,
            6.043_202_897_182_916,
            1.334_534_771_758_351_8,
        ),
        (
            1e-4,
            7.0,
            46.321_795_929_285_31,
            8_060.669_117_819_123,
            30.211_726_946_936_79,
            1.351_531_851_629_674_2,
        ),
        (
            1e-4,
            8.0,
            36.528_180_481_482_32,
            10_144.282_164_505_637,
            6.970_906_722_018_649,
            1.598_944_021_564_605_6,
        ),
        (
            1e-4,
            8.0,
            36.634_988_026_749_82,
            12_718.516_271_408_851,
            33.535_112_377_594_885,
            1.621_581_723_228_188,
        ),
        (
            1e-4,
            10.0,
            25.351_699_174_662_33,
            23_541.712_722_694_46,
            17.104_549_841_189_64,
            2.183_972_531_052_668,
        ),
    ];

    /// (Z, M, t Myr, L, R, Mc) from SSE's `hrdiag` in the Hertzsprung gap.
    const SSE_HG: &[(f64, f64, f64, f64, f64, f64)] = &[
        (
            0.02,
            1.0,
            11_288.239_634_371_148,
            2.306_068_825_501_887,
            1.886_394_298_777_529_7,
            0.129_566_799_781_671_02,
        ),
        (
            0.001,
            2.0,
            791.639_750_078_521_3,
            73.268_034_302_300_05,
            3.137_471_078_641_876,
            0.270_768_682_543_503_7,
        ),
        (
            0.02,
            5.0,
            104.243_429_766_339_87,
            1_120.015_414_113_294,
            18.083_965_039_504_47,
            0.852_369_694_210_765_2,
        ),
        (
            0.0001,
            3.0,
            270.821_582_525_407_8,
            375.037_363_800_633_3,
            21.533_398_377_182_13,
            0.445_369_467_909_315_64,
        ),
    ];
}
