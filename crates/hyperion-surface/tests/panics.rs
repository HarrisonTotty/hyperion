//! Every test of `hyperion-surface` that expects a panic, in a test binary of its own.
//!
//! On `wasm32-unknown-unknown` a panic is a trap, after which every later test of the same binary
//! runs in a best-effort state, so these tests are kept apart from the goldens and the unit tests
//! (plan R04, Design note 12). Each states `expected`, because on that target a bare
//! `should_panic` passes on any trap.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_base::units::{
    Gigayears, Kelvin, Metres, MetresPerSecondSquared, Pascals, PerSquareKilometre, Radians,
    SquareMetres,
};
use hyperion_surface::craters::{self, CraterParams, CraterParamsParts, Screening};
use hyperion_surface::cube::{Face, MAX_LEVEL, PatchKey, unit_dir, xyz_to_face_uv};
use hyperion_surface::field::{
    BodyRef, BoundaryKind, ClimateCell, ClimateModelKind, CoarseField, CoarseLevel, Cover, Crust,
    FieldHeader, FieldHeaderParts, FieldView, FlowDirection, LogArea, LogPrecipitation, LogRunoff,
    LogSteepness, MaterialPalette, PrecipitationSource, SurfaceClass, SynthesisCell, Wind,
    boundary_diameter, cell_index, coarse_level, month_at, month_blend,
};
use hyperion_surface::geometry::{finest_level, vertex_spacing};
use hyperion_surface::noise::{LatticeCache, NoiseKey, Octave, gradient_noise};
use hyperion_surface::num;
use hyperion_surface::patch::vertex::{PatchTerms, face_difference_morph_f32};
use hyperion_surface::patch::{BakeOptions, NormalScale, VertexPath, bake_patch};
use hyperion_surface::spheroid::Spheroid;
use hyperion_surface::synth::channels::{
    Channels, hack_area, head_length, main_stream_length, tributary_rise,
};
use hyperion_surface::synth::interp::{CellValues, interpolate};
use hyperion_surface::synth::relief::{
    PLAIN_STRUCTURE, RIDGED_STRUCTURE, Relief, StructuralVariance, finest_octave, nyquist_degree,
    octave_bound, octave_rms, octave_spacing,
};
use hyperion_surface::synth::{BandSpectrum, SpectrumShape, unresolved_variance};
use hyperion_surface::test_planet::{TEST_PLANET, octaves};
use hyperion_surface::wire::encode_payload;

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn octave() -> Octave {
    Octave::new(
        3,
        100.0,
        IDENTITY,
        [0.0; 3],
        NoiseKey::TestPlanet(hyperion_base::Seed::new(1)),
    )
}

#[test]
#[should_panic(expected = "a direction must be finite and non-zero")]
fn the_zero_vector_has_no_face() {
    let _ = xyz_to_face_uv([0.0, -0.0, 0.0]);
}

#[test]
#[should_panic(expected = "a direction must be finite and non-zero")]
fn a_nan_vector_has_no_face() {
    let _ = xyz_to_face_uv([f64::NAN, 1.0, 0.0]);
}

#[test]
#[should_panic(expected = "a direction must be finite and non-zero")]
fn the_zero_vector_has_no_unit_direction() {
    let _ = unit_dir([0.0, 0.0, 0.0]);
}

#[test]
#[should_panic(expected = "has no children")]
fn a_patch_at_the_maximum_level_has_no_children() {
    let _ = PatchKey::new(Face::PosZ, MAX_LEVEL, 0, 0)
        .unwrap()
        .children();
}

