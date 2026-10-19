//! The surface modifiers' tests (plan 14, P14.T51.d; decision-p14-t51d-weathering's list): the
//! Moon's reference, a magnetised twin, Mercury, the air's and a planet's shielding, the organic
//! cover, an Io's deposits, the units contract, and the tripwire that keeps the module off the
//! record until P14.T54.b.

use std::path::{Path, PathBuf};

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::partition::{
    ClimateSpan, ColumnInventory, Cover, Ground, WaterCover, partition,
};
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::hooks::figures::Crust;
use crate::planetary::hooks::figures::surface_age::resurfacing_time;
use crate::substance::by_key;
use crate::units::KilogramsPerCubicMetre;

fn id(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

fn au(a: f64) -> Metres {
    Metres::new(a * METRES_PER_AU)
}

/// The Moon (NASA's Moon fact sheet, *from memory*: 1.62 m s⁻²), airless and without
/// a field at the reference orbit, on a 3.9 Gyr surface (the Imbrian maria and Nectarian
/// highlands' age, *from memory*), with nothing resurfacing it.
fn moon(condensates: &Condensates) -> ModifierInputs<'_> {
    ModifierInputs {
        host: ModifierHost::sun(),
        orbit: HeliocentricOrbit {
            semi_major_axis: au(1.0),
            eccentricity: LUNAR_REFERENCE_ECCENTRICITY,
        },
        primary: Primary::Star,
        gravity: MetresPerSecondSquared::new(1.62),
        surface_pressure: Pascals::ZERO,
        field: MagneticField::None,
        surface_age: Years::new(3.9e9),
        resurfacing_time: Years::new(f64::INFINITY),
        heat_flow: WattsPerSquareMetre::new(0.018),
        surface_temperature: Kelvin::new(250.0),
        mantle_sulphur: Kilograms::new(1e19),
        condensates,
    }
}

/// Earth as the Moon's planet: 6,371 km, a strong field, the Moon at 384,400 km (*from memory*).
fn earth_magnetosphere() -> PlanetMagnetosphere {
    PlanetMagnetosphere {
        radius: Metres::new(6.371e6),
        field: MagneticField::Strong,
        semi_major_axis: Metres::new(3.844e8),
    }
}

/// Mercury (NASA's Mercury fact sheet, *from memory*: a = 0.387 098 au, e = 0.205 630, 3.70 m
/// s⁻²), airless under its weak dipole, on a 4.0 Gyr surface (*from memory*).
fn mercury(condensates: &Condensates) -> ModifierInputs<'_> {
    ModifierInputs {
        orbit: HeliocentricOrbit {
            semi_major_axis: au(0.387_098),
            eccentricity: 0.205_630,
        },
        gravity: MetresPerSecondSquared::new(3.70),
        field: MagneticField::Weak,
        surface_age: Years::new(4.0e9),
        ..moon(condensates)
    }
}

/// Jupiter (NASA's fact sheet, *from memory*: a = 5.2038 au, e = 0.0487, 71,492 km), its moon at
/// `planetocentric` m, a Galilean moon of 1.8 m s⁻².
fn jovian_moon(condensates: &Condensates, planetocentric: f64) -> ModifierInputs<'_> {
    ModifierInputs {
        orbit: HeliocentricOrbit {
            semi_major_axis: au(5.2038),
            eccentricity: 0.0487,
        },
        primary: Primary::Planet(PlanetMagnetosphere {
            radius: Metres::new(7.1492e7),
            field: MagneticField::Strong,
            semi_major_axis: Metres::new(planetocentric),
        }),
        gravity: MetresPerSecondSquared::new(1.8),
        ..moon(condensates)
    }
}

