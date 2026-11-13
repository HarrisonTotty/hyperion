//! The warm envelopes' bright haze's optical depth (plan 14, P14.T24.h.b; research-leans-50's
//! ruling, 2026-11-10), which the sim's `photochem::bright_haze` takes at its whole share from
//! generator version 32 through
//! [`tables::warm_haze`](hyperion_sim::tables::warm_haze).
//!
//! The fit, as the cold giants' hazes were fitted (`photochem::AMMONIA_HAZE_OPTICAL_DEPTH` on
//! Jupiter's Bond albedo): the snapshot path's closed form of GJ 1214 b
//! ([`GJ_1214_B`]: Cloutier et al. 2021, AJ 162, 174) read under 32's rules at
//! [`DEPTH_POINTS_KEY`] optical depths of its bright haze from [`DEPTH_MIN_KEY`] to
//! [`DEPTH_MAX_KEY`], three or more, each its `T_eq` at its own Bond albedo
//! ([`WarmFixture::light`]), and the depth bisected between the two that bracket the planet's
//! measured Bond albedo, 0.51 ± 0.06 (Kempton et al. 2023, Nature 620, 67, recalled). Where every
//! point lies below it the fit takes the greatest depth, the column's reach: the closed form reads
//! a column only above τ(550) 10 (`envelope_bond::FLOOR_OPTICAL_DEPTH`, decision-deep-decks-split
//! §3.3), so that a thicker haze adds nothing. The acceptance is the brief's: the Bond albedo
//! within 0.39–0.63, `p_V` 0.15–0.25, the colour's B/V 0.6–0.7 and R/V 1.1–1.6.
//!
//! Every probe reads the depths it is given, never the committed table, so that the table is no
//! input of its own fit.

use std::num::NonZeroUsize;

use hyperion_sim::planetary::derive::photometry::warm_haze::{GJ_1214_B, WarmFixture, WarmLight};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The table's one item.
pub const ITEMS: [&str; 1] = ["BRIGHT_HAZE_OPTICAL_DEPTH"];

/// The manifest's key of the Bond albedo fitted to.
pub const TARGET_KEY: &str = "target_bond_albedo";
/// The manifest's key of the least optical depth read.
pub const DEPTH_MIN_KEY: &str = "depth_min";
/// The manifest's key of the greatest optical depth read: the column's reach.
pub const DEPTH_MAX_KEY: &str = "depth_max";
/// The manifest's key of the number of optical depths read, three or more.
pub const DEPTH_POINTS_KEY: &str = "depth_points";
/// The manifest's key of the bisections between the bracketing depths.
pub const BISECTIONS_KEY: &str = "bisections";

/// The optical depths the fingerprint reads the fixture at.
pub const PROBE_DEPTHS: [f64; 3] = [1.0, 3.0, 10.0];

/// The acceptance's ranges, as the manifest states them: (key of the least, key of the greatest).
const RANGES: [(&str, &str, &str); 4] = [
    ("the Bond albedo", "bond_albedo_min", "bond_albedo_max"),
    ("`p_V`", "geometric_albedo_v_min", "geometric_albedo_v_max"),
    ("B/V", "blue_over_visual_min", "blue_over_visual_max"),
    ("R/V", "red_over_visual_min", "red_over_visual_max"),
];

/// The fit: the depths read and their light, the fitted depth and its light, and whether the
/// measured albedo lay between two of the depths read.
#[derive(Debug, Clone, PartialEq)]
pub struct WarmHazeFit {
    /// Each depth read and the fixture's light there, in rising depth.
    pub points: Vec<(f64, WarmLight)>,
    /// The fitted τ(550).
    pub optical_depth: f64,
    /// The fixture's light at it.
    pub light: WarmLight,
    /// Whether the target lay between two of the depths read; if not, the fit took an end.
    pub bracketed: bool,
}

/// The fixture `fixture`'s light at the bright haze's optical depth `depth`.
///
/// # Errors
///
/// [`RunTaskError::Input`] where the closed form gives it no column.
fn light_at(fixture: &WarmFixture, depth: f64) -> Result<WarmLight, RunTaskError> {
    fixture.light(depth).ok_or_else(|| RunTaskError::Input {
        task: "warm_haze",
        source: format!("the fixture's closed form gives no column at τ(550) {depth}").into(),
    })
}

