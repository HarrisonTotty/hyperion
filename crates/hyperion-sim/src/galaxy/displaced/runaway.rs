//! Runaways and walkaways: the closed-form model of plan 08's Design note 24, its shares, speeds
//! and ejection ages (P08.T9.c).
//!
//! A living star of layer D or E is a runaway or a walkaway with a probability that depends on its
//! mass alone, and moves at a speed drawn from a law of its kind. It was ejected either by a close
//! encounter in its young cluster, at an age uniform on 0–3 Myr, or by its binary companion's
//! supernova, at that companion's lifetime. The class table ([`class_table`](super::class_table))
//! integrates these laws over the birth component's ages to split the living stars of the thin
//! disc and the nuclear disc (whose young part is the other source of massive stars) into runaway
//! and walkaway classes by speed bin and time since ejection. Every constant is a named part of the
//! generator version.
//!
//! Both shares are **lifetime** shares, the probability that a star is ever ejected; the class
//! table counts a star as displaced only once its ejection age has passed, so the present-day
//! fractions the literature measures come out lower (ruling 128.2 of 2026-09-22).
//!
//! - **Runaway share** ([`RunawayModel::runaway_share`]): 0.03 below 8 M☉, rising linearly in log
//!   mass from 0.05 at 8 M☉ to 0.30 at 20 M☉ and flat above, chosen so that the living O stars'
//!   present-day share is about a fifth. Hoogerwerf, de Bruijne and de Zeeuw (2001, A&A 365, 49,
//!   §1) write "About 10–30% of the O stars and 5–10% of the B stars" are runaways, and
//!   Carretero-Castrillo, Ribó and Paredes (2023, A&A) find 25–30% of O stars in Gaia DR3. The
//!   5–10% of B stars is read as the early B stars of 8–16 M☉ (P08.T9.c's windows).
//! - **Walkaway share** ([`RunawayModel::walkaway_share`]): `W × r(m)` above 2.5 M☉, with
//!   [`walkaway_ramp`] `r(m)` the count of secondaries released by core-collapse primaries under a
//!   2.3 slope with q uniform on 0.1–1 (ruling 128.3, the advisor's derivation), and W solved by
//!   the class table so that living stars above 15 M☉ are 0.10 walkaways today, Renzo et al.'s
//!   (2019, A&A 624, A66, §4 and Table 1) figure, measured there only.
//! - **Speeds**: a runaway at 30 km/s plus an exponential of mean 20 km/s, cut at half the
//!   source's circular speed; a walkaway Rayleigh of σ 10 km/s, cut at 30 km/s. Each cut truncates
//!   the law, which is renormalised inside it. The walkaways' mean, median and 90th percentile
//!   (12.5, 11.7 and 21.5 km/s) match Renzo et al.'s 12.4, 10.4 and about 20 km/s (their Table 1
//!   and conclusions). Every walkaway lies in speed bin 0 whatever the law.
//! - **Ejection**: nine runaways in ten at an age uniform on 0–3 Myr (encounters; Renzo et al.'s
//!   binary-supernova channel gives only 0.5 (+1.0, −0.4)% runaways above 15 M☉), the rest, and
//!   every walkaway, at the lifetime of the companion, a primary of mass max(8 M☉, m ÷ q) with q
//!   uniform on 0.3–1 (supernova release).
//! - **Height** ([`RunawayModel::mean_height_after`]): the mean |z| of runaways some time after
//!   an isotropic ejection from the midplane in a given vertical force, the quantity P08.T9.c's
//!   600–800 ly at 10 Myr is stated in (ruling 128.4).

use super::{SPEED_BINS, SPEED_EDGES, SpeedBin};
use crate::math;
use crate::units::{KilometresPerSecond, SolarMasses, Years};

/// The runaway share of living stars below 8 M☉ (Design note 24; Hoogerwerf et al. 2001 give 5–10%
/// of B stars).
pub const RUNAWAY_SHARE_LOW: f64 = 0.03;

/// The runaway share at 8 M☉, where the rise in log mass begins.
pub const RUNAWAY_SHARE_AT_8: f64 = 0.05;

/// The lifetime runaway share at and above 20 M☉, set so that living O stars are about a fifth
/// runaways today (ruling 128.2; Hoogerwerf et al. 2001: 10–30% of O stars; Carretero-Castrillo
/// et al. 2023: 25–30%).
pub const RUNAWAY_SHARE_HIGH: f64 = 0.30;