/// The published condensates of an airless body of surface gravity `gravity` holding the column
/// weights `inventory`, Pa, over a climate from `cold` to `warm` K about `mean` K, its cold area
/// linear between the two (a step at an isothermal climate's temperature).
fn published(
    inventory: &[(&str, f64)],
    (cold, mean, warm): (f64, f64, f64),
    gravity: f64,
) -> Condensates {
    let climate =
        ClimateSpan::new(Kelvin::new(cold), Kelvin::new(mean), Kelvin::new(warm)).unwrap();
    let inventory =
        ColumnInventory::new(inventory.iter().map(|&(k, w)| (id(k), Pascals::new(w)))).unwrap();
    let solved = partition(
        &climate,
        &Ground::without_basins(MetresPerSecondSquared::new(gravity)),
        &inventory,
    );
    let cold_area = |t: Kelvin| {
        if warm > cold {
            ((t.value() - cold) / (warm - cold)).clamp(0.0, 1.0)
        } else if t.value() > warm {
            1.0
        } else {
            0.0
        }
    };
    let water = solved
        .condensate(id("H2O"))
        .map_or(0.0, |c| c.reservoir().value());
    let water = WaterCover {
        surface_water: KilogramsPerSquareMetre::new(water),
        potential_ocean: 0.0,
        ice_fraction: if water > 0.0 { 1.0 } else { 0.0 },
        ice_density: KilogramsPerCubicMetre::new(917.0),
    };
    solved.publish(&Cover {
        cold_area: &cold_area,
        state: SurfaceState::Airless,
        water,
    })
}

/// A Kuiper-belt body's climate, 30–45 K about 40 K (*from memory*: about 40 K at 40 au), and its
/// gravity, 0.6 m s⁻².
const KUIPER_CLIMATE: (f64, f64, f64) = (30.0, 40.0, 45.0);

/// The same body's climate held at its mean, 40 K, on which a lone condensable, held at the mean,
/// keeps its ice the year.
const KUIPER_ISOTHERMAL: (f64, f64, f64) = (40.0, 40.0, 40.0);
const KUIPER_GRAVITY: f64 = 0.6;

/// A Kuiper-belt body's published condensates with its methane alone, which keeps it the year at
/// its isothermal climate: 10⁶ Pa of weight, some 3 km of ice over the whole body.
fn methane_ice() -> Condensates {
    published(&[("CH4", 1e6)], KUIPER_ISOTHERMAL, KUIPER_GRAVITY)
}

/// A Kuiper-belt body at 40 au, airless and without a field, on a 4.0 Gyr surface that nothing
/// resurfaces.
fn kuiper(condensates: &Condensates) -> ModifierInputs<'_> {
    ModifierInputs {
        orbit: HeliocentricOrbit {
            semi_major_axis: au(40.0),
            eccentricity: 0.05,
        },
        gravity: MetresPerSecondSquared::new(KUIPER_GRAVITY),
        surface_age: Years::new(4.0e9),
        heat_flow: WattsPerSquareMetre::new(0.002),
        surface_temperature: Kelvin::new(KUIPER_CLIMATE.1),
        ..moon(condensates)
    }
}

/// The Moon's rate is the reference, 1 to 10⁻¹², as the Sun's own or as Earth's moon outside
/// Earth's magnetopause; its 3.9 Gyr regolith is mature, m ≥ 0.99 (decision-p14-t51d-weathering,
/// "For the lane").
#[test]
fn the_moon_s_rate_is_one_and_its_regolith_mature() {
    let none = Condensates::default();
    for primary in [Primary::Star, Primary::Planet(earth_magnetosphere())] {
        let modifiers = SurfaceModifiers::derive(&ModifierInputs {
            primary,
            ..moon(&none)
        })
        .unwrap();
        let k = modifiers.weathering_rate();
        assert!((k - 1.0).abs() < 1e-12, "{primary:?}: k {k}");
        let w = modifiers.weathering();
        assert!(
            (w.wind() - 1.0).abs() < 1e-12 && (w.dust() - 1.0).abs() < 1e-12,
            "{w:?}"
        );
        let m = modifiers.maturity();
        assert!((0.99..=1.0).contains(&m), "m {m}");
        assert_same_bits(modifiers.organic_cover(), 0.0);
        assert_eq!(modifiers.volcanic_deposits(), []);
        assert_same_bits(modifiers.resurfacing_m_per_yr(), 0.0);
    }
}

