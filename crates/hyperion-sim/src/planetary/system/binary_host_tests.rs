//! The planets of binary-evolved hosts (research-binary-hosts §3): P14.T1.e's light of a paired
//! star from its pair's timeline, against real systems of the sample whose pairs the binary engine
//! ran.

use hyperion_testkit::float::assert_same_bits;

use super::tests::SEED;
use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::planetary::context::XuvHistory;
use crate::planetary::fate::DestructionCause;
use crate::planetary::testing::{
    PairHistory, SampleFilter, first_paired_system, sample_records, synthetic_binary,
    with_every_27_hold,
};
use crate::stellar::remnant::{CompactRemnant, RemnantKind};
use crate::stellar::system::{PairTimeline, SystemStars};
use crate::time::ClockWindow;
use crate::units::AstronomicalUnits;

/// How many of the sample's systems, nearest the Sun-like point first, the tests search for a
/// pair of a history: a common-envelope merger comes about once in 120 systems there.
const SEARCH: usize = 4_000;

fn galaxy() -> Galaxy {
    Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// Whether `phase` is a white dwarf's.
const fn is_white_dwarf(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
    )
}

/// The zone about exactly the stars `members`, by body index, ascending.
fn zone_of_stars(ctx: &SystemContext, members: &[usize]) -> OrbitZone {
    ctx.zones()
        .into_iter()
        .find(|zone| zone.members().map(usize::from).eq(members.iter().copied()))
        .expect("a pair of two stars has a zone about it")
}

/// Pair `k`'s two stars, by body index, ascending.
fn stars_of_pair(ctx: &SystemContext, k: usize) -> [usize; 2] {
    let mut stars = ctx.pairs()[k].stars().map(|s| usize::from(s.get()));
    stars.sort_unstable();
    stars
}

/// The engine's states at `t` of every star of `stars`, by body index, where the system shows all
/// of them then.
fn engine_states(stars: &SystemStars, t: UniverseTime) -> Option<Vec<StarState>> {
    let state = stars.state_at(t)?;
    (state.stars().len() == stars.stars().len()).then(|| state.stars().to_vec())
}

/// Every star of a pair the engine ran reads, at the epoch and at the clock window's ends, as the
/// stellar stage's `SystemStars::state_at` has it, bit for bit, and its peak luminosity is no less
/// than its luminosity then.
#[test]
fn binary_host_light_reads_every_paired_star_as_the_stellar_stage_does() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 600, SampleFilter::ALL).unwrap();
    let mut paired = 0;
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        if stars.pairs().is_empty() {
            continue;
        }
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            let Some(engine) = engine_states(&stars, t) else {
                continue;
            };
            for (star, state) in engine.iter().enumerate() {
                assert_eq!(
                    ctx.star_state_at(star, t),
                    Some(*state),
                    "{:?}",
                    record.id()
                );
                let peak = ctx.max_luminosity_until(star, t).value();
                assert!(
                    peak >= state.luminosity().value(),
                    "{:?} star {star}: a peak of {peak} below {} L_sun",
                    record.id(),
                    state.luminosity().value()
                );
            }
            paired += 1;
        }
    }
    assert!(paired >= 30, "{paired} paired systems' times checked");
}

/// A giant that swallowed its companion in a common envelope and left a white dwarf lights its
/// pair's zone with the white dwarf alone, where the companion's own single-star model still
/// shines.
#[test]
fn binary_host_light_of_a_common_envelope_merger_is_its_white_dwarf_s_alone() {
    let galaxy = galaxy();
    let t = UniverseTime::EPOCH;
    let (stars, k) = first_paired_system(
        &galaxy,
        SEARCH,
        PairHistory::CommonEnvelopeMerger,
        |stars, k| {
            let pair = &stars.pairs()[k];
            pair.age_at(stars.stars(), t).is_some_and(|age| {
                let state = pair.timeline().state_at(age);
                state
                    .stars()
                    .iter()
                    .any(|star| is_white_dwarf(star.phase()))
                    && pair.stars().iter().zip(state.stars()).any(|(&s, star)| {
                        star.phase() == Phase::NoRemnant
                            && stars.stars()[usize::from(s.get())]
                                .state_at(t)
                                .is_some_and(|own| own.luminosity().value() > 0.0)
                    })
            })
        },
    )
    .expect("a common-envelope merger with a white dwarf among the sample's first systems");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let members = stars_of_pair(&ctx, k);
    let zone = zone_of_stars(&ctx, &members);
    let engine = engine_states(&stars, t).expect("the system shows its drawn stars");
    let dwarf = *members
        .iter()
        .find(|&&s| is_white_dwarf(engine[s].phase()))
        .expect("the merger left a white dwarf");
    let gone = *members.iter().find(|&&s| s != dwarf).unwrap();
    assert_eq!(engine[gone].phase(), Phase::NoRemnant);
    let lit = Epoch::of(&ctx, t).luminosity(&zone).unwrap();
    assert_same_bits(lit.value(), engine[dwarf].luminosity().value());
    let merged = ctx.star_state_at(gone, t).unwrap();
    assert_same_bits(merged.luminosity().value(), 0.0);
    assert_same_bits(merged.mass().value(), 0.0);
    let companion = ctx.stars()[gone].state_at(t).unwrap().luminosity();
    assert!(
        companion.value() > 0.0,
        "{:?}: the companion's own model shines {} L_sun",
        ctx.id(),
        companion.value()
    );
}

