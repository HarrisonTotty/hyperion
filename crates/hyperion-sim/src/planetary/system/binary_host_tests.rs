//! The planets of binary-evolved hosts (research-binary-hosts §3): P14.T1.e's light of a paired
//! star from its pair's timeline (`BINARY_HOST_LIGHT_VERSION`), against real systems of the
//! sample whose pairs the binary engine ran.

use hyperion_testkit::float::assert_same_bits;

use super::tests::SEED;
use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::planetary::context::XuvHistory;
use crate::planetary::context::{
    BINARY_HOST_LIGHT_VERSION, BinaryHostLightRules, with_binary_host_light,
};
use crate::planetary::testing::{
    PairHistory, SampleFilter, first_paired_system, sample_records, synthetic_binary,
    with_every_27_hold,
};
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

/// The paired stars' light is held until the 26 → 27 bump: the rules in force are the single
/// models', the tests' switch and 27's helper reach the engine's, and 26's and 25's helpers do not.
#[test]
fn the_binary_host_light_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < BINARY_HOST_LIGHT_VERSION,
        "the 26 → 27 bump reads a paired star's light from its pair's timeline: delete \
         `BINARY_HOST_LIGHT_VERSION`, `BinaryHostLightRules` with its `SingleModels` arm and its \
         matches in `SystemContext::star_state_at` and `max_luminosity_until`, the tests' \
         `with_binary_host_light`, its switch in `with_every_27_hold` (and the helper, if no other \
         27 hold is left), and this test; restate the tests held below 27"
    );
    assert_eq!(
        BinaryHostLightRules::in_force(),
        BinaryHostLightRules::SingleModels
    );
    assert_eq!(
        with_binary_host_light(BinaryHostLightRules::Engine, BinaryHostLightRules::in_force),
        BinaryHostLightRules::Engine
    );
    assert_eq!(
        with_every_27_hold(BinaryHostLightRules::in_force),
        BinaryHostLightRules::Engine
    );
    assert_eq!(
        with_every_27_hold(|| {
            with_binary_host_light(
                BinaryHostLightRules::SingleModels,
                BinaryHostLightRules::in_force,
            )
        }),
        BinaryHostLightRules::SingleModels
    );
}

/// Under the engine's rules every star of a pair the engine ran reads, at the epoch and at the
/// clock window's ends, as the stellar stage's `SystemStars::state_at` has it, bit for bit, and its
/// peak luminosity is no less than its luminosity then; under the single models every star reads
/// as its own model.
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
                with_binary_host_light(BinaryHostLightRules::Engine, || {
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
                });
                with_binary_host_light(BinaryHostLightRules::SingleModels, || {
                    assert_eq!(ctx.star_state_at(star, t), ctx.stars()[star].state_at(t));
                    assert_eq!(
                        ctx.max_luminosity_until(star, t),
                        ctx.stars()[star].max_luminosity_until(t)
                    );
                });
            }
            paired += 1;
        }
    }
    assert!(paired >= 30, "{paired} paired systems' times checked");
}

/// A giant that swallowed its companion in a common envelope and left a white dwarf lights its
/// pair's zone with the white dwarf alone under the engine's rules, where the single models still
/// shine the companion on it.
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
    let lit = with_binary_host_light(BinaryHostLightRules::Engine, || {
        Epoch::of(&ctx, t).luminosity(&zone).unwrap()
    });
    assert_same_bits(lit.value(), engine[dwarf].luminosity().value());
    with_binary_host_light(BinaryHostLightRules::Engine, || {
        let gone = ctx.star_state_at(gone, t).unwrap();
        assert_same_bits(gone.luminosity().value(), 0.0);
        assert_same_bits(gone.mass().value(), 0.0);
    });
    let single = with_binary_host_light(BinaryHostLightRules::SingleModels, || {
        Epoch::of(&ctx, t).luminosity(&zone).unwrap()
    });
    let companion = ctx.stars()[gone].state_at(t).unwrap().luminosity();
    assert!(
        single.value() >= companion.value() && companion.value() > 0.0,
        "{:?}: the single models light the zone with {} L_sun, the companion {} L_sun",
        ctx.id(),
        single.value(),
        companion.value()
    );
}

/// A pair that merged without a common envelope into a main-sequence star, a blue straggler,
/// lights its pair's zone with the product alone under the engine's rules, and its peak is the
/// product's own at least.
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
    with_binary_host_light(BinaryHostLightRules::Engine, || {
        let lit = Epoch::of(&ctx, t).luminosity(&zone).unwrap();
        assert_same_bits(lit.value(), engine[product].luminosity().value());
        let light = Epoch::of(&ctx, t).light(product).unwrap();
        assert_same_bits(
            light.luminosity().value(),
            engine[product].luminosity().value(),
        );
        assert!(ctx.max_luminosity_until(product, t) >= engine[product].luminosity());
    });
}

