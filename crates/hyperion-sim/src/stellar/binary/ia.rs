//! The Type Ia explosion mark (plan 11, P11.T6; design notes 8, 9 and 13): which of the binary
//! engine's pooled Type Ia candidates explode.
//!
//! The engine pools a binary's first event that could be a Type Ia supernova
//! ([`PooledIaEvent`]: two white dwarfs merging, or an accreting white dwarf reaching ignition,
//! sub-Chandrasekhar cases included) and carries on as if it had not exploded. The binary formulae
//! do not set the rate, which they are known to underpredict several times over (Claeys et al.
//! 2014, A&A 563, A83): an independent mark thins the pool towards the observed rate (the
//! brainstorm's "Supernova remnants: one route, not two"). Only a candidate that can detonate
//! explodes (`is_detonatable`, ruling 129.1's rule): a merger above the Chandrasekhar mass or
//! with a dwarf of 0.85 M☉ or more, or an accretion onto a dwarf of 0.85 M☉ or more (Shen et al.
//! 2018, ApJ 865, 15). Such a candidate explodes when its pair's integer draw on `binary.ia_mark`
//! ([`tags::BINARY_IA_MARK`]), at word 64n of redraw attempt n and keyed by the star the pair's
//! orbit brings in
//! ([`SystemHierarchy::pair_key`](crate::stellar::multiplicity::SystemHierarchy::pair_key)), falls
//! under η of the event's delay bin and channel ([`IaYieldTable`]; the scratch
//! [`IA_YIELD`](crate::tables::binary::IA_YIELD) until P15.T9.b fits it). An unexploded pooled
//! event, every merger too light to detonate among them (most helium-bearing ones), stays what the
//! engine made it: a massive white dwarf, an R Coronae Borealis star, a hot subdwarf.
//!
//! The mark is a pure function of the seed, the pair's key and the attempt, and nothing else reads
//! its stream, so it moves no other draw. The system stage reads it under P11.T6's hold (the
//! stellar rules' `ia_explosion`, from version 27): a grid binary whose marked explosion lies at or
//! before +H redraws, as the four binary classes do (design notes 8 and 9), since plan 09's Type
//! Ia class holds every recent Type Ia.

use super::classify::MIN_DETONATABLE_MASS;
use super::timeline::{BinaryTimeline, IaPoolChannel, PooledIaEvent};
use crate::GeneratorVersion;
use crate::id::BodyId;
use crate::rng::{Mark, ObjectKey, Seed, Stream, Threshold, tags};
use crate::stellar::multiplicity::RedrawAttempt;
use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;
use crate::tables::binary::IaYieldTable;
use crate::units::SolarMasses;

/// The generator version from which the grid redraws a binary whose pooled Type Ia candidate its
/// explosion mark explodes at or before +H (plan 11, P11.T6): the hold of the stellar rules'
/// `ia_explosion` field ([`IaExplosionRules`]).
pub(crate) const IA_EXPLOSION_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether the system stage reads the Type Ia explosion mark (the stellar rules' `ia_explosion`
/// field, the hold of [`IA_EXPLOSION_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum IaExplosionRules {
    /// Version 26's and before: no pooled candidate explodes, and every grid binary stays what
    /// the engine made it.
    Pooled,
    /// Version 27's: a pooled candidate the mark explodes at or before +H is a Type Ia, plan 09's
    /// class holds it, and the grid redraws its binary
    /// ([`SystemStars::exploded_pair`](crate::stellar::system::SystemStars::exploded_pair)).
    Marked,
}

/// A pair's Type Ia explosion mark at one redraw attempt (plan 11's Provides): the integer draw
/// that decides whether the pair's pooled Type Ia candidate explodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IaExplosionMark {
    mark: Mark,
}

impl IaExplosionMark {
    /// The mark of the pair keyed by `pair`, the star its orbit brings in
    /// ([`SystemHierarchy::pair_key`](crate::stellar::multiplicity::SystemHierarchy::pair_key)),
    /// at redraw attempt `attempt` in the universe of `seed`: word 64n of its `binary.ia_mark`
    /// stream, read alone.
    #[must_use]
    pub fn of(seed: Seed, pair: BodyId, attempt: RedrawAttempt) -> Self {
        let stream = Stream::open(seed, tags::BINARY_IA_MARK, ObjectKey::from(pair));
        Self {
            mark: Mark::from_word(stream.word_at(attempt.first_draw())),
        }
    }

