use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::solar::{
    SOLAR_AGE, derive_historic, placed_about_the_sun, solar_disc, table_bodies,
};
use crate::units::consts::METRES_PER_AU;

/// The table's body `name`, derived about the zero-age Sun at its own distance with its history,
/// and the inputs of its element inventory at the ranks `draws`.
fn table_body(
    name: &str,
    draws: VolatileDraws,
) -> (crate::planetary::derive::DerivedBody, InventoryInputs) {
    let disc = solar_disc();
    let &(_, kg, km, a, e) = table_bodies()
        .find(|(n, ..)| *n == name)
        .expect("the body is in the table");
    let placed = placed_about_the_sun(&disc, (kg, km, a, e));
    let body = derive_historic(&placed, &disc, SOLAR_AGE);
    let inputs = InventoryInputs {
        mass: body.mass(),
        fractions: body.fractions(),
        formed: body.formed(),
        formation_distance: Metres::new(a * METRES_PER_AU),
        snow_line: disc.snow_line(),
        draws,
        age: SOLAR_AGE,
    };
    (body, inputs)
}

/// Ranks to try: the median, both one-sigma ranks and the extremes, in each field.
fn draws_to_try() -> Vec<VolatileDraws> {
    let ranks = [
        0.5,
        0.158_655_253_931_457_05,
        0.841_344_746_068_542_9,
        1e-6,
        0.999_999,
    ];
    let r = |u: f64| UnitUniform::new(u).expect("inside (0, 1)");
    let mut all = vec![VolatileDraws::MEDIAN];
    for (i, &u) in ranks.iter().enumerate() {
        let v = ranks[(i + 2) % ranks.len()];
        all.push(VolatileDraws {
            water: r(u),
            carbon: r(v),
            nitrogen: r(u),
            sulphur: r(v),
            chlorine: r(u),
            redox: r(v),
        });
    }
    all
}

/// P14.T50.a: H₂O, CO₂, N₂ and Ar as today, to the bit: the inventory's carriers are
/// `volatile_inventory`'s, and for the table's bodies the atmosphere's own inventory, at every
/// rank tried, on either side of the snow line.
#[test]
fn the_carriers_are_today_s_to_the_bit() {
    for name in [
        "Mercury", "Venus", "Earth", "Mars", "Moon", "Ganymede", "Titan", "Ceres",
    ] {
        let (body, at_median) = table_body(name, VolatileDraws::MEDIAN);
        let view = *element_inventory(&at_median).volatiles();
        for (a, b) in [
            (view.water(), body.inventory().water()),
            (view.carbon_dioxide(), body.inventory().carbon_dioxide()),
            (view.nitrogen(), body.inventory().nitrogen()),
            (view.argon(), body.inventory().argon()),
        ] {
            assert_same_bits(a.value(), b.value());
        }
        for draws in draws_to_try() {
            for formed in [SnowLineSide::Inside, SnowLineSide::Beyond] {
                let inputs = InventoryInputs {
                    formed,
                    draws,
                    ..at_median
                };
                let view = *element_inventory(&inputs).volatiles();
                let today =
                    volatile_inventory(inputs.mass, &inputs.fractions, formed, &draws, inputs.age);
                for (a, b) in [
                    (view.water(), today.water()),
                    (view.carbon_dioxide(), today.carbon_dioxide()),
                    (view.nitrogen(), today.nitrogen()),
                    (view.argon(), today.argon()),
                ] {
                    assert_same_bits(a.value(), b.value());
                }
            }
        }
    }
}

