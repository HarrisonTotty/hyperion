//! Young hosts: when each planet forms (plan 14, design note 12, P14.T28.a).
//!
//! A disc lives for its drawn lifetime, P06.T15.c's law of the star's own rank for a
//! circumstellar disc and of `planet.disc`'s for a circumbinary one (ruling 33). Giants accrete
//! their gas while it lasts, so a giant forms at a host age drawn uniform in log between
//! [`EARLIEST_GIANT_FORMATION`] (0.5 Myr) and the lifetime; small planets finish as the gas goes,
//! at the lifetime itself. Before its formation age a body is
//! [`BodyState::NotYetFormed`](crate::planetary::fate::BodyState::NotYetFormed), which the fate
//! transform decides ([`fate`](crate::planetary::fate)). A terrestrial planet's surface is a magma
//! ocean until a host age drawn uniform in log between 10 and 100 Myr
//! ([`MAGMA_OCEAN_END_EARLIEST`], [`MAGMA_OCEAN_END_LATEST`]), which the surface stage (P14.T13,
//! T24) reads when it lands; nothing reads it yet.
//!
//! The formation is primordial (design note 1): a pure function of the seed, the body's ID, its
//! mass and its disc's lifetime, which the generator draws with the body (P14.T30.a) and the fate
//! transform reads at every time.
//!
//! # Streams
//!
//! [`tags::PLANET_ORIGIN`], opened with the body's [`BodyId`], so every planet has a stream of its
//! own: word 0 is the giant's formation rank and word 1 the magma ocean's, one uniform each, drawn
//! for every planet whatever its kind; words 2–7 are reserved.
//!
//! # In the vertical slice
//!
//! Plan 14's T28.a also puts a protoplanetary disc body in belt slot `0xE0` before the disc's
//! lifetime, `Destroyed { Dispersed }` afterwards. No slice task fills the belt slots, so it is
//! added with phase D's belts (P14.T21), with a bump, as the task's _Slice_ note says.

use std::error::Error;
use std::fmt;

use crate::Seed;
use crate::id::BodyId;
use crate::math;
use crate::planetary::params::{
    EARLIEST_GIANT_FORMATION, MAGMA_OCEAN_END_EARLIEST, MAGMA_OCEAN_END_LATEST, SPACING_GIANT_MASS,
};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::{EarthMasses, Megayears};

/// Words of [`tags::PLANET_ORIGIN`] a planet reads or reserves: its two ranks, then six reserved.
pub const ORIGIN_WORDS: u64 = 8;

/// How a planet forms: with its disc's gas, or as the gas goes (design note 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FormationKind {
    /// A giant, which accretes its envelope from the gas disc and so forms before the disc has
    /// gone: from [`SPACING_GIANT_MASS`], 0.1 Jupiter masses, the mass from which design note 7
    /// counts a planet as a giant.
    Giant,
    /// Any lighter planet, which finishes forming as the gas disc disperses.
    Small,
}

impl FormationKind {
    /// The kind of a planet of `mass`: [`FormationKind::Giant`] from [`SPACING_GIANT_MASS`].
    #[must_use]
    pub fn of(mass: EarthMasses) -> Self {
        if mass >= EarthMasses::from(SPACING_GIANT_MASS) {
            Self::Giant
        } else {
            Self::Small
        }
    }
}

/// The random variates of one planet's formation, as drawn from [`tags::PLANET_ORIGIN`] by
/// [`FormationDraws::for_body`], or given explicitly by a test or a tool.
///
/// The fields are plain ranks with no invariant between them, so they are public, as the disc's
/// [`DiscDraws`](crate::planetary::disc::DiscDraws) are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationDraws {
    /// The rank of a giant's formation age between [`EARLIEST_GIANT_FORMATION`] and its disc's
    /// lifetime, in the logarithm. Word 0.
    pub giant: UnitUniform,
    /// The rank of the host age at which a terrestrial planet's magma ocean ends, between
    /// [`MAGMA_OCEAN_END_EARLIEST`] and [`MAGMA_OCEAN_END_LATEST`], in the logarithm. Word 1.
    pub magma_ocean: UnitUniform,
}

impl FormationDraws {
    /// Every rank at its median.
    pub const MEDIAN: Self = Self {
        giant: UnitUniform::HALF,
        magma_ocean: UnitUniform::HALF,
    };

