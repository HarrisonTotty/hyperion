//! Second-generation planets of neutron stars (plan 14, design note 11, P14.T28.e), from generator
//! version 26.
//!
//! A supernova unbinds nearly every planet of its star, but the fallback disc it leaves about the
//! neutron star can grow a few rocky bodies again, as PSR B1257+12's three (Wolszczan and Frail
//! 1992). Design note 11: a neutron star draws such a system with probability
//! [`SECOND_GENERATION_PROBABILITY`], one to three terrestrial-mass bodies at 0.2–0.5 au, which
//! appear [`SECOND_GENERATION_DELAY`] after the death, in the second-generation slots `0xC0`
//! upward. The draw is fixed by the ID like any other; time enters only as that threshold.
//!
//! # The template
//!
//! P14.T6–T8's laws, with a fixed small template of this module's (lean, 2026-10-31):
//!
//! - a count uniform over 1–3;
//! - each body's semi-major axis log-uniform over [`SECOND_GENERATION_INNER_AU`] to
//!   [`SECOND_GENERATION_OUTER_AU`] and its mass log-uniform over
//!   [`SECOND_GENERATION_LIGHTEST`] to [`SECOND_GENERATION_HEAVIEST`], PSR B1257+12's span;
//! - inside out, a body closer to the last one kept than P14.T6's floor for circular orbits
//!   ([`spacing_floor`], design note 7) is not placed;
//! - each kept body's orbit as P14.T8.d draws a cold chain's ([`SECOND_GENERATION_ECCENTRICITY`],
//!   [`inclination_width`]'s cold width) in a plane of the disc's own, isotropic, each eccentricity
//!   held so that its orbit stays inside the midpoints to its neighbours, and so never crosses
//!   one.
//!
//! The bodies are derived by P14.T16 about the neutron star, lit by its spin-down luminosity alone
//! (`PlanetarySystem`'s queries; P06.T21's
//! [`PulsarState::spin_down_luminosity`](crate::stellar::remnant::PulsarState::spin_down_luminosity)).
//!
//! # Streams
//!
//! [`tags::PLANET_SECONDGEN`], keyed by the system's ID with the orbit host's number in the draw
//! number (design note 4): host h reads words 16h onwards ([`SECOND_GENERATION_WORDS_PER_HOST`]):
//! word 0 the mark, word 1 the count's rank, words 2–3 the plane (the cosine of its inclination
//! and its node), words 4–6 the three axes' ranks and words 7–9 the three masses'; words 10–15
//! are reserved ([`SecondGenerationDraws`]). Each body's own orbit is P14.T8.d's, on its own
//! `planet.orbit` stream ([`OrbitDraws`]).
//!
//! # Sources
//!
//! Cited from memory: the pod that built this task could not reach the papers. Re-check Niţu et
//! al. (2022) and Konacki and Wolszczan (2003) at source.

use core::f64::consts::TAU;

