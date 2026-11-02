//! P14.T24.h.b's parts at the area mix, each behind its own hold at 26 (decision-p14-t51-batch
//! §9.4): a minor condensable's own cloud cover, the organic sediment under a hazy air, the Rayleigh
//! layer added over the clear sky, and the haze as an absorbing layer with its anti-greenhouse.

use hyperion_testkit::float::assert_same_bits;

use super::tests::{air, id, partitioned, weights_of};
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

/// P14.T24.h.b's first part is held until the 26 → 27 bump: the rules in force are version 26's and
/// before, any condensable's cloud at water's share, and 27's helper reaches its own.
#[test]
fn the_minor_clouds_are_held_only_until_the_27_bump() {
    use crate::planetary::testing::with_every_27_hold;
    assert!(
        GENERATOR_VERSION < MINOR_CLOUDS_VERSION,
        "the 26 → 27 bump puts P14.T24.h.b's minor clouds on the record (decision-p14-t51-batch \
         §9.4): delete `MINOR_CLOUDS_VERSION`, `MinorCloudsRules` with its `WaterShare` arm and the \
         guard that reads it in `condensable_cloud_share`, the tests' `with_minor_clouds`, its \
         switch in `with_every_27_hold` and in `with_scattering_held` with its callers (and the \
         helpers, if no other 27 hold is left), the `WaterShare` arms of `scattering_tests`, and \
         this test"
    );
    assert_eq!(MinorCloudsRules::in_force(), MinorCloudsRules::WaterShare);
    assert_eq!(
        with_every_27_hold(MinorCloudsRules::in_force),
        MinorCloudsRules::OwnShare
    );
    assert_eq!(
        with_every_27_hold(|| with_minor_clouds(
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

/// P14.T24.h.b's second part is held until the 26 → 27 bump: the rules in force are version 26's
/// and before, no sediment, and 27's helper reaches it.
#[test]
fn the_organic_sediment_is_held_only_until_the_27_bump() {
    use crate::planetary::testing::with_every_27_hold;
    assert!(
        GENERATOR_VERSION < ORGANIC_SEDIMENT_VERSION,
        "the 26 → 27 bump puts P14.T24.h.b's organic sediment on the record \
         (decision-p14-t51-batch §9.4): delete `ORGANIC_SEDIMENT_VERSION`, `OrganicSedimentRules` \
         with its `Bare` arm and the guards that read it in `sediment_share` and \
         `surface_mix::pass_haze`, the tests' `with_organic_sediment`, its switch in \
         `with_every_27_hold` and in `with_scattering_held` with its callers (and the helpers, if \
         no other 27 hold is left), the `Bare` arms of `scattering_tests`, and this test"
    );
    assert_eq!(OrganicSedimentRules::in_force(), OrganicSedimentRules::Bare);
    assert_eq!(
        with_every_27_hold(OrganicSedimentRules::in_force),
        OrganicSedimentRules::Sediment
    );
    assert_eq!(
        with_every_27_hold(|| with_organic_sediment(
            OrganicSedimentRules::Bare,
            OrganicSedimentRules::in_force
        )),
        OrganicSedimentRules::Bare
    );
    assert_eq!(OrganicSedimentRules::in_force(), OrganicSedimentRules::Bare);
}

/// Partial pressures of `fractions` by mole at a surface pressure `pressure`, Pa.
fn partials_of(pressure: f64, fractions: &[(Gas, f64)]) -> PartialPressures {
    let pressures: Vec<(Gas, f64)> = fractions
        .iter()
        .map(|&(gas, x)| (gas, x * pressure))
        .collect();
    air(&pressures)
}

/// The ground's Bond albedo A beneath a conservative layer of reflectance `layer` whose total is
/// `total` ([`rayleigh_added`] inverted): A = (Rₜ − R) ÷ [(1 − R)² + R (Rₜ − R)].
fn beneath_rayleigh(total: f64, layer: f64) -> f64 {
    let through = 1.0 - layer;
    let above = total - layer;
    above / (through * through + layer * above)
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

/// P14.T24.h.b's fourth part is held until the 26 → 27 bump: the rules in force are version 26's
/// and before, the floor's max, and 27's helper reaches the adding rule.
#[test]
fn the_rayleigh_adding_is_held_only_until_the_27_bump() {
    use crate::planetary::testing::with_every_27_hold;
    assert!(
        GENERATOR_VERSION < RAYLEIGH_ADDING_VERSION,
        "the 26 → 27 bump puts P14.T24.h.b's adding rule on the record (decision-p14-t51-batch \
         §9.4): delete `RAYLEIGH_ADDING_VERSION`, `RayleighAddingRules` with its `Max` arm and the \
         guards that read it in `water_cloud_albedo` and `surface_mix::floored`, the tests' \
         `with_rayleigh_adding`, its switch in `with_every_27_hold` and in `with_scattering_held` \
         with its callers (and the helpers, if no other 27 hold is left), the `Max` arms of \
         `scattering_tests`, and this test"
    );
    assert_eq!(RayleighAddingRules::in_force(), RayleighAddingRules::Max);
    assert_eq!(
        with_every_27_hold(RayleighAddingRules::in_force),
        RayleighAddingRules::Added
    );
    assert_eq!(
        with_every_27_hold(|| with_rayleigh_adding(
            RayleighAddingRules::Max,
            RayleighAddingRules::in_force
        )),
        RayleighAddingRules::Max
    );
    assert_eq!(RayleighAddingRules::in_force(), RayleighAddingRules::Max);
}

/// The adding rule is its closed form: the ground's light passed through the layer both ways and
/// reflected between them, summed bounce by bounce, R + (1 − R)² A Σ (A R)ⁿ, to 10⁻¹²; R over
/// black, 1 over a white ground, the ground's own under no layer, at least either; and its inverse
/// gives the ground back. A cloud-free 10-bar N₂ world at 1 g about a Sun, over a ground of 0.2,
/// exceeds its ground by its layer (stated).
#[test]
fn the_adding_rule_takes_its_closed_form() {
    for layer in [0.0, 0.058, 0.26, 0.57, 0.9] {
        for ground in [0.0, 0.05, 0.3, 0.83, 1.0] {
            let (mut series, mut bounce) = (layer, (1.0 - layer) * (1.0 - layer) * ground);
            for _ in 0..2_000 {
                series += bounce;
                bounce *= ground * layer;
            }
            let added = rayleigh_added(layer, ground);
            assert!(
                (added - series).abs() < 1e-12,
                "{layer} over {ground}: {added}"
            );
            assert!(
                added >= layer && added >= ground - 1e-15,
                "{layer} over {ground}"
            );
            if ground < 1.0 && layer < 1.0 {
                let back = beneath_rayleigh(added, layer);
                assert!(
                    (back - ground).abs() < 1e-12,
                    "{layer} over {ground}: {back}"
                );
            }
        }
        assert_same_bits(rayleigh_added(layer, 0.0), layer);
        assert!((rayleigh_added(layer, 1.0) - 1.0).abs() < 1e-15);
    }
    assert_same_bits(rayleigh_added(0.0, 0.37), 0.37);
    let sun = HostShade::sun();
    let nitrogen = partials_of(1e6, &[(Gas::Nitrogen, 1.0)]);
    let layer = rayleigh_floor(&nitrogen, MetresPerSecondSquared::new(9.82), &sun);
    let ground = 0.2;
    eprintln!(
        "a cloud-free 10-bar N₂ world over a ground of {ground}: its layer {layer:.4}, its Bond \
         {:.4} by the adding rule, {:.4} by the floor's max",
        rayleigh_added(layer, ground),
        if layer > ground { layer } else { ground }
    );
    assert!(rayleigh_added(layer, ground) > layer.max(ground));
}

/// Under the adding rule the reference Earth keeps NASA's 0.294 to 10⁻¹², its own Rayleigh layer
/// (about 0.058) laid over its clear sky bin by bin under its cloud, the cloud re-split to do so;
/// under the max its cloud is T51.b's calibration to the bit. Both clouds are stated.
#[test]
fn under_the_adding_rule_the_reference_earth_keeps_0_294() {
    let held = water_cloud_albedo();
    let clear = reference_earth_clear_sky();
    let cloud = SurfaceState::Temperate.cloud_fraction();
    let earth = SurfaceState::Temperate
        .albedo(SurfaceMaterial::Rock)
        .value();
    assert!((under_cloud(clear, cloud, held) - earth).abs() < 1e-12);
    let (split, layer) = with_rayleigh_adding(RayleighAddingRules::Added, || {
        (water_cloud_albedo(), reference_earth_rayleigh())
    });
    let layers = rayleigh_bins(&reference_earth_air(), EARTH_SURFACE_GRAVITY).unwrap();
    let by_bin = layers.map(|r| under_cloud(rayleigh_added(r, clear), cloud, split));
    let total = band_weighted(&by_bin, &sun_bands());
    assert!(
        (rayleigh_added(layer, clear) - reference_earth_clear_sky_added()).abs() < 2e-3,
        "the bolometric rule and the bins' agree within 2 × 10⁻³ at Earth's thin layer"
    );
    eprintln!(
        "the reference Earth: its layer {layer:.4}, its cloud {held:.5} → {split:.5} re-split, its \
         Bond {total:.6}"
    );
    assert!((0.03..0.07).contains(&layer), "{layer}");
    assert!((total - earth).abs() < 1e-12, "{total}");
    assert!(split < held);
    let shade = with_rayleigh_adding(RayleighAddingRules::Added, HostShade::sun);
    assert_same_bits(shade.water_cloud, split);
    assert_same_bits(HostShade::sun().water_cloud, held);
}

/// Under the adding rule the Solar System table's airless Moon, Mercury and Ceres keep their Bond
/// albedos to the bit and Venus its deck's 0.76; its Earth and its Mars move by their layers, each
/// stated both ways and pinned as they stand. The plan's "the Moon and Mars move by under 0.005"
/// is the measured Mars's, 6 mbar: the table's Mars at the passes' stand-ins holds some 0.22 bar,
/// its layer about 0.056 (a deferred finding).
#[test]
fn under_the_adding_rule_the_table_s_airless_bodies_and_venus_keep_their_bonds() {
    use crate::planetary::derive::solar::historic_table;
    let held = historic_table();
    let added = with_rayleigh_adding(RayleighAddingRules::Added, historic_table);
    let mut seen = Vec::new();
    for ((name, before), (_, after)) in held.iter().zip(&added) {
        let (a, b) = (before.albedo().value(), after.albedo().value());
        eprintln!("{name}: Bond {a:.4} → {b:.4} under the adding rule");
        match *name {
            "Moon" | "Mercury" | "Ceres" | "Venus" => assert_same_bits(b, a),
            "Mars" => assert!((0.01..0.03).contains(&(b - a)), "{name}: {a} → {b}"),
            "Earth" => assert!((b - a).abs() < 0.005, "{name}: {a} → {b}"),
            _ => continue,
        }
        seen.push(*name);
    }
    seen.sort_unstable();
    assert_eq!(seen, ["Ceres", "Earth", "Mars", "Mercury", "Moon", "Venus"]);
}

/// P14.T24.h.b's third part is held until the 26 → 27 bump: the rules in force are version 26's
/// and before, no haze layer, and 27's helper reaches it.
#[test]
fn the_haze_layer_is_held_only_until_the_27_bump() {
    use crate::planetary::testing::with_every_27_hold;
    assert!(
        GENERATOR_VERSION < HAZE_LAYER_VERSION,
        "the 26 → 27 bump puts P14.T24.h.b's haze layer on the record (decision-p14-t51-batch \
         §9.4): delete `HAZE_LAYER_VERSION`, `HazeLayerRules` with its `Clear` arm and the guards \
         that read it in `hazed`, `hazed_bins`, `anti_greenhouse_share`, `step_anti_greenhouse`, \
         `surface_mix::pass_haze` and `escaping`'s solve, the tests' `with_haze_layer`, its switch \
         in `with_every_27_hold` and in `with_scattering_held` with its callers (and the helpers, \
         if no other 27 hold is left), the `Clear` arms of `scattering_tests`, and this test"
    );
    assert_eq!(HazeLayerRules::in_force(), HazeLayerRules::Clear);
    assert_eq!(
        with_every_27_hold(HazeLayerRules::in_force),
        HazeLayerRules::Absorbing
    );
    assert_eq!(
        with_every_27_hold(|| with_haze_layer(HazeLayerRules::Clear, HazeLayerRules::in_force)),
        HazeLayerRules::Clear
    );
    assert_eq!(HazeLayerRules::in_force(), HazeLayerRules::Clear);
}

/// The δ-Eddington layer: conservative and isotropic it is the Eddington closure's diffuse R =
/// ¾τ ÷ (1 + ¾τ) with T = 1 − R; an absorbing one keeps R + T under 1 and both in [0, 1],
/// darkening as ω falls; none at τ 0; and the aggregate's optics interpolate the optool nodes,
/// held beyond them.
#[test]
fn the_haze_layer_takes_its_closed_form() {
    for tau in [0.0, 0.1, 1.0, 8.1, 40.0] {
        let (r, t) = two_stream_layer(tau, 1.0, 0.0);
        let by_hand = 0.75 * tau / (1.0 + 0.75 * tau);
        assert!(
            (r - by_hand).abs() < 1e-12 && (r + t - 1.0).abs() < 1e-12,
            "{tau}"
        );
        let mut last = r;
        for omega in [0.99, 0.95, 0.8, 0.5] {
            let (r, t) = two_stream_layer(tau, omega, 0.81);
            assert!(
                (0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&t),
                "{tau} {omega}"
            );
            assert!(r + t <= 1.0 + 1e-12, "{tau} {omega}: {r} + {t}");
            assert!(r <= last + 1e-12, "{tau} {omega}: {r} over {last}");
            last = r;
        }
    }
    let (r, t) = two_stream_layer(0.0, 0.9, 0.8);
    assert!(r.abs() < 1e-15 && (t - 1.0).abs() < 1e-15);
    assert_same_bits(haze_aggregate_at(550.0).0, 15.8321 / 16.5393);
    assert_same_bits(haze_aggregate_at(300.0).0, haze_aggregate_at(380.0).0);
    assert_same_bits(haze_aggregate_at(4_000.0).1, haze_aggregate_at(780.0).1);
    let (omega, _) = haze_aggregate_at(495.0);
    assert!(omega > haze_aggregate_at(440.0).0 && omega < haze_aggregate_at(550.0).0);
    let (upper, lower) = haze_spectral_shape(550.0);
    assert!((upper + lower - 1.0).abs() < 1e-15);
    assert!(
        (upper - 0.83).abs() < 0.01,
        "the haze above 30 km at 550 nm: {upper}"
    );
    let whole = |nm: f64| haze_spectral_shape(nm).0 + haze_spectral_shape(nm).1;
    assert!(whole(400.0) > 1.0 && whole(1_000.0) < 1.0);
}

/// Titan's haze about a Sun: over its dark sediment under its Rayleigh layer, bin by bin, the haze
/// layer's Bond albedo and the share of the light its upper haze absorbs, stated; the
/// anti-greenhouse's surface (1 − γ) `T_grey`⁴ + (γ ÷ 2) `T_base`⁴ by hand, its γ = 1 limit `T_base` ÷
/// 2^¼ and `grey` to the bit at γ 0; and under `Clear`, with no haze or no light, the disc as it is
/// to the bit and γ 0.
#[test]
fn titan_s_haze_absorbs_and_cools_its_surface() {
    use crate::planetary::hooks::aerosols::TITAN_HAZE_OPTICAL_DEPTH;
    let sun = HostShade::sun();
    let haze = TITAN_HAZE_OPTICAL_DEPTH;
    let titan = partials_of(
        TITAN_SURFACE_PRESSURE.value(),
        &[(Gas::Nitrogen, 0.9435), (Gas::Methane, 0.0565)],
    );
    let ground = solid_bond(id("tholin"), &sun.bands).bond;
    let below = rayleigh_bins(&titan, TITAN_GRAVITY)
        .unwrap()
        .map(|layer| rayleigh_added(layer, ground));
    let unhazed = band_weighted(&below, &sun.bands);
    assert_same_bits(hazed_bins(&below, haze, &sun), unhazed);
    assert_same_bits(hazed(0.3, haze, &sun), 0.3);
    assert_same_bits(anti_greenhouse_share(0.3, 0.7), 0.0);
    let (grey, base) = (Kelvin::new(98.5), Kelvin::new(81.0));
    assert_same_bits(anti_greenhouse(grey, base, 0.0).value(), grey.value());
    with_haze_layer(HazeLayerRules::Absorbing, || {
        let total = hazed_bins(&below, haze, &sun);
        let absorbed = haze_absorbed(haze, &sun);
        let gamma = anti_greenhouse_share(absorbed, 1.0 - total);
        let cooled = anti_greenhouse(grey, base, gamma).value();
        eprintln!(
            "Titan's haze over its ground and air ({unhazed:.4}): Bond {total:.4}, its upper haze \
             absorbing {absorbed:.4} over black, γ {gamma:.3}; a grey {:.1} K over a base {:.1} K \
             cools to {cooled:.1} K",
            grey.value(),
            base.value()
        );
        assert!((0.1..0.6).contains(&absorbed), "{absorbed}");
        assert!((gamma - absorbed / (1.0 - total)).abs() < 1e-15);
        let by_hand = math::powf(
            (1.0 - gamma) * math::powf(grey.value(), 4.0)
                + gamma / 2.0 * math::powf(base.value(), 4.0),
            0.25,
        );
        assert!(
            (cooled - by_hand).abs() < 1e-9,
            "{cooled} against {by_hand}"
        );
        assert!(cooled < grey.value());
        let limit = anti_greenhouse(grey, base, 1.0).value();
        assert!(
            (limit - base.value() / math::powf(2.0, 0.25)).abs() < 1e-9,
            "{limit}"
        );
        assert_same_bits(hazed_bins(&below, 0.0, &sun), unhazed);
        assert_same_bits(anti_greenhouse_share(0.0, 0.7), 0.0);
        assert_same_bits(anti_greenhouse_share(0.3, 0.0), 0.0);
        assert_same_bits(anti_greenhouse_share(0.9, 0.5), 1.0);
    });
}
