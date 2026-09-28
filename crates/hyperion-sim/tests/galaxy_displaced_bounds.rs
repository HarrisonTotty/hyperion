//! Bounds for the flared displaced classes (plan 08, P08.T11): the flare factor's supremum and
//! the radial envelope's nearest corner, checked by sampling and by a hunt for violations in the
//! inner galaxy, against the test-only form table (`displaced_support`).

#[expect(
    dead_code,
    reason = "the bounds tests read only the disc-born rows of the shared table"
)]
mod displaced_support;

use displaced_support::{disc_born, scales_and_fields, seed_params};
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::bounds::CellBox;
use hyperion_sim::galaxy::displaced::bound::form_bound;
use hyperion_sim::galaxy::displaced::forms::{DiscBornForm, young_disc};
use hyperion_sim::galaxy::displaced::{AGE_BINS, AgeBin, GalaxyScales, SPEED_BINS};
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

/// Every disc-born class of the test-only table in one galaxy, flared, ballistic and blurred-arm.
fn classes(scales: &GalaxyScales, fields: &Fields) -> Vec<DiscBornForm> {
    let (young, arm) = young_disc(fields);
    let mut out = Vec::with_capacity(SPEED_BINS * AGE_BINS);
    for speed in 0..SPEED_BINS {
        for a in 0..AGE_BINS {
            let (layer, spheroid, mean_ut) = disc_born(speed, a);
            let age = AgeBin::new(u8::try_from(a).unwrap()).unwrap();
            out.push(
                DiscBornForm::new(&layer, &spheroid, age, mean_ut, (young, &arm), scales).unwrap(),
            );
        }
    }
    out
}

/// A random cell of layer D (64 ly) or E (128 ly), within 60,000 ly of the axis and 8,192 ly of
/// the plane, mostly near it.
fn random_cell(lcg: &mut Lcg) -> CellBox {
    let edge: u32 = if lcg.next_f64() < 0.5 { 64 } else { 128 };
    let e = i64::from(edge);
    let mut coord = |reach: f64, bias: f64| {
        let u = lcg.next_f64();
        let t = reach * hyperion_sim::math::powf(u, bias);
        let sign = if lcg.next_f64() < 0.5 { -1.0 } else { 1.0 };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a coordinate within the cube"
        )]
        let cells = (sign * t / f64::from(edge)).floor() as i64;
        i32::try_from(cells * e).unwrap()
    };
    let (x, y, z) = (
        coord(42_000.0, 1.0),
        coord(42_000.0, 1.0),
        coord(8_192.0, 3.0),
    );
    CellBox::new([x, y, z], edge).unwrap()
}

/// A uniformly random point of `cell`.
fn interior(cell: &CellBox, lcg: &mut Lcg) -> PointLy {
    let edge = f64::from(cell.edge());
    let [x, y, z] = cell.min_corner().map(f64::from);
    PointLy::new(
        x + edge * lcg.next_f64(),
        y + edge * lcg.next_f64(),
        z + edge * lcg.next_f64(),
    )
}

/// P08.T11: over 10⁵ random (cell, class) pairs of 512 interior points each, no density exceeds its
/// class's bound, and the bound lies within a factor of 1.6 of the sampled maximum on average.
#[test]
fn flare_bound_is_tight_and_safe() {
    let (scales, fields) = scales_and_fields(&GalaxyParams::milky_way_like());
    let forms = classes(&scales, &fields);
    let mut lcg = Lcg::new(0x0811_0000);
    let pairs = 100_000;
    let mut ratio_sum = 0.0;
    for _ in 0..pairs {
        let cell = random_cell(&mut lcg);
        let pick = usize::try_from(lcg.next_u64() % u64::try_from(forms.len()).unwrap()).unwrap();
        let form = &forms[pick];
        let bound = form_bound(form, &cell);
        let mut best: f64 = 0.0;
        for _ in 0..512 {
            let p = interior(&cell, &mut lcg);
            let d = form.density(&p);
            assert!(
                d <= bound,
                "class {pick}: {d:e} at {p:?} exceeds {bound:e} over {cell:?}"
            );
            best = best.max(d);
        }
        assert!(best > 0.0, "class {pick} over {cell:?}");
        ratio_sum += bound / best;
    }
    let mean = ratio_sum / f64::from(pairs);
    eprintln!("mean bound ÷ sampled maximum: {mean:.4}");
    assert!(mean <= 1.6, "the bound is loose: {mean}");
}

