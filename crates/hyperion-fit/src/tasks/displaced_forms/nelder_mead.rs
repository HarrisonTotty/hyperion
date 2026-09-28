//! Nelder and Mead's (1965) simplex minimiser over boxed parameters, for the form fits (plan 15,
//! P15.T6.c–f: "Nelder–Mead from a fixed start").
//!
//! The simplex moves in unbounded coordinates `t`; each parameter is its box's
//! `lo + (hi − lo)(1 + sin t) ÷ 2`, on a linear or a logarithmic scale ([`Bounded`]), so every
//! trial point is inside its box and a box edge is reachable, which is how P15.T6.c's "no
//! parameter on a box edge" is judged. The coefficients are the standard ones (reflection 1,
//! expansion 2, contraction ½, shrink ½; Lagarias et al. 1998, SIAM J. Optim. 9, 112), ties are
//! broken by vertex order, and every transcendental goes through `hyperion_sim::math`, so a fit is
//! bit-identical on every platform.

use hyperion_sim::math;

/// One parameter's box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounded {
    /// The least value.
    pub lo: f64,
    /// The greatest value.
    pub hi: f64,
    /// Whether the parameter moves on a logarithmic scale (both ends positive).
    pub log: bool,
}

impl Bounded {
    /// A box on a linear scale.
    #[must_use]
    pub const fn linear(lo: f64, hi: f64) -> Self {
        Self { lo, hi, log: false }
    }

    /// A box on a logarithmic scale; `lo` must be positive.
    #[must_use]
    ///
    /// # Panics
    ///
    /// If `lo` is not positive or `hi` not above it.
    pub const fn log(lo: f64, hi: f64) -> Self {
        assert!(lo > 0.0 && hi > lo, "a log box has positive ends in order");
        Self { lo, hi, log: true }
    }

    /// The ends on the scale the parameter moves on.
    fn ends(self) -> (f64, f64) {
        if self.log {
            (math::ln(self.lo), math::ln(self.hi))
        } else {
            (self.lo, self.hi)
        }
    }

    /// The value at the unbounded coordinate `t`.
    #[must_use]
    pub fn value(self, t: f64) -> f64 {
        let (low, high) = self.ends();
        let scaled = low + (high - low) * 0.5 * (1.0 + math::sin(t));
        let value = if self.log { math::exp(scaled) } else { scaled };
        value.clamp(self.lo, self.hi)
    }

    /// The unbounded coordinate of `value`, clamped into the box.
    #[must_use]
    pub fn coordinate(self, value: f64) -> f64 {
        let (a, b) = self.ends();
        let v = value.clamp(self.lo, self.hi);
        let s = if self.log { math::ln(v) } else { v };
        let unit = if b > a { (s - a) / (b - a) } else { 0.5 };
        math::asin((2.0 * unit - 1.0).clamp(-1.0, 1.0))
    }

    /// Whether `value` lies within `fraction` of the box's width (on its scale) of an end.
    #[must_use]
    pub fn at_edge(self, value: f64, fraction: f64) -> bool {
        let (a, b) = self.ends();
        let s = if self.log {
            math::ln(value.max(self.lo))
        } else {
            value
        };
        let width = b - a;
        s - a <= fraction * width || b - s <= fraction * width
    }
}

/// When the simplex stops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// The most evaluations of the objective.
    pub evaluations: usize,
    /// The spread of the simplex's values under which it has converged.
    pub tolerance: f64,
}

/// The minimum found.
#[derive(Debug, Clone, PartialEq)]
pub struct Minimum {
    /// The parameters, in their boxes.
    pub values: Vec<f64>,
    /// The objective there.
    pub objective: f64,
    /// Evaluations used.
    pub evaluations: usize,
}

/// Minimises `f` over the parameters boxed by `boxes`, from `start` (values, clamped into the
/// boxes), the first simplex stepping `step` in each unbounded coordinate, until `limits`. The
/// search is restarted once from the minimum it found, which undoes a collapsed simplex.
///
/// # Panics
///
/// If `boxes` and `start` differ in length or are empty.
pub fn minimise(
    mut f: impl FnMut(&[f64]) -> f64,
    boxes: &[Bounded],
    start: &[f64],
    step: f64,
    limits: Stop,
) -> Minimum {
    assert!(
        !boxes.is_empty() && boxes.len() == start.len(),
        "one start per box"
    );
    let mut values = vec![0.0; boxes.len()];
    let mut objective = |t: &[f64], values: &mut Vec<f64>| {
        for ((v, b), &ti) in values.iter_mut().zip(boxes).zip(t) {
            *v = b.value(ti);
        }
        f(values)
    };
    let mut t: Vec<f64> = boxes
        .iter()
        .zip(start)
        .map(|(b, &v)| b.coordinate(v))
        .collect();
    let mut used = 0;
    let mut best = f64::INFINITY;
    for pass in 0..2 {
        let budget = if pass == 0 {
            limits.evaluations - limits.evaluations / 4
        } else {
            limits.evaluations.saturating_sub(used)
        };
        let (point, value, n) = simplex(
            |x| objective(x, &mut values),
            &t,
            step,
            budget,
            limits.tolerance,
        );
        used += n;
        t = point;
        best = value;
    }
    Minimum {
        values: boxes.iter().zip(&t).map(|(b, &ti)| b.value(ti)).collect(),
        objective: best,
        evaluations: used,
    }
}

