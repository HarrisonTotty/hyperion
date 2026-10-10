//! The quantiser: the pass's `f64` working state turned into the field's codes, once (plan R09,
//! Design note 17).
//!
//! The quantised field is the only input of the synthesis, on the server and on the client alike,
//! so nothing downstream reads the working state. Each code is the surface crate's own quantiser of
//! its value ([`SynthesisCell::quantise_height`], [`LogArea::from_area`], the header's
//! temperature and anomaly quantisers and the rest), in the steps the header chooses: the finest
//! sea-level temperature and anomaly steps that hold the body's largest departure and anomaly, so
//! that none saturates. A value that no code holds is a fault of the step that wrote it, which
//! [`QuantiseFieldError`] names.

use std::error::Error;
use std::fmt;

use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::{
    BuildFieldError, BuildFieldHeaderError, ClimateCell, CoarseCrater, CoarseField, FieldHeader,
    FieldHeaderParts, LogArea, LogPrecipitation, LogSteepness, NO_ENTRY, PrecipitationSource,
    QuantiseValueError, SynthesisCell, Wind, cell_index,
};

use super::inputs::CoarseInputs;
use super::steps::{CellState, ClimateState, Working};
use crate::units::Metres;

/// Why [`quantise`] could not turn a working state into a field.
#[derive(Debug, Clone, PartialEq)]
pub enum QuantiseFieldError {
    /// The working state holds another number of cells or climate cells than its level has.
    Counts {
        /// The array's name.
        part: &'static str,
        /// The level's count.
        expected: u32,
        /// The array's length.
        found: usize,
    },
    /// A cell's value has no code.
    Cell {
        /// The cell's index.
        cell: u32,
        /// The value's name.
        part: &'static str,
        /// Why it has none.
        error: QuantiseValueError,
    },
    /// A climate cell's value has no code.
    Climate {
        /// The climate cell's index.
        cell: u32,
        /// The value's name.
        part: &'static str,
        /// Why it has none.
        error: QuantiseValueError,
    },
    /// No step holds the body's largest sea-level temperature departure or monthly anomaly.
    Step {
        /// Which step.
        part: &'static str,
        /// The largest value, kelvin.
        largest_k: f64,
    },
    /// The header's parts are not valid.
    Header(BuildFieldHeaderError),
    /// The records do not make a valid field.
    Field(BuildFieldError),
}

impl fmt::Display for QuantiseFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Counts {
                part,
                expected,
                found,
            } => write!(f, "{found} {part} for the level's {expected}"),
            Self::Cell { cell, part, .. } => write!(f, "cell {cell}'s {part} has no code"),
            Self::Climate { cell, part, .. } => {
                write!(f, "climate cell {cell}'s {part} has no code")
            }
            Self::Step { part, largest_k } => {
                write!(f, "no {part} step holds {largest_k} K")
            }
            Self::Header(_) => f.write_str("the field's header parts are not valid"),
            Self::Field(_) => f.write_str("the records do not make a valid field"),
        }
    }
}

impl Error for QuantiseFieldError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cell { error, .. } | Self::Climate { error, .. } => Some(error),
            Self::Header(error) => Some(error),
            Self::Field(error) => Some(error),
            Self::Counts { .. } | Self::Step { .. } => None,
        }
    }
}

