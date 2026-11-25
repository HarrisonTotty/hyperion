//! The vent gas's tests: its species and reactions, its anchors against the sources, its
//! continuity and monotony, its element budget, its inputs and refusals, and its golden.

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

use super::*;
use crate::GENERATOR_VERSION;
use crate::planetary::chem::redox::fmq_log_fugacity;
use crate::planetary::derive::solar::{historic, historic_table};
use crate::planetary::hooks::crust::ULTRAMAFIC_ERUPTION_TEMPERATURE;
use crate::substance::formation_range;

/// A gas of Earth's carriers' proportions, the table's exchangeable H/C of about 18 rounded, with
/// nitrogen at a tenth of the carbon and sulphur at `sulphur` of it, mol of atoms.
fn earthlike(sulphur: f64) -> VentElements {
    VentElements::new(18.0, 1.0, 0.1, sulphur)
}

/// The vent's inputs at ΔIW `delta_iw`, `t_k` kelvin and `p_bar` bar.
fn at(delta_iw: f64, t_k: f64, p_bar: f64) -> VentInputs {
    VentInputs {
        delta_iw: Dex::new(delta_iw),
        temperature: Kelvin::new(t_k),
        pressure: Pascals::new(p_bar * PASCALS_PER_BAR),
    }
}

/// The gas of `elements` at ΔIW `delta_iw`, `t_k` kelvin and `p_bar` bar.
fn gas(delta_iw: f64, t_k: f64, p_bar: f64, elements: &VentElements) -> VentGas {
    VentGas::speciate(&at(delta_iw, t_k, p_bar), elements).unwrap()
}

/// The share of the carbon that `species` carries.
fn carbon_share(gas: &VentGas, species: VentSpecies) -> f64 {
    gas.moles(species) / gas.budget().carbon_mol()
}

/// FMQ's offset above IW at `t_k`, log₁₀ units (O'Neill 1987; 1988).
fn fmq_above_iw_at(t_k: f64) -> f64 {
    let t = Kelvin::new(t_k);
    (fmq_log_fugacity(t) - iw_log_fugacity(t)).value()
}

/// |log₁₀(a ÷ b)|.
fn dex_apart(a: f64, b: f64) -> f64 {
    math::log10(a / b).abs()
}

/// Every species is a registry row, in registry order, with a record over the vents' range, and
/// every reaction balances.
#[test]
fn every_species_is_a_registry_row_in_registry_order_with_a_record() {
    let ids = VentSpecies::ALL.map(VentSpecies::substance);
    for (species, id) in VentSpecies::ALL.iter().zip(ids) {
        assert_eq!(id.substance().key().as_str(), species.key());
        assert_eq!(crate::substance::by_key(species.key()), Some(id));
        let (low, high) = formation_range(id).expect("every vent species has a record");
        assert!(
            low.value() <= 300.0 && high.value() >= 6_000.0,
            "{species:?}"
        );
        assert_eq!(VentSpecies::ALL[species.index()], *species);
    }
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "registry order");
    assert_same_bits(VentSpecies::Water.atoms(Element::H), 2.0);
    assert_same_bits(VentSpecies::CarbonDioxide.atoms(Element::O), 2.0);
    assert_same_bits(VentSpecies::Methane.atoms(Element::O), 0.0);
    // Each reaction balances, and has its constant at both ends of the range.
    for t_k in [300.0, 6_000.0] {
        let t = Kelvin::new(t_k);
        let constants = [
            log10_k(&WATER_FORMATION, t),
            log10_k(&CARBON_DIOXIDE_FORMATION, t),
            log10_k(&METHANATION, t),
            log10_k(&SULPHUR_OXIDATION, t),
        ];
        for (i, k) in constants.iter().enumerate() {
            assert!(
                k.is_some_and(f64::is_finite),
                "reaction {i} at {t_k} K: {k:?}"
            );
        }
    }
}

/// The melt sets the oxygen: log f(O₂) = IW(T) + ΔIW, and the gas holds O₂ at that fugacity.
#[test]
fn the_melt_sets_the_oxygen_fugacity() {
    for (delta_iw, t_k) in [(-4.0, 1_200.0), (0.01, 1_473.15), (3.6, 1_613.15)] {
        let g = gas(delta_iw, t_k, 1.0, &earthlike(0.1));
        let expected = iw_log_fugacity(Kelvin::new(t_k)) + Dex::new(delta_iw);
        assert_same_bits(g.log10_oxygen_fugacity().value(), expected.value());
        let o2_bar = g.partial_pressure(VentSpecies::Oxygen).value() / PASCALS_PER_BAR;
        assert!(
            (o2_bar / math::exp10(expected.value()) - 1.0).abs() < 1e-12,
            "{delta_iw}, {t_k}: {o2_bar}"
        );
    }
}

