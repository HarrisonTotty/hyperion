//! The coarse pass's steps and the working state they share (plan R09, Design note 5).
//!
//! The pass is one job in a fixed order, the brainstorm's ("The coarse global pass, once per
//! planet"): [`plates`] (R09.T11), [`relief`] (R09.T12.a–c and e), [`craters`] (R09.T12.d),
//! [`climate`] (R09.T13), [`erosion`] (R09.T14) and [`classes`] (R09.T15, with the crater
//! state). Each step is a function of the [`Pass`], which holds the seed, the inputs and the
//! grids, and of the [`Working`] state the steps before it left, which it updates in place. Every
//! loop runs in cell-index order and every sum in that order, every sort is keyed by (value, cell
//! index), the arithmetic is `f64`, and no `HashMap` is used (Design note 5). The working state
//! is `f64` throughout; only [`quantise`](super::quantise()) turns it into the field, whose codes
//! are the only input of the synthesis (Design note 17).
//!
//! One exception to the order, Design note 5's: on a world whose wet epoch ended before its
//! surface age ran out (a Mars), each coarse crater is drawn a formation age, and only those older
//! than the epoch's end are smoothed in before erosion; the younger ones are applied after it.
//! R09.T12.d splits the crater step for it: [`craters::run`] draws every coarse crater and smooths in
//! the older ones, and [`craters::run_after_erosion`], which the pass calls after [`erosion`], the
//! younger.
//!
//! Two hazards for the steps that fill this frame (sim-determinism, "Hazards across targets"): a
//! value is asserted finite (`hyperion_surface::num::assert_finite`) before it is sorted or
//! compared, since a NaN's sign, and so where `total_cmp` puts it, differs between targets; and an
//! extreme that reaches the header (the sea level, the realised relief) is found with
//! `hyperion_surface::num::{min, max}`, whose signed zeros are fixed, not `f64::min` and `max`.
//!
//! The steps, [`Pass`] and [`Working`] are public so that the benches of single steps (R09.T14's
//! `coarse/erosion_l8`) and of the pass (R09.T16's `coarse/pass`) can reach them, as Provides'
//! "each pub for tests" says; the working state's shape is the pass's own, unfrozen until R09.T16's
//! goldens.
//!
//! As built by R09.T10, every step was a no-op, so the pass yielded its initial state: a smooth
//! sphere at the datum ([`Working::initial`]). R09.T11 filled [`plates`]: on a mobile lid it writes
//! each cell's plate, crust and nearest boundary, and each oceanic cell's
//! [`SeafloorOrigin`]. R09.T12.a began [`relief`]'s coarse elevation, the crust's populations and
//! the sea floor, R09.T12.b its convergent landforms and R09.T12.c a stagnant lid's provinces;
//! R09.T12.d filled [`craters`]; and R09.T12.e added the relief step's second entry,
//! [`relief::run_after_craters`], which the pass calls after the craters: it scales the elevation
//! to plan 14's `σ_h`, places the sea and writes the realised figures. R09.T13.a filled
//! [`climate`] for a world whose regime names the seasonal energy-balance model: its zonal model
//! writes each climate cell's sea-level temperature and monthly anomalies, the lapse rate left 0.
//! R09.T13.b put that model on 72 × 72 points in latitude and longitude, and a locked world's on
//! 72 × 144 in tidally locked coordinates. R09.T13.c filled it for the other regimes: radiative
//! equilibrium with thermal inertia, the day resolved, under a thin air's slab and frost point,
//! and the isothermal surface. R09.T13.d normalised its field to plan 14's mean and signed
//! contrasts and writes the lapse rate, each flooded cell's liquid and each cell's ice by
//! substance; the water surfaces are the relief step's flood (decision-r09-t12-coarse R8).
//! R09.T13.g gave the energy-balance models the relief: each grid point's hypsometry of the cells'
//! heights above the sea level, its albedo, outgoing radiation and sea-ice heat capacity at the
//! ground's temperature, and τ solved for the surface mean. R09.T13.e added the labelled
//! precipitation heuristic and the winds on the energy-balance models: each climate cell's monthly
//! precipitation and 10 m wind, the land's rain carried along each month's recorded wind by
//! Hergarten and Robl's orographic model, solved in the order of each month's upwind components,
//! each large one's members along its flow, until one further sweep moves no land cell's rate by
//! more than 10⁻⁴ of it, the global water
//! balanced; zero and calm on the other models. R09.T13.f added the cold traps: each ice's
//! cold-trapped part, the inputs' `cold_traps`, placed after its zonal part in each field cell's
//! cold-trap fraction from plan 14's closed form, scaled to plan 14's area, one ice a cell, the
//! rest counted as undrawn. R09.T13.h balanced the radiative model's thin-air slab with its
//! column's own emission, bounded at the frost point aloft with its snowfall. R09.T14.a–c filled
//! [`erosion`]: the drainage, the stream-power solver and its multigrid, and the branches of a
//! world wet now, dry now and never wet, writing each cell's elevation, flow, drainage, steepness,
//! runoffs and, dry now, its channel incision. R09.T14.d added the rest of the step: the climate's
//! runoff read live, the construction's lakes by their water balance, `σ_h` matched again, the sea
//! placed again, the lakes on the final field, the climate's lapse term and mean re-applied with
//! the ices and liquids, and the final routing; and, after the late craters, the field routed
//! again as it stands. R09.T15 filled [`classes`]: each cell's Köppen–Geiger class or surface-state
//! form, and its crater state.

