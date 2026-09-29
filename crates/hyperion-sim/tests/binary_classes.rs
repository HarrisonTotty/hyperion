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
//!
//! A primary heavy enough to carry plan 06's companion-stripped mark takes it as P11.T1.d will
//! draw it (ruling 123.5): set exactly where the pair's periastron lies in plan 08's stripping band
//! (`binarity::stripping_band`), by redrawing the primary's draws at the next attempt until the
//! mark agrees (ruling 129.4d). Otherwise its first explosions would not be the generator's.

use std::collections::BTreeSet;

use hyperion_sim::Seed;
use hyperion_sim::coords::GenCell;
use hyperion_sim::galaxy::displaced::binarity::stripping_band;
use hyperion_sim::galaxy::imf::{Chabrier, MassFunction};
use hyperion_sim::id::{BodyId, Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation, peters_merger_time};
use hyperion_sim::stellar::binary::{
    AGE_OF_UNIVERSE, BinaryClass, BinaryInput, BinaryTimeline, Component, SupernovaRecord,
    can_interact, evolve,
};
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::multiplicity::{MultiplicityModel, stripped_mark_min_mass};
use hyperion_sim::stellar::remnant::{KickLawParams, RemnantKind};
use hyperion_sim::stellar::{Composition, Phase};
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

/// The most attempts of a primary's draws tried for a stripped mark that agrees with its orbit: a
/// mark is set with probability 0.25, so all of them miss with probability 0.75²⁵⁶ ≈ 10⁻³².
const MAX_MARK_ATTEMPTS: u32 = 256;

/// The layer-E pairs of the double-neutron-star census (ruling 129.4e).
const DNS_PAIRS: u64 = 12_000;

/// The window of bound double neutron stars among [`DNS_PAIRS`] layer-E pairs (rulings 129.4e and
/// 132.2): Vigna-Gómez et al.'s (2018, section 3.1.1) 0.13% of binaries with primaries of 5–100
/// M☉, about 0.24% of those above 8 M☉ under Kroupa's slope, within a factor of five: 0.05–1.2% a
/// pair.
const DNS_WINDOW: (u64, u64) = (6, 144);

/// The share of those double neutron stars at e < 0.3 (ruling 96.3; observed 60–67%, Andrews and
/// Mandel 2019, Table 1, and 62%, Grichener et al. 2026).
const DNS_CIRCULAR_SHARE: (f64, f64) = (0.5, 0.9);

/// Vigna-Gómez et al.'s (2018) fiducial rate of merging double neutron stars in the Galaxy, per
/// Myr (their Table 2), and the share of their double neutron stars that merge (f = 0.73), which
/// scale the census's merging yield to a Galactic rate (ruling 132.2).
const VG18_MERGING_RATE: f64 = 24.04;
/// See [`VG18_MERGING_RATE`].
const VG18_MERGING_SHARE: f64 = 0.73;
/// VG18's double neutron stars per binary with a primary of 8 M☉ or more (ruling 129.4).
const VG18_YIELD: f64 = 0.0024;

/// Pol, McLaughlin and Lorimer's (2019) Galactic merger rate of double neutron stars at 90%
/// confidence, per Myr: 42 (+30 −14). A rate above twice the upper limit re-opens the common
/// envelope's α λ with an advisor (ruling 132.2); anything below is P11.T12's finding.
#[expect(
    clippy::doc_markdown,
    reason = "McLaughlin is an author's name, not code"
)]
const POL_MERGING_RATE: (f64, f64, f64) = (28.0, 42.0, 72.0);

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
    let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(0.0));
    let primary = BodyId::new(system, 0);
    let draws = if SolarMasses::new(m1) >= stripped_mark_min_mass(&comp) {
        let band = stripping_band(SolarMasses::new(m1), m2 / m1, &comp);
        let periastron = orbit.periapsis();
        let stripped = periastron > band.merge && periastron <= band.strip;
        let law = KickLawParams::default();
        (0..MAX_MARK_ATTEMPTS)
            .map(|attempt| StarDraws::for_attempt(seed, primary, attempt))
            .find(|draws| law.is_stripped(draws.stripped()) == stripped)
            .expect("a mark of either kind within the attempts")
    } else {
        StarDraws::for_star(seed, primary)
    };
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        comp,
        orbit,
        [draws, StarDraws::for_star(seed, BodyId::new(system, 1))],
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

/// What a pair's two supernovae leave, where both made neutron stars and the orbit survived both.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TwoNeutronStars {
    /// A bound double neutron star just after the second supernova, of this eccentricity, and
    /// whether gravitational waves merge it within the age of the universe (Peters 1964).
    Bound { eccentricity: f64, merges: bool },
    /// The first neutron star had grown into a black hole by accretion before the second.
    FirstGrewIntoABlackHole,
}

