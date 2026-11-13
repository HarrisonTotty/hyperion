use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

use super::*;
use crate::GENERATOR_VERSION;
use crate::planetary::chem::AbundanceDraws;
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::radius::{CoreComposition, EARTH_CORE_MASS_FRACTION};
use crate::planetary::derive::solar::{PLANETS, SOLAR_AGE, solar_disc, solar_system};
use crate::units::consts::{METRES_PER_AU, STEFAN_BOLTZMANN};

/// The elements of an envelope other than hydrogen and helium.
fn metals() -> impl Iterator<Item = Element> {
    Element::ALL
        .into_iter()
        .filter(|e| !matches!(e, Element::H | Element::He))
}

/// A body's effective temperature from its record, its equilibrium temperature where it has no
/// luminosity of its own.
fn effective_temperature(body: &DerivedBody) -> Kelvin {
    body.effective_temperature()
        .unwrap_or_else(|| body.equilibrium_temperature())
}

/// The table's giant `name`, derived about the present Sun at its own distance, and its
/// envelope's inputs about a host of `host`'s ratios at solar metallicity.
pub(crate) fn table_giant(name: &str, host: HostAbundances) -> (DerivedBody, EnvelopeInputs) {
    let &(_, _, _, a, _) = PLANETS
        .iter()
        .find(|(n, ..)| *n == name)
        .expect("the giant is in the table");
    let (_, body) = solar_system()
        .into_iter()
        .find(|(n, _)| *n == name)
        .expect("the giant is in the table");
    let inputs = EnvelopeInputs {
        mass: body.mass(),
        fractions: body.fractions(),
        formation_distance: Metres::new(a * METRES_PER_AU),
        snow_line: solar_disc().snow_line(),
        fe_h: Dex::ZERO,
        alpha_fe: Dex::ZERO,
        host,
        effective_temperature: effective_temperature(&body),
        age: SOLAR_AGE,
        surface_gravity: body.surface_gravity(),
        primordial_helium_per_hydrogen: Element::He.protosolar_abundance_per_hydrogen(),
    };
    (body, inputs)
}

/// A body of `earths` Earth masses whose envelope is `envelope` of its mass on an Earth-like core,
/// formed where its disc was at `formation` (a snow line of 2.255 au), about a host of `fe_h` and
/// `alpha_fe` at the median ranks, at an effective temperature of `t_eff`, Jupiter's surface
/// gravity and the Sun's age, with protosolar helium.
pub(crate) fn synthetic(
    earths: f64,
    envelope: f64,
    formation: f64,
    (fe_h, alpha_fe): (f64, f64),
    t_eff: f64,
) -> EnvelopeInputs {
    let snow = 2.255 * METRES_PER_AU;
    let ratio = SNOW_LINE_TEMPERATURE.value() / formation;
    EnvelopeInputs {
        mass: EarthMasses::new(earths),
        fractions: MassFractions::of(
            CoreComposition::from_fractions(EARTH_CORE_MASS_FRACTION, 0.0),
            envelope,
        ),
        formation_distance: Metres::new(snow * ratio * ratio),
        snow_line: Metres::new(snow),
        fe_h: Dex::new(fe_h),
        alpha_fe: Dex::new(alpha_fe),
        host: HostAbundances::of(Dex::new(fe_h), &AbundanceDraws::MEDIAN),
        effective_temperature: Kelvin::new(t_eff),
        age: SOLAR_AGE,
        surface_gravity: JUPITER_SURFACE_GRAVITY,
        primordial_helium_per_hydrogen: Element::He.protosolar_abundance_per_hydrogen(),
    }
}

/// Formation temperatures inside each of [`DISC_CARRIERS`]' zones, warmest first.
const ZONES: [f64; 6] = [1_800.0, 900.0, 300.0, 100.0, 30.0, 15.0];

/// The grid the sum and the golden range over: masses from 1.5 M⊕ to 13 `M_J`, each with an
/// envelope share its class could hold.
const MASSES: [(f64, f64); 9] = [
    (1.5, 0.002),
    (5.0, 0.02),
    (10.0, 0.05),
    (17.0, 0.12),
    (50.0, 0.55),
    (95.0, 0.71),
    (318.0, 0.82),
    (1_000.0, 0.9),
    (4_000.0, 0.95),
];

/// P14.T50.b: Jupiter's C/H within a factor of 2 of the Galileo probe's at its E(M): 1.19 ×
/// 10⁻³ (Wong et al. 2004, as Guillot et al. 2022's Table 2 lists it), 4.3 times this registry's
/// protosolar value, about the Sun's own ratios and about a generated host of solar metallicity at
/// the median ranks, 0.067 dex poorer in carbon.
#[test]
fn jupiters_carbon_is_within_a_factor_of_two_of_the_galileo_probes() {
    let galileo = 1.19e-3;
    for host in [
        HostAbundances::protosolar(),
        HostAbundances::of(Dex::ZERO, &AbundanceDraws::MEDIAN),
    ] {
        let (_, inputs) = table_giant("Jupiter", host);
        let jupiter = EnvelopeAbundances::derive(&inputs).unwrap();
        let ratio = jupiter.per_hydrogen(Element::C) / galileo;
        assert!((0.5..=2.0).contains(&ratio), "{ratio}");
        assert_same_bits(jupiter.enrichment(), mass_enrichment(inputs.mass));
    }
}

