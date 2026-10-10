//! What the coarse pass reads of a body: [`CoarseInputs`], from plan 14's record through
//! [`CoarseInputs::for_body`] or from [`CoarseInputsBuilder`] (plan R09, Design note 3).
//!
//! [`CoarseInputs`] is the whole of what the pass reads, so that the field is a pure function of
//! the surface seed and these values (the brainstorm's "What the generator already owes us"):
//! the body's figure and gravity, plan 14's hypsometric standard deviation `σ_h`, its liquids and
//! ices by substance, its mean surface temperature with its signed contrasts, its spin and the
//! orbit that sets its seasons, the light of its hosts, its air, its surface state and material,
//! its tectonic regime with the continental fraction, its volcanism, heat flow and surface age,
//! its crater contract, its wet epoch, its climate regime, and (decision-composition §1.7, §7.4)
//! its crust and condensates by substance.
//!
//! Plan 14's record does not carry these yet: `record::Surface`, the section they arrive in
//! (P14.T48.e, with P14.T51.a's condensates and T51.c's crust from P14.T54.a), is an uninhabited
//! enum, so [`CoarseInputs::for_body`] answers [`SurfaceInputsError::NotModelled`] for every body
//! that has a solid surface, and every test, the reference worlds among them, builds its inputs
//! with [`CoarseInputsBuilder`], each value a plain argument (Design note 3, "Until an ask lands").
//! The types here are the pass's own vocabulary for those values, each named after the plan-14
//! task that will supply it; the reading of the record replaces the builder's arguments one for
//! one when the section is inhabited, and wherever the pass computes a figure plan 14 also states,
//! the result is constrained to plan 14's value.

use std::error::Error;
use std::fmt;

use hyperion_surface::craters::CraterParams;
use hyperion_surface::field::{BodyRef, ClimateModelKind};
use hyperion_surface::spheroid::Spheroid;

use crate::planetary::SystemContext;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::record::{BodyRecord, RecordSection, Section};
use crate::units::{
    Gigayears, Kelvin, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Pascals, Radians,
    Seconds, WattsPerSquareMetre, Years,
};

/// A substance of plan 14's registry, by its key: a formula in chemical case (`H2O`, `CO2`),
/// `e-` for the electron, or a lowercase name for a material with no one formula (`basalt`,
/// `mars_dust`) (decision-composition §1.1).
///
/// The record will name substances by P14.T49.a's `SubstanceId`, and the field's palette by R09.T2
/// follow-up B's `SubstanceKey`; neither exists yet, so the pass's inputs carry the key's text,
/// which the pass only passes on. [`CoarseInputsBuilder::build`] checks its length and characters
/// alone (1–16 printable ASCII bytes); the grammar is `SubstanceKey`'s to check once that type
/// exists, and this type then gives way to it (R09's Risks, "Deviations in T10, as built").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubstanceRef(&'static str);

impl SubstanceRef {
    /// The longest key, in bytes (decision-composition §1.1).
    pub const MAX_KEY_BYTES: usize = 16;

    /// The substance whose registry key is `key`.
    #[must_use]
    pub const fn new(key: &'static str) -> Self {
        Self(key)
    }

    /// The key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// Whether the key has the registry's length and characters: 1 to 16 bytes, each printable
    /// ASCII.
    #[must_use]
    fn is_well_formed(self) -> bool {
        (1..=Self::MAX_KEY_BYTES).contains(&self.0.len())
            && self.0.bytes().all(|b| b.is_ascii_graphic())
    }
}

impl fmt::Display for SubstanceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// One substance's share of the surface, as plan 14's `surface_liquids` and `surface_ices` list
/// them (P14.T24.b, by substance per decision-composition §1.10).
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaShare {
    /// The substance.
    pub substance: SubstanceRef,
    /// The share of the body's surface it covers all year, 0 to 1: the area R09.T13.d places it
    /// over, coldest cells first by their warmest month for an ice, so cover that comes and goes
    /// with the seasons is the climate's, not this share.
    pub area_fraction: f64,
}