/// Earth's upper mantle (IW+3.6) at 1 bar and 1,473.15 K is H₂O- and CO₂-led, with H₂O/H₂ ≈ 52
/// and CO₂/CO ≈ 21 (the research's estimates, 50 and 20). Erta Ale's basaltic gas (de Moor et al.
/// 2013, Geochem. Geophys. Geosyst. 14, 4076, Table 5, Giggenbach and Le Guern's 1974 samples at
/// 1,125–1,135 °C and ΔQFM −0.17, read) has H₂O/H₂ ≈ 53, CO₂/CO 22.6 and SO₂/H₂S ≈ 7.6; at its
/// 1,130 °C and ΔQFM the model gives 40, 18.4 and 11.9: the first two within 0.15 dex, SO₂/H₂S
/// within 0.25 (findings).
#[test]
fn earth_s_mantle_outgasses_water_and_carbon_dioxide_as_erta_ale_does() {
    let earth = gas(3.6, 1_473.15, 1.0, &earthlike(0.0));
    assert!((45.0..60.0).contains(&earth.water_to_hydrogen()));
    assert!((18.0..25.0).contains(&earth.carbon_dioxide_to_monoxide()));
    assert!(earth.fraction(VentSpecies::Water) > 0.8);
    assert!(carbon_share(&earth, VentSpecies::CarbonDioxide) > 0.9);

    let t_k = 1_130.0 + 273.15;
    let erta_ale = gas(fmq_above_iw_at(t_k) - 0.17, t_k, 1.0, &earthlike(0.1));
    assert!(dex_apart(erta_ale.water_to_hydrogen(), 794.0 / 14.9) < 0.15);
    assert!(dex_apart(erta_ale.carbon_dioxide_to_monoxide(), 22.6) < 0.15);
    let sulphur = erta_ale.sulphur_dioxide_to_hydrogen_sulphide().unwrap();
    assert!(dex_apart(sulphur, 7.6) < 0.25, "{sulphur}");
}

/// Below IW, H₂ and CO lead (Ortenzi et al. 2020, Fig. 1, at 1 bar), and from IW−2 "almost all
/// degassed carbon takes the form of CO", over 95% of it (Guimond et al. 2021); above IW+1.5, H₂O
/// and CO₂ lead. At the default vent and at Ortenzi et al.'s 2,000 K degassing temperature, where
/// H₂O/H₂ reaches 1 at IW−0.03, so that H₂ leads there from IW−0.5.
#[test]
fn below_iw_the_carbon_is_carbon_monoxide() {
    let elements = earthlike(0.0);
    for t_k in [DEFAULT_VENT_TEMPERATURE.value(), 2_000.0] {
        for delta_iw in [-6.0, -4.0, -2.0, -1.0, -0.01] {
            let g = gas(delta_iw, t_k, 1.0, &elements);
            assert!(
                g.carbon_dioxide_to_monoxide() < 1.0,
                "{t_k} K, IW{delta_iw}"
            );
            if delta_iw <= -0.5 || t_k < 2_000.0 {
                assert!(g.water_to_hydrogen() < 1.0, "{t_k} K, IW{delta_iw}");
            }
            let (co, co2) = (
                carbon_share(&g, VentSpecies::CarbonMonoxide),
                carbon_share(&g, VentSpecies::CarbonDioxide),
            );
            assert!(co > 0.5, "{t_k} K, IW{delta_iw}: CO {co}");
            if delta_iw <= -2.0 {
                assert!(co2 < 0.05, "{t_k} K, IW{delta_iw}: CO₂ {co2}");
            }
            let deficit = g.budget().oxygen_against_carriers_mol();
            assert!(deficit < 0.0, "{t_k} K, IW{delta_iw}: {deficit}");
        }
        for delta_iw in [1.5, 2.0, 3.6, 5.0] {
            let g = gas(delta_iw, t_k, 1.0, &elements);
            assert!(
                g.carbon_dioxide_to_monoxide() > 1.0,
                "{t_k} K, IW+{delta_iw}"
            );
            assert!(g.water_to_hydrogen() > 1.0, "{t_k} K, IW+{delta_iw}");
        }
    }
}

