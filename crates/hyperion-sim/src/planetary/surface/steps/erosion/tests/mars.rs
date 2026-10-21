//! The reference Mars's erosion (R09.T14.c; the lane's design §4.4–§4.5, decision-r09-t14-choices
//! F1, "The ceiling risk" and the "T14.b follow-up" item 5, and decision-r09-t14-mars-ceiling
//! §3–§4): F1's trunk incision against Black et al.'s, with (A0) the sub-cell volume of the
//! synthesis's closed form, pinned to it, (A2) recorded and (B) as an area quantile; the
//! residual against the incision; the paleo-water's records; the ceiling's measurement, with each
//! cell above (B); the three starts of a finer level; and the slow record (a finer grid, and the
//! epoch at the ends of Hoke et al.'s range).

use std::time::Instant;

use hyperion_surface::field::{CoarseLevel, Crust, LogRunoff, LogSteepness, SynthesisCell};
use hyperion_surface::num;
use hyperion_surface::synth::channels::{
    ChannelBody, MEAN_TRIBUTARY_LENGTH, MEAN_TRUNK_LENGTH, TRIBUTARY_SHARE,
    TRIBUTARY_VARIANCE_SHARE, VALLEY_WALL_GRADIENT, VALLEY_WALL_SHARE, WHOLE_VARIANCE_SHARE,
    bankfull_depth, bankfull_width, channel_forming_discharge_m3_s, channel_head_area,
    channel_variance, deepest_level, hack_area, head_length, main_stream_length,
    mean_discharge_m3_s, tributary_rise, tributary_steepness_m0_9, valley_depth,
};

use super::super::branches::{
    DryNow, WALL_RUN_PER_DEPTH, dry_now, dry_now_years, erode_dry_now_with, paleo_water_m3,
};
use super::super::erodibility::Erodibility;
use super::super::multigrid::{Options, Outcome, Start};
use super::super::run;
use super::super::solver::DRAINAGE_EXPONENT;
use super::*;
use crate::planetary::surface::inputs::WetEpoch;
use crate::planetary::surface::reference::builder_of;
use crate::planetary::surface::steps::relief::budget;
use crate::units::{MetresPerSecondSquared, SquareMetres, Years};

/// The drainage area from which a cell is a trunk reach, m²: 10⁴ km² (decision-r09-t14-choices
/// F1's (A1)).
const TRUNK_AREA_M2: f64 = 1.0e10;

/// The trunk incision Black et al. 2017 calibrate on, m: 50 to 350, "Trunk channels of Martian
/// valley networks incised ~50 to 350 m into older terrains" (Howard, Moore and Irwin 2005, JGR
/// 110, E12S14, as quoted in Black et al. 2017's supplementary Materials and Methods, Science 356,
/// 727; science-hoke §4; decision-r09-t14-choices F1's (A1)).
const TRUNK_INCISION_M: (f64, f64) = (50.0, 350.0);

/// (B)'s valley width, m: 10 km, the ruling's choice and not a single sourced figure
/// (decision-r09-t14-choices F3, after science-hoke §4): toward the upper end of Hynek, Beach and
/// Hoke 2010's valleys "hundreds of meters to 20 km wide" (JGR 115, E09008, ¶10), against
/// Williams and Phillips 2001's mean of 2,040 ± 1,548 m (JGR 106, 23737, abstract). With the 350 m
/// of trunk incision it gives (B)'s cross-section, 350 m × 10 km.
const CEILING_WIDTH_M: f64 = 10.0e3;

/// (B)'s area quantile: the 99th percentile (decision-r09-t14-mars-ceiling §4.1).
const CEILING_QUANTILE: f64 = 0.99;

/// Luo, Cang and Howard 2017's global equivalent layer of the valleys' void, m: 1.2
/// ((1.74 ± 0.8) × 10¹⁴ m³; decision-r09-t14-choices F1's (A2)).
const VALLEY_LAYER_M: f64 = 1.2;

/// The (A2) layer above which the coarse volume is a finding, m: 6.1, three times Luo et al.'s
/// combined HRSC-scaled 2.04 m (decision-r09-t14-mars-ceiling §4.2).
const VALLEY_LAYER_FINDING_M: f64 = 6.1;

/// The share of valleys deeper than 350 m, about 4.6%: a derived figure, not a measurement of
/// trunks, the tail above 350 m of a log-normal fitted to Williams and Phillips 2001's abstract
/// mean and standard deviation of depth, 109 ± 137 m, over all their valley cross-sections (JGR
/// 106, 23737; decision-r09-t14-mars-ceiling's relay, (A1)'s record; the science check's fit gives
/// 0.0460). The like-for-like comparison for trunks is [`TRUNK_INCISION_M`]'s range.
const DEEP_VALLEY_SHARE: f64 = 0.046;

/// The drainage classes by which (A1)'s deep tail is recorded, m², each from its lower bound to
/// the next: 10⁴–10⁵, 10⁵–10⁶ and above 10⁶ km² (decision-r09-t14-choices, "T14.c follow-up"
/// item 4).
const DRAINAGE_CLASSES_M2: [(f64, f64, &str); 3] = [
    (1.0e10, 1.0e11, "10⁴–10⁵ km²"),
    (1.0e11, 1.0e12, "10⁵–10⁶ km²"),
    (1.0e12, f64::INFINITY, "above 10⁶ km²"),
];

