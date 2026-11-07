//! What T13.c's passes read of a body's surface beyond [`AtmosphereInputs`], and the area-mix Bond
//! albedo over it (P14.T51.a–b; [`area_mix`]).
//!
//! A pass reads its surface through a seam, [`PassSurface`], in one of two climates
//! ([`PassClimate`]):
//!
//! - **The body's own** ([`PassClimate::Spun`], P14.T54.a): a body whose system holds its rotation
//!   law takes P14.T24.a's range under its spin at each greenhouse step, at θ from that step's own
//!   partition, over the basins, the water's surface share and the heat flow of the context it
//!   reads ([`PassContext`]), the previous pass's figures. Its published partition and
//!   condensates are the record's computation to the bit, over the record's conditions of its air
//!   and their one cold profile ([`partition_over`]; decision-p14-t54a-passes, "Cost escalation
//!   (after Batch B)", rule (f)): the record reproduces them from the published pass's context.
//! - **A climate given** ([`PassClimate::Given`]): the passes' stand-in for a body with no rotation
//!   law, labelled, a climate with no contrast at each step's mean ([`PassSurface::stand_in`]),
//!   whose cover is the stand-in's: its cold area linear in area across the pass's climate
//!   ([`linear_cold_area`]), and its water's cover from the pass's own water
//!   ([`pass_water_cover`]); the tests give it T24.a's spans.
//!
//! **The regolith** is the pass's own lid's row, under the body's own climate (P14.T54.a;
//! decision-pre-23-review item 17): each pass takes its crust's lid (P14.T51.c's
//! `CrustBasis::lid_at`, the basis built once a body) at its own state, mean surface temperature
//! (the record's mean `T_s` to the bit) and the warmest of its unwidened range, under the surface
//! age and tide of the context it reads, and its area mix reads that lid's row
//! ([`Regolith::with_lid`]); the pass gives its lid with its atmosphere, which the next pass's
//! ground reads ([`PassContext::lid`]), and the published pass's is the record's crust. Where no
//! lid is known (a first pass, under C₀, which holds no surface age; a body with no crust; the
//! passes' stand-in, a climate given) the material's rule stands ([`Regolith::of_material`]): an
//! airless rocky body takes the Moon's 0.11, any other a row by the material of the bulk its air
//! reads. Its light is the body's hosts', built once per body before its passes, or for a single
//! pass a host at the hottest host's temperature ([`PassLight`]).
//!
//! **The weathered regolith** (P14.T54.b.e, from generator version 24):
//! under the body's own climate each pass takes its maturity from the context it reads, P14.T51.d's
//! law over that context's field, surface age, resurfacing time and heat flow, its own air and lid,
//! and what its record gives once a body ([`PassWeathering`]), before it publishes its condensates
//! ([`pass_maturity`]); its regolith is its lid's row weathered at that maturity ([`pass_regolith`],
//! the one function its area mix and its bare regolith read). After it publishes, the modifiers
//! over its condensates and its water give `ferric`'s deposit, which joins the area mix. The
//! published pass's parts ride to the record ([`PassWeathered`]), whose modifiers read them, so
//! that they are that pass's to the bit. Where a pass takes no maturity, its regolith is the
//! stand-in's, with why ([`NoMaturity`]).
//!
//! **The cold traps** (P14.T54.b.f, from generator version 24): under the body's own climate a pass
//! that qualifies (a zonal profile, airless, its air not collisional) reads a cold-trap field of
//! its own inputs after its regolith and before it publishes: the derivation's sky, the heat flow,
//! crater contract and crust of the context it reads, its weathered regolith's Bond albedo,
//! airless, and its partition's condensables ([`SpunColdTraps`], `cold_traps::FieldInputs`), reused
//! from the derivation's one slot where they are the same bits. It publishes over it
//! (`cold_traps::publish_over`), and its field rides to the record (`cold_traps::PassColdTraps`).
//! Such a pass reads its conditions even where nothing in its column condenses, so that the
//! delivered water and carbon dioxide traps are its own (L2 closed, decision-p14-t54b-f-coldtraps
//! §2 (d)). A pass that reads no field publishes P14.T51.a's, with why (`cold_traps::NoColdTraps`).

use super::{
    Atmosphere, AtmosphereInputs, ClimateOf, Gas, PartialPressures, PresentClimate, PresentFigures,
    PublishedState, Rotation, RunawayDeck, SurfaceMaterial, SurfaceState, climate_class,
    published_ice_cover, state_to_carry,
};
use crate::planetary::chem::partition::{
    Basins, ClimateSpan, CondensatePhase, Condensates, Cover, CrustWater, Ground, Partition,
    WaterCover, partition,
};
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::derive::pass_record::{OpenWater, PassContext, RecordSpin, WaterGate};
use crate::planetary::derive::photometry::area_mix::{
    self, ColourBand, HostShade, IcyResurfacing, LidWeathering, Regolith, VolatileFrost,
};
use crate::planetary::derive::photometry::envelope_bond;
use crate::planetary::derive::photometry::{Bands, DiscColour, SurfaceColourRules};
use crate::planetary::hooks::cold_traps::{
    self, ColdTrapBody, ColdTrapSky, FieldInputs, FieldSlot, NoColdTraps, PassColdTraps, PassTraps,
    is_collisional, qualifies_by,
};
use crate::planetary::hooks::crust::{CrustBasis, CrustLid, LidAt};
use crate::planetary::hooks::figures::{
    Crust, DarkLag, FieldBody, PartitionGround, PartitionOver, PassFigures, WetEpoch, cover, lag,
    partition_over,
};
use crate::planetary::hooks::modifiers::{
    DeriveModifiersError, HeliocentricOrbit, ModifierHost, ModifierParts, Primary,
    SurfaceModifiers, SurfaceWeathering, WaterHistory, oxidisable, surface_weathering,
};
use crate::planetary::hooks::surface::{
    ColdProfile, OceanWeight, SurfaceConditions, SurfaceInputs, night_floor,
    published_surface_pressure, rotates,
};
use crate::planetary::moons::tides::radiating_surface_temperature;
use crate::substance::{SubstanceId, ice_ih_density};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{
    Kelvin, Kilograms, KilogramsPerSquareMetre, MetresPerSecondSquared, WattsPerSquareMetre,
};

#[cfg(doc)]
use super::atmosphere_under;

#[cfg(test)]
mod tests;

/// The regolith of a body whose bulk is of a material: the pass's regolith provider (P14.T51.b;
/// [`PassSurface::regolith`]).
pub(crate) type RegolithOf<'a> = dyn Fn(SurfaceMaterial) -> Regolith + 'a;

/// The light a pass's area mix reads (P14.T51.b; [`PassSurface::light`]).
#[derive(Debug, Clone, Copy)]
pub(crate) enum PassLight<'a> {
    /// A host at the hottest host's effective temperature ([`AtmosphereInputs::hottest_host`]),
    /// built when the pass reads it: the stand-in of a single pass with no shade of its own (the
    /// public [`atmosphere`](super::atmosphere)).
    HottestHost,
    /// The light of the body's hosts, built once per body before its passes.
    Shade(&'a HostShade),
}

/// What [`atmosphere_under`] reads of a body's surface beyond [`AtmosphereInputs`] (P14.T51.a–b,
/// P14.T54.a): the climate its partition reads, the ground its liquids lie on, the light of its
/// hosts and its regolith. A body's passes build one for each context they read, the stand-in's
/// once before the first.
#[derive(Clone, Copy)]
pub(crate) struct PassSurface<'a> {
    /// The climate the condensate partition reads at each step and that its published condensates
    /// are solved and covered over.
    pub(crate) climate: PassClimate<'a>,
    /// The basins the liquids fill (P14.T24.b's hypsometry): the context's, the previous pass's
    /// figures', from P14.T54.a, and none known in the stand-in.
    pub(crate) basins: Basins,
    /// The share of the condensed water at the surface (decision-p14-t51a-part2 §4.2): the
    /// context's, and all of it in the stand-in.
    pub(crate) water_at_surface: f64,
    /// The light of the body's hosts.
    pub(crate) light: PassLight<'a>,
    /// The pass's regolith provider where its lid is not known, on the material of the bulk its
    /// air reads ([`Regolith::of_material`], which takes the stand-ins of [`Regolith::bond`]): a
    /// pass under the body's own climate that takes a lid reads its row instead
    /// ([`Regolith::with_lid`]), weathered at its maturity under the weathered area mix
    /// (P14.T54.b.e; [`Regolith::weathered_lid`], [`pass_regolith`]).
    pub(crate) regolith: &'a RegolithOf<'a>,
    /// Whether a runaway's cloud is Venus's deck or, solved without it, the water cloud's
    /// (P14.T53.b part 1 (b)): [`RunawayDeck::Kept`] from both constructors, and
    /// [`RunawayDeck::Deckless`] for the passes of a runaway solved again without the deck
    /// ([`with_deck`](Self::with_deck)).
    pub(crate) deck: RunawayDeck,
    /// The state the pass publishes (P14.T53.b part 1 (c)): [`PublishedState::Climate`] from both
    /// constructors, and [`PublishedState::Hothouse`] for the passes of a body solved again as a
    /// hothouse between the limits ([`with_published`](Self::with_published)).
    pub(crate) published: PublishedState,
    /// The state the previous pass carried by P14.T53.e's rules 5–7, which this pass publishes and
    /// its state-keyed branches read in place of its climate's class (`escaping`'s
    /// `present_state`): `None` from the stand-in's constructor, the context's under the body's
    /// own climate ([`PassContext::present`]), and `None` there too at the first pass and after a
    /// pass whose state rules 1–4 decided or whose figures agreed with its class.
    pub(crate) present: Option<PresentClimate>,
}

/// The climate a pass reads ([`PassSurface::climate`]).
#[derive(Clone, Copy)]
pub(crate) enum PassClimate<'a> {
    /// A climate given at each step, from the step's mean surface temperature and the previous
    /// step's partial pressures, with the stand-in's cover over it: the cold area linear across the
    /// pass's final climate ([`linear_cold_area`]) and water's cover from the pass's own water
    /// ([`pass_water_cover`]). The passes' stand-in gives one with no contrast
    /// ([`PassSurface::stand_in`]); the tests give T24.a's spans.
    ///
    /// It must read nothing but those two: the greenhouse's bit-exact exit ([`ClimateOf`]) holds
    /// only while it is a function of them alone.
    Given(&'a ClimateOf<'a>),
    /// The body's own under its spin ([`SpunClimate`], P14.T54.a).
    Spun(&'a SpunClimate<'a>),
}