/// The fit of `fixture`'s bright haze to the Bond albedo `target` over `points` optical depths,
/// evenly spaced from `min` to `max`, by `bisections` bisections between the bracketing two.
///
/// # Errors
///
/// [`RunTaskError::Param`] for fewer than three points, a `min` that is not finite and positive or
/// a `max` not above it, and [`RunTaskError::Input`] where the closed form gives the fixture no
/// column at a depth read.
pub fn fit(
    fixture: &WarmFixture,
    target: f64,
    (min, max, points): (f64, f64, u32),
    bisections: u32,
) -> Result<WarmHazeFit, RunTaskError> {
    if points < 3 {
        return Err(ManifestParamError::new(DEPTH_POINTS_KEY, "three or more").into());
    }
    if !(min.is_finite() && min > 0.0) {
        return Err(ManifestParamError::new(DEPTH_MIN_KEY, "a finite positive depth").into());
    }
    if !(max.is_finite() && max > min) {
        return Err(
            ManifestParamError::new(DEPTH_MAX_KEY, "a finite depth above depth_min").into(),
        );
    }
    let steps = f64::from(points - 1);
    let mut read = Vec::with_capacity(usize::try_from(points).unwrap_or(0));
    for k in 0..points {
        let depth = min + (max - min) * f64::from(k) / steps;
        read.push((depth, light_at(fixture, depth)?));
    }
    let above = read
        .iter()
        .position(|(_, light)| light.bond_albedo >= target);
    let (optical_depth, bracketed) = match above {
        None => (max, false),
        Some(0) => (min, false),
        Some(i) => {
            let (mut low, mut high) = (read[i - 1].0, read[i].0);
            for _ in 0..bisections {
                let middle = f64::midpoint(low, high);
                if light_at(fixture, middle)?.bond_albedo < target {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            (f64::midpoint(low, high), true)
        }
    };
    Ok(WarmHazeFit {
        points: read,
        optical_depth,
        light: light_at(fixture, optical_depth)?,
        bracketed,
    })
}

/// The figures the acceptance reads of `light`: its Bond albedo, `p_V`, and its colour's B/V and
/// R/V (its spherical albedo's ratios, the disc's colour).
#[must_use]
pub fn figures(light: &WarmLight) -> [f64; 4] {
    let [b, v, r] = light.spherical_albedo;
    [light.bond_albedo, light.geometric_albedo[1], b / v, r / v]
}

/// The table for `fit`.
#[must_use]
pub fn render(fit: &WarmHazeFit) -> RustTable {
    let light = &fit.light;
    RustTable {
        summary: vec![
            "The warm envelopes' bright haze (plan 14, P14.T24.h.b, from generator version 32):"
                .to_owned(),
            "its τ(550) where it takes the whole of the soot's column, which".to_owned(),
            "`planetary::chem::photochem::bright_haze` draws.".to_owned(),
        ],
        notes: vec![
            "Fitted on GJ 1214 b's Bond albedo by the snapshot path's closed form of the planet"
                .to_owned(),
            "(`planetary::derive::photometry::warm_haze::GJ_1214_B`) under 32's rules: see the"
                .to_owned(),
            "`warm_haze` task of `hyperion-fit` and its manifest.".to_owned(),
        ],
        items: vec![TableItem::Scalar {
            name: ITEMS[0].to_owned(),
            doc: vec![
                "The bright haze's τ(550) at its whole share: GJ 1214 b's closed form gives a Bond"
                    .to_owned(),
                format!(
                    "albedo of {:.4} at it, its zero-albedo `T_eq` {:.1} K and its own {:.1} K.",
                    light.bond_albedo,
                    light.zero_albedo_temperature_k,
                    light.equilibrium_temperature_k
                ),
            ],
            value: fit.optical_depth,
        }],
    }
}

/// The acceptance of `fit` against the manifest's ranges, each figure with its range and verdict.
///
/// # Errors
///
/// [`ManifestParamError`] for a range's key missing or not a number.
fn acceptance(fit: &WarmHazeFit, manifest: &Manifest) -> Result<String, ManifestParamError> {
    let mut parts = Vec::with_capacity(RANGES.len() + 2);
    parts.push(format!(
        "GJ 1214 b's Bond albedo by its bright haze's τ(550): {}",
        fit.points
            .iter()
            .map(|(depth, light)| format!("{depth:.1} {:.4}", light.bond_albedo))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    parts.push(format!(
        "fitted τ(550) {:.4} ({})",
        fit.optical_depth,
        if fit.bracketed {
            "bisected between the bracketing depths"
        } else {
            "the measured albedo lies beyond every depth read: the nearest end"
        }
    ));
    for ((label, low, high), figure) in RANGES.iter().zip(figures(&fit.light)) {
        let (low, high) = (manifest.f64(low)?, manifest.f64(high)?);
        let verdict = if (low..=high).contains(&figure) {
            "passes"
        } else {
            "misses, a finding"
        };
        parts.push(format!("{label} {figure:.4} ({low}–{high}; {verdict})"));
    }
    let [b, v, r] = fit.light.geometric_albedo;
    parts.push(format!(
        "p in B, V and R {b:.4}, {v:.4} and {r:.4} at q {:.2}, {:.2} and {:.2}",
        fit.light.phase_integral[0], fit.light.phase_integral[1], fit.light.phase_integral[2]
    ));
    Ok(parts.join("; "))
}

/// The task: P14.T24.h.b's bright haze on GJ 1214 b, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct WarmHazeTask;

impl FitTask for WarmHazeTask {
    fn name(&self) -> &'static str {
        "warm_haze"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "warm_haze.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The fixture's Bond albedo, `p_V` and zero-albedo `T_eq` at [`PROBE_DEPTHS`], so that a
    /// change to the closed form, its optics or the fixture is caught; a depth the closed form
    /// gives no column at probes as NaN.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = Vec::with_capacity(3 * PROBE_DEPTHS.len());
        for depth in PROBE_DEPTHS {
            let light = GJ_1214_B.light(depth);
            let read = |f: fn(&WarmLight) -> f64| light.as_ref().map_or(f64::NAN, f);
            probes.extend([
                (
                    format!("GJ 1214 b's Bond albedo at τ(550) {depth}"),
                    read(|l| l.bond_albedo),
                ),
                (
                    format!("GJ 1214 b's p_V at τ(550) {depth}"),
                    read(|l| l.geometric_albedo[1]),
                ),
                (
                    format!("GJ 1214 b's zero-albedo T_eq at τ(550) {depth}"),
                    read(|l| l.zero_albedo_temperature_k),
                ),
            ]);
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let points = u32::try_from(manifest.u64(DEPTH_POINTS_KEY)?)
            .map_err(|_| ManifestParamError::new(DEPTH_POINTS_KEY, "three or more"))?;
        let bisections = u32::try_from(manifest.u64(BISECTIONS_KEY)?)
            .map_err(|_| ManifestParamError::new(BISECTIONS_KEY, "a small count"))?;
        let (min, max) = (manifest.f64(DEPTH_MIN_KEY)?, manifest.f64(DEPTH_MAX_KEY)?);
        let fit = fit(
            &GJ_1214_B,
            manifest.f64(TARGET_KEY)?,
            (min, max, points),
            bisections,
        )?;
        Ok(TaskOutput {
            table: render(&fit),
            source: "GJ 1214 b's Bond albedo, 0.51 ± 0.06 (Kempton et al. 2023, Nature 620, 67, \
                     recalled), on the sim's closed form of the planet and its host (Cloutier et \
                     al. 2021, AJ 162, 174, recalled) under 32's rules, its bright haze Khare et \
                     al.'s (1984, Icarus 60, 127) tholin (research-leans-50's ruling)"
                .to_owned(),
            acceptance: acceptance(&fit, manifest)?,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered table declares [`ITEMS`].
    #[test]
    fn the_table_declares_its_item() {
        let fit = fit(&GJ_1214_B, 0.51, (1.0, 10.0, 3), 0).expect("the fixture's column");
        let table = render(&fit);
        let names: Vec<&str> = table.items.iter().filter_map(TableItem::name).collect();
        assert_eq!(names, ITEMS);
    }

    /// The fit bisects a bracketed target to the depth whose albedo it is, and takes the nearest
    /// end where every depth read lies on one side of it.
    #[test]
    fn the_fit_bisects_a_bracketed_target_and_takes_an_end_otherwise() {
        let low = light_at(&GJ_1214_B, 1.0).expect("a column").bond_albedo;
        let high = light_at(&GJ_1214_B, 4.0).expect("a column").bond_albedo;
        let target = f64::midpoint(low, high);
        let bracketed = fit(&GJ_1214_B, target, (1.0, 4.0, 4), 30).expect("a column");
        assert!(bracketed.bracketed);
        assert!(
            (bracketed.light.bond_albedo - target).abs() < 1e-6,
            "{bracketed:?}"
        );
        let beyond = fit(&GJ_1214_B, 0.99, (1.0, 4.0, 3), 30).expect("a column");
        assert!(!beyond.bracketed);
        assert!((beyond.optical_depth - 4.0).abs() < 1e-12);
        let below = fit(&GJ_1214_B, 0.0, (1.0, 4.0, 3), 30).expect("a column");
        assert!((below.optical_depth - 1.0).abs() < 1e-12);
    }

    /// The fit refuses fewer than three depths and a range that is not one, naming the key.
    #[test]
    fn the_fit_refuses_a_range_that_is_not_one() {
        let refused = |range| match fit(&GJ_1214_B, 0.51, range, 0) {
            Err(RunTaskError::Param(error)) => error,
            other => panic!("expected a parameter error, got {other:?}"),
        };
        assert_eq!(
            refused((1.0, 10.0, 2)),
            ManifestParamError::new(DEPTH_POINTS_KEY, "three or more")
        );
        assert_eq!(
            refused((0.0, 10.0, 3)),
            ManifestParamError::new(DEPTH_MIN_KEY, "a finite positive depth")
        );
        assert_eq!(
            refused((5.0, 5.0, 3)),
            ManifestParamError::new(DEPTH_MAX_KEY, "a finite depth above depth_min")
        );
    }
}