pub mod classes;
pub mod climate;
pub mod craters;
pub mod erosion;
pub mod plates;
pub mod relief;

use hyperion_surface::field::{
    BoundaryKind, CoarseCrater, CoarseLevel, Crust, FlowDirection, SurfaceClass,
};
use hyperion_surface::synth::{BandSpectrum, SpectrumShape};

use super::grid::CellGraph;
use super::inputs::CoarseInputs;
use crate::planetary::hooks::SurfaceSeed;
use crate::units::{Kelvin, Metres, MetresPerSecond, Radians, Seconds, SquareMetres};

/// What every step reads and never changes: the body's surface seed, its inputs, its level and the
/// grids of the field and of its climate layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pass<'a> {
    seed: SurfaceSeed,
    inputs: &'a CoarseInputs,
    level: CoarseLevel,
    grid: &'a CellGraph,
    climate_grid: &'a CellGraph,
}

impl<'a> Pass<'a> {
    /// The pass of `inputs` under `seed`, on `grid`, the field's level, and `climate_grid`, the
    /// climate layer's.
    ///
    /// # Panics
    ///
    /// If `grid` is not at a coarse level or `climate_grid` not one level above it.
    #[must_use]
    pub fn new(
        seed: SurfaceSeed,
        inputs: &'a CoarseInputs,
        grid: &'a CellGraph,
        climate_grid: &'a CellGraph,
    ) -> Self {
        let level = CoarseLevel::new(grid.level()).expect("the field's grid is at a coarse level");
        assert_eq!(
            climate_grid.level(),
            level.climate_level(),
            "the climate grid is one level above the field's"
        );
        Self {
            seed,
            inputs,
            level,
            grid,
            climate_grid,
        }
    }

    /// The surface seed, which opens the `surface.coarse.*` tags and nothing else (Design note 2).
    #[must_use]
    pub const fn seed(&self) -> SurfaceSeed {
        self.seed
    }

    /// The body's inputs.
    #[must_use]
    pub const fn inputs(&self) -> &'a CoarseInputs {
        self.inputs
    }

    /// The field's level.
    #[must_use]
    pub const fn level(&self) -> CoarseLevel {
        self.level
    }

    /// The field's cells.
    #[must_use]
    pub const fn grid(&self) -> &'a CellGraph {
        self.grid
    }

    /// The climate layer's cells, one level above the field's.
    #[must_use]
    pub const fn climate_grid(&self) -> &'a CellGraph {
        self.climate_grid
    }
}