/// Mars's model mantle, IW+0.01 (P14.T50.a), outgasses a CO-led gas at the default vent and
/// 1 bar: CO₂/CO ≈ 0.34, as the route ruling computed from the registry's CEA records
/// (research-leans-58), and H₂ over H₂O; against Mars's measured CO₂ air, which P14.T55.a.c's
/// route carries. The table's Mars, at its median ranks, is that mantle.
#[test]
fn mars_s_vent_at_iw_plus_0_01_is_carbon_monoxide_led() {
    let model = gas(0.01, DEFAULT_VENT_TEMPERATURE.value(), 1.0, &earthlike(0.0));
    let ratio = model.carbon_dioxide_to_monoxide();
    assert!((0.32..0.36).contains(&ratio), "{ratio}");
    assert!(model.water_to_hydrogen() < 1.0);

    let mars = historic("Mars");
    let delta_iw = mars.element_inventory().mantle_delta_iw();
    assert!((delta_iw.value() - 0.01).abs() < 0.05, "{delta_iw:?}");
    let vent = VentGas::speciate(
        &VentInputs {
            delta_iw,
            temperature: DEFAULT_VENT_TEMPERATURE,
            pressure: Pascals::new(PASCALS_PER_BAR),
        },
        &VentElements::of_exchangeable(mars.element_inventory()),
    )
    .unwrap();
    assert!((0.30..0.38).contains(&vent.carbon_dioxide_to_monoxide()));
    assert!(vent.fraction(VentSpecies::CarbonMonoxide) > vent.fraction(VentSpecies::CarbonDioxide));
    assert!(vent.fraction(VentSpecies::Hydrogen) > vent.fraction(VentSpecies::Water));
}

/// Methane needs a reduced mantle, a cool vent and over about 10 bar (Tian and Heng 2024; their
/// T ≲ 1,000 K and f(O₂) ≲ IW): at IW−2, 1,000 K and 10 bar it carries most of the carbon, and
/// moving any one of the three to Earth's mantle, the default vent or 1 bar leaves it a minority.
/// At the default vent and 1 bar it is a trace at every ΔIW from −4.
#[test]
fn methane_needs_a_reduced_mantle_a_cool_vent_and_ten_bar() {
    let elements = earthlike(0.0);
    let methane = |delta_iw: f64, t_k: f64, p_bar: f64| {
        carbon_share(&gas(delta_iw, t_k, p_bar, &elements), VentSpecies::Methane)
    };
    let all_three = methane(-2.0, 1_000.0, 10.0);
    assert!(all_three > 0.5, "{all_three}");
    assert!(methane(3.6, 1_000.0, 10.0) < 0.05);
    assert!(methane(-2.0, DEFAULT_VENT_TEMPERATURE.value(), 10.0) < 0.05);
    assert!(methane(-2.0, 1_000.0, 1.0) < 0.5);
    for delta_iw in [-4.0, -2.0, 0.0, 2.0, 3.6] {
        let share = methane(delta_iw, DEFAULT_VENT_TEMPERATURE.value(), 1.0);
        assert!(share < 1e-3, "IW{delta_iw:+}: {share}");
    }
}

/// Sulphur degasses as SO₂ at 1 bar and as H₂S above a few tens of bar (Gaillard, Scaillet and
/// Arndt 2011) from Earth's mantle at the default vent, SO₂/H₂S falling as 1 ÷ p(H₂O); a mantle
/// at IW gives H₂S at 1 bar.
#[test]
fn sulphur_is_sulphur_dioxide_at_one_bar_and_hydrogen_sulphide_at_depth() {
    let elements = earthlike(0.01);
    let t_k = DEFAULT_VENT_TEMPERATURE.value();
    let ratio = |delta_iw: f64, p_bar: f64| {
        gas(delta_iw, t_k, p_bar, &elements)
            .sulphur_dioxide_to_hydrogen_sulphide()
            .unwrap()
    };
    assert!(ratio(3.6, 1.0) > 1.0);
    assert!(ratio(3.6, 10.0) > 1.0);
    assert!(ratio(3.6, 100.0) < 1.0);
    let (one, ten) = (
        gas(3.6, t_k, 1.0, &elements),
        gas(3.6, t_k, 10.0, &elements),
    );
    let law = ratio(3.6, 1.0)
        / ratio(3.6, 10.0)
        / (10.0 * ten.fraction(VentSpecies::Water) / one.fraction(VentSpecies::Water));
    assert!((law - 1.0).abs() < 1e-9, "{law}");
    assert!(ratio(0.0, 1.0) < 1.0);
}

/// The direction in which a ratio moves with an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trend {
    Rises,
    Falls,
    Flat,
}

