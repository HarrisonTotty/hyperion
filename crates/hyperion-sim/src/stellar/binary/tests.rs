//! Tests of the binary engine (P11.T4).

use std::sync::Arc;

use super::star::positive;
use super::*;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::rules::StellarRules;
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

/// T4.a: a close pair can interact, and a wide one cannot, by the pre-test's periastron, which its
/// lobe test reads. Since P11.T12's fix the wind's onset feeds the 10⁴-day pair whatever its
/// eccentricity, so the pair still passed over is circular at 10⁶ days, too wide for the wind.
#[test]
fn the_pre_test_reads_the_periastron() {
    let until = Years::new(1.2e10);
    assert!(can_interact(&pair(1.2, 0.8, 100.0, 0.0, 0.02), until));
    // A giant reaches about 170 R☉: 10⁴ days is out of reach circular, and in reach at e = 0.95.
    assert!(!lobe_reached(&pair(1.2, 0.8, 1.0e4, 0.0, 0.02), until));
    assert!(lobe_reached(&pair(1.2, 0.8, 1.0e4, 0.95, 0.02), until));
    assert!(can_interact(&pair(1.2, 0.8, 1.0e4, 0.95, 0.02), until));
    assert!(!can_interact(&pair(1.2, 0.8, 1.0e6, 0.0, 0.02), until));
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
/// reaches the cap on segments; no transfer step starts from a donor with nothing living (P11.T4.o);
/// no detached segment widens the orbit past its mass loss; no
/// pinned primary collapses from its hydrogen main sequence ([`check_no_main_sequence_collapse`]);
/// every swelled giant starts with an envelope around its own core ([`check_swells`], P11.T4.m).
///
/// The continuity's events are each a segment's edge: a change of kind, a change of phase, a
/// death, a strip, and the pinned primary's own main-sequence end, where a main sequence the pair
/// has prolonged steps onto the pin's own track in L and R (P11.T4.n).
#[expect(
    clippy::many_single_char_names,
    reason = "a sample's two states and their values"
)]
fn check_invariants(input: &BinaryInput, until: Years) {
    let timeline = evolve_from_living_donors(input, until);
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
    check_swells(&timeline, &what);
    check_collapses(input, &timeline, &what);
    check_no_main_sequence_collapse(input, &timeline, &what);
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

/// A swell (P11.T4.m): member `slot`, the accretor of a stable transfer, is a helium star or a
/// white dwarf at the end of one segment and a giant the binary carries at the start of the next,
/// with the same mass.
#[derive(Debug, Clone, Copy)]
struct SwellSeen {
    slot: usize,
    /// The swell's age, years.
    age: f64,
    /// The accretor's phase before it.
    from: Phase,
    /// The accretor's mass at the start of the step that swelled it and at its end, M☉.
    masses: [f64; 2],
    /// That step's length, years.
    step: f64,
    /// The accretor's core at the step's start, M☉: a helium giant's carbon–oxygen core.
    core_before: f64,
    /// Its τ at the swell, for a helium main-sequence star the binary carries.
    tau: Option<f64>,
    /// The giant at the swell.
    giant: crate::stellar::StarState,
    /// The giant's share of central helium burning done there (BSE's y).
    burnt: f64,
}

/// The swells of `timeline` ([`SwellSeen`]), read from its segments' members.
fn swells_of(timeline: &BinaryTimeline) -> Vec<SwellSeen> {
    use super::star::Member;

    let giant = |phase: Phase| {
        matches!(
            phase,
            Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb
        )
    };
    let fed = |phase: Phase| {
        matches!(
            phase,
            Phase::HeliumMainSequence
                | Phase::HeliumHertzsprungGap
                | Phase::HeliumGiantBranch
                | Phase::HeliumWhiteDwarf
                | Phase::CarbonOxygenWhiteDwarf
                | Phase::OxygenNeonWhiteDwarf
        )
    };
    let ctx = timeline.context();
    let mut out = Vec::new();
    for pair in timeline.segments().windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        let SegmentKind::StableTransfer { donor } = prev.kind() else {
            continue;
        };
        let age = next.start().value();
        if !prev.end().value().total_cmp(&age).is_eq() {
            continue;
        }
        let slot = 1 - donor.index();
        let (before, after) = (&prev.members()[slot], &next.members()[slot]);
        let Member::Shaped { mass: carried, .. } = after else {
            continue;
        };
        let (path, tau) = match before {
            Member::MainSequence {
                helium: true,
                mass,
                tau,
            } => (mass, Some(tau.last())),
            Member::Shaped { mass, .. } | Member::Remnant { mass, .. } => (mass, None),
            Member::MainSequence { .. }
            | Member::Track { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Gone => continue,
        };
        let knots = path.knots();
        let Some(&[[start, pre], [end, m]]) = knots.get(knots.len().saturating_sub(2)..) else {
            continue;
        };
        // A swell keeps the accretor's mass, where a merger at the same instant does not.
        if !carried.at(age).total_cmp(&m).is_eq() {
            continue;
        }
        let from = before.state_at(ctx, slot, end).phase();
        let Some(structure) = after.evaluate(ctx, slot, age, m, 0.0) else {
            continue;
        };
        if !fed(from) || !giant(structure.state.phase()) {
            continue;
        }
        let core_before = before
            .evaluate(ctx, slot, start, pre, tau.unwrap_or(0.0))
            .map_or(0.0, |s| s.state.core_mass().value());
        out.push(SwellSeen {
            slot,
            age,
            from,
            masses: [pre, m],
            step: end - start,
            core_before,
            tau,
            giant: structure.state,
            burnt: structure.burnt,
        });
    }
    out
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06): every swelled giant starts its segment with an
/// envelope, M − Mc > 0, and with the core the ruling asks for to 10⁻⁹: a helium star's or white
/// dwarf's mass at the start of the step that swelled it, a helium giant's carbon–oxygen core then.
fn check_swells(timeline: &BinaryTimeline, what: &impl Fn() -> String) {
    for swell in swells_of(timeline) {
        let (m, mc) = (swell.giant.mass().value(), swell.giant.core_mass().value());
        assert!(
            m > mc,
            "a swelled giant with no envelope, {swell:?}: {}",
            what()
        );
        let core = match swell.from {
            Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch => swell.core_before,
            Phase::HeliumMainSequence
            | Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf => swell.masses[0],
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::MainSequence
            | Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::PostAgb
            | Phase::NeutronStar
            | Phase::BlackHole
            | Phase::NoRemnant
            | Phase::Substellar => unreachable!("`swells_of` reads only fed phases: {swell:?}"),
        };
        assert!(
            (mc - core).abs() <= 1e-9 * core,
            "a swelled giant's core {mc} M☉ is not {core} M☉, {swell:?}: {}",
            what()
        );
    }
}

/// P11.T4.k's invariants on `timeline`, the timeline of `input` (rulings p11-supernova-pins and
/// p11-t4k-faults, 2026-10-06: every collapse is applied once, from the star's last living mass),
/// whose timeline is not capped: a pair the engine runs, or one the pre-test passes over, whose
/// collapses the closed form applies (P11.T4.l). A capped timeline never reaches its later deaths
/// (P11.T4.g's finding C2, `swell`'s core, can cycle a helium accretor to the cap; the 10³ suites
/// assert no pair reaches it), and is not checked. `what` describes the pair for the failure
/// message.
///
/// - No member goes from a living phase to a neutron star or black hole without a supernova
///   record of its component at that age (10⁻⁹ relative), but where it is the compact star its
///   companion leaves inside it (a Thorne–Żytkow object, BSE section 2.7.3).
/// - A pinned primary (design note 16) still living just before its pin has a record at its pin.
/// - No member is on a track that is a remnant from its own start (a helium star stripped past
///   its own end, finding F8) unless the engine placed it there by its death: a neutron star or
///   black hole has its record where the track is placed. A white dwarf's birth leaves no record;
///   `stripped_member_at`'s debug assertion keeps such a track from being placed alive.
fn check_collapses(input: &BinaryInput, timeline: &BinaryTimeline, what: &impl Fn() -> String) {
    use super::star::Member;

    let until = timeline.until();
    if timeline.hit_segment_cap() {
        return;
    }
    let compact = |phase: Phase| matches!(phase, Phase::NeutronStar | Phase::BlackHole);
    let recorded = |slot: usize, lo: f64, hi: f64| {
        timeline.supernovae().iter().any(|r| {
            let age = r.age().value();
            r.component().index() == slot && age >= lo * (1.0 - 1e-9) && age <= hi * (1.0 + 1e-9)
        })
    };
    let ctx = timeline.context();
    let end = until.value();
    for slot in 0..2 {
        // The member's phase just inside each segment's ends and either side of a death inside
        // it, in age order: every change of phase lies between two of them.
        let mut probes: Vec<(f64, Phase, Phase)> = Vec::new();
        for segment in timeline.segments() {
            let (start, stop) = (segment.start().value(), segment.end().value().min(end));
            let members = segment.members();
            let at = |age: f64| {
                (
                    members[slot].state_at(ctx, slot, age).phase(),
                    members[1 - slot].state_at(ctx, 1 - slot, age).phase(),
                )
            };
            // A death at the segment's end, to rounding, is the boundary's: the engine's age there
            // puts the track at its death or an ulp past it.
            let death = members[slot]
                .track()
                .and_then(|(track, offset)| track.lifetime().map(|life| offset + life.value()))
                .filter(|&death| death > start);
            let at_end = |death: f64| (death - stop).abs() <= 1e-9 * stop.max(1.0);
            let mut ages = vec![start];
            if let Some(death) = death.filter(|&death| death < stop && !at_end(death)) {
                ages.extend([super::detached::last_living(death), death]);
            }
            if stop > start {
                let last = stop - 1e-12 * stop.max(1.0);
                ages.push(match death.filter(|&death| at_end(death)) {
                    Some(death) => last.min(super::detached::last_living(death)),
                    None => last,
                });
            }
            for age in ages {
                let (own, other) = at(age);
                probes.push((age, own, other));
            }
        }
        for pair in probes.windows(2) {
            let [(a, before, other_before), (b, after, other_after)] = [pair[0], pair[1]];
            let swallowed = compact(other_before) && other_after == Phase::NoRemnant;
            if before.is_living() && compact(after) && !swallowed {
                assert!(
                    recorded(slot, a, b),
                    "member {slot} goes from {before:?} to {after:?} between {a} and {b} yr with no \
                     supernova record: {}",
                    what()
                );
            }
        }
    }
    if let Some(pin) = super::evolve::pinned_death_age_years(input).filter(|&pin| pin < end) {
        let before = timeline.state_at(Years::new(super::detached::last_living(pin)));
        if before.stars()[0].phase().is_living() {
            assert!(
                recorded(0, pin, pin),
                "the pinned primary, {:?} just before its pin at {pin} yr, has no record there: {}",
                before.stars()[0].phase(),
                what()
            );
        }
    }
    for segment in timeline.segments() {
        for (slot, member) in segment.members().iter().enumerate() {
            let Member::Track { track, offset } = member else {
                continue;
            };
            if !track.is_remnant_from_its_start() {
                continue;
            }
            let remnant = track.remnant().map(|r| r.kind());
            if matches!(
                remnant,
                Some(
                    crate::stellar::remnant::RemnantKind::NeutronStar
                        | crate::stellar::remnant::RemnantKind::BlackHole
                )
            ) {
                assert!(
                    recorded(slot, *offset, *offset),
                    "member {slot} is a track dead from its start at {offset} yr, a {remnant:?}, \
                     with no record there: {}",
                    what()
                );
            }
        }
    }
}

/// P11.T4.n's invariant on `timeline`, the pair of `input` (decision-observed-class-findings §B):
/// no pinned primary collapses from its hydrogen main sequence, a hot, non-degenerate core. Where
/// its pin is recorded, the primary just before it is a supergiant or a naked helium star, or held
/// at one (`Member::Frozen`). `what` describes the pair for the failure message.
fn check_no_main_sequence_collapse(
    input: &BinaryInput,
    timeline: &BinaryTimeline,
    what: &impl Fn() -> String,
) {
    let Some(pin) =
        super::evolve::pinned_death_age_years(input).filter(|&pin| pin < timeline.until().value())
    else {
        return;
    };
    let recorded = timeline.supernovae().iter().any(|r| {
        r.component() == Component::Primary && (r.age().value() - pin).abs() <= 1e-9 * pin
    });
    if !recorded {
        return;
    }
    let before = timeline.state_at(Years::new(super::detached::last_living(pin)));
    let star = before.stars()[0];
    assert_ne!(
        star.phase(),
        Phase::MainSequence,
        "the pinned primary collapses at {pin} yr from its main sequence, {} M☉ at log L {}: {}",
        star.mass().value(),
        crate::math::log10(star.luminosity().value()),
        what()
    );
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
    // With two wide pairs whose collapses the closed form applies (P11.T4.l).
    let no_kicks = BinaryParams {
        natal_kicks: false,
        ..BinaryParams::GENERATOR
    };
    let inputs: Vec<BinaryInput> = sample_pairs(12, 0x5eed_0003)
        .into_iter()
        .chain([
            pair(40.0, 10.0, 1.0e5, 0.0, 0.02).with_params(no_kicks),
            pair(15.0, 12.0, 1.0e5, 0.4, 0.02).with_params(no_kicks),
        ])
        .collect();
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
/// up, and kept so that no optimisation moves a result): each line is one pair's [`digest`]. Every
/// timeline also keeps P11.T4.k's invariants ([`check_collapses`]) and P11.T4.n's
/// ([`check_no_main_sequence_collapse`]), and P11.T4.o's: no transfer step starts from a dead
/// donor, and no accepted step moves more than 1% of its donor ([`transfer_shares`]), but for
/// pair 0171's one step at 693.35 Myr, 4.3% after its eight halvings (the first step of a helium
/// white dwarf's transfer; plan 11's Risks, "P11.T4.o as built").
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
        let timeline = evolve_from_living_donors(&input, until);
        let what = || format!("pinned pair {i:04}: {}", describe(&timeline));
        check_collapses(&input, &timeline, &what);
        check_no_main_sequence_collapse(&input, &timeline, &what);
        for (age, share) in transfer_shares(&timeline) {
            // Pair 0171's first step from its new helium white dwarf onto its carbon–oxygen one, a
            // ten-thousandth of the dwarf's Kelvin–Helmholtz time, keeps 4.3% after eight halvings.
            let most = if i == 171 && (age / 693.353_165e6 - 1.0).abs() < 1e-8 {
                0.043
            } else {
                0.01
            };
            assert!(
                share <= most,
                "{share} of the donor at {age} yr: {}",
                what()
            );
        }
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
/// to the end of its main sequence. The rejuvenated accretor leaves it at 1,035.446 Myr, once, at
/// the first step aimed at that end over which it ages (P11.T4.o, finding F18), where the steps
/// used to close in on its receding end until the segment cap. (1,052.889 Myr before T4.o: F18
/// alone ends it 0.50 Myr sooner, at that step rather than after more of them, a segment each, and
/// F17 alone 17.2 Myr sooner, its accretion taken in steps of at most 1% of the donor; 1,052.655
/// Myr before P11.T4.i, which starts the engine at the primary's arrival, 7 Myr before the
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
        (left - 1.035_446e9).abs() <= 1.0e3,
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

/// That `early` and `later`, the same pair run to `until` and past it, hold the same supernova
/// records before `until`, bit for bit (P11.T4.l's build-age contract covers the records as well as
/// the states: a record is a collapse's recoil, kick and velocities, which no state shows).
fn assert_same_records(early: &BinaryTimeline, later: &BinaryTimeline, until: Years) {
    let before = |timeline: &BinaryTimeline| -> Vec<SupernovaRecord> {
        timeline
            .supernovae()
            .iter()
            .filter(|record| record.age() < until)
            .copied()
            .collect()
    };
    assert_eq!(
        before(early),
        before(later),
        "the records before {until:?}:\n{}\nagainst\n{}",
        describe(early),
        describe(later)
    );
}

/// How a pair run to an earlier age compares with the same pair run to a later one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EarlierRun {
    /// The engine ran the pair both times: the same history to the earlier age.
    Same,
    /// The pre-test passed over the pair at the earlier age (two single stars on the drawn orbit),
    /// and the later run met no interaction before it.
    PassedOver,
    /// The pre-test passed over the pair at both ages, and a star collapsed before the earlier
    /// one (P11.T4.l): the same history to the earlier age, bit for bit.
    PassedOverThroughCollapses,
    /// The pre-test passed over the pair at the earlier age, but the later run holds an
    /// interaction before it ([`interaction_before`]): the pre-test failed to bound the decay.
    MissedInteraction,
}

/// Plan 11's consistency to any age (P11's build-age dependence, 2026-10-05), for `input` run to
/// `until` and to `later`. A pair that [`can_interact`] by `until`, or a star of which collapses by
/// then (P11.T10's fix, [`engine_runs`](super::evolve::engine_runs)), has the same history to `until` as the later run, bit for bit ([`assert_same_history`]), and
/// the same supernova records before it ([`assert_same_records`]), whether the engine runs it or,
/// under P11.T12's fix, it is wind-fed in closed form. One it passes over is a detached segment of
/// its stars' own forms on the drawn orbit to its first collapse (P11.T4.l). If the pre-test
/// passes it over at `later` too, the two runs are the same to `until`, records included, bit for
/// bit, or, where the later run's engine takes the pair over after a collapse, to that collapse,
/// its record included; otherwise each star on its own track is the later run's to the later
/// run's first event (a star below 0.1 M☉ on the cooling fits takes its companion's wind in the
/// later run).
fn check_any_age(input: &BinaryInput, until: Years, later: Years) -> EarlierRun {
    use super::star::Member;

    let early = evolve_from_living_donors(input, until);
    let late = evolve_from_living_donors(input, later);
    if super::evolve::engine_runs(input, until) || can_interact(input, until) {
        assert!(!early.hit_segment_cap(), "{}", describe(&early));
        assert_same_history(&early, &late, until);
        assert_same_records(&early, &late, until);
        return EarlierRun::Same;
    }
    let first_collapse = early
        .supernovae()
        .first()
        .map_or(until, SupernovaRecord::age);
    let first = &early.segments()[0];
    assert_eq!(first.kind(), SegmentKind::Detached, "{}", describe(&early));
    assert_eq!(first.end(), first_collapse, "{}", describe(&early));
    if !(super::evolve::engine_runs(input, later) || can_interact(input, later)) {
        // The same history to `until`, bit for bit, unless the pre-test asked again after a
        // collapse passes the pair at `later` and not at `until`: then the later run's engine
        // evolves the orbit from the collapse where the earlier keeps it fixed, and only the
        // history up to the collapse is the same, with no interaction before `until`.
        match (taken_over_at(&early), taken_over_at(&late)) {
            (None, Some(collapse)) if collapse < until => {
                assert_same_history(&early, &late, Years::new(collapse.value() * (1.0 - 1e-12)));
                assert_same_records(&early, &late, Years::new(collapse.value() * (1.0 + 1e-12)));
                assert!(
                    interaction_after_a_collapse(&late, until).is_none(),
                    "{}",
                    describe(&late)
                );
            }
            (None | Some(_), _) => {
                assert_same_history(&early, &late, until);
                assert_same_records(&early, &late, until);
            }
        }
        return if early.supernovae().is_empty() {
            EarlierRun::PassedOver
        } else {
            EarlierRun::PassedOverThroughCollapses
        };
    }
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

/// Where the engine took over a pair the pre-test passed over, after a collapse (P11.T4.l): the
/// start of `timeline`'s first segment whose orbit is the engine's, not fixed, if any (the closed
/// form's orbits are all fixed).
fn taken_over_at(timeline: &BinaryTimeline) -> Option<Years> {
    timeline
        .segments()
        .iter()
        .find(|s| s.paths().0.is_some_and(|o| o.fixed.is_none()))
        .map(Segment::start)
}

/// The first segment of `timeline` that is an interaction starting after its first supernova and
/// before `until`: stable transfer, a common envelope, contact, or a merger that leaves one star
/// (P11.T4.l's pre-test asked again after a collapse).
fn interaction_after_a_collapse(timeline: &BinaryTimeline, until: Years) -> Option<&Segment> {
    let first = timeline.supernovae().first()?.age();
    timeline.segments().iter().find(|s| {
        let interacts = match s.kind() {
            SegmentKind::Detached | SegmentKind::Disrupted { .. } => false,
            SegmentKind::Merged => one_star_left(timeline, s),
            SegmentKind::StableTransfer { .. }
            | SegmentKind::CommonEnvelope
            | SegmentKind::Contact => true,
        };
        interacts && s.start() >= first && s.start() < until
    })
}

/// The first segment of `timeline` that is an interaction starting before `until` and before the
/// pair's first supernova (ruling p11-channels of 2026-10-06, section 1.2): stable transfer, a
/// common envelope, contact, or a merger that leaves one star. A detached or disrupted segment is
/// none, and a supernova of a pair the pre-test passes over is not the pre-test's to foresee: the
/// closed form applies it, and asks the pre-test again of the orbit it leaves (P11.T4.l).
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
/// over (without and with a collapse before the age) and missed interactions, at the first ages,
/// between the arrivals, and for a wide massive pair ([`wide_massive_pair`]) beside every tenth,
/// whose collapses the pre-test leaves to the closed form (P11.T4.l). The ages between arrivals
/// and the wide pairs are drawn on streams of their own, so that the pairs and their first ages
/// are the sample's as before.
fn any_age_over(n: u32, seed: u64) -> [[u32; 4]; 3] {
    any_age_over_under(n, seed, None, 1.5e10)
}

/// [`any_age_over`] with each pair under `rules` where given, and otherwise the version's, and
/// each run to `later` years in place of 1.5 × 10¹⁰.
fn any_age_over_under(n: u32, seed: u64, rules: Option<StellarRules>, later: f64) -> [[u32; 4]; 3] {
    let mut mix = Mix(seed);
    let mut between = Mix(seed ^ 0x0a77_1fa1_0000_0000);
    let mut wide = Mix(seed ^ 0x0a77_1fa1_0000_0001);
    let mut counts = [[0_u32; 4]; 3];
    for i in 0..n {
        let input = pinned_pair(&mut mix, i);
        let input = match rules {
            Some(rules) => input.with_rules(rules),
            None => input,
        };
        let until =
            crate::math::exp(crate::math::ln(1.0e5) + mix.next() * crate::math::ln(1.2e10 / 1.0e5));
        let mut runs = vec![(0, until, input.clone())];
        if let Some((first, late)) = first_and_late(&input) {
            runs.push((1, first + (late - first) * between.next(), input));
        }
        if i % 10 == 0 {
            let (pair, age) = wide_massive_pair(&mut wide, i);
            let pair = match rules {
                Some(rules) => pair.with_rules(rules),
                None => pair,
            };
            runs.push((2, age, pair));
        }
        for (kind, until, input) in runs {
            let outcome = check_any_age(&input, Years::new(until), Years::new(later));
            let slot = match outcome {
                EarlierRun::Same => 0,
                EarlierRun::PassedOver => 1,
                EarlierRun::PassedOverThroughCollapses => 2,
                EarlierRun::MissedInteraction => 3,
            };
            counts[kind][slot] += 1;
        }
    }
    counts
}

/// A wide massive pair for the build-age suites (P11.T4.l), the `i`th of its stream: primary
/// log-uniform in 8–40 M☉, mass ratio uniform in 0.05–1 (no lighter than 0.08 M☉), period
/// log-uniform in 10⁵–10⁷ d, e uniform in 0–0.7, one of three metallicities and each star's own
/// draws, with an age log-uniform in 5 × 10⁶–1.2 × 10¹⁰ years to run it to: mostly passed over,
/// and mostly past its primary's collapse.
fn wide_massive_pair(mix: &mut Mix, i: u32) -> (BinaryInput, f64) {
    use crate::Seed;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};

    let m1 = log_uniform(8.0, 40.0, mix.next());
    let m2 = (m1 * (0.05 + 0.95 * mix.next())).max(0.08);
    let period = log_uniform(1.0e5, 1.0e7, mix.next());
    let e = 0.7 * mix.next();
    let z = [0.02, 0.004, 0.0005][usize::try_from(i % 3).expect("a small index")];
    let until = log_uniform(5.0e6, 1.2e10, mix.next());
    let base = pair(m1, m2, period, e, z);
    let x = i32::try_from(i).expect("a small index") - 500;
    let cell = GenCell::new(CellSize::Ly8, [x, 8, 0]).expect("a cell");
    let system = SystemId::from_parts(Layer::A, cell, 0).expect("a system");
    let draws = [0, 1].map(|k| StarDraws::for_star(Seed::new(0x0b1e_5eef), BodyId::new(system, k)));
    let input = BinaryInput::new(
        base.masses()[0],
        base.masses()[1],
        *base.composition(),
        *base.orbit(),
        draws,
        base.age_at_epoch(),
    )
    .expect("a pair");
    (input, until)
}

/// The engine's history does not depend on the age it is run to (P11's build-age dependence,
/// 2026-10-05): [`check_any_age`] over 60 pairs, between their stars' arrivals (P11.T4.i), and
/// over six wide massive pairs whose collapses the closed form applies (P11.T4.l). No pair the
/// pre-test passes over interacts before its age in the later run (P11.T4.j: the pre-test bounds
/// the decay that magnetic braking, tides and gravitational radiation make).
#[test]
fn a_timeline_is_the_same_whatever_age_it_is_run_to() {
    // The runs between the arrivals are checked as the others are; none of them passes the
    // pre-test in this sample (`a_late_companion_meets_its_primarys_envelope` compares such runs
    // directly).
    let [
        [same, passed, collapsed, missed],
        _,
        [_, _, wide_collapsed, wide_missed],
    ] = any_age_over(60, 0x0b1e_00a9);
    assert!(same > 10 && passed > 10, "{same} run, {passed} passed over");
    assert!(
        collapsed + wide_collapsed > 0,
        "{collapsed} + {wide_collapsed} passed over through collapses"
    );
    assert_eq!(missed + wide_missed, 0, "interactions missed");
}

/// [`a_timeline_is_the_same_whatever_age_it_is_run_to`] with each pair run on to the universe's
/// age, the horizon to which P11.T12's fix (`wind_accretion.rs`), which feeds a pair the lobe and
/// decay tests pass over its partner's wind in closed form, holds a pair the engine runs at no
/// age: such a pair has the same history to the earlier age as the later run, and one passed over
/// at the earlier age keeps its stars' own forms to the later run's first event. The brief's
/// wind-fed halo pairs have the same history to each earlier age.
#[test]
fn a_timeline_is_the_same_whatever_age_it_is_run_to_under_the_wind_accretion_arm() {
    use crate::planetary::context::UNIVERSE_AGE;
    let later = UNIVERSE_AGE.value();
    let [[same, passed, collapsed, missed], _, [_, _, _, wide_missed]] =
        any_age_over_under(60, 0x0b1e_00a9, None, later);
    println!("{same} same, {passed} passed over and {collapsed} through collapses");
    assert_eq!(missed + wide_missed, 0, "interactions missed");
    // The sample's pairs are rarely wind-fed, so the brief's halo pairs are checked too: 0.85 and
    // 0.80 M☉ at Z = 0.0003, at 10³–10⁴ days, run to ages from 11.5 to 13 Gyr.
    let mut fed = 0;
    for days in [1.0e3, 3.0e3, 1.0e4] {
        let input = pair(0.85, 0.80, days, 0.0, 0.0003);
        for until in [1.15e10, 1.2e10, 1.3e10] {
            let until = Years::new(until);
            if can_interact(&input, until) {
                assert_eq!(
                    check_any_age(&input, until, Years::new(later)),
                    EarlierRun::Same,
                    "{days} d at {until:?}"
                );
                fed += 1;
            }
        }
    }
    assert!(fed >= 6, "{fed} halo runs wind-fed");
}

/// A star's own sudden death runs its pair, the primary's at its pin where design note 16 pins
/// it, and a white dwarf's birth does not: the earliest of the two stars' collapses, on their own
/// tracks, whichever member it is.
#[test]
fn a_pair_s_first_sudden_death_is_either_star_s_collapse_and_no_white_dwarf_s_birth() {
    use super::evolve::first_sudden_death;
    use super::star::Member;
    use crate::stellar::sse::Track;

    let comp = Composition::SOLAR;
    let member = |m: f64| Member::Track {
        track: Arc::new(Track::full(
            SolarMasses::new(m),
            &comp,
            &StarDraws::median(),
        )),
        offset: 0.0,
    };
    let death = |m: f64| {
        let track = Track::full(SolarMasses::new(m), &comp, &StarDraws::median());
        let death = track.death().expect("a full track dies");
        (death.age().value(), death.kind().is_sudden())
    };
    let ((at_15, sudden_15), (at_3, sudden_3)) = (death(15.0), death(3.0));
    assert!(sudden_15 && !sudden_3);
    assert_eq!(
        first_sudden_death(&[member(15.0), member(3.0)], None),
        Some(at_15)
    );
    // The companion's own collapse, beside a star whose death is a white dwarf's birth.
    assert_eq!(
        first_sudden_death(&[member(3.0), member(15.0)], None),
        Some(at_15)
    );
    assert_eq!(first_sudden_death(&[member(3.0), member(1.0)], None), None);
    assert!(at_3 > 0.0);
    // The pin's age stands for the primary's own.
    assert_eq!(
        first_sudden_death(&[member(15.0), member(3.0)], Some(0.5 * at_15)),
        Some(0.5 * at_15)
    );
}

/// [`a_timeline_is_the_same_whatever_age_it_is_run_to`] over 10³ pairs, with the ages between
/// arrivals (P11.T4.i) and a hundred wide massive pairs (P11.T4.l). No interaction is missed
/// (P11.T4.j): before the decay-aware pre-test, 4 of the first ages and 9 of the 721 between the
/// arrivals were pairs under 1.4 d that braking or tides bring into contact or a merger before the
/// age, which the drawn-orbit test passed over.
#[test]
#[ignore = "slow: about 3.6 × 10³ binaries run through the engine"]
fn a_thousand_timelines_are_the_same_whatever_age_they_are_run_to() {
    let [
        [same, passed, collapsed, missed],
        [same_between, passed_between, _, missed_between],
        [same_wide, passed_wide, collapsed_wide, missed_wide],
    ] = any_age_over(1_000, 0x0b1e_00aa);
    eprintln!(
        "{same} run at both ages, {passed} passed over, {collapsed} through collapses, {missed} \
         interactions missed; between arrivals {same_between} run, {passed_between} passed over, \
         {missed_between} missed; wide massive pairs {same_wide} run, {passed_wide} passed over, \
         {collapsed_wide} through collapses, {missed_wide} missed"
    );
    assert_eq!(missed, 0, "{missed} interactions missed at the first ages");
    assert_eq!(
        missed_between, 0,
        "{missed_between} interactions missed between the arrivals"
    );
    assert_eq!(missed_wide, 0, "{missed_wide} missed in the wide pairs");
    assert!(same_between > 0, "{same_between} run between arrivals");
    assert!(
        collapsed_wide > 10,
        "{collapsed_wide} wide pairs through collapses"
    );
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

/// The primary's death as plan 06 gives it, for a pair of median draws.
fn plan_06_death(input: &BinaryInput) -> Years {
    plan_06_star(input)
        .death()
        .expect("a massive star dies")
        .age()
}

/// The orbit of the late companion's envelope test below, R☉: about half the drawn-orbit reach of a
/// 15 M☉ red supergiant's largest radius (1,420 R☉) beside a 1 M☉ companion.
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

/// The orbit after a kick-free collapse on a circular orbit of `a` (R☉), the pair's mass going
/// from `before` to `after` M☉ at once (BSE appendix A1; Blaauw 1961): a′ = a M′ ÷ (2M′ − M) and
/// e′ = M ÷ M′ − 1, or `None` where more than half the mass is lost and the pair unbinds.
fn blaauw_orbit_rsun(a: f64, before: f64, after: f64) -> Option<(f64, f64)> {
    let denominator = 2.0 * after - before;
    positive(denominator).then(|| (a * after / denominator, before / after - 1.0))
}

/// The semi-major axis (R☉) and eccentricity of `state`'s orbit, if it has one.
fn axis_rsun_and_e(state: &BinaryState) -> Option<(f64, f64)> {
    state.orbit().map(|o| {
        (
            o.semi_major_axis().value() / crate::units::consts::SOLAR_RADIUS_M,
            o.eccentricity().value(),
        )
    })
}

/// A kick's speed, m s⁻¹, zero for none.
fn kick_speed_m_s(record: &SupernovaRecord) -> f64 {
    record.kick().map_or(0.0, |k| k.speed().value())
}

/// The primary's supernova record in `timeline`.
fn primary_record(timeline: &BinaryTimeline) -> SupernovaRecord {
    *timeline
        .supernovae()
        .iter()
        .find(|s| s.component() == Component::Primary)
        .unwrap_or_else(|| panic!("the primary's collapse is recorded: {}", describe(timeline)))
}

/// P11.T4.k, finding F7 (ruling p11-supernova-pins, 2026-10-06): a 17 + 12 M☉ pair at 3,300 R☉,
/// circular at \[Fe/H\] 0 with the median draws, passes the pre-test but never interacts. Its
/// primary is still on its own track at its pinned death (12.06 Myr), where the pin wins the tie
/// with the track's own death and the track already shows its remnant. The collapse takes the
/// living star's 9.2378 M☉ (the death's progenitor mass) to plan 06's 1.4955 M☉ neutron star, and
/// the primary goes on as its own track's remnant, bit for bit. Without natal kicks the orbit after
/// is BSE appendix A1's: 4,396 → 10,388 R☉ and e 0 → 0.5768, bound. With them the record's
/// `bound` is whether an orbit follows: plan 06's 518 km/s draw unbinds the pair, `Disrupted` by
/// the primary. Before T4.k the pin read the remnant as nothing living: no record, and the pair
/// kept its orbit through the collapse.
#[test]
fn an_untouched_pinned_primary_collapses_on_its_orbit() {
    use crate::stellar::sse::Track;

    let (m1, m2, a) = (17.0, 12.0, 3_300.0);
    let until = Years::new(5.0e7);
    let own = Track::full(
        SolarMasses::new(m1),
        &Composition::SOLAR,
        &StarDraws::median(),
    );
    for kicks in [false, true] {
        let input = pair(m1, m2, period_days(m1, m2, a), 0.0, 0.02).with_params(BinaryParams {
            natal_kicks: kicks,
            ..BinaryParams::GENERATOR
        });
        assert!(can_interact(&input, until), "the pre-test passes the pair");
        let model = plan_06_star(&input);
        let death = model.death().expect("a massive star dies");
        let timeline = evolve(&input, until);
        let record = primary_record(&timeline);
        let d = death.age().value();
        assert!(
            (record.age().value() - d).abs() <= 1e-9 * d,
            "{:?} against plan 06's {d} yr",
            record.age()
        );
        assert!((d - 12.06e6).abs() < 1e4, "{d}");
        assert_eq!(Some(record.remnant()), model.remnant());
        assert!(
            (record.remnant().mass().value() - 1.4955).abs() < 1e-4,
            "{:?}",
            record.remnant()
        );
        let progenitor = death.progenitor();
        let living = progenitor.helium_core_mass().value() + progenitor.envelope_mass().value();
        assert!((living - 9.2378).abs() < 1e-4, "{living}");
        let before = timeline.state_at(Years::new(super::detached::last_living(d)));
        let after = timeline.state_at(death.age());
        assert!(
            (before.stars()[0].mass().value() - living).abs() < 1e-8,
            "the living mass before: {:?}",
            before.stars()[0]
        );
        assert!(
            timeline.segments().iter().all(|s| matches!(
                s.kind(),
                SegmentKind::Detached | SegmentKind::Disrupted { .. }
            )),
            "never interacts: {}",
            describe(&timeline)
        );
        assert_eq!(
            after.orbit().is_some(),
            record.bound(),
            "{}",
            describe(&timeline)
        );
        let (a0, e0) = axis_rsun_and_e(&before).expect("bound before the collapse");
        assert!(e0 < 1e-9, "circular before: e {e0}");
        if kicks {
            assert_eq!(record.kick(), model.natal_kick());
            assert!(kick_speed_m_s(&record) > 5.0e5, "{:?}", record.kick());
            assert!(!record.bound(), "{}", describe(&timeline));
            assert_eq!(
                after.kind(),
                SegmentKind::Disrupted {
                    by: Component::Primary
                }
            );
        } else {
            assert_eq!(record.kick(), None);
            let total = living + before.stars()[1].mass().value();
            let total_after = record.remnant().mass().value() + after.stars()[1].mass().value();
            let (a1, e1) = blaauw_orbit_rsun(a0, total, total_after).expect("less than half lost");
            let (a, e) = axis_rsun_and_e(&after).expect("bound after the collapse");
            assert!(record.bound(), "{}", describe(&timeline));
            assert!((a / a1 - 1.0).abs() < 1e-6, "a {a} R☉ against A1's {a1}");
            assert!((e - e1).abs() < 1e-6, "e {e} against A1's {e1}");
            assert!(
                (a0 - 4_396.0).abs() < 1.0 && (a - 10_388.0).abs() < 1.0,
                "{a0} → {a}"
            );
            assert!((e - 0.5768).abs() < 1e-4, "{e}");
        }
        for age in [d, d + 1.0e6, until.value()] {
            assert_eq!(
                timeline.state_at(Years::new(age)).stars()[0],
                own.state_at(Years::new(age)),
                "the primary is its own track's remnant at {age} yr"
            );
        }
    }
}

/// P11.T4.k, F7: a 25 + 20 M☉ pair at 3,500 R☉ ([Fe/H] 0, median draws) whose primary its own
/// winds strip to an 8.32 M☉ helium star, untouched by its companion, collapses directly at plan
/// 06's death (7.84 Myr) to an 8.32 M☉ black hole: no kick and no mass lost, so the orbit is kept
/// to 10⁻⁹, now with the record that was missing before T4.k.
#[test]
fn a_wind_stripped_primary_collapses_in_place() {
    let input = pair(25.0, 20.0, period_days(25.0, 20.0, 3_500.0), 0.0, 0.02);
    let model = plan_06_star(&input);
    let death = model.death().expect("a massive star dies");
    let timeline = evolve(&input, Years::new(5.0e7));
    let record = primary_record(&timeline);
    let d = death.age().value();
    assert!(
        (record.age().value() - d).abs() <= 1e-9 * d,
        "{:?} against plan 06's {d} yr",
        record.age()
    );
    assert!((d - 7.84e6).abs() < 1e4, "{d}");
    assert_eq!(Some(record.remnant()), model.remnant());
    assert_eq!(
        record.remnant().kind(),
        crate::stellar::remnant::RemnantKind::BlackHole
    );
    let before = timeline.state_at(Years::new(super::detached::last_living(d)));
    let after = timeline.state_at(death.age());
    let held = before.stars()[0];
    assert!(
        matches!(
            held.phase(),
            Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
        ),
        "{held:?}"
    );
    let bh = record.remnant().mass().value();
    assert!((bh - 8.32).abs() < 5e-3, "{bh}");
    assert!((held.mass().value() / bh - 1.0).abs() < 1e-9, "{held:?}");
    assert!(kick_speed_m_s(&record) <= 0.0, "{:?}", record.kick());
    assert!(record.bound(), "{}", describe(&timeline));
    let ((a0, e0), (a1, e1)) = (
        axis_rsun_and_e(&before).expect("bound before"),
        axis_rsun_and_e(&after).expect("bound after"),
    );
    assert!((a1 / a0 - 1.0).abs() < 1e-9, "a {a0} → {a1} R☉");
    assert!((e1 - e0).abs() < 1e-9, "e {e0} → {e1}");
}

/// The late companion's supernova orbit below, R☉: 0.9 of the drawn reach of a 20 M☉ red
/// supergiant's largest radius (1,511 R☉ at \[Fe/H\] 0, median draws) beside a 1 M☉ companion, the
/// periastron at which the giant would fill its Roche lobe: about 2,160 R☉.
fn late_supernova_orbit_rsun() -> f64 {
    let giant = crate::stellar::sse::Track::full(
        SolarMasses::new(20.0),
        &Composition::SOLAR,
        &StarDraws::median(),
    );
    let death = giant.lifetime().expect("built in full");
    0.9 * giant.max_radius_until(death).value() / lobe_fraction(20.0, 1.0)
}

/// P11.T4.i (ruling p11-channels, 2026-10-06) and P11.T4.k: a 1 M☉ companion of a 20 M☉ primary
/// (dead at 9.87 Myr, the companion arriving at 37 Myr), at 0.9 of the drawn reach of the
/// primary's giant (about 2,160 R☉), passes the pre-test, so the pair is run from the first
/// arrival. The giant's wind widens the orbit faster than the giant grows, so it never touches
/// its companion, and its collapse on its own track at plan 06's death is applied to the pair
/// (BSE appendix A1): recorded, and the pair bound or not as the state after it shows. Without a
/// kick that is Blaauw's criterion on the circular orbit, bound while less than half the pair's
/// mass is lost, here a 6.81 M☉ black hole from an 8.3 M☉ supergiant, the orbit widening from about
/// 4,670 to 5,750 R☉ and e from 0 to 0.187. Before T4.i the engine started after the death; before
/// T4.k the collapse, read at the pin as the track's remnant, left no record and the orbit at about
/// 4,670 R☉ (finding F7), and T4.i took the test at 1,200 R☉, an orbit the giant fills.
#[test]
fn a_late_companions_supernova_is_applied() {
    let (m1, m2) = (20.0, 1.0);
    let orbit = late_supernova_orbit_rsun();
    assert!((2_100.0..2_200.0).contains(&orbit), "{orbit} R☉");
    for kicks in [false, true] {
        let params = BinaryParams {
            natal_kicks: kicks,
            ..BinaryParams::GENERATOR
        };
        let input = pair(m1, m2, period_days(m1, m2, orbit), 0.0, 0.02).with_params(params);
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
        let record = primary_record(&timeline);
        assert!(
            (record.age().value() - death.value()).abs() <= 1e-9 * death.value(),
            "{:?} against plan 06's {death:?}",
            record.age()
        );
        let before = timeline.state_at(Years::new(super::detached::last_living(death.value())));
        let after = timeline.state_at(death);
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
            let (a0, e0) = axis_rsun_and_e(&before).expect("bound before");
            assert!(e0 < 1e-9, "circular before the collapse: e {e0}");
            let total = before.total_mass().value();
            let total_after = record.remnant().mass().value() + before.stars()[1].mass().value();
            let lost = total - total_after;
            assert!(lost > 0.0, "the collapse loses mass: {lost} M☉");
            assert_eq!(
                record.bound(),
                lost < 0.5 * total,
                "{lost} of {total} M☉ lost: {}",
                describe(&timeline)
            );
            let (a1, e1) = blaauw_orbit_rsun(a0, total, total_after).expect("less than half lost");
            let (a, e) = axis_rsun_and_e(&after).expect("bound after");
            assert!((a / a1 - 1.0).abs() < 1e-6, "a {a} R☉ against A1's {a1}");
            assert!((e - e1).abs() < 1e-6, "e {e} against A1's {e1}");
        }
    }
}

/// P11.T4.k, finding F8 (ruling p11-supernova-pins, 2026-10-06): 20 + 1 M☉ at 1,680 R☉ ([Fe/H] 0,
/// median draws). The supergiant's envelope engulfs the companion at 9.8665 Myr, 3.6 kyr before
/// plan 06's death at 9.8701 Myr, when the helium star of its core (HPT section 6,
/// `HeliumStar::from_early_agb`) has no life left: its own clock is past its end. The primary is
/// held at that helium star's last living state, a 6.81 M☉ helium Hertzsprung-gap or giant-branch
/// star, until its pin, where it collapses to plan 06's 6.81 M☉ black hole, not the helium track's
/// 3.93 M☉ one: direct, no kick and no mass lost, so the post-envelope orbit of 108.04 R☉ stays to
/// 10⁻⁶. Before T4.k it was placed as the helium track's 3.93 M☉ black hole from the envelope on,
/// and never collapsed.
#[test]
fn a_core_stripped_past_its_end_is_held_until_its_pin() {
    let input = pair(20.0, 1.0, period_days(20.0, 1.0, 1_680.0), 0.0, 0.02);
    let model = plan_06_star(&input);
    let death = model.death().expect("a massive star dies").age().value();
    let timeline = evolve(&input, Years::new(5.0e7));
    let envelope = timeline
        .segments()
        .iter()
        .find(|s| s.kind() == SegmentKind::CommonEnvelope)
        .unwrap_or_else(|| panic!("a common envelope: {}", describe(&timeline)))
        .start()
        .value();
    assert!(
        (envelope - 9.8665e6).abs() < 1.0e2 && envelope < death,
        "the envelope at {envelope} yr, the pin at {death}: {}",
        describe(&timeline)
    );
    let remnant = model.remnant().expect("a remnant");
    for k in 0..8_u32 {
        let age = envelope + (death - envelope) * f64::from(k) / 8.0;
        let held = timeline.state_at(Years::new(age)).stars()[0];
        assert!(
            matches!(
                held.phase(),
                Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
            ),
            "at {age} yr: {held:?}"
        );
        assert!(
            (held.mass().value() / remnant.mass().value() - 1.0).abs() < 1e-6,
            "at {age} yr: {held:?}"
        );
    }
    let record = primary_record(&timeline);
    assert!(
        (record.age().value() - death).abs() <= 1e-9 * death,
        "{:?} against the pin at {death} yr",
        record.age()
    );
    assert_eq!(record.remnant(), remnant);
    assert!((remnant.mass().value() - 6.812).abs() < 1e-3, "{remnant:?}");
    assert!(kick_speed_m_s(&record) <= 0.0, "{:?}", record.kick());
    assert!(record.bound(), "{}", describe(&timeline));
    let (a0, _) =
        axis_rsun_and_e(&timeline.state_at(Years::new(super::detached::last_living(death))))
            .expect("bound before");
    let (a1, _) = axis_rsun_and_e(&timeline.state_at(Years::new(death))).expect("bound after");
    assert!((a0 - 108.04).abs() < 0.01, "{a0} R☉");
    assert!((a1 / a0 - 1.0).abs() < 1e-6, "a {a0} → {a1} R☉");
}

/// P11.T4.k, F8 for any star but a pinned primary with its pin to come: a core whose helium star
/// has no life left at its stripping dies at once, through `Engine::die`. Two pairs of the pinned
/// thousand ([`pinned_pair`]):
///
/// - **0701** (28.20 + 11.45 M☉, 614 d, e 0.61, Z = 0.0005): the companion, grown to 27.07 M☉ by
///   the primary's transfer, gives mass back to the primary's black hole from 14.30 Myr in its core
///   helium burning, enters a common envelope around it at 14.4516 Myr, and collapses there to a
///   10.696 M☉ black hole: recorded, bound, kick-free (a direct collapse), with no recoil. (Until
///   generator version 26 the test read pair 0544, whose companion from 26, under the enforced
///   Humphreys–Davidson limit, collapses detached at 8.4794 Myr, before any envelope.)
/// - **0145** (1.22 + 0.77 M☉, 494 d, Z = 0.004): the primary's envelope at 4,377.6 Myr leaves a
///   0.524 M☉ helium star whose carbon–oxygen core is past its shell limit, a carbon–oxygen white
///   dwarf at once: the stripping's member dies now (before T4.k it was placed alive). The dwarf
///   keeps the helium star's whole mass (HPT section 6.1, ruling 40), so `lose_mass` takes nothing
///   and the orbit after is the one the envelope left.
#[test]
fn a_core_stripped_past_its_end_dies_at_once() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit, own_members};
    use super::star::Member;
    use super::timeline::Context;

    let until = Years::new(1.2e10);
    let last_envelope = |timeline: &BinaryTimeline| {
        timeline
            .segments()
            .iter()
            .rev()
            .find(|s| s.kind() == SegmentKind::CommonEnvelope)
            .unwrap_or_else(|| panic!("a common envelope: {}", describe(timeline)))
            .start()
            .value()
    };

    let collapse = evolve(&pinned_sample_pair(701), until);
    let envelope = last_envelope(&collapse);
    assert!((envelope / 14.4516e6 - 1.0).abs() < 1e-5, "{envelope} yr");
    let record = *collapse
        .supernovae()
        .iter()
        .find(|s| s.component() == Component::Secondary)
        .unwrap_or_else(|| panic!("the companion's collapse: {}", describe(&collapse)));
    assert!(
        (record.age().value() - envelope).abs() <= 1e-9 * envelope,
        "{:?} against the envelope at {envelope} yr",
        record.age()
    );
    assert_eq!(
        record.remnant().kind(),
        crate::stellar::remnant::RemnantKind::BlackHole
    );
    assert!(
        (record.remnant().mass().value() - 10.696).abs() < 1e-3,
        "{:?}",
        record.remnant()
    );
    assert!(record.bound(), "{}", describe(&collapse));
    assert!(kick_speed_m_s(&record) <= 0.0, "{:?}", record.kick());
    let recoil = record.recoil().expect("bound").metres_per_second();
    assert!(
        recoil.iter().all(|v| v.abs() < 1e-6),
        "no mass lost, no recoil: {recoil:?} m s⁻¹"
    );
    let after = collapse.state_at(Years::new(envelope));
    assert_eq!(after.stars()[1].phase(), Phase::BlackHole);

    let input = &pinned_sample_pair(145);
    let dwarf = evolve(input, until);
    let envelope = last_envelope(&dwarf);
    assert!((envelope / 4_377.62e6 - 1.0).abs() < 1e-5, "{envelope} yr");
    let after = dwarf.state_at(Years::new(envelope));
    let shown = after.stars()[0];
    assert_eq!(shown.phase(), Phase::CarbonOxygenWhiteDwarf);
    assert!((shown.mass().value() - 0.524).abs() < 1e-3, "{shown:?}");
    let segment = dwarf
        .segments()
        .iter()
        .find(|s| s.start().value() >= envelope && s.kind() == SegmentKind::Detached)
        .expect("detached after the envelope");
    let Member::Track { track, offset } = &segment.members()[0] else {
        panic!("the primary on its stripped track: {}", describe(&dwarf));
    };
    assert!(track.is_remnant_from_its_start(), "{}", describe(&dwarf));
    assert!(
        (offset - envelope).abs() <= 1e-9 * envelope,
        "placed at {offset} yr, the envelope at {envelope} yr"
    );
    let p = track.death().expect("dead").progenitor();
    let before = p.helium_core_mass().value() + p.envelope_mass().value();
    let born = track.remnant().expect("a white dwarf").mass().value();
    assert!(
        (before - born).abs() < 1e-12,
        "the dwarf keeps its helium star's {before} M☉: {born}"
    );
    // The stripping itself, at the envelope's age, on the primary's own track: it dies now.
    let (a, e) = axis_rsun_and_e(&after).expect("bound after the envelope");
    let orbit = LiveOrbit::new(
        envelope,
        a,
        e,
        *input.orbit().orientation(),
        input.orbit().mean_anomaly_at_epoch(),
    );
    let engine = Engine::new(
        Arc::new(Context::of(input)),
        envelope,
        until.value(),
        own_members(input, until.value(), None),
        orbit,
        None,
    );
    let (member, dies) = engine.stripped_member(0);
    assert!(dies, "the white dwarf is born now, through `die`");
    let Member::Track { track, .. } = member else {
        panic!("a stripped track: {member:?}");
    };
    assert!(track.is_remnant_from_its_start(), "{:?}", track.lifetime());
    assert_eq!(
        track.remnant().map(|r| r.kind()),
        Some(crate::stellar::remnant::RemnantKind::WhiteDwarf)
    );
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