/// The coordinate-ascent maximum of `density` over `cell` from `start`: one sweep of a
/// golden-section search of four steps along each axis in turn, keeping the best of the section's
/// points and the axis's two ends, clamped to the cell.
fn ascend(cell: &CellBox, start: [f64; 3], density: &impl Fn(&PointLy) -> f64) -> (f64, PointLy) {
    let low = cell.min_corner().map(f64::from);
    let edge = f64::from(cell.edge());
    let mut at = start;
    let value = |point: [f64; 3]| density(&PointLy::new(point[0], point[1], point[2]));
    let mut best = value(at);
    let ratio = 0.618_033_988_749_894_9;
    for axis in 0..3 {
        let base = at;
        let probe = |t: f64| {
            let mut point = base;
            point[axis] = t;
            (value(point), t)
        };
        let (mut a, mut b) = (low[axis], low[axis] + edge);
        let mut c = probe(b - ratio * (b - a));
        let mut d = probe(a + ratio * (b - a));
        let mut seen = [probe(a), probe(b), c, d];
        for _ in 0..4 {
            if c.0 > d.0 {
                b = d.1;
                d = c;
                c = probe(b - ratio * (b - a));
                seen[2] = c;
            } else {
                a = c.1;
                c = d;
                d = probe(a + ratio * (b - a));
                seen[3] = d;
            }
            for candidate in [c, d] {
                if candidate.0 > best {
                    best = candidate.0;
                    at[axis] = candidate.1;
                }
            }
        }
        for (v, t) in seen {
            if v > best {
                best = v;
                at[axis] = t;
            }
        }
    }
    (best, PointLy::new(at[0], at[1], at[2]))
}

/// A map of (x, y) that keeps z.
type Symmetry = fn(f64, f64) -> (f64, f64);

/// The eight symmetries of the square about the z axis: each one negates and swaps coordinates,
/// both exact in floating point.
const SYMMETRIES: [Symmetry; 8] = [
    |x, y| (x, y),
    |x, y| (-x, y),
    |x, y| (x, -y),
    |x, y| (-x, -y),
    |x, y| (y, x),
    |x, y| (-y, x),
    |x, y| (y, -x),
    |x, y| (-y, -x),
];

/// `p` under `symmetry`.
fn image_of(symmetry: Symmetry, p: &PointLy) -> PointLy {
    let (x, y) = symmetry(p.x, p.y);
    PointLy::new(x, y, p.z)
}

/// `cell` under `symmetry`: the cell spanned by its two corners' images, exact, since every
/// corner coordinate is an integer.
fn cell_image(symmetry: Symmetry, cell: &CellBox) -> CellBox {
    let low = cell.min_corner().map(f64::from);
    let edge = f64::from(cell.edge());
    let (ax, ay) = symmetry(low[0], low[1]);
    let (bx, by) = symmetry(low[0] + edge, low[1] + edge);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an integer corner within the cube"
    )]
    let corner = [ax.min(bx) as i32, ay.min(by) as i32, cell.min_corner()[2]];
    CellBox::new(corner, cell.edge()).unwrap()
}

/// The 27 points of the 3 × 3 × 3 lattice of `cell`, from its low corner by half edges.
fn lattice(cell: &CellBox) -> impl Iterator<Item = [f64; 3]> {
    let low = cell.min_corner().map(f64::from);
    let side = f64::from(cell.edge());
    (0..27_u8).map(move |i| {
        let offset = [i % 3, (i / 3) % 3, i / 9].map(|j| f64::from(j) * 0.5 * side);
        [low[0] + offset[0], low[1] + offset[1], low[2] + offset[2]]
    })
}

