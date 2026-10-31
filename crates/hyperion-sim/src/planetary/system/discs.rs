//! The discs of a system's young and dead hosts as bodies (plan 14, design notes 11 and 12): the
//! protoplanetary disc in belt slot `0xE0` (P14.T28.a) and a white dwarf's debris disc in `0xEE`
//! with its pollution mark (P14.T28.d), each behind its hold at generator version 26
//! (`PROTOPLANETARY_DISC_VERSION`, `WHITE_DWARF_DEBRIS_VERSION`).
//!
//! # The protoplanetary disc
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
//!
//! # A white dwarf's pollution and debris disc
//!
//! A system whose first single-star zone in zone order has a white dwarf by the end of the clock
//! window (lean, 2026-10-31: `0xEE` is one slot, and a circumbinary zone is not read) is marked
//! once ([`WhiteDwarfMarks`], on `belt.population`, [`hosts::white_dwarfs`](crate::planetary::hosts::white_dwarfs)),
//! from what the fate transform leaves its host at the white dwarf's birth: a belt present then and
//! at least one planet. Nothing about that host changes over its white dwarf phase, so the mark
//! holds through it ([`PlanetarySystem::white_dwarf`]). A system with a debris disc lists it as a
//! body: not yet formed before the white dwarf's birth, present from it until its drawn cooling age,
//! and `Destroyed { Dispersed }` after, between its dust's sublimation radius and the white dwarf's
//! Roche limit at the time; its mass is not modelled.

use crate::Seed;
use crate::planetary::SystemContext;
use crate::planetary::disc::DiscProfile;
use crate::planetary::fate::{BodyFate, BodyState, DestructionCause, FateHost};
use crate::planetary::hosts::white_dwarfs::{
    DEBRIS_DISC_SLOT, Survivors, WhiteDwarfDebrisRules, WhiteDwarfDraws, WhiteDwarfMarks,
    debris_disc_inner_edge, debris_disc_outer_edge,
};
use crate::planetary::hosts::young::{FormationKind, ProtoplanetaryDiscRules};
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::label::BodyLabel;
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::{
    BodyIdentity, BodyKind, BodyRecord, DebrisDiscRecord, DiscGap, DiscRecord, Population, Section,
};
use crate::stellar::remnant::RemnantKind;
use crate::stellar::system::StarModel;
use crate::time::{ClockWindow, UniverseTime};
use crate::units::{EarthMasses, Metres, Years};

use super::{Epoch, Part, PlanetarySystem, earliest, fate_host, first_death, time_at_age};

/// The belt slot of the protoplanetary disc: `Belt(0)`, slot `0xE0` (design note 3), which no
/// belt takes ([`FIRST_BELT_SLOT`](crate::planetary::belts::FIRST_BELT_SLOT) is 1).
pub const PROTOPLANETARY_DISC_SLOT: u8 = 0;

/// A system's protoplanetary disc body (P14.T28.a): the disc of its host, as a body of the
/// system's records in belt slot `0xE0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

/// A system's white dwarf host as P14.T28.d marks it: the zone's host, and whether the system is
/// polluted and has a debris disc, decided once at the white dwarf's birth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhiteDwarfSystem {
    host: OrbitHost,
    star: u8,
    born: UniverseTime,
    marks: WhiteDwarfMarks,
}

impl WhiteDwarfSystem {
    /// The white dwarf's zone's host.
    #[must_use]
    pub const fn host(&self) -> OrbitHost {
        self.host
    }

    /// The white dwarf's birth, the clock time of its star's death.
    #[must_use]
    pub const fn born(&self) -> UniverseTime {
        self.born
    }

    /// Its marks: whether the system is polluted, and how long its debris disc lasts.
    #[must_use]
    pub const fn marks(&self) -> &WhiteDwarfMarks {
        &self.marks
    }

    /// The debris disc's body index, belt slot `0xEE`, if the system has one.
    ///
    /// # Panics
    ///
    /// Never: a belt slot's population is a body index.
    #[must_use]
    pub fn debris_disc(&self) -> Option<BodyIndex> {
        self.marks.disc_until().map(|_| {
            BodyIndex::new(BodySlot::Belt(DEBRIS_DISC_SLOT), BodySub::Primary)
                .expect("a belt slot's population is a body index")
        })
    }