    /// The draws of the planet `body` in the universe of `seed`: words 0 and 1 of its
    /// [`tags::PLANET_ORIGIN`] stream.
    #[must_use]
    pub fn for_body(seed: Seed, body: BodyId) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_ORIGIN, ObjectKey::from(body));
        let giant = draw_rank(&mut stream);
        let magma_ocean = draw_rank(&mut stream);
        Self { giant, magma_ocean }
    }
}

/// The next word of `stream` as a rank.
#[must_use]
fn draw_rank(stream: &mut Stream) -> UnitUniform {
    UnitUniform::new(stream.uniform_open()).expect("an open uniform lies strictly between 0 and 1")
}

/// A [`Formation`] could not be built from the values given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildFormationError {
    /// The planet's mass was not finite and positive.
    MassNotPositive(EarthMasses),
    /// The disc's lifetime was not finite and positive.
    LifetimeNotPositive(Megayears),
}

impl fmt::Display for BuildFormationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassNotPositive(mass) => {
                write!(
                    f,
                    "planet mass {} M_earth is not finite and positive",
                    mass.value()
                )
            }
            Self::LifetimeNotPositive(lifetime) => {
                write!(
                    f,
                    "disc lifetime {} Myr is not finite and positive",
                    lifetime.value()
                )
            }
        }
    }
}

impl Error for BuildFormationError {}

/// When a planet forms, and when a terrestrial planet's magma ocean ends: both host ages, Myr since
/// the host's onset of collapse (P14.T28.a).
///
/// # Examples
///
/// A giant of a disc that lives 3 Myr forms before the gas has gone, and a small planet of the same
/// disc as it goes:
///
/// ```
/// use hyperion_sim::planetary::hosts::young::{Formation, FormationDraws, FormationKind};
/// use hyperion_sim::units::{EarthMasses, Megayears};
///
/// let disc = Megayears::new(3.0);
/// let jupiter = Formation::from_draws(EarthMasses::new(317.8), disc, &FormationDraws::MEDIAN)?;
/// assert_eq!(jupiter.kind(), FormationKind::Giant);
/// // The median giant forms at the geometric mean of 0.5 and 3 Myr.
/// assert!((jupiter.formed_at().value() - (0.5_f64 * 3.0).sqrt()).abs() < 1e-12);
///
/// let earth = Formation::from_draws(EarthMasses::new(1.0), disc, &FormationDraws::MEDIAN)?;
/// assert_eq!(earth.formed_at(), disc);
/// // Its magma ocean ends at the geometric mean of 10 and 100 Myr.
/// assert!((earth.molten_until().value() - 1000.0_f64.sqrt()).abs() < 1e-9);
/// # Ok::<(), hyperion_sim::planetary::hosts::young::BuildFormationError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Formation {
    kind: FormationKind,
    formed_at: Megayears,
    molten_until: Megayears,
}

impl Formation {
    /// The formation of the planet `body` of `mass` in the universe of `seed`, whose disc lives
    /// `disc_lifetime`: [`Formation::from_draws`] of [`FormationDraws::for_body`].
    ///
    /// # Errors
    ///
    /// As [`Formation::from_draws`].
    pub fn draw(
        seed: Seed,
        body: BodyId,
        mass: EarthMasses,
        disc_lifetime: Megayears,
    ) -> Result<Self, BuildFormationError> {
        Self::from_draws(mass, disc_lifetime, &FormationDraws::for_body(seed, body))
    }

