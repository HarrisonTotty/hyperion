//! The orographic model's tests (R09.T13.e's first part; decision-r09-t13ge §2.2, §2.4, §2.13).

use super::*;
use hyperion_surface::cube::Face as CubeFace;
use hyperion_testkit::float::assert_same_bits;

/// The Earth's mean radius, m (6,371 km, from memory), for the strip worlds.
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// The sea's large-scale rate in every test world, kg m⁻² s⁻¹: about 2.6 mm a day.
const SEA_RATE: f64 = 3.0e-5;

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

impl Mesh {
    /// A flat grid of `nx` × `ny` cells of `step_m` along x by `side_m` along y, labelled
    /// `label(x, y)`, a permutation of the cells' indices, its faces in the order −x, +x, −y, +y:
    /// periodic in x if `periodic`, and otherwise each cell at the grid's edge faced by itself
    /// across an edge of no length.
    fn planar(
        nx: u32,
        ny: u32,
        step_m: f64,
        side_m: f64,
        periodic: bool,
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
            } else if periodic {
                u32::try_from(to.rem_euclid(n)).ok()
            } else {
                None
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
    winds: Vec<CellWind>,
    rates: Vec<f64>,
}

impl World {
    /// The world's month.
    fn month(&self) -> Month<'_> {
        Month {
            heights_m: &self.heights_m,
            cover: &self.cover,
            winds: &self.winds,
            sea_rates_kg_m2_s: &self.rates,
        }
    }

    /// A world of `count` cells, each covered, raised and blown by `cell(i)`, the sea at
    /// [`SEA_RATE`].
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
        Self {
            heights_m,
            cover,
            winds,
            rates,
        }
    }
}

/// A wind along +x on a planar mesh.
const ALONG_X: CellWind = CellWind::Towards([1.0, 0.0, 0.0]);

/// The rates along a chain of `land` cells of `dx_m` between a sea upwind and a sea downwind,
/// under a wind along it, each land cell at `height(x)` of its centre's distance x from the coast,
/// m, the cells labelled against the wind if `reversed`: each land cell's (x, P) in order along
/// the wind, and the run's diagnostics.
fn chain(
    land: u32,
    dx_m: f64,
    height: impl Fn(f64) -> f64,
    parameters: &Parameters,
    reversed: bool,
) -> (Vec<(f64, f64)>, Diagnostics) {
    let n = land + 2;
    let label = |x: u32| if reversed { n - 1 - x } else { x };
    let mesh = Mesh::planar(n, 1, dx_m, 1_000.0, false, |x, _| label(x));
    let centre = |x: u32| (f64::from(x) - 0.5) * dx_m;
    // Cell i is the cell at x with label(x) = i, and label is its own inverse.
    let world = World::of(n, |i| {
        let x = label(i);
        if x == 0 || x == n - 1 {
            (Cover::Sea, 0.0, ALONG_X)
        } else {
            (Cover::Land, height(centre(x)), ALONG_X)
        }
    });
    let result = solve(&mesh, &world.month(), parameters);
    let rates = (1..=land)
        .map(|x| (centre(x), result.precipitation_kg_m2_s[slot(label(x))]))
        .collect();
    (rates, result.diagnostics)
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
    // The sea's air rains at its own rate in the model: F_c ÷ L_f = P_ls (eqs. 8, 31).
    let [vapour, cloud] = set.sea_air(SEA_RATE);
    assert!((cloud / lf / SEA_RATE - 1.0).abs() < 1e-15);
    assert!(((vapour + cloud) / (ll * SEA_RATE) - 1.0).abs() < 1e-15);
    assert_eq!(set.sweeps(), SWEEPS);
    eprintln!("β₀ = {beta_zero}, L̃_l = {:.1} km", effective / 1e3);
}

