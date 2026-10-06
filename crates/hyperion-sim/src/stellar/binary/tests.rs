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
/// to the end of its main sequence. The rejuvenated accretor leaves it at 1,052.889 Myr, once,
/// where the steps used to close in on its receding end until the segment cap. (1,052.655 Myr
/// before P11.T4.i, which starts the engine at the primary's arrival, 7 Myr before the
/// companion's, and so moves its knots.)
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
        (left - 1.052_889e9).abs() <= 1.0e3,
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

/// Regression (P11's protostar mergers, 2026-10-05): a pair of 2.27 and 2.21 M☉ at 1 d, run to
/// 0.4 Myr, before either star reaches the main sequence (at 5.1 and 5.5 Myr). A star's arrival
/// was read from its track's built segments alone, so a track built short of its main sequence had
/// none, and the engine stepped from zero age, where the protostars overfill the orbit: it merged
/// them at once into a 0.01 M☉ cooling star beside nothing. Now the pair waits for the first
/// arrival (`evolve.rs`, `arrival`; for both, before P11.T4.i): to 0.4 Myr it is two protostars,
/// each its own track's and of its own accreted mass, on the drawn orbit, as a run to a later age
/// has it.
#[test]
fn a_pair_run_short_of_the_main_sequence_stays_two_protostars() {
    use crate::stellar::sse::Track;

    let input = pair(2.27, 2.21, 1.0, 0.0, 0.02);
    let until = Years::new(4.0e5);
    let early = evolve(&input, until);
    let later = evolve(&input, Years::new(1.0e8));
    assert_eq!(early.segments().len(), 1, "{}", describe(&early));
    assert_eq!(early.segments()[0].kind(), SegmentKind::Detached);
    assert_eq!(early.merger_age(), None);
    let own = [0, 1].map(|i| {
        Track::to_age(
            input.masses()[i],
            input.composition(),
            &input.draws()[i],
            until,
        )
    });
    for k in 1..=40_u32 {
        let age = Years::new(until.value() * f64::from(k) / 40.0);
        let state = early.state_at(age);
        assert_eq!(state, later.state_at(age), "at {age:?}");
        assert_eq!(state.orbit(), Some(input.orbit()), "at {age:?}");
        for (star, track) in state.stars().iter().zip(&own) {
            assert_eq!(star.phase(), Phase::Protostar, "{star:?}");
            assert_eq!(star, &track.state_at(age));
        }
    }
}

/// Regression (P11's build-age dependence, 2026-10-05): a pair of 8.26 and 7.92 M☉ at 2.31 d, run
/// to 37.085 Myr, transferring mass on its primary's Hertzsprung gap. The track the primary was
/// carried on from the end of its main sequence was built to a reach guessed from the closed-form
/// lifetime, which fell short of its own gap: the pair's age lay past the track's end, the engine
/// stopped on the track's last boundary 4,096 times, to its cap, and the stars froze. A run past
/// the next segment had the track built further and went on. Now the track is built again from
/// where the star is placed on it (`evolve.rs`, `track_reaching`), and the two runs agree.
#[test]
fn a_track_built_short_of_the_pairs_age_is_built_again() {
    use crate::Seed;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};

    let (m1, m2) = (8.255_234_890_079_752, 7.924_133_480_725_55);
    let orbit = KeplerElements::from_period(
        Seconds::new(2.306_744_539_916_993_5 * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.3), Radians::new(0.1), Radians::new(0.2))
            .expect("an orientation"),
        Radians::new(1.0),
    )
    .expect("an orbit");
    let cell = GenCell::new(CellSize::Ly8, [3_058, 9, 0]).expect("a cell");
    let system = SystemId::from_parts(Layer::A, cell, 0).expect("a system");
    let draws = [0, 1].map(|k| StarDraws::for_star(Seed::new(0x0b1e_5eee), BodyId::new(system, k)));
    let input = BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::from_fe_h(
            crate::units::Dex::new(0.0),
            crate::units::HeliumExcess::ZERO,
        ),
        orbit,
        draws,
        Years::new(1.0e9),
    )
    .expect("a pair");
    let until = Years::new(37_085_007.925_425_24);
    let early = evolve(&input, until);
    let later = evolve(&input, Years::new(9.0e7));
    assert!(!early.hit_segment_cap(), "{}", describe(&early));
    assert_same_history(&early, &later, until);
    // Just past the end of the primary's main sequence (36.704 Myr) the rebuilt track once held no
    // gap at all, and the star was placed at its track's age zero, a protostar.
    assert!(check_after_events(&input, Years::new(4.0e7))[0] > 20);
}

/// The ages just past each event of `input`'s run to 1.5 × 10¹⁰ years, up to `last`: each
/// segment's start plus 10⁻³, 1, 10², 10⁴ and 10⁶ years, where a dependence on the run-to age
/// shows if anything does (a track built short of a phase just entered).
fn ages_after_events(later: &BinaryTimeline, last: Years) -> Vec<Years> {
    let mut ages: Vec<f64> = later
        .segments()
        .iter()
        .map(|s| s.start().value())
        .filter(|&start| start > 0.0)
        .flat_map(|start| [1e-3, 1.0, 1e2, 1e4, 1e6].map(|after| start + after))
        .filter(|&age| age <= last.value())
        .collect();
    ages.sort_by(f64::total_cmp);
    ages.dedup_by(|a, b| a.total_cmp(b).is_eq());
    ages.into_iter().map(Years::new).collect()
}

/// The ages between `input`'s two arrivals on the main sequence, where its stars' arrivals differ
/// (P11.T4.i: the engine runs from the first, with the later star its own zero-age self): the
/// first arrival plus 10⁻³, 1, 10², 10⁴ and 10⁶ years, and halfway, each before the later one.
fn ages_between_arrivals(input: &BinaryInput) -> Vec<Years> {
    let Some((first, late)) = first_and_late(input) else {
        return Vec::new();
    };
    [1e-3, 1.0, 1e2, 1e4, 1e6]
        .map(|after| first + after)
        .into_iter()
        .chain([first + 0.5 * (late - first)])
        .filter(|&age| age < late)
        .map(Years::new)
        .collect()
}

/// [`check_any_age`]'s first case at each of [`ages_after_events`] and
/// [`ages_between_arrivals`]: every run of `input` to an age just past one of its events, or
/// between its stars' arrivals, that the pre-test passes has the history of the run to
/// 1.5 × 10¹⁰ years, bit for bit, and meets no cap. The number of runs compared just past events
/// and between the arrivals.
fn check_after_events(input: &BinaryInput, last: Years) -> [usize; 2] {
    let later = evolve(input, Years::new(1.5e10));
    let mut compared = [0; 2];
    let ages = ages_after_events(&later, last)
        .into_iter()
        .map(|age| (0, age))
        .chain(ages_between_arrivals(input).into_iter().map(|age| (1, age)));
    for (kind, until) in ages {
        if !can_interact(input, until) {
            continue;
        }
        let early = evolve(input, until);
        assert!(
            !early.hit_segment_cap(),
            "to {until:?}: {}",
            describe(&early)
        );
        assert_same_history(&early, &later, until);
        compared[kind] += 1;
    }
    compared
}

/// [`check_after_events`] over the first `n` pairs of the pinned sample of seed `seed`, summed.
fn after_events_over(n: u32, seed: u64) -> [usize; 2] {
    let mut mix = Mix(seed);
    (0..n)
        .map(|i| check_after_events(&pinned_pair(&mut mix, i), Years::new(1.2e10)))
        .fold([0; 2], |[a, b], [c, d]| [a + c, b + d])
}

