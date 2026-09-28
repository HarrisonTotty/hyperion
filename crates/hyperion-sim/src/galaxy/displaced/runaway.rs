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
//! - **Runaway share** ([`RunawayModel::runaway_share`]): 0.03 below 8 M☉, rising linearly in log
//!   mass from 0.05 at 8 M☉ to 0.20 at 20 M☉ and flat above. Hoogerwerf, de Bruijne and de Zeeuw
//!   (2001, A&A 365, 49, §1) write "About 10–30% of the O stars and 5–10% of the B stars" are
//!   runaways; Design note 24's 0.03 below 8 M☉ departs from their B-star figure, and the class
//!   table's layer-D B stars come out at 0.028 runaways (P08.T9.c's test).
//! - **Walkaway share** ([`RunawayModel::walkaway_share`]): 0.10 above 2.5 M☉. Renzo et al.
//!   (2019, A&A 624, A66, §4) find most companions a supernova releases leave at under 30 km/s,
//!   for companions above 15 M☉; Design note 24 extends the share to 2.5 M☉.
//! - **Speeds**: a runaway at 30 km/s plus an exponential of mean 20 km/s, cut at half the
//!   source's circular speed; a walkaway Rayleigh with a mode of 10 km/s, cut at 30 km/s. Each cut
//!   truncates the law, which is renormalised inside it. Renzo et al.'s walkaway speeds peak near
//!   6 km/s; the mode of 10 km/s is Design note 24's.
//! - **Ejection**: half the runaways at an age uniform on 0–3 Myr (encounters); the other half, and
//!   every walkaway, at the lifetime of the companion, a primary of mass max(8 M☉, m ÷ q) with q
//!   uniform on 0.3–1 (supernova release).

use super::{SPEED_BINS, SPEED_EDGES, SpeedBin};
use crate::math;
use crate::units::{KilometresPerSecond, SolarMasses, Years};

/// The runaway share of living stars below 8 M☉ (Design note 24; Hoogerwerf et al. 2001 give 5–10%
/// of B stars).
pub const RUNAWAY_SHARE_LOW: f64 = 0.03;

/// The runaway share at 8 M☉, where the rise in log mass begins.
pub const RUNAWAY_SHARE_AT_8: f64 = 0.05;

/// The runaway share at and above 20 M☉ (Hoogerwerf et al. 2001: 10–30% of O stars).
pub const RUNAWAY_SHARE_HIGH: f64 = 0.20;

/// The mass where the runaway share's rise begins.
pub const RUNAWAY_RISE_START: SolarMasses = SolarMasses::new(8.0);

/// The mass where the runaway share's rise ends.
pub const RUNAWAY_RISE_END: SolarMasses = SolarMasses::new(20.0);

/// The walkaway share of living stars above [`WALKAWAY_MIN_MASS`] (Renzo et al. 2019, §4).
pub const WALKAWAY_SHARE: f64 = 0.10;

/// The least mass of a walkaway: lighter released companions are left out (brainstorm,
/// "Displaced objects: kicks and runaways").
pub const WALKAWAY_MIN_MASS: SolarMasses = SolarMasses::new(2.5);

/// A runaway's least speed.
pub const RUNAWAY_MIN_SPEED: KilometresPerSecond = KilometresPerSecond::new(30.0);

/// The mean of a runaway's speed above [`RUNAWAY_MIN_SPEED`].
pub const RUNAWAY_EXCESS_MEAN: KilometresPerSecond = KilometresPerSecond::new(20.0);

/// A runaway's speed is cut at this fraction of its source's circular speed.
pub const RUNAWAY_CUT_OF_V_C: f64 = 0.5;

/// The mode (the Rayleigh σ) of a walkaway's speed.
pub const WALKAWAY_MODE: KilometresPerSecond = KilometresPerSecond::new(10.0);

/// A walkaway's speed is cut here.
pub const WALKAWAY_CUT: KilometresPerSecond = KilometresPerSecond::new(30.0);

/// The share of runaways ejected by encounters; the rest are released by a supernova.
pub const ENCOUNTER_SHARE: f64 = 0.5;

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

/// Plan 08's Design note 24 as functions of mass and speed (module documentation). The model has
/// no state: its constants are the module's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RunawayModel;

impl RunawayModel {
    /// The share of living stars of initial mass `m` that are runaways, once ejected.
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

    /// The share of living stars of initial mass `m` that are walkaways, once released.
    #[must_use]
    pub fn walkaway_share(self, m: SolarMasses) -> f64 {
        if m.value() > WALKAWAY_MIN_MASS.value() {
            WALKAWAY_SHARE
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
}

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
        let m = RunawayModel;
        let share = |x: f64| m.runaway_share(SolarMasses::new(x));
        assert!((share(5.0) - 0.03).abs() < 1e-15);
        assert!((share(8.0) - 0.05).abs() < 1e-15);
        assert!((share(20.0) - 0.20).abs() < 1e-15);
        assert!((share(60.0) - 0.20).abs() < 1e-15);
        let mid = share(160.0_f64.sqrt());
        assert!((mid - 0.125).abs() < 1e-12, "{mid}");
        assert!(m.walkaway_share(SolarMasses::new(2.0)).abs() < 1e-15);
        assert!((m.walkaway_share(SolarMasses::new(3.0)) - 0.10).abs() < 1e-15);
    }

    #[test]
    fn speed_bins_sum_to_one_and_respect_the_cuts() {
        let m = RunawayModel;
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
        let m = RunawayModel;
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
