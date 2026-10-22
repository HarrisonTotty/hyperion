//! A body's surface section and its gas envelope on the wire (plan 14, P14.T35.e; the
//! simulation's `record::Surface` and `record::Envelope`, decision-p14-t35e-wire).
//!
//! # Exactly one of the two
//!
//! A body with a solid surface (a rocky or icy planet, a moon, a dwarf planet) has its surface
//! section and an envelope `not_applicable`; a body in the gas-envelope state (a gas or ice giant,
//! a sub-Neptune) has its envelope and a surface `not_applicable`, since within the model it has no
//! surface: no surface pressure, its heights from the 1-bar level, and its envelope's base at
//! kilobars. A ring, a belt or the halo has neither.
//!
//! # Substances are keys
//!
//! Every gas, condensable, liquid, ice, aerosol material, deck and absorber is a key of plan 14's
//! substance registry, a string (`H2O`, `CO2`, `tholin`), never a closed enum, so that a new
//! registry row reaches the wire with no change of its shape. The registry's keys are pinned by
//! `packages/protocol/fixtures/substances.json`, append-only. A reader names a substance it does
//! not know by the key's own rule and never refuses it (decision-composition §1.1, §1.10).
//!
//! # None, not modelled and a value
//!
//! A value the generator computes as none is `null`: an airless body's mean molar mass, a world
//! with no condensable's `condensable`, a world never wet's `wet_epoch`, a body computed to hold no
//! subsurface ocean's `subsurface_ocean`. A member this generator version does not compute is
//! absent ([`Modelled::NotModelled`]), and a reader takes it as not modelled, never as none. The
//! members whose tasks land later (P14.T24.c, T24.e, T51.d, T53.d and T55.b) are absent until
//! they do, and each task may reshape its own member's type while no build has sent a value of it;
//! once a build has, a change bumps [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION). The
//! condensates (P14.T51.a) and the crust (P14.T51.c) are sent from P14.T54.a's batch, so their
//! shapes are now fixed.
//!
//! # Closed classes
//!
//! Each closed enum here is a physics class with every variant the plan's drafted tasks name, so
//! that no later task adds one: a variant no rule produces yet is listed and documented as such
//! (`steam_envelope`, `tenuous`, `heat_pipe`, `episodic`, `ice_shell`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::modelled::Modelled;
use crate::surface::ClimateModelDto;

/// The state of a body's surface (plan 14's `SurfaceState`, P14.T13.c), less the gas envelope,
/// which has no surface section: its air is the envelope section's.
///
/// `RunawayGreenhouse`, `Temperate` and `Snowball` are the body's insolation class against its
/// hosts' habitable zone (P14.T12.b's limits), not its present climate: read the mean surface
/// temperature, the ice and ocean fractions and the condensates for that, as the climate regime
/// does (decision-p14-t54a-states). P14.T53.b makes the state follow the present climate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SurfaceStateDto {
    /// Molten rock at the surface: a young terrestrial planet, or one hotter than the silicate
    /// solidus.
    MagmaOcean,
    /// A surface pressure below the airless limit: bare rock or ice.
    Airless,
    /// Inside the runaway-greenhouse limit at the hosts' largest past luminosity when water was
    /// present: the oceans lost, a thick carbon-dioxide air, as on Venus. About an evolved host its
    /// present climate can be cold (decision-pre-23-review item 33).
    RunawayGreenhouse,
    /// Between the runaway limit, at the hosts' largest past luminosity, and the maximum-greenhouse
    /// limit now, as on Earth and Mars. A world here can be ice-covered, on the cold side of the
    /// ice-albedo bistability (decision-pre-23-review item 26).
    Temperate,
    /// Beyond the maximum-greenhouse limit now, where a carbon-dioxide and water greenhouse cannot
    /// hold the surface above freezing (Kopparapu et al. 2013), as on Titan. Until P14.T53.a's
    /// carbon-dioxide form, a dry carbon-dioxide world here can be published above melting with no
    /// ice (P14.T13.c's finding "No maximum-greenhouse ceiling"; decision-p14-t51a-partition §1).
    Snowball,
    /// A steam envelope over a magma ocean (P14.T53.b): produced by no rule until that task.
    SteamEnvelope,
    /// A thin or transient air held above the airless floor, as Pluto's (P14.T53.c): produced by
    /// no rule until that task.
    Tenuous,
}

/// What a surface is made of, rock or ice, by the body's water fraction (plan 14's
/// `SurfaceMaterial`, P14.T13.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SurfaceMaterialDto {
    /// Rock and dust.
    Rock,
    /// Ice: a body of at least 10% water by mass.
    Ice,
}

/// The class of the X-ray and extreme-ultraviolet flux a body's hosts give it, a decade of the
/// present Earth's each (plan 14's `RadiationClass`, P14.T24.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RadiationClassDto {
    /// No X-ray or ultraviolet light: a body with no luminous host.
    Dark,
    /// Below 10 times the present Earth's flux.
    Quiet,
    /// 10 to 100 times.
    Active,
    /// 100 to 1,000 times.
    Intense,
    /// 1,000 times or more.
    Extreme,
}

/// How much of a climate's heat its air moves from day to night (plan 14's `ThermalRegime`,
/// P14.T48.d, after Koll 2022's redistribution factor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ThermalRegimeDto {
    /// No air, an air collapsed onto its night side, or one too thin to move heat: each point
    /// near its own radiative equilibrium.
    AirlessLike,
    /// Between the two.
    Transitional,
    /// Efficient redistribution.
    Efficient,
}

/// What drives a climate's cycle (plan 14's `Forcing`, P14.T48.d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ClimateForcingDto {
    /// A fast rotator's seasons.
    Seasonal,
    /// A fast rotator at an obliquity of 53.9–126.1° that came from a colder state, with an
    /// equatorial ice belt.
    IceBelt,
    /// A slow rotator, its circulation converging on the substellar point.
    SlowRotator,
    /// A world locked 1:1 to its seasonal host, in tidally locked coordinates.
    Locked,
}

/// A body's tectonic regime, how its lithosphere carries its heat (plan 14's `TectonicRegime`,
/// P14.T24.b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TectonicRegimeDto {
    /// Plates in relative motion, as on Earth.
    MobileLid,
    /// One lid, as on Mars, the Moon and Mercury.
    StagnantLid,
    /// Heat lost through volcanic conduits, as on Io: reserved, produced by no rule yet.
    HeatPipe,
    /// A lid that overturns in episodes: reserved, produced by no rule yet.
    Episodic,
    /// An ice shell over an ocean, as on Europa: reserved, produced by no rule yet.
    IceShell,
}

/// A body's global intrinsic magnetic field, from a dynamo (plan 14's `MagneticField`, P14.T24.b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MagneticFieldClassDto {
    /// No dynamo: remanent or induced fields only.
    None,
    /// A dynamo whose surface dipole is under about 1 μT, as Mercury's.
    Weak,
    /// A dipolar dynamo of order 10 μT or more at the surface, as Earth's.
    Strong,
}

/// What a subsurface ocean rests on (plan 14's `OceanFloor`, P14.T24.b and P14.T51.g).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OceanFloorDto {
    /// The rock core or mantle.
    Rock,
    /// Ice III.
    IceIii,
    /// Ice V.
    IceV,
    /// Ice VI.
    IceVi,
    /// Ice VII.
    IceVii,
}

/// A crater target's scaling class in the gravity regime (`hyperion-surface`'s `TargetClass`;
/// Holsapple 1993 as Singer et al. 2019 apply it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CraterTargetClassDto {
    /// Rock and ice alike: μ 0.55, K₁ 0.095.
    NonPorous,
    /// Dry sand or regolith, or 30%-porous ice: μ 0.41, K₁ 0.132.
    Porous,
}

/// The phase a substance holds where it is (plan 14's `Phase`, P14.T49.a), as an aerosol mode and
/// a surface condensate state it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SubstancePhaseDto {
    /// An ice, a frost or a crystal.
    Solid,
    /// A liquid.
    Liquid,
    /// A gas: never an aerosol mode's or a condensate's phase.
    Vapour,
    /// At or above both the critical temperature and pressure, with no interface, as Venus's
    /// carbon dioxide: never an aerosol mode's.
    Supercritical,
}

/// An aerosol particle's shape class, which with its material and phase fixes its optics (plan
/// 14, P14.T24.c; rendering plan R08's Design note 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AerosolShapeDto {
    /// A sphere: liquid water, sulphuric acid and liquid methane, tholin and soot.
    Sphere,
    /// A non-spherical mineral: mineral dust and the silicates.
    NonSphericalMineral,
    /// A crystal: the ices.
    Crystal,
    /// A fractal aggregate of spherical monomers, as Titan's haze.
    Aggregate,
}

/// Which crust lies at a surface (plan 14's `LidCrust`, P14.T51.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CrustLidDto {
    /// The primary crust, unresurfaced since it formed.
    Primary,
    /// The secondary crust.
    Secondary,
    /// The undifferentiated body's own material.
    Bulk,
}

/// A body's redox class, from its mantle's oxygen fugacity (plan 14's `RedoxClass`, P14.T51.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RedoxClassDto {
    /// Reduced, as Mercury and the enstatite chondrites.
    Reduced,
    /// Between the two.
    Intermediate,
    /// Oxidised.
    Oxidised,
}

/// One gas of a mixture and its share by number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GasShareDto {
    /// The gas: a key of the substance registry.
    pub species: String,
    /// Its mole fraction, in `[0, 1]`.
    pub mole_fraction: f64,
}

/// One substance of a surface's cover and the share of the surface it covers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SubstanceAreaDto {
    /// The substance: a key of the substance registry.
    pub substance: String,
    /// The share of the body's surface it covers, in `[0, 1]`.
    pub area_fraction: f64,
}

/// A body's climate regime, the classifier of the rendering brainstorm's open question 9 (plan
/// 14's `ClimateRegime`, P14.T48.d): its three fields and the coarse model it names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClimateRegimeDto {
    /// How much of the climate's heat the air moves.
    pub thermal: ThermalRegimeDto,
    /// What drives the climate's cycle.
    pub forcing: ClimateForcingDto,
    /// The climate's condensable, a key of the substance registry: of the condensables whose phase
    /// changes inside the climate's range, the one with the largest partial pressure in the air.
    /// `null` for a world with none, as Venus.
    pub condensable: Option<String>,
    /// The coarse model the regime names, which rendering plan R09's climate step runs.
    pub model: ClimateModelDto,
}

/// An ocean under a body's surface (plan 14's `SubsurfaceOcean`, P14.T24.b and P14.T51.g).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SubsurfaceOceanDto {
    /// The ice shell over it, m, not negative.
    pub shell_thickness_m: f64,
    /// Its liquid's depth, from the shell's base to its floor, m, positive.
    pub thickness_m: f64,
    /// What it rests on.
    pub floor: OceanFloorDto,
    /// Its liquid: a key of the substance registry (`H2O` for every rule so far).
    pub liquid: String,
    /// The mass fraction of solutes in its liquid, in `[0, 1]`.
    pub solute_mass_fraction: f64,
    /// Its temperature at the ice shell's base, where the shell meets it, K, positive: the
    /// liquid's melting point there (273 K for every rule so far).
    pub base_temperature_k: f64,
}

