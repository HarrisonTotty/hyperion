//! A solid body's crust by substance (plan 14, P14.T51.c; science-composition-crusts §1, §2, §5,
//! §7 and §9; decision-composition §1.2): its primary, secondary and tertiary crusts, or the
//! undifferentiated body's own `bulk` material, each a substance-registry row; which of them lies
//! at the surface (`lid`) and the lithology of its volcanic provinces; the mantle's redox class;
//! and the share of the surface above a silicate lid's solidus, a lava world's dayside pool
//! ([`CrustComposition::derive`]).
//!
//! A body with no solid surface, in the gas-envelope state, has no crust: P14.T54.a does not call
//! the derivation for it, which refuses it ([`DeriveCrustError::NoSolidSurface`]).
//!
//! # The thermal history ([`history`])
//!
//! A conductive sphere's peak central temperature `T_c,max`, heated by ²⁶Al and the long-lived
//! nuclides from the body's formation time `t_f` ([`formation_time`] draws it for every body
//! that is neither a planet nor a giant-impact moon), decides between a crust and `bulk`. The
//! classes are decided in this order ([`ThermalHistory`]):
//!
//! 1. **Rock melted, giving a crust**: a planet, whose magma ocean P14.T28.a times; a rocky
//!    parent's giant-impact moon, which accretes from melt-rich debris (the lunar magma ocean); a
//!    tidally volcanic body (Io); and any body whose `T_c,max` reaches the chondritic solidus at
//!    low pressure, `chondrite_h`'s transition, 1,423.15 K (1,150 °C: at IW−1 "the solidi are
//!    probably ~1150-1160°C" for H and LL chondrites, Jurewicz, Jones, Weber and Mittlefehldt
//!    1993, LPSC XXIV, 739, read).
//! 2. **Ice melted, rock not**: a body formed beyond the snow line holding ice whose `T_c,max`
//!    reaches the ice's melting point (`H2O`'s transition, its triple point, or `nh3_h2o`'s
//!    eutectic where ammonia is carried) plus its latent heat over its specific heat,
//!    L `f_ice` ÷ c; and an icy parent's giant-impact moon (Charon).
//!    - At [`OCEAN_SEPARATION_RADIUS`] or more, where its ocean froze, the ocean separated and
//!      froze into a primary crust ("Ice crusts", below).
//!    - Smaller, or where its ocean did not freeze, it altered in place, giving `bulk`:
//!      `cometary` where its surface keeps water ice (its warmest temperature below
//!      [`WATER_ICE_RETENTION_TEMPERATURE`]); otherwise `chondrite_cv` where `T_c,max` reached
//!      [`DEHYDRATION_TEMPERATURE`]; otherwise `chondrite_ci` where its ice-to-rock mass ratio is
//!      at least [`CI_ICE_TO_ROCK`]; otherwise `chondrite_cm`.
//! 3. **Nothing melted, giving `bulk`**: `cometary` beyond the snow line; inside it, by the
//!    redox class of its ΔIW (the bulk rock's, a body with no mantle): `chondrite_e` where
//!    `Reduced`; `chondrite_h`, `chondrite_l` or `chondrite_ll` by [`ordinary_chondrite`]'s edges
//!    where `Intermediate`; `chondrite_ll` where `Oxidised`.
//!
//! # The crusts of a body whose rock melted
//!
//! - **Primary**, where a magma ocean existed, a planet's or a rocky parent's impact moon's: on a
//!   `Reduced` body graphite flotation, `graphite` (Mercury's low-reflectance material: Peplowski
//!   et al. 2016); below [`FLOTATION_MASS_LIMIT`], where plagioclase floats, a flotation crust
//!   as basin-scale impacts mixed it, `feldspathic` (the Moon); and from it an ultramafic crust
//!   after the cumulates' overturn, `ultramafic` (Hu, Ehlmann and Seager 2012, after
//!   Elkins-Tanton et al. 2005). A frozen ocean's shell takes precedence where it exists. A body
//!   melted by its thermal history or its tides alone formed no flotation crust, and has none.
//! - **Secondary**, the solid mantle's partial melt, by the first rule that applies:
//!   `basalt_reduced` on a `Reduced` body; `komatiite` where the mantle is hot
//!   ([`hot_mantle`]: the rock's radiogenic production at the age at least
//!   [`KOMATIITE_PRODUCTION_RATIO`] times the bulk silicate Earth's present, or tidally
//!   volcanic); `basalt_fe` where its source is iron-rich, a magma ocean's cumulates under a
//!   plagioclase flotation crust (the lunar maria) or a mantle of at least
//!   [`IRON_RICH_MANTLE_FEO`] by mass balance ([`mantle_iron_oxide_mass_fraction`]: Mars, Vesta);
//!   `basalt` otherwise. A periclase-normative mantle ([`MantleNorm`]) shifts `basalt` one step
//!   towards Mg-rich, to `komatiite`, which reaches Putirka and Xu's (2021) periclase-bearing
//!   mantle rocks without a special case; a quartz-normative one keeps its row (not modelled,
//!   below). An iron-rich source keeps `basalt_fe`: where its norm is periclase-normative its
//!   excess is its `FeO`'s (a body of little or no core, whose mass balance leaves its iron in its
//!   mantle), and its melt is iron-rich, not Mg-rich.
//! - **Tertiary**: `granite` where the continental fraction is above zero, which already needs a
//!   mobile lid and surface water (Campbell and Taylor 1983: "no water, no granites").
//!
//! # Ice crusts
//!
//! The primary crust of a body whose ice melted and whose ocean froze, where its crust is
//! ice-rich by [`Crust::of`], the one ice-rich predicate the craters already read, so that the
//! two agree (T51.c's primary crust replaces the pure ice that predicate's target assumes):
//! `H2O` where ice is at least [`ICE_SHELL_FRACTION`] of its mass or its rock differentiated
//! beneath the ocean (Europa), and `phyllosilicate`, with its salts, where rock is the larger part
//! of its ocean's load (Ceres: "the bulk of Ceres's original ocean", Castillo-Rogez et al. 2018,
//! p. 1838; Fu et al. 2017). The crust names a substance and carries no mass: which part of a
//! frozen body's water is the crust's is P14.T53.c's (decision-p14-t51a-partition §2).
//!
//! # The lid and the provinces
//!
//! The lid is `Bulk` on an undifferentiated body; otherwise `Primary` where the body has a
//! primary crust and nothing has resurfaced it since it formed (its surface age at least the time
//! since its magma ocean's end, or its age where it had none, as P14.T24.b's surface age reads
//! it), and always for an ice crust, since an icy body's resurfacing remakes the same ice and its
//! secondary crust lies under its ocean; otherwise `Secondary`. Where the lid is a rocky
//! `Primary`, a resurfaced share shows as provinces of the secondary crust where the volcanism
//! level is above zero (the Moon's maria); otherwise the provinces are the lid's own substance.
//!
//! A `graphite` primary crust is never the lid where the body has a secondary crust, which every
//! body whose rock melted has: the lid is then `Secondary`, its provinces the lid's own, and the
//! graphite stays the buried `primary` (decision-pre-23-review item 3). A minor-phase flotation
//! layer is disrupted and buried by early volcanism and impacts. It survives as buried remnants,
//! the low-reflectance material's source, and, mixed into the volcanic crust, as the surface's
//! darkening agent, which the lid's row does not carry (P14.T49.k's). On Mercury the oldest
//! surfaces formed "about 4.0–4.1 Gyr ago", and the craters' agreement "implies that resurfacing
//! was global and was due to volcanism" (Marchi et al. 2013, Nature 499, 59, abstract). Peplowski
//! et al. propose that "the heavily disrupted remnants" of its graphite flotation crust persist
//! beneath the present upper crust, and find its low-reflectance material 1–3 wt% richer in carbon
//! than its surroundings (2016, Nat. Geosci. 9, 273, abstract). Secondary volcanism and impacts
//! that mixed the primary and secondary crusts would explain the dark, low-`FeO` surface; graphite
//! is the one mineral buoyant in its melts, and its `FeO`-poor melts erupt readily from a shallow
//! mantle (Vander Kaaden and McCubbin 2015, JGR Planets 120, 195, abstract). The rule is an
//! extrapolation from Mercury to every reduced body that had a magma ocean. It holds whatever the
//! surface age, which on the generated Mercury is its whole age since its magma ocean (its present
//! heat flow resurfaces nothing; the cause is P14.T24.b's, recorded for P14.T33.c's census).
//!
//! # The melt area
//!
//! The share of the surface above a silicate lid's solidus, its row's transition
//! (decision-p14-t49b-condensed §2): 1 − [`SurfaceConditions::cold_area`] at it, T24.a's one
//! closed form. Outside the `MagmaOcean` state an ice lid (`H2O`) takes 0: its liquid is the
//! condensates' water, not a melt of the crust, so a warm zone on an ice-shelled body claims no
//! silicate vapour on the wire (decision-pre-23-review item 10). A world
//! locked 1:1 to its host reads its eyeball, a dayside pool and a solid night, ½\[1 − (`T_sol`⁴ −
//! `T_n`⁴) ÷ (`T_ss`⁴ − `T_n`⁴)\] (Léger et al. 2011; Kite et al. 2016); a rotating one its zonal
//! profile. A body in the `MagmaOcean` state takes 1, so that "a surface holding melt" agrees
//! with the wire's `silicate_vapour` rule. Rendering plan R09.T13.c applies each cell's own
//! entry's solidus.
//!
//! # In stages
//!
//! The derivation runs in three stages, each a pure function, so that the passes can read the lid
//! at their own climate (P14.T54.a), and [`CrustComposition::derive`] is their composition, to the
//! bit:
//!
//! 1. **Once a body**, what no pass moves: the inputs that are neither the climate's nor the
//!    record's, checked; the redox class; the mantle's `FeO` and norm; and the thermal history's
//!    peak wherever the origin can need it, whatever the tide, so that a change of tide never
//!    re-runs the series.
//! 2. **At a climate**, a pass's: the state, the mean and warmest surface temperatures, the surface
//!    age and the tide give the thermal class, the crusts and the lid.
//! 3. **On the record**: the volcanism level, the continental fraction and the surface conditions
//!    give the tertiary crust, the provinces and the melt area.
//!
//! # The lithologies it names
//!
//! A closed set, each a registry row with condensed columns ([`CrustLithology::ALL`]): `H2O`,
//! `graphite`, `feldspathic`, `basalt`, `basalt_fe`, `komatiite`, `basalt_reduced`, `granite`,
//! `ultramafic`, `phyllosilicate`, `chondrite_ci`, `chondrite_cm`, `chondrite_cv`,
//! `chondrite_h`, `chondrite_l`, `chondrite_ll`, `chondrite_e`, `cometary`.
//!
//! # Not modelled
//!
//! A closed set:
//!
//! - **a silica-rich secondary crust**: a quartz-normative mantle's basaltic andesite or boninite
//!   has no registry row, so such a mantle keeps its unshifted row;
//! - **minor crust substances**: salts, NH₃ hydrate and clathrate, which the crust's members carry
//!   no list for;
//! - **an eruption temperature**: no rule states one, and the melt area reads starlight alone, so
//!   Io's internally heated lava gives it none.
//!
//! # On the record
//!
//! P14.T54.a puts the crust on the record (decision-bump-plan-after-22; decision-pre-23-review
//! item 17): a body derived over its record takes its basis once before its passes, at the first
//! pass's own equilibrium temperature, at the albedo it reads (0.3), held at the cosmic
//! background's 2.725 K for a body no host lights, the formation temperature's stand-in; each pass
//! takes its lid at its own climate, under the surface age and tide of the
//! context it reads, and reads that lid's row as its regolith; and the record's surface section
//! holds the crust of the published pass's lid (`CrustComposition::of`)
//! ([`record::Surface::crust`](crate::planetary::record::Surface::crust)), so that its lid is the
//! row its last pass read, to the bit. The regolith's thermal inertia at the lid is P14.T49.j's
//! law, which the next pass's ground reads. The tripwire that held the module off the record at
//! generator version 22 is deleted, and [`tags::BODY_FORMATION`]'s word 0 is first read on the
//! record, once a derivation.
//!
//! # Determinism
//!
//! Pure functions of their arguments and the substance registry, with one draw on its own tag,
//! [`tags::BODY_FORMATION`]; fixed term, time and step counts; every sum in a declared order;
//! every transcendental through the crate's [`math`].