/// The body's own climate under its spin, over the context a pass reads (P14.T54.a;
/// decision-p14-t54a-passes §3.1 and rule (f)).
///
/// - **Each greenhouse step's climate** is P14.T24.a's range ([`step_climate`]) of the step's
///   column under the record's spin, gravity, radius and material, over a night floored at the
///   context's heat flow, at θ from the step's own partition: the potential ocean the step's
///   condensed water fills of the context's basins ([`PassContext::ocean_weight`]), gated by the
///   climate's division (step 3's state), 0 at the first step, whose column is the whole airborne
///   inventory. The body is fixed within the pass, so a step's climate is a function of the step's
///   temperature and partial pressures alone, which the greenhouse's bit-exact exit needs.
/// - **The published partition and condensates** are the record's computation: the conditions of
///   the pass's air under that spin, at the context's heat flow and the θ of its air, its ground's
///   inertia at the context's lid ([`SurfaceConditions::derive`]), the partition over their
///   climate on the context's ground and their one cold profile ([`partition_over`]), with no ice
///   bulk's crust listed ([`CrustWater::Unlisted`]: the area mix reads it as the regolith). A
///   column with nothing that condenses reads no climate there (lever (p), L2): its partition is
///   solved over the mean's isothermal span, which no part of it reads, and nothing of the cover
///   is read; but not a pass that can qualify for the cold traps, which reads its conditions all
///   the same (P14.T54.b.f).
/// - **The pass's lid** is its crust's basis's at the pass's climate ([`spun_lid`]): its regolith
///   is that lid's row.
///
/// [`step_climate`]: crate::planetary::hooks::surface::step_climate
#[derive(Clone, Copy)]
pub(crate) struct SpunClimate<'a> {
    /// The record's spin, insolation table and bulk.
    pub(crate) record: RecordSpin<'a>,
    /// The hosts' flux at the body, orbit-averaged, which its conditions state.
    pub(crate) stellar_flux: WattsPerSquareMetre,
    /// Its X-ray and ultraviolet flux, which its conditions class.
    pub(crate) xuv_flux: WattsPerSquareMetre,
    /// The context the pass reads.
    pub(crate) context: PassContext,
    /// Its volatile inventory's water (P14.T13.a), which its surface water reads.
    pub(crate) inventory_water: Kilograms,
    /// Its crust's basis (P14.T51.c's first stage), once a body before its passes: `None` for a
    /// body with no crust, whose passes take no lid.
    pub(crate) basis: Option<CrustBasis>,
    /// What its passes' maturity reads beyond each pass, once a body before them (P14.T54.b.e;
    /// [`PassWeathering`]), or why it takes none.
    pub(crate) weathering: Result<PassWeathering<'a>, NoMaturity>,
    /// What its passes' cold traps read beyond each pass, once a derivation before them
    /// (P14.T54.b.f): the derivation's sky and its one field slot, or why they read none (no
    /// seasonal orbit).
    pub(crate) cold_traps: Result<SpunColdTraps<'a>, NoColdTraps>,
}

/// What a body's passes' cold traps read beyond each pass, once a derivation before them
/// (P14.T54.b.f; decision-p14-t54b-f-coldtraps §2 (a) and §4): the derivation's sky, built once
/// from its record's spin, seasonal orbit and brightest star, and the one slot its passes' fields
/// are kept in, a local of the derivation.
#[derive(Clone, Copy)]
pub(crate) struct SpunColdTraps<'a> {
    /// The sky and the exposed fraction, as the record's cold traps read them.
    pub(crate) sky: ColdTrapSky,
    /// The derivation's slot.
    pub(crate) slot: &'a FieldSlot,
}

/// What a body's passes' maturity reads of its record beyond each pass, once a body before them
/// (P14.T54.b.e; `PassRecordOf::weathering`): the record's modifiers read the same parts of its
/// system (`system::modifiers`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WeatheringCase {
    /// Its host star's wind and light at the time: the zone's brightest star's radius and X-ray and
    /// ultraviolet luminosity at the system's age, and the seasonal orbit's G M.
    pub(crate) host: ModifierHost,
    /// Its orbit about its host, a moon's planet's: the seasonal orbit's a and e.
    pub(crate) orbit: HeliocentricOrbit,
    /// What it orbits: its star, or a moon's planet's magnetosphere from that planet's record.
    pub(crate) primary: Primary,
    /// What its field's dynamo law reads beyond the figures' first stage of the context a pass
    /// reads.
    pub(crate) field: FieldBody,
}

/// Why a pass takes no maturity, so that its regolith is the stand-in's (P14.T54.b.e), each
/// labelled and counted (the weathered area mix's census).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum NoMaturity {
    /// The passes' stand-in, with no rotation law, or a pass whose area mix reads no regolith (a
    /// gas envelope, a magma ocean, a body of no mass or radius).
    NoRegolith,
    /// No star of its zone at the time, or no orbit about its host then.
    NoHost,
    /// A moon whose planet's record gives no field (`system::modifiers::planet_field`).
    NoPlanetField,
    /// The context the pass read holds no figures: C₀, or a first stage refused.
    NoFigures,
    /// The pass took no lid: a body with no crust's basis.
    NoLid,
    /// The modifiers refused the pass's parts.
    Refused(DeriveModifiersError),
}

impl NoMaturity {
    /// The label a reader states.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P14.T54.b.l's bless note and census state the labels"
        )
    )]
    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::NoRegolith => {
                "no regolith in the area mix (the passes' stand-in, a gas envelope, a \
                 magma ocean)"
            }
            Self::NoHost => "no star of its zone, or no orbit about it, at the time",
            Self::NoPlanetField => "a moon whose planet's record gives no field",
            Self::NoFigures => "the context the pass read holds no figures (C0, a stage refused)",
            Self::NoLid => "the pass took no lid (no crust's basis)",
            Self::Refused(_) => "the modifiers refused the pass's parts",
        }
    }
}

/// A pass's wet epoch of its air at its equilibrium temperature with no liquid water at its surface
/// (`PassRecordOf::wet_epoch`), which an oxidised surface's deposit reads.
pub(crate) type WetEpochOf<'a> = dyn Fn(&Atmosphere, Kelvin) -> Option<WetEpoch> + 'a;

/// What a body's passes' maturity reads of the body beyond each pass, once a body before them
/// (P14.T54.b.e): its record's case ([`WeatheringCase`]), its record's bulk's gravity, its mantle's
/// sulphur (P14.T50.a's element inventory's, the record's) and its wet epoch at a pass.
#[derive(Clone, Copy)]
pub(crate) struct PassWeathering<'a> {
    /// Its record's case.
    pub(crate) case: WeatheringCase,
    /// Its record's bulk's mean surface gravity ([`RecordSpin`]'s).
    pub(crate) gravity: MetresPerSecondSquared,
    /// Its mantle's sulphur, which an Io's deposits read.
    pub(crate) mantle_sulphur: Kilograms,
    /// Its wet epoch at a pass, read only where an oxidised deposit's other conditions hold
    /// ([`oxidisable`]).
    pub(crate) wet_epoch: &'a WetEpochOf<'a>,
}

/// What a pass's maturity gave (P14.T54.b.e), which the published pass's carries to the record
/// (rule (f): the record's modifiers read its context's parts, so that they are its own to the
/// bit): its modifiers' parts, its water, its weathering and its weathered regolith.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PassWeathered {
    parts: ModifierParts,
    water: WaterHistory,
    weathering: SurfaceWeathering,
    regolith: Regolith,
}

impl PassWeathered {
    /// Its modifiers' parts: its record's case, its context's field, surface age, resurfacing
    /// time and heat flow, and its own air, lid and sulphur.
    #[must_use]
    pub(crate) const fn parts(&self) -> &ModifierParts {
        &self.parts
    }

    /// Its water: wet now by its published condensates' liquid water, else dry since its wet
    /// epoch where an oxidised deposit reads it, else never wet.
    #[must_use]
    pub(crate) const fn water(&self) -> WaterHistory {
        self.water
    }

    /// Its maturity m, dimensionless in [+0, 1].
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the census and the record's tests read it")
    )]
    #[must_use]
    pub(crate) const fn maturity(&self) -> f64 {
        self.weathering.maturity()
    }

    /// Its weathering rate's two terms.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the census and the record's tests read it")
    )]
    #[must_use]
    pub(crate) const fn weathering(&self) -> SurfaceWeathering {
        self.weathering
    }

    /// The regolith its area mix read ([`pass_regolith`] at its maturity).
    #[must_use]
    pub(crate) const fn regolith(&self) -> Regolith {
        self.regolith
    }
}

/// What a pass's area mix gives beside its albedo: its maturity's outcome ([`PassWeathered`]), or
/// why it took none.
pub(crate) type PassMaturity = Result<PassWeathered, NoMaturity>;

impl<'a> PassSurface<'a> {
    /// The passes' stand-in surface, under the light `light`: a climate with no contrast at each
    /// step's mean ([`ClimateSpan::isothermal`]), no basins known, all the condensed water at the
    /// surface, the cold area linear across the pass's climate ([`linear_cold_area`]), and the
    /// regolith's lid not known, weathered by the stand-in. Labelled: "the passes' stand-in: no
    /// rotation law, no basins, the bulk material's regolith; a system's record derives the body
    /// under its law, its basins and its lid (P14.T54.a)".
    #[must_use]
    pub(crate) fn stand_in(light: PassLight<'a>) -> Self {
        Self {
            climate: PassClimate::Given(&isothermal_climate),
            basins: Basins::UNKNOWN,
            water_at_surface: 1.0,
            light,
            regolith: &Regolith::of_material,
            deck: RunawayDeck::Kept,
            published: PublishedState::Climate,
            present: None,
        }
    }

    /// The surface of a pass under the context `spun` holds (P14.T54.a): the body's own climate,
    /// the context's basins and water share, under the light `light`, its regolith its own lid's
    /// row where it takes one ([`spun_lid`]) and its material's rule where it takes none.
    #[must_use]
    pub(crate) fn spun(spun: &'a SpunClimate<'a>, light: PassLight<'a>) -> Self {
        Self {
            basins: spun.context.basins(),
            water_at_surface: spun.context.water_share(),
            climate: PassClimate::Spun(spun),
            light,
            regolith: &Regolith::of_material,
            deck: RunawayDeck::Kept,
            published: PublishedState::Climate,
            present: spun.context.present(),
        }
    }

