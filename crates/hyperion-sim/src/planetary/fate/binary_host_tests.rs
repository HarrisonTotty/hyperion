//! P14.T28.g's tests: the fate of the bodies of a host star of a pair the binary engine ran, from
//! its pair's timeline (`BINARY_HOST_FATE_VERSION`), on real systems of the sample.

use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::orbit::{Eccentricity, Orientation};
use crate::planetary::hosts::young::FormationDraws;
use crate::planetary::testing::{PairHistory, first_paired_system, with_every_27_hold};
use crate::stellar::system::SystemStars;
use crate::units::{AstronomicalUnits, Megayears, Radians};

/// The system tests' universe.
const SEED: Seed = Seed::new(0x5eed_0014_0030);

/// How many of the sample's systems, nearest the Sun-like point first, the tests search for a
/// pair of a history.
const SEARCH: usize = 4_000;

fn galaxy() -> Galaxy {
    Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// A body of `mass` on a circular primordial orbit of `a_au` about `host_mass` M☉, formed at
/// 1 Myr, with no tides.
fn circular_body(a_au: f64, mass: EarthMasses, host_mass: SolarMasses) -> FateBody {
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::from(AstronomicalUnits::new(a_au)),
        GravitationalParameter::from_solar_masses(host_mass),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.2), Radians::new(1.1), Radians::new(0.4)).unwrap(),
        Radians::new(0.7),
    )
    .unwrap();
    let formation =
        Formation::from_draws(mass, Megayears::new(1.0), &FormationDraws::MEDIAN).unwrap();
    FateBody::new(formation, orbit, mass, KilogramsPerCubicMetre::new(5_513.0)).unwrap()
}

/// The zone of `ctx` about exactly the stars `members`, by body index, ascending.
fn zone_of(ctx: &SystemContext, members: &[u8]) -> OrbitZone {
    ctx.zones()
        .into_iter()
        .find(|zone| zone.members().eq(members.iter().copied()))
        .expect("a pair's stars each have a zone, and so does the pair")
}

/// Pair `k`'s two stars, by body index, ascending.
fn pair_stars(ctx: &SystemContext, k: usize) -> [u8; 2] {
    let mut stars = ctx.pairs()[k]
        .stars()
        .map(crate::stellar::multiplicity::StarIndex::get);
    stars.sort_unstable();
    stars
}

/// Pair `k`'s first stable transfer, with its accretor by body index, if it has one.
fn accretor(ctx: &SystemContext, k: usize) -> Option<(&crate::stellar::binary::Segment, u8)> {
    let pair = &ctx.pairs()[k];
    pair.timeline()
        .segments()
        .iter()
        .find_map(|segment| match segment.kind() {
            SegmentKind::StableTransfer { donor } => {
                Some((segment, pair.stars()[donor.other().index()].get()))
            }
            SegmentKind::Detached
            | SegmentKind::CommonEnvelope
            | SegmentKind::Merged
            | SegmentKind::Disrupted { .. }
            | SegmentKind::Contact => None,
        })
}

/// Whether the context of `stars` has a zone about exactly pair `k`'s two stars.
fn has_pair_zone(galaxy: &Galaxy, stars: &SystemStars, k: usize) -> bool {
    let ctx = SystemContext::from_stars(galaxy, stars);
    let members = pair_stars(&ctx, k);
    ctx.zones()
        .iter()
        .any(|zone| zone.members().eq(members.iter().copied()))
}

/// The start of the first segment of `kind` on pair `k`'s timeline, Julian years.
fn first_segment(ctx: &SystemContext, k: usize, kind: SegmentKind) -> f64 {
    ctx.pairs()[k]
        .timeline()
        .segments()
        .iter()
        .find(|s| s.kind() == kind)
        .expect("the pair has the segment")
        .start()
        .value()
}

/// The pair's total mass just before and just after its timeline's age `age`, M☉.
fn masses_across(ctx: &SystemContext, k: usize, age: f64) -> (f64, f64) {
    let timeline = ctx.pairs()[k].timeline();
    let mass = |age: f64| timeline.state_at(Years::new(age)).total_mass().value();
    (mass(age.next_down()), mass(age))
}

