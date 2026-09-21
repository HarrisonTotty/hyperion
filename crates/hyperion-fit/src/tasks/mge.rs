//! `run mge`: the Gaussian expansions behind the galaxy's potential (plan 02, Design note 6).
//!
//! The potential of a sum of axisymmetric Gaussians is a one-dimensional quadrature per Gaussian
//! (Emsellem, Monnet and Bacon 1994), so each density profile of the mass model is expanded as
//! `f(x) ≈ Σ wₖ exp(−x² ÷ 2σₖ²)` with dimensionless weights `wₖ` and widths `σₖ`. Two profiles
//! are enough for the first milestone:
//!
//! - `MGE_EXP`, the exponential `e^(−s)`, which serves a disc's radial and vertical profiles and
//!   the bulge's spheroid. It is fitted on `s` from 0.02 to 12 in relative error: each sample is
//!   weighted by `1 ÷ e^(−s)`.
//! - `MGE_BAR`, the azimuthal average of the long bar's surface density at its nominal shape, in
//!   units of the half-length. It is fitted in absolute error, with one more row for its total
//!   mass. A sum of centred Gaussians with non-negative weights is a completely monotone function
//!   of `x²`, so it cannot follow the bar's Gaussian end in relative terms; what it can match is
//!   the profile to a few per cent of its central value and its mass.
//!
//! The widths are fixed and log-spaced; the weights are the non-negative least-squares solution,
//! found by projected coordinate descent on the normal equations with a fixed number of sweeps
//! from zero, so the result depends on no tolerance test. Plan 15 refits with free widths.

use std::fmt::Write as _;

use hyperion_sim::galaxy::quad::gl_panels;
use hyperion_sim::galaxy::special::bessel_i0e;
use hyperion_sim::math;

/// The task's version, written into the table's header. A change to anything below that moves
/// the table bumps it.
pub const VERSION: u32 = 0;

/// The number of widths in each expansion.
pub const WIDTHS: usize = 14;

/// Sweeps of coordinate descent. The solution stops changing well before this: 20,000 sweeps
/// already agree with an exact active-set solution to the digits that matter.
const SWEEPS: u32 = 50_000;

/// `e^(−s)`: the fitted range of `s`, the number of log-spaced samples, and the range of widths.
const EXP_RANGE: (f64, f64) = (0.02, 12.0);
const EXP_SAMPLES: usize = 300;
const EXP_WIDTHS: (f64, f64) = (0.04, 3.5);

/// The long bar's nominal shape, in half-lengths: its Gaussian width across (0.1, the middle of
/// the drawn 0.08–0.12), the end of its level part (0.85) and the width of its Gaussian end
/// (0.15), as plan 02's P02.T6.a and P02.T7.c give them.
const BAR_WIDTH: f64 = 0.1;
const BAR_LEVEL: f64 = 0.85;
const BAR_END: f64 = 0.15;

/// The bar's fit: samples evenly spaced on `[0, BAR_REACH]`, the weight of the mass row, and the
/// range of widths.
const BAR_REACH: f64 = 2.0;
const BAR_SAMPLES: usize = 321;
const BAR_MASS_WEIGHT: f64 = 5.0;
const BAR_WIDTHS: (f64, f64) = (0.04, 0.8);

/// The fitted expansions, each `[(weight, width); WIDTHS]` with widths ascending.
#[derive(Debug, Clone, PartialEq)]
pub struct MgeTables {
    /// `e^(−s)`.
    pub exp: [(f64, f64); WIDTHS],
    /// The long bar's azimuthally averaged surface density, 1 at the centre.
    pub bar: [(f64, f64); WIDTHS],
}

/// `N` points log-spaced from `lo` to `hi`, both exactly included.
fn log_spaced<const N: usize>(lo: f64, hi: f64) -> [f64; N] {
    let step = (math::ln(hi) - math::ln(lo)) / index(N - 1);
    let mut points = [0.0; N];
    for (i, point) in points.iter_mut().enumerate() {
        *point = math::exp(math::ln(lo) + step * index(i));
    }
    points[0] = lo;
    points[N - 1] = hi;
    points
}

