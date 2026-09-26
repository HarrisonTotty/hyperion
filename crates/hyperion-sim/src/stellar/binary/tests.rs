//! Tests of the binary engine (P11.T4).

use super::star::positive;
use super::*;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::units::{GravitationalParameter, Radians, Seconds, SolarMasses, Years};

/// A pair of `m1` and `m2` M☉ on an orbit of `period_days` and `e`, in the reference plane.
pub(super) fn pair(m1: f64, m2: f64, period_days: f64, e: f64, z: f64) -> BinaryInput {
    let orbit = KeplerElements::from_period(
        Seconds::new(period_days * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::new(e).expect("an eccentricity in [0, 1)"),
        Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
            .expect("an orientation"),
        Radians::new(0.0),
    )
    .expect("an orbit");
    let comp = if (z - 0.02).abs() < 1e-12 {
        Composition::SOLAR
    } else {
        Composition::from_fe_h(
            crate::units::Dex::new(crate::math::log10(z / 0.02)),
            crate::units::HeliumExcess::new(0.0),
        )
    };
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        comp,
        orbit,
        [StarDraws::median(), StarDraws::median()],
        Years::new(1.0e10),
    )
    .expect("a pair")
}

/// A human-readable account of a timeline, for the tests' failure messages.
pub(super) fn describe(timeline: &BinaryTimeline) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for s in timeline.segments() {
        let start = timeline.state_at(s.start());
        let end = timeline.state_at(Years::new(s.end().value().min(timeline.until().value())));
        let _ = writeln!(
            out,
            "{:>12.4} – {:>12.4} Myr {:?}: [{:?} {:.3} → {:?} {:.3}] [{:?} {:.3} → {:?} {:.3}] a {:?} → {:?} e {:?} rate {:?}",
            s.start().value() / 1e6,
            s.end().value() / 1e6,
            s.kind(),
            start.stars()[0].phase(),
            start.stars()[0].mass().value(),
            end.stars()[0].phase(),
            end.stars()[0].mass().value(),
            start.stars()[1].phase(),
            start.stars()[1].mass().value(),
            end.stars()[1].phase(),
            end.stars()[1].mass().value(),
            start.orbit().map(|o| o.semi_major_axis().value() / 6.957e8),
            end.orbit().map(|o| o.semi_major_axis().value() / 6.957e8),
            end.orbit().map(|o| o.eccentricity().value()),
            end.transfer_rate()
                .map(crate::units::SolarMassesPerYear::value),
        );
        let _ = writeln!(
            out,
            "        forms {} | {}",
            s.members()[0].form(),
            s.members()[1].form()
        );
    }
    let _ = writeln!(out, "pooled {:?}", timeline.pooled_ia());
    for sn in timeline.supernovae() {
        let _ = writeln!(
            out,
            "SN {:?} at {:.3} Myr {:?} bound {}",
            sn.component(),
            sn.age().value() / 1e6,
            sn.remnant(),
            sn.bound()
        );
    }
    out
}

/// The period, days, of a circular pair of `m1` and `m2` M☉ at separation `a_rsun`.
pub(super) fn period_days(m1: f64, m2: f64, a_rsun: f64) -> f64 {
    let a = a_rsun * crate::units::consts::SOLAR_RADIUS_M;
    core::f64::consts::TAU * (a * a * a / (crate::units::consts::GM_SUN * (m1 + m2))).sqrt()
        / 86_400.0
}

/// The timeline of a pair evolved to 15 Gyr under `params`.
fn run(m1: f64, m2: f64, period: f64, e: f64, params: BinaryParams) -> BinaryTimeline {
    evolve(
        &pair(m1, m2, period, e, 0.02).with_params(params),
        Years::new(1.5e10),
    )
}

/// The pair's state at the start of each segment, with the segment's kind.
fn starts(timeline: &BinaryTimeline) -> Vec<(SegmentKind, BinaryState)> {
    timeline
        .segments()
        .iter()
        .map(|s| (s.kind(), timeline.state_at(s.start())))
        .collect()
}

/// The first segment at or after `from` whose start satisfies `test`, and its index.
fn find(
    stages: &[(SegmentKind, BinaryState)],
    from: usize,
    test: impl Fn(&SegmentKind, &BinaryState) -> bool,
) -> Option<usize> {
    (from..stages.len()).find(|&i| test(&stages[i].0, &stages[i].1))
}

/// The period of a state's orbit, days.
fn days(state: &BinaryState) -> f64 {
    state
        .orbit()
        .map_or(f64::INFINITY, |o| o.period().value() / 86_400.0)
}

use crate::stellar::Phase;

