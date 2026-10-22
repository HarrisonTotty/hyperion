use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;

/// The protosolar gas (Lodders 2010, the element registry's), every element heavier than helium
/// times `metallicity`.
fn protosolar(metallicity: f64) -> [(Element, f64); Element::ALL.len()] {
    Element::ALL.map(|e| {
        let n = e.protosolar_abundance_per_hydrogen();
        let scale = if matches!(e, Element::H | Element::He) {
            1.0
        } else {
            metallicity
        };
        (e, n * scale)
    })
}

fn network() -> Network {
    Network::registry().expect("the registry's network builds")
}

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// The gas of the by-hand FastChem cases: the element registry's protosolar abundances (Lodders
/// 2010), every element heavier than helium times `metallicity`, carbon at `c_to_o` times the
/// oxygen where given, and only `elements` where given.
fn gas(metallicity: f64, c_to_o: Option<f64>, elements: Option<&[Element]>) -> Vec<(Element, f64)> {
    let oxygen = Element::O.protosolar_abundance_per_hydrogen() * metallicity;
    protosolar(metallicity)
        .into_iter()
        .filter(|(e, _)| elements.is_none_or(|set| set.contains(e)))
        .map(|(e, n)| match (e, c_to_o) {
            (Element::C, Some(ratio)) => (e, ratio * oxygen),
            _ => (e, n),
        })
        .collect()
}

/// The share of `element` that `id` holds in `g`.
fn share(g: &GasEquilibrium, id: SubstanceId, element: Element) -> f64 {
    let n = f64::from(atoms(id, element));
    let mut total = 0.0;
    for (s, x) in g.mole_fractions() {
        total += f64::from(atoms(s, element)) * x;
    }
    n * g.mole_fraction(id).expect("carried") / total
}

/// Solved at `t` kelvin and `bar` bar.
fn solve_at(network: &Network, gas: &[(Element, f64)], t: f64, bar: f64) -> GasEquilibrium {
    network
        .solve(gas.iter().copied(), Kelvin::new(t), Pascals::new(bar * 1e5))
        .unwrap_or_else(|e| panic!("{t} K, {bar} bar: {e}"))
}

/// The network is the registry's gas rows with a record, each trace species holding one atom of
/// one trace element, over 300 to 6,000 K; every gas row but `formation::NOT_MODELLED`'s joins.
#[test]
fn the_registry_s_network_builds() {
    let network = network();
    assert!((network.floor().value() - 300.0).abs() < 1e-9);
    assert!((network.ceiling().value() - 6_000.0).abs() < 1e-9);
    let carried: Vec<SubstanceId> = network.species().collect();
    for id in crate::substance::ids() {
        if id.substance().gas().is_none() {
            continue;
        }
        let key = id.substance().key();
        let unmodelled = crate::substance::formation::NOT_MODELLED
            .iter()
            .any(|(k, _)| *k == key.as_str());
        assert_eq!(carried.contains(&id), !unmodelled, "{key}");
    }
    for s in &network.species {
        let traces = network
            .traces
            .iter()
            .filter(|t| atoms(s.id, t.element) > 0)
            .count();
        assert!(traces <= 1, "{}", s.id.substance().key());
    }
    // The basis: H₂, CH₄, NH₃, H₂O, H₂S and the electron; each trace element's, its atom, PH₃ or
    // HCl.
    for (k, key) in ["H2", "CH4", "NH3", "H2O", "H2S", "e-"].iter().enumerate() {
        assert_eq!(network.species[network.basis[k]].id, row(key));
    }
    for trace in &network.traces {
        let base = network.species[network.trace_basis(trace)].id;
        let want = match trace.element {
            Element::P => "PH3",
            Element::Cl => "HCl",
            e => e.symbol(),
        };
        assert_eq!(base, row(want), "{:?}", trace.element);
    }
}

/// Every element is conserved to 10⁻¹² and the charge balanced to 10⁻¹² over the domain: 200 K
/// (held at the floor) and 300 to 6,000 K, 10⁻⁸ to 10⁴ bar, a hundredth of protosolar to the
/// bound of hydrogen dominance, C/O from 0.3 to 2; the mole fractions finite, non-negative and
/// summing to 1. A gas beyond the bound
/// is refused, and no solution fails to converge.
#[test]
fn elements_and_charge_are_conserved_over_the_domain() {
    let network = network();
    let mut solved = 0;
    for t in [
        200.0, 300.0, 350.0, 500.0, 700.0, 1_000.0, 1_500.0, 2_000.0, 2_500.0, 3_000.0, 4_000.0,
        5_000.0, 6_000.0,
    ] {
        for bar in [1e-8, 1e-6, 1e-4, 1e-2, 1.0, 1e2, 1e4] {
            for z in [0.01, 1.0, 30.0, 150.0] {
                for c_to_o in [0.3, 0.9, 1.0, 2.0] {
                    let g = gas(z, Some(c_to_o), None);
                    let mut per = [0.0; Element::ALL.len()];
                    for &(e, n) in &g {
                        per[element_index(e)] = n;
                    }
                    let share = Gas::new(&network, &per).hydride_share;
                    let got =
                        network.solve(g.iter().copied(), Kelvin::new(t), Pascals::new(bar * 1e5));
                    if share > HYDROGEN_DOMINANCE_BOUND {
                        assert_eq!(
                            got,
                            Err(SolveEquilibriumError::NotHydrogenDominated { share })
                        );
                        continue;
                    }
                    let got = got
                        .unwrap_or_else(|e| panic!("{t} K, {bar} bar, Z {z}, C/O {c_to_o}: {e}"));
                    solved += 1;
                    let mut sum = 0.0;
                    for (_, x) in got.mole_fractions() {
                        assert!(x.is_finite() && x >= 0.0);
                        sum += x;
                    }
                    assert!((sum - 1.0).abs() < 1e-12);
                    for &(e, want) in &g {
                        if e == Element::H {
                            continue;
                        }
                        let have = got.per_hydrogen(e);
                        assert!(
                            (have / want - 1.0).abs() < 1e-12,
                            "{t} K, {bar} bar, Z {z}, C/O {c_to_o}, {e:?}: {have} against {want}"
                        );
                    }
                    assert!(
                        got.charge_imbalance().abs() < 1e-12,
                        "{t} K, {bar} bar, Z {z}, C/O {c_to_o}"
                    );
                }
            }
        }
    }
    assert!(solved > 1_000, "{solved}");
}

/// The temperature at `bar` bar where `a`'s mole fraction equals `b`'s, by bisection between
/// `low` and `high` kelvin, in a protosolar gas: `a` the cool side's carrier.
fn crossing(network: &Network, a: &str, b: &str, bar: f64, low: f64, high: f64) -> f64 {
    let (a, b) = (row(a), row(b));
    let g = gas(1.0, None, None);
    let excess = |t: f64| {
        let got = solve_at(network, &g, t, bar);
        math::ln(got.mole_fraction(a).unwrap()) - math::ln(got.mole_fraction(b).unwrap())
    };
    let (mut low, mut high) = (low, high);
    assert!(excess(low) > 0.0 && excess(high) < 0.0);
    for _ in 0..40 {
        let mid = low.midpoint(high);
        if excess(mid) > 0.0 {
            low = mid;
        } else {
            high = mid;
        }
    }
    low.midpoint(high)
}