/// Matsubara, Howard and Drummond 2011's highland standing water, 15–30% of the highlands at
/// their X of 7 to 3 (JGR 116, E04001, ¶61; decision-r09-t14-mars-ceiling §3).
const HIGHLAND_WATER_SHARE: (f64, f64) = (0.15, 0.30);

/// The wavelength below which the pin compares the channels' variance, m: 100 km (the lane's
/// research script's, the height golden's `channel variance finer than 100000 m`).
const PIN_FINER_THAN_M: f64 = 1.0e5;

/// The share of a level's wall width at which a smoothstep wall is steepest: 1.5 (the synthesis's
/// `STEEPEST_WALL`, private to it).
const STEEPEST_WALL: f64 = 1.5;

/// Prints `values`' count and distribution under `what`, metres.
fn print_distribution(what: &str, values: Vec<f64>) {
    let count = values.len();
    match quantiles(values) {
        Some([min, p10, p25, p50, p75, p90, max]) => eprintln!(
            "{what}: {count} cells, min {min:.2} m, p10 {p10:.2}, p25 {p25:.2}, median {p50:.2}, \
             p75 {p75:.2}, p90 {p90:.2}, max {max:.2} m"
        ),
        None => eprintln!("{what}: no cells"),
    }
}

/// The ceiling's figures of one dry-now erosion (decision-r09-t14-mars-ceiling §4): its
/// construction, its branch's result, and the cells it erodes.
struct Ceiling<'a> {
    dry: &'a DryNow,
    z0: &'a [f64],
    omega: &'a [f64],
    radius_m: f64,
    /// The eroded cells: not bound, of equivalent area above zero, of exposure above zero.
    eroded: Vec<usize>,
}

/// The share of `incision` deeper than Black et al.'s 350 m, 0 where it is empty.
fn deep_share(incision: &[f64]) -> f64 {
    let deep = incision.iter().filter(|&&d| d > TRUNK_INCISION_M.1).count();
    u32::try_from(deep).map_or(0.0, f64::from)
        / u32::try_from(incision.len().max(1)).map_or(1.0, f64::from)
}

/// One cell above (B), as §4.1 records it.
struct Above {
    cell: usize,
    lowering_m: f64,
    bound_m: f64,
}

impl<'a> Ceiling<'a> {
    fn new(dry: &'a DryNow, z0: &'a [f64], omega: &'a [f64], radius_m: f64) -> Self {
        let outcome = &dry.outcome;
        let eroded = (0..z0.len())
            .filter(|&c| {
                outcome.bounds[c].is_none()
                    && outcome.equivalent_area_m2[c] > 0.0
                    && dry.exposure[c] > 0.0
            })
            .collect();
        Self {
            dry,
            z0,
            omega,
            radius_m,
            eroded,
        }
    }

    /// The channel incision at `c`, unscaled, metres.
    fn incision(&self, c: usize) -> f64 {
        self.dry.incision_m[c]
    }

    /// The coarse cell's lowering at `c` as the branch writes it, metres.
    fn lowering(&self, c: usize) -> f64 {
        self.dry.lowering_m[c]
    }

    /// The cell's width, R √Ω, metres.
    fn width(&self, c: usize) -> f64 {
        self.radius_m * self.omega[c].sqrt()
    }

    /// (B)'s bound at `c`, 350 m × 10 km ÷ its width, metres.
    fn bound(&self, c: usize) -> f64 {
        TRUNK_INCISION_M.1 * CEILING_WIDTH_M / self.width(c)
    }

    /// The closure's `f_w` at `c`, (`w_bf` + 5d) ÷ w, uncapped ([`WALL_RUN_PER_DEPTH`]).
    fn f_w(&self, c: usize) -> f64 {
        (self.dry.floor_m[c] + WALL_RUN_PER_DEPTH * self.incision(c)) / self.width(c)
    }

    /// The trunk cells' channel incisions: the eroded cells draining at least 10⁴ km².
    fn trunk(&self) -> Vec<f64> {
        self.eroded
            .iter()
            .filter(|&&c| self.dry.outcome.drainage_m2[c] >= TRUNK_AREA_M2)
            .map(|&c| self.incision(c))
            .collect()
    }

    /// (A1): the trunk cells' median channel incision, metres.
    fn trunk_median(&self) -> f64 {
        quantiles(self.trunk()).expect("the reference Mars has trunk reaches")[3]
    }

    /// The coarse volume Σ Ω R² × lowering over the eroded cells, m³.
    fn volume_m3(&self) -> f64 {
        let r2 = self.radius_m * self.radius_m;
        self.eroded
            .iter()
            .map(|&c| self.omega[c] * r2 * self.lowering(c))
            .sum()
    }

    /// `volume_m3` as a global layer, metres.
    fn layer_m(&self, volume_m3: f64) -> f64 {
        let sphere: f64 = self.omega.iter().sum();
        volume_m3 / (sphere * self.radius_m * self.radius_m)
    }

    /// The root mean square by area of `values` over `cells`, metres.
    fn rms(&self, cells: &[usize], values: impl Fn(usize) -> f64) -> f64 {
        let (mut squares, mut area) = (0.0, 0.0);
        for &c in cells {
            let v = values(c);
            squares += self.omega[c] * v * v;
            area += self.omega[c];
        }
        (squares / area).sqrt()
    }