/// A system the binary engine ran no pair of, a synthetic binary and a single star among them,
/// generates and reads the same records under both light rules, bit for bit.
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
        let snapshot = |rules| {
            with_binary_host_light(rules, || {
                generate_planets(SEED, ctx).snapshot_at(ctx, UniverseTime::EPOCH)
            })
        };
        assert_eq!(
            snapshot(BinaryHostLightRules::SingleModels),
            snapshot(BinaryHostLightRules::Engine),
            "{:?}",
            ctx.id()
        );
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

/// The paired stars' X-ray and ultraviolet histories are held until the 26 → 27 bump: the rules
/// in force are the single models', the tests' switch and 27's helper reach the engine's, and
/// 26's and 25's helpers do not.
#[test]
fn the_binary_host_xuv_is_held_only_until_the_27_bump() {
    use crate::planetary::context::{
        BINARY_HOST_XUV_VERSION, BinaryHostXuvRules, with_binary_host_xuv,
    };
    assert!(
        GENERATOR_VERSION < BINARY_HOST_XUV_VERSION,
        "the 26 → 27 bump gives a paired star the X-ray and ultraviolet history of what the engine \
         made of it: delete `BINARY_HOST_XUV_VERSION`, `BinaryHostXuvRules` with its \
         `SingleModels` arm and its match in `SystemContext::xuv_histories`, the tests' \
         `with_binary_host_xuv`, its switch in `with_every_27_hold` (and the helper, if no other 27 \
         hold is left), and this test; restate the tests held below 27"
    );
    assert_eq!(
        BinaryHostXuvRules::in_force(),
        BinaryHostXuvRules::SingleModels
    );
    assert_eq!(
        with_binary_host_xuv(BinaryHostXuvRules::Engine, BinaryHostXuvRules::in_force),
        BinaryHostXuvRules::Engine
    );
    assert_eq!(
        with_every_27_hold(BinaryHostXuvRules::in_force),
        BinaryHostXuvRules::Engine
    );
    assert_eq!(
        with_every_27_hold(|| {
            with_binary_host_xuv(
                BinaryHostXuvRules::SingleModels,
                BinaryHostXuvRules::in_force,
            )
        }),
        BinaryHostXuvRules::SingleModels
    );
}

/// A star its companion swallowed in a common envelope emits no X-rays or ultraviolet after the
/// merger under the engine's rules: its luminosity is zero and the energy it delivered stays at
/// what it was then, its own closed form's, where the single models keep it shining.
#[test]
fn binary_host_xuv_of_a_star_merged_away_ends_at_the_merger() {
    use crate::planetary::context::{BinaryHostXuvRules, with_binary_host_xuv};
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
    let [single, engine] = [BinaryHostXuvRules::SingleModels, BinaryHostXuvRules::Engine]
        .map(|rules| with_binary_host_xuv(rules, || ctx.xuv_history(gone)));
    let au = Metres::from(AstronomicalUnits::new(1.0));
    let later = Years::new(merger.value() * 1.5);
    assert_same_bits(engine.luminosity(later).value(), 0.0);
    assert!(single.luminosity(later).value() > 0.0);
    assert_same_bits(
        engine.fluence(later, au, 0.0).value(),
        single.fluence(merger, au, 0.0).value(),
    );
    let before = Years::new(merger.value() * 0.5);
    assert_same_bits(
        engine.fluence(before, au, 0.0).value(),
        single.fluence(before, au, 0.0).value(),
    );
}

/// A merger on the main sequence, a blue straggler, shines in X-rays and ultraviolet from its
/// pair's first interaction as a main-sequence star of its new mass under the engine's rules, the
/// energy it has delivered never falling.
#[test]
fn binary_host_xuv_of_a_main_sequence_merger_is_its_product_s() {
    use crate::planetary::context::{BinaryHostXuvRules, with_binary_host_xuv};
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
    let engine = with_binary_host_xuv(BinaryHostXuvRules::Engine, || ctx.xuv_history(star));
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

/// A star of no pair the engine ran keeps its own X-ray and ultraviolet history under the engine's
/// rules, bit for bit.
#[test]
fn binary_host_xuv_leaves_an_unpaired_star_as_it_was() {
    use crate::planetary::context::{BinaryHostXuvRules, with_binary_host_xuv};
    let galaxy = galaxy();
    let records = sample_records(&galaxy, 200, SampleFilter::ALL).unwrap();
    let mut checked = 0;
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        let [single, engine] = [BinaryHostXuvRules::SingleModels, BinaryHostXuvRules::Engine]
            .map(|rules| with_binary_host_xuv(rules, || ctx.xuv_histories()));
        for (star, (a, b)) in single.iter().zip(&engine).enumerate() {
            let paired = u8::try_from(star)
                .ok()
                .and_then(|index| crate::stellar::system::pair_of(ctx.pairs(), index));
            if paired.is_none() {
                assert_eq!(a, b, "{:?} star {star}", ctx.id());
                checked += 1;
            }
        }
    }
    assert!(checked > 150, "{checked} unpaired stars");
}
