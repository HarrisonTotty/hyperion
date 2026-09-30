//! Hand-written optimisers (plan 15, Design note 3): a library's iteration order could change
//! between releases and move a table in a way the lock file cannot see.
//!
//! Each is deterministic: fixed iteration counts, no randomness, ties broken by index.

/// The minimum of `f` on `[lo, hi]` by golden-section search over `iterations` steps: the midpoint
/// of the last bracket. `f` should be unimodal on the interval.
#[must_use]
pub fn golden_section(mut f: impl FnMut(f64) -> f64, lo: f64, hi: f64, iterations: u32) -> f64 {
    let g = 0.5 * (5.0_f64.sqrt() - 1.0);
    let (mut a, mut b) = (lo, hi);
    let mut c = b - g * (b - a);
    let mut d = a + g * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..iterations {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - g * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + g * (b - a);
            fd = f(d);
        }
    }
    0.5 * (a + b)
}

/// The minimum of `f` from `start` by Nelder and Mead's (1965) simplex, the initial simplex
/// `start` plus `step` along each axis in turn, with the standard coefficients (reflection 1,
/// expansion 2, contraction ½, shrink ½), over `iterations` iterations: the best vertex and its
/// value.
///
/// # Panics
///
/// If `start` and `step` differ in length or are empty.
#[must_use]
pub fn nelder_mead(
    mut f: impl FnMut(&[f64]) -> f64,
    start: &[f64],
    step: &[f64],
    iterations: u32,
) -> (Vec<f64>, f64) {
    let n = start.len();
    assert!(
        n > 0 && step.len() == n,
        "a simplex needs matching start and steps"
    );
    let mut simplex: Vec<Vec<f64>> = vec![start.to_vec()];
    for i in 0..n {
        let mut v = start.to_vec();
        v[i] += step[i];
        simplex.push(v);
    }
    let mut values: Vec<f64> = simplex.iter().map(|v| f(v)).collect();
    let along = |from: &[f64], to: &[f64], t: f64| -> Vec<f64> {
        from.iter()
            .zip(to)
            .map(|(&a, &b)| a + t * (b - a))
            .collect()
    };
    for _ in 0..iterations {
        // Order the vertices by value, ties by their index, so that the order is reproducible.
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&i, &j| values[i].total_cmp(&values[j]).then(i.cmp(&j)));
        simplex = order.iter().map(|&i| simplex[i].clone()).collect();
        values = order.iter().map(|&i| values[i]).collect();
        let centroid: Vec<f64> = (0..n)
            .map(|k| {
                simplex[..n].iter().fold(0.0, |s, v| s + v[k])
                    / f64::from(u32::try_from(n).expect("a small dimension"))
            })
            .collect();
        let worst = simplex[n].clone();
        let reflected = along(&centroid, &worst, -1.0);
        let fr = f(&reflected);
        if fr < values[0] {
            let expanded = along(&centroid, &worst, -2.0);
            let fe = f(&expanded);
            if fe < fr {
                simplex[n] = expanded;
                values[n] = fe;
            } else {
                simplex[n] = reflected;
                values[n] = fr;
            }
        } else if fr < values[n - 1] {
            simplex[n] = reflected;
            values[n] = fr;
        } else {
            let (towards, f_towards) = if fr < values[n] {
                (reflected, fr)
            } else {
                (worst, values[n])
            };
            let contracted = along(&centroid, &towards, 0.5);
            let fc = f(&contracted);
            if fc < f_towards {
                simplex[n] = contracted;
                values[n] = fc;
            } else {
                let best = simplex[0].clone();
                for i in 1..=n {
                    simplex[i] = along(&best, &simplex[i], 0.5);
                    values[i] = f(&simplex[i]);
                }
            }
        }
    }
    let best = (0..=n)
        .min_by(|&i, &j| values[i].total_cmp(&values[j]).then(i.cmp(&j)))
        .expect("a simplex has vertices");
    (simplex[best].clone(), values[best])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_section_finds_a_parabola_s_minimum() {
        let x = golden_section(|x| (x - 0.3) * (x - 0.3), -2.0, 2.0, 80);
        assert!((x - 0.3).abs() < 1e-9, "{x}");
    }

    #[test]
    fn nelder_mead_finds_rosenbrock_s_minimum() {
        let rosenbrock = |v: &[f64]| {
            let (x, y) = (v[0], v[1]);
            (1.0 - x) * (1.0 - x) + 100.0 * (y - x * x) * (y - x * x)
        };
        let (best, value) = nelder_mead(rosenbrock, &[-1.2, 1.0], &[0.5, 0.5], 2_000);
        assert!(
            (best[0] - 1.0).abs() < 1e-6 && (best[1] - 1.0).abs() < 1e-6,
            "{best:?}"
        );
        assert!(value < 1e-12);
        assert_eq!(
            nelder_mead(rosenbrock, &[-1.2, 1.0], &[0.5, 0.5], 2_000),
            (best, value)
        );
    }
}
