//! The merger classes' prior members (plan 11's test helpers; P11.T8.c–d): the engine's own
//! mergers of first-attempt grid pairs, against which a merger class's sampler is checked and plan
//! 15's P15.T10.b fits it.
//!
//! A merger falls in the source horizon, L + 2H ≈ 2.6 × 10⁵ years, of about one grid system in
//! 10⁶ for the stellar mergers and one in 10¹⁰ for the neutron-star mergers, so the members of
//! [`brute_force_class_members`](super::brute_force_class_members), whose pairs are in a class
//! now, would cost hours to days. The members here are instead every pair the engine merged at any
//! age it ran, each kept where [`carved_class`] at its own merger age, the epoch at which its
//! merger is in the horizon, is the class. Pair by pair the engine's merger is the same whenever
//! it is seen: a pair's draws and orbit do not depend on its age.
//!
//! What the age changes is which pairs are seen. A grid system of age a is in the sample if it
//! merged by then, T ≤ a, which for a component with age density f and survival S(T) = P(a ≥ T)
//! happens at the rate S(T); a merger in the horizon now needs a ∈ [T − H, T + L + H], at the
//! rate f(T) (L + 2H). So each member carries the weight f(T) ÷ S(T), its component's age hazard
//! at its merger age ([`MergerMember::weight`]), and the weighted members are the mergers in the
//! horizon, as the catalogue holds them, to the extent that the record's composition (which follows
//! its age) leaves its pair's merger alone.

use crate::galaxy::Galaxy;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use crate::id::Layer;
use crate::stellar::binary::{BinaryState, BinaryTimeline, CarvedClass, SegmentKind, carved_class};
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::tables::binary::CLASS_SHARES;
use crate::units::Years;

use super::Mix;

/// One of the engine's mergers of a class: the grid record its pair came from, the pair's
/// timeline, its merger age T and its weight as a merger in the horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerMember {
    record: SystemRecord,
    timeline: BinaryTimeline,
    merger_age: Years,
    weight: f64,
}

impl MergerMember {
    /// The grid record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The pair's timeline, from the engine.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// The pair's merger age T, Julian years since the stars' onset of collapse.
    #[must_use]
    pub const fn merger_age(&self) -> Years {
        self.merger_age
    }

    /// Its weight as a merger in the source horizon now: its component's age density at T over
    /// the share of the component's born systems at least T old (module documentation).
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }

    /// The pair before T, as [`carved_class`] reads it: its state halfway through the last
    /// segment that began before T and was not the merged star's.
    ///
    /// # Panics
    ///
    /// Never: a member's timeline merged after its first segment began.
    #[must_use]
    pub fn before(&self) -> BinaryState {
        let t = self.merger_age.value();
        let segment = self
            .timeline
            .segments()
            .iter()
            .rfind(|s| s.start().value() < t && s.kind() != SegmentKind::Merged)
            .expect("a pair that merged was a pair before");
        self.timeline
            .state_at(Years::new(f64::midpoint(segment.start().value(), t)))
    }

    /// The merged star at T, or nothing left (a phase of no remnant).
    #[must_use]
    pub fn product(&self) -> crate::stellar::StarState {
        self.timeline.state_at(self.merger_age).stars()[0]
    }
}

/// `n` of the engine's mergers of `class` near the solar circle (plan 11's test helpers): the
/// first attempt of every two-star pair of grid records, run through the engine with no carve-out,
/// kept where the pair merged and [`carved_class`] at its merger age is `class`, from the layers in
/// the shares [`CLASS_SHARES`] gives the class's hosts, on the cells of stream `stream`.
///
/// Each layer with a share s of the class's hosts contributes `round(n s)` members, from random
/// cells within 4,000 ly along x and 2,000 ly along y of the point 26,000 ly out along +y, up to
/// eight records a cell, as [`brute_force_class_members_in`](super::brute_force_class_members_in)
/// draws them.
///
/// # Panics
///
/// Never: the layers' cells are a few light-years to 128 ly, and the cells it opens lie inside the
/// root cube.
#[must_use]
pub fn brute_force_merger_members_in(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
    stream: u64,
) -> Vec<MergerMember> {
    let row = CLASS_SHARES.row(class);
    let mut members = Vec::with_capacity(n + 8);
    let mut cell = Vec::new();
    for (index, band) in (0_u64..).zip(MassBand::ALL) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a member count, rounded, of a test's few hundred"
        )]
        let wanted = (n as f64 * row.layer_share(band)).round() as usize;
        if wanted == 0 {
            continue;
        }
        let layer = Layer::from(band);
        let mut mix = Mix(0x0b17_08c0 ^ (stream << 8) ^ index);
        let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
        let (along, across) = ((4_000 / size).max(1), (2_000 / size).max(1));
        let start = members.len();
        while members.len() - start < wanted {
            let key = CellKey::new(
                layer,
                [mix.within(along), 26_000 / size + mix.within(across), 0],
            )
            .expect("a cell of the grid");
            generate_cell(galaxy, key, &mut cell);
            for record in cell.iter().take(8) {
                mergers_of(galaxy, record, class, &mut members);
            }
        }
        members.truncate(start + wanted);
    }
    members
}

