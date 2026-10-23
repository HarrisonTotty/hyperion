//! The erosion step's branches: what each world solves (R09.T14.c; the lane's design §4.2, as
//! decision-r09-t14-choices rules it).
//!
//! [`Branch::of`] sorts a world by its wet epoch:
//!
//! - **Never wet** (no epoch): the step does nothing. Every cell stays terminal, with no drainage,
//!   steepness or runoff, so that no channel is cut on a Moon (decision-r09-t14-choices, "T14.d
//!   follow-up" C).
//! - **Dry now** (an epoch that ended before now): the solver's transient (Tzathas et al. 2024's
//!   eq. 8) for the epoch's effective flow, scaled by the share of the epoch the surface saw, with
//!   no uplift, the epoch's runoff on every cell, and the paleo-inventory spread over the cells
//!   and routed by Fill–Spill–Merge, its bodies the bounds; a surface no older than the epoch's
//!   end is left as it is (Q2 and Q4). The cells inside the rim of a crater younger than the
//!   epoch's end erode with the rest, and shed no runoff of their own afterwards (Q4); a crater
//!   formed inside the epoch erodes for the whole t (labelled: no per-cell age is modelled). The
//!   volcanic provinces take their own surface's exposure and bed, renewed after the solve where
//!   they are younger than the epoch, and every eroded cell lowers by its valley's void, not its
//!   channel's whole incision (decision-r09-t14-mars-ceiling §2.1–2.2).
//! - **Wet now** (an epoch that runs to now): the steady state (eq. 9) under each basin's uplift in
//!   proportion to its freeboard, a field fixed in space (Q1, as its "T14.c follow-up" item 1 rules
//!   it), each construction basin's mean elevation held by an additive refit and the receivers
//!   sticky ("T14.c follow-up 2" §1 and §3), no link rising more than the debris-flow slope times
//!   its length (the "T14.c follow-up" item 2), the climate's annual runoff the source, ice cells
//!   shedding it as it is (Q5), the sea's cells the bounds, and the construction's lakes by their
//!   water balance, the runoff from the land outside the lake against the lake's evaporation less
//!   its precipitation (Matsubara et al. 2011's X ratio), a terminal lake's bed a bound, a full
//!   lake routed through and graded, and a lake smaller than
//!   one cell a dry playa ([`WaterBalance`], Q3; [`wet_now_full_bodies`], "T14.d lakes"); its
//!   absolute heights then remapped in order onto its construction's, each cell no lower than its
//!   receiver and no link steeper than the threshold ([`remap_onto_construction`], "T14.c
//!   follow-up 2" §2 and "T14.c follow-up 3" item 3).
//!
//! Each choice is one function whose documentation cites the ruling's question.

use hyperion_surface::num;

use hyperion_surface::field::{Crust, LogIncision, LogSteepness};
use hyperion_surface::synth::channels::{
    ChannelBody, DEBRIS_FLOW_SLOPE, EpochValley, MEAN_TRUNK_LENGTH, bankfull_depth, bankfull_width,
    channel_forming_discharge_m3_s, mean_discharge_m3_s, valley_depth,
};

use super::super::climate::after_erosion::NetLoss;
use super::super::climate::annual_runoff_kg_m2_s;
use super::super::climate::seas::under_sea;
use super::super::craters::waits;
use super::super::plates::{cell_of, slot};
use super::super::{CellState, Pass, Working};
use super::depressions::Measure;
use super::drainage::canonical;
use super::erodibility::{Erodibility, Fluid};
use super::multigrid::{self, FullBodies, Options, Outcome, Problem, Uplift, Water};
use super::solver::{self, Cells, Threshold, Time};
use crate::math;
use crate::planetary::surface::inputs::{CoarseInputs, WetEpoch};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Gigayears, Metres, MetresPerSecondSquared, SquareMetres};

/// Which branch of the step a world takes, by its wet epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Branch {
    /// No wet epoch: the step does nothing.
    NeverWet,
    /// A wet epoch that ended before now (its end above zero).
    DryNow,
    /// A wet epoch that runs to now (its end zero).
    WetNow,
}

impl Branch {
    /// The branch of `inputs`: never wet without a wet epoch, wet now where the epoch ends at
    /// zero, dry now otherwise (the lane's design §4.2; the same test as the erodibility's
    /// [`Fluid::of`]).
    #[must_use]
    pub(crate) fn of(inputs: &CoarseInputs) -> Self {
        match inputs.wet_epoch() {
            None => Self::NeverWet,
            Some(epoch) if epoch.end.value() <= 0.0 => Self::WetNow,
            Some(_) => Self::DryNow,
        }
    }
}

/// The share of the wet epoch `epoch`'s flow that a surface `surface_age` old saw, 0 to 1, or
/// `None` where the surface is no older than the epoch's end (decision-r09-t14-choices Q4).
///
/// Where the surface age is at most the epoch's end the whole surface is younger than the epoch,
/// and the step leaves it untouched. Where it falls inside the epoch (end < age < start) the
/// surface saw (age − end) ÷ (start − end) of it; an older surface saw all of it, and so does any
/// surface older than a zero-length epoch. The reference Mars (surface 3.9 Ga, epoch 3.9–3.6 Ga)
/// takes 1.
#[must_use]
pub(crate) fn epoch_share(epoch: &WetEpoch, surface_age: Gigayears) -> Option<f64> {
    let (start, end, age) = (epoch.start.value(), epoch.end.value(), surface_age.value());
    if age <= end {
        return None;
    }
    let span = start - end;
    if span <= 0.0 {
        return Some(1.0);
    }
    Some(num::min(1.0, (age - end) / span))
}

/// The solver's time on a world dry now, Earth-equivalent years: the wet epoch's effective flow
/// (Design note 9, as R09.T0.b corrects it) times the share of the epoch the surface saw
/// ([`epoch_share`]), or `None` where the surface is no older than the epoch's end
/// (decision-r09-t14-choices Q4).
///
/// One t stands for the whole surface. Labelled (Q4): a crater formed inside the epoch erodes for
/// the whole t, since no per-cell age is modelled, and lava plains younger than the epoch are not
/// modelled either (the provinces aside, [`cell_exposure`]).
#[must_use]
pub(crate) fn dry_now_years(epoch: &WetEpoch, surface_age: Gigayears) -> Option<f64> {
    epoch_share(epoch, surface_age).map(|share| epoch.effective_flow.value() * share)
}