/// What screens a body's small impactors before they crater the ground (`hyperion-surface`'s
/// `Screening`; rendering plan R09's Design note 12). Tagged by `type`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum CraterScreeningDto {
    /// Nothing: an airless body.
    None,
    /// An atmosphere, by its column mass.
    Atmosphere {
        /// The surface pressure over the surface gravity, kg m⁻², not negative: about 10,300 on
        /// Earth.
        column_mass_kg_m2: f64,
    },
    /// A crater cutoff diameter given directly.
    Cutoff {
        /// The diameter below which no crater forms, m, positive.
        diameter_m: f64,
    },
}

/// A body's crater contract with rendering plan R09 (plan 14, P14.T48.c with T48.f's impactors;
/// `hyperion-surface`'s `CraterParams`), from which the shared crater density derives the rest.
///
/// The surface gravity the contract reads is the bulk section's `surface_gravity_m_s2`, not
/// repeated here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CraterContractDto {
    /// The lunar-equivalent cumulative crater density above 1 km, N☾(>1 km), per km², not
    /// negative: the Moon's density for the body's projectile flux, the lunar chronology at the
    /// surface age times the impactors' rate (science-r09-craters.md F1).
    pub lunar_equivalent_n_1km_per_km2: f64,
    /// What screens the small impactors.
    pub screening: CraterScreeningDto,
    /// The projectiles' bulk density, kg m⁻³, positive: 3,000 from a rocky belt, 500 from an icy
    /// one or the comets.
    pub projectile_density_kg_m3: f64,
    /// The target factor `k_target`, dimensionless and positive: 1 for rock, 0.12 for an ice-rich
    /// crust; it scales the simple-to-complex transition diameter.
    pub k_target: f64,
    /// The target's scaling class.
    pub target_class: CraterTargetClassDto,
    /// The target's bulk density, kg m⁻³, positive.
    pub target_density_kg_m3: f64,
    /// The age of the body's system, the chronology's clock, s, not negative.
    pub system_age_s: f64,
    /// The mean impact velocity, m s⁻¹, positive; `null` where the contract gives none (plan 14's
    /// impactors always give one).
    pub impact_velocity_m_s: Option<f64>,
}

/// The span over which a body's surface held liquid, with what rendering plan R09's erosion reads
/// of it (plan 14's `WetEpoch`, P14.T48.b), as times before the record's present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WetEpochDto {
    /// The liquid that flowed: a key of the substance registry (`H2O` for every rule so far).
    pub liquid: String,
    /// When the epoch began, s before the present, at least `end_ago_s`.
    pub start_ago_s: f64,
    /// When it ended, s before the present, not negative: zero for a world wet now.
    pub end_ago_s: f64,
    /// The time channel-forming flow ran at a typical point of the surface, in Earth-equivalent
    /// Julian years, not negative: R09's stream-power solver's t.
    pub effective_flow_yr: f64,
    /// The epoch's mean annual runoff, m per Julian year, not negative.
    pub runoff_m_per_yr: f64,
    /// The liquid inventory then, its mass over the body's whole surface, kg m⁻², not negative.
    pub paleo_inventory_kg_m2: f64,
}

/// One node of a temperature profile.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProfileNodeDto {
    /// The pressure, Pa, positive.
    pub pressure_pa: f64,
    /// The temperature there, K, positive.
    pub temperature_k: f64,
}

/// The vertical structure of a surface's air (plan 14, P14.T24.e's `RadiativeConvective`): a scaled
/// adiabat from the surface to an isothermal skin, T(p) = max(`T_ad`(p), `T_skin`), with
/// dln `T_ad` ÷ dln p = α R ÷ `c_p`(T) (Robinson and Catling 2012, Astrophys. J. 757, 104).
///
/// Its surface temperature and pressure are the section's `mean_surface_temperature_k` and
/// `surface_pressure_pa`, sent once. Absent from the wire until P14.T24.e, which may reshape it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VerticalStructureDto {
    /// α, the factor on the dry adiabat's R ÷ `c_p`, dimensionless and positive: 0.6 for water, 0.77
    /// for methane, the dry 0.88 otherwise (plan 14, P14.T24.e).
    pub alpha: f64,
    /// The isothermal skin's temperature, `2^(−1/4) T_eff`, K, positive.
    pub skin_temperature_k: f64,
    /// The adiabat's nodes from the surface to the tropopause, evenly in ln p, the pressures
    /// descending; the temperature between two nodes follows the power law between them.
    pub adiabat: Vec<ProfileNodeDto>,
}

/// An aerosol mode's size distribution, by its volume-to-area radius r = 3V ÷ 4A, A the
/// orientation-averaged projected area (plan 14, P14.T24.c; R08's science-r08-mmf.md §3). Tagged by
/// `type`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum SizeDistributionDto {
    /// Hansen and Travis 1974's gamma distribution.
    Gamma {
        /// The effective radius `r_eff` = (3 ÷ 4)⟨V⟩ ÷ ⟨A⟩, m, positive.
        effective_radius_m: f64,
        /// The effective variance `v_eff`, dimensionless and positive.
        effective_variance: f64,
    },
    /// Hansen and Travis 1974's modified log-normal distribution (their Eq. 2.60), by its
    /// effective radius and variance.
    LogNormal {
        /// The effective radius, m, positive.
        effective_radius_m: f64,
        /// The effective variance, dimensionless and positive.
        effective_variance: f64,
    },
    /// A fractal aggregate of spherical monomers (Tazaki and Tanaka 2018's model, validated to
    /// a fractal dimension of 2.5).
    Aggregate {
        /// The monomer's radius a₀, m, positive.
        monomer_radius_m: f64,
        /// The number of monomers N, at least 1.
        monomer_count: f64,
        /// The fractal dimension `D_f`, in `[1, 2.5]`.
        fractal_dimension: f64,
        /// The fractal prefactor `k_f`, positive: (5/3)^(`D_f`/2) unless the task states another.
        fractal_prefactor: f64,
    },
}

/// One aerosol mode: a cloud's, a haze's or a dust layer's particles of one material and phase
/// (plan 14, P14.T24.c).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AerosolModeDto {
    /// Its material: a key of the substance registry, one row for every phase of its substance.
    pub material: String,
    /// Its phase: `solid` or `liquid`, never `vapour` or `supercritical`.
    pub phase: SubstancePhaseDto,
    /// The deck it belongs to, by the deck's condensate key, so that a deck of several modes is
    /// one deck to its readers; `null` for a mode of no deck, as dust or a haze.
    pub deck: Option<String>,
    /// Its shape class.
    pub shape: AerosolShapeDto,
    /// Its size distribution.
    pub size_distribution: SizeDistributionDto,
    /// Its column mass, kg m⁻², not negative.
    pub column_mass_kg_m2: f64,
    /// Its column optical depth at 550 nm, dimensionless and not negative.
    pub tau_550: f64,
    /// The pressure of its base, Pa, positive and at least `top_pa`.
    pub base_pa: f64,
    /// The pressure of its top, Pa, positive.
    pub top_pa: f64,
    /// The exponent f of its mixing ratio, which goes as (p ÷ `p_base`)^f between its base and
    /// top: its deck's `f_sed` (Ackerman and Marley 2001), or 0 for a well-mixed layer; not
    /// negative.
    pub f_sed: f64,
}

/// A condensate deck: the condensate whose modes it holds, and its vapour above it (plan 14,
/// P14.T24.c).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CondensateDeckDto {
    /// The condensate: a key of the substance registry, which its modes' `deck` names.
    pub condensate: String,
    /// The vapour's mole fraction above the deck, the cold trap's, in `[0, 1]`.
    pub vapour_mole_fraction_above: f64,
}

/// One node of an absorber's profile.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MixingRatioNodeDto {
    /// The pressure, Pa, positive.
    pub pressure_pa: f64,
    /// The mixing ratio there relative to the profile's peak, in `[0, 1]`.
    pub relative_mixing_ratio: f64,
}

/// An absorbing gas that is not one of the section's well-mixed `gases`, with its column and
/// layer (plan 14, P14.T24.c): ozone, and sulphur dioxide by P14.T24.g.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AbsorberDto {
    /// The gas: a key of the substance registry.
    pub species: String,
    /// Its column, molecules m⁻² (not an area; plan 14's name), not negative.
    pub column_m2: f64,
    /// The pressure of its layer's base, Pa, positive and at least `top_pa`.
    pub base_pa: f64,
    /// The pressure of its layer's top, Pa, positive.
    pub top_pa: f64,
    /// Its profile of nodes, the pressures descending from `base_pa` to `top_pa`, its mixing ratio
    /// linear in ln x against ln p between them; `null` for a layer of constant mixing ratio.
    pub profile: Option<Vec<MixingRatioNodeDto>>,
}

/// A surface air's aerosol and absorber inventory (plan 14, P14.T24.c with T24.g). Absent from the
/// wire until P14.T24.c, which may reshape it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AerosolInventoryDto {
    /// The modes, every cloud's, haze's and dust layer's.
    pub modes: Vec<AerosolModeDto>,
    /// The condensate decks, which the modes name.
    pub decks: Vec<CondensateDeckDto>,
    /// The absorbers, in registry order.
    pub absorbers: Vec<AbsorberDto>,
}

