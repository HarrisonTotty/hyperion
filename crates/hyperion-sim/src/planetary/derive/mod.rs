//! Derivation: everything about a body that is computed from its mass, orbit and host rather than
//! drawn (plan 14, phase C; the brainstorm's "Everything else is computed, not rolled").
//!
//! Built so far, each a pure function of plain quantities that can be tested on Solar System
//! values without a generator:
//!
//! - [`radius`]: Chen and Kipping's mass–radius relation, through which a body's one drawn
//!   quantile sets its radius, and Zeng et al.'s curves of iron, rock and water (P14.T11.a–b).
//! - [`envelope`]: the radius of a core with a hydrogen and helium envelope, from Lopez and
//!   Fortney's and Fortney, Marley and Barnes's models (P14.T11.b).
//! - [`composition`](mod@composition): the solve that turns a (mass, radius) point into iron, rock, water and
//!   envelope, constrained by the side of the snow line the body formed on (P14.T11.c; design note
//!   8).
//! - [`irradiation`]: the flux a body receives from its hosts and its equilibrium temperature
//!   (P14.T12.a).
//! - [`habitable_zone`](mod@habitable_zone): Kopparapu et al.'s habitable zone of a host or of a
//!   multiple system (P14.T12.b).
//! - [`limits`]: Roche limits, Hill radii, the stability limit of satellites and the heaviest moon
//!   a close-in planet can keep (P14.T15).
//!
//! Giants from 0.414 Jupiter masses (P14.T11.d, which needs plan 13's cooling of giant planets),
//! atmospheres (T13), rotation and tides (T14) and the assembly `derive_body` (T16) follow.
//!
//! These read, from the stages above, plan 06's [`UnitUniform`](crate::stellar::draws::UnitUniform)
//! for the radius quantile and [`math::normal_quantile`](crate::math::normal_quantile) to apply
//! it, and a host's luminosity, effective temperature and radius as plain values from its
//! [`StarState`](crate::stellar::StarState) (ruling 34); `units` gains `EarthRadii`,
//! `JupiterRadii`, `WattsPerSquareMetre` and `EarthFluxes`, and `units::consts` the Earth and
//! Jupiter radii, σ and the solar constant.

pub mod composition;
pub mod envelope;
pub mod habitable_zone;
pub mod irradiation;
pub mod limits;
pub mod radius;

pub use composition::{MassFractions, SnowLineSide, SolvedComposition, composition};
pub use habitable_zone::{HabitableZone, habitable_zone, habitable_zone_of};
pub use irradiation::{BondAlbedo, HostLight, Illumination, equilibrium_temperature, total_flux};
pub use limits::{
    OrbitSense, hill_radius, maximum_surviving_moon_mass, roche_limit_fluid, roche_limit_rigid,
    satellite_stability_limit,
};
pub use radius::{CoreComposition, radius_chen_kipping, radius_zeng};
