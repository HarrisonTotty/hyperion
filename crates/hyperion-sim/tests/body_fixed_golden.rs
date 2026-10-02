//! Body-fixed conversions (plan R02, R02.T3): ten conversions through [`BodyFixedRotation`]
//! pinned bit for bit in `golden/coords/body_fixed.golden`.
//!
//! The rotations are hand-set: the identity, a quarter turn about the pole, and a tilted pole at
//! several prime-meridian angles, built here from `hyperion_sim::math` so that their bits are the
//! same on every target. The TypeScript client's `view/coords/` reads this file to check its own
//! body-fixed conversion (R02.T6.a).

use hyperion_sim::coords::{BodyFixedPosition, BodyFixedRotation, BodyPosition};
use hyperion_sim::{GENERATOR_VERSION, math};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// A rotation about a pole at `tilt` radians from +z (towards −y), at prime-meridian angle `w`
/// radians: its columns are the body-fixed axes in the body frame.
fn tilted(tilt: f64, w: f64) -> BodyFixedRotation {
    let (st, ct) = math::sin_cos(tilt);
    let (sw, cw) = math::sin_cos(w);
    let pole = [0.0, -st, ct];
    let e1 = [1.0, 0.0, 0.0];
    // e2 = pole × e1.
    let e2 = [0.0, ct, st];
    let x = [
        e1[0] * cw + e2[0] * sw,
        e1[1] * cw + e2[1] * sw,
        e1[2] * cw + e2[2] * sw,
    ];
    let y = [
        e1[0] * -sw + e2[0] * cw,
        e1[1] * -sw + e2[1] * cw,
        e1[2] * -sw + e2[2] * cw,
    ];
    BodyFixedRotation::from_rows([
        [x[0], y[0], pole[0]],
        [x[1], y[1], pole[1]],
        [x[2], y[2], pole[2]],
    ])
    .expect("built from an orthonormal triple")
}

fn write_rotation(w: &mut GoldenWriter, r: &BodyFixedRotation) {
    for (i, row) in r.rows().iter().enumerate() {
        for (j, value) in row.iter().enumerate() {
            w.f64(&format!("r[{i}][{j}]"), *value);
        }
    }
}

fn write_components(w: &mut GoldenWriter, label: &str, metres: [f64; 3]) {
    for (axis, value) in ["x", "y", "z"].into_iter().zip(metres) {
        w.f64(&format!("{label}.{axis}"), value);
    }
}

#[test]
fn body_fixed_conversions_are_pinned() {
    let quarter =
        BodyFixedRotation::from_rows([[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]])
            .expect("a quarter turn is a rotation");
    // Earth's obliquity, 23.44° (IERS Conventions 2010, IAU 2006: ε₀ = 84,381.406″), and
    // Uranus's, 97.77° (NASA Uranus fact sheet); only the matrices' shapes matter here.
    let obliquity = 23.44_f64.to_radians();
    let century = 2.2e5_f64 % std::f64::consts::TAU;
    // (name, rotation, a body-fixed point to take to the body frame, a body-frame point to take
    // back)
    let cases: [(&str, BodyFixedRotation, [f64; 3], [f64; 3]); 10] = [
        (
            "identity",
            BodyFixedRotation::IDENTITY,
            [6.371e6, 0.0, 0.0],
            [0.0, 6.371e6, 1.0],
        ),
        ("quarter turn", quarter, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        (
            "quarter turn",
            quarter,
            [6.371e6, 1.5e3, -2.5e6],
            [-4.0e5, 6.0e6, 1.0e6],
        ),
        (
            "tilted w=0",
            tilted(obliquity, 0.0),
            [6.371e6, 0.0, 0.0],
            [6.371e6, 0.0, 0.0],
        ),
        (
            "tilted w=1",
            tilted(obliquity, 1.0),
            [3.0e6, 4.0e6, 3.5e6],
            [3.0e6, 4.0e6, 3.5e6],
        ),
        (
            "tilted w=century",
            tilted(obliquity, century),
            [6.371e6, 0.0, 0.0],
            [0.0, 0.0, 6.371e6],
        ),
        (
            "tilted w=century",
            tilted(obliquity, century),
            [-1.2e6, 5.9e6, 2.1e6],
            [4.4e6, -4.4e6, 1.0e3],
        ),
        (
            "pole down w=2.5",
            tilted(3.0, 2.5),
            [1.0e3, 2.0e3, 3.0e3],
            [7.0e7, -3.0e7, 1.1e7],
        ),
        (
            "Uranus-like w=4",
            tilted(97.77_f64.to_radians(), 4.0),
            [2.5559e7, 0.0, 0.0],
            [0.0, 0.0, 2.5559e7],
        ),
        (
            "small moon w=6",
            tilted(0.1, 6.0),
            [1.1e4, -2.2e4, 5.0e3],
            [1.5e-3, 2.5e-3, -3.5e-3],
        ),
    ];
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (n, (name, r, fixed, body)) in cases.into_iter().enumerate() {
        w.line("");
        w.line(&format!("case {n}: {name}"));
        write_rotation(&mut w, &r);
        write_components(&mut w, "fixed", fixed);
        write_components(
            &mut w,
            "to_body",
            r.to_body(&BodyFixedPosition::new(fixed)).metres(),
        );
        write_components(&mut w, "body", body);
        write_components(
            &mut w,
            "to_body_fixed",
            r.to_body_fixed(&BodyPosition::new(body)).metres(),
        );
    }
    golden!("coords/body_fixed", w.as_str());
}
