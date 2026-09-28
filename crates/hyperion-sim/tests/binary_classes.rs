//! Plan 11's P11.T5, slow: every binary class is reached by pairs drawn from the multiplicity
//! model's priors, over mixed populations.
//!
//! Each pair is drawn from the prior inside one of the five layers' bands of primary mass
//! (Chabrier's system function there, then the model's period, mass-ratio and eccentricity laws at
//! that mass), at a metallicity uniform in [Fe/H] from −1.5 to +0.4, which spans the halo, the thick
//! and thin discs and the bulge, with each star's plan 06 draws from its own seed. A pair that can
//! interact within the age of the universe is evolved to it and classified at every segment's start
//! and midpoint. The layers are sampled in fixed numbers rather than in the mass function's own
//! proportions, since a massive pair costs some thirty times a light one and the rarest classes
//! (double neutron stars, millisecond pulsars) come from layers D and E.

use std::collections::BTreeSet;

use hyperion_sim::Seed;
use hyperion_sim::coords::GenCell;
use hyperion_sim::galaxy::imf::{Chabrier, MassFunction};
use hyperion_sim::id::{BodyId, Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::binary::{
    AGE_OF_UNIVERSE, BinaryClass, BinaryInput, BinaryTimeline, can_interact, evolve,
};
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::multiplicity::MultiplicityModel;
use hyperion_sim::units::{
    Days, Dex, GravitationalParameter, HeliumExcess, Radians, Seconds, SolarMasses, Years,
};
use hyperion_testkit::lcg::Lcg;

/// The five layers' bands of primary initial mass, M☉, and the pairs drawn in each.
const LAYERS: [(f64, f64, u64); 5] = [
    (0.08, 0.5, 200_000),
    (0.5, 0.75, 100_000),
    (0.75, 2.5, 20_000),
    (2.5, 8.0, 10_000),
    (8.0, 150.0, 30_000),
];

/// The seed of the sample.
const SEED: u64 = 0x0b1a_5eed;

/// Pair `index` of the band `lo`–`hi` M☉, from its own generator.
fn prior_pair(band: usize, index: u64, (lo, hi): (f64, f64)) -> BinaryInput {
    let mut lcg = Lcg::new(SEED ^ (u64::try_from(band).expect("a band") << 40) ^ index);
    let model = MultiplicityModel::default_v1();
    let m1 = Chabrier::provisional().quantile_in(lo, hi, lcg.next_f64());
    let periods = model.period_distribution(SolarMasses::new(m1));
    let period = Days::new(math::exp10(periods.quantile(lcg.next_f64())));
    let q = model
        .mass_ratio_distribution(SolarMasses::new(m1), period)
        .quantile(lcg.next_f64());
    let m2 = (q * m1).max(0.08);
    let e = model
        .eccentricity_distribution(SolarMasses::new(m1), period)
        .quantile(lcg.next_f64())
        .min(0.95);
    let fe_h = -1.5 + 1.9 * lcg.next_f64();
    let orbit = KeplerElements::from_period(
        Seconds::from(period),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::new(e).expect("an eccentricity below 1"),
        Orientation::new(
            Radians::new(math::acos(1.0 - 2.0 * lcg.next_f64())),
            Radians::new(core::f64::consts::TAU * lcg.next_f64()),
            Radians::new(core::f64::consts::TAU * lcg.next_f64()),
        )
        .expect("an orientation"),
        Radians::new(core::f64::consts::TAU * lcg.next_f64()),
    )
    .expect("an orbit");
    let cell = GenCell::new(Layer::E.cell_size(), [0, 0, 0]).expect("a cell");
    let system = SystemId::from_parts(Layer::E, cell, 0).expect("a system");
    let seed = Seed::new(lcg.next_u64());
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(0.0)),
        orbit,
        [
            StarDraws::for_star(seed, BodyId::new(system, 0)),
            StarDraws::for_star(seed, BodyId::new(system, 1)),
        ],
        Years::new(1.0e10),
    )
    .expect("a pair")
}

/// The classes a timeline passes through, read at each segment's start and midpoint.
fn classes_of(timeline: &BinaryTimeline) -> Vec<BinaryClass> {
    timeline
        .segments()
        .iter()
        .flat_map(|s| {
            let end = s.end().value().min(timeline.until().value());
            [s.start(), Years::new(f64::midpoint(s.start().value(), end))]
        })
        .map(|age| timeline.class_at(age))
        .collect()
}

#[test]
#[ignore = "slow: 360,000 prior-sampled binaries, the interacting ones run through the engine"]
fn every_class_is_reached_by_prior_sampled_binaries_of_mixed_populations() {
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let jobs: Vec<(usize, u64)> = LAYERS
        .iter()
        .enumerate()
        .flat_map(|(band, &(_, _, n))| (0..n).map(move |i| (band, i)))
        .collect();
    let jobs = &jobs;
    // The pairs are shared out among threads; each collects the classes it sees, and the union is
    // the same on any number of threads.
    let reached: BTreeSet<BinaryClass> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                scope.spawn(move || {
                    let mut seen = BTreeSet::new();
                    for &(band, index) in jobs.iter().skip(k).step_by(threads) {
                        let (lo, hi, _) = LAYERS[band];
                        let input = prior_pair(band, index, (lo, hi));
                        if !can_interact(&input, AGE_OF_UNIVERSE) {
                            continue;
                        }
                        seen.extend(classes_of(&evolve(&input, AGE_OF_UNIVERSE)));
                    }
                    seen
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("a worker finishes"))
            .collect()
    });
    let missing: Vec<BinaryClass> = BinaryClass::ALL
        .into_iter()
        .filter(|class| !reached.contains(class))
        .collect();
    assert!(missing.is_empty(), "never reached: {missing:?}");
}
