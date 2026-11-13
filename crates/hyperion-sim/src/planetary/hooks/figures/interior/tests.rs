use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::units::consts::SECONDS_PER_DAY;

/// An Earth's regime inputs: 1 M⊕, a radiogenic flux of 39 mW m⁻², temperate under 2.7 km of
/// water, at 9.8 m s⁻², with no tide and no ocean under ice.
fn earth_like() -> RegimeInputs {
    RegimeInputs {
        interior: Interior::Convecting,
        mass: EarthMasses::new(1.0),
        radiogenic: WattsPerSquareMetre::new(0.039),
        tidal: WattsPerSquareMetre::new(0.0),
        state: SurfaceState::Temperate,
        water_depth: Metres::new(2.7e3),
        surface_gravity: MetresPerSecondSquared::new(9.8),
        ocean_shell: None,
    }
}

/// An Earth's field inputs: a mobile lid, an iron share of 0.325, 39 mW m⁻² of radiogenic heat,
/// no tide, 4.57 Gyr old, turning in its sidereal day.
fn earth_field() -> FieldInputs {
    FieldInputs {
        class: PlanetClass::Rocky,
        interior: Interior::Convecting,
        mass: EarthMasses::new(1.0),
        iron_share: 0.325,
        regime: TectonicRegime::MobileLid,
        radiogenic: WattsPerSquareMetre::new(0.039),
        tidal: WattsPerSquareMetre::new(0.0),
        age: Years::new(4.57e9),
        rotation_period: Seconds::new(86_164.1),
    }
}

#[test]
fn the_isotopes_hold_the_bulk_silicate_earth_s_production() {
    let production =
        RadiogenicIsotope::ALL.map(RadiogenicIsotope::present_heat_production_w_per_kg);
    let total = production.iter().fold(0.0, |sum, &p| sum + p);
    eprintln!(
        "the bulk silicate Earth: {total:e} W kg⁻¹, shares {:?}",
        production.map(|p| p / total)
    );
    // Decision-p14-moons §F-ocean's 4.9 × 10⁻¹² W kg⁻¹, and Earth's 0.675 M⊕ of rock at 19.8 TW.
    assert!((total / 4.92e-12 - 1.0).abs() < 2e-3, "{total:e}");
    let earth = 0.675 * crate::units::consts::EARTH_MASS_KG * total;
    assert!((earth / 19.8e12 - 1.0).abs() < 5e-3, "{earth:e} W");
    // Today's shares: ²³⁸U 0.3874, ²³⁵U 0.0167, ²³²Th 0.4265, ⁴⁰K 0.1695 (science-p14-t24b-laws
    // §1.4), and the ruling's 4.9213 × 10⁻¹² W kg⁻¹.
    for (p, share) in production.iter().zip([0.3874, 0.0167, 0.4265, 0.1695]) {
        assert!((p / total - share).abs() < 5e-5, "{}", p / total);
    }
    assert!((total / 4.9213e-12 - 1.0).abs() < 1e-4, "{total:e}");
    // At the reference age the production is the present one, bit for bit.
    let now = radiogenic_heat_production_w_per_kg(Dex::new(0.0), RADIOGENIC_REFERENCE_AGE);
    for (now, present) in now.iter().zip(production) {
        assert_same_bits(*now, present);
    }
}

#[test]
fn each_isotope_decays_over_its_own_mean_life() {
    for (k, isotope) in RadiogenicIsotope::ALL.iter().enumerate() {
        let tau = isotope.mean_life().value();
        assert!((tau * core::f64::consts::LN_2 / isotope.half_life().value() - 1.0).abs() < 1e-15);
        let later = Years::new(RADIOGENIC_REFERENCE_AGE.value() + tau);
        let decayed = radiogenic_heat_production_w_per_kg(Dex::new(0.0), later)[k];
        let ratio = decayed / isotope.present_heat_production_w_per_kg();
        assert!(
            (ratio - math::exp(-1.0)).abs() < 1e-12,
            "{isotope:?}: {ratio}"
        );
    }
    // The young Earth made 4.95 times today's heat at its birth, ⁴⁰K and ²³⁵U most of it.
    let birth = radiogenic_heat_production_w_per_kg(Dex::new(0.0), Years::new(0.0));
    let today = radiogenic_heat_production_w_per_kg(Dex::new(0.0), RADIOGENIC_REFERENCE_AGE);
    let sum = |x: [f64; 4]| x.iter().fold(0.0, |s, &p| s + p);
    let ratio = sum(birth) / sum(today);
    eprintln!("at birth, {ratio} times today's production");
    assert!((ratio / 4.95 - 1.0).abs() < 0.01, "{ratio}");
}