/// The four ratios of `g`, in the order H₂O/H₂, CO₂/CO, CH₄/CO, SO₂/H₂S.
fn ratios(g: &VentGas) -> [f64; 4] {
    [
        g.water_to_hydrogen(),
        g.carbon_dioxide_to_monoxide(),
        g.methane_to_monoxide(),
        g.sulphur_dioxide_to_hydrogen_sulphide().unwrap(),
    ]
}

/// Each ratio moves along `points` as `trends` say, strictly where it moves and to the bit where
/// it is flat, and every share is finite and at least 0.
fn assert_trends(name: &str, points: &[VentGas], trends: [Trend; 4]) {
    for g in points {
        for species in VentSpecies::ALL {
            let x = g.fraction(species);
            assert!(x.is_finite() && x >= 0.0, "{name}: {species:?} {x}");
        }
    }
    for pair in points.windows(2) {
        let (a, b) = (ratios(&pair[0]), ratios(&pair[1]));
        for (i, trend) in trends.iter().enumerate() {
            match trend {
                Trend::Rises => assert!(b[i] > a[i], "{name}: ratio {i} {} → {}", a[i], b[i]),
                Trend::Falls => assert!(b[i] < a[i], "{name}: ratio {i} {} → {}", a[i], b[i]),
                Trend::Flat => assert_same_bits(b[i], a[i]),
            }
        }
    }
}

/// Each ratio is continuous and monotone in ΔIW, T and p: H₂O/H₂ rises with ΔIW and T, CO₂/CO
/// rises with ΔIW and falls with T, neither moving with p; CH₄/CO falls with ΔIW and T and rises
/// with p; SO₂/H₂S rises with ΔIW and T and falls with p. A relative step of 10⁻⁹ in any input
/// moves no ratio or share by over 10⁻⁷ relative.
#[test]
fn each_ratio_is_continuous_and_monotone_in_delta_iw_temperature_and_pressure() {
    use Trend::{Falls, Flat, Rises};
    let elements = earthlike(0.05);
    let t_vent = DEFAULT_VENT_TEMPERATURE.value();
    for (t_k, p_bar) in [(1_000.0, 10.0), (t_vent, 1.0), (1_800.0, 0.01)] {
        let points: Vec<VentGas> = (-24..=24)
            .map(|i| gas(f64::from(i) * 0.25, t_k, p_bar, &elements))
            .collect();
        assert_trends(
            &format!("ΔIW at {t_k} K"),
            &points,
            [Rises, Rises, Falls, Rises],
        );
    }
    for (delta_iw, p_bar) in [(-4.0, 10.0), (0.0, 1.0), (3.6, 1.0)] {
        let points: Vec<VentGas> = (0..=44)
            .map(|i| gas(delta_iw, 900.0 + 25.0 * f64::from(i), p_bar, &elements))
            .collect();
        assert_trends(
            &format!("T at IW{delta_iw:+}"),
            &points,
            [Rises, Falls, Falls, Rises],
        );
    }
    for (delta_iw, t_k) in [(-4.0, 1_000.0), (0.0, t_vent), (3.6, t_vent)] {
        let points: Vec<VentGas> = (-12..=12)
            .map(|i| gas(delta_iw, t_k, math::exp10(0.25 * f64::from(i)), &elements))
            .collect();
        assert_trends(
            &format!("p at IW{delta_iw:+}"),
            &points,
            [Flat, Flat, Rises, Falls],
        );
    }

    let close = |a: f64, b: f64| (a - b).abs() <= 1e-7 * a.abs().max(b.abs()).max(1e-300);
    let nudge = 1.0 + 1e-9;
    for (delta_iw, t_k, p_bar) in [
        (-3.0, 1_000.0, 30.0),
        (0.01, t_vent, 1.0),
        (3.6, 1_613.15, 0.1),
    ] {
        let base = gas(delta_iw, t_k, p_bar, &elements);
        for nudged in [
            gas(delta_iw * nudge, t_k, p_bar, &elements),
            gas(delta_iw, t_k * nudge, p_bar, &elements),
            gas(delta_iw, t_k, p_bar * nudge, &elements),
        ] {
            for (a, b) in ratios(&base).iter().zip(ratios(&nudged)) {
                assert!(
                    close(*a, b),
                    "IW{delta_iw:+} {t_k} K {p_bar} bar: {a} → {b}"
                );
            }
            for species in VentSpecies::ALL {
                let (a, b) = (base.fraction(species), nudged.fraction(species));
                assert!(
                    close(a, b) || (a - b).abs() < 1e-15,
                    "{species:?}: {a} → {b}"
                );
            }
        }
    }
}