    /// The same surface with a runaway's cloud `deck` (P14.T53.b part 1 (b)).
    #[must_use]
    pub(crate) const fn with_deck(self, deck: RunawayDeck) -> Self {
        Self { deck, ..self }
    }

    /// The same surface with the published state `published` (P14.T53.b part 1 (c)).
    #[must_use]
    pub(crate) const fn with_published(self, published: PublishedState) -> Self {
        Self { published, ..self }
    }

    /// The same surface with the state `present` the previous pass carried by P14.T53.e's rules
    /// 5–7: the stand-in's, whose passes carry it beside the albedo ([`PassSurface::present`]).
    #[must_use]
    pub(crate) const fn with_present(self, present: Option<PresentClimate>) -> Self {
        Self { present, ..self }
    }
}

/// A climate with no contrast at the step's mean `t`, whatever its column: the passes' stand-in
/// climate, for a body with no rotation law.
#[must_use]
fn isothermal_climate(t: Kelvin, _: &PartialPressures) -> ClimateSpan {
    ClimateSpan::isothermal(t)
}

/// The passes' stand-in cold area (decision-p14-t51a-part2 §3.5), for a body with no rotation law:
/// the share of the surface below `t`, linear in area across `span` from its coldest zone to its
/// warmest, as the
/// partition tests' cover by hand (`linear_cold_area`), and a step at the mean where the span has
/// no contrast: all of it below a temperature above the mean, none below one at or under it.
/// None below a temperature that is not a number.
#[must_use]
pub(crate) fn linear_cold_area(span: &ClimateSpan, t: Kelvin) -> f64 {
    let t = t.value();
    if t.is_nan() {
        return 0.0;
    }
    let (cold, warm) = (span.coldest().value(), span.warmest().value());
    if warm > cold {
        ((t - cold) / (warm - cold)).clamp(0.0, 1.0)
    } else if t > span.mean().value() {
        1.0
    } else {
        0.0
    }
}

/// The passes' stand-in for water's cover, for a body with no rotation law, over the partition
/// `solved` of a body in the state `state`, its water following `water_state`
/// ([`Atmosphere::water_state`]), whose bulk is of `material`, on the surface `surface` over the
/// climate `climate`: P14.T24.b's rules with no hypsometry, the cold area linear across it. A body
/// whose record holds its law takes the record's cover instead ([`SpunClimate`]).
///
/// - The surface water is the partition's condensed water times the ground's surface share,
///   under T24.b's gate (a temperate world or a snowball by its water's state;
///   decision-p14-t51a-part2 §4.2).
/// - The potential ocean is the share its layer fills of the surface's basins, none where no
///   basins are known ([`Basins::area_filled`]): in the stand-in no pass's water covers any area as
///   a sea (labelled).
/// - The ice is the cold area below T24.b's threshold of the state ([`cover::ice_threshold`]), a
///   `Snowball`'s water and an icy crust's uncapped, a silicate crust's capped by its surface water
///   at [`cover::ICE_SHEET_THICKNESS`] of ice Ih at the mean ([`cover::ice_fraction`]'s rule).
/// - No crust is listed ([`WaterCover::crust_water`]): the area mix reads an ice bulk's crust as
///   its regolith.
#[must_use]
fn pass_water_cover(
    solved: &Partition,
    (state, water_state, material): (SurfaceState, SurfaceState, SurfaceMaterial),
    surface: &PassSurface<'_>,
    climate: &ClimateSpan,
) -> WaterCover {
    let holds = match water_state {
        SurfaceState::Temperate | SurfaceState::Desert | SurfaceState::Snowball => true,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Tenuous
        | SurfaceState::SteamEnvelope => false,
    };
    let reservoir = solved
        .condensate(Gas::Water.substance())
        .map_or(0.0, |c| c.reservoir().value());
    let sigma = if holds && reservoir > 0.0 {
        reservoir * solved.ground().water_at_surface()
    } else {
        0.0
    };
    let layer = cover::water_layer(KilogramsPerSquareMetre::new(sigma));
    let ice_density = ice_ih_density(climate.mean());
    let cold = linear_cold_area(climate, cover::ice_threshold(state));
    let ice_fraction = match (water_state, material) {
        (SurfaceState::Snowball, _) | (_, SurfaceMaterial::Ice) => cold,
        (_, SurfaceMaterial::Rock) => {
            let cap = sigma / (ice_density.value() * cover::ICE_SHEET_THICKNESS.value());
            if cold < cap { cold } else { cap }
        }
    };
    WaterCover {
        surface_water: KilogramsPerSquareMetre::new(sigma),
        potential_ocean: surface.basins.area_filled(layer),
        ice_fraction,
        ice_density,
        // The area mix reads an ice bulk's crust as its regolith (`Regolith::of_material`), so the
        // pass lists no crust beside it (decision-p14-t54a-ganymede, C1 follow-up §C1.4).
        crust_water: CrustWater::Unlisted,
    }
}

/// What a pass's partition gives its area mix (P14.T51.a): its partition solved once more at the
/// pass's final climate, its condensates published over the pass's cover, under the body's own
/// climate the record's conditions it was solved over, `None` where it read none (a climate given,
/// or a column with nothing that condenses), and its water cover's ice
/// ([`WaterCover::ice_fraction`]), which P14.T53.e's rule 5 adds where the condensates list no
/// water ice, as the record's figures add T24.b's.
type PassPartition = (Partition, Condensates, Option<SurfaceConditions>, f64);