/// The mass where the runaway share's rise begins.
pub const RUNAWAY_RISE_START: SolarMasses = SolarMasses::new(8.0);

/// The mass where the runaway share's rise ends.
pub const RUNAWAY_RISE_END: SolarMasses = SolarMasses::new(20.0);

/// The present-day walkaway share of living stars above [`WALKAWAY_CALIBRATION_MASS`] that the
/// class table solves the scale W for (Renzo et al. 2019, §4 and Table 1: ∼10%).
pub const WALKAWAY_PRESENT_SHARE: f64 = 0.10;

/// The least mass of the stars [`WALKAWAY_PRESENT_SHARE`] is measured over (Renzo et al. 2019).
pub const WALKAWAY_CALIBRATION_MASS: SolarMasses = SolarMasses::new(15.0);

/// The least mass of a core-collapse primary in [`walkaway_ramp`].
pub const RAMP_PRIMARY_MASS: SolarMasses = SolarMasses::new(8.0);

/// The IMF slope of [`walkaway_ramp`]'s primaries.
pub const RAMP_SLOPE: f64 = 2.3;

/// The least mass ratio of [`walkaway_ramp`]'s pairs.
pub const RAMP_Q_MIN: f64 = 0.1;

/// The least mass of a walkaway: lighter released companions are left out (brainstorm,
/// "Displaced objects: kicks and runaways").
pub const WALKAWAY_MIN_MASS: SolarMasses = SolarMasses::new(2.5);

/// A runaway's least speed.
pub const RUNAWAY_MIN_SPEED: KilometresPerSecond = KilometresPerSecond::new(30.0);

/// The mean of a runaway's speed above [`RUNAWAY_MIN_SPEED`].
pub const RUNAWAY_EXCESS_MEAN: KilometresPerSecond = KilometresPerSecond::new(20.0);

/// A runaway's speed is cut at this fraction of its source's circular speed.
pub const RUNAWAY_CUT_OF_V_C: f64 = 0.5;

/// The Rayleigh σ, its mode, of a walkaway's speed: fitted to Renzo et al.'s mean, median and
/// 90th percentile, not their 6 km/s peak (ruling 128.3).
pub const WALKAWAY_MODE: KilometresPerSecond = KilometresPerSecond::new(10.0);

/// A walkaway's speed is cut here.
pub const WALKAWAY_CUT: KilometresPerSecond = KilometresPerSecond::new(30.0);

/// The share of runaways ejected by encounters; the rest are released by a supernova (ruling
/// 128.2: dynamical ejection dominates, Renzo et al. 2019; Carretero-Castrillo et al. 2023).
pub const ENCOUNTER_SHARE: f64 = 0.9;

/// The latest age of an encounter ejection: uniform on 0–3 Myr.
pub const ENCOUNTER_MAX_AGE: Years = Years::new(3.0e6);

/// The least mass of the companion whose supernova releases the star.
pub const RELEASING_MIN_MASS: SolarMasses = SolarMasses::new(8.0);

/// The mass ratio of the released star to its companion, uniform on this range.
pub const RELEASE_Q_RANGE: (f64, f64) = (0.3, 1.0);

/// What a displaced living star is: faster or slower than [`RUNAWAY_MIN_SPEED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ejected {
    /// A runaway, 30 km/s or more.
    Runaway,
    /// A walkaway, under 30 km/s.
    Walkaway,
}

/// The count of secondaries of mass `m` released by core-collapse primaries, relative to its value
/// above 8 M☉: `min(1, ((m ÷ 8)^2.3 − 0.1^2.3) ÷ (1 − 0.1^2.3))`, 0 where it would be negative
/// (ruling 128.3).
#[must_use]
pub fn walkaway_ramp(m: SolarMasses) -> f64 {
    let floor = math::powf(RAMP_Q_MIN, RAMP_SLOPE);
    let x = math::powf(m.value() / RAMP_PRIMARY_MASS.value(), RAMP_SLOPE);
    ((x - floor) / (1.0 - floor)).clamp(0.0, 1.0)
}

