//! Plan 15's census of the binary engine's Type Ia pool (P15.T9.b): every grid record's first
//! attempt's pairs of two stars run through the engine over a Hubble time, and each pair's pooled
//! candidate (P11.T6's pool), against which `hyperion-fit`'s `type_ia_yield` sets the explosion
//! mark's yield η per delay bin and channel.
//!
//! The records are [`class_census_in`]'s, in its order and parts, and a record the carve-out never
//! redraws (a displaced one, which the generator shows single) has no pairs, as there. Each pair
//! runs from zero age to the census's `until`, so that its candidate's delay is read over the whole
//! delay-time distribution, not only to the record's +H.
//!
//! [`class_census_in`]: super::class_census_in

use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::type_ia::DelayTimeDistribution;
use crate::galaxy::imf::MassBand;
use crate::id::BodyId;
use crate::stellar::Phase;
use crate::stellar::binary::carve::redraws;
use crate::stellar::binary::ia::is_detonatable;
use crate::stellar::binary::{BinaryTimeline, PooledIaEvent, SegmentKind};
use crate::stellar::system::{first_attempt_pairs_to, grid_multiplicity};
use crate::units::Years;

use super::members::for_census_records;
use super::{CensusPart, CensusRecords, CensusSite};

/// One pair's pooled Type Ia candidate in an [`IaPoolCensus`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IaPoolMember {
    record_age: Years,
    pair: BodyId,
    event: PooledIaEvent,
}

impl IaPoolMember {
    /// The pair's record's age at the epoch.
    #[must_use]
    pub const fn record_age(&self) -> Years {
        self.record_age
    }

    /// The pair's key (`SystemHierarchy::pair_key`), which its explosion mark reads
    /// ([`IaExplosionMark::of`](crate::stellar::binary::ia::IaExplosionMark::of)).
    #[must_use]
    pub const fn pair(&self) -> BodyId {
        self.pair
    }

    /// The candidate.
    #[must_use]
    pub const fn event(&self) -> &PooledIaEvent {
        &self.event
    }

    /// Whether it can detonate (ruling 129.1's rule, the explosion mark's own).
    #[must_use]
    pub fn detonatable(&self) -> bool {
        is_detonatable(&self.event)
    }
}

/// What an Ia pool census found ([`ia_pool_census_in`]): its records, the pairs it ran, their
/// candidates, and, for the double white dwarfs' present rate, the delay-time distribution's rate
/// at its records' ages and the pairs whose two white dwarfs come into contact in the Gyr before
/// their record's age.
#[derive(Debug, Clone, PartialEq)]
pub struct IaPoolCensus {
    band: MassBand,
    scanned: u64,
    pairs: u64,
    members: Vec<IaPoolMember>,
    dtd_rate_sum: f64,
    recent_contacts: u64,
}

impl IaPoolCensus {
    /// The band whose layer's records it ran.
    #[must_use]
    pub const fn band(&self) -> MassBand {
        self.band
    }

    /// The records it ran, in every population.
    #[must_use]
    pub const fn scanned(&self) -> u64 {
        self.scanned
    }

    /// The pairs of two stars it ran through the engine, those that can interact by its age.
    #[must_use]
    pub const fn pairs(&self) -> u64 {
        self.pairs
    }

    /// The pairs' pooled candidates, in the order met.
    #[must_use]
    pub fn members(&self) -> &[IaPoolMember] {
        &self.members
    }

    /// The sum over its records of the delay-time distribution's rate at each record's age, per
    /// year per M☉ formed (`DelayTimeDistribution::rate_per_year_per_solar_mass`): ψ at the same
    /// ages as the double white dwarfs' contacts.
    #[must_use]
    pub const fn dtd_rate_sum(&self) -> f64 {
        self.dtd_rate_sum
    }

    /// The pairs whose two white dwarfs come into contact, by stable transfer or a merger, in
    /// the Gyr up to their record's age (Maoz, Hallakoun and Badenes 2018's count, which holds
    /// every such contact, the stable ones and the light ones included).
    #[must_use]
    pub const fn recent_contacts(&self) -> u64 {
        self.recent_contacts
    }
}

