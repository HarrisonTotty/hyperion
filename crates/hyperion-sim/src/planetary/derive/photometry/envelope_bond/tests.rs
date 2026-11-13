//! P14.T24.h.b part 5: the engine's closed forms, the record's own envelopes read alike, and the
//! snapshot path's closed form against them and the measured giants.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::atmosphere::AtmosphereInputs;
use crate::planetary::derive::atmosphere::Gas;
use crate::planetary::hooks::EnvelopeAtmosphere;
use crate::planetary::hooks::aerosols::AerosolDeck;
use crate::substance::aloft::law_aloft;
use crate::substance::{CondensedPhase, by_key};
use crate::units::{Kelvin, Pascals};

/// The record's atmosphere `atmosphere` under `gravity` (m s⁻²) as the engine reads it (the tests'
/// reference): its 1-bar gas throughout but CH₄, its deck's vapour below it (the 1-bar gas's where
/// it has no deck) and held to its saturation above its base, and each of its modes with a τ
/// ([`assemble`]).
pub(crate) fn record_column(atmosphere: &EnvelopeAtmosphere, gravity: f64) -> EnvelopeColumn {
    let share = |key: &str| {
        let id = by_key(key).expect("a row");
        atmosphere
            .composition()
            .iter()
            .filter(|(s, _)| *s == id)
            .map(|&(_, x)| x)
            .sum::<f64>()
    };
    let methane_deck = atmosphere
        .decks()
        .iter()
        .find(|d| d.condensate() == by_key("CH4").expect("a row"));
    let gas = Absorbers {
        hydrogen: share("H2"),
        helium: share("He"),
        methane: methane_deck.map_or_else(|| share("CH4"), AerosolDeck::vapour_below),
        water: share("H2O"),
        carbon_monoxide: share("CO"),
        sodium: share("Na"),
        potassium: share("K"),
    };
    let modes: Vec<ColumnMode> = atmosphere
        .modes()
        .iter()
        .filter_map(column::column_mode)
        .collect();
    let methane_base = methane_deck.map(|d| d.base().value());
    let methane_law = law_aloft(by_key("CH4").expect("a row")).expect("a law");
    assemble(
        ColumnFloor::Visual,
        (gravity, atmosphere.mean_molar_mass_g_per_mol()),
        &gas,
        &modes,
        |p| atmosphere.profile().temperature_at(Pascals::new(p)).value(),
        |p, t| {
            let mut layer = gas;
            if methane_base.is_some_and(|b| p < b) {
                let saturated = methane_law
                    .ln_saturation_pressure(Kelvin::new(t), Pascals::new(p), 0.0)
                    .map_or(0.0, |ln| math::exp(ln) / p);
                layer.methane = layer.methane.min(saturated);
            }
            layer
        },
    )
}

/// The Sun's light by bin, as the area mix weighs it.
fn sun() -> HostShade {
    HostShade::sun()
}

/// A conservative isotropic layer's two-stream is its closed form, R = γ₁τ ÷ (1 + γ₁τ) with γ₁ ¾,
/// and a deep absorbing one's its limit with no overflow.
#[test]
fn the_two_stream_takes_its_limits() {
    let (r, t) = layer_two_stream(2.0, 1.0, 0.0);
    assert!((r - 1.5 / 2.5).abs() < 1e-12, "{r}");
    assert!((r + t - 1.0).abs() < 1e-12);
    let (r, t) = layer_two_stream(1e6, 0.5, 0.3);
    assert!(r.is_finite() && (0.0..1.0).contains(&r), "{r}");
    assert_same_bits(t, 0.0);
    let (r, _) = layer_two_stream(1e6, 0.0, 0.0);
    assert_same_bits(r, 0.0);
}

/// A homogeneous Henyey–Greenstein layer's diffuse reflectance over black, exact: the lane's
/// doubling (`fix-dark-sub-neptune`, 2026-11-10; 48 Gauss streams a hemisphere, the azimuthal mean
/// of the phase function to 200 Legendre terms, a layer of τ ÷ 2³⁰ doubled 30 times, lit
/// isotropically): τ, ω, g and R.
const EXACT_DIFFUSE: [(f64, f64, f64, f64); 12] = [
    (17.0, 0.1, 0.05, 0.0205),
    (17.0, 0.25, 0.0, 0.0598),
    (17.0, 0.25, 0.15, 0.0511),
    (17.0, 0.35, 0.2, 0.0737),
    (17.0, 0.5, 0.2, 0.1225),
    (17.0, 0.7, 0.3, 0.2042),
    (17.0, 0.9, 0.5, 0.3602),
    (17.0, 0.99, 0.7, 0.6457),
    (1.0, 1.0, 0.0, 0.4466),
    (1.0, 0.9, 0.5, 0.2280),
    (0.3, 0.5, 0.2, 0.0689),
    (2.0, 0.2, 0.1, 0.0414),
];