#[test]
fn the_cell_matrix_has_a_non_negative_inverse_at_every_path() {
    let set = Parameters::HERGARTEN_ROBL;
    for path in [1e-4, 1e-2, 0.5, 1.0, 1.7, 10.0, 1e3, 1e5] {
        for height_m in [-100.0, 0.0, 1_000.0, 3_000.0, 9_000.0] {
            for spread in [0.0, 0.8, 5.0] {
                let m = cell_matrix(&set, path, height_m, spread);
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
/// 0.5, 36 and 72 km, its maximum 4–33 km windward of the crest; here on a chain of the mesh.
#[test]
fn a_ridge_casts_a_rain_shadow_along_a_chain() {
    let set = Parameters::HERGARTEN_ROBL;
    // The lane's runs' cells: 1,000 km of land, whole cells.
    for (dx_m, land, research) in [
        (500.0, 2_000, 0.288),
        (36_000.0, 27, 0.309),
        (72_000.0, 13, 0.335),
    ] {
        let (points, diagnostics) = chain(land, dx_m, ridge, &set, false);
        let (plains, slopes, peak_m) = shadow(&points);
        eprintln!(
            "chain at {:.1} km: lee ÷ windward plain {plains:.3} (the lane's 1-D {research}), \
             slopes {slopes:.3}, maximum {:+.1} km from the crest; cut {}",
            dx_m / 1e3,
            peak_m / 1e3,
            diagnostics.cut_cells
        );
        assert!(plains < 0.5, "{plains}");
        assert!(
            peak_m < 0.0,
            "the maximum lies on the windward side: {peak_m} m"
        );
        assert_eq!(diagnostics.cut_cells, 0);
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
        let (points, _) = chain(land, dx_m, |_| 0.0, &set, false);
        let folding = e_folding_m(&points);
        let (dry_points, _) = chain(
            land,
            dx_m,
            |_| 0.0,
            &set.with_sea_level_recycled_share(0.0),
            false,
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

#[test]
fn a_ridge_casts_a_rain_shadow_on_the_cell_graph() {
    let set = Parameters::HERGARTEN_ROBL;
    for level in [7, 8] {
        let strip = Strip::new(level, -20.0, 1_000_000.0, 15.0, ridge);
        let result = solve(&strip.mesh, &strip.world.month(), &set);
        let rows = strip.equator(&result.precipitation_kg_m2_s);
        for row in &rows {
            let (plains, slopes, peak_m) = shadow(row);
            eprintln!(
                "level {level}: lee ÷ windward plain {plains:.3}, slopes {slopes:.3}, maximum \
                 {:+.1} km from the crest; {} land cells, {} cut; last sweep {:.2e} of the mean",
                peak_m / 1e3,
                result.diagnostics.land_cells,
                result.diagnostics.cut_cells,
                result.diagnostics.last_relative_changes().largest_kg_m2_s
            );
            assert!(plains < 0.5, "level {level}: {plains}");
            assert!(peak_m < 0.0, "level {level}: the maximum at {peak_m} m");
        }
        // The ruling's measure at the count, and the stricter largest cell's at 64 sweeps.
        let last = result.diagnostics.last_relative_changes();
        assert!(last.mean_kg_m2_s < 1e-3, "{last:?}");
        let converged = solve(&strip.mesh, &strip.world.month(), &set.with_sweeps(64));
        print_sweeps(level, "ridge", &converged.diagnostics);
        let last = converged.diagnostics.last_relative_changes();
        assert!(last.largest_kg_m2_s < 1e-3, "{last:?}");
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
                "level {level}: e-folding {:.1} km against L̃_l {:.1} km; last sweep {:.2e} of \
                 the mean",
                folding / 1e3,
                expected / 1e3,
                result.diagnostics.last_relative_changes().largest_kg_m2_s
            );
            assert!(
                (0.5 * expected..=2.0 * expected).contains(&folding),
                "{folding}"
            );
        }
        // The ruling's measure at the count, and the stricter largest cell's at 64 sweeps.
        let last = result.diagnostics.last_relative_changes();
        assert!(last.mean_kg_m2_s < 1e-3, "{last:?}");
        let converged = solve(&strip.mesh, &strip.world.month(), &set.with_sweeps(64));
        print_sweeps(level, "inland", &converged.diagnostics);
        let last = converged.diagnostics.last_relative_changes();
        assert!(last.largest_kg_m2_s < 1e-3, "{last:?}");
    }
}

/// Prints the first sweeps' and every eighth sweep's changes over the global mean: the largest
/// cell's, and the global mean's own; and the first sweep after which each stays under 10⁻³.
fn print_sweeps(level: u8, name: &str, diagnostics: &Diagnostics) {
    let mean = diagnostics.global_mean_kg_m2_s;
    let count = diagnostics.sweeps.len();
    let changes: Vec<String> = diagnostics
        .sweeps
        .iter()
        .enumerate()
        .filter(|(n, _)| *n < 4 || n % 8 == 7 || n + 1 == count)
        .map(|(n, change)| {
            format!(
                "{}: {:.1e} / {:.1e}",
                n + 1,
                change.largest_kg_m2_s / mean,
                change.mean_kg_m2_s / mean
            )
        })
        .collect();
    let settled = |of: fn(&SweepChange) -> f64| {
        (0..count)
            .find(|&n| diagnostics.sweeps[n..].iter().all(|c| of(c) / mean < 1e-3))
            .map_or_else(|| "never".to_owned(), |n| format!("sweep {}", n + 1))
    };
    eprintln!(
        "level {level} {name}: sweep changes ÷ mean, largest / mean's {}; under 1e-3 from {} \
         (largest), {} (mean's)",
        changes.join(", "),
        settled(|c| c.largest_kg_m2_s),
        settled(|c| c.mean_kg_m2_s)
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
    let mesh = Mesh::planar(nx, ny, 33_000.0, 33_000.0, false, label);
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
    let converged = Parameters::HERGARTEN_ROBL.with_sweeps(600);
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
        let d = solve(mesh, &world.month(), &converged).diagnostics;
        let out = d.runoff_kg_s + d.outflow_kg_s;
        eprintln!(
            "{name}: inflow {:.6e} kg/s, runoff {:.6e}, recycled {:.6e}, outflow {:.6e}; \
             closure {:.1e}",
            d.inflow_kg_s,
            d.runoff_kg_s,
            d.recycled_kg_s,
            d.outflow_kg_s,
            (d.inflow_kg_s - out) / d.inflow_kg_s
        );
        assert!(d.inflow_kg_s > 0.0 && d.recycled_kg_s > 0.0);
        assert!(
            (d.inflow_kg_s - out).abs() < 1e-12 * d.inflow_kg_s,
            "{name}"
        );
    }
    // On the sphere, at level 7, over flat land: converged, the same.
    let strip = Strip::new(7, -20.0, 1_500_000.0, 10.0, |_| 0.0);
    let d = solve(&strip.mesh, &strip.world.month(), &converged).diagnostics;
    let closure = (d.inflow_kg_s - d.runoff_kg_s - d.outflow_kg_s) / d.inflow_kg_s;
    eprintln!(
        "level 7 strip: closure {closure:.1e}, last sweep {:.1e}",
        d.last_relative_changes().largest_kg_m2_s
    );
    assert!(closure.abs() < 1e-10, "{closure}");
}

#[test]
fn the_rates_do_not_depend_on_the_cells_index_order() {
    let set = Parameters::HERGARTEN_ROBL;
    // A chain's order is its own, whichever way it is labelled: the same bits.
    let (forward, _) = chain(400, 2_500.0, ridge, &set, false);
    let (backward, _) = chain(400, 2_500.0, ridge, &set, true);
    for (&(x, p), &(_, q)) in forward.iter().zip(&backward) {
        assert_same_bits(q, p);
        assert!(p > 0.0, "at {x} m");
    }
    // A grid labelled by rows, by columns and backwards breaks its ties differently: the same
    // rates once converged.
    let converged = set.with_sweeps(400);
    let (nx, ny) = (24, 12);
    let labels: [&dyn Fn(u32, u32) -> u32; 3] =
        [&|x, y| y * nx + x, &|x, y| x * ny + y, &|x, y| {
            nx * ny - 1 - (y * nx + x)
        }];
    let mut runs = labels.iter().map(|label| {
        let (mesh, world) = planar_world(nx, ny, label, hill(8, 6), |_, _| false);
        let rates = solve(&mesh, &world.month(), &converged).precipitation_kg_m2_s;
        let mut by_place = vec![0.0; slot(nx * ny)];
        for y in 0..ny {
            for x in 0..nx {
                by_place[slot(y * nx + x)] = rates[slot(label(x, y))];
            }
        }
        by_place
    });
    let first = runs.next().unwrap();
    let largest = first.iter().copied().fold(0.0, f64::max);
    for other in runs {
        for (p, q) in first.iter().zip(&other) {
            assert!((p - q).abs() <= 1e-12 * largest, "{p} against {q}");
        }
    }
}

#[test]
fn a_cycle_is_cut_at_its_lowest_index_and_converges() {
    // A ring of three land rows round a periodic grid, sea rows beside it, fed by dispersion.
    let (nx, ny) = (40, 5);
    let converged = Parameters::HERGARTEN_ROBL.with_sweeps(300);
    let run = |shift: u32| {
        let label = move |x: u32, y: u32| y * nx + (x + shift) % nx;
        let mesh = Mesh::planar(nx, ny, 33_000.0, 33_000.0, true, label);
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
        let result = solve(&mesh, &world.month(), &converged);
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
    assert_eq!(d.cut_cells, ny - 2, "one cut a land row");
    assert_eq!(d.land_cells, nx * (ny - 2));
    let largest = first.iter().copied().fold(0.0, f64::max);
    for y in 1..ny - 1 {
        for x in 0..nx {
            let i = slot(y * nx + x);
            assert!(first[i] > 0.0);
            assert!((first[i] - second[i]).abs() <= 1e-12 * largest);
        }
    }
}

#[test]
fn a_calm_land_rains_out_its_coastal_inflow_and_nothing_inland() {
    let set = Parameters::HERGARTEN_ROBL;
    let (nx, ny) = (10, 5);
    let label = |x: u32, y: u32| y * nx + x;
    let (mesh, world) = planar_world(nx, ny, label, |_, _| 0.0, |_, _| true);
    let result = solve(&mesh, &world.month(), &set);
    // The coastal column takes the sea's L_l P_ls across each 33 km edge and keeps it all:
    // (1 − ε₀) P A = L_l P_ls ℓ.
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
    // A wholly calm month brings the land nothing.
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
    assert_same_bits(runoff_kg_m2_s(rate, 0.0, Cover::Land), 0.25 * rate);
    assert_same_bits(runoff_kg_m2_s(rate, -300.0, Cover::Land), 0.25 * rate);
    let high = runoff_kg_m2_s(rate, HEIGHT_SCALE_M, Cover::Land);
    assert!((high - (1.0 - 0.75 / core::f64::consts::E) * rate).abs() < 1e-18);
    assert_same_bits(runoff_kg_m2_s(rate, 0.0, Cover::Sea), 0.0);
    assert_same_bits(runoff_kg_m2_s(0.0, 1_000.0, Cover::Land), 0.0);
    // The solve's recycled shares are the runoff's.
    let (mesh, world) = planar_world(6, 4, |x, y| y * 6 + x, hill(2, 2), |_, _| false);
    let result = solve(&mesh, &world.month(), &Parameters::HERGARTEN_ROBL);
    for (i, &share) in result.recycled_shares.iter().enumerate() {
        let p = result.precipitation_kg_m2_s[i];
        let runoff = runoff_kg_m2_s(p, world.heights_m[i], world.cover[i]);
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

/// The cost of one month at level 8 (R09.T13.e's first part): the mesh, built once, and a month's
/// solve, on a sphere of some land and on one all land, under zonal belts that reverse at 30° and
/// 60° and under three cells whose trades and polar easterlies blow equatorward and westerlies
/// poleward; with each run's convergence.
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
            let d = &result.diagnostics;
            let last = d.last_relative_changes();
            eprintln!(
                "level 8, {name}, {winds}: mesh {built:.3} s once; a month {month:.3} s over {} \
                 land cells ({} cut), {SWEEPS} sweeps; last sweep {:.1e} (largest) and {:.1e} \
                 (mean's) of the mean",
                d.land_cells, d.cut_cells, last.largest_kg_m2_s, last.mean_kg_m2_s
            );
            if d.global_mean_kg_m2_s > 0.0 {
                print_sweeps(8, name, d);
            }
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
