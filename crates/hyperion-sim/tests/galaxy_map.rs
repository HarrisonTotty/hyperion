//! The galaxy map's column densities (plan 02, P02.T10): that a raster's rows are the same however
//! they are split into bands, that both views agree with a brute-force integral of the fields, that
//! the face-on map holds the galaxy's systems, that a disc thinner than a pixel keeps its light, and
//! that the values are pinned.

use hyperion_sim::coords::ROOT_HALF_WIDTH_LY;
use hyperion_sim::galaxy::fields::{Fields, MAX_COMPONENTS, Shape};
use hyperion_sim::galaxy::map::{
    BuildMapSpecError, MapSelection, MapSpec, MapView, column_density_edge_on,
    column_density_face_on, render_rows,
};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

/// The three pinned seeds of the galaxy golden files.
const PINNED: [u64; 3] = [
    0x0000_0000_0000_0001,
    0x5eed_0000_c0ff_ee00,
    0xdead_beef_cafe_f00d,
];

/// The root cube's half-width, ly: M1's maps span the cube.
const ROOT_HALF: f64 = 65_536.0;

/// Every view and selection a map can take.
const VIEWS: [(MapView, MapSelection); 4] = [
    (MapView::FaceOn, MapSelection::AllSystems),
    (MapView::FaceOn, MapSelection::YoungOnly),
    (MapView::EdgeOn, MapSelection::AllSystems),
    (MapView::EdgeOn, MapSelection::YoungOnly),
];

fn fixture() -> Fields {
    let params = GalaxyParams::milky_way_like();
    Fields::new(&params, &MassModel::new(&params))
}

/// A raster of the whole root cube, `size` pixels across.
fn whole_cube(view: MapView, selection: MapSelection, size: u32, height: u32) -> MapSpec {
    MapSpec::new(
        view,
        selection,
        [size, height],
        [0.0, 0.0],
        2.0 * ROOT_HALF / f64::from(size),
    )
    .expect("a raster of the root cube")
}

/// Asserts that two points of a picture are the same light-years, bit for bit.
#[track_caller]
fn assert_point(actual: [f64; 2], expected: [f64; 2]) {
    assert_same_bits(actual[0], expected[0]);
    assert_same_bits(actual[1], expected[1]);
}

/// Renders every row of `spec` in one call.
fn whole_map(fields: &Fields, spec: &MapSpec) -> Vec<f64> {
    let mut map = Vec::new();
    render_rows(fields, spec, 0..spec.height_px(), &mut map);
    map
}

#[test]
fn galaxy_map_specs_are_validated() {
    let all = MapSelection::AllSystems;
    let size = |size: [u32; 2]| MapSpec::new(MapView::FaceOn, all, size, [0.0, 0.0], 256.0);
    assert_eq!(
        size([0, 4]).unwrap_err(),
        BuildMapSpecError::EmptySize {
            width_px: 0,
            height_px: 4
        }
    );
    assert!(matches!(
        size([4, 0]).unwrap_err(),
        BuildMapSpecError::EmptySize { height_px: 0, .. }
    ));
    let pixel = |ly_per_px| MapSpec::new(MapView::FaceOn, all, [4, 4], [0.0, 0.0], ly_per_px);
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            pixel(bad).unwrap_err(),
            BuildMapSpecError::PixelSize { .. }
        ));
    }
    // The root cube's faces are allowed, since M1's maps span it exactly; a light-year beyond is
    // not, on either axis.
    assert_same_bits(ROOT_HALF, f64::from(ROOT_HALF_WIDTH_LY));
    assert_point(
        whole_cube(MapView::FaceOn, all, 512, 512).extent(),
        [2.0 * ROOT_HALF, 2.0 * ROOT_HALF],
    );
    let centred = |centre: [f64; 2]| MapSpec::new(MapView::EdgeOn, all, [512, 256], centre, 256.0);
    assert!(centred([0.0, 0.0]).is_ok());
    assert!(centred([0.0, 32_000.0]).is_ok());
    assert!(matches!(
        centred([1.0, 0.0]).unwrap_err(),
        BuildMapSpecError::OutsideRootCube { axis: 0, .. }
    ));
    assert!(matches!(
        centred([0.0, 33_000.0]).unwrap_err(),
        BuildMapSpecError::OutsideRootCube { axis: 1, .. }
    ));
    assert!(matches!(
        centred([f64::NAN, 0.0]).unwrap_err(),
        BuildMapSpecError::OutsideRootCube { axis: 0, .. }
    ));
    // A pixel of the order of the last place of the picture's own light-years is refused: two of a
    // row's edges would land on one `f64`, leaving it no height and its mean column a NaN.
    assert!(matches!(
        MapSpec::new(MapView::EdgeOn, all, [512, 256], [0.0, 32_000.0], 1e-12).unwrap_err(),
        BuildMapSpecError::UnresolvedRows { .. }
    ));
    let spec = whole_cube(MapView::EdgeOn, MapSelection::YoungOnly, 32, 16);
    assert_eq!(spec.view(), MapView::EdgeOn);
    assert_eq!(spec.selection(), MapSelection::YoungOnly);
    assert_eq!((spec.width_px(), spec.height_px()), (32, 16));
    assert_eq!(spec.pixel_count(), 512);
    assert_point(spec.centre(), [0.0, 0.0]);
    assert_same_bits(spec.ly_per_px(), 4_096.0);
    // Column i and row j have their centre where the wire's `DensityMap` says (plan 04).
    assert_point(spec.pixel_centre(0, 0), [-63_488.0, 30_720.0]);
    assert_point(spec.pixel_centre(31, 15), [63_488.0, -30_720.0]);
    // Edge-on, the rows' heights meet without a gap and hold their centres.
    assert_point(spec.pixel_span(0), [28_672.0, 32_768.0]);
    assert_point(spec.pixel_span(15), [-32_768.0, -28_672.0]);
    for row in 0..spec.height_px() - 1 {
        assert_same_bits(spec.pixel_span(row)[0], spec.pixel_span(row + 1)[1]);
        let [lo, hi] = spec.pixel_span(row);
        assert!((f64::midpoint(lo, hi) - spec.pixel_centre(0, row)[1]).abs() < 1e-9);
    }
}