/// P14.T50.a: the elements sum to the carriers at 10⁻⁹. The exchangeable elements are the four
/// carriers and the primordial noble gases, each carrier's elements are its mass, and the mantle's
/// are its sulphur and chlorine.
#[test]
fn the_elements_sum_to_the_carriers() {
    let close = |a: f64, b: f64, what: &str| {
        assert!((a - b).abs() <= 1e-9 * b.abs(), "{what}: {a} against {b}");
    };
    for name in ["Venus", "Earth", "Mars", "Titan"] {
        for draws in draws_to_try() {
            let (_, inputs) = table_body(name, draws);
            let inventory = element_inventory(&inputs);
            let v = inventory.volatiles();
            let exchangeable = INVENTORY_ELEMENTS.iter().fold(0.0, |sum, &e| {
                sum + inventory.mass(e, Reservoir::Exchangeable).value()
            });
            let noble = [Element::He, Element::Ne, Element::Kr, Element::Xe]
                .iter()
                .fold(inventory.primordial_argon().value(), |sum, &e| {
                    sum + inventory.mass(e, Reservoir::Exchangeable).value()
                });
            let carriers = v.water().value()
                + v.carbon_dioxide().value()
                + v.nitrogen().value()
                + v.argon().value();
            close(exchangeable, carriers + noble, name);
            // Each carrier, by its elements: the water's hydrogen, the carbon dioxide's carbon.
            let w = Element::standard_atomic_weight_g_per_mol;
            let water_h = inventory.mass(Element::H, Reservoir::Exchangeable).value();
            close(
                water_h * (2.0 * w(Element::H) + w(Element::O)) / (2.0 * w(Element::H)),
                v.water().value(),
                "water",
            );
            let carbon = inventory.mass(Element::C, Reservoir::Exchangeable).value();
            close(
                carbon * (w(Element::C) + 2.0 * w(Element::O)) / w(Element::C),
                v.carbon_dioxide().value(),
                "carbon dioxide",
            );
            close(
                inventory.mass(Element::O, Reservoir::Exchangeable).value() + water_h + carbon,
                v.water().value() + v.carbon_dioxide().value(),
                "oxygen",
            );
            let mantle = INVENTORY_ELEMENTS.iter().fold(0.0, |sum, &e| {
                sum + inventory.mass(e, Reservoir::Mantle).value()
            });
            close(
                mantle,
                inventory.mass(Element::S, Reservoir::Mantle).value()
                    + inventory.mass(Element::Cl, Reservoir::Mantle).value(),
                "mantle",
            );
            for e in Element::ALL {
                let both = inventory.mass(e, Reservoir::Exchangeable)
                    + inventory.mass(e, Reservoir::Mantle);
                assert_same_bits(inventory.total(e).value(), both.value());
                if !INVENTORY_ELEMENTS.contains(&e) {
                    assert_same_bits(inventory.total(e).value(), 0.0);
                }
            }
        }
    }
}

/// P14.T50.a: Earth's, Venus's and Mars's sulphur within stated ranges, from cited inventories,
/// each at its derived rock mass.
///
/// - Earth's at its median lies within the concordance's 235 ± 38 ppm of its silicate mass
///   (Wang et al. 2018, Table 1).
/// - Mars's bulk silicate 360 ppm ± 33% (Yoshizaki and McDonough 2020, GCA 273, 137,
///   arXiv:1909.07560, Table 5, read; from Wang and Becker 2017) lies within one sigma of the
///   median of a body of Mars's mass at Mars's distance, and the higher estimates up to
///   2,000 ppm they cite (Gaillard et al. 2013; Ding et al. 2015) within two.
/// - Venus's air holds 130 ± 50 ppmv of SO₂ (Marcq et al. 2008, JGR 113, E00B07, as
///   science-r08-sulphur-co2ice.md verified it): over 92 bar at 8.87 m s⁻² and a mean molar mass of
///   43.45 g mol⁻¹ (NASA's Venus fact sheet), some 5 × 10¹⁶ kg of sulphur, which the inventory
///   exceeds a thousandfold even two sigma below its median; and no body holds, at two sigma
///   above, more than CI chondrites' 5.35 wt% (Lodders 2010, Table 2).
#[test]
fn sulphur_lies_within_the_cited_inventories() {
    let ppm = |name: &str, draws: VolatileDraws| {
        let (_, inputs) = table_body(name, draws);
        let rock = inputs.fractions.rock() * inputs.mass.value() * EARTH_MASS_KG;
        let s = element_inventory(&inputs)
            .mass(Element::S, Reservoir::Mantle)
            .value();
        (s / rock * 1e6, s)
    };
    let at = |u: f64| VolatileDraws {
        sulphur: UnitUniform::new(u).expect("inside (0, 1)"),
        ..VolatileDraws::MEDIAN
    };
    let (one_down, one_up) = (0.158_655_253_931_457_05, 0.841_344_746_068_542_9);
    let (two_down, two_up) = (0.022_750_131_948_179_2, 0.977_249_868_051_820_8);
    let (earth, _) = ppm("Earth", VolatileDraws::MEDIAN);
    assert!((197.0..=273.0).contains(&earth), "Earth: {earth} ppm");
    let mars_band = |low: f64, high: f64| (ppm("Mars", at(low)).0, ppm("Mars", at(high)).0);
    let (low, high) = mars_band(one_down, one_up);
    assert!(
        low <= 360.0 && 360.0 <= high,
        "Mars at one sigma: {low}–{high} ppm"
    );
    let (low, high) = mars_band(two_down, two_up);
    assert!(
        low <= 240.0 && 2_000.0 <= high,
        "Mars at two sigma: {low}–{high} ppm"
    );
    // Venus's air: 92 bar over 4πR² at 8.87 m s⁻², 130 ppmv of SO₂ of mean molar mass 43.45.
    let radius = 6_051.8e3;
    let venus_air_kg = 92e5 / 8.87 * 4.0 * core::f64::consts::PI * radius * radius;
    let air_sulphur = venus_air_kg * 130e-6 * Element::S.standard_atomic_weight_g_per_mol() / 43.45;
    let (_, lowest) = ppm("Venus", at(two_down));
    assert!(
        lowest > 1e3 * air_sulphur,
        "{lowest} kg against the air's {air_sulphur}"
    );
    for name in ["Venus", "Earth", "Mars"] {
        let (high, _) = ppm(name, at(two_up));
        assert!(high < 53_500.0, "{name}: {high} ppm");
    }
}