/// The δ-PIFM is honest for a dark layer: on each of [`EXACT_DIFFUSE`]'s layers it lies within 10%
/// below the exact reflectance (0.5–9%), where the δ-Eddington lies up to 0.06 below it and returns
/// nothing for the darkest (soot's ω about 0.25 among them).
#[test]
fn the_closure_is_honest_for_a_dark_layer() {
    for (tau, omega, g, exact) in EXACT_DIFFUSE {
        let (pifm, _) = layer_two_stream(tau, omega, g);
        eprintln!("τ {tau} ω {omega} g {g}: exact {exact}, δ-PIFM {pifm:.4}");
        assert!(
            (0.90 * exact..=1.005 * exact).contains(&pifm),
            "τ {tau} ω {omega} g {g}: {pifm} against {exact}"
        );
    }
}

/// The column reads a haze above 1 Pa: a soot layer of τ(550) 2 between 0.1 and 0.27 Pa (Gao et
/// al.'s, of 50 nm) over a clear H₂–He gas is all of the column, which starts at its top, where it
/// darkens the blue that the gas's Rayleigh layer returns to under half.
#[test]
fn the_column_reads_a_haze_above_one_pascal() {
    let gas = Absorbers {
        hydrogen: 0.86,
        helium: 0.14,
        ..Absorbers::default()
    };
    let soot = particle_table("soot", CondensedPhase::Solid).expect("soot's table");
    let haze = ColumnMode {
        tau: 2.0,
        base: 0.1 * core::f64::consts::E,
        top: 0.1,
        exponent: 0.0,
        coverage: 1.0,
        optics: Some((table_for(soot, &gao_soot()), 5.0e-8)),
    };
    let haze_tau = |column: &EnvelopeColumn| -> f64 {
        column
            .layers
            .iter()
            .map(|layer| layer.particles[2][0])
            .sum()
    };
    let visual = ColumnFloor::Visual;
    let whole = assemble(visual, (12.0, 2.3), &gas, &[haze], |_| 600.0, |_, _| gas);
    let clear = bin_albedos(&assemble(
        visual,
        (12.0, 2.3),
        &gas,
        &[],
        |_| 600.0,
        |_, _| gas,
    ));
    assert_same_bits(whole.layers[0].top, 0.1);
    // The V bin's extinction over 550 nm's is within some per cent of 1 for 50 nm soot.
    let tau = haze_tau(&whole);
    eprintln!("the haze's extinction in the column's V bin {tau:.4}");
    assert!((1.8..=2.2).contains(&tau), "{tau}");
    let hazed = bin_albedos(&whole);
    assert!(
        hazed[1] < 0.5 * clear[1],
        "B: {} against the clear {}",
        hazed[1],
        clear[1]
    );
}

/// Gao et al.'s soot as the record draws it: a gamma of effective radius 50 nm and `v_eff` 0.05
/// (`photochem::SOOT_PARTICLES`).
fn gao_soot() -> crate::substance::SizeDistribution {
    crate::substance::SizeDistribution::gamma(crate::units::Metres::new(5.0e-8), 0.05)
        .expect("a gamma's radius and variance")
}

/// A gamma soot mode of `v_eff` 0.05 reads the narrow table, whose ω at 50 nm is the gamma's (the
/// lane's Mie sums, 0.158, 0.102 and 0.068 in B, V and R, within the table's interpolation), far
/// under the wide table's 0.22, 0.18 and 0.14; a log-normal soot, a gamma of another width and a
/// material with no narrow table read the wide one.
#[test]
fn a_gamma_soot_mode_reads_the_narrow_table() {
    use crate::substance::SizeDistribution;
    use crate::units::Metres;
    let soot = particle_table("soot", CondensedPhase::Solid).expect("soot's table");
    let haze = particle_table("ice_giant_haze", CondensedPhase::Solid).expect("its table");
    let log_normal = SizeDistribution::log_normal(Metres::new(5.0e-8), 0.6).expect("a log-normal");
    let wider = SizeDistribution::gamma(Metres::new(5.0e-8), 0.2).expect("a gamma");
    assert_eq!(table_for(soot, &gao_soot()), ParticleTable::Narrow(0));
    assert_eq!(table_for(soot, &log_normal), ParticleTable::Wide(soot));
    assert_eq!(table_for(soot, &wider), ParticleTable::Wide(soot));
    assert_eq!(table_for(haze, &gao_soot()), ParticleTable::Wide(haze));
    let omega = |table: ParticleTable, bin| {
        let at = log_bracket(table.radii(), 5.0e-8);
        particle_optics_at(table, at, bin)[1]
    };
    for (bin, narrow, wide) in [(1, 0.158, 0.221), (2, 0.102, 0.176), (3, 0.068, 0.143)] {
        let (n, w) = (
            omega(ParticleTable::Narrow(0), bin),
            omega(ParticleTable::Wide(soot), bin),
        );
        eprintln!("bin {bin}: ω narrow {n:.4} (Mie {narrow}), wide {w:.4} (Mie {wide})");
        assert!(
            (n - narrow).abs() < 0.006,
            "bin {bin}: narrow {n} against {narrow}"
        );
        assert!(
            (w - wide).abs() < 0.01,
            "bin {bin}: wide {w} against {wide}"
        );
    }
}

