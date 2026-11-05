//! The binary hosts' census (research-binary-hosts §1): the planets of the sample's systems whose
//! zones hold a star of a pair the binary engine ran, classed by the worst of those pairs'
//! histories before the epoch ([`PairHistory`]), and what each reads of its stars against what
//! the engine says of them (P14.T1.e, P14.T28.g and P14.T9.d).
//!
//! The galaxies are version 27's, the systems the [`SYSTEMS`] nearest the Sun-like point. Each planet
//! of such a zone is counted, and each present at the epoch on the fate in force read: the light
//! of its zone's stars as the stage reads it ([`Epoch::luminosity`]) and the mass its fate orbits
//! ([`BodyFate::host_mass`]), each against the sum of the engine's states of the same stars
//! ([`SystemStars::state_at`]); an S-type planet of a pair that went through a common envelope; and
//! a planet whose zone's stars the engine has left with no mass; a planet that lay outside its
//! zone at its pair's state at one of the pair's segment boundaries since it formed
//! ([`OrbitZone::limits_at`]); and a planet of a zone that holds a star a supernova released from
//! the system (P11.T10), which goes with its star or is unbound. The census runs under the
//! version's rules and prints its counts. A zone that holds a released star is read for the last
//! two columns alone, its stars' states not being the engine's to compare.
//!
//! Each census also counts, by cause, the planets unbound by the zones at their pairs' evaluated
//! states alone ([`ZoneLoss`]), and the planets placed on their zone's P-type limit, which the
//! placement margin keeps off it from 28 (research-leans-37 §1).
//!
//! The test is native only, as the other censuses are.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use super::*;
use crate::GeneratorVersion;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{Existence, SystemKind, SystemRecord};
use crate::planetary::placement::zones::{PairBound, ZONE_EDGE_TOLERANCE};
use crate::planetary::testing::{PairHistory, SampleFilter, sample_records, with_every_28_hold};
use crate::stellar::system::SystemStars;
use crate::stellar::system::{PairTimeline, SystemState, pair_of};

/// The census's systems in each universe, nearest the Sun-like point first: research-binary-hosts'
/// 10⁵.
const SYSTEMS: usize = 100_000;

/// The universes the census reads: research-binary-hosts' first of its five (11–15).
const GALAXY_SEEDS: [u64; 1] = [11];

/// How far a zone's light read may lie from the engine's, as a share, before the census counts it
/// off: 10%, research-binary-hosts' column, a label of the census's.
const LIGHT_TOLERANCE: f64 = 0.10;

/// How far the mass a planet orbits may lie from the engine's, as a share, before the census
/// counts it off: 5%, research-binary-hosts' column.
const MASS_TOLERANCE: f64 = 0.05;

/// The share within which a read counts as the engine's: 10⁻¹², the rounding of a sum of a few
/// luminosities taken in another order.
const UNEQUAL: f64 = 1e-12;

/// One history's counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Row {
    /// The systems with a planet in a zone of the history.
    systems: BTreeSet<SystemId>,
    /// The planets in such zones.
    planets: usize,
    /// Those present at the epoch.
    present: usize,
    /// Those unbound by the epoch.
    unbound: usize,
    /// Those destroyed by the epoch.
    destroyed: usize,
    /// Present planets whose zone's light read lies off the engine's ([`LIGHT_TOLERANCE`]).
    light_off: usize,
    /// Present planets whose zone's light read is not the engine's to [`UNEQUAL`]: the light
    /// ratio not 1.
    light_unequal: usize,
    /// Present planets whose orbited mass lies off the engine's ([`MASS_TOLERANCE`]).
    mass_off: usize,
    /// Present S-type planets of a star whose pair went through a common envelope.
    enveloped: usize,
    /// Present planets whose zone's stars the engine has left with no mass.
    massless: usize,
    /// Present planets that lay outside their zone at its pair's evaluated state at one of the
    /// pair's segment boundaries before the epoch, after they formed (P14.T9.d).
    outside: usize,
    /// Present planets of a zone that holds a star the system no longer shows (P11.T10).
    released: usize,
}

