//! The heavy-secondary equilibrium's tests: Woitke et al.'s types, the low-temperature limit, the
//! mass balance, continuity, CO's onset, the hot branch, graphite, the species not carried, the
//! range and its refusals, the golden, and the cost.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::substance::{Reaction, formation_range};

/// Each species' atoms of H, C, N and O, in [`SPECIES`]' order.
const ATOMS: [[f64; 4]; SPECIES.len()] = [
    [2.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0],
    [2.0, 0.0, 0.0, 1.0],
    [4.0, 1.0, 0.0, 0.0],
    [3.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 2.0, 0.0],
    [0.0, 0.0, 0.0, 2.0],
    [0.0, 1.0, 0.0, 2.0],
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 1.0],
];

/// The elements' places in [`ATOMS`]' rows.
const H: usize = 0;
const C: usize = 1;
const N: usize = 2;
const O: usize = 3;

/// A budget of `h`, `c`, `n` and `o` moles.
fn air(h: f64, c: f64, n: f64, o: f64) -> AirElements {
    AirElements::new([
        (Element::H, h),
        (Element::C, c),
        (Element::N, n),
        (Element::O, o),
    ])
    .expect("a budget")
}

/// The tests' budgets in each type's interior, by name: type A with H₂ (its hydrogen past the
/// NH₃ its nitrogen takes) and with N₂ (its hydrogen short of it), type B, type C and a
/// water-rich type C.
fn interiors() -> [(&'static str, WoitkeType, AirElements); 5] {
    [
        ("A with H2", WoitkeType::A, air(0.6, 0.05, 0.05, 0.1)),
        ("A with N2", WoitkeType::A, air(0.45, 0.05, 0.1, 0.1)),
        ("B", WoitkeType::B, air(0.2, 0.1, 0.1, 0.6)),
        ("C", WoitkeType::C, air(0.4, 0.15, 0.05, 0.25)),
        ("water-rich C", WoitkeType::C, air(0.6, 0.06, 0.05, 0.3)),
    ]
}

/// `elements` solved at `t_k` and `bar`.
fn at(elements: &AirElements, t_k: f64, bar: f64) -> SecondaryEquilibrium {
    solve(
        elements,
        Kelvin::new(t_k),
        Pascals::new(bar * PASCALS_PER_BAR),
    )
    .unwrap_or_else(|e| panic!("{elements:?} at {t_k} K and {bar} bar: {e}"))
}

/// The place of `key` in [`SPECIES`].
fn place(key: &str) -> usize {
    SPECIES
        .iter()
        .position(|&k| k == key)
        .unwrap_or_else(|| panic!("{key} is a species"))
}

/// The mole fraction of `key`.
fn x(air: &SecondaryEquilibrium, key: &str) -> f64 {
    air.fractions[place(key)]
}

/// The share of element `e` (an [`ATOMS`] column) that `keys` hold.
fn share(air: &SecondaryEquilibrium, e: usize, keys: &[&str]) -> f64 {
    let mut held = 0.0;
    let mut all = 0.0;
    for (place, key) in SPECIES.iter().enumerate() {
        let atoms = ATOMS[place][e] * air.fractions[place];
        all += atoms;
        if keys.contains(key) {
            held += atoms;
        }
    }
    held / all
}

/// The largest relative miss of any element's atoms in the gas against the budget's, each as a
/// share of all the atoms; an element the budget lacks must be absent.
fn balance_miss(elements: &AirElements, air: &SecondaryEquilibrium) -> f64 {
    let budget = [
        elements.moles(Element::H),
        elements.moles(Element::C),
        elements.moles(Element::N),
        elements.moles(Element::O),
        elements.moles(Element::He),
        elements.moles(Element::Ar),
    ];
    let mut held = [0.0; 6];
    for (place, row) in ATOMS.iter().enumerate() {
        for (e, &n) in row.iter().enumerate() {
            held[e] += n * air.fractions[place];
        }
    }
    held[4] = air.fractions[HE];
    held[5] = air.fractions[AR];
    let held_total: f64 = held.iter().sum();
    let budget_total: f64 = budget.iter().sum();
    let mut worst = 0.0_f64;
    for (h, b) in held.iter().zip(budget) {
        if b > 0.0 {
            worst = worst.max(((h / held_total) / (b / budget_total) - 1.0).abs());
        } else {
            assert!(*h <= 0.0, "{elements:?}: an absent element is held");
        }
    }
    worst
}

/// The registry's row of `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// The species are rows 0–8 and CO, in registry order, as [`SPECIES`] names them.
#[test]
fn the_species_are_the_registry_s_rows_in_its_order() {
    let ids = species_ids();
    for (id, key) in ids.iter().zip(SPECIES) {
        assert_eq!(*id, row(key));
    }
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "{ids:?}");
}