/// Plan 08's Design note 24 as functions of mass and speed (module documentation), with the
/// walkaways' scale W the class table solves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunawayModel {
    walkaway_scale: f64,
}

impl RunawayModel {
    /// The model whose walkaway lifetime share is `walkaway_scale × r(m)`.
    #[must_use]
    pub const fn new(walkaway_scale: f64) -> Self {
        Self { walkaway_scale }
    }

    /// W, the walkaway lifetime share above 8 M☉.
    #[must_use]
    pub const fn walkaway_scale(self) -> f64 {
        self.walkaway_scale
    }

    /// The lifetime share of stars of initial mass `m` that become runaways.
    #[must_use]
    pub fn runaway_share(self, m: SolarMasses) -> f64 {
        let (m, lo, hi) = (
            m.value(),
            RUNAWAY_RISE_START.value(),
            RUNAWAY_RISE_END.value(),
        );
        if m < lo {
            RUNAWAY_SHARE_LOW
        } else if m >= hi {
            RUNAWAY_SHARE_HIGH
        } else {
            let t = math::ln(m / lo) / math::ln(hi / lo);
            RUNAWAY_SHARE_AT_8 + t * (RUNAWAY_SHARE_HIGH - RUNAWAY_SHARE_AT_8)
        }
    }

    /// The lifetime share of stars of initial mass `m` that become walkaways: `W × r(m)` above
    /// [`WALKAWAY_MIN_MASS`].
    #[must_use]
    pub fn walkaway_share(self, m: SolarMasses) -> f64 {
        if m.value() > WALKAWAY_MIN_MASS.value() {
            self.walkaway_scale * walkaway_ramp(m)
        } else {
            0.0
        }
    }

    /// The share of `kind`'s ejected stars in each channel: (encounter, supernova), whatever
    /// their mass.
    #[must_use]
    pub fn channels(self, kind: Ejected) -> (f64, f64) {
        match kind {
            Ejected::Runaway => (ENCOUNTER_SHARE, 1.0 - ENCOUNTER_SHARE),
            Ejected::Walkaway => (0.0, 1.0),
        }
    }

    /// The share of `kind`'s speeds below `v`, for a source of circular speed `v_ref`: the
    /// truncated law's CDF.
    #[must_use]
    pub fn speed_cdf(
        self,
        kind: Ejected,
        v: KilometresPerSecond,
        v_ref: KilometresPerSecond,
    ) -> f64 {
        let v = v.value();
        match kind {
            Ejected::Runaway => {
                let (least, mean) = (RUNAWAY_MIN_SPEED.value(), RUNAWAY_EXCESS_MEAN.value());
                let cut = RUNAWAY_CUT_OF_V_C * v_ref.value();
                if cut <= least {
                    // A source too slow to hold a runaway: every one sits at the least speed.
                    return if v > least { 1.0 } else { 0.0 };
                }
                let tail = |x: f64| math::exp(-(x.clamp(least, cut) - least) / mean);
                (1.0 - tail(v)) / (1.0 - tail(cut))
            }
            Ejected::Walkaway => {
                let (mode, top) = (WALKAWAY_MODE.value(), WALKAWAY_CUT.value());
                let f = |x: f64| {
                    let x = x.clamp(0.0, top);
                    1.0 - math::exp(-x * x / (2.0 * mode * mode))
                };
                f(v) / f(top)
            }
        }
    }

    /// The share of `kind`'s speeds in each speed bin, the bins' edges times `v_ref`.
    #[must_use]
    pub fn speed_bins(self, kind: Ejected, v_ref: KilometresPerSecond) -> [f64; SPEED_BINS] {
        let mut out = [0.0; SPEED_BINS];
        let mut below = 0.0;
        for (i, o) in out.iter_mut().enumerate() {
            let upper = SPEED_EDGES.get(i).map_or(1.0, |&e| {
                self.speed_cdf(kind, KilometresPerSecond::new(e * v_ref.value()), v_ref)
            });
            *o = (upper - below).max(0.0);
            below = upper;
        }
        out
    }

