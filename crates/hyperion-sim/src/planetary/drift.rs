//! Evolving orbits: a continuous phase and drifting elements on aligned cells (plan 14, P14.T45.a).
//!
//! Three laws change an orbit's elements inside a segment of its life:
//! - a giant-impact moon's tidal recession (P14.T18);
//! - circularisation (P14.T8.e);
//! - adiabatic expansion as its host loses mass (P14.T28.b).
//!
//! **The phase is the integral of the mean motion.** An evolving orbit's mean anomaly is
//! M(t) = M(anchor) + ∫ n dt' from its anchor: the epoch for an orbit present then, else the
//! nearest end of its segment. A body then moves at its own elements' Kepler speed, and its
//! phase does not depend on where the clock's origin sits. The receding moon's integral is in
//! closed form. Circularisation and mass loss integrate by the 16-point Gauss–Legendre rule on
//! each interval between the law's break points ([`composite_excess`], P14.T45.d): the host's
//! track knots and segment boundaries, its sudden deaths and the eccentricity floor's corners.
//! Between two of them the mean motion is smooth (a polynomial of degree 6 in time where a host's
//! winds alone move it, since n ∝ M² and the track's mass is a cubic in age between knots), so
//! the rule is exact there to rounding, and a fixed set of panels makes the integral one function
//! of time. The one corner left unbroken is the track's floor at the core mass, which an envelope
//! run down at the AGB's end could meet inside a knot interval; the rule is then exact only to
//! that corner's error, as T45.a's one panel was at every kink. The host's mass is read smoothly in time for it
//! ([`StarModel::phase_and_mass_at`](crate::stellar::system::StarModel::phase_and_mass_at)):
//! read at a summed age of gigayears it steps every few seconds, and the steps' noise in the
//! integral stepped the velocity at cell ends by up to 10⁻² m s⁻¹ on AGB hosts. [`EvolvingLaw`] is
//! a law with its anchor, and its phase is the anchored elements' own phase plus
//! [`EvolvingLaw::phase_excess`], ∫ (n − n at the anchor) dt'.
//!
//! **What the integral holds to.** Inside the clock window the law's own phase error, times the
//! along-track factor a √((1 + e) ÷ (1 − e)), is at most a tenth of [`DRIFT_TOLERANCE`] (10 µm),
//! plus a rounding allowance of 4 · 2⁻⁵² times the phase ∫ n dt the panels add, times the same
//! factor: 4 to 8 units in the last place of the phase gained since the anchor, of the order a
//! fixed orbit's phase also carries. The allowance is the phase's and not the excess's because
//! each evaluation of n carries a relative rounding of a few 2⁻⁵³ before the anchor's mean motion
//! is subtracted. It dominates beyond days from the anchor: at the window's edge it is about
//! 0.2 m for an Earth formed at 3 au about a late-AGB host of 2 M☉, and the errors measured on
//! evolved hosts of 1–3 M☉ stay under a fifth of it. Before the window's start the panels are
//! millennia long and the rounding of the evaluations accumulates past it: the bound there is
//! 16 · 2⁻⁵² of the phase gained. For that Earth, the error was 2.1 times the 4 · 2⁻⁵² allowance
//! at −10⁵ yr (69 m, with 6.7 × 10⁴ rad gained), and at the source horizon's start, −(H + L),
//! 0.87 times it (107 m, with 2.6 × 10⁵ rad gained; the 16 · 2⁻⁵² bound is 490 m).
//!
//! **The trajectory is a model on aligned cells.** The model holds on dyadic cells [tᵣ, tₑ) of
//! S = 2^k s, 2^[`DRIFT_CELL_MIN_LOG2`] to 2^[`DRIFT_CELL_MAX_LOG2`], aligned to the epoch and
//! cut at the segment's ends, the clock window's and the law's break points, so that no kink of
//! the law lies inside a cell.
//! - **Inside a cell:** a = aᵣ + ȧ Δt, e = eᵣ + ė Δt and M = Mᵣ + nᵣ Δt + ½ ṅ Δt²
//!   ([`KeplerElements::drifting_state_at`]).
//! - **The rates:** secants that meet the law at both ends of the cell.
//! - **The cell's size:** the largest k whose model stays within [`DRIFT_TOLERANCE`] plus 2⁻⁵² of
//!   the apoapsis of the law at the cell's quarter points. The law's phase inside a cell is the
//!   cell's own integral, so the check measures the model and not the rounding of an integral
//!   from a distant anchor, about 10⁻¹⁶ of the phase gained since it, which the model shares out
//!   across the cell as it must to join its neighbours.
//!
//! Dyadic cells nest, so the search returns the same cell for every time inside it: records are
//! the same across clients and request times. The record carries the model ([`DriftingOrbit`]),
//! and the client evaluates the same formula (`lib/orbit.ts`), so the two agree as for a fixed
//! orbit. The record's `valid_until` is the cell's end, or the next change of the body's state or
//! segment if earlier.

