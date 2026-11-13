//! Vector arithmetic on grid coordinates (exact, in integers) and in `f64`.

pub(crate) type I3 = [i64; 3];
pub(crate) type F3 = [f64; 3];

#[must_use]
pub(crate) fn sub_i64(a: I3, b: I3) -> I3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The cross product, exact: grid differences are below 2²¹, so each term is below 2⁴³.
#[must_use]
pub(crate) fn cross_i64(a: I3, b: I3) -> I3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The dot product of two cross products, exact in 128 bits.
#[must_use]
pub(crate) fn dot_i128(a: I3, b: I3) -> i128 {
    (0..3).map(|i| i128::from(a[i]) * i128::from(b[i])).sum()
}

#[must_use]
pub(crate) fn to_f64(a: I3) -> F3 {
    a.map(crate::num::f64_of)
}

#[must_use]
pub(crate) fn sub(a: F3, b: F3) -> F3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[must_use]
pub(crate) fn add(a: F3, b: F3) -> F3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[must_use]
pub(crate) fn scale(a: F3, s: f64) -> F3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[must_use]
pub(crate) fn dot(a: F3, b: F3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[must_use]
pub(crate) fn cross(a: F3, b: F3) -> F3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[must_use]
pub(crate) fn length(a: F3) -> f64 {
    dot(a, a).sqrt()
}

/// The distance from `p` to the triangle `abc`: its closest point by the triangle's Voronoi
/// regions (Ericson 2005, "Real-Time Collision Detection", §5.1.5).
#[expect(
    clippy::many_single_char_names,
    reason = "Ericson's names, to read against the source"
)]
#[must_use]
pub(crate) fn point_triangle_distance(p: F3, a: F3, b: F3, c: F3) -> f64 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return length(ap);
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return length(bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return length(sub(p, add(a, scale(ab, v))));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return length(cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return length(sub(p, add(a, scale(ac, w))));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return length(sub(p, add(b, scale(sub(c, b), w))));
    }
    let denominator = 1.0 / (va + vb + vc);
    let v = vb * denominator;
    let w = vc * denominator;
    length(sub(p, add(a, add(scale(ab, v), scale(ac, w)))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_over_the_face_is_its_height_away() {
        let d = point_triangle_distance(
            [0.25, 0.25, 2.0],
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        );
        assert!((d - 2.0).abs() < 1e-12, "{d}");
    }

    #[test]
    fn a_point_beyond_an_edge_is_measured_to_the_edge() {
        let d =
            point_triangle_distance([0.5, -3.0, 4.0], [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((d - 5.0).abs() < 1e-12, "{d}");
    }

    #[test]
    fn a_point_beyond_a_corner_is_measured_to_the_corner() {
        let d =
            point_triangle_distance([4.0, -1.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((d - 10.0_f64.sqrt()).abs() < 1e-12, "{d}");
    }
}
