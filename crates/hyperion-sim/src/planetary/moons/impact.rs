//! Giant-impact moons and their tidal recession (plan 14, P14.T18).
//!
//! A rocky or icy planet or dwarf planet of 0.001–5 M⊕ ([`IMPACT_PARENT_MASS`]) has a moon from a
//! giant impact with probability 0.15 ([`IMPACT_PROBABILITY`]), whose mass ratio to its parent is
//! log-uniform over 0.002–0.05 for a planet ([`PLANET_MASS_RATIO`]); a dwarf planet's is, with
//! equal odds, an intact impactor of 0.03–0.2 ([`DWARF_INTACT_MASS_RATIO`], Charon's 0.12) or a
//! small disc moon of 0.0005–0.01 ([`DWARF_DISC_MASS_RATIO`]; ruling 83.6). It forms at 2.15 ±
//! 0.27 of its parent's fluid Roche limit, truncated to 1.3–2.4 ([`FORMATION_ROCHE_RADII`], ruling
//! 83.5), and recedes by the tide it raises on its parent:
//!
//! a(τ)^(13⁄2) = a₀^(13⁄2) + (13⁄2) × 3 (k₂ ÷ Q) √(G ÷ `M_p`) `R_p`⁵ m τ,
//!
//! the closed form of ȧ = 3 (k₂ ÷ Q) (m ÷ `M_p`) n `R_p`⁵ ÷ a⁵ (Murray and Dermott 1999, §4.9), which
//! is continuous and increasing in the age τ, with the parent's k₂ and Q (P14.T14.b's rocky 0.3
//! and 100). A moon whose orbit passes the prograde stability limit of its parent's Hill sphere
//! (P14.T15) is lost at that age, [`BodyState::Unbound`].
//!
//! The impact also sets the parent's obliquity to the isotropic branch of P14.T14.a and starts its
//! surface age clock (P14.T24.b); both read whether [`giant_impact_moon`] returned a moon, since
//! neither task is built. The impact is taken to end the parent's accretion at the system's birth,
//! the start of the recession's clock τ, as the plan's (age + t) has it.
//!
//! [`giant_impact_moon`] takes any [`MoonParent`] of the right class and mass, so that P14.T22 can
//! call it for a belt's largest members, which P14.T21.c makes eligible, as for planets.
//!
//! # Frame
//!
//! The moon forms in the equatorial plane of the debris disc, which after the impact is its
//! parent's equator, so its orbit is circular and on the parent's equator, and its inclination to
//! the parent's orbital plane is the parent's new obliquity, which P14.T14.a draws. Plan 14's
//! phase D gives "the rest" of the moons' orbits relative to the parent's orbital plane; this one
//! is relative to the equator until T14 places it (a deviation, recorded in the plan).
//!
//! # Draws
//!
//! On the parent's own [`tags::MOON_IMPACT`]: whether it has the moon, the rank of its mass ratio,
//! its mean anomaly at the epoch, the rank of its formation distance and, for a dwarf planet, its
//! class, words 0–4.

use crate::Seed;
use crate::math;
use crate::orbit::KeplerElements;
use crate::planetary::derive::{OrbitSense, PlanetClass};
use crate::planetary::fate::BodyState;
use crate::planetary::moons::{MoonParent, ParentKind, draw_rank, log_uniform, moon_orbit};
use crate::planetary::params::{ROCKY_LOVE_NUMBER, ROCKY_TIDAL_Q};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{GRAVITATIONAL_CONSTANT, SECONDS_PER_JULIAN_YEAR};
use crate::units::{
    EarthMasses, Kilograms, KilogramsPerCubicMetre, Metres, Radians, Seconds, Years,
};

/// The probability that an eligible parent has a giant-impact moon: 0.15 (P14.T18).
///
/// Elser, Moore, Stadel and Morishima (2011, Icarus 214, 357, abstract; arXiv:1105.4616) find
/// that more than 1 in 12 terrestrial planets has a large impact-generated moon, in a range from 1
/// in 45 to 1 in 4; 0.15 lies between 1 in 12 and 1 in 4. A parameter of the generator version.
pub const IMPACT_PROBABILITY: f64 = 0.15;

/// The masses of a parent that can have a giant-impact moon: 0.001–5 M⊕ (P14.T18).
pub const IMPACT_PARENT_MASS: (EarthMasses, EarthMasses) =
    (EarthMasses::new(0.001), EarthMasses::new(5.0));

