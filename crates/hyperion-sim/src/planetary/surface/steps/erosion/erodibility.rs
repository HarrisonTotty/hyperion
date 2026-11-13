//! The erodibility K of the stream-power law (R09.T14.b, Design note 9's erodibility, R09.T0.b):
//! K = K⊕ (g ÷ g⊕)(`ρ_f` ÷ `ρ_ref`) B, in m^0.1 a⁻¹ at m = 0.45 and n = 1.
//!
//! - **K⊕**, [`EARTH_ERODIBILITY_M0_1_PER_YEAR`], is Tzathas et al. 2024's calibration converted
//!   to m = 0.45 (low to medium-low confidence).
//! - **g** enters linearly: K goes as gⁿ in both the shear-stress and the unit-stream-power forms
//!   (Whipple and Tucker 1999, JGR 104, 17661, eqs. 8–10), g⊕ the pass's own Earth's, so that the
//!   reference Earth's factor is 1 exactly.
//! - **`ρ_f`**, the liquid's density, is taken linearly too: the same forms go as `ρ_f`^(1.5n) and
//!   `ρ_f`ⁿ, which bracket it. The liquid is the world's [`Fluid`]: on a world wet now its main
//!   liquid, the sea's palette entry; on one dry now its wet epoch's, the registry's row at its
//!   triple point (decision-r09-t14-choices Q6a: the present main liquid may be none, as on Mars).
//! - **B**, the bed's factor, is a (substrate, fluid) table, [`BED_FACTORS`]: the bed's mechanics
//!   (the crust's palette entry's [`MechanicsFamily`]) and whether the surface is a
//!   crater-saturated regolith ([`Surface`], by each crust's surface age, [`surface_at`]), against
//!   the fluid by its registry key. Any other pair takes 1, the rock-equivalent, **not modelled**:
//!   the table is a closed set, for the composition audit (plan R09's Risks).
//!
//! On a world wet now the steady state's elevation depends on K only through B's and the runoff's
//! variation within a basin (decision-r09-t14-choices Q1), so its global value cancels there; on a
//! world dry now K t is absolute, and these figures set the epoch's incision.

use hyperion_surface::craters::lunar_chronology;
use hyperion_surface::field::{Crust, MechanicsFamily};
use hyperion_surface::num;
use hyperion_surface::substance_key::SubstanceKey;

use super::super::CellState;
use super::super::relief::convergent::EARTH_GRAVITY;
use crate::planetary::hooks::figures::interior::REGIME_WATER_DENSITY;
use crate::planetary::hooks::figures::relief::BASIN_SATURATION_DENSITY;
use crate::planetary::surface::inputs::{CoarseInputs, WetEpoch};
use crate::units::{Gigayears, MetresPerSecondSquared, PerSquareKilometre};

/// K⊕, the erodibility of Earth's rock under water at the reference runoff, m^0.1 a⁻¹: 6.3 × 10⁻⁶
/// (Design note 9, R09.T0.b).
///
/// Tzathas et al. 2024's 2 × 10⁻⁵ m^(1−2m) a⁻¹ (CGF 43(2), doi:10.1111/cgf.15033, Table 1, with a
/// precipitation of 1 m a⁻¹) is for m = 0.4; at n = 1 it converts to m = 0.45 as K `A_ref`^(0.4
/// − 0.45) at `A_ref` = 10¹⁰ m² (7.1 × 10⁻⁶ at 10⁹ m², 5.6 × 10⁻⁶ at 10¹¹), which used unconverted
/// would erode about three times too fast. It lies at the volcaniclastic end of Stock and
/// Montgomery 1999's field values for m = 0.4 and n = 1 (JGR 104, 4983: 10⁻⁷–10⁻⁶ for granite and
/// metamorphic rock, 10⁻⁵–10⁻⁴ for volcaniclastics). Low to medium-low confidence.
pub(crate) const EARTH_ERODIBILITY_M0_1_PER_YEAR: f64 = 6.3e-6;

/// The surface the channels cut, per world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Surface {
    /// Bedrock, the calibration's own.
    Bedrock,
    /// A crater-saturated, impact-brecciated regolith, such as Mars's Noachian highlands
    /// ([`surface_at`]).
    SaturatedRegolith,
}