/// T4.a: a pair too wide to touch is two single stars on its orbit: one detached segment whose
/// stars are plan 06's own, at every age.
#[test]
fn a_wide_pair_is_two_single_stars() {
    let input = pair(1.2, 0.8, 1.0e6, 0.3, 0.02);
    let until = Years::new(1.2e10);
    assert!(!can_interact(&input, until));
    let timeline = evolve(&input, until);
    assert_eq!(timeline.segments().len(), 1);
    assert_eq!(timeline.segments()[0].kind(), SegmentKind::Detached);
    let tracks = [1.2, 0.8].map(|m| {
        crate::stellar::sse::Track::to_age(
            SolarMasses::new(m),
            &Composition::SOLAR,
            &StarDraws::median(),
            until,
        )
    });
    for k in 0..=240 {
        let age = Years::new(until.value() * f64::from(k) / 240.0);
        let state = timeline.state_at(age);
        for (star, track) in state.stars().iter().zip(&tracks) {
            assert_eq!(*star, track.state_at(age), "at {age:?}");
        }
        let orbit = state.orbit().expect("the pair stays bound");
        assert_eq!(
            orbit,
            input.orbit(),
            "the orbit of a pair that never interacts is its own"
        );
    }
}

/// T4.a: a close pair can interact, and a wide one cannot, by the pre-test's periastron.
#[test]
fn the_pre_test_reads_the_periastron() {
    let until = Years::new(1.2e10);
    assert!(can_interact(&pair(1.2, 0.8, 100.0, 0.0, 0.02), until));
    // A giant reaches about 170 R☉: 10⁴ days is out of reach circular, and in reach at e = 0.95.
    assert!(!can_interact(&pair(1.2, 0.8, 1.0e4, 0.0, 0.02), until));
    assert!(can_interact(&pair(1.2, 0.8, 1.0e4, 0.95, 0.02), until));
    // By 1 Gyr the 1.2 M☉ star is still on its main sequence.
    assert!(!can_interact(
        &pair(1.2, 0.8, 100.0, 0.0, 0.02),
        Years::new(1.0e9)
    ));
}

/// BSE section 3.1's Algol (2.9 + 0.9 M☉, 8 d, e = 0.7, `α_CE` = 3): tides circularise the orbit
/// on the main sequence; the primary fills its Roche lobe on the Hertzsprung gap, and the thermal
/// and then nuclear transfer inverts the mass ratio, the Algol paradox; the primary is left a
/// helium star of about 0.42 M☉ beside a blue straggler of nearly three solar masses, and becomes a
/// carbon–oxygen white dwarf; the straggler fills its own lobe as a giant and a common envelope
/// leaves a white dwarf and a helium star in an orbit of hours.
#[test]
fn the_papers_algol_reproduces_its_sequence() {
    let timeline = run(
        2.9,
        0.9,
        8.0,
        0.7,
        BinaryParams::GENERATOR.with_alpha_ce(3.0),
    );
    let stages = starts(&timeline);
    let onset = find(&stages, 0, |k, _| {
        matches!(
            k,
            SegmentKind::StableTransfer {
                donor: Component::Primary
            }
        )
    })
    .expect("the primary fills its Roche lobe");
    let (_, at_onset) = &stages[onset];
    assert_eq!(at_onset.stars()[0].phase(), Phase::HertzsprungGap);
    let e = at_onset.orbit().expect("bound").eccentricity().value();
    assert!(e < 1e-6, "the orbit is circular at the onset: {e}");
    // The paper: circularised to e = 0.28 and P = 3 d by 413 Myr, at the end of the main sequence.
    let before = timeline.state_at(Years::new(4.13e8));
    assert!((2.4..3.3).contains(&days(&before)), "{}", days(&before));
    let helium = find(&stages, onset, |_, s| {
        s.stars()[0].phase() == Phase::HeliumMainSequence
    })
    .expect("the primary is left a naked helium star");
    let (_, stripped) = &stages[helium];
    let [he, ms] = stripped.stars();
    assert!((0.38..0.47).contains(&he.mass().value()), "{:?}", he.mass());
    assert_eq!(ms.phase(), Phase::MainSequence);
    assert!((2.6..3.2).contains(&ms.mass().value()), "{:?}", ms.mass());
    // A blue straggler (BSE section 4.2.1): on the main sequence above the turn-off mass of its
    // age, older than a single star of its mass lives on it.
    let turn_off = crate::stellar::sse::turn_off_mass(stripped.age(), &Composition::SOLAR);
    assert!(
        ms.mass() > turn_off,
        "{:?} against the turn-off {turn_off:?}",
        ms.mass()
    );
    let white_dwarf = find(&stages, helium, |_, s| {
        s.stars()[0].phase() == Phase::CarbonOxygenWhiteDwarf
    })
    .expect("the helium star becomes a white dwarf");
    let common = find(&stages, white_dwarf, |k, _| {
        *k == SegmentKind::CommonEnvelope
    })
    .expect("the straggler's giant goes into a common envelope");
    let after = &stages[(common + 1).min(stages.len() - 1)].1;
    assert_eq!(after.stars()[0].phase(), Phase::CarbonOxygenWhiteDwarf);
    assert_eq!(after.stars()[1].phase(), Phase::HeliumMainSequence);
    assert!(days(after) < 0.2, "the paper's 0.04 d: {}", days(after));
    assert!(!timeline.hit_segment_cap());
}

