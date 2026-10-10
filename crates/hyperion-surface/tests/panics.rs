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
use hyperion_surface::craters::{CraterParams, CraterParamsParts, Screening};
use hyperion_surface::cube::{Face, MAX_LEVEL, PatchKey, unit_dir, xyz_to_face_uv};
use hyperion_surface::field::{
    BodyRef, BoundaryKind, ClimateCell, ClimateModelKind, CoarseField, CoarseLevel, Cover, Crust,
    FieldHeader, FieldHeaderParts, FieldView, FlowDirection, LogArea, LogPrecipitation,
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
    Relief, finest_octave, nyquist_degree, octave_bound, octave_rms, octave_spacing,
};
use hyperion_surface::synth::{BandSpectrum, unresolved_variance};
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
    let radius = 4.697e5;
    let craters = CraterParams::new(CraterParamsParts {
        n_1km: PerSquareKilometre::ZERO,
        screening: Screening::None,
        gravity: MetresPerSecondSquared::new(0.28),
        k_target: 1.0,
        impact_velocity: None,
    })
    .unwrap();
    let header = FieldHeader::new(FieldHeaderParts {
        body: BodyRef::new(1, 2),
        radius: Metres::new(radius),
        figure: Spheroid::from_volumetric(radius, 0.0).unwrap(),
        sea_level: Metres::ZERO,
        lapse_rate_k_per_m: 0.0,
        spectrum: BandSpectrum::new(1.9, SquareMetres::ZERO).unwrap(),
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
        water_surface_mm: 0,
        ice: 0,
        substances: SynthesisCell::NO_SUBSTANCES,
        class: SurfaceClass::UNCLASSIFIED,
        crater_state: 0,
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
    BandSpectrum::new(1.9, SquareMetres::new(1.0)).unwrap()
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