/// The bright haze's tholin has a narrow table and no wide one: its gamma of `v_eff` 0.05 reads it,
/// its ω at 0.1 µm 0.79, 0.87 and 0.92 in B, V and R (research-leans-50's Mie: 0.80, 0.88 and 0.92
/// at 450, 550 and 650 nm), and in another width it has no optics.
#[test]
fn the_bright_haze_reads_its_narrow_table() {
    use crate::substance::SizeDistribution;
    use crate::units::Metres;
    assert_eq!(particle_table("tholin", CondensedPhase::Solid), None);
    let tholin = SizeDistribution::gamma(Metres::new(1.0e-7), 0.05).expect("a gamma");
    let wider = SizeDistribution::gamma(Metres::new(1.0e-7), 0.2).expect("a gamma");
    let table = narrow_table_for("tholin", &tholin).expect("narrow");
    assert_eq!(table, ParticleTable::Narrow(1));
    assert_eq!(narrow_table_for("tholin", &wider), None);
    let at = log_bracket(table.radii(), 1.0e-7);
    for (bin, omega) in [(1, 0.786), (2, 0.870), (3, 0.919)] {
        let [extinction, w, _] = particle_optics_at(table, at, bin);
        assert!((w - omega).abs() < 0.002, "bin {bin}: ω {w}");
        assert!(extinction > 0.0);
    }
}

/// The bright haze's share of the hazes' light is its modes' τ ω over its and the soot's, bin by
/// bin, from their narrow tables, its coagulated mode not counted, and 0 with no bright haze; and
/// the light gives none to a column with no bright haze (the hot fixture's), which still takes its
/// colour.
#[test]
fn the_bright_haze_s_share_is_its_scattering_over_the_hazes() {
    use crate::planetary::chem::photochem::{
        HazeRule, UpperAtmosphere, bright_haze, coagulated_haze, soot,
    };
    use crate::planetary::hooks::envelope_atmosphere::haze_mode_at;
    use crate::units::MetresPerSecondSquared;
    let air = UpperAtmosphere {
        equilibrium_temperature: Kelvin::new(650.0),
        stratosphere_temperature: Kelvin::new(550.0),
        metallicity: 100.0,
        gravity: MetresPerSecondSquared::new(11.9),
        molar_mass_g_per_mol: 3.0,
    };
    let mode = |haze, weight| haze_mode_at(&haze, weight, |_| Kelvin::new(600.0)).expect("a mode");
    let soot_mode = mode(soot(&air, 0.01).expect("in range").expect("soot"), 0.72);
    let bright_mode = mode(
        bright_haze(&air, 0.01, 10.0)
            .expect("in range")
            .expect("a haze"),
        0.28,
    );
    let deep_mode = mode(
        coagulated_haze(&air, 0.01, 44.0)
            .expect("in range")
            .expect("a haze"),
        0.28,
    );
    let share = column::bright_share(&[
        (HazeRule::Soot, soot_mode),
        (HazeRule::Bright, bright_mode),
        (HazeRule::Coagulated, deep_mode),
    ]);
    let scattering = |mode| {
        let ColumnMode {
            tau,
            optics: Some((table, radius)),
            ..
        } = column::column_mode(mode).expect("a column mode")
        else {
            panic!("a mode with optics");
        };
        let at = log_bracket(table.radii(), radius);
        core::array::from_fn::<f64, OPTICS_BANDS, _>(|bin| {
            let [extinction, omega, _] = particle_optics_at(table, at, bin);
            tau * extinction * omega
        })
    };
    let (dark, bright) = (scattering(&soot_mode), scattering(&bright_mode));
    for bin in 0..OPTICS_BANDS {
        let want = bright[bin] / (dark[bin] + bright[bin]);
        assert!(
            (share[bin] - want).abs() < 1e-12,
            "bin {bin}: {} against {want}",
            share[bin]
        );
    }
    assert!(share.iter().all(|w| *w > 0.0 && *w < 1.0), "{share:?}");
    for bin in column::bright_share(&[
        (HazeRule::Soot, soot_mode),
        (HazeRule::Coagulated, deep_mode),
    ]) {
        assert_same_bits(bin, 0.0);
    }
    let (_, colour, hot) = gas_envelope_light(&sun(), &hot_jupiter_inputs());
    assert!(colour.is_some());
    assert_eq!(hot, None);
}