/// One cell's state in the pass, in SI units but its runoffs, which are in metres a Julian year as
/// their codes and Hergarten's normalisation are: what its synthesis record will hold before it
/// is quantised (Design note 17).
///
/// Plain data with public fields, which the steps write in turn; [`quantise`](super::quantise())
/// checks every value against its code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellState {
    /// The ground's elevation above the datum, along the spheroid's normal.
    pub elevation: Metres,
    /// The surface of the liquid over the cell, at or above the ground: the ground itself where
    /// the cell is dry.
    pub water_surface: Metres,
    /// The cell's plate, 0 on a world without plates.
    pub plate: u8,
    /// The crust under the cell.
    pub crust: Crust,
    /// The kind of the nearest plate boundary, [`BoundaryKind::Absent`] without plates.
    pub boundary: BoundaryKind,
    /// The signed great-circle distance to the nearest boundary, negative on the plate that
    /// subducts, or `None` without a boundary.
    pub boundary_distance: Option<Metres>,
    /// The obliquity of the relative motion at the nearest boundary, 0 to π ÷ 2.
    pub boundary_obliquity: Radians,
    /// Where the cell's water goes.
    pub flow: FlowDirection,
    /// The geometric (planimetric) area draining through the cell, its own included once it is
    /// routed (science-r09-drainage.md §1.3).
    pub drainage: SquareMetres,
    /// The channel steepness index `k_s` = S A^θ with θ = 0.45 and A the geometric drainage area,
    /// m^0.9, non-negative.
    pub steepness_index_m0_9: f64,
    /// The cell's own erosive runoff, metres of liquid a Julian year, non-negative: the source
    /// term the erosion took, its climate's annual runoff on a world wet now, its wet epoch's
    /// where the surface is older than the epoch's end on one dry now, and none elsewhere
    /// ([`SynthesisCell::runoff`](hyperion_surface::field::SynthesisCell::runoff)).
    pub runoff_m_per_year: f64,
    /// The area-weighted mean of [`runoff_m_per_year`](Self::runoff_m_per_year) over the cell's
    /// drainage area, its own included, metres a Julian year, non-negative: the solver's
    /// equivalent area over the geometric, times 1 m a⁻¹.
    pub catchment_runoff_m_per_year: f64,
    /// The erosion's channel incision at the cell on a world dry now, z0 less the solver's
    /// elevation, before any width closure: what the wet epoch cut (R09.T14.c;
    /// decision-r09-t14-choices F2, for R09.T6.c's valleys of a short wet epoch), metres,
    /// non-negative. Zero inside a waiting crater's rim, whose ground formed after the epoch, on
    /// every other world and before the erosion. Quantised into
    /// [`SynthesisCell::epoch_incision`](hyperion_surface::field::SynthesisCell::epoch_incision)
    /// on a world dry now, `LogIncision::NONE` on every other.
    pub(crate) channel_incision: Metres,
    /// The share of the cell under ice, 0 to 1.
    pub ice_fraction: f64,
    /// The palette entry of the cell's ice, an index into the inputs' palette
    /// ([`CoarseInputs::palette`]) of an `Ice` entry, or `None`: named wherever `ice_fraction` is
    /// above zero, and free to name an ice the cell holds none of (a seasonal frost's).
    pub ice_entry: Option<u8>,
    /// The palette entry of the liquid over the cell, a `Liquid` entry, or `None`: named wherever
    /// the water surface is above the ground.
    pub liquid_entry: Option<u8>,
    /// The cell's climate or surface-state class (R09.T15).
    pub class: SurfaceClass,
    /// The cell's crater state (R09.T15).
    pub crater_state: u8,
    /// The draped abyssal-hill relief H of the cell's sea floor, positive, or `None` off oceanic
    /// crust: R09.T12.a's, from its own ridge's half-rate and its age (`decision-r09-t5.md`
    /// item 5). R09.T11, which first writes oceanic crust, gives each oceanic cell the reference
    /// style's [`H_REF`](hyperion_surface::synth::relief::H_REF), so that the field holds hills
    /// exactly on oceanic crust, until R09.T12.a replaces it.
    pub hill_relief: Option<Metres>,
    /// The thickness of the ponded (turbidite) sediment over the cell, non-negative: R09.T12.a's,
    /// from its plate's passive margins (`decision-r09-t5.md` item 5); zero off oceanic crust and
    /// until it runs.
    pub ponded_sediment: Metres,
    /// Where the cell's sea floor formed, on oceanic crust: R09.T11's distance to its own plate's
    /// nearest ridge, the half-rate there and the age, which GDH1 and the sea floor's roughness
    /// read (R09.T12.a; `decision-r09-t5.md` item 5); `None` off oceanic crust. None of it
    /// travels: the field carries only what R09.T12.a makes of it.
    pub seafloor_origin: Option<SeafloorOrigin>,
    /// The steady-state age `τ_ss` of the cell's sea floor, on oceanic crust, `None` elsewhere.
    ///
    /// R09.T12.a's rank of the kinematic age ([`SeafloorOrigin::age`]) by area onto the
    /// triangular area–age distribution, which GDH1, the pelagic drape and the plains read
    /// (`decision-r09-t12-coarse.md` item 1). It does not travel.
    pub seafloor_age: Option<Seconds>,
    /// Whether the cell's plate subducts at its nearest boundary (R09.T11's polarity).
    ///
    /// True on a subduction boundary's subducting side, its boundary cells, at +0, included, so that a cell's
    /// signed distance is negative exactly where this holds and the distance is above +0; false
    /// without a boundary and at any boundary that is not a subduction. R09.T12.b reads it to put a
    /// trench on the subducting side and to measure an arc from the trench's axis. It does not
    /// travel.
    pub subducts: bool,
}

