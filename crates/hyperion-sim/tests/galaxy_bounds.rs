//! The bounds (plan 02, P02.T8): cell geometry, the envelopes' nearest-corner bounds, and the arm
//! factors' bounds from a cell's ranges of radius and phase.
//!
//! The checks over thousands of cells run in full as slow tests under `just test-slow`; the fast
//! suite runs the same checks on a sample a debug build can afford.

use hyperion_sim::Seed;
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::bounds::{CellBox, ScalarRange, UnimodalFactor};
use hyperion_sim::galaxy::fields::arms::{Arm, ArmGeometry, GentleArm, SharpArm};
use hyperion_sim::galaxy::fields::{Component, Fields, Shape};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::{ArmCount, GalaxyParams};
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::math;
use hyperion_sim::units::{Degrees, LightYears, Radians};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;

/// The three pinned seeds of the golden files, as the fields'.
const PINNED: [u64; 3] = [
    0x0000_0000_0000_0001,
    0x5eed_0000_c0ff_ee00,
    0xdead_beef_cafe_f00d,
];

/// The edges bounded: the stellar layers' 8–128 ly and the 4 ly and 4,096 ly grids of later
/// plans (plan 02, P02.T8.a).
const EDGES: [u32; 7] = [4, 8, 16, 32, 64, 128, 4_096];

/// The root cube's half-width, light-years.
const ROOT: f64 = 65_536.0;

fn fields_of(params: &GalaxyParams) -> Fields {
    Fields::new(params, &MassModel::new(params))
}

fn seeded(seed: u64) -> Fields {
    fields_of(&GalaxyParams::from_seed(
        Seed::new(seed),
        MassFunctionKind::Kroupa,
    ))
}

/// The cell of `edge` on its grid that contains `p`: each coordinate rounded down to a multiple of
/// the edge.
fn cell_at(p: [f64; 3], edge: u32) -> CellBox {
    let size = f64::from(edge);
    let min = p.map(|c| {
        let low = (c / size).floor() * size;
        assert!(low.abs() <= 2.0 * ROOT, "{c} lies far outside the cube");
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a whole number of light-years inside twice the root cube fits in i32"
        )]
        let low = low as i32;
        low
    });
    CellBox::new(min, edge).expect("a grid cell never straddles a plane")
}

/// A random cell of `edge` inside the root cube: log-uniform in radius from 1 ly to 70,000 ly,
/// three in ten touching the plane z = 0 and the rest log-uniform in height up to 40,000 ly,
/// and one in five touching each of the planes x = 0 and y = 0.
fn random_cell(lcg: &mut Lcg, edge: u32) -> CellBox {
    let size = f64::from(edge);
    let reach = ROOT - size;
    let log_uniform = |lcg: &mut Lcg, top: f64| math::exp(lcg.next_f64() * math::ln(top));
    let sign = |lcg: &mut Lcg| if lcg.next_f64() < 0.5 { -1.0 } else { 1.0 };
    let near_plane = |lcg: &mut Lcg| size * (2.0 * lcg.next_f64() - 1.0);
    loop {
        let r = log_uniform(lcg, 70_000.0);
        let theta = 2.0 * core::f64::consts::PI * lcg.next_f64();
        let mut x = r * math::cos(theta);
        let mut y = r * math::sin(theta);
        if lcg.next_f64() < 0.2 {
            x = near_plane(lcg);
        }
        if lcg.next_f64() < 0.2 {
            y = near_plane(lcg);
        }
        let z = if lcg.next_f64() < 0.3 {
            near_plane(lcg)
        } else {
            sign(lcg) * log_uniform(lcg, 40_000.0)
        };
        if x.abs() < reach && y.abs() < reach && z.abs() < reach {
            return cell_at([x, y, z], edge);
        }
    }
}

