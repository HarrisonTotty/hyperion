//! An absorber's curve of growth as the tracer reads it (plan R08, R08.T12.b; Design note 5,
//! "Curves of growth"): the client's `AbsorberCurve`, read by the client's own rule.
//!
//! An absorber term absorbs by the transmittance its curve gives the amount u = n ∫ρ ds a path
//! crosses, not by a coefficient: per channel and sun, T(u) is a band's weighted mean of e^(−σu)
//! (Design note 5), which is not a product over a path's legs, so the estimator keeps each
//! absorber's amount along a path and applies T once, to the whole path's amount, at each
//! next-event score (the `transport` module). "A bake that follows paths … applies it to each
//! path's column" (Design note 5). For a band average, each next-event score being one whole path
//! from the detector to the sun, it is the exact equivalent-path result where three things hold:
//! every other term's extinction, absorption and phase and the ground's albedo are grey within
//! the channel, under the band weight of the sun's light and the channel's response (the curves are
//! one a sun); the curve is the same all along the path (one pressure row), or, for a curve with
//! pressure rows (R08.T12.b.2; R08.T4.d's line absorbers), the path's lines are those of one
//! homogeneous path at its absorber-weighted pressure (the Curtis–Godson approximation; A. R.
//! Curtis 1952, QJRMS 78, 638; W. L. Godson 1953, QJRMS 79, 367), as the client's tables and twin
//! read every path, here over the whole path's amount; and, for several absorbers, their spectra
//! are uncorrelated under that weight, since their transmittances multiply (the multiplication
//! property, an approximation for smooth continua across a broad channel). A product of the legs'
//! transmittances is not exact even for one absorber: two factors that both fall with σ are
//! correlated, and their product is too low.
//!
//! The amount is of the absorber's kind (the client's `AbsorberAmount`): a column absorber's
//! column, molecules m⁻², or a pair absorber's pair column, molecule pairs m⁻⁵ (R08.T4.c), whose
//! term's relative density is already the square of its column's. The read is the same for both.
//!
//! The read is `view/atmosphere/absorbers.ts`'s `absorberTransmittance`, which is `depthAt` and
//! `depthAtPressure` (module-private there) and `clampedTransmittance`:
//!
//! - on each row, the optical depth τ at x = ln(u ÷ 1 unit) is linear in ln τ against x between
//!   two nodes, and linear in τ where either node's τ is 0; proportional to u below the first node
//!   (the linear regime, τ₀ e^(x − x₀)); and the last interval's power law beyond the last node;
//! - on a curve of one row, that is τ; on a curve of several, the pressure y = ln p̄ is held to
//!   [the first row, the lesser of the last row and the read's top], and τ is the 4-point Lagrange
//!   interpolant in ln p of the four rows nearest y (their stencil starting a row below the row at
//!   or below y, kept inside the rows), or linear in ln p between the two rows about y on a curve of
//!   under four rows, then clamped to [0, [`CURVE_DEPTH_MAX`]];
//! - T = e^(−τ), clamped to [[`TRANSMITTANCE_FLOOR`], 1]; T is exactly 1 at u = 0.
//!
//! A curve of one row has no ceiling on τ: the client's `CURVE_DEPTH_MAX` acts only on a curve
//! with pressure rows, so the floor alone bounds T. `absorber_curves.json`, written by the client's
//! `absorberCurves.test.ts`, holds this read to the client's to 10⁻¹².

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

    /// The transmittance through the amount `amount`, in the curve's unit (molecules m⁻², or
    /// molecule pairs m⁻⁵), at least 0: 1 at 0, and otherwise e^(−τ) clamped to
    /// [[`TRANSMITTANCE_FLOOR`], 1].
    #[must_use]
    pub(crate) fn transmittance(&self, amount: f64) -> f64 {
        if amount <= 0.0 {
            return 1.0;
        }
        math::exp(-self.depth_at(math::ln(amount))).clamp(TRANSMITTANCE_FLOOR, 1.0)
    }
}

/// The largest optical depth a curve with pressure rows reads, −ln [`TRANSMITTANCE_FLOOR`]: the
/// client's `CURVE_DEPTH_MAX` (`view/atmosphere/absorbers.ts`).
pub(crate) const CURVE_DEPTH_MAX: f64 = 13.815_510_557_964_274;