/// P14.T50.b: a 10 M⊕ sub-Neptune's metallicity is of order 100 times solar: its E, and its
/// metals by number over the protosolar metals, both between 30 and 300.
#[test]
fn a_ten_earth_mass_sub_neptune_holds_of_order_a_hundred_times_solar() {
    let inputs = synthetic(10.0, 0.05, 466.0, (0.0, 0.0), 0.0);
    let envelope = EnvelopeAbundances::derive(&inputs).unwrap();
    assert!(
        (30.0..=300.0).contains(&envelope.enrichment()),
        "{}",
        envelope.enrichment()
    );
    let by_number: f64 = metals().map(|e| envelope.per_hydrogen(e)).sum::<f64>()
        / metals()
            .map(Element::protosolar_abundance_per_hydrogen)
            .sum::<f64>();
    assert!((30.0..=300.0).contains(&by_number), "{by_number}");
}

/// P14.T50.b: the metals sum to E times the host's by mass, to 10⁻⁹, with the neon the rain took
/// put back, over every mass, zone, host and effective temperature of the grid.
#[test]
fn the_abundances_sum_consistently_with_e_to_1e_9() {
    let weight = |e: Element| e.standard_atomic_weight_g_per_mol();
    for &(earths, envelope) in &MASSES {
        for formation in ZONES {
            for host in [(-0.6, 0.3), (0.0, 0.0), (0.3, -0.04)] {
                for t_eff in [80.0, 132.0, 400.0] {
                    let inputs = synthetic(earths, envelope, formation, host, t_eff);
                    let got = EnvelopeAbundances::derive(&inputs).unwrap();
                    let kept_neon = neon_kept(got.helium_depletion());
                    let host_abundances =
                        host_per_hydrogen(inputs.fe_h, inputs.alpha_fe, &inputs.host);
                    let (mut envelope_metals, mut host_metals) = (0.0, 0.0);
                    for e in metals() {
                        let kept = if e == Element::Ne { kept_neon } else { 1.0 };
                        envelope_metals += weight(e) * got.per_hydrogen(e) / kept;
                        host_metals += weight(e) * host_abundances[index(e)];
                    }
                    let at = format!("{earths} M⊕ at {formation} K, host {host:?}, {t_eff} K");
                    let relative = envelope_metals / (got.enrichment() * host_metals) - 1.0;
                    assert!(relative.abs() < 1e-9, "{at}: {relative}");
                    let (x, y, z) = got.mass_fractions();
                    assert!((x + y + z - 1.0).abs() < 1e-12, "{at}");
                }
            }
        }
    }
}

/// P14.T50.b: C/O at median ranks is the host's times the ice-line factor, in every zone and at
/// every metallicity, and N/O the protosolar ratio.
#[test]
fn c_o_at_median_ranks_is_the_hosts_times_the_ice_line_factor() {
    for formation in ZONES {
        for fe_h in [-0.8, -0.2, 0.0, 0.4] {
            let inputs = synthetic(318.0, 0.82, formation, (fe_h, 0.0), 300.0);
            let got = EnvelopeAbundances::derive(&inputs).unwrap();
            let expected = inputs.host.carbon_to_oxygen() * ice_line_factor(Kelvin::new(formation));
            let at = format!("{formation} K, [Fe/H] {fe_h}");
            assert!(
                (got.carbon_to_oxygen() / expected - 1.0).abs() < 1e-12,
                "{at}"
            );
            assert_same_bits(
                got.ice_line_factor(),
                ice_line_factor(Kelvin::new(formation)),
            );
            let n_over_o = got.per_hydrogen(Element::N) / got.per_hydrogen(Element::O);
            assert!(
                (n_over_o / solar_nitrogen_to_oxygen() - 1.0).abs() < 1e-12,
                "{at}"
            );
        }
    }
    assert!((solar_nitrogen_to_oxygen() - 0.1349).abs() < 1e-4);
}

