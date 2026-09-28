//! The old populations' forms and the own-form shares (plan 15, P15.T6.d).
//!
//! - **Thick disc and halo:** one flattened cored power law per speed bin, weight 1, fitted to the
//!   class's (R, |z|) histogram as the disc-born forms are ([`fit_disc`](super::fit_disc)).
//! - **Bulge, bar and nuclear disc:** the brainstorm's mixture, a share `s` of the population's
//!   own form plus a spheroid, fitted to the class's bar-frame histogram in |x|, |y|, |z|
//!   ([`BarCells`]). The own form is the field component's density, the shape plan 08 places
//!   the retained share with (its Design note 13), integrated over the same cells. The bar lies
//!   along x in the sim's fields and in the orbits' rotating frame at the epoch alike.
//!
//! The run holds one corotation ratio (the fixture's) and one bar mass, so the share measured
//! there fills all three of `COROTATION_RATIO_NODES` until the runs at the other nodes exist
//! (plan 15, P15.T6.d; the table's notes say so).
//!
//! The bar's elongation against the unkicked control ([`elongation`]) is read from the same
//! histograms: `1 − √(⟨y²⟩ ÷ ⟨x²⟩)` over the bound members within two bar half-lengths of the
//! centre, each class's over the control's, and the length along the bar `√⟨x²⟩` likewise.

use std::num::NonZeroUsize;

use hyperion_sim::galaxy::displaced::forms::CoredPowerLawParams;
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::{PointLy, Population};
use hyperion_sim::math;

use super::cells::{
    BarCells, RzCells, fastest_counts, map_in_order, noise_floor, normalise, shares,
    total_variation,
};
use super::fit_disc::{EDGE_FRACTION, spheroid_shares};
use super::histogram::{BAR_CELLS, BAR_XY_AXIS, ClassHistogram, LogAxis, R_CELLS, Z_AXIS, Z_CELLS};
use super::nelder_mead::{Bounded, Stop, minimise};
use crate::parallel::BuildThreadPoolError;

/// The boxes of `[a, q, γ]` for the old sources, `R_d`: wider than the disc-born boxes, since the
/// nuclear disc's core is a few hundredths of a scale length and the fastest halo classes fall
/// more slowly than any disc-born one.
pub const SPHEROID_BOXES: [Bounded; 3] = [
    Bounded::log(0.005, 20.0),
    Bounded::log(0.02, 1.0),
    Bounded::linear(0.5, 10.0),
];

/// The box of the own-form share.
pub const SHARE_BOX: Bounded = Bounded::linear(0.0, 1.0);

/// The fixed start of `[a, q, γ]`, and the own-form share's.
pub const START: [f64; 4] = [0.5, 0.6, 3.5, 0.5];

/// One old class's fit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OldFit {
    /// The own-form share: 0 for the thick disc and halo.
    pub own_share: f64,
    /// The spheroid, weight 1.
    pub spheroid: CoredPowerLawParams,
    /// The misplaced share.
    pub misplaced: f64,
    /// Its noise floor.
    pub floor: f64,
    /// Parameters on a box edge.
    pub at_edge: usize,
    /// Whether the class had no bound member inside the cube.
    pub empty: bool,
}

/// How the old fits are run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OldSettings {
    /// The most evaluations of a fit.
    pub evaluations: usize,
    /// The simplex's tolerance.
    pub tolerance: f64,
}

fn empty(own: bool) -> OldFit {
    OldFit {
        own_share: if own { START[3] } else { 0.0 },
        spheroid: CoredPowerLawParams {
            weight: 1.0,
            a: START[0],
            q: START[1],
            gamma: START[2],
        },
        misplaced: 0.0,
        floor: 0.0,
        at_edge: 0,
        empty: true,
    }
}

