//! The orographic model's tests (R09.T13.e; decision-r09-t13ge §2.2, §2.4, §2.13, and
//! decision-r09-t13e-lfpm-convergence §3.5, the solve's).

use super::super::wind;
use super::*;
use hyperion_surface::cube::Face as CubeFace;
use hyperion_testkit::float::assert_same_bits;

/// The Earth's mean radius, m (6,371 km, from memory), for the strip worlds.
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// The sea's rate in every test world, kg m⁻² s⁻¹: about 2.6 mm a day.
const SEA_RATE: f64 = 3.0e-5;

/// The zonal budget's convergence over the land in the source tests, kg m⁻² s⁻¹: about 1.04 mm a
/// day, a test's set-up (decision-r09-t13e-revision §6.3 asks for any positive value), under which
/// a column far from any coast rains S ÷ (1 − ε₀) = 4.8 × 10⁻⁵, above [`SEA_RATE`], so that the
/// land's air disperses towards the sea.
const CONVERGENCE: f64 = 1.2e-5;

/// The ridge of decision-r09-t13ge §2.4 and §2.13: 3 km high, its half-width at half maximum
/// 100 km, its plains read at three half-widths from the crest.
const RIDGE_HEIGHT_M: f64 = 3_000.0;

/// The ridge's half-width at half maximum, m.
const RIDGE_HALF_WIDTH_M: f64 = 100_000.0;

/// The ridge's crest's distance from the coast, m, as the lane's 1-D runs put it.
const RIDGE_CREST_M: f64 = 400_000.0;

/// The ridge's height at `x_m` from the coast, m.
fn ridge(x_m: f64) -> f64 {
    let offset = (x_m - RIDGE_CREST_M) / RIDGE_HALF_WIDTH_M;
    RIDGE_HEIGHT_M * math::exp(-core::f64::consts::LN_2 * offset * offset)
}

/// How a planar mesh's x ends meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ends {
    /// Periodic in x: the last column faces the first.
    Periodic,
    /// Closed: each cell at the grid's edge faced by itself across an edge of no length.
    Closed,
}

/// How a chain's cells are labelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Labels {
    /// In order along the wind.
    AlongTheWind,
    /// In order against the wind.
    AgainstTheWind,
}

impl Labels {
    /// The label of the cell at place `x` along the wind of a chain of `n` cells; its own inverse.
    fn of(self, n: u32, x: u32) -> u32 {
        match self {
            Self::AlongTheWind => x,
            Self::AgainstTheWind => n - 1 - x,
        }
    }
}

impl Mesh {
    /// A flat grid of `nx` × `ny` cells of `step_m` along x by `side_m` along y, labelled
    /// `label(x, y)`, a permutation of the cells' indices, its faces in the order −x, +x, −y, +y,
    /// its x ends as `ends` says; along y each cell at the grid's edge is faced by itself across an
    /// edge of no length.
    fn planar(
        nx: u32,
        ny: u32,
        step_m: f64,
        side_m: f64,
        ends: Ends,
        label: impl Fn(u32, u32) -> u32,
    ) -> Self {
        let count = slot(nx * ny);
        let blank = Face {
            neighbour: 0,
            back: 0,
            normal: [0.0; 3],
            length_m: 0.0,
            conductance: 0.0,
        };
        let mut faces = vec![[blank; 4]; count];
        let wrap = |x: u32, step: i64| -> Option<u32> {
            let to = i64::from(x) + step;
            let n = i64::from(nx);
            if (0..n).contains(&to) {
                u32::try_from(to).ok()
            } else {
                match ends {
                    Ends::Periodic => u32::try_from(to.rem_euclid(n)).ok(),
                    Ends::Closed => None,
                }
            }
        };
        for y in 0..ny {
            for x in 0..nx {
                let me = label(x, y);
                let across = [
                    (
                        wrap(x, -1).map(|x| label(x, y)),
                        [-1.0, 0.0, 0.0],
                        side_m,
                        step_m,
                        1,
                    ),
                    (
                        wrap(x, 1).map(|x| label(x, y)),
                        [1.0, 0.0, 0.0],
                        side_m,
                        step_m,
                        0,
                    ),
                    (
                        y.checked_sub(1).map(|y| label(x, y)),
                        [0.0, -1.0, 0.0],
                        step_m,
                        side_m,
                        3,
                    ),
                    (
                        (y + 1 < ny).then(|| label(x, y + 1)),
                        [0.0, 1.0, 0.0],
                        step_m,
                        side_m,
                        2,
                    ),
                ];
                faces[slot(me)] = core::array::from_fn(|k| {
                    let (neighbour, normal, length_m, distance_m, back) = across[k];
                    match neighbour {
                        Some(neighbour) => Face {
                            neighbour,
                            back,
                            normal,
                            length_m,
                            conductance: length_m / distance_m,
                        },
                        None => Face {
                            neighbour: me,
                            back: u8::try_from(k).unwrap(),
                            normal,
                            length_m: 0.0,
                            conductance: 0.0,
                        },
                    }
                });
            }
        }
        Self {
            areas_m2: vec![step_m * side_m; count],
            ups: vec![[0.0, 0.0, 1.0]; count],
            faces,
        }
    }
}

/// A month's inputs, owned.
struct World {
    heights_m: Vec<f64>,
    cover: Vec<Cover>,
    ground: Vec<GroundState>,
    winds: Vec<CellWind>,
    rates: Vec<f64>,
    convergence: Vec<f64>,
}

impl World {
    /// The world's month.
    fn month(&self) -> Month<'_> {
        Month {
            heights_m: &self.heights_m,
            cover: &self.cover,
            ground: &self.ground,
            winds: &self.winds,
            sea_rates_kg_m2_s: &self.rates,
            convergence_kg_m2_s: &self.convergence,
        }
    }

    /// A world of `count` cells, each covered, raised and blown by `cell(i)`, the sea at
    /// [`SEA_RATE`], its ground thawed and with no source.
    fn of(count: u32, cell: impl Fn(u32) -> (Cover, f64, CellWind)) -> Self {
        let (mut heights_m, mut cover, mut winds, mut rates) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for i in 0..count {
            let (c, h, w) = cell(i);
            cover.push(c);
            heights_m.push(h);
            winds.push(w);
            rates.push(if c == Cover::Sea { SEA_RATE } else { 0.0 });
        }
        let cells = cover.len();
        Self {
            heights_m,
            cover,
            ground: vec![GroundState::Thawed; cells],
            winds,
            rates,
            convergence: vec![0.0; cells],
        }
    }

    /// The world with the zonal budget's convergence `convergence_kg_m2_s` at every land cell,
    /// kg m⁻² s⁻¹, and none at the sea's, which the model does not read.
    #[must_use]
    fn with_convergence(mut self, convergence_kg_m2_s: f64) -> Self {
        for (c, &cover) in self.convergence.iter_mut().zip(&self.cover) {
            *c = match cover {
                Cover::Land => convergence_kg_m2_s,
                Cover::Sea => 0.0,
            };
        }
        self
    }

    /// The world with each land cell `i` for which `frozen(i)` holds frozen and every other cell
    /// thawed.
    #[must_use]
    fn frozen_where(mut self, frozen: impl Fn(u32) -> bool) -> Self {
        for ((i, ground), &cover) in (0_u32..).zip(&mut self.ground).zip(&self.cover) {
            *ground = if cover == Cover::Land && frozen(i) {
                GroundState::Frozen
            } else {
                GroundState::Thawed
            };
        }
        self
    }
}

/// A wind along +x on a planar mesh.
const ALONG_X: CellWind = CellWind::Towards([1.0, 0.0, 0.0]);

/// The rates along a chain of `land` cells of `dx_m` between a sea upwind and a sea downwind,
/// under a wind along it, each land cell at `height(x)` of its centre's distance x from the coast,
/// m, the cells labelled as `labels` says: each land cell's (x, P) in order along the wind, and
/// the run's diagnostics.
fn chain(
    land: u32,
    dx_m: f64,
    height: impl Fn(f64) -> f64,
    parameters: &Parameters,
    labels: Labels,
) -> (Vec<(f64, f64)>, Diagnostics) {
    Chain::new(land, dx_m, height, labels).rates(parameters)
}

/// [`chain`]'s world, its month open to change before it is solved: `land` cells of `dx_m` along
/// a planar chain between a sea upwind and a sea downwind, under a wind along it.
struct Chain {
    /// The planar mesh: `land` + 2 cells of `dx_m` along x by 1 km along y, its x ends closed.
    mesh: Mesh,
    /// The month's inputs: a sea cell at either end and the land between, under the wind along
    /// +x, ground thawed and with no source until a test changes them.
    world: World,
    /// The land cells.
    land: u32,
    /// Each cell's length along the wind, m.
    dx_m: f64,
    /// How the cells are labelled.
    labels: Labels,
}

impl Chain {
    /// The chain of `land` cells of `dx_m`, each land cell at `height(x)` of its centre's distance
    /// x from the coast, m, the cells labelled as `labels` says, its ground thawed and with no
    /// source.
    fn new(land: u32, dx_m: f64, height: impl Fn(f64) -> f64, labels: Labels) -> Self {
        let n = land + 2;
        let mesh = Mesh::planar(n, 1, dx_m, 1_000.0, Ends::Closed, |x, _| labels.of(n, x));
        // Cell i is the cell at x with label(x) = i, and the label is its own inverse.
        let world = World::of(n, |i| {
            let x = labels.of(n, i);
            if x == 0 || x == n - 1 {
                (Cover::Sea, 0.0, ALONG_X)
            } else {
                (Cover::Land, height(Self::centre_at(dx_m, x)), ALONG_X)
            }
        });
        Self {
            mesh,
            world,
            land,
            dx_m,
            labels,
        }
    }

    /// The distance from the coast of the centre of the cell at place `x` along a chain of cells
    /// of `dx_m`, m, the upwind sea at place 0.
    fn centre_at(dx_m: f64, x: u32) -> f64 {
        (f64::from(x) - 0.5) * dx_m
    }

    /// The chain's month solved at `parameters`: each land cell's (x, P) in order along the wind,
    /// and the run's diagnostics.
    fn rates(&self, parameters: &Parameters) -> (Vec<(f64, f64)>, Diagnostics) {
        let n = self.land + 2;
        let result = solve(&self.mesh, &self.world.month(), parameters);
        let rates = (1..=self.land)
            .map(|x| {
                let p = result.precipitation_kg_m2_s[slot(self.labels.of(n, x))];
                (Self::centre_at(self.dx_m, x), p)
            })
            .collect();
        (rates, result.diagnostics)
    }
}

/// The rate at `x` interpolated linearly between `points`, sorted by their first member.
fn at(points: &[(f64, f64)], x: f64) -> f64 {
    let after = points
        .iter()
        .position(|&(px, _)| px >= x)
        .expect("the point lies within the samples");
    assert!(after > 0, "the point lies within the samples");
    let (start, end) = (points[after - 1], points[after]);
    start.1 + (end.1 - start.1) * (x - start.0) / (end.0 - start.0)
}

/// A rain shadow's figures on the samples `points` of (distance from the coast, P): the lee
/// plain's rate over the windward plain's, each three half-widths from the crest; the lee slope's
/// mean rate over the windward slope's, each within two half-widths of the crest; and the
/// maximum's offset from the crest, m.
fn shadow(points: &[(f64, f64)]) -> (f64, f64, f64) {
    let plains = at(points, RIDGE_CREST_M + 3.0 * RIDGE_HALF_WIDTH_M)
        / at(points, RIDGE_CREST_M - 3.0 * RIDGE_HALF_WIDTH_M);
    let mean = |range: core::ops::Range<f64>| {
        let inside: Vec<f64> = points
            .iter()
            .filter(|(x, _)| range.contains(x))
            .map(|&(_, p)| p)
            .collect();
        inside.iter().sum::<f64>() / f64::from(u32::try_from(inside.len()).unwrap())
    };
    let reach = 2.0 * RIDGE_HALF_WIDTH_M;
    let slopes =
        mean(RIDGE_CREST_M..RIDGE_CREST_M + reach) / mean(RIDGE_CREST_M - reach..RIDGE_CREST_M);
    let (peak_x, _) = points
        .iter()
        .copied()
        .fold((0.0, f64::NEG_INFINITY), |best, (x, p)| {
            if p > best.1 { (x, p) } else { best }
        });
    (plains, slopes, peak_x - RIDGE_CREST_M)
}

