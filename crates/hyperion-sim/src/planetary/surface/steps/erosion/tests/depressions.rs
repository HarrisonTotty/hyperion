//! The depressions' tests: a finite inventory filling the lowest basins first and spilling, a full
//! basin spilling to the sea through its outlet, a high basin spilling into the basin below it,
//! Priority-Flood's fill against the merge tree's when every depression is full, a world with no
//! sea storing all it is poured, and the water balance's lakes, playas and spills, a closed lake's
//! balance the X ratio among them.

use hyperion_surface::num;
use hyperion_testkit::float::assert_same_bits;

use super::super::branches::WaterBalance;
use super::super::depressions::{
    Body, Hierarchy, Measure, WaterBodies, route_with_water, water_bodies,
};
use super::super::drainage::{Routing, receiver_marks};
use super::*;
use crate::planetary::surface::reference;

/// Two basins that share a sill, on face +x: floor B at 0 m and floor A at 100 m, caps of
/// 0.15 rad about (1, ∓0.35, 0) normalised, 0.67 rad apart; the rest of the valley, within 0.6 rad
/// of (1, 0, 0), at 1,000 m, the sill between them; and everything else at 2,000 m.
struct Basins {
    graph: CellGraph,
    mesh: Mesh,
    z: Vec<f64>,
    floor_a: Vec<u32>,
    floor_b: Vec<u32>,
    /// The valley's cells: the floors and the sill, every cell at or below 1,000 m.
    valley: Vec<u32>,
    in_valley: Vec<bool>,
}

impl Basins {
    fn new(level: u8) -> Self {
        let (graph, mesh) = world(level);
        let a = cap(&graph, unit([1.0, 0.35, 0.0]), 0.15);
        let b = cap(&graph, unit([1.0, -0.35, 0.0]), 0.15);
        let in_valley = cap(&graph, unit([1.0, 0.0, 0.0]), 0.6);
        let z: Vec<f64> = (0..slot(graph.len()))
            .map(|c| {
                if b[c] {
                    0.0
                } else if a[c] {
                    100.0
                } else if in_valley[c] {
                    1_000.0
                } else {
                    2_000.0
                }
            })
            .collect();
        let (floor_a, floor_b, valley) = (cells_of(&a), cells_of(&b), cells_of(&in_valley));
        assert!(
            floor_a.len() >= 4 && floor_b.len() >= 4,
            "floors of {} and {} cells",
            floor_a.len(),
            floor_b.len()
        );
        for cells in [&floor_a, &floor_b, &valley] {
            assert!(connected(&graph, cells));
        }
        assert!(
            floor_a
                .iter()
                .all(|&c| graph.edge_neighbours(c).iter().all(|&m| !b[slot(m)])),
            "the floors do not touch"
        );
        assert!(floor_a.iter().chain(&floor_b).all(|&c| in_valley[slot(c)]));
        Self {
            graph,
            mesh,
            z,
            floor_a,
            floor_b,
            valley,
            in_valley,
        }
    }

    fn cells(&self) -> usize {
        slot(self.graph.len())
    }

    /// The area of `cells`, m².
    fn area(&self, cells: &[u32]) -> f64 {
        sum_over(self.mesh.areas_m2(), cells)
    }

    /// A's capacity to the sill, Σa × 900 m.
    fn capacity_a(&self) -> f64 {
        900.0 * self.area(&self.floor_a)
    }

    /// B's capacity to the sill, Σa × 1,000 m.
    fn capacity_b(&self) -> f64 {
        1_000.0 * self.area(&self.floor_b)
    }

    /// `volume` poured on cell `cell`, none elsewhere.
    fn poured(&self, cell: u32, volume: f64) -> Vec<f64> {
        let mut water = vec![0.0; self.cells()];
        water[slot(cell)] = volume;
        water
    }
}

/// `actual` is `expected` to 10⁻¹² of it.
#[track_caller]
fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-12 * expected.abs(),
        "{what}: {actual} against {expected}"
    );
}

/// The body whose pit is `pit`.
#[track_caller]
fn body_at(bodies: &WaterBodies, pit: u32) -> &Body {
    bodies
        .bodies()
        .iter()
        .find(|b| b.pit == pit)
        .unwrap_or_else(|| panic!("no body has pit {pit}: {:?}", bodies.bodies()))
}

