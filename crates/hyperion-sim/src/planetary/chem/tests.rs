use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

use super::*;
use crate::GENERATOR_VERSION;
use crate::id::BodyId;
use crate::planetary::derive::atmosphere::{SULPHUR_WORD, VOLATILE_WORDS, VolatileDraws};

/// The systems and bodies the draws are pinned for: two bodies of one system, one of a second.
fn bodies() -> [BodyId; 3] {
    let system = |raw: u64| SystemId::from_raw(raw).expect("a valid system ID");
    [
        BodyId::new(system(0x0200_0800_2000_0000), 0x0100),
        BodyId::new(system(0x0200_0800_2000_0000), 0x0300),
        BodyId::new(system(0x0200_0800_2000_0031), 0x0101),
    ]
}

/// P14.T50.a's draws: words 3–5 of `planet.volatiles` for three bodies and words 0–1 of
/// `system.abundances` for their two systems, under four seeds, pinned by their bits (words 0–2
/// beside them, which P14.T13.a's goldens already pin through every inventory).
#[test]
fn the_draws_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for raw_seed in [0, 7, 0x0014_0023, u64::MAX] {
        let seed = Seed::new(raw_seed);
        for body in bodies() {
            let d = VolatileDraws::for_body(seed, body);
            let label = |word: &str| format!("seed {raw_seed:016x} body {body} {word}");
            for (word, rank) in [
                ("0 water", d.water),
                ("1 carbon", d.carbon),
                ("2 nitrogen", d.nitrogen),
                ("3 sulphur", d.sulphur),
                ("4 chlorine", d.chlorine),
                ("5 redox", d.redox),
            ] {
                w.f64(&label(word), rank.value());
            }
        }
        for system in [bodies()[0].system(), bodies()[2].system()] {
            let a = AbundanceDraws::for_system(seed, system);
            let label = |word: &str| format!("seed {raw_seed:016x} system {system} {word}");
            w.f64(&label("0 carbon_to_oxygen"), a.carbon_to_oxygen.value());
            w.f64(
                &label("1 magnesium_to_silicon"),
                a.magnesium_to_silicon.value(),
            );
        }
    }
    golden!("planetary/chem_draws", w.as_str());
}

/// P14.T50.a: words 0–2 are drawn as P14.T13.a drew them, three uniforms in turn, and words 3–5
/// are the stream's words at those numbers, so that no earlier value moved; all six lie inside
/// the tag's reserved eight.
#[test]
fn the_new_words_leave_the_old_ones_untouched() {
    for body in bodies() {
        let seed = Seed::new(0x0014_0023);
        let draws = VolatileDraws::for_body(seed, body);
        let mut stream = Stream::open(seed, tags::PLANET_VOLATILES, ObjectKey::from(body));
        for old in [draws.water, draws.carbon, draws.nitrogen] {
            assert_same_bits(old.value(), stream.uniform_open());
        }
        for (n, new) in [draws.sulphur, draws.chlorine, draws.redox]
            .into_iter()
            .enumerate()
        {
            let word = SULPHUR_WORD + u64::try_from(n).expect("three words");
            assert!(word < VOLATILE_WORDS);
            let mut at = Stream::open(seed, tags::PLANET_VOLATILES, ObjectKey::from(body));
            at.seek(word);
            assert_same_bits(new.value(), at.uniform_open());
        }
    }
    // The host's two ranks are words 0 and 1 of `system.abundances`, inside its reserved eight.
    for body in bodies() {
        let seed = Seed::new(0x0014_0023);
        let draws = AbundanceDraws::for_system(seed, body.system());
        for (word, rank) in [draws.carbon_to_oxygen, draws.magnesium_to_silicon]
            .into_iter()
            .enumerate()
        {
            let word = u64::try_from(word).expect("two words");
            assert!(word < ABUNDANCE_WORDS);
            let mut at = Stream::open(
                seed,
                tags::SYSTEM_ABUNDANCES,
                ObjectKey::from(body.system()),
            );
            at.seek(word);
            assert_same_bits(rank.value(), at.uniform_open());
        }
    }
    assert_same_bits(VolatileDraws::MEDIAN.sulphur.value(), 0.5);
    assert_same_bits(VolatileDraws::MEDIAN.chlorine.value(), 0.5);
    assert_same_bits(VolatileDraws::MEDIAN.redox.value(), 0.5);
}

