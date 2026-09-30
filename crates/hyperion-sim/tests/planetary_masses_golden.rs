//! Golden values of plan 14's planet masses (P14.T7): the draws of chosen slots, every template
//! group's masses in chosen discs, and whether those discs grow a giant's core in time.
//!
//! They pin the arithmetic and the words drawn: a reordered sum, a changed constant or a moved
//! draw number changes a line here, which is a generator-version change. CI checks the same file
//! on 64-bit Arm and on wasm32.

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::planetary::architecture::ZoneLimit;
use hyperion_sim::planetary::architecture::template::TEMPLATES;
use hyperion_sim::planetary::disc::{self, Disc, DiscDraws, DiscHost, DiscProfile, Truncation};
use hyperion_sim::planetary::placement::masses::{
    GiantCore, MassDraws, REFERENCE_SOLID_MASS, gas_budget, giant_core, group_masses, solid_budget,
};
use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StandardNormal;
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::units::{AstronomicalUnits, Dex, HeliumExcess, Megayears, Metres, SolarMasses};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn system(cell: [i32; 3], index: u32) -> SystemId {
    SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, cell).unwrap(), index).unwrap()
}

fn planet(slot: u8) -> BodyIndex {
    BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap()
}

/// A host of plan 06's zero-age state at `mass` and `fe_h`.
fn zams_host(mass: f64, fe_h: f64) -> DiscHost {
    let mass = SolarMasses::new(mass);
    let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let coeffs = ZCoeffs::new(composition.z_fit());
    DiscHost::new(
        mass,
        composition.fe_h(),
        zams::luminosity(mass, &coeffs),
        zams::radius(mass, &coeffs),
    )
    .unwrap()
}

/// The discs the golden pins, each with its name.
fn discs() -> Vec<(&'static str, Disc)> {
    let median = |mass: f64, fe_h: f64, gas: f64, lifetime: f64| {
        let draws = DiscDraws {
            gas_fraction: StandardNormal::new(gas).unwrap(),
            ..DiscDraws::MEDIAN
        };
        disc::derive(
            &zams_host(mass, fe_h),
            Megayears::new(lifetime),
            &draws,
            Truncation::NONE,
        )
    };
    let drawn = disc::derive(
        &zams_host(0.9, 0.1),
        Megayears::new(3.2),
        &DiscDraws::for_host(Seed::new(42), system([-3, 10, 0], 7), 0),
        Truncation::NONE,
    );
    let cut = disc::derive(
        &zams_host(1.0, 0.0),
        Megayears::new(2.0),
        &DiscDraws::MEDIAN,
        Truncation::NONE.with_outer(Metres::from(AstronomicalUnits::new(6.0))),
    );
    vec![
        ("the median solar disc", median(1.0, 0.0, 0.0, 1.733)),
        ("a drawn disc of 0.9 M☉ at +0.1", drawn),
        ("a solar disc cut at 6 au", cut),
        ("a poor M dwarf's disc", median(0.3, -0.5, -1.5, 2.5)),
        ("a metal-poor solar disc", median(1.0, -0.5, 0.0, 1.733)),
        (
            "a heavy, short-lived A star's disc",
            median(1.8, 0.2, 1.0, 0.8),
        ),
        ("a light solar disc of 0.3 Myr", median(1.0, 0.0, -0.5, 0.3)),
    ]
}

#[test]
fn masses_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.f64("reference solid mass", REFERENCE_SOLID_MASS.value());

    let draw_cases = [
        (
            Seed::new(0x0123_4567_89ab_cdef),
            system([652, -4_584, 2_047], 7),
            1,
        ),
        (
            Seed::new(0x0123_4567_89ab_cdef),
            system([652, -4_584, 2_047], 7),
            2,
        ),
        (Seed::new(42), system([-3, 10, 0], 0), 1),
        (Seed::new(42), system([-3, 10, 0], 0), 191),
    ];
    for (seed, id, slot) in draw_cases {
        w.line("");
        w.line(&format!("draws seed {seed} system {id} slot {slot}"));
        let draws = MassDraws::for_planet(seed, id, planet(slot));
        w.f64("scatter", draws.scatter.value());
        w.f64("group", draws.group.value());
        w.f64("rank", draws.rank.value());
    }
    let second = BodyIndex::new(BodySlot::SecondGeneration(3), BodySub::Primary).unwrap();
    let draws = MassDraws::for_planet(Seed::new(42), system([-3, 10, 0], 0), second);
    w.line("");
    w.line("draws seed 42 second-generation slot 3");
    w.f64("scatter", draws.scatter.value());

    let (seed, id) = (Seed::new(42), system([-3, 10, 0], 7));
    for (name, disc) in discs() {
        let profile: &DiscProfile = disc.profile().expect("each pinned disc is present");
        w.line("");
        w.line(&format!("disc: {name}"));
        w.f64("solid budget", solid_budget(profile).value());
        w.f64("gas budget", gas_budget(profile).value());
        match giant_core(&disc, ZoneLimit::Unbounded) {
            GiantCore::Forms { by } => w.f64("giant core forms by (Myr)", by.value()),
            GiantCore::TooSlow { by } => w.f64("giant core too slow, by (Myr)", by.value()),
            other @ (GiantCore::NoDisc | GiantCore::TooFewSolids) => {
                w.line(&format!("giant core: {other:?}"));
            }
        }
        for template in &TEMPLATES {
            for (g, group) in template.groups().iter().enumerate() {
                for count in [1_u8, 4] {
                    let first = 1 + 10 * u8::try_from(g).unwrap();
                    let members: Vec<BodyIndex> = (first..first + count).map(planet).collect();
                    let masses = group_masses(seed, id, group, profile, &members);
                    let label = format!("{:?} group {g} ×{count}", template.class());
                    if let Some(m_c) = masses.characteristic() {
                        w.f64(&format!("{label} characteristic"), m_c.value());
                    }
                    w.line(&format!("{label} cap {:?}", masses.cap()));
                    for (i, mass) in masses.masses().iter().enumerate() {
                        w.f64(&format!("{label} [{i}]"), mass.value());
                    }
                }
            }
        }
    }
    golden!("planetary/masses", w.as_str());
}
