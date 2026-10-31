//! P14.T24.h.b's parts at the area mix, each behind its own hold at 26: a minor condensable's own
//! cloud cover (decision-p14-t51-batch §9.4).

use hyperion_testkit::float::assert_same_bits;

use super::tests::{id, partitioned, weights_of};
use super::*;

/// A Titan-like partition: 1.5 bar of N₂ over a methane inventory its air cannot hold at 90–95 K,
/// its excess a methane reservoir, at Titan's 1.352 m s⁻².
fn titan_like() -> Partition {
    partitioned(
        (90.0, 93.7, 95.0),
        weights_of(1.8e5, &[("N2", 0.83), ("CH4", 0.17)]),
        1.352,
    )
}

/// An Earth-like partition: its water's column well above the reference's, at 1 g.
fn earth_like() -> Partition {
    let mut wet = weights_of(1.0e5, &[("N2", 0.78), ("O2", 0.21), ("Ar", 0.01)]);
    wet.push((id("H2O"), Pascals::new(2.0e5)));
    partitioned((261.0, 288.0, 305.0), wet, 9.82)
}

/// P14.T24.h.b's first part is held until the 25 → 26 bump: the rules in force are version 25's and
/// before, any condensable's cloud at water's share, and 26's helper reaches its own.
#[test]
fn the_minor_clouds_are_held_only_until_the_26_bump() {
    use crate::planetary::testing::with_every_26_hold;
    assert!(
        GENERATOR_VERSION < MINOR_CLOUDS_VERSION,
        "the 25 → 26 bump puts P14.T24.h.b's minor clouds on the record (decision-p14-t51-batch \
         §9.4): delete `MINOR_CLOUDS_VERSION`, `MinorCloudsRules` with its `WaterShare` arm and the \
         guard that reads it in `condensable_cloud_share`, the tests' `with_minor_clouds`, its \
         switch in `with_every_26_hold` (and the helper, if no other 26 hold is left), the \
         `WaterShare` arms of `scattering_tests`, and this test"
    );
    assert_eq!(MinorCloudsRules::in_force(), MinorCloudsRules::WaterShare);
    assert_eq!(
        with_every_26_hold(MinorCloudsRules::in_force),
        MinorCloudsRules::OwnShare
    );
    assert_eq!(
        with_every_26_hold(|| with_minor_clouds(
            MinorCloudsRules::WaterShare,
            MinorCloudsRules::in_force
        )),
        MinorCloudsRules::WaterShare
    );
    assert_eq!(MinorCloudsRules::in_force(), MinorCloudsRules::WaterShare);
}

/// Under its own share a Titan-like air's methane cloud covers Titan's 0.2–0.6% (Bouchez and Brown
/// 2005), its condensable's availability whole, where water's share gave it 0.67; a water cloud,
/// an Earth's, keeps 0.67 to the bit under both, and a runaway solved without its deck the same.
#[test]
fn a_minor_condensable_s_cloud_covers_titan_s_share() {
    let sun = HostShade::sun();
    let titan = titan_like();
    let (condensable, available) = cloud_condensable(&titan, &sun).expect("its methane condenses");
    assert_eq!(condensable, id("CH4"));
    let snowball = (SurfaceState::Snowball, RunawayDeck::Kept);
    let water_share = cloud_fraction(snowball, &titan, &sun);
    let own = with_minor_clouds(MinorCloudsRules::OwnShare, || {
        cloud_fraction(snowball, &titan, &sun)
    });
    eprintln!(
        "a Titan-like air: methane available {available:.4}; its cloud {water_share:.4} at water's \
         share, {own:.5} at its own"
    );
    assert_same_bits(
        water_share,
        SurfaceState::Temperate.cloud_fraction() * available,
    );
    assert_same_bits(own, MINOR_CONDENSABLE_CLOUD_SHARE * available);
    assert!((0.002..=0.006).contains(&own), "{own}");
    let earth = earth_like();
    for state in [
        (SurfaceState::Temperate, RunawayDeck::Kept),
        (SurfaceState::Snowball, RunawayDeck::Kept),
        (SurfaceState::RunawayGreenhouse, RunawayDeck::Deckless),
    ] {
        let held = cloud_fraction(state, &earth, &sun);
        let mine = with_minor_clouds(MinorCloudsRules::OwnShare, || {
            cloud_fraction(state, &earth, &sun)
        });
        assert_same_bits(mine, held);
    }
    assert_same_bits(
        cloud_fraction((SurfaceState::Temperate, RunawayDeck::Kept), &earth, &sun),
        SurfaceState::Temperate.cloud_fraction(),
    );
}
