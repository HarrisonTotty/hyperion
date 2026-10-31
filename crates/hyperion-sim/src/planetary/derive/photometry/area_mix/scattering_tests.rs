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

/// P14.T24.h.b's second part is held until the 25 → 26 bump: the rules in force are version 25's
/// and before, no sediment, and 26's helper reaches it.
#[test]
fn the_organic_sediment_is_held_only_until_the_26_bump() {
    use crate::planetary::testing::with_every_26_hold;
    assert!(
        GENERATOR_VERSION < ORGANIC_SEDIMENT_VERSION,
        "the 25 → 26 bump puts P14.T24.h.b's organic sediment on the record \
         (decision-p14-t51-batch §9.4): delete `ORGANIC_SEDIMENT_VERSION`, `OrganicSedimentRules` \
         with its `Bare` arm and the guard that reads it in `sediment_share`, the tests' \
         `with_organic_sediment`, its switch in `with_every_26_hold` (and the helper, if no other \
         26 hold is left), the `Bare` arms of `scattering_tests`, and this test"
    );
    assert_eq!(OrganicSedimentRules::in_force(), OrganicSedimentRules::Bare);
    assert_eq!(
        with_every_26_hold(OrganicSedimentRules::in_force),
        OrganicSedimentRules::Sediment
    );
    assert_eq!(
        with_every_26_hold(|| with_organic_sediment(
            OrganicSedimentRules::Bare,
            OrganicSedimentRules::in_force
        )),
        OrganicSedimentRules::Bare
    );
    assert_eq!(OrganicSedimentRules::in_force(), OrganicSedimentRules::Bare);
}

/// Partial pressures of `fractions` by mole at a surface pressure `pressure`, Pa.
fn partials_of(pressure: f64, fractions: &[(Gas, f64)]) -> PartialPressures {
    PartialPressures::from_fn(|gas| {
        Pascals::new(
            fractions
                .iter()
                .find(|&&(g, _)| g == gas)
                .map_or(0.0, |&(_, x)| x * pressure),
        )
    })
}

/// The snapshot path's haze is T24.c's τ(550) at the air's index, scaled by its column over
/// Titan's: Titan's own air about a star gives Titan's 8.1; a Triton-like 1.4 Pa of N₂ with 2 ×
/// 10⁻⁴ of CH₄ some 10⁻⁵ of it; no star, no methane over its floor, or no air, none.
#[test]
fn the_snapshot_haze_is_t24c_s_scaled_by_the_column() {
    use crate::planetary::hooks::aerosols::TITAN_HAZE_OPTICAL_DEPTH;
    let sun = Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K);
    let titan = partials_of(
        TITAN_SURFACE_PRESSURE.value(),
        &[(Gas::Nitrogen, 0.9435), (Gas::Methane, 0.0565)],
    );
    let tau = snapshot_haze_optical_depth(&titan, TITAN_GRAVITY, sun);
    assert!((tau / TITAN_HAZE_OPTICAL_DEPTH - 1.0).abs() < 1e-3, "{tau}");
    let triton = partials_of(1.4, &[(Gas::Nitrogen, 0.9998), (Gas::Methane, 2e-4)]);
    let thin = snapshot_haze_optical_depth(&triton, MetresPerSecondSquared::new(0.779), sun);
    eprintln!("the snapshot haze: Titan {tau:.4}, a Triton-like air {thin:.3e}");
    assert!(thin > 0.0 && thin < 1e-3 * tau, "{thin}");
    assert_same_bits(
        snapshot_haze_optical_depth(&titan, TITAN_GRAVITY, Kelvin::ZERO),
        0.0,
    );
    let clean = partials_of(1e5, &[(Gas::Nitrogen, 1.0)]);
    assert_same_bits(
        snapshot_haze_optical_depth(&clean, MetresPerSecondSquared::new(9.8), sun),
        0.0,
    );
    assert_same_bits(
        snapshot_haze_optical_depth(&partials_of(0.0, &[]), TITAN_GRAVITY, sun),
        0.0,
    );
}

