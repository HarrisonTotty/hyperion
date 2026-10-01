//! Where a system's stars are at any time: each pair placed about its barycentre, from the root
//! down (plan 11, P11.T3.b), and with their velocities (`star_states_at`, added by rendering plan
//! R03's R03.T3).

use super::hierarchy::{HierarchyNode, NodeIndex, SystemHierarchy};
use crate::coords::{SystemPosition, SystemVelocity};
use crate::id::BodyId;
use crate::time::UniverseTime;

/// The position of every star of `h` at `t`, in the system frame from the system's barycentre,
/// written into `out` by body index (plan 11, P11.T3.b).
///
/// `out` is cleared first, so a caller can reuse one buffer. The walk starts with the root's
/// barycentre at the origin. A pair's orbit gives its outer member's barycentre relative to its
/// inner member's ([`KeplerElements::relative_state_at`](crate::orbit::KeplerElements::relative_state_at)),
/// and each member's barycentre is placed about the pair's by the other member's share of the
/// pair's mass: the inner one at `−(M_outer ÷ M) r`, the outer one at `+(M_inner ÷ M) r`. So the
/// system's barycentre stays at the origin at every time, to rounding, and a position is a pure
/// function of `h` and `t`, before or after the epoch. The masses are the stars' initial masses,
/// as the orbits' gravitational parameters are.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::SystemPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::multiplicity::{
///     MultiplicityContext, RedrawAttempt, draw_hierarchy, star_positions_at,
/// };
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let record = cell.first().expect("layer C is not empty at the solar circle");
/// let context = MultiplicityContext::ForcedMultiple { max_separation: None };
/// let stars = draw_hierarchy(&galaxy, record, context, RedrawAttempt::FIRST);
/// let mut positions = Vec::new();
/// star_positions_at(&stars, UniverseTime::EPOCH, &mut positions);
/// assert_eq!(positions.len(), stars.stars().len());
/// // The mass-weighted mean position is the barycentre, the origin.
/// let mut moment = [0.0; 3];
/// for ((_, at), star) in positions.iter().zip(stars.stars()) {
///     for (m, x) in moment.iter_mut().zip(at.metres()) {
///         *m += star.initial_mass().value() * x;
///     }
/// }
/// let total = stars.system_mass().value();
/// assert!(moment.iter().all(|m| (m / total).abs() < 1.0));
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
pub fn star_positions_at(
    h: &SystemHierarchy,
    t: UniverseTime,
    out: &mut Vec<(BodyId, SystemPosition)>,
) {
    out.clear();
    out.reserve(h.stars().len());
    place(h, h.root(), SystemPosition::ORIGIN, t, out);
}

/// Appends the stars under `node`, whose barycentre is at `centre`, in depth-first order, which is
/// body order.
fn place(
    h: &SystemHierarchy,
    node: NodeIndex,
    centre: SystemPosition,
    t: UniverseTime,
    out: &mut Vec<(BodyId, SystemPosition)>,
) {
    match h.node(node) {
        HierarchyNode::Star(star) => out.push((h.star(*star).body(), centre)),
        HierarchyNode::Pair {
            inner,
            outer,
            orbit,
        } => {
            let (separation, _) = orbit.relative_state_at(t);
            let inner_mass = h.node_mass(*inner).value();
            let outer_mass = h.node_mass(*outer).value();
            let mass = h.node_mass(node).value();
            place(
                h,
                *inner,
                centre.translated(separation * -(outer_mass / mass)),
                t,
                out,
            );
            place(
                h,
                *outer,
                centre.translated(separation * (inner_mass / mass)),
                t,
                out,
            );
        }
    }
}