    /// The area quantile `q` of the eroded cells' lowering, metres, and of their lowering over
    /// (B)'s bound: the least value whose cells, with all below it, hold at least `q` of the
    /// eroded area (each sorted by `total_cmp`, then cell index).
    fn area_quantiles(&self, q: f64) -> (f64, f64) {
        let quantile = |value: &dyn Fn(usize) -> f64| {
            let mut cells = self.eroded.clone();
            cells.sort_unstable_by(|&a, &b| value(a).total_cmp(&value(b)).then(a.cmp(&b)));
            let total: f64 = cells.iter().map(|&c| self.omega[c]).sum();
            let mut held = 0.0;
            for &c in &cells {
                held += self.omega[c];
                if held >= q * total {
                    return value(c);
                }
            }
            cells.last().map_or(0.0, |&c| value(c))
        };
        (
            quantile(&|c| self.lowering(c)),
            quantile(&|c| self.lowering(c) / self.bound(c)),
        )
    }

    /// The eroded cells above (B)'s bound, ascending by cell.
    fn above(&self) -> Vec<Above> {
        self.eroded
            .iter()
            .filter(|&&c| self.lowering(c) > self.bound(c))
            .map(|&c| Above {
                cell: c,
                lowering_m: self.lowering(c),
                bound_m: self.bound(c),
            })
            .collect()
    }

    /// The share of the eroded area above (B)'s bound.
    fn share_above(&self) -> f64 {
        let total: f64 = self.eroded.iter().map(|&c| self.omega[c]).sum();
        let above: f64 = self.above().iter().map(|a| self.omega[a.cell]).sum();
        above / total
    }

    /// (B) holds: the area p99 of the eroded cells' lowering over their bound is at most 1.
    fn ceiling_holds(&self) -> bool {
        self.area_quantiles(CEILING_QUANTILE).1 <= 1.0
    }

    /// The cells of full exposure among the eroded, whose final elevation is the solve's at the
    /// surface's exposure.
    fn full_exposure(&self) -> Vec<usize> {
        self.eroded
            .iter()
            .copied()
            .filter(|&c| self.dry.exposure[c].total_cmp(&self.dry.share).is_eq())
            .collect()
    }

    /// Prints the figures under `what`: (A1)'s trunk incision, its p90 and the share deeper than
    /// 350 m, whole and by drainage class ([`DRAINAGE_CLASSES_M2`]); the lowering; (B) as an area
    /// quantile, its maximum, the share above and the cells whose `f_w` reaches 1; (A2); the
    /// residuals by level; and the residual criterion (decision-r09-t14-mars-ceiling §4.2).
    fn print(&self, what: &str) {
        let trunk = self.trunk();
        print_distribution(
            &format!("{what}: (A1) trunk channel incision, eroded cells draining ≥ 10⁴ km²"),
            trunk.clone(),
        );
        eprintln!(
            "{what}: (A1) trunk cells deeper than 350 m: {:.2}% against about {:.1}% of all valleys \
             in a log-normal of Williams and Phillips's depths (derived; a finding for the w_v \
             check and R09.T6.c)",
            100.0 * deep_share(&trunk),
            100.0 * DEEP_VALLEY_SHARE
        );
        for (lo, hi, class) in DRAINAGE_CLASSES_M2 {
            let incision: Vec<f64> = self
                .eroded
                .iter()
                .filter(|&&c| (lo..hi).contains(&self.dry.outcome.drainage_m2[c]))
                .map(|&c| self.incision(c))
                .collect();
            let share = deep_share(&incision);
            match quantiles(incision.clone()) {
                Some(q) => eprintln!(
                    "{what}: (A1) drainage {class}: {} trunk cells, {:.2}% deeper than 350 m, \
                     median {:.1} m, p90 {:.1} m, max {:.1} m",
                    incision.len(),
                    100.0 * share,
                    q[3],
                    q[5],
                    q[6]
                ),
                None => eprintln!("{what}: (A1) drainage {class}: no trunk cells"),
            }
        }
        print_distribution(
            &format!("{what}: channel incision, every eroded cell"),
            self.eroded.iter().map(|&c| self.incision(c)).collect(),
        );
        print_distribution(
            &format!("{what}: coarse lowering, every eroded cell"),
            self.eroded.iter().map(|&c| self.lowering(c)).collect(),
        );
        let (p99, p99_ratio) = self.area_quantiles(CEILING_QUANTILE);
        let max = self
            .eroded
            .iter()
            .map(|&c| self.lowering(c))
            .fold(0.0, num::max);
        let full: Vec<usize> = self
            .eroded
            .iter()
            .copied()
            .filter(|&c| self.f_w(c) >= 1.0)
            .collect();
        eprintln!(
            "{what}: (B) area p99 of the coarse lowering {p99:.2} m against 91.3 m (p99 of the \
             lowering over each cell's bound {p99_ratio:.3}): {}; max {max:.2} m; {:.3}% of the \
             eroded area above its bound ({} cells); {} cells with f_w ≥ 1",
            if p99_ratio <= 1.0 { "holds" } else { "FAILS" },
            100.0 * self.share_above(),
            self.above().len(),
            full.len()
        );
        let volume = self.volume_m3();
        let layer = self.layer_m(volume);
        eprintln!(
            "{what}: (A2) coarse volume {volume:.4e} m³, a global layer of {layer:.4} m against \
             {VALLEY_LAYER_M} m{}",
            if layer > VALLEY_LAYER_FINDING_M {
                ", FINDING: above 6.1 m"
            } else {
                ""
            }
        );
        report(what, &self.dry.outcome);
        self.print_residual_criterion(what);
    }

