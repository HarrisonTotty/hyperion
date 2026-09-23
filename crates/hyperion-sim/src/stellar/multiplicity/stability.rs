//! Whether a hierarchy of stars can last: Mardling and Aarseth's stability criterion for every
//! pair, and the tidal cut that keeps every orbit inside the system's sphere of influence (plan
//! 11, P11.T2.b, Design note 4).
//!
//! The hierarchy draw ([`draw_hierarchy`](super::draw_hierarchy)) redraws a new orbit until the
//! whole hierarchy passes [`Limits::admits`], so that every hierarchy it returns is stable by
//! construction.
//!
//! [`mardling_aarseth_limit`] documents the criterion, the paper's text and how quadruples use it.

use std::f64::consts::PI;

use super::hierarchy::{HierarchyNode, NodeIndex, StarIndex, SystemHierarchy};
use crate::galaxy::PointLy;
use crate::galaxy::potential::PotentialTables;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, OpenOrbit, Orientation};
use crate::units::{Metres, Radians, SolarMasses};

/// Mardling and Aarseth's (2001, MNRAS 321, 398, eq. 90) constant C, "determined empirically":
/// the critical outer periastron of a coplanar prograde triple with circular outer orbit and a
/// vanishing third mass is 2.8 inner semi-major axes.
pub const MARDLING_AARSETH_C: f64 = 2.8;

/// The slope of Mardling and Aarseth's (2001, §4.1) reduction factor f = 1 − 0.3 i ÷ π for a
/// mutual inclination i: the critical periastron of a retrograde coplanar system is 70% of a
/// prograde one's.
const INCLINATION_SLOPE: f64 = 0.3;

/// The slope of Mardling and Aarseth's (2001, §4.2) correction factor f₁ = 1 + 0.1 × the ratio
/// of the smaller to the larger inner semi-major axis, for a 2 + 2 quadruple.
const QUADRUPLE_SLOPE: f64 = 0.1;

/// The smallest outer-to-inner semi-major axis ratio the criterion admits for any masses,
/// eccentricity and inclination: C × (1 − 0.3) = 1.96, at a circular outer orbit, a vanishing
/// third mass and i = 180°. Each factor of the criterion is at least 1 otherwise, and the outer
/// periastron is at most the outer semi-major axis, so every stable pair has a larger ratio. The
/// hierarchy draw restricts a new orbit's period by it before the full test.
pub(super) const NECESSARY_AXIS_RATIO: f64 = MARDLING_AARSETH_C * (1.0 - INCLINATION_SLOPE);

/// The share of a system's tidal radius inside which every apocentre lies: one half.
///
/// The share is the plan's choice (Design note 4), which cites no measurement or integration for
/// it; it keeps every orbit well inside the Jacobi radius, where the Galactic tide is a small
/// perturbation. The tidal radius is plan 02's Jacobi radius ([`PotentialTables::tidal_radius`])
/// at the system's epoch position and at the sum of its stars' initial masses, the mass the
/// orbits are bound to, [`SystemHierarchy::system_mass`]. The frame rule (plan 03), which reads
/// the radius at the primary's mass alone, therefore always finds a companion inside its system's
/// sphere of influence: at most six stars, each no heavier than the primary, put the cut at no
/// more than 0.91 of that radius.
pub const TIDAL_CUT_SHARE: f64 = 0.5;

/// The largest eccentricity a companion's orbit may have: plan 14's
/// [`OpenOrbit::MIN_ECCENTRICITY`], 0.9999. A bound orbit at or above it is an open orbit's to
/// carry, because Kepler's equation is ill-conditioned there (ruling 39 of 2026-09-22), and the
/// draw redraws it. Only the widest pairs reach it: the eccentricity envelope allows it from
/// periods of about 5,500 years.
pub(super) const MAX_ECCENTRICITY: f64 = OpenOrbit::MIN_ECCENTRICITY;

