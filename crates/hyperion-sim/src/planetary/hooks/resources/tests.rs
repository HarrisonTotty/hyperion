use super::*;
use crate::id::SystemId;
use crate::planetary::chem::inventory::{ElementInventory, InventoryInputs, element_inventory};
use crate::planetary::chem::solar_magnesium_to_silicon;
use crate::planetary::derive::atmosphere::VolatileDraws;
use crate::planetary::derive::composition;
use crate::units::consts::METRES_PER_AU;
use crate::units::{EarthFluxes, EarthMasses, EarthRadii, Metres};

/// The present Solar System's age, the radioactives' reference.
const NOW: Years = RADIOGENIC_REFERENCE_AGE;

/// A body of mass `mass` and radius `radius` formed on `side` of a snow line at 2.7 au, its
/// mass fractions solved and its element inventory at the median ranks.
fn body(
    mass: f64,
    radius: f64,
    side: SnowLineSide,
) -> (crate::planetary::derive::MassFractions, ElementInventory) {
    let mass = EarthMasses::new(mass);
    let flux = match side {
        SnowLineSide::Inside => EarthFluxes::new(1.0),
        SnowLineSide::Beyond => EarthFluxes::new(0.01),
    };
    let fractions = composition(mass, EarthRadii::new(radius), side, flux)
        .unwrap()
        .fractions();
    let distance = match side {
        SnowLineSide::Inside => 1.0,
        SnowLineSide::Beyond => 10.0,
    };
    let elements = element_inventory(&InventoryInputs {
        mass,
        fractions,
        formed: side,
        formation_distance: Metres::new(distance * METRES_PER_AU),
        snow_line: Metres::new(2.7 * METRES_PER_AU),
        draws: VolatileDraws::MEDIAN,
        age: NOW,
    });
    (fractions, elements)
}

/// An Earth: 1 M⊕ and 1 R⊕, inside the snow line.
fn earth() -> (crate::planetary::derive::MassFractions, ElementInventory) {
    body(1.0, 1.0, SnowLineSide::Inside)
}

/// A Ganymede-like icy body: 0.025 M⊕ of 0.41 R⊕, beyond the snow line.
fn icy() -> (crate::planetary::derive::MassFractions, ElementInventory) {
    body(0.025, 0.41, SnowLineSide::Beyond)
}

/// The bulk of `of` about a host of \[Fe/H\] `fe_h` and \[α/Fe\] `alpha_fe`.
fn bulk_of(
    of: (crate::planetary::derive::MassFractions, ElementInventory),
    fe_h: f64,
    alpha_fe: f64,
) -> BulkComposition {
    BulkComposition::new(of.0, of.1, Dex::new(fe_h), Some(Dex::new(alpha_fe)))
}

/// The abundances of `bulk`, formed on `side`, over `layer` at `age`, at `draws`, about a host of
/// the Sun's Mg/Si and C/O.
fn abundances(
    bulk: &BulkComposition,
    side: SnowLineSide,
    layer: OuterLayer<'_>,
    age: Years,
    draws: &ResourceDraws,
) -> ResourceAbundances {
    let inputs = ResourceInputs {
        bulk,
        formed: side,
        layer,
        age,
        host_magnesium_to_silicon: solar_magnesium_to_silicon(),
        host_carbon_to_oxygen: solar_carbon_to_oxygen(),
    };
    ResourceAbundances::derive(&inputs, draws)
}

/// Both rules of the ratios' hold.
const RULES: [ResourceRatiosRules; 2] = [ResourceRatiosRules::Held, ResourceRatiosRules::Ratios];

/// Jupiter's gas at 1 bar by mole fraction: H₂ 0.862, He 0.1359 (von Zahn, Hunten and Lehmacher
/// 1998, via the life brainstorm's "Gas-giant fuel"), and CH₄ 0.0021 (the rest), with no
/// condensate above its decks.
fn jupiter_gas() -> Vec<(SubstanceId, f64)> {
    vec![
        (Gas::Hydrogen.substance(), 0.862),
        (Gas::Helium.substance(), 0.1359),
        (Gas::Methane.substance(), 0.0021),
    ]
}

/// Seeded draws of a few dozen bodies, and the extreme ranks either side.
fn draws() -> Vec<ResourceDraws> {
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let mut draws: Vec<_> = (0..48_u16)
        .map(|n| ResourceDraws::for_body(Seed::new(26), BodyId::new(system, 0x0100 + n)))
        .collect();
    for rank in [1e-12, 1.0 - 1e-12] {
        draws.push(ResourceDraws {
            ranks: [UnitUniform::new(rank).unwrap(); RESOURCES],
        });
    }
    // One resource up, the rest down, and the reverse, for every resource.
    for resource in Resource::ALL {
        for (one, rest) in [(1.0 - 1e-9, 1e-9), (1e-9, 1.0 - 1e-9)] {
            let mut ranks = [UnitUniform::new(rest).unwrap(); RESOURCES];
            ranks[resource.index()] = UnitUniform::new(one).unwrap();
            draws.push(ResourceDraws { ranks });
        }
    }
    draws
}