/// The coarse field of `working`, the pass's final state over the body of `inputs`: its header,
/// a synthesis record per cell, a climate record per climate cell and the craters sorted by
/// (centre cell, diameter).
///
/// The header's reference temperature is plan 14's mean surface temperature, and its steps are
/// the finest that hold the climate's largest departure from it and its largest anomaly over the
/// year's months. Its palette, the entry of each crust's lithology and the main liquid are the
/// inputs' ([`CoarseInputs::palette`], [`CoarseInputs::crust_palette`],
/// [`CoarseInputs::main_liquid`]), and each cell's substance byte packs its ice and liquid
/// entries. The header's parts that travel as raw `f64` bits (the sea level, the lapse rate
/// and the realised figures) have a −0 made +0, so that a zero has one form on the wire whichever
/// way a step reached it.
///
/// Public, beside [`coarse_pass`](super::coarse_pass), so that a bench or test can quantise the
/// state a step leaves (R09.T14's and R09.T16's benches).
///
/// # Errors
///
/// [`QuantiseFieldError`] for arrays of another length than the level's, a value no code holds
/// (each naming its cell and value, a palette entry of 15 or more among them), a departure or
/// anomaly no step holds, header parts out of range, or records that break a rule of the field
/// ([`CoarseField::new`]: an entry past the palette or of the wrong role, or ice or a liquid whose
/// entry the cell does not name, among them).
///
/// # Examples
///
/// The initial state of a pass, quantised: a flat field at the datum.
///
/// ```
/// # use hyperion_sim::planetary::surface::{CellGraph, CoarseInputs, SurfaceSeed};
/// use hyperion_sim::planetary::surface::quantise;
/// use hyperion_sim::planetary::surface::steps::{Pass, Working};
/// use hyperion_surface::field::{FieldView, coarse_level};
/// # fn inputs() -> CoarseInputs { unimplemented!() }
/// # fn run(inputs: &CoarseInputs) -> Result<(), Box<dyn std::error::Error>> {
/// let level = coarse_level(inputs.radius());
/// let (grid, climate) = (CellGraph::for_field(level), CellGraph::for_climate(level));
/// let pass = Pass::new(SurfaceSeed::new(7), inputs, &grid, &climate);
/// let field = quantise(inputs, Working::initial(&pass))?;
/// assert!(field.synthesis().iter().all(|cell| cell.elevation_mm == 0));
/// # Ok(())
/// # }
/// ```
pub fn quantise(
    inputs: &CoarseInputs,
    working: Working,
) -> Result<CoarseField, QuantiseFieldError> {
    let reference = inputs.mean_surface_temperature();
    let months = usize::from(working.months);
    let largest_departure = working
        .climate
        .iter()
        .map(|c| (c.sea_level_temperature.value() - reference.value()).abs())
        .fold(0.0, f64::max);
    let largest_anomaly = if months > 1 {
        working
            .climate
            .iter()
            .flat_map(|c| c.month_anomaly_k.iter().take(months))
            .map(|a| a.abs())
            .fold(0.0, f64::max)
    } else {
        0.0
    };
    let temperature_step =
        FieldHeader::temperature_step_for(largest_departure).ok_or(QuantiseFieldError::Step {
            part: "temperature",
            largest_k: largest_departure,
        })?;
    let anomaly_step =
        FieldHeader::anomaly_step_for(largest_anomaly).ok_or(QuantiseFieldError::Step {
            part: "anomaly",
            largest_k: largest_anomaly,
        })?;
    let header = FieldHeader::new(FieldHeaderParts {
        body: inputs.body(),
        radius: inputs.radius(),
        figure: inputs.figure(),
        sea_level: Metres::new(unsigned_zero(working.sea_level.value())),
        lapse_rate_k_per_m: unsigned_zero(working.lapse_rate_k_per_m),
        spectrum: working.spectrum,
        craters: *inputs.craters(),
        climate_model: inputs.climate().model,
        precipitation: PrecipitationSource::Heuristic,
        realised_sigma_h: Metres::new(unsigned_zero(working.realised_sigma_h.value())),
        realised_relief: Metres::new(unsigned_zero(working.realised_relief.value())),
        months: working.months,
        season_eccentricity: inputs.seasonal_orbit().map_or(0.0, |o| o.eccentricity),
        reference_temperature: reference,
        temperature_step,
        anomaly_step,
        surface_age: inputs.surface_age(),
        surface_pressure: inputs.surface_pressure(),
        albedo_scale: None,
        // The field owns its palette while the borrowed inputs keep theirs: one copy of at most
        // 15 entries a field, cold beside the cells.
        palette: inputs.palette().clone(),
        crust_palette: inputs.crust_palette(),
        main_liquid: inputs.main_liquid(),
    })
    .map_err(QuantiseFieldError::Header)?;
    let level = header.level();
    check_count("cells", level.cell_count(), working.cells.len())?;
    check_count(
        "climate cells",
        level.climate_cell_count(),
        working.climate.len(),
    )?;
    let synthesis = (0_u32..)
        .zip(&working.cells)
        .map(|(n, cell)| synthesis_record(n, cell))
        .collect::<Result<Vec<_>, _>>()?;
    let climate = (0_u32..)
        .zip(&working.climate)
        .map(|(n, cell)| climate_record(&header, n, cell))
        .collect::<Result<Vec<_>, _>>()?;
    let craters = sorted_craters(level.get(), working.craters);
    CoarseField::new(header, synthesis, climate, craters).map_err(QuantiseFieldError::Field)
}

