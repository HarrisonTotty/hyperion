//! A body's surface section and its gas envelope on the wire (plan 14, P14.T35.e; the
//! simulation's `record::Surface` and `record::Envelope`, decision-p14-t35e-wire).
//!
//! # Exactly one of the two
//!
//! A body with a solid surface (a rocky or icy planet, a moon, a dwarf planet) has its surface
//! section and an envelope `not_applicable`; a body in the gas-envelope state (a gas or ice giant,
//! a sub-Neptune) has its envelope and a surface `not_applicable`, since within the model it has no
//! surface: no surface pressure, its heights from the 1-bar level, and its envelope's base at
//! kilobars. So, from generator version 24, does a solid body under a supercritical envelope
//! (P14.T53.b part 2), where a condensable above its critical temperature holds a liquid-like
//! layer of at least a thousandth of the body's mass over its ground, which then is no surface. A
//! ring, a belt or the halo has neither.
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
//! subsurface ocean's `subsurface_ocean`, an H₂/He envelope's `supercritical`. A member this
//! generator version does not compute is absent ([`Modelled::NotModelled`]), and a reader takes it
//! as not modelled, never as none. The members whose tasks land later (P14.T24.c, T24.e, T51.d,
//! T51.e, T53.d and T55.b) are absent until they do, and each task may reshape its own member's
//! type while no build has sent a value of it; once a build has, a change bumps
//! [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION). The condensates (P14.T51.a) and the crust
//! (P14.T51.c) are sent from P14.T54.a's batch, so their shapes are now fixed.
//!
//! # Closed classes
//!
//! Each closed enum here is a physics class with every variant the plan's drafted tasks name, so
//! that no later task adds one: a variant no rule produces yet is listed and documented as such
//! (`heat_pipe`, `episodic`, `ice_shell`; `tenuous` until generator version 24; the taxonomic
//! classes P14.T51.e's rules do not make, `cgh` among them), and one no record sends
//! (`steam_envelope`, whose body has its envelope section).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::modelled::Modelled;
use crate::surface::ClimateModelDto;

/// The state of a body's surface (plan 14's `SurfaceState`, P14.T13.c), less the gas envelope and,
/// from generator version 25, the steam envelope (P14.T53.b part 3), which have no surface section:
/// their air is the envelope section's.
///
/// `RunawayGreenhouse`, `Temperate` and `Snowball` are the body's insolation class against its
/// hosts' habitable zone (P14.T12.b's limits), not its present climate: read the mean surface
/// temperature, the ice and ocean fractions and the condensates for that, as the climate regime
/// does (decision-p14-t54a-states). Below generator version 24 the runaway limit is read at the
/// hosts' largest past luminosity. From 24 (P14.T53.b part 1) the class is the present
/// insolation's but for part 1 (c)'s hothouse, both limits read at the hosts' luminosities now,
/// and only the water's loss, with its oxygen and the carbon left airborne, reads the largest past
/// luminosity: a world inside the runaway limit at its hosts' peak alone is `Temperate` or
/// `Snowball` with its water lost, and a dry world between the limits too hot for liquid water at
/// its surface pressure is `RunawayGreenhouse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SurfaceStateDto {
    /// Molten rock at the surface: a young terrestrial planet, or one hotter than the silicate
    /// solidus.
    MagmaOcean,
    /// A surface pressure below the airless limit, 100 Pa: bare rock or ice; from generator
    /// version 24 (P14.T53.c) one whose column also lies under the tenuous floor, 10⁻⁴ kg m⁻², a
    /// thinner air above it being `tenuous`. An airless body lists no gases.
    Airless,
    /// Inside the runaway-greenhouse limit when water was present: the oceans lost, a
    /// carbon-dioxide air, as on Venus. Below generator version 24 the limit is read at the hosts'
    /// largest past luminosity, so that about an evolved host the present climate can be cold
    /// (decision-pre-23-review item 33), and every runaway keeps Venus's cloud deck. From 24
    /// (P14.T53.b part 1) it is inside the runaway limit now, its water lost, under whatever air
    /// escape left it, with Venus's deck only where the air beneath can cycle it: a runaway whose
    /// climate under the deck is colder than about 430 K is solved without it, its cloud the water
    /// cloud's. Only the water's loss, with its oxygen and the carbon left airborne, reads the
    /// hosts' largest past luminosity. From 24 it is also a world between the limits now that
    /// stores no carbon for want of water and holds no liquid water, its mean surface temperature
    /// at or above water's saturation temperature at its surface pressure (647.096 K at or above
    /// 22.064 megapascals; not judged below 611.657 Pa): a desiccated hothouse, Venus's state,
    /// whatever its present insolation, with Venus's deck where the air beneath can cycle it
    /// (P14.T53.b part 1 (c)).
    RunawayGreenhouse,
    /// Between the runaway limit and the maximum-greenhouse limit now, as on Earth and Mars. Below
    /// generator version 24 the runaway limit is read at the hosts' largest past luminosity; from
    /// 24 now, so that a world inside it at its hosts' peak alone is `Temperate` here, its water
    /// lost and its carbon airborne (P14.T53.b part 1). A world here can be ice-covered, on the
    /// cold side of the ice-albedo bistability (decision-pre-23-review item 26). From 24 a dry
    /// world here too hot for liquid water at its surface pressure is sent as `runaway_greenhouse`
    /// instead (P14.T53.b part 1 (c)).
    Temperate,
    /// Beyond the maximum-greenhouse limit now, where a carbon-dioxide and water greenhouse cannot
    /// hold the surface above freezing (Kopparapu et al. 2013), as on Titan. From generator
    /// version 24 a world inside the runaway limit at its hosts' largest past luminosity alone is
    /// `Snowball` here, its water lost and its carbon airborne (P14.T53.b part 1). Until
    /// P14.T53.a's carbon-dioxide form, a dry carbon-dioxide world here can be published above
    /// melting with no ice (P14.T13.c's finding "No maximum-greenhouse ceiling";
    /// decision-p14-t51a-partition §1).
    Snowball,
    /// Water above its critical point with no interface, whose liquid-like reservoir hides the
    /// body's surface: never sent, since such a body has its envelope section (from generator
    /// version 25, P14.T53.b part 3) and its surface is `not_applicable`, as a gas envelope's is
    /// (decision-bump-plan-after-22 R1.1). The variant stays so that the class needs no
    /// `PROTOCOL_VERSION` change.
    SteamEnvelope,
    /// A thin or transient air below 100 Pa whose column reaches the tenuous floor, 10⁻⁴ kg m⁻²
    /// (the column above which the wind's ions do not reach the ground), its gases listed, as
    /// Triton's and Pluto's nitrogen are held by their ice (P14.T53.c): from generator version 24,
    /// produced by no rule before it. Its surface is a bare one, as an airless body's; only its
    /// pressure and gases are its air's. An air below the floor is `airless`, with no gas listed.
    /// Its aerosols are absent, not modelled at version 24 (P14.T55.n), never the empty inventory.
    Tenuous,
}

/// What a surface is made of, rock or ice, by the body's water fraction (plan 14's
/// `SurfaceMaterial`, P14.T13.c): ice for a body of at least 10% water by mass, rock otherwise;
/// and from generator version 24 rock where a heated ice bulk's shell lost its ice, leaving a rock
/// lag (P14.T56.g).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SurfaceMaterialDto {
    /// Rock and dust: a rock bulk's, or a heated ice bulk's rock lag (P14.T56.g).
    Rock,
    /// Ice: a body of at least 10% water by mass, an ice bulk with no lag.
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
///
/// Below generator version 24 every ocean rests on rock. At 24 a rule makes rock, ice III and ice
/// V; ice VI and ice VII wait for P14.T55.k's warm oceans under high-pressure ice, since an ocean
/// at most 273.16 K lies below the V–VI–liquid point, 273.31 K (IAPWS R14-08(2011), Table 1).
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
    /// carbon dioxide, and, from generator version 25, a condensate's reservoir held above its
    /// critical isochore (P14.T53.b part 3): never an aerosol mode's.
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
    /// Its liquid: a key of the substance registry. `H2O` below generator version 24; from 24
    /// (P14.T51.g.b) the solution row of the solute that depresses its freezing point most,
    /// `brine_nacl` or `nh3_h2o`, where that solute's mass fraction is at least 10⁻³, and `H2O`
    /// otherwise.
    pub liquid: String,
    /// The mass fraction of solutes in its liquid, in `[0, 1]`: 0 below generator version 24; from
    /// 24 its sodium chloride's and ammonia's together, each held at its eutectic's, so at most
    /// 0.437.
    pub solute_mass_fraction: f64,
    /// Its temperature at the ice shell's base, where the shell meets it, K, positive: the
    /// liquid's melting point there. 273 K below generator version 24; from 24 (P14.T51.g.b) ice
    /// Ih's melting point at the base's pressure less the solutes' depression, at most 273.16 K and
    /// lower under a deeper base or with solutes.
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
/// `surface_pressure_pa`, which its first node repeats. Sent from P14.T54.b's bump at generator
/// version 24, and absent from a server whose generator does not model it.
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

