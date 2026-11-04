//! The network under generator version 27's registry (P14.T52.e): its further carriers, SH, OH,
//! S⁺, C⁺, H⁺, N⁺, O⁺, P, P₂, PO, PH₂ and Cl, P and Cl solved, with the plan's tests.
//!
//! The FastChem values are FastChem 4.0.3's, run by hand outside the repository as the asserted
//! cases' ([`Case`]) were, as pyfastchem 4.0.3 from `PyPI` built under `CPython` 3.12, its data
//! at the repository's tag v4.0.3 (ae67cbd559bc64a3233a1cee6030b8e6b50520de), `accuracyChem`
//! 10⁻⁸, the abundances this registry's protosolar ones (Lodders 2010), reproducing T52.a's values
//! to 3 × 10⁻⁷. Three networks: FastChem's full `logK.dat` ("full"); `logK.dat` cut to 27's
//! network's species by the lane's scratch script, every element but Ni, P and Cl ("matched", as
//! [`NETWORK_MATCHED`]); and each with its equilibrium condensation over
//! `logK_condensates.dat` (`FastChem(abundances, logK, logK_condensates, 0)`,
//! `equilibrium_condensation` on), the plan's rerun with condensation (decision-p14-t52a-rows).
//! Every point's flag was `FASTCHEM_SUCCESS`.

use super::*;
use crate::planetary::testing::with_every_27_hold;

/// P14.T52.e's carriers, rows 115 on.
const CARRIERS: [&str; 12] = [
    "SH", "OH", "S+", "C+", "H+", "N+", "O+", "P", "P2", "PO", "PH2", "Cl",
];

/// Under 27's registry the network carries every carrier as a species of its solved elements
/// (each holds only H, C, N, O, S, P and Cl), over the same 300–6,000 K, its last species.
#[test]
fn under_27_the_network_carries_the_carriers() {
    with_every_27_hold(|| {
        let network = network();
        assert!((network.floor().value() - 300.0).abs() < 1e-9);
        assert!((network.ceiling().value() - 6_000.0).abs() < 1e-9);
        let carried: Vec<SubstanceId> = network.species().collect();
        let last = carried.len() - CARRIERS.len();
        let keys: Vec<String> = carried[last..]
            .iter()
            .map(|id| id.substance().key().as_str().to_owned())
            .collect();
        assert_eq!(keys, CARRIERS);
        for key in CARRIERS {
            let place = carried
                .iter()
                .position(|&id| id == row(key))
                .unwrap_or_else(|| panic!("{key} is carried"));
            assert_eq!(network.species[place].trace, None, "{key}");
        }
        assert!(network.major_cations);
    });
}

/// Under 27 P and Cl are solved elements, each with its unknown and its basis, PH₃ and HCl, and no
/// longer trace elements; a row holding two atoms of trace elements is refused until both are
/// solved: NaCl, Na and Cl, refused under 26's solved elements and past the trace check under
/// 27's.
#[test]
fn under_27_phosphorus_and_chlorine_are_solved() {
    with_every_27_hold(|| {
        let network = network();
        assert_eq!(network.solved, SOLVED);
        for (k, key) in [
            (MAJOR_ELEMENTS.len(), "PH3"),
            (MAJOR_ELEMENTS.len() + 1, "HCl"),
        ] {
            let basis = network.basis[unknown_of(k)].expect("solved");
            assert_eq!(network.species[basis].id, row(key));
        }
        assert!(
            network
                .traces
                .iter()
                .all(|t| !matches!(t.element, Element::P | Element::Cl))
        );
        let rows = [row("NaCl")];
        let held = trace_elements(&rows, MAJOR_ELEMENTS.len()).unwrap();
        let basis = [Some(0); UNKNOWNS];
        assert_eq!(
            species_of(0, &rows, &basis, MAJOR_ELEMENTS.len(), &held).map(|_| ()),
            Err(BuildNetworkError::TwoTraceAtoms {
                key: row("NaCl").substance().key()
            })
        );
        let carried = trace_elements(&rows, SOLVED).unwrap();
        assert!(!matches!(
            species_of(0, &rows, &basis, SOLVED, &carried),
            Err(BuildNetworkError::TwoTraceAtoms { .. })
        ));
    });
}

