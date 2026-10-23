use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::planetary::chem::equilibrium::Network;
use crate::planetary::chem::quench::EddyDiffusion;
use crate::substance::aloft::law_aloft;
use crate::substance::{Element, by_key};
use crate::units::MetresPerSecondSquared;

/// Pascals per bar.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// Pascals at `bar` bar.
fn bar(bar: f64) -> Pascals {
    Pascals::new(bar * PASCALS_PER_BAR)
}

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// Ackerman and Marley's baseline for every deck.
fn baseline(_: SubstanceId) -> Sedimentation {
    Sedimentation::BASELINE
}

/// `f_sed` `efficiency` at `σ_g` 2 for every deck.
fn with(efficiency: f64) -> impl Fn(SubstanceId) -> Sedimentation {
    let sedimentation = Sedimentation::new(efficiency, 2.0).expect("a valid f_sed");
    move |_| sedimentation
}

/// The gas's amount of `vapour` left at `pressure`.
fn left(condensation: &Condensation, vapour: Vapour, pressure: Pascals) -> f64 {
    condensation
        .remaining(vapour, pressure)
        .expect("a finite, positive pressure")
}

/// The protosolar gas (Lodders 2010, the element registry's), every element heavier than helium
/// times `metallicity`.
fn protosolar(metallicity: f64) -> Vec<(Element, f64)> {
    Element::ALL
        .iter()
        .map(|&e| {
            let n = e.protosolar_abundance_per_hydrogen();
            let scale = if matches!(e, Element::H | Element::He) {
                1.0
            } else {
                metallicity
            };
            (e, n * scale)
        })
        .collect()
}

/// P14.T52.a's gas-only equilibrium of `abundances` at `t` kelvin and `p`: the deep gas the loop
/// reads (decision-p14-t52a-rows: "Its loop reads T52.a's gas-only deep mixing ratios,
/// `HeldAtFloor` below 300 K included").
fn speciated(abundances: &[(Element, f64)], t: f64, p: Pascals) -> Vec<(SubstanceId, f64)> {
    Network::registry()
        .expect("the registry's network builds")
        .solve(abundances.iter().copied(), Kelvin::new(t), p)
        .unwrap_or_else(|e| panic!("{t} K: {e}"))
        .mole_fractions()
        .collect()
}

/// A column of gravity `g` m s⁻², molar mass `mu` g mol⁻¹, `K_zz` `kzz` m² s⁻¹ and metallicity
/// `m` times solar, from `bottom` up to `top` bar.
fn column(g: f64, mu: f64, kzz: f64, m: f64, top: f64, bottom: f64) -> MixingColumn {
    MixingColumn {
        gravity: MetresPerSecondSquared::new(g),
        molar_mass_g_per_mol: mu,
        eddy_diffusion: EddyDiffusion::from_square_metres_per_second(kzz),
        metallicity: m,
        top: bar(top),
        bottom: bar(bottom),
    }
}

/// A profile that is the power law through `nodes` ((bar, K), the pressures descending) between
/// them, ln T linear in ln p, and held at the first and last nodes' temperatures beyond them.
fn piecewise(nodes: &'static [(f64, f64)]) -> impl Fn(Pascals) -> Kelvin {
    move |p: Pascals| {
        let u = math::ln(p.value() / PASCALS_PER_BAR);
        let (first, last) = (nodes[0], nodes[nodes.len() - 1]);
        if u >= math::ln(first.0) {
            return Kelvin::new(first.1);
        }
        for pair in nodes.windows(2) {
            let (deep, shallow) = (pair[0], pair[1]);
            let (ud, us) = (math::ln(deep.0), math::ln(shallow.0));
            if u >= us {
                let t = (ud - u) / (ud - us);
                let ln_t = math::ln(deep.1) + t * (math::ln(shallow.1) - math::ln(deep.1));
                return Kelvin::new(math::exp(ln_t));
            }
        }
        Kelvin::new(last.1)
    }
}

/// Jupiter's adiabat as a power law through the Galileo probe's 166.1 K at 1 bar and 427.7 K at
/// 22 bar (Seiff et al. 1998, from memory, as P14.T52.b's tests take them), held above the
/// tropopause at 110 K (about 0.1 bar): a test profile.
fn jupiter(p: Pascals) -> Kelvin {
    let power = math::ln(427.7 / 166.1) / math::ln(22.0);
    Kelvin::new((166.1 * math::powf(p.value() / PASCALS_PER_BAR, power)).max(110.0))
}

/// Jupiter's column: g = 24.79 m s⁻², μ = 2.3 g mol⁻¹, three times solar, from 100 bar up to
/// 10 mbar, and Ackerman and Marley's K of 2 × 10⁸ cm² s⁻¹ at its ammonia cloud (their §5).
fn jupiter_column() -> MixingColumn {
    column(24.79, 2.3, 2.0e4, 3.0, 0.01, 100.0)
}

/// Jupiter's deep gas: three times protosolar, P14.T52.a's equilibrium at 1 bar, held at its
/// 300 K floor, which puts the carbon, nitrogen, oxygen and sulphur all in CH₄, NH₃, H₂O and H₂S.
fn jupiter_gas() -> Vec<(SubstanceId, f64)> {
    speciated(&protosolar(3.0), 166.1, bar(1.0))
}

