//! Brown-dwarf companions (plan 11, P11.T2.d, Design note 15): a system's one bound companion of
//! 13 Jupiter masses to 0.08 M☉, drawn after its stars on a stream of its own.
//!
//! The brainstorm counts brown dwarfs at "one for every four or five stars, companions included"
//! and gives the free-floating ones a layer of their own (plan 13, one for every five or six
//! stars). The bound ones are the difference, a few per hundred stars, and are drawn here:
//!
//! 1. **Whether.** One mark on `system.substellar` decides whether the system has a brown-dwarf
//!    companion, with the probability [`substellar_companion_probability`] of its primary's
//!    initial mass.
//! 2. **Its orbit.** Its period comes from the primary's own period distribution
//!    ([`MultiplicityModel::period_distribution`]) thinned below [`SUBSTELLAR_DESERT_PERIOD`] by
//!    [`SUBSTELLAR_DESERT_FACTOR`], the brown-dwarf desert. Its mass ratio continues the stellar
//!    law's lowest segment, `q^γ`, from 0.08 M☉ ÷ m₁ down to
//!    [`MIN_SUBSTELLAR_COMPANION_MASS`] ÷ m₁; its eccentricity is the stellar law's at its period,
//!    its orientation isotropic and its mean anomaly at the epoch uniform.
//! 3. **Stability.** It joins the hierarchy as a new outermost orbit about the whole system, and
//!    the hierarchy with it must pass the stellar draw's whole test (Mardling and Aarseth's
//!    criterion, the tidal cut, a forced multiple's widest separation). A try that fails is
//!    redrawn on the next words, up to [`MAX_SUBSTELLAR_TRIES`] tries, and then the companion is
//!    dropped.
//!
//! One mark picks the period's window, below or above the desert's edge, by integer thresholds on
//! their weights, and the period inside it by the mark's residual, as the stellar draw picks a
//! companion's node (`draw_hierarchy`, "Where a companion goes"). The windows are the necessary
//! conditions of the test: outside the stellar orbits by Mardling and Aarseth's smallest axis
//! ratio, inside the tidal cut. So the period is an exact draw from the thinned law conditioned on
//! passing, and the one inexact case is the cap on tries.
//!
//! The companion is the last body of the system (Design note 5) and the outer member of the new
//! root, so the stars' numbering, their draws, their masses and every stellar orbit are those of
//! the draw without it, bit for bit: adding it moves no star. It is left out of every stellar
//! quadrature (mean mass, all-stars fraction) as plan 02's stand-in left companions under 0.08 M☉
//! out, and it never enters the binary engine. Its state is plan 06's cooling fits
//! ([`substellar::cooling`](crate::stellar::substellar::cooling)), through its
//! [`StarModel`](crate::stellar::system::StarModel). A brown-dwarf primary stays single, as plan
//! 13 says, and a system of [`MultiplicityContext::ForcedSingle`] gets none.
//!
//! # Draw numbers
//!
//! Attempt n reads words 64n to 64n + 63 of `system.substellar`, keyed by the system: word 64n is
//! the decision, and try r, r = 0 to [`MAX_SUBSTELLAR_TRIES`] − 1, reads words 64n + 1 + 7r to
//! 64n + 7 + 7r: the window-and-period mark, the mass ratio, the eccentricity, the cosine of the
//! inclination, the ascending node, the argument of periapsis and the mean anomaly.

use super::dist::{MIN_COMPANION_MASS, MIN_SUBSTELLAR_COMPANION_MASS};
use super::hierarchy::{
    DRAWS_PER_ATTEMPT, MultiplicityContext, RedrawAttempt, StarIndex, SystemHierarchy,
    WINDOW_MARGIN, draft_orbit, period_of, pick_window,
};
use super::model::MultiplicityModel;
use super::stability::{Innermost, Limits, NECESSARY_AXIS_RATIO};
use crate::Seed;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::SystemId;
use crate::math;
use crate::rng::{Mark, ObjectKey, PowerLaw, Stream, Threshold, Thresholds, tags};
use crate::units::{Days, Metres, SolarMasses};

/// The period below which brown-dwarf companions are thinned, the edge of the brown-dwarf desert:
/// 10³ days, about 2 au about a Sun-like star (Design note 15; Grether and Lineweaver 2006, ApJ
/// 640, 1051, who find the desert inside periods of about five years).
pub const SUBSTELLAR_DESERT_PERIOD: Days = Days::new(1_000.0);