/// A pair that merged without a common envelope into a main-sequence star, a blue straggler,
/// lights its pair's zone with the product alone, and its peak is the product's own at least.
#[test]
fn binary_host_light_of_a_main_sequence_merger_is_its_product_s() {
    let galaxy = galaxy();
    let t = UniverseTime::EPOCH;
    let (stars, k) = first_paired_system(&galaxy, SEARCH, PairHistory::Merger, |stars, k| {
        let pair = &stars.pairs()[k];
        pair.age_at(stars.stars(), t).is_some_and(|age| {
            let state = pair.timeline().state_at(age);
            state
                .stars()
                .iter()
                .any(|star| star.phase() == Phase::MainSequence)
        })
    })
    .expect("a main-sequence merger among the sample's first systems");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let members = stars_of_pair(&ctx, k);
    let zone = zone_of_stars(&ctx, &members);
    let engine = engine_states(&stars, t).expect("the system shows its drawn stars");
    let product = *members
        .iter()
        .find(|&&s| engine[s].phase() == Phase::MainSequence)
        .unwrap();
    let other = *members.iter().find(|&&s| s != product).unwrap();
    assert_eq!(engine[other].phase(), Phase::NoRemnant);
    let lit = Epoch::of(&ctx, t).luminosity(&zone).unwrap();
    assert_same_bits(lit.value(), engine[product].luminosity().value());
    let light = Epoch::of(&ctx, t).light(product).unwrap();
    assert_same_bits(
        light.luminosity().value(),
        engine[product].luminosity().value(),
    );
    assert!(ctx.max_luminosity_until(product, t) >= engine[product].luminosity());
}

/// A system the binary engine ran no pair of, a synthetic binary and a single star among them,
/// reads every star's light as its own single-star model's, bit for bit, at the epoch and at the
/// clock window's ends.
#[test]
fn binary_host_light_leaves_an_unpaired_system_as_it_was() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 200, SampleFilter::ALL).unwrap();
    let unpaired = records
        .iter()
        .map(|record| SystemStars::generate(&galaxy, record));
    let mut contexts: Vec<SystemContext> = unpaired
        .filter(|stars| stars.pairs().is_empty())
        .map(|stars| SystemContext::from_stars(&galaxy, &stars))
        .take(3)
        .collect();
    contexts.push(
        synthetic_binary(
            records[0].id(),
            SolarMasses::new(1.1),
            SolarMasses::new(0.9),
            Metres::from(AstronomicalUnits::new(20.0)),
            crate::orbit::Eccentricity::new(0.3).unwrap(),
            crate::units::Dex::ZERO,
            Years::new(5.0e9),
        )
        .unwrap(),
    );
    assert!(contexts.len() >= 3);
    for ctx in &contexts {
        assert_eq!(ctx.pairs(), &[]);
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            for (star, model) in ctx.stars().iter().enumerate() {
                assert_eq!(
                    ctx.star_state_at(star, t),
                    model.state_at(t),
                    "{:?}",
                    ctx.id()
                );
                assert_eq!(
                    ctx.max_luminosity_until(star, t),
                    model.max_luminosity_until(t),
                    "{:?}",
                    ctx.id()
                );
            }
        }
    }
}