impl CellState {
    /// Whether the cell is one of a plate boundary's own cells.
    ///
    /// Its signed distance is exactly +0, which R09.T11 writes there and nowhere else, every link
    /// being longer than zero (compared by `total_cmp`, so that only +0 matches).
    #[must_use]
    pub(crate) fn on_boundary(&self) -> bool {
        self.boundary_distance
            .is_some_and(|d| d.value().total_cmp(&0.0).is_eq())
    }
}

/// Where an oceanic cell's floor formed (R09.T11; `decision-r09-t5.md` item 5): the ridge of its
/// own plate that it came from, the spreading rate there and its age.
///
/// The pass has no plate history, so the rate at formation is today's rate at the source ridge
/// (labelled). Plain data with public fields, as [`CellState`] is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeafloorOrigin {
    /// `d_r`, the great-circle distance over the plate's own cells to its nearest divergent
    /// boundary, along the cell graph's shortest path (Design notes 6 and 7): zero on the ridge,
    /// and `None` where the plate has no divergent boundary its cells reach.
    pub ridge_distance: Option<Metres>,
    /// `u_h`, the half-rate of the ridge segment the floor came from, positive: the opening rate
    /// normal to the ridge at the source cell of the own-ridge distance transform, halved
    /// (symmetric spreading), which every cell the source reaches inherits; the mean half-rate,
    /// [`MEAN_HALF_RATE`](plates::MEAN_HALF_RATE), without a ridge.
    pub half_rate: MetresPerSecond,
    /// τ, the kinematic age, `d_r` ÷ `u_h` capped at [`OLDEST_FLOOR`](plates::OLDEST_FLOOR)
    /// (200 Myr), which R09.T12.a ranks into [`CellState::seafloor_age`]; the cap without a
    /// ridge.
    pub age: Seconds,
}

/// A month's 10 m wind: its mean speed and the direction its resultant blows towards, clockwise
/// from local north (`decision-r09-t2.md` item 1).
///
/// Plain data with public fields; the quantiser checks both against the record's codes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindSample {
    /// The month's mean scalar speed, non-negative.
    pub speed: MetresPerSecond,
    /// The resultant's azimuth, radians clockwise from local north in the body-fixed frame: any
    /// finite angle, taken modulo a turn (the record's code wraps it).
    pub azimuth: Radians,
}

impl WindSample {
    /// No wind.
    pub const CALM: Self = Self {
        speed: MetresPerSecond::ZERO,
        azimuth: Radians::ZERO,
    };
}

/// One climate cell's state in the pass, at the climate layer's level (Design note 17).
///
/// Plain data with public fields, as [`CellState`] is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClimateState {
    /// The annual mean surface temperature reduced to sea level by the lapse rate.
    pub sea_level_temperature: Kelvin,
    /// Each month's mean temperature less the annual mean, kelvin; none past the year's months,
    /// and none in a one-month year.
    pub month_anomaly_k: [f64; 12],
    /// Each month's precipitation rate, kg m⁻² s⁻¹; none past the year's months.
    pub month_precipitation_kg_m2_s: [f64; 12],
    /// Each month's 10 m wind; calm past the year's months.
    pub month_wind: [WindSample; 12],
}