/// Öberg et al.'s (2011) Table 1: the disc's C/O is 0.545, and its gas's 0.6 inside the snow line,
/// 0.857 to the CO₂ line and 1.0 to the CO line (their §2: "the maximum C/O∼1 reached between the
/// CO₂ and CO sublimation lines"); the factor steps at each line, a carrier being ice or dust at
/// its line itself.
#[test]
fn the_ice_line_factor_steps_at_each_line() {
    let (oxygen, carbon) = DISC_CARRIERS.iter().fold((0.0, 0.0), |(o, c), k| {
        (o + k.oxygen_per_hydrogen, c + k.carbon_per_hydrogen)
    });
    assert!((carbon / oxygen - 0.545).abs() < 5e-4);
    let disc = carbon / oxygen;
    for (t, gas) in [(300.0, 0.6), (100.0, 1.8 / 2.1), (30.0, 1.0)] {
        let factor = ice_line_factor(Kelvin::new(t));
        assert!(
            (factor * disc - gas).abs() < 1e-12,
            "{t} K: {}",
            factor * disc
        );
    }
    for (t, expected) in [
        (2_000.0, 1.0),
        (1_500.1, 1.0),
        (1_500.0, 22.0 / 15.0),
        (500.1, 22.0 / 15.0),
        (500.0, 1.1),
        (170.1, 1.1),
        (170.0, 11.0 / 7.0),
        (47.1, 11.0 / 7.0),
        (47.0, 11.0 / 6.0),
        (20.1, 11.0 / 6.0),
        (20.0, 1.0),
        (5.0, 1.0),
    ] {
        let factor = ice_line_factor(Kelvin::new(t));
        assert!((factor - expected).abs() < 1e-12, "{t} K: {factor}");
    }
}

/// P14.T50.b (decision-p14-t50b-envelope §3): helium rains only in a gas giant of at least
/// 0.15 `M_J` that has cooled and is old enough: linearly in the rain temperature (`T_eff` mapped
/// to Jupiter's gravity on its adiabat) from none at the 133.61 K onset through Jupiter's He/H₂ of
/// 0.157 at its 125.57 K to Saturn's 0.407 of protosolar at its 108.22 K and below, and linearly
/// in age from none at 0.7 Gyr to all of it at 1.5 Gyr, with neon falling by a fixed fraction of
/// what is left, to 0.043 of the other metals at Jupiter's depletion and on; a hot giant, a young
/// one, a sub-Neptune and an ice giant keep their helium (Fortney and Hubbard 2004; Uranus's,
/// Conrath et al. 1987).
#[test]
fn helium_rains_only_in_gas_giants_that_have_cooled() {
    let (jupiter_mass, now) = (EarthMasses::new(317.83), SOLAR_AGE);
    let depletion = |class, t: f64| helium_depletion(class, jupiter_mass, Kelvin::new(t), now);
    for t in [0.0, 50.0, 96.67, 115.0, 125.57, 300.0] {
        for class in [
            PlanetClass::Rocky,
            PlanetClass::Icy,
            PlanetClass::SubNeptune,
            PlanetClass::IceGiant,
        ] {
            assert_same_bits(depletion(class, t), 1.0);
        }
        // Below 0.15 Jupiter masses no giant has metallic hydrogen.
        let light = EarthMasses::new(45.0);
        let light = helium_depletion(PlanetClass::GasGiant, light, Kelvin::new(t), now);
        assert_same_bits(light, 1.0);
    }
    let onset = helium_rain_onset().value();
    assert!((onset - 133.61).abs() < 0.01, "{onset}");
    // Saturn's onset within the "∼100 - 120 K" Fortney and Hubbard quote from Stevenson and
    // Salpeter, and at Jupiter's gravity the onset itself.
    let saturn_onset = helium_rain_onset_at(SATURN_SURFACE_GRAVITY).value();
    assert!((100.0..=120.0).contains(&saturn_onset), "{saturn_onset}");
    assert!((saturn_onset - 119.35).abs() < 0.01, "{saturn_onset}");
    assert_same_bits(helium_rain_onset_at(JUPITER_SURFACE_GRAVITY).value(), onset);
    // Saturn's one-bar temperature over Jupiter's on Hubbard's fit, against Mankovich and
    // Fortney's adiabats of 140 and 166 K, within 2%.
    let (gravity, temperature) = HUBBARD_INDICES;
    let t1_ratio = math::powf(
        SATURN_SURFACE_GRAVITY.value() / JUPITER_SURFACE_GRAVITY.value(),
        -gravity,
    ) * math::powf(
        SATURN_EFFECTIVE_TEMPERATURE.value() / JUPITER_EFFECTIVE_TEMPERATURE.value(),
        temperature,
    );
    assert!(
        (t1_ratio / (140.0 / 166.0) - 1.0).abs() < 0.02,
        "{t1_ratio}"
    );
    // The gravities are GM over the mean radius squared.
    let (r_j, r_s) = (69_911e3_f64, 58_232e3_f64);
    let g_j = crate::units::consts::GM_JUPITER / (r_j * r_j);
    let g_s = crate::units::consts::GRAVITATIONAL_CONSTANT * 568.32e24 / (r_s * r_s);
    assert!(
        (g_j - JUPITER_SURFACE_GRAVITY.value()).abs() < 0.005,
        "{g_j}"
    );
    assert!(
        (g_s - SATURN_SURFACE_GRAVITY.value()).abs() < 0.005,
        "{g_s}"
    );
    assert!((depletion(PlanetClass::GasGiant, onset) - 1.0).abs() < 1e-12);
    assert_same_bits(depletion(PlanetClass::GasGiant, 200.0), 1.0);
    assert_same_bits(depletion(PlanetClass::GasGiant, 1_300.0), 1.0);
    let he_h2 = |d: f64| 2.0 * Element::He.protosolar_abundance_per_hydrogen() * d;
    let at_jupiter = depletion(PlanetClass::GasGiant, JUPITER_EFFECTIVE_TEMPERATURE.value());
    assert!(
        (he_h2(at_jupiter) - 0.157).abs() < 5e-4,
        "{}",
        he_h2(at_jupiter)
    );
    assert!((he_h2(at_jupiter) - jupiter_helium_to_molecular_hydrogen()).abs() < 1e-12);
    let saturn = SATURN_HELIUM_PER_HYDROGEN / Element::He.protosolar_abundance_per_hydrogen();
    assert!((saturn - 0.407).abs() < 1e-3);
    let x_saturn = rain_temperature(SATURN_EFFECTIVE_TEMPERATURE, SATURN_SURFACE_GRAVITY).value();
    assert!((x_saturn - 108.22).abs() < 0.01, "{x_saturn}");
    for t in [x_saturn, 80.0, 0.0] {
        assert!(
            (depletion(PlanetClass::GasGiant, t) - saturn).abs() < 1e-12,
            "{t} K"
        );
    }
    // Continuous and monotonic in the rain temperature, with no step steeper than the line.
    let slope = (at_jupiter - saturn) / (125.57 - x_saturn);
    let mut last = depletion(PlanetClass::GasGiant, 160.0);
    for k in 1..=800 {
        let t = 160.0 - 0.1 * f64::from(k);
        let now = depletion(PlanetClass::GasGiant, t);
        assert!(now <= last && last - now <= 0.1 * slope + 1e-12, "{t} K");
        last = now;
    }
}