/// A census of the Type Ia pool among `records` grid records of `band`'s layer at `site`, of the
/// populations `whose`, from the cells of `part` (plan 15's P15.T9.b): each record's first
/// attempt's pairs of two stars run through the engine to `until`, as the generator draws them
/// (`stellar::system::first_attempt_pairs_to`), and each pair's pooled candidate kept.
///
/// # Panics
///
/// If `band` is substellar or the site's cells leave the root cube.
#[must_use]
pub fn ia_pool_census_in(
    galaxy: &Galaxy,
    band: MassBand,
    site: CensusSite,
    whose: CensusRecords,
    records: u64,
    part: CensusPart,
    until: Years,
) -> IaPoolCensus {
    let mut census = IaPoolCensus {
        band,
        scanned: 0,
        pairs: 0,
        members: Vec::new(),
        dtd_rate_sum: 0.0,
        recent_contacts: 0,
    };
    let scanned = for_census_records(galaxy, band, site, whose, records, part, |record| {
        let age = record.age_at_epoch();
        census.dtd_rate_sum += DelayTimeDistribution::rate_per_year_per_solar_mass(age);
        let ctx = grid_multiplicity(record);
        if !redraws(galaxy, record, ctx) {
            return;
        }
        for (pair, timeline) in first_attempt_pairs_to(galaxy, record, ctx, until) {
            census.pairs += 1;
            if let Some(event) = timeline.pooled_ia() {
                census.members.push(IaPoolMember {
                    record_age: age,
                    pair,
                    event,
                });
            }
            let recent = |at: Years| at <= age && at.value() > age.value() - 1.0e9;
            census.recent_contacts += u64::from(dwarfs_contact_age(&timeline).is_some_and(recent));
        }
    });
    census.scanned = scanned.iter().sum();
    census
}

/// When `timeline`'s two white dwarfs first come into contact: the start of the first stable
/// transfer, contact, common envelope or merger that follows a detached phase ending with both
/// stars white dwarfs on a bound orbit, or `None` if none does.
#[must_use]
fn dwarfs_contact_age(timeline: &BinaryTimeline) -> Option<Years> {
    let dwarf = |phase: Phase| {
        matches!(
            phase,
            Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
        )
    };
    timeline.segments().windows(2).find_map(|pair| {
        let [before, after] = pair else {
            return None;
        };
        if before.kind() != SegmentKind::Detached
            || !matches!(
                after.kind(),
                SegmentKind::StableTransfer { .. }
                    | SegmentKind::Contact
                    | SegmentKind::CommonEnvelope
                    | SegmentKind::Merged
            )
        {
            return None;
        }
        // The state a year (or a part in 10¹² of the age, if larger) before the contact segment
        // starts: still the detached phase, the timeline's states being right-continuous at a
        // segment's start, and far above the clock's resolution there.
        let start = after.start().value();
        let state = timeline.state_at(Years::new(start - (start * 1e-12).max(1.0)));
        (state.orbit().is_some() && state.stars().iter().all(|s| dwarf(s.phase())))
            .then_some(after.start())
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::orbit::{Eccentricity, KeplerElements, Orientation};
    use crate::stellar::Composition;
    use crate::stellar::binary::evolve::{Engine, LiveOrbit};
    use crate::stellar::binary::star::{Member, Path};
    use crate::stellar::binary::timeline::Context;
    use crate::stellar::binary::{BinaryInput, evolve};
    use crate::stellar::draws::StarDraws;
    use crate::units::{GravitationalParameter, Megayears, Radians, Seconds, SolarMasses};

    /// A pair of `m1` and `m2` M☉ on a circular orbit of `period_days`, solar, median draws.
    fn pair(m1: f64, m2: f64, period_days: f64) -> BinaryInput {
        let orbit = KeplerElements::from_period(
            Seconds::new(period_days * 86_400.0),
            GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
            Eccentricity::CIRCULAR,
            Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
                .expect("an orientation"),
            Radians::new(0.0),
        )
        .expect("an orbit");
        BinaryInput::new(
            SolarMasses::new(m1),
            SolarMasses::new(m2),
            Composition::SOLAR,
            orbit,
            [StarDraws::median(), StarDraws::median()],
            Years::new(1.0e10),
        )
        .expect("a pair")
    }

    /// Two carbon–oxygen dwarfs of 0.6 M☉ born at 1 Gyr, 0.03 R☉ apart, merge by gravitational
    /// waves within a Gyr: their contact is that merger's start, after a detached phase with both
    /// stars dwarfs. Two main-sequence stars of 1 and 0.5 M☉ 10⁴ days apart never touch: none.
    #[test]
    fn two_dwarfs_contact_where_they_merge_and_wide_stars_never_do() {
        let input = pair(0.6, 0.6, 0.01);
        let start = 1.0e9;
        let dwarf = || Member::Remnant {
            phase: Phase::CarbonOxygenWhiteDwarf,
            birth: start,
            origin: Megayears::ZERO,
            mass: Path::starting(start, 0.6),
        };
        let k = input.orbit();
        let orbit = LiveOrbit::new(
            start,
            0.03,
            0.0,
            *k.orientation(),
            k.mean_anomaly_at_epoch(),
        );
        let mut engine = Engine::new(
            Arc::new(Context::of(&input)),
            start,
            3.0e9,
            [dwarf(), dwarf()],
            orbit,
            None,
        );
        engine.run();
        let timeline = engine.finish();
        let merged = timeline.merger_age().expect("the dwarfs merge");
        assert!(merged.value() < start + 1.0e9, "{merged:?}");
        assert_eq!(dwarfs_contact_age(&timeline), Some(merged));
        let wide = evolve(&pair(1.0, 0.5, 1.0e4), Years::new(1.0e9));
        assert_eq!(dwarfs_contact_age(&wide), None);
    }
}
