//! Frame selection: which system's frame a ship is in (plan 03, Design note 16).
//!
//! The brainstorm's "Coordinates": a system's sphere of influence is its tidal (Jacobi) radius,
//! and a ship inside several spheres belongs to the system for which distance ÷ radius is
//! smallest, the lower ID on an exact tie. It changes frame only when another system's ratio is
//! smaller by a tenth, so a ship drifting along a boundary does not flicker between frames. Outside
//! every sphere the ship is in the galactic frame. The rule needs only the systems near the ship,
//! which the range query supplies, and is evaluated at the query's time.
//!
//! [`select_frame`] is the rule itself, over candidates the caller has already found;
//! [`frame_at`] finds them, by walking each stellar layer over the reach of its own largest sphere
//! of influence and merging whatever the query's sources hold there. Both are pure functions of the
//! galaxy, the ship's position and the time: neither depends on the caller's cache, nor on the
//! order a walk happens to visit systems in.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::coords::GalacticPosition;
use crate::galaxy::placement::{CellCache, LayerSpec, STELLAR_LAYERS, SystemRecord};
use crate::galaxy::query::{
    LayerSet, QuerySphere, SystemHit, SystemSource, cells_in_sphere, hit_at, pad_for, pad_speed,
};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{Layer, SystemId};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::{LightYears, Metres, SolarMasses};

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

/// [`frame_at`] was asked about a time it cannot answer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindFrameError {
    /// The time lies outside the [`ClockWindow`], ±1,000 Julian years about the epoch, where
    /// present positions are guaranteed (plan 03, Design note 12). The search pads each layer's
    /// sphere by the farthest a system can drift since the epoch, so a time far outside the window
    /// would walk a sphere hundreds or millions of light-years across.
    TimeOutsideClockWindow(UniverseTime),
}

impl fmt::Display for FindFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimeOutsideClockWindow(t) => {
                write!(f, "the frame time {t} lies outside the clock window")
            }
        }
    }
}

impl Error for FindFrameError {}

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

/// How much wider than a layer's largest sphere of influence [`frame_at`] searches: a quarter
/// (plan 03, P03.T12.b).
///
/// A system holds the ship only while the ship is inside that system's own tidal radius, and within
/// a layer the radius is largest for the layer's heaviest primary, since it grows as the cube root
/// of the mass. Searching `1.25 ×` that largest radius, taken at the ship's own position, therefore
/// finds every system of the layer that could hold the ship unless the tidal radius of one mass
/// changes by more than a quarter between the ship and that system: the radius goes as
/// `(4Ω² − κ²)^(−⅓)`, so that needs the denominator to change by a factor of 1.95 within a few tens
/// of light-years, which the rotation curve does nowhere the grid places a system.
const SEARCH_MARGIN: f64 = 1.25;

