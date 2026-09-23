//! The parameters of the planetary stage that belong to the generator version, as named constants
//! in one place (plan 14, "Generator version").
//!
//! Changing any of these moves generated bodies and needs a generator-version bump. The plan
//! gathers here the class weight table, the spacing floors, the ring probabilities, the
//! pulsar-planet probability, [`SATELLITE_STABILITY_FRACTION`] and the white dwarf pollution fit,
//! each added by the task that first uses it. The disc's own figures, which are measurements with
//! their sources beside the physics that uses them, live in [`disc`](super::disc).

use crate::units::JupiterMasses;

/// How far out a prograde satellite on a circular orbit about a planet on a circular orbit stays
/// bound: 0.4895 of the planet's Hill radius (design note 14).
///
/// Domingos, Winter and Yokoyama (2006, MNRAS 373, 1227, abstract), whose fit of the critical
/// semi-major axis is 0.4895 (1 − 1.0305 eₚ − 0.2738 eₛ) `R_H`; Rosario-Franco et al. (2020, AJ
/// 159, 260, Table 1) tabulate it as 0.4895 ± 0.0363, and Namouni (2010, ApJ 719, L145) quotes
/// 0.48. Rosario-Franco et al. find 0.40 when all twenty starting phases must survive 10⁵ years
/// (their §3.1.1). The same fraction cuts every system at 0.49 of its sphere of influence and
/// bounds moons inside Hill spheres.
pub const SATELLITE_STABILITY_FRACTION: f64 = 0.4895;

/// The floor on the spacing of two small planets on circular orbits: 10 mutual Hill radii
/// (design note 7).
///
/// Pu and Wu (2015, ApJ 807, 44), abstract and eq. 12: systems of Kepler-like planets survive a
/// thousand million years only if spaced by about 10 mutual Hill radii when circular and coplanar.
pub const SPACING_FLOOR_SMALL_CIRCULAR: f64 = 10.0;

/// How the small planets' floor rises with their mean eccentricity: 80 mutual Hill radii per unit
/// of mean eccentricity (design note 7; ruling 38).
///
/// Pu and Wu (2015, eq. 14): the threshold rises by one mutual Hill radius for each 0.01 of σₑ,
/// the Rayleigh scale of the eccentricities, 100 per unit σₑ. The mean of a Rayleigh distribution
/// is √(π ÷ 2) σₑ = 1.25 σₑ, so per unit of mean eccentricity the slope is 100 ÷ 1.25 = 80, and
/// the floor reaches 12 at a mean eccentricity of 0.025, their σₑ of 0.02.
pub const SPACING_FLOOR_ECCENTRICITY_SLOPE: f64 = 80.0;

/// The small planets' floor at its highest: 12 mutual Hill radii (design note 7; Pu and Wu 2015,
/// abstract, "∼12 if planetary orbits have eccentricities ∼0.02").
pub const SPACING_FLOOR_SMALL_ECCENTRIC: f64 = 12.0;

/// The floor on the spacing of a pair that includes a giant: 7 mutual Hill radii (design note 7).
///
/// Plan 14's figure, after Chambers, Wetherill and Boss (1996, Icarus 119, 261), Marzari and
/// Weidenschilling (2002, Icarus 156, 570) and Chatterjee et al. (2008, ApJ 686, 580), and the
/// least certain number of the floor. Two giants are Hill stable beyond 2√3 ≈ 3.5 (Gladman 1993);
/// Chatterjee et al.'s three-giant systems, spaced in the same mutual Hill radius (their eq. B2),
/// are mostly stable for 10⁹ years at 5.5 (their Fig. 29), so 7 is conservative. Jupiter and
/// Saturn sit 7.9 apart.
pub const SPACING_FLOOR_GIANT: f64 = 7.0;

/// The mass from which a planet counts as a giant for the spacing floor: 0.1 Jupiter masses,
/// about 32 M⊕ (design note 7).
pub const SPACING_GIANT_MASS: JupiterMasses = JupiterMasses::new(0.1);

/// The least gap between an inner planet's apocentre and its outer neighbour's pericentre, in
/// mutual Hill radii: 2√3 ≈ 3.46 (design note 7).
///
/// Gladman (1993, Icarus 106, 247): two planets on circular orbits separated by more than 2√3
/// mutual Hill radii can never meet. Applied at closest approach it makes "no overlapping orbits"
/// a theorem of the generator.
pub const HILL_STABLE_GAP: f64 = 3.464_101_615_137_754_6;