/// Every pixel a band holds is the value of the public column density at that pixel, bit for bit.
#[test]
fn galaxy_map_rows_hold_the_column_densities() {
    let fields = fixture();
    for (view, selection) in VIEWS {
        let spec = whole_cube(view, selection, 24, 12);
        let map = whole_map(&fields, &spec);
        assert_eq!(map.len(), 24 * 12);
        for row in 0..spec.height_px() {
            for column in 0..spec.width_px() {
                let [x, up] = spec.pixel_centre(column, row);
                let [z_lo, z_hi] = spec.pixel_span(row);
                let expected = match view {
                    MapView::FaceOn => column_density_face_on(&fields, x, up, selection),
                    MapView::EdgeOn => column_density_edge_on(&fields, x, z_lo, z_hi, selection),
                };
                let at = usize::try_from(row * spec.width_px() + column).expect("a pixel");
                assert_same_bits(map[at], expected);
                assert!(map[at] >= 0.0, "{view:?} at ({x}, {up}): {}", map[at]);
            }
        }
    }
}

/// A map split into bands of any size holds the same bits as the whole map: what plan 04's pool of
/// band jobs needs (P04.T11.c).
#[test]
fn galaxy_map_rows_are_the_same_in_any_band() {
    let fields = fixture();
    for (view, selection) in VIEWS {
        let spec = whole_cube(view, selection, 21, 13);
        let whole = whole_map(&fields, &spec);
        let width = usize::try_from(spec.width_px()).expect("a width");
        for band in [1, 2, 3, 5, 8, 13] {
            let mut assembled = Vec::new();
            let mut rows = Vec::new();
            let mut start = 0;
            while start < spec.height_px() {
                let end = (start + band).min(spec.height_px());
                render_rows(&fields, &spec, start..end, &mut rows);
                assert_eq!(rows.len(), usize::try_from(end - start).unwrap() * width);
                assembled.extend_from_slice(&rows);
                start = end;
            }
            assert_eq!(assembled.len(), whole.len());
            for (at, (&ours, &theirs)) in assembled.iter().zip(&whole).enumerate() {
                assert_eq!(
                    bits(ours),
                    bits(theirs),
                    "{view:?} {selection:?}, bands of {band}, pixel {at}: {ours:e} against {theirs:e}"
                );
            }
        }
        // And in any order: plan 04's workers finish in whatever order they please, so the bands
        // are rendered back to front and out of step here, into buffers of their own, and laid down
        // by index. Nothing carries from one call to the next, so the bits must be the same.
        let bands: Vec<std::ops::Range<u32>> = vec![9..13, 0..2, 5..9, 2..5];
        let mut assembled = vec![f64::NAN; whole.len()];
        for rows in bands {
            let mut buf = Vec::new();
            render_rows(&fields, &spec, rows.clone(), &mut buf);
            let at = usize::try_from(rows.start).expect("a row") * width;
            assembled[at..at + buf.len()].copy_from_slice(&buf);
        }
        for (at, (&ours, &theirs)) in assembled.iter().zip(&whole).enumerate() {
            assert_eq!(
                bits(ours),
                bits(theirs),
                "{view:?} {selection:?}, bands out of order, pixel {at}: {ours:e} against {theirs:e}"
            );
        }
        // A band rendered twice, and one buffer reused, give the same bits again.
        let mut once = Vec::new();
        render_rows(&fields, &spec, 4..9, &mut once);
        let mut again = vec![f64::NAN; 3];
        render_rows(&fields, &spec, 4..9, &mut again);
        assert_eq!(once.len(), again.len());
        for (&a, &b) in once.iter().zip(&again) {
            assert_same_bits(a, b);
        }
        // An empty range renders nothing.
        render_rows(&fields, &spec, 7..7, &mut again);
        assert!(again.is_empty());
    }
}

