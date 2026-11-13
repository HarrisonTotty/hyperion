use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;

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

fn network() -> Network {
    Network::registry().expect("the registry's network builds")
}

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// Pascals at `bar` bar.
fn bar(bar: f64) -> Pascals {
    Pascals::new(bar * PASCALS_PER_BAR)
}

/// Jupiter's adiabat as a power law through the Galileo probe's 166.1 K at 1 bar and 427.7 K at
/// 22 bar (Seiff et al. 1998, J. Geophys. Res. 103, 22857, from memory, as P14.T24.d's test
/// takes them), extended deeper with the same exponent, about 0.306: a test profile, not
/// P14.T24.d's.
fn jupiter(p: Pascals) -> Kelvin {
    let power = math::ln(427.7 / 166.1) / math::ln(22.0);
    Kelvin::new(166.1 * math::powf(p.value() / PASCALS_PER_BAR, power))
}

/// Jupiter's column: g = 24.79 m s⁻² (its equatorial 1-bar gravity), μ = 2.3 g mol⁻¹, three
/// times solar (its carbon, about 3 times protosolar; Wong et al. 2004, from memory), searched
/// from 10 mbar to 1 kbar.
fn jupiter_column(eddy_diffusion: EddyDiffusion) -> MixingColumn {
    MixingColumn {
        gravity: MetresPerSecondSquared::new(24.79),
        molar_mass_g_per_mol: 2.3,
        eddy_diffusion,
        metallicity: 3.0,
        top: bar(0.01),
        bottom: bar(1_000.0),
    }
}

/// ZM14's planet of §3.1, `T_eff` 600 K and g = 10³ cm s⁻² (10 m s⁻²), with "a more Jupiter-like
/// Kzz = 10^7 cm2/s", whose CO "quench point" is "1.5 bars and 1280 K", on their cloudless solar
/// profile (Saumon and Marley 2008), which they do not tabulate.
const ANCHOR: (f64, f64, f64) = (600.0, 1.5, 1_280.0);

/// ZM14's R ÷ `c_p` at 1,000 K, 0.29 (their §4: "R/Cp decreases monotonically from 0.29 at 1000 K
/// to 0.25 at 2100 K").
const ADIABAT_POWER: f64 = 0.29;

/// A self-luminous planet of `T_eff` `effective` K at ZM14's gravity of 10³ cm s⁻²: the adiabat
/// through [`ANCHOR`]'s quench point at R ÷ `c_p` = [`ADIABAT_POWER`], its temperatures scaled with
/// `T_eff` at each pressure. A test profile, homologous in `T_eff`, standing in for ZM14's.
fn self_luminous(effective: f64) -> impl Fn(Pascals) -> Kelvin {
    let (teff, p_bar, t) = ANCHOR;
    move |p: Pascals| {
        let ratio = p.value() / PASCALS_PER_BAR / p_bar;
        Kelvin::new(effective / teff * t * math::powf(ratio, ADIABAT_POWER))
    }
}

/// ZM14's planet's column: g = 10 m s⁻², their 84% H₂ and 16% He (2.334 g mol⁻¹), solar,
/// searched from 1 mbar to 30 bar, where [`self_luminous`]'s homologous profile stays within the
/// network's 6,000 K up to a `T_eff` of 900 K.
fn self_luminous_column(eddy_diffusion: EddyDiffusion) -> MixingColumn {
    MixingColumn {
        gravity: MetresPerSecondSquared::new(10.0),
        molar_mass_g_per_mol: 2.334,
        eddy_diffusion,
        metallicity: 1.0,
        top: bar(1e-3),
        bottom: bar(30.0),
    }
}

/// `K_zz` of `cm2_per_s` cm² s⁻¹.
fn cgs(cm2_per_s: f64) -> EddyDiffusion {
    EddyDiffusion::from_square_metres_per_second(cm2_per_s * 1e-4)
}

/// The gas at `p` along `profile`: its local equilibrium, and quenched.
fn quenched_at(
    network: &Network,
    gas: &[(Element, f64)],
    quench: &Quench,
    profile: &impl Fn(Pascals) -> Kelvin,
    p: Pascals,
) -> (GasEquilibrium, QuenchedGas) {
    let local = network
        .solve(gas.iter().copied(), profile(p), p)
        .unwrap_or_else(|e| panic!("{} Pa: {e}", p.value()));
    let quenched = quench
        .apply(network, profile(p), p)
        .unwrap_or_else(|e| panic!("{} Pa: {e}", p.value()));
    (local, quenched)
}

