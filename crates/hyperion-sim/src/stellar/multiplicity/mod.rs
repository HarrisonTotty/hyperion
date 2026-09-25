//! Multiplicity: how many stellar companions a star has, the periods, mass ratios and
//! eccentricities of their orbits (plan 11, P11.T1), the hierarchy they form (P11.T2) and where
//! each star is at any time (P11.T3.b).
//!
//! [`MultiplicityModel`] is the one source of these for every consumer (Design note 1): the
//! hierarchy draw ([`draw_hierarchy`]) samples its distributions on the streams it opens, and the
//! quadratures here integrate the same distributions for the galaxy's mean mass (plan 02), the
//! stripped share of massive primaries (plans 06 and 08) and the fit of Chabrier's high-mass
//! scale (plan 15). The figures are Duchêne and Kraus's (2013, ARA&A 51, 269) review and
//! Raghavan et al.'s (2010, ApJS 190, 1) survey of solar-type stars, with Sana et al. (2012,
//! Science 337, 444) for O stars and Moe and Di Stefano (2017, ApJS 230, 15) for mass ratios.
//!
//! A hierarchy ([`SystemHierarchy`]) is stable by construction: every pair passes Mardling and
//! Aarseth's (2001, MNRAS 321, 398) criterion ([`mardling_aarseth_limit`]) and every apocentre
//! lies inside half the system's tidal radius ([`TIDAL_CUT_SHARE`]). [`star_positions_at`] places
//! its stars about the barycentre at any time.
//!
//! The model, the distributions and the quadratures open no stream. The hierarchy draw opens
//! `system.multiplicity` under the system's ID and `binary.orbit`,
//! `binary.orientation` and `binary.phase` under each companion's body ID, and reads plan 06's
//! companion-stripped mark of a massive primary. Plan 06's
//! [`SystemStars`](crate::stellar::system::SystemStars) calls it for every system (P11.T2.c).
//! Periods are in [`Days`](crate::units::Days) and, for densities and cumulative
//! distributions, in x = log₁₀(P ÷ 1 d); masses in [`SolarMasses`](crate::units::SolarMasses).

mod direct;
mod dist;
mod hierarchy;
mod model;
mod positions;
mod quadrature;
mod stability;
#[cfg(test)]
pub(crate) mod testing;

pub use dist::{
    CIRCULARISATION_PERIOD, ECCENTRICITY_ENVELOPE_PERIOD, EccentricityDistribution, LOG_PERIOD_MAX,
    LOG_PERIOD_MIN, MIN_COMPANION_MASS, MIN_SUBSTELLAR_COMPANION_MASS, MassRatioDistribution,
    PeriodDistribution,
};
#[cfg(test)]
pub(crate) use hierarchy::hand_built;
pub use hierarchy::{
    DRAWS_PER_ATTEMPT, HierarchyNode, MAX_REDRAWS, MAX_STABILITY_REDRAWS, MultiplicityContext,
    NodeIndex, PROVISIONAL_INTERACTING_PERIASTRON, PROVISIONAL_STRIPPED_SHARE, RedrawAttempt,
    STAR_BODY_INDEX_END, STRIPPED_MARK_MIN_MASS, SlotKind, StarIndex, StarSlot, SystemHierarchy,
    draw_hierarchy,
};
pub use model::{MAX_COMPANIONS, MultiplicityModel};
pub use positions::star_positions_at;
pub use quadrature::{all_stars_fraction_below, mean_companion_mass_per_system, stripped_share};
pub use stability::{
    MARDLING_AARSETH_C, TIDAL_CUT_SHARE, mardling_aarseth_limit, mutual_inclination,
};