    /// The mean speed of `kind` in `bin`, in units of `v_ref`, or the bin's lower edge if the law
    /// puts nothing there. By a 16-node Gauss–Legendre rule on the bin's part of the law's
    /// support.
    #[must_use]
    pub fn mean_speed_in_bin(
        self,
        kind: Ejected,
        bin: SpeedBin,
        v_ref: KilometresPerSecond,
    ) -> f64 {
        let v_ref = v_ref.value();
        let (lo_law, hi_law) = match kind {
            Ejected::Runaway => (RUNAWAY_MIN_SPEED.value(), RUNAWAY_CUT_OF_V_C * v_ref),
            Ejected::Walkaway => (0.0, WALKAWAY_CUT.value()),
        };
        let index = bin.index();
        let lo_bin = if index == 0 {
            0.0
        } else {
            SPEED_EDGES[index - 1] * v_ref
        };
        let hi_bin = SPEED_EDGES.get(index).map_or(f64::INFINITY, |e| e * v_ref);
        let (a, b) = (lo_bin.max(lo_law), hi_bin.min(hi_law));
        if b <= a {
            return lo_bin / v_ref;
        }
        let cdf = |x: f64| {
            self.speed_cdf(
                kind,
                KilometresPerSecond::new(x),
                KilometresPerSecond::new(v_ref),
            )
        };
        let (mut m0, mut m1) = (0.0, 0.0);
        let h = 1e-3 * (b - a);
        for (x, w) in gl16(a, b) {
            let p = (cdf(x + h) - cdf(x - h)) / (2.0 * h);
            m0 += w * p;
            m1 += w * p * x;
        }
        if m0 > 0.0 { m1 / m0 / v_ref } else { a / v_ref }
    }

    /// The mean |z|, ly, of runaways `after` their ejection from the midplane of a source of
    /// circular speed `v_ref`, isotropic, at speeds of Design note 24's law, in the vertical force
    /// `k_z(|z|)`, (km/s)² per ly pulling towards the plane (as `MassModel::vertical_force`
    /// gives it at the site).
    ///
    /// A quadrature, not a sample: 32 Gauss–Legendre speeds over the law's support by its
    /// density and 16 directions uniform in `|cos θ|`, each orbit integrated by kick-drift-kick
    /// leapfrog in [`HEIGHT_STEPS`] steps. It reads no form (ruling 128.4).
    #[must_use]
    pub fn mean_height_after(
        self,
        v_ref: KilometresPerSecond,
        after: Years,
        k_z: impl Fn(f64) -> f64,
    ) -> f64 {
        use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S as C;
        use crate::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
        let (least, cut) = (
            RUNAWAY_MIN_SPEED.value(),
            RUNAWAY_CUT_OF_V_C * v_ref.value(),
        );
        if cut <= least {
            return 0.0;
        }
        let mean = RUNAWAY_EXCESS_MEAN.value();
        let norm = 1.0 - math::exp(-(cut - least) / mean);
        let (half, mid) = (0.5 * (cut - least), f64::midpoint(least, cut));
        let dt = after.value() / f64::from(HEIGHT_STEPS);
        let (mut weight, mut sum) = (0.0, 0.0);
        for (&x, &w) in GL32_NODES.iter().zip(GL32_WEIGHTS.iter()) {
            let v = mid + half * x;
            let density = math::exp(-(v - least) / mean) / (mean * norm);
            for (mu, wm) in gl16(0.0, 1.0) {
                let (mut z, mut vz) = (0.0_f64, v * mu);
                for _ in 0..HEIGHT_STEPS {
                    vz -= 0.5 * dt * C * k_z(z.abs()) * z.signum();
                    z += dt * C * vz;
                    vz -= 0.5 * dt * C * k_z(z.abs()) * z.signum();
                }
                let p = w * half * density * wm;
                weight += p;
                sum += p * z.abs();
            }
        }
        sum / weight
    }
}

/// The leapfrog steps of each orbit of [`RunawayModel::mean_height_after`].
pub const HEIGHT_STEPS: u32 = 2_000;