#[test]
fn the_radiogenic_flow_scales_as_the_metals_and_the_rock_over_the_area() {
    let (rock, radius, age) = (
        Kilograms::from(EarthMasses::new(0.675)),
        Metres::new(6.371e6),
        Years::new(4.57e9),
    );
    let base = radiogenic_heat_flow(rock, radius, Dex::new(0.0), age).value();
    eprintln!("an Earth's radiogenic flow: {base} W m⁻²");
    assert!((base - 0.0389).abs() < 5e-4, "{base}");
    for fe_h in [-1.0, -0.3, 0.2, 0.5] {
        let scaled = radiogenic_heat_flow(rock, radius, Dex::new(fe_h), age).value();
        let expected = math::exp10(fe_h);
        assert!(
            (scaled / base / expected - 1.0).abs() < 1e-12,
            "[Fe/H] {fe_h}"
        );
    }
    let doubled = radiogenic_heat_flow(
        Kilograms::new(2.0 * rock.value()),
        radius,
        Dex::new(0.0),
        age,
    );
    assert!((doubled.value() / base - 2.0).abs() < 1e-12);
    let wider = radiogenic_heat_flow(rock, Metres::new(2.0 * radius.value()), Dex::new(0.0), age);
    assert!((wider.value() / base - 0.25).abs() < 1e-12);
    for r in [0.0, -1.0, f64::NAN] {
        assert_same_bits(
            radiogenic_heat_flow(rock, Metres::new(r), Dex::new(0.0), age).value(),
            0.0,
        );
    }
}

#[test]
fn the_lag_follows_the_ruling_s_spheres() {
    // Decision-p14-moons §F-ocean F1's spheres at 4.5 Gyr: Pluto's rock, 850 km; Enceladus's,
    // 190 km; Ganymede's, 1,800 km. Its present shares, 0.386, 0.014, 0.43 and 0.17, gave its
    // 1.26, 1.01 and 0.90; the production's own shares at the age give 1.285, 1.014 and 0.917,
    // since the lag rises some eight times as fast as ²³⁵U's share (science-p14-t24b-laws §1.4).
    let age = Years::new(4.5e9);
    let kappa = ROCK_THERMAL_DIFFUSIVITY_M2_PER_S;
    let ruled_shares = [0.386, 0.014, 0.43, 0.17];
    for (name, km, ruled, own) in [
        ("Pluto", 850.0, 1.26, 1.285),
        ("Enceladus", 190.0, 1.01, 1.014),
        ("Ganymede", 1_800.0, 0.90, 0.917),
    ] {
        let radius = Metres::new(km * 1e3);
        let lag = conductive_lag(radius, age, kappa);
        let at_ruled = lag_of(ruled_shares, radius, age, kappa);
        eprintln!("{name}: L {lag:.4}, {at_ruled:.4} at the ruling's shares");
        assert!((lag - own).abs() < 0.02, "{name}: {lag}");
        assert!((at_ruled - ruled).abs() < 0.005, "{name}: {at_ruled}");
    }
    // It tends to its steady state, 1 less the truncated modes' 0.0015, as κ grows.
    let steady = conductive_lag(Metres::new(850e3), age, f64::INFINITY);
    let truncated = (1..=LAG_MODES).fold(0.0, |sum, n| {
        sum + 6.0 / (f64::from(n * n) * core::f64::consts::PI * core::f64::consts::PI)
    });
    assert!((steady - truncated).abs() < 1e-15, "{steady}");
    assert!((steady - 1.0).abs() < 2e-3, "{steady}");
    let mut gap = f64::INFINITY;
    for kappa in [1e-6, 1e-5, 1e-4, 1e-3, 1e-2] {
        let lag = conductive_lag(Metres::new(850e3), age, kappa);
        assert!((lag - steady).abs() < gap, "{kappa}: {lag}");
        gap = (lag - steady).abs();
    }
    assert!(gap < 1e-4, "{gap}");
    assert_same_bits(conductive_lag(Metres::new(0.0), age, kappa), steady);
    // A rock that does not conduct lets nothing out, whatever its radius.
    assert_same_bits(conductive_lag(Metres::new(850e3), age, 0.0), 0.0);
    assert_same_bits(conductive_lag(Metres::new(0.0), age, 0.0), 0.0);
    // Pluto's lag over time: 0.67 at 1 Gyr, as its heat builds, and 1.20 at 12 Gyr
    // (science-p14-t24b-laws §1.4).
    for (gyr, expected) in [(1.0, 0.67), (12.0, 1.20)] {
        let lag = conductive_lag(Metres::new(850e3), Years::new(gyr * 1e9), kappa);
        eprintln!("Pluto at {gyr} Gyr: L {lag:.4}");
        assert!((lag - expected).abs() < 0.02, "{gyr} Gyr: {lag}");
    }
}