/// What an aerosol mode's `tau_550` rests on (plan 14, P14.T24.c's `DepthBasis` with P14.T49.e's
/// extinction basis). Closed: every variant is in the type from the first build that sends a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AerosolTauBasisDto {
    /// P14.T49.e's mass extinction times the column mass, of Mie spheres at the index file's
    /// refractive index: a `sphere` file, its own shape class.
    Sphere,
    /// P14.T49.e's mass extinction from the index file's own phase table (rendering plan R08.T5.c's
    /// or T5.d's), the mode's effective radius within the table's sizes.
    PhaseTable,
    /// P14.T49.e's mass extinction for a phase-table file's mode whose effective radius lies
    /// outside its table's sizes, where the particles are taken as spheres.
    SpheresOutsidePhaseTable,
    /// P14.T49.e's mass extinction for a `nonSphericalMineral` or `crystal` file with no phase table
    /// of its own: the spheres of its volume-to-area radius, a labelled fallback.
    SphereFallback,
    /// Ackerman and Marley's (2001) geometric scatterers, `Q_ext` = 2, for a mode larger than the
    /// extinction table's grid.
    GeometricLimit,
    /// A rule's own column, calibrated on a measured one (the methane haze's on Titan's), with no
    /// mass extinction behind it.
    Rule,
}

/// Why members of an aerosol mode are not modelled (plan 14, P14.T24.c with P14.T52.c's sizes and
/// P14.T49.e's optics; decision-p14-t54b-open §2.2): one reason a mode, which says which members
/// are absent. Tagged by `type`. Closed: every variant is in the type from the first build that
/// sends a mode.
///
/// | Reason | `size_distribution` | `column_mass_kg_m2` | `tau_550` | `temperature_k` |
/// | --- | --- | --- | --- | --- |
/// | `no_grain_density` | either | either | absent | present |
/// | `no_collision_constants`, `no_condensate`, `no_size_distribution` | absent | present | absent | present |
/// | `optics_not_modelled` | present | either | absent | present |
/// | `no_mass_extinction` | present | absent | present | absent |
///
/// A rule's mode whose optics are not modelled (the dust's, a sulphate layer's, the acid's) is sent
/// without its τ(550) or its column mass: the rule set the τ, but the generator refuses it with the
/// optics, and the column would be that τ over a mass extinction it does not have.
///
/// A client refuses a mode whose absent members are not its reason's row; the table is part of
/// the shape from the first send.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum AerosolNotModelledDto {
    /// The material has no grain density in the mode's phase at the temperature the generator took
    /// it at, so its sizes (a deck's settling) or its optics are not modelled.
    NoGrainDensity {
        /// The phase asked for: `solid` or `liquid`.
        phase: SubstancePhaseDto,
        /// The temperature asked for, K, positive: the mode's `temperature_k`.
        temperature_k: f64,
    },
    /// The air's most abundant gas has no collision constants, so its viscosity, and the deck's
    /// fall speed and sizes, are not modelled.
    NoCollisionConstants,
    /// The deck's column holds no condensate at any level of the generator's scan (a deck thinner
    /// than a step), so its sizes are not modelled.
    NoCondensate,
    /// The deck's levels give no size distribution: an effective radius that is not finite and
    /// positive.
    NoSizeDistribution,
    /// The generator's optics (P14.T49.e) do not cover the mode's material, phase, variant or size:
    /// no index file, or a size off the extinction table's grid.
    OpticsNotModelled,
    /// P14.T49.e holds no aggregate's mass extinction: an aggregate's column mass is not modelled,
    /// and no optics were taken at a temperature, its τ(550) being a rule's.
    NoMassExtinction,
}

/// One aerosol mode: a cloud's, a haze's or a dust layer's particles of one material and phase
/// (plan 14, P14.T24.c).
///
/// A member the generator does not model is absent, never `null` and never 0, and
/// `not_modelled` says why ([`AerosolNotModelledDto`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AerosolModeDto {
    /// Its material: a key of the substance registry, one row for every phase of its substance.
    pub material: String,
    /// Its phase: `solid` or `liquid`, never `vapour` or `supercritical`.
    pub phase: SubstancePhaseDto,
    /// The deck it belongs to, by the deck's condensate key, so that a deck of several modes is
    /// one deck to its readers: in a surface's inventory, one of its `decks`; in an envelope, one
    /// of its `condensate_decks`. `null` for a mode of no deck: the dust, a haze (every mode of an
    /// envelope's `hazes`), and a sulphate layer, which names no deck though its material,
    /// sulphuric acid, condenses as a deck on another world.
    pub deck: Option<String>,
    /// Its shape class.
    pub shape: AerosolShapeDto,
    /// Its size distribution; absent where it is not modelled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub size_distribution: Option<SizeDistributionDto>,
    /// Its column mass, kg m⁻², not negative; absent where it is not modelled (an aggregate's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub column_mass_kg_m2: Option<f64>,
    /// Its vertical column optical depth at 550 nm where it lies, over the share of the sky it
    /// covers (in-cloud, `coverage` not applied), dimensionless and not negative; absent where it
    /// is not modelled, with `tau_basis`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tau_550: Option<f64>,
    /// What `tau_550` rests on, present exactly when it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tau_basis: Option<AerosolTauBasisDto>,
    /// The pressure of its base, Pa, positive and at least `top_pa`.
    pub base_pa: f64,
    /// The pressure of its top, Pa, positive.
    pub top_pa: f64,
    /// The exponent f of its mixing ratio, which goes as (p ÷ `p_base`)^f between its base and
    /// top: its deck's `f_sed` (Ackerman and Marley 2001), or 0 for a well-mixed layer; not
    /// negative. Its `tau_550` is spread by the same profile, so that the share of it above a
    /// level p between its top and base is ((p ÷ `base_pa`)^(f+1) − (`top_pa` ÷
    /// `base_pa`)^(f+1)) ÷ (1 − (`top_pa` ÷ `base_pa`)^(f+1)): plan 08 cuts a mode at a gas
    /// envelope's floor by it.
    pub f_sed: f64,
    /// The share of the sky it covers, dimensionless, in `[0, 1]`: in a surface's inventory, a
    /// deck's mode the body's cloud fraction (P14.T51.b's) and a rule's mode (the dust, a haze, a
    /// sulphate layer) the whole sky; every mode of an envelope, its decks' and its hazes', the
    /// whole sky, 1, at generator version 24. A reader applies it as the fraction it is, never as
    /// 1 by rule.
    pub coverage: f64,
    /// The temperature its phase, grain density and τ(550) were taken at, K, positive
    /// (decision-p14-t54a-grain-density §3): a deck's base's, a sulphate layer's at the skin, the
    /// dust's at the ground, an envelope haze's at its base on the envelope's profile. Absent where
    /// no optics were taken (an aggregate's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub temperature_k: Option<f64>,
    /// Why its absent members are not modelled; absent where every member is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub not_modelled: Option<AerosolNotModelledDto>,
}