/// The range of a planet's giant-impact moon's mass over its own, log-uniform: 0.002–0.05
/// (P14.T18). The Moon's is 0.0123.
pub const PLANET_MASS_RATIO: (f64, f64) = (0.002, 0.05);

/// A dwarf planet's moon of an intact impactor: its mass over its parent's, log-uniform over
/// 0.03–0.2 (ruling 83.6). Charon is 0.12, Vanth 0.16 ± 0.02 (Brown and Butler 2023,
/// arXiv:2307.04848), Ilmarë about 0.09 and Actaea about 0.04.
pub const DWARF_INTACT_MASS_RATIO: (f64, f64) = (0.03, 0.2);

/// A dwarf planet's small moon from the impact's disc: its mass over its parent's, log-uniform
/// over 0.0005–0.01 (ruling 83.6). Dysnomia is about 0.005, Hi'iaka 0.0045 and Namaka 0.0005
/// (Ragozzine and Brown 2009, AJ 137, 4766), Weywot about 0.002.
pub const DWARF_DISC_MASS_RATIO: (f64, f64) = (0.0005, 0.01);

/// The share of dwarf planets' giant-impact moons that are intact impactors, the rest disc moons:
/// one half (ruling 83.6). Provisional, since discovery favours large moons.
pub const DWARF_INTACT_SHARE: f64 = 0.5;

/// Where a giant-impact moon forms, in its parent's fluid Roche limits for the moon's density:
/// normal about 2.15 with σ = 0.27, truncated to 1.3–2.4 (ruling 83.5, replacing P14.T18's
/// "just outside the fluid Roche limit").
///
/// Salmon and Canup (2012, ApJ 760, 83; arXiv:1210.0932): the Moon accretes at 2.15 ± 0.27 Roche
/// radii in their hybrid runs, and at 1.3 in pure N-body runs, the truncation's lower end.
pub const FORMATION_ROCHE_RADII: (f64, f64, f64, f64) = (2.15, 0.27, 1.3, 2.4);

/// The densest a giant-impact moon is: 3,300 kg m⁻³, a silicate mantle's, since such moons form
/// from the mantles of the target and the impactor (Canup 2004, Icarus 168, 433). The Moon's is
/// 3,344. A parent less dense than this gives its own density, as Pluto gives Charon (1,854
/// against Charon's 1,702).
pub const MANTLE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(3_300.0);

/// A giant-impact moon: its mass and density, where it formed, and how it recedes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpactMoon {
    mass: EarthMasses,
    density: KilogramsPerCubicMetre,
    formed: KeplerElements,
    recession: f64,
    limit: Metres,
}

impl ImpactMoon {
    /// The moon's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The moon's mean density.
    #[must_use]
    pub const fn density(&self) -> KilogramsPerCubicMetre {
        self.density
    }

    /// The moon's orbit as it formed, circular on the parent's equator at a₀.
    #[must_use]
    pub const fn formed(&self) -> &KeplerElements {
        &self.formed
    }

    /// Its semi-major axis at the parent's age `age`: a(τ)^(13⁄2) = a₀^(13⁄2) + K τ, where K is
    /// (13⁄2) × 3 (k₂ ÷ Q) √(G ÷ `M_p`) `R_p`⁵ m. At or before an age of zero it is a₀. Past
    /// [`lost_at`](Self::lost_at) it is the axis the moon would have had, which no body has.
    #[must_use]
    pub fn semi_major_axis_at(&self, age: Years) -> Metres {
        let tau = age.value() * SECONDS_PER_JULIAN_YEAR;
        let a0 = self.formed.semi_major_axis().value();
        if tau.is_nan() || tau <= 0.0 {
            return self.formed.semi_major_axis();
        }
        Metres::new(math::powf(
            math::powf(a0, 6.5) + self.recession * tau,
            1.0 / 6.5,
        ))
    }

    /// The age at which the moon's orbit passes its parent's prograde stability limit and it is
    /// lost; zero if it formed beyond the limit.
    #[must_use]
    pub fn lost_at(&self) -> Years {
        let a0 = self.formed.semi_major_axis().value();
        let limit = self.limit.value();
        if a0 >= limit || self.recession <= 0.0 {
            return Years::new(if a0 >= limit { 0.0 } else { f64::INFINITY });
        }
        let seconds = (math::powf(limit, 6.5) - math::powf(a0, 6.5)) / self.recession;
        Years::new(seconds / SECONDS_PER_JULIAN_YEAR)
    }