#[test]
fn the_lag_s_shares_are_the_production_s_at_every_age() {
    for step in 0..=28 {
        let age = Years::new(0.5e9 * f64::from(step));
        let production = radiogenic_heat_production_w_per_kg(Dex::new(0.3), age);
        let total = production.iter().fold(0.0, |sum, &p| sum + p);
        let sum = production.iter().fold(0.0, |sum, &p| sum + p / total);
        assert!((sum - 1.0).abs() < 1e-12, "{step}: {sum}");
    }
    // At the reference age they are the ruling's present shares.
    let production = radiogenic_heat_production_w_per_kg(Dex::new(0.0), RADIOGENIC_REFERENCE_AGE);
    let total = production.iter().fold(0.0, |sum, &p| sum + p);
    for (p, share) in production.iter().zip([0.3874, 0.0167, 0.4265, 0.1695]) {
        assert!((p / total - share).abs() < 5e-5, "{}", p / total);
    }
}

#[test]
fn the_lag_starts_at_nothing_and_is_continuous_in_time() {
    let radius = Metres::new(850e3);
    let kappa = ROCK_THERMAL_DIFFUSIVITY_M2_PER_S;
    for age in [0.0, -1.0, f64::NAN] {
        assert_same_bits(conductive_lag(radius, Years::new(age), kappa), 0.0);
    }
    // From its birth it rises from nothing, and a step's change shrinks with the step: at birth
    // as √t, a diffusion's start, so that halving the step takes off at least 1 − 1 ÷ √2 of it,
    // and later in proportion.
    for (start, shrink) in [(0.0, 0.75), (1e8, 0.55), (4.5e9, 0.55)] {
        let at = |dt: f64| conductive_lag(radius, Years::new(start + dt), kappa);
        let base = if start > 0.0 { at(0.0) } else { 0.0 };
        let (coarse, fine) = ((at(2e6) - base).abs(), (at(1e6) - base).abs());
        assert!(
            coarse < 0.05 && fine < shrink * coarse + 1e-12,
            "{start}: {coarse}, {fine}"
        );
    }
    // In steps of 10 Myr from 100 Myr to 13.8 Gyr, it moves by under two hundredths.
    let mut last = conductive_lag(radius, Years::new(1e8), kappa);
    let mut largest = 0.0_f64;
    for step in 1..1_370 {
        let lag = conductive_lag(radius, Years::new(1e8 + 1e7 * f64::from(step)), kappa);
        largest = largest.max((lag - last).abs());
        last = lag;
    }
    eprintln!("the lag's largest step over 10 Myr after 100 Myr: {largest}");
    assert!(largest < 0.02, "{largest}");
    // Where a mode's rate meets an isotope's, the term is its limit, sₙ t.
    for x in [0.0, 1e-17, -1e-17, 1e-12] {
        assert!((held_share(x) - 1.0).abs() < 1e-11, "{x}");
    }
    assert!((held_share(1.0) - (1.0 - math::exp(-1.0))).abs() < 1e-15);
}