/// The deck of `key` in `condensation`.
fn deck<'a>(condensation: &'a Condensation, key: &str) -> &'a Deck {
    condensation
        .decks()
        .iter()
        .find(|d| d.condensate() == row(key))
        .unwrap_or_else(|| panic!("a {key} deck"))
}

/// Whether `condensation` has a deck of `key`.
fn has_deck(condensation: &Condensation, key: &str) -> bool {
    condensation
        .decks()
        .iter()
        .any(|d| d.condensate() == row(key))
}

/// The base of `key`'s deck, bar.
fn base_bar(condensation: &Condensation, key: &str) -> f64 {
    deck(condensation, key).base().value() / PASCALS_PER_BAR
}

fn jupiter_clouds() -> Condensation {
    Condensation::derive(jupiter_gas(), jupiter, &jupiter_column(), baseline, 1.0)
        .expect("Jupiter's clouds")
}

/// Jupiter's ammonia deck lies at 0.5–1 bar (plan 14; Sudarsky et al. 2000, §6, "~ 0.7 bar"),
/// under it the ammonium hydrosulphide near 2.3–2.4 bar (Visscher et al. 2006, §3.2.4: "near the
/// 220 K, 2.4 bar level"; Bjoraker et al. 2018) and the water near 6 bar at three times solar.
/// As built: NH₃ 0.75 bar, NH₄SH 2.46 bar, H₂O 5.81 bar.
#[test]
fn jupiter_s_ammonia_deck_lies_at_half_to_one_bar() {
    let clouds = jupiter_clouds();
    let ammonia = base_bar(&clouds, "NH3");
    let salt = base_bar(&clouds, "NH4SH");
    let water = base_bar(&clouds, "H2O");
    assert!((0.5..1.0).contains(&ammonia), "NH3 at {ammonia} bar");
    assert!((1.8..3.0).contains(&salt), "NH4SH at {salt} bar");
    assert!((4.0..8.0).contains(&water), "H2O at {water} bar");
    assert_eq!(deck(&clouds, "NH3").phase(), CondensedPhase::Solid);
    assert_eq!(deck(&clouds, "H2O").phase(), CondensedPhase::Liquid);
    // The salt takes the sulphur, the scarcer of its two vapours, so no H₂S deck forms, and the
    // ammonia it leaves condenses above it.
    assert!(!has_deck(&clouds, "H2S"));
    let sulphide = Vapour::Species(SubstanceKey::new_const("H2S"));
    let (aloft, deep) = (
        left(&clouds, sulphide, bar(1.0)),
        left(&clouds, sulphide, bar(10.0)),
    );
    assert!(aloft < 1e-4 * deep, "H2S {aloft} at 1 bar of {deep}");
    // Hottest base first.
    let order: Vec<f64> = clouds
        .decks()
        .iter()
        .map(|d| d.base_temperature().value())
        .collect();
    assert!(order.windows(2).all(|w| w[0] >= w[1]), "{order:?}");
}

/// Uranus's profile: 76 K at 1 bar (Sromovsky et al. 2011's F1, read off its Fig. 5A by the lane)
/// and 148 K at 6 bar (Irwin et al. 2022's Fig. 23, where H₂S crosses it), a power law between,
/// held at its 53 K tropopause: a test profile.
fn uranus(p: Pascals) -> Kelvin {
    let power = math::ln(148.0 / 76.0) / math::ln(6.0);
    Kelvin::new((76.0 * math::powf(p.value() / PASCALS_PER_BAR, power)).max(53.0))
}

/// Uranus's deep gas: 4% CH₄ (Irwin et al. 2022's assumed deep value; Sromovsky et al. 2011's F1),
/// 6.4 × 10⁻⁴ H₂S (Irwin et al.'s Fig. 21 caption, 37 times solar), ammonia scarcer than the
/// sulphur, and H₂ and He at 0.85 : 0.15 of the rest.
fn uranus_gas() -> Vec<(SubstanceId, f64)> {
    let (ch4, h2s, nh3) = (0.04, 6.4e-4, 1.0e-5);
    let rest = 1.0 - ch4 - h2s - nh3;
    vec![
        (row("H2"), 0.85 * rest),
        (row("He"), 0.15 * rest),
        (row("CH4"), ch4),
        (row("H2S"), h2s),
        (row("NH3"), nh3),
    ]
}

/// Uranus's methane layer lies at 1–2 bar (Irwin et al. 2022: CH₄ "should condense at ∼1.5 bar",
/// their Table 2's 1.4–1.5 bar) and its H₂S layer below 5 bar ("based at p > 5–7 bar"). As built:
/// CH₄ 1.38 bar, H₂S 5.74 bar.
#[test]
fn an_ice_giant_s_methane_condenses_at_1_to_2_bar_over_its_hydrogen_sulphide() {
    let clouds = Condensation::derive(
        uranus_gas(),
        uranus,
        &column(8.87, 2.64, 1e3, 30.0, 0.05, 50.0),
        baseline,
        1.0,
    )
    .expect("Uranus's clouds");
    let methane = base_bar(&clouds, "CH4");
    let sulphide = base_bar(&clouds, "H2S");
    assert!((1.0..2.0).contains(&methane), "CH4 at {methane} bar");
    assert!(sulphide > 5.0, "H2S at {sulphide} bar");
    // The salt takes the scarcer nitrogen, deeper still.
    assert!(base_bar(&clouds, "NH4SH") > sulphide);
}