/// The simplex search itself, in unbounded coordinates: the best vertex, its value and the
/// evaluations used.
fn simplex(
    mut f: impl FnMut(&[f64]) -> f64,
    start: &[f64],
    step: f64,
    budget: usize,
    tolerance: f64,
) -> (Vec<f64>, f64, usize) {
    let n = start.len();
    let mut vertices: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    vertices.push(start.to_vec());
    for i in 0..n {
        let mut v = start.to_vec();
        v[i] += step;
        vertices.push(v);
    }
    let mut values: Vec<f64> = vertices.iter().map(|v| f(v)).collect();
    let mut used = n + 1;
    let mut order: Vec<usize> = (0..=n).collect();
    let mut centroid = vec![0.0; n];
    let mut trial = vec![0.0; n];
    let mut second = vec![0.0; n];
    while used < budget {
        order.sort_by(|&a, &b| values[a].total_cmp(&values[b]).then(a.cmp(&b)));
        let (best, worst, next) = (order[0], order[n], order[n - 1]);
        if values[worst] - values[best] <= tolerance {
            break;
        }
        centroid.fill(0.0);
        for &k in &order[..n] {
            for (c, x) in centroid.iter_mut().zip(&vertices[k]) {
                *c += x;
            }
        }
        #[expect(clippy::cast_precision_loss, reason = "a handful of parameters")]
        let inv = 1.0 / n as f64;
        for c in &mut centroid {
            *c *= inv;
        }
        let along = |t: &mut Vec<f64>, factor: f64, from: &[f64]| {
            for ((ti, c), w) in t.iter_mut().zip(&centroid).zip(from) {
                *ti = c + factor * (c - w);
            }
        };
        along(&mut trial, 1.0, &vertices[worst]);
        let reflected = f(&trial);
        used += 1;
        if reflected < values[best] {
            along(&mut second, 2.0, &vertices[worst]);
            let expanded = f(&second);
            used += 1;
            if expanded < reflected {
                vertices[worst].clone_from(&second);
                values[worst] = expanded;
            } else {
                vertices[worst].clone_from(&trial);
                values[worst] = reflected;
            }
            continue;
        }
        if reflected < values[next] {
            vertices[worst].clone_from(&trial);
            values[worst] = reflected;
            continue;
        }
        let outside = reflected < values[worst];
        along(
            &mut second,
            if outside { 0.5 } else { -0.5 },
            &vertices[worst],
        );
        let contracted = f(&second);
        used += 1;
        if contracted < reflected.min(values[worst]) {
            vertices[worst].clone_from(&second);
            values[worst] = contracted;
            continue;
        }
        let anchor = vertices[best].clone();
        for &k in &order[1..] {
            for (x, a) in vertices[k].iter_mut().zip(&anchor) {
                *x = a + 0.5 * (*x - a);
            }
            values[k] = f(&vertices[k]);
            used += 1;
        }
    }
    let best = (0..=n)
        .min_by(|&a, &b| values[a].total_cmp(&values[b]).then(a.cmp(&b)))
        .expect("a simplex has vertices");
    (vertices[best].clone(), values[best], used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_finds_a_boxed_quadratics_minimum_and_an_edge() {
        let boxes = [Bounded::linear(-5.0, 5.0), Bounded::log(0.01, 100.0)];
        let stop = Stop {
            evaluations: 2_000,
            tolerance: 1e-14,
        };
        let m = minimise(
            |p| math::powi(p[0] - 1.5, 2) + math::powi(math::ln(p[1]) - math::ln(3.0), 2),
            &boxes,
            &[0.0, 1.0],
            0.5,
            stop,
        );
        assert!((m.values[0] - 1.5).abs() < 1e-5, "{m:?}");
        assert!((m.values[1] - 3.0).abs() < 1e-4, "{m:?}");
        // A minimum outside the box lands on its edge.
        let m = minimise(
            |p| math::powi(p[0] - 9.0, 2),
            &boxes[..1],
            &[0.0],
            0.5,
            stop,
        );
        assert!(boxes[0].at_edge(m.values[0], 1e-6), "{m:?}");
        assert!(!boxes[0].at_edge(0.0, 1e-3));
    }

    #[test]
    fn the_transform_round_trips() {
        for b in [Bounded::linear(0.0, 1.0), Bounded::log(0.2, 12.0)] {
            for v in [b.lo, f64::midpoint(b.lo, b.hi), b.hi] {
                assert!(
                    (b.value(b.coordinate(v)) - v).abs() < 1e-12 * b.hi,
                    "{b:?} {v}"
                );
            }
        }
    }
}
