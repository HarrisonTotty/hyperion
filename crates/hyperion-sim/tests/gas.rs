//! The gas and dust field's fast integration tests (plan 07).
//!
//! The field's own arithmetic is unit-tested inside each `galaxy::gas` module, where the smooth
//! components and the normalisations are reachable; what belongs here is what only the public API
//! can see, and the golden files that pin the generator's output. P07.T12's slow statistical tests
//! live in `gas_statistics.rs`.

use hyperion_sim::galaxy::gas::params::GasParams;
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The three seeds the parameter golden pins, the same ones plan 02's own parameter golden uses so
/// that the two files can be read side by side.
const PINNED_SEEDS: [u64; 3] = [
    0x0000_0000_0000_0001,
    0x5eed_0000_c0ff_ee00,
    0xdead_beef_cafe_f00d,
];

/// Every getter of `gas`, labelled `<prefix>.<name>`, in Design note 3's table order: plan 02's
/// three rows first, then the eleven drawn here and the two constants.
fn write_gas_params(w: &mut GoldenWriter, prefix: &str, gas: &GasParams) {
    let mut f = |name: &str, value: f64| w.f64(&format!("{prefix}.{name}"), value);
    f("mass", gas.gas_mass().value());
    f("radial_scale", gas.radial_scale().value());
    f("neutral_height", gas.neutral_height().value());
    f("hole_scale", gas.hole_scale().value());
    f("neutral_fraction", gas.neutral_fraction());
    f("warm_fraction", gas.warm_fraction());
    f("warm_height", gas.warm_height().value());
    let molecular = gas.molecular_disc();
    f("molecular_fraction", molecular.fraction());
    f("molecular_mass", molecular.mass().value());
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
            &GasParams::from_galaxy(seed, &galaxy),
        );
    }
    golden!("gas/params", w.as_str());
}