/// A condensate deck: the condensate whose modes it holds, where it lies, and its vapour below
/// and above it (plan 14, P14.T24.c with P14.T52.c's loop and P14.T24.g's acid).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CondensateDeckDto {
    /// The condensate: a key of the substance registry, which its modes' `deck` names.
    pub condensate: String,
    /// Its phase at its base: `solid` or `liquid`. A deck keeps the phase it forms in, so that a
    /// liquid deck that rises above its freezing level stays liquid.
    pub phase: SubstancePhaseDto,
    /// The pressure of its base, Pa, positive and at least `top_pa`: the deepest level of its
    /// column at which its vapour reaches saturation, or the column's bottom (a surface air's
    /// ground, an envelope's 10 kbar, 10⁹ Pa) where the vapour is saturated there already, its
    /// condensate from below the column not counted.
    pub base_pa: f64,
    /// The temperature at its base on the air's vertical structure or the envelope's profile, K,
    /// positive.
    pub base_temperature_k: f64,
    /// The pressure of its top, Pa, positive.
    pub top_pa: f64,
    /// Its limiting vapour's mole fraction below the deck, dimensionless, in `[0, 1]`: in a
    /// surface's air, the air's at the ground; in an envelope, its quenched gas's at 1 bar before
    /// the decks trim it, the gas its decks' loop starts from at the column's 10-kbar bottom; in
    /// each, less what a deeper deck takes of it.
    ///
    /// Whose vapour it is follows the condensate's saturation law: the condensate's own gas for a
    /// law that reads one species (a volatile's own curve, as every deck of the generated sample's
    /// surface airs has; for sulphuric acid, the share `ε_v` of the sulphur dioxide that is its
    /// vapour); for a product law (NH₄SH's), the scarcer reactant; and for an element's law
    /// (forsterite's magnesium), the element's atoms per molecule of gas, which is not one gas's
    /// mole fraction.
    pub vapour_mole_fraction_below: f64,
    /// The same vapour's mole fraction above the deck, the cold trap's, in `[0, 1]`: as
    /// `vapour_mole_fraction_below`'s, the condensate's own gas, the scarcer reactant, or the
    /// element's atoms per molecule of gas, by its law.
    pub vapour_mole_fraction_above: f64,
    /// Its `f_sed` (Ackerman and Marley 2001), its modes' exponent, not negative: 0 for a deck of
    /// well-mixed modes (P14.T24.g's acid).
    pub f_sed: f64,
    /// Whether the saturation law that placed its base is applied outside what it was fitted over.
    pub extrapolated: bool,
}

/// Why a condensable whose vapour the air holds forms no deck (plan 14, P14.T52.c's
/// `NotFormed`). Tagged by `type`. Closed: every variant is in the type from the first build that
/// sends one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum NotFormedReasonDto {
    /// Its vapour holds at least half of the air (a Mars's or a Venus's carbon dioxide, a Triton's
    /// nitrogen): its condensation sets the air's own profile, which the decks' trace balance does
    /// not model.
    BulkGas,
    /// Its law gives no deck of an air that holds the vapour of the condensate `by`, which takes
    /// its limiting vapour first (quartz where enstatite can take its silicon).
    Displaced {
        /// The condensate that displaces it: a key of the substance registry.
        by: String,
    },
    /// Its vapour's partial pressure reaches its critical pressure where the air's profile cools
    /// through its critical temperature (P14.T52.f): it passes from supercritical fluid to
    /// compressed liquid around the critical point, crossing no saturation curve, so no deck has
    /// a base. Its dense fluid is not a cloud; the vapour stays in the air above, as a bulk gas's.
    Supercritical,
}

/// A condensable the inventory labels rather than traps as a deck, with why (plan 14,
/// P14.T52.c).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NotFormedCondensateDto {
    /// The condensate: a key of the substance registry.
    pub condensate: String,
    /// Why it forms no deck.
    pub reason: NotFormedReasonDto,
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

/// An absorbing gas with its column and layer (plan 14, P14.T24.c): in a surface's inventory, one
/// that is not one of the section's well-mixed `gases` (ozone, and sulphur dioxide by P14.T24.g);
/// in an envelope, its 10-mbar gas's (P14.T52.d), which its `composition` at 1 bar may hold too.
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

/// A surface air's aerosol and absorber inventory (plan 14, P14.T24.c with T24.g). Sent from
/// P14.T54.b's bump at generator version 24, and absent from a server whose generator does not
/// model it; its shape is P14.T54.b.b's (decision-p14-t54b-open §2). A surface with no air (an
/// airless body's, a melt's whose volatiles are gone) has the empty inventory: every list empty,
/// never `null`. A magma ocean that holds an air has none sent, as not modelled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AerosolInventoryDto {
    /// The modes, every cloud's, haze's and dust layer's: each deck's, in the decks' order, then
    /// the sulphate layer's, the haze's and the dust's.
    pub modes: Vec<AerosolModeDto>,
    /// The condensate decks, hottest base first, which the modes name: at most one deck a
    /// condensate, so that a mode's `deck` names one.
    pub decks: Vec<CondensateDeckDto>,
    /// The absorbers, in registry order.
    pub absorbers: Vec<AbsorberDto>,
    /// The condensables whose vapour the air holds that form no deck, each with why, in registry
    /// order.
    pub not_formed: Vec<NotFormedCondensateDto>,
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
    /// key, and its amount where its layer is absent; 0 for a supercritical fluid that is the air at
    /// the surface, and from generator version 25 a supercritical reservoir's whole, the
    /// liquid-like fluid above its critical isochore (P14.T53.b part 3). On a body whose water
    /// froze, as `reservoir_gel_m`. Finite and not negative, and always sent.
    pub reservoir_kg_m2: f64,
    /// Its condensed reservoir as a global equivalent layer, m, not negative: its mass over the
    /// whole surface over its grain density in its phase at the temperature it lies at; 0 for a
    /// supercritical fluid that is the air at the surface, and from generator version 25 a
    /// supercritical reservoir's over its liquid's density at its critical temperature, an upper
    /// bound (P14.T53.b part 3). On a body whose water froze, the whole frozen water the partition
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
    /// is the crust's own ice); from generator version 25 a supercritical reservoir's as a
    /// liquid's, the share of the basins its layer fills beside the other liquids (P14.T53.b part
    /// 3); 0 for a supercritical fluid that is the air at the surface, the only entry with no mass.
    /// Entries may overlap: an ice over a sea, and ices in the same
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

/// A surface's modifiers (plan 14, P14.T51.d with T51.j; decision-p14-t51d-weathering §2). Absent
/// from the wire until P14.T54.b's bump at generator version 24 puts them on the record
/// (P14.T54.b.c). P14.T51.d and T51.j gave it this shape, which may change until a build sends a
/// value.
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