/// The position and velocity of every star of `h` at `t`, in the system frame from the system's
/// barycentre, written into `out` by body index: [`star_positions_at`]'s walk with each star's
/// velocity beside its position (asked of plan 11 by rendering plan R03, and added by R03.T3).
///
/// The positions are [`star_positions_at`]'s bit for bit, by the same arithmetic in the same
/// order. A pair's orbit gives its outer member's barycentric velocity relative to its inner
/// member's ([`KeplerElements::relative_state_at`](crate::orbit::KeplerElements::relative_state_at)),
/// shared by the other member's share of the mass as the positions are, so the velocities are
/// relative to the system's barycentre, in m/s along the galactic axes, and the barycentre stays
/// at rest.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::multiplicity::{
///     MultiplicityContext, RedrawAttempt, draw_hierarchy, star_positions_at, star_states_at,
/// };
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let record = cell.first().expect("layer C is not empty at the solar circle");
/// let context = MultiplicityContext::ForcedMultiple { max_separation: None };
/// let stars = draw_hierarchy(&galaxy, record, context, RedrawAttempt::FIRST);
/// let (mut positions, mut states) = (Vec::new(), Vec::new());
/// star_positions_at(&stars, UniverseTime::EPOCH, &mut positions);
/// star_states_at(&stars, UniverseTime::EPOCH, &mut states);
/// // The same positions, now with velocities: the momentum about the barycentre is zero.
/// let mut momentum = [0.0; 3];
/// for (((_, at), (_, same, velocity)), star) in positions.iter().zip(&states).zip(stars.stars()) {
///     assert_eq!(at, same);
///     for (p, v) in momentum.iter_mut().zip(velocity.metres_per_second()) {
///         *p += star.initial_mass().value() * v;
///     }
/// }
/// assert!(momentum.iter().all(|p| p.abs() < 1e-6));
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
pub fn star_states_at(
    h: &SystemHierarchy,
    t: UniverseTime,
    out: &mut Vec<(BodyId, SystemPosition, SystemVelocity)>,
) {
    out.clear();
    out.reserve(h.stars().len());
    place_state(
        h,
        h.root(),
        (SystemPosition::ORIGIN, SystemVelocity::ZERO),
        t,
        out,
    );
}

