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

/// SSE's τ of an early-AGB member's core remnant at `age`, years, its core radius, R☉, and the τ
/// at which the remnant's radius would reach `lobe` R☉, `None` if it never does by τ = 1, where the
/// blend ends (for the margin of BSE section 3.2's common envelope); `None` off the early AGB.
fn early_agb_core_at(
    member: &super::star::Member,
    age: f64,
    lobe: f64,
) -> Option<(f64, f64, Option<f64>)> {
    use super::star::Member;
    let (track, track_age, mass) = match member {
        Member::Track { track, offset } => {
            let track_age = age - offset;
            let mass = track.state_at(Years::new(track_age)).mass().value();
            (track, track_age, mass)
        }
        Member::Shaped {
            track,
            offset,
            mass,
        } => (track, age - offset, mass.at(age)),
        Member::MainSequence { .. }
        | Member::Cooling { .. }
        | Member::Frozen { .. }
        | Member::Remnant { .. }
        | Member::Gone => return None,
    };
    let (tau, radius) = track.early_agb_remnant(track_age, mass)?;
    let rc = track.structure_at(track_age, mass).core_radius.value();
    // The remnant's radius rises with τ up to 1: bisect for the τ at which it reaches the lobe.
    let filling = (radius(1.0) >= lobe).then(|| {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..60 {
            let mid = f64::midpoint(lo, hi);
            if radius(mid) < lobe {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    });
    Some((tau, rc, filling))
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
    // The margin of the first common envelope (ruling p11-stripped-core, amendment 2): the
    // giant's core radius, which SSE's τ sets, against the core's Roche lobe on the orbit the
    // envelope leaves. The cores coalesce, and the sequence is lost, if it fills the lobe.
    let ce = &timeline.segments()[first];
    // The pair as the envelope leaves it, read from the next segment itself: the timeline's state
    // at that age is the second envelope's, which follows at once.
    let next = &timeline.segments()[first + 1];
    let at = next.start().value();
    let a_f = next
        .paths()
        .0
        .expect("the cores survive the first common envelope")
        .axis
        .at(at);
    let mass = |i: usize| {
        next.members()[i]
            .state_at(timeline.context(), i, at)
            .mass()
            .value()
    };
    let lobe = super::evolve::roche_lobe(mass(0), mass(1), a_f);
    let (tau, rc, filling) = early_agb_core_at(&ce.members()[0], ce.start().value(), lobe)
        .expect("the primary enters the common envelope on its early AGB");
    eprintln!(
        "BSE section 3.2, first common envelope at {:.3} Myr: SSE's τ = {tau:.4}, core radius \
         {rc:.4} R☉ against the core's lobe of {lobe:.4} R☉ (a_f = {a_f:.4} R☉), which the core \
         would fill from τ = {filling:.4?}",
        entry.age().value() / 1e6
    );
    assert!(
        rc < lobe,
        "the core fills its lobe: {}",
        describe(&timeline)
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

/// How far past `M_start` ÷ `M_end` a detached segment's orbit may widen (ruling 129.4a's test): the
/// tides that hand a spun-up star's angular momentum to the orbit, and the wind a companion
/// accretes, widen it by up to 23% more over the 10³ pairs below (a carbon–oxygen white dwarf
/// beside a main-sequence accretor spun up by its transfer). The fault the test guards against
/// widened orbits ×2–25.
const WIDENING_TOLERANCE: f64 = 0.5;

/// Ruling 129.4a's invariant on `timeline`: a detached segment has no supernova, transfer or
/// common envelope inside it, and what it loses as wind widens the orbit by at most `M_start` ÷
/// `M_end` (Jeans's mode, the widest a wind alone gives), with [`WIDENING_TOLERANCE`] for the spin
/// tides hand back. `what` describes the pair for the failure message.
fn check_detached_widening(timeline: &BinaryTimeline, what: &impl Fn() -> String) {
    for segment in timeline.segments() {
        // The protostars accrete until t_p (P06.T15.a); the pair's mass only falls after.
        let (start, end) = (
            segment
                .start()
                .value()
                .max(crate::stellar::premain::PROTOSTAR_YEARS),
            segment.end().value(),
        );
        if segment.kind() != SegmentKind::Detached || !positive(end - start) {
            continue;
        }
        // Just inside the segment: a supernova at either end is the neighbouring segment's.
        let (a, b) = (
            timeline.state_at(Years::new(start + 1e-9 * (end - start))),
            timeline.state_at(Years::new(end - 1e-9 * (end - start))),
        );
        if let (Some(p), Some(q)) = (a.orbit(), b.orbit()) {
            let widening = q.semi_major_axis().value() / p.semi_major_axis().value();
            let loss = a.total_mass().value() / b.total_mass().value();
            assert!(
                widening <= loss * (1.0 + WIDENING_TOLERANCE),
                "a detached segment [{start}, {end}] widens the orbit ×{widening} for a mass \
                 loss of ×{loss}: {}",
                what()
            );
        }
    }
}

/// Ruling 132.3's invariant on `timeline`: no segment of no length repeats the kind (and donor)
/// of the one before it, as a main sequence closed in on without end did. A common envelope has
/// no length by construction, and two at one instant are two envelopes: a giant's hydrogen, then
/// the helium giant it leaves, when that fills the lobe the first left (a 5.26 + 5.23 M☉ pair at
/// 902 d of the 10³ below). One envelope met again and again reaches the segment cap, which
/// [`check_invariants`] asserts no pair does. `what` describes the pair for the failure message.
fn check_no_repeated_instants(timeline: &BinaryTimeline, what: &impl Fn() -> String) {
    for pair in timeline.segments().windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        let span = after.end().value() - after.start().value();
        let repeats = after.kind() == before.kind() && after.kind() != SegmentKind::CommonEnvelope;
        assert!(
            !(repeats && span <= 1e-9 * after.start().value().max(1.0)),
            "a {:?} segment of no length at {:?} repeats the one before it: {}",
            after.kind(),
            after.start(),
            what()
        );
    }
}

/// Checks P11.T4's invariants on the timeline of `input`: inside every segment the state is
/// continuous (no jump above 1% between samples 10⁻⁶ of the segment apart); no main-sequence star
/// is older than its effective lifetime; the pair's total mass never rises; the timeline never
/// reaches the cap on segments; no detached segment widens the orbit past its mass loss.
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
                    // The main sequence starts at the arrival on it (P06.T15.b).
                    let arrival = crate::stellar::sse::arrival_time(x.mass(), input.composition());
                    assert!(
                        x.age().value() <= (lifetime + arrival) * (1.0 + 1e-6),
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
    check_detached_widening(&timeline, &what);
    check_no_repeated_instants(&timeline, &what);
    check_no_bare_giant(&timeline, &what);
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
        // The protostars accrete until t_p (P06.T15.a); the pair's mass only falls after.
        if age < crate::stellar::premain::PROTOSTAR_YEARS {
            continue;
        }
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

/// P11.T4.g: a star the binary carries with no envelope is a naked helium star or a white dwarf
/// (HPT section 6; BSE `hrdiag`). At each segment's start and at every step inside it (the
/// knots of a carried star's mass), no living star the binary carries in a hydrogen giant phase
/// (HG to TPAGB) has M ≤ Mc, the engine's own test. The ruling's margin of 10⁻⁹ M☉ is not kept: a donor's wind or
/// transfer can leave an envelope of 10⁻¹² M☉ at a step, which is not yet none. Between two steps
/// the core may outgrow the mass, as in BSE, whose `hrdiag` acts at the next step. Every held
/// (`Frozen`) member's core radius is no larger than its radius, and is one of sse's: zero,
/// `R_ZHe(Mc)` or 5 `R_WD(Mc)`, each held to R (the core-helium-burning rule's τ is not kept on a held
/// state).
fn check_no_bare_giant(timeline: &BinaryTimeline, what: &impl Fn() -> String) {
    use crate::stellar::Phase;
    use crate::stellar::remnant::RemnantRecipe;
    use crate::stellar::remnant::structure::white_dwarf_radius;

    let giant = |phase: Phase| {
        matches!(
            phase,
            Phase::HertzsprungGap
                | Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb
        )
    };
    let until = timeline.until().value();
    for segment in timeline.segments() {
        let (start, end) = (segment.start().value(), segment.end().value().min(until));
        let knots = segment.members().iter().flat_map(|member| match member {
            super::star::Member::Shaped { mass, .. } => mass.knots(),
            _ => &[],
        });
        let ages = core::iter::once(start).chain(
            knots
                .map(|knot| knot[0])
                .filter(|&age| start <= age && age < end),
        );
        for t in ages {
            let state = timeline.state_at(Years::new(t));
            for (star, member) in state.stars().iter().zip(segment.members()) {
                // A star on its own single-star track is plan 06's, whose last instant on the
                // thermally pulsing AGB has M = Mc: only a star the binary carries is checked.
                let carried = matches!(member, super::star::Member::Shaped { .. });
                if carried && giant(star.phase()) {
                    assert!(
                        star.mass().value() > star.core_mass().value(),
                        "a {:?} star of {} M☉ with a {} M☉ core at {t} yr: {}",
                        star.phase(),
                        star.mass().value(),
                        star.core_mass().value(),
                        what()
                    );
                }
            }
        }
        for member in segment.members() {
            let super::star::Member::Frozen { state, core_radius } = member else {
                continue;
            };
            let (r, rc, mc) = (
                state.radius().value(),
                core_radius.value(),
                state.core_mass().value(),
            );
            assert!(
                rc <= r,
                "a held core of {rc} R☉ in a star of {r} R☉: {}",
                what()
            );
            if state.phase() == Phase::CoreHeliumBurning || mc <= 0.0 {
                continue;
            }
            if state.phase() == Phase::EarlyAgb {
                // The helium star of the helium core at the core's luminosity,
                // R_HeGB(Mc,He, Lc) = min(R₁, R₂) (HPT section 6.3 after equation 105; `hrdiag`
                // kw = 5), which Lc's blend at SSE's τ puts anywhere up to its brightest (ruling
                // p11-stripped-core, amendments 1 and 2): the held state keeps no τ to check it by.
                let bound = crate::stellar::sse::early_agb_core_radius_bound(mc).min(r);
                assert!(
                    rc > 0.0 && rc <= bound * (1.0 + 1e-12),
                    "a held early-AGB star's core radius {rc} R☉ is not a helium giant's of a \
                     {mc} M☉ core (at most {bound} R☉): {}",
                    what()
                );
                continue;
            }
            let candidates = [
                0.0,
                crate::stellar::sse::helium_zams_radius(mc).min(r),
                (5.0 * white_dwarf_radius(RemnantRecipe::default(), SolarMasses::new(mc)).value())
                    .min(r),
            ];
            assert!(
                candidates.iter().any(|c| c.total_cmp(&rc).is_eq()),
                "a held {:?} star's core radius {rc} R☉ is none of sse's {candidates:?}: {}",
                state.phase(),
                what()
            );
        }
    }
}

/// P11.T4's invariants over a sample of close pairs (the plan's 10³ in the slow suite).
#[test]
fn close_pairs_keep_the_engines_invariants() {
    for input in sample_pairs(60, 0x5eed_0001) {
        check_invariants(&input, Years::new(1.2e10));
    }
}

/// Ruling 129.4a: a star's death takes its mass at the death, not through the orbit's angular
/// momentum in the step before it. Two pairs of the 10³ below whose detached segments widened
/// ×24.6 and ×8.6 past their mass loss: a pinned primary stripped on its early AGB, whose helium
/// giant dies before plan 06's age and was held at its remnant's mass, and a black hole's
/// companion, a helium star whose collapse the orbit felt twice.
#[test]
fn a_death_does_not_widen_the_orbit_through_the_mass_it_takes() {
    for (m1, m2, period, e) in [
        (
            10.365_251_834_098_531,
            5.536_839_708_853_019,
            2_974.736_093_124_113_7,
            0.276_692_339_410_206_5,
        ),
        (
            20.516_256_252_776_362,
            17.721_739_714_111_806,
            44.824_683_315_930_756,
            0.059_032_969_727_265_655,
        ),
    ] {
        check_invariants(&pair(m1, m2, period, e, 0.02), Years::new(1.2e10));
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
        0.0,
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
            0.0,
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

/// Rulings 108.2 and 111.4: with `α_CE` = 1 and λ = 0.5, the common envelopes of first-giant-branch
/// progenitors of 1.0–1.5 M☉ with 0.3 M☉ companions leave white dwarf and main-sequence pairs in
/// the post-common-envelope binaries' observed range of periods, 1.9 h (Nebot Gómez-Morán et al.
/// 2011, A&A 536, A43) to 4.36 d, the longest of Zorotovic et al.'s sample, SDSS J1434+5335 at
/// 4.357 d (2010, A&A 520, A86, Table A.1): none shorter, and none longer but at most one from the
/// widest orbit, which meets the giant at the tip of its branch. As built the longest in range is
/// 4.35 d.
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
    let (shortest, longest) = (1.9 / 24.0, 4.36);
    assert!(periods.iter().all(|p| p.2 >= shortest), "{periods:?}");
    // The allowance, α = 1's known excess (ruling 111.4; Zorotovic et al. 2010 find α 0.2–0.3),
    // recorded in plan 11's Risks: the widest orbit, which reaches the giant at the tip of its
    // branch where its envelope is least bound, may land above 4.36 d (7.5 d from 1.0 M☉ at 450 d
    // as built), and no more than one pair.
    let above: Vec<_> = periods.iter().filter(|p| p.2 > longest).collect();
    assert!(
        above.len() <= 1 && above.iter().all(|p| p.1 >= 450.0),
        "{periods:?}"
    );
}

/// An engine holding a 1.5 M☉ main-sequence donor and a 1.2 M☉ main-sequence accretor in
/// transfer, on a circular orbit sized so that the accretor overfills its Roche lobe by
/// `overfill` of its radius, with the transfer begun at age 0 and the accretor first touching its
/// lobe at `touched`, years: for ruling 114's cases.
fn contact_engine(overfill: f64, touched: f64) -> super::evolve::Engine {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit, roche_lobe};
    use super::star::{Member, Path};
    use super::timeline::{Component, Context};

    let (m_donor, m_accretor) = (1.5, 1.2);
    let input = pair(m_donor, m_accretor, 1.0, 0.0, 0.02);
    let star = |m: f64, tau: f64| Member::MainSequence {
        helium: false,
        mass: Path::starting(0.0, m),
        tau: Path::starting(0.0, tau),
    };
    let k = input.orbit();
    let ctx = Arc::new(Context::of(&input));
    let at_unit = LiveOrbit::new(0.0, 1.0, 0.0, *k.orientation(), k.mean_anomaly_at_epoch());
    let mut engine = Engine::new(
        Arc::clone(&ctx),
        0.0,
        1.0e10,
        [star(m_donor, 0.5), star(m_accretor, 0.3)],
        at_unit,
        None,
    );
    let radius = engine
        .structure(1, 0.0, m_accretor, 0.3)
        .expect("a main-sequence accretor")
        .state
        .radius()
        .value();
    // The lobe is linear in the separation.
    let a = radius / ((1.0 + overfill) * roche_lobe(m_accretor, m_donor, 1.0));
    engine.orbit = Some(LiveOrbit::new(
        0.0,
        a,
        0.0,
        *k.orientation(),
        k.mean_anomaly_at_epoch(),
    ));
    engine.kind = SegmentKind::StableTransfer {
        donor: Component::of_index(0),
    };
    let lifetime = crate::stellar::sse::main_sequence_lifetime(ctx.coeffs(), false, m_donor);
    engine.overflow_onset = (0.0, lifetime);
    engine.first_contact = Some(touched);
    engine
}

/// The donor's thermal rate M ÷ `τ_KH` in [`contact_engine`]'s pair, M☉ yr⁻¹, and the lighter
/// star's thermal timescale, years.
fn contact_rates(engine: &super::evolve::Engine) -> (f64, f64) {
    use super::rlof::kelvin_helmholtz;
    use super::star::Kind;

    let thermal = |i: usize| {
        let (m, tau) = engine.current(i);
        let s = engine.structure(i, engine.age, m, tau).expect("a star");
        kelvin_helmholtz(&s, Kind::of(s.state.phase(), m))
    };
    (engine.current(0).0 / thermal(0), thermal(1))
}

/// Ruling 114.2: a rapid (AR) contact, reached at twice the donor's thermal rate soon after the
/// onset, whose accretor overfills its lobe by 5% is temporary: the pair goes on in semi-detached
/// transfer and is not marked in contact. At 9% it is still temporary; the same pair touching its
/// lobe too late for case AR (a fifth of the donor's main sequence after the onset) is not.
#[test]
fn a_shallow_rapid_contact_returns_to_semi_detached_transfer() {
    for overfill in [0.05, 0.09] {
        let mut engine = contact_engine(overfill, 1.0e5);
        let (thermal_rate, _) = contact_rates(&engine);
        assert!(engine.contact_relaxes(0, 2.0 * thermal_rate), "{overfill}");
        // What `transfer_phase` does on the step: the transfer goes on.
        assert!(
            !engine.accretor_contact(0, 2.0 * thermal_rate),
            "{overfill}"
        );
        assert!(matches!(engine.kind, SegmentKind::StableTransfer { .. }));
        assert!(engine.segments.is_empty(), "{:?}", engine.segments);
        assert_eq!(engine.first_contact, Some(1.0e5));
    }
    // The first contact of a transfer is its time.
    let mut first = contact_engine(0.05, 0.0);
    first.first_contact = None;
    let (thermal_rate, _) = contact_rates(&first);
    assert!(!first.accretor_contact(0, 2.0 * thermal_rate));
    assert_eq!(first.first_contact, Some(first.age));
    let engine = contact_engine(0.05, 0.0);
    let (thermal_rate, _) = contact_rates(&engine);
    // Slow transfer is the W Ursae Majoris channel, never temporary.
    assert!(!engine.contact_relaxes(0, 0.5 * thermal_rate));
    let (_, lifetime) = engine.overflow_onset;
    let late = contact_engine(0.05, 0.2 * lifetime);
    assert!(!late.contact_relaxes(0, 2.0 * thermal_rate));
    let mut late = late;
    late.contact(0, 2.0 * thermal_rate);
    assert_eq!(late.kind, SegmentKind::Contact);
    assert!(
        late.contact_until - late.age > 1.0e8,
        "{}",
        late.contact_until
    );
}

/// Ruling 114.2: a rapid (AR) contact whose accretor overfills its lobe by 20% ends the transfer
/// and merges on the lighter star's thermal timescale; ruling 114.1: one at twenty times the
/// donor's thermal rate (case AD) merges dynamically at once.
#[test]
fn a_deep_rapid_contact_merges() {
    let mut engine = contact_engine(0.20, 1.0e5);
    let (thermal_rate, light_thermal) = contact_rates(&engine);
    assert!(!engine.contact_relaxes(0, 2.0 * thermal_rate));
    assert!(engine.accretor_contact(0, 2.0 * thermal_rate));
    assert!(
        matches!(
            engine.segments.last().map(Segment::kind),
            Some(SegmentKind::StableTransfer { .. })
        ),
        "{:?}",
        engine.segments
    );
    assert_eq!(engine.kind, SegmentKind::Contact);
    let lasts = engine.contact_until - engine.age;
    assert!(
        (lasts - light_thermal).abs() <= 1e-9 * light_thermal,
        "{lasts} yr against {light_thermal}"
    );
    engine.contact_phase();
    assert_eq!(engine.kind, SegmentKind::Merged);
    let mut dynamic = contact_engine(0.05, 1.0e5);
    assert!(!dynamic.contact_relaxes(0, 20.0 * thermal_rate));
    dynamic.contact(0, 20.0 * thermal_rate);
    assert_eq!(dynamic.kind, SegmentKind::Merged);
}

/// Ruling 129.4b: transfer from a helium Hertzsprung-gap star onto a neutron star or black hole
/// is stable at any mass ratio (Tauris et al. 2015, section 6), and onto any other accretor above
/// BSE's 0.784 it is still a common envelope.
#[test]
fn case_bb_transfer_onto_a_compact_star_is_stable() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit};
    use super::rlof::Stability;
    use super::star::{Member, Path};
    use super::timeline::Context;
    use crate::stellar::sse::Track;

    let input = pair(4.0, 1.4, 1.0, 0.0, 0.02);
    let ctx = Arc::new(Context::of(&input));
    let helium = Arc::new(Track::helium_star_full(
        SolarMasses::new(4.0),
        &Composition::SOLAR,
        &StarDraws::median(),
    ));
    let gap = helium
        .age_in_phase(Phase::HeliumHertzsprungGap, 0.5)
        .expect("a 4 M☉ helium star crosses its gap");
    let mass = helium.mass_at(gap);
    let k = input.orbit();
    let stability = |accretor: Member| {
        let engine = Engine::new(
            Arc::clone(&ctx),
            gap,
            1.0e10,
            [
                Member::Track {
                    track: Arc::clone(&helium),
                    offset: 0.0,
                },
                accretor,
            ],
            LiveOrbit::new(gap, 5.0, 0.0, *k.orientation(), k.mean_anomaly_at_epoch()),
            None,
        );
        engine.stability(0)
    };
    for phase in [Phase::NeutronStar, Phase::BlackHole] {
        let compact = Member::Remnant {
            phase,
            birth: 0.0,
            origin: crate::units::Megayears::ZERO,
            mass: Path::starting(0.0, 1.4),
        };
        assert_eq!(
            stability(compact),
            Stability::Stable,
            "{mass} M☉ onto {phase:?}"
        );
    }
    let dwarf = Member::Remnant {
        phase: Phase::CarbonOxygenWhiteDwarf,
        birth: 0.0,
        origin: crate::units::Megayears::ZERO,
        mass: Path::starting(0.0, 1.0),
    };
    assert_eq!(
        stability(dwarf),
        Stability::CommonEnvelope,
        "{mass} M☉ onto a white dwarf"
    );
}

/// Ruling 129.4c: a pinned primary held at its bare core after a common envelope with a neutron
/// star (pair 0911 of the pinned thousand: two 12.6 M☉ stars, whose double common envelope leaves
/// two helium stars 2 R☉ apart) has no envelope left to eject when it touches its companion
/// again, and merges with it rather than meeting it envelope after envelope to the cap.
#[test]
fn a_held_bare_core_that_touches_its_companion_merges() {
    let mut mix = Mix(0x0b1e_0001);
    let input = (0..=911_u32)
        .map(|i| pinned_pair(&mut mix, i))
        .last()
        .expect("pair 0911");
    let timeline = evolve(&input, Years::new(1.2e10));
    assert!(!timeline.hit_segment_cap(), "{}", describe(&timeline));
    assert!(timeline.merger_age().is_some(), "{}", describe(&timeline));
}

/// Ruling 132.3: pair 0077 of the pinned thousand (2.23 + 1.16 M☉ at 0.56 d) feeds its companion
/// to the end of its main sequence. The rejuvenated accretor leaves it at 1,052.655 Myr, once,
/// where the steps used to close in on its receding end until the segment cap.
#[test]
fn a_rejuvenated_accretor_leaves_its_main_sequence_once() {
    let mut mix = Mix(0x0b1e_0001);
    let input = (0..=77_u32)
        .map(|i| pinned_pair(&mut mix, i))
        .last()
        .expect("pair 0077");
    let timeline = evolve(&input, Years::new(1.2e10));
    assert!(!timeline.hit_segment_cap(), "{}", describe(&timeline));
    let left = timeline
        .segments()
        .iter()
        .map(|s| timeline.state_at(s.start()))
        .find(|state| {
            state.stars()[1].phase() != Phase::MainSequence && state.age().value() > 1.0e9
        })
        .map(|state| state.age().value())
        .expect("the accretor leaves its main sequence");
    assert!(
        (left - 1.052_655e9).abs() <= 1.0e3,
        "left at {left} yr: {}",
        describe(&timeline)
    );
}

/// P11.T4.g: a star the binary carries that has no envelope left on the step that lands on its
/// phase boundary is stripped first (BSE's `hrdiag` before `evolv2`), not taken through the
/// boundary: a 5 M☉ star carried at 10⁻⁷ M☉ above its core 10 yr before the end of its
/// Hertzsprung gap, whose core grows by 4 × 10⁻⁷ M☉ in those 10 yr while its wind takes
/// 10⁻¹⁰ M☉, leaves the step that lands on the boundary as a naked helium star, marked stripped.
#[test]
fn a_bare_star_at_its_phase_boundary_is_stripped_first() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit};
    use super::star::{Member, Path};
    use super::timeline::Context;
    use crate::stellar::sse::Track;

    let input = pair(5.0, 1.0, 1.0e5, 0.0, 0.02);
    let ctx = Arc::new(Context::of(&input));
    let track = Arc::new(Track::full(
        SolarMasses::new(5.0),
        &Composition::SOLAR,
        &StarDraws::median(),
    ));
    let gap = track
        .age_in_phase(Phase::HertzsprungGap, 0.5)
        .expect("a 5 M☉ star crosses the gap");
    let (_, end) = track.phase_span(gap);
    let t = end - 10.0;
    let core = track.structure_at(t, 5.0).state.core_mass().value();
    let core_at_end = track
        .structure_at(end * (1.0 - 1e-15), 5.0)
        .state
        .core_mass()
        .value();
    assert!(
        core_at_end > core + 1e-7,
        "the core grows past the mass in the step"
    );
    let companion = Member::MainSequence {
        helium: false,
        mass: Path::starting(t, 1.0),
        tau: Path::starting(t, 0.1),
    };
    let bare = Member::Shaped {
        track: Arc::clone(&track),
        offset: 0.0,
        mass: Path::starting(t, core + 1e-7),
    };
    let k = input.orbit();
    let mut engine = Engine::new(
        Arc::clone(&ctx),
        t,
        1.0e10,
        [bare, companion],
        LiveOrbit::new(t, 1.0e5, 0.0, *k.orientation(), k.mean_anomaly_at_epoch()),
        None,
    );
    engine.detached_phase();
    assert!(engine.stripped[0], "{}", engine.members[0].form());
    let (m, tau) = engine.current(0);
    let state = engine
        .structure(0, engine.age, m, tau)
        .expect("a helium star")
        .state;
    assert_eq!(state.phase(), Phase::HeliumMainSequence, "{state:?}");
}
