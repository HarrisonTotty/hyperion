//! The solver's tests, on river networks built by hand: the plan's (a ridge-to-sea profile against
//! the closed-form steady state; a profile's lowering far from base level linear in t and in
//! runoff^0.45; the saturated regolith's K t at the reference Mars's epoch), the lane's (bounds
//! and cells of no equivalent area unchanged, a bound a base level for its donors, no cell without
//! uplift raised, and the elevation linear in the uplift's scale to the bit), and the threshold
//! slope's (decision-r09-t14-choices, "T14.c follow-up" item 2 and "T14.c follow-up 2" §5: the
//! ridge-to-sea profile held at it near its ridge, a dry headwater held at it, and none off the
//! steady state).

use hyperion_surface::field::Crust;
use hyperion_surface::synth::channels::DEBRIS_FLOW_SLOPE;
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;

use super::super::branches::wet_now_threshold;
use super::super::erodibility::{EARTH_ERODIBILITY_M0_1_PER_YEAR, Erodibility};
use super::super::solver::{
    Cells, DRAINAGE_EXPONENT, Network, Solution, Threshold, Time, solve, solve_with,
};
use crate::math;
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::plates::{cell_of, slot};

/// Tzathas et al. 2024's uplift, m a⁻¹: 10⁻³ (CGF 43(2), Table 1).
const UPLIFT_M_PER_YEAR: f64 = 1.0e-3;

/// Hack's law's constant for the ridge-to-sea profile, L = C A^h with lengths in metres: 0.32, the
/// lane design's §3.4 profile (a hand-built test profile, not a calibration).
const HACK_CONSTANT: f64 = 0.32;

/// Hack's law's exponent: 0.6 (Hack 1957; Tzathas et al. 2024, Table 1).
const HACK_EXPONENT: f64 = 0.6;

/// A river network built by hand: each cell's receiver and link length, its slope correction, and
/// the depth-first order from the roots in ascending order, each cell's donors in ascending order.
struct Tree {
    downstream: Vec<Option<(u32, f64)>>,
    correction: Vec<f64>,
    preorder: Vec<u32>,
    depth: Vec<u32>,
}

impl Tree {
    /// The network of `downstream`, each receiver's index below its donor's or not, with no slope
    /// correction (1 everywhere).
    fn new(downstream: Vec<Option<(u32, f64)>>) -> Self {
        let n = downstream.len();
        let mut donors: Vec<Vec<u32>> = vec![Vec::new(); n];
        for (c, link) in downstream.iter().enumerate() {
            if let Some((r, _)) = link {
                donors[slot(*r)].push(cell_of(c));
            }
        }
        let mut preorder = Vec::with_capacity(n);
        let mut depth = vec![0; n];
        for root in (0..n).filter(|&c| downstream[c].is_none()) {
            let mut stack = vec![cell_of(root)];
            while let Some(c) = stack.pop() {
                preorder.push(c);
                for &d in donors[slot(c)].iter().rev() {
                    depth[slot(d)] = depth[slot(c)] + 1;
                    stack.push(d);
                }
            }
        }
        assert_eq!(preorder.len(), n, "the receivers are a forest");
        Self {
            downstream,
            correction: vec![1.0; n],
            preorder,
            depth,
        }
    }

    /// A 1-D chain of `n` cells over links of `link_m`: cell 0 the root at the bound, cell i
    /// draining to cell i − 1.
    fn chain(n: usize, link_m: f64) -> Self {
        Self::new(
            (0..n)
                .map(|i| i.checked_sub(1).map(|r| (cell_of(r), link_m)))
                .collect(),
        )
    }
}

impl Network for Tree {
    fn len(&self) -> u32 {
        cell_of(self.downstream.len())
    }

    fn preorder(&self) -> &[u32] {
        &self.preorder
    }

    fn depth(&self, cell: u32) -> u32 {
        self.depth[slot(cell)]
    }

    fn downstream(&self, cell: u32) -> Option<(u32, f64)> {
        self.downstream[slot(cell)]
    }

    fn correction(&self, cell: u32) -> f64 {
        self.correction[slot(cell)]
    }
}

/// A solve's per-cell inputs, owned.
#[derive(Debug, Clone)]
struct Inputs {
    z0_m: Vec<f64>,
    uplift_m_per_year: Vec<f64>,
    erodibility_m0_1_per_year: Vec<f64>,
    equivalent_area_m2: Vec<f64>,
    fixed: Vec<Option<f64>>,
}