/// The paired hosts' fate is held until the 26 → 27 bump: the rules in force are the single
/// models', the tests' switch and 27's helper reach the engine's, and 26's and 25's helpers do not.
#[test]
fn the_binary_host_fate_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < BINARY_HOST_FATE_VERSION,
        "the 26 → 27 bump reads a paired host's fate from its pair's timeline: delete \
         `BINARY_HOST_FATE_VERSION`, `BinaryHostFateRules` with its `SingleModels` arm and its \
         match in `FateHost::evolved`, the tests' `with_binary_host_fate`, its switch in \
         `with_every_27_hold` (and the helper, if no other 27 hold is left), and this test; \
         restate the tests held below 27"
    );
    assert_eq!(
        BinaryHostFateRules::in_force(),
        BinaryHostFateRules::SingleModels
    );
    assert_eq!(
        with_binary_host_fate(BinaryHostFateRules::Engine, BinaryHostFateRules::in_force),
        BinaryHostFateRules::Engine
    );
    assert_eq!(
        with_every_27_hold(BinaryHostFateRules::in_force),
        BinaryHostFateRules::Engine
    );
    assert_eq!(
        with_every_27_hold(|| {
            with_binary_host_fate(
                BinaryHostFateRules::SingleModels,
                BinaryHostFateRules::in_force,
            )
        }),
        BinaryHostFateRules::SingleModels
    );
}

/// A common envelope engulfs the planets of either of its pair's stars: under the engine's rules
/// each is `Destroyed { Engulfed }` by the envelope's onset, the smaller star's at that instant,
/// where the single models keep the companion's.
#[test]
fn a_common_envelope_engulfs_the_planets_of_either_star() {
    let galaxy = galaxy();
    let (stars, k) = first_paired_system(
        &galaxy,
        SEARCH,
        PairHistory::CommonEnvelopeMerger,
        |_, _| true,
    )
    .expect("a common-envelope merger among the sample's first systems");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let envelope = first_segment(&ctx, k, SegmentKind::CommonEnvelope);
    let at = clock_time(&ctx.stars()[0], Years::new(envelope)).unwrap();
    let mut at_onset = 0;
    for star in pair_stars(&ctx, k) {
        let zone = zone_of(&ctx, &[star]);
        let reach = zone
            .outer()
            .map_or(0.05, |outer| 0.5 * AstronomicalUnits::from(outer).value());
        let body = circular_body(reach, EarthMasses::new(1.0), zone.host_mass());
        let ending = with_binary_host_fate(BinaryHostFateRules::Engine, || {
            let host = FateHost::evolved(&ctx, &zone);
            BodyFate::resolve(&body, &host).ending()
        });
        let Some(BodyState::Destroyed {
            cause: DestructionCause::Engulfed,
            at: when,
        }) = ending
        else {
            panic!("{:?} star {star}: {ending:?} at the envelope", ctx.id());
        };
        assert!(
            when <= at,
            "{:?} star {star}: engulfed after the envelope",
            ctx.id()
        );
        if when == at {
            at_onset += 1;
            let single = with_binary_host_fate(BinaryHostFateRules::SingleModels, || {
                let host = FateHost::evolved(&ctx, &zone);
                BodyFate::resolve(&body, &host).at(at).state()
            });
            assert_eq!(single, BodyState::Present, "{:?} star {star}", ctx.id());
        }
    }
    assert!(
        at_onset >= 1,
        "{:?}: no planet lived to the envelope",
        ctx.id()
    );
}

