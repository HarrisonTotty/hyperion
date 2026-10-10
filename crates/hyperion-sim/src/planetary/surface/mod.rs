//! The coarse pass: a body's surface at one cube-sphere level, computed once per body on the
//! server and quantised into the [`CoarseField`] that both sides' synthesis reads (rendering plan
//! R09, R09.T10–T16; the brainstorm's "The coarse global pass, once per planet").
//!
//! [`coarse_pass`] is a pure function of the body's [`SurfaceSeed`] and its [`CoarseInputs`], plan
//! 14's global figures (Design note 3): it reads no clock, keeps no state and spawns no thread, so
//! the server runs it as one bulk job and caches the field it returns, never storing it (Design
//! notes 5 and 19). The field's types, its quantisation, its codec and the whole local synthesis
//! are the surface crate's (`hyperion_surface`), which the client's workers run too; this module
//! hands its field to that crate as data (Design note 1).
//!
//! # The pass
//!
//! 1. The grid: the body's level from its radius ([`coarse_level`], Design note 4), the field's
//!    cells and its climate layer's one level above as [`CellGraph`]s.
//! 2. The steps, in the brainstorm's order ([`steps`]): plates, coarse elevation, coarse craters,
//!    climate, erosion, then the classes and crater state, each updating the `f64`
//!    [`Working`](steps::Working) state in place, every loop and sum in cell-index order.
//! 3. The quantiser ([`quantise()`]): the working state turned into the field's codes once, so
//!    that the server's own synthesis and collision read exactly what a client receives (Design
//!    note 17).
//!
//! The surface seed opens the `surface.coarse.*` tags of the surface crate's registry, each with
//! the one key form its row documents, and nothing else (Design note 2).
//!
//! # What is built
//!
//! R09.T10 built the frame: the inputs and their builder, [`CoarseInputs::for_body`] (which answers
//! [`SurfaceInputsError::NotModelled`] until plan 14's surface section carries values), the cell
//! graph, the quantiser, the six steps as no-ops and, behind the crate's `testing` feature,
//! `reference`'s four reference worlds. Run on any inputs, the pass so far yields its initial
//! state: a smooth, dry sphere at the datum under a still climate at the body's mean surface
//! temperature. R09.T11–T15 fill the steps and R09.T16 pins the reference worlds' fields.
//!
//! # Consumed items, by their paths in the code
//!
//! | Plan | Item | Path |
//! | ---- | ---- | ---- |
//! | R05 | cells, neighbours, centres | [`hyperion_surface::cube::PatchKey`]'s `edge_neighbour`, `corner_neighbours` and `vertex_dir`, and `st_to_uv` for the solid angles |
//! | R09.T1.a | the surface seed | [`SurfaceSeed`] (base's, re-exported at `planetary::hooks`) |
//! | R09.T2 | the field | [`hyperion_surface::field`]'s `CoarseField`, `FieldHeader`, `SynthesisCell`, `ClimateCell` and their quantisers; `coarse_level`, `cell_index` and `cell_at_index` |
//! | R09.T2 | the header's types | [`hyperion_surface::craters::CraterParams`], [`hyperion_surface::synth::BandSpectrum`], `ClimateModelKind` |
//! | 14 | the record | [`BodyRecord`](crate::planetary::record::BodyRecord)'s surface section's tag, [`RecordSection`](crate::planetary::record::RecordSection) |
//! | 14 | the surface state and material | [`SurfaceState`](crate::planetary::derive::atmosphere::SurfaceState), [`SurfaceMaterial`](crate::planetary::derive::atmosphere::SurfaceMaterial) |
//! | 14 | everything else of Design note 3 | builder arguments until P14.T48 and P14.T54.a put them on the record |

pub mod grid;
pub mod inputs;
pub mod quantise;
#[cfg(any(test, feature = "testing"))]
pub mod reference;
pub mod steps;

