use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::orbit::{Eccentricity, Orientation};
use crate::units::{GravitationalParameter, Radians};

/// A synthetic evolving orbit: an axis growing linearly at `rate` m s⁻¹ from `axis` m at the
/// epoch, an eccentricity falling linearly at `e_rate` s⁻¹ from `e`, about the Earth's μ, with its
/// phase excess by [`gauss_legendre_excess`].
struct Linear {
    anchored: KeplerElements,
    rate: f64,
    e_rate: f64,
}

impl Linear {
    fn new(axis: f64, rate: f64, e: f64, e_rate: f64) -> Self {
        let anchored = KeplerElements::from_semi_major_axis(
            Metres::new(axis),
            GravitationalParameter::new(3.986_004e14),
            Eccentricity::new(e).unwrap(),
            Orientation::new(Radians::new(0.3), Radians::new(1.1), Radians::new(2.0)).unwrap(),
            Radians::new(0.7),
        )
        .unwrap();
        Self {
            anchored,
            rate,
            e_rate,
        }
    }

    fn mean_motion_at(&self, t: UniverseTime) -> f64 {
        TAU / self.shape_at(t).period().value()
    }
}

impl EvolvingLaw for Linear {
    fn anchored(&self) -> &KeplerElements {
        &self.anchored
    }

    fn shape_at(&self, t: UniverseTime) -> KeplerElements {
        let dt = seconds_between(UniverseTime::EPOCH, t);
        KeplerElements::from_semi_major_axis(
            Metres::new(self.anchored.semi_major_axis().value() + self.rate * dt),
            self.anchored.gravitational_parameter(),
            Eccentricity::new(self.anchored.eccentricity().value() + self.e_rate * dt).unwrap(),
            *self.anchored.orientation(),
            self.anchored.mean_anomaly_at_epoch(),
        )
        .unwrap()
    }

    fn phase_excess(&self, t: UniverseTime) -> f64 {
        let n = TAU / self.anchored.period().value();
        self.local_excess(UniverseTime::EPOCH, t, n)
    }

    fn local_excess(&self, from: UniverseTime, to: UniverseTime, n_from_rad_per_s: f64) -> f64 {
        // ∫ n = n_from_rad_per_s Δt ψ(y) with y = ȧ Δt ÷ a_from and ψ(y) = (2 ÷ y)(1 − (1 + y)^(−1/2)):
        // ψ − 1 = −2 Σ C(−1/2, k) y^(k − 1) from k = 2, summed to 40 terms.
        let dt = seconds_between(from, to);
        let y = self.rate * dt / self.shape_at(from).semi_major_axis().value();
        let mut coefficient = -0.5;
        let mut power = 1.0;
        let mut sum = 0.0;
        for k in 1..40 {
            let k = f64::from(k);
            coefficient *= (-0.5 - k) / (k + 1.0);
            sum += -2.0 * coefficient * power * y;
            power *= y;
        }
        n_from_rad_per_s * dt * sum
    }
}

