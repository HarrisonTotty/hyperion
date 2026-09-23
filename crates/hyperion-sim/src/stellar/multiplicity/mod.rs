//! Multiplicity: how many stellar companions a star has, and the periods, mass ratios and
//! eccentricities of their orbits (plan 11, P11.T1).
//!
//! [`MultiplicityModel`] is the one source of these for every consumer (Design note 1): the
//! hierarchy draw (P11.T2) samples its distributions on streams it opens, and the quadratures
//! here integrate the same distributions for the galaxy's mean mass (plan 02), the stripped share
//! of massive primaries (plans 06 and 08) and the fit of Chabrier's high-mass scale (plan 15).
//! The figures are Duchêne and Kraus's (2013, ARA&A 51, 269) review and Raghavan et al.'s (2010,
//! ApJS 190, 1) survey of solar-type stars, with Sana et al. (2012, Science 337, 444) for O stars.
//!
//! Nothing here opens a stream or registers a domain tag, and nothing generated calls it yet.
//! Periods are in [`Days`](crate::units::Days) and, for densities and cumulative distributions,
//! in x = log₁₀(P ÷ 1 d); masses in [`SolarMasses`](crate::units::SolarMasses).

mod dist;
mod model;
mod quadrature;

pub use dist::{
    CIRCULARISATION_PERIOD, EccentricityDistribution, LOG_PERIOD_MAX, LOG_PERIOD_MIN,
    MIN_COMPANION_MASS, MIN_SUBSTELLAR_COMPANION_MASS, MassRatioDistribution, PeriodDistribution,
};
pub use model::{MAX_COMPANIONS, MultiplicityModel};
pub use quadrature::{all_stars_fraction_below, mean_companion_mass_per_system, stripped_share};