/// CH₄ = CO at Lodders and Fegley's (2002, Icarus 155, 393, p. 403) boundary for solar
/// metallicity: "at the one-bar level, the CH4-CO boundary is at ∼1125 K, while at the 10-bar
/// level, it is at ∼1390 K", within 2.5%. Their system's C/O is about 0.48 with 16% of its oxygen
/// in rock (their eqs. 17–20), so 0.57 in the gas, against this protosolar gas's 0.457 with all
/// its oxygen; at the boundary only the water counts, 4.45 × 10⁻⁴ per H theirs against
/// 4.65 × 10⁻⁴ here, which moves the crossing by about 2 K, so the tolerance is the
/// thermochemistries': their own Fig. 4 and its Gibbs-energy check run 15–25 K hotter than the
/// text.
#[test]
fn methane_equals_carbon_monoxide_at_lodders_and_fegley_s_boundary() {
    let network = network();
    for (bar, want) in [(1.0, 1_125.0), (10.0, 1_390.0)] {
        let got = crossing(&network, "CH4", "CO", bar, 700.0, 2_500.0);
        assert!((got / want - 1.0).abs() < 0.025, "{bar} bar: {got} K");
    }
}

/// NH₃ = N₂ at Lodders and Fegley's (2002) boundary for solar metallicity, their Fig. 18b, which
/// prints no figure: read from the figure by the lane (400 dpi, 0.01–0.03 dex), 559 K at 0.1 bar,
/// 697 K at 1 bar and 925 K at 10 bar, and checked against Heng and Tsai's (2016, Appendix A)
/// Gibbs energies at their N/H (555, 699 and 932 K); within 3%, their N/H of 9.3 × 10⁻⁵ against
/// this gas's 8.1 × 10⁻⁵.
#[test]
fn ammonia_equals_nitrogen_at_lodders_and_fegley_s_boundary() {
    let network = network();
    for (bar, want) in [(0.1, 559.0), (1.0, 697.0), (10.0, 925.0)] {
        let got = crossing(&network, "NH3", "N2", bar, 400.0, 2_000.0);
        assert!((got / want - 1.0).abs() < 0.03, "{bar} bar: {got} K");
    }
}

/// H₂S is the dominant sulphur gas in Visscher, Lodders and Fegley's (2006, ApJ 648, 1181, §3.1)
/// H₂S field: at 1,000 K and 1 bar, and at 1,500 K and 10 mbar, where log p > 6.82 − 17,570 ÷ T
/// (their eq. 4) and log p > 5.81 − 16,046 ÷ T (eq. 10). Each share is held to what their eq. 3,
/// log(H₂S ÷ SH) = −3.37 + 8,785 ÷ T + 0.5 log p + 0.5 log x(H₂), leaves it: over 0.99 at 1,000 K
/// (SH 4 × 10⁻⁶ of H₂S), and over 0.95 at 1,500 K, where SH, not carried, would take 3.4%
/// (P14.T52.e).
#[test]
fn hydrogen_sulphide_is_the_dominant_sulphur_gas() {
    let network = network();
    let g = gas(1.0, None, None);
    for (t, bar, least) in [(1_000.0_f64, 1.0_f64, 0.99), (1_500.0, 0.01, 0.95)] {
        assert!(math::log10(bar) > 6.82 - 17_570.0 / t && math::log10(bar) > 5.81 - 16_046.0 / t);
        let got = solve_at(&network, &g, t, bar);
        assert!(share(&got, row("H2S"), Element::S) > least, "{t} K");
    }
}

/// The S atom is the dominant sulphur gas past Visscher, Lodders and Fegley's (2006, §3.1) eq. 7,
/// where log p < 4.80 − 14,522 ÷ T (SH = S, above their triple point at about 1,509 K), away from
/// SH's field between it and eq. 4: at 3,000 K and 10 mbar and at 5,000 K and 1 bar, each about
/// two decades past the line (eq. 7's K₅ is fitted over 800–2,500 K; CEA's records put the line
/// at log p 2.0 at 5,000 K, against eq. 7's 1.9, so the point stays two decades past it). FastChem 4.0.3's full network there (the asserted cases' version,
/// data and command, [`Case`]: its `logK.dat`, the 25 elements at protosolar abundances, Lodders
/// 2010) gives S 0.953 and 0.985 of the sulphur, SH, not carried (P14.T52.e), 4.5% and 1.4%, and
/// H₂S 7 × 10⁻⁴ and 5 × 10⁻⁵. SH's sulphur goes to S and H₂S here in their own ratio, so S's share
/// is held over FastChem's rounded down, 0.95 and 0.98 (0.9992 and 0.9999 here), and H₂S's under
/// 0.01: H₂S as the carrier, as a network without the S atom would publish it, fails both by far.
#[test]
fn atomic_sulphur_is_the_dominant_sulphur_gas_past_its_line() {
    let network = network();
    let g = gas(1.0, None, None);
    for (t, bar, least) in [(3_000.0_f64, 0.01_f64, 0.95), (5_000.0, 1.0, 0.98)] {
        assert!(
            t > 1_509.0 && math::log10(bar) < 4.80 - 14_522.0 / t - 1.0,
            "{t} K, {bar} bar is not a decade past eq. 7's line, above the triple point"
        );
        let got = solve_at(&network, &g, t, bar);
        assert!(share(&got, row("S"), Element::S) > least, "{t} K");
        assert!(share(&got, row("H2S"), Element::S) < 0.01, "{t} K");
    }
}

/// Iron is monatomic above its cloud (Visscher, Lodders and Fegley 2010, ApJ 716, 1060: "Monatomic
/// Fe is the most abundant Fe-bearing gas throughout the atmospheres of EGPs and L dwarfs"): above
/// its condensation temperature, 10⁴ ÷ T = 5.44 − 0.48 log p (their eq. 2; 1,838 K at 1 bar),
/// the atom holds over 99% of the iron at 1 bar and at 10 mbar, the ion the rest.
#[test]
fn iron_is_monatomic_above_its_cloud() {
    let network = network();
    let g = gas(1.0, None, None);
    for (t, bar) in [(2_000.0_f64, 1.0_f64), (2_000.0, 0.01), (2_500.0, 1.0)] {
        assert!(t > 1e4 / (5.44 - 0.48 * math::log10(bar)));
        let got = solve_at(&network, &g, t, bar);
        assert!(
            share(&got, row("Fe"), Element::Fe) > 0.99,
            "{t} K, {bar} bar"
        );
    }
}