/// P11.T4.j: a wide pair is still two single stars. 1 + 0.8 M☉ at 10⁶ d fails at 13.8 Gyr (at
/// 10⁴ d the companion takes the primary's wind since P11.T12's fix). The
/// bound's boundary for a circular 1 + 0.8 M☉ pair ([Fe/H] 0, median draws) lies at P₀ ≈ 0.73 d
/// at 1 Gyr and 1.54 d at 10 Gyr, against the science check's 0.64 and 1.22 d from BSE equation
/// 50 at a locked spin alone (plan 11's Risks): above them by the spins' reservoir and the
/// safety factor, and over twice the lobe test's own boundary (0.30 and 0.54 d).
#[test]
fn a_wide_pair_is_still_passed_over() {
    assert!(!can_interact(
        &pair(1.0, 0.8, 1.0e6, 0.0, 0.02),
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
/// merger. Pair 0030 of the pinned thousand (8.18 + 2.36 M☉, 3,657 d, e 0.11, [Fe/H] 0): at
/// 40.67 Myr the primary's super-AGB track goes through the post-AGB (1.37 M☉) to an oxygen–neon
/// white dwarf, and its death sheds the envelope its progenitor still holds at once, through BSE
/// appendix A1's instantaneous mass loss with no kick (ruling 129.4a), which unbinds the orbit. The
/// physics, an adiabatic superwind instead, is left for its own ruling. (The ruling's own trace,
/// 10.2 + 7.88 M☉ at 9,373 d, is now unbound earlier, by its pinned primary's collapse, which
/// P11.T4.k's F7 applies.)
#[test]
fn a_white_dwarfs_birth_that_unbinds_the_orbit_disrupts_it() {
    let input = pinned_sample_pair(30);
    // As the pinned thousand runs it: the pre-test passes the pair by then (P11.T4.j's bound).
    let timeline = evolve(&input, Years::new(1.2e10));
    let unbound = timeline
        .segments()
        .iter()
        .find(|s| s.kind() != SegmentKind::Detached)
        .expect("the orbit is unbound");
    assert_eq!(
        unbound.kind(),
        SegmentKind::Disrupted {
            by: Component::Primary
        },
        "{}",
        describe(&timeline)
    );
    let at = unbound.start().value();
    assert!((at - 40.67e6).abs() < 1.0e4, "unbound at {at} yr");
    let state = timeline.state_at(unbound.start());
    assert_eq!(state.stars()[0].phase(), Phase::OxygenNeonWhiteDwarf);
    assert!(
        state.stars().iter().all(|s| s.phase() != Phase::NoRemnant) && state.orbit().is_none(),
        "{}",
        describe(&timeline)
    );
    assert!(timeline.supernovae().is_empty(), "{}", describe(&timeline));
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
///
/// From generator version 26, under the enforced Humphreys–Davidson limit and on the tables
/// refitted at 26, the pre-test passes over 13 (1 of the pinned sample's ranges, 12 of the giants'
/// captures): primaries of 32–39 M☉ on orbits of 6,400–15,000 days that the engine takes into a
/// common envelope or a stable transfer at 5.8–6.8 Myr. A finding, pinned as it stands, at most 13.
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
    assert!(
        missed <= 13,
        "{missed} interactions the pre-test passed over"
    );
}

/// P11.T4.j: each term of the bound is the engine's, and goes with its [`BinaryParams`] switch.
/// The braked 1.04 + 0.45 M☉ pair at 0.69 d passes at 2 Gyr only with magnetic braking and the
/// tides that pass it to the orbit. A 0.3 + 0.3 M☉ pair at 0.2 d, below BSE's braking floor and
/// inside its lobes on the drawn orbit, passes at 13.8 Gyr by gravitational radiation alone, which
/// merges it in the engine at about 3.6 Gyr, and fails without it. A giant near its reach passes
/// only with the wind's spin (P11.T4.l).
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
    // P11.T4.l: a giant near its drawn reach, 1.5 + 0.45 M☉ at 2,300 d, passes by 13.8 Gyr only
    // by the spin its wind takes beyond its inertia's share, which the tides restore from the
    // orbit, and so only with the wind's spin switch on.
    let no_wind_spin = with(BinaryParams {
        wind_angular_momentum: false,
        ..BinaryParams::GENERATOR
    });
    let giant = || pair(1.5, 0.45, 2_300.0, 0.0, 0.02);
    assert!(!lobe_reached(&giant(), until));
    assert!(can_interact(&generator(giant()), until));
    assert!(
        !can_interact(&no_wind_spin(giant()), until),
        "without the wind's spin"
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

/// The `i`th pair of the pinned thousand ([`pinned_pair`]).
fn pinned_sample_pair(i: u32) -> BinaryInput {
    let mut mix = Mix(0x0b1e_0001);
    let mut input = pinned_pair(&mut mix, 0);
    for k in 1..=i {
        input = pinned_pair(&mut mix, k);
    }
    input
}

/// The primary's single-star model as plan 06 gives it, with the pair's own draws: its death,
/// remnant and kick (design note 16's pin).
fn plan_06_star(input: &BinaryInput) -> crate::stellar::system::StarModel {
    crate::stellar::system::StarModel::new(
        input.masses()[0],
        *input.composition(),
        input.draws()[0].clone(),
        input.age_at_epoch(),
    )
    .expect("a star")
}

/// The segment of `timeline` that ends at `age` (years, to 10⁻⁹ of it) and has length.
fn segment_ending_at(timeline: &BinaryTimeline, age: f64) -> &Segment {
    timeline
        .segments()
        .iter()
        .find(|s| {
            (s.end().value() - age).abs() <= 1e-9 * age && s.end().value() > s.start().value()
        })
        .unwrap_or_else(|| panic!("a segment ends at {age} yr: {}", describe(timeline)))
}

/// P11.T4.k, the carried tie (ruling p11-t4k-faults, 2026-10-06, §2.2): pair 0446 of the pinned
/// thousand (22.16 + 14.86 M☉, 147 d, e 0.04, Z = 0.0005). Its primary, which the binary carries
/// on the pin's own track after its transfer, dies on that track exactly at its pin (9.8218 Myr).
/// The pin wins the tie, and the star is read at its last living instant on its closed forms: an
/// early-AGB star of 8.625 M☉, the mass the binary gives it there (the landing step's, as
/// `Engine::die` reads a carried star's, equal to the path's at the last living instant to
/// 10⁻¹²). It collapses to plan 06's 8.576 M☉ black hole, capped at that mass, kick-free and
/// bound, and the orbit after is BSE appendix A1's for that mass before. Before T4.k the pin read
/// the remnant the track shows at its death as nothing living, and wrote no record.
#[test]
fn a_carried_primary_collapses_at_its_pin() {
    use super::star::Member;

    let input = pinned_sample_pair(446);
    let pin = super::evolve::pinned_death_age_years(&input).expect("a pinned primary");
    let timeline = evolve(&input, Years::new(1.2e10));
    let record = primary_record(&timeline);
    assert!(
        (record.age().value() - pin).abs() <= 1e-9 * pin,
        "{:?} against the pin at {pin} yr",
        record.age()
    );
    let before = segment_ending_at(&timeline, pin);
    let Member::Shaped {
        track,
        offset,
        mass,
    } = &before.members()[0]
    else {
        panic!("a carried primary before its pin: {}", describe(&timeline));
    };
    let death = offset + track.lifetime().expect("built to its death").value();
    assert!(
        (death - pin).abs() <= 1e-9 * pin,
        "its own track dies at the pin"
    );
    let last = super::detached::last_living(pin);
    let living = timeline.state_at(Years::new(last)).stars()[0];
    assert_eq!(living.phase(), Phase::EarlyAgb);
    let carried = mass.at(last);
    assert!(
        (living.mass().value() / carried - 1.0).abs() < 1e-12,
        "{living:?} against the carried {carried} M☉"
    );
    // The engine's mass before is the landing step's carried mass, the path's at the pin, as
    // `Engine::die` reads it: the path's at the last living instant, to the wind of 10⁻¹² of the
    // age.
    assert!(
        (mass.last() / carried - 1.0).abs() < 1e-12,
        "{} against {carried} M☉",
        mass.last()
    );
    assert!((carried - 8.625).abs() < 1e-3, "{carried}");
    let own = plan_06_star(&input).remnant().expect("a remnant");
    let expected = if own.mass().value() > carried {
        crate::stellar::remnant::CompactRemnant::new(own.kind(), SolarMasses::new(carried))
    } else {
        own
    };
    assert_eq!(record.remnant(), expected);
    assert!(
        (expected.mass().value() - 8.576).abs() < 1e-3,
        "{expected:?}"
    );
    assert!(record.bound(), "{}", describe(&timeline));
    assert!(kick_speed_m_s(&record) <= 0.0, "{:?}", record.kick());
    // The orbit after is BSE appendix A1's for that mass before, kick-free on the circular orbit:
    // a mass before off by 4 × 10⁻⁵ M☉ would move a′ by 10⁻⁶.
    let shown = timeline.state_at(Years::new(last));
    let (a0, e0) = axis_rsun_and_e(&shown).expect("bound before");
    assert!(e0 < 1e-9, "circular before the collapse: e {e0}");
    let companion = shown.stars()[1].mass().value();
    let (a1, e1) = blaauw_orbit_rsun(a0, carried + companion, expected.mass().value() + companion)
        .expect("less than half lost");
    let (a, e) = axis_rsun_and_e(&timeline.state_at(Years::new(pin))).expect("bound after");
    assert!((a / a1 - 1.0).abs() < 1e-6, "a {a} R☉ against A1's {a1}");
    assert!((e - e1).abs() < 1e-6, "e {e} against A1's {e1}");
}

/// P11.T4.k, finding A (ruling p11-t4k-faults, 2026-10-06, §1.1): pair 0257 of the pinned thousand
/// (21.48 + 18.99 M☉, 230 d, e 0.22, Z = 0.0005). The pinned primary, a stable-transfer donor
/// carried on its own track, dies on it exactly at its pin, 10.1571 Myr, on the transfer's last
/// step. The step reads the dying donor inside its lobe as its remnant, but it lands on the pin, so
/// the detachment test does not end it: the pin is acted on, an 8.283 M☉ black hole, bound, and the
/// transfer ends with its donor. Before T4.k the "transfer is over" return came first and the pin
/// was never seen again: no record. (Until generator version 26 the test read pair 0004, whose
/// transfer from 26, under the enforced Humphreys–Davidson limit, ends in a common envelope some
/// 4,100 yr before the pin.)
#[test]
fn a_transfers_last_step_applies_the_pin() {
    let input = pinned_sample_pair(257);
    let pin = super::evolve::pinned_death_age_years(&input).expect("a pinned primary");
    let timeline = evolve(&input, Years::new(1.2e10));
    let record = primary_record(&timeline);
    assert!(
        (record.age().value() - pin).abs() <= 1e-9 * pin,
        "{:?} against the pin at {pin} yr",
        record.age()
    );
    assert!((pin - 10.1571e6).abs() < 1.0e2, "{pin}");
    assert_eq!(
        segment_ending_at(&timeline, pin).kind(),
        SegmentKind::StableTransfer {
            donor: Component::Primary
        }
    );
    assert_eq!(
        record.remnant().kind(),
        crate::stellar::remnant::RemnantKind::BlackHole
    );
    assert!(
        (record.remnant().mass().value() - 8.283).abs() < 1e-3,
        "{:?}",
        record.remnant()
    );
    assert!(record.bound(), "{}", describe(&timeline));
    let after = timeline.state_at(Years::new(pin));
    assert!(
        !matches!(after.kind(), SegmentKind::StableTransfer { .. }),
        "the transfer ends with its donor: {}",
        describe(&timeline)
    );
}

/// P11.T4.k, finding A: pair 0318 of the pinned thousand (11.32 + 10.62 M☉ at 14.7 d). The
/// companion, its envelope taken, is a helium star, which transfers onto the primary's 1.51 M☉
/// neutron star from 26.1266 Myr (case BB, ruling 129.4b) and dies in that transfer at 26.1370 Myr,
/// as Tauris, Langer and Podsiadlowski (2015) find most such donors do. The transfer's last step
/// lands on the death and acts on it: a 1.856 M☉ neutron star, bound beside the first. No merger
/// follows. Before T4.k the death was never seen, and the dead donor was read past its death on its
/// closed forms. (Until P11.T4.o this test read pair 0741, whose helium star now collapses
/// detached: F17's shorter steps leave its primary 0.005 M☉ heavier after its first transfer.)
#[test]
fn a_transfers_last_step_applies_the_death() {
    let input = pinned_sample_pair(318);
    let timeline = evolve(&input, Years::new(1.2e10));
    let record = *timeline
        .supernovae()
        .iter()
        .find(|s| s.component() == Component::Secondary)
        .unwrap_or_else(|| panic!("the companion's collapse: {}", describe(&timeline)));
    let death = record.age().value();
    assert!((death - 26.136_98e6).abs() < 1.0e2, "{death}");
    assert_eq!(
        segment_ending_at(&timeline, death).kind(),
        SegmentKind::StableTransfer {
            donor: Component::Secondary
        }
    );
    let before = timeline.state_at(Years::new(super::detached::last_living(death)));
    assert!(
        matches!(
            before.stars()[1].phase(),
            Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
        ),
        "a helium-star donor (case BB): {:?}",
        before.stars()[1]
    );
    assert_eq!(before.stars()[0].phase(), Phase::NeutronStar);
    assert_eq!(
        record.remnant().kind(),
        crate::stellar::remnant::RemnantKind::NeutronStar
    );
    assert!(
        (record.remnant().mass().value() - 1.856).abs() < 1e-3,
        "{:?}",
        record.remnant()
    );
    assert!(record.bound(), "{}", describe(&timeline));
    assert_eq!(timeline.merger_age(), None, "{}", describe(&timeline));
}

/// P11.T4.k, finding C (ruling p11-t4k-faults, 2026-10-06, §1.2), as P11.T4.n leaves it: pair
/// 0277 of the pinned thousand (7.68 + 3.88 M☉, 0.674 d, Z = 0.004). After its Case A transfer the
/// pair is in contact from 19.67 Myr, and its primary, kept on its main sequence by the mass it
/// lost, would be there still at plan 06's death, 46.831 Myr. The contact's knots stop at the
/// primary's own main-sequence end, 41.914 Myr, as they stop at the pin (T4.k): the primary goes on
/// along its own track there, and the pair coalesces
/// ([`pinned_contact_pairs_collapse_from_past_their_main_sequences`]). A run to 30 Myr, inside the
/// contact, records nothing, stays in contact, and is the full run's to 30 Myr bit for bit, as
/// runs to 41 and 41.95 Myr are, after the contact's last knot before the end (40.55 Myr) and just
/// past the end, and one to 46.9 Myr, past the pin. Before T4.k the contact ran to its coalescence
/// at 47.51 Myr, past the pin, and the merger's product died at 69.05 Myr; before T4.n its knots
/// stopped at the pin, and the primary collapsed from its main sequence there.
#[test]
fn a_contact_stops_at_its_primarys_own_main_sequence_end() {
    let input = pinned_sample_pair(277);
    let (track, _) = super::evolve::pinned_track_and_pin(&input).expect("a pinned primary");
    let end = track.main_sequence_end().expect("a main sequence").value();
    assert!((end - 41.914e6).abs() < 1.0e3, "{end}");
    let timeline = evolve(&input, Years::new(1.2e10));
    assert_eq!(
        segment_ending_at(&timeline, end).kind(),
        SegmentKind::Contact
    );
    let early = Years::new(3.0e7);
    let short = evolve(&input, early);
    assert!(short.supernovae().is_empty(), "{}", describe(&short));
    assert_eq!(
        short.segments().last().map(Segment::kind),
        Some(SegmentKind::Contact)
    );
    // The build-age contract either side of the end: at 41 Myr, after the contact's last knot
    // before it, whose next knot is clamped to the end past that age; at 41.95 Myr, past the
    // coalescence; and at 46.9 Myr, past the collapse.
    for until in [
        early,
        Years::new(4.1e7),
        Years::new(4.195e7),
        Years::new(4.69e7),
    ] {
        assert_eq!(
            check_any_age(&input, until, Years::new(1.2e10)),
            EarlierRun::Same,
            "to {until:?}"
        );
    }
}

/// P11.T4.n (decision-observed-class-findings §B): pairs 0223 (14.16 + 6.00 M☉, 0.996 d) and 0277
/// (7.68 + 3.88 M☉, 0.674 d) of the pinned thousand, both at Z = 0.004, are contact pairs whose
/// primaries the binary kept on their main sequences to plan 06's deaths, where they collapsed at
/// τ 0.999 and 0.987 (Risks, "P11.T4.k's known departures"). Each contact now ends at its
/// primary's own main-sequence end, 15.521 and 41.914 Myr: the primary goes on along the pin's own
/// track at offset 0, and the pair coalesces, as a contact does when a star leaves its main
/// sequence, through a common envelope into a Hertzsprung-gap product of 20.00 and 11.50 M☉. That
/// product's own track dies before the pin, and it is held there, an early-AGB supergiant, until
/// the pin collapses it into plan 06's neutron star, bit for bit: 1.4222 and 1.1881 M☉, at 17.195
/// and 46.831 Myr.
#[test]
fn pinned_contact_pairs_collapse_from_past_their_main_sequences() {
    use hyperion_testkit::float::bits;

    use super::star::Member;

    for (i, end_myr, product_msun) in [(223, 15.5205, 19.997), (277, 41.9137, 11.500)] {
        let input = pinned_sample_pair(i);
        let timeline = evolve(&input, Years::new(1.2e10));
        let what = || format!("pinned pair {i:04}: {}", describe(&timeline));
        let (track, pin) = super::evolve::pinned_track_and_pin(&input).expect("a pinned primary");
        let end = track.main_sequence_end().expect("a main sequence").value();
        assert!((end / 1e6 - end_myr).abs() < 1e-4, "{end}: {}", what());
        assert_eq!(
            segment_ending_at(&timeline, end).kind(),
            SegmentKind::Contact
        );
        let envelope = timeline
            .segments()
            .iter()
            .find(|s| s.kind() == SegmentKind::CommonEnvelope)
            .unwrap_or_else(|| panic!("the contact coalesces: {}", what()));
        assert_eq!(bits(envelope.start().value()), bits(end), "{}", what());
        let Member::Shaped { offset, .. } = &envelope.members()[0] else {
            panic!("the primary on the pin's own track: {}", what());
        };
        assert_eq!(bits(*offset), bits(0.0), "at offset {offset}");
        let after = timeline.state_at(Years::new(end));
        assert_eq!(after.kind(), SegmentKind::Merged, "{}", what());
        assert_eq!(after.stars()[0].phase(), Phase::HertzsprungGap);
        assert!(
            (after.stars()[0].mass().value() - product_msun).abs() < 1e-3,
            "{:?}",
            after.stars()[0]
        );
        let at = pin.death.age().value();
        let record = primary_record(&timeline);
        assert!(
            (record.age().value() - at).abs() <= 1e-9 * at,
            "{record:?} against {at}"
        );
        let before = timeline.state_at(Years::new(super::detached::last_living(at)));
        assert_eq!(before.stars()[0].phase(), Phase::EarlyAgb, "{}", what());
        assert_eq!(Some(record.remnant()), plan_06_star(&input).remnant());
        assert_eq!(record.kick(), pin.kick);
        check_no_main_sequence_collapse(&input, &timeline, &what);
    }
}

/// P11.T4.n (decision-observed-class-findings §B): pair 0040 of the pinned thousand (14.77 +
/// 2.93 M☉, 2.23 d, Z = 0.004), whose stars merge on their main sequences into a 17.656 M☉ star on
/// a track of its own, younger than the primary (BSE equation 80), that holds the pin. At the
/// primary's own main-sequence end, 14.637 Myr, a step of the merged star lands on it, and the
/// product goes on along the pin's own track at offset 0: a Hertzsprung-gap star of its mass with
/// that track's 3.62 M☉ core, its L up 0.21 dex, from log L 4.522 to 4.734. It collapses at the
/// pin, 16.210 Myr, an early-AGB supergiant, into plan 06's remnant with plan 06's kick, bit for
/// bit. Before T4.n the product collapsed there from its main sequence.
#[test]
fn a_pinned_merger_product_leaves_its_main_sequence_at_the_primarys_end() {
    use hyperion_testkit::float::bits;

    use super::star::Member;

    let input = pinned_sample_pair(40);
    let timeline = evolve(&input, Years::new(1.2e10));
    let what = || format!("pinned pair 0040: {}", describe(&timeline));
    let (track, pin) = super::evolve::pinned_track_and_pin(&input).expect("a pinned primary");
    let end = track.main_sequence_end().expect("a main sequence").value();
    assert!((end - 14.6366e6).abs() < 1.0e2, "{end}");
    let closed = segment_ending_at(&timeline, end);
    assert_eq!(closed.kind(), SegmentKind::Merged, "{}", what());
    let Member::Track {
        track: product,
        offset,
    } = &closed.members()[0]
    else {
        panic!("the merger's product on its own track: {}", what());
    };
    let own = product
        .main_sequence_end()
        .expect("built past its main sequence");
    assert!(
        offset + own.value() > end,
        "its main sequence outlasts the primary's"
    );
    let next = timeline
        .segments()
        .iter()
        .find(|s| (s.start().value() - end).abs() <= 1e-9 * end && s.end() > s.start())
        .unwrap_or_else(|| panic!("a segment starts at the end: {}", what()));
    assert_eq!(bits(next.start().value()), bits(end));
    let Member::Shaped {
        track: shaped,
        offset,
        ..
    } = &next.members()[0]
    else {
        panic!("the product on the pin's own track: {}", what());
    };
    assert_eq!(bits(*offset), bits(0.0), "at offset {offset}");
    assert_eq!(shaped.initial_mass(), track.initial_mass());
    let [was, now] =
        [end * (1.0 - 1e-12), end].map(|t| timeline.state_at(Years::new(t)).stars()[0]);
    assert_eq!(was.phase(), Phase::MainSequence);
    assert_eq!(now.phase(), Phase::HertzsprungGap);
    assert!((was.mass().value() - 17.656).abs() < 1e-3, "{was:?}");
    assert!(
        (now.mass().value() / was.mass().value() - 1.0).abs() < 1e-9,
        "{now:?}"
    );
    let log_l = |s: &crate::stellar::StarState| crate::math::log10(s.luminosity().value());
    assert!((log_l(&was) - 4.522).abs() < 1e-3, "{was:?}");
    assert!((log_l(&now) - 4.734).abs() < 1e-3, "{now:?}");
    assert!((now.core_mass().value() - 3.623).abs() < 1e-3, "{now:?}");
    let at = pin.death.age().value();
    let record = primary_record(&timeline);
    assert!(
        (record.age().value() - at).abs() <= 1e-9 * at,
        "{record:?} against {at}"
    );
    let before = timeline.state_at(Years::new(super::detached::last_living(at)));
    assert_eq!(before.stars()[0].phase(), Phase::EarlyAgb, "{}", what());
    assert_eq!(Some(record.remnant()), plan_06_star(&input).remnant());
    assert_eq!(record.kick(), pin.kick);
    check_no_main_sequence_collapse(&input, &timeline, &what);
    for until in [end * (1.0 - 1e-6), end * (1.0 + 1e-6)] {
        assert_eq!(
            check_any_age(&input, Years::new(until), Years::new(1.2e10)),
            EarlierRun::Same,
            "to {until} yr"
        );
    }
}

/// The pair of record 822C72E00000008C's primary in the universe LOOK72 (seed
/// `0xa49f_649b_ef43_4b20`, the full potential), as `SystemStars` runs it: its input, its timeline
/// and the primary's model.
fn look72_case_a_pair() -> (
    BinaryInput,
    BinaryTimeline,
    crate::stellar::system::StarModel,
) {
    use crate::galaxy::Galaxy;
    use crate::galaxy::placement::resolve;
    use crate::stellar::multiplicity::{HierarchyNode, StarIndex};
    use crate::stellar::system::SystemStars;

    let galaxy = Galaxy::new(crate::Seed::new(0xa49f_649b_ef43_4b20)).with_full_potential();
    let id: crate::id::SystemId = "822c72e00000008c".parse().expect("a system ID");
    let record = resolve(&galaxy, id).expect("a grid system of LOOK72");
    let stars = SystemStars::generate(&galaxy, &record);
    let pair = stars
        .pairs()
        .iter()
        .find(|pair| pair.stars()[0] == StarIndex::PRIMARY)
        .expect("the engine runs the primary's pair");
    let HierarchyNode::Pair { orbit, .. } = *stars.hierarchy().node(pair.node()) else {
        panic!("a pair's node is a pair");
    };
    let [primary, companion] = pair.stars().map(|s| &stars.stars()[usize::from(s.get())]);
    let input = BinaryInput::new(
        primary.initial_mass(),
        companion.initial_mass(),
        *primary.composition(),
        orbit,
        [primary.draws().clone(), companion.draws().clone()],
        primary.age_at_epoch(),
    )
    .expect("a pair");
    (input, pair.timeline().clone(), primary.clone())
}

/// P11.T4.n (decision-observed-class-findings §B): record 822C72E00000008C of the universe LOOK72
/// ([`look72_case_a_pair`]), a pinned primary of 10.486 M☉ at Z = 0.03
/// whose companion keeps it on its main sequence through a case-A transfer. At its own track's
/// main-sequence end, 21.8206 Myr, the segment closes inside the transfer, and the donor, a
/// 6.674 M☉ main-sequence star the binary carries, goes on along the pin's own track at offset 0:
/// a Hertzsprung-gap star of that mass with the track's 2.196 M☉ core (Mc,TMS at the track's own
/// mass there, 10.15 M☉ after its wind; the ruling's ≈ 2.3 M☉ is that of 10.49 M☉). L steps up
/// 0.766 dex there, from log L 3.517 to 4.282 (the ruling's 3.56 to 4.31). Case AB strips it to a
/// naked helium star of 2.24 M☉ by 21.866 Myr, which collapses at its pin, 24.6187 Myr, into plan
/// 06's 1.1927 M☉ neutron star with plan 06's kick, bit for bit. Before T4.n it collapsed from its
/// main sequence, a 5.80 M☉ star at log L 3.31.
#[test]
fn a_pinned_case_a_donor_leaves_its_main_sequence_at_its_own_end() {
    use hyperion_testkit::float::bits;

    use super::star::Member;

    let (input, timeline, primary) = look72_case_a_pair();
    let timeline = &timeline;
    let what = || format!("822C72E00000008C: {}", describe(timeline));
    let (track, pin) = super::evolve::pinned_track_and_pin(&input).expect("a pinned primary");
    let end = track.main_sequence_end().expect("a main sequence").value();
    assert!((end - 21.8206e6).abs() < 1.0e2, "{end}");
    let closed = segment_ending_at(timeline, end);
    assert_eq!(
        closed.kind(),
        SegmentKind::StableTransfer {
            donor: Component::Primary
        },
        "{}",
        what()
    );
    assert!(matches!(
        closed.members()[0],
        Member::MainSequence { helium: false, .. }
    ));
    let next = timeline
        .segments()
        .iter()
        .find(|s| (s.start().value() - end).abs() <= 1e-9 * end && s.end() > s.start())
        .unwrap_or_else(|| panic!("a segment starts at the end: {}", what()));
    // The step lands on the end exactly, and the track is placed at zero there.
    assert_eq!(bits(next.start().value()), bits(end));
    let Member::Shaped {
        track: shaped,
        offset,
        ..
    } = &next.members()[0]
    else {
        panic!("the donor on the pin's own track: {}", what());
    };
    assert_eq!(bits(*offset), bits(0.0), "at offset {offset}");
    assert_eq!(shaped.initial_mass(), track.initial_mass());
    assert_eq!(shaped.main_sequence_end(), track.main_sequence_end());
    let [was, now] =
        [end * (1.0 - 1e-12), end].map(|t| timeline.state_at(Years::new(t)).stars()[0]);
    assert_eq!(was.phase(), Phase::MainSequence);
    assert_eq!(now.phase(), Phase::HertzsprungGap);
    assert!((was.mass().value() - 6.674).abs() < 1e-3, "{was:?}");
    assert!(
        (now.mass().value() / was.mass().value() - 1.0).abs() < 1e-9,
        "{now:?}"
    );
    let log_l = |s: &crate::stellar::StarState| crate::math::log10(s.luminosity().value());
    assert!((log_l(&was) - 3.517).abs() < 1e-3, "{was:?}");
    assert!((log_l(&now) - 4.282).abs() < 1e-3, "{now:?}");
    assert!((now.core_mass().value() - 2.196).abs() < 1e-3, "{now:?}");
    let stripped = timeline.state_at(Years::new(2.2e7)).stars()[0];
    assert_eq!(stripped.phase(), Phase::HeliumMainSequence, "{}", what());
    assert!(
        (stripped.mass().value() - 2.24).abs() < 0.01,
        "{stripped:?}"
    );
    let at = pin.death.age().value();
    assert!((at - 24.6187e6).abs() < 1.0e2, "{at}");
    let collapse = primary_record(timeline);
    assert!(
        (collapse.age().value() - at).abs() <= 1e-9 * at,
        "{collapse:?} against {at}"
    );
    let before = timeline.state_at(Years::new(super::detached::last_living(at)));
    assert_eq!(before.stars()[0].phase(), Phase::HeliumMainSequence);
    assert_eq!(Some(collapse.remnant()), primary.remnant());
    assert_eq!(collapse.kick(), primary.natal_kick());
    check_no_main_sequence_collapse(&input, timeline, &what);
    // The build-age contract either side of the end: a run to just before it, whose last step
    // lands on it past the age asked, and one to just past it.
    for until in [end * (1.0 - 1e-6), end * (1.0 + 1e-6)] {
        assert_eq!(
            check_any_age(&input, Years::new(until), timeline.until()),
            EarlierRun::Same,
            "to {until} yr"
        );
    }
}

/// The 17 + 12 M☉ pair at 3,300 R☉ ([Fe/H] 0, median draws) without natal kicks, and its pin's
/// age, years.
fn untouched_pinned_pair() -> (BinaryInput, f64) {
    let (m1, m2) = (17.0, 12.0);
    let input = pair(m1, m2, period_days(m1, m2, 3_300.0), 0.0, 0.02).with_params(BinaryParams {
        natal_kicks: false,
        ..BinaryParams::GENERATOR
    });
    let pin = super::evolve::pinned_death_age_years(&input).expect("a pinned primary");
    (input, pin)
}

/// An engine for `input`, started at `age_years` on a circular orbit of `a_rsun`, with its primary
/// on the pin's own track and the pin still pending.
fn engine_with_its_pin(input: &BinaryInput, age_years: f64, a_rsun: f64) -> super::evolve::Engine {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit, own_members, pinned_track_and_pin};
    use super::timeline::Context;

    let until = 5.0e7;
    let (track, pin) = pinned_track_and_pin(input).expect("a pinned primary");
    let orbit = LiveOrbit::new(
        age_years,
        a_rsun,
        0.0,
        *input.orbit().orientation(),
        input.orbit().mean_anomaly_at_epoch(),
    );
    Engine::new(
        Arc::new(Context::of(input)),
        age_years,
        until,
        own_members(input, until, Some(track)),
        orbit,
        Some(pin),
    )
}

/// P11.T4.n's backstop: the pinned primary's own main-sequence end, still pending at the engine's
/// age as a phase's exit before its stop leaves it (a transfer that detaches on the step that
/// lands on the end), is acted on before the next phase. An engine at that end on the 17 + 12 M☉
/// pair's 4,396 R☉ orbit, its primary a carried main-sequence star of its own mass at τ 0.9, takes
/// the primary onto the pin's own track there, at offset 0 and its mass, where no step could land
/// on the end, and the pin collapses it later, once.
#[test]
fn a_main_sequence_end_due_at_the_engines_age_is_applied() {
    use super::star::{Member, Path};

    let (input, at) = untouched_pinned_pair();
    let end = super::evolve::own_main_sequence_end_of(&input);
    let (track, _) = super::evolve::pinned_track_and_pin(&input).expect("a pinned primary");
    let end_years = track.main_sequence_end().expect("a main sequence").value();
    let mut engine =
        engine_with_its_pin(&input, end_years.next_down(), 4_396.0).with_own_main_sequence_end(end);
    let mass = input.masses()[0].value();
    engine.members[0] = Member::MainSequence {
        helium: false,
        mass: Path::starting(engine.age, mass),
        tau: Path::starting(engine.age, 0.9),
    };
    // The engine reaches the end by an exit other than its stop.
    engine.age = end_years;
    engine.run();
    let timeline = engine.finish();
    let next = timeline
        .segments()
        .iter()
        .find(|s| s.start().value() >= end_years && s.end() > s.start())
        .unwrap_or_else(|| panic!("a segment from the end: {}", describe(&timeline)));
    assert!(
        (next.start().value() - end_years).abs() <= 1e-9 * end_years,
        "{}",
        describe(&timeline)
    );
    let Member::Shaped {
        track: shaped,
        offset,
        mass: carried,
    } = &next.members()[0]
    else {
        panic!(
            "the primary on the pin's own track: {}",
            describe(&timeline)
        );
    };
    assert!(offset.abs() <= 1e-9 * end_years, "at offset {offset}");
    assert_eq!(shaped.initial_mass(), track.initial_mass());
    assert!((carried.at(end_years) / mass - 1.0).abs() < 1e-12);
    let after = timeline.state_at(Years::new(end_years));
    assert_eq!(after.stars()[0].phase(), Phase::HertzsprungGap);
    let records = timeline
        .supernovae()
        .iter()
        .filter(|s| s.component() == Component::Primary)
        .count();
    assert_eq!(records, 1, "{}", describe(&timeline));
    let record = primary_record(&timeline);
    assert!(
        (record.age().value() - at).abs() <= 1e-9 * at,
        "{record:?} against {at}"
    );
}

/// P11.T4.k's backstop (ruling p11-t4k-faults, 2026-10-06, §2.1): a pin still pending at the
/// engine's age, as a phase's exit before its stop leaves it (T4.g's strip winning the step that
/// lands on the pin), is acted on before the next phase: an engine started at the pin, on the
/// 17 + 12 M☉ pair's 4,396 R☉ orbit, records the primary's collapse there once, with plan 06's
/// remnant.
#[test]
fn a_pin_due_at_the_engines_age_is_applied() {
    let (input, at) = untouched_pinned_pair();
    let mut engine = engine_with_its_pin(&input, at, 4_396.0);
    let pin = engine.pin.expect("pending");
    engine.run();
    let timeline = engine.finish();
    let records: Vec<_> = timeline
        .supernovae()
        .iter()
        .filter(|s| s.component() == Component::Primary)
        .collect();
    assert_eq!(records.len(), 1, "{}", describe(&timeline));
    assert!(
        (records[0].age().value() - at).abs() <= 1e-9 * at,
        "{:?} against the pin at {at} yr",
        records[0].age()
    );
    assert_eq!(records[0].remnant(), pin.remnant);
}

/// P11.T4.k's backstop: no phase passes a pin it has not acted on. An engine started past its
/// pin, which no phase can reach again, fails the debug assertion.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "lies behind the engine")]
fn a_pin_left_behind_is_a_fault() {
    let (input, at) = untouched_pinned_pair();
    let mut engine = engine_with_its_pin(&input, at + 1.0e3, 4_396.0);
    engine.run();
}

/// P11.T4.k, finding A's correction (ruling p11-t4k-faults, 2026-10-06, §2.1): a transfer whose
/// donor a death or the pin has left with nothing living ends there, the pair going on as
/// `quiet_kind` decides, here detached on its orbit; a living donor's transfer goes on.
#[test]
fn a_transfer_without_a_living_donor_ends() {
    use super::star::{Member, Path};

    let (input, at) = untouched_pinned_pair();
    let mut engine = engine_with_its_pin(&input, 0.5 * at, 4_396.0);
    let transfer = SegmentKind::StableTransfer {
        donor: Component::Primary,
    };
    engine.kind = transfer;
    engine.end_without_a_living_donor(0);
    assert_eq!(engine.kind, transfer, "a living donor's transfer goes on");
    engine.members[0] = Member::Remnant {
        phase: Phase::NeutronStar,
        birth: engine.age,
        origin: crate::units::Megayears::ZERO,
        mass: Path::starting(engine.age, 1.4),
    };
    engine.end_without_a_living_donor(0);
    assert_eq!(engine.kind, SegmentKind::Detached);
}

// --- P11.T4.l: a passed-over pair's collapses --------------------------------------------------

/// The universe time of the engine's age `age_years` in `input`'s pair, as the engine maps it.
fn universe_time_of(input: &BinaryInput, age_years: f64) -> crate::time::UniverseTime {
    super::supernova::universe_time_at(&super::timeline::Context::of(input), age_years)
}

/// The full tracks of `input`'s two stars, to compare a passed-over pair's stars with their own.
fn own_tracks(input: &BinaryInput) -> [crate::stellar::sse::Track; 2] {
    core::array::from_fn(|i| {
        crate::stellar::sse::Track::full(
            super::evolve::track_mass(input.masses()[i]),
            input.composition(),
            &input.draws()[i],
        )
    })
}

/// That every state of `timeline`, `input`'s run, holds each star as its own track's state, at 64
/// even ages and either side of each supernova.
fn assert_own_models(input: &BinaryInput, timeline: &BinaryTimeline) {
    let tracks = own_tracks(input);
    let until = timeline.until().value();
    let ages = (0..=64_u32).map(|k| until * f64::from(k) / 64.0).chain(
        timeline.supernovae().iter().flat_map(|s| {
            let age = s.age().value();
            [super::detached::last_living(age), age]
        }),
    );
    for age in ages {
        let state = timeline.state_at(Years::new(age));
        for (star, track) in state.stars().iter().zip(&tracks) {
            assert_eq!(
                *star,
                track.state_at(Years::new(age)),
                "at {age} yr: {}",
                describe(timeline)
            );
        }
    }
}

/// P11.T4.l (ruling p11-supernova-pins of 2026-10-06, finding F3): a wide pair the pre-test passes
/// over is unbound by its primary's collapse. 15 + 3 M☉ at P = 10⁵ d (about 23,760 R☉), circular,
/// \[Fe/H\] 0 with the median draws, kicks off: the pinned primary collapses at plan 06's death
/// (14.32 Myr) from its living 12.6 M☉ to a 1.43 M☉ neutron star, more than half the pair's mass,
/// so the pair is `Disrupted` by the primary (Blaauw 1961). With no kick each star keeps the
/// velocity it had in the barycentre's frame, on the orbit's rails widened by Jeans's mode, to
/// 10⁻⁹; there is no recoil, and both stars are their own models throughout. Before T4.l the pair
/// kept its drawn orbit through the collapse, with no record.
#[test]
fn a_wide_pair_is_unbound_by_its_primarys_collapse() {
    let params = BinaryParams {
        natal_kicks: false,
        ..BinaryParams::GENERATOR
    };
    let input = pair(15.0, 3.0, 1.0e5, 0.0, 0.02).with_params(params);
    let until = Years::new(1.0e8);
    assert!(!can_interact(&input, until), "the pre-test passes it over");
    let timeline = evolve(&input, until);
    let death = plan_06_star(&input).death().expect("a massive star dies");
    let d = death.age().value();
    let [record] = timeline.supernovae() else {
        panic!("one collapse: {}", describe(&timeline));
    };
    assert_eq!(record.component(), Component::Primary);
    assert!(
        (record.age().value() - d).abs() <= 1e-9 * d,
        "{:?} against plan 06's {d} yr",
        record.age()
    );
    assert_eq!(record.kick(), None);
    assert!(!record.bound());
    assert_eq!((record.recoil(), timeline.recoil()), (None, None));
    let kinds: Vec<SegmentKind> = timeline.segments().iter().map(Segment::kind).collect();
    assert_eq!(
        kinds,
        [
            SegmentKind::Detached,
            SegmentKind::Disrupted {
                by: Component::Primary
            }
        ],
        "{}",
        describe(&timeline)
    );
    let p = death.progenitor();
    let living = p.helium_core_mass().value() + p.envelope_mass().value();
    let before = timeline.state_at(Years::new(super::detached::last_living(d)));
    let companion = before.stars()[1].mass().value();
    let total = living + companion;
    let lost = total - (record.remnant().mass().value() + companion);
    assert!(lost > 0.5 * total, "{lost} of {total} M☉ lost");
    assert_eq!(
        before.orbit(),
        Some(input.orbit()),
        "the drawn orbit to the collapse"
    );
    assert_eq!(timeline.state_at(record.age()).orbit(), None);
    // Each star's velocity in the barycentre's frame before, on the widened rails.
    let f = (15.0 + 3.0) / total;
    let (_, v) = input
        .orbit()
        .relative_state_at(universe_time_of(&input, record.age().value()));
    let v = v.metres_per_second().map(|c| c / f);
    let speed = v.iter().map(|c| c * c).sum::<f64>().sqrt();
    let expected = [
        v.map(|c| -c * companion / total),
        v.map(|c| c * living / total),
    ];
    for (shown, expected) in record.velocities().iter().zip(expected) {
        for (a, b) in shown.metres_per_second().iter().zip(expected) {
            assert!((a - b).abs() <= 1e-9 * speed, "{a} against {b} m/s");
        }
    }
    assert_own_models(&input, &timeline);
}

/// P11.T4.l: a direct-collapse primary beside a wide companion stays bound. 40 + 10 M☉ at
/// P = 10⁵ d, circular, \[Fe/H\] 0 with the median draws: plan 06's 40 M☉ star, stripped by its
/// own winds to a 9.83 M☉ helium star, collapses at 5.51 Myr into a black hole of its whole mass
/// with no kick. BSE appendix A1 from the rails widened by Jeans's mode, by f = 50 ÷ 19.8 = 2.52
/// for the winds of both stars, leaves the orbit at A1's a′ and e′ for the mass lost (none, to
/// rounding, at version 23: the plan's "small mass lost" was the prototype's), here the widened
/// 84,260 R☉ circular orbit, with a record and the pair's recoil. The stars stay their own
/// models, and the companion's own collapse at 27.82 Myr is recorded on the orbit the first left.
/// With kicks off that collapse, which loses most of the companion's mass but under half the
/// pair's, keeps the pair bound: A1 from the first collapse's orbit, widened by Jeans's mode for the
/// companion's wind since, gives Blaauw's a′ and e′ and the recoil v `m_BH` ΔM ÷ (M M′), to 10⁻⁹.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the orbits' a and e, before and after, and the widening's f"
)]
fn a_wide_black_hole_pair_stays_bound() {
    let input = pair(40.0, 10.0, 1.0e5, 0.0, 0.02);
    let until = Years::new(1.0e8);
    assert!(!can_interact(&input, until), "the pre-test passes it over");
    let timeline = evolve(&input, until);
    let record = primary_record(&timeline);
    let d = record.age().value();
    assert!((d - 5.5118e6).abs() < 1.0e3, "{d}");
    assert_eq!(
        record.remnant().kind(),
        crate::stellar::remnant::RemnantKind::BlackHole
    );
    assert!(kick_speed_m_s(&record) <= 0.0, "{:?}", record.kick());
    assert!(record.bound(), "{}", describe(&timeline));
    let before = timeline.state_at(Years::new(super::detached::last_living(d)));
    let after = timeline.state_at(record.age());
    let (a0, e0) = axis_rsun_and_e(&before).expect("the drawn orbit");
    assert!(e0 < 1e-12 && (a0 - 33_401.1).abs() < 0.1, "{a0} R☉, e {e0}");
    let death = plan_06_star(&input).death().expect("a massive star dies");
    let p = death.progenitor();
    let living = p.helium_core_mass().value() + p.envelope_mass().value();
    let total = living + before.stars()[1].mass().value();
    let total_after = record.remnant().mass().value() + before.stars()[1].mass().value();
    let f = 50.0 / total;
    assert!((f - 2.52).abs() < 0.01, "f {f}");
    let (a1, e1) = blaauw_orbit_rsun(a0 * f, total, total_after).expect("bound");
    let (a, e) = axis_rsun_and_e(&after).expect("bound after the collapse");
    assert!((a / a1 - 1.0).abs() < 1e-9, "a {a} R☉ against A1's {a1}");
    assert!((e - e1).abs() < 1e-9, "e {e} against A1's {e1}");
    assert!((a - 84_263.4).abs() < 1.0, "{a} R☉");
    let recoil = record.recoil().expect("a bound pair recoils");
    let lost = total - total_after;
    let (_, v) = input.orbit().relative_state_at(universe_time_of(&input, d));
    let speed = v.speed().value() / f;
    let expected = speed * before.stars()[1].mass().value() * lost / (total * total_after);
    assert!(
        (recoil.speed().value() - expected).abs() <= 1e-9 * speed,
        "recoil {:?} against {expected} m/s",
        recoil.speed()
    );
    assert_eq!(timeline.recoil(), None, "the companion's kick unbinds it");
    let [_, second] = timeline.supernovae() else {
        panic!("the companion collapses too: {}", describe(&timeline));
    };
    assert_eq!(second.component(), Component::Secondary);
    assert!((second.age().value() - 27.8177e6).abs() < 1.0e3);
    assert_own_models(&input, &timeline);

    let no_kicks = input.clone().with_params(BinaryParams {
        natal_kicks: false,
        ..BinaryParams::GENERATOR
    });
    let timeline = evolve(&no_kicks, until);
    let [first, second] = timeline.supernovae() else {
        panic!("two collapses: {}", describe(&timeline));
    };
    assert!(first.bound() && second.bound(), "{}", describe(&timeline));
    let c = second.age().value();
    let before = timeline.state_at(Years::new(super::detached::last_living(c)));
    let after = timeline.state_at(second.age());
    let (a0, e0) = axis_rsun_and_e(&before).expect("the first collapse's orbit");
    assert!(e0 < 1e-12, "circular: e {e0}");
    let p = own_tracks(&no_kicks)[1]
        .death()
        .expect("the companion dies")
        .progenitor();
    let living = p.helium_core_mass().value() + p.envelope_mass().value();
    let hole = first.remnant().mass().value();
    let rails = hole + timeline.state_at(first.age()).stars()[1].mass().value();
    let (total, total_after) = (hole + living, hole + second.remnant().mass().value());
    let lost = total - total_after;
    assert!(
        lost > 0.25 * total && lost < 0.5 * total,
        "{lost} of {total} M☉ lost"
    );
    let f = rails / total;
    let (a1, e1) = blaauw_orbit_rsun(a0 * f, total, total_after).expect("bound");
    let (a, e) = axis_rsun_and_e(&after).expect("bound after the collapse");
    assert!((a / a1 - 1.0).abs() < 1e-9, "a {a} R☉ against A1's {a1}");
    assert!((e - e1).abs() < 1e-9, "e {e} against A1's {e1}");
    let (_, v) = before
        .orbit()
        .expect("the first collapse's orbit")
        .relative_state_at(universe_time_of(&no_kicks, c));
    let speed = v.speed().value() / f;
    let expected = speed * hole * lost / (total * total_after);
    let recoil = second.recoil().expect("a bound pair recoils");
    assert!(expected > 1.0e3, "{expected} m/s");
    assert!(
        (recoil.speed().value() - expected).abs() <= 1e-9 * speed,
        "recoil {:?} against {expected} m/s",
        recoil.speed()
    );
}

/// P11.T4.l: A1's input on a passed-over pair's rails is widened by Jeans's mode (Hurley, Tout and
/// Pols 2002, section 2.2): for the mass falling from M₀ to M, the separation is the drawn one × f,
/// f = M₀ ÷ M, the velocity ÷ f, so that the state lies on the orbit of a × f about M, of the
/// drawn e, at the true anomaly the rails give then, to 10⁻¹².
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the state's r and v, the orbit's e and k, the time t and the widening's f"
)]
fn the_widening_is_jeans_mode() {
    use crate::orbit::{Orbit, elements_from_state};

    use super::supernova::jeans_widened_state;

    let mu = |m: f64| GravitationalParameter::from_solar_masses(SolarMasses::new(m));
    let (m0, now) = (25.0, 16.0);
    let f = m0 / now;
    for (e, mean_anomaly, years) in [(0.0, 0.0, 1.0e7), (0.3, 1.0, 2.3e7), (0.7, 4.0, 3.1e8)] {
        let drawn = KeplerElements::from_period(
            Seconds::new(1.0e5 * 86_400.0),
            mu(m0),
            Eccentricity::new(e).expect("an eccentricity"),
            Orientation::new(Radians::new(0.3), Radians::new(0.5), Radians::new(1.2))
                .expect("an orientation"),
            Radians::new(mean_anomaly),
        )
        .expect("an orbit");
        let t = crate::time::UniverseTime::EPOCH
            .checked_add(
                crate::time::Span::from_seconds_f64(
                    years * crate::units::consts::SECONDS_PER_JULIAN_YEAR,
                )
                .expect("a span"),
            )
            .expect("a time");
        let (r, v) = drawn.relative_state_at(t);
        let (widened, slower) = jeans_widened_state(&drawn, m0, now, t);
        assert_eq!(widened, r * f, "the separation × f");
        for (a, b) in slower.metres_per_second().iter().zip(v.metres_per_second()) {
            assert!(
                (a * f - b).abs() <= 1e-15 * b.abs().max(1.0),
                "{a} × {f} against {b}"
            );
        }
        let Ok(Orbit::Bound(k)) = elements_from_state(widened, slower, mu(now), t) else {
            panic!("the widened state is bound");
        };
        let axis = drawn.semi_major_axis().value() * f;
        assert!(
            (k.semi_major_axis().value() / axis - 1.0).abs() < 1e-12,
            "a {:?} against {axis} m",
            k.semi_major_axis()
        );
        assert!(
            (k.eccentricity().value() - e).abs() < 1e-12,
            "e {:?} against {e}",
            k.eccentricity()
        );
        if e > 0.0 {
            // At the drawn e, the same mean anomaly is the same true anomaly.
            let turn = core::f64::consts::TAU;
            let phase =
                (k.mean_anomaly_at(t).value() - drawn.mean_anomaly_at(t).value()).rem_euclid(turn);
            assert!(
                phase.min(turn - phase) < 1e-12,
                "the phase moves by {phase} rad"
            );
            for (a, b) in [
                (k.inclination(), drawn.inclination()),
                (k.ascending_node(), drawn.ascending_node()),
                (k.argument_of_periapsis(), drawn.argument_of_periapsis()),
            ] {
                assert!((a.value() - b.value()).abs() < 1e-12, "{a:?} against {b:?}");
            }
        }
    }
}