/// |a ÷ b − 1|, or |a| where b is 0.
fn relative(a: f64, b: f64) -> f64 {
    if b > 0.0 {
        (a / b - 1.0).abs()
    } else {
        a.abs()
    }
}

/// `vent`, the gas of `elements` at `inputs`, closes its elements to 10⁻¹² relative, its shares
/// sum to 1 and its partial pressures to the vent's, and it is in equilibrium: CH₄ by CO + 3 H₂ ⇌
/// CH₄ + H₂O and SO₂ by 2 H₂S + 3 O₂ ⇌ 2 SO₂ + 2 H₂O, each within 10⁻⁹ in log₁₀ K, where none of
/// their species is below 10⁻²⁰⁰ of the gas.
fn assert_closed_and_in_equilibrium(vent: &VentGas, inputs: &VentInputs, elements: &VentElements) {
    let label = format!("{elements:?} {inputs:?}");
    let budget = vent.budget();
    for (got, given) in [
        (budget.hydrogen_mol(), elements.hydrogen_mol()),
        (budget.carbon_mol(), elements.carbon_mol()),
        (budget.nitrogen_mol(), elements.nitrogen_mol()),
        (budget.sulphur_mol(), elements.sulphur_mol()),
    ] {
        assert!(
            relative(got, given) < 1e-12,
            "{label}: {got} against {given}"
        );
    }
    let shares = VentSpecies::ALL
        .iter()
        .fold(0.0, |s, &x| s + vent.fraction(x));
    assert!((shares - 1.0).abs() < 1e-12, "{label}: {shares}");
    let partials = VentSpecies::ALL
        .iter()
        .fold(0.0, |s, &x| s + vent.partial_pressure(x).value());
    assert!(
        relative(partials, inputs.pressure.value()) < 1e-12,
        "{label}"
    );
    assert_same_bits(
        budget.oxygen_against_carriers_mol(),
        budget.oxygen_mol() - (0.5 * budget.hydrogen_mol() + 2.0 * budget.carbon_mol()),
    );
    let log_p = |x: VentSpecies| math::log10(vent.partial_pressure(x).value() / PASCALS_PER_BAR);
    let reactions: [&[(VentSpecies, i32)]; 2] = [&METHANATION, &SULPHUR_OXIDATION];
    for reaction in reactions {
        if reaction.iter().any(|&(x, _)| vent.fraction(x) < 1e-200) {
            continue;
        }
        let terms: Vec<(SubstanceId, i32)> = reaction
            .iter()
            .map(|&(x, nu)| (x.substance(), nu))
            .collect();
        let k = Reaction::new(&terms)
            .unwrap()
            .log10_equilibrium_constant(inputs.temperature)
            .unwrap();
        let q = reaction
            .iter()
            .fold(0.0, |sum, &(x, nu)| sum + f64::from(nu) * log_p(x));
        assert!(
            (q - k).abs() < 1e-9,
            "{label}: {reaction:?} {q} against {k}"
        );
    }
}

/// The element budget closes, the shares and pressures sum and the gas is in equilibrium
/// ([`assert_closed_and_in_equilibrium`]) over a grid of mantles, vents and compositions, every
/// refusal being a vent whose f(O₂) reaches its pressure.
#[test]
fn the_element_budget_closes() {
    let compositions = [
        earthlike(0.1),
        VentElements::new(1.0, 2.0, 0.05, 0.3),
        VentElements::new(1.0e-6, 1.0, 0.0, 0.01),
        VentElements::new(5.0, 0.0, 0.0, 0.0),
        VentElements::new(0.0, 1.0, 0.0, 0.2),
        VentElements::new(0.0, 0.0, 1.0, 0.0),
        VentElements::new(0.0, 0.0, 0.0, 1.0),
    ];
    let mut solved = 0_u32;
    for elements in &compositions {
        for delta_iw in [-6.0, -3.0, 0.0, 3.6, 6.0] {
            for t_k in [900.0, 1_473.15, 2_000.0] {
                for p_bar in [1e-4, 1.0, 100.0, 1_000.0] {
                    let inputs = at(delta_iw, t_k, p_bar);
                    match VentGas::speciate(&inputs, elements) {
                        Ok(vent) => {
                            solved += 1;
                            assert_closed_and_in_equilibrium(&vent, &inputs, elements);
                        }
                        Err(SpeciateVentGasError::OxygenFugacity { fugacity_bar, .. }) => {
                            assert!(fugacity_bar >= p_bar);
                        }
                        Err(e) => panic!("{inputs:?}: {e}"),
                    }
                }
            }
        }
    }
    assert!(solved > 300, "{solved}");
}

