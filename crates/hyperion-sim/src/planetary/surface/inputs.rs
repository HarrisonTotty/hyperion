//! What the coarse pass reads of a body: [`CoarseInputs`], from plan 14's record through
//! [`CoarseInputs::for_body`] or from [`CoarseInputsBuilder`] (plan R09, Design note 3).
//!
//! [`CoarseInputs`] is the whole of what the pass reads, so that the field is a pure function of
//! the surface seed and these values (the brainstorm's "What the generator already owes us"):
//! the body's figure and gravity, plan 14's hypsometric standard deviation `σ_h`, its liquids and
//! ices by substance, each ice's cold-trapped part with what its cold traps read of its star and
//! ground (R09.T13.f), its mean surface temperature with its signed contrasts, its spin and the
//! orbit that sets its seasons, the light of its hosts, its air, its surface state and material,
//! its tectonic regime with the continental fraction, its volcanism, heat flow and surface age,
//! its crater contract, its wet epoch, its climate regime, and (decision-composition §1.7, §7.4)
//! its crust and condensates by substance and its palette: the substances its field names, each
//! with the values the server resolves from plan 14's registry, its deposits among them.
//!
//! Plan 14's record carries all of these to the pass from P14.T54.a: `record::Surface`, the
//! section they arrive in, carries the conditions and figures from P14.T48.e, and P14.T51.a's
//! condensates and P14.T51.c's crust from P14.T54.a. [`CoarseInputs::for_body`] reads them, with
//! the palette's resolution from the registry's rows of the substances they name (P14.T49.b–c,
//! T49.h and T49.i; the `palette` module), and answers `Ok` for every generated body with a solid
//! surface, once it has checked a moon's planet's record (R09.T10.f), but the recorded gaps,
//! which it answers not modelled: a crust the record does not model, an unlit record's mean
//! surface temperature of 0 K, and a substance whose entry the registry does not resolve (solid
//! argon's density, which it has not read). [`CoarseInputs::from_record`] is the reading of the
//! rest (R09.T10.e): every other value from the record's sections at [`RECORD_TIME`], a moon's
//! seasons from its planet's record, with the crust, condensates and palette given as arguments,
//! the record's own from `for_body`.
//! The tests and the reference worlds build their inputs with [`CoarseInputsBuilder`], each value
//! a plain argument (Design note 3, "Until an ask lands"). The server serves no generated body's
//! field until its serve gate opens (P14.T54.a's `GENERATED_SURFACES_FROM`, expected at 24), so
//! that R09's and R10's changes re-bless at the version in force until then.
//! The types here are the pass's own vocabulary for those values, each named after the plan-14
//! task that supplies it, and wherever the pass computes a figure plan 14 also states, the result
//! is constrained to plan 14's value.
//!
//! Every substance is named by its registry key, [`SubstanceKey`] (decision-composition §1.1),
//! the key the field's palette carries. The record names substances by P14.T49.a's
//! [`SubstanceId`], and the reading turns each into its row's key (R09's Risks, "Deviations in
//! T10, as built").

use core::f64::consts::{FRAC_PI_2, TAU};
use core::num::FpCategory;
use std::error::Error;
use std::fmt;

use hyperion_surface::craters::CraterParams;
use hyperion_surface::field::{
    BodyRef, BuildPaletteError, ClimateModelKind, Crust, MaterialPalette, PaletteRole,
};
use hyperion_surface::spheroid::Spheroid;
use hyperion_surface::substance_key::SubstanceKey;

use super::palette::{self, ResolveBodyPaletteError};
use crate::galaxy::consts::YEARS_PER_GIGAYEAR;
use crate::id::BodyId;
use crate::math;
use crate::orbit::{BuildOrbitError, Eccentricity, KeplerElements, Orientation, seconds_between};
use crate::planetary::chem::partition::{self, CondensateEntry};
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::frames::BodyFixedFrame;
use crate::planetary::hooks::cold_traps::trap_threshold;
use crate::planetary::hooks::crust::CrustComposition;
use crate::planetary::hooks::surface::{SeasonalSpin, SeasonsFrom};
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::{
    BodyKind, BodyOrbit, BodyRecord, Modelled, RecordSection, Section, Surface,
};
use crate::planetary::{BodyIndex, SystemContext};
use crate::substance::{SubstanceId, by_key};
use crate::time::UniverseTime;
use crate::units::{
    Gigayears, GravitationalParameter, Kelvin, KilogramsPerSquareMetre, Metres,
    MetresPerSecondSquared, Pascals, Radians, Seconds, WattsPerSquareMetre, Years,
};

/// One substance's share of the surface, as plan 14's `surface_liquids` and `surface_ices` list
/// them (P14.T24.b, by substance per decision-composition §1.10), or the cold-trapped part of an
/// ice's ([`CoarseInputs::cold_traps`], P14.T51.f).
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaShare {
    /// The substance.
    pub substance: SubstanceKey,
    /// The share of the body's surface it covers all year, 0 (not −0) to 1: the area the climate
    /// step places it over, for an ice its zonal part coldest cells first by their annual
    /// radiative mean (R09.T13.d), and its cold-trapped part in each cell's cold-trap fraction
    /// (R09.T13.f), so cover that comes and goes with the seasons is the climate's, not this
    /// share.
    pub area_fraction: f64,
}

/// One gas of the air at the surface, as P14.T24.a lists them.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasShare {
    /// The gas.
    pub substance: SubstanceKey,
    /// Its mole fraction at the surface, above 0 and at most 1.
    pub mole_fraction: f64,
}

/// How the body turns under its hosts: P14.T14's body-fixed frame and rotation law as the climate
/// reads them (Design note 8).
///
/// The field's cells are in that body-fixed frame: its third axis, the cube's +z, is the pole, and
/// its first, +x, the prime meridian, which P14.T14 sets facing the primary at pericentre on a
/// locked body (`frames`). So a world locked 1:1 to its seasonal host has its substellar axis on
/// +x, and no member states it; any other resonance moves the substellar point, which the
/// rotation period, the solar day and the seasonal orbit give.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spin {
    /// The angle between the spin axis and the normal of the seasonal orbit
    /// ([`SeasonalOrbit`]), 0 to π: above π ÷ 2 the body turns retrograde.
    pub obliquity: Radians,
    /// The sidereal rotation period, seconds, finite and positive: the rate Ω the climate's
    /// transport scales with (Design note 8's transport law).
    pub rotation_period: Seconds,
    /// The mean solar day, seconds, finite and positive, or `None` where the hosts stand still in
    /// the sky, a 1:1 lock to the host of the seasonal orbit. The slow-rotator onset (P14.T48.d)
    /// reads this or the sidereal period: Yang et al. 2014's Table 1 states sidereal periods.
    pub solar_day: Option<Seconds>,
    /// Where on the seasonal orbit the northern spring equinox falls, as a true anomaly from
    /// periapsis in the orbit's direction of motion, radians, 0 to below 2π: the phase of the
    /// seasons against the distance cycle (77.05° on Earth, 180° less its perihelion longitude
    /// 102.947°, the J2000 mean element of NASA's Earth fact sheet; 109.0° on Mars, whose
    /// perihelion falls at Ls 251.0°, Allison and McEwen 2000, PSS 48, 215, eqs. 16–17, as Mars24
    /// implements them). North is the frame's pole, the spin's angular momentum by the right-hand
    /// rule and the field's +z: for a retrograde spin it is the IAU's south (Venus's, Uranus's),
    /// and a phase compared with IAU-labelled seasons is half a turn off there.
    pub equinox_true_anomaly: Radians,
}

/// The orbit that sets the body's seasons, the sun's declination and distance: the body's own
/// about its star or stars, or its planet's for a moon (`decision-r09-t2.md` item 1).
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeasonalOrbit {
    /// The orbit's eccentricity, from 0 (not −0) to below 1: the field's months are twelve equal
    /// spans of its eccentric anomaly from periapsis, and the header carries it as its
    /// `season_eccentricity`.
    pub eccentricity: f64,
    /// The orbital period, seconds, finite and positive: the length of the body's year.
    pub period: Seconds,
}

/// A body's tectonic regime: plan 14's, P14.T24.b's (science-p14-t24b-laws §1.1), with every
/// variant the wire pins (decision-composition §1.10), re-exported here for the pass.
pub use crate::planetary::hooks::figures::TectonicRegime;

/// The span over which the surface held liquid, plan 14's volatile history (P14.T48.b), as times
/// before the present.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetEpoch {
    /// The liquid that flowed, a registry row with a liquid density: plan 14's
    /// `WetEpoch::liquid`, water on every epoch plan 14 computes today. On a world dry now the
    /// erosion's fluid (its erodibility's density and bed factor) and the paleo-inventory's
    /// volume are this liquid's, since the present main liquid may be none
    /// (decision-r09-t14-choices Q6a).
    pub liquid: SubstanceId,
    /// When the epoch began, thousands of millions of years ago, at or after `end`.
    pub start: Gigayears,
    /// When it ended, thousands of millions of years ago: zero for a world wet now.
    pub end: Gigayears,
    /// The time channel-forming flow ran at a typical point of the surface over the epoch, over
    /// the reference flood intermittency of 5% (Hoke, Hynek and Tucker 2011's storms, ruled in
    /// science-hoke.md), in Earth-equivalent years: the stream-power solver's t (Design note 9,
    /// as corrected by R09.T0.b), plan 14's `WetEpoch::effective_flow`. Finite and non-negative.
    pub effective_flow: Years,
    /// The epoch's runoff at the reference flood intermittency, metres of liquid a Julian year,
    /// finite and non-negative: the storms' runoff while they flow times 0.05 (science-hoke.md
    /// §3), plan 14's `WetEpoch::runoff_m_per_year`. On a world dry now it weights the drainage
    /// area, uniform over the cells, since no past climate is modelled (Design note 9;
    /// science-hoke.md §3: "`WetEpoch` therefore gains `runoff` (m a⁻¹)").
    pub runoff_m_per_year: f64,
    /// The liquid inventory then, as its mass over the body's whole surface, M ÷ 4πR², finite and
    /// non-negative: what the pass places as a paleo-sea or spreads through closed basins, by the
    /// liquid's density (Design note 9). A mass, so that it needs no density to be stated.
    pub paleo_inventory: KilogramsPerSquareMetre,
}

impl From<&crate::planetary::hooks::figures::WetEpoch> for WetEpoch {
    /// Plan 14's wet epoch (P14.T48.b) in the pass's vocabulary, as
    /// [`CoarseInputs::from_record`] reads the record's figures: its liquid, its span, its
    /// effective flow, its runoff and its paleo-inventory, each the record's to the bit.
    fn from(epoch: &crate::planetary::hooks::figures::WetEpoch) -> Self {
        Self {
            liquid: epoch.liquid(),
            start: epoch.start(),
            end: epoch.end(),
            effective_flow: epoch.effective_flow(),
            runoff_m_per_year: epoch.runoff_m_per_year(),
            paleo_inventory: epoch.paleo_inventory(),
        }
    }
}

/// How much of the climate's heat the air moves from day to night, the classifier's thermal
/// regime: plan 14's, P14.T48.d's, re-exported here for the pass.
pub use crate::planetary::hooks::surface::ThermalRegime;

/// What drives the climate's cycle, the classifier's forcing: plan 14's, P14.T48.d's, re-exported
/// here for the pass.
pub use crate::planetary::hooks::surface::Forcing;

/// A body's climate regime, the classifier of the rendering brainstorm's open question 9
/// (P14.T48.d): its three fields and the coarse model it names.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClimateRegime {
    /// How much heat the air moves.
    pub thermal: ThermalRegime,
    /// What drives the cycle.
    pub forcing: Forcing,
    /// The climate's condensable: the condensable with the largest partial pressure whose phase
    /// changes inside the climate's range, or `None` (P14.T48.d, by registry loop; signoff-3.md,
    /// item 1.12): water on Earth, carbon dioxide on Mars, methane on a Titan.
    pub condensable: Option<SubstanceKey>,
    /// The coarse model the regime names, which the field's header carries (Design note 8).
    pub model: ClimateModelKind,
}

impl From<&crate::planetary::hooks::surface::ClimateRegime> for ClimateRegime {
    /// Plan 14's regime in the pass's vocabulary, its condensable by its registry row's key, as
    /// [`CoarseInputs::for_body`] reads the record from P14.T48.e.
    fn from(regime: &crate::planetary::hooks::surface::ClimateRegime) -> Self {
        Self {
            thermal: regime.thermal(),
            forcing: regime.forcing(),
            condensable: regime.condensable().map(|id| id.substance().key()),
            model: regime.model(),
        }
    }
}

/// Which of a body's crusts, or its bulk material, is at its surface, the lid's (P14.T51.c's `lid`;
/// science-composition-crusts.md §5 and §10.1 item 3d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LidCrust {
    /// The primary crust, where nothing has resurfaced it (the Moon's highlands, a Ceres).
    Primary,
    /// The secondary crust, over any buried primary one (Mars, Mercury, Venus, Earth's oceans).
    Secondary,
    /// The undifferentiated body's own material.
    Bulk,
}

/// A body's crust by substance, P14.T51.c's crust composition as the pass reads it (its redox
/// class is not read here): a differentiated body's crusts, or an undifferentiated body's own
/// material alone, and which of them the surface shows.
///
/// The classes are by mode of origin (Taylor and McLennan 2009, Planetary Crusts, Cambridge
/// University Press, its chapters' titles and summaries read; science-r09-drainage.md §5, whose
/// reading of a frozen ocean's shell as primary is an analogy, of medium confidence): a crust
/// crystallised from the body's first global liquid is primary, whatever the liquid, and a solid
/// mantle's partial melt is secondary. The inputs take one of two forms
/// (science-composition-crusts.md §1):
///
/// - **a differentiated body's crusts:** at least one of the primary and the secondary, and it may
///   lack either: the reference Earth and Mars name no primary crust, and a Ceres, whose rock
///   "avoided igneous differentiation" (Fu et al. 2017, EPSL 476, 153), has no secondary;
/// - **an undifferentiated body's bulk material alone:** its accreted, never-melted rock or
///   ice–dust (a chondrite, a comet's dust mantle; or, on a fragment of a differentiated body, its
///   exposed mantle or core, which no rule produces yet), with no primary, secondary or tertiary
///   crust beside it. It is not a crust in Taylor's sense.
///
/// The lid ([`lid`](Self::lid)) names which of them is at the surface: the primary crust where
/// nothing has resurfaced it, the secondary over any buried primary one, or the bulk material
/// (science-composition-crusts.md §5). A buried primary crust stays in the inputs, being what
/// large craters excavate.
///
/// The lithologies name the palette entries of the field's [`Crust`] variants
/// (decision-composition §1.7), as [`CoarseInputs::crust_palette`] states: `Continental` the
/// tertiary crust's, `Oceanic` the secondary's (R09.T12.a), or none without one, `Lid` the entry of
/// the crust or bulk material `lid` names, and `Province` the provinces'.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them, refusing a crust
/// with neither a primary nor a secondary crust nor a bulk material
/// ([`BuildCoarseInputsError::NoCrust`]), a bulk material beside a crust
/// ([`BuildCoarseInputsError::BulkBesideCrust`]), a lid naming a crust or bulk material the body
/// does not have ([`BuildCoarseInputsError::LidAbsent`]), and a lithology the palette has no entry
/// for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrustInputs {
    /// The primary crust, crystallised from the body's first global liquid, buried or at the
    /// surface, as `lid` says: a magma ocean's flotation crust (`feldspathic` on the Moon), or a
    /// frozen ocean's shell (`phyllosilicate` on a Ceres, `H2O` on an icy body); or `None`.
    pub primary: Option<SubstanceKey>,
    /// The secondary crust, a solid mantle's partial melt (`basalt` by default), or `None` on a
    /// body whose mantle never melted (a Ceres).
    pub secondary: Option<SubstanceKey>,
    /// The tertiary crust, from remelting of the secondary (`granite`, only where the continental
    /// fraction is above zero), or `None`.
    pub tertiary: Option<SubstanceKey>,
    /// An undifferentiated body's own material at its surface (`chondrite_h`, `cometary`), with no
    /// crust beside it, or `None`: not a crust in Taylor's sense
    /// (science-composition-crusts.md §1).
    pub bulk: Option<SubstanceKey>,
    /// Which crust or bulk material the surface shows: [`Lid`](Crust::Lid) takes its entry, and
    /// the melt test its solidus.
    pub lid: LidCrust,
    /// The volcanic provinces' lithology.
    pub provinces: SubstanceKey,
    /// The share of the surface above a silicate lid's solidus, the lid's crust or bulk
    /// material's, 0 to 1 (a locked lava world's dayside pool); 0 for an ice lid, whose liquid is
    /// a condensate.
    ///
    /// R09.T13.c applies the solidus per cell. The lid's solidus amends science-r09-drainage.md
    /// §5.2 item 4's "the secondary's, or the primary's where there is none", which on a Moon
    /// tested its maria's solidus against its highland surface (science-composition-crusts.md §5).
    pub melt_area_fraction: f64,
}

/// The phase a condensate lies in at the surface (P14.T51.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CondensatePhase {
    /// Solid: below the triple point, or a solution's eutectic.
    Solid,
    /// Liquid, inside its window at the surface pressure.
    Liquid,
    /// Supercritical, above its critical point, with no interface.
    Supercritical,
}

/// Whether a condensate stays through the year (P14.T51.a's per-phase `seasonal`,
/// [`Condensate::is_seasonal_as`](crate::planetary::chem::partition::Condensate::is_seasonal_as)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Persistence {
    /// It stays all year: the warmest zone keeps its phase on the surface.
    Perennial,
    /// The warmest zone takes its phase off the surface, so part of it comes and goes: a solid
    /// that melts there, or, held toward its cold trap, sublimes there at its own partial
    /// pressure; or a liquid that boils there at the surface pressure
    /// (decision-p14-t51a-partition §3). Earth's ocean is perennial and its ice seasonal, Mars's
    /// carbon dioxide ice seasonal and its water ice perennial. It is a spatial proxy for the
    /// seasons, from the orbit-averaged climate's warmest zone, until P14.T55.e's frost by month.
    Seasonal,
}

/// One condensate at the surface, P14.T51.a's partition as the pass reads it.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Condensate {
    /// The substance.
    pub substance: SubstanceKey,
    /// Its phase where it lies.
    pub phase: CondensatePhase,
    /// The condensed reservoir as its mass over the body's whole surface, M ÷ 4πR² (P14.T51.a's
    /// `M_s − p_s 4πR² ÷ g`, per unit area), finite and non-negative, not −0: a mass, so that
    /// substances of different densities sort as plan 14 sorts them.
    pub reservoir: KilogramsPerSquareMetre,
    /// The share of the surface it covers all year, 0 (not −0) to 1, as
    /// [`AreaShare::area_fraction`].
    pub area_fraction: f64,
    /// Whether it stays through the year.
    pub persistence: Persistence,
}

impl From<partition::CondensatePhase> for CondensatePhase {
    /// Plan 14's phase (P14.T51.a) in the pass's vocabulary.
    fn from(phase: partition::CondensatePhase) -> Self {
        match phase {
            partition::CondensatePhase::Solid => Self::Solid,
            partition::CondensatePhase::Liquid => Self::Liquid,
            partition::CondensatePhase::Supercritical => Self::Supercritical,
        }
    }
}

impl From<&CondensateEntry> for Condensate {
    /// Plan 14's published condensate (P14.T51.a) in the pass's vocabulary, as
    /// [`CoarseInputs::for_body`] reads the record from P14.T54.a: its substance by its row's key,
    /// its phase, its reservoir and its area to the bit, and seasonal where the record says so,
    /// perennial otherwise. Its cold-trapped part is not read: the record carries none until
    /// P14.T54.b ([`CoarseInputs::cold_traps`]).
    fn from(entry: &CondensateEntry) -> Self {
        Self {
            substance: entry.substance().substance().key(),
            phase: CondensatePhase::from(entry.phase()),
            reservoir: entry.reservoir(),
            area_fraction: entry.area_fraction(),
            persistence: if entry.is_seasonal() {
                Persistence::Seasonal
            } else {
                Persistence::Perennial
            },
        }
    }
}

/// What a body's cold traps read of its star and its ground beyond the other inputs (P14.T51.f;
/// R09.T13.f): the star's angular radius at periapsis and the bare ground's Bond albedo. The
/// greatest declination, the flux at periapsis and the heat flow come from
/// [`CoarseInputs::spin`], [`CoarseInputs::host_flux`] with the seasonal orbit's eccentricity, and
/// [`CoarseInputs::heat_flow`], by plan 14's own arithmetic (`ColdTrapGround`).
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColdTrapInputs {
    /// The angular radius α of the star the cold traps see, from the body at its seasonal orbit's
    /// periapsis, radians in [0, π/2), zero or with a normal square (`cold_trap::Body`'s rule).
    /// It is plan 14's brightest zone star, and for a moon its planet's periapsis: 0.27° for the
    /// Sun from the Moon, 0.87° from Mercury at perihelion. Zero treats the star as a point,
    /// Hayne et al. 2021's convention. On Mercury the disk raises the polar ground's annual
    /// maximum from 110 to 172 K.
    pub star_angular_radius: Radians,
    /// The Bond albedo of the bare ground the cold traps' zonal annual maximum and shadows read,
    /// in [0, 1), dimensionless. It is plan 14's (P14.T51.f's; the lid regolith's about the
    /// host's bands from P14.T51.i), not the climate model's `SurfaceState::albedo`.
    pub bond_albedo: f64,
}

/// The ground's space weathering, the field header's paired `maturity` and `weathering_rate`
/// (decision-p14-t51d-weathering §2.3: present together or not at all), plan 14's
/// `SurfaceModifiers` (P14.T51.d), which [`CoarseInputs::from_record`] reads from the record's
/// modifiers where they are modelled (P14.T54.b.c).
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundWeathering {
    /// The background regolith's maturity `m_body`, dimensionless in [+0, 1] (a −0 is refused, as
    /// the header refuses it): 0 for fresh ground, 1 for the Moon's mature background.
    pub maturity: f64,
    /// The weathering rate k, the body's surface-reaching weathering dose rate relative to the
    /// Moon's present one, dimensionless, finite and at least +0 (a −0 is refused), with no upper
    /// bound.
    pub weathering_rate: f64,
}

/// The most condensates plan 14 lists for a body (P14.T51.a).
pub const MAX_CONDENSATES: usize = 8;

/// How far a sum of shares may exceed one: the rounding of a few additions.
const SHARE_TOLERANCE: f64 = 1e-9;

/// How far the crater contract's gravity may differ from the body's, relative.
const GRAVITY_TOLERANCE: f64 = 1e-12;

/// Everything the coarse pass reads of a body (Design note 3), validated.
///
/// Built by [`CoarseInputs::builder`] (every test, and the reference worlds of `reference`, behind
/// the crate's `testing` feature) or by [`CoarseInputs::for_body`] from plan 14's record.
/// [`coarse_pass`](super::coarse_pass)'s example builds one.
#[derive(Debug, Clone, PartialEq)]
pub struct CoarseInputs {
    body: BodyRef,
    figure: Spheroid,
    gravity: MetresPerSecondSquared,
    sigma_h: Metres,
    liquids: Vec<AreaShare>,
    ices: Vec<AreaShare>,
    cold_traps: Vec<AreaShare>,
    cold_trap_inputs: Option<ColdTrapInputs>,
    mean_surface_temperature: Kelvin,
    equator_pole_contrast_k: f64,
    day_night_contrast_k: f64,
    spin: Spin,
    host_flux: WattsPerSquareMetre,
    seasonal_orbit: Option<SeasonalOrbit>,
    surface_pressure: Pascals,
    gases: Vec<GasShare>,
    surface_state: SurfaceState,
    surface_material: SurfaceMaterial,
    tectonics: TectonicRegime,
    continental_fraction: f64,
    volcanism: f64,
    heat_flow: WattsPerSquareMetre,
    surface_age: Gigayears,
    province_surface_age: Gigayears,
    craters: CraterParams,
    wet_epoch: Option<WetEpoch>,
    climate: ClimateRegime,
    crust: CrustInputs,
    condensates: Vec<Condensate>,
    palette: MaterialPalette,
    crust_palette: [Option<u8>; 4],
    lid_entry: u8,
    main_liquid: Option<u8>,
    ground_inertia_j_per_m2_k_sqrt_s: Option<f64>,
    weathering: Option<GroundWeathering>,
    organic_cover: Option<f64>,
}

/// Why a record's sections cannot give a body's inputs: the surface section's tag, or a section
/// the pass reads not modelled or withheld, which [`CoarseInputs::for_body`] and
/// [`CoarseInputs::from_record`] answer as [`ReadCoarseInputsError::Section`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceInputsError {
    /// The body has no solid surface: its record's surface section is
    /// [`Section::NotApplicable`], as a gas giant's, an ice giant's or (from P14.T48.e) a
    /// sub-Neptune's is.
    NoSolidSurface,
    /// A section the pass reads is not modelled by this generator version.
    NotModelled(RecordSection),
    /// A section the pass reads is withheld by the record's detail level: the pass reads a full
    /// record.
    NotResolved(RecordSection),
}

impl fmt::Display for SurfaceInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSolidSurface => f.write_str("the body has no solid surface"),
            Self::NotModelled(section) => write!(f, "the body's {section} section is not modelled"),
            Self::NotResolved(section) => {
                write!(f, "the record withholds the body's {section} section")
            }
        }
    }
}

impl Error for SurfaceInputsError {}

/// The time a body's record is read at for its coarse inputs: the epoch.
///
/// The field is keyed without a time, so its inputs come from the record at one time, the same
/// whoever asks and whenever (R09.T17). The server's `surface::RECORD_TIME`, at which it fetches
/// the records, is this constant re-exported. [`CoarseInputs::from_record`] reads the body's spin
/// and its seasonal orbit at it, so the records it is given are the snapshot at it. The epoch is
/// R09.T17's choice and decision-bump-plan-after-22's lean; P14.T54.a settles it, and from the
/// serve gate's bump it is part of the payload's contract (ibid. §1.4).
pub const RECORD_TIME: UniverseTime = UniverseTime::EPOCH;

/// Whose orbit sets the seasons of the body [`CoarseInputs::from_record`] reads, with the
/// planet's record where it is a moon's planet's (`decision-r09-t2.md` item 1).
///
/// The record-bearing form of plan 14's
/// [`SeasonsFrom`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeasonsFromRecord<'a> {
    /// The body's own orbit: a planet's, a dwarf planet's or a belt member's about its host.
    Own,
    /// The orbit of a moon's planet about the planet's host, from the planet's record at
    /// [`RECORD_TIME`].
    Planet(&'a BodyRecord),
}

/// Why [`CoarseInputs::for_body`] or [`CoarseInputs::from_record`] cannot give a body's inputs.
///
/// A server answers [`Section`](Self::Section) as the body's own state, and every other variant as
/// a fault of its own reading: a planet's record not given or not the moon's, a record not taken
/// at [`RECORD_TIME`], or inputs the builder refuses (decision-backlog-2 §3.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReadCoarseInputsError {
    /// A section the pass reads has no solid surface, is not modelled or is withheld.
    Section(SurfaceInputsError),
    /// The body is a moon of this planet, whose orbit sets the moon's seasons, and no planet's
    /// record was given: `for_body`'s `None`, or [`SeasonsFromRecord::Own`].
    PlanetMissing(BodyId),
    /// The record given as the body's planet is not the planet it orbits: another body, or a
    /// body of another system.
    NotItsPlanet {
        /// The planet the body orbits, or `None` for a body that is not a moon, whose seasons are
        /// its own orbit's.
        planet: Option<BodyId>,
        /// The body whose record was given.
        given: BodyId,
    },
    /// The record's seasonal orbit does not hold at [`RECORD_TIME`]: it holds only until that
    /// time or before, or its drift cell (P14.T45.a) starts after it, as for a record taken at
    /// another time.
    NotAtRecordTime,
    /// The seasonal orbit's drift model at [`RECORD_TIME`], inside its cell, gives no bound orbit.
    SeasonalOrbit(BuildOrbitError),
    /// The values read, with the crust, condensates and palette given, break a rule of
    /// [`CoarseInputsBuilder::build`]: a palette with no entry for a substance on the ground or a
    /// lithology, a crust the builder refuses, or a record's value out of its range.
    Build(BuildCoarseInputsError),
}

impl fmt::Display for ReadCoarseInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Section(error) => error.fmt(f),
            Self::PlanetMissing(planet) => write!(
                f,
                "a moon of body {planet} has its planet's seasons, and no planet's record was given"
            ),
            Self::NotItsPlanet {
                planet: Some(planet),
                given,
            } => write!(
                f,
                "body {given}'s record was given as the planet of a moon of body {planet}"
            ),
            Self::NotItsPlanet {
                planet: None,
                given,
            } => write!(
                f,
                "body {given}'s record was given as the planet of a body that is not a moon"
            ),
            Self::NotAtRecordTime => {
                f.write_str("the record's seasonal orbit does not hold at the record's time")
            }
            Self::SeasonalOrbit(_) => {
                f.write_str("the seasonal orbit at the record's time is not a bound orbit")
            }
            Self::Build(_) => f.write_str("the coarse inputs read from the record were refused"),
        }
    }
}

impl Error for ReadCoarseInputsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            // Its text is the section's own, so the section's error is not its cause again.
            Self::Section(error) => error.source(),
            Self::PlanetMissing(_) | Self::NotItsPlanet { .. } | Self::NotAtRecordTime => None,
            Self::SeasonalOrbit(error) => Some(error),
            Self::Build(error) => Some(error),
        }
    }
}

impl From<SurfaceInputsError> for ReadCoarseInputsError {
    fn from(error: SurfaceInputsError) -> Self {
        Self::Section(error)
    }
}

impl From<BuildCoarseInputsError> for ReadCoarseInputsError {
    fn from(error: BuildCoarseInputsError) -> Self {
        Self::Build(error)
    }
}

impl CoarseInputs {
    /// A builder with no value set.
    #[must_use]
    pub fn builder() -> CoarseInputsBuilder {
        CoarseInputsBuilder::default()
    }