    /// Whether pooled `event` explodes as a Type Ia supernova: it can detonate
    /// (`is_detonatable`), and the mark is below η of its delay bin and channel in `table`.
    #[must_use]
    pub fn explodes(self, event: &PooledIaEvent, table: &IaYieldTable) -> bool {
        is_detonatable(event)
            && self.mark.is_below(Threshold::from_probability(
                table.eta(event.age(), event.channel()),
            ))
    }
}

/// Whether a pooled Type Ia candidate can detonate, by ruling 129.1's rule as the double
/// degenerate class applies it
/// ([`BinaryClass::TypeIaProgenitor`](super::BinaryClass::TypeIaProgenitor)):
///
/// - a merger whose two dwarfs together exceed the Chandrasekhar mass, or whose heavier is at
///   least [`MIN_DETONATABLE_MASS`], 0.85 M☉ (Shen et al. 2018, ApJ 865, 15, section 4.1). A
///   helium dwarf never reaches it (it ignites at 0.7 M☉), so the masses decide without the
///   dwarfs' phases;
/// - an accretion onto a dwarf ([`PooledIaEvent::masses`]' first) of at least
///   [`MIN_DETONATABLE_MASS`]: the Chandrasekhar mass, or an edge-lit detonation on a
///   carbon–oxygen dwarf of 0.85 M☉ or more, and not a helium dwarf's ignition at 0.7 M☉. The
///   dwarf's mass at ignition is compared, its accreted helium shell (0.15 M☉ or more) included,
///   since the event carries no core mass: a core of about 0.70 M☉ passes.
///
/// Every other candidate, a helium-bearing merger below the limit among them, stays what the
/// engine made it.
#[must_use]
pub(crate) fn is_detonatable(event: &PooledIaEvent) -> bool {
    let [m0, m1] = event.masses().map(SolarMasses::value);
    match event.channel() {
        IaPoolChannel::Merger => {
            m0 + m1 > CHANDRASEKHAR_MASS.value() || m0.max(m1) >= MIN_DETONATABLE_MASS.value()
        }
        IaPoolChannel::Accretion => m0 >= MIN_DETONATABLE_MASS.value(),
    }
}