/// A column that holds the bright haze is read in each bin down to where that bin's own τ reaches
/// 10 ([`ColumnFloor::EachBin`]), its crossing layer cut: a deep well-mixed tholin layer in no
/// absorbing gas reads in each bin as one homogeneous layer of that bin's τ 10 (or the whole layer,
/// where it is thinner), where the visual floor reads every bin to τ(550) 10; and only a column
/// with the bright haze takes it.
#[test]
fn a_bright_hazed_column_is_read_in_each_bin_to_its_own_floor() {
    use crate::planetary::chem::photochem::HazeRule;
    let hazed = [HazeRule::Soot, HazeRule::Bright, HazeRule::Coagulated];
    assert_eq!(ColumnFloor::of(hazed), ColumnFloor::EachBin);
    assert_eq!(
        ColumnFloor::of([HazeRule::Soot, HazeRule::IceGiant]),
        ColumnFloor::Visual
    );
    let tholin = narrow::NARROW_KEYS
        .iter()
        .position(|k| *k == "tholin")
        .map(ParticleTable::Narrow)
        .expect("tholin's narrow table");
    let radius = 5.0e-7;
    let deep = ColumnMode {
        tau: 100.0,
        base: 1.0e3,
        top: 0.1,
        exponent: 0.0,
        coverage: 1.0,
        optics: Some((tholin, radius)),
    };
    let none = Absorbers::default();
    let read = |floor| {
        let column = assemble(floor, (10.0, 2.3), &none, &[deep], |_| 500.0, |_, _| none);
        bin_albedos(&column)
    };
    let (each, visual) = (read(ColumnFloor::EachBin), read(ColumnFloor::Visual));
    let at = log_bracket(tholin.radii(), radius);
    for bin in 0..OPTICS_BANDS {
        let [extinction, omega, g] = particle_optics_at(tholin, at, bin);
        // A bin whose whole layer is thinner than its floor reads it whole.
        let own_tau = (deep.tau * extinction).min(FLOOR_OPTICAL_DEPTH);
        let (own, _) = layer_two_stream(own_tau, omega, g);
        let (to_550, _) = layer_two_stream(FLOOR_OPTICAL_DEPTH * extinction, omega, g);
        assert!(
            (each[bin] - own).abs() < 1e-9,
            "bin {bin}: {} against {own}",
            each[bin]
        );
        assert!(
            (visual[bin] - to_550).abs() < 1e-9,
            "bin {bin}: {} against {to_550}",
            visual[bin]
        );
    }
}

/// The Rayleigh factor of a bin is its mean of (550 ÷ λ)^4.08: 1 for a bin at 550 nm alone, about
/// (550 ÷ 545)^4.08 for V's.
#[test]
fn the_rayleigh_factor_is_its_bin_s_mean() {
    let v = rayleigh_bin_factor(2);
    let mut mean = 0.0;
    let (low, high) = (495.0, 595.0);
    for i in 0..10_000 {
        let l = low + (high - low) * (f64::from(i) + 0.5) / 10_000.0;
        mean += math::powf(550.0 / l, 4.08) / 10_000.0;
    }
    assert!((v - mean).abs() < 1e-8, "{v} against {mean}");
}

/// A derived giant `body`'s pass inputs at its published state: its bulk, its derivation's
/// equilibrium and heated temperatures and helium, the Sun's age and light (what the closed form
/// reads; the rest, which no gas envelope's albedo reads, at neutral values).
pub(crate) fn giant_inputs(body: &crate::planetary::derive::DerivedBody) -> AtmosphereInputs {
    use crate::planetary::derive::atmosphere::{
        Crust, EnvelopeHistory, Insolation, SurfaceMaterial,
    };
    use crate::planetary::derive::solar::SOLAR_AGE;
    use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;
    use crate::units::{
        JoulesPerSquareMetre, Kilograms, Metres, MetresPerSecond, WattsPerSquareMetre, Years,
    };
    AtmosphereInputs {
        mass: Kilograms::from(body.mass()),
        radius: Metres::from(body.radius()),
        material: SurfaceMaterial::of(&body.fractions()),
        envelope_fraction: body.fractions().envelope(),
        class: body.class(),
        inventory: *body.inventory(),
        formed: body.formed(),
        equilibrium: body.equilibrium_temperature(),
        irradiation_temperature: body.equilibrium_temperature(),
        heated: body
            .effective_temperature()
            .unwrap_or(body.equilibrium_temperature()),
        xuv_fluence: JoulesPerSquareMetre::new(0.0),
        saturated_xuv_flux: WattsPerSquareMetre::new(0.0),
        insolation: Insolation::BeyondMaximumGreenhouse,
        present_insolation: Insolation::BeyondMaximumGreenhouse,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K),
        impact_speed: MetresPerSecond::new(0.0),
        xuv_saturation_time: Years::ZERO,
        envelope_history: EnvelopeHistory::NONE,
        flux: body.flux(),
        maximum_greenhouse_flux_ratio: 0.0,
        envelope_helium: body.envelope_helium().copied(),
    }
}