impl Inputs {
    /// `n` cells at z0 0, uplift `uplift`, erodibility `k` and equivalent area `area_m2`, none
    /// fixed.
    fn uniform(n: usize, uplift: f64, k: f64, area_m2: f64) -> Self {
        Self {
            z0_m: vec![0.0; n],
            uplift_m_per_year: vec![uplift; n],
            erodibility_m0_1_per_year: vec![k; n],
            equivalent_area_m2: vec![area_m2; n],
            fixed: vec![None; n],
        }
    }

    fn cells(&self) -> Cells<'_> {
        Cells {
            z0_m: &self.z0_m,
            uplift_m_per_year: &self.uplift_m_per_year,
            erodibility_m0_1_per_year: &self.erodibility_m0_1_per_year,
            equivalent_area_m2: &self.equivalent_area_m2,
            fixed: &self.fixed,
        }
    }

    fn solve(&self, network: &Tree, time: Time) -> Solution {
        solve(network, &self.cells(), time)
    }

    /// The steady state on `network` held to the wet-now branch's threshold slope.
    fn solve_held(&self, network: &Tree) -> Solution {
        solve_with(network, &self.cells(), Time::Steady, wet_now_threshold())
    }
}

/// `x` cells as an `f64`.
fn real(x: usize) -> f64 {
    f64::from(u32::try_from(x).unwrap())
}

/// A ridge-to-sea profile matches the closed-form steady state under the threshold slope
/// (decision-r09-t14-choices, "T14.c follow-up 2" §5, restating T14.b's test): on a chain of 2,000
/// links of 50 m from a sea cell (a bound at the sea's level, 0 m, its ground at −50 m) up to a
/// ridge at L = 100.05 km, the area following Hack's law, A(x) = ((L − x) ÷ 0.32)^(1/0.6), under
/// uniform uplift (10⁻³ m a⁻¹) and erodibility (K⊕), held to [`DEBRIS_FLOW_SLOPE`]:
///
/// - the steady state is the piecewise-constant sum from the sea, to the bit, of each link's rise:
///   u δ ÷ (K A^m) where that is below 0.1 δ, so that every cell below the first link held equals
///   the threshold-free solve's bits, and exactly 0.1 δ where the threshold holds it;
/// - each link is held exactly where its fluvial slope passes 0.1, the last 5.9 km before the
///   ridge (from x = 94.15 km);
/// - the profile lies within 1% of ∫₀^x u ÷ (K A(s)^m) ds = (u ÷ K) C^(m/h) (L^(1 − m/h) − (L −
///   x)^(1 − m/h)) ÷ (1 − m/h) wherever x ≤ 0.9 L, below the first link held;
/// - the sea cell keeps its ground.
///
/// The remap onto the construction's hypsometry is the wet-now branch's, so it does not apply
/// here.
#[test]
fn a_ridge_to_sea_profile_matches_the_closed_form_steady_state() {
    let (tree, inputs, cells, delta) = ridge_to_sea();
    let ridge = real(cells) * delta;
    let area = |x: f64| math::powf((ridge - x) / HACK_CONSTANT, 1.0 / HACK_EXPONENT);
    let erodibility = EARTH_ERODIBILITY_M0_1_PER_YEAR;
    let free = inputs.solve(&tree, Time::Steady);
    let solution = inputs.solve_held(&tree);
    assert_same_bits(solution.elevation_m(0, 1.0), -50.0);
    let power = DRAINAGE_EXPONENT / HACK_EXPONENT;
    let scale = UPLIFT_M_PER_YEAR / erodibility * math::powf(HACK_CONSTANT, power);
    let cap = DEBRIS_FLOW_SLOPE * delta;
    let (mut sum, mut worst, mut first_held) = (0.0, 0.0_f64, None);
    for i in 1..cells {
        let x = real(i) * delta;
        let fluvial = UPLIFT_M_PER_YEAR
            * (delta * (1.0 / (erodibility * math::powf(area(x), DRAINAGE_EXPONENT))));
        let held = fluvial > cap;
        assert_eq!(solution.held_at_threshold()[i], held, "cell {i}");
        if held {
            first_held.get_or_insert(i);
        }
        sum += if held { cap } else { fluvial };
        let height = solution.elevation_m(cell_of(i), 1.0);
        assert_same_bits(height, 0.0 + 1.0 * sum);
        if first_held.is_none() {
            assert_same_bits(height, free.elevation_m(cell_of(i), 1.0));
        }
        if x <= 0.9 * ridge {
            assert!(
                first_held.is_none(),
                "a link below 0.9 L is held, at cell {i}"
            );
            let closed = scale
                * (math::powf(ridge, 1.0 - power) - math::powf(ridge - x, 1.0 - power))
                / (1.0 - power);
            worst = worst.max((height / closed - 1.0).abs());
        }
    }
    let first = first_held.expect("the profile steepens past the threshold at its ridge");
    eprintln!(
        "the steady profile rises {:.1} m to the ridge held, {:.1} m free; the threshold first \
         holds at cell {first} of {cells}, x = {:.0} m; worst error against the closed form below \
         0.9 L: {:.3}%",
        solution.elevation_m(cell_of(cells - 1), 1.0),
        free.elevation_m(cell_of(cells - 1), 1.0),
        real(first) * delta,
        100.0 * worst
    );
    assert!(worst <= 0.01, "{worst}");
}