    /// The state of its debris disc at `t`, the system's generated from `ctx` (the
    /// [module](self) documentation).
    #[must_use]
    fn debris_state(&self, ctx: &SystemContext, t: UniverseTime) -> BodyState {
        let model = &ctx.stars()[usize::from(self.star)];
        let death = model
            .death()
            .expect("a marked white dwarf has a death")
            .age()
            .value();
        let until = self.marks.disc_until().map_or(0.0, Years::value);
        let end = time_at_age(Years::new(death + until), model.age_at_epoch());
        if t < self.born {
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
}

/// The disc bodies a system has, found once as it is generated under the rules then in force
/// ([`SystemDiscs::find`]), so that a system keeps them whichever test reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SystemDiscs {
    /// Its protoplanetary disc, from [`PROTOPLANETARY_DISC_VERSION`](crate::planetary::hosts::young::PROTOPLANETARY_DISC_VERSION).
    protoplanetary: Option<ProtoplanetaryDisc>,
    /// Its white dwarf's marks, from [`WHITE_DWARF_DEBRIS_VERSION`](crate::planetary::hosts::white_dwarfs::WHITE_DWARF_DEBRIS_VERSION).
    white_dwarf: Option<WhiteDwarfSystem>,
}

impl SystemDiscs {
    /// No disc body: a system [`generate_planets`](super::generate_planets) makes, and every
    /// system below the holds.
    pub(crate) const NONE: Self = Self {
        protoplanetary: None,
        white_dwarf: None,
    };

    /// The disc bodies of `system`, generated from `ctx` in the universe of `seed`, under the rules
    /// in force: from
    /// [`PROTOPLANETARY_DISC_VERSION`](crate::planetary::hosts::young::PROTOPLANETARY_DISC_VERSION)
    /// the disc of its first host in zone order that has one, and from
    /// [`WHITE_DWARF_DEBRIS_VERSION`](crate::planetary::hosts::white_dwarfs::WHITE_DWARF_DEBRIS_VERSION)
    /// its white dwarf's marks (the [module](self) documentation).
    #[must_use]
    pub(crate) fn find(seed: Seed, ctx: &SystemContext, system: &PlanetarySystem) -> Self {
        let protoplanetary = match ProtoplanetaryDiscRules::in_force() {
            ProtoplanetaryDiscRules::Held => None,
            ProtoplanetaryDiscRules::Disc => system
                .hosts
                .iter()
                .find(|host| host.disc.profile().is_some())
                .map(|host| ProtoplanetaryDisc { host: host.host }),
        };
        let white_dwarf = match WhiteDwarfDebrisRules::in_force() {
            WhiteDwarfDebrisRules::Held => None,
            WhiteDwarfDebrisRules::Debris => system.white_dwarf_of(seed, ctx),
        };
        Self {
            protoplanetary,
            white_dwarf,
        }
    }

    /// The disc bodies' indices, in index order.
    pub(crate) fn indices(&self) -> impl Iterator<Item = BodyIndex> {
        self.protoplanetary
            .iter()
            .map(ProtoplanetaryDisc::index)
            .chain(self.white_dwarf.and_then(|wd| wd.debris_disc()))
    }
}

impl PlanetarySystem {
    /// The system's protoplanetary disc body, if it has one (P14.T28.a): none below generator
    /// version 26, and none for a system without a disc.
    #[must_use]
    pub fn protoplanetary_disc(&self) -> Option<&ProtoplanetaryDisc> {
        self.discs.protoplanetary.as_ref()
    }

    /// The system's white dwarf host as P14.T28.d marks it, if it has one: none below generator
    /// version 26, and none for a system whose first single-star zone has no white dwarf by the
    /// end of the clock window.
    #[must_use]
    pub fn white_dwarf(&self) -> Option<&WhiteDwarfSystem> {
        self.discs.white_dwarf.as_ref()
    }