#[test]
#[should_panic(expected = "is outside a patch of 64 quads")]
fn a_vertex_beyond_the_patch_is_refused() {
    let _ = PatchKey::root(Face::PosX).vertex_dir(65, 0);
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn a_zero_radius_has_no_finest_level() {
    let _ = finest_level(0.0);
}

#[test]
#[should_panic(expected = "is above the maximum")]
fn spacing_above_the_maximum_level_is_refused() {
    let _ = vertex_spacing(6.371e6, MAX_LEVEL + 1);
}

#[test]
#[should_panic(expected = "min of a NaN")]
fn min_refuses_a_nan() {
    let _ = num::min(f64::NAN, 1.0);
}

#[test]
#[should_panic(expected = "max of a NaN")]
fn max_refuses_a_nan() {
    let _ = num::max(1.0, f64::NAN);
}

#[test]
#[should_panic(expected = "a height must be finite")]
fn assert_finite_refuses_infinity() {
    let _ = num::assert_finite(f64::INFINITY);
}

#[test]
#[should_panic(expected = "an octave index is 0 to 31")]
fn an_octave_index_above_31_is_refused() {
    let _ = Octave::new(
        32,
        1.0,
        IDENTITY,
        [0.0; 3],
        NoiseKey::TestPlanet(hyperion_base::Seed::new(1)),
    );
}

#[test]
#[should_panic(expected = "an octave's rotation must be orthonormal")]
fn a_rotation_that_is_not_orthonormal_is_refused() {
    let stretched = [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let _ = Octave::new(
        3,
        1.0,
        stretched,
        [0.0; 3],
        NoiseKey::TestPlanet(hyperion_base::Seed::new(1)),
    );
}

#[test]
#[should_panic(expected = "a covered ball must be finite")]
fn a_cover_of_an_infinite_ball_is_refused() {
    LatticeCache::new().cover(&octave(), [0.0; 3], f64::INFINITY);
}

#[test]
#[should_panic(expected = "beyond the key's 32 bits")]
fn a_point_beyond_the_keys_reach_is_refused() {
    let _ = gradient_noise([1e15, 0.0, 0.0], &octave(), &mut LatticeCache::new());
}

#[test]
#[should_panic(expected = "is beyond the finest")]
fn an_octave_beyond_the_finest_has_no_spacing() {
    let _ = octaves::spacing_m(octaves::FINEST_OCTAVE + 1);
}

#[test]
#[should_panic(expected = "a skirt margin must be finite and non-negative")]
fn a_negative_skirt_margin_is_refused() {
    let opts = BakeOptions {
        vertex_path: VertexPath::BakedOffsets,
        normals: NormalScale::Mesh,
        skirt_m: -1.0,
    };
    let _ = bake_patch(
        &TEST_PLANET,
        PatchKey::root(Face::PosX),
        &opts,
        &mut LatticeCache::new(),
    );
}

#[test]
#[should_panic(expected = "is outside a patch")]
fn a_morph_target_beyond_the_patch_is_refused() {
    let terms = PatchTerms::new(PatchKey::root(Face::PosX), &Spheroid::WGS84, 0.0).narrow();
    let _ = face_difference_morph_f32(&terms, 65, 1, |_, _| 0.0);
}

#[test]
#[should_panic(expected = "a sphere's radius must be finite and positive")]
fn a_sphere_of_no_radius_is_refused() {
    let _ = Spheroid::sphere(0.0);
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn a_body_of_no_radius_has_no_coarse_level() {
    let _ = coarse_level(Metres::ZERO);
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn a_body_of_no_radius_has_no_boundary_diameter() {
    let _ = boundary_diameter(CoarseLevel::MIN, Metres::new(f64::NAN));
}

#[test]
#[should_panic(expected = "cell indices number levels up to 14")]
fn a_cell_below_level_14_has_no_index() {
    let _ = cell_index(PatchKey::new(Face::PosZ, 15, 0, 0).unwrap());
}

#[test]
#[should_panic(expected = "a direction must be finite and non-zero")]
fn the_zero_vector_is_in_no_patch() {
    let _ = PatchKey::containing(3, [0.0; 3]);
}

#[test]
#[should_panic(expected = "a cover holds cells below u32::MAX")]
fn a_cover_cannot_hold_the_last_index() {
    let _ = Cover::from_cells([u32::MAX]);
}

#[test]
#[should_panic(expected = "a cover of level 5 holds cells below 6144")]
fn a_cover_past_its_level_has_no_margin() {
    let _ = Cover::from_cells([6_144]).with_margin(CoarseLevel::MIN, 5);
}

/// A dry, flat, airless field of a Ceres-sized body at level 5, with no crater, built from the
/// public types alone (this binary sees no `testing` feature).
fn level_five_field() -> CoarseField {
    level_five_field_flowing(FlowDirection::Terminal)
}

/// A level-5 field whose every cell's water goes `flow`, with a channel where it goes anywhere.
fn level_five_field_flowing(flow: FlowDirection) -> CoarseField {
    level_five_field_of(BodyRef::new(1, 2), flow)
}

/// [`level_five_field_flowing`] of the body `body`.
fn level_five_field_of(body: BodyRef, flow: FlowDirection) -> CoarseField {
    level_five_field_weathered(body, flow, None)
}

/// [`level_five_field_of`] with its ground's space-weathering maturity `maturity` at the Moon's
/// rate, or neither.
fn level_five_field_weathered(
    body: BodyRef,
    flow: FlowDirection,
    maturity: Option<f64>,
) -> CoarseField {
    let radius = 4.697e5;
    let craters = CraterParams::new(CraterParamsParts {
        n_1km: PerSquareKilometre::ZERO,
        screening: Screening::None,
        gravity: MetresPerSecondSquared::new(0.28),
        k_target: 1.0,
        impact_velocity: None,
        projectile_density: craters::LUNAR_PROJECTILE_DENSITY,
        target: craters::LUNAR_TARGET,
        system_age: craters::SOLAR_SYSTEM_AGE,
    })
    .unwrap();
    let header = FieldHeader::new(FieldHeaderParts {
        body,
        radius: Metres::new(radius),
        figure: Spheroid::from_volumetric(radius, 0.0).unwrap(),
        sea_level: Metres::ZERO,
        lapse_rate_k_per_m: 0.0,
        spectrum: BandSpectrum::anchored(
            SpectrumShape::SILICATE,
            Metres::new(radius),
            Metres::ZERO,
        ),
        craters,
        climate_model: ClimateModelKind::RadiativeEquilibrium,
        precipitation: PrecipitationSource::Heuristic,
        realised_sigma_h: Metres::ZERO,
        realised_relief: Metres::ZERO,
        months: 1,
        season_eccentricity: 0.0,
        reference_temperature: Kelvin::new(160.0),
        temperature_step: 0,
        anomaly_step: 0,
        surface_age: Gigayears::new(4.0),
        surface_pressure: Pascals::ZERO,
        albedo_scale: None,
        palette: MaterialPalette::default(),
        crust_palette: [None; 4],
        main_liquid: None,
        maturity,
        organic_cover: None,
        weathering_rate: maturity.map(|_| 1.0),
    })
    .unwrap();
    let cell = SynthesisCell {
        elevation_mm: 0,
        boundary_distance_km: SynthesisCell::NO_BOUNDARY_KM,
        plate: 0,
        crust: Crust::Lid,
        boundary: BoundaryKind::Absent,
        boundary_obliquity: 0,
        flow,
        drainage: LogArea::new(30_000),
        steepness: if flow == FlowDirection::Terminal {
            LogSteepness::ZERO
        } else {
            LogSteepness::new(64)
        },
        runoff: LogRunoff::NONE,
        catchment_runoff: LogRunoff::NONE,
        water_surface_mm: 0,
        ice: 0,
        substances: SynthesisCell::NO_SUBSTANCES,
        class: SurfaceClass::UNCLASSIFIED,
        crater_state: 0,
        seafloor: SynthesisCell::NO_SEAFLOOR,
    };
    let climate = ClimateCell {
        sea_level_temperature: 0,
        month_anomaly: [0; 12],
        month_precipitation: [LogPrecipitation::NONE; 12],
        wind: [Wind::CALM; 12],
    };
    CoarseField::new(header, vec![cell; 6_144], vec![climate; 1_536], vec![]).unwrap()
}

#[test]
#[should_panic(expected = "a cover of level 5 holds cells below 6144")]
fn a_payload_of_cells_past_its_field_is_refused() {
    let field = level_five_field();
    let _ = encode_payload(&field, &Cover::from_cells([0, 6_200]), None);
}

/// Cells of level 0, one a face, which the interpolant does not read.
struct WholeFaces;

impl CellValues for WholeFaces {
    fn level(&self) -> u8 {
        0
    }

    fn value(&self, _: PatchKey) -> Option<f64> {
        Some(0.0)
    }
}

#[test]
#[should_panic(expected = "the interpolant reads levels 1 to 24, not level 0")]
fn the_interpolant_refuses_level_0() {
    let _ = interpolate(&WholeFaces, [0.0, 0.0, 1.0]);
}

#[test]
#[should_panic(expected = "a mean anomaly must be finite")]
fn a_time_of_no_mean_anomaly_has_no_month_blend() {
    let _ = month_blend(level_five_field().header(), Radians::new(f64::NAN));
}

#[test]
#[should_panic(expected = "a mean anomaly must be finite")]
fn a_time_of_no_mean_anomaly_has_no_month() {
    let _ = month_at(level_five_field().header(), Radians::new(f64::INFINITY));
}

fn spectrum() -> BandSpectrum {
    BandSpectrum::anchored(
        SpectrumShape::SILICATE,
        Metres::new(4.697e5),
        Metres::new(60.0),
    )
}

#[test]
#[should_panic(expected = "a Nyquist degree is for levels 0 to 40")]
fn a_nyquist_degree_past_level_40_is_refused() {
    let _ = nyquist_degree(41);
}

#[test]
#[should_panic(expected = "an octave index is 0 to 31")]
fn an_octave_spacing_past_octave_31_is_refused() {
    let _ = octave_spacing(Metres::new(6.371e6), 32);
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn the_finest_octave_of_no_body_is_refused() {
    let _ = finest_octave(Metres::ZERO);
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn the_structural_variance_of_no_body_is_refused() {
    let _ = StructuralVariance::new(&spectrum(), CoarseLevel::MIN, Metres::ZERO);
}

#[test]
#[should_panic(expected = "level 0 has no band below a coarser level")]
fn octave_0_has_no_rms() {
    let _ = octave_rms(&spectrum(), 0);
}

#[test]
#[should_panic(expected = "level 0 has no band below a coarser level")]
fn octave_0_has_no_bound() {
    let _ = octave_bound(&spectrum(), 0);
}

#[test]
#[should_panic(expected = "an octave index is 0 to 31")]
fn an_octave_past_31_has_no_bound() {
    let _ = octave_bound(&spectrum(), 32);
}

#[test]
#[should_panic(expected = "a lag must be non-negative")]
fn a_structure_function_at_a_negative_lag_is_refused() {
    let _ = PLAIN_STRUCTURE.at(-1.0);
}

#[test]
#[should_panic(expected = "a lag must be non-negative")]
fn a_structure_function_at_a_nan_lag_is_refused() {
    let _ = RIDGED_STRUCTURE.at(f64::NAN);
}

#[test]
#[should_panic(expected = "degrees start at 1")]
fn the_variance_from_degree_0_is_refused() {
    let _ = spectrum().variance_from_degree(0);
}

#[test]
#[should_panic(expected = "runs backwards")]
fn a_band_of_degrees_that_runs_backwards_is_refused() {
    let _ = spectrum().band_variance(5, 3);
}

#[test]
#[should_panic(expected = "level 0 has no band below a coarser level")]
fn level_0_has_no_band() {
    let _ = spectrum().level_variance(0);
}

#[test]
#[should_panic(expected = "a degree must be finite and at least 1")]
fn the_degree_variance_below_degree_1_is_refused() {
    let _ = spectrum().degree_variance(0.5);
}

#[test]
#[should_panic(expected = "a degree must be finite and at least 1")]
fn the_degree_variance_at_no_degree_is_refused() {
    let _ = spectrum().degree_variance(f64::NAN);
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn a_spectrum_anchored_on_no_body_is_refused() {
    let _ = BandSpectrum::anchored(SpectrumShape::SILICATE, Metres::ZERO, Metres::new(60.0));
}

#[test]
#[should_panic(expected = "an anchor RMS must be finite and non-negative")]
fn a_spectrum_anchored_at_a_negative_rms_is_refused() {
    let _ = BandSpectrum::anchored(SpectrumShape::SILICATE, Metres::new(1e6), Metres::new(-1.0));
}

#[test]
#[should_panic(expected = "a spectrum shape must give a valid law on this body")]
fn a_spectrum_shape_whose_break_lies_beyond_every_degree_is_refused() {
    // A break wavelength of a sub-normal metre puts the break degree at infinity.
    let shape = SpectrumShape::new(2.0, Metres::new(1e-310), 3.0).unwrap();
    let _ = BandSpectrum::anchored(shape, Metres::new(1e6), Metres::new(60.0));
}

#[test]
#[should_panic(expected = "a wavelength must be finite and positive")]
fn an_unresolved_variance_below_no_wavelength_is_refused() {
    let field = level_five_field();
    let cell = PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap();
    let _ = unresolved_variance(&field, cell, Metres::ZERO);
}

#[test]
#[should_panic(expected = "the cell is not of the field's level")]
fn an_unresolved_variance_of_another_level_s_cell_is_refused() {
    let field = level_five_field();
    let cell = PatchKey::containing(6, [0.0, 0.0, 1.0]).unwrap();
    let _ = unresolved_variance(&field, cell, Metres::new(1e3));
}

#[test]
#[should_panic(expected = "a covered ball must be finite")]
fn a_relief_cover_of_negative_radius_is_refused() {
    let field = level_five_field();
    let relief = Relief::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let mut cache = hyperion_surface::synth::SynthCache::new();
    relief.cover(&field, [0.0, 0.0, 1.0], Metres::new(-1.0), 31, &mut cache);
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn a_relief_octave_rms_past_octave_31_is_refused() {
    let field = level_five_field();
    let relief = Relief::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let _ = relief.octave_rms(32);
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn a_relief_ridged_share_past_octave_31_is_refused() {
    let field = level_five_field();
    let relief = Relief::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let _ = relief.ridged_share(32);
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn a_relief_belt_factor_past_octave_31_is_refused() {
    let field = level_five_field();
    let relief = Relief::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let _ = relief.belt_factor(32);
}

#[test]
#[should_panic(expected = "a segment is of level 5 or finer, not 4")]
fn a_channel_segment_above_the_first_level_is_refused() {
    let field = level_five_field();
    let channels = Channels::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let mut network = channels.network(&field);
    let _ = network.segment(PatchKey::containing(4, [0.0, 0.0, 1.0]).unwrap());
}

#[test]
#[should_panic(expected = "a key point is of level 5 or finer, not 4")]
fn a_key_point_above_the_first_level_is_refused() {
    let field = level_five_field();
    let channels = Channels::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let mut network = channels.network(&field);
    let _ = network.key_point(PatchKey::containing(4, [0.0, 0.0, 1.0]).unwrap());
}

#[test]
#[should_panic(expected = "a key point is of level 5 or finer, not 3")]
fn an_inheritance_above_the_first_level_is_refused() {
    let field = level_five_field();
    let channels = Channels::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let mut network = channels.network(&field);
    let _ = network.is_inherited(PatchKey::containing(3, [0.0, 0.0, 1.0]).unwrap());
}

#[test]
#[should_panic(expected = "a segment's parameter is in [0, 1], got 1.5")]
fn a_segment_parameter_beyond_its_outlet_is_refused() {
    let field = level_five_field_flowing(FlowDirection::UMax);
    let channels = Channels::new(field.header(), hyperion_base::rng::DetailSeed::new(1));
    let mut network = channels.network(&field);
    let trunk = network
        .segment(PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap())
        .unwrap()
        .unwrap();
    let _ = trunk.bed_at(1.5);
}

#[test]
#[should_panic(expected = "a drainage area must be finite and non-negative")]
fn a_main_stream_of_a_negative_area_is_refused() {
    let _ = main_stream_length(SquareMetres::new(-1.0));
}

#[test]
#[should_panic(expected = "a stream's length must be finite and non-negative")]
fn a_hack_area_of_a_negative_length_is_refused() {
    let _ = hack_area(Metres::new(-1.0));
}

#[test]
#[should_panic(expected = "a body's radius must be finite and positive")]
fn a_head_length_on_no_body_is_refused() {
    let _ = head_length(Metres::ZERO, 9);
}

#[test]
#[should_panic(expected = "a level is at most 24, got 25")]
fn a_head_length_past_the_deepest_level_is_refused() {
    let _ = head_length(Metres::new(6.371e6), 25);
}

#[test]
#[should_panic(
    expected = "a tributary's steepness, head and length must be finite and non-negative"
)]
fn a_tributary_rise_of_negative_steepness_is_refused() {
    let _ = tributary_rise(-1.0, Metres::new(10.0), Metres::new(10.0));
}

/// The incision's and the hydraulic geometry's refusals (plan R09, T6.b), in a module of their own
/// so that their imports stay apart from the rest.
mod incision {
    use hyperion_base::rng::DetailSeed;
    use hyperion_base::units::MetresPerSecondSquared;
    use hyperion_base::units::{Metres, SquareMetres};
    use hyperion_surface::cube::PatchKey;
    use hyperion_surface::field::{BodyRef, FieldView};
    use hyperion_surface::synth::SynthCache;
    use hyperion_surface::synth::channels::{
        ChannelBody, Channels, bankfull_depth, bankfull_width, channel_forming_discharge_m3_s,
        channel_head_area, channel_structure_function, channel_variance, deepest_level,
        mean_discharge_m3_s, tributary_steepness_m0_9, valley_depth,
    };
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Earth's standard gravity.
    const G: MetresPerSecondSquared = MetresPerSecondSquared::new(9.806_65);

    /// The level-5 test body as the closed forms read it.
    fn body() -> ChannelBody {
        ChannelBody::of(super::level_five_field().header())
    }

    #[test]
    #[should_panic(expected = "a runoff and an area must be finite and non-negative")]
    fn a_mean_discharge_of_a_negative_runoff_is_refused() {
        let _ = mean_discharge_m3_s(-0.1, SquareMetres::new(1e6));
    }

    #[test]
    #[should_panic(expected = "a discharge and an area must be finite and non-negative")]
    fn a_channel_forming_discharge_of_a_nan_area_is_refused() {
        let _ = channel_forming_discharge_m3_s(1.0, SquareMetres::new(f64::NAN));
    }

    #[test]
    #[should_panic(expected = "a discharge must be finite and non-negative")]
    fn a_bankfull_width_of_a_negative_discharge_is_refused() {
        let _ = bankfull_width(-1.0, G);
    }

    #[test]
    #[should_panic(expected = "a discharge must be finite and non-negative")]
    fn a_bankfull_depth_of_an_infinite_discharge_is_refused() {
        let _ = bankfull_depth(f64::INFINITY, G);
    }

    #[test]
    #[should_panic(expected = "a gravity finite and positive")]
    fn a_bankfull_width_without_gravity_is_refused() {
        let _ = bankfull_width(1.0, MetresPerSecondSquared::new(0.0));
    }

    #[test]
    #[should_panic(expected = "a steepness and two runoffs must be finite and non-negative")]
    fn a_tributary_steepness_of_a_negative_runoff_is_refused() {
        let _ = tributary_steepness_m0_9(10.0, -0.1, 0.3);
    }

    #[test]
    #[should_panic(expected = "a steepness index must be finite and non-negative")]
    fn a_channel_head_of_a_negative_steepness_is_refused() {
        let _ = channel_head_area(-1.0);
    }

    #[test]
    #[should_panic(expected = "a steepness index must be finite and non-negative")]
    fn a_valley_of_a_nan_steepness_is_refused() {
        let _ = valley_depth(f64::NAN, Metres::new(6.371e6), 8);
    }

    #[test]
    #[should_panic(expected = "a wavelength must be finite and positive and a lag finite")]
    fn a_channel_structure_function_at_a_negative_lag_is_refused() {
        let field = super::level_five_field();
        let cell = PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap();
        let _ = channel_structure_function(&field, cell, Metres::new(1e3), Metres::new(-1.0));
    }

    #[test]
    #[should_panic(expected = "a body's radius must be finite and positive")]
    fn a_cut_on_no_body_is_refused() {
        let field = super::level_five_field();
        let cell = field
            .cell(PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap())
            .unwrap();
        let body = ChannelBody {
            radius: Metres::ZERO,
            ..body()
        };
        let _ = deepest_level(cell, body);
    }

    #[test]
    #[should_panic(expected = "a level is at most 24, got 25")]
    fn a_cut_past_the_deepest_level_is_refused() {
        let field = super::level_five_field();
        let cell = field
            .cell(PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap())
            .unwrap();
        let body = ChannelBody {
            level: 25,
            ..body()
        };
        let _ = deepest_level(cell, body);
    }

    #[test]
    #[should_panic(expected = "a wavelength must be finite and positive, got 0")]
    fn a_channel_variance_at_no_wavelength_is_refused() {
        let field = super::level_five_field();
        let cell = field
            .cell(PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap())
            .unwrap();
        let _ = channel_variance(cell, body(), Metres::ZERO);
    }

    #[test]
    #[should_panic(expected = "the cell is not of the field's level")]
    fn a_channel_structure_function_at_another_level_is_refused() {
        let field = super::level_five_field();
        let cell = PatchKey::containing(6, [0.0, 0.0, 1.0]).unwrap();
        let _ = channel_structure_function(&field, cell, Metres::new(1e3), Metres::new(1.0));
    }

    #[test]
    #[should_panic(expected = "a segment's parameter is in [0, 1], got 1.5")]
    fn a_depth_below_the_top_beyond_the_outlet_is_refused() {
        let field = super::level_five_field_flowing(super::FlowDirection::UMax);
        let channels = Channels::new(field.header(), DetailSeed::new(1));
        let trunk = channels
            .network(&field)
            .segment(PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap())
            .unwrap()
            .unwrap();
        let _ = trunk.below_top(1.5);
    }

    #[test]
    #[should_panic(expected = "a channel network reads the field whose header made its channels")]
    fn an_incision_over_another_body_s_field_is_refused() {
        let ours = super::level_five_field();
        let theirs = super::level_five_field_of(BodyRef::new(3, 4), super::FlowDirection::Terminal);
        let channels = Channels::new(ours.header(), DetailSeed::new(1));
        let _ = channels.incision_at(&theirs, [0.0, 0.0, 1.0], 9, &mut SynthCache::new());
    }

    #[test]
    #[should_panic(expected = "a channel network reads the field whose header made its channels")]
    fn a_cached_network_over_another_body_s_field_is_refused() {
        let ours = super::level_five_field();
        let theirs = super::level_five_field_of(BodyRef::new(3, 4), super::FlowDirection::Terminal);
        let channels = Channels::new(ours.header(), DetailSeed::new(1));
        let mut cache = SynthCache::new();
        let _ = channels.network_in(&theirs, &mut cache);
    }

    #[test]
    #[should_panic(expected = "a direction must be finite and non-zero")]
    fn an_incision_whose_query_is_not_a_direction_is_refused() {
        let field = super::level_five_field_flowing(super::FlowDirection::UMax);
        let channels = Channels::new(field.header(), DetailSeed::new(1));
        let _ = channels.incision_at(&field, [f64::NAN, 0.0, 1.0], 9, &mut SynthCache::new());
    }
}

/// The cold-trap fraction's refusals (plan R09, T7.d), in a module of their own so that their
/// imports stay apart from the rest.
mod cold_trap {
    use hyperion_base::units::{Kelvin, Radians, WattsPerSquareMetre};
    use hyperion_surface::cold_trap::{
        Body, BodyParts, DepthRatios, Terrain, TerrainParts, bowl_permanent_shadow, fraction,
        fractions, permanent_shadow, shadow_temperature,
    };
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn body() -> Body {
        Body::new(BodyParts {
            flux: WattsPerSquareMetre::new(1361.0),
            greatest_declination: Radians::new(0.027),
            star_radius: Radians::ZERO,
            bond_albedo: 0.12,
            emissivity: 0.95,
            heat_flow: WattsPerSquareMetre::ZERO,
        })
        .unwrap()
    }

    fn terrain() -> Terrain {
        Terrain::new(TerrainParts {
            crater_fraction: 0.2,
            crater_depths: DepthRatios::HAYNE_A,
            plains_slope: Radians::new(0.1),
            share_under_cutoff: 0.0,
        })
        .unwrap()
    }

    #[test]
    #[should_panic(expected = "a latitude must be in [-π/2, π/2]")]
    fn a_fraction_beyond_the_pole_is_refused() {
        let _ = fraction(Radians::new(1.6), &body(), &terrain(), Kelvin::new(110.0));
    }

    #[test]
    #[should_panic(expected = "the thresholds and their fractions must be as many")]
    fn fractions_of_another_length_than_their_thresholds_are_refused() {
        let mut shares = [0.0; 2];
        fractions(
            Radians::new(1.5),
            &body(),
            &terrain(),
            &[Kelvin::new(110.0)],
            &mut shares,
        );
    }

    #[test]
    #[should_panic(expected = "a threshold must be finite and positive")]
    fn fractions_with_a_threshold_that_is_not_finite_are_refused() {
        let mut shares = [0.0; 2];
        fractions(
            Radians::new(1.5),
            &body(),
            &terrain(),
            &[Kelvin::new(110.0), Kelvin::new(f64::NAN)],
            &mut shares,
        );
    }

    #[test]
    #[should_panic(expected = "a latitude must be in [-π/2, π/2]")]
    fn a_shadow_beyond_the_pole_is_refused() {
        let _ = permanent_shadow(Radians::new(-1.6), &body(), &terrain());
    }

    #[test]
    #[should_panic(expected = "a threshold must be finite and positive")]
    fn a_fraction_below_absolute_zero_is_refused() {
        let _ = fraction(Radians::new(1.5), &body(), &terrain(), Kelvin::new(-1.0));
    }

    #[test]
    #[should_panic(expected = "a bowl's depth ratio must be in (0, 0.5]")]
    fn a_bowl_deeper_than_a_hemisphere_is_refused() {
        let _ = shadow_temperature(0.6, Radians::new(0.03), &body());
    }

    #[test]
    #[should_panic(expected = "a star's elevation must be in [0, π/2]")]
    fn a_star_below_the_horizon_is_refused() {
        let _ = shadow_temperature(0.2, Radians::new(-0.1), &body());
    }

    #[test]
    #[should_panic(expected = "a bowl's depth ratio must be finite and positive")]
    fn a_bowl_of_no_depth_is_refused() {
        let _ = bowl_permanent_shadow(0.0, Radians::new(0.1), Radians::new(0.03));
    }

    #[test]
    #[should_panic(expected = "a colatitude must be in [0, π/2]")]
    fn a_colatitude_past_the_equator_is_refused() {
        let _ = bowl_permanent_shadow(0.2, Radians::new(1.6), Radians::new(0.03));
    }

    #[test]
    #[should_panic(expected = "a star's declination and radius must sum to [0, π)")]
    fn a_band_of_negative_width_is_refused() {
        let _ = bowl_permanent_shadow(0.2, Radians::new(0.1), Radians::new(-0.1));
    }
}

/// The airless Moon's crater contract at saturation's N(>1 km).
fn lunar_craters() -> CraterParams {
    CraterParams::new(CraterParamsParts {
        n_1km: PerSquareKilometre::new(0.018),
        screening: Screening::None,
        gravity: MetresPerSecondSquared::new(1.62),
        k_target: 1.0,
        impact_velocity: None,
        projectile_density: craters::LUNAR_PROJECTILE_DENSITY,
        target: craters::LUNAR_TARGET,
        system_age: craters::SOLAR_SYSTEM_AGE,
    })
    .unwrap()
}

#[test]
#[should_panic(expected = "a crater diameter must be finite and positive")]
fn a_density_at_no_diameter_is_refused() {
    let _ = craters::cumulative_density(&lunar_craters(), Metres::ZERO);
}

#[test]
#[should_panic(expected = "a crater diameter must be finite and positive")]
fn a_density_at_a_diameter_too_small_for_kilometres_is_refused() {
    let _ = craters::cumulative_density(&lunar_craters(), Metres::new(1e-322));
}

#[test]
#[should_panic(expected = "a crater diameter must be finite and positive")]
fn a_production_at_a_nan_diameter_is_refused() {
    let _ = craters::production_density(&lunar_craters(), Metres::new(f64::NAN));
}

#[test]
#[should_panic(expected = "a crater diameter must be finite and positive")]
fn a_saturation_at_a_negative_diameter_is_refused() {
    let _ = craters::saturation(Metres::new(-1.0));
}

#[test]
#[should_panic(expected = "a crater diameter must be finite and positive")]
fn a_shape_of_no_diameter_is_refused() {
    let _ = craters::crater_shape(&lunar_craters(), Metres::ZERO);
}

#[test]
#[should_panic(expected = "a distance from a crater's centre must be finite and non-negative")]
fn a_relief_at_a_negative_distance_is_refused() {
    let shape = craters::crater_shape(&lunar_craters(), Metres::new(1e3));
    let _ = shape.relief(-0.5);
}

#[test]
#[should_panic(expected = "a surface age must be finite and non-negative")]
fn a_chronology_of_a_negative_surface_age_is_refused() {
    let _ = craters::lunar_chronology(Gigayears::new(-1.0), craters::SOLAR_SYSTEM_AGE);
}

#[test]
#[should_panic(expected = "is older than its system")]
fn a_chronology_of_a_surface_older_than_its_system_is_refused() {
    let _ = craters::lunar_chronology(Gigayears::new(2.0), Gigayears::new(1.0));
}

#[test]
#[should_panic(expected = "a system age must be finite and non-negative")]
fn a_chronology_age_in_an_infinite_system_is_refused() {
    let _ = craters::chronology_age(PerSquareKilometre::new(0.01), Gigayears::new(f64::INFINITY));
}

#[test]
#[should_panic(expected = "a crater density must not be NaN")]
fn a_chronology_age_of_a_nan_density_is_refused() {
    let _ = craters::chronology_age(PerSquareKilometre::new(f64::NAN), craters::SOLAR_SYSTEM_AGE);
}

/// The small craters' refusals (R09.T7.b).
mod small_craters {
    use hyperion_base::rng::DetailSeed;
    use hyperion_base::units::{Gigayears, Metres, PerSquareKilometre};
    use hyperion_surface::craters::{SOLAR_SYSTEM_AGE, crater_shape};
    use hyperion_surface::cube::PatchKey;
    use hyperion_surface::field::FieldView;
    use hyperion_surface::synth::BandLevel;
    use hyperion_surface::synth::craters::{
        CraterCache, CraterOctaves, CraterProfile, CraterWear, OctaveWear, SmallCraters,
        diffusion_length, profile, relief_factor, structure_function, unresolved_variance,
    };
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{level_five_field, lunar_craters};

    fn small() -> SmallCraters {
        SmallCraters::new(level_five_field().header(), DetailSeed::new(5))
    }

    #[test]
    #[should_panic(expected = "a crater's age must be finite and non-negative")]
    fn a_diffusion_length_of_a_negative_age_is_refused() {
        let _ = diffusion_length(Gigayears::new(-1.0));
    }

    #[test]
    #[should_panic(expected = "a rim radius must be finite and positive")]
    fn a_relief_factor_of_no_rim_radius_is_refused() {
        let _ = relief_factor(Metres::ZERO, CraterWear::FRESH);
    }

    #[test]
    #[should_panic(
        expected = "a crater's diffusion length and share must be finite and non-negative"
    )]
    fn a_wear_of_a_negative_diffusion_is_refused() {
        let _ = CraterWear::new(Metres::new(-1.0), 0.0);
    }

    #[test]
    #[should_panic(
        expected = "a crater's diffusion length and share must be finite and non-negative"
    )]
    fn a_wear_of_a_negative_share_is_refused() {
        let _ = CraterWear::new(Metres::ZERO, -0.5);
    }

    #[test]
    #[should_panic(
        expected = "a crater's diffusion length and share must be finite and non-negative"
    )]
    fn a_wear_of_a_nan_share_is_refused() {
        let _ = CraterWear::new(Metres::ZERO, f64::NAN);
    }

    #[test]
    #[should_panic(expected = "an octave's production must be finite and non-negative and its")]
    fn an_octave_s_wear_of_no_saturation_is_refused() {
        let _ = OctaveWear::new(
            Gigayears::new(4.0),
            SOLAR_SYSTEM_AGE,
            PerSquareKilometre::new(1e-3),
            PerSquareKilometre::ZERO,
        );
    }

    #[test]
    #[should_panic(expected = "an octave's production must be finite and non-negative and its")]
    fn an_octave_s_wear_of_a_nan_production_is_refused() {
        let _ = OctaveWear::new(
            Gigayears::new(4.0),
            SOLAR_SYSTEM_AGE,
            PerSquareKilometre::new(f64::NAN),
            PerSquareKilometre::new(1e-3),
        );
    }

    #[test]
    #[should_panic(expected = "an octave's production must be finite and non-negative and its")]
    fn an_octave_s_wear_of_a_negative_production_is_refused() {
        let _ = OctaveWear::new(
            Gigayears::new(4.0),
            SOLAR_SYSTEM_AGE,
            PerSquareKilometre::new(-1e-3),
            PerSquareKilometre::new(1e-3),
        );
    }

    #[test]
    #[should_panic(expected = "is older than its system")]
    fn an_octave_s_wear_of_a_surface_older_than_its_system_is_refused() {
        let _ = OctaveWear::new(
            Gigayears::new(2.0),
            Gigayears::new(1.0),
            PerSquareKilometre::new(1e-3),
            PerSquareKilometre::new(1e-3),
        );
    }

    #[test]
    #[should_panic(expected = "a rim radius must be finite and positive")]
    fn an_octave_s_mean_square_relief_of_no_rim_radius_is_refused() {
        let wear = OctaveWear::new(
            Gigayears::new(4.0),
            SOLAR_SYSTEM_AGE,
            PerSquareKilometre::new(1e-3),
            PerSquareKilometre::new(1e-3),
        );
        let _ = wear.mean_square_relief(Metres::new(f64::NAN));
    }

    #[test]
    #[should_panic(expected = "a fillet's half-width must be finite and non-negative")]
    fn a_profile_of_a_negative_band_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100.0));
        let _ = CraterProfile::new(shape, Metres::new(-1.0), CraterWear::FRESH);
    }

    #[test]
    #[should_panic(expected = "a fillet's half-width must be finite and non-negative")]
    fn a_profile_of_a_nan_fillet_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100.0));
        let _ = CraterProfile::filleted(shape, Metres::new(f64::NAN));
    }

    #[test]
    #[should_panic(expected = "a distance from a crater's centre must be finite and non-negative")]
    fn a_profile_at_a_negative_distance_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100.0));
        let _ = profile(-0.5, &shape, Metres::new(1.0), CraterWear::FRESH);
    }

    #[test]
    #[should_panic(expected = "a boundary diameter must be finite and positive")]
    fn octaves_of_no_boundary_diameter_are_refused() {
        let field = level_five_field();
        let header = field.header();
        let _ = CraterOctaves::for_body(
            *header.craters(),
            header.figure(),
            Metres::ZERO,
            header.surface_age(),
        );
    }

    #[test]
    #[should_panic(expected = "the body has no crater octave 99")]
    fn craters_of_an_octave_the_body_lacks_are_refused() {
        let cell = PatchKey::containing(10, [0.0, 0.0, 1.0]).unwrap();
        let _ = small().craters_in(99, cell, &mut CraterCache::new());
    }

    #[test]
    #[should_panic(expected = "the cell is not at the octave's level")]
    fn craters_of_a_cell_at_another_level_are_refused() {
        let craters = small();
        let octave = craters.octaves().octaves()[0];
        let cell = PatchKey::containing(octave.level() + 1, [0.0, 0.0, 1.0]).unwrap();
        let _ = craters.craters_in(0, cell, &mut CraterCache::new());
    }

    #[test]
    #[should_panic(expected = "a direction must be finite and non-zero")]
    fn small_craters_at_no_direction_are_refused() {
        let _ = small().at(
            [0.0; 3],
            BandLevel::new(10).unwrap(),
            0,
            &mut CraterCache::new(),
        );
    }

    #[test]
    #[should_panic(expected = "a wavelength must be finite and positive")]
    fn a_crater_variance_below_no_wavelength_is_refused() {
        let field = level_five_field();
        let cell = PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap();
        let _ = unresolved_variance(&field, cell, Metres::ZERO);
    }

    #[test]
    #[should_panic(expected = "the cell is not of the field's level")]
    fn a_crater_variance_of_another_level_s_cell_is_refused() {
        let field = level_five_field();
        let cell = PatchKey::containing(6, [0.0, 0.0, 1.0]).unwrap();
        let _ = unresolved_variance(&field, cell, Metres::new(1e3));
    }

    #[test]
    #[should_panic(expected = "a lag must be finite and non-negative")]
    fn a_crater_structure_function_at_a_negative_lag_is_refused() {
        let field = level_five_field();
        let cell = PatchKey::containing(5, [0.0, 0.0, 1.0]).unwrap();
        let _ = structure_function(&field, cell, Metres::new(1e3), Metres::new(-1.0));
    }
}

