//! The procedural test meshes the builder is proved on (plan R14, T5.a): an icosphere with noise,
//! a torus knot and a terrain grid.
//!
//! They stand in for content no plan has made yet. Their noise is value noise on an integer
//! lattice hashed from the seed, interpolated by smoothstep, so it uses basic operations alone;
//! the torus knot's sines and cosines go through `libm`. So each mesh is the same bits on every
//! platform, and so are its pages.

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::geometry::{F3, add, cross, length, scale, sub};
use crate::mesh::SourceMesh;
use crate::num::{f64_of, f64_of_count, i64_of_integral, ix, u32_of};

/// A terrain patch: `columns` × `rows` quads of `spacing_m`, each split on its (0, 0)–(1, 1)
/// diagonal, its heights eight octaves of noise of `amplitude_m`, the first octave's wavelength
/// half the shorter side; UVs span [0, 1] over the patch, with no seam.
///
/// # Panics
///
/// If `columns` or `rows` is 0, or the grid's vertices overflow `u32`.
#[must_use]
pub fn terrain(columns: u32, rows: u32, spacing_m: f64, amplitude_m: f64, seed: u64) -> SourceMesh {
    assert!(columns > 0 && rows > 0, "a terrain needs a quad");
    let (width, height) = (columns + 1, rows + 1);
    let wavelength_m = f64::from(columns.min(rows)) * spacing_m * 0.5;
    let mut positions = Vec::with_capacity(ix(width) * ix(height));
    let mut uvs = Vec::with_capacity(positions.capacity());
    for j in 0..height {
        for i in 0..width {
            let (x, y) = (f64::from(i) * spacing_m, f64::from(j) * spacing_m);
            let h = amplitude_m * fbm(seed, [x / wavelength_m, y / wavelength_m, 0.0], 8);
            positions.push([x, y, h]);
            uvs.push([
                f64::from(i) / f64::from(columns),
                f64::from(j) / f64::from(rows),
            ]);
        }
    }
    let mut triangles = Vec::with_capacity(2 * ix(columns) * ix(rows));
    for j in 0..rows {
        for i in 0..columns {
            let v = |di: u32, dj: u32| (j + dj) * width + i + di;
            triangles.push([v(0, 0), v(1, 0), v(1, 1)]);
            triangles.push([v(0, 0), v(1, 1), v(0, 1)]);
        }
    }
    SourceMesh::new(positions, triangles)
        .and_then(|mesh| mesh.with_uvs(uvs))
        .expect("a grid with a UV a vertex is a valid mesh")
}

/// An icosphere: an icosahedron subdivided `subdivisions` times (20 × 4ⁿ triangles) of
/// `radius_m`, each vertex moved along its direction by `relief` × the radius × six octaves of
/// noise; no normals or UVs, so the builder computes the normals.
///
/// # Panics
///
/// If the subdivided mesh's vertices overflow `u32`.
#[must_use]
pub fn icosphere(subdivisions: u32, radius_m: f64, relief: f64, seed: u64) -> SourceMesh {
    let t = f64::midpoint(1.0, 5.0_f64.sqrt());
    let mut directions: Vec<F3> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .to_vec();
    let mut triangles: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..subdivisions {
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
        let mut midpoint = |a: u32, b: u32, directions: &mut Vec<F3>| {
            *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                directions.push(scale(add(directions[ix(a)], directions[ix(b)]), 0.5));
                u32_of(directions.len() - 1)
            })
        };
        let mut finer = Vec::with_capacity(4 * triangles.len());
        for &[a, b, c] in &triangles {
            let ab = midpoint(a, b, &mut directions);
            let bc = midpoint(b, c, &mut directions);
            let ca = midpoint(c, a, &mut directions);
            finer.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        triangles = finer;
    }
    let positions = directions
        .iter()
        .map(|&d| {
            let unit = scale(d, 1.0 / length(d));
            let lift = 1.0 + relief * fbm(seed, scale(unit, 4.0), 6);
            scale(unit, radius_m * lift)
        })
        .collect();
    SourceMesh::new(positions, triangles).expect("a subdivided icosahedron is a valid mesh")
}

