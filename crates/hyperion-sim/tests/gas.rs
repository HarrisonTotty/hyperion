//! The gas and dust field's fast integration tests (plan 07).
//!
//! The field's own arithmetic is unit-tested inside each `galaxy::gas` module, where the smooth
//! components and the normalisations are reachable; what belongs here is what only the public API
//! can see, and the golden files that pin the generator's output. P07.T12's slow statistical tests
//! live in `gas_statistics.rs`.

#[expect(dead_code, reason = "the gas tests use only the shared assertions")]
mod common;

use std::fmt::Debug;
use std::num::NonZeroU32;
use std::panic::{RefUnwindSafe, UnwindSafe};

use hyperion_sim::coords::{GalacticPosition, LyCell, UnitVector};
use hyperion_sim::galaxy::bounds::CellBox;
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::fields::arms::ArmGeometry;
use hyperion_sim::galaxy::gas::ccm::{Band, HYDROGEN_COLUMN_PER_MAG, extinction_ratio};
use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, Sightline, horizon, sightline};
use hyperion_sim::galaxy::gas::field::GasField;
use hyperion_sim::galaxy::gas::lanes::Lanes;
use hyperion_sim::galaxy::gas::map::{
    extinction_edge_on, extinction_face_on, render_extinction_rows,
};
use hyperion_sim::galaxy::gas::modifiers::{GasModifier, GasModifierSource, NoModifiers};
use hyperion_sim::galaxy::gas::noise::{NoiseCache, SmoothingScale, log_normal_factor};
use hyperion_sim::galaxy::gas::params::{BuildGasParamsError, GasParams};
use hyperion_sim::galaxy::gas::phase::GasPhase;
use hyperion_sim::galaxy::gas::smooth::{GasLayer, SmoothGas};
use hyperion_sim::galaxy::gas::{CENTIMETRES_PER_LIGHT_YEAR, SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::map::{MapSelection, MapSpec, MapView};
use hyperion_sim::galaxy::params::{GalaxyParams, GalaxyParamsBuilder};
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::{BuildGalaxyError, Galaxy, PointLy, Population};
use hyperion_sim::math;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{
    HydrogenPerCm3, LightYears, Magnitudes, Micrometres, SolarMasses, Years,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

use crate::common::{assert_relative, ensemble_mean, whole_ly};

/// The three seeds the parameter golden pins, the same ones plan 02's own parameter golden uses so
/// that the two files can be read side by side.
const PINNED_SEEDS: [u64; 3] = [
    0x0000_0000_0000_0001,
    0x5eed_0000_c0ff_ee00,
    0xdead_beef_cafe_f00d,
];

/// Every getter of `gas`, labelled `<prefix>.<name>`, in Design note 3's table order: plan 02's
/// three rows first, then the eleven drawn here with what each derives, and the two constants.
fn write_gas_params(w: &mut GoldenWriter, prefix: &str, gas: &GasParams) {
    let mut f = |name: &str, value: f64| w.f64(&format!("{prefix}.{name}"), value);
    f("mass", gas.gas_mass().value());
    f("radial_scale", gas.radial_scale().value());
    f("neutral_height", gas.neutral_height().value());
    f("hole_scale", gas.hole_scale().value());
    f("neutral_mass", gas.neutral_mass().value());
    f("neutral_fraction", gas.neutral_fraction());
    f("warm_density", gas.warm_density().value());
    f("warm_height", gas.warm_height().value());
    f("warm_mass", gas.warm_mass().value());
    f("warm_fraction", gas.warm_fraction());
    let molecular = gas.molecular_disc();
    f("molecular_mass", molecular.mass().value());
    f("molecular_fraction", molecular.fraction());
    f("molecular_length", molecular.length().value());
    f("molecular_height", molecular.height().value());
    f("corona_density", gas.corona_density().value());
    f("pressure_floor", gas.pressure_floor().value());
    f("pressure_height", gas.pressure_height().value());
    f("pressure_speed", gas.pressure_speed().value());
    f("sigma_ln", gas.sigma_ln());
    let lane = gas.lane();
    f("lane_offset", lane.offset().value());
    f("lane_width", lane.width().value());
    f("lane_fraction", lane.fraction());
}

/// Three pinned seeds, bit for bit: the file moves if a parameter's law, its range or its word
/// index changes, which is what pins Design note 3's draw order (P07.T1).
#[test]
fn gas_params_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for raw in PINNED_SEEDS {
        let seed = Seed::new(raw);
        w.line(&format!("# seed {seed}"));
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        write_gas_params(
            &mut w,
            &seed.to_string(),
            &GasParams::from_galaxy(seed, &galaxy).expect("a drawn galaxy's gas"),
        );
    }
    golden!("gas/params", w.as_str());
}

/// The gas mass in M☉ of the smooth field with the lanes on, by a midpoint sum in `ln R` over
/// 400 radii from 0.25 ly to 40 scale lengths and 1,024 azimuths, each layer's closed-form vertical
/// column standing for its height integral; and the same sum with the lanes off.
///
/// The azimuths are the trapezoid rule of a periodic function, exact far past the 2% the budget is
/// held to wherever there is gas to weigh: the narrowest lane at 100,000 ly still spans 1.6 of
/// them.
fn masses_with_and_without_lanes(gas: &SmoothGas, lanes: &Lanes, radial_scale: f64) -> (f64, f64) {
    let (radii, azimuths) = (400_u32, 1_024_u32);
    let (lo, hi) = (0.25, 40.0 * radial_scale);
    let step = math::ln(hi / lo) / f64::from(radii);
    let per_column = SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY / CENTIMETRES_PER_LIGHT_YEAR;
    let (mut with, mut without) = (0.0, 0.0);
    for i in 0..radii {
        let r = lo * math::exp(step * (f64::from(i) + 0.5));
        // `2π R dR` in `u = ln R` is `2π R² du`.
        let ring = core::f64::consts::TAU * r * r * step * per_column;
        let lane_mean = (0..azimuths)
            .map(|j| {
                lanes.factor(
                    r,
                    core::f64::consts::TAU * f64::from(j) / f64::from(azimuths),
                )
            })
            .sum::<f64>()
            / f64::from(azimuths);
        let neutral = gas.column(GasLayer::Neutral, r);
        let rest = gas.column(GasLayer::Warm, r) + gas.column(GasLayer::Molecular, r);
        with += ring * (neutral * lane_mean + rest);
        without += ring * (neutral + rest);
    }
    (with, without)
}

/// P07.T2's mass budget still closes with P07.T3's lanes on: the lanes gather the neutral gas onto
/// the arms' inner edges and move none of it between radii, so the field with lanes weighs plan
/// 02's gas mass to 2%, and the same as the field without them to 10⁻⁹, for the Milky Way fixture
/// and eight drawn galaxies.
#[test]
fn the_gas_mass_closes_with_the_lanes_on() {
    let mut cases = vec![(GalaxyParams::milky_way_like(), GasParams::milky_way_like())];
    for n in 0..8 {
        let seed = Seed::new(0x0700_5eed_0003_0000 | n);
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let gas = GasParams::from_galaxy(seed, &galaxy).expect("a drawn galaxy's gas");
        cases.push((galaxy, gas));
    }
    for (galaxy, params) in cases {
        let gas = SmoothGas::new(&params);
        let lanes = Lanes::new(ArmGeometry::of(&galaxy), params.lane());
        let (with, without) =
            masses_with_and_without_lanes(&gas, &lanes, params.radial_scale().value());
        assert_relative(
            "the gas mass with lanes",
            with,
            params.gas_mass().value(),
            0.02,
        );
        assert_relative("the lanes' effect on the mass", with, without, 1e-9);
    }
}

/// The six points the noise golden pins: on a lattice point of every octave, a hair below a
/// lattice face, at the Sun's radius in and above the plane, near the centre, and on the root
/// cube's far corner, where the lattice's last planes are.
fn noise_points() -> [GalacticPosition; 6] {
    let at = |cell: [i32; 3], offset: [f64; 3]| {
        GalacticPosition::new(LyCell::new(cell), offset.map(|f| f * METRES_PER_LIGHT_YEAR))
            .expect("a canonical offset")
    };
    [
        at([2_048, -3_072, 1_024], [0.0; 3]),
        at([1_023, 511, -65], [0.999_999_999, 0.5, 0.25]),
        at([26_013, -418, 0], [0.7, 0.7, 0.0]),
        at([22_516, 13_000, 1_750], [0.7, 0.0, 0.9]),
        at([-37, 120, -12], [0.125, 0.875, 0.5]),
        at([65_535, -65_536, 65_535], [0.999, 0.0, 0.5]),
    ]
}

/// The lattice noise of P07.T4, bit for bit: the log-normal factor at six points for two seeds,
/// every octave and the three coarsest, at the fixture's `σ_ln`. It moves if the lattice word, the
/// lattice normals, the fade, the normalisation, the octave table or the smoothing rule changes.
#[test]
fn gas_noise_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut cache = NoiseCache::with_capacity(256);
    let scales = [
        ("full", SmoothingScale::Full),
        (
            "at_least_250",
            SmoothingScale::AtLeast(LightYears::new(250.0)),
        ),
    ];
    for raw in [0x0000_0000_0000_0001, 0xdead_beef_cafe_f00d] {
        let seed = Seed::new(raw);
        w.line(&format!("# seed {seed}"));
        for (i, p) in noise_points().iter().enumerate() {
            for (name, scale) in scales {
                let factor = log_normal_factor(seed, p, 2.3, scale, &mut cache);
                w.f64(&format!("{seed}.point{i}.{name}"), factor);
            }
        }
    }
    golden!("gas/noise", w.as_str());
}