/// The critical outer periastron of a triple, in units of its inner semi-major axis (Mardling and
/// Aarseth 2001, MNRAS 321, 398, eq. 90, with their inclination factor).
///
/// ```text
/// R_p,crit ÷ a_in = 2.8 (1 − 0.3 i ÷ π) [(1 + q_out) (1 + e_out) ÷ (1 − e_out)^½]^⅖
/// ```
///
/// for an outer mass ratio `q_out` = m₃ ÷ (m₁ + m₂), outer eccentricity `e_out` and mutual
/// inclination `mutual_inclination` in [0, π]. A triple is stable when its outer periastron
/// exceeds this many inner semi-major axes.
///
/// Mardling and Aarseth (2001, _Tidal interactions in star cluster simulations_, MNRAS 321, 398,
/// §4.1, eq. 90), re-checked against the paper: a triple whose inner binary has semi-major axis
/// `a_in`, and whose third body orbits the inner pair's centre of mass on an outer orbit of
/// eccentricity `e_out` and periastron `R_p`, with `q_out = m₃ ÷ (m₁ + m₂)`, is stable when
///
/// ```text
/// R_p ÷ a_in > C [(1 + q_out) (1 + e_out) ÷ (1 − e_out)^½]^⅖,   C = 2.8,
/// ```
///
/// "determined empirically", for coplanar prograde orbits, and "holds for `q_out` ≤ 5". Inclined
/// and retrograde systems are more stable, "up to around 30 per cent closer at outer periastron",
/// for which the paper employs "an additional ad hoc linear reduction factor f = 1 − 0.3 i ÷ 180
/// (with i in degrees)", i the mutual inclination of the two orbits, on the right-hand side. For
/// higher-order systems (§4.2) the same formula is used: in a 3 + 1 quadruple "the triple … plays
/// the role of the inner binary, with its own inner binary acting as a single object", and in a
/// 2 + 2 "the more tightly bound binary plays the role of the outer body", with "a correction
/// factor f₁ = 1 + 0.1 min(a ÷ a₂, a₂ ÷ a)" and "the largest semimajor axis … used in (90),
/// together with the appropriate mass ratio".
///
/// Here that is applied to every pair of a hierarchy, each pair's members taken as point masses:
///
/// - a pair of two stars is a binary, and the criterion does not apply;
/// - a pair of a star and a pair is a triple, whose inner binary is the member pair;
/// - a pair of two pairs is a 2 + 2, whose inner binary is the member with the larger semi-major
///   axis and whose third body is the other, with f₁.
///
/// A light inner binary about a heavy star has `q_out` above 5, outside the range the paper
/// states. The formula is applied there too: it grows as `q_out^⅖`, faster than the `q_out^⅓` of
/// the Hill radius that bounds such a binary, so it errs on the stable side.
///
/// # Examples
///
/// Three equal masses (`q_out` = ½) on circular coplanar orbits need the outer orbit about 3.3
/// times wider than the inner one, and an eccentric outer orbit much more:
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::orbit::Eccentricity;
/// use hyperion_sim::stellar::multiplicity::mardling_aarseth_limit;
/// use hyperion_sim::units::Radians;
///
/// let circular = mardling_aarseth_limit(0.5, Eccentricity::CIRCULAR, Radians::ZERO);
/// assert!((circular - 2.8 * math::powf(1.5, 0.4)).abs() < 1e-12);
/// let eccentric = mardling_aarseth_limit(0.5, Eccentricity::new(0.6)?, Radians::ZERO);
/// // The outer periastron is 0.4 of the outer semi-major axis.
/// assert!(eccentric / 0.4 > 3.0 * circular);
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[must_use]
pub fn mardling_aarseth_limit(q_out: f64, e_out: Eccentricity, mutual_inclination: Radians) -> f64 {
    let e = e_out.value();
    let reduction = 1.0 - INCLINATION_SLOPE * mutual_inclination.value() / PI;
    let bracket = (1.0 + q_out) * (1.0 + e) / (1.0 - e).sqrt();
    MARDLING_AARSETH_C * reduction * math::powf(bracket, 0.4)
}

/// The angle between two orbits' planes, in [0, π]: the angle between their angular momenta.
#[must_use]
pub fn mutual_inclination(first: &Orientation, second: &Orientation) -> Radians {
    let [ax, ay, az] = first.normal();
    let [bx, by, bz] = second.normal();
    let cosine = (ax * bx + ay * by + az * bz).clamp(-1.0, 1.0);
    Radians::new(math::acos(cosine))
}