/// The ridge-to-sea profile's chain, inputs, cell count and link, metres: 2,000 links of 50 m from
/// a sea cell (a bound at 0 m, its ground at −50 m) up to a ridge at L = 100.05 km, the area
/// following Hack's law, A(x) = ((L − x) ÷ 0.32)^(1/0.6), under uniform uplift (10⁻³ m a⁻¹) and
/// erodibility (K⊕).
fn ridge_to_sea() -> (Tree, Inputs, usize, f64) {
    let (cells, delta) = (2_001, 50.0);
    let ridge = real(cells) * delta;
    let tree = Tree::chain(cells, delta);
    let area = |x: f64| math::powf((ridge - x) / HACK_CONSTANT, 1.0 / HACK_EXPONENT);
    let mut inputs = Inputs::uniform(
        cells,
        UPLIFT_M_PER_YEAR,
        EARTH_ERODIBILITY_M0_1_PER_YEAR,
        0.0,
    );
    for (i, a) in inputs.equivalent_area_m2.iter_mut().enumerate() {
        *a = area(real(i) * delta);
    }
    inputs.z0_m[0] = -50.0;
    inputs.fixed[0] = Some(0.0);
    (tree, inputs, cells, delta)
}

/// A dry headwater cell rises exactly the threshold slope times its link (decision-r09-t14-choices,
/// "T14.c follow-up" item 2): on a chain of a sea cell (a bound at 0 m), a cell of 10⁹ m² of
/// equivalent area and a headwater of 10⁴ m² (a cell of 10³ km² shedding 10⁻⁵ m a⁻¹), links of 20
/// km, uplift 10⁻³ m a⁻¹ and K⊕, held to [`DEBRIS_FLOW_SLOPE`], the headwater's fluvial slope u ÷
/// (K A^m), 2.52, is held and its F is its receiver's plus exactly 0.1 × 20 km (bits), while the
/// wet cell's, 0.014, is not and keeps the threshold-free bits.
#[test]
fn a_dry_headwater_rises_exactly_the_threshold_slope() {
    let delta = 20_000.0;
    let tree = Tree::chain(3, delta);
    let mut inputs = Inputs::uniform(3, UPLIFT_M_PER_YEAR, EARTH_ERODIBILITY_M0_1_PER_YEAR, 1.0e9);
    inputs.fixed[0] = Some(0.0);
    inputs.equivalent_area_m2[2] = 1.0e4;
    let free = inputs.solve(&tree, Time::Steady);
    let held = inputs.solve_held(&tree);
    let slope = |a: f64| {
        UPLIFT_M_PER_YEAR / (EARTH_ERODIBILITY_M0_1_PER_YEAR * math::powf(a, DRAINAGE_EXPONENT))
    };
    eprintln!(
        "fluvial slopes: the wet cell {:.4}, the headwater {:.3}",
        slope(1.0e9),
        slope(1.0e4)
    );
    assert!(slope(1.0e9) < DEBRIS_FLOW_SLOPE && slope(1.0e4) > DEBRIS_FLOW_SLOPE);
    assert_eq!(held.held_at_threshold(), &[false, false, true]);
    assert_same_bits(held.per_lambda_m()[1], free.per_lambda_m()[1]);
    assert_same_bits(
        held.per_lambda_m()[2],
        held.per_lambda_m()[1] + DEBRIS_FLOW_SLOPE * delta,
    );
    assert_same_bits(held.base_m()[2], 0.0);
}