/// The coarse craters' rims' refusals (R09.T7.c).
mod coarse_craters_rims {
    use hyperion_base::units::{Gigayears, Metres};
    use hyperion_surface::craters::crater_shape;
    use hyperion_surface::field::{CoarseCrater, Cover, FieldView, Morphology};
    use hyperion_surface::synth::BandLevel;
    use hyperion_surface::synth::craters::rims::{CoarseRims, RimSharpening};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{level_five_field, lunar_craters};

    #[test]
    #[should_panic(expected = "a crater's kept relief must lie in [0, 1]")]
    fn a_sharpening_keeping_more_than_its_fresh_relief_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100e3));
        let _ = RimSharpening::new(shape, Metres::new(1.0), Metres::new(90e3), 1.5);
    }

    #[test]
    #[should_panic(expected = "a level's band and the coarse band must be finite and non-negative")]
    fn a_sharpening_of_a_negative_band_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100e3));
        let _ = RimSharpening::new(shape, Metres::new(-1.0), Metres::new(90e3), 1.0);
    }

    #[test]
    #[should_panic(expected = "a level's band and the coarse band must be finite and non-negative")]
    fn a_sharpening_of_a_nan_coarse_band_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100e3));
        let _ = RimSharpening::new(shape, Metres::new(1.0), Metres::new(f64::NAN), 1.0);
    }

    #[test]
    #[should_panic(expected = "a distance from a crater's centre must be finite and non-negative")]
    fn a_sharpening_that_adds_nothing_at_a_negative_distance_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100e3));
        let _ = RimSharpening::new(shape, Metres::new(90e3), Metres::new(90e3), 1.0).at(-1.0);
    }

    #[test]
    #[should_panic(expected = "a distance from a crater's centre must be finite and non-negative")]
    fn a_sharpening_at_a_negative_distance_is_refused() {
        let shape = crater_shape(&lunar_craters(), Metres::new(100e3));
        let _ = RimSharpening::new(shape, Metres::new(1.0), Metres::new(90e3), 1.0).at(-1.0);
    }

    #[test]
    #[should_panic(expected = "a crater diameter must be finite and positive")]
    fn a_coarse_profile_of_no_diameter_is_refused() {
        let rims = CoarseRims::new(level_five_field().header());
        let _ = rims.coarse_profile(Metres::ZERO);
    }

    /// A listed crater of `diameter_m`, fresh, at the pole.
    fn crater(diameter_m: f64) -> CoarseCrater {
        CoarseCrater {
            centre: [0.0, 0.0, 1.0],
            diameter: Metres::new(diameter_m),
            morphology: Morphology::PeakRing,
            age: Gigayears::new(3.9),
            degradation: 0,
            reach: Cover::from_cells([0]),
        }
    }

    #[test]
    #[should_panic(expected = "a wavelength must be finite and positive")]
    fn a_crater_s_sharpening_moment_at_no_wavelength_is_refused() {
        let rims = CoarseRims::new(level_five_field().header());
        let _ = rims.sharpening_moment_m4(&crater(100e3), Metres::new(f64::NAN));
    }

    #[test]
    #[should_panic(expected = "a crater diameter must be finite and positive")]
    fn a_crater_s_sharpening_moment_of_no_diameter_is_refused() {
        let rims = CoarseRims::new(level_five_field().header());
        let _ = rims.sharpening_moment_m4(&crater(0.0), Metres::new(1e4));
    }

    #[test]
    #[should_panic(expected = "a crater diameter must be finite and positive")]
    fn a_crater_s_increase_of_no_diameter_is_refused() {
        let rims = CoarseRims::new(level_five_field().header());
        let _ = rims.increase_m4(&crater(0.0));
    }

    #[test]
    #[should_panic(expected = "a direction must be finite and non-zero")]
    fn coarse_rims_at_no_direction_are_refused() {
        let field = level_five_field();
        let rims = CoarseRims::new(field.header());
        let _ = rims.at(&field, [0.0; 3], BandLevel::new(10).unwrap());
    }

    #[test]
    #[should_panic(expected = "coarse rims read the field whose header made them")]
    fn coarse_rims_over_another_body_s_field_are_refused() {
        let field = level_five_field();
        let header = field.header();
        // Another body's crater contract on the same figure and level.
        let rims = CoarseRims::for_body(lunar_craters(), header.figure(), header.level());
        let _ = rims.at(&field, [0.0, 0.0, 1.0], BandLevel::new(10).unwrap());
    }
}

