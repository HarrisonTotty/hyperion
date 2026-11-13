//! The 5% metric's mirror, for R08.T13's sweep (plan R08, R08.T13.a; Design note 10;
//! `decision-r08-t13-scope.md`, ruling 6): the part of the client's `view/atmosphere/gate.ts` that
//! the sweep's walk decides by, in one channel.
//!
//! For each geometry g of a traced point, held against the twin's radiance `L_twin`:
//!
//! - its **image** I(g), the geometries that share its observer and its sun, and `L_max` the
//!   image's brightest reference radiance (`gate.ts`'s `imageMaxima`);
//! - the tolerance `T = max(0.05 L_ref, F L_max)`, F = 3 × 10⁻⁴ (`metricTolerance`);
//! - it **passes** where `|L_twin − L_ref| ≤ T + 4σ_ref`;
//! - the reference is **refused** there as `noisy` where `σ_ref > T ÷ 8`, or else as `unscaled`
//!   where `σ_ref` stands more than 25% from its first quarter's `σ_q √(N_q ÷ N)`, a σ exact at
//!   both counts (0 at each) scaling (`refusals`, `sigmaScales`).
//!
//! The constants have `gate.ts`'s names and values. `metric_verdicts.json`, written by the client's
//! `reference/metricVerdicts.test.ts` from `gate.ts` itself, holds this mirror to the client's
//! verdicts and constants, verdict for verdict; R08.T13.c2 recomputes every cell from the sweep's
//! outputs with `gate.ts` (ruling 6).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The metric's relative tolerance: 5% of the reference's radiance (`gate.ts`'s
/// `METRIC_RELATIVE_TOLERANCE`).
pub const METRIC_RELATIVE_TOLERANCE: f64 = 0.05;

/// The metric's floor, F: 3 × 10⁻⁴ of the image's brightest diffuse radiance (`METRIC_FLOOR`).
pub const METRIC_FLOOR: f64 = 3e-4;

/// The reference's standard errors the tolerance allows: 4 (`METRIC_SIGMA_ALLOWANCE`).
pub const METRIC_SIGMA_ALLOWANCE: f64 = 4.0;

/// The reference is refused where its σ exceeds T over this: 8 (`METRIC_SIGMA_DIVISOR`).
pub const METRIC_SIGMA_DIVISOR: f64 = 8.0;

/// The largest σ an aggregate's reference may have, relative to its flux: 0.25%
/// (`AGGREGATE_SIGMA_CAP`).
pub const AGGREGATE_SIGMA_CAP: f64 = 0.0025;

/// How far a σ may stand from its first quarter's `σ_q √(N_q ÷ N)`, relatively: 25%
/// (`SIGMA_SCALING_TOLERANCE`).
pub const SIGMA_SCALING_TOLERANCE: f64 = 0.25;

/// One geometry of a traced point, in one channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading<'a> {
    /// I(g), the image it is held in.
    pub image: &'a str,
    /// `L_ref`, the reference's diffuse radiance.
    pub radiance: f64,
    /// `σ_ref` over the whole run.
    pub standard_error: f64,
    /// N.
    pub samples: u64,
    /// `N_q`, the first quarter's samples.
    pub quarter_samples: u64,
    /// `σ_q` over the first quarter.
    pub quarter_standard_error: f64,
}

/// Why the reference is refused at a geometry (`gate.ts`'s `RefusalReason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Refusal {
    /// `σ_ref > T ÷ 8`.
    Noisy,
    /// `σ_ref` not within 25% of `σ_q √(N_q ÷ N)`.
    Unscaled,
}

/// The metric's verdict at one geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    /// T.
    pub tolerance: f64,
    /// `T + 4σ_ref`.
    pub bound: f64,
    /// `|L_twin − L_ref| ≤ T + 4σ_ref`.
    pub passes: bool,
    /// Where the reference is refused, and why: `noisy` first where both hold.
    pub refusal: Option<Refusal>,
}

/// `T = max(0.05 L_ref, F L_max)`: the metric's tolerance at a radiance in an image.
#[must_use]
pub fn metric_tolerance(reference_radiance: f64, image_maximum: f64) -> f64 {
    (METRIC_RELATIVE_TOLERANCE * reference_radiance).max(METRIC_FLOOR * image_maximum)
}

/// `n` as an `f64`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a sample count stays below 2⁵³, where every integer is an exact f64"
)]
#[must_use]
fn count(n: u64) -> f64 {
    n as f64
}

/// `σ_q √(N_q ÷ N)`, the σ a first quarter predicts.
#[must_use]
pub fn predicted_standard_error(reading: &Reading<'_>) -> f64 {
    reading.quarter_standard_error
        * (count(reading.quarter_samples) / count(reading.samples)).sqrt()
}

/// Whether a standard error falls as N^(−1/2) from its first quarter's: within 25% of
/// `σ_q √(N_q ÷ N)`, a value exact at both counts doing so.
#[must_use]
pub fn sigma_scales(reading: &Reading<'_>) -> bool {
    let sigma = reading.standard_error;
    // Standard errors are not negative, so this is `gate.ts`'s `=== 0` on both, NaN included.
    if sigma <= 0.0 && reading.quarter_standard_error <= 0.0 {
        return true;
    }
    let expected = predicted_standard_error(reading);
    (sigma - expected).abs() <= SIGMA_SCALING_TOLERANCE * expected
}