/// Under every 27 hold the planets of merged pairs, which the engine's light leaves a dark member
/// in their zone (`NoRemnant`, no light, no radius), derive and record whole: the first systems of
/// the sample with a planet present in the zone of a common-envelope merger, of a merger without
/// one and of a post-common-envelope pair (whose planets the 27 fate mostly ends at the envelope),
/// each snapshot at the epoch and walked as the audit walks it.
#[test]
fn binary_host_light_derives_the_planets_of_merged_pairs() {
    use super::audit_tests::audit_walk_system;
    let galaxy = galaxy();
    let t = UniverseTime::EPOCH;
    let wanted = [
        (PairHistory::CommonEnvelopeMerger, 3),
        (PairHistory::Merger, 2),
        (PairHistory::PostCommonEnvelope, 1),
    ];
    let mut found = [0_usize; 3];
    let records = sample_records(&galaxy, SEARCH, SampleFilter::ALL).unwrap();
    with_every_27_hold(|| {
        for record in &records {
            if found.iter().zip(&wanted).all(|(&f, &(_, n))| f >= n) {
                break;
            }
            let stars = SystemStars::generate(&galaxy, record);
            let Some(slot) = stars.pairs().iter().find_map(|pair| {
                let age = pair.age_at(stars.stars(), t)?;
                let history = PairHistory::of(pair.timeline(), age);
                let slot = wanted.iter().position(|&(h, _)| h == history)?;
                (found[slot] < wanted[slot].1).then_some(slot)
            }) else {
                continue;
            };
            let Ok(ctx) = SystemContext::for_stars(&galaxy, &stars) else {
                continue;
            };
            let system = generate_planets(SEED, &ctx);
            let paired: u32 = ctx
                .pairs()
                .iter()
                .flat_map(PairTimeline::stars)
                .fold(0, |bits, s| bits | 1 << s.get());
            let present = system.planets().any(|planet| {
                members_of(system.zone_of(planet)) & paired != 0
                    && system.position_at(&ctx, planet.index, t).unwrap().is_some()
            });
            if !present {
                continue;
            }
            let snapshot = system.snapshot_at(&ctx, t);
            assert_eq!(snapshot.bodies().len(), system.indices().len());
            let mut derived = 0;
            audit_walk_system("binary hosts", &ctx, &system, &mut |_| derived += 1);
            assert!(derived > 0, "{:?}: no planet derived", ctx.id());
            found[slot] += 1;
        }
    });
    assert_eq!(
        found,
        wanted.map(|(_, n)| n),
        "the sample's first {SEARCH} systems hold too few merged pairs with planets"
    );
}

/// A star its companion swallowed in a common envelope emits no X-rays or ultraviolet after the
/// merger: its luminosity is zero and the energy it delivered stays at what it was then, its own
/// closed form's, which shines before the merger.
#[test]
fn binary_host_xuv_of_a_star_merged_away_ends_at_the_merger() {
    let galaxy = galaxy();
    let (stars, k) = first_paired_system(
        &galaxy,
        SEARCH,
        PairHistory::CommonEnvelopeMerger,
        |stars, k| {
            let pair = &stars.pairs()[k];
            let timeline = pair.timeline();
            timeline
                .state_at(timeline.until())
                .stars()
                .iter()
                .any(|star| star.phase() == Phase::NoRemnant)
        },
    )
    .expect("a common-envelope merger among the sample's first systems");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let pair = &ctx.pairs()[k];
    let timeline = pair.timeline();
    let end = timeline.state_at(timeline.until());
    let component = (0..2)
        .find(|&c| end.stars()[c].phase() == Phase::NoRemnant)
        .unwrap();
    let gone = usize::from(pair.stars()[component].get());
    let merger = timeline.merger_age().unwrap();
    let engine = ctx.xuv_history(gone);
    let au = Metres::from(AstronomicalUnits::new(1.0));
    let later = Years::new(merger.value() * 1.5);
    assert_same_bits(engine.luminosity(later).value(), 0.0);
    assert_same_bits(
        engine.fluence(later, au, 0.0).value(),
        engine.fluence(merger, au, 0.0).value(),
    );
    let before = Years::new(merger.value() * 0.5);
    assert!(engine.luminosity(before).value() > 0.0);
}

/// A merger on the main sequence, a blue straggler, shines in X-rays and ultraviolet from its
/// pair's first interaction as a main-sequence star of its new mass, the energy it has delivered
/// never falling.
#[test]
fn binary_host_xuv_of_a_main_sequence_merger_is_its_product_s() {
    use crate::stellar::binary::SegmentKind;
    let galaxy = galaxy();
    let t = UniverseTime::EPOCH;
    let (stars, k) = first_paired_system(&galaxy, SEARCH, PairHistory::Merger, |stars, k| {
        let pair = &stars.pairs()[k];
        let timeline = pair.timeline();
        timeline
            .segments()
            .iter()
            .find(|s| s.kind() == SegmentKind::Merged)
            .is_some_and(|merged| {
                timeline
                    .state_at(merged.start())
                    .stars()
                    .iter()
                    .any(|star| star.phase() == Phase::MainSequence)
            })
    })
    .expect("a main-sequence merger among the sample's first systems");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let pair = &ctx.pairs()[k];
    let timeline = pair.timeline();
    let merged = timeline
        .segments()
        .iter()
        .find(|s| s.kind() == SegmentKind::Merged)
        .unwrap();
    let product = (0..2)
        .find(|&c| timeline.state_at(merged.start()).stars()[c].phase() == Phase::MainSequence)
        .unwrap();
    let star = usize::from(pair.stars()[product].get());
    let mass = timeline.state_at(merged.start()).stars()[product].mass();
    let engine = ctx.xuv_history(star);
    let coeffs = crate::stellar::sse::ZCoeffs::new(ctx.composition().z_fit());
    let own = XuvHistory::new(mass, crate::stellar::sse::zams::luminosity(mass, &coeffs));
    let age = ctx.age_at(t);
    assert_same_bits(engine.luminosity(age).value(), own.luminosity(age).value());
    let au = Metres::from(AstronomicalUnits::new(1.0));
    let fluences: Vec<f64> = (1..=60)
        .map(|i| {
            engine
                .fluence(Years::new(age.value() * f64::from(i) / 50.0), au, 0.0)
                .value()
        })
        .collect();
    assert!(fluences.windows(2).all(|w| w[0] <= w[1]), "{fluences:?}");
}