/// The outer periastron below which the pair `pair` of `h` is unstable by Mardling and
/// Aarseth's criterion, or `None` for a pair of two stars, a binary, to which it does not apply
/// (see [`mardling_aarseth_limit`]).
///
/// # Panics
///
/// If `pair` is not a pair of `h`.
#[must_use]
pub(super) fn critical_periapsis(h: &SystemHierarchy, pair: NodeIndex) -> Option<Metres> {
    let HierarchyNode::Pair {
        inner,
        outer,
        orbit,
    } = h.node(pair)
    else {
        panic!("node {pair:?} is a star, not a pair");
    };
    let (binary, binary_orbit, third, factor) = match (h.node(*inner), h.node(*outer)) {
        (HierarchyNode::Star(_), HierarchyNode::Star(_)) => return None,
        (HierarchyNode::Pair { orbit: b, .. }, HierarchyNode::Star(_)) => (*inner, b, *outer, 1.0),
        (HierarchyNode::Star(_), HierarchyNode::Pair { orbit: b, .. }) => (*outer, b, *inner, 1.0),
        (HierarchyNode::Pair { orbit: a, .. }, HierarchyNode::Pair { orbit: b, .. }) => {
            let (a_inner, a_outer) = (a.semi_major_axis(), b.semi_major_axis());
            if a_inner >= a_outer {
                (
                    *inner,
                    a,
                    *outer,
                    1.0 + QUADRUPLE_SLOPE * (a_outer / a_inner),
                )
            } else {
                (
                    *outer,
                    b,
                    *inner,
                    1.0 + QUADRUPLE_SLOPE * (a_inner / a_outer),
                )
            }
        }
    };
    let q_out = h.node_mass(third) / h.node_mass(binary);
    let limit = mardling_aarseth_limit(
        q_out,
        orbit.eccentricity(),
        mutual_inclination(binary_orbit.orientation(), orbit.orientation()),
    );
    Some(binary_orbit.semi_major_axis() * (factor * limit))
}

/// What the primary's own orbit, the pair whose inner member is the primary, must satisfy: the
/// condition the primary's companion-stripped mark puts on it (plan 11, Design note 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Innermost {
    /// Nothing: the primary is too light for the mark, or the context ignores it.
    Free,
    /// The mark is set: the primary has a companion, and its orbit's periastron is below the
    /// interacting periastron given.
    Interacting(Metres),
    /// The mark is not set: the primary is single, or its orbit's periastron is at or beyond the
    /// interacting periastron given.
    Wide(Metres),
}

/// Everything a hierarchy must satisfy besides stability: the tidal cut at its place, a context's
/// widest separation, and the primary's stripped mark.
#[derive(Debug, Clone, Copy)]
pub(super) struct Limits<'g> {
    potential: &'g PotentialTables,
    point: PointLy,
    max_separation: Option<Metres>,
    innermost: Innermost,
}

impl<'g> Limits<'g> {
    /// The limits of a system at `point` in the galaxy of `potential`.
    #[must_use]
    pub(super) fn new(
        potential: &'g PotentialTables,
        point: PointLy,
        max_separation: Option<Metres>,
        innermost: Innermost,
    ) -> Self {
        Self {
            potential,
            point,
            max_separation,
            innermost,
        }
    }

    /// What the primary's stripped mark asks of its orbit.
    #[must_use]
    pub(super) fn innermost(&self) -> Innermost {
        self.innermost
    }

    /// The tidal cut of a system of total initial mass `mass`: [`TIDAL_CUT_SHARE`] of its tidal
    /// radius.
    #[must_use]
    pub(super) fn tidal_cut(&self, mass: SolarMasses) -> Metres {
        self.potential.tidal_radius(mass, &self.point) * TIDAL_CUT_SHARE
    }

    /// The largest semi-major axis any orbit of a system of total initial mass `mass` can have:
    /// its tidal cut, which bounds the apocentre and so the axis, or the context's widest
    /// separation if that is smaller.
    #[must_use]
    pub(super) fn widest_axis(&self, mass: SolarMasses) -> Metres {
        let cut = self.tidal_cut(mass);
        match self.max_separation {
            Some(widest) if widest < cut => widest,
            _ => cut,
        }
    }

    /// Whether `h` passes everything: every pair stable by Mardling and Aarseth's criterion
    /// ([`critical_periapsis`]), every apocentre inside the tidal cut at the system's total
    /// initial mass, every semi-major axis inside the context's widest separation, and the
    /// primary's orbit as its stripped mark requires.
    #[must_use]
    pub(super) fn admits(&self, h: &SystemHierarchy) -> bool {
        let cut = self.tidal_cut(h.system_mass());
        let orbits_fit = h.pairs().all(|(pair, orbit)| {
            orbit.apoapsis() <= cut
                && self
                    .max_separation
                    .is_none_or(|widest| orbit.semi_major_axis() <= widest)
                && critical_periapsis(h, pair).is_none_or(|critical| orbit.periapsis() > critical)
        });
        orbits_fit && self.innermost_fits(h)
    }

