//! Frame selection: which system's frame a ship is in (plan 03, Design note 16).
//!
//! The brainstorm's "Coordinates": a system's sphere of influence is its tidal (Jacobi) radius,
//! and a ship inside several spheres belongs to the system for which distance ÷ radius is
//! smallest, the lower ID on an exact tie. It changes frame only when another system's ratio is
//! smaller by a tenth, so a ship drifting along a boundary does not flicker between frames. Outside
//! every sphere the ship is in the galactic frame. The rule needs only the systems near the ship,
//! which the range query supplies, and is evaluated at the query's time.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::id::SystemId;
use crate::units::Metres;

/// How much smaller a rival's ratio of distance to tidal radius must be before it takes the ship
/// from its current frame: a tenth (brainstorm, "Coordinates": "It changes frame only when another
/// system's ratio is smaller by a tenth").
pub const FRAME_HYSTERESIS: f64 = 0.1;

/// A [`FrameCandidate`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildFrameCandidateError {
    /// The distance is not finite and non-negative.
    InvalidDistance,
    /// The tidal radius is not finite and positive.
    InvalidTidalRadius,
}

impl fmt::Display for BuildFrameCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDistance => "a frame candidate's distance must be finite and non-negative",
            Self::InvalidTidalRadius => {
                "a frame candidate's tidal radius must be finite and positive"
            }
        })
    }
}

impl Error for BuildFrameCandidateError {}

/// A system near the ship, as the frame rule sees it: its ID, the ship's distance from it and its
/// tidal radius, both at the time the frame is asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameCandidate {
    id: SystemId,
    distance: Metres,
    tidal_radius: Metres,
}

impl FrameCandidate {
    /// A candidate system `id`, `distance` from the ship, with the tidal radius `tidal_radius`.
    ///
    /// A distance of −0 is stored as +0, so that the ratio's order under `total_cmp` is the
    /// ratio's value.
    ///
    /// # Errors
    ///
    /// - [`BuildFrameCandidateError::InvalidDistance`] unless `distance` is finite and not
    ///   negative.
    /// - [`BuildFrameCandidateError::InvalidTidalRadius`] unless `tidal_radius` is finite and
    ///   positive.
    pub fn new(
        id: SystemId,
        distance: Metres,
        tidal_radius: Metres,
    ) -> Result<Self, BuildFrameCandidateError> {
        if !(distance.value().is_finite() && distance.value() >= 0.0) {
            return Err(BuildFrameCandidateError::InvalidDistance);
        }
        if !(tidal_radius.value().is_finite() && tidal_radius.value() > 0.0) {
            return Err(BuildFrameCandidateError::InvalidTidalRadius);
        }
        Ok(Self {
            id,
            distance: Metres::new(distance.value() + 0.0),
            tidal_radius,
        })
    }

    /// The system.
    #[must_use]
    pub const fn id(&self) -> SystemId {
        self.id
    }

    /// The ship's distance from the system's barycentre.
    #[must_use]
    pub const fn distance(&self) -> Metres {
        self.distance
    }

    /// The system's tidal (Jacobi) radius: its sphere of influence.
    #[must_use]
    pub const fn tidal_radius(&self) -> Metres {
        self.tidal_radius
    }

    /// Distance ÷ tidal radius: at most 1 inside the sphere.
    ///
    /// Never negative or NaN; infinite only for a ship far outside the sphere.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        self.distance / self.tidal_radius
    }

    /// Whether the ship is inside the system's sphere of influence: a ratio of at most 1.
    #[must_use]
    pub fn contains_ship(&self) -> bool {
        self.ratio() <= 1.0
    }
}

/// Orders candidates by ratio, then by ID: the first is the frame a ship with no current frame
/// takes.
#[must_use]
fn rank(a: &FrameCandidate, b: &FrameCandidate) -> Ordering {
    a.ratio().total_cmp(&b.ratio()).then(a.id.cmp(&b.id))
}