/// Each asserted case of FastChem 4.0.3 (Stock et al. 2018, MNRAS 479, 865; Stock, Kitzmann and
/// Patzer 2022, MNRAS 517, 4070; Kitzmann, Stock and Patzer 2024, MNRAS 527, 7263; GPL-3.0),
/// run by hand outside the repository (decision-composition §5) as pyfastchem 4.0.3 from PyPI,
/// built under CPython 3.14, its data files at the FastChem repository's tag v4.0.3
/// (ae67cbd559bc64a3233a1cee6030b8e6b50520de), gas phase only, `accuracyChem` 10⁻⁸:
/// `FastChem(abundances, logK, 0)`, `calcDensities` at each (T, p), the mole fraction each
/// species' number density over their sum. The abundances are this registry's protosolar ones
/// (Lodders 2010), written as A(X) = log₁₀(N ÷ `N_H`) + 12, times the case's metallicity for
/// every element heavier than helium, carbon at the case's C/O times the oxygen where it has one.
///
/// Each entry is a species holding at least 1% of one of its elements in FastChem's run, or, on
/// the 4,600 K dayside, the electrons and H⁻, with FastChem's mole fraction and the relative
/// tolerance asserted.
struct Case {
    label: &'static str,
    metallicity: f64,
    c_to_o: Option<f64>,
    elements: &'static [Element],
    temperature: f64,
    bar: f64,
    species: &'static [(&'static str, f64, f64)],
}

/// Every element but Ni, P and Cl: the network-matched runs' set. FastChem always carries the
/// atoms of P and Cl, which this network does not, and its `logK.dat` record of Ni⁺ (a JANAF fit)
/// gives K(Ni⁺) ÷ K(Fe⁺) of 108–247 over 1,500–6,000 K, against CEA's records' 1.97, 0.56 and 0.42
/// at 1,500, 3,000 and 6,000 K, and the Saha equation's 0.420 at 6,000 K from the ionisation
/// energies (7.640 and 7.902 eV) and NIST's partition functions.
const NETWORK_MATCHED: [Element; 22] = [
    Element::H,
    Element::He,
    Element::C,
    Element::N,
    Element::O,
    Element::Ne,
    Element::Na,
    Element::Mg,
    Element::Al,
    Element::Si,
    Element::S,
    Element::Ar,
    Element::K,
    Element::Ca,
    Element::Ti,
    Element::V,
    Element::Cr,
    Element::Mn,
    Element::Fe,
    Element::Zn,
    Element::Kr,
    Element::Xe,
];

/// Heng and Tsai's (2016) elements, with helium.
const CHON: [Element; 5] = [Element::H, Element::He, Element::C, Element::N, Element::O];

/// The same with sulphur.
const CHONS: [Element; 6] = [
    Element::H,
    Element::He,
    Element::C,
    Element::N,
    Element::O,
    Element::S,
];