/// The partition of the atmosphere `air` of the body `inputs` describe under a climate given
/// `climate_of` ([`PassClimate::Given`]), on the surface `surface`, its body of gravity `gravity`:
/// the climate at the air's surface temperature and partial pressures, as
/// [`GlobalFigures::condensates_and_ice_over`](crate::planetary::hooks::figures::GlobalFigures::condensates_and_ice_over)
/// solves the record's, published over the stand-in's cover ([`pass_water_cover`]).
#[must_use]
fn given_partition(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
    (climate_of, gravity): (&ClimateOf<'_>, MetresPerSecondSquared),
) -> PassPartition {
    let climate = climate_of(air.surface_temperature, &air.partials);
    let ground = Ground::new(gravity, surface.basins, surface.water_at_surface);
    let solved = partition(&climate, &ground, &air.column_inventory());
    let water = pass_water_cover(
        &solved,
        (air.state, air.water_state, inputs.material),
        surface,
        &climate,
    );
    let cold_area = |t: Kelvin| linear_cold_area(&climate, t);
    let published = solved.publish(&Cover {
        cold_area: &cold_area,
        state: air.state,
        water,
    });
    (solved, published, None, water.ice_fraction)
}

/// The partition's ground of the atmosphere `air` under the body's own climate `spun`: the
/// context's ([`PassContext::partition_ground`]), its basins and water share, the surface water
/// and potential ocean of the air's own vapour, gated by its own state and retention, and the
/// crust its craters give, with no ice bulk's crust listed.
#[must_use]
fn spun_ground(air: &Atmosphere, spun: &SpunClimate<'_>) -> PartitionGround {
    let record = &spun.record;
    let water = WaterGate::of_air(air, spun.inventory_water, record.bulk());
    spun.context
        .partition_ground(air, &water, record.material, CrustWater::Unlisted)
}

/// The record's conditions of the atmosphere `air` of the body `inputs` describe under its own
/// climate `spun`, on the partition's ground `ground` (rule (f)): the air under the record's spin,
/// gravity, radius and material, at the context's heat flow, the θ of the ground's potential
/// ocean, and its ground's inertia at the context's lid ([`SurfaceInputs::lid`]); `None` in the
/// gas-envelope state, which has no surface.
#[must_use]
fn spun_conditions(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    spun: &SpunClimate<'_>,
    ground: &PartitionGround,
) -> Option<SurfaceConditions> {
    let record = &spun.record;
    SurfaceConditions::derive(&SurfaceInputs {
        atmosphere: air,
        equilibrium_temperature: inputs.equilibrium,
        surface_gravity: record.surface_gravity,
        radius: record.radius,
        material: record.material,
        lid: spun.context.lid().map(|lid| lid.lithology),
        stellar_flux: spun.stellar_flux,
        xuv_flux: spun.xuv_flux,
        spin: record.spin,
        heat_flow: spun.context.heat_flow(),
        ocean_weight: OceanWeight::of_potential_ocean(ground.potential_ocean),
        // The previous pass's open water, the context's, as the pass's steps read it (P14.T24.j).
        holds_liquid_water: spun.context.holds_liquid_water(),
    })
}

/// A spun pass's partition before it publishes its condensates ([`spun_partition`]): its partition,
/// the cover it publishes over, and the conditions it read, `None` where nothing condenses. A pass
/// takes its lid and its maturity between the two (P14.T54.b.e), which read no condensate.
struct SpunPartition {
    /// The partition, solved.
    solved: Partition,
    /// The cover its condensates are published over.
    cover: SpunCover,
    /// The state its cover reads.
    state: SurfaceState,
    /// The record's conditions it read, `None` where nothing condenses and it cannot qualify for
    /// the cold traps.
    conditions: Option<SurfaceConditions>,
    /// Its ground's crust, the context's, which its cold traps read.
    crust: Crust,
    /// Its ground's surface gravity, which its cold traps' collisional test reads.
    gravity: MetresPerSecondSquared,
}

/// The cover a spun pass publishes its condensates over ([`SpunPartition`]).
enum SpunCover {
    /// Nothing condenses (L2): the mean's isothermal span, which no part of the cover reads.
    Isothermal {
        /// The span.
        climate: ClimateSpan,
        /// Water's cover, with no ice.
        water: WaterCover,
    },
    /// The conditions' cold profile and water's cover ([`partition_over`]).
    Profiled {
        /// The profile.
        profile: ColdProfile,
        /// Water's cover.
        water: WaterCover,
    },
}

impl SpunPartition {
    /// Its condensates published over its cover, with the partition and the conditions it read: the
    /// record's computation, to the bit. Where the pass read a cold-trap field (`traps`, of the
    /// atmosphere `air`), over that field ([`cold_traps::publish_over`]), with the body the record
    /// builds by the same constructor ([`PassColdTraps::body`]); P14.T51.a's otherwise.
    #[must_use]
    fn publish(self, traps: Option<&PassColdTraps>, air: &Atmosphere) -> PassPartition {
        let water_ice = match &self.cover {
            SpunCover::Isothermal { water, .. } | SpunCover::Profiled { water, .. } => {
                water.ice_fraction
            }
        };
        let published = match &self.cover {
            SpunCover::Isothermal { climate, water } => {
                debug_assert!(traps.is_none(), "a pass with no profile reads no field");
                let cold_area = |t: Kelvin| linear_cold_area(climate, t);
                self.solved.publish(&Cover {
                    cold_area: &cold_area,
                    state: self.state,
                    water: *water,
                })
            }
            SpunCover::Profiled { profile, water } => {
                let cold_area = |t: Kelvin| profile.area_below(t);
                let cover = Cover {
                    cold_area: &cold_area,
                    state: self.state,
                    water: *water,
                };
                match traps {
                    Some(traps) => cold_traps::publish_over(
                        &self.solved,
                        &cover,
                        &traps.body(air, profile, self.gravity),
                        traps.field(),
                    ),
                    None => self.solved.publish(&cover),
                }
            }
        };
        (self.solved, published, self.conditions, water_ice)
    }
}

/// What a pass under its body's own climate `spun` publishes of the atmosphere `air` of the body
/// `inputs` describe (P14.T54.a; rule (f)), before it publishes ([`SpunPartition::publish`]): the
/// record's computation of its condensates, to the bit ([`SpunClimate`]), with the conditions it
/// read.
///
/// The ground is the context's ([`spun_ground`]). The conditions are the record's of the air
/// ([`spun_conditions`]), and the partition and its cover are [`partition_over`]'s. A column with
/// nothing that condenses is solved over the mean's isothermal span instead, which its partition
/// does not read, and no part of its cover is read: it publishes nothing, and reads no
/// conditions (L2). A pass that can qualify for the cold traps ([`can_take_cold_traps`]) is not:
/// it reads its conditions, so that its delivered water and carbon dioxide traps are published by
/// it (P14.T54.b.f; decision-p14-t54b-f-coldtraps §2 (d)).
#[must_use]
fn spun_partition(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    spun: &SpunClimate<'_>,
) -> SpunPartition {
    let ground = spun_ground(air, spun);
    let (crust, gravity) = (ground.crust, ground.ground.gravity());
    let inventory = air.column_inventory();
    let reads_conditions = inventory.condenses() || can_take_cold_traps(air, spun, gravity);
    let conditions = reads_conditions
        .then(|| spun_conditions(air, inputs, spun, &ground))
        .flatten();
    let Some(conditions) = conditions else {
        // Nothing condenses (L2): no climate is read, and the mean's isothermal span stands in.
        let climate = ClimateSpan::isothermal(air.surface_temperature);
        let solved = partition(&climate, &ground.ground, &inventory);
        return SpunPartition {
            solved,
            cover: SpunCover::Isothermal {
                climate,
                water: WaterCover {
                    surface_water: ground.surface_water,
                    potential_ocean: ground.potential_ocean,
                    ice_fraction: 0.0,
                    ice_density: ice_ih_density(air.surface_temperature),
                    crust_water: ground.crust_water,
                },
            },
            state: air.state,
            conditions: None,
            crust,
            gravity,
        };
    };
    let PartitionOver {
        partition: solved,
        profile,
        water,
    } = partition_over((air, &inventory), &conditions, &ground, spun.record.table);
    SpunPartition {
        solved,
        cover: SpunCover::Profiled { profile, water },
        state: air.state,
        conditions: Some(conditions),
        crust,
        gravity,
    }
}

/// Whether a pass of the atmosphere `air` under its body's own climate `spun`, of surface gravity
/// `gravity`, can qualify for the cold traps before it reads its conditions (P14.T54.b.f;
/// decision-p14-t54b-f-coldtraps §2 (d)): a sky ([`SpunClimate::cold_traps`]), a spin that turns
/// under its seasonal host (the conditions' zonal profile, [`rotates`]), the airless state and an
/// air that is not collisional.
#[must_use]
fn can_take_cold_traps(
    air: &Atmosphere,
    spun: &SpunClimate<'_>,
    gravity: MetresPerSecondSquared,
) -> bool {
    spun.cold_traps.is_ok()
        && rotates(&spun.record.spin)
        && air.state == SurfaceState::Airless
        && !is_collisional(air, gravity)
}

/// The cold traps of the pass of the atmosphere `air` under its body's own climate `spun`, before
/// it publishes the partition `pending`, whose bare regolith is `regolith` under the light `shade`
/// (P14.T54.b.f; decision-p14-t54b-f-coldtraps §2): the field of its own inputs, from the
/// derivation's slot ([`FieldSlot::traps`]) where it qualifies, its context holds figures and its
/// cold traps have a sky; otherwise why it reads none.
///
/// The inputs are the derivation's sky, the context's heat flow, crater contract and crust (its
/// ground's), the regolith's Bond albedo in the airless state about the hosts' bands (the one
/// regolith its area mix reads, [`pass_regolith`]), its partition's condensables and its profile's
/// area points (`FieldInputs::of`).
fn pass_cold_traps(
    air: &Atmosphere,
    spun: &SpunClimate<'_>,
    pending: &SpunPartition,
    (regolith, shade): (&Regolith, &HostShade),
) -> PassTraps {
    let traps = spun.cold_traps?;
    let SpunCover::Profiled { profile, .. } = &pending.cover else {
        return Err(NoColdTraps::Unqualified);
    };
    if !qualifies_by(air, profile, pending.gravity) {
        return Err(NoColdTraps::Unqualified);
    }
    let figures = spun.context.figures().ok_or(NoColdTraps::NoFigures)?;
    let body = ColdTrapBody {
        atmosphere: air,
        profile,
        gravity: pending.gravity,
        heat_flow: spun.context.heat_flow(),
        crust: pending.crust,
        craters: figures.craters(),
        sky: &traps.sky,
        regolith_albedo: regolith.bond(SurfaceState::Airless, shade.bands()).bond,
    };
    let inputs = FieldInputs::of(&body, &pending.solved)
        .expect("a qualifying pass's profile is zonal, and so holds a field's area points");
    Ok(traps.slot.traps(&inputs))
}

/// Why a pass over the surface `surface` that reads no partition reads no cold-trap field, `why`:
/// a climate given's, or no sky's where its spun climate's cold traps read none.
#[must_use]
fn no_cold_traps(surface: &PassSurface<'_>, why: NoColdTraps) -> NoColdTraps {
    match surface.climate {
        PassClimate::Given(_) => NoColdTraps::ClimateGiven,
        PassClimate::Spun(spun) => spun.cold_traps.err().unwrap_or(why),
    }
}

/// The lid a pass under its body's own climate `spun` takes (P14.T54.a; decision-pre-23-review
/// item 17): its crust's basis's ([`CrustBasis::lid_at`]) at the pass's state and final mean
/// surface temperature, the record's mean `T_s` to the bit, under the surface age and the tide of
/// the context it reads, the cometary test reading the warmest of its unwidened range,
/// `conditions`' where the pass read them and otherwise its record's conditions, built only where
/// that test is read ([`spun_conditions`]). `None` where the body has no basis, or the context no
/// surface age (C₀, a body with no figures): the material's rule then stands.
///
/// A lid is a function of the pass's albedo and context alone, as the pass is: so a lid that
/// changes between passes is a jump across the diagonal for T51.h's bracket, and the solver's
/// re-check lists it.
#[must_use]
fn spun_lid(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    spun: &SpunClimate<'_>,
    conditions: Option<&SurfaceConditions>,
) -> Option<CrustLid> {
    let basis = spun.basis.as_ref()?;
    let surface_age = spun.context.surface_age()?;
    let at = LidAt {
        state: air.state,
        mean_surface_temperature: air.surface_temperature,
        surface_age,
        tidal: spun.context.tidal(),
    };
    let warmest = || {
        if let Some(conditions) = conditions {
            return conditions.climate_range().1;
        }
        let warmest = spun_conditions(air, inputs, spun, &spun_ground(air, spun))
            .expect("the lid refuses the gas-envelope state before it reads its range")
            .climate_range()
            .1;
        // The deferred warmest, which the sample test holds to its record's (the review's D3-2).
        #[cfg(test)]
        spun_read::deferred(warmest);
        warmest
    };
    let lid = basis.lid_at(&at, warmest);
    // The figures' surface age lies in [0, the age], and the area mix asks no gas envelope's lid:
    // a refusal is a bug, which the debug builds catch; the pass then takes the material's rule.
    debug_assert!(lid.is_ok(), "{lid:?}");
    lid.ok()
}

/// The largest Bond albedo the type holds, the largest `f64` below 1: a lossless frost's Bond
/// albedo of 1 (`O2`'s, `Ar`'s, `H2S`'s and `NH4SH`'s, which P14.T49.i holds lossless to 5 µm, no
/// infrared primary being found for them) is held just below it.
pub(crate) const LARGEST_BOND_ALBEDO: f64 = 1.0 - f64::EPSILON / 2.0;

/// The area mix's albedo `value` as a [`BondAlbedo`], held into 0 ≤ A < 1
/// ([`LARGEST_BOND_ALBEDO`]); the state's table albedo for a value that is not a number, which no
/// registry row gives (labelled).
#[must_use]
fn bond_albedo(value: f64, state: SurfaceState, material: SurfaceMaterial) -> BondAlbedo {
    if value.is_nan() {
        return state.albedo(material);
    }
    let held = value.clamp(0.0, LARGEST_BOND_ALBEDO);
    debug_assert!(
        !held.is_nan(),
        "a clamp of a number into [0, 1) is a number"
    );
    BondAlbedo::from_fraction(held)
}

/// [`atmosphere_under`]'s batch arm (P14.T51.b): the atmosphere `air` of the body `inputs`
/// describe, whose state and greenhouse the batch's rules gave, with its Bond albedo and cloud
/// fraction from the area mix over the surface `surface` in place of T13.c's table, and under the
/// body's own climate the lid the pass takes ([`spun_lid`], P14.T54.a):
///
/// - a gas envelope its class's ([`area_mix::giant_bond`]) at its heated temperature and for its
///   planet class, under a full cover, which its class's albedo includes; it has no lid;
/// - a magma ocean the melt's 0.10 ([`area_mix::melt_bond`]), cloudless, whatever its lid;
/// - every other state the area mix over the condensates its pass publishes and its regolith, its
///   lid's row where it takes one, with the cloud fraction of its state and condensable at its
///   cloud's albedo ([`area_mix::under_cloud`]), a runaway's Venus's deck or, solved without it,
///   the water cloud's (the surface's [`deck`](PassSurface::deck), P14.T53.b part 1 (b)), which
///   the atmosphere then publishes ([`Atmosphere::runaway_deck`]: kept for every other state);
/// - and every state but a gas envelope floored at its air's own Rayleigh layer's, from version 24
///   (P14.T24.h.a; [`floored`]).
///
/// The atmosphere also carries whether the condensates the pass published list liquid water,
/// which P14.T53.b part 1 (c)'s test reads ([`solved_as_a_hothouse`]): none for a gas envelope
/// and a magma ocean, which publish none; and whether a pass under a locked world's deck melts its
/// water in its warmest zone, which the next pass's deck reads (P14.T24.j;
/// [`Atmosphere::melts_water_under_deck`]). Beside the atmosphere and the lid it gives its
/// maturity's outcome (P14.T54.b.e; [`PassMaturity`]) and its cold traps' (P14.T54.b.f;
/// `cold_traps::PassTraps`).
///
/// [`solved_as_a_hothouse`]: super::solved_as_a_hothouse
pub(super) fn area_mixed(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
) -> (Atmosphere, Option<CrustLid>, PassMaturity, PassTraps) {
    let hottest;
    let shade = match surface.light {
        PassLight::Shade(shade) => shade,
        PassLight::HottestHost => {
            hottest = HostShade::of_host(inputs.hottest_host);
            &hottest
        }
    };
    let haze = pass_haze(air, inputs);
    let SurfaceMix {
        disc,
        bands,
        lid,
        liquid_water,
        deck_water,
        maturity,
        cold_traps,
        present,
    } = match air.state {
        SurfaceState::GasEnvelope => SurfaceMix {
            disc: Disc::Whole {
                albedo: envelope_bond::gas_envelope_bond(shade, inputs),
                cloud_fraction: area_mix::cloud_share_of_state(SurfaceState::GasEnvelope),
            },
            bands: None,
            lid: None,
            liquid_water: false,
            deck_water: OpenWater::Dry,
            maturity: Err(NoMaturity::NoRegolith),
            cold_traps: Err(no_cold_traps(surface, NoColdTraps::Unqualified)),
            present: None,
        },
        SurfaceState::MagmaOcean => {
            let lid = match surface.climate {
                PassClimate::Spun(spun) => spun_lid(air, inputs, spun, None),
                PassClimate::Given(_) => None,
            };
            SurfaceMix {
                disc: Disc::Whole {
                    albedo: area_mix::melt_bond(),
                    cloud_fraction: 0.0,
                },
                bands: None,
                lid,
                liquid_water: false,
                deck_water: OpenWater::Dry,
                maturity: Err(NoMaturity::NoRegolith),
                cold_traps: Err(no_cold_traps(surface, NoColdTraps::Unqualified)),
                present: None,
            }
        }
        // A steam envelope's photometry is a runaway's, its label read by no pass (P14.T53.b
        // part 3).
        SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Desert
        | SurfaceState::Snowball
        | SurfaceState::Tenuous
        | SurfaceState::SteamEnvelope => surface_and_cloud(air, inputs, surface, (shade, haze)),
    };
    #[cfg(test)]
    if let PassClimate::Spun(_) = surface.climate {
        spun_read::trace(lid.as_ref());
    }
    let albedo = floored(disc, air, inputs, (shade, haze));
    let band_albedo = bands.and_then(|bands| bands.laid(disc, air, inputs, (shade, haze)));
    let runaway_deck = match air.state {
        SurfaceState::RunawayGreenhouse | SurfaceState::SteamEnvelope => surface.deck,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::Temperate
        | SurfaceState::Desert
        | SurfaceState::Snowball
        | SurfaceState::Tenuous => RunawayDeck::Kept,
    };
    let air = Atmosphere {
        albedo: bond_albedo(albedo, air.state, inputs.material),
        cloud_fraction: disc.cloud_fraction(),
        band_albedo,
        runaway_deck,
        liquid_water,
        deck_water,
        present,
        ..*air
    };
    (air, lid, maturity, cold_traps)
}

/// What a pass's area mix gives ([`area_mixed`]): its disc before its air's layer, in the B, V and
/// R bins too from generator version 30, the lid it
/// took, whether its condensates list liquid water, whether it melts its water under a locked
/// world's deck ([`Atmosphere::melts_water_under_deck`]), its maturity's outcome, its cold traps'
/// and the state it carries to the next pass by P14.T53.e's rules 5–7 ([`state_to_carry`]).
struct SurfaceMix {
    disc: Disc,
    /// Its disc in the B, V and R bins before its air's layer, from generator version 30
    /// ([`DiscBands`]): `None` below 30 and for a disc of one albedo.
    bands: Option<DiscBands>,
    lid: Option<CrustLid>,
    liquid_water: bool,
    deck_water: OpenWater,
    maturity: PassMaturity,
    cold_traps: PassTraps,
    present: Option<PresentClimate>,
}

/// A pass's disc as its area mix gives it, before its air's Rayleigh layer and haze ([`floored`]):
/// its Bond albedo and its cloud fraction, and how they are made, which the layer reads
/// (P14.T24.h.b).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Disc {
    /// One albedo for its whole disc: a gas envelope's class, its clouds included, or a melt's.
    Whole {
        /// Its Bond albedo, 0 to 1.
        albedo: f64,
        /// Its share under cloud, 0 to 1.
        cloud_fraction: f64,
    },
    /// A ground beneath a cloud, which lies above the air's Rayleigh layer.
    Clouded {
        /// The ground's Bond albedo, the clear sky's, 0 to 1.
        ground: f64,
        /// Its share under cloud, 0 to 1.
        cloud_fraction: f64,
        /// The cloud's Bond albedo, 0 to 1.
        cloud_albedo: f64,
    },
}