/// P14.T50.b (decision-p14-t50b-envelope §3 and §6): the rain is continuous in gravity, waits on
/// age (none before 0.7 Gyr, all from 1.5 Gyr) and takes neon by a fixed fraction of what is left.
#[test]
fn the_rain_is_continuous_in_gravity_and_age_and_takes_neon() {
    let (jupiter_mass, now) = (EarthMasses::new(317.83), SOLAR_AGE);
    let saturn = SATURN_HELIUM_PER_HYDROGEN / Element::He.protosolar_abundance_per_hydrogen();
    let at_jupiter = helium_depletion(
        PlanetClass::GasGiant,
        jupiter_mass,
        JUPITER_EFFECTIVE_TEMPERATURE,
        now,
    );
    // Continuous and monotonic in gravity at a fixed T_eff of 112 K: a giant of higher gravity
    // lies on a cooler adiabat and rains more.
    let at_gravity = |g: f64| {
        let x = rain_temperature(Kelvin::new(112.0), MetresPerSecondSquared::new(g));
        helium_depletion(PlanetClass::GasGiant, jupiter_mass, x, now)
    };
    let mut last = at_gravity(5.0);
    for k in 1..=1_100 {
        let g = 5.0 + 0.05 * f64::from(k);
        let now = at_gravity(g);
        assert!(now <= last && last - now < 5e-3, "{g} m s⁻²");
        last = now;
    }
    assert_same_bits(at_gravity(5.0), 1.0);
    assert!((at_gravity(60.0) - saturn).abs() < 1e-12);
    // A cold giant of 0.2 M_J with no luminosity of its own waits on age: none at 0.1 and 0.7 Gyr,
    // half at 1.1 Gyr, all from 1.5 Gyr, continuously.
    let cold = |gyr: f64| {
        let age = Years::new(gyr * 1e9);
        helium_depletion(
            PlanetClass::GasGiant,
            EarthMasses::new(63.6),
            Kelvin::new(80.0),
            age,
        )
    };
    assert_same_bits(cold(0.1), 1.0);
    assert_same_bits(cold(0.7), 1.0);
    assert!((cold(1.1) - (1.0 - 0.5 * (1.0 - saturn))).abs() < 1e-12);
    assert!((cold(1.5) - saturn).abs() < 1e-12);
    assert!((cold(4.6) - saturn).abs() < 1e-12);
    let mut last = cold(0.0);
    for k in 1..=200 {
        let now = cold(0.01 * f64::from(k));
        assert!(now <= last && last - now <= 0.01 / 0.8 * (1.0 - saturn) + 1e-12);
        last = now;
    }
    // Jupiter's effective temperature is the 14.098 W m⁻² it emits (Li et al. 2018).
    let t_jupiter = (14.098 / STEFAN_BOLTZMANN).sqrt().sqrt();
    assert!((t_jupiter - JUPITER_EFFECTIVE_TEMPERATURE.value()).abs() < 5e-3);
    // Neon: all kept without rain, falling by a fixed fraction of what is left, to 0.043 at
    // Jupiter's depletion and on, to 0.043^3.16 at Saturn's.
    assert_same_bits(neon_kept(1.0), 1.0);
    let halfway = 1.0 - 0.5 * (1.0 - at_jupiter);
    assert!((neon_kept(halfway) - RAINED_NEON.sqrt()).abs() < 1e-12);
    assert!((neon_kept(at_jupiter) - RAINED_NEON).abs() < 1e-12);
    let at_saturn = math::powf(RAINED_NEON, (1.0 - saturn) / (1.0 - at_jupiter));
    assert!((neon_kept(saturn) - at_saturn).abs() < 1e-15);
    assert!((at_saturn / 4.9e-5 - 1.0).abs() < 0.05, "{at_saturn}");
}