#[test]
fn the_outgassing_is_earth_s_unit_above_the_melt() {
    assert_same_bits(outgassing(GENERATED_EARTH_HEAT_FLOW), 1.0);
    assert_same_bits(outgassing(SURFACE_MELT_HEAT_FLOW), 0.0);
    for f in [0.0, 0.01, 0.024_999, -1.0, f64::NAN] {
        assert_same_bits(outgassing(WattsPerSquareMetre::new(f)), 0.0);
    }
    let mut last = 0.0;
    for step in 0..=200 {
        let o = outgassing(WattsPerSquareMetre::new(0.001 * f64::from(step)));
        assert!(o >= last, "{step}");
        last = o;
    }
    // It is linear in the heat flow above the melt: twice Earth's excess, twice its rate.
    let twice = SURFACE_MELT_HEAT_FLOW.value()
        + 2.0 * (GENERATED_EARTH_HEAT_FLOW.value() - SURFACE_MELT_HEAT_FLOW.value());
    assert!((outgassing(WattsPerSquareMetre::new(twice)) - 2.0).abs() < 1e-12);
}

#[test]
fn an_ice_shell_and_a_heat_pipe_come_before_any_lid() {
    let earth = earth_like();
    assert_eq!(tectonic_regime(&earth), TectonicRegime::MobileLid);
    let cases = [
        // 1. An ocean under a thin shell, whatever its tides.
        (
            RegimeInputs {
                ocean_shell: Some(Metres::new(2.7e3)),
                tidal: WattsPerSquareMetre::new(0.234),
                ..earth
            },
            TectonicRegime::IceShell,
        ),
        (
            RegimeInputs {
                ocean_shell: Some(ICE_SHELL_LIMIT),
                ..earth
            },
            TectonicRegime::IceShell,
        ),
        // A thick shell is not one: the body's own clause decides.
        (
            RegimeInputs {
                ocean_shell: Some(Metres::new(60.1e3)),
                ..earth
            },
            TectonicRegime::MobileLid,
        ),
        // 2. Io's tides, and a magma ocean.
        (
            RegimeInputs {
                tidal: HEAT_PIPE_TIDAL_FLUX,
                ..earth
            },
            TectonicRegime::HeatPipe,
        ),
        (
            RegimeInputs {
                state: SurfaceState::MagmaOcean,
                ..earth
            },
            TectonicRegime::HeatPipe,
        ),
        // 3. A conducting body, under any tide below the pipe's.
        (
            RegimeInputs {
                interior: Interior::Conducting,
                ..earth
            },
            TectonicRegime::StagnantLid,
        ),
        (
            RegimeInputs {
                interior: Interior::Conducting,
                tidal: WattsPerSquareMetre::new(1.0),
                ..earth
            },
            TectonicRegime::StagnantLid,
        ),
    ];
    for (inputs, regime) in cases {
        assert_eq!(tectonic_regime(&inputs), regime, "{inputs:?}");
    }
}

#[test]
fn a_lid_fails_where_it_is_massive_productive_and_above_high_pressure_ice() {
    let earth = earth_like();
    let cases = [
        // 4. A dry Earth (30 m of water), a runaway one and a snowball.
        (
            RegimeInputs {
                water_depth: Metres::new(30.0),
                ..earth
            },
            TectonicRegime::Episodic,
        ),
        (
            RegimeInputs {
                state: SurfaceState::RunawayGreenhouse,
                ..earth
            },
            TectonicRegime::Episodic,
        ),
        (
            RegimeInputs {
                state: SurfaceState::Snowball,
                ..earth
            },
            TectonicRegime::MobileLid,
        ),
        (
            RegimeInputs {
                water_depth: PLATE_WATER_DEPTH,
                ..earth
            },
            TectonicRegime::MobileLid,
        ),
        // 5. A small, an unproductive and a drowned Earth: under 200 km of water its floor,
        // 1.96 GPa, is high-pressure ice.
        (
            RegimeInputs {
                mass: EarthMasses::new(0.29),
                ..earth
            },
            TectonicRegime::StagnantLid,
        ),
        (
            RegimeInputs {
                radiogenic: WattsPerSquareMetre::new(0.019),
                ..earth
            },
            TectonicRegime::StagnantLid,
        ),
        (
            RegimeInputs {
                water_depth: Metres::new(200e3),
                ..earth
            },
            TectonicRegime::StagnantLid,
        ),
    ];
    for (inputs, regime) in cases {
        assert_eq!(tectonic_regime(&inputs), regime, "{inputs:?}");
    }
    // The edges are closed: the least mass and flux fail.
    let edge = RegimeInputs {
        mass: LID_FAILURE_MASS,
        radiogenic: LID_FAILURE_FLUX,
        ..earth
    };
    assert_eq!(tectonic_regime(&edge), TectonicRegime::MobileLid);
    // The floor's pressure: 632.4 MPa at 9.8 m s⁻² is 64.5 km of water.
    let floor = HIGH_PRESSURE_ICE_PRESSURE.value() / (REGIME_WATER_DENSITY.value() * 9.8);
    for (depth, regime) in [
        (floor * 0.999, TectonicRegime::MobileLid),
        (floor * 1.001, TectonicRegime::StagnantLid),
    ] {
        let inputs = RegimeInputs {
            water_depth: Metres::new(depth),
            ..earth
        };
        assert_eq!(tectonic_regime(&inputs), regime, "{depth} m");
    }
}

