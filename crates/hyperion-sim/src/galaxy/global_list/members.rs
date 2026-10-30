//! The global list's members and the record they share (plan 10, Design note 13).
//!
//! A dwarf core's members are placed in its nested grid by plan 09's machinery
//! ([`dwarf_core`](super::dwarf_core), P10.T7.b). Each is a plan 03 [`SystemRecord`] of origin
//! [`SystemOrigin::GlobalListMember`] with what a member carries beyond one
//! ([`GlobalMemberRecord`]): no density component, the halo's population, and the age,
//! composition and velocity of its core.

use crate::coords::GalacticVelocity;
use crate::galaxy::Galaxy;
use crate::galaxy::features::interior::MemberClass;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::stellar::Composition;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::SystemStars;

/// A member of the global list: its record and what it carries beyond one (Design note 13).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalMemberRecord {
    record: SystemRecord,
    class: MemberClass,
    composition: Composition,
    velocity: GalacticVelocity,
}

impl GlobalMemberRecord {
    /// A member from its parts: its record, of origin [`SystemOrigin::GlobalListMember`], its
    /// class, its composition and its velocity at the epoch, galactic.
    ///
    /// # Panics
    ///
    /// In debug builds, if the record's origin is not a global-list member's.
    #[must_use]
    pub(crate) fn new(
        record: SystemRecord,
        class: MemberClass,
        composition: Composition,
        velocity: GalacticVelocity,
    ) -> Self {
        debug_assert!(
            matches!(record.origin(), SystemOrigin::GlobalListMember { .. }),
            "a global-list member's record, not {:?}",
            record.origin()
        );
        Self {
            record,
            class,
            composition,
            velocity,
        }
    }

    /// Its plan 03 record, of origin [`SystemOrigin::GlobalListMember`].
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Its class.
    #[must_use]
    pub const fn class(&self) -> &MemberClass {
        &self.class
    }

    /// Its composition: its own \[Fe/H\], drawn about its core's or its stream's.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Its velocity at the epoch, galactic: its core's or its tube's bulk motion plus its own.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The attempt its primary's draws are read at, which its class's conditional draw chose.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        match self.record.origin() {
            SystemOrigin::GlobalListMember { attempt }
            | SystemOrigin::FeatureMember { attempt, .. }
            | SystemOrigin::CentreMember { attempt } => u32::from(attempt),
            SystemOrigin::Grid(_) => 0,
        }
    }

    /// Its stars: [`SystemStars::generate_with`] at its composition, its multiplicity its own to
    /// draw ([`MultiplicityContext::Free`]): a core's and a stream's classes are single
    /// (P10.T7.a, as built), and plan 11 draws a member's companions as a field star's.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        SystemStars::generate_with(
            galaxy,
            &self.record,
            &self.composition,
            MultiplicityContext::Free,
        )
    }
}