/// The hunt's cells of `edge` along `azimuth` (degrees) out to radius `reach` (ly): every 384 ly
/// in R and in z, R under `reach` and z from the plane to under 4,096 ly.
fn hunt_cells(edge: u32, azimuth: f64, reach: f64) -> Vec<CellBox> {
    let side = f64::from(edge);
    let snap = |v: f64| {
        #[expect(clippy::cast_possible_truncation, reason = "inside the cube")]
        let cells = (v / side).floor() as i32;
        cells * i32::try_from(edge).unwrap()
    };
    let (sin, cos) = hyperion_sim::math::sin_cos(azimuth.to_radians());
    let mut cells = Vec::new();
    let mut radius = 0.0;
    while radius < reach {
        let mut height = 0.0;
        while height < 4_096.0 {
            let corner = [snap(radius * cos), snap(radius * sin), snap(height)];
            cells.push(CellBox::new(corner, edge).unwrap());
            height += 384.0;
        }
        radius += 384.0;
    }
    cells
}

/// P08.T11's hunt over `seeds`, which the four slow tests below share out; it prints the worst
/// density ÷ bound it found.
///
/// A class without an arm is exactly axisymmetric: its density reads x and y only through
/// `hypot(x, y)` (libm's, which drops both signs and orders its arguments first), and its bound
/// only through the cell's `R` range, `√(x² + y²)` at two corners, and its least |z|, so negating
/// or swapping x and y moves neither by a bit. So at 90° and 135° such a class is hunted only in
/// a cell that no symmetry of the square maps onto a cell already hunted at 0° or 45°; for every
/// other cell the test checks instead that the bound and the densities at the 27 starts are those
/// of the image, bit for bit. The classes with an arm are hunted along all four azimuths.
fn hunt(seeds: std::ops::Range<u64>) {
    let mut worst: f64 = 0.0;
    let (mut hunted, mut mirrored) = (0_u64, 0_u64);
    for seed in seeds.clone() {
        let params = seed_params(seed);
        let (scales, fields) = scales_and_fields(&params);
        let forms = classes(&scales, &fields);
        let reach = 3.0 * scales.r_d().value();
        for edge in [64_u32, 128] {
            let covered: std::collections::BTreeSet<[i32; 3]> = [0.0, 45.0]
                .into_iter()
                .flat_map(|azimuth| hunt_cells(edge, azimuth, reach))
                .map(|cell| cell.min_corner())
                .collect();
            for azimuth in [0.0_f64, 45.0, 90.0, 135.0] {
                for cell in hunt_cells(edge, azimuth, reach) {
                    let image = if azimuth < 90.0 {
                        None
                    } else {
                        SYMMETRIES.into_iter().find_map(|symmetry| {
                            let image = cell_image(symmetry, &cell);
                            covered
                                .contains(&image.min_corner())
                                .then_some((symmetry, image))
                        })
                    };
                    for (k, form) in forms.iter().enumerate() {
                        let bound = form_bound(form, &cell);
                        if let (None, Some((symmetry, image))) = (form.arm(), &image) {
                            assert_eq!(
                                bits(bound),
                                bits(form_bound(form, image)),
                                "seed {seed} class {k}: {cell:?} and its image {image:?}"
                            );
                            for start in lattice(&cell) {
                                let p = PointLy::new(start[0], start[1], start[2]);
                                let q = image_of(*symmetry, &p);
                                assert_eq!(
                                    bits(form.density(&p)),
                                    bits(form.density(&q)),
                                    "seed {seed} class {k}: {p:?} and its image {q:?}"
                                );
                            }
                            mirrored += 1;
                            continue;
                        }
                        let density = |p: &PointLy| form.density(p);
                        for start in lattice(&cell) {
                            let (value, at) = ascend(&cell, start, &density);
                            assert!(
                                value <= bound * (1.0 + 1e-12),
                                "seed {seed} class {k}: {value:e} at {at:?} over {bound:e} in {cell:?}"
                            );
                            worst = worst.max(value / bound);
                        }
                        hunted += 1;
                    }
                }
            }
        }
    }
    eprintln!(
        "seeds {seeds:?}: {hunted} (cell, class) pairs hunted, {mirrored} checked as images of \
         hunted ones; worst density ÷ bound {worst}"
    );
}