/// The factor by which the period distribution of brown-dwarf companions is thinned below
/// [`SUBSTELLAR_DESERT_PERIOD`]: the brown-dwarf desert (Design note 15), 0.3.
///
/// No survey states the factor; it is derived against the Sun-like period law. With 6% of
/// Sun-like primaries holding a brown dwarf and no desert, 1.3% would lie inside five years, where
/// Grether and Lineweaver (2006, ApJ 640, 1051) find under 1% (so f < 0.75) and Sahlmann et al.
/// (2011, A&A 525, A95) an upper limit of 0.6% (f ≲ 0.45); Kiefer et al.'s (2019, A&A 631, A125)
/// "at least 2%" within 10 au is what no desert gives out to about 10⁴ days, so the desert is
/// confined inside a few au. 0.3 lies in the defensible 0.2–0.5 and puts about 0.3% of Sun-like
/// primaries' brown dwarfs inside 10³ days. **Provisional**, a research agent's derivation
/// (plan 11's Risks, "Deviations in P11.T2.d, as built").
pub const SUBSTELLAR_DESERT_FACTOR: f64 = 0.3;

/// The probability that a primary has a bound brown-dwarf companion (13–80 Jupiter masses) at any
/// separation, by its initial mass: (M☉, probability), linear in ln m between anchors and constant
/// outside.
///
/// | Anchor, M☉ | p     | Measured                                                                  |
/// | ---------- | ----- | ------------------------------------------------------------------------- |
/// | 0.3        | 0.035 | M dwarfs: 2.3 (+5.0 −0.7)% at 10–70 au (Dieterich et al. 2012, AJ 144, 64) |
/// | 1.0        | 0.06  | 3.2 (+3.1 −2.7)% at 28–1,590 au (Metchev and Hillenbrand 2009, ApJS 181, 62), at least 2.0 ± 0.5% within 10 au (Kiefer et al. 2019, A&A 631, A125), and about 0.5% for the gap between |
/// | 2.5        | 0.03  | 0.8 (+0.8 −0.5)% at 10–100 au over the GPIES sample (Nielsen et al. 2019, AJ 158, 13) |
/// | 8          | 0.02  | none                                                                      |
///
/// Only the Sun-like anchor is measured over most separations; the M dwarfs' adds about 1% inside
/// 5 au and 1% beyond 100 au, and the A and B stars' are largely extrapolated. Weighted by the
/// default's primaries (67 : 12 : 17 : 3.4 : 1% by layer) they give about 0.040 companions per
/// system, 0.028 per star at 1.44 stars per system, inside the 0.02–0.06 per star that the
/// brainstorm's counts leave (one brown dwarf for every four or five stars, companions included,
/// less plan 13's one free-floating for every five or six); Kirkpatrick et al.'s (2024, ApJS 271,
/// 55) star-to-brown-dwarf ratio of 4 : 1 then leaves about one free-floating per 4.5 stars.
/// **Provisional**, from a research agent's reading of the surveys (plan 11's Risks,
/// "Deviations in P11.T2.d, as built").
pub const SUBSTELLAR_ANCHORS: [(f64, f64); 4] =
    [(0.3, 0.035), (1.0, 0.06), (2.5, 0.03), (8.0, 0.02)];

/// Tries of a brown-dwarf companion's orbit in one attempt's block: 9, the most that fit after
/// the decision at seven words a try.
pub const MAX_SUBSTELLAR_TRIES: u64 = 9;

/// The word of an attempt's block of `system.substellar` that decides whether there is a
/// companion.
const DECISION_WORD: u64 = 0;

/// The first word of try 0.
const FIRST_TRY_WORD: u64 = 1;

/// Words one try reads.
const WORDS_PER_TRY: u64 = 7;

/// The offset, within a try, of the mean anomaly's word: after the mark, the mass ratio, the
/// eccentricity and the three angles.
const PHASE_WORD: u64 = 6;

const _: () = assert!(
    FIRST_TRY_WORD + WORDS_PER_TRY * MAX_SUBSTELLAR_TRIES <= DRAWS_PER_ATTEMPT,
    "every try fits its attempt's block"
);