    /// The residual criterion (decision-r09-t14-mars-ceiling §4.2): the final RMS residual
    /// against half the RMS of the unscaled channel incision, both over the cells of full
    /// exposure, with the all-cells figures beside them.
    fn print_residual_criterion(&self, what: &str) {
        let outcome = &self.dry.outcome;
        let full = self.full_exposure();
        let residual = self.rms(&full, |c| outcome.last_change_m[c]);
        let incision = self.rms(&full, |c| self.incision(c));
        let free: Vec<usize> = (0..self.z0.len())
            .filter(|&c| outcome.bounds[c].is_none())
            .collect();
        eprintln!(
            "{what}: residual criterion over {} cells of full exposure: final RMS residual \
             {residual:.2} m against half the RMS incision {:.2} m: {}; over every cell not bound \
             ({}), residual {:.2} m, half the RMS incision {:.2} m",
            full.len(),
            0.5 * incision,
            if residual > 0.5 * incision {
                "FINDING, the trigger trips"
            } else {
                "below"
            },
            free.len(),
            self.rms(&free, |c| outcome.last_change_m[c]),
            0.5 * self.rms(&free, |c| self.incision(c))
        );
    }

    /// Prints each cell above (B)'s bound under `what`: its lowering and bound, incision,
    /// drainage, the construction's slope to its receiver on the solve's last routing, its crust,
    /// whether it lies in a coarse crater's reach or below a lake's outlet (a donor a bound), and
    /// its `f_w`.
    fn print_above(&self, what: &str, working: &Working) {
        let outcome = &self.dry.outcome;
        let mut reach = vec![false; self.z0.len()];
        for crater in &working.craters {
            for cell in crater.reach.cells() {
                reach[slot(cell)] = true;
            }
        }
        let (mut in_reach, mut area) = (0.0, 0.0);
        for &c in &self.eroded {
            area += self.omega[c];
            if reach[c] {
                in_reach += self.omega[c];
            }
        }
        eprintln!(
            "{what}: {:.1}% of the eroded area lies in a coarse crater's reach",
            100.0 * in_reach / area
        );
        for a in self.above() {
            let cell = cell_of(a.cell);
            let slope = outcome.routing.receiver(cell).map_or(0.0, |link| {
                (self.z0[a.cell] - self.z0[slot(link.cell)]) / link.length_m
            });
            let outlet = outcome
                .routing
                .donors(cell)
                .iter()
                .any(|&d| outcome.bounds[slot(d)].is_some());
            eprintln!(
                "{what}: above (B): cell {} lowering {:.1} m (bound {:.1}), incision {:.1} m, \
                 drainage {:.3e} m², slope {:.4}, {:?}, crater reach {}, lake outlet {outlet}, f_w \
                 {:.3}",
                a.cell,
                a.lowering_m,
                a.bound_m,
                self.incision(a.cell),
                outcome.drainage_m2[a.cell],
                slope,
                working.cells[a.cell].crust,
                reach[a.cell],
                self.f_w(a.cell)
            );
        }
    }
}

/// A world's dry-now erosion: its staging, construction and the branch's result.
fn eroded(inputs: CoarseInputs) -> (Staged, Vec<f64>, DryNow) {
    let staged = Staged::of(inputs);
    let z0 = staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let dry = dry_now(&staged.pass(), &staged.working).expect("its surface saw the epoch");
    (staged, z0, dry)
}

/// Prints the paleo-water's records under `what` (decision-r09-t14-mars-ceiling §3): the bodies
/// (full, terminal and dry playas), the flooded share by crust against Matsubara et al.'s 15–30%
/// of the highlands, and the paleo-sea's share of the sphere.
fn print_paleo_water(what: &str, working: &Working, outcome: &Outcome, omega: &[f64]) {
    let bodies = outcome.bodies.bodies();
    let terminal = bodies.iter().filter(|b| !b.full).count();
    let playas = bodies.iter().filter(|b| b.is_playa()).count();
    let mut flooded = vec![false; working.cells.len()];
    for body in bodies {
        for &c in &body.cells {
            flooded[slot(c)] = true;
        }
    }
    eprintln!(
        "{what}: paleo-water bodies {} ({} full, {terminal} terminal, {playas} dry playas)",
        bodies.len(),
        bodies.iter().filter(|b| b.full).count()
    );
    for crust in [Crust::Lid, Crust::Province] {
        let (mut wet, mut all, mut cells, mut wet_cells) = (0.0, 0.0, 0_usize, 0_usize);
        for (c, cell) in working.cells.iter().enumerate() {
            if cell.crust == crust {
                all += omega[c];
                cells += 1;
                if flooded[c] {
                    wet += omega[c];
                    wet_cells += 1;
                }
            }
        }
        if cells > 0 {
            eprintln!(
                "{what}: {crust:?} flooded {wet_cells} of {cells} cells, {:.1}% of its area{}",
                100.0 * wet / all,
                if crust == Crust::Lid {
                    format!(
                        " (Matsubara et al.'s {:.0}–{:.0}% of the highlands; Q2's static fill, an \
                         upper bound on the paleo-lakes' extent, its cells bounds during the epoch, \
                         recorded beside (A2))",
                        100.0 * HIGHLAND_WATER_SHARE.0,
                        100.0 * HIGHLAND_WATER_SHARE.1
                    )
                } else {
                    String::new()
                }
            );
        }
    }
    let sphere: f64 = omega.iter().sum();
    if let Some(sea) = bodies.iter().max_by_key(|b| b.cells.len()) {
        let area: f64 = sea.cells.iter().map(|&c| omega[slot(c)]).sum();
        eprintln!(
            "{what}: the paleo-sea {} cells at {:.1} m, {:.1}% of the sphere",
            sea.cells.len(),
            sea.level_m,
            100.0 * area / sphere
        );
    }
}