/// P14.T50.b (decision-p14-t50b-envelope §6): an envelope carries the rain on its primordial
/// helium, which need not be protosolar, and its neon with it.
#[test]
fn envelopes_carry_the_rain_on_their_primordial_helium() {
    let (jupiter_mass, now) = (EarthMasses::new(317.83), SOLAR_AGE);
    let saturn = SATURN_HELIUM_PER_HYDROGEN / Element::He.protosolar_abundance_per_hydrogen();
    let at_jupiter = helium_depletion(
        PlanetClass::GasGiant,
        jupiter_mass,
        JUPITER_EFFECTIVE_TEMPERATURE,
        now,
    );
    let at_saturn = math::powf(RAINED_NEON, (1.0 - saturn) / (1.0 - at_jupiter));
    // In an envelope: the cold giant takes Saturn's helium and loses its neon; the hot giant, the
    // one at Jupiter's gravity just above the onset, the young one, the sub-Neptune and the ice
    // giant keep the protosolar helium and their neon.
    let protosolar = Element::He.protosolar_abundance_per_hydrogen();
    for (earths, envelope, t_eff, gyr, rained) in [
        (318.0, 0.82, 80.0, 4.57, true),
        (318.0, 0.82, 300.0, 4.57, false),
        (318.0, 0.82, 134.0, 4.57, false),
        (318.0, 0.82, 140.0, 4.57, false),
        (63.6, 0.6, 80.0, 0.1, false),
        (5.0, 0.02, 60.0, 4.57, false),
        (17.0, 0.12, 60.0, 4.57, false),
    ] {
        let mut inputs = synthetic(earths, envelope, 100.0, (0.0, 0.0), t_eff);
        inputs.age = Years::new(gyr * 1e9);
        let got = EnvelopeAbundances::derive(&inputs).unwrap();
        let host = host_per_hydrogen(inputs.fe_h, inputs.alpha_fe, &inputs.host);
        let neon = got.per_hydrogen(Element::Ne) / (got.enrichment() * host[index(Element::Ne)]);
        let at = format!("{earths} M⊕ at {t_eff} K and {gyr} Gyr");
        if rained {
            assert!(
                (got.per_hydrogen(Element::He) - SATURN_HELIUM_PER_HYDROGEN).abs() < 1e-15,
                "{at}"
            );
            assert!((neon - at_saturn).abs() < 1e-12, "{at}");
        } else {
            assert_same_bits(got.per_hydrogen(Element::He), protosolar);
            assert!((neon - 1.0).abs() < 1e-12, "{at}");
        }
    }
    // A primordial helium other than the protosolar is carried through: kept where nothing rains,
    // and depleted by the same share where it does.
    for (t_eff, share) in [(300.0, 1.0), (80.0, saturn)] {
        let mut inputs = synthetic(318.0, 0.82, 100.0, (0.0, 0.0), t_eff);
        inputs.primordial_helium_per_hydrogen = 0.085;
        let got = EnvelopeAbundances::derive(&inputs).unwrap();
        assert!(
            (got.per_hydrogen(Element::He) - 0.085 * share).abs() < 1e-15,
            "{t_eff} K"
        );
        assert!((got.helium_depletion() - share).abs() < 1e-15, "{t_eff} K");
    }
}