    /// The moon's orbit about `parent` at the parent's age `age`, circular at
    /// [`semi_major_axis_at`](Self::semi_major_axis_at), with its phase at the epoch as it formed.
    ///
    /// # Panics
    ///
    /// Never: the axis is positive and finite at every age.
    #[must_use]
    pub fn orbit_at(&self, parent: &MoonParent, age: Years) -> KeplerElements {
        KeplerElements::from_semi_major_axis(
            self.semi_major_axis_at(age),
            parent.mu_with(self.mass),
            self.formed.eccentricity(),
            *self.formed.orientation(),
            self.formed.mean_anomaly_at_epoch(),
        )
        .expect("a receding moon's axis is positive and finite")
    }

    /// The moon's state at `t` in a system aged `age` at the epoch: not yet formed before the
    /// system's birth, present until [`lost_at`](Self::lost_at), and unbound from then.
    #[must_use]
    pub fn state_at(&self, age: Years, t: UniverseTime) -> BodyState {
        let now = age.value() + t.since_epoch().as_julian_years_f64();
        if now <= 0.0 {
            return BodyState::NotYetFormed;
        }
        let lost = self.lost_at().value();
        if now < lost {
            return BodyState::Present;
        }
        let at = Span::from_seconds_f64((lost - age.value()) * SECONDS_PER_JULIAN_YEAR)
            .and_then(|span| UniverseTime::EPOCH.checked_add(span))
            .unwrap_or(t);
        BodyState::Unbound { at }
    }
}

/// Whether `parent` can have a giant-impact moon: rocky or icy, with no envelope, and of 0.001–5
/// M⊕ (P14.T18).
#[must_use]
pub fn can_have_impact_moon(parent: &MoonParent) -> bool {
    let solid = match parent.class() {
        PlanetClass::Rocky | PlanetClass::Icy => true,
        PlanetClass::SubNeptune | PlanetClass::IceGiant | PlanetClass::GasGiant => false,
    };
    let (lo, hi) = IMPACT_PARENT_MASS;
    solid && parent.mass() >= lo && parent.mass() <= hi
}

/// The formation distance at `rank`, in fluid Roche limits: [`FORMATION_ROCHE_RADII`]'s normal law
/// truncated to its range, by inversion.
#[must_use]
pub fn formation_roche_radii(rank: UnitUniform) -> f64 {
    let (mean, sigma, lo, hi) = FORMATION_ROCHE_RADII;
    let cdf = |x: f64| 0.5 * math::erfc(-(x - mean) / (sigma * core::f64::consts::SQRT_2));
    let (p_lo, p_hi) = (cdf(lo), cdf(hi));
    let p = p_lo + rank.value() * (p_hi - p_lo);
    (mean + sigma * math::normal_quantile(p)).clamp(lo, hi)
}

/// The coefficient K of the recession of a moon of mass `moon` about a parent of mass `parent`,
/// radius `radius` and tidal response `k2_over_q`, in m^(13⁄2) s⁻¹: (13⁄2) × 3 (k₂ ÷ Q) √(G ÷
/// `M_p`) `R_p`⁵ m, so that a^(13⁄2) grows by K each second (P14.T18).
#[must_use]
pub fn recession_coefficient(
    parent: Kilograms,
    radius: Metres,
    k2_over_q: f64,
    moon: Kilograms,
) -> f64 {
    6.5 * 3.0
        * k2_over_q
        * (GRAVITATIONAL_CONSTANT / parent.value()).sqrt()
        * math::powi(radius.value(), 5)
        * moon.value()
}

/// The semi-major axis after `elapsed` of a moon that started at `a0` and recedes with the
/// coefficient `k` of [`recession_coefficient`] (P14.T18).
///
/// # Examples
///
/// The Moon, formed at 3.5 Earth radii, is near its present distance after 4.5 Gyr for an
/// effective Q of 35:
///
/// ```
/// use hyperion_sim::planetary::moons::impact::{receded_axis, recession_coefficient};
/// use hyperion_sim::units::{Kilograms, Metres, Seconds};
///
/// let k = recession_coefficient(
///     Kilograms::new(5.972e24),
///     Metres::new(6.371e6),
///     0.3 / 35.0,
///     Kilograms::new(7.346e22),
/// );
/// let a = receded_axis(Metres::new(3.5 * 6.371e6), k, Seconds::new(4.5e9 * 3.156e7));
/// assert!((3e8..5e8).contains(&a.value()));
/// ```
#[must_use]
pub fn receded_axis(a0: Metres, k: f64, elapsed: Seconds) -> Metres {
    Metres::new(math::powf(
        math::powf(a0.value(), 6.5) + k * elapsed.value().max(0.0),
        1.0 / 6.5,
    ))
}