/// A threshold slope holds at steady state only: a solve after 10⁴ years with one panics.
#[test]
#[should_panic(expected = "a threshold slope holds at steady state")]
fn a_threshold_slope_off_the_steady_state_is_refused() {
    let tree = Tree::chain(3, 1_000.0);
    let inputs = Inputs::uniform(3, 0.0, EARTH_ERODIBILITY_M0_1_PER_YEAR, 1.0e8);
    let _ = solve_with(
        &tree,
        &inputs.cells(),
        Time::Years(1.0e4),
        Threshold::Slope(DEBRIS_FLOW_SLOPE),
    );
}

/// The lowering at cell `cell` of the ramp of slope `slope` (z0 = slope × x on a chain of 1 km
/// links, cell 0 at the bound) with no uplift, erodibility `k` and a uniform geometric area of
/// `area_m2` under runoff `runoff`, after `t` years: its z0 less its elevation.
fn ramp_lowering(slope: f64, k: f64, area_m2: f64, runoff: f64, t: f64) -> Vec<f64> {
    let (n, delta) = (201, 1_000.0);
    let tree = Tree::chain(n, delta);
    let mut inputs = Inputs::uniform(n, 0.0, k, runoff * area_m2);
    for (i, z) in inputs.z0_m.iter_mut().enumerate() {
        *z = slope * real(i) * delta;
    }
    let solution = inputs.solve(&tree, Time::Years(t));
    (0..n)
        .map(|i| inputs.z0_m[i] - solution.elevation_m(cell_of(i), 0.0))
        .collect()
}

/// Over a step short enough that slopes barely change, a profile's lowering far from base level is
/// linear in t and in runoff^0.45 (to 1%): on a ramp of slope 0.01 with no uplift, a uniform
/// geometric area of 10⁹ m² and K = 2.4 × 10⁻⁵ m^0.1 a⁻¹, every cell beyond 50 km of the bound
/// lowers by S₀ K (r A)^m t (the knickpoint's retreat times the slope) at 10⁴ and 2 × 10⁴ years
/// and at 0.1 and 0.2 m a⁻¹, so doubling t doubles it and doubling the runoff multiplies it by
/// 2^0.45.
#[test]
fn a_profile_s_lowering_is_linear_in_time_and_in_runoff_to_the_m() {
    let (slope, k, area) = (0.01, 2.4e-5, 1.0e9);
    let expected =
        |runoff: f64, t: f64| slope * k * math::powf(runoff * area, DRAINAGE_EXPONENT) * t;
    let far = 50;
    let mut lowering = Vec::new();
    for (runoff, t) in [(0.1, 1.0e4), (0.1, 2.0e4), (0.2, 1.0e4)] {
        let profile = ramp_lowering(slope, k, area, runoff, t);
        let want = expected(runoff, t);
        for (i, &dz) in profile.iter().enumerate().skip(far) {
            assert!(
                (dz / want - 1.0).abs() <= 0.01,
                "cell {i} at {runoff} m a⁻¹ after {t} yr: {dz} m against {want} m"
            );
        }
        eprintln!(
            "{runoff} m a⁻¹ for {t} yr: {} m against {want} m",
            profile[far]
        );
        lowering.push(profile[far]);
    }
    let in_time = lowering[1] / lowering[0];
    let in_runoff = lowering[2] / lowering[0];
    assert!((in_time / 2.0 - 1.0).abs() <= 0.01, "{in_time}");
    assert!(
        (in_runoff / math::powf(2.0, DRAINAGE_EXPONENT) - 1.0).abs() <= 0.01,
        "{in_runoff}"
    );
}

