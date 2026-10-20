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
//!    the coarse elevation's scale to plan 14's `σ_h` and its sea (the relief step's second entry,
//!    which counts the craters), climate, erosion, the coarse craters younger than a wet epoch's
//!    end (Design note 5), then the classes and crater state, each updating the `f64`
//!    [`Working`] state in place, every loop and sum in cell-index order.
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
//! `reference`'s four reference worlds. R09.T10.b added the palette's sources: the inputs carry the
//! body's palette, whose entries' values the server resolves from plan 14's registry, and the
//! header and the cells' substance bytes carry it. R09.T10.c made the secondary crust optional
//! (a Ceres's crust is primary, and its rock never melted), and the reference rows' densities
//! grain densities (science-r09-drainage.md §3 and §5). R09.T10.d added an undifferentiated body's
//! bulk material and the lid, which crust or bulk material the surface shows, and the reference
//! Moon's and Mars's own rows, the feldspathic highlands and the iron-rich basalt
//! (science-composition-crusts.md §1, §2, §5 and §7). R09.T10.e wrote the reading of the record,
//! [`CoarseInputs::from_record`]: every input but the crust, the condensates and the palette from
//! the record's sections at [`RECORD_TIME`], a moon's seasons from its planet's record, with those
//! three given, which P14.T54.a connects to the record. R09.T10.f gave [`CoarseInputs::for_body`]
//! a moon's planet's record, the one [`CoarseInputs::planet_of`] names, which the server fetches at
//! [`RECORD_TIME`] beside the body's, and [`ReadCoarseInputsError`] for its answer: the signature
//! P14.T54.a connects (decision-backlog-2 §3). R09.T11 filled the plates, and R09.T12.a began the
//! coarse elevation: a mobile lid's continental and oceanic populations with its sea floor, and
//! R09.T12.b its belts, trenches and arcs; a stagnant lid's one population, and R09.T12.c its
//! volcanic provinces; and R09.T12.d the coarse craters, listed and smoothed in. R09.T12.e anchors
//! the header's structural spectrum by crust class, scales the elevation so that the whole
//! surface's `σ_h` is plan 14's, the coarse craters' share through their degradation bytes, and
//! places the sea: the connected ocean that covers plan 14's ocean fraction, filled with the main
//! liquid. R09.T13.a built the climate step's zonal moist energy-balance model, which writes the
//! climate cells' sea-level temperatures and monthly anomalies on a world whose regime names it
//! ([`steps::climate`]); R09.T13.b extended it in longitude, 72 × 72 points, and to a locked world
//! in tidally locked coordinates, 72 × 144 points lit by its host's beam. R09.T13.c built the
//! other regimes: radiative equilibrium with thermal inertia for airless and thin-air worlds, a
//! column of regolith per point with the day resolved and averaged over local time, and the
//! isothermal surface of a Venus. R09.T13.d normalised the climate to plan 14's mean and signed
//! contrasts and placed the ices, and the liquids on the relief step's flood, by substance, with
//! the lapse rate. R09.T13.g gave the energy-balance model the relief, each grid point's
//! hypsometry setting its albedo, outgoing radiation and sea-ice heat capacity at the ground's
//! temperature, τ solved for the surface mean. R09.T13.e added the labelled precipitation heuristic
//! and each month's 10 m wind: Siler et al.'s Hadley-cell partition of the model's own flux,
//! centred on each month's energy-flux equator held at Guendelman and Kaspi's ascending-branch
//! cap, Priestley and Taylor's evaporation over open liquid, and Hergarten and Robl's orographic
//! precipitation over land along the recorded winds, solved in the order of each month's upwind
//! components and, within the large ones, along the flow, to 10⁻⁴ of each land cell's rate
//! (decision-r09-t13e-lfpm-convergence), the global water balanced. R09.T13.f added the cold
//! traps: [`CoarseInputs::cold_traps`], the cold-trapped part of each ice's share, with
//! [`ColdTrapInputs`], what they read of the star and the ground, both none from the record until
//! P14.T54.b, and their placement in each field cell's cold-trap fraction, read through plan 14's
//! `ColdTrapGround`, scaled to plan 14's area and held to the fraction, after the zonal parts.
//! R09.T13.h balanced the thin air's grey slab with its column's own emission, bounded at the
//! bulk gas's frost point aloft, its deficit falling as snow. Erosion and the classes are still
//! no-ops: run on any inputs, the pass yields that relief and
//! sea, with their ices and liquids, under the climate its regime names, with the inputs'
//! palette. The rest of R09.T13, R09.T14 and T15 fill the rest and R09.T16 pins the reference
//! worlds' fields.
//!
//! # Consumed items, by their paths in the code
//!
//! | Plan | Item | Path |
//! | ---- | ---- | ---- |
//! | R05 | cells, neighbours, centres | [`hyperion_surface::cube::PatchKey`]'s `edge_neighbour`, `corner_neighbours` and `vertex_dir`, and `st_to_uv` for the solid angles |
//! | R09.T1.a | the surface seed | [`SurfaceSeed`] (base's, re-exported at `planetary::hooks`) |
//! | R09.T2 | the field | [`hyperion_surface::field`]'s `CoarseField`, `FieldHeader`, `SynthesisCell`, `ClimateCell` and their quantisers; `coarse_level`, `cell_index` and `cell_at_index` |
//! | R09.T2 | the header's types | [`hyperion_surface::craters::CraterParams`], [`hyperion_surface::synth::BandSpectrum`], `ClimateModelKind` |
//! | R09.T2 (follow-up B) | the palette and the substance byte | [`hyperion_surface::field`]'s `MaterialPalette`, `PaletteEntry`, `PaletteRole` and `SynthesisCell::pack_substances`; [`hyperion_surface::substance_key::SubstanceKey`], every substance's key |
//! | 14 | the record | [`BodyRecord`](crate::planetary::record::BodyRecord)'s surface section's tag, [`RecordSection`](crate::planetary::record::RecordSection) |
//! | 14 | the surface state and material | [`SurfaceState`](crate::planetary::derive::atmosphere::SurfaceState), [`SurfaceMaterial`](crate::planetary::derive::atmosphere::SurfaceMaterial) |
//! | 14 | the surface section's members (P14.T48.e) and the bulk, figure, rotation and orbit sections | [`Surface`](crate::planetary::record::Surface)'s state, material, regime, [`SurfaceConditions`](crate::planetary::hooks::surface::SurfaceConditions) and [`GlobalFigures`](crate::planetary::hooks::figures::GlobalFigures); [`BulkProperties`](crate::planetary::record::BulkProperties), [`BodyFigure`](crate::planetary::derive::figure::BodyFigure), [`BodyFixedFrame`](crate::planetary::frames::BodyFixedFrame) with [`SeasonalSpin::of`](crate::planetary::hooks::surface::SeasonalSpin::of), [`BodyOrbit`](crate::planetary::record::BodyOrbit) and its drift |
//! | 14 | the crust, the condensates and the palette's sources | builder arguments, and [`CoarseInputs::from_record`]'s, until P14.T54.a puts them on the record |