/// The law's position at `t`: its shape then, at its phase, the anchored phase plus the excess.
fn law_position(law: &impl EvolvingLaw, t: UniverseTime) -> [f64; 3] {
    let phase = law.anchored().unreduced_mean_anomaly_at(t) + law.phase_excess(t);
    law.shape_at(t)
        .rephased(t, phase)
        .unwrap()
        .relative_state_at(t)
        .0
        .metres()
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

fn years(y: f64) -> UniverseTime {
    UniverseTime::EPOCH
        .checked_add(Span::from_seconds_f64(y * 31_557_600.0).unwrap())
        .unwrap()
}

/// A moon like `close_binary`'s `.0201`, receding 8 cm a year, and a young one at 13 m a year.
fn moons() -> [Linear; 2] {
    let year = 31_557_600.0;
    [
        Linear::new(1.88e8, 0.0818 / year, 0.0, 0.0),
        Linear::new(1.0e8, 13.0 / year, 0.05, -1e-9 / year),
    ]
}

/// The tolerance a fit holds `law` to near `t`.
fn tolerance(law: &impl EvolvingLaw, t: UniverseTime) -> f64 {
    DRIFT_TOLERANCE.value() + f64::EPSILON * law.shape_at(t).apoapsis().value()
}

#[test]
fn a_cell_s_model_meets_the_law_at_both_ends() {
    for law in moons() {
        for y in [-990.0, -3.3, 0.0, 0.5, 412.7, 990.0] {
            let t = years(y);
            let cell = drift_cell(&law, t, ClockWindow::START, None);
            let reference = cell.orbit.drift().unwrap().reference();
            let at_start = cell.orbit.relative_state_at(reference).0.metres();
            assert!(distance(at_start, law_position(&law, reference)) < tolerance(&law, t));
            let last = cell.end.checked_sub(Span::new(0, 1).unwrap()).unwrap();
            let at_end = cell.orbit.relative_state_at(last).0.metres();
            assert!(distance(at_end, law_position(&law, last)) < tolerance(&law, t));
        }
    }
}

#[test]
fn adjacent_cells_join_within_the_tolerance() {
    for law in moons() {
        let mut t = years(-20.0);
        while t < years(20.0) {
            let cell = drift_cell(&law, t, ClockWindow::START, None);
            let next = drift_cell(&law, cell.end, ClockWindow::START, None);
            let last = cell.end.checked_sub(Span::new(0, 1).unwrap()).unwrap();
            let before = cell.orbit.relative_state_at(last).0.metres();
            let after = next.orbit.relative_state_at(last).0.metres();
            assert!(
                distance(before, after) < tolerance(&law, t),
                "{} m at {}",
                distance(before, after),
                cell.end
            );
            t = cell.end;
        }
    }
}

#[test]
fn every_time_in_one_cell_gives_the_same_model() {
    for law in moons() {
        for y in [-500.0, 0.25, 700.0] {
            let cell = drift_cell(&law, years(y), ClockWindow::START, None);
            let reference = cell.orbit.drift().unwrap().reference();
            let last = cell.end.checked_sub(Span::new(0, 1).unwrap()).unwrap();
            for t in [reference, quarter(reference, cell.end, 2), last] {
                assert_eq!(drift_cell(&law, t, ClockWindow::START, None), cell);
            }
        }
    }
}

#[test]
fn the_model_follows_a_fine_integration_of_the_law_across_the_window() {
    // The phase by the trapezoidal rule on 1,000 steps a cell, against the model at the cell's
    // own times: the law's integral and the model's agree to the tolerance.
    for law in moons() {
        for y in [-999.0, -400.0, 0.0, 300.0, 998.0] {
            let cell = drift_cell(&law, years(y), ClockWindow::START, None);
            let drift = *cell.orbit.drift().unwrap();
            let reference = drift.reference();
            let n0 = law.mean_motion_at(reference);
            let length = seconds_between(reference, cell.end);
            let steps = 1_000;
            let h = length / f64::from(steps);
            let at = |k: i32| {
                reference
                    .checked_add(Span::from_seconds_f64(h * f64::from(k)).unwrap())
                    .unwrap()
            };
            let mut gained = 0.0;
            for k in 0..steps {
                let left = law.mean_motion_at(at(k)) - n0;
                let right = law.mean_motion_at(at(k + 1)) - n0;
                gained += 0.5 * h * (left + right);
                if (k + 1) % 250 == 0 {
                    let dt = h * f64::from(k + 1);
                    let model = 0.5 * drift.mean_motion_rate_rad_per_s2() * dt * dt;
                    let along = law.shape_at(at(k + 1)).semi_major_axis().value();
                    assert!(
                        ((gained - model) * along).abs() < tolerance(&law, reference),
                        "{} m at {dt} s",
                        (gained - model) * along
                    );
                }
            }
        }
    }
}

#[test]
fn a_slowly_evolving_orbit_takes_the_largest_cell_and_a_fast_one_halves_it() {
    let [slow, _] = moons();
    assert_eq!(
        drift_cell(&slow, years(990.0), ClockWindow::START, None).log2,
        DRIFT_CELL_MAX_LOG2
    );
    // A planet's axis shrinking ten metres a second at 1 au cannot hold a year to 0.1 mm.
    let fast = Linear::new(1.5e11, -10.0, 0.1, 0.0);
    let cell = drift_cell(&fast, years(1.0), ClockWindow::START, None);
    assert!(cell.log2 < DRIFT_CELL_MAX_LOG2, "{}", cell.log2);
}

#[test]
fn an_orbit_too_fast_for_any_cell_falls_to_the_floor_and_its_error_there_is_pinned() {
    // A 10,000 km orbit whose axis grows 10 m a second: no cell holds it to 0.1 mm, and the
    // smallest is returned with the error it has, which this pins (P14.T45.a).
    let law = Linear::new(1.0e7, 10.0, 0.0, 0.0);
    let t = UniverseTime::new(100_000, 0).unwrap();
    let cell = drift_cell(&law, t, ClockWindow::START, None);
    assert_eq!(cell.log2, DRIFT_CELL_MIN_LOG2);
    let reference = cell.orbit.drift().unwrap().reference();
    let worst = (1..=3)
        .map(|k| {
            let at = quarter(reference, cell.end, k);
            distance(
                cell.orbit.relative_state_at(at).0.metres(),
                law_position(&law, at),
            )
        })
        .fold(0.0, f64::max);
    assert!(worst > tolerance(&law, t), "{worst} m");
    assert!(
        (worst - FLOOR_ERROR_M).abs() <= 1e-9 * FLOOR_ERROR_M,
        "{worst} m"
    );
}

/// The floor test's largest displacement at the quarter points, m, as measured (2026-10-02): the
/// model is far outside the tolerance there, and the floor bounds the re-send rate, not the error.
const FLOOR_ERROR_M: f64 = 127_186.401_026_552_25;

#[test]
fn cells_are_cut_at_the_segment_and_the_window() {
    let [law, _] = moons();
    let from = years(0.3);
    let until = years(0.6);
    let cell = drift_cell(&law, years(0.4), from, Some(until));
    assert_eq!(cell.orbit.drift().unwrap().reference(), from);
    assert!(cell.end >= until);
    let edge = drift_cell(&law, ClockWindow::END, ClockWindow::START, None);
    assert!(edge.orbit.drift().unwrap().reference() <= ClockWindow::END);
}

#[test]
fn a_fixed_orbit_is_propagated_as_its_elements() {
    let [law, _] = moons();
    let elements = *law.anchored();
    let orbit = DriftingOrbit::fixed(elements);
    let t = years(12.5);
    for (x, y) in orbit
        .relative_state_at(t)
        .0
        .metres()
        .into_iter()
        .zip(elements.relative_state_at(t).0.metres())
    {
        assert_same_bits(x, y);
    }
}

#[test]
fn the_recession_phase_ratio_is_continuous_and_matches_its_closed_form() {
    // The binomial series to 80 terms, which converges for |x| < 1.
    let series = |x: f64| {
        let c = 10.0 / 13.0;
        let mut term = (c - 1.0) / 2.0 * x;
        let mut sum = term;
        for k in 2..80 {
            let k = f64::from(k);
            term *= (c - k) / (k + 1.0) * x;
            sum += term;
        }
        sum
    };
    for x in [1e-2, 0.05, 0.099, 0.3, -0.3] {
        let got = recession_phase_ratio_less_one(x);
        assert!(
            (got - series(x)).abs() <= 1e-14 * series(x).abs(),
            "{x}: {got}"
        );
    }
    let closed = (crate::math::powf(4.0, 10.0 / 13.0) - 1.0) / (10.0 / 13.0 * 3.0) - 1.0;
    assert!((recession_phase_ratio_less_one(3.0) - closed).abs() <= 1e-14);
    // Either side of the series' edge, and its first term for a tiny x.
    let below = recession_phase_ratio_less_one(0.1 - 1e-15);
    let above = recession_phase_ratio_less_one(0.1);
    assert!((below - above).abs() < 1e-13);
    let tiny = 1e-9;
    assert!(
        (recession_phase_ratio_less_one(tiny) - (10.0 / 13.0 - 1.0) / 2.0 * tiny).abs() < 1e-19
    );
    assert_same_bits(recession_phase_ratio_less_one(0.0), 0.0);
}