/// A 1,200 K giant's profile: an adiabat of R ÷ `c_p` 0.3 through 1,500 K at 1 bar, held at its
/// grey skin, 2^(−1/4) × 1,200 K: a test profile, whose photosphere (T = `T_eff`) lies at 0.47 bar.
fn warm_giant(p: Pascals) -> Kelvin {
    let skin = 1_200.0 * math::powf(2.0, -0.25);
    Kelvin::new((1_500.0 * math::powf(p.value() / PASCALS_PER_BAR, 0.3)).max(skin))
}

fn warm_giant_clouds() -> Condensation {
    let gas = speciated(&protosolar(1.0), 3_000.0, bar(10.0));
    Condensation::derive(
        gas,
        warm_giant,
        &column(25.0, 2.33, 1e3, 1.0, 1e-3, 30.0),
        baseline,
        1.0,
    )
    .expect("the giant's clouds")
}

/// A 1,200 K giant has its iron deck below its forsterite and enstatite, all below its
/// photosphere, and none of NH₃ or H₂O (Sudarsky et al. 2000: "Teff ~ 1200K … a tropospheric
/// silicate cloud deck above a deeper iron cloud deck"; Morley et al. 2012's Fig. 3 orders them
/// iron, forsterite, enstatite, deepest first). As built: Fe 2.22 bar, Mg₂SiO₄ 1.60 bar, MgSiO₃
/// 1.26 bar, the photosphere 0.475 bar.
#[test]
fn a_1200_k_giant_has_iron_and_silicates_below_its_photosphere_and_no_ices() {
    let clouds = warm_giant_clouds();
    let (iron, forsterite, enstatite) = (
        base_bar(&clouds, "Fe"),
        base_bar(&clouds, "Mg2SiO4"),
        base_bar(&clouds, "MgSiO3"),
    );
    let photosphere = math::powf(1_200.0 / 1_500.0, 1.0 / 0.3);
    assert!(
        iron > forsterite && forsterite > enstatite && enstatite > photosphere,
        "Fe {iron}, Mg2SiO4 {forsterite}, MgSiO3 {enstatite}, photosphere {photosphere} bar"
    );
    assert!(!has_deck(&clouds, "NH3") && !has_deck(&clouds, "H2O"));
    // Forsterite leaves enstatite about half the silicon (Visscher et al. 2010, §5.1).
    let silicon = deck(&clouds, "MgSiO3").vapour_below();
    let deep = left(&clouds, Vapour::Element(Element::Si), bar(30.0));
    assert!(
        (0.4..0.6).contains(&(silicon / deep)),
        "{silicon} of {deep}"
    );
    // Quartz is displaced by enstatite.
    assert!(clouds.not_formed().contains(&(
        row("SiO2"),
        NotFormed::Displaced {
            by: SubstanceKey::new_const("MgSiO3")
        }
    )));
}

/// GJ 1214b's profile at 50 times solar: 900 K at 0.1 bar, R ÷ `c_p` 0.25 below, held at a 560 K
/// skin: a test profile through Morley et al. 2013's Fig. 1 crossings.
fn sub_neptune(p: Pascals) -> Kelvin {
    Kelvin::new((900.0 * math::powf(p.value() / bar(0.1).value(), 0.25)).max(560.0))
}

/// A 600 K sub-Neptune at 50 times solar has KCl and ZnS decks high in its air, ZnS below KCl,
/// and Na₂S deeper (Morley et al. 2013, ApJ 775, 33: "Only KCl and ZnS form that high in the
/// atmosphere"; their Fig. 1's cloud-free 50× models put KCl at 0.02–0.11 bar and ZnS at
/// 0.04–0.15 bar, as the lane digitised them). As built: KCl 0.073 bar, ZnS 0.103 bar, Na₂S
/// 0.27 bar.
#[test]
fn a_metal_rich_sub_neptune_s_highest_decks_are_kcl_over_zns() {
    let gas = speciated(&protosolar(50.0), 2_800.0, bar(10.0));
    let clouds = Condensation::derive(
        gas,
        sub_neptune,
        &column(8.9, 4.0, 1e3, 50.0, 1e-4, 10.0),
        with(0.1),
        1.0,
    )
    .expect("the sub-Neptune's clouds");
    let (kcl, zns, na2s) = (
        base_bar(&clouds, "KCl"),
        base_bar(&clouds, "ZnS"),
        base_bar(&clouds, "Na2S"),
    );
    assert!((0.02..0.11).contains(&kcl), "KCl at {kcl} bar");
    assert!((0.04..0.15).contains(&zns), "ZnS at {zns} bar");
    assert!(na2s > zns && zns > kcl, "Na2S {na2s}, ZnS {zns}, KCl {kcl}");
    assert!(
        deck(&clouds, "KCl").extrapolated(),
        "50× solar is outside its fit"
    );
}

/// A hot Jupiter's profile with an inversion, after Spiegel, Silverio and Burrows 2009's Fig. 4
/// for HD 209458b (the lane's digitisation): 2,400 K at a kilobar, 1,700 K at 1 bar, its 1,274 K
/// minimum at 45 mbar, and 1,845 K from a millibar up.
const HOT_JUPITER: [(f64, f64); 4] = [
    (1e3, 2_400.0),
    (1.0, 1_700.0),
    (0.045, 1_274.0),
    (1e-3, 1_845.0),
];