/// **P08.T11's hunt** (slow), in four shards of five seeds: layer-E and layer-D cells with R under
/// 3 `R_d` and |z| under 4,096 ly, every 384 ly (a stride of three layer-E cells) in R and in z
/// along four azimuths (0°, 45°, 90° and 135°, which cross the arms at every phase the pitch
/// gives), every disc-born class of the test-only table, flared, ballistic and blurred-arm,
/// maximised by coordinate ascent from the 27 points of a 3 × 3 × 3 lattice of the cell, over 20
/// seeds: no density beyond its bound by more than 10⁻¹² relative. The axisymmetric classes are
/// hunted at 90° and 135° only where those cells are new ([`hunt`]). The plan's every cell on a
/// stride of three would be about 10⁴ times the work.
#[test]
#[ignore = "slow: coordinate ascent over 5 seeds' inner cells and 56 classes"]
fn hunt_flared_violations_in_the_inner_galaxy_seeds_0_to_4() {
    hunt(0..5);
}

/// [`hunt_flared_violations_in_the_inner_galaxy_seeds_0_to_4`]'s second shard.
#[test]
#[ignore = "slow: coordinate ascent over 5 seeds' inner cells and 56 classes"]
fn hunt_flared_violations_in_the_inner_galaxy_seeds_5_to_9() {
    hunt(5..10);
}

/// [`hunt_flared_violations_in_the_inner_galaxy_seeds_0_to_4`]'s third shard.
#[test]
#[ignore = "slow: coordinate ascent over 5 seeds' inner cells and 56 classes"]
fn hunt_flared_violations_in_the_inner_galaxy_seeds_10_to_14() {
    hunt(10..15);
}

/// [`hunt_flared_violations_in_the_inner_galaxy_seeds_0_to_4`]'s fourth shard.
#[test]
#[ignore = "slow: coordinate ascent over 5 seeds' inner cells and 56 classes"]
fn hunt_flared_violations_in_the_inner_galaxy_seeds_15_to_19() {
    hunt(15..20);
}

/// What the hunt's shortcut rests on: every class without an arm has the same density, bit for
/// bit, at a point and at its image under each symmetry of the square about the z axis, and the
/// same bound over a cell and over its image, in the Milky Way fixture.
#[test]
fn the_armless_classes_are_exactly_symmetric_about_the_axis() {
    let (scales, fields) = scales_and_fields(&GalaxyParams::milky_way_like());
    let forms = classes(&scales, &fields);
    let armless: Vec<&DiscBornForm> = forms.iter().filter(|f| f.arm().is_none()).collect();
    assert!(!armless.is_empty() && armless.len() < forms.len());
    let mut lcg = Lcg::new(0x0811_5e11);
    for _ in 0..200 {
        let cell = random_cell(&mut lcg);
        let p = interior(&cell, &mut lcg);
        for form in &armless {
            let (d, b) = (form.density(&p), form_bound(form, &cell));
            for symmetry in SYMMETRIES {
                let q = image_of(symmetry, &p);
                assert_eq!(bits(d), bits(form.density(&q)), "{p:?} and {q:?}");
                let image = cell_image(symmetry, &cell);
                assert_eq!(
                    bits(b),
                    bits(form_bound(form, &image)),
                    "{cell:?} and {image:?}"
                );
            }
        }
    }
}