/// BSE section 3.2's cataclysmic variable (6.0 + 1.3 M☉, 630 d, `α_CE` = 1): the primary fills its
/// lobe on the early AGB at 78 Myr, a common envelope leaves a helium giant of 1.5 M☉ that soon
/// fills its lobe again, and a second leaves a carbon–oxygen white dwarf of 0.94 M☉ with the
/// 1.3 M☉ star in an orbit under a day. Angular-momentum loss brings the main-sequence star into
/// contact with its lobe, and it gives its mass to the dwarf through novae, across the
/// Hertzsprung gap and up the giant branch, until it is a helium white dwarf of about 0.16 M☉ in
/// a 4.5 d orbit.
#[test]
fn the_papers_cataclysmic_variable_reproduces_its_sequence() {
    let timeline = run(
        6.0,
        1.3,
        630.0,
        0.0,
        BinaryParams::GENERATOR.with_alpha_ce(1.0),
    );
    let stages = starts(&timeline);
    let first =
        find(&stages, 0, |k, _| *k == SegmentKind::CommonEnvelope).expect("a common envelope");
    let (_, entry) = &stages[first];
    assert!(
        (7.5e7..8.2e7).contains(&entry.age().value()),
        "{:?}",
        entry.age()
    );
    let second =
        find(&stages, first + 1, |k, _| *k == SegmentKind::CommonEnvelope).expect("a second one");
    let after = &stages[second + 1].1;
    assert_eq!(after.stars()[0].phase(), Phase::CarbonOxygenWhiteDwarf);
    assert!(
        (0.88..1.0).contains(&after.stars()[0].mass().value()),
        "{:?}",
        after.stars()[0].mass()
    );
    assert!(days(after) < 1.0, "{}", days(after));
    let cv = find(&stages, second, |k, s| {
        matches!(
            k,
            SegmentKind::StableTransfer {
                donor: Component::Secondary
            }
        ) && s.stars()[1].phase() == Phase::MainSequence
    })
    .expect("the main-sequence star fills its lobe: a cataclysmic variable");
    let mid = Years::new(f64::midpoint(
        stages[cv].1.age().value(),
        timeline.segments()[cv].end().value(),
    ));
    let rate = timeline
        .state_at(mid)
        .transfer_rate()
        .expect("transfer")
        .value();
    assert!(
        rate > 0.0 && rate < 1.03e-7,
        "novae, below 1.03 × 10⁻⁷ M☉/yr: {rate}"
    );
    let end = timeline.state_at(Years::new(1.5e10));
    assert_eq!(end.stars()[1].phase(), Phase::HeliumWhiteDwarf);
    assert!(
        (0.12..0.2).contains(&end.stars()[1].mass().value()),
        "{:?}",
        end.stars()[1].mass()
    );
    assert!(
        (2.5..7.0).contains(&days(&end)),
        "the paper's 4.5 d: {}",
        days(&end)
    );
}

/// BSE section 3.4's double neutron star, in the variant of 13.6 M☉ with a lighter companion that
/// the paper runs without kicks (as its comparison with Portegies Zwart and Verbunt does): stable
/// transfer from the Hertzsprung gap leaves a helium star, whose supernova the pair survives; the
/// companion's envelope goes in a common envelope around the neutron star; its helium core
/// explodes in turn, and two neutron stars remain bound.
#[test]
fn a_massive_pair_without_kicks_makes_a_double_neutron_star() {
    let params = BinaryParams {
        natal_kicks: false,
        ..BinaryParams::GENERATOR.with_alpha_ce(3.0)
    };
    let timeline = run(13.6, 10.0, period_days(13.6, 10.0, 100.0), 0.0, params);
    let stages = starts(&timeline);
    let transfer = find(&stages, 0, |k, s| {
        matches!(
            k,
            SegmentKind::StableTransfer {
                donor: Component::Primary
            }
        ) && s.stars()[0].phase() == Phase::HertzsprungGap
    });
    assert!(transfer.is_some(), "{}", describe(&timeline));
    let supernovae = timeline.supernovae();
    assert_eq!(supernovae.len(), 2, "{}", describe(&timeline));
    assert!(supernovae.iter().all(SupernovaRecord::bound));
    let common =
        find(&stages, 0, |k, _| *k == SegmentKind::CommonEnvelope).expect("a common envelope");
    assert!(stages[common].1.age() > supernovae[0].age());
    let pair = timeline.state_at(Years::new(supernovae[1].age().value() * (1.0 + 1e-9)));
    assert_eq!(pair.stars()[0].phase(), Phase::NeutronStar);
    assert_eq!(pair.stars()[1].phase(), Phase::NeutronStar);
    assert!(pair.orbit().is_some());
}