/// The e-folding length of `points`' rates, m: −1 over the least-squares slope of ln P against
/// the distance from 500 to 2,500 km inland (decision-r09-t13ge §2.2).
fn e_folding_m(points: &[(f64, f64)]) -> f64 {
    let fit: Vec<(f64, f64)> = points
        .iter()
        .filter(|(x, _)| (500_000.0..=2_500_000.0).contains(x))
        .map(|&(x, p)| (x, math::ln(p)))
        .collect();
    let count = f64::from(u32::try_from(fit.len()).unwrap());
    let (mx, my) = fit
        .iter()
        .fold((0.0, 0.0), |(a, b), &(x, y)| (a + x / count, b + y / count));
    let (sxy, sxx) = fit.iter().fold((0.0, 0.0), |(a, b), &(x, y)| {
        (a + (x - mx) * (y - my), b + (x - mx) * (x - mx))
    });
    -sxx / sxy
}

/// A solve's iteration and components, for the tests' prints.
fn describe(d: &Diagnostics) -> String {
    let c = &d.components;
    format!(
        "{} restarts, {} sweeps, measure {:.1e}{}, relative residual {:.1e}; {} land cells in \
         components of 1: {}, 2: {}, 3–16: {}, > 16: {} ({} cells, the largest {}), the flow \
         order's backward share {:.4} (the largest's {:.4}); {} below zero",
        d.restarts,
        d.sweeps,
        d.measure,
        if d.converged { "" } else { " (the cap)" },
        d.relative_residual,
        d.land_cells,
        c.single,
        c.pairs,
        c.small,
        c.large,
        c.large_cells,
        c.largest,
        d.backward_share,
        d.largest_backward_share,
        d.negative_cells
    )
}

/// Each cell's wind against its edges in `month` on `mesh`, as [`solve`] reads it.
fn locals_of(mesh: &Mesh, month: &Month<'_>) -> Vec<Local> {
    (0..mesh.areas_m2.len())
        .map(|i| Local::of(mesh, i, month.cover[i], month.winds[i]))
        .collect()
}

/// The share of the internal advective weight of `components`' components larger than
/// [`EXACT_BLOCK_CELLS`] that their members' order as it stands leaves pointing backward: the
/// measure of [`Diagnostics::backward_share`], of any order.
fn backward_share_of(mesh: &Mesh, locals: &[Local], components: &Components) -> f64 {
    let mut place = vec![u32::MAX; locals.len()];
    let (mut backward, mut internal) = (0.0, 0.0);
    for c in 0..components.len() {
        let range = components.range(c);
        if range.len() <= EXACT_BLOCK_CELLS {
            continue;
        }
        let mut members = components.cells[range.clone()].to_vec();
        members.sort_unstable();
        for (p, &cell) in (0_u32..).zip(&members) {
            place[slot(cell)] = p;
        }
        let mut position = vec![0; members.len()];
        for (t, &cell) in components.cells[range].iter().enumerate() {
            position[slot(place[slot(cell)])] = t;
        }
        for (p, &cell) in members.iter().enumerate() {
            let links = Links::of(mesh, locals, &members, &place, cell);
            for &(q, weight) in links.successors.iter().flatten() {
                internal += weight;
                if position[slot(q)] < position[p] {
                    backward += weight;
                }
            }
        }
    }
    backward / internal
}

/// The land's budget's closure, (inflow + source − runoff − outflow) ÷ (inflow + source).
fn closure(d: &Diagnostics) -> f64 {
    let given = d.inflow_kg_s + d.source_kg_s;
    (given - d.runoff_kg_s - d.outflow_kg_s) / given
}

/// Each land cell's error against a deep solve, |P − P*| ÷ max(P*, [`RATE_FLOOR_SHARE`] `P̄_sea`)
/// (decision-r09-t13e-lfpm-convergence §3.5): the largest and the share of land cells above
/// 10⁻³.
fn errors(cover: &[Cover], rates: &[f64], reference: &Orographic) -> (f64, f64) {
    let floor = RATE_FLOOR_SHARE * reference.diagnostics.sea_mean_kg_m2_s;
    let (mut worst, mut above, mut land) = (0.0_f64, 0_u32, 0_u32);
    for ((&c, &p), &q) in cover
        .iter()
        .zip(rates)
        .zip(&reference.precipitation_kg_m2_s)
    {
        if c == Cover::Land {
            let error = (p - q).abs() / q.max(floor);
            worst = worst.max(error);
            above += u32::from(error > 1e-3);
            land += 1;
        }
    }
    (worst, f64::from(above) / f64::from(land))
}

/// Asserts the ruled convergence of `month` on `mesh` (decision-r09-t13e-lfpm-convergence §3.5):
/// its solve at the production tolerance converges within the cap, every land cell's rate lies
/// within 1% of a deep solve's, |P − P*| ≤ 0.01 max(P*, 10⁻³ `P̄_sea`), and the land's budget
/// closes within 10⁻³ of its inflow and source, which give it water; the deep solve meets the stop
/// test at [`DEEP_MEASURE`]. Prints both. With no source the inflow alone must be positive; with
/// one, the net inflow may be negative, where the land's air, richer than the sea's, disperses
/// back across the coast.
fn assert_converges(name: &str, mesh: &Mesh, month: &Month<'_>) -> Orographic {
    let solved = solve(mesh, month, &Parameters::HERGARTEN_ROBL);
    let reference = solve(mesh, month, &Parameters::deep());
    let (d, dd) = (&solved.diagnostics, &reference.diagnostics);
    let (worst, above) = errors(month.cover, &solved.precipitation_kg_m2_s, &reference);
    eprintln!(
        "{name}: {}; the budget's closure {:.1e}; against the deep solve (closure {:.1e}; {}) the \
         worst land cell {worst:.2e}, {:.2}% above 0.1%",
        describe(d),
        closure(d),
        closure(dd),
        describe(dd),
        100.0 * above
    );
    assert!(d.converged, "{name}: {d:?}");
    assert!(d.measure <= TOLERANCE, "{name}: {d:?}");
    assert!(
        dd.measure <= DEEP_MEASURE,
        "{name}: the deep solve's {dd:?}"
    );
    assert!(worst <= 0.01, "{name}: the worst land cell is {worst} off");
    assert!(closure(d).abs() <= 1e-3, "{name}: {d:?}");
    assert!(
        d.inflow_kg_s + d.source_kg_s > 0.0 && d.runoff_kg_s > 0.0,
        "{name}: {d:?}"
    );
    solved
}

#[test]
fn the_set_fixes_beta_zero_and_the_effective_long_range_length() {
    let set = Parameters::HERGARTEN_ROBL;
    let (lc, lf, ll) = (CONDENSATION_LENGTH_M, FALLOUT_LENGTH_M, LONG_RANGE_LENGTH_M);
    // Eq. 33, and eq. 32 expanded: L_l ÷ L_f + L_c ÷ L_l − φ − 1.
    let beta_zero = set.sea_level_reevaporation();
    assert!((beta_zero - 18.05).abs() < 1e-12, "β₀ = {beta_zero}");
    assert!((beta_zero - (ll / lf + lc / ll - lc / lf - 1.0)).abs() < 1e-12);
    assert_same_bits(set.reevaporation(-50.0), beta_zero);
    assert!((set.reevaporation(HEIGHT_SCALE_M) - beta_zero / core::f64::consts::E).abs() < 1e-12);
    // Eq. 24's root directly, and eq. 48's approximation, L_l ÷ (1 − ε₀) = 2,000 km.
    let effective = set.effective_long_range_length_m();
    let phi = lc / lf;
    let fallout = (1.0 - SEA_LEVEL_RECYCLED_SHARE) * phi;
    let half = 0.5 * (1.0 + beta_zero + SEA_LEVEL_RECYCLED_SHARE * phi + fallout);
    let smaller = half - (half * half - fallout).sqrt();
    assert!((effective - lc / smaller).abs() < 1e-6 * effective);
    assert!(
        (effective - 2_003_800.0).abs() < 100.0,
        "L̃_l = {effective} m"
    );
    assert!((effective / (ll / (1.0 - SEA_LEVEL_RECYCLED_SHARE)) - 1.0).abs() < 2.5e-3);
    // Without recycling, the long-range length is the one β₀ was set from (eqs. 24, 33).
    let dry = set.with_sea_level_recycled_share(0.0);
    assert!((dry.effective_long_range_length_m() / ll - 1.0).abs() < 1e-12);
    // The sea's air rains at its own rate in the model: F_c ÷ L_f = P_sea (eqs. 8, 31).
    let [vapour, cloud] = set.sea_air(SEA_RATE);
    assert!((cloud / lf / SEA_RATE - 1.0).abs() < 1e-15);
    assert!(((vapour + cloud) / (ll * SEA_RATE) - 1.0).abs() < 1e-15);
    // The ruled stop (decision-r09-t13e-lfpm-convergence §3.2 item 8).
    assert_same_bits(set.tolerance(), TOLERANCE);
    assert_same_bits(TOLERANCE, 1e-4);
    assert_eq!(set.max_restarts(), MAX_RESTARTS);
    assert_eq!((RESTART, MAX_RESTARTS, EXACT_BLOCK_CELLS), (10, 30, 16));
    eprintln!("β₀ = {beta_zero}, L̃_l = {:.1} km", effective / 1e3);
}