pub mod grid;
pub mod inputs;
pub mod quantise;
#[cfg(any(test, feature = "testing"))]
pub mod reference;
pub mod steps;

pub use grid::{BuildCellGraphError, CellGraph};
pub use inputs::{
    AreaShare, BuildCoarseInputsError, ClimateRegime, CoarseInputs, CoarseInputsBuilder,
    ColdTrapInputs, Condensate, CondensatePhase, CrustInputs, Forcing, GasShare, LidCrust,
    MAX_CONDENSATES, Persistence, RECORD_TIME, ReadCoarseInputsError, SeasonalOrbit,
    SeasonsFromRecord, Spin, SurfaceInputsError, TectonicRegime, ThermalRegime, WetEpoch,
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
///     ClimateRegime, CoarseInputs, CrustInputs, Forcing, LidCrust, SeasonalOrbit, Spin,
///     SurfaceSeed, TectonicRegime, ThermalRegime, coarse_pass,
/// };
/// use hyperion_sim::units::{
///     Gigayears, Kelvin, KilogramsPerCubicMetre, Metres, MetresPerSecond, MetresPerSecondSquared,
///     Pascals, PerSquareKilometre, Radians, Seconds, WattsPerSquareMetre,
/// };
/// use hyperion_surface::craters::{
///     CraterParams, CraterParamsParts, CraterTarget, LUNAR_PROJECTILE_DENSITY, SOLAR_SYSTEM_AGE,
///     Screening, TargetClass,
/// };
/// use hyperion_surface::field::{
///     ClimateModelKind, FieldView, MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole,
/// };
/// use hyperion_surface::spheroid::Spheroid;
/// use hyperion_surface::substance_key::SubstanceKey;
///
/// // Its one lithology, with the values the server will resolve from plan 14's registry: Ceres's
/// // normal albedo, a phyllosilicate's grain density and the chondritic solidus of its dehydrated
/// // residue (`reference`'s rows). It is a primary crust, the shell of Ceres's frozen ocean, and
/// // its lid; Ceres has no secondary crust, its rock having never melted.
/// let phyllosilicate = SubstanceKey::new("phyllosilicate")?;
/// let palette = MaterialPalette::new(vec![PaletteEntry {
///     substance: phyllosilicate,
///     role: PaletteRole::PrimaryCrust,
///     normal_albedo_bvr: [0.094; 3],
///     phase_row: 0,
///     grain_density_kg_m3: 2_500.0,
///     transition: Kelvin::new(1_423.15),
///     mechanics: MechanicsFamily::Silicate,
///     // Ceres's rock does not melt, so the entry carries no liquid record.
///     liquid: None,
///     feo_mass_fraction: None,
/// }])?;
///
/// let gravity = MetresPerSecondSquared::new(0.284);
/// let craters = CraterParams::new(CraterParamsParts {
///     n_1km: PerSquareKilometre::new(0.06),
///     screening: Screening::None,
///     gravity,
///     k_target: 0.12,
///     impact_velocity: Some(MetresPerSecond::new(5_600.0)),
///     projectile_density: LUNAR_PROJECTILE_DENSITY,
///     target: CraterTarget {
///         class: TargetClass::NonPorous,
///         density: KilogramsPerCubicMetre::new(1_287.0),
///     },
///     system_age: SOLAR_SYSTEM_AGE,
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
///         primary: Some(phyllosilicate),
///         secondary: None,
///         tertiary: None,
///         bulk: None,
///         lid: LidCrust::Primary,
///         provinces: phyllosilicate,
///         melt_area_fraction: 0.0,
///     })
///     .palette(palette)
///     .build()?;
/// let field = coarse_pass(SurfaceSeed::new(0x5eed), &inputs);
/// // A body of 470 km takes the shallowest level, 6,144 cells of about 21 km.
/// assert_eq!(field.header().level().get(), 5);
/// assert_eq!(field.synthesis().len(), 6_144);
/// // Its lid names its lithology, the palette's one entry, and with no secondary crust it has
/// // no oceanic crust's.
/// assert_eq!(field.header().crust_palette()[2], Some(0));
/// assert_eq!(field.header().crust_palette()[1], None);
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
    steps::relief::run_after_craters(&pass, &mut working);
    steps::climate::run(&pass, &mut working);
    steps::erosion::run(&pass, &mut working);
    // The coarse craters younger than a wet epoch's end, placed on the eroded surface (Design
    // note 5).
    steps::craters::run_after_erosion(&pass, &mut working);
    steps::classes::run(&pass, &mut working);
    quantise(inputs, working).unwrap_or_else(|error| {
        panic!("the coarse pass left a field it cannot quantise: {error} ({error:?})")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_surface::field::{
        BoundaryKind, ClimateModelKind, Cover, CoverRange, FieldView, ResolutionCode, SynthesisCell,
    };
    use hyperion_surface::spheroid::Spheroid;
    use hyperion_surface::wire::encode_payload;
    use hyperion_testkit::order::assert_order_independent;

    /// The reference Earth's inputs on a 600 km sphere, at level 5: the pass runs every step that
    /// only the Earth reaches among the reference worlds (the moving plates and their belts,
    /// R09.T12.e's sea and reflood, the energy-balance model with its hypsometry, R09.T13.d's liquids
    /// and R09.T13.e's hydrology) on 6,144 cells (decision-r09-t13-test-cost).
    fn small_earth() -> CoarseInputs {
        reference::builder_of(&reference::earth_like())
            .figure(Spheroid::sphere(600.0e3))
            .build()
            .expect("the reference Earth's inputs hold on a smaller sphere")
    }

    /// Each of `worlds` under two seeds, its pass computed forwards, backwards, permuted and alone,
    /// gives the same field ([`assert_order_independent`]).
    fn assert_order_independent_over(worlds: &[CoarseInputs]) {
        let keys: Vec<(SurfaceSeed, usize)> = [reference::SEED, SurfaceSeed::new(0x0dd5)]
            .into_iter()
            .flat_map(|seed| (0..worlds.len()).map(move |world| (seed, world)))
            .collect();
        assert_order_independent(&keys, |&(seed, world)| coarse_pass(seed, &worlds[world]));
    }

    /// The pass is a pure function of its seed and inputs: computed forwards, backwards and
    /// interleaved, each (seed, world) gives the same field (sim-determinism, "Order
    /// independence"), which the server's cache of fields relies on (Design note 19). The small
    /// Earth stands in for the reference Earth, so that every step's code is order-tested in the
    /// fast tier, the Earth-only steps' under both seeds; the reference Earth's level-8 passes are
    /// the slow [`the_reference_earth_s_pass_is_order_independent`]'s (decision-r09-t13-test-cost
    /// §2.2).
    #[test]
    fn the_pass_is_order_independent() {
        assert_order_independent_over(&[
            reference::ceres_like(),
            reference::moon_like(),
            reference::mars_like(),
            small_earth(),
        ]);
    }

    /// The pass is order-independent on the four reference worlds themselves, the reference Earth
    /// at level 8 interleaved with the small worlds: the fast test's property at the Earth's own
    /// level, for a hidden state only level 8 would show (decision-r09-t13-test-cost §2.2).
    #[test]
    #[ignore = "slow: the four reference worlds' coarse passes in four orders under two seeds, \
                eight of them the Earth at level 8"]
    fn the_reference_earth_s_pass_is_order_independent() {
        assert_order_independent_over(&[
            reference::ceres_like(),
            reference::moon_like(),
            reference::mars_like(),
            reference::earth_like(),
        ]);
    }

    /// The whole field's payload, every cell surveyed at the finest code.
    fn bytes(field: &CoarseField) -> Vec<u8> {
        let count = field.header().level().cell_count();
        let cover =
            Cover::from_ranges([CoverRange::new(0, count, ResolutionCode::FINEST).unwrap()])
                .unwrap();
        encode_payload(field, &cover, None)
    }

    /// Each world's pass, run twice, yields the same field and the same payload, its header
    /// carrying the inputs; and the small Earth's field keeps reaching the steps only the Earth
    /// reaches: a sea and land, a plate boundary that is not a transform, the energy-balance model,
    /// a main liquid and rain (decision-r09-t13-test-cost §3.2). The reference Earth's level 8
    /// is held by `reference`'s `the_reference_worlds_are_the_bodies_they_name`.
    #[test]
    fn the_pass_yields_a_valid_field_twice_with_identical_bytes() {
        let mut small_earth_field = None;
        for (name, inputs, level) in [
            ("Ceres", reference::ceres_like(), 5),
            ("Moon", reference::moon_like(), 6),
            ("Mars", reference::mars_like(), 7),
            ("small Earth", small_earth(), 5),
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
            assert_eq!(first.header().palette(), inputs.palette(), "{name}");
            assert_eq!(
                first.header().crust_palette(),
                inputs.crust_palette(),
                "{name}"
            );
            assert_eq!(first.header().main_liquid(), inputs.main_liquid(), "{name}");
            if name == "small Earth" {
                small_earth_field = Some(first);
            }
        }

        // The small Earth keeps reaching the Earth-only steps: R09.T12.e's flood and the land it
        // leaves, a mobile lid's belts and ridges, the energy-balance model, R09.T13.d's liquid and
        // R09.T13.e's rain.
        let field = small_earth_field.expect("the small Earth's row ran");
        let cells = field.synthesis();
        assert!(
            cells.iter().any(SynthesisCell::is_under_water),
            "the small Earth has a sea"
        );
        assert!(
            cells.iter().any(|cell| !cell.is_under_water()),
            "the small Earth has land"
        );
        assert!(
            cells.iter().any(|cell| matches!(
                cell.boundary,
                BoundaryKind::Subduction | BoundaryKind::Collision | BoundaryKind::Divergent
            )),
            "the small Earth has a plate boundary that is not a transform"
        );
        assert_eq!(
            field.header().climate_model(),
            ClimateModelKind::EnergyBalance
        );
        assert!(
            field.header().main_liquid().is_some(),
            "the small Earth has a main liquid"
        );
        assert!(
            field.climate_layer().iter().any(|cell| cell
                .month_precipitation
                .iter()
                .any(|month| month.rate_kg_per_m2_s() > 0.0)),
            "the small Earth rains somewhere"
        );
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