/// A small index as a float.
fn index(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("a table index fits in u32"))
}

/// The non-negative least-squares weights for `rows · w ≈ rhs`, by projected coordinate descent
/// on the normal equations: each sweep minimises over one weight at a time, in order, and clamps
/// it at zero.
fn nnls(rows: &[[f64; WIDTHS]], rhs: &[f64]) -> [f64; WIDTHS] {
    let mut gram = [[0.0; WIDTHS]; WIDTHS];
    let mut projected = [0.0; WIDTHS];
    for (row, &b) in rows.iter().zip(rhs) {
        for k in 0..WIDTHS {
            projected[k] += row[k] * b;
            for j in 0..WIDTHS {
                gram[k][j] += row[k] * row[j];
            }
        }
    }
    let mut weights = [0.0; WIDTHS];
    for _ in 0..SWEEPS {
        for k in 0..WIDTHS {
            let mut gradient = -projected[k];
            for (g, w) in gram[k].iter().zip(&weights) {
                gradient += g * w;
            }
            weights[k] = (weights[k] - gradient / gram[k][k]).max(0.0);
        }
    }
    weights
}

/// Pairs weights with widths.
fn table(weights: [f64; WIDTHS], widths: [f64; WIDTHS]) -> [(f64, f64); WIDTHS] {
    let mut table = [(0.0, 0.0); WIDTHS];
    for (entry, (w, s)) in table.iter_mut().zip(weights.into_iter().zip(widths)) {
        *entry = (w, s);
    }
    table
}

/// The expansion of `e^(−s)`, in relative error.
fn fit_exp() -> [(f64, f64); WIDTHS] {
    let widths: [f64; WIDTHS] = log_spaced(EXP_WIDTHS.0, EXP_WIDTHS.1);
    let samples: [f64; EXP_SAMPLES] = log_spaced(EXP_RANGE.0, EXP_RANGE.1);
    // Each row divided by e^(−s): exp(−s² ÷ 2σ²) ÷ e^(−s) = exp(s − s² ÷ 2σ²).
    let rows: Vec<[f64; WIDTHS]> = samples
        .iter()
        .map(|&s| widths.map(|sigma| math::exp(s - s * s / (2.0 * sigma * sigma))))
        .collect();
    let rhs = vec![1.0; rows.len()];
    table(nnls(&rows, &rhs), widths)
}

/// The bar's level profile along its length: 1 to [`BAR_LEVEL`], then a Gaussian end of width
/// [`BAR_END`].
fn bar_length_profile(x: f64) -> f64 {
    if x <= BAR_LEVEL {
        1.0
    } else {
        let t = (x - BAR_LEVEL) / BAR_END;
        math::exp(-0.5 * t * t)
    }
}

/// The azimuthal average at radius `u` (half-lengths) of the bar's surface density
/// `L(|x|) exp(−y² ÷ 2w²)`, which is 1 at the centre.
///
/// Where the bar is level all round the circle, `u ≤ 0.85`, the average is the closed form
/// `e^(−b) I₀(b)` with `b = u² ÷ 4w²`, since `(1 ÷ 2π) ∫ exp(−2b sin²θ) dθ = e^(−b) I₀(b)`.
/// Beyond, the shortfall of the Gaussian end is subtracted: `(2 ÷ π) ∫₀^θₖ (1 − L(u cos θ))
/// exp(−u² sin²θ ÷ 2w²) dθ` with `cos θₖ = 0.85 ÷ u`, by `gl32` on panels that narrow towards
/// `θ = 0`, where the cross-section's Gaussian peaks.
#[must_use]
pub fn bar_profile(u: f64) -> f64 {
    let level = bessel_i0e(u * u / (4.0 * BAR_WIDTH * BAR_WIDTH));
    if u <= BAR_LEVEL {
        return level;
    }
    let theta_k = math::acos(BAR_LEVEL / u);
    let edges = [0.0, 1.0 / 16.0, 0.125, 0.25, 0.5, 1.0].map(|f| f * theta_k);
    let shortfall = gl_panels(
        |theta| {
            let (sin, cos) = math::sin_cos(theta);
            let across = u * sin / BAR_WIDTH;
            (1.0 - bar_length_profile(u * cos)) * math::exp(-0.5 * across * across)
        },
        &edges,
    );
    level - shortfall * 2.0 / core::f64::consts::PI
}

