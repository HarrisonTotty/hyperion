//! Tests of the cold traps (P14.T51.f): the exposed fraction's draw, the hold, the bodies and
//! condensables they take, the zonal annual maximum, the latitude rule, the plan's figures for the
//! Moon, Ceres and a tilted body, the merge and the record path.

use hyperion_surface::cold_trap::permanent_shadow;
use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

use super::*;
use crate::id::SystemId;
use crate::planetary::chem::partition::{
    ClimateSpan, ColumnInventory, Ground, WaterCover, partition,
};
use crate::planetary::derive::solar::historic_table;
use crate::planetary::hooks::figures::table_figures;
use crate::planetary::hooks::surface::tests::{inputs_of, spin_of};
use crate::planetary::hooks::surface::{SurfaceConditions, SurfaceInputs, ZONAL_AREA_POINTS};
use crate::units::KilogramsPerCubicMetre;
use crate::units::consts::METRES_PER_AU;

/// Over 10⁴ bodies the exposed fractions are log-uniform on [0.016, 0.45] (Kolmogorov–Smirnov on
/// ln(f ÷ 0.016) ÷ ln(0.45 ÷ 0.016)), each the open uniform of word 0 of its own
/// `body.cold_traps` stream and no other word, and the same twice.
#[test]
fn the_exposed_fraction_is_log_uniform_in_its_bounds_and_reads_word_0_alone() {
    let seed = Seed::new(0x0014_0051);
    let system = SystemId::from_raw(0x0200_0800_2000_0051).unwrap();
    let span = math::ln(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR);
    let mut ranks: Vec<f64> = (0..10_000_u16)
        .map(|i| {
            let body = BodyId::new(system, 0x0100 + i);
            let f = exposed_fraction(seed, body);
            assert_same_bits(f, exposed_fraction(seed, body));
            assert!(
                (EXPOSED_FRACTION_FLOOR..=EXPOSED_FRACTION_CEILING).contains(&f),
                "{body}: {f}"
            );
            let mut stream = Stream::open(seed, tags::BODY_COLD_TRAPS, ObjectKey::from(body));
            let u = stream.uniform_open();
            assert_eq!(stream.position(), 1);
            let expected = EXPOSED_FRACTION_FLOOR
                * math::powf(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR, u);
            assert_same_bits(
                f,
                expected.clamp(EXPOSED_FRACTION_FLOOR, EXPOSED_FRACTION_CEILING),
            );
            math::ln(f / EXPOSED_FRACTION_FLOOR) / span
        })
        .collect();
    let ks = ks_one_sample(&mut ranks, |x| x.clamp(0.0, 1.0));
    assert_p_value("the exposed fractions' log ranks", ks.p_value, ALPHA);
}

/// The draw takes its own stream: a body's fraction is word 0 of `body.cold_traps` alone, not of
/// any other body tag it shares a key with, and it moves with the seed and the body.
#[test]
fn the_exposed_fraction_takes_its_own_stream() {
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let body = BodyId::new(system, 0x1300);
    let fraction = |raw_seed, body| exposed_fraction(Seed::new(raw_seed), body);
    let rank = |tag, raw_seed| {
        Stream::open(Seed::new(raw_seed), tag, ObjectKey::from(body)).uniform_open()
    };
    let of_rank = |u: f64| {
        (EXPOSED_FRACTION_FLOOR * math::powf(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR, u))
            .clamp(EXPOSED_FRACTION_FLOOR, EXPOSED_FRACTION_CEILING)
    };
    for raw_seed in [0, 7, 0x0014_0051, u64::MAX] {
        let f = fraction(raw_seed, body);
        assert_same_bits(f, of_rank(rank(tags::BODY_COLD_TRAPS, raw_seed)));
        for other in [
            tags::BODY_FORMATION,
            tags::BODY_CONTINENTS,
            tags::BODY_SURFACE,
        ] {
            assert_ne!(bits(f), bits(of_rank(rank(other, raw_seed))));
        }
    }
    assert_ne!(bits(fraction(7, body)), bits(fraction(8, body)));
    let neighbour = BodyId::new(system, 0x1301);
    assert_ne!(bits(fraction(7, body)), bits(fraction(7, neighbour)));
    assert_eq!(EXPOSED_FRACTION_WORDS, 8);
}

/// The Moon's whole area, km², at its mean radius of 1,737.4 km.
const MOON_KM2: f64 = 4.0 * PI * 1_737.4 * 1_737.4;

/// The lunar ground the plan's tests read: the real Sun at Earth's perihelion ([`lunar_sky`]),
/// Hayne et al.'s albedo 0.12 and the Apollo-era heat flow 0.018 W m⁻² (R09.T7.d's real Moon).
fn real_moon() -> Sunlit {
    Sunlit::new(lunar_sky(), 0.12, WattsPerSquareMetre::new(0.018))
}

/// Hayne et al.'s best-fit lunar terrain with craters of `depths`: 20% craters, plains at an RMS
/// slope of 0.1 and the lunar share under the conduction cut-off.
fn lunar_terrain(depths: DepthRatios) -> Terrain {
    Terrain::new(TerrainParts {
        crater_fraction: SATURATED_CRATER_FRACTION,
        crater_depths: depths,
        plains_slope: Radians::new(math::atan(PLAINS_RMS_SLOPE)),
        share_under_cutoff: LUNAR_SHARE_UNDER_CUTOFF,
    })
    .unwrap()
}

/// The cold-trap threshold of the gas `gas` at 144 kg m⁻² Gyr⁻¹.
fn threshold(gas: Gas) -> Kelvin {
    threshold_at(gas, AIRLESS_ICE_LOSS_KG_PER_M2_GYR)
}

/// The cold-trap threshold of the gas `gas` at the loss `loss`, kg m⁻² Gyr⁻¹.
fn threshold_at(gas: Gas, loss: f64) -> Kelvin {
    exosphere_threshold(gas.substance(), loss).unwrap()
}

/// The share of the surface `ground`'s traps of `threshold` cover between `lo` and `hi` degrees
/// of latitude, by the rule's own nodes (the bands' edges include 70° and 80°).
fn band(ground: &Sunlit, terrain: &Terrain, threshold: Kelvin, (lo, hi): (f64, f64)) -> f64 {
    let shadows = ground.shadows().unwrap();
    over_latitude(|latitude| {
        let degrees = latitude.value().to_degrees();
        if (lo..=hi).contains(&degrees)
            && ground.annual_maximum(Radians::new(FRAC_PI_2 - latitude.value())) >= threshold
        {
            cold_trap::fraction(latitude, &shadows, terrain, threshold)
        } else {
            0.0
        }
    })
}

/// ∫ f(φ) cos φ dφ over `lo`..`hi` degrees by the midpoint rule at `step` degrees.
fn midpoint(f: impl Fn(Radians) -> f64, (lo, hi): (f64, f64), step: f64) -> f64 {
    let n = ((hi - lo) / step).round();
    let width = Radians::from(Degrees::new((hi - lo) / n)).value();
    let mut sum = 0.0;
    let mut i = 0.0;
    while i < n {
        let latitude = Radians::from(Degrees::new(lo + (hi - lo) * (i + 0.5) / n));
        sum += width * math::cos(latitude.value()) * f(latitude);
        i += 1.0;
    }
    sum
}

