//! Golden values of the classification and photometry of a fixed set of states (plan 06, P06.T23
//! and P06.T20.b).
//!
//! Nothing generated calls the classifier yet, so this golden pins it on its own: each state is
//! built from explicit values (the plan's named stars, catalogue stars from their published
//! temperature, luminosity and mass, every luminosity class, white dwarfs along a cooling track
//! with pinned draws, and the remnants without light). A changed line means a table, a scale, a
//! boundary or a threshold moved, which is a generator-version change once the classification
//! reaches generated output.

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{BodyId, Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::rng::Mark;
use hyperion_sim::stellar::classify::{ClassExtras, SpectralType, classify};
use hyperion_sim::stellar::draws::{StarDraws, StarDrawsParts};
use hyperion_sim::stellar::photometry::{
    absolute_magnitude_v, bolometric_correction_v, colour_b_v,
};
use hyperion_sim::stellar::{Composition, Phase, StarState, StarStateParts};
use hyperion_sim::units::{
    Dex, HeliumExcess, SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// A state of `phase`, mass `m` (M☉), luminosity `l` (L☉) and radius `r` (R☉).
fn state(phase: Phase, m: f64, l: f64, r: f64) -> StarState {
    let mass = SolarMasses::new(m);
    StarState::new(StarStateParts {
        phase,
        age: Years::new(1e9),
        mass,
        core_mass: if phase.is_remnant() {
            mass
        } else {
            SolarMasses::new(0.1 * m)
        },
        luminosity: SolarLuminosities::new(l),
        radius: SolarRadii::new(r),
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.5,
    })
}

/// A living star of `phase` and mass `m` (M☉) at luminosity `l` (L☉) and effective temperature
/// `t` (K).
fn star(phase: Phase, m: f64, l: f64, t: f64) -> StarState {
    let r = l.sqrt() * (5_772.0 / t) * (5_772.0 / t);
    state(phase, m, l, r)
}

/// The mark of the uniform `u`: the top 53 bits of the word `u × 2⁶⁴`.
fn mark(u: f64) -> Mark {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "u is in (0, 1), so u × 2⁶⁴ fits a u64"
    )]
    let word = (u * 18_446_744_073_709_551_616.0) as u64;
    Mark::from_word(word)
}

fn write(w: &mut GoldenWriter, label: &str, s: &StarState, fe_h: f64, draws: &StarDraws) {
    let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let class = classify(s, &composition, draws, &ClassExtras::NONE);
    w.line(&format!("{label}: {class}"));
    match class.spectral_type() {
        SpectralType::Sequence(code) => w.f64(&format!("{label}.code"), code.value()),
        SpectralType::WhiteDwarf(wd) => {
            if let Some(index) = wd.temperature_index() {
                w.f64(&format!("{label}.index"), index);
            }
        }
        SpectralType::NeutronStar(_) | SpectralType::BlackHole | SpectralType::NoRemnant => {}
    }
    let teff = s.effective_temperature();
    if let Some(bc) = bolometric_correction_v(teff) {
        w.f64(&format!("{label}.bc_v"), bc.value());
    }
    if let Some(colour) = colour_b_v(teff) {
        w.f64(&format!("{label}.b_v"), colour.value());
    }
    if let Some(m_v) = absolute_magnitude_v(s) {
        w.f64(&format!("{label}.m_v"), m_v.value());
    }
}