/// P11's build-age dependence (2026-10-05, the determinism audit's sweep): runs to just past each
/// event of eight pinned-sample pairs, and between their stars' arrivals (P11.T4.i), agree with a
/// run to 1.5 × 10¹⁰ years.
#[test]
fn a_timeline_run_to_just_past_an_event_is_the_same() {
    // None of these eight pairs passes the pre-test between its arrivals
    // (`a_late_companion_meets_its_primarys_envelope` compares such runs directly).
    let [events, _] = after_events_over(8, 0x0b1e_00ab);
    assert!(events > 50, "{events} runs compared just past events");
}

/// [`a_timeline_run_to_just_past_an_event_is_the_same`] over 100 pairs.
#[test]
#[ignore = "slow: about 10⁴ binaries run through the engine"]
fn a_hundred_timelines_run_to_just_past_their_events_are_the_same() {
    let [events, between] = after_events_over(100, 0x0b1e_00ac);
    eprintln!("{events} runs compared just past events, {between} between arrivals");
    assert!(events > 500, "{events} runs compared just past events");
    assert!(between > 0, "{between} runs compared between arrivals");
}

/// P11's build-age dependence (2026-10-05): a track the engine rebuilds holds its star to the
/// pair's age where the reach guessed from the closed forms falls short, at both of
/// [`super::evolve::track_reaching`]'s call sites. A 5 M☉ star's gap starts after its closed-form
/// main sequence ends, so a build to just past that end has no gap, which `after_boundary` once
/// placed at track age 0; and a build that holds a main sequence at τ = 0.999 ends before a span
/// past it, which `main_sequence_star` once left short.
#[test]
fn a_rebuilt_track_reaches_the_pairs_age() {
    use hyperion_testkit::float::bits;

    use super::evolve::track_reaching;
    use crate::stellar::sse::{self, Track};

    let (m, comp, draws) = (
        SolarMasses::new(5.0),
        Composition::SOLAR,
        StarDraws::median(),
    );
    let full = Track::full(m, &comp, &draws);
    let build = |reach: f64| Track::to_age(m, &comp, &draws, Years::new(reach));
    let coeffs = sse::ZCoeffs::new(comp.z_fit());
    let lifetime = sse::main_sequence_lifetime(&coeffs, false, m.value());
    let start = sse::main_sequence_start(m, &comp);

    let gap = full
        .age_in_phase(Phase::HertzsprungGap, 0.0)
        .expect("a 5 M☉ star crosses the gap");
    let guess = start + lifetime;
    assert!(
        guess < gap,
        "the guess {guess} yr falls short of the gap at {gap} yr"
    );
    let span = 0.5 * (gap - guess);
    let gap_of = |track: &Track| track.age_in_phase(Phase::HertzsprungGap, 0.0);
    assert_eq!(gap_of(&build(guess + span)), None, "built short of the gap");
    let (track, placed) = track_reaching(guess, span, build, gap_of);
    assert_eq!(placed.map(bits), Some(bits(gap)));
    assert!(track.built_until().value() >= gap + span);

    let tau = 0.999;
    let on = full
        .age_in_phase(Phase::MainSequence, tau)
        .expect("a main sequence");
    let guess = start + tau * lifetime;
    let span = 2.0 * (gap - on);
    let on_of = |track: &Track| track.age_in_phase(Phase::MainSequence, tau);
    assert!(
        build(guess + span).built_until().value() < on + span,
        "built short of the span"
    );
    let (track, placed) = track_reaching(guess, span, build, on_of);
    assert_eq!(placed.map(bits), Some(bits(on)));
    assert!(track.built_until().value() >= on + span);
}

/// That `early` and `later`, the same pair run to `until` and past it, have the same states at
/// every age to `until`: at 257 even ages, at each of `early`'s segments' starts and halfway
/// through each, bit for bit.
fn assert_same_history(early: &BinaryTimeline, later: &BinaryTimeline, until: Years) {
    let mut ages: Vec<f64> = (0..=256_u32)
        .map(|k| until.value() * f64::from(k) / 256.0)
        .collect();
    for s in early.segments() {
        let (start, end) = (s.start().value(), s.end().value().min(until.value()));
        ages.extend([start, start + 0.5 * (end - start)]);
    }
    for age in ages {
        let age = Years::new(age);
        assert_eq!(
            early.state_at(age),
            later.state_at(age),
            "at {age:?}:\n{}\nagainst\n{}",
            describe(early),
            describe(later)
        );
    }
}

/// How a pair run to an earlier age compares with the same pair run to a later one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EarlierRun {
    /// The engine ran the pair both times: the same history to the earlier age.
    Same,
    /// The pre-test passed over the pair at the earlier age (two single stars on the drawn orbit),
    /// and the later run met no interaction before it.
    PassedOver,
    /// The pre-test passed over the pair at the earlier age, but the later run holds an
    /// interaction before it ([`interaction_before`]): the pre-test failed to bound the decay.
    MissedInteraction,
}

/// Plan 11's consistency to any age (P11's build-age dependence, 2026-10-05), for `input` run to
/// `until` and to `later`. A pair that [`can_interact`] by `until` has the same history to `until`
/// as the later run, bit for bit ([`assert_same_history`]). One it passes over is one detached
/// segment of its stars' own forms on the drawn orbit, and each star on its own track is the later
/// run's to the later run's first event (a star below 0.1 M☉ on the cooling fits takes its
/// companion's wind in the later run).
fn check_any_age(input: &BinaryInput, until: Years, later: Years) -> EarlierRun {
    use super::star::Member;

    let early = evolve(input, until);
    let late = evolve(input, later);
    if can_interact(input, until) {
        assert!(!early.hit_segment_cap(), "{}", describe(&early));
        assert_same_history(&early, &late, until);
        return EarlierRun::Same;
    }
    assert_eq!(early.segments().len(), 1, "{}", describe(&early));
    // To the later run's first event, which is not compared unless it lies past `until`.
    let first_event = late.segments()[0].end();
    let (horizon, last) = if first_event < until {
        (first_event.value(), 63)
    } else {
        (until.value(), 64)
    };
    let members = late.segments()[0].members();
    for k in 0..=last {
        let age = Years::new(horizon * f64::from(k) / 64.0);
        let (a, b) = (early.state_at(age), late.state_at(age));
        for (i, member) in members.iter().enumerate() {
            if matches!(member, Member::Track { .. }) {
                assert_eq!(
                    a.stars()[i],
                    b.stars()[i],
                    "at {age:?}:\n{}",
                    describe(&late)
                );
            }
        }
    }
    if interaction_before(&late, until).is_some() {
        EarlierRun::MissedInteraction
    } else {
        EarlierRun::PassedOver
    }
}

/// The first segment of `timeline` that is an interaction starting before `until` and before the
/// pair's first supernova (ruling p11-channels of 2026-10-06, section 1.2): stable transfer, a
/// common envelope, contact, or a merger that leaves one star. A detached or disrupted segment is
/// none, and a supernova of a pair the pre-test passes over is not the pre-test's to foresee
/// (finding F3: plan 11's P11.T10 decides wide pairs' supernovae).
fn interaction_before(timeline: &BinaryTimeline, until: Years) -> Option<&Segment> {
    let first_supernova = timeline
        .supernovae()
        .iter()
        .map(|s| s.age().value())
        .fold(f64::INFINITY, f64::min);
    timeline.segments().iter().find(|s| {
        let interacts = match s.kind() {
            SegmentKind::Detached | SegmentKind::Disrupted { .. } => false,
            SegmentKind::Merged => one_star_left(timeline, s),
            SegmentKind::StableTransfer { .. }
            | SegmentKind::CommonEnvelope
            | SegmentKind::Contact => true,
        };
        interacts && s.start() < until && s.start().value() < first_supernova
    })
}

