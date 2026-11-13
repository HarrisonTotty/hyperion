//! Step 6, the classes and the crater state (R09.T15, Design note 11;
//! `decision-r09-t15-classes.md`).
//!
//! The step writes each cell's [`SurfaceClass`] and its crater state, cell by cell, from what the
//! steps before it left at the final heights: the climate's months, the ices, the liquids and the
//! lakes, and the coarse craters. It reads no stream.
//!
//! # Which scheme
//!
//! A world whose climate is a seasonal water cycle, an [`EnergyBalance`] model over twelve months
//! with water as its condensable (decision-r09-t15-classes §1), classifies its land by
//! Köppen–Geiger ([`KoppenGeiger`], codes 1 to 30), by Peel, Finlayson and McMahon 2007's Table 1
//! with Beck et al. 2018's rules applied to rates over each cell's own summer (`koppen`'s
//! documentation): precipitation exists only on the energy-balance models, and the table's
//! thresholds are water's. Every other world (a one-month year, a locked world, a radiative or
//! isothermal model, another condensable) classifies surface state ([`SurfaceForm`], codes 64 to
//! 72), with its regime's named zones as forms: a locked world's substellar ocean is
//! [`SurfaceForm::Sea`] and its nightside glacier [`SurfaceForm::Ice`]. The classes classify
//! climate and state, not life: no class is a vegetation class, and a lifeless Earth-like world
//! keeps its climate class with bare ground.
//!
//! # The order of decision
//!
//! The first that holds wins (decision-r09-t15-classes §3):
//!
//! 1. [`SurfaceForm::Melt`], of the crust's entry: on every cell where plan 14's state is
//!    [`MagmaOcean`] (its melt area is the whole surface, decision-p14-t54a-states §3.4), and where
//!    the cell's annual mean at its height exceeds the transition of a crust entry that is no ice
//!    (an ice lid's liquid is a condensate, which the climate step places);
//! 2. under liquid: [`SurfaceForm::FrozenSea`] at least half under ice, of the ice entry; else
//!    [`SurfaceForm::Sea`] where the water's surface is the sea level to the bit, or
//!    [`SurfaceForm::Lake`], each of the liquid entry (R09.T14.d: a lake is a cell under liquid
//!    whose surface is not ℓ);
//! 3. on a Köppen–Geiger world, its class, ice sheets included (they classify by their
//!    temperatures; the ice byte says the ice);
//! 4. [`SurfaceForm::Ice`] at least half under ice and [`SurfaceForm::Frost`] under less, of the
//!    ice entry;
//! 5. [`SurfaceForm::OrganicSediment`] under an atmosphere whose palette holds an organic cover, of
//!    that entry;
//! 6. [`SurfaceForm::Regolith`] under less than 30 kPa of air (the photometric threshold reused, a
//!    labelled choice), else [`SurfaceForm::Rock`], each of the crust's entry.
//!
//! The ice share is read on its quantised byte, at least 128 being at least half, so that the
//! classes agree cell by cell with what R10's classifier reads of the field. The pressure is the
//! header's (zero on an airless world, whatever the inputs say). A form whose substance the
//! palette lacks is classed all the same.
//!
//! # The crater state
//!
//! Each cell's [`CraterState`]: the lunar-equivalent crater density N(>1 km) its own surface has
//! seen, n₁ C(a) ÷ C(S), n₁ the body's N☾(>1 km), C the lunar chronology in the body's system and
//! S the surface age (decision-r09-t15-classes §6). Its age a is its crust's (the surface age; a
//! province's age; an oceanic floor's own), or the age of the youngest coarse crater whose rim
//! holds the cell's centre where that is younger, the floor it made being new ground. A surface
//! that shows no craters is [`CraterState::NONE_RETAINED`]: a liquid, a melt, and an ice share of
//! at least half that flows or renews (under an atmosphere, or a volatile ice); an airless body's
//! water ice keeps its craters.
//!
//! [`EnergyBalance`]: ClimateModelKind::EnergyBalance
//! [`MagmaOcean`]: SurfaceState::MagmaOcean

mod koppen;
#[cfg(test)]
mod tests;

use hyperion_surface::craters::lunar_chronology;
use hyperion_surface::field::{
    ClimateModelKind, CraterState, Crust, KoppenGeiger, MaterialPalette, MechanicsFamily,
    PaletteRole, SurfaceClass, SurfaceForm, SynthesisCell, month_edges,
};
use hyperion_surface::material::THIN_ATMOSPHERE_PRESSURE;
use hyperion_surface::num;
use hyperion_surface::substance_key::SubstanceKey;

use super::{CellState, ClimateState, Pass, Working, craters};
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::surface::grid::parent_index;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::planetary::surface::quantise::header_pressure;
use crate::units::{Gigayears, Kelvin, Pascals, PerSquareKilometre, Radians};
use koppen::{MONTHS, Month};