/// The cold traps are held until P14.T54.b's bump, and held they publish P14.T51.a's entries bit
/// for bit; under the batch every body of the table that is not an airless one with no
/// collisional air is P14.T51.a's too.
#[test]
fn the_cold_traps_are_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < COLD_TRAPS_VERSION,
        "the 23 → 24 bump (P14.T54.b) puts the cold traps on the record: every airless body with \
         no collisional air that is not locked to its star takes the zonal annual maximum's \
         airless branch and its cold-trapped ices. Delete `COLD_TRAPS_VERSION`, `ColdTrapRules` \
         and its `Held` arm, `RecordColdTraps::rules` and `condensates_under`'s `Held` branch, \
         `RecordRules` with `SurfaceCase::record` and `PlanetarySystem::with_record_rules` (the \
         record's partition then follows the snapshot's hold, deleted at 23), and this test. \
         Settle too that T13.c's passes (`surface_mix::pass_condensates`) keep P14.T51.a's \
         airless branch: give them `cold_traps::publish_over` over one `ColdTrapField` a body \
         (P14.T51.i), with the record's sky and these rules, or the Bond albedo reads other areas \
         than the record publishes (plan 14, P14.T51.f and T51.i as built)"
    );
    assert_eq!(ColdTrapRules::in_force(), ColdTrapRules::Held);
    let sky = lunar_sky();
    let mut held_bodies = 0;
    for (name, body) in historic_table() {
        let Some(figures) = table_figures(&body) else {
            continue;
        };
        let inputs = SurfaceInputs {
            heat_flow: figures.heat_flow(),
            ocean_weight: figures.ocean_weight(),
            ..inputs_of(&body, spin_of(name))
        };
        let Some(conditions) = SurfaceConditions::derive(&inputs) else {
            continue;
        };
        let (air, g) = (body.atmosphere(), body.surface_gravity());
        let table = conditions.zonal_insolation();
        let under = |traps| {
            figures.condensates_under(
                air,
                &conditions,
                g,
                &RecordColdTraps::at_the_albedo_of(traps, &sky, air),
                &table,
            )
        };
        let (solved, held) = under(ColdTrapRules::Held);
        let profile = conditions.cold_profile();
        let cold_area = |t: Kelvin| profile.area_below(t);
        let t51a = solved.publish(&Cover {
            cold_area: &cold_area,
            state: air.state(),
            water: figures.water_cover(&conditions, &profile),
        });
        assert_eq!(format!("{held:?}"), format!("{t51a:?}"), "{name}");
        let (_, batch) = under(ColdTrapRules::Batch);
        let qualifies = air.state() == SurfaceState::Airless && !is_collisional(air, g);
        if !qualifies {
            assert_eq!(format!("{batch:?}"), format!("{held:?}"), "{name}");
            held_bodies += 1;
        }
        eprintln!("{name}: qualifies {qualifies}; held {held:?}; batch {batch:?}");
    }
    assert!(held_bodies >= 3, "{held_bodies}");
}

/// The Moon and Pluto parted by the collisional test (decision-p14-t51a-part2 §3.3): the Moon's
/// exobase lies at its ground below 2.5 × 10⁻⁷ Pa of N₂, Pluto's 1 Pa and Triton's 1.4 Pa are
/// collisional though airless by P14.T13.c's 100 Pa, and so are the table's airs; a NaN is too.
#[test]
fn the_collisional_test_parts_the_moon_from_pluto() {
    let n2 = Gas::Nitrogen.molar_mass_g_per_mol();
    let moon = MetresPerSecondSquared::new(1.62);
    let edge = n2 * ATOMIC_MASS_CONSTANT_KG * 1.62 / COLLISION_CROSS_SECTION_M2;
    assert!((2.4e-7..2.6e-7).contains(&edge), "{edge}");
    assert!(!collisional(Pascals::new(3e-10), n2, moon));
    assert!(!collisional(Pascals::new(0.99 * edge), n2, moon));
    assert!(collisional(Pascals::new(1.01 * edge), n2, moon));
    assert!(collisional(
        Pascals::new(1.0),
        n2,
        MetresPerSecondSquared::new(0.62)
    ));
    assert!(collisional(
        Pascals::new(1.4),
        n2,
        MetresPerSecondSquared::new(0.78)
    ));
    assert!(collisional(Pascals::new(f64::NAN), n2, moon));
    for (name, body) in historic_table() {
        let air = body.atmosphere();
        let thin = !is_collisional(air, body.surface_gravity());
        eprintln!(
            "{name}: {:?} at {:?}, no collisional air {thin}",
            air.state(),
            air.surface_pressure()
        );
        if air.state() != SurfaceState::Airless {
            assert!(!thin, "{name}");
        }
        if name == "Moon" {
            assert!(thin, "the table's Moon");
        }
    }
}

/// The condensables are the inventory's species with a phase diagram and the delivered water and
/// carbon dioxide, in registry order, each once.
#[test]
fn the_condensables_are_the_inventory_s_and_the_delivered_in_registry_order() {
    let climate =
        ClimateSpan::new(Kelvin::new(30.0), Kelvin::new(40.0), Kelvin::new(50.0)).unwrap();
    let ground = Ground::without_basins(MetresPerSecondSquared::new(0.6));
    let empty = partition(&climate, &ground, &ColumnInventory::default());
    assert_eq!(
        condensables(&empty),
        [Gas::Water.substance(), Gas::CarbonDioxide.substance()]
    );
    let gases = [
        Gas::Helium,
        Gas::Methane,
        Gas::Nitrogen,
        Gas::CarbonDioxide,
        Gas::Argon,
    ];
    let inventory =
        ColumnInventory::new(gases.iter().map(|g| (g.substance(), Pascals::new(1e-3)))).unwrap();
    let held = condensables(&partition(&climate, &ground, &inventory));
    let mut expected: Vec<SubstanceId> = gases
        .iter()
        .map(|g| g.substance())
        .filter(|s| s.substance().phase_diagram().is_some())
        .chain([Gas::Water.substance()])
        .collect();
    expected.sort_unstable();
    expected.dedup();
    assert_eq!(held, expected);
    assert!(held.is_sorted());
}

/// The sky takes min(ε, 180° − ε), the flux and the star's radius to periapsis.
#[test]
fn the_sky_is_taken_at_periapsis() {
    let a = Metres::new(1.495_978_707e11);
    let sky = |degrees: f64, e: f64| {
        ColdTrapSky::at_periapsis(
            Radians::from(Degrees::new(degrees)),
            WattsPerSquareMetre::new(1361.0),
            (a, e),
            Some(SolarRadii::new(1.0)),
            0.035,
        )
    };
    let venus = sky(177.36, 0.0).greatest_declination.value().to_degrees();
    assert!((venus - 2.64).abs() < 1e-9, "{venus}");
    let earth = sky(23.44, 0.016_709);
    assert!((earth.greatest_declination.value().to_degrees() - 23.44).abs() < 1e-9);
    let e: f64 = 0.016_709;
    let expected = 1361.0 * (1.0 - e * e).sqrt() / ((1.0 - e) * (1.0 - e));
    assert!((earth.periapsis_flux.value() - expected).abs() < 1e-9);
    // The Sun at Earth's perihelion: 0.2711° (R09.T7.d's real Moon).
    let alpha = earth.star_radius.value().to_degrees();
    assert!((alpha - 0.2711).abs() < 2e-4, "{alpha}");
    let point = ColdTrapSky::at_periapsis(
        Radians::ZERO,
        WattsPerSquareMetre::new(1361.0),
        (a, 0.0),
        None,
        0.035,
    );
    assert_same_bits(point.star_radius.value(), 0.0);
}

/// The zonal annual maximum on flat ground (the research's checks, science-r09-drainage.md §4.1
/// gives 156 K and 224 K without the disk and the heat flow): the Moon's pole 158 K and 85° 227 K
/// at the sim's albedo 0.11 and heat flow 0.0119 W m⁻², Mercury's pole 172 K with the Sun's disk
/// and 110 K as a point; a body at 12.3 au has a rocky equator at about 110 K. A smooth lunar pole
/// lays no zonal water cap.
#[test]
fn the_zonal_annual_maximum_matches_the_research_s() {
    let moon = Sunlit::new(lunar_sky(), 0.11, WattsPerSquareMetre::new(0.0119));
    let at = |ground: &Sunlit, latitude: f64| {
        ground
            .annual_maximum(Radians::from(Degrees::new(90.0 - latitude)))
            .value()
    };
    let (pole, eighty_five) = (at(&moon, 90.0), at(&moon, 85.0));
    assert!((pole - 158.13).abs() < 0.5, "{pole}");
    assert!((eighty_five - 226.87).abs() < 0.5, "{eighty_five}");
    let mercury = |radius: f64| {
        Sunlit::new(
            ColdTrapSky {
                greatest_declination: Radians::from(Degrees::new(0.034)),
                periapsis_flux: WattsPerSquareMetre::new(1361.0 / (0.3075 * 0.3075)),
                star_radius: Radians::from(Degrees::new(radius)),
                exposed_fraction: 0.45,
            },
            0.07,
            WattsPerSquareMetre::new(0.02),
        )
    };
    let (disk, point) = (at(&mercury(0.87), 90.0), at(&mercury(0.0), 90.0));
    assert!((disk - 172.07).abs() < 0.5, "{disk}");
    assert!((point - 110.27).abs() < 0.5, "{point}");
    let far = Sunlit::new(
        ColdTrapSky {
            periapsis_flux: WattsPerSquareMetre::new(1361.0 / (12.3 * 12.3)),
            star_radius: Radians::ZERO,
            ..lunar_sky()
        },
        0.11,
        WattsPerSquareMetre::new(0.0),
    );
    let equator = at(&far, 0.0);
    assert!((equator - 110.41).abs() < 0.5, "{equator}");
    // A smooth lunar pole: no zonal water cap.
    let maxima = moon.maxima(&area_points());
    assert!(
        maxima.windows(2).all(|w| w[1].1 <= w[0].1),
        "falls toward the pole"
    );
    assert_same_bits(area_below(&maxima, threshold(Gas::Water)), 0.0);
    eprintln!(
        "T_max: Moon {pole} K, 85° {eighty_five} K; Mercury {disk} K ({point} K); 12.3 au {equator} K"
    );
}