use std::cmp::Ordering;
use std::f64::consts::TAU;

use crate::coords::{SystemVector, SystemVelocity};
use crate::math;
use crate::orbit::{DriftRates, KeplerElements, seconds_between};
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::time::{ClockWindow, NANOS_PER_SECOND, Span, UniverseTime};
use crate::units::Metres;

/// The largest drift cell, 2²⁵ s, about 1.06 Julian years (P14.T45.a).
///
/// It keeps a record's displayed axis and eccentricity within a year of the time asked, which is
/// plan 14's D18 re-request rule. It also bounds the traffic: one re-send per evolving orbit per
/// 2²⁵ s of scene time, about 5.6 minutes of real time at 100,000×.
pub const DRIFT_CELL_MAX_LOG2: u32 = 25;

/// The smallest drift cell, 2¹⁶ s, about 18 hours (P14.T45.a): the cell an orbit falls back to
/// when no larger one meets [`DRIFT_TOLERANCE`].
pub const DRIFT_CELL_MIN_LOG2: u32 = 16;

/// How far the model of an evolving orbit may stray from its law inside a cell, m, before the
/// cell is halved: 10⁻⁴ m, to which 2⁻⁵² of the orbit's apoapsis is added, one unit in the last
/// place of a position there (P14.T45.a).
///
/// The bound is checked at the cell's quarter points, on a first-order bound of the
/// displacement: |δa| (1 + e) + a (1 + 1 ÷ √(1 − e²)) |δe| + a √((1 + e) ÷ (1 − e)) |δM|. Each
/// factor bounds the largest displacement over the orbit per unit change of its element at a fixed
/// mean anomaly. The two-body relations are Murray and Dermott 1999, *Solar System Dynamics*,
/// eq. 2.52 and §2.5 for ∂E ÷ ∂e: r ÷ a ≤ 1 + e for the axis, and v ÷ n, largest at pericentre,
/// for the mean anomaly. For the eccentricity, |∂r ÷ ∂e| ÷ a is 2 on a circle and about 22 at
/// e = 0.999; 1 + 1 ÷ √(1 − e²) is an envelope of it, not taken from the source but checked
/// numerically over 0 ≤ e ≤ 0.9999 and every eccentric anomaly, tight as e → 0. The rates are
/// secants and the phase term is fitted to the cell's whole integral, so the error vanishes at
/// both ends of the cell. The phase's leftover cubic error peaks at two thirds of the cell, where
/// it is 4 ÷ 27 of its scale against the three-quarter point's 9 ÷ 64, so the samples see about
/// 95% (243 ÷ 256) of it: the peak may exceed the checked bound by up to 5.3%.
pub const DRIFT_TOLERANCE: Metres = Metres::new(1e-4);

/// The rates of an evolving orbit's model on one drift cell (P14.T45.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitDrift {
    reference: UniverseTime,
    semi_major_axis_rate_m_per_s: f64,
    eccentricity_rate_per_s: f64,
    mean_motion_rate_rad_per_s2: f64,
}

impl OrbitDrift {
    /// The time the elements hold at exactly: the cell's start.
    #[must_use]
    pub const fn reference(&self) -> UniverseTime {
        self.reference
    }

    /// ȧ, m s⁻¹.
    #[must_use]
    pub const fn semi_major_axis_rate_m_per_s(&self) -> f64 {
        self.semi_major_axis_rate_m_per_s
    }

    /// ė, s⁻¹.
    #[must_use]
    pub const fn eccentricity_rate_per_s(&self) -> f64 {
        self.eccentricity_rate_per_s
    }

    /// ṅ, rad s⁻².
    #[must_use]
    pub const fn mean_motion_rate_rad_per_s2(&self) -> f64 {
        self.mean_motion_rate_rad_per_s2
    }

