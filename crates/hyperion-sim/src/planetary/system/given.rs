//! A system of one given planet (the planet viewer, rendering plan R15): [`generate`](super::generate)
//! with its placement's planets replaced by one planet given by its primordial mass and orbit.
//!
//! Every zone takes its disc, class and plane as [`generate_planets`](super::generate_planets)
//! gives them; the first star's zone then holds the given planet ([`place_given`]) in place of its
//! class's placement, and no other zone holds a planet. The planet takes its own draws as a placed
//! one does ([`Body::planet`]): its radius rank, formation, volatiles and spin on its own streams,
//! but where a spin or a volatile rank is given ([`GivenSpin`], [`GivenVolatiles`]), which stands in
//! place of its draw and nothing else. [`generate`](super::generate)'s steps 6 to 10 follow
//! unchanged ([`complete`]): belts, satellites, the halo, the rotation laws, the Laplace surfaces,
//! the disc bodies and the second-generation planets, each derived from the planet as it is.

use crate::Seed;
use crate::planetary::SystemContext;
use crate::planetary::architecture::{
    ArchitectureClass, ClassConstraints, ZoneLimit, class_weights, draw_class,
};
use crate::planetary::derive::atmosphere::VolatileDraws;
use crate::planetary::derive::rotation::{BodyRotation, SpinDraws};
use crate::planetary::disc;
use crate::planetary::index::BodyIndex;
use crate::planetary::placement::{
    GivenOrbit, OrbitHost, PlacedPlanet, PlacementHost, ZoneDiscInputs, place_given,
};
use crate::stellar::draws::UnitUniform;
use crate::units::{Radians, Seconds, SolarMasses};

use super::discs::SystemDiscs;
use super::{
    Body, FIRST_PLANET_SLOT, Part, PlanetaryHost, PlanetarySystem, complete, host_circularisations,
    host_plane, members_under, plane_of, stellar_hierarchy, within,
};

/// A planet's primordial spin as given in place of its draws' (plan R15), each `None` for the
/// drawn: its primordial period, which the rotation law still floors at its break-up period, and
/// its obliquity.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct GivenSpin {
    /// The primordial period, positive and finite.
    pub(crate) period: Option<Seconds>,
    /// The obliquity, in `[0, π]`.
    pub(crate) obliquity: Option<Radians>,
}

impl GivenSpin {
    /// Nothing given: every planet the galaxy's generator places.
    pub(crate) const DRAWN: Self = Self {
        period: None,
        obliquity: None,
    };

    /// `drawn`, a body's own draws, with what is given in place of its ranks.
    #[must_use]
    pub(crate) fn over(self, drawn: SpinDraws) -> SpinDraws {
        let drawn = self
            .period
            .map_or(drawn, |period| drawn.with_primordial_period(period));
        self.obliquity
            .map_or(drawn, |obliquity| drawn.with_obliquity(obliquity))
    }
}

/// A planet's volatile ranks as given in place of its draws' (plan R15), each `None` for the
/// drawn: its water, carbon and nitrogen per unit mass ([`VolatileDraws`]' words 0–2). Sulphur,
/// chlorine and the redox rank are always drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct GivenVolatiles {
    /// The water rank.
    pub(crate) water: Option<UnitUniform>,
    /// The carbon rank.
    pub(crate) carbon: Option<UnitUniform>,
    /// The nitrogen rank.
    pub(crate) nitrogen: Option<UnitUniform>,
}

impl GivenVolatiles {
    /// `drawn`, a body's own draws, with what is given in place of its ranks.
    #[must_use]
    fn over(self, drawn: VolatileDraws) -> VolatileDraws {
        VolatileDraws {
            water: self.water.unwrap_or(drawn.water),
            carbon: self.carbon.unwrap_or(drawn.carbon),
            nitrogen: self.nitrogen.unwrap_or(drawn.nitrogen),
            ..drawn
        }
    }
}

/// One planet as the planet viewer gives it (plan R15): its primordial mass and orbit, and its
/// spin and volatiles where they are given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GivenPlanet {
    /// Its mass, orbit and formation distance.
    pub(crate) orbit: GivenOrbit,
    /// Its spin, where given.
    pub(crate) spin: GivenSpin,
    /// Its volatile ranks, where given.
    pub(crate) volatiles: GivenVolatiles,
}