/// Why a planet unbound at the epoch is unbound by its zone at its pair's evaluated state alone
/// (P14.T9.d): no star of its zone has left the system (P11.T10). A crossing's limit is the one nearer the orbit a second before, or
/// the orbit as placed for a body that left as it formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ZoneLoss {
    /// Its pericentre crossed the inner limit, and it was placed with its pericentre on its zone's
    /// limit: the pair's fit drifting past it (research-leans-37 §1).
    InnerEdge,
    /// Its pericentre crossed the inner limit, placed off it: a fit that grew past it.
    Inner,
    /// Its apocentre crossed the outer limit, and it was placed with its apocentre on its zone's.
    OuterEdge,
    /// Its apocentre crossed the outer limit, placed off it.
    Outer,
    /// Alone, the zone keeps it: scattered after a sibling left (ruling 71).
    Scattered,
}

/// The census's counts, by history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Census {
    rows: BTreeMap<PairHistory, Row>,
    /// Systems the stellar stage shows fewer of their stars at the epoch than it drew (P11.T10's
    /// released stars).
    released: usize,
    /// The planets unbound by their zones at their pairs' evaluated states alone, by cause.
    zone_losses: BTreeMap<ZoneLoss, usize>,
    /// The planets placed with their pericentre on their zone's P-type limit, to
    /// [`ZONE_EDGE_TOLERANCE`] of it.
    on_the_limit: usize,
}

impl Census {
    /// Every row's counts summed.
    fn total(&self) -> Row {
        self.rows.values().fold(Row::default(), |mut sum, row| {
            sum.systems.extend(row.systems.iter().copied());
            sum.planets += row.planets;
            sum.present += row.present;
            sum.unbound += row.unbound;
            sum.destroyed += row.destroyed;
            sum.light_off += row.light_off;
            sum.light_unequal += row.light_unequal;
            sum.mass_off += row.mass_off;
            sum.enveloped += row.enveloped;
            sum.massless += row.massless;
            sum.outside += row.outside;
            sum.released += row.released;
            sum
        })
    }

    /// The census as a table, a row a history and a total.
    fn table(&self, title: &str) -> String {
        let mut out = format!(
            "{title}\n{:<26} {:>7} {:>8} {:>8} {:>8} {:>9} {:>9} {:>7} {:>8} {:>9} {:>8} {:>7} {:>8}\n",
            "history",
            "systems",
            "planets",
            "present",
            "unbound",
            "destroyed",
            "light off",
            "light ≠",
            "mass off",
            "enveloped",
            "massless",
            "outside",
            "released"
        );
        let line = |out: &mut String, name: &str, row: &Row| {
            writeln!(
                out,
                "{name:<26} {:>7} {:>8} {:>8} {:>8} {:>9} {:>9} {:>7} {:>8} {:>9} {:>8} {:>7} {:>8}",
                row.systems.len(),
                row.planets,
                row.present,
                row.unbound,
                row.destroyed,
                row.light_off,
                row.light_unequal,
                row.mass_off,
                row.enveloped,
                row.massless,
                row.outside,
                row.released
            )
            .expect("a string takes every write");
        };
        for (history, row) in &self.rows {
            line(&mut out, &format!("{history:?}"), row);
        }
        line(&mut out, "total", &self.total());
        writeln!(out, "systems with a released star: {}", self.released)
            .expect("a string takes every write");
        writeln!(
            out,
            "unbound by the zone check alone: {} {:?}; placed on a P-type limit: {}",
            self.zone_losses.values().sum::<usize>(),
            self.zone_losses,
            self.on_the_limit
        )
        .expect("a string takes every write");
        out
    }
}

/// Whether `read` lies off `engine` by more than the share `tolerance`: any read at all where the
/// engine gives nothing.
fn off(read: f64, engine: f64, tolerance: f64) -> bool {
    if engine > 0.0 {
        (read / engine - 1.0).abs() > tolerance
    } else {
        read > 0.0
    }
}

