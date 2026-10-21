//! What T13.c's passes read of a body's surface beyond [`AtmosphereInputs`], and the area-mix Bond
//! albedo over it (P14.T51.a–b; [`area_mix`]).
//!
//! The passes have no cold profile, hypsometry or spin of their own until P14.T54.a gives them the
//! body's (one cold profile a pass, decision-p14-t51a-part2 §3.5). So the cover that the pass's
//! partition is published over comes from a seam, [`PassSurface`], whose stand-ins are labelled:
//! its cold area linear in area across the pass's climate ([`linear_cold_area`]), its basins
//! unknown and its water's cover from the pass's own water ([`pass_water_cover`]), and its
//! regolith's lid not known ([`Regolith::of_material`]): an airless rocky body takes the Moon's
//! 0.11, any other a row by the material of the bulk its air reads. Its light is the body's hosts',
//! built once per body before its passes, or for a single pass a host at the hottest host's
//! temperature ([`PassLight`]).

use super::{
    Atmosphere, AtmosphereInputs, ClimateOf, Gas, PartialPressures, SurfaceMaterial, SurfaceState,
};
use crate::planetary::chem::partition::{
    Basins, ClimateSpan, Condensates, Cover, Ground, Partition, WaterCover, partition,
};
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::derive::photometry::area_mix::{self, HostShade, Regolith};
use crate::planetary::hooks::figures::cover;
use crate::substance::ice_ih_density;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{Kelvin, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared};

#[cfg(doc)]
use super::atmosphere_under;

/// The share of a body's surface, 0 to 1, whose annual-mean climate lies below a temperature, over
/// the pass's climate span: the pass's cover provider (P14.T51.b; [`PassSurface::cold_area`]).
pub(crate) type ColdAreaOf<'a> = dyn Fn(&ClimateSpan, Kelvin) -> f64 + 'a;

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

/// What [`atmosphere_under`] reads of a body's surface beyond [`AtmosphereInputs`] (P14.T51.a–b):
/// the climate its partition reads, the ground its liquids lie on, the cover its condensates are
/// published over, the light of its hosts and its regolith. The passes build it once, before the
/// first.
#[derive(Clone, Copy)]
pub(crate) struct PassSurface<'a> {
    /// The climate the condensate partition reads at each step, from the step's mean surface
    /// temperature and the previous step's partial pressures.
    pub(crate) climate_of: &'a ClimateOf<'a>,
    /// The basins the liquids fill (P14.T24.b's hypsometry): the previous pass's from P14.T54.a.
    pub(crate) basins: Basins,
    /// The share of the condensed water at the surface (decision-p14-t51a-part2 §4.2).
    pub(crate) water_at_surface: f64,
    /// The pass's cover provider: the share of the surface below a temperature over the pass's
    /// final climate, one cold profile a pass from P14.T54.a.
    pub(crate) cold_area: &'a ColdAreaOf<'a>,
    /// The light of the body's hosts.
    pub(crate) light: PassLight<'a>,
    /// The pass's regolith provider, on the material of the bulk its air reads: its lid not known
    /// ([`Regolith::of_material`], which takes the stand-ins of [`Regolith::bond`]) until P14.T54.a
    /// gives T51.c's lid ([`Regolith::with_lid`]) and P14.T54.b weathers it
    /// ([`Regolith::weathered`]).
    pub(crate) regolith: &'a RegolithOf<'a>,
}

impl<'a> PassSurface<'a> {
    /// The passes' surface until P14.T54.a gives them the body's, under the light `light`: a
    /// climate with no contrast at each step's mean ([`ClimateSpan::isothermal`]), no basins
    /// known, all the condensed water at the surface, the cold area linear across the pass's
    /// climate ([`linear_cold_area`]), and the regolith's lid not known, weathered by the stand-in.
    #[must_use]
    pub(crate) fn stand_in(light: PassLight<'a>) -> Self {
        Self {
            climate_of: &isothermal_climate,
            basins: Basins::UNKNOWN,
            water_at_surface: 1.0,
            cold_area: &linear_cold_area,
            light,
            regolith: &Regolith::of_material,
        }
    }
}

/// A climate with no contrast at the step's mean `t`, whatever its column: the passes' climate
/// until P14.T54.a gives them the body's range from its spin.
#[must_use]
fn isothermal_climate(t: Kelvin, _: &PartialPressures) -> ClimateSpan {
    ClimateSpan::isothermal(t)
}

/// The passes' stand-in cold area until P14.T54.a (decision-p14-t51a-part2 §3.5): the share of the
/// surface below `t`, linear in area across `span` from its coldest zone to its warmest, as the
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

/// The passes' stand-in for water's cover until P14.T54.a, over the partition `solved` of a body in
/// the state `state` whose bulk is of `material`, on the surface `surface` over the climate
/// `climate`: P14.T24.b's rules with no hypsometry.
///
/// - The surface water is the partition's condensed water times the ground's surface share,
///   under T24.b's gate (a temperate world or a snowball; decision-p14-t51a-part2 §4.2).
/// - The potential ocean is the share its layer fills of the surface's basins, none where no
///   basins are known ([`Basins::area_filled`]): until P14.T54.a gives the passes their basins, no
///   pass's water covers any area as a sea (labelled).
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
    let layer = Metres::new(sigma / cover::liquid_water_density().value());
    let ice_density = ice_ih_density(climate.mean());
    let cold = (surface.cold_area)(climate, cover::ice_threshold(state));
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
        crust_water: false,
    }
}

/// The partition of the atmosphere `air` of the body `inputs` describe, solved once more at the
/// pass's final climate (`surface.climate_of` at its surface temperature and partial pressures)
/// as [`GlobalFigures::condensates_under`](crate::planetary::hooks::figures::GlobalFigures::condensates_under)
/// solves the record's, and its condensates published over the pass's cover; `None` for a body
/// of no mass or radius, which holds nothing.
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
    let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
    let climate = (surface.climate_of)(air.surface_temperature, &air.partials);
    let ground = Ground::new(gravity, surface.basins, surface.water_at_surface);
    let solved = partition(&climate, &ground, &air.column_inventory());
    let water = pass_water_cover(&solved, (air.state, inputs.material), surface, &climate);
    let cold_area = |t: Kelvin| (surface.cold_area)(&climate, t);
    let published = solved.publish(&Cover {
        cold_area: &cold_area,
        state: air.state,
        water,
    });
    Some((solved, published))
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