/// The annual-mean profile's area points, x = 0, (k + ½) ÷ 64 and 1 (P14.T24.a's).
fn area_points() -> Vec<(f64, f64)> {
    let n = f64::from(ZONAL_AREA_POINTS);
    core::iter::once(0.0)
        .chain((0..ZONAL_AREA_POINTS).map(|k| (f64::from(k) + 0.5) / n))
        .chain(core::iter::once(1.0))
        .map(|x| (x, 0.0))
        .collect()
}

/// The latitude rule's 24 nodes within 5 × 10⁻⁴ of a midpoint rule at 0.02° (−3.99 × 10⁻⁴ measured
/// with B, +1.07 × 10⁻⁴ with A), for the Moon's water traps under craters of distribution B and A;
/// nothing lies below 60°.
#[test]
fn the_latitude_rule_is_within_its_error_of_a_fine_midpoint_rule() {
    let ground = real_moon();
    let shadows = ground.shadows().unwrap();
    let water = threshold(Gas::Water);
    for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
        let terrain = lunar_terrain(depths);
        let rule = ground.trap_fraction(&shadows, &terrain, water);
        let fine = midpoint(
            |latitude| cold_trap::fraction(latitude, &shadows, &terrain, water),
            (60.0, 90.0),
            0.02,
        );
        let error = rule / fine - 1.0;
        eprintln!("{depths:?}: the rule {rule:e}, the midpoint {fine:e}, error {error:e}");
        assert!(error.abs() < 5e-4, "{depths:?}: {error:e}");
        assert_same_bits(band(&ground, &terrain, water, (0.0, 60.0)), 0.0);
    }
    // The rule integrates cos φ, the hemisphere's area, to 1.
    let area = over_latitude(|_| 1.0);
    assert!((area - 1.0).abs() < 1e-9, "{area}");
}

/// The Moon's cold traps are 0.024–0.2% of its surface and 1.6–13% of 80–90°, with almost none
/// below 70° (plan 14's bounds: half of Williams et al. 2019's Diviner-mapped 0.048% and 3.2% to
/// twice Hayne et al.'s point-source 0.10% and 6.7%); the research's figures are B 0.092% and
/// 5.84%, A 0.059% and 3.87%.
#[test]
fn the_moon_s_cold_traps_lie_within_the_plan_s_bands() {
    let ground = real_moon();
    let shadows = ground.shadows().unwrap();
    let water = threshold(Gas::Water);
    let polar_area = 1.0 - math::sin(Radians::from(Degrees::new(80.0)).value());
    for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
        let terrain = lunar_terrain(depths);
        let whole = ground.trap_fraction(&shadows, &terrain, water);
        let polar = band(&ground, &terrain, water, (80.0, 90.0)) / polar_area;
        let low = band(&ground, &terrain, water, (0.0, 70.0));
        eprintln!(
            "{depths:?}: whole {:.4}%, 80–90° {:.3}%, below 70° {low:e}",
            100.0 * whole,
            100.0 * polar
        );
        assert!((0.000_24..=0.002).contains(&whole), "{depths:?}: {whole}");
        assert!((0.016..=0.13).contains(&polar), "{depths:?}: {polar}");
        assert!(low <= 0.01 * whole, "{depths:?}: {low} of {whole}");
    }
}

/// The Moon's exposed water at Li et al.'s exposed fraction 0.035 (PNAS 115, 8907) is within a
/// factor of two of theirs: 3.5% of the cold traps, over Diviner's 18,300 km² (Williams et al.
/// 2019's 0.048%) and over Hayne et al.'s all-scale 40,000 km².
#[test]
fn the_moon_s_exposed_water_is_within_two_of_li_et_al() {
    let ground = real_moon();
    let shadows = ground.shadows().unwrap();
    let water = threshold(Gas::Water);
    let f_x = lunar_sky().exposed_fraction;
    for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
        let terrain = lunar_terrain(depths);
        let exposed = f_x * ground.trap_fraction(&shadows, &terrain, water) * MOON_KM2;
        for reference in [18_300.0, 40_000.0] {
            let ratio = exposed / (f_x * reference);
            eprintln!("{depths:?}: {exposed:.0} km² exposed, ×{ratio:.2} of {reference} km²'s");
            assert!((0.5..=2.0).contains(&ratio), "{depths:?}: ×{ratio}");
        }
    }
}

/// At the temperature T49.b's curve gives carbon dioxide for Schorghofer et al. 2021's 10 kg m⁻²
/// Gyr⁻¹ (55.6 K on Bryson et al. 1974's), half the Moon's CO₂ cold-trap area lies within 30–408
/// km² (half of their peak-rate 59 km² to twice their time-averaged 204 km², GRL 48, e2021GL095533,
/// Table 1); the research gets 198 and 191. The area at the rule's own threshold, the registry's
/// 58.18 K at 144 kg m⁻² Gyr⁻¹ (the research's 280 and 266 km² at its estimated 57 K), is printed,
/// not asserted.
#[test]
fn half_the_moon_s_co2_traps_at_10_kg_lie_in_schorghofer_s_bracket() {
    let ground = real_moon();
    let shadows = ground.shadows().unwrap();
    let strict = threshold_at(Gas::CarbonDioxide, 10.0);
    let rule = threshold(Gas::CarbonDioxide);
    assert!(strict < rule, "{strict:?} against {rule:?}");
    for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
        let terrain = lunar_terrain(depths);
        let half = |t: Kelvin| 0.5 * MOON_KM2 * ground.trap_fraction(&shadows, &terrain, t);
        let (at_10, at_144) = (half(strict), half(rule));
        eprintln!(
            "{depths:?}: CO₂ at {:.2} K {at_10:.0} km²; at {:.2} K {at_144:.0} km²",
            strict.value(),
            rule.value()
        );
        assert!((30.0..=408.0).contains(&at_10), "{depths:?}: {at_10}");
    }
}

/// Ceres's permanent shadow (`hyperion_surface::cold_trap::permanent_shadow`, at its perihelion's
/// flux and the Sun's 0.1042°, with Hayne et al.'s lunar terrain) holds Schorghofer et al. 2024's
/// 2,282 km² at a 4° greatest declination between craters of distribution B and A, and at 20°
/// under 68 km² with either (PSJ 5, 99: "essentially zero"), by the same latitude rule.
#[test]
fn ceres_s_permanent_shadow_lies_between_b_and_a_by_the_rule() {
    let ceres_km2 = 4.0 * PI * 469.7 * 469.7;
    let ceres = |tilt: f64| {
        Sunlit::new(
            ColdTrapSky {
                greatest_declination: Radians::from(Degrees::new(tilt)),
                periapsis_flux: WattsPerSquareMetre::new(1361.0 / (2.5577 * 2.5577)),
                star_radius: Radians::from(Degrees::new(0.1042)),
                exposed_fraction: 0.016,
            },
            0.12,
            WattsPerSquareMetre::new(0.0),
        )
        .shadows()
        .unwrap()
    };
    let shadow_km2 = |tilt: f64, depths: DepthRatios| {
        let (body, terrain) = (ceres(tilt), lunar_terrain(depths));
        ceres_km2 * over_latitude(|latitude| permanent_shadow(latitude, &body, &terrain))
    };
    let (b, a) = (
        shadow_km2(4.0, DepthRatios::HAYNE_B),
        shadow_km2(4.0, DepthRatios::HAYNE_A),
    );
    eprintln!("Ceres at 4°: B {b:.0} km², A {a:.0} km²");
    assert!(b <= 2_282.0 && 2_282.0 <= a, "B {b} km², A {a} km²");
    for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
        let tilted = shadow_km2(20.0, depths);
        eprintln!("Ceres at 20°: {depths:?} {tilted:.1} km²");
        assert!(tilted < 68.0, "{depths:?} at 20°: {tilted} km²");
    }
}