/// A common envelope that ends in coalescence (BSE section 4.2.2: 2.0 and 0.2 M☉ from 50 R☉
/// coalesce in the envelope of the AGB star): one star is left.
#[test]
fn a_common_envelope_can_end_in_coalescence() {
    let timeline = run(
        2.0,
        0.2,
        period_days(2.0, 0.2, 50.0),
        0.0,
        BinaryParams::GENERATOR.with_alpha_ce(3.0),
    );
    let stages = starts(&timeline);
    let common = find(&stages, 0, |k, _| *k == SegmentKind::CommonEnvelope);
    assert!(common.is_some(), "{}", describe(&timeline));
    let merged = timeline.merger_age().expect("the cores coalesce");
    assert!(merged >= stages[common.unwrap_or(0)].1.age());
    let after = timeline.state_at(Years::new(merged.value() * (1.0 + 1e-9)));
    assert_eq!(after.stars()[1].phase(), Phase::NoRemnant);
    assert!(after.orbit().is_none());
}

/// A Type Ia candidate (Tout et al.'s Algol, BSE section 3.4: 3.2 + 2.0 M☉, 5 d, `α_CE` = 3): two
/// common envelopes leave two carbon–oxygen white dwarfs in an orbit of hours, which gravitational
/// radiation brings together; the merger is a pooled Type Ia candidate (plan 11, P11.T4.f), and the
/// engine continues with the merged dwarf.
#[test]
fn a_double_white_dwarf_merger_is_a_pooled_type_ia_candidate() {
    let timeline = run(
        3.2,
        2.0,
        5.0,
        0.0,
        BinaryParams::GENERATOR.with_alpha_ce(3.0),
    );
    let envelopes = timeline
        .segments()
        .iter()
        .filter(|s| s.kind() == SegmentKind::CommonEnvelope)
        .count();
    assert_eq!(envelopes, 2, "{}", describe(&timeline));
    let pooled = timeline.pooled_ia().expect("a pooled candidate");
    assert_eq!(pooled.channel(), IaPoolChannel::Merger);
    let [a, b] = pooled.masses();
    assert!(a.value() > 0.3 && b.value() > 0.3, "{a:?} {b:?}");
    let merger = timeline.merger_age().expect("the dwarfs merge");
    assert!((merger.value() - pooled.age().value()).abs() < 1.0);
    let after = timeline.state_at(Years::new(1.5e10));
    assert_eq!(after.stars()[0].phase(), Phase::CarbonOxygenWhiteDwarf);
    assert!((after.stars()[0].mass().value() - (a.value() + b.value())).abs() < 1e-9);
}

/// A splitmix64 generator, for the tests' samples.
struct Mix(u64);

impl Mix {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a 53-bit word and 2⁵³ are exact in an f64"
    )]
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z >> 11) as f64 / (1_u64 << 53) as f64
    }
}

/// `n` close pairs drawn log-uniform in primary mass (0.8–25 M☉) and period (0.5–3,000 d), uniform
/// in mass ratio (0.1–1) and, above 5 d, in eccentricity (0–0.6).
fn sample_pairs(n: usize, seed: u64) -> Vec<BinaryInput> {
    let mut mix = Mix(seed);
    (0..n)
        .map(|_| {
            let m1 =
                crate::math::exp(crate::math::ln(0.8) + mix.next() * crate::math::ln(25.0 / 0.8));
            let m2 = (m1 * (0.1 + 0.9 * mix.next())).max(0.1);
            let period =
                crate::math::exp(crate::math::ln(0.5) + mix.next() * crate::math::ln(6000.0));
            let e = if period > 5.0 { 0.6 * mix.next() } else { 0.0 };
            pair(m1, m2, period, e, 0.02)
        })
        .collect()
}

/// The relative change from `a` to `b`, zero where both are zero.
fn change(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale > 0.0 {
        (a - b).abs() / scale
    } else {
        0.0
    }
}

