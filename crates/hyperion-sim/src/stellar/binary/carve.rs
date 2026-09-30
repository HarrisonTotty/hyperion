//! The grid's half of the carve (plan 11, P11.T7, design notes 8 and 9): a grid system one of
//! whose pairs falls into a carved catalogue class at any age of the source horizon
//! ([`carved_class`](super::carved_class)) is redrawn on its next attempt, in
//! [`SystemStars::generate`](crate::stellar::system::SystemStars::generate).
//!
//! The redraw is rejection sampling of the system's binaries: attempt n draws the hierarchy on
//! words 64n to 64n + 63 of its streams and each companion on its own
//! [`StarDraws::for_attempt`](crate::stellar::draws::StarDraws::for_attempt) n, and the first
//! attempt with no carved pair is kept. So the grid holds exactly the binaries of the model
//! conditioned on not being in a class, and the catalogue (P11.T8) holds those that are; the
//! classes' observed shares leave the field through the share matrix
//! ([`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES)). The primary is never redrawn: its
//! draws, track, death and kick stay plan 06's and plan 08's.

use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::stellar::multiplicity::MultiplicityContext;

/// Whether the system of `record` under `ctx` is redrawn when one of its pairs is carved: a grid
/// record under its own multiplicity. A feature member's binaries are plan 09's classes' and
/// P11.T8.f's, and a forced context's are its caller's.
#[must_use]
pub(crate) fn grid_redraws(record: &SystemRecord, ctx: MultiplicityContext) -> bool {
    matches!(record.origin(), SystemOrigin::Grid(_)) && matches!(ctx, MultiplicityContext::Free)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::CellKey;
    use crate::galaxy::{Galaxy, Population};
    use crate::id::Layer;
    use crate::stellar::multiplicity::RedrawAttempt;
    use crate::stellar::system::SystemStars;
    use crate::units::{SolarMasses, Years};

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0b17_0007), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    }

    /// A young-disc grid record at the Sun-like point: candidate `index` of one cell, with a
    /// primary of `mass` M☉ formed `age` years ago.
    fn record(galaxy: &Galaxy, index: u32, mass: f64, age: f64) -> SystemRecord {
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

    /// Candidate 21 of the test cell, a 12 M☉ primary 30 Myr old, whose first attempt holds an
    /// X-ray binary (found by searching 300 candidates of 12 M☉ at 30 Myr; 4 of them carve).
    const CARVED: (u32, f64, f64) = (21, 12.0, 3.0e7);

    fn primary_of(galaxy: &Galaxy, record: &SystemRecord) -> crate::stellar::system::StarModel {
        SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle)
            .primary()
            .clone()
    }

    /// A grid system whose first attempt is carved is redrawn: every attempt before the kept one
    /// carves, the kept one does not, it is `at_attempt`'s system at that attempt, the primary is
    /// the same model, and the brief keeps the first attempt's star count.
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
            first.brief_at(now).map(|b| b.star_count())
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
        assert!(grid_redraws(&r, MultiplicityContext::Free));
        for ctx in [
            MultiplicityContext::ForcedSingle,
            MultiplicityContext::ForcedMultiple {
                max_separation: None,
            },
        ] {
            assert!(!grid_redraws(&r, ctx));
            assert_eq!(
                SystemStars::generate_in(&galaxy, &r, ctx).attempt(),
                RedrawAttempt::FIRST
            );
        }
    }
}