/// `x` with a −0 made +0, and every other value as it is: −0 + 0 is +0 under round-to-nearest.
#[must_use]
fn unsigned_zero(x: f64) -> f64 {
    x + 0.0
}

/// Refuses an array of `found` entries for a level of `expected`.
fn check_count(part: &'static str, expected: u32, found: usize) -> Result<(), QuantiseFieldError> {
    if u32::try_from(found) == Ok(expected) {
        Ok(())
    } else {
        Err(QuantiseFieldError::Counts {
            part,
            expected,
            found,
        })
    }
}

/// The synthesis record of cell `n` in state `cell`.
fn synthesis_record(n: u32, cell: &CellState) -> Result<SynthesisCell, QuantiseFieldError> {
    let at = |part: &'static str| {
        move |error| QuantiseFieldError::Cell {
            cell: n,
            part,
            error,
        }
    };
    Ok(SynthesisCell {
        elevation_mm: SynthesisCell::quantise_height(cell.elevation).map_err(at("elevation"))?,
        boundary_distance_km: SynthesisCell::quantise_boundary_distance(cell.boundary_distance)
            .map_err(at("boundary distance"))?,
        plate: cell.plate,
        crust: cell.crust,
        boundary: cell.boundary,
        boundary_obliquity: SynthesisCell::quantise_obliquity(cell.boundary_obliquity)
            .map_err(at("boundary obliquity"))?,
        flow: cell.flow,
        drainage: LogArea::from_area(cell.drainage).map_err(at("drainage"))?,
        steepness: LogSteepness::from_index_m0_9(cell.steepness_index_m0_9)
            .map_err(at("steepness"))?,
        water_surface_mm: SynthesisCell::quantise_height(cell.water_surface)
            .map_err(at("water surface"))?,
        ice: SynthesisCell::quantise_ice(cell.ice_fraction).map_err(at("ice"))?,
        substances: substance_byte(n, cell)?,
        class: cell.class,
        crater_state: cell.crater_state,
    })
}

/// The substance byte of cell `n` in state `cell`: its ice entry in the high nibble and its liquid
/// entry in the low, [`NO_ENTRY`] for none. An entry the nibble cannot hold, 15 or more, has no
/// code; one past the palette or of the wrong role is [`CoarseField::new`]'s to refuse.
fn substance_byte(n: u32, cell: &CellState) -> Result<u8, QuantiseFieldError> {
    for (part, entry) in [
        ("ice entry", cell.ice_entry),
        ("liquid entry", cell.liquid_entry),
    ] {
        if let Some(entry) = entry
            && entry >= NO_ENTRY
        {
            return Err(QuantiseFieldError::Cell {
                cell: n,
                part,
                error: QuantiseValueError::OutOfRange(f64::from(entry)),
            });
        }
    }
    Ok(
        SynthesisCell::pack_substances(cell.ice_entry, cell.liquid_entry)
            .expect("both entries are below the nibble's none"),
    )
}

/// The climate record of climate cell `n` in state `cell`, in `header`'s steps and months.
fn climate_record(
    header: &FieldHeader,
    n: u32,
    cell: &ClimateState,
) -> Result<ClimateCell, QuantiseFieldError> {
    let at = |part: &'static str| {
        move |error| QuantiseFieldError::Climate {
            cell: n,
            part,
            error,
        }
    };
    let months = usize::from(header.months());
    let mut month_anomaly = [0_i8; 12];
    let mut month_precipitation = [LogPrecipitation::NONE; 12];
    for month in 0..months {
        if months > 1 {
            month_anomaly[month] = header
                .quantise_month_anomaly(cell.month_anomaly_k[month])
                .map_err(at("monthly anomaly"))?;
        }
        month_precipitation[month] =
            LogPrecipitation::from_rate(cell.month_precipitation_kg_m2_s[month])
                .map_err(at("monthly precipitation"))?;
    }
    Ok(ClimateCell {
        sea_level_temperature: header
            .quantise_sea_level_temperature(cell.sea_level_temperature)
            .map_err(at("sea-level temperature"))?,
        month_anomaly,
        month_precipitation,
        wind: record_winds(cell, header.months()).map_err(at("wind"))?,
    })
}