/// The fixed point the ensemble tests read: inside a lattice cell of every octave, at a fraction
/// of 0.23–0.81 across it on every axis, so that every octave's normalisation matters there.
fn ensemble_point() -> GalacticPosition {
    GalacticPosition::from_light_years([26_013.7, -417.3, 300.9]).expect("inside the root cube")
}

/// The log-normal factor keeps its mean: over 10⁴ seeds at a fixed point its ensemble mean is 1
/// within four standard errors, the error taken from the log-normal's own variance `e^(σ_eff²) −
/// 1`, for `σ_ln` of 2.0 and 2.5 and for the full field and the field smoothed to 250 ly; and its
/// logarithm's variance is `σ_eff²` to 10% (P07.T4.b's fast counterpart; the slow test in
/// `gas_statistics.rs` takes 10⁶ seeds and 2%).
#[test]
fn the_log_normal_factor_keeps_its_mean_over_10000_seeds() {
    let p = ensemble_point();
    let seeds = || (0..10_000_u64).map(|n| Seed::new(0x0704_b000_0000_0000 | n));
    let mut cache = NoiseCache::with_capacity(64);
    for sigma in [2.0, 2.5] {
        for scale in [
            SmoothingScale::Full,
            SmoothingScale::AtLeast(LightYears::new(250.0)),
        ] {
            let variance = sigma * sigma * scale.kept_variance();
            let mean = ensemble_mean(
                |seed| log_normal_factor(seed, &p, sigma, scale, &mut cache),
                seeds(),
            );
            let standard_error = (math::exp_m1(variance) / 1e4).sqrt();
            assert!(
                (mean.mean - 1.0).abs() < 4.0 * standard_error,
                "σ {sigma}, {scale:?}: the mean factor is {} ± {standard_error}",
                mean.mean
            );
            let logs = ensemble_mean(
                |seed| math::ln(log_normal_factor(seed, &p, sigma, scale, &mut cache)),
                seeds(),
            );
            assert_relative(
                &format!("σ {sigma}, {scale:?}: the variance of ln F"),
                logs.variance,
                variance,
                0.10,
            );
            assert_relative(
                &format!("σ {sigma}, {scale:?}: the mean of ln F"),
                logs.mean,
                -0.5 * variance,
                0.05,
            );
        }
    }
}

/// The Milky Way fixture's stellar fields, which a gas field copies its arms and metallicity from.
fn milky_way_fields() -> Fields {
    let params = GalaxyParams::milky_way_like();
    Fields::new(&params, &MassModel::new(&params))
}

/// The fixture's gas on the fixture's fields, with `seed`'s noise.
fn milky_way_gas(fields: &Fields, seed: Seed) -> GasField {
    GasField::with_params(seed, GasParams::milky_way_like(), fields)
}

fn at(ly: [f64; 3]) -> GalacticPosition {
    GalacticPosition::from_light_years(ly).expect("inside the root cube")
}

/// A point drawn uniformly from the box `±half` ly in x and y and `±height` ly in z.
fn random_point(lcg: &mut Lcg, half: f64, height: f64) -> GalacticPosition {
    let mut coordinate = |extent: f64| extent * (2.0 * lcg.next_f64() - 1.0);
    at([coordinate(half), coordinate(half), coordinate(height)])
}

/// The gas field is part of the shareable galaxy handle: it holds no cell and no lock.
#[test]
fn the_gas_field_is_send_sync_and_unwind_safe() {
    fn shareable<T: Send + Sync + RefUnwindSafe + UnwindSafe + Clone + Debug + PartialEq>() {}
    fn movable<T: Send + Sync + Clone + Debug>() {}
    shareable::<GasField>();
    movable::<NoiseCache>();
}

/// Plan 09's shell reads the gas smoothed to 250 ly: over 10⁴ points of the disc its density keeps
/// the mean field's mean — the log-normal factor `(n − n_cor) ÷ (n̄ − n_cor)` averages 1 within
/// four standard errors, the error from the log-normal's own variance at `σ_eff` — and its
/// logarithm varies less than the full field's.
#[test]
fn the_state_smoothed_to_250_ly_keeps_the_mean_and_loses_variance() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(0x0706_a000));
    let corona = gas.smooth().corona_density();
    let smoothed = SmoothingScale::AtLeast(LightYears::new(250.0));
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut lcg = Lcg::new(0x0706_000a);
    let (mut factors, mut logs_smoothed, mut logs_full) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..10_000 {
        let p = random_point(&mut lcg, 50_000.0, 1_000.0);
        let disc = gas.mean_density(&p).value() - corona;
        let state = gas.state(&p, smoothed, &mut cache);
        let factor = (state.density().value() - corona) / disc;
        let full = (gas.density(&p, SmoothingScale::Full, &mut cache).value() - corona) / disc;
        factors.push(factor);
        logs_smoothed.push(math::ln(factor));
        logs_full.push(math::ln(full));
    }
    let n = 10_000.0;
    let mean = factors.iter().sum::<f64>() / n;
    let sigma = gas.params().sigma_ln();
    let error = (math::exp_m1(sigma * sigma * smoothed.kept_variance()) / n).sqrt();
    assert!(
        (mean - 1.0).abs() < 4.0 * error,
        "the smoothed factor averages {mean} ± {error}"
    );
    let variance = |logs: &[f64]| {
        let m = logs.iter().sum::<f64>() / n;
        logs.iter().map(|l| (l - m) * (l - m)).sum::<f64>() / (n - 1.0)
    };
    let (smooth_variance, full_variance) = (variance(&logs_smoothed), variance(&logs_full));
    assert!(
        smooth_variance < full_variance,
        "ln F varies {smooth_variance} smoothed and {full_variance} in full"
    );
}

/// A site's pressure is never below the floor, anywhere in the root cube, and its phase and
/// temperature agree.
#[test]
fn a_states_pressure_is_never_below_the_floor() {
    let fields = milky_way_fields();
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut lcg = Lcg::new(0x0706_000f);
    for n in 0..4 {
        let gas = milky_way_gas(&fields, Seed::new(0x0706_f000 | n));
        let floor = gas.params().pressure_floor();
        for _ in 0..2_500 {
            let p = random_point(&mut lcg, 65_000.0, 65_000.0);
            let state = gas.state(&p, SmoothingScale::Full, &mut cache);
            assert!(
                state.pressure() >= floor,
                "{:?} below {floor:?}",
                state.pressure()
            );
            assert_eq!(state.phase(), gas.phase(state.density(), state.pressure()));
            let t = state.temperature().value();
            match state.phase() {
                GasPhase::Hot => assert!(t > 1e5, "{state:?}"),
                GasPhase::Warm => {
                    // Ruling 98: warm gas reads 5,000–10,000 K.
                    assert!((5_000.0..=10_000.0).contains(&t), "{state:?}");
                }
                GasPhase::Cold | GasPhase::Molecular => assert!(t <= 5_000.0, "{state:?}"),
            }
            // Rulings 91 and 98: T × x n = P with the state's own particle count, wherever the
            // warm clamp does not bind.
            let balance = t * state.particles_per_hydrogen() * state.density().value()
                / state.pressure().value();
            if !state.temperature_clamped() {
                assert!((balance - 1.0).abs() < 1e-15, "{state:?}");
            }
            assert!(state.thermal_sound_speed().value() > 0.0);
            assert!(state.isothermal_sound_speed() < state.thermal_sound_speed());
        }
    }
}