/// The chemical times are ZM14's: their eq. 14's harmonic sum of eqs. 12 and 13, and eqs. 32, 40
/// and 44, each at a point; the two CO–CH₄ fits cross at 994 K at 1 bar (17,000 ÷ ln(2.667 ×
/// 10⁷), the lane's arithmetic), where the sum is half of each; and ZM14's 8.0 × 10⁴ s for NH₃ at
/// 10 bar and 1,750 K, the point their eq. 32 is pinned at ("roughly the same time scale as Eq 32
/// at 10 bars and 1750 K").
#[test]
fn the_chemical_times_are_zahnle_and_marley_s() {
    let time = |system: QuenchedSystem, t: f64, p: f64, m: f64| {
        system.chemical_time(Kelvin::new(t), bar(p), m).value()
    };
    let close = |got: f64, want: f64, tolerance: f64| {
        assert!((got / want - 1.0).abs() < tolerance, "{got} against {want}");
    };
    let warm = 1.5e-6 / 2.0 * math::powf(3.0, -0.7) * math::exp(42_000.0 / 1_300.0);
    let cold = 40.0 / 4.0 * math::exp(25_000.0 / 1_300.0);
    close(
        time(QuenchedSystem::CarbonMonoxide, 1_300.0, 2.0, 3.0),
        1.0 / (1.0 / warm + 1.0 / cold),
        1e-12,
    );
    let crossing = 17_000.0 / math::ln(1.5_f64.recip() * 40.0 * 1e6);
    assert!((crossing - 994.0).abs() < 1.0, "{crossing}");
    let warm = 1.5e-6 * math::exp(42_000.0 / crossing);
    close(
        time(QuenchedSystem::CarbonMonoxide, crossing, 1.0, 1.0),
        warm / 2.0,
        1e-9,
    );
    close(
        time(QuenchedSystem::Ammonia, 1_750.0, 10.0, 1.0),
        8.0e4,
        0.01,
    );
    close(
        time(QuenchedSystem::HydrogenCyanide, 1_500.0, 5.0, 2.0),
        1.5e-4 / 5.0 * math::powf(2.0, -0.7) * math::exp(24.0),
        1e-12,
    );
    close(
        time(QuenchedSystem::CarbonDioxide, 1_000.0, 4.0, 10.0),
        1.0e-10 / 2.0 * math::exp(38.0),
        1e-12,
    );
}

/// ZM14's own quench point, "1.5 bars and 1280 K" for their 600 K planet at g = 10³ cm s⁻² and
/// `K_zz` = 10⁷ cm² s⁻¹, found by their kinetics model (not by eq. 14): there `t_CO` ÷ `t_mix` is
/// 0.83 (the lane's arithmetic, with L = H and μ = 2.334), within a factor of two of 1, so the
/// relations and the mixing time are theirs.
#[test]
fn the_co_time_meets_the_mixing_at_zahnle_and_marley_s_quench_point() {
    let column = self_luminous_column(EddyDiffusion::JUPITER_LIKE);
    let (_, p, t) = ANCHOR;
    let t = Kelvin::new(t);
    let ratio = QuenchedSystem::CarbonMonoxide
        .chemical_time(t, bar(p), 1.0)
        .value()
        / mixing_time(t, &column).value();
    assert!((ratio - 0.83).abs() < 0.01, "{ratio}");
}

/// Each level found is a crossing where the chemical time equals the mixing time to 10⁻⁹, and it
/// is the deepest: a level 1% deeper is still in equilibrium.
#[test]
fn the_quench_level_is_where_the_times_meet() {
    let column = jupiter_column(EddyDiffusion::JUPITER_LIKE);
    for system in QuenchedSystem::ALL {
        let level = quench_level(system, jupiter, &column).expect("a level");
        assert_eq!(level.system(), system);
        if level.bound() != QuenchBound::Crossing {
            assert_eq!(system, QuenchedSystem::Ammonia, "{level:?}");
            continue;
        }
        let excess = |p: Pascals| {
            let t = jupiter(p);
            math::ln(system.chemical_time(t, p, column.metallicity).value())
                - math::ln(mixing_time(t, &column).value())
        };
        assert!(excess(level.pressure()).abs() < 1e-9, "{level:?}");
        assert!(excess(level.pressure() * 1.01) < 0.0, "{level:?}");
        assert_same_bits(
            level.temperature().value(),
            jupiter(level.pressure()).value(),
        );
    }
}