/// The shared fixture's hot Jupiter (`body_detail_hot_jupiter`, an HD 209458 b) as the pass reads
/// it: the fixture's mass, radius, `T_eq` and `T_eff`, the generator's fractions for its mass,
/// formed beyond the snow line, 3.1 Gyr old (Torres, Winn and Holman 2008), about the Sun's light.
pub(crate) fn hot_jupiter_inputs() -> AtmosphereInputs {
    use crate::planetary::derive::composition::{SnowLineSide, giant_composition};
    use crate::planetary::derive::solar::{found, solar_system};
    use crate::units::{EarthMasses, Kilograms, Metres, Years};
    let mass = Kilograms::new(1.300_215_349_175_194_7e27);
    let fractions = giant_composition(EarthMasses::from(mass), SnowLineSide::Beyond)
        .expect("a giant's mass")
        .fractions();
    AtmosphereInputs {
        mass,
        radius: Metres::new(97_157_628.0),
        envelope_fraction: fractions.envelope(),
        equilibrium: Kelvin::new(1_449.0),
        irradiation_temperature: Kelvin::new(1_449.0 * 2f64.sqrt()),
        heated: Kelvin::new(1_456.9),
        age: Years::new(3.1e9),
        formed: SnowLineSide::Beyond,
        envelope_helium: None,
        ..giant_inputs(found(&solar_system(), "Jupiter"))
    }
}

/// The four giants of the Solar System table and the measured Bond albedos the plan's test names
/// (P14.T24.h.b; the brief's): Jupiter's 0.503 ± 0.012 (Li et al. 2018, Nat. Commun. 9, 3709),
/// Saturn's 0.342 ± 0.030 (Hanel et al. 1983, Icarus 53, 262), Uranus's 0.300 ± 0.049 (Pearl et
/// al. 1990, Icarus 84, 12) and Neptune's 0.290 ± 0.067 (Pearl and Conrath 1991, JGR 96, 18921),
/// each from memory as plan 14 cites it, with the albedo the closed form gives each under every
/// 27 hold, pinned: part 6's chromophore and ammonia haze bring Jupiter within its measurement
/// (from part 5's 0.2859) and Saturn to 0.4017 (from 0.1899), within Wang et al.'s (2024) 0.41 ±
/// 0.02, and part 7's methane layer Uranus and Neptune within theirs (from 0.4829 and 0.5255);
/// part 8's cold-opacity law moves Saturn to 0.3882, Uranus to 0.3084 and Neptune to 0.3238, each
/// still within its measurement (Saturn's Wang et al.'s); from 31 the chromophore mixed in its
/// haze's gas moves Jupiter to 0.5013 and Saturn to 0.3077; from 32 the column read whole, whose
/// δ-PIFM returns some of the light of the absorbing g-points' layers that the δ-Eddington held at
/// R = 0, moves Jupiter to 0.5049, Saturn to 0.3186, Uranus to 0.3236 and Neptune to 0.3394:
/// Jupiter +0.2 sigma of Li et al.'s 0.503, Saturn −0.8 sigma of Hanel et al.'s 0.342 and under
/// Wang et al.'s 0.41 ± 0.02 (a finding), the ice giants +0.5 and +0.7 sigma of theirs.
const GIANTS: [(&str, (f64, f64), f64); 4] = [
    ("Jupiter", (0.503, 0.012), JUPITER),
    ("Saturn", (0.342, 0.030), 0.3186),
    ("Uranus", (0.300, 0.049), 0.3236),
    ("Neptune", (0.290, 0.067), 0.3394),
];

/// Jupiter's own Bond albedo, pinned: 0.5049 from 32, the ammonia haze's and the chromophore's
/// mixing's fit (`photochem::AMMONIA_HAZE_OPTICAL_DEPTH`, `photochem::CHROMOPHORE_MIXING_RATIO`;
/// 0.5003 to 30, 0.5013 at 31) under the column read whole.
const JUPITER: f64 = 0.5049;