/// The water held plus the water discharged is `poured`, to 10⁻¹², and every lake is level: each
/// of its cells has its level, bit for bit, and no other cell has water.
#[track_caller]
fn assert_conserved_and_level(bodies: &WaterBodies, poured: f64, cells: usize) {
    let held: f64 = bodies.bodies().iter().map(|b| b.amount).sum();
    assert_close(held + bodies.discharged(), poured, "the water");
    let mut wet = 0;
    for body in bodies.bodies() {
        for &c in &body.cells {
            assert_same_bits(bodies.water_level_m(c).unwrap(), body.level_m);
        }
        wet += body.cells.len();
    }
    let flooded = (0..cells)
        .filter(|&c| bodies.water_level_m(cell_of(c)).is_some())
        .count();
    assert_eq!(flooded, wet, "the flooded cells are the bodies'");
}

/// A finite inventory fills the lowest basins first and spills (Barnes, Callaghan and Wickert 2021,
/// eqs. 3 and 6): poured into the upper basin A (floor 100 m) of two that share a sill at 1,000 m,
/// on a world with no sea, half A's capacity makes one terminal lake on A at 100 m + V ÷ `Σa_A`, B
/// a dry playa at its floor; A's capacity and 40% of B's fill A to the sill, full, and spill the
/// rest into B, terminal at (V − `V_A`) ÷ `Σa_B`; and `V_A` + `V_B` + 250 m × the valley's area
/// rises over the shared sill as one lake, at 1,000 m + (V − `V_A` − `V_B`) ÷ Σa of the valley.
/// Each floor's pit lies under a node that spills at the sill; each level matches its closed form
/// to 10⁻¹², each lake's cells are exactly the cells below it, nothing is discharged, and the water
/// is conserved and level.
#[test]
fn a_finite_inventory_fills_the_lowest_basins_first_and_spills() {
    let basins = Basins::new(4);
    let (mesh, z) = (&basins.mesh, &basins.z);
    let hierarchy = Hierarchy::build(mesh, z, &vec![false; basins.cells()]);
    let (pit_a, pit_b) = (basins.floor_a[0], basins.floor_b[0]);
    // A flat floor's cells start several leaves, which merge at the floor's own height; the node
    // they merge into spills at the sill.
    for (pit, floor_m) in [(pit_a, 100.0), (pit_b, 0.0)] {
        let mut node = hierarchy.node(hierarchy.holder(pit).unwrap());
        assert_eq!(node.pit(), pit);
        while node.spill().unwrap().level_m <= floor_m {
            node = hierarchy.node(node.parent().unwrap());
            assert_eq!(node.pit(), pit);
        }
        let spill = node.spill().unwrap();
        assert_same_bits(spill.level_m, 1_000.0);
        assert_same_bits(z[slot(spill.outlet)], 1_000.0);
        assert!(node.parent().is_some(), "a basin below the sill merges");
    }
    let (cap_a, cap_b) = (basins.capacity_a(), basins.capacity_b());
    let (area_a, area_b) = (basins.area(&basins.floor_a), basins.area(&basins.floor_b));
    let fill = |volume: f64| {
        let water = basins.poured(pit_a, volume);
        let bodies = hierarchy.fill_spill_merge(mesh, Measure::Volume { water_m3: &water });
        assert_conserved_and_level(&bodies, volume, basins.cells());
        assert_same_bits(bodies.discharged(), 0.0);
        bodies
    };

    let half = fill(0.5 * cap_a);
    assert_eq!(half.bodies().len(), 2, "{:?}", half.bodies());
    let lake = body_at(&half, pit_a);
    assert!(!lake.full && !lake.is_playa());
    assert_close(lake.level_m, 100.0 + 0.5 * cap_a / area_a, "A's lake");
    assert_eq!(lake.cells, basins.floor_a);
    let playa = body_at(&half, pit_b);
    assert!(playa.is_playa(), "B is dry: {playa:?}");
    assert_same_bits(playa.base_level_m().unwrap(), 0.0);

    let spilled = fill(cap_a + 0.4 * cap_b);
    assert_eq!(spilled.bodies().len(), 2);
    let upper = body_at(&spilled, pit_a);
    assert!(upper.full && upper.base_level_m().is_none());
    assert_same_bits(upper.level_m, 1_000.0);
    assert_close(upper.amount, cap_a, "A full");
    assert_eq!(upper.cells, basins.floor_a, "A's sill stays dry");
    let lower = body_at(&spilled, pit_b);
    assert!(!lower.full);
    assert_close(lower.level_m, 0.4 * cap_b / area_b, "B's lake");
    assert_eq!(lower.cells, basins.floor_b);

    let over = 250.0 * basins.area(&basins.valley);
    let merged = fill(cap_a + cap_b + over);
    assert_eq!(merged.bodies().len(), 1);
    let lake = body_at(&merged, pit_b);
    assert!(!lake.full);
    assert!(
        lake.level_m > 1_000.0,
        "the merged lake rises over the sill"
    );
    assert_close(
        lake.level_m,
        1_000.0 + over / basins.area(&basins.valley),
        "the merged lake",
    );
    assert_eq!(lake.cells, basins.valley);
}