/// The hazard API: inside a hole a ship reads the hole's interior density, outside it the field
/// unchanged, bit for bit; a cloud adds its central density at its centre; and with no modifiers
/// the hazard density is the full-noise density.
#[test]
fn the_hazard_density_applies_holes_and_clouds_by_the_point_rule() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(0x0706_04a2));
    let mut cache = NoiseCache::with_capacity(4_096);
    let centre = at([0.0, 26_000.0, 10.0]);
    let hole = GasModifier::Hole {
        centre,
        radius: LightYears::new(150.0),
        interior: HydrogenPerCm3::new(0.004),
    };
    let inside = at([60.0, 26_050.0, 20.0]);
    let outside = at([400.0, 26_050.0, 20.0]);
    assert_same_bits(
        gas.density_with(&inside, &[hole], &mut cache).value(),
        0.004,
    );
    let field = gas
        .density(&outside, SmoothingScale::Full, &mut cache)
        .value();
    assert_same_bits(
        gas.density_with(&outside, &[hole], &mut cache).value(),
        field,
    );
    let mut none = Vec::new();
    NoModifiers.modifiers_near_segment(&inside, &outside, &mut none);
    assert!(none.is_empty());
    for p in [inside, outside, centre] {
        assert_same_bits(
            gas.density_with(&p, &none, &mut cache).value(),
            gas.density(&p, SmoothingScale::Full, &mut cache).value(),
        );
    }
    let cloud = GasModifier::Cloud {
        centre,
        core_radius: LightYears::new(6.0),
        central_density: HydrogenPerCm3::new(1_000.0),
        dust_per_hydrogen: 1.0,
    };
    let at_centre = gas.density_with(&centre, &[cloud], &mut cache).value();
    let without = gas
        .density(&centre, SmoothingScale::Full, &mut cache)
        .value();
    assert!(
        (at_centre - without - 1_000.0).abs() < 1e-9,
        "{at_centre} against {without}"
    );
}

/// The dust per hydrogen nucleus is `10^[M/H]` of the young thin disc's gas: 1 where its mean
/// metallicity is 0, never above `10^0.5`, the same along a vertical line, and the young disc's own
/// mean at every radius.
#[test]
fn the_dust_follows_the_young_discs_metallicity() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(1));
    let young = fields
        .components()
        .iter()
        .find(|c| c.population() == Population::YoungThinDisc)
        .expect("the fields hold the young thin disc");
    let feh = |r: f64| {
        young
            .metallicity(&PointLy::new(r, 0.0, 0.0), Years::ZERO)
            .mean()
            .value()
    };
    // The solar radius of the young disc's metallicity, by bisection on its mean.
    let solar = hyperion_sim::galaxy::quad::bisect(feh, 1_000.0, 60_000.0, 80);
    assert!(feh(solar).abs() < 1e-12);
    let zeta = gas.dust_per_hydrogen(&at([solar, 0.0, 0.0]));
    assert!((zeta - 1.0).abs() < 1e-9, "ζ = {zeta} where [M/H] = 0");
    for i in 0..=130 {
        let r = 500.0 * f64::from(i);
        let zeta = gas.dust_per_hydrogen(&at([0.0, r, 0.0]));
        assert!(zeta <= math::exp10(0.5), "ζ = {zeta} at {r} ly");
        assert_relative("ζ", zeta, math::exp10(feh(r)), 1e-12);
        let high = gas.dust_per_hydrogen(&at([0.0, r, 3_000.0]));
        assert_relative("ζ along a vertical line", high, zeta, 1e-12);
    }
}

/// Two fields from one seed agree bit for bit at 1,000 points; a field from another seed differs.
#[test]
fn the_gas_field_is_a_pure_function_of_its_seed() {
    let params = GalaxyParams::from_seed(Seed::new(0x0706_0d00), MassFunctionKind::default());
    let fields = Fields::new(&params, &MassModel::new(&params));
    let seed = Seed::new(0x0706_0d00);
    let one = GasField::new(seed, &params, &fields).expect("a drawn galaxy's gas");
    let two = GasField::new(seed, &params, &fields).expect("a drawn galaxy's gas");
    let other =
        GasField::new(Seed::new(0x0706_0d01), &params, &fields).expect("a drawn galaxy's gas");
    assert_eq!(one, two);
    assert_ne!(one, other);
    let (mut first_cache, mut second_cache) = (
        NoiseCache::with_capacity(4_096),
        NoiseCache::with_capacity(7),
    );
    let mut lcg = Lcg::new(0x0706_000d);
    let mut differ = 0;
    for _ in 0..1_000 {
        let p = random_point(&mut lcg, 60_000.0, 4_000.0);
        let first = one.state(&p, SmoothingScale::Full, &mut first_cache);
        let second = two.state(&p, SmoothingScale::Full, &mut second_cache);
        assert_same_bits(first.density().value(), second.density().value());
        assert_same_bits(first.pressure().value(), second.pressure().value());
        assert_eq!(first.phase(), second.phase());
        assert_same_bits(one.mean_density(&p).value(), two.mean_density(&p).value());
        let third = other
            .density(&p, SmoothingScale::Full, &mut first_cache)
            .value();
        if (third - first.density().value()).abs() > 1e-12 * third {
            differ += 1;
        }
    }
    assert!(
        differ > 990,
        "only {differ} of 1,000 points differ between seeds"
    );
}

/// The twelve positions the field golden pins: the centre, the molecular disc's edge, the bar's
/// end, a lane and the arm beside it at the Sun's radius, the Sun itself and above it, the outer
/// disc, the corona, and the cube's far reaches.
fn field_points() -> [GalacticPosition; 12] {
    [
        at([0.3, 0.2, 0.1]),
        at([250.0, -120.0, 30.0]),
        at([15_900.0, 1_200.0, -40.0]),
        at([17_300.0, 19_800.0, 5.0]),
        at([-17_600.0, -20_100.0, -5.0]),
        at([0.0, 26_000.0, 20.0]),
        at([0.0, 26_000.0, 1_500.0]),
        at([-41_000.0, 3_000.0, 100.0]),
        at([2_000.0, -30_000.0, 12_000.0]),
        at([500.0, 500.0, -40_000.0]),
        at([-65_000.0, 64_000.0, 1.0]),
        at([63_000.0, -2_000.0, 65_000.0]),
    ]
}

/// P07.T6.a's field, bit for bit: at twelve positions for two seeds' own galaxies, the mean and the
/// realised density, the pressure, the phase and the dust-to-gas ratio.
#[test]
fn gas_field_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut cache = NoiseCache::with_capacity(4_096);
    for raw in [PINNED_SEEDS[0], PINNED_SEEDS[2]] {
        let seed = Seed::new(raw);
        w.line(&format!("# seed {seed}"));
        let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let fields = Fields::new(&params, &MassModel::new(&params));
        let gas = GasField::new(seed, &params, &fields).expect("a drawn galaxy's gas");
        for (i, p) in field_points().iter().enumerate() {
            let label = |name: &str| format!("{seed}.point{i:02}.{name}");
            let state = gas.state(p, SmoothingScale::Full, &mut cache);
            w.f64(&label("mean_density"), gas.mean_density(p).value());
            w.f64(&label("density"), state.density().value());
            w.f64(&label("pressure"), state.pressure().value());
            w.line(&format!("{} = {:?}", label("phase"), state.phase()));
            w.f64(&label("neutral_share"), state.neutral_share());
            w.f64(&label("temperature"), state.temperature().value());
            w.f64(&label("dust_per_hydrogen"), gas.dust_per_hydrogen(p));
        }
    }
    golden!("gas/field", w.as_str());
}