/// The pair the binary engine ran that bounds `zone` of `ctx` at its evaluated state, with how: a
/// star's zone of the pair, or the pair's own (P14.T9.d).
fn zone_bound<'c>(
    ctx: &'c SystemContext,
    zone: &OrbitZone,
) -> Option<(&'c PairTimeline, PairBound)> {
    let members: Vec<u8> = zone.members().collect();
    let (pair, component) = pair_of(ctx.pairs(), members[0])?;
    match members.as_slice() {
        [_] => Some((pair, PairBound::Star(component))),
        [_, second] if pair.stars().iter().any(|s| s.get() == *second) => {
            Some((pair, PairBound::Pair))
        }
        _ => None,
    }
}

/// Whether the body of `fate` in `zone` of `ctx`, which `pair` bounds as `bound`, lay outside the
/// zone at its pair's evaluated state at one of the pair's segment boundaries after it formed and
/// at or before `t`: by the fate's own reading where its host holds the bound, and otherwise at
/// the segment's opening state, past the fate's [`ZONE_EDGE_TOLERANCE`] of a limit.
fn was_outside(
    ctx: &SystemContext,
    zone: &OrbitZone,
    (pair, bound): (&PairTimeline, PairBound),
    fate: &BodyFate<'_>,
    t: UniverseTime,
) -> bool {
    let formed = fate.formed_at().expect("a present body has formed");
    pair.timeline().segments().iter().any(|segment| {
        let Some(at) = time_at_age(segment.start(), ctx.age_at_epoch()) else {
            return false;
        };
        if at <= formed || at > t {
            return false;
        }
        if let Some(clearance) = fate.zone_clearance_at(at) {
            return clearance < 0.0;
        }
        let Some(orbit) = fate.at(at).orbit().copied() else {
            return false;
        };
        let mass = fate.host_mass(at).value();
        if mass <= 0.0 {
            return false;
        }
        let state = pair.timeline().state_at(segment.start());
        let (inner, outer) = zone.limits_at(&state, bound, zone.host_mass().value() / mass);
        // Past the fate's own easing of each limit.
        outer.is_some_and(|r| orbit.apoapsis() > r * (1.0 + ZONE_EDGE_TOLERANCE))
            || inner.is_some_and(|r| orbit.periapsis() < r * (1.0 - ZONE_EDGE_TOLERANCE))
    })
}

/// Why `zone_bodies[me]`, a planet of `zone` of `ctx` about `host` unbound at `t` under the rules
/// in force, is unbound by its zone at its pair's evaluated state alone, if it is ([`ZoneLoss`]).
fn zone_loss(
    ctx: &SystemContext,
    zone: &OrbitZone,
    host: &FateHost<'_>,
    (zone_bodies, me): (&[&FateBody], usize),
    t: UniverseTime,
) -> Option<ZoneLoss> {
    if zone
        .members()
        .any(|member| ctx.has_left(usize::from(member), t))
    {
        return None;
    }
    let body = zone_bodies[me];
    let alone = BodyFate::resolve(body, host);
    let BodyState::Unbound { at } = alone.at(t).state() else {
        return Some(ZoneLoss::Scattered);
    };
    // The orbit a second before it left, or as placed where it left as it formed.
    let just = at
        .checked_sub(Span::from_seconds(1))
        .expect("a second before a body leaves is a time");
    let placed = body.orbit();
    let orbit = alone.at(just).orbit().copied().unwrap_or(*placed);
    let (inner, outer) = host.limits_at(zone, at);
    let inside = inner.map_or(f64::INFINITY, |r| orbit.periapsis() / r - 1.0);
    let outside = outer.map_or(f64::INFINITY, |r| 1.0 - orbit.apoapsis() / r);
    let on = |x: Metres, limit: Option<Metres>| {
        limit.is_some_and(|r| (x / r - 1.0).abs() <= ZONE_EDGE_TOLERANCE)
    };
    Some(if inside <= outside {
        if on(placed.periapsis(), zone.inner()) {
            ZoneLoss::InnerEdge
        } else {
            ZoneLoss::Inner
        }
    } else if on(placed.apoapsis(), zone.outer()) {
        ZoneLoss::OuterEdge
    } else {
        ZoneLoss::Outer
    })
}