/// Checks P11.T4's invariants on the timeline of `input`: inside every segment the state is
/// continuous (no jump above 1% between samples 10⁻⁶ of the segment apart); no main-sequence star
/// is older than its effective lifetime; the pair's total mass never rises; the timeline never
/// reaches the cap on segments.
#[expect(
    clippy::many_single_char_names,
    reason = "a sample's two states and their values"
)]
fn check_invariants(input: &BinaryInput, until: Years) {
    let timeline = evolve(input, until);
    let what = || {
        format!(
            "{:?} P {} d e {}\n{}",
            input.masses(),
            input.orbit().period().value() / 86_400.0,
            input.orbit().eccentricity().value(),
            describe(&timeline)
        )
    };
    assert!(!timeline.hit_segment_cap(), "{}", what());
    let coeffs = crate::stellar::sse::ZCoeffs::new(input.composition().z_fit());
    for segment in timeline.segments() {
        let (start, end) = (segment.start().value(), segment.end().value());
        let span = end - start;
        if !positive(span) {
            continue;
        }
        for k in 1..8_u32 {
            let t = start + span * f64::from(k) / 8.0;
            let (a, b) = (
                timeline.state_at(Years::new(t)),
                timeline.state_at(Years::new(t + 1e-6 * span)),
            );
            for (x, y) in a.stars().iter().zip(b.stars()) {
                for (p, q, name) in [
                    (x.mass().value(), y.mass().value(), "mass"),
                    (x.luminosity().value(), y.luminosity().value(), "luminosity"),
                    (x.radius().value(), y.radius().value(), "radius"),
                ] {
                    assert!(
                        change(p, q) < 0.01,
                        "{name} jumps {p} → {q} at {t} in {:?}: {}",
                        segment.kind(),
                        what()
                    );
                }
                if x.phase() == crate::stellar::Phase::MainSequence && x.mass().value() >= 0.1 {
                    let lifetime = crate::stellar::sse::main_sequence_lifetime(
                        &coeffs,
                        false,
                        x.mass().value(),
                    );
                    assert!(
                        x.age().value() <= lifetime * (1.0 + 1e-6),
                        "a main-sequence star of {:?} at {:?} past its {lifetime} yr at {t} in [{start}, {end}] ({}, {}): {}",
                        x.mass(),
                        x.age(),
                        segment.members()[0].form(),
                        segment.members()[1].form(),
                        what()
                    );
                }
            }
            if let (Some(p), Some(q)) = (a.orbit(), b.orbit()) {
                let (p, q) = (p.semi_major_axis().value(), q.semi_major_axis().value());
                assert!(
                    change(p, q) < 0.01,
                    "the orbit jumps {p} → {q} at {t}: {}",
                    what()
                );
            }
        }
    }
    // Ruling 108.1: no contact pair lives below Rasio's (1995) mass ratio.
    for segment in timeline.segments() {
        if segment.kind() == SegmentKind::Contact {
            let state = timeline.state_at(segment.start());
            let [a, b] = state.stars().map(|s| s.mass().value());
            assert!(
                a.min(b) >= common_envelope::CONTACT_MIN_Q * a.max(b),
                "a contact pair of {a} and {b} M☉: {}",
                what()
            );
        }
    }
    let mut last = (f64::INFINITY, 0.0);
    for k in 0..=600_u32 {
        let age = until.value() * f64::from(k) / 600.0;
        let total = timeline.state_at(Years::new(age)).total_mass().value();
        assert!(
            total <= last.0 + 1e-6,
            "the mass rises {} → {total} from {} to {age} yr (e {}): {}",
            last.0,
            last.1,
            input.orbit().eccentricity().value(),
            what()
        );
        last = (total, age);
    }
}

/// P11.T4's invariants over a sample of close pairs (the plan's 10³ in the slow suite).
#[test]
fn close_pairs_keep_the_engines_invariants() {
    for input in sample_pairs(60, 0x5eed_0001) {
        check_invariants(&input, Years::new(1.2e10));
    }
}

/// P11.T4's invariants over 10³ close pairs (the plan's figure).
#[test]
#[ignore = "slow: 10³ binaries run through the engine"]
fn a_thousand_close_pairs_keep_the_engines_invariants() {
    for input in sample_pairs(1_000, 0x5eed_0002) {
        check_invariants(&input, Years::new(1.2e10));
    }
}

/// The engine is a pure function of its input: the same timeline whatever was run before, and
/// the same states whatever was asked before.
#[test]
fn a_timeline_is_the_same_in_any_order() {
    let inputs = sample_pairs(12, 0x5eed_0003);
    let until = Years::new(1.2e10);
    let forward: Vec<_> = inputs.iter().map(|i| evolve(i, until)).collect();
    let backward: Vec<_> = inputs.iter().rev().map(|i| evolve(i, until)).collect();
    for (a, b) in forward.iter().zip(backward.iter().rev()) {
        assert_eq!(a, b);
    }
    let timeline = &forward[0];
    let ages: Vec<f64> = (0..50)
        .map(|k| until.value() * f64::from(k) / 49.0)
        .collect();
    let ahead: Vec<_> = ages
        .iter()
        .map(|&t| timeline.state_at(Years::new(t)))
        .collect();
    let behind: Vec<_> = ages
        .iter()
        .rev()
        .map(|&t| timeline.state_at(Years::new(t)))
        .collect();
    assert!(ahead.iter().eq(behind.iter().rev()));
}