/// TiO is cold-trapped where CaTiO₃ forms below it: above the deck's cold minimum the titanium
/// stays out of the gas even where the inversion is hot enough to hold it (Spiegel et al. 2009:
/// "Without macroscopic mixing, fTiO = 0 at the top of the atmosphere").
#[test]
fn tio_is_cold_trapped_where_catio3_forms_below_it() {
    let gas = speciated(&protosolar(1.0), 2_400.0, bar(100.0));
    let clouds = Condensation::derive(
        gas,
        piecewise(&HOT_JUPITER),
        &column(9.4, 2.33, 1e3, 1.0, 1e-5, 1e3),
        baseline,
        1.0,
    )
    .expect("the hot Jupiter's clouds");
    let titanate = deck(&clouds, "CaTiO3");
    let titanium = Vapour::Element(Element::Ti);
    let deep = left(&clouds, titanium, bar(1e3));
    let aloft = left(&clouds, titanium, bar(1e-4));
    assert!(titanate.base().value() > bar(1.0).value());
    assert!(aloft < 1e-6 * deep, "Ti {aloft} aloft of {deep}");
    // At 1 mbar the inversion's 1,845 K is above perovskite's condensation for the deep gas's
    // titanium: without the trap the gas would hold it.
    let law = law_aloft(row("CaTiO3")).expect("a law");
    let free = law
        .condensation_temperature(bar(1e-3), deep, 0.0)
        .expect("it condenses");
    assert!(free.value() < 1_845.0, "{}", free.value());
    assert!(clouds.not_formed().contains(&(
        row("TiO2"),
        NotFormed::Displaced {
            by: SubstanceKey::new_const("CaTiO3")
        }
    )));
}

/// The tests' columns, by name, with their metallicities in dex.
const CASES: [(&str, f64); 7] = [
    ("Jupiter", 0.477_121_254_719_662_44),
    ("Ackerman and Marley's Jupiter", 0.477_121_254_719_662_44),
    ("Uranus", 1.477_121_254_719_662_4),
    ("a 1,200 K giant", 0.0),
    ("a 600 K sub-Neptune", 1.698_970_004_336_018_7),
    ("a hot Jupiter", 0.0),
    ("a Venus-like air", 0.0),
];

/// The condensation of the test column `name` (one of [`CASES`]).
fn case(name: &str) -> Condensation {
    match name {
        "Jupiter" => jupiter_clouds(),
        "Ackerman and Marley's Jupiter" => Condensation::derive(
            ackerman_and_marley_gas(),
            jupiter,
            &jupiter_column(),
            baseline,
            1.0,
        )
        .expect("the cloud"),
        "Uranus" => Condensation::derive(
            uranus_gas(),
            uranus,
            &column(8.87, 2.64, 1e3, 30.0, 0.05, 50.0),
            baseline,
            1.0,
        )
        .expect("Uranus's clouds"),
        "a 1,200 K giant" => warm_giant_clouds(),
        "a 600 K sub-Neptune" => Condensation::derive(
            speciated(&protosolar(50.0), 2_800.0, bar(10.0)),
            sub_neptune,
            &column(8.9, 4.0, 1e3, 50.0, 1e-4, 10.0),
            with(0.1),
            1.0,
        )
        .expect("the sub-Neptune's clouds"),
        "a hot Jupiter" => Condensation::derive(
            speciated(&protosolar(1.0), 2_400.0, bar(100.0)),
            piecewise(&HOT_JUPITER),
            &column(9.4, 2.33, 1e3, 1.0, 1e-5, 1e3),
            baseline,
            0.5,
        )
        .expect("the hot Jupiter's clouds"),
        "a Venus-like air" => venus_clouds(),
        _ => panic!("{name} is no case"),
    }
}

/// The tests' columns: each case's name, its condensation and its metallicity in dex.
fn cases() -> Vec<(&'static str, Condensation, f64)> {
    CASES
        .iter()
        .map(|&(name, metallicity)| (name, case(name), metallicity))
        .collect()
}

/// Every base that crosses lies on its law's saturation curve to 10⁻⁶ in pressure, in every
/// test column: the vapour laws, the product law, the rows' own curves and the acid's.
#[test]
fn each_base_lies_on_its_saturation_curve() {
    let mut checked = 0;
    for (name, clouds, metallicity) in &cases() {
        for d in clouds.decks() {
            if d.bound() != DeckBound::Crossing {
                continue;
            }
            let law = law_aloft(d.condensate()).expect("a law");
            let ln_sat = law
                .ln_saturation_pressure(d.base_temperature(), d.base(), *metallicity)
                .expect("it condenses");
            let ln_vapour = match law.law() {
                SaturationLaw::Product { .. } => {
                    let other = law.others()[0].vapour();
                    let first = left(clouds, law.limiting().vapour(), d.base());
                    let second = left(clouds, other, d.base());
                    math::ln(first * second * d.base().value() * d.base().value())
                }
                _ => math::ln(d.vapour_below() * d.base().value()),
            };
            assert!(
                (ln_vapour - ln_sat).abs() < 1e-6,
                "{name} {}: {ln_vapour} against {ln_sat}",
                d.condensate().substance().key().as_str()
            );
            checked += 1;
        }
    }
    assert!(checked >= 30, "{checked} bases checked");
}

