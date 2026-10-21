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
//! Either way the regolith's lid is not known until P14.T54.a's C3 ([`Regolith::of_material`]): an
//! airless rocky body takes the Moon's 0.11, any other a row by the material of the bulk its air
//! reads. Its light is the body's hosts', built once per body before its passes, or for a single
//! pass a host at the hottest host's temperature ([`PassLight`]).

use super::{
    Atmosphere, AtmosphereInputs, ClimateOf, Gas, PartialPressures, SurfaceMaterial, SurfaceState,
};
use crate::planetary::chem::partition::{
    Basins, ClimateSpan, Condensates, Cover, CrustWater, Ground, Partition, WaterCover, partition,
};
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::derive::pass_record::{PassContext, RecordSpin, WaterGate};
use crate::planetary::derive::photometry::area_mix::{self, HostShade, Regolith};
use crate::planetary::hooks::figures::{PartitionOver, cover, partition_over};
use crate::planetary::hooks::surface::{OceanWeight, SurfaceConditions, SurfaceInputs};
use crate::substance::ice_ih_density;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{
    Kelvin, Kilograms, KilogramsPerSquareMetre, MetresPerSecondSquared, WattsPerSquareMetre,
};

#[cfg(doc)]
use super::atmosphere_under;

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
    /// The pass's regolith provider, on the material of the bulk its air reads: its lid not known
    /// ([`Regolith::of_material`], which takes the stand-ins of [`Regolith::bond`]) until P14.T54.a's
    /// C3 gives T51.c's lid ([`Regolith::with_lid`]) and P14.T54.b weathers it
    /// ([`Regolith::weathered`]).
    pub(crate) regolith: &'a RegolithOf<'a>,
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
///   the pass's air under that spin, at the context's heat flow and the θ of its air
///   ([`SurfaceConditions::derive`]), the partition over their climate on the context's ground and
///   their one cold profile ([`partition_over`]), with no ice bulk's crust listed
///   ([`CrustWater::Unlisted`]: the area mix reads it as the regolith). A column with nothing that
///   condenses reads no climate there (lever (p), L2): its partition is solved over the mean's
///   isothermal span, which no part of it reads, and nothing of the cover is read.
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
}

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
        }
    }

    /// The surface of a pass under the context `spun` holds (P14.T54.a): the body's own climate,
    /// the context's basins and water share, under the light `light`, the regolith's lid not known
    /// until C3.
    #[must_use]
    pub(crate) fn spun(spun: &'a SpunClimate<'a>, light: PassLight<'a>) -> Self {
        Self {
            basins: spun.context.basins(),
            water_at_surface: spun.context.water_share(),
            climate: PassClimate::Spun(spun),
            light,
            regolith: &Regolith::of_material,
        }
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
/// `solved` of a body in the state `state` whose bulk is of `material`, on the surface `surface`
/// over the climate `climate`: P14.T24.b's rules with no hypsometry, the cold area linear across
/// it. A body whose record holds its law takes the record's cover instead ([`SpunClimate`]).
///
/// - The surface water is the partition's condensed water times the ground's surface share,
///   under T24.b's gate (a temperate world or a snowball; decision-p14-t51a-part2 §4.2).
/// - The potential ocean is the share its layer fills of the surface's basins, none where no
///   basins are known ([`Basins::area_filled`]): in the stand-in no pass's water covers any area as
///   a sea (labelled).
/// - The ice is the cold area below T24.b's threshold ([`cover::ice_threshold`]), a `Snowball`'s
///   and an icy crust's uncapped, a silicate crust's capped by its surface water at
///   [`cover::ICE_SHEET_THICKNESS`] of ice Ih at the mean ([`cover::ice_fraction`]'s rule).
/// - No crust is listed ([`WaterCover::crust_water`]): the area mix reads an ice bulk's crust as
///   its regolith.
#[must_use]
fn pass_water_cover(
    solved: &Partition,
    (state, material): (SurfaceState, SurfaceMaterial),
    surface: &PassSurface<'_>,
    climate: &ClimateSpan,
) -> WaterCover {
    let holds = match state {
        SurfaceState::Temperate | SurfaceState::Snowball => true,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse => false,
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
    let ice_fraction = match (state, material) {
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

/// The partition of the atmosphere `air` of the body `inputs` describe, solved once more at the
/// pass's final climate, and its condensates published over the pass's cover; `None` for a body of
/// no mass or radius, which holds nothing.
///
/// - **A climate given** ([`PassClimate::Given`]): the climate at the air's surface temperature
///   and partial pressures, as
///   [`GlobalFigures::condensates_and_ice_over`](crate::planetary::hooks::figures::GlobalFigures::condensates_and_ice_over)
///   solves the record's, published over the stand-in's cover ([`pass_water_cover`]).
/// - **The body's own** ([`PassClimate::Spun`]): the record's computation ([`spun_partition`]).
#[must_use]
fn pass_condensates(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
) -> Option<(Partition, Condensates)> {
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    if !(m.is_finite() && m > 0.0 && r.is_finite() && r > 0.0) {
        return None;
    }
    let climate_of = match surface.climate {
        PassClimate::Given(climate_of) => climate_of,
        PassClimate::Spun(spun) => return Some(spun_partition(air, inputs, spun)),
    };
    let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
    let climate = climate_of(air.surface_temperature, &air.partials);
    let ground = Ground::new(gravity, surface.basins, surface.water_at_surface);
    let solved = partition(&climate, &ground, &air.column_inventory());
    let water = pass_water_cover(&solved, (air.state, inputs.material), surface, &climate);
    let cold_area = |t: Kelvin| linear_cold_area(&climate, t);
    let published = solved.publish(&Cover {
        cold_area: &cold_area,
        state: air.state,
        water,
    });
    Some((solved, published))
}

/// What a pass under its body's own climate `spun` publishes of the atmosphere `air` of the body
/// `inputs` describe (P14.T54.a; rule (f)): the record's computation of its condensates, to the
/// bit ([`SpunClimate`]).
///
/// The ground is the context's ([`PassContext::partition_ground`]): its basins and water share,
/// the surface water and potential ocean of the air's own vapour, gated by its own state and
/// retention, and the crust its craters give, with no ice bulk's crust listed. The conditions are
/// the record's of the air at the context's heat flow and that potential ocean's θ, and the
/// partition and its cover are [`partition_over`]'s. A column with nothing that condenses is
/// solved over the mean's isothermal span instead, which its partition does not read, and no part
/// of its cover is read: it publishes nothing.
#[must_use]
fn spun_partition(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    spun: &SpunClimate<'_>,
) -> (Partition, Condensates) {
    let record = &spun.record;
    let water = WaterGate::of_air(air, spun.inventory_water, record);
    let ground = spun
        .context
        .partition_ground(air, &water, record.material, CrustWater::Unlisted);
    let inventory = air.column_inventory();
    let conditions = inventory
        .condenses()
        .then(|| {
            SurfaceConditions::derive(&SurfaceInputs {
                atmosphere: air,
                equilibrium_temperature: inputs.equilibrium,
                surface_gravity: record.surface_gravity,
                radius: record.radius,
                material: record.material,
                stellar_flux: spun.stellar_flux,
                xuv_flux: spun.xuv_flux,
                spin: record.spin,
                heat_flow: spun.context.heat_flow(),
                ocean_weight: OceanWeight::of_potential_ocean(ground.potential_ocean),
            })
        })
        .flatten();
    let Some(conditions) = conditions else {
        // Nothing condenses (L2): no climate is read, and the mean's isothermal span stands in.
        let climate = ClimateSpan::isothermal(air.surface_temperature);
        let solved = partition(&climate, &ground.ground, &inventory);
        let cold_area = |t: Kelvin| linear_cold_area(&climate, t);
        let published = solved.publish(&Cover {
            cold_area: &cold_area,
            state: air.state,
            water: WaterCover {
                surface_water: ground.surface_water,
                potential_ocean: ground.potential_ocean,
                ice_fraction: 0.0,
                ice_density: ice_ih_density(air.surface_temperature),
                crust_water: ground.crust_water,
            },
        });
        #[cfg(test)]
        spun_read::record(None, &solved, &published);
        return (solved, published);
    };
    let PartitionOver {
        partition: solved,
        profile,
        water,
    } = partition_over(air, &conditions, &ground, record.table);
    let cold_area = |t: Kelvin| profile.area_below(t);
    let published = solved.publish(&Cover {
        cold_area: &cold_area,
        state: air.state,
        water,
    });
    #[cfg(test)]
    spun_read::record(Some(&conditions), &solved, &published);
    (solved, published)
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
/// fraction from the area mix over the surface `surface` in place of T13.c's table:
///
/// - a gas envelope its class's ([`area_mix::giant_bond`]) at its heated temperature and for its
///   planet class, under a full cover, which its class's albedo includes;
/// - a magma ocean the melt's 0.10 ([`area_mix::melt_bond`]), cloudless;
/// - every other state the area mix over the condensates its pass publishes and its regolith, with
///   the cloud fraction of its state and condensable at its cloud's albedo
///   ([`area_mix::under_cloud`]).
#[must_use]
pub(super) fn area_mixed(
    air: Atmosphere,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
) -> Atmosphere {
    let hottest;
    let shade = match surface.light {
        PassLight::Shade(shade) => shade,
        PassLight::HottestHost => {
            hottest = HostShade::of_host(inputs.hottest_host);
            &hottest
        }
    };
    let (albedo, cloud_fraction) = match air.state {
        SurfaceState::GasEnvelope => (
            area_mix::giant_bond(shade, inputs.heated, inputs.class),
            area_mix::cloud_share_of_state(SurfaceState::GasEnvelope),
        ),
        SurfaceState::MagmaOcean => (area_mix::melt_bond(), 0.0),
        SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball => surface_and_cloud(&air, inputs, surface, shade),
    };
    Atmosphere {
        albedo: bond_albedo(albedo, air.state, inputs.material),
        cloud_fraction,
        ..air
    }
}

/// The area-mix Bond albedo and cloud fraction of a body with a surface, the atmosphere `air` of
/// the body `inputs` describe on the surface `surface` under the light `shade` ([`area_mixed`]).
#[must_use]
fn surface_and_cloud(
    air: &Atmosphere,
    inputs: &AtmosphereInputs,
    surface: &PassSurface<'_>,
    shade: &HostShade,
) -> (f64, f64) {
    let regolith = (surface.regolith)(inputs.material)
        .bond(air.state, shade.bands())
        .bond;
    let Some((solved, published)) = pass_condensates(air, inputs, surface) else {
        return (regolith, 0.0);
    };
    let ground = area_mix::surface_bond(&published, &solved, regolith, shade);
    let cloud = area_mix::cloud_fraction(air.state, &solved, shade);
    let albedo = area_mix::under_cloud(ground, cloud, area_mix::cloud_albedo(air.state, shade));
    (albedo, cloud)
}

/// What the last spun pass on a thread published (P14.T54.a's rule (f) tests): its conditions, if
/// it read any, its partition and its condensates, kept only while a test asks for them
/// ([`spun_read::capture`]).
#[cfg(test)]
pub(crate) mod spun_read {
    use core::cell::{Cell, RefCell};

    use crate::planetary::chem::partition::{Condensates, Partition};
    use crate::planetary::hooks::surface::SurfaceConditions;

    /// What one spun pass published.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct SpunRead {
        /// Its conditions, `None` where nothing condensed.
        pub(crate) conditions: Option<SurfaceConditions>,
        /// Its partition.
        pub(crate) partition: Partition,
        /// Its condensates.
        pub(crate) condensates: Condensates,
    }

    thread_local! {
        /// Whether a test on this thread asks for the passes' publications.
        static ON: Cell<bool> = const { Cell::new(false) };
        /// The last spun pass's on this thread, a test's own.
        static LAST: RefCell<Option<SpunRead>> = const { RefCell::new(None) };
    }

    /// Records a spun pass's publication as the thread's last, where a test asks for it.
    pub(crate) fn record(
        conditions: Option<&SurfaceConditions>,
        partition: &Partition,
        condensates: &Condensates,
    ) {
        if !ON.with(Cell::get) {
            return;
        }
        LAST.with(|last| {
            *last.borrow_mut() = Some(SpunRead {
                conditions: conditions.cloned(),
                partition: partition.clone(),
                condensates: condensates.clone(),
            });
        });
    }

    /// `run`'s result and the last spun pass it published, if any. The switch is set back after.
    #[must_use]
    pub(crate) fn capture<R>(run: impl FnOnce() -> R) -> (R, Option<SpunRead>) {
        let before = ON.with(|on| on.replace(true));
        LAST.with(|last| last.borrow_mut().take());
        let result = run();
        ON.with(|on| on.set(before));
        (result, LAST.with(|last| last.borrow_mut().take()))
    }
}