/// Whether one of the pair's stars is gone at the start of `segment` of `timeline`, as a merger
/// leaves it.
fn one_star_left(timeline: &BinaryTimeline, segment: &Segment) -> bool {
    timeline
        .state_at(segment.start())
        .stars()
        .iter()
        .any(|s| s.phase() == Phase::NoRemnant)
}

/// Runs `n` pinned-sample pairs ([`pinned_pair`]), each to an age log-uniform in 10⁵–1.2 × 10¹⁰
/// years, and one whose stars' arrivals differ also to an age uniform between them (P11.T4.i), and
/// each to 1.5 × 10¹⁰, through [`check_any_age`], and returns how many runs were the same, passed
/// over and missed interactions, at the first ages and between the arrivals. The ages between
/// arrivals are drawn on a stream of their own, so that the pairs and their first ages are the
/// sample's as before.
fn any_age_over(n: u32, seed: u64) -> [[u32; 3]; 2] {
    let mut mix = Mix(seed);
    let mut between = Mix(seed ^ 0x0a77_1fa1_0000_0000);
    let mut counts = [[0_u32; 3]; 2];
    for i in 0..n {
        let input = pinned_pair(&mut mix, i);
        let until =
            crate::math::exp(crate::math::ln(1.0e5) + mix.next() * crate::math::ln(1.2e10 / 1.0e5));
        let mut ages = vec![(0, until)];
        if let Some((first, late)) = first_and_late(&input) {
            ages.push((1, first + (late - first) * between.next()));
        }
        for (kind, until) in ages {
            let outcome = check_any_age(&input, Years::new(until), Years::new(1.5e10));
            let slot = match outcome {
                EarlierRun::Same => 0,
                EarlierRun::PassedOver => 1,
                EarlierRun::MissedInteraction => 2,
            };
            counts[kind][slot] += 1;
        }
    }
    counts
}

/// The engine's history does not depend on the age it is run to (P11's build-age dependence,
/// 2026-10-05): [`check_any_age`] over 60 pairs, and between their stars' arrivals (P11.T4.i).
/// No pair the pre-test passes over interacts before its age in the later run (P11.T4.j: the
/// pre-test bounds the decay that magnetic braking, tides and gravitational radiation make).
#[test]
fn a_timeline_is_the_same_whatever_age_it_is_run_to() {
    // The runs between the arrivals are checked as the others are; none of them passes the
    // pre-test in this sample (`a_late_companion_meets_its_primarys_envelope` compares such runs
    // directly).
    let [[same, passed, missed], _] = any_age_over(60, 0x0b1e_00a9);
    assert!(same > 10 && passed > 10, "{same} run, {passed} passed over");
    assert_eq!(missed, 0, "{missed} interactions missed");
}

/// [`a_timeline_is_the_same_whatever_age_it_is_run_to`] over 10³ pairs, with the ages between
/// arrivals (P11.T4.i). No interaction is missed (P11.T4.j): before the decay-aware pre-test, 4 of
/// the first ages and 9 of the 721 between the arrivals were pairs under 1.4 d that braking or
/// tides bring into contact or a merger before the age, which the drawn-orbit test passed over.
#[test]
#[ignore = "slow: about 3.4 × 10³ binaries run through the engine"]
fn a_thousand_timelines_are_the_same_whatever_age_they_are_run_to() {
    let [
        [same, passed, missed],
        [same_between, passed_between, missed_between],
    ] = any_age_over(1_000, 0x0b1e_00aa);
    eprintln!(
        "{same} run at both ages, {passed} passed over, {missed} interactions missed; between \
         arrivals {same_between} run, {passed_between} passed over, {missed_between} missed"
    );
    assert_eq!(missed, 0, "{missed} interactions missed at the first ages");
    assert_eq!(
        missed_between, 0,
        "{missed_between} interactions missed between the arrivals"
    );
    assert!(same_between > 0, "{same_between} run between arrivals");
}

/// Each star's arrival on its main sequence, years ([`Track::main_sequence_arrival`], which needs
/// no main sequence built), or `None` for a star below 0.1 M☉ on the cooling fits.
///
/// [`Track::main_sequence_arrival`]: crate::stellar::sse::Track::main_sequence_arrival
fn arrivals(input: &BinaryInput) -> [Option<f64>; 2] {
    use crate::stellar::sse::{MIN_INITIAL_MASS, Track};

    [0, 1].map(|i| {
        let m = input.masses()[i];
        (m >= MIN_INITIAL_MASS).then(|| {
            Track::to_age(
                super::evolve::track_mass(m),
                input.composition(),
                &input.draws()[i],
                Years::ZERO,
            )
            .main_sequence_arrival()
            .expect("a hydrogen star arrives")
            .value()
        })
    })
}

/// The first and the later of `input`'s two arrivals, years, where both stars have one and they
/// differ.
fn first_and_late(input: &BinaryInput) -> Option<(f64, f64)> {
    let [Some(a), Some(b)] = arrivals(input) else {
        return None;
    };
    let (first, late) = if a <= b { (a, b) } else { (b, a) };
    (first < late).then_some((first, late))
}

/// Eggleton's (1983) Roche-lobe radius over the separation, `r_L ÷ a` (dimensionless), of a star of
/// `m` beside one of `other` (M☉): a star's reach, the periastron at which it fills its lobe, is
/// its radius over this.
fn lobe_fraction(m: f64, other: f64) -> f64 {
    crate::orbit::roche_lobe_radius(m / other, crate::units::Metres::new(1.0)).value()
}

/// A 1 M☉ star's track built past its arrival on the main sequence, the arrival (years) and its
/// structure there, at solar metallicity and the median draws.
fn one_solar_mass_late() -> (
    std::sync::Arc<crate::stellar::sse::Track>,
    f64,
    crate::stellar::sse::Structure,
) {
    let late = std::sync::Arc::new(crate::stellar::sse::Track::to_age(
        SolarMasses::new(1.0),
        &Composition::SOLAR,
        &StarDraws::median(),
        Years::new(1.0e8),
    ));
    let arrival = late
        .main_sequence_arrival()
        .expect("a 1 M☉ star arrives")
        .value();
    let zams = late.own_structure_at(arrival).expect("a star");
    (late, arrival, zams)
}

/// P11.T4.i (ruling p11-channels, 2026-10-06): a star that has not arrived on its main sequence is
/// its own zero-age main-sequence star to the engine, bit for bit its track's structure at the
/// arrival, at τ = 0 ([`engine_track_age_years`](super::star::engine_track_age_years)), while the
/// timeline shows it contracting on its own track.
#[test]
fn a_late_star_is_its_zero_age_self_to_the_engine() {
    use hyperion_testkit::float::bits;

    use super::star::Member;
    use super::timeline::Context;
    use crate::stellar::premain::PROTOSTAR_YEARS;

    let input = pair(15.0, 1.0, period_days(15.0, 1.0, 3_000.0), 0.0, 0.02);
    let ctx = Context::of(&input);
    let (late, arrival, zams) = one_solar_mass_late();
    assert_eq!(zams.state.phase(), Phase::MainSequence);
    assert_eq!(
        late.main_sequence_fraction(arrival).map(bits),
        Some(bits(0.0))
    );
    let member = Member::Track {
        track: std::sync::Arc::clone(&late),
        offset: 0.0,
    };
    for age in [
        PROTOSTAR_YEARS,
        1.0e6,
        0.5 * arrival,
        arrival * (1.0 - 1e-9),
    ] {
        let shown = late.state_at(Years::new(age));
        assert_eq!(shown.phase(), Phase::PreMainSequence, "at {age} yr");
        assert_eq!(member.state_at(&ctx, 1, age), shown, "at {age} yr");
        let read = member.evaluate(&ctx, 1, age, 1.0, 0.0).expect("a star");
        assert_eq!(format!("{read:?}"), format!("{zams:?}"), "at {age} yr");
        assert_eq!(
            member.radius(&ctx, 1, age, 1.0, 0.0).map(bits),
            Some(bits(zams.state.radius().value())),
            "at {age} yr"
        );
        assert_eq!(
            bits(member.mass_at(age)),
            bits(zams.state.mass().value()),
            "at {age} yr"
        );
        if age <= 0.5 * arrival {
            assert!(
                shown.radius().value() > zams.state.radius().value(),
                "a contracting star is larger than its zero-age self"
            );
        }
    }
    // From its arrival it is its own track's star again.
    let later = 1.5 * arrival;
    assert_eq!(
        format!("{:?}", member.evaluate(&ctx, 1, later, 1.0, 0.0)),
        format!("{:?}", late.own_structure_at(later))
    );
}