/// Water, the condensable whose cycle Köppen–Geiger's thresholds are fitted to.
const WATER: SubstanceKey = SubstanceKey::new_const("H2O");

/// The quantised ice byte from which a cell is at least half under ice: 128, since no code is
/// exactly a half (code 128 is 0.502), so that "at least a half" and "over a half" agree on the
/// byte, as R10's classifier reads it (decision-r09-t15-classes §3).
const MASSIVE_ICE_CODE: u8 = 128;

/// Runs the step on `working`: every cell's class and crater state.
///
/// # Panics
///
/// If a cell's temperature, ice share or age is not finite, its ice share is outside 0 to 1, or a
/// month's precipitation is not finite or is negative (a fault of the steps before it).
pub fn run(pass: &Pass<'_>, working: &mut Working) {
    let world = World::of(pass.inputs(), working.months);
    let rims = youngest_rims(pass, working);
    let Working {
        cells,
        climate,
        sea_level,
        lapse_rate_k_per_m,
        ..
    } = working;
    let surface = Surface {
        sea_level_m: sea_level.value(),
        lapse_k_per_m: *lapse_rate_k_per_m,
    };
    for ((n, cell), rim) in (0_u32..).zip(cells.iter_mut()).zip(&rims) {
        let state = &climate[usize::try_from(parent_index(n)).expect("a cell index fits a usize")];
        let reading = world.read(cell, state, surface);
        cell.class = reading.class;
        cell.crater_state = world.crater_state(cell, reading, *rim).get();
    }
}

/// The sea level and the lapse rate a cell's temperature is read at.
#[derive(Debug, Clone, Copy)]
struct Surface {
    /// ℓ, m.
    sea_level_m: f64,
    /// Γ, K m⁻¹.
    lapse_k_per_m: f64,
}

/// What the step reads of the body, once.
#[derive(Debug)]
struct World<'a> {
    inputs: &'a CoarseInputs,
    /// The months' edges in mean anomaly on a Köppen–Geiger world, else `None`.
    koppen: Option<[Radians; MONTHS + 1]>,
    /// The header's surface pressure.
    pressure: Pascals,
    /// Whether the palette holds an organic cover and the body an atmosphere.
    organic_cover: bool,
    /// N☾(>1 km) ÷ C(S), the density a unit of the chronology is worth; zero on a surface of age
    /// zero.
    density_per_chronology: PerSquareKilometre,
}

/// A cell's class, with what its crater state reads of it.
#[derive(Debug, Clone, Copy)]
struct Reading {
    class: SurfaceClass,
    /// The cell's quantised ice byte.
    ice_code: u8,
}

impl<'a> World<'a> {
    /// The body of `inputs` over a year of `months` months.
    #[must_use]
    fn of(inputs: &'a CoarseInputs, months: u8) -> Self {
        let regime = inputs.climate();
        let koppen = (regime.model == ClimateModelKind::EnergyBalance
            && usize::from(months) == MONTHS
            && regime.condensable == Some(WATER))
        .then(|| {
            let e = inputs
                .seasonal_orbit()
                .map_or(0.0, |orbit| orbit.eccentricity);
            month_edges(e).expect("a seasonal orbit's eccentricity is in [0, 1)")
        });
        let pressure = header_pressure(inputs);
        let craters = inputs.craters();
        let whole = lunar_chronology(inputs.surface_age(), craters.system_age()).value();
        Self {
            inputs,
            koppen,
            pressure,
            organic_cover: pressure.value() > 0.0 && organic_cover(inputs.palette()).is_some(),
            density_per_chronology: if whole > 0.0 {
                craters.n_1km() / whole
            } else {
                PerSquareKilometre::ZERO
            },
        }
    }

    /// The class of `cell`, whose climate cell is `state`, read at `surface`.
    #[must_use]
    fn read(&self, cell: &CellState, state: &ClimateState, surface: Surface) -> Reading {
        let ice_code = SynthesisCell::quantise_ice(cell.ice_fraction)
            .expect("the ice share is a fraction of the cell");
        let massive = ice_code >= MASSIVE_ICE_CODE;
        let height = cell.water_surface.value() - surface.sea_level_m;
        let annual = state.sea_level_temperature.value() - surface.lapse_k_per_m * height;
        assert!(
            annual.is_finite(),
            "a cell's annual mean is not finite: {annual}"
        );
        let form = |form: SurfaceForm| Reading {
            class: SurfaceClass::from(form),
            ice_code,
        };
        if self.melts(cell.crust, annual) {
            return form(SurfaceForm::Melt);
        }
        if cell.water_surface > cell.elevation {
            return form(if massive {
                SurfaceForm::FrozenSea
            } else if unsigned_zero(cell.water_surface.value())
                .total_cmp(&unsigned_zero(surface.sea_level_m))
                .is_eq()
            {
                SurfaceForm::Sea
            } else {
                SurfaceForm::Lake
            });
        }
        if let Some(edges) = &self.koppen {
            let months: [Month; MONTHS] = core::array::from_fn(|m| Month {
                anomaly_k: state.month_anomaly_k[m],
                precipitation_kg_m2_s: state.month_precipitation_kg_m2_s[m],
            });
            let class: KoppenGeiger = koppen::classify(&months, edges, Kelvin::new(annual)).class;
            return Reading {
                class: SurfaceClass::from(class),
                ice_code,
            };
        }
        form(if massive {
            SurfaceForm::Ice
        } else if ice_code > 0 {
            SurfaceForm::Frost
        } else if self.organic_cover {
            SurfaceForm::OrganicSediment
        } else if self.pressure < THIN_ATMOSPHERE_PRESSURE {
            SurfaceForm::Regolith
        } else {
            SurfaceForm::Rock
        })
    }

