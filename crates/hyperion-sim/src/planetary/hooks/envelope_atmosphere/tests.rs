//! P14.T24.d's internal temperature and profile on the Solar System table's giants: the cooling
//! law's anchors, its blend with the record's heat, and κ₁'s fit on Jupiter.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::HostAbundances;
use crate::planetary::chem::envelope_abundances::{EnvelopeAbundances, EnvelopeInputs};
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::solar::{PLANETS, SOLAR_AGE, found, solar_disc, solar_system};
use crate::planetary::hooks::structure::envelope::{
    EnvelopeProfile, EnvelopeProfileInputs, GreyProfile, MEAN_REDISTRIBUTION, OPACITY_RATIO,
    OPTICAL_DEPTH_EXPONENT, THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG, irradiation_temperature,
};
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
    let law = |m: f64| cooling_law_temperature(EarthMasses::new(m), SOLAR_AGE).value();
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
        envelope_internal_temperature(luminosity, radius, mass, SOLAR_AGE).value()
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
/// 0 (and below) it is a 1 Myr envelope's, and at 13.8 Gyr cooler than at 4.57; a mass, age,
/// luminosity or radius out of its range gives NaN, and a luminosity of −0 the same bits as +0.
#[test]
fn the_internal_temperature_is_positive_at_every_age() {
    for m in [0.5, 2.0, 10.0, 95.161, 131.6] {
        let mass = EarthMasses::new(m);
        let zero = cooling_law_temperature(mass, Years::new(0.0)).value();
        let young = cooling_law_temperature(mass, Years::new(1.0e6)).value();
        let old = cooling_law_temperature(mass, Years::new(13.8e9)).value();
        let now = cooling_law_temperature(mass, SOLAR_AGE).value();
        assert_same_bits(zero, young);
        assert!(
            old.is_finite() && old > 0.0 && old < now && now < young,
            "{m} M⊕"
        );
    }
    assert!(
        cooling_law_temperature(EarthMasses::new(0.0), SOLAR_AGE)
            .value()
            .is_nan()
    );
    let saturn = COOLING_LAW_MASS;
    assert!(
        cooling_law_temperature(saturn, Years::new(f64::NAN))
            .value()
            .is_nan()
    );
    assert!(
        cooling_law_temperature(saturn, Years::new(f64::INFINITY))
            .value()
            .is_nan()
    );
    assert_same_bits(
        cooling_law_temperature(saturn, Years::new(-1.0)).value(),
        cooling_law_temperature(saturn, MIN_AGE).value(),
    );
    let radius = Metres::new(5.8e7);
    for (luminosity, r) in [
        (f64::NAN, 5.8e7),
        (-1.0, 5.8e7),
        (1e17, 0.0),
        (1e17, f64::NAN),
    ] {
        let t = envelope_internal_temperature(
            Watts::new(luminosity),
            Metres::new(r),
            saturn,
            SOLAR_AGE,
        );
        assert!(t.value().is_nan(), "L {luminosity} W, R {r} m: {t:?}");
    }
    let zero = envelope_internal_temperature(Watts::new(-0.0), radius, saturn, SOLAR_AGE);
    assert_same_bits(
        zero.value(),
        envelope_internal_temperature(Watts::new(0.0), radius, saturn, SOLAR_AGE).value(),
    );
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
/// within its measured value's −5 K to +40%, as printed.
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
    let t_int = envelope_internal_temperature(Watts::new(0.0), Metres::new(1.4e7), mass, SOLAR_AGE);
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
    );
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