/// The hot fixture's own Bond albedo about a Sun, pinned: 0.1370 (0.1323 to 31), near HD 209458 b's
/// dark dayside (its geometric albedo 0.038 ± 0.045 over 400–700 nm, Rowe et al. 2008, ApJ 689,
/// 1345, and 0.096 ± 0.016 in CHEOPS's band, Brandeker et al. 2022, A&A 659, L4, as P14.T52.h cites
/// them).
const HOT_JUPITER: f64 = 0.1370;

/// An H₂/He column over black: brighter to the blue over the visible, bin 3 within 0.01 of one
/// Rayleigh layer of its floor's τ there (the layering's and the pairs' share), dark beyond 1 µm
/// under H₂–H₂'s pairs to its 100-bar floor; a column of absorbing gas dark in the infrared.
#[test]
fn a_rayleigh_column_is_its_layer_s_two_stream() {
    let gas = Absorbers {
        hydrogen: 0.86,
        helium: 0.14,
        ..Absorbers::default()
    };
    let column = assemble(
        ColumnFloor::Visual,
        (24.79, 2.31),
        &gas,
        &[],
        |_| 150.0,
        |_, _| gas,
    );
    let bins = bin_albedos(&column);
    for pair in bins[..6].windows(2) {
        assert!(pair[0] >= pair[1], "{bins:?}");
    }
    assert!(bins[6..].iter().all(|&b| b < 0.01), "{bins:?}");
    // The floor's τ(550) is 10, so bin 3's (495–595 nm) layers hold 10 times its factor in all.
    let tau: f64 = column
        .layers
        .iter()
        .map(|l| {
            column.rayleigh_550_m2 * (l.bottom - l.top)
                / (column.molar_mass_g_per_mol / 1_000.0 * BOLTZMANN_CONSTANT / MOLAR_GAS_CONSTANT
                    * column.gravity)
        })
        .sum();
    assert!((tau - FLOOR_OPTICAL_DEPTH).abs() < 1e-6, "{tau}");
    let one = layer_two_stream(tau * rayleigh_bin_factor(2), 1.0, 0.0).0;
    assert!(
        (bins[2] - one).abs() < 0.01,
        "{} against one layer's {one}",
        bins[2]
    );
    let dark = Absorbers {
        methane: 0.5,
        ..gas
    };
    let column = assemble(
        ColumnFloor::Visual,
        (24.79, 2.31),
        &gas,
        &[],
        |_| 150.0,
        |_, _| dark,
    );
    assert!(bin_albedos(&column)[8] < 1e-3);
}

/// The visible bins' CH₄ is Karkoschka and Tomasko 2010's as the client holds it: each bin's
/// g-points' weighted mean k is the bin's mean over the client's 0.25 nm rows, uniform in
/// wavelength, at 100 K, their table's coldest (the reduction keeps a bin's mean k), within the
/// table's six figures; bin 1 holds its 380–390 nm rows over its whole 90 nm, the rest none.
#[test]
fn the_visible_methane_is_karkoschka_and_tomasko_s() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/hyperion/src/renderer/src/view/atmosphere/absorbers/crossSections.json"
    ));
    // The CH₄ table's rows, `[nm, σ(100 K), σ(198 K), σ(296 K)]`, read from its `"rows"` array.
    let table = &text[text.find("\"species\": \"CH4\"").expect("CH4's table")..];
    let rows_at = table.find("\"rows\": [").expect("its rows") + "\"rows\": [".len();
    // Each `[nm, …` row, in ascending nm until the next table's first.
    let mut rows: Vec<(f64, f64)> = Vec::new();
    for row in table[rows_at..].split('[') {
        let mut values = row.split([',', ']']).map(str::trim);
        let (Some(Ok(nm)), Some(Ok(sigma))) = (
            values.next().map(str::parse::<f64>),
            values.next().map(str::parse::<f64>),
        ) else {
            continue;
        };
        if rows.last().is_some_and(|&(last, _)| nm <= last) {
            break;
        }
        rows.push((nm, sigma));
    }
    assert_eq!(rows.len(), 1_681, "380–800 nm every 0.25 nm");
    let methane = Gas::Methane
        .substance()
        .substance()
        .molar_mass_g_per_mol()
        .expect("a mass");
    let per_kg = AVOGADRO_PER_MOL / (methane / 1_000.0);
    for bin in 0..5 {
        let (low, high) = (SPECTRAL_BIN_EDGES_NM[bin], SPECTRAL_BIN_EDGES_NM[bin + 1]);
        let sum: f64 = rows
            .iter()
            .filter(|&&(nm, _)| low <= nm && nm < high && nm < 800.0)
            .map(|&(_, sigma)| sigma * 0.25)
            .sum();
        let expected = sum / (high - low) * per_kg;
        let mean: f64 = opacity::G_WEIGHTS
            .iter()
            .enumerate()
            .map(|(g, w)| w * math::exp(opacity::METHANE[bin][g][3][0]))
            .sum();
        assert!(
            (mean - expected).abs() <= 1e-4 * expected + 1e-30,
            "bin {bin}: {mean} against {expected} m2/kg"
        );
    }
}

