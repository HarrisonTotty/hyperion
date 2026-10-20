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
//!   epoch's end erode with the rest, and shed no runoff of their own afterwards (Q4). The
//!   volcanic provinces take their own surface's exposure and bed, renewed after the solve where
//!   they are younger than the epoch, and every eroded cell lowers by its valley's void, not its
//!   channel's whole incision (decision-r09-t14-mars-ceiling §2.1–2.2).
//! - **Wet now** (an epoch that runs to now): the steady state (eq. 9) under each basin's uplift in
//!   proportion to its freeboard, a field fixed in space (Q1, as its "T14.c follow-up" item 1 rules
//!   it), each construction basin's mean elevation held by an additive refit and the receivers
//!   sticky ("T14.c follow-up 2" §1 and §3), no link rising more than the debris-flow slope times
//!   its length (the "T14.c follow-up" item 2), the climate's annual runoff the source, ice cells
//!   shedding it as it is (Q5), the sea's cells the bounds, and every closed depression a dry playa
//!   until R09.T14.d's water balance (Q3); its heights above base then remapped in order onto its
//!   construction's ([`remap_onto_construction`], "T14.c follow-up 2" §2).
//!
//! Each choice is one function whose documentation cites the ruling's question.

use hyperion_surface::num;

use hyperion_surface::field::Crust;
use hyperion_surface::synth::channels::{
    DEBRIS_FLOW_SLOPE, bankfull_width, channel_forming_discharge_m3_s, mean_discharge_m3_s,
};

use super::super::climate::seas::under_sea;
use super::super::craters::waits;
use super::super::plates::{cell_of, slot};
use super::super::{CellState, Pass, Working};
use super::erodibility::{Erodibility, Fluid};
use super::multigrid::{self, Options, Outcome, Problem, Uplift, Water};
use super::solver::{self, Cells, Threshold, Time};
use crate::planetary::surface::inputs::{CoarseInputs, WetEpoch};
use crate::units::{Gigayears, MetresPerSecondSquared, SquareMetres};

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

/// The coarse cell's lowering on a world dry now, metres: the void of one trunk valley cut into
/// an upland that stays at its construction, over the cell, `L` = min(d, d (`w_bf` + 5d) ÷ w), for
/// the channel's incision `incision_m` d at full K, the valley's floor `floor_m` `w_bf` (its
/// bankfull channel, [`valley_floor_m`]) and the cell's width `cell_m` w, √(Ω R²)
/// (decision-r09-t14-mars-ceiling §2.2).
///
/// The valley's walls rise at a mean gradient of 0.2 from its floor, so its top width is
/// `w_bf` + 10d and its void d (`w_bf` + 5d) a unit length: the width "about ten times the
/// depth" (Luo, Cang and Howard 2017, Nat. Commun. 8, 15766, Methods, after Williams and Phillips
/// 2001, JGR 106, 23737), medium-low confidence. Capped at d, where the valley is as wide as its
/// cell, the space-filling limit. The 10:1 walls are Mars's rock; an ice or methane epoch is an
/// extrapolation (labelled). K, t and the runoff never move to pass the ceiling; the channel's
/// incision is kept unscaled apart ([`CellState::channel_incision`]). R09.T6.c's valleys take the
/// same cross-section, and that task's commit makes this count exactly the valleys it draws.
///
/// [`CellState::channel_incision`]: super::super::CellState::channel_incision
#[must_use]
pub(crate) fn dry_now_lowering_m(incision_m: f64, floor_m: f64, cell_m: f64) -> f64 {
    if incision_m <= 0.0 {
        return 0.0;
    }
    num::min(
        incision_m,
        incision_m * (floor_m + 5.0 * incision_m) / cell_m,
    )
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
}

/// A world dry now's erosion on `working` as the climate step left it, or `None` where the
/// surface is no older than its wet epoch's end (decision-r09-t14-choices Q2 and Q4;
/// decision-r09-t14-mars-ceiling §2.1–2.2): the solve ([`erode_dry_now`]) at the surface's own
/// exposure on every cell; then each cell of an exposure below it renewed, its channel the
/// transient at its own exposure's time on the solve's last routing (its construction to the bit
/// where it saw none: the reference Mars's provinces, its surface renewed after the epoch, as the
/// late craters are placed on eroded ground); then each eroded cell lowered by its valley's void
/// ([`dry_now_lowering_m`]).
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
    let omega = pass.grid().solid_angles_sr();
    let r = inputs.radius().value();
    let n = z0.len();
    let (mut incision_m, mut floor_m) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut lowering_m, mut elevation_m) = (Vec::with_capacity(n), Vec::with_capacity(n));
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
        let lowering = dry_now_lowering_m(d, floor, r * omega[c].sqrt());
        incision_m.push(d);
        floor_m.push(floor);
        lowering_m.push(lowering);
        elevation_m.push(z0[c] - lowering);
    }
    Some(DryNow {
        outcome,
        share,
        exposure,
        incision_m,
        floor_m,
        lowering_m,
        elevation_m,
    })
}