/// A circumbinary planet takes a common envelope's loss of its pair's mass as a sudden step: on
/// a circular orbit it is `Unbound` where the pair loses more than half its mass at once, and
/// otherwise stays bound with e = ΔM ÷ `M_after` (Hadjidemetriou 1963).
#[test]
fn a_circumbinary_planet_takes_a_common_envelope_as_a_sudden_loss() {
    let galaxy = galaxy();
    let lost = |most: bool| {
        let galaxy = &galaxy;
        move |stars: &SystemStars, k: usize| {
            let pair = &stars.pairs()[k];
            let timeline = pair.timeline();
            let Some(envelope) = timeline
                .segments()
                .iter()
                .find(|s| s.kind() == SegmentKind::CommonEnvelope)
            else {
                return false;
            };
            let age = envelope.start().value();
            let mass = |age: f64| timeline.state_at(Years::new(age)).total_mass().value();
            let kept = mass(age) / mass(age.next_down());
            kept > 0.0 && (kept < 0.5) == most && has_pair_zone(galaxy, stars, k)
        }
    };
    for (history, most) in [
        (PairHistory::CommonEnvelopeMerger, true),
        (PairHistory::PostCommonEnvelope, false),
    ] {
        let (stars, k) = first_paired_system(&galaxy, SEARCH, history, lost(most))
            .unwrap_or_else(|| panic!("a {history:?} pair among the sample's first systems"));
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        let members = pair_stars(&ctx, k);
        let zone = zone_of(&ctx, &members);
        let envelope = first_segment(&ctx, k, SegmentKind::CommonEnvelope);
        let at = clock_time(&ctx.stars()[0], Years::new(envelope)).unwrap();
        let (before, after) = masses_across(&ctx, k, envelope);
        let a_au = 40.0
            * zone
                .inner()
                .map_or(1.0, |inner| AstronomicalUnits::from(inner).value());
        let body = circular_body(a_au, EarthMasses::new(1.0), zone.host_mass());
        with_binary_host_fate(BinaryHostFateRules::Engine, || {
            let host = FateHost::evolved(&ctx, &zone);
            let fate = BodyFate::resolve(&body, &host);
            if most {
                assert_eq!(
                    fate.ending(),
                    Some(BodyState::Unbound { at }),
                    "{:?}",
                    ctx.id()
                );
            } else {
                let now = fate.at(at);
                assert_eq!(now.state(), BodyState::Present, "{:?}", ctx.id());
                let e = now.orbit().unwrap().eccentricity().value();
                let expected = before / after - 1.0;
                assert!(
                    (e - expected).abs() < 1e-9,
                    "{:?}: e {e} after a loss from {before} to {after} M_sun",
                    ctx.id()
                );
            }
        });
    }
}

/// Between its pair's events a planet's orbit follows the engine's mass of its host by a M =
/// constant (Veras et al. 2011, eq. 18), to 10⁻¹², through a stable transfer onto its star, and
/// so moves in where the star gains mass.
#[test]
fn a_paired_host_s_planet_keeps_a_m_between_its_events() {
    let galaxy = galaxy();
    let roomy = |stars: &SystemStars, k: usize| {
        let ctx = SystemContext::from_stars(&galaxy, stars);
        accretor(&ctx, k).is_some_and(|(_, star)| {
            ctx.zones().iter().any(|zone| {
                zone.members().eq([star])
                    && zone
                        .outer()
                        .is_some_and(|outer| AstronomicalUnits::from(outer).value() > 0.5)
            })
        })
    };
    let (stars, k) = first_paired_system(&galaxy, SEARCH, PairHistory::StableTransfer, roomy)
        .expect("a stable transfer onto a star with an S-type zone past 0.5 au");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let first = &ctx.stars()[0];
    let (transfer, accretor) = accretor(&ctx, k).expect("found with one");
    let zone = zone_of(&ctx, &[accretor]);
    let a_au = zone
        .outer()
        .map_or(0.1, |outer| 0.2 * AstronomicalUnits::from(outer).value());
    let body = circular_body(a_au, EarthMasses::new(1.0), zone.host_mass());
    with_binary_host_fate(BinaryHostFateRules::Engine, || {
        let host = FateHost::evolved(&ctx, &zone);
        let fate = BodyFate::resolve(&body, &host);
        let a0m0 = body.orbit.semi_major_axis().value() * zone.host_mass().value();
        let (onset, end) = (transfer.start().value(), transfer.end().value());
        let mut checked = 0;
        for i in 0..=20 {
            let age = onset + (end - onset) * f64::from(i) / 20.0;
            let Some(t) = clock_time(first, Years::new(age)) else {
                continue;
            };
            let now = fate.at(t);
            if now.state() != BodyState::Present {
                continue;
            }
            let am = now.orbit().unwrap().semi_major_axis().value() * fate.host_mass(t).value();
            assert!(
                (am / a0m0 - 1.0).abs() < 1e-12,
                "{:?} at {age} yr: a M off by {}",
                ctx.id(),
                am / a0m0 - 1.0
            );
            checked += 1;
        }
        assert!(
            checked > 0,
            "{:?}: no present time in the transfer",
            ctx.id()
        );
        // Across the transfer the orbit moves against the mass: in where the star gains.
        let [from, to] = [onset, end].map(|age| clock_time(first, Years::new(age)).unwrap());
        if fate.at(to).state() == BodyState::Present {
            let axis = |t| fate.at(t).orbit().unwrap().semi_major_axis().value();
            let (m_from, m_to) = (fate.host_mass(from).value(), fate.host_mass(to).value());
            assert_eq!(axis(to) < axis(from), m_to > m_from, "{:?}", ctx.id());
        }
    });
}