/// Fits the thick disc's or the halo's eight classes, one spheroid each, on `threads` threads.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
pub fn fit_spheroids(
    cells: &RzCells,
    records: &[ClassHistogram],
    settings: OldSettings,
    threads: NonZeroUsize,
) -> Result<Vec<OldFit>, BuildThreadPoolError> {
    map_in_order(records.len(), threads, |i| {
        let record = &records[i];
        let counts = if i + 1 == records.len() {
            fastest_counts(&records.iter().collect::<Vec<_>>())
        } else {
            record.bound.clone()
        };
        let members: u64 = counts.iter().sum();
        if members == 0 {
            return empty(false);
        }
        let observed = shares(&counts);
        let mut model = vec![0.0; R_CELLS * Z_CELLS];
        let found = minimise(
            |p| {
                spheroid_shares(cells, p[0], p[1], p[2], &mut model);
                total_variation(&model, &observed)
            },
            &SPHEROID_BOXES,
            &START[..3],
            0.3,
            Stop {
                evaluations: settings.evaluations,
                tolerance: settings.tolerance,
            },
        );
        let p = &found.values;
        spheroid_shares(cells, p[0], p[1], p[2], &mut model);
        OldFit {
            own_share: 0.0,
            spheroid: CoredPowerLawParams {
                weight: 1.0,
                a: p[0],
                q: p[1],
                gamma: p[2],
            },
            misplaced: total_variation(&model, &observed),
            floor: noise_floor(&model, members),
            at_edge: (0..3)
                .filter(|&k| SPHEROID_BOXES[k].at_edge(p[k], EDGE_FRACTION))
                .count(),
            empty: false,
        }
    })
}

/// The own form of `population`, its field density integrated over the bar-frame cells and
/// normalised: lengths in `R_d` of `r_d` ly.
#[must_use]
pub fn own_form(fields: &Fields, population: Population, cells: &BarCells, r_d: f64) -> Vec<f64> {
    let mut out = vec![0.0; BAR_CELLS.iter().product()];
    cells.integrate(
        |x, y, z| fields.population_density(population, &PointLy::new(x * r_d, y * r_d, z * r_d)),
        &mut out,
    );
    normalise(&mut out);
    out
}

/// The spheroid's shares of the bar-frame cells.
fn spheroid_box_shares(cells: &BarCells, a: f64, q: f64, gamma: f64, out: &mut [f64]) {
    let (inv_core_sq, inv_axis_sq) = (1.0 / (a * a), 1.0 / (q * q));
    cells.integrate(
        |x, y, z| {
            math::exp(
                -0.5 * gamma * math::ln_1p((x * x + y * y + z * z * inv_axis_sq) * inv_core_sq),
            )
        },
        out,
    );
    normalise(out);
}

/// Fits a barred source's eight classes: the own-form share and the spheroid, on `threads`
/// threads.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
pub fn fit_mixtures(
    cells: &BarCells,
    own: &[f64],
    records: &[ClassHistogram],
    settings: OldSettings,
    threads: NonZeroUsize,
) -> Result<Vec<OldFit>, BuildThreadPoolError> {
    let boxes = [
        SPHEROID_BOXES[0],
        SPHEROID_BOXES[1],
        SPHEROID_BOXES[2],
        SHARE_BOX,
    ];
    map_in_order(records.len(), threads, |i| {
        let record = &records[i];
        let Some(bar) = record
            .bar_frame
            .as_ref()
            .filter(|_| record.in_cube_bound > 0)
        else {
            return empty(true);
        };
        let observed = shares(bar);
        let n = own.len();
        let (mut spheroid, mut model) = (vec![0.0; n], vec![0.0; n]);
        let evaluate = |p: &[f64], spheroid: &mut [f64], model: &mut [f64]| {
            spheroid_box_shares(cells, p[0], p[1], p[2], spheroid);
            for ((m, o), s) in model.iter_mut().zip(own).zip(spheroid.iter()) {
                *m = p[3] * o + (1.0 - p[3]) * s;
            }
            total_variation(model, &observed)
        };
        let found = minimise(
            |p| evaluate(p, &mut spheroid, &mut model),
            &boxes,
            &START,
            0.3,
            Stop {
                evaluations: settings.evaluations,
                tolerance: settings.tolerance,
            },
        );
        let p = &found.values;
        let misplaced = evaluate(p, &mut spheroid, &mut model);
        OldFit {
            own_share: p[3],
            spheroid: CoredPowerLawParams {
                weight: 1.0,
                a: p[0],
                q: p[1],
                gamma: p[2],
            },
            misplaced,
            floor: noise_floor(&model, record.in_cube_bound),
            at_edge: (0..4)
                .filter(|&k| boxes[k].at_edge(p[k], EDGE_FRACTION))
                .count(),
            empty: false,
        }
    })
}

