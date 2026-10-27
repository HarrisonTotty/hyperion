//! P14.T24.d's internal temperature and profile on the Solar System table's giants: the cooling
//! law's anchors, its blend with the record's heat, and κ₁'s fit on Jupiter. P14.T52.d's visible
//! atmosphere: the table's Jupiter against the probe, Sudarsky et al. 2000's classes, Fortney et
//! al. 2008's TiO split, a KELT-9b-like dayside, the sub-Neptunes, the `f_sed` kinds, continuity,
//! determinism and the golden.

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::HostAbundances;
use crate::planetary::chem::envelope_abundances::tests::{synthetic, table_giant};
use crate::planetary::chem::envelope_abundances::{EnvelopeAbundances, EnvelopeInputs};
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::derive::solar::{PLANETS, SOLAR_AGE, found, solar_disc, solar_system};
use crate::planetary::hooks::aerosols::{
    AbsorberLayer, LEAST_SEDIMENTATION_EFFICIENCY, ParticleSize,
    SEDIMENTATION_SPEED_AT_UNIT_SUPPLY, SEDIMENTATION_SUPPLY_EXPONENT,
};
use crate::planetary::hooks::structure::envelope::{
    EnvelopeProfile, EnvelopeProfileInputs, GreyProfile, MEAN_REDISTRIBUTION, OPACITY_RATIO,
    OPTICAL_DEPTH_EXPONENT, THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG, irradiation_temperature,
};
use crate::substance::aloft::laws_aloft;
use crate::substance::{Element, SubstanceId, by_key};
use crate::units::consts::METRES_PER_AU;
use crate::units::{Dex, JupiterMasses, MetresPerSecondSquared, Pascals};

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// The table's giant `name`, derived about the present Sun at its own distance.
fn table(name: &str) -> DerivedBody {
    *found(&solar_system(), name)
}

/// The internal temperature this module states for the table's giant `body`, at the table's age.
fn internal(body: &DerivedBody) -> Kelvin {
    envelope_internal_temperature(
        body.internal_luminosity(),
        Metres::from(body.radius()),
        body.mass(),
        SOLAR_AGE,
    )
    .expect("a table giant's internal temperature")
}