/// The pass's working state: every cell's and climate cell's, the coarse craters, and the body's
/// figures that the header carries, all in SI units.
///
/// Plain data with public fields, which the steps write in turn; [`quantise`](super::quantise())
/// checks it whole.
#[derive(Debug, Clone, PartialEq)]
pub struct Working {
    /// Each cell's state, in cell-index order.
    pub cells: Vec<CellState>,
    /// Each climate cell's state, in the climate layer's cell-index order.
    pub climate: Vec<ClimateState>,
    /// The craters of the boundary diameter and wider. The list's order is not output: the
    /// quantiser sorts them by (centre cell, diameter), a key no two may share. The order in which
    /// a step smooths them into the elevation is a summation order, and so is output: draw order.
    pub craters: Vec<CoarseCrater>,
    /// The sea surface's height above the datum: on a world with no sea, the lowest elevation.
    pub sea_level: Metres,
    /// The lapse rate the climate's temperatures are reduced to sea level with, kelvin per metre,
    /// positive where the temperature falls with height (Earth's standard atmosphere: 0.0065).
    pub lapse_rate_k_per_m: f64,
    /// The spectrum of relief finer than the cells, which fixes the synthesis's amplitudes.
    pub spectrum: BandSpectrum,
    /// The months of the year, 1 or 12.
    pub months: u8,
    /// The whole surface's realised `σ_h`, the reconstructed field's variance with what the
    /// synthesis adds over it at its band limit (R09.T12.e, and again R09.T14.d).
    pub realised_sigma_h: Metres,
    /// The realised greatest relief, the cells' greatest less least elevation, which bounds the
    /// reconstructed field (R09.T12.e, and again R09.T14.d).
    pub realised_relief: Metres,
    /// The plates, in plate-index order (R09.T11): none on a world without them.
    pub plates: Vec<plates::Plate>,
    /// Whether the erosion step routed the field, writing its flow, drainage, steepness and
    /// runoffs (R09.T14.c): false on a world never wet, where the surface is no older than its
    /// wet epoch's end, and before the step, where every cell stays terminal with no drainage,
    /// steepness or runoff (decision-r09-t14-choices, "T14.d follow-up" C), so that R09.T14.d's
    /// re-routing after the late craters runs only where the step routed.
    pub(crate) routed: bool,
    /// The climate step's own records, its normalised model on its grid, which R09.T14.d's
    /// re-application of the lapse term and the mean after the erosion and its lakes' net loss
    /// read (`climate::after_erosion`; decision-r09-t14-choices Q6(b) and Q3). Owned by the
    /// climate step, which sets it, and no output: the quantiser does not read it. `None` before
    /// the climate step.
    pub(crate) climate_records: Option<climate::after_erosion::ClimateRecords>,
}

impl Working {
    /// The state before the first step: a smooth, dry sphere at the datum on one stagnant lid, no
    /// plates, no water, no drainage or runoff, no ice, no craters and no relief finer than the
    /// cells, under a still climate at the body's mean surface temperature everywhere, with no
    /// lapse rate, in the inputs' months.
    ///
    /// Its realised `σ_h` and relief are zero and its sea level is its lowest elevation, zero,
    /// which is what a flat field realises. Each step that changes the elevation keeps them true.
    ///
    /// # Panics
    ///
    /// Never: the default spectrum is valid.
    #[must_use]
    pub fn initial(pass: &Pass<'_>) -> Self {
        let cell = CellState {
            elevation: Metres::ZERO,
            water_surface: Metres::ZERO,
            plate: 0,
            crust: Crust::Lid,
            boundary: BoundaryKind::Absent,
            boundary_distance: None,
            boundary_obliquity: Radians::ZERO,
            flow: FlowDirection::Terminal,
            drainage: SquareMetres::ZERO,
            steepness_index_m0_9: 0.0,
            runoff_m_per_year: 0.0,
            catchment_runoff_m_per_year: 0.0,
            channel_incision: Metres::ZERO,
            ice_fraction: 0.0,
            ice_entry: None,
            liquid_entry: None,
            class: SurfaceClass::UNCLASSIFIED,
            crater_state: 0,
            hill_relief: None,
            ponded_sediment: Metres::ZERO,
            seafloor_origin: None,
            seafloor_age: None,
            subducts: false,
        };
        let climate = ClimateState {
            sea_level_temperature: pass.inputs().mean_surface_temperature(),
            month_anomaly_k: [0.0; 12],
            month_precipitation_kg_m2_s: [0.0; 12],
            month_wind: [WindSample::CALM; 12],
        };
        let cells = pass.grid().len();
        let climate_cells = pass.climate_grid().len();
        Self {
            cells: vec![cell; usize::try_from(cells).expect("a coarse level's cells fit a usize")],
            climate: vec![
                climate;
                usize::try_from(climate_cells)
                    .expect("a climate level's cells fit a usize")
            ],
            craters: Vec::new(),
            sea_level: Metres::ZERO,
            lapse_rate_k_per_m: 0.0,
            // A silicate crust's law anchored at zero, no relief finer than the cells, until
            // R09.T12.e anchors the body's crust class (`decision-r09-t5.md` item 3).
            spectrum: BandSpectrum::anchored(
                SpectrumShape::SILICATE,
                pass.inputs().radius(),
                Metres::ZERO,
            ),
            months: pass.inputs().months(),
            realised_sigma_h: Metres::ZERO,
            realised_relief: Metres::ZERO,
            plates: Vec::new(),
            routed: false,
            climate_records: None,
        }
    }
}