/// What a channel's bed is: its mechanics and its surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Substrate {
    /// The mechanics of the crust's palette entry.
    pub(crate) mechanics: MechanicsFamily,
    /// Whether it is bedrock or a saturated regolith.
    pub(crate) surface: Surface,
}

/// One row of the bed table: B for a substrate under a fluid, with its source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedFactor {
    /// The bed.
    pub(crate) substrate: Substrate,
    /// The fluid, by its registry key: matched by substance, not by mechanics family, since
    /// methane, ethane, N₂ and CO₂ all freeze to [`MechanicsFamily::VolatileIce`].
    pub(crate) fluid: &'static str,
    /// B, dimensionless.
    pub(crate) factor: f64,
    /// Where it comes from.
    pub(crate) source: &'static str,
}

/// Rock: silicate bedrock.
const ROCK: Substrate = Substrate {
    mechanics: MechanicsFamily::Silicate,
    surface: Surface::Bedrock,
};

/// A silicate crust's crater-saturated regolith.
const REGOLITH: Substrate = Substrate {
    mechanics: MechanicsFamily::Silicate,
    surface: Surface::SaturatedRegolith,
};

/// Water-ice bedrock, a Titan's crust.
const WATER_ICE: Substrate = Substrate {
    mechanics: MechanicsFamily::WaterIce,
    surface: Surface::Bedrock,
};

/// Water-ice bedrock under a hydrocarbon liquid: 8, a combined bed-and-fluid factor that puts
/// Titan's K at about half of Earth's at equal runoff (Design note 9).
///
/// Collins 2005 (GRL 32, L22202, ¶18) finds Titan's incision rates "likely to be surprisingly
/// similar to terrestrial rates, given similar stream conditions" (saltation abrasion after Sklar
/// and Dietrich 2004: cold ice about 53 times as erodible per unit impact energy as sandstone,
/// offset by grains that strike with about 50 times less energy), and Litwin et al. 2012 (JGR 117,
/// E08013) find ice strengthening by 7 kPa K⁻¹ from 260 K down to 110 K, about twice as strong at
/// Titan's 94 K by extrapolation. With g and `ρ_f` (8 × 0.138 × 0.448 ≈ 0.49).
const ICE_UNDER_HYDROCARBON: f64 = 8.0;

/// The source of [`ICE_UNDER_HYDROCARBON`]'s rows.
const ICE_UNDER_HYDROCARBON_SOURCE: &str =
    "Collins 2005, GRL 32, L22202, ¶18; Litwin et al. 2012, JGR 117, E08013 (Design note 9)";

/// The bed table B(substrate, fluid), R09.T0.b's three calibrated cases (decision-composition):
///
/// - (silicate bedrock, H₂O) 1: the calibration's own (Tzathas et al. 2024; Stock and Montgomery
///   1999).
/// - (silicate saturated regolith, H₂O) 10: Barnhart, Howard and Moore 2009 (JGR 114, E01003, ¶78,
///   "regolith ten times the bedrock's erodibility", in a model that is "essentially the DELIM
///   model as reported by Howard [1994a, 1997, 2007]", ¶71).
/// - (water-ice bedrock, CH₄), and the same under ethane and Titan's CH₄–C₂H₆–N₂ sea, 8
///   ([`ICE_UNDER_HYDROCARBON`]).
///
/// A pair with no row takes its substrate's bedrock row where its surface is a saturated
/// regolith, and any other pair is **not modelled** and takes 1, the rock-equivalent
/// ([`bed_factor`]): a closed set, for the composition audit (plan R09's Risks). A row is added as
/// data. So a crater-saturated water ice under methane takes 8, a **lower bound** for an icy
/// regolith, which is surely at least as erodible as its bedrock: Barnhart et al.'s tenfold is a
/// silicate regolith's under water, and is not carried to ice without a source
/// (decision-r09-t14-choices, "T14.b follow-up" item (b)).
pub(crate) const BED_FACTORS: [BedFactor; 5] = [
    BedFactor {
        substrate: ROCK,
        fluid: "H2O",
        factor: 1.0,
        source: "Tzathas et al. 2024, CGF 43(2), Table 1; Stock and Montgomery 1999, JGR 104, 4983",
    },
    BedFactor {
        substrate: REGOLITH,
        fluid: "H2O",
        factor: 10.0,
        source: "Barnhart, Howard and Moore 2009, JGR 114, E01003, ¶78 and ¶71",
    },
    BedFactor {
        substrate: WATER_ICE,
        fluid: "CH4",
        factor: ICE_UNDER_HYDROCARBON,
        source: ICE_UNDER_HYDROCARBON_SOURCE,
    },
    BedFactor {
        substrate: WATER_ICE,
        fluid: "C2H6",
        factor: ICE_UNDER_HYDROCARBON,
        source: ICE_UNDER_HYDROCARBON_SOURCE,
    },
    BedFactor {
        substrate: WATER_ICE,
        fluid: "ch4_c2h6_n2",
        factor: ICE_UNDER_HYDROCARBON,
        source: ICE_UNDER_HYDROCARBON_SOURCE,
    },
];