/// One gas of the air at the surface, as P14.T24.a lists them.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasShare {
    /// The gas.
    pub substance: SubstanceRef,
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
    /// 102.947°; 109.0° on Mars, whose perihelion falls at Ls 251.0°).
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

/// A body's tectonic regime, plan 14's (P14.T24.b, with every variant the wire pins,
/// decision-composition §1.10).
///
/// A closed physics class. Heat-pipe, episodic and ice-shell worlds are produced by no rule of
/// plan 14 yet, and their morphologies are recorded for a later R09.T11–T12 (R09's Risks,
/// "Composition"); the pass names each so that none is silently treated as another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TectonicRegime {
    /// Plates in relative motion, as on Earth: the pass draws plates (R09.T11).
    MobileLid,
    /// One lid, as on Mars, Venus as usually modelled, the Moon and Mercury: volcanic provinces
    /// and their flexure, no plate boundaries (Design note 6).
    StagnantLid,
    /// Heat lost through volcanic conduits, as on Io.
    HeatPipe,
    /// A lid that overturns in episodes.
    Episodic,
    /// An ice shell over a subsurface ocean, as on Europa.
    IceShell,
}

/// The span over which the surface held liquid, plan 14's volatile history (P14.T48.b), as times
/// before the present.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetEpoch {
    /// When the epoch began, thousands of millions of years ago, at or after `end`.
    pub start: Gigayears,
    /// When it ended, thousands of millions of years ago: zero for a world wet now.
    pub end: Gigayears,
    /// The epoch in Earth-equivalent years, its length scaled by its flood frequency relative to
    /// an arid-to-semiarid Earth's: the stream-power solver's t (Design note 9, as corrected by
    /// R09.T0.b). Finite and non-negative.
    pub effective_flow: Years,
    /// The liquid inventory then, as its mass over the body's whole surface, M ÷ 4πR², finite and
    /// non-negative: what the pass places as a paleo-sea or spreads through closed basins, by the
    /// liquid's density (Design note 9). A mass, so that it needs no density to be stated.
    pub paleo_inventory: KilogramsPerSquareMetre,
}

/// How much of the climate's heat the air moves from day to night, the classifier's thermal
/// regime (P14.T48.d, after Koll 2022's redistribution factor f = 2/3 − (5/12) X ÷ (k + X) at
/// k = 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ThermalRegime {
    /// Airless-like: X below 0.087 k, so each point sits near its own radiative equilibrium.
    AirlessLike,
    /// Between the two, with a redistribution factor between 2/3 and 1/4.
    Transitional,
    /// Efficient redistribution, X above 15.7 k.
    Efficient,
}

/// What drives the climate's cycle, the classifier's forcing (P14.T48.d): from the locking state,
/// the solar day and the obliquity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Forcing {
    /// A fast rotator's seasons, its ice at the poles.
    Seasonal,
    /// A fast rotator at an obliquity of 54–126° that reached its state from a colder one, with an
    /// equatorial ice belt (the 54° of Kilic, Raible and Stocker 2017, ApJ 844, 147, which Kilic
    /// et al. 2018 reach only from a colder state; with P14.T48.b's history).
    IceBelt,
    /// A slow rotator, past the onset that rises with flux, its circulation converging on the
    /// substellar point: in Yang et al. 2014's Table 1 (sidereal rotation periods, at an orbital
    /// period of 225 d), between 8 and 16 d at 1.40 S⊕ and between 32 and 48 d at 1.92 S⊕.
    SlowRotator,
    /// A world locked to its host, in tidally locked coordinates about the substellar axis.
    Locked,
}

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
    /// The climate's condensable: the most massive condensate whose phase changes inside the
    /// climate's range, or `None` (P14.T48.d, by registry loop): water on Earth, carbon dioxide on
    /// Mars, methane on a Titan.
    pub condensable: Option<SubstanceRef>,
    /// The coarse model the regime names, which the field's header carries (Design note 8).
    pub model: ClimateModelKind,
}