/// Each cell's own runoff on a world dry now, m a⁻¹: the wet epoch's on every cell, one a cell of
/// `cells` (decision-r09-t14-choices Q4).
///
/// No past climate is modelled, so the epoch's runoff is uniform (Design note 9; science-hoke
/// §3). The ground under a crater younger than the epoch's end was ordinary eroding ground during
/// it, so those cells take it too and are not bounds: the solver erodes them, the crater is
/// smoothed in untouched afterwards, and only the output's own runoff is zero inside its rim
/// ([`output_runoff_m_per_year`]).
#[must_use]
pub(crate) fn dry_now_runoff_m_per_year(epoch: &WetEpoch, cells: usize) -> Vec<f64> {
    vec![epoch.runoff_m_per_year; cells]
}

/// Each cell's paleo-water on a world dry now, m³, in cell-index order of `pass`'s grid: the wet
/// epoch's paleo-inventory as a uniform depth, its mass over the sphere divided by the epoch
/// liquid's density at its triple point ([`Fluid::of_epoch`]), times the cell's area, its solid
/// angle times R² (decision-r09-t14-choices Q2).
///
/// Fill–Spill–Merge (Barnes, Callaghan and Wickert 2021) routes it: with no ocean the sphere's
/// root fills to the level its volume reaches, the lowest, largest basin the paleo-sea, smaller
/// basins filling and spilling, and every water cell a bound whose surface is its donors' base
/// level. Nothing is written as a water surface: the world is dry now. Labelled: no evaporation,
/// infiltration or ground ice during the epoch, so the paleo-lakes are as full as the inventory
/// allows, about 1.5–3 times the highland standing water of Matsubara et al. 2011's X of 3–7,
/// which also holds each closed basin's base level where its infill would have raised it
/// (decision-r09-t14-mars-ceiling §3; lakes by the balance enter with closed-basin deposition, a
/// later item); and the inventory stands all at once, not episode by episode.
#[must_use]
pub(crate) fn paleo_water_m3(pass: &Pass<'_>, epoch: &WetEpoch) -> Vec<f64> {
    let depth_m = epoch.paleo_inventory.value() / Fluid::of_epoch(epoch).density_kg_m3;
    let r = pass.inputs().radius().value();
    let r2 = r * r;
    pass.grid()
        .solid_angles_sr()
        .iter()
        .map(|&omega| depth_m * (omega * r2))
        .collect()
}

/// Each cell of `working` that is a sea cell for the solver: under liquid now, its water surface
/// above its ground (the relief step's flood, `seas::under_sea`), a bound whose base is the sea's
/// level. On a world wet now it is the ocean, the base level of every basin draining to it
/// (decision-r09-t14-choices Q1's `ℓ_b`); on a world dry now there is none on every built world
/// (the reference Mars has none), and the paleo-water's bodies are the bounds (Q2).
#[must_use]
pub(crate) fn sea_cells(working: &Working) -> Vec<bool> {
    working.cells.iter().map(under_sea).collect()
}

/// Each cell's exposure on a world dry now, 0 to 1: Q4's share of the wet epoch `epoch`
/// ([`epoch_share`], 0 where none) at its own surface's age, the provinces' surface age
/// ([`CoarseInputs::province_surface_age`]) for a [`Crust::Province`] cell and the surface age
/// for every other (decision-r09-t14-mars-ceiling §2.1).
///
/// One age stands for all the provinces: their histories one by one are not modelled, so Mars's
/// dissected old volcanoes (Alba Patera, Ceraunius and Hecates Tholus) come out undissected, and
/// partial resurfacing is not modelled either (labelled).
#[must_use]
pub(crate) fn cell_exposure(
    inputs: &CoarseInputs,
    epoch: &WetEpoch,
    cells: &[CellState],
) -> Vec<f64> {
    let share = |age: Gigayears| epoch_share(epoch, age).unwrap_or(0.0);
    let (world, province) = (
        share(inputs.surface_age()),
        share(inputs.province_surface_age()),
    );
    cells
        .iter()
        .map(|cell| match cell.crust {
            Crust::Province => province,
            Crust::Continental | Crust::Oceanic | Crust::Lid => world,
        })
        .collect()
}

/// The coarse cell's lowering on a world dry now, metres: the void of the valley R09.T6.c's
/// synthesis draws in it, one trunk's epoch valley `valley` ([`EpochValley`]) cut into an upland
/// that stays at its construction, over the cell, `L` = min(d, ℓ G (`w_bf` + W) ÷ w²) with ℓ the
/// trunk's mean length in the cell, [`MEAN_TRUNK_LENGTH`] cells of width `cell_m` w, √(Ω R²), and d
/// the channel's incision `incision_m` at full K (decision-r09-t14-mars-ceiling §2.2 and §5;
/// decision-r09-t14-choices F2).
///
/// G is the lesser of R09.T6.b's steady-state groove and the epoch's incision as the field codes
/// it, and the walls rise 5 G each at most
/// ([`WALL_RUN_PER_DEPTH`](hyperion_surface::synth::channels::WALL_RUN_PER_DEPTH)), so that its top width is
/// `w_bf` + 10G: the cell lowers by the void the synthesis cuts, whose mean it adds back to the
/// upland (T6.b's zero-mean rule), and the upland stays at z0. A dry-now cell carries its trunk
/// alone (main's ruling for R09.T6.c, option A), so the trunk is every valley it draws. Capped at
/// d, where the valley is as wide as its cell, the space-filling limit. K, t and the runoff never
/// move to pass the ceiling; the channel's incision is kept unscaled apart
/// ([`CellState::channel_incision`]).
///
/// [`CellState::channel_incision`]: super::super::CellState::channel_incision
#[must_use]
pub(crate) fn dry_now_lowering_m(valley: &EpochValley, incision_m: f64, cell_m: f64) -> f64 {
    if incision_m <= 0.0 || valley.depth().value() <= 0.0 {
        return 0.0;
    }
    num::min(incision_m, MEAN_TRUNK_LENGTH * valley.void_m2() / cell_m)
}