/// A star of no pair the engine ran keeps its own X-ray and ultraviolet history, bit for bit: its
/// closed form of its initial mass and its zero-age luminosity.
#[test]
fn binary_host_xuv_leaves_an_unpaired_star_as_it_was() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 200, SampleFilter::ALL).unwrap();
    let mut checked = 0;
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        let zone_stars = ctx.zone_stars();
        for (star, history) in ctx.xuv_histories().iter().enumerate() {
            let paired = u8::try_from(star)
                .ok()
                .and_then(|index| crate::stellar::system::pair_of(ctx.pairs(), index));
            if paired.is_none() {
                let own = XuvHistory::new(
                    ctx.stars()[star].initial_mass(),
                    zone_stars[star].zams_luminosity,
                );
                assert_eq!(history, &own, "{:?} star {star}", ctx.id());
                checked += 1;
            }
        }
    }
    assert!(checked > 150, "{checked} unpaired stars");
}

/// Every star of a pair the engine ran reads, for the halo and the pulsar host, as the stellar
/// stage has it: its state as `SystemStars::state_at`'s and its pulsar as its summary's, bit for
/// bit.
#[test]
fn binary_host_history_reads_every_paired_star_as_the_stellar_stage_does() {
    use crate::stellar::system::RemnantDetail;
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 600, SampleFilter::ALL).unwrap();
    let mut paired = 0;
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        if stars.pairs().is_empty() {
            continue;
        }
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            let Some(engine) = engine_states(&stars, t) else {
                continue;
            };
            let summary = stars.summary_at(t);
            for (star, state) in engine.iter().enumerate() {
                let pulsar = match summary.stars()[star].remnant_detail() {
                    Some(RemnantDetail::NeutronStar(pulsar)) => Some(pulsar),
                    Some(RemnantDetail::BlackHole(_)) | None => None,
                };
                assert_eq!(
                    ctx.history_state_at(star, t),
                    Some(*state),
                    "{:?}",
                    record.id()
                );
                assert_eq!(ctx.pulsar_at(star, t), pulsar, "{:?}", record.id());
            }
            paired += 1;
        }
    }
    assert!(paired >= 30, "{paired} paired systems' times checked");
}

/// A merger on the main sequence, a blue straggler, keeps its own main-sequence clock: its time left and its effective age add up to the main-sequence
/// lifetime of the product's mass at the merger, within the 1% its winds since move it (Hurley,
/// Pols and Tout 2000, §5.1's `t_MS`), and not to its progenitor's, which its single model reads.
#[test]
fn binary_host_history_s_clock_of_a_main_sequence_merger_is_its_product_s() {
    use crate::stellar::binary::SegmentKind;
    let galaxy = galaxy();
    let t = UniverseTime::EPOCH;
    let (stars, k) = first_paired_system(&galaxy, SEARCH, PairHistory::Merger, |stars, k| {
        let pair = &stars.pairs()[k];
        pair.age_at(stars.stars(), t).is_some_and(|age| {
            pair.timeline()
                .state_at(age)
                .stars()
                .iter()
                .any(|star| star.phase() == Phase::MainSequence)
        })
    })
    .expect("a main-sequence merger among the sample's first systems");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let pair = &ctx.pairs()[k];
    let timeline = pair.timeline();
    let age = pair.age_at(ctx.stars(), t).unwrap();
    let component = crate::stellar::binary::Component::BOTH
        .into_iter()
        .find(|c| timeline.state_at(age).stars()[c.index()].phase() == Phase::MainSequence)
        .unwrap();
    let merged = timeline
        .segments()
        .iter()
        .find(|s| s.kind() == SegmentKind::Merged)
        .unwrap();
    let product = timeline.state_at(merged.start()).stars()[component.index()].mass();
    let state = timeline.state_at(age).stars()[component.index()];
    let left = {
        let star = usize::from(pair.stars()[component.index()].get());
        let (_, _, at) = ctx.history_pair_at(star, t).unwrap();
        timeline.main_sequence_left(component, at).unwrap()
    };
    let coeffs = crate::stellar::sse::ZCoeffs::new(ctx.composition().z_fit());
    let lifetime = crate::stellar::sse::main_sequence_lifetime(&coeffs, false, product.value());
    let sum = left.value() + state.age().value();
    assert!(
        (sum / lifetime - 1.0).abs() < 1e-2,
        "{:?}: {} yr left at an effective age of {} yr, against a lifetime of {lifetime} yr",
        ctx.id(),
        left.value(),
        state.age().value()
    );
    let model = &ctx.stars()[usize::from(pair.stars()[component.index()].get())];
    let own = model
        .shared_track()
        .and_then(|track| track.main_sequence_ends())
        .map(|end| end.value() - model.age_at(t).value());
    assert!(
        own.is_none_or(|own| (own - left.value()).abs() > 1e-3 * lifetime),
        "{:?}: the product's clock is its progenitor's",
        ctx.id()
    );
}