/// The engine's state at `t` of each star of `stars`, by body index, from `state`, the system's
/// then: `None` for a star the system no longer shows (P11.T10).
fn engine_states(
    stars: &SystemStars,
    state: &SystemState,
    t: UniverseTime,
) -> Vec<Option<StarState>> {
    let shown = stars.shown_at(t);
    let mut states = state.stars().iter();
    (0_u8..)
        .take(stars.stars().len())
        .map(|index| {
            shown
                .holds(index)
                .then(|| *states.next().expect("one a shown star"))
        })
        .collect()
}

/// Each pair the engine ran of `ctx`, its stars a bit each with its history, where its timeline
/// holds `t`.
fn pair_histories(ctx: &SystemContext, t: UniverseTime) -> Vec<(u32, PairHistory)> {
    ctx.pairs()
        .iter()
        .filter_map(|pair| {
            let age = pair.age_at(ctx.stars(), t)?;
            let bits = pair.stars().iter().fold(0, |b, s| b | 1 << s.get());
            Some((bits, PairHistory::of(pair.timeline(), age)))
        })
        .collect()
}

/// The planets of `zone` of `system` in body order, as `PlanetarySystem::fates` resolves them.
fn zone_planets<'s>(system: &'s PlanetarySystem, zone: &OrbitZone) -> Vec<&'s FateBody> {
    system
        .bodies
        .iter()
        .filter(|body| body.host == zone.host() && matches!(body.part, Part::Planet(_)))
        .filter_map(Body::fate)
        .collect()
}

/// How many of `bodies`, planets of `zone`, were placed with their pericentre on its P-type limit,
/// to [`ZONE_EDGE_TOLERANCE`] of it.
fn on_the_limit(zone: &OrbitZone, bodies: &[&FateBody]) -> usize {
    let Some(inner) = zone.inner() else {
        return 0;
    };
    bodies
        .iter()
        .filter(|body| (body.orbit().periapsis() / inner - 1.0).abs() <= ZONE_EDGE_TOLERANCE)
        .count()
}

/// The census of `records`, systems of `galaxy`, under the rules in force on this thread, added to
/// `census`.
fn census_of(galaxy: &Galaxy, records: &[SystemRecord], census: &mut Census) {
    let t = UniverseTime::EPOCH;
    for record in records {
        if record.kind() == SystemKind::RoguePlanet {
            continue;
        }
        let stars = SystemStars::generate(galaxy, record);
        if stars.pairs().is_empty() || record.existence_at(t) != Existence::Exists {
            continue;
        }
        let Some(state) = stars.state_at(t) else {
            continue;
        };
        let engine = engine_states(&stars, &state, t);
        census.released += usize::from(engine.iter().any(Option::is_none));
        let Ok(ctx) = SystemContext::for_stars(galaxy, &stars) else {
            continue;
        };
        let pairs = pair_histories(&ctx, t);
        let system = generate_planets(galaxy.seed(), &ctx);
        let epoch = Epoch::of(&ctx, t);
        let hosts: Vec<FateHost<'_>> = system
            .zones
            .iter()
            .map(|zone| fate_host(&ctx, zone))
            .collect();
        let fates = system.fates(&hosts);
        for (zone, host) in system.zones.iter().zip(&hosts) {
            let members = members_of(zone);
            let Some(history) = pairs
                .iter()
                .filter(|&&(bits, _)| bits & members != 0)
                .map(|&(_, history)| history)
                .min()
                .filter(|&history| history != PairHistory::Detached)
            else {
                continue;
            };
            let enveloped_pair = matches!(zone.host(), OrbitHost::Star(_))
                && pairs.iter().any(|&(bits, history)| {
                    bits & members != 0
                        && matches!(
                            history,
                            PairHistory::CommonEnvelopeMerger
                                | PairHistory::CommonEnvelopeThenMerger
                                | PairHistory::PostCommonEnvelope
                        )
                });
            let gone = zone
                .members()
                .any(|member| engine[usize::from(member)].is_none());
            let (light, mass) = zone.members().fold((0.0, 0.0), |(l, m), member| {
                engine[usize::from(member)].map_or((l, m), |star| {
                    (l + star.luminosity().value(), m + star.mass().value())
                })
            });
            let zone_bodies = zone_planets(&system, zone);
            census.on_the_limit += on_the_limit(zone, &zone_bodies);
            let row = census.rows.entry(history).or_default();
            let planets = system.bodies.iter().zip(&fates).filter(|(body, _)| {
                body.host == zone.host() && matches!(body.part, Part::Planet(_))
            });
            // Every planet has its fate, so `me` is its place among `zone_bodies` too.
            for (me, (_, fate)) in planets.enumerate() {
                row.systems.insert(ctx.id());
                row.planets += 1;
                let Some(fate) = fate else {
                    continue;
                };
                match fate.at(t).state() {
                    BodyState::Present => {}
                    BodyState::Unbound { .. } => {
                        row.unbound += 1;
                        if let Some(loss) = zone_loss(&ctx, zone, host, (&zone_bodies, me), t) {
                            *census.zone_losses.entry(loss).or_default() += 1;
                        }
                        continue;
                    }
                    BodyState::Destroyed { .. } => {
                        row.destroyed += 1;
                        continue;
                    }
                    BodyState::NotYetFormed => continue,
                }
                row.present += 1;
                if let Some(bound) = zone_bound(&ctx, zone) {
                    row.outside += usize::from(was_outside(&ctx, zone, bound, fate, t));
                }
                if gone {
                    row.released += 1;
                    continue;
                }
                let read = epoch.luminosity(zone).map_or(0.0, SolarLuminosities::value);
                row.light_off += usize::from(off(read, light, LIGHT_TOLERANCE));
                row.light_unequal += usize::from(off(read, light, UNEQUAL));
                row.mass_off += usize::from(off(fate.host_mass(t).value(), mass, MASS_TOLERANCE));
                row.enveloped += usize::from(enveloped_pair);
                row.massless += usize::from(mass <= 0.0);
            }
        }
    }
}