/// The ages a passed-over pair of `timeline` is run to in
/// [`a_passed_over_pair_is_the_same_whatever_age_it_is_run_to`]: before its first collapse, just
/// past each collapse and between them, and late, but never a collapse's own age, where the run to
/// it ends at the instant a run past it applies the collapse.
fn ages_around_collapses(timeline: &BinaryTimeline) -> Vec<Years> {
    let collapses: Vec<f64> = timeline
        .supernovae()
        .iter()
        .map(|s| s.age().value())
        .collect();
    let mut ages = vec![0.5 * collapses[0], 0.999_999 * collapses[0]];
    for (k, &age) in collapses.iter().enumerate() {
        let next = collapses
            .get(k + 1)
            .copied()
            .unwrap_or(0.9 * timeline.until().value());
        ages.extend([
            age + 1.0e-3,
            age + 1.0e3,
            f64::midpoint(age, next),
            next * (1.0 - 1e-9),
        ]);
    }
    ages.into_iter().map(Years::new).collect()
}

/// P11.T4.l's build-age contract: a pair the pre-test passes over at every age shows the drawn
/// orbit up to its first collapse, and the closed form starts at each collapse, so a run to any age
/// before, between or after its collapses is the run to 10⁸ years to then, bit for bit, with the
/// same records. Over four wide pairs, kicks on and off, bound and unbound by each collapse.
#[test]
fn a_passed_over_pair_is_the_same_whatever_age_it_is_run_to() {
    let no_kicks = BinaryParams {
        natal_kicks: false,
        ..BinaryParams::GENERATOR
    };
    let later = Years::new(1.0e8);
    let (mut bound, mut unbound) = (0, 0);
    for input in [
        pair(15.0, 3.0, 1.0e5, 0.0, 0.02).with_params(no_kicks),
        pair(40.0, 10.0, 1.0e5, 0.0, 0.02),
        pair(15.0, 12.0, 1.0e5, 0.4, 0.02).with_params(no_kicks),
        pair(15.0, 12.0, 1.0e5, 0.0, 0.004),
    ] {
        assert!(!can_interact(&input, later));
        let late = evolve(&input, later);
        assert!(!late.supernovae().is_empty(), "{}", describe(&late));
        for record in late.supernovae() {
            if record.bound() {
                bound += 1;
            } else {
                unbound += 1;
            }
        }
        for until in ages_around_collapses(&late) {
            assert!(!can_interact(&input, until), "to {until:?}");
            let early = evolve(&input, until);
            assert_same_history(&early, &late, until);
            let reached: Vec<_> = late
                .supernovae()
                .iter()
                .filter(|s| s.age() < until)
                .copied()
                .collect();
            assert_eq!(early.supernovae(), reached, "to {until:?}");
        }
    }
    assert!(bound > 0 && unbound > 0, "{bound} bound, {unbound} unbound");
}