/// [P14.T26] Every fraction lies within 0–1 and they sum to at most 1, to rounding, over every
/// layer, metallicities from the halo's to the most metal-rich disc's, both sides of the snow line
/// and seeded and extreme draws, under both rules of the ratios' hold.
#[test]
fn the_fractions_lie_within_zero_and_one_and_sum_to_at_most_one() {
    let gas = jupiter_gas();
    let layers = [
        OuterLayer::Bulk,
        OuterLayer::SilicateCrust,
        OuterLayer::IceShell,
        OuterLayer::Gas(&gas),
    ];
    let mut checked = 0_u32;
    for rules in RULES {
        with_resource_ratios(rules, || {
            for (of, side) in [
                (earth(), SnowLineSide::Inside),
                (icy(), SnowLineSide::Beyond),
            ] {
                for (fe_h, alpha_fe) in [(-2.5, 0.4), (-1.0, 0.3), (0.0, 0.0), (0.5, -0.1)] {
                    let bulk = bulk_of(of, fe_h, alpha_fe);
                    for layer in layers {
                        for age in [Years::new(1e7), NOW, Years::new(12e9)] {
                            for d in &draws() {
                                let a = abundances(&bulk, side, layer, age, d);
                                for (resource, x) in a.iter() {
                                    assert!(
                                        (0.0..=1.0).contains(&x),
                                        "{rules:?}: {resource:?} = {x} on {layer:?}, [Fe/H] {fe_h}"
                                    );
                                }
                                assert!(
                                    a.total() <= 1.0 + 1e-12,
                                    "{rules:?}: {} on {layer:?}",
                                    a.total()
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
        });
    }
    assert!(checked > 2_000, "{checked}");
}

/// [P14.T26] Below 27 (the ratios' hold), at fixed composition and ranks, the iron group scales
/// with 10^\[Fe/H\]: on a differentiated body's crust and an undifferentiated body's bulk, at the
/// median ranks, whose fractions are the mean's, while the metals fill less than the layer's
/// solids (an Earth's undifferentiated bulk, a third iron, reaches them above \[Fe/H\] +0.3 at
/// \[α/Fe\] +0.2). From 27 \[Fe/H\] moves no fraction
/// (`under_27_the_metallicity_moves_no_fraction_at_fixed_ratios`).
#[test]
fn the_iron_group_scales_with_ten_to_the_metallicity_at_fixed_composition() {
    with_resource_ratios(ResourceRatiosRules::Held, || {
        for layer in [OuterLayer::SilicateCrust, OuterLayer::Bulk] {
            let iron = |fe_h: f64| {
                let bulk = bulk_of(earth(), fe_h, 0.0);
                abundances(
                    &bulk,
                    SnowLineSide::Inside,
                    layer,
                    NOW,
                    &ResourceDraws::MEDIAN,
                )
                .fraction(Resource::IronGroup)
            };
            let solar = iron(0.0);
            assert!(solar > 0.01, "{layer:?}: {solar}");
            for fe_h in [-2.0, -1.0, -0.5, 0.3] {
                let ratio = iron(fe_h) / solar;
                let expected = math::exp10(fe_h);
                assert!(
                    (ratio / expected - 1.0).abs() < 1e-12,
                    "{layer:?} at [Fe/H] {fe_h}: {ratio} against {expected}"
                );
            }
        }
    });
}

/// [P14.T26] The radioactives' ²³⁸U share halves over 4.5 Gyr, 2^(−4.5 ÷ 4.47) by its half-life:
/// at the median ranks exactly as its decay, and at seeded ranks within a part in 10⁵, where the
/// odds' denominator and the sum's division move with the radioactives' own tiny share; under both
/// rules of the ratios' hold.
#[test]
fn the_radioactives_uranium_238_share_halves_over_four_and_a_half_gigayears() {
    let bulk = bulk_of(earth(), 0.0, 0.0);
    let expected = math::exp(-4.5e9 / RadiogenicIsotope::Uranium238.mean_life().value());
    assert!((expected - 0.5).abs() < 0.005, "{expected}");
    for rules in RULES {
        with_resource_ratios(rules, || {
            for layer in [OuterLayer::SilicateCrust, OuterLayer::Bulk] {
                for (d, tolerance) in [(ResourceDraws::MEDIAN, 1e-12), (draws()[0], 1e-5)] {
                    let uranium = |age: f64| {
                        abundances(&bulk, SnowLineSide::Inside, layer, Years::new(age), &d)
                            .radioactive(RadiogenicIsotope::Uranium238)
                    };
                    let ratio = uranium(6e9) / uranium(1.5e9);
                    assert!(
                        (ratio / expected - 1.0).abs() < tolerance,
                        "{rules:?} on {layer:?}: {ratio} against {expected}"
                    );
                    assert!(
                        (ratio - 0.5).abs() < 0.005,
                        "{rules:?} on {layer:?}: {ratio}"
                    );
                }
            }
        });
    }
}

/// [P14.T26] Below 27 (the ratios' hold), a metal-poor halo planet, \[Fe/H\] −1.5 and \[α/Fe\]
/// +0.35, is poorer in every metal group than its disc twin of solar abundances, of the same
/// composition and at the same ranks, and richer in silicates: at the median ranks and the seeded
/// ones. The bounds test's constructed ranks are left out: at seven σ every resource's odds
/// saturate, and the division by their sum then evens them out. From 27 it is poorer per kilogram
/// in its non-α metals alone (`under_27_a_halo_twin_s_crust_is_poorer_only_in_its_non_alpha_metals`).
#[test]
fn a_metal_poor_halo_planet_is_poorer_in_every_metal_than_its_disc_twin() {
    with_resource_ratios(ResourceRatiosRules::Held, || {
        let metals = [
            Resource::IronGroup,
            Resource::LightLithophiles,
            Resource::RareEarths,
            Resource::PlatinumGroup,
            Resource::Radioactives,
        ];
        for (of, side) in [
            (earth(), SnowLineSide::Inside),
            (icy(), SnowLineSide::Beyond),
        ] {
            let (halo, disc) = (bulk_of(of, -1.5, 0.35), bulk_of(of, 0.0, 0.0));
            for layer in [OuterLayer::SilicateCrust, OuterLayer::Bulk] {
                for d in draws().iter().take(48).chain([&ResourceDraws::MEDIAN]) {
                    let poor = abundances(&halo, side, layer, NOW, d);
                    let rich = abundances(&disc, side, layer, NOW, d);
                    for metal in metals {
                        assert!(
                            poor.fraction(metal) < rich.fraction(metal),
                            "{metal:?} on {layer:?}: {} against {}",
                            poor.fraction(metal),
                            rich.fraction(metal)
                        );
                    }
                    assert!(
                        poor.fraction(Resource::Silicates) > rich.fraction(Resource::Silicates)
                    );
                }
            }
        }
    });
}

/// Differentiation takes the platinum group into the core: an undifferentiated body's bulk holds
/// over a hundred times its differentiated twin's crust, at a chondrite's parts per million.
#[test]
fn the_platinum_group_is_rich_in_a_bulk_and_poor_in_a_crust() {
    let bulk = bulk_of(earth(), 0.0, 0.0);
    let platinum = |layer| {
        abundances(
            &bulk,
            SnowLineSide::Inside,
            layer,
            NOW,
            &ResourceDraws::MEDIAN,
        )
        .fraction(Resource::PlatinumGroup)
    };
    let (undifferentiated, crust) = (
        platinum(OuterLayer::Bulk),
        platinum(OuterLayer::SilicateCrust),
    );
    assert!(
        (1e-6..1e-5).contains(&undifferentiated),
        "{undifferentiated}"
    );
    assert!((1e-8..1e-7).contains(&crust), "{crust}");
    assert!(undifferentiated > 100.0 * crust);
}

/// Below 27 (the ratios' hold), a crust's composition at solar abundances is the bulk silicate
/// Earth's, less what the list does not count: its iron group 6.8%, its light lithophiles 28%,
/// its rare earths 28 ppm (CI's 10.0 × 23,500 ÷ 8,500), its silicates the rest. From 27 its iron
/// is its own mantle's (`under_27_a_mars_s_crust_holds_twice_an_earth_s_iron`).
#[test]
fn a_crust_at_solar_abundances_is_the_bulk_silicate_earth() {
    with_resource_ratios(ResourceRatiosRules::Held, || {
        let bulk = bulk_of(earth(), 0.0, 0.0);
        let crust = abundances(
            &bulk,
            SnowLineSide::Inside,
            OuterLayer::SilicateCrust,
            NOW,
            &ResourceDraws::MEDIAN,
        );
        let rock = 1.0
            - bulk.fractions().water()
                / (bulk.fractions().iron() + bulk.fractions().rock() + bulk.fractions().water());
        let per_rock = |resource| crust.fraction(resource) / rock;
        assert!((per_rock(Resource::IronGroup) - 0.068_42).abs() < 1e-4);
        assert!((per_rock(Resource::LightLithophiles) - 0.281).abs() < 1e-3);
        assert!((per_rock(Resource::RareEarths) - 27.67e-6).abs() < 0.01e-6);
        // ²³⁸U is the bulk silicate Earth's at the present age.
        let uranium = crust.radioactive(RadiogenicIsotope::Uranium238) / rock;
        assert!(
            (uranium / RadiogenicIsotope::Uranium238.bulk_silicate_earth_mass_fraction() - 1.0)
                .abs()
                < 1e-12
        );
        assert_eq!(crust.fraction(Resource::Hydrocarbons), 0.0);
        assert_eq!(crust.fraction(Resource::Helium3), 0.0);
    });
}

/// An ice shell holds no metal, and an icy body's ices carry its carbon dioxide and ammonia, and
/// its water deuterium at the ocean's D/H.
#[test]
fn an_ice_shell_is_its_ices_alone() {
    let bulk = bulk_of(icy(), 0.0, 0.0);
    let shell = abundances(
        &bulk,
        SnowLineSide::Beyond,
        OuterLayer::IceShell,
        NOW,
        &ResourceDraws::MEDIAN,
    );
    for metal in [
        Resource::IronGroup,
        Resource::LightLithophiles,
        Resource::RareEarths,
        Resource::PlatinumGroup,
        Resource::Radioactives,
        Resource::Silicates,
    ] {
        assert_eq!(shell.fraction(metal), 0.0, "{metal:?}");
    }
    assert!(shell.fraction(Resource::OtherIces) > 0.0);
    let deuterium = shell.fraction(Resource::Deuterium);
    let water = shell.fraction(Resource::WaterIce) + deuterium;
    assert!(
        (deuterium / water - 3.48e-5).abs() < 0.01e-5,
        "{}",
        deuterium / water
    );
    assert!((shell.total() - 1.0).abs() < 1e-12, "{}", shell.total());
}

/// Hydrocarbons only on an undifferentiated body formed beyond the snow line: CI's carbon per unit
/// of its rock, 5.6% of it.
#[test]
fn hydrocarbons_lie_only_in_an_icy_body_s_bulk() {
    let icy_bulk = bulk_of(icy(), 0.0, 0.0);
    let bulk = abundances(
        &icy_bulk,
        SnowLineSide::Beyond,
        OuterLayer::Bulk,
        NOW,
        &ResourceDraws::MEDIAN,
    );
    let fractions = icy_bulk.fractions();
    let rock = fractions.rock() / (fractions.iron() + fractions.rock() + fractions.water());
    assert!((bulk.fraction(Resource::Hydrocarbons) / rock - 0.0561).abs() < 1e-3);
    let earth = bulk_of(earth(), 0.0, 0.0);
    for layer in [OuterLayer::Bulk, OuterLayer::SilicateCrust] {
        let inside = abundances(
            &earth,
            SnowLineSide::Inside,
            layer,
            NOW,
            &ResourceDraws::MEDIAN,
        );
        assert_eq!(inside.fraction(Resource::Hydrocarbons), 0.0, "{layer:?}");
    }
}

/// A giant's scoop: Jupiter's gas holds about 3 × 10⁻⁵ of helium-3 by mass (the life brainstorm's
/// "Gas-giant fuel"), and deuterium at the protosolar D/H, 2.9 × 10⁻⁵, against the brainstorm's
/// 3–4 × 10⁻⁵ at the Galileo probe's D/H; its methane is its other ices, and it holds no metal.
#[test]
fn a_giant_s_gas_holds_its_helium_3_and_deuterium() {
    let gas = jupiter_gas();
    let bulk = bulk_of(earth(), 0.0, 0.0);
    let scoop = abundances(
        &bulk,
        SnowLineSide::Beyond,
        OuterLayer::Gas(&gas),
        NOW,
        &ResourceDraws::MEDIAN,
    );
    let helium_3 = scoop.fraction(Resource::Helium3);
    let deuterium = scoop.fraction(Resource::Deuterium);
    assert!((2.8e-5..3.1e-5).contains(&helium_3), "{helium_3}");
    assert!((2.7e-5..3.1e-5).contains(&deuterium), "{deuterium}");
    assert!((scoop.fraction(Resource::OtherIces) - 0.0146).abs() < 1e-3);
    assert_eq!(scoop.fraction(Resource::IronGroup), 0.0);
    assert_eq!(scoop.fraction(Resource::Silicates), 0.0);
    assert!(scoop.total() < 0.02, "{}", scoop.total());
    // No gas at all is no layer.
    let none = abundances(
        &bulk,
        SnowLineSide::Beyond,
        OuterLayer::Gas(&[]),
        NOW,
        &ResourceDraws::MEDIAN,
    );
    assert_eq!(none.total(), 0.0);
}

/// Helium-3 is modelled in a gas layer alone: every solid layer's 0 stands for the implanted
/// helium the model leaves out, so its modelled fraction is none, while a giant's gas gives its
/// fraction. Every other resource's modelled fraction is its fraction, on every layer.
#[test]
fn helium_3_is_modelled_only_in_a_gas_layer() {
    let gas = jupiter_gas();
    let bulk = bulk_of(earth(), 0.0, 0.0);
    let of = |layer| {
        abundances(
            &bulk,
            SnowLineSide::Inside,
            layer,
            NOW,
            &ResourceDraws::MEDIAN,
        )
    };
    for layer in [
        OuterLayer::Bulk,
        OuterLayer::SilicateCrust,
        OuterLayer::IceShell,
    ] {
        let solid = of(layer);
        assert_eq!(solid.fraction(Resource::Helium3), 0.0, "{layer:?}");
        assert_eq!(
            solid.modelled_fraction(Resource::Helium3),
            None,
            "{layer:?}"
        );
        for (resource, fraction) in solid.iter().filter(|&(r, _)| r != Resource::Helium3) {
            assert_eq!(
                solid.modelled_fraction(resource),
                Some(fraction),
                "{layer:?} {resource:?}"
            );
        }
    }
    let scoop = of(OuterLayer::Gas(&gas));
    for (resource, fraction) in scoop.iter() {
        assert_eq!(
            scoop.modelled_fraction(resource),
            Some(fraction),
            "{resource:?}"
        );
    }
    assert!(scoop.fraction(Resource::Helium3) > 0.0);
}

/// The median ranks give every multiple 1, and so the mean's fractions and isotopes, to rounding,
/// over every layer, both sides of the snow line and the halo's to the metal-rich disc's hosts.
#[test]
fn the_median_ranks_give_the_mean_to_rounding() {
    for resource in Resource::ALL {
        assert_eq!(
            ResourceDraws::MEDIAN.multiple(resource),
            1.0,
            "{resource:?}"
        );
    }
    let gas = jupiter_gas();
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-15 * a.abs().max(b.abs());
    for (of, side) in [
        (earth(), SnowLineSide::Inside),
        (icy(), SnowLineSide::Beyond),
    ] {
        for (fe_h, alpha_fe) in [(-2.5, 0.4), (0.0, 0.0), (0.5, -0.1)] {
            let bulk = bulk_of(of, fe_h, alpha_fe);
            for (layer, solid) in [
                (OuterLayer::Bulk, Some(Solid::Bulk)),
                (OuterLayer::SilicateCrust, Some(Solid::SilicateCrust)),
                (OuterLayer::IceShell, Some(Solid::IceShell)),
                (OuterLayer::Gas(&gas), None),
            ] {
                let inputs = ResourceInputs {
                    bulk: &bulk,
                    formed: side,
                    layer,
                    age: NOW,
                    host_magnesium_to_silicon: solar_magnesium_to_silicon(),
                    host_carbon_to_oxygen: solar_carbon_to_oxygen(),
                };
                let mean = solid.map_or_else(|| gas_mean(&gas), |solid| solid_mean(&inputs, solid));
                let median = ResourceAbundances::derive(&inputs, &ResourceDraws::MEDIAN);
                for (resource, x) in median.iter() {
                    let m = mean.fractions[resource.index()];
                    assert!(close(x, m), "{resource:?} on {layer:?}: {x} against {m}");
                }
                for (isotope, &m) in RadiogenicIsotope::ALL.iter().zip(&mean.radioactive) {
                    let x = median.radioactive(*isotope);
                    assert!(close(x, m), "{isotope:?} on {layer:?}: {x} against {m}");
                }
            }
        }
    }
}

/// Each lithology the crust names takes its layer: an ice shell for `H2O` alone, the bulk for an
/// undifferentiated body's own material and Ceres's frozen ocean, a silicate crust otherwise.
#[test]
fn every_lid_takes_its_layer() {
    for lid in CrustLithology::ALL {
        let expected = match lid.key() {
            "H2O" => OuterLayer::IceShell,
            "phyllosilicate" | "cometary" => OuterLayer::Bulk,
            key if key.starts_with("chondrite_") => OuterLayer::Bulk,
            _ => OuterLayer::SilicateCrust,
        };
        assert_eq!(OuterLayer::of_lid(lid), expected, "{}", lid.key());
    }
}

/// A body's ranks are words 0–10 of its `body.resources` stream, in turn, and the same seed and
/// body give the same ranks twice.
#[test]
fn the_ranks_are_the_stream_s_first_eleven_words() {
    let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), 0x0300);
    let draws = ResourceDraws::for_body(Seed::new(7), body);
    assert_eq!(draws, ResourceDraws::for_body(Seed::new(7), body));
    for (n, rank) in (0_u64..).zip(draws.ranks) {
        let mut stream = Stream::open(Seed::new(7), tags::BODY_RESOURCES, ObjectKey::from(body));
        stream.seek(n);
        assert_eq!(rank.value(), stream.uniform_open(), "word {n}");
    }
    assert!(u64::try_from(RESOURCES).unwrap() < RESOURCE_WORDS);
}

/// P14.T26's ratios are held until the 26 → 27 bump: the rules in force are the metallicity's
/// scale, the tests' switch and 27's helper reach the ratios, and 26's helper does not.
#[test]
fn the_resource_ratios_are_held_only_until_the_27_bump() {
    use crate::planetary::testing::{with_every_26_hold, with_every_27_hold};
    assert!(
        GENERATOR_VERSION < RESOURCE_RATIOS_VERSION,
        "the 26 → 27 bump puts P14.T26's ratios in a body's resources: delete \
         `RESOURCE_RATIOS_VERSION`, `ResourceRatiosRules` with its `Held` arm, its match in \
         `solid_mean`, `held_metals`, the tests' `with_resource_ratios`, its switch in \
         `with_every_27_hold` (and the helper, if no other 27 hold is left), `RULES` and this \
         test; restate or delete the tests held below 27, and re-bless `planetary/resources`, as \
         plan 14's record of P14.T26's ratios lists"
    );
    assert_eq!(ResourceRatiosRules::in_force(), ResourceRatiosRules::Held);
    assert_eq!(
        with_resource_ratios(ResourceRatiosRules::Ratios, ResourceRatiosRules::in_force),
        ResourceRatiosRules::Ratios
    );
    assert_eq!(
        with_every_27_hold(ResourceRatiosRules::in_force),
        ResourceRatiosRules::Ratios
    );
    assert_eq!(
        with_every_26_hold(ResourceRatiosRules::in_force),
        ResourceRatiosRules::Held
    );
    assert_eq!(
        with_every_27_hold(|| {
            with_resource_ratios(ResourceRatiosRules::Held, ResourceRatiosRules::in_force)
        }),
        ResourceRatiosRules::Held
    );
    assert_eq!(ResourceRatiosRules::in_force(), ResourceRatiosRules::Held);
}

/// `a`'s fractions and radioactive isotopes are `b`'s, each to the bit.
#[track_caller]
fn assert_same_abundances(a: &ResourceAbundances, b: &ResourceAbundances) {
    use hyperion_testkit::float::assert_same_bits;
    for ((_, x), (_, y)) in a.iter().zip(b.iter()) {
        assert_same_bits(x, y);
    }
    for isotope in RadiogenicIsotope::ALL {
        assert_same_bits(a.radioactive(isotope), b.radioactive(isotope));
    }
}

/// [P14.T26's ratios] Under every 27 hold, at a fixed \[α/Fe\] and fixed host ratios, \[Fe/H\] −1,
/// 0 and +0.3 give the same fractions, to the bit, on every solid layer, at the median ranks and
/// the seeded ones: a host's \[Fe/H\] sets how much rock forms, not what a kilogram of it holds.
/// It restates the plan's "iron group scales with 10^\[Fe/H\]" (lean, 2026-11-01).
#[test]
fn under_27_the_metallicity_moves_no_fraction_at_fixed_ratios() {
    use crate::planetary::testing::with_every_27_hold;
    with_every_27_hold(|| {
        for (of, side) in [
            (earth(), SnowLineSide::Inside),
            (icy(), SnowLineSide::Beyond),
        ] {
            for alpha_fe in [0.0, 0.3] {
                let solar = bulk_of(of, 0.0, alpha_fe);
                for fe_h in [-1.0, 0.3] {
                    let bulk = bulk_of(of, fe_h, alpha_fe);
                    for layer in [
                        OuterLayer::Bulk,
                        OuterLayer::SilicateCrust,
                        OuterLayer::IceShell,
                    ] {
                        for d in draws().iter().take(8).chain([&ResourceDraws::MEDIAN]) {
                            assert_same_abundances(
                                &abundances(&bulk, side, layer, NOW, d),
                                &abundances(&solar, side, layer, NOW, d),
                            );
                        }
                    }
                }
            }
        }
    });
}

/// [P14.T26's ratios] Under every 27 hold, an undifferentiated body's iron group is its own
/// metal's, its iron over its solids times `METAL`'s iron-group share, at \[Fe/H\] −1 as at 0: below
/// 27 a halo body reports a tenth of the iron its own mass fractions hold.
#[test]
fn under_27_a_bulk_s_iron_group_is_its_own_iron() {
    use crate::planetary::testing::with_every_27_hold;
    with_every_27_hold(|| {
        for (of, side) in [
            (earth(), SnowLineSide::Inside),
            (icy(), SnowLineSide::Beyond),
        ] {
            for fe_h in [-1.0, 0.0] {
                let bulk = bulk_of(of, fe_h, 0.3);
                let fractions = bulk.fractions();
                let solids = fractions.iron() + fractions.rock() + fractions.water();
                let own = fractions.iron() / solids * METAL.iron_group;
                let iron = abundances(&bulk, side, OuterLayer::Bulk, NOW, &ResourceDraws::MEDIAN)
                    .fraction(Resource::IronGroup);
                assert!(
                    (iron / own - 1.0).abs() < 1e-15,
                    "[Fe/H] {fe_h}: {iron} against {own}"
                );
            }
        }
    });
}

/// [P14.T26's ratios] Under every 27 hold, a halo twin's crust, \[Fe/H\] −1 and \[α/Fe\] +0.3,
/// against its disc twin's at 0 and 0, of the same composition and host ratios and at the median
/// ranks: its platinum group, rare earths, ⁴⁰K and iron group less its own mantle's iron are
/// 10^(−0.3) of the disc twin's, its uranium and thorium the same, and its light lithophiles,
/// nine tenths α elements, 0.97 of them, where below 27 they were a quarter. It restates the plan's
/// "a metal-poor halo planet is poorer in every metal than its disc twin" (lean, 2026-11-01).
#[test]
fn under_27_a_halo_twin_s_crust_is_poorer_only_in_its_non_alpha_metals() {
    use crate::planetary::testing::with_every_27_hold;
    with_every_27_hold(|| {
        let (halo, disc) = (bulk_of(earth(), -1.0, 0.3), bulk_of(earth(), 0.0, 0.0));
        let crust = |bulk: &BulkComposition| {
            abundances(
                bulk,
                SnowLineSide::Inside,
                OuterLayer::SilicateCrust,
                NOW,
                &ResourceDraws::MEDIAN,
            )
        };
        let (poor, rich) = (crust(&halo), crust(&disc));
        let fractions = disc.fractions();
        let rock =
            1.0 - fractions.water() / (fractions.iron() + fractions.rock() + fractions.water());
        // The iron group less the iron its mantle's `FeO` holds at the twin's [α/Fe].
        let traces = |a: &ResourceAbundances, alpha_fe: f64| {
            let feo = mantle_iron_oxide_mass_fraction(
                core_share(&fractions),
                solar_magnesium_to_silicon(),
                Dex::new(alpha_fe),
            );
            a.fraction(Resource::IronGroup) - rock * feo / iron_oxide_per_iron()
        };
        let s = math::exp10(-0.3);
        let ratio = |of: &dyn Fn(&ResourceAbundances) -> f64| of(&poor) / of(&rich);
        for (name, got, tolerance) in [
            (
                "platinum group",
                ratio(&|a| a.fraction(Resource::PlatinumGroup)),
                1e-12,
            ),
            (
                "rare earths",
                ratio(&|a| a.fraction(Resource::RareEarths)),
                1e-12,
            ),
            (
                "40K",
                ratio(&|a| a.radioactive(RadiogenicIsotope::Potassium40)),
                1e-12,
            ),
            ("iron traces", traces(&poor, 0.3) / traces(&rich, 0.0), 1e-9),
        ] {
            assert!(
                (got / s - 1.0).abs() < tolerance,
                "{name}: {got} against {s}"
            );
        }
        for isotope in [
            RadiogenicIsotope::Uranium238,
            RadiogenicIsotope::Uranium235,
            RadiogenicIsotope::Thorium232,
        ] {
            let got = ratio(&|a| a.radioactive(isotope));
            assert!((got - 1.0).abs() < 1e-15, "{isotope:?}: {got}");
        }
        let lithophiles = ratio(&|a| a.fraction(Resource::LightLithophiles));
        assert!((0.96..0.97).contains(&lithophiles), "{lithophiles}");
        let held = with_resource_ratios(ResourceRatiosRules::Held, || {
            crust(&halo).fraction(Resource::LightLithophiles)
                / crust(&disc).fraction(Resource::LightLithophiles)
        });
        assert!((0.24..0.26).contains(&held), "{held}");
    });
}

/// [P14.T26's ratios] Under every 27 hold a silicate crust's iron is its own mantle's `FeO`'s:
/// about a solar host, a Mars's core share (0.204) gives its crust 12.3 wt% Fe, near twice an
/// Earth's (0.325) 6.26, the bulk silicate Earth's (McDonough and Sun 1995), its iron group less
/// the bulk silicate Earth's other iron-peak metals; below 27 both were the bulk silicate Earth's.
#[test]
fn under_27_a_mars_s_crust_holds_twice_an_earth_s_iron() {
    use crate::planetary::testing::with_every_27_hold;
    let iron = |core: f64| {
        let fractions = crate::planetary::derive::MassFractions::solid(core, 1.0 - core, 0.0);
        let bulk = BulkComposition::new(fractions, earth().1, Dex::ZERO, Some(Dex::ZERO));
        let crust = abundances(
            &bulk,
            SnowLineSide::Inside,
            OuterLayer::SilicateCrust,
            NOW,
            &ResourceDraws::MEDIAN,
        );
        crust.fraction(Resource::IronGroup) - BULK_SILICATE_EARTH_IRON_TRACES
    };
    with_every_27_hold(|| {
        let (mars, earth) = (iron(0.204), iron(0.325));
        assert!((0.122..0.124).contains(&mars), "{mars}");
        let bse = BULK_SILICATE_EARTH_IRON_GROUP[3];
        assert!((earth / bse - 1.0).abs() < 0.005, "{earth} against {bse}");
        assert!((1.9..2.0).contains(&(mars / earth)), "{}", mars / earth);
    });
    let held = with_resource_ratios(ResourceRatiosRules::Held, || (iron(0.204), iron(0.325)));
    let bse = BULK_SILICATE_EARTH_IRON_GROUP[3];
    assert!((held.0 / bse - 1.0).abs() < 1e-12 && (held.1 / bse - 1.0).abs() < 1e-12);
}

/// Under every 27 hold the light lithophiles' odds form moves them by under 2% and the fill cap
/// never binds, over hosts of \[α/Fe\] −0.2 to +0.5, Mg/Si 0.5–2 and C/O 0.2–1.2 and bodies from no
/// core to nine tenths iron, with and without water, on a bulk formed beyond the snow line and a
/// silicate crust: the [module](super)'s "The mean".
#[test]
fn under_27_neither_the_odds_nor_the_cap_binds() {
    use crate::planetary::testing::with_every_27_hold;
    with_every_27_hold(|| {
        let elements = icy().1;
        let mut checked = 0_u32;
        for core in [0.0, 0.1, 0.204, 0.325, 0.5, 0.7, 0.9] {
            for water in [0.0, 0.3] {
                let solid = 1.0 - water;
                let fractions = crate::planetary::derive::MassFractions::solid(
                    core * solid,
                    (1.0 - core) * solid,
                    water,
                );
                for alpha_fe in [-0.2, 0.0, 0.3, 0.5] {
                    let bulk = BulkComposition::new(
                        fractions,
                        elements,
                        Dex::ZERO,
                        Some(Dex::new(alpha_fe)),
                    );
                    for (mg_si, c_o) in [(0.5, 0.2), (1.0, solar_carbon_to_oxygen()), (2.0, 1.2)] {
                        let inputs = ResourceInputs {
                            bulk: &bulk,
                            formed: SnowLineSide::Beyond,
                            layer: OuterLayer::Bulk,
                            age: NOW,
                            host_magnesium_to_silicon: mg_si,
                            host_carbon_to_oxygen: c_o,
                        };
                        for (layer, metal, rock, column) in [
                            (Solid::Bulk, core * solid, (1.0 - core) * solid, &CI_ROCK),
                            (Solid::SilicateCrust, 0.0, solid, &BULK_SILICATE),
                        ] {
                            let of = Reservoirs {
                                layer,
                                metal,
                                rock,
                                column,
                                since: 0.0,
                                organic: layer == Solid::Bulk,
                            };
                            let m = ratio_metals(&inputs, &of);
                            let metals = m.iron_group
                                + m.platinum_group
                                + m.lithophiles
                                + m.rare_earths
                                + m.radioactive.iter().sum::<f64>();
                            let available = metal + rock - m.hydrocarbons;
                            assert!(
                                metals < 0.95 * available,
                                "{layer:?}, core {core}, [α/Fe] {alpha_fe}: {metals} of {available}"
                            );
                            let linear = rock
                                * (column.alpha_lithophiles
                                    + column.other_lithophiles * math::exp10(-alpha_fe));
                            assert!(
                                (m.lithophiles / linear - 1.0).abs() < 0.02,
                                "{layer:?}, [α/Fe] {alpha_fe}: {} against {linear}",
                                m.lithophiles
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 7 * 2 * 4 * 3 * 2);
    });
}