/// What `timeline`'s two supernovae leave ([`TwoNeutronStars`]), if both made neutron stars and
/// the orbit survived both.
fn two_neutron_stars(timeline: &BinaryTimeline) -> Option<TwoNeutronStars> {
    let [first, second] = timeline.supernovae() else {
        return None;
    };
    let neutron = |r: &SupernovaRecord| r.remnant().kind() == RemnantKind::NeutronStar && r.bound();
    if !(neutron(first) && neutron(second)) {
        return None;
    }
    let after = timeline.state_at(Years::new(second.age().value() * (1.0 + 1e-9)));
    let orbit = after.orbit()?;
    let [a, b] = after.stars();
    if a.phase() != Phase::NeutronStar || b.phase() != Phase::NeutronStar {
        return Some(TwoNeutronStars::FirstGrewIntoABlackHole);
    }
    let merges = peters_merger_time(
        a.mass(),
        b.mass(),
        orbit.semi_major_axis(),
        orbit.eccentricity(),
    ) <= AGE_OF_UNIVERSE;
    Some(TwoNeutronStars::Bound {
        eccentricity: orbit.eccentricity().value(),
        merges,
    })
}

/// Rulings 129.4e and 132.2: the first 12,000 layer-E prior pairs of the census above leave 6–144
/// bound double neutron stars, 50–90% of them at e < 0.3. The merging ones, scaled by VG18's
/// fiducial yield to a Galactic rate, are recorded against Pol et al. (2019) for P11.T12, and fail
/// only above twice Pol's upper limit. The pairs whose first neutron star accreted into a black
/// hole before the second supernova are recorded too.
#[test]
#[ignore = "slow: 12,000 layer-E prior pairs, the interacting ones run through the engine"]
fn layer_e_prior_pairs_leave_bound_double_neutron_stars_in_the_window() {
    const BAND: usize = 4;
    let (lo, hi, _) = LAYERS[BAND];
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    // Each thread collects its share of the pairs; the totals are the same on any number of
    // threads.
    let outcomes: Vec<TwoNeutronStars> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                scope.spawn(move || {
                    let mut found = Vec::new();
                    for index in (0..DNS_PAIRS).skip(k).step_by(threads) {
                        let input = prior_pair(BAND, index, (lo, hi));
                        if !can_interact(&input, AGE_OF_UNIVERSE) {
                            continue;
                        }
                        found.extend(two_neutron_stars(&evolve(&input, AGE_OF_UNIVERSE)));
                    }
                    found
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("a worker finishes"))
            .collect()
    });
    let bound: Vec<(f64, bool)> = outcomes
        .iter()
        .filter_map(|o| match *o {
            TwoNeutronStars::Bound {
                eccentricity,
                merges,
            } => Some((eccentricity, merges)),
            TwoNeutronStars::FirstGrewIntoABlackHole => None,
        })
        .collect();
    let count = u64::try_from(bound.len()).expect("a count");
    let circular = bound.iter().filter(|&&(e, _)| e < 0.3).count();
    let merging = bound.iter().filter(|&&(_, m)| m).count();
    let grown = outcomes.len() - bound.len();
    let as_f64 = |n: usize| f64::from(u32::try_from(n).expect("a small count"));
    let share = as_f64(circular) / as_f64(bound.len().max(1));
    let pairs = f64::from(u32::try_from(DNS_PAIRS).expect("a small count"));
    let rate = VG18_MERGING_RATE * (as_f64(merging) / pairs) / (VG18_YIELD * VG18_MERGING_SHARE);
    println!(
        "{count} bound double neutron stars in {DNS_PAIRS} layer-E prior pairs, {circular} of \
         them ({:.0}%) at e < 0.3, {merging} merging within the age of the universe: about \
         {rate:.0} per Myr in the Galaxy by VG18's yield, against Pol et al.'s {} ({}–{}); \
         {grown} more whose first neutron star accreted into a black hole before the second \
         supernova",
        100.0 * share,
        POL_MERGING_RATE.1,
        POL_MERGING_RATE.0,
        POL_MERGING_RATE.2,
    );
    assert!(
        (DNS_WINDOW.0..=DNS_WINDOW.1).contains(&count),
        "{count} bound double neutron stars in {DNS_PAIRS} layer-E pairs, outside {DNS_WINDOW:?}"
    );
    assert!(
        (DNS_CIRCULAR_SHARE.0..=DNS_CIRCULAR_SHARE.1).contains(&share),
        "{:.1}% of the double neutron stars at e < 0.3, outside {DNS_CIRCULAR_SHARE:?}",
        100.0 * share
    );
    assert!(
        rate <= 2.0 * POL_MERGING_RATE.2,
        "about {rate:.0} merging double neutron stars per Myr, above twice Pol et al.'s upper \
         limit of {}: re-open the common envelope's α λ with an advisor (ruling 132.2)",
        POL_MERGING_RATE.2
    );
}