pub mod history;

#[cfg(test)]
pub(crate) mod tests;

pub use crate::planetary::surface::LidCrust;
pub use history::{
    ChondriticRock, ConductiveSphere, HeatSource, chondritic_sources, peak_central_temperature,
};

use core::fmt;
use std::error::Error;

use crate::Seed;
use crate::id::BodyId;
use crate::math;
use crate::planetary::chem::redox::{RedoxClass, fmq_above_iw};
use crate::planetary::chem::solar_magnesium_to_silicon;
use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
use crate::planetary::derive::{MassFractions, SnowLineSide};
use crate::planetary::hooks::figures::interior::{
    ROCK_THERMAL_DIFFUSIVITY_M2_PER_S, RadiogenicIsotope, radiogenic_heat_production_w_per_kg,
};
use crate::planetary::hooks::figures::{Birthplace, Crust};
use crate::planetary::hooks::surface::SurfaceConditions;
use crate::rng::{ObjectKey, Stream, tags};
use crate::substance::{Element, PhaseDiagram, SubstanceId};
use crate::units::{Dex, EarthMasses, Kelvin, Kilograms, Megayears, Metres, Years};

/// A lithology the crust can name: a registry row with condensed columns ([module](self)'s closed
/// set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CrustLithology {
    /// Water ice, `H2O`: an icy body's frozen ocean's shell.
    Water,
    /// `graphite`: a reduced magma ocean's flotation crust (Mercury's).
    Graphite,
    /// `feldspathic`: a plagioclase flotation crust as impacts mixed it (the lunar highlands).
    Feldspathic,
    /// `basalt`: a mantle's partial melt (Earth's ocean floor, Venus).
    Basalt,
    /// `basalt_fe`: an iron-rich basalt (the lunar maria, Mars, Vesta).
    IronRichBasalt,
    /// `komatiite`: a hot or Mg-rich mantle's ultramafic lava (the Archean, Io).
    Komatiite,
    /// `basalt_reduced`: a reduced mantle's low-iron lava (Mercury's plains).
    ReducedBasalt,
    /// `granite`: the tertiary crust of a wet mobile lid (Earth's continents).
    Granite,
    /// `ultramafic`: an Earth- or Mars-mass magma ocean's crust after its cumulates overturn.
    Ultramafic,
    /// `phyllosilicate`: a frozen ocean's rock-laden shell (Ceres).
    Phyllosilicate,
    /// `chondrite_ci`: an icy body's rock altered in place at a high ice-to-rock ratio.
    ChondriteCi,
    /// `chondrite_cm`: an icy body's rock altered in place at a lower one.
    ChondriteCm,
    /// `chondrite_cv`: altered rock heated past its dehydration.
    ChondriteCv,
    /// `chondrite_h`: the most reduced ordinary chondrite.
    ChondriteH,
    /// `chondrite_l`: the ordinary chondrite between H and LL.
    ChondriteL,
    /// `chondrite_ll`: the least reduced ordinary chondrite.
    ChondriteLl,
    /// `chondrite_e`: an enstatite chondrite, below IW−3.
    ChondriteE,
    /// `cometary`: an icy body's never-melted ice and dust.
    Cometary,
}

impl CrustLithology {
    /// Every lithology, in registry order.
    pub const ALL: [Self; 18] = [
        Self::Water,
        Self::Graphite,
        Self::Feldspathic,
        Self::Basalt,
        Self::IronRichBasalt,
        Self::Komatiite,
        Self::ReducedBasalt,
        Self::Granite,
        Self::Ultramafic,
        Self::Phyllosilicate,
        Self::ChondriteCi,
        Self::ChondriteCm,
        Self::ChondriteCv,
        Self::ChondriteH,
        Self::ChondriteL,
        Self::ChondriteLl,
        Self::ChondriteE,
        Self::Cometary,
    ];

    /// Its registry key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Water => "H2O",
            Self::Graphite => "graphite",
            Self::Feldspathic => "feldspathic",
            Self::Basalt => "basalt",
            Self::IronRichBasalt => "basalt_fe",
            Self::Komatiite => "komatiite",
            Self::ReducedBasalt => "basalt_reduced",
            Self::Granite => "granite",
            Self::Ultramafic => "ultramafic",
            Self::Phyllosilicate => "phyllosilicate",
            Self::ChondriteCi => "chondrite_ci",
            Self::ChondriteCm => "chondrite_cm",
            Self::ChondriteCv => "chondrite_cv",
            Self::ChondriteH => "chondrite_h",
            Self::ChondriteL => "chondrite_l",
            Self::ChondriteLl => "chondrite_ll",
            Self::ChondriteE => "chondrite_e",
            Self::Cometary => "cometary",
        }
    }

    /// Its row's index, append-only and never renumbered, so that no lookup by key is needed; the
    /// module's tests hold each against its key.
    #[must_use]
    const fn index(self) -> u16 {
        match self {
            Self::Water => 2,
            Self::Graphite => 73,
            Self::Feldspathic => 87,
            Self::Basalt => 88,
            Self::IronRichBasalt => 89,
            Self::Komatiite => 90,
            Self::ReducedBasalt => 91,
            Self::Granite => 92,
            Self::Ultramafic => 93,
            Self::Phyllosilicate => 95,
            Self::ChondriteCi => 104,
            Self::ChondriteCm => 105,
            Self::ChondriteCv => 106,
            Self::ChondriteH => 107,
            Self::ChondriteL => 108,
            Self::ChondriteLl => 109,
            Self::ChondriteE => 110,
            Self::Cometary => 111,
        }
    }

    /// Its registry row.
    ///
    /// # Panics
    ///
    /// Never: every index is a row of the registry, as the module's tests hold.
    #[must_use]
    pub fn substance(self) -> SubstanceId {
        SubstanceId::from_index(self.index()).expect(
            "every lithology's index is a registry row, as \
             every_lithology_named_is_a_registry_row_with_condensed_columns holds",
        )
    }

    /// Its first melt, its row's transition (decision-p14-t49b-condensed §2).
    #[must_use]
    pub fn transition(self) -> Kelvin {
        first_melt(self.substance())
    }

    /// The lithology whose registry key is `key`; `None` for a key outside the closed set.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::crust::CrustLithology;
    ///
    /// assert_eq!(CrustLithology::of_key("basalt_fe"), Some(CrustLithology::IronRichBasalt));
    /// assert_eq!(CrustLithology::of_key("mars_dust"), None);
    /// ```
    #[must_use]
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|lithology| lithology.key() == key)
    }

    /// Whether it is an ice, whose liquid is a condensate and not a melt of the crust
    /// ([module](self)'s "The melt area"): `H2O` alone; every other lithology's transition is a
    /// silicate's solidus, or graphite's sublimation.
    #[must_use]
    const fn is_ice(self) -> bool {
        match self {
            Self::Water => true,
            Self::Graphite
            | Self::Feldspathic
            | Self::Basalt
            | Self::IronRichBasalt
            | Self::Komatiite
            | Self::ReducedBasalt
            | Self::Granite
            | Self::Ultramafic
            | Self::Phyllosilicate
            | Self::ChondriteCi
            | Self::ChondriteCm
            | Self::ChondriteCv
            | Self::ChondriteH
            | Self::ChondriteL
            | Self::ChondriteLl
            | Self::ChondriteE
            | Self::Cometary => false,
        }
    }
}

/// What the crust does not model, a closed set ([module](self)'s "Not modelled"), each with its
/// reason there.
pub const NOT_MODELLED: [&str; 3] = [
    "a silica-rich secondary crust",
    "minor crust substances",
    "an eruption temperature",
];

/// The index of `nh3_h2o`, the ammonia–water solution, whose eutectic is an ammoniated ice's
/// melting point; the module's tests hold it against its key.
const AMMONIA_WATER_INDEX: u16 = 84;

/// The first melt of the row `id`, its condensed columns' transition.
///
/// # Panics
///
/// For a row with no condensed columns, which no caller here names: the module's tests hold every
/// lithology and `nh3_h2o` to have them.
#[must_use]
fn first_melt(id: SubstanceId) -> Kelvin {
    id.substance()
        .condensed()
        .expect("every row the crust reads has condensed columns, as its tests hold")
        .transition()
        .temperature()
}

/// The ice's melting point: `H2O`'s triple point, or `nh3_h2o`'s eutectic where ammonia is
/// carried.
///
/// # Panics
///
/// Never: `nh3_h2o`'s index is a registry row with condensed columns, as the module's tests
/// hold.
#[must_use]
pub fn ice_melting_point(ammonia: AmmoniaIce) -> Kelvin {
    match ammonia {
        AmmoniaIce::Absent => CrustLithology::Water.transition(),
        AmmoniaIce::Carried => first_melt(
            SubstanceId::from_index(AMMONIA_WATER_INDEX)
                .expect("nh3_h2o is a registry row, as the crust's tests hold"),
        ),
    }
}

/// Water ice's latent heat of fusion, J kg⁻¹: `H2O`'s enthalpy of fusion at its triple point over
/// its molar mass (6,007.099 J mol⁻¹ over 18.015 g mol⁻¹, 333.4 kJ kg⁻¹, as IAPWS R10-06 gives it;
/// the module's tests hold the two within 0.1%).
///
/// # Panics
///
/// Never: water's row has a phase diagram with a measured enthalpy of fusion and a molar mass, as
/// the module's tests hold.
#[must_use]
pub fn ice_latent_heat_j_per_kg() -> f64 {
    let water = Gas::Water.substance().substance();
    let fusion = water
        .phase_diagram()
        .and_then(PhaseDiagram::triple_fusion_enthalpy_j_per_mol)
        .expect("water's diagram has its enthalpy of fusion, as the crust's tests hold");
    let molar = water
        .molar_mass_g_per_mol()
        .expect("water has a molar mass");
    fusion / (molar * 1e-3)
}

/// How a body came to be, as its crust's thermal history reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CrustOrigin {
    /// A planet, whose formation and magma ocean are P14.T28.a's.
    Planet,
    /// A giant's regular moon (P14.T17).
    RegularMoon,
    /// A giant-impact moon (P14.T18), on a parent of the given surface.
    ImpactMoon(ParentSurface),
    /// A captured moon (P14.T19).
    CapturedMoon,
    /// A belt's member, a dwarf planet included (P14.T21).
    BeltMember,
    /// A comet (P14.T21.d).
    Comet,
}

/// Which surface a giant-impact moon's parent has, as the debris it accreted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ParentSurface {
    /// A rocky parent: melt-rich debris, a magma ocean (the Moon).
    Rocky,
    /// An icy parent: icy debris, which melts its ice (Charon).
    Icy,
}

/// Whether a body's ice carries ammonia, which lowers its first melt to the NH₃–H₂O eutectic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AmmoniaIce {
    /// Ammonia was carried: P14.T50.c's `nitrogen_boosted` holds at the body's formation
    /// temperature.
    Carried,
    /// No ammonia.
    Absent,
}

/// Whether a body's tides make it volcanic (P14.T56.d's tidal branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TidalVolcanism {
    /// Volcanic: a moon on P14.T56.d's heated branch (Io).
    Volcanic,
    /// Quiet: every other body, an ocean branch's or a cold branch's moon included.
    Quiet,
}

/// Why a body's rock melted ([`ThermalHistory::RockMelted`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MeltCause {
    /// A planet's magma ocean (P14.T28.a).
    MagmaOcean,
    /// A rocky parent's giant-impact debris, melt-rich (the lunar magma ocean).
    ImpactDebris,
    /// Its tides: a tidally volcanic body.
    Tides,
    /// Its radiogenic heat: its peak central temperature reached the threshold.
    Radiogenic,
}