    /// The inputs of the body of `record`, in the system of `ctx`, from the record's sections
    /// (Design note 3), with `planet` the record of the planet a moon orbits
    /// (decision-backlog-2 §3).
    ///
    /// `record` and `planet` are their bodies' records at [`RECORD_TIME`], taken together, and
    /// `planet` is `Some` exactly where [`planet_of`](Self::planet_of) is: a moon's planet's
    /// record, whose orbit sets the moon's seasons and whose bulk and photometry its planetshine
    /// and eclipses will read (R09.T13). Every other body takes `None`. `ctx` is not read, the
    /// hosts' light coming from the record's conditions; a companion star's own cycle will read
    /// it (R09.T13), and it is kept so that this signature, frozen at generator version 23
    /// (decision-bump-plan-after-22, "Contracts that freeze at 23"), need not grow.
    ///
    /// The surface section is decided from its tag alone, never from the body's class: a body
    /// whose surface is [`Section::NotApplicable`] has no solid surface, whatever its class
    /// (P14.T48.e's gas-envelope split, decision-p14-t35e-wire). An [`Section::Ok`] section
    /// (P14.T48.e) carries the surface state and material, the conditions with their climate
    /// regime, which reads into the pass's vocabulary by
    /// `From<&hooks::surface::ClimateRegime>` (each substance by its registry row's key), and
    /// the global figures, and from P14.T54.a its condensates (P14.T51.a) and its crust
    /// (P14.T51.c). The inputs come from it with the record's `bulk` (gravity), `figure` (the
    /// spheroid), `rotation` (the spin) and `orbit` sections, a moon's planet's `orbit`, the hosts'
    /// light and the ground's thermal inertia, at [`RECORD_TIME`], as
    /// [`from_record`](Self::from_record) reads them (R09.T10.e), with:
    ///
    /// - **the crust**, the section's to the bit, each crust and the bulk material by its row's
    ///   key, its lid, provinces and melt area;
    /// - **the condensates**, the section's every entry, by `From<&CondensateEntry>`;
    /// - **the palette**, resolved from the registry's rows of the substances the section names
    ///   (P14.T49.b–c, T49.h and T49.i; decision-r11-palette-liquids §1.7): an entry for each
    ///   crust and the provinces, each ice and each liquid, the liquids with their records and a
    ///   melting crust with its melt's, by the `palette` module's rules. An airless body's palette
    ///   carries no cold-trapped `Ice` entry until P14.T51.f, wired by P14.T54.b, since no
    ///   condensate on the record names one.
    ///
    /// Both records are taken at [`RECORD_TIME`], the epoch, which settles the record's time
    /// (decision-backlog-2 §3; decision-bump-plan-after-22 §1.2 and §1.5).
    ///
    /// # Errors
    ///
    /// In this order, so that the planet is checked before the crust (decision-backlog-2 §3.2):
    ///
    /// - [`ReadCoarseInputsError::Section`] for the surface section's tag:
    ///   [`SurfaceInputsError::NoSolidSurface`] for a section that is [`Section::NotApplicable`],
    ///   [`SurfaceInputsError::NotModelled`] naming [`RecordSection::Surface`] for one that is
    ///   [`Section::NotModelled`], and [`SurfaceInputsError::NotResolved`] naming it for one a
    ///   degraded record withholds;
    /// - [`ReadCoarseInputsError::PlanetMissing`] for a moon given no planet's record, and
    ///   [`ReadCoarseInputsError::NotItsPlanet`] for a planet's record that is not the moon's
    ///   planet's (another body's, or a body's of another system), or one given for a body that
    ///   is not a moon;
    /// - [`ReadCoarseInputsError::Section`] with [`SurfaceInputsError::NotModelled`] naming
    ///   [`RecordSection::Surface`] for an [`Section::Ok`] section whose crust is not modelled (a
    ///   body whose passes took no lid), whose mean surface temperature is not positive (an unlit
    ///   body's, its equilibrium temperature of 0 K), or naming a substance of which the registry
    ///   does not resolve the palette's entry (its optics, its grain density where it lies, which
    ///   solid argon's lacks, or a liquid's record) or more substances than a palette holds
    ///   ([`MaterialPalette::MAX_ENTRIES`]): each a recorded gap, whose cause the error, frozen at
    ///   23, does not carry;
    /// - those of [`from_record`](Self::from_record).
    ///
    /// # Panics
    ///
    /// Never: the entries the record's substances resolve to keep every other rule of a palette by
    /// construction.
    ///
    /// # Examples
    ///
    /// A moon's inputs need its planet's record, at the same time as its own: the one
    /// [`planet_of`](Self::planet_of) names, which the server fetches beside the body's.
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::placement::CellKey;
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::planetary::record::Section;
    /// use hyperion_sim::planetary::surface::{CoarseInputs, RECORD_TIME, ReadCoarseInputsError};
    /// use hyperion_sim::planetary::{SystemContext, generate};
    /// use hyperion_sim::units::{Dex, SolarMasses, Years};
    ///
    /// // The first moon with a surface of Sun-like systems 1, 2, …, at the record's time.
    /// let cell = CellKey::new(Layer::C, [0, 812, 0])?;
    /// let mut found = None;
    /// for index in 1..64 {
    ///     let ctx = SystemContext::builder()
    ///         .system(cell.candidate_id(index).ok_or("an index in range")?)
    ///         .star(SolarMasses::new(1.0))
    ///         .fe_h(Dex::ZERO)
    ///         .age_at_epoch(Years::new(4.57e9))
    ///         .build()?;
    ///     let system = generate(Seed::new(0x5eed_0009_0010), &ctx);
    ///     let snapshot = system.snapshot_at(&ctx, RECORD_TIME);
    ///     let moon = snapshot.bodies().iter().find(|record| {
    ///         let solid = matches!(record.surface(), Section::Ok(_));
    ///         solid && CoarseInputs::planet_of(record).is_some()
    ///     });
    ///     if let Some(moon) = moon.cloned() {
    ///         found = Some((ctx, system, moon));
    ///         break;
    ///     }
    /// }
    /// let (ctx, system, moon) = found.ok_or("a Sun-like system holds a moon with a surface")?;
    /// // The planet's record, taken at the moon's time.
    /// let planet = CoarseInputs::planet_of(&moon)
    ///     .map(|index| system.body_at(&ctx, index, RECORD_TIME))
    ///     .transpose()?;
    /// // With it, the moon's inputs read its record's crust, condensates and palette; without
    /// // it, the moon's seasons, its planet's orbit's, cannot be read.
    /// let inputs = CoarseInputs::for_body(&moon, planet.as_ref(), &ctx)?;
    /// let lid = inputs.palette().get(inputs.lid_entry()).ok_or("the lid has an entry")?;
    /// assert!(lid.normal_albedo_bvr.iter().all(|a| *a > 0.0));
    /// assert!(inputs.seasonal_orbit().is_some());
    /// assert!(matches!(
    ///     CoarseInputs::for_body(&moon, None, &ctx),
    ///     Err(ReadCoarseInputsError::PlanetMissing(_))
    /// ));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn for_body(
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<Self, ReadCoarseInputsError> {
        let surface = surface_of(record)?;
        let seasons = checked_seasons(record, planet)?;
        let Modelled::Ok(crust) = surface.crust() else {
            return Err(SurfaceInputsError::NotModelled(RecordSection::Surface).into());
        };
        // An unlit body's record states its mean surface temperature as its equilibrium
        // temperature, 0 K, which gives the pass no climate to impose: not modelled.
        let mean = surface.conditions().mean_surface_temperature().value();
        let lit = mean.is_finite() && mean > 0.0;
        if !lit {
            return Err(SurfaceInputsError::NotModelled(RecordSection::Surface).into());
        }
        let palette = palette::resolve(surface, crust).map_err(palette_not_modelled)?;
        // `from_record` takes the hosts' light from the record's conditions, so the context is
        // not read; it is kept for a companion star's own cycle (R09.T13).
        let _ = ctx;
        Self::from_record(
            record,
            seasons,
            crust_inputs(crust),
            condensates_of(surface),
            palette,
        )
    }

    /// The planet whose record [`for_body`](Self::for_body) takes with this body's, in the body's
    /// system: `Some` for a moon, by its kind ([`BodyKind::Moon`] about [`OrbitHost::Body`]), whose
    /// seasons are its planet's orbit's (`decision-r09-t2.md` item 1), and `None` for every other
    /// body, a belt's member and a ring among them.
    ///
    /// A belt's member is listed under its belt but orbits the belt's host (P14.T21.c), so a body
    /// is a moon by its kind, not by its parent's being a body. This is the rule
    /// [`from_record`](Self::from_record) checks its [`SeasonsFromRecord`] by, public so that the
    /// server, which fetches the planet's record at [`RECORD_TIME`] beside the body's, never
    /// restates which bodies are moons (decision-backlog-2 §3.2).
    #[must_use]
    pub fn planet_of(record: &BodyRecord) -> Option<BodyIndex> {
        let identity = record.identity();
        match (identity.kind(), identity.parent()) {
            (BodyKind::Moon(_), Some(OrbitHost::Body(planet))) => Some(planet),
            (
                BodyKind::Moon(_),
                Some(OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre) | None,
            )
            | (
                BodyKind::Planet
                | BodyKind::DwarfPlanet
                | BodyKind::Ring
                | BodyKind::Belt(_)
                | BodyKind::CometaryHalo
                | BodyKind::ProtoplanetaryDisc
                | BodyKind::DebrisDisc
                | BodyKind::Unresolved,
                _,
            ) => None,
        }
    }

    /// The inputs of the body of `record`, read from its record's sections, with the crust,
    /// condensates and palette given (R09.T10.e; decision-bump-plan-after-22 §1.5): the reading
    /// [`for_body`](Self::for_body) answers with, giving it the record's own three (P14.T54.a).
    ///
    /// `record`, and the planet's record in `seasons`, are their bodies' records at
    /// [`RECORD_TIME`], at which the reading takes the spin and the seasonal orbit. `seasons` is
    /// [`SeasonsFromRecord::Planet`] with a moon's planet's record, whose orbit sets the moon's
    /// seasons (`decision-r09-t2.md` item 1), and [`SeasonsFromRecord::Own`] for every other body:
    /// a planet, a dwarf planet or a belt's member, whose own orbit sets them. The values come
    /// from:
    ///
    /// - **the surface section** (P14.T48.e): its state and material, its climate regime (each
    ///   substance by its registry row's key), its conditions (the mean surface temperature and
    ///   the signed contrasts, the pressure and the gases, and the hosts' light, P14.T12's sum of
    ///   L ÷ (4π a² √(1 − e²)) over the hosts at the record's time, the orbit average that the
    ///   pass draws over the seasonal orbit, a companion's and a planet's shares with it, as
    ///   [`host_flux`](Self::host_flux) says, and the ground's thermal inertia Γ its night read,
    ///   [`ground_inertia_j_per_m2_k_sqrt_s`](Self::ground_inertia_j_per_m2_k_sqrt_s)), and its
    ///   figures (`σ_h`, the liquids and ices with an area, the tectonic regime and continental
    ///   fraction, the volcanism level, the heat flow, the surface age, the crater contract and the
    ///   wet epoch);
    /// - **the bulk section**'s gravity and **the figure section**'s spheroid;
    /// - **the rotation section**'s body-fixed frame: the spin at [`RECORD_TIME`] under the
    ///   seasonal orbit ([`SeasonalSpin::of`], as the record's conditions were derived), with the
    ///   equinox's true anomaly from the frame's pole and the orbit's plane and periapsis;
    /// - **the orbit section**, the body's own or its planet's: its eccentricity and period at
    ///   [`RECORD_TIME`], along its drift for an evolving orbit (P14.T45.a).
    ///
    /// An `Airless` surface's pressure is exactly zero, a contract of the field's header
    /// (decision-r11-palette-liquids §2, item 5), and every other state keeps the record's. A
    /// liquid or an ice whose area is zero lies nowhere on the ground (the record lists water at
    /// a zero share on a dry world, P14.T24.b), so it is not one of the inputs' liquids or ices,
    /// and it needs no palette entry. The surface section's crust and condensates are not read
    /// here: `crust` and `condensates` stand for them, and `palette` for their resolution, which
    /// [`for_body`](Self::for_body) gives from the record.
    ///
    /// It gives no cold traps: [`cold_traps`](Self::cold_traps) is empty and
    /// [`cold_trap_inputs`](Self::cold_trap_inputs) `None`, until P14.T54.b puts P14.T51.f's cold
    /// traps on the record at generator version 24, each solid entry's trapped area with the star's
    /// angular radius and the regolith's albedo they read (decision-r09-t13f-cold-traps §1.1,
    /// §2.2). Each ice's share is the record's whole share, its cold-trapped part included.
    ///
    /// The ground's weathering ([`weathering`](Self::weathering), the maturity with the weathering
    /// rate) and its [`organic_cover`](Self::organic_cover) are the surface section's modifiers'
    /// (P14.T51.d, wired by P14.T54.b.c), to the bit, where the record models them, the cover +0
    /// where no ground takes one; and none of the three where it does not, every record's below
    /// generator version 24, which holds the modifiers off the record.
    ///
    /// # Errors
    ///
    /// - [`ReadCoarseInputsError::Section`] as [`for_body`](Self::for_body) answers the surface
    ///   section's tag; and for a bulk, figure, rotation or orbit section, the body's or its
    ///   planet's, that is not modelled or does not apply,
    ///   [`SurfaceInputsError::NotModelled`] naming it (a body that orbits nothing has no seasons
    ///   this reading models), or that is withheld, [`SurfaceInputsError::NotResolved`] naming it;
    /// - [`ReadCoarseInputsError::PlanetMissing`] for a moon given [`SeasonsFromRecord::Own`], and
    ///   [`ReadCoarseInputsError::NotItsPlanet`] for a planet's record that is not the moon's
    ///   planet's (another body's, or a body's of another system), or one given for a body that is
    ///   not a moon;
    /// - [`ReadCoarseInputsError::NotAtRecordTime`] for a seasonal orbit whose record does not hold
    ///   at [`RECORD_TIME`], as for a record taken at another time;
    /// - [`ReadCoarseInputsError::SeasonalOrbit`] for a seasonal orbit whose drift gives no bound
    ///   orbit at [`RECORD_TIME`];
    /// - [`ReadCoarseInputsError::Build`] for what [`CoarseInputsBuilder::build`] refuses: a
    ///   palette without an entry for a liquid, an ice, a condensate on the ground or a lithology,
    ///   a crust it refuses, or a value of the record outside its range.
    ///
    /// # Examples
    ///
    /// A generated dry rocky planet's inputs, with a crust and palette of the example's own in
    /// place of the record's, which [`for_body`](Self::for_body) reads. The palette must name each
    /// ice that lies on the ground, as the record's figures list them; a planet with a sea would
    /// also need the liquid's entry, with its optical record (`PaletteEntry::liquid`, R11's), which
    /// a dry planet does without:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::placement::CellKey;
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::planetary::derive::PlanetClass;
    /// use hyperion_sim::planetary::record::{BodyKind, Section};
    /// use hyperion_sim::planetary::surface::{
    ///     CoarseInputs, CrustInputs, LidCrust, RECORD_TIME, SeasonsFromRecord,
    /// };
    /// use hyperion_sim::planetary::{SystemContext, generate};
    /// use hyperion_sim::units::{Dex, Kelvin, SolarMasses, Years};
    /// use hyperion_surface::field::{MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole};
    /// use hyperion_surface::substance_key::SubstanceKey;
    ///
    /// // The first of Sun-like systems 1, 2, … that holds a dry rocky planet, at the record's
    /// // time.
    /// let cell = CellKey::new(Layer::C, [0, 812, 0])?;
    /// let mut found = None;
    /// for index in 1..64 {
    ///     let ctx = SystemContext::builder()
    ///         .system(cell.candidate_id(index).ok_or("an index in range")?)
    ///         .star(SolarMasses::new(1.0))
    ///         .fe_h(Dex::ZERO)
    ///         .age_at_epoch(Years::new(4.57e9))
    ///         .build()?;
    ///     let system = generate(Seed::new(0x5eed_0009_0010), &ctx);
    ///     let snapshot = system.snapshot_at(&ctx, RECORD_TIME);
    ///     let rocky = snapshot.bodies().iter().find(|record| {
    ///         let dry = matches!(record.surface(), Section::Ok(surface)
    ///             if surface.figures().surface_liquids().iter().all(|&(_, area)| area <= 0.0));
    ///         dry && record.identity().kind() == BodyKind::Planet
    ///             && record.bulk().ok().is_some_and(|bulk| bulk.class() == PlanetClass::Rocky)
    ///     });
    ///     if let Some(rocky) = rocky {
    ///         found = Some((ctx, rocky.clone()));
    ///         break;
    ///     }
    /// }
    /// let (ctx, rocky) = found.ok_or("a Sun-like system holds a dry rocky planet")?;
    /// let Section::Ok(surface) = rocky.surface() else {
    ///     return Err("a rocky planet has a surface section".into());
    /// };
    /// // A basalt lid, and an entry for each ice with an area on the ground.
    /// let basalt = SubstanceKey::new("basalt")?;
    /// let entry = |substance, role| PaletteEntry {
    ///     substance,
    ///     role,
    ///     normal_albedo_bvr: [0.1; 3],
    ///     phase_row: 0,
    ///     grain_density_kg_m3: 3_010.0,
    ///     transition: Kelvin::new(1_253.15),
    ///     mechanics: MechanicsFamily::Silicate,
    ///     // Neither the rock nor an ice is a liquid, so neither carries a liquid record.
    ///     liquid: None,
    ///     feo_mass_fraction: None,
    /// };
    /// let mut entries = vec![entry(basalt, PaletteRole::SecondaryCrust)];
    /// let ices = surface.figures().surface_ices().iter();
    /// let on_the_ground = ices.filter(|&&(_, area_fraction)| area_fraction > 0.0);
    /// entries.extend(on_the_ground.map(|&(id, _)| entry(id.substance().key(), PaletteRole::Ice)));
    /// let palette = MaterialPalette::new(entries)?;
    /// let crust = CrustInputs {
    ///     primary: None,
    ///     secondary: Some(basalt),
    ///     tertiary: None,
    ///     bulk: None,
    ///     lid: LidCrust::Secondary,
    ///     provinces: basalt,
    ///     melt_area_fraction: 0.0,
    /// };
    /// // A planet's seasons are its own orbit's, so it needs no planet's record.
    /// let inputs = CoarseInputs::from_record(&rocky, SeasonsFromRecord::Own, crust, [], palette)?;
    /// let bulk = rocky.bulk().ok().ok_or("a rocky planet has a bulk section")?;
    /// assert_eq!(inputs.gravity(), bulk.surface_gravity());
    /// assert!(inputs.seasonal_orbit().is_some());
    /// // `for_body` reads the record's own crust, condensates and palette, and the same other
    /// // sections; a planet takes no planet's record (`CoarseInputs::planet_of` is `None`).
    /// assert_eq!(CoarseInputs::planet_of(&rocky), None);
    /// let own = CoarseInputs::for_body(&rocky, None, &ctx)?;
    /// assert_eq!(own.gravity(), inputs.gravity());
    /// assert_eq!(own.seasonal_orbit(), inputs.seasonal_orbit());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_record(
        record: &BodyRecord,
        seasons: SeasonsFromRecord<'_>,
        crust: CrustInputs,
        condensates: impl IntoIterator<Item = Condensate>,
        palette: MaterialPalette,
    ) -> Result<Self, ReadCoarseInputsError> {
        let surface = surface_of(record)?;
        let bulk = section(record.bulk(), RecordSection::Bulk)?;
        let figure = section(record.figure(), RecordSection::Figure)?;
        let frame = section(record.rotation(), RecordSection::Rotation)?;
        let (orbit, whose) = seasonal_orbit_of(record, seasons)?;
        let seasonal = elements_at(orbit, RECORD_TIME)?;
        let (conditions, figures) = (surface.conditions(), surface.figures());
        let identity = record.identity();
        let mut builder = Self::builder()
            .body(BodyRef::new(
                identity.system().raw(),
                identity.index().get(),
            ))
            .figure(figure.spheroid())
            .gravity(bulk.surface_gravity())
            .sigma_h(figures.sigma_h())
            .liquids(on_the_ground(figures.surface_liquids()))
            .ices(on_the_ground(figures.surface_ices()))
            .mean_surface_temperature(conditions.mean_surface_temperature())
            .equator_pole_contrast_k(conditions.equator_pole_contrast_k())
            .day_night_contrast_k(conditions.day_night_contrast_k())
            .spin(spin_of(frame, &seasonal, whose))
            .host_flux(conditions.stellar_flux())
            .seasonal_orbit(SeasonalOrbit {
                eccentricity: seasonal.eccentricity().value(),
                period: seasonal.period(),
            })
            .surface_pressure(surface_pressure_of(
                surface.state(),
                conditions.surface_pressure(),
            ))
            .gases(
                conditions
                    .gases()
                    .iter()
                    .map(|&(id, mole_fraction)| GasShare {
                        substance: id.substance().key(),
                        mole_fraction,
                    }),
            )
            .surface(surface.state(), surface.material())
            .tectonics(figures.tectonic_regime(), figures.continental_fraction())
            .volcanism(figures.volcanism())
            .heat_flow(figures.heat_flow())
            // As the crater contract converts its ages (`hooks::figures`), so that a surface as
            // old as its system stays no older than it.
            .surface_age(Gigayears::new(
                figures.surface_age().value() / YEARS_PER_GIGAYEAR,
            ))
            // Interim, labelled: 0, the provinces erupting to now. That is right for every wet
            // epoch its outgassing clause ends, which leaves the volcanism running, and wrong only
            // where a dimming host ended the epoch after the volcanism died. Tripwire, for plan
            // 14's `GlobalFigures::volcanic_surface_age` (decision-r09-t14-mars-ceiling §2.1, the
            // P14.T48 sub-task main briefs): when it lands, read it here (in Gyr, as the surface
            // age above) and delete this comment, before the gate.
            .province_surface_age(Gigayears::new(0.0))
            .craters(*figures.craters())
            .climate(ClimateRegime::from(surface.climate_regime()))
            .crust(crust)
            .condensates(condensates)
            .palette(palette)
            // The record's Γ, which its conditions' night read, so that R09's radiative model
            // reads it too (decision-p14-t54a-passes §3.5).
            .ground_inertia_j_per_m2_k_sqrt_s(conditions.ground().inertia_j_per_m2_k_sqrt_s());
        if let Some(epoch) = figures.wet_epoch() {
            builder = builder.wet_epoch(WetEpoch::from(epoch));
        }
        // The header's weathering parts and organic cover, where the record's modifiers are
        // modelled (P14.T54.b.c): absent together until then, so that every field keeps its bits.
        if let Modelled::Ok(modifiers) = surface.modifiers() {
            builder = builder
                .weathering(GroundWeathering {
                    maturity: modifiers.maturity(),
                    weathering_rate: modifiers.weathering_rate(),
                })
                .organic_cover(modifiers.organic_cover());
        }
        Ok(builder.build()?)
    }

    /// The body.
    #[must_use]
    pub const fn body(&self) -> BodyRef {
        self.body
    }

    /// The body's rotational spheroid, the datum of every height (R07's Design note 19; P14.T46's
    /// `figure()`).
    #[must_use]
    pub const fn figure(&self) -> Spheroid {
        self.figure
    }

    /// The body's volumetric mean radius, ∛(a² c) of its figure, from which the field's level and
    /// boundary diameter follow (Design note 4).
    #[must_use]
    pub fn radius(&self) -> Metres {
        Metres::new(self.figure.volumetric_radius_m())
    }

    /// The mean surface gravity, G M ÷ R² (the record's bulk section).
    #[must_use]
    pub const fn gravity(&self) -> MetresPerSecondSquared {
        self.gravity
    }

    /// Plan 14's hypsometric standard deviation `σ_h`, the area-weighted RMS elevation about the
    /// mean, relative to the reference equipotential with degree 1 included (P14.T48.a): what the
    /// reconstructed field's realises (Design note 7).
    #[must_use]
    pub const fn sigma_h(&self) -> Metres {
        self.sigma_h
    }

    /// The surface liquids by substance, largest area first, ties by key (P14.T24.b's
    /// `surface_liquids`).
    #[must_use]
    pub fn liquids(&self) -> &[AreaShare] {
        &self.liquids
    }

    /// The surface ices by substance, largest area first, ties by key (P14.T24.b's
    /// `surface_ices`), each its whole share, its cold-trapped part
    /// ([`cold_traps`](Self::cold_traps)) included.
    #[must_use]
    pub fn ices(&self) -> &[AreaShare] {
        &self.ices
    }

    /// The cold-trapped part of each ice's share of the surface, by substance, largest first, ties
    /// by key (P14.T51.f): the share its exposed ice covers in permanent shadows that stay below
    /// its cold-trap threshold all year, `f_ct` × `f_x`, 0 (not −0) to 1, never larger than its
    /// [`ices`](Self::ices) share. R09.T13.f places it in proportion to each cell's cold-trap
    /// fraction, and R09.T13.d places the rest of the share, its zonal part, by its own rule.
    ///
    /// Empty for a body with no cold traps. It is empty from the record until P14.T54.b puts
    /// P14.T51.f's cold traps on it at generator version 24, and
    /// [`cold_trap_inputs`](Self::cold_trap_inputs) is `Some` wherever a cold-trapped share is
    /// above zero.
    #[must_use]
    pub fn cold_traps(&self) -> &[AreaShare] {
        &self.cold_traps
    }

    /// What the cold traps read of the body's star and ground beyond the other inputs, or `None`
    /// where the inputs give no cold traps (from the record until P14.T54.b). It is `Some`
    /// wherever a cold-trapped share of [`cold_traps`](Self::cold_traps) is above zero.
    #[must_use]
    pub const fn cold_trap_inputs(&self) -> Option<&ColdTrapInputs> {
        self.cold_trap_inputs.as_ref()
    }

    /// The ocean fraction: the liquids' shares summed in their order from +0, 0 to 1 (+0 with
    /// none, where a float `sum` would give −0).
    #[must_use]
    pub fn ocean_fraction(&self) -> f64 {
        self.liquids
            .iter()
            .fold(0.0, |sum, s| sum + s.area_fraction)
    }

    /// The ice fraction: the ices' shares summed in their order from +0, 0 to 1. Ice may lie on a
    /// liquid, so the two fractions may overlap.
    #[must_use]
    pub fn ice_fraction(&self) -> f64 {
        self.ices.iter().fold(0.0, |sum, s| sum + s.area_fraction)
    }

    /// The area-weighted mean surface temperature, which the climate step imposes (P14.T24.a).
    ///
    /// On an airless body plan 14 states its equilibrium temperature (P14.T13.c), a radiative
    /// mean: the Moon's is about 270 K, where its time-and-area mean temperature is far lower
    /// (Diviner's equator swings between about 95 and 395 K, Williams et al. 2017, Icarus 283,
    /// 300). On the radiative model R09.T13.d imposes it as that radiative mean, the brightness
    /// temperature of the emitted flux less the night floor's, averaged as flux over the surface
    /// and the year; on the other models as the area-weighted arithmetic mean
    /// (decision-r09-t13d-normalisation).
    #[must_use]
    pub const fn mean_surface_temperature(&self) -> Kelvin {
        self.mean_surface_temperature
    }

    /// The equator–pole contrast T(0°) − T(90°), kelvin, signed: a warm pole is negative
    /// (P14.T48.e). The climate step imposes it on the annual field's ends by a correction shaped
    /// like P₂(sin φ). On a field T₀ + T₂ P₂(sin φ) the contrast is −(3/2) T₂, so the coefficient
    /// is then −⅔ of it (Design note 8).
    #[must_use]
    pub const fn equator_pole_contrast_k(&self) -> f64 {
        self.equator_pole_contrast_k
    }

    /// The day–night contrast, kelvin, signed, about P14.T14's substellar axis on a locked world
    /// (P14.T48.e), which the climate step imposes there on the μ-weighted day and night means by
    /// a P₁(cos γ) correction, so that a P₁ field's coefficient is ¾ of it (Design note 8).
    #[must_use]
    pub const fn day_night_contrast_k(&self) -> f64 {
        self.day_night_contrast_k
    }

    /// How the body turns under its hosts.
    #[must_use]
    pub const fn spin(&self) -> &Spin {
        &self.spin
    }

    /// The light of every host at the body, averaged over the orbit, watts per square metre
    /// (P14.T12's sum of L ÷ (4π a² √(1 − e²)) over the hosts): zero for a free-floating body.
    ///
    /// The pass draws all of it over the seasonal orbit's (a ÷ r)² cycle, which is exact for one
    /// host. A companion star's share, on the pair's orbit, and a moon's planetshine, on the
    /// moon's own, have their own cycles: their annual mean is exact, their seasonal shape the
    /// seasonal orbit's (R09's Risks, "Deviations in T10.e, as built").
    #[must_use]
    pub const fn host_flux(&self) -> WattsPerSquareMetre {
        self.host_flux
    }

    /// The orbit that sets the seasons, or `None` for a body that orbits no star, which has no
    /// seasons.
    #[must_use]
    pub const fn seasonal_orbit(&self) -> Option<&SeasonalOrbit> {
        self.seasonal_orbit.as_ref()
    }

    /// The mean surface pressure (P14.T24.a), zero on an airless body.
    ///
    /// 0 also on a surface holding melt whose volatiles are gone, whose own vapour is not modelled
    /// (P14.T55.b; decision-backlog-2 §2.5).
    #[must_use]
    pub const fn surface_pressure(&self) -> Pascals {
        self.surface_pressure
    }

    /// The air's gases at the surface, largest mole fraction first, ties by key (P14.T24.a).
    #[must_use]
    pub fn gases(&self) -> &[GasShare] {
        &self.gases
    }

    /// The surface state (P14.T13.c), never a gas envelope: such a body has no surface to build.
    #[must_use]
    pub const fn surface_state(&self) -> SurfaceState {
        self.surface_state
    }

    /// The surface material, rock or ice (P14.T13.c).
    #[must_use]
    pub const fn surface_material(&self) -> SurfaceMaterial {
        self.surface_material
    }

    /// The tectonic regime (P14.T24.b).
    #[must_use]
    pub const fn tectonics(&self) -> TectonicRegime {
        self.tectonics
    }

    /// The continental fraction `f_c`, 0 to 1 (P14.T48.a): 0.405 on Earth, zero on a stagnant lid.
    #[must_use]
    pub const fn continental_fraction(&self) -> f64 {
        self.continental_fraction
    }

    /// The volcanism level V, dimensionless and non-negative, that builds constructional relief
    /// over the surface's history (P14.T48.a, calibrated so that Mars's is 1; not the present
    /// outgassing, T24.b's O).
    #[must_use]
    pub const fn volcanism(&self) -> f64 {
        self.volcanism
    }

    /// The surface heat flow, watts per square metre (P14.T24.b): Earth's mean is 0.0916 (Davies
    /// and Davies 2010, Solid Earth 1, 5, Table 7: 46.7 TW over 5.101 × 10¹⁴ m²).
    #[must_use]
    pub const fn heat_flow(&self) -> WattsPerSquareMetre {
        self.heat_flow
    }

    /// The surface age, the time since the last global resurfacing (P14.T24.b).
    #[must_use]
    pub const fn surface_age(&self) -> Gigayears {
        self.surface_age
    }

    /// The volcanic provinces' surface age, 0 to the surface age: the time since their surface
    /// last formed, the time since the heat flow last stood at or above the volcanism's threshold
    /// (plan 14's `volcanic_surface_age`, decision-r09-t14-mars-ceiling §2.1). The erosion's
    /// exposure of a [`Crust::Province`] cell is its wet epoch's share by this age rather than the
    /// global one: a province no older than the epoch's end is renewed to its construction after
    /// the solve. [`from_record`](Self::from_record) takes 0 until plan 14's figure lands.
    #[must_use]
    pub const fn province_surface_age(&self) -> Gigayears {
        self.province_surface_age
    }

    /// The crater contract (P14.T48.c), whose gravity is the body's.
    #[must_use]
    pub const fn craters(&self) -> &CraterParams {
        &self.craters
    }

    /// The wet epoch, or `None` for a world that never held surface liquid (P14.T48.b).
    #[must_use]
    pub const fn wet_epoch(&self) -> Option<&WetEpoch> {
        self.wet_epoch.as_ref()
    }

    /// The climate regime (P14.T48.d).
    #[must_use]
    pub const fn climate(&self) -> &ClimateRegime {
        &self.climate
    }

    /// The crust by substance (P14.T51.c).
    #[must_use]
    pub const fn crust(&self) -> &CrustInputs {
        &self.crust
    }

    /// The condensates, at most [`MAX_CONDENSATES`], largest reservoir first, ties by key then
    /// phase (P14.T51.a).
    #[must_use]
    pub fn condensates(&self) -> &[Condensate] {
        &self.condensates
    }

    /// The palette, which the field's header carries whole.
    ///
    /// It holds every substance the field names, its lithologies, ices and liquids and its
    /// deposits, each with the values the server resolves from plan 14's registry (P14.T49.b–c),
    /// so that the field stays the client's only input (decision-composition §1.7, Design note
    /// 17).
    #[must_use]
    pub const fn palette(&self) -> &MaterialPalette {
        &self.palette
    }

    /// The palette entry of each [`Crust`] variant's lithology, in its code order (continental,
    /// oceanic, lid, province), as the field's header carries it: `Continental` the tertiary
    /// crust in [`PaletteRole::TertiaryCrust`], or `None` without one; `Oceanic` the secondary
    /// crust, or `None` without one (a Ceres, an undifferentiated body); `Lid` the entry of the
    /// crust or bulk material [`CrustInputs::lid`] names, [`lid_entry`](Self::lid_entry): the
    /// primary crust's in [`PaletteRole::PrimaryCrust`] (a Moon's feldspathic highlands, a Ceres's
    /// phyllosilicate), the secondary crust's (a Mars's basalt, over any buried primary crust;
    /// Earth's), or the bulk material's in [`PaletteRole::Bulk`] (a chondritic asteroid's); and
    /// `Province` the provinces' lithology.
    ///
    /// The secondary crust's entry is its [`PaletteRole::SecondaryCrust`] one and the provinces'
    /// their [`PaletteRole::Province`] one, but where the two are one substance either entry names
    /// both, the variant's own role tried first: a Mars's basaltic provinces take its lid's
    /// `SecondaryCrust` basalt, and a Moon's maria, basaltic provinces that are its secondary
    /// crust, its `Province` basalt. Where the provinces are the primary crust's or the bulk
    /// material's substance and have no entry of their own, they take its `PrimaryCrust` or `Bulk`
    /// entry, after the secondary's where both crusts are that substance: a Ceres's provinces would
    /// be its phyllosilicate lid's (science-r09-drainage.md §5), and an undifferentiated body's its
    /// bulk material's (science-composition-crusts.md §1). The primary crust's entry is its
    /// `PrimaryCrust` one alone, and the bulk material's its `Bulk` one.
    #[must_use]
    pub const fn crust_palette(&self) -> [Option<u8>; 4] {
        self.crust_palette
    }

    /// The palette entry of the crust or bulk material [`CrustInputs::lid`] names, which
    /// [`crust_palette`](Self::crust_palette)'s [`Lid`](Crust::Lid) holds.
    ///
    /// The builder resolves it once, so that no step matches or unwraps the crust's options. Its
    /// transition is the solidus of the body's melt test ([`CrustInputs::melt_area_fraction`],
    /// P14.T51.c's) and of R09.T13.c's on the lid's cells (science-composition-crusts.md §5).
    #[must_use]
    pub const fn lid_entry(&self) -> u8 {
        self.lid_entry
    }

    /// The palette entry of the body's main liquid, its sea's: the [`PaletteRole::Liquid`] entry of
    /// the liquid of largest area ([`liquids`](Self::liquids)' first), or `None` without one.
    #[must_use]
    pub const fn main_liquid(&self) -> Option<u8> {
        self.main_liquid
    }

    /// The ground's thermal inertia Γ, J m⁻² K⁻¹ s^−½, finite and positive, where the inputs
    /// carry plan 14's: the record's conditions' ground (P14.T54.a), P14.T49.j's law at the lid
    /// the record's published pass's context read, at the body's equilibrium temperature, with
    /// P14.T24.a's constants as its fallback alone (decision-pre-23-review item 7), which
    /// [`from_record`](Self::from_record) reads. `None` for inputs built by hand (the reference
    /// worlds), whose radiative model takes the law at their own lid at their equilibrium
    /// temperature.
    ///
    /// R09's radiative model reads it (R09.T13.c), so that its nights and the record's read one Γ
    /// on every body it runs on, airless or under a thin air (decision-p14-t54a-passes §3.5;
    /// Batch D's review, D3-1). Under air the law is a vacuum lower bound, with no pore gas
    /// (P14.T49.l's).
    #[must_use]
    pub const fn ground_inertia_j_per_m2_k_sqrt_s(&self) -> Option<f64> {
        self.ground_inertia_j_per_m2_k_sqrt_s
    }

    /// The ground's space weathering, its maturity with the weathering rate, which the field's
    /// header carries as its `maturity` and `weathering_rate` (R09's reserved parts, R10.T1.c's
    /// fade): plan 14's modifiers' (P14.T51.d), `None` where the inputs carry none, as from a
    /// record below generator version 24, whose modifiers are held off it (P14.T54.b.c).
    #[must_use]
    pub const fn weathering(&self) -> Option<&GroundWeathering> {
        self.weathering.as_ref()
    }

    /// The organic cover, the covered fraction of the ground that can take one, dimensionless in
    /// [+0, 1], which the field's header carries (R10.T1.c's cover blend): plan 14's modifiers'
    /// (P14.T51.d), +0 where no ground takes one, and `None` where the inputs carry none, as from a
    /// record below generator version 24 (P14.T54.b.c).
    #[must_use]
    pub const fn organic_cover(&self) -> Option<f64> {
        self.organic_cover
    }

    /// The months of the field's year: 1 for a world with no seasonal forcing, a circular
    /// seasonal orbit with no obliquity to it, or no orbit at all; 12 otherwise, each a twelfth of
    /// the seasonal orbit in eccentric anomaly (`decision-r09-t2.md` item 1, Design note 8).
    ///
    /// A lock adds nothing (Design note 8, signed off 2026-10-10): a world locked 1:1 to its star
    /// has seasons wherever its orbit is eccentric or it has obliquity, and a moon locked to its
    /// planet has its planet's orbit's seasons at its own obliquity to that orbit.
    #[must_use]
    pub fn months(&self) -> u8 {
        let Some(orbit) = self.seasonal_orbit else {
            return 1;
        };
        let circular = orbit.eccentricity <= 0.0;
        let obliquity = self.spin.obliquity.value();
        let untilted = obliquity <= 0.0 || obliquity >= core::f64::consts::PI;
        if circular && untilted { 1 } else { 12 }
    }
}