mod synth_assembly {
    use hyperion_base::rng::DetailSeed;
    use hyperion_base::units::Metres;
    use hyperion_surface::cube::{Face, PatchKey};
    use hyperion_surface::synth::{SynthCache, Synthesiser, structure_function, unresolved_rms};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::level_five_field;

    /// A cell of the level-5 field.
    fn cell() -> PatchKey {
        PatchKey::new(Face::PosX, 5, 3, 4).unwrap()
    }

    #[test]
    #[should_panic(expected = "vertex (65, 0) is outside a grid of 65 a side")]
    fn a_baked_patch_s_vertex_past_its_side_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let bake = synth.bake_patch(&mut SynthCache::new(), cell()).unwrap();
        let _ = bake.sample(65, 0);
    }

    #[test]
    #[should_panic(expected = "the parent level's heights are at even vertices, not (3, 4)")]
    fn a_parent_height_at_an_odd_vertex_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let bake = synth.bake_patch(&mut SynthCache::new(), cell()).unwrap();
        let _ = bake.parent_height_m(3, 4);
    }

    #[test]
    #[should_panic(expected = "a lag must be finite and non-negative, got -1 m")]
    fn the_ground_s_structure_function_at_a_negative_lag_is_refused() {
        let field = level_five_field();
        let _ = structure_function(&field, cell(), Metres::new(1e3), Metres::new(-1.0));
    }

    #[test]
    #[should_panic(expected = "a wavelength must be finite and positive, got 0")]
    fn the_unresolved_rms_at_no_resolution_is_refused() {
        let field = level_five_field();
        let _ = unresolved_rms(&field, cell(), Metres::ZERO);
    }
}