/// Plan 11's design note 16: a primary heavy enough for placement to read its death collapses at
/// plan 06's death age, with plan 06's remnant and kick, whatever its companion does.
#[test]
fn a_massive_primary_dies_when_plan_06_says() {
    let mut checked = 0;
    for (m1, m2, a) in [
        (10.0, 6.0, 40.0),
        (15.0, 3.0, 300.0),
        (25.0, 20.0, 80.0),
        (12.0, 11.0, 1000.0),
        (40.0, 8.0, 150.0),
    ] {
        let input = pair(m1, m2, period_days(m1, m2, a), 0.0, 0.02);
        let timeline = evolve(&input, Years::new(5.0e7));
        let model = crate::stellar::system::StarModel::new(
            SolarMasses::new(m1),
            Composition::SOLAR,
            StarDraws::median(),
            input.age_at_epoch(),
        )
        .expect("a star");
        let death = model.death().expect("a massive star dies");
        let record = timeline
            .supernovae()
            .iter()
            .find(|s| s.component() == Component::Primary);
        if let Some(record) = record {
            assert!(
                (record.age().value() - death.age().value()).abs() <= 1e-6 * death.age().value(),
                "{m1}: {:?} against {:?}",
                record.age(),
                death.age()
            );
            // Plan 06's remnant, held to the star's own mass where the binary stripped it
            // below that.
            let own = model.remnant().expect("a remnant");
            assert_eq!(record.remnant().kind(), own.kind());
            assert!(record.remnant().mass() <= own.mass());
            assert_eq!(record.kick(), model.natal_kick());
            checked += 1;
        } else {
            // The primary merged away into nothing living before its death: no collapse.
            let at = timeline.state_at(death.age());
            assert!(
                !at.stars()[0].phase().is_living(),
                "{m1}: {}",
                describe(&timeline)
            );
        }
    }
    assert!(checked >= 3, "{checked}");
}

/// T4.b: a pair of 0.6 M☉ carbon–oxygen white dwarfs 0.1 d apart spirals in by gravitational
/// radiation and merges when Peters's closed form says (the Roche-lobe contact that ends it comes
/// under 10⁻⁵ of the time before a = 0).
#[test]
fn a_tenth_of_a_day_double_white_dwarf_merges_at_peters_time() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit};
    use super::star::{Member, Path};
    use super::timeline::Context;
    use crate::orbit::peters_merger_time;

    let m = 0.6;
    let input = pair(m, m, 0.1, 0.0, 0.02);
    let expected = peters_merger_time(
        SolarMasses::new(m),
        SolarMasses::new(m),
        input.orbit().semi_major_axis(),
        Eccentricity::CIRCULAR,
    );
    let until = 2.0 * expected.value();
    let dwarf = || Member::Remnant {
        phase: Phase::CarbonOxygenWhiteDwarf,
        birth: 0.0,
        origin: crate::units::Megayears::ZERO,
        mass: Path::starting(0.0, m),
    };
    let a = input.orbit().semi_major_axis().value() / crate::units::consts::SOLAR_RADIUS_M;
    let orbit = LiveOrbit::new(
        0.0,
        a,
        0.0,
        *input.orbit().orientation(),
        input.orbit().mean_anomaly_at_epoch(),
    );
    let mut engine = Engine::new(
        Arc::new(Context::of(&input)),
        until,
        [dwarf(), dwarf()],
        orbit,
        None,
    );
    engine.run();
    let timeline = engine.finish();
    let merged = timeline.merger_age().expect("the dwarfs merge");
    // The midpoint steps of 2% of the orbit's angular momentum (BSE equation 88) keep the
    // inspiral to about 0.4% of the closed form.
    assert!(
        (merged.value() / expected.value() - 1.0).abs() < 1e-2,
        "{merged:?} against Peters's {expected:?}"
    );
    let pooled = timeline
        .pooled_ia()
        .expect("a merger of two white dwarfs is pooled");
    assert_eq!(pooled.channel(), IaPoolChannel::Merger);
}

/// A 64-bit FNV-1a hash that text and bits are written into, for the pinned timelines.
struct Fnv(u64);

impl Fnv {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn f64(&mut self, x: f64) {
        self.bytes(&hyperion_testkit::float::bits(x).to_le_bytes());
    }
}

impl core::fmt::Write for Fnv {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.bytes(s.as_bytes());
        Ok(())
    }
}