/// Refuses a crater contract of another gravity than the body's, or of a system younger than its
/// surface, which the chronology cannot count.
fn check_craters(
    craters: &CraterParams,
    gravity: MetresPerSecondSquared,
    surface_age: Gigayears,
) -> Result<(), BuildCoarseInputsError> {
    if (craters.gravity().value() / gravity.value() - 1.0).abs() > GRAVITY_TOLERANCE {
        return Err(BuildCoarseInputsError::CraterGravity {
            body: gravity,
            craters: craters.gravity(),
        });
    }
    if surface_age.value() > craters.system_age().value() {
        return Err(BuildCoarseInputsError::SurfaceOlderThanSystem {
            surface: surface_age,
            system: craters.system_age(),
        });
    }
    Ok(())
}

/// The pass's crust of the record's `crust` (P14.T51.c), to the bit: each crust and the bulk
/// material by its row's key, the lid, the provinces and the melt area.
#[must_use]
fn crust_inputs(crust: &CrustComposition) -> CrustInputs {
    let key = |id: SubstanceId| id.substance().key();
    CrustInputs {
        primary: crust.primary().map(key),
        secondary: crust.secondary().map(key),
        tertiary: crust.tertiary().map(key),
        bulk: crust.bulk().map(key),
        lid: crust.lid(),
        provinces: key(crust.provinces()),
        melt_area_fraction: crust.melt_area_fraction(),
    }
}

/// The pass's condensates of the record's surface section `surface`: every published entry
/// (P14.T51.a), in the pass's vocabulary (`From<&CondensateEntry>`).
fn condensates_of(surface: &Surface) -> impl Iterator<Item = Condensate> + '_ {
    surface.condensates().entries().iter().map(Condensate::from)
}

/// What [`CoarseInputs::for_body`] answers where the palette of a body's record is not resolved,
/// `palette::resolve`'s refusal `error`: the surface section not modelled, the body's own state,
/// where the registry does not resolve a substance the section names (a solid's optics or its
/// grain density where it lies, a liquid's condensed columns or its record), and where the section
/// names more substances than a palette holds ([`MaterialPalette::MAX_ENTRIES`]), each a recorded
/// gap. The refusal names its cause, which the frozen [`ReadCoarseInputsError`] cannot carry
/// (inputs.rs's tests list it, `unread_cause`).
///
/// # Panics
///
/// Never: every other rule of a palette holds for the entries `palette::resolve` gives, by
/// construction (the `palette` module's rules).
fn palette_not_modelled(error: ResolveBodyPaletteError) -> ReadCoarseInputsError {
    match error {
        ResolveBodyPaletteError::Solid { .. }
        | ResolveBodyPaletteError::LiquidNotCondensed(_)
        | ResolveBodyPaletteError::Liquid { .. }
        | ResolveBodyPaletteError::Palette(BuildPaletteError::TooManyEntries(_)) => {
            SurfaceInputsError::NotModelled(RecordSection::Surface).into()
        }
        ResolveBodyPaletteError::Palette(
            refused @ (BuildPaletteError::Unsorted { .. }
            | BuildPaletteError::Albedo { .. }
            | BuildPaletteError::Density { .. }
            | BuildPaletteError::Transition { .. }
            | BuildPaletteError::LiquidMissing { .. }
            | BuildPaletteError::LiquidMisplaced { .. }
            | BuildPaletteError::LiquidDensity { .. }
            | BuildPaletteError::LiquidMember { .. }
            | BuildPaletteError::FeoMassFraction { .. }
            | BuildPaletteError::FeoOnLiquid { .. }),
        ) => unreachable!(
            "a resolved palette keeps the palette's rules: its entries in the map's (role, key) \
             order, each solid's albedo, grain density and transition its registry row's, finite \
             and positive (the registry's tests hold the albedos and transitions), each liquid \
             with its record and each melt's, `liquid_properties`', a liquid's density its grain \
             density, no ice with a liquid record and no FeO_T: {refused}"
        ),
    }
}

/// The surface section of `record`, decided from its tag alone (decision-p14-t35e-wire §2.4):
/// a section that does not apply has no solid surface, and one not modelled or withheld names
/// the surface.
fn surface_of(record: &BodyRecord) -> Result<&Surface, SurfaceInputsError> {
    match record.surface() {
        Section::Ok(surface) => Ok(surface),
        Section::NotApplicable => Err(SurfaceInputsError::NoSolidSurface),
        Section::NotModelled => Err(SurfaceInputsError::NotModelled(RecordSection::Surface)),
        Section::NotResolved => Err(SurfaceInputsError::NotResolved(RecordSection::Surface)),
    }
}

/// The contents of the section `name` of a record whose surface section is `Ok`: a section that
/// is not modelled, or that does not apply where the pass needs it (a free-floating body's orbit),
/// is a reading this version does not model, and a withheld one is not resolved.
fn section<T>(section: &Section<T>, name: RecordSection) -> Result<&T, SurfaceInputsError> {
    match section {
        Section::Ok(value) => Ok(value),
        Section::NotModelled | Section::NotApplicable => Err(SurfaceInputsError::NotModelled(name)),
        Section::NotResolved => Err(SurfaceInputsError::NotResolved(name)),
    }
}

/// Whose orbit sets the seasons of the body of `record`, given `planet`, the record given as its
/// planet's: [`SeasonsFromRecord::Planet`] with a moon's planet's record, and
/// [`SeasonsFromRecord::Own`] for every other body, given none ([`CoarseInputs::planet_of`]'s
/// rule). A planet's record is the moon's planet's if it is the same body of the same system.
///
/// # Errors
///
/// [`ReadCoarseInputsError::PlanetMissing`] for a moon given no planet's record, and
/// [`ReadCoarseInputsError::NotItsPlanet`] for a record that is not the moon's planet's, or one
/// given for a body that is not a moon.
fn checked_seasons<'a>(
    record: &BodyRecord,
    planet: Option<&'a BodyRecord>,
) -> Result<SeasonsFromRecord<'a>, ReadCoarseInputsError> {
    let system = record.identity().system();
    let moon_of = CoarseInputs::planet_of(record).map(|index| index.body_id(system));
    match (moon_of, planet) {
        (None, None) => Ok(SeasonsFromRecord::Own),
        (Some(wanted), Some(planet)) if body_id_of(planet) == wanted => {
            Ok(SeasonsFromRecord::Planet(planet))
        }
        (Some(wanted), None) => Err(ReadCoarseInputsError::PlanetMissing(wanted)),
        (moon_of, Some(given)) => Err(ReadCoarseInputsError::NotItsPlanet {
            planet: moon_of,
            given: body_id_of(given),
        }),
    }
}

/// The orbit that sets the seasons of the body of `record`, and whose it is: a moon's planet's,
/// from the planet's record in `seasons`, and every other body's own (`decision-r09-t2.md` item
/// 1), checked by [`checked_seasons`].
fn seasonal_orbit_of<'a>(
    record: &'a BodyRecord,
    seasons: SeasonsFromRecord<'a>,
) -> Result<(&'a BodyOrbit, SeasonsFrom), ReadCoarseInputsError> {
    let given = match seasons {
        SeasonsFromRecord::Own => None,
        SeasonsFromRecord::Planet(planet) => Some(planet),
    };
    match checked_seasons(record, given)? {
        SeasonsFromRecord::Own => Ok((
            section(record.orbit(), RecordSection::Orbit)?,
            SeasonsFrom::Own,
        )),
        SeasonsFromRecord::Planet(planet) => Ok((
            section(planet.orbit(), RecordSection::Orbit)?,
            SeasonsFrom::Planet,
        )),
    }
}

/// The ID of the body of `record`: its index in its system.
#[must_use]
fn body_id_of(record: &BodyRecord) -> BodyId {
    let identity = record.identity();
    identity.index().body_id(identity.system())
}

/// The elements of `orbit` at `t`: its own for an orbit that holds, and for an evolving orbit its
/// drift cell's model then (P14.T45.a), the eccentricity eᵣ + ė Δt and the mean motion
/// nᵣ + ṅ Δt that [`DriftingOrbit::relative_state_at`](crate::planetary::drift::DriftingOrbit)
/// propagates, so the period 2π ÷ n, with the axis aᵣ + ȧ Δt and the gravitational parameter
/// n² a³ that Kepler's third law gives them. At the cell's reference time they are the record's
/// elements, to the bit; at [`RECORD_TIME`], the epoch, every drift cell of a record taken then
/// starts, since the cells are aligned on it. The phase is not the model's: the elements keep the
/// reference's mean anomaly at the epoch, since the reading takes only their shape, period and
/// orientation.
///
/// # Errors
///
/// - [`ReadCoarseInputsError::NotAtRecordTime`] if the record's orbit does not hold at `t`: its
///   `valid_until` is `t` or earlier, or its drift cell starts after `t`;
/// - [`ReadCoarseInputsError::SeasonalOrbit`] if the model gives no bound orbit at `t`.
fn elements_at(
    orbit: &BodyOrbit,
    t: UniverseTime,
) -> Result<KeplerElements, ReadCoarseInputsError> {
    let ended = orbit.valid_until().is_some_and(|until| until <= t);
    let elements = *orbit.elements();
    let Some(drift) = orbit.drift() else {
        return if ended {
            Err(ReadCoarseInputsError::NotAtRecordTime)
        } else {
            Ok(elements)
        };
    };
    if ended || t < drift.reference() {
        return Err(ReadCoarseInputsError::NotAtRecordTime);
    }
    if t == drift.reference() {
        return Ok(elements);
    }
    let dt = seconds_between(drift.reference(), t);
    let model = || {
        let e = Eccentricity::new(
            elements.eccentricity().value() + drift.eccentricity_rate_per_s() * dt,
        )?;
        let n = TAU / elements.period().value() + drift.mean_motion_rate_rad_per_s2() * dt;
        let a = elements.semi_major_axis().value() + drift.semi_major_axis_rate_m_per_s() * dt;
        KeplerElements::from_period(
            Seconds::new(TAU / n),
            GravitationalParameter::new(n * n * a * a * a),
            e,
            *elements.orientation(),
            elements.mean_anomaly_at_epoch(),
        )
    };
    model().map_err(ReadCoarseInputsError::SeasonalOrbit)
}

/// The spin at [`RECORD_TIME`] of the body whose frame is `frame`, under the seasonal orbit
/// `seasonal`, which is `whose`: [`SeasonalSpin::of`]'s obliquity, sidereal period and solar day,
/// as the record's conditions read them, and the equinox's true anomaly
/// ([`equinox_true_anomaly`]).
#[must_use]
fn spin_of(frame: &BodyFixedFrame, seasonal: &KeplerElements, whose: SeasonsFrom) -> Spin {
    let spin = SeasonalSpin::of(frame, RECORD_TIME, seasonal, whose);
    Spin {
        obliquity: spin.obliquity,
        rotation_period: spin.rotation_period,
        solar_day: spin.solar_day,
        equinox_true_anomaly: equinox_true_anomaly(frame.pole(), seasonal.orientation()),
    }
}

/// The smallest in-plane share of the pole, |p̂ × n̂|, at which the equator crosses the seasonal
/// orbit's plane on a line: below it the body has no obliquity to speak of, and its equinox is
/// set at periapsis.
const EQUINOX_MIN_TILT: f64 = 1e-12;

/// The true anomaly on the orbit whose plane and periapsis are `orbit` at which the northern
/// spring equinox falls for a body whose pole is `pole`, rad, in [0, 2π): where the sun, seen
/// from the body, crosses its equator going north (as [`Spin::equinox_true_anomaly`] states it).
/// `pole` is the spin axis's unit vector on the system frame's axes, as
/// [`BodyFixedFrame::pole`] gives it, the axes of `orbit`'s elements (a moon's planet's elements
/// about its host, and the moon's frame, share them).
///
/// That sun lies along p̂ × n̂, with p̂ the pole and n̂ the orbit's normal, so the body lies along
/// n̂ × p̂, whose components on the orbit's periapsis direction P and the direction Q a quarter-turn
/// ahead of it give ν = atan2(p̂ · P, −p̂ · Q). For a moon the orbit is its planet's, and the sun
/// it sees is the planet's. A pole along the normal, within [`EQUINOX_MIN_TILT`], has no equinox,
/// and takes 0.
#[must_use]
fn equinox_true_anomaly(pole: [f64; 3], orbit: &Orientation) -> Radians {
    let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let p = orbit.periapsis_direction();
    let n = orbit.normal();
    // Q = n̂ × P, a quarter-turn ahead of periapsis in the direction of motion.
    let q = [
        n[1] * p[2] - n[2] * p[1],
        n[2] * p[0] - n[0] * p[2],
        n[0] * p[1] - n[1] * p[0],
    ];
    let (along_p, along_q) = (dot(pole, p), dot(pole, q));
    if math::hypot(along_p, along_q) < EQUINOX_MIN_TILT {
        return Radians::ZERO;
    }
    let anomaly = math::atan2(along_p, -along_q);
    // atan2 gives [−π, π] (−π for a −0 along P with the pole behind Q): a negative angle a turn
    // up, where −π comes to π and a tiny one rounds to a whole turn, which is periapsis again;
    // `+ 0.0` makes a −0 the +0 the range holds.
    let anomaly = if anomaly < 0.0 {
        anomaly + TAU
    } else {
        anomaly + 0.0
    };
    Radians::new(if anomaly < TAU { anomaly } else { 0.0 })
}

/// The shares of `shares` that lie on the ground, in the pass's vocabulary: every share but one of
/// no area (±0), so that the builder still refuses a negative or NaN share.
fn on_the_ground(shares: &[(SubstanceId, f64)]) -> impl Iterator<Item = AreaShare> + '_ {
    shares
        .iter()
        .filter(|&&(_, area_fraction)| area_fraction.classify() != FpCategory::Zero)
        .map(|&(id, area_fraction)| AreaShare {
            substance: id.substance().key(),
            area_fraction,
        })
}

/// The surface pressure the pass reads of a surface in the state `state` whose conditions give
/// `pressure`: exactly zero for an `Airless` state, the header's contract
/// (decision-r11-palette-liquids §2, item 5), and the conditions' for every other.
#[must_use]
fn surface_pressure_of(state: SurfaceState, pressure: Pascals) -> Pascals {
    match state {
        SurfaceState::Airless => Pascals::ZERO,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball => pressure,
    }
}

/// Why [`CoarseInputsBuilder::build`] refused its values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildCoarseInputsError {
    /// A value the pass needs was not given.
    Missing(&'static str),
    /// A value is not finite or outside its range (each value's documentation states it).
    OutOfRange {
        /// The value's name.
        part: &'static str,
        /// The value.
        value: f64,
    },
    /// The figure's radii are not finite with 0 < c ≤ a.
    Figure(Spheroid),
    /// The surface state is a gas envelope: the body has no solid surface.
    NoSolidSurface,
    /// The palette has no entry for a substance the inputs name where it lies: an ice, a liquid,
    /// a condensate on the ground as one, or a lithology of the crust or its bulk material.
    NotInPalette {
        /// The part that names it.
        part: &'static str,
        /// The substance.
        substance: SubstanceKey,
        /// The role its entry would take.
        role: PaletteRole,
    },
    /// The crust names neither a primary nor a secondary crust, nor an undifferentiated body's
    /// bulk material (science-composition-crusts.md §1).
    NoCrust,
    /// The crust names an undifferentiated body's bulk material beside a crust.
    BulkBesideCrust,
    /// The lid names a crust or bulk material the body does not have.
    LidAbsent(LidCrust),
    /// A list names a substance twice (a condensate twice in one phase).
    Duplicate {
        /// The list's name.
        part: &'static str,
        /// The substance.
        substance: SubstanceKey,
    },
    /// A list's shares add up to more than the whole.
    SharesExceedWhole {
        /// The list's name.
        part: &'static str,
        /// Their sum.
        total: f64,
    },
    /// A cold-trapped share is larger than its substance's ice share, or its substance has no ice
    /// share at all (decision-r09-t13f-cold-traps §1.3).
    ColdTrapExceedsIce {
        /// The substance.
        substance: SubstanceKey,
        /// Its cold-trapped share.
        trapped: f64,
        /// Its ice share, 0 where it has none.
        ice: f64,
    },
    /// A cold-trapped substance's registry row has no cold-trap threshold: no phase diagram or no
    /// molar mass, or no row at all (plan 14's `trap_threshold` is `None`), so it takes no cold
    /// traps.
    ColdTrapWithoutThreshold(SubstanceKey),
    /// More condensates than plan 14 lists.
    TooManyCondensates(usize),
    /// The wet epoch's liquid has no liquid density in the registry (no phase diagram), so its
    /// paleo-inventory has no volume and its erosion no fluid (decision-r09-t14-choices Q6a).
    WetEpochLiquid(SubstanceId),
    /// The wet epoch ends before it starts.
    WetEpochOrder {
        /// Its start, thousands of millions of years ago.
        start: Gigayears,
        /// Its end, thousands of millions of years ago.
        end: Gigayears,
    },
    /// The crater contract's gravity is not the body's.
    CraterGravity {
        /// The body's gravity.
        body: MetresPerSecondSquared,
        /// The crater contract's.
        craters: MetresPerSecondSquared,
    },
    /// The surface is older than its system, the crater contract's system age, which the
    /// chronology cannot count (R09.T7.e).
    SurfaceOlderThanSystem {
        /// The surface age.
        surface: Gigayears,
        /// The crater contract's system age.
        system: Gigayears,
    },
    /// The provinces' surface age is not finite or lies outside 0 to the surface age
    /// (decision-r09-t14-mars-ceiling §2.1).
    ProvinceSurfaceAge {
        /// The provinces' surface age.
        province: Gigayears,
        /// The surface age.
        surface: Gigayears,
    },
}

impl fmt::Display for BuildCoarseInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(part) => write!(f, "the coarse inputs lack the {part}"),
            Self::OutOfRange { part, value } => {
                write!(f, "the {part} {value} is not finite or is out of range")
            }
            Self::Figure(s) => write!(
                f,
                "figure a = {} m, c = {} m is not a spheroid with 0 < c ≤ a",
                s.equatorial_radius_m, s.polar_radius_m
            ),
            Self::NoSolidSurface => f.write_str("a gas envelope has no solid surface"),
            Self::NotInPalette {
                part,
                substance,
                role,
            } => write!(
                f,
                "the palette has no {role:?} entry for {substance}, named by the {part}"
            ),
            Self::NoCrust => f.write_str(
                "the crust has neither a primary nor a secondary crust nor a bulk material",
            ),
            Self::BulkBesideCrust => f.write_str("the crust names a bulk material beside a crust"),
            Self::LidAbsent(_) => {
                f.write_str("the lid names a crust or bulk material the body does not have")
            }
            Self::Duplicate { part, substance } => {
                write!(f, "the {part} name {substance} twice")
            }
            Self::SharesExceedWhole { part, total } => {
                write!(
                    f,
                    "the {part}' shares add up to {total}, more than the whole"
                )
            }
            Self::ColdTrapExceedsIce {
                substance,
                trapped,
                ice,
            } => write!(
                f,
                "the cold-trapped share {trapped} of {substance} exceeds its ice share {ice}"
            ),
            Self::ColdTrapWithoutThreshold(substance) => {
                write!(f, "{substance} has no cold-trap threshold")
            }
            Self::TooManyCondensates(count) => write!(
                f,
                "{count} condensates are more than the {MAX_CONDENSATES} plan 14 lists"
            ),
            Self::WetEpochLiquid(liquid) => write!(
                f,
                "the wet epoch's liquid {} has no liquid density in the registry",
                liquid.substance().key().as_str()
            ),
            Self::WetEpochOrder { start, end } => write!(
                f,
                "a wet epoch from {} Gyr ago cannot end {} Gyr ago",
                start.value(),
                end.value()
            ),
            Self::CraterGravity { body, craters } => write!(
                f,
                "the crater contract's gravity {} m s⁻² is not the body's {} m s⁻²",
                craters.value(),
                body.value()
            ),
            Self::SurfaceOlderThanSystem { surface, system } => write!(
                f,
                "a surface of {} Gyr is older than its system of {} Gyr",
                surface.value(),
                system.value()
            ),
            Self::ProvinceSurfaceAge { province, surface } => write!(
                f,
                "a province surface of {} Gyr is not from 0 to the surface's {} Gyr",
                province.value(),
                surface.value()
            ),
        }
    }
}

impl Error for BuildCoarseInputsError {}

/// Builds [`CoarseInputs`] from plain values, each the plan-14 figure it names (Design note 3,
/// "Until an ask lands").
///
/// Every value is required, the palette among them, but the body (by default
/// [`BodyRef::default`]) and the lists and optional parts, whose default is none: no liquid, ice,
/// cold-trapped share, gas or condensate, no cold-trap inputs, no seasonal orbit, no wet epoch, no
/// ground inertia of plan 14's and no weathering or organic cover.
/// The cold-trap inputs are required where a cold-trapped share is above zero.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoarseInputsBuilder {
    body: BodyRef,
    figure: Option<Spheroid>,
    gravity: Option<MetresPerSecondSquared>,
    sigma_h: Option<Metres>,
    liquids: Vec<AreaShare>,
    ices: Vec<AreaShare>,
    cold_traps: Vec<AreaShare>,
    cold_trap_inputs: Option<ColdTrapInputs>,
    mean_surface_temperature: Option<Kelvin>,
    equator_pole_contrast_k: Option<f64>,
    day_night_contrast_k: Option<f64>,
    spin: Option<Spin>,
    host_flux: Option<WattsPerSquareMetre>,
    seasonal_orbit: Option<SeasonalOrbit>,
    surface_pressure: Option<Pascals>,
    gases: Vec<GasShare>,
    surface_state: Option<SurfaceState>,
    surface_material: Option<SurfaceMaterial>,
    tectonics: Option<TectonicRegime>,
    continental_fraction: Option<f64>,
    volcanism: Option<f64>,
    heat_flow: Option<WattsPerSquareMetre>,
    surface_age: Option<Gigayears>,
    province_surface_age: Option<Gigayears>,
    craters: Option<CraterParams>,
    wet_epoch: Option<WetEpoch>,
    climate: Option<ClimateRegime>,
    crust: Option<CrustInputs>,
    condensates: Vec<Condensate>,
    palette: Option<MaterialPalette>,
    ground_inertia_j_per_m2_k_sqrt_s: Option<f64>,
    weathering: Option<GroundWeathering>,
    organic_cover: Option<f64>,
}

