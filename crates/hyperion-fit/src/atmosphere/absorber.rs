//! An absorber's curve of growth as the tracer reads it (plan R08, R08.T12.b.1; Design note 5,
//! "Curves of growth"): one row of the client's `AbsorberCurve`, read by the client's own rule.
//!
//! An absorber term absorbs by the transmittance its curve gives the column u = n ∫ρ ds a path
//! crosses, not by a coefficient: per channel and sun, T(u) is a band's weighted mean of e^(−σu)
//! (Design note 5), which is not a product over a path's legs, so the estimator keeps each
//! absorber's column along a path and applies T once, to the whole path's column, at each
//! next-event score (the `transport` module). "A bake that follows paths … applies it to each
//! path's column" (Design note 5). For a band average, each next-event score being one whole path
//! from the detector to the sun, it is the exact equivalent-path result where three things hold:
//! every other term's extinction, absorption and phase and the ground's albedo are grey within
//! the channel, under the band weight of the sun's light and the channel's response (the curves are
//! one a sun); the curve is the same all along the path (one pressure row, which is all a case
//! carries yet; with pressure rows it needs a correlated-k or Curtis–Godson assumption besides);
//! and, for several absorbers, their spectra are uncorrelated under that weight, since their
//! transmittances multiply (the multiplication property, an approximation for smooth continua
//! across a broad channel). A product of the legs' transmittances is not exact even for one
//! absorber: two factors that both fall with σ are correlated, and their product is too low.
//!
//! The read is `view/atmosphere/absorbers.ts`'s `absorberTransmittance` on a curve of one pressure
//! row, which is `depthAt` (module-private there) and `clampedTransmittance`:
//!
//! - the optical depth τ at x = ln(u ÷ 1 m⁻²) is linear in ln τ against x between two nodes, and
//!   linear in τ where either node's τ is 0; proportional to u below the first node (the linear
//!   regime, τ₀ e^(x − x₀)); and the last interval's power law beyond the last node;
//! - T = e^(−τ), clamped to [[`TRANSMITTANCE_FLOOR`], 1]; T is exactly 1 at u = 0.
//!
//! A curve of one row has no ceiling on τ: the client's `CURVE_DEPTH_MAX` acts only between the
//! rows of a curve with pressure rows, which a case does not carry yet (R08.T12.b.2), so the floor
//! alone bounds T. `absorber_curves.json`, written by the client's `absorberCurves.test.ts`, holds
//! this read to the client's to 10⁻¹².

use hyperion_sim::math;

/// The least transmittance a curve gives: 10⁻⁶, the client's `ABSORBER_TRANSMITTANCE_FLOOR`
/// (`view/atmosphere/absorbers.ts`; Design note 5's clamp to [10⁻⁶, 1]).
pub(crate) const TRANSMITTANCE_FLOOR: f64 = 1e-6;

/// The most absorber terms a case may have: 5, the client's `ABSORBER_COLUMNS`, the absorber
/// columns its per-planet tables store, so that a path carries a fixed array of columns.
pub(crate) const MAX_ABSORBERS: usize = 5;

/// One row of a curve of growth at one wavelength and sun: ln u at each node, ascending, and the
/// optical depth −ln T there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CurveRow<'a> {
    log_columns: &'a [f64],
    depths: &'a [f64],
}

impl<'a> CurveRow<'a> {
    /// The row of `depths` on `log_columns`, which the case has checked: at least two nodes,
    /// ln u finite and strictly ascending, as many depths as nodes, each finite and not negative.
    #[must_use]
    pub(crate) fn new(log_columns: &'a [f64], depths: &'a [f64]) -> Self {
        debug_assert!(
            log_columns.len() >= 2 && log_columns.len() == depths.len(),
            "a curve row has at least two nodes and a depth each"
        );
        Self {
            log_columns,
            depths,
        }
    }

    /// The optical depth at x = ln(u ÷ 1 m⁻²), by the client's `depthAt` (module documentation).
    #[must_use]
    pub(crate) fn depth_at(&self, ln_u: f64) -> f64 {
        let last = self.log_columns.len() - 1;
        let first = self.log_columns[0];
        if ln_u <= first {
            return self.depths[0] * math::exp(ln_u - first);
        }
        // The client's interval is the first one below the last whose upper node is at or past
        // ln u, or the last: with ln u strictly ascending, the number of interior nodes below it.
        let interval = self.log_columns[1..last].partition_point(|&node| node < ln_u);
        let (ln_low, ln_high) = (self.log_columns[interval], self.log_columns[interval + 1]);
        let (low, high) = (self.depths[interval], self.depths[interval + 1]);
        let fraction = (ln_u - ln_low) / (ln_high - ln_low);
        if low > 0.0 && high > 0.0 {
            let (ln_depth_low, ln_depth_high) = (math::ln(low), math::ln(high));
            math::exp(ln_depth_low + (ln_depth_high - ln_depth_low) * fraction)
        } else {
            low + (high - low) * fraction
        }
    }