/// A small body's taxonomic class (plan 14's `TaxonomicClass`, P14.T51.e): a label of its
/// surface's spectrum, not a substance. On the wire each is the class's name in lower case (`s`,
/// `cgh`, `xc`, `red`).
///
/// Closed, with every variant at its first send (decision-p14-t51e-contract §1.6): `DeMeo` et al.
/// 2009's 24 Bus–`DeMeo` classes (Icarus 202, 160), in their S, C and X complexes and end members;
/// Tholen's P, `DeMeo` and Carry 2013's low-albedo split of X (Icarus 226, 723); and the two
/// Kuiper-belt colours. At generator version 24 the rules make S, Q, C, Ch, K, Xc, V, A, D, P, red
/// and neutral; the others are made by no rule, so that a later rule needs no protocol change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TaxonomicClassDto {
    /// A-type, an end member (`DeMeo` et al. 2009): an ultramafic lid's. Made from generator
    /// version 24.
    A,
    /// B-type, of the C complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    B,
    /// C-type, of the C complex (`DeMeo` et al. 2009): a CI chondrite's or a phyllosilicate lid's.
    /// Made from generator version 24.
    C,
    /// Cb-type, of the C complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Cb,
    /// Cg-type, of the C complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Cg,
    /// Cgh-type, of the C complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Cgh,
    /// Ch-type, of the C complex (`DeMeo` et al. 2009): a CM chondrite lid's. Made from generator
    /// version 24.
    Ch,
    /// D-type, an end member (`DeMeo` et al. 2009): an asteroid belt's member or a moon, outside a
    /// Kuiper-like belt, with a classed lid under a tholin cover over at least half its surface.
    /// Made from generator version 24.
    D,
    /// K-type, an end member (`DeMeo` et al. 2009): a CV chondrite lid's. Made from generator
    /// version 24.
    K,
    /// L-type, an end member (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    L,
    /// O-type, an end member (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    O,
    /// Q-type, an end member (`DeMeo` et al. 2009): an ordinary chondrite's fresh surface. Made
    /// from generator version 24.
    Q,
    /// R-type, an end member (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    R,
    /// S-type, of the S complex (`DeMeo` et al. 2009): an ordinary chondrite's weathered surface,
    /// or one under an air whose column, above 10⁻⁴ kg m⁻², stops the wind's ions. Made from
    /// generator version 24.
    S,
    /// Sa-type, of the S complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Sa,
    /// Sq-type, of the S complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Sq,
    /// Sr-type, of the S complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Sr,
    /// Sv-type, of the S complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Sv,
    /// T-type, an end member (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    T,
    /// V-type, an end member (`DeMeo` et al. 2009): a basaltic lid's, as Vesta's crust. Made from
    /// generator version 24.
    V,
    /// X-type, of the X complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    X,
    /// Xc-type, of the X complex (`DeMeo` et al. 2009): an enstatite chondrite lid's. Made from
    /// generator version 24.
    Xc,
    /// Xe-type, of the X complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Xe,
    /// Xk-type, of the X complex (`DeMeo` et al. 2009). Made by no rule at generator version 24.
    Xk,
    /// P-type, Tholen's, the low-albedo X of `DeMeo` and Carry 2013 (geometric albedo under
    /// 0.075): a cometary lid's under no majority veneer. Made from generator version 24.
    P,
    /// A Kuiper-belt body's red colour: a tholin cover over at least half its surface, whatever
    /// its lid, as Quaoar's (Brown 2012, Annu. Rev. Earth Planet. Sci. 40, 467). Made from
    /// generator version 24.
    Red,
    /// A Kuiper-belt body's neutral colour: less of its surface under tholin, its water ice
    /// showing, as Orcus's and Haumea's neutral colours (Brown 2012). Made from generator version
    /// 24.
    Neutral,
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
/// generator version computes, `not_applicable` for a body in the gas-envelope state, or, from
/// generator version 24, a solid body under a supercritical envelope (P14.T53.b part 2), whose
/// air is its [`BodyEnvelopeDto`], and for a ring, a belt or the halo. Every quantity's name
/// carries its unit. The module documentation says what `null` and an absent member mean.
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
    /// airless body, and for a surface holding melt whose volatiles are gone; a `tenuous` body's
    /// own, under 100 Pa (P14.T53.c).
    ///
    /// It does not include the melt's own vapour ([`silicate_vapour`](Self::silicate_vapour)).
    pub surface_pressure_pa: f64,
    /// The gases at the surface by number, largest first with ties by registry order, their mole
    /// fractions summing to 1 to 10⁻⁹; empty for an airless body, or for a surface holding melt
    /// with no volatile air, which does not deny the melt's own vapour (`silicate_vapour`), and
    /// listed for a `tenuous` one (P14.T53.c).
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
    /// no rule computes. Below generator version 24 a regular moon's alone is computed. From 24
    /// (P14.T51.g.b) every solid surface's is, absent where the solve has no answer (liquid only
    /// under high-pressure ice, or a base or floor outside its ice's field, ice II's among them),
    /// and `null` on a moon whose heated shell lost its ice (P14.T56.g).
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<SubsurfaceOceanDto>>", optional)]
    pub subsurface_ocean: Modelled<SubsurfaceOceanDto>,
    /// The crater contract (P14.T48.c, with T48.f's impactors).
    pub craters: CraterContractDto,
    /// The wet epoch (P14.T48.b); `null` for a world never wet.
    pub wet_epoch: Option<WetEpochDto>,
    /// The vertical structure of the surface's air (P14.T24.e): `null` for a surface computed to
    /// hold no air, an airless body's or a melt's whose volatiles are gone, and absent from a
    /// server whose generator does not model it, every one before P14.T54.b's bump at generator
    /// version 24.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<VerticalStructureDto>>", optional)]
    pub vertical_structure: Modelled<VerticalStructureDto>,
    /// The aerosol and absorber inventory of the surface's air (P14.T24.c with T24.g): the empty
    /// inventory, never `null`, for a surface with no air (an airless body's, a melt's whose
    /// volatiles are gone), and absent from a server whose generator does not model it, every one
    /// before P14.T54.b's bump at generator version 24, and from a magma ocean that holds an air,
    /// whose clouds the generator does not model. It is absent from a `tenuous` surface too, whose
    /// haze and dust the generator does not model at 24 (P14.T55.n).
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
    /// own ice or a `Snowball`'s cover, whose water the partition does not hold); and below
    /// generator version 24, in `surface_liquids`, the ocean under a `Snowball`'s ice where no
    /// `H2O` liquid is listed, its water inside the `H2O` solid, which from 24 (P14.T53.c) is no
    /// surface liquid and is not listed. Nothing else.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<Vec<SurfaceCondensateDto>>>", optional)]
    pub condensates: Modelled<Vec<SurfaceCondensateDto>>,
    /// The crust by substance (P14.T51.c). Sent from P14.T54.a's batch for every surface whose
    /// generator computes it; absent from a server whose generator does not model it, and for a
    /// body whose record holds none.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<CrustCompositionDto>>", optional)]
    pub crust: Modelled<CrustCompositionDto>,
    /// The surface's modifiers (P14.T51.d with T51.j), absent from a server whose generator does
    /// not model them, every one before P14.T54.b's bump at generator version 24, and for a
    /// surface whose record gives no crust, no host star or, for a moon, no planet's field, or
    /// whose inputs the generator refuses (an unlit body at 0 K).
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<SurfaceModifiersDto>>", optional)]
    pub modifiers: Modelled<SurfaceModifiersDto>,
    /// The body's taxonomic class (P14.T51.e), by its kind, its crust's lid, its veneer (its
    /// modifiers' tholin cover and its condensates' perennial ice) and, for an ordinary chondrite,
    /// its weathering. Absent from a server whose generator does not model it, every one before
    /// P14.T54.b's bump at generator version 24, and for a body whose rule reads a member its
    /// record does not give (its crust, but for a Kuiper-belt body's, or its modifiers), never to
    /// be read as none; `null` for a planet, and for a member or moon the rules leave unclassed (a
    /// majority of uncovered ice, or a lid no class is given for); otherwise the class.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<TaxonomicClassDto>>", optional)]
    pub taxonomic_class: Modelled<TaxonomicClassDto>,
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
    /// (P14.T53.b's `steam_envelope` is never a surface's: such a body has its envelope section)
    /// and, from P14.T54.a, a `crust.melt_area_fraction` above 0. Elsewhere its absence says
    /// nothing. From P14.T55.b it is `null` on a surface holding no melt, and the vapour over
    /// melt (decision-backlog-2 §2.3).
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<SilicateVapourDto>>", optional)]
    pub silicate_vapour: Modelled<SilicateVapourDto>,
}

/// The parameters of an envelope's irradiated grey profile above its join to the adiabat: Guillot
/// 2010's (A&A 520, A27) Eq. 29 in the thermal optical depth τ, with τ = `κ_th` p₁ ÷ (n g) × (p ÷
/// p₁)ⁿ, p₁ = 1 bar (10⁵ Pa), n the optical depth's exponent and g the bulk section's surface
/// gravity. Guillot 2010 assumes constant opacities (its §6). The pressure-dependent form follows
/// because, with γ fixed, the moment equations are local in τ, so that Eq. 29 holds in τ: this
/// generator's consequence, not Guillot's statement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GuillotProfileDto {
    /// The thermal opacity at 1 bar, `κ_th`, m² kg⁻¹, positive: at a pressure p the opacity is
    /// `κ_th` (p ÷ 1 bar)ⁿ⁻¹.
    pub thermal_opacity_m2_per_kg: f64,
    /// n, the exponent of the optical depth's power law τ ∝ pⁿ, dimensionless, at least 1: 1 for an
    /// opacity independent of pressure, 2 for one proportional to it, as collision-induced
    /// absorption is (Robinson and Catling 2012, Astrophys. J. 757, 104, Eq. 6). 2 for every
    /// envelope from P14.T24.d.
    pub optical_depth_exponent: f64,
    /// γ, the visible opacity over the thermal, `κ_v` ÷ `κ_th`, dimensionless and positive: the
    /// same at every depth. `κ_v` is the grey profile's mean opacity to the host's light, a
    /// parameter of T(p), not an extinction at 550 nm.
    pub opacity_ratio: f64,
    /// f, the redistribution of the irradiation over the planet, dimensionless and positive: ¼ for
    /// an average over the whole sphere.
    pub redistribution_factor: f64,
}