/// The corner of plan 02's ranges that ruling 22 names: the lightest thin disc, the least gas and
/// the longest gas disc, with the longest bar, whose large hole makes the warm layer's density at
/// the Sun's radius stand for the most mass. Some 34 seeds in 4,096 draw a warm layer heavier than
/// its 2.5 × 10⁹ M☉ of gas, which ruling 22 refused until ruling 91 clamped the layer.
fn corner_galaxy() -> GalaxyParams {
    GalaxyParamsBuilder::new()
        .stellar_mass(SolarMasses::new(3e10))
        .thick_share(0.14)
        .bulge_bar_share(0.35)
        .nuclear_disc_share(0.025)
        .halo_share(0.014)
        .gas_mass_fraction(0.175)
        .thin_length(LightYears::new(11_500.0))
        .gas_length_ratio(2.0)
        .bar_half_length(LightYears::new(18_000.0))
        .build()
        .expect("every value is inside plan 02's ranges")
}

/// Ruling 91: at the corner that ruling 22 refused, every one of 4,096 seeds builds, with at least
/// half its gas neutral. The warm layer is clamped where it would weigh more than half the gas less
/// the molecular disc — on some seeds of the corner, and on no drawn galaxy — and elsewhere keeps
/// its drawn density; the clamped galaxies build through `GasField::new` and `Galaxy::from_params`
/// too. The error ruling 22 added stays in the API, and reads as it did.
#[test]
fn the_warm_layer_is_clamped_so_that_every_galaxy_keeps_half_its_gas_neutral() {
    let params = corner_galaxy();
    let fields = Fields::new(&params, &MassModel::new(&params));
    let (mut clamped, mut least) = (None, f64::INFINITY);
    for n in 0..4_096 {
        let seed = Seed::new(0x0722_0000 | n);
        let gas = GasParams::from_galaxy(seed, &params).expect("the clamp leaves half the gas");
        let share = gas.neutral_fraction();
        assert!(share > 0.5 - 1e-12, "{seed}: {share}");
        least = least.min(share);
        if (share - 0.5).abs() < 1e-12 {
            clamped = clamped.or(Some(seed));
        }
    }
    assert!((least - 0.5).abs() < 1e-12, "the least share is {least}");
    let seed = clamped.expect("the clamp binds at the corner");
    let gas = GasField::new(seed, &params, &fields).expect("a clamped galaxy's gas");
    assert!((gas.params().neutral_fraction() - 0.5).abs() < 1e-12);
    assert!(gas.params().warm_density().value() < 0.025);
    Galaxy::from_params(seed, params.clone()).expect("a clamped galaxy builds");
    let error = BuildGasParamsError::NoNeutralGas {
        gas_mass: params.gas_disc().mass(),
        warm_mass: params.gas_disc().mass(),
        molecular_mass: SolarMasses::new(3e6),
    };
    assert!(
        error
            .to_string()
            .ends_with("M☉ gas disc for the neutral layer"),
        "{error}"
    );
    let wrapped = BuildGalaxyError::Gas(error);
    let source = std::error::Error::source(&wrapped).map(ToString::to_string);
    assert_eq!(source, Some(error.to_string()));
}

/// The mean neutral gas over a cell is bounded above by `neutral_bound` at every point of the cell
/// as computed, for a few cells of 128 ly and 4,096 ly at the bar's end, across a lane, at the
/// neutral disc's peak radius, at the Sun, above the plane and at the centre (P07.T6.b's fast suite;
/// `gas_statistics.rs` hunts over 10⁴ cells).
#[test]
fn the_neutral_bound_holds_over_a_few_cells() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(2));
    let geometry = fields.arms();
    let lane_r = 20_000.0;
    let lane_theta = geometry.ridge_azimuth(lane_r + gas.lanes().shift().value(), 0);
    let (lane_x, lane_y) = (
        lane_r * math::cos(lane_theta),
        lane_r * math::sin(lane_theta),
    );
    let floor = |v: f64, edge: u32| whole_ly((v / f64::from(edge)).floor() * f64::from(edge));
    let mut lcg = Lcg::new(0x0706_000b);
    for edge in [128_u32, 4_096] {
        let peak = gas.smooth().neutral_peak_radius();
        for (x, y, z) in [
            (16_000.0, 100.0, 0.0),
            (lane_x, lane_y, 0.0),
            (peak, 1_000.0, 0.0),
            (0.0, 26_000.0, 0.0),
            (-9_000.0, -20_000.0, 1_000.0),
            (1.0, 1.0, 0.0),
            (-1.0, 1.0, -1.0),
        ] {
            let min = [floor(x, edge), floor(y, edge), floor(z, edge)];
            let cell = CellBox::new(min, edge).expect("a cell of the cube");
            let bound = gas.neutral_bound(&cell).value();
            for _ in 0..2_000 {
                let point =
                    [0, 1, 2].map(|axis| f64::from(min[axis]) + f64::from(edge) * lcg.next_f64());
                let n = gas.mean_neutral_density(&at(point)).value();
                assert!(
                    n <= bound,
                    "{n} above the bound {bound} of {cell:?} at {point:?}"
                );
            }
        }
    }
}

/// P07.T7's law, bit for bit: `A_λ ÷ A_V` at the ten bands.
#[test]
fn gas_ccm_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for band in Band::ALL {
        w.f64(&format!("ratio.{}", band.name()), band.ratio());
    }
    for microns in [0.1, 0.125, 0.2, 0.2175, 0.3, 0.95, 1.0, 1.05, 3.3, 5.0] {
        let ratio = extinction_ratio(Micrometres::new(microns)).expect("inside the law's range");
        w.f64(&format!("ratio.at_{microns}_um"), ratio);
    }
    w.f64("hydrogen_column_per_mag", HYDROGEN_COLUMN_PER_MAG);
    golden!("gas/ccm", w.as_str());
}

/// The dust-weighted density `(n̄ − n_cor) ζ` along the segment from `a` to `b` by a midpoint sum
/// of `steps` points in float light-years, cm⁻² once times the length: an independent quadrature of
/// the mean field, which shares none of the integral's pieces, rule or step.
fn reference_a_v(gas: &GasField, a: [f64; 3], b: [f64; 3], steps: u32) -> f64 {
    let corona = gas.smooth().corona_density();
    let delta = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    let mut sum = 0.0;
    for i in 0..steps {
        let t = (f64::from(i) + 0.5) / f64::from(steps);
        let p = at([0, 1, 2].map(|axis| a[axis] + t * (b[axis] - a[axis])));
        sum += (gas.mean_density(&p).value() - corona) * gas.dust_per_hydrogen(&p);
    }
    sum / f64::from(steps) * length * CENTIMETRES_PER_LIGHT_YEAR / HYDROGEN_COLUMN_PER_MAG
}

/// P07.T8.a: the mean-mode integral against an independent midpoint quadrature of the mean field
/// to 0.1%, on an in-plane line at the Sun's radius, the line from the Sun to the centre, a line
/// through the molecular disc and one steeply through the plane.
#[test]
fn the_mean_integral_matches_an_independent_quadrature() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(3));
    let mut cache = NoiseCache::with_capacity(0);
    for (a, b, steps) in [
        ([-1_500.0, 26_000.0, 0.0], [1_500.0, 26_000.0, 0.0], 30_000),
        ([26_000.0, 0.0, 0.0], [0.0, 0.0, 0.0], 260_000),
        ([-900.0, 40.0, 10.0], [900.0, -40.0, -10.0], 180_000),
        (
            [100.0, 26_000.0, -3_000.0],
            [-400.0, 25_600.0, 3_000.0],
            60_000,
        ),
    ] {
        let line = sightline(
            &gas,
            &at(a),
            &at(b),
            NoiseMode::Mean,
            Quality::Full,
            &[],
            &mut cache,
        );
        let reference = reference_a_v(&gas, a, b, steps);
        assert_relative(
            &format!("A_V from {a:?} to {b:?}"),
            line.a_v().value(),
            reference,
            1e-3,
        );
    }
}