/// Whether the receivers of `cell` lead out of the cells `inside` by `exit`, following them from
/// `cell` until they leave.
fn leaves_by(routing: &Routing, inside: &[bool], cell: u32, exit: u32) -> bool {
    let mut x = cell;
    while inside[slot(x)] {
        match routing.receiver(x) {
            Some(link) => x = link.cell,
            None => return false,
        }
    }
    x == exit
}

/// A full basin spills its excess to the sea through its outlet: with everything beyond a
/// one-cell rim at 2,000 m around the valley a sea at −500 m, and one notch in the rim at 1,200 m,
/// twice the valley's capacity rained on it leaves one full body at the notch's height exactly,
/// holding the capacity Σ a (1,200 m − z) over the valley (to 10⁻¹²), its cells the valley's, and
/// discharges the rest; the routing's roots are the sea's cells, the valley is filled flat to the
/// notch, and every valley cell's receivers lead out through it. Half A's capacity poured on A
/// instead leaves a terminal lake on A whose pit is a root, to which A's floor drains.
#[test]
fn a_full_basin_spills_its_excess_to_the_sea_through_its_outlet() {
    let basins = Basins::new(4);
    let (graph, mesh) = (&basins.graph, &basins.mesh);
    let ring = rim(graph, &basins.in_valley);
    let mut shore = basins.in_valley.clone();
    for &c in &ring {
        shore[slot(c)] = true;
    }
    let notch = *ring
        .iter()
        .find(|&&c| graph.edge_neighbours(c).iter().any(|&m| !shore[slot(m)]))
        .unwrap();
    let mut z = basins.z.clone();
    for (c, h) in z.iter_mut().enumerate() {
        if !shore[c] {
            *h = -500.0;
        }
    }
    z[slot(notch)] = 1_200.0;
    let sea: Vec<bool> = shore.iter().map(|&s| !s).collect();
    let marks = receiver_marks(reference::SEED, graph);
    let capacity: f64 = basins
        .valley
        .iter()
        .map(|&c| mesh.area_m2(c) * (1_200.0 - z[slot(c)]))
        .sum();
    let valley_area = basins.area(&basins.valley);
    let mut water = vec![0.0; basins.cells()];
    for &c in &basins.valley {
        water[slot(c)] = 2.0 * capacity * mesh.area_m2(c) / valley_area;
    }
    let poured: f64 = water.iter().sum();
    let bodies = water_bodies(mesh, &z, &sea, Measure::Volume { water_m3: &water });
    let routing = route_with_water(mesh, &z, &sea, SEA_LEVEL_M, &bodies, &marks);
    assert_conserved_and_level(&bodies, poured, basins.cells());
    assert_eq!(bodies.bodies().len(), 1, "{:?}", bodies.bodies());
    let body = &bodies.bodies()[0];
    assert!(body.full);
    assert_same_bits(body.level_m, 1_200.0);
    assert_close(body.amount, capacity, "the valley's capacity");
    assert_eq!(body.cells, basins.valley);
    assert_close(bodies.discharged(), poured - capacity, "the discharge");
    assert_eq!(routing.roots(), cells_of(&sea));
    for &c in &basins.valley {
        assert_same_bits(routing.filled_m()[slot(c)], 1_200.0);
        assert!(
            leaves_by(&routing, &basins.in_valley, c, notch),
            "valley cell {c} drains out by the notch"
        );
    }

    let pit_a = basins.floor_a[0];
    let volume = 0.5 * basins.capacity_a();
    let water = basins.poured(pit_a, volume);
    let bodies = water_bodies(mesh, &z, &sea, Measure::Volume { water_m3: &water });
    let routing = route_with_water(mesh, &z, &sea, SEA_LEVEL_M, &bodies, &marks);
    assert_conserved_and_level(&bodies, volume, basins.cells());
    let lake = body_at(&bodies, pit_a);
    assert!(!lake.full);
    assert!(routing.roots().binary_search(&pit_a).is_ok());
    for &c in &basins.floor_a {
        assert_same_bits(routing.filled_m()[slot(c)], lake.level_m);
        assert_eq!(
            routing.root_of(c),
            pit_a,
            "A's floor cell {c} drains to its lake's pit"
        );
        assert_same_bits(routing.base_level_m(c), lake.level_m);
    }
}