/// The sediment's share is the haze over Titan's under the rule, held at 1, none at no haze and
/// none under `Bare`; and the sedimented surface takes `tholin`'s albedo for its regolith and its
/// listed `H2O` ice at Titan's haze, its seas as they are, and is `surface_bond` to the bit at
/// none.
#[test]
fn titan_s_solid_ground_is_its_haze_s_sediment() {
    use crate::planetary::chem::partition::{Cover, CrustWater, WaterCover};
    use crate::planetary::hooks::aerosols::TITAN_HAZE_OPTICAL_DEPTH;
    assert_same_bits(sediment_share(TITAN_HAZE_OPTICAL_DEPTH), 0.0);
    with_organic_sediment(OrganicSedimentRules::Sediment, || {
        assert_same_bits(sediment_share(TITAN_HAZE_OPTICAL_DEPTH), 1.0);
        assert_same_bits(sediment_share(2.0 * TITAN_HAZE_OPTICAL_DEPTH), 1.0);
        assert!((sediment_share(0.5 * TITAN_HAZE_OPTICAL_DEPTH) - 0.5).abs() < 1e-15);
        assert_same_bits(sediment_share(0.0), 0.0);
        assert_same_bits(sediment_share(f64::NAN), 0.0);
    });
    let sun = HostShade::sun();
    let titan = partitioned(
        (89.7, 93.65, 93.65),
        vec![
            (id("CH4"), Pascals::new(5.5e3)),
            (id("N2"), Pascals::new(1.418e5)),
            (id("H2O"), Pascals::new(1e9)),
        ],
        1.352,
    );
    let profile = |t: Kelvin| ((t.value() - 85.0) / 10.0).clamp(0.0, 1.0);
    let water = WaterCover {
        surface_water: KilogramsPerSquareMetre::ZERO,
        potential_ocean: 0.0,
        ice_fraction: 0.5,
        ice_density: crate::units::KilogramsPerCubicMetre::new(917.0),
        crust_water: CrustWater::Listed,
    };
    let published = titan.publish(&Cover {
        cold_area: &profile,
        state: SurfaceState::Snowball,
        water,
    });
    let ice = published
        .entry(id("H2O"), CondensatePhase::Solid)
        .expect("its listed crust");
    let seas = published
        .entry(id("ch4_c2h6_n2"), CondensatePhase::Liquid)
        .expect("its seas");
    eprintln!(
        "Titan's published ground: ice {:.4}, seas {:.4}",
        ice.area_fraction(),
        seas.area_fraction()
    );
    let regolith = 0.35;
    let haze = TITAN_HAZE_OPTICAL_DEPTH;
    let bare = surface_bond(&published, &titan, (regolith, &[]), &sun);
    for tau in [0.0, haze] {
        assert_same_bits(
            sedimented_surface_bond(&published, &titan, (regolith, &[]), &sun, tau),
            bare,
        );
    }
    let tholin = solid_bond(id("tholin"), &sun.bands).bond;
    let sedimented = with_organic_sediment(OrganicSedimentRules::Sediment, || {
        assert_same_bits(
            sedimented_surface_bond(&published, &titan, (regolith, &[]), &sun, 0.0),
            bare,
        );
        sedimented_surface_bond(&published, &titan, (regolith, &[]), &sun, haze)
    });
    let by_hand = surface_bond_with(&published, &titan, (tholin, &[]), &sun, &|row| {
        if row == id("H2O") {
            tholin
        } else {
            solid_bond(row, &sun.bands).bond
        }
    });
    assert_same_bits(sedimented, by_hand);
    eprintln!(
        "Titan's ground: bare {bare:.4}, under its sediment {sedimented:.4} (tholin {tholin:.4})"
    );
    assert!(sedimented < bare, "{sedimented} against {bare}");
}