/// Why a body's ice melted where its rock did not ([`ThermalHistory::IceMelted`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IceMeltCause {
    /// An icy parent's giant-impact debris (Charon).
    ImpactDebris,
    /// Its radiogenic heat: its peak central temperature reached its ice's threshold.
    Radiogenic,
}

/// What a body's thermal history melted ([module](self)'s three classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ThermalHistory {
    /// Its rock melted and it differentiated: it has crusts.
    RockMelted(MeltCause),
    /// Its ice melted and its rock did not.
    IceMelted(IceMeltCause),
    /// Nothing melted: it has its own `bulk` material.
    Unmelted,
}

/// The radius from which a body whose ice melted lets its ocean separate and freeze into a
/// primary crust: 200 km (science-composition-crusts §1.4's lean, `R_ocean`; tested at build).
/// Smaller, its ice altered its rock in place.
pub const OCEAN_SEPARATION_RADIUS: Metres = Metres::new(200e3);

/// The ice share of a body's mass from which its frozen ocean's shell is `H2O` rather than
/// `phyllosilicate`: 0.3 (science-composition-crusts §1.4's lean; tested at build). Ceres, a
/// quarter water, keeps a rock-laden shell.
pub const ICE_SHELL_FRACTION: f64 = 0.3;

/// The peak central temperature at which an aqueously altered body's rock dehydrates, giving
/// `chondrite_cv`, the registry's metamorphosed anhydrous carbonaceous chondrite: 900 K
/// (science-composition-crusts §9's lean).
///
/// Heated CM and CI chondrites lose their phyllosilicates' water in stages, "Stage I (< 300 °C),
/// Stage II (300 − 500 °C), Stage III (500 − 750 °C), and Stage IV (> 750 °C)", Stage IV fully
/// anhydrous, and "These temperatures decrease by approximately 100 °C with longer heating
/// periods" (Nakamura 2005, J. Mineral. Petrol. Sci. 100, 260, pp. 264–265, read), so a
/// radiogenic history's full dehydration starts near 650 °C, 923 K. Those heated chondrites stay
/// their own kind; the registry has no row for them, so `chondrite_cv` stands in, stated.
pub const DEHYDRATION_TEMPERATURE: Kelvin = Kelvin::new(900.0);

/// The ice-to-rock mass ratio from which a body's rock altered in place is `chondrite_ci` rather
/// than `chondrite_cm`: 0.4, where the reacted water-to-rock mass ratios part, "CM = 0.10–0.40,
/// and CI = 0.38–0.43 (Alexander 2019 …)" (Lee et al. 2025, Space Sci. Rev. 221, 11, read).
///
/// It replaces the lean's 0.5, under which every CI value lies (science-composition-crusts §9;
/// Clayton and Mayeda 1999, GCA 63, 2089, not read). The ratios are of the water that reacted,
/// which the ice accreted bounds from above.
pub const CI_ICE_TO_ROCK: f64 = 0.4;

/// The warmest surface temperature below which a small icy body keeps its water ice at the
/// surface, its frozen, never-melted lid: 110 K, at which hexagonal ice sublimes into vacuum at
/// 144 kg m⁻² Gyr⁻¹, 0.15 m per Gyr (Schorghofer and Williams 2020, PSJ 1, 54, Table 1 and its
/// Hertz–Knudsen eq. 8, read): Bland and Travis's (2017) "undifferentiated frozen crust" survives
/// there.
///
/// It is this module's constant until P14.T51.f computes each species' threshold from its
/// saturation curve at 24. It is compared with [`SurfaceConditions::climate_range`]'s warmest,
/// which includes the dayside's excess and so bounds a zone's annual maximum from above.
pub const WATER_ICE_RETENTION_TEMPERATURE: Kelvin = Kelvin::new(110.0);

/// The mass below which a magma ocean's plagioclase floats into a flotation crust,
/// `feldspathic`, and from which its crust is `ultramafic` after the cumulates overturn:
/// 0.0363 M⊕, the geometric mean of the Moon's 0.0123 and Mars's 0.107 (the lane's, stated;
/// science-composition-crusts §1's lean).
///
/// No numeric edge was found. Hu, Ehlmann and Seager (2012, ApJ 752, 7, §3.1, read) give the two
/// ends: "feldspathic surfaces, such as the lunar highlands, are primitive products of
/// crystallization from a magma ocean", while "For a relatively large planet with mass similar
/// to that of Earth and Mars, the predicted crust composition after the overturn … is dominated
/// by Mg-rich olivines and pyroxenes, i.e., an ultramafic surface (Elkins-Tanton et al., 2005)";
/// on the Moon "Above 70-80% solidification, plagioclase begins to crystallize" (Schaefer and
/// Elkins-Tanton 2018, Phil. Trans. R. Soc. A 376, 20180109, read).
pub const FLOTATION_MASS_LIMIT: EarthMasses = EarthMasses::new(0.036_3);

/// How many times the bulk silicate Earth's present radiogenic production a mantle's rock must
/// make at the age to be hot enough for komatiite: 2, the Neoarchaean's, whose "rate of
/// radiogenic heat production was on average twice the present-day rate" (Reimink and Smye 2024,
/// Nature 629, 609, read), at 2.6 Ga by the crate's isotopes (1.97 Gyr after an Earth's
/// formation). Komatiites, ultramafic lavas of at least 18 wt% `MgO` (Le Bas 2000, J. Petrol. 41,
/// 1467, abstract read), grow rare after it: the Al-depleted ones occur only from 4 to 2.8 Ga, the
/// Al-undepleted until about 1.9 Ga (Pierru et al. 2022, Sci. Adv. 8, eabo1036, read). Arndt,
/// Lesher and Barnes 2008 and Herzberg, Condie and Korenaga 2010 were not read.
pub const KOMATIITE_PRODUCTION_RATIO: f64 = 2.0;

/// The mantle `FeO`, by mass, from which a basalt's source is iron-rich, `basalt_fe`: 12 wt%, the
/// geometric mean of the bulk silicate Earth's 8.05 wt% (McDonough and Sun 1995, Chem. Geol. 120,
/// 223, as Solomatova and Caracas 2019, JGR Solid Earth 124, 11232, Table 1, quote it, read) and
/// Mars's mantle's 17.9 (Dreibus and Wänke 1985, Meteoritics 20, 367, Table 1, read).
///
/// After InSight Mars's mantle is put at 13.8 ± 1.0 wt% (Khan et al. 2022 and 2023, as Sossi et
/// al. 2025, Space Sci. Rev. 221, 118, Table 1, give it, read), still above the edge.
pub const IRON_RICH_MANTLE_FEO: f64 = 0.12;

/// The iron fraction of a core's mass: 0.85, the rest nickel and light elements: McDonough and
/// Arevalo 2008's 85 wt% and Javoy et al. 2010's 85.5 ± 1.1, against a concordance mean of
/// 82.8 ± 2.9 (Wang, Lineweaver and Ireland 2018, Icarus 299, 460, Table 2, read).
pub const CORE_IRON_FRACTION: f64 = 0.85;

/// The bulk silicate Earth's Mg/Si by number: 1.252, from its `MgO` 37.8 and `SiO₂` 45.0 wt%
/// (McDonough and Sun 1995, as Solomatova and Caracas 2019 quote them, read) at the standard
/// atomic weights, against the Sun's 1.023 (Lodders 2010): the calibrator of a mantle's Mg/Si
/// from its host's ([`mantle_magnesium_to_silicon`]).
pub const BULK_SILICATE_EARTH_MAGNESIUM_TO_SILICON: f64 = 1.252;

/// The molar (Mg + Fe) ÷ Si of a mantle below which it is quartz-normative, and above which it is
/// periclase-normative: 1 and 2, the cation ratios of pyroxene and olivine ([`MantleNorm`]).
///
/// Putirka and Xu (2021, Nat. Commun. 12, 6168, read) class their exotic mantles by a normative
/// mineralogy at 2 GPa and 1,350 °C, quartz-saturated with no olivine or periclase-saturated with
/// no orthopyroxene, and state no Mg/Si edge; these are the lane's reading of their norm.
pub const NORMATIVE_EDGES: (f64, f64) = (1.0, 2.0);

/// The ordinary chondrites' oxygen fugacities relative to FMQ, log₁₀ units, H, L and LL: −6.64,
/// −6.02 and −5.45, the centres of their groups read off Righter and Neff's (2007, Polar Sci. 1,
/// 25, Fig. 16B, after Righter et al. 2006; the NTRS manuscript, read at 300 dpi, ±0.2), each
/// "compared to the values of a common buffer such as fayalite-magnetite-quartz (FMQ) at the
/// same temperature" (their §4.2).
///
/// Other methods spread by 1–2 log units: Schaefer and Fegley (2007, Icarus 186, 462, §3.1 and
/// Fig. 8a, read) put average H material "slightly below the QFI buffer", L "directly on" and LL
/// "slightly above". Brett and Sato 1984 and McSween and Labotka 1993 were not read.
pub const ORDINARY_CHONDRITE_DELTA_FMQ: [Dex; 3] =
    [Dex::new(-6.64), Dex::new(-6.02), Dex::new(-5.45)];

/// ΔIW, log₁₀ units, below which an ordinary chondrite is H and from which it is LL, the L
/// chondrites between: the midpoints of the groups' centres ([`ORDINARY_CHONDRITE_DELTA_FMQ`])
/// moved to IW on the crate's own scale, by [`fmq_above_iw`]'s 3.593 at 1,400 K and 1 bar, on which
/// [`RedoxClass`]'s edges stand: H −3.05, L −2.43 and LL −1.86, so −2.74 and −2.14.
///
/// They lie within the `Intermediate` band's IW−3 to IW+1.59, below IW, since the ordinary
/// chondrites carry Fe–Ni metal (P14.T50.a's deferred finding); the H centre falls at the
/// `Reduced` edge, and the band's H part is its upper half. At the chondrites' own metamorphic
/// temperatures the offset is larger (4.06 at 850 °C by the same buffers), which would move the
/// edges up by about 0.5, within the methods' spread.
#[must_use]
pub fn ordinary_chondrite_edges() -> (Dex, Dex) {
    let [h, l, ll] = ORDINARY_CHONDRITE_DELTA_FMQ.map(|delta_fmq| delta_fmq + fmq_above_iw());
    (
        Dex::new(f64::midpoint(h.value(), l.value())),
        Dex::new(f64::midpoint(l.value(), ll.value())),
    )
}

/// The ordinary chondrite of an `Intermediate` body's bulk rock at `delta_iw`
/// ([`ordinary_chondrite_edges`]): H, L or LL from reduced to oxidised.
#[must_use]
pub fn ordinary_chondrite(delta_iw: Dex) -> CrustLithology {
    let (h_below, ll_from) = ordinary_chondrite_edges();
    if delta_iw.value() < h_below.value() {
        CrustLithology::ChondriteH
    } else if delta_iw.value() < ll_from.value() {
        CrustLithology::ChondriteL
    } else {
        CrustLithology::ChondriteLl
    }
}

/// The bulk iron mass fraction `w_Fe` of a body's solids, iron and rock, about a host of Mg/Si
/// `host_magnesium_to_silicon` and \[α/Fe\] `alpha_fe`, by a stoichiometric mass balance: per
/// silicon atom, r = Mg/Si magnesium as `MgO`, one silicon as `SiO₂` and n = F (1 + r) iron as Fe,
/// F = Fe/(Mg + Si) the protosolar ratio (Lodders 2010) times 10^(−\[α/Fe\]), since no host
/// Fe/(Mg + Si) is drawn (P14.T50.a): `w_Fe` = n `M_Fe` ÷ (r `M_MgO` + `M_SiO₂` + n `M_Fe`).
///
/// The solids are counted as these three alone, with every iron as metal, so that the bulk Earth's
/// 32 wt% (31.4 at the Sun's ratios) is its iron's whether in its core or its mantle.
#[must_use]
pub fn bulk_iron_mass_fraction(host_magnesium_to_silicon: f64, alpha_fe: Dex) -> f64 {
    let a = |e: Element| e.protosolar_log_abundance().value();
    let protosolar =
        math::exp10(a(Element::Fe)) / (math::exp10(a(Element::Mg)) + math::exp10(a(Element::Si)));
    let iron_per_magnesium_and_silicon = protosolar * math::exp10(-alpha_fe.value());
    let r = host_magnesium_to_silicon;
    let n = iron_per_magnesium_and_silicon * (1.0 + r);
    let (m_fe, m_mgo, m_sio2) = oxide_weights();
    n * m_fe / (r * m_mgo + m_sio2 + n * m_fe)
}