/// The table's giant `name`'s He ÷ H₂ by number, P14.T50.b's, about a protosolar host, its
/// effective temperature the envelope's, (`T_eq`⁴ + `T_int`⁴)^¼, as P14.T54.b passes it.
fn helium_to_hydrogen(name: &str, body: &DerivedBody) -> f64 {
    let &(_, _, _, a, _) = PLANETS
        .iter()
        .find(|(n, ..)| *n == name)
        .expect("the giant is in the table");
    let t_eq = body.equilibrium_temperature().value();
    let t_int = internal(body).value();
    let t_eff = math::powf(math::powi(t_eq, 4) + math::powi(t_int, 4), 0.25);
    let abundances = EnvelopeAbundances::derive(&EnvelopeInputs {
        mass: body.mass(),
        fractions: body.fractions(),
        formation_distance: Metres::new(a * METRES_PER_AU),
        snow_line: solar_disc().snow_line(),
        fe_h: Dex::ZERO,
        alpha_fe: Dex::ZERO,
        host: HostAbundances::protosolar(),
        effective_temperature: Kelvin::new(t_eff),
        age: SOLAR_AGE,
        surface_gravity: body.surface_gravity(),
        primordial_helium_per_hydrogen: Element::He.protosolar_abundance_per_hydrogen(),
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    abundances.helium_to_molecular_hydrogen()
}

/// A hydrogen–helium mixture of He ÷ H₂ `ratio` by number.
fn hydrogen_helium(ratio: f64) -> Vec<(SubstanceId, f64)> {
    let he = ratio / (1.0 + ratio);
    vec![(row("H2"), 1.0 - he), (row("He"), he)]
}

/// The profile of the table's giant `name` on its hydrogen and helium with Guillot's parameters
/// `grey`.
fn table_profile(name: &str, grey: GreyProfile) -> EnvelopeProfile {
    let body = table(name);
    let gases = hydrogen_helium(helium_to_hydrogen(name, &body));
    EnvelopeProfile::derive(&EnvelopeProfileInputs {
        internal_temperature: internal(&body),
        irradiation_temperature: irradiation_temperature(body.equilibrium_temperature()),
        gravity: body.surface_gravity(),
        gases: &gases,
        grey,
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The temperature at `bar` bar of `profile`, K.
fn at_bar(profile: &EnvelopeProfile, bar: f64) -> f64 {
    profile.temperature_at(Pascals::new(bar * 1e5)).value()
}

/// The cooling law holds its anchors: Saturn's 84.13 K at 95.161 M⊕ and Neptune's 52.57 K at
/// 17.148 M⊕, the table's masses, at 4.57 Gyr; its mass index is the two's.
#[test]
fn the_cooling_law_holds_its_anchors() {
    let saturn = table("Saturn").mass().value();
    let neptune = table("Neptune").mass().value();
    assert!(
        (saturn / COOLING_LAW_MASS.value() - 1.0).abs() < 1e-4,
        "Saturn {saturn} M⊕"
    );
    assert!(
        (neptune / 17.148 - 1.0).abs() < 1e-4,
        "Neptune {neptune} M⊕"
    );
    let law = |m: f64| {
        cooling_law_temperature(EarthMasses::new(m), SOLAR_AGE)
            .unwrap()
            .value()
    };
    assert!((law(COOLING_LAW_MASS.value()) - 84.13).abs() < 1e-12);
    assert!(
        (law(17.148) - 52.57).abs() < 0.01,
        "Neptune {}",
        law(17.148)
    );
    let index = math::ln(84.13 / 52.57) / math::ln(95.161 / 17.148);
    assert!((index - COOLING_LAW_MASS_INDEX).abs() < 1e-4, "b {index}");
    let flux = |t: f64| STEFAN_BOLTZMANN * math::powi(t, 4);
    assert!(
        (flux(84.13) - 2.84).abs() < 0.005,
        "Saturn's flux {}",
        flux(84.13)
    );
    assert!(
        (flux(52.57) - 0.433).abs() < 0.001,
        "Neptune's flux {}",
        flux(52.57)
    );
}

/// The internal temperature is the law's below 0.3 Jupiter masses (the generated Saturn's 84.13 K),
/// the record's from 0.414 (Jupiter's), and continuous at both ends with a record luminosity faded
/// in by `giant_share`, as the record's is.
#[test]
fn the_internal_temperature_blends_the_record_s_heat_by_giant_share() {
    let saturn = table("Saturn");
    assert_same_bits(saturn.internal_luminosity().value(), 0.0);
    let t = internal(&saturn).value();
    assert!((t - 84.13).abs() < 0.01, "the generated Saturn's {t} K");
    let jupiter = table("Jupiter");
    let r = Metres::from(jupiter.radius()).value();
    let record = math::powf(
        jupiter.internal_luminosity().value()
            / (4.0 * core::f64::consts::PI * r * r * STEFAN_BOLTZMANN),
        0.25,
    );
    assert!(
        (internal(&jupiter).value() / record - 1.0).abs() < 1e-14,
        "Jupiter's"
    );
    assert!(
        (100.0..110.0).contains(&record),
        "Jupiter's record {record} K"
    );
    let full = Watts::new(4.0e17);
    let radius = Metres::new(7.0e7);
    let at = |m: f64| {
        let mass = EarthMasses::new(m);
        let luminosity = Watts::new(full.value() * giant_share(mass));
        envelope_internal_temperature(luminosity, radius, mass, SOLAR_AGE)
            .unwrap()
            .value()
    };
    for edge in [0.3, 0.414] {
        let m = EarthMasses::from(JupiterMasses::new(edge)).value();
        let (below, above) = (at(m * (1.0 - 1e-9)), at(m * (1.0 + 1e-9)));
        assert!(
            (below / above - 1.0).abs() < 1e-7,
            "at {edge} M_J: {below} against {above}"
        );
    }
}

/// The internal temperature is finite and positive at every age and every envelope's mass: at age
/// 0 (and below) it is a 1 Myr envelope's, and at 13.8 Gyr cooler than at 4.57; a luminosity of −0
/// gives the same bits as +0.
#[test]
fn the_internal_temperature_is_positive_at_every_age() {
    for m in [0.5, 2.0, 10.0, 95.161, 131.6] {
        let mass = EarthMasses::new(m);
        let law = |t: Years| cooling_law_temperature(mass, t).unwrap().value();
        let (zero, young) = (law(Years::new(0.0)), law(Years::new(1.0e6)));
        let (old, now) = (law(Years::new(13.8e9)), law(SOLAR_AGE));
        assert_same_bits(zero, young);
        assert!(
            old.is_finite() && old > 0.0 && old < now && now < young,
            "{m} M⊕"
        );
    }
    let saturn = COOLING_LAW_MASS;
    assert_same_bits(
        cooling_law_temperature(saturn, Years::new(-1.0))
            .unwrap()
            .value(),
        cooling_law_temperature(saturn, MIN_AGE).unwrap().value(),
    );
    let radius = Metres::new(5.8e7);
    let zero = envelope_internal_temperature(Watts::new(-0.0), radius, saturn, SOLAR_AGE).unwrap();
    assert_same_bits(
        zero.value(),
        envelope_internal_temperature(Watts::new(0.0), radius, saturn, SOLAR_AGE)
            .unwrap()
            .value(),
    );
}

/// The internal temperature's inputs out of their ranges are each refused by their own variant,
/// carrying the value given: a mass that is not finite and positive, an age that is not finite, a
/// luminosity that is not finite and not negative, a radius that is not finite and positive.
#[test]
fn an_input_out_of_range_is_refused_by_its_own_variant() {
    let saturn = COOLING_LAW_MASS;
    for mass in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mass = EarthMasses::new(mass);
        assert_eq!(
            cooling_law_temperature(mass, SOLAR_AGE).map_err(|e| format!("{e:?}")),
            Err(format!("{:?}", DeriveInternalTemperatureError::Mass(mass)))
        );
    }
    for age in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let age = Years::new(age);
        assert_eq!(
            cooling_law_temperature(saturn, age).map_err(|e| format!("{e:?}")),
            Err(format!("{:?}", DeriveInternalTemperatureError::Age(age)))
        );
    }
    let radius = Metres::new(5.8e7);
    let internal_at = |luminosity: f64, r: f64| {
        envelope_internal_temperature(Watts::new(luminosity), Metres::new(r), saturn, SOLAR_AGE)
    };
    // The value refused is the value given (a NaN as a NaN, whose bits are unspecified).
    let same = |carried: f64, given: f64| {
        if given.is_nan() {
            assert!(carried.is_nan(), "{carried} against NaN");
        } else {
            assert_same_bits(carried, given);
        }
    };
    for luminosity in [f64::NAN, -1.0, f64::INFINITY] {
        match internal_at(luminosity, 5.8e7) {
            Err(DeriveInternalTemperatureError::Luminosity(l)) => same(l.value(), luminosity),
            other => panic!("L {luminosity} W: {other:?}"),
        }
    }
    for r in [0.0, -1.0, f64::NAN] {
        match internal_at(1e17, r) {
            Err(DeriveInternalTemperatureError::Radius(m)) => same(m.value(), r),
            other => panic!("R {r} m: {other:?}"),
        }
    }
    assert!(matches!(
        envelope_internal_temperature(Watts::new(1e17), radius, EarthMasses::new(0.0), SOLAR_AGE),
        Err(DeriveInternalTemperatureError::Mass(_))
    ));
}

/// κ₁ is fitted through this module on the table's Jupiter, its hydrogen and helium (P14.T50.b's He
/// ÷ H₂), its record's `T_int` and `T_eq`, to the Galileo probe's 166.1 K at 1 bar (Seiff et al.
/// 1998): the fit lies within decision-p14-t24d-profile's 2.7–3.0 × 10⁻³ m² kg⁻¹ (its own check
/// 2.866 × 10⁻³), and [`THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG`] is it to three figures.
#[test]
fn the_thermal_opacity_is_fitted_on_jupiter() {
    let one_bar = |kappa: f64| {
        let grey = GreyProfile::new(
            kappa,
            OPTICAL_DEPTH_EXPONENT,
            OPACITY_RATIO,
            MEAN_REDISTRIBUTION,
        )
        .unwrap();
        at_bar(&table_profile("Jupiter", grey), 1.0)
    };
    let (mut low, mut high) = (math::ln(1e-3), math::ln(1e-2));
    assert!(one_bar(math::exp(low)) < 166.1 && one_bar(math::exp(high)) > 166.1);
    for _ in 0..40 {
        let mid = f64::midpoint(low, high);
        if one_bar(math::exp(mid)) < 166.1 {
            low = mid;
        } else {
            high = mid;
        }
    }
    let fit = math::exp(f64::midpoint(low, high));
    assert!((2.7e-3..3.0e-3).contains(&fit), "κ₁ fits at {fit}");
    let rounded = (fit * 1e5).round() / 1e5;
    assert!(
        (THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG - rounded).abs() < 1e-12,
        "κ₁ fits at {fit:.5e}, against the adopted {THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG:e}"
    );
}

/// The table's Jupiter on the adopted parameters matches the probe: 166.1 K at 1 bar to 0.5 K, its
/// 22 bar within 2 K of 427.7 K (decision-p14-t24d-profile: 427.0 K), 0.1 bar at 100–120 K (110.2
/// K) and its join at 0.5–1.5 bar (0.902 bar).
#[test]
fn the_table_s_jupiter_follows_the_galileo_probe() {
    let jupiter = table_profile("Jupiter", GreyProfile::ADOPTED);
    let (one, deep, high) = (
        at_bar(&jupiter, 1.0),
        at_bar(&jupiter, 22.0),
        at_bar(&jupiter, 0.1),
    );
    let join = jupiter.join_pressure().value() / 1e5;
    assert!((one - 166.1).abs() < 0.5, "T(1 bar) {one} K");
    assert!((deep - 427.7).abs() < 2.0, "T(22 bar) {deep} K");
    assert!((100.0..120.0).contains(&high), "T(0.1 bar) {high} K");
    assert!((0.5..1.5).contains(&join), "the join at {join} bar");
}

/// The cold giants run warm at 1 bar on the grey profile, recorded (decision-p14-t24d-profile §1.5:
/// Saturn 149.7 K against 134 ± 4 K, Uranus 99.1 K against 76 K, Neptune 94.8 K against 72 ± 2 K):
/// each joins its adiabat below its photosphere and above 2 bar, and its 1-bar temperature lies
/// within its measured value's −5 K to +40%, as pinned in `planetary/envelope_profile`.
#[test]
fn the_cold_giants_join_and_run_warm_at_one_bar() {
    for (name, measured) in [("Saturn", 134.0), ("Uranus", 76.0), ("Neptune", 72.0)] {
        let profile = table_profile(name, GreyProfile::ADOPTED);
        let one = at_bar(&profile, 1.0);
        let join = profile.join_pressure().value() / 1e5;
        assert!(
            one > measured - 5.0 && one < measured * 1.4 && join < 2.0,
            "{name}: T(1 bar) {one} K against {measured} K, its join at {join} bar"
        );
    }
}

/// A weakly heated sub-Neptune under strong light, 5 M⊕ at `T_eq` 600 K on the law's heat, joins
/// below 100 bar (decision-p14-t24d-profile §1.4: 129 bar), and its adiabat still runs one segment
/// of 0.3 in ln p below the join.
#[test]
fn a_join_below_one_hundred_bar_keeps_one_segment() {
    let mass = EarthMasses::new(5.0);
    let t_int = envelope_internal_temperature(Watts::new(0.0), Metres::new(1.4e7), mass, SOLAR_AGE)
        .unwrap();
    let gases = hydrogen_helium(0.194);
    let profile = EnvelopeProfile::derive(&EnvelopeProfileInputs {
        internal_temperature: t_int,
        irradiation_temperature: irradiation_temperature(Kelvin::new(600.0)),
        gravity: MetresPerSecondSquared::new(10.0),
        gases: &gases,
        grey: GreyProfile::ADOPTED,
    })
    .unwrap();
    let join = profile.join_pressure().value();
    assert!(join > 1e7, "the join at {} bar", join / 1e5);
    let last = profile.nodes()[profile.nodes().len() - 1]
        .pressure()
        .value();
    assert!(
        (math::ln(last / join) - 0.3).abs() < 1e-12,
        "the adiabat's span"
    );
}

/// The envelope profiles of the Solar System table's four giants (their hydrogen and helium,
/// P14.T50.b's He ÷ H₂, and this module's internal temperature), a 5 M⊕ sub-Neptune on the law's
/// heat at `T_eq` 600 K (its join below 100 bar) and a hot Jupiter by hand, pinned: each one's
/// inputs, Guillot's parameters, its join, skin and photosphere, every node, and `temperature_at`
/// at stated pressures, above, at and below the join and below the last node, for rendering plan
/// R08's copy of `temperature_at`'s envelope arm. Floats are their bits with Rust's shortest
/// round-trip decimal beside.
#[test]
fn the_envelope_profiles_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut cases: Vec<(String, EnvelopeProfile, f64)> = Vec::new();
    for name in ["Jupiter", "Saturn", "Uranus", "Neptune"] {
        let body = table(name);
        let ratio = helium_to_hydrogen(name, &body);
        cases.push((
            name.to_lowercase(),
            table_profile(name, GreyProfile::ADOPTED),
            ratio,
        ));
    }
    let by_hand = |t_int: f64, t_eq: f64, g: f64| {
        let gases = hydrogen_helium(0.194);
        EnvelopeProfile::derive(&EnvelopeProfileInputs {
            internal_temperature: Kelvin::new(t_int),
            irradiation_temperature: irradiation_temperature(Kelvin::new(t_eq)),
            gravity: MetresPerSecondSquared::new(g),
            gases: &gases,
            grey: GreyProfile::ADOPTED,
        })
        .unwrap()
    };
    let sub_neptune = envelope_internal_temperature(
        Watts::new(0.0),
        Metres::new(1.4e7),
        EarthMasses::new(5.0),
        SOLAR_AGE,
    )
    .expect("the sub-Neptune's internal temperature");
    cases.push((
        "sub_neptune".to_owned(),
        by_hand(sub_neptune.value(), 600.0, 10.0),
        0.194,
    ));
    cases.push((
        "hot_jupiter".to_owned(),
        by_hand(100.0, 1_450.0, 10.0),
        0.194,
    ));

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    w.line("# P14.T24.d's envelope profiles (hyperion_sim::planetary::hooks::structure::envelope,");
    w.line(
        "# with hooks::envelope_atmosphere's internal temperature): the Solar System table's four",
    );
    w.line("# giants on their hydrogen and helium, a 5 M_E sub-Neptune at T_eq 600 K and a hot");
    w.line("# Jupiter by hand (T_int 100 K, T_eq 1,450 K, 10 m/s2, protosolar helium). Per body:");
    w.line(
        "# He/H2, T_int and T_irr (K), g (m/s2), kappa_1 (m2/kg at 1 bar), n, gamma, f; its join,",
    );
    w.line(
        "# skin and photosphere; node[k] the adiabat's k-th node, pressure (Pa) then temperature",
    );
    w.line("# (K), the pressures ascending from the join; level[k] temperature_at at a stated pressure.");
    w.line("# Pure until P14.T54.b; nothing on the record reads it.");
    for (n, profile, ratio) in &cases {
        pin(&mut w, n, profile, *ratio);
    }
    golden!("planetary/envelope_profile", w.as_str());
}

/// Pins the envelope profile `profile` of He ÷ H₂ `ratio` under the label `n` (the golden's
/// header lists the columns).
fn pin(
    w: &mut hyperion_testkit::golden::GoldenWriter,
    n: &str,
    profile: &EnvelopeProfile,
    ratio: f64,
) {
    let grey = profile.grey();
    w.f64(&format!("{n}.helium_to_molecular_hydrogen"), ratio);
    w.f64(
        &format!("{n}.internal_temperature_k"),
        profile.internal_temperature().value(),
    );
    w.f64(
        &format!("{n}.irradiation_temperature_k"),
        profile.irradiation_temperature().value(),
    );
    w.f64(&format!("{n}.gravity_m_s2"), profile.gravity().value());
    w.f64(
        &format!("{n}.thermal_opacity_m2_per_kg"),
        grey.thermal_opacity_m2_per_kg(),
    );
    w.f64(
        &format!("{n}.optical_depth_exponent"),
        grey.optical_depth_exponent(),
    );
    w.f64(&format!("{n}.opacity_ratio"), grey.opacity_ratio());
    w.f64(&format!("{n}.redistribution"), grey.redistribution());
    w.f64(
        &format!("{n}.join_pressure_pa"),
        profile.join_pressure().value(),
    );
    w.f64(
        &format!("{n}.join_temperature_k"),
        profile.nodes()[0].temperature().value(),
    );
    w.f64(
        &format!("{n}.skin_temperature_k"),
        profile.skin_temperature().value(),
    );
    w.f64(
        &format!("{n}.photosphere_pressure_pa"),
        profile.photosphere_pressure().value(),
    );
    w.line(&format!("{n}.segments = {}", profile.nodes().len() - 1));
    for (k, node) in profile.nodes().iter().enumerate() {
        w.f64(
            &format!("{n}.node[{k:02}].pressure_pa"),
            node.pressure().value(),
        );
        w.f64(
            &format!("{n}.node[{k:02}].temperature_k"),
            node.temperature().value(),
        );
    }
    let join = profile.join_pressure().value();
    let levels = [
        0.0,
        1e-1,
        1e1,
        1e3,
        1e4,
        3e4,
        join * 0.999,
        join,
        1e5,
        3e5,
        1e6,
        2.2e6,
        1e7,
        1e8,
        1e9,
    ];
    for (k, p) in levels.into_iter().enumerate() {
        w.f64(&format!("{n}.level[{k:02}].pressure_pa"), p);
        w.f64(
            &format!("{n}.level[{k:02}].temperature_k"),
            profile.temperature_at(Pascals::new(p)).value(),
        );
    }
}

/// The registry's network, built once for the tests (P14.T52.a: build it once, solve at a few
/// levels).
fn network() -> &'static Network {
    static NETWORK: OnceLock<Network> = OnceLock::new();
    NETWORK.get_or_init(|| Network::registry().expect("the registry's network builds"))
}

/// (`T_eq`⁴ + `T_int`⁴)^¼, K: the envelope's effective temperature.
fn effective(t_eq: f64, t_int: f64) -> f64 {
    math::powf(math::powi(t_eq, 4) + math::powi(t_int, 4), 0.25)
}

/// An envelope by its inputs: its abundances, internal and equilibrium temperatures (K), gravity
/// (m s⁻²) and Guillot's parameters, its host at solar \[Fe/H\].
#[derive(Debug, Clone)]
struct Case {
    abundances: EnvelopeAbundances,
    internal: f64,
    equilibrium: f64,
    gravity: f64,
    grey: GreyProfile,
}

impl Case {
    /// The derivation's inputs, starlit or not.
    fn inputs(&self, starlit: bool) -> EnvelopeAtmosphereInputs<'_> {
        EnvelopeAtmosphereInputs {
            abundances: &self.abundances,
            network: network(),
            internal_temperature: Kelvin::new(self.internal),
            equilibrium_temperature: Kelvin::new(self.equilibrium),
            gravity: MetresPerSecondSquared::new(self.gravity),
            fe_h: Dex::ZERO,
            grey: self.grey,
            starlit,
        }
    }

    /// Its starlit atmosphere, or why none.
    fn derive(&self) -> Result<EnvelopeAtmosphere, DeriveEnvelopeAtmosphereError> {
        EnvelopeAtmosphere::derive(&self.inputs(true))
    }

    /// Its starlit atmosphere.
    fn atmosphere(&self) -> EnvelopeAtmosphere {
        self.derive().unwrap_or_else(|e| panic!("{e}"))
    }
}