/// P11.T4.i: a star the binary carries before its arrival on the main sequence (a 1 M☉ companion
/// of a 15 M☉ primary past the end of its main sequence at 12.8 Myr, at 13.5 Myr) becomes a
/// main-sequence star of its track's mass at τ = 0 (`rlof.rs`'s `carry`), and the engine starts at
/// the primary's arrival. Before then it has its whole main sequence left
/// (`Engine::main_sequence_left`, which a contact reads), not the time to its arrival.
#[test]
fn a_late_star_is_carried_at_zero_age() {
    use std::sync::Arc;

    use hyperion_testkit::float::bits;

    use super::evolve::{Engine, LiveOrbit};
    use super::star::Member;
    use super::timeline::Context;
    use crate::stellar::sse::Track;

    let input = pair(15.0, 1.0, period_days(15.0, 1.0, 3_000.0), 0.0, 0.02);
    let (late, arrival, zams) = one_solar_mass_late();
    let (_, main_sequence_end) = late.phase_span(arrival);
    let primary = Arc::new(Track::to_age(
        SolarMasses::new(15.0),
        &Composition::SOLAR,
        &StarDraws::median(),
        Years::new(2.0e7),
    ));
    let members = [primary, late].map(|track| Member::Track { track, offset: 0.0 });
    let start = super::evolve::arrival(&members, 2.0e7);
    assert_eq!(
        bits(start),
        bits(
            members[0]
                .track()
                .and_then(|(t, _)| t.main_sequence_arrival())
                .expect("an arrival")
                .value()
        ),
        "the engine starts at the primary's arrival"
    );
    let k = input.orbit();
    let orbit = LiveOrbit::new(
        0.0,
        k.semi_major_axis().value() / crate::units::consts::SOLAR_RADIUS_M,
        0.0,
        *k.orientation(),
        k.mean_anomaly_at_epoch(),
    );
    let mut engine = Engine::new(
        Arc::new(Context::of(&input)),
        start,
        2.0e7,
        members,
        orbit,
        None,
    );
    engine.age = 1.35e7;
    assert_eq!(
        bits(engine.main_sequence_left(1)),
        bits(main_sequence_end - arrival),
        "a late star has its whole main sequence left"
    );
    engine.carry(1);
    match &engine.members[1] {
        Member::MainSequence {
            helium: false,
            mass,
            tau,
        } => {
            assert_eq!(bits(mass.last()), bits(zams.state.mass().value()));
            assert_eq!(bits(tau.last()), bits(0.0));
        }
        other => panic!("carried as {}", other.form()),
    }
}

/// P11.T4.i: the engine starts at the first arrival (`evolve.rs`'s `arrival`). Run to an age
/// between the two arrivals, a pair is tested with the later star's zero-age main-sequence radius,
/// not its contracting one: 5 + 1 M☉ at a periastron the contracting companion overfills but its
/// zero-age self does not is passed over. A pair on an orbit inside the companion's zero-age reach
/// (and so the primary's, which is the larger) is stepped from the first arrival and interacts in
/// its first steps, long before the companion's own arrival.
#[test]
fn the_engine_starts_at_the_first_arrival() {
    use std::sync::Arc;

    use hyperion_testkit::float::bits;

    use super::star::Member;
    use crate::stellar::sse::Track;

    let (m1, m2) = (5.0, 1.0);
    let base = pair(m1, m2, 10.0, 0.0, 0.02);
    let (first, late) = first_and_late(&base).expect("two arrivals");
    let tracks = [m1, m2].map(|m| {
        Arc::new(Track::to_age(
            SolarMasses::new(m),
            &Composition::SOLAR,
            &StarDraws::median(),
            Years::new(late * 1.1),
        ))
    });
    let members = tracks
        .clone()
        .map(|track| Member::Track { track, offset: 0.0 });
    assert_eq!(
        bits(super::evolve::arrival(&members, 1.0e10)),
        bits(first),
        "the first arrival"
    );
    assert_eq!(
        bits(first),
        bits(
            tracks[0]
                .main_sequence_arrival()
                .expect("an arrival")
                .value()
        )
    );

    let until = 2.0 * first;
    assert!(
        until < late,
        "{until} yr is before the companion's arrival at {late} yr"
    );
    let contracting = tracks[1].state_at(Years::new(until)).radius().value();
    let zams = tracks[1]
        .own_structure_at(late)
        .expect("a star")
        .state
        .radius()
        .value();
    let primary = tracks[0].max_radius_until(Years::new(until)).value();
    let reach = |r: f64, m: f64, other: f64| r / lobe_fraction(m, other);
    let (reach_zams, reach_contracting, reach_primary) = (
        reach(zams, m2, m1),
        reach(contracting, m2, m1),
        reach(primary, m1, m2),
    );
    let circular = |a_rsun: f64| pair(m1, m2, period_days(m1, m2, a_rsun), 0.0, 0.02);
    let between = reach_contracting.min(reach_primary.max(reach_zams) * 1.5);
    assert!(
        reach_primary.max(reach_zams) < between && between < reach_contracting,
        "a separation the contracting companion overfills alone: at {until:.4e} yr the companion \
         reaches {reach_contracting:.3} R☉ ({contracting:.3} R☉; zero-age {reach_zams:.3}, \
         {zams:.3} R☉), the primary {reach_primary:.3} R☉"
    );
    let apart = circular(between);
    assert!(
        !can_interact(&apart, Years::new(until)),
        "passed over at {between} R☉"
    );
    let run = evolve(&apart, Years::new(until));
    assert_eq!(run.segments().len(), 1, "{}", describe(&run));

    let touching = circular(0.95 * reach_zams);
    assert!(
        can_interact(&touching, Years::new(until)),
        "inside the zero-age reach"
    );
    let run = evolve(&touching, Years::new(until));
    let met = run
        .segments()
        .iter()
        .find(|s| s.kind() != SegmentKind::Detached)
        .expect("an interaction");
    assert!(
        met.start().value() >= first && met.start().value() <= first + 1e-3 * (late - first),
        "{:?} at {:?}, the first arrival at {first} yr: {}",
        met.kind(),
        met.start(),
        describe(&run)
    );
}

/// The primary's death as plan 06 gives it, for a pair of median draws at solar metallicity.
fn plan_06_death(input: &BinaryInput) -> Years {
    crate::stellar::system::StarModel::new(
        input.masses()[0],
        *input.composition(),
        StarDraws::median(),
        input.age_at_epoch(),
    )
    .expect("a star")
    .death()
    .expect("a massive star dies")
    .age()
}

/// The orbit of the two late-companion tests below, R☉: about half the drawn-orbit reach of a 15
/// or 20 M☉ red supergiant's largest radius (1,420 and 1,511 R☉) beside a 1 M☉ companion.
const LATE_COMPANION_ORBIT_RSUN: f64 = 1_200.0;