/// The traps conserve what they draw on: at every level, for each vapour a deck draws (an
/// element's atoms, or a species), the gas's amount with what the decks have drawn of it is the
/// deep gas's, and lies between none and all of it. Each deck's draws are recomputed here from its
/// public vapours: its limiting vapour gives what it can of the deck's loss, and each other vapour
/// its count's share of what that gave, each held at what is left. At a deck's top the gas keeps
/// no more of its limiting vapour than the deck leaves. (The vapour itself may rise again above a
/// cold minimum, where the balance's lofted condensate evaporates into a warmer layer.)
#[test]
fn the_limiting_element_is_conserved_through_the_traps() {
    for (name, clouds, _) in &cases() {
        let mut vapours: Vec<Vapour> = clouds.draws.iter().flatten().map(|&(v, _)| v).collect();
        vapours.sort();
        vapours.dedup();
        let deep: Vec<f64> = vapours.iter().map(|&v| amount(v, &clouds.gas)).collect();
        let place = |v: Vapour| vapours.iter().position(|w| *w == v).expect("a vapour");
        let bottom = clouds
            .decks()
            .iter()
            .map(|d| d.base().value())
            .fold(0.0, f64::max);
        for k in 0..=400 {
            let p = Pascals::new(bottom * math::exp(-0.05 * f64::from(k)));
            let mut gas = deep.clone();
            let mut drawn = vec![0.0; vapours.len()];
            for (d, draws) in clouds.decks().iter().zip(&clouds.draws) {
                let loss = if p.value() >= d.base().value() {
                    0.0
                } else {
                    (d.vapour_below() - d.vapour_at(p).expect("a positive pressure")).max(0.0)
                };
                let limiting = place(draws[0].0);
                let given = loss.min(gas[limiting]);
                gas[limiting] -= given;
                drawn[limiting] += given;
                for &(v, ratio) in &draws[1..] {
                    let at = place(v);
                    let taken = (ratio * given).min(gas[at]);
                    gas[at] -= taken;
                    drawn[at] += taken;
                }
            }
            for (at, &vapour) in vapours.iter().enumerate() {
                let kept = left(clouds, vapour, p);
                assert!(
                    (kept + drawn[at] - deep[at]).abs() <= 1e-12 * deep[at],
                    "{name} {vapour:?} at {} Pa: {kept} + {} against {}",
                    p.value(),
                    drawn[at],
                    deep[at]
                );
                assert!(
                    (0.0..=deep[at] * (1.0 + 1e-12)).contains(&kept),
                    "{name} {vapour:?}: {kept} of {}",
                    deep[at]
                );
            }
        }
        for d in clouds.decks() {
            let law = law_aloft(d.condensate()).expect("a law");
            if law.vapour_share() < 1.0 || matches!(law.law(), SaturationLaw::Product { .. }) {
                continue;
            }
            // To rounding in the deep gas's amount, which the gas left is a difference from.
            let kept = left(clouds, law.limiting().vapour(), d.top());
            assert!(
                kept <= d.vapour_above() + 1e-12 * d.vapour_below(),
                "{name} {}: {kept} above {}",
                d.condensate().substance().key().as_str(),
                d.vapour_above()
            );
        }
    }
}

/// Ackerman and Marley's Jupiter gas: 3 × 10⁻⁵ of ammonia below the cloud (Kunde et al. 1982's
/// NEB value at 0.6 bar, their §5) in H₂ and He.
fn ackerman_and_marley_gas() -> Vec<(SubstanceId, f64)> {
    vec![(row("H2"), 0.864), (row("He"), 0.136), (row("NH3"), 3e-5)]
}

/// Ackerman and Marley's own Jupiter: 3 × 10⁻⁵ of ammonia below the cloud, `f_sed` 3 and `σ_g` 2,
/// K 2 × 10⁸ cm² s⁻¹. Their full model gives a base at 0.42 bar and 129 K on Voyager's profile,
/// `r_w` 35 µm, an `r_eff` of 46 µm at the base and 44 µm over the cloud, a column of 52 g m⁻² and
/// τ about 2.1 (their §5 and Fig. 5a, as the lane read them). Here, on the test profile with
/// L = H: the cloud's `r_eff` within 30% of 44 µm, and a column of 40–120 g m⁻² (L = H is
/// expected to thicken it, their stable layer's L < H not modelled). As built: base 0.53 bar and
/// 137 K, `r_eff` 44 µm, 105 g m⁻², τ 4.4 by the geometric limit.
#[test]
fn the_ammonia_cloud_s_size_and_column_follow_ackerman_and_marley() {
    let gas = vec![(row("H2"), 0.864), (row("He"), 0.136), (row("NH3"), 3e-5)];
    let clouds =
        Condensation::derive(gas, jupiter, &jupiter_column(), baseline, 1.0).expect("the cloud");
    let ammonia = deck(&clouds, "NH3");
    let mode = ammonia.modes()[0];
    let size = mode.size().expect("a size");
    let r_eff = size.effective_radius().value() * 1e6;
    let grams = ammonia.column_mass().value() * 1e3;
    assert!(
        (44.0 * 0.7..44.0 * 1.3).contains(&r_eff),
        "r_eff {r_eff} µm"
    );
    assert!((40.0..120.0).contains(&grams), "{grams} g m⁻²");
    assert!(
        (size.effective_variance() - (math::exp(math::ln(2.0) * math::ln(2.0)) - 1.0)).abs()
            < 1e-12
    );
    let tau = mode.optical_depth().expect("a τ");
    assert_eq!(tau.basis(), OpticalDepthBasis::GeometricLimit);
    assert!((1.0..8.0).contains(&tau.value()), "τ {}", tau.value());
    // A larger f_sed rains more out: a thinner column of larger particles.
    let rainier = Condensation::derive(
        ackerman_and_marley_gas(),
        jupiter,
        &jupiter_column(),
        with(5.0),
        1.0,
    )
    .expect("the cloud");
    let wetter = deck(&rainier, "NH3");
    assert!(wetter.column_mass().value() < ammonia.column_mass().value());
    let larger = wetter.modes()[0]
        .size()
        .expect("a size")
        .effective_radius()
        .value();
    assert!(larger > size.effective_radius().value());
}