/// The living stars: (label, phase, M☉, L☉, effective temperature in K, [Fe/H]).
const LIVING: [(&str, Phase, f64, f64, f64, f64); 25] = [
    ("sun", Phase::MainSequence, 1.0, 1.0, 5_772.0, 0.0),
    ("vega", Phase::MainSequence, 2.1, 44.0, 9_600.0, 0.0),
    (
        "arcturus",
        Phase::FirstGiantBranch,
        1.1,
        185.0,
        4_300.0,
        -0.5,
    ),
    (
        "red_supergiant",
        Phase::CoreHeliumBurning,
        15.0,
        1e5,
        3_600.0,
        0.0,
    ),
    ("sirius", Phase::MainSequence, 2.06, 25.4, 9_940.0, 0.0),
    ("procyon", Phase::MainSequence, 1.5, 6.9, 6_530.0, 0.0),
    ("pollux", Phase::CoreHeliumBurning, 1.9, 43.0, 4_666.0, 0.0),
    (
        "aldebaran",
        Phase::FirstGiantBranch,
        1.16,
        440.0,
        3_900.0,
        0.0,
    ),
    (
        "rigel",
        Phase::CoreHeliumBurning,
        21.0,
        1.2e5,
        12_100.0,
        0.0,
    ),
    (
        "deneb",
        Phase::CoreHeliumBurning,
        19.0,
        1.96e5,
        8_525.0,
        0.0,
    ),
    (
        "polaris",
        Phase::CoreHeliumBurning,
        5.4,
        2_500.0,
        6_015.0,
        0.0,
    ),
    ("spica", Phase::MainSequence, 11.4, 20_500.0, 25_300.0, 0.0),
    (
        "mira",
        Phase::ThermallyPulsingAgb,
        1.2,
        8_400.0,
        3_000.0,
        0.0,
    ),
    ("o_dwarf", Phase::MainSequence, 40.0, 2e5, 42_000.0, 0.0),
    (
        "yellow_hypergiant",
        Phase::CoreHeliumBurning,
        30.0,
        7e5,
        7_000.0,
        0.0,
    ),
    ("subgiant", Phase::HertzsprungGap, 1.3, 4.0, 5_400.0, 0.0),
    ("t_tauri", Phase::PreMainSequence, 1.0, 2.5, 4_300.0, 0.0),
    ("m_subdwarf", Phase::MainSequence, 0.3, 0.012, 3_430.0, -1.2),
    (
        "k_extreme_subdwarf",
        Phase::MainSequence,
        0.55,
        0.06,
        4_100.0,
        -2.0,
    ),
    (
        "late_m_dwarf",
        Phase::MainSequence,
        0.09,
        5e-4,
        2_380.0,
        0.0,
    ),
    ("l_dwarf", Phase::Substellar, 0.07, 2e-4, 1_920.0, 0.0),
    ("t_dwarf", Phase::Substellar, 0.04, 1e-5, 950.0, 0.0),
    ("y_dwarf", Phase::Substellar, 0.01, 1e-7, 350.0, 0.0),
    (
        "helium_star",
        Phase::HeliumMainSequence,
        10.0,
        1e5,
        90_000.0,
        0.0,
    ),
    ("post_agb", Phase::PostAgb, 0.6, 5e3, 30_000.0, 0.0),
];

/// White dwarfs along a cooling track under fixed marks, and one with drawn marks.
fn write_white_dwarfs(w: &mut GoldenWriter) {
    // White dwarfs of 0.0125 R☉ from 120,000 to 4,500 K, under marks that give each kind of
    // atmosphere history: always helium, helium then carbon, hydrogen with metals, hydrogen.
    let marks = [
        (0.03, 0.9, 0.9),
        (0.2, 0.2, 0.9),
        (0.5, 0.9, 0.05),
        (0.95, 0.9, 0.9),
    ];
    for (i, (atmosphere, carbon, metals)) in marks.into_iter().enumerate() {
        let draws = StarDraws::from_parts(StarDrawsParts {
            wd_atmosphere: mark(atmosphere),
            wd_carbon: mark(carbon),
            wd_metals: mark(metals),
            ..StarDrawsParts::MEDIAN
        });
        for t in [
            120_000.0, 60_000.0, 35_000.0, 20_000.0, 12_000.0, 8_500.0, 6_000.0, 4_500.0,
        ] {
            let r = 0.0125;
            let l = r * r * math::powi(t / 5_772.0, 4);
            let wd = state(Phase::CarbonOxygenWhiteDwarf, 0.6, l, r);
            write(w, &format!("white_dwarf_{i}_{t}"), &wd, 0.0, &draws);
        }
    }
    let cell = GenCell::new(CellSize::Ly8, [12, -40, 3]).unwrap();
    let body = BodyId::new(SystemId::from_parts(Layer::A, cell, 5).unwrap(), 0);
    let drawn = StarDraws::for_star(Seed::new(42), body);
    let wd = state(Phase::HeliumWhiteDwarf, 0.3, 1e-3, 0.02);
    write(w, "helium_white_dwarf_drawn", &wd, 0.0, &drawn);
}

#[test]
fn classifications_are_pinned() {
    let median = StarDraws::median();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (label, phase, m, l, t, fe_h) in LIVING {
        write(&mut w, label, &star(phase, m, l, t), fe_h, &median);
    }
    write_white_dwarfs(&mut w);
    write(
        &mut w,
        "neutron_star",
        &state(Phase::NeutronStar, 1.4, 0.0, 1.75e-5),
        0.0,
        &median,
    );
    write(
        &mut w,
        "black_hole",
        &state(Phase::BlackHole, 10.0, 0.0, 4.2e-5),
        0.0,
        &median,
    );
    let nothing = StarState::new(StarStateParts {
        phase: Phase::NoRemnant,
        age: Years::new(1e10),
        mass: SolarMasses::ZERO,
        core_mass: SolarMasses::ZERO,
        luminosity: SolarLuminosities::ZERO,
        radius: SolarRadii::ZERO,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.0,
    });
    write(&mut w, "no_remnant", &nothing, 0.0, &median);
    golden!("stellar/classify", w.as_str());
}