/// The asserted cases.
///
/// - **In Heng and Tsai's domain** (500–3,000 K; C, H, O, N and He, with S in one case), FastChem's
///   full `logK.dat` network: each carrier within 2.5%. Heng and Tsai find their own solution
///   "accurate at the ∼1% level"; the misses here, at most 1.9% (CO at 1,100 K and 1 bar), are the
///   two thermochemistries', CEA's records and FastChem's fits.
/// - **Network-matched** (`logK.dat`'s entries for this network's species alone, cut from it by
///   the lane's scratch script, every element but Ni, P and Cl; FastChem adds every element's
///   atom, so the species are this network's but CH₃OH and C₂H₆, which its data does not hold and
///   which hold under 0.2% of the carbon on the grid here): each carrier holding most of its
///   element within 1% (2% for H₂O and CO at 2,500 K and 100 bar), the rest within the
///   thermochemistries' difference, entry by entry: NH₃, CO, CO₂, CH₄ and TiO 5% where they hold a
///   minor share, HCN 20%, CH₄ beside CO 30% at 2,500 K, Ti 25%; on the 4,600 K dayside the
///   iron's ionisation, CEA's K for Fe ⇌ Fe⁺ + e⁻ being 0.92 of FastChem's there (the two runs'
///   x(Fe⁺) x(e⁻) ÷ x(Fe)): Fe⁺ 5% though it holds 56–82% of the iron, Fe 10%, the electrons and
///   H⁻, which carry it, 2%, and Mg and Si⁺, each the minor state of its metal, 2%; and vanadium,
///   whose VO record differs most (CEA's K for VO is 0.10–0.41 of FastChem's at 1,000–2,500 K),
///   VO 20–80% and the V atom up to 4.5 times.
///
/// The runs: `fastchem_run.py <site-packages> <logK file> <cases.json> <work dir> <out.json>`, a
/// scratch driver that writes each case's abundance file (`e-` at 0, then A(X) per element), builds
/// `pyfastchem.FastChem(abundance file, logK file, 0)`, sets the case's parameters, calls
/// `calcDensities` over its (T, p) points in bar, and writes each species' number density over the
/// sum of all of them, electrons included; every point's flag was `FASTCHEM_SUCCESS`.
const CASES: [Case; 12] = [
    Case {
        label: "Heng and Tsai, protosolar, 1,100 K, 1 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &CHON,
        temperature: 1_100.0,
        bar: 1.0,
        species: &[
            ("H2", 8.363_465e-1, 0.025),
            ("He", 1.622_554e-1, 0.025),
            ("H2O", 8.673_945e-4, 0.025),
            ("CH4", 3.193_520e-4, 0.025),
            ("NH3", 2.029_436e-6, 0.025),
            ("N2", 6.708_772e-5, 0.025),
            ("CO", 1.420_280e-4, 0.025),
        ],
    },
    Case {
        label: "Heng and Tsai, ten times protosolar, 2,000 K, 1 bar",
        metallicity: 10.0,
        c_to_o: None,
        elements: &CHON,
        temperature: 2_000.0,
        bar: 1.0,
        species: &[
            ("H2", 8.265_517e-1, 0.025),
            ("He", 1.612_634e-1, 0.025),
            ("H2O", 5.440_817e-3, 0.025),
            ("N2", 6.766_539e-4, 0.025),
            ("CO", 4.580_428e-3, 0.025),
        ],
    },
    Case {
        label: "Heng and Tsai, C/O 1, 1,500 K, 1 bar",
        metallicity: 1.0,
        c_to_o: Some(1.0),
        elements: &CHON,
        temperature: 1_500.0,
        bar: 1.0,
        species: &[
            ("H2", 8.367_801e-1, 0.025),
            ("He", 1.620_775e-1, 0.025),
            ("H2O", 4.957_920e-5, 0.025),
            ("CH4", 4.933_600e-5, 0.025),
            ("N2", 6.771_651e-5, 0.025),
            ("CO", 9.589_850e-4, 0.025),
        ],
    },
    Case {
        label: "Heng and Tsai with sulphur, protosolar, 1,100 K, 1 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &CHONS,
        temperature: 1_100.0,
        bar: 1.0,
        species: &[
            ("H2", 8.363_194e-1, 0.025),
            ("H2O", 8.673_858e-4, 0.025),
            ("CH4", 3.193_433e-4, 0.025),
            ("N2", 6.708_777e-5, 0.025),
            ("CO", 1.420_366e-4, 0.025),
            ("H2S", 2.717_599e-5, 0.025),
        ],
    },
    Case {
        label: "network-matched, protosolar, 1,000 K, 1 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 1_000.0,
        bar: 1.0,
        species: &[
            ("H2", 8.358_690e-1, 0.01),
            ("He", 1.622_282e-1, 0.01),
            ("H2O", 9.262_451e-4, 0.01),
            ("CH4", 4.435_390e-4, 0.01),
            ("NH3", 3.679_173e-6, 0.05),
            ("N2", 6.625_232e-5, 0.01),
            ("CO", 1.788_374e-5, 0.05),
            ("H2S", 2.717_229e-5, 0.01),
            ("Na", 3.750_821e-6, 0.01),
            ("K", 2.421_733e-7, 0.01),
            ("Fe", 5.421_585e-5, 0.01),
            ("Mg", 6.670_007e-5, 0.01),
            ("Ca", 3.927_591e-6, 0.01),
            ("SiO", 6.518_179e-5, 0.01),
            ("TiO", 1.600_022e-7, 0.01),
            ("VO", 1.836_965e-8, 0.01),
            ("Al", 5.421_585e-6, 0.01),
            ("Mn", 5.944_650e-7, 0.01),
            ("Zn", 8.397_041e-8, 0.01),
            ("Cr", 8.397_041e-7, 0.01),
        ],
    },
    Case {
        label: "network-matched, protosolar, 1,500 K, 10 mbar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 1_500.0,
        bar: 0.01,
        species: &[
            ("H2", 8.363_122e-1, 0.01),
            ("H2O", 4.821_642e-4, 0.01),
            ("N2", 6.802_390e-5, 0.01),
            ("CO", 4.609_005e-4, 0.01),
            ("H2S", 2.711_017e-5, 0.01),
            ("K", 2.406_126e-7, 0.01),
            ("Fe", 5.416_324e-5, 0.01),
            ("SiO", 6.511_853e-5, 0.01),
            ("TiO", 1.596_606e-7, 0.01),
            ("VO", 1.769_126e-8, 0.2),
            ("V", 6.616_335e-10, 4.5),
        ],
    },
    Case {
        label: "network-matched, a hundred times protosolar, 1,000 K, 10 mbar",
        metallicity: 100.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 1_000.0,
        bar: 0.01,
        species: &[
            ("H2", 7.218_645e-1, 0.01),
            ("He", 1.481_949e-1, 0.01),
            ("H2O", 4.089_967e-2, 0.01),
            ("N2", 6.220_039e-3, 0.01),
            ("CO2", 3.199_365e-3, 0.05),
            ("CO", 3.895_267e-2, 0.01),
            ("H2S", 2.482_179e-3, 0.01),
            ("Fe", 4.952_599e-3, 0.01),
            ("SiO", 5.954_333e-3, 0.01),
            ("TiO", 1.461_615e-5, 0.01),
            ("VO", 1.678_157e-6, 0.01),
        ],
    },
    Case {
        label: "network-matched, C/O 1.5, 1,500 K, 1 bar",
        metallicity: 1.0,
        c_to_o: Some(1.5),
        elements: &NETWORK_MATCHED,
        temperature: 1_500.0,
        bar: 1.0,
        species: &[
            ("H2", 8.358_586e-1, 0.01),
            ("CH4", 5.690_782e-4, 0.05),
            ("N2", 6.657_252e-5, 0.01),
            ("CO", 9.392_597e-4, 0.01),
            ("H2S", 2.715_002e-5, 0.01),
            ("HCN", 2.534_431e-6, 0.2),
            ("Ti", 1.890_727e-8, 0.25),
            ("TiO", 1.409_660e-7, 0.05),
            ("VO", 3.466_488e-9, 0.8),
            ("V", 1.488_942e-8, 0.25),
        ],
    },
    Case {
        label: "network-matched, protosolar, 700 K, 100 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 700.0,
        bar: 100.0,
        species: &[
            ("H2", 8.357_572e-1, 0.01),
            ("H2O", 9.443_452e-4, 0.01),
            ("CH4", 4.615_294e-4, 0.01),
            ("NH3", 1.361_336e-4, 0.01),
            ("H2S", 2.717_687e-5, 0.01),
            ("SiO", 6.519_276e-5, 0.01),
        ],
    },
    Case {
        label: "network-matched, protosolar, 2,500 K, 100 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 2_500.0,
        bar: 100.0,
        species: &[
            ("H2", 8.343_248e-1, 0.01),
            ("H2O", 4.982_646e-4, 0.02),
            ("CH4", 1.537_136e-5, 0.3),
            ("NH3", 6.497_332e-6, 0.05),
            ("N2", 6.424_622e-5, 0.01),
            ("CO", 4.441_246e-4, 0.02),
            ("H2S", 2.704_621e-5, 0.01),
            ("K", 2.245_198e-7, 0.01),
            ("Ti", 3.501_137e-8, 0.25),
            ("TiO", 1.246_709e-7, 0.05),
            ("VO", 3.704_144e-9, 0.8),
            ("V", 1.462_982e-8, 0.25),
            ("K+", 1.716_930e-8, 0.01),
        ],
    },
    Case {
        label: "network-matched, protosolar, 4,600 K, 10 mbar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 4_600.0,
        bar: 0.01,
        species: &[
            ("H", 9.101_264e-1, 0.01),
            ("He", 8.822_768e-2, 0.01),
            ("O", 4.704_315e-4, 0.01),
            ("C", 1.723_650e-4, 0.01),
            ("CO", 7.859_521e-5, 0.01),
            ("N", 7.330_713e-5, 0.01),
            ("N2", 3.782_238e-7, 0.01),
            ("S", 1.477_763e-5, 0.01),
            ("Mg+", 3.290_739e-5, 0.01),
            ("Mg", 3.367_398e-6, 0.02),
            ("Si", 1.925_706e-5, 0.01),
            ("Si+", 1.617_827e-5, 0.02),
            ("Fe+", 2.407_293e-5, 0.05),
            ("Fe", 5.412_325e-6, 0.1),
            ("Ca+", 2.131_321e-6, 0.01),
            ("Na+", 2.038_386e-6, 0.01),
            ("e-", 8.119_206e-5, 0.02),
            ("H-", 2.589_810e-9, 0.02),
        ],
    },
    Case {
        label: "network-matched, protosolar, 4,600 K, 50 mbar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 4_600.0,
        bar: 0.05,
        species: &[
            ("H", 9.079_423e-1, 0.01),
            ("He", 8.842_236e-2, 0.01),
            ("O", 3.862_262e-4, 0.01),
            ("CO", 1.639_335e-4, 0.01),
            ("C", 8.758_006e-5, 0.01),
            ("N", 7.070_789e-5, 0.01),
            ("N2", 1.759_390e-6, 0.01),
            ("S", 1.481_023e-5, 0.01),
            ("Mg+", 2.666_307e-5, 0.01),
            ("Mg", 9.691_755e-6, 0.02),
            ("Si", 2.866_397e-5, 0.01),
            ("Si+", 6.779_337e-6, 0.02),
            ("Fe+", 1.642_934e-5, 0.05),
            ("Fe", 1.312_098e-5, 0.1),
            ("Ca+", 2.124_105e-6, 0.01),
            ("Na+", 2.039_069e-6, 0.01),
            ("e-", 5.768_125e-5, 0.02),
            ("H-", 9.177_311e-9, 0.02),
        ],
    },
];