/// A body's crust by substance, P14.T51.c's crust composition as the pass reads it (its redox
/// class is not read here).
///
/// The lithologies become the palette entries of the field's
/// [`Crust`](hyperion_surface::field::Crust) variants (decision-composition §1.7): R09.T12.a gives
/// `Oceanic` the secondary crust's and `Continental` the tertiary's, and which entry a stagnant
/// lid's `Lid` and its `Province` cells take is R09.T12's and follow-up B's to set.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrustInputs {
    /// The primary crust, a flotation crust from a magma ocean (`anorthosite` on the Moon), or
    /// `None`.
    pub primary: Option<SubstanceRef>,
    /// The secondary crust, from partial melting of the mantle (`basalt` by default, `H2O` on an
    /// icy body).
    pub secondary: SubstanceRef,
    /// The tertiary crust, from remelting of the secondary (`granite`, only where the continental
    /// fraction is above zero), or `None`.
    pub tertiary: Option<SubstanceRef>,
    /// The volcanic provinces' lithology.
    pub provinces: SubstanceRef,
    /// The share of the surface above the secondary crust's solidus, 0 to 1 (a locked lava
    /// world's dayside pool); R09.T13.c applies the solidus per cell.
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

/// Whether a condensate stays through the year (P14.T51.a's `seasonal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Persistence {
    /// It stays all year.
    Perennial,
    /// The warmest zone rises above its frost point, so part of it comes and goes.
    Seasonal,
}

/// One condensate at the surface, P14.T51.a's partition as the pass reads it.
///
/// Plain data with public fields: [`CoarseInputsBuilder::build`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Condensate {
    /// The substance.
    pub substance: SubstanceRef,
    /// Its phase where it lies.
    pub phase: CondensatePhase,
    /// The condensed reservoir as its mass over the body's whole surface, M ÷ 4πR² (P14.T51.a's
    /// `M_s − p_s 4πR² ÷ g`, per unit area), finite and non-negative: a mass, so that substances of
    /// different densities sort as plan 14 sorts them.
    pub reservoir: KilogramsPerSquareMetre,
    /// The share of the surface it covers all year, 0 to 1, as [`AreaShare::area_fraction`].
    pub area_fraction: f64,
    /// Whether it stays through the year.
    pub persistence: Persistence,
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
/// the crate's `testing` feature) or by [`CoarseInputs::for_body`] from plan 14's record, once its
/// surface section carries values. [`coarse_pass`](super::coarse_pass)'s example builds one.
#[derive(Debug, Clone, PartialEq)]
pub struct CoarseInputs {
    body: BodyRef,
    figure: Spheroid,
    gravity: MetresPerSecondSquared,
    sigma_h: Metres,
    liquids: Vec<AreaShare>,
    ices: Vec<AreaShare>,
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
    craters: CraterParams,
    wet_epoch: Option<WetEpoch>,
    climate: ClimateRegime,
    crust: CrustInputs,
    condensates: Vec<Condensate>,
}

/// Why [`CoarseInputs::for_body`] cannot give a body's inputs.
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

impl CoarseInputs {
    /// A builder with no value set.
    #[must_use]
    pub fn builder() -> CoarseInputsBuilder {
        CoarseInputsBuilder::default()
    }