/// A gas row holding two atoms of a trace element is refused until the element is solved: 27's
/// rows, P₂ among them, with 26's solved elements are refused at P₂, and with 27's they build.
#[test]
fn under_27_diphosphorus_is_refused_until_phosphorus_is_solved() {
    with_every_27_hold(|| {
        let rows: Vec<SubstanceId> = crate::substance::ids()
            .filter(|&id| crate::substance::formation_range(id).is_some())
            .collect();
        assert_eq!(
            Network::of_rows(&rows, MAJOR_ELEMENTS.len()).map(|_| ()),
            Err(BuildNetworkError::TwoTraceAtoms {
                key: row("P2").substance().key()
            })
        );
        assert!(Network::of_rows(&rows, SOLVED).is_ok());
    });
}

/// SH holds the sulphur in its field between Visscher, Lodders and Fegley's (2006, §3.1) eq. 7,
/// where log p > 4.80 − 14,522 ÷ T (S beyond it), and eq. 4, where log p < 6.82 − 17,570 ÷ T
/// (H₂S beyond it): at 3,000 K and 3 bar, 3,500 K and 17 bar and 4,000 K and 63 bar, each near the
/// field's middle in log p, SH is the largest sulphur carrier, and its share is FastChem's within
/// 2% of the matched network's (0.4625, 0.4781 and 0.4883; here +1.0%, +0.7% and +0.4%) and 5% of
/// the full network's (0.4490, 0.4676 and 0.4815; +4.0% at most), whose `SiS`, CS and SO take
/// 1.4–2.4% of the sulphur here. Nothing condenses at these points, so the condensing runs are the
/// same.
#[test]
fn under_27_sh_holds_its_field_between_vlf2006_s_eqs_4_and_7() {
    with_every_27_hold(|| {
        let network = network();
        let g = gas(1.0, None, None);
        for (t, bar, matched, full) in [
            (3_000.0_f64, 3.0_f64, 0.462_507_6, 0.448_993_9),
            (3_500.0, 17.0, 0.478_111_8, 0.467_633_8),
            (4_000.0, 63.0, 0.488_328_2, 0.481_513_3),
        ] {
            let log_p = math::log10(bar);
            assert!(
                log_p > 4.80 - 14_522.0 / t && log_p < 6.82 - 17_570.0 / t,
                "{t} K, {bar} bar lies outside SH's field"
            );
            let got = solve_at(&network, &g, t, bar);
            let mercapto = share(&got, row("SH"), Element::S);
            for other in ["H2S", "S", "S+", "SO2"] {
                assert!(
                    mercapto > share(&got, row(other), Element::S),
                    "{t} K, {bar} bar: SH {mercapto} against {other}"
                );
            }
            assert!(
                (mercapto / matched - 1.0).abs() < 0.02,
                "{t} K: SH {mercapto}"
            );
            assert!((mercapto / full - 1.0).abs() < 0.05, "{t} K: SH {mercapto}");
        }
    });
}

/// OH's share of the oxygen at 3,000 K and 10 mbar: 0.1269 here, against FastChem's 0.1207
/// (matched) and 0.1207 (full), within 6%. The miss is the two thermochemistries': CEA's OH
/// record takes Ruscic et al.'s 2002 D₀(H–OH) (its comment), FastChem's a fit to NIST-JANAF (Chase
/// 1998), and the matched run, the same species and abundances, misses by the same +5.1%; O is
/// −2.1% and H₂O −0.4% beside it. Nothing condenses there.
#[test]
fn under_27_oh_holds_its_share_at_3000_k_and_10_mbar() {
    with_every_27_hold(|| {
        let got = solve_at(&network(), &gas(1.0, None, None), 3_000.0, 0.01);
        let hydroxyl = share(&got, row("OH"), Element::O);
        for want in [0.120_710_3, 0.120_653_7] {
            assert!(
                (hydroxyl / want - 1.0).abs() < 0.06,
                "OH {hydroxyl} against FastChem's {want}"
            );
        }
        assert!(hydroxyl > share(&got, row("H2O"), Element::O));
    });
}

/// The electrons at 6,000 K and 10 mbar, where H⁺ gives most of them: 6.329 × 10⁻⁴ here, against
/// FastChem's 6.316 × 10⁻⁴ (matched) and 6.326 × 10⁻⁴ (full), within 1% (+0.2% and +0.04%); 26's
/// network, without H⁺, C⁺ or S⁺, was about 6× low. H⁺ gives over three quarters of them
/// (FastChem's full run: 0.80).
#[test]
fn under_27_the_electrons_at_6000_k_and_10_mbar_are_the_run_s() {
    let g = gas(1.0, None, None);
    let electron = row("e-");
    with_every_27_hold(|| {
        let got = solve_at(&network(), &g, 6_000.0, 0.01);
        let electrons = got.mole_fraction(electron).unwrap();
        for want in [6.315_879e-4, 6.326_151e-4] {
            assert!(
                (electrons / want - 1.0).abs() < 0.01,
                "e⁻ {electrons:e} against FastChem's {want:e}"
            );
        }
        assert!(got.mole_fraction(row("H+")).unwrap() > 0.75 * electrons);
    });
}