/// The valley R09.T6.c's synthesis draws on a dry-now cell's trunk, as the branch knows it before
/// the final routing ([`EpochValley::of`]): its steady-state depth at the steepness index
/// `steepness_m0_9` (m^0.9, the cell's own, the epoch's runoff being uniform), its bankfull channel
/// at the wet epoch's runoff `runoff_m_per_year` over its drainage area `drainage_m2`, and its
/// incision `incision_m` as the field codes it ([`LogIncision`]); `body` the field's.
///
/// The synthesis reads the same from the final field's codes: its routing after the re-match and
/// the late craters, the quantised steepness, drainage and catchment runoff. The two differ only
/// where those move, and only where the steady-state depth binds (the plan's "Deviations in T6.c,
/// as built").
///
/// # Panics
///
/// If an input is negative or not finite.
#[must_use]
pub(crate) fn drawn_valley(
    steepness_m0_9: f64,
    runoff_m_per_year: f64,
    drainage_m2: f64,
    incision_m: f64,
    body: ChannelBody,
) -> EpochValley {
    let area = SquareMetres::new(drainage_m2);
    let forming =
        channel_forming_discharge_m3_s(mean_discharge_m3_s(runoff_m_per_year, area), area);
    let valley = if steepness_m0_9 > 0.0 {
        valley_depth(steepness_m0_9, body.radius, body.level)
    } else {
        Metres::ZERO
    };
    let coded = LogIncision::from_metres(Metres::new(incision_m))
        .expect("a finite, non-negative incision has a code, the last saturating")
        .incision()
        .expect("a world dry now's incision code is the epoch's");
    EpochValley::of(
        valley,
        bankfull_width(forming, body.gravity),
        bankfull_depth(forming, body.gravity),
        coded,
        body,
    )
}

/// A dry-now cell's channel steepness index on the construction, m^0.9, as the branch can know it
/// before the final routing: its fall to its receiver on the solve's last routing over the link's
/// length, held at zero where the receiver stands higher, times its drainage area there to the
/// θ ([`LogSteepness::THETA`]), `write_routing`'s index on the construction `z0`.
#[must_use]
fn construction_steepness_m0_9(outcome: &Outcome, z0: &[f64], c: usize) -> f64 {
    outcome.routing.receiver(cell_of(c)).map_or(0.0, |link| {
        let slope = num::max(0.0, z0[c] - z0[slot(link.cell)]) / link.length_m;
        num::assert_finite(slope * math::powf(outcome.drainage_m2[c], LogSteepness::THETA))
    })
}

/// The floor of a dry-now cell's valley, metres: the bankfull width of its channel at the wet
/// epoch's runoff `runoff_m_per_year` over its drainage area `drainage_m2`, under `gravity`
/// (`hyperion_surface`'s R09.T6.b channel: [`bankfull_width`] at the
/// [`channel_forming_discharge_m3_s`] of the [`mean_discharge_m3_s`];
/// decision-r09-t14-mars-ceiling §2.2): 36–350 m from 1.5 × 10³ to 10⁶ km² on the reference Mars.
#[must_use]
pub(crate) fn valley_floor_m(
    runoff_m_per_year: f64,
    drainage_m2: f64,
    gravity: MetresPerSecondSquared,
) -> f64 {
    let area = SquareMetres::new(drainage_m2);
    let mean = mean_discharge_m3_s(runoff_m_per_year, area);
    bankfull_width(channel_forming_discharge_m3_s(mean, area), gravity).value()
}

/// A world dry now's erosion: the multigrid's outcome, and per cell its exposure, its channel's
/// incision and the elevation the branch writes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DryNow {
    /// The multigrid's solve at the surface's own exposure.
    pub(crate) outcome: Outcome,
    /// The epoch's share the surface saw, the solve's.
    pub(crate) share: f64,
    /// Each cell's own exposure ([`cell_exposure`]).
    pub(crate) exposure: Vec<f64>,
    /// Each cell's channel incision, z0 less its solved channel at its own exposure, metres: the
    /// solve's where its exposure is the surface's, a renewed cell's at its own (zero where it saw
    /// none).
    pub(crate) incision_m: Vec<f64>,
    /// Each cell's valley floor ([`valley_floor_m`]) where it is incised, metres; zero elsewhere.
    pub(crate) floor_m: Vec<f64>,
    /// Each cell inside a waiting crater's rim ([`inside_waiting_rims`]): its ground formed after
    /// the epoch's flow, so it sheds no runoff of its own and carries no epoch valley.
    pub(crate) young: Vec<bool>,
    /// Each cell's drawn valley's depth G ([`drawn_valley`]), metres: zero inside a waiting rim
    /// and where the epoch cut nothing.
    pub(crate) valley_m: Vec<f64>,
    /// Each cell's coarse lowering ([`dry_now_lowering_m`]), metres.
    pub(crate) lowering_m: Vec<f64>,
    /// Each cell's elevation as written, z0 less its lowering, metres; a bound's its ground.
    pub(crate) elevation_m: Vec<f64>,
}

impl DryNow {
    /// Each cell's own runoff in the output, m a⁻¹: the epoch's, but none on the young surface,
    /// inside a waiting crater's rim (`young`, [`inside_waiting_rims`]) or of an exposure below the
    /// surface's, renewed after the epoch (decision-r09-t14-choices Q4;
    /// decision-r09-t14-mars-ceiling §2.1).
    #[must_use]
    pub(crate) fn output_runoff_m_per_year(&self, epoch: &WetEpoch, young: &[bool]) -> Vec<f64> {
        let renewed: Vec<bool> = young
            .iter()
            .zip(&self.exposure)
            .map(|(&late, &exposure)| late || exposure < self.share)
            .collect();
        output_runoff_m_per_year(&dry_now_runoff_m_per_year(epoch, renewed.len()), &renewed)
    }

    /// Each cell's channel incision in the output, metres: [`incision_m`](Self::incision_m), but
    /// zero inside a waiting crater's rim ([`young`](Self::young)), whose ground formed after the
    /// epoch's flow, so that a late crater's ground carries no epoch valley
    /// ([`LogIncision::UNCUT`] in the field; R09.T6.c), and none is taken from it
    /// ([`lowering_m`](Self::lowering_m)). The solve is untouched.
    #[must_use]
    pub(crate) fn output_incision_m(&self) -> Vec<f64> {
        self.incision_m
            .iter()
            .zip(&self.young)
            .map(|(&incision, &late)| if late { 0.0 } else { incision })
            .collect()
    }
}