/// Under the engine's rules each body of every zone of the sample's paired systems keeps the
/// sequence not yet formed → present → gone, at times through the system's life to the clock
/// window's end.
#[test]
fn the_states_of_a_paired_host_s_bodies_stay_a_prefix() {
    use hyperion_testkit::lcg::Lcg;
    let galaxy = galaxy();
    let rank = |state: BodyState| match state {
        BodyState::NotYetFormed => 0,
        BodyState::Present => 1,
        BodyState::Destroyed { .. } | BodyState::Unbound { .. } => 2,
    };
    let records = crate::planetary::testing::sample_records(
        &galaxy,
        800,
        crate::planetary::testing::SampleFilter::ALL,
    )
    .unwrap();
    let mut lcg = Lcg::new(0x28_9000);
    let mut paired = 0;
    with_binary_host_fate(BinaryHostFateRules::Engine, || {
        for record in &records {
            let stars = SystemStars::generate(&galaxy, record);
            if stars.pairs().is_empty() {
                continue;
            }
            let ctx = SystemContext::from_stars(&galaxy, &stars);
            let age_end = ctx.age_at(ClockWindow::END).value();
            let times: Vec<UniverseTime> = (1..=40)
                .filter_map(|i| {
                    let age = age_end * f64::from(i) / 40.0;
                    clock_time(&ctx.stars()[0], Years::new(age))
                })
                .collect();
            for zone in ctx.zones() {
                let host = FateHost::evolved(&ctx, &zone);
                let bodies: Vec<FateBody> = [0.05, 0.5, 5.0, 50.0]
                    .iter()
                    .map(|&a| {
                        let mass = EarthMasses::new(crate::math::exp10(3.0 * lcg.next_f64()));
                        circular_body(a, mass, zone.host_mass())
                    })
                    .collect();
                let refs: Vec<&FateBody> = bodies.iter().collect();
                for fate in BodyFate::resolve_all(&refs, &host) {
                    let ranks: Vec<u8> = times.iter().map(|&t| rank(fate.at(t).state())).collect();
                    assert!(
                        ranks.windows(2).all(|w| w[0] <= w[1]),
                        "{:?}: {ranks:?}",
                        ctx.id()
                    );
                }
            }
            paired += 1;
        }
    });
    assert!(paired >= 30, "{paired} paired systems");
}

/// The zone of pair `k`'s stars at birth with its limits at its pair's state at the timeline's age
/// `age` (P14.T9.d), widened by the pair's mass at birth over its mass then.
fn pair_limits_at(ctx: &SystemContext, k: usize, zone: &OrbitZone, age: f64) -> Option<Metres> {
    let state = ctx.pairs()[k].timeline().state_at(Years::new(age));
    let widening = zone.host_mass().value() / state.total_mass().value();
    zone.limits_at(&state, PairBound::Pair, widening).0
}

/// A stable transfer that widens its pair moves the pair's P-type limit out past a circumbinary
/// planet placed just outside it at birth: under the zones at the evaluated state the planet is
/// `Unbound` at the crossing, inside the transfer, where under the zones at birth it is present
/// then.
#[test]
fn stable_transfer_s_widening_unbinds_an_inner_circumbinary_planet() {
    use crate::planetary::placement::zones::{BinaryHostZonesRules, with_binary_host_zones};
    let galaxy = galaxy();
    let widens = |stars: &SystemStars, k: usize| {
        let ctx = SystemContext::from_stars(&galaxy, stars);
        let members = pair_stars(&ctx, k);
        let Some(zone) = ctx
            .zones()
            .into_iter()
            .find(|zone| zone.members().eq(members.iter().copied()))
        else {
            return false;
        };
        let Some((transfer, _)) = accretor(&ctx, k) else {
            return false;
        };
        let birth = zone.inner().map_or(0.0, Metres::value);
        pair_limits_at(&ctx, k, &zone, transfer.end().value())
            .is_some_and(|inner| inner.value() > 1.2 * birth)
    };
    let (stars, k) = first_paired_system(&galaxy, SEARCH, PairHistory::StableTransfer, widens)
        .expect("a stable transfer that widens its pair's P-type limit by a fifth");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let members = pair_stars(&ctx, k);
    let zone = zone_of(&ctx, &members);
    let (transfer, _) = accretor(&ctx, k).unwrap();
    let inner = AstronomicalUnits::from(zone.inner().unwrap()).value();
    let body = circular_body(1.1 * inner, EarthMasses::new(1.0), zone.host_mass());
    let first = &ctx.stars()[0];
    let [onset, end] =
        [transfer.start(), transfer.end()].map(|age| clock_time(first, age).unwrap());
    with_binary_host_fate(BinaryHostFateRules::Engine, || {
        // A host is read under the rules in force when it is built, as a system's is.
        let ending = with_binary_host_zones(BinaryHostZonesRules::Evaluated, || {
            let host = FateHost::evolved(&ctx, &zone);
            BodyFate::resolve(&body, &host).ending()
        });
        let Some(BodyState::Unbound { at }) = ending else {
            panic!("{:?}: {ending:?} as its pair widens", ctx.id());
        };
        assert!(
            onset < at && at <= end,
            "{:?}: unbound outside the transfer",
            ctx.id()
        );
        // The zones at birth keep it at the crossing, whatever its host's radii do to it later.
        let birth = with_binary_host_zones(BinaryHostZonesRules::Birth, || {
            let host = FateHost::evolved(&ctx, &zone);
            BodyFate::resolve(&body, &host).at(at).state()
        });
        assert_eq!(birth, BodyState::Present, "{:?}", ctx.id());
    });
}