    /// Whether ground of `crust` whose annual mean is `annual_k` is melt: everywhere on a magma
    /// ocean, and above its crust entry's transition where that entry is no ice.
    #[must_use]
    fn melts(&self, crust: Crust, annual_k: f64) -> bool {
        if self.inputs.surface_state() == SurfaceState::MagmaOcean {
            return true;
        }
        let palette = self.inputs.palette();
        self.inputs.crust_palette()[usize::from(u8::from(crust))]
            .and_then(|index| palette.get(index))
            .is_some_and(|entry| {
                !matches!(
                    entry.mechanics,
                    MechanicsFamily::WaterIce | MechanicsFamily::VolatileIce
                ) && annual_k > entry.transition.value()
            })
    }

    /// The crater state of `cell`, classed as `reading`, where `rim` is the age of the youngest
    /// coarse crater whose rim holds its centre.
    #[must_use]
    fn crater_state(
        &self,
        cell: &CellState,
        reading: Reading,
        rim: Option<Gigayears>,
    ) -> CraterState {
        let form = reading.class.surface_form();
        let massive = reading.ice_code >= MASSIVE_ICE_CODE;
        let renewing_ice = massive
            && (self.pressure.value() > 0.0
                || cell
                    .ice_entry
                    .and_then(|index| self.inputs.palette().get(index))
                    .is_some_and(|entry| entry.mechanics == MechanicsFamily::VolatileIce));
        let hidden =
            cell.water_surface > cell.elevation || form == Some(SurfaceForm::Melt) || renewing_ice;
        if hidden {
            return CraterState::NONE_RETAINED;
        }
        let crust_age = match cell.crust {
            Crust::Continental | Crust::Lid => self.inputs.surface_age(),
            Crust::Province => self.inputs.province_surface_age(),
            Crust::Oceanic => cell
                .seafloor_age
                .map_or(self.inputs.surface_age(), Gigayears::from),
        };
        let age = rim.map_or(crust_age, |rim| {
            Gigayears::new(num::min(rim.value(), crust_age.value()))
        });
        let since = lunar_chronology(age, self.inputs.craters().system_age()).value();
        CraterState::from_density(self.density_per_chronology * since)
            .expect("a crater density is finite and non-negative")
    }
}

/// The first organic cover of `palette`: its first `Cover` entry of the organic family (R10's
/// rule).
#[must_use]
fn organic_cover(palette: &MaterialPalette) -> Option<u8> {
    (0_u8..)
        .zip(palette.entries())
        .find(|(_, e)| e.role == PaletteRole::Cover && e.mechanics == MechanicsFamily::Organic)
        .map(|(index, _)| index)
}

/// Each cell's youngest coarse crater whose rim holds its centre, by its age: the chord from the
/// crater centre's point on the body's figure to the cell centre's under half the diameter, the
/// chord the craters' lift is profiled at (`craters::each_lift`, through the one
/// `craters::chord_m`).
#[must_use]
fn youngest_rims(pass: &Pass<'_>, working: &Working) -> Vec<Option<Gigayears>> {
    let figure = pass.inputs().figure();
    let grid = pass.grid();
    let mut youngest: Vec<Option<Gigayears>> = vec![None; working.cells.len()];
    for crater in &working.craters {
        let centre = figure.point(crater.centre);
        let rim = 0.5 * crater.diameter.value();
        let age = crater.age.value();
        assert!(
            age.is_finite(),
            "a coarse crater's age is not finite: {age}"
        );
        for cell in crater.reach.cells() {
            if craters::chord_m(centre, figure.point(grid.centre(cell))) < rim {
                let slot = &mut youngest[usize::try_from(cell).expect("a cell index fits a usize")];
                *slot = Some(Gigayears::new(
                    slot.map_or(age, |held| num::min(held.value(), age)),
                ));
            }
        }
    }
    youngest
}

/// `x` with a −0 made +0: −0 + 0 is +0 under round-to-nearest, so that a sea level of either zero
/// matches a surface of either.
#[must_use]
fn unsigned_zero(x: f64) -> f64 {
    x + 0.0
}