/// A high basin spills into the basin below it, not the sea (Barnes, Callaghan and Wickert 2020's
/// oceanlink): at level 5, the valley's rim has a notch at 1,200 m onto a plain at 1,100 m that
/// drains to a distant sea; a basin H just outside the rim, its floor at 1,500 m, walled at
/// 2,000 m, has its outlet at 1,800 m in the valley's rim. H's capacity, A's, B's and 100 m over
/// the valley, poured on H, leave H full at its outlet's height and the valley's lake at 1,100 m,
/// to 10⁻¹², with nothing discharged.
#[test]
fn a_high_basin_spills_into_the_basin_below_it() {
    let basins = Basins::new(5);
    let (graph, mesh) = (&basins.graph, &basins.mesh);
    let n = basins.cells();
    let ring = rim(graph, &basins.in_valley);
    let mut walled = basins.in_valley.clone();
    for &c in &ring {
        walled[slot(c)] = true;
    }
    let h_floor: Vec<bool> = cap(graph, unit([0.75, 0.0, 0.66]), 0.06)
        .iter()
        .zip(&walled)
        .map(|(&inside, &w)| inside && !w)
        .collect();
    let h_cells = cells_of(&h_floor);
    assert!(!h_cells.is_empty() && connected(graph, &h_cells));
    let h_rim = rim(graph, &h_floor);
    let h_notch = *h_rim
        .iter()
        .find(|&&c| walled[slot(c)] && !basins.in_valley[slot(c)])
        .expect("H's floor touches the valley's rim");
    let far = unit([0.8, 0.0, -0.6]);
    let notch = *ring
        .iter()
        .filter(|&&c| !h_rim.contains(&c))
        .max_by(|&&a, &&b| dot(graph.centre(a), far).total_cmp(&dot(graph.centre(b), far)))
        .unwrap();
    let sea = cap(graph, unit([-1.0, 0.0, 0.0]), 1.5);
    let mut z = vec![1_100.0; n];
    for c in 0..graph.len() {
        let ci = slot(c);
        z[ci] = if basins.in_valley[ci] {
            basins.z[ci]
        } else if c == h_notch {
            1_800.0
        } else if c == notch {
            1_200.0
        } else if walled[ci] || h_rim.contains(&c) {
            2_000.0
        } else if h_floor[ci] {
            1_500.0
        } else if sea[ci] {
            -500.0
        } else {
            1_100.0
        };
    }
    // Its lowest neighbour on the ocean's side is then a cell of the valley, at 1,000 m, below
    // the plain, and no sea cell, which the walk takes first.
    assert!(
        graph
            .edge_neighbours(h_notch)
            .iter()
            .all(|&m| !sea[slot(m)]),
        "H's outlet does not touch the sea"
    );
    let cap_h = 300.0 * basins.area(&h_cells);
    let over = 100.0 * basins.area(&basins.valley);
    let volume = cap_h + basins.capacity_a() + basins.capacity_b() + over;
    let water = basins.poured(h_cells[0], volume);
    let bodies = water_bodies(mesh, &z, &sea, Measure::Volume { water_m3: &water });
    assert_conserved_and_level(&bodies, volume, n);
    assert_same_bits(bodies.discharged(), 0.0);
    let wet = bodies.bodies().iter().filter(|b| !b.is_playa()).count();
    assert_eq!(wet, 2, "{:?}", bodies.bodies());
    let high = body_at(&bodies, h_cells[0]);
    assert!(high.full);
    assert_same_bits(high.level_m, 1_800.0);
    assert_close(high.amount, cap_h, "H's capacity");
    assert_eq!(high.cells, h_cells);
    let low = body_at(&bodies, basins.floor_b[0]);
    assert!(!low.full);
    assert_close(low.level_m, 1_100.0, "the valley's lake");
    assert_eq!(low.cells, basins.valley);
}