/// P11.T4.l: the engine that takes over a passed-over pair at a collapse carries each living star
/// with its zero-age spin's angular momentum times the wind's factor (M ÷ M₀)^{2/(3k′₃)}, held at
/// most at its break-up now, where [`super::evolve::Engine::new`] gives it the zero-age velocity
/// at its radius then. 15 + 14.9 M☉ (median draws, \[Fe/H\] 0): at the primary's death (14.32
/// Myr) the companion has left its main sequence, and `Engine::new`'s start would spin it past
/// break-up; a 3 M☉ companion is still on its main sequence and keeps its zero-age angular
/// momentum less its wind's factor's ulps. The remnant keeps `Engine::new`'s.
#[test]
fn a_taken_over_star_keeps_its_zero_age_spin() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit};
    use super::star::{G, Member, moment_of_inertia, zams_spin};
    use hyperion_testkit::float::bits;

    for (m2, evolved) in [(14.9, true), (3.0, false)] {
        let input = pair(15.0, m2, 1.0e5, 0.0, 0.02);
        let tracks = own_tracks(&input).map(Arc::new);
        let death = tracks[0].lifetime().expect("built in full").value();
        let arrival = tracks[1]
            .main_sequence_arrival()
            .expect("a main sequence")
            .value();
        let members = tracks.map(|track| Member::Track { track, offset: 0.0 });
        let k = input.orbit();
        let orbit = LiveOrbit::new(
            death,
            k.semi_major_axis().value() / crate::units::consts::SOLAR_RADIUS_M,
            0.5,
            *k.orientation(),
            k.mean_anomaly_at_epoch(),
        );
        let ctx = Arc::new(super::timeline::Context::of(&input));
        let mut engine = Engine::new(ctx, death, 1.0e8, members, orbit, None);
        let started = engine.spins;
        engine.take_over(Vec::new(), Vec::new());
        assert_eq!(bits(engine.spins[0]), bits(started[0]), "the remnant's");
        let (mass, tau) = engine.current(1);
        let now = engine.structure(1, death, mass, tau).expect("living");
        let zero_age = engine
            .structure(1, arrival, mass, tau)
            .expect("its arrival");
        let zero_age_momentum = moment_of_inertia(&zero_age)
            * zams_spin(
                zero_age.state.mass().value(),
                zero_age.state.radius().value(),
            );
        let (m, r) = (now.state.mass().value(), now.state.radius().value());
        let m0 = zero_age.state.mass().value();
        let wind =
            crate::math::powf_positive(m / m0, 2.0 / (3.0 * crate::stellar::sse::CORE_GYRATION));
        assert!(wind <= 1.0, "{m} M☉ from {m0}");
        let break_up = moment_of_inertia(&now) * (G * m / (r * r * r)).sqrt();
        assert_eq!(
            bits(engine.spins[1]),
            bits((zero_age_momentum * wind).min(break_up)),
            "{:?}",
            now.state
        );
        assert_eq!(
            now.state.phase() != Phase::MainSequence,
            evolved,
            "{:?}",
            now.state
        );
        if evolved {
            assert!(started[1] > break_up, "{} against {break_up}", started[1]);
            assert!(engine.spins[1] < started[1]);
        } else {
            assert!(
                1.0 - wind < 1e-5,
                "a main sequence's wind takes little: {wind}"
            );
            assert_eq!(bits(engine.spins[1]), bits(zero_age_momentum * wind));
        }
    }
}