/// One evaluated level's grooves below a coarse cell, as the synthesis's closed form takes them
/// (`hyperion_surface`'s `channels::incision::level_terms_to`, ported for (A0) from the lane's
/// research script `t14c-subcell-volume.py`): ρ metres of groove a square metre, of depth G, floor
/// h (half the bankfull width) and wall W.
#[derive(Debug, Clone, Copy)]
struct Groove {
    level: u8,
    density: f64,
    depth: f64,
    floor: f64,
    wall: f64,
    trunk: bool,
}

impl Groove {
    /// Its mean depth over the cell, ρ 2G (h + W ÷ 2), metres.
    fn mean_m(&self) -> f64 {
        self.density * 2.0 * self.depth * (self.floor + 0.5 * self.wall)
    }

    /// Its variance, max(0, ρ 2G² (h + 13W ÷ 35) − mean²), a tributary's taken at
    /// [`TRIBUTARY_VARIANCE_SHARE`], square metres.
    fn variance_m2(&self) -> f64 {
        let square =
            self.density * 2.0 * self.depth * self.depth * (self.floor + 13.0 * self.wall / 35.0);
        let mean = self.mean_m();
        let v = num::max(square - mean * mean, 0.0);
        if self.trunk {
            v
        } else {
            TRIBUTARY_VARIANCE_SHARE * v
        }
    }
}

/// The mean cell width of `level`, √(4πR² ÷ (6 · 4^level)), metres.
fn mean_cell_width(radius_m: f64, level: u8) -> f64 {
    radius_m * (4.0 * core::f64::consts::PI / 6.0).sqrt() / f64::from(1_u32 << level)
}

/// A channel's bankfull width and depth, metres, draining `area_m2` at `runoff` m a⁻¹ under
/// `gravity`.
fn channel_size(runoff: f64, area_m2: f64, gravity: MetresPerSecondSquared) -> (f64, f64) {
    let area = SquareMetres::new(area_m2);
    let forming = channel_forming_discharge_m3_s(mean_discharge_m3_s(runoff, area), area);
    (
        bankfull_width(forming, gravity).value(),
        bankfull_depth(forming, gravity).value(),
    )
}

/// The grooves below `cell` of `body` to the cut's deepest level (the synthesis's
/// `level_terms_to` with `deepest_level`): the trunk, of the cell's valley depth at its
/// tributaries' steepness held between the bankfull depth and the level's cap, and each finer
/// level's tributaries.
fn grooves(cell: &SynthesisCell, body: ChannelBody) -> Vec<Groove> {
    let (r, level) = (body.radius.value(), body.level);
    let mut out = Vec::new();
    if cell.flow.edge().is_none() {
        return out;
    }
    let channels = cell.steepness != LogSteepness::ZERO && !cell.is_under_water();
    let tributaries = (channels && cell.runoff != LogRunoff::NONE)
        .then(|| {
            tributary_steepness_m0_9(
                cell.steepness.index_m0_9(),
                cell.runoff.metres_per_year(),
                cell.catchment_runoff.metres_per_year(),
            )
        })
        .flatten();
    let wall = VALLEY_WALL_SHARE * mean_cell_width(r, level);
    let cap = VALLEY_WALL_GRADIENT * wall / STEEPEST_WALL;
    let (width, depth) = channel_size(
        cell.catchment_runoff.metres_per_year(),
        cell.drainage.area().value(),
        body.gravity,
    );
    let valley = tributaries.map_or(0.0, |k| valley_depth(k, body.radius, level).value());
    let g = num::min(num::max(valley, depth), cap);
    if g > 0.0 {
        out.push(Groove {
            level,
            density: MEAN_TRUNK_LENGTH / mean_cell_width(r, level),
            depth: g,
            floor: 0.5 * width,
            wall,
            trunk: true,
        });
    }
    let Some(steepness) = tributaries else {
        return out;
    };
    let runoff = cell.runoff.metres_per_year();
    let heads = main_stream_length(channel_head_area(steepness)).value();
    for k in level + 1..=deepest_level(cell, body) {
        let cell_width = mean_cell_width(r, k);
        let wall = VALLEY_WALL_SHARE * cell_width;
        let cap = VALLEY_WALL_GRADIENT * wall / STEEPEST_WALL;
        let length = MEAN_TRIBUTARY_LENGTH * cell_width;
        let head = head_length(body.radius, k).value();
        let middle = head + 0.5 * length;
        let below = if middle > heads {
            tributary_rise(steepness, Metres::new(heads), Metres::new(middle - heads)).value()
        } else {
            0.0
        };
        let area = hack_area(Metres::new(head + length)).value();
        let (width, depth) = channel_size(runoff, area, body.gravity);
        out.push(Groove {
            level: k,
            density: TRIBUTARY_SHARE * length / (cell_width * cell_width),
            depth: num::min(num::max(below, depth), cap),
            floor: 0.5 * width,
            wall,
            trunk: false,
        });
    }
    out
}

/// The synthesis's `channel_variance` of `grooves` in wavelengths shorter than `finer_than_m`,
/// rebuilt: each level's variance in its band share, summed, at [`WHOLE_VARIANCE_SHARE`].
fn rebuilt_variance(grooves: &[Groove], radius_m: f64, finer_than_m: f64) -> f64 {
    let sum: f64 = grooves
        .iter()
        .map(|g| {
            let share = num::max(
                0.0,
                num::min(
                    math::log2(finer_than_m / mean_cell_width(radius_m, g.level)),
                    1.0,
                ),
            );
            share * g.variance_m2()
        })
        .sum();
    WHOLE_VARIANCE_SHARE * sum
}