/// P11.T4.i (ruling p11-channels, 2026-10-06): a 1 M☉ companion of a 15 M☉ primary arrives on its
/// main sequence (at 37 Myr) long after the primary's death (at 14.3 Myr). On an orbit of 1,200 R☉
/// the supergiant fills its Roche lobe: a common envelope or a merger before plan 06's pinned
/// death, the collapse recorded at that death, and the companion shown contracting on its own
/// track until the interaction. Before T4.i the engine waited for both arrivals, after the death,
/// and the pair was one detached segment with no supernova record.
#[test]
fn a_late_companion_meets_its_primarys_envelope() {
    let input = pair(
        15.0,
        1.0,
        period_days(15.0, 1.0, LATE_COMPANION_ORBIT_RSUN),
        0.0,
        0.02,
    );
    let death = plan_06_death(&input);
    let (first, late) = first_and_late(&input).expect("two arrivals");
    assert!(
        late > death.value(),
        "the companion arrives after the death"
    );
    let timeline = evolve(&input, Years::new(5.0e7));
    let met = timeline
        .segments()
        .iter()
        .find(|s| s.kind() != SegmentKind::Detached)
        .expect("an interaction");
    assert!(
        matches!(
            met.kind(),
            SegmentKind::CommonEnvelope | SegmentKind::Merged
        ) && met.start() < death
            && met.start().value() > first,
        "{}",
        describe(&timeline)
    );
    let [primary, _] = timeline.supernova_ages();
    let primary = primary.expect("the primary's collapse is recorded");
    assert!(
        (primary.value() - death.value()).abs() <= 1e-9 * death.value(),
        "{primary:?} against plan 06's {death:?}: {}",
        describe(&timeline)
    );
    for k in 0..32_u32 {
        let age = Years::new(first + (met.start().value() - first) * f64::from(k) / 32.0);
        let shown = timeline.state_at(age).stars()[1];
        assert_eq!(shown.phase(), Phase::PreMainSequence, "at {age:?}");
    }
    // The build-age contract between the two arrivals, where the pair is run with its companion as
    // its zero-age self: from the age the pre-test first passes (the supergiant's drawn reach) to
    // the envelope, a run to an age there is the run to 50 Myr, bit for bit.
    let (mut lo, mut hi) = (first, met.start().value());
    assert!(
        can_interact(&input, Years::new(hi)),
        "the pre-test passes by the envelope"
    );
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if can_interact(&input, Years::new(mid)) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    assert!(
        hi < met.start().value(),
        "the pre-test passes before the envelope"
    );
    for share in [0.25, 0.5, 0.75] {
        let until = Years::new(hi + share * (met.start().value() - hi));
        assert_eq!(
            check_any_age(&input, until, Years::new(5.0e7)),
            EarlierRun::Same,
            "to {until:?}"
        );
    }
}

/// P11.T4.i: a 1 M☉ companion of a 20 M☉ primary (dead at 9.9 Myr, the companion arriving at
/// 37 Myr), on an orbit of 1,200 R☉ the primary's giant reaches, passes the pre-test. The common
/// envelope leaves the helium star and the companion on a close orbit, and the primary's collapse
/// at plan 06's death is applied to it (BSE appendix A1): recorded, and the pair bound or not as
/// the state after it shows. Without a kick that is Blaauw's criterion on a circular orbit: bound
/// while less than half the pair's mass is lost. Before T4.i the engine started after the death,
/// recorded no supernova, and left the pair bound on the drawn orbit.
#[test]
fn a_late_companions_supernova_is_applied() {
    let (m1, m2) = (20.0, 1.0);
    for kicks in [false, true] {
        let params = BinaryParams {
            natal_kicks: kicks,
            ..BinaryParams::GENERATOR
        };
        let input = pair(
            m1,
            m2,
            period_days(m1, m2, LATE_COMPANION_ORBIT_RSUN),
            0.0,
            0.02,
        )
        .with_params(params);
        let death = plan_06_death(&input);
        let (_, late) = first_and_late(&input).expect("two arrivals");
        assert!(
            late > death.value(),
            "the companion arrives after the death"
        );
        assert!(
            can_interact(&input, Years::new(5.0e7)),
            "inside the drawn reach"
        );
        let timeline = evolve(&input, Years::new(5.0e7));
        assert!(
            timeline
                .segments()
                .iter()
                .any(|s| s.kind() == SegmentKind::CommonEnvelope && s.start() < death),
            "a common envelope before the death: {}",
            describe(&timeline)
        );
        let record = timeline
            .supernovae()
            .iter()
            .find(|s| s.component() == Component::Primary)
            .expect("the primary's supernova is recorded");
        assert!(
            (record.age().value() - death.value()).abs() <= 1e-9 * death.value(),
            "{:?} against plan 06's {death:?}",
            record.age()
        );
        let before = timeline.state_at(Years::new(death.value() * (1.0 - 1e-9)));
        let after = timeline.state_at(Years::new(death.value() * (1.0 + 1e-9)));
        assert!(
            before.orbit().is_some(),
            "bound before: {}",
            describe(&timeline)
        );
        assert_eq!(
            after.orbit().is_some(),
            record.bound(),
            "{}",
            describe(&timeline)
        );
        if !kicks {
            let e = before.orbit().map_or(1.0, |o| o.eccentricity().value());
            assert!(e < 1e-9, "circular before the collapse: e {e}");
            let total = before.total_mass().value();
            let lost = total - (record.remnant().mass().value() + before.stars()[1].mass().value());
            assert!(lost > 0.0, "the collapse loses mass: {lost} M☉");
            assert_eq!(
                record.bound(),
                lost < 0.5 * total,
                "{lost} of {total} M☉ lost: {}",
                describe(&timeline)
            );
        }
    }
}

/// P11.T4.j: a 1.04 + 0.45 M☉ pair at a = 3.75 R☉ (P = 0.69 d) whose stars stay inside their
/// lobes on the drawn orbit is brought into transfer and contact by magnetic braking at about
/// 2.2 Gyr. The pre-test passes it at 2.0 Gyr, so a run to 2.0 Gyr is the run to 3 Gyr to then,
/// bit for bit; before P11.T4.j it was two single stars on the drawn orbit.
#[test]
fn a_braked_pair_is_run_before_its_contact() {
    let input = pair(1.04, 0.45, period_days(1.04, 0.45, 3.75), 0.0, 0.004);
    let at = Years::new(2.0e9);
    assert!(
        !lobe_reached(&input, at),
        "inside the lobes on the drawn orbit"
    );
    assert!(can_interact(&input, at));
    let later = evolve(&input, Years::new(3.0e9));
    let stages = starts(&later);
    let transfer = find(&stages, 0, |kind, _| *kind != SegmentKind::Detached)
        .expect("braking brings the pair into contact");
    assert!(
        matches!(
            stages[transfer].0,
            SegmentKind::StableTransfer {
                donor: Component::Primary
            }
        ),
        "{}",
        describe(&later)
    );
    let contact =
        find(&stages, transfer, |kind, _| *kind == SegmentKind::Contact).expect("a contact pair");
    let [onset, touch] = [transfer, contact].map(|k| later.segments()[k].start().value());
    assert!(
        (2.1e9..2.3e9).contains(&onset) && (2.15e9..2.35e9).contains(&touch),
        "transfer at {onset} yr, contact at {touch} yr:\n{}",
        describe(&later)
    );
    // The orbit shrinks by braking alone before the transfer: a third of its axis.
    let before = later.state_at(Years::new(onset * (1.0 - 1e-9)));
    let a = before
        .orbit()
        .map_or(f64::INFINITY, |o| o.semi_major_axis().value())
        / crate::units::consts::SOLAR_RADIUS_M;
    assert!(a < 2.5, "a = {a} R☉ at the onset");
    assert_same_history(&evolve(&input, at), &later, at);
}