/// The pressure the pressure-weighted amounts are taken over, Pa: 10⁵, 1 bar, the client's
/// `ABSORBER_PRESSURE_REFERENCE_PA` (`view/atmosphere/medium.ts`), the unit of a case's
/// `pressureBar`.
pub(crate) const PRESSURE_REFERENCE_PA: f64 = 1e5;

/// The least absorber-weighted pressure a path is read at, Pa: 10⁻³, the client's
/// `ABSORBER_PRESSURE_FLOOR_PA`, far under any curve's first row (10 Pa ÷ f), so that a path whose
/// pressure-weighted amount is 0 reads at that row rather than at ln 0.
pub(crate) const PRESSURE_FLOOR_PA: f64 = 1e-3;

/// A curve of growth at one wavelength and sun: its rows, one per pressure (R08.T12.b.2; one for a
/// curve without lines), on one set of nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Curve<'a> {
    log_columns: &'a [f64],
    /// ln(p ÷ 1 Pa) at each row, strictly ascending; a curve of one row reads none.
    log_pressures: &'a [f64],
    /// ln(p ÷ 1 Pa) above which the read holds p̄; a curve of one row reads none.
    read_top_log_pressure: f64,
    /// −ln T at each row and node, row-major, `[r × nodes + j]`.
    depths: &'a [f64],
}

impl<'a> Curve<'a> {
    /// The curve of `depths` on `log_columns` and `log_pressures`, which the case has checked: at
    /// least two nodes, ln u finite and strictly ascending, ln p finite and strictly ascending (or
    /// of at most one row), and one finite depth at least 0 per row and node.
    #[must_use]
    pub(crate) fn new(
        log_columns: &'a [f64],
        log_pressures: &'a [f64],
        read_top_log_pressure: f64,
        depths: &'a [f64],
    ) -> Self {
        debug_assert!(
            depths.len() == log_columns.len() * log_pressures.len().max(1),
            "a curve has a depth per row and node"
        );
        Self {
            log_columns,
            log_pressures,
            read_top_log_pressure,
            depths,
        }
    }

    /// How many pressure rows the curve has: 1 for one without lines.
    #[must_use]
    pub(crate) fn rows(&self) -> usize {
        self.log_pressures.len().max(1)
    }

    /// Whether its read takes a path's pressure: a curve of several rows.
    #[must_use]
    pub(crate) fn reads_pressure(&self) -> bool {
        self.rows() > 1
    }

    /// Row `r`.
    fn row(&self, r: usize) -> CurveRow<'a> {
        let nodes = self.log_columns.len();
        CurveRow::new(self.log_columns, &self.depths[r * nodes..(r + 1) * nodes])
    }

    /// The optical depth at x = ln(u ÷ 1 unit) and the absorber-weighted pressure `pressure_pa`,
    /// Pa, above 0 (module documentation): row 0's on a curve of one row, which reads no pressure.
    #[must_use]
    pub(crate) fn depth_at(&self, ln_u: f64, pressure_pa: f64) -> f64 {
        let rows = self.rows();
        if rows == 1 {
            return self.row(0).depth_at(ln_u);
        }
        let first = self.log_pressures[0];
        let top = self.log_pressures[rows - 1].min(self.read_top_log_pressure);
        let y = math::ln(pressure_pa).max(first).min(top);
        // The client's row: the last below the second-to-last whose successor lies below y.
        let mut r = 0;
        while r < rows - 2 && self.log_pressures[r + 1] < y {
            r += 1;
        }
        let depth = if rows < 4 {
            let (low, high) = (self.log_pressures[r], self.log_pressures[r + 1]);
            let (a, b) = (self.row(r).depth_at(ln_u), self.row(r + 1).depth_at(ln_u));
            a + ((b - a) * (y - low)) / (high - low)
        } else {
            let start = r.saturating_sub(1).min(rows - 4);
            let stencil = start..start + 4;
            let mut depth = 0.0;
            for k in stencil.clone() {
                let xk = self.log_pressures[k];
                let mut weight = 1.0;
                for m in stencil.clone() {
                    if m != k {
                        weight *= (y - self.log_pressures[m]) / (xk - self.log_pressures[m]);
                    }
                }
                depth += weight * self.row(k).depth_at(ln_u);
            }
            depth
        };
        depth.clamp(0.0, CURVE_DEPTH_MAX)
    }

    /// The transmittance through the amount `amount`, in the curve's unit, at least 0, at the
    /// absorber-weighted pressure `pressure_pa`, Pa (read only on a curve of several rows): 1 at 0,
    /// and otherwise e^(−τ) clamped to [[`TRANSMITTANCE_FLOOR`], 1]. A curve of one row reads as
    /// its [`CurveRow`] does, bit for bit.
    #[must_use]
    pub(crate) fn transmittance(&self, amount: f64, pressure_pa: f64) -> f64 {
        if self.rows() == 1 {
            return self.row(0).transmittance(amount);
        }
        if amount <= 0.0 {
            return 1.0;
        }
        math::exp(-self.depth_at(math::ln(amount), pressure_pa)).clamp(TRANSMITTANCE_FLOOR, 1.0)
    }
}

