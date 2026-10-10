//! The coarse pass's steps and the working state they share (plan R09, Design note 5).
//!
//! The pass is one job in a fixed order, the brainstorm's ("The coarse global pass, once per
//! planet"): [`plates`] (R09.T11), [`relief`] (R09.T12.a–c and e), [`craters`] (R09.T12.d),
//! [`climate`] (R09.T13), [`erosion`] (R09.T14) and [`classes`] (R09.T15, with the crater
//! state). Each step is a function of the [`Pass`], which holds the seed, the inputs and the
//! grids, and of the [`Working`] state the steps before it left, which it updates in place. Every
//! loop runs in cell-index order and every sum in that order, every sort is keyed by (value, cell
//! index), the arithmetic is `f64`, and no `HashMap` is used (Design note 5). The working state
//! is `f64` throughout; only [`quantise`](super::quantise) turns it into the field, whose codes are
//! the only input of the synthesis (Design note 17).
//!
//! One exception to the order, Design note 5's: on a world whose wet epoch ended before its
//! surface age ran out (a Mars), each coarse crater is drawn a formation age, and only those older
//! than the epoch's end are smoothed in before erosion; the younger ones are applied after it.
//! R09.T12.d and R09.T14 split the crater step for it.
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
//! As built by R09.T10, every step is a no-op, so the pass yields its initial state: a smooth
//! sphere at the datum ([`Working::initial`]).

pub mod classes;
pub mod climate;
pub mod craters;
pub mod erosion;
pub mod plates;
pub mod relief;

use hyperion_surface::field::{
    BoundaryKind, CoarseCrater, CoarseLevel, Crust, FlowDirection, SurfaceClass,
};
use hyperion_surface::synth::BandSpectrum;

use super::grid::CellGraph;
use super::inputs::CoarseInputs;
use crate::planetary::hooks::SurfaceSeed;
use crate::units::{Kelvin, Metres, MetresPerSecond, Radians, SquareMetres};

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

/// One cell's state in the pass, in SI units: what its synthesis record will hold before it is
/// quantised (Design note 17).
///
/// Plain data with public fields, which the steps write in turn; [`quantise`](super::quantise)
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
    /// The area draining through the cell, its own included once it is routed.
    pub drainage: SquareMetres,
    /// The channel steepness index `k_s` = S A^θ with θ = 0.45, m^0.9, non-negative.
    pub steepness_index_m0_9: f64,
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
/// Plain data with public fields, which the steps write in turn; [`quantise`](super::quantise)
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
    /// The reconstructed field's realised `σ_h` (R09.T12.e, and again R09.T14.d).
    pub realised_sigma_h: Metres,
    /// The reconstructed field's realised greatest relief (R09.T12.e, and again R09.T14.d).
    pub realised_relief: Metres,
}

impl Working {
    /// The state before the first step: a smooth, dry sphere at the datum on one stagnant lid, no
    /// plates, no water, no ice, no craters and no relief finer than the cells, under a still
    /// climate at the body's mean surface temperature everywhere, with no lapse rate, in the
    /// inputs' months.
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
            ice_fraction: 0.0,
            ice_entry: None,
            liquid_entry: None,
            class: SurfaceClass::UNCLASSIFIED,
            crater_state: 0,
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
            spectrum: BandSpectrum::new(BandSpectrum::DEFAULT_EXPONENT, SquareMetres::ZERO)
                .expect("the default spectrum is valid"),
            months: pass.inputs().months(),
            realised_sigma_h: Metres::ZERO,
            realised_relief: Metres::ZERO,
        }
    }
}