/// The places where an envelope is likeliest to beat its corner: the centre, the bar's end, the
/// bulge's switch between its terms, and each halo component's core, break and cut, in every
/// octant and touching the axis planes.
fn targeted_cells(fields: &Fields, edge: u32) -> Vec<CellBox> {
    let mut points: Vec<[f64; 3]> = vec![[0.0; 3]];
    for component in fields.components() {
        match component.shape() {
            Shape::Bar(bar) => {
                let l = bar.half_length().value();
                for x in [0.85 * l, l, 1.15 * l] {
                    points.push([x, 0.0, 0.0]);
                    points.push([x, bar.width().value(), bar.height().value()]);
                }
            }
            Shape::Bulge(bulge) => {
                let (a, c) = (bulge.scale_x().value(), bulge.scale_z().value());
                // The planar and vertical terms equal, and the ratio at the series' switch.
                let switch = math::exp(-math::ln(256.0) / bulge.boxiness());
                for s in [0.25, 1.0, 3.0] {
                    points.push([a * s, 0.0, c * s]);
                    points.push([a * s, 0.0, c * s * switch]);
                    points.push([a * s * switch, 0.0, c * s]);
                }
            }
            Shape::Halo(halo) => {
                let q = halo.flattening();
                let mut radii = vec![halo.core().value(), halo.cut_radius().value()];
                radii.extend(halo.break_radius().map(LightYears::value));
                let diagonal = core::f64::consts::FRAC_1_SQRT_2;
                for m in radii {
                    let m = m.min(ROOT - 2.0 * f64::from(edge));
                    points.push([m, 0.0, 0.0]);
                    points.push([m * diagonal, m * diagonal, 0.0]);
                    points.push([0.0, 0.0, m * q]);
                    points.push([0.0, 0.0, m]);
                    points.push([m / 3.0_f64.sqrt(); 3]);
                }
            }
            Shape::Disc(_) => {}
        }
    }
    // Every sign, with a zero coordinate on both sides of its plane.
    let mut cells = Vec::new();
    for p in points {
        for signs in 0..8 {
            let q: [f64; 3] = std::array::from_fn(|axis| {
                let negative = signs & (1 << axis) != 0;
                match (negative, p[axis] > 0.0) {
                    (false, _) => p[axis],
                    (true, true) => -p[axis],
                    (true, false) => -0.5,
                }
            });
            cells.push(cell_at(q, edge));
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

/// The points an envelope check visits in `cell`: an `n³` lattice that includes the corners,
/// and the nearest corner stepped outwards by up to 32 units in the last place along each axis
/// and all three together, where floating point would first show a rise.
fn envelope_probes(cell: &CellBox, n: u32) -> Vec<PointLy> {
    let min = cell.min_corner().map(f64::from);
    let edge = f64::from(cell.edge());
    let steps = f64::from(n - 1);
    let lattice = usize::try_from(n * n * n).expect("a lattice of a few thousand points");
    let mut points = Vec::with_capacity(lattice + 4 * 32);
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let at = |m: f64, i: u32| m + edge * f64::from(i) / steps;
                points.push(PointLy::new(at(min[0], i), at(min[1], j), at(min[2], k)));
            }
        }
    }
    let near = cell.nearest_corner();
    let far = cell.farthest_corner();
    let outward = |from: f64, towards: f64| {
        if towards > from {
            from.next_up()
        } else {
            from.next_down()
        }
    };
    let (mut x, mut y, mut z) = (near.x, near.y, near.z);
    let (mut xs, mut ys, mut zs) = (near.x, near.y, near.z);
    for _ in 0..32 {
        xs = outward(xs, far.x);
        ys = outward(ys, far.y);
        zs = outward(zs, far.z);
        points.push(PointLy::new(xs, near.y, near.z));
        points.push(PointLy::new(near.x, ys, near.z));
        points.push(PointLy::new(near.x, near.y, zs));
        x = outward(x, far.x);
        y = outward(y, far.y);
        z = outward(z, far.z);
        points.push(PointLy::new(x, y, z));
    }
    points
}

/// For every component of `fields`: the envelope bound over `cell` equals the envelope at the
/// nearest corner to 1 part in 10¹², and no probe exceeds it.
fn assert_envelopes_bounded(fields: &Fields, cell: &CellBox, lattice: u32) {
    let probes = envelope_probes(cell, lattice);
    for p in &probes {
        assert!(cell.contains(p), "{p:?} outside {cell:?}");
    }
    for (i, component) in fields.components().iter().enumerate() {
        let bound = component.envelope_bound(cell);
        let corner = component.envelope(&cell.nearest_corner());
        if corner > 0.0 {
            assert!(
                (bound / corner - 1.0).abs() <= 1e-12,
                "component {i} in {cell:?}: bound {bound:e} against the corner's {corner:e}"
            );
        } else {
            assert!(
                bound.abs() < f64::MIN_POSITIVE,
                "component {i} in {cell:?}: a bound of {bound:e} over a zero corner"
            );
        }
        for p in &probes {
            let envelope = component.envelope(p);
            assert!(
                envelope <= bound,
                "component {i}: the envelope {envelope:e} at {p:?} exceeds the bound {bound:e} \
                 over {cell:?}"
            );
        }
    }
}