/// Priority-Flood's fill equals the merge tree's when every depression is full (Barnes, Callaghan
/// and Wickert 2020 §6.1): on the rough field at level 5 under 10⁵ m of rain, every water body is
/// full at its tree's spill level, the routing's roots are the sea's cells, and each land cell's
/// filled level is, bit for bit, its tree's spill level where its ground is below it, and its
/// ground elsewhere; the water is conserved and level.
#[test]
fn priority_flood_fills_as_the_merge_tree_when_every_depression_is_full() {
    let (graph, mesh) = world(5);
    let (z, sea) = rough_terrain(&graph, 2);
    let water = rain(&mesh, 1e5);
    let bodies = water_bodies(&mesh, &z, &sea, Measure::Volume { water_m3: &water });
    let routing = route_with_water(
        &mesh,
        &z,
        &sea,
        SEA_LEVEL_M,
        &bodies,
        &receiver_marks(reference::SEED, &graph),
    );
    assert_conserved_and_level(&bodies, water.iter().sum(), slot(graph.len()));
    let hierarchy = Hierarchy::build(&mesh, &z, &sea);
    eprintln!("{} bodies", bodies.bodies().len());
    assert!(bodies.bodies().len() > 1, "the field has depressions");
    for body in bodies.bodies() {
        assert!(body.full, "{body:?}");
        let node = hierarchy.node(body.node);
        assert!(node.parent().is_none(), "a full body is its tree's root");
        assert_same_bits(body.level_m, node.spill().unwrap().level_m);
    }
    assert_eq!(routing.roots(), cells_of(&sea));
    for c in (0..graph.len()).filter(|&c| !sea[slot(c)]) {
        let ground = z[slot(c)];
        let expected = hierarchy.holder(c).map_or(ground, |mut k| {
            while let Some(p) = hierarchy.node(k).parent() {
                k = p;
            }
            let spill = hierarchy.node(k).spill().unwrap().level_m;
            if ground < spill { spill } else { ground }
        });
        assert_same_bits(routing.filled_m()[slot(c)], expected);
    }
}

/// A world with no sea stores all it is poured (decision-r09-t14-choices Q2): on the rough field
/// at level 5 with no sea cell, a uniform 50 m of water is all held, to 10⁻¹², and none
/// discharged; and 5 km, deeper than the field's relief, fills the sphere's root, the one body,
/// terminal with no spill, over every cell, at 5 km plus the area-weighted mean ground (to 10⁻¹²).
#[test]
fn a_world_with_no_sea_stores_all_it_is_poured() {
    let (graph, mesh) = world(5);
    let (z, _) = rough_terrain(&graph, 4);
    let dry = vec![false; slot(graph.len())];
    for depth_m in [50.0, 5_000.0] {
        let water = rain(&mesh, depth_m);
        let bodies = water_bodies(&mesh, &z, &dry, Measure::Volume { water_m3: &water });
        assert_conserved_and_level(&bodies, water.iter().sum(), slot(graph.len()));
        assert_same_bits(bodies.discharged(), 0.0);
        eprintln!("{depth_m} m: {} bodies", bodies.bodies().len());
    }
    let water = rain(&mesh, 5_000.0);
    let hierarchy = Hierarchy::build(&mesh, &z, &dry);
    let bodies = hierarchy.fill_spill_merge(&mesh, Measure::Volume { water_m3: &water });
    assert_eq!(bodies.bodies().len(), 1, "{} bodies", bodies.bodies().len());
    let sphere = &bodies.bodies()[0];
    let root = hierarchy.node(sphere.node);
    assert!(root.parent().is_none() && root.spill().is_none());
    assert!(!sphere.full);
    let areas = mesh.areas_m2();
    let area: f64 = areas.iter().sum();
    let mean: f64 = areas.iter().zip(&z).map(|(a, h)| a * h).sum::<f64>() / area;
    assert_close(sphere.level_m, 5_000.0 + mean, "the sphere's level");
    assert_eq!(sphere.cells, (0..graph.len()).collect::<Vec<u32>>());
}

