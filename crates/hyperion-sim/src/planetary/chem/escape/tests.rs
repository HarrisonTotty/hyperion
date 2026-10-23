//! The escape's tests: its hold's tripwire, and the rules a test sets on its thread.

use super::*;
use crate::planetary::derive::atmosphere::GreenhouseConstants;

/// P14.T53.a is held until the 23 → 24 bump: the rules in force are versions 22's and 23's, and
/// the record's greenhouse is P14.T57.a's fits.
#[test]
fn the_escape_is_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < ESCAPE_VERSION,
        "the 23 → 24 bump (P14.T54.b) puts P14.T53.a on the record: the greenhouse's broadened \
         carbon dioxide term with its refits (`GreenhouseConstants::BATCH`, Earth's \
         `BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K`) and the greenhouse gain's (1 − x)(1 \
         − d) weight. Delete `ESCAPE_VERSION`, `EscapeRules` and its `Held` arm (every `match` on \
         it keeps its `Batch` arm), `GreenhouseConstants::FITTED`'s role as the record's (its \
         `in_force` then gives `BATCH` alone, and the tests that pin version 23's figures under \
         `FITTED` move to the batch's), the tests' `with_rules`, and this test. From P14.T53.b \
         part 1: delete the tests' `with_present_state` and `PresentState` (its `Present` arm \
         stands), so that `present_state_in_force` reads the batch alone. From P14.T53.b part 2: \
         keep the supercritical envelope's test (`Atmosphere::supercritical_reservoir`, read by \
         `has_surface_section` at the record's three section tags) and delete only its hold, \
         `supercritical_section_in_force`, so that it reads the batch alone"
    );
    assert_eq!(EscapeRules::in_force(), EscapeRules::Held);
    assert_eq!(GreenhouseConstants::in_force(), GreenhouseConstants::FITTED);
    assert!(!crate::planetary::derive::atmosphere::present_state_in_force());
    assert!(!crate::planetary::derive::atmosphere::supercritical_section_in_force());
}

/// A test's rules hold on its thread inside [`with_rules`], nested or not, and the rules before
/// come back after; the greenhouse follows them, and the Rayleigh floor's hold is switched with
/// them (version 24 lifts both).
#[test]
fn under_the_escape_batch_a_test_s_rules_hold_inside_with_rules_and_are_set_back_after() {
    use crate::planetary::derive::photometry::area_mix::RayleighFloorRules;
    let outside = (EscapeRules::in_force(), RayleighFloorRules::in_force());
    let (batch, held, constants) = with_rules(EscapeRules::Batch, || {
        let held = with_rules(EscapeRules::Held, || {
            (EscapeRules::in_force(), RayleighFloorRules::in_force())
        });
        (
            (EscapeRules::in_force(), RayleighFloorRules::in_force()),
            held,
            GreenhouseConstants::in_force(),
        )
    });
    assert_eq!(batch, (EscapeRules::Batch, RayleighFloorRules::Floored));
    assert_eq!(held, (EscapeRules::Held, RayleighFloorRules::Held));
    assert_eq!(constants, GreenhouseConstants::BATCH);
    assert_eq!(
        (EscapeRules::in_force(), RayleighFloorRules::in_force()),
        outside
    );
}

// ---------------------------------------------------------------------------------------------
// Part B: the pure laws. Figures marked "state" are printed (`--nocapture`) as well as checked.
//
// Solar System figures are NASA's planetary and satellite fact sheets (NSSDCA, D. R. Williams;
// orbital and escape speeds, km s⁻¹, and semi-major axes, au; from memory): Mercury 47.36 and
// 4.25 at 0.387; Venus 35.02 and 10.36 at 0.723; Earth 29.78 and 11.19 at 1; Mars 24.07 and 5.03
// at 1.524; Jupiter 13.06 at 5.20; Saturn 9.68 at 9.58. The moons' orbital and escape speeds are
// √(GM ÷ a) and √(2 GM ÷ R) of JPL's planetary satellite parameters (from memory): Ganymede
// 10.880 and 2.741, Callisto 8.204 and 2.440, Titan 5.572 and 2.639, the Moon 1.022 and 2.38.
// ---------------------------------------------------------------------------------------------

use crate::planetary::derive::atmosphere::{
    EARTH_CARBON_DIOXIDE_PER_MASS, ICY_VOLATILE_BOOST, earth_xuv_fluence, energy_limited_loss,
    oxygen_escape_parameter, runaway_oxygen,
};
use crate::units::consts::{EARTH_MASS_KG, METRES_PER_AU, SOLAR_MASS_KG};
use crate::units::{SolarLuminosities, SolarMasses};
use hyperion_testkit::float::assert_same_bits;

/// The Avogadro constant, mol⁻¹ (exact in the 2019 SI).
const AVOGADRO_PER_MOL: f64 = 6.022_140_76e23;

/// One bar over Earth's surface, kg: 10⁵ Pa × 4π R⊕² ÷ g, g = G M⊕ ÷ R⊕².
fn earth_bar_kg() -> f64 {
    let g = GRAVITATIONAL_CONSTANT * EARTH_MASS_KG / (EARTH_RADIUS_M * EARTH_RADIUS_M);
    1e5 * 4.0 * PI * EARTH_RADIUS_M * EARTH_RADIUS_M / g
}

fn km_s(v: f64) -> MetresPerSecond {
    MetresPerSecond::new(v * 1e3)
}

fn earth() -> (Kilograms, Metres) {
    (Kilograms::new(EARTH_MASS_KG), Metres::new(EARTH_RADIUS_M))
}

/// Earth's ocean, kg (Charette and Smith 2010, as `EARTH_WATER_PER_MASS` cites it, rounded).
const OCEAN_KG: f64 = 1.4e21;

/// The present Sun's history at the crate's zero-age luminosity, 0.70 L☉.
fn sun() -> XuvHistory {
    XuvHistory::new(SolarMasses::new(1.0), SolarLuminosities::new(0.70))
}

/// Every ramp is continuous at its ends, with its slope: 0 and 1 there, and within 3ε² of them a
/// step ε inside.
#[test]
fn the_ramps_are_continuous_at_their_ends() {
    let eps = 1e-6;
    assert_same_bits(smoothstep(0.0), 0.0);
    assert_same_bits(smoothstep(1.0), 1.0);
    assert!(smoothstep(eps) <= 3.0 * eps * eps);
    assert!(1.0 - smoothstep(1.0 - eps) <= 3.0 * eps * eps + 1e-15);
    assert_same_bits(smoothstep(-1.0), 0.0);
    assert_same_bits(smoothstep(2.0), 1.0);
    assert_same_bits(smoothstep(f64::NAN), 0.0);
    assert!((smoothstep(0.5) - 0.5).abs() < 1e-15);
    // Through the laws: the shoreline at ±w, impacts at 4 and 5.
    let w = SHORELINE_HALF_WIDTH_DEX;
    assert!(xuv_shoreline_share(-w + 1e-9) < 1e-15);
    assert!(1.0 - xuv_shoreline_share(w - 1e-9) < 1e-15);
    assert!((xuv_shoreline_share(0.0) - 0.5).abs() < 1e-15);
    assert!(impact_shoreline_share(4.0 * (1.0 + 1e-9)) < 1e-15);
    assert!(1.0 - impact_shoreline_share(5.0 * (1.0 - 1e-9)) < 1e-15);
}