/// A (p, q) torus knot: the curve (cos qφ + 2)(cos pφ, sin pφ) and −sin qφ along z, scaled by
/// `scale_m`, swept by a tube of `tube_m` with `segments` rings of `sides` vertices about its
/// Frenet frame; its normals are the tube's own, and its UVs run along and around it with a seam
/// where each wraps, so that the builder's seams are exercised.
///
/// # Panics
///
/// If `segments` or `sides` is below 3, or the mesh's vertices overflow `u32`.
#[must_use]
pub fn torus_knot(
    p: u32,
    q: u32,
    segments: u32,
    sides: u32,
    scale_m: f64,
    tube_m: f64,
) -> SourceMesh {
    assert!(
        segments >= 3 && sides >= 3,
        "a tube needs three rings of three sides"
    );
    let curve = |phi: f64| -> F3 {
        let r = libm::cos(f64::from(q) * phi) + 2.0;
        let (sp, cp) = (libm::sin(f64::from(p) * phi), libm::cos(f64::from(p) * phi));
        scale([r * cp, r * sp, -libm::sin(f64::from(q) * phi)], scale_m)
    };
    let step = 2.0 * PI / f64::from(segments);
    let mut ring_points = Vec::with_capacity(ix(segments) * ix(sides));
    let mut ring_normals = Vec::with_capacity(ring_points.capacity());
    for i in 0..segments {
        let phi = f64::from(i) * step;
        let (before, here, after) = (curve(phi - step), curve(phi), curve(phi + step));
        let tangent = unit(sub(after, before));
        let bend = sub(add(after, before), scale(here, 2.0));
        let binormal = unit(cross(tangent, bend));
        let normal = cross(binormal, tangent);
        for j in 0..sides {
            let theta = 2.0 * PI * f64::from(j) / f64::from(sides);
            let out = add(
                scale(normal, libm::cos(theta)),
                scale(binormal, libm::sin(theta)),
            );
            ring_points.push(add(here, scale(out, tube_m)));
            ring_normals.push(out);
        }
    }
    let (columns, rows) = (segments + 1, sides + 1);
    let mut positions = Vec::with_capacity(ix(columns) * ix(rows));
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(positions.capacity());
    for i in 0..columns {
        for j in 0..rows {
            let k = ix((i % segments) * sides + j % sides);
            positions.push(ring_points[k]);
            normals.push(ring_normals[k]);
            uvs.push([
                f64::from(i) / f64::from(segments),
                f64::from(j) / f64::from(sides),
            ]);
        }
    }
    let mut triangles = Vec::with_capacity(2 * ix(segments) * ix(sides));
    for i in 0..segments {
        for j in 0..sides {
            let v = |di: u32, dj: u32| (i + di) * rows + j + dj;
            triangles.push([v(0, 0), v(1, 1), v(1, 0)]);
            triangles.push([v(0, 0), v(0, 1), v(1, 1)]);
        }
    }
    SourceMesh::new(positions, triangles)
        .and_then(|mesh| mesh.with_normals(normals))
        .and_then(|mesh| mesh.with_uvs(uvs))
        .expect("a tube with a normal and a UV a vertex is a valid mesh")
}

fn unit(v: F3) -> F3 {
    scale(v, 1.0 / length(v))
}

/// `octaves` of value noise at `p`, each of twice the last's frequency and half its amplitude,
/// normalised to [−1, 1].
fn fbm(seed: u64, p: F3, octaves: u32) -> f64 {
    let (mut sum, mut weight, mut amplitude, mut frequency) = (0.0, 0.0, 1.0, 1.0);
    for octave in 0..octaves {
        sum += amplitude * value_noise(seed.wrapping_add(u64::from(octave)), scale(p, frequency));
        weight += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    sum / weight
}

/// Value noise in [−1, 1]: the lattice's hashed values, interpolated by smoothstep.
fn value_noise(seed: u64, p: F3) -> f64 {
    let cell = p.map(|x| i64_of_integral(x.floor()));
    let f: F3 = std::array::from_fn(|axis| p[axis] - f64_of(cell[axis]));
    let s = f.map(|t| t * t * (3.0 - 2.0 * t));
    let corner =
        |dx: i64, dy: i64, dz: i64| lattice(seed, [cell[0] + dx, cell[1] + dy, cell[2] + dz]);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), s[0]);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), s[0]);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), s[0]);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), s[0]);
    lerp(lerp(x00, x10, s[1]), lerp(x01, x11, s[1]), s[2])
}

/// A lattice point's value in [−1, 1), from `SplitMix64`'s finaliser (Steele, Lea and Flood 2014,
/// "Fast Splittable Pseudorandom Number Generators", OOPSLA) over the seed and its coordinates.
fn lattice(seed: u64, point: [i64; 3]) -> f64 {
    let mut h = seed;
    for c in point {
        h = mix(h ^ c.cast_unsigned());
    }
    f64_of_count(usize::try_from(h >> 11).expect("53 bits fit usize")) / 4_503_599_627_370_496.0
        - 1.0
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Six times the volume a closed mesh encloses, positive when its triangles face outward.
    fn six_volumes(mesh: &SourceMesh) -> f64 {
        let p = mesh.positions();
        mesh.triangles()
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|v| p[ix(v)]);
                crate::geometry::dot(a, cross(b, c))
            })
            .sum()
    }

    #[test]
    fn the_closed_meshes_face_outward() {
        assert!(six_volumes(&icosphere(2, 1.0, 0.05, 1)) > 0.0);
        assert!(six_volumes(&torus_knot(2, 3, 64, 8, 1.0, 0.3)) > 0.0);
    }

    #[test]
    fn the_meshes_have_their_counts() {
        assert_eq!(icosphere(3, 1.0, 0.0, 1).triangles().len(), 20 * 64);
        assert_eq!(
            torus_knot(2, 3, 64, 8, 1.0, 0.3).triangles().len(),
            2 * 64 * 8
        );
        assert_eq!(terrain(10, 4, 1.0, 1.0, 1).triangles().len(), 80);
    }

    #[test]
    fn noise_stays_in_its_range() {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for i in 0..1000 {
            let x = f64::from(i) * 0.137;
            let v = fbm(3, [x, -0.7 * x, 0.3], 8);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(-1.0 <= lo && hi <= 1.0 && hi - lo > 0.5, "{lo} {hi}");
    }
}