/// An airless body whose star reaches 30° from its equator has no cold traps of water or carbon
/// dioxide, with either crater distribution.
#[test]
fn an_airless_body_at_30_degrees_obliquity_has_none() {
    let tilted = Sunlit::new(
        ColdTrapSky {
            greatest_declination: Radians::from(Degrees::new(30.0)),
            ..lunar_sky()
        },
        0.12,
        WattsPerSquareMetre::new(0.018),
    );
    let shadows = tilted.shadows().unwrap();
    for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
        let terrain = lunar_terrain(depths);
        for gas in DELIVERED {
            let trapped = tilted.trap_fraction(&shadows, &terrain, threshold(gas));
            assert_same_bits(trapped, 0.0);
        }
    }
}

/// A body whose zonal annual maximum lies below water's threshold everywhere (a rocky equator at
/// 20 au) lays its water over its whole surface, capped by its reservoir at the 1.93 km sheet, and
/// has no cold traps: they lie only where the maximum reaches the threshold.
#[test]
fn a_body_colder_than_its_threshold_everywhere_lays_a_whole_cover() {
    let far = Sunlit::new(
        ColdTrapSky {
            periapsis_flux: WattsPerSquareMetre::new(1361.0 / (20.0 * 20.0)),
            ..lunar_sky()
        },
        0.11,
        WattsPerSquareMetre::new(0.01),
    );
    let water = threshold(Gas::Water);
    let maxima = far.maxima(&area_points());
    assert_same_bits(area_below(&maxima, water), 1.0);
    let shadows = far.shadows().unwrap();
    let trapped = far.trap_fraction(&shadows, &lunar_terrain(DepthRatios::HAYNE_B), water);
    assert_same_bits(trapped, 0.0);
    let warm_below = |t: Kelvin| area_below(&maxima, t);
    let g = MetresPerSecondSquared::new(1.0);
    let climate =
        ClimateSpan::new(Kelvin::new(40.0), Kelvin::new(50.0), Kelvin::new(60.0)).unwrap();
    for (column_pa, whole) in [(100.0, false), (1e7, true)] {
        let inventory =
            ColumnInventory::new([(Gas::Water.substance(), Pascals::new(column_pa))]).unwrap();
        let solved = partition(&climate, &Ground::without_basins(g), &inventory);
        let published = solved.publish_with(
            &Cover {
                cold_area: &warm_below,
                state: SurfaceState::Airless,
                water: WaterCover {
                    surface_water: KilogramsPerSquareMetre::ZERO,
                    potential_ocean: 0.0,
                    ice_fraction: 0.0,
                    ice_density: KilogramsPerCubicMetre::new(917.0),
                    crust_water: false,
                },
            },
            SolidEdges::Exosphere {
                water_keeps_cover: false,
            },
        );
        let ice = published
            .entry(Gas::Water.substance(), CondensatePhase::Solid)
            .unwrap();
        let reservoir = solved
            .condensate(Gas::Water.substance())
            .unwrap()
            .reservoir()
            .value();
        let layer = ice.reservoir_gel().unwrap().value();
        if whole {
            assert_same_bits(ice.area_fraction(), 1.0);
            assert!((layer - 1_930.0).abs() < 1e-9, "{layer}");
            assert!(ice.reservoir().value() < reservoir);
        } else {
            assert!((ice.area_fraction() - layer / 1_930.0).abs() < 1e-15);
            assert!(ice.area_fraction() < 1.0);
            assert_same_bits(ice.reservoir().value(), reservoir);
        }
    }
}

/// The merge adds a trap's area, reservoir and layer to its substance's solid entry, keeping that
/// entry's seasonal flag, inserts a new one as perennial, skips an empty trap, and keeps P14.T51.a's
/// order, held to its eight entries; its trapped part is none in a zonal-only entry, the trap's area
/// in a merged one and the whole area in an inserted one, and held to 1 the trap takes what the
/// zonal part leaves (P14.T51.i).
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the merge's cases: summed, inserted, skipped, held, ordered and capped"
)]
fn the_merge_keeps_t51a_s_order() {
    let co2 = Gas::CarbonDioxide.substance();
    let climate =
        ClimateSpan::new(Kelvin::new(147.0), Kelvin::new(214.0), Kelvin::new(242.0)).unwrap();
    let inventory = ColumnInventory::new([
        (co2, Pascals::new(1_200.0)),
        (Gas::Nitrogen.substance(), Pascals::new(11.0)),
        (Gas::Argon.substance(), Pascals::new(12.0)),
    ])
    .unwrap();
    let mars = partition(
        &climate,
        &Ground::without_basins(MetresPerSecondSquared::new(3.72)),
        &inventory,
    );
    let cold_area = |t: Kelvin| ((t.value() - 150.0) / 75.0).clamp(0.0, 1.0);
    let published = mars.publish(&Cover {
        cold_area: &cold_area,
        state: SurfaceState::Temperate,
        water: WaterCover {
            surface_water: KilogramsPerSquareMetre::ZERO,
            potential_ocean: 0.0,
            ice_fraction: 0.0,
            ice_density: KilogramsPerCubicMetre::new(917.0),
            crust_water: false,
        },
    });
    let caps = *published.entry(co2, CondensatePhase::Solid).unwrap();
    let trap = |gas: Gas, kg: f64, area: f64| TrappedIce {
        substance: gas.substance(),
        reservoir: KilogramsPerSquareMetre::new(kg),
        area_fraction: area,
        threshold: exosphere_threshold(gas.substance(), AIRLESS_ICE_LOSS_KG_PER_M2_GYR)
            .unwrap_or(Kelvin::new(20.0)),
    };
    let merged = published.clone().with_cold_traps(&[
        trap(Gas::CarbonDioxide, 2.0, 1e-4),
        trap(Gas::Water, 0.5, 3e-5),
        trap(Gas::Methane, 0.0, 1e-3),
        trap(Gas::Ammonia, 1.0, 0.0),
    ]);
    let both = merged.entry(co2, CondensatePhase::Solid).unwrap();
    assert_same_bits(both.area_fraction(), caps.area_fraction() + 1e-4);
    // The trapped part (P14.T51.i): none in a zonal-only entry, the trap's area in a merged one.
    assert_same_bits(caps.trapped_area_fraction(), 0.0);
    assert_same_bits(both.trapped_area_fraction(), 1e-4);
    assert_same_bits(both.reservoir().value(), caps.reservoir().value() + 2.0);
    let rho = co2
        .substance()
        .grain_density(
            crate::substance::CondensedPhase::Solid,
            threshold(Gas::CarbonDioxide),
        )
        .unwrap()
        .value();
    assert_same_bits(
        both.reservoir_gel().unwrap().value(),
        caps.reservoir_gel().unwrap().value() + 2.0 / rho,
    );
    assert_eq!(both.is_seasonal(), caps.is_seasonal());
    let ice = merged
        .entry(Gas::Water.substance(), CondensatePhase::Solid)
        .unwrap();
    assert!(!ice.is_seasonal());
    assert_same_bits(ice.area_fraction(), 3e-5);
    // An inserted trap's trapped part is its whole area.
    assert_same_bits(ice.trapped_area_fraction(), ice.area_fraction());
    // Held to 1, the zonal part is kept whole and the trap takes what is left.
    let flooded = published
        .clone()
        .with_cold_traps(&[trap(Gas::CarbonDioxide, 2.0, 1.0)]);
    let full_cap = flooded.entry(co2, CondensatePhase::Solid).unwrap();
    assert_same_bits(full_cap.area_fraction(), 1.0);
    assert_same_bits(full_cap.trapped_area_fraction(), 1.0 - caps.area_fraction());
    assert_eq!(
        merged.entry(Gas::Methane.substance(), CondensatePhase::Solid),
        None
    );
    assert_eq!(
        merged.entry(Gas::Ammonia.substance(), CondensatePhase::Solid),
        None
    );
    assert_eq!(merged.entries().len(), published.entries().len() + 1);
    let ordered = |c: &Condensates| {
        c.entries().windows(2).all(|w| {
            let (a, b) = (&w[0], &w[1]);
            b.reservoir()
                .total_cmp(&a.reservoir())
                .then(a.substance().cmp(&b.substance()))
                .then(a.phase().cmp(&b.phase()))
                .is_lt()
        })
    };
    assert!(ordered(&merged));
    // Held to eight, the smallest dropped.
    let many: Vec<TrappedIce> = Gas::ALL
        .iter()
        .enumerate()
        .map(|(i, &gas)| {
            trap(
                gas,
                1e-6 * (1.0 + f64::from(u8::try_from(i).unwrap())),
                1e-6,
            )
        })
        .collect();
    let full = merged.with_cold_traps(&many);
    assert_eq!(
        full.entries().len(),
        crate::planetary::surface::MAX_CONDENSATES
    );
    assert!(ordered(&full));
}