/// A magnetised twin of the Moon weathers more slowly, its weak or strong dipole closing most of
/// its surface to the wind; on a 100 Myr surface it is less mature, and at the Moon's age both
/// saturate (the ruling's §2.2 anchors).
#[test]
fn a_magnetised_twin_is_less_weathered() {
    let none = Condensates::default();
    let young = |field| {
        SurfaceModifiers::derive(&ModifierInputs {
            field,
            surface_age: Years::new(1e8),
            ..moon(&none)
        })
        .unwrap()
    };
    let [bare, weak, strong] = [
        MagneticField::None,
        MagneticField::Weak,
        MagneticField::Strong,
    ]
    .map(young);
    assert!(strong.weathering_rate() < weak.weathering_rate());
    assert!(weak.weathering_rate() < bare.weathering_rate());
    assert!(strong.maturity() < weak.maturity() && weak.maturity() < bare.maturity());
    // The dust's term is the field's to keep.
    assert_same_bits(strong.weathering().dust(), bare.weathering().dust());
    eprintln!(
        "a Moon twin's k and m at 100 Myr: none {:.4} {:.4}, weak {:.4} {:.4} (wind {:.4}), \
         strong {:.4} {:.4} (wind {:.4})",
        bare.weathering_rate(),
        bare.maturity(),
        weak.weathering_rate(),
        weak.maturity(),
        weak.weathering().wind(),
        strong.weathering_rate(),
        strong.maturity(),
        strong.weathering().wind(),
    );
    let old = SurfaceModifiers::derive(&ModifierInputs {
        field: MagneticField::Strong,
        ..moon(&none)
    })
    .unwrap();
    assert!(old.maturity() > 0.99, "{old:?}");
}

/// Mercury weathers faster than the Moon, k > 1, its two terms printed. Its dust's term is
/// 0.387^(−2.8) ≈ 14, within 10% of Cintala 1992's "almost 14 times more impact melt" than the
/// Moon's (JGR 97, 947, abstract read), the ruling's check. Braden and Robinson 2013 (JGR Planets
/// 118, 1903, abstract read) find Mercury's optical maturation "up to 4 times faster" than the
/// Moon's: the ruling's shares and laws give about 9, the recorded model difference, pinned here
/// within a factor of 3 above it (plan 14's deferred findings from P14.T51.d).
#[test]
fn mercury_weathers_faster_than_the_moon() {
    let none = Condensates::default();
    let modifiers = SurfaceModifiers::derive(&mercury(&none)).unwrap();
    let k = modifiers.weathering_rate();
    let w = modifiers.weathering();
    eprintln!(
        "Mercury: k {k:.4} = {WIND_SHARE} × wind {:.4} + {DUST_SHARE} × dust {:.4}, m {}",
        w.wind(),
        w.dust(),
        modifiers.maturity()
    );
    assert!(k > 1.0, "k {k}");
    assert!((w.dust() / 14.0 - 1.0).abs() < 0.1, "dust {}", w.dust());
    // Its weak dipole closes part of its surface: an unshielded Mercury's wind term would be its
    // 1 ÷ a² √(1 − e²) over the Moon's.
    let bare = SurfaceModifiers::derive(&ModifierInputs {
        field: MagneticField::None,
        ..mercury(&none)
    })
    .unwrap();
    assert!(w.wind() < bare.weathering().wind(), "{w:?}");
    let ratio = k / 4.0;
    assert!(
        (1.0..3.0).contains(&ratio),
        "Mercury's k {k} against Braden and Robinson's up to 4: the recorded misfit moved"
    );
}