impl Disc {
    /// Its Bond albedo: the whole disc's, or the area mix of its ground under its cloud
    /// ([`area_mix::under_cloud`]).
    #[must_use]
    fn albedo(self) -> f64 {
        match self {
            Self::Whole { albedo, .. } => albedo,
            Self::Clouded {
                ground,
                cloud_fraction,
                cloud_albedo,
            } => area_mix::under_cloud(ground, cloud_fraction, cloud_albedo),
        }
    }

    /// Its share under cloud.
    #[must_use]
    const fn cloud_fraction(self) -> f64 {
        match self {
            Self::Whole { cloud_fraction, .. } | Self::Clouded { cloud_fraction, .. } => {
                cloud_fraction
            }
        }
    }
}

/// A pass's disc in the B, V and R bins before its air's Rayleigh layer and haze (the surface
/// colour, from generator version 30; `area_mix::ColourBand`): its ground's albedo in each bin,
/// its area mix under the light of that bin alone (`HostShade::in_band`), and its cloud's
/// ([`area_mix::cloud_band_albedo`]). Its cloud fraction is its disc's, the one its hosts' light
/// gives.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DiscBands {
    /// The ground's albedo in each bin, the clear sky's, 0 to 1.
    ground: Bands,
    /// The cloud's albedo in each bin, 0 to 1.
    cloud_albedo: Bands,
}

/// `of(band)` in each of B, V and R.
#[must_use]
fn by_band(of: impl Fn(ColourBand) -> f64) -> Bands {
    Bands {
        b: of(ColourBand::B),
        v: of(ColourBand::V),
        r: of(ColourBand::R),
    }
}

/// `bands`' value in `band`.
#[must_use]
const fn in_band(bands: Bands, band: ColourBand) -> f64 {
    match band {
        ColourBand::B => bands.b,
        ColourBand::V => bands.v,
        ColourBand::R => bands.r,
    }
}

impl DiscBands {
    /// The disc's bands of a pass whose ground and cloud are under the state `state` and `deck`
    /// (`ground` its ground in a band's light alone, from its condensates, deposits, regolith and
    /// sediment), under the light `shade`.
    #[must_use]
    fn of(
        (state, deck): (SurfaceState, RunawayDeck),
        shade: &HostShade,
        ground: impl Fn(ColourBand, &HostShade) -> f64,
    ) -> Self {
        Self {
            ground: by_band(|band| ground(band, &shade.in_band(band))),
            cloud_albedo: by_band(|band| area_mix::cloud_band_albedo((state, deck), shade, band)),
        }
    }

    /// The disc's colour, [`Atmosphere::band_albedo`]: each band's ground under the cloud
    /// fraction of `disc` at the cloud's albedo in the band, under the Rayleigh layer and haze of
    /// the atmosphere `air` of the body `inputs` describes in that band's light alone
    /// ([`layered`]), as [`floored`] lays them under the adding rule. `None` where the albedos are
    /// not a colour ([`DiscColour::new`]).
    #[must_use]
    fn laid(
        self,
        disc: Disc,
        air: &Atmosphere,
        inputs: &AtmosphereInputs,
        (shade, haze): (&HostShade, f64),
    ) -> Option<DiscColour> {
        DiscColour::new(by_band(|band| {
            let disc = Disc::Clouded {
                ground: in_band(self.ground, band),
                cloud_fraction: disc.cloud_fraction(),
                cloud_albedo: in_band(self.cloud_albedo, band),
            };
            layered(disc, air, inputs, (&shade.in_band(band), haze))
        }))
    }
}

/// The snapshot haze's τ(550) of the pass of the atmosphere `air` of the body `inputs` describes
/// ([`area_mix::snapshot_haze_optical_depth`]), which its sediment and its haze layer read
/// (P14.T24.h.b), once a pass; 0, unread, under both their holds and for a body of no mass or
/// radius.
#[must_use]
fn pass_haze(air: &Atmosphere, inputs: &AtmosphereInputs) -> f64 {
    let read = area_mix::OrganicSedimentRules::in_force()
        == area_mix::OrganicSedimentRules::Sediment
        || area_mix::HazeLayerRules::in_force() == area_mix::HazeLayerRules::Absorbing;
    match surface_gravity(inputs) {
        Some(gravity) if read => {
            area_mix::snapshot_haze_optical_depth(&air.partials, gravity, inputs.hottest_host)
        }
        Some(_) | None => 0.0,
    }
}