/// The digest of everything a timeline holds, bit for bit: every segment's kind, bounds, members
/// (each distinct track in full, by its `Debug`, which prints every `f64` exactly), paths and
/// orbit; the supernovae and the pooled Type Ia candidate; and the pair's state at 257 even ages
/// and at every segment's start and middle, which is what a caller reads.
fn digest(timeline: &BinaryTimeline) -> u64 {
    use core::fmt::Write as _;

    use super::star::Member;

    let mut h = Fnv::new();
    let mut tracks: Vec<*const crate::stellar::sse::Track> = Vec::new();
    let mut track = |h: &mut Fnv, t: &std::sync::Arc<crate::stellar::sse::Track>| {
        let ptr = std::sync::Arc::as_ptr(t);
        if let Some(k) = tracks.iter().position(|&p| p == ptr) {
            write!(h, "track #{k}").expect("a hash");
        } else {
            write!(h, "track #{} {t:?}", tracks.len()).expect("a hash");
            tracks.push(ptr);
        }
    };
    write!(
        h,
        "{:?} {:?} {} {:?}",
        timeline.pooled_ia(),
        timeline.supernovae(),
        timeline.hit_segment_cap(),
        timeline.recoil()
    )
    .expect("a hash");
    h.f64(timeline.until().value());
    for segment in timeline.segments() {
        write!(h, "{:?}", segment.kind()).expect("a hash");
        h.f64(segment.start().value());
        h.f64(segment.end().value());
        for member in segment.members() {
            match member {
                Member::Track { track: t, offset } => {
                    write!(h, "Track").expect("a hash");
                    h.f64(*offset);
                    track(&mut h, t);
                }
                Member::Shaped {
                    track: t,
                    offset,
                    mass,
                } => {
                    write!(h, "Shaped {mass:?}").expect("a hash");
                    h.f64(*offset);
                    track(&mut h, t);
                }
                other => write!(h, "{other:?}").expect("a hash"),
            }
        }
        let (orbit, rates) = segment.paths();
        write!(h, "{orbit:?} {rates:?}").expect("a hash");
    }
    let until = timeline.until().value();
    let mut ages: Vec<f64> = (0..=256_u32)
        .map(|k| until * f64::from(k) / 256.0)
        .collect();
    for segment in timeline.segments() {
        let (start, end) = (segment.start().value(), segment.end().value());
        ages.push(start);
        ages.push(start + 0.5 * (end - start));
    }
    for age in ages {
        write!(h, "{:?}", timeline.state_at(Years::new(age))).expect("a hash");
    }
    h.0
}

/// The `i`th pair of the pinned sample: log-uniform in primary mass (0.8–40 M☉) and period
/// (0.3–10⁴ d), uniform in mass ratio (0.05–1, the companion no lighter than 0.06 M☉) and, above
/// 5 d, in eccentricity (0–0.7); one of three metallicities; each star's own draws, as plan 06
/// makes them; and every seventh pair with BSE's `α_CE` of 3.
fn pinned_pair(mix: &mut Mix, i: u32) -> BinaryInput {
    use crate::Seed;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};

    let m1 = crate::math::exp(crate::math::ln(0.8) + mix.next() * crate::math::ln(40.0 / 0.8));
    let m2 = (m1 * (0.05 + 0.95 * mix.next())).max(0.06);
    let period = crate::math::exp(crate::math::ln(0.3) + mix.next() * crate::math::ln(1.0e4 / 0.3));
    let e = if period > 5.0 { 0.7 * mix.next() } else { 0.0 };
    let z = [0.02, 0.004, 0.0005][usize::try_from(i % 3).expect("a small index")];
    let base = pair(m1, m2, period, e, z);
    let x = i32::try_from(i).expect("a small index") - 500;
    let cell = GenCell::new(CellSize::Ly8, [x, 7, 0]).expect("a cell");
    let system = SystemId::from_parts(Layer::A, cell, 0).expect("a system");
    let draws = [0, 1].map(|k| StarDraws::for_star(Seed::new(0x0b1e_5eed), BodyId::new(system, k)));
    let input = BinaryInput::new(
        base.masses()[0],
        base.masses()[1],
        *base.composition(),
        *base.orbit(),
        draws,
        base.age_at_epoch(),
    )
    .expect("a pair");
    if i % 7 == 3 {
        input.with_params(BinaryParams::GENERATOR.with_alpha_ce(3.0))
    } else {
        input
    }
}

