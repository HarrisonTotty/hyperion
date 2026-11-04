//! P14.T28.g's tests: the fate of the bodies of a host star of a pair the binary engine ran, from
//! its pair's timeline, on real systems of the sample.

use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::orbit::{Eccentricity, Orientation};
use crate::planetary::hosts::young::FormationDraws;
use crate::planetary::testing::{PairHistory, first_paired_system, with_every_27_hold};
use crate::stellar::system::SystemStars;
use crate::units::{AstronomicalUnits, Megayears, Radians};
use crate::{GENERATOR_VERSION, GeneratorVersion};

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

/// A common envelope engulfs the planets of either of its pair's stars: each is
/// `Destroyed { Engulfed }` by the envelope's onset, the smaller star's at that instant.
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
        let host = FateHost::evolved(&ctx, &zone);
        let ending = BodyFate::resolve(&body, &host).ending();
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
        at_onset += u32::from(when == at);
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
    assert!(paired >= 30, "{paired} paired systems");
}

/// The inner limit of the zone of pair `k`'s stars at its pair's state at the timeline's age `age`
/// (P14.T9.d): the pair's P-type fit then.
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
}

/// The binary hosts' census's universe (research-binary-hosts §1) at version 27, whose traces the
/// zones' edge cases come from (research-leans-35 §4).
fn census_galaxy() -> Galaxy {
    Galaxy::from_params_at_version(
        Seed::new(11),
        GalaxyParams::milky_way_like(),
        GeneratorVersion::new(27),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The context of `galaxy`'s system `raw` with its planets, as the census generates them.
fn census_system(
    galaxy: &Galaxy,
    raw: u64,
) -> (SystemContext, crate::planetary::system::PlanetarySystem) {
    let id = crate::id::SystemId::from_raw(raw).expect("a trace's ID");
    let ctx = SystemContext::for_system(galaxy, id).expect("a trace's system has stars");
    let system = crate::planetary::system::generate_planets(galaxy.seed(), &ctx);
    (ctx, system)
}

/// The primordial orbits of `system`'s planets about `zone`'s host.
fn planets_of<'s>(
    system: &'s crate::planetary::system::PlanetarySystem,
    zone: &OrbitZone,
) -> impl Iterator<Item = &'s FateBody> {
    let host = zone.host();
    system
        .planets()
        .filter(move |planet| planet.host() == host)
        .filter_map(crate::planetary::system::Body::fate)
}

/// A planet placed on its zone's inner limit stays in its zone while nothing moves the limit or
/// its orbit, and leaves it only once the limit lies past its orbit by more than
/// [`ZONE_EDGE_TOLERANCE`] of it (research-leans-35 §4, W1). `SystemId(0x41ffacb1ff800002)`'s
/// circumbinary planet lies on its zone's P-type limit of 1.166 au, on a circular orbit about a
/// pair whose mass holds to 3.25 Gyr. There the primary's wind turns on, and the pair's fit moves
/// out past the planet by 10⁻⁶ of it within 2 Myr: the fate unbound it a rounding past the limit.
#[test]
fn a_planet_on_its_zone_s_inner_limit_stays_while_nothing_moves_it() {
    use crate::planetary::placement::zones::with_binary_host_zones;
    let galaxy = census_galaxy();
    with_every_27_hold(|| {
        let (ctx, system) = census_system(&galaxy, 0x41ff_acb1_ff80_0002);
        let (zone, body) = system
            .zones()
            .iter()
            .find_map(|zone| {
                let inner = zone.inner()?;
                planets_of(&system, zone)
                    .find(|body| (body.orbit().periapsis() / inner - 1.0).abs() < 1e-12)
                    .map(|body| (zone, body))
            })
            .expect("a planet placed on its zone's inner limit");
        let (pair, _) = pair_of(ctx.pairs(), zone.members().next().unwrap()).unwrap();
        let timeline = pair.timeline();
        let host = FateHost::evolved(&ctx, zone);
        let fate = BodyFate::resolve(body, &host);
        let formed = fate.formed_at().unwrap();
        let mass = fate.host_mass(formed);
        // The last of the pair's knots before its mass moves by 10⁻⁶, the pair detached to it.
        let quiet = timeline
            .knot_ages(0.0, timeline.until().value())
            .into_iter()
            .filter_map(|age| clock_time(&ctx.stars()[0], Years::new(age)))
            .filter(|&t| t > formed && t <= UniverseTime::EPOCH)
            .take_while(|&t| (fate.host_mass(t) / mass - 1.0).abs() < 1e-6)
            .last()
            .unwrap();
        let age = ctx.stars()[0].age_at(quiet);
        assert!(
            age.value() > 3.25e9
                && timeline
                    .segments()
                    .iter()
                    .take_while(|segment| segment.start() <= age)
                    .all(|segment| segment.kind() == SegmentKind::Detached),
            "{:?}: the pair's mass or its orbit moves before 3.25 Gyr",
            ctx.id()
        );
        assert_eq!(
            fate.at(quiet).state(),
            BodyState::Present,
            "{:?}: {:?}",
            ctx.id(),
            fate.ending()
        );
        // Where it leaves, the limit lies past its pericentre by the tolerance's 10⁻⁹ of it, on the
        // orbit the zones at birth keep it on, and not by a rounding: half of it, a margin for the
        // arithmetic's own rounding at the bisected second.
        let Some(BodyState::Unbound { at }) = fate.ending() else {
            panic!(
                "{:?}: {:?} as the pair's fit moves out",
                ctx.id(),
                fate.ending()
            );
        };
        let periapsis = with_binary_host_zones(BinaryHostZonesRules::Birth, || {
            let host = FateHost::evolved(&ctx, zone);
            BodyFate::resolve(body, &host)
                .at(at)
                .orbit()
                .unwrap()
                .periapsis()
        });
        let inner = host.limits_at(zone, at).0.unwrap();
        assert!(
            at > quiet && inner / periapsis - 1.0 > 0.5e-9,
            "{:?}: unbound at {at:?} with the limit {} of its pericentre beyond it",
            ctx.id(),
            inner / periapsis - 1.0
        );
    });
}

