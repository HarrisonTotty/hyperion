//! The body-frame rule: which body's non-rotating frame a camera, and later a ship, is in within a
//! system (plan R02, Design note 6).
//!
//! The rendering brainstorm's "The floating origin is already in the simulation" puts a camera
//! near a body in that body's frame, so that the numbers it differences stay small. The boundary
//! is a **precision device, not a dynamical one**: with every body's gravity integrated, the
//! choice of frame changes only rounding, so the larger, already computed Hill sphere is kept
//! rather than the Laplace sphere of influence of patched conics. The radius is plan 14's
//! pericentre form, a (1 − e) (m ÷ 3M)^⅓
//! ([`hill_radius`](crate::planetary::derive::limits::hill_radius)), constant in time, so the
//! boundary does not breathe with the orbit.
//!
//! The rule follows the galaxy's own ([`galaxy::frame::select_frame`], plan 03), with nesting
//! added: the camera is in the frame of the **innermost** body whose sphere holds it, the system
//! frame otherwise; among siblings at one depth, whose spheres can overlap, the smallest ratio of
//! distance to Hill radius wins, a rival taking over only at (1 − [`FRAME_HYSTERESIS`]) of the
//! current ratio, the lower ID on an exact tie; and each sphere's own boundary is a Schmitt band,
//! entered at a ratio of at most [`BODY_FRAME_ENTRY`] and left above 1.
//!
//! A body frame is non-rotating and **free-falling, not inertial**: it has no Coriolis or
//! centrifugal terms, but a flight model that adopts the rule must integrate the other bodies'
//! tidal residual (the indirect term) in every body frame, as Cowell and Encke propagation do.
//!
//! The client's camera runs a TypeScript twin of [`select_body_frame`]
//! (`view/camera/frames.ts`), held to it by the golden file `frame/body_frames.golden`.
//!
//! [`galaxy::frame::select_frame`]: crate::galaxy::frame::select_frame

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::galaxy::frame::FRAME_HYSTERESIS;
use crate::id::BodyId;
use crate::units::Metres;

/// The ratio of distance to Hill radius at or below which a camera enters a body's frame: 0.9
/// (plan R02, Design note 6). It leaves above 1, so a camera on the boundary does not flicker; the
/// band is 5.8 × 10⁶ m for the Moon and 1.5 × 10⁸ m for the Earth.
///
/// Its sibling for systems is [`FRAME_HYSTERESIS`], which this rule also applies between
/// overlapping siblings.
pub const BODY_FRAME_ENTRY: f64 = 0.9;

/// A [`BodyFrameCandidate`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildBodyFrameCandidateError {
    /// The distance is not finite and non-negative.
    InvalidDistance,
    /// The Hill radius is not finite and positive.
    InvalidHillRadius,
    /// The body names itself as its parent.
    OwnParent,
}

impl fmt::Display for BuildBodyFrameCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDistance => {
                "a body-frame candidate's distance must be finite and non-negative"
            }
            Self::InvalidHillRadius => {
                "a body-frame candidate's Hill radius must be finite and positive"
            }
            Self::OwnParent => "a body-frame candidate cannot be its own parent",
        })
    }
}

impl Error for BuildBodyFrameCandidateError {}

/// A body near the camera, as the body-frame rule sees it: its ID, the body it orbits, the
/// camera's distance from it and its Hill radius.
///
/// Only planets, dwarf planets and moons are candidates; a star is not, and a wide multiple's
/// stellar components are a matter for the craft plan. The distance is geometric and present,
/// from system-frame positions at the frame time, never an apparent one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyFrameCandidate {
    id: BodyId,
    parent: Option<BodyId>,
    distance: Metres,
    hill_radius: Metres,
}