/// The probability that a primary of initial mass `m1` has a bound brown-dwarf companion at any
/// separation ([`SUBSTELLAR_ANCHORS`], linear in ln m); 0 for a primary below the stellar range,
/// which stays single.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::multiplicity::substellar_companion_probability;
/// use hyperion_sim::units::SolarMasses;
///
/// let sun = substellar_companion_probability(SolarMasses::new(1.0));
/// let m_dwarf = substellar_companion_probability(SolarMasses::new(0.2));
/// assert!(sun > m_dwarf && sun < 0.1);
/// // A brown dwarf has no brown-dwarf companion here.
/// assert!(substellar_companion_probability(SolarMasses::new(0.05)) <= 0.0);
/// ```
#[must_use]
pub fn substellar_companion_probability(m1: SolarMasses) -> f64 {
    let mass = m1.value();
    if mass.is_nan() || mass < MIN_COMPANION_MASS.value() {
        return 0.0;
    }
    let first = SUBSTELLAR_ANCHORS[0];
    let last = SUBSTELLAR_ANCHORS[SUBSTELLAR_ANCHORS.len() - 1];
    if mass <= first.0 {
        return first.1;
    }
    if mass >= last.0 {
        return last.1;
    }
    let ln_mass = math::ln(mass);
    SUBSTELLAR_ANCHORS
        .windows(2)
        .find(|pair| mass <= pair[1].0)
        .map_or(last.1, |pair| {
            let (lower, upper) = (math::ln(pair[0].0), math::ln(pair[1].0));
            let share = (ln_mass - lower) / (upper - lower);
            pair[0].1 + (pair[1].1 - pair[0].1) * share
        })
}

/// The stream of `system.substellar` of system `system`.
#[must_use]
fn stream(seed: Seed, system: SystemId) -> Stream {
    Stream::open(seed, tags::SYSTEM_SUBSTELLAR, ObjectKey::from(system))
}

/// Whether the system `system`, whose primary formed with `m1`, has a brown-dwarf companion to
/// try at `attempt` under `ctx`: its decision word alone, for the count-only draw too.
#[must_use]
pub(super) fn wants_companion(
    seed: Seed,
    system: SystemId,
    m1: SolarMasses,
    ctx: MultiplicityContext,
    attempt: RedrawAttempt,
) -> bool {
    match ctx {
        MultiplicityContext::ForcedSingle => return false,
        MultiplicityContext::Free | MultiplicityContext::ForcedMultiple { .. } => {}
    }
    let p = substellar_companion_probability(m1);
    if p <= 0.0 {
        return false;
    }
    let word = stream(seed, system).word_at(attempt.first_draw() + DECISION_WORD);
    Mark::from_word(word).is_below(Threshold::from_probability(p))
}

/// `stellar`, the stars [`draw_hierarchy`](super::draw_hierarchy) drew for `record` under `ctx`
/// at `attempt`, with the system's brown-dwarf companion if it has one and a try of its orbit
/// passes the test (module documentation).
#[must_use]
pub(super) fn with_companion(
    galaxy: &Galaxy,
    record: &SystemRecord,
    ctx: MultiplicityContext,
    attempt: RedrawAttempt,
    stellar: SystemHierarchy,
) -> SystemHierarchy {
    let m1 = stellar.star(StarIndex::PRIMARY).initial_mass();
    if !wants_companion(galaxy.seed(), record.id(), m1, ctx, attempt) {
        return stellar;
    }
    let limits = Limits::new(
        galaxy.potential(),
        PointLy::from(record.epoch_position()),
        match ctx {
            MultiplicityContext::ForcedMultiple { max_separation } => max_separation,
            MultiplicityContext::Free | MultiplicityContext::ForcedSingle => None,
        },
        // The stars already satisfy the primary's stripped mark; a brown dwarf is no part of it.
        Innermost::Free,
    );
    let model = MultiplicityModel::default_v1();
    let Some(windows) = Windows::new(&model, &stellar, &limits) else {
        return stellar;
    };
    let base = attempt.first_draw();
    let mut words = stream(galaxy.seed(), record.id());
    let mut phases = stream(galaxy.seed(), record.id());
    (0..MAX_SUBSTELLAR_TRIES)
        .find_map(|r| {
            let first = base + FIRST_TRY_WORD + WORDS_PER_TRY * r;
            words.seek(first);
            phases.seek(first + PHASE_WORD);
            let (index, u) = pick_window(&windows.thresholds, words.mark());
            let (lo, hi) = windows.bounds[index];
            let period = windows.periods.quantile_in(u, lo, hi);
            let log_period = math::log10(period.value());
            let gamma = model.substellar_mass_ratio_slope(m1, log_period);
            let q = mass_ratio_law(m1, gamma).quantile(words.uniform());
            let e = model
                .eccentricity_distribution(m1, period)
                .quantile(words.uniform());
            let orbit = draft_orbit(period, e, &mut words, &mut phases)?;
            let candidate = stellar.with_outer_brown_dwarf(m1 * q, &orbit);
            limits.admits(&candidate).then_some(candidate)
        })
        .unwrap_or(stellar)
}