// Cell geometry (P02.T8.a).

/// A generation cell of every size and every layer's grid, anywhere in the root cube, is a
/// `CellBox`: the planes x, y, z = 0 are faces of every grid.
#[test]
fn every_grid_cell_is_a_cell_box() {
    let mut lcg = Lcg::new(0x0208_ce11);
    for edge in EDGES {
        for _ in 0..1_000 {
            let cell = random_cell(&mut lcg, edge);
            let min = cell.min_corner();
            let size = i32::try_from(edge).unwrap();
            // The corners are whole light-years, so their coordinates compare exactly as integers.
            let whole = |c: f64| {
                assert!(c.fract().abs() < f64::MIN_POSITIVE, "{c} in {cell:?}");
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a whole number of light-years inside the root cube"
                )]
                let c = c as i32;
                c
            };
            let (near, far) = (cell.nearest_corner(), cell.farthest_corner());
            for (axis, (n, f)) in [(near.x, far.x), (near.y, far.y), (near.z, far.z)]
                .into_iter()
                .enumerate()
            {
                let (n, f) = (whole(n), whole(f));
                assert_eq!(n.abs() + size, f.abs(), "axis {axis} of {cell:?}");
                assert!(
                    n == min[axis] || n == min[axis] + size,
                    "axis {axis} of {cell:?}"
                );
            }
            let centre = cell.centre();
            let radii = cell.r_cyl_range();
            let r_centre = (centre.x * centre.x + centre.y * centre.y).sqrt();
            assert!(
                radii.lo <= r_centre && r_centre <= radii.hi,
                "the centre's {r_centre} outside {radii:?} of {cell:?}"
            );
            let width = f64::from(edge) * 2.0_f64.sqrt();
            assert!(
                radii.hi - radii.lo <= width + 1e-9,
                "{radii:?} wider than the diagonal of {cell:?}"
            );
        }
    }
}

// Envelope bounds (P02.T8.a).

/// For every component of the three pinned seeds, on a sample of random cells of every size and
/// on the targeted cells, no envelope exceeds its bound and each bound is its corner's value to
/// 10⁻¹². The full count runs as a slow test.
#[test]
fn envelopes_never_exceed_their_bounds() {
    for (n, seed) in PINNED.into_iter().enumerate() {
        let fields = seeded(seed);
        let mut lcg = Lcg::new(0x0208_e0e1 ^ seed);
        for edge in EDGES {
            for _ in 0..12 {
                assert_envelopes_bounded(&fields, &random_cell(&mut lcg, edge), 9);
            }
            // The targeted cells of one seed per size keep the debug build's time in hand.
            if usize::try_from(edge.trailing_zeros()).unwrap() % PINNED.len() == n {
                for cell in targeted_cells(&fields, edge) {
                    assert_envelopes_bounded(&fields, &cell, 3);
                }
            }
        }
    }
}

/// The full check of P02.T8.a for one seed: 2,000 random cells of every size, each probed on a
/// 17³ lattice, and every targeted cell likewise.
fn envelopes_never_exceed_their_bounds_over_many_cells(seed: u64) {
    let fields = seeded(seed);
    let mut lcg = Lcg::new(0x0208_e0e2 ^ seed);
    for edge in EDGES {
        for _ in 0..2_000 {
            assert_envelopes_bounded(&fields, &random_cell(&mut lcg, edge), 17);
        }
        for cell in targeted_cells(&fields, edge) {
            assert_envelopes_bounded(&fields, &cell, 17);
        }
    }
}

#[test]
#[ignore = "slow: 14,000 cells on a 17³ lattice for every component"]
fn envelopes_never_exceed_their_bounds_over_many_cells_seed_0() {
    envelopes_never_exceed_their_bounds_over_many_cells(PINNED[0]);
}

#[test]
#[ignore = "slow: 14,000 cells on a 17³ lattice for every component"]
fn envelopes_never_exceed_their_bounds_over_many_cells_seed_1() {
    envelopes_never_exceed_their_bounds_over_many_cells(PINNED[1]);
}

#[test]
#[ignore = "slow: 14,000 cells on a 17³ lattice for every component"]
fn envelopes_never_exceed_their_bounds_over_many_cells_seed_2() {
    envelopes_never_exceed_their_bounds_over_many_cells(PINNED[2]);
}

