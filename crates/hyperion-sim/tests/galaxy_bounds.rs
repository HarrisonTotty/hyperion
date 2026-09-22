//! The bounds (plan 02, P02.T8): cell geometry, the envelopes' nearest-corner bounds, the arm
//! factors' bounds from a cell's ranges of radius and phase, the layers' bounds, the violation
//! hunt and the golden file.
//!
//! The checks over thousands of cells run in full as slow tests under `just test-slow`; the fast
//! suite runs the same checks on a sample a debug build can afford.

use std::collections::BTreeSet;

use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::bounds::{CellBox, ScalarRange, UnimodalFactor};
use hyperion_sim::galaxy::fields::arms::{Arm, ArmGeometry, GentleArm, SharpArm};
use hyperion_sim::galaxy::fields::{Component, Fields, MAX_COMPONENTS, Shape};
use hyperion_sim::galaxy::imf::{BandShares, MassBand, MassFunctionKind};
use hyperion_sim::galaxy::params::{ArmCount, GalaxyParams, GalaxyParamsBuilder};
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::shares::ShareMatrix;
use hyperion_sim::math;
use hyperion_sim::units::{Degrees, LightYears, Radians, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
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
        MassFunctionKind::default(),
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
/// bulge's switch between its terms, each halo component's core, break and cut, and the discs'
/// vertical tables where their segments change width, where their dispersion stops rising and at
/// their end, in every octant and touching the axis planes.
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
            Shape::Disc(disc) => {
                let l = disc.length().value();
                let reach = disc.profile().gradient_reach().value();
                let top = ROOT - 2.0 * f64::from(edge);
                for z in [
                    0.5, 1.0, 127.5, 128.0, 256.0, 4_096.0, reach, 32_768.0, 49_152.0, top,
                ] {
                    points.push([l, 0.0, z]);
                    points.push([0.0, 0.5 * l, z]);
                }
            }
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
        if corner >= f64::MIN_POSITIVE {
            assert!(
                (bound / corner - 1.0).abs() <= 1e-12,
                "component {i} in {cell:?}: bound {bound:e} against the corner's {corner:e}"
            );
        } else if corner > 0.0 {
            // A subnormal corner keeps fewer bits, and its margin rounds to the nearest unit in
            // the last place, which can exceed 10⁻¹² of it (`bounds.rs`, "Floating point"): the
            // young disc far above the plane, the bar's Gaussian end.
            let unit = f64::MIN_POSITIVE * f64::EPSILON;
            assert!(
                corner <= bound && bound - corner <= 1e-12 * corner + unit,
                "component {i} in {cell:?}: bound {bound:e} against the subnormal corner's \
                 {corner:e}"
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

/// The bulge, whose envelope can rise by a last bit, never becomes subnormal inside the root
/// cube, where the bounds' relative margin would round away: at the cube's far corner, for the
/// smallest bulge the parameters allow and both ends of its boxiness, it is a normal number far
/// above the least one.
#[test]
fn the_bulge_stays_normal_across_the_root_cube() {
    for boxiness in [3.0, 4.0] {
        let params = GalaxyParamsBuilder::new()
            .bulge_length(LightYears::new(1_700.0))
            .bulge_b_over_a(0.5)
            .bulge_c_over_a(0.3)
            .bulge_boxiness(boxiness)
            .build()
            .unwrap();
        let fields = fields_of(&params);
        let bulge = fields
            .components()
            .iter()
            .find(|c| matches!(c.shape(), Shape::Bulge(_)))
            .unwrap();
        let centre = bulge.envelope(&PointLy::default());
        let corner = bulge.envelope(&PointLy::new(ROOT, ROOT, ROOT));
        assert!(
            corner > 1e-65 * centre && corner.is_normal(),
            "{corner:e} at c∥ {boxiness}"
        );
    }
}

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

/// The bits of every component's envelope bound and bound, of `component_bounds` and of every
/// layer's bound over a few cells.
fn bound_bits(fields: &Fields, cells: &[CellBox]) -> Vec<u64> {
    let shares = default_shares();
    let mut bits = Vec::new();
    for cell in cells {
        for c in fields.components() {
            bits.extend([c.envelope_bound(cell), c.bound(cell)].map(hyperion_testkit::float::bits));
        }
        let mut bounds = [0.0; MAX_COMPONENTS];
        fields.component_bounds(cell, &mut bounds);
        bits.extend(bounds.map(hyperion_testkit::float::bits));
        for band in MassBand::ALL {
            bits.push(hyperion_testkit::float::bits(
                fields.layer_bound(&shares, band, cell),
            ));
        }
    }
    bits
}

/// The bounds are a pure function of the galaxy and the cell: the same seed gives the same bits
/// twice, and building galaxies or bounding cells in any order changes nothing (the
/// sim-determinism rule for anything a cache may hold).
#[test]
fn bounds_are_pure_and_order_independent() {
    let mut lcg = Lcg::new(0x0208_0bde);
    let cells: Vec<CellBox> = EDGES
        .iter()
        .flat_map(|&edge| [random_cell(&mut lcg, edge), random_cell(&mut lcg, edge)])
        .collect();
    let first = bound_bits(&seeded(PINNED[1]), &cells);
    let _other = seeded(PINNED[0]);
    assert_eq!(first, bound_bits(&seeded(PINNED[1]), &cells));
    assert_order_independent(&PINNED, |&seed| bound_bits(&seeded(seed), &cells));
    let fields = seeded(PINNED[2]);
    assert_order_independent(&cells, |cell| {
        bound_bits(&fields, std::slice::from_ref(cell))
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

// Layer bounds and the violation hunt (P02.T8.c).

/// The five stellar layers: cell edge and mass band.
const LAYERS: [(u32, MassBand); 5] = [
    (8, MassBand::A),
    (16, MassBand::B),
    (32, MassBand::C),
    (64, MassBand::D),
    (128, MassBand::E),
];

/// The share matrix every galaxy of the first milestone uses.
fn default_shares() -> ShareMatrix {
    ShareMatrix::uniform(&BandShares::of(
        MassFunctionKind::default().to_mass_function().as_ref(),
    ))
}

/// `layer_bound` is `Σ share × bound` over the components in order, bit for bit, and
/// `component_bounds` holds each component's `bound` and zeros past the last.
#[test]
fn layer_bounds_are_share_weighted_sums_in_component_order() {
    let shares = default_shares();
    let mut lcg = Lcg::new(0x0208_c0a1);
    for seed in PINNED {
        let fields = seeded(seed);
        for edge in EDGES {
            for _ in 0..10 {
                let cell = random_cell(&mut lcg, edge);
                let mut bounds = [f64::NAN; MAX_COMPONENTS];
                fields.component_bounds(&cell, &mut bounds);
                let count = fields.components().len();
                for (id, component) in fields.component_ids().zip(fields.components()) {
                    let bound = bounds[id.index()];
                    assert_same_bits(bound, component.bound(&cell));
                    assert_same_bits(bound, fields.component_bound(id, &cell));
                }
                for &unused in &bounds[count..] {
                    assert_same_bits(unused, 0.0);
                }
                for band in MassBand::ALL {
                    let expected = fields
                        .components()
                        .iter()
                        .zip(bounds)
                        .fold(0.0, |sum, (c, b)| sum + shares.component_share(band, c) * b);
                    assert_same_bits(fields.layer_bound(&shares, band, &cell), expected);
                }
            }
        }
    }
}

/// The directions of the hunt's pattern search: the three axes and the plane's two diagonals, both
/// ways, since arm ridges run obliquely through a cell.
const DIRECTIONS: [[f64; 3]; 10] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
    [1.0, 1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, 1.0, 0.0],
];

/// A search for the greatest value of a function over a cell.
struct Search<'a> {
    cell: &'a CellBox,
    lo: [f64; 3],
    hi: [f64; 3],
}

impl<'a> Search<'a> {
    fn new(cell: &'a CellBox) -> Self {
        let lo = cell.min_corner().map(f64::from);
        let hi = lo.map(|low| low + f64::from(cell.edge()));
        Self { cell, lo, hi }
    }

    /// An `m³` lattice of the cell, corners included.
    fn lattice(&self, m: u32) -> Vec<PointLy> {
        let edge = f64::from(self.cell.edge());
        let steps = f64::from(m - 1);
        let mut points = Vec::with_capacity(usize::try_from(m * m * m).expect("a small lattice"));
        for i in 0..m {
            for j in 0..m {
                for k in 0..m {
                    let at = |low: f64, i: u32| low + edge * f64::from(i) / steps;
                    points.push(PointLy::new(
                        at(self.lo[0], i),
                        at(self.lo[1], j),
                        at(self.lo[2], k),
                    ));
                }
            }
        }
        points
    }

    /// Compass search from `start`: move to any better point one step away along
    /// [`DIRECTIONS`], clamped to the cell, and quarter the step when none is better, down to a
    /// millionth of the edge.
    fn ascend(&self, start: PointLy, step: f64, f: &impl Fn(&PointLy) -> f64) -> (f64, PointLy) {
        let floor = f64::from(self.cell.edge()) / 1_048_576.0;
        let mut at = [start.x, start.y, start.z];
        let mut best = f(&start);
        let mut step = step;
        while step > floor {
            let mut moved = false;
            for d in DIRECTIONS {
                let q: [f64; 3] =
                    std::array::from_fn(|a| (at[a] + step * d[a]).clamp(self.lo[a], self.hi[a]));
                let value = f(&PointLy::new(q[0], q[1], q[2]));
                if value > best {
                    best = value;
                    at = q;
                    moved = true;
                }
            }
            if !moved {
                step *= 0.25;
            }
        }
        (best, PointLy::new(at[0], at[1], at[2]))
    }

    /// The greatest value of `f` found by an `m³` lattice refined by compass search from the
    /// lattice's best point and from the nearest corner.
    fn maximise(&self, m: u32, f: &impl Fn(&PointLy) -> f64) -> (f64, PointLy) {
        let best = self
            .lattice(m)
            .into_iter()
            .map(|p| (f(&p), p))
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .expect("a lattice has points");
        let step = 0.5 * f64::from(self.cell.edge()) / f64::from(m - 1);
        [best.1, self.cell.nearest_corner()]
            .into_iter()
            .map(|start| self.ascend(start, step, f))
            .fold(best, |a, b| if b.0 > a.0 { b } else { a })
    }
}

/// The greatest density ÷ bound the hunt found, per quantity.
#[derive(Debug, Default)]
struct Worst {
    young: f64,
    sub_disc: f64,
    layer: f64,
    other: f64,
    cells: usize,
}

impl Worst {
    fn note(slot: &mut f64, value: f64, bound: f64) {
        if bound > 0.0 {
            *slot = slot.max(value / bound);
        }
    }
}

/// Which quantities a cell's search maximises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effort {
    /// The arms: the young disc, the youngest sub-disc, and the layer at a lattice and the arms'
    /// maxima.
    Arms,
    /// The arms, and the layer refined by compass search.
    ArmsAndLayer,
    /// Every component and the layer, each refined by compass search.
    Everything,
}

/// Hunts `cell` of the layer of `band` for a point where a density exceeds its bound, and panics
/// with the point if it finds one.
fn hunt_cell(
    fields: &Fields,
    shares: &ShareMatrix,
    band: MassBand,
    cell: &CellBox,
    effort: Effort,
    worst: &mut Worst,
) {
    let search = Search::new(cell);
    let mut bounds = [0.0; MAX_COMPONENTS];
    fields.component_bounds(cell, &mut bounds);
    let mut maxima = vec![cell.nearest_corner()];
    for (i, component) in fields.components().iter().enumerate() {
        let (slot, lattice) = match (i, effort) {
            (0, _) => (&mut worst.young, 5),
            (1, _) => (&mut worst.sub_disc, 3),
            (_, Effort::Everything) => (&mut worst.other, 3),
            (_, Effort::Arms | Effort::ArmsAndLayer) => continue,
        };
        let (value, at) = search.maximise(lattice, &|p| component.density(p));
        assert!(
            value <= bounds[i],
            "component {i}: {value:e} at {at:?} exceeds the bound {:e} over {cell:?}",
            bounds[i]
        );
        Worst::note(slot, value, bounds[i]);
        maxima.push(at);
    }
    let bound = fields.layer_bound(shares, band, cell);
    let layer = |p: &PointLy| fields.layer_density(shares, band, p);
    let (value, at) = if effort == Effort::Arms {
        maxima
            .into_iter()
            .chain(search.lattice(3))
            .map(|p| (layer(&p), p))
            .fold(
                (0.0, cell.nearest_corner()),
                |a, b| if b.0 > a.0 { b } else { a },
            )
    } else {
        search.maximise(3, &layer)
    };
    assert!(
        value <= bound,
        "layer {band:?}: {value:e} at {at:?} exceeds the bound {bound:e} over {cell:?}"
    );
    Worst::note(&mut worst.layer, value, bound);
    worst.cells += 1;
}

/// The cells of `edge` that the arms' ridges cross between 0.8 and 2 bar half-lengths, walked in
/// steps of 16 ly along each ridge, with their eight neighbours in the plane, on the side z ≥ 0
/// (the densities and bounds are the same bits below it).
fn ridge_cells(arms: &ArmGeometry, edge: u32) -> BTreeSet<[i32; 3]> {
    let l = arms.bar_half_length().value();
    // Steps of 16 ly of arc, or of one cell where cells are smaller, so that two steps never skip a
    // cell's neighbour; along a logarithmic spiral ds = dR ÷ sin p.
    let step = 16.0_f64.min(f64::from(edge)) * math::sin(arms.pitch().value());
    let size = i32::try_from(edge).expect("a layer's edge");
    let mut cells = BTreeSet::new();
    for ridge in 0..arms.count().get() {
        let mut r = 0.8 * l;
        while r <= 2.0 * l {
            let theta = arms.ridge_azimuth(r, ridge);
            let [x, y, _] =
                cell_at([r * math::cos(theta), r * math::sin(theta), 0.0], edge).min_corner();
            for dx in [-size, 0, size] {
                for dy in [-size, 0, size] {
                    cells.insert([x + dx, y + dy, 0]);
                }
            }
            r += step;
        }
    }
    cells
}

/// A random cell of `edge` in the central 1,000 ly.
fn central_cell(lcg: &mut Lcg, edge: u32) -> CellBox {
    let p = [0; 3].map(|_| 1_000.0 * (2.0 * lcg.next_f64() - 1.0));
    cell_at(p, edge)
}

/// The hunt of P02.T8.c over one galaxy: every `stride`-th ridge cell of every layer, the
/// targeted cells, and `random` random cells per layer, half of them in the central 1,000 ly.
/// Returns the worst ratios per layer.
fn hunt(params: &GalaxyParams, stride: usize, random: usize, seed: u64) -> Vec<Worst> {
    let fields = fields_of(params);
    let shares = default_shares();
    let mut lcg = Lcg::new(0x0208_c0a2 ^ seed);
    let mut report = Vec::new();
    for (edge, band) in LAYERS {
        let mut worst = Worst::default();
        for min in ridge_cells(fields.arms(), edge).into_iter().step_by(stride) {
            let cell = CellBox::new(min, edge).expect("a ridge cell");
            hunt_cell(&fields, &shares, band, &cell, Effort::Arms, &mut worst);
        }
        for cell in targeted_cells(&fields, edge) {
            hunt_cell(
                &fields,
                &shares,
                band,
                &cell,
                Effort::Everything,
                &mut worst,
            );
        }
        for i in 0..random {
            let cell = if i % 2 == 0 {
                random_cell(&mut lcg, edge)
            } else {
                central_cell(&mut lcg, edge)
            };
            hunt_cell(
                &fields,
                &shares,
                band,
                &cell,
                Effort::ArmsAndLayer,
                &mut worst,
            );
        }
        report.push(worst);
    }
    report
}

// Golden values (P02.T8.c).

/// Ten places, each pinned as the cell of every stellar layer that holds it: fifty cells. The
/// centre on both sides of every plane, the bulge, the bar and its end, the solar circle on and
/// off the arms, above the disc, the outer halo across the bar, where the bar's Gaussian is
/// subnormal and the bound's margin rounds away, and the halo's cut.
const GOLDEN_POINTS: [[f64; 3]; 10] = [
    [0.0, 0.0, 0.0],
    [-0.5, -0.5, -0.5],
    [900.0, 400.0, 150.0],
    [13_600.0, 0.0, 0.0],
    [16_000.0, 300.0, 40.0],
    [22_516.7, 13_000.0, 50.0],
    [-6_000.0, -25_300.0, 5.0],
    [0.0, 26_000.0, 1_500.0],
    [0.0, 61_000.0, 0.0],
    [45_952.0, 45_952.0, 0.0],
];

/// The fifty pinned cells, with the band of the layer each belongs to.
fn golden_cells() -> Vec<(CellBox, MassBand)> {
    LAYERS
        .iter()
        .flat_map(|&(edge, band)| GOLDEN_POINTS.iter().map(move |&p| (cell_at(p, edge), band)))
        .collect()
}

/// A cell's label: its low corner and edge.
fn cell_label(cell: &CellBox) -> String {
    let [x, y, z] = cell.min_corner();
    format!("({x}, {y}, {z}; {})", cell.edge())
}

/// Every component's bound, the young disc's envelope bound and range of phase, and the layer
/// bound, over each pinned cell of the fixture.
fn write_fixture_bounds(w: &mut GoldenWriter) {
    let fields = fields_of(&GalaxyParams::milky_way_like());
    let shares = default_shares();
    let young = &fields.components()[0];
    for (cell, band) in golden_cells() {
        let label = format!("milky_way{}", cell_label(&cell));
        let mut bounds = [0.0; MAX_COMPONENTS];
        fields.component_bounds(&cell, &mut bounds);
        for (i, bound) in bounds[..fields.components().len()].iter().enumerate() {
            w.f64(&format!("{label}.component[{i}]"), *bound);
        }
        w.f64(
            &format!("{label}.young_envelope"),
            young.envelope_bound(&cell),
        );
        let radii = cell.r_cyl_range();
        w.f64(&format!("{label}.r_cyl.lo"), radii.lo);
        w.f64(&format!("{label}.r_cyl.hi"), radii.hi);
        let phase = fields.arms().phase_range(&cell);
        w.f64(&format!("{label}.phase.lo"), phase.lo);
        w.f64(&format!("{label}.phase.hi"), phase.hi);
        w.f64(
            &format!("{label}.layer_{band:?}"),
            fields.layer_bound(&shares, band, &cell),
        );
    }
}

/// The young disc's, the youngest sub-disc's and the layer's bound over each pinned cell of a
/// seed.
fn write_seed_bounds(w: &mut GoldenWriter, seed: Seed) {
    let fields = fields_of(&GalaxyParams::from_seed(seed, MassFunctionKind::default()));
    let shares = default_shares();
    for (cell, band) in golden_cells() {
        let label = format!("{seed}{}", cell_label(&cell));
        w.f64(
            &format!("{label}.young"),
            fields.components()[0].bound(&cell),
        );
        w.f64(
            &format!("{label}.sub_disc_1"),
            fields.components()[1].bound(&cell),
        );
        w.f64(
            &format!("{label}.layer_{band:?}"),
            fields.layer_bound(&shares, band, &cell),
        );
    }
}

/// The bounds of fifty pinned cells, bit for bit (P02.T8.c): every component of the fixture,
/// and the arms and layers of the three pinned seeds.
#[test]
fn galaxy_bounds_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("# milky_way");
    write_fixture_bounds(&mut w);
    for seed in PINNED {
        let seed = Seed::new(seed);
        w.line(&format!("# {seed}"));
        write_seed_bounds(&mut w, seed);
    }
    golden!("galaxy_bounds", w.as_str());
}

/// The hunt's five seeds (P02.T8.c), found by scanning the first 1,500 seeds of the family
/// `0x0208_4a47_0000_0000 | n`, with the property each was chosen for.
const HUNT_SEEDS: [(&str, u64); 5] = [
    ("sharpest_arms", 0x0208_4a47_0000_0368),
    ("sharpest_four_arms", 0x0208_4a47_0000_00f0),
    ("longest_bar", 0x0208_4a47_0000_05ce),
    ("shortest_bar", 0x0208_4a47_0000_05bb),
    ("tightest_four_arms", 0x0208_4a47_0000_0086),
];

/// The seed's parameters, checked to still have the property it was chosen for: a change to the
/// parameter draws that moves them makes this fail rather than quietly weaken the hunt.
fn hunt_params(name: &str, seed: u64) -> GalaxyParams {
    let params = GalaxyParams::from_seed(Seed::new(seed), MassFunctionKind::default());
    let arms = params.arms();
    let pitch = Degrees::from(arms.pitch()).value();
    let width = arms.young_width().value();
    let bar = params.bar().half_length().value();
    let holds = match name {
        "sharpest_arms" => arms.count() == ArmCount::Two && pitch > 17.5 && width < 260.0,
        "sharpest_four_arms" => arms.count() == ArmCount::Four && pitch > 17.5 && width < 260.0,
        "longest_bar" => bar >= 17_999.0 && arms.count() == ArmCount::Two,
        "shortest_bar" => bar <= 10_001.0 && arms.count() == ArmCount::Four,
        "tightest_four_arms" => arms.count() == ArmCount::Four && pitch < 10.1 && bar >= 17_999.0,
        _ => false,
    };
    assert!(
        holds,
        "{name}: {:?}, {pitch}°, {width} ly, bar {bar} ly",
        arms.count()
    );
    params
}

/// Galaxies with parameters at the edges of their ranges: the sharpest two arms on the shortest
/// bar, four tightly wound sharp arms, and the densest centre, with the smallest bulge, nuclear
/// disc, bar and halo cores, the nearest halo break and its steepest slope.
fn edge_params(name: &str) -> GalaxyParams {
    let builder = GalaxyParamsBuilder::new();
    let builder = match name {
        "edge_sharp_two" => builder
            .arm_count(ArmCount::Two)
            .arm_pitch(Degrees::new(18.0))
            .arm_young_width(LightYears::new(250.0))
            .arm_young_fraction(0.9)
            .arm_old_amplitude(0.3)
            .bar_half_length(LightYears::new(10_000.0))
            .young_height(LightYears::new(130.0)),
        "edge_tight_four" => builder
            .arm_count(ArmCount::Four)
            .arm_pitch(Degrees::new(10.0))
            .arm_young_width(LightYears::new(250.0))
            .arm_young_fraction(0.9)
            .arm_old_amplitude(0.3)
            .bar_half_length(LightYears::new(10_000.0))
            .young_height(LightYears::new(130.0)),
        "edge_dense_centre" => builder
            .bulge_length(LightYears::new(1_700.0))
            .bulge_b_over_a(0.5)
            .bulge_c_over_a(0.3)
            .bulge_boxiness(3.0)
            .nuclear_length(LightYears::new(200.0))
            .nuclear_height_ratio(0.3)
            .bar_half_length(LightYears::new(10_000.0))
            .bar_width_ratio(0.08)
            .bar_height(LightYears::new(500.0))
            .halo_in_situ(0.3, 0.45, LightYears::new(1_500.0), 2.8, Years::new(12.0e9))
            .halo_dominant(0.6, 0.6, LightYears::new(2_000.0), 2.8, Years::new(12.0e9))
            .halo_dominant_break_radius(LightYears::new(52_000.0))
            .halo_dominant_break_steepening(2.5),
        _ => panic!("no edge galaxy {name}"),
    };
    builder.build().expect("every value inside its range")
}

/// Runs the full hunt over one galaxy and prints the worst ratio found per layer.
fn hunt_and_report(name: &str, params: &GalaxyParams, random: usize, seed: u64) {
    for ((edge, _), worst) in LAYERS.iter().zip(hunt(params, 1, random, seed)) {
        println!(
            "{name}, {edge} ly: {} cells; density ÷ bound at most {:.12} (young), {:.12} \
             (sub-disc), {:.12} (other components), {:.12} (layer)",
            worst.cells, worst.young, worst.sub_disc, worst.other, worst.layer
        );
    }
}

/// A sample of the hunt on the fixture: one ridge cell in 40, the targeted cells, and 6 random
/// cells per layer. The whole hunt runs as slow tests.
#[test]
fn bounds_hold_in_a_sample_of_the_hunt() {
    let report = hunt(&GalaxyParams::milky_way_like(), 40, 6, 0);
    for worst in report {
        assert!(worst.cells > 0);
        assert!(worst.young > 0.9 && worst.layer > 0.9, "{worst:?}");
    }
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_the_sharpest_arms() {
    let (name, seed) = HUNT_SEEDS[0];
    hunt_and_report(name, &hunt_params(name, seed), 10_000, seed);
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_the_sharpest_four_arms() {
    let (name, seed) = HUNT_SEEDS[1];
    hunt_and_report(name, &hunt_params(name, seed), 10_000, seed);
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_the_longest_bar() {
    let (name, seed) = HUNT_SEEDS[2];
    hunt_and_report(name, &hunt_params(name, seed), 10_000, seed);
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_the_shortest_bar() {
    let (name, seed) = HUNT_SEEDS[3];
    hunt_and_report(name, &hunt_params(name, seed), 10_000, seed);
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_the_tightest_four_arms() {
    let (name, seed) = HUNT_SEEDS[4];
    hunt_and_report(name, &hunt_params(name, seed), 10_000, seed);
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_sharp_arms_on_the_shortest_bar() {
    let name = "edge_sharp_two";
    hunt_and_report(name, &edge_params(name), 2_000, 1);
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_four_tight_sharp_arms() {
    let name = "edge_tight_four";
    hunt_and_report(name, &edge_params(name), 2_000, 2);
}

#[test]
#[ignore = "slow: the violation hunt over one galaxy"]
fn bounds_hold_in_the_hunt_with_the_densest_centre() {
    let name = "edge_dense_centre";
    hunt_and_report(name, &edge_params(name), 2_000, 3);
}