/// The molar masses of Fe, `MgO` and `SiO₂`, g mol⁻¹, from the element registry's standard atomic
/// weights.
#[must_use]
fn oxide_weights() -> (f64, f64, f64) {
    let w = Element::standard_atomic_weight_g_per_mol;
    let oxygen = w(Element::O);
    (
        w(Element::Fe),
        w(Element::Mg) + oxygen,
        w(Element::Si) + 2.0 * oxygen,
    )
}

/// `FeO`'s mass over its iron's, 1.2865: (`M_Fe` + `M_O`) ÷ `M_Fe` from the standard atomic weights.
#[must_use]
fn iron_oxide_per_iron() -> f64 {
    let w = Element::standard_atomic_weight_g_per_mol;
    (w(Element::Fe) + w(Element::O)) / w(Element::Fe)
}

/// The mantle's `FeO` mass fraction, 0 to 1, of a body whose core is `core_share` of its solids
/// (`f_core` = iron ÷ (iron + rock), in [0, 1)), about a host of Mg/Si
/// `host_magnesium_to_silicon` and \[α/Fe\] `alpha_fe`, by mass balance:
/// 1.2865 × max(0, `w_Fe` − `f_core` × [`CORE_IRON_FRACTION`]) ÷ (1 − `f_core`), with `w_Fe` the
/// bulk iron fraction ([`bulk_iron_mass_fraction`]); 0 for a core share of 1 or more, or NaN,
/// a body of no mantle.
///
/// # Examples
///
/// An Earth's core leaves its mantle under the iron-rich edge, and a Mars's smaller core leaves
/// its mantle above it:
///
/// ```
/// use hyperion_sim::planetary::chem::solar_magnesium_to_silicon;
/// use hyperion_sim::planetary::hooks::crust::{IRON_RICH_MANTLE_FEO, mantle_iron_oxide_mass_fraction};
/// use hyperion_sim::units::Dex;
///
/// let feo = |core: f64| mantle_iron_oxide_mass_fraction(core, solar_magnesium_to_silicon(), Dex::new(0.0));
/// assert!(feo(0.325) < IRON_RICH_MANTLE_FEO && feo(0.24) > IRON_RICH_MANTLE_FEO);
/// ```
#[must_use]
pub fn mantle_iron_oxide_mass_fraction(
    core_share: f64,
    host_magnesium_to_silicon: f64,
    alpha_fe: Dex,
) -> f64 {
    if core_share.is_nan() || core_share >= 1.0 {
        return 0.0;
    }
    let bulk = bulk_iron_mass_fraction(host_magnesium_to_silicon, alpha_fe);
    let mantle_iron = bulk - core_share * CORE_IRON_FRACTION;
    let mantle_iron = if mantle_iron > 0.0 { mantle_iron } else { 0.0 };
    iron_oxide_per_iron() * mantle_iron / (1.0 - core_share)
}

/// The core's share of a body's solids, iron ÷ (iron + rock); 0 for a body with neither.
#[must_use]
fn core_share(fractions: &MassFractions) -> f64 {
    let solids = fractions.iron() + fractions.rock();
    if solids > 0.0 {
        fractions.iron() / solids
    } else {
        0.0
    }
}

/// A mantle's Mg/Si by number, from its host's: the host's times the bulk silicate Earth's over
/// the Sun's ([`BULK_SILICATE_EARTH_MAGNESIUM_TO_SILICON`] over Lodders's 1.023), the silicon
/// a core takes and the volatility the Earth's mantle shows (the lane's relation, stated).
#[must_use]
pub fn mantle_magnesium_to_silicon(host_magnesium_to_silicon: f64) -> f64 {
    host_magnesium_to_silicon * BULK_SILICATE_EARTH_MAGNESIUM_TO_SILICON
        / solar_magnesium_to_silicon()
}

/// A mantle's normative mineralogy, by its molar (Mg + Fe) ÷ Si ([`NORMATIVE_EDGES`]; Putirka and
/// Xu 2021, Nat. Commun. 12, 6168).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MantleNorm {
    /// Below 1: free silica in its norm, as Putirka and Xu's quartz pyroxenites.
    QuartzNormative,
    /// 1 to 2: olivine and pyroxene, as the Earth's.
    OlivinePyroxene,
    /// Above 2: free magnesia, as their periclase dunites.
    PericlaseNormative,
}

impl MantleNorm {
    /// The norm of a mantle of `FeO` mass fraction `iron_oxide`, in [0, 1), about a host of Mg/Si
    /// `host_magnesium_to_silicon`: its Mg/Si ([`mantle_magnesium_to_silicon`]) plus its Fe/Si,
    /// n = x (r `M_MgO` + `M_SiO₂`) ÷ (`M_FeO` (1 − x)) for `FeO` x of a mantle of `MgO`, `SiO₂` and `FeO`
    /// alone.
    #[must_use]
    pub fn of(host_magnesium_to_silicon: f64, iron_oxide: f64) -> Self {
        let cations = mantle_cations_per_silicon(host_magnesium_to_silicon, iron_oxide);
        let (quartz_below, periclase_above) = NORMATIVE_EDGES;
        if cations < quartz_below {
            Self::QuartzNormative
        } else if cations > periclase_above {
            Self::PericlaseNormative
        } else {
            Self::OlivinePyroxene
        }
    }
}

/// A mantle's molar (Mg + Fe) ÷ Si ([`MantleNorm::of`]), for an `FeO` mass fraction
/// `iron_oxide` in [0, 1); infinite at 1, a mantle of iron oxide alone.
#[must_use]
pub fn mantle_cations_per_silicon(host_magnesium_to_silicon: f64, iron_oxide: f64) -> f64 {
    let r = mantle_magnesium_to_silicon(host_magnesium_to_silicon);
    let (m_fe, m_mgo, m_sio2) = oxide_weights();
    let m_feo = m_fe + Element::O.standard_atomic_weight_g_per_mol();
    let x = iron_oxide;
    let iron = x * (r * m_mgo + m_sio2) / (m_feo * (1.0 - x));
    r + iron
}

/// Whether a mantle is hot enough for komatiite: its rock's radiogenic production at the age `age`
/// about a host of \[Fe/H\] `fe_h` ([`radiogenic_heat_production_w_per_kg`], summed) at least
/// [`KOMATIITE_PRODUCTION_RATIO`] times the bulk silicate Earth's present, or tidally volcanic.
#[must_use]
pub fn hot_mantle(fe_h: Dex, age: Years, tidal: TidalVolcanism) -> bool {
    let sum = |p: [f64; 4]| p.iter().fold(0.0, |sum, &x| sum + x);
    let present = RadiogenicIsotope::ALL
        .iter()
        .fold(0.0, |sum, i| sum + i.present_heat_production_w_per_kg());
    let production = sum(radiogenic_heat_production_w_per_kg(fe_h, age));
    production >= KOMATIITE_PRODUCTION_RATIO * present || tidal == TidalVolcanism::Volcanic
}

/// Words of [`tags::BODY_FORMATION`] a body reads or reserves: word 0, its rank in its window, then
/// seven reserved.
pub const FORMATION_WORDS: u64 = 8;

/// The span of host ages, Myr after the host's onset of collapse, in which a body that is neither
/// a planet nor a giant-impact moon formed ([`formation_time`]).
///
/// - A regular moon: from its giant's formation (P14.T28.a's `formed_at`) to the disc's lifetime
///   L.
/// - A belt's member, a captured moon or a comet: from 0 to L ([`FormationWindow::disc`]).
/// - A giant-impact moon needs none: its class is its parent's debris's, and it takes its parent's
///   formation time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationWindow {
    earliest: Megayears,
    latest: Megayears,
}

/// A [`FormationWindow`] could not be built from the ages given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildFormationWindowError {
    /// An end was not finite, or was negative.
    NotFiniteOrNegative(Megayears),
    /// The earliest age was after the latest.
    Reversed {
        /// The earliest age given.
        earliest: Megayears,
        /// The latest age given.
        latest: Megayears,
    },
}

impl fmt::Display for BuildFormationWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFiniteOrNegative(age) => {
                write!(
                    f,
                    "formation age {} Myr is not finite and non-negative",
                    age.value()
                )
            }
            Self::Reversed { earliest, latest } => write!(
                f,
                "earliest formation age {} Myr is after the latest, {} Myr",
                earliest.value(),
                latest.value()
            ),
        }
    }
}

impl Error for BuildFormationWindowError {}

impl FormationWindow {
    /// The window from `earliest` to `latest`.
    ///
    /// # Errors
    ///
    /// [`BuildFormationWindowError::NotFiniteOrNegative`] for an end that is not finite and
    /// non-negative, and [`BuildFormationWindowError::Reversed`] for `earliest` after `latest`.
    pub fn new(earliest: Megayears, latest: Megayears) -> Result<Self, BuildFormationWindowError> {
        for age in [earliest, latest] {
            if !(age.value().is_finite() && age.value() >= 0.0) {
                return Err(BuildFormationWindowError::NotFiniteOrNegative(age));
            }
        }
        if earliest.value() > latest.value() {
            return Err(BuildFormationWindowError::Reversed { earliest, latest });
        }
        Ok(Self { earliest, latest })
    }

    /// The window of a disc that lives `lifetime`, from the host's onset of collapse to its end:
    /// a belt member's, a captured moon's or a comet's.
    ///
    /// # Errors
    ///
    /// As [`FormationWindow::new`], for a lifetime that is not finite and non-negative.
    pub fn disc(lifetime: Megayears) -> Result<Self, BuildFormationWindowError> {
        Self::new(Megayears::new(0.0), lifetime)
    }

    /// The earliest age.
    #[must_use]
    pub const fn earliest(&self) -> Megayears {
        self.earliest
    }

    /// The latest age.
    #[must_use]
    pub const fn latest(&self) -> Megayears {
        self.latest
    }
}

/// The formation time `t_f` of the body `body` in the universe of `seed`, uniform in `window`:
/// word 0 of its [`tags::BODY_FORMATION`] stream, one open uniform u, at `earliest` + u (`latest`
/// − `earliest`), held to the window.
///
/// The tag is new, so no existing value moves (P14.T51.c's determinism;
/// decision-bump-plan-after-22).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::hooks::crust::{FormationWindow, formation_time};
/// use hyperion_sim::units::Megayears;
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000).expect("a valid system ID");
/// let window = FormationWindow::disc(Megayears::new(3.0))?;
/// let t = formation_time(Seed::new(7), BodyId::new(system, 0x0100), window);
/// assert!((0.0..=3.0).contains(&t.value()));
/// # Ok::<(), hyperion_sim::planetary::hooks::crust::BuildFormationWindowError>(())
/// ```
#[must_use]
pub fn formation_time(seed: Seed, body: BodyId, window: FormationWindow) -> Megayears {
    let mut stream = Stream::open(seed, tags::BODY_FORMATION, ObjectKey::from(body));
    let u = stream.uniform_open();
    let (low, high) = (window.earliest.value(), window.latest.value());
    let t = low + u * (high - low);
    Megayears::new(if t > high { high } else { t })
}