/// P14.T50.b, the fit checked: about the Sun's own ratios, the table's four giants' carbon lies
/// within 0.15 dex of their measured C/H (Guillot et al. 2022, Table 2); the table's Jupiter rains,
/// at its own effective temperature within 123–128 K and its He/H₂ within 0.145–0.170
/// (decision-p14-t54a-ganymede §2), its Ne/H stated; at Jupiter's measured state its helium is the
/// Galileo probe's and its oxygen, sulphur, noble gases and neon fall inside what the probe and
/// Juno measured; Saturn, with no luminosity of its own, takes Saturn's helium; Uranus and Neptune
/// keep protosolar helium, as Voyager's and ISO's measurements allow.
#[test]
fn the_table_giants_match_their_measured_atmospheres() {
    let protosolar = |e: Element| e.protosolar_abundance_per_hydrogen();
    for (name, carbon) in [
        ("Jupiter", 1.19e-3),
        ("Saturn", 2.50e-3),
        ("Uranus", 1.95e-2),
        ("Neptune", 2.45e-2),
    ] {
        let (_, inputs) = table_giant(name, HostAbundances::protosolar());
        let got = EnvelopeAbundances::derive(&inputs).unwrap();
        let residual = math::log10(got.per_hydrogen(Element::C) / carbon);
        assert!(residual.abs() < 0.15, "{name}: {residual} dex");
    }
    let (body, mut inputs) = table_giant("Jupiter", HostAbundances::protosolar());
    assert_eq!(body.class(), PlanetClass::GasGiant);
    let table = EnvelopeAbundances::derive(&inputs).unwrap();
    let he_h2 = table.helium_to_molecular_hydrogen();
    // The table's Jupiter rains, below the protosolar 0.194. Under T13.c's Bond albedo, 0.34, it
    // was 2.3 K warmer than measured (127.84 K) and rained less than Galileo found, He/H₂ 0.171.
    // Under P14.T51.b's area mix it takes class I's hazy 0.48 about a Sun
    // (decision-p14-t54a-ganymede §2; Li et al. 2018 measured 0.503 ± 0.012), which puts it within
    // 123–128 K of the measured 125.57 ± 0.07 K (Li et al. 2012) and its He/H₂ within 0.145–0.170
    // of the probe's 0.157 (von Zahn et al. 1998). The rest of its effective temperature, about
    // −1.6 K, is the cooling fit's internal flux, 6.84 W m⁻² against 7.485 ± 0.160 W m⁻² (Li et al.
    // 2018), recorded, not tuned. Its Ne/H is stated against the probe's 1.24 × 10⁻⁵ (Mahaffy et
    // al. 2000, as Atreya et al. 2016, Table 2.1, list it).
    let protosolar_he_h2 = 2.0 * protosolar(Element::He);
    let t_eff = inputs.effective_temperature.value();
    let neon = table.per_hydrogen(Element::Ne);
    eprintln!(
        "the table's Jupiter: T_eff {t_eff:.3} K at a Bond albedo of {:.5}, He/H2 {he_h2:.4}, Ne/H \
         {neon:.4e} ({:.3} of the probe's 1.24e-5)",
        body.albedo().value(),
        neon / 1.24e-5
    );
    assert!(he_h2 < protosolar_he_h2, "{he_h2}");
    assert!((123.0..=128.0).contains(&t_eff), "{t_eff} K");
    assert!((0.145..=0.170).contains(&he_h2), "{he_h2}");
    // The rest at Jupiter's measured state.
    inputs.effective_temperature = JUPITER_EFFECTIVE_TEMPERATURE;
    inputs.surface_gravity = JUPITER_SURFACE_GRAVITY;
    let jupiter = EnvelopeAbundances::derive(&inputs).unwrap();
    let measured = |e: Element, per_h: f64, low: f64, high: f64| {
        let ours = jupiter.per_hydrogen(e) / protosolar(e);
        let theirs = per_h / protosolar(e);
        assert!(
            (theirs * low..=theirs * high).contains(&ours),
            "{e:?}: {ours} against {theirs}"
        );
    };
    // Juno's O/H, 1.45 (+1.28, −0.93) × 10⁻³ (Li et al. 2020, in Guillot et al.'s Table 2), at
    // one sigma.
    measured(Element::O, 1.45e-3, 0.52 / 1.45, 2.73 / 1.45);
    // Galileo's S/H, 4.45 ± 1.05 × 10⁻⁵ (Wong et al. 2004), at one sigma.
    measured(Element::S, 4.45e-5, 3.40 / 4.45, 5.50 / 4.45);
    // Galileo's noble gases and neon (Mahaffy et al. 2000, in Atreya et al. 2016's Table 2.1),
    // within a factor of 1.5.
    for (e, per_h) in [
        (Element::Ar, 9.10e-6),
        (Element::Kr, 4.65e-9),
        (Element::Xe, 4.45e-10),
        (Element::Ne, 1.24e-5),
    ] {
        measured(e, per_h, 1.0 / 1.5, 1.5);
    }
    // The ice giants take the ice giants' class I, 0.32 about a Sun (decision-p14-t54a-ganymede
    // §3): Uranus lies within 56–62 K of its measured 59.1 ± 0.3 K. Neptune, about 46 K against its
    // measured 59.3 ± 0.8 K (Pearl and Conrath 1991), emits 2.61 times what it absorbs, and the
    // record gives a giant under 0.3 M_J no internal heat: the giants' cooling, stated, not fixed.
    for name in ["Uranus", "Neptune"] {
        let (body, inputs) = table_giant(name, HostAbundances::protosolar());
        assert_eq!(body.class(), PlanetClass::IceGiant, "{name}");
        let t = body.equilibrium_temperature().value();
        eprintln!(
            "the table's {name}: T_eq {t:.3} K at a Bond albedo of {:.5}, its own heat {:?}",
            body.albedo().value(),
            body.effective_temperature()
        );
        if name == "Uranus" {
            assert!((56.0..=62.0).contains(&t), "{name}: {t} K");
        }
        let got = EnvelopeAbundances::derive(&inputs).unwrap();
        assert_same_bits(got.per_hydrogen(Element::He), protosolar(Element::He));
        // Uranus's helium mole fraction, 0.152 ± 0.033 (Conrath et al. 1987), as He/H₂.
        let he_h2 = got.helium_to_molecular_hydrogen();
        assert!(
            (0.119 / 0.881..=0.185 / 0.815).contains(&he_h2),
            "{name}: {he_h2}"
        );
    }
    let (saturn, inputs) = table_giant("Saturn", HostAbundances::protosolar());
    assert_eq!(saturn.class(), PlanetClass::GasGiant);
    assert!(saturn.effective_temperature().is_none());
    let got = EnvelopeAbundances::derive(&inputs).unwrap();
    assert!((got.per_hydrogen(Element::He) - SATURN_HELIUM_PER_HYDROGEN).abs() < 1e-15);
}