    /// The formation of a planet of `mass` whose disc lives `disc_lifetime`, from the ranks
    /// `draws` (design note 12).
    ///
    /// - A giant ([`FormationKind::of`]) forms at L₀^(1 − u) L^u for its rank u, with L₀ the
    ///   smaller of [`EARLIEST_GIANT_FORMATION`] and the lifetime L: uniform in log between them,
    ///   and never after L. A disc shorter than 0.5 Myr, which P06.T15.c's floor of 0.3 Myr allows,
    ///   has its giants at its lifetime.
    /// - A small planet forms at the lifetime.
    /// - The magma ocean ends at 10^(1 + u) Myr for its rank u, uniform in log over 10–100 Myr,
    ///   and no earlier than the planet's formation.
    ///
    /// # Errors
    ///
    /// [`BuildFormationError::MassNotPositive`] and [`BuildFormationError::LifetimeNotPositive`]
    /// unless the mass and the lifetime are finite and positive.
    pub fn from_draws(
        mass: EarthMasses,
        disc_lifetime: Megayears,
        draws: &FormationDraws,
    ) -> Result<Self, BuildFormationError> {
        if !(mass.value().is_finite() && mass.value() > 0.0) {
            return Err(BuildFormationError::MassNotPositive(mass));
        }
        let lifetime = disc_lifetime.value();
        if !(lifetime.is_finite() && lifetime > 0.0) {
            return Err(BuildFormationError::LifetimeNotPositive(disc_lifetime));
        }
        let kind = FormationKind::of(mass);
        let formed_at = match kind {
            FormationKind::Giant => {
                let earliest = EARLIEST_GIANT_FORMATION.value();
                if lifetime <= earliest {
                    lifetime
                } else {
                    let (low, high) = (math::ln(earliest), math::ln(lifetime));
                    // The exponential may round past either end, and a giant never forms after
                    // its disc.
                    math::exp(low + draws.giant.value() * (high - low)).clamp(earliest, lifetime)
                }
            }
            FormationKind::Small => lifetime,
        };
        let (low, high) = (
            math::log10(MAGMA_OCEAN_END_EARLIEST.value()),
            math::log10(MAGMA_OCEAN_END_LATEST.value()),
        );
        let molten_until =
            math::exp10(low + draws.magma_ocean.value() * (high - low)).max(formed_at);
        Ok(Self {
            kind,
            formed_at: Megayears::new(formed_at),
            molten_until: Megayears::new(molten_until),
        })
    }

    /// How the planet forms.
    #[must_use]
    pub const fn kind(&self) -> FormationKind {
        self.kind
    }

    /// The host age at which the planet forms, Myr: positive, and never after its disc's lifetime.
    #[must_use]
    pub const fn formed_at(&self) -> Megayears {
        self.formed_at
    }