impl BodyFrameCandidate {
    /// A candidate body `id` orbiting `parent`, `distance` from the camera, with Hill radius
    /// `hill_radius`.
    ///
    /// `parent` is the body `id` orbits: a planet's star (or `None`), a moon's planet. Nesting is
    /// read from it: a candidate whose parent is another candidate is one level deeper. A
    /// distance of −0 is stored as +0, so that the ratio's order under `total_cmp` is its value.
    ///
    /// # Errors
    ///
    /// - [`BuildBodyFrameCandidateError::InvalidDistance`] unless `distance` is finite and not
    ///   negative.
    /// - [`BuildBodyFrameCandidateError::InvalidHillRadius`] unless `hill_radius` is finite and
    ///   positive.
    /// - [`BuildBodyFrameCandidateError::OwnParent`] if `parent` is `id`.
    pub fn new(
        id: BodyId,
        parent: Option<BodyId>,
        distance: Metres,
        hill_radius: Metres,
    ) -> Result<Self, BuildBodyFrameCandidateError> {
        if !(distance.value().is_finite() && distance.value() >= 0.0) {
            return Err(BuildBodyFrameCandidateError::InvalidDistance);
        }
        if !(hill_radius.value().is_finite() && hill_radius.value() > 0.0) {
            return Err(BuildBodyFrameCandidateError::InvalidHillRadius);
        }
        if parent == Some(id) {
            return Err(BuildBodyFrameCandidateError::OwnParent);
        }
        Ok(Self {
            id,
            parent,
            distance: Metres::new(distance.value() + 0.0),
            hill_radius,
        })
    }

    /// The body.
    #[must_use]
    pub const fn id(&self) -> BodyId {
        self.id
    }

    /// The body it orbits, if any.
    #[must_use]
    pub const fn parent(&self) -> Option<BodyId> {
        self.parent
    }

    /// The camera's distance from the body's centre.
    #[must_use]
    pub const fn distance(&self) -> Metres {
        self.distance
    }

    /// The body's Hill radius at pericentre.
    #[must_use]
    pub const fn hill_radius(&self) -> Metres {
        self.hill_radius
    }

    /// Distance ÷ Hill radius: at most 1 inside the sphere. Never negative or NaN.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        self.distance / self.hill_radius
    }
}

/// Orders candidates by ratio, then by ID: the first of a set of siblings is the frame a camera
/// with no current frame among them takes.
fn rank(a: &BodyFrameCandidate, b: &BodyFrameCandidate) -> Ordering {
    a.ratio().total_cmp(&b.ratio()).then(a.id.cmp(&b.id))
}