/// Each image's brightest reference radiance, `L_max(I)`.
#[must_use]
pub fn image_maxima<'a>(readings: &[Reading<'a>]) -> BTreeMap<&'a str, f64> {
    let mut maxima = BTreeMap::new();
    for r in readings {
        let seen = maxima.entry(r.image).or_insert(0.0_f64);
        *seen = seen.max(r.radiance);
    }
    maxima
}

/// The metric's verdict at each reading, the twin's radiance there `twin[i]`.
///
/// # Panics
///
/// If `twin` does not give one radiance a reading, a caller's bug.
#[must_use]
pub fn verdicts(readings: &[Reading<'_>], twin: &[f64]) -> Vec<Verdict> {
    assert_eq!(readings.len(), twin.len(), "one twin radiance a reading");
    let maxima = image_maxima(readings);
    readings
        .iter()
        .zip(twin)
        .map(|(r, &client)| {
            let maximum = maxima.get(r.image).copied().unwrap_or(0.0);
            let tolerance = metric_tolerance(r.radiance, maximum);
            let bound = tolerance + METRIC_SIGMA_ALLOWANCE * r.standard_error;
            let refusal = if r.standard_error > tolerance / METRIC_SIGMA_DIVISOR {
                Some(Refusal::Noisy)
            } else if sigma_scales(r) {
                None
            } else {
                Some(Refusal::Unscaled)
            };
            Verdict {
                tolerance,
                bound,
                passes: (client - r.radiance).abs() <= bound,
                refusal,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// `metric_verdicts.json` (module documentation).
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct SharedValues {
        description: String,
        constants: Constants,
        cases: Vec<SharedCase>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Constants {
        relative_tolerance: f64,
        floor: f64,
        sigma_allowance: f64,
        sigma_divisor: f64,
        sigma_scaling_tolerance: f64,
        aggregate_sigma_cap: f64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct SharedCase {
        name: String,
        geometries: Vec<SharedGeometry>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct SharedGeometry {
        name: String,
        image: String,
        radiance: f64,
        standard_error: f64,
        samples: u64,
        quarter_samples: u64,
        quarter_standard_error: f64,
        twin: f64,
        tolerance: f64,
        bound: f64,
        passes: bool,
        refusal: Option<String>,
    }

    const SHARED: &str = include_str!("metric_verdicts.json");

    #[test]
    fn atmosphere_metric_constants_are_the_clients() {
        let shared: SharedValues = serde_json::from_str(SHARED).unwrap();
        assert!(shared.description.contains("metricVerdicts.test.ts"));
        let c = &shared.constants;
        for (ours, theirs, name) in [
            (
                METRIC_RELATIVE_TOLERANCE,
                c.relative_tolerance,
                "relative tolerance",
            ),
            (METRIC_FLOOR, c.floor, "floor"),
            (METRIC_SIGMA_ALLOWANCE, c.sigma_allowance, "σ allowance"),
            (METRIC_SIGMA_DIVISOR, c.sigma_divisor, "σ divisor"),
            (
                SIGMA_SCALING_TOLERANCE,
                c.sigma_scaling_tolerance,
                "σ scaling",
            ),
            (
                AGGREGATE_SIGMA_CAP,
                c.aggregate_sigma_cap,
                "aggregate σ cap",
            ),
        ] {
            assert!(
                ours.total_cmp(&theirs).is_eq(),
                "{name}: {ours} against {theirs}"
            );
        }
    }

    #[test]
    fn atmosphere_metric_gives_the_clients_verdicts() {
        let shared: SharedValues = serde_json::from_str(SHARED).unwrap();
        let mut compared = 0;
        for case in &shared.cases {
            let readings: Vec<Reading<'_>> = case
                .geometries
                .iter()
                .map(|g| Reading {
                    image: &g.image,
                    radiance: g.radiance,
                    standard_error: g.standard_error,
                    samples: g.samples,
                    quarter_samples: g.quarter_samples,
                    quarter_standard_error: g.quarter_standard_error,
                })
                .collect();
            let twin: Vec<f64> = case.geometries.iter().map(|g| g.twin).collect();
            for (g, v) in case.geometries.iter().zip(verdicts(&readings, &twin)) {
                let label = format!("{}, {}", case.name, g.name);
                assert!(v.tolerance.total_cmp(&g.tolerance).is_eq(), "{label}: T");
                assert!(v.bound.total_cmp(&g.bound).is_eq(), "{label}: T + 4σ");
                assert_eq!(v.passes, g.passes, "{label}: the verdict");
                let refusal = v.refusal.map(|r| match r {
                    Refusal::Noisy => "noisy",
                    Refusal::Unscaled => "unscaled",
                });
                assert_eq!(refusal, g.refusal.as_deref(), "{label}: the refusal");
                compared += 1;
            }
        }
        assert!(compared >= 20, "{compared} comparisons");
    }
}