    /// The rates as [`KeplerElements::drifting_state_at`] takes them.
    #[must_use]
    const fn rates(&self) -> DriftRates {
        DriftRates {
            a_m_per_s: self.semi_major_axis_rate_m_per_s,
            e_per_s: self.eccentricity_rate_per_s,
            n_rad_per_s2: self.mean_motion_rate_rad_per_s2,
        }
    }
}

/// An orbit as a record holds it: elements, and for an evolving orbit the rates they change at
/// from its reference time (P14.T45.a).
///
/// A record's orbit gives the body's position about its primary up to its `valid_until`; for an
/// evolving orbit that is the end of its drift cell, past which the next record holds.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::drift::DriftingOrbit;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{GravitationalParameter, Metres, Radians};
///
/// // A Moon-like orbit that holds: propagated as its elements say.
/// let elements = KeplerElements::from_semi_major_axis(
///     Metres::new(3.844e8),
///     GravitationalParameter::new(4.035e14),
///     Eccentricity::new(0.0549)?,
///     Orientation::new(Radians::new(0.09), Radians::ZERO, Radians::ZERO)?,
///     Radians::ZERO,
/// )?;
/// let orbit = DriftingOrbit::fixed(elements);
/// let t = UniverseTime::from_julian_years(100).expect("in range");
/// assert_eq!(orbit.relative_state_at(t), elements.relative_state_at(t));
/// assert!(orbit.drift().is_none());
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriftingOrbit {
    elements: KeplerElements,
    drift: Option<OrbitDrift>,
}

impl DriftingOrbit {
    /// An orbit whose elements hold: propagated as they are.
    #[must_use]
    pub const fn fixed(elements: KeplerElements) -> Self {
        Self {
            elements,
            drift: None,
        }
    }

    /// Elements `elements`, changing as `drift` says from its reference time, or holding for
    /// `None`.
    #[must_use]
    pub const fn new(elements: KeplerElements, drift: Option<OrbitDrift>) -> Self {
        Self { elements, drift }
    }

    /// The elements: at the drift's reference time for an evolving orbit.
    #[must_use]
    pub const fn elements(&self) -> &KeplerElements {
        &self.elements
    }

    /// The rates, for an evolving orbit.
    #[must_use]
    pub const fn drift(&self) -> Option<&OrbitDrift> {
        self.drift.as_ref()
    }

    /// The position and velocity relative to the primary at `t`, m and m s⁻¹: the elements'
    /// Kepler state, or for an evolving orbit the model of its cell
    /// ([`KeplerElements::drifting_state_at`]).
    #[must_use]
    pub fn relative_state_at(&self, t: UniverseTime) -> (SystemVector, SystemVelocity) {
        match &self.drift {
            None => self.elements.relative_state_at(t),
            Some(drift) => self
                .elements
                .drifting_state_at(t, drift.reference, drift.rates()),
        }
    }
}

/// A law that changes an orbit's elements with time, with the anchor its phase is counted from
/// (P14.T45.a).
pub(crate) trait EvolvingLaw {
    /// The elements at the anchor, whose own phase is the orbit's there.
    fn anchored(&self) -> &KeplerElements;

    /// The axis, eccentricity, gravitational parameter and orientation at `t`; their mean anomaly
    /// at the epoch is not read.
    fn shape_at(&self, t: UniverseTime) -> KeplerElements;

    /// ∫ (n(t') − n(anchor)) dt' from the anchor to `t`, rad.
    fn phase_excess(&self, t: UniverseTime) -> f64;

    /// ∫ (n(t') − `n_from_rad_per_s`) dt' from `from` to `to`, rad, where `n_from_rad_per_s` is the mean motion at
    /// `from`: the shape of the phase inside one cell, computed there so that the cell's check
    /// does not difference two integrals from a distant anchor.
    fn local_excess(&self, from: UniverseTime, to: UniverseTime, n_from_rad_per_s: f64) -> f64;

    /// The law's last break point at or before `t` and its first after it, if any: the times at
    /// which its mean motion may change its slope, where [`drift_cell`] cuts a cell as at a
    /// segment's ends (P14.T45.d). A law smooth throughout, as the receding moon's, has none.
    fn breaks_around(&self, t: UniverseTime) -> (Option<UniverseTime>, Option<UniverseTime>) {
        let _ = t;
        (None, None)
    }
}

/// The mean motion 2π ÷ P of `orbit`, rad s⁻¹.
#[must_use]
fn mean_motion(orbit: &KeplerElements) -> f64 {
    TAU / orbit.period().value()
}