mod ground {
    use hyperion_base::rng::DetailSeed;
    use hyperion_surface::ground::ground_at;
    use hyperion_surface::patch::HeightSource;
    use hyperion_surface::synth::{SynthCache, Synthesiser};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::level_five_field;

    #[test]
    #[should_panic(
        expected = "a point must be finite and not the body's centre, got [0.0, 0.0, 0.0]"
    )]
    fn the_ground_under_the_body_s_centre_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let _ = ground_at(&synth, &mut SynthCache::new(), &[0.0, 0.0, 0.0]);
    }

    #[test]
    #[should_panic(
        expected = "a point must be finite and not the body's centre, got [NaN, 1.0, 0.0]"
    )]
    fn the_ground_under_a_point_that_is_not_finite_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let _ = ground_at(&synth, &mut SynthCache::new(), &[f64::NAN, 1.0, 0.0]);
    }

    #[test]
    #[should_panic(
        expected = "band level 25 is above the maximum 24: R05's levels are band levels"
    )]
    fn the_synthesiser_as_a_height_source_refuses_a_level_past_the_deepest() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let _ = HeightSource::height(&synth, &mut SynthCache::new(), [0.0, 0.0, 1.0], 25);
    }
}

/// R10.T1.b's classifier.
mod material {
    use hyperion_base::rng::DetailSeed;
    use hyperion_surface::cube::{Face, FaceUv};
    use hyperion_surface::material::material_at;
    use hyperion_surface::synth::{SynthCache, Synthesiser};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::level_five_field;