/// F1 and the ceiling on the reference Mars (decision-r09-t14-choices F1;
/// decision-r09-t14-mars-ceiling §4; the lane's design §4.5 test 4), its whole pass to the erosion
/// and then the step:
///
/// - (A1), asserted: the median channel incision over the eroded cells (not bound, of equivalent
///   area and exposure above zero) draining at least 10⁴ km² lies within Black et al. 2017's
///   50–350 m (187.8 m as built); its p90 and the share deeper than 350 m are recorded, whole and
///   by drainage class, 10⁴–10⁵, 10⁵–10⁶ and above 10⁶ km² ([`DRAINAGE_CLASSES_M2`]). The tail
///   is a finding, not an assertion: 21.2% of the trunk cells lie deeper than 350 m against about
///   4.6% of all valleys in a log-normal of Williams and Phillips 2001's abstract mean and standard
///   deviation of depth (109 ± 137 m; JGR 106, 23737), a derived figure over every valley
///   cross-section, not a trunk measurement, Howard et al.'s trunk range being the like-for-like
///   comparison; the tail is the construction's own, the incision S₀ times the
///   knickpoint's travel, which grows as (r A)^0.45 K t, so that the steeper, larger trunks run
///   deep. It goes to the `w_v` science check and to R09.T6.c, which takes the solver's incision as
///   its valley depth: a depth relation by drainage belongs there, not in K t
///   (decision-r09-t14-choices, "T14.c follow-up" item 4);
/// - (A0), recorded: the coarse volume plus the synthesis's sub-cell term, the trunks' mean depth
///   of T6.b's closed form over each channelled cell, its port pinned by rebuilding
///   `channel_variance` on every channelled cell to 10⁻⁹;
/// - (A2), recorded: the coarse volume against Luo et al.'s 1.2 m, a finding above 6.1 m (2.226
///   m as built); beside it, the paleo-water floods 46.4% of the lid's area under the reference
///   Mars's 156 m of global layer, Q2's static fill, an upper bound on the paleo-lakes' extent,
///   labelled: 46% of the cells are bounds during the epoch (the "T14.c follow-up" item 6);
/// - (B), asserted: the area p99 of the eroded cells' coarse lowering over 350 m × 10 km ÷ each
///   cell's width is at most 1 (88.3 m against 91.3 m at the mean cell, measured, the p99 accepted
///   as the assertion by the "T14.c follow-up" item 6), the maximum and the share above printed
///   (the cells above in [`measure_the_reference_mars_s_ceiling`]);
/// - the residual criterion and the paleo-water's records, printed.
#[test]
fn the_reference_mars_s_trunk_incision_is_black_et_al_s() {
    let (staged, z0, dry) = eroded(reference::mars_like());
    let pass = staged.pass();
    let omega = pass.grid().solid_angles_sr();
    let r = pass.inputs().radius().value();
    let ceiling = Ceiling::new(&dry, &z0, omega, r);
    ceiling.print("reference Mars");
    print_paleo_water("reference Mars", &staged.working, &dry.outcome, omega);
    let mut working = staged.working.clone();
    run(&pass, &mut working);
    let body = ChannelBody {
        radius: pass.inputs().radius(),
        level: pass.level().get(),
        gravity: pass.inputs().craters().gravity(),
    };
    let (mut sub_cell, mut channelled, mut worst) = (0.0, 0_usize, 0.0_f64);
    for (c, record) in budget::records(&working).enumerate() {
        let terms = grooves(&record, body);
        if terms.is_empty() {
            continue;
        }
        channelled += 1;
        let expected = channel_variance(&record, body, Metres::new(PIN_FINER_THAN_M)).value();
        let rebuilt = rebuilt_variance(&terms, r, PIN_FINER_THAN_M);
        let error = if expected > 0.0 {
            (rebuilt / expected - 1.0).abs()
        } else {
            rebuilt.abs()
        };
        worst = num::max(worst, error);
        assert!(
            error <= 1e-9,
            "cell {c}: the rebuilt channel variance {rebuilt} m² is not the synthesis's {expected}"
        );
        if let Some(trunk) = terms.iter().find(|g| g.trunk) {
            sub_cell += trunk.mean_m() * omega[c] * r * r;
        }
    }
    let coarse = ceiling.volume_m3();
    eprintln!(
        "(A0): {channelled} channelled cells (the pin's worst relative error {worst:.2e}); coarse \
         {:.4} m + sub-cell trunks {:.4} m = {:.4} m of global layer",
        ceiling.layer_m(coarse),
        ceiling.layer_m(sub_cell),
        ceiling.layer_m(coarse + sub_cell)
    );
    assert!(channelled > 0);
    let median = ceiling.trunk_median();
    eprintln!("(A1): the trunk median {median:.2} m against Black et al.'s 50–350 m");
    assert!(
        (TRUNK_INCISION_M.0..=TRUNK_INCISION_M.1).contains(&median),
        "the trunk cells' median channel incision is {median} m, outside 50–350 m"
    );
    let (p99, ratio) = ceiling.area_quantiles(CEILING_QUANTILE);
    assert!(
        ceiling.ceiling_holds(),
        "(B): the area p99 of the eroded cells' lowering is {p99} m, {ratio} of its bound"
    );
}