/// A gas-envelope body's visible atmosphere (plan 14, P14.T24.d with P14.T52.d on P14.T50.b's
/// abundances; the simulation's `record::Envelope`): its internal heat and irradiation, its grey
/// profile joined to its dry adiabat, its composition at 1 bar, its condensate decks with their
/// modes, its photochemical hazes, its absorbers at 10 mbar and its condensables not formed.
///
/// Sent `ok` from P14.T54.b's bump at generator version 24 where the generator models it, and
/// `not_modelled` otherwise: below 24; for an envelope that is not hydrogen-dominated, or whose
/// 1-bar or 10-mbar level is hotter than the chemistry's records (6,000 K); for a body whose host
/// has no \[α/Fe\] of its own or no disc to place its formation, or whose inputs a step of the
/// chain refuses; and, from 24, for a solid body under a supercritical envelope (P14.T53.b part 2,
/// and from 25 part 3's steam envelope) until P14.T53.f. `not_applicable` for every other body,
/// whose surface section is the applicable one. Its shape is P14.T54.b.i's, fixed from 24's first
/// send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyEnvelopeDto {
    /// The condensable whose supercritical fluid the envelope is, by its registry key: `null` for
    /// an H₂/He envelope, a giant's or a sub-Neptune's (decision-absurd-states-24 R2.3).
    ///
    /// A solid body under a supercritical envelope (P14.T53.b parts 2 and 3, a steam envelope among
    /// them) is sent its envelope `not_modelled`, so no build sends this member other than `null`
    /// before P14.T53.f gives that envelope its model and this its value. Always present on an `ok`
    /// envelope, as `null` where none.
    pub supercritical: Option<String>,
    /// The internal temperature `T_int` the profile reads, K, positive: the record's internal heat
    /// (ruling 112.7) from 0.414 Jupiter masses, and below that blended with P14.T24.d's cooling
    /// law, which alone gives it below 0.3 Jupiter masses. Below 0.414 Jupiter masses it is
    /// therefore not the bulk section's (`T_eff`⁴ − `T_eq`⁴)^¼, whose `effective_temperature_k` is
    /// the record's own.
    pub internal_temperature_k: f64,
    /// The irradiation temperature `T_irr`, K, not negative: `T_irr`⁴ = `T_eq`⁴ ÷ f, from the bulk
    /// section's equilibrium temperature, which carries the Bond albedo, so that the profile
    /// radiates the bulk section's absorbed starlight.
    pub irradiation_temperature_k: f64,
    /// The grey profile's parameters.
    pub profile: GuillotProfileDto,
    /// The pressure at which the profile joins the dry adiabat, where its gradient dln T ÷ dln p
    /// first exceeds R ÷ `c_p`, Pa, positive and finite: with n = 2 and a positive `T_int` the
    /// gradient tends to ½ at depth, above any ideal gas's R ÷ `c_p`, so every envelope has one.
    pub join_pressure_pa: f64,
    /// The adiabat's nodes from `join_pressure_pa` to the larger of 100 bar (10⁷ Pa) and e^0.3
    /// times the join's pressure, the pressures ascending evenly in ln p (P14.T24.e's spacing), the
    /// first at the join at the grey profile's temperature there; between two nodes the
    /// temperature follows the power law between them, and below the last node the last
    /// segment's.
    pub adiabat: Vec<ProfileNodeDto>,
    /// The envelope's gases at 1 bar (10⁵ Pa) by number, quenched and trimmed by its decks, each
    /// at least 10⁻¹², largest first (ties in registry order), their mole fractions summing to 1
    /// to 10⁻⁹. It is the gas at that level alone, from which the decks form: the generator
    /// speciates the envelope at 1 bar and at its absorbers' 10 mbar only, and states no profile
    /// between or below them; above a deck its condensate's vapour is that deck's
    /// `vapour_mole_fraction_above`, by its law.
    pub composition: Vec<GasShareDto>,
    /// The gases' mean molar mass, g mol⁻¹, positive.
    pub mean_molar_mass_g_per_mol: f64,
    /// The gases' mixed `c_p` ÷ R at the 1-bar level's temperature, dimensionless and positive: for
    /// display, since no reader derives a lapse rate from it.
    pub heat_capacity_over_r: f64,
    /// The modes of its condensate decks (P14.T52.c's loop along the profile from 10 kbar, 10⁹ Pa,
    /// up to 1 Pa), each deck's in `condensate_decks`' order, each naming one of them as its
    /// `deck` and covering the whole sky, `coverage` 1 at generator version 24. A client refuses a
    /// mode here that names no listed deck, and one in `hazes` that names one; the split is part
    /// of the shape from the first send.
    pub decks: Vec<AerosolModeDto>,
    /// Its photochemical hazes' modes, on an envelope a star's ultraviolet lights (P14.T52.b's
    /// rules, in this order): the S₈ haze (Gao et al. 2017), soot at an equilibrium temperature
    /// between 600 and 950 K (Gao et al. 2020, tapered in below their grid's 700 K), the ice
    /// giants' haze from a CH₄ deck's base (Irwin et al. 2022) and the chromophore over an NH₃
    /// deck with no CH₄ deck above it (Braude et al. 2020). Each is of its own material, names no
    /// deck (`deck` `null`) and covers the whole sky, `coverage` 1 at generator version 24; the
    /// list is empty for an envelope no star lights. No other haze is modelled: a cool giant's
    /// stratospheric haze beyond these rules is not computed, so a lit envelope's list is these
    /// four rules' output, not a clear upper air.
    pub hazes: Vec<AerosolModeDto>,
    /// The hazes' optical depth at 550 nm, dimensionless and not negative: the sum of `hazes`'
    /// `tau_550`, in their order, over those that carry one; 0 where none does.
    pub haze_tau_550: f64,
    /// Its condensate decks, hottest base first, at most one a condensate, which `decks`' modes
    /// name (P14.T24.c's deck shape; decision-p14-t54b-open §2.2 (5)).
    pub condensate_decks: Vec<CondensateDeckDto>,
    /// Its absorbers at 10 mbar (10³ Pa; R08.T4.e's lines), in registry order, each species at
    /// most once: the atoms of P14.T52.a's metals (Na, K, Mg, Al, Si, Ca, Ti, V, Cr, Mn, Fe, Ni and
    /// Zn) and their first ions by their registry keys (`Na+`, `K+`, …), `TiO`, `VO`, `SiO`, `H`,
    /// `H-` and `e-`, each where its share of the 10-mbar gas (quenched, trimmed by the decks and
    /// renormalised) is at least 10⁻¹², as a slab of that share from 10 mbar to 1 Pa (`profile`
    /// `null`); and, on an envelope a star's ultraviolet lights, photochemical SO₂'s layer (Tsai et
    /// al. 2023), a slab too. A slab is the 10-mbar level's share of its species, stated from 10
    /// mbar up: a species that `composition` also holds is one gas read at two levels, so the slab
    /// neither adds to its 1-bar share above 10 mbar nor replaces it below. `FeH`, which has no
    /// thermochemistry, is never listed: it is not modelled.
    pub absorbers: Vec<AbsorberDto>,
    /// The condensables whose vapour its gas holds that its decks' loop labels rather than traps,
    /// each with why, in registry order.
    pub not_formed: Vec<NotFormedCondensateDto>,
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
            taxonomic_class: Modelled::NotModelled,
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

    /// A Jupiter's envelope, illustrative, on the shared fixture's Jupiter's bulk section (`T_eq`
    /// 111.6 K, `T_eff` 128.9 K): `T_int` (128.9⁴ − 111.6⁴)^¼ = 104.9 K and `T_irr` (4 × 111.6⁴)^¼
    /// = 157.8 K, the generator's grey profile (κ 2.87 × 10⁻³ m² kg⁻¹, n 2, γ 0.4, f ¼), the
    /// protosolar atmosphere depleted of helium (He/H₂ 0.157 and CH₄/H₂ 2.1 × 10⁻³ by number), the
    /// adiabat through 166.1 K at 1 bar, an ammonia-ice deck at 0.45–0.7 bar
    /// ([`ammonia_deck`], [`ammonia_condensate_deck`]), its red chromophore over it
    /// ([`chromophore_haze`]), a sodium slab at 10 mbar (a warmer giant's: the generator's Jupiter
    /// traps its sodium in a deep `Na₂S` deck and lists no absorber) and the generator's Jupiter's
    /// quartz, displaced by enstatite.
    pub(crate) fn jupiter_envelope() -> BodyEnvelopeDto {
        BodyEnvelopeDto {
            supercritical: None,
            internal_temperature_k: 104.9,
            irradiation_temperature_k: 157.8,
            profile: GuillotProfileDto {
                thermal_opacity_m2_per_kg: 2.87e-3,
                optical_depth_exponent: 2.0,
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
            hazes: vec![chromophore_haze()],
            haze_tau_550: 0.31,
            condensate_decks: vec![ammonia_condensate_deck()],
            absorbers: vec![AbsorberDto {
                species: "Na".to_owned(),
                column_m2: 1.05e19,
                base_pa: 1e3,
                top_pa: 1.0,
                profile: None,
            }],
            not_formed: vec![NotFormedCondensateDto {
                condensate: "SiO2".to_owned(),
                reason: NotFormedReasonDto::Displaced {
                    by: "MgSiO3".to_owned(),
                },
            }],
        }
    }

    /// The ammonia-ice deck's mode of [`jupiter_envelope`], illustrative, over the whole sky at its
    /// base's temperature on the envelope's adiabat (about 149 K at 0.7 bar).
    pub(crate) fn ammonia_deck() -> AerosolModeDto {
        AerosolModeDto {
            material: "NH3".to_owned(),
            phase: SubstancePhaseDto::Solid,
            deck: Some("NH3".to_owned()),
            shape: AerosolShapeDto::Crystal,
            size_distribution: Some(SizeDistributionDto::Gamma {
                effective_radius_m: 1e-6,
                effective_variance: 0.1,
            }),
            column_mass_kg_m2: Some(0.012),
            tau_550: Some(8.0),
            tau_basis: Some(AerosolTauBasisDto::PhaseTable),
            base_pa: 7e4,
            top_pa: 4.5e4,
            f_sed: 2.0,
            coverage: 1.0,
            temperature_k: Some(149.1),
            not_modelled: None,
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
            "tau_basis": "phase_table",
            "base_pa": 7e4,
            "top_pa": 4.5e4,
            "f_sed": 2.0,
            "coverage": 1.0,
            "temperature_k": 149.1,
        })
    }

    /// The ammonia-ice deck of [`jupiter_envelope`], which [`ammonia_deck`] names, illustrative:
    /// its vapour below about the generator's Jupiter's, 2.4 × 10⁻⁴, and its cold trap's above.
    pub(crate) fn ammonia_condensate_deck() -> CondensateDeckDto {
        CondensateDeckDto {
            condensate: "NH3".to_owned(),
            phase: SubstancePhaseDto::Solid,
            base_pa: 7e4,
            base_temperature_k: 149.1,
            top_pa: 4.5e4,
            vapour_mole_fraction_below: 2.4e-4,
            vapour_mole_fraction_above: 2e-7,
            f_sed: 2.0,
            extrapolated: false,
        }
    }

    pub(crate) fn ammonia_condensate_deck_json() -> Value {
        json!({
            "condensate": "NH3",
            "phase": "solid",
            "base_pa": 7e4,
            "base_temperature_k": 149.1,
            "top_pa": 4.5e4,
            "vapour_mole_fraction_below": 2.4e-4,
            "vapour_mole_fraction_above": 2e-7,
            "f_sed": 2.0,
            "extrapolated": false,
        })
    }

    /// The red chromophore of [`jupiter_envelope`], illustrative, after the generator's Jupiter's:
    /// Braude et al. 2020's column and particles, a well-mixed slab between its Gaussian's 1/e
    /// points about 0.2 bar, its optics at its base's temperature on the envelope's profile, and no
    /// deck of its own.
    pub(crate) fn chromophore_haze() -> AerosolModeDto {
        AerosolModeDto {
            material: "chromophore".to_owned(),
            phase: SubstancePhaseDto::Solid,
            deck: None,
            shape: AerosolShapeDto::Sphere,
            size_distribution: Some(SizeDistributionDto::Gamma {
                effective_radius_m: 5e-8,
                effective_variance: 0.05,
            }),
            column_mass_kg_m2: Some(6.3e-4),
            tau_550: Some(0.31),
            tau_basis: Some(AerosolTauBasisDto::Sphere),
            base_pa: 2.57e4,
            top_pa: 1.56e4,
            f_sed: 0.0,
            coverage: 1.0,
            temperature_k: Some(123.5),
            not_modelled: None,
        }
    }

    pub(crate) fn chromophore_haze_json() -> Value {
        json!({
            "material": "chromophore",
            "phase": "solid",
            "deck": null,
            "shape": "sphere",
            "size_distribution": {
                "type": "gamma",
                "effective_radius_m": 5e-8,
                "effective_variance": 0.05,
            },
            "column_mass_kg_m2": 6.3e-4,
            "tau_550": 0.31,
            "tau_basis": "sphere",
            "base_pa": 2.57e4,
            "top_pa": 1.56e4,
            "f_sed": 0.0,
            "coverage": 1.0,
            "temperature_k": 123.5,
        })
    }

    pub(crate) fn jupiter_envelope_json() -> Value {
        json!({
            "supercritical": null,
            "internal_temperature_k": 104.9,
            "irradiation_temperature_k": 157.8,
            "profile": {
                "thermal_opacity_m2_per_kg": 2.87e-3,
                "optical_depth_exponent": 2.0,
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
            "hazes": [chromophore_haze_json()],
            "haze_tau_550": 0.31,
            "condensate_decks": [ammonia_condensate_deck_json()],
            "absorbers": [
                {
                    "species": "Na",
                    "column_m2": 1.05e19,
                    "base_pa": 1e3,
                    "top_pa": 1.0,
                    "profile": null,
                },
            ],
            "not_formed": [
                { "condensate": "SiO2", "reason": { "type": "displaced", "by": "MgSiO3" } },
            ],
        })
    }

    /// A warm sub-Neptune's envelope, illustrative, of 10 M⊕ at 4.57 Gyr: P14.T24.d's cooling
    /// law's `T_int`, 84.13 K × (10 ÷ 95.161)^0.2744 = 45.34 K, `T_irr` 640 K (`T_eq` 452.5 K),
    /// water and methane beside the hydrogen and helium, and every list empty, the wire form of an
    /// envelope with none (the generator's would hold its refractories' and salts' decks, far
    /// below 100 bar).
    pub(crate) fn sub_neptune_envelope() -> BodyEnvelopeDto {
        BodyEnvelopeDto {
            internal_temperature_k: 45.34,
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
            hazes: Vec::new(),
            haze_tau_550: 0.0,
            condensate_decks: Vec::new(),
            absorbers: Vec::new(),
            not_formed: Vec::new(),
            ..jupiter_envelope()
        }
    }

    pub(crate) fn sub_neptune_envelope_json() -> Value {
        let mut wire = jupiter_envelope_json();
        for (key, value) in [
            ("internal_temperature_k", json!(45.34)),
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
            ("hazes", json!([])),
            ("haze_tau_550", json!(0.0)),
            ("condensate_decks", json!([])),
            ("absorbers", json!([])),
            ("not_formed", json!([])),
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

    /// An aerosol inventory, illustrative: its modes ([`inventory_modes`]) over its decks
    /// ([`inventory_decks`]), ozone on a profile beside a sulphur-dioxide slab, and two
    /// condensables not formed.
    fn inventory() -> AerosolInventoryDto {
        AerosolInventoryDto {
            modes: inventory_modes(),
            decks: inventory_decks(),
            not_formed: vec![
                NotFormedCondensateDto {
                    condensate: "CO2".to_owned(),
                    reason: NotFormedReasonDto::BulkGas,
                },
                NotFormedCondensateDto {
                    condensate: "SiO2".to_owned(),
                    reason: NotFormedReasonDto::Displaced {
                        by: "MgSiO3".to_owned(),
                    },
                },
            ],
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

    /// The modes of [`inventory`]: a water cloud of droplets, a Titan-like haze of aggregates (its
    /// column mass and temperature not modelled) and a hydrogen sulphide ice deck whose size and τ
    /// are not modelled for want of its solid's grain density.
    fn inventory_modes() -> Vec<AerosolModeDto> {
        vec![
            AerosolModeDto {
                material: "H2O".to_owned(),
                phase: SubstancePhaseDto::Liquid,
                deck: Some("H2O".to_owned()),
                shape: AerosolShapeDto::Sphere,
                size_distribution: Some(SizeDistributionDto::LogNormal {
                    effective_radius_m: 1e-5,
                    effective_variance: 0.1,
                }),
                column_mass_kg_m2: Some(0.05),
                tau_550: Some(7.5),
                tau_basis: Some(AerosolTauBasisDto::Sphere),
                base_pa: 85_000.0,
                top_pa: 60_000.0,
                f_sed: 1.0,
                coverage: 0.67,
                temperature_k: Some(281.5),
                not_modelled: None,
            },
            AerosolModeDto {
                material: "tholin".to_owned(),
                phase: SubstancePhaseDto::Solid,
                deck: None,
                shape: AerosolShapeDto::Aggregate,
                size_distribution: Some(SizeDistributionDto::Aggregate {
                    monomer_radius_m: 5e-8,
                    monomer_count: 3_000.0,
                    fractal_dimension: 2.0,
                    fractal_prefactor: 1.666_666_666_666_666_7,
                }),
                column_mass_kg_m2: None,
                tau_550: Some(0.4),
                tau_basis: Some(AerosolTauBasisDto::Rule),
                base_pa: 20_000.0,
                top_pa: 10.0,
                f_sed: 0.0,
                coverage: 1.0,
                temperature_k: None,
                not_modelled: Some(AerosolNotModelledDto::NoMassExtinction),
            },
            AerosolModeDto {
                material: "H2S".to_owned(),
                phase: SubstancePhaseDto::Solid,
                deck: Some("H2S".to_owned()),
                shape: AerosolShapeDto::Crystal,
                size_distribution: None,
                column_mass_kg_m2: Some(1e-4),
                tau_550: None,
                tau_basis: None,
                base_pa: 9_000.0,
                top_pa: 4_000.0,
                f_sed: 0.5,
                coverage: 0.67,
                temperature_k: Some(180.0),
                not_modelled: Some(AerosolNotModelledDto::NoGrainDensity {
                    phase: SubstancePhaseDto::Solid,
                    temperature_k: 180.0,
                }),
            },
        ]
    }

    /// The decks of [`inventory`], which its modes name.
    fn inventory_decks() -> Vec<CondensateDeckDto> {
        vec![
            CondensateDeckDto {
                condensate: "H2O".to_owned(),
                phase: SubstancePhaseDto::Liquid,
                base_pa: 85_000.0,
                base_temperature_k: 281.5,
                top_pa: 60_000.0,
                vapour_mole_fraction_below: 0.0131,
                vapour_mole_fraction_above: 3e-6,
                f_sed: 1.0,
                extrapolated: false,
            },
            CondensateDeckDto {
                condensate: "H2S".to_owned(),
                phase: SubstancePhaseDto::Solid,
                base_pa: 9_000.0,
                base_temperature_k: 180.0,
                top_pa: 4_000.0,
                vapour_mole_fraction_below: 2e-6,
                vapour_mole_fraction_above: 1e-8,
                f_sed: 0.5,
                extrapolated: true,
            },
        ]
    }

    fn inventory_json() -> Value {
        json!({
            "modes": inventory_modes_json(),
            "decks": [
                {
                    "condensate": "H2O",
                    "phase": "liquid",
                    "base_pa": 85_000.0,
                    "base_temperature_k": 281.5,
                    "top_pa": 60_000.0,
                    "vapour_mole_fraction_below": 0.0131,
                    "vapour_mole_fraction_above": 3e-6,
                    "f_sed": 1.0,
                    "extrapolated": false,
                },
                {
                    "condensate": "H2S",
                    "phase": "solid",
                    "base_pa": 9_000.0,
                    "base_temperature_k": 180.0,
                    "top_pa": 4_000.0,
                    "vapour_mole_fraction_below": 2e-6,
                    "vapour_mole_fraction_above": 1e-8,
                    "f_sed": 0.5,
                    "extrapolated": true,
                },
            ],
            "not_formed": [
                { "condensate": "CO2", "reason": { "type": "bulk_gas" } },
                { "condensate": "SiO2", "reason": { "type": "displaced", "by": "MgSiO3" } },
            ],
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

    /// The wire form of [`inventory_modes`].
    fn inventory_modes_json() -> Value {
        json!([
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
                    "tau_basis": "sphere",
                    "base_pa": 85_000.0,
                    "top_pa": 60_000.0,
                    "f_sed": 1.0,
                    "coverage": 0.67,
                    "temperature_k": 281.5,
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
                    "tau_550": 0.4,
                    "tau_basis": "rule",
                    "base_pa": 20_000.0,
                    "top_pa": 10.0,
                    "f_sed": 0.0,
                    "coverage": 1.0,
                    "not_modelled": { "type": "no_mass_extinction" },
                },
                {
                    "material": "H2S",
                    "phase": "solid",
                    "deck": "H2S",
                    "shape": "crystal",
                    "column_mass_kg_m2": 1e-4,
                    "base_pa": 9_000.0,
                    "top_pa": 4_000.0,
                    "f_sed": 0.5,
                    "coverage": 0.67,
                    "temperature_k": 180.0,
                    "not_modelled": {
                        "type": "no_grain_density",
                        "phase": "solid",
                        "temperature_k": 180.0,
                    },
                },
        ])
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

    /// The shared fixture's Earth's crust (`packages/protocol/fixtures/planetary.json`) in the
    /// form the server sends (P14.T54.a): the sim's `CrustComposition::derive` example's Earth,
    /// basalt ocean floor and granite continents over a buried ultramafic crust, its melt area 0.
    pub(crate) fn earth_crust() -> CrustCompositionDto {
        CrustCompositionDto {
            primary: Some("ultramafic".to_owned()),
            secondary: Some("basalt".to_owned()),
            tertiary: Some("granite".to_owned()),
            bulk: None,
            lid: CrustLidDto::Secondary,
            provinces: "basalt".to_owned(),
            redox: RedoxClassDto::Oxidised,
            mantle_redox_iw: 3.55,
            melt_area_fraction: 0.0,
        }
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

    /// P14.T54.b.d: an envelope names the condensable it is the supercritical fluid of, and an
    /// H₂/He envelope sends the member `null`, never leaves it out.
    #[test]
    fn an_envelope_names_its_supercritical_condensable_or_sends_null() {
        let water = BodyEnvelopeDto {
            supercritical: Some("H2O".to_owned()),
            ..sub_neptune_envelope()
        };
        let mut wire = sub_neptune_envelope_json();
        wire["supercritical"] = json!("H2O");
        assert_wire_form(&water, wire);
        let hydrogen = serde_json::to_value(jupiter_envelope()).unwrap();
        assert_eq!(hydrogen.get("supercritical"), Some(&Value::Null));
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
                "taxonomic_class" => {
                    surface.taxonomic_class = modelled(value, || TaxonomicClassDto::V);
                }
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
            ("taxonomic_class", json!("v")),
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
        for deck in &mut aerosols.decks {
            deck.condensate = key();
        }
        for absorber in &mut aerosols.absorbers {
            absorber.species = key();
        }
        aerosols.not_formed[0].condensate = key();
        aerosols.not_formed[1].reason = NotFormedReasonDto::Displaced { by: key() };
        surface.aerosols = Modelled::Value(aerosols);
        let mut aerosols = inventory_json();
        for mode in 0..3 {
            aerosols["modes"][mode]["material"] = json!(unknown);
            aerosols["modes"][mode]["deck"] = json!(unknown);
        }
        for listed in 0..2 {
            aerosols["decks"][listed]["condensate"] = json!(unknown);
            aerosols["absorbers"][listed]["species"] = json!(unknown);
        }
        aerosols["not_formed"][0]["condensate"] = json!(unknown);
        aerosols["not_formed"][1]["reason"]["by"] = json!(unknown);
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
        envelope.supercritical = Some(key());
        envelope.composition[0].species = key();
        envelope.decks[0].material = key();
        envelope.decks[0].deck = Some(key());
        envelope.hazes[0].material = key();
        envelope.condensate_decks[0].condensate = key();
        envelope.absorbers[0].species = key();
        envelope.not_formed[0].condensate = key();
        envelope.not_formed[0].reason = NotFormedReasonDto::Displaced { by: key() };
        let mut wire = jupiter_envelope_json();
        wire["supercritical"] = json!(unknown);
        wire["composition"][0]["species"] = json!(unknown);
        wire["decks"][0]["material"] = json!(unknown);
        wire["decks"][0]["deck"] = json!(unknown);
        wire["hazes"][0]["material"] = json!(unknown);
        wire["condensate_decks"][0]["condensate"] = json!(unknown);
        wire["absorbers"][0]["species"] = json!(unknown);
        wire["not_formed"][0]["condensate"] = json!(unknown);
        wire["not_formed"][0]["reason"]["by"] = json!(unknown);
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

    /// P14.T54.b.b: the τ(550) basis's six strings, closed from the first send
    /// (decision-p14-t54b-open §2.2).
    #[test]
    fn aerosol_tau_basis_strings() {
        assert_wire_strings(&[
            (AerosolTauBasisDto::Sphere, "sphere"),
            (AerosolTauBasisDto::PhaseTable, "phase_table"),
            (
                AerosolTauBasisDto::SpheresOutsidePhaseTable,
                "spheres_outside_phase_table",
            ),
            (AerosolTauBasisDto::SphereFallback, "sphere_fallback"),
            (AerosolTauBasisDto::GeometricLimit, "geometric_limit"),
            (AerosolTauBasisDto::Rule, "rule"),
        ]);
    }

    /// P14.T54.b.b: a mode's not-modelled reason, one closed enum tagged by `type`, each of its six
    /// variants' wire form (decision-p14-t54b-open §2.2).
    #[test]
    fn aerosol_not_modelled_wire_forms() {
        assert_wire_form(
            &AerosolNotModelledDto::NoGrainDensity {
                phase: SubstancePhaseDto::Liquid,
                temperature_k: 250.0,
            },
            json!({ "type": "no_grain_density", "phase": "liquid", "temperature_k": 250.0 }),
        );
        for (reason, tag) in [
            (
                AerosolNotModelledDto::NoCollisionConstants,
                "no_collision_constants",
            ),
            (AerosolNotModelledDto::NoCondensate, "no_condensate"),
            (
                AerosolNotModelledDto::NoSizeDistribution,
                "no_size_distribution",
            ),
            (
                AerosolNotModelledDto::OpticsNotModelled,
                "optics_not_modelled",
            ),
            (
                AerosolNotModelledDto::NoMassExtinction,
                "no_mass_extinction",
            ),
        ] {
            assert_wire_form(&reason, json!({ "type": tag }));
        }
    }

    /// P14.T54.b.b: a condensable not formed is sent with its reason, `bulk_gas`, `displaced` with
    /// the condensate that displaces it, or (P14.T52.f) `supercritical`.
    #[test]
    fn not_formed_wire_forms() {
        assert_wire_form(
            &NotFormedCondensateDto {
                condensate: "CO2".to_owned(),
                reason: NotFormedReasonDto::BulkGas,
            },
            json!({ "condensate": "CO2", "reason": { "type": "bulk_gas" } }),
        );
        assert_wire_form(
            &NotFormedReasonDto::Displaced {
                by: "CaTiO3".to_owned(),
            },
            json!({ "type": "displaced", "by": "CaTiO3" }),
        );
        assert_wire_form(
            &NotFormedCondensateDto {
                condensate: "N2".to_owned(),
                reason: NotFormedReasonDto::Supercritical,
            },
            json!({ "condensate": "N2", "reason": { "type": "supercritical" } }),
        );
    }

    /// P14.T54.b.b (decision-p14-t54b-open §2.2): a mode's member the generator does not model is
    /// absent, written with no key and never `null` or 0, and a mode read without one holds none;
    /// the empty inventory is every list empty.
    #[test]
    fn a_mode_s_unmodelled_members_are_absent_on_the_wire() {
        let fixture = inventory();
        let wire = serde_json::to_value(&fixture.modes[1]).unwrap();
        for member in ["column_mass_kg_m2", "temperature_k"] {
            assert!(wire.get(member).is_none(), "the haze's {member}: {wire}");
        }
        let wire = serde_json::to_value(&fixture.modes[2]).unwrap();
        for member in ["size_distribution", "tau_550", "tau_basis"] {
            assert!(
                wire.get(member).is_none(),
                "the H2S deck's {member}: {wire}"
            );
        }
        let wire = serde_json::to_value(&fixture.modes[0]).unwrap();
        assert!(wire.get("not_modelled").is_none(), "{wire}");
        assert_wire_form(
            &AerosolInventoryDto {
                modes: Vec::new(),
                decks: Vec::new(),
                absorbers: Vec::new(),
                not_formed: Vec::new(),
            },
            json!({ "modes": [], "decks": [], "absorbers": [], "not_formed": [] }),
        );
    }

    #[test]
    fn crust_lid_strings() {
        assert_wire_strings(&[
            (CrustLidDto::Primary, "primary"),
            (CrustLidDto::Secondary, "secondary"),
            (CrustLidDto::Bulk, "bulk"),
        ]);
    }

    /// P14.T51.e: every class at its first send, each its name in lower case.
    #[test]
    fn taxonomic_class_strings() {
        assert_wire_strings(&[
            (TaxonomicClassDto::A, "a"),
            (TaxonomicClassDto::B, "b"),
            (TaxonomicClassDto::C, "c"),
            (TaxonomicClassDto::Cb, "cb"),
            (TaxonomicClassDto::Cg, "cg"),
            (TaxonomicClassDto::Cgh, "cgh"),
            (TaxonomicClassDto::Ch, "ch"),
            (TaxonomicClassDto::D, "d"),
            (TaxonomicClassDto::K, "k"),
            (TaxonomicClassDto::L, "l"),
            (TaxonomicClassDto::O, "o"),
            (TaxonomicClassDto::Q, "q"),
            (TaxonomicClassDto::R, "r"),
            (TaxonomicClassDto::S, "s"),
            (TaxonomicClassDto::Sa, "sa"),
            (TaxonomicClassDto::Sq, "sq"),
            (TaxonomicClassDto::Sr, "sr"),
            (TaxonomicClassDto::Sv, "sv"),
            (TaxonomicClassDto::T, "t"),
            (TaxonomicClassDto::V, "v"),
            (TaxonomicClassDto::X, "x"),
            (TaxonomicClassDto::Xc, "xc"),
            (TaxonomicClassDto::Xe, "xe"),
            (TaxonomicClassDto::Xk, "xk"),
            (TaxonomicClassDto::P, "p"),
            (TaxonomicClassDto::Red, "red"),
            (TaxonomicClassDto::Neutral, "neutral"),
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
        const DIMENSIONLESS: [&str; 28] = [
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
            "vapour_mole_fraction_below",
            "coverage",
            "relative_mixing_ratio",
            "optical_depth_exponent",
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