/// A world dry now's erosion on `working` as the climate step left it, or `None` where the
/// surface is no older than its wet epoch's end (decision-r09-t14-choices Q2 and Q4;
/// decision-r09-t14-mars-ceiling §2.1–2.2): the solve ([`erode_dry_now`]) at the surface's own
/// exposure on every cell; then each cell of an exposure below it renewed, its channel the
/// transient at its own exposure's time on the solve's last routing (its construction to the bit
/// where it saw none: the reference Mars's provinces, its surface renewed after the epoch, as the
/// late craters are placed on eroded ground); then each eroded cell outside a waiting crater's rim
/// lowered by the void of the valley the synthesis draws in it ([`dry_now_lowering_m`]).
///
/// A cell upstream of a renewed one keeps the incision its characteristic took through the
/// renewed ground during the epoch, and may end below it: a valley embayed by younger lava, which
/// R09.T14.d's final routing fills (intended). Bouley et al. 2016's pre-Tharsis routing is not
/// modelled (labelled).
///
/// # Panics
///
/// As [`erode_dry_now`].
#[must_use]
pub(crate) fn dry_now(pass: &Pass<'_>, working: &Working) -> Option<DryNow> {
    let inputs = pass.inputs();
    let epoch = inputs.wet_epoch().expect("a world dry now has a wet epoch");
    let share = epoch_share(epoch, inputs.surface_age())?;
    let outcome = erode_dry_now(pass, working)?;
    let exposure = cell_exposure(inputs, epoch, &working.cells);
    let z0: Vec<f64> = working
        .cells
        .iter()
        .map(|cell| cell.elevation.value())
        .collect();
    let solved = renewed_channels(pass, working, &outcome, &z0, &exposure, share);
    let young = inside_waiting_rims(pass, working);
    let omega = pass.grid().solid_angles_sr();
    let r = inputs.radius().value();
    let body = ChannelBody {
        radius: inputs.radius(),
        level: pass.level().get(),
        gravity: inputs.craters().gravity(),
    };
    let n = z0.len();
    let (mut incision_m, mut floor_m) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut valley_m, mut lowering_m) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let mut elevation_m = Vec::with_capacity(n);
    for c in 0..n {
        let d = num::max(0.0, z0[c] - solved[c]);
        let floor = if d > 0.0 {
            valley_floor_m(
                epoch.runoff_m_per_year,
                outcome.drainage_m2[c],
                inputs.gravity(),
            )
        } else {
            0.0
        };
        let (valley, lowering) = if d > 0.0 && !young[c] {
            let drawn = drawn_valley(
                construction_steepness_m0_9(&outcome, &z0, c),
                epoch.runoff_m_per_year,
                outcome.drainage_m2[c],
                d,
                body,
            );
            (
                drawn.depth().value(),
                dry_now_lowering_m(&drawn, d, r * omega[c].sqrt()),
            )
        } else {
            (0.0, 0.0)
        };
        incision_m.push(d);
        floor_m.push(floor);
        valley_m.push(valley);
        lowering_m.push(lowering);
        elevation_m.push(z0[c] - lowering);
    }
    Some(DryNow {
        outcome,
        share,
        exposure,
        incision_m,
        floor_m,
        young,
        valley_m,
        lowering_m,
        elevation_m,
    })
}

/// Each cell's solved channel, metres: the outcome's where its exposure is the surface's `share`;
/// its construction to the bit where it saw none; and otherwise the transient at its own
/// exposure's time, `effective_flow` × its exposure, on the outcome's last routing with the
/// solve's own equivalent areas, erodibilities and bounds, one solve for each such exposure in
/// ascending order (decision-r09-t14-mars-ceiling §2.1).
#[must_use]
fn renewed_channels(
    pass: &Pass<'_>,
    working: &Working,
    outcome: &Outcome,
    z0: &[f64],
    exposure: &[f64],
    share: f64,
) -> Vec<f64> {
    let mut solved = outcome.z_m.clone();
    let mut partial: Vec<f64> = exposure
        .iter()
        .copied()
        .filter(|&x| x > 0.0 && x < share)
        .collect();
    partial.sort_unstable_by(f64::total_cmp);
    partial.dedup_by(|a, b| a.total_cmp(b).is_eq());
    let erodibility = Erodibility::of(pass.inputs())
        .expect("a world with a wet epoch has a fluid")
        .per_cell(&working.cells);
    let effective_flow = pass
        .inputs()
        .wet_epoch()
        .expect("a world dry now has a wet epoch")
        .effective_flow
        .value();
    let uplift = vec![0.0; z0.len()];
    for &x in &partial {
        let cells = Cells {
            z0_m: z0,
            uplift_m_per_year: &uplift,
            erodibility_m0_1_per_year: &erodibility,
            equivalent_area_m2: &outcome.equivalent_area_m2,
            fixed: &outcome.bounds,
        };
        let renewal = solver::solve(&outcome.routing, &cells, Time::Years(effective_flow * x));
        for (c, &own) in exposure.iter().enumerate() {
            if own.total_cmp(&x).is_eq() {
                solved[c] = renewal.elevation_m(cell_of(c), 0.0);
            }
        }
    }
    for (c, &own) in exposure.iter().enumerate() {
        if own <= 0.0 {
            solved[c] = z0[c];
        }
    }
    solved
}

/// The erosion of a world dry now, on `working` as the climate step left it, or `None` where the
/// surface is no older than its wet epoch's end (decision-r09-t14-choices Q2 and Q4).
///
/// The multigrid's transient ([`Time::Years`] of [`dry_now_years`]) with no uplift
/// ([`Uplift::None`]: the construction is "the initial surface rather than a rate"), the epoch's
/// runoff on every cell ([`dry_now_runoff_m_per_year`]), K of each cell's crust under the epoch's
/// liquid ([`Erodibility`]), the paleo-water ([`paleo_water_m3`]) as a volume, and the sea's
/// cells ([`sea_cells`], none on the built worlds) and the paleo-sea's and paleo-lakes' beds the
/// bounds, the full ones too ([`dry_now_full_bodies`]).
///
/// # Panics
///
/// If `pass`'s world has no wet epoch, or as [`multigrid::erode`].
#[must_use]
pub(crate) fn erode_dry_now(pass: &Pass<'_>, working: &Working) -> Option<Outcome> {
    erode_dry_now_with(pass, working, Options::DRY_NOW)
}

/// [`erode_dry_now`] with the multigrid run as `options` say.
///
/// # Panics
///
/// As [`erode_dry_now`].
#[must_use]
pub(crate) fn erode_dry_now_with(
    pass: &Pass<'_>,
    working: &Working,
    options: Options,
) -> Option<Outcome> {
    let inputs = pass.inputs();
    let epoch = inputs.wet_epoch().expect("a world dry now has a wet epoch");
    let years = dry_now_years(epoch, inputs.surface_age())?;
    let runoff = dry_now_runoff_m_per_year(epoch, working.cells.len());
    let water = paleo_water_m3(pass, epoch);
    Some(erode(
        pass,
        working,
        &runoff,
        Water::Volume { water_m3: &water },
        (
            Uplift::None,
            Time::Years(years),
            dry_now_threshold(),
            dry_now_full_bodies(),
        ),
        options,
    ))
}