/// P14.T50.a: two bodies of one system read one C/O and one Mg/Si; a body of another system reads
/// its own.
#[test]
fn two_bodies_of_one_system_read_one_host() {
    let seed = Seed::new(7);
    let [b, c, other] = bodies();
    assert_eq!(b.system(), c.system());
    let host = |body: BodyId| {
        HostAbundances::of(
            Dex::new(0.1),
            &AbundanceDraws::for_system(seed, body.system()),
        )
    };
    assert_same_bits(host(b).carbon_to_oxygen(), host(c).carbon_to_oxygen());
    assert_same_bits(
        host(b).magnesium_to_silicon(),
        host(c).magnesium_to_silicon(),
    );
    assert_ne!(
        AbundanceDraws::for_system(seed, b.system()),
        AbundanceDraws::for_system(seed, other.system())
    );
}

/// P14.T50.a: the same seed gives the same draws twice, and another seed others.
#[test]
fn the_same_seed_gives_the_same_draws_twice() {
    for body in bodies() {
        let (seed, other) = (Seed::new(11), Seed::new(12));
        assert_eq!(
            VolatileDraws::for_body(seed, body),
            VolatileDraws::for_body(seed, body)
        );
        assert_ne!(
            VolatileDraws::for_body(seed, body),
            VolatileDraws::for_body(other, body)
        );
        assert_eq!(
            AbundanceDraws::for_system(seed, body.system()),
            AbundanceDraws::for_system(seed, body.system())
        );
        assert_ne!(
            AbundanceDraws::for_system(seed, body.system()),
            AbundanceDraws::for_system(other, body.system())
        );
    }
}

/// The host's ratios follow the cited trends about Lodders's solar values: the median host of
/// solar metallicity 0.067 dex poorer in carbon than the Sun (Brewer and Fischer), the plateau
/// above −0.1 dex, the trend held below −1; Mg/Si 0.011 dex below solar at solar metallicity and
/// falling by 0.0873 dex per dex of \[Fe/H\] (Adibekyan et al.), held outside −0.6 to +0.4; and
/// the scatters at one sigma.
#[test]
fn the_host_ratios_follow_the_cited_trends() {
    assert!((solar_carbon_to_oxygen() - 0.457).abs() < 5e-4);
    assert!((solar_magnesium_to_silicon() - 1.023).abs() < 1e-3);
    let median = |fe_h: f64| HostAbundances::of(Dex::new(fe_h), &AbundanceDraws::MEDIAN);
    let relative = |fe_h: f64| {
        median(fe_h).log_carbon_to_oxygen().value() - math::log10(solar_carbon_to_oxygen())
    };
    assert!((relative(0.0) + 0.067).abs() < 1e-3, "{}", relative(0.0));
    // The plateau: within 0.06 dex from −0.1 to +0.6, below solar, and falling steeply below it.
    let plateau = [-0.1, 0.0, 0.2, 0.4, 0.6].map(relative);
    let (low, high) = plateau
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &r| {
            (lo.min(r), hi.max(r))
        });
    assert!(high - low < 0.06 && high < 0.0, "{plateau:?}");
    assert!(relative(-0.8) < relative(-0.4) - 0.1);
    assert_same_bits(relative(-1.5), relative(-1.0));
    assert_same_bits(relative(0.9), relative(0.6));
    for (fe_h, held) in [
        (-1.0, -0.6),
        (-0.6, -0.6),
        (0.0, 0.0),
        (0.3, 0.3),
        (0.5, 0.4),
    ] {
        let mg = median(fe_h).log_magnesium_to_silicon().value()
            - math::log10(solar_magnesium_to_silicon());
        let expected = math::log10(1.02 / 1.047) - 0.0873 * held;
        assert!((mg - expected).abs() < 1e-12, "[Fe/H] {fe_h}: {mg}");
    }
    let sigma = UnitUniform::new(0.841_344_746_068_542_9).expect("inside (0, 1)");
    let high = HostAbundances::of(
        Dex::new(0.0),
        &AbundanceDraws {
            carbon_to_oxygen: sigma,
            magnesium_to_silicon: sigma,
        },
    );
    let shift = |a: Dex, b: Dex| (a - b).value();
    assert!(
        (shift(
            high.log_carbon_to_oxygen(),
            median(0.0).log_carbon_to_oxygen()
        ) - CARBON_TO_OXYGEN_SCATTER.value())
        .abs()
            < 1e-6
    );
    assert!(
        (shift(
            high.log_magnesium_to_silicon(),
            median(0.0).log_magnesium_to_silicon()
        ) - MAGNESIUM_TO_SILICON_SCATTER.value())
        .abs()
            < 1e-6
    );
}