/// The asserted cases rerun with condensation (decision-p14-t52a-rows), under 27's network: each
/// case's FastChem values are the condensing run's, the matched cases' with 27's species. A
/// species whose condensing value lies more than 0.5% from the same network's gas-only run is
/// left out: its element condenses, or the gas the condensates take renormalises it, and the gas
/// network does not condense, P14.T52.c's cold traps removing each condensate's element above
/// its base. Nothing condenses in Heng and Tsai's cases or at 2,500 K and above, so their values
/// are the gas-only runs' to the digits. Below, FastChem condenses the refractories, and with them
/// 7–21% of the oxygen (silicates) and 2–67% of the sulphur (FeS): at 1,000 K and 1 bar only H₂,
/// He, NH₃, N₂ and Zn stay (CH₄ +0.6%, as CO goes to the silicates' oxygen), and at a hundred
/// times protosolar, 1,000 K and 10 mbar none, H₂ itself moving +3.9%, so that case is left out.
/// Beside 26's values, 27's move H₂S by −0.4% to −7.4% (SH takes its share), and nothing else by
/// more than 0.8%.
#[test]
fn under_27_every_asserted_case_is_within_its_tolerance() {
    with_every_27_hold(|| every_case_is_within_its_tolerance(&CASES_27));
}

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
///
/// Each rerun under 27's network with condensation, the species condensation moves left out
/// ([`under_27_every_asserted_case_is_within_its_tolerance`]).
const CASES_27: [Case; 11] = [
    Case {
        label: "Heng and Tsai, protosolar, 1,100 K, 1 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &CHON,
        temperature: 1100.0,
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
        temperature: 2000.0,
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
        temperature: 1500.0,
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
        temperature: 1100.0,
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
        temperature: 1000.0,
        bar: 1.0,
        species: &[
            ("H2", 8.361_815e-1, 0.01),
            ("He", 1.622_617e-1, 0.01),
            ("NH3", 3.681_593e-6, 0.05),
            ("N2", 6.626_518e-5, 0.01),
            ("Zn", 8.398_776e-8, 0.01),
        ],
    },
    Case {
        label: "network-matched, protosolar, 1,500 K, 10 mbar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 1500.0,
        bar: 0.01,
        species: &[
            ("H2", 8.364_371e-1, 0.01),
            ("N2", 6.802_982e-5, 0.01),
            ("CO", 4.609_517e-4, 0.01),
            ("H2S", 2.617_312e-5, 0.01),
            ("K", 2.406_336e-7, 0.01),
        ],
    },
    Case {
        label: "network-matched, C/O 1.5, 1,500 K, 1 bar",
        metallicity: 1.0,
        c_to_o: Some(1.5),
        elements: &NETWORK_MATCHED,
        temperature: 1500.0,
        bar: 1.0,
        species: &[("H2", 8.359_700e-1, 0.01), ("N2", 6.656_124e-5, 0.01)],
    },
    Case {
        label: "network-matched, protosolar, 700 K, 100 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 700.0,
        bar: 100.0,
        species: &[
            ("H2", 8.360_735e-1, 0.01),
            ("CH4", 4.616_223e-4, 0.01),
            ("NH3", 1.361_610e-4, 0.01),
        ],
    },
    Case {
        label: "network-matched, protosolar, 2,500 K, 100 bar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 2500.0,
        bar: 100.0,
        species: &[
            ("H2", 8.343_250e-1, 0.01),
            ("H2O", 4.979_818e-4, 0.02),
            ("CH4", 1.537_978e-5, 0.3),
            ("NH3", 6.497_318e-6, 0.05),
            ("N2", 6.424_590e-5, 0.01),
            ("CO", 4.441_151e-4, 0.02),
            ("H2S", 2.504_105e-5, 0.01),
            ("K", 2.245_195e-7, 0.01),
            ("Ti", 3.502_686e-8, 0.25),
            ("TiO", 1.246_552e-7, 0.05),
            ("VO", 3.702_462e-9, 0.8),
            ("V", 1.463_148e-8, 0.25),
            ("K+", 1.716_929e-8, 0.01),
        ],
    },
    Case {
        label: "network-matched, protosolar, 4,600 K, 10 mbar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 4600.0,
        bar: 0.01,
        species: &[
            ("H", 9.101_251e-1, 0.01),
            ("He", 8.822_766e-2, 0.01),
            ("O", 4.699_988e-4, 0.01),
            ("C", 1.723_733e-4, 0.01),
            ("CO", 7.852_669e-5, 0.01),
            ("N", 7.330_709e-5, 0.01),
            ("N2", 3.782_234e-7, 0.01),
            ("S", 1.473_677e-5, 0.01),
            ("Mg+", 3.288_337e-5, 0.01),
            ("Mg", 3.391_410e-6, 0.02),
            ("Si", 1.932_590e-5, 0.01),
            ("Si+", 1.610_938e-5, 0.02),
            ("Fe+", 2.403_821e-5, 0.05),
            ("Fe", 5.447_033e-6, 0.1),
            ("Ca+", 2.131_284e-6, 0.01),
            ("Na+", 2.038_374e-6, 0.01),
            ("e-", 8.183_072e-5, 0.02),
            ("H-", 2.610_177e-9, 0.02),
        ],
    },
    Case {
        label: "network-matched, protosolar, 4,600 K, 50 mbar",
        metallicity: 1.0,
        c_to_o: None,
        elements: &NETWORK_MATCHED,
        temperature: 4600.0,
        bar: 0.05,
        species: &[
            ("H", 9.079_415e-1, 0.01),
            ("He", 8.842_250e-2, 0.01),
            ("O", 3.844_524e-4, 0.01),
            ("CO", 1.636_652e-4, 0.01),
            ("C", 8.784_012e-5, 0.01),
            ("N", 7.070_800e-5, 0.01),
            ("N2", 1.759_396e-6, 0.01),
            ("S", 1.477_579e-5, 0.01),
            ("Mg+", 2.664_410e-5, 0.01),
            ("Mg", 9.710_789e-6, 0.02),
            ("Si", 2.867_894e-5, 0.01),
            ("Si+", 6.764_766e-6, 0.02),
            ("Fe+", 1.640_986e-5, 0.05),
            ("Fe", 1.314_051e-5, 0.1),
            ("Ca+", 2.124_064e-6, 0.01),
            ("Na+", 2.039_058e-6, 0.01),
            ("e-", 5.783_569e-5, 0.02),
            ("H-", 9.201_876e-9, 0.02),
        ],
    },
];