/// B for `substrate` under the fluid `fluid`: its row of [`BED_FACTORS`]; for a saturated
/// regolith with no row, its mechanics' bedrock row, a lower bound (decision-r09-t14-choices,
/// "T14.b follow-up" item (b)); else 1, the rock-equivalent, for a pair the table does not hold
/// (not modelled).
#[must_use]
pub(crate) fn bed_factor(substrate: Substrate, fluid: SubstanceKey) -> f64 {
    let row = |substrate: Substrate| {
        BED_FACTORS
            .iter()
            .find(|row| row.substrate == substrate && row.fluid == fluid.as_str())
            .map(|row| row.factor)
    };
    row(substrate)
        .or_else(|| match substrate.surface {
            Surface::SaturatedRegolith => row(Substrate {
                surface: Surface::Bedrock,
                ..substrate
            }),
            Surface::Bedrock => None,
        })
        .unwrap_or(1.0)
}

/// The liquid that cuts the channels: its registry key and its density.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fluid {
    /// The substance, by its registry key.
    pub(crate) key: SubstanceKey,
    /// Its density, kg m⁻³, finite and positive.
    pub(crate) density_kg_m3: f64,
}

impl Fluid {
    /// The fluid of `inputs`' erosion, or `None` for a world never wet (decision-r09-t14-choices
    /// Q6a).
    ///
    /// On a world wet now (its wet epoch ends at 0) it is the main liquid's palette entry, its
    /// liquid record's density; one with no liquid on the ground now takes its epoch's liquid. On
    /// a world dry now it is the epoch's liquid, the registry's row, at its saturated liquid's
    /// density at the triple point (as plan 14's ocean takes water's).
    ///
    /// # Panics
    ///
    /// Never for built inputs: the builder checks the main liquid's entry and the epoch's liquid's
    /// phase diagram.
    #[must_use]
    pub(crate) fn of(inputs: &CoarseInputs) -> Option<Self> {
        let epoch = inputs.wet_epoch()?;
        let wet_now = epoch.end.value() <= 0.0;
        if wet_now && let Some(entry) = inputs.main_liquid() {
            let entry = inputs
                .palette()
                .get(entry)
                .expect("the main liquid's entry is the palette's");
            let liquid = entry
                .liquid
                .as_ref()
                .expect("a liquid entry carries its liquid record");
            return Some(Self {
                key: entry.substance,
                density_kg_m3: liquid.density_kg_m3,
            });
        }
        Some(Self::of_epoch(epoch))
    }

    /// The liquid of the wet epoch `epoch`: its registry row, at its saturated liquid's density at
    /// the triple point (as plan 14's ocean takes water's; decision-r09-t14-choices Q2 and Q6a).
    ///
    /// # Panics
    ///
    /// Never for built inputs: the builder checks the epoch's liquid's phase diagram.
    #[must_use]
    pub(crate) fn of_epoch(epoch: &WetEpoch) -> Self {
        let row = epoch.liquid.substance();
        let density = row
            .phase_diagram()
            .expect("the builder checks the epoch's liquid's phase diagram")
            .triple_liquid_density();
        Self {
            key: row.key(),
            density_kg_m3: density.value(),
        }
    }
}