#[test]
#[should_panic(expected = "reach past the map's")]
fn galaxy_map_rows_beyond_the_raster_are_refused() {
    let fields = fixture();
    let spec = whole_cube(MapView::FaceOn, MapSelection::AllSystems, 8, 8);
    let mut out = Vec::new();
    render_rows(&fields, &spec, 4..9, &mut out);
}

/// The young selection counts the young thin disc alone, and its column is the whole column's
/// young part.
#[test]
fn galaxy_map_counts_the_young_disc_alone_when_asked() {
    let fields = fixture();
    let young = &fields.components()[0];
    assert_eq!(young.population(), Population::YoungThinDisc);
    let Shape::Disc(disc) = young.shape() else {
        panic!("the young thin disc is a disc")
    };
    for (x, y) in [(0.0, 0.0), (26_000.0, 0.0), (-12_000.0, 17_000.0)] {
        let ours = column_density_face_on(&fields, x, y, MapSelection::YoungOnly);
        let r = (x * x + y * y).sqrt();
        let arm = disc.arm().expect("the young disc has arms");
        let expected = 2.0
            * disc.height().value()
            * disc.envelope(r, 0.0)
            * arm.factor(&fields.arms().point(x, y));
        assert_same_bits(ours, expected);
        assert!(ours < column_density_face_on(&fields, x, y, MapSelection::AllSystems));
    }
}

/// The face-on map's integral over the plane is the galaxy's system count: every component's
/// column, summed over the pixels and multiplied by a pixel's area, is what its density integrates
/// to over the root cube.
///
/// The map's square loses the discs' tails beyond the cube, so the sum falls a little short of N:
/// 0.1226% of N by an independent two-dimensional quadrature of each disc over the square, 0.1202
/// of it the two thin discs at 8,480 ly of scale length against the cube's 65,536 (the thick disc
/// adds 0.0024% and nothing else reaches the edge, the halo's cut lying inside it).
///
/// What the resolution buys is the rest: the midpoint rule over the pixel grid, which is almost
/// entirely the nuclear disc, whose 290 ly of scale length a 512 ly grid under-reads by some 5%
/// (0.085% of N, the whole of the 256 × 256 raster's shortfall beyond the tails). At 128 ly the
/// nuclear disc is resolved and the shortfall is the tails alone.
#[test]
#[ignore = "slow: a 1,024 × 1,024 face-on map of the whole cube"]
fn galaxy_map_face_on_holds_the_galaxys_systems() {
    let params = GalaxyParams::milky_way_like();
    let fields = Fields::new(&params, &MassModel::new(&params));
    for size in [256_u32, 1_024] {
        let spec = whole_cube(MapView::FaceOn, MapSelection::AllSystems, size, size);
        let area = spec.ly_per_px() * spec.ly_per_px();
        let total: f64 = whole_map(&fields, &spec).iter().sum::<f64>() * area;
        let count = params.system_count();
        println!(
            "face-on map {size} × {size}: {total:e} systems against N = {count:e}, \
             {:+.3}%",
            100.0 * (total / count - 1.0)
        );
        assert!(
            (total / count - 1.0).abs() < 0.005,
            "{size} × {size}: {total:e} against {count:e}"
        );
    }
}

/// The pixels an edge-on brute force is checked at: the plane and the heights above it at several
/// radii, the bar's end, a pixel straddling the plane, and the cube's corner.
fn edge_on_pixels(height: f64) -> Vec<(f64, f64, f64)> {
    let mut pixels = Vec::new();
    for x in [0.0, 128.0, 2_000.0, 8_000.0, 16_000.0, 26_000.0, 60_000.0] {
        for z in [0.0, 512.0, 4_000.0, 20_000.0] {
            pixels.push((x, z, z + height));
        }
    }
    // Across the plane, and beyond the halo's cut.
    pixels.push((26_000.0, -0.5 * height, 0.5 * height));
    pixels.push((-40_000.0, -30_000.0, -30_000.0 + height));
    pixels
}