/// The table's giant `name`: P14.T50.b's abundances about a protosolar host at the envelope's
/// `T_eff` ([`table_giant`] with this module's `T_int`), the record's `T_eq` and gravity, and the
/// adopted grey profile.
fn table_case(name: &str) -> Case {
    let (body, inputs) = table_giant(name, HostAbundances::protosolar());
    let (t_eq, t_int) = (
        body.equilibrium_temperature().value(),
        internal(&body).value(),
    );
    let abundances = EnvelopeAbundances::derive(&EnvelopeInputs {
        effective_temperature: Kelvin::new(effective(t_eq, t_int)),
        ..inputs
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    Case {
        abundances,
        internal: t_int,
        equilibrium: t_eq,
        gravity: body.surface_gravity().value(),
        grey: GreyProfile::ADOPTED,
    }
}

/// Sudarsky et al. 2000's giant at `T_eff` `t_eff` K: 1 `M_J` (envelope 0.82 of its mass, formed
/// inside its snow line, so E 2.65 and C/O 1.1 times its host's, P14.T50.b) about a solar host, at
/// Jupiter's gravity (24.79 m s⁻²), `T_int` 100 K and `T_eq` = (`T_eff`⁴ − `T_int`⁴)^¼.
fn sudarsky(t_eff: f64) -> Case {
    let abundances =
        EnvelopeAbundances::derive(&synthetic(317.83, 0.82, 300.0, (0.0, 0.0), t_eff)).unwrap();
    Case {
        abundances,
        internal: 100.0,
        equilibrium: math::powf(math::powi(t_eff, 4) - 1e8, 0.25),
        gravity: 24.79,
        grey: GreyProfile::ADOPTED,
    }
}

/// A KELT-9b-like dayside (Gaudi et al. 2017, Nature 546, 514: 2.88 `M_J`, `T_eq` 4,050 K, g about
/// 20 m s⁻²): E 1 (P14.T50.b's floor above 2.69 `M_J`), formed at 900 K, `T_int` 300 K (stated), and
/// Guillot's dayside redistribution, f = ½.
fn kelt_9b() -> Case {
    let abundances =
        EnvelopeAbundances::derive(&synthetic(915.4, 0.9, 900.0, (0.0, 0.0), 4_600.0)).unwrap();
    let grey = GreyProfile::new(
        THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG,
        OPTICAL_DEPTH_EXPONENT,
        OPACITY_RATIO,
        0.5,
    )
    .unwrap();
    Case {
        abundances,
        internal: 300.0,
        equilibrium: 4_050.0,
        gravity: 20.0,
        grey,
    }
}

/// A sub-Neptune of `earths` M⊕ with `envelope` of its mass in its envelope, formed inside its snow
/// line about a solar host, at `T_eff` 550 K on the law's heat (radius 2.8 R⊕, 1.78 × 10⁷ m), under
/// 10 m s⁻².
fn sub_neptune(earths: f64, envelope: f64) -> Case {
    let mass = EarthMasses::new(earths);
    let t_int =
        envelope_internal_temperature(Watts::new(0.0), Metres::new(1.78e7), mass, SOLAR_AGE)
            .expect("the law's heat")
            .value();
    let abundances =
        EnvelopeAbundances::derive(&synthetic(earths, envelope, 300.0, (0.0, 0.0), 550.0)).unwrap();
    Case {
        abundances,
        internal: t_int,
        equilibrium: math::powf(math::powi(550.0, 4) - math::powi(t_int, 4), 0.25),
        gravity: 10.0,
        grey: GreyProfile::ADOPTED,
    }
}

/// The share of `gas` that the row `key` holds.
fn share(gas: &[(SubstanceId, f64)], key: &str) -> f64 {
    let id = row(key);
    gas.iter().filter(|(s, _)| *s == id).map(|&(_, x)| x).sum()
}

/// The share of `gas`'s atoms of `element` that the row `key` holds.
fn element_share(gas: &[(SubstanceId, f64)], key: &str, element: Element) -> f64 {
    let atoms = |id: SubstanceId| {
        id.substance()
            .elements()
            .iter()
            .find(|(e, _)| *e == element)
            .map_or(0.0, |&(_, n)| f64::from(n))
    };
    let total: f64 = gas.iter().map(|&(id, x)| atoms(id) * x).sum();
    atoms(row(key)) * share(gas, key) / total
}

/// The deck of the condensate `key`, if one formed.
fn deck_of<'a>(atmosphere: &'a EnvelopeAtmosphere, key: &str) -> Option<&'a AerosolDeck> {
    let id = row(key);
    atmosphere.decks().iter().find(|d| d.condensate() == id)
}

/// The condensate's key.
fn key_of(id: SubstanceId) -> String {
    id.substance().key().as_str().to_owned()
}

/// The topmost deck that is optically thick enough to see, τ(550) above 0.1: the lowest base
/// pressure among them.
fn topmost(atmosphere: &EnvelopeAtmosphere) -> Option<&AerosolDeck> {
    atmosphere
        .decks()
        .iter()
        .filter(|d| {
            atmosphere
                .deck_optical_depth(d.condensate())
                .is_some_and(|tau| tau > 0.1)
        })
        .min_by(|a, b| a.base().value().total_cmp(&b.base().value()))
}

/// Whether the row `key` is among `atmosphere`'s absorbers.
fn absorbs(atmosphere: &EnvelopeAtmosphere, key: &str) -> bool {
    let id = row(key);
    atmosphere.absorbers().iter().any(|a| a.species() == id)
}

/// The table's Jupiter (P14.T50.b's abundances about a protosolar host, its record's internal
/// heat) follows the Galileo probe and its clouds: T(1 bar) within 10% of the probe's 166.1 K
/// (Seiff et al. 1998), its ammonia deck's base at 0.5–1 bar (P14.T52.c's 0.75 bar on a probe-like
/// profile), and CH₄ ÷ H₂ at 1.5–3 × 10⁻³ (Wong et al. 2004's C/H of 1.19 × 10⁻³, a CH₄ ÷ H₂ of
/// 2.4 × 10⁻³, as Guillot et al. 2022's Table 2 lists it; P14.T50.b holds the envelope's carbon
/// within a factor of two of it).
#[test]
fn jupiter_follows_the_probe_its_ammonia_cloud_and_its_methane() {
    let jupiter = table_case("Jupiter").atmosphere();
    let one_bar = at_bar(jupiter.profile(), 1.0);
    let ammonia = deck_of(&jupiter, "NH3")
        .expect("an ammonia deck")
        .base()
        .value()
        / 1e5;
    let methane = share(jupiter.composition(), "CH4") / share(jupiter.composition(), "H2");
    assert!((one_bar - 166.1).abs() < 16.61, "T(1 bar) {one_bar} K");
    assert!((0.5..1.0).contains(&ammonia), "NH3's base at {ammonia} bar");
    assert!((1.5e-3..3e-3).contains(&methane), "CH4/H2 {methane}");
}