/// The saturated regolith's K at the reference Mars's epoch gives K (runoff ÷ 1 m a⁻¹)^0.45 t =
/// 1.9 m^0.1 to 2% (Black et al. 2017's, Design note 9): the reference Mars's lid K, its epoch's
/// 0.1 m a⁻¹ and 2.2 × 10⁵ years, measured as the solver's lowering of a ramp (slope 0.01, a
/// geometric area of 10⁸ m²) over S₀ A^0.45 beyond 100 km of the bound, and computed directly.
#[test]
fn the_saturated_regolith_s_k_t_at_the_mars_epoch_is_black_et_al_s() {
    let mars = reference::mars_like();
    let epoch = *mars.wet_epoch().unwrap();
    let k = Erodibility::of(&mars).unwrap().of_crust(Crust::Lid);
    let (runoff, t) = (epoch.runoff_m_per_year, epoch.effective_flow.value());
    assert_same_bits(runoff, 0.1);
    assert_same_bits(t, 2.2e5);
    let direct = k * math::powf(runoff, DRAINAGE_EXPONENT) * t;
    let (slope, area) = (0.01, 1.0e8);
    let profile = ramp_lowering(slope, k, area, runoff, t);
    let measured = profile[100] / (slope * math::powf(area, DRAINAGE_EXPONENT));
    eprintln!(
        "K = {k:.4e} m^0.1 a⁻¹; K r^0.45 t = {direct:.4} m^0.1, from the solver's lowering \
         ({:.2} m) {measured:.4}",
        profile[100]
    );
    for value in [direct, measured] {
        assert!((value / 1.9 - 1.0).abs() <= 0.02, "{value} m^0.1");
    }
    assert!((measured / direct - 1.0).abs() < 1e-9);
}

/// The test tree of the bounds: a sea cell 0 (a bound at 0 m, its ground −100 m) ← 1 ← 2, a lake's
/// bed (a bound at 300 m, its ground 280 m) ← 3 ← 4, with 7 also draining into the lake, and 5 ← 6
/// draining to 1 with no equivalent area; links of 1 km, uniform uplift 10⁻³ m a⁻¹, K⊕ and 10⁸ m².
fn bounded_tree() -> (Tree, Inputs) {
    let delta = 1_000.0;
    let receivers = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(3),
        Some(1),
        Some(5),
        Some(2),
    ];
    let tree = Tree::new(
        receivers
            .iter()
            .map(|r| r.map(|r: u32| (r, delta)))
            .collect(),
    );
    let mut inputs = Inputs::uniform(
        receivers.len(),
        UPLIFT_M_PER_YEAR,
        EARTH_ERODIBILITY_M0_1_PER_YEAR,
        1.0e8,
    );
    inputs.z0_m = vec![-100.0, 50.0, 280.0, 400.0, 500.0, 700.0, 900.0, 450.0];
    inputs.fixed[0] = Some(0.0);
    inputs.fixed[2] = Some(300.0);
    inputs.equivalent_area_m2[5] = 0.0;
    inputs.equivalent_area_m2[6] = 0.0;
    (tree, inputs)
}

/// A bound is unchanged and is the base level of its donors, and a cell with no equivalent area is
/// unchanged: at steady state and after 10⁴ and 10⁶ years, on the bounded tree, the sea cell and
/// the lake's bed keep their ground (bits) at every uplift scale, the cells of no equivalent area
/// keep theirs, and at steady state the lake's donors rise from its surface, 300 m (bits), each by
/// the sum of u δ ÷ (K A^m) from the lake, while the sea's donor rises from the sea's 0 m.
#[test]
fn bounds_and_cells_of_no_area_are_unchanged() {
    let (tree, inputs) = bounded_tree();
    for time in [Time::Steady, Time::Years(1.0e4), Time::Years(1.0e6)] {
        let solution = inputs.solve(&tree, time);
        for c in [0, 2, 5, 6] {
            for lambda in [0.0, 1.0, 10.0] {
                assert_same_bits(solution.elevation_m(c, lambda), inputs.z0_m[slot(c)]);
            }
        }
    }
    let solution = inputs.solve(&tree, Time::Steady);
    let step = UPLIFT_M_PER_YEAR
        * (1_000.0
            * (1.0 / (EARTH_ERODIBILITY_M0_1_PER_YEAR * math::powf(1.0e8, DRAINAGE_EXPONENT))));
    for (c, base, sigma) in [
        (1, 0.0, step),
        (3, 300.0, step),
        (4, 300.0, step + step),
        (7, 300.0, step),
    ] {
        assert_same_bits(solution.base_m()[c], base);
        assert_same_bits(solution.per_lambda_m()[c], sigma);
    }
}