/// Air above the tenuous floor's stand-in stops the wind's ions, and air below it does not; Mars's
/// 636 Pa (NASA's Mars fact sheet, as the partition's tests read it) at 3.73 m s⁻² stops both
/// terms, k < 0.01.
#[test]
fn air_above_the_floor_stops_the_wind_and_mars_barely_weathers() {
    let none = Condensates::default();
    let with_air = |pressure: f64| {
        SurfaceModifiers::derive(&ModifierInputs {
            surface_pressure: Pascals::new(pressure),
            ..moon(&none)
        })
        .unwrap()
    };
    // 1 Pa over the Moon's gravity, 0.62 kg m⁻²: Pluto's order.
    let thin = with_air(1.0);
    assert_same_bits(thin.weathering().wind(), 0.0);
    let column = 1.0 / 1.62;
    let dust = thin.weathering().dust();
    assert!((dust - math::exp(-column / 0.3)).abs() < 1e-12, "{dust}");
    // An Io's column, 6 × 10⁻⁵ kg m⁻², lets the wind through.
    let tenuous = with_air(6e-5 * 1.62);
    assert!((tenuous.weathering().wind() - 1.0).abs() < 1e-12);
    let mars = SurfaceModifiers::derive(&ModifierInputs {
        orbit: HeliocentricOrbit {
            semi_major_axis: au(1.523_68),
            eccentricity: 0.0934,
        },
        gravity: MetresPerSecondSquared::new(3.73),
        surface_pressure: Pascals::new(636.0),
        ..moon(&none)
    })
    .unwrap();
    let k = mars.weathering_rate();
    assert!(k < 0.01, "Mars's k {k}");
    assert_same_bits(mars.weathering().wind(), 0.0);
    eprintln!("Mars: k {k:e} (dust {:e})", mars.weathering().dust());
}

/// A moon inside its planet's dayside magnetopause takes no wind (the planet's own plasma, which
/// weathers Io, Europa and Ganymede, is not modelled, labelled); one beyond it takes the wind at
/// its planet's distance. Under the strong class's 30 µT stand-in Jupiter's magnetopause lies at
/// about 17 `R_J`, so Callisto, at 26 `R_J`, is exposed: a recorded miss.
#[test]
fn a_moon_inside_its_planet_s_magnetopause_takes_no_wind() {
    let none = Condensates::default();
    let io = SurfaceModifiers::derive(&jovian_moon(&none, 4.217e8)).unwrap();
    assert_same_bits(io.weathering().wind(), 0.0);
    assert!(io.weathering().dust() > 0.0);
    let callisto = SurfaceModifiers::derive(&jovian_moon(&none, 1.8827e9)).unwrap();
    let wind = callisto.weathering().wind();
    let expected =
        (1.0 - 0.0167 * 0.0167_f64).sqrt() / (5.2038 * 5.2038) / (1.0 - 0.0487 * 0.0487_f64).sqrt();
    assert!(
        (wind / expected - 1.0).abs() < 1e-9,
        "{wind} against {expected}"
    );
    eprintln!(
        "Io-like k {:.4} (wind 0), Callisto-like k {:.4} (wind {wind:.4})",
        io.weathering_rate(),
        callisto.weathering_rate()
    );
}