/// P11.T4.l's label in the engine too (the train's fix): a pair its primary's collapse unbinds is
/// `Disrupted { by: Primary }` from then on, its companion's later collapse included, as the
/// closed form labels it (`Engine::passed_over`). 15 + 10.5 M☉ at 10 d (median draws, \[Fe/H\] 0),
/// which the engine runs: the primary's neutron star unbinds the pair at 14.32 Myr, and the
/// companion collapses alone at 19.75 Myr.
#[test]
fn a_disrupted_pair_keeps_the_label_of_the_death_that_unbound_it() {
    let input = pair(15.0, 10.5, 10.0, 0.0, 0.02);
    let until = Years::new(1.0e8);
    assert!(can_interact(&input, until));
    let timeline = evolve(&input, until);
    let [first, second] = timeline.supernovae() else {
        panic!("two collapses: {}", describe(&timeline));
    };
    assert_eq!(first.component(), Component::Primary);
    assert!(!first.bound(), "{}", describe(&timeline));
    assert_eq!(second.component(), Component::Secondary);
    assert!(
        (first.age().value() - 14.32e6).abs() < 1.0e4
            && (second.age().value() - 19.75e6).abs() < 1.0e4,
        "{}",
        describe(&timeline)
    );
    let after: Vec<&Segment> = timeline
        .segments()
        .iter()
        .filter(|s| s.start() >= first.age())
        .collect();
    assert!(
        after.iter().any(|s| s.start() >= second.age()),
        "{}",
        describe(&timeline)
    );
    for segment in after {
        assert_eq!(
            segment.kind(),
            SegmentKind::Disrupted {
                by: Component::Primary
            },
            "{}",
            describe(&timeline)
        );
    }
}