/// The frame a ship is in, given the systems near it and the frame it was in: a system's ID, or
/// `None` for the galactic frame (plan 03, Design note 16).
///
/// A ship is inside a system's sphere when distance ÷ tidal radius ≤ 1. With no current frame,
/// the smallest ratio wins, the lower ID on an exact tie. With a current frame whose ratio is
/// still ≤ 1, the best rival takes over only when its ratio is at most (1 − [`FRAME_HYSTERESIS`])
/// × the current one, so a ship drifting along a boundary does not flicker between frames. If the
/// ship has left its current frame's sphere, or the current system is not among `candidates`, the
/// rule runs as if there were no current frame. Outside every sphere the answer is `None`.
///
/// Ratios are compared with `total_cmp`. If an ID appears more than once, its smallest ratio
/// counts. The answer does not depend on the order of `candidates`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::frame::{FrameCandidate, select_frame};
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::units::{LightYears, Metres};
///
/// let ly = |value| Metres::from(LightYears::new(value));
/// let near = SystemId::from_raw(0x0200_0800_2000_0001)?;
/// let far = SystemId::from_raw(0x0200_0800_2000_0002)?;
/// // A ship 2 ly from one system (radius 4 ly) and 2.6 ly from another (radius 5 ly).
/// let candidates = [
///     FrameCandidate::new(near, ly(2.0), ly(4.0))?,
///     FrameCandidate::new(far, ly(2.6), ly(5.0))?,
/// ];
/// // Arriving from interstellar space it takes the smaller ratio, 0.5 against 0.52.
/// assert_eq!(select_frame(&candidates, None), Some(near));
/// // Already in the other's frame it stays: 0.5 is not a tenth below 0.52.
/// assert_eq!(select_frame(&candidates, Some(far)), Some(far));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn select_frame(candidates: &[FrameCandidate], current: Option<SystemId>) -> Option<SystemId> {
    let inside = || candidates.iter().filter(|c| c.contains_ship());
    let best = inside().min_by(|a, b| rank(a, b))?;
    let Some(current) =
        current.and_then(|id| inside().filter(|c| c.id == id).min_by(|a, b| rank(a, b)))
    else {
        return Some(best.id);
    };
    if best.ratio() <= (1.0 - FRAME_HYSTERESIS) * current.ratio() {
        Some(best.id)
    } else {
        Some(current.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::Layer;
    use crate::units::LightYears;

    fn id(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [0, 3_250, 0]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn ly(value: f64) -> Metres {
        Metres::from(LightYears::new(value))
    }

    /// A candidate at `ratio` of a 4 ly tidal radius.
    fn at_ratio(index: u32, ratio: f64) -> FrameCandidate {
        FrameCandidate::new(id(index), ly(4.0 * ratio), ly(4.0)).unwrap()
    }

    #[test]
    fn the_smallest_ratio_wins_without_a_current_frame() {
        let candidates = [at_ratio(1, 0.7), at_ratio(2, 0.3), at_ratio(3, 0.5)];
        assert_eq!(select_frame(&candidates, None), Some(id(2)));
        let mut reversed = candidates;
        reversed.reverse();
        assert_eq!(select_frame(&reversed, None), Some(id(2)));
    }

    #[test]
    fn an_exact_tie_goes_to_the_lower_id() {
        let tied = [at_ratio(9, 0.5), at_ratio(4, 0.5), at_ratio(7, 0.5)];
        assert_eq!(tied[0].ratio().total_cmp(&tied[1].ratio()), Ordering::Equal);
        assert_eq!(select_frame(&tied, None), Some(id(4)));
    }

    #[test]
    fn a_rival_takes_over_only_a_tenth_below_the_current_ratio() {
        let current = at_ratio(1, 0.8);
        let close = at_ratio(2, 0.8 * 0.95);
        assert_eq!(select_frame(&[current, close], Some(id(1))), Some(id(1)));
        let clear = at_ratio(3, 0.8 * 0.89);
        assert_eq!(select_frame(&[current, clear], Some(id(1))), Some(id(3)));
        assert_eq!(
            select_frame(&[current, close, clear], Some(id(1))),
            Some(id(3))
        );
        // Without the current frame, the smaller ratio wins at once.
        assert_eq!(select_frame(&[current, close], None), Some(id(2)));
    }

    #[test]
    fn a_ship_moving_along_the_boundary_of_two_equal_spheres_changes_frame_at_most_once() {
        // Two systems 4 ly apart on the x axis, each with a 5 ly radius. The ship runs from near
        // the first to near the second, zigzagging across the plane that bisects them.
        let (first, second) = (id(1), id(2));
        let (first_x, second_x, radius) = (-2.0, 2.0, 5.0);
        let mut frame = None;
        let mut changes = 0;
        for step in 0..=400_u32 {
            let progress = f64::from(step) / 400.0;
            let wobble = if step % 2 == 0 { 0.05 } else { -0.05 };
            let (x, y) = (-0.5 + progress + wobble, 3.0 * progress - 1.5);
            let distance = |centre_x: f64| ly(((x - centre_x) * (x - centre_x) + y * y).sqrt());
            let candidates = [
                FrameCandidate::new(first, distance(first_x), ly(radius)).unwrap(),
                FrameCandidate::new(second, distance(second_x), ly(radius)).unwrap(),
            ];
            let next = select_frame(&candidates, frame);
            assert!(next.is_some(), "the ship stays inside both spheres");
            if frame.is_some() && next != frame {
                changes += 1;
            }
            frame = next;
        }
        assert!(changes <= 1, "{changes} changes of frame");
        // The same path without hysteresis would flicker at every step near the plane.
        let flickers = (0..=400_u32)
            .map(|step| {
                let progress = f64::from(step) / 400.0;
                let wobble = if step % 2 == 0 { 0.05 } else { -0.05 };
                -0.5 + progress + wobble < 0.0
            })
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|pair| pair[0] != pair[1])
            .count();
        assert!(flickers > 10);
    }

    #[test]
    fn outside_every_sphere_the_ship_is_in_the_galactic_frame() {
        let outside = [at_ratio(1, 1.2), at_ratio(2, 3.0)];
        assert_eq!(select_frame(&outside, None), None);
        assert_eq!(select_frame(&outside, Some(id(1))), None);
        assert_eq!(select_frame(&[], Some(id(1))), None);
        // On the sphere itself the ship is inside.
        assert_eq!(select_frame(&[at_ratio(5, 1.0)], None), Some(id(5)));
    }

    #[test]
    fn a_current_frame_the_ship_has_left_is_dropped() {
        let left = at_ratio(1, 1.01);
        let rival = at_ratio(2, 0.99);
        // The rival is not a tenth below, but the current frame no longer holds the ship.
        assert_eq!(select_frame(&[left, rival], Some(id(1))), Some(id(2)));
        // A current system that is no longer among the candidates is dropped too.
        assert_eq!(select_frame(&[rival], Some(id(1))), Some(id(2)));
    }

    #[test]
    fn a_repeated_id_counts_at_its_smallest_ratio() {
        // The current system is listed twice. At its first entry, 0.9, the rival at 0.47 would be
        // a tenth below; at its smallest, 0.5, the rival is not.
        let candidates = [at_ratio(1, 0.9), at_ratio(1, 0.5), at_ratio(2, 0.47)];
        assert_eq!(select_frame(&candidates, Some(id(1))), Some(id(1)));
        // An entry outside the sphere does not stand for an entry inside it.
        let candidates = [at_ratio(3, 1.5), at_ratio(3, 0.6), at_ratio(4, 0.58)];
        assert_eq!(select_frame(&candidates, Some(id(3))), Some(id(3)));
    }

    /// Every order of `items`, by Heap's algorithm.
    fn permutations<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
        fn heap<T: Copy>(k: usize, items: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
            if k <= 1 {
                out.push(items.clone());
                return;
            }
            for i in 0..k {
                heap(k - 1, items, out);
                let swap_with = if k.is_multiple_of(2) { i } else { 0 };
                items.swap(swap_with, k - 1);
            }
        }
        let mut items = items.to_vec();
        let mut out = Vec::new();
        heap(items.len(), &mut items, &mut out);
        out
    }

    #[test]
    fn the_frame_does_not_depend_on_the_order_of_the_candidates() {
        // A repeated ID (one entry inside, one outside), an exact tie that the lower ID wins, a
        // rival about 0.89 of the others, and a system outside its sphere.
        let candidates = [
            at_ratio(5, 0.8),
            at_ratio(5, 1.2),
            at_ratio(3, 0.8),
            at_ratio(8, 0.8 * 0.89),
            at_ratio(9, 2.0),
        ];
        let orders = permutations(&candidates);
        assert_eq!(orders.len(), 120);
        for current in [None, Some(id(3)), Some(id(5)), Some(id(8)), Some(id(9))] {
            let answer = select_frame(&candidates, current);
            for order in &orders {
                assert_eq!(select_frame(order, current), answer, "{current:?}");
            }
        }
        assert_eq!(select_frame(&candidates, None), Some(id(8)));
        assert_eq!(select_frame(&candidates, Some(id(9))), Some(id(8)));
    }

    #[test]
    fn a_negative_zero_distance_ranks_as_zero() {
        let at_zero = FrameCandidate::new(id(7), Metres::new(-0.0), ly(4.0)).unwrap();
        let also_zero = FrameCandidate::new(id(6), Metres::ZERO, ly(4.0)).unwrap();
        // The lower ID wins the exact tie: −0 does not sort before +0.
        assert_eq!(select_frame(&[at_zero, also_zero], None), Some(id(6)));
    }

    #[test]
    fn candidates_reject_bad_distances_and_radii() {
        for distance in [-1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                FrameCandidate::new(id(1), Metres::new(distance), ly(4.0)),
                Err(BuildFrameCandidateError::InvalidDistance),
                "{distance}"
            );
        }
        for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                FrameCandidate::new(id(1), ly(1.0), Metres::new(radius)),
                Err(BuildFrameCandidateError::InvalidTidalRadius),
                "{radius}"
            );
        }
        assert!(FrameCandidate::new(id(1), Metres::ZERO, ly(4.0)).is_ok());
    }
}