/// An evolved star of a pair the engine ran leaves its main sequence where its pair's timeline
/// says, by the epoch: on it just before the age found, off it at that age.
#[test]
fn binary_host_history_s_main_sequence_end_is_where_its_timeline_leaves_it() {
    let galaxy = galaxy();
    let t = UniverseTime::EPOCH;
    let mut checked = 0;
    for history in [PairHistory::PostCommonEnvelope, PairHistory::StableTransfer] {
        let (stars, k) = first_paired_system(&galaxy, SEARCH, history, |_, _| true)
            .unwrap_or_else(|| panic!("a {history:?} pair among the sample's first systems"));
        let pair = &stars.pairs()[k];
        let timeline = pair.timeline();
        let age = pair.age_at(stars.stars(), t).unwrap();
        for component in crate::stellar::binary::Component::BOTH {
            let Some((end, then)) = timeline.main_sequence_end_before(component, age) else {
                continue;
            };
            let phase =
                |at: f64| timeline.state_at(Years::new(at)).stars()[component.index()].phase();
            assert_ne!(
                phase(end.value()),
                Phase::MainSequence,
                "{:?}",
                stars.record().id()
            );
            assert_eq!(
                phase(end.value() * (1.0 - 1e-12)),
                Phase::MainSequence,
                "{:?}",
                stars.record().id()
            );
            assert!(then.value() > 0.0 && end <= age);
            checked += 1;
        }
    }
    assert!(checked >= 2, "{checked} main-sequence ends checked");
}

/// A system the binary engine ran no pair of reads every star's history, its cometary halo's and
/// its habitability clock's among them, as its own single-star model's: no star has a pair's
/// history to read, at the epoch and at the clock window's ends.
#[test]
fn binary_host_history_leaves_an_unpaired_system_as_it_was() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 200, SampleFilter::ALL).unwrap();
    let contexts: Vec<SystemContext> = records
        .iter()
        .map(|record| SystemStars::generate(&galaxy, record))
        .filter(|stars| stars.pairs().is_empty())
        .map(|stars| SystemContext::from_stars(&galaxy, &stars))
        .take(3)
        .collect();
    assert_eq!(contexts.len(), 3);
    for ctx in &contexts {
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            for (star, model) in ctx.stars().iter().enumerate() {
                assert!(ctx.history_pair_at(star, t).is_none(), "{:?}", ctx.id());
                assert_eq!(
                    ctx.history_state_at(star, t),
                    model.state_at(t),
                    "{:?}",
                    ctx.id()
                );
                assert_eq!(ctx.pulsar_at(star, t), model.pulsar_at(t), "{:?}", ctx.id());
            }
        }
    }
}