/// The exposure's fluence and flux are its history's, at the closed form's precision.
#[test]
fn the_exposure_is_its_history_seen_from_its_orbit() {
    for (mass, luminosity) in [(1.0, 0.7), (0.1, 1e-3), (0.5, 0.04)] {
        let history = XuvHistory::new(SolarMasses::new(mass), SolarLuminosities::new(luminosity));
        for (au, e) in [(1.0, 0.0), (0.02, 0.3), (5.2, 0.05)] {
            let a = Metres::new(au * METRES_PER_AU);
            let exposure = XuvExposure::of(&history, a, e);
            for age in [0.0, 3e6, 1e8, 4.2e8, 1e9, 4.57e9, 1.2e10] {
                let age = Years::new(age);
                let ours = exposure.fluence_between(Years::ZERO, age).value();
                let theirs = history.fluence(age, a, e).value();
                assert!(
                    (ours - theirs).abs() <= 1e-12 * theirs,
                    "{ours} against {theirs}"
                );
                let flux = exposure.flux_at(age).value();
                let their_flux = history.flux(age, a, e).value();
                assert!((flux - their_flux).abs() <= 1e-12 * their_flux);
            }
            let split = exposure
                .fluence_between(Years::ZERO, Years::new(2e8))
                .value()
                + exposure
                    .fluence_between(Years::new(2e8), Years::new(5e9))
                    .value();
            let whole = exposure
                .fluence_between(Years::ZERO, Years::new(5e9))
                .value();
            assert!((split - whole).abs() <= 1e-12 * whole);
        }
    }
    assert_same_bits(
        XuvExposure::NONE
            .fluence_between(Years::ZERO, Years::new(1e9))
            .value(),
        0.0,
    );
    assert_eq!(
        XuvExposure::new(WattsPerSquareMetre::new(f64::NAN), Years::new(1e8)),
        XuvExposure::NONE
    );
}

/// The lunar chronology's share of impacts: 0.499, 0.967, 0.9975, 0.9989 and 1 at 0.1, 0.5, 1, 2
/// and 4.5 Gyr; continuous and non-decreasing.
#[test]
fn the_impact_chronology_takes_its_impacts_early() {
    for (gyr, expected, tolerance) in [
        (0.1, 0.499, 5e-4),
        (0.5, 0.967, 5e-4),
        (1.0, 0.9975, 5e-5),
        (2.0, 0.9989, 5e-5),
        (4.5, 1.0, 0.0),
    ] {
        let share = impact_chronology(Years::new(gyr * 1e9));
        eprintln!("impact chronology at {gyr} Gyr: {share:.5}");
        assert!((share - expected).abs() <= tolerance, "{gyr} Gyr: {share}");
    }
    assert_same_bits(impact_chronology(Years::ZERO), 0.0);
    assert_same_bits(impact_chronology(Years::new(1e10)), 1.0);
    let mut last = 0.0;
    for k in 0..=4_600 {
        let share = impact_chronology(Years::new(f64::from(k) * 1e6));
        assert!(share >= last, "falls at {k} Myr");
        assert!(share - last < 0.008, "jumps at {k} Myr");
        last = share;
    }
    assert!(1.0 - impact_chronology(Years::new(4.5e9 - 1.0)) < 1e-12);
}

/// Zahnle et al.'s (2003) eq. 1 gives their Table 1's mean impact speeds on Ganymede, Callisto
/// and Titan, 20, 15 and 10.5 km s⁻¹, within 10% (state); Ganymede's and Callisto's impact ratios
/// are about 7.1 and 6.2, Titan's under 4.
#[test]
fn the_moons_impact_speeds_are_zahnle_et_al_s() {
    for (name, orbit, parent, escape, table, ratio) in [
        ("Ganymede", 10.880, 13.06, 2.741, 20.0, 7.1),
        ("Callisto", 8.204, 13.06, 2.440, 15.0, 6.2),
        ("Titan", 5.572, 9.68, 2.639, 10.5, 3.99),
    ] {
        let speed = moon_impact_speed(km_s(orbit), km_s(parent), km_s(escape)).value() / 1e3;
        eprintln!(
            "{name}: impact speed {speed:.2} km/s against Zahnle et al.'s {table} ({:+.1}%), \
             ratio {:.2}",
            100.0 * (speed / table - 1.0),
            speed / escape
        );
        assert!((speed / table - 1.0).abs() < 0.10);
        assert!((speed / escape - ratio).abs() < 0.05, "{name}");
    }
}

/// The bodies of the spec's shoreline cases: (name, fluence over Earth's, escape speed, impact
/// speed, age).
fn shoreline_bodies() -> Vec<(&'static str, f64, MetresPerSecond, MetresPerSecond, Years)> {
    let solar = Years::new(4.57e9);
    let planet = |orbit: f64, escape: f64| planet_impact_speed(km_s(orbit), km_s(escape));
    let moon = |orbit: f64, parent: f64, escape: f64| {
        moon_impact_speed(km_s(orbit), km_s(parent), km_s(escape))
    };
    let at = |au: f64| 1.0 / (au * au);
    // TRAPPIST-1 (Agol et al. 2021, PSJ 2, 1, Tables 2, 6 and 7, read): 0.0898 M☉, 5.53 × 10⁻⁴
    // L☉, b at 0.01154 au every 1.510826 d with 12.400 km/s, c at 0.01580 au every 2.421937 d with
    // 12.205 km/s; 7.6 Gyr old (Burgasser and Mamajek 2017, ApJ 845, 110; from memory). The
    // present luminosity is taken as the zero-age one.
    let trappist = XuvHistory::new(SolarMasses::new(0.0898), SolarLuminosities::new(5.53e-4));
    let earth_fluence = earth_xuv_fluence().value();
    let fluence = |history: &XuvHistory, au: f64, age: f64| {
        history
            .fluence(Years::new(age), Metres::new(au * METRES_PER_AU), 0.0)
            .value()
            / earth_fluence
    };
    let orbital = |au: f64, days: f64| {
        MetresPerSecond::new(2.0 * PI * au * METRES_PER_AU / (days * 86_400.0))
    };
    // LHS 3844 b (Vanderspek et al. 2019, ApJ Lett. 871, L24, arXiv:1809.07242v1, Table 1, read):
    // 0.151 M☉, 0.00272 L☉, 0.00623 au every 0.46292792 d, 1.32 R⊕; its mass 2.27 M⊕ (Hacker et al.
    // 2026, arXiv:2603.28237, abstract). Its age is not measured; its 128-day rotation marks an old,
    // inactive star, taken as 5 Gyr (the verdict holds from 0.21 Gyr).
    let lhs = XuvHistory::new(SolarMasses::new(0.151), SolarLuminosities::new(0.00272));
    let lhs_escape = MetresPerSecond::new(
        (2.0 * GRAVITATIONAL_CONSTANT * 2.27 * EARTH_MASS_KG / (1.32 * EARTH_RADIUS_M)).sqrt(),
    );
    let lhs_orbit = orbital(0.00623, 0.462_927_92);
    vec![
        ("Earth", at(1.0), km_s(11.19), planet(29.78, 11.19), solar),
        ("Venus", at(0.723), km_s(10.36), planet(35.02, 10.36), solar),
        ("Mars", at(1.524), km_s(5.03), planet(24.07, 5.03), solar),
        ("Mercury", at(0.387), km_s(4.25), planet(47.36, 4.25), solar),
        ("Moon", at(1.0), km_s(2.38), moon(1.022, 29.78, 2.38), solar),
        (
            "Ganymede",
            at(5.20),
            km_s(2.741),
            moon(10.880, 13.06, 2.741),
            solar,
        ),
        (
            "Callisto",
            at(5.20),
            km_s(2.440),
            moon(8.204, 13.06, 2.440),
            solar,
        ),
        (
            "Titan",
            at(9.58),
            km_s(2.639),
            moon(5.572, 9.68, 2.639),
            solar,
        ),
        (
            "TRAPPIST-1 b",
            fluence(&trappist, 0.01154, 7.6e9),
            km_s(12.400),
            planet_impact_speed(orbital(0.01154, 1.510_826), km_s(12.400)),
            Years::new(7.6e9),
        ),
        (
            "TRAPPIST-1 c",
            fluence(&trappist, 0.01580, 7.6e9),
            km_s(12.205),
            planet_impact_speed(orbital(0.01580, 2.421_937), km_s(12.205)),
            Years::new(7.6e9),
        ),
        (
            "LHS 3844 b",
            fluence(&lhs, 0.00623, 5e9),
            lhs_escape,
            planet_impact_speed(lhs_orbit, lhs_escape),
            Years::new(5e9),
        ),
    ]
}