/// P07.T8.a: a vertical line from the plane to the root cube's edge carries the closed-form
/// column of the three layers, lanes on, and the corona to 0.1%, and its extinction is that column's
/// dust over `HYDROGEN_COLUMN_PER_MAG`.
#[test]
fn a_vertical_line_carries_the_closed_form_column() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(4));
    let mut cache = NoiseCache::with_capacity(0);
    for (x, y) in [
        (0.0, 26_000.0),
        (17_300.0, 19_800.0),
        (9_000.0, 0.0),
        (150.0, 100.0),
    ] {
        let top = 65_535.0;
        let line = sightline(
            &gas,
            &at([x, y, 0.0]),
            &at([x, y, top]),
            NoiseMode::Mean,
            Quality::Full,
            &[],
            &mut cache,
        );
        let r = (x * x + y * y).sqrt();
        let smooth = gas.smooth();
        let lane_factor = gas.lanes().factor(r, math::atan2(y, x));
        let disc = lane_factor * smooth.column_between(GasLayer::Neutral, r, 0.0, top)
            + smooth.column_between(GasLayer::Warm, r, 0.0, top)
            + smooth.column_between(GasLayer::Molecular, r, 0.0, top);
        let corona = smooth.corona_density() * top * CENTIMETRES_PER_LIGHT_YEAR;
        let at_r = at([x, y, 0.0]);
        assert_relative(
            "the hydrogen column",
            line.hydrogen_column().value(),
            disc + corona,
            1e-3,
        );
        let dust = disc * gas.dust_per_hydrogen(&at_r) / HYDROGEN_COLUMN_PER_MAG;
        assert_relative("the polar extinction", line.a_v().value(), dust, 1e-3);
    }
}

/// P07.T8.a and Design note 15: two-way visibility is exact. `A(a, b)` and `A(b, a)` are the same
/// bits for 10⁴ random pairs across the root cube, in both modes (at a budget of 16 steps, which
/// the canonical order serves as it serves every quality) and for a hundred at full quality.
#[test]
fn two_way_visibility_is_exact() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(5));
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut lcg = Lcg::new(0x0708_0a0b);
    let sixteen = Quality::Budget(NonZeroU32::new(16).expect("sixteen"));
    for i in 0..10_000 {
        let a = random_point(&mut lcg, 65_000.0, 30_000.0);
        let b = random_point(&mut lcg, 65_000.0, 30_000.0);
        let (mode, quality) = match i % 100 {
            0 => (NoiseMode::Realised, Quality::Full),
            1 => (NoiseMode::Mean, Quality::Full),
            k if k % 2 == 0 => (NoiseMode::Realised, sixteen),
            _ => (NoiseMode::Mean, sixteen),
        };
        let there = sightline(&gas, &a, &b, mode, quality, &[], &mut cache);
        let back = sightline(&gas, &b, &a, mode, quality, &[], &mut cache);
        assert_same_bits(there.a_v().value(), back.a_v().value());
        assert_same_bits(
            there.hydrogen_column().value(),
            back.hydrogen_column().value(),
        );
        assert_same_bits(
            there.neutral_hydrogen_column().value(),
            back.neutral_hydrogen_column().value(),
        );
        assert_eq!(there.steps(), back.steps());
    }
}

/// P07.T8.a: a segment of no length passes through nothing, and a segment wholly outside the slab
/// takes no steps and carries only the corona's dust-free column.
#[test]
fn a_point_is_nothing_and_the_corona_takes_no_steps() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(6));
    let mut cache = NoiseCache::with_capacity(64);
    let p = at([1.0, 26_000.0, 3.0]);
    let point = sightline(
        &gas,
        &p,
        &p,
        NoiseMode::Realised,
        Quality::Full,
        &[],
        &mut cache,
    );
    assert_eq!(point, Sightline::ZERO);
    let (a, b) = (at([0.0, 0.0, 40_000.0]), at([20_000.0, 5_000.0, 45_000.0]));
    let high = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Realised,
        Quality::Full,
        &[],
        &mut cache,
    );
    assert_eq!(high.steps(), 0);
    assert_same_bits(high.a_v().value(), 0.0);
    let length = a.distance_to(&b).value() / METRES_PER_LIGHT_YEAR;
    let corona = gas.smooth().corona_density() * length * CENTIMETRES_PER_LIGHT_YEAR;
    assert_relative(
        "the corona's column",
        high.hydrogen_column().value(),
        corona,
        1e-12,
    );
}

/// P07.T8.b: the realised line keeps the mean. Over 256 seeds the mean extinction of a fixed
/// 3,000 ly in-plane segment at the Sun's radius agrees with the mean mode's within four standard
/// errors, at full quality and at a budget of 64 steps; a budget is never exceeded; and a warm
/// cache changes nothing.
#[test]
fn the_realised_line_keeps_the_mean_over_256_seeds() {
    let fields = milky_way_fields();
    let (a, b) = (
        at([-1_500.0, 26_000.0, 10.0]),
        at([1_500.0, 26_000.0, 10.0]),
    );
    let sixty_four = Quality::Budget(NonZeroU32::new(64).expect("sixty-four"));
    for quality in [Quality::Full, sixty_four] {
        let mut cache = NoiseCache::with_capacity(4_096);
        let mean_line = sightline(
            &milky_way_gas(&fields, Seed::new(7)),
            &a,
            &b,
            NoiseMode::Mean,
            quality,
            &[],
            &mut cache,
        );
        let realised = ensemble_mean(
            |seed| {
                let gas = milky_way_gas(&fields, seed);
                let cold = sightline(&gas, &a, &b, NoiseMode::Realised, quality, &[], &mut cache);
                let warm = sightline(&gas, &a, &b, NoiseMode::Realised, quality, &[], &mut cache);
                assert_eq!(cold, warm, "a warm cache changed the line");
                let fresh = &mut NoiseCache::with_capacity(0);
                assert_eq!(
                    cold,
                    sightline(&gas, &a, &b, NoiseMode::Realised, quality, &[], fresh)
                );
                if let Quality::Budget(steps) = quality {
                    assert!(cold.steps() <= steps.get());
                }
                cold.a_v().value()
            },
            (0..256_u64).map(|n| Seed::new(0x0708_b000_0000_0000 | n)),
        );
        assert!(
            (realised.mean - mean_line.a_v().value()).abs() < 4.0 * realised.standard_error,
            "{quality:?}: the realised mean {} ± {} against the mean mode's {}",
            realised.mean,
            realised.standard_error,
            mean_line.a_v().value()
        );
    }
}

/// P07.T8.c: a hole. A line through the centre of a hole of radius `r` in a uniform field — the
/// corona, above the slab, where it is all there is — loses exactly the column of `2r`.
#[test]
fn a_hole_takes_out_the_gas_of_its_chord() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(8));
    let mut cache = NoiseCache::with_capacity(4_096);
    let radius = 300.0;
    let hole = |centre: [f64; 3], interior: f64| GasModifier::Hole {
        centre: at(centre),
        radius: LightYears::new(radius),
        interior: HydrogenPerCm3::new(interior),
    };
    let (a, b) = (
        at([-2_000.0, 5_000.0, 40_000.0]),
        at([2_000.0, 5_000.0, 40_000.0]),
    );
    let open = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Mean,
        Quality::Full,
        &[],
        &mut cache,
    );
    let holed = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Mean,
        Quality::Full,
        &[hole([10.0, 5_000.0, 40_000.0], 0.0)],
        &mut cache,
    );
    let lost = open.hydrogen_column().value() - holed.hydrogen_column().value();
    let expected = gas.smooth().corona_density() * 2.0 * radius * CENTIMETRES_PER_LIGHT_YEAR;
    assert_relative("the column a hole takes", lost, expected, 1e-9);
}