/// A vent short of an element: with no hydrogen its sulphur is all SO₂ and no ratio of it to H₂S
/// exists; with hydrogen alone it is H₂ and H₂O by the buffer; with nitrogen alone, N₂ and the
/// melt's O₂.
#[test]
fn a_vent_short_of_an_element_speciates_what_it_has() {
    let t_k = DEFAULT_VENT_TEMPERATURE.value();
    let dry = gas(0.0, t_k, 1.0, &VentElements::new(0.0, 1.0, 0.0, 0.5));
    assert_same_bits(dry.moles(VentSpecies::SulphurDioxide), 0.5);
    assert_eq!(dry.sulphur_dioxide_to_hydrogen_sulphide(), None);
    assert_same_bits(dry.methane_to_monoxide(), 0.0);
    for species in [
        VentSpecies::Hydrogen,
        VentSpecies::Water,
        VentSpecies::Methane,
    ] {
        assert_same_bits(dry.fraction(species), 0.0);
    }

    let wet = gas(0.0, t_k, 1.0, &VentElements::new(2.0, 0.0, 0.0, 0.0));
    let split = wet.moles(VentSpecies::Water) / wet.moles(VentSpecies::Hydrogen);
    assert!((split / wet.water_to_hydrogen() - 1.0).abs() < 1e-12);
    assert_same_bits(wet.fraction(VentSpecies::CarbonMonoxide), 0.0);

    let nitrogen = gas(0.0, t_k, 1.0, &VentElements::new(0.0, 0.0, 1.0, 0.0));
    let x = nitrogen.fraction(VentSpecies::Nitrogen) + nitrogen.fraction(VentSpecies::Oxygen);
    assert!((x - 1.0).abs() < 1e-15);
}

/// The graphite cap is `K_f(CO)` f(O₂)^½, rising with ΔIW: at 1 bar a carbon-rich gas at IW−6
/// passes it, so that graphite would saturate the vent, and at IW it does not.
#[test]
fn the_graphite_cap_is_the_formation_constant_of_carbon_monoxide() {
    let t = DEFAULT_VENT_TEMPERATURE;
    let elements = VentElements::new(0.1, 1.0, 0.0, 0.0);
    let k = log10_formation_constant(VentSpecies::CarbonMonoxide.substance(), t).unwrap();
    let (reduced, neutral) = (
        gas(-6.0, t.value(), 1.0, &elements),
        gas(0.0, t.value(), 1.0, &elements),
    );
    for g in [&reduced, &neutral] {
        assert_same_bits(g.log10_carbon_monoxide_formation_constant(), k);
        let cap = g.carbon_monoxide_at_graphite().value() / PASCALS_PER_BAR;
        let expected = math::exp10(k + 0.5 * g.log10_oxygen_fugacity().value());
        assert!((cap / expected - 1.0).abs() < 1e-12);
    }
    assert!(reduced.carbon_monoxide_at_graphite() < neutral.carbon_monoxide_at_graphite());
    let co = VentSpecies::CarbonMonoxide;
    assert!(reduced.partial_pressure(co) > reduced.carbon_monoxide_at_graphite());
    assert!(neutral.partial_pressure(co) < neutral.carbon_monoxide_at_graphite());
}

/// The exchangeable carriers' elements, by the element registry's weights: water's hydrogen,
/// carbon dioxide's carbon and the nitrogen, with no sulphur (P14.T50.a holds it at zero).
#[test]
fn the_exchangeable_elements_are_the_carriers() {
    let earth = historic("Earth");
    let carriers = earth.inventory();
    let elements = VentElements::of_exchangeable(earth.element_inventory());
    let weight = Element::standard_atomic_weight_g_per_mol;
    let (w_h, w_c, w_n, w_o) = (
        weight(Element::H),
        weight(Element::C),
        weight(Element::N),
        weight(Element::O),
    );
    let close = |a: f64, b: f64| (a / b - 1.0).abs() < 1e-12;
    assert!(close(
        elements.hydrogen_mol(),
        2.0 * carriers.water().value() * 1e3 / (2.0 * w_h + w_o)
    ));
    assert!(close(
        elements.carbon_mol(),
        carriers.carbon_dioxide().value() * 1e3 / (w_c + 2.0 * w_o)
    ));
    assert!(close(
        elements.nitrogen_mol(),
        carriers.nitrogen().value() * 1e3 / w_n
    ));
    assert_same_bits(elements.sulphur_mol(), 0.0);
    assert_eq!(
        VentElements::new(-1.0, f64::NAN, f64::INFINITY, 2.0),
        VentElements::new(0.0, 0.0, 0.0, 2.0)
    );
}