/// A common envelope shrinks its pair's orbit, and the S-type zones its stars had at birth close
/// with it: after the envelope each is smaller than its zone at birth, widened as its planets'
/// orbits are, and the donor's, which the envelope stripped, is under a fifth of it.
#[test]
fn a_post_common_envelope_pair_s_s_type_zones_close() {
    use crate::planetary::placement::zones::{BinaryHostZonesRules, with_binary_host_zones};
    let galaxy = galaxy();
    let roomy = |stars: &SystemStars, k: usize| {
        let ctx = SystemContext::from_stars(&galaxy, stars);
        let members = pair_stars(&ctx, k);
        members
            .iter()
            .all(|&star| ctx.zones().iter().any(|zone| zone.members().eq([star])))
    };
    let (stars, k) = first_paired_system(&galaxy, SEARCH, PairHistory::PostCommonEnvelope, roomy)
        .expect("a post-common-envelope pair with both S-type zones");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let system = crate::planetary::system::generate_planets(SEED, &ctx);
    let envelope = first_segment(&ctx, k, SegmentKind::CommonEnvelope);
    let after = clock_time(&ctx.stars()[0], Years::new(envelope + 1.0e3)).unwrap();
    with_binary_host_fate(BinaryHostFateRules::Engine, || {
        let (at_birth, evaluated) = (
            with_binary_host_zones(BinaryHostZonesRules::Birth, || system.zones_at(&ctx, after)),
            with_binary_host_zones(BinaryHostZonesRules::Evaluated, || {
                system.zones_at(&ctx, after)
            }),
        );
        let timeline = ctx.pairs()[k].timeline();
        let lost = |component: crate::stellar::binary::Component| {
            let mass = |age: f64| {
                timeline.state_at(Years::new(age)).stars()[component.index()]
                    .mass()
                    .value()
            };
            mass(envelope.next_down()) - mass(envelope)
        };
        let donor = if lost(crate::stellar::binary::Component::Primary)
            > lost(crate::stellar::binary::Component::Secondary)
        {
            ctx.pairs()[k].stars()[0].get()
        } else {
            ctx.pairs()[k].stars()[1].get()
        };
        for star in pair_stars(&ctx, k) {
            let find = |zones: &[OrbitZone]| {
                zones
                    .iter()
                    .find(|zone| zone.members().eq([star]))
                    .and_then(OrbitZone::outer)
                    .unwrap()
                    .value()
            };
            let (birth, now) = (find(&at_birth), find(&evaluated));
            let share = if star == donor { 0.2 } else { 1.0 };
            assert!(
                now < share * birth,
                "{:?} star {star}: the S-type zone is {now} m against {birth} m",
                ctx.id()
            );
        }
    });
}