/// The full tracks of `input`'s stars, where they have tracks (not below 0.1 M☉), as
/// [`can_interact_with_tracks`] takes them.
fn full_tracks(input: &BinaryInput) -> [Option<std::sync::Arc<crate::stellar::sse::Track>>; 2] {
    use crate::stellar::sse::{MIN_INITIAL_MASS, Track};
    core::array::from_fn(|i| {
        let m = super::evolve::track_mass(input.masses()[i]);
        (m >= MIN_INITIAL_MASS)
            .then(|| std::sync::Arc::new(Track::full(m, input.composition(), &input.draws()[i])))
    })
}

/// P11.T4.j: the pre-test only widens with age, and reads the same on any build of the stars'
/// tracks. Over the 60 pairs of [`a_timeline_is_the_same_whatever_age_it_is_run_to`] and four
/// massive wide pairs whose primary is pinned (design note 16), at 48 ages from 10⁵ to 1.5 × 10¹⁰
/// years, each pair's own age there and the ages between its stars' arrivals: a pair that can
/// interact by one age can by every later one; and [`can_interact`] (tracks built to the age) is
/// [`can_interact_with_tracks`] on full tracks, as `evolve`'s pinned primary and plan 06's
/// `SystemStars` give them, and on tracks built to 1.5 × 10¹⁰ years.
#[test]
fn the_pre_test_only_widens_with_age() {
    use crate::stellar::sse::{MIN_INITIAL_MASS, Track};

    let mut mix = Mix(0x0b1e_00a9);
    let massive = [
        (12.0, 0.3, 2_000.0),
        (12.0, 3.0, 4_000.0),
        (30.0, 1.0, 3_000.0),
        (30.0, 10.0, 6_000.0),
    ]
    .map(|(m1, m2, period)| (pair(m1, m2, period, 0.0, 0.02), None));
    let pairs = (0..60)
        .map(|i| {
            let input = pinned_pair(&mut mix, i);
            let own =
                crate::math::exp(crate::math::ln(1.0e5) + mix.next() * crate::math::ln(1.2e5));
            (input, Some(own))
        })
        .chain(massive);
    let mut passes = 0;
    for (i, (input, own)) in pairs.enumerate() {
        let built = |i: usize| {
            let m = super::evolve::track_mass(input.masses()[i]);
            (m >= MIN_INITIAL_MASS).then(|| {
                std::sync::Arc::new(Track::to_age(
                    m,
                    input.composition(),
                    &input.draws()[i],
                    Years::new(1.5e10),
                ))
            })
        };
        let long = [built(0), built(1)];
        let mut ages: Vec<f64> = (0..48)
            .map(|k| {
                crate::math::exp(
                    crate::math::ln(1.0e5) + f64::from(k) / 47.0 * crate::math::ln(1.5e5),
                )
            })
            .chain(own)
            .chain(ages_between_arrivals(&input).into_iter().map(Years::value))
            .collect();
        ages.sort_by(f64::total_cmp);
        let tracks = full_tracks(&input);
        let mut first_pass: Option<f64> = None;
        for age in ages {
            let until = Years::new(age);
            let can = can_interact_with_tracks(&input, until, tracks.clone());
            assert_eq!(can, can_interact(&input, until), "pair {i} at {age} yr");
            assert_eq!(
                can,
                can_interact_with_tracks(&input, until, long.clone()),
                "pair {i} at {age} yr"
            );
            if let Some(first) = first_pass {
                assert!(can, "pair {i} passes at {first} yr but not at {age} yr");
            } else if can {
                first_pass = Some(age);
                passes += 1;
            }
        }
    }
    assert!(passes > 10, "{passes} pairs pass at some age");
}