/// Each cell's solved channel, metres: the outcome's where its exposure is the surface's `share`;
/// its construction to the bit where it saw none; and otherwise the transient at its own
/// exposure's time, `effective_flow` × its exposure, on the outcome's last routing with the
/// solve's own equivalent areas, erodibilities and bounds, one solve for each such exposure in
/// ascending order (decision-r09-t14-mars-ceiling §2.1).
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
/// bounds.
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
        (Uplift::None, Time::Years(years), dry_now_threshold()),
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
/// hold the bed: fluvial incision "rarely extends to slopes greater than ∼0.03 to 0.10" (Stock
/// and Dietrich 2003, WRR 39(4), 1089, doi:10.1029/2001WR001057, abstract). Their hillslope term
/// (§6.1) is negligible at the coarse cell, and their threshold of 0.57 (30°) is a hillslope's,
/// above the channel's. Labelled: one threshold on every body; debris flows' threshold under
/// another gravity or liquid is not modelled.
#[must_use]
pub(crate) const fn wet_now_threshold() -> Threshold {
    Threshold::Slope(DEBRIS_FLOW_SLOPE)
}

/// The water on the land of a world wet now: none, every closed depression a dry playa whose
/// floor is its basin's base level, until R09.T14.d brings the lakes' water balance, the
/// catchment's runoff against the lake's evaporation less its precipitation
/// (decision-r09-t14-choices Q3).
#[must_use]
pub(crate) const fn wet_now_water() -> Water<'static> {
    Water::None
}

/// Each field cell's own runoff on a world wet now, m of liquid a Julian year, read for every
/// cell before any changes: the climate's annual runoff-generating rate
/// (`climate::annual_runoff_kg_m2_s`, kg m⁻² s⁻¹) times [`SECONDS_PER_JULIAN_YEAR`] ÷ `fluid`'s
/// density, the quotient taken once (decision-r09-t14-choices Q1 and Q5).
///
/// Ice cells shed it as it is (Q5). On a steady ice sheet the accumulation leaves as ice flux, so
/// the equivalent area accumulated through an ice cell is its balance ice flux, and eroding the
/// bed at it makes the bed's erosion scale with the ice's discharge, as simple glacial-valley
/// models do (Anderson, Molnar and Kessler 2006, JGR 111, F01004; from memory): long-term glacial
/// and fluvial erosion rates are comparable (Koppes and Montgomery 2009, Nat. Geosci. 2, 644;
/// bibliographic record checked, the paper not read). Not modelled, later: cold-based ice that
/// protects its bed; overdeepening below the base level; the ice sheet's own surface (the
/// field's elevation is the ground's, and ice thickness is not modelled); and meltwater's timing.
///
/// Read in the crate's own tests only until R09.T14.d, whose commit lifts the climate function's
/// `expect(dead_code)` with the climate's re-application (the lane's design §0).
///
/// [`SECONDS_PER_JULIAN_YEAR`]: crate::units::consts::SECONDS_PER_JULIAN_YEAR
#[cfg(test)]
#[must_use]
pub(crate) fn wet_now_runoff_m_per_year(
    pass: &Pass<'_>,
    working: &Working,
    fluid: Fluid,
) -> Vec<f64> {
    use super::super::climate::annual_runoff_kg_m2_s;
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
    let scale = SECONDS_PER_JULIAN_YEAR / fluid.density_kg_m3;
    (0..pass.grid().len())
        .map(|cell| annual_runoff_kg_m2_s(pass, working, cell) * scale)
        .collect()
}

/// The erosion of a world wet now, on `working` as the climate step left it, with each cell's own
/// runoff `runoff_m_per_year` (m a⁻¹, `wet_now_runoff_m_per_year`): the multigrid's steady state
/// ([`wet_now_time`]) under each basin's freeboard ([`wet_now_uplift`]) held to the threshold
/// slope ([`wet_now_threshold`]), with sticky receivers ([`Options::WET_NOW`]), K of each cell's
/// crust under the main liquid ([`Erodibility`]), the sea's cells ([`sea_cells`]) the bounds at the
/// sea's level, and no water on the land ([`wet_now_water`]). The solve, before the remap
/// ([`remap_onto_construction`]).
///
/// # Panics
///
/// If `pass`'s world has no wet epoch, or as [`multigrid::erode`].
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.d calls it outside the tests, with the climate's runoff"
    )
)]
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
    erode(
        pass,
        working,
        runoff_m_per_year,
        wet_now_water(),
        (wet_now_uplift(), wet_now_time(), wet_now_threshold()),
        options,
    )
}

/// A world wet now's field remapped onto its construction's hypsometry
/// ([`remap_onto_construction`]).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.d calls the wet-now branch outside the tests, with the climate's runoff"
    )
)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Remapped {
    /// Each cell's elevation, metres: a remapped cell's base plus its construction's height above
    /// base at its rank, re-capped; any other cell's the solve's, its ground.
    pub(crate) elevation_m: Vec<f64>,
    /// The cells the solver moved, which the remap maps: not bound and of equivalent area above
    /// zero.
    pub(crate) moved: Vec<bool>,
    /// The links the threshold re-capped after the remap.
    pub(crate) recapped: usize,
    /// The largest lowering of a re-capped cell, metres; zero where none is.
    pub(crate) largest_recap_m: f64,
}