    #[test]
    #[should_panic(expected = "a direction must be finite and non-zero, got [1.0, NaN, 0.0]")]
    fn the_material_at_a_point_that_is_not_finite_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let at = FaceUv {
            face: Face::PosX,
            u: f64::NAN,
            v: 0.0,
        };
        let _ = material_at(&synth, &mut SynthCache::new(), at);
    }
}

/// P14.T51.d's space-weathering maturity law and R10.T1.c's exposure, one-crater formula,
/// excavated share and halo: their inputs out of range are refused.
mod maturity {
    use hyperion_base::rng::DetailSeed;
    use hyperion_base::units::{Gigayears, Metres};
    use hyperion_surface::cube::{Face, PatchKey};
    use hyperion_surface::field::{CoarseField, FieldView};
    use hyperion_surface::material::{
        GroundExposure, MATURITY_NODES, class_fractions, crater_maturity, excavated_share,
        halo_reach, halo_share, haloed_albedo_scale, maturity_after, maturity_mix,
        maturity_node_albedo, maturity_node_share, patch_maturity, reworked_depth,
    };
    use hyperion_surface::synth::{SynthCache, Synthesiser};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    #[should_panic(expected = "an exposure's maturity must be in [0, 1]")]
    fn an_exposure_refuses_a_maturity_above_one() {
        let _ = GroundExposure::NONE.with_maturity(1.0 + 1e-12);
    }

    #[test]
    #[should_panic(expected = "an exposure's maturity must be in [0, 1]")]
    fn an_exposure_refuses_a_nan_maturity() {
        let _ = GroundExposure::NONE
            .with_maturity(f64::NAN)
            .with_organic_cover(0.5);
    }

    #[test]
    #[should_panic(expected = "an exposure's maturity must be in [0, 1]")]
    fn an_exposure_refuses_a_point_s_negative_maturity() {
        let _ = GroundExposure::NONE.with_maturity(-1e-9);
    }

    #[test]
    #[should_panic(expected = "an exposure's organic cover must be in [0, 1]")]
    fn an_exposure_refuses_a_negative_cover() {
        let _ = GroundExposure::NONE.with_organic_cover(-0.25);
    }

    #[test]
    #[should_panic(expected = "an exposure's organic cover must be in [0, 1]")]
    fn an_exposure_refuses_a_nan_cover() {
        let _ = GroundExposure::NONE
            .with_maturity(0.5)
            .with_organic_cover(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "a crater's fresh share must be in [0, 1]")]
    fn the_one_crater_formula_refuses_a_share_above_one() {
        let _ = crater_maturity(1.0, Gigayears::new(0.1), 1.0, 1.5);
    }

    #[test]
    #[should_panic(expected = "a crater's fresh share must be in [0, 1]")]
    fn the_one_crater_formula_refuses_a_negative_share() {
        let _ = crater_maturity(1.0, Gigayears::new(0.1), 1.0, -0.1);
    }

    #[test]
    #[should_panic(expected = "a crater's fresh share must be in [0, 1]")]
    fn the_one_crater_formula_refuses_a_nan_share() {
        let _ = crater_maturity(1.0, Gigayears::new(0.1), 1.0, f64::NAN);
    }

    #[test]
    #[should_panic(expected = "a body's maturity must be in [0, 1]")]
    fn the_one_crater_formula_refuses_a_body_maturity_above_one() {
        let _ = crater_maturity(2.0, Gigayears::new(0.1), 1.0, 0.5);
    }

    #[test]
    #[should_panic(expected = "an exposure must be finite")]
    fn the_reworked_depth_refuses_an_infinite_exposure() {
        let _ = reworked_depth(Gigayears::new(f64::INFINITY));
    }

    #[test]
    #[should_panic(expected = "a surface's and a crater's ages must be finite and non-negative")]
    fn the_excavated_share_refuses_a_negative_crater_age() {
        let _ = excavated_share(
            Gigayears::new(3.5),
            Gigayears::new(-1e-9),
            Metres::new(100.0),
        );
    }

    #[test]
    #[should_panic(expected = "a surface's and a crater's ages must be finite and non-negative")]
    fn the_excavated_share_refuses_a_nan_surface_age() {
        let _ = excavated_share(
            Gigayears::new(f64::NAN),
            Gigayears::ZERO,
            Metres::new(100.0),
        );
    }

    #[test]
    #[should_panic(expected = "a crater's diameter must be finite and positive")]
    fn the_excavated_share_refuses_a_zero_diameter() {
        let _ = excavated_share(Gigayears::new(3.5), Gigayears::ZERO, Metres::ZERO);
    }

    #[test]
    #[should_panic(expected = "a distance must be non-negative")]
    fn the_halo_refuses_a_nan_distance() {
        let _ = halo_share(f64::NAN, 10.0);
    }

    #[test]
    #[should_panic(expected = "a distance must be non-negative")]
    fn the_halo_refuses_a_negative_distance() {
        let _ = halo_share(-0.5, 10.0);
    }

    #[test]
    #[should_panic(expected = "a halo must end finite and at or beyond the rim")]
    fn the_halo_refuses_an_end_inside_the_rim() {
        let _ = halo_share(0.5, 0.9);
    }

    #[test]
    #[should_panic(expected = "a halo must end finite and at or beyond the rim")]
    fn the_halo_refuses_an_infinite_end() {
        let _ = halo_share(2.0, f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "an exposure must be finite and non-negative")]
    fn the_maturity_refuses_a_negative_exposure() {
        let _ = maturity_after(Gigayears::new(-1e-9), 1.0);
    }

    #[test]
    #[should_panic(expected = "an exposure must be finite and non-negative")]
    fn the_maturity_refuses_a_nan_exposure() {
        let _ = maturity_after(Gigayears::new(f64::NAN), 1.0);
    }

    #[test]
    #[should_panic(expected = "a weathering rate must be finite and non-negative")]
    fn the_maturity_refuses_an_infinite_rate() {
        let _ = maturity_after(Gigayears::new(1.0), f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "a halo's share must be in (0, 1]")]
    fn the_inverse_halo_refuses_a_share_of_zero() {
        let _ = halo_reach(0.0, 10.0);
    }

    #[test]
    #[should_panic(expected = "a halo's share must be in (0, 1]")]
    fn the_inverse_halo_refuses_a_share_above_one() {
        let _ = halo_reach(1.0 + 1e-12, 10.0);
    }

    #[test]
    #[should_panic(expected = "a halo's share must be in (0, 1]")]
    fn the_inverse_halo_refuses_a_nan_share() {
        let _ = halo_reach(f64::NAN, 10.0);
    }

    #[test]
    #[should_panic(expected = "a halo must end finite and at or beyond the rim")]
    fn the_inverse_halo_refuses_an_end_inside_the_rim() {
        let _ = halo_reach(0.5, 0.9);
    }