/// P07.T8.c: in the disc a hole takes the dust of its chord and gives back its interior density;
/// a hole the segment misses changes nothing, bit for bit; and an empty modifier list is the call
/// without any.
#[test]
fn a_hole_in_the_disc_takes_its_chords_dust() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(8));
    let mut cache = NoiseCache::with_capacity(4_096);
    let radius = 300.0;
    let hole = |centre: [f64; 3], interior: f64| GasModifier::Hole {
        centre: at(centre),
        radius: LightYears::new(radius),
        interior: HydrogenPerCm3::new(interior),
    };
    // In the plane: the hole's chord carries its interior and no dust.
    let (a, b) = (at([-1_500.0, 26_000.0, 0.0]), at([1_500.0, 26_000.0, 0.0]));
    let open = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Mean,
        Quality::Full,
        &[],
        &mut cache,
    );
    let mods = [hole([0.0, 26_000.0, 0.0], 0.004)];
    let holed = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Mean,
        Quality::Full,
        &mods,
        &mut cache,
    );
    let (entry, exit) = (at([-radius, 26_000.0, 0.0]), at([radius, 26_000.0, 0.0]));
    let chord = sightline(
        &gas,
        &entry,
        &exit,
        NoiseMode::Mean,
        Quality::Full,
        &[],
        &mut cache,
    );
    assert_relative(
        "the dust a hole takes",
        open.a_v().value() - holed.a_v().value(),
        chord.a_v().value(),
        1e-6,
    );
    let interior = 0.004 * 2.0 * radius * CENTIMETRES_PER_LIGHT_YEAR;
    assert_relative(
        "the gas a hole leaves",
        holed.hydrogen_column().value(),
        open.hydrogen_column().value() - chord.hydrogen_column().value() + interior,
        1e-6,
    );
    // A hole the segment misses, and the source with none.
    let missed = [hole([0.0, 26_000.0, 2_000.0], 0.0)];
    assert_eq!(
        sightline(
            &gas,
            &a,
            &b,
            NoiseMode::Realised,
            Quality::Full,
            &missed,
            &mut cache
        ),
        sightline(
            &gas,
            &a,
            &b,
            NoiseMode::Realised,
            Quality::Full,
            &[],
            &mut cache
        )
    );
    let mut none = Vec::new();
    NoModifiers.modifiers_near_segment(&a, &b, &mut none);
    assert_eq!(
        sightline(
            &gas,
            &a,
            &b,
            NoiseMode::Realised,
            Quality::Full,
            &none,
            &mut cache
        ),
        sightline(
            &gas,
            &a,
            &b,
            NoiseMode::Realised,
            Quality::Full,
            &[],
            &mut cache
        )
    );
}

/// P07.T8.c: a cloud's column along a long line through its centre is `4 a n_c ÷ 3` to 10⁻⁹, the
/// same in both directions, with its own dust and all of it neutral.
#[test]
fn a_clouds_column_through_its_centre_is_four_thirds_of_its_core() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(9));
    let mut cache = NoiseCache::with_capacity(0);
    let (core, central, zeta) = (5.0, 2_000.0, 1.3);
    let cloud = GasModifier::Cloud {
        centre: at([0.0, 26_000.0, 45_000.0]),
        core_radius: LightYears::new(core),
        central_density: HydrogenPerCm3::new(central),
        dust_per_hydrogen: zeta,
    };
    let (a, b) = (
        at([-20_000.0, 26_000.0, 45_000.0]),
        at([20_000.0, 26_000.0, 45_000.0]),
    );
    let open = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Mean,
        Quality::Full,
        &[],
        &mut cache,
    );
    for (from, to) in [(a, b), (b, a)] {
        let line = sightline(
            &gas,
            &from,
            &to,
            NoiseMode::Mean,
            Quality::Full,
            &[cloud],
            &mut cache,
        );
        let column = line.hydrogen_column().value() - open.hydrogen_column().value();
        let expected = 4.0 / 3.0 * core * central * CENTIMETRES_PER_LIGHT_YEAR;
        assert_relative("the cloud's column", column, expected, 1e-9);
        assert_relative(
            "the cloud's neutral column",
            line.neutral_hydrogen_column().value(),
            expected,
            1e-9,
        );
        assert_relative(
            "the cloud's extinction",
            line.a_v().value(),
            zeta * expected / HYDROGEN_COLUMN_PER_MAG,
            1e-9,
        );
    }
}

/// P07.T8.d: the neutral column is never above the whole column on short lines that stay in cold
/// or molecular gas, where every sample is wholly neutral and the corona is summed two ways: step
/// by step into the neutral column, and in closed form into the whole column.
#[test]
fn the_neutral_column_is_never_above_the_whole_on_short_neutral_lines() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(10));
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut lcg = Lcg::new(0x0708_d001);
    let mut neutral_lines = 0;
    for _ in 0..4_000 {
        let r = 200.0 + 30_000.0 * lcg.next_f64();
        let theta = core::f64::consts::TAU * lcg.next_f64();
        let (sin, cos) = math::sin_cos(theta);
        let a = [r * cos, r * sin, 20.0 * (lcg.next_f64() - 0.5)];
        let length = 1.0 + 60.0 * lcg.next_f64();
        let b = [a[0] + length * cos, a[1] + length * sin, a[2]];
        let line = sightline(
            &gas,
            &at(a),
            &at(b),
            NoiseMode::Realised,
            Quality::Full,
            &[],
            &mut cache,
        );
        let (neutral, whole) = (
            line.neutral_hydrogen_column().value(),
            line.hydrogen_column().value(),
        );
        if neutral > 0.999_999 * whole {
            neutral_lines += 1;
        }
        assert!(
            neutral <= whole,
            "{neutral:e} > {whole:e} from {a:?} to {b:?}"
        );
    }
    assert!(
        neutral_lines > 100,
        "only {neutral_lines} wholly neutral lines"
    );
}

/// P07.T8.d: the neutral column is never above the whole column; on in-plane lines of 3,000 ly at
/// the Sun's radius the neutral share is 0.8–1.0 in both modes.
#[test]
fn the_neutral_column_is_most_of_the_plane_and_never_above_the_whole() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(10));
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut lcg = Lcg::new(0x0708_d000);
    for i in 0..600 {
        let a = random_point(&mut lcg, 65_000.0, 20_000.0);
        let b = random_point(&mut lcg, 65_000.0, 20_000.0);
        let mode = if i % 2 == 0 {
            NoiseMode::Mean
        } else {
            NoiseMode::Realised
        };
        let quality = Quality::Budget(NonZeroU32::new(64).expect("sixty-four"));
        let line = sightline(&gas, &a, &b, mode, quality, &[], &mut cache);
        assert!(
            line.neutral_hydrogen_column() <= line.hydrogen_column(),
            "{line:?}"
        );
    }
    for k in 0..16 {
        let theta = core::f64::consts::TAU * f64::from(k) / 16.0;
        let (sin, cos) = math::sin_cos(theta);
        let centre = [26_000.0 * cos, 26_000.0 * sin, 0.0];
        let a = at([centre[0] - 1_500.0 * sin, centre[1] + 1_500.0 * cos, 0.0]);
        let b = at([centre[0] + 1_500.0 * sin, centre[1] - 1_500.0 * cos, 0.0]);
        for mode in [NoiseMode::Mean, NoiseMode::Realised] {
            let line = sightline(&gas, &a, &b, mode, Quality::Full, &[], &mut cache);
            let share = line.neutral_hydrogen_column().value() / line.hydrogen_column().value();
            assert!(
                (0.8..=1.0).contains(&share),
                "{mode:?} at {theta}: a neutral share of {share}"
            );
        }
    }
}

/// P07.T8.d: an instrument's horizon. Looking coreward in the Milky Way plane from 26,000 ly in
/// mean mode, the visual horizon at 5 mag is 7,000–13,000 ly; in K it is longer on every line
/// tried, and radio sees to the end of its range.
#[test]
fn the_visual_horizon_coreward_is_about_ten_thousand_light_years() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(11));
    let mut cache = NoiseCache::with_capacity(4_096);
    let (limit, range) = (Magnitudes::new(5.0), LightYears::new(60_000.0));
    let reach = |origin: &GalacticPosition,
                 direction: UnitVector,
                 band: Band,
                 mode: NoiseMode,
                 cache: &mut NoiseCache| {
        horizon(
            &gas,
            origin,
            direction,
            band,
            limit,
            range,
            mode,
            Quality::Full,
            cache,
        )
    };
    for k in 0..8 {
        let theta = core::f64::consts::TAU * f64::from(k) / 8.0;
        let (sin, cos) = math::sin_cos(theta);
        let sun = at([26_000.0 * cos, 26_000.0 * sin, 0.0]);
        let coreward = UnitVector::from_components([-cos, -sin, 0.0]).expect("a direction");
        let visual = reach(&sun, coreward, Band::V, NoiseMode::Mean, &mut cache).value();
        assert!(
            (7_000.0..=13_000.0).contains(&visual),
            "the V horizon at {theta} is {visual} ly"
        );
        for mode in [NoiseMode::Mean, NoiseMode::Realised] {
            let v = reach(&sun, coreward, Band::V, mode, &mut cache);
            let k_band = reach(&sun, coreward, Band::K, mode, &mut cache);
            assert!(
                k_band > v,
                "{mode:?} at {theta}: K reaches {k_band:?}, V {v:?}"
            );
            assert_eq!(reach(&sun, coreward, Band::Radio, mode, &mut cache), range);
        }
    }
}