/// The binary hosts' census at the version's rules: every present planet's zone light
/// reads as the engine's (P14.T1.e) and the mass its orbit follows is the engine's, no S-type
/// planet of a pair that went through a common envelope is left, and none orbits a host the
/// engine left with no mass (P14.T28.g), none has left its zone at its pair's evaluated state,
/// and none is of a zone that holds a star a supernova released (P14.T9.d), where under the
/// single models, in the research's sample of 500,000 systems, some 19,000, 19,600, 392, 549 and
/// 1,432 did not (research-binary-hosts §1). No planet is placed on its zone's P-type limit,
/// where below 28 some were (research-leans-37 §1); the planets unbound by the zone check alone
/// are printed by cause, unjudged.
#[test]
#[ignore = "slow: the binary hosts' census, 100,000 systems, about 90 s"]
fn the_binary_hosts_census_reads_every_paired_star_as_the_engine_does() {
    let mut margin = Census::default();
    for seed in GALAXY_SEEDS {
        let galaxy = Galaxy::from_params_at_version(
            Seed::new(seed),
            GalaxyParams::milky_way_like(),
            GeneratorVersion::new(27),
        )
        .expect("the Milky Way fixture's gas is mostly neutral");
        let records = sample_records(&galaxy, SYSTEMS, SampleFilter::ALL).unwrap();
        with_every_28_hold(|| census_of(&galaxy, &records, &mut margin));
    }
    eprintln!("{}", margin.table("every 28 hold"));
    {
        let (version, census) = (28, &margin);
        for (history, row) in &census.rows {
            assert_eq!(
                row.light_unequal, 0,
                "{version}, {history:?}: zones lit off the engine's"
            );
            assert_eq!(
                row.mass_off, 0,
                "{version}, {history:?}: orbits about a mass off the engine's"
            );
            assert_eq!(
                row.enveloped, 0,
                "{version}, {history:?}: S-type planets through an envelope"
            );
            assert_eq!(
                row.massless, 0,
                "{version}, {history:?}: planets of a host with no mass"
            );
            assert_eq!(
                row.outside, 0,
                "{version}, {history:?}: planets that left their zone"
            );
            assert_eq!(
                row.released, 0,
                "{version}, {history:?}: planets of a released star"
            );
        }
    }
    assert_eq!(margin.on_the_limit, 0, "planets on a P-type limit at 28");
}