    #[test]
    #[should_panic(expected = "a halo must end finite and at or beyond the rim")]
    fn the_inverse_halo_refuses_an_infinite_end() {
        let _ = halo_reach(0.5, f64::INFINITY);
    }

    /// The level-5 field weathered at maturity 0.5, with no crater: its mix is one node.
    fn weathered_field() -> CoarseField {
        super::level_five_field_weathered(
            super::BodyRef::new(1, 2),
            super::FlowDirection::Terminal,
            Some(0.5),
        )
    }

    #[test]
    #[should_panic(expected = "a maturity mix of m_body 0.5 is solved at its own maturity")]
    fn the_haloed_solve_refuses_a_mix_of_another_maturity() {
        let field = weathered_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let mix = maturity_mix(&synth).expect("the field carries a maturity");
        let fractions = class_fractions(&synth, &mut SynthCache::new()).unwrap();
        let _ = haloed_albedo_scale(
            &fractions,
            field.header().palette(),
            GroundExposure::NONE.with_maturity(0.4),
            &mix,
            [0.1, 0.12, 0.14],
        );
    }

    #[test]
    #[should_panic(expected = "a maturity mix of m_body 0.5 is solved at its own maturity")]
    fn the_haloed_solve_refuses_an_exposure_of_no_maturity() {
        let field = weathered_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let mix = maturity_mix(&synth).expect("the field carries a maturity");
        let fractions = class_fractions(&synth, &mut SynthCache::new()).unwrap();
        let _ = haloed_albedo_scale(
            &fractions,
            field.header().palette(),
            GroundExposure::NONE,
            &mix,
            [0.1, 0.12, 0.14],
        );
    }

    #[test]
    #[should_panic(expected = "a deficit must be non-negative")]
    fn the_mix_refuses_a_negative_deficit() {
        let field = weathered_field();
        let mix = maturity_mix(&Synthesiser::new(&field, DetailSeed::new(1))).unwrap();
        let _ = mix.share_above(-1e-9);
    }

    #[test]
    #[should_panic(expected = "a deficit must be non-negative")]
    fn the_mix_refuses_a_nan_deficit() {
        let field = weathered_field();
        let mix = maturity_mix(&Synthesiser::new(&field, DetailSeed::new(1))).unwrap();
        let _ = mix.share_above(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "a patch's maturity takes its 4225 vertices, got 3")]
    fn a_patch_s_maturity_refuses_a_list_that_is_not_its_vertices() {
        let field = weathered_field();
        let _ = patch_maturity(
            &Synthesiser::new(&field, DetailSeed::new(1)),
            &mut SynthCache::new(),
            PatchKey::root(Face::PosZ),
            &[None; 3],
        );
    }

    #[test]
    #[should_panic(expected = "a maturity node is one of 16, got 16")]
    fn the_node_grading_refuses_a_node_past_its_count() {
        let _ = maturity_node_share(MATURITY_NODES);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 0.5], its background's, got 0.6")]
    fn the_node_interpolation_refuses_a_maturity_above_its_background_s() {
        let _ = maturity_node_albedo(&[[0.1; 3]; MATURITY_NODES], 0.5, 0.6);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 0.5], its background's, got -0.1")]
    fn the_node_interpolation_refuses_a_negative_maturity() {
        let _ = maturity_node_albedo(&[[0.1; 3]; MATURITY_NODES], 0.5, -0.1);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 0.5], its background's, got NaN")]
    fn the_node_interpolation_refuses_a_nan_maturity() {
        let _ = maturity_node_albedo(&[[0.1; 3]; MATURITY_NODES], 0.5, f64::NAN);
    }

    #[test]
    #[should_panic(expected = "a background maturity must be in [0, 1], got NaN")]
    fn the_node_interpolation_refuses_a_nan_background() {
        let _ = maturity_node_albedo(&[[0.1; 3]; MATURITY_NODES], f64::NAN, 0.0);
    }

    #[test]
    #[should_panic(expected = "a background maturity must be in [0, 1], got 1.5")]
    fn the_node_interpolation_refuses_a_background_above_one() {
        let _ = maturity_node_albedo(&[[0.1; 3]; MATURITY_NODES], 1.5, 1.0);
    }

    #[test]
    #[should_panic(expected = "a background maturity must be in [0, 1], got -0.5")]
    fn the_node_interpolation_refuses_a_negative_background() {
        let _ = maturity_node_albedo(&[[0.1; 3]; MATURITY_NODES], -0.5, 0.0);
    }
}

mod weathering {
    use hyperion_base::units::Kelvin;
    use hyperion_surface::field::{MechanicsFamily, PaletteEntry, PaletteRole};
    use hyperion_surface::material::{
        PhaseLaw, npfe_optical_depth, weathered_entry_bvr, weathered_hemispherical_albedos,
        weathered_normal_albedo, weathered_spherical_albedo,
    };
    use hyperion_surface::substance_key::SubstanceKey;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    #[should_panic(expected = "a fresh albedo must be finite and non-negative")]
    fn the_weathering_law_refuses_a_negative_fresh_albedo() {
        let _ = weathered_normal_albedo(-1e-9, 550.0, PhaseLaw::Regolith, 3_000.0, 0.1, 1.0);
    }

    #[test]
    #[should_panic(expected = "a fresh albedo must be finite and non-negative")]
    fn the_weathering_law_refuses_a_nan_fresh_albedo() {
        let _ = weathered_spherical_albedo(f64::NAN, 550.0, PhaseLaw::Ice, 917.0, 0.0, 0.5);
    }

    #[test]
    #[should_panic(expected = "a fresh albedo must be finite and non-negative")]
    fn the_weathering_law_refuses_an_infinite_hemispherical_albedo() {
        let _ = weathered_hemispherical_albedos(
            [0.5, 0.5, f64::INFINITY, 0.5],
            550.0,
            PhaseLaw::Regolith,
            3_000.0,
            0.1,
            1.0,
        );
    }

    #[test]
    #[should_panic(expected = "a wavelength must be finite and positive")]
    fn the_weathering_law_refuses_a_zero_wavelength() {
        let _ = npfe_optical_depth(PhaseLaw::Lambert, 0.0, 3_000.0, 0.1, 1.0);
    }

    #[test]
    #[should_panic(expected = "a grain density must be finite and positive")]
    fn the_weathering_law_refuses_an_infinite_density() {
        let _ = weathered_normal_albedo(0.1, 550.0, PhaseLaw::Regolith, f64::INFINITY, 0.1, 1.0);
    }

    #[test]
    #[should_panic(expected = "an FeO_T mass fraction must be in [0, 1]")]
    fn the_weathering_law_refuses_an_iron_fraction_above_one() {
        let _ = weathered_normal_albedo(0.1, 550.0, PhaseLaw::Regolith, 3_000.0, 1.5, 1.0);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 1]")]
    fn the_weathering_law_refuses_a_nan_maturity() {
        let _ = npfe_optical_depth(PhaseLaw::Ice, 550.0, 917.0, 0.0, f64::NAN);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 1]")]
    fn the_weathering_law_refuses_a_negative_maturity_for_an_entry_without_iron() {
        let entry = PaletteEntry {
            substance: SubstanceKey::new("granite").unwrap(),
            role: PaletteRole::PrimaryCrust,
            normal_albedo_bvr: [0.275; 3],
            phase_row: 0,
            grain_density_kg_m3: 2_650.0,
            transition: Kelvin::new(1_233.15),
            mechanics: MechanicsFamily::Silicate,
            liquid: None,
            feo_mass_fraction: None,
        };
        let _ = weathered_entry_bvr(&entry, -0.1);
    }
}

mod reading {
    use hyperion_base::rng::DetailSeed;
    use hyperion_base::units::Metres;
    use hyperion_surface::cube::{Face, FaceUv, PatchKey};
    use hyperion_surface::field::{FieldView, PartialField};
    use hyperion_surface::reading::{slant_range, surveyed_reading};
    use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::level_five_field;

    /// A client's field of the level-5 field holding no cell yet.
    fn empty_client() -> PartialField {
        PartialField::new(level_five_field().header().clone())
    }

    /// A point on face +x.
    const AT: FaceUv = FaceUv {
        face: Face::PosX,
        u: 0.1,
        v: -0.2,
    };

    #[test]
    #[should_panic(expected = "a slope's baseline must be finite and positive, got 0 m")]
    fn a_slope_over_no_baseline_is_refused() {
        let field = empty_client();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let _ = surveyed_reading(&synth, &mut SynthCache::new(), AT, Metres::ZERO);
    }

    #[test]
    #[should_panic(expected = "a slope's baseline must be finite and positive, got NaN m")]
    fn a_slope_over_a_baseline_that_is_not_finite_is_refused() {
        let field = empty_client();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let _ = surveyed_reading(&synth, &mut SynthCache::new(), AT, Metres::new(f64::NAN));
    }

    #[test]
    #[should_panic(expected = "a line of sight's origin must be finite, got [NaN, 0.0, 0.0]")]
    fn a_line_of_sight_from_a_point_that_is_not_finite_is_refused() {
        let field = empty_client();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let origin = [f64::NAN, 0.0, 0.0];
        let _ = slant_range(
            &synth,
            &mut SynthCache::new(),
            &origin,
            &[1.0, 0.0, 0.0],
            Metres::new(1e3),
        );
    }