/// Avogadro's number, mol⁻¹: R ÷ k, both exact in the 2019 SI.
const AVOGADRO_PER_MOL: f64 = MOLAR_GAS_CONSTANT / BOLTZMANN_CONSTANT;

/// The table's giants under every 27 hold take their own columns' albedos, each pinned and stated
/// against its measurement (a miss is a finding, the outbox's), and the hot fixture its own, near
/// 0.1. Every one lies in (0, 1); Jupiter's within its measured 0.503 ± 0.012 (Li et al. 2018),
/// at an effective temperature in P14.T50.b's 123–128 K about its measured 125.57 K, and the ice
/// giants' within their measurements, Uranus's at a `T_eq` in 56–62 K about its measured 59.1 K.
#[test]
fn the_giants_own_albedos_are_pinned_against_their_measurements() {
    use crate::planetary::derive::solar::{found, solar_system};
    let own = solar_system();
    for (name, (measured, error), pinned) in GIANTS {
        let body = found(&own, name);
        let albedo = body.albedo().value();
        eprintln!(
            "{name}: own Bond albedo {albedo:.4} at T_eq {:.2} K, against {measured} ± {error} \
             measured ({:+.1} sigma)",
            body.equilibrium_temperature().value(),
            (albedo - measured) / error
        );
        assert!(
            (albedo - pinned).abs() < 0.003,
            "{name}: {albedo} against {pinned}"
        );
        assert!(0.0 < albedo && albedo < 1.0);
    }
    let jupiter = found(&own, "Jupiter");
    assert!((jupiter.albedo().value() - 0.503).abs() <= 0.012);
    let t_eff = jupiter.effective_temperature().expect("a giant's").value();
    assert!((123.0..=128.0).contains(&t_eff), "{t_eff} K");
    for (name, (measured, error), _) in &GIANTS[2..] {
        let albedo = found(&own, name).albedo().value();
        assert!((albedo - measured).abs() <= *error, "{name}: {albedo}");
    }
    let uranus = found(&own, "Uranus").equilibrium_temperature().value();
    assert!((56.0..=62.0).contains(&uranus), "{uranus} K");
    let hot = column::snapshot_bond(&hot_jupiter_inputs(), &sun()).expect("the fixture's column");
    eprintln!("the hot fixture: own Bond albedo {hot:.4}");
    assert!((hot - HOT_JUPITER).abs() < 0.003, "{hot}");
    assert!((0.05..0.2).contains(&hot), "{hot}");
}

/// The snapshot's closed form follows the record's own envelope read alike: on the table's four
/// giants and the hot fixture, both at every 27 hold's state (the record's ammonia haze and κ₁
/// part 6's), each Bond albedo about a Sun within 0.04 of the one the record's decks and hazes give
/// ([`record_column`]); their differences are the closed form's departures, stated.
#[test]
fn the_closed_form_follows_the_record_s_own_envelope() {
    use crate::planetary::derive::solar::{found, solar_system};
    use crate::planetary::system::envelope::tests::{fixture_hot_jupiter, table_envelope};
    use crate::planetary::testing::envelope_by_hand;
    let table = solar_system();
    let mut cases = Vec::new();
    for (name, ..) in GIANTS {
        let inputs = giant_inputs(found(&table, name));
        let (of, envelope) = table_envelope(name);
        let record = record_column(envelope.atmosphere(), of.gravity.value());
        cases.push((name, inputs, record));
    }
    let hot = envelope_by_hand(&fixture_hot_jupiter()).expect("an envelope");
    cases.push((
        "the hot fixture",
        hot_jupiter_inputs(),
        record_column(hot.atmosphere(), 9.193_210_521_502_017),
    ));
    for (name, inputs, record) in cases {
        let closed = (column::snapshot_bond(&inputs, &sun())).expect("a closed form");
        let read = column_bond(&record, &sun());
        eprintln!("{name}: closed form {closed:.4}, the record's decks {read:.4}");
        assert!(
            (closed - read).abs() < 0.04,
            "{name}: {closed} against {read}"
        );
    }
}

