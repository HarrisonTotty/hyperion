//! The gas and dust field's slow statistical tests (plan 07), run by `just test-slow`.
//!
//! Each builds thousands of galaxies, which even optimised cost some 17 ms apiece for plan 02's σ
//! estimator, so they are marked slow and run under the slow-test profile only; the fast suite runs
//! the same checks on fewer seeds inside `galaxy::gas`'s own unit tests.

use hyperion_sim::Seed;
use hyperion_sim::galaxy::gas::params::GasParams;
use hyperion_sim::galaxy::gas::smooth::{GasLayer, SmoothGas};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;

/// The least share of the gas the neutral disc may hold: half, the ruling's "a neutral layer that is
/// not most of the gas is not this galaxy" (plan 07, ruling 19 of 2026-09-22). The Milky Way's local
/// column is 87% atomic or molecular (McKee, Parravano and Hollenbach 2015, ApJ 814, 13, Table 2:
/// 11.9 of 13.7 M☉ pc⁻²) and the fixture's disc 85%. A seed below it is a finding to report, never a
/// share to clamp.
const NEUTRAL_SHARE_FLOOR: f64 = 0.5;

/// The neutral disc keeps most of every galaxy's gas once the warm ionised layer is drawn by its own
/// density at the Sun's radius and the molecular disc by its own mass (ruling 19), over 2,000 seeds
/// each drawn against its own galaxy — so the spread of plan 02's gas masses and scale lengths is in
/// the sweep, which is what moves the share. The warm layer holds its drawn density at the Sun's
/// radius for every seed.
///
/// The distribution of the share is printed, since the lightest galaxies set its floor: their warm
/// layer weighs what a Milky Way's does, some 10⁹ M☉, against a gas disc of a few 10⁹.
#[test]
#[ignore = "slow: 2,000 galaxies' parameters, about 15 s optimised"]
fn the_neutral_disc_holds_most_of_the_gas_over_2000_galaxies() {
    let reference = GasParams::REFERENCE_RADIUS.value();
    let mut shares = Vec::with_capacity(2_000);
    for n in 0..2_000_u64 {
        let seed = Seed::new(0x0700_5eed_0000_0000 | n);
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let params = GasParams::from_galaxy(seed, &galaxy);
        let share = params.neutral_fraction();
        assert!(
            share >= NEUTRAL_SHARE_FLOOR,
            "{seed}: the neutral disc holds {share} of {:e} M☉ of gas, the warm layer {:e} M☉",
            params.gas_mass().value(),
            params.warm_mass().value(),
        );
        let warm = SmoothGas::new(&params).plane_density(GasLayer::Warm, reference);
        let drawn = params.warm_density().value();
        assert!(
            ((warm - drawn) / drawn).abs() < 1e-14,
            "{seed}: the warm layer reads {warm} at the Sun against its drawn {drawn}"
        );
        shares.push(share);
    }
    shares.sort_by(f64::total_cmp);
    let at = |percent: usize| shares[percent * (shares.len() - 1) / 100];
    eprintln!(
        "neutral share over 2,000 galaxies: least {:.3}, 1% {:.3}, 16% {:.3}, median {:.3}, \
         84% {:.3}, most {:.3}",
        shares[0],
        at(1),
        at(16),
        at(50),
        at(84),
        shares[shares.len() - 1],
    );
}