/// The mean surface temperature of an airless body of equilibrium temperature `equilibrium` over
/// the surface `surface` (P14.T24.i): the larger of `equilibrium` and the night floor at the heat
/// flow of the context the pass reads, (`T_CMB`⁴ + F ÷ `ε_r` σ)^¼
/// ([`night_floor`](crate::planetary::hooks::surface::night_floor)), from generator version 24.
///
/// A record with no host light publishes `T_eq` = 0 K, under its own night floor of 12–20 K, and
/// `for_body` refuses it (P14.T54.a's labelled refusals); the floor gives it the mean its
/// conditions' climate already holds. A max of continuous functions is continuous, and a body
/// whose `T_eq` is above its floor keeps it to the bit. The passes' stand-in reads no heat flow,
/// so its floor is the cosmic background's. The flux sum (`T_eq`⁴ + `T_floor`⁴)^¼ for lit bodies is a
/// later refinement, since it moves every airless body (recorded).
///
/// It is the base temperature `T_base` of every body with a surface, in `state_and_greenhouse`
/// (P14.T24.i's follow-up, decision-t24i-airless-state D1.3): the
/// greenhouse's temperature argument, so that its steps start at it and its grey surface is
/// `T_base` (1 + ¾τ)^¼; the 100 Pa test reads that greenhouse's partial pressures and an airless
/// body publishes them at it; and the ice-held stand-in reads it. The floor is every point's lower
/// bound, so no ice on the body lies colder, and the area mix partitions the column at the
/// published mean: one base keeps the state, the `T_s` and the partition at one temperature, and
/// keeps `T_s` continuous across the 100 Pa line (D1.2). The max is T24.i's adopted lower bound on a
/// grey air's base over an internal flux, the flux sum (Eddington's grey solution with both
/// fluxes) its recorded refinement. The exobase, a gas envelope's heated temperature, the magma
/// ocean's solidus, the hottest host's cap and the climate body's own `T_eq` and heat flow do not
/// read it.
///
/// A moon, whose context holds its tide, takes that refinement in its tides' own form (P14.T55.j):
/// the larger of its floor and (`T_eq`⁴ + F ÷ σ)^¼ ([`radiating_surface_temperature`]), the
/// surface its shell's top reads,
/// at the context's F, its whole heat flow. A moon whose F is not positive, a planet and a member
/// keep the max above to the bit. The floor's `ε_r` and the lit surface's unit emissivity are both
/// lower bounds on a grey regolith's (`T_CMB`⁴ + (`T_eq`⁴ + F ÷ σ) ÷ `ε_r`)^¼, and the floor wins
/// only below `T_eq` = (1 ÷ `ε_r` − 1)^¼ (F ÷ σ)^¼, where it lies at most `ε_r`^−¼ − 1 = 1.29%
/// above the lit balance (lean, 2026-11-02).
#[must_use]
pub(super) fn airless_mean(equilibrium: Kelvin, surface: &PassSurface<'_>) -> Kelvin {
    let heat_flow = heat_flow(surface);
    let floor = night_floor(heat_flow);
    let lit = match surface.climate {
        PassClimate::Spun(spun) if spun.context.tide().is_some() => {
            radiating_surface_temperature(equilibrium, heat_flow)
        }
        PassClimate::Spun(_) | PassClimate::Given(_) => equilibrium,
    };
    if floor.value() > lit.value() {
        floor
    } else {
        lit
    }
}

/// The light of the pass over `surface` of the body `inputs` describes: its hosts' shade, or the
/// hottest host's for a single pass with none ([`PassLight`]).
#[must_use]
pub(super) fn pass_shade(surface: &PassSurface<'_>, inputs: &AtmosphereInputs) -> HostShade {
    match surface.light {
        PassLight::Shade(shade) => *shade,
        PassLight::HottestHost => HostShade::of_host(inputs.hottest_host),
    }
}

/// The surface heat flow a pass over `surface` reads: its context's, the previous pass's figures',
/// under the body's own climate, and none for a climate given, the passes' stand-in, which reads
/// no heat flow ([`airless_mean`]'s floor; P14.T53.c's ice temperature).
#[must_use]
pub(super) fn heat_flow(surface: &PassSurface<'_>) -> WattsPerSquareMetre {
    match surface.climate {
        PassClimate::Spun(spun) => spun.context.heat_flow(),
        PassClimate::Given(_) => WattsPerSquareMetre::ZERO,
    }
}

/// The surface gravity of the body `inputs` describe, G M ÷ R², `None` for a body of no mass or
/// radius.
#[must_use]
fn surface_gravity(inputs: &AtmosphereInputs) -> Option<MetresPerSecondSquared> {
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    (m.is_finite() && m > 0.0 && r.is_finite() && r > 0.0)
        .then(|| MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r)))
}

/// The Bond albedo of the disc `disc` of the atmosphere `air` of the body `inputs` describe, under
/// its air's own Rayleigh layer and haze, under the light and the haze's τ(550) of `lit`.
///
/// To 25 it is floored at its air's own Rayleigh layer's (P14.T24.h.a; `area_mix::rayleigh_floor`):
/// the larger of the two on every body with a surface, a gas envelope's class albedo excepted, and
/// the disc's own otherwise, or where it is not a number (which [`bond_albedo`] labels) or the
/// body has no mass or radius. The floor binds only where an air is darker than its own Rayleigh
/// layer: a max of continuous functions, continuous in the passes' map, and the disc's to the bit
/// wherever it is at least the floor.
///
/// Under the adding rule (P14.T24.h.b's fourth part, [`area_mix::RayleighAddingRules::Added`]) the
/// layer is laid over the clear sky's ground in each of the optics' bins instead
/// ([`area_mix::rayleigh_bins`], [`area_mix::rayleigh_added`]), under its cloud, which lies above
/// it (the lane's lean, 2026-10-31): a runaway's deck keeps Venus's measured 0.76 whole, where the
/// layer laid over it would count its air twice. Under the haze layer (P14.T24.h.b's third part,
/// [`area_mix::HazeLayerRules::Absorbing`]) a hazy air's haze lies over all of it, bin by bin
/// ([`area_mix::hazed_bins`]).
#[must_use]
fn floored(
    disc: Disc,
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    (shade, haze): (&HostShade, f64),
) -> f64 {
    let albedo = disc.albedo();
    let Some(gravity) = beneath_its_air(disc, air, inputs) else {
        return albedo;
    };
    // The floor is the oracle's, and the tests' record of it.
    #[cfg(test)]
    let floor = area_mix::rayleigh_floor(&air.partials, gravity, shade);
    #[cfg(test)]
    floor_read::record(floor);
    match area_mix::RayleighAddingRules::in_force() {
        #[cfg(test)]
        area_mix::RayleighAddingRules::Max => {
            let held = if floor > albedo { floor } else { albedo };
            area_mix::hazed(held, haze, shade)
        }
        area_mix::RayleighAddingRules::Added => added(disc, &air.partials, gravity, (shade, haze)),
    }
}

/// The surface gravity of the body `inputs` describe where the disc `disc` of its atmosphere `air`
/// lies beneath its air's own layer ([`floored`]): every state but a gas envelope, whose class
/// albedo is its whole disc's; `None` there, where the disc's albedo is not a number (which
/// [`bond_albedo`] labels) and for a body of no mass or radius.
#[must_use]
fn beneath_its_air(
    disc: Disc,
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
) -> Option<MetresPerSecondSquared> {
    let surface = match air.state {
        SurfaceState::GasEnvelope => false,
        SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Desert
        | SurfaceState::Snowball
        | SurfaceState::Tenuous
        | SurfaceState::SteamEnvelope => true,
    };
    if !surface || disc.albedo().is_nan() {
        return None;
    }
    surface_gravity(inputs)
}

/// [`floored`]'s disc `disc` under the adding rule (P14.T24.h.b's fourth part,
/// [`area_mix::RayleighAddingRules::Added`]), its air of partial pressures `partials` under the
/// gravity `gravity`, under the light `shade` and the haze's τ(550) `haze`: its Rayleigh layer over the
/// clear sky's ground in each of the optics' bins, under its cloud, and its haze over all of it.
#[must_use]
fn added(
    disc: Disc,
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
    (shade, haze): (&HostShade, f64),
) -> f64 {
    // An air with no Rayleigh depth lays no layer: its disc, to the bit.
    let Some(layers) = area_mix::rayleigh_bins(partials, gravity) else {
        return area_mix::hazed(disc.albedo(), haze, shade);
    };
    let (ground, cloud) = match disc {
        Disc::Whole { albedo, .. } => (albedo, None),
        Disc::Clouded {
            ground,
            cloud_fraction,
            cloud_albedo,
        } => (ground, Some((cloud_fraction, cloud_albedo))),
    };
    let clear = layers.map(|layer| area_mix::rayleigh_added(layer, ground));
    let clouded = |clear: f64| {
        cloud.map_or(clear, |(fraction, albedo)| {
            area_mix::under_cloud(clear, fraction, albedo)
        })
    };
    if area_mix::hazes(haze) {
        area_mix::hazed_bins(&clear.map(clouded), haze, shade)
    } else {
        // The cloud over the light-weighted clear sky, so that a whole deck keeps its
        // albedo to the bit.
        clouded(area_mix::band_weighted(&clear, shade.bands()))
    }
}

/// The disc `disc` of the atmosphere `air` of the body `inputs` describes laid under its air's
/// layer and haze by the adding rule, as [`floored`] lays it, with no floor read: the surface
/// colour's, in one band's light alone ([`DiscBands::laid`]), from generator version 30.
#[must_use]
fn layered(
    disc: Disc,
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    (shade, haze): (&HostShade, f64),
) -> f64 {
    match beneath_its_air(disc, air, inputs) {
        Some(gravity) => added(disc, &air.partials, gravity, (shade, haze)),
        None => disc.albedo(),
    }
}

/// The floor the last floored pass on a thread read (P14.T24.h.a's tests): the published pass's,
/// when a test re-runs it last ([`PassDriver::Again`](crate::planetary::derive::PassDriver)).
#[cfg(test)]
pub(crate) mod floor_read {
    use core::cell::Cell;

    thread_local! {
        /// The last floor read on this thread, a test's own.
        static LAST: Cell<Option<f64>> = const { Cell::new(None) };
    }

    /// Records a pass's floor as the thread's last.
    pub(crate) fn record(floor: f64) {
        LAST.with(|last| last.set(Some(floor)));
    }

    /// The last floor read on this thread since the last take, if any.
    #[must_use]
    pub(crate) fn take() -> Option<f64> {
        LAST.with(Cell::take)
    }