/// What a recycled pulsar's companion is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Companion {
    /// A carbon–oxygen or oxygen–neon white dwarf: an intermediate-mass donor's core.
    HeavyWhiteDwarf,
    /// A neutron star: a Case BB donor's remnant.
    NeutronStar,
    /// A helium white dwarf: a low-mass donor's core.
    HeliumWhiteDwarf,
}

/// A recycled pulsar beside a remnant companion: its field, G, and its period, ms.
#[derive(Debug, Clone, Copy)]
struct Recycled {
    field: f64,
    period_ms: f64,
    companion: Companion,
}

/// Every recycled pulsar of `timeline` beside a white-dwarf or neutron-star companion, read at the
/// first segment start where it is recycled and its companion is that remnant.
fn recycled_pulsars(timeline: &BinaryTimeline) -> Vec<Recycled> {
    let mut seen = [false; 2];
    let mut out = Vec::new();
    for segment in timeline.segments() {
        let age = segment.start();
        let context = timeline.class_context(age);
        let state = timeline.state_at(age);
        for c in Component::BOTH {
            let i = c.index();
            if seen[i] {
                continue;
            }
            let Some(pulsar) = context.pulsar(c).filter(|p| p.recycled()) else {
                continue;
            };
            let companion = match state.stars()[1 - i].phase() {
                Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
                    Companion::HeavyWhiteDwarf
                }
                Phase::NeutronStar => Companion::NeutronStar,
                Phase::HeliumWhiteDwarf => Companion::HeliumWhiteDwarf,
                _ => continue,
            };
            seen[i] = true;
            out.push(Recycled {
                field: pulsar.state().field().value(),
                period_ms: pulsar.state().period().value() * 1e3,
                companion,
            });
        }
    }
    out
}

/// The 25th, 50th and 75th percentiles of `values`, which it sorts.
fn quartiles(values: &mut [f64]) -> [f64; 3] {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n == 0 {
        return [f64::NAN; 3];
    }
    [values[n / 4], values[n / 2], values[(3 * n) / 4]]
}

/// Ruling 132.1: under the Eddington limit, pulsars recycled by an intermediate-mass or Case BB
/// donor, whose companions are carbon–oxygen or oxygen–neon white dwarfs or neutron stars, are
/// mildly recycled, spinning at 10–50 ms at the end of accretion (Tauris, Langer and Kramer 2012,
/// section 4): their median is asserted there. Their fields, which the ruling's arithmetic puts at
/// about 3 × 10¹⁰–10¹² G under ruling 129.5's field burial, and those of the helium-white-dwarf
/// millisecond pulsars, are recorded for P11.T12, which checks the recycled fields.
#[test]
#[ignore = "slow: 22,000 layer-D and layer-E prior pairs, the interacting ones run through the engine"]
fn pulsars_recycled_beside_heavy_companions_are_mildly_recycled() {
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let jobs: Vec<(usize, u64)> = (0..LAYERS[3].2)
        .map(|i| (3, i))
        .chain((0..DNS_PAIRS).map(|i| (4, i)))
        .collect();
    let jobs = &jobs;
    let found: Vec<Recycled> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                scope.spawn(move || {
                    let mut found = Vec::new();
                    for &(band, index) in jobs.iter().skip(k).step_by(threads) {
                        let (lo, hi, _) = LAYERS[band];
                        let input = prior_pair(band, index, (lo, hi));
                        if !can_interact(&input, AGE_OF_UNIVERSE) {
                            continue;
                        }
                        found.extend(recycled_pulsars(&evolve(&input, AGE_OF_UNIVERSE)));
                    }
                    found
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("a worker finishes"))
            .collect()
    });
    let stats = |pick: &dyn Fn(Companion) -> bool| {
        let set: Vec<&Recycled> = found.iter().filter(|r| pick(r.companion)).collect();
        let mut fields: Vec<f64> = set.iter().map(|r| r.field).collect();
        let mut periods: Vec<f64> = set.iter().map(|r| r.period_ms).collect();
        (set.len(), quartiles(&mut fields), quartiles(&mut periods))
    };
    for (name, pick) in [
        (
            "a CO/ONe white dwarf",
            &(|c| c == Companion::HeavyWhiteDwarf) as &dyn Fn(Companion) -> bool,
        ),
        ("a neutron star", &|c| c == Companion::NeutronStar),
        ("a helium white dwarf", &|c| {
            c == Companion::HeliumWhiteDwarf
        }),
    ] {
        let (n, [f1, f2, f3], [p1, p2, p3]) = stats(pick);
        println!(
            "recycled beside {name}: {n}, field {f1:.2e} / {f2:.2e} / {f3:.2e} G, period \
             {p1:.1} / {p2:.1} / {p3:.1} ms (quartiles)"
        );
    }
    let (n, _, [_, period, _]) = stats(&|c| c != Companion::HeliumWhiteDwarf);
    assert!(n > 0, "no pulsar recycled beside a heavy companion");
    assert!(
        (10.0..=50.0).contains(&period),
        "median period {period:.1} ms beside heavy companions, outside 10–50 ms"
    );
}