/// The giant-impact moon of `parent` in the universe of `seed`, or `None` (P14.T18; see the
/// [module](self) documentation).
///
/// `parent` may be any body of the right class and mass, a planet or a dwarf planet: P14.T22
/// calls this for a belt's largest members as for planets. One that cannot have such a moon
/// ([`can_have_impact_moon`]) draws nothing.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{CellSize, GenCell};
/// use hyperion_sim::id::{Layer, SystemId};
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::derive::PlanetClass;
/// use hyperion_sim::planetary::moons::{MoonParent, MoonParentParts, ParentKind, giant_impact_moon};
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
/// use hyperion_sim::units::{EarthMasses, GravitationalParameter, Kilograms, Metres, Radians, SolarMasses, Years};
///
/// let system = SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, [1, 2, 0])?, 3)?;
/// let orbit = KeplerElements::from_semi_major_axis(
///     Metres::new(METRES_PER_AU),
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///     Eccentricity::new(0.0167)?,
///     Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?,
///     Radians::ZERO,
/// )?;
/// let with_moons = (0..200u8).filter_map(|slot| {
///     let id = BodyIndex::new(BodySlot::Planet(1 + slot % 100), BodySub::Primary).ok()?;
///     let earth = MoonParent::new(MoonParentParts {
///         id: id.body_id(system),
///         kind: ParentKind::Planet,
///         mass: EarthMasses::new(1.0),
///         radius: Metres::new(6.371e6),
///         class: PlanetClass::Rocky,
///         orbit,
///         host_mass: Kilograms::new(SOLAR_MASS_KG),
///         maximum_moon_mass: EarthMasses::new(1.0),
///     }).ok()?;
///     giant_impact_moon(Seed::new(u64::from(slot) / 100), &earth)
/// });
/// for moon in with_moons {
///     // Every moon of an Earth at 1 au survives the Solar System's age.
///     assert!(moon.lost_at().value() > 4.57e9);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn giant_impact_moon(seed: Seed, parent: &MoonParent) -> Option<ImpactMoon> {
    if !can_have_impact_moon(parent) {
        return None;
    }
    let mut stream = Stream::open(seed, tags::MOON_IMPACT, ObjectKey::from(parent.id()));
    if draw_rank(&mut stream).value() >= IMPACT_PROBABILITY {
        return None;
    }
    let ratio_rank = draw_rank(&mut stream);
    let mean_anomaly = Radians::new(core::f64::consts::TAU * draw_rank(&mut stream).value());
    let distance_rank = draw_rank(&mut stream);
    let (lo, hi) = match parent.kind() {
        ParentKind::Planet => PLANET_MASS_RATIO,
        ParentKind::DwarfPlanet if draw_rank(&mut stream).value() < DWARF_INTACT_SHARE => {
            DWARF_INTACT_MASS_RATIO
        }
        ParentKind::DwarfPlanet => DWARF_DISC_MASS_RATIO,
    };
    let mass = parent.mass() * log_uniform(lo, hi, ratio_rank);
    let density = if parent.density() < MANTLE_DENSITY {
        parent.density()
    } else {
        MANTLE_DENSITY
    };
    let a0 = parent.roche_limit_fluid(density) * formation_roche_radii(distance_rank);
    let formed = moon_orbit(
        parent,
        mass,
        a0,
        0.0,
        Radians::ZERO,
        [Radians::ZERO, Radians::ZERO, mean_anomaly],
    );
    Some(ImpactMoon {
        mass,
        density,
        formed,
        recession: recession_coefficient(
            Kilograms::from(parent.mass()),
            parent.radius(),
            ROCKY_LOVE_NUMBER / ROCKY_TIDAL_Q,
            Kilograms::from(mass),
        ),
        limit: parent.stability_limit(0.0, OrbitSense::Prograde),
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

    use super::*;
    use crate::planetary::moons::testing::{earth, parent, planet_id};
    use crate::units::consts::SECONDS_PER_GIGAYEAR;

    /// Up to `n` moons of the parents `make(i)` for i in 0..`tries`, with how many were tried.
    fn moons_of(tries: u32, make: impl Fn(u32) -> MoonParent) -> Vec<(MoonParent, ImpactMoon)> {
        (0..tries)
            .filter_map(|i| {
                let parent = make(i);
                giant_impact_moon(Seed::new(11), &parent).map(|moon| (parent, moon))
            })
            .collect()
    }

    #[test]
    fn earth_moon_values_give_three_to_five_hundred_thousand_km_at_4_5_gyr() {
        let (earth_kg, moon_kg, radius) = (5.972_2e24, 7.346e22, Metres::new(6.371e6));
        let earth_density = KilogramsPerCubicMetre::new(5_513.0);
        let roche =
            crate::planetary::derive::roche_limit_fluid(radius, earth_density, MANTLE_DENSITY);
        // Every formation distance the law allows, 1.3–2.4 Roche radii, 3.8–7.0 Earth radii.
        for rank in [1e-9, 0.25, 0.5, 0.75, 1.0 - 1e-9] {
            let radii = formation_roche_radii(UnitUniform::new(rank).unwrap());
            assert!((1.3..=2.4).contains(&radii), "{radii}");
            let a0 = roche * radii;
            check_recession(earth_kg, moon_kg, radius, a0);
        }
        // The truncation at 2.4, 0.93 σ above the mean, pulls the median down to 2.09.
        let median = formation_roche_radii(UnitUniform::HALF);
        assert!((median - 2.090).abs() < 0.005, "{median}");
    }

    /// The Moon from `a0` is at 3–5 × 10⁸ m after 4.5 Gyr for an effective Q of 30–40 and for the
    /// generator's 100.
    fn check_recession(earth_kg: f64, moon_kg: f64, radius: Metres, a0: Metres) {
        let elapsed = Seconds::new(4.5 * SECONDS_PER_GIGAYEAR);
        for q in [30.0, 35.0, 40.0, ROCKY_TIDAL_Q] {
            let k = recession_coefficient(
                Kilograms::new(earth_kg),
                radius,
                ROCKY_LOVE_NUMBER / q,
                Kilograms::new(moon_kg),
            );
            let a = receded_axis(a0, k, elapsed).value();
            assert!((3e8..5e8).contains(&a), "Q = {q}: {a} m");
        }
    }

    #[test]
    fn the_orbit_is_monotone_and_continuous_in_time() {
        let moons = moons_of(400, |i| earth(planet_id(i, 3)));
        assert!(!moons.is_empty());
        for (parent, moon) in &moons {
            let mut last = moon.formed().semi_major_axis();
            assert_eq!(moon.semi_major_axis_at(Years::new(0.0)), last);
            for step in 1..=1000 {
                let age = Years::new(f64::from(step) * 1e7);
                let a = moon.semi_major_axis_at(age);
                assert!(a > last, "the axis grows with age");
                let near = moon.semi_major_axis_at(Years::new(age.value() + 1.0));
                assert!((near / a - 1.0).abs() < 1e-7, "continuous over a year");
                last = a;
            }
            let orbit = moon.orbit_at(parent, Years::new(4.57e9));
            assert_eq!(
                orbit.semi_major_axis(),
                moon.semi_major_axis_at(Years::new(4.57e9))
            );
            hyperion_testkit::float::assert_same_bits(orbit.eccentricity().value(), 0.0);
        }
    }

    #[test]
    fn a_moon_of_a_planet_at_a_tenth_of_an_au_is_lost_within_a_gigayear() {
        for mass in [0.01, 0.1, 1.0, 5.0] {
            let radius = 6_371.0 * math::powf(mass, 0.279);
            let moons = moons_of(300, |i| {
                parent(
                    planet_id(i, 2),
                    ParentKind::Planet,
                    mass,
                    radius,
                    PlanetClass::Rocky,
                    0.1,
                    0.0,
                )
            });
            assert!(!moons.is_empty());
            for (_, moon) in moons {
                let lost = moon.lost_at().value();
                assert!(lost < 1e9, "{mass} M⊕: lost at {lost} yr");
                assert_eq!(
                    moon.state_at(Years::new(4.0e9), UniverseTime::EPOCH),
                    BodyState::Unbound {
                        at: Span::from_seconds_f64((lost - 4.0e9) * SECONDS_PER_JULIAN_YEAR)
                            .and_then(|s| UniverseTime::EPOCH.checked_add(s))
                            .unwrap()
                    }
                );
            }
        }
    }

    #[test]
    fn the_state_runs_from_not_yet_formed_through_present_to_unbound() {
        let moons = moons_of(400, |i| {
            parent(
                planet_id(i, 2),
                ParentKind::Planet,
                1.0,
                6_371.0,
                PlanetClass::Rocky,
                0.3,
                0.0,
            )
        });
        for (_, moon) in moons {
            let lost = moon.lost_at().value();
            assert!(lost > 0.0 && lost.is_finite());
            let at = |age: f64| moon.state_at(Years::new(age), UniverseTime::EPOCH);
            assert_eq!(at(-1.0), BodyState::NotYetFormed);
            assert_eq!(at(lost * 0.5), BodyState::Present);
            assert!(matches!(at(lost * 2.0), BodyState::Unbound { .. }));
        }
    }

    #[test]
    fn one_eligible_parent_in_about_seven_has_a_moon_in_its_range_of_mass() {
        let tries = 20_000;
        let planets = moons_of(tries, |i| {
            earth(planet_id(i, 1 + u8::try_from(i % 7).unwrap()))
        });
        assert_poisson_count(
            "giant-impact moons",
            u64::try_from(planets.len()).unwrap(),
            IMPACT_PROBABILITY * f64::from(tries),
            ALPHA,
        );
        let ratio = |(parent, moon): &(MoonParent, ImpactMoon)| moon.mass() / parent.mass();
        assert!(
            planets
                .iter()
                .map(ratio)
                .all(|r| (0.002..0.05).contains(&r))
        );
        let dwarfs = moons_of(8_000, |i| {
            parent(
                planet_id(i, 9),
                ParentKind::DwarfPlanet,
                0.0022,
                1_188.0,
                PlanetClass::Icy,
                39.5,
                0.25,
            )
        });
        assert!(!dwarfs.is_empty());
        assert!(
            dwarfs
                .iter()
                .map(ratio)
                .all(|r| (0.0005..0.01).contains(&r) || (0.03..0.2).contains(&r))
        );
        let intact = dwarfs.iter().map(ratio).filter(|&r| r >= 0.03).count();
        let share = f64::from(u32::try_from(intact).unwrap())
            / f64::from(u32::try_from(dwarfs.len()).unwrap());
        assert!((share - DWARF_INTACT_SHARE).abs() < 0.1, "{share}");
        // A dwarf planet's moon takes its parent's lower density, as Charon does.
        for (parent, moon) in &dwarfs {
            assert_eq!(moon.density(), parent.density());
            assert!(moon.formed().semi_major_axis() > parent.roche_limit_fluid(moon.density()));
        }
    }

    #[test]
    fn only_rocky_and_icy_bodies_of_a_thousandth_to_five_earth_masses_have_one() {
        let cases = [
            (0.0009, PlanetClass::Rocky),
            (5.1, PlanetClass::Rocky),
            (2.0, PlanetClass::SubNeptune),
            (3.0, PlanetClass::IceGiant),
            (300.0, PlanetClass::GasGiant),
        ];
        for (mass, class) in cases {
            let moons = moons_of(500, |i| {
                parent(
                    planet_id(i, 4),
                    ParentKind::Planet,
                    mass,
                    6_371.0,
                    class,
                    1.0,
                    0.0,
                )
            });
            assert!(moons.is_empty(), "{mass} M⊕ {class:?}");
        }
        for (mass, class) in [(0.001, PlanetClass::Icy), (5.0, PlanetClass::Rocky)] {
            let p = parent(
                planet_id(0, 4),
                ParentKind::Planet,
                mass,
                6_371.0,
                class,
                1.0,
                0.0,
            );
            assert!(can_have_impact_moon(&p));
        }
    }

    #[test]
    fn two_calls_agree_bit_for_bit() {
        for i in 0..500 {
            let parent = earth(planet_id(i, 3));
            assert_eq!(
                giant_impact_moon(Seed::new(5), &parent),
                giant_impact_moon(Seed::new(5), &parent)
            );
        }
    }
}