/// Brewer and Fischer: "none of 849 FGK dwarfs reaches C/O 0.8" (their largest is 0.66 on their
/// scale, 0.537 for the Sun). At any metallicity of their sample, the host's C/O at the 99.9th
/// percentile rank is below 0.7, and so below 0.8, on Lodders's scale, which is lower still.
#[test]
fn no_host_of_the_sample_s_range_is_carbon_rich() {
    let rank = UnitUniform::new(0.999).expect("inside (0, 1)");
    for step in 0..=32 {
        let fe_h = -1.0 + 1.6 * f64::from(step) / 32.0;
        let host = HostAbundances::of(
            Dex::new(fe_h),
            &AbundanceDraws {
                carbon_to_oxygen: rank,
                magnesium_to_silicon: rank,
            },
        );
        assert!(host.carbon_to_oxygen() < 0.7, "[Fe/H] {fe_h}");
    }
}

/// The derived values themselves, pinned by their bits: the FMQ–IW offset, each table body's ΔIW
/// and element inventory, and the host's ratios over metallicity and rank. Nothing on the record
/// reads them until P14.T54.a, so they move no other golden; a reordered sum in a later task of
/// the batch moves a line here instead of passing unseen.
#[test]
fn the_derived_values_are_pinned() {
    use crate::planetary::chem::inventory::{
        INVENTORY_ELEMENTS, InventoryInputs, Reservoir, element_inventory,
    };
    use crate::planetary::chem::redox::{RedoxInputs, delta_iw, fmq_above_iw, mean_delta_iw};
    use crate::planetary::derive::solar::{
        SOLAR_AGE, derive_historic, placed_about_the_sun, solar_disc, table_bodies,
    };
    use crate::units::Metres;
    use crate::units::consts::METRES_PER_AU;

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.f64("fmq_above_iw", fmq_above_iw().value());
    let rank = |u: f64| UnitUniform::new(u).expect("inside (0, 1)");
    let sigma = rank(0.841_344_746_068_542_9);
    let disc = solar_disc();
    for &(name, kg, km, a, e) in table_bodies() {
        let placed = placed_about_the_sun(&disc, (kg, km, a, e));
        let body = derive_historic(&placed, &disc, SOLAR_AGE);
        let formation_distance = Metres::new(a * METRES_PER_AU);
        let redox = RedoxInputs {
            formation_distance,
            snow_line: disc.snow_line(),
            water_fraction: body.fractions().water(),
            mass: body.mass(),
        };
        w.f64(
            &format!("{name} mean_delta_iw"),
            mean_delta_iw(&redox).value(),
        );
        w.f64(
            &format!("{name} delta_iw at one sigma"),
            delta_iw(&redox, sigma).value(),
        );
        let inventory = element_inventory(&InventoryInputs {
            mass: body.mass(),
            fractions: body.fractions(),
            formed: body.formed(),
            formation_distance,
            snow_line: disc.snow_line(),
            draws: VolatileDraws {
                sulphur: sigma,
                chlorine: rank(0.3),
                redox: rank(0.7),
                ..VolatileDraws::MEDIAN
            },
            age: SOLAR_AGE,
        });
        for element in INVENTORY_ELEMENTS {
            for reservoir in Reservoir::ALL {
                w.f64(
                    &format!("{name} {} {reservoir:?}", element.symbol()),
                    inventory.mass(element, reservoir).value(),
                );
            }
        }
        w.f64(
            &format!("{name} mantle_delta_iw"),
            inventory.mantle_delta_iw().value(),
        );
    }
    for fe_h in [-1.5, -0.6, -0.2, 0.0, 0.3, 0.8] {
        for u in [0.1, 0.5, 0.9] {
            let host = HostAbundances::of(
                Dex::new(fe_h),
                &AbundanceDraws {
                    carbon_to_oxygen: rank(u),
                    magnesium_to_silicon: rank(1.0 - u),
                },
            );
            let label = |what: &str| format!("host [Fe/H] {fe_h} rank {u} {what}");
            w.f64(&label("log C/O"), host.log_carbon_to_oxygen().value());
            w.f64(&label("log Mg/Si"), host.log_magnesium_to_silicon().value());
        }
    }
    golden!("planetary/chem_values", w.as_str());
}