/// Jupiter's CO above its equilibrium value at 1 bar, by the quench (the plan's test): the quench
/// level's CO, near 1,000 K and some 350 bar, against the 1-bar level's equilibrium (the
/// network's floor held, CH₄ all but all-in), by more than ten orders of magnitude. Bézard et al.
/// 2002 measured (1.0 ± 0.2) × 10⁻⁹ (Icarus 159, 95, through Visscher, Moses and Saslow 2010,
/// §2.4, for the 6-bar level); with ZM14's Jupiter-like mixing the quench gives it within a
/// factor of three: 1.5 × 10⁻⁹ as built, quenched at 356 bar and 1,002 K on this profile.
#[test]
fn jupiter_s_carbon_monoxide_is_above_its_equilibrium_at_one_bar() {
    let network = network();
    let gas = protosolar(3.0);
    let column = jupiter_column(EddyDiffusion::JUPITER_LIKE);
    let quench = Quench::derive(&network, &gas, jupiter, &column).expect("a quench");
    let level = quench.level(QuenchedSystem::CarbonMonoxide);
    assert_eq!(level.bound(), QuenchBound::Crossing);
    let (local, quenched) = quenched_at(&network, &gas, &quench, &jupiter, bar(1.0));
    assert!(quenched.is_quenched(QuenchedSystem::CarbonMonoxide));
    let co = row("CO");
    let equilibrium = local.mole_fraction(co).expect("carried");
    let got = quenched.mole_fraction(co).expect("carried");
    assert!(got > 1e10 * equilibrium, "{got} against {equilibrium}");
    assert!((1.0e-9 / 3.0..3.0e-9).contains(&got), "{got}");
}

/// The quenched CO and CH₄ of [`self_luminous`]'s planet, above every quench level: CO ÷ CH₄.
fn carbon_ratio(network: &Network, effective: f64, eddy_diffusion: EddyDiffusion) -> f64 {
    let gas = protosolar(1.0);
    let profile = self_luminous(effective);
    let column = self_luminous_column(eddy_diffusion);
    let quench = Quench::derive(network, &gas, &profile, &column).expect("a quench");
    let (_, quenched) = quenched_at(network, &gas, &quench, &profile, bar(0.01));
    assert!(quenched.is_quenched(QuenchedSystem::CarbonMonoxide));
    quenched.mole_fraction(row("CO")).expect("carried")
        / quenched.mole_fraction(row("CH4")).expect("carried")
}