/// One condensate at a surface (plan 14, P14.T51.a's partition): a substance in one phase, with
/// its share of the substance's reservoir at the surface, as a mass and as a layer, the area it
/// covers, the part of that area in cold traps and whether it is seasonal. Sent from P14.T54.a's
/// batch (decision-p14-t54a-water-consistency §5).
///
/// On an ice-bulk body whose lid is `H2O` (or not known) the `H2O` solid entry is the crust's
/// ice-held frozen water, all of it (shell and deeper ice), at the cover's ice area, until
/// P14.T53.c divides it; under any other lid (`cometary`, a chondrite, `phyllosilicate`, a
/// silicate) the crust's water stays in the bulk, unlisted, until P14.T53.c divides it.
/// P14.T24.b's surface water, the water free to pool, is P14.T13.b's (decision-p14-t54a-ganymede,
/// C1 follow-up §C1.4, as §C3.1 amended it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SurfaceCondensateDto {
    /// The substance: a key of the substance registry, for a liquid the solution it lies as
    /// (`ch4_c2h6_n2` for a methane sea under nitrogen).
    pub substance: String,
    /// Its phase where it lies: `solid`, `liquid` or `supercritical`, never `vapour`.
    pub phase: SubstancePhaseDto,
    /// Its condensed mass in this phase over the body's whole surface, kg m⁻²: the list's sort
    /// key, and its amount where its layer is absent; 0 for a supercritical fluid. On a body whose
    /// water froze, as `reservoir_gel_m`. Finite and not negative, and always sent.
    pub reservoir_kg_m2: f64,
    /// Its condensed reservoir as a global equivalent layer, m, not negative: its mass over the
    /// whole surface over its grain density in its phase at the temperature it lies at, and 0 for a
    /// supercritical fluid. On a body whose water froze, the whole frozen water the partition
    /// holds, shell, any ocean under a `Snowball`'s ice and deeper ice included, until P14.T53.c
    /// divides it between the condensate, the crust and the shell (decision-pre-23-review item
    /// 12).
    ///
    /// Absent where the registry has not read the phase's density (argon's ice and oxygen's β ice
    /// among them), so that a reader takes the layer as not modelled, never as none. Optional on
    /// the wire, as a ring's [`normal`](crate::RingDto::normal) is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reservoir_gel_m: Option<f64>,
    /// The share of the surface it covers, in `[0, 1]`: a liquid's the share of the body's basins
    /// its layer fills (water's alone, the other liquids' together, divided by volume), so none
    /// where its reservoir lies whole as ice; a solid's the climate's cold area below its edge,
    /// at most its mass spread at 1.93 km of its density (but water on an ice-rich crust, which
    /// is the crust's own ice); 0 for a supercritical fluid, which is the air at the surface and
    /// the only entry with no mass. Entries may overlap: an ice over a sea, and ices in the same
    /// cold zones (decision-p14-t54a-water-consistency §4, W4–W5).
    pub area_fraction: f64,
    /// The part of `area_fraction` in cold traps (P14.T51.f), in `[0, area_fraction]`. Absent
    /// from a server whose generator does not model cold traps, every server before P14.T54.b's
    /// bump, which a reader takes as not modelled, never as none; from it, sent on every entry, 0
    /// where none of the deposit lies in a cold trap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trapped_area_fraction: Option<f64>,
    /// Whether it is seasonal: the warmest zone takes this phase off the surface, melting or
    /// subliming a solid and boiling a liquid.
    pub seasonal: bool,
}

/// A body's crust by substance (plan 14, P14.T51.c): its crusts, buried or not, the one at the
/// surface, its provinces and its redox. Sent from P14.T54.a's batch, which fixed its shape
/// (decision-bump-plan-after-22, "Contracts that freeze at 23"). The thermal history that decided
/// the crusts, and the peak central temperature it read, stay on the generator's record and are
/// not sent.
///
/// Either `bulk` is present alone, or at least one of `primary` and `secondary` is, and `lid`
/// names a present member. No `tertiary` is present without a `secondary`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrustCompositionDto {
    /// The primary crust, a key of the substance registry; `null` for none.
    pub primary: Option<String>,
    /// The secondary crust, a key; `null` for none.
    pub secondary: Option<String>,
    /// The tertiary crust, a key; `null` for none.
    pub tertiary: Option<String>,
    /// An undifferentiated body's own material, a key; `null` for a body with a crust.
    pub bulk: Option<String>,
    /// Which of them lies at the surface.
    pub lid: CrustLidDto,
    /// The lithology of the lid's volcanic provinces, a key.
    pub provinces: String,
    /// The redox class, the banding of `mantle_redox_iw`.
    pub redox: RedoxClassDto,
    /// The mantle's oxygen fugacity relative to the iron–wüstite buffer, ΔIW, log₁₀ units, finite
    /// and of either sign: below 0 more reduced than the buffer.
    pub mantle_redox_iw: f64,
    /// The share of the surface above a silicate lid's solidus, in `[0, 1]`; 0 for an ice lid,
    /// whose liquid is a condensate.
    pub melt_area_fraction: f64,
}

/// A surface's modifiers (plan 14, P14.T51.d; decision-p14-t51d-weathering §2). Absent from the
/// wire until P14.T54.b fills it. P14.T51.d gave it this shape, which may change until a build
/// sends a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SurfaceModifiersDto {
    /// The space-weathering maturity, dimensionless in `[0, 1]`: the background regolith's
    /// nanophase-iron content as a fraction of the Moon's mature background's, per unit of its
    /// non-metallic iron as `FeO`, 0 for fresh ground and 1 for the Moon's mature background.
    pub maturity: f64,
    /// The space-weathering rate, dimensionless, finite and not negative, with no upper bound: the
    /// body's surface-reaching weathering dose rate relative to the Moon's present one, so that the
    /// Moon's is 1.
    pub weathering_rate: f64,
    /// The organic cover: the covered fraction of the ground that can take one, in `[0, 1]`.
    pub organic_cover: f64,
    /// The resurfacing rate from volcanism or tidal heat, m per Julian year, not negative.
    pub resurfacing_m_per_yr: f64,
    /// The volcanic deposits an Io is resurfaced with, SO₂ frost and sulphur, each a key of the
    /// substance registry with its share of the surface in `[0, 1]`; empty for every other body.
    /// With the weathering deposits, together at most 1.
    pub volcanic_deposits: Vec<SubstanceAreaDto>,
    /// The weathering deposits a surface carries (P14.T51.j): an oxidised `basalt_fe` surface's
    /// `ferric` over the dusty share of its dry, ice-free land, each a key of the substance
    /// registry with its share of the surface in `[0, 1]`, at most one today; empty for every other
    /// body. With the volcanic deposits, together at most 1.
    pub weathering_deposits: Vec<SubstanceAreaDto>,
}

/// One net surface flux of a biosphere.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BiosphereFluxDto {
    /// The gas: a key of the substance registry.
    pub species: String,
    /// Its net surface flux in units of Earth's present biotic flux of it, finite and of either
    /// sign: negative for a net sink.
    pub flux_earth: f64,
}

/// A biosphere's fluxes (plan 14, P14.T53.d). Absent from the wire until a life plan exists: every
/// generated body is abiotic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BiosphereDto {
    /// The net surface fluxes, by species.
    pub fluxes: Vec<BiosphereFluxDto>,
}

/// The vapour a surface's melt holds over it: the rock-forming species the melt evaporates,
/// over a volatile-free melt chiefly Na, O₂, O and `SiO` (Schaefer and Fegley 2009, Astrophys. J.
/// 703, L113), which plan 14's P14.T55.b models as silicate-vapour atmospheres.
///
/// Its members are P14.T55.b's, which no task has built, so the type has no value yet and
/// every server sends [`BodySurfaceDto::silicate_vapour`] absent. TypeScript sees it as
/// `never`. P14.T55.b gives it its members, each quantity's name carrying its unit as here, and
/// may reshape it while no build has sent a value (plan 04's Design note 15). It derives only
/// what a vapour of figures and registry keys can keep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SilicateVapourDto {}