/// The body frame a camera is in, given the bodies near it and the frame it was in: a body's ID,
/// or `None` for the system frame (plan R02, Design note 6).
///
/// While the camera is inside `current`'s sphere (`current` is among the candidates with a ratio
/// of at most 1), `current` and its ancestors among the candidates form the **chain**. A
/// candidate is **eligible** when its ratio of distance to Hill radius is at most
/// [`BODY_FRAME_ENTRY`], or at most 1 when it is in the chain. Of the eligible, the deepest in the
/// parent chain wins (a moon over its planet). Among eligible siblings at that depth the smallest
/// ratio wins, the lower ID on an exact tie, except that the one of them in the chain keeps the
/// camera until a rival's ratio is at most (1 − [`FRAME_HYSTERESIS`]) times its own. When the
/// camera has left `current`'s sphere (ratio above 1), or `current` is not among the candidates,
/// the chain is empty and the whole rule runs as if there were no current frame, as
/// [`select_frame`](crate::galaxy::frame::select_frame) does. With nothing eligible the answer is
/// `None`.
///
/// Depth is counted along `parent` links between the candidates themselves: a candidate whose
/// parent is not a candidate is at depth 0. A cycle of parents, which no real system has, is cut
/// after as many steps as there are candidates, and the depths it gives are unspecified. If an ID
/// appears more than once, its smallest ratio
/// counts. The answer does not depend on the order of `candidates`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::body_frame::{BodyFrameCandidate, select_body_frame};
/// use hyperion_sim::units::Metres;
///
/// let sun = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let (earth, moon) = (BodyId::new(sun, 0x103), BodyId::new(sun, 0x203));
/// // A camera 5.4 × 10⁷ m from the Moon: 0.93 of its Hill radius, 0.27 of the Earth's.
/// let at = |moon_distance: f64| -> Result<_, Box<dyn std::error::Error>> {
///     Ok([
///         BodyFrameCandidate::new(earth, None, Metres::new(3.9e8), Metres::new(1.47e9))?,
///         BodyFrameCandidate::new(moon, Some(earth), Metres::new(moon_distance), Metres::new(5.8e7))?,
///     ])
/// };
/// // Arriving from the Earth's frame, the camera is not yet in the Moon's: 0.93 is above 0.9.
/// assert_eq!(select_body_frame(&at(5.4e7)?, Some(earth)), Some(earth));
/// // Already in the Moon's frame, it stays there until it leaves the sphere.
/// assert_eq!(select_body_frame(&at(5.4e7)?, Some(moon)), Some(moon));
/// assert_eq!(select_body_frame(&at(5.9e7)?, Some(moon)), Some(earth));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn select_body_frame(
    candidates: &[BodyFrameCandidate],
    current: Option<BodyId>,
) -> Option<BodyId> {
    // One entry per ID, its smallest-ratio candidate, in ID order: nothing below depends on the
    // order of `candidates`.
    let mut by_id: BTreeMap<BodyId, BodyFrameCandidate> = BTreeMap::new();
    for candidate in candidates {
        by_id
            .entry(candidate.id)
            .and_modify(|kept| {
                if rank(candidate, kept)
                    .then(candidate.parent.cmp(&kept.parent))
                    .is_lt()
                {
                    *kept = *candidate;
                }
            })
            .or_insert(*candidate);
    }
    // The chain from `id` up through its candidate ancestors, cut after as many steps as there
    // are candidates so that a malformed cycle of parents cannot loop.
    let ancestors = |id: BodyId| -> Vec<BodyId> {
        let mut chain = Vec::new();
        let mut next = by_id.get(&id);
        while let Some(candidate) = next {
            if chain.len() >= by_id.len() {
                break;
            }
            chain.push(candidate.id);
            next = candidate.parent.and_then(|parent| by_id.get(&parent));
        }
        chain
    };
    let depths: BTreeMap<BodyId, usize> =
        by_id.keys().map(|&id| (id, ancestors(id).len())).collect();
    let depth = |id: BodyId| depths.get(&id).copied().unwrap_or(0);
    let chain = current
        .filter(|id| by_id.get(id).is_some_and(|c| c.ratio() <= 1.0))
        .map(ancestors)
        .unwrap_or_default();
    let eligible = |c: &&BodyFrameCandidate| {
        c.ratio() <= BODY_FRAME_ENTRY || (chain.contains(&c.id) && c.ratio() <= 1.0)
    };

    let deepest = by_id.values().filter(eligible).map(|c| depth(c.id)).max()?;
    let at_depth = || {
        by_id
            .values()
            .filter(eligible)
            .filter(move |c| depth(c.id) == deepest)
    };
    let best = at_depth().min_by(|a, b| rank(a, b))?;
    let Some(incumbent) = at_depth().find(|c| chain.contains(&c.id)) else {
        return Some(best.id);
    };
    if best.ratio() <= (1.0 - FRAME_HYSTERESIS) * incumbent.ratio() {
        Some(best.id)
    } else {
        Some(incumbent.id)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::id::SystemId;
    use crate::planetary::derive::limits::hill_radius;
    use crate::units::Kilograms;

    fn system() -> SystemId {
        SystemId::from_raw(0x0200_0800_2000_0000).unwrap()
    }

    fn body(index: u16) -> BodyId {
        BodyId::new(system(), index)
    }

    fn candidate(
        id: BodyId,
        parent: Option<BodyId>,
        distance: f64,
        hill: f64,
    ) -> BodyFrameCandidate {
        BodyFrameCandidate::new(id, parent, Metres::new(distance), Metres::new(hill)).unwrap()
    }

    /// The Earth's and the Moon's Hill radii at pericentre (plan 14's form): the Earth's orbit a
    /// = 1.496 × 10¹¹ m, e = 0.0167, 5.972 × 10²⁴ kg about 1.989 × 10³⁰ kg; the Moon's a = 3.844 ×
    /// 10⁸ m, e = 0.0549, 7.342 × 10²² kg (NASA planetary fact sheets).
    fn earth_and_moon_hill() -> (f64, f64) {
        let earth = hill_radius(
            Metres::new(1.496e11),
            0.0167,
            Kilograms::new(5.972e24),
            Kilograms::new(1.989e30),
        );
        let moon = hill_radius(
            Metres::new(3.844e8),
            0.0549,
            Kilograms::new(7.342e22),
            Kilograms::new(5.972e24),
        );
        (earth.value(), moon.value())
    }

    /// The Earth and the Moon as candidates, for a camera `ratio` of the Moon's Hill radius from
    /// the Moon on the side away from the Earth.
    fn earth_moon_at(ratio: f64) -> [BodyFrameCandidate; 2] {
        let (earth_hill, moon_hill) = earth_and_moon_hill();
        let from_moon = ratio * moon_hill;
        [
            candidate(body(1), None, 3.844e8 + from_moon, earth_hill),
            candidate(body(2), Some(body(1)), from_moon, moon_hill),
        ]
    }

    #[test]
    fn hill_radii_are_the_plan_s_figures() {
        let (earth, moon) = earth_and_moon_hill();
        assert!((1.45e9..1.5e9).contains(&earth), "{earth}");
        assert!((5.7e7..5.9e7).contains(&moon), "{moon}");
    }

    #[test]
    fn the_moon_s_frame_is_entered_at_nine_tenths_and_left_above_one() {
        let (earth, moon) = (Some(body(1)), Some(body(2)));
        assert_eq!(select_body_frame(&earth_moon_at(0.95), earth), earth);
        assert_eq!(select_body_frame(&earth_moon_at(0.9), earth), moon);
        assert_eq!(select_body_frame(&earth_moon_at(0.95), moon), moon);
        assert_eq!(select_body_frame(&earth_moon_at(1.0), moon), moon);
        assert_eq!(select_body_frame(&earth_moon_at(1.01), moon), earth);
        // With no current frame the entry band applies to both.
        assert_eq!(select_body_frame(&earth_moon_at(0.95), None), earth);
        assert_eq!(select_body_frame(&earth_moon_at(0.5), None), moon);
    }

    #[test]
    fn outside_every_sphere_the_camera_is_in_the_system_frame() {
        let (earth_hill, moon_hill) = earth_and_moon_hill();
        let far = [
            candidate(body(1), None, 2.0 * earth_hill, earth_hill),
            candidate(body(2), Some(body(1)), 2.0 * earth_hill, moon_hill),
        ];
        assert_eq!(select_body_frame(&far, None), None);
        assert_eq!(select_body_frame(&far, Some(body(2))), None);
        assert_eq!(select_body_frame(&[], Some(body(1))), None);
        // In the Earth's band but not past its entry: none from outside, the Earth from inside.
        let band = [candidate(body(1), None, 0.95 * earth_hill, earth_hill)];
        assert_eq!(select_body_frame(&band, None), None);
        assert_eq!(select_body_frame(&band, Some(body(1))), Some(body(1)));
    }

    #[test]
    fn overlapping_siblings_hand_over_only_at_a_tenth_s_advantage() {
        let (a, b) = (body(1), body(2));
        let at = |ra: f64, rb: f64| {
            [
                candidate(a, None, ra * 1e9, 1e9),
                candidate(b, None, rb * 1e9, 1e9),
            ]
        };
        assert_eq!(select_body_frame(&at(0.5, 0.46), Some(a)), Some(a));
        assert_eq!(select_body_frame(&at(0.5, 0.45), Some(a)), Some(b));
        assert_eq!(select_body_frame(&at(0.5, 0.46), None), Some(b));
        // An exact tie goes to the lower ID, and only with no incumbent.
        assert_eq!(select_body_frame(&at(0.5, 0.5), None), Some(a));
        assert_eq!(select_body_frame(&at(0.5, 0.5), Some(b)), Some(b));
    }

    #[test]
    fn a_massive_moon_nests_as_the_moon_does() {
        // Pluto (a = 5.906 × 10¹² m, e = 0.2488 about the Sun: NASA Pluto fact sheet; 1.303 ×
        // 10²² kg) and Charon (a = 1.96 × 10⁷ m, e = 0, 1.586 × 10²¹ kg): the masses from the
        // system's GM fit, and Charon's orbit, in Brozović et al. 2015, Icarus 246, 317.
        let pluto_hill = hill_radius(
            Metres::new(5.906e12),
            0.2488,
            Kilograms::new(1.303e22),
            Kilograms::new(1.989e30),
        )
        .value();
        let charon_hill = hill_radius(
            Metres::new(1.96e7),
            0.0,
            Kilograms::new(1.586e21),
            Kilograms::new(1.303e22),
        )
        .value();
        assert!((6.6e6..6.8e6).contains(&charon_hill), "{charon_hill}");
        let (pluto, charon) = (body(1), body(2));
        let at = |ratio: f64| {
            [
                candidate(pluto, None, 1.96e7 + ratio * charon_hill, pluto_hill),
                candidate(charon, Some(pluto), ratio * charon_hill, charon_hill),
            ]
        };
        assert_eq!(select_body_frame(&at(0.95), Some(pluto)), Some(pluto));
        assert_eq!(select_body_frame(&at(0.85), Some(pluto)), Some(charon));
        assert_eq!(select_body_frame(&at(0.95), Some(charon)), Some(charon));
        assert_eq!(select_body_frame(&at(1.05), Some(charon)), Some(pluto));
    }

    #[test]
    fn the_answer_does_not_depend_on_the_candidates_order() {
        let (earth_hill, moon_hill) = earth_and_moon_hill();
        let set = [
            candidate(body(1), None, 3.9e8, earth_hill),
            candidate(body(2), Some(body(1)), 0.95 * moon_hill, moon_hill),
            candidate(body(3), Some(body(1)), 0.93 * moon_hill, moon_hill),
            candidate(body(4), None, 3.0e8, 2.0e9),
            candidate(body(5), Some(body(4)), 1.0e7, 5.0e7),
        ];
        let orders: Vec<Vec<BodyFrameCandidate>> = (0..set.len())
            .map(|shift| {
                let mut order = set.to_vec();
                order.rotate_left(shift);
                if shift % 2 == 1 {
                    order.reverse();
                }
                order
            })
            .collect();
        for current in [
            None,
            Some(body(1)),
            Some(body(2)),
            Some(body(4)),
            Some(body(5)),
        ] {
            let first = select_body_frame(&set, current);
            for order in &orders {
                assert_eq!(select_body_frame(order, current), first, "{current:?}");
            }
            assert_order_independent(&orders, |o| select_body_frame(o, current));
        }
        // The deepest eligible body wins: body 5 at 0.2 of its sphere, inside body 4's.
        assert_eq!(select_body_frame(&set, None), Some(body(5)));
    }

    #[test]
    fn leaving_a_moon_s_sphere_re_runs_the_rule_with_no_current_frame() {
        let (earth_hill, moon_hill) = earth_and_moon_hill();
        // Beyond the Moon's sphere and in the Earth's band (0.95): not re-entered from outside.
        let set = [
            candidate(body(1), None, 0.95 * earth_hill, earth_hill),
            candidate(body(2), Some(body(1)), 1.01 * moon_hill, moon_hill),
        ];
        assert_eq!(select_body_frame(&set, Some(body(2))), None);
        // A current frame no longer among the candidates is the same as none.
        assert_eq!(select_body_frame(&set[..1], Some(body(2))), None);
        assert_eq!(select_body_frame(&set[..1], Some(body(1))), Some(body(1)));
    }

    #[test]
    fn a_moon_s_moon_nests_two_deep() {
        let set = [
            candidate(body(1), None, 3.0e8, 1.5e9),
            candidate(body(2), Some(body(1)), 3.0e7, 6.0e7),
            candidate(body(3), Some(body(2)), 1.0e5, 1.0e6),
        ];
        assert_eq!(select_body_frame(&set, None), Some(body(3)));
        assert_eq!(select_body_frame(&set, Some(body(1))), Some(body(3)));
        // Beyond the sub-moon's sphere, its planet's moon holds the camera.
        let out = [
            set[0],
            set[1],
            candidate(body(3), Some(body(2)), 2.0e6, 1.0e6),
        ];
        assert_eq!(select_body_frame(&out, Some(body(3))), Some(body(2)));
    }

    #[test]
    fn a_duplicate_id_counts_its_smallest_ratio() {
        let set = [
            candidate(body(1), None, 9.5e8, 1e9),
            candidate(body(1), None, 5.0e8, 1e9),
        ];
        assert_eq!(select_body_frame(&set, None), Some(body(1)));
    }

    #[test]
    fn a_cycle_of_parents_ends() {
        let set = [
            candidate(body(1), Some(body(2)), 5.0e8, 1e9),
            candidate(body(2), Some(body(1)), 6.0e8, 1e9),
        ];
        assert!(select_body_frame(&set, Some(body(1))).is_some());
    }

    #[test]
    fn malformed_candidates_are_refused() {
        let id = body(1);
        let refuse = |d: f64, h: f64, parent| {
            BodyFrameCandidate::new(id, parent, Metres::new(d), Metres::new(h)).unwrap_err()
        };
        assert_eq!(
            refuse(-1.0, 1.0, None),
            BuildBodyFrameCandidateError::InvalidDistance
        );
        assert_eq!(
            refuse(f64::NAN, 1.0, None),
            BuildBodyFrameCandidateError::InvalidDistance
        );
        assert_eq!(
            refuse(1.0, 0.0, None),
            BuildBodyFrameCandidateError::InvalidHillRadius
        );
        assert_eq!(
            refuse(1.0, f64::INFINITY, None),
            BuildBodyFrameCandidateError::InvalidHillRadius
        );
        assert_eq!(
            refuse(1.0, 1.0, Some(id)),
            BuildBodyFrameCandidateError::OwnParent
        );
        let zero = candidate(id, None, -0.0, 1.0);
        assert!(zero.ratio().is_sign_positive());
    }
}