use crate::id::SystemId;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements};
use crate::planetary::architecture::template::EccentricityLaw;
use crate::planetary::index::{BLOCK_LEN, BodyIndex, BodySlot, BodySub};
use crate::planetary::placement::classes::orbits::{
    OrbitDraws, PlaneDraws, eccentricity, inclination_width, mutual_inclination,
    orientation_in_plane,
};
use crate::planetary::placement::{mutual_hill_radius, spacing_floor};
use crate::rng::{Mark, ObjectKey, Seed, Stream, Threshold, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::consts::{GM_EARTH, GM_SUN, SECONDS_PER_DAY};
use crate::units::{
    AstronomicalUnits, Days, EarthMasses, GravitationalParameter, Megayears, Metres, Radians,
    SolarMasses,
};

/// The probability that a neutron star has a second-generation system: 0.005, a parameter of the
/// generator version (design note 11). Niţu et al. (2022, MNRAS 512, 2446) bound pulsar planets
/// to under about half a per cent of pulsars, so this is that upper bound, and may only come
/// down.
pub const SECOND_GENERATION_PROBABILITY: f64 = 0.005;

/// How long after its host's death a second-generation body appears: 10 Myr (design note 11).
pub const SECOND_GENERATION_DELAY: Megayears = Megayears::new(10.0);

/// The innermost semi-major axis a second-generation body is drawn at: 0.2 au (design note 11;
/// PSR B1257+12's innermost is at 0.19 au, Konacki and Wolszczan 2003, ApJ 591, L147).
pub const SECOND_GENERATION_INNER_AU: AstronomicalUnits = AstronomicalUnits::new(0.2);

/// The outermost semi-major axis a second-generation body is drawn at: 0.5 au (design note 11;
/// PSR B1257+12's outermost is at 0.46 au, Konacki and Wolszczan 2003).
pub const SECOND_GENERATION_OUTER_AU: AstronomicalUnits = AstronomicalUnits::new(0.5);

/// The lightest second-generation body: 0.020 M⊕, PSR B1257+12 A's (Konacki and Wolszczan 2003).
pub const SECOND_GENERATION_LIGHTEST: EarthMasses = EarthMasses::new(0.020);

/// The heaviest second-generation body: 4.3 M⊕, PSR B1257+12 B's (Konacki and Wolszczan 2003).
pub const SECOND_GENERATION_HEAVIEST: EarthMasses = EarthMasses::new(4.3);

/// The most second-generation bodies a host draws: 3 (design note 11, "one to three").
pub const SECOND_GENERATION_MAX_COUNT: u8 = 3;

/// The law of a second-generation body's eccentricity: P14.T8.d's cold chains', Rayleigh with
/// σ = 0.04 (`architecture::template`'s `COLD_CHAIN`); PSR B1257+12 B's and C's are 0.019 and
/// 0.025 (Konacki and Wolszczan 2003).
pub const SECOND_GENERATION_ECCENTRICITY: EccentricityLaw =
    EccentricityLaw::Rayleigh { sigma: 0.04 };

/// Words of [`tags::PLANET_SECONDGEN`] that one orbit host owns: host h reads words 16h onwards.
/// Changing it moves every host after the first, which is a generator-version change.
pub const SECOND_GENERATION_WORDS_PER_HOST: u64 = 16;

/// The first second-generation slot as a [`BodySlot::SecondGeneration`] number: 0, slot `0xC0`.
pub const FIRST_SECOND_GENERATION_SLOT: u8 = 0;

/// The draws of one orbit host's second-generation system (P14.T28.e), from its words of
/// [`tags::PLANET_SECONDGEN`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondGenerationDraws {
    /// Whether the host has a system, against [`SECOND_GENERATION_PROBABILITY`]. Word 0.
    pub mark: Mark,
    /// The count's rank over 1–3. Word 1.
    pub count: UnitUniform,
    /// The disc's plane, isotropic. Words 2 and 3.
    pub plane: PlaneDraws,
    /// Each body's semi-major axis's rank in its log-uniform range, in draw order. Words 4–6.
    pub axes: [UnitUniform; 3],
    /// Each body's mass's rank in its log-uniform range, in draw order. Words 7–9.
    pub masses: [UnitUniform; 3],
}

impl SecondGenerationDraws {
    /// The draws of orbit host number `host` of `system`, in the universe of `seed`.
    ///
    /// # Panics
    ///
    /// Never: an open uniform is a rank.
    #[must_use]
    pub fn for_host(seed: Seed, system: SystemId, host: u8) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_SECONDGEN, ObjectKey::from(system));
        stream.seek(u64::from(host) * SECOND_GENERATION_WORDS_PER_HOST);
        let mark = Mark::from_word(stream.next_u64());
        let mut rank = || {
            UnitUniform::new(stream.uniform_open())
                .expect("an open uniform lies strictly between 0 and 1")
        };
        let count = rank();
        let cos_inclination = 1.0 - 2.0 * rank().value();
        let node = Radians::new(TAU * rank().value());
        let axes = [rank(), rank(), rank()];
        let masses = [rank(), rank(), rank()];
        Self {
            mark,
            count,
            plane: PlaneDraws {
                cos_inclination,
                node,
            },
            axes,
            masses,
        }
    }

    /// Whether the host has a second-generation system.
    #[must_use]
    pub fn has_system(&self) -> bool {
        self.mark
            .is_below(Threshold::from_probability(SECOND_GENERATION_PROBABILITY))
    }

    /// How many bodies it draws, 1–3, before the spacing floor drops any.
    #[must_use]
    pub fn drawn_count(&self) -> u8 {
        let count = self.count.value() * f64::from(SECOND_GENERATION_MAX_COUNT);
        let mut n = 1;
        while n < SECOND_GENERATION_MAX_COUNT && f64::from(n) <= count {
            n += 1;
        }
        n
    }
}