/// Chlorine is the bulk silicate Earth's per unit mass of rock inside the snow line, and CI
/// chondrites' beyond it, at the same rank: 698 ÷ 16.2 times more.
#[test]
fn chlorine_takes_the_chondrites_abundance_beyond_the_snow_line() {
    let (_, inside) = table_body("Earth", VolatileDraws::MEDIAN);
    let beyond = InventoryInputs {
        formed: SnowLineSide::Beyond,
        ..inside
    };
    let cl = |inputs: &InventoryInputs| {
        element_inventory(inputs)
            .mass(Element::Cl, Reservoir::Mantle)
            .value()
    };
    let rock = inside.fractions.rock() * inside.mass.value() * EARTH_MASS_KG;
    assert!((cl(&inside) / (BSE_CHLORINE_PER_MASS * rock) - 1.0).abs() < 1e-12);
    assert!((cl(&beyond) / cl(&inside) - 698.0 / 16.2).abs() < 1e-9);
    // Sulphur keeps the bulk silicate Earth's on either side.
    let s = |inputs: &InventoryInputs| {
        element_inventory(inputs)
            .mass(Element::S, Reservoir::Mantle)
            .value()
    };
    assert_same_bits(s(&beyond), s(&inside));
}

/// The ranks scale sulphur and chlorine log-normally, half a decade at one sigma; the noble gases
/// take no draw; the mantle's ΔIW is the redox model's at the redox rank.
#[test]
fn the_ranks_set_what_they_name_and_nothing_else() {
    let (_, median) = table_body("Mars", VolatileDraws::MEDIAN);
    let sigma = UnitUniform::new(0.841_344_746_068_542_9).expect("inside (0, 1)");
    let base = element_inventory(&median);
    for (draws, element) in [
        (
            VolatileDraws {
                sulphur: sigma,
                ..VolatileDraws::MEDIAN
            },
            Element::S,
        ),
        (
            VolatileDraws {
                chlorine: sigma,
                ..VolatileDraws::MEDIAN
            },
            Element::Cl,
        ),
    ] {
        let moved = element_inventory(&InventoryInputs { draws, ..median });
        let ratio = moved.total(element) / base.total(element);
        assert!(
            (ratio - math::exp10(0.5)).abs() < 1e-6,
            "{element:?}: {ratio}"
        );
    }
    for draws in draws_to_try() {
        let other = element_inventory(&InventoryInputs { draws, ..median });
        for e in [Element::He, Element::Ne, Element::Kr, Element::Xe] {
            assert_same_bits(
                other.mass(e, Reservoir::Exchangeable).value(),
                base.mass(e, Reservoir::Exchangeable).value(),
            );
        }
        assert_same_bits(
            other.primordial_argon().value(),
            base.primordial_argon().value(),
        );
        let redox = RedoxInputs {
            formation_distance: median.formation_distance,
            snow_line: median.snow_line,
            water_fraction: median.fractions.water(),
            mass: median.mass,
        };
        assert_same_bits(
            other.mantle_delta_iw().value(),
            redox::delta_iw(&redox, draws.redox).value(),
        );
        assert_eq!(other.redox_class(), RedoxClass::of(other.mantle_delta_iw()));
    }
    // The noble gases are CI chondrites' per unit mass of rock.
    let rock = median.fractions.rock() * median.mass.value() * EARTH_MASS_KG;
    for (e, per_mass) in [
        (Element::He, CI_HELIUM_PER_MASS),
        (Element::Ne, CI_NEON_PER_MASS),
        (Element::Kr, CI_KRYPTON_PER_MASS),
        (Element::Xe, CI_XENON_PER_MASS),
    ] {
        let got = base.mass(e, Reservoir::Exchangeable).value();
        assert!((got / (per_mass * rock) - 1.0).abs() < 1e-12, "{e:?}");
    }
    assert!((base.primordial_argon().value() / (CI_ARGON_PER_MASS * rock) - 1.0).abs() < 1e-12);
}

/// A body of no mass, or of a negative one, holds nothing.
#[test]
fn an_empty_body_holds_nothing() {
    let (_, earth) = table_body("Earth", VolatileDraws::MEDIAN);
    for mass in [0.0, -1.0] {
        let inventory = element_inventory(&InventoryInputs {
            mass: EarthMasses::new(mass),
            ..earth
        });
        for e in [Element::S, Element::Cl, Element::He, Element::Xe] {
            assert_same_bits(inventory.total(e).value(), 0.0);
        }
    }
}