/// A methane-ice body under a high dose, a Kuiper-belt body's at 40 au over 4 Gyr, carries a cover
/// near its steady state; one without carbon-bearing ice, water's, carbon dioxide's or nitrogen's
/// alone, carries none, nor one whose methane is seasonal; under air above the floor the dose does
/// not reach it (a hazy world's fallout is P14.T53.c's) (plan 14's test).
#[test]
fn a_methane_ice_body_under_a_high_dose_carries_a_cover() {
    let methane = methane_ice();
    let ice = methane
        .entry(id("CH4"), CondensatePhase::Solid)
        .expect("its methane ice");
    assert!(ice.area_fraction() > 0.0 && !ice.is_seasonal(), "{ice:?}");
    let covered = SurfaceModifiers::derive(&kuiper(&methane)).unwrap();
    let cover = covered.organic_cover();
    assert_same_bits(covered.organic_ground(), ice.area_fraction());
    assert!((0.9..=1.0).contains(&cover), "cover {cover}");
    let perennial = |e: &CondensateEntry| {
        e.phase() == CondensatePhase::Solid && !e.is_seasonal() && e.area_fraction() > 0.0
    };
    for (name, inventory) in [
        ("water", "H2O"),
        ("carbon dioxide", "CO2"),
        ("nitrogen", "N2"),
    ] {
        let bare = published(&[(inventory, 1e7)], KUIPER_ISOTHERMAL, KUIPER_GRAVITY);
        assert!(bare.entries().iter().any(perennial), "{name}: {bare:?}");
        let modifiers = SurfaceModifiers::derive(&kuiper(&bare)).unwrap();
        assert_same_bits(modifiers.organic_ground(), 0.0);
        assert_same_bits(modifiers.organic_cover(), 0.0);
    }
    // Methane under a dominant nitrogen air, on the climate's 30–45 K, comes and goes with the
    // seasons: no cover.
    let seasonal = published(&[("CH4", 1e6), ("N2", 1e3)], KUIPER_CLIMATE, KUIPER_GRAVITY);
    let ice = seasonal
        .entry(id("CH4"), CondensatePhase::Solid)
        .expect("its methane ice");
    assert!(ice.is_seasonal(), "{ice:?}");
    let modifiers = SurfaceModifiers::derive(&kuiper(&seasonal)).unwrap();
    assert_same_bits(modifiers.organic_ground(), 0.0);
    assert_same_bits(modifiers.organic_cover(), 0.0);
    // Under 1 Pa of its own air, the ground takes no dose.
    let hazy = SurfaceModifiers::derive(&ModifierInputs {
        surface_pressure: Pascals::new(1.0),
        ..kuiper(&methane)
    })
    .unwrap();
    assert!(hazy.organic_ground() > 0.0);
    assert_same_bits(hazy.organic_cover(), 0.0);
    // A young surface renewed every 500 kyr, Sputnik Planitia's (McKinnon et al. 2016, Nature 534,
    // 82, abstract read), still saturates airless: the recorded finding.
    let renewed = SurfaceModifiers::derive(&ModifierInputs {
        surface_age: Years::new(5e5),
        resurfacing_time: Years::new(5e5),
        ..kuiper(&methane)
    })
    .unwrap();
    eprintln!(
        "Kuiper-belt methane: ground {:.4}, cover {cover:.6}; renewed every 500 kyr, cover {:.4}",
        covered.organic_ground(),
        renewed.organic_cover()
    );
}

/// An `Extreme`-volcanism moon, an Io (its heat flow about 2.5 W m⁻² and its surface about 110 K,
/// *from memory*), is resurfaced with SO₂ frost and sulphur in Geissler et al.'s shares; a dormant
/// moon, a warm one, or one with no sulphur is not. Its resurfacing rate lies within an order of
/// magnitude of Johnson et al. 1979's about 1 cm per year (plan 14's test).
#[test]
fn an_extreme_volcanism_moon_is_resurfaced_with_so2_frost_and_sulphur() {
    let none = Condensates::default();
    let flow = WattsPerSquareMetre::new(2.5);
    let io = || ModifierInputs {
        heat_flow: flow,
        resurfacing_time: resurfacing_time(Crust::Silicate, flow),
        surface_age: Years::new(1e3),
        surface_temperature: Kelvin::new(110.0),
        ..jovian_moon(&none, 4.217e8)
    };
    let modifiers = SurfaceModifiers::derive(&io()).unwrap();
    let deposits = modifiers.volcanic_deposits();
    assert_eq!(deposits.len(), 2, "{deposits:?}");
    assert_eq!(deposits[0].substance(), id("SO2"));
    assert_eq!(deposits[1].substance(), id("S8"));
    assert_same_bits(deposits[0].area_fraction(), IO_SO2_FROST_SHARE);
    assert!((deposits[1].area_fraction() - 0.716).abs() < 1e-12);
    let rate = modifiers.resurfacing_m_per_yr();
    assert!((1e-3..=1e-1).contains(&rate), "{rate} m per year");
    let frost = id("SO2")
        .substance()
        .phase_diagram()
        .unwrap()
        .saturation_temperature(IO_SO2_PRESSURE)
        .unwrap();
    eprintln!(
        "Io-like: resurfacing time {:.0} yr, {:.2} cm per year, SO2 frost point {frost:?}",
        io().resurfacing_time.value(),
        rate * 100.0
    );
    let dormant = SurfaceModifiers::derive(&ModifierInputs {
        heat_flow: WattsPerSquareMetre::new(0.009),
        ..io()
    })
    .unwrap();
    let warm = SurfaceModifiers::derive(&ModifierInputs {
        surface_temperature: Kelvin::new(300.0),
        ..io()
    })
    .unwrap();
    let dry = SurfaceModifiers::derive(&ModifierInputs {
        mantle_sulphur: Kilograms::ZERO,
        ..io()
    })
    .unwrap();
    for body in [dormant, warm, dry] {
        assert_eq!(body.volcanic_deposits(), [], "{body:?}");
    }
}