/// The nodes and weights of the 16-node Gauss–Legendre rule on `[a, b]`, from plan 01's table.
pub(crate) fn gl16(a: f64, b: f64) -> impl Iterator<Item = (f64, f64)> {
    use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
    let (half, mid) = (0.5 * (b - a), f64::midpoint(a, b));
    GL16_NODES
        .iter()
        .zip(GL16_WEIGHTS.iter())
        .map(move |(&x, &w)| (mid + half * x, w * half))
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn kms(v: f64) -> KilometresPerSecond {
        KilometresPerSecond::new(v)
    }

    #[test]
    fn shares_follow_design_note_24() {
        let m = RunawayModel::new(0.34);
        let share = |x: f64| m.runaway_share(SolarMasses::new(x));
        assert!((share(5.0) - 0.03).abs() < 1e-15);
        assert!((share(8.0) - 0.05).abs() < 1e-15);
        assert!((share(20.0) - 0.30).abs() < 1e-15);
        assert!((share(60.0) - 0.30).abs() < 1e-15);
        let mid = share(160.0_f64.sqrt());
        assert!((mid - 0.175).abs() < 1e-12, "{mid}");
        assert!(m.walkaway_share(SolarMasses::new(2.0)).abs() < 1e-15);
        // The ramp: 0.064 at 2.5 M☉, 0.199 at 4, 0.514 at 6 and 1 from 8 (ruling 128.3 rounds them
        // to 0.064, 0.20 and 0.52).
        for (mass, ramp) in [
            (2.5, 0.064),
            (4.0, 0.199),
            (6.0, 0.514),
            (8.0, 1.0),
            (40.0, 1.0),
        ] {
            let r = walkaway_ramp(SolarMasses::new(mass));
            assert!((r - ramp).abs() < 5e-4, "{mass}: {r}");
        }
        let w = m.walkaway_share(SolarMasses::new(3.0));
        assert!((w - 0.34 * walkaway_ramp(SolarMasses::new(3.0))).abs() < 1e-15);
    }

    /// With no force the mean |z| is the ballistic ⟨v⟩ t ÷ 2 (811 ly at 10 Myr for the law's
    /// 48.6 km/s mean at the fixture's 224 km/s); a pull lowers it.
    #[test]
    fn the_mean_height_is_ballistic_without_a_force() {
        use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S as C;
        let m = RunawayModel::new(0.34);
        let free = m.mean_height_after(kms(224.0), Years::new(1e7), |_| 0.0);
        let cut = 112.0_f64;
        let tail = 1.0 - math::exp(-(cut - 30.0) / 20.0);
        let mean_v = 30.0 + 20.0 - (cut - 30.0) * math::exp(-(cut - 30.0) / 20.0) / tail;
        let expected = 0.5 * mean_v * C * 1e7;
        assert!((free / expected - 1.0).abs() < 1e-6, "{free} {expected}");
        let pulled = m.mean_height_after(kms(224.0), Years::new(1e7), |z| 2e-3 * z.min(600.0));
        assert!(pulled < free, "{pulled}");
    }

    #[test]
    fn speed_bins_sum_to_one_and_respect_the_cuts() {
        let m = RunawayModel::new(0.34);
        for v_ref in [130.0, 224.0, 280.0] {
            for kind in [Ejected::Runaway, Ejected::Walkaway] {
                let bins = m.speed_bins(kind, kms(v_ref));
                assert!((bins.iter().sum::<f64>() - 1.0).abs() < 1e-12, "{bins:?}");
                assert!(bins.iter().all(|&b| b >= 0.0));
            }
            // A runaway is at most half the circular speed: nothing at or above 0.5 v_c.
            let runaway = m.speed_bins(Ejected::Runaway, kms(v_ref));
            assert!(runaway[2..].iter().all(|&b| b < 1e-15), "{runaway:?}");
            let bin = SpeedBin::new(1).expect("a bin");
            let mean = m.mean_speed_in_bin(Ejected::Runaway, bin, kms(v_ref));
            assert!((0.25..0.5).contains(&mean), "{mean}");
        }
        // The truncated Rayleigh's median: √(−200 ln(1 − (1 − e^−4.5) ÷ 2)) = 11.68 km/s.
        assert!(
            (walkaway_median() - 11.68).abs() < 0.01,
            "{}",
            walkaway_median()
        );
    }

    /// The walkaways' median speed, km/s: a Rayleigh of mode 10 cut at 30 km/s.
    fn walkaway_median() -> f64 {
        let m = RunawayModel::new(0.34);
        let (mut lo, mut hi) = (0.0, WALKAWAY_CUT.value());
        for _ in 0..60 {
            let mid = f64::midpoint(lo, hi);
            if m.speed_cdf(Ejected::Walkaway, kms(mid), kms(224.0)) < 0.5 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }
}
