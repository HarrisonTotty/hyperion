//! The grid's half of the carve (plan 11, P11.T7, design notes 8 and 9): a grid system one of
//! whose pairs falls into a carved catalogue class at any age of the source horizon
//! ([`carved_class`](super::carved_class)) is redrawn on its next attempt, in
//! [`SystemStars::generate`](crate::stellar::system::SystemStars::generate), and so is one whose
//! pair's pooled Type Ia candidate its explosion mark explodes, at or before +H
//! ([`SystemStars::exploded_pair`](crate::stellar::system::SystemStars::exploded_pair)): plan 09's
//! Type Ia class holds every recent one.
//!
//! The redraw is rejection sampling of the system's binaries: attempt n draws the hierarchy on
//! words 64n to 64n + 63 of its streams and each companion on its own
//! [`StarDraws::for_attempt`](crate::stellar::draws::StarDraws::for_attempt) n, and the first
//! attempt with no carved or exploded pair is kept. So the grid holds exactly the binaries of the
//! model conditioned on not being in a class, and the catalogue (P11.T8) holds those that are; the
//! classes' observed shares leave the field through the share matrix
//! ([`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES)). The primary is never redrawn: its
//! draws, track, death and kick stay plan 06's and plan 08's. From generator version 27 a feature's
//! binary members are redrawn the same way, since their feature's list holds its carved binaries
//! (P11.T8.f).

use crate::galaxy::Galaxy;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::stellar::multiplicity::MultiplicityContext;

/// Whether the system of `record` under `ctx` in `galaxy` is redrawn when one of its pairs is
/// carved: a grid record under its own multiplicity, and from generator version 27 a feature's
/// binary member, whose carved binaries its feature's list holds (P11.T8.f). Every other forced
/// context's binaries are its caller's.
#[must_use]
pub(crate) fn redraws(_galaxy: &Galaxy, record: &SystemRecord, ctx: MultiplicityContext) -> bool {
    match (record.origin(), ctx) {
        (SystemOrigin::Grid(_), MultiplicityContext::Free)
        | (SystemOrigin::FeatureMember { .. }, MultiplicityContext::ForcedMultiple { .. }) => true,
        (
            SystemOrigin::Grid(_),
            MultiplicityContext::ForcedSingle | MultiplicityContext::ForcedMultiple { .. },
        )
        | (
            SystemOrigin::FeatureMember { .. },
            MultiplicityContext::Free | MultiplicityContext::ForcedSingle,
        )
        | (
            SystemOrigin::CentreMember { .. }
            | SystemOrigin::GlobalListMember { .. }
            | SystemOrigin::CatalogueSystem(_),
            MultiplicityContext::Free
            | MultiplicityContext::ForcedSingle
            | MultiplicityContext::ForcedMultiple { .. },
        ) => false,
    }
}

/// The carve's test record, shared with the census's hierarchy bound (P11.T16).
#[cfg(test)]
pub(crate) mod testing {
    use crate::Seed;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
    use crate::galaxy::{Galaxy, Population};
    use crate::id::Layer;
    use crate::units::{SolarMasses, Years};

    /// The carve tests' galaxy, the Milky Way fixture under a seed of their own.
    pub(crate) fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0b17_0007), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    }

    /// A young-disc grid record at the Sun-like point: candidate `index` of one cell, with a
    /// primary of `mass` M☉ formed `age` years ago.
    pub(crate) fn record(galaxy: &Galaxy, index: u32, mass: f64, age: f64) -> SystemRecord {
        let key = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the root cube");
        SystemRecord::from_parts(
            key.candidate_id(index).expect("a candidate of the cell"),
            GalacticPosition::from_light_years([0.0, 26_000.0, 30.0]).expect("inside the cube"),
            SystemOrigin::Grid(galaxy.fields().component_id(0).expect("the young disc")),
            Population::YoungThinDisc,
            SolarMasses::new(mass),
            Years::new(age),
        )
    }

    /// Candidate 4 of the test cell, a 12 M☉ primary 30 Myr old, whose first attempt holds an
    /// X-ray binary (found by searching 300 candidates of 12 M☉ at 30 Myr; 24 of them carve at 27,
    /// against 4 until 27, when it was candidate 21, whose first attempt 27 no longer carves).
    pub(crate) const CARVED: (u32, f64, f64) = (4, 12.0, 3.0e7);
}