/// The crater density N(>1 km) the erosion saw on `inputs`' surface, per km²: N☾(>1 km) times the
/// share of the surface's craters that formed before the wet epoch's end,
/// `N_e` = N☾ (C(S) − C(E)) ÷ C(S), C the lunar chronology in the body's system
/// ([`lunar_chronology`]), S the surface age and E the lesser of the epoch's end and S (0 on a
/// world never wet or wet now). Zero on a surface of age zero. It is [`crater_density_seen`] at
/// the surface age.
///
/// The reference Mars's is about 0.050 km⁻², the reference Earth's about 8.4 × 10⁻⁵ (the research
/// subagent's figures, R09.T14's lane design §3.1). Only the erosion's tests read it.
#[cfg(test)]
#[must_use]
pub(crate) fn effective_crater_density(inputs: &CoarseInputs) -> PerSquareKilometre {
    crater_density_seen(inputs, inputs.surface_age())
}

/// The crater density N(>1 km) the erosion saw on a surface of `inputs` `age` old (at most the
/// surface age), per km²: N☾(>1 km) (C(`S_x`) − C(min(E, `S_x`))) ÷ C(S), `S_x` = `age`, the
/// craters on it that formed before the wet epoch's end E (0 on a world never wet or wet now), C
/// the lunar chronology in the body's system ([`lunar_chronology`]) and S the surface age, as
/// the whole surface's count at the surface age (decision-r09-t14-mars-ceiling §2.1).
/// Zero where `age` is at most the epoch's end, and on a surface of age zero.
#[must_use]
pub(crate) fn crater_density_seen(inputs: &CoarseInputs, age: Gigayears) -> PerSquareKilometre {
    let craters = inputs.craters();
    let system = craters.system_age();
    let whole = lunar_chronology(inputs.surface_age(), system).value();
    if whole <= 0.0 {
        return PerSquareKilometre::ZERO;
    }
    let end = num::min(
        inputs.wet_epoch().map_or(0.0, |epoch| epoch.end.value()),
        age.value(),
    );
    let since = lunar_chronology(age, system).value();
    let before_end = lunar_chronology(Gigayears::new(end), system).value();
    craters.n_1km() * ((since - before_end) / whole)
}

/// The surface a channel cut on `inputs` where the ground is `age` old: a crater-saturated
/// regolith where the erosion saw at least [`BASIN_SATURATION_DENSITY`] (0.018 km⁻², the lunar
/// chronology at about 3.8 Ga) of craters on it ([`crater_density_seen`]), else bedrock.
///
/// Lean, the research subagent's criterion as the lane records it (R09.T14's lane design §3.1):
/// the craters the erosion sees are those older than the epoch's end, and a surface saturated with
/// basins is brecciated to depth (Barnhart, Howard and Moore 2009's Noachian regolith). A volcanic
/// province no older than the epoch's end saw none: an erupting surface, permeable and never
/// saturated during the epoch, bedrock (Gulick and Baker 1990; decision-r09-t14-mars-ceiling
/// §2.1).
#[must_use]
pub(crate) fn surface_at(inputs: &CoarseInputs, age: Gigayears) -> Surface {
    if crater_density_seen(inputs, age).value() >= BASIN_SATURATION_DENSITY.value() {
        Surface::SaturatedRegolith
    } else {
        Surface::Bedrock
    }
}

/// Whether `inputs`' surface is a crater-saturated regolith, per world: its
/// `effective_crater_density` at or above [`BASIN_SATURATION_DENSITY`] ([`surface_at`] the
/// surface age). Only the erosion's tests read it.
#[cfg(test)]
#[must_use]
pub(crate) fn crater_saturated(inputs: &CoarseInputs) -> bool {
    surface_at(inputs, inputs.surface_age()) == Surface::SaturatedRegolith
}

/// The surface each [`Crust`] variant's ground shows on `inputs`, in its byte's order: the
/// world's ([`surface_at`] its surface age), but a volcanic province's by the provinces' own
/// surface age ([`CoarseInputs::province_surface_age`]; decision-r09-t14-mars-ceiling §2.1).
#[must_use]
pub(crate) fn crust_surfaces(inputs: &CoarseInputs) -> [Surface; 4] {
    let world = surface_at(inputs, inputs.surface_age());
    let province = surface_at(inputs, inputs.province_surface_age());
    core::array::from_fn(|k| {
        let crust = Crust::try_from(u8::try_from(k).expect("four crusts"))
            .expect("each of the four bytes is a crust");
        match crust {
            Crust::Province => province,
            Crust::Continental | Crust::Oceanic | Crust::Lid => world,
        }
    })
}