/// A bowl for the water balance's tests, at level 4: a paraboloid floor rising from about 0 m to
/// 800 m over 0.4 rad about (1, 0.1, −0.2) normalised, behind a one-cell rim at 2,000 m with one
/// notch at 1,200 m onto a sea at −500 m beyond, every cell losing `net_loss` metres a year over
/// open liquid.
struct Bowl {
    graph: CellGraph,
    mesh: Mesh,
    z: Vec<f64>,
    sea: Vec<bool>,
    /// The bowl's cells, ascending.
    cells: Vec<u32>,
    /// The bowl's cells in their fill order, by (ground, cell index).
    fill_order: Vec<u32>,
    net_loss: Vec<f64>,
}

impl Bowl {
    fn new(net_loss_m_per_year: f64) -> Self {
        let (graph, mesh) = world(4);
        let centre = unit([1.0, 0.1, -0.2]);
        let inside = cap(&graph, centre, 0.4);
        let ring = rim(&graph, &inside);
        let mut shore = inside.clone();
        for &c in &ring {
            shore[slot(c)] = true;
        }
        let notch = *ring
            .iter()
            .find(|&&c| graph.edge_neighbours(c).iter().any(|&m| !shore[slot(m)]))
            .unwrap();
        let rise = 800.0 / (1.0 - math::cos(0.4));
        let z: Vec<f64> = (0..graph.len())
            .map(|c| {
                if inside[slot(c)] {
                    rise * (1.0 - dot(graph.centre(c), centre))
                } else if c == notch {
                    1_200.0
                } else if shore[slot(c)] {
                    2_000.0
                } else {
                    -500.0
                }
            })
            .collect();
        let sea: Vec<bool> = shore.iter().map(|&s| !s).collect();
        let cells = cells_of(&inside);
        let mut fill_order = cells.clone();
        fill_order.sort_by(|&a, &b| z[slot(a)].total_cmp(&z[slot(b)]).then(a.cmp(&b)));
        let hierarchy = Hierarchy::build(&mesh, &z, &sea);
        let leaf = hierarchy.holder(fill_order[0]);
        assert!(
            cells.iter().all(|&c| hierarchy.holder(c) == leaf),
            "the bowl is one depression"
        );
        let net_loss = vec![net_loss_m_per_year; slot(graph.len())];
        Self {
            graph,
            mesh,
            z,
            sea,
            cells,
            fill_order,
            net_loss,
        }
    }

    fn pit(&self) -> u32 {
        self.fill_order[0]
    }

    /// The bowl's loss at its sill, Σ (E − P) a over its cells, m³ a⁻¹.
    fn loss_at_sill(&self) -> f64 {
        self.cells
            .iter()
            .map(|&c| self.net_loss[slot(c)] * self.mesh.area_m2(c))
            .sum()
    }

    /// The water bodies of `inflow` m³ a⁻¹ entering at the pit.
    fn balance(&self, inflow: f64) -> WaterBodies {
        let mut inflows = vec![0.0; slot(self.graph.len())];
        inflows[slot(self.pit())] = inflow;
        let bodies = water_bodies(
            &self.mesh,
            &self.z,
            &self.sea,
            Measure::Balance {
                inflow_m3_per_year: &inflows,
                net_loss_m_per_year: &self.net_loss,
            },
        );
        assert_conserved_and_level(&bodies, inflow, slot(self.graph.len()));
        bodies
    }
}