/// P07.T9: seen face-on, the fixture's dust at the Sun's radius, averaged around the circle, is
/// twice its polar extinction to 1% — the map's closed form against the line integral — and lies
/// in 0.45–0.75 mag.
///
/// The plan's bracket was 0.3–0.45, twice the 0.18 mag to the pole of version 8. Rulings 1 and 19
/// since set the disc's column to McKee, Parravano and Hollenbach's measured 13.7 ± 1.6 M☉ pc⁻², and
/// a column is its face-on extinction: `ζ × 1.22 × 10²¹ cm⁻² ÷ 1.87 × 10²¹` per 13.7 M☉ pc⁻², so
/// 0.48–0.61 mag at the version-10 fixture's `ζ` of 0.84 and 0.57–0.73 at ruling 21's solar anchor,
/// which the bracket holds (plan 07, Risks).
#[test]
fn the_face_on_dust_at_the_sun_is_twice_its_polar_extinction() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(12));
    let mut cache = NoiseCache::with_capacity(0);
    let (mut face, mut polar) = (0.0, 0.0);
    let azimuths = 256;
    for k in 0..azimuths {
        let theta = core::f64::consts::TAU * f64::from(k) / f64::from(azimuths);
        let (sin, cos) = math::sin_cos(theta);
        let (x, y) = (26_000.0 * cos, 26_000.0 * sin);
        face += extinction_face_on(&gas, x, y, 0.0).value();
        let line = sightline(
            &gas,
            &at([x, y, 0.0]),
            &at([x, y, 65_535.0]),
            NoiseMode::Mean,
            Quality::Full,
            &[],
            &mut cache,
        );
        polar += line.a_v().value();
    }
    let (face, polar) = (face / f64::from(azimuths), polar / f64::from(azimuths));
    assert_relative("face-on against twice the polar", face, 2.0 * polar, 0.01);
    assert!(
        (0.45..=0.75).contains(&face),
        "{face} mag face-on at the Sun's radius"
    );
}

/// P07.T9: the lanes show. At 26,000 ly a 256 ly pixel on a lane holds at least 1.5 times the dust
/// of one half-way to the next lane at the same radius.
#[test]
fn the_face_on_map_shows_the_lanes() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(13));
    let r = 26_000.0;
    let arms = fields.arms();
    let count = f64::from(arms.count().get());
    for j in 0..arms.count().get() {
        let lane = arms.ridge_azimuth(r + gas.lanes().shift().value(), j);
        let between = lane + core::f64::consts::PI / count;
        let pixel = |theta: f64| {
            let (sin, cos) = math::sin_cos(theta);
            extinction_face_on(&gas, r * cos, r * sin, 256.0).value()
        };
        let contrast = pixel(lane) / pixel(between);
        assert!(contrast >= 1.5, "lane {j}: a contrast of {contrast}");
    }
}

/// The plan 04 raster edge-on: 512 by 256 pixels of 256 ly across the whole root cube.
fn edge_on_spec(size: [u32; 2]) -> MapSpec {
    let ly_per_px = 131_072.0 / f64::from(size[0]);
    MapSpec::new(
        MapView::EdgeOn,
        MapSelection::AllSystems,
        size,
        [0.0, 0.0],
        ly_per_px,
    )
    .expect("a map of the whole cube")
}

/// P07.T9: edge-on, the centre is behind more than 25 magnitudes in the pixels beside the plane,
/// and the raster is its own mirror image under `z → −z`, bit for bit.
#[test]
fn edge_on_the_centre_is_dark_and_the_map_is_symmetric() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(14));
    let spec = edge_on_spec([64, 32]);
    let mut rows = Vec::new();
    render_extinction_rows(&gas, &spec, 0..32, &mut rows);
    for row in 0..16 {
        let (top, bottom) = (row * 64, (31 - row) * 64);
        for column in 0..64 {
            assert_same_bits(rows[top + column], rows[bottom + column]);
        }
    }
    let full = edge_on_spec([512, 256]);
    for row in [127, 128] {
        let [z_lo, z_hi] = full.pixel_span(row);
        for column in [255, 256] {
            let x = full.pixel_centre(column, row)[0];
            let dark = extinction_edge_on(&gas, x, z_lo, z_hi).value();
            assert!(
                dark > 25.0,
                "{dark} mag through the centre at ({x}, {z_lo}–{z_hi})"
            );
        }
    }
}

/// P07.T9: seen face-on the dust is point-symmetric about the centre, since two and four arms
/// repeat every half-turn, to rounding.
#[test]
fn face_on_the_dust_repeats_every_half_turn() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(15));
    let mut lcg = Lcg::new(0x0709_f0ce);
    for _ in 0..500 {
        let [x, y] = [0; 2].map(|_| 60_000.0 * (2.0 * lcg.next_f64() - 1.0));
        let here = extinction_face_on(&gas, x, y, 256.0).value();
        let there = extinction_face_on(&gas, -x, -y, 256.0).value();
        assert_relative("the half-turn", there, here, 1e-9);
    }
}

/// P07.T9: a pixel depends on the pixel alone. Rows rendered in two bands are the rows rendered in
/// one, bit for bit, in both views, and each is the single-pixel function's value.
#[test]
fn rows_rendered_in_bands_are_the_rows_rendered_whole() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(16));
    let face = MapSpec::new(
        MapView::FaceOn,
        MapSelection::YoungOnly,
        [24, 16],
        [1_000.0, 20_000.0],
        1_024.0,
    )
    .expect("a face-on map");
    for spec in [face, edge_on_spec([24, 12])] {
        let rows = spec.height_px();
        let (mut whole, mut top, mut bottom) = (Vec::new(), Vec::new(), Vec::new());
        render_extinction_rows(&gas, &spec, 0..rows, &mut whole);
        render_extinction_rows(&gas, &spec, 0..5, &mut top);
        render_extinction_rows(&gas, &spec, 5..rows, &mut bottom);
        top.extend_from_slice(&bottom);
        assert_eq!(whole.len(), top.len());
        for (a, b) in whole.iter().zip(&top) {
            assert_same_bits(*a, *b);
        }
        let width = usize::try_from(spec.width_px()).expect("a small map");
        for row in [0, rows - 1] {
            for column in [0, spec.width_px() - 1] {
                let [x, y] = spec.pixel_centre(column, row);
                let single = match spec.view() {
                    MapView::FaceOn => extinction_face_on(&gas, x, y, spec.ly_per_px()),
                    MapView::EdgeOn => {
                        let [z_lo, z_hi] = spec.pixel_span(row);
                        extinction_edge_on(&gas, x, z_lo, z_hi)
                    }
                };
                let index = usize::try_from(row).expect("a row") * width
                    + usize::try_from(column).expect("a column");
                assert_same_bits(whole[index], single.value());
            }
        }
    }
}

/// P07.T9's maps, bit for bit: a 64 × 64 face-on map of the whole root cube and a 64 × 32 edge-on
/// one, for the fixture's gas with one seed's noise (which the maps do not read).
#[test]
fn gas_map_is_pinned() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(1));
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let face = MapSpec::new(
        MapView::FaceOn,
        MapSelection::AllSystems,
        [64, 64],
        [0.0, 0.0],
        2_048.0,
    )
    .expect("a map of the whole cube");
    for (name, spec) in [("face_on", face), ("edge_on", edge_on_spec([64, 32]))] {
        let mut pixels = Vec::new();
        render_extinction_rows(&gas, &spec, 0..spec.height_px(), &mut pixels);
        let width = usize::try_from(spec.width_px()).expect("a small map");
        for (i, value) in pixels.iter().enumerate() {
            w.f64(&format!("{name}.{:02}.{:02}", i / width, i % width), *value);
        }
    }
    golden!("gas/map", w.as_str());
}