#[test]
fn the_residual_carries_the_core_s_heat() {
    let q = WattsPerSquareMetre::new(0.04);
    // With no core it is the production over the silicate Urey ratio.
    for (regime, urey) in [
        (TectonicRegime::MobileLid, MOBILE_LID_UREY_RATIO),
        (TectonicRegime::StagnantLid, STAGNANT_LID_UREY_RATIO),
        (TectonicRegime::Episodic, STAGNANT_LID_UREY_RATIO),
        (TectonicRegime::HeatPipe, STAGNANT_LID_UREY_RATIO),
        (TectonicRegime::IceShell, STAGNANT_LID_UREY_RATIO),
    ] {
        let flow = convecting_heat_flow(q, 0.0, regime).value();
        assert!(
            (flow / (q.value() / urey) - 1.0).abs() < 1e-12,
            "{regime:?}"
        );
    }
    // The ruling's form, q_rad [1 + (1 ÷ Ur − 1)(1 + γ x ÷ (1 − x))], with q_rad = (1 − x) q_solid.
    for x in [0.1, 0.325, 0.7, 0.9] {
        let flow = convecting_heat_flow(q, x, TectonicRegime::StagnantLid).value();
        let q_rad = (1.0 - x) * q.value();
        let ruled = q_rad
            * (1.0
                + (1.0 / STAGNANT_LID_UREY_RATIO - 1.0)
                    * (1.0 + CORE_TO_MANTLE_HEAT_CAPACITY * x / (1.0 - x)));
        assert!((flow / ruled - 1.0).abs() < 1e-12, "{x}");
    }
    // A body of iron alone stays finite.
    let iron = convecting_heat_flow(q, 1.0, TectonicRegime::StagnantLid).value();
    assert!(iron.is_finite() && iron > 0.0, "{iron}");
    assert!(
        (iron - q.value() * (1.0 / STAGNANT_LID_UREY_RATIO - 1.0) * CORE_TO_MANTLE_HEAT_CAPACITY)
            .abs()
            < 1e-15
    );
    // The interior's branch: an icy body's rock conducts below 2,000 km and convects from it.
    for (class, radius, interior) in [
        (PlanetClass::Icy, 1_999e3, Some(Interior::Conducting)),
        (PlanetClass::Icy, 2_000e3, Some(Interior::Convecting)),
        (PlanetClass::Rocky, 100e3, Some(Interior::Convecting)),
        (PlanetClass::SubNeptune, 1e6, None),
        (PlanetClass::IceGiant, 1e6, None),
        (PlanetClass::GasGiant, 1e6, None),
    ] {
        assert_eq!(
            Interior::of(class, Metres::new(radius)),
            interior,
            "{class:?}"
        );
    }
}