/// A water-balance lake takes whole cells in fill order and is level (decision-r09-t14-choices
/// Q3): in the bowl losing 1 m a⁻¹, an inflow of 30% of its loss at the sill makes one terminal
/// lake holding the inflow, which takes the bowl's cells in their fill order, each while the
/// loss of those taken and half its own fit within the inflow (the half-cell rule), so that its
/// loss is within half a cell's of the inflow; its level is the ground of the first cell it does
/// not take, every cell it takes lies under it, each at the level bit for bit, and nothing is
/// discharged.
#[test]
fn a_balance_lake_takes_whole_cells_in_fill_order_and_is_level() {
    let bowl = Bowl::new(1.0);
    let inflow = 0.3 * bowl.loss_at_sill();
    let bodies = bowl.balance(inflow);
    assert_same_bits(bodies.discharged(), 0.0);
    assert_eq!(bodies.bodies().len(), 1, "{:?}", bodies.bodies());
    let lake = body_at(&bodies, bowl.pit());
    assert!(!lake.full && !lake.is_playa());
    assert_same_bits(lake.amount, inflow);
    let (mut loss, mut taken) = (0.0, 0);
    for &c in &bowl.fill_order {
        let cell_loss = bowl.mesh.area_m2(c);
        if loss + 0.5 * cell_loss > inflow {
            break;
        }
        loss += cell_loss;
        taken += 1;
    }
    assert!(
        taken >= 2 && taken < bowl.cells.len(),
        "{taken} cells taken"
    );
    let next = bowl.fill_order[taken];
    assert!((loss - inflow).abs() <= 0.5 * bowl.mesh.area_m2(next));
    assert_same_bits(lake.level_m, bowl.z[slot(next)]);
    assert_same_bits(lake.base_level_m().unwrap(), lake.level_m);
    let mut expected = bowl.fill_order[..taken].to_vec();
    expected.sort_unstable();
    assert_eq!(lake.cells, expected);
}

/// A lake whose inflow cannot cover half its pit's loss is a dry playa: in the bowl losing
/// 1 m a⁻¹, an inflow of 40% of the pit cell's area in m³ a⁻¹ leaves one terminal body with no
/// cell under it, its level and base level the pit's ground; the routing with it has the pit for
/// a root, and every bowl cell drains to it at its own ground, unfilled, the pit's ground its
/// basin's base level.
#[test]
fn a_lake_that_cannot_take_half_its_pit_is_a_dry_playa() {
    let bowl = Bowl::new(1.0);
    let pit = bowl.pit();
    let bodies = bowl.balance(0.4 * bowl.mesh.area_m2(pit));
    assert_eq!(bodies.bodies().len(), 1, "{:?}", bodies.bodies());
    let playa = body_at(&bodies, pit);
    assert!(playa.is_playa());
    assert_same_bits(playa.level_m, bowl.z[slot(pit)]);
    assert_same_bits(playa.base_level_m().unwrap(), bowl.z[slot(pit)]);
    let routing = route_with_water(
        &bowl.mesh,
        &bowl.z,
        &bowl.sea,
        SEA_LEVEL_M,
        &bodies,
        &receiver_marks(reference::SEED, &bowl.graph),
    );
    assert!(routing.roots().binary_search(&pit).is_ok());
    for &c in &bowl.cells {
        assert_same_bits(routing.filled_m()[slot(c)], bowl.z[slot(c)]);
        assert_eq!(routing.root_of(c), pit, "bowl cell {c}");
        assert_same_bits(routing.base_level_m(c), bowl.z[slot(pit)]);
    }
}

/// A water-balance lake spills once its inflow reaches its loss at the sill: in the bowl losing
/// 1 m a⁻¹, an inflow a millionth short of the loss at the sill (Σ (E − P) a over the bowl) keeps
/// it terminal, discharging nothing; one and a half times it fills the bowl to the notch's height
/// exactly, full, over every bowl cell, taking the loss at the sill and discharging the rest to the
/// sea (to 10⁻¹²).
#[test]
fn a_balance_lake_spills_once_its_inflow_reaches_its_loss_at_the_sill() {
    let bowl = Bowl::new(1.0);
    let sill = bowl.loss_at_sill();
    let short = bowl.balance((1.0 - 1e-6) * sill);
    let lake = body_at(&short, bowl.pit());
    assert!(!lake.full);
    assert_same_bits(short.discharged(), 0.0);
    let inflow = 1.5 * sill;
    let spilling = bowl.balance(inflow);
    let lake = body_at(&spilling, bowl.pit());
    assert!(lake.full);
    assert_same_bits(lake.level_m, 1_200.0);
    assert_close(lake.amount, sill, "the loss at the sill");
    assert_close(spilling.discharged(), inflow - sill, "the spill");
    assert_eq!(lake.cells, bowl.cells);
}