/// Every envelope bound's bits over a few cells.
fn envelope_bound_bits(fields: &Fields, cells: &[CellBox]) -> Vec<u64> {
    cells
        .iter()
        .flat_map(|cell| {
            fields
                .components()
                .iter()
                .map(|c| hyperion_testkit::float::bits(c.envelope_bound(cell)))
        })
        .collect()
}

/// The bounds are a pure function of the galaxy and the cell: the same seed gives the same bits
/// twice, and building galaxies or bounding cells in any order changes nothing (the
/// sim-determinism rule for anything a cache may hold).
#[test]
fn envelope_bounds_are_pure_and_order_independent() {
    let mut lcg = Lcg::new(0x0208_0bde);
    let cells: Vec<CellBox> = EDGES
        .iter()
        .flat_map(|&edge| [random_cell(&mut lcg, edge), random_cell(&mut lcg, edge)])
        .collect();
    let first = envelope_bound_bits(&seeded(PINNED[1]), &cells);
    let _other = seeded(PINNED[0]);
    assert_eq!(first, envelope_bound_bits(&seeded(PINNED[1]), &cells));
    assert_order_independent(&PINNED, |&seed| envelope_bound_bits(&seeded(seed), &cells));
    let fields = seeded(PINNED[2]);
    assert_order_independent(&cells, |cell| {
        envelope_bound_bits(&fields, std::slice::from_ref(cell))
    });
}

// Arm bounds (P02.T8.b).

const PI: f64 = core::f64::consts::PI;
const TAU: f64 = core::f64::consts::TAU;

/// Arms at the corners of their ranges: two and four arms, pitches of 10° and 18°, bars of 10,000
/// and 18,000 ly; each with the sharp arm at a width of 250 ly and of 500 ly and at a fraction of
/// 0.9 and of 1 (beyond the drawn 0.7–0.9, where the factor nears 0 between arms), and the gentle
/// arm at its greatest amplitude, 0.3.
fn extreme_arms() -> Vec<Arm> {
    let mut arms = Vec::new();
    for count in [ArmCount::Two, ArmCount::Four] {
        for pitch in [10.0, 18.0] {
            for bar in [10_000.0, 18_000.0] {
                let geometry = ArmGeometry::new(
                    count,
                    Radians::from(Degrees::new(pitch)),
                    LightYears::new(bar),
                )
                .unwrap();
                for (width, fraction) in [(250.0, 0.9), (500.0, 1.0)] {
                    let sharp = SharpArm::new(geometry, LightYears::new(width), fraction);
                    arms.push(Arm::Sharp(sharp.unwrap()));
                }
                arms.push(Arm::Gentle(GentleArm::new(geometry, 0.3).unwrap()));
            }
        }
    }
    arms
}

/// A random cell for the arm checks: half as [`random_cell`] does, half in the plane between half
/// and three times the bar's half-length, where the arms fade in and are sharpest against the
/// cell.
fn random_arm_cell(lcg: &mut Lcg, geometry: &ArmGeometry, edge: u32) -> CellBox {
    if lcg.next_f64() < 0.5 {
        return random_cell(lcg, edge);
    }
    let l = geometry.bar_half_length().value();
    let r = l * (0.5 + 2.5 * lcg.next_f64());
    let theta = TAU * lcg.next_f64();
    let z = f64::from(edge) * (2.0 * lcg.next_f64() - 1.0);
    cell_at([r * math::cos(theta), r * math::sin(theta), z], edge)
}

/// The points of `cell`'s square face an arm scan visits: an `n × n` lattice with its corners,
/// and the points where each ridge crosses the square, sampled every `edge ÷ 64` in radius.
fn arm_probes(cell: &CellBox, geometry: &ArmGeometry, n: u32) -> Vec<(f64, f64)> {
    let [x0, y0, _] = cell.min_corner().map(f64::from);
    let edge = f64::from(cell.edge());
    let steps = f64::from(n - 1);
    let mut points = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let at = |m: f64, i: u32| m + edge * f64::from(i) / steps;
            points.push((at(x0, i), at(y0, j)));
        }
    }
    let radii = cell.r_cyl_range();
    let inside = |x: f64, y: f64| (x0..=x0 + edge).contains(&x) && (y0..=y0 + edge).contains(&y);
    let samples = 64.0 * (radii.hi - radii.lo) / edge;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few hundred samples at most"
    )]
    let samples = samples.ceil() as u32;
    for ridge in 0..geometry.count().get() {
        for i in 0..=samples {
            let r = radii.lo + (radii.hi - radii.lo) * f64::from(i) / f64::from(samples.max(1));
            if r <= 0.0 {
                continue;
            }
            let theta = geometry.ridge_azimuth(r, ridge);
            let (x, y) = (r * math::cos(theta), r * math::sin(theta));
            if inside(x, y) {
                points.push((x, y));
            }
        }
    }
    points
}