/// ∫ (n(t') − `n_from_rad_per_s`) dt' from `from` to `to`, rad, for a law whose mean motion at a
/// time is `mean_motion_at` and is smooth between `breaks` (ascending): the 16-point
/// Gauss–Legendre rule on each interval between the breaks strictly inside the span, its nodes in
/// their table's order, the intervals summed in ascending time order. What circularisation and
/// mass loss use (P14.T45.d).
///
/// The panels depend on the span and the breaks alone, so the integral is one fixed function of
/// its two ends. A span that runs back in time gives the negated integral of the same span
/// forwards, so the two directions agree to the bit.
#[must_use]
pub(crate) fn composite_excess(
    from: UniverseTime,
    to: UniverseTime,
    n_from_rad_per_s: f64,
    breaks: &[UniverseTime],
    mean_motion_at: impl Fn(UniverseTime) -> f64,
) -> f64 {
    composite_excess_by(
        (&GL16_NODES, &GL16_WEIGHTS),
        from,
        to,
        n_from_rad_per_s,
        breaks,
        mean_motion_at,
    )
}

/// [`composite_excess`] by the Gauss–Legendre rule of `rule`'s nodes and weights on [−1, 1]: the
/// tests compare the 16-point rule with the 32-point one on the same intervals.
#[must_use]
pub(crate) fn composite_excess_by(
    rule: (&[f64], &[f64]),
    from: UniverseTime,
    to: UniverseTime,
    n_from_rad_per_s: f64,
    breaks: &[UniverseTime],
    mean_motion_at: impl Fn(UniverseTime) -> f64,
) -> f64 {
    let (low, high, sign) = match from.cmp(&to) {
        Ordering::Equal => return 0.0,
        Ordering::Less => (from, to, 1.0),
        Ordering::Greater => (to, from, -1.0),
    };
    let first = breaks.partition_point(|&at| at <= low);
    let last = breaks.partition_point(|&at| at < high).max(first);
    let mut sum = 0.0;
    let mut left = low;
    for &right in breaks[first..last].iter().chain(std::iter::once(&high)) {
        sum += panel_excess(rule, left, right, n_from_rad_per_s, &mean_motion_at);
        left = right;
    }
    sign * sum
}

/// ∫ (n(t') − `n_from_rad_per_s`) dt' from `from` to the later `to` by the Gauss–Legendre rule
/// `rule` on the one panel between them, its nodes in their table's order.
#[must_use]
fn panel_excess(
    (nodes, weights): (&[f64], &[f64]),
    from: UniverseTime,
    to: UniverseTime,
    n_from_rad_per_s: f64,
    mean_motion_at: &impl Fn(UniverseTime) -> f64,
) -> f64 {
    let half = 0.5 * seconds_between(from, to);
    let mut sum = 0.0;
    for (node, weight) in nodes.iter().zip(weights) {
        let at = Span::from_seconds_f64(half * (1.0 + node))
            .and_then(|offset| from.checked_add(offset))
            .expect("a node between two times of the clock is a time of the clock");
        sum += weight * (mean_motion_at(at) - n_from_rad_per_s);
    }
    half * sum
}

/// The fitted model on one cell, with how far it strays from the law at the quarter points, m.
struct Fit {
    orbit: DriftingOrbit,
    worst: f64,
    tolerance: f64,
}

/// The time `fraction` ÷ 4 of the way from `from` to `to`.
#[must_use]
fn quarter(from: UniverseTime, to: UniverseTime, fraction: i128) -> UniverseTime {
    let nanos_per_second = i128::from(NANOS_PER_SECOND);
    let total = (i128::from(to.seconds()) - i128::from(from.seconds())) * nanos_per_second
        + i128::from(to.subsec_nanos())
        - i128::from(from.subsec_nanos());
    let offset = total * fraction / 4;
    let seconds = i64::try_from(offset.div_euclid(nanos_per_second))
        .expect("a part of a cell is a span of the clock");
    let nanos = u32::try_from(offset.rem_euclid(nanos_per_second))
        .expect("a remainder of a second's nanoseconds is under 10⁹");
    from.checked_add(Span::new(seconds, nanos).expect("normalised nanoseconds"))
        .expect("a time inside a cell is a time of the clock")
}