/// `value`, or [`BuildCoarseInputsError::Missing`] naming `part`.
fn required<T>(value: Option<T>, part: &'static str) -> Result<T, BuildCoarseInputsError> {
    value.ok_or(BuildCoarseInputsError::Missing(part))
}

/// Refuses `value` unless it is finite and `ok` holds for it.
fn check(part: &'static str, value: f64, ok: bool) -> Result<(), BuildCoarseInputsError> {
    if value.is_finite() && ok {
        Ok(())
    } else {
        Err(BuildCoarseInputsError::OutOfRange { part, value })
    }
}

/// Refuses a fraction outside 0 to 1.
fn check_fraction(part: &'static str, value: f64) -> Result<(), BuildCoarseInputsError> {
    check(part, value, (0.0..=1.0).contains(&value))
}

/// Refuses a share outside 0 to 1, or a −0, which a sort by `total_cmp` would put before a +0
/// and so order by the zero's sign rather than by key: a share sorts a list.
fn check_share(part: &'static str, value: f64) -> Result<(), BuildCoarseInputsError> {
    check(
        part,
        value,
        (0.0..=1.0).contains(&value) && value.is_sign_positive(),
    )
}

/// Refuses a value that is negative or not finite.
fn check_non_negative(part: &'static str, value: f64) -> Result<(), BuildCoarseInputsError> {
    check(part, value, value >= 0.0)
}

/// Refuses a value that is not finite and positive.
fn check_positive(part: &'static str, value: f64) -> Result<(), BuildCoarseInputsError> {
    check(part, value, value > 0.0)
}

/// Validates a list of area shares and sorts it largest first, ties by key.
fn area_shares(
    part: &'static str,
    mut shares: Vec<AreaShare>,
) -> Result<Vec<AreaShare>, BuildCoarseInputsError> {
    for share in &shares {
        check_share(part, share.area_fraction)?;
    }
    shares.sort_by(|a, b| {
        b.area_fraction
            .total_cmp(&a.area_fraction)
            .then_with(|| a.substance.cmp(&b.substance))
    });
    refuse_duplicates(part, shares.iter().map(|s| (s.substance, ())))?;
    let total: f64 = shares.iter().map(|s| s.area_fraction).sum();
    if total > 1.0 + SHARE_TOLERANCE {
        return Err(BuildCoarseInputsError::SharesExceedWhole { part, total });
    }
    Ok(shares)
}

/// Validates the cold-trapped shares `shares` against the ices `ices` and sorts them largest
/// first, ties by key (decision-r09-t13f-cold-traps §1.3), refusing in this order: a share that is
/// not finite, outside 0 to 1 or −0 ([`BuildCoarseInputsError::OutOfRange`]), a substance named
/// twice ([`BuildCoarseInputsError::Duplicate`]), and then, share by share in their order, one
/// larger than its substance's ice share or of a substance with no ice share at all
/// ([`BuildCoarseInputsError::ColdTrapExceedsIce`]) and one of a row with no cold-trap threshold
/// ([`BuildCoarseInputsError::ColdTrapWithoutThreshold`]). Their sum needs no test of its own: each
/// is at most its ice's, and the ices' sum is at most the whole.
fn cold_trap_shares(
    mut shares: Vec<AreaShare>,
    ices: &[AreaShare],
) -> Result<Vec<AreaShare>, BuildCoarseInputsError> {
    const PART: &str = "cold traps";
    for share in &shares {
        check_share(PART, share.area_fraction)?;
    }
    shares.sort_by(|a, b| {
        b.area_fraction
            .total_cmp(&a.area_fraction)
            .then_with(|| a.substance.cmp(&b.substance))
    });
    refuse_duplicates(PART, shares.iter().map(|s| (s.substance, ())))?;
    for share in &shares {
        let ice = ices.iter().find(|ice| ice.substance == share.substance);
        let held = ice.map_or(0.0, |ice| ice.area_fraction);
        if ice.is_none() || share.area_fraction > held {
            return Err(BuildCoarseInputsError::ColdTrapExceedsIce {
                substance: share.substance,
                trapped: share.area_fraction,
                ice: held,
            });
        }
        if by_key(share.substance.as_str())
            .and_then(trap_threshold)
            .is_none()
        {
            return Err(BuildCoarseInputsError::ColdTrapWithoutThreshold(
                share.substance,
            ));
        }
    }
    Ok(shares)
}

/// The cold-trapped shares `shares`, validated against the ices `ices` ([`cold_trap_shares`]),
/// sorted, with the cold-trap inputs `inputs` checked where they are given
/// ([`check_cold_trap_inputs`]) and required where a share is above zero
/// ([`BuildCoarseInputsError::Missing`]), in that order.
fn cold_traps_of(
    shares: Vec<AreaShare>,
    inputs: Option<&ColdTrapInputs>,
    ices: &[AreaShare],
) -> Result<Vec<AreaShare>, BuildCoarseInputsError> {
    let shares = cold_trap_shares(shares, ices)?;
    match inputs {
        Some(group) => check_cold_trap_inputs(group)?,
        None if shares.iter().any(|s| s.area_fraction > 0.0) => {
            return Err(BuildCoarseInputsError::Missing("cold-trap inputs"));
        }
        None => {}
    }
    Ok(shares)
}

/// Refuses cold-trap inputs outside their ranges (decision-r09-t13f-cold-traps §2.2): a star's
/// angular radius outside [0, π/2), or positive with a square that is not a normal `f64`
/// (`hyperion_surface::cold_trap::Body`'s rule), and a Bond albedo outside [0, 1).
fn check_cold_trap_inputs(inputs: &ColdTrapInputs) -> Result<(), BuildCoarseInputsError> {
    let alpha = inputs.star_angular_radius.value();
    let normal = alpha <= 0.0 || (alpha * alpha).is_normal();
    check(
        "star angular radius",
        alpha,
        normal && (0.0..FRAC_PI_2).contains(&alpha),
    )?;
    let albedo = inputs.bond_albedo;
    check(
        "cold-trap Bond albedo",
        albedo,
        (0.0..1.0).contains(&albedo),
    )
}

/// Refuses a list in which a key occurs twice: each item is a substance and a second key that
/// may tell two items of one substance apart.
fn refuse_duplicates<K: Ord>(
    part: &'static str,
    items: impl IntoIterator<Item = (SubstanceKey, K)>,
) -> Result<(), BuildCoarseInputsError> {
    let mut seen = std::collections::BTreeSet::new();
    for (substance, key) in items {
        if !seen.insert((substance, key)) {
            return Err(BuildCoarseInputsError::Duplicate { part, substance });
        }
    }
    Ok(())
}

impl CoarseInputsBuilder {
    /// The body (by default [`BodyRef::default`]).
    #[must_use]
    pub const fn body(mut self, body: BodyRef) -> Self {
        self.body = body;
        self
    }

    /// The body's rotational spheroid (P14.T46's `figure()`), finite with 0 < c ≤ a.
    #[must_use]
    pub const fn figure(mut self, figure: Spheroid) -> Self {
        self.figure = Some(figure);
        self
    }

    /// The mean surface gravity, finite and positive.
    #[must_use]
    pub const fn gravity(mut self, gravity: MetresPerSecondSquared) -> Self {
        self.gravity = Some(gravity);
        self
    }

    /// Plan 14's `σ_h`, finite and non-negative.
    #[must_use]
    pub const fn sigma_h(mut self, sigma_h: Metres) -> Self {
        self.sigma_h = Some(sigma_h);
        self
    }

    /// The surface liquids by substance, in any order, each share 0 to 1 and their sum at most 1.
    #[must_use]
    pub fn liquids(mut self, liquids: impl IntoIterator<Item = AreaShare>) -> Self {
        self.liquids = liquids.into_iter().collect();
        self
    }

    /// The surface ices by substance, in any order, each share 0 to 1 and their sum at most 1.
    #[must_use]
    pub fn ices(mut self, ices: impl IntoIterator<Item = AreaShare>) -> Self {
        self.ices = ices.into_iter().collect();
        self
    }

    /// The cold-trapped part of each ice's share, by substance, in any order, each 0 to 1 and at
    /// most its substance's [`ices`](Self::ices) share. The default is none.
    #[must_use]
    pub fn cold_traps(mut self, cold_traps: impl IntoIterator<Item = AreaShare>) -> Self {
        self.cold_traps = cold_traps.into_iter().collect();
        self
    }

    /// What the cold traps read of the star and the ground, required where any cold-trapped share
    /// is above zero.
    #[must_use]
    pub const fn cold_trap_inputs(mut self, inputs: ColdTrapInputs) -> Self {
        self.cold_trap_inputs = Some(inputs);
        self
    }

    /// The mean surface temperature, finite and positive.
    #[must_use]
    pub const fn mean_surface_temperature(mut self, temperature: Kelvin) -> Self {
        self.mean_surface_temperature = Some(temperature);
        self
    }

    /// The signed equator–pole contrast, kelvin, finite: negative for a warm pole.
    #[must_use]
    pub const fn equator_pole_contrast_k(mut self, contrast: f64) -> Self {
        self.equator_pole_contrast_k = Some(contrast);
        self
    }

    /// The signed day–night contrast, kelvin, finite.
    #[must_use]
    pub const fn day_night_contrast_k(mut self, contrast: f64) -> Self {
        self.day_night_contrast_k = Some(contrast);
        self
    }

    /// The spin.
    #[must_use]
    pub const fn spin(mut self, spin: Spin) -> Self {
        self.spin = Some(spin);
        self
    }

    /// The hosts' orbit-averaged light at the body, finite and non-negative.
    #[must_use]
    pub const fn host_flux(mut self, flux: WattsPerSquareMetre) -> Self {
        self.host_flux = Some(flux);
        self
    }

    /// The orbit that sets the seasons (none by default, for a body that orbits no star).
    #[must_use]
    pub const fn seasonal_orbit(mut self, orbit: SeasonalOrbit) -> Self {
        self.seasonal_orbit = Some(orbit);
        self
    }

    /// The mean surface pressure, finite and non-negative.
    #[must_use]
    pub const fn surface_pressure(mut self, pressure: Pascals) -> Self {
        self.surface_pressure = Some(pressure);
        self
    }

    /// The gases at the surface, in any order, each fraction above 0 and at most 1 and their sum
    /// at most 1.
    #[must_use]
    pub fn gases(mut self, gases: impl IntoIterator<Item = GasShare>) -> Self {
        self.gases = gases.into_iter().collect();
        self
    }

    /// The surface state and material.
    #[must_use]
    pub const fn surface(mut self, state: SurfaceState, material: SurfaceMaterial) -> Self {
        self.surface_state = Some(state);
        self.surface_material = Some(material);
        self
    }

    /// The tectonic regime and the continental fraction, 0 to 1.
    #[must_use]
    pub const fn tectonics(mut self, regime: TectonicRegime, continental_fraction: f64) -> Self {
        self.tectonics = Some(regime);
        self.continental_fraction = Some(continental_fraction);
        self
    }

    /// The volcanism level V, finite and non-negative.
    #[must_use]
    pub const fn volcanism(mut self, volcanism: f64) -> Self {
        self.volcanism = Some(volcanism);
        self
    }

    /// The surface heat flow, finite and non-negative.
    #[must_use]
    pub const fn heat_flow(mut self, heat_flow: WattsPerSquareMetre) -> Self {
        self.heat_flow = Some(heat_flow);
        self
    }

    /// The surface age, finite and non-negative, and at most the crater contract's system age.
    #[must_use]
    pub const fn surface_age(mut self, age: Gigayears) -> Self {
        self.surface_age = Some(age);
        self
    }

    /// The volcanic provinces' surface age, finite and from 0 to the surface age
    /// ([`CoarseInputs::province_surface_age`]).
    #[must_use]
    pub const fn province_surface_age(mut self, age: Gigayears) -> Self {
        self.province_surface_age = Some(age);
        self
    }

    /// The crater contract, whose gravity must be the body's and whose system must be at least
    /// as old as the surface.
    #[must_use]
    pub const fn craters(mut self, craters: CraterParams) -> Self {
        self.craters = Some(craters);
        self
    }

    /// The wet epoch (none by default).
    #[must_use]
    pub const fn wet_epoch(mut self, epoch: WetEpoch) -> Self {
        self.wet_epoch = Some(epoch);
        self
    }

    /// The climate regime.
    #[must_use]
    pub const fn climate(mut self, regime: ClimateRegime) -> Self {
        self.climate = Some(regime);
        self
    }

    /// The crust by substance.
    #[must_use]
    pub const fn crust(mut self, crust: CrustInputs) -> Self {
        self.crust = Some(crust);
        self
    }

    /// The condensates, in any order, at most [`MAX_CONDENSATES`], each substance once a phase.
    #[must_use]
    pub fn condensates(mut self, condensates: impl IntoIterator<Item = Condensate>) -> Self {
        self.condensates = condensates.into_iter().collect();
        self
    }

    /// The body's palette, as the server resolves it from plan 14's registry.
    ///
    /// Its entries' values are the registry rows' (P14.T49.b–c, T49.h and T49.i), which
    /// [`CoarseInputs::for_body`] resolves for the record's substances (P14.T54.a). It must hold an
    /// [`PaletteRole::Ice`] entry for every
    /// ice and solid condensate, a [`PaletteRole::Liquid`] entry for every liquid and liquid
    /// condensate, and an entry for every lithology of the crust, a buried primary crust's and the
    /// bulk material's among them, as [`CoarseInputs::crust_palette`] finds them; it may hold
    /// more, such as a deposit's
    /// ([`PaletteRole::Deposit`], the sand and dust source where it is not the crust's: a Mars's
    /// `mars_dust`, a Ceres's `Na2CO3`).
    #[must_use]
    pub fn palette(mut self, palette: MaterialPalette) -> Self {
        self.palette = Some(palette);
        self
    }

    /// The ground's thermal inertia Γ, J m⁻² K⁻¹ s^−½, finite and positive: plan 14's, the
    /// record's conditions' ([`CoarseInputs::ground_inertia_j_per_m2_k_sqrt_s`]). None by
    /// default: inputs built by hand take P14.T49.j's law at their lid at their equilibrium
    /// temperature where the radiative model reads it.
    #[must_use]
    pub const fn ground_inertia_j_per_m2_k_sqrt_s(mut self, inertia: f64) -> Self {
        self.ground_inertia_j_per_m2_k_sqrt_s = Some(inertia);
        self
    }

    /// The ground's space weathering, its maturity in [+0, 1] with its weathering rate, finite and
    /// at least +0, neither a −0 ([`CoarseInputs::weathering`]). None by default.
    #[must_use]
    pub const fn weathering(mut self, weathering: GroundWeathering) -> Self {
        self.weathering = Some(weathering);
        self
    }

    /// The organic cover, in [+0, 1], not a −0 ([`CoarseInputs::organic_cover`]). None by default.
    #[must_use]
    pub const fn organic_cover(mut self, organic_cover: f64) -> Self {
        self.organic_cover = Some(organic_cover);
        self
    }

    /// The inputs, validated, with every list in its documented order.
    ///
    /// # Errors
    ///
    /// [`BuildCoarseInputsError::Missing`] for a required value not given, the cold-trap inputs
    /// among them where a cold-trapped share is above zero, and otherwise the first rule broken,
    /// as each value's setter states it: a value out of range or not finite, a figure that is not
    /// a spheroid, a gas-envelope surface, a substance named twice, shares above the whole, a
    /// cold-trapped share above its ice's or of a substance with no ice share
    /// ([`BuildCoarseInputsError::ColdTrapExceedsIce`]) or with no cold-trap threshold
    /// ([`BuildCoarseInputsError::ColdTrapWithoutThreshold`]), cold-trap inputs out of range,
    /// too many condensates, a wet epoch that ends before it starts, the provinces' surface age
    /// not finite or outside zero to the surface age
    /// ([`BuildCoarseInputsError::ProvinceSurfaceAge`]), a crater contract of another gravity or
    /// of a system younger than the surface
    /// ([`BuildCoarseInputsError::SurfaceOlderThanSystem`]), a crust with neither a primary nor a
    /// secondary crust nor a bulk material ([`BuildCoarseInputsError::NoCrust`]), with a bulk
    /// material beside a crust ([`BuildCoarseInputsError::BulkBesideCrust`]) or with a lid naming
    /// a crust or bulk material it does not have ([`BuildCoarseInputsError::LidAbsent`]), or a
    /// substance the palette has no entry for.
    #[expect(
        clippy::too_many_lines,
        reason = "one value's rule a line, in the order the errors list them"
    )]
    pub fn build(self) -> Result<CoarseInputs, BuildCoarseInputsError> {
        let figure = required(self.figure, "figure")?;
        let Spheroid {
            equatorial_radius_m: a,
            polar_radius_m: c,
        } = figure;
        if !(a.is_finite() && c.is_finite() && c > 0.0 && c <= a) {
            return Err(BuildCoarseInputsError::Figure(figure));
        }
        let gravity = required(self.gravity, "gravity")?;
        check_positive("gravity", gravity.value())?;
        let sigma_h = required(self.sigma_h, "sigma_h")?;
        check_non_negative("sigma_h", sigma_h.value())?;
        let liquids = area_shares("liquids", self.liquids)?;
        let ices = area_shares("ices", self.ices)?;
        let cold_traps = cold_traps_of(self.cold_traps, self.cold_trap_inputs.as_ref(), &ices)?;
        let mean_surface_temperature =
            required(self.mean_surface_temperature, "mean surface temperature")?;
        check_positive("mean surface temperature", mean_surface_temperature.value())?;
        let equator_pole_contrast_k =
            required(self.equator_pole_contrast_k, "equator-pole contrast")?;
        check("equator-pole contrast", equator_pole_contrast_k, true)?;
        let day_night_contrast_k = required(self.day_night_contrast_k, "day-night contrast")?;
        check("day-night contrast", day_night_contrast_k, true)?;
        let spin = required(self.spin, "spin")?;
        check_spin(&spin)?;
        let host_flux = required(self.host_flux, "host flux")?;
        check_non_negative("host flux", host_flux.value())?;
        if let Some(orbit) = &self.seasonal_orbit {
            // A −0 is refused, as the field's header refuses it, so a circular orbit has one form.
            check(
                "seasonal eccentricity",
                orbit.eccentricity,
                (0.0..1.0).contains(&orbit.eccentricity) && orbit.eccentricity.is_sign_positive(),
            )?;
            check_positive("seasonal period", orbit.period.value())?;
        }
        let surface_pressure = required(self.surface_pressure, "surface pressure")?;
        check_non_negative("surface pressure", surface_pressure.value())?;
        let gases = gas_shares(self.gases)?;
        let surface_state = required(self.surface_state, "surface state")?;
        if surface_state == SurfaceState::GasEnvelope {
            return Err(BuildCoarseInputsError::NoSolidSurface);
        }
        let surface_material = required(self.surface_material, "surface material")?;
        let tectonics = required(self.tectonics, "tectonic regime")?;
        let continental_fraction = required(self.continental_fraction, "continental fraction")?;
        check_fraction("continental fraction", continental_fraction)?;
        let volcanism = required(self.volcanism, "volcanism")?;
        check_non_negative("volcanism", volcanism)?;
        let heat_flow = required(self.heat_flow, "heat flow")?;
        check_non_negative("heat flow", heat_flow.value())?;
        let surface_age = required(self.surface_age, "surface age")?;
        check_non_negative("surface age", surface_age.value())?;
        let province_surface_age = province_surface_age(self.province_surface_age, surface_age)?;
        let craters = required(self.craters, "crater contract")?;
        check_craters(&craters, gravity, surface_age)?;
        if let Some(epoch) = &self.wet_epoch {
            check_wet_epoch(epoch)?;
        }
        let climate = required(self.climate, "climate regime")?;
        let crust = required(self.crust, "crust")?;
        let lid = check_crust(&crust)?;
        let condensates = condensates(self.condensates)?;
        let palette = required(self.palette, "palette")?;
        let main_liquid = check_on_the_ground(&palette, &liquids, &ices, &condensates)?;
        let (crust_palette, lid_entry) = crust_palette(&palette, &crust, lid)?;
        if let Some(inertia) = self.ground_inertia_j_per_m2_k_sqrt_s {
            check_positive("ground inertia", inertia)?;
        }
        if let Some(weathering) = &self.weathering {
            check_share("maturity", weathering.maturity)?;
            let rate = weathering.weathering_rate;
            check(
                "weathering rate",
                rate,
                rate >= 0.0 && rate.is_sign_positive(),
            )?;
        }
        if let Some(cover) = self.organic_cover {
            check_share("organic cover", cover)?;
        }
        Ok(CoarseInputs {
            body: self.body,
            figure,
            gravity,
            sigma_h,
            liquids,
            ices,
            cold_traps,
            cold_trap_inputs: self.cold_trap_inputs,
            mean_surface_temperature,
            equator_pole_contrast_k,
            day_night_contrast_k,
            spin,
            host_flux,
            seasonal_orbit: self.seasonal_orbit,
            surface_pressure,
            gases,
            surface_state,
            surface_material,
            tectonics,
            continental_fraction,
            volcanism,
            heat_flow,
            surface_age,
            province_surface_age,
            craters,
            wet_epoch: self.wet_epoch,
            climate,
            crust,
            condensates,
            palette,
            crust_palette,
            lid_entry,
            main_liquid,
            ground_inertia_j_per_m2_k_sqrt_s: self.ground_inertia_j_per_m2_k_sqrt_s,
            weathering: self.weathering,
            organic_cover: self.organic_cover,
        })
    }
}

/// Refuses a spin outside its ranges.
fn check_spin(spin: &Spin) -> Result<(), BuildCoarseInputsError> {
    let obliquity = spin.obliquity.value();
    check(
        "obliquity",
        obliquity,
        (0.0..=core::f64::consts::PI).contains(&obliquity),
    )?;
    check_positive("rotation period", spin.rotation_period.value())?;
    if let Some(day) = spin.solar_day {
        check_positive("solar day", day.value())?;
    }
    let equinox = spin.equinox_true_anomaly.value();
    check(
        "equinox true anomaly",
        equinox,
        (0.0..core::f64::consts::TAU).contains(&equinox),
    )
}

/// The substance of the crust or bulk material `crust.lid` names, refusing, in this order, a crust
/// with neither a primary nor a secondary crust nor a bulk material, a bulk material beside a
/// crust, a lid naming a crust or bulk material it does not have, and a melt share outside 0 to 1.
fn check_crust(crust: &CrustInputs) -> Result<SubstanceKey, BuildCoarseInputsError> {
    if crust.primary.is_none() && crust.secondary.is_none() && crust.bulk.is_none() {
        return Err(BuildCoarseInputsError::NoCrust);
    }
    let crusts = [crust.primary, crust.secondary, crust.tertiary];
    if crust.bulk.is_some() && crusts.iter().any(Option::is_some) {
        return Err(BuildCoarseInputsError::BulkBesideCrust);
    }
    let lid = match crust.lid {
        LidCrust::Primary => crust.primary,
        LidCrust::Secondary => crust.secondary,
        LidCrust::Bulk => crust.bulk,
    }
    .ok_or(BuildCoarseInputsError::LidAbsent(crust.lid))?;
    check_fraction("melt area fraction", crust.melt_area_fraction)?;
    Ok(lid)
}

/// The provinces' surface age `province`, refused where it is missing, not finite, or outside 0
/// to the surface age `surface`.
fn province_surface_age(
    province: Option<Gigayears>,
    surface: Gigayears,
) -> Result<Gigayears, BuildCoarseInputsError> {
    let province = required(province, "province surface age")?;
    let age = province.value();
    if age.is_finite() && age >= 0.0 && age <= surface.value() {
        Ok(province)
    } else {
        Err(BuildCoarseInputsError::ProvinceSurfaceAge { province, surface })
    }
}

/// Refuses a wet epoch outside its ranges or ending before it starts.
fn check_wet_epoch(epoch: &WetEpoch) -> Result<(), BuildCoarseInputsError> {
    check_non_negative("wet epoch's start", epoch.start.value())?;
    check_non_negative("wet epoch's end", epoch.end.value())?;
    if epoch.end.value() > epoch.start.value() {
        return Err(BuildCoarseInputsError::WetEpochOrder {
            start: epoch.start,
            end: epoch.end,
        });
    }
    check_non_negative("wet epoch's effective flow", epoch.effective_flow.value())?;
    check_non_negative("wet epoch's runoff", epoch.runoff_m_per_year)?;
    if epoch.liquid.substance().phase_diagram().is_none() {
        return Err(BuildCoarseInputsError::WetEpochLiquid(epoch.liquid));
    }
    check_non_negative("wet epoch's paleo-inventory", epoch.paleo_inventory.value())
}

/// The entry of `substance` in `role`, or else in the first of `fallbacks` that has one, that
/// `palette` holds, or [`BuildCoarseInputsError::NotInPalette`] naming `part` and `role`.
fn entry(
    palette: &MaterialPalette,
    part: &'static str,
    substance: SubstanceKey,
    role: PaletteRole,
    fallbacks: impl IntoIterator<Item = PaletteRole>,
) -> Result<u8, BuildCoarseInputsError> {
    core::iter::once(role)
        .chain(fallbacks)
        .find_map(|r| palette.find(r, substance))
        .ok_or(BuildCoarseInputsError::NotInPalette {
            part,
            substance,
            role,
        })
}

/// Refuses an ice, a liquid or a condensate on the ground that `palette` has no entry for, and
/// returns the main liquid's entry: the first of `liquids`, which are sorted largest first.
///
/// A supercritical condensate has no interface, so it lies on the ground as neither and names no
/// entry.
fn check_on_the_ground(
    palette: &MaterialPalette,
    liquids: &[AreaShare],
    ices: &[AreaShare],
    condensates: &[Condensate],
) -> Result<Option<u8>, BuildCoarseInputsError> {
    let mut main_liquid = None;
    for share in liquids {
        let at = entry(
            palette,
            "liquids",
            share.substance,
            PaletteRole::Liquid,
            None,
        )?;
        main_liquid = main_liquid.or(Some(at));
    }
    for share in ices {
        entry(palette, "ices", share.substance, PaletteRole::Ice, None)?;
    }
    for c in condensates {
        let role = match c.phase {
            CondensatePhase::Solid => PaletteRole::Ice,
            CondensatePhase::Liquid => PaletteRole::Liquid,
            CondensatePhase::Supercritical => continue,
        };
        entry(palette, "condensates", c.substance, role, None)?;
    }
    Ok(main_liquid)
}

/// The palette entry of each [`Crust`] variant's lithology, in its code order, and the lid's,
/// by the rule [`CoarseInputs::crust_palette`] states, refusing a lithology of `crust` that
/// `palette` has no entry for. `lid` is the lid's substance, which [`check_crust`] resolved from
/// `crust.lid`.
fn crust_palette(
    palette: &MaterialPalette,
    crust: &CrustInputs,
    lid: SubstanceKey,
) -> Result<([Option<u8>; 4], u8), BuildCoarseInputsError> {
    // Where a crust or the bulk material and the provinces are one substance, its entry names the
    // provinces too, and the secondary crust may take the provinces' own.
    let secondary_shared = crust.secondary == Some(crust.provinces);
    let primary_shared = crust.primary == Some(crust.provinces);
    let bulk_shared = crust.bulk == Some(crust.provinces);
    // The entry of `substance` as the primary crust, the secondary or the bulk material, whichever
    // `member` names: the lid takes its entry by the same rule as that member's own.
    let entry_of = |member: LidCrust, substance| match member {
        LidCrust::Primary => entry(
            palette,
            "primary crust",
            substance,
            PaletteRole::PrimaryCrust,
            None,
        ),
        LidCrust::Secondary => entry(
            palette,
            "secondary crust",
            substance,
            PaletteRole::SecondaryCrust,
            secondary_shared.then_some(PaletteRole::Province),
        ),
        LidCrust::Bulk => entry(palette, "bulk material", substance, PaletteRole::Bulk, None),
    };
    let secondary = crust
        .secondary
        .map(|s| entry_of(LidCrust::Secondary, s))
        .transpose()?;
    // A buried primary crust names no variant's entry but keeps its own, which large craters
    // excavate (science-composition-crusts.md §5.1).
    if let Some(primary) = crust.primary {
        entry_of(LidCrust::Primary, primary)?;
    }
    // A bulk material stands alone, so it is the lid's, and its entry is the lid's.
    let lid = entry_of(crust.lid, lid)?;
    let tertiary = crust
        .tertiary
        .map(|t| {
            entry(
                palette,
                "tertiary crust",
                t,
                PaletteRole::TertiaryCrust,
                None,
            )
        })
        .transpose()?;
    let provinces = entry(
        palette,
        "provinces",
        crust.provinces,
        PaletteRole::Province,
        [
            (secondary_shared, PaletteRole::SecondaryCrust),
            (primary_shared, PaletteRole::PrimaryCrust),
            (bulk_shared, PaletteRole::Bulk),
        ]
        .into_iter()
        .filter_map(|(shared, role)| shared.then_some(role)),
    )?;
    let mut entries = [None; 4];
    for (slot, crust) in entries.iter_mut().zip(Crust::ALL) {
        *slot = match crust {
            Crust::Continental => tertiary,
            Crust::Oceanic => secondary,
            Crust::Lid => Some(lid),
            Crust::Province => Some(provinces),
        };
    }
    Ok((entries, lid))
}