/// The threshold slope on a world dry now: none (decision-r09-t14-choices, "T14.c follow-up" item
/// 2). The transient never raises a cell, and a threshold there would erode crater walls, which no
/// fluvial process cuts.
#[must_use]
pub(crate) const fn dry_now_threshold() -> Threshold {
    Threshold::None
}

/// A full paleo-lake on a world dry now: a bound of the transient, its ground untouched, its level
/// its donors' base (decision-r09-t14-choices Q2, as built; its "T14.d lakes" leaves it so).
#[must_use]
pub(crate) const fn dry_now_full_bodies() -> FullBodies {
    FullBodies::Bounds
}

/// The solver's time on a world wet now: the steady state (decision-r09-t14-choices Q1).
///
/// The transient on the epoch's effective flow is degenerate there, since that is the span
/// (4.4 × 10⁹ yr on the reference Earth), over which every path is planed to its base level; the
/// brainstorm's "runs to steady state, with the sea as base level and uplift from the plates".
#[must_use]
pub(crate) const fn wet_now_time() -> Time {
    Time::Steady
}

/// The rock uplift on a world wet now: each basin's freeboard on its construction, scaled to keep
/// its construction's mean height, `λ_b` max(0, z0 − `ℓ_b`) (decision-r09-t14-choices Q1, Pinet
/// and Souriau 1988's denudation in proportion to a basin's mean elevation; a labelled closure,
/// medium-low confidence).
#[must_use]
pub(crate) const fn wet_now_uplift() -> Uplift {
    Uplift::BasinFreeboard
}

/// The threshold slope on a world wet now: no link of the steady state rises more than
/// [`DEBRIS_FLOW_SLOPE`] (0.1, the synthesis's own constant) times its length
/// (decision-r09-t14-choices, "T14.c follow-up" item 2).
///
/// Stream power alone has no relief bound at its ridges: its solutions "lead to singularities
/// close to the ridges … which are therefore less impacted by erosion and eventually degenerate
/// into infinitely steep slopes. In practice, fluvial erosion is negligible or even not applicable
/// in these locations, in which other erosion processes dominate" (Tzathas et al. 2024, CGF
/// 43(2), §2). This is their §6.2 threshold term (eqs. 27–29) in its hard limit, `k_t` → ∞:
/// wherever the fluvial steady state would be steeper, the bed is held at the threshold. At the
/// coarse cell the threshold is the channel's, the slope above which debris flows, not rivers,
/// cut the valleys: the stream power law's steady state, channel slope as an inverse power of
/// drainage area, holds only below it, "this inverse power law rarely extends to slopes greater
/// than ∼0.03 to 0.10, values below which debris flows rarely travel" (Stock and Dietrich 2003,
/// WRR 39(4), 1089, doi:10.1029/2001WR001057, abstract). Tzathas et al.'s hillslope term
/// (§6.1) is negligible at the coarse cell, and their threshold of 0.57 (30°) is a hillslope's,
/// above the channel's. Labelled: one threshold on every body; debris flows' threshold under
/// another gravity or liquid is not modelled.
#[must_use]
pub(crate) const fn wet_now_threshold() -> Threshold {
    Threshold::Slope(DEBRIS_FLOW_SLOPE)
}

/// The lakes' water balance on a world wet now (decision-r09-t14-choices Q3): each cell's own
/// inflow and its net loss over open liquid, one a cell in cell-index order, which Fill–Spill–Merge
/// spreads ([`Measure::Balance`]), on the construction in the solve ([`wet_now_water`]) and on the
/// final field for its lakes (`outputs`).
///
/// - **The inflow** is the cell's own runoff r times its area, its solid angle times R², m³ a⁻¹:
///   the catchment's runoff, with what upstream bodies spill, is Fill–Spill–Merge's to gather.
/// - **The net loss** is (E − P) + r, metres of the main liquid a Julian year. E − P is the
///   climate's ([`NetLoss::at_water_surfaces`]): E the Priestley–Taylor evaporation of the
///   climate condensable's own open liquid were it to stand at the cell's water surface (its
///   ground where it is dry, the sea's level over the sea: the level the fill reaches is not known
///   before it, so a lake's cell is taken at its ground, a labelled error of Γ times the lake's
///   depth, under a kelvin on the coarse cells), none where it is frozen, and P its climate cell's
///   precipitation. It may be negative, where the cell gains more than it loses.
/// - **The cell's own runoff in the loss** takes back, from a cell the lake floods, the runoff the
///   inflow counted on it, exactly when the lake floods it: a flooded cell sheds no runoff, its
///   precipitation falling on the lake, where E − P counts it. So a lake's inflow is the runoff
///   from the land outside it, and a terminal lake's area `A_L` in a closed basin of area `A_B`
///   balances r (`A_B` − `A_L`) = (E − P) `A_L`, the X ratio of closed-basin hydrology,
///   `A_L` ÷ (`A_B` − `A_L`) = r ÷ (E − P) (decision-r09-t14-choices Q3; Matsubara, Howard and
///   Drummond 2011, JGR 116, E04001, abstract: X the "ratio of net evaporative loss from lakes to
///   the surface runoff from uplands"). Without it a lake's precipitation would count twice,
///   through the inflow and against its evaporation, and a terminal lake would take
///   `A_L` ÷ `A_B` = r ÷ (E − P), 1 ÷ X, not 1 ÷ (1 + X).
///
/// A world with no main liquid has no lake, and its balance is none ([`WaterBalance::of`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WaterBalance {
    /// Each cell's own inflow, its runoff times its area, m³ a⁻¹, finite and non-negative.
    pub(crate) inflow_m3_per_year: Vec<f64>,
    /// Each cell's net loss were the lake to flood it, (E − P) + its own runoff, m a⁻¹, finite.
    pub(crate) net_loss_m_per_year: Vec<f64>,
}