/// P11.T4.j: a wide pair is still two single stars. 1 + 0.8 M☉ at 10⁴ d fails at 13.8 Gyr. The
/// bound's boundary for a circular 1 + 0.8 M☉ pair ([Fe/H] 0, median draws) lies at P₀ ≈ 0.73 d
/// at 1 Gyr and 1.54 d at 10 Gyr, against the science check's 0.64 and 1.22 d from BSE equation
/// 50 at a locked spin alone (plan 11's Risks): above them by the spins' reservoir and the
/// safety factor, and over twice the lobe test's own boundary (0.30 and 0.54 d).
#[test]
fn a_wide_pair_is_still_passed_over() {
    assert!(!can_interact(
        &pair(1.0, 0.8, 1.0e4, 0.0, 0.02),
        Years::new(1.38e10)
    ));
    let tracks = full_tracks(&pair(1.0, 0.8, 1.0, 0.0, 0.02));
    let boundary = |until: f64, test: &dyn Fn(&BinaryInput, Years) -> bool| {
        let (mut lo, mut hi) = (0.1_f64, 30.0_f64);
        for _ in 0..40 {
            let mid = (lo * hi).sqrt();
            if test(&pair(1.0, 0.8, mid, 0.0, 0.02), Years::new(until)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    };
    let bound =
        |input: &BinaryInput, until: Years| can_interact_with_tracks(input, until, tracks.clone());
    let [p1, p10] = [1.0e9, 1.0e10].map(|u| boundary(u, &bound));
    let [l1, l10] = [1.0e9, 1.0e10].map(|u| boundary(u, &lobe_reached));
    eprintln!(
        "1 + 0.8 M☉: the bound passes P₀ ≤ {p1:.3} d at 1 Gyr and ≤ {p10:.3} d at 10 Gyr (science \
         check 0.64 and 1.22 d); the lobe test alone {l1:.3} and {l10:.3} d"
    );
    assert!((0.64..0.85).contains(&p1), "{p1} d at 1 Gyr");
    assert!((1.22..1.8).contains(&p10), "{p10} d at 10 Gyr");
    assert!(l1 < 0.5 * p1 && l10 < 0.5 * p10, "{l1} and {l10} d");
}

/// Finding F1 of ruling p11-channels (2026-10-06), the label: a white dwarf's birth that sheds
/// enough at once to unbind the orbit leaves both stars, a disruption by the star that died, not a
/// merger. The ruling's trace, 10.2 + 7.88 M☉ at P = 9,373 d and e = 0.45 (\[Fe/H\] 0, median
/// draws), run past the pre-test: at 43.9 Myr the secondary's super-AGB track goes from its early
/// AGB through the post-AGB (1.37 M☉) to an oxygen–neon white dwarf, and its death sheds the
/// envelope its progenitor still holds at once, through BSE appendix A1's instantaneous mass loss
/// with no kick (ruling 129.4a), which unbinds the orbit. The physics, an adiabatic superwind
/// instead, is left for its own ruling.
#[test]
fn a_white_dwarfs_birth_that_unbinds_the_orbit_disrupts_it() {
    let input = pair(10.2, 7.88, 9_373.0, 0.45, 0.02);
    let timeline = super::evolve::evolve_past_the_pre_test(&input, Years::new(1.0e8));
    let unbound = timeline
        .segments()
        .iter()
        .find(|s| s.kind() != SegmentKind::Detached)
        .expect("the orbit is unbound");
    assert_eq!(
        unbound.kind(),
        SegmentKind::Disrupted {
            by: Component::Secondary
        },
        "{}",
        describe(&timeline)
    );
    let state = timeline.state_at(unbound.start());
    assert!(
        state.stars().iter().all(|s| s.phase() != Phase::NoRemnant) && state.orbit().is_none(),
        "{}",
        describe(&timeline)
    );
    assert!(timeline.merger_age().is_none());
}

/// A pair of `m1` and `m2` M☉ at `period_days` and `e`, of \[Fe/H\] `fe_h`, with each star's own
/// draws (the `i`th system of a layer-A strip of cells): the pairs of ruling p11-channels' probe
/// `bound_conservative` (section 1.2).
fn drawn_pair(m1: f64, m2: f64, period_days: f64, e: f64, fe_h: f64, i: u32) -> BinaryInput {
    use crate::Seed;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};
    use crate::units::{Dex, HeliumExcess};

    let orbit = KeplerElements::from_period(
        Seconds::new(period_days * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::new(e).expect("an eccentricity in [0, 1)"),
        Orientation::new(Radians::new(0.3), Radians::new(0.1), Radians::new(0.2))
            .expect("an orientation"),
        Radians::new(1.0),
    )
    .expect("an orbit");
    let x = i32::try_from(i % 4_000).expect("a small index") - 2_000;
    let z = i32::try_from(i / 4_000).expect("a small index");
    let cell = GenCell::new(CellSize::Ly8, [x, 9, z]).expect("a cell");
    let system = SystemId::from_parts(Layer::A, cell, 0).expect("a system");
    let draws = [0, 1].map(|k| StarDraws::for_star(Seed::new(0x0b1e_5eee), BodyId::new(system, k)));
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
        orbit,
        draws,
        Years::new(1.0e9),
    )
    .expect("a pair")
}

/// `exp(ln lo + x ln(hi ÷ lo))`: log-uniform on `lo`–`hi` at `x` in [0, 1).
fn log_uniform(lo: f64, hi: f64, x: f64) -> f64 {
    crate::math::exp(crate::math::ln(lo) + x * crate::math::ln(hi / lo))
}

/// The `n` pairs of sample `kind` of ruling p11-channels' check of the pre-test against the engine
/// (section 1.2), each with the age it is run to, at \[Fe/H\] 0, −0.7 and −1.6 in turn:
///
/// - 0, enriched in the braking channel: 0.3–2.5 M☉ primaries, P 0.2–8 d (eccentric to 0.4 above
///   2 d), ages 10⁷–1.38 × 10¹⁰ years, all log-uniform;
/// - 1, the pinned sample's ranges: 0.8–40 M☉, 0.3–10⁴ d (eccentric to 0.7 above 5 d), ages
///   10⁵–1.6 × 10¹⁰ years;
/// - 2, giants near the drawn threshold: 1–40 M☉ at a periastron 0.98–1.6 times plan 08's
///   [`interacting_periastron`](crate::galaxy::displaced::binarity::interacting_periastron),
///   half of them eccentric to 0.5, run to 1.38 × 10¹⁰ years.
///
/// The companion takes a mass ratio uniform in 0.05–1, no lighter than 0.08 M☉.
fn decay_sample(kind: u8, n: u32) -> Vec<(BinaryInput, Years)> {
    let mut mix = Mix(0x0b1e_0c11 ^ u64::from(kind));
    (0..n)
        .map(|i| {
            let fe_h = [0.0, -0.7, -1.6][usize::try_from(i % 3).expect("a small index")];
            let (m1, m2, period, e, until) = match kind {
                0 => {
                    let m1 = log_uniform(0.3, 2.5, mix.next());
                    let m2 = (m1 * (0.05 + 0.95 * mix.next())).max(0.08);
                    let period = log_uniform(0.2, 8.0, mix.next());
                    let e = if period > 2.0 { 0.4 * mix.next() } else { 0.0 };
                    (m1, m2, period, e, log_uniform(1.0e7, 1.38e10, mix.next()))
                }
                1 => {
                    let m1 = log_uniform(0.8, 40.0, mix.next());
                    let m2 = (m1 * (0.05 + 0.95 * mix.next())).max(0.08);
                    let period = log_uniform(0.3, 1.0e4, mix.next());
                    let e = if period > 5.0 { 0.7 * mix.next() } else { 0.0 };
                    let until = crate::math::exp10(5.0 + 5.2 * mix.next());
                    (m1, m2, period, e, until)
                }
                _ => {
                    use crate::galaxy::displaced::binarity::interacting_periastron;
                    use crate::units::{Dex, HeliumExcess};
                    let m1 = log_uniform(1.0, 40.0, mix.next());
                    let m2 = (m1 * (0.05 + 0.95 * mix.next())).max(0.08);
                    let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
                    let threshold =
                        interacting_periastron(SolarMasses::new(m1), m2 / m1, &comp).value();
                    let factor = 0.98 + 0.62 * mix.next();
                    let e = if mix.next() < 0.5 {
                        0.0
                    } else {
                        0.5 * mix.next()
                    };
                    let a = threshold * factor / (1.0 - e);
                    let gm = crate::units::consts::GM_SUN * (m1 + m2);
                    let period = core::f64::consts::TAU * (a * a * a / gm).sqrt() / 86_400.0;
                    (m1, m2, period, e, 1.38e10)
                }
            };
            (drawn_pair(m1, m2, period, e, fe_h, i), Years::new(until))
        })
        .collect()
}

/// One sample's check of the pre-test against the engine run past it.
#[derive(Debug, Clone, Default)]
struct DecayTally {
    pairs: u32,
    /// Pairs the lobe test alone passes, the engine interacts, and the lobe test alone misses.
    drawn: u32,
    interacting: u32,
    missed_by_drawn: u32,
    /// Pairs the pre-test passes, misses, and passes that do not interact.
    passes: u32,
    missed: Vec<String>,
    passes_without_interaction: u32,
    /// Merged segments with both stars still present (finding F1): none.
    merged_with_both: u32,
    /// Pairs the pre-test passes at their age but not at 1.6 × 10¹⁰ years: none.
    narrowed: u32,
}

impl DecayTally {
    fn merge(&mut self, part: Self) {
        self.pairs += part.pairs;
        self.drawn += part.drawn;
        self.interacting += part.interacting;
        self.missed_by_drawn += part.missed_by_drawn;
        self.passes += part.passes;
        self.missed.extend(part.missed);
        self.passes_without_interaction += part.passes_without_interaction;
        self.merged_with_both += part.merged_with_both;
        self.narrowed += part.narrowed;
    }

    /// Checks `input` run to `until`: the lobe test alone and the pre-test, on the stars' own
    /// tracks built once, against the engine run past it.
    fn check(&mut self, input: &BinaryInput, until: Years) {
        use super::evolve::{
            arrival, evolve_past_the_pre_test, interacts, largest_radii_rsun, own_members,
            reaches_lobe,
        };
        let u = until.value();
        let members = own_members(input, u, None);
        let drawn =
            arrival(&members, u) < u && reaches_lobe(input, largest_radii_rsun(input, &members, u));
        let passes = interacts(input, &members, u);
        self.narrowed += u32::from(passes && !can_interact(input, Years::new(1.6e10)));
        let timeline = evolve_past_the_pre_test(input, until);
        let interaction = interaction_before(&timeline, until);
        self.pairs += 1;
        self.drawn += u32::from(drawn);
        self.passes += u32::from(passes);
        self.merged_with_both += u32::try_from(
            timeline
                .segments()
                .iter()
                .filter(|s| s.kind() == SegmentKind::Merged && !one_star_left(&timeline, s))
                .count(),
        )
        .expect("a few segments");
        if let Some(segment) = interaction {
            self.interacting += 1;
            self.missed_by_drawn += u32::from(!drawn);
            if !passes {
                let [m1, m2] = input.masses().map(SolarMasses::value);
                self.missed.push(format!(
                    "{m1:.3} + {m2:.3} M☉ at {:.3} d, e {:.2}, to {u:.3e} yr: {:?} at {:.4e} yr",
                    input.orbit().period().value() / 86_400.0,
                    input.orbit().eccentricity().value(),
                    segment.kind(),
                    segment.start().value()
                ));
            }
        } else if passes && !drawn {
            self.passes_without_interaction += 1;
        }
    }
}

/// `check` over `items` in eight shares, share k taking the items k, k + 8, …, merged in share
/// order: on threads of their own, or one after another on wasm32-wasip1, which has none.
fn in_shares<I: Sync>(items: &[I], check: impl Fn(&mut DecayTally, &I) + Sync) -> DecayTally {
    const SHARES: usize = 8;
    let share = |k: usize| {
        let mut part = DecayTally::default();
        for item in items.iter().skip(k).step_by(SHARES) {
            check(&mut part, item);
        }
        part
    };
    #[cfg(not(target_family = "wasm"))]
    let parts: Vec<DecayTally> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..SHARES)
            .map(|k| {
                let share = &share;
                scope.spawn(move || share(k))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a share's thread"))
            .collect()
    });
    #[cfg(target_family = "wasm")]
    let parts: Vec<DecayTally> = (0..SHARES).map(share).collect();
    let mut total = DecayTally::default();
    for part in parts {
        total.merge(part);
    }
    total
}