/// The cosmic shoreline's verdicts on the spec's bodies (state the shares): Earth and Venus keep
/// their air to 10⁻⁹, Titan over 0.99, Mars under 0.05, Ganymede and Callisto none by the impact
/// term with the X-ray and ultraviolet term alone under ½, and the Moon, Mercury, TRAPPIST-1 b and
/// c and LHS 3844 b none.
#[test]
fn the_shoreline_keeps_the_solar_system_s_airs_and_strips_the_rest() {
    for (name, fluence, escape, impact, age) in shoreline_bodies() {
        let shoreline = Shoreline::of(fluence, escape, impact, age);
        let distance = shoreline_distance_dex(fluence, escape);
        let ratio = impact.value() / escape.value();
        eprintln!(
            "{name}: fluence {fluence:.4} F⊕, v_esc {:.2} km/s, {distance:+.3} dex, xuv {:.4}; \
             impact ratio {ratio:.2}, share {:.4}, chronology {:.4}; kept {:.3e}",
            escape.value() / 1e3,
            shoreline.xuv(),
            shoreline.impact(),
            shoreline.chronology(),
            shoreline.kept()
        );
        let kept = shoreline.kept();
        match name {
            "Earth" | "Venus" => assert!(kept >= 1.0 - 1e-9, "{name}"),
            "Titan" => assert!(kept > 0.99, "{name}"),
            "Mars" => {
                assert!(kept < 0.05, "{name}");
                assert!((ratio - 4.89).abs() < 0.02, "Mars's impact ratio {ratio}");
            }
            "Ganymede" | "Callisto" => {
                assert!(
                    1.0 - shoreline.impact() * shoreline.chronology() < 1e-9,
                    "{name}"
                );
                assert!(shoreline.xuv() < 0.5, "{name}");
            }
            _ => assert!(kept < 1e-9, "{name}"),
        }
    }
}

/// The shoreline's verdict is continuous in fluence and age, falls with fluence and with age.
#[test]
fn the_shoreline_is_continuous_and_monotone_in_fluence_and_age() {
    let (escape, impact) = (km_s(5.03), planet_impact_speed(km_s(24.07), km_s(5.03)));
    let mut last = f64::INFINITY;
    for k in 0..=4_000 {
        let fluence = math::exp10(-2.0 + f64::from(k) * 1e-3);
        let kept = Shoreline::of(fluence, escape, km_s(15.0), Years::new(4.57e9)).kept();
        assert!(kept <= last, "rises at {fluence}");
        assert!(
            last.is_infinite() || last - kept < 0.01,
            "jumps at {fluence}"
        );
        last = kept;
    }
    let mut last = f64::INFINITY;
    for k in 0..=1_000 {
        let age = Years::new(f64::from(k) * 5e6);
        let kept = Shoreline::of(0.43, escape, impact, age).kept();
        assert!(kept <= last, "rises at {} Myr", f64::from(k) * 5.0);
        assert!(
            last.is_infinite() || last - kept < 0.04,
            "jumps at {}",
            f64::from(k) * 5.0
        );
        last = kept;
    }
}

/// The binary diffusion coefficients give their sources' D = b ÷ n at one atmosphere: H₂–N₂ 0.61
/// cm² s⁻¹ at 300 K, CH₄–N₂ 0.214 at 298 K; Titan's methane at its 1.48% and 150 K escapes at about
/// 6 × 10⁹ cm⁻² s⁻¹ by the formula, twice Yelle et al.'s measured 2.5–3.0 × 10⁹ (state).
#[test]
fn the_diffusion_coefficients_and_titan_s_methane_limit_are_their_sources() {
    let density_cm3 = |t: f64| 101_325.0 / (BOLTZMANN_CONSTANT * t) * 1e-6;
    let d =
        |b: &BinaryDiffusion, t: f64| b.per_metre_second_at(Kelvin::new(t)) * 1e-2 / density_cm3(t);
    let hydrogen = d(&HYDROGEN_NITROGEN_DIFFUSION, 300.0);
    let methane = d(&METHANE_NITROGEN_DIFFUSION, 298.0);
    eprintln!("D(H₂–N₂, 300 K) {hydrogen:.3} cm²/s, D(CH₄–N₂, 298 K) {methane:.4} cm²/s");
    assert!((hydrogen - 0.61).abs() < 0.01);
    assert!((methane - 0.214).abs() < 0.001);
    let helium_b = HYDROGEN_HELIUM_DIFFUSION.per_metre_second_at(Kelvin::new(1_000.0));
    assert!((helium_b / (1.04e20 * math::powf(1_000.0, 0.732)) - 1.0).abs() < 1e-12);
    let (mass, radius) = (Kilograms::new(1.3452e23), Metres::new(2.574_73e6));
    let t = Kelvin::new(150.0);
    let flux = diffusion_limited_flux_per_m2_s(
        0.0148,
        Gas::Methane.molecular_mass(),
        Gas::Nitrogen.molecular_mass(),
        METHANE_NITROGEN_DIFFUSION.per_metre_second_at(t),
        mass,
        radius,
        t,
    ) * 1e-4;
    eprintln!(
        "Titan's methane diffusion limit {flux:.3e} cm⁻² s⁻¹, {:.2} times Yelle et al.'s 2.75e9",
        flux / 2.75e9
    );
    assert!((flux / 6e9 - 1.0).abs() < 0.1);
    // A heavier light gas, or none, does not diffuse out.
    let none = diffusion_limited_flux_per_m2_s(
        0.01,
        Gas::Nitrogen.molecular_mass(),
        Gas::Methane.molecular_mass(),
        1e20,
        mass,
        radius,
        t,
    );
    assert_same_bits(none, 0.0);
}

/// The crossover mass's fractionation factor is Hu et al.'s 1 − `φ_DL` `X_H` ÷ `φ_H`, held to [0, 1].
#[test]
fn the_fractionation_factor_is_hu_et_al_s() {
    let hydrogen = Kilograms::new(hydrogen_atom_mass());
    let helium = Gas::Helium.molecular_mass();
    let (coefficient, gravity, temperature, share) = (3.6e22, 17.0, Kelvin::new(3_000.0), 0.9);
    let limit = gravity * (helium.value() - hydrogen.value()) * coefficient
        / (BOLTZMANN_CONSTANT * temperature.value());
    let surface = MetresPerSecondSquared::new(gravity);
    for flux in [1e17, 1e18, 1e19, 1e20, 1e21] {
        let crossover = crossover_mass(flux, coefficient, surface, temperature, share);
        let factor = fractionation_factor(crossover, helium, hydrogen);
        let hu = (1.0 - limit * share / flux).clamp(0.0, 1.0);
        assert!((factor - hu).abs() < 1e-12, "{flux}: {factor} against {hu}");
    }
    assert_same_bits(
        fractionation_factor(Kilograms::new(f64::INFINITY), helium, hydrogen),
        1.0,
    );
    assert_same_bits(fractionation_factor(hydrogen, helium, hydrogen), 0.0);
    assert_same_bits(
        crossover_mass(0.0, coefficient, surface, temperature, share).value(),
        hydrogen.value(),
    );
}

/// Kepler-18 b (Malsky et al. 2023, Table 1: 6.99 M⊕, 2.00 R⊕, 451.56 F⊕ about a G star), taken at
/// the present Sun's 452 F⊕ about the crate's Sun, losing its energy-limited loss.
struct Kepler18 {
    mass: Kilograms,
    radius: Metres,
    exposure: XuvExposure,
    history: XuvHistory,
    orbit: Metres,
    hill: Metres,
}