/// The mass-ratio law of a brown-dwarf companion of a primary of `m1`: `q^γ` from
/// [`MIN_SUBSTELLAR_COMPANION_MASS`] ÷ m₁ to [`MIN_COMPANION_MASS`] ÷ m₁.
#[must_use]
fn mass_ratio_law(m1: SolarMasses, gamma: f64) -> PowerLaw {
    PowerLaw::new(
        -gamma,
        MIN_SUBSTELLAR_COMPANION_MASS / m1,
        MIN_COMPANION_MASS / m1,
    )
    .expect("a primary of 0.08 M_sun or more gives a mass-ratio range inside (0, 1]")
}

/// The two period windows of a brown-dwarf companion, below and above the desert's edge, with
/// their thresholds.
struct Windows {
    periods: super::dist::PeriodDistribution,
    bounds: [(Days, Days); 2],
    thresholds: Thresholds,
}

impl Windows {
    /// The windows the necessary conditions of the test leave a new outermost orbit of
    /// `stellar`, or `None` if they leave none.
    #[must_use]
    fn new(
        model: &MultiplicityModel,
        stellar: &SystemHierarchy,
        limits: &Limits<'_>,
    ) -> Option<Self> {
        let system_mass = stellar.system_mass();
        let m1 = stellar.star(StarIndex::PRIMARY).initial_mass();
        let (heaviest, lightest) = (
            system_mass + MIN_COMPANION_MASS,
            system_mass + MIN_SUBSTELLAR_COMPANION_MASS,
        );
        let a_lo = stellar.pairs().next().map_or(Metres::ZERO, |(_, root)| {
            root.semi_major_axis() * NECESSARY_AXIS_RATIO
        });
        let a_hi = limits.widest_axis(heaviest);
        let p_lo = period_of(a_lo, heaviest) * (1.0 - WINDOW_MARGIN);
        let p_hi = period_of(a_hi, lightest) * (1.0 + WINDOW_MARGIN);
        let periods = model.period_distribution(m1);
        let shortest = math::exp10(periods.support().0);
        let lo = Days::new(Days::from(p_lo).value().max(shortest));
        let hi = Days::from(p_hi);
        if hi <= lo {
            return None;
        }
        let edge = SUBSTELLAR_DESERT_PERIOD;
        let below = (lo, if hi < edge { hi } else { edge });
        let above = (if lo > edge { lo } else { edge }, hi);
        let share = |(a, b): (Days, Days)| if b > a { periods.share_in(a, b) } else { 0.0 };
        let weights = [SUBSTELLAR_DESERT_FACTOR * share(below), share(above)];
        let total = weights[0] + weights[1];
        if total <= 0.0 {
            return None;
        }
        Some(Self {
            thresholds: Thresholds::from_weights(&weights, total),
            periods,
            bounds: [below, above],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::direct::PERIOD_CORRECTION;
    use super::super::hierarchy::{HierarchyNode, SlotKind, draw_hierarchy, draw_hierarchy_with};
    use super::super::quadrature::{all_stars_fraction_below, all_stars_fraction_below_as_drawn};
    use super::super::testing::{self, SAMPLE};
    use super::*;
    use crate::galaxy::imf::Kroupa;
    use crate::orbit::KeplerElements;
    use hyperion_testkit::float::assert_same_bits;

    /// The draw's stars alone, without the companion.
    fn stellar(galaxy: &Galaxy, record: &SystemRecord) -> SystemHierarchy {
        draw_hierarchy_with(
            galaxy,
            record,
            None,
            MultiplicityContext::Free,
            RedrawAttempt::FIRST,
            &PERIOD_CORRECTION,
        )
    }

    fn free(galaxy: &Galaxy, record: &SystemRecord) -> SystemHierarchy {
        draw_hierarchy(
            galaxy,
            record,
            MultiplicityContext::Free,
            RedrawAttempt::FIRST,
        )
    }

    /// The companion's orbit: the new root's.
    fn companion_orbit(h: &SystemHierarchy) -> Option<&KeplerElements> {
        let last = h.stars().last()?;
        (last.kind() == SlotKind::BrownDwarf).then(|| {
            let HierarchyNode::Pair { orbit, .. } = h.node(h.root()) else {
                unreachable!("a system with a companion has a root pair")
            };
            orbit
        })
    }

    /// Adding the companion moves no star: every star, node mass and stellar orbit of 10⁴
    /// systems over the whole mass function is the draw's without it, bit for bit, one node
    /// further on, and the companion is the last body, the outer member of the new root.
    #[test]
    fn adding_the_companion_moves_no_star() {
        let galaxy = testing::galaxy();
        let mut with = 0;
        for record in testing::imf_records(&galaxy, SAMPLE, &testing::sunlike(), 0x2d_0001) {
            let (all, stars) = (free(&galaxy, &record), stellar(&galaxy, &record));
            if all.star_count() == stars.star_count() {
                assert_eq!(all, stars);
                continue;
            }
            with += 1;
            let n = stars.stars().len();
            assert_eq!(all.stars().len(), n + 1);
            assert_eq!(&all.stars()[..n], stars.stars());
            let companion = all.stars()[n];
            assert_eq!(companion.kind(), SlotKind::BrownDwarf);
            assert_eq!(usize::from(companion.body().body_index()), n);
            assert!(
                (MIN_SUBSTELLAR_COMPANION_MASS.value()..MIN_COMPANION_MASS.value())
                    .contains(&companion.initial_mass().value())
            );
            let HierarchyNode::Pair { inner, outer, .. } = *all.node(all.root()) else {
                panic!("a root pair")
            };
            assert_eq!(inner.get(), 1);
            assert!(
                matches!(all.node(outer), HierarchyNode::Star(s) if usize::from(s.get()) == n),
                "the companion is the root's outer member"
            );
            assert_eq!(all.nodes().len(), stars.nodes().len() + 2);
            assert_eq!(
                all.system_mass(),
                stars.system_mass() + companion.initial_mass()
            );
            for (i, node) in stars.nodes().iter().enumerate() {
                let moved = all.nodes()[i + 1];
                match (node, moved) {
                    (HierarchyNode::Star(a), HierarchyNode::Star(b)) => assert_eq!(*a, b),
                    (
                        HierarchyNode::Pair {
                            inner,
                            outer,
                            orbit,
                        },
                        HierarchyNode::Pair {
                            inner: i2,
                            outer: o2,
                            orbit: b,
                        },
                    ) => {
                        assert_eq!((inner.get() + 1, outer.get() + 1), (i2.get(), o2.get()));
                        assert_eq!(*orbit, b);
                    }
                    (HierarchyNode::Star(_), HierarchyNode::Pair { .. })
                    | (HierarchyNode::Pair { .. }, HierarchyNode::Star(_)) => {
                        panic!("node {i} changed kind")
                    }
                }
            }
            assert_eq!(all.dropped_companions(), stars.dropped_companions());
        }
        assert!(with > 100, "{with} companions in {SAMPLE} systems");
    }

    /// P11.T2.b's properties with the companion: every hierarchy that holds one passes the whole
    /// test (Mardling and Aarseth for every pair, every apocentre inside the tidal cut) at the
    /// Sun-like point and in the inner disc, and the draw does not depend on what was drawn before.
    #[test]
    fn hierarchies_with_a_companion_pass_the_whole_test_in_any_order() {
        let galaxy = testing::galaxy();
        for (at, salt) in [
            (testing::sunlike(), 0x2d_0004),
            (testing::inner_disc(), 0x2d_0005),
        ] {
            let limits = Limits::new(
                galaxy.potential(),
                PointLy::from(&at),
                None,
                Innermost::Free,
            );
            let records = testing::imf_records(&galaxy, SAMPLE, &at, salt);
            let mut with = 0;
            for record in &records {
                let h = free(&galaxy, record);
                if h.stars()
                    .last()
                    .is_some_and(|s| s.kind() == SlotKind::BrownDwarf)
                {
                    with += 1;
                    assert!(limits.admits(&h), "{:?}", record.id());
                }
            }
            assert!(with > 50, "{with} companions");
            hyperion_testkit::order::assert_order_independent(&records[..400], |r| {
                free(&galaxy, r)
            });
        }
    }

    /// Bound brown dwarfs number 0.02–0.06 per star over the whole mass function (P11.T2.d's
    /// test), which with plan 13's free-floating one for every five or six stars gives the
    /// brainstorm's one for every four or five.
    #[test]
    fn bound_brown_dwarfs_number_two_to_six_per_hundred_stars() {
        let galaxy = testing::galaxy();
        let (mut stars, mut dwarfs) = (0_u32, 0_u32);
        for record in testing::imf_records(&galaxy, SAMPLE, &testing::sunlike(), 0x2d_0002) {
            for slot in free(&galaxy, &record).stars() {
                match slot.kind() {
                    SlotKind::Star => stars += 1,
                    SlotKind::BrownDwarf => dwarfs += 1,
                }
            }
        }
        let per_star = f64::from(dwarfs) / f64::from(stars);
        println!("{dwarfs} brown dwarfs over {stars} stars: {per_star:.4} per star");
        assert!((0.02..=0.06).contains(&per_star), "{per_star}");
    }

    /// Fewer than 1% of Sun-like primaries have a brown dwarf inside 10³ days (P11.T2.d's test,
    /// the desert), and some have one beyond.
    #[test]
    fn few_sun_like_primaries_have_one_in_the_desert() {
        let galaxy = testing::galaxy();
        let (mut inside, mut beyond) = (0_u32, 0_u32);
        for record in testing::records_of_mass(&galaxy, SAMPLE, &testing::sunlike(), 1.0) {
            let h = free(&galaxy, &record);
            if let Some(orbit) = companion_orbit(&h) {
                if Days::from(orbit.period()) < SUBSTELLAR_DESERT_PERIOD {
                    inside += 1;
                } else {
                    beyond += 1;
                }
            }
        }
        let share = f64::from(inside) / f64::from(SAMPLE);
        println!("inside 10^3 d: {inside}, beyond: {beyond} of {SAMPLE}");
        assert!(share < 0.01, "{share}");
        assert!(beyond > 200, "{beyond}");
    }

    /// A forced single star and a brown-dwarf primary have no companion, and the count-only draw
    /// counts it.
    #[test]
    fn a_forced_single_and_a_brown_dwarf_primary_stay_single() {
        let galaxy = testing::galaxy();
        for record in testing::imf_records(&galaxy, 2_000, &testing::sunlike(), 0x2d_0003) {
            let single = draw_hierarchy(
                &galaxy,
                &record,
                MultiplicityContext::ForcedSingle,
                RedrawAttempt::FIRST,
            );
            assert_eq!(single.star_count(), 1);
            for attempt in [RedrawAttempt::FIRST, RedrawAttempt::new(3).unwrap()] {
                assert_eq!(
                    super::super::draw_star_count(
                        &galaxy,
                        &record,
                        MultiplicityContext::Free,
                        attempt
                    ),
                    draw_hierarchy(&galaxy, &record, MultiplicityContext::Free, attempt)
                        .star_count()
                );
            }
        }
        assert!(substellar_companion_probability(SolarMasses::new(0.07)).abs() < f64::MIN_POSITIVE);
        assert!(
            substellar_companion_probability(SolarMasses::new(f64::NAN)).abs() < f64::MIN_POSITIVE
        );
        let at = |m: f64| substellar_companion_probability(SolarMasses::new(m));
        assert!((at(0.1) - 0.035).abs() < 1e-15 && (at(150.0) - 0.02).abs() < 1e-15);
        assert!((at(1.0) - 0.06).abs() < 1e-15);
    }

    /// The companion is left out of every stellar quadrature: the all-stars fraction is the
    /// model's, bit for bit (P11.T2.d's test).
    #[test]
    fn the_all_stars_fraction_is_unchanged_to_the_last_bit() {
        let model = MultiplicityModel::default_v1();
        for ((m, spine, drawn), label) in PINNED_FRACTIONS.into_iter().zip(["0.5", "1"]) {
            let at = SolarMasses::new(m);
            let (a, b) = (
                all_stars_fraction_below(&Kroupa, &model, at),
                all_stars_fraction_below_as_drawn(&Kroupa, &model, at),
            );
            println!("below {label} M☉: {a:?}, as drawn {b:?}");
            assert_same_bits(a, spine);
            assert_same_bits(b, drawn);
        }
    }

    /// The all-stars fractions below 0.5 and 1 M☉ under Kroupa's function, spine and as drawn,
    /// as P11.T1.d left them: (mass, spine, as drawn).
    const PINNED_FRACTIONS: [(f64, f64, f64); 2] = [
        (0.5, 0.766_322_810_095_879, 0.767_809_616_117_806),
        (1.0, 0.901_830_230_693_662_9, 0.902_703_374_114_515),
    ];
}