/// What [`CrustComposition::derive`] reads of a body, as plain values, each with its source on
/// the record (which P14.T54.a wires).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrustCompositionInputs<'a> {
    /// How it came to be.
    pub origin: CrustOrigin,
    /// Its formation time `t_f`, a host age, Myr after the host's onset of collapse (the
    /// inclusions' time): a planet's P14.T28.a's `Formation::formed_at`, a giant-impact moon's its
    /// parent's, and every other body's [`formation_time`]'s draw. In [0, the age].
    pub formation: Megayears,
    /// Its system's age at the time, positive.
    pub age: Years,
    /// The system age at which its magma ocean ended, P14.T28.a's `Formation::molten_until`, a
    /// planet's only; `None` for every other body, which takes its age, as P14.T24.b's surface
    /// age reads it.
    pub molten_until: Option<Years>,
    /// Its mean radius, the bulk section's, positive.
    pub radius: Metres,
    /// Its mass, the bulk section's, positive.
    pub mass: EarthMasses,
    /// Its mass fractions, the bulk section's (P14.T11), summing to 1: its core is its iron, its
    /// ice its water.
    pub fractions: MassFractions,
    /// The side of the snow line it formed on: a host orbiter's its own, a moon's its parent's ice
    /// line's (P14.T48.f's `MoonBirth::formed`).
    pub snow_line_side: SnowLineSide,
    /// Whether its ice carries ammonia.
    pub ammonia: AmmoniaIce,
    /// Its mantle's ΔIW, log₁₀ units (P14.T50.a's `ElementInventory::mantle_delta_iw`; a body
    /// with no mantle's bulk rock's).
    pub delta_iw: Dex,
    /// Its host's Mg/Si by number (P14.T50.a's `HostAbundances::magnesium_to_silicon`), positive.
    pub host_magnesium_to_silicon: f64,
    /// Its system's \[α/Fe\], dex (P14.T23's `BulkComposition::alpha_fe`, 0 for none), for the
    /// host's Fe/(Mg + Si).
    pub alpha_fe: Dex,
    /// Its host's \[Fe/H\], dex, which scales its long-lived nuclides.
    pub fe_h: Dex,
    /// Its surface age, P14.T24.b's `GlobalFigures::surface_age`, in [0, the age].
    pub surface_age: Years,
    /// Its volcanism level V, P14.T48.a's `GlobalFigures::volcanism`, at least 0.
    pub volcanism: f64,
    /// Its continental fraction `f_c`, P14.T48.a's `GlobalFigures::continental_fraction`, 0 to 1.
    pub continental_fraction: f64,
    /// Whether its tides make it volcanic: P14.T56.d's heated branch, which T54.a maps to
    /// [`TidalVolcanism::Volcanic`], and every other branch to [`TidalVolcanism::Quiet`].
    pub tidal: TidalVolcanism,
    /// Its surface state (P14.T13.c), which is not the gas-envelope state.
    pub state: SurfaceState,
    /// The surface temperature `T_s` of its thermal history, positive: the lean is its present
    /// equilibrium temperature (P14.T12).
    pub formation_temperature: Kelvin,
    /// Its surface conditions (P14.T24.a), for the melt area and the warmest temperature.
    pub surface: &'a SurfaceConditions,
}

/// An input that must be finite ([`DeriveCrustError::NotFinite`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CrustInput {
    /// [`CrustCompositionInputs::delta_iw`].
    DeltaIw,
    /// [`CrustCompositionInputs::alpha_fe`].
    AlphaFe,
    /// [`CrustCompositionInputs::fe_h`].
    FeH,
    /// [`CrustCompositionInputs::surface_age`].
    SurfaceAge,
    /// [`CrustCompositionInputs::molten_until`].
    MoltenUntil,
    /// [`CrustCompositionInputs::volcanism`].
    Volcanism,
    /// [`CrustCompositionInputs::continental_fraction`].
    ContinentalFraction,
}

impl CrustInput {
    /// The input's field name in [`CrustCompositionInputs`], as an error message names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeltaIw => "delta_iw",
            Self::AlphaFe => "alpha_fe",
            Self::FeH => "fe_h",
            Self::SurfaceAge => "surface_age",
            Self::MoltenUntil => "molten_until",
            Self::Volcanism => "volcanism",
            Self::ContinentalFraction => "continental_fraction",
        }
    }
}

/// [`CrustComposition::derive`] was given an input out of range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveCrustError {
    /// The body is in the gas-envelope state, with no solid surface and so no crust.
    NoSolidSurface,
    /// The radius was not finite and positive.
    RadiusNotPositive(Metres),
    /// The mass was not finite and positive.
    MassNotPositive(EarthMasses),
    /// The age was not finite and positive.
    AgeNotPositive(Years),
    /// The thermal history's surface temperature was not finite and positive.
    TemperatureNotPositive(Kelvin),
    /// The host's Mg/Si was not finite and positive.
    MagnesiumToSiliconNotPositive(f64),
    /// The mass fractions did not lie in [0, 1] and sum to 1 within 10⁻⁹: their sum.
    FractionsNotNormalised(f64),
    /// The formation time lay outside [0, the age].
    FormationOutsideAge {
        /// The formation time given.
        formation: Megayears,
        /// The age given.
        age: Years,
    },
    /// An input was not finite.
    NotFinite(CrustInput),
    /// The surface age lay outside [0, the age].
    SurfaceAgeOutsideAge {
        /// The surface age given.
        surface_age: Years,
        /// The age given.
        age: Years,
    },
    /// The volcanism level was negative.
    VolcanismNegative(f64),
    /// The continental fraction lay outside [0, 1].
    ContinentalFractionOutsideUnit(f64),
}

impl fmt::Display for DeriveCrustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RadiusNotPositive(r) => {
                write!(f, "radius {} m is not finite and positive", r.value())
            }
            Self::MassNotPositive(m) => {
                write!(f, "mass {} M_earth is not finite and positive", m.value())
            }
            Self::AgeNotPositive(a) => write!(f, "age {} yr is not finite and positive", a.value()),
            Self::TemperatureNotPositive(t) => write!(
                f,
                "formation temperature {} K is not finite and positive",
                t.value()
            ),
            Self::MagnesiumToSiliconNotPositive(r) => {
                write!(f, "host Mg/Si {r} is not finite and positive")
            }
            Self::FractionsNotNormalised(sum) => {
                write!(
                    f,
                    "mass fractions sum to {sum}, not 1, or lie outside [0, 1]"
                )
            }
            Self::FormationOutsideAge { formation, age } => write!(
                f,
                "formation time {} Myr lies outside [0, the age {} yr]",
                formation.value(),
                age.value()
            ),
            Self::NotFinite(input) => write!(f, "{} is not finite", input.name()),
            Self::NoSolidSurface => write!(f, "a gas-envelope body has no solid surface"),
            Self::SurfaceAgeOutsideAge { surface_age, age } => write!(
                f,
                "surface age {} yr lies outside [0, the age {} yr]",
                surface_age.value(),
                age.value()
            ),
            Self::VolcanismNegative(v) => write!(f, "volcanism level {v} is negative"),
            Self::ContinentalFractionOutsideUnit(f_c) => {
                write!(f, "continental fraction {f_c} lies outside [0, 1]")
            }
        }
    }
}

impl Error for DeriveCrustError {}

/// A solid body's crust by substance (the [module](self)'s model): its crusts or its `bulk`
/// material, its lid and provinces, its redox class and the mantle ΔIW it bands, its melt area,
/// and the thermal history that decided between them.
///
/// Built only by [`CrustComposition::derive`], which guarantees that `bulk` is present alone or
/// at least one of `primary` and `secondary` is, that no `tertiary` is present without a
/// `secondary`, that `lid` names a present member, that the redox class is the ΔIW's
/// ([`RedoxClass::of`]), and that the melt area lies in [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrustComposition {
    primary: Option<SubstanceId>,
    secondary: Option<SubstanceId>,
    tertiary: Option<SubstanceId>,
    bulk: Option<SubstanceId>,
    lid: LidCrust,
    provinces: SubstanceId,
    redox: RedoxClass,
    mantle_delta_iw: Dex,
    melt_area_fraction: f64,
    thermal_history: ThermalHistory,
    peak_central_temperature: Option<Kelvin>,
}

/// A body's crusts before its lid, as the rules give them at a climate; a melted body's tertiary
/// crust is the record's, which its continents set ([`CrustComposition::of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Crusts {
    /// An undifferentiated body's own material.
    Bulk(CrustLithology),
    /// A frozen ocean's shell over rock that never melted: a primary crust alone (Ceres).
    Shell(CrustLithology),
    /// A body whose rock melted: its secondary crust, and its primary where it has one.
    Melted {
        primary: Option<CrustLithology>,
        secondary: CrustLithology,
        /// Whether the primary crust is a frozen ocean's shell.
        ice_shell: bool,
    },
}