impl Kepler18 {
    fn new() -> Self {
        let history = sun();
        let orbit = Metres::new(METRES_PER_AU / 452.0_f64.sqrt());
        let mass = Kilograms::new(6.99 * EARTH_MASS_KG);
        Self {
            mass,
            radius: Metres::new(2.00 * EARTH_RADIUS_M),
            exposure: XuvExposure::of(&history, orbit, 0.0),
            history,
            orbit,
            hill: Metres::new(orbit.value() * math::cbrt(mass.value() / (3.0 * SOLAR_MASS_KG))),
        }
    }

    fn lost_by(&self, age: Years) -> Kilograms {
        energy_limited_loss(
            self.mass,
            self.radius,
            self.history.fluence(age, self.orbit, 0.0),
            self.hill,
        )
    }

    fn helium(&self, fraction: f64, age: Years) -> EnvelopeHelium {
        envelope_helium(
            Kilograms::new(fraction * self.mass.value()),
            self.lost_by(age),
            self.mass,
            self.radius,
            &self.exposure,
            age,
            MALSKY_HELIUM,
        )
    }
}

/// Malsky et al.'s solar envelope, X 0.74, Y 0.24, Z 0.02, as helium's share of its hydrogen and
/// helium.
const MALSKY_HELIUM: f64 = 0.24 / 0.98;

/// No loss keeps the helium bit for bit; a loss far above the diffusion limit, x₂ → 1, keeps it to
/// 10⁻⁶.
#[test]
fn an_envelope_that_loses_nothing_or_all_together_keeps_its_helium() {
    let k = Kepler18::new();
    let envelope = Kilograms::new(0.007 * k.mass.value());
    let age = Years::new(5e9);
    for lost in [0.0, -1.0, f64::NAN] {
        let kept = envelope_helium(
            envelope,
            Kilograms::new(lost),
            k.mass,
            k.radius,
            &k.exposure,
            age,
            MALSKY_HELIUM,
        );
        assert_same_bits(kept.helium_mass_fraction(), MALSKY_HELIUM);
        assert_same_bits(kept.hydrogen_kept_share(), 1.0);
    }
    // Half the envelope in its first year.
    let fast = envelope_helium(
        envelope,
        Kilograms::new(0.5 * envelope.value()),
        k.mass,
        k.radius,
        &k.exposure,
        Years::new(1.0),
        MALSKY_HELIUM,
    );
    assert!((fast.helium_mass_fraction() - MALSKY_HELIUM).abs() < 1e-6);
    assert!((fast.hydrogen_kept_share() - 0.5).abs() < 1e-3);
}

/// Kepler-18 b's envelope draws, 0.1–1% of its mass, reach a helium mass fraction above 0.4 for
/// some by 5 Gyr (Malsky et al. find Y ≥ 0.4 after ≳ 2 Gyr at 3,000 K; state which).
#[test]
fn kepler_18_b_ends_rich_in_helium_for_some_draws() {
    let k = Kepler18::new();
    let age = Years::new(5e9);
    let lost = k.lost_by(age).value() / k.mass.value();
    eprintln!("Kepler-18 b loses {lost:.5} of its mass by 5 Gyr");
    let mut rich = 0;
    for step in 1..=10 {
        let fraction = 1e-3 * f64::from(step);
        let helium = k.helium(fraction, age);
        let two = k.helium(fraction, Years::new(2e9)).helium_mass_fraction();
        eprintln!(
            "Kepler-18 b, envelope {fraction:.3}: Y {:.4} at 5 Gyr ({two:.4} at 2 Gyr), He/H \
             {:.4}, hydrogen kept {:.4}, helium kept {:.4}",
            helium.helium_mass_fraction(),
            helium.helium_to_hydrogen(),
            helium.hydrogen_kept_share(),
            helium.helium_kept_share()
        );
        if helium.helium_mass_fraction() > 0.4 && helium.hydrogen_kept_share() > 0.0 {
            rich += 1;
        }
    }
    assert!(rich > 0, "no draw reaches Y > 0.4");
    for step in 0..=7 {
        let fraction = 0.0076 + 2e-4 * f64::from(step);
        let helium = k.helium(fraction, age);
        eprintln!(
            "  finer: envelope {fraction:.4}: Y {:.4} at 5 Gyr, hydrogen kept {:.4}",
            helium.helium_mass_fraction(),
            helium.hydrogen_kept_share()
        );
    }
}

/// An envelope's helium never falls with age, and is continuous in it across a grid node.
#[test]
fn an_envelope_s_helium_never_falls_with_age() {
    let k = Kepler18::new();
    for fraction in [0.005, 0.007, 0.01] {
        let mut last = 0.0;
        for step in 0..=400 {
            let age = Years::new(math::exp10(6.0 + f64::from(step) * 0.01));
            let y = k.helium(fraction, age).helium_mass_fraction();
            assert!(y >= last - 1e-12, "{fraction} falls at {} yr", age.value());
            last = y;
        }
    }
    let node = math::exp10(140.0 / 16.0);
    let below = k
        .helium(0.007, Years::new(node * (1.0 - 1e-9)))
        .helium_mass_fraction();
    let above = k
        .helium(0.007, Years::new(node * (1.0 + 1e-9)))
        .helium_mass_fraction();
    assert!((above - below).abs() < 1e-6);
}

/// The critical flux is where `oxygen_escape_parameter`'s x is 1: η = (x − 1) ÷ (x + 8) at twice
/// and ten times it, and some 0.18 W m⁻² for Earth.
#[test]
fn the_critical_flux_is_the_oxygen_escape_parameter_s() {
    let (mass, radius) = earth();
    let critical = critical_xuv_flux(mass, radius).value();
    eprintln!("Earth's critical flux {critical:.4} W/m²");
    assert!((critical - 0.178).abs() < 0.002);
    for x in [2.0, 10.0] {
        let eta = oxygen_escape_parameter(mass, radius, WattsPerSquareMetre::new(x * critical));
        assert!((eta - (x - 1.0) / (x + 8.0)).abs() < 1e-12);
    }
}

/// With no melt, a saturation outlasting the loss, and the sink over the whole age, the timed
/// route is `runaway_oxygen` to 10⁻⁹, below and above the critical flux.
#[test]
fn the_timed_route_is_the_single_route_at_a_constant_flux() {
    let (mass, radius) = earth();
    let water = Kilograms::new(OCEAN_KG);
    let age = Years::new(3e8);
    for flux in [0.1, 0.5, 5.0] {
        let exposure = XuvExposure::new(WattsPerSquareMetre::new(flux), Years::new(1e10));
        let timed = runaway_oxygen_timed(
            water,
            mass,
            radius,
            &exposure,
            MeltWindow::NONE,
            EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR,
            age,
        );
        let single = runaway_oxygen(
            water,
            mass,
            radius,
            WattsPerSquareMetre::new(flux),
            Years::ZERO,
            age,
        );
        assert!(single.value() > 0.0);
        assert!(
            (timed.kept().value() - single.value()).abs() <= 1e-9 * single.value(),
            "{flux}: {} against {}",
            timed.kept().value(),
            single.value()
        );
        assert!((timed.water_lost().value() - OCEAN_KG).abs() <= 1e-9 * OCEAN_KG);
        assert_oxygen_conserved(&timed);
    }
}

fn assert_oxygen_conserved(outcome: &OxygenOutcome) {
    let parts = outcome.dragged().value()
        + outcome.sequestered().value()
        + outcome.sunk().value()
        + outcome.kept().value();
    let freed = outcome.freed().value();
    assert!(
        (parts - freed).abs() <= 1e-12 * freed.max(1.0),
        "{outcome:?}"
    );
    for part in [
        outcome.water_lost(),
        outcome.dragged(),
        outcome.sequestered(),
        outcome.sunk(),
        outcome.kept(),
    ] {
        assert!(
            part.value() >= 0.0 && part.value().is_finite(),
            "{outcome:?}"
        );
    }
}

