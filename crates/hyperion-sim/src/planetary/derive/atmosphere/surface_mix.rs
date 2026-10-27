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
//! **The weathered regolith** (P14.T54.b.e, held at 24, `area_mix::WEATHERED_AREA_MIX_VERSION`):
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
//! **The cold traps** (P14.T54.b.f, held at 24, `cold_traps::COLD_TRAPS_VERSION`; the record's
//! rules): under the body's own climate a pass that qualifies (a zonal profile, airless, its air
//! not collisional) reads a cold-trap field of its own inputs after its regolith and before it
//! publishes: the derivation's sky, the heat flow, crater contract and crust of the context it
//! reads, its weathered regolith's Bond albedo, airless, and its partition's condensables
//! ([`SpunColdTraps`], `cold_traps::FieldInputs`), reused from the derivation's one slot where they
//! are the same bits. It publishes over it (`cold_traps::publish_over`), and its field rides to the
//! record (`cold_traps::PassColdTraps`). Such a pass reads its conditions even where nothing in its
//! column condenses, so that the delivered water and carbon dioxide traps are its own (L2 closed,
//! decision-p14-t54b-f-coldtraps §2 (d)). A pass that reads no field publishes P14.T51.a's, with
//! why (`cold_traps::NoColdTraps`).

use super::{
    Atmosphere, AtmosphereInputs, ClimateOf, Gas, PartialPressures, PublishedState, RunawayDeck,
    SurfaceMaterial, SurfaceState,
};
use crate::planetary::chem::partition::{
    Basins, ClimateSpan, CondensatePhase, Condensates, Cover, CrustWater, Ground, Partition,
    WaterCover, partition,
};
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::derive::pass_record::{PassContext, RecordSpin, WaterGate};
use crate::planetary::derive::photometry::area_mix::{
    self, AreaMixWeathering, HostShade, RayleighFloorRules, Regolith,
};
use crate::planetary::hooks::cold_traps::{
    self, ColdTrapBody, ColdTrapSky, FieldInputs, FieldSlot, NoColdTraps, PassColdTraps, PassTraps,
    is_collisional, qualifies_by,
};
use crate::planetary::hooks::crust::{CrustBasis, CrustLid, LidAt};
use crate::planetary::hooks::figures::{
    Crust, FieldBody, PartitionGround, PartitionOver, WetEpoch, cover, partition_over,
};
use crate::planetary::hooks::modifiers::{
    DeriveModifiersError, HeliocentricOrbit, ModifierHost, ModifierParts, Primary,
    SurfaceModifiers, SurfaceWeathering, WaterHistory, oxidisable, surface_weathering,
};
use crate::planetary::hooks::surface::{
    ColdProfile, OceanWeight, SurfaceConditions, SurfaceInputs, night_floor,
    published_surface_pressure, rotates,
};
use crate::substance::{SubstanceId, ice_ih_density};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{
    Kelvin, Kilograms, KilogramsPerSquareMetre, MetresPerSecondSquared, WattsPerSquareMetre,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

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
///   is read; but not, under the cold traps' batch, a pass that can qualify for them, which reads
///   its conditions all the same (P14.T54.b.f).
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
    /// [`PassWeathering::in_force`]), or why it takes none.
    pub(crate) weathering: Result<PassWeathering<'a>, NoMaturity>,
    /// What its passes' cold traps read beyond each pass, once a derivation before them
    /// (P14.T54.b.f): the derivation's sky and its one field slot, or why they read none (the
    /// record's hold, or no seasonal orbit).
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
    /// The weathered area mix's hold, [`AreaMixWeathering::Held`]: nothing of it is computed.
    Held,
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
    /// Why a pass whose area mix reads no regolith takes no maturity: [`NoMaturity::NoRegolith`],
    /// or under the hold [`NoMaturity::Held`], as every pass's is there.
    #[must_use]
    fn no_regolith() -> Self {
        match AreaMixWeathering::in_force() {
            AreaMixWeathering::Held => Self::Held,
            AreaMixWeathering::Weathered => Self::NoRegolith,
        }
    }

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
            Self::Held => "the weathered area mix is held",
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

impl PassWeathering<'_> {
    /// What the passes' maturity reads under the weathered area mix's rules in force: `of()` under
    /// [`AreaMixWeathering::Weathered`], and nothing computed under [`AreaMixWeathering::Held`]
    /// ([`NoMaturity::Held`]). The one place the passes read the hold.
    ///
    /// # Errors
    ///
    /// [`NoMaturity::Held`] under the hold, and `of`'s error otherwise.
    pub(crate) fn in_force(
        of: impl FnOnce() -> Result<Self, NoMaturity>,
    ) -> Result<Self, NoMaturity> {
        match AreaMixWeathering::in_force() {
            AreaMixWeathering::Held => Err(NoMaturity::Held),
            AreaMixWeathering::Weathered => of(),
        }
    }
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
        SurfaceState::Temperate | SurfaceState::Snowball => true,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Tenuous => false,
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
/// pass's final climate, its condensates published over the pass's cover, and under the body's
/// own climate the record's conditions it was solved over, `None` where it read none (a climate
/// given, or a column with nothing that condenses).
type PassPartition = (Partition, Condensates, Option<SurfaceConditions>);

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
    (solved, published, None)
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
        (self.solved, published, self.conditions)
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
/// conditions (L2). Under the cold traps' batch a pass that can qualify for them
/// ([`can_take_cold_traps`]) is not: it reads its conditions, so that its delivered water and
/// carbon dioxide traps are published by it (P14.T54.b.f; decision-p14-t54b-f-coldtraps §2 (d)).
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
/// decision-p14-t54b-f-coldtraps §2 (d)): the cold traps' batch with a sky
/// ([`SpunClimate::cold_traps`]), a spin that turns under its seasonal host (the conditions'
/// zonal profile, [`rotates`]), the airless state and an air that is not collisional. False under
/// the hold, so that L2 stands there to the bit.
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
/// cold traps are under the batch with a sky; otherwise why it reads none.
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
/// a climate given's, or the hold's and no sky's where its spun climate's cold traps read none.
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
/// - and every state but a gas envelope floored at its air's own Rayleigh layer's, from
///   [`RAYLEIGH_FLOOR_VERSION`](area_mix::RAYLEIGH_FLOOR_VERSION) (P14.T24.h.a; [`floored`]).
///
/// The atmosphere also carries whether the condensates the pass published list liquid water,
/// which P14.T53.b part 1 (c)'s test reads ([`solved_as_a_hothouse`]): none for a gas envelope
/// and a magma ocean, which publish none. Beside the atmosphere and the lid it gives its
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
    let SurfaceMix {
        albedo,
        cloud_fraction,
        lid,
        liquid_water,
        maturity,
        cold_traps,
    } = match air.state {
        SurfaceState::GasEnvelope => SurfaceMix {
            albedo: area_mix::giant_bond(shade, inputs.heated, inputs.class),
            cloud_fraction: area_mix::cloud_share_of_state(SurfaceState::GasEnvelope),
            lid: None,
            liquid_water: false,
            maturity: Err(NoMaturity::no_regolith()),
            cold_traps: Err(no_cold_traps(surface, NoColdTraps::Unqualified)),
        },
        SurfaceState::MagmaOcean => {
            let lid = match surface.climate {
                PassClimate::Spun(spun) => spun_lid(air, inputs, spun, None),
                PassClimate::Given(_) => None,
            };
            SurfaceMix {
                albedo: area_mix::melt_bond(),
                cloud_fraction: 0.0,
                lid,
                liquid_water: false,
                maturity: Err(NoMaturity::no_regolith()),
                cold_traps: Err(no_cold_traps(surface, NoColdTraps::Unqualified)),
            }
        }
        SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball
        | SurfaceState::Tenuous => surface_and_cloud(air, inputs, surface, shade),
    };
    #[cfg(test)]
    if let PassClimate::Spun(_) = surface.climate {
        spun_read::trace(lid.as_ref());
    }
    let albedo = floored(albedo, air, inputs, shade);
    let runaway_deck = match air.state {
        SurfaceState::RunawayGreenhouse => surface.deck,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::Temperate
        | SurfaceState::Snowball
        | SurfaceState::Tenuous => RunawayDeck::Kept,
    };
    let air = Atmosphere {
        albedo: bond_albedo(albedo, air.state, inputs.material),
        cloud_fraction,
        runaway_deck,
        liquid_water,
        ..*air
    };
    (air, lid, maturity, cold_traps)
}