    /// Whether the primary's orbit satisfies [`Innermost`].
    #[must_use]
    fn innermost_fits(&self, h: &SystemHierarchy) -> bool {
        let primary_orbit = primary_orbit(h);
        match self.innermost {
            Innermost::Free => true,
            Innermost::Interacting(threshold) => {
                primary_orbit.is_some_and(|orbit| orbit.periapsis() < threshold)
            }
            Innermost::Wide(threshold) => {
                primary_orbit.is_none_or(|orbit| orbit.periapsis() >= threshold)
            }
        }
    }
}

/// The orbit of the pair whose inner member is the primary, star 0, or `None` for a single star.
///
/// The primary is the first star of the hierarchy's depth-first order, so the pair is the last
/// one met going from the root through inner members.
#[must_use]
pub(super) fn primary_orbit(h: &SystemHierarchy) -> Option<&KeplerElements> {
    let mut node = h.root();
    let mut orbit = None;
    while let HierarchyNode::Pair {
        inner, orbit: o, ..
    } = h.node(node)
    {
        orbit = Some(o);
        node = *inner;
    }
    debug_assert_eq!(
        h.node(node),
        &HierarchyNode::Star(StarIndex::PRIMARY),
        "the first star in depth-first order is the primary"
    );
    orbit
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn orientation(i: f64, node: f64, argument: f64) -> Orientation {
        Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument)).unwrap()
    }

    /// The paper's eq. 90 at hand-computed points: 2.8 × 2^0.4 = 3.6946 for a third body as heavy
    /// as the inner pair on circular coplanar orbits, and the bracket's three factors each raised
    /// to 2/5.
    #[test]
    fn the_criterion_reproduces_equation_ninety() {
        let equal = mardling_aarseth_limit(1.0, Eccentricity::CIRCULAR, Radians::ZERO);
        assert!((equal - 3.694_622_15).abs() < 1e-8, "{equal}");
        let e = Eccentricity::new(0.5).unwrap();
        let eccentric = mardling_aarseth_limit(0.5, e, Radians::ZERO);
        let by_hand = 2.8 * math::powf(1.5 * 1.5 / 0.5_f64.sqrt(), 0.4);
        assert!((eccentric - by_hand).abs() < 1e-12, "{eccentric} {by_hand}");
        // A vanishing third body on a circular orbit: the constant itself.
        assert_same_bits(
            mardling_aarseth_limit(0.0, Eccentricity::CIRCULAR, Radians::ZERO),
            MARDLING_AARSETH_C,
        );
    }

    /// The inclination factor runs linearly from 1 for prograde coplanar orbits to 0.7 for
    /// retrograde ones (Mardling and Aarseth 2001, §4.1: "up to around 30 per cent closer").
    #[test]
    fn inclined_and_retrograde_orbits_are_more_stable() {
        let at = |i: f64| mardling_aarseth_limit(0.3, Eccentricity::CIRCULAR, Radians::new(i));
        let prograde = at(0.0);
        assert!((at(PI / 2.0) / prograde - 0.85).abs() < 1e-15);
        assert!((at(PI) / prograde - 0.7).abs() < 1e-15);
        assert!(at(1.0) < prograde && at(2.0) < at(1.0));
        assert_same_bits(
            NECESSARY_AXIS_RATIO,
            mardling_aarseth_limit(0.0, Eccentricity::CIRCULAR, Radians::new(PI)),
        );
    }

    #[test]
    fn the_mutual_inclination_is_the_angle_between_the_planes() {
        let flat = orientation(0.0, 0.0, 0.0);
        let tilted = orientation(0.7, 1.9, 0.3);
        assert!(mutual_inclination(&flat, &flat).value().abs() < 1e-7);
        assert!((mutual_inclination(&flat, &tilted).value() - 0.7).abs() < 1e-12);
        let reversed = orientation(PI - 0.7, 1.9, 0.3);
        let between = mutual_inclination(&tilted, &reversed).value();
        // Same node, inclinations 0.7 and π − 0.7: the planes are π − 1.4 apart.
        assert!((between - (PI - 1.4)).abs() < 1e-12, "{between}");
        assert_same_bits(
            mutual_inclination(&tilted, &reversed).value(),
            mutual_inclination(&reversed, &tilted).value(),
        );
    }
}