/// A net gain over a lake spills more than its inflow (decision-r09-t14-choices Q3, P > E): in the
/// bowl gaining 0.5 m a⁻¹ over open liquid, an inflow of 0.1 m a⁻¹ times its area fills it to the
/// notch, full, its loss at the sill −0.5 m a⁻¹ times its area, and discharges the inflow and that
/// gain, more than the inflow (to 10⁻¹²).
#[test]
fn a_net_gain_over_a_lake_spills_more_than_its_inflow() {
    let bowl = Bowl::new(-0.5);
    let area = sum_over(bowl.mesh.areas_m2(), &bowl.cells);
    let inflow = 0.1 * area;
    let bodies = bowl.balance(inflow);
    let lake = body_at(&bodies, bowl.pit());
    assert!(lake.full);
    assert_same_bits(lake.level_m, 1_200.0);
    assert_close(lake.amount, -0.5 * area, "the gain at the sill");
    assert!(bodies.discharged() > inflow);
    assert_close(bodies.discharged(), inflow + 0.5 * area, "the spill");
    assert_eq!(lake.cells, bowl.cells);
}

/// A closed lake's balance is the X ratio of closed-basin hydrology (decision-r09-t14-choices Q3;
/// Matsubara, Howard and Drummond 2011's X = (E − P) ÷ R, R the runoff from the land outside the
/// lake): in the bowl, its cells shedding r = 0.2 m a⁻¹ of runoff and its open liquid losing
/// E − P = 1 m a⁻¹, X = 5, Matsubara et al.'s modal value, the balance
/// (`WaterBalance::from_rates`: each cell's net loss (E − P) + r) makes one terminal lake whose
/// area `A_L` ÷ (`A_B` − `A_L`), `A_B` the bowl's, is r ÷ (E − P) to the half-cell rule: `A_L`
/// lies within half the largest bowl cell's area of r `A_B` ÷ (E − P + r), and not within it of
/// r `A_B` ÷ (E − P), the lake that counting a flooded cell's runoff as its inflow too would take.
#[test]
fn a_closed_lake_s_balance_is_the_x_ratio() {
    let bowl = Bowl::new(1.0);
    let n = slot(bowl.graph.len());
    let (runoff, open_liquid) = (0.2, 1.0);
    let mut runoffs = vec![0.0; n];
    for &c in &bowl.cells {
        runoffs[slot(c)] = runoff;
    }
    let balance = WaterBalance::from_rates(bowl.mesh.areas_m2(), &runoffs, &vec![open_liquid; n]);
    let bodies = water_bodies(&bowl.mesh, &bowl.z, &bowl.sea, balance.measure());
    assert_eq!(bodies.bodies().len(), 1, "{:?}", bodies.bodies());
    let lake = body_at(&bodies, bowl.pit());
    assert!(!lake.full && !lake.is_playa(), "{lake:?}");
    let areas = bowl.mesh.areas_m2();
    let basin = sum_over(areas, &bowl.cells);
    let area = sum_over(areas, &lake.cells);
    let largest = bowl
        .cells
        .iter()
        .fold(0.0, |most, &c| num::max(most, bowl.mesh.area_m2(c)));
    let expected = runoff * basin / (open_liquid + runoff);
    let double_counted = runoff * basin / open_liquid;
    eprintln!(
        "the bowl's lake: {} of {} cells, A_L ÷ (A_B − A_L) = {:.4} against r ÷ (E − P) = {:.4}; \
         A_L {area:.4e} m² against {expected:.4e} m² (the double count's {double_counted:.4e} m²), \
         half the largest cell {:.4e} m²",
        lake.cells.len(),
        bowl.cells.len(),
        area / (basin - area),
        runoff / open_liquid,
        0.5 * largest
    );
    assert!(
        (area - expected).abs() <= 0.5 * largest,
        "the lake's area {area} m² against r A_B ÷ (E − P + r) = {expected} m²"
    );
    assert!(
        (area - double_counted).abs() > 0.5 * largest,
        "the bowl tells the X ratio from the double count: {area} m² against {double_counted} m²"
    );
}