/// An edge-on pixel by brute force: Simpson's rule over the line of sight inside the root cube and
/// over the pixel's height, divided by that height.
fn brute_force_edge_on(fields: &Fields, x: f64, z_lo: f64, z_hi: f64, steps: u32) -> f64 {
    let mut densities = [0.0; MAX_COMPONENTS];
    let mut along = |z: f64| {
        let h = 2.0 * ROOT_HALF / f64::from(steps);
        let at = |y: f64, densities: &mut [f64; MAX_COMPONENTS]| {
            fields.densities(&PointLy::new(x, y, z), densities)
        };
        let mut sum = at(-ROOT_HALF, &mut densities) + at(ROOT_HALF, &mut densities);
        for i in 1..steps {
            let weight = if i % 2 == 0 { 2.0 } else { 4.0 };
            sum += weight * at(-ROOT_HALF + f64::from(i) * h, &mut densities);
        }
        sum * h / 3.0
    };
    let steps_z = 8_u32;
    let h = (z_hi - z_lo) / f64::from(steps_z);
    let mut sum = along(z_lo) + along(z_hi);
    for i in 1..steps_z {
        let weight = if i % 2 == 0 { 2.0 } else { 4.0 };
        sum += weight * along(z_lo + f64::from(i) * h);
    }
    sum * h / 3.0 / (z_hi - z_lo)
}

/// Asserts the edge-on column at `pixels` against the brute force, and returns the worst relative
/// error.
fn assert_edge_on_against_brute_force(
    fields: &Fields,
    pixels: &[(f64, f64, f64)],
    steps: u32,
) -> f64 {
    let mut worst = 0.0_f64;
    for &(x, z_lo, z_hi) in pixels {
        let ours = column_density_edge_on(fields, x, z_lo, z_hi, MapSelection::AllSystems);
        let theirs = brute_force_edge_on(fields, x, z_lo, z_hi, steps);
        let error = (ours / theirs - 1.0).abs();
        assert!(
            error < 3e-3,
            "x {x}, z {z_lo}–{z_hi}: {ours:e} against {theirs:e}, {error:e} apart"
        );
        worst = worst.max(error);
    }
    worst
}

/// The edge-on columns against a brute-force integral over the line of sight and the pixel's
/// height, at a handful of pixels; the slow test below covers 200.
#[test]
fn galaxy_map_edge_on_columns_match_a_brute_force_integral() {
    let fields = fixture();
    let pixels = [
        (128.0, 0.0, 256.0),
        (26_000.0, 0.0, 256.0),
        (26_000.0, -128.0, 128.0),
        (8_000.0, 2_000.0, 2_256.0),
        (16_000.0, 20_000.0, 20_256.0),
        (-40_000.0, 8_000.0, 8_256.0),
    ];
    let worst = assert_edge_on_against_brute_force(&fields, &pixels, 2_000);
    println!("edge-on, six pixels: worst relative error {worst:e}");
}

/// The plan's check: 200 pixels, mid-plane pixels of 256 ly among them, to 3 × 10⁻³.
#[test]
#[ignore = "slow: 200 edge-on pixels against a 4,000-step brute force"]
fn galaxy_map_edge_on_columns_match_a_brute_force_integral_at_two_hundred_pixels() {
    let fields = fixture();
    let mut pixels = edge_on_pixels(256.0);
    let mut lcg = Lcg::new(0x0210_b000_0000_0001);
    while pixels.len() < 200 {
        let x = 131_000.0 * (lcg.next_f64() - 0.5);
        let z = 60_000.0 * (lcg.next_f64() - 0.5);
        pixels.push((x, z, z + 256.0));
    }
    let worst = assert_edge_on_against_brute_force(&fields, &pixels, 4_000);
    println!("edge-on, 200 pixels: worst relative error {worst:e}");
}