impl CrustComposition {
    /// The crust of the body `inputs` describe (the [module](self)'s model).
    ///
    /// # Errors
    ///
    /// A [`DeriveCrustError`] for an input out of range, the first found in this order: a body in
    /// the gas-envelope state; a radius, mass, age, temperature or Mg/Si that is not finite and
    /// positive; mass fractions that do not lie in [0, 1] and sum to 1; a formation time outside
    /// [0, the age]; any other input not finite; a surface age outside [0, the age]; a negative
    /// volcanism level; and a continental fraction outside [0, 1].
    ///
    /// # Examples
    ///
    /// An Earth, derived as [`SurfaceConditions::derive`]'s example derives it, shows its
    /// oceans' basalt with granite continents over a buried ultramafic crust, where the same world
    /// 1.5 Gyr old, its mantle hotter, erupts komatiite:
    ///
    /// ```
    /// # use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// # use hyperion_sim::planetary::derive::atmosphere::SurfaceMaterial;
    /// # use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    /// # use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    /// # use hyperion_sim::planetary::hooks::surface::{
    /// #     OceanWeight, SeasonalSpin, SurfaceConditions, SurfaceInputs,
    /// # };
    /// # use hyperion_sim::stellar::Composition;
    /// # use hyperion_sim::stellar::draws::UnitUniform;
    /// # use hyperion_sim::time::UniverseTime;
    /// # use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
    /// # use hyperion_sim::units::{
    /// #     Degrees, Dex, EarthMasses, GravitationalParameter, Kelvin, Kilograms, Megayears, Metres,
    /// #     Radians, Seconds, SolarLuminosities, SolarMasses, SolarRadii, WattsPerSquareMetre, Years,
    /// # };
    /// # let host = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
    /// # let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
    /// # let disc = disc.profile().expect("a median disc exists");
    /// # let a = Metres::new(METRES_PER_AU);
    /// # let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
    /// # let orientation = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
    /// # let orbit = KeplerElements::from_semi_major_axis(a, mu, Eccentricity::new(0.0167)?, orientation, Radians::ZERO)?;
    /// # let earth = PlacedBody::new(EarthMasses::new(1.0), orbit, a, UnitUniform::new(0.384).expect("inside (0, 1)"))?;
    /// # let sun = [HostLight::new(SolarLuminosities::new(1.0), Kelvin::new(5_772.0), SolarRadii::new(1.0))?];
    /// # let hosts = BodyHosts::new(Kilograms::new(SOLAR_MASS_KG), Composition::SOLAR, &sun, &[])?;
    /// # let derived = derive_body(&earth, &hosts, disc, Years::new(4.57e9), UniverseTime::EPOCH)?;
    /// # let conditions = SurfaceConditions::derive(&SurfaceInputs {
    /// #     atmosphere: derived.atmosphere(),
    /// #     equilibrium_temperature: derived.equilibrium_temperature(),
    /// #     surface_gravity: derived.surface_gravity(),
    /// #     radius: Metres::from(derived.radius()),
    /// #     material: SurfaceMaterial::of(&derived.fractions()),
    /// #     lid: None,
    /// #     stellar_flux: WattsPerSquareMetre::from(derived.flux()),
    /// #     xuv_flux: derived.xuv_flux(),
    /// #     spin: SeasonalSpin {
    /// #         obliquity: Radians::from(Degrees::new(23.44)),
    /// #         rotation_period: Seconds::new(86_164.1),
    /// #         solar_day: Some(Seconds::new(86_400.0)),
    /// #     },
    /// #     heat_flow: WattsPerSquareMetre::new(0.0925),
    /// #     ocean_weight: OceanWeight::FULL,
    /// # })
    /// # .expect("an Earth has a surface");
    /// use hyperion_sim::planetary::chem::solar_magnesium_to_silicon;
    /// use hyperion_sim::planetary::derive::SnowLineSide;
    /// use hyperion_sim::planetary::hooks::crust::{
    ///     AmmoniaIce, CrustComposition, CrustCompositionInputs, CrustLithology, CrustOrigin,
    ///     LidCrust, TidalVolcanism,
    /// };
    ///
    /// let inputs = CrustCompositionInputs {
    ///     origin: CrustOrigin::Planet,
    ///     formation: Megayears::new(3.0),
    ///     age: Years::new(4.57e9),
    ///     molten_until: Some(Years::new(3.16e7)),
    ///     radius: Metres::from(derived.radius()),
    ///     mass: derived.mass(),
    ///     fractions: derived.fractions(),
    ///     snow_line_side: SnowLineSide::Inside,
    ///     ammonia: AmmoniaIce::Absent,
    ///     delta_iw: Dex::new(3.55),
    ///     host_magnesium_to_silicon: solar_magnesium_to_silicon(),
    ///     alpha_fe: Dex::new(0.0),
    ///     fe_h: Dex::new(0.0),
    ///     surface_age: Years::new(6.42e7),
    ///     volcanism: 0.96,
    ///     continental_fraction: 0.405,
    ///     tidal: TidalVolcanism::Quiet,
    ///     state: derived.atmosphere().state(),
    ///     formation_temperature: derived.equilibrium_temperature(),
    ///     surface: &conditions,
    /// };
    /// let crust = CrustComposition::derive(&inputs)?;
    /// assert_eq!(crust.lid(), LidCrust::Secondary);
    /// assert_eq!(crust.secondary(), Some(CrustLithology::Basalt.substance()));
    /// assert_eq!(crust.tertiary(), Some(CrustLithology::Granite.substance()));
    /// assert_eq!(crust.primary(), Some(CrustLithology::Ultramafic.substance()));
    /// let young = CrustCompositionInputs { age: Years::new(1.5e9), ..inputs };
    /// let crust = CrustComposition::derive(&young)?;
    /// assert_eq!(crust.secondary(), Some(CrustLithology::Komatiite.substance()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn derive(inputs: &CrustCompositionInputs<'_>) -> Result<Self, DeriveCrustError> {
        validate(inputs)?;
        let basis = CrustBasis::derive(&CrustBasisInputs::of(inputs))?;
        let lid = basis.lid(&LidClimate::of(inputs))?;
        Self::of(&lid, &CrustRecord::of(inputs))
    }

    /// The crust on the record of the lid `lid` ([`CrustBasis::lid`]) under what only the record
    /// reads, `record` (P14.T54.a; the [module](self)'s "In stages"): the tertiary crust, the
    /// provinces and the melt area.
    ///
    /// # Errors
    ///
    /// A [`DeriveCrustError`] for a volcanism level or a continental fraction out of range, as
    /// [`CrustComposition::derive`] orders them.
    pub(crate) fn of(lid: &CrustLid, record: &CrustRecord<'_>) -> Result<Self, DeriveCrustError> {
        validate_record(record)?;
        let (primary, secondary, tertiary, bulk) = match lid.crusts {
            Crusts::Bulk(bulk) => (None, None, None, Some(bulk)),
            Crusts::Shell(shell) => (Some(shell), None, None, None),
            Crusts::Melted {
                primary, secondary, ..
            } => (
                primary,
                Some(secondary),
                (record.continental_fraction > 0.0).then_some(CrustLithology::Granite),
                None,
            ),
        };
        let provinces = match lid.crusts {
            Crusts::Melted {
                secondary,
                ice_shell: false,
                ..
            } if lid.lid == LidCrust::Primary && record.volcanism > 0.0 => secondary,
            Crusts::Bulk(_) | Crusts::Shell(_) | Crusts::Melted { .. } => lid.lithology,
        };
        Ok(Self {
            primary: primary.map(CrustLithology::substance),
            secondary: secondary.map(CrustLithology::substance),
            tertiary: tertiary.map(CrustLithology::substance),
            bulk: bulk.map(CrustLithology::substance),
            lid: lid.lid,
            provinces: provinces.substance(),
            redox: lid.redox,
            mantle_delta_iw: lid.delta_iw,
            melt_area_fraction: melt_area(lid.state, record.surface, lid.lithology),
            thermal_history: lid.thermal_history,
            peak_central_temperature: lid.peak,
        })
    }

    /// The primary crust, crystallised from the body's first global liquid, buried or at the
    /// surface: a magma ocean's flotation crust or a frozen ocean's shell; `None` for none.
    #[must_use]
    pub const fn primary(&self) -> Option<SubstanceId> {
        self.primary
    }

    /// The secondary crust, the solid mantle's partial melt; `None` where the mantle never melted.
    #[must_use]
    pub const fn secondary(&self) -> Option<SubstanceId> {
        self.secondary
    }

    /// The tertiary crust, `granite` where the continental fraction is above zero; `None` for
    /// none.
    #[must_use]
    pub const fn tertiary(&self) -> Option<SubstanceId> {
        self.tertiary
    }

    /// The undifferentiated body's own material at its surface, with no crust beside it; `None`
    /// for a body with crusts.
    #[must_use]
    pub const fn bulk(&self) -> Option<SubstanceId> {
        self.bulk
    }

    /// Which of its crusts, or its bulk material, lies at the surface.
    #[must_use]
    pub const fn lid(&self) -> LidCrust {
        self.lid
    }

    /// The lithology of its volcanic provinces.
    #[must_use]
    pub const fn provinces(&self) -> SubstanceId {
        self.provinces
    }

    /// The mantle's redox class, P14.T50.a's banding of its ΔIW.
    #[must_use]
    pub const fn redox(&self) -> RedoxClass {
        self.redox
    }

    /// The mantle's oxygen fugacity, ΔIW, in dex relative to the iron–wüstite buffer: finite and
    /// of either sign, below 0 more reduced than the buffer. It is the crust basis's input
    /// [`CrustCompositionInputs::delta_iw`], P14.T50.a's
    /// [`ElementInventory::mantle_delta_iw`](crate::planetary::chem::inventory::ElementInventory::mantle_delta_iw),
    /// which the [`redox`](Self::redox) class bands; the wire sends it as `mantle_redox_iw`
    /// (P14.T54.a).
    #[must_use]
    pub const fn mantle_delta_iw(&self) -> Dex {
        self.mantle_delta_iw
    }

    /// The share of the surface above a silicate lid's solidus, 0 to 1; 0 for an ice lid, whose
    /// liquid is a condensate, outside the `MagmaOcean` state, which takes 1.
    #[must_use]
    pub const fn melt_area_fraction(&self) -> f64 {
        self.melt_area_fraction
    }

    /// What its thermal history melted, and why.
    #[must_use]
    pub const fn thermal_history(&self) -> ThermalHistory {
        self.thermal_history
    }

    /// Its peak central temperature `T_c,max` ([`peak_central_temperature`]), K: the conductive
    /// sphere's, a threshold test with no convection, latent heat or melt loss, so that above the
    /// solidus it is not a temperature the body reached (Batch D's science check, S3-2); `None`
    /// where a rule decided its class without it (a planet, a rocky parent's impact moon, a
    /// tidally volcanic body).
    #[must_use]
    pub const fn peak_central_temperature(&self) -> Option<Kelvin> {
        self.peak_central_temperature
    }
}

/// Refuses the inputs out of range ([`CrustComposition::derive`]'s errors), in the order its
/// `# Errors` lists them.
fn validate(inputs: &CrustCompositionInputs<'_>) -> Result<(), DeriveCrustError> {
    if inputs.state == SurfaceState::GasEnvelope {
        return Err(DeriveCrustError::NoSolidSurface);
    }
    validate_body(&BodyChecked {
        radius: inputs.radius,
        mass: inputs.mass,
        age: inputs.age,
        formation_temperature: inputs.formation_temperature,
        host_magnesium_to_silicon: inputs.host_magnesium_to_silicon,
        fractions: inputs.fractions,
        formation: inputs.formation,
    })?;
    let finite = [
        (inputs.delta_iw.value(), CrustInput::DeltaIw),
        (inputs.alpha_fe.value(), CrustInput::AlphaFe),
        (inputs.fe_h.value(), CrustInput::FeH),
        (inputs.surface_age.value(), CrustInput::SurfaceAge),
        (
            inputs.molten_until.map_or(0.0, Years::value),
            CrustInput::MoltenUntil,
        ),
        (inputs.volcanism, CrustInput::Volcanism),
        (inputs.continental_fraction, CrustInput::ContinentalFraction),
    ];
    if let Some((_, input)) = finite.into_iter().find(|(x, _)| !x.is_finite()) {
        return Err(DeriveCrustError::NotFinite(input));
    }
    let surface_age = inputs.surface_age.value();
    if !(surface_age >= 0.0 && surface_age <= inputs.age.value()) {
        return Err(DeriveCrustError::SurfaceAgeOutsideAge {
            surface_age: inputs.surface_age,
            age: inputs.age,
        });
    }
    if inputs.volcanism < 0.0 {
        return Err(DeriveCrustError::VolcanismNegative(inputs.volcanism));
    }
    if !(0.0..=1.0).contains(&inputs.continental_fraction) {
        return Err(DeriveCrustError::ContinentalFractionOutsideUnit(
            inputs.continental_fraction,
        ));
    }
    Ok(())
}

/// What a body's crust reads that no pass moves (P14.T54.a; the [module](self)'s "In stages"):
/// [`CrustCompositionInputs`]'s pass-invariant fields, as their doc gives them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CrustBasisInputs {
    /// How it came to be.
    pub(crate) origin: CrustOrigin,
    /// Its formation time `t_f` ([`CrustCompositionInputs::formation`]).
    pub(crate) formation: Megayears,
    /// Its system's age at the time, positive.
    pub(crate) age: Years,
    /// The system age at which its magma ocean ended, a planet's only.
    pub(crate) molten_until: Option<Years>,
    /// Its mean radius, positive.
    pub(crate) radius: Metres,
    /// Its mass, positive.
    pub(crate) mass: EarthMasses,
    /// Its mass fractions, summing to 1.
    pub(crate) fractions: MassFractions,
    /// The side of the snow line it formed on.
    pub(crate) snow_line_side: SnowLineSide,
    /// Whether its ice carries ammonia.
    pub(crate) ammonia: AmmoniaIce,
    /// Its mantle's ΔIW, log₁₀ units.
    pub(crate) delta_iw: Dex,
    /// Its host's Mg/Si by number, positive.
    pub(crate) host_magnesium_to_silicon: f64,
    /// Its system's \[α/Fe\], dex.
    pub(crate) alpha_fe: Dex,
    /// Its host's \[Fe/H\], dex.
    pub(crate) fe_h: Dex,
    /// The surface temperature `T_s` of its thermal history, positive: the formation stand-in.
    pub(crate) formation_temperature: Kelvin,
}

impl CrustBasisInputs {
    /// The pass-invariant fields of `inputs`.
    #[must_use]
    pub(crate) const fn of(inputs: &CrustCompositionInputs<'_>) -> Self {
        Self {
            origin: inputs.origin,
            formation: inputs.formation,
            age: inputs.age,
            molten_until: inputs.molten_until,
            radius: inputs.radius,
            mass: inputs.mass,
            fractions: inputs.fractions,
            snow_line_side: inputs.snow_line_side,
            ammonia: inputs.ammonia,
            delta_iw: inputs.delta_iw,
            host_magnesium_to_silicon: inputs.host_magnesium_to_silicon,
            alpha_fe: inputs.alpha_fe,
            fe_h: inputs.fe_h,
            formation_temperature: inputs.formation_temperature,
        }
    }
}

/// What a pass's climate gives a body's crust (P14.T54.a; the [module](self)'s "In stages"): its
/// state, its mean and warmest surface temperatures, its surface age and its tide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LidClimate {
    /// Its surface state, which is not the gas-envelope state.
    pub(crate) state: SurfaceState,
    /// Its mean surface temperature, the craters' ice-rich test's
    /// ([`SurfaceConditions::mean_surface_temperature`]).
    pub(crate) mean_surface_temperature: Kelvin,
    /// Its warmest temperature, the cometary test's (the warmest of
    /// [`SurfaceConditions::climate_range`]).
    pub(crate) warmest: Kelvin,
    /// Its surface age ([`CrustCompositionInputs::surface_age`]), in [0, the age].
    pub(crate) surface_age: Years,
    /// Whether its tides make it volcanic ([`CrustCompositionInputs::tidal`]).
    pub(crate) tidal: TidalVolcanism,
}