/// The record path under the batch: the table's airless Moon publishes its water and carbon dioxide
/// cold traps, `f_ct` × `f_x` of its surface over its reservoirs' placeholder columns, the same twice,
/// and none held.
#[test]
fn the_record_path_publishes_the_moon_s_cold_traps_under_the_batch() {
    let bodies = historic_table();
    let (_, moon) = bodies.iter().find(|(name, _)| *name == "Moon").unwrap();
    let figures = table_figures(moon).unwrap();
    let inputs = SurfaceInputs {
        heat_flow: figures.heat_flow(),
        ocean_weight: figures.ocean_weight(),
        ..inputs_of(moon, spin_of("Moon"))
    };
    let conditions = SurfaceConditions::derive(&inputs).unwrap();
    let (air, g) = (moon.atmosphere(), moon.surface_gravity());
    let sky = lunar_sky();
    let table = conditions.zonal_insolation();
    let under = |traps| {
        figures
            .condensates_under(
                air,
                &conditions,
                g,
                &RecordColdTraps::at_the_albedo_of(traps, &sky, air),
                &table,
            )
            .1
    };
    let (held, batch) = (under(ColdTrapRules::Held), under(ColdTrapRules::Batch));
    assert_eq!(
        format!("{batch:?}"),
        format!("{:?}", under(ColdTrapRules::Batch))
    );
    let ground = Sunlit::new(sky, air.albedo().value(), figures.heat_flow());
    let shadows = ground.shadows().unwrap();
    let terrain = terrain_of(figures.craters());
    for gas in DELIVERED {
        let substance = gas.substance();
        assert_eq!(
            held.entry(substance, CondensatePhase::Solid),
            None,
            "{gas:?}"
        );
        let trapped = ground.trap_fraction(&shadows, &terrain, threshold(gas));
        let ice = batch.entry(substance, CondensatePhase::Solid).unwrap();
        assert_same_bits(ice.area_fraction(), trapped * sky.exposed_fraction);
        assert_same_bits(ice.trapped_area_fraction(), ice.area_fraction());
        assert_same_bits(
            ice.reservoir().value(),
            delivered_column(substance).value() * trapped,
        );
        assert!(!ice.is_seasonal());
        eprintln!(
            "the table's Moon: {gas:?} f_ct {trapped:e}, area {:e}, reservoir {:e} kg m⁻², layer \
             {:?}",
            ice.area_fraction(),
            ice.reservoir().value(),
            ice.reservoir_gel()
        );
    }
}

/// The cost of the cold traps per body of the table, in release: the batch's published
/// condensates against the held ones, the best of five interleaved rounds each, Mercury and Ceres
/// in their own skies (plan 14's Risks record it; P14.T51.i's budget, at most 0.6 ms for each
/// qualifying table body, all its species, decision-p14-t51-batch §1.5). A timing alone,
/// asserting nothing.
#[test]
#[ignore = "a timing, run by name in release"]
fn time_the_cold_traps() {
    // Mercury and Ceres in their own skies (NASA's fact sheets, *from memory*: Mercury 0.387 au,
    // e 0.2056, 0.034°; Ceres 2.767 au, e 0.0785, 4°), the Moon in its own, every other body in the
    // Moon's (none of them qualifies).
    let own = |degrees: f64, (a_au, e): (f64, f64)| {
        ColdTrapSky::at_periapsis(
            Radians::from(Degrees::new(degrees)),
            WattsPerSquareMetre::new(1361.0 / (a_au * a_au * (1.0 - e * e).sqrt())),
            (Metres::new(a_au * METRES_PER_AU), e),
            Some(SolarRadii::new(1.0)),
            0.035,
        )
    };
    for (name, body) in historic_table() {
        let sky = match name {
            "Mercury" => own(0.034, (0.387_098, 0.205_63)),
            "Ceres" => own(4.0, (2.767, 0.0785)),
            _ => lunar_sky(),
        };
        let Some(figures) = table_figures(&body) else {
            continue;
        };
        let inputs = SurfaceInputs {
            heat_flow: figures.heat_flow(),
            ocean_weight: figures.ocean_weight(),
            ..inputs_of(&body, spin_of(name))
        };
        let Some(conditions) = SurfaceConditions::derive(&inputs) else {
            continue;
        };
        let (air, g) = (body.atmosphere(), body.surface_gravity());
        // Built once, outside the timed rounds, as a record builds its body's table.
        let table = conditions.zonal_insolation();
        let n = 200_u32;
        let mut each = [f64::INFINITY; 2];
        let mut entries = 0;
        // The best of five rounds of each, interleaved, so that the machine's load moves both.
        for _ in 0..5 {
            for (k, traps) in [ColdTrapRules::Held, ColdTrapRules::Batch]
                .into_iter()
                .enumerate()
            {
                let start = std::time::Instant::now();
                for _ in 0..n {
                    let (_, published) = figures.condensates_under(
                        air,
                        &conditions,
                        g,
                        &RecordColdTraps::at_the_albedo_of(traps, &sky, air),
                        &table,
                    );
                    entries += published.entries().len();
                }
                let ms = start.elapsed().as_secs_f64() / f64::from(n) * 1e3;
                if ms < each[k] {
                    each[k] = ms;
                }
            }
        }
        eprintln!(
            "{name}: held {:.4} ms, batch {:.4} ms, cold traps {:.4} ms ({entries})",
            each[0],
            each[1],
            each[1] - each[0]
        );
    }
}

/// The table's Moon and its figures under the batch, whose crater contract the record path's
/// terrain reads ([`terrain_of`]).
fn table_moon() -> (
    crate::planetary::derive::DerivedBody,
    crate::planetary::hooks::figures::GlobalFigures,
) {
    let (_, moon) = historic_table()
        .into_iter()
        .find(|(name, _)| *name == "Moon")
        .unwrap();
    let figures = table_figures(&moon).unwrap();
    (moon, figures)
}

/// The parts of a ground under [`lunar_sky`]'s sky, over the crater contract `craters`: the Moon's
/// 1.54°, the flux at Earth's perihelion as the orbit-mean flux of an orbit of eccentricity 0, so
/// that `S_p` is the sky's to the bit, the Sun's 0.2711° there, and [`real_moon`]'s albedo 0.12 and
/// heat flow 0.018 W m⁻².
fn lunar_parts(craters: &CraterParams) -> ColdTrapGroundParts<'_> {
    ColdTrapGroundParts {
        obliquity: Radians::from(Degrees::new(1.54)),
        orbit_mean_flux: WattsPerSquareMetre::new(1361.0 / (0.98329 * 0.98329)),
        eccentricity: 0.0,
        star_radius: Radians::from(Degrees::new(0.2711)),
        bond_albedo: 0.12,
        heat_flow: WattsPerSquareMetre::new(0.018),
        craters,
    }
}

/// The cold-trap fraction of `threshold` over the surface, by the latitude integral of
/// `ground`'s [`ColdTrapGround::fractions_at`] at that threshold alone.
fn integral_of(ground: &ColdTrapGround, threshold: Kelvin) -> f64 {
    over_latitude(|latitude| {
        let mut fraction = [f64::NAN];
        ground.fractions_at(latitude, &[threshold], &mut fraction);
        fraction[0]
    })
}