/// Sudarsky et al. 2000's class I, "Jovian" ammonia clouds below about 150 K of `T_eff` (their
/// abstract; the class reads the envelope's `T_eff`, (`T_eq`⁴ + `T_int`⁴)^¼, decision-p14-t24d-profile
/// §5): at 130 K the highest deck of τ(550) above 0.1 is ammonia's.
#[test]
fn a_cold_giant_s_highest_cloud_is_ammonia() {
    let giant = sudarsky(130.0).atmosphere();
    let top = topmost(&giant).expect("a visible deck");
    assert_eq!(key_of(top.condensate()), "NH3");
}

/// Sudarsky et al. 2000's class II, water clouds near 250 K of `T_eff` (their §1 and §9, not their
/// abstract): at 250 K the highest deck of τ(550) above 0.1 is water's, and no ammonia deck forms.
#[test]
fn a_temperate_giant_s_highest_cloud_is_water() {
    let giant = sudarsky(250.0).atmosphere();
    let top = topmost(&giant).expect("a visible deck");
    assert_eq!(key_of(top.condensate()), "H2O");
    assert!(deck_of(&giant, "NH3").is_none(), "an ammonia deck at 250 K");
}

/// Sudarsky et al. 2000's class III, clear above about 350 K of `T_eff` (their §1 and §9): at 400 K
/// no volatile's deck lies at or above the thermal photosphere (τ = ⅔).
#[test]
fn a_warm_giant_is_clear_above_its_photosphere() {
    let giant = sudarsky(400.0).atmosphere();
    let photosphere = giant.profile().photosphere_pressure().value();
    for deck in giant.decks() {
        let volatile = DeckKind::of(deck.condensate()) == Some(DeckKind::Volatile);
        assert!(
            !(volatile && deck.base().value() <= photosphere),
            "{}'s base at {} Pa, the photosphere at {photosphere} Pa",
            key_of(deck.condensate()),
            deck.base().value()
        );
    }
}

/// Sudarsky et al. 2000's class IV, the alkalis at 900–1,500 K of `T_eff` (their abstract): at
/// 1,200 K sodium's and potassium's atoms are absorbers above the floor at 10 mbar, and no silicate
/// deck's base lies at or above the photosphere.
#[test]
fn a_hot_giant_shows_its_alkalis_over_buried_silicates() {
    let giant = sudarsky(1_200.0).atmosphere();
    assert!(absorbs(&giant, "Na") && absorbs(&giant, "K"), "the alkalis");
    let photosphere = giant.profile().photosphere_pressure().value();
    for key in ["Mg2SiO4", "MgSiO3"] {
        if let Some(deck) = deck_of(&giant, key) {
            assert!(
                deck.base().value() > photosphere,
                "{key}'s base at {} Pa, the photosphere at {photosphere} Pa",
                deck.base().value()
            );
        }
    }
}

/// Sudarsky et al. 2000's class V, silicate clouds high in the atmosphere above about 1,500 K of
/// `T_eff` (their abstract): at 1,800 K a silicate deck's base lies at or above the thermal
/// photosphere (its base and top printed against it). The grey profile draws it in a window: below
/// about 1,650 K the silicates' base lies at their adiabat crossing, a hundred bars or more down, and
/// from about 1,850 K no silicate deck forms (recorded).
#[test]
fn a_roaster_s_silicates_reach_its_photosphere() {
    let giant = sudarsky(1_800.0).atmosphere();
    let photosphere = giant.profile().photosphere_pressure().value();
    let deck = ["Mg2SiO4", "MgSiO3"]
        .into_iter()
        .filter_map(|key| deck_of(&giant, key))
        .min_by(|a, b| a.base().value().total_cmp(&b.base().value()))
        .expect("a silicate deck");
    eprintln!(
        "class V: {} base {} Pa, top {} Pa, photosphere {photosphere} Pa",
        key_of(deck.condensate()),
        deck.base().value(),
        deck.top().value()
    );
    assert!(
        deck.base().value() <= photosphere,
        "{}'s base at {} Pa, top {} Pa, the photosphere at {photosphere} Pa",
        key_of(deck.condensate()),
        deck.base().value(),
        deck.top().value()
    );
}

/// The share of the envelope's titanium that TiO holds in `atmosphere`'s 10-mbar gas: its share of
/// the gas over the envelope's Ti per molecule, Ti ÷ H times the gas's hydrogen atoms per molecule.
fn titanium_oxide_share(atmosphere: &EnvelopeAtmosphere, case: &Case) -> f64 {
    let gas = atmosphere.upper_gas();
    let hydrogen: f64 = gas
        .iter()
        .map(|&(id, x)| {
            id.substance()
                .elements()
                .iter()
                .find(|(e, _)| *e == Element::H)
                .map_or(0.0, |&(_, n)| f64::from(n) * x)
        })
        .sum();
    share(gas, "TiO") / (case.abundances.per_hydrogen(Element::Ti) * hydrogen)
}

/// The `T_eq` (K) bracket, from 1,000 to 2,000 K and ten halvings, at which `holds` first holds on
/// Sudarsky's giant.
fn bisect_t_eq(holds: impl Fn(&EnvelopeAtmosphere, &Case) -> bool) -> (f64, f64) {
    let at = |t_eq: f64| {
        let case = sudarsky(effective(t_eq, 100.0));
        holds(&case.atmosphere(), &case)
    };
    let (mut cold, mut hot) = (1_000.0, 2_000.0);
    for _ in 0..10 {
        let middle = f64::midpoint(cold, hot);
        if at(middle) {
            hot = middle;
        } else {
            cold = middle;
        }
    }
    (cold, hot)
}

/// Fortney et al. 2008's pM and pL classes split by TiO in the upper air: on Sudarsky's giant at
/// `T_eq` 2,000 K (pM) TiO is an absorber above the floor at 10 mbar and holds at least 1% of the
/// envelope's titanium; at 1,000 K (pL) it is cold-trapped below (CaTiO₃'s deck; Spiegel, Silverio
/// and Burrows 2009). The split is bisected in `T_eq` by both, and printed against Fortney et al.'s
/// 0.04–0.05 au about a Sun-like star (`T_eq` 1,246–1,393 K at A = 0) and Baxter et al. 2020's
/// observed 1,660 ± 100 K (A&A 639, A36, arXiv:2007.15287): it falls higher, CaTiO₃'s deep cold
/// trap near the join setting it, which Fortney et al. do not take.
#[test]
fn titanium_oxide_splits_the_pm_and_pl_classes() {
    let at = |t_eq: f64| {
        let case = sudarsky(effective(t_eq, 100.0));
        (case.atmosphere(), case)
    };
    let (hot, hot_case) = at(2_000.0);
    assert!(absorbs(&hot, "TiO"), "TiO listed at 2,000 K");
    let share = titanium_oxide_share(&hot, &hot_case);
    assert!(share >= 0.01, "TiO holds {share} of the Ti at 2,000 K");
    assert!(!absorbs(&at(1_000.0).0, "TiO"), "TiO at 1,000 K");
    let listed = bisect_t_eq(|a, _| absorbs(a, "TiO"));
    let held = bisect_t_eq(|a, case| titanium_oxide_share(a, case) >= 0.01);
    eprintln!(
        "TiO's split at T_eq {listed:?} K listed, {held:?} K at 1% of the Ti, against Fortney et \
         al.'s 1,246–1,393 K and Baxter et al.'s 1,660 ± 100 K"
    );
    assert!(listed.0 < listed.1 && held.0 < held.1);
}

/// A KELT-9b-like dayside at 10 mbar (Kitzmann et al. 2018, ApJ 863, 183, Fig. 2; Lothringer,
/// Barman and Koskinen 2018, ApJ 866, 27), where the profile is above 4,300 K: water and TiO
/// dissociated, each under 1% of its oxygen or titanium; Fe⁺ above Fe (above about 4,300 K at
/// 10 mbar) and Ca⁺ above Ca (above about 3,000 K); and every one of P14.T50.b's metals' atoms an
/// absorber.
#[test]
fn a_kelt_9b_like_dayside_dissociates_and_ionises_at_ten_millibar() {
    let dayside = kelt_9b().atmosphere();
    let t = dayside.profile().temperature_at(ABSORBER_LEVEL).value();
    assert!(t > 4_300.0, "T(10 mbar) {t} K");
    let gas = dayside.upper_gas();
    let water = element_share(gas, "H2O", Element::O);
    let oxide = element_share(gas, "TiO", Element::Ti);
    assert!(water < 0.01 && oxide < 0.01, "H2O {water}, TiO {oxide}");
    assert!(share(gas, "Fe+") > share(gas, "Fe"), "Fe+ against Fe");
    assert!(share(gas, "Ca+") > share(gas, "Ca"), "Ca+ against Ca");
    for metal in [
        "Na", "K", "Mg", "Al", "Si", "Ca", "Ti", "V", "Cr", "Mn", "Fe", "Ni", "Zn",
    ] {
        assert!(absorbs(&dayside, metal), "{metal}");
    }
}

/// The chromophore crowns a Jupiter-like ammonia cloud alone, an NH₃ deck with no CH₄ deck above it
/// (Braude et al. 2020 retrieve it on Jupiter alone): the table's Jupiter and Saturn carry it, and
/// Uranus and Neptune, whose NH₃ decks lie at 7–8 bar under their methane, do not.
#[test]
fn the_chromophore_crowns_a_jupiter_like_ammonia_cloud_alone() {
    let chromophore = row("chromophore");
    for (name, carries) in [
        ("Jupiter", true),
        ("Saturn", true),
        ("Uranus", false),
        ("Neptune", false),
    ] {
        let atmosphere = table_case(name).atmosphere();
        let found = atmosphere
            .modes()
            .iter()
            .any(|m| m.material() == chromophore);
        assert_eq!(found, carries, "{name}");
    }
}