    #[test]
    #[should_panic(expected = "a direction must be finite and non-zero, got [0.0, 0.0, 0.0]")]
    fn a_line_of_sight_with_no_direction_is_refused() {
        let field = empty_client();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let origin = [1e6, 0.0, 0.0];
        let _ = slant_range(
            &synth,
            &mut SynthCache::new(),
            &origin,
            &[0.0, 0.0, 0.0],
            Metres::new(1e3),
        );
    }

    #[test]
    #[should_panic(expected = "the cell is not of the field's level")]
    fn what_a_level_leaves_out_of_a_cell_of_another_level_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let level = BandLevel::new(9).unwrap();
        let _ = synth.omitted_variance(PatchKey::new(Face::PosX, 4, 0, 0).unwrap(), level);
    }

    #[test]
    #[should_panic(expected = "the cell is not of the field's level")]
    fn the_rms_a_level_leaves_out_of_a_cell_of_another_level_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let level = BandLevel::new(9).unwrap();
        let _ = synth.omitted_rms(PatchKey::new(Face::PosX, 6, 0, 0).unwrap(), level);
    }

    #[test]
    #[should_panic(expected = "the cell is not of the field's level")]
    fn the_structure_a_level_leaves_out_of_a_cell_of_another_level_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let cell = PatchKey::new(Face::PosX, 4, 0, 0).unwrap();
        let level = BandLevel::new(9).unwrap();
        let _ = synth.omitted_structure_function(cell, level, Metres::new(2.0));
    }

    #[test]
    #[should_panic(expected = "a lag must be finite and non-negative, got NaN m")]
    fn the_structure_a_level_leaves_out_at_no_lag_is_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let cell = PatchKey::new(Face::PosX, 5, 0, 0).unwrap();
        let level = BandLevel::new(9).unwrap();
        let _ = synth.omitted_structure_function(cell, level, Metres::new(f64::NAN));
    }

    #[test]
    #[should_panic(expected = "a line of sight's reach must be finite and non-negative, got -1 m")]
    fn a_line_of_sight_of_negative_reach_is_refused() {
        let field = empty_client();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let origin = [1e6, 0.0, 0.0];
        let _ = slant_range(
            &synth,
            &mut SynthCache::new(),
            &origin,
            &[-1.0, 0.0, 0.0],
            Metres::new(-1.0),
        );
    }
}

/// R10.T2's class weights: a point that is not finite is refused, before the level is read.
mod weights {
    use hyperion_base::rng::DetailSeed;
    use hyperion_surface::cube::{Face, FaceUv};
    use hyperion_surface::material::weights_at;
    use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::level_five_field;

    #[test]
    #[should_panic(expected = "a direction must be finite and non-zero, got [1.0, NaN, 0.0]")]
    fn the_weights_at_a_point_that_is_not_finite_are_refused() {
        let field = level_five_field();
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let at = FaceUv {
            face: Face::PosX,
            u: f64::NAN,
            v: 0.0,
        };
        let _ = weights_at(
            &synth,
            &mut SynthCache::new(),
            at,
            BandLevel::new(4).unwrap(),
        );
    }
}

/// R10.T6.a's bake on real terrain: a skirt margin that is not finite and non-negative is refused,
/// as R05's bake refuses it, before any height is asked; and R05's bake refuses a source whose
/// patch heights are another patch's.
mod bake {
    use hyperion_base::rng::DetailSeed;
    use hyperion_surface::bake::{
        HorizonLayer, TerrainBakeOptions, WeightsLayer, bake_terrain, decode_maturity,
        encode_maturity,
    };
    use hyperion_surface::cube::{Face, PatchKey};
    use hyperion_surface::field::{CoarseField, FieldView, PartialField};
    use hyperion_surface::height::HeightSample;
    use hyperion_surface::patch::{BakeOptions, HeightSource, NormalScale, VertexPath, bake_patch};
    use hyperion_surface::spheroid::Spheroid;
    use hyperion_surface::synth::{PatchHeights, QueryHeightError, SynthCache, Synthesiser};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::level_five_field;

    /// R09's synthesiser, but answering every bake with the heights of face −z's root patch.
    struct OtherPatch<'a>(Synthesiser<'a, CoarseField>);

    impl HeightSource for OtherPatch<'_> {
        type Cache = SynthCache;
        type Error = QueryHeightError;

        fn figure(&self) -> Spheroid {
            HeightSource::figure(&self.0)
        }

        fn height(
            &self,
            cache: &mut SynthCache,
            dir: [f64; 3],
            level: u8,
        ) -> Result<HeightSample, QueryHeightError> {
            HeightSource::height(&self.0, cache, dir, level)
        }

        fn level_bound_m(&self, level: u8) -> f64 {
            HeightSource::level_bound_m(&self.0, level)
        }

        fn height_range_m(&self, key: PatchKey) -> (f64, f64) {
            HeightSource::height_range_m(&self.0, key)
        }

        fn patch_heights(
            &self,
            cache: &mut SynthCache,
            _: PatchKey,
        ) -> Result<PatchHeights, QueryHeightError> {
            self.0.bake_patch(cache, PatchKey::root(Face::NegZ))
        }
    }

    #[test]
    #[should_panic(expected = "a source's patch heights must be the baked patch's")]
    fn a_source_s_patch_heights_of_another_patch_are_refused() {
        let field = level_five_field();
        let source = OtherPatch(Synthesiser::new(&field, DetailSeed::new(1)));
        let opts = BakeOptions {
            vertex_path: VertexPath::FaceDifferences,
            normals: NormalScale::Mesh,
            skirt_m: 0.0,
        };
        let _ = bake_patch(
            &source,
            PatchKey::root(Face::PosZ),
            &opts,
            &mut SynthCache::new(),
        );
    }

    #[test]
    #[should_panic(expected = "a background maturity must be in [0, 1], got 1.5")]
    fn a_maturity_code_on_a_background_above_one_is_refused() {
        let _ = encode_maturity(0.5, 1.5);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 0.5], its background's, got 0.6")]
    fn a_maturity_code_of_a_maturity_above_its_background_is_refused() {
        let _ = encode_maturity(0.6, 0.5);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 0.5], its background's, got -0.1")]
    fn a_maturity_code_of_a_negative_maturity_is_refused() {
        let _ = encode_maturity(-0.1, 0.5);
    }

    #[test]
    #[should_panic(expected = "a maturity must be in [0, 0.5], its background's, got NaN")]
    fn a_maturity_code_of_a_nan_maturity_is_refused() {
        let _ = encode_maturity(f64::NAN, 0.5);
    }

    #[test]
    #[should_panic(expected = "a background maturity must be in [0, 1], got NaN")]
    fn a_maturity_decoded_on_a_nan_background_is_refused() {
        let _ = decode_maturity(17, f64::NAN);
    }

    #[test]
    #[should_panic(expected = "a skirt margin must be finite and non-negative, got -1")]
    fn a_terrain_bake_with_a_negative_skirt_margin_is_refused() {
        let field = PartialField::new(level_five_field().header().clone());
        let synth = Synthesiser::new(&field, DetailSeed::new(1));
        let opts = TerrainBakeOptions {
            patch: BakeOptions {
                vertex_path: VertexPath::FaceDifferences,
                normals: NormalScale::Mesh,
                skirt_m: -1.0,
            },
            weights: WeightsLayer::Baked,
            horizon: HorizonLayer::Baked,
        };
        let _ = bake_terrain(
            &synth,
            &mut SynthCache::new(),
            PatchKey::root(Face::PosZ),
            &opts,
            &[],
        );
    }

    /// R10.T8.a's horizon map: a far-field point past its level's lattice, an elevation that is
    /// not a number, a lookup at an azimuth that is not finite, and a grid azimuth at a point
    /// behind its face or of a direction that is not finite are refused.
    mod horizon {
        use hyperion_base::units::Radians;
        use hyperion_surface::bake::{HorizonMap, encode_horizon, far_field_point, grid_azimuth};
        use hyperion_surface::cube::Face;
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        use wasm_bindgen_test::wasm_bindgen_test as test;

        #[test]
        #[should_panic(expected = "far-field point (17, 0) is past level 1's 16 intervals")]
        fn a_far_field_point_past_its_lattice_is_refused() {
            let _ = far_field_point(Face::PosX, 1, 17, 0);
        }

        #[test]
        #[should_panic(expected = "a horizon's elevation is a number")]
        fn an_elevation_that_is_not_a_number_is_refused() {
            let _ = encode_horizon(Radians::new(f64::NAN));
        }

        #[test]
        #[should_panic(expected = "an azimuth must be finite, got inf")]
        fn a_lookup_at_an_azimuth_that_is_not_finite_is_refused() {
            let _ = HorizonMap::default().elevation_at(0, 0, Radians::new(f64::INFINITY));
        }

        #[test]
        #[should_panic(expected = "[-1.0, 0.2, 0.1] is not in front of face PosX")]
        fn a_grid_azimuth_behind_its_face_is_refused() {
            let _ = grid_azimuth(Face::PosX, [-1.0, 0.2, 0.1], [0.0, 1.0, 0.0]);
        }

        #[test]
        #[should_panic(expected = "a grid azimuth's directions must be finite")]
        fn a_grid_azimuth_of_a_direction_that_is_not_finite_is_refused() {
            let _ = grid_azimuth(Face::PosX, [1.0, 0.2, 0.1], [0.0, f64::NAN, 0.0]);
        }
    }
}