/// Latitudes every half degree from pole to pole, rad, with the exact poles and both zeros.
fn pole_to_pole() -> Vec<Radians> {
    (0..=360)
        .map(|i| Radians::from(Degrees::new(-90.0 + 0.5 * f64::from(i))))
        .filter(|latitude| latitude.value().abs() < FRAC_PI_2)
        .chain([-FRAC_PI_2, -0.0, 0.0, FRAC_PI_2].map(Radians::new))
        .collect()
}

/// The record path's cold traps are the latitude integral of the ground's fractions: the table's
/// Moon publishes `f_x` times that integral of a ground built from its obliquity, flux and
/// eccentricity, to the bit, and so does a sky taken to periapsis on an eccentric orbit (Mercury's)
/// and on a retrograde one, whose zonal maxima are the ground's too.
#[test]
fn the_trap_fraction_is_the_latitude_integral_of_fractions_at() {
    let (moon, figures) = table_moon();
    let inputs = SurfaceInputs {
        heat_flow: figures.heat_flow(),
        ocean_weight: figures.ocean_weight(),
        ..inputs_of(&moon, spin_of("Moon"))
    };
    let conditions = SurfaceConditions::derive(&inputs).unwrap();
    let (air, g) = (moon.atmosphere(), moon.surface_gravity());
    let sky = lunar_sky();
    let (_, batch) = figures.condensates_under(
        air,
        &conditions,
        g,
        &RecordColdTraps::at_the_albedo_of(ColdTrapRules::Batch, &sky, air),
        &conditions.zonal_insolation(),
    );
    let ground = ColdTrapGround::new(&ColdTrapGroundParts {
        bond_albedo: air.albedo().value(),
        heat_flow: figures.heat_flow(),
        ..lunar_parts(figures.craters())
    })
    .unwrap();
    for gas in DELIVERED {
        let trapped = integral_of(&ground, threshold(gas));
        assert!(trapped > 0.0, "{gas:?}: {trapped}");
        let ice = batch
            .entry(gas.substance(), CondensatePhase::Solid)
            .unwrap();
        assert_same_bits(ice.area_fraction(), trapped * sky.exposed_fraction);
    }
    let au = METRES_PER_AU;
    for (degrees, (a_au, e)) in [
        (0.034, (0.387_098, 0.205_630_f64)),
        (178.46, (1.0, 0.016_7)),
    ] {
        let obliquity = Radians::from(Degrees::new(degrees));
        let flux = WattsPerSquareMetre::new(1361.0 / (a_au * a_au * (1.0 - e * e).sqrt()));
        let star = Some(SolarRadii::new(1.0));
        let sky =
            ColdTrapSky::at_periapsis(obliquity, flux, (Metres::new(a_au * au), e), star, 0.1);
        let (albedo, heat_flow) = (0.12, WattsPerSquareMetre::new(0.018));
        let sunlit = Sunlit::new(sky, albedo, heat_flow);
        let shadows = sunlit.shadows().unwrap();
        let terrain = terrain_of(figures.craters());
        let ground = ColdTrapGround::new(&ColdTrapGroundParts {
            obliquity,
            orbit_mean_flux: flux,
            eccentricity: e,
            star_radius: sky.star_radius,
            bond_albedo: albedo,
            heat_flow,
            craters: figures.craters(),
        })
        .unwrap();
        for gas in DELIVERED {
            let t = threshold(gas);
            let trapped = sunlit.trap_fraction(&shadows, &terrain, t);
            assert!(trapped > 0.0, "{degrees}°, {gas:?}: {trapped}");
            assert_same_bits(trapped, integral_of(&ground, t));
        }
        for (x, t) in sunlit.maxima(&area_points()) {
            let latitude = Radians::new(math::asin(x));
            assert_same_bits(t, ground.annual_maximum(latitude).value());
        }
    }
}

/// Several thresholds in one call are one call per threshold, to the bit, in their order: the
/// closed form where the zonal annual maximum reaches the threshold and 0 where it does not, which
/// shuts some ground the closed form alone would trap. Up to the zonal maximum a fraction does not
/// fall as its threshold rises.
#[test]
fn fractions_at_over_several_thresholds_is_one_call_per_threshold() {
    let (_, figures) = table_moon();
    let ground = ColdTrapGround::new(&lunar_parts(figures.craters())).unwrap();
    let thresholds = [
        threshold(Gas::Water),
        Kelvin::new(30.0),
        threshold(Gas::CarbonDioxide),
        Kelvin::new(200.0),
        threshold_at(Gas::CarbonDioxide, 10.0),
        Kelvin::new(158.0),
        Kelvin::new(400.0),
    ];
    let mut all = [f64::NAN; 7];
    let (mut open, mut shut) = (0, 0);
    for latitude in pole_to_pole() {
        ground.fractions_at(latitude, &thresholds, &mut all);
        let maximum = ground.annual_maximum(latitude);
        for (&t, &f) in thresholds.iter().zip(&all) {
            let mut one = [f64::NAN];
            ground.fractions_at(latitude, &[t], &mut one);
            assert_same_bits(f, one[0]);
            let closed = cold_trap::fraction(latitude, &ground.shadows, &ground.terrain, t);
            if maximum >= t {
                assert_same_bits(f, closed);
                open += usize::from(f > 0.0);
            } else {
                assert_same_bits(f, 0.0);
                shut += usize::from(closed > 0.0);
            }
            assert!((0.0..=1.0).contains(&f), "{latitude:?}, {t:?}: {f}");
        }
        for (&low, &f_low) in thresholds.iter().zip(&all) {
            for (&high, &f_high) in thresholds.iter().zip(&all) {
                if low < high && high <= maximum {
                    assert!(
                        f_low <= f_high,
                        "{latitude:?}: {low:?} {f_low} > {high:?} {f_high}"
                    );
                }
            }
        }
    }
    assert!(open > 0 && shut > 0, "open {open}, shut by the gate {shut}");
    ground.fractions_at(Radians::ZERO, &[], &mut []);
}

/// A southern latitude reads its northern mirror: the fractions and the zonal annual maximum at −φ
/// are φ's, to the bit, from pole to pole.
#[test]
fn the_cold_traps_are_symmetric_about_the_equator() {
    let (_, figures) = table_moon();
    let ground = ColdTrapGround::new(&lunar_parts(figures.craters())).unwrap();
    let thresholds = [threshold(Gas::Water), threshold(Gas::CarbonDioxide)];
    let (mut north, mut south) = ([f64::NAN; 2], [f64::NAN; 2]);
    let mut trapped = 0;
    for latitude in pole_to_pole() {
        let mirror = Radians::new(-latitude.value());
        ground.fractions_at(latitude, &thresholds, &mut north);
        ground.fractions_at(mirror, &thresholds, &mut south);
        for (n, s) in north.iter().zip(&south) {
            assert_same_bits(*n, *s);
            trapped += usize::from(*n > 0.0);
        }
        assert_same_bits(
            ground.annual_maximum(latitude).value(),
            ground.annual_maximum(mirror).value(),
        );
    }
    assert!(trapped > 0, "{trapped}");
}

/// On a rocky body at 40 au the zonal annual maximum lies below water's threshold at every
/// latitude, so its water takes no cold traps anywhere, though the closed form alone would trap
/// its polar shadows: the zonal ice covers that ground.
#[test]
fn a_rocky_body_at_40_au_has_no_water_traps_at_any_latitude() {
    let (_, figures) = table_moon();
    let ground = ColdTrapGround::new(&ColdTrapGroundParts {
        orbit_mean_flux: WattsPerSquareMetre::new(1361.0 / (40.0 * 40.0)),
        star_radius: Radians::new(math::asin(SOLAR_RADIUS_M / (40.0 * METRES_PER_AU))),
        bond_albedo: 0.11,
        heat_flow: WattsPerSquareMetre::new(0.0119),
        ..lunar_parts(figures.craters())
    })
    .unwrap();
    let water = threshold(Gas::Water);
    let equator = ground.annual_maximum(Radians::ZERO);
    assert!(equator < water, "{equator:?}");
    for latitude in pole_to_pole() {
        assert!(ground.annual_maximum(latitude) <= equator, "{latitude:?}");
        let mut fraction = [f64::NAN];
        ground.fractions_at(latitude, &[water], &mut fraction);
        assert_same_bits(fraction[0], 0.0);
    }
    assert_same_bits(integral_of(&ground, water), 0.0);
    let pole = Radians::new(FRAC_PI_2);
    assert!(cold_trap::fraction(pole, &ground.shadows, &ground.terrain, water) > 0.0);
    eprintln!("40 au: the equator's zonal annual maximum {equator:?}");
}