    /// Forgets the thread's last floor, so that the next [`take`] reads only a pass run after it.
    pub(crate) fn clear() {
        LAST.with(|last| last.set(None));
    }
}

/// The regolith a pass's area mix reads on the surface its condensates leave, the one function
/// its area mix and its bare regolith read (P14.T51.b, P14.T54.a, P14.T54.b.e; the cold traps'
/// bare regolith is its [`Regolith::bond`] in the airless state): with no lid, the passes'
/// regolith provider's on the material `material` (`of_material`, the material's rule); with a
/// lid of `lid` and no maturity, its row by the stand-in ([`Regolith::with_lid`]); and with a lid
/// and the pass's maturity m, mean surface temperature, surface age and state, its row weathered
/// at m where it lies, or, an airless silicate lid on a surface younger than a regolith, capped
/// ([`Regolith::weathered_lid`]; decision-p14-t54b-e-findings §A.5). A maturity comes only with a
/// lid, which it weathers. A pass computes it before it publishes its condensates.
#[must_use]
pub(crate) fn pass_regolith(
    of_material: &RegolithOf<'_>,
    material: SurfaceMaterial,
    lid: Option<(&CrustLid, Option<LidWeathering>)>,
) -> Regolith {
    match lid {
        None => of_material(material),
        Some((lid, None)) => Regolith::with_lid(lid.row(), material),
        Some((lid, Some(at))) => Regolith::weathered_lid(lid.row(), material, at),
    }
}

/// What the pass of the atmosphere `air` under the context `context` knows of its body's
/// resurfacing, which an `H2O` regolith's lag reads (P14.T49.m; [`Regolith::with_resurfacing`]):
/// the tidal heat flux of the context's figures, the record's tide, not known where it reads no
/// context (the passes' stand-in) or its context holds no figures (the first pass's C₀); and
/// whether its air's ice holds a volatile ([`Atmosphere::ice_holds_a_volatile`]), on either.
#[must_use]
pub(crate) fn pass_resurfacing(air: &Atmosphere, context: Option<&PassContext>) -> IcyResurfacing {
    let tidal = context
        .and_then(PassContext::figures)
        .map(PassFigures::tidal_heat_flow);
    let frost = if air.ice_holds_a_volatile() {
        VolatileFrost::Held
    } else {
        VolatileFrost::None
    };
    IcyResurfacing::new(tidal, frost)
}

/// The maturity of the pass of the atmosphere `air` under its body's own climate `spun`, whose lid
/// is `lid` (P14.T54.b.e), before it publishes its condensates: P14.T51.d's weathering law
/// ([`surface_weathering`]) over its modifiers' parts, each the record's member to the bit where
/// the record has it:
///
/// - **from its record, once a body** ([`PassWeathering`]): the host, the orbit, what it orbits,
///   the record's bulk's gravity and the mantle's sulphur;
/// - **from the context it reads** (its figures' first stage, K of the pass before): the field by
///   its dynamo law over the record's class, mass, age and rotation period (`PassFigures`'s
///   `magnetic_field`, the record's figures' one function), the surface age, the resurfacing time
///   at that stage's heat flow, and the heat flow;
/// - **from itself:** its air's surface pressure as its conditions publish it (none in the airless
///   state, [`published_surface_pressure`]) and mean surface temperature, its lid's row and its
///   air's partial pressures, the record's conditions' and gases' to the bit.
///
/// It reads no condensate and no water. `Err` with why where it takes none: the record's case, no
/// figures in the context it reads (C₀), no lid, or the parts refused.
fn pass_maturity<'a>(
    air: &Atmosphere,
    spun: &SpunClimate<'a>,
    lid: Option<&CrustLid>,
) -> Result<Maturing<'a>, NoMaturity> {
    let weathering = spun.weathering?;
    let figures = spun.context.figures().ok_or(NoMaturity::NoFigures)?;
    let lid = lid.ok_or(NoMaturity::NoLid)?;
    let case = &weathering.case;
    let parts = ModifierParts {
        host: case.host,
        orbit: case.orbit,
        primary: case.primary,
        gravity: weathering.gravity,
        surface_pressure: published_surface_pressure(air),
        field: figures.magnetic_field(&case.field),
        surface_age: figures.surface_age(),
        resurfacing_time: figures.resurfacing_time(),
        heat_flow: figures.heat_flow(),
        surface_temperature: air.surface_temperature,
        mantle_sulphur: weathering.mantle_sulphur,
        lid: lid.row(),
        gases: air.partials,
    };
    let weathered = surface_weathering(&parts).map_err(NoMaturity::Refused)?;
    Ok(Maturing {
        parts,
        weathering: weathered,
        record: weathering,
    })
}

/// A pass's maturity before it publishes ([`pass_maturity`]): its modifiers' parts, its weathering
/// and what its record gives its maturity.
#[derive(Clone, Copy)]
struct Maturing<'a> {
    parts: ModifierParts,
    weathering: SurfaceWeathering,
    record: PassWeathering<'a>,
}