/// P11.T4.e's label for a pinned pair instability (the train's fix): a pinned primary the binary
/// carries off its own track, whose plan 06 death is a pair instability below the 150 M☉ cap,
/// leaves nothing at its pin ([`super::star::Member::Gone`]), and the pair that death unbinds is
/// `Disrupted { by: Primary }` from then on, as the closed form labels the same death: no merger,
/// so no `merger_age`. 149.9 + 74.95 M☉ at 10 d, Z = 10⁻⁴ (median draws): a 65.18 M☉ helium core
/// at the pin, 2.757 Myr, after a stable transfer from the primary. (Until generator version 26 it
/// read 145 + 72.5 M☉, a 66 M☉ core; from 26 that primary's track leaves a 62.98 M☉ core, a black
/// hole, and at Z = 10⁻⁴ only a primary from about 149.6 M☉ keeps the 65 M☉ core of a pair
/// instability below the cap.)
#[test]
fn a_pinned_pair_instability_disrupts_the_pair() {
    use crate::stellar::remnant::RemnantKind;

    let input = pair(149.9, 74.95, 10.0, 0.0, 0.0001);
    let until = Years::new(1.0e8);
    let (_, pin) = super::evolve::pinned_track_and_pin(&input).expect("a pinned primary");
    assert_eq!(pin.remnant.kind(), RemnantKind::None, "a pair instability");
    assert!(can_interact(&input, until));
    let timeline = evolve(&input, until);
    let collapse = timeline
        .supernovae()
        .iter()
        .find(|r| r.component() == Component::Primary)
        .unwrap_or_else(|| panic!("the pin: {}", describe(&timeline)));
    let at = pin.death.age().value();
    assert!(
        (collapse.age().value() - at).abs() <= 1e-9 * at,
        "{collapse:?} at {at}"
    );
    assert_eq!(collapse.remnant().kind(), RemnantKind::None);
    assert!(!collapse.bound(), "{}", describe(&timeline));
    assert!(
        timeline
            .segments()
            .iter()
            .any(|s| s.start() < collapse.age()
                && matches!(s.kind(), SegmentKind::StableTransfer { .. })),
        "{}",
        describe(&timeline)
    );
    assert_eq!(timeline.merger_age(), None, "{}", describe(&timeline));
    // The companion collapses later, alone: `quiet_kind` keeps the label past it.
    let companion = timeline
        .supernovae()
        .iter()
        .find(|r| r.component() == Component::Secondary)
        .unwrap_or_else(|| panic!("the companion's collapse: {}", describe(&timeline)));
    let after: Vec<&Segment> = timeline
        .segments()
        .iter()
        .filter(|s| s.start() >= collapse.age())
        .collect();
    assert!(
        after.first().is_some_and(|s| s.members()[0].is_gone())
            && after.iter().any(|s| s.start() >= companion.age()),
        "{}",
        describe(&timeline)
    );
    for segment in after {
        assert_eq!(
            segment.kind(),
            SegmentKind::Disrupted {
                by: Component::Primary
            },
            "{}",
            describe(&timeline)
        );
    }
}

/// P11.T4.l: the pre-test asked again after a passed-over pair's bound collapse never passes over
/// an interaction. Over 3,000 wide massive pairs ([`wide_massive_pair`]) run to their ages, every
/// pair whose bound collapse the closed form keeps on its fixed orbit shows no stable transfer,
/// common envelope, contact or merger after the collapse before its age when the engine runs it
/// from the collapse anyway ([`super::evolve::evolve_past_the_second_pre_test`]). None is allowed.
/// The bound pairs checked, the engine's takeovers and the interactions are recorded.
#[test]
#[ignore = "slow: 3,000 wide massive pairs, those bound after a collapse run through the engine"]
fn the_second_pre_test_never_passes_over_an_interaction() {
    use super::evolve::evolve_past_the_second_pre_test;

    let mut mix = Mix(0x0b1e_0c12);
    let pairs: Vec<(BinaryInput, Years)> = (0..3_000)
        .map(|i| {
            let (input, until) = wide_massive_pair(&mut mix, i);
            (input, Years::new(until))
        })
        .collect();
    // In the tally: `pairs` the pairs passed over with a bound collapse before their age,
    // `passes` those the engine takes over, `interacting` those the engine run from the collapse
    // brings into an interaction, and `missed` those of them the closed form kept.
    let t = in_shares(&pairs, |tally, (input, until)| {
        if can_interact(input, *until) {
            return;
        }
        let timeline = evolve(input, *until);
        if !timeline.supernovae().iter().any(SupernovaRecord::bound) {
            return;
        }
        tally.pairs += 1;
        let taken_over = taken_over_at(&timeline).is_some();
        tally.passes += u32::from(taken_over);
        let forced = evolve_past_the_second_pre_test(input, *until);
        if let Some(segment) = interaction_after_a_collapse(&forced, *until) {
            tally.interacting += 1;
            if !taken_over {
                let [m1, m2] = input.masses().map(SolarMasses::value);
                tally.missed.push(format!(
                    "{m1:.3} + {m2:.3} M☉ at {:.3e} d, e {:.2}, to {:.3e} yr: {:?} at {:.4e} yr",
                    input.orbit().period().value() / 86_400.0,
                    input.orbit().eccentricity().value(),
                    until.value(),
                    segment.kind(),
                    segment.start().value()
                ));
            }
        }
    });
    eprintln!(
        "{} passed-over pairs bound after a collapse; the pre-test asked again passes {}; the \
         engine run from the collapse interacts in {}; missed {}",
        t.pairs,
        t.passes,
        t.interacting,
        t.missed.len()
    );
    for line in t.missed.iter().take(20) {
        eprintln!("  missed: {line}");
    }
    assert!(t.pairs > 100, "{} bound pairs checked", t.pairs);
    assert_eq!(
        t.missed.len(),
        0,
        "interactions the pre-test asked again passed over"
    );
}

// --- P11.T4.m: a fed helium star or white dwarf swells around its own core ---------------------

/// An engine at `age_years` of a pair at metallicity `z` holding `accretor` in slot 0 and a
/// 2.5 M☉ main-sequence donor at τ = 0.5 in slot 1, in stable transfer from it on a circular
/// 10 R☉ orbit: the state at the end of a step of transfer, for the swell's unit tests.
fn fed_engine(accretor: super::star::Member, age_years: f64, z: f64) -> super::evolve::Engine {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit};
    use super::star::{Member, Path};
    use super::timeline::Context;

    let input = pair(2.0, 2.5, 1.0, 0.0, z);
    let k = input.orbit();
    let donor = Member::MainSequence {
        helium: false,
        mass: Path::starting(age_years, 2.5),
        tau: Path::starting(age_years, 0.5),
    };
    let orbit = LiveOrbit::new(
        age_years,
        10.0,
        0.0,
        *k.orientation(),
        k.mean_anomaly_at_epoch(),
    );
    let mut engine = Engine::new(
        Arc::new(Context::of(&input)),
        age_years,
        1.0e10,
        [accretor, donor],
        orbit,
        None,
    );
    engine.segment_start = age_years;
    engine.kind = SegmentKind::StableTransfer {
        donor: Component::Secondary,
    };
    engine
}

/// A helium main-sequence star of `m` M☉ at τ = `tau` that the binary carries from `age_years`.
fn carried_helium_star(m: f64, tau: f64, age_years: f64) -> super::star::Member {
    use super::star::{Member, Path};

    Member::MainSequence {
        helium: true,
        mass: Path::starting(age_years, m),
        tau: Path::starting(age_years, tau),
    }
}

/// The core mass of `member`, slot 0 of `engine`, at `age_years` and `m` M☉.
fn core_of(
    engine: &super::evolve::Engine,
    member: &super::star::Member,
    age_years: f64,
    m: f64,
) -> f64 {
    member
        .evaluate(&engine.ctx, 0, age_years, m, 0.0)
        .expect("a star")
        .state
        .core_mass()
        .value()
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06, §3.1 item 4, §3.3 A): a 2 M☉ helium
/// main-sequence accretor at τ = 0.5 fed hydrogen over a step of 10³ yr swells only where both
/// gates pass. Fed at half the placed giant's core growth over the step, its shell would burn the
/// hydrogen as it lands (the burn gate); fed a hundred times that but losing 41/42 of it to its
/// own wind, its accretion does not beat 1.05 times its wind (BSE's gate, `evolv2.f` 1480), though
/// it gains mass. Each stays a helium star with the step's mass, and no segment closes. Fed twice
/// the growth with no wind, it swells into a core-helium-burning giant around its whole mass at
/// the step's start, with the step's gain as its envelope.
#[test]
fn a_swell_the_shell_would_burn_is_refused() {
    use super::rlof::Fed;
    use super::star::{Kind, Member};
    use crate::stellar::sse::NewStar;

    let (before, tau, age, dt) = (2.0, 0.5, 5.0e7, 1.0e3);
    let probe = fed_engine(carried_helium_star(before, tau, age), age, 0.02);
    let burnt = probe
        .structure(0, age, before, tau)
        .expect("a helium star")
        .burnt;
    let giant = probe
        .swelled_star(
            0,
            NewStar::CoreHeliumBurning { burnt },
            before * (1.0 + 1e-3),
            before,
        )
        .expect("a core-helium-burning star has a 2 M☉ core");
    // Read well above the core, where the core is not held at the mass.
    let heavy = 2.0 * before;
    let growth = core_of(&probe, &giant, age + dt, heavy) - core_of(&probe, &giant, age, heavy);
    assert!(
        growth > 0.0,
        "the giant's core grows over the step: {growth}"
    );
    for (gained, wind, swells) in [
        (0.5 * growth, 0.0, false),
        (100.0 * growth, 100.0 * growth * 41.0 / 42.0, false),
        (2.0 * growth, 0.0, true),
    ] {
        let m = before + gained - wind;
        let mut engine = fed_engine(carried_helium_star(m, tau, age), age, 0.02);
        let s = engine.structure(0, age, m, tau).expect("a helium star");
        let fed = Fed {
            kind: Kind::HeliumMainSequence,
            before,
            core_before: 0.0,
            gained,
            dt,
        };
        let swelled = engine.swell(0, Kind::HeliumMainSequence, m, &s, fed);
        let case =
            format!("fed {gained:e} M☉ against a wind of {wind:e} and a growth of {growth:e}");
        assert_eq!(swelled, swells, "{case}");
        if swells {
            assert_eq!(engine.segments.len(), 1, "{case}");
            let star = engine.members[0].state_at(&engine.ctx, 0, age);
            assert_eq!(star.phase(), Phase::CoreHeliumBurning, "{case}");
            let core = star.core_mass().value();
            assert!(
                (core - before).abs() <= 1e-9 * before,
                "{case}: core {core}"
            );
            let envelope = star.mass().value() - core;
            assert!(
                (envelope - (m - before)).abs() <= 1e-9 * m,
                "{case}: envelope {envelope}"
            );
        } else {
            assert!(
                engine.segments.is_empty(),
                "{case}: a refusal closes no segment"
            );
            assert!(
                matches!(engine.members[0], Member::MainSequence { helium: true, .. }),
                "{case}: {}",
                engine.members[0].form()
            );
            assert!(
                (engine.current(0).0 - m).abs() <= 1e-15 * m,
                "{case}: it keeps the step's mass"
            );
        }
    }
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06, §3.1 item 2): a helium Hertzsprung-gap star of
/// 1.45 M☉ (a 1.5 M☉ helium star halfway through its gap, Z = 0.02) fed hydrogen with no wind
/// swells into a thermally pulsing AGB star around its carbon–oxygen core at the step's start, to
/// 10⁻⁹, with the step's mass (BSE `evolv2.f` 1482 and 1487, `massc(j2)`). The same step read as
/// one over which the accretor's type changed swells nothing (BSE reads `kstar(j2)` at the step's
/// start), and closes no segment.
#[test]
fn a_fed_helium_giant_swells_around_its_carbon_oxygen_core() {
    use std::sync::Arc;

    use super::rlof::Fed;
    use super::star::{Kind, Member, Path};
    use crate::stellar::sse::{NewStar, Track};

    let (age, dt) = (5.0e7, 1.0e3);
    let track = Arc::new(Track::helium_star_full(
        SolarMasses::new(1.5),
        &Composition::SOLAR,
        &StarDraws::median(),
    ));
    let at = track
        .age_in_phase(Phase::HeliumHertzsprungGap, 0.5)
        .expect("a helium Hertzsprung gap");
    let offset = super::evolve::offset_for(age, at);
    let before = track.mass_at(age - offset);
    let giant = |m: f64| Member::Shaped {
        track: Arc::clone(&track),
        offset,
        mass: Path::starting(age, m),
    };
    let probe = fed_engine(giant(before), age, 0.02);
    let start = probe
        .structure(0, age, before, 0.0)
        .expect("a helium giant");
    assert_eq!(start.state.phase(), Phase::HeliumHertzsprungGap);
    let core_before = start.state.core_mass().value();
    let placed = probe
        .swelled_star(0, NewStar::PulsingAgb, 2.0 * before, core_before)
        .expect("a thermally pulsing AGB star has the core");
    let growth = core_of(&probe, &placed, age + dt, 2.0 * before)
        - core_of(&probe, &placed, age, 2.0 * before);
    let gained = 2.0 * growth.max(1e-9);
    let m = before + gained;
    for (step_kind, swells) in [(Kind::HeliumMainSequence, false), (Kind::HeliumGap, true)] {
        let mut engine = fed_engine(giant(m), age, 0.02);
        let s = engine.structure(0, age, m, 0.0).expect("a helium giant");
        let fed = Fed {
            kind: step_kind,
            before,
            core_before,
            gained,
            dt,
        };
        let case = format!("a step from a {step_kind:?}");
        assert_eq!(
            engine.swell(0, Kind::HeliumGap, m, &s, fed),
            swells,
            "{case}"
        );
        let star = engine.members[0].state_at(&engine.ctx, 0, age);
        if swells {
            assert_eq!(engine.segments.len(), 1, "{case}");
            assert_eq!(star.phase(), Phase::ThermallyPulsingAgb, "{case}");
            let core = star.core_mass().value();
            assert!(
                (core - core_before).abs() <= 1e-9 * core_before,
                "{case}: core {core} against {core_before}"
            );
            assert!(
                star.mass().value().total_cmp(&m).is_eq(),
                "{case}: a mass of {:?} for {m}",
                star.mass()
            );
        } else {
            assert!(
                engine.segments.is_empty(),
                "{case}: a refusal closes no segment"
            );
            assert_eq!(star.phase(), Phase::HeliumHertzsprungGap, "{case}");
        }
    }
}