/// The frame a ship at `ship` is in at time `t`, having been in `current`: a system's ID, or `None`
/// for the galactic frame (plan 03, Design note 16).
///
/// This is [`select_frame`] over the systems that could hold the ship. Each stellar layer is walked
/// separately, over a sphere of 1.25 times the tidal radius of the layer's heaviest primary at the
/// ship's position, so a layer of small stars costs a handful of cells rather than
/// the reach of layer E's giants; there is no census and no limit, because the answer is one ID and
/// the spheres are a few light-years across. Systems are placed at `t` by
/// [`position_at`](super::query::position_at), tested there by
/// [`hit_at`](super::query::hit_at) — a system not yet born holds nothing — and each one's tidal
/// radius is read at its own position from its primary's initial mass
/// ([`PotentialTables::tidal_radius`](super::potential::PotentialTables::tidal_radius)), which is
/// the brainstorm's sphere of influence until plans 06 and 11 give a present-day system mass.
///
/// `cache` is the caller's, as everywhere else in this plan, and the answer is the same whatever it
/// holds. `sources` are the query's non-grid sources, asked over the widest of the layers' spheres
/// and for every stellar layer, and honoured in both directions: their systems can take the ship,
/// and a source that suppresses a grid system — a pinned volume holding its own content — also
/// keeps that system from taking it.
///
/// A ship at the exact galactic centre, or beside a system there, is left to the galactic frame:
/// the tidal radius is zero at the centre, which is no sphere of influence, and plan 09's rule for
/// the centre's own members ("the smaller radius wins") is layered on there.
///
/// # Errors
///
/// [`FindFrameError::TimeOutsideClockWindow`] if `t` lies outside the
/// [`ClockWindow`], as [`RangeQuery::build`](super::query::RangeQuery::build) refuses such a time
/// for a range query: the pad for motion grows with |t|, so the walk would grow without bound.
///
/// # Panics
///
/// If a system's drift would take it out of the addressable cube, which
/// [`position_at`](super::query::position_at) cannot do while velocities are zero.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::frame::frame_at;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::from_params(Seed::new(19), GalaxyParams::milky_way_like());
/// let mut cache = NoCache::new();
///
/// // A ship sitting on a generated system is in that system's frame.
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let system = cell.first().expect("a 32 ly cell of the solar circle holds systems");
/// let here = system.epoch_position();
/// let frame = frame_at(&galaxy, &mut cache, &[], here, UniverseTime::EPOCH, None)?;
/// assert_eq!(frame, Some(system.id()));
///
/// // Far above the disc no sphere of influence reaches, so the ship is in the galactic frame.
/// let halo = GalacticPosition::from_light_years([0.0, 0.0, 60_000.0]).expect("in the cube");
/// assert_eq!(frame_at(&galaxy, &mut cache, &[], &halo, UniverseTime::EPOCH, None)?, None);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn frame_at<C: CellCache>(
    galaxy: &Galaxy,
    cache: &mut C,
    sources: &[&dyn SystemSource],
    ship: &GalacticPosition,
    t: UniverseTime,
    current: Option<SystemId>,
) -> Result<Option<SystemId>, FindFrameError> {
    if !ClockWindow::contains(t) {
        return Err(FindFrameError::TimeOutsideClockWindow(t));
    }
    let mut candidates = Vec::new();
    let mut widest: Option<QuerySphere> = None;
    for spec in STELLAR_LAYERS {
        let layer = spec.layer();
        let Some(sphere) =
            search_sphere(galaxy, ship, t, layer, SolarMasses::new(spec.band().hi()))
        else {
            continue;
        };
        for key in cells_in_sphere(layer, &sphere) {
            cache.with_cell(galaxy, key, |cell| {
                for record in cell {
                    // Tested first, then offered to the sources, so that a source is asked to
                    // suppress only systems that are in reach at `t`, which is the order its
                    // contract describes and the one `range_query` uses.
                    if let Some(hit) = hit_at(galaxy, record, &sphere)
                        && !suppressed(sources, galaxy, record, t)
                    {
                        candidates.extend(candidate_for(galaxy, &hit));
                    }
                }
            });
        }
        if widest.is_none_or(|held| {
            sphere
                .padded_radius()
                .total_cmp(&held.padded_radius())
                .is_gt()
        }) {
            widest = Some(sphere);
        }
    }

    // The sources are asked once, over the widest sphere any layer was walked with and for every
    // stellar layer, because a source reports its members by the layer they would fall in and this
    // rule has no census to admit layers by. A member farther away than its own layer's search
    // radius can only fail to hold the ship, never win.
    if let Some(sphere) = widest {
        let layers: LayerSet = STELLAR_LAYERS.iter().map(LayerSpec::layer).collect();
        let mut hits = Vec::new();
        for source in sources {
            source.systems_in_sphere(galaxy, &sphere, layers, &mut hits);
        }
        candidates.extend(hits.iter().filter_map(|hit| candidate_for(galaxy, hit)));
    }

    Ok(select_frame(&candidates, current))
}