/// The bound and the floor: a 13 `M_J` giant holds its host's metals (E = 1, beyond the trend's
/// 2.69 `M_J`), and a light, puffy body whose bulk is mostly envelope is held to the bulk's metal
/// fraction (f = 1, Thorngren and Fortney 2019), below its trend.
#[test]
fn the_envelope_is_neither_poorer_than_its_host_nor_richer_than_its_bulk() {
    let heavy = synthetic(4_000.0, 0.95, 100.0, (0.0, 0.0), 300.0);
    let got = EnvelopeAbundances::derive(&heavy).unwrap();
    assert!(mass_enrichment(heavy.mass) < 1.0);
    assert_same_bits(got.enrichment(), 1.0);
    let puffy = synthetic(3.0, 0.6, 100.0, (0.0, 0.0), 300.0);
    let got = EnvelopeAbundances::derive(&puffy).unwrap();
    assert!(got.enrichment() < mass_enrichment(puffy.mass));
    let (_, _, z) = got.mass_fractions();
    assert!(
        (z - (1.0 - puffy.fractions.envelope())).abs() < 1e-12,
        "{z}"
    );
    // The trend reaches 1 at 2.69 Jupiter masses.
    let at = |jupiters: f64| mass_enrichment(EarthMasses::from(JupiterMasses::new(jupiters)));
    assert!(at(2.68) > 1.0 && at(2.70) < 1.0);
}

/// The host: the α elements rise with \[α/Fe\] and the rest do not, magnesium follows silicon at
/// the host's Mg/Si, and every metal rises with \[Fe/H\] (carbon, nitrogen and oxygen, shared out
/// at a fixed ratio, rise with them).
#[test]
fn the_alpha_elements_follow_alpha_fe_and_the_rest_follow_iron() {
    let at = |fe_h: f64, alpha: f64| {
        let mut inputs = synthetic(318.0, 0.82, 100.0, (fe_h, alpha), 300.0);
        inputs.host = HostAbundances::protosolar();
        EnvelopeAbundances::derive(&inputs).unwrap()
    };
    let (base, alpha, iron) = (at(0.0, 0.0), at(0.0, 0.3), at(0.3, 0.0));
    let shift =
        |a: &EnvelopeAbundances, e: Element| math::log10(a.per_hydrogen(e) / base.per_hydrogen(e));
    for e in metals() {
        let expected_alpha = if ALPHA_ELEMENTS.contains(&e) || matches!(e, Element::C | Element::N)
        {
            0.3
        } else {
            0.0
        };
        assert!(
            (shift(&alpha, e) - expected_alpha).abs() < 1e-12,
            "{e:?} with [α/Fe]"
        );
        assert!((shift(&iron, e) - 0.3).abs() < 1e-12, "{e:?} with [Fe/H]");
    }
    let mg_si = base.per_hydrogen(Element::Mg) / base.per_hydrogen(Element::Si);
    assert!((mg_si / HostAbundances::protosolar().magnesium_to_silicon() - 1.0).abs() < 1e-12);
}