/// Validates the gases and sorts them largest first, ties by key.
fn gas_shares(mut gases: Vec<GasShare>) -> Result<Vec<GasShare>, BuildCoarseInputsError> {
    for gas in &gases {
        check(
            "mole fraction",
            gas.mole_fraction,
            gas.mole_fraction > 0.0 && gas.mole_fraction <= 1.0,
        )?;
    }
    gases.sort_by(|a, b| {
        b.mole_fraction
            .total_cmp(&a.mole_fraction)
            .then_with(|| a.substance.cmp(&b.substance))
    });
    refuse_duplicates("gases", gases.iter().map(|g| (g.substance, ())))?;
    let total: f64 = gases.iter().map(|g| g.mole_fraction).sum();
    if total > 1.0 + SHARE_TOLERANCE {
        return Err(BuildCoarseInputsError::SharesExceedWhole {
            part: "gases",
            total,
        });
    }
    Ok(gases)
}

/// Validates the condensates and sorts them by reservoir, largest first, ties by key then phase.
fn condensates(mut list: Vec<Condensate>) -> Result<Vec<Condensate>, BuildCoarseInputsError> {
    if list.len() > MAX_CONDENSATES {
        return Err(BuildCoarseInputsError::TooManyCondensates(list.len()));
    }
    for c in &list {
        // The reservoir sorts the list, so a −0 is refused as a share's is.
        let reservoir = c.reservoir.value();
        check(
            "condensate's reservoir",
            reservoir,
            reservoir >= 0.0 && reservoir.is_sign_positive(),
        )?;
        check_share("condensate's area fraction", c.area_fraction)?;
    }
    list.sort_by(|a, b| {
        b.reservoir
            .value()
            .total_cmp(&a.reservoir.value())
            .then_with(|| a.substance.cmp(&b.substance))
            .then_with(|| a.phase.cmp(&b.phase))
    });
    refuse_duplicates("condensates", list.iter().map(|c| (c.substance, c.phase)))?;
    Ok(list)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::reference::{self, builder_of};
    use super::*;
    use crate::galaxy::placement::CellKey;
    use crate::id::Layer;
    use crate::planetary::derive::PlanetClass;
    use crate::planetary::derive::atmosphere::Gas;
    use crate::planetary::generate;
    use crate::planetary::hooks::cold_traps::COLD_TRAPS_VERSION;
    use crate::planetary::record::DetailLevel;
    use crate::planetary::testing::{SampleFilter, sample_contexts, synthetic_star};
    use crate::time::UniverseTime;
    use crate::units::{Degrees, Dex, SolarMasses};
    use crate::{GENERATOR_VERSION, Seed};
    use hyperion_surface::field::{
        FieldView, LiquidBasis, LiquidProperties, MechanicsFamily, PaletteEntry,
    };
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden::GoldenWriter;

    /// The key `key`, which a test knows to be valid.
    fn key(key: &str) -> SubstanceKey {
        SubstanceKey::new(key).unwrap()
    }

    /// A sea of liquid methane at Titan's surface temperature, 93.65 K, `Derived`: the surface
    /// crate's synthetic record (`hyperion_surface::testing::liquid_methane_titan`, whose figures
    /// and sources these are, which this crate's tests cannot reach without its `testing`
    /// feature): Setzmann and Wagner 1991's 447.6 kg m⁻³ at 93.65 K and Huygens's 1,467 hPa, and
    /// Martonchik and Orton 1994's n at 90 K carried to it by Lorentz–Lorenz from 452.4 kg m⁻³,
    /// the registry's scattering at 93.65 K (P14.T49.h; decision-r10-t10b-r09-t2e §7), copied as
    /// shortest round-trip decimals and held to the bit by
    /// `the_methane_sea_carries_the_registry_s_scattering`, the other members not modelled
    /// (decision-r11-palette-liquids §1.8). The client's
    /// `view/atmosphere/materials/paletteLiquids.test.ts` holds its bins to the file's.
    fn methane_sea() -> LiquidProperties {
        LiquidProperties {
            temperature: Kelvin::new(93.65),
            density_kg_m3: 447.6,
            basis: LiquidBasis::Derived,
            refractive_index: [
                1.296_516, 1.295_764, 1.294_671, 1.293_578, 1.292_486, 1.291_394, 1.290_300,
                1.289_209, 1.288_562, 1.287_754, 1.287_153, 1.286_718, 1.286_668, 1.286_660,
                1.286_355,
            ],
            absorption_per_m: None,
            scattering_per_m: Some([
                0.003_865_809_149_989_438_4,
                0.002_989_879_265_953_119,
                0.002_339_992_177_513_175_5,
                0.001_855_513_682_522_143_7,
                0.001_488_726_493_932_331_1,
                0.001_207_091_327_156_444_5,
                0.000_988_092_827_130_07,
                0.000_815_891_544_344_150_6,
                0.000_681_679_037_661_234_1,
                0.000_572_882_364_591_951_5,
                0.000_485_555_912_859_613_3,
                0.000_414_686_511_108_925_1,
                0.000_357_392_279_184_591_24,
                0.000_309_766_467_640_019_4,
                0.000_269_111_878_909_328_93,
            ]),
            surface_tension_n_per_m: None,
            viscosity_pa_s: None,
        }
    }

    /// An entry of `substance` in `role`, with any valid values; a liquid's with a record, a sea of
    /// methane at Titan's temperature ([`methane_sea`]), and its record's density.
    fn entry(substance: &str, role: PaletteRole) -> PaletteEntry {
        let liquid = (role == PaletteRole::Liquid).then(methane_sea);
        PaletteEntry {
            substance: key(substance),
            role,
            normal_albedo_bvr: [0.1; 3],
            phase_row: 0,
            grain_density_kg_m3: liquid.as_ref().map_or(1_000.0, |l| l.density_kg_m3),
            transition: Kelvin::new(300.0),
            mechanics: MechanicsFamily::Silicate,
            liquid,
            feo_mass_fraction: None,
        }
    }

    /// The palette of `entries`, sorted.
    fn palette(mut entries: Vec<PaletteEntry>) -> MaterialPalette {
        entries.sort_by_key(|e| (e.role, e.substance));
        MaterialPalette::new(entries).unwrap()
    }

    /// The reference Earth's builder values, as a builder to vary.
    fn earth_builder() -> CoarseInputsBuilder {
        builder_of(&reference::earth_like())
    }

    #[test]
    fn the_builder_gives_back_what_it_was_given() {
        for world in [
            reference::earth_like(),
            reference::mars_like(),
            reference::moon_like(),
            reference::ceres_like(),
        ] {
            assert_eq!(builder_of(&world).build().unwrap(), world);
        }
    }

    #[test]
    fn the_builder_needs_every_figure_but_the_lists() {
        assert_eq!(
            CoarseInputs::builder().build(),
            Err(BuildCoarseInputsError::Missing("figure"))
        );
        let mut missing = earth_builder();
        missing.crust = None;
        assert_eq!(
            missing.build(),
            Err(BuildCoarseInputsError::Missing("crust"))
        );
        let mut missing = earth_builder();
        missing.palette = None;
        assert_eq!(
            missing.build(),
            Err(BuildCoarseInputsError::Missing("palette"))
        );
        let bare = earth_builder()
            .liquids([])
            .ices([])
            .gases([])
            .condensates([])
            .build()
            .unwrap();
        assert!(bare.liquids().is_empty() && bare.condensates().is_empty());
        assert!(bare.ocean_fraction().abs() < 1e-15);
    }

    #[test]
    fn the_builder_refuses_values_out_of_range() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        assert_eq!(
            refused(earth_builder().gravity(MetresPerSecondSquared::new(0.0))),
            BuildCoarseInputsError::OutOfRange {
                part: "gravity",
                value: 0.0
            }
        );
        let inverted = Spheroid {
            equatorial_radius_m: 1e6,
            polar_radius_m: 2e6,
        };
        assert_eq!(
            refused(earth_builder().figure(inverted)),
            BuildCoarseInputsError::Figure(inverted)
        );
        assert_eq!(
            refused(earth_builder().surface(SurfaceState::GasEnvelope, SurfaceMaterial::Rock)),
            BuildCoarseInputsError::NoSolidSurface
        );
        assert!(matches!(
            refused(earth_builder().tectonics(TectonicRegime::MobileLid, 1.2)),
            BuildCoarseInputsError::OutOfRange {
                part: "continental fraction",
                ..
            }
        ));
        let mut spin = *reference::earth_like().spin();
        spin.obliquity = Radians::new(4.0);
        assert!(matches!(
            refused(earth_builder().spin(spin)),
            BuildCoarseInputsError::OutOfRange {
                part: "obliquity",
                ..
            }
        ));
        assert!(matches!(
            refused(earth_builder().seasonal_orbit(SeasonalOrbit {
                eccentricity: 1.0,
                period: Seconds::new(1.0)
            })),
            BuildCoarseInputsError::OutOfRange {
                part: "seasonal eccentricity",
                ..
            }
        ));
        let late = WetEpoch {
            liquid: Gas::Water.substance(),
            start: Gigayears::new(1.0),
            end: Gigayears::new(2.0),
            effective_flow: Years::new(1.0),
            runoff_m_per_year: 0.1,
            paleo_inventory: KilogramsPerSquareMetre::new(0.0),
        };
        assert_eq!(
            refused(earth_builder().wet_epoch(late)),
            BuildCoarseInputsError::WetEpochOrder {
                start: late.start,
                end: late.end
            }
        );
        let mars = reference::mars_like();
        assert!(matches!(
            refused(earth_builder().craters(*mars.craters())),
            BuildCoarseInputsError::CraterGravity { .. }
        ));
        // A surface older than the contract's system, which the chronology cannot count, before
        // the coarse pass's header would refuse it.
        let system = reference::earth_like().craters().system_age();
        assert_eq!(
            refused(earth_builder().surface_age(Gigayears::new(5.0))),
            BuildCoarseInputsError::SurfaceOlderThanSystem {
                surface: Gigayears::new(5.0),
                system
            }
        );
        // The record's Γ is finite and positive: a zero, a negative and a NaN are refused.
        for bad in [0.0, -1.0, f64::NAN] {
            match refused(earth_builder().ground_inertia_j_per_m2_k_sqrt_s(bad)) {
                BuildCoarseInputsError::OutOfRange {
                    part: "ground inertia",
                    value,
                } => assert!(value.total_cmp(&bad).is_eq(), "{value} for {bad}"),
                error => panic!("{bad}: {error:?}"),
            }
        }
    }

    /// The provinces' surface age is required and lies from 0 to the surface age
    /// (decision-r09-t14-mars-ceiling §2.1): the builder refuses none, a NaN, a negative age and
    /// one above the surface age, and takes 0 and the surface age itself; the reference Mars's is
    /// 0, the reference Earth's, Moon's and Ceres's their surface age.
    #[test]
    fn the_province_surface_age_lies_from_zero_to_the_surface_age() {
        let earth = reference::earth_like();
        let surface = earth.surface_age();
        let mut missing = earth_builder();
        missing.province_surface_age = None;
        assert_eq!(
            missing.build().unwrap_err(),
            BuildCoarseInputsError::Missing("province surface age")
        );
        for bad in [f64::NAN, -1e-9, surface.value() + 1e-9, f64::INFINITY] {
            let error = earth_builder()
                .province_surface_age(Gigayears::new(bad))
                .build()
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    BuildCoarseInputsError::ProvinceSurfaceAge { surface: s, .. } if s == surface
                ),
                "{bad}: {error:?}"
            );
            assert!(error.to_string().contains("province surface"));
        }
        for good in [0.0, 0.5 * surface.value(), surface.value()] {
            let inputs = earth_builder()
                .province_surface_age(Gigayears::new(good))
                .build()
                .unwrap();
            assert_same_bits(inputs.province_surface_age().value(), good);
        }
        assert_same_bits(reference::mars_like().province_surface_age().value(), 0.0);
        for world in [earth, reference::moon_like(), reference::ceres_like()] {
            assert_same_bits(
                world.province_surface_age().value(),
                world.surface_age().value(),
            );
        }
    }

    /// The builder refuses a wet epoch whose runoff is negative, or whose liquid has no liquid
    /// density in the registry (decision-r09-t14-choices Q6a).
    #[test]
    fn the_builder_refuses_a_wet_epoch_s_bad_runoff_or_liquid() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        let epoch = WetEpoch {
            liquid: Gas::Water.substance(),
            start: Gigayears::new(2.0),
            end: Gigayears::new(1.0),
            effective_flow: Years::new(1.0),
            runoff_m_per_year: 0.1,
            paleo_inventory: KilogramsPerSquareMetre::new(0.0),
        };
        assert!(earth_builder().wet_epoch(epoch).build().is_ok());
        assert!(matches!(
            refused(earth_builder().wet_epoch(WetEpoch {
                runoff_m_per_year: -0.1,
                ..epoch
            })),
            BuildCoarseInputsError::OutOfRange {
                part: "wet epoch's runoff",
                ..
            }
        ));
        let basalt = by_key("basalt").expect("the registry holds basalt");
        assert!(
            basalt.substance().phase_diagram().is_none(),
            "basalt has no phase diagram"
        );
        assert_eq!(
            refused(earth_builder().wet_epoch(WetEpoch {
                liquid: basalt,
                ..epoch
            })),
            BuildCoarseInputsError::WetEpochLiquid(basalt)
        );
    }

    #[test]
    fn the_builder_refuses_bad_substances_and_shares() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        let share = |k, area_fraction| AreaShare {
            substance: key(k),
            area_fraction,
        };
        assert_eq!(
            refused(earth_builder().ices([share("H2O", 0.1), share("H2O", 0.2)])),
            BuildCoarseInputsError::Duplicate {
                part: "ices",
                substance: key("H2O")
            }
        );
        assert!(matches!(
            refused(earth_builder().liquids([share("H2O", 0.7), share("CH4", 0.4)])),
            BuildCoarseInputsError::SharesExceedWhole {
                part: "liquids",
                ..
            }
        ));
        let water = |phase| Condensate {
            substance: key("H2O"),
            phase,
            reservoir: KilogramsPerSquareMetre::new(1.0),
            area_fraction: 0.1,
            persistence: Persistence::Perennial,
        };
        assert_eq!(
            refused(
                earth_builder()
                    .condensates([water(CondensatePhase::Solid), water(CondensatePhase::Solid)])
            ),
            BuildCoarseInputsError::Duplicate {
                part: "condensates",
                substance: key("H2O")
            }
        );
        let nine =
            ["H2O", "CO2", "N2", "CH4", "NH3", "Ar", "CO", "SO2", "H2S"].map(|k| Condensate {
                substance: key(k),
                ..water(CondensatePhase::Solid)
            });
        assert_eq!(
            refused(earth_builder().condensates(nine)),
            BuildCoarseInputsError::TooManyCondensates(9)
        );
    }

    #[test]
    fn the_builder_refuses_signed_zeros_angles_and_gases_out_of_place() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        let year = Seconds::new(3e7);
        assert!(matches!(
            refused(earth_builder().seasonal_orbit(SeasonalOrbit {
                eccentricity: -0.0,
                period: year
            })),
            BuildCoarseInputsError::OutOfRange {
                part: "seasonal eccentricity",
                ..
            }
        ));
        let mut spin = *reference::earth_like().spin();
        spin.equinox_true_anomaly = Radians::new(core::f64::consts::TAU);
        assert!(matches!(
            refused(earth_builder().spin(spin)),
            BuildCoarseInputsError::OutOfRange {
                part: "equinox true anomaly",
                ..
            }
        ));
        let gas = |k, mole_fraction| GasShare {
            substance: key(k),
            mole_fraction,
        };
        assert_eq!(
            refused(earth_builder().gases([gas("N2", 0.0)])),
            BuildCoarseInputsError::OutOfRange {
                part: "mole fraction",
                value: 0.0
            }
        );
        assert!(matches!(
            refused(earth_builder().gases([gas("N2", 0.8), gas("O2", 0.3)])),
            BuildCoarseInputsError::SharesExceedWhole { part: "gases", .. }
        ));
    }

    #[test]
    fn the_builder_refuses_a_melt_share_outside_0_to_1() {
        let mut crust = *reference::earth_like().crust();
        crust.melt_area_fraction = -0.1;
        assert_eq!(
            earth_builder().crust(crust).build(),
            Err(BuildCoarseInputsError::OutOfRange {
                part: "melt area fraction",
                value: -0.1
            })
        );
        crust.melt_area_fraction = 1.0;
        let molten = earth_builder().crust(crust).build().unwrap();
        hyperion_testkit::float::assert_same_bits(molten.crust().melt_area_fraction, 1.0);
    }

    /// Every ice, liquid and condensate on the ground, and every lithology of the crust, has its
    /// palette entry; a supercritical condensate, on the ground as neither, needs none.
    #[test]
    fn the_palette_names_every_substance_on_the_ground_and_every_lithology() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        let not_in = |part, k, role| BuildCoarseInputsError::NotInPalette {
            part,
            substance: key(k),
            role,
        };
        let share = |k, area_fraction| AreaShare {
            substance: key(k),
            area_fraction,
        };
        assert_eq!(
            refused(earth_builder().ices([share("CO2", 0.01)])),
            not_in("ices", "CO2", PaletteRole::Ice)
        );
        assert_eq!(
            refused(earth_builder().liquids([share("CH4", 0.1)])),
            not_in("liquids", "CH4", PaletteRole::Liquid)
        );
        let condensate = |k, phase| Condensate {
            substance: key(k),
            phase,
            reservoir: KilogramsPerSquareMetre::new(1.0),
            area_fraction: 0.0,
            persistence: Persistence::Perennial,
        };
        assert_eq!(
            refused(earth_builder().condensates([condensate("N2", CondensatePhase::Solid)])),
            not_in("condensates", "N2", PaletteRole::Ice)
        );
        assert_eq!(
            refused(earth_builder().condensates([condensate("NH3", CondensatePhase::Liquid)])),
            not_in("condensates", "NH3", PaletteRole::Liquid)
        );
        let supercritical = earth_builder()
            .condensates([condensate("CO2", CondensatePhase::Supercritical)])
            .build()
            .unwrap();
        assert_eq!(supercritical.condensates().len(), 1);
        let mut crust = *reference::earth_like().crust();
        crust.primary = Some(key("anorthosite"));
        assert_eq!(
            refused(earth_builder().crust(crust)),
            not_in("primary crust", "anorthosite", PaletteRole::PrimaryCrust)
        );
        // The provinces are not the secondary crust's basalt, so they need an entry of their own.
        let mut crust = *reference::earth_like().crust();
        crust.provinces = key("komatiite");
        assert_eq!(
            refused(earth_builder().crust(crust)),
            not_in("provinces", "komatiite", PaletteRole::Province)
        );
        // An ice of the right key but the wrong role is not the ice's entry.
        let earth = reference::earth_like();
        let without_ice: Vec<PaletteEntry> = earth
            .palette()
            .entries()
            .iter()
            .filter(|e| e.role != PaletteRole::Ice)
            .cloned()
            .collect();
        assert_eq!(
            refused(earth_builder().palette(palette(without_ice))),
            not_in("ices", "H2O", PaletteRole::Ice)
        );
        let without_granite: Vec<PaletteEntry> = earth
            .palette()
            .entries()
            .iter()
            .filter(|e| e.role != PaletteRole::TertiaryCrust)
            .cloned()
            .collect();
        assert_eq!(
            refused(earth_builder().palette(palette(without_granite))),
            not_in("tertiary crust", "granite", PaletteRole::TertiaryCrust)
        );
        // With no basalt entry in either role, the secondary crust has none.
        let without_basalt: Vec<PaletteEntry> = earth
            .palette()
            .entries()
            .iter()
            .filter(|e| e.substance != key("basalt"))
            .cloned()
            .collect();
        assert_eq!(
            refused(earth_builder().palette(palette(without_basalt))),
            not_in("secondary crust", "basalt", PaletteRole::SecondaryCrust)
        );
    }

    /// Each crust variant names its lithology's entry, the variant's own role first where the
    /// secondary crust and the provinces are one substance, and the main liquid is the largest.
    #[test]
    fn the_crusts_name_their_lithologies_and_the_main_liquid_is_the_largest() {
        let at = |inputs: &CoarseInputs, k, role| inputs.palette().find(role, key(k));
        let earth = reference::earth_like();
        let basalt = at(&earth, "basalt", PaletteRole::SecondaryCrust);
        assert_eq!(
            earth.crust_palette(),
            [
                at(&earth, "granite", PaletteRole::TertiaryCrust),
                basalt,
                basalt,
                basalt
            ]
        );
        assert!(basalt.is_some());
        assert_eq!(earth.main_liquid(), at(&earth, "H2O", PaletteRole::Liquid));
        assert!(earth.main_liquid().is_some());

        let moon = reference::moon_like();
        let maria = at(&moon, "basalt_fe", PaletteRole::Province);
        let highlands = at(&moon, "feldspathic", PaletteRole::PrimaryCrust);
        assert_eq!(moon.crust_palette(), [None, maria, highlands, maria]);
        assert!(maria.is_some() && highlands.is_some());
        assert_eq!(moon.main_liquid(), None);

        let mars = reference::mars_like();
        let lid = at(&mars, "basalt_fe", PaletteRole::SecondaryCrust);
        assert_eq!(mars.crust_palette(), [None, lid, lid, lid]);
        assert!(lid.is_some());
        assert_eq!(mars.main_liquid(), None);

        // Ceres has no secondary crust, so no oceanic crust's entry, and its lid and its
        // provinces, were it to build any, take its primary phyllosilicate's.
        let ceres = reference::ceres_like();
        let shell = at(&ceres, "phyllosilicate", PaletteRole::PrimaryCrust);
        assert_eq!(ceres.crust_palette(), [None, None, shell, shell]);
        assert!(shell.is_some());
        assert_eq!(ceres.main_liquid(), None);

        // The lid's entry, resolved once, is the `Lid` variant's.
        for world in [&earth, &moon, &mars, &ceres] {
            assert_eq!(Some(world.lid_entry()), world.crust_palette()[2]);
        }

        // Of two seas, the larger's liquid is the main one, whichever order they were given in.
        let mut entries = earth.palette().entries().to_vec();
        entries.push(entry("CH4", PaletteRole::Liquid));
        let two_seas = earth_builder()
            .liquids([share_of("H2O", 0.2), share_of("CH4", 0.5)])
            .palette(palette(entries))
            .build()
            .unwrap();
        assert_eq!(
            two_seas.main_liquid(),
            at(&two_seas, "CH4", PaletteRole::Liquid)
        );
        // Of two equal seas, the key that sorts first, given in either order.
        let tied = |liquids: [AreaShare; 2]| {
            earth_builder()
                .liquids(liquids)
                .palette(two_seas.palette().clone())
                .build()
                .unwrap()
                .main_liquid()
        };
        let methane = at(&two_seas, "CH4", PaletteRole::Liquid);
        assert_eq!(tied([share_of("H2O", 0.3), share_of("CH4", 0.3)]), methane);
        assert_eq!(tied([share_of("CH4", 0.3), share_of("H2O", 0.3)]), methane);
    }

    /// A crust needs a primary or a secondary crust, or an undifferentiated body's bulk material,
    /// for its lid, and may lack either crust: Ceres has no secondary, and Earth no primary. A
    /// tertiary crust alone is none, and the refusal comes before the lid's.
    #[test]
    fn a_crust_with_neither_crust_nor_bulk_material_is_refused() {
        let mut crust = *reference::earth_like().crust();
        assert_eq!((crust.primary, crust.bulk), (None, None));
        assert!(crust.tertiary.is_some());
        crust.secondary = None;
        assert_eq!(
            earth_builder().crust(crust).build(),
            Err(BuildCoarseInputsError::NoCrust)
        );
        let ceres = reference::ceres_like();
        assert_eq!((ceres.crust().secondary, ceres.crust().bulk), (None, None));
        let mut crust = *ceres.crust();
        crust.primary = None;
        assert_eq!(
            builder_of(&ceres).crust(crust).build(),
            Err(BuildCoarseInputsError::NoCrust)
        );
    }

    /// An undifferentiated chondritic body: Ceres's figures with an ordinary chondrite as its bulk
    /// material, at its surface, and no crust; its palette Ceres's with the chondrite's `Bulk`
    /// entry in place of the phyllosilicate's.
    fn chondritic() -> CoarseInputsBuilder {
        let ceres = reference::ceres_like();
        let chondrite = key("chondrite_h");
        let entries = ceres
            .palette()
            .entries()
            .iter()
            .filter(|e| e.role != PaletteRole::PrimaryCrust)
            .cloned()
            .chain([entry("chondrite_h", PaletteRole::Bulk)])
            .collect();
        builder_of(&ceres)
            .crust(CrustInputs {
                primary: None,
                secondary: None,
                tertiary: None,
                bulk: Some(chondrite),
                lid: LidCrust::Bulk,
                provinces: chondrite,
                melt_area_fraction: 0.0,
            })
            .palette(palette(entries))
    }

    /// An undifferentiated body's bulk material stands alone: beside a primary, a secondary or a
    /// tertiary crust it is refused, before its lid is looked at (science-composition-crusts.md
    /// §1).
    #[test]
    fn a_bulk_material_beside_a_crust_is_refused() {
        let alone = chondritic().build().unwrap();
        let bulk = *alone.crust();
        let beside = [
            CrustInputs {
                primary: Some(key("phyllosilicate")),
                ..bulk
            },
            CrustInputs {
                secondary: Some(key("basalt")),
                ..bulk
            },
            CrustInputs {
                tertiary: Some(key("granite")),
                ..bulk
            },
            // A lid naming the crust beside it is refused for the bulk material all the same.
            CrustInputs {
                secondary: Some(key("basalt")),
                lid: LidCrust::Secondary,
                ..bulk
            },
            // A lid naming a crust the body does not have is refused for the bulk material beside
            // a crust first, the lid's refusal coming after.
            CrustInputs {
                tertiary: Some(key("granite")),
                lid: LidCrust::Primary,
                ..bulk
            },
        ];
        for crust in beside {
            assert_eq!(
                chondritic().crust(crust).build(),
                Err(BuildCoarseInputsError::BulkBesideCrust),
                "{crust:?}"
            );
        }
    }

    /// The lid names a crust or bulk material the body has (science-composition-crusts.md §5):
    /// Earth has no primary crust and no bulk material, Ceres no secondary crust, and a chondritic
    /// body no crust; and a lid that names one is refused before a melt share out of range.
    #[test]
    fn a_lid_naming_an_absent_crust_is_refused() {
        let ceres = reference::ceres_like();
        let cases = [
            (
                earth_builder(),
                *reference::earth_like().crust(),
                LidCrust::Primary,
            ),
            (
                earth_builder(),
                *reference::earth_like().crust(),
                LidCrust::Bulk,
            ),
            (builder_of(&ceres), *ceres.crust(), LidCrust::Secondary),
            (
                chondritic(),
                *chondritic().build().unwrap().crust(),
                LidCrust::Primary,
            ),
        ];
        for (builder, crust, lid) in cases {
            let absent = CrustInputs {
                lid,
                melt_area_fraction: -0.1,
                ..crust
            };
            assert_eq!(
                builder.crust(absent).build(),
                Err(BuildCoarseInputsError::LidAbsent(lid)),
                "{absent:?}"
            );
        }
    }

    /// An undifferentiated body's lid and provinces take its bulk material's `Bulk` entry, and its
    /// oceanic and continental crusts none, having no crust; the bulk material's entry is its
    /// `Bulk` one alone (science-composition-crusts.md §1).
    #[test]
    fn a_chondritic_body_s_lid_and_provinces_take_its_bulk_entry() {
        let body = chondritic().build().unwrap();
        let bulk = body.palette().find(PaletteRole::Bulk, key("chondrite_h"));
        assert!(bulk.is_some());
        assert_eq!(body.crust_palette(), [None, None, bulk, bulk]);
        assert_eq!(Some(body.lid_entry()), bulk);
        // The pass builds its field, whose header takes a `Bulk` entry for the lid.
        let field = super::super::coarse_pass(reference::SEED, &body);
        assert_eq!(field.header().crust_palette(), body.crust_palette());
        // A chondrite's entry in a crust's role does not stand for its bulk material's.
        let as_a_crust: Vec<PaletteEntry> = body
            .palette()
            .entries()
            .iter()
            .filter(|e| e.role != PaletteRole::Bulk)
            .cloned()
            .chain([entry("chondrite_h", PaletteRole::PrimaryCrust)])
            .collect();
        assert_eq!(
            chondritic().palette(palette(as_a_crust)).build(),
            Err(BuildCoarseInputsError::NotInPalette {
                part: "bulk material",
                substance: key("chondrite_h"),
                role: PaletteRole::Bulk
            })
        );
    }

    /// A Mars given a buried primary crust under its secondary lid takes the secondary's entry for
    /// its lid, as science-composition-crusts.md §5 rules, and the primary keeps an entry of its
    /// own, which large craters excavate; naming the primary as the lid instead takes that.
    #[test]
    fn a_lid_takes_the_crust_it_names_over_a_buried_primary() {
        let mars = reference::mars_like();
        let at = |inputs: &CoarseInputs, k, role| inputs.palette().find(role, key(k));
        let mut crust = *mars.crust();
        assert_eq!(crust.lid, LidCrust::Secondary);
        crust.primary = Some(key("ultramafic"));
        let mut entries = mars.palette().entries().to_vec();
        entries.push(entry("ultramafic", PaletteRole::PrimaryCrust));
        let with_primary = palette(entries);
        let buried = builder_of(&mars)
            .crust(crust)
            .palette(with_primary.clone())
            .build()
            .unwrap();
        let basalt = at(&buried, "basalt_fe", PaletteRole::SecondaryCrust);
        assert!(basalt.is_some());
        assert_eq!(buried.crust_palette(), [None, basalt, basalt, basalt]);
        assert_eq!(Some(buried.lid_entry()), basalt);
        // The buried primary crust needs its entry all the same.
        assert_eq!(
            builder_of(&mars).crust(crust).build(),
            Err(BuildCoarseInputsError::NotInPalette {
                part: "primary crust",
                substance: key("ultramafic"),
                role: PaletteRole::PrimaryCrust
            })
        );
        let exposed = builder_of(&mars)
            .crust(CrustInputs {
                lid: LidCrust::Primary,
                ..crust
            })
            .palette(with_primary)
            .build()
            .unwrap();
        let ultramafic = at(&exposed, "ultramafic", PaletteRole::PrimaryCrust);
        assert!(ultramafic.is_some());
        assert_eq!(exposed.crust_palette(), [None, basalt, ultramafic, basalt]);
        assert_eq!(Some(exposed.lid_entry()), ultramafic);
    }

    /// The provinces take their own `Province` entry first, then the entry of a crust of their
    /// substance, the secondary's before the primary's; the primary crust takes its
    /// `PrimaryCrust` entry alone, and the provinces only a crust's.
    #[test]
    fn the_provinces_share_a_crust_s_entry_and_the_primary_its_own() {
        let ceres = reference::ceres_like();
        let at = |inputs: &CoarseInputs, k, role| inputs.palette().find(role, key(k));
        let with = |extra: PaletteEntry| {
            let mut entries = ceres.palette().entries().to_vec();
            entries.push(extra);
            palette(entries)
        };
        // A province entry of the lid's substance is the provinces' own, and the lid keeps its.
        let own = builder_of(&ceres)
            .palette(with(entry("phyllosilicate", PaletteRole::Province)))
            .build()
            .unwrap();
        let shell = at(&own, "phyllosilicate", PaletteRole::PrimaryCrust);
        let provinces = at(&own, "phyllosilicate", PaletteRole::Province);
        assert_eq!(own.crust_palette(), [None, None, shell, provinces]);
        assert_ne!(shell, provinces);
        // Where both crusts are the provinces' substance, the secondary's entry names them.
        let mut both = *ceres.crust();
        both.secondary = Some(key("phyllosilicate"));
        let both = builder_of(&ceres)
            .crust(both)
            .palette(with(entry("phyllosilicate", PaletteRole::SecondaryCrust)))
            .build()
            .unwrap();
        let shell = at(&both, "phyllosilicate", PaletteRole::PrimaryCrust);
        let melt = at(&both, "phyllosilicate", PaletteRole::SecondaryCrust);
        assert_eq!(both.crust_palette(), [None, melt, shell, melt]);
        // The primary crust does not take a province's entry of its substance.
        let provinces_only: Vec<PaletteEntry> = ceres
            .palette()
            .entries()
            .iter()
            .filter(|e| e.role != PaletteRole::PrimaryCrust)
            .cloned()
            .chain([entry("phyllosilicate", PaletteRole::Province)])
            .collect();
        let refused = builder_of(&ceres)
            .palette(palette(provinces_only))
            .build()
            .unwrap_err();
        assert_eq!(
            refused,
            BuildCoarseInputsError::NotInPalette {
                part: "primary crust",
                substance: key("phyllosilicate"),
                role: PaletteRole::PrimaryCrust
            }
        );
        // A palette whose `Na2CO3` is a `Deposit` entry only has no `Province` entry for it: a
        // deposit's entry never names the provinces.
        let mut salt = *ceres.crust();
        salt.provinces = key("Na2CO3");
        assert_eq!(
            builder_of(&ceres).crust(salt).build().unwrap_err(),
            BuildCoarseInputsError::NotInPalette {
                part: "provinces",
                substance: key("Na2CO3"),
                role: PaletteRole::Province
            }
        );
    }

    /// A −0 share or reservoir is refused, so that a list sorts by key, not by a zero's sign.
    #[test]
    fn the_builder_refuses_a_signed_zero_share_or_reservoir() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        assert_eq!(
            refused(earth_builder().ices([share_of("H2O", -0.0)])),
            BuildCoarseInputsError::OutOfRange {
                part: "ices",
                value: -0.0
            }
        );
        let mut ocean = reference::earth_like().condensates()[0];
        ocean.reservoir = KilogramsPerSquareMetre::new(-0.0);
        assert!(matches!(
            refused(earth_builder().condensates([ocean])),
            BuildCoarseInputsError::OutOfRange {
                part: "condensate's reservoir",
                ..
            }
        ));
        ocean.reservoir = KilogramsPerSquareMetre::new(0.0);
        ocean.area_fraction = -0.0;
        assert!(matches!(
            refused(earth_builder().condensates([ocean])),
            BuildCoarseInputsError::OutOfRange {
                part: "condensate's area fraction",
                ..
            }
        ));
    }

    /// The share of `k` over `area_fraction` of the surface.
    fn share_of(k: &str, area_fraction: f64) -> AreaShare {
        AreaShare {
            substance: key(k),
            area_fraction,
        }
    }

    #[test]
    fn errors_say_what_was_refused() {
        assert_eq!(
            BuildCoarseInputsError::NotInPalette {
                part: "ices",
                substance: key("CO2"),
                role: PaletteRole::Ice
            }
            .to_string(),
            "the palette has no Ice entry for CO2, named by the ices"
        );
        assert_eq!(
            BuildCoarseInputsError::Missing("crust").to_string(),
            "the coarse inputs lack the crust"
        );
        assert_eq!(
            BuildCoarseInputsError::NoCrust.to_string(),
            "the crust has neither a primary nor a secondary crust nor a bulk material"
        );
        assert_eq!(
            BuildCoarseInputsError::BulkBesideCrust.to_string(),
            "the crust names a bulk material beside a crust"
        );
        assert_eq!(
            BuildCoarseInputsError::LidAbsent(LidCrust::Primary).to_string(),
            "the lid names a crust or bulk material the body does not have"
        );
        assert_eq!(
            BuildCoarseInputsError::Duplicate {
                part: "ices",
                substance: key("H2O")
            }
            .to_string(),
            "the ices name H2O twice"
        );
        assert_eq!(
            BuildCoarseInputsError::ColdTrapExceedsIce {
                substance: key("H2O"),
                trapped: 0.5,
                ice: 0.25
            }
            .to_string(),
            "the cold-trapped share 0.5 of H2O exceeds its ice share 0.25"
        );
        assert_eq!(
            BuildCoarseInputsError::ColdTrapWithoutThreshold(key("He")).to_string(),
            "He has no cold-trap threshold"
        );
        assert_eq!(
            BuildCoarseInputsError::Missing("cold-trap inputs").to_string(),
            "the coarse inputs lack the cold-trap inputs"
        );
        assert_eq!(
            SurfaceInputsError::NoSolidSurface.to_string(),
            "the body has no solid surface"
        );
        assert_eq!(
            SurfaceInputsError::NotResolved(RecordSection::Surface).to_string(),
            "the record withholds the body's surface section"
        );
    }

    /// T51.f's lunar cold-trap inputs: the Sun's 0.2711° at Earth's perihelion and the bare
    /// regolith's Bond albedo 0.12.
    fn lunar_cold_trap_inputs() -> ColdTrapInputs {
        ColdTrapInputs {
            star_angular_radius: Radians::from(Degrees::new(0.2711)),
            bond_albedo: 0.12,
        }
    }

    /// The reference Moon with 2 × 10⁻⁵ of water ice on its ground, 1.7 × 10⁻⁵ of it
    /// cold-trapped, under [`lunar_cold_trap_inputs`], and with carbon dioxide's and helium's ice
    /// entries in its palette, as a builder to vary.
    fn trapped_moon() -> CoarseInputsBuilder {
        let moon = reference::moon_like();
        let mut entries = moon.palette().entries().to_vec();
        entries.extend([
            entry("CO2", PaletteRole::Ice),
            entry("He", PaletteRole::Ice),
        ]);
        builder_of(&moon)
            .ices([share_of("H2O", 2e-5)])
            .cold_traps([share_of("H2O", 1.7e-5)])
            .cold_trap_inputs(lunar_cold_trap_inputs())
            .palette(palette(entries))
    }

    /// Each refusal of the cold-trapped shares (decision-r09-t13f-cold-traps §1.3), by its variant:
    /// a share out of range, a substance named twice, a share above its ice's or of a substance
    /// with no ice share, a substance with no cold-trap threshold, and a positive share with no
    /// cold-trap inputs. A zero share needs no inputs.
    #[test]
    fn the_builder_refuses_cold_traps_out_of_place() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        assert!(trapped_moon().build().is_ok());
        for value in [f64::NAN, -0.1, 1.5, -0.0, f64::INFINITY] {
            let error = refused(trapped_moon().cold_traps([share_of("H2O", value)]));
            let BuildCoarseInputsError::OutOfRange {
                part: "cold traps",
                value: refused_value,
            } = error
            else {
                panic!("{value}: {error:?}");
            };
            if value.is_nan() {
                assert!(refused_value.is_nan());
            } else {
                assert_same_bits(refused_value, value);
            }
        }
        assert_eq!(
            refused(trapped_moon().cold_traps([share_of("H2O", 1e-6), share_of("H2O", 2e-6)])),
            BuildCoarseInputsError::Duplicate {
                part: "cold traps",
                substance: key("H2O")
            }
        );
        assert_eq!(
            refused(trapped_moon().cold_traps([share_of("H2O", 3e-5)])),
            BuildCoarseInputsError::ColdTrapExceedsIce {
                substance: key("H2O"),
                trapped: 3e-5,
                ice: 2e-5
            }
        );
        // A substance with no ice share at all, at a positive share or a zero one.
        for trapped in [1e-6, 0.0] {
            assert_eq!(
                refused(trapped_moon().cold_traps([share_of("CO2", trapped)])),
                BuildCoarseInputsError::ColdTrapExceedsIce {
                    substance: key("CO2"),
                    trapped,
                    ice: 0.0
                }
            );
        }
        // Helium's row has no phase diagram, so no cold-trap threshold.
        assert_eq!(
            refused(
                trapped_moon()
                    .ices([share_of("H2O", 2e-5), share_of("He", 1e-5)])
                    .cold_traps([share_of("He", 1e-6)])
            ),
            BuildCoarseInputsError::ColdTrapWithoutThreshold(key("He"))
        );
        let without = |traps: Vec<AreaShare>| {
            let mut builder = trapped_moon().cold_traps(traps);
            builder.cold_trap_inputs = None;
            builder
        };
        assert_eq!(
            refused(without(vec![share_of("H2O", 1.7e-5)])),
            BuildCoarseInputsError::Missing("cold-trap inputs")
        );
        let untrapped = without(vec![share_of("H2O", 0.0)]).build().unwrap();
        assert_eq!(untrapped.cold_traps(), [share_of("H2O", 0.0)].as_slice());
        assert_eq!(untrapped.cold_trap_inputs(), None);
    }

    /// The cold-trap inputs' refusals (decision-r09-t13f-cold-traps §2.2), by their variant: a
    /// star's angular radius outside [0, π/2), or with a square that is not a normal `f64`, and a
    /// Bond albedo outside [0, 1). A point star is in range, and inputs with no traps are harmless,
    /// but checked.
    #[test]
    fn the_builder_refuses_cold_trap_inputs_out_of_range() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        let inputs = |star_angular_radius: f64, bond_albedo: f64| ColdTrapInputs {
            star_angular_radius: Radians::new(star_angular_radius),
            bond_albedo,
        };
        for alpha in [-0.1, core::f64::consts::FRAC_PI_2, f64::NAN, 1e-200] {
            let error = refused(trapped_moon().cold_trap_inputs(inputs(alpha, 0.12)));
            assert!(
                matches!(
                    error,
                    BuildCoarseInputsError::OutOfRange {
                        part: "star angular radius",
                        ..
                    }
                ),
                "{alpha}: {error:?}"
            );
        }
        for albedo in [-0.1, 1.0, f64::NAN] {
            let error = refused(trapped_moon().cold_trap_inputs(inputs(0.0047, albedo)));
            assert!(
                matches!(
                    error,
                    BuildCoarseInputsError::OutOfRange {
                        part: "cold-trap Bond albedo",
                        ..
                    }
                ),
                "{albedo}: {error:?}"
            );
        }
        assert!(
            trapped_moon()
                .cold_trap_inputs(inputs(0.0, 0.0))
                .build()
                .is_ok()
        );
        let alone = trapped_moon().cold_traps([]).build().unwrap();
        assert_eq!(alone.cold_traps(), []);
        assert_eq!(alone.cold_trap_inputs(), Some(&lunar_cold_trap_inputs()));
        assert!(matches!(
            refused(
                trapped_moon()
                    .cold_traps([])
                    .cold_trap_inputs(inputs(2.0, 0.12))
            ),
            BuildCoarseInputsError::OutOfRange {
                part: "star angular radius",
                ..
            }
        ));
    }

    /// Valid cold traps and their inputs come back through their getters, the traps largest first,
    /// ties by key, whichever order they were given in, each ice keeping its whole share; and
    /// through the builder of the inputs built (`reference::builder_of`).
    #[test]
    fn cold_traps_and_their_inputs_round_trip_through_their_getters() {
        let inputs = trapped_moon()
            .ices([share_of("CO2", 1e-6), share_of("H2O", 2e-5)])
            .cold_traps([share_of("CO2", 1e-6), share_of("H2O", 1.7e-5)])
            .build()
            .unwrap();
        assert_eq!(
            inputs.cold_traps(),
            [share_of("H2O", 1.7e-5), share_of("CO2", 1e-6)].as_slice()
        );
        assert_eq!(
            inputs.ices(),
            [share_of("H2O", 2e-5), share_of("CO2", 1e-6)].as_slice()
        );
        assert_eq!(inputs.cold_trap_inputs(), Some(&lunar_cold_trap_inputs()));
        assert_eq!(builder_of(&inputs).build().unwrap(), inputs);
        let tied = trapped_moon()
            .ices([share_of("H2O", 2e-5), share_of("CO2", 2e-5)])
            .cold_traps([share_of("H2O", 1e-6), share_of("CO2", 1e-6)])
            .build()
            .unwrap();
        let keys: Vec<&str> = tied
            .cold_traps()
            .iter()
            .map(|s| s.substance.as_str())
            .collect();
        assert_eq!(keys, ["CO2", "H2O"]);
        for world in [
            reference::earth_like(),
            reference::mars_like(),
            reference::moon_like(),
            reference::ceres_like(),
        ] {
            assert_eq!(world.cold_traps(), []);
            assert_eq!(world.cold_trap_inputs(), None);
        }
    }

    /// The record gives no cold traps before the 23 → 24 bump: a generated planet's and a
    /// generated moon's inputs have none, and no cold-trap inputs (decision-r09-t13f-cold-traps
    /// §7.2). It fails at 24, so that P14.T54.b wires them.
    #[test]
    fn the_record_gives_no_cold_traps_until_the_24_bump() {
        assert!(
            GENERATOR_VERSION < COLD_TRAPS_VERSION,
            "the 23 → 24 bump (P14.T54.b) puts P14.T51.f's cold traps on the record: fill \
             `from_record`'s `cold_traps` from each solid entry's \
             `CondensateEntry::trapped_area_fraction`, and its `cold_trap_inputs` from the star's \
             angular radius and the bare regolith's Bond albedo the record's cold traps read, to \
             the bit, test that a generated qualifying body's field places its record's trapped \
             areas (placed plus undrawn to one cell, short 0), and delete this test \
             (decision-r09-t13f-cold-traps §6 step 4 and §8)"
        );
        let (_, _, planet) = planet_reading();
        let (_, moon, its_planet) = generated_moon();
        let moon = read(&moon, SeasonsFromRecord::Planet(&its_planet));
        for inputs in [planet, moon] {
            assert!(inputs.cold_traps().is_empty(), "{:?}", inputs.cold_traps());
            assert_eq!(inputs.cold_trap_inputs(), None);
        }
    }

    #[test]
    fn lists_are_sorted_largest_first_ties_by_key() {
        let gas = |k, mole_fraction| GasShare {
            substance: key(k),
            mole_fraction,
        };
        let inputs = earth_builder()
            .gases([
                gas("Ne", 0.01),
                gas("O2", 0.2),
                gas("N2", 0.78),
                gas("Ar", 0.01),
            ])
            .build()
            .unwrap();
        let keys: Vec<&str> = inputs
            .gases()
            .iter()
            .map(|g| g.substance.as_str())
            .collect();
        assert_eq!(keys, ["N2", "O2", "Ar", "Ne"]);
        let mars = reference::mars_like();
        let keys: Vec<&str> = mars.ices().iter().map(|s| s.substance.as_str()).collect();
        assert_eq!(keys, ["H2O", "CO2"]);
    }

    #[test]
    fn a_year_has_one_month_only_without_seasonal_forcing() {
        let year = Seconds::new(3e7);
        let orbit = |eccentricity| SeasonalOrbit {
            eccentricity,
            period: year,
        };
        let spin = |obliquity: f64, solar_day: Option<Seconds>| Spin {
            obliquity: Radians::new(obliquity),
            rotation_period: Seconds::new(86_400.0),
            solar_day,
            equinox_true_anomaly: Radians::ZERO,
        };
        let day = Some(Seconds::new(86_400.0));
        let months = |builder: CoarseInputsBuilder| builder.build().unwrap().months();
        assert_eq!(months(earth_builder()), 12);
        // Locked 1:1 to its host on a circular orbit: seasons from its tilt alone, since the
        // lock adds nothing (Design note 8, signed off 2026-10-10), and none without one.
        assert_eq!(
            months(
                earth_builder()
                    .seasonal_orbit(orbit(0.0))
                    .spin(spin(0.3, None))
            ),
            12
        );
        assert_eq!(
            months(
                earth_builder()
                    .seasonal_orbit(orbit(0.0))
                    .spin(spin(0.0, None))
            ),
            1
        );
        // Locked on an eccentric orbit keeps twelve, which carry the libration and the distance.
        assert_eq!(
            months(
                earth_builder()
                    .seasonal_orbit(orbit(0.1))
                    .spin(spin(0.0, None))
            ),
            12
        );
        // A circular orbit with no tilt, prograde or retrograde, has no seasons.
        assert_eq!(
            months(
                earth_builder()
                    .seasonal_orbit(orbit(0.0))
                    .spin(spin(0.0, day))
            ),
            1
        );
        let retrograde = spin(core::f64::consts::PI, day);
        assert_eq!(
            months(earth_builder().seasonal_orbit(orbit(0.0)).spin(retrograde)),
            1
        );
        assert_eq!(
            months(
                earth_builder()
                    .seasonal_orbit(orbit(0.0))
                    .spin(spin(0.3, day))
            ),
            12
        );
        // A body that orbits no star has no seasons.
        let mut free = earth_builder();
        free.seasonal_orbit = None;
        assert_eq!(months(free), 1);
    }

    /// The universe the record tests generate in.
    const SEED: Seed = Seed::new(0x5eed_0009_0010);

    /// A Sun-like system's context and a record at the epoch of a rocky planet of it, and the same
    /// of a gas giant and of a sub-Neptune, from the first of Sun-like systems 1, 2, … that hold
    /// each.
    fn rocky_giant_and_sub_neptune() -> [(SystemContext, BodyRecord); 3] {
        let (mut rocky, mut giant, mut sub_neptune) = (None, None, None);
        for index in 1..64 {
            let id = CellKey::new(Layer::C, [0, 812, 0])
                .unwrap()
                .candidate_id(index)
                .unwrap();
            let ctx =
                synthetic_star(id, SolarMasses::new(1.0), Dex::ZERO, Years::new(4.57e9)).unwrap();
            let system = generate(SEED, &ctx);
            for record in system.snapshot_at(&ctx, UniverseTime::EPOCH).bodies() {
                let Some(bulk) = record.bulk().ok() else {
                    continue;
                };
                match bulk.class() {
                    PlanetClass::Rocky if rocky.is_none() => {
                        rocky = Some((ctx.clone(), record.clone()));
                    }
                    PlanetClass::GasGiant if giant.is_none() => {
                        giant = Some((ctx.clone(), record.clone()));
                    }
                    PlanetClass::SubNeptune if sub_neptune.is_none() => {
                        sub_neptune = Some((ctx.clone(), record.clone()));
                    }
                    _ => {}
                }
            }
            if let (Some(r), Some(g), Some(s)) = (&rocky, &giant, &sub_neptune) {
                return [r.clone(), g.clone(), s.clone()];
            }
        }
        panic!("63 Sun-like systems hold no rocky planet, gas giant and sub-Neptune");
    }

    /// P14.T54.a: a generated rocky planet's inputs are `Ok`, its crust, condensates and ground
    /// inertia its record's to the bit, and every other value as `from_record` reads it.
    #[test]
    fn a_generated_rocky_body_reads_its_record_and_a_giant_or_sub_neptune_has_no_solid_surface() {
        let [
            (rocky_ctx, rocky),
            (giant_ctx, giant),
            (sub_ctx, sub_neptune),
        ] = rocky_giant_and_sub_neptune();
        // The rocky planet's section carries its conditions and figures (P14.T48.e), and from
        // P14.T54.a its crust and condensates, which the reading reads.
        let Section::Ok(surface) = rocky.surface() else {
            panic!(
                "a generated rocky planet's surface is {:?}",
                rocky.surface()
            );
        };
        let Modelled::Ok(crust) = surface.crust() else {
            panic!("{:?}", surface.crust());
        };
        let inputs = CoarseInputs::for_body(&rocky, None, &rocky_ctx)
            .unwrap_or_else(|e| panic!("a generated rocky planet reads into inputs: {e:?}"));
        assert_eq!(*inputs.crust(), crust_inputs(crust));
        let key = |id: Option<SubstanceId>| id.map(|id| id.substance().key());
        assert_eq!(inputs.crust().secondary, key(crust.secondary()));
        assert_eq!(inputs.crust().lid, crust.lid());
        assert_same_bits(
            inputs.crust().melt_area_fraction,
            crust.melt_area_fraction(),
        );
        let mut condensates: Vec<Condensate> = condensates_of(surface).collect();
        condensates.sort_by(|a, b| {
            b.reservoir
                .value()
                .total_cmp(&a.reservoir.value())
                .then_with(|| a.substance.cmp(&b.substance))
                .then_with(|| a.phase.cmp(&b.phase))
        });
        assert_eq!(inputs.condensates(), condensates.as_slice());
        assert_eq!(
            inputs
                .ground_inertia_j_per_m2_k_sqrt_s()
                .map(hyperion_testkit::float::bits),
            Some(hyperion_testkit::float::bits(
                surface.conditions().ground().inertia_j_per_m2_k_sqrt_s()
            ))
        );
        // The rest is `from_record`'s reading, with the record's own three.
        let palette = palette::resolve(surface, crust).unwrap();
        let read = CoarseInputs::from_record(
            &rocky,
            SeasonsFromRecord::Own,
            crust_inputs(crust),
            condensates_of(surface),
            palette,
        )
        .unwrap();
        assert_eq!(inputs, read);
        // The pass's regime is the record's, its condensable by key.
        let regime = ClimateRegime::from(surface.climate_regime());
        assert_eq!(regime.model, surface.climate_regime().model());
        assert_eq!(
            regime.condensable,
            surface
                .climate_regime()
                .condensable()
                .map(|id| id.substance().key())
        );
        // A giant and a sub-Neptune have no solid surface, from the tag alone
        // (decision-p14-t35e-wire §2.4).
        assert_eq!(
            CoarseInputs::for_body(&giant, None, &giant_ctx),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NoSolidSurface
            ))
        );
        assert_eq!(sub_neptune.surface(), &Section::NotApplicable);
        assert_eq!(
            CoarseInputs::for_body(&sub_neptune, None, &sub_ctx),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NoSolidSurface
            ))
        );
        // A record degraded below the surface section's level withholds it.
        assert_eq!(
            CoarseInputs::for_body(&rocky.degrade(DetailLevel::Bulk), None, &rocky_ctx),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NotResolved(RecordSection::Surface)
            ))
        );
        assert_eq!(
            SurfaceInputsError::NotModelled(RecordSection::Surface).to_string(),
            "the body's surface section is not modelled"
        );
    }

    // R09.T10.e: the reading of the record's other sections, on generated records with crusts,
    // condensates and palettes of the tests' own, since the record's are not modelled before
    // P14.T54.a.

    /// A hand-made crust: a basalt secondary crust at the surface, its provinces' too.
    fn basalt_crust() -> CrustInputs {
        CrustInputs {
            primary: None,
            secondary: Some(key("basalt")),
            tertiary: None,
            bulk: None,
            lid: LidCrust::Secondary,
            provinces: key("basalt"),
            melt_area_fraction: 0.0,
        }
    }

    /// The palette [`basalt_crust`] needs on `record`'s body, with `also`: the basalt's entry and
    /// an entry of each liquid and ice the record's figures put on the ground, by its row's key,
    /// each once, and none for a share with no area.
    fn palette_for(record: &BodyRecord, also: &[(PaletteRole, &str)]) -> MaterialPalette {
        let Section::Ok(surface) = record.surface() else {
            panic!("{:?} has no surface section", record.index());
        };
        let figures = surface.figures();
        let mut entries =
            std::collections::BTreeSet::from([(PaletteRole::SecondaryCrust, key("basalt"))]);
        entries.extend(also.iter().map(|&(role, substance)| (role, key(substance))));
        for (shares, role) in [
            (figures.surface_liquids(), PaletteRole::Liquid),
            (figures.surface_ices(), PaletteRole::Ice),
        ] {
            entries.extend(on_the_ground(shares).map(|share| (role, share.substance)));
        }
        palette(
            entries
                .into_iter()
                .map(|(role, substance)| PaletteEntry {
                    substance,
                    ..entry("basalt", role)
                })
                .collect(),
        )
    }

    /// The records at [`RECORD_TIME`] of the systems of `contexts`, generated in [`SEED`]'s
    /// universe, each list with its system's context.
    fn records_at_record_time(contexts: &[SystemContext]) -> Vec<(SystemContext, Vec<BodyRecord>)> {
        contexts
            .iter()
            .map(|ctx| {
                let snapshot = generate(SEED, ctx).snapshot_at(ctx, RECORD_TIME);
                (ctx.clone(), snapshot.bodies().to_vec())
            })
            .collect()
    }

    /// The Sun-like systems 1, 2, … of [`rocky_giant_and_sub_neptune`], with their records at
    /// [`RECORD_TIME`].
    fn sun_like_records() -> impl Iterator<Item = (SystemContext, Vec<BodyRecord>)> {
        (1..64).map(|index| {
            let id = CellKey::new(Layer::C, [0, 812, 0])
                .unwrap()
                .candidate_id(index)
                .unwrap();
            let ctx =
                synthetic_star(id, SolarMasses::new(1.0), Dex::ZERO, Years::new(4.57e9)).unwrap();
            records_at_record_time(&[ctx]).pop().unwrap()
        })
    }

    /// The record among `records` of the planet of `record`'s body if it is a moon, or `None`.
    fn planet_of<'a>(record: &BodyRecord, records: &'a [BodyRecord]) -> Option<&'a BodyRecord> {
        match (record.identity().kind(), record.identity().parent()) {
            (BodyKind::Moon(_), Some(OrbitHost::Body(planet))) => {
                records.iter().find(|r| r.index() == planet)
            }
            _ => None,
        }
    }

    /// Whose orbit sets the seasons of `record`'s body, among `records`, its system's.
    fn seasons_of<'a>(record: &BodyRecord, records: &'a [BodyRecord]) -> SeasonsFromRecord<'a> {
        planet_of(record, records).map_or(SeasonsFromRecord::Own, SeasonsFromRecord::Planet)
    }

    /// `record`'s inputs with [`basalt_crust`], no condensates and [`palette_for`] it.
    fn read(record: &BodyRecord, seasons: SeasonsFromRecord<'_>) -> CoarseInputs {
        CoarseInputs::from_record(
            record,
            seasons,
            basalt_crust(),
            [],
            palette_for(record, &[]),
        )
        .unwrap_or_else(|e| panic!("{:?} reads into no inputs: {e:?}", record.index()))
    }

    /// The dot product of two vectors.
    fn dot(u: [f64; 3], v: [f64; 3]) -> f64 {
        u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
    }

    /// Checks that `equinox` is where the sun, seen from a body whose pole is `pole` on the orbit
    /// `orbit`, crosses the body's equator going north: the sun's direction there lies in the
    /// equator's plane, and its motion about the orbit's normal carries it north.
    fn assert_northward_equinox(equinox: Radians, pole: [f64; 3], orbit: &Orientation) {
        let nu = equinox.value();
        assert!(
            (0.0..TAU).contains(&nu),
            "the equinox's anomaly {nu} is outside [0, 2π)"
        );
        let (p, n) = (orbit.periapsis_direction(), orbit.normal());
        let q = [
            n[1] * p[2] - n[2] * p[1],
            n[2] * p[0] - n[0] * p[2],
            n[0] * p[1] - n[1] * p[0],
        ];
        let (cos, sin) = (math::cos(nu), math::sin(nu));
        // The sun, seen from the body at true anomaly ν, lies along −(cos ν P + sin ν Q).
        let sun = [0, 1, 2].map(|i| -(cos * p[i] + sin * q[i]));
        assert!(
            dot(sun, pole).abs() < 1e-9,
            "at the equinox the sun is {} off the equator",
            dot(sun, pole)
        );
        let motion = [
            n[1] * sun[2] - n[2] * sun[1],
            n[2] * sun[0] - n[0] * sun[2],
            n[0] * sun[1] - n[1] * sun[0],
        ];
        assert!(
            dot(motion, pole) > 0.0,
            "at the equinox the sun moves south, {}",
            dot(motion, pole)
        );
    }

    #[test]
    fn the_equinox_is_where_the_sun_crosses_the_equator_going_north() {
        // In ecliptic axes with the vernal point on +x, a pole tilted ε towards +y, and an orbit
        // in the plane whose perihelion lies at longitude ϖ: the equinox falls at ν = 180° − ϖ.
        let tilted = |obliquity_deg: f64| {
            let (sin, cos) = math::sin_cos(Radians::from(Degrees::new(obliquity_deg)).value());
            [0.0, sin, cos]
        };
        let ecliptic = |perihelion_deg: f64| {
            Orientation::new(
                Radians::ZERO,
                Radians::ZERO,
                Radians::from(Degrees::new(perihelion_deg)),
            )
            .unwrap()
        };
        let degrees = |r: Radians| Degrees::from(r).value();
        // Earth: perihelion longitude 102.94719° (NASA's Earth fact sheet, J2000 mean elements),
        // so 77.05281°.
        let earth = equinox_true_anomaly(tilted(23.44), &ecliptic(102.947_19));
        assert!(
            (degrees(earth) - 77.052_81).abs() < 1e-9,
            "{}",
            degrees(earth)
        );
        // Mars: perihelion at Ls 251.000° (Allison and McEwen 2000, eqs. 16–17, at J2000, as
        // Mars24 implements them), a longitude of 71.0° on these axes, so 109.0°.
        let mars = equinox_true_anomaly(tilted(25.19), &ecliptic(71.0));
        assert!((degrees(mars) - 109.0).abs() < 1e-9, "{}", degrees(mars));
        // Any orbit and pole, a retrograde spin among them.
        for (inclination, node, argument) in [(0.3, 1.0, 2.0), (2.9, 4.0, 0.5), (1.2, 5.5, 6.1)] {
            let orbit = Orientation::new(
                Radians::new(inclination),
                Radians::new(node),
                Radians::new(argument),
            )
            .unwrap();
            for pole in [
                [0.6, 0.0, 0.8],
                [0.0, -0.28, -0.96],
                [-0.48, 0.6, 0.64],
                [1.0, 0.0, 0.0],
            ] {
                let equinox = equinox_true_anomaly(pole, &orbit);
                assert_northward_equinox(equinox, pole, &orbit);
            }
        }
        // A pole along the orbit's normal, either way, has no equinox.
        let orbit = ecliptic(40.0);
        assert_eq!(equinox_true_anomaly([0.0, 0.0, 1.0], &orbit), Radians::ZERO);
        assert_eq!(
            equinox_true_anomaly([0.0, 0.0, -1.0], &orbit),
            Radians::ZERO
        );
    }

    /// A hand-made seasonal water frost.
    fn frost() -> Condensate {
        Condensate {
            substance: key("H2O"),
            phase: CondensatePhase::Solid,
            reservoir: KilogramsPerSquareMetre::new(12.0),
            area_fraction: 0.01,
            persistence: Persistence::Seasonal,
        }
    }

    /// The palette [`planet_reading`] gives its planet: [`palette_for`] it, with water's ice entry
    /// for the [`frost`].
    fn frosted_palette(record: &BodyRecord) -> MaterialPalette {
        palette_for(record, &[(PaletteRole::Ice, "H2O")])
    }

    /// The rocky planet of [`rocky_giant_and_sub_neptune`], with its system's context, and its
    /// inputs read with [`basalt_crust`], the [`frost`] and the [`frosted_palette`].
    fn planet_reading() -> (SystemContext, BodyRecord, CoarseInputs) {
        let [(ctx, rocky), ..] = rocky_giant_and_sub_neptune();
        assert_eq!(
            RECORD_TIME,
            UniverseTime::EPOCH,
            "the helper's records are the epoch's"
        );
        let palette = frosted_palette(&rocky);
        let inputs = CoarseInputs::from_record(
            &rocky,
            SeasonsFromRecord::Own,
            basalt_crust(),
            [frost()],
            palette,
        )
        .unwrap();
        (ctx, rocky, inputs)
    }

    #[test]
    fn a_generated_planet_reads_its_sections_to_the_bit() {
        let (_, rocky, inputs) = planet_reading();
        let Section::Ok(surface) = rocky.surface() else {
            panic!(
                "a generated rocky planet's surface is {:?}",
                rocky.surface()
            );
        };
        let (conditions, figures) = (surface.conditions(), surface.figures());
        // The body, its figure and its gravity.
        let identity = rocky.identity();
        assert_eq!(
            inputs.body(),
            BodyRef::new(identity.system().raw(), identity.index().get())
        );
        let bulk = rocky.bulk().ok().unwrap();
        assert_eq!(inputs.figure(), rocky.figure().ok().unwrap().spheroid());
        assert_eq!(inputs.gravity(), bulk.surface_gravity());
        // The surface section's state, material, regime, conditions and figures, to the bit.
        assert_eq!(inputs.surface_state(), surface.state());
        assert_eq!(inputs.surface_material(), surface.material());
        assert_eq!(
            *inputs.climate(),
            ClimateRegime::from(surface.climate_regime())
        );
        assert_eq!(
            inputs.mean_surface_temperature(),
            conditions.mean_surface_temperature()
        );
        assert_same_bits(
            inputs.equator_pole_contrast_k(),
            conditions.equator_pole_contrast_k(),
        );
        assert_same_bits(
            inputs.day_night_contrast_k(),
            conditions.day_night_contrast_k(),
        );
        let pressure = match surface.state() {
            SurfaceState::Airless => Pascals::ZERO,
            _ => conditions.surface_pressure(),
        };
        assert_eq!(inputs.surface_pressure(), pressure);
        let gases: Vec<_> = conditions
            .gases()
            .iter()
            .map(|&(id, x)| (id.substance().key(), x))
            .collect();
        let read_gases: Vec<_> = inputs
            .gases()
            .iter()
            .map(|g| (g.substance, g.mole_fraction))
            .collect();
        assert_eq!(read_gases, gases, "the gases, in the record's order");
        assert_eq!(inputs.host_flux(), conditions.stellar_flux());
        assert_eq!(inputs.sigma_h(), figures.sigma_h());
        let wet =
            |shares: &[(SubstanceId, f64)]| -> Vec<AreaShare> { on_the_ground(shares).collect() };
        assert_eq!(inputs.liquids(), wet(figures.surface_liquids()).as_slice());
        assert_eq!(inputs.ices(), wet(figures.surface_ices()).as_slice());
        assert_eq!(inputs.tectonics(), figures.tectonic_regime());
        assert_same_bits(
            inputs.continental_fraction(),
            figures.continental_fraction(),
        );
        assert_same_bits(inputs.volcanism(), figures.volcanism());
        assert_eq!(inputs.heat_flow(), figures.heat_flow());
        assert_same_bits(
            inputs.surface_age().value(),
            figures.surface_age().value() / 1e9,
        );
        // The interim until plan 14's `volcanic_surface_age` (decision-r09-t14-mars-ceiling §2.1).
        assert_same_bits(inputs.province_surface_age().value(), 0.0);
        assert_eq!(inputs.craters(), figures.craters());
        assert_eq!(
            inputs.wet_epoch().copied(),
            figures.wet_epoch().map(WetEpoch::from)
        );
        // The arguments, as given.
        assert_eq!(*inputs.crust(), basalt_crust());
        assert_eq!(inputs.condensates(), [frost()].as_slice());
        assert_eq!(inputs.palette(), &frosted_palette(&rocky));
    }

    #[test]
    fn a_generated_planet_takes_its_seasons_and_light_from_its_own_orbit() {
        let (ctx, rocky, inputs) = planet_reading();
        // The seasons: the planet's own orbit at the record's time, and its spin under it.
        let orbit = rocky.orbit().ok().unwrap();
        let elements = elements_at(orbit, RECORD_TIME).unwrap();
        let seasonal = inputs.seasonal_orbit().unwrap();
        assert_same_bits(seasonal.eccentricity, elements.eccentricity().value());
        assert_eq!(seasonal.period, elements.period());
        let frame = rocky.rotation().ok().unwrap();
        let spin = inputs.spin();
        assert_eq!(spin.obliquity, frame.obliquity());
        let rate = frame.rate().rate_at(RECORD_TIME);
        assert_same_bits(spin.rotation_period.value(), TAU / rate);
        match spin.solar_day {
            // A day turns at the spin less the sun's mean motion, prograde below 90°.
            Some(day) => {
                let synodic =
                    (1.0 / spin.rotation_period.value() - 1.0 / seasonal.period.value()).abs();
                assert!((day.value() * synodic - 1.0).abs() < 1e-9, "{day:?}");
            }
            None => assert!(matches!(
                frame.state_at(RECORD_TIME),
                crate::planetary::derive::rotation::SpinState::Locked(_)
            )),
        }
        assert_northward_equinox(
            spin.equinox_true_anomaly,
            frame.pole(),
            elements.orientation(),
        );
        // The hosts' light: one star's, along the seasonal orbit at the record's time.
        let star = ctx.stars()[0].state_at(RECORD_TIME).unwrap();
        let light =
            WattsPerSquareMetre::from(crate::planetary::derive::irradiation::luminosity_flux(
                star.luminosity(),
                elements.semi_major_axis(),
                elements.eccentricity().value(),
            ));
        assert!(
            (inputs.host_flux().value() / light.value() - 1.0).abs() < 1e-12,
            "{:?} against the star's {light:?}",
            inputs.host_flux()
        );
        // `for_body` reads the same seasons and light, with the record's own crust.
        let own = CoarseInputs::for_body(&rocky, None, &ctx).unwrap();
        assert_eq!(own.seasonal_orbit(), inputs.seasonal_orbit());
        assert_eq!(own.spin(), inputs.spin());
        assert_eq!(own.host_flux(), inputs.host_flux());
    }

    /// The first moon with a surface section in the Sun-like systems 1, 2, … of
    /// [`rocky_giant_and_sub_neptune`], with its system's context, its record and its planet's,
    /// at [`RECORD_TIME`].
    fn generated_moon() -> (SystemContext, BodyRecord, BodyRecord) {
        for (ctx, records) in sun_like_records() {
            for record in &records {
                if let Section::Ok(_) = record.surface()
                    && let Some(planet) = planet_of(record, &records)
                {
                    return (ctx, record.clone(), planet.clone());
                }
            }
        }
        panic!("63 Sun-like systems hold no moon with a surface");
    }

    #[test]
    fn a_moon_takes_its_seasons_from_its_planet_s_orbit() {
        let (ctx, moon, planet) = generated_moon();
        let inputs = read(&moon, SeasonsFromRecord::Planet(&planet));
        // The planet's orbit sets the year, not the moon's own about its planet.
        let planet_orbit = elements_at(planet.orbit().ok().unwrap(), RECORD_TIME).unwrap();
        let seasonal = inputs.seasonal_orbit().unwrap();
        assert_same_bits(seasonal.eccentricity, planet_orbit.eccentricity().value());
        assert_eq!(seasonal.period, planet_orbit.period());
        assert_ne!(
            seasonal.period,
            moon.orbit().ok().unwrap().elements().period()
        );
        // Its obliquity is to the planet's orbit, and its equinox on it; it turns under its star.
        let frame = moon.rotation().ok().unwrap();
        let normal = planet_orbit.orientation().normal();
        let tilt = math::acos(dot(frame.pole(), normal).clamp(-1.0, 1.0));
        let spin = inputs.spin();
        assert!((spin.obliquity.value() - tilt).abs() < 1e-12, "{spin:?}");
        assert_northward_equinox(
            spin.equinox_true_anomaly,
            frame.pole(),
            planet_orbit.orientation(),
        );
        assert!(spin.solar_day.is_some(), "a moon has a solar day: {spin:?}");
        // A moon needs its planet's record, and no other's.
        let read_with = |seasons| {
            CoarseInputs::from_record(&moon, seasons, basalt_crust(), [], palette_for(&moon, &[]))
        };
        assert_eq!(
            read_with(SeasonsFromRecord::Own),
            Err(ReadCoarseInputsError::PlanetMissing(body_id_of(&planet)))
        );
        assert_eq!(
            read_with(SeasonsFromRecord::Planet(&moon)),
            Err(ReadCoarseInputsError::NotItsPlanet {
                planet: Some(body_id_of(&planet)),
                given: body_id_of(&moon),
            })
        );
        // A body of another system with the planet's index is not its planet.
        let namesake = sun_like_records()
            .filter(|(other, _)| other.id() != ctx.id())
            .find_map(|(_, records)| records.into_iter().find(|r| r.index() == planet.index()))
            .expect("another Sun-like system has a body with the planet's index");
        assert_eq!(
            read_with(SeasonsFromRecord::Planet(&namesake)),
            Err(ReadCoarseInputsError::NotItsPlanet {
                planet: Some(body_id_of(&planet)),
                given: body_id_of(&namesake),
            })
        );
        // The planet's orbit, withheld from a contact's record.
        assert_eq!(
            read_with(SeasonsFromRecord::Planet(
                &planet.degrade(DetailLevel::Contact)
            )),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NotResolved(RecordSection::Orbit)
            ))
        );
        // A body that is not a moon takes no planet's record.
        let [(_, rocky), ..] = rocky_giant_and_sub_neptune();
        assert_eq!(
            CoarseInputs::from_record(
                &rocky,
                SeasonsFromRecord::Planet(&planet),
                basalt_crust(),
                [],
                palette_for(&rocky, &[])
            ),
            Err(ReadCoarseInputsError::NotItsPlanet {
                planet: None,
                given: body_id_of(&planet),
            })
        );
        let own = CoarseInputs::for_body(&moon, Some(&planet), &ctx).unwrap();
        assert_eq!(own.seasonal_orbit(), inputs.seasonal_orbit());
        assert_eq!(own.spin(), inputs.spin());
    }

    /// The first belt's member with a surface section in the Sun-like systems 1, 2, … of
    /// [`rocky_giant_and_sub_neptune`] (a dwarf planet, listed under its belt), with its system's
    /// context, its record and its belt's, at [`RECORD_TIME`].
    fn generated_belt_member() -> (SystemContext, BodyRecord, BodyRecord) {
        for (ctx, records) in sun_like_records() {
            for record in &records {
                if let (Section::Ok(_), Some(OrbitHost::Body(belt))) =
                    (record.surface(), record.identity().parent())
                    && record.identity().kind() == BodyKind::DwarfPlanet
                {
                    let belt = records.iter().find(|r| r.index() == belt).unwrap();
                    return (ctx, record.clone(), belt.clone());
                }
            }
        }
        panic!("63 Sun-like systems hold no belt's member with a surface");
    }

    /// R09.T10.f (decision-backlog-2 §3): `for_body` takes a moon's planet's record, the one
    /// `planet_of` names, and checks it after the surface section's tag and before the crust;
    /// from P14.T54.a a body that passes reads `Ok`.
    #[test]
    fn for_body_checks_a_moon_s_planet_before_the_crust() {
        let (ctx, moon, planet) = generated_moon();
        // A moon given its planet passes the check, and reads its crust.
        assert_eq!(CoarseInputs::planet_of(&moon), Some(planet.index()));
        assert!(
            CoarseInputs::for_body(&moon, Some(&planet), &ctx).is_ok(),
            "{:?}",
            CoarseInputs::for_body(&moon, Some(&planet), &ctx)
        );
        // Without it, or with another body, the planet is refused before the crust is reached.
        assert_eq!(
            CoarseInputs::for_body(&moon, None, &ctx),
            Err(ReadCoarseInputsError::PlanetMissing(body_id_of(&planet)))
        );
        assert_eq!(
            CoarseInputs::for_body(&moon, Some(&moon), &ctx),
            Err(ReadCoarseInputsError::NotItsPlanet {
                planet: Some(body_id_of(&planet)),
                given: body_id_of(&moon),
            })
        );
        // A planet takes none, and is refused one.
        let [(rocky_ctx, rocky), (giant_ctx, giant), _] = rocky_giant_and_sub_neptune();
        assert_eq!(CoarseInputs::planet_of(&rocky), None);
        assert!(CoarseInputs::for_body(&rocky, None, &rocky_ctx).is_ok());
        assert_eq!(
            CoarseInputs::for_body(&rocky, Some(&planet), &rocky_ctx),
            Err(ReadCoarseInputsError::NotItsPlanet {
                planet: None,
                given: body_id_of(&planet),
            })
        );
        // A belt's member is listed under its belt, a body, but orbits the belt's host: it takes
        // none, and its belt's record is not its planet's.
        let (member_ctx, member, belt) = generated_belt_member();
        assert!(
            matches!(belt.identity().kind(), BodyKind::Belt(_)),
            "{:?}",
            belt.identity()
        );
        assert_eq!(
            member.identity().parent(),
            Some(OrbitHost::Body(belt.index()))
        );
        assert_eq!(CoarseInputs::planet_of(&member), None);
        // It passes the planet's check: it reads, or stops at its surface's own state (a cold
        // member's argon ice, whose solid density the registry has not read).
        let Section::Ok(surface) = member.surface() else {
            unreachable!("the finder took a member with a surface");
        };
        let read = CoarseInputs::for_body(&member, None, &member_ctx);
        match unread_cause(surface) {
            None => assert!(read.is_ok(), "{read:?}"),
            Some(cause) => assert_eq!(
                read,
                Err(ReadCoarseInputsError::Section(
                    SurfaceInputsError::NotModelled(RecordSection::Surface)
                )),
                "{cause}"
            ),
        }
        assert_eq!(
            CoarseInputs::for_body(&member, Some(&belt), &member_ctx),
            Err(ReadCoarseInputsError::NotItsPlanet {
                planet: None,
                given: body_id_of(&belt),
            })
        );
        // The surface section's tag comes first: a giant has no solid surface whatever planet it
        // is given, and a moon's withheld section is named before its missing planet.
        assert_eq!(
            CoarseInputs::for_body(&giant, Some(&planet), &giant_ctx),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NoSolidSurface
            ))
        );
        assert_eq!(
            CoarseInputs::for_body(&moon.degrade(DetailLevel::Bulk), None, &ctx),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NotResolved(RecordSection::Surface)
            ))
        );
    }

    #[test]
    fn the_seasonal_orbit_follows_its_drift_inside_its_cell_and_holds_nowhere_else() {
        // The first evolving orbit, with a drift and a cell's end, of the Sun-like systems.
        let orbit = sun_like_records()
            .find_map(|(_, records)| {
                records.iter().find_map(|record| {
                    let orbit = *record.orbit().ok()?;
                    let drift = orbit.drift()?;
                    let moving = drift.eccentricity_rate_per_s().abs() > 0.0
                        && drift.mean_motion_rate_rad_per_s2().abs() > 0.0;
                    (moving && orbit.valid_until().is_some()).then_some(orbit)
                })
            })
            .expect("a Sun-like system has an evolving orbit");
        let drift = *orbit.drift().unwrap();
        let (start, end) = (drift.reference(), orbit.valid_until().unwrap());
        assert_eq!(
            start, RECORD_TIME,
            "a cell of a record at the epoch starts there"
        );
        let elements = orbit.elements();
        // At the cell's start, the record's elements to the bit.
        assert_eq!(elements_at(&orbit, start), Ok(*elements));
        // Inside it, the model's eccentricity, mean motion and axis.
        let mid =
            UniverseTime::new(start.seconds() + (end.seconds() - start.seconds()) / 2, 0).unwrap();
        let dt = seconds_between(start, mid);
        assert!(dt > 0.0, "the cell is {start:?} to {end:?}");
        let at = elements_at(&orbit, mid).unwrap();
        assert_same_bits(
            at.eccentricity().value(),
            elements.eccentricity().value() + drift.eccentricity_rate_per_s() * dt,
        );
        let n = TAU / elements.period().value() + drift.mean_motion_rate_rad_per_s2() * dt;
        assert_same_bits(at.period().value(), TAU / n);
        let a = elements.semi_major_axis().value() + drift.semi_major_axis_rate_m_per_s() * dt;
        assert!(
            (at.semi_major_axis().value() / a - 1.0).abs() < 1e-14,
            "{:?} against {a} m",
            at.semi_major_axis()
        );
        assert_eq!(at.orientation(), elements.orientation());
        // Before the cell and from its end the record does not hold, as for a record taken at
        // another time.
        let before = UniverseTime::new(start.seconds() - 1, 0).unwrap();
        assert_eq!(
            elements_at(&orbit, before),
            Err(ReadCoarseInputsError::NotAtRecordTime)
        );
        assert_eq!(
            elements_at(&orbit, end),
            Err(ReadCoarseInputsError::NotAtRecordTime)
        );
        // An orbit that holds has no start, but an end.
        let holding = BodyOrbit::new(*elements, Some(end));
        assert_eq!(elements_at(&holding, before), Ok(*elements));
        assert_eq!(
            elements_at(&holding, end),
            Err(ReadCoarseInputsError::NotAtRecordTime)
        );
    }

    #[test]
    fn a_section_the_reading_cannot_read_is_named() {
        assert_eq!(section(&Section::Ok(7_u8), RecordSection::Bulk), Ok(&7));
        // A free-floating body's orbit does not apply, and gives no seasons this reading models.
        assert_eq!(
            section(&Section::<u8>::NotApplicable, RecordSection::Orbit),
            Err(SurfaceInputsError::NotModelled(RecordSection::Orbit))
        );
        assert_eq!(
            section(&Section::<u8>::NotModelled, RecordSection::Rotation),
            Err(SurfaceInputsError::NotModelled(RecordSection::Rotation))
        );
        assert_eq!(
            section(&Section::<u8>::NotResolved, RecordSection::Figure),
            Err(SurfaceInputsError::NotResolved(RecordSection::Figure))
        );
    }

    #[test]
    fn a_share_with_no_area_lies_nowhere_on_the_ground() {
        let (water, methane, ammonia) = (
            Gas::Water.substance(),
            Gas::Methane.substance(),
            Gas::Ammonia.substance(),
        );
        // A negative share is kept, for the builder to refuse.
        let shares = [
            (water, 0.0),
            (methane, 0.25),
            (water, -0.0),
            (ammonia, -0.1),
        ];
        let ground: Vec<_> = on_the_ground(&shares).collect();
        let share = |id: SubstanceId, area_fraction| AreaShare {
            substance: id.substance().key(),
            area_fraction,
        };
        assert_eq!(ground, [share(methane, 0.25), share(ammonia, -0.1)]);
    }

    #[test]
    fn the_reading_refuses_what_it_cannot_read() {
        let [(_, rocky), (_, giant), _] = rocky_giant_and_sub_neptune();
        let read_with = |record: &BodyRecord, crust, palette| {
            CoarseInputs::from_record(record, SeasonsFromRecord::Own, crust, [], palette)
        };
        assert_eq!(
            read_with(&giant, basalt_crust(), palette_for(&rocky, &[])),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NoSolidSurface
            ))
        );
        assert_eq!(
            read_with(
                &rocky.degrade(DetailLevel::Bulk),
                basalt_crust(),
                palette_for(&rocky, &[])
            ),
            Err(ReadCoarseInputsError::Section(
                SurfaceInputsError::NotResolved(RecordSection::Surface)
            ))
        );
        // The builder's refusals of the arguments: a palette without the crust's lithology, and a
        // lid naming a crust the body lacks.
        let water_only = palette(vec![
            entry("H2O", PaletteRole::Ice),
            entry("H2O", PaletteRole::Liquid),
        ]);
        let no_basalt = read_with(&rocky, basalt_crust(), water_only);
        assert_eq!(
            no_basalt,
            Err(ReadCoarseInputsError::Build(
                BuildCoarseInputsError::NotInPalette {
                    part: "secondary crust",
                    substance: key("basalt"),
                    role: PaletteRole::SecondaryCrust,
                }
            ))
        );
        let primary_lid = CrustInputs {
            lid: LidCrust::Primary,
            ..basalt_crust()
        };
        assert_eq!(
            read_with(&rocky, primary_lid, palette_for(&rocky, &[])),
            Err(ReadCoarseInputsError::Build(
                BuildCoarseInputsError::LidAbsent(LidCrust::Primary)
            ))
        );
    }

    #[test]
    fn the_reading_s_refusals_say_why() {
        let [(_, rocky), ..] = rocky_giant_and_sub_neptune();
        let id = body_id_of(&rocky);
        let not_in_palette = BuildCoarseInputsError::NotInPalette {
            part: "secondary crust",
            substance: key("basalt"),
            role: PaletteRole::SecondaryCrust,
        };
        let texts = [
            (
                ReadCoarseInputsError::Section(SurfaceInputsError::NotModelled(
                    RecordSection::Surface,
                )),
                "the body's surface section is not modelled".to_owned(),
            ),
            (
                ReadCoarseInputsError::PlanetMissing(id),
                format!(
                    "a moon of body {id} has its planet's seasons, and no planet's record was \
                     given"
                ),
            ),
            (
                ReadCoarseInputsError::NotItsPlanet {
                    planet: Some(id),
                    given: id,
                },
                format!("body {id}'s record was given as the planet of a moon of body {id}"),
            ),
            (
                ReadCoarseInputsError::NotItsPlanet {
                    planet: None,
                    given: id,
                },
                format!("body {id}'s record was given as the planet of a body that is not a moon"),
            ),
            (
                ReadCoarseInputsError::NotAtRecordTime,
                "the record's seasonal orbit does not hold at the record's time".to_owned(),
            ),
            (
                ReadCoarseInputsError::SeasonalOrbit(BuildOrbitError::EccentricityOutOfRange {
                    value: -1e-9,
                }),
                "the seasonal orbit at the record's time is not a bound orbit".to_owned(),
            ),
            (
                ReadCoarseInputsError::Build(not_in_palette),
                "the coarse inputs read from the record were refused".to_owned(),
            ),
        ];
        for (error, text) in texts {
            assert_eq!(error.to_string(), text);
        }
        assert!(
            id.to_string().contains('.'),
            "a body's ID names its system: {id}"
        );
        let refused = ReadCoarseInputsError::Build(not_in_palette);
        assert_eq!(
            refused
                .source()
                .unwrap()
                .downcast_ref::<BuildCoarseInputsError>(),
            Some(&not_in_palette)
        );
        for error in [
            ReadCoarseInputsError::Section(SurfaceInputsError::NoSolidSurface),
            ReadCoarseInputsError::NotAtRecordTime,
        ] {
            assert!(error.source().is_none(), "{error:?}");
        }
    }

    /// How many of the sample's real systems the census reads.
    const CENSUS: usize = 40;

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one walk of the census's bodies, each reading checked and tallied"
    )]
    fn every_generated_solid_body_reads_into_inputs_the_builder_takes() {
        let contexts = sample_contexts(CENSUS, SEED, SampleFilter::ALL).unwrap();
        let (mut bodies, mut moons, mut airless, mut aired, mut drifting, mut dry) =
            (0, 0, 0, 0, 0, 0);
        let mut zeroed = 0;
        let (mut read_ok, mut melting, mut seas) = (0, 0, 0);
        let mut unread: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for (ctx, records) in records_at_record_time(&contexts) {
            for record in &records {
                // `planet_of` is the reading's rule, on every body: a moon's planet, by its kind.
                assert_eq!(
                    CoarseInputs::planet_of(record),
                    planet_of(record, &records).map(BodyRecord::index),
                    "{:?}",
                    record.index()
                );
                let Section::Ok(surface) = record.surface() else {
                    continue;
                };
                let seasons = seasons_of(record, &records);
                // The palette names only what lies on the ground: a share with no area needs no
                // entry.
                let inputs = read(record, seasons);
                let figures = surface.figures();
                bodies += 1;
                dry += usize::from(
                    figures
                        .surface_liquids()
                        .iter()
                        .chain(figures.surface_ices())
                        .any(|&(_, area_fraction)| area_fraction.classify() == FpCategory::Zero),
                );
                // Airless is zero pressure, to the bit; every other state keeps the record's.
                if surface.state() == SurfaceState::Airless {
                    airless += 1;
                    // The record's tenuous air, which the header states as none.
                    zeroed += usize::from(surface.conditions().surface_pressure().value() > 0.0);
                    assert_same_bits(inputs.surface_pressure().value(), 0.0);
                } else {
                    aired += 1;
                    assert_eq!(
                        inputs.surface_pressure(),
                        surface.conditions().surface_pressure()
                    );
                }
                // The seasons: the moon's planet's orbit or the body's own.
                let orbit = match seasons {
                    SeasonsFromRecord::Own => record.orbit(),
                    SeasonsFromRecord::Planet(planet) => {
                        moons += 1;
                        planet.orbit()
                    }
                };
                let orbit = orbit.ok().unwrap();
                drifting += usize::from(orbit.drift().is_some());
                let elements = elements_at(orbit, RECORD_TIME).unwrap();
                assert_eq!(
                    inputs.seasonal_orbit().map(|o| o.period),
                    Some(elements.period())
                );
                let spin = inputs.spin();
                if math::sin(spin.obliquity.value()) > 1e-6 {
                    assert_northward_equinox(
                        spin.equinox_true_anomaly,
                        record.rotation().ok().unwrap().pole(),
                        elements.orientation(),
                    );
                }
                // `for_body` takes the planet `planet_of` names, as the server fetches it, and
                // reads every generated body whose crust the record models (P14.T54.a).
                let planet = CoarseInputs::planet_of(record).map(|index| {
                    records
                        .iter()
                        .find(|r| r.index() == index)
                        .expect("a moon's planet is a body of its system")
                });
                let seasonal_planet = match seasons {
                    SeasonsFromRecord::Own => None,
                    SeasonsFromRecord::Planet(planet) => Some(planet.index()),
                };
                assert_eq!(planet.map(BodyRecord::index), seasonal_planet);
                let own = match CoarseInputs::for_body(record, planet, &ctx) {
                    Ok(own) => own,
                    Err(error) => {
                        assert_eq!(
                            error,
                            ReadCoarseInputsError::Section(SurfaceInputsError::NotModelled(
                                RecordSection::Surface
                            )),
                            "{:?}",
                            record.index()
                        );
                        let cause = unread_cause(surface).unwrap_or_else(|| {
                            panic!("{:?} reads no inputs for no cause", record.index())
                        });
                        *unread.entry(cause).or_default() += 1;
                        continue;
                    }
                };
                assert_eq!(unread_cause(surface), None, "{:?}", record.index());
                read_ok += 1;
                check_record_palette(&own);
                melting += usize::from(
                    own.palette()
                        .entries()
                        .iter()
                        .any(|e| e.role != PaletteRole::Liquid && e.liquid.is_some()),
                );
                seas += usize::from(own.main_liquid().is_some());
                // The header's pressure is exactly 0 on an airless body (R09.T2.e).
                let header = super::super::quantise::header_pressure(&own).value();
                if surface.state() == SurfaceState::Airless {
                    assert_same_bits(own.surface_pressure().value(), 0.0);
                    assert_same_bits(header, 0.0);
                }
            }
        }
        eprintln!(
            "census: {bodies} bodies: {moons} moons, {airless} airless ({zeroed} with a tenuous \
             air zeroed), {aired} with air, {drifting} on drifting seasonal orbits, {dry} listing \
             a share with no area; {read_ok} read by `for_body` ({seas} with a sea, {melting} \
             with a melting crust entry); not modelled: {unread:?}"
        );
        assert!(
            moons > 0 && airless > 0 && aired > 0 && bodies > moons && dry > 0,
            "{bodies} bodies: {moons} moons, {airless} airless, {aired} with air, \
             {drifting} on drifting orbits, {dry} listing a share with no area"
        );
        let unmodelled: usize = unread.values().sum();
        assert!(
            read_ok > 0 && read_ok + unmodelled == bodies,
            "{read_ok} of {bodies}"
        );
        // The recorded gaps alone: no crust, an unlit record's 0 K, and the ices whose solid
        // density the registry has not read under 23's rules (argon's, β-O2's: P14.T49.n reads
        // them, held until 24; `under_24_s_densities_the_census_refuses_no_ice_for_its_density`).
        for cause in unread.keys() {
            let known = cause == NO_CRUST
                || cause == UNLIT
                || ["Ar Ice", "O2 Ice"]
                    .iter()
                    .any(|ice| cause == &format!("{ice}: DensityNotModelled"));
            assert!(known, "{cause}");
        }
    }

    /// P14.T49.n's densities, held until 24 (`READ_DENSITIES_VERSION`): the census's systems,
    /// generated and read under 24's densities, the census's walk of `for_body` alone. It refuses
    /// no body for an ice whose solid density the registry has not read, solid argon's (fcc) and
    /// β-O2's being read: only the census's other gaps remain, a crust not modelled and an unlit
    /// record's 0 K. Under 23's rules the census
    /// (`every_generated_solid_body_reads_into_inputs_the_builder_takes`) allows the "Ar Ice" and
    /// "O2 Ice" refusals.
    #[test]
    fn under_24_s_densities_the_census_refuses_no_ice_for_its_density() {
        use crate::substance::{ReadDensitiesRules, with_read_densities};
        let contexts = sample_contexts(CENSUS, SEED, SampleFilter::ALL).unwrap();
        let (bodies, read_ok, unread) = with_read_densities(ReadDensitiesRules::Read, || {
            let (mut bodies, mut read_ok) = (0_usize, 0_usize);
            let mut unread: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for (ctx, records) in records_at_record_time(&contexts) {
                for record in &records {
                    let Section::Ok(surface) = record.surface() else {
                        continue;
                    };
                    bodies += 1;
                    let planet = planet_of(record, &records);
                    match CoarseInputs::for_body(record, planet, &ctx) {
                        Ok(_) => {
                            assert_eq!(unread_cause(surface), None, "{:?}", record.index());
                            read_ok += 1;
                        }
                        Err(error) => {
                            assert_eq!(
                                error,
                                ReadCoarseInputsError::Section(SurfaceInputsError::NotModelled(
                                    RecordSection::Surface
                                )),
                                "{:?}",
                                record.index()
                            );
                            let cause = unread_cause(surface).unwrap_or_else(|| {
                                panic!("{:?} reads no inputs for no cause", record.index())
                            });
                            *unread.entry(cause).or_default() += 1;
                        }
                    }
                }
            }
            (bodies, read_ok, unread)
        });
        eprintln!(
            "census under 24's densities: {bodies} bodies, {read_ok} read by `for_body`; not \
             modelled: {unread:?}"
        );
        let unmodelled: usize = unread.values().sum();
        assert!(
            read_ok > 0 && read_ok + unmodelled == bodies,
            "{read_ok} of {bodies}"
        );
        for cause in unread.keys() {
            assert!(cause == NO_CRUST || cause == UNLIT, "{cause}");
        }
    }

    /// What `unread_cause` names a body whose crust the record does not model.
    const NO_CRUST: &str = "no crust";

    /// What `unread_cause` names an unlit body, whose record's mean surface temperature is 0 K.
    const UNLIT: &str = "unlit";

    /// Why `for_body` reads no inputs for a body whose surface section is `surface`, or `None`
    /// where it reads them: its crust not modelled, its mean surface temperature not positive, or
    /// the first palette entry the registry does not resolve, as `"<key> <role>: <cause>"`. Also
    /// P14.T49.n's census's reading (`planetary/system/tests.rs`), which cannot reach the
    /// palette's module.
    pub(crate) fn unread_cause(surface: &Surface) -> Option<String> {
        let Modelled::Ok(crust) = surface.crust() else {
            return Some(NO_CRUST.to_owned());
        };
        let mean = surface.conditions().mean_surface_temperature().value();
        let lit = mean.is_finite() && mean > 0.0;
        if !lit {
            return Some(UNLIT.to_owned());
        }
        match palette::resolve(surface, crust) {
            Ok(_) => None,
            Err(palette::ResolveBodyPaletteError::Solid {
                substance,
                role,
                cause,
            }) => Some(format!("{substance} {role:?}: {cause:?}")),
            Err(other) => Some(format!("{other:?}")),
        }
    }

    /// Checks the palette `for_body` resolved for a body, its inputs `inputs` (the palette
    /// resolution's rules, P14.T54.a): an entry for each crust and the provinces, each
    /// condensate as its phase's, each ice and liquid on the ground, and nothing else; each solid
    /// entry its registry row's, phase row 1 or 2, with no `FeO_T`; each liquid its record's, its
    /// grain density the record's density, its albedo its Fresnel reflectance, phase row 0; each
    /// record's temperature in its liquid's range; and every phase row 0 to 2
    /// (decision-r11-palette-liquids §2 item 6).
    fn check_record_palette(inputs: &CoarseInputs) {
        use crate::substance::by_key;
        let palette = inputs.palette();
        let crust = inputs.crust();
        let lithologies = [crust.primary, crust.secondary, crust.tertiary, crust.bulk]
            .into_iter()
            .flatten()
            .chain([crust.provinces]);
        let mut named: std::collections::BTreeSet<(bool, SubstanceKey)> =
            lithologies.map(|key| (true, key)).collect();
        for condensate in inputs.condensates() {
            if condensate.phase != CondensatePhase::Supercritical {
                named.insert((false, condensate.substance));
            }
        }
        for share in inputs.ices().iter().chain(inputs.liquids()) {
            named.insert((false, share.substance));
        }
        for entry in palette.entries() {
            let key = entry.substance;
            assert!(entry.phase_row <= 2, "{key}: phase row {}", entry.phase_row);
            assert_eq!(entry.feo_mass_fraction, None, "{key}");
            let crust_role = entry.role.is_crust();
            assert!(
                named.contains(&(crust_role, key)),
                "{key} in {:?} is not named by the record",
                entry.role
            );
            let row = by_key(key.as_str()).unwrap();
            match entry.role {
                PaletteRole::Liquid => {
                    let record = entry.liquid.as_ref().expect("a liquid carries its record");
                    assert_same_bits(entry.grain_density_kg_m3, record.density_kg_m3);
                    let fresnel = record.fresnel_normal_albedo_bvr();
                    for (held, f) in entry.normal_albedo_bvr.iter().zip(fresnel) {
                        assert_same_bits(*held, f);
                    }
                    assert_eq!(entry.phase_row, 0, "{key}");
                    let first_melt = row.substance().condensed().unwrap().transition();
                    assert!(
                        record.temperature >= first_melt.temperature(),
                        "{key}: {:?}",
                        record.temperature
                    );
                }
                PaletteRole::Ice
                | PaletteRole::PrimaryCrust
                | PaletteRole::SecondaryCrust
                | PaletteRole::TertiaryCrust
                | PaletteRole::Province
                | PaletteRole::Bulk
                | PaletteRole::Cover
                | PaletteRole::Deposit => {
                    let optics = row.substance().condensed_optics().unwrap();
                    for (held, a) in entry
                        .normal_albedo_bvr
                        .iter()
                        .zip(optics.normal_albedo_bvr())
                    {
                        assert_same_bits(*held, a);
                    }
                    let condensed = row.substance().condensed().unwrap();
                    assert_eq!(entry.phase_row, u8::from(condensed.phase_law()), "{key}");
                    assert!((1..=2).contains(&entry.phase_row), "{key}");
                    if let Some(melt) = &entry.liquid {
                        assert!(crust_role, "{key}: a melt record on {:?}", entry.role);
                        assert!(melt.temperature >= entry.transition, "{key}: {melt:?}");
                    }
                }
            }
        }
        // The melt rule: the lid's entry melts where the crust gives a melt area, and its row has
        // a melt model.
        let lid = palette.get(inputs.lid_entry()).unwrap();
        if crust.melt_area_fraction > 0.0 {
            let modelled = crate::substance::liquid_properties(
                by_key(lid.substance.as_str()).unwrap(),
                Kelvin::new(lid.transition.value() + 1.0),
                crate::substance::Composition::Reference,
            )
            .is_ok();
            let ice = matches!(
                lid.mechanics,
                MechanicsFamily::WaterIce | MechanicsFamily::VolatileIce
            );
            assert_eq!(
                lid.liquid.is_some(),
                modelled && !ice,
                "{}: melt area {}",
                lid.substance,
                crust.melt_area_fraction
            );
        }
    }

    /// The first body of the plan-14 sample's systems (`system::tests::whole`, in order), at
    /// [`RECORD_TIME`], whose surface section `wanted` picks among those whose crust is modelled,
    /// with its system's context and its inputs from `for_body`; `None` where none is.
    fn first_generated(
        wanted: impl Fn(&Surface) -> bool,
    ) -> Option<(SystemContext, BodyRecord, CoarseInputs)> {
        for (ctx, system) in crate::planetary::system::tests::whole() {
            let snapshot = system.snapshot_at(ctx, RECORD_TIME);
            let records = snapshot.bodies();
            for record in records {
                let Section::Ok(surface) = record.surface() else {
                    continue;
                };
                if matches!(surface.crust(), Modelled::NotModelled) || !wanted(surface) {
                    continue;
                }
                let planet = planet_of(record, records);
                let inputs = CoarseInputs::for_body(record, planet, ctx)
                    .unwrap_or_else(|e| panic!("{:?} reads no inputs: {e:?}", record.index()));
                return Some((ctx.clone(), record.clone(), inputs));
            }
        }
        None
    }

    /// The entry of `key` in `role` in `inputs`' palette.
    fn entry_of<'a>(inputs: &'a CoarseInputs, role: PaletteRole, key: &str) -> &'a PaletteEntry {
        let at = inputs
            .palette()
            .find(role, SubstanceKey::new(key).unwrap())
            .unwrap_or_else(|| panic!("no {role:?} entry of {key}: {:?}", inputs.palette()));
        inputs.palette().get(at).unwrap()
    }

    /// Whether `surface` holds a liquid condensate of `key` over some of its area.
    fn has_liquid(surface: &Surface, key: &str) -> bool {
        surface.condensates().entries().iter().any(|entry| {
            entry.phase() == partition::CondensatePhase::Liquid
                && entry.substance().substance().key().as_str() == key
                && entry.area_fraction() > 0.0
        })
    }

    /// The first Titan-like body of the plan-14 sample, with a `ch4_c2h6_n2` sea and its mean
    /// surface temperature near Titan's 93.65 K (Huygens), with its context and its inputs from
    /// `for_body`.
    fn first_titan_like() -> (SystemContext, BodyRecord, CoarseInputs) {
        first_generated(|surface| {
            let t = surface.conditions().mean_surface_temperature().value();
            has_liquid(surface, "ch4_c2h6_n2") && (85.0..=100.0).contains(&t)
        })
        .expect("the sample holds a Titan-like body with a ch4_c2h6_n2 sea")
    }

    /// The first Earth-like body of the plan-14 sample, temperate with a water sea and its mean
    /// surface temperature at 275–305 K, with its context and its inputs from `for_body`.
    fn first_earth_like() -> (SystemContext, BodyRecord, CoarseInputs) {
        first_generated(|surface| {
            let t = surface.conditions().mean_surface_temperature().value();
            surface.state() == SurfaceState::Temperate
                && has_liquid(surface, "H2O")
                && (275.0..=305.0).contains(&t)
        })
        .expect("the sample holds a temperate body with a water sea")
    }

    /// The first airless body of the plan-14 sample that `for_body` reads, with its context and its
    /// inputs.
    fn first_airless() -> (SystemContext, BodyRecord, CoarseInputs) {
        first_generated(|surface| {
            surface.state() == SurfaceState::Airless && unread_cause(surface).is_none()
        })
        .expect("the sample holds an airless body that for_body reads")
    }

    /// Pins the inputs `inputs` that `for_body` gives the body `name` to `w`: each palette entry
    /// (its role and key, its normal albedo in B, V and R, its phase row, grain density,
    /// transition and mechanics family, and a liquid's or a melt's record: its temperature,
    /// density and basis), the ground's thermal inertia Γ and every condensate.
    fn pin_record_inputs(w: &mut GoldenWriter, name: &str, inputs: &CoarseInputs) {
        for (i, entry) in inputs.palette().entries().iter().enumerate() {
            let at = format!("{name}_palette_{i}");
            w.line(&format!(
                "{at} = {:?} {} {:?} phase row {}",
                entry.role, entry.substance, entry.mechanics, entry.phase_row
            ));
            for (band, albedo) in ["b", "v", "r"].iter().zip(entry.normal_albedo_bvr) {
                w.f64(&format!("{at}_normal_albedo_{band}"), albedo);
            }
            w.f64(&format!("{at}_grain_density"), entry.grain_density_kg_m3);
            w.f64(&format!("{at}_transition"), entry.transition.value());
            match &entry.liquid {
                Some(record) => {
                    w.f64(
                        &format!("{at}_liquid_temperature"),
                        record.temperature.value(),
                    );
                    w.f64(&format!("{at}_liquid_density"), record.density_kg_m3);
                    w.line(&format!("{at}_liquid_basis = {:?}", record.basis));
                }
                None => w.line(&format!("{at}_liquid = none")),
            }
        }
        match inputs.ground_inertia_j_per_m2_k_sqrt_s() {
            Some(inertia) => w.f64(&format!("{name}_ground_inertia"), inertia),
            None => w.line(&format!("{name}_ground_inertia = none")),
        }
        for (i, condensate) in inputs.condensates().iter().enumerate() {
            let at = format!("{name}_condensate_{i}");
            w.line(&format!(
                "{at} = {} {:?} {:?}",
                condensate.substance, condensate.phase, condensate.persistence
            ));
            w.f64(&format!("{at}_reservoir"), condensate.reservoir.value());
            w.f64(&format!("{at}_area_fraction"), condensate.area_fraction);
        }
    }

    /// Batch E's review, DE1: `for_body`'s palettes, its Γ and its condensates, pinned at header
    /// 22 (an extension, no bump) on the plan-14 sample's first Titan-like, Earth-like, magma-ocean,
    /// partial-melt and airless bodies, those its tests find.
    #[test]
    fn the_record_s_coarse_inputs_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for (kind, (ctx, record, inputs)) in [
            ("titan_like", first_titan_like()),
            ("earth_like", first_earth_like()),
            ("magma_ocean", first_magma_ocean()),
            ("partial_melt", first_partial_melt()),
            ("airless", first_airless()),
        ] {
            let id = record.index().body_id(ctx.id());
            w.line(&format!("{kind} = {id}"));
            pin_record_inputs(&mut w, kind, &inputs);
            eprintln!(
                "{kind} {id}: {:?}, T_s {:.2} K, {:.4e} Pa; liquids {:?}; ices {:?}",
                inputs.surface_state(),
                inputs.mean_surface_temperature().value(),
                inputs.surface_pressure().value(),
                inputs.liquids(),
                inputs.ices()
            );
        }
        hyperion_testkit::golden!("planetary/surface_inputs", w.as_str());
    }

    /// Batch E's review, DE2: `for_body` is independent of the order bodies are read in, the
    /// bodies of two of the sample's systems, the Titan-like body's and the partial melt's, read A
    /// then B, B then A, in a permutation and each alone; and B's, read from its system generated
    /// again alone, the same. Its palettes' floats are compared by their bits (each pinned as the
    /// golden pins it).
    #[test]
    fn for_body_does_not_depend_on_the_order_it_reads() {
        let systems = crate::planetary::system::tests::whole();
        let (a, _, _) = first_titan_like();
        let (mut b, _, _) = first_partial_melt();
        if b.id() == a.id() {
            (b, _, _) = first_earth_like();
        }
        assert_ne!(a.id(), b.id(), "two systems");
        // The records of a system's bodies `for_body` reads, each with its planet's.
        let readable = |system: &crate::planetary::PlanetarySystem, ctx: &SystemContext| {
            let snapshot = system.snapshot_at(ctx, RECORD_TIME);
            let records = snapshot.bodies();
            records
                .iter()
                .map(|record| (record, planet_of(record, records)))
                .filter(|&(record, planet)| CoarseInputs::for_body(record, planet, ctx).is_ok())
                .map(|(record, planet)| (ctx.clone(), record.clone(), planet.cloned()))
                .collect::<Vec<_>>()
        };
        let of = |ctx: &SystemContext| {
            let (_, system) = systems
                .iter()
                .find(|(c, _)| c.id() == ctx.id())
                .expect("a sample system");
            readable(system, ctx)
        };
        let (bodies_a, bodies_b) = (of(&a), of(&b));
        assert!(!bodies_a.is_empty() && !bodies_b.is_empty());
        let pinned = |(ctx, record, planet): &(SystemContext, BodyRecord, Option<BodyRecord>)| {
            let inputs = CoarseInputs::for_body(record, planet.as_ref(), ctx)
                .unwrap_or_else(|e| panic!("{:?}: {e:?}", record.index()));
            let mut w = GoldenWriter::new();
            pin_record_inputs(&mut w, "body", &inputs);
            w.finish()
        };
        let keys: Vec<_> = bodies_a.iter().chain(&bodies_b).cloned().collect();
        hyperion_testkit::order::assert_order_independent(&keys, pinned);
        // B alone: its system generated again, its bodies read with nothing read before.
        let alone = readable(&generate(crate::planetary::system::tests::SEED, &b), &b);
        assert_eq!(alone.len(), bodies_b.len());
        for (fresh, kept) in alone.iter().zip(&bodies_b) {
            assert_eq!(fresh.1.index(), kept.1.index());
            assert_eq!(pinned(fresh), pinned(kept), "{:?}", kept.1.index());
        }
        eprintln!(
            "order independence: {} bodies of {} and {} of {}",
            bodies_a.len(),
            a.id(),
            bodies_b.len(),
            b.id(),
        );
    }

    /// P14.T54.a's tests (decision-r11-palette-liquids §4): a Titan-like generated body's
    /// `ch4_c2h6_n2` sea carries its record, P14.T49.h's at the record's temperature and
    /// reference composition to the bit, every member without a source absent and every present
    /// one positive, never 0; its temperature in the solution's range, its basis at least
    /// `Derived`.
    #[test]
    fn a_titan_like_body_s_sea_carries_its_record() {
        use crate::substance::{Composition, by_key, liquid_properties};
        let (_, record, inputs) = first_titan_like();
        let sea = entry_of(&inputs, PaletteRole::Liquid, "ch4_c2h6_n2");
        let held = sea.liquid.as_ref().expect("a sea carries its record");
        let solution = by_key("ch4_c2h6_n2").unwrap();
        let resolved =
            liquid_properties(solution, held.temperature, Composition::Reference).unwrap();
        assert_eq!(held, &resolved, "{:?}", record.index());
        assert!(held.basis >= LiquidBasis::Derived, "{:?}", held.basis);
        let positive = |values: &[f64]| values.iter().all(|v| *v > 0.0);
        assert!(positive(&held.refractive_index));
        assert!(held.density_kg_m3 > 0.0);
        for bins in [held.absorption_per_m, held.scattering_per_m]
            .into_iter()
            .flatten()
        {
            assert!(positive(&bins), "{bins:?}");
        }
        for scalar in [held.surface_tension_n_per_m, held.viscosity_pa_s]
            .into_iter()
            .flatten()
        {
            assert!(scalar > 0.0, "{held:?}");
        }
        let Section::Ok(surface) = record.surface() else {
            unreachable!("the finder read its surface");
        };
        eprintln!(
            "{:?}: {:?}, T_s {:.2} K, the sea's record at {:.2} K, {:?}, absorption {}, \
             scattering {}, surface tension {:?}, viscosity {:?}",
            record.index(),
            surface.state(),
            surface.conditions().mean_surface_temperature().value(),
            held.temperature.value(),
            held.basis,
            held.absorption_per_m.is_some(),
            held.scattering_per_m.is_some(),
            held.surface_tension_n_per_m,
            held.viscosity_pa_s,
        );
    }

    /// P14.T54.a's tests: a generated Earth-like body's sea record is water's, `Measured`, at its
    /// mean surface temperature inside water's checked windows.
    #[test]
    fn an_earth_like_body_s_sea_record_is_measured() {
        let (_, record, inputs) = first_earth_like();
        let sea = entry_of(&inputs, PaletteRole::Liquid, "H2O");
        let held = sea.liquid.as_ref().expect("a sea carries its record");
        assert_eq!(
            held.basis,
            LiquidBasis::Measured,
            "{:?}: {held:?}",
            record.index()
        );
        assert_same_bits(
            held.temperature.value(),
            inputs.mean_surface_temperature().value(),
        );
        assert_eq!(
            inputs.main_liquid(),
            inputs.palette().find(PaletteRole::Liquid, sea.substance)
        );
        // A pure fluid's grain density is its saturated liquid's at the record's temperature.
        let rho = crate::substance::by_key("H2O")
            .unwrap()
            .substance()
            .grain_density(crate::substance::CondensedPhase::Liquid, held.temperature)
            .unwrap();
        assert_same_bits(sea.grain_density_kg_m3, rho.value());
    }

    /// The first magma ocean of the plan-14 sample whose crust gives a melt area, the whole surface
    /// melt, with its context and its inputs from `for_body`.
    fn first_magma_ocean() -> (SystemContext, BodyRecord, CoarseInputs) {
        first_generated(|surface| {
            surface.state() == SurfaceState::MagmaOcean
                && surface
                    .crust()
                    .ok()
                    .is_some_and(|crust| crust.melt_area_fraction() > 0.0)
        })
        .expect("the sample holds a magma ocean whose crust gives a melt area")
    }

    /// The first solid world of the plan-14 sample whose crust gives a partial melt area (a locked
    /// world's dayside pool), with its context and its inputs from `for_body`.
    fn first_partial_melt() -> (SystemContext, BodyRecord, CoarseInputs) {
        first_generated(|surface| {
            surface.state() != SurfaceState::MagmaOcean
                && surface
                    .crust()
                    .ok()
                    .is_some_and(|crust| crust.melt_area_fraction() > 0.0)
        })
        .expect("the sample holds a solid world whose crust gives a partial melt area")
    }

    /// P14.T54.a's tests: a lava world's lid crust entry carries its melt's record, at or above
    /// its solidus, at least `Derived` (a melt's members are stand-ins): a magma ocean's, the
    /// whole surface melt, and a solid world's whose crust gives a partial melt area (a locked
    /// world's dayside pool), the first of each in the sample, which holds both.
    #[test]
    fn a_lava_world_s_lid_entry_carries_a_melt_record() {
        for (_, record, inputs) in [first_magma_ocean(), first_partial_melt()] {
            let lid = inputs.palette().get(inputs.lid_entry()).unwrap();
            let melt = lid.liquid.as_ref().unwrap_or_else(|| {
                panic!(
                    "{:?}: the lid {} carries no melt",
                    record.index(),
                    lid.substance
                )
            });
            assert!(melt.temperature >= lid.transition, "{melt:?}");
            assert!(melt.basis >= LiquidBasis::Derived, "{melt:?}");
            let Section::Ok(surface) = record.surface() else {
                unreachable!("the finder read its surface");
            };
            eprintln!(
                "{:?}: {:?}, the lid {} melts over {:.3}, its melt at {:.1} K (solidus {:.1} K, \
                 T_s {:.1} K, warmest {:.1} K), {:?}",
                record.index(),
                surface.state(),
                lid.substance,
                inputs.crust().melt_area_fraction,
                melt.temperature.value(),
                lid.transition.value(),
                surface.conditions().mean_surface_temperature().value(),
                surface.conditions().climate_range().1.value(),
                melt.basis,
            );
        }
    }

    /// The recorded gaps, each answered not modelled (the lane's ruling, Batch E): a body whose
    /// palette names an ice the registry has no solid density of under 23's rules, solid argon's
    /// on the coldest airless members (P14.T49.b's `ARGON` diagram reads none; P14.T49.n's fcc
    /// density is held until 24, `under_24_s_densities_the_argon_ice_body_reads`), names the
    /// substance and the cause in its palette's refusal; and an unlit body, whose record states
    /// its mean surface temperature as its equilibrium temperature, 0 K, is not read.
    #[test]
    fn a_surface_the_registry_or_the_record_does_not_model_answers_not_modelled() {
        use crate::substance::ResolvePaletteEntryError;
        let not_modelled = Err(ReadCoarseInputsError::Section(
            SurfaceInputsError::NotModelled(RecordSection::Surface),
        ));
        let (mut argon, mut unlit) = (None, None);
        'systems: for (ctx, system) in crate::planetary::system::tests::whole() {
            let snapshot = system.snapshot_at(ctx, RECORD_TIME);
            let records = snapshot.bodies();
            for record in records {
                let Section::Ok(surface) = record.surface() else {
                    continue;
                };
                let Modelled::Ok(crust) = surface.crust() else {
                    continue;
                };
                let mean = surface.conditions().mean_surface_temperature().value();
                let read = || CoarseInputs::for_body(record, planet_of(record, records), ctx);
                if unlit.is_none() && mean <= 0.0 {
                    assert_eq!(read(), not_modelled, "{:?}", record.index());
                    unlit = Some(record.index());
                } else if argon.is_none()
                    && mean > 0.0
                    && palette::resolve(surface, crust)
                        == Err(palette::ResolveBodyPaletteError::Solid {
                            substance: key("Ar"),
                            role: PaletteRole::Ice,
                            cause: ResolvePaletteEntryError::DensityNotModelled,
                        })
                {
                    assert_eq!(read(), not_modelled, "{:?}", record.index());
                    let ar = crate::substance::by_key("Ar").unwrap().substance();
                    assert!(
                        ar.condensed_optics().is_some(),
                        "argon's optics are modelled"
                    );
                    eprintln!(
                        "{:?}: {:?} at T_s {mean:.1} K holds argon ice, whose solid density the \
                         registry has not read",
                        record.index(),
                        surface.state()
                    );
                    argon = Some(record.index());
                }
                if argon.is_some() && unlit.is_some() {
                    break 'systems;
                }
            }
        }
        assert!(argon.is_some() && unlit.is_some(), "{argon:?}, {unlit:?}");
    }

    /// P14.T49.n's densities, held until 24 (`READ_DENSITIES_VERSION`): the first body of the
    /// sample whose palette 23's rules refuse for its argon ice
    /// (`a_surface_the_registry_or_the_record_does_not_model_answers_not_modelled`'s), generated
    /// and read under 24's densities, reads into inputs, its argon ice's entry at solid argon's
    /// density (fcc, about 1.6–1.77 × 10³ kg m⁻³ below its triple point).
    #[test]
    fn under_24_s_densities_the_argon_ice_body_reads() {
        use crate::planetary::system::tests::{SEED as SYSTEMS_SEED, whole};
        use crate::substance::{ReadDensitiesRules, ResolvePaletteEntryError, with_read_densities};
        let refused = whole().iter().find_map(|(ctx, system)| {
            let snapshot = system.snapshot_at(ctx, RECORD_TIME);
            snapshot.bodies().iter().find_map(|record| {
                let surface = record.surface().ok()?;
                let Modelled::Ok(crust) = surface.crust() else {
                    return None;
                };
                let mean = surface.conditions().mean_surface_temperature().value();
                (mean > 0.0
                    && palette::resolve(surface, crust)
                        == Err(palette::ResolveBodyPaletteError::Solid {
                            substance: key("Ar"),
                            role: PaletteRole::Ice,
                            cause: ResolvePaletteEntryError::DensityNotModelled,
                        }))
                .then(|| (ctx.clone(), record.index()))
            })
        });
        let (ctx, index) = refused.expect("the sample holds an argon ice 23 refuses");
        let (mean, ice) = with_read_densities(ReadDensitiesRules::Read, || {
            let snapshot = generate(SYSTEMS_SEED, &ctx).snapshot_at(&ctx, RECORD_TIME);
            let records = snapshot.bodies();
            let record = records.iter().find(|r| r.index() == index).unwrap();
            let Section::Ok(surface) = record.surface() else {
                panic!("{index:?} has a surface under 24's densities");
            };
            let inputs = CoarseInputs::for_body(record, planet_of(record, records), &ctx)
                .unwrap_or_else(|e| panic!("{index:?} reads under 24's densities: {e:?}"));
            let ice = inputs
                .palette()
                .find(PaletteRole::Ice, key("Ar"))
                .and_then(|at| inputs.palette().get(at))
                .unwrap_or_else(|| panic!("{index:?} keeps its argon ice"))
                .clone();
            (surface.conditions().mean_surface_temperature().value(), ice)
        });
        assert!(
            (1_550.0..1_800.0).contains(&ice.grain_density_kg_m3),
            "{ice:?}"
        );
        eprintln!(
            "{index:?} at T_s {mean:.1} K under 24's densities: its argon ice's entry at {:.1} \
             kg m⁻³",
            ice.grain_density_kg_m3
        );
    }

    /// The surface tests' methane sea carries the registry's scattering, to the bit: its copy, as
    /// shortest round-trip decimals, is `liquid_properties`' b at the record's temperature
    /// (P14.T49.h; decision-r10-t10b-r09-t2e §7, condition 1).
    #[test]
    fn the_methane_sea_carries_the_registry_s_scattering() {
        use crate::substance::{Composition, by_key, liquid_properties};
        let sea = methane_sea();
        let methane = by_key("CH4").unwrap();
        let resolved = liquid_properties(methane, sea.temperature, Composition::Reference).unwrap();
        for (h, r) in sea
            .scattering_per_m
            .unwrap()
            .iter()
            .zip(&resolved.scattering_per_m.unwrap())
        {
            assert_same_bits(*h, *r);
        }
    }
}