/// Phosphorus at a hot Jupiter's photosphere, carried as FastChem's run carries it under 27's
/// network (P, P₂, PO, PH₂ and PH₃), against the matched run with condensation, P and Cl among its
/// elements (every element but Ni):
///
/// - **1,500 K and 10 mbar** (HD 209458 b's photosphere): P₂ the largest carrier, as in FastChem's
///   full network (0.718, beside PO 0.093, PH₂ 0.072 and PS 0.067); the matched run's P₂ 0.7990,
///   the P atom 0.02443 and PH₃ 0.00458 within 5% (here +3.5%, +2.7% and +2.6%), and PO's 0.0965
///   within 20% (+16.5%: the silicates FastChem condenses there hold 6.7% of the oxygen, which this
///   gas network keeps; against its gas-only run's 0.1074, +4.7%).
/// - **2,000 K and 10 mbar**: the P atom the largest, then PO, as in the full run (0.517 and 0.332,
///   PH 0.068); the matched run's P 0.5823 and PO 0.3729 within 2% (+1.0% and +1.6%) and P₂'s
///   0.0213 within 5% (+1.0%). Nothing condenses there.
///
/// At both, PH₂ is 0.4–0.5 of the run's (0.0306 against 0.0755, 0.0116 against 0.0233): CEA's PH₂
/// record is Gurvich 1989's, FastChem's a fit to NIST-JANAF (Chase 1998), and their K differ by
/// that much; PH₂ holds under 8% of the phosphorus at either, so its share is held within a factor
/// of 3.
#[test]
fn under_27_phosphorus_at_a_hot_jupiter_photosphere_is_the_run_s() {
    with_every_27_hold(|| {
        let network = network();
        let g = gas(1.0, None, None);
        let photosphere = solve_at(&network, &g, 1_500.0, 0.01);
        let p = |got: &GasEquilibrium, key: &str| share(got, row(key), Element::P);
        for other in ["PO", "PH2", "PH3", "P"] {
            assert!(
                p(&photosphere, "P2") > p(&photosphere, other),
                "P2 against {other}"
            );
        }
        for (key, want, tolerance) in [
            ("P2", 0.799_044, 0.05),
            ("P", 0.024_43, 0.05),
            ("PH3", 0.004_58, 0.05),
            ("PO", 0.096_472, 0.2),
        ] {
            let have = p(&photosphere, key);
            assert!(
                (have / want - 1.0).abs() < tolerance,
                "1,500 K: {key} {have} against FastChem's {want}"
            );
        }
        let hotter = solve_at(&network, &g, 2_000.0, 0.01);
        for other in ["PO", "PH2", "PH3", "P2"] {
            assert!(p(&hotter, "P") > p(&hotter, other), "P against {other}");
        }
        assert!(p(&hotter, "PO") > p(&hotter, "P2"));
        for (key, want, tolerance) in [
            ("P", 0.582_342, 0.02),
            ("PO", 0.372_926, 0.02),
            ("P2", 0.021_296, 0.05),
        ] {
            let have = p(&hotter, key);
            assert!(
                (have / want - 1.0).abs() < tolerance,
                "2,000 K: {key} {have} against FastChem's {want}"
            );
        }
        for (got, want) in [(&photosphere, 0.075_474), (&hotter, 0.023_259)] {
            let ratio = p(got, "PH2") / want;
            assert!(
                (1.0 / 3.0..3.0).contains(&ratio),
                "PH2 at {ratio} of the run's"
            );
        }
    });
}