#[cfg(test)]
mod tests {
    use super::testing::{CARVED, galaxy, record};
    use super::*;
    use crate::galaxy::Galaxy;
    use crate::stellar::brief::{BriefModel, BriefRoute};
    use crate::stellar::multiplicity::RedrawAttempt;
    use crate::stellar::system::SystemStars;

    fn primary_of(galaxy: &Galaxy, record: &SystemRecord) -> crate::stellar::system::StarModel {
        SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle)
            .primary()
            .clone()
    }

    /// A grid system whose first attempt is carved is redrawn: every attempt before the kept one
    /// carves, the kept one does not, it is `at_attempt`'s system at that attempt, and the primary
    /// is the same model. Its brief counts the kept attempt's stars; its range row's model reads
    /// the kept system on the paired route and keeps the first attempt's star count (P11.T11.a;
    /// plan 11's Risks, "Deviations in P11.T7, as built").
    #[test]
    fn a_carved_first_attempt_is_redrawn() {
        let galaxy = galaxy();
        let (index, mass, age) = CARVED;
        let r = record(&galaxy, index, mass, age);
        let primary = primary_of(&galaxy, &r);
        let at = |attempt| {
            SystemStars::at_attempt(&galaxy, &r, &primary, MultiplicityContext::Free, attempt)
        };
        let first = at(RedrawAttempt::FIRST);
        assert_eq!(
            first.carved_pair().map(|(_, class)| class),
            Some(super::super::CarvedClass::XrayBinary)
        );
        let kept = SystemStars::generate(&galaxy, &r);
        assert!(kept.attempt() > RedrawAttempt::FIRST);
        assert_eq!(kept.carved_pair(), None);
        for n in 0..kept.attempt().get() {
            assert!(
                at(RedrawAttempt::new(n).unwrap()).carved_pair().is_some(),
                "attempt {n}"
            );
        }
        let again = at(kept.attempt());
        assert_eq!(again.hierarchy(), kept.hierarchy());
        assert_eq!(again.stars(), kept.stars());
        assert_eq!(again.pairs(), kept.pairs());
        assert_eq!(kept.primary(), &primary);
        let now = crate::time::UniverseTime::EPOCH;
        assert_eq!(
            kept.brief_at(now).map(|b| b.star_count()),
            Some(kept.star_count())
        );
        let model = BriefModel::new(&galaxy, &r);
        assert_eq!(model.route(), BriefRoute::Paired);
        assert_eq!(model.star_count(), first.star_count());
        assert_eq!(
            model.brief_at(now),
            kept.brief_at(now)
                .map(|b| b.with_star_count(first.star_count()))
        );
        // The same system whatever was generated before, and twice the same.
        assert_eq!(kept, SystemStars::generate(&galaxy, &r));
    }

    /// Only a grid record under its own multiplicity is redrawn: a forced context keeps its first
    /// attempt, carved or not.
    #[test]
    fn a_forced_context_is_not_redrawn() {
        let galaxy = galaxy();
        let (index, mass, age) = CARVED;
        let r = record(&galaxy, index, mass, age);
        assert!(redraws(&galaxy, &r, MultiplicityContext::Free));
        for ctx in [
            MultiplicityContext::ForcedSingle,
            MultiplicityContext::ForcedMultiple {
                max_separation: None,
            },
        ] {
            assert!(!redraws(&galaxy, &r, ctx));
            assert_eq!(
                SystemStars::generate_in(&galaxy, &r, ctx).attempt(),
                RedrawAttempt::FIRST
            );
        }
    }
}