/// Venus-like: an air of CO₂ with SO₂ forms the acid's deck from `ε_v` of its SO₂, liquid, while
/// its bulk CO₂ is labelled rather than trapped. Venus's deck itself is P14.T24.g's to assert.
#[test]
fn a_venus_like_air_forms_the_acid_deck_and_labels_its_bulk() {
    let clouds = venus_clouds();
    let acid = deck(&clouds, "H2SO4");
    assert_eq!(acid.phase(), CondensedPhase::Liquid);
    assert_same_bits(acid.vapour_below(), 0.06 * 1.3e-4);
    assert!(
        clouds
            .not_formed()
            .contains(&(row("CO2"), NotFormed::BulkGas))
    );
}

/// A Venus-like air: 96.5% CO₂ and 3.5% N₂ with 130 ppm of SO₂, on a test profile of 737 K at
/// 92 bar, 350 K at 1 bar, 260 K at 0.1 bar and 230 K at 10 mbar.
fn venus_clouds() -> Condensation {
    let gas = vec![
        (row("CO2"), 0.965),
        (row("N2"), 0.035),
        (row("SO2"), 1.3e-4),
    ];
    let venus = piecewise(&[(92.0, 737.0), (1.0, 350.0), (0.1, 260.0), (0.01, 230.0)]);
    Condensation::derive(
        gas,
        venus,
        &column(8.87, 43.4, 1e3, 1.0, 1e-3, 92.0),
        baseline,
        1.0,
    )
    .expect("Venus's clouds")
}

/// The loop is pure and its order is its own: every case gives the same condensation, to the
/// bit, whatever was derived before it, and the gas given in reverse gives the same as in order.
#[test]
fn the_condensation_is_independent_of_order() {
    let names: Vec<&str> = CASES.iter().map(|&(name, _)| name).collect();
    assert_order_independent(&names, |name| format!("{:?}", case(name)));
    let forward = jupiter_gas();
    let mut backward = forward.clone();
    backward.reverse();
    let derive = |gas: Vec<(SubstanceId, f64)>| {
        Condensation::derive(gas, jupiter, &jupiter_column(), baseline, 1.0).expect("the clouds")
    };
    assert_eq!(
        format!("{:?}", derive(forward)),
        format!("{:?}", derive(backward))
    );
}

/// A deck takes its own sedimentation: a larger `f_sed` on the ammonia alone thins its deck and
/// leaves the others as they were.
#[test]
fn each_deck_takes_its_own_sedimentation() {
    let ammonia = row("NH3");
    let rainier = |id: SubstanceId| {
        let efficiency = if id == ammonia { 6.0 } else { 3.0 };
        Sedimentation::new(efficiency, 2.0).expect("valid")
    };
    let base = jupiter_clouds();
    let varied = Condensation::derive(jupiter_gas(), jupiter, &jupiter_column(), rainier, 1.0)
        .expect("the clouds");
    let (thin, thick) = (deck(&varied, "NH3"), deck(&base, "NH3"));
    assert!(thin.column_mass().value() < thick.column_mass().value());
    assert_same_bits(thin.sedimentation(), 6.0);
    assert_same_bits(thin.modes()[0].profile().exponent(), 6.0);
    for key in ["H2O", "NH4SH"] {
        assert_same_bits(
            deck(&varied, key).column_mass().value(),
            deck(&base, key).column_mass().value(),
        );
    }
}

/// A gas whose most abundant species has no collision constants gives its modes no size, labelled,
/// and its decks their column all the same: ammonia in krypton.
#[test]
fn a_background_without_collision_constants_gives_no_size() {
    let gas = vec![(row("Kr"), 0.9), (row("He"), 0.0997), (row("NH3"), 3e-4)];
    let clouds =
        Condensation::derive(gas, jupiter, &jupiter_column(), baseline, 1.0).expect("the clouds");
    let ammonia = deck(&clouds, "NH3");
    assert!(ammonia.column_mass().value() > 0.0);
    assert_eq!(
        ammonia.modes()[0].size(),
        Err(SizeModeError::NoCollisionConstants {
            key: SubstanceKey::new_const("Kr")
        })
    );
}