/// Mallama, Krobusek and Pavlov's (2017, Icarus 282, 19) Table 7 geometric albedos of Jupiter and
/// Saturn in B, V and R (`photometry.rs`'s templates): `p_B` ÷ `p_V` and `p_R` ÷ `p_V`, the
/// colours the cold giants' own bins are held to from 31.
const TABLE_7_COLOURS: [(&str, f64, f64); 2] = [
    ("Jupiter", 0.443 / 0.538, 0.495 / 0.538),
    ("Saturn", 0.339 / 0.499, 0.568 / 0.499),
];

/// From 31 (the cold giants' chromophore mixed in their haze's gas,
/// `photochem::CHROMOPHORE_MIXING_RATIO`, its haze at `AMMONIA_HAZE_OPTICAL_DEPTH`) the
/// table's Jupiter, the fit, takes its own Bond albedo within its measured 0.503 ± 0.012 (Li et
/// al. 2018) and its 390–495 nm bin over its 495–595 nm bin within 10% of Table 7's `p_B` ÷
/// `p_V`; Saturn, the check, its two bins over the 495–595 nm one within 15% of Table 7's `p_B` ÷
/// `p_V` and `p_R` ÷ `p_V`, and its Bond albedo is stated against Wang et al.'s (2024) 0.41 ± 0.02
/// (a miss, the lane's finding). Each figure is pinned: from 32, with the column read whole,
/// Jupiter's bins 0.8820 and 1.0379, 7.1% from Table 7's `p_B` ÷ `p_V`, and Saturn's 0.7182 and
/// 1.0639, 5.7% and 6.5% from its two (31's 0.8794 and 1.0384, and 0.7006 and 1.0648).
#[test]
fn the_cold_giants_colours_are_their_measured_ones() {
    use crate::planetary::derive::solar::{found, solar_system};
    // The pinned figures: name, Bond albedo, bin 1 ÷ bin 2 and bin 3 ÷ bin 2.
    const PINNED: [(&str, f64, f64, f64); 2] = [
        ("Jupiter", JUPITER, 0.8820, 1.0379),
        ("Saturn", 0.3186, 0.7182, 1.0639),
    ];
    let own_system = solar_system();
    for (name, bond, blue, red) in PINNED {
        let body = found(&own_system, name);
        let bins = bin_albedos(&column::snapshot_column(&giant_inputs(body)).expect("a column"));
        let (b, r) = (bins[1] / bins[2], bins[3] / bins[2]);
        let own = body.albedo().value();
        let &(_, want_b, want_r) = TABLE_7_COLOURS
            .iter()
            .find(|(n, ..)| *n == name)
            .expect("a measured colour");
        eprintln!(
            "{name}: own Bond albedo {own:.4}, bins B/V {b:.4} against {want_b:.4} and R/V \
             {r:.4} against {want_r:.4}, T_eq {:.2} K",
            body.equilibrium_temperature().value()
        );
        assert!((own - bond).abs() < 0.003, "{name}: {own} against {bond}");
        assert!((b - blue).abs() < 0.003, "{name}: B/V {b} against {blue}");
        assert!((r - red).abs() < 0.003, "{name}: R/V {r} against {red}");
        if name == "Jupiter" {
            assert!((own - 0.503).abs() <= 0.012, "{own}");
            assert!((b / want_b - 1.0).abs() <= 0.10, "{b}");
        } else {
            assert!((b / want_b - 1.0).abs() <= 0.15, "{b}");
            assert!((r / want_r - 1.0).abs() <= 0.15, "{r}");
        }
    }
}

/// The same inputs give the same bits, twice.
#[test]
fn the_same_inputs_give_the_same_bits() {
    let inputs = hot_jupiter_inputs();
    let (a, b) = {
        (
            column::snapshot_bond(&inputs, &sun()),
            column::snapshot_bond(&inputs, &sun()),
        )
    };
    assert_same_bits(a.expect("a"), b.expect("b"));
}

/// The snapshot path's cost a giant's pass, stated: the closed form's column and the engine's
/// light, each timed over 300 calls on the table's Jupiter and Neptune (this build's profile, the
/// machine's load).
#[test]
#[ignore = "slow: timing"]
fn the_snapshot_path_s_cost_a_pass_is_stated() {
    use crate::planetary::derive::solar::{found, solar_system};
    let table = solar_system();
    for (name, ..) in GIANTS {
        let inputs = giant_inputs(found(&table, name));
        {
            let column = column::snapshot_column(&inputs).expect("a column");
            let n = 300;
            let start = std::time::Instant::now();
            for _ in 0..n {
                core::hint::black_box(column::snapshot_column(&inputs));
            }
            let built = start.elapsed() / n;
            let start = std::time::Instant::now();
            for _ in 0..n {
                core::hint::black_box(bin_albedos(&column));
            }
            let light = start.elapsed() / n;
            eprintln!(
                "cost {name}: column {built:?}, light {light:?}, {} layers",
                column.layers.len()
            );
        };
    }
}