/// Appends the stars under `node`, whose barycentre has the state `centre`, in depth-first order,
/// which is body order: [`place`] with the velocities.
fn place_state(
    h: &SystemHierarchy,
    node: NodeIndex,
    centre: (SystemPosition, SystemVelocity),
    t: UniverseTime,
    out: &mut Vec<(BodyId, SystemPosition, SystemVelocity)>,
) {
    let (at, moving) = centre;
    match h.node(node) {
        HierarchyNode::Star(star) => out.push((h.star(*star).body(), at, moving)),
        HierarchyNode::Pair {
            inner,
            outer,
            orbit,
        } => {
            let (separation, relative) = orbit.relative_state_at(t);
            let inner_mass = h.node_mass(*inner).value();
            let outer_mass = h.node_mass(*outer).value();
            let mass = h.node_mass(node).value();
            place_state(
                h,
                *inner,
                (
                    at.translated(separation * -(outer_mass / mass)),
                    moving + relative * -(outer_mass / mass),
                ),
                t,
                out,
            );
            place_state(
                h,
                *outer,
                (
                    at.translated(separation * (inner_mass / mass)),
                    moving + relative * (inner_mass / mass),
                ),
                t,
                out,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::order::assert_order_independent;

    use super::super::hierarchy::{MultiplicityContext, RedrawAttempt, draw_hierarchy};
    use super::super::testing::{SAMPLE, galaxy, imf_records, sunlike};
    use super::*;
    use crate::time::{ClockWindow, Span};
    use hyperion_testkit::float::assert_same_bits;

    /// 10⁴ hierarchies of the galaxy's mass function at the Sun-like point, multiples only.
    fn multiples() -> Vec<SystemHierarchy> {
        let galaxy = galaxy();
        let context = MultiplicityContext::ForcedMultiple {
            max_separation: None,
        };
        imf_records(&galaxy, SAMPLE, &sunlike(), 21)
            .iter()
            .map(|record| draw_hierarchy(&galaxy, record, context, RedrawAttempt::FIRST))
            .collect()
    }

    /// The times the tests ask about: ±H, the epoch, and a few in between.
    fn times() -> [UniverseTime; 5] {
        let year = Span::from_julian_years(1).expect("a year is a span");
        [
            ClockWindow::START,
            UniverseTime::EPOCH
                .checked_sub(year)
                .expect("inside the window"),
            UniverseTime::EPOCH,
            UniverseTime::EPOCH
                .checked_add(Span::from_seconds(123_456_789))
                .expect("inside the window"),
            ClockWindow::END,
        ]
    }

    /// `a × b` as an unevaluated sum `p + e`, exactly: Dekker's (1971) product, with Veltkamp's
    /// split, and no fused multiply-add.
    fn two_product(a: f64, b: f64) -> (f64, f64) {
        let split = |x: f64| {
            let c = 134_217_729.0 * x;
            let high = c - (c - x);
            (high, x - high)
        };
        let p = a * b;
        let ((ah, al), (bh, bl)) = (split(a), split(b));
        (p, ((ah * bh - p) + ah * bl + al * bh) + al * bl)
    }

    /// The mass-weighted mean of `positions`, m, computed exactly enough that only the positions'
    /// own rounding shows (each product exact, the sum compensated), and the largest distance
    /// among them.
    fn barycentre(h: &SystemHierarchy, positions: &[(BodyId, SystemPosition)]) -> (f64, f64) {
        let mut moment = [(0.0_f64, 0.0_f64); 3];
        let mut farthest: f64 = 0.0;
        for ((body, at), star) in positions.iter().zip(h.stars()) {
            assert_eq!(*body, star.body(), "positions come in body order");
            let mass = star.initial_mass().value();
            for ((sum, carry), coordinate) in moment.iter_mut().zip(at.metres()) {
                let (product, error) = two_product(mass, coordinate);
                for term in [product, error] {
                    // Neumaier's compensated sum.
                    let t = *sum + term;
                    *carry += if sum.abs() >= term.abs() {
                        (*sum - t) + term
                    } else {
                        (term - t) + *sum
                    };
                    *sum = t;
                }
            }
            farthest = farthest.max(at.distance_from_origin().value());
        }
        let mass = h.system_mass().value();
        let [x, y, z] = moment.map(|(sum, carry)| (sum + carry) / mass);
        ((x * x + y * y + z * z).sqrt(), farthest)
    }

    /// P11.T3.b: the barycentre of every one of 10⁴ hierarchies stays at the origin, at ±H and at
    /// the epoch, to 1 m.
    #[test]
    fn the_barycentre_of_ten_thousand_hierarchies_stays_at_the_origin() {
        let hierarchies = multiples();
        let mut positions = Vec::new();
        let (mut worst, mut worst_at, mut widest) = (0.0_f64, 0.0_f64, 0.0_f64);
        for h in &hierarchies {
            for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
                star_positions_at(h, t, &mut positions);
                assert_eq!(positions.len(), h.stars().len());
                let (offset, farthest) = barycentre(h, &positions);
                // The positions' own rounding: an f64 at 2 × 10¹⁶ m is spaced 4 m apart, so the
                // plan's 1 m is the positions' resolution at the widest orbits, and this is the
                // bound that holds at any separation.
                assert!(
                    offset <= f64::EPSILON * farthest.max(1.0),
                    "a barycentre {offset} m out, with the farthest star {farthest} m out"
                );
                if offset > worst {
                    (worst, worst_at) = (offset, farthest);
                }
                widest = widest.max(farthest);
            }
        }
        println!(
            "worst barycentre offset {worst:.3e} m, with a star {worst_at:.3e} m out; the \
             farthest star is {widest:.3e} m out"
        );
        // The plan's 1 m, where the positions can resolve it. The widest orbits now reach
        // 1.5–1.6 × 10¹⁶ m, where an f64 is spaced 2 m apart: since plan 02's P02.T12 (the
        // fixture's worst offset 1.29 m) and ruling 74's heavier massive-star anchors (1.48 m), so
        // the bound is the positions' own resolution there.
        assert!(
            worst < f64::max(1.0, f64::EPSILON * worst_at),
            "a barycentre {worst} m from the origin, with a star {worst_at} m out"
        );
    }

    /// P11.T3.b: a position is the same whatever was asked before, through the testkit's helper.
    #[test]
    fn a_position_does_not_depend_on_what_was_asked_before() {
        let hierarchies = multiples();
        let keys: Vec<(usize, UniverseTime)> =
            (0..300).flat_map(|i| times().map(|t| (i, t))).collect();
        assert_order_independent(&keys, |&(i, t)| {
            let mut out = Vec::new();
            star_positions_at(&hierarchies[i], t, &mut out);
            out
        });
        // A buffer reused across calls gives the same answer as a fresh one.
        let mut reused = Vec::new();
        for (i, t) in keys.iter().rev() {
            star_positions_at(&hierarchies[*i], *t, &mut reused);
            let mut fresh = Vec::new();
            star_positions_at(&hierarchies[*i], *t, &mut fresh);
            assert_eq!(reused, fresh);
        }
    }

    /// Each pair's members sit on its orbit: the outer member's barycentre less the inner one's is
    /// the orbit's relative position.
    #[test]
    fn each_pair_is_placed_on_its_orbit() {
        let hierarchies = multiples();
        let mut positions = Vec::new();
        for h in hierarchies.iter().take(2_000) {
            for t in times() {
                star_positions_at(h, t, &mut positions);
                let centre_of = |node: NodeIndex| {
                    let mut sum = [0.0; 3];
                    let mut mass = 0.0;
                    let mut stack = vec![node];
                    while let Some(n) = stack.pop() {
                        match h.node(n) {
                            HierarchyNode::Star(star) => {
                                let m = h.star(*star).initial_mass().value();
                                let at = positions[usize::from(star.get())].1.metres();
                                for (s, x) in sum.iter_mut().zip(at) {
                                    *s += m * x;
                                }
                                mass += m;
                            }
                            HierarchyNode::Pair { inner, outer, .. } => {
                                stack.extend([*inner, *outer]);
                            }
                        }
                    }
                    sum.map(|s| s / mass)
                };
                for (pair, orbit) in h.pairs() {
                    let HierarchyNode::Pair { inner, outer, .. } = *h.node(pair) else {
                        unreachable!("a pair");
                    };
                    let (inner_at, outer_at) = (centre_of(inner), centre_of(outer));
                    let expected = orbit.relative_state_at(t).0.metres();
                    let scale = orbit.apoapsis().value();
                    for axis in 0..3 {
                        let got = outer_at[axis] - inner_at[axis];
                        assert!(
                            (got - expected[axis]).abs() <= 1e-9 * scale + 1.0,
                            "axis {axis}: {got} against {}",
                            expected[axis]
                        );
                    }
                }
            }
        }
    }

    /// R03.T3: the states' positions are `star_positions_at`'s bit for bit.
    #[test]
    fn a_state_s_position_is_star_positions_at_s_bit_for_bit() {
        let hierarchies = multiples();
        let (mut positions, mut states) = (Vec::new(), Vec::new());
        for h in hierarchies.iter().take(2_000) {
            for t in times() {
                star_positions_at(h, t, &mut positions);
                star_states_at(h, t, &mut states);
                assert_eq!(positions.len(), states.len());
                for ((body, at), (same_body, same_at, _)) in positions.iter().zip(&states) {
                    assert_eq!(body, same_body);
                    for (x, y) in at.metres().into_iter().zip(same_at.metres()) {
                        assert_same_bits(x, y);
                    }
                }
            }
        }
    }

    /// R03.T3: a star's velocity is the time derivative of its position, by central differences
    /// over a second, to 10⁻⁶ relative, with a floor for the positions' own rounding: a Kepler
    /// state is good to a few ε of its orbit's apocentre (more near the pericentre of an
    /// eccentric orbit, where Kepler's equation amplifies it), so with X the widest apocentre or
    /// star distance, 4 ε X is allowed, which a difference over 2 s carries into the velocity.
    #[test]
    fn a_star_s_velocity_is_the_derivative_of_its_position() {
        let hierarchies = multiples();
        let second = Span::from_seconds(1);
        let (mut before, mut after, mut states) = (Vec::new(), Vec::new(), Vec::new());
        let mut checked = 0;
        for h in hierarchies.iter().take(2_000) {
            for t in times() {
                let (Some(early), Some(late)) = (t.checked_sub(second), t.checked_add(second))
                else {
                    continue;
                };
                star_positions_at(h, early, &mut before);
                star_positions_at(h, late, &mut after);
                star_states_at(h, t, &mut states);
                let reach = h
                    .pairs()
                    .map(|(_, orbit)| orbit.apoapsis().value())
                    .chain(
                        states
                            .iter()
                            .map(|(_, at, _)| at.distance_from_origin().value()),
                    )
                    .fold(0.0, f64::max);
                let floor = 4.0 * f64::EPSILON * reach;
                for (((_, x0), (_, x1)), (_, _, velocity)) in before.iter().zip(&after).zip(&states)
                {
                    let v = velocity.metres_per_second();
                    let speed = velocity.speed().value();
                    for (axis, v_axis) in v.into_iter().enumerate() {
                        let derivative = (x1.metres()[axis] - x0.metres()[axis]) / 2.0;
                        assert!(
                            (derivative - v_axis).abs() <= 1e-6 * speed + floor,
                            "axis {axis}: {derivative} m/s by differences against {v_axis} m/s"
                        );
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 1_000, "{checked} stars checked");
    }
}