/// A star of a pair the engine ran that its pair has left on its own track, its state its single
/// model's, reads as its model does, to the bit: the habitability
/// clock before and on its main sequence, the hot history's end and luminosity past it, a
/// remnant's included, the halo's mass-loss rate, and, for a star its pair leaves alone to the end
/// of its timeline, its death, whether by then or after it.
#[test]
fn binary_host_history_reads_a_star_its_pair_left_alone_as_its_model() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 4_000, SampleFilter::ALL).unwrap();
    let (mut clocks, mut ends, mut rates, mut deaths) = (0, 0, 0, 0);
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        if stars.pairs().is_empty() {
            continue;
        }
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        for pair in ctx.pairs() {
            let timeline = pair.timeline();
            let until = timeline.until();
            for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
                let Some(age) = pair.age_at(ctx.stars(), t) else {
                    continue;
                };
                if PairHistory::of(timeline, age) != PairHistory::Detached {
                    continue;
                }
                for (k, star) in pair.stars().iter().enumerate() {
                    let component = crate::stellar::binary::Component::of_index(k);
                    let index = usize::from(star.get());
                    let model = &ctx.stars()[index];
                    let state = timeline.state_at(age).stars()[k];
                    let Some(track) = model.shared_track() else {
                        continue;
                    };
                    if model.state_at(t) != Some(state) {
                        continue;
                    }
                    assert_same_bits(
                        fastest_mass_loss_rate_in(&ctx, index, t).value(),
                        fastest_mass_loss_rate(model, t).value(),
                    );
                    rates += 1;
                    let alone = PairHistory::of(timeline, until) == PairHistory::Detached
                        && model.state_at(ClockWindow::END)
                            == Some(timeline.state_at(until).stars()[k]);
                    if alone {
                        assert_eq!(
                            first_death_in(&ctx, &[index], t),
                            first_death(&[model]),
                            "{:?}",
                            record.id()
                        );
                        deaths += 1;
                    }
                    match state.phase() {
                        Phase::Protostar | Phase::PreMainSequence | Phase::MainSequence => {
                            let own = track.main_sequence_ends().map(|end| {
                                Years::new((end.value() - model.age_at(t).value()).max(0.0))
                            });
                            assert_eq!(timeline.main_sequence_left(component, age), own);
                            clocks += 1;
                        }
                        Phase::Substellar | Phase::NoRemnant => {}
                        Phase::HertzsprungGap
                        | Phase::FirstGiantBranch
                        | Phase::CoreHeliumBurning
                        | Phase::EarlyAgb
                        | Phase::ThermallyPulsingAgb
                        | Phase::HeliumMainSequence
                        | Phase::HeliumHertzsprungGap
                        | Phase::HeliumGiantBranch
                        | Phase::PostAgb
                        | Phase::HeliumWhiteDwarf
                        | Phase::CarbonOxygenWhiteDwarf
                        | Phase::OxygenNeonWhiteDwarf
                        | Phase::NeutronStar
                        | Phase::BlackHole => {
                            let Some(end) = track.main_sequence_end() else {
                                continue;
                            };
                            assert_eq!(
                                timeline.main_sequence_end_before(component, age),
                                Some((end, track.state_at(end).luminosity())),
                                "{:?}",
                                record.id()
                            );
                            ends += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(
        clocks >= 50 && ends >= 5 && rates >= 50 && deaths >= 20,
        "{clocks} clocks, {ends} ends, {rates} rates and {deaths} deaths checked"
    );
}

/// A star of a pair the engine ran that explodes there has an infinite halo mass-loss rate from
/// its explosion, and a finite one before it.
#[test]
fn binary_host_history_s_rate_is_infinite_from_a_paired_explosion() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, SEARCH, SampleFilter::ALL).unwrap();
    let mut checked = 0;
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        let Some((k, sn)) = stars.pairs().iter().enumerate().find_map(|(k, pair)| {
            let sn = pair.timeline().supernovae().first()?;
            (sn.age() < record.age_at_epoch()).then_some((k, *sn))
        }) else {
            continue;
        };
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        let pair = &ctx.pairs()[k];
        let star = usize::from(pair.stars()[sn.component().index()].get());
        let model = &ctx.stars()[star];
        let [before, after] = [0.5, 1.0 + 1e-6].map(|share| {
            time_at_age(Years::new(sn.age().value() * share), model.age_at_epoch()).unwrap()
        });
        let rate = |at| fastest_mass_loss_rate_in(&ctx, star, at).value();
        assert!(rate(before).is_finite(), "{:?}", ctx.id());
        assert_eq!(rate(after), f64::INFINITY, "{:?}", ctx.id());
        checked += 1;
        if checked >= 3 {
            break;
        }
    }
    assert_eq!(
        checked, 3,
        "paired supernovae before the epoch in the sample"
    );
}

/// A star its companion swallows in a common envelope has no living star from its merger on, its
/// first death, as the halo reads it, at or before the merger: alive just before it, gone at it.
#[test]
fn binary_host_history_s_death_of_a_star_merged_away_is_at_its_merger() {
    let galaxy = galaxy();
    let (stars, k) = first_paired_system(
        &galaxy,
        SEARCH,
        PairHistory::CommonEnvelopeMerger,
        |stars, k| {
            let timeline = stars.pairs()[k].timeline();
            timeline
                .state_at(timeline.until())
                .stars()
                .iter()
                .any(|star| star.phase() == Phase::NoRemnant)
        },
    )
    .expect("a common-envelope merger among the sample's first systems");
    let timeline = stars.pairs()[k].timeline();
    let merger = timeline.merger_age().unwrap();
    let component = crate::stellar::binary::Component::BOTH
        .into_iter()
        .find(|c| {
            timeline.state_at(timeline.until()).stars()[c.index()].phase() == Phase::NoRemnant
        })
        .unwrap();
    let death = timeline.first_death_age(component).unwrap();
    assert!(
        death <= merger,
        "{:?}: dead at {death:?}, merged at {merger:?}",
        stars.record().id()
    );
    let phase = |at: f64| timeline.state_at(Years::new(at)).stars()[component.index()].phase();
    let dead = |phase: Phase| phase.is_remnant() || phase == Phase::NoRemnant;
    assert!(dead(phase(death.value())));
    assert!(
        !dead(phase(death.value().next_down())),
        "{:?}",
        stars.record().id()
    );
}