/// What a pass's area mix gives ([`area_mixed`]): its Bond albedo before its floor, its cloud
/// fraction, the lid it took, whether its condensates list liquid water, its maturity's outcome
/// and its cold traps'.
struct SurfaceMix {
    albedo: f64,
    cloud_fraction: f64,
    lid: Option<CrustLid>,
    liquid_water: bool,
    maturity: PassMaturity,
    cold_traps: PassTraps,
}

/// The generator version from which an airless body's mean surface temperature is at least its
/// night floor at its heat flow ([`airless_mean`]): 24, the 23 → 24 batch's bump, P14.T54.b
/// (P14.T24.i, before the serve gate; decision-bump-plan-after-22, "Version-24 lanes after 23",
/// V3.3).
///
/// Below it [`UnlitMeanRules::in_force`] is [`UnlitMeanRules::Held`], so that every airless mean
/// is its equilibrium temperature bit for bit. P14.T54.b deletes this hold, [`UnlitMeanRules`]
/// with its `Held` arm and the tests' switch, and its tripwire
/// (`the_unlit_mean_is_held_only_until_the_24_bump`).
pub(crate) const UNLIT_MEAN_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether an airless body's mean surface temperature is floored at its night floor (the hold,
/// [`UNLIT_MEAN_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum UnlitMeanRules {
    /// Versions 23's and before: an airless mean is its equilibrium temperature, 0 K unlit.
    Held,
    /// The 23 → 24 batch's: the larger of the equilibrium temperature and the night floor.
    Floored,
}