/// The mergers of `class` among the first attempt's pairs of grid record `record`.
fn mergers_of(
    galaxy: &Galaxy,
    record: &SystemRecord,
    class: CarvedClass,
    out: &mut Vec<MergerMember>,
) {
    let SystemOrigin::Grid(component) = record.origin() else {
        return;
    };
    let composition = draw_metallicity(galaxy, record);
    let primary = StarModel::new_under(
        record.primary_initial_mass(),
        composition,
        primary_draws(galaxy, record),
        record.age_at_epoch(),
        galaxy.rules().stellar,
    )
    .expect("a stellar layer's primary is a star with a finite age");
    let stars = SystemStars::at_attempt(
        galaxy,
        record,
        &primary,
        MultiplicityContext::Free,
        RedrawAttempt::FIRST,
    );
    let ages = galaxy.fields().component(component).ages();
    for pair in stars.pairs() {
        let timeline = pair.timeline();
        let Some(t) = timeline.merger_age() else {
            continue;
        };
        if carved_class(timeline, t) != Some(class) {
            continue;
        }
        let survival = 1.0 - ages.cdf(t);
        if survival <= 0.0 {
            continue;
        }
        out.push(MergerMember {
            record: *record,
            timeline: timeline.clone(),
            merger_age: t,
            weight: ages.pdf(t) / survival,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stellar::system::{PairTimeline, grid_multiplicity};

    /// The fits' census galaxy (`hyperion-fit`'s binary manifests) at generator version 31's
    /// rules, at which the merger step is held (P15.T10.b).
    fn census_galaxy() -> Galaxy {
        Galaxy::from_params_for_binary_refit(
            crate::rng::Seed::new(0x0b17_15a0_0000_0001),
            crate::galaxy::params::GalaxyParams::milky_way_like(),
            crate::GeneratorVersion::new(31),
        )
        .expect("the Milky Way fixture builds")
    }

    /// The grid record of ID `raw` in `galaxy`.
    fn grid_record(galaxy: &Galaxy, raw: u64) -> SystemRecord {
        let id = crate::id::SystemId::from_raw(raw).expect("a grid ID");
        let mut cell = Vec::new();
        generate_cell(galaxy, CellKey::of(id).expect("a grid cell"), &mut cell);
        *cell
            .iter()
            .find(|record| record.id() == id)
            .expect("the cell holds its record")
    }

    /// From 31 a merger census's mergers of a record are those of its pairs at the generator's
    /// first attempt, as the class census's members are (P15.T10.b; train 27's finding): under the
    /// record's own multiplicity, so that a displaced record, which the generator shows single,
    /// has none, and on its primary's kept attempt of plan 08's kick loop. On the fits' census
    /// galaxy the census took the displaced remnant `0x81d8b2e000000008` and the retained
    /// `0x81d8b2e000000002` for stellar mergers their primaries' own draws make; the retained
    /// `0x81d8b2e000000001` is a stellar merger and `0x81dbb2e00000000d` a neutron-star merger at
    /// the generator's first attempt too.
    #[test]
    fn from_31_a_record_s_mergers_are_its_pairs_at_the_generator_s_first_attempt() {
        let galaxy = census_galaxy();
        let mut generated_mergers = 0;
        for (raw, class) in [
            (0x81d8_b2e0_0000_0008, CarvedClass::StellarMerger),
            (0x81d8_b2e0_0000_0002, CarvedClass::StellarMerger),
            (0x81d8_b2e0_0000_0001, CarvedClass::StellarMerger),
            (0x81db_b2e0_0000_000d, CarvedClass::NeutronStarMerger),
        ] {
            let record = grid_record(&galaxy, raw);
            let mut found = Vec::new();
            mergers_of(&galaxy, &record, class, &mut found);
            let stars = SystemStars::generate_at(
                &galaxy,
                &record,
                &draw_metallicity(&galaxy, &record),
                grid_multiplicity(&record),
                RedrawAttempt::FIRST,
            );
            let generated: Vec<&BinaryTimeline> = stars
                .pairs()
                .iter()
                .map(PairTimeline::timeline)
                .filter(|timeline| {
                    timeline
                        .merger_age()
                        .is_some_and(|t| carved_class(timeline, t) == Some(class))
                })
                .collect();
            let census: Vec<&BinaryTimeline> = found.iter().map(MergerMember::timeline).collect();
            assert!(
                census == generated,
                "{raw:#x}: the census's {} mergers against the generator's {}",
                census.len(),
                generated.len()
            );
            generated_mergers += generated.len();
        }
        assert_eq!(
            generated_mergers, 2,
            "the last two records keep their mergers"
        );
    }
}
