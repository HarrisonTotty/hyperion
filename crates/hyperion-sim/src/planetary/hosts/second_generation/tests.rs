//! P14.T28.e's second-generation planets: the hold, the draws, the share and the template.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::coords::{CellSize, GenCell};
use crate::id::Layer;

/// System `index` of a run of cells, a thousand to a cell.
fn system(index: u32) -> SystemId {
    let column = i32::try_from(index / 1_000).expect("a small index");
    let cell = GenCell::new(CellSize::Ly8, [5, column, -3]).expect("a cell of the grid");
    SystemId::from_parts(Layer::A, cell, index % 1_000).expect("a system of the cell")
}

#[test]
fn the_draws_are_the_host_s_words_of_the_system_s_stream() {
    let seed = Seed::new(0x0014_0028_e000_0001);
    let a = SecondGenerationDraws::for_host(seed, system(3), 2);
    assert_eq!(a, SecondGenerationDraws::for_host(seed, system(3), 2));
    assert_ne!(a, SecondGenerationDraws::for_host(seed, system(3), 1));
    assert_ne!(a, SecondGenerationDraws::for_host(seed, system(4), 2));
    let mut stream = Stream::open(seed, tags::PLANET_SECONDGEN, ObjectKey::from(system(3)));
    stream.seek(32);
    assert_eq!(a.mark, Mark::from_word(stream.next_u64()));
    assert_same_bits(a.count.value(), stream.uniform_open());
    assert_same_bits(a.plane.cos_inclination, 1.0 - 2.0 * stream.uniform_open());
}

/// The count's rank maps to 1, 2 or 3 in thirds.
#[test]
fn a_system_draws_one_to_three_bodies() {
    let count = |u: f64| {
        let mut draws = SecondGenerationDraws::for_host(Seed::new(1), system(1), 0);
        draws.count = UnitUniform::new(u).expect("a rank");
        draws.drawn_count()
    };
    assert_eq!(count(1e-9), 1);
    assert_eq!(count(0.33), 1);
    assert_eq!(count(0.34), 2);
    assert_eq!(count(0.66), 2);
    assert_eq!(count(0.67), 3);
    assert_eq!(count(1.0 - 1e-9), 3);
}

/// P14.T28.e's test (e), its share: 0.005 of neutron stars, to a Poisson interval (4σ), so under
/// 1%.
#[test]
fn half_a_per_cent_of_neutron_stars_have_second_generation_planets() {
    let seed = Seed::new(0x0014_0028_e000_0002);
    let n = 200_000_u32;
    let with = (0..n)
        .filter(|&i| SecondGenerationDraws::for_host(seed, system(i), 0).has_system())
        .count();
    let with = f64::from(u32::try_from(with).expect("fits"));
    let expected = SECOND_GENERATION_PROBABILITY * f64::from(n);
    assert!(
        (with - expected).abs() < 4.0 * expected.sqrt(),
        "{with} of {n} against {expected}"
    );
    assert!(with / f64::from(n) < 0.01);
}

/// The template: one to three rocky bodies at 0.2–0.5 au of 0.02–4.3 M⊕, in slots from `0xC0`,
/// each clear of the one inside it by P14.T6's floor, none crossing another, close to one plane.
#[test]
fn the_template_places_up_to_three_rocky_bodies_at_a_fifth_to_half_an_au() {
    let seed = Seed::new(0x0014_0028_e000_0003);
    let ns = SolarMasses::new(1.4);
    let au = |x: f64| Metres::from(AstronomicalUnits::new(x)).value();
    let mut systems = 0;
    let mut counts = [0_u32; 4];
    for i in 0..400_000_u32 {
        let draws = SecondGenerationDraws::for_host(seed, system(i), 0);
        let placed = place_second_generation(seed, system(i), 0, ns, 0);
        assert_eq!(placed.is_empty(), !draws.has_system());
        if placed.is_empty() {
            continue;
        }
        systems += 1;
        counts[placed.len()] += 1;
        assert!(placed.len() <= usize::from(draws.drawn_count()));
        for (k, planet) in placed.iter().enumerate() {
            assert_eq!(
                planet.index().slot(),
                BodySlot::SecondGeneration(u8::try_from(k).expect("small"))
            );
            assert_eq!(planet.index().sub(), BodySub::Primary);
            let a = planet.orbit().semi_major_axis().value();
            assert!(au(0.2) * (1.0 - 1e-12) <= a && a <= au(0.5) * (1.0 + 1e-12));
            let m = planet.mass().value();
            assert!((0.02 * (1.0 - 1e-12)..=4.3 * (1.0 + 1e-12)).contains(&m));
            assert!(planet.orbit().eccentricity().value() < 0.3);
        }
        for pair in placed.windows(2) {
            let (inner, outer) = (pair[0].orbit(), pair[1].orbit());
            let spacing = (outer.semi_major_axis() - inner.semi_major_axis())
                / mutual_hill_radius(
                    pair[0].mass(),
                    pair[1].mass(),
                    ns,
                    inner.semi_major_axis(),
                    outer.semi_major_axis(),
                );
            assert!(spacing >= spacing_floor(pair[0].mass(), pair[1].mass(), 0.0, 0.0));
            assert!(
                inner.apoapsis() <= outer.periapsis(),
                "orbits of {:?} cross",
                system(i)
            );
        }
    }
    assert!(systems > 1_500, "{systems} systems");
    assert!(
        counts[1] > 0 && counts[2] > 0 && counts[3] > 0,
        "{counts:?}"
    );
    // From slot 0xCE only two slots are free.
    let full = (0..400_000_u32)
        .find(|&i| {
            SecondGenerationDraws::for_host(seed, system(i), 0).drawn_count() == 3
                && place_second_generation(seed, system(i), 0, ns, 0).len() == 3
        })
        .expect("a system of three");
    assert_eq!(
        place_second_generation(seed, system(full), 0, ns, 14).len(),
        2
    );
}