#[test]
fn every_asserted_case_is_within_its_tolerance() {
    let network = network();
    for case in &CASES {
        let g = gas(case.metallicity, case.c_to_o, Some(case.elements));
        let got = solve_at(&network, &g, case.temperature, case.bar);
        for &(key, want, tolerance) in case.species {
            let have = got.mole_fraction(row(key)).unwrap();
            assert!(
                (have / want - 1.0).abs() < tolerance,
                "{}: {key} {have:e} against FastChem's {want:e}",
                case.label
            );
        }
    }
}

/// A level of the KELT-9b-like dayside at 4,600 K: its pressure, bar; FastChem's mole fractions
/// of H, S, N and O, the electrons and H⁻, each with its relative tolerance; and FastChem's share
/// of the carbon in the C atom.
struct DaysideLevel {
    bar: f64,
    species: [(&'static str, f64, f64); 6],
    carbon_atom: f64,
}

/// The KELT-9b-like dayside's levels: 4,600 K, Gaudi et al.'s (2017, Nature 546, 514) dayside of
/// about 4,600 K and science-r08-unowned-absorbers §2.2's reference at 0.05 bar, at 10, 30 and
/// 50 mbar. FastChem 4.0.3's full network (the asserted cases' version, data and command,
/// [`Case`]: its `logK.dat`, gas only, `accuracyChem` 10⁻⁸), the 25 elements at protosolar
/// abundances (Lodders 2010); every point's flag was `FASTCHEM_SUCCESS`. Each tolerance is stated
/// against what FastChem holds there in species not carried:
///
/// - **H**, 0.1%: they hold under 3 × 10⁻⁶ of the hydrogen (H⁺ and OH the most).
/// - **S**, 1%: S⁺ and SH hold 0.20–0.28% of the sulphur, which goes to S here (S +0.21% to
///   +0.28%).
/// - **N**, 1%: NH, CN and NO hold 0.04–0.16% of the nitrogen (N +0.05% to +0.16%).
/// - **O**, 1%: OH holds 0.09–0.37% of the oxygen, which goes to O here (O +0.14% to +0.50%).
/// - **The electrons and H⁻**, 4%: H⁺, C⁺ and S⁺, not carried, are 0.35–0.91% of FastChem's
///   cations, and its Ni⁺ record (`NETWORK_MATCHED`'s reason for leaving Ni out) ionises 99% of
///   the nickel where CEA's records ionise 33–63%, a further 0.8–2.0% of the electrons; the
///   electrons are 1.8–2.5% low here, and H⁻, at K p(H) p(e⁻), with them. Without Ni, P and Cl
///   (the network-matched cases at 4,600 K) both are within 0.8%.
/// - **The C : CO split**: C's share of the carbon within 0.01 of FastChem's (0.687, 0.455 and
///   0.349, so the atom holds the carbon at 10 mbar and CO from 30 mbar; about 0.001 here), C⁺,
///   CH and CN holding under 0.06% of it and CEA's K for C + O ⇌ CO being 0.5% below FastChem's
///   (the network-matched runs); and C and CO together over 0.999 of it (FastChem's
///   0.9994–0.9995).
const KELT_9B_DAYSIDE: [DaysideLevel; 3] = [
    DaysideLevel {
        bar: 0.01,
        species: [
            ("H", 9.101_218e-1, 0.001),
            ("S", 1.473_683e-5, 0.01),
            ("N", 7.327_507e-5, 0.01),
            ("O", 4.700_093e-4, 0.01),
            ("e-", 8.326_568e-5, 0.04),
            ("H-", 2.655_939e-9, 0.04),
        ],
        carbon_atom: 0.6867,
    },
    DaysideLevel {
        bar: 0.03,
        species: [
            ("H", 9.090_457e-1, 0.001),
            ("S", 1.476_392e-5, 0.01),
            ("N", 7.188_567e-5, 0.01),
            ("O", 4.114_881e-4, 0.01),
            ("e-", 6.688_316e-5, 0.04),
            ("H-", 6.392_582e-9, 0.04),
        ],
        carbon_atom: 0.4550,
    },
    DaysideLevel {
        bar: 0.05,
        species: [
            ("H", 9.079_383e-1, 0.001),
            ("S", 1.477_458e-5, 0.01),
            ("N", 7.060_189e-5, 0.01),
            ("O", 3.845_165e-4, 0.01),
            ("e-", 5.911_495e-5, 0.04),
            ("H-", 9.405_378e-9, 0.04),
        ],
        carbon_atom: 0.3490,
    },
];

/// A KELT-9b-like dayside (decision-p14-t52a-rows) is held in atoms and ions as FastChem's full
/// network holds it ([`KELT_9B_DAYSIDE`]): H, S, N, O, the electrons and H⁻ within their
/// tolerances, the C : CO split, and Fe⁺ above Fe and Ca⁺ above Ca at every level (FastChem's
/// Fe⁺ 0.81, 0.64 and 0.55 of the iron, Ca⁺ 0.99 of the calcium).
#[test]
fn a_kelt_9b_like_dayside_is_atomic_and_ionised() {
    let network = network();
    let g = gas(1.0, None, None);
    for level in &KELT_9B_DAYSIDE {
        let got = solve_at(&network, &g, 4_600.0, level.bar);
        for &(key, want, tolerance) in &level.species {
            let have = got.mole_fraction(row(key)).unwrap();
            assert!(
                (have / want - 1.0).abs() < tolerance,
                "{} bar: {key} {have:e} against FastChem's {want:e}",
                level.bar
            );
        }
        let atom = share(&got, row("C"), Element::C);
        let monoxide = share(&got, row("CO"), Element::C);
        assert!(
            (atom - level.carbon_atom).abs() < 0.01,
            "{} bar: C holds {atom} of the carbon",
            level.bar
        );
        assert!(atom + monoxide > 0.999, "{} bar", level.bar);
        for (ion, neutral) in [("Fe+", "Fe"), ("Ca+", "Ca")] {
            let ionised = got.mole_fraction(row(ion)).unwrap();
            let held = got.mole_fraction(row(neutral)).unwrap();
            assert!(
                ionised > held,
                "{} bar: {ion} {ionised:e} against {neutral} {held:e}",
                level.bar
            );
        }
    }
}

/// Below the records' floor the floor's composition is held, the same bits at 166 K (Jupiter's
/// 1-bar level) as at 300 K, so that it is continuous at the floor; at a bar its major carriers
/// are all but all-in, the plan's limit below 500 K: CH₄ over 0.999 999 of the carbon, NH₃ over
/// 0.999 of the nitrogen, H₂S, PH₃ and HCl over 0.99 of theirs, and H₂O over 0.9 of the oxygen,
/// the rest in SiO, TiO and VO.
#[test]
fn below_the_floor_the_floor_s_composition_is_held() {
    let network = network();
    let g = gas(1.0, None, None);
    let cold = solve_at(&network, &g, 166.0, 1.0);
    let floor = solve_at(&network, &g, 300.0, 1.0);
    assert_eq!(cold.regime(), Regime::HeldAtFloor);
    assert_eq!(floor.regime(), Regime::Equilibrium);
    for ((a, x), (b, y)) in cold.mole_fractions().zip(floor.mole_fractions()) {
        assert_eq!(a, b);
        assert_same_bits(x, y);
    }
    for (key, element, least) in [
        ("CH4", Element::C, 0.999_999),
        ("NH3", Element::N, 0.999),
        ("H2O", Element::O, 0.9),
        ("H2S", Element::S, 0.99),
        ("PH3", Element::P, 0.99),
        ("HCl", Element::Cl, 0.99),
    ] {
        assert!(share(&cold, row(key), element) > least, "{key}");
    }
    for &(e, want) in &g {
        assert!((cold.per_hydrogen(e) / want - 1.0).abs() < 1e-12, "{e:?}");
    }
}

/// A gas richer in carbon than oxygen is accepted (decision-p14-t50b-envelope §5): its oxygen is
/// all in CO and the rest of the carbon in CH₄, C₂H₂ and HCN, the water a trace.
#[test]
fn carbon_richer_than_oxygen_is_accepted() {
    let network = network();
    let g = gas(1.0, Some(1.5), Some(&CHON));
    let got = solve_at(&network, &g, 1_500.0, 1.0);
    assert!(share(&got, row("CO"), Element::O) > 0.95);
    assert!(share(&got, row("H2O"), Element::O) < 0.05);
    let excess: f64 = ["CH4", "C2H2", "HCN"]
        .iter()
        .map(|k| share(&got, row(k), Element::C))
        .sum();
    assert!((excess - 1.0 / 3.0).abs() < 0.05, "{excess}");
}

/// The bound of hydrogen dominance, where P14.T50.b's E(M) puts it: at protosolar ratios, an
/// envelope of 4.5 M⊕ (E about 176) is solved and one of 3.5 M⊕ (E about 226), whose all-in
/// hydrides would take more than half its hydrogen, is refused with its share; the bound's E is
/// about 194, near 4.1 M⊕.
#[test]
fn a_gas_beyond_hydrogen_dominance_is_refused() {
    use crate::planetary::chem::envelope_abundances::mass_enrichment;
    use crate::units::EarthMasses;

    let network = network();
    let rich = gas(mass_enrichment(EarthMasses::new(4.5)), None, None);
    network
        .solve(rich, Kelvin::new(1_000.0), Pascals::new(1e5))
        .unwrap_or_else(|e| panic!("4.5 M⊕: {e}"));
    let mut per = [0.0; Element::ALL.len()];
    for (e, n) in gas(1.0, None, None) {
        per[element_index(e)] = n;
    }
    let bound = HYDROGEN_DOMINANCE_BOUND / Gas::new(&network, &per).hydride_share;
    assert!((bound - 194.0).abs() < 1.0, "{bound}");
    let richer = gas(mass_enrichment(EarthMasses::new(3.5)), None, None);
    match network.solve(richer, Kelvin::new(1_000.0), Pascals::new(1e5)) {
        Err(SolveEquilibriumError::NotHydrogenDominated { share }) => {
            assert!(share > HYDROGEN_DOMINANCE_BOUND && share < 1.0, "{share}");
        }
        other => panic!("{other:?}"),
    }
}

/// Each refusal by its variant.
#[test]
fn each_refusal_names_its_cause() {
    let network = network();
    let g = || gas(1.0, None, None);
    let t = Kelvin::new(1_000.0);
    let p = Pascals::new(1e5);
    for bad in [f64::NAN, 0.0, -1.0, f64::INFINITY] {
        assert_eq!(
            network
                .solve(g(), Kelvin::new(bad), p)
                .map(|_| ())
                .unwrap_err()
                .to_string(),
            SolveEquilibriumError::Temperature(Kelvin::new(bad)).to_string()
        );
    }
    let hot = Kelvin::new(6_000.000_001);
    assert_eq!(
        network.solve(g(), hot, p),
        Err(SolveEquilibriumError::AboveRecords(hot))
    );
    network
        .solve(g(), Kelvin::new(6_000.0), p)
        .unwrap_or_else(|e| panic!("6,000 K: {e}"));
    for bad in [0.0, -1.0, f64::INFINITY] {
        assert_eq!(
            network.solve(g(), t, Pascals::new(bad)),
            Err(SolveEquilibriumError::Pressure(Pascals::new(bad)))
        );
    }
    assert!(matches!(
        network.solve(g(), t, Pascals::new(f64::NAN)),
        Err(SolveEquilibriumError::Pressure(_))
    ));
    let mut negative = g();
    negative[3].1 = -1e-9;
    assert_eq!(
        network.solve(negative, t, p),
        Err(SolveEquilibriumError::Abundance {
            element: Element::N,
            value: -1e-9
        })
    );
    let mut twice = g();
    twice.push((Element::O, 1e-4));
    assert_eq!(
        network.solve(twice, t, p),
        Err(SolveEquilibriumError::Repeated(Element::O))
    );
    let without: Vec<(Element, f64)> = g().into_iter().filter(|(e, _)| *e != Element::H).collect();
    assert_eq!(
        network.solve(without, t, p),
        Err(SolveEquilibriumError::NoHydrogen)
    );
    // Only relative abundances matter: hydrogen's own value normalises the rest.
    let doubled: Vec<(Element, f64)> = g().into_iter().map(|(e, n)| (e, 2.0 * n)).collect();
    let (one, two) = (
        network.solve(g(), t, p).unwrap(),
        network.solve(doubled, t, p).unwrap(),
    );
    for ((a, x), (b, y)) in one.mole_fractions().zip(two.mole_fractions()) {
        assert_eq!(a, b);
        assert!((x - y).abs() <= 1e-12 * x.abs().max(1e-300));
    }
}

/// An element the gas does not hold leaves its species at zero, and the rest solved.
#[test]
fn an_absent_element_leaves_its_species_at_zero() {
    let network = network();
    let hydrogen_alone = [(Element::H, 1.0)];
    let got = solve_at(&network, &hydrogen_alone, 3_000.0, 0.01);
    for (id, x) in got.mole_fractions() {
        let key = id.substance().key();
        match key.as_str() {
            "H2" | "H" => assert!(x > 0.0, "{key}"),
            _ => assert_same_bits(x, 0.0),
        }
    }
    let carbon_free = gas(
        1.0,
        None,
        Some(&[Element::H, Element::He, Element::O, Element::Na]),
    );
    let got = solve_at(&network, &carbon_free, 2_500.0, 1.0);
    assert_same_bits(got.mole_fraction(row("CO")).unwrap(), 0.0);
    assert!(got.mole_fraction(row("Na+")).unwrap() > 0.0);
    assert!(got.charge_imbalance().abs() < 1e-12);
}

/// The composition is continuous in temperature: from 200 to 6,000 K in steps of 10 K, across the
/// floor, at 1 bar and at 10 mbar, no species' share of its element moves by more than 0.25 in a
/// step.
#[test]
fn the_composition_is_continuous_in_temperature() {
    let network = network();
    let g = gas(1.0, None, None);
    for bar in [1.0, 0.01] {
        let mut previous: Option<Vec<f64>> = None;
        let mut t = 200.0;
        while t <= 6_000.0 {
            let got = solve_at(&network, &g, t, bar);
            let shares: Vec<f64> = got
                .mole_fractions()
                .map(|(id, _)| {
                    id.substance()
                        .elements()
                        .iter()
                        .map(|&(e, _)| share(&got, id, e))
                        .fold(0.0, f64::max)
                })
                .collect();
            if let Some(before) = &previous {
                for ((id, _), (a, b)) in got.mole_fractions().zip(before.iter().zip(&shares)) {
                    assert!(
                        (a - b).abs() < 0.25,
                        "{} at {t} K, {bar} bar",
                        id.substance().key()
                    );
                }
            }
            previous = Some(shares);
            t += 10.0;
        }
    }
}

/// The same inputs give the same bits.
#[test]
fn the_solution_is_deterministic() {
    let (first, second) = (network(), network());
    let rich = gas(3.0, Some(0.8), None);
    for (t, bar) in [(400.0, 10.0), (1_200.0, 1.0), (4_600.0, 0.05)] {
        let once = solve_at(&first, &rich, t, bar);
        let again = solve_at(&second, &rich, t, bar);
        for ((id, x), (other, y)) in once.mole_fractions().zip(again.mole_fractions()) {
            assert_eq!(id, other);
            assert_same_bits(x, y);
        }
    }
}

/// A gas at one level: its abundances, its temperature, K, and its pressure, bar.
type Level = (Vec<(Element, f64)>, f64, f64);

/// One `Network`, reused across gases and levels in any order, gives each the same bits as
/// alone (the sim-determinism skill's order independence): the network holds nothing from a
/// solution to the next.
#[test]
fn a_shared_network_is_order_independent() {
    let network = network();
    let cases: Vec<Level> = vec![
        (gas(1.0, None, None), 1_000.0, 1.0),
        (gas(1.0, Some(1.5), None), 2_000.0, 0.01),
        (gas(1.0, None, Some(&CHON)), 1_500.0, 1.0),
        (vec![(Element::H, 1.0)], 3_000.0, 0.01),
        (gas(1.0, None, None), 166.0, 1.0),
        (gas(30.0, None, None), 4_600.0, 0.05),
    ];
    assert_order_independent(&cases, |(g, t, bar)| {
        format!(
            "{:?}",
            network.solve(g.iter().copied(), Kelvin::new(*t), Pascals::new(bar * 1e5))
        )
    });
}

/// The solution's early stop gives the fixed count's bits: at the golden's levels, and where an
/// unknown is inactive, the steps run to [`NEWTON_STEPS`] land on the same unknowns as those that
/// stop at the first refused step.
#[test]
fn stopping_at_the_fixed_point_gives_the_fixed_count_s_bits() {
    let network = network();
    let levels: [Level; 6] = [
        (gas(1.0, None, None), 1_000.0, 1.0),
        (gas(1.0, None, None), 4_600.0, 0.05),
        (gas(100.0, None, None), 800.0, 10.0),
        (gas(1.0, Some(1.5), None), 2_000.0, 0.01),
        (gas(1.0, None, Some(&CHON)), 1_500.0, 1.0),
        (vec![(Element::H, 1.0)], 3_000.0, 0.01),
    ];
    for (g, t, bar) in &levels {
        let mut per_hydrogen = [0.0; Element::ALL.len()];
        for &(e, n) in g {
            per_hydrogen[element_index(e)] = n;
        }
        let state_gas = Gas::new(&network, &per_hydrogen);
        let log_k: Vec<f64> = network
            .species
            .iter()
            .map(|s| {
                s.formation.as_ref().map_or(0.0, |(reaction, per)| {
                    reaction
                        .log10_equilibrium_constant(Kelvin::new(*t))
                        .expect("within the records")
                        * core::f64::consts::LN_10
                        * per
                })
            })
            .collect();
        let state = State::new(&network, &state_gas, &log_k, *bar);
        let (early, _) = state.refine(state.start(), Stop::AtFixedPoint);
        let (full, _) = state.refine(state.start(), Stop::FullCount);
        for (a, b) in early.iter().zip(&full) {
            assert_same_bits(*a, *b);
        }
    }
}

/// `Network::registry`'s refusals, each by its variant, through the helpers it builds with.
#[test]
fn a_network_that_cannot_be_built_is_refused() {
    assert_eq!(
        trace_elements(&[row("Na+")]).map(|_| ()),
        Err(BuildNetworkError::NoNeutralSpecies {
            element: Element::Na
        })
    );
    assert_eq!(
        common_range(&[row("FeH")]),
        Err(BuildNetworkError::NoCommonRange)
    );
    let rows = [row("NaCl")];
    let traces = trace_elements(&rows).expect("NaCl is neutral");
    assert_eq!(
        species_of(0, &rows, &[0; CHARGE + 1], &traces).map(|_| ()),
        Err(BuildNetworkError::TwoTraceAtoms {
            key: row("NaCl").substance().key()
        })
    );
}

/// The species' Hill formula (C first, then H, then the rest alphabetically; with no carbon every
/// element alphabetically, H among them), as `NotCarried` names them.
fn hill(id: SubstanceId) -> String {
    let elements = id.substance().elements();
    let mut symbols: Vec<(&str, u16)> = elements.iter().map(|&(e, n)| (e.symbol(), n)).collect();
    let carbon = symbols.iter().any(|&(s, _)| s == "C");
    symbols.sort_by(|a, b| {
        let rank = |s: &str| match (carbon, s) {
            (true, "C") => 0,
            (true, "H") => 1,
            _ => 2,
        };
        rank(a.0).cmp(&rank(b.0)).then(a.0.cmp(b.0))
    });
    symbols
        .iter()
        .map(|&(s, n)| {
            if n > 1 {
                format!("{s}{n}")
            } else {
                s.to_owned()
            }
        })
        .collect()
}

/// P14.T52.e's tripwire: no `NOT_CARRIED` species is a registry gas row, and the set is sorted by
/// formula, charge and isomer, and unique. As a row joins the network its entry must go.
#[test]
fn no_species_not_carried_is_a_registry_gas_row() {
    let rows: Vec<(String, i8)> = crate::substance::ids()
        .filter(|id| id.substance().gas().is_some())
        .map(|id| (hill(id), id.substance().charge()))
        .collect();
    assert_eq!(hill(row("CH3OH")), "CH4O");
    assert_eq!(hill(row("NH3")), "H3N");
    for entry in &NOT_CARRIED {
        assert!(
            !rows.contains(&(entry.formula.to_owned(), entry.charge)),
            "{} ({}) is a registry gas row",
            entry.formula,
            entry.charge
        );
        assert!(entry.largest_share >= 0.01 && entry.largest_share <= 1.0);
    }
    for pair in NOT_CARRIED.windows(2) {
        let key = |e: &NotCarried| (e.formula, e.charge, e.isomer);
        assert!(key(&pair[0]) < key(&pair[1]), "{:?}", pair[1]);
    }
}

/// A pinned gas: its name, its abundances and its levels, each a temperature, K, and a pressure,
/// bar.
type PinnedGas = (&'static str, Vec<(Element, f64)>, &'static [(f64, f64)]);

/// The speciation pinned bit for bit at the header in force, so that the native and WebAssembly
/// targets are compared on it before P14.T54.b first reads it (the sim-determinism skill): a
/// protosolar gas over the levels a hot and a cold giant's T52.d evaluates (the coldest held at
/// the floor), a metal-rich and a carbon-rich one, and gases without some elements, whose unknowns
/// are inactive (C, H, O, N and He alone, with no charge solved; no carbon or nitrogen; hydrogen
/// alone).
/// Nothing on the record calls the speciation before P14.T54.b, so this golden is re-blessed with
/// the rules it pins, at the version in force (22, then 23), until then.
#[test]
fn the_speciation_is_pinned() {
    use crate::GENERATOR_VERSION;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let network = network();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let cases: [PinnedGas; 7] = [
        (
            "protosolar",
            gas(1.0, None, None),
            &[
                (166.0, 1.0),
                (300.0, 1.0),
                (700.0, 100.0),
                (1_000.0, 1.0),
                (1_500.0, 0.01),
                (2_000.0, 1.0),
                (3_000.0, 0.01),
                (4_600.0, 0.05),
                (6_000.0, 1e-4),
            ],
        ),
        (
            "a hundred times protosolar",
            gas(100.0, None, None),
            &[(800.0, 10.0), (1_200.0, 0.1), (2_500.0, 1.0)],
        ),
        (
            "C/O 1.5",
            gas(1.0, Some(1.5), None),
            &[(1_000.0, 1.0), (2_000.0, 0.01)],
        ),
        (
            "a hundredth of protosolar",
            gas(0.01, None, None),
            &[(1_500.0, 1e-6), (3_500.0, 1e3)],
        ),
        (
            "Heng and Tsai's elements",
            gas(1.0, None, Some(&CHON)),
            &[(1_500.0, 1.0)],
        ),
        (
            "no carbon or nitrogen",
            gas(
                1.0,
                None,
                Some(&[Element::H, Element::He, Element::O, Element::Na]),
            ),
            &[(2_500.0, 1.0)],
        ),
        (
            "hydrogen alone",
            vec![(Element::H, 1.0)],
            &[(3_000.0, 0.01)],
        ),
    ];
    for (name, g, levels) in &cases {
        for &(t, bar) in *levels {
            let got = solve_at(&network, g, t, bar);
            w.line(&format!(
                "{name} at {t} K and {bar} bar: {:?}",
                got.regime()
            ));
            for (id, x) in got.mole_fractions() {
                w.f64(
                    &format!("{name} {t} K {bar} bar {}", id.substance().key().as_str()),
                    x,
                );
            }
        }
    }
    golden!("planetary/equilibrium", w.as_str());
}

/// The start takes each major element's atom at the start's p(H₂) (`State::atom_ratio`, and
/// `pair_root`'s linear share): `pair_root` solves total = linear a + 2 κ a², and gives total ÷
/// linear at κ = 0. Where the atoms hold the sulphur, the nitrogen and most of the carbon (3,000 K
/// at 10 mbar; the KELT-9b-like dayside at 4,600 K, 10 and 50 mbar; 5,750 K at 10 mbar, where
/// the start without the atoms stalled at every metallicity; 6,000 K at 10⁻⁴ bar), the start's
/// ln p of H₂, CH₄, NH₃, H₂O and H₂S and of the hydrogen nuclei lies within 0.25 of the
/// solution's (0.12 at most, CH₄ and H₂O at 3,000 K), where H₂S taken all-in lay 7 to 31 off.
#[test]
fn the_start_takes_the_major_elements_atoms() {
    for (total, linear, kappa) in [
        (1e-3, 1.0, 0.0),
        (1e-3, 50.0, 0.0),
        (2e-4, 3.0, 1e6),
        (1e-5, 1e8, 1e12),
    ] {
        let a = pair_root(total, linear, kappa);
        let back = linear * a + 2.0 * kappa * a * a;
        assert!(
            (back / total - 1.0).abs() < 1e-12,
            "{total} {linear} {kappa}: {back}"
        );
        if kappa == 0.0 {
            assert!((a * linear / total - 1.0).abs() < 1e-15, "{linear}");
        }
    }
    let network = network();
    let mut per_hydrogen = [0.0; Element::ALL.len()];
    for (e, x) in gas(1.0, None, None) {
        per_hydrogen[element_index(e)] = x;
    }
    let gas = Gas::new(&network, &per_hydrogen);
    for (t, bar) in [
        (3_000.0, 0.01),
        (4_600.0, 0.01),
        (4_600.0, 0.05),
        (5_750.0, 0.01),
        (6_000.0, 1e-4),
    ] {
        let log_k: Vec<f64> = network
            .species
            .iter()
            .map(|s| {
                s.formation.as_ref().map_or(0.0, |(r, per)| {
                    r.log10_equilibrium_constant(Kelvin::new(t)).unwrap()
                        * core::f64::consts::LN_10
                        * per
                })
            })
            .collect();
        let state = State::new(&network, &gas, &log_k, bar);
        let start = state.start();
        let (solved, _) = state.refine(start, Stop::AtFixedPoint);
        for k in [0, 1, 2, 3, 4, NUCLEI] {
            assert!(
                (start[k] - solved[k]).abs() < 0.25,
                "{t} K, {bar} bar: unknown {k} starts at {} against {}",
                start[k],
                solved[k]
            );
        }
        let all_in = math::ln(gas.majors[4]) + start[NUCLEI];
        assert!(all_in - solved[4] > 5.0, "{t} K, {bar} bar");
    }
}