/// Each input that is not an envelope's is refused.
#[test]
fn inputs_that_are_not_an_envelope_s_are_refused() {
    let good = synthetic(318.0, 0.82, 100.0, (0.0, 0.0), 120.0);
    let refused =
        |inputs: EnvelopeInputs| format!("{:?}", EnvelopeAbundances::derive(&inputs).unwrap_err());
    let mut bad = good;
    bad.mass = EarthMasses::new(0.0);
    assert_eq!(refused(bad), "Mass(EarthMasses(0.0))");
    let mut bad = good;
    bad.fractions = MassFractions::of(
        CoreComposition::from_fractions(EARTH_CORE_MASS_FRACTION, 0.0),
        0.000_5,
    );
    assert_eq!(refused(bad), "NotAnEnvelope(0.0005)");
    let mut bad = good;
    bad.snow_line = Metres::new(f64::NAN);
    assert_eq!(refused(bad), "SnowLine(Metres(NaN))");
    let mut bad = good;
    bad.formation_distance = Metres::new(-1.0);
    assert_eq!(refused(bad), "FormationDistance(Metres(-1.0))");
    let mut bad = good;
    bad.fe_h = Dex::new(f64::NAN);
    assert_eq!(refused(bad), "FeH(Dex(NaN))");
    let mut bad = good;
    bad.alpha_fe = Dex::new(f64::INFINITY);
    assert_eq!(refused(bad), "AlphaFe(Dex(inf))");
    let mut bad = good;
    bad.fe_h = Dex::new(-10.5);
    assert_eq!(refused(bad), "FeH(Dex(-10.5))");
    let mut bad = good;
    bad.age = Years::new(-1.0);
    assert_eq!(refused(bad), "Age(Years(-1.0))");
    let mut bad = good;
    bad.surface_gravity = MetresPerSecondSquared::new(0.0);
    assert_eq!(refused(bad), "SurfaceGravity(MetresPerSecondSquared(0.0))");
    let mut bad = good;
    bad.primordial_helium_per_hydrogen = f64::NAN;
    assert_eq!(refused(bad), "PrimordialHelium(NaN)");
    let mut bad = good;
    bad.effective_temperature = Kelvin::new(-1.0);
    assert_eq!(refused(bad), "EffectiveTemperature(Kelvin(-1.0))");
    assert_eq!(
        EnvelopeAbundances::derive(&bad).unwrap_err().to_string(),
        "effective temperature -1 K is not finite and non-negative"
    );
}

/// The same inputs give the same bits twice.
#[test]
fn the_same_inputs_give_the_same_abundances_twice() {
    let inputs = synthetic(50.0, 0.55, 30.0, (0.2, 0.05), 110.0);
    let (a, b) = (
        EnvelopeAbundances::derive(&inputs).unwrap(),
        EnvelopeAbundances::derive(&inputs).unwrap(),
    );
    for ((ea, na), (eb, nb)) in a.elements().zip(b.elements()) {
        assert_eq!(ea, eb);
        assert_same_bits(na, nb);
    }
}

/// The abundances pinned by their bits at header 22: the table's four giants about the Sun's own
/// ratios and about a generated host at solar metallicity, and the grid's bodies in every zone,
/// each about one of three hosts and effective temperatures in turn. Nothing on the record reads
/// them until P14.T54.b, so they move no other golden.
#[test]
fn the_envelope_abundances_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut pin = |label: &str, got: &EnvelopeAbundances| {
        w.f64(&format!("{label} enrichment"), got.enrichment());
        w.f64(&format!("{label} ice_line_factor"), got.ice_line_factor());
        w.f64(&format!("{label} helium_depletion"), got.helium_depletion());
        for (e, n) in got.elements() {
            w.f64(&format!("{label} {}/H", e.symbol()), n);
        }
        let (x, y, z) = got.mass_fractions();
        w.f64(&format!("{label} X"), x);
        w.f64(&format!("{label} Y"), y);
        w.f64(&format!("{label} Z"), z);
    };
    for name in ["Jupiter", "Saturn", "Uranus", "Neptune"] {
        for (host_label, host) in [
            ("protosolar", HostAbundances::protosolar()),
            (
                "median",
                HostAbundances::of(Dex::ZERO, &AbundanceDraws::MEDIAN),
            ),
        ] {
            let (_, inputs) = table_giant(name, host);
            let got = EnvelopeAbundances::derive(&inputs).unwrap();
            pin(&format!("{name} {host_label}"), &got);
        }
    }
    let hosts = [
        ((-0.6, 0.3), 80.0),
        ((0.0, 0.0), 132.0),
        ((0.3, -0.04), 400.0),
    ];
    for (i, &(earths, envelope)) in MASSES.iter().enumerate() {
        for (j, formation) in ZONES.into_iter().enumerate() {
            // One host and effective temperature per body, in turn, to keep the golden small.
            let (host, t_eff) = hosts[(i + j) % hosts.len()];
            let inputs = synthetic(earths, envelope, formation, host, t_eff);
            let got = EnvelopeAbundances::derive(&inputs).unwrap();
            pin(
                &format!(
                    "{earths} M⊕ {envelope} at {formation} K [Fe/H] {} [α/Fe] {} T_eff {t_eff}",
                    host.0, host.1
                ),
                &got,
            );
        }
    }
    golden!("planetary/envelope_abundances", w.as_str());
}