    /// The transmittance through the column `column_per_m2`, in the curve's unit (molecules m⁻²),
    /// at least 0: 1 at 0, and otherwise e^(−τ) clamped to [[`TRANSMITTANCE_FLOOR`], 1].
    #[must_use]
    pub(crate) fn transmittance(&self, column_per_m2: f64) -> f64 {
        if column_per_m2 <= 0.0 {
            return 1.0;
        }
        math::exp(-self.depth_at(math::ln(column_per_m2))).clamp(TRANSMITTANCE_FLOOR, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// `absorber_curves.json`: curves as the client writes them and the client's own
    /// `absorberTransmittance` at columns from 0 past the last node.
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct SharedValues {
        description: String,
        curves: Vec<SharedCurve>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct SharedCurve {
        name: String,
        log_columns: Vec<f64>,
        /// Per channel.
        optical_depth: Vec<Vec<f64>>,
        columns_per_m2: Vec<f64>,
        /// Per channel, at each column.
        transmittance: Vec<Vec<f64>>,
    }

    const SHARED: &str = include_str!("absorber_curves.json");

    #[test]
    fn atmosphere_absorber_curves_read_as_the_clients_absorber_transmittance() {
        let shared: SharedValues = serde_json::from_str(SHARED).unwrap();
        assert!(shared.description.contains("absorberCurves.test.ts"));
        assert!(
            shared.curves.len() >= 2,
            "Earth's ozone and a saturated curve"
        );
        let mut largest = 0.0_f64;
        let mut floored = 0;
        let mut beyond = 0;
        for curve in &shared.curves {
            let last = *curve.log_columns.last().unwrap();
            for (c, (depths, expected)) in curve
                .optical_depth
                .iter()
                .zip(&curve.transmittance)
                .enumerate()
            {
                let row = CurveRow::new(&curve.log_columns, depths);
                for (&u, &t) in curve.columns_per_m2.iter().zip(expected) {
                    let read = row.transmittance(u);
                    let difference = (read - t).abs() / t;
                    assert!(
                        difference <= 1e-12,
                        "{}, channel {c}, u {u:e}: {read:e} against the client's {t:e}",
                        curve.name
                    );
                    largest = largest.max(difference);
                    if (t - TRANSMITTANCE_FLOOR).abs() < f64::EPSILON * TRANSMITTANCE_FLOOR {
                        floored += 1;
                    }
                    if u > 0.0 && math::ln(u) > last {
                        beyond += 1;
                    }
                }
            }
            assert!(
                curve.columns_per_m2.first().is_some_and(|&u| u <= 0.0),
                "{}: from u = 0",
                curve.name
            );
        }
        assert!(floored > 0, "some read reaches the floor");
        assert!(beyond > 0, "some read lies past the last node");
        eprintln!(
            "absorber curves: the largest relative difference from the client's read {largest:e}"
        );
    }

    #[test]
    fn atmosphere_a_curve_row_reads_its_regimes() {
        // τ = 2 × 10⁻²⁰ u on four nodes a decade apart: linear in u everywhere, so the log–log
        // read, the linear regime below the first node and the power law past the last are exact.
        let decades = [1.0, 10.0, 100.0, 1000.0];
        let log_columns: Vec<f64> = decades.iter().map(|d| math::ln(1e19 * d)).collect();
        let depths: Vec<f64> = decades.iter().map(|d| 0.2 * d).collect();
        let row = CurveRow::new(&log_columns, &depths);
        for u in [1e15, 1e19, 3.3e19, 7e20, 1e22, 2e22] {
            let expected = math::exp(-2e-20 * u).max(TRANSMITTANCE_FLOOR);
            assert!(
                (row.transmittance(u) / expected - 1.0).abs() < 1e-12,
                "{u:e}"
            );
        }
        assert!((row.transmittance(0.0) - 1.0).abs() < f64::MIN_POSITIVE);
        // Where a node's τ is 0, linear in τ: half way between 0 and 0.4 in ln u.
        let flat = [0.0, 0.4];
        let row = CurveRow::new(&log_columns[..2], &flat);
        let middle = math::exp(f64::midpoint(log_columns[0], log_columns[1]));
        assert!((row.transmittance(middle) - math::exp(-0.2)).abs() < 1e-15);
        // Deep past the floor.
        assert!((row.transmittance(1e80) - TRANSMITTANCE_FLOOR).abs() < f64::MIN_POSITIVE);
    }
}