/// A circumbinary planet that survives its pair's common envelope on a near-circular orbit stays
/// present after it (research-leans-35 §4, W2): the pair's own fit then bounds its zone from
/// inside, not the limit at birth widened by the envelope's loss.
/// `SystemId(0x41ffaca5ffc00008)`'s planet at 41.2 au, on a circular orbit about a pair that loses
/// under half its mass in a common envelope, was unbound at the envelope by its zone at birth.
#[test]
fn a_circumbinary_planet_that_survives_a_common_envelope_stays_present_after_it() {
    let galaxy = census_galaxy();
    with_every_27_hold(|| {
        let (ctx, system) = census_system(&galaxy, 0x41ff_aca5_ffc0_0008);
        let k = ctx
            .pairs()
            .iter()
            .position(|pair| {
                pair.timeline()
                    .segments()
                    .iter()
                    .any(|segment| segment.kind() == SegmentKind::CommonEnvelope)
            })
            .expect("a pair through a common envelope");
        let members = pair_stars(&ctx, k);
        let zone = system
            .zones()
            .iter()
            .find(|zone| zone.members().eq(members))
            .expect("the pair's own zone");
        let envelope = first_segment(&ctx, k, SegmentKind::CommonEnvelope);
        let at = clock_time(&ctx.stars()[0], Years::new(envelope)).unwrap();
        let (before, after) = masses_across(&ctx, k, envelope);
        let host = FateHost::evolved(&ctx, zone);
        let just = at.checked_sub(Span::from_seconds(1)).unwrap();
        let mut circular = 0;
        for body in planets_of(&system, zone) {
            let fate = BodyFate::resolve(body, &host);
            let Some(orbit) = fate.at(just).orbit().copied() else {
                continue;
            };
            if orbit.eccentricity().value() >= 0.01 {
                continue;
            }
            circular += 1;
            for t in [at, UniverseTime::EPOCH] {
                assert_eq!(
                    fate.at(t).state(),
                    BodyState::Present,
                    "{:?}: the planet at {} au through a loss from {before} to {after} M_sun: \
                     {:?}",
                    ctx.id(),
                    AstronomicalUnits::from(orbit.semi_major_axis()).value(),
                    fate.ending()
                );
            }
        }
        assert!(
            circular > 0 && after > 0.5 * before,
            "{:?}: {circular} near-circular planets through a loss from {before} to {after} M_sun",
            ctx.id()
        );
    });
}

/// A pair bounds its own zone from inside only while it has an orbit (research-leans-35 §4, W2):
/// just before a common envelope that ends in a merger the zone's inner limit is the pair's P-type
/// fit on its state then alone, not the limit at birth widened, and from the merger on there is
/// none, in the fate's reading of the zone and in [`PlanetarySystem::zones_at`]'s.
///
/// [`PlanetarySystem::zones_at`]: crate::planetary::system::PlanetarySystem::zones_at
#[test]
fn a_merged_pair_s_own_zone_has_no_inner_limit() {
    let galaxy = galaxy();
    let (stars, k) = first_paired_system(
        &galaxy,
        SEARCH,
        PairHistory::CommonEnvelopeMerger,
        |stars, k| has_pair_zone(&galaxy, stars, k),
    )
    .expect("a common-envelope merger with a zone about its pair");
    let ctx = SystemContext::from_stars(&galaxy, &stars);
    let zone = zone_of(&ctx, &pair_stars(&ctx, k));
    let timeline = ctx.pairs()[k].timeline();
    let merged = first_segment(&ctx, k, SegmentKind::Merged);
    let limit = |age: f64| {
        let state = timeline.state_at(Years::new(age));
        let widening = zone.host_mass().value() / state.total_mass().value();
        let fit = state.orbit().map(|orbit| {
            let masses = state.stars().map(|star| star.mass().value());
            orbit.semi_major_axis().value()
                * crate::planetary::placement::holman_wiegert_p_type(
                    masses[0].min(masses[1]) / (masses[0] + masses[1]),
                    orbit.eccentricity().value(),
                )
        });
        (zone.limits_at(&state, PairBound::Pair, widening).0, fit)
    };
    let (before, fit) = limit(first_segment(&ctx, k, SegmentKind::CommonEnvelope).next_down());
    // The pair's fit before the envelope, bit for bit.
    hyperion_testkit::float::assert_same_bits(
        before.expect("a P-type limit before the envelope").value(),
        fit.expect("a pair with an orbit before the envelope"),
    );
    assert_eq!(limit(merged), (None, None), "{:?}", ctx.id());
    let at = clock_time(&ctx.stars()[0], Years::new(merged)).unwrap();
    let system = crate::planetary::system::generate_planets(SEED, &ctx);
    with_every_27_hold(|| {
        assert_eq!(FateHost::evolved(&ctx, &zone).limits_at(&zone, at).0, None);
        let now = system
            .zones_at(&ctx, at)
            .into_iter()
            .find(|now| now.host() == zone.host())
            .expect("the pair's zone after the merger");
        assert_eq!(now.inner(), None, "{:?}", ctx.id());
    });
}