/// The units contract (decision-r11-palette-liquids §2 item 4): m, the cover and k finite and +0
/// or more, never −0; m and the cover in [0, 1]; the resurfacing rate finite and +0 or more; over
/// hosts, orbits, airs, fields, ages and resurfacing times.
#[test]
fn the_modifiers_keep_the_units_contract() {
    let methane = methane_ice();
    let solar = solar_xuv_luminosity().value();
    let mut count = 0_u32;
    for xuv in [0.0, solar, 1e3 * solar] {
        for a in [0.02, 0.387, 1.0, 5.2, 40.0] {
            for e in [0.0, -0.0, 0.2, 0.9] {
                for pressure in [0.0, 1e-5, 1.0, 636.0, 9.2e6] {
                    for field in [
                        MagneticField::None,
                        MagneticField::Weak,
                        MagneticField::Strong,
                    ] {
                        for age in [0.0, -0.0, 1e3, 1e8, 4.5e9] {
                            for tau in [1e3, 6.42e7, f64::INFINITY] {
                                let inputs = ModifierInputs {
                                    host: ModifierHost {
                                        xuv_luminosity: Watts::new(xuv),
                                        ..ModifierHost::sun()
                                    },
                                    orbit: HeliocentricOrbit {
                                        semi_major_axis: au(a),
                                        eccentricity: e,
                                    },
                                    surface_pressure: Pascals::new(pressure),
                                    field,
                                    surface_age: Years::new(age),
                                    resurfacing_time: Years::new(tau),
                                    ..kuiper(&methane)
                                };
                                let m = SurfaceModifiers::derive(&inputs).unwrap();
                                for (name, value, unit) in [
                                    ("maturity", m.maturity(), true),
                                    ("organic cover", m.organic_cover(), true),
                                    ("organic ground", m.organic_ground(), true),
                                    ("weathering rate", m.weathering_rate(), false),
                                    ("wind", m.weathering().wind(), false),
                                    ("dust", m.weathering().dust(), false),
                                    ("resurfacing", m.resurfacing_m_per_yr(), false),
                                ] {
                                    assert!(
                                        value.is_finite()
                                            && value >= 0.0
                                            && value.is_sign_positive(),
                                        "{name} {value} for {inputs:?}"
                                    );
                                    assert!(!unit || value <= 1.0, "{name} {value} for {inputs:?}");
                                }
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 3 * 5 * 4 * 5 * 3 * 5 * 3);
}

/// The derivation draws nothing and repeats to the bit: the same inputs give the same values, and
/// the module's code names no stream, tag or seed.
#[test]
fn the_modifiers_draw_nothing_and_repeat_to_the_bit() {
    let methane = methane_ice();
    let none = Condensates::default();
    for inputs in [
        kuiper(&methane),
        mercury(&none),
        jovian_moon(&none, 4.217e8),
    ] {
        let [a, b] = [0, 1].map(|_| SurfaceModifiers::derive(&inputs).unwrap());
        assert_same_bits(a.maturity(), b.maturity());
        assert_same_bits(a.weathering().wind(), b.weathering().wind());
        assert_same_bits(a.weathering().dust(), b.weathering().dust());
        assert_same_bits(a.organic_ground(), b.organic_ground());
        assert_same_bits(a.organic_cover(), b.organic_cover());
        assert_same_bits(a.resurfacing_m_per_yr(), b.resurfacing_m_per_yr());
        assert_eq!(a, b);
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/planetary/hooks/modifiers.rs");
    let code = scanned_code(&std::fs::read_to_string(src).unwrap());
    let draws: Vec<&str> = ["Stream", "rng::", "tags::", "Seed", "UnitUniform"]
        .into_iter()
        .filter(|pattern| code.contains(pattern))
        .collect();
    assert_eq!(draws, Vec::<&str>::new());
}

/// Inputs out of range are refused, each by name.
#[test]
fn the_modifiers_refuse_inputs_out_of_range() {
    let none = Condensates::default();
    let refused = |inputs: ModifierInputs<'_>| SurfaceModifiers::derive(&inputs).unwrap_err();
    let cases = [
        (
            ModifierInputs {
                surface_age: Years::new(f64::NAN),
                ..moon(&none)
            },
            ModifierInput::SurfaceAge,
        ),
        (
            ModifierInputs {
                surface_pressure: Pascals::new(-1.0),
                ..moon(&none)
            },
            ModifierInput::SurfacePressure,
        ),
        (
            ModifierInputs {
                orbit: HeliocentricOrbit {
                    semi_major_axis: au(1.0),
                    eccentricity: 1.0,
                },
                ..moon(&none)
            },
            ModifierInput::Eccentricity,
        ),
        (
            ModifierInputs {
                resurfacing_time: Years::ZERO,
                ..moon(&none)
            },
            ModifierInput::ResurfacingTime,
        ),
        (
            ModifierInputs {
                gravity: MetresPerSecondSquared::new(f64::INFINITY),
                ..moon(&none)
            },
            ModifierInput::Gravity,
        ),
        (
            jovian_moon(&none, -1.0),
            ModifierInput::PlanetocentricSemiMajorAxis,
        ),
    ];
    for (inputs, input) in cases {
        let error = refused(inputs);
        assert_eq!(error, DeriveModifiersError::OutOfRange(input));
        assert!(error.to_string().starts_with("the body's "), "{error}");
    }
}

/// A weathering rate that is not finite on inputs each in its range is refused before the
/// maturity, which would panic on it: a host whose G M★ is the largest `f64`, so that the
/// micrometeoroids' speed term (G M★ ÷ r)^(3/2) overflows, and an orbit of 10⁻¹⁷⁰ m, whose
/// semi-major axis's square underflows to 0 under the wind's flux.
#[test]
fn the_modifiers_refuse_a_rate_that_is_not_finite() {
    let none = Condensates::default();
    let heavy = ModifierInputs {
        host: ModifierHost {
            gravitational_parameter: GravitationalParameter::new(f64::MAX),
            ..ModifierHost::sun()
        },
        ..moon(&none)
    };
    let close = ModifierInputs {
        orbit: HeliocentricOrbit {
            semi_major_axis: Metres::new(1e-170),
            eccentricity: 0.0,
        },
        ..moon(&none)
    };
    for inputs in [heavy, close] {
        let error = SurfaceModifiers::derive(&inputs).unwrap_err();
        assert_eq!(error, DeriveModifiersError::RateNotFinite);
        assert_eq!(
            error.to_string(),
            "the body's weathering rate is not finite"
        );
    }
}

/// Every `.rs` file under `dir`, recursively, in sorted order.
fn source_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            source_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// The code of a source file as the tripwire scans it: its lines up to its inline test module,
/// the first `#[cfg(test)]` whose next line opens `mod tests {`, its comment lines left out (a doc
/// may name the module).
fn scanned_code(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let end = lines
        .windows(2)
        .position(|pair| {
            pair[0].trim() == "#[cfg(test)]" && pair[1].trim_start().starts_with("mod tests {")
        })
        .unwrap_or(lines.len());
    lines[..end]
        .iter()
        .filter(|line| !line.trim_start().starts_with("//"))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Nothing on the record calls the surface modifiers or the shared maturity law at generator
/// version 22 (decision-bump-plan-after-22 §1.3: P14.T51.d is a pure rider of 24). P14.T54.b, the
/// 23 → 24 bump, puts [`SurfaceModifiers`] in the record's surface section, fills the field
/// header's `maturity`, `weathering_rate` and `organic_cover` from it, and deletes this test.
///
/// It walks the sim's `src/` outside `planetary/hooks/modifiers*` and the surface crate's `src/`
/// outside `material/maturity.rs`, in sorted order, and fails on a call form, path or name of
/// either in each file's code (the file less its comment lines and its inline test module).
/// `hooks/mod.rs`'s `pub mod modifiers;` and its one `pub use modifiers::{…};`, and
/// `material/mod.rs`'s `mod maturity;` and its one `pub use maturity::{…};`, are allowed. The
/// bare name `SurfaceModifiers` is not matched, since `record.rs` has its own placeholder of that
/// name, nor `modifiers::{`, the galaxy's gas modifiers' path.
#[test]
fn modifiers_nothing_on_the_record_reads_the_modifiers_yet() {
    // `modifiers::{` alone would match the galaxy's gas modifiers.
    const SIM_PATTERNS: [&str; 10] = [
        "SurfaceModifiers::derive(",
        "ModifierInputs",
        "hooks::modifiers",
        "super::modifiers",
        "solar_xuv_luminosity",
        "wind_mass_loss_kg_per_s",
        "organic_ground(",
        "body_maturity",
        "maturity_after",
        "LUNAR_MATURATION_TIME",
    ];
    const SURFACE_PATTERNS: [&str; 4] = [
        "body_maturity",
        "maturity_after",
        "LUNAR_MATURATION_TIME",
        "maturity::",
    ];
    // (file, hold): files allowed to match while a named hold stands; none yet.
    const HELD: [(&str, &str); 0] = [];
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates = [
        (
            manifest.join("src"),
            "planetary/hooks/modifiers",
            &SIM_PATTERNS[..],
        ),
        (
            manifest.join("../hyperion-surface/src"),
            "material/maturity.rs",
            &SURFACE_PATTERNS[..],
        ),
    ];
    let mut readers = Vec::new();
    for (src, own, patterns) in &crates {
        let mut files = Vec::new();
        source_files(src, &mut files);
        assert!(
            files.len() > 20,
            "the walk found {}'s sources",
            src.display()
        );
        for path in files {
            let relative = path
                .strip_prefix(src)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if relative.starts_with(own) || HELD.iter().any(|(file, _)| *file == relative) {
                continue;
            }
            let code = scanned_code(&std::fs::read_to_string(&path).unwrap());
            for line in code.lines() {
                let trimmed = line.trim();
                let allowed = (relative == "planetary/hooks/mod.rs"
                    && trimmed.starts_with("pub use modifiers::{"))
                    || (relative == "material/mod.rs"
                        && trimmed.starts_with("pub use maturity::{"));
                if allowed {
                    continue;
                }
                for pattern in *patterns {
                    if line.contains(pattern) {
                        readers.push(format!("{relative}: {pattern}"));
                    }
                }
            }
        }
    }
    assert_eq!(
        readers,
        Vec::<String>::new(),
        "the surface modifiers or the maturity law are read on the record before P14.T54.b"
    );
    // The allowed declarations are there, once each.
    let hooks = std::fs::read_to_string(manifest.join("src/planetary/hooks/mod.rs")).unwrap();
    assert_eq!(hooks.matches("pub use modifiers::{").count(), 1);
    assert_eq!(hooks.matches("pub mod modifiers;").count(), 1);
    let material =
        std::fs::read_to_string(manifest.join("../hyperion-surface/src/material/mod.rs")).unwrap();
    assert_eq!(material.matches("pub use maturity::{").count(), 1);
    assert_eq!(material.matches("mod maturity;").count(), 1);
}