/// K = K⊕ (g ÷ g⊕)(`ρ_f` ÷ `ρ_ref`) B, m^0.1 a⁻¹, for a surface gravity `gravity`, the fluid's
/// density `density_kg_m3` and the bed factor `bed`; `ρ_ref` is [`REGIME_WATER_DENSITY`], 1,000
/// kg m⁻³, and g⊕ the pass's Earth's, GM⊕ ÷ (6,371 km)² (`relief::convergent::EARTH_GRAVITY`).
///
/// # Panics
///
/// If the gravity, the density or the bed factor is not finite and positive.
#[must_use]
pub(crate) fn erodibility_m0_1_per_year(
    gravity: MetresPerSecondSquared,
    density_kg_m3: f64,
    bed: f64,
) -> f64 {
    for (what, x) in [
        ("gravity", gravity.value()),
        ("fluid density", density_kg_m3),
        ("bed factor", bed),
    ] {
        assert!(
            x.is_finite() && x > 0.0,
            "a {what} is finite and positive, not {x}"
        );
    }
    EARTH_ERODIBILITY_M0_1_PER_YEAR
        * (gravity.value() / EARTH_GRAVITY.value())
        * (density_kg_m3 / REGIME_WATER_DENSITY.value())
        * bed
}

/// The erodibility of each crust of a world, for its fluid and surfaces: the one place the cells'
/// K and the bed lookup are made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Erodibility {
    /// The fluid, which only the erosion's tests read.
    #[cfg(test)]
    fluid: Fluid,
    /// Each crust's surface, which only the erosion's tests read.
    #[cfg(test)]
    surfaces: [Surface; 4],
    by_crust_m0_1_per_year: [f64; 4],
}

impl Erodibility {
    /// The erodibility of `inputs`' crusts, or `None` for a world never wet: for each [`Crust`]
    /// variant, K of its palette entry's mechanics on its [`Surface`] ([`crust_surfaces`], the
    /// world's, a province's by its own age) under the world's [`Fluid`]. A variant with no entry,
    /// which no cell carries, takes the lid's.
    ///
    /// # Panics
    ///
    /// Never for built inputs: the builder resolves each crust's entry in the palette.
    #[must_use]
    pub(crate) fn of(inputs: &CoarseInputs) -> Option<Self> {
        let fluid = Fluid::of(inputs)?;
        let surfaces = crust_surfaces(inputs);
        let entries = inputs.crust_palette();
        let by_crust_m0_1_per_year = core::array::from_fn(|k| {
            let entry = entries[k].unwrap_or(inputs.lid_entry());
            let mechanics = inputs
                .palette()
                .get(entry)
                .expect("a crust's entry is the palette's")
                .mechanics;
            let surface = surfaces[k];
            let bed = bed_factor(Substrate { mechanics, surface }, fluid.key);
            erodibility_m0_1_per_year(inputs.gravity(), fluid.density_kg_m3, bed)
        });
        Some(Self {
            #[cfg(test)]
            fluid,
            #[cfg(test)]
            surfaces,
            by_crust_m0_1_per_year,
        })
    }

    /// The fluid.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn fluid(&self) -> Fluid {
        self.fluid
    }

    /// The surface of crust `crust`'s ground, bedrock or a saturated regolith.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn surface(&self, crust: Crust) -> Surface {
        self.surfaces[usize::from(u8::from(crust))]
    }

    /// K on crust `crust`, m^0.1 a⁻¹.
    #[must_use]
    pub(crate) fn of_crust(&self, crust: Crust) -> f64 {
        self.by_crust_m0_1_per_year[usize::from(u8::from(crust))]
    }

    /// K of each of `cells`, by its crust, m^0.1 a⁻¹, in their order.
    #[must_use]
    pub(crate) fn per_cell(&self, cells: &[CellState]) -> Vec<f64> {
        cells.iter().map(|cell| self.of_crust(cell.crust)).collect()
    }
}