/// The mass of the bar's surface density, in half-lengths squared: `√(2π) w × 2 (0.85 + 0.15
/// √(π ÷ 2))`, the Gaussian cross-section times the length profile's integral.
#[must_use]
pub fn bar_mass() -> f64 {
    let pi = core::f64::consts::PI;
    (2.0 * pi).sqrt() * BAR_WIDTH * 2.0 * (BAR_LEVEL + BAR_END * (0.5 * pi).sqrt())
}

/// The expansion of the bar's profile, in absolute error with a row for the mass.
fn fit_bar() -> [(f64, f64); WIDTHS] {
    let widths: [f64; WIDTHS] = log_spaced(BAR_WIDTHS.0, BAR_WIDTHS.1);
    let step = BAR_REACH / index(BAR_SAMPLES - 1);
    let mut rows = Vec::with_capacity(BAR_SAMPLES + 1);
    let mut rhs = Vec::with_capacity(BAR_SAMPLES + 1);
    for i in 0..BAR_SAMPLES {
        let u = step * index(i);
        rows.push(widths.map(|sigma| math::exp(-u * u / (2.0 * sigma * sigma))));
        rhs.push(bar_profile(u));
    }
    // A Gaussian of unit amplitude and width σ holds 2πσ² in the plane.
    let two_pi = 2.0 * core::f64::consts::PI;
    rows.push(widths.map(|sigma| BAR_MASS_WEIGHT * two_pi * sigma * sigma / bar_mass()));
    rhs.push(BAR_MASS_WEIGHT);
    table(nnls(&rows, &rhs), widths)
}

/// Runs both fits. Single-threaded, with no randomness, so the result is the same on every run
/// and platform.
#[must_use]
pub fn fit() -> MgeTables {
    MgeTables {
        exp: fit_exp(),
        bar: fit_bar(),
    }
}

/// `value` as a Rust float literal: the shortest decimal that reads back to the same `f64`, with
/// its digits grouped in threes as Clippy asks.
fn literal(value: f64) -> String {
    let shortest = format!("{value:?}");
    let (mantissa, exponent) = match shortest.split_once('e') {
        Some((m, e)) => (m, Some(e)),
        None => (shortest.as_str(), None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, "0"));
    let mut out = String::new();
    let digits: Vec<char> = whole.chars().collect();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('_');
        }
        out.push(*c);
    }
    out.push('.');
    for (i, c) in fraction.chars().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('_');
        }
        out.push(c);
    }
    if let Some(e) = exponent {
        out.push('e');
        out.push_str(e);
    }
    out
}

/// `n` with its thousands separated by commas, as prose writes it.
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Writes one table as a `pub const`.
fn write_table(out: &mut String, doc: &str, name: &str, entries: &[(f64, f64); WIDTHS]) {
    out.push_str(doc);
    writeln!(out, "pub const {name}: [(f64, f64); {WIDTHS}] = [").expect("writing to a String");
    for &(weight, width) in entries {
        writeln!(out, "    ({}, {}),", literal(weight), literal(width))
            .expect("writing to a String");
    }
    out.push_str("];\n");
}