#[test]
fn a_field_needs_a_convecting_core_of_note_and_no_tidal_mantle() {
    let earth = earth_field();
    assert_eq!(magnetic_field(&earth), MagneticField::Strong);
    let day = SECONDS_PER_DAY;
    let cases = [
        // 1. Every gas envelope.
        (
            FieldInputs {
                class: PlanetClass::GasGiant,
                ..earth
            },
            MagneticField::Strong,
        ),
        (
            FieldInputs {
                class: PlanetClass::IceGiant,
                rotation_period: Seconds::new(0.72 * day),
                ..earth
            },
            MagneticField::Strong,
        ),
        (
            FieldInputs {
                class: PlanetClass::SubNeptune,
                rotation_period: Seconds::new(30.0 * day),
                ..earth
            },
            MagneticField::Strong,
        ),
        // 2. A conducting interior, and a core of no note.
        (
            FieldInputs {
                class: PlanetClass::Icy,
                interior: Interior::Conducting,
                ..earth
            },
            MagneticField::None,
        ),
        (
            FieldInputs {
                iron_share: 0.099,
                ..earth
            },
            MagneticField::None,
        ),
        // 3. Io's tides.
        (
            FieldInputs {
                tidal: WattsPerSquareMetre::new(2.2),
                ..earth
            },
            MagneticField::None,
        ),
    ];
    for (inputs, field) in cases {
        assert_eq!(magnetic_field(&inputs), field, "{inputs:?}");
    }
}

#[test]
fn a_dynamo_is_thermal_or_deep() {
    let earth = earth_field();
    let day = SECONDS_PER_DAY;
    let cases = [
        // 4. A thermal dynamo: an Earth twin locked in a 10-day orbit is multipolar; a young
        // Mars strong and a young Venus weak.
        (
            FieldInputs {
                rotation_period: Seconds::new(10.0 * day),
                ..earth
            },
            MagneticField::Weak,
        ),
        (
            FieldInputs {
                rotation_period: DIPOLAR_ROTATION_PERIOD,
                ..earth
            },
            MagneticField::Strong,
        ),
        (
            FieldInputs {
                mass: EarthMasses::new(0.107),
                iron_share: 0.24,
                regime: TectonicRegime::StagnantLid,
                age: Years::new(0.5e9),
                rotation_period: Seconds::new(1.026 * day),
                ..earth
            },
            MagneticField::Strong,
        ),
        (
            FieldInputs {
                mass: EarthMasses::new(0.815),
                iron_share: 0.30,
                regime: TectonicRegime::Episodic,
                age: Years::new(0.5e9),
                rotation_period: Seconds::new(243.0 * day),
                ..earth
            },
            MagneticField::Weak,
        ),
        // 5. Mercury's deep dynamo, which a Moon-sized core of iron lacks.
        (
            FieldInputs {
                mass: EarthMasses::new(0.055),
                iron_share: 0.70,
                regime: TectonicRegime::StagnantLid,
                rotation_period: Seconds::new(58.6 * day),
                ..earth
            },
            MagneticField::Weak,
        ),
        (
            FieldInputs {
                mass: EarthMasses::new(0.029),
                iron_share: 0.70,
                regime: TectonicRegime::StagnantLid,
                ..earth
            },
            MagneticField::None,
        ),
        // 6. An old Mars and an old Venus.
        (
            FieldInputs {
                mass: EarthMasses::new(0.107),
                iron_share: 0.24,
                regime: TectonicRegime::StagnantLid,
                rotation_period: Seconds::new(1.026 * day),
                ..earth
            },
            MagneticField::None,
        ),
        (
            FieldInputs {
                mass: EarthMasses::new(0.815),
                iron_share: 0.30,
                regime: TectonicRegime::Episodic,
                rotation_period: Seconds::new(243.0 * day),
                ..earth
            },
            MagneticField::None,
        ),
    ];
    for (inputs, field) in cases {
        assert_eq!(magnetic_field(&inputs), field, "{inputs:?}");
    }
    // The early age is closed above: at 0.6 Gyr an old Mars's dynamo is gone.
    let mars_at = |age: f64| FieldInputs {
        mass: EarthMasses::new(0.107),
        iron_share: 0.24,
        regime: TectonicRegime::StagnantLid,
        age: Years::new(age),
        rotation_period: Seconds::new(1.026 * day),
        ..earth
    };
    assert_eq!(magnetic_field(&mars_at(0.599_999e9)), MagneticField::Strong);
    assert_eq!(magnetic_field(&mars_at(0.6e9)), MagneticField::None);
}