/// No cell without uplift rises (no deposition): on a chain whose construction climbs and falls
/// (0, 100, 50, 60, 70, 40, 90 m), with no uplift, every cell ends at or below its construction at
/// steady state and after 10² to 10⁶ years, and the cell below the 100 m sill keeps its 50 m while
/// the sill's knickpoint has not reached it; and a cell with no uplift above an uplifted one is
/// held at its construction at any scale of the uplift below it.
#[test]
fn no_cell_without_uplift_rises() {
    let tree = Tree::chain(7, 1_000.0);
    let mut inputs = Inputs::uniform(7, 0.0, 2.4e-5, 1.0e8);
    inputs.z0_m = vec![0.0, 100.0, 50.0, 60.0, 70.0, 40.0, 90.0];
    let mut times = vec![Time::Steady];
    times.extend([1.0e2, 1.0e3, 1.0e4, 1.0e5, 1.0e6].map(Time::Years));
    for time in times {
        let solution = inputs.solve(&tree, time);
        for c in 0..7 {
            let z = solution.elevation_m(cell_of(c), 1.0);
            assert!(z <= inputs.z0_m[c], "{time:?}: cell {c} rose to {z} m");
        }
        if time == Time::Years(1.0e2) {
            assert_same_bits(solution.elevation_m(2, 1.0), 50.0);
        }
    }
    let tree = Tree::chain(3, 1_000.0);
    let mut inputs = Inputs::uniform(3, UPLIFT_M_PER_YEAR, EARTH_ERODIBILITY_M0_1_PER_YEAR, 1.0e8);
    inputs.z0_m = vec![0.0, 100.0, 10.0];
    inputs.uplift_m_per_year[2] = 0.0;
    let solution = inputs.solve(&tree, Time::Steady);
    for lambda in [1.0, 10.0, 1_000.0] {
        assert!(solution.elevation_m(1, lambda) > 10.0);
        assert_same_bits(solution.elevation_m(2, lambda), 10.0);
    }
}

/// The elevation is linear in the uplift's scale to the bit: on a random forest of 500 cells (two
/// roots, three bounds inside it, a tenth of the cells without uplift, random links, slope
/// corrections, areas, erodibilities and constructions; the testkit's LCG), at steady state and
/// after 10⁵ years, solving with the uplift scaled by 4 or ¼ gives the same base to the bit, F
/// scaled by exactly 4 or ¼, and the same elevation at λ = 1 as the unscaled solve at λ = 4 or ¼.
#[test]
fn the_elevation_is_linear_in_the_uplift_s_scale_to_the_bit() {
    let n = 500;
    let mut lcg = Lcg::new(14);
    let receivers: Vec<Option<(u32, f64)>> = (0..n)
        .map(|i| {
            (i != 0 && i != 250).then(|| {
                let r = lcg.next_below(u64::try_from(i).unwrap());
                (u32::try_from(r).unwrap(), 500.0 + 1_500.0 * lcg.next_f64())
            })
        })
        .collect();
    let mut tree = Tree::new(receivers);
    for c in &mut tree.correction {
        *c = 0.5 + 0.5 * lcg.next_f64();
    }
    let mut inputs = Inputs::uniform(n, 0.0, 0.0, 0.0);
    for c in 0..n {
        inputs.z0_m[c] = 3_000.0 * lcg.next_f64();
        inputs.uplift_m_per_year[c] = if lcg.next_f64() < 0.1 {
            0.0
        } else {
            2.0e-3 * lcg.next_f64()
        };
        inputs.erodibility_m0_1_per_year[c] = 1.0e-6 + 9.0e-6 * lcg.next_f64();
        inputs.equivalent_area_m2[c] = math::powf(10.0, 6.0 + 4.0 * lcg.next_f64());
    }
    for c in [0, 250, 17, 101, 333] {
        inputs.fixed[c] = Some(inputs.z0_m[c] + 10.0);
    }
    for time in [Time::Steady, Time::Years(1.0e5)] {
        let one = inputs.solve(&tree, time);
        for scale in [4.0, 0.25] {
            let mut scaled = inputs.clone();
            for u in &mut scaled.uplift_m_per_year {
                *u *= scale;
            }
            let other = scaled.solve(&tree, time);
            for c in 0..n {
                assert_same_bits(other.base_m()[c], one.base_m()[c]);
                assert_same_bits(other.per_lambda_m()[c], scale * one.per_lambda_m()[c]);
                assert_same_bits(
                    other.elevation_m(cell_of(c), 1.0),
                    one.elevation_m(cell_of(c), scale),
                );
            }
        }
    }
}
