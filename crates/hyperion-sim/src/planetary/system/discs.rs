//! The discs of a system's young hosts as bodies (plan 14, design note 12): the protoplanetary
//! disc in belt slot `0xE0` (P14.T28.a), behind its hold at generator version 26
//! (`PROTOPLANETARY_DISC_VERSION`).
//!
//! A system has one protoplanetary disc body, its first host's in zone order with a disc (P14.T3),
//! which is the primary's, or the pair's for a circumbinary zone (lean, 2026-10-31: belt slot
//! `0xE0` is one slot, so a wide binary's second disc has no body). It reads no stream: everything
//! it carries is its host's disc and planets. Its state is a prefix of not yet formed → present →
//! destroyed, as every body's is:
//!
//! - not yet formed before the system's birth;
//! - present from the birth until the disc's lifetime, carrying the disc's edges, snow line, taper
//!   and masses at birth, and a gap at the semi-major axis of each of its host's giants formed by
//!   then (design note 12: giants form below the lifetime, small planets at it);
//! - `Destroyed { Dispersed }` from the lifetime on, or from the first death of one of its host's
//!   stars if that comes first, since a death ends its disc (as P14.T28.a's formations have it).

use crate::planetary::disc::DiscProfile;
use crate::planetary::fate::{BodyFate, BodyState, DestructionCause};
use crate::planetary::hosts::young::{FormationKind, ProtoplanetaryDiscRules};
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::label::BodyLabel;
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::{
    BodyIdentity, BodyKind, BodyRecord, DiscGap, DiscRecord, Population, Section,
};
use crate::stellar::system::StarModel;
use crate::time::UniverseTime;
use crate::units::{EarthMasses, Years};

use super::{Epoch, Part, PlanetarySystem, earliest, fate_host, first_death, time_at_age};

/// The belt slot of the protoplanetary disc: `Belt(0)`, slot `0xE0` (design note 3), which no
/// belt takes ([`FIRST_BELT_SLOT`](crate::planetary::belts::FIRST_BELT_SLOT) is 1).
pub const PROTOPLANETARY_DISC_SLOT: u8 = 0;

/// A system's protoplanetary disc body (P14.T28.a): the disc of its host, as a body of the
/// system's records in belt slot `0xE0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtoplanetaryDisc {
    host: OrbitHost,
}

impl ProtoplanetaryDisc {
    /// The orbit host whose disc it is.
    #[must_use]
    pub const fn host(&self) -> OrbitHost {
        self.host
    }

    /// Its body index: belt slot `0xE0`, sub-index 0.
    ///
    /// # Panics
    ///
    /// Never: a belt slot's population is a body index.
    #[must_use]
    pub fn index(&self) -> BodyIndex {
        BodyIndex::new(BodySlot::Belt(PROTOPLANETARY_DISC_SLOT), BodySub::Primary)
            .expect("a belt slot's population is a body index")
    }
}

/// The disc bodies a system has, found once as it is generated under the rules then in force
/// ([`SystemDiscs::find`]), so that a system keeps them whichever test reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SystemDiscs {
    /// Its protoplanetary disc, from [`PROTOPLANETARY_DISC_VERSION`](crate::planetary::hosts::young::PROTOPLANETARY_DISC_VERSION).
    protoplanetary: Option<ProtoplanetaryDisc>,
}

impl SystemDiscs {
    /// No disc body: a system [`generate_planets`](super::generate_planets) makes, and every
    /// system below the holds.
    pub(crate) const NONE: Self = Self {
        protoplanetary: None,
    };

    /// The disc bodies of `system` under the rules in force: from
    /// [`PROTOPLANETARY_DISC_VERSION`](crate::planetary::hosts::young::PROTOPLANETARY_DISC_VERSION)
    /// the disc of its first host in zone order that has one.
    #[must_use]
    pub(crate) fn find(system: &PlanetarySystem) -> Self {
        let protoplanetary = match ProtoplanetaryDiscRules::in_force() {
            ProtoplanetaryDiscRules::Held => None,
            ProtoplanetaryDiscRules::Disc => system
                .hosts
                .iter()
                .find(|host| host.disc.profile().is_some())
                .map(|host| ProtoplanetaryDisc { host: host.host }),
        };
        Self { protoplanetary }
    }

    /// The disc bodies' indices, in index order.
    pub(crate) fn indices(&self) -> impl Iterator<Item = BodyIndex> {
        self.protoplanetary.iter().map(ProtoplanetaryDisc::index)
    }
}

impl PlanetarySystem {
    /// The system's protoplanetary disc body, if it has one (P14.T28.a): none below generator
    /// version 26, and none for a system without a disc.
    #[must_use]
    pub fn protoplanetary_disc(&self) -> Option<&ProtoplanetaryDisc> {
        self.discs.protoplanetary.as_ref()
    }