    /// The host age until which a terrestrial planet's surface is a magma ocean, Myr: 10–100 Myr,
    /// and no earlier than [`Formation::formed_at`].
    ///
    /// It is drawn for every planet, and means something only for a terrestrial one, which the
    /// surface stage decides (P14.T13, T24).
    #[must_use]
    pub const fn molten_until(&self) -> Megayears {
        self.molten_until
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{Layer, SystemId};
    use crate::units::JupiterMasses;

    fn body(index: u32, planet: u16) -> BodyId {
        let cell = GenCell::new(CellSize::Ly8, [3, -2, 1]).expect("a cell of the grid");
        let system = SystemId::from_parts(Layer::A, cell, index).expect("a system of the cell");
        BodyId::new(system, planet << 8)
    }

    fn rank(u: f64) -> UnitUniform {
        UnitUniform::new(u).expect("a rank in (0, 1)")
    }

    #[test]
    fn a_giant_forms_from_a_tenth_of_a_jupiter_mass() {
        let threshold = EarthMasses::from(JupiterMasses::new(0.1));
        assert_eq!(FormationKind::of(threshold), FormationKind::Giant);
        assert_eq!(
            FormationKind::of(threshold * (1.0 - 1e-12)),
            FormationKind::Small
        );
        assert_eq!(
            FormationKind::of(EarthMasses::new(317.8)),
            FormationKind::Giant
        );
        assert_eq!(
            FormationKind::of(EarthMasses::new(17.1)),
            FormationKind::Small
        );
    }

    /// Test (a): no giant forms after its disc has gone, and small planets form as it goes.
    #[test]
    fn no_giant_forms_after_its_disc_has_gone() {
        let seed = Seed::new(0x0014_0028_a000_5eed);
        let mut lcg = Lcg::new(28);
        let mut logs = Vec::new();
        for i in 0..10_000_u32 {
            let lifetime = Megayears::new(0.3 + 14.7 * lcg.next_f64());
            let giant = EarthMasses::new(32.0 + 4_000.0 * lcg.next_f64());
            let small = EarthMasses::new(0.01 + 30.0 * lcg.next_f64());
            let draws = FormationDraws::for_body(seed, body(i, 1));
            let g = Formation::from_draws(giant, lifetime, &draws).expect("valid");
            let s = Formation::from_draws(small, lifetime, &draws).expect("valid");
            let earliest = lifetime.value().min(0.5);
            assert_eq!(g.kind(), FormationKind::Giant);
            assert!(
                (earliest..=lifetime.value()).contains(&g.formed_at().value()),
                "a giant of a {lifetime:?} disc formed at {:?}",
                g.formed_at()
            );
            assert_eq!(s.kind(), FormationKind::Small);
            assert_eq!(s.formed_at(), lifetime);
            for f in [g, s] {
                let molten = f.molten_until().value();
                assert!(
                    (10.0..=100.0).contains(&molten)
                        || molten.total_cmp(&f.formed_at().value()).is_eq()
                );
                assert!(molten >= f.formed_at().value());
            }
            if lifetime.value() > 0.5 {
                logs.push(math::ln(g.formed_at().value() / 0.5) / math::ln(lifetime.value() / 0.5));
            }
        }
        // Uniform in log between 0.5 Myr and the lifetime.
        let ks = ks_one_sample(&mut logs, |x| x.clamp(0.0, 1.0));
        assert_p_value("giant formation ages, in log", ks.p_value, ALPHA);
    }

    #[test]
    fn the_ranks_map_to_their_ages() {
        let lifetime = Megayears::new(4.0);
        let giant = EarthMasses::new(300.0);
        let at = |u: f64| {
            let draws = FormationDraws {
                giant: rank(u),
                magma_ocean: rank(u),
            };
            Formation::from_draws(giant, lifetime, &draws).expect("valid")
        };
        assert!((at(1e-12).formed_at().value() - 0.5).abs() < 1e-9);
        assert!((at(1.0 - 1e-12).formed_at().value() - 4.0).abs() < 1e-9);
        assert!((at(0.5).formed_at().value() - 2.0_f64.sqrt()).abs() < 1e-12);
        assert!((at(1e-12).molten_until().value() - 10.0).abs() < 1e-9);
        assert!((at(1.0 - 1e-12).molten_until().value() - 100.0).abs() < 1e-7);
        // A disc shorter than 0.5 Myr has its giants at its lifetime.
        let brief = Megayears::new(0.3);
        let early = Formation::from_draws(giant, brief, &FormationDraws::MEDIAN).expect("valid");
        assert_eq!(early.formed_at(), brief);
    }

    #[test]
    fn the_draws_are_the_bodys_own_and_repeat() {
        let seed = Seed::new(0x0014_0028_a000_0001);
        let a = FormationDraws::for_body(seed, body(7, 1));
        assert_eq!(a, FormationDraws::for_body(seed, body(7, 1)));
        assert_ne!(a, FormationDraws::for_body(seed, body(7, 2)));
        assert_ne!(a, FormationDraws::for_body(seed, body(8, 1)));
        assert_ne!(a.giant, a.magma_ocean);
        let mut stream = Stream::open(seed, tags::PLANET_ORIGIN, ObjectKey::from(body(7, 1)));
        assert_same_bits(a.giant.value(), stream.uniform_open());
        assert_same_bits(a.magma_ocean.value(), stream.uniform_open());
        let formation = Formation::draw(
            seed,
            body(7, 1),
            EarthMasses::new(100.0),
            Megayears::new(2.0),
        )
        .expect("valid");
        assert_eq!(
            formation,
            Formation::from_draws(EarthMasses::new(100.0), Megayears::new(2.0), &a).expect("valid")
        );
    }

    #[test]
    fn a_formation_refuses_what_it_cannot_use() {
        let draws = FormationDraws::MEDIAN;
        assert_eq!(
            Formation::from_draws(EarthMasses::new(0.0), Megayears::new(2.0), &draws),
            Err(BuildFormationError::MassNotPositive(EarthMasses::new(0.0)))
        );
        assert_eq!(
            Formation::from_draws(EarthMasses::new(1.0), Megayears::new(f64::NAN), &draws)
                .map_err(|e| e.to_string()),
            Err("disc lifetime NaN Myr is not finite and positive".to_owned())
        );
        assert_eq!(
            BuildFormationError::MassNotPositive(EarthMasses::new(-1.0)).to_string(),
            "planet mass -1 M_earth is not finite and positive"
        );
    }
}