/// `horizon`'s edges: a limit or a range that is not positive reaches nowhere, a range past 2¹⁹ ly
/// is looked along only that far and, where the limit is never reached, is returned as asked; and
/// an origin whose range would leave the coordinates' `i32` returns its range.
#[test]
fn a_horizons_edges_are_as_documented() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(17));
    let mut cache = NoiseCache::with_capacity(64);
    let sun = at([0.0, 26_000.0, 0.0]);
    let up = UnitVector::NORTH;
    let (mode, quality) = (NoiseMode::Mean, Quality::Full);
    let reach = |limit: f64, range: f64, origin: &GalacticPosition, cache: &mut NoiseCache| {
        horizon(
            &gas,
            origin,
            up,
            Band::V,
            Magnitudes::new(limit),
            LightYears::new(range),
            mode,
            quality,
            cache,
        )
    };
    assert_eq!(reach(0.0, 1_000.0, &sun, &mut cache), LightYears::ZERO);
    assert_eq!(reach(-1.0, 1_000.0, &sun, &mut cache), LightYears::ZERO);
    assert_eq!(reach(f64::NAN, 1_000.0, &sun, &mut cache), LightYears::ZERO);
    assert_eq!(reach(1.0, 0.0, &sun, &mut cache), LightYears::ZERO);
    assert_eq!(reach(1.0, f64::NAN, &sun, &mut cache), LightYears::ZERO);
    // A quarter of a magnitude to the pole: 5 mag is never reached, even over a billion light-years.
    assert_eq!(reach(5.0, 1e9, &sun, &mut cache), LightYears::new(1e9));
    // Half the pole's extinction is reached within the neutral layer.
    let half = 0.5
        * sightline(
            &gas,
            &sun,
            &at([0.0, 26_000.0, 65_535.0]),
            mode,
            quality,
            &[],
            &mut cache,
        )
        .a_v()
        .value();
    let height = reach(half, 60_000.0, &sun, &mut cache).value();
    assert!(
        (100.0..2_000.0).contains(&height),
        "half the polar dust by {height} ly"
    );
    let edge = GalacticPosition::new(LyCell::new([0, 26_000, i32::MAX - 10]), [0.0; 3])
        .expect("a canonical offset");
    assert_eq!(reach(5.0, 100.0, &edge, &mut cache), LightYears::new(100.0));
}

/// A site's sound speed is `√(γ P ÷ ρ)` with `γ = 5 ÷ 3` and `ρ = 1.4 m_H n`: some 10 km/s in the
/// plane's warm and cold gas and some 100 km/s in the corona. Its isothermal sound speed, which a
/// supernova shell merges against (ruling 98), is `√(P ÷ ρ)`, `√(3 ÷ 5)` of it.
#[test]
fn a_sites_sound_speed_is_the_adiabatic_one() {
    use hyperion_sim::units::consts::{BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG};
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(18));
    let mut cache = NoiseCache::with_capacity(256);
    for (point, lo, hi) in [
        ([0.0, 26_000.0, 0.0], 1e3, 1e5),
        ([0.0, 26_000.0, 40_000.0], 3e4, 3e5),
    ] {
        let state = gas.state(&at(point), SmoothingScale::Full, &mut cache);
        let (n, p) = (state.density().value(), state.pressure().value());
        let expected = (5.0 / 3.0 * p * BOLTZMANN_CONSTANT / (1.4 * HYDROGEN_MASS_KG * n)).sqrt();
        let speed = state.thermal_sound_speed().value();
        assert_relative("the sound speed", speed, expected, 1e-12);
        let isothermal = state.isothermal_sound_speed().value();
        assert_relative(
            "the isothermal",
            isothermal,
            expected * 0.6_f64.sqrt(),
            1e-12,
        );
        assert!((lo..hi).contains(&speed), "{speed} m/s at {point:?}");
    }
}

/// The order a source lists its clouds in is immaterial to a line of sight, bit for bit.
#[test]
fn a_sightline_does_not_depend_on_the_order_of_its_clouds() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(19));
    let mut cache = NoiseCache::with_capacity(4_096);
    let cloud = |x: f64, core: f64| GasModifier::Cloud {
        centre: at([x, 26_000.0, 3.0]),
        core_radius: LightYears::new(core),
        central_density: HydrogenPerCm3::new(300.0),
        dust_per_hydrogen: 1.1,
    };
    let clouds = [
        cloud(-400.0, 3.0),
        cloud(250.0, 9.0),
        cloud(10.0, 2.0),
        cloud(250.0, 4.0),
    ];
    let mut reversed = clouds;
    reversed.reverse();
    let (a, b) = (at([-1_500.0, 26_000.0, 0.0]), at([1_500.0, 26_000.0, 0.0]));
    let one = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Realised,
        Quality::Full,
        &clouds,
        &mut cache,
    );
    let two = sightline(
        &gas,
        &a,
        &b,
        NoiseMode::Realised,
        Quality::Full,
        &reversed,
        &mut cache,
    );
    assert_eq!(one, two);
    let back = sightline(
        &gas,
        &b,
        &a,
        NoiseMode::Realised,
        Quality::Full,
        &reversed,
        &mut cache,
    );
    assert_eq!(one, back);
}

/// P07.T8's integral, bit for bit: eight lines through the fixture's gas — in the plane, to the
/// centre, out of the disc, through a hole and a cloud — in both modes, at full quality and at 64
/// steps, and the visual and K horizons from the Sun. P07.T12's `sightlines.golden` is its own.
#[test]
fn gas_extinction_is_pinned() {
    let fields = milky_way_fields();
    let gas = milky_way_gas(&fields, Seed::new(1));
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let hole = GasModifier::Hole {
        centre: at([0.0, 26_000.0, 50.0]),
        radius: LightYears::new(200.0),
        interior: HydrogenPerCm3::new(0.005),
    };
    let cloud = GasModifier::Cloud {
        centre: at([700.0, 26_010.0, -5.0]),
        core_radius: LightYears::new(6.0),
        central_density: HydrogenPerCm3::new(800.0),
        dust_per_hydrogen: 1.0,
    };
    let lines: [([f64; 3], [f64; 3], &[GasModifier]); 8] = [
        ([-1_500.0, 26_000.0, 0.0], [1_500.0, 26_000.0, 0.0], &[]),
        ([26_000.0, 0.0, 0.0], [0.0, 0.0, 0.0], &[]),
        ([0.0, 26_000.0, 0.0], [0.0, 26_000.0, 65_535.0], &[]),
        (
            [-9_000.0, -20_000.0, 300.0],
            [12_000.0, 5_000.0, -800.0],
            &[],
        ),
        ([17_300.0, 19_800.0, 5.0], [-17_600.0, -20_100.0, -5.0], &[]),
        (
            [-1_500.0, 26_000.0, 0.0],
            [1_500.0, 26_000.0, 0.0],
            &[hole, cloud],
        ),
        ([0.0, 0.0, 30_000.0], [40_000.0, 40_000.0, 50_000.0], &[]),
        (
            [63_000.0, -64_000.0, 20.0],
            [-60_000.0, 62_000.0, -20.0],
            &[],
        ),
    ];
    let sixty_four = Quality::Budget(NonZeroU32::new(64).expect("sixty-four"));
    for (i, (a, b, mods)) in lines.iter().enumerate() {
        for (mode_name, mode) in [("mean", NoiseMode::Mean), ("realised", NoiseMode::Realised)] {
            for (quality_name, quality) in [("full", Quality::Full), ("budget64", sixty_four)] {
                let line = sightline(&gas, &at(*a), &at(*b), mode, quality, mods, &mut cache);
                let label = |name: &str| format!("line{i}.{mode_name}.{quality_name}.{name}");
                w.f64(&label("a_v"), line.a_v().value());
                w.f64(&label("hydrogen_column"), line.hydrogen_column().value());
                w.f64(
                    &label("neutral_column"),
                    line.neutral_hydrogen_column().value(),
                );
                w.line(&format!("{} = {}", label("steps"), line.steps()));
            }
        }
    }
    let sun = at([0.0, 26_000.0, 0.0]);
    for band in [Band::V, Band::K] {
        for mode in [NoiseMode::Mean, NoiseMode::Realised] {
            let reach = horizon(
                &gas,
                &sun,
                -UnitVector::Y,
                band,
                Magnitudes::new(5.0),
                LightYears::new(60_000.0),
                mode,
                Quality::Full,
                &mut cache,
            );
            w.f64(&format!("horizon.{}.{mode:?}", band.name()), reach.value());
        }
    }
    golden!("gas/extinction", w.as_str());
}