/// Soot is drawn only within Gao et al. 2020's grid (`T_eq` 700–2,800 K): the table's Jupiter and
/// Saturn carry none, and at `T_eq` 650 K the soot is half the rule's column, its taper's middle.
#[test]
fn soot_is_drawn_only_within_gao_s_grid() {
    let soot_row = row("soot");
    for name in ["Jupiter", "Saturn"] {
        let atmosphere = table_case(name).atmosphere();
        assert!(
            atmosphere.modes().iter().all(|m| m.material() != soot_row),
            "{name}'s soot"
        );
    }
    let case = sudarsky(effective(650.0, 100.0));
    let atmosphere = case.atmosphere();
    let drawn = atmosphere
        .modes()
        .iter()
        .find(|m| m.material() == soot_row)
        .and_then(AerosolMode::column_mass)
        .expect("soot at 650 K")
        .value();
    let gas = atmosphere.upper_gas();
    let air = UpperAtmosphere {
        equilibrium_temperature: Kelvin::new(case.equilibrium),
        stratosphere_temperature: atmosphere.profile().skin_temperature(),
        metallicity: case.abundances.enrichment(),
        gravity: MetresPerSecondSquared::new(case.gravity),
        molar_mass_g_per_mol: mean_molar_mass_g_per_mol(gas).unwrap(),
    };
    let rule = soot(&air, share(gas, "CH4")).unwrap().unwrap();
    let HazeAmount::ColumnMass(column) = rule.amount() else {
        panic!("soot's amount is a column");
    };
    assert!(
        (drawn / (0.5 * column.value()) - 1.0).abs() < 1e-9,
        "{drawn} against half of {}",
        column.value()
    );
}

/// A level the record reads hotter than the network's 6,000 K is refused with its own variant, not
/// "not modelled": a KELT-9b-like dayside at `T_eq` 5,500 K, whose 1 bar is near 7,300 K.
#[test]
fn a_read_level_above_the_records_is_refused() {
    let case = Case {
        equilibrium: 5_500.0,
        ..kelt_9b()
    };
    let error = case.derive().unwrap_err();
    assert!(
        matches!(
            error,
            DeriveEnvelopeAtmosphereError::LevelAboveRecords { temperature, .. }
                if temperature.value() > 6_000.0
        ),
        "{error:?}"
    );
    assert!(!error.is_not_modelled());
}

/// The atmospheres are the same whatever was derived before them (the derivation reads no state of
/// its own): Jupiter, the KELT-9b-like dayside and the sub-Neptune, forwards, backwards, permuted and
/// alone.
#[test]
fn the_atmospheres_do_not_depend_on_their_order() {
    let cases = [table_case("Jupiter"), kelt_9b(), sub_neptune(8.0, 0.03)];
    hyperion_testkit::order::assert_order_independent(&cases, |case| {
        format!("{:?}", case.atmosphere())
    });
}

/// A sub-Neptune's envelope is modelled (P14.T52.d): 8 M⊕ with 3% of it in its envelope, about 100
/// times its host's metals (P14.T50.b), still hydrogen-dominated (P14.T52.a's bound near 4–5 M⊕), at
/// `T_eff` 550 K. Its composition sums to 1, hydrogen first.
#[test]
fn a_sub_neptune_s_envelope_is_modelled() {
    let case = sub_neptune(8.0, 0.03);
    assert!(
        (60.0..150.0).contains(&case.abundances.enrichment()),
        "E {}",
        case.abundances.enrichment()
    );
    let envelope = case.atmosphere();
    let total: f64 = envelope.composition().iter().map(|&(_, x)| x).sum();
    assert!((total - 1.0).abs() < 1e-9, "{total}");
    assert_eq!(key_of(envelope.composition()[0].0), "H2");
}

/// An envelope past P14.T52.a's hydrogen dominance (3 M⊕, its metals about 260 times its host's) is
/// refused with its own variant, for which the record's whole envelope section is not modelled
/// (decision-p14-t24d-profile §4).
#[test]
fn an_envelope_past_hydrogen_dominance_is_not_modelled() {
    let refused = sub_neptune(3.0, 0.02).derive();
    assert!(
        matches!(
            refused,
            Err(DeriveEnvelopeAtmosphereError::NotHydrogenDominated { share }) if share > 0.5
        ),
        "{refused:?}"
    );
    assert!(refused.is_err_and(|e| e.is_not_modelled()));
}

/// The atmosphere is continuous in its inputs away from a deck's onset (each deck's onset, and its
/// haze's, is a state change): a part in 10⁶ of `T_eq`, `T_int` or g moves the table's Jupiter's
/// temperatures, its composition's shares of at least 10⁻⁶ and each deck's τ by under 10⁻⁴, the
/// same decks forming.
#[test]
fn the_atmosphere_is_continuous_in_its_inputs() {
    let base = table_case("Jupiter");
    let reference = base.atmosphere();
    let mut moved = Vec::new();
    for k in 0..3 {
        let mut case = base.clone();
        match k {
            0 => case.equilibrium *= 1.0 + 1e-6,
            1 => case.internal *= 1.0 + 1e-6,
            _ => case.gravity *= 1.0 + 1e-6,
        }
        moved.push(case.atmosphere());
    }
    let relative = |a: f64, b: f64| ((a - b) / b).abs();
    for (k, other) in moved.iter().enumerate() {
        for p in [1e2, 1e3, 1e4, 1e5, 1e6, 1e7] {
            let (a, b) = (
                at_bar(other.profile(), p / 1e5),
                at_bar(reference.profile(), p / 1e5),
            );
            assert!(
                relative(a, b) < 1e-4,
                "input {k}: T({p} Pa) {a} against {b}"
            );
        }
        for &(id, x) in reference.composition().iter().filter(|(_, x)| *x >= 1e-6) {
            let y = share(other.composition(), &key_of(id));
            assert!(
                relative(y, x) < 1e-4,
                "input {k}: {} {y} against {x}",
                key_of(id)
            );
        }
        let condensates = |a: &EnvelopeAtmosphere| {
            a.decks()
                .iter()
                .map(AerosolDeck::condensate)
                .collect::<Vec<_>>()
        };
        assert_eq!(condensates(other), condensates(&reference), "input {k}");
        for deck in reference.decks() {
            if let Some(tau) = reference.deck_optical_depth(deck.condensate()) {
                let moved_tau = other.deck_optical_depth(deck.condensate()).unwrap();
                assert!(
                    relative(moved_tau, tau) < 1e-4,
                    "input {k}: {}'s τ {moved_tau} against {tau}",
                    key_of(deck.condensate())
                );
            }
        }
    }
}

/// The same inputs give the same bits twice (the golden's cases).
#[test]
fn the_same_inputs_give_the_same_bits() {
    for (name, case) in golden_cases() {
        let (a, b) = (case.atmosphere(), case.atmosphere());
        assert_eq!(a, b, "{name}");
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "{name}");
    }
}

/// Every deck takes its kind's rule (decision-bump-plan-after-22, "The decks' `f_sed`, K and
/// humidity", G5): the table's Jupiter forms a deck of each kind; a refractory's `f_sed` is 2, a
/// sulphide's or a salt's 4.5, and a volatile's its supply's (P14.T24.c's v₁ `M_vap`^β H ÷ K at its
/// published base and the published composition's molar mass, within 2%: the supply is read at the
/// first pass's base, in the 1-bar gas trimmed by the first pass's decks).
#[test]
fn each_deck_takes_its_kind_s_sedimentation() {
    let case = table_case("Jupiter");
    let jupiter = case.atmosphere();
    let mut kinds = Vec::new();
    for deck in jupiter.decks() {
        let condensate = deck.condensate();
        let kind = DeckKind::of(condensate).expect("a law aloft");
        let f_sed = deck.sedimentation();
        if let Some(fixed) = kind.fixed_sedimentation() {
            assert_same_bits(f_sed, fixed);
        } else {
            let law = law_aloft(condensate).expect("a law aloft");
            let mu = jupiter.mean_molar_mass_g_per_mol();
            let epsilon = condensate.substance().molar_mass_g_per_mol().unwrap()
                / f64::from(law.limiting().count())
                / mu;
            let supply = epsilon * deck.vapour_below() * deck.base().value() / case.gravity;
            let speed = SEDIMENTATION_SPEED_AT_UNIT_SUPPLY.value()
                * math::powf(supply, SEDIMENTATION_SUPPLY_EXPONENT);
            let height =
                MOLAR_GAS_CONSTANT * deck.base_temperature().value() / (mu / 1e3 * case.gravity);
            let rule = (speed * height / CLOUD_EDDY_DIFFUSION.square_metres_per_second())
                .max(LEAST_SEDIMENTATION_EFFICIENCY);
            assert!(
                (f_sed / rule - 1.0).abs() < 0.02,
                "{}: f_sed {f_sed} against its supply's {rule}",
                key_of(condensate)
            );
        }
        kinds.push(kind);
    }
    for kind in [
        DeckKind::Volatile,
        DeckKind::Refractory,
        DeckKind::SulphideOrSalt,
    ] {
        assert!(kinds.contains(&kind), "no {kind:?} deck on Jupiter");
    }
}

/// The kinds are a closed set: every row with a law aloft has one, and the refractories and the
/// sulphides and salts are exactly the rows with a vapour law, apart, each a registry row.
#[test]
fn the_deck_kinds_are_a_closed_set() {
    let mut vapour_laws = Vec::new();
    for (id, law) in laws_aloft() {
        assert!(DeckKind::of(id).is_some(), "{}", key_of(id));
        if matches!(law.law(), SaturationLaw::Vapour { .. }) {
            vapour_laws.push(key_of(id));
        }
    }
    let mut listed: Vec<String> = REFRACTORY_CONDENSATES
        .iter()
        .chain(&SULPHIDE_AND_SALT_CONDENSATES)
        .map(|&key| key.to_owned())
        .collect();
    for key in &listed {
        assert!(by_key(key).is_some(), "{key} is a row");
    }
    for key in REFRACTORY_CONDENSATES {
        assert!(!SULPHIDE_AND_SALT_CONDENSATES.contains(&key), "{key} twice");
    }
    listed.sort();
    vapour_laws.sort();
    assert_eq!(listed, vapour_laws);
}