impl UnlitMeanRules {
    /// The rules this generator version gives: the floor from [`UNLIT_MEAN_VERSION`], none below
    /// it; in the tests, the rules a test's switch sets on its thread ([`with_unlit_mean`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = UNLIT_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= UNLIT_MEAN_VERSION {
            Self::Floored
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The unlit mean's rules a test sets on its thread, `None` for the version's.
    static UNLIT_RULES: core::cell::Cell<Option<UnlitMeanRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the unlit mean's rules `rules` on this thread, for the tests to reach 24's
/// airless means ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. P14.T54.b deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_unlit_mean<R>(rules: UnlitMeanRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<UnlitMeanRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            UNLIT_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(UNLIT_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The mean surface temperature of an airless body of equilibrium temperature `equilibrium` over
/// the surface `surface` (P14.T24.i): the larger of `equilibrium` and the night floor at the heat
/// flow of the context the pass reads, (`T_CMB`⁴ + F ÷ `ε_r` σ)^¼
/// ([`night_floor`](crate::planetary::hooks::surface::night_floor)), where the floor's rules are in
/// force ([`UnlitMeanRules`], held at 24), and `equilibrium` otherwise.
///
/// A record with no host light publishes `T_eq` = 0 K, under its own night floor of 12–20 K, and
/// `for_body` refuses it (P14.T54.a's labelled refusals); the floor gives it the mean its
/// conditions' climate already holds. A max of continuous functions is continuous, and a body
/// whose `T_eq` is above its floor keeps it to the bit. The passes' stand-in reads no heat flow,
/// so its floor is the cosmic background's. The flux sum (`T_eq`⁴ + `T_floor`⁴)^¼ for lit bodies is a
/// later refinement, since it moves every airless body (recorded).
///
/// It is the base temperature `T_base` of every body with a surface, on both paths of
/// `state_and_greenhouse` (P14.T24.i's follow-up, decision-t24i-airless-state D1.3): the
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
#[must_use]
pub(super) fn airless_mean(equilibrium: Kelvin, surface: &PassSurface<'_>) -> Kelvin {
    if UnlitMeanRules::in_force() == UnlitMeanRules::Held {
        return equilibrium;
    }
    let floor = night_floor(heat_flow(surface));
    if floor.value() > equilibrium.value() {
        floor
    } else {
        equilibrium
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

/// The Bond albedo `albedo` of the atmosphere `air` of the body `inputs` describe, under the light
/// `shade`, floored at its air's own Rayleigh layer's (P14.T24.h.a; [`area_mix::rayleigh_floor`]):
/// the larger of the two where the floor's rules are in force ([`RayleighFloorRules`], held at 24),
/// on every body with a surface, a gas envelope's class albedo excepted, and `albedo` itself
/// otherwise, or where it is not a number (which [`bond_albedo`] labels) or the body has no mass
/// or radius. The floor binds only where an air is darker than its own Rayleigh layer: a max of
/// continuous functions, continuous in the passes' map, and `albedo` to the bit wherever it is at
/// least the floor.
#[must_use]
fn floored(albedo: f64, air: &Atmosphere, inputs: &AtmosphereInputs, shade: &HostShade) -> f64 {
    let surface = match air.state {
        SurfaceState::GasEnvelope => false,
        SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball
        | SurfaceState::Tenuous => true,
    };
    if !surface || albedo.is_nan() || RayleighFloorRules::in_force() == RayleighFloorRules::Held {
        return albedo;
    }
    let Some(gravity) = surface_gravity(inputs) else {
        return albedo;
    };
    let floor = area_mix::rayleigh_floor(&air.partials, gravity, shade);
    #[cfg(test)]
    floor_read::record(floor);
    if floor > albedo { floor } else { albedo }
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
/// lid `lid` and no maturity, its row by the stand-in ([`Regolith::with_lid`]); and with a lid and
/// the maturity m and mean surface temperature of `weathered`, its row weathered at m where it
/// lies ([`Regolith::weathered_lid`]). A pass computes it before it publishes its condensates.
#[must_use]
pub(crate) fn pass_regolith(
    of_material: &RegolithOf<'_>,
    material: SurfaceMaterial,
    lid: Option<&CrustLid>,
    weathered: Option<(f64, Kelvin)>,
) -> Regolith {
    match (lid, weathered) {
        (None, _) => of_material(material),
        (Some(lid), None) => Regolith::with_lid(lid.row(), material),
        (Some(lid), Some((maturity, temperature))) => {
            Regolith::weathered_lid(lid.row(), material, maturity, temperature)
        }
    }
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
/// It reads no condensate and no water. `Err` with why where it takes none: the hold, the
/// record's case, no figures in the context it reads (C₀), no lid, or the parts refused.
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

/// The area-mix Bond albedo and cloud fraction of a body with a surface, the atmosphere `air` of
/// the body `inputs` describe on the surface `surface` under the light `shade` ([`area_mixed`]),
/// its cloud a runaway's by the surface's deck, the lid its pass takes under the body's own
/// climate, whose row its regolith is, whether the condensates it publishes list liquid water
/// (P14.T53.b part 1 (c)), and its maturity's outcome (P14.T54.b.e).
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
    shade: &HostShade,
) -> SurfaceMix {
    let Some(gravity) = surface_gravity(inputs) else {
        // A body of no mass or radius holds nothing, and takes no lid.
        let regolith = pass_regolith(surface.regolith, inputs.material, None, None);
        return SurfaceMix {
            albedo: regolith.bond(air.state, shade.bands()).bond,
            cloud_fraction: 0.0,
            lid: None,
            liquid_water: false,
            maturity: Err(NoMaturity::no_regolith()),
            cold_traps: Err(no_cold_traps(surface, NoColdTraps::Unqualified)),
        };
    };
    let ((solved, published, _), lid, (deposits, maturity), regolith, cold_traps) =
        match surface.climate {
            PassClimate::Given(climate_of) => (
                given_partition(air, inputs, surface, (climate_of, gravity)),
                None,
                (Vec::new(), Err(NoMaturity::no_regolith())),
                pass_regolith(surface.regolith, inputs.material, None, None),
                Err(NoColdTraps::ClimateGiven),
            ),
            PassClimate::Spun(spun) => {
                let pending = spun_partition(air, inputs, spun);
                let lid = spun_lid(air, inputs, spun, pending.conditions.as_ref());
                let maturity = pass_maturity(air, spun, lid.as_ref());
                let at = maturity
                    .as_ref()
                    .ok()
                    .map(|maturing| (maturing.weathering.maturity(), air.surface_temperature));
                let regolith = pass_regolith(surface.regolith, inputs.material, lid.as_ref(), at);
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
    let ground = area_mix::surface_bond(
        &published,
        &solved,
        (regolith.bond(air.state, shade.bands()).bond, &deposits),
        shade,
    );
    let deck = surface.deck;
    let cloud = area_mix::cloud_fraction((air.state, deck), &solved, shade);
    let albedo = area_mix::under_cloud(
        ground,
        cloud,
        area_mix::cloud_albedo((air.state, deck), shade),
    );
    let liquid_water = published
        .entry(Gas::Water.substance(), CondensatePhase::Liquid)
        .is_some();
    SurfaceMix {
        albedo,
        cloud_fraction: cloud,
        lid,
        liquid_water,
        maturity,
        cold_traps,
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