impl LidClimate {
    /// The climate `inputs` give: their state, surface age and tide, and their surface's mean and
    /// warmest temperatures.
    #[must_use]
    pub(crate) const fn of(inputs: &CrustCompositionInputs<'_>) -> Self {
        Self {
            state: inputs.state,
            mean_surface_temperature: inputs.surface.mean_surface_temperature(),
            warmest: inputs.surface.climate_range().1,
            surface_age: inputs.surface_age,
            tidal: inputs.tidal,
        }
    }

    /// The climate but for its warmest temperature ([`LidAt`]).
    #[must_use]
    const fn at(&self) -> LidAt {
        LidAt {
            state: self.state,
            mean_surface_temperature: self.mean_surface_temperature,
            surface_age: self.surface_age,
            tidal: self.tidal,
        }
    }
}

/// A pass's climate as its lid reads it beside its warmest temperature (P14.T54.a;
/// [`CrustBasis::lid_at`]): its state, its mean surface temperature, its surface age and its tide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LidAt {
    /// Its surface state, which is not the gas-envelope state.
    pub(crate) state: SurfaceState,
    /// Its mean surface temperature, the craters' ice-rich test's.
    pub(crate) mean_surface_temperature: Kelvin,
    /// Its surface age, in [0, the age].
    pub(crate) surface_age: Years,
    /// Whether its tides make it volcanic.
    pub(crate) tidal: TidalVolcanism,
}

/// What only the record reads of a body's crust (P14.T54.a; the [module](self)'s "In stages"):
/// its volcanism level, its continental fraction and its surface conditions, for the provinces,
/// the tertiary crust and the melt area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CrustRecord<'a> {
    /// Its volcanism level V ([`CrustCompositionInputs::volcanism`]), at least 0.
    pub(crate) volcanism: f64,
    /// Its continental fraction `f_c` ([`CrustCompositionInputs::continental_fraction`]), 0 to 1.
    pub(crate) continental_fraction: f64,
    /// Its surface conditions, for the melt area.
    pub(crate) surface: &'a SurfaceConditions,
}

impl<'a> CrustRecord<'a> {
    /// What `inputs` give the record.
    #[must_use]
    pub(crate) const fn of(inputs: &CrustCompositionInputs<'a>) -> Self {
        Self {
            volcanism: inputs.volcanism,
            continental_fraction: inputs.continental_fraction,
            surface: inputs.surface,
        }
    }
}

/// How a body's thermal history melts it, as its origin and its series decide before any pass
/// (the [module](self)'s three classes).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Melting {
    /// A planet's magma ocean or a rocky parent's impact debris, whatever its tide.
    ByRule(MeltCause),
    /// Any other origin: its tide's where it is volcanic; otherwise the class `quiet` its series'
    /// peak central temperature `peak` gives.
    ByHeat {
        /// Its peak central temperature `T_c,max`.
        peak: Kelvin,
        /// Its class with no tidal volcanism.
        quiet: ThermalHistory,
    },
}

/// A body's crust before any pass (P14.T54.a; the [module](self)'s "In stages"): what no pass
/// moves, once a body. Its inputs validated, its redox class, its mantle's `FeO` and norm, and its
/// thermal history's peak wherever its origin can need it, whatever its tide, so that a tide's
/// change between passes never re-runs the series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CrustBasis {
    inputs: CrustBasisInputs,
    redox: RedoxClass,
    iron_oxide: f64,
    norm: MantleNorm,
    melting: Melting,
}

impl CrustBasis {
    /// The basis of the body `inputs` describe.
    ///
    /// # Errors
    ///
    /// A [`DeriveCrustError`] for an input out of range, the first in
    /// [`CrustComposition::derive`]'s order that these inputs hold.
    pub(crate) fn derive(inputs: &CrustBasisInputs) -> Result<Self, DeriveCrustError> {
        validate_basis(inputs)?;
        let iron_oxide = mantle_iron_oxide_mass_fraction(
            core_share(&inputs.fractions),
            inputs.host_magnesium_to_silicon,
            inputs.alpha_fe,
        );
        Ok(Self {
            inputs: *inputs,
            redox: RedoxClass::of(inputs.delta_iw),
            iron_oxide,
            norm: MantleNorm::of(inputs.host_magnesium_to_silicon, iron_oxide),
            melting: melting(inputs),
        })
    }

    /// The lid at the climate `climate` (the [module](self)'s "In stages"): the thermal class, the
    /// crusts and which of them lies at the surface.
    ///
    /// # Errors
    ///
    /// A [`DeriveCrustError`] for a body in the gas-envelope state, or a surface age that is not
    /// finite or lies outside [0, the age], in [`CrustComposition::derive`]'s order.
    pub(crate) fn lid(&self, climate: &LidClimate) -> Result<CrustLid, DeriveCrustError> {
        self.lid_at(&climate.at(), || climate.warmest)
    }

    /// [`lid`](Self::lid) at the climate `at` with its warmest temperature `warmest`, which is read
    /// only where the cometary test needs it: a body whose ice melted and whose ocean did not
    /// freeze into a shell (the [module](self)'s class 2). So a pass whose column holds nothing
    /// that condenses, and builds no climate's range of its own, builds one only for such a body
    /// (P14.T54.a). The same lid, to the bit, as [`lid`](Self::lid) at that warmest temperature.
    ///
    /// # Errors
    ///
    /// As [`lid`](Self::lid).
    pub(crate) fn lid_at(
        &self,
        at: &LidAt,
        warmest: impl FnOnce() -> Kelvin,
    ) -> Result<CrustLid, DeriveCrustError> {
        validate_climate(at, self.inputs.age)?;
        let (thermal_history, peak) = match (self.melting, at.tidal) {
            (Melting::ByRule(cause), _) => (ThermalHistory::RockMelted(cause), None),
            (Melting::ByHeat { .. }, TidalVolcanism::Volcanic) => {
                (ThermalHistory::RockMelted(MeltCause::Tides), None)
            }
            (Melting::ByHeat { peak, quiet }, TidalVolcanism::Quiet) => (quiet, Some(peak)),
        };
        let crusts = match thermal_history {
            ThermalHistory::RockMelted(cause) => melted_crusts(self, cause, at),
            ThermalHistory::IceMelted(_) => ice_melted(&self.inputs, at, warmest, peak),
            ThermalHistory::Unmelted => Crusts::Bulk(unmelted_bulk(&self.inputs, self.redox)),
        };
        let (lid, lithology) = lid_of(&self.inputs, at.surface_age, crusts);
        Ok(CrustLid {
            crusts,
            lid,
            lithology,
            redox: self.redox,
            delta_iw: self.inputs.delta_iw,
            thermal_history,
            peak,
            state: at.state,
        })
    }

    /// `lid_of`'s unresurfaced test at the surface age `surface_age`: whether nothing has
    /// resurfaced the body since its primary crust formed (its surface age at least the time
    /// since its magma ocean's end, or its age where it had none, within the subtraction's
    /// rounding). The passes' context holds it as a key member, since it chooses the next pass's
    /// lid (decision-p14-t54a-passes §1.5).
    #[must_use]
    pub(crate) fn unresurfaced(&self, surface_age: Years) -> bool {
        unresurfaced(&self.inputs, surface_age)
    }
}

/// A body's crust at a pass's climate (P14.T54.a; [`CrustBasis::lid`]): its crusts, its lid and
/// the lithology the lid names, with its redox class and the ΔIW it bands, its thermal history and
/// its state, all that the record's crust reads beyond [`CrustRecord`] ([`CrustComposition::of`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CrustLid {
    crusts: Crusts,
    lid: LidCrust,
    lithology: CrustLithology,
    redox: RedoxClass,
    delta_iw: Dex,
    thermal_history: ThermalHistory,
    peak: Option<Kelvin>,
    state: SurfaceState,
}

impl CrustLid {
    /// The registry row at the surface, the lid's lithology's: the row the regolith and the
    /// ground's thermal inertia read.
    #[must_use]
    pub(crate) fn row(&self) -> SubstanceId {
        self.lithology.substance()
    }

    /// The lithology at the surface.
    #[must_use]
    pub(crate) const fn lithology(&self) -> CrustLithology {
        self.lithology
    }

    /// Which crust, or the bulk material, lies at the surface.
    #[must_use]
    pub(crate) const fn crust(&self) -> LidCrust {
        self.lid
    }
}

/// What [`validate`] and [`validate_basis`] check alike, the body's pass-invariant scalars in
/// [`CrustComposition::derive`]'s order (one helper, so that the stages' refusals are the
/// derivation's by construction; Batch D's review, RC1-c).
struct BodyChecked {
    radius: Metres,
    mass: EarthMasses,
    age: Years,
    formation_temperature: Kelvin,
    host_magnesium_to_silicon: f64,
    fractions: MassFractions,
    formation: Megayears,
}

/// Refuses the body's scalars `body` out of range, in [`CrustComposition::derive`]'s order: the
/// radius, the mass, the age, the formation temperature and the host's Mg/Si, each positive and
/// finite; the fractions normalised; and the formation time within the age.
fn validate_body(body: &BodyChecked) -> Result<(), DeriveCrustError> {
    let positive = |x: f64| x.is_finite() && x > 0.0;
    if !positive(body.radius.value()) {
        return Err(DeriveCrustError::RadiusNotPositive(body.radius));
    }
    if !positive(body.mass.value()) {
        return Err(DeriveCrustError::MassNotPositive(body.mass));
    }
    if !positive(body.age.value()) {
        return Err(DeriveCrustError::AgeNotPositive(body.age));
    }
    if !positive(body.formation_temperature.value()) {
        return Err(DeriveCrustError::TemperatureNotPositive(
            body.formation_temperature,
        ));
    }
    if !positive(body.host_magnesium_to_silicon) {
        return Err(DeriveCrustError::MagnesiumToSiliconNotPositive(
            body.host_magnesium_to_silicon,
        ));
    }
    let f = body.fractions;
    let parts = [f.iron(), f.rock(), f.water(), f.envelope()];
    let sum = parts.iter().fold(0.0, |sum, &x| sum + x);
    if !(parts.iter().all(|x| (0.0..=1.0).contains(x)) && (sum - 1.0).abs() <= 1e-9) {
        return Err(DeriveCrustError::FractionsNotNormalised(sum));
    }
    let formed = Years::from(body.formation).value();
    if !(formed >= 0.0 && formed <= body.age.value()) {
        return Err(DeriveCrustError::FormationOutsideAge {
            formation: body.formation,
            age: body.age,
        });
    }
    Ok(())
}

/// Refuses the pass-invariant inputs out of range ([`CrustBasis::derive`]), in
/// [`CrustComposition::derive`]'s order.
fn validate_basis(inputs: &CrustBasisInputs) -> Result<(), DeriveCrustError> {
    validate_body(&BodyChecked {
        radius: inputs.radius,
        mass: inputs.mass,
        age: inputs.age,
        formation_temperature: inputs.formation_temperature,
        host_magnesium_to_silicon: inputs.host_magnesium_to_silicon,
        fractions: inputs.fractions,
        formation: inputs.formation,
    })?;
    let finite = [
        (inputs.delta_iw.value(), CrustInput::DeltaIw),
        (inputs.alpha_fe.value(), CrustInput::AlphaFe),
        (inputs.fe_h.value(), CrustInput::FeH),
        (
            inputs.molten_until.map_or(0.0, Years::value),
            CrustInput::MoltenUntil,
        ),
    ];
    if let Some((_, input)) = finite.into_iter().find(|(x, _)| !x.is_finite()) {
        return Err(DeriveCrustError::NotFinite(input));
    }
    Ok(())
}

