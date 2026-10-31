//! A rocky belt's member whose bulk is ice takes P14.T56.g's rock lag above the melting point, as
//! an icy belt's member does, from generator version 25.
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

/// The lag a belt member with no given bulk is asked: [`SolvedLag::AboveMeltingOverIce`], from
/// generator version 25.
#[must_use]
pub(crate) const fn member_solved_lag() -> SolvedLag {
    SolvedLag::AboveMeltingOverIce
}

#[cfg(test)]
mod tests {
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::planetary::SystemContext;
    use crate::planetary::derive::atmosphere::SurfaceMaterial;
    use crate::planetary::record::{Surface, SystemSnapshot};
    use crate::planetary::system::generate;
    use crate::planetary::system::tests::SEED;
    use crate::planetary::testing::{SampleFilter, sample_records};
    use crate::time::UniverseTime;

    /// The first rocky belt's member P14.T33.c's census found with an airless `Ice` surface above
    /// the melting point at T0, by its system's rank in the sample and its ID: its solved bulk a
    /// third water, its T0 371.3 K (the 10⁶ census's #7511).
    const HOT_ICE_MEMBER: (usize, &str) = (7_511, "41ffecb5ff000004.e106");

    /// [`HOT_ICE_MEMBER`]'s system at the epoch: version 24's `Ice` member takes the rock lag, and
    /// the same system twice gives the same records.
    #[test]
    fn a_rocky_belt_s_hot_ice_member_takes_the_lag() {
        let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
            .expect("the Milky Way's parameters build a galaxy");
        let (rank, id) = HOT_ICE_MEMBER;
        let records = sample_records(&galaxy, rank + 1, SampleFilter::ALL)
            .expect("the solar circle holds the sample");
        let ctx = SystemContext::from_record(&galaxy, &records[rank]);
        let snapshot = || generate(SEED, &ctx).snapshot_at(&ctx, UniverseTime::EPOCH);
        let material = |snapshot: &SystemSnapshot| {
            snapshot
                .bodies()
                .iter()
                .find(|record| record.identity().id().to_string() == id)
                .and_then(|record| record.surface().ok().map(Surface::material))
        };
        let members = snapshot();
        assert_eq!(snapshot(), members, "the same system twice");
        assert_eq!(material(&members), Some(SurfaceMaterial::Rock));
    }
}