/// What component `star` of `ctx` leaves by +H and when it dies by its own model alone: its
/// remnant and its birth, and its death's age.
fn own_remnant(
    ctx: &SystemContext,
    star: usize,
) -> (Option<(CompactRemnant, Years)>, Option<Years>) {
    let model = &ctx.stars()[star];
    let remnant = model.remnant_age_at(ClockWindow::END).and_then(|_| {
        let remnant = model.remnant().filter(|r| r.kind() != RemnantKind::None)?;
        Some((remnant, model.death()?.age()))
    });
    (remnant, model.death().map(|d| d.age()))
}

/// What the generation reads component `star` of `ctx` leaves by +H and when it dies, as
/// [`own_remnant`] gives a model's.
fn read_remnant(
    ctx: &SystemContext,
    star: usize,
) -> (Option<(CompactRemnant, Years)>, Option<Years>) {
    (
        ctx.remnant_by_end(star).map(|r| (r.remnant, r.born)),
        ctx.death_age(star),
    )
}

/// A star of a pair the engine ran that its pair leaves as its own model to +H reads its model's
/// remnant, birth and death, to the bit; one its pair changed reads the remnant its timeline leaves
/// it at +H, born at the first age it is that kind (at its model's death where the two agree), at
/// its mass then (P14.T1.h, from generator version 28).
#[test]
fn binary_host_remnants_read_a_star_its_pair_left_alone_as_its_model() {
    use crate::planetary::context::at_own_death;
    use crate::stellar::system::remnant_kind;
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 4_000, SampleFilter::ALL).unwrap();
    let (mut alone, mut changed) = (0, 0);
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        if stars.pairs().is_empty() {
            continue;
        }
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        for pair in ctx.pairs() {
            let timeline = pair.timeline();
            for (k, star) in pair.stars().iter().enumerate() {
                let index = usize::from(star.get());
                let model = &ctx.stars()[index];
                let engine = (ctx.remnant_by_end(index), ctx.death_age(index));
                let end = timeline.state_at(timeline.until()).stars()[k];
                if model.state_at(ClockWindow::END) == Some(end) {
                    assert_eq!(
                        read_remnant(&ctx, index),
                        own_remnant(&ctx, index),
                        "{:?} star {index}",
                        record.id()
                    );
                    alone += 1;
                    continue;
                }
                let kind = remnant_kind(end.phase()).filter(|&kind| kind != RemnantKind::None);
                assert_eq!(
                    engine.0.map(|r| r.remnant.kind()),
                    kind,
                    "{:?}",
                    record.id()
                );
                if let Some(ended) = engine.0 {
                    let component = crate::stellar::binary::Component::of_index(k);
                    let raw = timeline
                        .first_age_where(component, |phase| remnant_kind(phase) == kind)
                        .unwrap();
                    assert_eq!(ended.born, at_own_death(model, raw));
                    let born = timeline.state_at(raw).stars()[k];
                    assert_eq!(remnant_kind(born.phase()), kind);
                    assert_same_bits(ended.remnant.mass().value(), born.mass().value());
                    if raw.value() > 0.0 {
                        let before = timeline
                            .state_at(Years::new(raw.value().next_down()))
                            .stars()[k];
                        assert_ne!(remnant_kind(before.phase()), kind, "{:?}", record.id());
                    }
                }
                changed += 1;
            }
        }
    }
    assert!(
        alone >= 100 && changed >= 20,
        "{alone} left alone, {changed} changed"
    );
}