/// Refuses a climate out of range for a body of age `age` ([`CrustBasis::lid`]), in
/// [`CrustComposition::derive`]'s order.
fn validate_climate(climate: &LidAt, age: Years) -> Result<(), DeriveCrustError> {
    if climate.state == SurfaceState::GasEnvelope {
        return Err(DeriveCrustError::NoSolidSurface);
    }
    let surface_age = climate.surface_age.value();
    if !surface_age.is_finite() {
        return Err(DeriveCrustError::NotFinite(CrustInput::SurfaceAge));
    }
    if !(surface_age >= 0.0 && surface_age <= age.value()) {
        return Err(DeriveCrustError::SurfaceAgeOutsideAge {
            surface_age: climate.surface_age,
            age,
        });
    }
    Ok(())
}

/// Refuses what the record reads out of range ([`CrustComposition::of`]), in
/// [`CrustComposition::derive`]'s order.
fn validate_record(record: &CrustRecord<'_>) -> Result<(), DeriveCrustError> {
    let finite = [
        (record.volcanism, CrustInput::Volcanism),
        (record.continental_fraction, CrustInput::ContinentalFraction),
    ];
    if let Some((_, input)) = finite.into_iter().find(|(x, _)| !x.is_finite()) {
        return Err(DeriveCrustError::NotFinite(input));
    }
    if record.volcanism < 0.0 {
        return Err(DeriveCrustError::VolcanismNegative(record.volcanism));
    }
    if !(0.0..=1.0).contains(&record.continental_fraction) {
        return Err(DeriveCrustError::ContinentalFractionOutsideUnit(
            record.continental_fraction,
        ));
    }
    Ok(())
}

/// Whether the body holds ice: formed beyond the snow line with water above zero.
#[must_use]
fn holds_ice(inputs: &CrustBasisInputs) -> bool {
    inputs.snow_line_side == SnowLineSide::Beyond && inputs.fractions.water() > 0.0
}

/// The specific heat and rock of the body's thermal history: an ice–rock mixture's for a body
/// holding ice, and dry rock's otherwise, of ordinary chondrite inside the snow line and
/// carbonaceous rock beyond it.
#[must_use]
fn thermal_material(inputs: &CrustBasisInputs) -> (f64, ChondriticRock) {
    let rock = match inputs.snow_line_side {
        SnowLineSide::Inside => ChondriticRock::Ordinary,
        SnowLineSide::Beyond => ChondriticRock::Carbonaceous,
    };
    let c = if holds_ice(inputs) {
        history::ice_rock_specific_heat_j_per_kg_k(inputs.fractions.water())
    } else {
        history::DRY_ROCK_SPECIFIC_HEAT_J_PER_KG_K
    };
    (c, rock)
}

/// How the body's thermal history melts it (the [module](self)'s three classes): by its origin's
/// rule, or by its series' peak central temperature, computed wherever its origin can need it.
#[must_use]
fn melting(inputs: &CrustBasisInputs) -> Melting {
    let by_rule = match inputs.origin {
        CrustOrigin::Planet => Some(MeltCause::MagmaOcean),
        CrustOrigin::ImpactMoon(ParentSurface::Rocky) => Some(MeltCause::ImpactDebris),
        CrustOrigin::RegularMoon
        | CrustOrigin::ImpactMoon(ParentSurface::Icy)
        | CrustOrigin::CapturedMoon
        | CrustOrigin::BeltMember
        | CrustOrigin::Comet => None,
    };
    if let Some(cause) = by_rule {
        return Melting::ByRule(cause);
    }
    let (c, rock) = thermal_material(inputs);
    let ice = inputs.fractions.water();
    let rock_share = 1.0 - ice - inputs.fractions.envelope();
    let sphere = ConductiveSphere {
        radius: inputs.radius,
        diffusivity_m2_per_s: ROCK_THERMAL_DIFFUSIVITY_M2_PER_S,
        specific_heat_j_per_kg_k: c,
        surface_temperature: inputs.formation_temperature,
    };
    let sources = chondritic_sources(inputs.formation, rock, rock_share, inputs.fe_h);
    let duration = Years::new(inputs.age.value() - Years::from(inputs.formation).value());
    let peak = peak_central_temperature(&sphere, &sources, duration);
    let quiet = if peak >= CrustLithology::ChondriteH.transition() {
        ThermalHistory::RockMelted(MeltCause::Radiogenic)
    } else if inputs.origin == CrustOrigin::ImpactMoon(ParentSurface::Icy) {
        ThermalHistory::IceMelted(IceMeltCause::ImpactDebris)
    } else if holds_ice(inputs)
        && peak.value()
            >= ice_melting_point(inputs.ammonia).value() + ice_latent_heat_j_per_kg() * ice / c
    {
        ThermalHistory::IceMelted(IceMeltCause::Radiogenic)
    } else {
        ThermalHistory::Unmelted
    };
    Melting::ByHeat { peak, quiet }
}

/// Whether the body's ocean froze into an ice-rich crust under the mean surface temperature
/// `mean_surface_temperature`, by [`Crust::of`]: the craters' one ice-rich predicate, its
/// birthplace icy for a body formed beyond the snow line.
#[must_use]
fn ocean_froze(inputs: &CrustBasisInputs, mean_surface_temperature: Kelvin) -> bool {
    let birthplace = match inputs.snow_line_side {
        SnowLineSide::Beyond => Birthplace::Icy,
        SnowLineSide::Inside => Birthplace::Other,
    };
    let water = Kilograms::new(Kilograms::from(inputs.mass).value() * inputs.fractions.water());
    inputs.fractions.water() > 0.0
        && Crust::of(birthplace, mean_surface_temperature, water, inputs.radius) == Crust::IceRich
}

/// The crusts of the body of basis `basis` whose rock melted, by `cause`, at the climate
/// `climate` ([module](self)'s "The crusts of a body whose rock melted").
#[must_use]
fn melted_crusts(basis: &CrustBasis, cause: MeltCause, climate: &LidAt) -> Crusts {
    let inputs = &basis.inputs;
    let reduced = basis.redox == RedoxClass::Reduced;
    let magma_ocean = match cause {
        MeltCause::MagmaOcean | MeltCause::ImpactDebris => true,
        MeltCause::Tides | MeltCause::Radiogenic => false,
    };
    let flotation = magma_ocean && !reduced && inputs.mass < FLOTATION_MASS_LIMIT;
    let ice_shell = ocean_froze(inputs, climate.mean_surface_temperature);
    let primary = if ice_shell {
        // Its rock differentiated beneath its ocean (Europa), so its shell is ice.
        Some(CrustLithology::Water)
    } else if !magma_ocean {
        None
    } else if reduced {
        Some(CrustLithology::Graphite)
    } else if flotation {
        Some(CrustLithology::Feldspathic)
    } else {
        Some(CrustLithology::Ultramafic)
    };
    let secondary = if reduced {
        CrustLithology::ReducedBasalt
    } else if hot_mantle(inputs.fe_h, inputs.age, climate.tidal) {
        CrustLithology::Komatiite
    } else if flotation || basis.iron_oxide >= IRON_RICH_MANTLE_FEO {
        // An iron-rich source stays iron-rich: where its norm is periclase-normative, its excess
        // is its `FeO`'s, and its melt is not Mg-rich.
        CrustLithology::IronRichBasalt
    } else {
        match basis.norm {
            MantleNorm::PericlaseNormative => CrustLithology::Komatiite,
            // A silica-rich secondary crust has no row (not modelled).
            MantleNorm::QuartzNormative | MantleNorm::OlivinePyroxene => CrustLithology::Basalt,
        }
    };
    Crusts::Melted {
        primary,
        secondary,
        ice_shell,
    }
}

/// The crust at the climate `climate` of a body whose ice melted and whose rock did not, whose
/// peak central temperature is `peak` ([module](self)'s class 2), its warmest temperature
/// `warmest` read only for the cometary test of a body whose ocean did not freeze into a shell.
#[must_use]
fn ice_melted(
    inputs: &CrustBasisInputs,
    climate: &LidAt,
    warmest: impl FnOnce() -> Kelvin,
    peak: Option<Kelvin>,
) -> Crusts {
    let water = inputs.fractions.water();
    if inputs.radius >= OCEAN_SEPARATION_RADIUS
        && ocean_froze(inputs, climate.mean_surface_temperature)
    {
        let shell = if water >= ICE_SHELL_FRACTION {
            CrustLithology::Water
        } else {
            CrustLithology::Phyllosilicate
        };
        return Crusts::Shell(shell);
    }
    let rock = inputs.fractions.iron() + inputs.fractions.rock();
    let bulk = if warmest() < WATER_ICE_RETENTION_TEMPERATURE {
        CrustLithology::Cometary
    } else if peak.is_some_and(|t| t >= DEHYDRATION_TEMPERATURE) {
        CrustLithology::ChondriteCv
    } else if rock > 0.0 && water / rock >= CI_ICE_TO_ROCK {
        CrustLithology::ChondriteCi
    } else {
        CrustLithology::ChondriteCm
    };
    Crusts::Bulk(bulk)
}

/// The `bulk` material of a body that never melted ([module](self)'s class 3).
#[must_use]
fn unmelted_bulk(inputs: &CrustBasisInputs, redox: RedoxClass) -> CrustLithology {
    match (inputs.snow_line_side, redox) {
        (SnowLineSide::Beyond, _) => CrustLithology::Cometary,
        (SnowLineSide::Inside, RedoxClass::Reduced) => CrustLithology::ChondriteE,
        (SnowLineSide::Inside, RedoxClass::Intermediate) => ordinary_chondrite(inputs.delta_iw),
        (SnowLineSide::Inside, RedoxClass::Oxidised) => CrustLithology::ChondriteLl,
    }
}

/// The relative rounding within which a surface age equal to the time since the magma ocean's end
/// counts as unresurfaced: the two are one subtraction apart on the record.
const SURFACE_AGE_ROUNDING: f64 = 1e-9;

/// Whether nothing has resurfaced the body `inputs` describe by the surface age `surface_age`
/// (`lid_of`'s test): that age at least the time since its magma ocean's end, or its age where it
/// had none, within [`SURFACE_AGE_ROUNDING`].
#[must_use]
fn unresurfaced(inputs: &CrustBasisInputs, surface_age: Years) -> bool {
    let since_melt = inputs
        .molten_until
        .map_or(inputs.age.value(), |end| inputs.age.value() - end.value());
    surface_age.value() >= since_melt * (1.0 - SURFACE_AGE_ROUNDING)
}

/// The body's lid under the surface age `surface_age` ([module](self)'s "The lid and the
/// provinces"), and the lithology it names: a `graphite` primary crust is never the lid, since a
/// melted body's secondary crust buries it.
#[must_use]
fn lid_of(
    inputs: &CrustBasisInputs,
    surface_age: Years,
    crusts: Crusts,
) -> (LidCrust, CrustLithology) {
    let unresurfaced = unresurfaced(inputs, surface_age);
    match crusts {
        Crusts::Bulk(bulk) => (LidCrust::Bulk, bulk),
        Crusts::Shell(shell) => (LidCrust::Primary, shell),
        Crusts::Melted {
            primary: Some(primary),
            ice_shell,
            ..
        } if ice_shell || (unresurfaced && primary != CrustLithology::Graphite) => {
            (LidCrust::Primary, primary)
        }
        Crusts::Melted { secondary, .. } => (LidCrust::Secondary, secondary),
    }
}

/// The share of the surface `surface` above the solidus of `lid` ([module](self)'s "The melt
/// area"), in [0, 1], in the state `state`: 1 in the `MagmaOcean` state, and otherwise 0 for an
/// ice lid, whose liquid is a condensate.
#[must_use]
fn melt_area(state: SurfaceState, surface: &SurfaceConditions, lid: CrustLithology) -> f64 {
    if state == SurfaceState::MagmaOcean {
        return 1.0;
    }
    if lid.is_ice() {
        return 0.0;
    }
    let melt = 1.0 - surface.cold_area(lid.transition());
    // By comparison, so that a NaN, which no valid surface makes, becomes no melt.
    if melt > 1.0 {
        1.0
    } else if melt > 0.0 {
        melt
    } else {
        0.0
    }
}