    /// Whether the system is polluted at `t` (P14.T28.d): its white dwarf is marked polluted and
    /// was born by `t`.
    ///
    /// # Panics
    ///
    /// If `ctx` is not the context the system was generated from.
    #[must_use]
    pub fn is_polluted_at(&self, ctx: &SystemContext, t: UniverseTime) -> bool {
        assert_eq!(
            ctx.id(),
            self.system,
            "a system is queried with the context it was generated from"
        );
        self.discs
            .white_dwarf
            .is_some_and(|wd| wd.marks.is_polluted() && t >= wd.born)
    }

    /// The first single-star zone's white dwarf, marked from its draws and what its host keeps at
    /// its birth, or `None` if it has none by the end of the clock window.
    #[must_use]
    fn white_dwarf_of(&self, seed: Seed, ctx: &SystemContext) -> Option<WhiteDwarfSystem> {
        let (zone, star) = self.zones.iter().find_map(|zone| {
            let mut members = zone.members();
            let star = members.next()?;
            let model = &ctx.stars()[usize::from(star)];
            let white_dwarf = members.next().is_none()
                && model
                    .remnant()
                    .is_some_and(|remnant| remnant.kind() == RemnantKind::WhiteDwarf)
                && model.remnant_age_at(ClockWindow::END).is_some();
            white_dwarf.then_some((zone, star))
        })?;
        let model = &ctx.stars()[usize::from(star)];
        let birth = model
            .death()
            .and_then(|death| time_at_age(death.age(), model.age_at_epoch()))
            .expect("a white dwarf by the window's end has a death on the clock");
        let host = zone.host();
        let hosts: Vec<FateHost<'_>> = self.zones.iter().map(|zone| fate_host(ctx, zone)).collect();
        let fates = self.fates(&hosts);
        let planet = self.bodies.iter().zip(&fates).any(|(body, fate)| {
            matches!(body.part, Part::Planet(_))
                && body.host == host
                && fate
                    .as_ref()
                    .is_some_and(|fate| fate.at(birth).state() == BodyState::Present)
        });
        let belt = planet
            && self.belts.iter().any(|belt| {
                belt.host() == host
                    && self
                        .body_at(ctx, belt.index(), birth)
                        .is_ok_and(|record| record.identity().state() == BodyState::Present)
            });
        let survivors = if belt {
            Survivors::BeltAndPlanet
        } else {
            Survivors::Fewer
        };
        Some(WhiteDwarfSystem {
            host,
            star,
            born: birth,
            marks: WhiteDwarfMarks::decide(
                &WhiteDwarfDraws::for_system(seed, self.system),
                survivors,
            ),
        })
    }

    /// The record of the white dwarf `wd`'s debris disc at the epoch's time, labelled `label`.
    #[must_use]
    fn debris_record(
        &self,
        epoch: &Epoch<'_>,
        wd: WhiteDwarfSystem,
        label: BodyLabel,
    ) -> BodyRecord {
        let index = wd
            .debris_disc()
            .expect("a debris disc's white dwarf has one");
        let state = wd.debris_state(epoch.ctx, epoch.t);
        let identity = BodyIdentity::new(
            self.system,
            index,
            BodyKind::DebrisDisc,
            Some(wd.host),
            state,
        )
        .with_label(label);
        let population = match (state, epoch.states[usize::from(wd.star)]) {
            // Within the clock's rounding of the birth the track may not have reached the white
            // dwarf yet, and the disc's edges wait for it.
            (BodyState::Present, Some(now)) if now.phase().is_remnant() => {
                let outer = debris_disc_outer_edge(now.mass(), now.radius());
                let inner = debris_disc_inner_edge(now.radius(), now.effective_temperature());
                Section::Ok(Population::DebrisDisc(DebrisDiscRecord::new(
                    wd.host,
                    Metres::new(inner.value().min(outer.value())),
                    outer,
                )))
            }
            _ => Section::NotApplicable,
        };
        BodyRecord::builder(identity)
            .mass(Section::NotModelled)
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
        if let Some(disc) = self
            .discs
            .protoplanetary
            .filter(|disc| disc.index() == index)
        {
            return Some(self.protoplanetary_record(epoch, fates, disc, label));
        }
        self.discs
            .white_dwarf
            .filter(|wd| wd.debris_disc() == Some(index))
            .map(|wd| self.debris_record(epoch, wd, label))
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