/// No factor at any probe of `cell` exceeds the arm's bound over the cell; returns the greatest
/// probe ÷ bound.
fn assert_arm_bounded(arm: &Arm, cell: &CellBox, n: u32) -> f64 {
    let geometry = arm.geometry();
    let bound = arm
        .across(cell.r_cyl_range())
        .sup(geometry.phase_range(cell));
    let mut worst: f64 = 0.0;
    for (x, y) in arm_probes(cell, geometry, n) {
        let factor = arm.factor(&geometry.point(x, y));
        assert!(
            factor <= bound,
            "{arm:?}: the factor {factor} at ({x}, {y}) exceeds the bound {bound} over {cell:?}"
        );
        worst = worst.max(factor / bound);
    }
    worst
}

/// The phase at every point of a cell lies in the cell's range of phase, unwrapped about its
/// middle.
#[test]
fn phase_ranges_hold_every_phase_of_their_cells() {
    let mut lcg = Lcg::new(0x0208_b0a5);
    for arm in extreme_arms() {
        let geometry = arm.geometry();
        for edge in EDGES {
            for _ in 0..40 {
                let cell = random_arm_cell(&mut lcg, geometry, edge);
                let range = geometry.phase_range(&cell);
                if range.hi.is_infinite() {
                    assert!(range.lo.is_infinite(), "{range:?} over {cell:?}");
                    continue;
                }
                let middle = f64::midpoint(range.lo, range.hi);
                let half = 0.5 * (range.hi - range.lo);
                assert!(half < PI, "{range:?} over {cell:?}");
                let [x0, y0, _] = cell.min_corner().map(f64::from);
                for _ in 0..16 {
                    let x = x0 + f64::from(edge) * lcg.next_f64();
                    let y = y0 + f64::from(edge) * lcg.next_f64();
                    let phase = geometry.phase(x, y).unwrap();
                    let off = phase - middle - TAU * ((phase - middle) / TAU).round();
                    assert!(
                        off.abs() <= half * (1.0 + 1e-12) + 1e-12,
                        "the phase {phase} at ({x}, {y}) lies {off} from the middle of {range:?}"
                    );
                }
            }
        }
    }
}

/// Each arm factor's bound against a dense scan of the factor over 5,000 random cells of every
/// size (P02.T8.b): a 17 × 17 lattice of the cell's face with its corners, and every ridge
/// crossing. The factor does not depend on z.
#[test]
fn arm_factor_bounds_hold_over_random_cells() {
    let arms = extreme_arms();
    let mut lcg = Lcg::new(0x0208_b0a6);
    for i in 0..5_000 {
        let arm = &arms[i % arms.len()];
        let edge = EDGES[(i / arms.len()) % EDGES.len()];
        let cell = random_arm_cell(&mut lcg, arm.geometry(), edge);
        assert_arm_bounded(arm, &cell, 17);
    }
}

/// The same scan over 5,000 cells on a 33 × 33 lattice, with every arm in every cell.
#[test]
#[ignore = "slow: 5,000 cells on a 33² lattice for 24 arms"]
fn arm_factor_bounds_hold_over_random_cells_densely() {
    let arms = extreme_arms();
    let mut lcg = Lcg::new(0x0208_b0a7);
    for i in 0..5_000 {
        let edge = EDGES[i % EDGES.len()];
        let cell = random_arm_cell(&mut lcg, arms[i % arms.len()].geometry(), edge);
        for arm in &arms {
            assert_arm_bounded(arm, &cell, 33);
        }
    }
}