/// The plan's dimming M dwarf: an Earth at 0.02 au of a 0.1 M☉ star (10⁻³ L☉, saturated for 1
/// Gyr) with five oceans, whose mantle (Earth's, 4.01 × 10²⁴ kg at 8.05 wt% `FeO`) freezes at 50 Myr
/// while its water lasts beyond: it keeps only the oxygen left after 50 Myr, less its sink (state).
struct DimmingDwarf {
    exposure: XuvExposure,
    water: Kilograms,
    capacity: Kilograms,
}

impl DimmingDwarf {
    fn new() -> Self {
        let host = XuvHistory::new(SolarMasses::new(0.1), SolarLuminosities::new(1e-3));
        assert!((host.saturation_time().value() - 1e9).abs() < 1.0);
        // A quarter mole of O₂ per mole of FeO (71.844 g/mol) oxidised to FeO₁.₅.
        let capacity = 4.01e24 * 0.0805 / 71.844 * 0.25 * Gas::Oxygen.molar_mass_g_per_mol();
        Self {
            exposure: XuvExposure::of(&host, Metres::new(0.02 * METRES_PER_AU), 0.0),
            water: Kilograms::new(5.0 * OCEAN_KG),
            capacity: Kilograms::new(capacity),
        }
    }

    /// The oxygen left behind while the water lasts, kg yr⁻¹, by hand: 9 r k `F_c`, the flux
    /// being above the critical one; and the sink's take, kg yr⁻¹.
    fn rates() -> (f64, f64) {
        let (mass, radius) = earth();
        let k =
            WATER_LOSS_ESCAPE_EFFICIENCY * PI * EARTH_RADIUS_M * EARTH_RADIUS_M * EARTH_RADIUS_M
                / (GRAVITATIONAL_CONSTANT * EARTH_MASS_KG);
        let critical = critical_xuv_flux(mass, radius).value();
        (
            9.0 * oxygen_per_water() * k * critical * SECONDS_PER_JULIAN_YEAR,
            oxygen_sink_kg_per_year(EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR, EARTH_RADIUS_M),
        )
    }

    fn outcome(&self, melt_until: f64, age: f64) -> OxygenOutcome {
        let (mass, radius) = earth();
        runaway_oxygen_timed(
            self.water,
            mass,
            radius,
            &self.exposure,
            MeltWindow {
                until: Years::new(melt_until),
                capacity: self.capacity,
            },
            EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR,
            Years::new(age),
        )
    }
}

#[test]
fn a_dimming_m_dwarf_s_planet_keeps_only_the_oxygen_left_after_its_mantle_froze() {
    let dwarf = DimmingDwarf::new();
    let (mass, radius) = earth();
    let saturated = dwarf.exposure.saturated().value();
    let critical = critical_xuv_flux(mass, radius).value();
    assert!(saturated > critical);
    let k = WATER_LOSS_ESCAPE_EFFICIENCY * PI * EARTH_RADIUS_M * EARTH_RADIUS_M * EARTH_RADIUS_M
        / (GRAVITATIONAL_CONSTANT * EARTH_MASS_KG);
    // By hand: the flux is constant while the water lasts, inside the saturation.
    let dry_years =
        dwarf.water.value() / (k * (saturated + 8.0 * critical) * SECONDS_PER_JULIAN_YEAR);
    let (accrual, sink) = DimmingDwarf::rates();
    eprintln!(
        "dimming M dwarf: F_s {saturated:.3} W/m² against F_c {critical:.4}; water gone at {:.1} \
         Myr; oxygen left at {:.3e} kg/yr, sink {sink:.3e} kg/yr; melt capacity {:.3e} kg",
        dry_years / 1e6,
        accrual,
        dwarf.capacity.value()
    );
    assert!(dry_years > 5e7);
    for age in [1e8, 2e8, 1e9, 4e9, 8e9] {
        let outcome = dwarf.outcome(5e7, age);
        assert_oxygen_conserved(&outcome);
        let expected = (accrual * (dry_years.min(age) - 5e7) - sink * (age - 5e7)).max(0.0);
        eprintln!(
            "  at {:.0} Myr: water lost {:.3e} kg, freed {:.3e}, dragged {:.3e}, sequestered \
             {:.3e}, sunk {:.3e}, kept {:.3e} kg ({:.0} bar)",
            age / 1e6,
            outcome.water_lost().value(),
            outcome.freed().value(),
            outcome.dragged().value(),
            outcome.sequestered().value(),
            outcome.sunk().value(),
            outcome.kept().value(),
            outcome.kept().value() / earth_bar_kg()
        );
        assert!(
            (outcome.kept().value() - expected).abs() <= 1e-9 * dwarf.water.value(),
            "{age}: {} against {expected}",
            outcome.kept().value()
        );
        assert!((outcome.sequestered().value() - accrual * 5e7).abs() <= 1e-9 * accrual * 5e7);
    }
}

/// The dimming M dwarf's oxygen is continuous and non-increasing in the melt's end, and rises and
/// then falls with age as the sink takes over (state the peak).
#[test]
fn the_runaway_oxygen_falls_with_a_later_freeze_and_peaks_in_age() {
    let dwarf = DimmingDwarf::new();
    let age = 4e8;
    let mut last = f64::INFINITY;
    let step = 1e6;
    let (accrual, sink) = DimmingDwarf::rates();
    let biggest = 1.01 * (accrual + sink) * step;
    for k in 0..=300 {
        let kept = dwarf.outcome(f64::from(k) * step, age).kept().value();
        assert!(kept <= last * (1.0 + 1e-12), "rises at {k} Myr");
        assert!(
            last.is_infinite() || last - kept <= biggest,
            "jumps at {k} Myr"
        );
        last = kept;
    }
    let mut ages = Vec::new();
    for k in 0..=1_000 {
        let age = 1e7 + f64::from(k) * 1e7;
        ages.push((age, dwarf.outcome(5e7, age).kept().value()));
    }
    let peak = ages
        .iter()
        .copied()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("ages are sampled");
    eprintln!(
        "the dimming M dwarf's oxygen peaks at {:.0} Myr with {:.3e} kg ({:.0} bar)",
        peak.0 / 1e6,
        peak.1,
        peak.1 / earth_bar_kg()
    );
    for pair in ages.windows(2) {
        let ((_, before), (when, after)) = (pair[0], pair[1]);
        if when <= peak.0 {
            assert!(after >= before, "falls before the peak at {when}");
        } else {
            assert!(after <= before, "rises after the peak at {when}");
        }
    }
}

/// Wordsworth and Pierrehumbert's (2014) moisture number puts the moist line near their Fig. 1's
/// 340 K over 1 bar of nitrogen, 295 K over 0.1 bar and 255 K over 0.01 bar (state, within 8 K).
#[test]
fn the_moist_line_is_wordsworth_and_pierrehumbert_s() {
    // Water's latent heat of vaporisation, J kg⁻¹: 2.501 × 10⁶ − 2,370 (T − 273.15) (the linear fit
    // of Rogers and Yau 1989, A Short Course in Cloud Physics, 3rd ed.; from memory); nitrogen's
    // c_p 1,040 J kg⁻¹ K⁻¹ (NIST, ideal gas at 300 K; from memory).
    let air = |background: f64, t: f64| MoistAir {
        vapour_pressure: crate::substance::saturation_pressure(
            Gas::Water.substance(),
            Kelvin::new(t),
        )
        .expect("water has a phase diagram"),
        background_pressure: Pascals::new(background),
        background_molar_mass_g_per_mol: Gas::Nitrogen.molar_mass_g_per_mol(),
        background_heat_capacity_j_per_kg_k: 1_040.0,
        latent_heat_j_per_kg: 2.501e6 - 2_370.0 * (t - 273.15),
        surface_temperature: Kelvin::new(t),
    };
    for (background, theirs) in [(1e5, 340.0), (1e4, 295.0), (1e3, 255.0)] {
        let (mut low, mut high) = (150.0, 450.0);
        for _ in 0..60 {
            let mid = f64::midpoint(low, high);
            if moist_upper_atmosphere(&air(background, mid)) < 1.0 {
                low = mid;
            } else {
                high = mid;
            }
        }
        eprintln!(
            "moist above {low:.1} K over {} bar of N₂ (their {theirs} K); M at theirs {:.3}",
            background / 1e5,
            moist_upper_atmosphere(&air(background, theirs))
        );
        assert!((low - theirs).abs() < 8.0);
    }
    assert!(moist_upper_atmosphere(&air(0.0, 288.0)).is_infinite());
}

