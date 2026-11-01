//! The lifetime-only build pinned bit for bit over a sweep (plan 08, P08.T12's layer-E fates): the
//! death age and the fate of `lifetime_of` and `fate_of` at 2,000 masses, log-spaced over the
//! track's 0.1–150 M☉, at five compositions and two draws, under the stellar rules of versions 24,
//! 25, 26 and 27, digested. A change that makes the build cheaper must leave every bit; one that
//! moves a bit fails here, whichever rule it moves it under.

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::threads::in_shares;

use super::*;
use crate::GeneratorVersion;
use crate::id::{BodyId, SystemId};
use crate::rng::Seed;
use crate::units::{Dex, HeliumExcess};

/// The masses of the sweep, M☉: 2,000, log-spaced from 0.1 to 150.
fn masses() -> Vec<f64> {
    let (lo, hi) = (crate::math::ln(0.1), crate::math::ln(150.0));
    (0..2_000)
        .map(|k| crate::math::exp(lo + (hi - lo) * f64::from(k) / 1_999.0).clamp(0.1, 150.0))
        .collect()
}

/// The sweep's compositions: the solar one and [Fe/H] −2.5, −1, −0.5 and +0.3, one with a helium
/// excess.
fn compositions() -> [Composition; 5] {
    [
        Composition::SOLAR,
        Composition::from_fe_h(Dex::new(-2.5), HeliumExcess::ZERO),
        Composition::from_fe_h(Dex::new(-1.0), HeliumExcess::new(0.04)),
        Composition::from_fe_h(Dex::new(-0.5), HeliumExcess::ZERO),
        Composition::from_fe_h(Dex::new(0.3), HeliumExcess::ZERO),
    ]
}

/// The digest of every death age and fate of the sweep under `rules`: each fate's `Debug` text,
/// which prints every float to its shortest round-trip form, folded with the lifetime's bits.
fn sweep_digest(rules: StellarRules) -> u64 {
    let masses = masses();
    let draws = [
        StarDraws::median(),
        StarDraws::for_star(
            Seed::new(0x0812_e000_0000_0001),
            BodyId::new(
                SystemId::from_raw(0x4200_2cb2_0000_0001).expect("a system ID"),
                0,
            ),
        ),
    ];
    let compositions = compositions();
    let options = TrackOptions::default().with_rules(rules);
    let parts = in_shares(compositions.len(), 0, |i| {
        let comp = &compositions[i];
        let mut values = Vec::with_capacity(masses.len() * draws.len() * 2);
        for &m in &masses {
            for d in &draws {
                let m = SolarMasses::new(m);
                let fate = fate_of(m, comp, d, options);
                let life = lifetime_of(m, comp, d, options);
                assert_same_bits(life.value(), fate.death.age().value());
                values.push(life.value());
                let text = format!("{fate:?}");
                values.push(f64::from_bits(
                    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
                    }),
                ));
            }
        }
        f64_digest(&values)
    });
    f64_digest(&parts.into_iter().map(f64::from_bits).collect::<Vec<_>>())
}

/// The digests, under the stellar rules of versions 24 to 27, pinned before the lifetime-only
/// build was made cheaper.
#[test]
#[ignore = "slow: 80,000 builds at each of four rules, some 66 s on five threads in a pod"]
fn the_lifetime_build_keeps_every_bit_over_the_sweep() {
    let digests: Vec<u64> = [24, 25, 26, 27]
        .map(|v| sweep_digest(StellarRules::of_version(GeneratorVersion::new(v))))
        .to_vec();
    assert_eq!(digests, PINNED);
}

/// The sweep's digests at versions 24, 25, 26 and 27.
const PINNED: [u64; 4] = [
    0xf005_6673_608f_0cb7,
    0xf005_6673_608f_0cb7,
    0x10a6_d6ad_cf73_892e,
    0x10a6_d6ad_cf73_892e,
];