/// The binary hosts' zones are held until the 26 → 27 bump: the rules in force are the birth
/// zones', the tests' switch and 27's helper reach the evaluated zones, and 26's and 25's helpers
/// do not.
#[test]
fn the_binary_host_zones_are_held_only_until_the_27_bump() {
    use crate::planetary::placement::zones::{
        BINARY_HOST_ZONES_VERSION, BinaryHostZonesRules, with_binary_host_zones,
    };
    assert!(
        GENERATOR_VERSION < BINARY_HOST_ZONES_VERSION,
        "the 26 → 27 bump holds a binary host's bodies to their zones at its pair's evaluated \
         state: delete `BINARY_HOST_ZONES_VERSION`, `BinaryHostZonesRules` with its `Birth` arm \
         and its matches in `FateHost::limits_at` and `BodyFate::crossing`, the tests' \
         `with_binary_host_zones`, its switch in `with_every_27_hold` (and the helper, if no other \
         27 hold is left), and this test; restate the tests held below 27"
    );
    assert_eq!(
        BinaryHostZonesRules::in_force(),
        BinaryHostZonesRules::Birth
    );
    assert_eq!(
        with_binary_host_zones(
            BinaryHostZonesRules::Evaluated,
            BinaryHostZonesRules::in_force
        ),
        BinaryHostZonesRules::Evaluated
    );
    assert_eq!(
        with_every_27_hold(BinaryHostZonesRules::in_force),
        BinaryHostZonesRules::Evaluated
    );
    assert_eq!(
        with_every_27_hold(|| {
            with_binary_host_zones(BinaryHostZonesRules::Birth, BinaryHostZonesRules::in_force)
        }),
        BinaryHostZonesRules::Birth
    );
}

/// A star a supernova releases from its system (P11.T10) leaves with its planets: under the zones'
/// rules at 27 a planet of its own zone is `Unbound` from the system at the release, where the
/// zones at birth keep it, and the system's zones after the release hold no zone of it.
#[test]
fn a_released_star_s_planets_leave_with_it() {
    use crate::planetary::placement::zones::{BinaryHostZonesRules, with_binary_host_zones};
    use crate::planetary::testing::{SampleFilter, sample_records};
    let galaxy = Galaxy::from_params_at_version(
        SEED,
        GalaxyParams::milky_way_like(),
        crate::GeneratorVersion::new(27),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let records = sample_records(&galaxy, SEARCH, SampleFilter::ALL).unwrap();
    let (ctx, star, left) = records
        .iter()
        .filter(|record| record.kind() != crate::galaxy::placement::SystemKind::RoguePlanet)
        .find_map(|record| {
            let stars = SystemStars::generate(&galaxy, record);
            let ctx = SystemContext::from_stars(&galaxy, &stars);
            let (star, left) = (0..ctx.stars().len()).find_map(|star| {
                let left = ctx.departure(star)?;
                let roomy = ctx.zones().iter().any(|zone| {
                    zone.members().eq([u8::try_from(star).unwrap()])
                        && zone
                            .outer()
                            .is_none_or(|outer| AstronomicalUnits::from(outer).value() > 0.5)
                });
                (roomy && left < ctx.age_at_epoch()).then_some((star, left))
            })?;
            Some((ctx, star, left))
        })
        .expect("a star released before the epoch, with an S-type zone, in the sample");
    let star = u8::try_from(star).unwrap();
    let zone = zone_of(&ctx, &[star]);
    let a_au = zone
        .outer()
        .map_or(1.0, |outer| 0.3 * AstronomicalUnits::from(outer).value());
    let body = circular_body(a_au, EarthMasses::new(1.0), zone.host_mass());
    let at = clock_time(&ctx.stars()[0], left).unwrap();
    let system = crate::planetary::system::generate_planets(SEED, &ctx);
    with_binary_host_fate(BinaryHostFateRules::Engine, || {
        let evaluated = with_binary_host_zones(BinaryHostZonesRules::Evaluated, || {
            let host = FateHost::evolved(&ctx, &zone);
            let zones = system.zones_at(&ctx, UniverseTime::EPOCH);
            assert!(
                zones.iter().all(|zone| !zone.members().any(|m| m == star)),
                "{:?}: a zone of the released star {star} at the epoch",
                ctx.id()
            );
            BodyFate::resolve(&body, &host).ending()
        });
        let ended = evaluated.and_then(|ending| ending.ended_at());
        assert!(
            ended.is_some_and(|ended| ended <= at),
            "{:?}: {evaluated:?}",
            ctx.id()
        );
        if ended == Some(at) {
            assert_eq!(evaluated, Some(BodyState::Unbound { at }), "{:?}", ctx.id());
        }
        let birth = with_binary_host_zones(BinaryHostZonesRules::Birth, || {
            let host = FateHost::evolved(&ctx, &zone);
            BodyFate::resolve(&body, &host).at(at).state()
        });
        if ended == Some(at) {
            assert_eq!(birth, BodyState::Present, "{:?}", ctx.id());
        }
    });
}