#[test]
fn the_cell_matrix_has_a_non_negative_inverse_at_every_path() {
    let set = Parameters::HERGARTEN_ROBL;
    for path in [1e-4, 1e-2, 0.5, 1.0, 1.7, 10.0, 1e3, 1e5] {
        for height_m in [-100.0, 0.0, 1_000.0, 3_000.0, 9_000.0] {
            for spread in [0.0, 0.8, 5.0] {
                let m = cell_matrix(&set, path, height_m, set.recycled_share(height_m), spread);
                assert!(m[0][1] <= 0.0 && m[1][0] <= 0.0 && m[0][0] > 0.0 && m[1][1] > 0.0);
                assert!(m[0][0] * m[1][1] - m[0][1] * m[1][0] > 0.0);
                let inv = inverse(m);
                for entry in inv.iter().flatten() {
                    assert!(
                        *entry >= 0.0 && entry.is_finite(),
                        "path {path}, {height_m} m, spread {spread}: {inv:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_mesh_s_edges_close_each_cell_and_agree_across_it() {
    let graph = CellGraph::new(7).unwrap();
    let radius = Metres::new(EARTH_RADIUS_M);
    let mesh = Mesh::of_graph(&graph, radius);
    assert_eq!(mesh.len(), graph.len());
    let total: f64 = mesh.areas_m2.iter().sum();
    let sphere = 4.0 * core::f64::consts::PI * EARTH_RADIUS_M * EARTH_RADIUS_M;
    assert!((total / sphere - 1.0).abs() < 1e-12);
    for (i, faces) in mesh.faces.iter().enumerate() {
        let up = mesh.ups[i];
        // Σ ℓ n̂ of a closed cell vanishes but for the sphere's curvature.
        let closure = faces.iter().fold([0.0; 3], |sum, face| {
            [
                sum[0] + face.length_m * face.normal[0],
                sum[1] + face.length_m * face.normal[1],
                sum[2] + face.length_m * face.normal[2],
            ]
        });
        let perimeter: f64 = faces.iter().map(|face| face.length_m).sum();
        assert!(norm(closure) < 1e-3 * perimeter, "cell {i}: {closure:?}");
        for face in faces {
            assert!(dot(face.normal, up).abs() < 1e-15);
            let other = &mesh.faces[slot(face.neighbour)][usize::from(face.back)];
            assert_eq!(slot(other.neighbour), i);
            assert_same_bits(other.length_m, face.length_m);
            assert_same_bits(other.conductance, face.conductance);
            assert!(dot(other.normal, face.normal) < -0.999);
        }
    }
}

/// Decision-r09-t13ge §2.4's ridge on the lane's 1-D runs: 0.288, 0.309 and 0.335 at spacings of
/// 0.5, 36 and 72 km, its maximum 4–33 km windward of the crest; here on a chain of the mesh, whose
/// cells are each a component of their own and carry no dispersion, so that the start is the solve.
#[test]
fn a_ridge_casts_a_rain_shadow_along_a_chain() {
    let set = Parameters::HERGARTEN_ROBL;
    // The lane's runs' cells: 1,000 km of land, whole cells.
    for (dx_m, land, research) in [
        (500.0, 2_000, 0.288),
        (36_000.0, 27, 0.309),
        (72_000.0, 13, 0.335),
    ] {
        let (points, diagnostics) = chain(land, dx_m, ridge, &set, Labels::AlongTheWind);
        let (plains, slopes, peak_m) = shadow(&points);
        eprintln!(
            "chain at {:.1} km: lee ÷ windward plain {plains:.3} (the lane's 1-D {research}), \
             slopes {slopes:.3}, maximum {:+.1} km from the crest; {}",
            dx_m / 1e3,
            peak_m / 1e3,
            describe(&diagnostics)
        );
        assert!(plains < 0.5, "{plains}");
        assert!(
            peak_m < 0.0,
            "the maximum lies on the windward side: {peak_m} m"
        );
        assert_eq!(diagnostics.components.single, land);
        assert_eq!(diagnostics.restarts, 0);
        assert_same_bits(diagnostics.measure, 0.0);
        if dx_m < 1_000.0 {
            assert!(
                (plains - research).abs() < 0.005,
                "{plains} against {research}"
            );
        }
    }
}

/// Decision-r09-t13ge §2.2: the lane's 1-D scheme gives 2,004, 2,022 and 2,040 km at 0.5, 36 and
/// 72 km, and with no recycling 500.2, 517.8 and 535.2 km, backward Euler's closed form.
#[test]
fn inland_rain_decays_over_the_effective_long_range_length_along_a_chain() {
    let set = Parameters::HERGARTEN_ROBL;
    let expected = set.effective_long_range_length_m();
    // 3,000 km of land, whole cells.
    for (dx_m, land, research) in [
        (500.0, 6_000, 2_004.0),
        (36_000.0, 83, 2_022.0),
        (72_000.0, 41, 2_040.0),
    ] {
        let (points, _) = chain(land, dx_m, |_| 0.0, &set, Labels::AlongTheWind);
        let folding = e_folding_m(&points);
        let (dry_points, _) = chain(
            land,
            dx_m,
            |_| 0.0,
            &set.with_sea_level_recycled_share(0.0),
            Labels::AlongTheWind,
        );
        let dry = e_folding_m(&dry_points);
        let euler = dx_m / math::ln_1p(dx_m / LONG_RANGE_LENGTH_M);
        eprintln!(
            "chain at {:.1} km: e-folding {:.1} km (the lane's {research}; L̃_l {:.1} km); \
             without recycling {:.1} km (backward Euler {:.1} km)",
            dx_m / 1e3,
            folding / 1e3,
            expected / 1e3,
            dry / 1e3,
            euler / 1e3
        );
        assert!((0.5 * expected..=2.0 * expected).contains(&folding));
        assert!((folding / 1e3 - research).abs() < 2.0, "{folding} m");
        assert!(
            (dry / euler - 1.0).abs() < 2e-3,
            "{dry} m against {euler} m"
        );
    }
}

/// A sphere of sea with a strip of land on face +x across the equator, under a wind towards the
/// east everywhere.
struct Strip {
    graph: CellGraph,
    mesh: Mesh,
    world: World,
    /// Each cell's distance east of the strip's coast meridian, m.
    east_m: Vec<f64>,
}

impl Strip {
    /// The strip at `level`: land where a cell's centre lies on face +x within `half_width_deg`
    /// of the equator and from 0 to `length_m` east of the meridian at `coast_deg` east, at
    /// `height(d)` of that distance d, m.
    fn new(
        level: u8,
        coast_deg: f64,
        length_m: f64,
        half_width_deg: f64,
        height: impl Fn(f64) -> f64,
    ) -> Self {
        let graph = CellGraph::new(level).unwrap();
        let mesh = Mesh::of_graph(&graph, Metres::new(EARTH_RADIUS_M));
        let (sin, cos) = math::sin_cos(coast_deg.to_radians());
        let coast_east = [-sin, cos, 0.0];
        let east_m: Vec<f64> = graph
            .centres()
            .iter()
            .map(|&c| EARTH_RADIUS_M * math::asin(dot(c, coast_east)))
            .collect();
        let world = World::of(graph.len(), |n| {
            let c = graph.centre(n);
            let latitude_deg = math::asin(c[2]).to_degrees();
            let d = east_m[slot(n)];
            let land = graph.cell(n).face() == CubeFace::PosX
                && latitude_deg.abs() < half_width_deg
                && (0.0..=length_m).contains(&d);
            let east = [-c[1], c[0], 0.0];
            let wind = if norm(east) > 1e-12 {
                CellWind::Towards(east)
            } else {
                CellWind::Calm
            };
            if land {
                (Cover::Land, height(d), wind)
            } else {
                (Cover::Sea, 0.0, wind)
            }
        });
        Self {
            graph,
            mesh,
            world,
            east_m,
        }
    }

    /// The land's (distance east of the coast, P) along each of the two rows of cells about the
    /// equator on face +x, west to east.
    fn equator(&self, rates: &[f64]) -> [Vec<(f64, f64)>; 2] {
        let half = 1_u32 << (self.graph.level() - 1);
        [half - 1, half].map(|row| {
            let mut cells: Vec<u32> = (0..self.graph.len())
                .filter(|&n| {
                    let cell = self.graph.cell(n);
                    cell.face() == CubeFace::PosX
                        && cell.j() == row
                        && self.world.cover[slot(n)] == Cover::Land
                })
                .collect();
            cells.sort_by_key(|&n| self.graph.cell(n).i());
            cells
                .iter()
                .map(|&n| (self.east_m[slot(n)], rates[slot(n)]))
                .collect()
        })
    }
}

/// A sphere of sea at some level with land on face +x where a predicate of each cell's centre's
/// latitude and longitude holds, flat or raised, under a wind and a sea rate of each cell's.
struct Sphere {
    mesh: Mesh,
    world: World,
}

impl Sphere {
    /// The sphere at `level`: land where a cell's centre lies on face +x and `land(latitude°,
    /// longitude°)` holds, at the height `height(latitude°, longitude°)`, every cell under
    /// `wind(centre)`, the sea's rate `rate(latitude°)`.
    fn new(
        level: u8,
        land: impl Fn(f64, f64) -> bool,
        height: impl Fn(f64, f64) -> f64,
        wind: impl Fn([f64; 3]) -> CellWind,
        rate: impl Fn(f64) -> f64,
    ) -> Self {
        let graph = CellGraph::new(level).unwrap();
        let mesh = Mesh::of_graph(&graph, Metres::new(EARTH_RADIUS_M));
        let mut world = World::of(graph.len(), |n| {
            let c = graph.centre(n);
            let (latitude, longitude) = (
                math::asin(c[2]).to_degrees(),
                math::atan2(c[1], c[0]).to_degrees(),
            );
            if graph.cell(n).face() == CubeFace::PosX && land(latitude, longitude) {
                (Cover::Land, height(latitude, longitude), wind(c))
            } else {
                (Cover::Sea, 0.0, wind(c))
            }
        });
        for (n, r) in (0_u32..).zip(&mut world.rates) {
            if world.cover[slot(n)] == Cover::Sea {
                *r = rate(math::asin(graph.centre(n)[2]).to_degrees());
            }
        }
        Self { mesh, world }
    }
}

/// The wind at `c` towards local east times `u` and local north times `v`, any length.
fn towards(c: [f64; 3], u: f64, v: f64) -> CellWind {
    let (east, north) = wind::east_north(c);
    CellWind::Towards([
        u * east[0] + v * north[0],
        u * east[1] + v * north[1],
        u * east[2] + v * north[2],
    ])
}

/// The Earth's sidereal day, s (86,164.0989 s, from memory), for the convergence line's Coriolis
/// parameter.
const SIDEREAL_DAY_S: f64 = 86_164.098_9;

/// The ruled convergence line (decision-r09-t13e-lfpm-convergence §3.5, like its replica's
/// scenario F): a 3,000 km square continent on the equator, flat, under trades converging on it,
/// v = −1.5 tanh(φ ÷ 8°) m/s with u by the winds' drag balance, the sea's rate 1 + 4 e^(−(φ ÷ 5°)²)
/// of [`SEA_RATE`].
fn convergence_line(level: u8) -> Sphere {
    Sphere::new(
        level,
        |latitude, longitude| latitude.abs() <= 13.5 && longitude.abs() <= 13.5,
        |_, _| 0.0,
        |c| {
            let latitude = math::asin(c[2]).to_degrees();
            let v = -1.5 * math::tanh(latitude / 8.0);
            let (u, _) = wind::drag_balance(v, wind::coriolis_per_s(SIDEREAL_DAY_S, c));
            towards(c, u, v)
        },
        |latitude| SEA_RATE * (1.0 + 4.0 * math::exp(-(latitude / 5.0) * (latitude / 5.0))),
    )
}

/// The ruled side-edge world (decision-r09-t13e-lfpm-convergence §3.5, like its replica's
/// scenario I): about 3,000 by 1,500 km of flat land at 28.25–41.75° S, 17° W to 17° E, on face
/// +x, under a due-south wind. The cells' east and west edges are meridians, whose normals lean
/// poleward by sin φ sin(Δλ ÷ 2) in the cell's tangent plane, so that every east–west pair passes
/// air both ways and each row of land is one component (plan 09's E-L2); and the cell index, which
/// rises northward on the face, runs against the flow.
fn due_south() -> Sphere {
    Sphere::new(
        7,
        |latitude, longitude| (-41.75..=-28.25).contains(&latitude) && longitude.abs() <= 17.0,
        |_, _| 0.0,
        |c| towards(c, 0.0, -1.0),
        |_| SEA_RATE,
    )
}

/// The ruled pole world (decision-r09-t13e-lfpm-convergence, E-L8 §1 and §3, like its replica's
/// "pole"): the due-south world's land ([`due_south`]) under a due-south wind, but for its two
/// westernmost columns of land cells, under a wind towards the north-east, and its two
/// easternmost, towards the north-west (0.6 of the way east or west and 0.8 north, the
/// replica's). The side edges merge each row into a cycle (plan 09's E-L2), and the end columns
/// carry the air back north and inward, which joins the rows into one component; the cell index,
/// which rises northward on the face, runs against the interior's flow. On face +x a column of
/// cells is a meridian.
fn pole() -> Sphere {
    let mut sphere = due_south();
    let graph = CellGraph::new(7).unwrap();
    let land = |n: u32| sphere.world.cover[slot(n)] == Cover::Land;
    let columns = (0..graph.len())
        .filter(|&n| land(n))
        .map(|n| graph.cell(n).i());
    let (west, east) = columns.fold((u32::MAX, 0), |(w, e), i| (w.min(i), e.max(i)));
    for n in 0..graph.len() {
        if !land(n) {
            continue;
        }
        let (i, c) = (graph.cell(n).i(), graph.centre(n));
        if i <= west + 1 {
            sphere.world.winds[slot(n)] = towards(c, 0.6, 0.8);
        } else if i + 1 >= east {
            sphere.world.winds[slot(n)] = towards(c, -0.6, 0.8);
        }
    }
    sphere
}

/// The ruled pole test (decision-r09-t13e-lfpm-convergence, E-L8 §3): on the pole world, whose
/// rows join into one component with its index against the flow, the solve converges within eight
/// restarts, every land cell within 1% of a deep solve ([`assert_converges`]); and the flow order
/// leaves under half the backward weight that the index order does.
#[test]
fn a_poleward_world_joined_into_one_component_against_its_index_converges_within_eight_restarts() {
    let pole = pole();
    let month = pole.world.month();
    let solved = assert_converges("level 7 pole", &pole.mesh, &month);
    let d = &solved.diagnostics;
    let locals = locals_of(&pole.mesh, &month);
    let index_share =
        backward_share_of(&pole.mesh, &locals, &upwind_components(&pole.mesh, &locals));
    eprintln!(
        "  the backward share in index order {index_share:.4}, in the flow order {:.4}",
        d.backward_share
    );
    assert!(
        d.components.largest * 5 >= d.land_cells * 4,
        "the rows join into one component: {}",
        describe(d)
    );
    assert!(d.restarts <= 8, "{}", describe(d));
    assert!(
        d.backward_share < 0.5 * index_share,
        "{} against {index_share}",
        d.backward_share
    );
}

/// The ruled determinism of the order (decision-r09-t13e-lfpm-convergence, E-L8 §3): the pole
/// world's flow order is the same twice, its components and its backward weights to the bit, and
/// so is its solve; and the order is not the index order.
#[test]
fn the_flow_order_and_its_solve_are_the_same_bits_twice() {
    let pole = pole();
    let month = pole.world.month();
    let order = || {
        let locals = locals_of(&pole.mesh, &month);
        let mut components = upwind_components(&pole.mesh, &locals);
        let flow = order_along_flow(&pole.mesh, &locals, &mut components);
        (components, flow)
    };
    let ((first, a), (second, b)) = (order(), order());
    assert_eq!(first, second);
    assert_ne!(
        first,
        upwind_components(&pole.mesh, &locals_of(&pole.mesh, &month)),
        "the order moved the large components' members"
    );
    for (x, y) in [
        (a.backward, b.backward),
        (a.internal, b.internal),
        (a.largest.1, b.largest.1),
        (a.largest.2, b.largest.2),
    ] {
        assert_same_bits(x, y);
    }
    assert_eq!(a.largest.0, b.largest.0);
    assert!(a.backward > 0.0 && a.internal > a.backward, "{a:?}");
    let (p, q) = (
        solve(&pole.mesh, &month, &Parameters::HERGARTEN_ROBL),
        solve(&pole.mesh, &month, &Parameters::HERGARTEN_ROBL),
    );
    for (&x, &y) in p.precipitation_kg_m2_s.iter().zip(&q.precipitation_kg_m2_s) {
        assert_same_bits(x, y);
    }
    assert_same_bits(p.diagnostics.backward_share, a.share());
    assert_same_bits(p.diagnostics.largest_backward_share, a.largest_share());
    assert_eq!(p.diagnostics.restarts, q.diagnostics.restarts);
}

#[test]
fn a_ridge_casts_a_rain_shadow_on_the_cell_graph() {
    let set = Parameters::HERGARTEN_ROBL;
    for level in [7, 8] {
        let strip = Strip::new(level, -20.0, 1_000_000.0, 15.0, ridge);
        let result = solve(&strip.mesh, &strip.world.month(), &set);
        for row in strip.equator(&result.precipitation_kg_m2_s) {
            let (plains, slopes, peak_m) = shadow(&row);
            eprintln!(
                "level {level}: lee ÷ windward plain {plains:.3}, slopes {slopes:.3}, maximum \
                 {:+.1} km from the crest; {}",
                peak_m / 1e3,
                describe(&result.diagnostics)
            );
            assert!(plains < 0.5, "level {level}: {plains}");
            assert!(peak_m < 0.0, "level {level}: the maximum at {peak_m} m");
        }
    }
}

#[test]
fn inland_rain_decays_over_the_effective_long_range_length_on_the_cell_graph() {
    let set = Parameters::HERGARTEN_ROBL;
    let expected = set.effective_long_range_length_m();
    for level in [7, 8] {
        let strip = Strip::new(level, -35.0, 3_000_000.0, 15.0, |_| 0.0);
        let result = solve(&strip.mesh, &strip.world.month(), &set);
        for row in strip.equator(&result.precipitation_kg_m2_s) {
            let folding = e_folding_m(&row);
            eprintln!(
                "level {level}: e-folding {:.1} km against L̃_l {:.1} km; {}",
                folding / 1e3,
                expected / 1e3,
                describe(&result.diagnostics)
            );
            assert!(
                (0.5 * expected..=2.0 * expected).contains(&folding),
                "{folding}"
            );
        }
    }
}

/// The ruled convergence test (decision-r09-t13e-lfpm-convergence §3.5): on the flat 3,000 km
/// strip and the 3 km ridge at levels 7 and 8, a land convergence line on the equator and a
/// due-south wind against the cell order, every land cell's rate at the production tolerance lies
/// within 1% of a deep solve's, the deep solve meets the stop test at 10⁻¹⁰, and the land's budget
/// closes within 10⁻³ of its inflow.
#[test]
fn the_solve_lies_within_one_percent_of_a_deep_solve_and_closes_the_land_s_budget() {
    for level in [7, 8] {
        let strip = Strip::new(level, -35.0, 3_000_000.0, 15.0, |_| 0.0);
        assert_converges(
            &format!("level {level} strip"),
            &strip.mesh,
            &strip.world.month(),
        );
        let ridge = Strip::new(level, -20.0, 1_000_000.0, 15.0, ridge);
        assert_converges(
            &format!("level {level} ridge"),
            &ridge.mesh,
            &ridge.world.month(),
        );
    }
    let line = convergence_line(8);
    let solved = assert_converges("level 8 convergence line", &line.mesh, &line.world.month());
    // The line's rain against the sea's maximum (E-L7's trap).
    let peak = solved
        .precipitation_kg_m2_s
        .iter()
        .copied()
        .fold(0.0, f64::max);
    eprintln!(
        "  the line's wettest land cell {:.2} times the sea's maximum rate",
        peak / (5.0 * SEA_RATE)
    );
    let south = due_south();
    let solved = assert_converges("level 7 due south", &south.mesh, &south.world.month());
    assert!(
        solved.diagnostics.components.large > 0,
        "the rows are cycles"
    );
}

/// A planar world of `nx` × `ny` cells of 33 km, sea on its rim and land within, at `height` of
/// the cell, every land cell for which `calm` holds still, the wind along +x.
fn planar_world(
    nx: u32,
    ny: u32,
    label: impl Fn(u32, u32) -> u32 + Copy,
    height: impl Fn(u32, u32) -> f64,
    calm: impl Fn(u32, u32) -> bool,
) -> (Mesh, World) {
    let mesh = Mesh::planar(nx, ny, 33_000.0, 33_000.0, Ends::Closed, label);
    let mut at = vec![(0, 0); slot(nx * ny)];
    for y in 0..ny {
        for x in 0..nx {
            at[slot(label(x, y))] = (x, y);
        }
    }
    let world = World::of(nx * ny, |i| {
        let (x, y) = at[slot(i)];
        if x == 0 || y == 0 || x + 1 == nx || y + 1 == ny {
            (Cover::Sea, 0.0, ALONG_X)
        } else if calm(x, y) {
            (Cover::Land, height(x, y), CellWind::Calm)
        } else {
            (Cover::Land, height(x, y), ALONG_X)
        }
    });
    (mesh, world)
}

/// A Gaussian hill on a planar world: 3 km high, its half-width at half maximum three cells,
/// centred on cell (`cx`, `cy`).
fn hill(cx: u32, cy: u32) -> impl Fn(u32, u32) -> f64 {
    move |x, y| {
        let (dx, dy) = (f64::from(x) - f64::from(cx), f64::from(y) - f64::from(cy));
        RIDGE_HEIGHT_M * math::exp(-core::f64::consts::LN_2 * (dx * dx + dy * dy) / 9.0)
    }
}

#[test]
fn the_land_s_water_budget_closes() {
    let row_major = |x: u32, y: u32| y * 30 + x;
    let cases: [(&str, (Mesh, World)); 3] = [
        (
            "flat",
            planar_world(30, 14, row_major, |_, _| 0.0, |_, _| false),
        ),
        (
            "hill",
            planar_world(30, 14, row_major, hill(10, 7), |_, _| false),
        ),
        (
            "hill and calm",
            planar_world(30, 14, row_major, hill(10, 7), |x, y| x == 20 && y > 4),
        ),
    ];
    for (name, (mesh, world)) in &cases {
        let d = solve(mesh, &world.month(), &Parameters::deep()).diagnostics;
        let out = d.runoff_kg_s + d.outflow_kg_s;
        eprintln!(
            "{name}: inflow {:.6e} kg/s, runoff {:.6e}, recycled {:.6e}, outflow {:.6e}; \
             closure {:.1e}; {}",
            d.inflow_kg_s,
            d.runoff_kg_s,
            d.recycled_kg_s,
            d.outflow_kg_s,
            closure(&d),
            describe(&d)
        );
        assert!(d.inflow_kg_s > 0.0 && d.recycled_kg_s > 0.0);
        assert!(
            (d.inflow_kg_s - out).abs() < 1e-12 * d.inflow_kg_s,
            "{name}"
        );
    }
    // On the sphere, at level 7, over flat land: deep, the same.
    let strip = Strip::new(7, -20.0, 1_500_000.0, 10.0, |_| 0.0);
    let d = solve(&strip.mesh, &strip.world.month(), &Parameters::deep()).diagnostics;
    eprintln!(
        "level 7 strip: closure {:.1e}; {}",
        closure(&d),
        describe(&d)
    );
    assert!(closure(&d).abs() < 1e-10, "{d:?}");
}

/// The ruled relabelling test (decision-r09-t13e-lfpm-convergence §3.5): a chain labelled either
/// way gives the same bits, and a grid labelled by rows, by columns and backwards agrees within
/// 10⁻⁹ of its largest rate at the deep tolerance.
#[test]
fn the_rates_do_not_depend_on_the_cells_index_order() {
    let set = Parameters::HERGARTEN_ROBL;
    // A chain's order is its own, whichever way it is labelled: the same bits.
    let (forward, _) = chain(400, 2_500.0, ridge, &set, Labels::AlongTheWind);
    let (backward, _) = chain(400, 2_500.0, ridge, &set, Labels::AgainstTheWind);
    for (&(x, p), &(_, q)) in forward.iter().zip(&backward) {
        assert_same_bits(q, p);
        assert!(p > 0.0, "at {x} m");
    }
    // A grid labelled by rows, by columns and backwards breaks its ties differently: the same
    // rates once deep.
    let (nx, ny) = (24, 12);
    let labels: [&dyn Fn(u32, u32) -> u32; 3] =
        [&|x, y| y * nx + x, &|x, y| x * ny + y, &|x, y| {
            nx * ny - 1 - (y * nx + x)
        }];
    let mut runs = labels.iter().map(|label| {
        let (mesh, world) = planar_world(nx, ny, label, hill(8, 6), |_, _| false);
        let result = solve(&mesh, &world.month(), &Parameters::deep());
        assert!(result.diagnostics.measure <= DEEP_MEASURE);
        let mut by_place = vec![0.0; slot(nx * ny)];
        for y in 0..ny {
            for x in 0..nx {
                by_place[slot(y * nx + x)] = result.precipitation_kg_m2_s[slot(label(x, y))];
            }
        }
        by_place
    });
    let first = runs.next().unwrap();
    let largest = first.iter().copied().fold(0.0, f64::max);
    for other in runs {
        for (p, q) in first.iter().zip(&other) {
            assert!((p - q).abs() <= 1e-9 * largest, "{p} against {q}");
        }
    }
}

#[test]
fn a_zonal_ring_is_one_component_a_row_and_converges_wherever_its_labels_start() {
    // A ring of three land rows round a periodic grid, sea rows beside it, fed by dispersion.
    let (nx, ny) = (40, 5);
    let run = |shift: u32| {
        let label = move |x: u32, y: u32| y * nx + (x + shift) % nx;
        let mesh = Mesh::planar(nx, ny, 33_000.0, 33_000.0, Ends::Periodic, label);
        let mut at = vec![(0, 0); slot(nx * ny)];
        for y in 0..ny {
            for x in 0..nx {
                at[slot(label(x, y))] = (x, y);
            }
        }
        let world = World::of(nx * ny, |i| {
            let (_, y) = at[slot(i)];
            let cover = if y == 0 || y + 1 == ny {
                Cover::Sea
            } else {
                Cover::Land
            };
            (cover, 0.0, ALONG_X)
        });
        let result = solve(&mesh, &world.month(), &Parameters::deep());
        let mut by_place = vec![0.0; slot(nx * ny)];
        for y in 0..ny {
            for x in 0..nx {
                by_place[slot(y * nx + x)] = result.precipitation_kg_m2_s[slot(label(x, y))];
            }
        }
        (by_place, result.diagnostics)
    };
    let (first, d) = run(0);
    let (second, _) = run(17);
    eprintln!("zonal ring: {}", describe(&d));
    assert_eq!(d.land_cells, nx * (ny - 2));
    assert_eq!(
        (
            d.components.large,
            d.components.large_cells,
            d.components.largest
        ),
        (ny - 2, nx * (ny - 2), nx),
        "one component a land row"
    );
    assert!(d.measure <= DEEP_MEASURE);
    let largest = first.iter().copied().fold(0.0, f64::max);
    for y in 1..ny - 1 {
        for x in 0..nx {
            let i = slot(y * nx + x);
            assert!(first[i] > 0.0);
            assert!((first[i] - second[i]).abs() <= 1e-9 * largest);
        }
    }
}

/// The ruled ring test (decision-r09-t13e-lfpm-convergence, E-L8 §3): three land rows round a
/// periodic grid under a wind along it, labelled out of order along each row (17 x + 5 mod 40) or
/// against the flow, are each one component, whose flow order starts at its least index, runs
/// downstream and has exactly one backward edge, from its last cell into its first; the solve
/// reports that order's backward share, a fortieth.
#[test]
fn a_zonal_ring_s_flow_order_has_one_backward_edge_and_runs_downstream() {
    let (nx, ny) = (40, 5);
    let labels: [&dyn Fn(u32, u32) -> u32; 2] = [&|x, y| y * nx + (17 * x + 5) % nx, &|x, y| {
        y * nx + (nx - 1 - x)
    }];
    for (name, label) in ["17 x + 5", "backwards"].into_iter().zip(labels) {
        let mesh = Mesh::planar(nx, ny, 33_000.0, 33_000.0, Ends::Periodic, label);
        let mut row = vec![0; slot(nx * ny)];
        for y in 0..ny {
            for x in 0..nx {
                row[slot(label(x, y))] = y;
            }
        }
        let world = World::of(nx * ny, |i| {
            let y = row[slot(i)];
            let cover = if y == 0 || y + 1 == ny {
                Cover::Sea
            } else {
                Cover::Land
            };
            (cover, 0.0, ALONG_X)
        });
        let month = world.month();
        let locals = locals_of(&mesh, &month);
        let mut components = upwind_components(&mesh, &locals);
        let index_share = backward_share_of(&mesh, &locals, &components);
        let flow = order_along_flow(&mesh, &locals, &mut components);
        // Each cell's +x face is its second; the wind crosses no other.
        let east = |cell: u32| mesh.faces[slot(cell)][1].neighbour;
        let mut rings = 0;
        for c in 0..components.len() {
            let members = &components.cells[components.range(c)];
            if members.len() <= EXACT_BLOCK_CELLS {
                continue;
            }
            rings += 1;
            assert_eq!(members.len(), slot(nx));
            assert_eq!(Some(&members[0]), members.iter().min(), "{name}");
            for pair in members.windows(2) {
                assert_eq!(east(pair[0]), pair[1], "{name}: downstream");
            }
            assert_eq!(east(members[slot(nx) - 1]), members[0], "{name}");
        }
        assert_eq!(rings, ny - 2, "{name}");
        eprintln!(
            "ring labelled {name}: the backward share in index order {index_share:.4}, in the flow \
             order {:.4}",
            flow.share()
        );
        assert!((flow.share() - 1.0 / 40.0).abs() < 1e-15, "{flow:?}");
        assert!(index_share > flow.share(), "{name}: {index_share}");
        let d = solve(&mesh, &month, &Parameters::HERGARTEN_ROBL).diagnostics;
        assert_same_bits(d.backward_share, flow.share());
        assert!(d.converged, "{name}: {}", describe(&d));
    }
}

/// The ruled two-cell cycle (decision-r09-t13e-lfpm-convergence §3.5): two land cells blowing into
/// each other between two seas blowing onshore, at different heights, so that all they take in
/// rains out between them. Their 4 × 4 balance, `M_A` `Q_A` − `Q_B` = `s_A` and `M_B` `Q_B` − `Q_A`
/// = `s_B` (each passes all its air to the other), has the closed form (`M_A` − `M_B`⁻¹) `Q_A` =
/// `s_A` + `M_B`⁻¹ `s_B`, `Q_B` = `M_B`⁻¹ (`s_B` + `Q_A`); the solve equals it to 10⁻¹² in its
/// first sweep.
#[test]
fn a_two_cell_cycle_equals_the_closed_form_of_its_balance_in_one_sweep() {
    let set = Parameters::HERGARTEN_ROBL;
    let side_m = 33_000.0;
    let mesh = Mesh::planar(4, 1, side_m, side_m, Ends::Closed, |x, _| x);
    let (east, west) = (ALONG_X, CellWind::Towards([-1.0, 0.0, 0.0]));
    let world = World {
        heights_m: vec![0.0, 500.0, 1_500.0, 0.0],
        cover: vec![Cover::Sea, Cover::Land, Cover::Land, Cover::Sea],
        ground: vec![GroundState::Thawed; 4],
        winds: vec![east, east, west, west],
        rates: vec![SEA_RATE, 0.0, 0.0, 2.0 * SEA_RATE],
        convergence: vec![0.0; 4],
    };
    let result = solve(&mesh, &world.month(), &set);
    let d = &result.diagnostics;
    eprintln!("two-cell cycle: {}", describe(d));
    assert_eq!((d.land_cells, d.components.pairs), (2, 1));
    assert_eq!(d.restarts, 0, "the first sweep is the solve");
    assert_same_bits(d.measure, 0.0);
    // The closed form, by 2 × 2 inverses alone.
    let path = side_m * side_m / side_m / CONDENSATION_LENGTH_M;
    let (m_a, m_b) = (
        cell_matrix(&set, path, 500.0, set.recycled_share(500.0), 0.0),
        cell_matrix(&set, path, 1_500.0, set.recycled_share(1_500.0), 0.0),
    );
    let sea = |rate: f64| set.sea_air(rate).map(|flux| flux * side_m);
    let (s_a, s_b) = (sea(SEA_RATE), sea(2.0 * SEA_RATE));
    let apply = |m: [[f64; 2]; 2], v: [f64; 2]| {
        [
            m[0][0] * v[0] + m[0][1] * v[1],
            m[1][0] * v[0] + m[1][1] * v[1],
        ]
    };
    let inv_b = inverse(m_b);
    let schur = [
        [m_a[0][0] - inv_b[0][0], m_a[0][1] - inv_b[0][1]],
        [m_a[1][0] - inv_b[1][0], m_a[1][1] - inv_b[1][1]],
    ];
    let carried = apply(inv_b, s_b);
    let q_a = apply(inverse(schur), [s_a[0] + carried[0], s_a[1] + carried[1]]);
    let q_b = apply(inv_b, [s_b[0] + q_a[0], s_b[1] + q_a[1]]);
    for (cell, q) in [(1, q_a), (2, q_b)] {
        let expected = q[1] / (side_m * FALLOUT_LENGTH_M);
        let p = result.precipitation_kg_m2_s[cell];
        eprintln!("  cell {cell}: P {p:.12e} against the closed form's {expected:.12e}");
        assert!(
            (p / expected - 1.0).abs() <= 1e-12,
            "{p} against {expected}"
        );
    }
    // Nothing leaves: all the sea gives rains out.
    assert_same_bits(d.outflow_kg_s, 0.0);
    assert!(
        (d.runoff_kg_s / d.inflow_kg_s - 1.0).abs() <= 1e-12,
        "{d:?}"
    );
}

/// The ruled order test (decision-r09-t13e-lfpm-convergence §3.5): on a hand-built graph of a
/// 2-cycle, a 3-cycle and a 20-cell cycle with chains between them, its nodes labelled out of
/// order and some not members, every edge between components points forward, each component's
/// members are contiguous and in index order, the components are the cycles and the chains' nodes
/// alone, and the same graph gives the same order twice.
#[test]
fn the_components_are_in_topological_order_each_contiguous_and_in_index_order() {
    const NODES: usize = 40;
    // The graph by name: entry → feed → pair₀ ↔ pair₁ → link → triple₀ → triple₁ → triple₂ →
    // triple₀, triple₂ → bridge → ring₀ → … → ring₁₉ → ring₀, ring₁₀ → exit, side → ring₅. Names
    // 0..31; 31..40 are no members.
    let (entry, feed, link, bridge, exit, side) = (0, 1, 4, 8, 9, 10);
    let (pair, triple) = ([2, 3], [5, 6, 7]);
    let ring = |k: usize| 11 + k;
    let mut edges = vec![
        (entry, feed),
        (feed, pair[0]),
        (pair[0], pair[1]),
        (pair[1], pair[0]),
    ];
    edges.extend([
        (pair[1], link),
        (link, triple[0]),
        (triple[0], triple[1]),
        (triple[1], triple[2]),
        (triple[2], triple[0]),
        (triple[2], bridge),
        (bridge, ring(0)),
        (ring(10), exit),
        (side, ring(5)),
    ]);
    edges.extend((0..20).map(|k| (ring(k), ring((k + 1) % 20))));
    // Each name's label: 17 n + 5 mod 40, a permutation.
    let label = |n: usize| (17 * n + 5) % NODES;
    let mut successors = vec![[None; 4]; NODES];
    for &(from, to) in &edges {
        let slots: &mut [Option<u32>; 4] = &mut successors[label(from)];
        let free = slots.iter().position(Option::is_none).unwrap();
        slots[free] = Some(index(label(to)));
    }
    let member = |n: usize| (0..31).any(|name| label(name) == n);
    let find = || strongly_connected(NODES, member, |n| successors[n]);
    let components = find();
    assert_eq!(components, find(), "the same graph, the same order");
    let mut place = [usize::MAX; NODES];
    for c in 0..components.len() {
        let members = &components.cells[components.range(c)];
        assert!(
            members.windows(2).all(|pair| pair[0] < pair[1]),
            "{members:?}"
        );
        for &m in members {
            place[slot(m)] = c;
        }
    }
    assert_eq!(components.cells.len(), 31);
    for (n, &at) in place.iter().enumerate() {
        assert_eq!(member(n), at != usize::MAX, "node {n}");
    }
    for &(from, to) in &edges {
        let (cf, ct) = (place[label(from)], place[label(to)]);
        assert!(cf <= ct, "the edge {from} → {to} points back");
    }
    let mut sizes: Vec<usize> = (0..components.len())
        .map(|c| components.range(c).len())
        .collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [1, 1, 1, 1, 1, 1, 2, 3, 20]);
    let together = |names: &[usize]| {
        names
            .iter()
            .all(|&n| place[label(n)] == place[label(names[0])])
    };
    assert!(together(&pair) && together(&triple));
    assert!(together(&(0..20).map(ring).collect::<Vec<_>>()));
    assert!(place[label(entry)] < place[label(pair[0])]);
    assert!(place[label(triple[2])] < place[label(ring(0))]);
    let sizes = components.sizes();
    assert_eq!(
        (
            sizes.single,
            sizes.pairs,
            sizes.small,
            sizes.large,
            sizes.large_cells,
            sizes.largest
        ),
        (6, 1, 1, 1, 20, 20)
    );
}

/// The ruled cap (decision-r09-t13e-lfpm-convergence §3.2 item 6, §3.5): a world that needs more
/// than one restart, solved with the cap lowered to one, reports that it did not converge and
/// returns finite rates, none negative.
#[test]
fn a_solve_that_reaches_its_cap_is_flagged_and_returns_finite_rates() {
    let (mesh, world) = planar_world(30, 14, |x, y| y * 30 + x, hill(10, 7), |_, _| false);
    let full = solve(&mesh, &world.month(), &Parameters::HERGARTEN_ROBL);
    eprintln!("uncapped: {}", describe(&full.diagnostics));
    assert!(full.diagnostics.converged && full.diagnostics.restarts > 1);
    let capped = solve(
        &mesh,
        &world.month(),
        &Parameters::HERGARTEN_ROBL.with_max_restarts(1),
    );
    let d = &capped.diagnostics;
    eprintln!("capped at one: {}", describe(d));
    assert!(!d.converged, "{d:?}");
    assert_eq!(d.restarts, 1);
    assert!(d.measure > TOLERANCE, "{d:?}");
    for &p in &capped.precipitation_kg_m2_s {
        assert!(p.is_finite() && p >= 0.0, "{p}");
    }
}

#[test]
fn a_calm_land_rains_out_its_coastal_inflow_and_nothing_inland() {
    let set = Parameters::HERGARTEN_ROBL;
    let (nx, ny) = (10, 5);
    let label = |x: u32, y: u32| y * nx + x;
    let (mesh, world) = planar_world(nx, ny, label, |_, _| 0.0, |_, _| true);
    let result = solve(&mesh, &world.month(), &set);
    // The coastal column takes the sea's L_l P_sea across each 33 km edge and keeps it all:
    // (1 − ε₀) P A = L_l P_sea ℓ.
    let coastal = LONG_RANGE_LENGTH_M * SEA_RATE / ((1.0 - SEA_LEVEL_RECYCLED_SHARE) * 33_000.0);
    for y in 1..ny - 1 {
        for x in 1..nx - 1 {
            let p = result.precipitation_kg_m2_s[slot(label(x, y))];
            if x == 1 {
                assert!((p / coastal - 1.0).abs() < 1e-14, "{p} against {coastal}");
            } else {
                assert_same_bits(p, 0.0);
            }
        }
    }
    let d = &result.diagnostics;
    assert!((d.runoff_kg_s / d.inflow_kg_s - 1.0).abs() < 1e-14);
    assert_same_bits(d.outflow_kg_s, 0.0);
    // No land's air moves on, so the solve stops at the start.
    assert_eq!(
        (d.restarts, d.sweeps, d.components.single),
        (0, 1, d.land_cells)
    );
    // A wholly calm month with no source brings the land nothing.
    let still = World {
        winds: vec![CellWind::Calm; world.winds.len()],
        ..world
    };
    let result = solve(&mesh, &still.month(), &set);
    for &p in &result.precipitation_kg_m2_s {
        assert_same_bits(p, 0.0);
    }
    assert_same_bits(result.diagnostics.inflow_kg_s, 0.0);
}

#[test]
fn the_runoff_is_the_unrecycled_share_of_the_rate_and_none_over_liquid() {
    let rate = 2.0e-5;
    assert_same_bits(
        runoff_kg_m2_s(rate, 0.0, Cover::Land, GroundState::Thawed),
        0.25 * rate,
    );
    assert_same_bits(
        runoff_kg_m2_s(rate, -300.0, Cover::Land, GroundState::Thawed),
        0.25 * rate,
    );
    let high = runoff_kg_m2_s(rate, HEIGHT_SCALE_M, Cover::Land, GroundState::Thawed);
    assert!((high - (1.0 - 0.75 / core::f64::consts::E) * rate).abs() < 1e-18);
    assert_same_bits(
        runoff_kg_m2_s(rate, 0.0, Cover::Sea, GroundState::Thawed),
        0.0,
    );
    assert_same_bits(
        runoff_kg_m2_s(0.0, 1_000.0, Cover::Land, GroundState::Thawed),
        0.0,
    );
    // The solve's recycled shares are the runoff's.
    let (mesh, world) = planar_world(6, 4, |x, y| y * 6 + x, hill(2, 2), |_, _| false);
    let result = solve(&mesh, &world.month(), &Parameters::HERGARTEN_ROBL);
    for (i, &share) in result.recycled_shares.iter().enumerate() {
        let p = result.precipitation_kg_m2_s[i];
        let runoff = runoff_kg_m2_s(p, world.heights_m[i], world.cover[i], world.ground[i]);
        match world.cover[i] {
            Cover::Land => assert!((runoff - (1.0 - share) * p).abs() <= 1e-15 * p),
            Cover::Sea => {
                assert_same_bits(share, 0.0);
                assert_same_bits(p, 0.0);
                assert_same_bits(runoff, 0.0);
            }
        }
    }
}

/// Decision-r09-t13e-revision §6.3, new unit test 1: a land ring under a zonal wind with no sea
/// and a uniform source S ([`CONVERGENCE`]), every cell land round a periodic planar grid of three
/// rows of 40 cells of 33 km at the sea level, under a wind along its rows. At the deep tolerance
/// every cell's rate is S ÷ (1 − ε₀) to 10⁻⁹ relative, and S with every cell frozen (ε = 0, §4.4):
/// far from any coast a column's P − E is its convergence and its E is ε P (§4.2). The world is
/// exact: on a planar periodic grid with the wind along a row, each cell's inflow width (its
/// upwind neighbour's +x edge) equals its outflow width (its own +x edge) exactly and no side edge
/// is downwind, so that the uniform state is the discrete balance's solution, with no coast and no
/// first-order error. Its budget takes in nothing from a sea and passes nothing to one: what runs
/// off is its source.
#[test]
fn a_land_ring_with_no_sea_rains_its_source_over_the_share_that_runs_off() {
    let (nx, ny, side_m) = (40, 3, 33_000.0);
    let mesh = Mesh::planar(nx, ny, side_m, side_m, Ends::Periodic, |x, y| y * nx + x);
    let area_m2 = side_m * side_m * f64::from(nx * ny);
    for (ground, share) in [
        (GroundState::Thawed, SEA_LEVEL_RECYCLED_SHARE),
        (GroundState::Frozen, 0.0),
    ] {
        let world = World::of(nx * ny, |_| (Cover::Land, 0.0, ALONG_X))
            .with_convergence(CONVERGENCE)
            .frozen_where(|_| ground == GroundState::Frozen);
        let result = solve(&mesh, &world.month(), &Parameters::deep());
        let d = &result.diagnostics;
        let floor = CONVERGENCE / (1.0 - share);
        let worst = result
            .precipitation_kg_m2_s
            .iter()
            .fold(0.0_f64, |worst, &p| worst.max((p / floor - 1.0).abs()));
        eprintln!(
            "{ground:?} land ring: every cell within {worst:.2e} of S ÷ (1 − ε) = {floor:.6e} \
             kg/m²/s; inflow {:.1e} kg/s, source {:.9e}, runoff {:.9e}, recycled {:.6e}, outflow \
             {:.1e}; closure {:.1e}; {}",
            d.inflow_kg_s,
            d.source_kg_s,
            d.runoff_kg_s,
            d.recycled_kg_s,
            d.outflow_kg_s,
            closure(d),
            describe(d)
        );
        assert!(d.converged && d.measure <= DEEP_MEASURE, "{}", describe(d));
        assert!(worst <= 1e-9, "{ground:?}: the worst cell is {worst} off");
        assert_same_bits(d.inflow_kg_s, 0.0);
        assert_same_bits(d.outflow_kg_s, 0.0);
        assert!(
            (d.source_kg_s / (CONVERGENCE * area_m2) - 1.0).abs() <= 1e-12,
            "{d:?}"
        );
        assert!((d.runoff_kg_s / d.source_kg_s - 1.0).abs() <= 1e-9, "{d:?}");
    }
}

/// Decision-r09-t13e-revision §6.3, new unit test 2, as its "Review points (follow-up 2)" restate
/// it: the strip with a source. Along a planar chain of 400 land cells of 25 km (10,000 km) at the
/// sea level between two seas at the sea's rate `P_sea` ([`SEA_RATE`]), under a wind along it,
/// each land cell taking a uniform source S, the rate tends inland to its floor P∞ = S ÷ (1 − ε₀)
/// (§4.2: P∞ + (`P_coast` − P∞) e^(−d/`L̃_l`), GPCP's form in its §2.2). From 3 `L̃_l` inland, up
/// to the last two land cells before the downwind sea, every land cell lies within a tenth of the
/// coastal excess of the floor, |P − P∞| ≤ 0.1 |`P_sea` − P∞|; and |P − P∞|, its logarithm fitted
/// by least squares from 500 to 2,500 km inland, e-folds within 2× of `L̃_l`. The decay gives
/// e⁻³ ≈ 0.05 at 3 `L̃_l` at the exact length, and 0.053 at the cell graph's 2,024–2,043 km, so the
/// bound has a margin of about 2×; being relative to the coastal excess, it does not hold by the
/// choice of source. The floor itself is unit test 1's, the ring's S ÷ (1 − ε₀) to 10⁻⁹
/// ([`a_land_ring_with_no_sea_rains_its_source_over_the_share_that_runs_off`]).
///
/// Three sources, as follow-up 2 rules: P∞ = 0.5 `P_sea`, a floor below the coast, as GPCP's
/// 50–55° N transect decays to one (Eurasia's 1.40 against 2.85 mm a day, the ruling's §2.2);
/// 1.5 `P_sea`, a floor above it, so that both directions of approach are held; and 0.9 `P_sea`,
/// the first case.
///
/// The bound leaves out the last two land cells before the downwind sea as a margin only. On this
/// chain no edge disperses: each edge along the wind lies square to it, so that its crosswind
/// share 1 − (ŵ · n̂)² is zero, and the side edges have no length. The downwind sea therefore does
/// not reach those cells, which follow the inland decay (the test prints them). For the same reason
/// the production solve's rates are the deep solve's: each land cell is a component of its own,
/// and the start is the solve.
#[test]
fn inland_of_a_coast_the_rate_tends_to_the_source_s_floor_over_the_effective_long_range_length() {
    let set = Parameters::HERGARTEN_ROBL;
    let effective_m = set.effective_long_range_length_m();
    for floor_share in [0.5, 1.5, 0.9] {
        let floor = floor_share * SEA_RATE;
        let source = floor * (1.0 - SEA_LEVEL_RECYCLED_SHARE);
        let mut strip = Chain::new(400, 25_000.0, |_| 0.0, Labels::AlongTheWind);
        strip.world = strip.world.with_convergence(source);
        let (points, d) = strip.rates(&set);
        let (deep, _) = strip.rates(&Parameters::deep());
        let off_deep = points
            .iter()
            .zip(&deep)
            .fold(0.0_f64, |worst, (&(_, p), &(_, q))| {
                worst.max((p / q - 1.0).abs())
            });
        // Each cell's distance from the floor over the coastal excess, |P − P∞| ÷ |P_sea − P∞|.
        let coastal_excess = (SEA_RATE - floor).abs();
        let off = |p: f64| (p - floor).abs() / coastal_excess;
        // Up to, but excluding, the last two land cells before the downwind sea.
        let inland = &points[..points.len() - 2];
        let beyond: Vec<(f64, f64)> = inland
            .iter()
            .filter(|&&(x, _)| x >= 3.0 * effective_m)
            .map(|&(x, p)| (x, off(p)))
            .collect();
        let (worst_m, worst) = beyond
            .iter()
            .copied()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("the chain's land runs beyond 3 L̃_l");
        let excess: Vec<(f64, f64)> = points
            .iter()
            .map(|&(x, p)| (x, (p - floor).abs()))
            .collect();
        let folding = e_folding_m(&excess);
        eprintln!(
            "strip with a source, P∞ = {floor_share} P_sea = {floor:.4e} kg/m²/s: the coastal cell \
             {:.4} of P∞ ({:.4} of P_sea); |P − P∞| e-folds over {:.1} km against L̃_l {:.1} km; \
             from 3 L̃_l ({:.0} km, {} cells) the worst cell {worst:.4} of the coastal excess \
             |P_sea − P∞|, at {:.0} km; the last two land cells, left out, {:?} of it; the deep \
             solve's rates within {off_deep:.1e}; {}",
            points[0].1 / floor,
            points[0].1 / SEA_RATE,
            folding / 1e3,
            effective_m / 1e3,
            3.0 * effective_m / 1e3,
            beyond.len(),
            worst_m / 1e3,
            points[points.len() - 2..]
                .iter()
                .map(|&(_, p)| off(p))
                .collect::<Vec<_>>(),
            describe(&d)
        );
        assert!(
            worst <= 0.1,
            "P∞ = {floor_share} P_sea: {worst} of the coastal excess at {worst_m} m"
        );
        assert!(
            (0.5 * effective_m..=2.0 * effective_m).contains(&folding),
            "P∞ = {floor_share} P_sea: {folding} m"
        );
        assert!(
            off_deep <= 1e-12,
            "P∞ = {floor_share} P_sea: the deep solve's rates differ by {off_deep}"
        );
    }
}

/// The zonal ring world of
/// `a_zonal_ring_is_one_component_a_row_and_converges_wherever_its_labels_start`: three land rows
/// round a periodic grid of 40 columns of 33 km cells between two sea rows, at the sea level,
/// under a wind along the rows, labelled by rows.
fn zonal_ring() -> (Mesh, World) {
    let (nx, ny) = (40, 5);
    let mesh = Mesh::planar(nx, ny, 33_000.0, 33_000.0, Ends::Periodic, |x, y| {
        y * nx + x
    });
    let world = World::of(nx * ny, |i| {
        let y = i / nx;
        let cover = if y == 0 || y + 1 == ny {
            Cover::Sea
        } else {
            Cover::Land
        };
        (cover, 0.0, ALONG_X)
    });
    (mesh, world)
}

/// Decision-r09-t13e-revision §6.3, new unit test 3: the land's budget with a source closes at the
/// production tolerance within 10⁻³ of inflow + source (§4.7), on the sphere's flat 3,000 km strip
/// and its 3 km ridge at level 7 and on the planar zonal ring, each land cell taking
/// [`CONVERGENCE`]; and each solve lies within 1% of a deep solve's ([`assert_converges`]). Where
/// the land's air is richer than the sea's it disperses back across the coast, which the net
/// inflow counts: the ring's, fed by dispersion alone, is negative.
#[test]
fn the_land_s_budget_with_a_source_closes_on_the_strip_the_ridge_and_the_ring() {
    let Strip { mesh, world, .. } = Strip::new(7, -35.0, 3_000_000.0, 15.0, |_| 0.0);
    let flat = (mesh, world.with_convergence(CONVERGENCE));
    let Strip { mesh, world, .. } = Strip::new(7, -20.0, 1_000_000.0, 15.0, ridge);
    let raised = (mesh, world.with_convergence(CONVERGENCE));
    let (mesh, world) = zonal_ring();
    let ring = (mesh, world.with_convergence(CONVERGENCE));
    for (name, (mesh, world)) in [
        ("level 7 strip with a source", flat),
        ("level 7 ridge with a source", raised),
        ("zonal ring with a source", ring),
    ] {
        let solved = assert_converges(name, &mesh, &world.month());
        let d = &solved.diagnostics;
        eprintln!(
            "  inflow {:.6e} kg/s, source {:.6e}, runoff {:.6e}, recycled {:.6e}, outflow {:.6e}",
            d.inflow_kg_s, d.source_kg_s, d.runoff_kg_s, d.recycled_kg_s, d.outflow_kg_s
        );
        assert!(d.source_kg_s > 0.0, "{name}: {d:?}");
    }
}

/// Decision-r09-t13e-revision §6.3, new unit test 4: a sink with a source. Between two seas under a
/// wind along a planar chain, a land cell at 1,200 m whose air goes nowhere (calm, or blowing
/// across the chain, where its downwind edge has no length) takes in the upwind sea's air across
/// their shared edge, `Q_in` = `L_l` `P_sea` (ŵ · n̂)⁺ ℓ (eq. 31), and its own source S A, with
/// S = C e^(−H/H₀) (§4.2), and runs it all off: P = (`Q_in` + S A) ÷ ((1 − ε) A) to 10⁻¹², with
/// ε = ε₀ e^(−H/H₀) on thawed ground and 0 on frozen ground (§4.4).
#[test]
fn a_sink_with_a_source_runs_off_its_inflow_and_its_source() {
    let set = Parameters::HERGARTEN_ROBL;
    let (dx_m, side_m, height_m) = (30_000.0, 40_000.0, 1_200.0);
    let mesh = Mesh::planar(3, 1, dx_m, side_m, Ends::Closed, |x, _| x);
    let area_m2 = dx_m * side_m;
    // The sea's +x edge, of length side_m, faces the wind square on: (ŵ · n̂)⁺ = 1.
    let [vapour, cloud] = set.sea_air(SEA_RATE);
    assert!(((vapour + cloud) / (LONG_RANGE_LENGTH_M * SEA_RATE) - 1.0).abs() <= 1e-15);
    let inflow_kg_s = (vapour + cloud) * side_m;
    let column = math::exp(-height_m / HEIGHT_SCALE_M);
    let source_kg_s = CONVERGENCE * column * area_m2;
    let across = CellWind::Towards([0.0, 1.0, 0.0]);
    for (air, wind) in [("calm", CellWind::Calm), ("across", across)] {
        for (ground, share) in [
            (GroundState::Thawed, SEA_LEVEL_RECYCLED_SHARE * column),
            (GroundState::Frozen, 0.0),
        ] {
            let world = World::of(3, |i| {
                if i == 1 {
                    (Cover::Land, height_m, wind)
                } else {
                    (Cover::Sea, 0.0, ALONG_X)
                }
            })
            .with_convergence(CONVERGENCE)
            .frozen_where(|_| ground == GroundState::Frozen);
            let result = solve(&mesh, &world.month(), &set);
            let d = &result.diagnostics;
            let expected = (inflow_kg_s + source_kg_s) / ((1.0 - share) * area_m2);
            let p = result.precipitation_kg_m2_s[1];
            let off = (p / expected - 1.0).abs();
            eprintln!(
                "{air}, {ground:?} sink: P {p:.12e} kg/m²/s against {expected:.12e} by hand \
                 ({off:.1e}); inflow {:.9e} kg/s, source {:.9e}, runoff {:.9e}; {}",
                d.inflow_kg_s,
                d.source_kg_s,
                d.runoff_kg_s,
                describe(d)
            );
            assert_eq!((d.land_cells, d.components.single), (1, 1));
            assert!(off <= 1e-12, "{air}, {ground:?}: {p} against {expected}");
            assert!((d.inflow_kg_s / inflow_kg_s - 1.0).abs() <= 1e-12, "{d:?}");
            assert!((d.source_kg_s / source_kg_s - 1.0).abs() <= 1e-12, "{d:?}");
            assert_same_bits(d.outflow_kg_s, 0.0);
            assert!(
                (d.runoff_kg_s / (inflow_kg_s + source_kg_s) - 1.0).abs() <= 1e-12,
                "{d:?}"
            );
        }
    }
}

/// What unit test 5 pins of a world's solve
/// ([`with_no_source_and_every_cell_thawed_five_worlds_solve_to_their_as_built_bits`]): its name,
/// the sum of its land cells' rates in cell order, kg m⁻² s⁻¹, its [`Diagnostics::restarts`], and
/// the rates of its first, middle and last land cells by index, kg m⁻² s⁻¹.
type Pinned = (&'static str, f64, u32, [f64; 3]);

/// The as-built solves of unit test 5's worlds, as the test printed them (Debug's shortest
/// round-trip form), 2026-10-20.
#[expect(
    clippy::unreadable_literal,
    reason = "the solves' printed digits as the test prints them, so that a re-take compares by eye"
)]
const PINNED: [Pinned; 5] = [
    (
        "ridge chain",
        0.008998127113649916,
        0,
        [
            2.9925625733261988e-5,
            1.9147542496626338e-5,
            7.0888148962233326e-6,
        ],
    ),
    (
        "level 7 strip",
        0.029211721839406693,
        1,
        [
            2.917765169932618e-5,
            2.8810620604174042e-5,
            2.233300102153046e-5,
        ],
    ),
    (
        "pole",
        0.023121115474501432,
        2,
        [
            6.235461855267233e-6,
            1.6683814000330068e-5,
            2.7360550775274634e-5,
        ],
    ),
    (
        "hill and calm",
        0.017163815396264492,
        2,
        [
            2.962764042068446e-5,
            2.9531441062232045e-5,
            2.2493308696957797e-5,
        ],
    ),
    (
        "zonal ring",
        0.0034678279885848227,
        1,
        [
            2.900305066791714e-5,
            2.868958711108753e-5,
            2.9003073944479683e-5,
        ],
    ),
];

/// Decision-r09-t13e-revision §6.3, new unit test 5: with no source and no frozen cell, the solve
/// is as built, bit for bit. Five of the tests' worlds, every cell thawed and no land cell given a
/// source, are solved at the production set and pinned with `assert_same_bits` against f64
/// literals ([`PINNED`]): the ridge chain (400 land cells of 2.5 km), the level-7 flat strip, the
/// pole world ([`pole`]), the hill with a calm column and the zonal ring ([`zonal_ring`]); for
/// each, the sum of its land cells' rates in cell order, its restarts, and its first, middle and
/// last land cells' rates by index. The literals are the as-built solves', which the lane's probe
/// showed equal to the code before R09.T13.j (commit 1870973b) to the bit, by 64-bit fingerprints
/// of every rate, share and budget figure, before and after. They are re-taken, as this test
/// prints them, on a deliberate change to the solve.
#[test]
fn with_no_source_and_every_cell_thawed_five_worlds_solve_to_their_as_built_bits() {
    let set = Parameters::HERGARTEN_ROBL;
    let chain = Chain::new(400, 2_500.0, ridge, Labels::AlongTheWind);
    let strip = Strip::new(7, -35.0, 3_000_000.0, 15.0, |_| 0.0);
    let pole = pole();
    let row_major = |x: u32, y: u32| y * 30 + x;
    let (hill_mesh, hill_world) =
        planar_world(30, 14, row_major, hill(10, 7), |x, y| x == 20 && y > 4);
    let (ring_mesh, ring_world) = zonal_ring();
    let worlds: [(&Mesh, &World); 5] = [
        (&chain.mesh, &chain.world),
        (&strip.mesh, &strip.world),
        (&pole.mesh, &pole.world),
        (&hill_mesh, &hill_world),
        (&ring_mesh, &ring_world),
    ];
    let solved: Vec<(f64, u32, [f64; 3])> = worlds
        .iter()
        .zip(&PINNED)
        .map(|(&(mesh, world), &(name, ..))| {
            assert!(
                world.ground.iter().all(|&g| g == GroundState::Thawed),
                "{name}: every cell thawed"
            );
            for &c in &world.convergence {
                assert_same_bits(c, 0.0);
            }
            let result = solve(mesh, &world.month(), &set);
            let land: Vec<usize> = (0..world.cover.len())
                .filter(|&i| world.cover[i] == Cover::Land)
                .collect();
            let sum = land
                .iter()
                .fold(0.0, |sum, &i| sum + result.precipitation_kg_m2_s[i]);
            let cells = [land[0], land[land.len() / 2], land[land.len() - 1]];
            let rates = cells.map(|i| result.precipitation_kg_m2_s[i]);
            eprintln!(
                "{name}: {} land cells, their rates' sum {sum:?}, {} restarts; cells {cells:?}: \
                 {rates:?}",
                land.len(),
                result.diagnostics.restarts
            );
            (sum, result.diagnostics.restarts, rates)
        })
        .collect();
    for ((sum, restarts, rates), &(name, pinned_sum, pinned_restarts, pinned_rates)) in
        solved.iter().zip(&PINNED)
    {
        assert_eq!(*restarts, pinned_restarts, "{name}: the restarts");
        assert_same_bits(*sum, pinned_sum);
        for (&rate, &pinned) in rates.iter().zip(&pinned_rates) {
            assert_same_bits(rate, pinned);
        }
    }
}

/// Decision-r09-t13e-revision §6.3, new unit test 6: the column factor. A lone calm land cell
/// 3 km up, in a month with no sea and no wind, takes e^(−1.5) of its band's convergence C as its
/// source, S = C e^(−H/H₀) with H₀ = 2 km (§4.2), to 10⁻¹², and rains it where it forms,
/// P = S ÷ (1 − ε) with ε = ε₀ e^(−H/H₀) (§4.5's wholly calm month); one at the sea level, and one
/// 200 m below it, which reads as at it, take all of C.
#[test]
fn a_cell_three_kilometres_up_takes_e_to_the_minus_one_and_a_half_of_its_band_s_convergence() {
    let set = Parameters::HERGARTEN_ROBL;
    let side_m = 33_000.0;
    let mesh = Mesh::planar(1, 1, side_m, side_m, Ends::Closed, |_, _| 0);
    let area_m2 = side_m * side_m;
    assert_same_bits(set.column_share(3_000.0), math::exp(-1.5));
    assert_same_bits(set.column_share(-200.0), 1.0);
    for (height_m, column) in [(3_000.0, math::exp(-1.5)), (0.0, 1.0), (-200.0, 1.0)] {
        let world =
            World::of(1, |_| (Cover::Land, height_m, CellWind::Calm)).with_convergence(CONVERGENCE);
        let result = solve(&mesh, &world.month(), &set);
        let d = &result.diagnostics;
        let taken = d.source_kg_s / (CONVERGENCE * area_m2);
        let expected = CONVERGENCE * column / (1.0 - SEA_LEVEL_RECYCLED_SHARE * column);
        let p = result.precipitation_kg_m2_s[0];
        eprintln!(
            "a calm cell at {height_m} m takes {taken:.15} of its convergence (e^(−H/H₀) \
             {column:.15}, {:.1e} off) and rains {p:.12e} kg/m²/s against S ÷ (1 − ε) \
             {expected:.12e} ({:.1e} off)",
            (taken / column - 1.0).abs(),
            (p / expected - 1.0).abs()
        );
        assert!(
            (taken / column - 1.0).abs() <= 1e-12,
            "{taken} against {column}"
        );
        assert!(
            (p / expected - 1.0).abs() <= 1e-12,
            "{p} against {expected}"
        );
    }
}

/// Frozen ground in the solve (decision-r09-t13e-revision §4.4; its §6.3 new unit test 7 holds
/// the rule on the ruled Earth's inputs, in the step's tests). On a planar world with a 3 km hill
/// whose southern half of land is frozen, the solve's recycled share is 0 on frozen land,
/// ε₀ e^(−H/H₀) on thawed land (the bits of [`Parameters::recycled_share`]) and 0 over the sea, and
/// its budget closes. A chain with every land cell frozen gives the same bits as the chain with no
/// recycling and every land cell thawed, flat or over the ridge: the cell matrix and the sink's
/// rule read ε = 0, while β keeps its height factor. So frozen ground's inland decay is `L_l`'s,
/// backward Euler's Δx ÷ ln(1 + Δx ÷ `L_l`), about 500 km (§4.4).
#[test]
fn frozen_ground_recycles_nothing_and_its_rain_decays_over_the_long_range_length() {
    let set = Parameters::HERGARTEN_ROBL;
    let (mesh, world) = planar_world(30, 14, |x, y| y * 30 + x, hill(10, 7), |_, _| false);
    let world = world.frozen_where(|i| i / 30 < 7);
    let result = solve(&mesh, &world.month(), &Parameters::deep());
    let (mut frozen_cells, mut thawed_cells) = (0, 0);
    for (i, &share) in result.recycled_shares.iter().enumerate() {
        match (world.cover[i], world.ground[i]) {
            (Cover::Sea, _) => assert_same_bits(share, 0.0),
            (Cover::Land, GroundState::Frozen) => {
                frozen_cells += 1;
                assert_same_bits(share, 0.0);
            }
            (Cover::Land, GroundState::Thawed) => {
                thawed_cells += 1;
                assert_same_bits(share, set.recycled_share(world.heights_m[i]));
            }
        }
    }
    let d = &result.diagnostics;
    eprintln!(
        "hill, half frozen: {frozen_cells} frozen and {thawed_cells} thawed land cells; closure \
         {:.1e}; {}",
        closure(d),
        describe(d)
    );
    assert!(
        frozen_cells > 0 && thawed_cells > 0,
        "the hill has frozen and thawed land: {frozen_cells} frozen, {thawed_cells} thawed"
    );
    assert!(closure(d).abs() < 1e-10, "{d:?}");
    let dry = set.with_sea_level_recycled_share(0.0);
    let frozen_chain = |land, dx_m, height: fn(f64) -> f64| {
        let mut frozen = Chain::new(land, dx_m, height, Labels::AlongTheWind);
        frozen.world = frozen.world.frozen_where(|_| true);
        frozen.rates(&set)
    };
    let flat: fn(f64) -> f64 = |_| 0.0;
    // The lane's no-recycling e-foldings, as the chain test of L̃_l prints them.
    for (dx_m, land, research) in [
        (500.0, 6_000, 500.2),
        (36_000.0, 83, 517.8),
        (72_000.0, 41, 535.2),
    ] {
        let (points, d) = frozen_chain(land, dx_m, flat);
        let (dry_points, dry_d) = chain(land, dx_m, flat, &dry, Labels::AlongTheWind);
        for (&(_, p), &(_, q)) in points.iter().zip(&dry_points) {
            assert_same_bits(p, q);
        }
        assert_same_bits(d.runoff_kg_s, dry_d.runoff_kg_s);
        let folding = e_folding_m(&points);
        let euler = dx_m / math::ln_1p(dx_m / LONG_RANGE_LENGTH_M);
        eprintln!(
            "frozen chain at {:.1} km: e-folding {:.1} km, the bits of the chain with no \
             recycling (the lane's 1-D {research} km; backward Euler {:.1} km)",
            dx_m / 1e3,
            folding / 1e3,
            euler / 1e3
        );
        assert!(
            (folding / euler - 1.0).abs() < 2e-3,
            "{folding} m against {euler} m"
        );
    }
    let (points, _) = frozen_chain(400, 2_500.0, ridge);
    let (dry_points, _) = chain(400, 2_500.0, ridge, &dry, Labels::AlongTheWind);
    for (&(_, p), &(_, q)) in points.iter().zip(&dry_points) {
        assert_same_bits(p, q);
    }
}

/// Frozen ground's runoff (decision-r09-t13e-revision §4.4–§4.5, the reworded runoff test's unit):
/// on frozen ground the runoff function is the whole rate at every height, and none over liquid;
/// and on a world with frozen cells the solve's recycled shares remain the runoff's, to the bit.
#[test]
fn the_runoff_on_frozen_ground_is_the_whole_rate_and_the_solve_s_shares_remain_the_runoff_s() {
    let rate = 2.0e-5;
    for height_m in [-300.0, 0.0, 1_000.0, HEIGHT_SCALE_M, 5_000.0] {
        assert_same_bits(
            runoff_kg_m2_s(rate, height_m, Cover::Land, GroundState::Frozen),
            rate,
        );
        assert_same_bits(
            runoff_kg_m2_s(rate, height_m, Cover::Sea, GroundState::Frozen),
            0.0,
        );
        assert!(runoff_kg_m2_s(rate, height_m, Cover::Land, GroundState::Thawed) < rate);
    }
    assert_same_bits(
        runoff_kg_m2_s(0.0, 1_000.0, Cover::Land, GroundState::Frozen),
        0.0,
    );
    let (mesh, world) = planar_world(6, 4, |x, y| y * 6 + x, hill(2, 2), |_, _| false);
    let world = world.frozen_where(|i| i % 2 == 0);
    let result = solve(&mesh, &world.month(), &Parameters::HERGARTEN_ROBL);
    let mut frozen_cells = 0;
    for (i, &share) in result.recycled_shares.iter().enumerate() {
        let p = result.precipitation_kg_m2_s[i];
        let runoff = runoff_kg_m2_s(p, world.heights_m[i], world.cover[i], world.ground[i]);
        match world.cover[i] {
            Cover::Land => {
                assert!(p > 0.0, "cell {i}");
                assert_same_bits(runoff, (1.0 - share) * p);
                if world.ground[i] == GroundState::Frozen {
                    frozen_cells += 1;
                    assert_same_bits(runoff, p);
                }
            }
            Cover::Sea => {
                assert_same_bits(share, 0.0);
                assert_same_bits(runoff, 0.0);
            }
        }
    }
    assert!(frozen_cells > 0);
}

/// The cost of one month at level 8 (R09.T13.e): the mesh, built once, and a month's solve, on a
/// sphere of some land and on one all land, under zonal belts that reverse at 30° and 60° and under
/// three cells whose trades and polar easterlies blow equatorward and westerlies poleward; with
/// each run's iteration. A timing alone, asserting nothing, run by name like the crate's other
/// timings (`planetary::hooks::cold_traps::tests::time_the_cold_traps` among them), and recorded in
/// plan 09's Risks.
#[test]
#[ignore = "a timing, run by name"]
fn time_one_month_at_level_8() {
    let graph = CellGraph::new(8).unwrap();
    let start = std::time::Instant::now();
    let mesh = Mesh::of_graph(&graph, Metres::new(EARTH_RADIUS_M));
    let built = start.elapsed().as_secs_f64();
    // Towards the east, or west, and a share of the way towards or away from the nearer pole.
    let wind = |c: [f64; 3], meridional: f64| {
        let east = [-c[1], c[0], 0.0];
        if norm(east) < 1e-12 {
            return CellWind::Calm;
        }
        let east = unit(east);
        let north = unit(tangent([0.0, 0.0, 1.0], c));
        let latitude = math::asin(c[2]).to_degrees();
        let poleward = if latitude < 0.0 {
            scale(north, -1.0)
        } else {
            north
        };
        let westerly = (30.0..60.0).contains(&latitude.abs());
        let (zonal, towards_pole) = if westerly {
            (1.0, meridional)
        } else {
            (-1.0, -meridional)
        };
        CellWind::Towards([
            zonal * east[0] + towards_pole * poleward[0],
            zonal * east[1] + towards_pole * poleward[1],
            zonal * east[2] + towards_pole * poleward[2],
        ])
    };
    let lands: [(&str, Land); 2] = [("some land", some_land), ("all land", all_land)];
    for (name, land) in lands {
        for (winds, meridional) in [("zonal belts", 0.0), ("three cells", 0.4)] {
            let world = World::of(graph.len(), |n| {
                let c = graph.centre(n);
                let cover = if land(c) { Cover::Land } else { Cover::Sea };
                (cover, 1_000.0 * c[2] * c[2], wind(c, meridional))
            });
            let start = std::time::Instant::now();
            let result = solve(&mesh, &world.month(), &Parameters::HERGARTEN_ROBL);
            let month = start.elapsed().as_secs_f64();
            eprintln!(
                "level 8, {name}, {winds}: mesh {built:.3} s once; a month {month:.3} s; {}",
                describe(&result.diagnostics)
            );
        }
    }
}

/// Whether a direction is land, in the timing world.
type Land = fn([f64; 3]) -> bool;

/// The timing world's land: about a quarter of the sphere, in blobs.
fn some_land(c: [f64; 3]) -> bool {
    math::sin(5.0 * c[0]) * math::cos(4.0 * c[1]) > 0.35
}

/// Every cell land.
fn all_land(_: [f64; 3]) -> bool {
    true
}