/// A mode built for a rule takes its τ(550) as the loop's do, and its inputs are checked.
#[test]
fn a_rule_s_mode_takes_its_optics_as_the_loop_s() {
    let clouds = jupiter_clouds();
    let ammonia = deck(&clouds, "NH3");
    let loop_mode = ammonia.modes()[0];
    let rebuilt = Mode::new(
        loop_mode.material(),
        loop_mode.phase(),
        loop_mode.deck(),
        loop_mode.size().expect("a size"),
        loop_mode.column_mass(),
        loop_mode.profile(),
        loop_mode.coverage(),
        ammonia.base_temperature(),
    )
    .expect("a mode");
    assert_eq!(rebuilt, loop_mode);
    let well_mixed = ModeProfile::new(bar(0.7), bar(0.3), 0.0).expect("a profile");
    assert_same_bits(well_mixed.exponent(), 0.0);
    assert!(matches!(
        ModeProfile::new(bar(0.3), bar(0.7), 0.0),
        Err(BuildModeError::Profile { .. })
    ));
    assert!(matches!(
        ModeProfile::new(bar(0.7), bar(0.3), -1.0),
        Err(BuildModeError::Profile { .. })
    ));
    let size = loop_mode.size().expect("a size");
    let mass = KilogramsPerSquareMetre::new(-1.0);
    let bad = Mode::new(
        loop_mode.material(),
        CondensedPhase::Solid,
        loop_mode.deck(),
        size,
        mass,
        well_mixed,
        1.0,
        Kelvin::new(150.0),
    );
    assert_eq!(bad, Err(BuildModeError::ColumnMass { column_mass: mass }));
}

