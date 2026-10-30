//! Where an observed query takes each system's stars from: the caller's cache, since the sim holds
//! none (plan 12, P12.T3), in the manner of plan 03's
//! [`CellCache`](crate::galaxy::placement::CellCache).

use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::system_of_record;
use crate::galaxy::features::members::{FeatureInteriorCache, NoInteriorCache, resolve_member};
use crate::galaxy::placement::{ResolveSystemError, SystemKind, SystemOrigin, SystemRecord};
use crate::id::SystemIdKind;
use crate::stellar::system::SystemStars;

/// Lends a system's [`SystemStars`] to an observed query, generating them if the cache has not got
/// them.
///
/// An observed row reads the system's brief at the emitted time through
/// [`SystemStars::brief_at`], the summary's primary then, from its pair's timeline where the
/// engine ran one, which holds for any time back to the source horizon (P12.T0 as built), and
/// building a system's stars costs a millisecond or two per evolved star, so a caller that asks
/// about the same systems again keeps them. An implementation must lend exactly what
/// [`stars_of`] builds for `record` in `galaxy`, and one that keeps stars between calls keys them
/// by galaxy as well as by system. Nothing the query returns then depends on what the cache held.
/// The query asks for no rogue planet's stars, since it has none, and for no member of the galactic
/// centre nor any catalogue system, whose motion it refuses first
/// ([`TraceMotionError`](crate::observe::TraceMotionError)), so it never meets a class whose stars
/// [`stars_of`] refuses.
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
///             .or_insert_with(|| {
///                 stars_of(galaxy, &NoInteriorCache, record)
///                     .expect("the observed query asks for no catalogue system's stars")
///             }))
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
///
/// # Panics
///
/// As [`stars_of`], and where `stars_of` refuses a catalogue system's stars, which the observed
/// query never asks for ([`StarsCache`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoStarsCache;

impl StarsCache for NoStarsCache {
    fn with_stars<R>(
        &mut self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        f: impl FnOnce(&SystemStars) -> R,
    ) -> R {
        f(&stars_of(galaxy, &NoInteriorCache, record)
            .expect("the observed query asks for no catalogue system's stars (`StarsCache`)"))
    }
}

/// The stars of `record`'s system in `galaxy`, whatever placed it: a grid system's from plan 06's
/// [`SystemStars::generate`], a feature member's from its member record at its cluster's
/// composition ([`MemberRecord::stars`](crate::galaxy::features::members::MemberRecord::stars),
/// plan 09), its feature's interior taken from `interiors`, and a catalogue system's its class's.
///
/// # Errors
///
/// [`ResolveSystemError::KindNotGenerated`] for a catalogue system whose class does not model its
/// members' stars by the stellar stage, a Type Ia's (plan 09, P09.T34.a), as the server's system
/// cache and [`SystemContext::for_system`](crate::planetary::SystemContext::for_system) answer for
/// it; a core collapse's are modelled (P09.T33.a).
///
/// # Panics
///
/// - For a rogue planet, which has no stars, and for a member of the galactic centre, whose stars
///   wait for plan 09's centre composition (the server's system cache refuses them too).
/// - For a member of the global list, whose stars are its member record's
///   ([`GlobalMemberRecord::stars`](crate::galaxy::global_list::members::GlobalMemberRecord::stars)),
///   which the caller's list gives.
/// - If `record` is a feature member that does not resolve in `galaxy`, which only a record of
///   another galaxy can be.
/// - If `record` is a catalogue system that does not resolve in `galaxy`.
pub fn stars_of(
    galaxy: &Galaxy,
    interiors: &dyn FeatureInteriorCache,
    record: &SystemRecord,
) -> Result<SystemStars, ResolveSystemError> {
    assert!(
        record.kind() != SystemKind::RoguePlanet,
        "a rogue planet has no stars: {:?}",
        record.id()
    );
    match record.origin() {
        SystemOrigin::Grid(_) => Ok(SystemStars::generate(galaxy, record)),
        SystemOrigin::FeatureMember { .. } => {
            let SystemIdKind::FeatureMember(id) = record.id().kind() else {
                unreachable!("a feature member's record is built with a member ID")
            };
            Ok(resolve_member(galaxy, interiors, id)
                .expect("a feature member's record resolves in the galaxy that placed it")
                .stars(galaxy))
        }
        SystemOrigin::CentreMember { .. } => {
            panic!(
                "a galactic-centre member's stars are not generated yet: {:?}",
                record.id()
            )
        }
        SystemOrigin::GlobalListMember { .. } => {
            panic!(
                "a global-list member's stars are its member record's, through its list: {:?}",
                record.id()
            )
        }
        // A catalogue system's stars are its class's (plan 09, P09.T33.a), and a Type Ia's are not
        // the stellar stage's (P09.T34.a).
        SystemOrigin::CatalogueSystem(_) => system_of_record(galaxy, record)
            .stars(galaxy)
            .ok_or(ResolveSystemError::KindNotGenerated),
    }
}