    /// Whether body `index` is one of the system's disc bodies.
    #[must_use]
    pub(super) fn is_disc(&self, index: BodyIndex) -> bool {
        self.discs.indices().any(|disc| disc == index)
    }

    /// The record of the disc body `index` at the epoch's time, labelled `label`, given every
    /// body's history in body order (`fates`, as [`fates`](Self::fates) gives them), or `None` if
    /// `index` is not a disc body.
    #[must_use]
    pub(super) fn disc_record(
        &self,
        epoch: &Epoch<'_>,
        fates: &[Option<BodyFate<'_>>],
        index: BodyIndex,
        label: BodyLabel,
    ) -> Option<BodyRecord> {
        self.discs
            .protoplanetary
            .filter(|disc| disc.index() == index)
            .map(|disc| self.protoplanetary_record(epoch, fates, disc, label))
    }

    /// The state of the protoplanetary disc `disc` at `t` (the [module](self) documentation).
    #[must_use]
    pub(super) fn protoplanetary_state(
        &self,
        epoch: &Epoch<'_>,
        disc: ProtoplanetaryDisc,
        t: UniverseTime,
    ) -> BodyState {
        let zone = self
            .zone(disc.host)
            .expect("a disc's host is one of its system's zones");
        let lifetime = self
            .host(disc.host)
            .and_then(|host| host.disc.profile())
            .map(DiscProfile::lifetime)
            .expect("a disc body's host has a disc");
        let host = fate_host(epoch.ctx, zone);
        let stars: Vec<&StarModel> = host.members().to_vec();
        // As the fate transform reads a formation's host age (`clock_time`): the first star's age
        // at the epoch, every star's.
        let age_at_epoch = stars[0].age_at_epoch();
        let born = time_at_age(Years::new(0.0), age_at_epoch);
        let end = earliest(
            time_at_age(Years::from(lifetime), age_at_epoch),
            first_death(&stars),
        );
        if born.is_none_or(|born| t < born) {
            BodyState::NotYetFormed
        } else if let Some(at) = end.filter(|&end| t >= end) {
            BodyState::Destroyed {
                cause: DestructionCause::Dispersed,
                at,
            }
        } else {
            BodyState::Present
        }
    }

    /// The record of the protoplanetary disc `disc` at the epoch's time, labelled `label`, its
    /// gaps read off `fates` (the [module](self) documentation).
    #[must_use]
    fn protoplanetary_record(
        &self,
        epoch: &Epoch<'_>,
        fates: &[Option<BodyFate<'_>>],
        disc: ProtoplanetaryDisc,
        label: BodyLabel,
    ) -> BodyRecord {
        let profile = self
            .host(disc.host)
            .and_then(|host| host.disc.profile())
            .expect("a disc body's host has a disc");
        let state = self.protoplanetary_state(epoch, disc, epoch.t);
        let identity = BodyIdentity::new(
            self.system,
            disc.index(),
            BodyKind::ProtoplanetaryDisc,
            Some(disc.host),
            state,
        )
        .with_label(label);
        let population = match state {
            BodyState::Present => {
                let mut gaps: Vec<DiscGap> = self
                    .bodies
                    .iter()
                    .zip(fates)
                    .filter_map(|(body, fate)| {
                        let Part::Planet(planet) = &body.part else {
                            return None;
                        };
                        if body.host != disc.host
                            || planet.fate.formation().kind() != FormationKind::Giant
                        {
                            return None;
                        }
                        let at = fate.as_ref()?.at(epoch.t);
                        let orbit = at.orbit()?;
                        Some(DiscGap::new(body.index, orbit.semi_major_axis()))
                    })
                    .collect();
                gaps.sort_by(|a, b| {
                    a.radius()
                        .value()
                        .total_cmp(&b.radius().value())
                        .then(a.giant().cmp(&b.giant()))
                });
                Section::Ok(Population::ProtoplanetaryDisc(DiscRecord::new(
                    disc.host, profile, gaps,
                )))
            }
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {
                Section::NotApplicable
            }
        };
        BodyRecord::builder(identity)
            .mass(Section::Ok(
                EarthMasses::from(profile.gas_mass()) + profile.solid_mass(),
            ))
            .orbit(Section::NotApplicable)
            .moons(Section::NotApplicable)
            .rings(Section::NotApplicable)
            .bulk(Section::NotApplicable)
            .surface(Section::NotApplicable)
            .envelope(Section::NotApplicable)
            .hooks(Section::NotApplicable)
            .photometry(Section::NotApplicable)
            .figure(Section::NotApplicable)
            .rotation(Section::NotApplicable)
            .population(population)
            .build()
            .expect("a generated record withholds no section and has a known kind")
    }
}

#[cfg(test)]
mod tests;