/// Every species' formation constant holds from [`FLOOR`] to [`CEILING`], and the floor is the
/// highest of their lower bounds: graphite's 300 K, under CH₄, CO and CO₂.
#[test]
fn the_floor_is_graphite_s_and_the_ceiling_inside_the_records() {
    let mut highest_floor = 0.0_f64;
    for place in [H2O, CH4, NH3, CO2, CO] {
        let (lower, upper) = formation_range(species_ids()[place]).expect("a record");
        assert!(lower.value() <= FLOOR.value() && CEILING.value() <= upper.value());
        highest_floor = highest_floor.max(lower.value());
    }
    assert!(
        (highest_floor - FLOOR.value()).abs() < 1e-12,
        "{highest_floor}"
    );
    let (lower, _) = formation_range(row("CH4")).expect("a record");
    assert!((lower.value() - 300.0).abs() < 1e-12);
}

/// Woitke et al.'s (2021) types at 300–600 K and 1 bar: each type's species hold at least 99.99%
/// of each element at 300 K, 99.9% at 400 K, 99% at 500 K and 90% at 600 K (where the water-rich
/// type C has turned 5% of its hydrogen to H₂), and type A's nitrogen is NH₃ at 300 K (at least
/// 99.9%, with H₂ left over), where at 1 bar it turns to N₂ by 600 K.
#[test]
fn woitke_s_types_hold_at_300_to_600_k() {
    // Each element's carriers in each type: H, C, N, O.
    let carriers = |kind: WoitkeType| -> [&'static [&'static str]; 4] {
        match kind {
            WoitkeType::A => [
                &["H2O", "CH4", "NH3", "H2"],
                &["CH4"],
                &["NH3", "N2"],
                &["H2O"],
            ],
            WoitkeType::B => [&["H2O"], &["CO2"], &["N2"], &["O2", "H2O", "CO2"]],
            WoitkeType::C => [&["H2O", "CH4"], &["CH4", "CO2"], &["N2"], &["H2O", "CO2"]],
        }
    };
    for (name, kind, elements) in interiors() {
        assert_eq!(elements.woitke_type(), kind, "{name}");
        for t in [300.0, 400.0, 500.0, 600.0] {
            let gas = at(&elements, t, 1.0);
            let floor = if t < 350.0 {
                0.9999
            } else if t < 450.0 {
                0.999
            } else if t < 550.0 {
                0.99
            } else {
                0.9
            };
            for (e, keys) in carriers(kind).into_iter().enumerate() {
                let held = share(&gas, e, keys);
                assert!(
                    held >= floor,
                    "{name} at {t} K: element {e} {held} in {keys:?}"
                );
            }
        }
    }
    let a = at(&air(0.6, 0.05, 0.05, 0.1), 300.0, 1.0);
    assert!(share(&a, N, &["NH3"]) >= 0.999);
    assert!(x(&a, "H2") > 0.1);
    let hot = at(&air(0.6, 0.05, 0.05, 0.1), 600.0, 1.0);
    assert!(share(&hot, N, &["N2"]) > 0.9);
}

/// The low-temperature limit: at 300 K the solution is the stoichiometric partition
/// ([`AirElements::low_temperature_limit`]) of each type's interior within 1% in every species it
/// holds, and every species it leaves out is under 10⁻⁴; at 1 and 100 bar. Type A's N₂ variant
/// splits the hydrogen its H₂O and CH₄ leave between NH₃ and H₂ by the Haber equilibrium, not all
/// to NH₃: its NH₃ is held within 5% (3.6% under at 1 bar) and its H₂ under 1% of the gas (0.4%).
#[test]
fn the_low_temperature_limit_is_the_stoichiometric_partition() {
    for (name, _, elements) in interiors() {
        let limit = elements.low_temperature_limit().expect("a gas limit");
        let total: f64 = limit.iter().sum();
        for bar in [1.0, 100.0] {
            let gas = at(&elements, 300.0, bar);
            for (place, key) in SPECIES.iter().enumerate() {
                let want = limit[place] / total;
                let got = gas.fractions[place];
                if name == "A with N2" && *key == "NH3" {
                    assert!(
                        (got / want - 1.0).abs() < 0.05,
                        "{name} at {bar} bar: {got}"
                    );
                } else if name == "A with N2" && *key == "H2" {
                    assert!(got < 0.01, "{name} at {bar} bar: {got}");
                } else if want > 0.0 {
                    assert!(
                        (got / want - 1.0).abs() < 0.01,
                        "{name} at {bar} bar: {key} {got} against {want}"
                    );
                } else {
                    assert!(got < 1e-4, "{name} at {bar} bar: {key} {got}");
                }
            }
        }
    }
}