/// Hydrogen alone keeps H⁺ and its electrons under 27's network, and H⁻ with them; every other
/// species is zero.
#[test]
fn under_27_an_absent_element_leaves_its_species_at_zero() {
    with_every_27_hold(|| {
        let got = solve_at(&network(), &[(Element::H, 1.0)], 3_000.0, 0.01);
        for (id, x) in got.mole_fractions() {
            let key = id.substance().key();
            match key.as_str() {
                "H2" | "H" | "H+" | "e-" | "H-" => assert!(x > 0.0, "{key}"),
                _ => assert_same_bits(x, 0.0),
            }
        }
        assert!(got.charge_imbalance().abs() < 1e-12);
    });
}

/// Under 27's network the start's ln p of H₂, CH₄, NH₃, H₂O and H₂S and of the hydrogen nuclei
/// lies within 0.25 of the solution's at 26's levels (0.12 at most, CH₄ and H₂O at 3,000 K, with
/// OH and SH in the start beside the O and S atoms), but at 6,000 K and 10⁻⁴ bar, where C⁺ and S⁺
/// hold 42% and 75% of their elements and the start's closed forms, which precede the
/// electrons', take none: CH₄ 0.60 and H₂S 1.42 off there, within 1.5. PH₃'s and HCl's lie within
/// 0.05 (0.045 at most, PH₃ at 6,000 K), within 0.25; the electrons' within 2.3 (2.30 at 5,750 K),
/// within 2.5, where 26's network's start misses them by 1.3–6.1 at the same levels.
#[test]
fn under_27_the_start_takes_the_major_elements_atoms() {
    with_every_27_hold(|| {
        let network = network();
        let mut per_hydrogen = [0.0; Element::ALL.len()];
        for (e, x) in gas(1.0, None, None) {
            per_hydrogen[element_index(e)] = x;
        }
        let gas = Gas::new(&network, &per_hydrogen);
        for (t, bar, bound) in [
            (3_000.0, 0.01, 0.25),
            (4_600.0, 0.01, 0.25),
            (4_600.0, 0.05, 0.25),
            (5_750.0, 0.01, 0.25),
            (6_000.0, 1e-4, 1.5),
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
            for (k, within) in [
                (0, bound),
                (1, bound),
                (2, bound),
                (3, bound),
                (4, bound),
                (NUCLEI, bound),
                (unknown_of(MAJOR_ELEMENTS.len()), 0.25),
                (unknown_of(MAJOR_ELEMENTS.len() + 1), 0.25),
                (CHARGE, 2.5),
            ] {
                assert!(
                    (start[k] - solved[k]).abs() < within,
                    "{t} K, {bar} bar: unknown {k} starts at {} against {}",
                    start[k],
                    solved[k]
                );
            }
        }
    });
}