/// A world wet now's solve `s_m` on `outcome`'s last routing remapped onto its construction
/// `z0_m`'s area-weighted heights above base (decision-r09-t14-choices, "T14.c follow-up 2" §2).
///
/// 1. **Heights above base.** Every cell not bound and of equivalent area above zero (the sea's
///    cells, lake beds and cells of zero equivalent area keep their ground) takes h = s − the
///    base of its own root on the routing.
/// 2. **The order.** Those cells sorted by h, each asserted finite, ties broken by the routing's
///    preorder (each receiver before its donors), with their cumulative area, each takes the
///    construction's height above base at its own cumulative midpoint: the construction's heights
///    z0 − `ℓ_b` over the same cells, sorted by (value, cell index) with their cumulative area, the
///    value of the construction cell whose interval holds the midpoint, with no interpolation,
///    clamped at zero or above. Its elevation is its base plus that value. The ruling's tie-break
///    is the cell index; the preorder is the lane's, since a link of no rise (a cell without
///    uplift, level with its receiver) would otherwise put a donor of lower index below its
///    receiver (12 cells on the reference Earth).
/// 3. **The threshold again.** In the routing's preorder, any such cell more than
///    [`DEBRIS_FLOW_SLOPE`] times its link above its receiver (the water's surface over a bound) is
///    lowered to its receiver plus that, undoing the remap's local stretching.
///
/// The map is monotone in the height above base and a path shares one base, so the drainage is
/// the solve's. Labelled: the coarse land's hypsometry on a world wet now is its construction's,
/// to the re-cap and the cells' sizes. T12's construction is calibrated on an eroded planet's
/// hypsometry (Earth2014's), so the steady state's own reshaping of it, a concave profile that
/// keeps each basin's mean and so lengthens its upper tail, would count erosion twice; the
/// steady state organises the drainage, not the distribution. Basin means move by the global map,
/// and `k_s` is the final field's S A^θ, no longer u ÷ K exactly.
///
/// Recorded: the heights are taken above the final routing's bases and given back from the
/// construction's own, so a cell whose root has moved (a captured basin) moves with its base. On
/// the reference Earth the heights above base are the construction's to the re-cap (their highest
/// 8,299 m), but above the sea the land's median falls from 550 to 490 m and its highest rises
/// from 8,594 to 10,696 m, a cell draining to a closed basin of high base.
///
/// # Panics
///
/// If an input is not one a cell of `outcome`, the outcome carries no construction base levels
/// (it is not a world wet now's), or a height is not finite.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.d calls the wet-now branch outside the tests, with the climate's runoff"
    )
)]
#[must_use]
pub(crate) fn remap_onto_construction(outcome: &Outcome, z0_m: &[f64], s_m: &[f64]) -> Remapped {
    let n = s_m.len();
    for (what, len) in [
        ("construction", z0_m.len()),
        ("bound", outcome.bounds.len()),
        ("construction base", outcome.construction_base_m.len()),
    ] {
        assert_eq!(len, n, "one {what} a cell");
    }
    let routing = &outcome.routing;
    let area = outcome.mesh.areas_m2();
    let moved: Vec<bool> = (0..n)
        .map(|c| outcome.bounds[c].is_none() && outcome.equivalent_area_m2[c] > 0.0)
        .collect();
    let base = |c: usize| routing.base_level_m(cell_of(c));
    let mut order = vec![0; n];
    for (rank, &cell) in routing.preorder().iter().enumerate() {
        order[slot(cell)] = rank;
    }
    let ranked = |height: &dyn Fn(usize) -> f64, tie: &dyn Fn(usize) -> usize| {
        let mut cells: Vec<(f64, usize)> = (0..n)
            .filter(|&c| moved[c])
            .map(|c| (num::assert_finite(height(c)), c))
            .collect();
        cells.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(tie(a.1).cmp(&tie(b.1))));
        cells
    };
    let solved = ranked(&|c| s_m[c] - base(c), &|c| order[c]);
    let built = ranked(&|c| z0_m[c] - outcome.construction_base_m[c], &|c| c);
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
        elevation_m[c] = base(c) + num::max(0.0, built[j].0);
    }
    let (mut recapped, mut largest_recap_m) = (0, 0.0);
    for &cell in routing.preorder() {
        let c = slot(cell);
        let Some(link) = routing.receiver(cell).filter(|_| moved[c]) else {
            continue;
        };
        let r = slot(link.cell);
        let top = outcome.bounds[r].unwrap_or(elevation_m[r]) + DEBRIS_FLOW_SLOPE * link.length_m;
        if elevation_m[c] > top {
            largest_recap_m = num::max(largest_recap_m, elevation_m[c] - top);
            elevation_m[c] = top;
            recapped += 1;
        }
    }
    Remapped {
        elevation_m,
        moved,
        recapped,
        largest_recap_m,
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
    (uplift, time, threshold): (Uplift, Time, Threshold),
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
