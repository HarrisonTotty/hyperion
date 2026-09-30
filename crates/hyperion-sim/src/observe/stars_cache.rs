//! Where an observed query takes each system's stars from: the caller's cache, since the sim holds
//! none (plan 12, P12.T3), in the manner of plan 03's
//! [`CellCache`](crate::galaxy::placement::CellCache).

use crate::galaxy::Galaxy;
use crate::galaxy::features::members::{FeatureInteriorCache, NoInteriorCache, resolve_member};
use crate::galaxy::placement::{SystemKind, SystemOrigin, SystemRecord};
use crate::id::SystemIdKind;
use crate::stellar::system::SystemStars;

/// Lends a system's [`SystemStars`] to an observed query, generating them if the cache has not got
/// them.
///
/// An observed row reads the system's brief at the emitted time through
/// [`SystemStars::brief_at`], which holds for any time back to the source horizon (P12.T0 as
/// built), and building a system's stars costs a millisecond or two per evolved star, so a caller
/// that asks about the same systems again keeps them. An implementation must lend exactly what
/// [`stars_of`] builds for `record` in `galaxy`, and one that keeps stars between calls keys them
/// by galaxy as well as by system. Nothing the query returns then depends on what the cache held.
/// The query asks for no rogue planet's stars, since it has none, and for no member of the galactic
/// centre, which it refuses first.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
///
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::SystemRecord;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::galaxy::features::members::NoInteriorCache;
/// use hyperion_sim::observe::{StarsCache, stars_of};
/// use hyperion_sim::stellar::system::SystemStars;
///
/// /// Keeps every system it is asked for, for one galaxy.
/// #[derive(Default)]
/// struct KeepStars(BTreeMap<SystemId, SystemStars>);
///
/// impl StarsCache for KeepStars {
///     fn with_stars<R>(
///         &mut self,
///         galaxy: &Galaxy,
///         record: &SystemRecord,
///         f: impl FnOnce(&SystemStars) -> R,
///     ) -> R {
///         f(self
///             .0
///             .entry(record.id())
///             .or_insert_with(|| stars_of(galaxy, &NoInteriorCache, record)))
///     }
/// }
/// # let _ = KeepStars::default();
/// ```
pub trait StarsCache {
    /// Calls `f` with the stars of `record`'s system in `galaxy`.
    fn with_stars<R>(
        &mut self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        f: impl FnOnce(&SystemStars) -> R,
    ) -> R;
}

/// The cache that keeps nothing: every call generates the system's stars ([`stars_of`]) and drops
/// them.
///
/// The right choice for a one-off query and for a test; its answers are a keeping cache's. It keeps
/// no feature interior either, so each feature member it is asked for builds its feature's
/// interior again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoStarsCache;

impl StarsCache for NoStarsCache {
    fn with_stars<R>(
        &mut self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        f: impl FnOnce(&SystemStars) -> R,
    ) -> R {
        f(&stars_of(galaxy, &NoInteriorCache, record))
    }
}

/// The stars of `record`'s system in `galaxy`, whatever placed it: a grid system's from plan 06's
/// [`SystemStars::generate`], a feature member's from its member record at its cluster's
/// composition ([`MemberRecord::stars`](crate::galaxy::features::members::MemberRecord::stars),
/// plan 09), its feature's interior taken from `interiors`.
///
/// # Panics
///
/// - For a rogue planet, which has no stars, and for a member of the galactic centre, whose stars
///   wait for plan 09's centre composition (the server's system cache refuses them too).
/// - If `record` is a feature member that does not resolve in `galaxy`, which only a record of
///   another galaxy can be.
#[must_use]
pub fn stars_of(
    galaxy: &Galaxy,
    interiors: &dyn FeatureInteriorCache,
    record: &SystemRecord,
) -> SystemStars {
    assert!(
        record.kind() != SystemKind::RoguePlanet,
        "a rogue planet has no stars: {:?}",
        record.id()
    );
    match record.origin() {
        SystemOrigin::Grid(_) => SystemStars::generate(galaxy, record),
        SystemOrigin::FeatureMember { .. } => {
            let SystemIdKind::FeatureMember(id) = record.id().kind() else {
                unreachable!("a feature member's record is built with a member ID")
            };
            resolve_member(galaxy, interiors, id)
                .expect("a feature member's record resolves in the galaxy that placed it")
                .stars(galaxy)
        }
        SystemOrigin::CentreMember { .. } => {
            panic!(
                "a galactic-centre member's stars are not generated yet: {:?}",
                record.id()
            )
        }
    }
}
