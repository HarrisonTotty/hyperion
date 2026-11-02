//! The binary hosts' census (research-binary-hosts §1): the planets of the sample's systems whose
//! zones hold a star of a pair the binary engine ran, classed by the worst of those pairs'
//! histories before the epoch ([`PairHistory`]), and what each reads of its stars against what
//! the engine says of them (P14.T1.e).
//!
//! The galaxies are version 27's, the systems the [`SYSTEMS`] nearest the Sun-like point. Each planet
//! of such a zone is counted, and each present at the epoch on the fate in force read: the light
//! of its zone's stars as the stage reads it ([`Epoch::luminosity`]) and the mass its fate orbits
//! ([`BodyFate::host_mass`]), each against the sum of the engine's states of the same stars
//! ([`SystemStars::state_at`]); an S-type planet of a pair that went through a common envelope; and
//! a planet whose zone's stars the engine has left with no mass. The census runs under every 27
//! hold, before with the binary hosts' holds set back to their held arms and after as 27 has them,
//! and prints both.
//!
//! The test is native only, as the other censuses are.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use super::*;
use crate::GeneratorVersion;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{Existence, SystemKind, SystemRecord};
use crate::planetary::context::{BinaryHostLightRules, with_binary_host_light};
use crate::planetary::testing::{PairHistory, SampleFilter, sample_records, with_every_27_hold};
use crate::stellar::system::SystemStars;

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
}

/// The census's counts, by history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Census {
    rows: BTreeMap<PairHistory, Row>,
    /// Systems left out because the stellar stage shows fewer of their stars at the epoch than it
    /// drew (P11.T10's released stars), whose states cannot be matched star by star.
    released: usize,
}

impl Census {
    /// Every row's counts summed.
    fn total(&self) -> Row {
        self.rows.values().fold(Row::default(), |mut sum, row| {
            sum.systems.extend(row.systems.iter().copied());
            sum.planets += row.planets;
            sum.present += row.present;
            sum.light_off += row.light_off;
            sum.light_unequal += row.light_unequal;
            sum.mass_off += row.mass_off;
            sum.enveloped += row.enveloped;
            sum.massless += row.massless;
            sum
        })
    }

    /// The census as a table, a row a history and a total.
    fn table(&self, title: &str) -> String {
        let mut out = format!(
            "{title}\n{:<26} {:>7} {:>8} {:>8} {:>9} {:>7} {:>8} {:>9} {:>8}\n",
            "history",
            "systems",
            "planets",
            "present",
            "light off",
            "light ≠",
            "mass off",
            "enveloped",
            "massless"
        );
        let line = |out: &mut String, name: &str, row: &Row| {
            writeln!(
                out,
                "{name:<26} {:>7} {:>8} {:>8} {:>9} {:>7} {:>8} {:>9} {:>8}",
                row.systems.len(),
                row.planets,
                row.present,
                row.light_off,
                row.light_unequal,
                row.mass_off,
                row.enveloped,
                row.massless
            )
            .expect("a string takes every write");
        };
        for (history, row) in &self.rows {
            line(&mut out, &format!("{history:?}"), row);
        }
        line(&mut out, "total", &self.total());
        writeln!(out, "systems left out as released: {}", self.released)
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
        if state.stars().len() != stars.stars().len() {
            census.released += 1;
            continue;
        }
        let Ok(ctx) = SystemContext::for_stars(galaxy, &stars) else {
            continue;
        };
        let engine = state.stars();
        // Each pair the engine ran, with its stars and its history, where its timeline holds `t`.
        let pairs: Vec<(u32, PairHistory)> = ctx
            .pairs()
            .iter()
            .filter_map(|pair| {
                let age = pair.age_at(ctx.stars(), t)?;
                let bits = pair.stars().iter().fold(0, |b, s| b | 1 << s.get());
                Some((bits, PairHistory::of(pair.timeline(), age)))
            })
            .collect();
        let system = generate_planets(galaxy.seed(), &ctx);
        let epoch = Epoch::of(&ctx, t);
        let hosts: Vec<FateHost<'_>> = system
            .zones
            .iter()
            .map(|zone| fate_host(&ctx, zone))
            .collect();
        let fates = system.fates(&hosts);
        for zone in &system.zones {
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
            let (light, mass) = zone.members().fold((0.0, 0.0), |(l, m), member| {
                let star = &engine[usize::from(member)];
                (l + star.luminosity().value(), m + star.mass().value())
            });
            let row = census.rows.entry(history).or_default();
            for (body, fate) in system.bodies.iter().zip(&fates) {
                if body.host != zone.host() || !matches!(body.part, Part::Planet(_)) {
                    continue;
                }
                row.systems.insert(ctx.id());
                row.planets += 1;
                let Some(fate) = fate else {
                    continue;
                };
                if fate.at(t).state() != BodyState::Present {
                    continue;
                }
                row.present += 1;
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

/// The census before and after the binary hosts' holds at 27: every present planet's zone light
/// reads as the engine's after P14.T1.e, where before, in the research's sample, nearly 19,000 of
/// 500,000 systems' did not (research-binary-hosts §1).
#[test]
#[ignore = "slow: the binary hosts' census, 100,000 systems under two rules, about 80 s"]
fn the_binary_hosts_census_reads_every_paired_star_s_light_as_the_engine_s() {
    let (mut before, mut after) = (Census::default(), Census::default());
    for seed in GALAXY_SEEDS {
        let galaxy = Galaxy::from_params_at_version(
            Seed::new(seed),
            GalaxyParams::milky_way_like(),
            GeneratorVersion::new(27),
        )
        .expect("the Milky Way fixture's gas is mostly neutral");
        let records = sample_records(&galaxy, SYSTEMS, SampleFilter::ALL).unwrap();
        with_every_27_hold(|| {
            with_binary_host_light(BinaryHostLightRules::SingleModels, || {
                census_of(&galaxy, &records, &mut before);
            });
        });
        with_every_27_hold(|| census_of(&galaxy, &records, &mut after));
    }
    eprintln!(
        "{}\n{}",
        before.table("before: every 27 hold, the binary hosts' set back"),
        after.table("after: every 27 hold")
    );
    assert!(
        before.total().light_off > 0,
        "the single models light no zone off the engine's"
    );
    for (history, row) in &after.rows {
        assert_eq!(
            row.light_unequal, 0,
            "{history:?}: zones lit off the engine's"
        );
    }
    assert_eq!(before.total().planets, after.total().planets);
}