pub use grid::{BuildCellGraphError, CellGraph};
pub use inputs::{
    AreaShare, BuildCoarseInputsError, ClimateRegime, CoarseInputs, CoarseInputsBuilder,
    Condensate, CondensatePhase, CrustInputs, Forcing, GasShare, MAX_CONDENSATES, Persistence,
    SeasonalOrbit, Spin, SubstanceRef, SurfaceInputsError, TectonicRegime, ThermalRegime, WetEpoch,
};
pub use quantise::{QuantiseFieldError, quantise};

use hyperion_surface::field::{CoarseField, coarse_level};

pub use crate::planetary::hooks::SurfaceSeed;
use steps::{Pass, Working};

/// The coarse field of the body of `inputs`, under its surface seed `seed` (Design notes 3–5 and
/// 17): the grid at the body's level, the six steps in order, and the quantiser.
///
/// The same seed and inputs give the same field to the bit, whatever was computed before.
///
/// # Panics
///
/// If a step leaves a value that no code of the field holds, or records that break a rule of the
/// field: a fault of that step. The message carries the [`QuantiseFieldError`] whole, with the cell
/// or climate cell, the value's name and its cause.
///
/// # Examples
///
/// A Ceres-like airless body, every figure given to the builder as plan 14 will give it:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
/// use hyperion_sim::planetary::surface::{
///     ClimateRegime, CoarseInputs, CrustInputs, Forcing, SeasonalOrbit, Spin, SubstanceRef,
///     SurfaceSeed, TectonicRegime, ThermalRegime, coarse_pass,
/// };
/// use hyperion_sim::units::{
///     Gigayears, Kelvin, Metres, MetresPerSecondSquared, Pascals, PerSquareKilometre, Radians,
///     Seconds, WattsPerSquareMetre,
/// };
/// use hyperion_surface::craters::{CraterParams, CraterParamsParts, Screening};
/// use hyperion_surface::field::{ClimateModelKind, FieldView};
/// use hyperion_surface::spheroid::Spheroid;
///
/// let gravity = MetresPerSecondSquared::new(0.284);
/// let craters = CraterParams::new(CraterParamsParts {
///     n_1km: PerSquareKilometre::new(0.06),
///     screening: Screening::None,
///     gravity,
///     k_target: 0.12,
///     impact_velocity: None,
/// })?;
/// let inputs = CoarseInputs::builder()
///     .figure(Spheroid::sphere(469.7e3))
///     .gravity(gravity)
///     .sigma_h(Metres::new(2_080.0))
///     .mean_surface_temperature(Kelvin::new(163.0))
///     .equator_pole_contrast_k(55.0)
///     .day_night_contrast_k(100.0)
///     .spin(Spin {
///         obliquity: Radians::new(0.07),
///         rotation_period: Seconds::new(32_667.0),
///         solar_day: Some(Seconds::new(32_674.0)),
///         equinox_true_anomaly: Radians::ZERO,
///     })
///     .host_flux(WattsPerSquareMetre::new(178.0))
///     .seasonal_orbit(SeasonalOrbit { eccentricity: 0.076, period: Seconds::new(1.45e8) })
///     .surface_pressure(Pascals::ZERO)
///     .surface(SurfaceState::Airless, SurfaceMaterial::Rock)
///     .tectonics(TectonicRegime::StagnantLid, 0.0)
///     .volcanism(0.0)
///     .heat_flow(WattsPerSquareMetre::new(0.002))
///     .surface_age(Gigayears::new(4.0))
///     .craters(craters)
///     .climate(ClimateRegime {
///         thermal: ThermalRegime::AirlessLike,
///         forcing: Forcing::Seasonal,
///         condensable: None,
///         model: ClimateModelKind::RadiativeEquilibrium,
///     })
///     .crust(CrustInputs {
///         primary: None,
///         secondary: SubstanceRef::new("phyllosilicate"),
///         tertiary: None,
///         provinces: SubstanceRef::new("phyllosilicate"),
///         melt_area_fraction: 0.0,
///     })
///     .build()?;
/// let field = coarse_pass(SurfaceSeed::new(0x5eed), &inputs);
/// // A body of 470 km takes the shallowest level, 6,144 cells of about 21 km.
/// assert_eq!(field.header().level().get(), 5);
/// assert_eq!(field.synthesis().len(), 6_144);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn coarse_pass(seed: SurfaceSeed, inputs: &CoarseInputs) -> CoarseField {
    let level = coarse_level(inputs.radius());
    let grid = CellGraph::for_field(level);
    let climate_grid = CellGraph::for_climate(level);
    let pass = Pass::new(seed, inputs, &grid, &climate_grid);
    let mut working = Working::initial(&pass);
    steps::plates::run(&pass, &mut working);
    steps::relief::run(&pass, &mut working);
    steps::craters::run(&pass, &mut working);
    steps::climate::run(&pass, &mut working);
    steps::erosion::run(&pass, &mut working);
    steps::classes::run(&pass, &mut working);
    quantise(inputs, working).unwrap_or_else(|error| {
        panic!("the coarse pass left a field it cannot quantise: {error} ({error:?})")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_surface::field::{Cover, CoverRange, FieldView, ResolutionCode};
    use hyperion_surface::wire::encode_payload;
    use hyperion_testkit::order::assert_order_independent;

    /// The pass is a pure function of its seed and inputs: computed forwards, backwards and
    /// interleaved, each (seed, world) gives the same field (sim-determinism, "Order
    /// independence"), which the server's cache of fields relies on (Design note 19).
    #[test]
    fn the_pass_is_order_independent() {
        let worlds = [reference::ceres_like(), reference::moon_like()];
        let keys: Vec<(SurfaceSeed, usize)> = [reference::SEED, SurfaceSeed::new(0x0dd5)]
            .into_iter()
            .flat_map(|seed| (0..worlds.len()).map(move |world| (seed, world)))
            .collect();
        assert_order_independent(&keys, |&(seed, world)| coarse_pass(seed, &worlds[world]));
    }

    /// The whole field's payload, every cell surveyed at the finest code.
    fn bytes(field: &CoarseField) -> Vec<u8> {
        let count = field.header().level().cell_count();
        let cover =
            Cover::from_ranges([CoverRange::new(0, count, ResolutionCode::FINEST).unwrap()])
                .unwrap();
        encode_payload(field, &cover, None)
    }

    #[test]
    fn a_no_op_pass_yields_a_valid_field_twice_with_identical_bytes() {
        for (name, inputs, level) in [
            ("Ceres", reference::ceres_like(), 5),
            ("Moon", reference::moon_like(), 6),
            ("Mars", reference::mars_like(), 7),
            ("Earth", reference::earth_like(), 8),
        ] {
            let first = coarse_pass(reference::SEED, &inputs);
            let second = coarse_pass(reference::SEED, &inputs);
            assert_eq!(first.header().level().get(), level, "{name}");
            assert_eq!(first, second, "{name}");
            assert_eq!(bytes(&first), bytes(&second), "{name}");
            assert_eq!(
                first.header().radius(),
                inputs.radius(),
                "{name}'s header carries its radius"
            );
            assert_eq!(first.header().figure(), inputs.figure(), "{name}");
            assert_eq!(first.header().craters(), inputs.craters(), "{name}");
            assert_eq!(first.header().months(), inputs.months(), "{name}");
            assert!(
                first.synthesis().iter().all(|c| c.elevation_mm == 0),
                "{name}"
            );
        }
    }

    #[test]
    fn the_field_s_header_carries_the_inputs_figures() {
        let ceres = reference::ceres_like();
        let field = coarse_pass(reference::SEED, &ceres);
        let header = field.header();
        assert_eq!(header.body(), ceres.body());
        assert_eq!(header.surface_age(), ceres.surface_age());
        assert_eq!(header.surface_pressure(), ceres.surface_pressure());
        assert_eq!(
            header.reference_temperature(),
            ceres.mean_surface_temperature()
        );
        assert_eq!(header.climate_model(), ceres.climate().model);
        assert_eq!(header.albedo_scale(), None);
        assert_ne!(header.body(), reference::moon_like().body());
    }
}