/// Earth's air at 288 K over `nitrogen` Pa of nitrogen.
fn earth_air(nitrogen: f64) -> MoistAir {
    let t = 288.0;
    MoistAir {
        vapour_pressure: crate::substance::saturation_pressure(
            Gas::Water.substance(),
            Kelvin::new(t),
        )
        .expect("water has a phase diagram"),
        background_pressure: Pascals::new(nitrogen),
        background_molar_mass_g_per_mol: Gas::Nitrogen.molar_mass_g_per_mol(),
        background_heat_capacity_j_per_kg_k: 1_040.0,
        latent_heat_j_per_kg: 2.501e6 - 2_370.0 * (t - 273.15),
        surface_temperature: Kelvin::new(t),
    }
}

/// An Earth with 0.01 bar of nitrogen at 288 K builds oxygen over 4 Gyr (state how much, at Earth's
/// sink, at Wordsworth and Pierrehumbert's 5 × 10⁹ molecules cm⁻² s⁻¹ and at none); with 0.78 bar
/// it builds none. At Earth's sink, sixteen times theirs, the balance drains its ocean, after which
/// the sink takes the air's oxygen; at theirs and at none it keeps an oxygen air.
#[test]
fn a_nitrogen_poor_earth_builds_oxygen_and_earth_none() {
    let (mass, radius) = earth();
    let exposure = XuvExposure::of(&sun(), Metres::new(METRES_PER_AU), 0.0);
    let water = Kilograms::new(OCEAN_KG);
    let age = Years::new(4e9);
    let theirs = 5e13 * SECONDS_PER_JULIAN_YEAR / AVOGADRO_PER_MOL;
    let gigayear = Years::new(1e9);
    let poor = earth_air(1e3);
    eprintln!(
        "N₂-poor Earth: M {:.2}, vapour {:.0} Pa",
        moist_upper_atmosphere(&poor),
        poor.vapour_pressure.value()
    );
    for (label, sink) in [
        ("Earth's sink", EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR),
        ("their sink", theirs),
        ("no sink", 0.0),
    ] {
        let outcome = cold_trap_failure_oxygen(water, mass, radius, &poor, &exposure, sink, age);
        assert_oxygen_conserved(&outcome);
        eprintln!(
            "  {label} ({sink:.3e} mol/m²/yr), 4 Gyr: O₂ kept {:.4} bar, made {:.2} bar, sunk \
             {:.2} bar, water lost {:.4} oceans, M at the end {:.3}",
            outcome.kept().value() / earth_bar_kg(),
            outcome.freed().value() / earth_bar_kg(),
            outcome.sunk().value() / earth_bar_kg(),
            outcome.water_lost().value() / OCEAN_KG,
            moisture(
                &poor,
                outcome.kept().value() * GRAVITATIONAL_CONSTANT * EARTH_MASS_KG
                    / (EARTH_RADIUS_M * EARTH_RADIUS_M)
                    / (4.0 * PI * EARTH_RADIUS_M * EARTH_RADIUS_M)
            )
        );
        let early = cold_trap_failure_oxygen(water, mass, radius, &poor, &exposure, sink, gigayear);
        eprintln!(
            "    at 1 Gyr: O₂ kept {:.4} bar, made {:.2} bar, water lost {:.4} oceans",
            early.kept().value() / earth_bar_kg(),
            early.freed().value() / earth_bar_kg(),
            early.water_lost().value() / OCEAN_KG
        );
        assert!(outcome.freed().value() > 0.0, "{label}");
        if sink < EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR {
            assert!(outcome.kept().value() > 0.0, "{label}");
        }
        assert_same_bits(outcome.dragged().value(), 0.0);
    }
    let rich = cold_trap_failure_oxygen(
        water,
        mass,
        radius,
        &earth_air(7.8e4),
        &exposure,
        EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR,
        age,
    );
    assert_same_bits(rich.freed().value(), 0.0);
    assert_same_bits(rich.kept().value(), 0.0);
}

/// A moist air that loses all its water loses its inventory to the bit, never more: the law's water
/// lost is the inventory less what is left (P14.T53.a's review: a drained world whose loss
/// overshot its water by a rounding turned `Airless`). An Earth under 0.1 bar of nitrogen and 0.1
/// bar of vapour at 300 K is moist (M ≈ 5), and with a surface sink that takes all its oxygen it
/// loses 10¹⁹–10²⁰ kg of water at the diffusion limit over tens of time steps, all of it, each to
/// the bit.
#[test]
fn a_cold_trap_that_drains_its_water_loses_its_inventory_to_the_bit() {
    let (mass, radius) = earth();
    let exposure = XuvExposure::of(&sun(), Metres::new(METRES_PER_AU), 0.0);
    let air = MoistAir {
        vapour_pressure: Pascals::new(1e4),
        background_pressure: Pascals::new(1e4),
        background_molar_mass_g_per_mol: Gas::Nitrogen.molar_mass_g_per_mol(),
        background_heat_capacity_j_per_kg_k: 1_040.0,
        latent_heat_j_per_kg: 2.44e6,
        surface_temperature: Kelvin::new(300.0),
    };
    let moisture = moist_upper_atmosphere(&air);
    eprintln!("moisture number {moisture:.3}");
    assert!(moisture > MOIST_RAMP.1);
    for water in [1e19, 2.7e19, 4.1e19, 6.3e19, 1e20] {
        let outcome = cold_trap_failure_oxygen(
            Kilograms::new(water),
            mass,
            radius,
            &air,
            &exposure,
            1e3,
            Years::new(4.57e9),
        );
        eprintln!(
            "{water:e} kg: lost {:e} kg, O₂ kept {:e} kg",
            outcome.water_lost().value(),
            outcome.kept().value()
        );
        assert_same_bits(outcome.water_lost().value(), water);
        assert!((outcome.freed().value() / (oxygen_per_water() * water) - 1.0).abs() < 1e-12);
    }
}

/// The water's diffusion limit through nitrogen lies below the rate at which hydrogen drags oxygen,
/// at 0.37 of it for f = 1, on a body of any mass.
#[test]
fn the_cold_trap_s_diffusion_limit_drags_no_oxygen() {
    for (mass_earths, radius_earths) in [(1.0, 1.0), (0.1, 0.5), (5.0, 1.6)] {
        let (mass, radius) = (
            Kilograms::new(mass_earths * EARTH_MASS_KG),
            Metres::new(radius_earths * EARTH_RADIUS_M),
        );
        let loss = WaterLoss::new(mass, radius, &XuvExposure::NONE).expect("a body");
        let r = radius.value();
        let g = GRAVITATIONAL_CONSTANT * mass.value() / (r * r);
        let water = Gas::Water.molecular_mass().value();
        let separation = Gas::Nitrogen.molecular_mass().value() - water;
        let full = WATER_AIR_DIFFUSION.per_metre_second_at(COLD_TRAP_HOMOPAUSE_TEMPERATURE)
            * g
            * separation
            / (BOLTZMANN_CONSTANT * COLD_TRAP_HOMOPAUSE_TEMPERATURE.value())
            * 4.0
            * PI
            * r
            * r
            * water;
        let ratio = full / loss.drag_free_rate();
        eprintln!("{mass_earths} M⊕: diffusion limit over drag-free rate {ratio:.3}");
        assert!((ratio - 0.37).abs() < 0.02);
    }
}