/// A body's surface section: its surface state and material, its surface conditions and climate
/// regime, its global figures and the members whose tasks land later (design note 16; plan 14,
/// P14.T48.e and P14.T35.e).
///
/// The section is `ok` for a present body with a solid surface whose spin and derivation this
/// generator version computes, `not_applicable` for a body in the gas-envelope state, whose air is
/// its [`BodyEnvelopeDto`], and for a ring, a belt or the halo. Every quantity's name carries its
/// unit. The module documentation says what `null` and an absent member mean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodySurfaceDto {
    /// The surface state.
    pub state: SurfaceStateDto,
    /// The surface material.
    pub material: SurfaceMaterialDto,
    /// The mean surface temperature, K, positive (P14.T24.a): the temperature rendering plan R08's
    /// profile starts from. It is the surface's emission mean, (⟨σT⁴⟩ ÷ σ)^¼, about 1.3 K above
    /// the area mean for Earth-like contrasts (decision-p14-t57a-fits §2.2).
    pub mean_surface_temperature_k: f64,
    /// The annual-mean equator–pole contrast T(0°) − T(90°), K: negative where the poles are the
    /// warmer, as at an obliquity of 53.9° to 126.1°.
    pub equator_pole_contrast_k: f64,
    /// The day–night contrast, the dayside's less the nightside's temperature, K, not negative:
    /// about the substellar axis for a world locked 1:1 to its seasonal host.
    pub day_night_contrast_k: f64,
    /// The mean surface pressure of the modelled air, the volatiles', Pa, not negative: zero for an
    /// airless body, and for a surface holding melt whose volatiles are gone.
    ///
    /// It does not include the melt's own vapour ([`silicate_vapour`](Self::silicate_vapour)).
    pub surface_pressure_pa: f64,
    /// The gases at the surface by number, largest first with ties by registry order, their mole
    /// fractions summing to 1 to 10⁻⁹; empty for an airless body, or for a surface holding melt
    /// with no volatile air, which does not deny the melt's own vapour (`silicate_vapour`).
    pub gases: Vec<GasShareDto>,
    /// The gases' mean molar mass μ = Σ xᵢ μᵢ, g mol⁻¹, positive; `null` with an empty `gases`: an
    /// airless body, or a surface holding melt with no volatile air.
    pub mean_molar_mass_g_per_mol: Option<f64>,
    /// The gases' mixed ideal-gas `c_p` ÷ R, Σ xᵢ (`c_p,ᵢ` ÷ R), dimensionless and positive, at
    /// the mean surface temperature (the registry's statistical sum over each gas's molecular
    /// constants, P14.T49.b's); `null` with an empty `gases` (an airless body, or a surface
    /// holding melt with no volatile air), and at a mean surface temperature not above 0 K or not
    /// finite, where the statistical sum has no `c_p`.
    pub heat_capacity_over_r: Option<f64>,
    /// The flux of the hosts' light at the body, orbit-averaged, W m⁻², not negative.
    pub stellar_flux_w_m2: f64,
    /// The class of the hosts' X-ray and ultraviolet flux at the body.
    pub radiation_class: RadiationClassDto,
    /// Whether water can be liquid at the surface pressure somewhere in the climate's range of
    /// temperature: a condition of the surface, whether or not the body holds water.
    pub liquid_water: bool,
    /// The climate regime (P14.T48.d).
    pub climate_regime: ClimateRegimeDto,
    /// The ocean fraction, the share of the surface under liquid, in `[0, 1]`: the sum of
    /// `surface_liquids`, the cover by substance, from `condensates` by its rule, held at 1. With
    /// `ice_fraction` it may exceed 1, since an ice may lie over an ocean.
    pub ocean_fraction: f64,
    /// The ice fraction, the share of the surface under perennial ice, in `[0, 1]`: the sum of
    /// `surface_ices`, the cover by substance, from `condensates` by its rule, held at 1. An ice
    /// may lie over an ocean.
    pub ice_fraction: f64,
    /// The cloud fraction, in `[0, 1]`.
    pub cloud_fraction: f64,
    /// The liquids at the surface by substance: the cover by substance, from `condensates` by its
    /// rule, summing to at most 1 but for rounding: a list divided by its total to fit the
    /// surface may sum back to an ulp over it, where `ocean_fraction` is 1.
    pub surface_liquids: Vec<SubstanceAreaDto>,
    /// The ices at the surface by substance: the cover by substance, from `condensates` by its
    /// rule, summing to at most 1 but for rounding: a list divided by its total to fit the surface
    /// may sum back to an ulp over it, where `ice_fraction` is 1.
    pub surface_ices: Vec<SubstanceAreaDto>,
    /// The hypsometric standard deviation `σ_h`, m, not negative, relative to the reference
    /// equipotential (P14.T48.a).
    pub sigma_h_m: f64,
    /// The continental fraction, in `[0, 1]` (P14.T48.a).
    pub continental_fraction: f64,
    /// The heat flow at the surface, W m⁻², not negative: radiogenic, residual and tidal.
    pub heat_flow_w_m2: f64,
    /// The tectonic regime.
    pub tectonic_regime: TectonicRegimeDto,
    /// The volcanism level V, dimensionless, not negative: the melt the surface's life has erupted
    /// times its plume vigour, over the Solar System table's Mars's, whose V is 1 (P14.T48.a).
    pub volcanism_level: f64,
    /// The present volcanic outgassing, relative to Earth's present rate, not negative (P14.T24.g's
    /// O, 1 on the generated Earth). It is not the volcanism level, which builds relief over the
    /// surface's history.
    pub outgassing_earth: f64,
    /// The global intrinsic magnetic field's class.
    pub magnetic_field_class: MagneticFieldClassDto,
    /// The surface age, the time since the last global resurfacing, s, not negative and at most the
    /// system's age.
    pub surface_age_s: f64,
    /// The ocean under the surface: `null` for a body computed to hold none, and absent for a body
    /// no rule computes, every body but a regular moon until P14.T51.g.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<SubsurfaceOceanDto>>", optional)]
    pub subsurface_ocean: Modelled<SubsurfaceOceanDto>,
    /// The crater contract (P14.T48.c, with T48.f's impactors).
    pub craters: CraterContractDto,
    /// The wet epoch (P14.T48.b); `null` for a world never wet.
    pub wet_epoch: Option<WetEpochDto>,
    /// The vertical structure (P14.T24.e); absent until that task.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<VerticalStructureDto>>", optional)]
    pub vertical_structure: Modelled<VerticalStructureDto>,
    /// The aerosol and absorber inventory (P14.T24.c); absent until that task.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<AerosolInventoryDto>>", optional)]
    pub aerosols: Modelled<AerosolInventoryDto>,
    /// The condensates at the surface, at most eight, one for each substance and phase (P14.T51.a):
    /// by reservoir mass, largest first; ties by registry order, then solid, liquid, supercritical;
    /// the smallest dropped beyond eight. An empty list where the surface holds none, never `null`.
    /// Sent from P14.T54.a's batch, and absent from a server whose generator does not model them.
    ///
    /// `surface_liquids` and `surface_ices` are the cover they give
    /// (decision-p14-t54a-water-consistency §4, W1–W3): every liquid and every solid entry is in
    /// its list, in the entries' order, at its own area, divided by the list's total where that
    /// total exceeds 1. At most one `H2O` item is appended to each without an entry: in
    /// `surface_ices`, P14.T24.b's water ice where no `H2O` solid is listed (an ice-rich crust's
    /// own ice or a `Snowball`'s cover, whose water the partition does not hold); in
    /// `surface_liquids`, the ocean under a `Snowball`'s ice where no `H2O` liquid is listed, its
    /// water inside the `H2O` solid. Nothing else; P14.T53.c ends both.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<Vec<SurfaceCondensateDto>>>", optional)]
    pub condensates: Modelled<Vec<SurfaceCondensateDto>>,
    /// The crust by substance (P14.T51.c). Sent from P14.T54.a's batch for every surface whose
    /// generator computes it; absent from a server whose generator does not model it, and for a
    /// body whose record holds none.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<CrustCompositionDto>>", optional)]
    pub crust: Modelled<CrustCompositionDto>,
    /// The surface's modifiers (P14.T51.d); absent until that task.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<SurfaceModifiersDto>>", optional)]
    pub modifiers: Modelled<SurfaceModifiersDto>,
    /// The biosphere's fluxes (P14.T53.d); absent until a life plan exists.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<BiosphereDto>>", optional)]
    pub biosphere: Modelled<BiosphereDto>,
    /// The melt's own vapour, its silicate vapour (P14.T55.b); absent until that task, from
    /// every server.
    ///
    /// It is not part of the volatile air that `surface_pressure_pa` and `gases` hold: the
    /// melt's temperature sets it where melt lies, not a volatile inventory's weight over the
    /// globe. It applies where a surface holds melt, a fact on the wire: the state `magma_ocean`
    /// (and P14.T53.b's `steam_envelope`, where its surface section is the applicable one) and,
    /// from P14.T54.a, a `crust.melt_area_fraction` above 0. Elsewhere its absence says
    /// nothing. From P14.T55.b it is `null` on a surface holding no melt, and the vapour over
    /// melt (decision-backlog-2 §2.3).
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<SilicateVapourDto>>", optional)]
    pub silicate_vapour: Modelled<SilicateVapourDto>,
}

/// The parameters of an envelope's irradiated grey profile above its join to the adiabat: Guillot
/// 2010's (A&A 520, A27) Eq. 29, with τ = `κ_th` p ÷ g and g the bulk section's surface gravity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GuillotProfileDto {
    /// The mean thermal opacity `κ_th`, m² kg⁻¹, positive.
    pub thermal_opacity_m2_per_kg: f64,
    /// γ, the visible opacity over the thermal, `κ_v` ÷ `κ_th`, dimensionless and positive.
    pub opacity_ratio: f64,
    /// f, the redistribution of the irradiation over the planet, dimensionless and positive: ¼ for
    /// an average over the whole sphere.
    pub redistribution_factor: f64,
}