/// The weathering deposits of a pass whose maturity is `maturity`, which published `published`
/// (P14.T51.j's `ferric`; P14.T54.b.e), each a row and its area, with the maturity's outcome its
/// published pass carries: the modifiers over its parts, its published condensates and its water
/// ([`SurfaceModifiers::derive`]), whose maturity is the one its regolith took (asserted in the
/// debug builds), its regolith `regolith`. The water is wet now where its condensates list liquid
/// water (the record's conditions' flag, `with_condensates`), else, where an oxidised deposit's
/// other conditions hold ([`oxidisable`]), dry since its wet epoch at the pass, else never wet,
/// which the deposit then does not read. No deposit where the modifiers refuse (the water's dry
/// time alone, which a wet epoch's end never makes), and none at all without a maturity.
fn pass_deposits(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    (maturity, regolith): (Result<Maturing<'_>, NoMaturity>, Regolith),
    published: &Condensates,
) -> (Vec<(SubstanceId, f64)>, PassMaturity) {
    let Maturing {
        parts,
        weathering,
        record,
    } = match maturity {
        Ok(maturity) => maturity,
        Err(none) => return (Vec::new(), Err(none)),
    };
    let wet_now = published
        .entry(Gas::Water.substance(), CondensatePhase::Liquid)
        .is_some();
    let water = if wet_now {
        WaterHistory::WetNow
    } else if oxidisable(&parts) {
        (record.wet_epoch)(air, inputs.equilibrium).map_or(WaterHistory::NeverWet, |epoch| {
            WaterHistory::DrySince(epoch.end())
        })
    } else {
        // The deposit, the water's one reader, does not read it here (`oxidisable`).
        WaterHistory::NeverWet
    };
    let derived = SurfaceModifiers::derive(&parts.with_published(published, water));
    debug_assert!(
        derived
            .as_ref()
            .is_ok_and(|m| m.maturity().total_cmp(&weathering.maturity()).is_eq()),
        "the modifiers' maturity is the pass's own: {derived:?}"
    );
    let deposits = derived.map_or_else(
        |_| Vec::new(),
        |modifiers| {
            modifiers
                .weathering_deposits()
                .iter()
                .map(|deposit| (deposit.substance(), deposit.area_fraction()))
                .collect()
        },
    );
    let weathered = PassWeathered {
        parts,
        water,
        weathering,
        regolith,
    };
    (deposits, Ok(weathered))
}

/// The area mix of a body of no mass or radius, the atmosphere `air` of the body `inputs` describe
/// on the surface `surface` under the light `shade` ([`surface_and_cloud`]): it holds nothing and
/// takes no lid, its albedo its regolith's.
#[must_use]
fn weightless(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
    shade: &HostShade,
) -> SurfaceMix {
    let regolith = pass_regolith(surface.regolith, inputs.material, None);
    SurfaceMix {
        disc: Disc::Whole {
            albedo: regolith.bond(air.state, shade.bands()).bond,
            cloud_fraction: 0.0,
        },
        bands: None,
        lid: None,
        liquid_water: false,
        deck_water: OpenWater::Dry,
        maturity: Err(NoMaturity::NoRegolith),
        cold_traps: Err(no_cold_traps(surface, NoColdTraps::Unqualified)),
        present: None,
    }
}

/// The area-mix Bond albedo and cloud fraction of a body with a surface, the atmosphere `air` of
/// the body `inputs` describe on the surface `surface` under the light `shade` ([`area_mixed`]),
/// its cloud a runaway's by the surface's deck, the lid its pass takes under the body's own
/// climate, whose row its regolith is, whether the condensates it publishes list liquid water
/// (P14.T53.b part 1 (c)) and whether it melts its water under a locked world's deck (P14.T24.j),
/// and its maturity's outcome (P14.T54.b.e).
///
/// Under the body's own climate a pass partitions and takes its conditions, its lid, its maturity
/// ([`pass_maturity`]), its regolith ([`pass_regolith`]) and its cold traps' field
/// ([`pass_cold_traps`], P14.T54.b.f), and then publishes its condensates, over that field where it
/// read one, over which its modifiers give `ferric`'s deposit ([`pass_deposits`]); the area mix
/// then reads the condensates, the deposit and the regolith on the rest.
#[must_use]
fn surface_and_cloud(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
    (shade, haze): (&HostShade, f64),
) -> SurfaceMix {
    let Some(gravity) = surface_gravity(inputs) else {
        return weightless(air, inputs, surface, shade);
    };
    let ((solved, published, _, water_ice), lid, (deposits, maturity), regolith, cold_traps) =
        match surface.climate {
            PassClimate::Given(climate_of) => (
                given_partition(air, inputs, surface, (climate_of, gravity)),
                None,
                (Vec::new(), Err(NoMaturity::NoRegolith)),
                pass_regolith(surface.regolith, inputs.material, None)
                    .with_resurfacing(pass_resurfacing(air, None)),
                Err(NoColdTraps::ClimateGiven),
            ),
            PassClimate::Spun(spun) => {
                let pending = spun_partition(air, inputs, spun);
                let lid = spun_lid(air, inputs, spun, pending.conditions.as_ref());
                let maturity = pass_maturity(air, spun, lid.as_ref());
                let at = maturity.as_ref().ok().map(|maturing| LidWeathering {
                    maturity: maturing.weathering.maturity(),
                    temperature: air.surface_temperature,
                    surface_age: maturing.parts.surface_age,
                    state: air.state,
                });
                let lid_at = lid.as_ref().map(|lid| (lid, at));
                let regolith = pass_regolith(surface.regolith, inputs.material, lid_at)
                    .with_resurfacing(pass_resurfacing(air, Some(&spun.context)));
                // The cold traps' field, of this pass's own inputs, between its regolith and its
                // publish (P14.T54.b.f; decision-p14-t54b-f-coldtraps §2 (b)).
                let cold_traps = pass_cold_traps(air, spun, &pending, (&regolith, shade));
                let partition = pending.publish(cold_traps.as_ref().ok(), air);
                #[cfg(test)]
                spun_read::record(
                    partition.2.as_ref(),
                    &partition.0,
                    &partition.1,
                    lid.as_ref(),
                );
                let deposits = pass_deposits(air, inputs, (maturity, regolith), &partition.1);
                (partition, lid, deposits, regolith, cold_traps)
            }
        };
    let ground = area_mix::sedimented_surface_bond(
        &published,
        &solved,
        (regolith.bond(air.state, shade.bands()).bond, &deposits),
        shade,
        haze,
    );
    let deck = surface.deck;
    let disc = Disc::Clouded {
        ground,
        cloud_fraction: area_mix::cloud_fraction((air.state, deck), &solved, shade),
        cloud_albedo: area_mix::cloud_albedo((air.state, deck), shade),
    };
    let bands = pass_bands(
        (air.state, deck),
        (&published, &solved),
        (&regolith, &deposits),
        (shade, haze),
    );
    let liquid_water = published
        .entry(Gas::Water.substance(), CondensatePhase::Liquid)
        .is_some();
    let deck_water = deck_water(surface.climate, &solved);
    // P14.T53.e: the state the next pass reads, by rules 5–7 on what this pass publishes; none in
    // a hothouse's re-solve, whose state is fixed (P14.T53.b part 1 (c)).
    let present = match surface.published {
        PublishedState::Climate => state_to_carry(
            air,
            (climate_class(inputs, surface.published), deck),
            || PresentFigures {
                // The record's own ice cover: under a dark lid over an ice bulk, the appended water
                // ice is its lag's exposed share from 25 (P14.T53.c).
                ice_cover: published_ice_cover(
                    &published,
                    lag::shown(
                        DarkLag::of(inputs.material, lid.as_ref().map(CrustLid::lithology)),
                        water_ice,
                    ),
                ),
                liquid_water,
                surface_temperature: air.surface_temperature,
                surface_pressure: air.surface_pressure,
                // A world locked 1:1 has no solar day; the stand-in reads no spin.
                rotation: match surface.climate {
                    PassClimate::Spun(spun) if !rotates(&spun.record.spin) => Rotation::Locked,
                    PassClimate::Spun(_) | PassClimate::Given(_) => Rotation::Turning,
                },
            },
        ),
        PublishedState::Hothouse => None,
    };
    SurfaceMix {
        disc,
        bands,
        lid,
        liquid_water,
        deck_water,
        maturity,
        cold_traps,
        present,
    }
}

/// A pass's disc in the B, V and R bins ([`DiscBands`]; the surface colour, from generator version
/// 30): [`surface_and_cloud`]'s ground, its published condensates `published` over its partition
/// `solved`, its regolith `regolith` on the rest and its deposits `deposits`, each in a band's
/// light alone, under its sediment of the haze's τ(550) `haze`, and its cloud in the state `state`
/// and `deck`, under the light `shade`. `None` below 30 ([`SurfaceColourRules::Template`]).
#[must_use]
fn pass_bands(
    (state, deck): (SurfaceState, RunawayDeck),
    (published, solved): (&Condensates, &Partition),
    (regolith, deposits): (&Regolith, &[(SubstanceId, f64)]),
    (shade, haze): (&HostShade, f64),
) -> Option<DiscBands> {
    match SurfaceColourRules::in_force() {
        SurfaceColourRules::Template => None,
        SurfaceColourRules::Surface => Some(DiscBands::of((state, deck), shade, |band, alone| {
            area_mix::sedimented_surface_bond(
                published,
                solved,
                (regolith.band_albedo(state, shade, band), deposits),
                alone,
                haze,
            )
        })),
    }
}

/// The open water a pass under the climate `climate`, its partition `solved`, holds under a locked
/// world's substellar deck (P14.T24.j; [`Atmosphere::melts_water_under_deck`]).
///
/// It is open where the pass was solved under the deck over the open water its context read and
/// its water's condensate takes the liquid in its warmest zone, the deck's ramp's own open water,
/// though its listing, the main phase near the mean, may show only ice. It is dry under the
/// stand-in's given climate and where the context read no open water: under the deck's hold, and
/// where no deck read the previous pass.
#[must_use]
fn deck_water(climate: PassClimate<'_>, solved: &Partition) -> OpenWater {
    let melts = || {
        solved
            .condensate(Gas::Water.substance())
            .is_some_and(|water| water.phases().contains(CondensatePhase::Liquid))
    };
    match climate {
        PassClimate::Spun(spun) if spun.context.holds_liquid_water() && melts() => OpenWater::Open,
        PassClimate::Spun(_) | PassClimate::Given(_) => OpenWater::Dry,
    }
}

/// What the last spun pass on a thread published (P14.T54.a's rule (f) tests): its conditions, if
/// it read any, its partition, its condensates and the lid whose row its regolith was, kept only
/// while a test asks for them ([`spun_read::capture`]).
#[cfg(test)]
pub(crate) mod spun_read {
    use core::cell::{Cell, RefCell};

    use crate::planetary::chem::partition::{Condensates, Partition};
    use crate::planetary::hooks::crust::CrustLid;
    use crate::planetary::hooks::surface::SurfaceConditions;
    use crate::units::Kelvin;

    /// What one spun pass published.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct SpunRead {
        /// Its conditions, `None` where nothing condensed.
        pub(crate) conditions: Option<SurfaceConditions>,
        /// Its partition.
        pub(crate) partition: Partition,
        /// Its condensates.
        pub(crate) condensates: Condensates,
        /// The lid it took, whose row its regolith read; `None` where it took none.
        pub(crate) lid: Option<CrustLid>,
        /// The lid every spun pass of the capture took, in order, a magma ocean's among them,
        /// which publishes no partition: the re-check's lid flips (decision-p14-t54a-passes §1.8).
        pub(crate) lids: Vec<Option<CrustLid>>,
        /// Beside each of `lids`, the warmest of the record's conditions that pass's lid built for
        /// its cometary test where the pass read no conditions of its own (`spun_lid`'s deferred
        /// range), `None` where it built none.
        pub(crate) deferred: Vec<Option<Kelvin>>,
    }

    thread_local! {
        /// Whether a test on this thread asks for the passes' publications.
        static ON: Cell<bool> = const { Cell::new(false) };
        /// The last spun pass's on this thread, a test's own.
        static LAST: RefCell<Option<SpunRead>> = const { RefCell::new(None) };
        /// The lids of the spun passes since the capture began, a test's own.
        static LIDS: RefCell<Vec<Option<CrustLid>>> = const { RefCell::new(Vec::new()) };
        /// The deferred warmest the pass now running built for its lid, if any.
        static DEFERRED: Cell<Option<Kelvin>> = const { Cell::new(None) };
        /// Beside each of `LIDS`, the deferred warmest its pass built.
        static DEFERREDS: RefCell<Vec<Option<Kelvin>>> = const { RefCell::new(Vec::new()) };
    }

    /// Records the lid a spun pass took, with the deferred warmest it built for it, where a test
    /// asks for it.
    pub(crate) fn trace(lid: Option<&CrustLid>) {
        let deferred = DEFERRED.with(Cell::take);
        if ON.with(Cell::get) {
            LIDS.with(|lids| lids.borrow_mut().push(lid.copied()));
            DEFERREDS.with(|all| all.borrow_mut().push(deferred));
        }
    }

    /// Records the warmest a spun pass's lid built of its record's conditions, where it read none
    /// of its own.
    pub(crate) fn deferred(warmest: Kelvin) {
        DEFERRED.with(|cell| cell.set(Some(warmest)));
    }

    /// Records a spun pass's publication as the thread's last, where a test asks for it.
    pub(crate) fn record(
        conditions: Option<&SurfaceConditions>,
        partition: &Partition,
        condensates: &Condensates,
        lid: Option<&CrustLid>,
    ) {
        if !ON.with(Cell::get) {
            return;
        }
        LAST.with(|last| {
            *last.borrow_mut() = Some(SpunRead {
                conditions: conditions.cloned(),
                partition: partition.clone(),
                condensates: condensates.clone(),
                lid: lid.copied(),
                lids: Vec::new(),
                deferred: Vec::new(),
            });
        });
    }

    /// How many lids, each with the deferred warmest beside it, the capture has recorded so far:
    /// the mark [`rewind`] goes back to.
    #[must_use]
    pub(crate) fn mark() -> usize {
        LIDS.with(|lids| lids.borrow().len())
    }

    /// Forgets the lids, and the deferred warmest beside each, of the spun passes recorded since
    /// `mark` ([`mark`]): a body solved once more without its deck or as a hothouse (P14.T53.b
    /// part 1 (b) and (c)) lists its published solve's passes alone, as it counts them, and every
    /// earlier body's stay.
    pub(crate) fn rewind(mark: usize) {
        LIDS.with(|lids| lids.borrow_mut().truncate(mark));
        DEFERREDS.with(|all| all.borrow_mut().truncate(mark));
    }

    /// `run`'s result and the last spun pass it published, if any, with the lids of every spun
    /// pass it ran. The switch is set back after.
    #[must_use]
    pub(crate) fn capture<R>(run: impl FnOnce() -> R) -> (R, Option<SpunRead>) {
        let before = ON.with(|on| on.replace(true));
        LAST.with(|last| last.borrow_mut().take());
        LIDS.with(|lids| lids.borrow_mut().clear());
        DEFERREDS.with(|all| all.borrow_mut().clear());
        DEFERRED.with(Cell::take);
        let result = run();
        ON.with(|on| on.set(before));
        let lids = LIDS.with(|lids| core::mem::take(&mut *lids.borrow_mut()));
        let deferred = DEFERREDS.with(|all| core::mem::take(&mut *all.borrow_mut()));
        let read = LAST.with(|last| last.borrow_mut().take());
        (
            result,
            read.map(|read| SpunRead {
                lids,
                deferred,
                ..read
            }),
        )
    }
}