impl WaterBalance {
    /// The balance of `working`'s cells on `pass`'s world with each cell's own runoff
    /// `runoff_m_per_year` (m a⁻¹), or `None` on a world with no main liquid, which has no lake
    /// (decision-r09-t14-choices Q3; the type's documentation): [`from_rates`](Self::from_rates)
    /// of the cells' areas, Ω R², their runoffs and the climate's E − P at each cell's water
    /// surface.
    ///
    /// # Panics
    ///
    /// If `runoff_m_per_year` is not one a cell, or as [`NetLoss::new`],
    /// [`NetLoss::at_water_surfaces`] and [`from_rates`](Self::from_rates).
    #[must_use]
    pub(crate) fn of(
        pass: &Pass<'_>,
        working: &Working,
        runoff_m_per_year: &[f64],
    ) -> Option<Self> {
        pass.inputs().main_liquid()?;
        assert_eq!(
            runoff_m_per_year.len(),
            working.cells.len(),
            "one runoff a cell"
        );
        let r = pass.inputs().radius().value();
        let r2 = r * r;
        let areas_m2: Vec<f64> = pass
            .grid()
            .solid_angles_sr()
            .iter()
            .map(|&w| w * r2)
            .collect();
        let open_liquid_m_per_year = NetLoss::new(pass, working).at_water_surfaces(pass, working);
        Some(Self::from_rates(
            &areas_m2,
            runoff_m_per_year,
            &open_liquid_m_per_year,
        ))
    }

    /// The balance of cells of areas `areas_m2` (m²), own runoffs `runoff_m_per_year` (m a⁻¹,
    /// finite and non-negative) and open liquid's net loss `open_liquid_m_per_year` (E − P, m a⁻¹,
    /// finite), one a cell in cell-index order: each cell's inflow r a, and its net loss (E − P) +
    /// r, so that a flooded cell's own runoff leaves the balance exactly when the lake floods it
    /// (decision-r09-t14-choices Q3; Matsubara, Howard and Drummond 2011's X ratio; the type's
    /// documentation).
    ///
    /// # Panics
    ///
    /// If the inputs are not one a cell, or a value is not finite.
    #[must_use]
    pub(crate) fn from_rates(
        areas_m2: &[f64],
        runoff_m_per_year: &[f64],
        open_liquid_m_per_year: &[f64],
    ) -> Self {
        let n = areas_m2.len();
        for (what, len) in [
            ("runoff", runoff_m_per_year.len()),
            ("open liquid's net loss", open_liquid_m_per_year.len()),
        ] {
            assert_eq!(len, n, "one {what} a cell");
        }
        let inflow_m3_per_year = runoff_m_per_year
            .iter()
            .zip(areas_m2)
            .map(|(&q, &a)| num::assert_finite(q * a))
            .collect();
        let net_loss_m_per_year = open_liquid_m_per_year
            .iter()
            .zip(runoff_m_per_year)
            .map(|(&e_minus_p, &q)| num::assert_finite(e_minus_p + q))
            .collect();
        Self {
            inflow_m3_per_year,
            net_loss_m_per_year,
        }
    }

    /// The balance as Fill–Spill–Merge's measure.
    #[must_use]
    pub(crate) fn measure(&self) -> Measure<'_> {
        Measure::Balance {
            inflow_m3_per_year: &self.inflow_m3_per_year,
            net_loss_m_per_year: &self.net_loss_m_per_year,
        }
    }
}

/// The water on the land of a world wet now: the construction's lakes by `balance`, Q3's water
/// balance ([`WaterBalance`]), so that the closed basins the solve holds, Q1's, are the ruled
/// ones, each terminal lake's bed a bound and each terminal lake's or dry playa's level its
/// basin's base level, a full lake routed through ([`wet_now_full_bodies`]); with no balance (no
/// main liquid), none, every closed depression a dry playa whose floor is its basin's base level
/// (decision-r09-t14-choices Q1 and Q3).
#[must_use]
pub(crate) fn wet_now_water(balance: Option<&WaterBalance>) -> Water<'_> {
    balance.map_or(Water::None, |b| Water::Balance {
        inflow_m3_per_year: &b.inflow_m3_per_year,
        net_loss_m_per_year: &b.net_loss_m_per_year,
    })
}

/// A full lake on a world wet now, one whose inflow meets its loss at its sill so that it spills:
/// no bound, its cells routed through on the filled surface and solved like any other
/// (decision-r09-t14-choices, "T14.d lakes" item 1). The sea, the terminal lakes' beds and the
/// playas' pits are the only bounds and base levels.
///
/// In a fluvial steady state a lake with an outlet is transient: its overflow incises its sill
/// and its basin fills with sediment, until the depression is part of the graded network; a
/// steady state along a routing is monotone, so it holds no closed depression a river runs
/// through. The steady state then grades through the old depression, its floor raised to the
/// profile and its sill cut to it, with no process added. Only closed basins persist: the
/// terminal lakes, whose inflow does not meet their loss at the sill, and the dry playas. Q3's
/// placement on the final field then finds those. Lake basins that glacial erosion cuts are not
/// modelled: glacial erosion is labelled later (Q5).
#[must_use]
pub(crate) const fn wet_now_full_bodies() -> FullBodies {
    FullBodies::RoutedThrough
}

/// Each field cell's own runoff on a world wet now, m of liquid a Julian year, read for every
/// cell before any changes: the climate's annual runoff-generating rate
/// (`climate::annual_runoff_kg_m2_s`, kg m⁻² s⁻¹) times [`SECONDS_PER_JULIAN_YEAR`] ÷ `fluid`'s
/// density, the quotient taken once (decision-r09-t14-choices Q1 and Q5).
///
/// Ice cells shed it as it is (Q5). On a steady ice sheet the accumulation leaves as ice flux, so
/// the equivalent area accumulated through an ice cell is its balance ice flux, the flux's
/// catchment-size equivalent `A_i` = `q_i` ÷ `p₀` (Hergarten 2021, Earth Surf. Dynam. 9, 937, eq.
/// 17), and eroding the bed at it is Hergarten's glacial stream-power law in the fluvial law's
/// form, E = `K_g` `A_i`^m S^n (his eqs. 18–19), at `K_g` = K: "Kg = K would mean that a glacier
/// at a given ice flux and a given slope of the ice surface has the same erosion rate as a river
/// at the same discharge and the same channel slope". That equality is the rule's choice, not a
/// measured rate: Anderson, Molnar and Kessler 2006 (JGR 111, F01004, abstract) take a glacier's
/// erosion in proportion to its ice discharge per unit valley width and assume that "glacial
/// erosion rates far exceed fluvial erosion rates", and Koppes and Montgomery 2009 (Nat. Geosci.
/// 2, 644), whom the ruling cites for comparable long-term rates, were not read (labelled). Not
/// modelled, later: cold-based ice that protects its bed; overdeepening below the base level;
/// the ice sheet's own surface (the field's elevation is the ground's, and ice thickness is not
/// modelled); and meltwater's timing.
///
/// [`SECONDS_PER_JULIAN_YEAR`]: crate::units::consts::SECONDS_PER_JULIAN_YEAR
#[must_use]
pub(crate) fn wet_now_runoff_m_per_year(
    pass: &Pass<'_>,
    working: &Working,
    fluid: Fluid,
) -> Vec<f64> {
    let scale = SECONDS_PER_JULIAN_YEAR / fluid.density_kg_m3;
    (0..pass.grid().len())
        .map(|cell| annual_runoff_kg_m2_s(pass, working, cell) * scale)
        .collect()
}