/// The pooled Type Ia candidate of `timeline` that explodes under `mark` and `table`, if it has
/// one ([`IaExplosionMark::explodes`]). Its age is the explosion's, at or before the age the
/// timeline was run to.
#[must_use]
pub(crate) fn marked_explosion(
    timeline: &BinaryTimeline,
    mark: IaExplosionMark,
    table: &IaYieldTable,
) -> Option<PooledIaEvent> {
    timeline
        .pooled_ia()
        .filter(|event| mark.explodes(event, table))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

    use super::super::evolve::{Engine, LiveOrbit};
    use super::super::star::{Member, Path};
    use super::super::tests::pair;
    use super::super::timeline::{Context, IaPoolChannel};
    use super::*;
    use crate::galaxy::Galaxy;
    use crate::galaxy::catalogue_classes::type_ia::{DelayTimeDistribution, IaProgenitor};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::CellKey;
    use crate::galaxy::{POPULATIONS, Population};
    use crate::id::Layer;
    use crate::stellar::Phase;
    use crate::tables::binary::{IA_DELAY_BINS, IA_YIELD};
    use crate::tables::type_ia_delay::DELAY_EDGES;
    use crate::time::{CLOCK_WINDOW_H, Span, UniverseTime};
    use crate::units::consts::SOLAR_RADIUS_M;
    use crate::units::{Megayears, SolarMasses, Years};

    const SEED: u64 = 0x0b17_0006_0000_0001;

    /// A sixth in every delay bin and for both channels: the scratch yield before only
    /// detonatable candidates exploded, which the mark's tests and its pinned labels read.
    const SIXTH: IaYieldTable = IaYieldTable::new([[1.0 / 6.0; 2]; IA_DELAY_BINS]);

    /// The pair key of synthetic pair `k`: its secondary in candidate `k` of a layer-D cell.
    fn key(k: u32) -> BodyId {
        let cell = CellKey::new(Layer::D, [0, 406, 0]).expect("a cell of the root cube");
        BodyId::new(cell.candidate_id(k).expect("under the capacity"), 1)
    }

    /// P11.T6: attempt n's mark is word 64n of the pair's `binary.ia_mark` stream, read alone, so
    /// it is the same whichever attempts were read before it and in whatever order.
    #[test]
    fn a_pair_s_mark_at_attempt_n_is_its_word_64n_read_alone() {
        let seed = Seed::new(SEED);
        let attempts: Vec<RedrawAttempt> = RedrawAttempt::all().collect();
        for k in 0..16 {
            let pair = key(k);
            let stream = Stream::open(seed, tags::BINARY_IA_MARK, ObjectKey::from(pair));
            for &attempt in &attempts {
                assert_eq!(
                    IaExplosionMark::of(seed, pair, attempt),
                    IaExplosionMark {
                        mark: Mark::from_word(stream.word_at(64 * u64::from(attempt.get()))),
                    }
                );
            }
            assert_order_independent(&attempts, |&attempt| {
                IaExplosionMark::of(seed, pair, attempt)
            });
        }
    }

    /// P11.T6: exploded ÷ pooled is η to Poisson accuracy, in every delay bin and for both
    /// channels, over 2,000 synthetic detonatable candidates each under [`SIXTH`], every one on
    /// its own pair's mark; and with η of 0 none explodes, with η of 1 every one.
    #[test]
    fn exploded_over_pooled_is_eta_to_poisson_accuracy() {
        let seed = Seed::new(SEED);
        let per_bin = 2_000_u32;
        let masses = [SolarMasses::new(0.9), SolarMasses::new(0.6)];
        let (mut total, mut exploded) = (0_u32, 0_u64);
        for bin in 0..IA_DELAY_BINS {
            let delay = Years::new(f64::midpoint(DELAY_EDGES[bin], DELAY_EDGES[bin + 1]));
            for channel in [IaPoolChannel::Merger, IaPoolChannel::Accretion] {
                let event = PooledIaEvent::new(delay, channel, masses);
                let base = total;
                let count = (0..per_bin)
                    .filter(|&i| {
                        IaExplosionMark::of(seed, key(base + i), RedrawAttempt::FIRST)
                            .explodes(&event, &SIXTH)
                    })
                    .count();
                let count = u64::try_from(count).expect("a count");
                let eta = SIXTH.eta_in(bin, channel);
                // α over the 48 cells together, so that each cell is tested at α ÷ 48.
                assert_poisson_count(
                    &format!("bin {bin} {channel:?}"),
                    count,
                    eta * f64::from(per_bin),
                    ALPHA / 48.0,
                );
                total += per_bin;
                exploded += count;
            }
        }
        assert_poisson_count("every bin", exploded, f64::from(total) / 6.0, ALPHA);
        let event = PooledIaEvent::new(Years::new(1.0e9), IaPoolChannel::Merger, masses);
        let never = IaYieldTable::new([[0.0; 2]; IA_DELAY_BINS]);
        let always = IaYieldTable::new([[1.0; 2]; IA_DELAY_BINS]);
        for k in 0..500 {
            let mark = IaExplosionMark::of(seed, key(k), RedrawAttempt::FIRST);
            assert!(!mark.explodes(&event, &never));
            assert!(mark.explodes(&event, &always));
        }
    }

    /// P11.T6: η is read from the candidate's own delay bin and channel. Under a table that is 1 in
    /// one bin for one channel and 0 elsewhere, a candidate explodes only in that bin and channel.
    #[test]
    fn eta_is_read_from_the_candidate_s_own_bin_and_channel() {
        let index = |channel| match channel {
            IaPoolChannel::Merger => 0,
            IaPoolChannel::Accretion => 1,
        };
        let channels = [IaPoolChannel::Merger, IaPoolChannel::Accretion];
        let masses = [SolarMasses::new(0.9), SolarMasses::new(0.6)];
        let mark = IaExplosionMark::of(Seed::new(SEED), key(0), RedrawAttempt::FIRST);
        for bin in [0, 7, IA_DELAY_BINS - 1] {
            for channel in channels {
                let mut eta = [[0.0; 2]; IA_DELAY_BINS];
                eta[bin][index(channel)] = 1.0;
                let table = IaYieldTable::new(eta);
                for (b, w) in DELAY_EDGES.windows(2).enumerate() {
                    for c in channels {
                        let delay = Years::new(f64::midpoint(w[0], w[1]));
                        let event = PooledIaEvent::new(delay, c, masses);
                        assert_eq!(
                            mark.explodes(&event, &table),
                            b == bin && c == channel,
                            "η 1 in bin {bin} {channel:?}, a candidate in bin {b} {c:?}"
                        );
                    }
                }
            }
        }
    }

    /// P11.T6: eight pairs' explosion marks at every attempt are pinned bit for bit, with whether
    /// each explodes a candidate under [`SIXTH`] (the scratch yield's η of 1 would label every one
    /// "explodes"): the tag, the key and the word are output from version 27.
    #[test]
    fn the_explosion_marks_are_pinned() {
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;

        let seed = Seed::new(SEED);
        let masses = [SolarMasses::new(0.8), SolarMasses::new(0.7)];
        let event = PooledIaEvent::new(Years::new(1.0e9), IaPoolChannel::Merger, masses);
        let mut w = GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        for k in 0..8 {
            for attempt in RedrawAttempt::all() {
                let mark = IaExplosionMark::of(seed, key(k), attempt);
                w.u64_hex(
                    &format!(
                        "pair {k} attempt {}: explodes {}",
                        attempt.get(),
                        mark.explodes(&event, &SIXTH)
                    ),
                    mark.mark.get(),
                );
            }
        }
        golden!("stellar/ia_marks", w.as_str());
    }

    /// A pooled candidate of `channel` at 1 Gyr, of `masses` M☉.
    fn candidate(channel: IaPoolChannel, masses: [f64; 2]) -> PooledIaEvent {
        PooledIaEvent::new(Years::new(1.0e9), channel, masses.map(SolarMasses::new))
    }

    /// P11.T6 and ruling 129.1: a merger detonates above the Chandrasekhar mass or with a dwarf of
    /// 0.85 M☉ or more, and an accretion onto a dwarf of 0.85 M☉ or more, each at its edge.
    #[test]
    fn a_candidate_is_detonatable_by_ruling_129_1_s_rule() {
        use IaPoolChannel::{Accretion, Merger};
        let cases = [
            (Merger, [0.90, 0.60], true),
            (Merger, [0.30, 0.85], true),
            (Merger, [0.84, 0.61], true),
            (Merger, [0.84, 0.59], false),
            (Merger, [0.677, 0.293], false),
            (Merger, [0.45, 0.30], false),
            (Accretion, [CHANDRASEKHAR_MASS.value(), 0.6], true),
            (Accretion, [0.85, 0.3], true),
            (Accretion, [0.84, 1.2], false),
            (Accretion, [0.7, 0.4], false),
        ];
        for (channel, masses, detonatable) in cases {
            assert_eq!(
                is_detonatable(&candidate(channel, masses)),
                detonatable,
                "{channel:?} {masses:?}"
            );
        }
    }

    /// P11.T6: a 0.45 + 0.30 M☉ merger and a helium dwarf igniting at 0.7 M☉ never explode,
    /// whatever the mark: not under the scratch yield's η of 1, at any attempt of 2,000 pairs,
    /// where a detonatable candidate of the same delay and channel always does.
    #[test]
    fn a_candidate_that_cannot_detonate_never_explodes() {
        let seed = Seed::new(SEED);
        let light = [
            candidate(IaPoolChannel::Merger, [0.45, 0.30]),
            candidate(IaPoolChannel::Accretion, [0.7, 0.4]),
        ];
        let heavy = light.map(|event| candidate(event.channel(), [0.9, 0.6]));
        for k in 0..2_000 {
            for attempt in RedrawAttempt::all() {
                let mark = IaExplosionMark::of(seed, key(k), attempt);
                for (event, twin) in light.iter().zip(&heavy) {
                    assert!(mark.explodes(twin, &IA_YIELD));
                    assert!(
                        !mark.explodes(event, &IA_YIELD),
                        "{k} {attempt:?} {event:?}"
                    );
                }
            }
        }
    }

    /// P11.T6: a 0.90 + 0.60 M☉ merger and an accretor at the Chandrasekhar mass explode exactly
    /// when the mark is below η, under the scratch yield, [`SIXTH`] and a half, at every attempt
    /// of 2,000 pairs; under [`SIXTH`] about a sixth of them do. Attempt n's answer is the same
    /// whichever attempts were read before it.
    #[test]
    fn a_detonatable_candidate_explodes_exactly_when_its_mark_is_below_eta() {
        let seed = Seed::new(SEED);
        let heavy = [
            candidate(IaPoolChannel::Merger, [0.90, 0.60]),
            candidate(IaPoolChannel::Accretion, [CHANDRASEKHAR_MASS.value(), 0.6]),
        ];
        let half = IaYieldTable::new([[0.5; 2]; IA_DELAY_BINS]);
        let attempts: Vec<RedrawAttempt> = RedrawAttempt::all().collect();
        let mut exploded = [0_u64; 2];
        for k in 0..2_000 {
            let pair = key(k);
            for &attempt in &attempts {
                let mark = IaExplosionMark::of(seed, pair, attempt);
                for (i, event) in heavy.iter().enumerate() {
                    for (table, eta) in [(&IA_YIELD, 1.0), (&SIXTH, 1.0 / 6.0), (&half, 0.5)] {
                        assert_eq!(
                            mark.explodes(event, table),
                            mark.mark.is_below(Threshold::from_probability(eta)),
                            "{k} {attempt:?} {event:?} at η {eta}"
                        );
                    }
                    if attempt == RedrawAttempt::FIRST {
                        exploded[i] += u64::from(mark.explodes(event, &SIXTH));
                    }
                }
            }
            assert_order_independent(&attempts, |&attempt| {
                let mark = IaExplosionMark::of(seed, pair, attempt);
                heavy.map(|event| mark.explodes(&event, &SIXTH))
            });
        }
        for (event, count) in heavy.iter().zip(exploded) {
            assert_poisson_count(&format!("{event:?}"), count, 2_000.0 / 6.0, ALPHA / 2.0);
        }
    }

    /// A white dwarf of `mass` M☉ as the engine holds one formed at `birth` years: helium below
    /// 0.45 M☉, oxygen–neon above 1.05 M☉, carbon–oxygen between.
    fn dwarf(mass: f64, birth: f64) -> Member {
        let phase = if mass < 0.45 {
            Phase::HeliumWhiteDwarf
        } else if mass > 1.05 {
            Phase::OxygenNeonWhiteDwarf
        } else {
            Phase::CarbonOxygenWhiteDwarf
        };
        Member::Remnant {
            phase,
            birth,
            origin: Megayears::ZERO,
            mass: Path::starting(birth, mass),
        }
    }

    /// P11.T6 with plan 09's Type Ia entry (P09.T18.b): a double white dwarf the entry draws delay
    /// first, its two dwarfs at its separation after the common envelope from the second dwarf's
    /// birth, run through the engine, merges at its drawn delay to 1%, and its inspiral matches
    /// the entry's to 1%, since the entry solves the separation from Peters's (1964) time and the
    /// engine integrates his equation; the merger is the pair's pooled candidate.
    #[test]
    fn plan_09_s_double_white_dwarf_merges_in_the_engine_at_its_drawn_delay() {
        let galaxy = Galaxy::from_params(Seed::new(0x0918_0001), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds");
        let delays = DelayTimeDistribution::from_galaxy(&galaxy);
        let after = UniverseTime::EPOCH
            .checked_sub(Span::from_julian_years(4_000_000).expect("a span"))
            .expect("inside the clock");
        let until = UniverseTime::EPOCH
            .checked_add(CLOCK_WINDOW_H)
            .expect("inside the clock");
        let (mut checked, mut worst) = (0_u32, 0.0_f64);
        for k in 0..4_000_u64 {
            if checked >= 60 {
                break;
            }
            let population: Population = POPULATIONS[usize::try_from(k % 7).expect("an index")];
            let mut stream = Stream::open(
                Seed::new(0x0b17_0006_0000_0002),
                tags::SELFTEST_STREAM,
                ObjectKey::galaxy_item(k),
            );
            let Some(p) = IaProgenitor::draw(&delays, &mut stream, population, after, until) else {
                continue;
            };
            let (Some(m2), Some(a), Some(born), Some(inspiral)) = (
                p.secondary_dwarf(),
                p.separation_after_common_envelope(),
                p.secondary_lifetime(),
                p.inspiral(),
            ) else {
                continue;
            };
            let m1 = p.primary_dwarf();
            let (start, delay) = (born.as_julian_years_f64(), p.delay().as_julian_years_f64());
            let input = pair(m1.value(), m2.value(), 0.1, 0.0, 0.02);
            let orbit = LiveOrbit::new(
                start,
                a.value() / SOLAR_RADIUS_M,
                0.0,
                *input.orbit().orientation(),
                input.orbit().mean_anomaly_at_epoch(),
            );
            let mut engine = Engine::new(
                Arc::new(Context::of(&input)),
                start,
                delay * 1.05,
                [dwarf(m1.value(), start), dwarf(m2.value(), start)],
                orbit,
                None,
            );
            engine.run();
            let timeline = engine.finish();
            let merged = timeline.merger_age().expect("the dwarfs merge").value();
            let off = (merged / delay - 1.0).abs();
            let inspiral = inspiral.as_julian_years_f64();
            let inspiral_off = ((merged - start) / inspiral - 1.0).abs();
            assert!(off < 1e-2, "{k}: {merged} yr against the delay {delay}");
            assert!(
                inspiral_off < 1e-2,
                "{k}: an inspiral of {} yr against the entry's {inspiral}",
                merged - start
            );
            let pooled = timeline.pooled_ia().expect("a merger of two white dwarfs");
            assert_eq!(pooled.channel(), IaPoolChannel::Merger);
            worst = worst.max(inspiral_off);
            checked += 1;
        }
        println!("{checked} entries: the engine's inspiral within {worst:.2e} of the entry's");
        assert!(checked >= 60, "only {checked} double white dwarfs drawn");
    }
}