/// P11.T4.j's gate (ruling p11-channels of 2026-10-06, section 3.3): over the three samples of the
/// ruling's probe, 6,000 pairs each, every pair the pre-test passes over at its age shows no
/// stable transfer, common envelope, contact or merger before it ([`interaction_before`]) when
/// the engine runs it anyway. None is allowed. The lobe test alone missed 530, 36 and 160 of
/// them (the braking channel, the pinned sample's ranges, giants' tidal captures). The passes that
/// do not interact are recorded. No merger leaves both stars (finding F1), and every pair the
/// pre-test passes at its age it passes at 1.6 × 10¹⁰ years too.
#[test]
#[ignore = "slow: 1.8 × 10⁴ binaries run through the engine"]
fn the_decay_bound_never_passes_over_an_interaction() {
    let mut missed = 0;
    for kind in 0..3_u8 {
        let sample = decay_sample(kind, 6_000);
        let t = in_shares(&sample, |tally, (input, until)| tally.check(input, *until));
        eprintln!(
            "sample {kind}: {} pairs; the lobe test passes {}, the engine interacts in {}, the lobe \
             test misses {}; the pre-test passes {}, misses {}, passes {} that do not interact",
            t.pairs,
            t.drawn,
            t.interacting,
            t.missed_by_drawn,
            t.passes,
            t.missed.len(),
            t.passes_without_interaction,
        );
        for line in t.missed.iter().take(20) {
            eprintln!("  missed: {line}");
        }
        assert_eq!(
            t.merged_with_both, 0,
            "sample {kind}: mergers leaving both stars"
        );
        assert_eq!(
            t.narrowed, 0,
            "sample {kind}: passes lost by 1.6 × 10¹⁰ years"
        );
        assert!(t.missed_by_drawn > 0, "sample {kind} tests the bound");
        missed += t.missed.len();
    }
    assert_eq!(missed, 0, "interactions the pre-test passed over");
}

/// P11.T4.j: each term of the bound is the engine's, and goes with its [`BinaryParams`] switch.
/// The braked 1.04 + 0.45 M☉ pair at 0.69 d passes at 2 Gyr only with magnetic braking and the
/// tides that pass it to the orbit. A 0.3 + 0.3 M☉ pair at 0.2 d, below BSE's braking floor and
/// inside its lobes on the drawn orbit, passes at 13.8 Gyr by gravitational radiation alone, which
/// merges it in the engine at about 3.6 Gyr, and fails without it.
#[test]
fn the_bound_follows_the_engines_switches() {
    let with = |params: BinaryParams| move |input: BinaryInput| input.with_params(params);
    let generator = with(BinaryParams::GENERATOR);
    let no_braking = with(BinaryParams {
        magnetic_braking: false,
        ..BinaryParams::GENERATOR
    });
    let no_tides = with(BinaryParams {
        tides: false,
        ..BinaryParams::GENERATOR
    });
    let no_radiation = with(BinaryParams {
        gravitational_radiation: false,
        ..BinaryParams::GENERATOR
    });
    let braked = || pair(1.04, 0.45, 0.69, 0.0, 0.02);
    let at = Years::new(2.0e9);
    assert!(can_interact(&generator(braked()), at));
    assert!(can_interact(&no_radiation(braked()), at));
    assert!(!can_interact(&no_braking(braked()), at), "without braking");
    assert!(!can_interact(&no_tides(braked()), at), "without tides");
    let dwarfs = || pair(0.3, 0.3, 0.2, 0.0, 0.02);
    let until = Years::new(1.38e10);
    assert!(!lobe_reached(&dwarfs(), until));
    assert!(can_interact(&generator(dwarfs()), until));
    assert!(can_interact(&no_tides(dwarfs()), until));
    assert!(
        !can_interact(&no_radiation(dwarfs()), until),
        "without radiation"
    );
    let timeline = evolve(&dwarfs(), until);
    let merger = timeline.merger_age().map(Years::value);
    assert!(
        merger.is_some_and(|age| (3.0e9..4.2e9).contains(&age)),
        "{merger:?}:\n{}",
        describe(&timeline)
    );
}

/// P11.T4.j: a giant's tidal capture beyond the drawn threshold is run. A 2 + 0.2 M☉ pair at 1.2
/// times plan 08's `interacting_periastron`, circular (P ≈ 2,000 d), never reaches a lobe on its
/// drawn orbit, but the giant spins up at the orbit's expense (Darwin's instability) and engulfs
/// its companion: a common envelope on the thermally pulsing asymptotic giant branch at about
/// 1.50 Gyr, the orbit shrunk by tides before it. A run to 1.5 Gyr agrees with it
/// ([`check_any_age`]).
#[test]
fn a_giants_tidal_capture_is_run() {
    use crate::galaxy::displaced::binarity::interacting_periastron;

    let (m1, q) = (2.0, 0.1);
    let a = 1.2 * interacting_periastron(SolarMasses::new(m1), q, &Composition::SOLAR).value()
        / crate::units::consts::SOLAR_RADIUS_M;
    let input = pair(m1, m1 * q, period_days(m1, m1 * q, a), 0.0, 0.02);
    let until = Years::new(1.38e10);
    assert!(
        !lobe_reached(&input, until),
        "inside its lobe on the drawn orbit"
    );
    assert!(can_interact(&input, until));
    let timeline = evolve(&input, until);
    let stages = starts(&timeline);
    let envelope = find(&stages, 0, |kind, _| *kind != SegmentKind::Detached)
        .expect("the giant engulfs its companion");
    let age = timeline.segments()[envelope].start().value();
    assert_eq!(
        stages[envelope].0,
        SegmentKind::CommonEnvelope,
        "{}",
        describe(&timeline)
    );
    assert!((1.49e9..1.51e9).contains(&age), "at {age} yr");
    // The envelope is a segment of no length: the giant is read just before it.
    let before = timeline.state_at(Years::new(age * (1.0 - 1e-9)));
    assert_eq!(
        before.stars()[0].phase(),
        Phase::ThermallyPulsingAgb,
        "{}",
        describe(&timeline)
    );
    let axis = before
        .orbit()
        .map_or(f64::INFINITY, |o| o.semi_major_axis().value());
    let drawn = input.orbit().semi_major_axis().value();
    assert!(
        axis < drawn,
        "the orbit shrinks before the envelope: {axis} m from {drawn} m"
    );
    assert_ne!(
        check_any_age(&input, Years::new(1.5e9), until),
        EarlierRun::MissedInteraction
    );
}