/// Each species is trimmed by one vapour at most: over the network's species and every law aloft's
/// vapours, none is both a drawn species and a carrier of a drawn element, nor carries two drawn
/// elements, nor two of [`CO_TRAPPED`]'s metals; and no co-trapped metal has a law of its own.
#[test]
fn each_species_is_trimmed_by_one_vapour() {
    let mut vapours = Vec::new();
    for (_, law) in laws_aloft() {
        for reactant in core::iter::once(law.limiting()).chain(law.others().iter().copied()) {
            vapours.push(reactant.vapour());
        }
    }
    for id in network().species() {
        let own = vapours.contains(&Vapour::Species(id.substance().key()));
        let elements = id
            .substance()
            .elements()
            .iter()
            .filter(|&&(e, _)| vapours.contains(&Vapour::Element(e)))
            .count();
        let co_trapped = id
            .substance()
            .elements()
            .iter()
            .filter(|&&(e, _)| CO_TRAPPED.iter().any(|&(metal, _)| metal == e))
            .count();
        assert!(
            !(own && elements > 0) && elements <= 1 && co_trapped <= 1,
            "{}: its own vapour {own}, {elements} drawn elements, {co_trapped} co-trapped",
            key_of(id)
        );
    }
    for (metal, host) in CO_TRAPPED {
        assert!(
            !laws_aloft().any(|(_, law)| law.limiting().vapour() == Vapour::Element(metal)),
            "{metal:?} has a law of its own"
        );
        assert!(
            laws_aloft().any(|(_, law)| law.limiting().vapour() == Vapour::Element(host)),
            "{host:?} limits a law"
        );
    }
}

/// Nickel, vanadium and calcium, which no law aloft takes, are held with their hosts' decks
/// ([`CO_TRAPPED`]): on the table's Jupiter none of Ca, Ni, V or VO reaches the floor in its 1-bar
/// composition or among its absorbers, while on the KELT-9b-like dayside, where no deck forms, the
/// atoms of all three are absorbers.
#[test]
fn nickel_vanadium_and_calcium_fall_with_their_hosts() {
    let jupiter = table_case("Jupiter").atmosphere();
    for key in ["Ca", "Ni", "V", "VO"] {
        assert!(!absorbs(&jupiter, key), "Jupiter's {key} absorbs");
        assert!(
            share(jupiter.composition(), key) < COMPOSITION_FLOOR,
            "Jupiter's {key} at 1 bar"
        );
    }
    let dayside = kelt_9b().atmosphere();
    for key in ["Ca", "Ni", "V"] {
        assert!(absorbs(&dayside, key), "the dayside's {key}");
    }
}

/// The absorbers are the network's species, and FeH, the plan's other, has no thermochemistry
/// (P14.T49.d's `NOT_MODELLED`): it is the atmosphere's one row not modelled.
#[test]
fn the_absorbers_are_the_network_s_and_iron_hydride_is_not_modelled() {
    let species: Vec<SubstanceId> = network().species().collect();
    for key in ABSORBERS {
        assert!(species.contains(&row(key)), "{key}");
    }
    let iron_hydride = row("FeH");
    assert!(!species.contains(&iron_hydride));
    let jupiter = table_case("Jupiter").atmosphere();
    assert_eq!(jupiter.not_modelled(), [iron_hydride]);
}

/// An envelope no star lights has no photochemistry: no haze, no SO₂ (P14.T52.b's rules read the
/// host's ultraviolet).
#[test]
fn an_unlit_envelope_has_no_photochemistry() {
    let case = table_case("Jupiter");
    let lit = case.atmosphere();
    let unlit = EnvelopeAtmosphere::derive(&case.inputs(false)).unwrap();
    assert!(lit.haze_optical_depth() > 0.0, "the lit Jupiter's hazes");
    assert_same_bits(unlit.haze_optical_depth(), 0.0);
    let hazes = ["S8", "soot", "ice_giant_haze", "chromophore"].map(row);
    assert!(unlit.modes().iter().all(|m| !hazes.contains(&m.material())));
    assert!(!absorbs(&unlit, "SO2"));
    assert_eq!(unlit.decks(), lit.decks());
}

/// The composition is whole, floored and sorted: its shares sum to 1 within 10⁻⁹ (P14.T54.b's
/// `composition`'s), none is under the floor, each is a gas row, and they run largest first.
#[test]
fn the_composition_is_floored_sorted_and_whole() {
    for (name, case) in golden_cases() {
        let atmosphere = case.atmosphere();
        let composition = atmosphere.composition();
        let total: f64 = composition.iter().map(|&(_, x)| x).sum();
        assert!((total - 1.0).abs() < 1e-9, "{name}: {total}");
        for pair in composition.windows(2) {
            assert!(pair[0].1 >= pair[1].1, "{name}: sorted");
        }
        for &(id, x) in composition {
            assert!(x >= COMPOSITION_FLOOR, "{name}: {} {x}", key_of(id));
            assert!(id.substance().gas().is_some(), "{name}: {}", key_of(id));
        }
        let mu = atmosphere.mean_molar_mass_g_per_mol();
        let cp = atmosphere.heat_capacity_over_r();
        assert!(mu > 1.0 && cp > 2.4, "{name}: mu {mu}, c_p/R {cp}");
    }
}

/// Its inputs are checked: an internal temperature of 0 or NaN, a negative equilibrium temperature,
/// a gravity of 0, a NaN \[Fe/H\] and one whose power of ten overflows are each refused by their own
/// variant, none of them "not modelled"; an equilibrium temperature of −0 K is +0's.
#[test]
fn the_inputs_are_checked() {
    let case = table_case("Jupiter");
    let with = |change: &dyn Fn(&mut EnvelopeAtmosphereInputs<'_>)| {
        let mut inputs = case.inputs(true);
        change(&mut inputs);
        EnvelopeAtmosphere::derive(&inputs).unwrap_err()
    };
    for t in [0.0, f64::NAN] {
        let error = with(&|i| i.internal_temperature = Kelvin::new(t));
        assert!(
            matches!(error, DeriveEnvelopeAtmosphereError::InternalTemperature(_)),
            "{error:?}"
        );
        assert!(!error.is_not_modelled());
    }
    assert!(matches!(
        with(&|i| i.equilibrium_temperature = Kelvin::new(-1.0)),
        DeriveEnvelopeAtmosphereError::EquilibriumTemperature(_)
    ));
    assert!(matches!(
        with(&|i| i.gravity = MetresPerSecondSquared::new(0.0)),
        DeriveEnvelopeAtmosphereError::Gravity(_)
    ));
    assert!(matches!(
        with(&|i| i.fe_h = Dex::new(f64::NAN)),
        DeriveEnvelopeAtmosphereError::FeH(_)
    ));
    assert!(matches!(
        with(&|i| i.fe_h = Dex::new(400.0)),
        DeriveEnvelopeAtmosphereError::Metallicity(_)
    ));
    let neutral = EnvelopeAtmosphere::derive(&EnvelopeAtmosphereInputs {
        equilibrium_temperature: Kelvin::new(-0.0),
        starlit: false,
        ..case.inputs(true)
    })
    .unwrap();
    let positive = EnvelopeAtmosphere::derive(&EnvelopeAtmosphereInputs {
        equilibrium_temperature: Kelvin::new(0.0),
        starlit: false,
        ..case.inputs(true)
    })
    .unwrap();
    assert_eq!(
        format!("{neutral:?}"),
        format!("{positive:?}"),
        "−0 K as +0"
    );
}

/// Every gas envelope of the Solar System table, and every case of the goldens, has an internal
/// temperature that is `Ok`, finite and positive (main's check: no NaN from `T_int` reaches the
/// record).
#[test]
fn every_table_envelope_s_internal_temperature_is_finite_and_positive() {
    let mut envelopes = 0;
    for (name, body) in solar_system() {
        if body.atmosphere().state() != SurfaceState::GasEnvelope {
            continue;
        }
        let t = envelope_internal_temperature(
            body.internal_luminosity(),
            Metres::from(body.radius()),
            body.mass(),
            SOLAR_AGE,
        );
        assert!(
            t.is_ok_and(|t| t.value().is_finite() && t.value() > 0.0),
            "{name}: {t:?}"
        );
        envelopes += 1;
    }
    assert!(envelopes >= 4, "{envelopes} envelopes");
    for (name, case) in golden_cases() {
        assert!(
            case.internal.is_finite() && case.internal > 0.0,
            "{name}: {}",
            case.internal
        );
    }
    let sub_neptune = envelope_internal_temperature(
        Watts::new(0.0),
        Metres::new(1.4e7),
        EarthMasses::new(5.0),
        SOLAR_AGE,
    );
    assert!(sub_neptune.is_ok_and(|t| t.value() > 0.0));
}

/// The files outside this module whose code reaches the envelope's atmosphere behind a named
/// hold, each with the name of its hold, which its code (outside an inline test module, comments
/// left out) must name: P14.T54.b.i's wiring of the record's envelope section, behind the air's
/// sections' hold (`AirRules`), and its tests (decision-p14-t54b-open §3.3). **P14.T54.b.l deletes
/// this list** with the tripwire below.
const HELD: [(&str, &str); 2] = [
    ("planetary/system/envelope.rs", "AirRules"),
    ("planetary/system/envelope/tests.rs", "AirRules"),
];

/// Whether the code of `text` (its lines before an inline `mod tests {`, comments left out) names
/// `hold`.
fn names_its_hold(text: &str, hold: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .take_while(|code| !code.starts_with("mod tests {"))
        .filter(|code| !code.starts_with("//"))
        .any(|code| code.contains(hold))
}