impl Body {
    /// This planet with `spin` and `volatiles` in place of its draws where given; any other body
    /// as it is.
    #[must_use]
    fn with_given(mut self, spin: GivenSpin, volatiles: GivenVolatiles) -> Self {
        if let Part::Planet(planet) = &mut self.part {
            planet.spin = spin;
            planet.volatiles = volatiles.over(planet.volatiles);
        }
        self
    }
}

/// The system of `ctx` in the universe of `seed` with `given` as the one planet of its first
/// star's zone (the [module](self) documentation), whole: `None` where that star has no disc, about
/// which no planet forms.
///
/// The result is a pure function of `seed`, `ctx` and `given`.
///
/// # Panics
///
/// For a rogue planet's context, which has no star; and as [`place_given`] does, for a semi-major
/// axis that is not positive and finite.
#[must_use]
pub(crate) fn generate_given(
    seed: Seed,
    ctx: &SystemContext,
    given: &GivenPlanet,
) -> Option<PlanetarySystem> {
    let id = ctx.id();
    let hierarchy = stellar_hierarchy(ctx);
    let under = members_under(hierarchy);
    let stars = ctx.zone_stars();
    let strip = ctx.strip_radius();
    let zones = ctx.zones();
    let mut hosts = Vec::with_capacity(zones.len());
    let mut bodies = Vec::new();
    for zone in &zones {
        let inputs = ZoneDiscInputs::for_zone(seed, id, zone, &stars, ctx.fe_h())
            .expect("a context gives every component a positive, finite zero-age state");
        let limits = within(zone.truncation(), strip);
        let disc = disc::derive(inputs.host(), inputs.lifetime(), inputs.draws(), limits);
        let reach = limits
            .outer()
            .map_or(ZoneLimit::Unbounded, ZoneLimit::Outer);
        let constraints = ClassConstraints::new(&disc, reach);
        let weights = class_weights(zone.host_mass(), ctx.fe_h());
        let drawn = draw_class(seed, id, zone.host_number(), &weights, &constraints);
        let plane = plane_of(hierarchy, &under, zone);
        // The given planet is the first star's; every other zone places nothing (the class it
        // drew is kept as drawn, and its placed class says it placed nothing, ruling 112.8).
        let class = if zone.host() == OrbitHost::Star(0) {
            let host = PlacementHost::new(zone.host_number(), *inputs.host(), plane);
            let planet = place_given(seed, id, &host, &disc, &given.orbit, FIRST_PLANET_SLOT)?;
            let circularisation =
                host_circularisations(&[planet], zone.host_mass(), ctx.age_at_epoch())
                    .into_iter()
                    .next()
                    .expect("one circularisation a planet");
            bodies.push(
                Body::planet(seed, id, zone, planet, circularisation, inputs.lifetime())
                    .with_given(given.spin, given.volatiles),
            );
            drawn
        } else {
            ArchitectureClass::Barren
        };
        hosts.push(PlanetaryHost {
            host: zone.host(),
            limits,
            disc,
            plane: host_plane(seed, id, zone.host_number(), plane),
            drawn_class: drawn,
            class,
        });
    }
    if bodies.is_empty() {
        return None;
    }
    let system = PlanetarySystem {
        seed,
        system: id,
        zones,
        hosts,
        bodies,
        satellites: Vec::new(),
        belts: Vec::new(),
        halo: None,
        discs: SystemDiscs::NONE,
        rogue: None,
    };
    Some(complete(seed, ctx, system))
}

/// What a planet of a generated system was given or drew, as the planet viewer reads its inputs
/// back (plan R15, `viewer::inputs_of`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlanetParts {
    /// Its zone's host's initial mass: its star's, or a pair's total for a circumbinary planet.
    pub(crate) host_mass: SolarMasses,
    /// The planet as placed: its primordial mass, orbit and formation distance.
    pub(crate) placed: PlacedPlanet,
    /// Its volatile ranks.
    pub(crate) volatiles: VolatileDraws,
    /// Its rotation law, if it has one.
    pub(crate) rotation: Option<BodyRotation>,
}

impl PlanetarySystem {
    /// The parts of the planet `index` ([`PlanetParts`]); `None` for any other body or none.
    #[must_use]
    pub(crate) fn planet_parts(&self, index: BodyIndex) -> Option<PlanetParts> {
        let body = self.body(index)?;
        let Part::Planet(planet) = &body.part else {
            return None;
        };
        Some(PlanetParts {
            host_mass: self.zone_of(body).host_mass(),
            placed: planet.placed,
            volatiles: planet.volatiles,
            rotation: body.rotation,
        })
    }
}