/// One second-generation body as placed (P14.T28.e): its index, mass and orbit about its
/// neutron star, with μ = G (M + m).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondGenerationPlanet {
    index: BodyIndex,
    mass: EarthMasses,
    orbit: KeplerElements,
}

impl SecondGenerationPlanet {
    /// Its body index, in the second-generation block.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// Its mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// Its orbit about its neutron star, as it forms.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }
}

/// The second-generation bodies of orbit host number `host` of `system`, a neutron star of mass
/// `mass`, in the universe of `seed`, from second-generation slot number `first_slot`: none unless
/// its mark says it has a system (the [module](self) documentation), and none beyond the
/// sixteenth slot.
///
/// # Panics
///
/// Never for a positive, finite `mass`: the template's axes and masses are positive.
#[must_use]
pub fn place_second_generation(
    seed: Seed,
    system: SystemId,
    host: u8,
    mass: SolarMasses,
    first_slot: u8,
) -> Vec<SecondGenerationPlanet> {
    let draws = SecondGenerationDraws::for_host(seed, system, host);
    if !draws.has_system() {
        return Vec::new();
    }
    let n = usize::from(draws.drawn_count());
    let (inner, outer) = (
        Metres::from(SECOND_GENERATION_INNER_AU).value(),
        Metres::from(SECOND_GENERATION_OUTER_AU).value(),
    );
    let (light, heavy) = (
        SECOND_GENERATION_LIGHTEST.value(),
        SECOND_GENERATION_HEAVIEST.value(),
    );
    let mut bodies: Vec<(Metres, EarthMasses)> = draws.axes[..n]
        .iter()
        .zip(&draws.masses[..n])
        .map(|(a, m)| {
            (
                Metres::new(inner * math::powf(outer / inner, a.value())),
                EarthMasses::new(light * math::powf(heavy / light, m.value())),
            )
        })
        .collect();
    bodies.sort_by(|x, y| x.0.value().total_cmp(&y.0.value()));
    // Inside out, each body clear of the last one kept by P14.T6's floor for circular orbits.
    let mut kept: Vec<(Metres, EarthMasses)> = Vec::with_capacity(n);
    for (a, m) in bodies {
        let clear = kept.last().is_none_or(|&(a0, m0)| {
            (a - a0) / mutual_hill_radius(m0, m, mass, a0, a) >= spacing_floor(m0, m, 0.0, 0.0)
        });
        if clear {
            kept.push((a, m));
        }
    }
    let plane = draws.plane.plane();
    let free = usize::from(BLOCK_LEN.saturating_sub(first_slot));
    kept.iter()
        .enumerate()
        .take(free)
        .map(|(k, &(a, m))| {
            let number = first_slot + u8::try_from(k).expect("at most three bodies");
            let index = BodyIndex::new(BodySlot::SecondGeneration(number), BodySub::Primary)
                .expect("a second-generation slot's planet is a body index");
            let mu = GravitationalParameter::new(GM_SUN * mass.value() + GM_EARTH * m.value());
            let period =
                Days::new(TAU * (a.value() * (a.value() / mu.value()).sqrt()) / SECONDS_PER_DAY);
            // Each orbit stays inside the midpoints to its neighbours, so that none crosses one.
            let gap_in = k.checked_sub(1).map(|j| a.value() - kept[j].0.value());
            let gap_out = kept.get(k + 1).map(|&(next, _)| next.value() - a.value());
            let limit = [gap_in, gap_out]
                .into_iter()
                .flatten()
                .fold(1.0, |limit: f64, gap| limit.min(gap / (2.0 * a.value())));
            let orbit_draws = OrbitDraws::for_planet(seed, system, index);
            let e = eccentricity(
                SECOND_GENERATION_ECCENTRICITY,
                orbit_draws.eccentricity,
                period,
                limit,
            );
            let width = inclination_width(SECOND_GENERATION_ECCENTRICITY, period, true);
            let inclination = mutual_inclination(width, orbit_draws.inclination);
            let orientation =
                orientation_in_plane(&plane, inclination, orbit_draws.node, orbit_draws.periapsis)
                    .expect("the angles are finite and the inclination an arccosine");
            let orbit = KeplerElements::from_semi_major_axis(
                a,
                mu,
                Eccentricity::new(e).expect("an eccentricity held below 1"),
                orientation,
                orbit_draws.mean_anomaly,
            )
            .expect("a positive, finite axis about a positive mass");
            SecondGenerationPlanet {
                index,
                mass: m,
                orbit,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