/// Nothing on the record reaches the loop or the laws aloft before P14.T54.b wires them
/// (decision-bump-plan-after-22 §1.3): no code of the crate's `src/` outside the two modules,
/// their tests and P14.T24.c's and T24.g's rules (`hooks/aerosols` and `hooks/sulphur`, pure
/// themselves until P14.T54.b) names them, comments and the modules' own declarations left out.
/// **P14.T54.b deletes this tripwire** with `substance::aerosol`'s.
#[test]
fn nothing_on_the_record_calls_the_condensation_yet() {
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
    let own = [
        "planetary/chem/condense",
        "substance/aloft",
        "planetary/hooks/aerosols",
        "planetary/hooks/sulphur",
    ];
    let patterns = [
        "chem::condense",
        "condense::",
        "substance::aloft",
        "aloft::",
    ];
    let declarations = ["pub mod condense;", "pub mod aloft;"];
    let mut found = Vec::new();
    for path in &files {
        let relative = path
            .strip_prefix(&src)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/");
        if own.iter().any(|o| relative.starts_with(o)) {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("a readable source");
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") || declarations.contains(&code) {
                continue;
            }
            if patterns.iter().any(|p| code.contains(p)) {
                found.push(format!("{relative}:{}: {code}", n + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "nothing on the record calls them before P14.T54.b: {found:#?}"
    );
}

/// Its inputs are checked.
#[test]
fn the_inputs_are_checked() {
    let h2 = row("H2");
    let column = jupiter_column();
    let bad = Condensation::derive([(h2, f64::NAN)], jupiter, &column, baseline, 1.0);
    assert!(matches!(bad, Err(DeriveCondensationError::Amount { .. })));
    let twice = Condensation::derive([(h2, 0.5), (h2, 0.5)], jupiter, &column, baseline, 1.0);
    assert_eq!(twice, Err(DeriveCondensationError::Repeated { id: h2 }));
    let cover = Condensation::derive([(h2, 1.0)], jupiter, &column, baseline, 1.5);
    assert!(matches!(
        cover,
        Err(DeriveCondensationError::Coverage { .. })
    ));
    let mut inverted = jupiter_column();
    inverted.top = bar(1e3);
    let refused = Condensation::derive([(h2, 1.0)], jupiter, &inverted, baseline, 1.0);
    assert!(matches!(
        refused,
        Err(DeriveCondensationError::Column { field: "top", .. })
    ));
    let cold = Condensation::derive([(h2, 1.0)], |_| Kelvin::new(0.0), &column, baseline, 1.0);
    assert!(matches!(cold, Err(DeriveCondensationError::Profile { .. })));
    assert_eq!(
        Sedimentation::new(0.0, 2.0),
        Err(BuildSedimentationError::Efficiency { efficiency: 0.0 })
    );
    for width in [1.0, 2.5, f64::NAN] {
        assert!(matches!(
            Sedimentation::new(3.0, width),
            Err(BuildSedimentationError::Width { .. })
        ));
    }
    let clouds = jupiter_clouds();
    let ammonia = Vapour::Species(SubstanceKey::new_const("NH3"));
    assert_eq!(clouds.remaining(ammonia, Pascals::new(f64::NAN)), None);
    assert_eq!(clouds.remaining(ammonia, Pascals::new(0.0)), None);
    assert_eq!(deck(&clouds, "NH3").vapour_at(Pascals::new(-1.0)), None);
}

/// The laws aloft and the decks of the tests' columns are pinned: each law's condensation
/// temperature at three pressures, and each deck's base, top, temperature, vapours, the gas's
/// vapour left at its base and top, its column, size and τ(550), in placing order, and each row
/// not formed.
#[test]
fn the_condensation_is_pinned() {
    use crate::GENERATOR_VERSION;
    use crate::substance::aloft::laws_aloft;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (id, law) in laws_aloft() {
        let key = id.substance().key();
        let key = key.as_str();
        let vapour = law.reference().map_or(1e-6, |r| r.solar_mole_fraction);
        w.line(&format!(
            "{key} law: {:?}, limiting {:?}, others {:?}, share {}",
            law.law(),
            law.limiting(),
            law.others(),
            law.vapour_share()
        ));
        for p in [1e-3, 1.0, 100.0] {
            match law.condensation_temperature(bar(p), vapour, 0.0) {
                Some(t) => w.f64(&format!("{key} law at {p} bar"), t.value()),
                None => w.line(&format!("{key} law at {p} bar: none")),
            }
        }
    }
    for (name, clouds, _) in &cases() {
        for d in clouds.decks() {
            let key = d.condensate().substance().key();
            let key = key.as_str();
            let law = law_aloft(d.condensate()).expect("a law");
            w.line(&format!("{name} {key}: {:?} {:?}", d.bound(), d.phase()));
            w.f64(&format!("{name} {key} base"), d.base().value());
            w.f64(
                &format!("{name} {key} base temperature"),
                d.base_temperature().value(),
            );
            w.f64(&format!("{name} {key} top"), d.top().value());
            w.f64(&format!("{name} {key} vapour below"), d.vapour_below());
            w.f64(&format!("{name} {key} vapour above"), d.vapour_above());
            for (at, p) in [("base", d.base()), ("top", d.top())] {
                w.f64(
                    &format!("{name} {key} gas left at its {at}"),
                    left(clouds, law.limiting().vapour(), p),
                );
            }
            w.f64(
                &format!("{name} {key} column mass"),
                d.column_mass().value(),
            );
            let mode = d.modes()[0];
            match mode.size() {
                Ok(size) => {
                    w.f64(
                        &format!("{name} {key} r_eff"),
                        size.effective_radius().value(),
                    );
                }
                Err(error) => w.line(&format!("{name} {key} size: {error}")),
            }
            match mode.optical_depth() {
                Ok(tau) => {
                    w.line(&format!("{name} {key} tau basis: {:?}", tau.basis()));
                    w.f64(&format!("{name} {key} tau"), tau.value());
                }
                Err(error) => w.line(&format!("{name} {key} tau: {error}")),
            }
            w.f64(&format!("{name} {key} coverage"), mode.coverage());
        }
        for (id, why) in clouds.not_formed() {
            w.line(&format!(
                "{name} {} not formed: {why:?}",
                id.substance().key().as_str()
            ));
        }
    }
    golden!("planetary/condense", w.as_str());
}

/// A large `f_sed`'s column follows the balance's steady state, s `q_s` ÷ (`f_sed` − s), with no
/// floor from the substeps (corrected 2026-10-22 under P14.T24.c): Uranus's methane column falls as
/// 1 ÷ `f_sed` from 10³ to 10⁵, each decade to 2% (as built, 10.04 and 10.004), where the
/// relaxation alone floored it near 14.6 kg m⁻².
#[test]
fn a_large_sedimentation_s_column_falls_as_its_inverse() {
    let at = |efficiency: f64| {
        let clouds = Condensation::derive(
            uranus_gas(),
            uranus,
            &column(8.87, 2.64, 10.0, 30.0, 0.05, 50.0),
            with(efficiency),
            1.0,
        )
        .expect("Uranus's clouds");
        deck(&clouds, "CH4").column_mass().value()
    };
    let (thick, middle, thin) = (at(1e3), at(1e4), at(1e5));
    for (a, b) in [(thick, middle), (middle, thin)] {
        assert!((a / b / 10.0 - 1.0).abs() < 0.01, "{a} against {b}");
    }
}

/// Halving the substeps (to 2 of [`SUBSTEPS`]' 5, the nearest whole half) and doubling them each
/// move a deck's column by under 10⁻³ of itself at `f_sed` 3 and at 10⁴: the exact step converges
/// (decision-bump-plan-after-22, "The decks' `f_sed`, K and humidity", G4).
#[test]
fn halving_the_substeps_moves_a_column_under_a_thousandth() {
    for efficiency in [3.0, 1e4] {
        let at = |substeps: u32| {
            let clouds = Condensation::derive_in(
                uranus_gas(),
                uranus,
                &column(8.87, 2.64, 10.0, 30.0, 0.05, 50.0),
                with(efficiency),
                1.0,
                substeps,
            )
            .expect("Uranus's clouds");
            ["CH4", "H2S"].map(|key| deck(&clouds, key).column_mass().value())
        };
        let (half, built, double) = (at(SUBSTEPS / 2), at(SUBSTEPS), at(2 * SUBSTEPS));
        for other in [half, double] {
            for (a, b) in other.iter().zip(&built) {
                assert!(
                    (a / b - 1.0).abs() < 1e-3,
                    "f_sed {efficiency}: {a} against {b}"
                );
            }
        }
    }
}

/// At `f_sed` 3 the exact step moves T52.c's columns by under 1% from the relaxation's, its golden
/// at 23 before the correction (2026-10-22): Jupiter's NH₃, NH₄SH and H₂O, Ackerman and Marley's
/// Jupiter and Uranus's CH₄ and H₂S.
#[test]
fn the_exact_step_moves_the_baseline_s_columns_under_a_percent() {
    for (name, key, before) in [
        ("Jupiter", "NH3", 1.639_787_196_084_204_3),
        ("Jupiter", "NH4SH", 3.767_188_712_851_041),
        ("Jupiter", "H2O", 89.692_829_265_047_41),
        (
            "Ackerman and Marley's Jupiter",
            "NH3",
            0.105_268_665_704_398_02,
        ),
        ("Uranus", "CH4", 772.846_191_755_199_6),
        ("Uranus", "H2S", 113.415_605_843_203_08),
    ] {
        let now = deck(&case(name), key).column_mass().value();
        assert!(
            (now / before - 1.0).abs() < 0.01,
            "{name}'s {key}: {now} against {before}"
        );
    }
}