/// The vent's temperature is its lid's eruption temperature, but an ice shell's and a flotation
/// crust's, which are the mantle's at the default, 1,473.15 K: every vent lies from a basalt's to
/// an ultramafic lava's, 1,613.15 K.
#[test]
fn the_vent_temperature_is_the_lid_s_eruption_temperature() {
    assert_same_bits(DEFAULT_VENT_TEMPERATURE.value(), 1_473.15);
    let not_lava = [
        CrustLithology::Water,
        CrustLithology::Graphite,
        CrustLithology::Feldspathic,
    ];
    for lid in CrustLithology::ALL {
        let expected = if not_lava.contains(&lid) {
            DEFAULT_VENT_TEMPERATURE
        } else {
            lid.eruption_temperature()
        };
        assert_eq!(vent_temperature(lid), expected, "{lid:?}");
        let t = vent_temperature(lid);
        assert!(
            DEFAULT_VENT_TEMPERATURE <= t && t <= ULTRAMAFIC_ERUPTION_TEMPERATURE,
            "{lid:?}: {t:?}"
        );
    }
    assert_eq!(
        vent_temperature(CrustLithology::Komatiite),
        ULTRAMAFIC_ERUPTION_TEMPERATURE
    );
}

/// Inputs it cannot speciate are refused, each with its own variant, first in the documented
/// order.
#[test]
fn inputs_it_cannot_speciate_are_refused() {
    let elements = earthlike(0.1);
    let refused = |inputs: VentInputs, elements: &VentElements| {
        VentGas::speciate(&inputs, elements).unwrap_err()
    };
    let t = DEFAULT_VENT_TEMPERATURE.value();
    assert!(matches!(
        refused(at(f64::NAN, t, 1.0), &elements),
        SpeciateVentGasError::DeltaIw(x) if x.is_nan()
    ));
    for t_k in [100.0, 7_000.0, f64::INFINITY] {
        assert_eq!(
            refused(at(0.0, t_k, 1.0), &elements),
            SpeciateVentGasError::Temperature(t_k)
        );
    }
    for p_bar in [0.0, -1.0, f64::INFINITY] {
        assert_eq!(
            refused(at(0.0, t, p_bar), &elements),
            SpeciateVentGasError::Pressure(p_bar * PASCALS_PER_BAR)
        );
    }
    assert!(matches!(
        refused(at(10.0, t, 1e-3), &elements),
        SpeciateVentGasError::OxygenFugacity { fugacity_bar, pressure_bar }
            if fugacity_bar >= pressure_bar
    ));
    assert_eq!(
        refused(at(0.0, t, 1.0), &VentElements::default()),
        SpeciateVentGasError::NoVolatiles
    );
    assert_eq!(
        refused(at(-1e4, t, 1.0), &elements),
        SpeciateVentGasError::Overflow
    );
    // Two faults at once: the first in the documented order is the one named.
    let none = VentElements::default();
    assert!(matches!(
        refused(at(f64::NAN, 100.0, 0.0), &none),
        SpeciateVentGasError::DeltaIw(_)
    ));
    assert_eq!(
        refused(at(0.0, 100.0, 0.0), &none),
        SpeciateVentGasError::Temperature(100.0)
    );
    assert_eq!(
        refused(at(0.0, t, 0.0), &none),
        SpeciateVentGasError::Pressure(0.0)
    );
    assert!(matches!(
        refused(at(10.0, t, 1e-3), &none),
        SpeciateVentGasError::OxygenFugacity { .. }
    ));
    assert_eq!(
        refused(at(-1e4, t, 1.0), &none),
        SpeciateVentGasError::NoVolatiles
    );
    assert_eq!(
        SpeciateVentGasError::Overflow.to_string(),
        "the vent's gas overflows the floats"
    );
    assert_eq!(
        SpeciateVentGasError::Pressure(-1.0).to_string(),
        "vent pressure -1 Pa is not finite and positive"
    );
    assert_eq!(
        SpeciateVentGasError::NoVolatiles.to_string(),
        "the vent is given no hydrogen, carbon, nitrogen or sulphur"
    );
}