/// A barred class's shape against the unkicked control's, within `radius` ly of the centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elongation {
    /// `1 − √(⟨y²⟩ ÷ ⟨x²⟩)` over the class's over the control's.
    pub relative: f64,
    /// `√⟨x²⟩` over the control's.
    pub length: f64,
}

/// The moments `(⟨x²⟩, ⟨y²⟩)` of a bar-frame histogram within `radius` ly, from the cells'
/// geometric centres; `None` if no member lies there.
fn moments(bar: &[u64], radius: f64) -> Option<(f64, f64)> {
    let centre = |i: usize, axis: LogAxis, cells: usize| -> f64 {
        if i == 0 {
            return 0.5 * axis.first_ly;
        }
        #[expect(clippy::cast_precision_loss, reason = "cell indices below 64")]
        let per = axis.octaves / (cells - 1) as f64;
        #[expect(clippy::cast_precision_loss, reason = "cell indices below 64")]
        let lo = (i - 1) as f64;
        axis.first_ly * math::exp2(per * (lo + 0.5))
    };
    let (mut total, mut sum_xx, mut sum_yy) = (0.0, 0.0, 0.0);
    for ix in 0..BAR_CELLS[0] {
        let x = centre(ix, BAR_XY_AXIS, BAR_CELLS[0]);
        for iy in 0..BAR_CELLS[1] {
            let y = centre(iy, BAR_XY_AXIS, BAR_CELLS[1]);
            for iz in 0..BAR_CELLS[2] {
                let z = centre(iz, Z_AXIS, BAR_CELLS[2]);
                let count = bar[(ix * BAR_CELLS[1] + iy) * BAR_CELLS[2] + iz];
                if count == 0 || x * x + y * y + z * z > radius * radius {
                    continue;
                }
                #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
                let count = count as f64;
                total += count;
                sum_xx += count * x * x;
                sum_yy += count * y * y;
            }
        }
    }
    (total > 0.0).then(|| (sum_xx / total, sum_yy / total))
}

/// The elongation of each of a barred source's classes against its control, within `radius` ly;
/// `None` for a class, or a control, with no bound member there.
#[must_use]
pub fn elongation(
    records: &[ClassHistogram],
    control: &ClassHistogram,
    radius: f64,
) -> Vec<Option<Elongation>> {
    let shape = |r: &ClassHistogram| {
        r.bar_frame
            .as_deref()
            .and_then(|bar| moments(bar, radius))
            .map(|(xx, yy)| (1.0 - (yy / xx).sqrt(), xx.sqrt()))
    };
    let Some((e0, l0)) = shape(control).filter(|&(e, _)| e > 0.0) else {
        return vec![None; records.len()];
    };
    records
        .iter()
        .map(|r| {
            shape(r).map(|(e, l)| Elongation {
                relative: e / e0,
                length: l / l0,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::displaced_forms::histogram::{Integration, bar_cell};

    /// A control stretched along x and a round class: the class keeps none of the elongation and
    /// has the control's length only where it reaches as far.
    #[test]
    fn elongation_is_read_against_the_control() {
        let put = |points: &[[f64; 3]]| {
            let mut r = ClassHistogram::new(true);
            for &x in points {
                r.record(x, [0.0; 3], true, Integration::default());
            }
            r
        };
        let control = put(&[[4_000.0, 1_000.0, 0.0], [-4_000.0, -1_000.0, 10.0]]);
        let round = put(&[[2_000.0, 2_000.0, 0.0], [-2_000.0, 2_000.0, 10.0]]);
        let same = control.clone();
        let out = elongation(&[round, same], &control, 20_000.0);
        let round = out[0].expect("members inside the radius");
        assert!(round.relative.abs() < 1e-12, "{round:?}");
        let same = out[1].expect("members inside the radius");
        assert!((same.relative - 1.0).abs() < 1e-12 && (same.length - 1.0).abs() < 1e-12);
        assert!(bar_cell([4_000.0, 1_000.0, 0.0]) < BAR_CELLS.iter().product());
        // Nothing inside the radius reads as unknown.
        assert!(elongation(std::slice::from_ref(&control), &control, 10.0)[0].is_none());
    }
}
