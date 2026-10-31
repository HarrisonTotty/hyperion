//! A rocky belt's member whose bulk is ice takes P14.T56.g's rock lag above the melting point, as
//! an icy belt's member does, behind its hold at generator version 25 ([`MEMBER_LAG_VERSION`]).
//!
//! P14.T56.g asks the lag's verdict of an icy belt's members alone (`icy_member_given_bulk`), whose
//! record states their ice (decision-p14-t56g §3). A rocky belt's member keeps its composition
//! solve's bulk, which can hold a third of its mass in water, so to version 24 such a member can
//! publish an airless `Ice` surface whose zero-albedo temperature lies above the melting point,
//! where bare ice retreats by kilometres a year ([`SurfaceLag`]).
//! That breaks decision-p14-t56g §4's second assertion, "no `Airless` record has an `Ice` surface
//! at T0 above 273.15 K": P14.T33.c's census found 31 such members in 10⁶ bodies, and none in the
//! audit's sets. From 25 every belt member without a given bulk takes
//! [`SolvedLag::AboveMeltingOverIce`]: lagged exactly where its solved bulk is ice and its T0 lies
//! above the melting point, so a rock bulk and an ice bulk under the melting point keep version
//! 24's bits.
//!
//! [`SurfaceLag`]: crate::planetary::derive::SurfaceLag

use crate::planetary::derive::SolvedLag;
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which a rocky belt's member takes [`SolvedLag::AboveMeltingOverIce`]:
/// 25, the first planetary bump after 24's.
///
/// It is this rule's one hold. Below it [`MemberLagRules::in_force`] is [`MemberLagRules::Held`]:
/// only an icy belt's members are asked the lag, and every figure on the record is version 24's
/// bit for bit. 25's bump deletes this hold, [`MemberLagRules`] with its `Held` arm, the tests'
/// switch and its tripwire (`the_member_lag_is_held_only_until_the_25_bump`).
pub(crate) const MEMBER_LAG_VERSION: GeneratorVersion = GeneratorVersion::new(25);

/// Whether every belt member is asked the lag (its hold, [`MEMBER_LAG_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MemberLagRules {
    /// Version 24's and before: an icy belt's members alone.
    Held,
    /// Version 25's: a rocky belt's members too, over their solved bulk.
    Members,
}

impl MemberLagRules {
    /// The rules this generator version gives: every member from [`MEMBER_LAG_VERSION`], the icy
    /// belts' alone below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_member_lag`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= MEMBER_LAG_VERSION {
            Self::Members
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The member lag's rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<MemberLagRules>> = const { core::cell::Cell::new(None) };
}

/// `run`'s result with the member lag's rules `rules` on this thread, for the tests to reach
/// version 25's ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. 25's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_member_lag<R>(rules: MemberLagRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<MemberLagRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Whether every belt member is asked the lag ([`MemberLagRules::Members`]); never under its
/// hold.
#[must_use]
pub(crate) fn member_lag_in_force() -> bool {
    MemberLagRules::in_force() == MemberLagRules::Members
}

/// The lag a belt member with no given bulk is asked: [`SolvedLag::AboveMeltingOverIce`] where
/// the rule is in force, [`SolvedLag::Bare`] under its hold.
#[must_use]
pub(crate) fn member_solved_lag() -> SolvedLag {
    if member_lag_in_force() {
        SolvedLag::AboveMeltingOverIce
    } else {
        SolvedLag::Bare
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::planetary::SystemContext;
    use crate::planetary::derive::atmosphere::SurfaceMaterial;
    use crate::planetary::record::{BodyRecord, Surface, SystemSnapshot};
    use crate::planetary::system::generate;
    use crate::planetary::system::tests::SEED;
    use crate::planetary::testing::{SampleFilter, sample_records};
    use crate::time::UniverseTime;

    /// The first rocky belt's member P14.T33.c's census found with an airless `Ice` surface above
    /// the melting point at T0, by its system's rank in the sample and its ID: its solved bulk a
    /// third water, its T0 371.3 K (the 10⁶ census's #7511).
    const HOT_ICE_MEMBER: (usize, &str) = (7_511, "41ffecb5ff000004.e106");

    /// The member lag is held until the 24 → 25 bump: the rules in force are version 24's, and a
    /// rocky belt's member is asked no lag.
    #[test]
    fn the_member_lag_is_held_only_until_the_25_bump() {
        assert!(
            GENERATOR_VERSION < MEMBER_LAG_VERSION,
            "the 24 → 25 bump gives a rocky belt's ice-bulk members P14.T56.g's lag: delete \
             `MEMBER_LAG_VERSION`, `MemberLagRules` with its `Held` arm, `member_lag_in_force`, \
             `member_solved_lag`'s held arm, the tests' `with_member_lag`, its switch in \
             `with_every_25_hold` and this test"
        );
        assert_eq!(MemberLagRules::in_force(), MemberLagRules::Held);
        assert_eq!(member_solved_lag(), SolvedLag::Bare);
        assert_eq!(
            with_member_lag(MemberLagRules::Members, member_solved_lag),
            SolvedLag::AboveMeltingOverIce
        );
        assert!(!member_lag_in_force());
    }

    /// [`HOT_ICE_MEMBER`]'s system at the epoch: version 24's `Ice` member takes the rock lag
    /// under the rule, the same system twice gives the same records, and no other record of it
    /// moves.
    #[test]
    fn a_rocky_belt_s_hot_ice_member_takes_the_lag_and_nothing_else_moves() {
        let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
            .expect("the Milky Way's parameters build a galaxy");
        let (rank, id) = HOT_ICE_MEMBER;
        let records = sample_records(&galaxy, rank + 1, SampleFilter::ALL)
            .expect("the solar circle holds the sample");
        let ctx = SystemContext::from_record(&galaxy, &records[rank]);
        let snapshot = |rules| {
            with_member_lag(rules, || {
                generate(SEED, &ctx).snapshot_at(&ctx, UniverseTime::EPOCH)
            })
        };
        let material = |snapshot: &SystemSnapshot| {
            snapshot
                .bodies()
                .iter()
                .find(|record| record.identity().id().to_string() == id)
                .and_then(|record| record.surface().ok().map(Surface::material))
        };
        let (held, members) = (
            snapshot(MemberLagRules::Held),
            snapshot(MemberLagRules::Members),
        );
        assert_eq!(
            snapshot(MemberLagRules::Members),
            members,
            "the same system twice"
        );
        assert_eq!(material(&held), Some(SurfaceMaterial::Ice));
        assert_eq!(material(&members), Some(SurfaceMaterial::Rock));
        let moved: Vec<String> = held
            .bodies()
            .iter()
            .zip(members.bodies())
            .filter(|(at_24, at_25)| at_24 != at_25)
            .map(|(_, record): (&BodyRecord, _)| record.identity().id().to_string())
            .collect();
        assert_eq!(moved, [id]);
    }
}