/// The committed source of `crates/hyperion-sim/src/tables/mge.rs`.
#[must_use]
pub fn render(tables: &MgeTables) -> String {
    let mut out = format!(
        "\
//! Gaussian expansions of the profiles the galaxy's potential is built from (plan 02, Design
//! note 6). Provisional: plan 15 refits them with free widths and replaces them.
//!
//! Generated by `hyperion-fit run mge`, version {VERSION}. Do not edit: `cargo test -p
//! hyperion-fit` renders the fit again and fails unless this file matches it byte for byte.
//!
//! Each table is `[(weight, width); {WIDTHS}]`, widths ascending, and stands for the profile
//! `Σ weight × exp(−x² ÷ 2 width²)`. The widths are fixed and log-spaced; the weights are the
//! non-negative least-squares solution by projected coordinate descent on the normal equations,
//! {sweeps} sweeps from zero. Inputs:
//!
//! - [`MGE_EXP`]: `e^(−s)` at {EXP_SAMPLES} log-spaced `s` from {} to {}, each row weighted by
//!   `1 ÷ e^(−s)` so that the relative error is what is fitted. Widths from {} to {}.
//! - [`MGE_BAR`]: the azimuthal average of the long bar's surface density `L(|x|) exp(−y² ÷ 2w²)`,
//!   with `w = {}` and `L` level to {} with a Gaussian end of width {}, all in half-lengths.
//!   At {BAR_SAMPLES} evenly spaced `u` from 0 to {}, in absolute error, with one more row for
//!   the total mass, weighted {}. Widths from {} to {}.
",
        EXP_RANGE.0,
        EXP_RANGE.1,
        EXP_WIDTHS.0,
        EXP_WIDTHS.1,
        BAR_WIDTH,
        BAR_LEVEL,
        BAR_END,
        BAR_REACH,
        BAR_MASS_WEIGHT,
        BAR_WIDTHS.0,
        BAR_WIDTHS.1,
        sweeps = thousands(SWEEPS),
    );
    out.push('\n');
    write_table(
        &mut out,
        "/// `e^(−s) ≈ Σ weight × exp(−s² ÷ 2 width²)` for `s ≥ 0`.\n",
        "MGE_EXP",
        &tables.exp,
    );
    out.push('\n');
    write_table(
        &mut out,
        "/// The long bar's azimuthally averaged surface density, 1 at the centre: at `u`\n\
         /// half-lengths it is `≈ Σ weight × exp(−u² ÷ 2 width²)`, the widths in half-lengths.\n",
        "MGE_BAR",
        &tables.bar,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_read_back_to_the_same_value_and_are_grouped() {
        for (value, expected) in [
            (0.04, "0.04"),
            (3.5, "3.5"),
            (1.0, "1.0"),
            (0.0, "0.0"),
            (1_234.567_8, "1_234.567_8"),
            (0.001_234_567_891_234_5, "0.001_234_567_891_234_5"),
            (1.234_567e-7, "1.234_567e-7"),
        ] {
            let text = literal(value);
            assert_eq!(text, expected);
            let back: f64 = text.replace('_', "").parse().unwrap();
            assert!(back.total_cmp(&value).is_eq(), "{text}");
        }
    }

    /// The level part of the bar's average is the closed form, and it joins the quadrature
    /// continuously at 0.85.
    #[test]
    fn the_bar_profile_is_continuous_and_falls() {
        let below = bar_profile(BAR_LEVEL);
        let above = bar_profile(BAR_LEVEL + 1e-9);
        assert!((below - above).abs() < 1e-9, "{below} against {above}");
        assert!((bar_profile(0.0) - 1.0).abs() < 1e-15);
        let mut previous = bar_profile(0.0);
        for i in 1..=200 {
            let value = bar_profile(f64::from(i) * 0.01);
            assert!(value < previous, "rises at {}", f64::from(i) * 0.01);
            previous = value;
        }
    }

    /// The bar's mass from its average profile, `2π ∫ Σ̄ u du`, matches the closed form.
    #[test]
    fn the_bar_profile_holds_the_bar_mass() {
        let edges = [0.0, 0.25, 0.5, 0.85, 1.1, 1.4, 2.0, 3.0];
        let mass = 2.0 * core::f64::consts::PI * gl_panels(|u| u * bar_profile(u), &edges);
        assert!(
            (mass / bar_mass() - 1.0).abs() < 1e-9,
            "{mass} against {}",
            bar_mass()
        );
    }

    #[test]
    fn widths_are_log_spaced_and_end_exactly() {
        let widths: [f64; WIDTHS] = log_spaced(0.04, 3.5);
        assert!(widths[0].total_cmp(&0.04).is_eq());
        assert!(widths[WIDTHS - 1].total_cmp(&3.5).is_eq());
        let ratio = widths[1] / widths[0];
        for pair in widths.windows(2) {
            assert!((pair[1] / pair[0] - ratio).abs() < 1e-12);
        }
    }
}