/// P11.T4.m, the determinism audit: pinned pairs 0255 (a helium star's swell), 0305 (a white
/// dwarf's) and 0487 (a white dwarf's swell refused, its exact placement failing) are the same
/// timelines in either order, and each run to just past each of its events, its swells among them,
/// agrees with a run to 1.5 × 10¹⁰ years: the swell's track builds hold no state between runs.
#[test]
fn a_swell_is_the_same_in_any_order_and_at_any_age() {
    let until = Years::new(1.2e10);
    let inputs: Vec<BinaryInput> = [255, 305, 487]
        .map(pinned_sample_pair)
        .into_iter()
        .collect();
    let forward: Vec<_> = inputs.iter().map(|i| evolve(i, until)).collect();
    let backward: Vec<_> = inputs.iter().rev().map(|i| evolve(i, until)).collect();
    for (a, b) in forward.iter().zip(backward.iter().rev()) {
        assert_eq!(a, b);
    }
    assert!(
        forward[..2].iter().all(|t| !swells_of(t).is_empty()),
        "0255 and 0305 swell"
    );
    let events: usize = inputs
        .iter()
        .map(|input| check_after_events(input, until)[0])
        .sum();
    assert!(events > 20, "{events} runs compared just past events");
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06, §3.3 B): only a hydrogen-rich donor swells a helium
/// star, which then takes the whole transfer (BSE `evolv2.f` 1478–1479, the else branch of
/// `kstar(j1).ge.7`; Hurley, Tout and Pols 2002, section 2.6.6). A white dwarf, a helium star or a
/// neutron star feeding one gives it the thermally limited share, min(1, 10 `τ_Ṁ` ÷ `τ_KH`)
/// (BSE 1477), and it does not swell.
#[test]
fn a_white_dwarf_donor_does_not_swell_a_helium_star() {
    use super::rlof::kelvin_helmholtz;
    use super::star::Kind;

    let (m, tau, age) = (1.2, 0.3, 5.0e7);
    let engine = fed_engine(carried_helium_star(m, tau, age), age, 0.02);
    let sa = engine.structure(0, age, m, tau).expect("a helium star");
    let ka = Kind::HeliumMainSequence;
    for rate in [1e-9, 1e-6, 1e-3] {
        let thermal = (10.0 * (m / rate) / kelvin_helmholtz(&sa, ka)).min(1.0);
        for kd in [
            Kind::HeliumWhiteDwarf,
            Kind::CarbonOxygenWhiteDwarf,
            Kind::OxygenNeonWhiteDwarf,
            Kind::HeliumMainSequence,
            Kind::HeliumGiant,
            Kind::NeutronStar,
        ] {
            let accretion = engine.accretion(kd, ka, rate, m, &sa);
            assert!(!accretion.swells, "{kd:?} at {rate:e} M☉/yr");
            assert!(
                (accretion.share - thermal).abs() <= 1e-15,
                "{kd:?} at {rate:e} M☉/yr: {} against {thermal}",
                accretion.share
            );
        }
        for kd in [Kind::MainSequence, Kind::GiantBranch, Kind::PulsingAgb] {
            let accretion = engine.accretion(kd, ka, rate, m, &sa);
            assert!(accretion.swells, "{kd:?} at {rate:e} M☉/yr");
            assert!(
                accretion.share.total_cmp(&1.0).is_eq(),
                "{kd:?} at {rate:e} M☉/yr: a share of {}",
                accretion.share
            );
        }
    }
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06, §3.3 C; finding F15): a helium main-sequence
/// accretor keeps its fraction burnt as it accretes, τ + dt ÷ `t_HeMS`(m₁), as BSE's code
/// (`evolv2.f` 1889–1900) and COMPAS's `HURLEY` option do; a hydrogen main-sequence accretor
/// outside 0.35–1.25 M☉ is still mixed, τ m₀ ÷ m₁ + dt ÷ `t_MS`(m₁), and one inside keeps its τ.
#[test]
fn a_helium_accretor_keeps_its_fraction_burnt() {
    use super::star::{Member, Path};
    use crate::stellar::sse::main_sequence_lifetime;

    let (age, dt, tau) = (5.0e7, 1.0e3, 0.6);
    let mut engine = fed_engine(carried_helium_star(1.0, tau, age), age, 0.02);
    let ctx = std::sync::Arc::clone(&engine.ctx);
    let lifetime = |helium: bool, m: f64| main_sequence_lifetime(ctx.coeffs(), helium, m);
    let (m0, m1) = (1.0, 1.01);
    let helium = engine.rejuvenated_tau(0, tau, m0, m1, dt);
    let kept = tau + dt / lifetime(true, m1);
    assert!((helium - kept).abs() <= 1e-15, "{helium} against {kept}");
    for (m0, m1, mixed) in [(3.0, 3.03, true), (1.0, 1.01, false), (0.3, 0.303, true)] {
        engine.members[0] = Member::MainSequence {
            helium: false,
            mass: Path::starting(age, m0),
            tau: Path::starting(age, tau),
        };
        let got = engine.rejuvenated_tau(0, tau, m0, m1, dt);
        let start = if mixed { tau * m0 / m1 } else { tau };
        let expected = start + dt / lifetime(false, m1);
        assert!(
            (got - expected).abs() <= 1e-15,
            "a hydrogen accretor from {m0} to {m1} M☉: {got} against {expected}"
        );
    }
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06, §3.3 A4): a swelled giant's built track has the
/// core asked for at its placement, to 10⁻⁹ of it, or the placement is refused. Core-helium-burning
/// stars at y = 0.1, 0.5 and 0.9 with cores of 0.8–8 M☉ and thermally pulsing AGB stars with
/// carbon–oxygen cores of 0.55–1.3 M☉, at Z = 0.02 and 0.004. `placed_star`, which every other
/// caller keeps (finding F16), misses by up to −8.4% over them (core-helium-burning at y = 0.1,
/// 8 M☉, Z = 0.02, the one case refused: its exact ask lies at about 1.108 times the core, past
/// the bracket's 1.1); the miss is printed, not asserted, being F16's.
#[test]
fn the_swells_placement_is_exact() {
    use crate::stellar::sse::NewStar;

    let age = 5.0e7;
    let mut placed = 0;
    let mut refused = Vec::new();
    let mut worst_miss: f64 = 0.0;
    for z in [0.02, 0.004] {
        let engine = fed_engine(carried_helium_star(1.0, 0.5, age), age, z);
        let mut cases: Vec<(NewStar, f64)> = Vec::new();
        for y in [0.1, 0.5, 0.9] {
            for core in [0.8, 1.5, 2.5, 4.0, 8.0] {
                cases.push((NewStar::CoreHeliumBurning { burnt: y }, core));
            }
        }
        for core in [0.55, 0.7, 1.0, 1.3] {
            cases.push((NewStar::PulsingAgb, core));
        }
        for (target, core) in cases {
            let m = core * (1.0 + 1e-4);
            if let Some(old) = engine.placed_star(0, target, m, core) {
                worst_miss = worst_miss.min(core_of(&engine, &old, age, m) / core - 1.0);
            }
            match engine.swelled_star(0, target, m, core) {
                Some(member) => {
                    let built = core_of(&engine, &member, age, m);
                    assert!(
                        (built - core).abs() <= 1e-9 * core,
                        "{target:?} of {core} M☉ at Z = {z}: built {built}"
                    );
                    assert!(
                        member.mass_at(age).total_cmp(&m).is_eq(),
                        "{target:?} of {core} M☉ at Z = {z}: a mass of {} for {m}",
                        member.mass_at(age)
                    );
                    placed += 1;
                }
                None => refused.push(format!("{target:?} of {core} M☉ at Z = {z}")),
            }
        }
    }
    eprintln!(
        "{placed} placed exactly, {} refused {refused:?}; placed_star's worst miss {worst_miss:.4}",
        refused.len()
    );
    assert!(
        refused.len() * 10 <= placed,
        "{} of {} refused: {refused:?}",
        refused.len(),
        placed + refused.len()
    );
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06, §3.4): pinned pair 0255 (20.79 + 20.51 M☉ at
/// 969 d, Z = 0.02). Its primary, stripped to a 5.53 M☉ helium star, is fed hydrogen by its
/// main-sequence companion faster than its wind takes mass and than a hydrogen shell on its core
/// would burn it: at 8.6114 Myr it swells into a core-helium-burning star whose core is its helium
/// mass at the step's start, 5.530251 M☉, to 10⁻⁹, whose envelope is the step's net gain,
/// 3.19 × 10⁻⁴ M☉, to 10⁻⁹ of its mass, whose fraction burnt is the helium star's τ, and which
/// stays compact (under 1 R☉, HPT's small-envelope perturbation). Before P11.T4.m it swelled at
/// 8.6095 Myr into a 1,076 R☉ giant around its whole mass, with nothing accreted.
#[test]
fn a_fed_helium_star_swells_around_its_own_core() {
    let input = pinned_sample_pair(255);
    let timeline = evolve(&input, Years::new(1.2e10));
    let swells = swells_of(&timeline);
    let swell = swells
        .iter()
        .find(|s| s.slot == 0 && s.from == Phase::HeliumMainSequence)
        .unwrap_or_else(|| panic!("the primary swells: {}", describe(&timeline)));
    let [before, after] = swell.masses;
    let (m, core) = (swell.giant.mass().value(), swell.giant.core_mass().value());
    let tau = swell
        .tau
        .expect("a helium main-sequence star the binary carries");
    eprintln!(
        "pair 0255 swells at {} yr, fed {:e} M☉/yr over {} yr: core {core} M☉, envelope {:e} M☉, \
         y {} against τ {tau}, R {} R☉",
        swell.age,
        (after - before) / swell.step,
        swell.step,
        m - core,
        swell.burnt,
        swell.giant.radius().value()
    );
    assert!((swell.age / 8.6114e6 - 1.0).abs() < 1e-4, "{swell:?}");
    assert_eq!(swell.giant.phase(), Phase::CoreHeliumBurning, "{swell:?}");
    assert!((before - 5.530_251).abs() < 1e-6, "{swell:?}");
    assert!((core - before).abs() <= 1e-9 * before, "{swell:?}");
    assert!(
        ((m - core) - (after - before)).abs() <= 1e-9 * m,
        "the envelope is the step's net gain: {swell:?}"
    );
    assert!((after - before - 3.19e-4).abs() < 0.01e-4, "{swell:?}");
    assert!((swell.burnt - tau).abs() <= 1e-9, "{swell:?}");
    assert!(swell.giant.radius().value() < 1.0, "{swell:?}");
}

/// P11.T4.m (ruling p11-c2-swell of 2026-10-06, §3.1 item 3): a white dwarf fed hydrogen at
/// 2.71 × 10⁻⁷ M☉ yr⁻¹ or more swells into a giant around itself. In pinned pair 0118 (2.64 +
/// 2.42 M☉ at 605 d, Z = 0.004) the primary's 0.715 M☉ carbon–oxygen dwarf becomes a thermally
/// pulsing AGB star whose core is the dwarf's mass at the step's start, to 10⁻⁹, and whose
/// envelope is the step's net gain. In pinned pair 0305 (3.30 + 2.22 M☉ at 76.9 d), whose dwarf
/// was swelled bare before P11.T4.m, kept everything and grew to a 1.77 M☉ carbon–oxygen dwarf
/// that collapsed at 526.2 Myr, the dwarf swells around its own 0.8036 M☉ at 532.19 Myr, fed by
/// its companion's Hertzsprung gap, and no carbon–oxygen dwarf above the Chandrasekhar mass is
/// shown. (Between P11.T4.m and P11.T4.o it swelled at 526.23 Myr around 1.2745 M☉, which one
/// 1.57 Myr step of 0.49 M☉ had brought it to: finding F17.)
#[test]
fn a_fed_white_dwarf_swells_around_itself() {
    use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;

    let until = Years::new(1.2e10);
    for (i, dwarf) in [(118, 0.715_027), (305, 0.803_613)] {
        let input = pinned_sample_pair(i);
        let timeline = evolve(&input, until);
        let swells = swells_of(&timeline);
        let swell = swells
            .iter()
            .find(|s| s.from == Phase::CarbonOxygenWhiteDwarf)
            .unwrap_or_else(|| panic!("pair {i:04}'s dwarf swells: {}", describe(&timeline)));
        let [before, after] = swell.masses;
        let (m, core) = (swell.giant.mass().value(), swell.giant.core_mass().value());
        assert!((before - dwarf).abs() < 1e-6, "pair {i:04}: {swell:?}");
        assert!(
            (after - before) / swell.step >= 2.71e-7,
            "pair {i:04}: {swell:?}"
        );
        assert_eq!(
            swell.giant.phase(),
            Phase::ThermallyPulsingAgb,
            "pair {i:04}: {swell:?}"
        );
        assert!(
            (core - before).abs() <= 1e-9 * before,
            "pair {i:04}: {swell:?}"
        );
        assert!(
            ((m - core) - (after - before)).abs() <= 1e-9 * m,
            "pair {i:04}: the envelope is the step's net gain: {swell:?}"
        );
        let mut ages: Vec<f64> = Vec::new();
        for segment in timeline.segments() {
            ages.extend([segment.start().value(), segment.end().value()]);
            for member in segment.members() {
                if let super::star::Member::Remnant { mass, .. } = member {
                    ages.extend(mass.knots().iter().map(|knot| knot[0]));
                }
            }
        }
        for age in ages.into_iter().filter(|&age| age < until.value()) {
            for star in timeline.state_at(Years::new(age)).stars() {
                assert!(
                    star.phase() != Phase::CarbonOxygenWhiteDwarf
                        || star.mass() <= CHANDRASEKHAR_MASS,
                    "pair {i:04}: a {:?} carbon–oxygen dwarf at {age} yr",
                    star.mass()
                );
            }
        }
    }
}

// --- P11.T4.o: the transfer loop's steps -------------------------------------------------------

/// The transfer steps this thread has started from a donor with nothing living
/// (`rlof::DEAD_DONOR_STEPS`, `Engine::transfer_phase`'s backstop).
fn dead_donor_steps() -> u32 {
    super::rlof::DEAD_DONOR_STEPS.with(core::cell::Cell::get)
}

/// The timeline of `input` to `until`, asserting that no transfer step of it started from a donor
/// with nothing living (P11.T4.o).
fn evolve_from_living_donors(input: &BinaryInput, until: Years) -> BinaryTimeline {
    let dead = dead_donor_steps();
    let timeline = evolve(input, until);
    assert_eq!(
        dead_donor_steps(),
        dead,
        "a transfer step from a dead donor: {:?} P {} d\n{}",
        input.masses(),
        input.orbit().period().value() / 86_400.0,
        describe(&timeline)
    );
    timeline
}

/// The share of its donor's mass that each accepted step of stable transfer in `timeline` moved:
/// each knot of a transfer segment's rates is a step's end and its mean rate there, and the
/// donor's mass path holds its mass at the step's start.
fn transfer_shares(timeline: &BinaryTimeline) -> Vec<(f64, f64)> {
    let mut shares = Vec::new();
    for segment in timeline.segments() {
        let SegmentKind::StableTransfer { donor } = segment.kind() else {
            continue;
        };
        let (_, Some(rates)) = segment.paths() else {
            continue;
        };
        let mut start = segment.start().value();
        for &[age, rate] in rates.knots() {
            let donor_mass = segment.members()[donor.index()].mass_at(start);
            if positive(donor_mass) && age > start {
                shares.push((age, rate * (age - start) / donor_mass));
            }
            start = age;
        }
    }
    shares
}

/// P11.T4.o, finding F17 (ruling p11-c2-swell of 2026-10-06, §4.4): a step of transfer that moves
/// more than 2 × BSE equation 92's 0.5% of its donor is taken again at half its length. In pinned
/// pair 0305 (3.30 + 2.22 M☉ at 76.9 d) one 1.57 Myr step, which a slower step's longer hint had
/// started, moved 0.49 M☉ onto its white dwarf at 526.23 Myr, 27% of its donor; no step of the
/// pair moves more than 1% now.
#[test]
fn a_transfer_step_moves_at_most_a_hundredth_of_its_donor() {
    let input = pinned_sample_pair(305);
    let timeline = evolve(&input, Years::new(1.2e10));
    let shares = transfer_shares(&timeline);
    assert!(shares.len() > 100, "{}", describe(&timeline));
    for (age, share) in shares {
        assert!(share <= 0.01, "{share} of the donor at {age} yr");
    }
}

/// P11.T4.o, finding F18: a fed accretor that does not age over the step aimed at its main
/// sequence's end, its rejuvenation at or above its ageing, keeps its main sequence, and the step
/// closes no segment. In sample 17 of D's cell 4783 (5.62 + 4.41 M☉ at 6.72 d, under a seed of
/// P11.T4.o's probe, the fit's with its last byte changed) the gainer, 8.98 M☉ near its end, is fed so on three such steps in a row from
/// 97.70 Myr; each ends inside its segment, with the gainer still on its main sequence.
#[test]
fn a_fed_accretor_that_does_not_age_keeps_its_main_sequence() {
    use super::star::Member;

    let grid = pair_light::PairGrid::of(crate::id::Layer::D).expect("D's grid");
    let input = pair_light::sample_input(&grid, 4_783, 17, pair_light::FIT_SEED ^ 0x0d);
    let timeline = evolve(&input, Years::new(pair_light::LAST_AGE_YEARS));
    let coeffs = crate::stellar::sse::ZCoeffs::new(input.composition().z_fit());
    let mut kept = 0;
    for segment in timeline.segments() {
        let SegmentKind::StableTransfer { donor } = segment.kind() else {
            continue;
        };
        let Member::MainSequence { helium, mass, tau } = &segment.members()[1 - donor.index()]
        else {
            continue;
        };
        let (masses, taus) = (mass.knots(), tau.knots());
        for k in 1..taus.len() {
            let dt = taus[k][0] - taus[k - 1][0];
            let rest = (1.0 - taus[k - 1][1])
                * crate::stellar::sse::main_sequence_lifetime(&coeffs, *helium, masses[k - 1][1]);
            let aimed = dt > 0.0 && (dt - rest).abs() <= 1e-9 * rest;
            if aimed && masses[k][1] > masses[k - 1][1] && taus[k][1] <= taus[k - 1][1] {
                assert!(
                    k + 1 < taus.len(),
                    "the step ends its segment: {}",
                    describe(&timeline)
                );
                kept += 1;
            }
        }
    }
    assert_eq!(kept, 3, "{}", describe(&timeline));
}

/// P11.T4.o, finding F18: an unfed star's main sequence ends where it did before T4.o, to 10⁻⁹ of
/// the age. A carried main sequence that is not fed lands short of its end only as its own wind
/// lengthens its lifetime, and the phase goes on to land on it, closing no segment at the short
/// landing (it closed one there before). Pinned pairs 0004, 0005, 0010 and 0015, each companion
/// left on its own by its primary's death, have one, one, one and two segments fewer. From
/// generator version 26 pair 0004's history moves: its companion's main sequence ends at
/// 10.1938 Myr (10.1959 Myr until 26), in 13 segments (10); the other three are as they were.
#[test]
fn an_unfed_main_sequence_ends_where_it_did() {
    use super::star::Member;

    for (i, end, segments) in [
        (4, 1.019_375_460_369_789_2e7, 13),
        (5, 1.744_360_084_346_238_4e8, 15),
        (10, 7.287_576_551_262_443e7, 15),
        (15, 1.269_029_482_407_306e7, 14),
    ] {
        let timeline = evolve(&pinned_sample_pair(i), Years::new(1.2e10));
        let left = timeline
            .segments()
            .windows(2)
            .find(|w| {
                matches!(w[0].members()[1], Member::MainSequence { .. })
                    && matches!(w[1].members()[1], Member::Shaped { .. })
            })
            .unwrap_or_else(|| panic!("pair {i:04}'s companion: {}", describe(&timeline)))[1]
            .start()
            .value();
        assert!(
            (left - end).abs() <= 1e-9 * end,
            "pair {i:04}: {left} yr against {end} yr"
        );
        assert_eq!(timeline.segments().len(), segments, "pair {i:04}");
    }
}

/// P11.T4.o, the engine finding of "P11.T17.b as built": the fit's sample 49 of E's cell 3579
/// (32.75 + 31.83 M☉ at [Fe/H] −1.98, circular at 65.8 R☉). The secondary's common envelope leaves
/// a 19.9 M☉ helium star, which gives mass to the primary's black hole until its helium main
/// sequence ends at 8.5057 Myr, and plan 06's helium star of that mass has no Hertzsprung gap: the
/// end is its death. It collapses there (BSE section 2.5) to a 19.14 M☉ black hole, bound beside
/// the 12.51 M☉ one, which no transfer step follows. Before T4.o it was left at its remnant with
/// nothing acted on, the next transfer step started from it (a debug assertion), and a release
/// build merged the pair into one 31.65 M☉ black hole at once.
#[test]
fn a_helium_donor_whose_main_sequence_end_is_its_death_collapses() {
    let grid = pair_light::PairGrid::of(crate::id::Layer::E).expect("E's grid");
    let input = pair_light::sample_input(&grid, 3_579, 49, pair_light::FIT_SEED);
    let timeline = evolve_from_living_donors(&input, Years::new(pair_light::LAST_AGE_YEARS));
    let record = *timeline
        .supernovae()
        .iter()
        .find(|s| s.component() == Component::Secondary)
        .unwrap_or_else(|| panic!("the secondary's collapse: {}", describe(&timeline)));
    let death = record.age().value();
    assert!((death / 8.5057e6 - 1.0).abs() < 1e-4, "{death}");
    assert_eq!(
        record.remnant().kind(),
        crate::stellar::remnant::RemnantKind::BlackHole
    );
    assert!(
        (record.remnant().mass().value() - 19.14).abs() < 0.01,
        "{record:?}"
    );
    assert!(record.bound(), "{}", describe(&timeline));
    let before = timeline.state_at(Years::new(super::detached::last_living(death)));
    assert_eq!(before.stars()[1].phase(), Phase::HeliumMainSequence);
    let after = timeline.state_at(Years::new(death * (1.0 + 1e-6)));
    assert_eq!(
        after.kind(),
        SegmentKind::Detached,
        "{}",
        describe(&timeline)
    );
    assert_eq!(after.stars().map(|s| s.phase()), [Phase::BlackHole; 2]);
}

/// P11.T4.o: a transfer step that would start from a donor with nothing living, its death inside
/// the last step not landed on, acts on the death instead (the backstop of
/// `Engine::transfer_phase`): a 19.5 M☉ helium star placed at its own death beside a 12.5 M☉
/// black hole collapses, as at `Stop::Death`, and the transfer ends.
#[test]
fn a_transfer_from_a_dead_donor_acts_on_its_death() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit, offset_for};
    use super::star::{Member, Path};
    use super::timeline::Context;

    let age = 8.5e6;
    let input = pair(32.75, 31.83, 7.7, 0.0, 2.1e-4);
    let ctx = Arc::new(Context::of(&input));
    let track = crate::stellar::sse::Track::helium_star_full(
        SolarMasses::new(19.5),
        ctx.composition(),
        ctx.draws(1),
    );
    let lifetime = track.lifetime().expect("a helium star dies").value();
    let donor = Member::Shaped {
        track: Arc::new(track),
        offset: offset_for(age, lifetime),
        mass: Path::starting(age, 19.5),
    };
    let hole = Member::Remnant {
        phase: Phase::BlackHole,
        birth: 6.7e6,
        origin: crate::units::Megayears::ZERO,
        mass: Path::starting(age, 12.5),
    };
    let k = input.orbit();
    let orbit = LiveOrbit::new(age, 60.0, 0.0, *k.orientation(), k.mean_anomaly_at_epoch());
    let mut engine = Engine::new(ctx, age, 1.0e10, [hole, donor], orbit, None);
    engine.segment_start = age;
    engine.kind = SegmentKind::StableTransfer {
        donor: Component::Secondary,
    };
    let dead = dead_donor_steps();
    engine.transfer_phase(1);
    assert_eq!(dead_donor_steps(), dead + 1);
    let record = engine
        .supernovae
        .iter()
        .find(|s| s.component() == Component::Secondary)
        .expect("the donor's collapse");
    assert!(
        (record.age().value() - age).abs() <= 1e-9 * age,
        "{record:?}"
    );
    assert!(
        !matches!(engine.kind, SegmentKind::StableTransfer { .. }),
        "{:?}",
        engine.kind
    );
}

/// P11.T4.o, finding F18: [`super::detached::main_sequence_end_landed`]'s four cases, on member 1
/// of a step from τ = 0.999: any other stop passes as it is; a step that reaches τ = 1 takes the
/// end; a fed member that ages takes it early, with τ set to 1; a fed member that does not age,
/// and an unfed one that falls short, land on nothing, their τ as the step left it.
#[test]
fn a_main_sequence_end_is_taken_where_reached_or_fed_and_aged() {
    use super::detached::{Snapshot, Stop, main_sequence_end_landed};

    let start = Snapshot {
        age: 1.0e8,
        j: 1.0,
        e: 0.0,
        spins: [1.0; 2],
        masses: [1.0, 8.0],
        taus: [0.5, 0.999],
    };
    let end = |mass: f64, tau: f64| Snapshot {
        age: 1.01e8,
        masses: [1.0, mass],
        taus: [0.5, tau],
        ..start
    };
    let ends = Some(Stop::MainSequenceEnd(1));
    for (stop, mass, tau, landed, tau_after) in [
        (
            Some(Stop::Boundary),
            8.0,
            0.9995,
            Some(Stop::Boundary),
            0.9995,
        ),
        (ends, 7.99, 1.0, ends, 1.0),
        (ends, 8.01, 0.9995, ends, 1.0),
        (ends, 8.01, 0.9985, None, 0.9985),
        (ends, 7.99, 0.9995, None, 0.9995),
    ] {
        let mut next = end(mass, tau);
        assert_eq!(
            main_sequence_end_landed(stop, &start, &mut next),
            landed,
            "{stop:?} to {mass} M☉ at τ {tau}"
        );
        assert_eq!(
            hyperion_testkit::float::bits(next.taus[1]),
            hyperion_testkit::float::bits(tau_after),
            "{stop:?} to {mass} M☉ at τ {tau}: τ {}",
            next.taus[1]
        );
    }
}