/// A path's absorber-weighted (Curtis–Godson) pressure, Pa, from its amount u and its
/// pressure-weighted amount `w = n ∫ρ (p ÷ p_ref) ds`, both in the absorber's unit: `p_ref w ÷ u`, at
/// least [`PRESSURE_FLOOR_PA`] (the client's tables' and twin's reading). `amount` is above 0.
#[must_use]
pub(crate) fn weighted_pressure_pa(amount: f64, weighted: f64) -> f64 {
    (PRESSURE_REFERENCE_PA * weighted / amount).max(PRESSURE_FLOOR_PA)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// `absorber_curves.json`: curves as the client writes them and the client's own
    /// `absorberTransmittance` at amounts from 0 past the last node and, for a curve with
    /// pressure rows, at pressures about and past its rows.
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
        /// `column` or `pair`, which reads alike.
        kind: String,
        log_columns: Vec<f64>,
        log_pressures: Vec<f64>,
        read_top_log_pressure: f64,
        /// Per channel, row-major.
        optical_depth: Vec<Vec<f64>>,
        /// The amounts read, in the curve's unit.
        amounts: Vec<f64>,
        /// The absorber-weighted pressure of each read, Pa, for a curve with pressure rows.
        #[serde(default)]
        pressures_pa: Option<Vec<f64>>,
        /// Per channel, at each read.
        transmittance: Vec<Vec<f64>>,
    }

    const SHARED: &str = include_str!("absorber_curves.json");

    #[test]
    fn atmosphere_absorber_curves_read_as_the_clients_absorber_transmittance() {
        let shared: SharedValues = serde_json::from_str(SHARED).unwrap();
        assert!(shared.description.contains("absorberCurves.test.ts"));
        let mut largest = 0.0_f64;
        let (mut floored, mut beyond, mut ceiled, mut held) = (0, 0, 0, 0);
        let mut kinds = std::collections::BTreeSet::new();
        let mut rows = std::collections::BTreeSet::new();
        for curve in &shared.curves {
            kinds.insert(curve.kind.as_str());
            rows.insert(curve.log_pressures.len());
            let last = *curve.log_columns.last().unwrap();
            let pressures = curve
                .pressures_pa
                .clone()
                .unwrap_or_else(|| vec![f64::NAN; curve.amounts.len()]);
            assert_eq!(pressures.len(), curve.amounts.len(), "{}", curve.name);
            for (c, (depths, expected)) in curve
                .optical_depth
                .iter()
                .zip(&curve.transmittance)
                .enumerate()
            {
                let read = Curve::new(
                    &curve.log_columns,
                    &curve.log_pressures,
                    curve.read_top_log_pressure,
                    depths,
                );
                assert_eq!(
                    read.reads_pressure(),
                    curve.pressures_pa.is_some(),
                    "{}",
                    curve.name
                );
                for ((&u, &p), &t) in curve.amounts.iter().zip(&pressures).zip(expected) {
                    let transmittance = read.transmittance(u, p);
                    let difference = (transmittance - t).abs() / t;
                    assert!(
                        difference <= 1e-12,
                        "{}, channel {c}, u {u:e}, p {p:e}: {transmittance:e} against the \
                         client's {t:e}",
                        curve.name
                    );
                    largest = largest.max(difference);
                    if (t - TRANSMITTANCE_FLOOR).abs() < f64::EPSILON * TRANSMITTANCE_FLOOR {
                        floored += 1;
                    }
                    if u > 0.0 && math::ln(u) > last {
                        beyond += 1;
                    }
                    if read.reads_pressure() && u > 0.0 {
                        let depth = read.depth_at(math::ln(u), p);
                        if (depth - CURVE_DEPTH_MAX).abs() < f64::MIN_POSITIVE {
                            ceiled += 1;
                        }
                        let first = curve.log_pressures[0];
                        let top = curve
                            .log_pressures
                            .last()
                            .unwrap()
                            .min(curve.read_top_log_pressure);
                        if !(first..=top).contains(&math::ln(p)) {
                            held += 1;
                        }
                    }
                }
            }
            assert!(
                curve.amounts.first().is_some_and(|&u| u <= 0.0),
                "{}: from u = 0",
                curve.name
            );
        }
        assert!(floored > 0, "some read reaches the floor");
        assert!(beyond > 0, "some read lies past the last node");
        assert!(
            ceiled > 0,
            "some read of a curve with pressure rows reaches its ceiling"
        );
        assert!(held > 0, "some read's pressure lies past its rows");
        assert_eq!(kinds.into_iter().collect::<Vec<_>>(), ["column", "pair"]);
        assert!(
            rows.contains(&1)
                && rows.contains(&2)
                && rows.contains(&3)
                && rows.iter().any(|&r| r >= 4),
            "curves of one, two, three and four or more rows: {rows:?}"
        );
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

    #[test]
    fn atmosphere_a_curve_with_pressure_rows_reads_its_regimes() {
        assert!((CURVE_DEPTH_MAX + math::ln(TRANSMITTANCE_FLOOR)).abs() < 4.0 * f64::EPSILON);
        // τ = 10⁻²⁰ u (1 + y ÷ 10) at y = ln p, linear in ln p: the 4-point Lagrange read and the
        // two-row linear one are exact, and the pressure is held at the first row below it and
        // at the read's top above it.
        let log_columns: Vec<f64> = [1.0, 10.0, 100.0]
            .iter()
            .map(|d| math::ln(1e19 * d))
            .collect();
        let log_pressures = [2.0, 5.0, 8.0, 11.0, 14.0];
        let tau = |u: f64, y: f64| 1e-20 * u * (1.0 + y / 10.0);
        let depths: Vec<f64> = log_pressures
            .iter()
            .flat_map(|&y| log_columns.iter().map(move |&x| tau(math::exp(x), y)))
            .collect();
        let curve = Curve::new(&log_columns, &log_pressures, 12.5, &depths);
        assert!(curve.reads_pressure());
        let u = 3e19;
        for (y, held) in [
            (4.0, 4.0),
            (9.5, 9.5),
            (12.0, 12.0),
            (1.0, 2.0),
            (13.5, 12.5),
        ] {
            let expected = math::exp(-tau(u, held));
            assert!(
                (curve.transmittance(u, math::exp(y)) / expected - 1.0).abs() < 1e-13,
                "ln p {y}"
            );
        }
        let two = Curve::new(&log_columns, &log_pressures[..2], 6.0, &depths[..6]);
        let expected = math::exp(-tau(u, 3.5));
        assert!((two.transmittance(u, math::exp(3.5)) / expected - 1.0).abs() < 1e-13);
        assert!((curve.transmittance(0.0, 1e5) - 1.0).abs() < f64::MIN_POSITIVE);
        // Past its ceiling, a curve with pressure rows reads −ln 10⁻⁶ however deep its rows.
        let deep: Vec<f64> = depths.iter().map(|d| 100.0 * d).collect();
        let deep = Curve::new(&log_columns, &log_pressures, 12.5, &deep);
        assert!((deep.depth_at(math::ln(1e21), 1e5) - CURVE_DEPTH_MAX).abs() < f64::MIN_POSITIVE);
        // A curve of one row reads as its row, whatever the pressure.
        let one = Curve::new(&log_columns, &log_pressures[..1], f64::NAN, &depths[..3]);
        assert!(!one.reads_pressure());
        let row = CurveRow::new(&log_columns, &depths[..3]);
        assert_eq!(
            hyperion_testkit::float::bits(one.transmittance(u, f64::NAN)),
            hyperion_testkit::float::bits(row.transmittance(u))
        );
        let none = Curve::new(&log_columns, &[], f64::NAN, &depths[..3]);
        assert_eq!(
            hyperion_testkit::float::bits(none.transmittance(u, 1e5)),
            hyperion_testkit::float::bits(row.transmittance(u))
        );
        // The absorber-weighted pressure and its floor.
        assert!((weighted_pressure_pa(2.0, 1.5) - 0.75e5).abs() < 1e-9);
        assert!((weighted_pressure_pa(2.0, 0.0) - PRESSURE_FLOOR_PA).abs() < f64::MIN_POSITIVE);
    }
}