/// The ground's zonal annual maximum gives P14.T51.f's figures to 0.01 K, by the record's own
/// arithmetic to the bit: the Moon's pole 158.13 K and 85° 226.87 K at the sim's albedo 0.11 and
/// heat flow 0.0119 W m⁻², and Mercury's pole 172.07 K with the Sun's 0.87° disk and 110.27 K as a
/// point.
#[test]
fn the_ground_s_annual_maximum_gives_t51f_s_figures() {
    let (_, figures) = table_moon();
    let craters = figures.craters();
    let (albedo, heat_flow) = (0.11, WattsPerSquareMetre::new(0.0119));
    let moon = ColdTrapGround::new(&ColdTrapGroundParts {
        bond_albedo: albedo,
        heat_flow,
        ..lunar_parts(craters)
    })
    .unwrap();
    let record = Sunlit::new(lunar_sky(), albedo, heat_flow);
    let eighty_five = Radians::from(Degrees::new(85.0));
    for (latitude, expected) in [(Radians::new(FRAC_PI_2), 158.13), (eighty_five, 226.87)] {
        let t = moon.annual_maximum(latitude).value();
        assert!((t - expected).abs() < 0.01, "{latitude:?}: {t} K");
        let colatitude = Radians::new(FRAC_PI_2 - latitude.value());
        assert_same_bits(t, record.annual_maximum(colatitude).value());
    }
    for (radius, expected) in [(0.87, 172.07), (0.0, 110.27)] {
        let mercury = ColdTrapGround::new(&ColdTrapGroundParts {
            obliquity: Radians::from(Degrees::new(0.034)),
            orbit_mean_flux: WattsPerSquareMetre::new(1361.0 / (0.3075 * 0.3075)),
            eccentricity: 0.0,
            star_radius: Radians::from(Degrees::new(radius)),
            bond_albedo: 0.07,
            heat_flow: WattsPerSquareMetre::new(0.02),
            craters,
        })
        .unwrap();
        let pole = mercury.annual_maximum(Radians::new(FRAC_PI_2)).value();
        assert!((pole - expected).abs() < 0.01, "α {radius}°: {pole} K");
    }
}

/// The cold-trap threshold is the exosphere's at the airless loss, to the bit, for water and carbon
/// dioxide, within 1 K of 110 K and at the registry's 58.18 K; helium, with no phase diagram, has
/// none.
#[test]
fn the_trap_threshold_is_the_exosphere_threshold_at_the_airless_loss() {
    for gas in DELIVERED {
        let t = trap_threshold(gas.substance()).unwrap();
        let exosphere =
            exosphere_threshold(gas.substance(), AIRLESS_ICE_LOSS_KG_PER_M2_GYR).unwrap();
        assert_same_bits(t.value(), exosphere.value());
    }
    let water = trap_threshold(Gas::Water.substance()).unwrap().value();
    assert!((water - 110.0).abs() < 1.0, "{water}");
    let co2 = trap_threshold(Gas::CarbonDioxide.substance())
        .unwrap()
        .value();
    assert!((co2 - 58.18).abs() < 0.01, "{co2}");
    assert_eq!(trap_threshold(Gas::Helium.substance()), None);
}

/// The Moon's ground under [`lunar_parts`], for the panics.
fn lunar_ground() -> ColdTrapGround {
    let (_, figures) = table_moon();
    ColdTrapGround::new(&lunar_parts(figures.craters())).unwrap()
}

/// `fractions_at` refuses a buffer of another length than its thresholds.
#[test]
#[should_panic(expected = "the thresholds and their fractions must be as many")]
fn fractions_at_refuses_lengths_that_differ() {
    let thresholds = [threshold(Gas::Water), threshold(Gas::CarbonDioxide)];
    lunar_ground().fractions_at(Radians::ZERO, &thresholds, &mut [0.0]);
}

/// `fractions_at` refuses a latitude past the north pole.
#[test]
#[should_panic(expected = "a latitude must be in [-π/2, π/2]")]
fn fractions_at_refuses_a_latitude_past_the_pole() {
    let past = Radians::new(FRAC_PI_2.next_up());
    lunar_ground().fractions_at(past, &[threshold(Gas::Water)], &mut [0.0]);
}

/// `annual_maximum` refuses a latitude past the south pole.
#[test]
#[should_panic(expected = "a latitude must be in [-π/2, π/2]")]
fn annual_maximum_refuses_a_latitude_past_the_south_pole() {
    let _ = lunar_ground().annual_maximum(Radians::new((-FRAC_PI_2).next_down()));
}

/// `fractions_at` refuses a threshold that is not finite, even where the zonal maximum's gate
/// would shut it.
#[test]
#[should_panic(expected = "a threshold must be finite and positive")]
fn fractions_at_refuses_a_threshold_that_is_not_finite() {
    let thresholds = [threshold(Gas::Water), Kelvin::new(f64::NAN)];
    lunar_ground().fractions_at(Radians::ZERO, &thresholds, &mut [0.0; 2]);
}

/// A ground the closed form refuses has no cold traps: a bare ground of albedo 1 absorbs nothing,
/// and a star of no flux gives no light, so `new` answers `None` for both.
#[test]
fn a_ground_the_closed_form_refuses_has_no_cold_traps() {
    let (_, figures) = table_moon();
    let craters = figures.craters();
    let white = ColdTrapGroundParts {
        bond_albedo: 1.0,
        ..lunar_parts(craters)
    };
    assert!(ColdTrapGround::new(&white).is_none());
    let dark = ColdTrapGroundParts {
        orbit_mean_flux: WattsPerSquareMetre::new(0.0),
        ..lunar_parts(craters)
    };
    assert!(ColdTrapGround::new(&dark).is_none());
}

/// `new` refuses an eccentricity of 1: a caller's fault, not a body without cold traps.
#[test]
#[should_panic(expected = "an eccentricity must be in [0, 1)")]
fn a_ground_refuses_an_eccentricity_of_one() {
    let (_, figures) = table_moon();
    let _ = ColdTrapGround::new(&ColdTrapGroundParts {
        eccentricity: 1.0,
        ..lunar_parts(figures.craters())
    });
}

/// `new` refuses a heat flow that is not a number.
#[test]
#[should_panic(expected = "a heat flow must be finite and non-negative")]
fn a_ground_refuses_a_heat_flow_that_is_not_a_number() {
    let (_, figures) = table_moon();
    let _ = ColdTrapGround::new(&ColdTrapGroundParts {
        heat_flow: WattsPerSquareMetre::new(f64::NAN),
        ..lunar_parts(figures.craters())
    });
}

/// P14.T51.i: a field's `f_ct` is the latitude integral of the ground's `fractions_at` at each
/// threshold alone, to the bit (decision-r09-t13f-cold-traps §7.4), for every condensable a body is
/// tested for, one without a threshold left out; its maxima are the ground's at the area points.
#[test]
fn the_field_s_f_ct_is_the_latitude_integral_of_fractions_at() {
    let (_, figures) = table_moon();
    let sunlit = Sunlit::new(lunar_sky(), 0.12, WattsPerSquareMetre::new(0.018));
    let terrain = terrain_of(figures.craters());
    let shadows = sunlit.shadows().unwrap();
    let species: Vec<SubstanceId> = [
        Gas::Helium,
        Gas::Methane,
        Gas::Ammonia,
        Gas::Nitrogen,
        Gas::CarbonDioxide,
        Gas::Water,
    ]
    .iter()
    .map(|gas| gas.substance())
    .collect();
    let field = ColdTrapField::new(&sunlit, terrain, &area_points(), &species);
    let mut trapping = 0;
    for &substance in &species {
        let Some(t) = trap_threshold(substance) else {
            assert!(field.trap(substance).is_none(), "{substance:?}");
            continue;
        };
        let (threshold, f_ct) = field.trap(substance).unwrap();
        assert_same_bits(threshold.value(), t.value());
        assert_same_bits(f_ct, sunlit.trap_fraction(&shadows, &terrain, t));
        trapping += usize::from(f_ct > 0.0);
        eprintln!("the field's {substance:?}: {t:?}, f_ct {f_ct:e}");
    }
    assert!(trapping >= 2, "{trapping}");
    let maxima = sunlit.maxima(&area_points());
    assert_eq!(field.maxima().len(), maxima.len());
    for (&(x, t), &(y, u)) in field.maxima().iter().zip(&maxima) {
        assert_same_bits(x, y);
        assert_same_bits(t, u);
    }
}