    /// The inputs of the body of `record`, in the system of `ctx`, from the record's sections
    /// (Design note 3).
    ///
    /// The surface section is decided from its tag alone, never from the body's class: a body
    /// whose surface is [`Section::NotApplicable`] has no solid surface, whatever its class
    /// (P14.T48.e's gas-envelope split, decision-p14-t35e-wire). Once the section is inhabited,
    /// the inputs come from it with the record's `bulk` (gravity), `figure` (the spheroid),
    /// `rotation` (the spin) and `orbit` sections and the hosts of `ctx` (their light over the
    /// orbit, which P14.T12's `Illumination` gives only as an orbit average). Until then
    /// `record::Surface` has no value, so no record gets past its tag.
    ///
    /// # Errors
    ///
    /// - [`SurfaceInputsError::NoSolidSurface`] for a surface section that is
    ///   [`Section::NotApplicable`];
    /// - [`SurfaceInputsError::NotModelled`] naming [`RecordSection::Surface`] for one that is
    ///   [`Section::NotModelled`], which is every surface section of this generator version;
    /// - [`SurfaceInputsError::NotResolved`] naming [`RecordSection::Surface`] for one a degraded
    ///   record withholds.
    pub fn for_body(record: &BodyRecord, ctx: &SystemContext) -> Result<Self, SurfaceInputsError> {
        let surface = match record.surface() {
            Section::Ok(surface) => surface,
            Section::NotApplicable => return Err(SurfaceInputsError::NoSolidSurface),
            Section::NotModelled => {
                return Err(SurfaceInputsError::NotModelled(RecordSection::Surface));
            }
            Section::NotResolved => {
                return Err(SurfaceInputsError::NotResolved(RecordSection::Surface));
            }
        };
        // The hosts are read with the surface section's members, which P14.T48.e and P14.T54.a
        // define; until then the section is uninhabited and nothing reaches them.
        let _ = ctx;
        match *surface {}
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
    /// `surface_ices`).
    #[must_use]
    pub fn ices(&self) -> &[AreaShare] {
        &self.ices
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
    /// 300). R09.T13.c–d decide how it is imposed (R09's Risks, "Deviations in T10, as built").
    #[must_use]
    pub const fn mean_surface_temperature(&self) -> Kelvin {
        self.mean_surface_temperature
    }

    /// The equator–pole contrast T(0°) − T(90°), kelvin, signed: a warm pole is negative
    /// (P14.T48.e). With the annual field T₀ + T₂ P₂(sin φ), the contrast is −(3/2) T₂, so the
    /// climate step sets the P₂ coefficient to −⅔ of it (Design note 8).
    #[must_use]
    pub const fn equator_pole_contrast_k(&self) -> f64 {
        self.equator_pole_contrast_k
    }

    /// The day–night contrast, kelvin, signed, about P14.T14's substellar axis on a locked world
    /// (P14.T48.e), which the climate step imposes there by P₁(cos γ) (Design note 8).
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

    /// The months of the field's year: 1 for a world with no seasonal forcing, a world locked 1:1
    /// to its seasonal host on a circular orbit, a circular orbit with no tilt, or no orbit at
    /// all; 12 otherwise, each a twelfth of the seasonal orbit in eccentric anomaly
    /// (`decision-r09-t2.md` item 1, Design note 8).
    #[must_use]
    pub fn months(&self) -> u8 {
        let Some(orbit) = self.seasonal_orbit else {
            return 1;
        };
        let circular = orbit.eccentricity <= 0.0;
        let obliquity = self.spin.obliquity.value();
        let untilted = obliquity <= 0.0 || obliquity >= core::f64::consts::PI;
        if circular && (self.spin.solar_day.is_none() || untilted) {
            1
        } else {
            12
        }
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
    /// A substance's key is not 1–16 printable ASCII bytes.
    SubstanceKey(SubstanceRef),
    /// A list names a substance twice (a condensate twice in one phase).
    Duplicate {
        /// The list's name.
        part: &'static str,
        /// The substance.
        substance: SubstanceRef,
    },
    /// A list's shares add up to more than the whole.
    SharesExceedWhole {
        /// The list's name.
        part: &'static str,
        /// Their sum.
        total: f64,
    },
    /// More condensates than plan 14 lists.
    TooManyCondensates(usize),
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
            Self::SubstanceKey(key) => write!(
                f,
                "substance key {:?} is not 1–16 printable ASCII bytes",
                key.as_str()
            ),
            Self::Duplicate { part, substance } => {
                write!(f, "the {part} name {substance} twice")
            }
            Self::SharesExceedWhole { part, total } => {
                write!(
                    f,
                    "the {part}' shares add up to {total}, more than the whole"
                )
            }
            Self::TooManyCondensates(count) => write!(
                f,
                "{count} condensates are more than the {MAX_CONDENSATES} plan 14 lists"
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
        }
    }
}

impl Error for BuildCoarseInputsError {}

/// Builds [`CoarseInputs`] from plain values, each the plan-14 figure it names (Design note 3,
/// "Until an ask lands").
///
/// Every value is required but the body (by default [`BodyRef::default`]) and the lists and
/// optional parts, whose default is none: no liquid, ice, gas or condensate, no seasonal orbit
/// and no wet epoch.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoarseInputsBuilder {
    body: BodyRef,
    figure: Option<Spheroid>,
    gravity: Option<MetresPerSecondSquared>,
    sigma_h: Option<Metres>,
    liquids: Vec<AreaShare>,
    ices: Vec<AreaShare>,
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
    craters: Option<CraterParams>,
    wet_epoch: Option<WetEpoch>,
    climate: Option<ClimateRegime>,
    crust: Option<CrustInputs>,
    condensates: Vec<Condensate>,
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

/// Refuses a value that is negative or not finite.
fn check_non_negative(part: &'static str, value: f64) -> Result<(), BuildCoarseInputsError> {
    check(part, value, value >= 0.0)
}

/// Refuses a value that is not finite and positive.
fn check_positive(part: &'static str, value: f64) -> Result<(), BuildCoarseInputsError> {
    check(part, value, value > 0.0)
}

/// Refuses a substance whose key is not well formed.
fn check_key(substance: SubstanceRef) -> Result<(), BuildCoarseInputsError> {
    if substance.is_well_formed() {
        Ok(())
    } else {
        Err(BuildCoarseInputsError::SubstanceKey(substance))
    }
}

/// Validates a list of area shares and sorts it largest first, ties by key.
fn area_shares(
    part: &'static str,
    mut shares: Vec<AreaShare>,
) -> Result<Vec<AreaShare>, BuildCoarseInputsError> {
    for share in &shares {
        check_key(share.substance)?;
        check_fraction(part, share.area_fraction)?;
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

/// Refuses a list in which a key occurs twice: each item is a substance and a second key that
/// may tell two items of one substance apart.
fn refuse_duplicates<K: Ord>(
    part: &'static str,
    items: impl IntoIterator<Item = (SubstanceRef, K)>,
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

    /// The surface age, finite and non-negative.
    #[must_use]
    pub const fn surface_age(mut self, age: Gigayears) -> Self {
        self.surface_age = Some(age);
        self
    }

    /// The crater contract, whose gravity must be the body's.
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

    /// The inputs, validated, with every list in its documented order.
    ///
    /// # Errors
    ///
    /// [`BuildCoarseInputsError::Missing`] for a required value not given, and otherwise the
    /// first rule broken, as each value's setter states it: a value out of range or not finite, a
    /// figure that is not a spheroid, a gas-envelope surface, a malformed key, a substance named
    /// twice, shares above the whole, too many condensates, a wet epoch that ends before it
    /// starts, or a crater contract of another gravity.
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
        let craters = required(self.craters, "crater contract")?;
        if (craters.gravity().value() / gravity.value() - 1.0).abs() > GRAVITY_TOLERANCE {
            return Err(BuildCoarseInputsError::CraterGravity {
                body: gravity,
                craters: craters.gravity(),
            });
        }
        if let Some(epoch) = &self.wet_epoch {
            check_wet_epoch(epoch)?;
        }
        let climate = required(self.climate, "climate regime")?;
        if let Some(condensable) = climate.condensable {
            check_key(condensable)?;
        }
        let crust = required(self.crust, "crust")?;
        check_crust(&crust)?;
        let condensates = condensates(self.condensates)?;
        Ok(CoarseInputs {
            body: self.body,
            figure,
            gravity,
            sigma_h,
            liquids,
            ices,
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
            craters,
            wet_epoch: self.wet_epoch,
            climate,
            crust,
            condensates,
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
    check_non_negative("wet epoch's paleo-inventory", epoch.paleo_inventory.value())
}

/// Refuses a crust with a malformed key or a melt share outside 0 to 1.
fn check_crust(crust: &CrustInputs) -> Result<(), BuildCoarseInputsError> {
    for substance in [
        crust.primary,
        Some(crust.secondary),
        crust.tertiary,
        Some(crust.provinces),
    ]
    .into_iter()
    .flatten()
    {
        check_key(substance)?;
    }
    check_fraction("melt area fraction", crust.melt_area_fraction)
}

/// Validates the gases and sorts them largest first, ties by key.
fn gas_shares(mut gases: Vec<GasShare>) -> Result<Vec<GasShare>, BuildCoarseInputsError> {
    for gas in &gases {
        check_key(gas.substance)?;
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
        check_key(c.substance)?;
        check_non_negative("condensate's reservoir", c.reservoir.value())?;
        check_fraction("condensate's area fraction", c.area_fraction)?;
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
mod tests {
    use super::super::reference;
    use super::*;
    use crate::Seed;
    use crate::galaxy::placement::CellKey;
    use crate::id::Layer;
    use crate::planetary::derive::PlanetClass;
    use crate::planetary::generate;
    use crate::planetary::record::DetailLevel;
    use crate::planetary::testing::synthetic_star;
    use crate::time::UniverseTime;
    use crate::units::{Dex, SolarMasses};

    /// The reference Earth's builder values, as a builder to vary.
    fn earth_builder() -> CoarseInputsBuilder {
        let earth = reference::earth_like();
        CoarseInputs::builder()
            .body(earth.body())
            .figure(earth.figure())
            .gravity(earth.gravity())
            .sigma_h(earth.sigma_h())
            .liquids(earth.liquids().iter().copied())
            .ices(earth.ices().iter().copied())
            .mean_surface_temperature(earth.mean_surface_temperature())
            .equator_pole_contrast_k(earth.equator_pole_contrast_k())
            .day_night_contrast_k(earth.day_night_contrast_k())
            .spin(*earth.spin())
            .host_flux(earth.host_flux())
            .seasonal_orbit(*earth.seasonal_orbit().unwrap())
            .surface_pressure(earth.surface_pressure())
            .gases(earth.gases().iter().copied())
            .surface(earth.surface_state(), earth.surface_material())
            .tectonics(earth.tectonics(), earth.continental_fraction())
            .volcanism(earth.volcanism())
            .heat_flow(earth.heat_flow())
            .surface_age(earth.surface_age())
            .craters(*earth.craters())
            .wet_epoch(*earth.wet_epoch().unwrap())
            .climate(*earth.climate())
            .crust(*earth.crust())
            .condensates(earth.condensates().iter().copied())
    }

    #[test]
    fn the_builder_gives_back_what_it_was_given() {
        assert_eq!(earth_builder().build().unwrap(), reference::earth_like());
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
            start: Gigayears::new(1.0),
            end: Gigayears::new(2.0),
            effective_flow: Years::new(1.0),
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
    }

    #[test]
    fn the_builder_refuses_bad_substances_and_shares() {
        let refused = |builder: CoarseInputsBuilder| builder.build().unwrap_err();
        let share = |key, area_fraction| AreaShare {
            substance: SubstanceRef::new(key),
            area_fraction,
        };
        assert_eq!(
            refused(earth_builder().liquids([share("", 0.1)])),
            BuildCoarseInputsError::SubstanceKey(SubstanceRef::new(""))
        );
        assert_eq!(
            refused(earth_builder().liquids([share("a_name_of_17_char", 0.1)])),
            BuildCoarseInputsError::SubstanceKey(SubstanceRef::new("a_name_of_17_char"))
        );
        assert_eq!(
            refused(earth_builder().ices([share("H2O", 0.1), share("H2O", 0.2)])),
            BuildCoarseInputsError::Duplicate {
                part: "ices",
                substance: SubstanceRef::new("H2O")
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
            substance: SubstanceRef::new("H2O"),
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
                substance: SubstanceRef::new("H2O")
            }
        );
        let nine =
            ["H2O", "CO2", "N2", "CH4", "NH3", "Ar", "CO", "SO2", "H2S"].map(|key| Condensate {
                substance: SubstanceRef::new(key),
                ..water(CondensatePhase::Solid)
            });
        assert_eq!(
            refused(earth_builder().condensates(nine)),
            BuildCoarseInputsError::TooManyCondensates(9)
        );
    }

    #[test]
    fn the_builder_refuses_signed_zeros_angles_gases_and_keys_out_of_place() {
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
        let gas = |key, mole_fraction| GasShare {
            substance: SubstanceRef::new(key),
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
        let mut climate = *reference::earth_like().climate();
        climate.condensable = Some(SubstanceRef::new("water vapour"));
        assert_eq!(
            refused(earth_builder().climate(climate)),
            BuildCoarseInputsError::SubstanceKey(SubstanceRef::new("water vapour"))
        );
        let mut crust = *reference::earth_like().crust();
        crust.tertiary = Some(SubstanceRef::new(""));
        assert_eq!(
            refused(earth_builder().crust(crust)),
            BuildCoarseInputsError::SubstanceKey(SubstanceRef::new(""))
        );
    }

    #[test]
    fn errors_say_what_was_refused() {
        assert_eq!(
            BuildCoarseInputsError::SubstanceKey(SubstanceRef::new("")).to_string(),
            "substance key \"\" is not 1–16 printable ASCII bytes"
        );
        assert_eq!(
            BuildCoarseInputsError::Missing("crust").to_string(),
            "the coarse inputs lack the crust"
        );
        assert_eq!(
            BuildCoarseInputsError::Duplicate {
                part: "ices",
                substance: SubstanceRef::new("H2O")
            }
            .to_string(),
            "the ices name H2O twice"
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

    #[test]
    fn lists_are_sorted_largest_first_ties_by_key() {
        let gas = |key, mole_fraction| GasShare {
            substance: SubstanceRef::new(key),
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
        // Locked 1:1 to its host on a circular orbit: one month, whatever the tilt.
        assert_eq!(
            months(
                earth_builder()
                    .seasonal_orbit(orbit(0.0))
                    .spin(spin(0.3, None))
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
    /// of a gas giant, from the first of Sun-like systems 1, 2, … that hold each.
    fn rocky_and_giant() -> [(SystemContext, BodyRecord); 2] {
        let (mut rocky, mut giant) = (None, None);
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
                    _ => {}
                }
            }
            if let (Some(r), Some(g)) = (&rocky, &giant) {
                return [r.clone(), g.clone()];
            }
        }
        panic!("63 Sun-like systems hold no rocky planet and gas giant");
    }

    #[test]
    fn a_generated_rocky_body_is_not_modelled_and_a_giant_has_no_solid_surface() {
        let [(rocky_ctx, rocky), (giant_ctx, giant)] = rocky_and_giant();
        assert_eq!(
            CoarseInputs::for_body(&rocky, &rocky_ctx),
            Err(SurfaceInputsError::NotModelled(RecordSection::Surface))
        );
        assert_eq!(
            CoarseInputs::for_body(&giant, &giant_ctx),
            Err(SurfaceInputsError::NoSolidSurface)
        );
        // A record degraded below the surface section's level withholds it.
        assert_eq!(
            CoarseInputs::for_body(&rocky.degrade(DetailLevel::Bulk), &rocky_ctx),
            Err(SurfaceInputsError::NotResolved(RecordSection::Surface))
        );
        assert_eq!(
            SurfaceInputsError::NotModelled(RecordSection::Surface).to_string(),
            "the body's surface section is not modelled"
        );
    }
}