/// The hold check reads the code alone: a comment naming the hold, or code inside an inline test
/// module, does not count.
#[test]
fn the_envelope_s_hold_check_reads_the_code_alone() {
    assert!(names_its_hold(
        "match rules { AirRules::Held => 0 }\n",
        "AirRules"
    ));
    assert!(!names_its_hold("// AirRules\nfn f() {}\n", "AirRules"));
    assert!(!names_its_hold(
        "fn f() {}\n#[cfg(test)]\nmod tests {\n    use super::AirRules;\n}\n",
        "AirRules"
    ));
}

/// Nothing on the record calls the envelope's atmosphere but through its hold before P14.T54.b's
/// bump (decision-bump-plan-after-22 §1.3): no code of the crate's `src/` outside this module, its
/// tests and [`HELD`]'s files calls it or builds its inputs, comments, its module line and
/// `hooks`' re-export (whatever lines it takes) left out; each of [`HELD`]'s files is a source file
/// whose code names its hold, so that a caller in any other file fails, and so does a listed file
/// that drops its hold (decision-p14-t54b-open §3.3). The bare type is not matched: the record's
/// envelope (`record::Envelope`) stores an [`EnvelopeAtmosphere`] and reads only what it holds, as
/// the inventory's scan reads its type; under the hold's `Held` arm the record calls nothing here
/// (`the_held_envelope_computes_nothing`). **P14.T54.b.l deletes this tripwire.**
#[test]
fn nothing_on_the_record_calls_the_envelope_atmosphere_yet() {
    fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .expect("a source directory")
            .map(|e| e.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, files);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&src, &mut files);
    assert!(files.len() > 100, "the walk found the crate's sources");
    // Its calls and the inputs it would be called with, by name.
    let patterns = [
        "EnvelopeAtmosphere::derive",
        "EnvelopeAtmosphereInputs",
        "envelope_atmosphere::",
        "envelope_internal_temperature",
        "cooling_law_temperature",
        "DeckKind",
    ];
    let relative_of = |path: &std::path::Path| {
        path.strip_prefix(&src)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/")
    };
    for (held, hold) in HELD {
        let path = files
            .iter()
            .find(|path| relative_of(path.as_path()) == held)
            .unwrap_or_else(|| panic!("{held} is a source file"));
        let text = std::fs::read_to_string(path).expect("a readable source");
        assert!(
            names_its_hold(&text, hold),
            "{held} calls the envelope's atmosphere behind {hold}, which its code no longer names"
        );
    }
    let mut found = Vec::new();
    for path in &files {
        let relative = relative_of(path);
        if relative.starts_with("planetary/hooks/envelope_atmosphere")
            || HELD.iter().any(|(held, _)| *held == relative)
        {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("a readable source");
        let mut in_reexport = false;
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if in_reexport {
                in_reexport = !code.trim_end().ends_with(';');
                continue;
            }
            if relative == "planetary/hooks/mod.rs" {
                if code == "pub mod envelope_atmosphere;" {
                    continue;
                }
                if code.starts_with("pub use envelope_atmosphere::") {
                    in_reexport = !code.trim_end().ends_with(';');
                    continue;
                }
            }
            if code.starts_with("//") {
                continue;
            }
            if patterns.iter().any(|p| code.contains(p)) {
                found.push(format!("{relative}:{}: {code}", n + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "nothing on the record calls it but through its hold before P14.T54.b: {found:#?}"
    );
}

/// No haze's material can be a deck's condensate, so that every mode of an envelope is a deck's or
/// a haze's by whether its deck is a listed deck's condensate (the record's and the server's rule,
/// P14.T54.b.i): P14.T52.c's loop forms decks of the rows with a law aloft alone
/// ([`laws_aloft`]), and none of the hazes' four materials has one (S₈ is listed as not modelled
/// aloft, a photochemical haze rather than a deep vapour's deck; soot, the ice giants' haze and the
/// chromophore have no vapour). On the table's Jupiter and Neptune the modes are the decks' first,
/// each naming a listed deck, then the hazes'.
#[test]
fn no_haze_material_condenses_as_a_deck() {
    let hazes = ["S8", "soot", "ice_giant_haze", "chromophore"].map(row);
    for (id, _) in laws_aloft() {
        assert!(!hazes.contains(&id), "{} has a law aloft", key_of(id));
    }
    for name in ["Jupiter", "Neptune"] {
        let atmosphere = table_case(name).atmosphere();
        let decks: Vec<SubstanceId> = atmosphere
            .decks()
            .iter()
            .map(AerosolDeck::condensate)
            .collect();
        let deck_modes: usize = atmosphere.decks().len();
        let split = atmosphere
            .modes()
            .iter()
            .position(|mode| !decks.contains(&mode.deck()))
            .unwrap_or(atmosphere.modes().len());
        for (k, mode) in atmosphere.modes().iter().enumerate() {
            assert_eq!(k < split, decks.contains(&mode.deck()), "{name} mode {k}");
            assert_eq!(
                k >= split,
                hazes.contains(&mode.material()),
                "{name} mode {k}"
            );
        }
        assert!(
            split >= deck_modes && split < atmosphere.modes().len(),
            "{name}: {split}"
        );
    }
}

/// Recorded, not asserted beyond being finite (printed with `--nocapture`): Jupiter's NH₄SH deck's τ
/// against Irwin et al. 2026's 1–2 bar cloud, τ 6–38 (decision-bump-plan-after-22, G2); Saturn's
/// hazes' τ against the plan's ~10, its tropospheric haze (Fletcher et al. 2018); Neptune's CH₄
/// deck's base and τ against T24.c's measured-profile 5.82, Irwin et al. 2022's 0.5–4 and R08's
/// cloud-deck split of 10 (G3: the "lower K" route is spent, decision-p14-t24d-profile §5).
#[test]
fn the_giants_records_are_printed() {
    let jupiter = table_case("Jupiter").atmosphere();
    let saturn = table_case("Saturn").atmosphere();
    let neptune = table_case("Neptune").atmosphere();
    let salt = jupiter.deck_optical_depth(row("NH4SH")).unwrap_or(f64::NAN);
    let haze = saturn.haze_optical_depth();
    let methane = neptune.deck_optical_depth(row("CH4")).unwrap_or(f64::NAN);
    let base = deck_of(&neptune, "CH4").map_or(f64::NAN, |d| d.base().value() / 1e5);
    eprintln!(
        "records: Jupiter NH4SH tau {salt} (6-38); Saturn haze tau {haze} (~10); Neptune CH4 \
         base {base} bar, tau {methane} (5.82; 0.5-4; under 10: {})",
        methane < 10.0
    );
    assert!(salt.is_finite() && haze.is_finite() && methane.is_finite() && base.is_finite());
}

/// The τ(550) of a deck of τ `tau` between its base `base` and top `top` (Pa), its mixing ratio as
/// (p ÷ `p_b`)^`f_sed`, that lies above `level`: τ ((p ÷ `p_b`)^(f+1) − (`p_t` ÷ `p_b`)^(f+1)) ÷
/// (1 − (`p_t` ÷ `p_b`)^(f+1)) at p = `level`, 0 above the top and all of it below the base
/// (decision-deep-decks-split §8.1).
fn optical_depth_above(tau: f64, base: f64, top: f64, f_sed: f64, level: f64) -> f64 {
    if level <= top {
        return 0.0;
    }
    if level >= base {
        return tau;
    }
    let power = |p: f64| math::powf(p / base, f_sed + 1.0);
    tau * (power(level) - power(top)) / (1.0 - power(top))
}

/// The τ(550) of a deck's mode in Ackerman and Marley's geometric limit, (3 ÷ 2) M ÷ (ρ `r_eff`)
/// (their eq. 16, `Q_ext` = 2), where its size and density are known but its optics are not (a
/// liquid refractory has no index file): a stand-in for the stop line, not a modelled τ.
fn geometric_optical_depth(
    atmosphere: &EnvelopeAtmosphere,
    k: usize,
    deck: &AerosolDeck,
) -> Option<f64> {
    let mode = atmosphere
        .modes()
        .iter()
        .find(|m| m.deck() == deck.condensate())?;
    let Ok(ParticleSize::Distribution(size)) = mode.size() else {
        return None;
    };
    let density = deck
        .condensate()
        .substance()
        .grain_density(deck.phase(), deck.base_temperature())?;
    let column = atmosphere.deck_column_masses()[k].value();
    Some(1.5 * column / (density.value() * size.effective_radius().value()))
}

/// The refractory and salt decks of `atmosphere`, one line each: τ(550) (or why its mode has none,
/// and its geometric-limit stand-in where its size and density are known), base, top, `f_sed`, the
/// τ its profile puts above 1 bar, and whether its base lies below the join; with the largest of
/// those below the join, modelled and stood in (the stop line's), and above it.
fn deep_deck_lines(name: &str, atmosphere: &EnvelopeAtmosphere) -> (Vec<String>, [f64; 3]) {
    let join = atmosphere.profile().join_pressure().value();
    let one_bar = COMPOSITION_LEVEL.value();
    let mut lines = Vec::new();
    // The largest τ above 1 bar: below the join modelled, below it stood in, and above it.
    let mut largest = [0.0_f64; 3];
    for (k, deck) in atmosphere.decks().iter().enumerate() {
        let kind = DeckKind::of(deck.condensate());
        if !matches!(kind, Some(DeckKind::Refractory | DeckKind::SulphideOrSalt)) {
            continue;
        }
        let below = deck.base().value() > join;
        let (base, top, f_sed) = (
            deck.base().value(),
            deck.top().value(),
            deck.sedimentation(),
        );
        assert!(
            base.is_finite() && top.is_finite() && f_sed.is_finite(),
            "{name}'s {}",
            key_of(deck.condensate())
        );
        let line = if let Some(tau) = atmosphere.deck_optical_depth(deck.condensate()) {
            let above = optical_depth_above(tau, base, top, f_sed, one_bar);
            assert!(tau.is_finite() && above.is_finite(), "{name}'s τ");
            let slot = if below { 0 } else { 2 };
            largest[slot] = largest[slot].max(above);
            format!("tau {tau:.4e}, above 1 bar {above:.4e}")
        } else {
            let reason = atmosphere
                .modes()
                .iter()
                .find(|m| m.deck() == deck.condensate())
                .and_then(|m| m.optical_depth().err())
                .map_or_else(|| "no mode".to_owned(), |e| e.to_string());
            match geometric_optical_depth(atmosphere, k, deck) {
                Some(tau) => {
                    let above = optical_depth_above(tau, base, top, f_sed, one_bar);
                    assert!(tau.is_finite() && above.is_finite(), "{name}'s stand-in τ");
                    let slot = if below { 1 } else { 2 };
                    largest[slot] = largest[slot].max(above);
                    format!(
                        "tau not modelled ({reason}); geometric stand-in {tau:.4e}, above 1 bar \
                         {above:.4e}"
                    )
                }
                None => format!("tau not modelled ({reason})"),
            }
        };
        let side = if below { "below" } else { "above" };
        lines.push(format!(
            "{name} {} ({side} the join): base {:.4e} bar, top {:.4e} bar, f_sed {f_sed}, {line}",
            key_of(deck.condensate()),
            base / 1e5,
            top / 1e5
        ));
    }
    (lines, largest)
}

/// Printed, not asserted beyond being finite (decision-deep-decks-split §5, §8.1): on the table's
/// Jupiter and Saturn and Sudarsky's giants at `T_eff` 900, 1,200 and 1,500 K, each refractory or
/// salt deck below the join (and, beside them, those above it), its τ(550), base, top, `f_sed` and
/// the τ its profile puts above 1 bar, at 23's densities and at P14.T49.n's read ones (24's). The read ones give the liquid refractories
/// a density but no optics (no index file is a liquid's), so their τ is not modelled even then; where
/// a size and a density are known, a geometric-limit stand-in is printed beside it.
/// The stop line: a deck that puts τ 1 or more above 1 bar clouds the visible column, which main
/// rules on (eq. 5 by per-deck K).
#[test]
fn the_deep_decks_put_little_above_one_bar() {
    let cases = || {
        [
            ("Jupiter", table_case("Jupiter")),
            ("Saturn", table_case("Saturn")),
            ("Sudarsky 900 K", sudarsky(900.0)),
            ("Sudarsky 1,200 K", sudarsky(1_200.0)),
            ("Sudarsky 1,500 K", sudarsky(1_500.0)),
        ]
    };
    let print = |rules: &str| {
        let mut largest = [0.0_f64; 3];
        for (name, case) in cases() {
            let (lines, deck_largest) = deep_deck_lines(name, &case.atmosphere());
            for line in lines {
                eprintln!("deep decks ({rules}): {line}");
            }
            for (slot, value) in largest.iter_mut().zip(deck_largest) {
                *slot = slot.max(value);
            }
        }
        eprintln!(
            "deep decks ({rules}): the largest τ above 1 bar below the join {:.4e} modelled, \
             {:.4e} by a geometric stand-in; above the join {:.4e}",
            largest[0], largest[1], largest[2]
        );
        largest[0].max(largest[1]).max(largest[2])
    };
    let held = print("23's densities");
    let read =
        crate::substance::with_read_densities(crate::substance::ReadDensitiesRules::Read, || {
            print("24's read densities")
        });
    assert!(held.is_finite() && read.is_finite());
}

/// The golden's envelopes, by name: the Solar System table's four giants, the KELT-9b-like dayside
/// and the 8 M⊕ sub-Neptune.
fn golden_cases() -> Vec<(&'static str, Case)> {
    vec![
        ("jupiter", table_case("Jupiter")),
        ("saturn", table_case("Saturn")),
        ("uranus", table_case("Uranus")),
        ("neptune", table_case("Neptune")),
        ("kelt_9b", kelt_9b()),
        ("sub_neptune", sub_neptune(8.0, 0.03)),
    ]
}

/// The visible atmospheres of the Solar System table's four giants, a KELT-9b-like dayside and an
/// 8 M⊕ sub-Neptune, pinned: each one's inputs, its composition at 1 bar, mean molar mass and
/// `c_p` ÷ R, every deck, every mode (its size, column, τ or their labels, profile and coverage),
/// every absorber, its hazes' τ, and the rows not formed and not modelled. Floats are their bits
/// with Rust's shortest round-trip decimal beside.
#[test]
fn the_envelope_atmosphere_is_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    w.line(
        "# P14.T52.d's visible atmospheres (hyperion_sim::planetary::hooks::envelope_atmosphere):",
    );
    w.line("# the Solar System table's four giants (protosolar host, this module's T_int, adopted grey");
    w.line("# profile, starlit), a KELT-9b-like dayside (f 1/2) and an 8 M_E sub-Neptune. Per body: its");
    w.line(
        "# inputs; its composition at 1 bar by mole fraction, largest first; mu (g/mol), c_p/R;",
    );
    w.line("# each deck (kind, bound, phase, base Pa, base K, top Pa, f_sed, column kg/m2, vapour");
    w.line("# below and above); each mode; each absorber (molecules/m2, slab Pa); the hazes' tau(550);");
    w.line("# the rows not formed and not modelled. Pure until P14.T54.b; nothing on the record reads it.");
    for (name, case) in golden_cases() {
        pin_atmosphere(&mut w, name, &case);
    }
    golden!("planetary/envelope_atmosphere", w.as_str());
}

/// Pins the case `case` under the label `n` (the golden's header lists the columns).
fn pin_atmosphere(w: &mut hyperion_testkit::golden::GoldenWriter, n: &str, case: &Case) {
    let atmosphere = case.atmosphere();
    w.f64(&format!("{n}.internal_temperature_k"), case.internal);
    w.f64(&format!("{n}.equilibrium_temperature_k"), case.equilibrium);
    w.f64(&format!("{n}.gravity_m_s2"), case.gravity);
    w.f64(&format!("{n}.redistribution"), case.grey.redistribution());
    w.f64(&format!("{n}.enrichment"), case.abundances.enrichment());
    for (k, &(id, x)) in atmosphere.composition().iter().enumerate() {
        w.f64(&format!("{n}.composition[{k:02}].{}", key_of(id)), x);
    }
    w.f64(
        &format!("{n}.mean_molar_mass_g_per_mol"),
        atmosphere.mean_molar_mass_g_per_mol(),
    );
    w.f64(
        &format!("{n}.heat_capacity_over_r"),
        atmosphere.heat_capacity_over_r(),
    );
    pin_decks(w, n, &atmosphere);
    for (k, mode) in atmosphere.modes().iter().enumerate() {
        pin_mode(w, &format!("{n}.mode[{k:02}]"), mode);
    }
    for absorber in atmosphere.absorbers() {
        let tag = format!("{n}.absorber.{}", key_of(absorber.species()));
        w.f64(&format!("{tag}.column_per_m2"), absorber.column_per_m2());
        if let AbsorberLayer::Slab { base, top } = absorber.layer() {
            w.f64(&format!("{tag}.base_pa"), base.value());
            w.f64(&format!("{tag}.top_pa"), top.value());
        }
    }
    w.f64(
        &format!("{n}.haze_optical_depth"),
        atmosphere.haze_optical_depth(),
    );
    for (id, why) in atmosphere.not_formed() {
        w.line(&format!("{n}.not_formed.{} = {why:?}", key_of(*id)));
    }
    for id in atmosphere.not_modelled() {
        w.line(&format!("{n}.not_modelled.{}", key_of(*id)));
    }
}

/// Pins `atmosphere`'s decks under the label `n`.
fn pin_decks(
    w: &mut hyperion_testkit::golden::GoldenWriter,
    n: &str,
    atmosphere: &EnvelopeAtmosphere,
) {
    for (k, deck) in atmosphere.decks().iter().enumerate() {
        let tag = format!("{n}.deck[{k:02}].{}", key_of(deck.condensate()));
        w.line(&format!(
            "{tag} = {:?} {:?} {:?} extrapolated {}",
            DeckKind::of(deck.condensate()),
            atmosphere.deck_bounds()[k],
            deck.phase(),
            deck.extrapolated()
        ));
        w.f64(&format!("{tag}.base_pa"), deck.base().value());
        w.f64(&format!("{tag}.base_k"), deck.base_temperature().value());
        w.f64(&format!("{tag}.top_pa"), deck.top().value());
        w.f64(&format!("{tag}.f_sed"), deck.sedimentation());
        w.f64(
            &format!("{tag}.column_kg_m2"),
            atmosphere.deck_column_masses()[k].value(),
        );
        w.f64(&format!("{tag}.vapour_below"), deck.vapour_below());
        w.f64(&format!("{tag}.vapour_above"), deck.vapour_above());
    }
}

/// Pins the mode `mode` under the label `tag`.
fn pin_mode(w: &mut hyperion_testkit::golden::GoldenWriter, tag: &str, mode: &AerosolMode) {
    w.line(&format!(
        "{tag} = {} {:?} {} of deck {}",
        key_of(mode.material()),
        mode.phase(),
        mode.shape().name(),
        key_of(mode.deck())
    ));
    match mode.size() {
        Ok(ParticleSize::Distribution(size)) => {
            w.line(&format!("{tag}.size = {}", size.kind().name()));
            w.f64(&format!("{tag}.r_eff_m"), size.effective_radius().value());
            w.f64(&format!("{tag}.v_eff"), size.effective_variance());
        }
        Ok(ParticleSize::Aggregate(a)) => {
            w.f64(
                &format!("{tag}.monomer_radius_m"),
                a.monomer_radius().value(),
            );
            w.f64(&format!("{tag}.monomers"), a.monomers());
        }
        Err(error) => w.line(&format!("{tag}.size: {error}")),
    }
    match mode.column_mass() {
        Some(mass) => w.f64(&format!("{tag}.column_kg_m2"), mass.value()),
        None => w.line(&format!("{tag}.column: not modelled")),
    }
    match mode.optical_depth() {
        Ok(depth) => {
            w.line(&format!("{tag}.tau_basis = {:?}", depth.basis()));
            w.f64(&format!("{tag}.tau"), depth.value());
        }
        Err(error) => w.line(&format!("{tag}.tau: {error}")),
    }
    w.f64(&format!("{tag}.base_pa"), mode.profile().base().value());
    w.f64(&format!("{tag}.top_pa"), mode.profile().top().value());
    w.f64(&format!("{tag}.exponent"), mode.profile().exponent());
    w.f64(&format!("{tag}.coverage"), mode.coverage());
}