/// The model of `law` on the cell from `start` to `end`, fitted as the [module](self) says.
#[must_use]
fn fit(law: &impl EvolvingLaw, start: UniverseTime, end: UniverseTime) -> Fit {
    let anchored = law.anchored();
    let shape = law.shape_at(start);
    let excess = law.phase_excess(start);
    let phase = anchored.unreduced_mean_anomaly_at(start) + excess;
    let elements = shape
        .rephased(start, phase)
        .expect("a law's phase is a finite angle");
    let tolerance = DRIFT_TOLERANCE.value() + f64::EPSILON * elements.apoapsis().value();
    let length = seconds_between(start, end);
    if length <= 0.0 {
        return Fit {
            orbit: DriftingOrbit {
                elements,
                drift: Some(OrbitDrift {
                    reference: start,
                    semi_major_axis_rate_m_per_s: 0.0,
                    eccentricity_rate_per_s: 0.0,
                    mean_motion_rate_rad_per_s2: 0.0,
                }),
            },
            worst: 0.0,
            tolerance,
        };
    }
    let (n_anchor_rad_per_s, n_start) = (mean_motion(anchored), mean_motion(&elements));
    let (a0, e0) = (
        shape.semi_major_axis().value(),
        shape.eccentricity().value(),
    );
    let end_shape = law.shape_at(end);
    let a_rate = (end_shape.semi_major_axis().value() - a0) / length;
    let e_rate = (end_shape.eccentricity().value() - e0) / length;
    // The law's phase gained over the cell, less what the start's mean motion gains: the anchored
    // phase gains n_anchor_rad_per_s exactly, so only small quantities are differenced. The model meets the
    // law's phase from the anchor at both ends, so that adjacent cells join.
    let surplus = (n_anchor_rad_per_s - n_start) * length + (law.phase_excess(end) - excess);
    let n_rate = 2.0 * surplus / (length * length);
    assert!(
        a_rate.is_finite() && e_rate.is_finite() && n_rate.is_finite(),
        "a drift cell's rates are finite"
    );
    // Inside the cell the law's phase is taken as the cell's own integral. What it falls short of
    // the integral from the anchor by is rounding alone, about 10⁻¹⁶ of the phase gained since
    // the anchor: the receding moon's integral is in closed form, and the composite rule of
    // circularisation and mass loss is exact to rounding between the break points the cell lies
    // between (P14.T45.d). It belongs to the law's evaluation and not to the model: it is shared
    // out as the model shares it, quadratically, so that the check measures the model's
    // curvature alone.
    let shortfall = surplus - law.local_excess(start, end, n_start);
    let mut worst: f64 = 0.0;
    for fraction in 1..=3 {
        let at = quarter(start, end, fraction);
        let dt = seconds_between(start, at);
        let law_shape = law.shape_at(at);
        let (a, e) = (
            law_shape.semi_major_axis().value(),
            law_shape.eccentricity().value(),
        );
        let da = (a - a0) - a_rate * dt;
        let de = (e - e0) - e_rate * dt;
        let part = dt / length;
        let dm =
            law.local_excess(start, at, n_start) + shortfall * part * part - 0.5 * n_rate * dt * dt;
        let along = a * ((1.0 + e) / (1.0 - e)).sqrt();
        let across = a * (1.0 + 1.0 / ((1.0 - e) * (1.0 + e)).sqrt());
        let error = da.abs() * (1.0 + e) + across * de.abs() + along * dm.abs();
        // `max` would pass over a NaN and accept the cell.
        assert!(error.is_finite(), "a drift cell's error is finite");
        worst = worst.max(error);
    }
    Fit {
        orbit: DriftingOrbit {
            elements,
            drift: Some(OrbitDrift {
                reference: start,
                semi_major_axis_rate_m_per_s: a_rate,
                eccentricity_rate_per_s: e_rate,
                mean_motion_rate_rad_per_s2: n_rate,
            }),
        },
        worst,
        tolerance,
    }
}

/// The drift cell holding `t` of an evolving orbit, and its model (P14.T45.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DriftCell {
    /// The model on the cell.
    pub(crate) orbit: DriftingOrbit,
    /// The end of the aligned cell, cut at the law's next break point and, for a time before
    /// `START`, at `START`: the time the record's elements hold until, unless the body's state or
    /// segment changes first.
    pub(crate) end: UniverseTime,
    /// The cell's size, log₂ s.
    pub(crate) log2: u32,
}