/// P14.T51.i: `fractions_at` through the shared form is one closed-form fraction per threshold with
/// the gate, to the bit (decision-r09-t13f-cold-traps §7.4), over the table's Moon, a Mercury and
/// a Ceres, every degree from pole to pole, and thresholds of 20–200 K with water's and carbon
/// dioxide's.
#[test]
fn fractions_at_through_the_shared_form_is_one_fraction_per_threshold_with_the_gate() {
    let (_, figures) = table_moon();
    let craters = figures.craters();
    let au = METRES_PER_AU;
    let periapsis = |a_au: f64, e: f64| {
        let (a, near) = (a_au * au, 1.0 - e);
        Radians::new(math::asin(6.957e8 / (a * near)))
    };
    let grounds = [
        lunar_parts(craters),
        ColdTrapGroundParts {
            obliquity: Radians::from(Degrees::new(0.034)),
            orbit_mean_flux: WattsPerSquareMetre::new(
                1361.0 / (0.387_098 * 0.387_098 * (1.0 - 0.205_63 * 0.205_63_f64).sqrt()),
            ),
            eccentricity: 0.205_63,
            star_radius: periapsis(0.387_098, 0.205_63),
            bond_albedo: 0.11,
            heat_flow: WattsPerSquareMetre::new(0.02),
            craters,
        },
        ColdTrapGroundParts {
            obliquity: Radians::from(Degrees::new(4.0)),
            orbit_mean_flux: WattsPerSquareMetre::new(
                1361.0 / (2.767 * 2.767 * (1.0 - 0.0785 * 0.0785_f64).sqrt()),
            ),
            eccentricity: 0.0785,
            star_radius: periapsis(2.767, 0.0785),
            bond_albedo: 0.09,
            heat_flow: WattsPerSquareMetre::new(0.001),
            craters,
        },
    ]
    .map(|parts| ColdTrapGround::new(&parts).unwrap());
    let mut thresholds: Vec<Kelvin> = (2..=20).map(|k| Kelvin::new(10.0 * f64::from(k))).collect();
    thresholds.extend([threshold(Gas::Water), threshold(Gas::CarbonDioxide)]);
    let mut got = vec![f64::NAN; thresholds.len()];
    let mut open = 0;
    for ground in &grounds {
        for i in -90..=90 {
            let latitude = Radians::from(Degrees::new(f64::from(i)));
            ground.fractions_at(latitude, &thresholds, &mut got);
            let north = latitude.abs();
            let maximum = ground
                .sunlit
                .annual_maximum(Radians::new(FRAC_PI_2 - north.value()));
            for (&share, &t) in got.iter().zip(&thresholds) {
                let alone = if maximum >= t {
                    cold_trap::fraction(north, &ground.shadows, &ground.terrain, t)
                } else {
                    0.0
                };
                assert_same_bits(share, alone);
                open += usize::from(alone > 0.0);
            }
        }
    }
    assert!(open > 100, "{open} open fractions");
}

/// The table's Moon on the record path under the batch: its conditions, its figures, its
/// atmosphere and gravity, and its partition and cover as `condensates_under` builds them.
struct RecordMoon {
    figures: crate::planetary::hooks::figures::GlobalFigures,
    conditions: SurfaceConditions,
    air: Atmosphere,
    gravity: MetresPerSecondSquared,
}

impl RecordMoon {
    fn new() -> Self {
        let (moon, figures) = table_moon();
        let inputs = SurfaceInputs {
            heat_flow: figures.heat_flow(),
            ocean_weight: figures.ocean_weight(),
            ..inputs_of(&moon, spin_of("Moon"))
        };
        let conditions = SurfaceConditions::derive(&inputs).unwrap();
        Self {
            figures,
            conditions,
            air: *moon.atmosphere(),
            gravity: moon.surface_gravity(),
        }
    }

    /// Its published condensates under the batch's cold traps, its atmosphere `air` and its
    /// regolith's albedo `regolith`.
    fn published(&self, air: &Atmosphere, regolith: f64, sky: &ColdTrapSky) -> Condensates {
        let traps = RecordColdTraps {
            rules: ColdTrapRules::Batch,
            sky,
            regolith_albedo: regolith,
        };
        self.figures
            .condensates_under(
                air,
                &self.conditions,
                self.gravity,
                &traps,
                &self.conditions.zonal_insolation(),
            )
            .1
    }
}

/// P14.T51.i: the cold traps read the bare regolith's albedo, not the area mix's
/// (decision-p14-t51-batch §6). The table's Moon whose area-mix albedo has risen to 0.6 under a
/// frost keeps, to the bit, the condensates its regolith's 0.11 gives, and a brighter regolith
/// moves them.
#[test]
fn a_body_whose_area_mix_albedo_rises_keeps_the_traps_its_regolith_gives() {
    let moon = RecordMoon::new();
    let sky = lunar_sky();
    let regolith = moon.air.albedo().value();
    let frosted = moon
        .air
        .with_albedo(crate::planetary::derive::BondAlbedo::new(0.6).unwrap());
    let bare = moon.published(&moon.air, regolith, &sky);
    let frost = moon.published(&frosted, regolith, &sky);
    assert_eq!(format!("{frost:?}"), format!("{bare:?}"));
    let water = Gas::Water.substance();
    let ice = bare.entry(water, CondensatePhase::Solid).unwrap();
    assert!(ice.trapped_area_fraction() > 0.0, "{ice:?}");
    let brighter = moon.published(&moon.air, 0.6, &sky);
    assert_ne!(format!("{brighter:?}"), format!("{bare:?}"));
}

/// P14.T51.i: one field, built once a body, serves every publish (decision-p14-t51-batch §6):
/// publishing over it is the record's publish to the bit, under the Moon's atmosphere and under
/// one whose area mix has risen, and a body whose air does not qualify keeps P14.T51.a's publish
/// over it.
#[test]
fn one_field_built_once_serves_every_publish() {
    let moon = RecordMoon::new();
    let sky = lunar_sky();
    let (solved, _) = moon.figures.condensates_under(
        &moon.air,
        &moon.conditions,
        moon.gravity,
        &RecordColdTraps::at_the_albedo_of(ColdTrapRules::Held, &sky, &moon.air),
        &moon.conditions.zonal_insolation(),
    );
    let profile = moon.conditions.cold_profile();
    let cold_area = |t: Kelvin| profile.area_below(t);
    let cover = Cover {
        cold_area: &cold_area,
        state: moon.air.state(),
        water: moon.figures.water_cover(&moon.conditions, &profile),
    };
    let body = |atmosphere| ColdTrapBody {
        atmosphere,
        profile: &profile,
        gravity: moon.gravity,
        heat_flow: moon.figures.heat_flow(),
        crust: moon.figures.impactors().crust(),
        craters: moon.figures.craters(),
        sky: &sky,
        regolith_albedo: moon.air.albedo().value(),
    };
    let field = ColdTrapField::of(&body(&moon.air), &solved).unwrap();
    let frosted = moon
        .air
        .with_albedo(crate::planetary::derive::BondAlbedo::new(0.6).unwrap());
    for air in [&moon.air, &frosted] {
        let over = publish_over(&solved, &cover, &body(air), &field);
        let alone = publish(&solved, &cover, &body(air));
        assert_eq!(format!("{over:?}"), format!("{alone:?}"));
        assert!(
            over.entry(Gas::Water.substance(), CondensatePhase::Solid)
                .is_some()
        );
    }
    let bodies = historic_table();
    let (_, earth) = bodies.iter().find(|(name, _)| *name == "Earth").unwrap();
    let over = publish_over(&solved, &cover, &body(earth.atmosphere()), &field);
    assert_eq!(format!("{over:?}"), format!("{:?}", solved.publish(&cover)));
}