/// The sphere of `layer` searched about `ship` at `t`, or `None` where the layer's largest sphere of
/// influence is not a sphere at all: at the exact galactic centre the tidal radius is zero.
///
/// The radius is [`SEARCH_MARGIN`] times the tidal radius of `largest` at the ship's position, and
/// the pad is the layer's, as in the range query, so that cells chosen by epoch position cannot
/// miss a system that has drifted in by `t` (plan 03, Design note 13).
#[must_use]
fn search_sphere(
    galaxy: &Galaxy,
    ship: &GalacticPosition,
    t: UniverseTime,
    layer: Layer,
    largest: SolarMasses,
) -> Option<QuerySphere> {
    let reach = galaxy
        .potential()
        .tidal_radius(largest, &PointLy::from(ship));
    let radius = LightYears::from(reach) * SEARCH_MARGIN;
    // The only rejection a positive finite radius can meet is a pad that is not finite, and the pad
    // is a few light-years over the whole clock window; a radius of zero is the centre, handled by
    // returning `None`.
    QuerySphere::new(*ship, radius, t, pad_for(t, pad_speed(layer))).ok()
}

/// The candidate a system near the ship makes, or `None` for a system whose tidal radius is no
/// sphere of influence.
///
/// The distance and the position are the hit's, so they are already taken at the query's time; the
/// tidal radius is read at that position, from the primary's initial mass (plan 03, Design note 16).
#[must_use]
fn candidate_for(galaxy: &Galaxy, hit: &SystemHit) -> Option<FrameCandidate> {
    let tidal_radius = galaxy.potential().tidal_radius(
        hit.record().primary_initial_mass(),
        &PointLy::from(hit.position()),
    );
    // The error is dropped rather than propagated because the one case that reaches it is a system
    // at the exact galactic centre, whose tidal radius is zero: it is no sphere of influence, and
    // the centre's members are plan 09's rule, not this one (plan 03, T12.a as built).
    FrameCandidate::new(hit.id(), Metres::from(hit.distance()), tidal_radius).ok()
}

/// Whether any source replaces this grid system at `t`, so that it cannot hold the ship either.
///
/// Asked in the order the sources were given and only until one says yes, exactly as
/// [`range_query`](super::query::range_query) asks it.
#[must_use]
fn suppressed(
    sources: &[&dyn SystemSource],
    galaxy: &Galaxy,
    record: &SystemRecord,
    t: UniverseTime,
) -> bool {
    sources
        .iter()
        .any(|source| source.suppresses(galaxy, record, t))
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
    fn the_hysteresis_boundary_is_inclusive_and_exact_ties_settle_once() {
        // Ratios chosen exact in f64: the current frame at 1, a rival at (1 − 0.1) × 1 = 0.9.
        let metres = |value| Metres::new(value);
        let current = FrameCandidate::new(id(1), metres(1.0), metres(1.0)).unwrap();
        let at_the_boundary = FrameCandidate::new(id(2), metres(0.9), metres(1.0)).unwrap();
        let just_short =
            FrameCandidate::new(id(3), metres(0.9_f64.next_up()), metres(1.0)).unwrap();
        assert_eq!(
            select_frame(&[current, at_the_boundary], Some(id(1))),
            Some(id(2)),
            "a rival exactly a tenth below takes over (Design note 16: ≤)"
        );
        assert_eq!(
            select_frame(&[current, just_short], Some(id(1))),
            Some(id(1)),
            "a rival one ulp short of a tenth below does not"
        );
        // A ship on two systems at once: both ratios are 0, and 0 ≤ 0.9 × 0, so the literal rule
        // hands the ship to the lower ID even from the higher, and then keeps it there.
        let on_both = [
            FrameCandidate::new(id(7), metres(0.0), metres(1.0)).unwrap(),
            FrameCandidate::new(id(8), metres(0.0), metres(1.0)).unwrap(),
        ];
        assert_eq!(select_frame(&on_both, None), Some(id(7)));
        assert_eq!(select_frame(&on_both, Some(id(8))), Some(id(7)));
        assert_eq!(select_frame(&on_both, Some(id(7))), Some(id(7)));
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