/// Where a cell holds a ridge, the bound is the ridge's peak at the cell's outer radius, and a
/// cell whose range of phase just misses a ridge is bounded by its nearer end.
#[test]
fn arm_factor_bounds_follow_the_ridges() {
    let geometry = ArmGeometry::new(
        ArmCount::Two,
        Radians::from(Degrees::new(14.0)),
        LightYears::new(15_000.0),
    )
    .unwrap();
    let sharp = SharpArm::new(geometry, LightYears::new(300.0), 0.8).unwrap();
    let r = 30_000.0;
    let theta = geometry.ridge_azimuth(r, 0);
    let (x, y) = (r * math::cos(theta), r * math::sin(theta));
    let cell = cell_at([x, y, 0.0], 8);
    let radii = cell.r_cyl_range();
    let bound = Arm::Sharp(sharp)
        .across(radii)
        .sup(geometry.phase_range(&cell));
    let peak = 1.0
        + geometry.fade(radii.hi)
            * sharp.fraction()
            * (SharpArm::profile(1.0, sharp.k(radii.hi)) - 1.0);
    assert!(
        (bound / peak - 1.0).abs() < 1e-11,
        "{bound} against the peak {peak} over {cell:?}"
    );
    // Midway between the two ridges, π ÷ 2 round the circle, the range is the trough's: the
    // bound is barely above 1 − A.
    let trough_theta = theta + 0.5 * PI;
    let (x, y) = (r * math::cos(trough_theta), r * math::sin(trough_theta));
    let far = cell_at([x, y, 0.0], 8);
    let trough = Arm::Sharp(sharp)
        .across(far.r_cyl_range())
        .sup(geometry.phase_range(&far));
    assert!(
        trough < 1.0 - 0.99 * sharp.fraction(),
        "{trough} over {far:?}"
    );
    // The whole circle holds a ridge.
    let every = ScalarRange::new(f64::NEG_INFINITY, f64::INFINITY);
    assert!(Arm::Sharp(sharp).sup(radii, every) >= bound);
}

/// A disc's bound is its envelope bound times its arm factor's bound; a component without arms
/// has its envelope bound.
#[test]
fn a_disc_bound_is_envelope_times_arm() {
    let fields = seeded(PINNED[0]);
    let mut lcg = Lcg::new(0x0208_b0a8);
    for edge in EDGES {
        for _ in 0..20 {
            let cell = random_cell(&mut lcg, edge);
            for component in fields.components() {
                let expected = match component.arm() {
                    Some(arm) => {
                        component.envelope_bound(&cell)
                            * arm
                                .across(cell.r_cyl_range())
                                .sup(arm.geometry().phase_range(&cell))
                    }
                    None => component.envelope_bound(&cell),
                };
                assert_same_bits(component.bound(&cell), expected);
            }
        }
    }
}

/// The mean density of `component` over `cell`, from an `m³` grid of cell midpoints.
fn mean_density(component: &Component, cell: &CellBox, m: u32) -> f64 {
    let min = cell.min_corner().map(f64::from);
    let step = f64::from(cell.edge()) / f64::from(m);
    let mut sum = 0.0;
    for i in 0..m {
        for j in 0..m {
            for k in 0..m {
                let at = |low: f64, i: u32| low + step * (f64::from(i) + 0.5);
                sum +=
                    component.density(&PointLy::new(at(min[0], i), at(min[1], j), at(min[2], k)));
            }
        }
    }
    sum / f64::from(m * m * m)
}

/// Tightness (P02.T8.b): over 128 ly cells on the mid-plane between the bar's end and 40,000 ly,
/// the young disc's mean bound over its mean density, which is the candidates placed per system
/// kept, is under 3.5 for the fixture. The brainstorm's 2.8 was for another profile; the figure
/// is recorded in the plan (Risks, R17).
#[test]
fn the_young_disc_bound_is_tight_on_the_mid_plane() {
    let params = GalaxyParams::milky_way_like();
    let fields = fields_of(&params);
    let young = &fields.components()[0];
    assert!(matches!(young.arm(), Some(Arm::Sharp(_))));
    let (inner, outer) = (params.bar().half_length().value(), 40_000.0);
    let mut lcg = Lcg::new(0x0208_b0a9);
    let (mut bounds, mut densities) = (0.0, 0.0);
    let mut cells = 0;
    while cells < 3_000 {
        // Uniform over the annulus's area, on either side of the plane.
        let r = (inner * inner + (outer * outer - inner * inner) * lcg.next_f64()).sqrt();
        let theta = TAU * lcg.next_f64();
        let z = if lcg.next_f64() < 0.5 { 1.0 } else { -1.0 };
        let cell = cell_at([r * math::cos(theta), r * math::sin(theta), z], 128);
        let centre = cell.centre();
        let r_centre = (centre.x * centre.x + centre.y * centre.y).sqrt();
        if !(inner..=outer).contains(&r_centre) {
            continue;
        }
        bounds += young.bound(&cell);
        densities += mean_density(young, &cell, 6);
        cells += 1;
    }
    let ratio = bounds / densities;
    println!("young disc, 128 ly mid-plane cells: mean bound ÷ mean density = {ratio:.3}");
    assert!((1.0..3.5).contains(&ratio), "{ratio}");
}