/// The erosion of a world wet now, on `working` as the climate step left it, with each cell's own
/// runoff `runoff_m_per_year` (m a⁻¹, [`wet_now_runoff_m_per_year`]): the multigrid's steady state
/// ([`wet_now_time`]) under each basin's freeboard ([`wet_now_uplift`]) held to the threshold
/// slope ([`wet_now_threshold`]), with sticky receivers ([`Options::WET_NOW`]), K of each cell's
/// crust under the main liquid ([`Erodibility`]), the sea's cells ([`sea_cells`]) the bounds at the
/// sea's level, and the construction's lakes by their water balance ([`wet_now_water`] of
/// [`WaterBalance::of`]), the terminal ones' beds bounds and the full ones routed through
/// ([`wet_now_full_bodies`]). The solve, before the remap ([`remap_onto_construction`]).
///
/// # Panics
///
/// If `pass`'s world has no wet epoch, or as [`multigrid::erode`] and [`WaterBalance::of`].
#[must_use]
pub(crate) fn erode_wet_now(
    pass: &Pass<'_>,
    working: &Working,
    runoff_m_per_year: &[f64],
) -> Outcome {
    erode_wet_now_with(pass, working, runoff_m_per_year, Options::WET_NOW)
}

/// [`erode_wet_now`] with the multigrid run as `options` say.
///
/// # Panics
///
/// As [`erode_wet_now`].
#[must_use]
pub(crate) fn erode_wet_now_with(
    pass: &Pass<'_>,
    working: &Working,
    runoff_m_per_year: &[f64],
    options: Options,
) -> Outcome {
    let balance = WaterBalance::of(pass, working, runoff_m_per_year);
    erode(
        pass,
        working,
        runoff_m_per_year,
        wet_now_water(balance.as_ref()),
        (
            wet_now_uplift(),
            wet_now_time(),
            wet_now_threshold(),
            wet_now_full_bodies(),
        ),
        options,
    )
}

/// A world wet now's field remapped onto its construction's absolute hypsometry
/// ([`remap_onto_construction`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Remapped {
    /// Each cell's elevation, metres: a remapped cell's construction elevation at its rank, lifted
    /// to its receiver's surface or held to the threshold above it; any other cell's the solve's,
    /// its ground for a bound, a cell of zero equivalent area or a held cell.
    pub(crate) elevation_m: Vec<f64>,
    /// The cells the remap maps: the unbound cells the solve moved, not bound, of equivalent area
    /// above zero and not held ([`held`](Self::held)).
    pub(crate) moved: Vec<bool>,
    /// The held cells: unbound, of equivalent area above zero, with no uplift, at their
    /// construction to the bit below their receiver on the solve, closed depressions that the
    /// lakes on the final field take (decision-r09-t14-choices, "T14.c follow-up 3" item 1).
    pub(crate) held: Vec<bool>,
    /// The remapped cells the pass lifted to their receiver's surface.
    pub(crate) lifted: usize,
    /// The largest lift, metres; zero where none is.
    pub(crate) largest_lift_m: f64,
    /// The remapped cells the pass lowered to the threshold above their receiver.
    pub(crate) lowered: usize,
    /// The largest lowering, metres; zero where none is.
    pub(crate) largest_lowering_m: f64,
}

