//! Tests of the cold traps (P14.T51.f).

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

use super::*;
use crate::id::SystemId;

/// Over 10⁴ bodies the exposed fractions are log-uniform on [0.016, 0.45] (Kolmogorov–Smirnov on
/// ln(f ÷ 0.016) ÷ ln(0.45 ÷ 0.016)), each the open uniform of word 0 of its own
/// `body.cold_traps` stream and no other word, and the same twice.
#[test]
fn the_exposed_fraction_is_log_uniform_in_its_bounds_and_reads_word_0_alone() {
    let seed = Seed::new(0x0014_0051);
    let system = SystemId::from_raw(0x0200_0800_2000_0051).unwrap();
    let span = math::ln(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR);
    let mut ranks: Vec<f64> = (0..10_000_u16)
        .map(|i| {
            let body = BodyId::new(system, 0x0100 + i);
            let f = exposed_fraction(seed, body);
            assert_same_bits(f, exposed_fraction(seed, body));
            assert!(
                (EXPOSED_FRACTION_FLOOR..=EXPOSED_FRACTION_CEILING).contains(&f),
                "{body}: {f}"
            );
            let mut stream = Stream::open(seed, tags::BODY_COLD_TRAPS, ObjectKey::from(body));
            let u = stream.uniform_open();
            assert_eq!(stream.position(), 1);
            let expected = EXPOSED_FRACTION_FLOOR
                * math::powf(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR, u);
            assert_same_bits(
                f,
                expected.clamp(EXPOSED_FRACTION_FLOOR, EXPOSED_FRACTION_CEILING),
            );
            math::ln(f / EXPOSED_FRACTION_FLOOR) / span
        })
        .collect();
    let ks = ks_one_sample(&mut ranks, |x| x.clamp(0.0, 1.0));
    assert_p_value("the exposed fractions' log ranks", ks.p_value, ALPHA);
}

/// The draw takes its own stream: a body's fraction is word 0 of `body.cold_traps` alone, not of
/// any other body tag it shares a key with, and it moves with the seed and the body.
#[test]
fn the_exposed_fraction_takes_its_own_stream() {
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let body = BodyId::new(system, 0x1300);
    let fraction = |raw_seed, body| exposed_fraction(Seed::new(raw_seed), body);
    let rank = |tag, raw_seed| {
        Stream::open(Seed::new(raw_seed), tag, ObjectKey::from(body)).uniform_open()
    };
    let of_rank = |u: f64| {
        (EXPOSED_FRACTION_FLOOR * math::powf(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR, u))
            .clamp(EXPOSED_FRACTION_FLOOR, EXPOSED_FRACTION_CEILING)
    };
    for raw_seed in [0, 7, 0x0014_0051, u64::MAX] {
        let f = fraction(raw_seed, body);
        assert_same_bits(f, of_rank(rank(tags::BODY_COLD_TRAPS, raw_seed)));
        for other in [
            tags::BODY_FORMATION,
            tags::BODY_CONTINENTS,
            tags::BODY_SURFACE,
        ] {
            assert_ne!(bits(f), bits(of_rank(rank(other, raw_seed))));
        }
    }
    assert_ne!(bits(fraction(7, body)), bits(fraction(8, body)));
    let neighbour = BodyId::new(system, 0x1301);
    assert_ne!(bits(fraction(7, body)), bits(fraction(7, neighbour)));
    assert_eq!(EXPOSED_FRACTION_WORDS, 8);
}