/// The solution's mass balance closes to 10⁻¹² relative for every element over the domain: the
/// H–C–O triangle in twentieths (the carbon the gas can carry), N at 0, 2%, 20% and 100% of them,
/// with Ar at 0 or 5%, at 300–2,000 K and 10⁻³–10³ bar.
#[test]
fn the_mass_balance_closes_over_the_domain() {
    let steps = 20_u32;
    let mut solved = 0_u32;
    let mut worst = 0.0_f64;
    for i in 0..=steps {
        for j in 0..=(steps - i) {
            let h = f64::from(i) / f64::from(steps);
            let c = f64::from(j) / f64::from(steps);
            let o = f64::from(steps - i - j) / f64::from(steps);
            for n in [0.0, 0.02, 0.2, 1.0] {
                for argon in [0.0, 0.05] {
                    let elements = AirElements::new([
                        (Element::H, h),
                        (Element::C, c),
                        (Element::N, n),
                        (Element::O, o),
                        (Element::Ar, argon),
                    ])
                    .expect("a budget");
                    if c > 0.0 && c >= 0.25 * h + o {
                        continue;
                    }
                    for t in [300.0, 400.0, 500.0, 600.0, 800.0, 1_000.0, 1_500.0, 2_000.0] {
                        for bar in [1e-3, 1.0, 100.0, 1e3] {
                            let gas = at(&elements, t, bar);
                            let miss = balance_miss(&elements, &gas);
                            assert!(miss <= 1e-12, "{elements:?} at {t} K, {bar} bar: {miss}");
                            worst = worst.max(miss);
                            solved += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(solved > 30_000, "{solved}");
    assert!(worst > 0.0);
}

/// Each reaction's quotient at the solution is its K from the species' own Gibbs energies, by the
/// gases' references alone (no graphite): the water-gas shift, methanation, the Haber equilibrium
/// and water's formation, within 10⁻¹⁰ in ln K, at 300–2,000 K.
#[test]
fn the_solution_is_each_reaction_s_equilibrium() {
    let reaction = |terms: &[(&str, i32)]| {
        let terms: Vec<(SubstanceId, i32)> = terms.iter().map(|&(k, nu)| (row(k), nu)).collect();
        Reaction::new(&terms).expect("a balanced reaction")
    };
    let reactions = [
        reaction(&[("CO", -1), ("H2O", -1), ("CO2", 1), ("H2", 1)]),
        reaction(&[("CO", -1), ("H2", -3), ("CH4", 1), ("H2O", 1)]),
        reaction(&[("N2", -1), ("H2", -3), ("NH3", 2)]),
        reaction(&[("H2", -2), ("O2", -1), ("H2O", 2)]),
    ];
    for (name, _, elements) in interiors() {
        for t in [300.0, 600.0, 1_000.0, 1_500.0, 2_000.0] {
            let bar = 3.0;
            let gas = at(&elements, t, bar);
            for r in &reactions {
                let ln_k = r
                    .log10_equilibrium_constant(Kelvin::new(t))
                    .expect("a record")
                    * core::f64::consts::LN_10;
                let mut quotient = 0.0;
                for (id, nu) in r.species() {
                    let fraction = gas.mole_fraction(id).expect("a species");
                    quotient += f64::from(nu) * math::ln(fraction * bar);
                }
                assert!(
                    (quotient - ln_k).abs() < 1e-10 * ln_k.abs().max(1.0),
                    "{name} at {t} K: {quotient} against {ln_k}"
                );
            }
        }
    }
}

/// The mole fractions along `budgets` at `t_k` and 1 bar, each solved.
fn sweep(budgets: &[AirElements], t_k: f64) -> Vec<SecondaryEquilibrium> {
    budgets.iter().map(|b| at(b, t_k, 1.0)).collect()
}

/// The largest change of any mole fraction between neighbours of `airs`.
fn largest_step(airs: &[SecondaryEquilibrium]) -> f64 {
    let mut worst = 0.0_f64;
    for pair in airs.windows(2) {
        for (a, b) in pair[0].fractions.iter().zip(pair[1].fractions) {
            worst = worst.max((a - b).abs());
        }
    }
    worst
}

/// Continuous across each type's boundary: at 300, 400 and 1,000 K and 1 bar, with H 0.5, C 0.1
/// and N 0.05, the oxygen swept across A–C (O = H ÷ 2 − 2 C = 0.05) and C–B (O = H ÷ 2 + 2 C =
/// 0.45) in steps of 10⁻³ moves no mole fraction by more than 0.01 a step, and the two sides of
/// each boundary, 10⁻⁹ apart, agree within 10⁻⁶.
#[test]
fn the_solution_is_continuous_across_each_type_s_boundary() {
    let budget = |o: f64| air(0.5, 0.1, 0.05, o);
    let budgets: Vec<AirElements> = (20..=500).map(|k| budget(f64::from(k) * 1e-3)).collect();
    for t in [300.0, 400.0, 1_000.0] {
        let airs = sweep(&budgets, t);
        let kinds: Vec<WoitkeType> = budgets.iter().map(AirElements::woitke_type).collect();
        assert!(kinds.contains(&WoitkeType::A) && kinds.contains(&WoitkeType::B));
        let step = largest_step(&airs);
        assert!(step < 0.01, "at {t} K: {step}");
        for boundary in [0.05, 0.45] {
            let below = at(&budget(boundary - 5e-10), t, 1.0);
            let above = at(&budget(boundary + 5e-10), t, 1.0);
            assert_ne!(
                budget(boundary - 5e-10).woitke_type(),
                budget(boundary + 5e-10).woitke_type()
            );
            for (key, (a, b)) in SPECIES
                .iter()
                .zip(below.fractions.iter().zip(above.fractions))
            {
                assert!(
                    (a - b).abs() < 1e-6,
                    "{key} at {boundary}, {t} K: {a} and {b}"
                );
            }
        }
    }
}

/// Continuous in temperature: each interior budget at 1 bar from 300 to 2,000 K in steps of 1 K
/// moves no mole fraction by more than 0.005 a step.
#[test]
fn the_solution_is_continuous_in_temperature() {
    for (name, _, elements) in interiors() {
        let airs: Vec<SecondaryEquilibrium> = (300..=2_000)
            .map(|t| at(&elements, f64::from(t), 1.0))
            .collect();
        let step = largest_step(&airs);
        assert!(step < 0.005, "{name}: {step}");
    }
}

/// CO appears only above about 600 K (recalled, Woitke et al. 2021): at 1 bar it holds under 1% of
/// every type's carbon at 300–600 K, and over 5% of type A's and C's at 800 K, where the oxygen
/// left by type B's O₂ keeps it under 1% to 2,000 K.
#[test]
fn carbon_monoxide_appears_only_above_about_600_k() {
    for (name, kind, elements) in interiors() {
        for t in [300.0, 400.0, 500.0, 600.0] {
            let gas = at(&elements, t, 1.0);
            let co = share(&gas, C, &["CO"]);
            assert!(co < 0.01, "{name} at {t} K: {co}");
        }
        let hot = at(&elements, 800.0, 1.0);
        let co = share(&hot, C, &["CO"]);
        match kind {
            WoitkeType::A | WoitkeType::C => assert!(co > 0.05, "{name} at 800 K: {co}"),
            WoitkeType::B => {
                for t in [800.0, 1_000.0, 1_500.0, 2_000.0] {
                    let co = share(&at(&elements, t, 1.0), C, &["CO"]);
                    assert!(co < 0.01, "{name} at {t} K: {co}");
                }
            }
        }
    }
}

/// The hot branch (research-leans-58): at 1 bar CO holds over a quarter of type A's and C's carbon
/// at 1,000 K and over half at 1,500 K, CH₄ under a tenth at 1,000 K and under 10⁻⁴ at 1,500 K,
/// and H₂ over a quarter of their hydrogen at 1,500 K; at 100 bar CH₄ holds more than at 1 bar
/// (Le Chatelier).
#[test]
fn the_hot_branch_carries_its_carbon_as_co() {
    for (name, kind, elements) in interiors() {
        if kind == WoitkeType::B {
            continue;
        }
        for t in [1_000.0, 1_500.0] {
            let gas = at(&elements, t, 1.0);
            let co = share(&gas, C, &["CO"]);
            let (least, methane) = if t < 1_200.0 {
                (0.25, 0.1)
            } else {
                (0.5, 1e-4)
            };
            assert!(co > least, "{name} at {t} K: CO {co}");
            let ch4 = share(&gas, C, &["CH4"]);
            assert!(ch4 < methane, "{name} at {t} K: CH4 {ch4}");
            let dense = at(&elements, t, 100.0);
            assert!(
                share(&dense, C, &["CH4"]) > share(&gas, C, &["CH4"]),
                "{name} at {t} K"
            );
        }
        let gas = at(&elements, 1_500.0, 1.0);
        assert!(share(&gas, H, &["H2"]) > 0.25, "{name}");
    }
}

/// Graphite: type C's activity reaches 1 near its carbon-rich edge (the CH₄–CO₂ line, here O =
/// 2 C − H ÷ 2 + 0.005) at 300–600 K and 1 bar, flagged saturated, as it is in the middle of type C
/// at 300 K, where a budget at its water corner (carbon 1% of the atoms) and type A's and type B's
/// interiors stay under 1 from 300 to 2,000 K.
#[test]
fn graphite_saturates_at_type_c_s_carbon_rich_edge() {
    let edge = air(0.3, 0.2, 0.05, 0.4 - 0.15 + 0.005);
    assert_eq!(edge.woitke_type(), WoitkeType::C);
    for t in [300.0, 400.0, 500.0, 600.0] {
        let gas = at(&edge, t, 1.0);
        assert!(
            gas.graphite_saturated(),
            "{t} K: {}",
            gas.graphite_activity()
        );
    }
    let middle = at(&air(0.4, 0.15, 0.05, 0.25), 300.0, 1.0);
    assert!(
        middle.graphite_saturated(),
        "{}",
        middle.graphite_activity()
    );
    let watery = air(0.66, 0.01, 0.05, 0.335);
    assert_eq!(watery.woitke_type(), WoitkeType::C);
    for t in [300.0, 400.0, 500.0, 600.0, 1_000.0, 2_000.0] {
        let gas = at(&watery, t, 1.0);
        assert!(
            !gas.graphite_saturated(),
            "{t} K: {}",
            gas.graphite_activity()
        );
    }
    for (name, kind, elements) in interiors() {
        if kind == WoitkeType::C {
            continue;
        }
        for t in [300.0, 600.0, 800.0, 1_000.0, 1_500.0, 2_000.0] {
            let gas = at(&elements, t, 1.0);
            assert!(gas.graphite_activity() < 1.0, "{name} at {t} K");
        }
    }
    let none = at(&air(0.6, 0.0, 0.1, 0.2), 400.0, 1.0);
    assert_same_bits(none.graphite_activity(), 0.0);
}

/// The share of each element's atoms that `key`, a registry row the solve does not carry, would
/// hold at `gas`'s potentials, by its own formation constant: the largest over H, C, N and O.
fn not_carried_share(gas: &SecondaryEquilibrium, key: &str) -> f64 {
    let id = row(key);
    let t = gas.temperature().value().max(FLOOR.value());
    let bar = gas.pressure().value() / PASCALS_PER_BAR;
    let ln_ref = |element: Element| match element {
        Element::H => 0.5 * math::ln(x(gas, "H2") * bar),
        Element::N => 0.5 * math::ln(x(gas, "N2") * bar),
        Element::O => 0.5 * math::ln(x(gas, "O2") * bar),
        Element::C => math::ln(gas.graphite_activity()),
        other => panic!("{other:?}"),
    };
    let mut ln_p =
        log10_formation_constant(id, Kelvin::new(t)).expect("a record") * core::f64::consts::LN_10;
    for &(element, count) in id.substance().elements() {
        ln_p += f64::from(count) * ln_ref(element);
    }
    let p = math::exp(ln_p);
    let mut worst = 0.0_f64;
    for (e, element) in [
        (H, Element::H),
        (C, Element::C),
        (N, Element::N),
        (O, Element::O),
    ] {
        let count = id
            .substance()
            .elements()
            .iter()
            .find(|&&(el, _)| el == element)
            .map_or(0, |&(_, n)| n);
        if count == 0 {
            continue;
        }
        let mut carried = 0.0;
        for (place, row) in ATOMS.iter().enumerate() {
            carried += row[e] * gas.fractions[place] * bar;
        }
        worst = worst.max(f64::from(count) * p / carried);
    }
    worst
}

/// The registry's gases of H, C, N and O that the solve does not carry stay minor below the
/// ceiling (the [module](super)'s range): at the interiors and three single-carrier airs
/// (steam, a hydrogen-poor oxidised air, an Earth-like one), 1 and 100 bar, each holds under 1% of
/// any element to 1,500 K (OH 0.74% of the hydrogen-poor air's hydrogen there, the most); at
/// 2,000 K OH holds up to a fifth of that air's hydrogen, the H atom up to 0.27% of it, and every
/// other under 0.1% of any element.
#[test]
fn the_species_not_carried_stay_minor() {
    let others = [
        "OH", "H", "O", "N", "C", "HCN", "C2H2", "C2H4", "C2H6", "CH3OH", "N2O", "O3",
    ];
    let mut budgets: Vec<(&str, AirElements)> =
        interiors().into_iter().map(|(n, _, e)| (n, e)).collect();
    budgets.push(("steam", air(0.66, 0.0, 0.001, 0.34)));
    budgets.push(("hydrogen-poor", air(1e-4, 0.32, 0.02, 0.66)));
    budgets.push(("Earth-like", air(2e-3, 4e-4, 1.56, 0.42)));
    for (name, elements) in &budgets {
        for bar in [1.0, 100.0] {
            for t in [300.0, 600.0, 1_000.0, 1_500.0] {
                let gas = at(elements, t, bar);
                for key in others {
                    let held = not_carried_share(&gas, key);
                    assert!(held < 0.01, "{name} at {t} K, {bar} bar: {key} {held}");
                }
            }
            let gas = at(elements, 2_000.0, bar);
            for key in others {
                let held = not_carried_share(&gas, key);
                let bound = match key {
                    "OH" => 0.2,
                    "H" => 3e-3,
                    _ => 1e-3,
                };
                assert!(held < bound, "{name} at 2,000 K, {bar} bar: {key} {held}");
            }
        }
    }
}

/// Below [`FLOOR`] the floor's composition is held, at the same pressure, and so marked.
#[test]
fn below_the_floor_the_floor_s_composition_is_held() {
    for (_, _, elements) in interiors() {
        let cold = at(&elements, 150.0, 2.0);
        let floor = at(&elements, 300.0, 2.0);
        assert_eq!(cold.regime(), Regime::HeldAtFloor);
        assert_eq!(floor.regime(), Regime::Equilibrium);
        assert!((cold.temperature().value() - 150.0).abs() < 1e-12);
        for (a, b) in cold.fractions.iter().zip(floor.fractions) {
            assert_same_bits(*a, b);
        }
    }
}

/// Budgets lacking elements solve: no hydrogen (CO₂, CO, O₂ and N₂), no oxygen (CH₄, H₂, NH₃ and
/// N₂), no carbon, no nitrogen, steam alone, hydrogen alone, oxygen alone, nitrogen alone and the
/// inert gases alone, each closing its balance.
#[test]
fn budgets_lacking_elements_solve() {
    let budgets = [
        air(0.0, 0.3, 0.1, 0.7),
        air(0.0, 0.3, 0.0, 0.4),
        air(0.8, 0.1, 0.1, 0.0),
        air(0.6, 0.0, 0.1, 0.2),
        air(0.6, 0.1, 0.0, 0.2),
        air(2.0, 0.0, 0.0, 1.0),
        air(1.0, 0.0, 0.0, 0.0),
        air(0.0, 0.0, 0.0, 1.0),
        air(0.0, 0.0, 1.0, 0.0),
        AirElements::new([(Element::He, 1.0), (Element::Ar, 2.0)]).expect("a budget"),
        AirElements::new([(Element::H, 1.0), (Element::He, 1.0)]).expect("a budget"),
    ];
    for elements in &budgets {
        for t in [300.0, 1_000.0, 2_000.0] {
            let gas = at(elements, t, 1.0);
            let miss = balance_miss(elements, &gas);
            assert!(miss <= 1e-12, "{elements:?} at {t} K: {miss}");
            let total: f64 = gas.fractions.iter().sum();
            assert!((total - 1.0).abs() < 1e-14);
        }
    }
    let oxygen = at(&air(0.0, 0.0, 0.0, 1.0), 500.0, 1.0);
    assert!((x(&oxygen, "O2") - 1.0).abs() < 1e-15);
    let inert = at(&budgets[9], 500.0, 1.0);
    assert!((x(&inert, "Ar") - 2.0 / 3.0).abs() < 1e-15);
}

/// The refusals: a temperature that is not finite and positive or above the ceiling, a pressure
/// that is not, carbon past what its gas can carry, and the budget's own.
#[test]
fn the_solve_refuses_what_it_cannot_solve() {
    let elements = air(0.6, 0.05, 0.05, 0.1);
    let bar = Pascals::new(1e5);
    for t in [f64::NAN, 0.0, -5.0, f64::INFINITY] {
        assert!(matches!(
            solve(&elements, Kelvin::new(t), bar),
            Err(SolveSecondaryError::Temperature(_))
        ));
    }
    assert!(matches!(
        solve(&elements, Kelvin::new(2_000.5), bar),
        Err(SolveSecondaryError::AboveRange(t)) if (t.value() - 2_000.5).abs() < 1e-12
    ));
    assert!(solve(&elements, CEILING, bar).is_ok());
    for p in [f64::NAN, 0.0, -1.0, f64::INFINITY, 1e-310] {
        assert!(matches!(
            solve(&elements, Kelvin::new(500.0), Pascals::new(p)),
            Err(SolveSecondaryError::Pressure(_))
        ));
    }
    let sooty = air(0.4, 0.35, 0.0, 0.2);
    let Err(SolveSecondaryError::CarbonBeyondGas { carbon, carriers }) =
        solve(&sooty, Kelvin::new(500.0), bar)
    else {
        panic!("carbon past its carriers is refused");
    };
    assert!((carbon - 0.35).abs() < 1e-15 && (carriers - 0.3).abs() < 1e-15);
    assert!(matches!(
        sooty.low_temperature_limit(),
        Err(SolveSecondaryError::CarbonBeyondGas { .. })
    ));
    assert!(matches!(
        solve(&air(0.0, 1.0, 0.0, 0.0), Kelvin::new(500.0), bar),
        Err(SolveSecondaryError::CarbonBeyondGas { .. })
    ));
    assert_eq!(
        AirElements::new([(Element::S, 1.0)]),
        Err(BuildAirElementsError::NotCarried(Element::S))
    );
    assert_eq!(
        AirElements::new([(Element::H, 1.0), (Element::H, 2.0)]),
        Err(BuildAirElementsError::Repeated(Element::H))
    );
    assert!(matches!(
        AirElements::new([(Element::O, -1.0)]),
        Err(BuildAirElementsError::Moles {
            element: Element::O,
            ..
        })
    ));
    assert!(matches!(
        AirElements::new([(Element::O, f64::NAN)]),
        Err(BuildAirElementsError::Moles { .. })
    ));
    assert_eq!(
        AirElements::new([(Element::O, 0.0)]),
        Err(BuildAirElementsError::Empty)
    );
    assert_eq!(AirElements::new([]), Err(BuildAirElementsError::Empty));
    // −0.0 is kept as +0.0.
    let signed = AirElements::new([(Element::H, -0.0), (Element::He, 1.0)]).expect("a budget");
    assert_same_bits(signed.moles(Element::H), 0.0);
}

/// The lookups: a carried species' partial pressure is its mole fraction times the total, and a
/// row the solve does not carry has neither.
#[test]
fn the_lookups_read_the_carried_species_alone() {
    let gas = at(&air(0.4, 0.15, 0.05, 0.25), 450.0, 3.0);
    let water = row("H2O");
    let fraction = gas.mole_fraction(water).expect("a species");
    let pressure = gas.partial_pressure(water).expect("a species");
    assert!((pressure.value() - fraction * 3.0 * PASCALS_PER_BAR).abs() < 1e-9);
    assert!(gas.mole_fraction(row("OH")).is_none());
    assert!(gas.partial_pressure(row("OH")).is_none());
    assert_eq!(gas.mole_fractions().len(), SPECIES.len());
}

/// The same inputs give the same bits twice, and only the budget's ratios matter: a budget scaled
/// by 10⁶ gives the same composition within 10⁻¹⁴ relative, the rounding of its shares.
#[test]
fn the_same_inputs_give_the_same_bits() {
    for (_, _, elements) in interiors() {
        let a = at(&elements, 450.0, 1.0);
        let b = at(&elements, 450.0, 1.0);
        for (p, q) in a.fractions.iter().zip(b.fractions) {
            assert_same_bits(*p, q);
        }
        assert_same_bits(a.graphite_activity(), b.graphite_activity());
        let scaled = AirElements::new(
            [Element::H, Element::C, Element::N, Element::O].map(|e| (e, elements.moles(e) * 1e6)),
        )
        .expect("a budget");
        let c = at(&scaled, 450.0, 1.0);
        for (p, q) in a.fractions.iter().zip(c.fractions) {
            assert!((p - q).abs() <= 1e-14 * p.abs().max(1e-300), "{p} {q}");
        }
    }
}

/// The type and the limit at the triangle's corners and on its lines.
#[test]
fn the_types_meet_on_their_lines() {
    assert_eq!(air(1.0, 0.0, 0.0, 0.0).woitke_type(), WoitkeType::A);
    assert_eq!(air(0.0, 0.0, 0.0, 1.0).woitke_type(), WoitkeType::B);
    assert_eq!(air(0.0, 1.0, 0.0, 0.0).woitke_type(), WoitkeType::C);
    assert_eq!(air(0.0, 0.0, 1.0, 0.0).woitke_type(), WoitkeType::A);
    // Pure water lies on both A's line and B's; A is taken first.
    assert_eq!(air(2.0, 0.0, 0.0, 1.0).woitke_type(), WoitkeType::A);
    let water = air(2.0, 0.0, 0.0, 1.0)
        .low_temperature_limit()
        .expect("a gas");
    assert!((water[H2O] - 1.0).abs() < 1e-15);
    assert_same_bits(water[H2], 0.0);
    // Past the CH₄–CO₂ line: CH₄ on the hydrogen, CO and CO₂ on the oxygen.
    let sooty = air(0.4, 0.3, 0.0, 0.3);
    assert_eq!(sooty.woitke_type(), WoitkeType::C);
    let limit = sooty.low_temperature_limit().expect("a gas");
    assert!((limit[CH4] - 0.1).abs() < 1e-15);
    assert!((limit[CO2] - 0.1).abs() < 1e-15);
    assert!((limit[CO] - 0.1).abs() < 1e-15);
}

/// The golden: each interior budget's type and low-temperature limit, and its composition and
/// graphite activity at 200 (held), 300, 450, 600, 1,000, 1,500 and 2,000 K and at 0.01, 1 and
/// 100 bar.
#[test]
fn the_secondary_equilibrium_is_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    let write = || {
        let mut w = GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        for (name, _, elements) in interiors() {
            w.line(&format!("{name}: {:?}", elements.woitke_type()));
            let limit = elements.low_temperature_limit().expect("a gas limit");
            for (key, moles) in SPECIES.iter().zip(limit) {
                w.f64(&format!("{name} low-temperature limit {key}"), moles);
            }
            for t in [200.0, 300.0, 450.0, 600.0, 1_000.0, 1_500.0, 2_000.0] {
                for bar in [0.01, 1.0, 100.0] {
                    let gas = at(&elements, t, bar);
                    let label = format!("{name} {t} K {bar} bar");
                    w.line(&format!("{label}: {:?}", gas.regime()));
                    for (id, fraction) in gas.mole_fractions() {
                        w.f64(
                            &format!("{label} {}", id.substance().key().as_str()),
                            fraction,
                        );
                    }
                    w.f64(
                        &format!("{label} graphite_activity"),
                        gas.graphite_activity(),
                    );
                }
            }
        }
        w
    };
    let first = write();
    assert_eq!(first.as_str(), write().as_str());
    golden!("planetary/secondary_equilibrium", first.as_str());
}

/// One solve's cost, stated: each type's budget at 300–2,000 K and 1 bar, each timed over 20,000
/// solves (this build's profile and the machine's load; run it with `--release` and one thread).
#[test]
#[ignore = "slow: timing"]
fn one_solve_s_cost_is_stated() {
    let budgets = [
        ("A", air(0.6, 0.05, 0.05, 0.1)),
        ("B", air(0.2, 0.1, 0.1, 0.6)),
        ("C", air(0.4, 0.15, 0.05, 0.25)),
    ];
    let n = 20_000_u32;
    let mut all = core::time::Duration::ZERO;
    let mut cases = 0_u32;
    for (name, elements) in &budgets {
        for t in [300.0, 400.0, 500.0, 600.0, 1_000.0, 1_500.0, 2_000.0] {
            let temperature = Kelvin::new(t);
            let pressure = Pascals::new(1e5);
            let start = std::time::Instant::now();
            for _ in 0..n {
                let air = solve(
                    core::hint::black_box(elements),
                    core::hint::black_box(temperature),
                    core::hint::black_box(pressure),
                );
                core::hint::black_box(air).expect("a solution");
            }
            let each = start.elapsed() / n;
            all += each;
            cases += 1;
            println!("type {name} at {t} K and 1 bar: {each:?} a solve");
        }
    }
    let constants = std::time::Instant::now();
    for _ in 0..n {
        core::hint::black_box(Constants::at(
            core::hint::black_box(Kelvin::new(450.0)),
            &species_ids(),
        ));
    }
    println!("the constants alone: {:?}", constants.elapsed() / n);
    println!("mean over the cases: {:?} a solve", all / cases);
}