/// The ceiling's measurement (the lane's design §4.4; decision-r09-t14-choices, "The ceiling
/// risk"; decision-r09-t14-mars-ceiling §4.1): the reference Mars staged to the end of the climate
/// step, then the dry-now branch, printing its inputs, its water bodies, [`Ceiling::print`]'s
/// figures, each cell above (B) with its record, the lowering by crust, the trunk cells'
/// construction slope and the branch's wall time (λ is none: no uplift). Asserts nothing.
#[test]
#[ignore = "measurement: the reference Mars's ceiling, R09.T14.c"]
fn measure_the_reference_mars_s_ceiling() {
    let clock = Instant::now();
    let staged = Staged::of(reference::mars_like());
    let staging_s = clock.elapsed().as_secs_f64();
    let pass = staged.pass();
    let inputs = pass.inputs();
    let epoch = inputs
        .wet_epoch()
        .expect("the reference Mars has a wet epoch");
    let years = dry_now_years(epoch, inputs.surface_age()).expect("its surface saw the epoch");
    let erodibility = Erodibility::of(inputs).expect("a wet epoch has a fluid");
    let k = erodibility.per_cell(&staged.working.cells);
    let k_max = k.iter().fold(0.0, |hi, &x| num::max(hi, x));
    let r = inputs.radius().value();
    let water = paleo_water_m3(&pass, epoch);
    let omega = pass.grid().solid_angles_sr();
    let sphere: f64 = omega.iter().sum();
    eprintln!(
        "reference Mars: level {}, {} cells, R {:.1} km, mean cell width {:.2} km; staged to the \
         climate's end in {staging_s:.2} s",
        pass.grid().level(),
        omega.len(),
        r / 1e3,
        r * (sphere / f64::from(pass.grid().len())).sqrt() / 1e3
    );
    eprintln!(
        "epoch: t {years:.4e} yr, runoff {} m a⁻¹, fluid {} at {:.3} kg m⁻³, the lid's surface \
         {:?} and the provinces' {:?}, K up to {k_max:.4e} m^0.1 a⁻¹, K t (r ÷ 1 m a⁻¹)^0.45 \
         {:.4} m^0.1; paleo-water {:.3} m deep",
        epoch.runoff_m_per_year,
        erodibility.fluid().key.as_str(),
        erodibility.fluid().density_kg_m3,
        erodibility.surface(Crust::Lid),
        erodibility.surface(Crust::Province),
        k_max * years * math::powf(epoch.runoff_m_per_year, DRAINAGE_EXPONENT),
        water.iter().sum::<f64>() / (sphere * r * r),
    );
    let z0: Vec<f64> = staged
        .working
        .cells
        .iter()
        .map(|cell| cell.elevation.value())
        .collect();
    let clock = Instant::now();
    let dry = dry_now(&pass, &staged.working).expect("its surface saw the epoch");
    let step_s = clock.elapsed().as_secs_f64();
    print_paleo_water("reference Mars", &staged.working, &dry.outcome, omega);
    let ceiling = Ceiling::new(&dry, &z0, omega, r);
    ceiling.print("reference Mars");
    ceiling.print_above("reference Mars", &staged.working);
    let volume = ceiling.volume_m3();
    for crust in [Crust::Lid, Crust::Province] {
        let cells: Vec<usize> = (0..z0.len())
            .filter(|&c| dry.outcome.bounds[c].is_none() && staged.working.cells[c].crust == crust)
            .collect();
        let share: f64 = cells
            .iter()
            .map(|&c| omega[c] * r * r * ceiling.lowering(c))
            .sum::<f64>()
            / volume;
        print_distribution(
            &format!("by crust {crust:?} (volume share {share:.3}), lowering"),
            cells.iter().map(|&c| ceiling.lowering(c)).collect(),
        );
        print_distribution(
            &format!("by crust {crust:?}, incision"),
            cells.iter().map(|&c| ceiling.incision(c)).collect(),
        );
    }
    let slopes: Vec<f64> = ceiling
        .eroded
        .iter()
        .filter(|&&c| dry.outcome.drainage_m2[c] >= TRUNK_AREA_M2)
        .filter_map(|&c| {
            dry.outcome
                .routing
                .receiver(cell_of(c))
                .map(|link| (z0[c] - z0[slot(link.cell)]) / link.length_m)
        })
        .collect();
    if let Some(q) = quantiles(slopes) {
        eprintln!(
            "trunk cells' construction slope to the final receiver: p10 {:.4}, p25 {:.4}, median \
             {:.4}, p75 {:.4}, p90 {:.4}, max {:.4}",
            q[1], q[2], q[3], q[4], q[5], q[6]
        );
    }
    eprintln!("the branch's wall time {step_s:.2} s");
}

/// The three starts of a finer level on the reference Mars (decision-r09-t14-choices, "T14.b
/// follow-up" item 3), dry now: each level's first and last residual for T14.b's upsample, the
/// construction and the correction, with the solve's trunk median. Asserts nothing.
#[test]
#[ignore = "measurement: the multigrid's level starts on the reference Mars, R09.T14.c"]
fn measure_the_level_starts_on_the_reference_mars() {
    let staged = Staged::of(reference::mars_like());
    let pass = staged.pass();
    let z0: Vec<f64> = staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    for start in [Start::AsBuilt, Start::Construction, Start::Correction] {
        let outcome = erode_dry_now_with(
            &pass,
            &staged.working,
            Options {
                start,
                ..Options::DRY_NOW
            },
        )
        .unwrap();
        let what = format!("reference Mars, {start:?}");
        report(&what, &outcome);
        let trunk: Vec<f64> = (0..z0.len())
            .filter(|&c| {
                outcome.bounds[c].is_none()
                    && outcome.equivalent_area_m2[c] > 0.0
                    && outcome.drainage_m2[c] >= TRUNK_AREA_M2
            })
            .map(|c| z0[c] - outcome.z_m[c])
            .collect();
        let last = outcome.diagnostics.residuals.last().unwrap();
        eprintln!(
            "{what}: finest level first {:.2} m, last {:.2} m; the solve's trunk median {:.2} m",
            last.first_m(),
            last.last_m(),
            quantiles(trunk).map_or(f64::NAN, |q| q[3])
        );
    }
}