/// The `T_eff` at which [`self_luminous`]'s planet's quenched CO equals its CH₄, by bisection
/// between 300 and 900 K.
fn transition(network: &Network, eddy_diffusion: EddyDiffusion) -> f64 {
    let (mut low, mut high) = (300.0_f64, 900.0_f64);
    assert!(carbon_ratio(network, low, eddy_diffusion) < 1.0);
    assert!(carbon_ratio(network, high, eddy_diffusion) > 1.0);
    for _ in 0..30 {
        let middle = low.midpoint(high);
        if carbon_ratio(network, middle, eddy_diffusion) < 1.0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    low.midpoint(high)
}

/// The CH₄ → CO transition near 500 K for Jupiter-like mixing (the plan's test; ZM14's abstract:
/// "If vertical mixing is like Jupiter's, the transition from methane to CO occurs at 500 K in a
/// planet. Sluggish vertical mixing can raise this to 600 K; but clouds or more vigorous vertical
/// mixing could lower this to 400 K"): on [`self_luminous`]'s profiles, quenched CO equals CH₄
/// within 450–550 K at `K_zz` = 10⁷ cm² s⁻¹, and the transition moves up with sluggish mixing and
/// down with vigorous (ZM14 do not say which `K_zz` their 600 K and 400 K are for, so only the order
/// is asserted at 10⁴ and 10¹⁰). ZM14's 600 K planet, by their §3.1: at `K_zz` = 10¹⁰ cm² s⁻¹ strong mixing
/// "maintains fCO ≫ fCH4" (here CO over ten times CH₄); at 10⁷ "the 600 K planet (g = 10^3
/// cm/s2) has abundant CO and relatively little CH4"; and at 10⁴ its quench point is "cool
/// enough, barely, to fall into the CH4 field, so that fCH4 > fCO". As built: the transition at
/// 494 K with `K_zz` = 10⁷, 689 K with 10⁴ and 363 K with 10¹⁰; the 600 K planet's CO ÷ CH₄ 40, 4.0
/// and 0.35.
#[test]
fn the_methane_to_co_transition_is_near_500_k_for_jupiter_like_mixing() {
    let network = network();
    let jupiter_like = transition(&network, EddyDiffusion::JUPITER_LIKE);
    let sluggish = transition(&network, cgs(1e4));
    let vigorous = transition(&network, cgs(1e10));
    assert!((450.0..550.0).contains(&jupiter_like), "{jupiter_like} K");
    assert!(
        sluggish > jupiter_like && jupiter_like > vigorous,
        "{sluggish} K > {jupiter_like} K > {vigorous} K"
    );
    let (teff, ..) = ANCHOR;
    let strong = carbon_ratio(&network, teff, cgs(1e10));
    let moderate = carbon_ratio(&network, teff, EddyDiffusion::JUPITER_LIKE);
    let weak = carbon_ratio(&network, teff, cgs(1e4));
    assert!(strong > 10.0, "{strong}");
    assert!(moderate > 1.0, "{moderate}");
    assert!((0.1..1.0).contains(&weak), "{weak}");
}

/// Above the NH₃–N₂ level, NH₃ is its level's and N₂ takes the nitrogen the others leave
/// (decision-p14-t52a-rows): at Jupiter's 1-bar level, where the floor's held equilibrium keeps
/// almost no N₂, the quenched N₂ is what ammonia's quench left, and the nitrogen is the gas's.
#[test]
fn nitrogen_takes_what_ammonia_s_quench_leaves() {
    let network = network();
    let gas = protosolar(3.0);
    let column = jupiter_column(EddyDiffusion::JUPITER_LIKE);
    let quench = Quench::derive(&network, &gas, jupiter, &column).expect("a quench");
    let (local, quenched) = quenched_at(&network, &gas, &quench, &jupiter, bar(1.0));
    assert!(quenched.is_quenched(QuenchedSystem::Ammonia));
    let level = quench.level(QuenchedSystem::Ammonia);
    let deep = network
        .solve(gas.iter().copied(), level.temperature(), level.pressure())
        .expect("the level's gas");
    let (n2, nh3) = (row("N2"), row("NH3"));
    let ratio = |a: Option<f64>, b: Option<f64>| a.expect("carried") / b.expect("carried");
    assert!(
        (ratio(quenched.mole_fraction(nh3), quenched.mole_fraction(n2))
            / ratio(deep.mole_fraction(nh3), deep.mole_fraction(n2))
            - 1.0)
            .abs()
            < 0.05
    );
    assert!(quenched.mole_fraction(n2) > local.mole_fraction(n2).map(|x| 10.0 * x));
    let nitrogen = quenched.per_hydrogen(Element::N);
    assert!((nitrogen / local.per_hydrogen(Element::N) - 1.0).abs() < 1e-12);
}

/// Every element and the charge are conserved by the quench, to 10⁻¹² of the local
/// equilibrium's, at every level of Jupiter's and of the self-luminous planets' profiles, and at
/// ten and a hundred times solar and C/O 1.5; no balancing carrier is held at zero there.
#[test]
fn elements_and_charge_are_conserved() {
    type Case = (
        Vec<(Element, f64)>,
        Box<dyn Fn(Pascals) -> Kelvin>,
        MixingColumn,
        [f64; 5],
    );
    let network = network();
    let carbon_rich = {
        let oxygen = Element::O.protosolar_abundance_per_hydrogen();
        protosolar(1.0)
            .into_iter()
            .map(|(e, n)| {
                if e == Element::C {
                    (e, 1.5 * oxygen)
                } else {
                    (e, n)
                }
            })
            .collect::<Vec<_>>()
    };
    let deep = [1e-2, 0.1, 1.0, 10.0, 100.0];
    let shallow = [1e-3, 1e-2, 0.1, 1.0, 10.0];
    let cases: [Case; 4] = [
        (
            protosolar(3.0),
            Box::new(jupiter),
            jupiter_column(EddyDiffusion::JUPITER_LIKE),
            deep,
        ),
        (
            protosolar(10.0),
            Box::new(self_luminous(700.0)),
            self_luminous_column(EddyDiffusion::JUPITER_LIKE),
            shallow,
        ),
        (
            protosolar(100.0),
            Box::new(self_luminous(900.0)),
            self_luminous_column(cgs(1e10)),
            shallow,
        ),
        (
            carbon_rich,
            Box::new(self_luminous(500.0)),
            self_luminous_column(cgs(1e4)),
            shallow,
        ),
    ];
    for (gas, profile, column, levels) in &cases {
        let quench = Quench::derive(&network, gas, profile, column).expect("a quench");
        for &p in levels {
            let (local, quenched) = quenched_at(&network, gas, &quench, profile, bar(p));
            for element in Element::ALL {
                let want = local.per_hydrogen(element);
                let got = quenched.per_hydrogen(element);
                assert!(
                    (got - want).abs() <= 1e-12 * want,
                    "{element:?} at {p} bar: {got} against {want}"
                );
            }
            let charge = |fractions: &mut dyn Iterator<Item = (SubstanceId, f64)>| {
                let mut sum = 0.0;
                for (id, x) in fractions {
                    sum += f64::from(id.substance().charge()) * x;
                }
                sum
            };
            let before = charge(&mut local.mole_fractions());
            let after = charge(&mut quenched.mole_fractions());
            assert!(
                (after - before).abs() < 1e-15,
                "{p} bar: {after} against {before}"
            );
            for key in ["N2", "H2O", "H2", "CO", "CH4"] {
                let x = quenched.mole_fraction(row(key)).expect("carried");
                let deep = local.mole_fraction(row(key)).expect("carried");
                assert!(x > 0.0 || deep <= 0.0, "{key} held at zero at {p} bar");
            }
        }
    }
}

/// The gas is continuous across each quench level: just above and just below it (10⁻⁹ in ln p)
/// every species' mole fraction agrees to 10⁻⁶ of itself.
#[test]
fn the_gas_is_continuous_across_each_quench_level() {
    let network = network();
    let gas = protosolar(1.0);
    let profile = self_luminous(700.0);
    let column = self_luminous_column(EddyDiffusion::JUPITER_LIKE);
    let quench = Quench::derive(&network, &gas, &profile, &column).expect("a quench");
    for system in QuenchedSystem::ALL {
        let level = quench.level(system);
        if level.bound() != QuenchBound::Crossing {
            continue;
        }
        let (_, above) = quenched_at(
            &network,
            &gas,
            &quench,
            &profile,
            level.pressure() * (1.0 - 1e-9),
        );
        let (_, below) = quenched_at(
            &network,
            &gas,
            &quench,
            &profile,
            level.pressure() * (1.0 + 1e-9),
        );
        assert!(above.is_quenched(system) && !below.is_quenched(system));
        for ((id, a), (_, b)) in above.mole_fractions().zip(below.mole_fractions()) {
            assert!(
                (a - b).abs() <= 1e-6 * a.max(b) + 1e-300,
                "{} at the {} level: {a} against {b}",
                id.substance().key(),
                system.name()
            );
        }
    }
}

/// The quench is continuous in its inputs: Jupiter's quenched CO at 1 bar moves by under 3% in
/// each step of 0.01 dex in `K_zz` from 10⁵ to 10¹⁰ cm² s⁻¹ (VM11's Jupiter fit goes as `K_zz`^0.443,
/// about 1% a step), and the self-luminous planet's quenched CO ÷ CH₄ by under 5% in each 1 K of
/// `T_eff` from 450 to 550 K.
#[test]
fn the_quench_is_continuous_in_its_inputs() {
    let network = network();
    let gas = protosolar(3.0);
    let co = row("CO");
    let mut previous: Option<f64> = None;
    let mut dex = 5.0;
    while dex <= 10.0 {
        let column = jupiter_column(cgs(math::exp10(dex)));
        let quench = Quench::derive(&network, &gas, jupiter, &column).expect("a quench");
        let (_, quenched) = quenched_at(&network, &gas, &quench, &jupiter, bar(1.0));
        let x = quenched.mole_fraction(co).expect("carried");
        if let Some(before) = previous {
            assert!(
                (x / before - 1.0).abs() < 0.03,
                "10^{dex}: {x} against {before}"
            );
        }
        previous = Some(x);
        dex += 0.01;
    }
    let mut previous: Option<f64> = None;
    for teff in 450..=550 {
        let ratio = carbon_ratio(&network, f64::from(teff), EddyDiffusion::JUPITER_LIKE);
        if let Some(before) = previous {
            assert!(
                (ratio / before - 1.0).abs() < 0.05,
                "{teff} K: {ratio} against {before}"
            );
        }
        previous = Some(ratio);
    }
}

/// CO₂ is in equilibrium with the quenched CO and H₂O by the registry's own K, which agrees with
/// ZM14's eq. 43, `K_CO2` = pCO pH2O ÷ (pCO2 pH2) = 18.3 exp(−2,376 ÷ T − (932 ÷ T)²), within 10%
/// over 800–2,000 K, the range their CO₂ quenches in.
#[test]
fn the_carbon_dioxide_constant_agrees_with_zahnle_and_marley_s() {
    let reaction = Reaction::new(&[
        (row("CO"), -1),
        (row("H2O"), -1),
        (row("CO2"), 1),
        (row("H2"), 1),
    ])
    .expect("balanced");
    for t in [800.0, 1_000.0, 1_300.0, 1_600.0, 2_000.0] {
        let registry = math::exp10(
            reaction
                .log10_equilibrium_constant(Kelvin::new(t))
                .expect("in the records"),
        );
        let zahnle_and_marley = 1.0 / (18.3 * math::exp(-2_376.0 / t - (932.0 / t) * (932.0 / t)));
        assert!(
            (registry / zahnle_and_marley - 1.0).abs() < 0.1,
            "{t} K: {registry} against {zahnle_and_marley}"
        );
    }
}

/// `id`'s molecules per hydrogen atom in a gas of mole fractions `fractions`.
fn per_hydrogen_atom(fractions: impl Iterator<Item = (SubstanceId, f64)>, id: SubstanceId) -> f64 {
    let mut hydrogen = 0.0;
    let mut held = 0.0;
    for (s, x) in fractions {
        let h = s
            .substance()
            .elements()
            .iter()
            .find(|&&(e, _)| e == Element::H)
            .map_or(0.0, |&(_, n)| f64::from(n));
        hydrogen += h * x;
        if s == id {
            held = x;
        }
    }
    held / hydrogen
}

/// Below the CO–CH₄ level CO₂ is the local equilibrium's; between it and CO₂'s own level it obeys
/// K (CO + H₂O ⇌ CO₂ + H₂) with the quenched CO, H₂O and H₂ at the local temperature (ZM14 §6's
/// procedure), and above, at CO₂'s level's temperature, each to 10⁻¹² per hydrogen atom; the
/// quenched CO₂ lies far above the local equilibrium's there.
#[test]
fn carbon_dioxide_quenches_against_the_quenched_carbon_monoxide() {
    let network = network();
    let gas = protosolar(1.0);
    let profile = self_luminous(700.0);
    let column = self_luminous_column(EddyDiffusion::JUPITER_LIKE);
    let quench = Quench::derive(&network, &gas, &profile, &column).expect("a quench");
    let carbon = quench.level(QuenchedSystem::CarbonMonoxide);
    let dioxide = quench.level(QuenchedSystem::CarbonDioxide);
    assert_eq!(dioxide.bound(), QuenchBound::Crossing);
    assert!(dioxide.pressure().value() < carbon.pressure().value());
    let k = |t: Kelvin| {
        math::exp10(
            quench
                .carbon_dioxide
                .log10_equilibrium_constant(t)
                .expect("in the records"),
        )
    };
    let places = quench.places;
    let frozen = &quench.frozen[0];
    let co2 = row("CO2");
    let check = |p: Pascals, t: Kelvin| {
        let (local, quenched) = quenched_at(&network, &gas, &quench, &profile, p);
        let want =
            k(t) * frozen[places.carbon_monoxide] * frozen[places.water] / frozen[places.hydrogen];
        let got = per_hydrogen_atom(quenched.mole_fractions(), co2);
        assert!(
            (got / want - 1.0).abs() < 1e-12,
            "{} Pa: {got} against {want}",
            p.value()
        );
        let equilibrium = local.mole_fraction(co2).expect("carried");
        assert!(quenched.mole_fraction(co2) > Some(equilibrium));
    };
    let middle = Pascals::new(math::exp(
        math::ln(carbon.pressure().value()).midpoint(math::ln(dioxide.pressure().value())),
    ));
    check(middle, profile(middle));
    check(dioxide.pressure() * 0.1, dioxide.temperature());
    let (local, low) = quenched_at(&network, &gas, &quench, &profile, carbon.pressure() * 1.5);
    assert!(!low.is_quenched(QuenchedSystem::CarbonMonoxide));
    let deep = per_hydrogen_atom(local.mole_fractions(), co2);
    let got = per_hydrogen_atom(low.mole_fractions(), co2);
    assert!((got / deep - 1.0).abs() < 1e-12, "{got} against {deep}");
}

/// The bounds: a system already slower than the mixing at the range's bottom quenches there, one
/// faster to its top is in equilibrium throughout, and CO₂ slower than the mixing at the CO–CH₄
/// level is pinned to it.
#[test]
fn each_bound_is_reported() {
    let network = network();
    let gas = protosolar(1.0);
    let shallow = MixingColumn {
        bottom: bar(2.0),
        ..jupiter_column(EddyDiffusion::JUPITER_LIKE)
    };
    let quench = Quench::derive(&network, &gas, jupiter, &shallow).expect("a quench");
    let carbon = quench.level(QuenchedSystem::CarbonMonoxide);
    assert_eq!(carbon.bound(), QuenchBound::Bottom);
    assert_same_bits(carbon.pressure().value(), shallow.bottom.value());
    let dioxide = quench.level(QuenchedSystem::CarbonDioxide);
    assert_eq!(dioxide.bound(), QuenchBound::Pinned);
    assert_same_bits(dioxide.pressure().value(), carbon.pressure().value());
    let hot = |_: Pascals| Kelvin::new(3_000.0);
    let level = quench_level(QuenchedSystem::CarbonMonoxide, hot, &shallow).expect("a level");
    assert_eq!(level.bound(), QuenchBound::Top);
    assert_same_bits(level.pressure().value(), shallow.top.value());
}

/// Each refusal names its cause.
#[test]
fn each_refusal_names_its_cause() {
    let good = jupiter_column(EddyDiffusion::JUPITER_LIKE);
    let refused = |column: MixingColumn| {
        quench_level(QuenchedSystem::CarbonMonoxide, jupiter, &column).expect_err("refused")
    };
    assert_eq!(
        refused(MixingColumn {
            gravity: MetresPerSecondSquared::new(0.0),
            ..good
        }),
        FindQuenchLevelError::Gravity(MetresPerSecondSquared::new(0.0))
    );
    assert_eq!(
        refused(MixingColumn {
            molar_mass_g_per_mol: f64::NAN,
            ..good
        })
        .to_string(),
        "molar mass NaN g/mol is not finite and positive"
    );
    assert_eq!(
        refused(MixingColumn {
            eddy_diffusion: EddyDiffusion::from_square_metres_per_second(-1.0),
            ..good
        }),
        FindQuenchLevelError::EddyDiffusion(EddyDiffusion::from_square_metres_per_second(-1.0))
    );
    assert_eq!(
        refused(MixingColumn {
            metallicity: f64::INFINITY,
            ..good
        }),
        FindQuenchLevelError::Metallicity(f64::INFINITY)
    );
    assert_eq!(
        refused(MixingColumn {
            top: bar(10.0),
            bottom: bar(1.0),
            ..good
        }),
        FindQuenchLevelError::Range {
            top: bar(10.0),
            bottom: bar(1.0)
        }
    );
    let broken = |p: Pascals| {
        if p.value() > 5e7 {
            Kelvin::new(f64::NAN)
        } else {
            jupiter(p)
        }
    };
    let error = quench_level(QuenchedSystem::CarbonDioxide, broken, &good).expect_err("refused");
    assert!(
        matches!(error, FindQuenchLevelError::Temperature { .. }),
        "{error}"
    );
    let network = network();
    let error = Quench::derive(&network, &protosolar(1.0), broken, &good).expect_err("refused");
    assert!(
        matches!(
            error,
            DeriveQuenchError::Level(FindQuenchLevelError::Temperature { .. })
        ),
        "{error}"
    );
    let no_hydrogen = [(Element::He, 0.1), (Element::C, 1e-4)];
    assert_eq!(
        Quench::derive(&network, &no_hydrogen, jupiter, &good),
        Err(DeriveQuenchError::Equilibrium {
            system: QuenchedSystem::CarbonMonoxide,
            error: SolveEquilibriumError::NoHydrogen
        })
    );
    let quench = Quench::derive(&network, &protosolar(1.0), jupiter, &good).expect("a quench");
    assert_eq!(
        quench.apply(&network, Kelvin::new(7_000.0), bar(1.0)),
        Err(ApplyQuenchError::Equilibrium(
            SolveEquilibriumError::AboveRecords(Kelvin::new(7_000.0))
        ))
    );
}

/// The same inputs give the same bits, and one network and quench serve levels in any order.
#[test]
fn the_quench_is_deterministic() {
    let gas = protosolar(3.0);
    let column = jupiter_column(EddyDiffusion::JUPITER_LIKE);
    let (first, second) = (network(), network());
    let once = Quench::derive(&first, &gas, jupiter, &column).expect("a quench");
    let again = Quench::derive(&second, &gas, jupiter, &column).expect("a quench");
    for system in QuenchedSystem::ALL {
        assert_same_bits(
            once.level(system).pressure().value(),
            again.level(system).pressure().value(),
        );
    }
    let levels = [1e-2, 1.0, 30.0, 300.0, 0.3];
    assert_order_independent(&levels, |&p| {
        let (_, quenched) = quenched_at(&first, &gas, &once, &jupiter, bar(p));
        format!("{quenched:?}")
    });
    for p in levels {
        let (_, a) = quenched_at(&first, &gas, &once, &jupiter, bar(p));
        let (_, b) = quenched_at(&second, &gas, &again, &jupiter, bar(p));
        for ((id, x), (other, y)) in a.mole_fractions().zip(b.mole_fractions()) {
            assert_eq!(id, other);
            assert_same_bits(x, y);
        }
    }
}

/// The quench pinned bit for bit: each system's level on Jupiter's profile and on the
/// self-luminous planet's at 700 K, and the quenched gas at 1 bar, 0.5 bar (on the planet, between
/// its CO–CH₄ and CO₂ levels) and 10 mbar. Nothing on the record calls the quench before
/// P14.T54.b, so this golden is re-blessed with the rules it pins, and with P14.T52.a's, at the
/// version in force (22, then 23), until then, with no bump (the sim-determinism skill's chemistry
/// exception, main's ruling of 2026-10-13, as for P14.T52.a's golden); a commit that moves its
/// values names that exception.
#[test]
fn the_quench_is_pinned() {
    use crate::GENERATOR_VERSION;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let network = network();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let jupiter_profile: Box<dyn Fn(Pascals) -> Kelvin> = Box::new(jupiter);
    let planet_profile: Box<dyn Fn(Pascals) -> Kelvin> = Box::new(self_luminous(700.0));
    let cases = [
        (
            "Jupiter",
            protosolar(3.0),
            jupiter_profile,
            jupiter_column(EddyDiffusion::JUPITER_LIKE),
        ),
        (
            "a 700 K self-luminous planet",
            protosolar(1.0),
            planet_profile,
            self_luminous_column(EddyDiffusion::JUPITER_LIKE),
        ),
    ];
    for (name, gas, profile, column) in &cases {
        let quench = Quench::derive(&network, gas, profile, column).expect("a quench");
        for system in QuenchedSystem::ALL {
            let level = quench.level(system);
            w.line(&format!("{name} {}: {:?}", system.name(), level.bound()));
            w.f64(
                &format!("{name} {} pressure", system.name()),
                level.pressure().value(),
            );
            w.f64(
                &format!("{name} {} temperature", system.name()),
                level.temperature().value(),
            );
        }
        for p in [1.0, 0.5, 0.01] {
            let (_, quenched) = quenched_at(&network, gas, &quench, profile, bar(p));
            for (id, x) in quenched.mole_fractions() {
                w.f64(
                    &format!("{name} {p} bar {}", id.substance().key().as_str()),
                    x,
                );
            }
        }
    }
    golden!("planetary/quench", w.as_str());
}