/// Titan at its median carbon (100 × Earth's per unit mass beyond the snow line, as methane mole
/// for mole) keeps more than half its methane over 4.57 Gyr, some 80% (state the split).
#[test]
fn titan_keeps_most_of_its_methane() {
    let (mass, radius) = (Kilograms::new(1.3452e23), Metres::new(2.574_73e6));
    let reservoir = Kilograms::new(
        EARTH_CARBON_DIOXIDE_PER_MASS
            * ICY_VOLATILE_BOOST
            * mass.value()
            * Gas::Methane.molar_mass_g_per_mol()
            / Gas::CarbonDioxide.molar_mass_g_per_mol(),
    );
    let titan = |age: f64| {
        methane_outcome(
            reservoir,
            0.0148,
            Kelvin::new(150.0),
            mass,
            radius,
            1.0,
            Years::new(age),
        )
    };
    let outcome = titan(4.57e9);
    let share = |m: Kilograms| m.value() / reservoir.value();
    eprintln!(
        "Titan's methane ({:.3e} kg) over 4.57 Gyr: escaped {:.3}, photolysed {:.3}, kept {:.3}; \
         Titan's insolation {:.2} W/m²",
        reservoir.value(),
        share(outcome.escaped()),
        share(outcome.photolysed()),
        share(outcome.kept()),
        TITAN_INSOLATION.value()
    );
    assert!(share(outcome.kept()) > 0.5);
    assert!((share(outcome.kept()) - 0.8).abs() < 0.1);
    let sum = outcome.escaped().value() + outcome.photolysed().value() + outcome.kept().value();
    assert!((sum - reservoir.value()).abs() <= 1e-12 * reservoir.value());
    // A brighter body runs out: its two sinks share the reservoir by their rates.
    let bright = methane_outcome(
        reservoir,
        0.0148,
        Kelvin::new(150.0),
        mass,
        radius,
        100.0,
        Years::new(4.57e9),
    );
    assert_same_bits(bright.kept().value(), 0.0);
    assert!(
        (bright.escaped().value() + bright.photolysed().value() - reservoir.value()).abs()
            <= 1e-12 * reservoir.value()
    );
    let mut last = f64::INFINITY;
    for k in 0..=100 {
        let kept = titan(f64::from(k) * 5e7).kept().value();
        assert!(kept <= last);
        last = kept;
    }
}

/// Gao et al.'s (2015) conversion: 40% under an M dwarf at and below 1 ppm of hydrogen, none from
/// 10 ppm, continuous between; atoms and mass conserved; none under a hotter star.
#[test]
fn a_desiccated_carbon_dioxide_air_under_an_m_dwarf_turns_two_fifths_to_co_and_oxygen() {
    let dwarf = UltravioletHost::of(Kelvin::new(3_400.0));
    assert_eq!(dwarf, UltravioletHost::MDwarf);
    assert_eq!(
        UltravioletHost::of(Kelvin::new(3_900.0)),
        UltravioletHost::Hotter
    );
    let co = |h: f64| desiccated_carbon_dioxide(1_000.0, h, dwarf).carbon_monoxide_mol();
    assert!((co(1e-7) - 400.0).abs() < 1e-9);
    assert!((co(1e-6) - 400.0).abs() < 1e-9);
    assert!((co(0.0) - 400.0).abs() < 1e-9);
    assert!(co(1e-5) <= 0.0);
    assert!(co(1e-6 * (1.0 + 1e-9)) > 400.0 * (1.0 - 1e-12));
    assert!(co(1e-5 * (1.0 - 1e-9)) < 1e-9);
    let mut last = f64::INFINITY;
    for k in 0..=200 {
        let converted = co(math::exp10(-7.0 + f64::from(k) * 0.015));
        assert!(converted <= last);
        assert!(last.is_infinite() || last - converted < 10.0);
        last = converted;
    }
    for h in [1e-8, 3e-6, 1e-4] {
        let split = desiccated_carbon_dioxide(1_000.0, h, dwarf);
        let carbon = split.carbon_dioxide_mol() + split.carbon_monoxide_mol();
        let oxygen = 2.0 * split.carbon_dioxide_mol()
            + split.carbon_monoxide_mol()
            + 2.0 * split.oxygen_mol();
        assert!((carbon - 1_000.0).abs() < 1e-9 && (oxygen - 2_000.0).abs() < 1e-9);
        let carbon_weight = Element::C.standard_atomic_weight_g_per_mol();
        let oxygen_weight = Element::O.standard_atomic_weight_g_per_mol();
        let dioxide = carbon_weight + 2.0 * oxygen_weight;
        let after = split.carbon_dioxide_mol() * dioxide
            + split.carbon_monoxide_mol() * (carbon_weight + oxygen_weight)
            + split.oxygen_mol() * 2.0 * oxygen_weight;
        assert!((after - 1_000.0 * dioxide).abs() < 1e-9 * after);
    }
    assert!(
        desiccated_carbon_dioxide(1_000.0, 1e-8, UltravioletHost::Hotter).carbon_monoxide_mol()
            <= 0.0
    );
    eprintln!(
        "Gao et al.: converted {:.3} at 0.1 ppm, {:.3} at 1 ppm, {:.3} at 3 ppm, {:.3} at 10 ppm",
        co(1e-7) / 1e3,
        co(1e-6) / 1e3,
        co(3e-6) / 1e3,
        co(1e-5) / 1e3
    );
}

/// A thin envelope's hydrogen and helium are kept as air below the line and none at or above it.
#[test]
fn a_thin_envelope_is_kept_as_air() {
    let mass = Kilograms::new(EARTH_MASS_KG);
    let (h2, he) = thin_envelope_air(1e-4, mass, 0.27);
    assert!((h2.value() - 0.73e-4 * EARTH_MASS_KG).abs() < 1e-6 * h2.value());
    assert!((he.value() - 0.27e-4 * EARTH_MASS_KG).abs() < 1e-6 * he.value());
    for fraction in [THIN_ENVELOPE_FRACTION, 0.01, 0.0, -1.0, f64::NAN] {
        let (h2, he) = thin_envelope_air(fraction, mass, 0.27);
        assert_same_bits(h2.value(), 0.0);
        assert_same_bits(he.value(), 0.0);
    }
}