impl DriftCell {
    /// The time the record at `t` holds until, if any: the cell's end, cut at the law's next break
    /// point (P14.T45.d), or `next`, the body's next change of state or segment, if earlier.
    /// Inside the clock window it is stated only where it falls in the window, as every record's
    /// `valid_until` is; before `START` it is always stated, since a cell there ends at `START` or
    /// earlier and its model holds no further (P14.T45.a).
    #[must_use]
    pub(crate) fn holds_until(
        &self,
        t: UniverseTime,
        next: Option<UniverseTime>,
    ) -> Option<UniverseTime> {
        let until = next.map_or(self.end, |next| next.min(self.end));
        (t < ClockWindow::START || ClockWindow::contains(until)).then_some(until)
    }
}

/// The start of the aligned cell of 2^`log2` s holding `t`.
#[must_use]
fn aligned_start(t: UniverseTime, log2: u32) -> i64 {
    let size = 1_i64 << log2;
    t.seconds().div_euclid(size) * size
}

/// The cell of `law` holding `t`, whose segment runs from `from` to `until` (the next change of
/// the body's state or segment, if any): the largest aligned cell from 2^[`DRIFT_CELL_MAX_LOG2`]
/// s down to 2^[`DRIFT_CELL_MIN_LOG2`] s on which the model meets [`DRIFT_TOLERANCE`], or the
/// smallest.
///
/// The clock window's ends cut a cell on either side: a time inside the window is held to a cell
/// cut at them, and a time outside to one that ends at `START` or starts at `END`, so that every
/// time of one cut cell gets the same record. The law's break points around `t` cut it as
/// `from` and `until` do (P14.T45.d): every time between two breaks sees the same two, so the
/// cells still nest.
#[must_use]
pub(crate) fn drift_cell(
    law: &impl EvolvingLaw,
    t: UniverseTime,
    from: UniverseTime,
    until: Option<UniverseTime>,
) -> DriftCell {
    let (before, after) = (t < ClockWindow::START, t > ClockWindow::END);
    let (break_before, break_after) = law.breaks_around(t);
    let mut log2 = DRIFT_CELL_MAX_LOG2;
    loop {
        let size = 1_i64 << log2;
        let first = aligned_start(t, log2);
        let cell_start = UniverseTime::new(first, 0).expect("an aligned second is a time");
        let cell_end = first
            .checked_add(size)
            .and_then(|end| UniverseTime::new(end, 0).ok())
            .unwrap_or(UniverseTime::new(i64::MAX, 0).expect("the clock's last second"));
        let cell_end = if before {
            cell_end.min(ClockWindow::START)
        } else {
            cell_end
        };
        let cell_end = break_after.map_or(cell_end, |cut| cell_end.min(cut));
        let mut start = cell_start.max(from);
        if let Some(cut) = break_before {
            start = start.max(cut);
        }
        if !before {
            start = start.max(ClockWindow::START);
        }
        if after {
            start = start.max(ClockWindow::END);
        }
        let mut end = until.map_or(cell_end, |until| cell_end.min(until));
        if !after {
            end = end.min(ClockWindow::END);
        }
        let fitted = fit(law, start, end);
        if fitted.worst <= fitted.tolerance || log2 == DRIFT_CELL_MIN_LOG2 {
            return DriftCell {
                orbit: fitted.orbit,
                end: cell_end,
                log2,
            };
        }
        log2 -= 1;
    }
}

/// φ(x) − 1 for φ(x) = ((1 + x)^(10/13) − 1) ÷ ((10/13) x), the ratio of a receding moon's phase
/// to its anchor's mean motion times the time (P14.T45.a).
///
/// Below |x| = 0.1, where the difference would cancel, by twenty terms of the binomial series,
/// (c − 1) ÷ 2 x + (c − 1)(c − 2) ÷ 6 x² + …, with c = 10/13, whose last is under 10⁻²⁰ of
/// the first; above it in closed form.
#[must_use]
pub(crate) fn recession_phase_ratio_less_one(x: f64) -> f64 {
    const C: f64 = 10.0 / 13.0;
    if x.abs() < 0.1 {
        // The k-th binomial coefficient of c over c, times x^(k − 1), from k = 2.
        let mut term = (C - 1.0) / 2.0 * x;
        let mut sum = term;
        for k in 2_u32..21 {
            let k = f64::from(k);
            term *= (C - k) / (k + 1.0) * x;
            sum += term;
        }
        sum
    } else {
        (math::exp_m1(C * math::ln_1p(x)) - C * x) / (C * x)
    }
}

#[cfg(test)]
mod tests;