/// The engine's output is pinned bit for bit over 10³ pairs (written before the engine was sped
/// up, and kept so that no optimisation moves a result): each line is one pair's [`digest`].
#[test]
fn a_thousand_timelines_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let until = Years::new(1.2e10);
    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    let mut mix = Mix(0x0b1e_0001);
    for i in 0..1_000_u32 {
        let input = pinned_pair(&mut mix, i);
        let timeline = evolve(&input, until);
        let [m1, m2] = input.masses().map(SolarMasses::value);
        w.u64_hex(
            &format!(
                "{i:04} {m1:.3}+{m2:.3} M☉ P {:.2} d, {} segments",
                input.orbit().period().value() / 86_400.0,
                timeline.segments().len()
            ),
            digest(&timeline),
        );
    }
    golden!("stellar/binary_timelines", w.as_str());
}

/// Ruling 108.1: a W Ursae Majoris pair, 1.0 and 0.5 M☉ at 0.35 d, reaches contact by slow transfer and
/// stays in contact for at least 10⁸ yr, as observed contact binaries do, rather than for a
/// thermal timescale.
#[test]
fn a_w_ursae_majoris_pair_stays_in_contact() {
    let timeline = evolve(&pair(1.0, 0.5, 0.35, 0.0, 0.02), Years::new(1.2e10));
    let contact = timeline
        .segments()
        .iter()
        .find(|s| s.kind() == SegmentKind::Contact)
        .unwrap_or_else(|| panic!("the pair comes into contact: {}", describe(&timeline)));
    let lasts = contact.end().value() - contact.start().value();
    assert!(lasts >= 1.0e8, "{lasts} yr: {}", describe(&timeline));
}

/// Ruling 108.1: a contact pair whose mass ratio is below Rasio's (1995) 0.09 is tidally unstable
/// and coalesces at once, while one just above it stays in contact.
#[test]
fn a_contact_pair_below_the_tidal_limit_coalesces_at_once() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit};
    use super::star::{Member, Path};
    use super::timeline::Context;

    let kind_after_contact = |m2: f64| {
        let input = pair(1.2, m2, 0.4, 0.0, 0.02);
        let star = |m: f64| Member::MainSequence {
            helium: false,
            mass: Path::starting(0.0, m),
            tau: Path::starting(0.0, 0.3),
        };
        let k = input.orbit();
        let a = k.semi_major_axis().value() / crate::units::consts::SOLAR_RADIUS_M;
        let orbit = LiveOrbit::new(0.0, a, 0.0, *k.orientation(), k.mean_anomaly_at_epoch());
        let mut engine = Engine::new(
            Arc::new(Context::of(&input)),
            1.0e9,
            [star(1.2), star(m2)],
            orbit,
            None,
        );
        engine.contact(0, 0.0);
        engine.kind
    };
    assert_eq!(kind_after_contact(0.105), SegmentKind::Merged);
    assert_eq!(kind_after_contact(0.115), SegmentKind::Contact);
}

/// Ruling 108.2: with `α_CE` = 1 and λ = 0.5, the common envelopes of first-giant-branch
/// progenitors of 1.0–1.5 M☉ with 0.3 M☉ companions leave white dwarf and main-sequence pairs in
/// the post-common-envelope binaries' observed range of periods, 1.9 h to 4.3 d (Nebot
/// Gómez-Morán et al. 2011, A&A 536, A43): none shorter, and none longer but at most two from the
/// widest orbits, which meet the giant at the tip of its branch.
#[test]
fn post_common_envelope_pairs_land_in_the_observed_periods() {
    let mut periods = Vec::new();
    for m1 in [1.0, 1.1, 1.2, 1.3, 1.4, 1.5] {
        for initial in [70.0, 100.0, 300.0, 450.0] {
            let timeline = evolve(&pair(m1, 0.3, initial, 0.0, 0.02), Years::new(1.3e10));
            let stages = starts(&timeline);
            let Some(ce) = find(&stages, 0, |k, _| *k == SegmentKind::CommonEnvelope) else {
                continue;
            };
            let Some(after) = stages.get(ce + 1) else {
                continue;
            };
            let [dwarf, companion] = after.1.stars().map(|s| s.phase());
            if after.0 == SegmentKind::Detached
                && dwarf == Phase::HeliumWhiteDwarf
                && companion == Phase::MainSequence
            {
                periods.push((m1, initial, days(&after.1)));
            }
        }
    }
    assert!(periods.len() >= 20, "{periods:?}");
    let (shortest, longest) = (1.9 / 24.0, 4.3);
    assert!(periods.iter().all(|p| p.2 >= shortest), "{periods:?}");
    // The allowance, recorded in plan 11's Risks: the widest orbits, which reach the giant at the
    // tip of its branch where its envelope is least bound, may land above 4.3 d (7.5 d from
    // 1.0 M☉ at 450 d as built), and no more than two of them.
    let above: Vec<_> = periods.iter().filter(|p| p.2 > longest).collect();
    assert!(
        above.len() <= 2 && above.iter().all(|p| p.1 >= 450.0),
        "{periods:?}"
    );
}