/// P11.T4.o, the dead donor's cause in a detached pair: a carried 19.5 M☉ helium main-sequence star
/// at Z = 2.1 × 10⁻⁴, on a wide orbit beside a 12.5 M☉ black hole, whose helium-star track has no
/// Hertzsprung gap, dies at its main sequence's end: a supernova record there, and a remnant on its
/// own track from it. Before T4.o `Engine::after_boundary` placed it at its track's death and acted
/// on nothing, so that it was shown as a remnant of the carried mass with no record.
#[test]
fn a_detached_helium_star_whose_main_sequence_end_is_its_death_collapses() {
    use std::sync::Arc;

    use super::evolve::{Engine, LiveOrbit};
    use super::star::{Member, Path};
    use super::timeline::Context;

    let age = 8.0e6;
    let input = pair(32.75, 31.83, 7.7, 0.0, 2.1e-4);
    let ctx = Arc::new(Context::of(&input));
    let star = Member::MainSequence {
        helium: true,
        mass: Path::starting(age, 19.5),
        tau: Path::starting(age, 0.999),
    };
    let hole = Member::Remnant {
        phase: Phase::BlackHole,
        birth: 6.7e6,
        origin: crate::units::Megayears::ZERO,
        mass: Path::starting(age, 12.5),
    };
    let k = input.orbit();
    let orbit = LiveOrbit::new(age, 2.0e4, 0.0, *k.orientation(), k.mean_anomaly_at_epoch());
    let mut engine = Engine::new(ctx, age, 1.0e10, [hole, star], orbit, None);
    engine.segment_start = age;
    for _ in 0..8 {
        if !engine.supernovae.is_empty() {
            break;
        }
        engine.detached_phase();
    }
    let record = engine
        .supernovae
        .iter()
        .find(|s| s.component() == Component::Secondary)
        .expect("the helium star's collapse at its main sequence's end");
    assert!(record.age().value() > age, "{record:?}");
    assert!(
        matches!(&engine.members[1], Member::Track { .. }),
        "{:?}",
        engine.members[1].form()
    );
}

/// P11.T4.o (the determinism audit): a timeline run to just past each of its events is the full
/// run's to there, bit for bit, for pairs that reach each new path: pinned pairs 0305 and 0171
/// (F17's halvings, 0171's eight), the fed accretor of D's cell 4783 that does not age over its
/// steps aimed at its main sequence's end (F18), and E's cell 3579 sample 49, whose helium donor
/// dies at its main sequence's end (`Engine::after_boundary`).
#[test]
fn the_steps_of_t4o_are_the_same_whatever_age_they_are_run_to() {
    let grid = |layer| pair_light::PairGrid::of(layer).expect("a grid");
    let inputs = [
        pinned_sample_pair(305),
        pinned_sample_pair(171),
        pair_light::sample_input(
            &grid(crate::id::Layer::D),
            4_783,
            17,
            pair_light::FIT_SEED ^ 0x0d,
        ),
        pair_light::sample_input(&grid(crate::id::Layer::E), 3_579, 49, pair_light::FIT_SEED),
    ];
    for input in &inputs {
        let [after_events, _] = check_after_events(input, Years::new(1.5e10));
        assert!(after_events > 0, "{:?}", input.masses());
    }
}

// --- Version 25's wiring: the held floor's drop ------------------------------------------------

/// Version 25's wiring drops P11.T17.c's held floor (`decision-galaxy-25-train.md`, "The held
/// floor"), on the check research-leans-2 asked for: the one capped timeline that P11.T4.o's
/// re-run of the follow-up's probes left, sample 11 of D's cell 2027 under the probe's seed (the
/// fit's with its last byte changed; 3.68 + 2.76 M☉ at 108.2 d, e 0.90, Z 0.0142), ends as it
/// does with the engine's cap on segments raised.
///
/// A common envelope at 275.38 Myr leaves a 0.61 M☉ helium star beside a 1.03 M☉ one at 0.72 R☉.
/// The heavier fills its lobe at 290.22 Myr and, shrinking as it loses mass and as it ages, gives
/// mass in 24 spells to 294.80 Myr, detaching after each until the orbit's decay brings its lobe
/// back to it, a segment each. Where its helium main sequence ends, the pair's second common
/// envelope merges it at 294.85 Myr, in the 64th segment, where the cap stops the engine. That
/// segment holds the merger's 1.63 M☉ helium Hertzsprung-gap star on its own track, no star the
/// binary carries on its main sequence, and the track makes it a 1.44 M☉ oxygen–neon white dwarf at
/// 295.58 Myr. With the cap raised the run takes that birth as a 65th segment and stops there. Up
/// to the cap the two runs are the same segment for segment, and from the birth on they hold the
/// same states, with the same records and merger. No companion is left whose gravitational-wave
/// merger could lie past the cap.
///
/// Between the cap and the birth, 0.72 Myr, both hold the star living, but the capped run's keeps
/// its mass at the cap, 1.63 M☉, where the raised run's wind takes it to 1.44 M☉ and strips it in
/// its last 2 × 10³ years. The raised run's star is brightest at M<sub>V</sub> −7.51, the capped
/// run's at −7.28: 0.23 mag fainter, within the 0.3 mag by which the pair-light tables brighten
/// what their walk reads ([`MARGIN_MAG`](pair_light::MARGIN_MAG)).
#[test]
fn the_last_capped_sample_ends_as_it_does_with_the_cap_raised() {
    use super::star::Member;

    let grid = pair_light::PairGrid::of(crate::id::Layer::D).expect("D's grid");
    let (cell, index) = (2_027, 11);
    let input = pair_light::sample_input(&grid, cell, index, pair_light::FIT_SEED ^ 0x0d);
    let until = Years::new(pair_light::LAST_AGE_YEARS);
    let capped = evolve(&input, until);
    let raised = super::evolve::evolve_with_segment_cap(&input, until, 64 * MAX_SEGMENTS);
    assert!(capped.hit_segment_cap(), "{}", describe(&capped));
    assert!(!raised.hit_segment_cap(), "{}", describe(&raised));
    assert_eq!(capped.segments().len(), MAX_SEGMENTS);
    assert_eq!(
        raised.segments().len(),
        MAX_SEGMENTS + 1,
        "{}",
        describe(&raised)
    );
    let (last, before) = capped.segments().split_last().expect("segments");
    assert_eq!(before, &raised.segments()[..MAX_SEGMENTS - 1]);
    let [merged, birth] = [MAX_SEGMENTS - 1, MAX_SEGMENTS].map(|k| &raised.segments()[k]);
    let cap = last.start();
    assert_eq!((merged.kind(), merged.start()), (last.kind(), cap));
    assert!((cap.value() / 294.853e6 - 1.0).abs() < 1e-5, "{cap:?}");
    // The merger's star, on the same track at the same offset in both runs, its companion gone.
    assert_eq!(last.members()[0].track(), merged.members()[0].track());
    assert!(matches!(last.members()[0], Member::Shaped { .. }));
    assert!(
        matches!(last.members()[1], Member::Gone) && matches!(merged.members()[1], Member::Gone)
    );
    let born = birth.start().value();
    assert!((born / 295.575e6 - 1.0).abs() < 1e-5, "{born}");
    let v = |state: crate::stellar::StarState| {
        crate::stellar::photometry::absolute_v_of_state(&state)
            .map_or(f64::INFINITY, crate::units::Magnitudes::value)
    };
    let mut brightest = [f64::INFINITY; 2];
    for k in 0..20_000 {
        let age = Years::new(cap.value() + (born - cap.value()) * f64::from(k) / 20_000.0);
        let (held, run) = (
            capped.state_at(age).stars()[0],
            raised.state_at(age).stars()[0],
        );
        assert!(
            held.phase().is_living() && run.phase().is_living(),
            "at {age:?}"
        );
        assert!(held.mass().value() >= run.mass().value(), "at {age:?}");
        brightest = [brightest[0].min(v(held)), brightest[1].min(v(run))];
    }
    assert!(
        brightest[0] <= brightest[1] + pair_light::MARGIN_MAG,
        "the brightest V, capped and raised: {brightest:?}"
    );
    for age in [born, f64::midpoint(born, until.value()), until.value()] {
        let age = Years::new(age);
        assert_eq!(capped.state_at(age), raised.state_at(age), "at {age:?}");
    }
    assert_eq!(capped.supernovae(), raised.supernovae());
    assert_eq!(capped.pooled_ia(), raised.pooled_ia());
    assert_eq!(capped.merger_age(), raised.merger_age());
    assert!(
        raised.merger_age().is_some_and(|m| m <= cap),
        "{:?}",
        raised.merger_age()
    );
    let end = raised.state_at(until);
    assert_eq!(end.orbit(), None);
    assert_eq!(
        end.stars().map(|s| s.phase()),
        [Phase::OxygenNeonWhiteDwarf, Phase::NoRemnant]
    );
}

// --- P11.T4: the dwarf limit (held at 27) ------------------------------------------------------

/// The version's rules with P11.T4's dwarf-limit arm on alone, so that the other 27 holds move
/// nothing it holds equal (`stellar/rules/dwarf_limit_hold.rs`).
fn dwarf_limit_rules() -> crate::stellar::rules::StellarRules {
    crate::stellar::rules::StellarRules {
        dwarf_limit: super::dwarf_limit::DwarfLimitRules::Chandrasekhar,
        ..crate::stellar::rules::StellarRules::of_version(crate::GENERATOR_VERSION)
    }
}

/// The state of `timeline` at the start of its first merged segment, and that age, years.
fn merged_state(timeline: &BinaryTimeline) -> (BinaryState, f64) {
    let merged = timeline
        .segments()
        .iter()
        .find(|s| s.kind() == SegmentKind::Merged)
        .unwrap_or_else(|| panic!("the pair merges: {}", describe(timeline)));
    let age = merged.start().value();
    (timeline.state_at(Years::new(age)), age)
}

/// P11.T4's dwarf limit: a merger's product around a carbon–oxygen core that no star of its kind
/// can hold leaves the engine's own remnant only under the arm. Pinned pairs 0252 and 0902 merge
/// into a thermally pulsing AGB product around a carbon–oxygen core of 1.765 and 1.948 M☉, which
/// the held arm shows as a carbon–oxygen dwarf of that mass to 12 Gyr and the arm collapses at
/// once to the 1.26 M☉ neutron star of an accretion-induced collapse. Under the arm the same pair
/// twice gives the same timeline.
#[test]
fn an_unplaced_carbon_oxygen_core_collapses_at_the_chandrasekhar_mass() {
    use super::testing::shows_a_dwarf_above_the_limit;
    use crate::stellar::remnant::RemnantKind;

    let until = 1.2e10;
    let held_rules = crate::stellar::rules::StellarRules::of_version(crate::GENERATOR_VERSION);
    let run =
        |input: &BinaryInput, rules| evolve(&input.clone().with_rules(rules), Years::new(until));
    for (name, input, core) in [
        ("0252", pinned_sample_pair(252), 1.764_8),
        ("0902", pinned_sample_pair(902), 1.947_7),
    ] {
        let (held, arm) = (run(&input, held_rules), run(&input, dwarf_limit_rules()));
        let (state, age) = merged_state(&held);
        assert_eq!(
            state.stars()[0].phase(),
            Phase::CarbonOxygenWhiteDwarf,
            "{name}"
        );
        assert!(
            (state.stars()[0].mass().value() - core).abs() < 1e-4,
            "{name}: {state:?}"
        );
        assert!(shows_a_dwarf_above_the_limit(&held, until), "{name}");
        assert!(
            !shows_a_dwarf_above_the_limit(&arm, until),
            "{name}: {}",
            describe(&arm)
        );
        let collapse = arm
            .supernovae()
            .iter()
            .find(|s| (s.age().value() - age).abs() <= 1e-9 * age)
            .unwrap_or_else(|| {
                panic!(
                    "{name}'s product collapses at the merger: {}",
                    describe(&arm)
                )
            });
        assert_eq!(
            collapse.remnant().kind(),
            RemnantKind::NeutronStar,
            "{name}"
        );
        assert_eq!(
            collapse.remnant(),
            crate::stellar::remnant::collapse::electron_capture_remnant(),
            "{name}"
        );
        assert_eq!(run(&input, dwarf_limit_rules()), arm, "{name}");
    }
}

/// P11.T4's dwarf limit: a merger's product around a helium core that no star of its kind can hold
/// leaves the engine's own remnant only under the arm. Pinned pair 0438 (13.7 + 3.7 M☉ at 16.7 d)
/// and a 108 + 78 M☉ pair at 8.67 d merge into a core-helium-burning product around a helium core
/// of 3.22 and 28.6 M☉, which the held arm shows as a carbon–oxygen dwarf and the arm makes the
/// naked helium star of that core, which then dies by plan 06; pinned pair 0787's product around
/// a 0.457 M☉ carbon–oxygen core, a helium dwarf under the held arm, is a carbon–oxygen one under
/// the arm.
#[test]
fn an_unplaced_helium_core_is_its_helium_star() {
    use super::testing::shows_a_dwarf_above_the_limit;

    let until = 1.2e10;
    let held_rules = crate::stellar::rules::StellarRules::of_version(crate::GENERATOR_VERSION);
    let run =
        |input: &BinaryInput, rules| evolve(&input.clone().with_rules(rules), Years::new(until));
    // The 108 + 78 M☉ pair and pinned pair 0787 merge so under every 27 hold alone (P11.T4's
    // pinned white dwarfs, folded, hold a product its primary's pin is pending for at its core's
    // last living state; both arms set that hold back while it was held).
    let every_27 = crate::stellar::rules::StellarRules::every_27_hold();
    let every_27_held = crate::stellar::rules::StellarRules {
        dwarf_limit: super::dwarf_limit::DwarfLimitRules::Unchecked,
        ..every_27
    };
    for (name, input, core, [held_rules, arm_rules]) in [
        (
            "0438",
            pinned_sample_pair(438),
            3.224_5,
            [held_rules, dwarf_limit_rules()],
        ),
        (
            "108 + 78",
            pair(108.0, 78.0, 8.67, 0.0, 0.02),
            28.614,
            [every_27_held, every_27],
        ),
    ] {
        let (held, arm) = (run(&input, held_rules), run(&input, arm_rules));
        let (state, _) = merged_state(&held);
        assert_eq!(
            state.stars()[0].phase(),
            Phase::CarbonOxygenWhiteDwarf,
            "{name}"
        );
        assert!(
            (state.stars()[0].mass().value() - core).abs() < 1e-3,
            "{name}: {state:?}"
        );
        assert!(
            !shows_a_dwarf_above_the_limit(&arm, until),
            "{name}: {}",
            describe(&arm)
        );
        let (state, age) = merged_state(&arm);
        let star = state.stars()[0];
        assert_eq!(
            star.phase(),
            Phase::HeliumMainSequence,
            "{name}: {}",
            describe(&arm)
        );
        assert!(
            (star.mass().value() - core).abs() < 1e-3,
            "{name}: {star:?}"
        );
        let death = arm
            .supernovae()
            .iter()
            .find(|s| s.age().value() > age)
            .unwrap_or_else(|| panic!("{name}'s helium star dies: {}", describe(&arm)));
        println!("{name}: a {core} M☉ helium star at {age} yr, then {death:?}");
    }
    // Pinned pair 0787 merges so under every 27 hold alone.
    let input = pinned_sample_pair(787);
    let (held, arm) = (run(&input, every_27_held), run(&input, every_27));
    let ((held, _), (arm, _)) = (merged_state(&held), merged_state(&arm));
    assert_eq!(held.stars()[0].phase(), Phase::HeliumWhiteDwarf);
    assert_eq!(arm.stars()[0].phase(), Phase::CarbonOxygenWhiteDwarf);
    assert_eq!(arm.stars()[0].mass(), held.stars()[0].mass());
    assert!(
        (arm.stars()[0].mass().value() - 0.457_2).abs() < 1e-4,
        "{arm:?}"
    );
}

/// P11.T4's dwarf limit: a carbon–oxygen dwarf fed helium past the edge-lit detonation's 0.15 M☉
/// collapses at the Chandrasekhar mass only under the arm. Two pairs whose primary's dwarf is fed
/// by its companion's naked helium star after a common envelope (5.43 + 1.69 M☉ at 7 d and 6.6 +
/// 1.65 M☉ at 3.4 d, solar) show a dwarf above the Chandrasekhar mass under the held arm; under
/// the arm none, and the dwarf collapses to the 1.26 M☉ neutron star of an accretion-induced
/// collapse, its edge-lit candidate pooled first.
#[test]
fn a_dwarf_fed_helium_collapses_at_the_chandrasekhar_mass() {
    use super::testing::shows_a_dwarf_above_the_limit;

    let until = 1.2e10;
    let held_rules = crate::stellar::rules::StellarRules::of_version(crate::GENERATOR_VERSION);
    for (m1, m2, period) in [(5.43, 1.69, 7.0), (6.6, 1.65, 3.4)] {
        let input = pair(m1, m2, period, 0.0, 0.02);
        let held = evolve(&input.clone().with_rules(held_rules), Years::new(until));
        let arm = evolve(&input.with_rules(dwarf_limit_rules()), Years::new(until));
        assert!(shows_a_dwarf_above_the_limit(&held, until), "{m1} + {m2}");
        assert!(
            !shows_a_dwarf_above_the_limit(&arm, until),
            "{m1} + {m2}: {}",
            describe(&arm)
        );
        let pooled = arm.pooled_ia().expect("the edge-lit candidate is pooled");
        assert_eq!(pooled.channel(), IaPoolChannel::Accretion);
        assert_eq!(held.pooled_ia(), Some(pooled), "{m1} + {m2}");
        let collapse = arm
            .supernovae()
            .iter()
            .find(|s| s.age() > pooled.age())
            .unwrap_or_else(|| panic!("{m1} + {m2}'s dwarf collapses: {}", describe(&arm)));
        assert_eq!(
            collapse.remnant(),
            crate::stellar::remnant::collapse::electron_capture_remnant(),
            "{m1} + {m2}"
        );
        let before = arm.state_at(Years::new(collapse.age().value() * (1.0 - 1e-12)));
        let dwarf = before.stars()[collapse.component().index()];
        assert_eq!(dwarf.phase(), Phase::CarbonOxygenWhiteDwarf, "{m1} + {m2}");
        println!(
            "{m1} + {m2}: collapses at {:?} from {:?}",
            collapse.age(),
            dwarf.mass()
        );
    }
}

/// P11.T4's dwarf limit: over the pinned thousand the arm moves exactly the pairs that show a
/// white dwarf above the Chandrasekhar mass under the held arm, which are listed: three whose
/// merger's product no star holds (0252 and 0902 around a carbon–oxygen core, 0438 around a
/// helium one).
#[test]
fn the_dwarf_limit_moves_only_the_pairs_that_reach_it() {
    use super::testing::shows_a_dwarf_above_the_limit;

    let held_rules = crate::stellar::rules::StellarRules::of_version(crate::GENERATOR_VERSION);
    let until = 1.2e10;
    let mut mix = Mix(0x0b1e_0001);
    let (mut moved, mut above) = (Vec::new(), Vec::new());
    for i in 0..1000 {
        let input = pinned_pair(&mut mix, i);
        let held = evolve(&input.clone().with_rules(held_rules), Years::new(until));
        let arm = evolve(&input.with_rules(dwarf_limit_rules()), Years::new(until));
        if !same_outcome(&held, &arm) {
            moved.push(i);
        }
        if shows_a_dwarf_above_the_limit(&held, until) {
            above.push(i);
        }
    }
    assert_eq!(moved, above);
    assert_eq!(moved, DWARF_LIMIT_MOVERS);
}

/// Whether timelines `a` and `b` hold the same segments, events and states at every segment's
/// edge, bit for bit, whatever rules their tracks were built under (their `Debug` and so their
/// [`digest`]s carry them).
fn same_outcome(a: &BinaryTimeline, b: &BinaryTimeline) -> bool {
    let edges = |t: &BinaryTimeline| -> Vec<(SegmentKind, f64, f64)> {
        t.segments()
            .iter()
            .map(|s| (s.kind(), s.start().value(), s.end().value()))
            .collect()
    };
    edges(a) == edges(b)
        && a.supernovae() == b.supernovae()
        && a.pooled_ia() == b.pooled_ia()
        && edges(a).iter().all(|&(_, start, end)| {
            [start, end.min(a.until().value())]
                .iter()
                .all(|&age| a.state_at(Years::new(age)) == b.state_at(Years::new(age)))
        })
}

/// The pinned pairs the dwarf limit moves, as measured (2026-11-02, at generator version 25).
const DWARF_LIMIT_MOVERS: [u32; 3] = [252, 438, 902];