/// Each element's parts, from the laws, sum to its inventory, and the outcome refuses an element
/// out of place.
#[test]
fn each_element_s_outcome_sums_to_its_inventory() {
    let kg = Kilograms::new;
    // Hydrogen and helium: Kepler-18 b's 0.7% envelope.
    let k = Kepler18::new();
    let envelope = 0.007 * k.mass.value();
    let helium = k.helium(0.007, Years::new(5e9));
    let hydrogen_total = (1.0 - MALSKY_HELIUM) * envelope;
    let helium_total = MALSKY_HELIUM * envelope;
    let hydrogen_kept = helium.hydrogen_kept_share() * hydrogen_total;
    let helium_kept = helium.helium_kept_share() * helium_total;
    // Carbon: Titan's methane, by carbon's share of its mass.
    let (mass, radius) = (kg(1.3452e23), Metres::new(2.574_73e6));
    let reservoir = kg(2.2e-3 * mass.value());
    let methane = methane_outcome(
        reservoir,
        0.0148,
        Kelvin::new(150.0),
        mass,
        radius,
        1.0,
        Years::new(4.57e9),
    );
    let carbon =
        Element::C.standard_atomic_weight_g_per_mol() / Gas::Methane.molar_mass_g_per_mol();
    // Oxygen: the dimming M dwarf's.
    let oxygen = DimmingDwarf::new().outcome(5e7, 2e8);
    let elements = [
        ElementOutcome::new(
            Element::H,
            kg(hydrogen_total - hydrogen_kept),
            kg(hydrogen_kept),
            kg(0.0),
            kg(0.0),
        ),
        ElementOutcome::new(
            Element::He,
            kg(helium_total - helium_kept),
            kg(helium_kept),
            kg(0.0),
            kg(0.0),
        ),
        ElementOutcome::new(
            Element::C,
            methane.escaped() * carbon,
            methane.kept() * carbon,
            kg(0.0),
            methane.photolysed() * carbon,
        ),
        ElementOutcome::new(Element::N, kg(0.0), kg(3.9e18), kg(0.0), kg(0.0)),
        ElementOutcome::new(
            Element::O,
            oxygen.dragged(),
            oxygen.kept(),
            oxygen.sequestered() + oxygen.sunk(),
            kg(0.0),
        ),
        ElementOutcome::new(Element::Ar, kg(0.0), kg(6.6e16), kg(0.0), kg(0.0)),
    ];
    let outcome = EscapeOutcome::new(elements, Some(helium), Shoreline::NONE, oxygen, kg(0.0))
        .expect("the elements are in place");
    for (element, inventory) in [
        (Element::H, hydrogen_total),
        (Element::He, helium_total),
        (Element::C, reservoir.value() * carbon),
        (Element::N, 3.9e18),
        (Element::O, oxygen.freed().value()),
        (Element::Ar, 6.6e16),
    ] {
        let sum = outcome
            .element(element)
            .expect("recorded")
            .inventory()
            .value();
        assert!(
            (sum - inventory).abs() <= 1e-12 * inventory,
            "{element:?}: {sum} against {inventory}"
        );
    }
    assert!(outcome.element(Element::Ne).is_none());
    let mut swapped = elements;
    swapped.swap(0, 1);
    assert_eq!(
        EscapeOutcome::new(swapped, None, Shoreline::NONE, OxygenOutcome::NONE, kg(0.0)),
        Err(BuildEscapeOutcomeError::ElementOutOfPlace {
            index: 0,
            expected: Element::H,
            found: Element::He,
        })
    );
}

/// The inputs no law may answer with a number that is not one: not finite, or not positive.
const BAD_INPUTS: [f64; 5] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0];

fn assert_not_nan(x: f64, what: &str) {
    assert!(!x.is_nan(), "{what}");
}

/// The shoreline's and diffusion's laws answer a bad input with a number.
#[test]
fn the_shoreline_and_diffusion_laws_answer_a_bad_input_with_a_number() {
    let (mass, radius) = earth();
    let exposure = XuvExposure::of(&sun(), Metres::new(METRES_PER_AU), 0.0);
    let (hydrogen, helium) = (Gas::Hydrogen.molecular_mass(), Gas::Helium.molecular_mass());
    for x in BAD_INPUTS {
        let speed = MetresPerSecond::new(x);
        assert_not_nan(shoreline_distance_dex(x, km_s(5.0)), "distance, fluence");
        assert_not_nan(shoreline_distance_dex(1.0, speed), "distance, speed");
        assert_not_nan(xuv_shoreline_share(x), "xuv share");
        assert_not_nan(impact_shoreline_share(x), "impact share");
        assert_not_nan(impact_chronology(Years::new(x)), "chronology");
        assert_not_nan(planet_impact_speed(speed, km_s(5.0)).value(), "planet");
        assert_not_nan(
            moon_impact_speed(km_s(5.0), speed, km_s(2.0)).value(),
            "moon",
        );
        assert_not_nan(
            Shoreline::of(x, speed, km_s(10.0), Years::new(x)).kept(),
            "kept",
        );
        let t = Kelvin::new(x);
        assert_not_nan(
            diffusion_limited_flux_per_m2_s(x, hydrogen, helium, x, mass, radius, t),
            "diffusion",
        );
        // The masses too, each through the module's `positive`: no flux, no fractionation.
        let bad = Kilograms::new(x);
        let warm = Kelvin::new(300.0);
        for (light, heavy) in [(bad, helium), (hydrogen, bad), (bad, bad)] {
            assert_same_bits(
                diffusion_limited_flux_per_m2_s(0.5, light, heavy, 1e20, mass, radius, warm),
                0.0,
            );
        }
        let atom = Kilograms::new(hydrogen_atom_mass());
        assert_not_nan(
            fractionation_factor(Kilograms::new(x), helium, atom),
            "fractionation",
        );
        let crossover = Kilograms::new(1e-24);
        for (heavy, light) in [(bad, atom), (helium, bad), (bad, bad)] {
            assert_same_bits(fractionation_factor(crossover, heavy, light), 0.0);
        }
        let gravity = MetresPerSecondSquared::new(x);
        assert_not_nan(crossover_mass(x, x, gravity, t, x).value(), "crossover");
        assert_not_nan(
            exposure.fluence_between(Years::ZERO, Years::new(x)).value(),
            "fluence",
        );
        assert_not_nan(exposure.flux_at(Years::new(x)).value(), "flux");
    }
}

/// The envelope's and oxygen's laws answer a bad input with a number.
#[test]
fn the_envelope_and_oxygen_laws_answer_a_bad_input_with_a_number() {
    let (mass, radius) = earth();
    let exposure = XuvExposure::of(&sun(), Metres::new(METRES_PER_AU), 0.0);
    let ocean = Kilograms::new(OCEAN_KG);
    for x in BAD_INPUTS {
        let (bad_mass, bad_age) = (Kilograms::new(x), Years::new(x));
        let helium = envelope_helium(
            Kilograms::new(1e22),
            bad_mass,
            mass,
            radius,
            &exposure,
            bad_age,
            x,
        );
        for value in [
            helium.helium_mass_fraction(),
            helium.helium_to_hydrogen(),
            helium.hydrogen_kept_share(),
            helium.helium_kept_share(),
        ] {
            assert_not_nan(value, "envelope helium");
        }
        let melt = MeltWindow {
            until: bad_age,
            capacity: bad_mass,
        };
        let thin = MoistAir {
            background_pressure: Pascals::new(x),
            ..earth_air(1e3)
        };
        let gigayear = Years::new(1e9);
        for outcome in [
            runaway_oxygen_timed(bad_mass, mass, radius, &exposure, melt, x, bad_age),
            runaway_oxygen_timed(ocean, mass, radius, &exposure, melt, x, gigayear),
            cold_trap_failure_oxygen(ocean, mass, radius, &thin, &exposure, x, gigayear),
            cold_trap_failure_oxygen(bad_mass, mass, radius, &thin, &exposure, 0.0, bad_age),
        ] {
            for part in [
                outcome.water_lost(),
                outcome.freed(),
                outcome.dragged(),
                outcome.sequestered(),
                outcome.sunk(),
                outcome.kept(),
            ] {
                assert_not_nan(part.value(), "oxygen");
            }
        }
        let unheated = MoistAir {
            latent_heat_j_per_kg: x,
            ..earth_air(x)
        };
        assert_not_nan(moist_upper_atmosphere(&unheated), "moisture");
    }
}

/// Carbon's and the thin envelope's laws answer a bad input with a number.
#[test]
fn the_carbon_and_thin_envelope_laws_answer_a_bad_input_with_a_number() {
    let (mass, radius) = earth();
    for x in BAD_INPUTS {
        let methane = methane_outcome(
            Kilograms::new(1e20),
            x,
            Kelvin::new(x),
            mass,
            radius,
            x,
            Years::new(x),
        );
        let total =
            methane.kept().value() + methane.escaped().value() + methane.photolysed().value();
        assert_not_nan(total, "methane");
        let split = desiccated_carbon_dioxide(x, x, UltravioletHost::MDwarf);
        let moles = split.carbon_monoxide_mol() + split.carbon_dioxide_mol() + split.oxygen_mol();
        assert_not_nan(moles, "Gao");
        let (h2, he) = thin_envelope_air(x, Kilograms::new(x), x);
        assert_not_nan(h2.value() + he.value(), "thin envelope");
    }
}