/// The reference Mars's slow record (the lane's design §4.5 test 6; decision-r09-t14-mars-ceiling
/// §4.2), recorded and not asserted: its figures at level 7 and at level 8 (the reference Mars
/// staged and eroded on a level-8 grid); the cell-mean lowering of one basin, the level-7 run's
/// largest by eroded cells, against the level-8 run's lowering and unscaled incision aggregated to
/// its level-7 parents by solid angle; and the coarse volume, trunk median and (B) at the ends of
/// Hoke, Hynek and Tucker 2011's range, 10⁵ yr at 0.073 m a⁻¹ and 10⁸ yr at 1.10 m a⁻¹
/// (science-hoke §4).
///
/// **The dependence on resolution, a finding for R09.T6.c and the `w_v` check**
/// (decision-r09-t14-choices, "T14.c follow-up" item 6): at level 8 (cells of about 19 km), (B)'s
/// p99 is 1.94 times its bound and (A2) is 5.64 m, against 0.97 and 2.226 m at level 7. Every
/// coarse cell carries a trunk, so a finer level carries four times the valley length and the void
/// per area grows with resolution. Not a blocker, since a Mars-sized body is always level 7; but
/// any world needs the closure and the network invariant with the level: a cell carries a valley,
/// and the closure, only where its drainage exceeds the epoch's channel-initiation area, which a
/// larger world dry now, at level 8, depends on.
#[test]
#[ignore = "slow: the reference Mars at levels 7 and 8 and two more epochs, R09.T14.c"]
fn the_reference_mars_s_slow_record() {
    let (staged, z0, dry) = eroded(reference::mars_like());
    let pass = staged.pass();
    let omega = pass.grid().solid_angles_sr();
    let r = pass.inputs().radius().value();
    let coarse = Ceiling::new(&dry, &z0, omega, r);
    coarse.print("level 7");
    let outcome = &dry.outcome;
    let mut basins = vec![0_usize; z0.len()];
    for &c in &coarse.eroded {
        basins[slot(outcome.routing.root_of(cell_of(c)))] += 1;
    }
    let basin = (0..basins.len())
        .max_by_key(|&b| (basins[b], core::cmp::Reverse(b)))
        .expect("a basin");
    let in_basin: Vec<usize> = coarse
        .eroded
        .iter()
        .copied()
        .filter(|&c| slot(outcome.routing.root_of(cell_of(c))) == basin)
        .collect();
    let fine_staged = Staged::at(reference::mars_like(), CoarseLevel::new(8).unwrap());
    let fine_z0: Vec<f64> = fine_staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let fine = dry_now(&fine_staged.pass(), &fine_staged.working).unwrap();
    let fine_pass = fine_staged.pass();
    let fine_omega = fine_pass.grid().solid_angles_sr();
    let fine_ceiling = Ceiling::new(&fine, &fine_z0, fine_omega, r);
    fine_ceiling.print("level 8");
    let (mut lowering, mut area) = (0.0, 0.0);
    let (mut fine_lowering, mut fine_incision, mut fine_area) = (0.0, 0.0, 0.0);
    for &c in &in_basin {
        lowering += omega[c] * coarse.lowering(c);
        area += omega[c];
        for (child, &w) in fine_omega.iter().enumerate().skip(4 * c).take(4) {
            fine_lowering += w * fine_ceiling.lowering(child);
            fine_incision += w * fine_ceiling.incision(child);
            fine_area += w;
        }
    }
    eprintln!(
        "one basin (root {basin}, {} eroded level-7 cells): mean L_i {:.2} m at level 7; at level \
         8 aggregated to its parents, mean lowering {:.2} m, mean unscaled incision {:.2} m",
        in_basin.len(),
        lowering / area,
        fine_lowering / fine_area,
        fine_incision / fine_area
    );
    for (flow, runoff) in [(1.0e5, 0.073), (1.0e8, 1.10)] {
        let base = reference::mars_like();
        let epoch = WetEpoch {
            effective_flow: Years::new(flow),
            runoff_m_per_year: runoff,
            ..*base.wet_epoch().unwrap()
        };
        let inputs = builder_of(&base).wet_epoch(epoch).build().unwrap();
        let (staged, z0, dry) = eroded(inputs);
        let pass = staged.pass();
        let ceiling = Ceiling::new(&dry, &z0, pass.grid().solid_angles_sr(), r);
        let volume = ceiling.volume_m3();
        let (p99, ratio) = ceiling.area_quantiles(CEILING_QUANTILE);
        eprintln!(
            "{flow:.0e} yr at {runoff} m a⁻¹: coarse volume {volume:.4e} m³, {:.3} m of global \
             layer; trunk median {:.2} m; (B) area p99 {p99:.2} m ({ratio:.3} of the bound)",
            ceiling.layer_m(volume),
            ceiling.trunk_median()
        );
    }
}