/// A world wet now's solve `s_m` on `outcome`'s last routing remapped onto its construction
/// `z0_m`'s area-weighted absolute heights (decision-r09-t14-choices, "T14.c follow-up 2" §2 as
/// its "T14.c follow-up 3" item 3 overrules its frame).
///
/// 1. **The set.** The unbound cells the solve moved: every cell not bound (the sea's cells and
///    lake beds keep their ground), of equivalent area above zero, and not held, a cell with no
///    uplift kept at its construction to the bit below its receiver on the solve (a closed
///    depression that the lakes on the final field take; the follow-up's item 1). The same set on
///    both sides.
/// 2. **The map.** The set sorted by (s, the routing's preorder position), each asserted finite,
///    with its cumulative area; each cell takes the construction's elevation z0 at its own
///    cumulative midpoint, the construction's set sorted by (z0, cell index) with its cumulative
///    area, the value of the construction cell whose interval holds the midpoint, with no
///    interpolation. The preorder tie-break gives a receiver its value before its donors at a tie
///    (a flat or a capped link; the follow-up's item 2, ratified).
/// 3. **The pass.** In the routing's preorder, each remapped cell with a receiver becomes
///    min(max(z′, `w_r`), `w_r` + `s_c` δ), `w_r` its receiver's final surface (a bound's water
///    surface or ground, otherwise the receiver's own final height), `s_c` [`DEBRIS_FLOW_SLOPE`]
///    and δ the link: the max lifts a cell the map put below its receiver's surface up to it, a
///    flat shore below a high lake; the min holds the threshold, undoing the map's local
///    stretching. Held cells are skipped.
///
/// Any one monotone map of absolute height keeps the order along every path, whatever its base,
/// and the pass leaves every remapped cell at or above its receiver with no link steeper than the
/// threshold, so the drainage is the solve's. The land's highest point above the sea is at most
/// the construction's: the map's values are the construction's, and the pass only lowers a cell
/// or lifts it to a surface no higher. Labelled: the coarse land's hypsometry on a world wet now
/// is its construction's above the sea, to the lifts, the lowerings and the cells' sizes, in the
/// frame Earth2014, T12.a's bands and plan 14's hypsometry are calibrated in. T12's construction
/// is calibrated on an eroded planet's hypsometry, so the steady state's own reshaping of it, a
/// concave profile that keeps each basin's mean and so lengthens its upper tail, would count
/// erosion twice; the steady state organises the drainage, not the distribution. Basin means move
/// by the global map, and `k_s` is the final field's S A^θ, no longer u ÷ K exactly. The solver's
/// uplift still reads heights above each basin's base level (Q1).
///
/// # Panics
///
/// If an input is not one a cell of `outcome`, or a height is not finite.
#[must_use]
pub(crate) fn remap_onto_construction(outcome: &Outcome, z0_m: &[f64], s_m: &[f64]) -> Remapped {
    let n = s_m.len();
    for (what, len) in [
        ("construction", z0_m.len()),
        ("bound", outcome.bounds.len()),
        ("uplift", outcome.uplift_m_per_year.len()),
    ] {
        assert_eq!(len, n, "one {what} a cell");
    }
    let routing = &outcome.routing;
    let area = outcome.mesh.areas_m2();
    let unbound: Vec<bool> = (0..n)
        .map(|c| outcome.bounds[c].is_none() && outcome.equivalent_area_m2[c] > 0.0)
        .collect();
    let held: Vec<bool> = (0..n)
        .map(|c| {
            // Signed zeros made positive, as the drainage's are, so that a −0 against a +0 is
            // held, and below is below by `<`.
            unbound[c]
                && outcome.uplift_m_per_year[c] <= 0.0
                && canonical(s_m[c]).total_cmp(&canonical(z0_m[c])).is_eq()
                && routing.receiver(cell_of(c)).is_some_and(|link| {
                    canonical(s_m[c])
                        .total_cmp(&canonical(s_m[slot(link.cell)]))
                        .is_lt()
                })
        })
        .collect();
    let moved: Vec<bool> = (0..n).map(|c| unbound[c] && !held[c]).collect();
    let mut order = vec![0; n];
    for (rank, &cell) in routing.preorder().iter().enumerate() {
        order[slot(cell)] = rank;
    }
    let ranked = |height: &[f64], tie: &dyn Fn(usize) -> usize| {
        let mut cells: Vec<(f64, usize)> = (0..n)
            .filter(|&c| moved[c])
            .map(|c| (canonical(height[c]), c))
            .collect();
        cells.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(tie(a.1).cmp(&tie(b.1))));
        cells
    };
    let solved = ranked(s_m, &|c| order[c]);
    let built = ranked(z0_m, &|c| c);
    let mut elevation_m = s_m.to_vec();
    let (mut below, mut j) = (0.0, 0);
    let mut upto = built.first().map_or(0.0, |&(_, c)| area[c]);
    for &(_, c) in &solved {
        let midpoint = below + 0.5 * area[c];
        below += area[c];
        while j + 1 < built.len() && upto <= midpoint {
            j += 1;
            upto += area[built[j].1];
        }
        elevation_m[c] = built[j].0;
    }
    let (mut lifted, mut largest_lift_m) = (0, 0.0);
    let (mut lowered, mut largest_lowering_m) = (0, 0.0);
    for &cell in routing.preorder() {
        let c = slot(cell);
        let Some(link) = routing.receiver(cell).filter(|_| moved[c]) else {
            continue;
        };
        let r = slot(link.cell);
        let surface = outcome.bounds[r].unwrap_or(elevation_m[r]);
        let top = surface + DEBRIS_FLOW_SLOPE * link.length_m;
        let height = elevation_m[c];
        if height < surface {
            largest_lift_m = num::max(largest_lift_m, surface - height);
            elevation_m[c] = surface;
            lifted += 1;
        } else if height > top {
            largest_lowering_m = num::max(largest_lowering_m, height - top);
            elevation_m[c] = top;
            lowered += 1;
        }
    }
    Remapped {
        elevation_m,
        moved,
        held,
        lifted,
        largest_lift_m,
        lowered,
        largest_lowering_m,
    }
}

/// The multigrid on `working`'s elevation as its construction, with `runoff`, `water`, `uplift`,
/// `time` and `threshold`, run as `options` say: K of each cell's crust under the world's fluid,
/// the sea's cells ([`sea_cells`]) at the working sea level, the climate grid the field's parents.
fn erode(
    pass: &Pass<'_>,
    working: &Working,
    runoff: &[f64],
    water: Water<'_>,
    (uplift, time, threshold, full_bodies): (Uplift, Time, Threshold, FullBodies),
    options: Options,
) -> Outcome {
    let inputs = pass.inputs();
    let z0_m: Vec<f64> = working
        .cells
        .iter()
        .map(|cell| cell.elevation.value())
        .collect();
    let erodibility = Erodibility::of(inputs)
        .expect("a world with a wet epoch has a fluid")
        .per_cell(&working.cells);
    let sea = sea_cells(working);
    multigrid::erode(
        &Problem {
            seed: pass.seed(),
            radius: inputs.radius(),
            grid: pass.grid(),
            parent_grid: Some(pass.climate_grid()),
            z0_m: &z0_m,
            runoff_m_per_year: runoff,
            erodibility_m0_1_per_year: &erodibility,
            sea: &sea,
            sea_level_m: working.sea_level.value(),
            water,
            uplift,
            time,
            threshold,
            full_bodies,
        },
        options,
    )
}

/// Each cell of `pass`'s grid whose centre lies inside the rim of a coarse crater of `working`
/// that waits for the erosion ([`waits`]): the young surface (decision-r09-t14-choices Q4), its
/// chord from the crater centre's spheroid point to the cell centre's below the rim radius, as
/// the crater's own profile measures it (`craters::each_lift`). The continuous ejecta beyond the
/// rim is not counted (labelled).
#[must_use]
pub(crate) fn inside_waiting_rims(pass: &Pass<'_>, working: &Working) -> Vec<bool> {
    let figure = pass.inputs().figure();
    let grid = pass.grid();
    let mut inside = vec![false; working.cells.len()];
    for crater in working.craters.iter().filter(|c| waits(pass, c.age)) {
        let centre = figure.point(crater.centre);
        let rim = 0.5 * crater.diameter.value();
        for cell in crater.reach.cells() {
            let p = figure.point(grid.centre(cell));
            let gap = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
            let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
            if chord < rim {
                inside[slot(cell)] = true;
            }
        }
    }
    inside
}

/// Each cell's own runoff in the output, m a⁻¹: `source`, the runoff the solver took, but zero on
/// the young surface `young` (each cell inside a waiting crater's rim, [`inside_waiting_rims`]),
/// which formed after the epoch's flow and so carries no tributaries of its own
/// (decision-r09-t14-choices Q4; science-r09-drainage §1.3 item 5).
#[must_use]
pub(crate) fn output_runoff_m_per_year(source: &[f64], young: &[bool]) -> Vec<f64> {
    source
        .iter()
        .zip(young)
        .map(|(&r, &late)| if late { 0.0 } else { r })
        .collect()
}