/// A disc thinner than a pixel keeps its light: the edge-on column is the mean over the pixel's
/// height, not a sample at its centre, so the mid-plane pixel of the young thin disc stands above
/// the point sample by the ratio of its profile's mean to its value there.
#[test]
fn galaxy_map_keeps_a_thin_disc_in_a_pixel_taller_than_it() {
    let fields = fixture();
    let Shape::Disc(disc) = fields.components()[0].shape() else {
        panic!("the young thin disc is a disc")
    };
    let profile = disc.profile();
    let height = disc.height().value();
    assert!(height < 512.0, "the young disc is thinner than a pixel");
    let x = 26_000.0;
    let young = MapSelection::YoungOnly;
    let mean = |lo: f64, hi: f64| {
        (profile.integral_to(hi).value() - profile.integral_to(lo).value()) / (hi - lo)
    };
    for pixel in [256.0, 1_024.0, 8_192.0, 2.0 * ROOT_HALF] {
        let ours = column_density_edge_on(&fields, x, 0.0, pixel, young);
        // A point sample at the pixel's centre: a pixel a tenth of a light-year tall there.
        let middle = 0.5 * pixel;
        let sample = column_density_edge_on(&fields, x, middle - 0.05, middle + 0.05, young);
        let against_point = mean(0.0, pixel) / profile.value(middle);
        assert!(against_point > 1.0, "{pixel} ly tall: {against_point}");
        println!(
            "young disc, a pixel {pixel} ly tall: {against_point:.4} times a point sample at its \
             centre, which reads {sample:e}"
        );
        if sample > 0.0 {
            // The column is the mean of the profile over the pixel, so the two stand in the ratio
            // of those means.
            let expected = mean(0.0, pixel) / mean(middle - 0.05, middle + 0.05);
            assert!(
                (ours / sample / expected - 1.0).abs() < 1e-12,
                "{pixel} ly tall: {ours:e} against {sample:e}, a factor of {expected:e}"
            );
        } else {
            // At the centre of a pixel this tall the young disc is under the map's floor and is
            // left out, and yet the pixel keeps its light.
            assert!(ours > 0.0, "{pixel} ly tall: {ours:e}");
            assert!(against_point > 1e6, "{pixel} ly tall: {against_point:e}");
        }
    }
    // A column of a 512 × 256 raster holds the same light as one pixel spanning it: the means
    // times their heights add up, to the rounding of a sum of 256 terms and to the rows where the
    // disc falls under the map's floor.
    let spec = whole_cube(MapView::EdgeOn, young, 512, 256);
    let map = whole_map(&fields, &spec);
    let width = usize::try_from(spec.width_px()).expect("a width");
    let column = 255_u32;
    let at = usize::try_from(column).expect("a column");
    let stack: f64 = (0..256).map(|row| map[row * width + at]).sum::<f64>() * spec.ly_per_px();
    let (top, bottom) = (spec.pixel_span(0)[1], spec.pixel_span(255)[0]);
    let across =
        column_density_edge_on(&fields, spec.pixel_centre(column, 0)[0], bottom, top, young)
            * (top - bottom);
    assert!(
        (stack / across - 1.0).abs() < 1e-9,
        "{stack:e} against {across:e}"
    );
}

/// Both views of a 16 × 16 raster of the whole cube, for the fixture and the three pinned seeds.
fn write_map(w: &mut GoldenWriter, label: &str, galaxy: &Galaxy) {
    let mut write = |name: &str, spec: &MapSpec| {
        let map = whole_map(galaxy.fields(), spec);
        let width = usize::try_from(spec.width_px()).expect("a width");
        for (at, &value) in map.iter().enumerate() {
            let (row, column) = (at / width, at % width);
            w.f64(&format!("{label}.{name}[{row}][{column}]"), value);
        }
    };
    for (view, selection) in VIEWS {
        let name = match (view, selection) {
            (MapView::FaceOn, MapSelection::AllSystems) => "face_on.all",
            (MapView::FaceOn, MapSelection::YoungOnly) => "face_on.young",
            (MapView::EdgeOn, MapSelection::AllSystems) => "edge_on.all",
            (MapView::EdgeOn, MapSelection::YoungOnly) => "edge_on.young",
        };
        write(name, &whole_cube(view, selection, 16, 16));
    }
    // An odd number of rows, so that the middle one straddles the plane: the only raster whose
    // pixels take the split quadrature and the sum of two half-columns.
    write(
        "edge_on.straddle",
        &whole_cube(MapView::EdgeOn, MapSelection::AllSystems, 16, 15),
    );
}

#[test]
fn galaxy_map_values_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("# milky_way, seed 0");
    write_map(
        &mut w,
        "milky_way",
        &Galaxy::from_params(Seed::new(0), GalaxyParams::milky_way_like()),
    );
    for s in PINNED {
        let seed = Seed::new(s);
        w.line(&format!("# {seed}"));
        write_map(&mut w, &seed.to_string(), &Galaxy::new(seed));
    }
    golden!("galaxy_map", w.as_str());
}
