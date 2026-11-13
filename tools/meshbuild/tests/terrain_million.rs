//! The 10⁶-triangle terrain mesh (plan R14, T5.a): its DAG's checks, its bytes, and the
//! acceptance's density: at τ = 1 px, 10⁶ triangles over a sixteenth of 1080p (129,600 px) draw
//! at most 2 triangles a covered pixel, and at most 1.2 times a hand-picked uniform cut's.

mod common;

use common::{
    assert_cuts_watertight, assert_group_borders_kept, assert_monotone_and_nested, build_twice,
    clusters, open_points, sha256, summary, triangles,
};
use meshbuild::cut::{CutView, cut, in_uniform_cut};
use meshbuild::procedural::terrain;

/// 1,000 × 500 quads of 2 m: 10⁶ triangles over 2 × 1 km, with 80 m of relief.
const COLUMNS: u32 = 1000;
const ROWS: u32 = 500;
const SPACING_M: f64 = 2.0;

/// The view: 1920 px across 60°, the patch seen straight down so that it covers 129,600 px.
const WIDTH_PX: f64 = 1920.0;
const COVERED_PX: f64 = 129_600.0;

#[test]
fn a_million_triangle_terrain_draws_under_two_triangles_a_covered_pixel_at_one_pixel() {
    let source = terrain(COLUMNS, ROWS, SPACING_M, 80.0, 23);
    assert_eq!(source.triangles().len(), 1_000_000);
    let built = build_twice(&source);
    eprintln!("terrain 10⁶: {}", summary(&built));
    let file = &built.file;
    let open = open_points(file);
    assert_monotone_and_nested(file);
    common::assert_group_table(file);
    common::assert_root_pages(file);
    assert_group_borders_kept(file, &open);
    assert_cuts_watertight(file, &open, 12, 3);

    let (width_m, height_m) = (f64::from(COLUMNS) * SPACING_M, f64::from(ROWS) * SPACING_M);
    let fov = 60_f64.to_radians();
    let focal_px = WIDTH_PX / (2.0 * libm::tan(0.5 * fov));
    let distance_m = focal_px * (width_m * height_m / COVERED_PX).sqrt();
    let origin = file.pages()[0].origin_m();
    let mean_height_m: f64 = source.positions().iter().map(|p| p[2]).sum::<f64>()
        / f64::from(u32::try_from(source.positions().len()).unwrap());
    let eye_mesh = [0.5 * width_m, 0.5 * height_m, mean_height_m + distance_m];
    let eye = std::array::from_fn(|k| eye_mesh[k] - origin[k]);
    let view = CutView::new(eye, WIDTH_PX, fov);
    let drawn = cut(file, |c| view.draws(c, 1.0));
    common::assert_watertight(&drawn, &open, "the acceptance's view");
    let drawn_triangles = triangles(&drawn);

    let nearest_m = source
        .positions()
        .iter()
        .map(|p| {
            (0..3)
                .map(|k| (p[k] - eye_mesh[k]) * (p[k] - eye_mesh[k]))
                .sum::<f64>()
                .sqrt()
        })
        .fold(f64::INFINITY, f64::min);
    let uniform_error_m = nearest_m / view.focal_px();
    let uniform = cut(file, |c| in_uniform_cut(c, uniform_error_m));
    common::assert_watertight(&uniform, &open, "the uniform cut");
    let uniform_triangles = triangles(&uniform);
    let per_pixel = f64::from(u32::try_from(drawn_triangles).unwrap()) / COVERED_PX;
    let ratio = f64::from(u32::try_from(drawn_triangles).unwrap())
        / f64::from(u32::try_from(uniform_triangles).unwrap());
    eprintln!(
        "at τ = 1 px from {distance_m:.0} m: {drawn_triangles} triangles, {per_pixel:.3} a covered pixel; \
         the uniform cut at {uniform_error_m:.3} m: {uniform_triangles}, ratio {ratio:.3}; \
         of {} clusters",
        clusters(file).len()
    );
    assert!(per_pixel <= 2.0, "{per_pixel} triangles a covered pixel");
    assert!(ratio <= 1.2, "{ratio} times the uniform cut");
    assert_eq!(
        sha256(&built.bytes),
        "efc08f57060e016c385ba852b9c17239facc5b575e8a03be1f0ae27c9acad15f",
        "the 10⁶-triangle terrain's pages moved"
    );
}