/// A star its companion swallows in a common envelope leaves no remnant, and dies at or before its
/// merger, where its own model leaves its own remnant at its own death.
#[test]
fn binary_host_remnants_of_a_star_merged_away_are_none() {
    // The pair's component that the merger leaves nothing of, whose own model dies by +H.
    let swallowed = |stars: &SystemStars, k: usize| {
        let pair = &stars.pairs()[k];
        let timeline = pair.timeline();
        let end = timeline.state_at(timeline.until());
        (0..2).find(|&c| {
            end.stars()[c].phase() == Phase::NoRemnant
                && stars.stars()[usize::from(pair.stars()[c].get())]
                    .remnant_age_at(ClockWindow::END)
                    .is_some()
        })
    };
    let galaxy = galaxy();
    let (stars, k) = first_paired_system(
        &galaxy,
        SEARCH,
        PairHistory::CommonEnvelopeMerger,
        |stars, k| swallowed(stars, k).is_some(),
    )
    .expect("a common-envelope merger whose swallowed star's model dies by +H");
    let component = swallowed(&stars, k).unwrap();
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let pair = &ctx.pairs()[k];
    let star = usize::from(pair.stars()[component].get());
    let (remnant, death) = (ctx.remnant_by_end(star), ctx.death_age(star));
    assert_eq!(remnant, None, "{:?}", ctx.id());
    let (death, merger) = (death.unwrap(), pair.timeline().merger_age().unwrap());
    assert!(
        death <= merger,
        "{:?}: dead at {death:?}, merged at {merger:?}",
        ctx.id()
    );
    assert!(own_remnant(&ctx, star).0.is_some(), "{:?}", ctx.id());
}

/// The generation's readers follow the context's remnants and deaths: over the sample's paired
/// systems, a few its pairs leave alone and every one whose remnants its pairs change, a white
/// dwarf system's birth is its white dwarf's (`remnant_by_end`), and a protoplanetary disc ends at
/// the earlier of its lifetime and its stars' first death (`death_age`). Second-generation planets,
/// 9 in 500,000 systems at 27, are rarer than the sample.
#[test]
fn binary_host_remnants_drive_the_discs_as_their_readers_say() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, SEARCH, SampleFilter::ALL).unwrap();
    let t = ClockWindow::END;
    let (mut checked, mut apart) = (0, 0);
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        if stars.pairs().is_empty() {
            continue;
        }
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        let differs =
            (0..ctx.stars().len()).any(|star| read_remnant(&ctx, star) != own_remnant(&ctx, star));
        if !differs && checked >= 6 {
            continue;
        }
        let system = generate(SEED, &ctx);
        if let Some(wd) = system.white_dwarf() {
            let OrbitHost::Star(n) = wd.host() else {
                panic!("a white dwarf system's host is a star");
            };
            let star = usize::from(n);
            let ended = ctx.remnant_by_end(star).unwrap();
            assert_eq!(ended.remnant.kind(), RemnantKind::WhiteDwarf);
            let age_at_epoch = ctx.stars()[star].age_at_epoch();
            assert_eq!(Some(wd.born()), time_at_age(ended.born, age_at_epoch));
        }
        if let Some(disc) = system.protoplanetary_disc() {
            let zone = system.zone(disc.host()).unwrap();
            let lifetime = system
                .host(disc.host())
                .and_then(|host| host.disc().profile())
                .unwrap()
                .lifetime();
            let first = usize::from(zone.members().next().unwrap());
            let age_at_epoch = ctx.stars()[first].age_at_epoch();
            let died = zone
                .members()
                .filter_map(|m| {
                    let star = usize::from(m);
                    time_at_age(ctx.death_age(star)?, ctx.stars()[star].age_at_epoch())
                })
                .min();
            let end = earliest(time_at_age(Years::from(lifetime), age_at_epoch), died);
            let state = system.protoplanetary_state(&Epoch::new(&system, &ctx, t), *disc, t);
            match end.filter(|&end| t >= end) {
                Some(at) => assert_eq!(
                    state,
                    BodyState::Destroyed {
                        cause: DestructionCause::Dispersed,
                        at
                    },
                    "{:?}",
                    ctx.id()
                ),
                None => assert_ne!(state, BodyState::NotYetFormed),
            }
        }
        checked += 1;
        apart += usize::from(differs);
        if apart >= 3 && checked >= 12 {
            break;
        }
    }
    assert!(
        apart >= 1,
        "{apart} of {checked} systems' remnants read apart from their models'"
    );
}

/// A system the binary engine ran no pair of reads each star's remnant and death as its own
/// model's, bit for bit: its stars have no pair to read.
#[test]
fn binary_host_remnants_leave_an_unpaired_system_as_it_was() {
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 200, SampleFilter::ALL).unwrap();
    let contexts: Vec<SystemContext> = records
        .iter()
        .map(|record| SystemStars::generate(&galaxy, record))
        .filter(|stars| stars.pairs().is_empty())
        .map(|stars| SystemContext::from_stars(&galaxy, &stars))
        .take(3)
        .collect();
    assert_eq!(contexts.len(), 3);
    for ctx in &contexts {
        for star in 0..ctx.stars().len() {
            assert_eq!(
                read_remnant(ctx, star),
                own_remnant(ctx, star),
                "{:?} star {star}",
                ctx.id()
            );
        }
    }
}