/// A gas-envelope body's visible atmosphere: its profile, composition and decks (plan 14,
/// P14.T24.d with T52.d; the simulation's `record::Envelope`).
///
/// The section is `not_modelled` for every gas-envelope body until P14.T24.d, which may give these
/// members their final shape before a build first sends one, and `not_applicable` for every other
/// body, whose surface section is the applicable one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyEnvelopeDto {
    /// The internal temperature `T_int`, K, not negative: 0 for a body with no internal heat.
    pub internal_temperature_k: f64,
    /// The irradiation temperature `T_irr`, K, not negative.
    pub irradiation_temperature_k: f64,
    /// The grey profile's parameters.
    pub profile: GuillotProfileDto,
    /// The pressure at which the profile joins the dry adiabat, where its gradient
    /// dln T ÷ dln p first exceeds R ÷ `c_p`, Pa, positive.
    pub join_pressure_pa: f64,
    /// The adiabat's nodes below the join, from `join_pressure_pa` to 100 bar, the pressures
    /// ascending; deeper, the temperature follows the last segment's exponent.
    pub adiabat: Vec<ProfileNodeDto>,
    /// The envelope's gases by number, largest first, their mole fractions summing to 1 to 10⁻⁹.
    pub composition: Vec<GasShareDto>,
    /// The gases' mean molar mass, g mol⁻¹, positive.
    pub mean_molar_mass_g_per_mol: f64,
    /// The gases' mixed `c_p` ÷ R at the 1-bar level's temperature, dimensionless and positive: for
    /// display, since no reader derives a lapse rate from it.
    pub heat_capacity_over_r: f64,
    /// The condensate decks, as aerosol modes.
    pub decks: Vec<AerosolModeDto>,
    /// The haze's optical depth at 550 nm, dimensionless and not negative.
    pub haze_tau_550: f64,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::testing::{assert_wire_form, assert_wire_strings};

    /// An Earth's surface, illustrative and of the generator's Earth's order: 1.01 bar of
    /// nitrogen with water, argon and carbon dioxide (no oxygen, which is biotic), a temperate
    /// mobile lid with a strong field, and every member of a later task absent.
    pub(crate) fn earth_surface() -> BodySurfaceDto {
        BodySurfaceDto {
            state: SurfaceStateDto::Temperate,
            material: SurfaceMaterialDto::Rock,
            mean_surface_temperature_k: 287.9,
            equator_pole_contrast_k: 41.5,
            day_night_contrast_k: 9.8,
            surface_pressure_pa: 101_325.0,
            gases: vec![
                GasShareDto {
                    species: "N2".to_owned(),
                    mole_fraction: 0.9805,
                },
                GasShareDto {
                    species: "H2O".to_owned(),
                    mole_fraction: 0.0102,
                },
                GasShareDto {
                    species: "Ar".to_owned(),
                    mole_fraction: 0.009,
                },
                GasShareDto {
                    species: "CO2".to_owned(),
                    mole_fraction: 0.0003,
                },
            ],
            mean_molar_mass_g_per_mol: Some(28.024_232_7),
            heat_capacity_over_r: Some(3.496_35),
            stellar_flux_w_m2: 1_361.0,
            radiation_class: RadiationClassDto::Quiet,
            liquid_water: true,
            climate_regime: ClimateRegimeDto {
                thermal: ThermalRegimeDto::Efficient,
                forcing: ClimateForcingDto::Seasonal,
                condensable: Some("H2O".to_owned()),
                model: ClimateModelDto::EnergyBalance,
            },
            ocean_fraction: 0.71,
            ice_fraction: 0.03,
            cloud_fraction: 0.67,
            surface_liquids: vec![SubstanceAreaDto {
                substance: "H2O".to_owned(),
                area_fraction: 0.71,
            }],
            surface_ices: vec![SubstanceAreaDto {
                substance: "H2O".to_owned(),
                area_fraction: 0.03,
            }],
            sigma_h_m: 2_510.0,
            continental_fraction: 0.405,
            heat_flow_w_m2: 0.0925,
            tectonic_regime: TectonicRegimeDto::MobileLid,
            volcanism_level: 0.85,
            outgassing_earth: 1.0,
            magnetic_field_class: MagneticFieldClassDto::Strong,
            surface_age_s: 2.025e15,
            subsurface_ocean: Modelled::NotModelled,
            craters: CraterContractDto {
                lunar_equivalent_n_1km_per_km2: 6.4e-5,
                screening: CraterScreeningDto::Atmosphere {
                    column_mass_kg_m2: 10_318.0,
                },
                projectile_density_kg_m3: 3_000.0,
                k_target: 1.0,
                target_class: CraterTargetClassDto::NonPorous,
                target_density_kg_m3: 2_550.0,
                system_age_s: 1.442e17,
                impact_velocity_m_s: Some(20_100.0),
            },
            wet_epoch: Some(WetEpochDto {
                liquid: "H2O".to_owned(),
                start_ago_s: 1.37e17,
                end_ago_s: 0.0,
                effective_flow_yr: 4.3e7,
                runoff_m_per_yr: 0.3,
                paleo_inventory_kg_m2: 2.7e6,
            }),
            vertical_structure: Modelled::NotModelled,
            aerosols: Modelled::NotModelled,
            condensates: Modelled::NotModelled,
            crust: Modelled::NotModelled,
            modifiers: Modelled::NotModelled,
            biosphere: Modelled::NotModelled,
            silicate_vapour: Modelled::NotModelled,
        }
    }

    pub(crate) fn earth_surface_json() -> Value {
        json!({
            "state": "temperate",
            "material": "rock",
            "mean_surface_temperature_k": 287.9,
            "equator_pole_contrast_k": 41.5,
            "day_night_contrast_k": 9.8,
            "surface_pressure_pa": 101_325.0,
            "gases": [
                { "species": "N2", "mole_fraction": 0.9805 },
                { "species": "H2O", "mole_fraction": 0.0102 },
                { "species": "Ar", "mole_fraction": 0.009 },
                { "species": "CO2", "mole_fraction": 0.0003 },
            ],
            "mean_molar_mass_g_per_mol": 28.024_232_7,
            "heat_capacity_over_r": 3.496_35,
            "stellar_flux_w_m2": 1_361.0,
            "radiation_class": "quiet",
            "liquid_water": true,
            "climate_regime": {
                "thermal": "efficient",
                "forcing": "seasonal",
                "condensable": "H2O",
                "model": "energy_balance",
            },
            "ocean_fraction": 0.71,
            "ice_fraction": 0.03,
            "cloud_fraction": 0.67,
            "surface_liquids": [{ "substance": "H2O", "area_fraction": 0.71 }],
            "surface_ices": [{ "substance": "H2O", "area_fraction": 0.03 }],
            "sigma_h_m": 2_510.0,
            "continental_fraction": 0.405,
            "heat_flow_w_m2": 0.0925,
            "tectonic_regime": "mobile_lid",
            "volcanism_level": 0.85,
            "outgassing_earth": 1.0,
            "magnetic_field_class": "strong",
            "surface_age_s": 2.025e15,
            "craters": {
                "lunar_equivalent_n_1km_per_km2": 6.4e-5,
                "screening": { "type": "atmosphere", "column_mass_kg_m2": 10_318.0 },
                "projectile_density_kg_m3": 3_000.0,
                "k_target": 1.0,
                "target_class": "non_porous",
                "target_density_kg_m3": 2_550.0,
                "system_age_s": 1.442e17,
                "impact_velocity_m_s": 20_100.0,
            },
            "wet_epoch": {
                "liquid": "H2O",
                "start_ago_s": 1.37e17,
                "end_ago_s": 0.0,
                "effective_flow_yr": 4.3e7,
                "runoff_m_per_yr": 0.3,
                "paleo_inventory_kg_m2": 2.7e6,
            },
        })
    }

    /// A Venus's surface, illustrative: the generator's 57.8 bar of carbon dioxide with nitrogen,
    /// no condensable, an isothermal slow rotator under a stagnant lid, never wet.
    pub(crate) fn venus_surface() -> BodySurfaceDto {
        BodySurfaceDto {
            state: SurfaceStateDto::RunawayGreenhouse,
            mean_surface_temperature_k: 735.0,
            equator_pole_contrast_k: 1.2,
            day_night_contrast_k: 0.6,
            surface_pressure_pa: 5.78e6,
            gases: vec![
                GasShareDto {
                    species: "CO2".to_owned(),
                    mole_fraction: 0.9835,
                },
                GasShareDto {
                    species: "N2".to_owned(),
                    mole_fraction: 0.0165,
                },
            ],
            mean_molar_mass_g_per_mol: Some(43.745_082_5),
            heat_capacity_over_r: Some(4.319_583_333_333_334),
            stellar_flux_w_m2: 2_601.0,
            liquid_water: false,
            climate_regime: ClimateRegimeDto {
                thermal: ThermalRegimeDto::Efficient,
                forcing: ClimateForcingDto::SlowRotator,
                condensable: None,
                model: ClimateModelDto::Isothermal,
            },
            ocean_fraction: 0.0,
            ice_fraction: 0.0,
            cloud_fraction: 1.0,
            surface_liquids: vec![SubstanceAreaDto {
                substance: "H2O".to_owned(),
                area_fraction: 0.0,
            }],
            surface_ices: vec![SubstanceAreaDto {
                substance: "H2O".to_owned(),
                area_fraction: 0.0,
            }],
            sigma_h_m: 1_120.0,
            continental_fraction: 0.0,
            heat_flow_w_m2: 0.049,
            tectonic_regime: TectonicRegimeDto::StagnantLid,
            volcanism_level: 1.2,
            outgassing_earth: 0.32,
            magnetic_field_class: MagneticFieldClassDto::None,
            surface_age_s: 2.4e16,
            craters: CraterContractDto {
                lunar_equivalent_n_1km_per_km2: 7.1e-4,
                screening: CraterScreeningDto::Atmosphere {
                    column_mass_kg_m2: 650_000.0,
                },
                ..earth_surface().craters
            },
            wet_epoch: None,
            ..earth_surface()
        }
    }

    pub(crate) fn venus_surface_json() -> Value {
        let mut wire = earth_surface_json();
        let fields = wire.as_object_mut().unwrap();
        for (key, value) in [
            ("state", json!("runaway_greenhouse")),
            ("mean_surface_temperature_k", json!(735.0)),
            ("equator_pole_contrast_k", json!(1.2)),
            ("day_night_contrast_k", json!(0.6)),
            ("surface_pressure_pa", json!(5.78e6)),
            (
                "gases",
                json!([
                    { "species": "CO2", "mole_fraction": 0.9835 },
                    { "species": "N2", "mole_fraction": 0.0165 },
                ]),
            ),
            ("mean_molar_mass_g_per_mol", json!(43.745_082_5)),
            ("heat_capacity_over_r", json!(4.319_583_333_333_334)),
            ("stellar_flux_w_m2", json!(2_601.0)),
            ("liquid_water", json!(false)),
            (
                "climate_regime",
                json!({
                    "thermal": "efficient",
                    "forcing": "slow_rotator",
                    "condensable": null,
                    "model": "isothermal",
                }),
            ),
            ("ocean_fraction", json!(0.0)),
            ("ice_fraction", json!(0.0)),
            ("cloud_fraction", json!(1.0)),
            (
                "surface_liquids",
                json!([{ "substance": "H2O", "area_fraction": 0.0 }]),
            ),
            (
                "surface_ices",
                json!([{ "substance": "H2O", "area_fraction": 0.0 }]),
            ),
            ("sigma_h_m", json!(1_120.0)),
            ("continental_fraction", json!(0.0)),
            ("heat_flow_w_m2", json!(0.049)),
            ("tectonic_regime", json!("stagnant_lid")),
            ("volcanism_level", json!(1.2)),
            ("outgassing_earth", json!(0.32)),
            ("magnetic_field_class", json!("none")),
            ("surface_age_s", json!(2.4e16)),
            ("wet_epoch", Value::Null),
        ] {
            fields.insert(key.to_owned(), value);
        }
        wire["craters"]["lunar_equivalent_n_1km_per_km2"] = json!(7.1e-4);
        wire["craters"]["screening"]["column_mass_kg_m2"] = json!(650_000.0);
        wire
    }

    /// A Jupiter's envelope, illustrative: Guillot 2010's fiducial opacities, the protosolar
    /// atmosphere depleted of helium (He/H₂ 0.157 and CH₄/H₂ 2.1 × 10⁻³ by number), the adiabat
    /// through 166.1 K at 1 bar, and an ammonia-ice deck at 0.45–0.7 bar.
    pub(crate) fn jupiter_envelope() -> BodyEnvelopeDto {
        BodyEnvelopeDto {
            internal_temperature_k: 99.0,
            irradiation_temperature_k: 155.4,
            profile: GuillotProfileDto {
                thermal_opacity_m2_per_kg: 1e-3,
                opacity_ratio: 0.4,
                redistribution_factor: 0.25,
            },
            join_pressure_pa: 5e4,
            adiabat: vec![
                ProfileNodeDto {
                    pressure_pa: 5e4,
                    temperature_k: 134.7,
                },
                ProfileNodeDto {
                    pressure_pa: 1e5,
                    temperature_k: 166.1,
                },
                ProfileNodeDto {
                    pressure_pa: 1e6,
                    temperature_k: 336.4,
                },
                ProfileNodeDto {
                    pressure_pa: 1e7,
                    temperature_k: 681.3,
                },
            ],
            composition: vec![
                GasShareDto {
                    species: "H2".to_owned(),
                    mole_fraction: 0.8627,
                },
                GasShareDto {
                    species: "He".to_owned(),
                    mole_fraction: 0.1355,
                },
                GasShareDto {
                    species: "CH4".to_owned(),
                    mole_fraction: 0.0018,
                },
            ],
            mean_molar_mass_g_per_mol: 2.310_487_1,
            heat_capacity_over_r: 3.3654,
            decks: vec![ammonia_deck()],
            haze_tau_550: 0.5,
        }
    }

    /// The ammonia-ice deck of [`jupiter_envelope`].
    pub(crate) fn ammonia_deck() -> AerosolModeDto {
        AerosolModeDto {
            material: "NH3".to_owned(),
            phase: SubstancePhaseDto::Solid,
            deck: Some("NH3".to_owned()),
            shape: AerosolShapeDto::Crystal,
            size_distribution: SizeDistributionDto::Gamma {
                effective_radius_m: 1e-6,
                effective_variance: 0.1,
            },
            column_mass_kg_m2: 0.012,
            tau_550: 8.0,
            base_pa: 7e4,
            top_pa: 4.5e4,
            f_sed: 2.0,
        }
    }

    pub(crate) fn ammonia_deck_json() -> Value {
        json!({
            "material": "NH3",
            "phase": "solid",
            "deck": "NH3",
            "shape": "crystal",
            "size_distribution": {
                "type": "gamma",
                "effective_radius_m": 1e-6,
                "effective_variance": 0.1,
            },
            "column_mass_kg_m2": 0.012,
            "tau_550": 8.0,
            "base_pa": 7e4,
            "top_pa": 4.5e4,
            "f_sed": 2.0,
        })
    }

    pub(crate) fn jupiter_envelope_json() -> Value {
        json!({
            "internal_temperature_k": 99.0,
            "irradiation_temperature_k": 155.4,
            "profile": {
                "thermal_opacity_m2_per_kg": 1e-3,
                "opacity_ratio": 0.4,
                "redistribution_factor": 0.25,
            },
            "join_pressure_pa": 5e4,
            "adiabat": [
                { "pressure_pa": 5e4, "temperature_k": 134.7 },
                { "pressure_pa": 1e5, "temperature_k": 166.1 },
                { "pressure_pa": 1e6, "temperature_k": 336.4 },
                { "pressure_pa": 1e7, "temperature_k": 681.3 },
            ],
            "composition": [
                { "species": "H2", "mole_fraction": 0.8627 },
                { "species": "He", "mole_fraction": 0.1355 },
                { "species": "CH4", "mole_fraction": 0.0018 },
            ],
            "mean_molar_mass_g_per_mol": 2.310_487_1,
            "heat_capacity_over_r": 3.3654,
            "decks": [ammonia_deck_json()],
            "haze_tau_550": 0.5,
        })
    }

    /// A warm sub-Neptune's envelope, illustrative: no internal heat stated, water and methane
    /// beside the hydrogen and helium, and no deck.
    pub(crate) fn sub_neptune_envelope() -> BodyEnvelopeDto {
        BodyEnvelopeDto {
            internal_temperature_k: 0.0,
            irradiation_temperature_k: 640.0,
            join_pressure_pa: 2e6,
            adiabat: vec![
                ProfileNodeDto {
                    pressure_pa: 2e6,
                    temperature_k: 1_020.0,
                },
                ProfileNodeDto {
                    pressure_pa: 1e7,
                    temperature_k: 1_660.0,
                },
            ],
            composition: vec![
                GasShareDto {
                    species: "H2".to_owned(),
                    mole_fraction: 0.8379,
                },
                GasShareDto {
                    species: "He".to_owned(),
                    mole_fraction: 0.1421,
                },
                GasShareDto {
                    species: "H2O".to_owned(),
                    mole_fraction: 0.012,
                },
                GasShareDto {
                    species: "CH4".to_owned(),
                    mole_fraction: 0.008,
                },
            ],
            mean_molar_mass_g_per_mol: 2.602_556_7,
            heat_capacity_over_r: 3.3679,
            decks: Vec::new(),
            haze_tau_550: 0.0,
            ..jupiter_envelope()
        }
    }

    pub(crate) fn sub_neptune_envelope_json() -> Value {
        let mut wire = jupiter_envelope_json();
        for (key, value) in [
            ("internal_temperature_k", json!(0.0)),
            ("irradiation_temperature_k", json!(640.0)),
            ("join_pressure_pa", json!(2e6)),
            (
                "adiabat",
                json!([
                    { "pressure_pa": 2e6, "temperature_k": 1_020.0 },
                    { "pressure_pa": 1e7, "temperature_k": 1_660.0 },
                ]),
            ),
            (
                "composition",
                json!([
                    { "species": "H2", "mole_fraction": 0.8379 },
                    { "species": "He", "mole_fraction": 0.1421 },
                    { "species": "H2O", "mole_fraction": 0.012 },
                    { "species": "CH4", "mole_fraction": 0.008 },
                ]),
            ),
            ("mean_molar_mass_g_per_mol", json!(2.602_556_7)),
            ("heat_capacity_over_r", json!(3.3679)),
            ("decks", json!([])),
            ("haze_tau_550", json!(0.0)),
        ] {
            wire[key] = value;
        }
        wire
    }

    /// A vertical structure, illustrative: Earth's moist α, its skin and three nodes.
    fn earth_structure() -> VerticalStructureDto {
        VerticalStructureDto {
            alpha: 0.6,
            skin_temperature_k: 214.5,
            adiabat: vec![
                ProfileNodeDto {
                    pressure_pa: 101_325.0,
                    temperature_k: 287.9,
                },
                ProfileNodeDto {
                    pressure_pa: 40_000.0,
                    temperature_k: 247.0,
                },
                ProfileNodeDto {
                    pressure_pa: 14_000.0,
                    temperature_k: 214.5,
                },
            ],
        }
    }

    fn earth_structure_json() -> Value {
        json!({
            "alpha": 0.6,
            "skin_temperature_k": 214.5,
            "adiabat": [
                { "pressure_pa": 101_325.0, "temperature_k": 287.9 },
                { "pressure_pa": 40_000.0, "temperature_k": 247.0 },
                { "pressure_pa": 14_000.0, "temperature_k": 214.5 },
            ],
        })
    }

    /// An aerosol inventory, illustrative: a water cloud of droplets and crystals, a Titan-like
    /// haze of aggregates, and ozone on a profile beside a sulphur-dioxide slab.
    fn inventory() -> AerosolInventoryDto {
        AerosolInventoryDto {
            modes: vec![
                AerosolModeDto {
                    material: "H2O".to_owned(),
                    phase: SubstancePhaseDto::Liquid,
                    deck: Some("H2O".to_owned()),
                    shape: AerosolShapeDto::Sphere,
                    size_distribution: SizeDistributionDto::LogNormal {
                        effective_radius_m: 1e-5,
                        effective_variance: 0.1,
                    },
                    column_mass_kg_m2: 0.05,
                    tau_550: 7.5,
                    base_pa: 85_000.0,
                    top_pa: 60_000.0,
                    f_sed: 1.0,
                },
                AerosolModeDto {
                    material: "tholin".to_owned(),
                    phase: SubstancePhaseDto::Solid,
                    deck: None,
                    shape: AerosolShapeDto::Aggregate,
                    size_distribution: SizeDistributionDto::Aggregate {
                        monomer_radius_m: 5e-8,
                        monomer_count: 3_000.0,
                        fractal_dimension: 2.0,
                        fractal_prefactor: 1.666_666_666_666_666_7,
                    },
                    column_mass_kg_m2: 0.003,
                    tau_550: 0.4,
                    base_pa: 20_000.0,
                    top_pa: 10.0,
                    f_sed: 0.0,
                },
            ],
            decks: vec![CondensateDeckDto {
                condensate: "H2O".to_owned(),
                vapour_mole_fraction_above: 3e-6,
            }],
            absorbers: vec![
                AbsorberDto {
                    species: "O3".to_owned(),
                    column_m2: 8.06e22,
                    base_pa: 30_000.0,
                    top_pa: 30.0,
                    profile: Some(vec![
                        MixingRatioNodeDto {
                            pressure_pa: 30_000.0,
                            relative_mixing_ratio: 0.05,
                        },
                        MixingRatioNodeDto {
                            pressure_pa: 1_000.0,
                            relative_mixing_ratio: 1.0,
                        },
                        MixingRatioNodeDto {
                            pressure_pa: 30.0,
                            relative_mixing_ratio: 0.2,
                        },
                    ]),
                },
                AbsorberDto {
                    species: "SO2".to_owned(),
                    column_m2: 1e19,
                    base_pa: 5_000.0,
                    top_pa: 1_000.0,
                    profile: None,
                },
            ],
        }
    }

    fn inventory_json() -> Value {
        json!({
            "modes": [
                {
                    "material": "H2O",
                    "phase": "liquid",
                    "deck": "H2O",
                    "shape": "sphere",
                    "size_distribution": {
                        "type": "log_normal",
                        "effective_radius_m": 1e-5,
                        "effective_variance": 0.1,
                    },
                    "column_mass_kg_m2": 0.05,
                    "tau_550": 7.5,
                    "base_pa": 85_000.0,
                    "top_pa": 60_000.0,
                    "f_sed": 1.0,
                },
                {
                    "material": "tholin",
                    "phase": "solid",
                    "deck": null,
                    "shape": "aggregate",
                    "size_distribution": {
                        "type": "aggregate",
                        "monomer_radius_m": 5e-8,
                        "monomer_count": 3_000.0,
                        "fractal_dimension": 2.0,
                        "fractal_prefactor": 1.666_666_666_666_666_7,
                    },
                    "column_mass_kg_m2": 0.003,
                    "tau_550": 0.4,
                    "base_pa": 20_000.0,
                    "top_pa": 10.0,
                    "f_sed": 0.0,
                },
            ],
            "decks": [{ "condensate": "H2O", "vapour_mole_fraction_above": 3e-6 }],
            "absorbers": [
                {
                    "species": "O3",
                    "column_m2": 8.06e22,
                    "base_pa": 30_000.0,
                    "top_pa": 30.0,
                    "profile": [
                        { "pressure_pa": 30_000.0, "relative_mixing_ratio": 0.05 },
                        { "pressure_pa": 1_000.0, "relative_mixing_ratio": 1.0 },
                        { "pressure_pa": 30.0, "relative_mixing_ratio": 0.2 },
                    ],
                },
                {
                    "species": "SO2",
                    "column_m2": 1e19,
                    "base_pa": 5_000.0,
                    "top_pa": 1_000.0,
                    "profile": null,
                },
            ],
        })
    }

    fn condensates() -> Vec<SurfaceCondensateDto> {
        vec![SurfaceCondensateDto {
            substance: "CO2".to_owned(),
            phase: SubstancePhaseDto::Solid,
            reservoir_kg_m2: 320.0,
            reservoir_gel_m: Some(0.2),
            area_fraction: 0.02,
            trapped_area_fraction: Some(0.005),
            seasonal: true,
        }]
    }

    /// The shared fixture's Earth's condensates (`packages/protocol/fixtures/planetary.json`,
    /// illustrative): its sea, then its ice, by reservoir mass, as its liquids and ices list them;
    /// the ice sends its trapped area and the sea does not, so that the client decodes both.
    pub(crate) fn earth_condensates() -> Vec<SurfaceCondensateDto> {
        vec![
            SurfaceCondensateDto {
                substance: "H2O".to_owned(),
                phase: SubstancePhaseDto::Liquid,
                reservoir_kg_m2: 2_652_000.0,
                reservoir_gel_m: Some(2_600.0),
                area_fraction: 0.71,
                trapped_area_fraction: None,
                seasonal: false,
            },
            SurfaceCondensateDto {
                substance: "H2O".to_owned(),
                phase: SubstancePhaseDto::Solid,
                reservoir_kg_m2: 53_100.0,
                reservoir_gel_m: Some(57.9),
                area_fraction: 0.03,
                trapped_area_fraction: Some(0.0),
                seasonal: false,
            },
        ]
    }

    fn condensates_json() -> Value {
        json!([{
            "substance": "CO2",
            "phase": "solid",
            "reservoir_kg_m2": 320.0,
            "reservoir_gel_m": 0.2,
            "area_fraction": 0.02,
            "trapped_area_fraction": 0.005,
            "seasonal": true,
        }])
    }

    fn crust() -> CrustCompositionDto {
        CrustCompositionDto {
            primary: Some("feldspathic".to_owned()),
            secondary: Some("basalt_fe".to_owned()),
            tertiary: None,
            bulk: None,
            lid: CrustLidDto::Primary,
            provinces: "basalt_fe".to_owned(),
            redox: RedoxClassDto::Intermediate,
            mantle_redox_iw: -1.5,
            melt_area_fraction: 0.0,
        }
    }

    fn crust_json() -> Value {
        json!({
            "primary": "feldspathic",
            "secondary": "basalt_fe",
            "tertiary": null,
            "bulk": null,
            "lid": "primary",
            "provinces": "basalt_fe",
            "redox": "intermediate",
            "mantle_redox_iw": -1.5,
            "melt_area_fraction": 0.0,
        })
    }

    fn modifiers() -> SurfaceModifiersDto {
        SurfaceModifiersDto {
            maturity: 0.8,
            weathering_rate: 1.0,
            organic_cover: 0.0,
            resurfacing_m_per_yr: 0.04,
            volcanic_deposits: vec![
                SubstanceAreaDto {
                    substance: "SO2".to_owned(),
                    area_fraction: 0.27,
                },
                SubstanceAreaDto {
                    substance: "S8".to_owned(),
                    area_fraction: 0.716,
                },
            ],
            weathering_deposits: vec![],
        }
    }

    fn modifiers_json() -> Value {
        json!({
            "maturity": 0.8,
            "weathering_rate": 1.0,
            "organic_cover": 0.0,
            "resurfacing_m_per_yr": 0.04,
            "volcanic_deposits": [
                { "substance": "SO2", "area_fraction": 0.27 },
                { "substance": "S8", "area_fraction": 0.716 },
            ],
            "weathering_deposits": [],
        })
    }

    fn biosphere() -> BiosphereDto {
        BiosphereDto {
            fluxes: vec![BiosphereFluxDto {
                species: "O2".to_owned(),
                flux_earth: 1.0,
            }],
        }
    }

    fn biosphere_json() -> Value {
        json!({ "fluxes": [{ "species": "O2", "flux_earth": 1.0 }] })
    }

    fn europa_ocean() -> SubsurfaceOceanDto {
        SubsurfaceOceanDto {
            shell_thickness_m: 12_800.0,
            thickness_m: 124_200.0,
            floor: OceanFloorDto::Rock,
            liquid: "H2O".to_owned(),
            solute_mass_fraction: 0.0,
            base_temperature_k: 273.15,
        }
    }

    fn europa_ocean_json() -> Value {
        json!({
            "shell_thickness_m": 12_800.0,
            "thickness_m": 124_200.0,
            "floor": "rock",
            "liquid": "H2O",
            "solute_mass_fraction": 0.0,
            "base_temperature_k": 273.15,
        })
    }

    #[test]
    fn an_earth_s_surface_wire_form() {
        assert_wire_form(&earth_surface(), earth_surface_json());
    }

    #[test]
    fn a_venus_s_surface_wire_form() {
        assert_wire_form(&venus_surface(), venus_surface_json());
    }

    #[test]
    fn a_jupiter_s_envelope_wire_form() {
        assert_wire_form(&jupiter_envelope(), jupiter_envelope_json());
    }

    #[test]
    fn a_sub_neptune_s_envelope_wire_form() {
        assert_wire_form(&sub_neptune_envelope(), sub_neptune_envelope_json());
    }

    /// Decision-p14-t35e-wire §1.5 item 4: every member a later task fills round-trips absent,
    /// `null` and a value, and an absent one is written with no key.
    #[test]
    fn each_later_member_round_trips_absent_null_and_a_value() {
        /// The member's `null` where `value` is `false`, and the fixture's value otherwise.
        fn modelled<T>(value: bool, fixture: impl FnOnce() -> T) -> Modelled<T> {
            if value {
                Modelled::Value(fixture())
            } else {
                Modelled::Null
            }
        }
        fn with(member: &str, value: bool) -> BodySurfaceDto {
            let mut surface = earth_surface();
            match member {
                "subsurface_ocean" => surface.subsurface_ocean = modelled(value, europa_ocean),
                "vertical_structure" => {
                    surface.vertical_structure = modelled(value, earth_structure);
                }
                "aerosols" => surface.aerosols = modelled(value, inventory),
                "condensates" => surface.condensates = modelled(value, condensates),
                "crust" => surface.crust = modelled(value, crust),
                "modifiers" => surface.modifiers = modelled(value, modifiers),
                _ => surface.biosphere = modelled(value, biosphere),
            }
            surface
        }
        let members = [
            ("subsurface_ocean", europa_ocean_json()),
            ("vertical_structure", earth_structure_json()),
            ("aerosols", inventory_json()),
            ("condensates", condensates_json()),
            ("crust", crust_json()),
            ("modifiers", modifiers_json()),
            ("biosphere", biosphere_json()),
        ];
        // Absent: the Earth's own form has none of the keys.
        let absent = earth_surface_json();
        for (member, _) in &members {
            assert!(absent.get(member).is_none(), "{member}");
        }
        assert_wire_form(&earth_surface(), absent);
        for (member, value) in members {
            let mut wire = earth_surface_json();
            wire[member] = Value::Null;
            assert_wire_form(&with(member, false), wire.clone());
            wire[member] = value;
            assert_wire_form(&with(member, true), wire);
        }
    }

    /// Decision-pre-23-review item 12: a condensate's layer is absent where the registry has not
    /// read its phase's density, written with no key, and a condensate without one reads as
    /// absent, as a ring's `normal` does.
    #[test]
    fn a_condensate_s_layer_is_optional_on_the_wire() {
        let argon = SurfaceCondensateDto {
            substance: "Ar".to_owned(),
            reservoir_gel_m: None,
            ..condensates()[0].clone()
        };
        let mut wire = condensates_json()[0].clone();
        wire["substance"] = json!("Ar");
        wire.as_object_mut()
            .expect("a condensate is an object")
            .remove("reservoir_gel_m");
        assert_wire_form(&argon, wire.clone());
        wire["reservoir_gel_m"] = Value::Null;
        let null: SurfaceCondensateDto = serde_json::from_value(wire).unwrap();
        assert_eq!(null, argon);
        assert_wire_form(&condensates()[0], condensates_json()[0].clone());
    }

    /// Decision-p14-t54a-water-consistency §5.2: a condensate's mass is always sent, so that a
    /// condensate without it is refused, never read as none.
    #[test]
    fn a_condensate_s_mass_is_required() {
        let mut wire = condensates_json()[0].clone();
        wire.as_object_mut()
            .expect("a condensate is an object")
            .remove("reservoir_kg_m2");
        let refused = serde_json::from_value::<SurfaceCondensateDto>(wire).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("missing field `reservoir_kg_m2`"),
            "{refused}"
        );
    }

    /// Decision-p14-t54a-water-consistency §5.3: a condensate's trapped area is absent until
    /// P14.T54.b, written with no key, and a condensate without one, or with `null`, reads as
    /// absent, as its layer does.
    #[test]
    fn a_condensate_s_trapped_area_is_optional_on_the_wire() {
        let untrapped = SurfaceCondensateDto {
            trapped_area_fraction: None,
            ..condensates()[0].clone()
        };
        let mut wire = condensates_json()[0].clone();
        wire.as_object_mut()
            .expect("a condensate is an object")
            .remove("trapped_area_fraction");
        assert_wire_form(&untrapped, wire.clone());
        wire["trapped_area_fraction"] = Value::Null;
        let null: SurfaceCondensateDto = serde_json::from_value(wire).unwrap();
        assert_eq!(null, untrapped);
    }

    /// Decision-p14-t35e-wire §1.5 item 3: an `ok` surface with a member the types lack still
    /// decodes, so that a later member is additive.
    #[test]
    fn a_surface_with_an_unknown_member_decodes() {
        let mut wire = earth_surface_json();
        wire["albedo_map"] = json!({ "bands": [0.1, 0.2] });
        assert_eq!(
            serde_json::from_value::<BodySurfaceDto>(wire).unwrap(),
            earth_surface()
        );
    }

    /// P14.T35.f (decision-backlog-2 §2.3): the melt's own vapour is written with no key while it
    /// is not modelled and a surface with no key reads it so, `null` round-trips as computed none,
    /// and no value decodes until P14.T55.b gives its type members.
    #[test]
    fn the_melt_s_vapour_is_absent_or_null_and_holds_no_value_yet() {
        let absent = earth_surface_json();
        assert_eq!(earth_surface().silicate_vapour, Modelled::NotModelled);
        assert!(absent.get("silicate_vapour").is_none(), "{absent}");
        assert_wire_form(&earth_surface(), earth_surface_json());
        // A surface with no key, read on its own: plan 14's T35.f names it apart from the round trip.
        assert_eq!(
            serde_json::from_value::<BodySurfaceDto>(absent)
                .unwrap()
                .silicate_vapour,
            Modelled::NotModelled
        );

        let mut wire = earth_surface_json();
        wire["silicate_vapour"] = Value::Null;
        let none = BodySurfaceDto {
            silicate_vapour: Modelled::Null,
            ..earth_surface()
        };
        assert_wire_form(&none, wire.clone());

        wire["silicate_vapour"] = json!({ "Na": 1.0 });
        let error = serde_json::from_value::<BodySurfaceDto>(wire).unwrap_err();
        assert!(
            error.to_string().contains("there are no variants"),
            "unexpected error: {error}"
        );
    }

    /// Decision-composition §1.10: every keyed member holds any string, a key no registry row has
    /// yet included, so that a new row is additive.
    #[test]
    fn every_keyed_member_holds_any_registry_key() {
        let unknown = "Xe";
        let mut surface = earth_surface();
        surface.gases[0].species = unknown.to_owned();
        surface.climate_regime.condensable = Some(unknown.to_owned());
        surface.surface_liquids[0].substance = unknown.to_owned();
        surface.surface_ices[0].substance = unknown.to_owned();
        let mut wire = earth_surface_json();
        wire["gases"][0]["species"] = json!(unknown);
        wire["climate_regime"]["condensable"] = json!(unknown);
        wire["surface_liquids"][0]["substance"] = json!(unknown);
        wire["surface_ices"][0]["substance"] = json!(unknown);
        if let Some(epoch) = surface.wet_epoch.as_mut() {
            epoch.liquid = unknown.to_owned();
        }
        wire["wet_epoch"]["liquid"] = json!(unknown);
        // And every keyed member of the later tasks' members.
        let key = || unknown.to_owned();
        surface.subsurface_ocean = Modelled::Value(SubsurfaceOceanDto {
            liquid: key(),
            ..europa_ocean()
        });
        let mut ocean = europa_ocean_json();
        ocean["liquid"] = json!(unknown);
        wire["subsurface_ocean"] = ocean;
        let mut aerosols = inventory();
        for mode in &mut aerosols.modes {
            mode.material = key();
            mode.deck = Some(key());
        }
        aerosols.decks[0].condensate = key();
        for absorber in &mut aerosols.absorbers {
            absorber.species = key();
        }
        surface.aerosols = Modelled::Value(aerosols);
        let mut aerosols = inventory_json();
        for mode in 0..2 {
            aerosols["modes"][mode]["material"] = json!(unknown);
            aerosols["modes"][mode]["deck"] = json!(unknown);
            aerosols["absorbers"][mode]["species"] = json!(unknown);
        }
        aerosols["decks"][0]["condensate"] = json!(unknown);
        wire["aerosols"] = aerosols;
        surface.condensates = Modelled::Value(vec![SurfaceCondensateDto {
            substance: key(),
            ..condensates()[0].clone()
        }]);
        let mut condensed = condensates_json();
        condensed[0]["substance"] = json!(unknown);
        wire["condensates"] = condensed;
        surface.crust = Modelled::Value(CrustCompositionDto {
            primary: Some(key()),
            secondary: Some(key()),
            tertiary: Some(key()),
            provinces: key(),
            ..crust()
        });
        let mut crusts = crust_json();
        for member in ["primary", "secondary", "tertiary", "provinces"] {
            crusts[member] = json!(unknown);
        }
        wire["crust"] = crusts;
        surface.biosphere = Modelled::Value(BiosphereDto {
            fluxes: vec![BiosphereFluxDto {
                species: key(),
                flux_earth: 1.0,
            }],
        });
        wire["biosphere"] = json!({ "fluxes": [{ "species": unknown, "flux_earth": 1.0 }] });
        assert_wire_form(&surface, wire);
        let mut envelope = jupiter_envelope();
        envelope.composition[0].species = unknown.to_owned();
        envelope.decks[0].material = unknown.to_owned();
        envelope.decks[0].deck = Some(unknown.to_owned());
        let mut wire = jupiter_envelope_json();
        wire["composition"][0]["species"] = json!(unknown);
        wire["decks"][0]["material"] = json!(unknown);
        wire["decks"][0]["deck"] = json!(unknown);
        assert_wire_form(&envelope, wire);
    }

    /// The four fixtures' mixtures, the Earth's, the Venus's, the Jupiter's and the
    /// sub-Neptune's, sum to 1 to 10⁻⁹ and descend.
    #[test]
    fn the_fixtures_gases_sum_to_one_and_descend() {
        for gases in [
            earth_surface().gases,
            venus_surface().gases,
            jupiter_envelope().composition,
            sub_neptune_envelope().composition,
        ] {
            let sum = gases.iter().fold(0.0, |sum, gas| sum + gas.mole_fraction);
            assert!((sum - 1.0).abs() <= 1e-9, "{gases:?}");
            assert!(
                gases
                    .windows(2)
                    .all(|pair| pair[0].mole_fraction >= pair[1].mole_fraction),
                "{gases:?}"
            );
        }
    }

    #[test]
    fn surface_state_strings() {
        assert_wire_strings(&[
            (SurfaceStateDto::MagmaOcean, "magma_ocean"),
            (SurfaceStateDto::Airless, "airless"),
            (SurfaceStateDto::RunawayGreenhouse, "runaway_greenhouse"),
            (SurfaceStateDto::Temperate, "temperate"),
            (SurfaceStateDto::Snowball, "snowball"),
            (SurfaceStateDto::SteamEnvelope, "steam_envelope"),
            (SurfaceStateDto::Tenuous, "tenuous"),
        ]);
    }

    #[test]
    fn surface_material_strings() {
        assert_wire_strings(&[
            (SurfaceMaterialDto::Rock, "rock"),
            (SurfaceMaterialDto::Ice, "ice"),
        ]);
    }

    #[test]
    fn radiation_class_strings() {
        assert_wire_strings(&[
            (RadiationClassDto::Dark, "dark"),
            (RadiationClassDto::Quiet, "quiet"),
            (RadiationClassDto::Active, "active"),
            (RadiationClassDto::Intense, "intense"),
            (RadiationClassDto::Extreme, "extreme"),
        ]);
    }

    #[test]
    fn thermal_regime_strings() {
        assert_wire_strings(&[
            (ThermalRegimeDto::AirlessLike, "airless_like"),
            (ThermalRegimeDto::Transitional, "transitional"),
            (ThermalRegimeDto::Efficient, "efficient"),
        ]);
    }

    #[test]
    fn climate_forcing_strings() {
        assert_wire_strings(&[
            (ClimateForcingDto::Seasonal, "seasonal"),
            (ClimateForcingDto::IceBelt, "ice_belt"),
            (ClimateForcingDto::SlowRotator, "slow_rotator"),
            (ClimateForcingDto::Locked, "locked"),
        ]);
    }

    /// The climate model is R09's `ClimateModelDto`, pinned there too; the regime names it.
    #[test]
    fn climate_model_strings() {
        assert_wire_strings(&[
            (ClimateModelDto::EnergyBalance, "energy_balance"),
            (
                ClimateModelDto::LockedEnergyBalance,
                "locked_energy_balance",
            ),
            (
                ClimateModelDto::RadiativeEquilibrium,
                "radiative_equilibrium",
            ),
            (ClimateModelDto::Isothermal, "isothermal"),
        ]);
    }

    #[test]
    fn tectonic_regime_strings() {
        assert_wire_strings(&[
            (TectonicRegimeDto::MobileLid, "mobile_lid"),
            (TectonicRegimeDto::StagnantLid, "stagnant_lid"),
            (TectonicRegimeDto::HeatPipe, "heat_pipe"),
            (TectonicRegimeDto::Episodic, "episodic"),
            (TectonicRegimeDto::IceShell, "ice_shell"),
        ]);
    }

    #[test]
    fn magnetic_field_class_strings() {
        assert_wire_strings(&[
            (MagneticFieldClassDto::None, "none"),
            (MagneticFieldClassDto::Weak, "weak"),
            (MagneticFieldClassDto::Strong, "strong"),
        ]);
    }

    #[test]
    fn ocean_floor_strings() {
        assert_wire_strings(&[
            (OceanFloorDto::Rock, "rock"),
            (OceanFloorDto::IceIii, "ice_iii"),
            (OceanFloorDto::IceV, "ice_v"),
            (OceanFloorDto::IceVi, "ice_vi"),
            (OceanFloorDto::IceVii, "ice_vii"),
        ]);
    }

    #[test]
    fn crater_target_class_strings() {
        assert_wire_strings(&[
            (CraterTargetClassDto::NonPorous, "non_porous"),
            (CraterTargetClassDto::Porous, "porous"),
        ]);
    }

    #[test]
    fn substance_phase_strings() {
        assert_wire_strings(&[
            (SubstancePhaseDto::Solid, "solid"),
            (SubstancePhaseDto::Liquid, "liquid"),
            (SubstancePhaseDto::Vapour, "vapour"),
            (SubstancePhaseDto::Supercritical, "supercritical"),
        ]);
    }

    #[test]
    fn aerosol_shape_strings() {
        assert_wire_strings(&[
            (AerosolShapeDto::Sphere, "sphere"),
            (
                AerosolShapeDto::NonSphericalMineral,
                "non_spherical_mineral",
            ),
            (AerosolShapeDto::Crystal, "crystal"),
            (AerosolShapeDto::Aggregate, "aggregate"),
        ]);
    }

    #[test]
    fn crust_lid_strings() {
        assert_wire_strings(&[
            (CrustLidDto::Primary, "primary"),
            (CrustLidDto::Secondary, "secondary"),
            (CrustLidDto::Bulk, "bulk"),
        ]);
    }

    #[test]
    fn redox_class_strings() {
        assert_wire_strings(&[
            (RedoxClassDto::Reduced, "reduced"),
            (RedoxClassDto::Intermediate, "intermediate"),
            (RedoxClassDto::Oxidised, "oxidised"),
        ]);
    }

    #[test]
    fn crater_screening_wire_forms() {
        assert_wire_form(&CraterScreeningDto::None, json!({ "type": "none" }));
        assert_wire_form(
            &CraterScreeningDto::Atmosphere {
                column_mass_kg_m2: 10_318.0,
            },
            json!({ "type": "atmosphere", "column_mass_kg_m2": 10_318.0 }),
        );
        assert_wire_form(
            &CraterScreeningDto::Cutoff { diameter_m: 104.0 },
            json!({ "type": "cutoff", "diameter_m": 104.0 }),
        );
    }

    #[test]
    fn size_distribution_wire_forms() {
        assert_wire_form(
            &SizeDistributionDto::Gamma {
                effective_radius_m: 1e-6,
                effective_variance: 0.1,
            },
            json!({ "type": "gamma", "effective_radius_m": 1e-6, "effective_variance": 0.1 }),
        );
        assert_wire_form(
            &SizeDistributionDto::LogNormal {
                effective_radius_m: 1e-5,
                effective_variance: 0.2,
            },
            json!({ "type": "log_normal", "effective_radius_m": 1e-5, "effective_variance": 0.2 }),
        );
        assert_wire_form(
            &SizeDistributionDto::Aggregate {
                monomer_radius_m: 5e-8,
                monomer_count: 3_000.0,
                fractal_dimension: 2.0,
                fractal_prefactor: 1.666_666_666_666_666_7,
            },
            json!({
                "type": "aggregate",
                "monomer_radius_m": 5e-8,
                "monomer_count": 3_000.0,
                "fractal_dimension": 2.0,
                "fractal_prefactor": 1.666_666_666_666_666_7,
            }),
        );
    }

    #[test]
    fn a_subsurface_ocean_wire_form() {
        assert_wire_form(&europa_ocean(), europa_ocean_json());
    }

    #[test]
    fn an_aerosol_inventory_wire_form() {
        assert_wire_form(&inventory(), inventory_json());
    }

    #[test]
    fn a_vertical_structure_wire_form() {
        assert_wire_form(&earth_structure(), earth_structure_json());
    }

    /// Every quantity's name carries its unit: each numeric key of a whole surface and envelope
    /// ends in a unit, or is a dimensionless quantity named as one.
    #[test]
    fn every_quantity_s_name_carries_its_unit() {
        const DIMENSIONLESS: [&str; 25] = [
            "mole_fraction",
            "area_fraction",
            "heat_capacity_over_r",
            "ocean_fraction",
            "ice_fraction",
            "cloud_fraction",
            "continental_fraction",
            "volcanism_level",
            "outgassing_earth",
            "solute_mass_fraction",
            "k_target",
            "alpha",
            "effective_variance",
            "monomer_count",
            "fractal_dimension",
            "fractal_prefactor",
            "tau_550",
            "f_sed",
            "vapour_mole_fraction_above",
            "relative_mixing_ratio",
            "opacity_ratio",
            "redistribution_factor",
            "haze_tau_550",
            "flux_earth",
            "trapped_area_fraction",
        ];
        const MORE: [&str; 5] = [
            "mantle_redox_iw",
            "melt_area_fraction",
            "maturity",
            "organic_cover",
            "weathering_rate",
        ];
        const UNITS: [&str; 14] = [
            "_k",
            "_pa",
            "_m",
            "_s",
            "_w_m2",
            "_g_per_mol",
            "_per_km2",
            "_kg_m3",
            "_m_s",
            "_yr",
            "_m_per_yr",
            "_kg_m2",
            "_m2",
            "_m2_per_kg",
        ];
        fn numbers(value: &Value, out: &mut Vec<String>) {
            match value {
                Value::Object(fields) => {
                    for (key, inner) in fields {
                        if inner.is_number() {
                            out.push(key.clone());
                        }
                        numbers(inner, out);
                    }
                }
                Value::Array(items) => items.iter().for_each(|item| numbers(item, out)),
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
            }
        }
        let mut surface = earth_surface();
        surface.subsurface_ocean = Modelled::Value(europa_ocean());
        surface.vertical_structure = Modelled::Value(earth_structure());
        surface.aerosols = Modelled::Value(inventory());
        surface.condensates = Modelled::Value(condensates());
        surface.crust = Modelled::Value(crust());
        surface.modifiers = Modelled::Value(modifiers());
        surface.biosphere = Modelled::Value(biosphere());
        let mut keys = Vec::new();
        numbers(&serde_json::to_value(&surface).unwrap(), &mut keys);
        numbers(
            &serde_json::to_value(jupiter_envelope()).unwrap(),
            &mut keys,
        );
        for key in keys {
            let named = DIMENSIONLESS.contains(&key.as_str())
                || MORE.contains(&key.as_str())
                || UNITS.iter().any(|unit| key.ends_with(unit));
            assert!(named, "{key} names no unit");
        }
    }
}