/// The climate record's twelve winds from the state's, in a year of `months` months: each month's
/// 10 m wind, and in a one-month year its one wind followed by eleven calm ones
/// (`decision-r09-t2.md` item 1).
#[must_use = "the record's winds are its quantised climate"]
fn record_winds(cell: &ClimateState, months: u8) -> Result<[Wind; 12], QuantiseValueError> {
    let mut winds = [Wind::CALM; 12];
    let held = if months == 1 { 1 } else { winds.len() };
    for (wind, sample) in winds.iter_mut().zip(&cell.month_wind).take(held) {
        *wind = Wind::from_velocity(sample.speed, sample.azimuth)?;
    }
    Ok(winds)
}

/// `craters` sorted by (the cell index of the centre's cell at `level`, diameter), the field's
/// list order; a crater whose centre is not a finite, non-zero vector sorts last, for
/// [`CoarseField::new`] to refuse.
#[must_use]
fn sorted_craters(level: u8, craters: Vec<CoarseCrater>) -> Vec<CoarseCrater> {
    let key = |c: &CoarseCrater| {
        let valid =
            c.centre.iter().all(|v| v.is_finite()) && c.centre.iter().any(|v| v.abs() > 0.0);
        let cell = if valid {
            PatchKey::containing(level, c.centre).map_or(u32::MAX, cell_index)
        } else {
            u32::MAX
        };
        (cell, c.diameter)
    };
    let mut keyed: Vec<((u32, Metres), CoarseCrater)> =
        craters.into_iter().map(|c| (key(&c), c)).collect();
    keyed.sort_by(|(a, _), (b, _)| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.value().total_cmp(&b.1.value()))
    });
    keyed.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::super::steps::{Pass, WindSample};
    use super::super::{grid::CellGraph, reference};
    use super::*;
    use crate::planetary::hooks::SurfaceSeed;
    use crate::units::{Kelvin, MetresPerSecond, Radians, SquareMetres};
    use hyperion_surface::field::{CoarseLevel, Cover, FieldView, Morphology, PaletteRole};
    use hyperion_surface::substance_key::SubstanceKey;
    use hyperion_testkit::float::assert_same_bits;

    fn moon_working() -> (CoarseInputs, CellGraph, CellGraph) {
        let inputs = reference::moon_like();
        let level = hyperion_surface::field::coarse_level(inputs.radius());
        (
            inputs,
            CellGraph::for_field(level),
            CellGraph::for_climate(level),
        )
    }

    #[test]
    fn the_initial_state_quantises_to_a_flat_still_field() {
        let (inputs, grid, climate) = moon_working();
        let pass = Pass::new(SurfaceSeed::new(1), &inputs, &grid, &climate);
        let field = quantise(&inputs, Working::initial(&pass)).unwrap();
        assert_eq!(field.header().level(), CoarseLevel::new(6).unwrap());
        assert!(field.synthesis().iter().all(|c| c.elevation_mm == 0
            && c.water_surface_mm == 0
            && c.boundary_distance_km == SynthesisCell::NO_BOUNDARY_KM
            && c.substances == SynthesisCell::NO_SUBSTANCES));
        assert!(
            field
                .climate_layer()
                .iter()
                .all(|c| c.sea_level_temperature == 0
                    && c.month_anomaly == [0; 12]
                    && c.wind.iter().all(|&w| w == Wind::CALM))
        );
        assert!(field.craters().is_empty());
        assert_eq!(
            field.header().reference_temperature(),
            inputs.mean_surface_temperature()
        );
        assert_eq!(field.header().temperature_step(), 0);
        assert_eq!(field.header().months(), 12);
    }

    #[test]
    fn values_reach_their_codes_and_a_value_no_code_holds_is_named() {
        let (inputs, grid, climate) = moon_working();
        let pass = Pass::new(SurfaceSeed::new(1), &inputs, &grid, &climate);
        let mut working = Working::initial(&pass);
        working.cells[7].elevation = Metres::new(1_234.567_4);
        working.cells[7].water_surface = Metres::new(1_234.567_4);
        working.cells[7].drainage = SquareMetres::new(1e9);
        working.climate[3].sea_level_temperature =
            Kelvin::new(inputs.mean_surface_temperature().value() + 400.0);
        working.climate[3].month_anomaly_k[5] = -20.0;
        working.sea_level = Metres::new(-0.0);
        let field = quantise(&inputs, working.clone()).unwrap();
        assert_eq!(field.synthesis()[7].elevation_mm, 1_234_567);
        // A −0 sea level is written +0, so that a zero has one form on the wire.
        assert_same_bits(field.header().sea_level().value(), 0.0);
        assert_eq!(
            field.synthesis()[7].drainage,
            LogArea::from_area(SquareMetres::new(1e9)).unwrap()
        );
        // 400 K needs the step 0.01 K × 2 to fit an i16, and 20 K the anomaly step 0.25 K.
        assert_eq!(field.header().temperature_step(), 1);
        assert_eq!(field.header().anomaly_step(), 0);
        assert_eq!(field.climate_layer()[3].sea_level_temperature, 20_000);
        assert_eq!(field.climate_layer()[3].month_anomaly[5], -80);

        working.cells[9].ice_fraction = 1.5;
        let error = quantise(&inputs, working.clone()).unwrap_err();
        assert_eq!(
            error,
            QuantiseFieldError::Cell {
                cell: 9,
                part: "ice",
                error: QuantiseValueError::OutOfRange(1.5),
            }
        );
        assert_eq!(error.to_string(), "cell 9's ice has no code");
        assert!(Error::source(&error).is_some());
        working.cells[9].ice_fraction = 0.0;
        working.cells[11].water_surface = Metres::new(-1.0);
        assert_eq!(
            quantise(&inputs, working.clone()),
            Err(QuantiseFieldError::Field(
                BuildFieldError::WaterBelowGround { cell: 11 }
            ))
        );
        working.cells[11].water_surface = Metres::ZERO;
        let mut short = working.clone();
        short.cells.pop();
        assert!(matches!(
            quantise(&inputs, short),
            Err(QuantiseFieldError::Counts { part: "cells", .. })
        ));
    }

    #[test]
    fn a_climate_no_code_or_step_holds_and_bad_header_parts_are_named() {
        let (inputs, grid, climate) = moon_working();
        let pass = Pass::new(SurfaceSeed::new(1), &inputs, &grid, &climate);
        let initial = Working::initial(&pass);
        let mut wet = initial.clone();
        wet.climate[2].month_precipitation_kg_m2_s[4] = -1.0;
        assert_eq!(
            quantise(&inputs, wet),
            Err(QuantiseFieldError::Climate {
                cell: 2,
                part: "monthly precipitation",
                error: QuantiseValueError::OutOfRange(-1.0),
            })
        );
        let mut hot = initial.clone();
        hot.climate[0].sea_level_temperature = Kelvin::new(1e9);
        let error = quantise(&inputs, hot).unwrap_err();
        assert!(
            matches!(
                error,
                QuantiseFieldError::Step {
                    part: "temperature",
                    ..
                }
            ),
            "{error:?}"
        );
        let mut sunk = initial;
        sunk.realised_sigma_h = Metres::new(-1.0);
        assert!(matches!(
            quantise(&inputs, sunk),
            Err(QuantiseFieldError::Header(
                BuildFieldHeaderError::Negative {
                    part: "realised sigma_h"
                }
            ))
        ));
    }

    /// The header carries the inputs' palette, crust entries and main liquid, each cell its ice
    /// and liquid entries, and an entry no nibble, palette or role holds is named.
    #[test]
    fn the_header_carries_the_palette_and_each_cell_its_substances() {
        let inputs = reference::ceres_like();
        let level = hyperion_surface::field::coarse_level(inputs.radius());
        let (grid, climate) = (CellGraph::for_field(level), CellGraph::for_climate(level));
        let pass = Pass::new(SurfaceSeed::new(1), &inputs, &grid, &climate);
        let mut working = Working::initial(&pass);
        let water_ice = SubstanceKey::new("H2O").unwrap();
        let ice = inputs.palette().find(PaletteRole::Ice, water_ice).unwrap();
        working.cells[5].ice_fraction = 0.5;
        working.cells[5].ice_entry = Some(ice);
        // A cell may name an ice it holds none of, as a seasonal frost's.
        working.cells[6].ice_entry = Some(ice);
        let field = quantise(&inputs, working.clone()).unwrap();
        let header = field.header();
        assert_eq!(header.palette(), inputs.palette());
        assert_eq!(header.crust_palette(), inputs.crust_palette());
        assert_eq!(header.main_liquid(), None);
        let cells = field.synthesis();
        assert_eq!(
            (cells[5].ice_entry(), cells[5].liquid_entry()),
            (Some(ice), None)
        );
        assert_eq!(cells[6].ice_entry(), Some(ice));
        assert_eq!(cells[7].substances, SynthesisCell::NO_SUBSTANCES);

        let refused = |edit: &dyn Fn(&mut CellState)| {
            let mut bad = working.clone();
            edit(&mut bad.cells[9]);
            quantise(&inputs, bad).unwrap_err()
        };
        assert_eq!(
            refused(&|c| c.ice_fraction = 0.2),
            QuantiseFieldError::Field(BuildFieldError::IceWithoutEntry { cell: 9 })
        );
        assert_eq!(
            refused(&|c| c.water_surface = Metres::new(10.0)),
            QuantiseFieldError::Field(BuildFieldError::LiquidWithoutEntry { cell: 9 })
        );
        assert_eq!(
            refused(&|c| c.liquid_entry = Some(15)),
            QuantiseFieldError::Cell {
                cell: 9,
                part: "liquid entry",
                error: QuantiseValueError::OutOfRange(15.0),
            }
        );
        let beyond = u8::try_from(inputs.palette().len()).unwrap();
        assert_eq!(
            refused(&|c| c.ice_entry = Some(beyond)),
            QuantiseFieldError::Field(BuildFieldError::SubstanceIndex { cell: 9 })
        );
        // The crust's entry is no ice's.
        let crust = inputs.crust_palette()[2];
        assert_eq!(
            refused(&|c| c.ice_entry = crust),
            QuantiseFieldError::Field(BuildFieldError::SubstanceRole { cell: 9 })
        );
    }

    #[test]
    fn each_month_keeps_its_wind_and_a_one_month_year_one() {
        let (inputs, grid, climate) = moon_working();
        let pass = Pass::new(SurfaceSeed::new(1), &inputs, &grid, &climate);
        let mut state = Working::initial(&pass).climate[0];
        let east = Radians::new(core::f64::consts::FRAC_PI_2);
        for (speed, wind) in (1_u8..).zip(state.month_wind.iter_mut()) {
            *wind = WindSample {
                speed: MetresPerSecond::new(f64::from(speed)),
                azimuth: east,
            };
        }
        let winds = record_winds(&state, 12).unwrap();
        for (month, (wind, sample)) in winds.iter().zip(&state.month_wind).enumerate() {
            assert_eq!(
                *wind,
                Wind::from_velocity(sample.speed, east).unwrap(),
                "month {month}"
            );
        }
        assert_ne!(winds[0], winds[11]);
        let one = record_winds(&state, 1).unwrap();
        assert_eq!(one[0], winds[0]);
        assert!(one[1..].iter().all(|&w| w == Wind::CALM));
    }

    #[test]
    fn the_header_carries_the_seasonal_orbit_s_eccentricity() {
        let (inputs, grid, climate) = moon_working();
        let pass = Pass::new(SurfaceSeed::new(1), &inputs, &grid, &climate);
        let field = quantise(&inputs, Working::initial(&pass)).unwrap();
        let e = inputs.seasonal_orbit().unwrap().eccentricity;
        assert_same_bits(field.header().season_eccentricity(), e);
    }

    #[test]
    fn craters_are_listed_by_centre_cell_then_diameter() {
        let (inputs, grid, climate) = moon_working();
        let pass = Pass::new(SurfaceSeed::new(1), &inputs, &grid, &climate);
        let mut working = Working::initial(&pass);
        let d_b = hyperion_surface::field::boundary_diameter(pass.level(), inputs.radius());
        let crater = |centre: [f64; 3], km: f64| {
            let cell = cell_index(PatchKey::containing(6, centre).unwrap());
            CoarseCrater {
                centre,
                diameter: Metres::new(d_b.value() + km * 1e3),
                morphology: Morphology::PeakRing,
                age: crate::units::Gigayears::new(3.9),
                degradation: 0,
                reach: Cover::from_cells([cell]),
            }
        };
        working.craters = vec![
            crater([0.0, 0.0, 1.0], 10.0),
            crater([1.0, 0.0, 0.0], 30.0),
            crater([1.0, 0.0, 0.0], 20.0),
        ];
        let field = quantise(&inputs, working).unwrap();
        let listed: Vec<f64> = field
            .craters()
            .iter()
            .map(|c| c.diameter.value() - d_b.value())
            .collect();
        assert_eq!(listed.len(), 3);
        assert!((listed[0] - 20e3).abs() < 1e-6 && (listed[1] - 30e3).abs() < 1e-6);
        let first = PatchKey::containing(6, [1.0, 0.0, 0.0]).unwrap();
        assert_eq!(field.craters_reaching(first).count(), 2);
    }
}
