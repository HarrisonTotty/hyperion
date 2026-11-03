//! A neutron star's second-generation planets as bodies of its system (plan 14, design note 11,
//! P14.T28.e), [`generate`](super::generate)'s step 10, from generator version 26.
//!
//! Each single-star zone whose star is a neutron star by the end of the clock window (from
//! generator version 28, for a star of a pair the binary engine ran and changed, by its pair's
//! history: [`SystemContext::remnant_by_end`], P14.T1.h), and whose
//! host has a primordial disc for the derivation's snow line to read, draws its system on
//! `planet.secondgen` ([`place_second_generation`]), in the second-generation slots from `0xC0`,
//! zone by zone. Each body is a planet ([`Body::is_second_generation`]): its own radius, volatile
//! and formation draws, formed [`SECOND_GENERATION_DELAY`] after its host's death on its orbit
//! about the remnant ([`Generation::Second`]), with no satellites (lean, 2026-10-31: a fallback
//! disc's few rocky bodies, as PSR B1257+12's, have none known) and its rotation law from its
//! record at the moon generators' time, as every planet's.
//!
//! In a system with such planets the neutron star's light, which every one of its bodies' records
//! reads (its queries' epoch), is its spin-down luminosity at the time
//! ([`PulsarState::spin_down_luminosity`](crate::stellar::remnant::PulsarState::spin_down_luminosity)),
//! at its surface's temperature and radius, with no X-ray history and no brighter past: the only
//! irradiation P14.T28.e gives them (lean, 2026-10-31: so a survivor of the supernova about the
//! same host sees it too). Nothing else of the system moves.

use crate::Seed;
use crate::id::SystemId;
use crate::planetary::SystemContext;
use crate::planetary::derive::atmosphere::VolatileDraws;
use crate::planetary::derive::rotation::BodyRotation;
use crate::planetary::fate::{FateBody, Generation, ScatterDraws};
use crate::planetary::hosts::evolved::Circularisation;
use crate::planetary::hosts::second_generation::{
    FIRST_SECOND_GENERATION_SLOT, SECOND_GENERATION_DELAY, SECOND_GENERATION_ECCENTRICITY,
    SecondGenerationPlanet, place_second_generation,
};
use crate::planetary::hosts::young::{Formation, FormationDraws};
use crate::planetary::index::BodySlot;
use crate::planetary::placement::{OrbitHost, OrbitZone, PlacedPlanet};
use crate::stellar::remnant::RemnantKind;
use crate::units::{Megayears, Years};

use super::{
    Body, Epoch, Part, PlanetPart, PlanetarySystem, parent_time, primordial_radius, radius_rank,
    sphere_density,
};

impl Body {
    /// The second-generation planet `placed` of the host `zone` of `system`, in the universe of
    /// `seed`, formed at the host age `formed_at`: its radius rank, its volatiles and its
    /// formation's magma ocean on its own streams, as a primordial planet's.
    #[must_use]
    fn second_generation(
        seed: Seed,
        system: SystemId,
        zone: &OrbitZone,
        placed: &SecondGenerationPlanet,
        formed_at: Megayears,
    ) -> Self {
        let id = placed.index().body_id(system);
        let mass = placed.mass();
        let formation =
            Formation::second_generation(mass, formed_at, &FormationDraws::for_body(seed, id))
                .expect("a placed body's mass and its formation age are positive");
        let fate = FateBody::new(
            formation,
            *placed.orbit(),
            mass,
            sphere_density(mass, primordial_radius(mass)),
        )
        .expect("a placed body's mass and its density are positive")
        .with_circularisation(Circularisation::NONE)
        .with_scatter_draws(ScatterDraws::Stream { seed, body: id })
        .second_generation()
        .evaporating();
        Self {
            index: placed.index(),
            host: zone.host(),
            part: Part::Planet(PlanetPart {
                placed: PlacedPlanet::second_generation(
                    placed.index(),
                    mass,
                    *placed.orbit(),
                    SECOND_GENERATION_ECCENTRICITY,
                ),
                radius_rank: radius_rank(seed, id),
                volatiles: VolatileDraws::for_body(seed, id),
                fate,
            }),
            rotation: None,
            laplace: None,
        }
    }

    /// Whether the body is a second-generation planet (P14.T28.e).
    #[must_use]
    pub fn is_second_generation(&self) -> bool {
        match &self.part {
            Part::Planet(planet) => planet.fate.generation() == Generation::Second,
            Part::Moon(_) | Part::Ring(_) | Part::Member(_) => false,
        }
    }
}

impl PlanetarySystem {
    /// [`generate`](super::generate)'s step 10: each neutron star's second-generation planets, with
    /// their rotation laws (the [module](self) documentation).
    pub(super) fn add_second_generation(&mut self, seed: Seed, ctx: &SystemContext) {
        let mut slot = FIRST_SECOND_GENERATION_SLOT;
        let mut added = Vec::new();
        for (zone, host) in self.zones.iter().zip(&self.hosts) {
            let OrbitHost::Star(star) = zone.host() else {
                continue;
            };
            // From 28 a paired star's neutron star is its pair's history's (P14.T1.h).
            let Some(ended) = ctx
                .remnant_by_end(usize::from(star))
                .filter(|ended| ended.remnant.kind() == RemnantKind::NeutronStar)
            else {
                continue;
            };
            if host.disc.profile().is_none() {
                continue;
            }
            let formed_at =
                Megayears::from(Years::new(ended.born.value())) + SECOND_GENERATION_DELAY;
            let placed = place_second_generation(
                seed,
                self.system,
                zone.host_number(),
                ended.remnant.mass(),
                slot,
            );
            for planet in &placed {
                if let BodySlot::SecondGeneration(n) = planet.index().slot() {
                    slot = n + 1;
                }
                added.push(Body::second_generation(
                    seed,
                    self.system,
                    zone,
                    planet,
                    formed_at,
                ));
            }
        }
        if added.is_empty() {
            return;
        }
        self.bodies.extend(added);
        self.bodies.sort_by_key(Body::index);
        let then = Epoch::new(self, ctx, parent_time(ctx)).with_shades();
        let records = self.records_at_parent_time(&then);
        let rotations: Vec<(usize, Option<BodyRotation>)> = self
            .bodies
            .iter()
            .zip(&records)
            .enumerate()
            .filter(|(_, (body, _))| body.is_second_generation())
            .map(|(k, (body, record))| {
                (
                    k,
                    record
                        .as_ref()
                        .and_then(|record| self.rotation_then(ctx, body, record)),
                )
            })
            .collect();
        for (k, rotation) in rotations {
            self.bodies[k].rotation = rotation;
        }
    }

    /// The star whose second-generation planets the system holds, by its index, if any: the
    /// neutron star whose light its epochs read as its spin-down luminosity.
    #[must_use]
    pub(super) fn pulsar_host(&self) -> Option<usize> {
        self.bodies
            .iter()
            .find(|body| body.is_second_generation())
            .and_then(|body| match body.host {
                OrbitHost::Star(star) => Some(usize::from(star)),
                OrbitHost::Pair(_) | OrbitHost::Barycentre | OrbitHost::Body(_) => None,
            })
    }
}

#[cfg(test)]
mod tests;