/// Writes `g`'s figures under `label`: its oxygen, its graphite cap, its ratios, its shares and its
/// budget.
fn write_gas(w: &mut GoldenWriter, label: &str, g: &VentGas) {
    w.f64(
        &format!("{label} log10_fo2_bar"),
        g.log10_oxygen_fugacity().value(),
    );
    w.f64(
        &format!("{label} log10_kf_co"),
        g.log10_carbon_monoxide_formation_constant(),
    );
    w.f64(
        &format!("{label} co_at_graphite_pa"),
        g.carbon_monoxide_at_graphite().value(),
    );
    w.f64(&format!("{label} h2o_to_h2"), g.water_to_hydrogen());
    w.f64(
        &format!("{label} co2_to_co"),
        g.carbon_dioxide_to_monoxide(),
    );
    w.f64(&format!("{label} ch4_to_co"), g.methane_to_monoxide());
    match g.sulphur_dioxide_to_hydrogen_sulphide() {
        Some(r) => w.f64(&format!("{label} so2_to_h2s"), r),
        None => w.line(&format!("{label} so2_to_h2s: none")),
    }
    for species in VentSpecies::ALL {
        w.f64(&format!("{label} x {}", species.key()), g.fraction(species));
    }
    let b = g.budget();
    w.f64(&format!("{label} budget H mol"), b.hydrogen_mol());
    w.f64(&format!("{label} budget C mol"), b.carbon_mol());
    w.f64(&format!("{label} budget N mol"), b.nitrogen_mol());
    w.f64(&format!("{label} budget O mol"), b.oxygen_mol());
    w.f64(&format!("{label} budget S mol"), b.sulphur_mol());
    w.f64(
        &format!("{label} budget O against carriers mol"),
        b.oxygen_against_carriers_mol(),
    );
}

/// The Solar System's vents pinned by their bits (the sim-determinism skill's goldens), the same
/// twice; `just test-wasm-fast` runs it as `wasm32-wasip1`, its native–wasip1 twin.
///
/// - Each lid's vent temperature.
/// - Each body of the table, about the present Sun at its median ranks: its mantle's ΔIW and its
///   exchangeable elements, and its vent at the default temperature, at 1 bar and, where it has an
///   air, at its surface pressure.
/// - A sulphur-bearing gas of Earth's proportions at three mantles (IW−4, IW and IW+3.6), two
///   vents (1,200 and 1,613.15 K) and three pressures (0.01, 1 and 100 bar).
#[test]
fn the_solar_system_s_vents_are_pinned() {
    let write = || {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for lid in CrustLithology::ALL {
            w.f64(
                &format!("vent_temperature {}", lid.key()),
                vent_temperature(lid).value(),
            );
        }
        for (name, body) in historic_table() {
            let inventory = body.element_inventory();
            let elements = VentElements::of_exchangeable(inventory);
            let delta_iw = inventory.mantle_delta_iw();
            w.f64(&format!("{name} delta_iw"), delta_iw.value());
            w.f64(&format!("{name} H mol"), elements.hydrogen_mol());
            w.f64(&format!("{name} C mol"), elements.carbon_mol());
            w.f64(&format!("{name} N mol"), elements.nitrogen_mol());
            w.f64(&format!("{name} S mol"), elements.sulphur_mol());
            let surface = body
                .atmosphere()
                .surface_pressure()
                .filter(|p| p.value() > 0.0);
            let pressures = [
                ("1 bar", Some(Pascals::new(PASCALS_PER_BAR))),
                ("surface", surface),
            ];
            for (at_label, pressure) in pressures {
                let Some(pressure) = pressure else {
                    w.line(&format!("{name} {at_label}: no air"));
                    continue;
                };
                let inputs = VentInputs {
                    delta_iw,
                    temperature: DEFAULT_VENT_TEMPERATURE,
                    pressure,
                };
                let label = format!("{name} {at_label}");
                match VentGas::speciate(&inputs, &elements) {
                    Ok(g) => {
                        w.f64(&format!("{label} pressure_pa"), pressure.value());
                        write_gas(&mut w, &label, &g);
                    }
                    Err(e) => w.line(&format!("{label}: {e}")),
                }
            }
        }
        let elements = earthlike(0.1);
        for delta_iw in [-4.0, 0.0, 3.6] {
            for t_k in [1_200.0, 1_613.15] {
                for p_bar in [0.01, 1.0, 100.0] {
                    let label = format!("earthlike IW{delta_iw:+} {t_k} K {p_bar} bar");
                    write_gas(&mut w, &label, &gas(delta_iw, t_k, p_bar, &elements));
                }
            }
        }
        w
    };
    let first = write();
    assert_eq!(first.as_str(), write().as_str());
    golden!("planetary/outgassing", first.as_str());
}
