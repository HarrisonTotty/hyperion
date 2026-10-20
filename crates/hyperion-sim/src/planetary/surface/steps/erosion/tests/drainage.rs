//! The drainage's tests: the outlets' drainage area, no cell draining uphill, the same seed's
//! receivers, flats without ε, the flood against a minimax oracle, and the receivers' draw in
//! proportion to the drop per metre.

use hyperion_surface::cube::Edge;
use hyperion_surface::field::FlowDirection;
use hyperion_surface::num;
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

use super::super::depressions::{Measure, WaterBodies, route_with_water, water_bodies};
use super::super::drainage::{Routing, Seed, accumulate, receiver_uniforms, route};
use super::*;
use crate::planetary::hooks::SurfaceSeed;
use crate::planetary::surface::reference;

/// The rough field at `level` (LCG seed `field`), rained on 0.3 m deep, routed with its water
/// under `seed`'s receivers.
fn rough_drainage(
    level: u8,
    field: u64,
    seed: SurfaceSeed,
) -> (CellGraph, Mesh, Vec<f64>, Routing, WaterBodies) {
    let (graph, mesh) = world(level);
    let (z, sea) = rough_terrain(&graph, field);
    let water = rain(&mesh, 0.3);
    let bodies = water_bodies(&mesh, &z, &sea, Measure::Volume { water_m3: &water });
    let uniforms = receiver_uniforms(seed, &graph);
    let routing = route_with_water(&mesh, &z, &sea, SEA_LEVEL_M, &bodies, &uniforms);
    (graph, mesh, z, routing, bodies)
}

/// The drainage area at every outlet sums to the draining area: on the rough field at level 5,
/// with a sea and terminal lakes, the roots are exactly the sea's cells and the terminal lakes'
/// pits, each root's base level is the sea's or its lake's, the geometric accumulation of the
/// cells' areas at the roots sums to the whole surface's area to 10⁻¹² of it, and every cell's
/// drainage area holds at least its own.
#[test]
fn the_drainage_area_at_the_outlets_sums_to_the_draining_area() {
    let (graph, mesh, z, routing, bodies) = rough_drainage(5, 1, reference::SEED);
    let sea_cells = z.iter().filter(|&&h| h < SEA_LEVEL_M).count();
    let terminal: Vec<_> = bodies.bodies().iter().filter(|b| !b.full).collect();
    let full = bodies.bodies().len() - terminal.len();
    eprintln!(
        "{sea_cells} sea cells, {} terminal and {full} full bodies of {} cells",
        terminal.len(),
        graph.len()
    );
    assert!(
        sea_cells > 0 && !terminal.is_empty(),
        "the field has a sea and lakes"
    );
    assert_eq!(routing.roots().len(), sea_cells + terminal.len());
    for body in &terminal {
        assert_eq!(routing.root_of(body.pit), body.pit);
        assert_same_bits(routing.base_level_m(body.pit), body.base_level_m().unwrap());
    }
    for &root in routing.roots() {
        if z[slot(root)] < SEA_LEVEL_M {
            assert_same_bits(routing.base_level_m(root), SEA_LEVEL_M);
        }
    }
    let area = accumulate(&routing, mesh.areas_m2());
    let at_outlets: f64 = routing.roots().iter().map(|&r| area[slot(r)]).sum();
    let total: f64 = mesh.areas_m2().iter().sum();
    assert!(
        (at_outlets - total).abs() <= 1e-12 * total,
        "{at_outlets} m² at the outlets against {total} m²"
    );
    for c in 0..graph.len() {
        assert!(area[slot(c)] >= mesh.area_m2(c), "cell {c}");
    }
}

/// No cell drains uphill after filling: on the rough field with lakes, the filled surface is at or
/// above the ground; every receiver is the edge neighbour its edge names, at its link's length,
/// coded by its flow direction, and no higher on the filled surface; the slope correction lies in
/// [0, 1]; and the receivers are a forest over the seeds: the preorder holds every cell once, each
/// after its receiver at its receiver's depth plus one, each cell's donors ascend and name it, and
/// every chain of receivers ends after its depth's steps at a root, the cell's basin, whose filled
/// level is the basin's base level.
#[test]
fn no_cell_drains_uphill_after_filling() {
    let (graph, mesh, z, routing, _) = rough_drainage(5, 1, reference::SEED);
    let n = routing.len();
    assert_eq!(n, graph.len());
    let filled = routing.filled_m();
    let mut position = vec![usize::MAX; slot(n)];
    for (at, &c) in routing.preorder().iter().enumerate() {
        assert_eq!(
            position[slot(c)],
            usize::MAX,
            "cell {c} twice in the preorder"
        );
        position[slot(c)] = at;
    }
    let mut donor_count = 0;
    for c in 0..n {
        let ci = slot(c);
        assert!(
            filled[ci] >= z[ci],
            "cell {c}: filled {} below ground {}",
            filled[ci],
            z[ci]
        );
        assert!((0.0..=1.0).contains(&routing.correction(c)), "cell {c}");
        if let Some(link) = routing.receiver(c) {
            let receiver = link.cell;
            let edge = Edge::ALL.iter().position(|&e| e == link.edge).unwrap();
            assert_eq!(graph.edge_neighbour(c, link.edge), receiver);
            assert_same_bits(link.length_m, mesh.link_m(c, edge));
            assert_eq!(routing.flow(c), FlowDirection::from(link.edge));
            assert!(
                filled[slot(receiver)] <= filled[ci],
                "cell {c} drains up to {receiver}"
            );
            assert!(
                position[slot(receiver)] < position[ci],
                "cell {c} before its receiver"
            );
            assert_eq!(routing.depth(c), routing.depth(receiver) + 1);
            assert!(routing.donors(receiver).binary_search(&c).is_ok());
        } else {
            assert!(routing.roots().binary_search(&c).is_ok(), "cell {c}");
            assert_eq!(routing.depth(c), 0);
            assert_eq!(routing.flow(c), FlowDirection::Terminal);
        }
        let donors = routing.donors(c);
        assert!(
            donors.windows(2).all(|pair| pair[0] < pair[1]),
            "cell {c}'s donors"
        );
        for &donor in donors {
            assert_eq!(routing.receiver(donor).map(|l| l.cell), Some(c));
        }
        donor_count += donors.len();
        let mut root = c;
        for _ in 0..routing.depth(c) {
            root = routing.receiver(root).unwrap().cell;
        }
        assert!(
            routing.receiver(root).is_none(),
            "cell {c}'s chain ends at a root"
        );
        assert_eq!(routing.root_of(c), root);
        assert_same_bits(routing.base_level_m(c), filled[slot(root)]);
    }
    assert_eq!(donor_count, slot(n) - routing.roots().len());
}

/// The same seed gives the same receivers: one surface seed's receiver uniforms lie in [0, 1) and
/// are the same bits twice, and so is the drainage drawn with them (receivers, link lengths, slope
/// corrections, filled surface and order); another seed's uniforms send some cell to another
/// receiver.
#[test]
fn the_same_seed_gives_the_same_receivers() {
    let (graph, _) = world(4);
    let (first_uniforms, second_uniforms) = (
        receiver_uniforms(reference::SEED, &graph),
        receiver_uniforms(reference::SEED, &graph),
    );
    for (a, b) in first_uniforms.iter().zip(&second_uniforms) {
        assert!((0.0..1.0).contains(a));
        assert_same_bits(*a, *b);
    }
    let (_, _, _, first, _) = rough_drainage(4, 3, reference::SEED);
    let (_, _, _, second, _) = rough_drainage(4, 3, reference::SEED);
    for c in 0..first.len() {
        let (a, b) = (first.receiver(c), second.receiver(c));
        assert_eq!(a.map(|l| (l.cell, l.edge)), b.map(|l| (l.cell, l.edge)));
        if let (Some(a), Some(b)) = (a, b) {
            assert_same_bits(a.length_m, b.length_m);
        }
        assert_same_bits(first.correction(c), second.correction(c));
        assert_same_bits(first.filled_m()[slot(c)], second.filled_m()[slot(c)]);
    }
    assert_eq!(first.preorder(), second.preorder());
    let other_seed = SurfaceSeed::new(reference::SEED.get() ^ 0x9e37_79b9_7f4a_7c15);
    let (_, _, _, other, _) = rough_drainage(4, 3, other_seed);
    let moved = (0..first.len())
        .filter(|&c| other.receiver(c).map(|l| l.cell) != first.receiver(c).map(|l| l.cell))
        .count();
    eprintln!("{moved} of {} cells take another receiver", first.len());
    assert!(moved > 0, "another seed moves no receiver");
}

/// Flats drain without ε: a flat floor at 0 m behind a one-cell rim at 500 m with one notch at
/// 300 m, the outside at 100 m and its cells the seeds, fills to the notch's height exactly; every
/// floor cell drains to a cell at that same level with a slope correction of 1, and its chain of
/// receivers leaves the floor through the notch, which drains down to the outside: the flood's
/// breadth-first parent links, no cell raised by an ε.
#[test]
fn flats_drain_through_their_spill_without_epsilon() {
    let (graph, mesh) = world(4);
    let floor = cap(&graph, unit([1.0, 0.2, 0.3]), 0.25);
    let floor_cells = cells_of(&floor);
    assert!(floor_cells.len() >= 8, "{} floor cells", floor_cells.len());
    assert!(connected(&graph, &floor_cells));
    let ring = rim(&graph, &floor);
    let mut inner = floor.clone();
    for &c in &ring {
        inner[slot(c)] = true;
    }
    let notch = *ring
        .iter()
        .find(|&&c| graph.edge_neighbours(c).iter().any(|&m| !inner[slot(m)]))
        .unwrap();
    let z: Vec<f64> = (0..graph.len())
        .map(|c| {
            if floor[slot(c)] {
                0.0
            } else if c == notch {
                300.0
            } else if inner[slot(c)] {
                500.0
            } else {
                100.0
            }
        })
        .collect();
    let seeds: Vec<Seed> = (0..graph.len())
        .filter(|&c| !inner[slot(c)])
        .map(|c| Seed {
            cell: c,
            level_m: z[slot(c)],
        })
        .collect();
    let routing = route(
        &mesh,
        &z,
        &seeds,
        &receiver_uniforms(reference::SEED, &graph),
    );
    let w = routing.filled_m();
    for &c in &floor_cells {
        assert_same_bits(w[slot(c)], 300.0);
        let link = routing.receiver(c).unwrap();
        assert_same_bits(w[slot(link.cell)], 300.0);
        assert_same_bits(routing.correction(c), 1.0);
        let mut x = c;
        while floor[slot(x)] {
            x = routing.receiver(x).unwrap().cell;
        }
        assert_eq!(x, notch, "floor cell {c} leaves the floor by {x}");
    }
    let out = routing.receiver(notch).unwrap().cell;
    assert!(!inner[slot(out)]);
    assert_same_bits(w[slot(out)], 100.0);
}

/// The flood's filled surface is the minimax level from the seeds: on a random field of up to
/// 1 km at level 3, with every 64th cell a seed up to 200 m above its ground, it equals, bit for
/// bit, the fixed point of W = max(z, the least of the neighbours' W), relaxed from +∞ with the
/// seeds held (Barnes, Lehman and Mulla 2014's lowest-saddle property).
#[test]
fn the_flood_is_the_minimax_level_from_the_seeds() {
    let (graph, mesh) = world(3);
    let n = slot(graph.len());
    let mut lcg = Lcg::new(5);
    let z: Vec<f64> = (0..n).map(|_| 1_000.0 * lcg.next_f64()).collect();
    let seeds: Vec<Seed> = (0..graph.len())
        .step_by(64)
        .map(|c| Seed {
            cell: c,
            level_m: z[slot(c)] + 200.0 * lcg.next_f64(),
        })
        .collect();
    let routing = route(&mesh, &z, &seeds, &vec![0.5; n]);
    let mut oracle = vec![f64::INFINITY; n];
    let mut held = vec![false; n];
    for seed in &seeds {
        oracle[slot(seed.cell)] = seed.level_m;
        held[slot(seed.cell)] = true;
    }
    let mut changed = true;
    while changed {
        changed = false;
        for c in 0..graph.len() {
            if held[slot(c)] {
                continue;
            }
            let lowest = graph
                .edge_neighbours(c)
                .iter()
                .map(|&m| oracle[slot(m)])
                .fold(f64::INFINITY, num::min);
            let level = num::max(z[slot(c)], lowest);
            if level < oracle[slot(c)] {
                oracle[slot(c)] = level;
                changed = true;
            }
        }
    }
    for (&filled, &expected) in routing.filled_m().iter().zip(&oracle) {
        assert_same_bits(filled, expected);
    }
}

/// The receiver's draw is proportional to the drop per metre: on every face's interior cells of
/// one parity at level 6 (11,532 cells), whose four neighbours are seeds at their ground on a
/// plane rising 1 m a cell in i and 2 m in j, each cell takes its u = 0 neighbour with probability
/// (1 m ÷ `l_u`) ÷ (1 m ÷ `l_u` + 2 m ÷ `l_v`), l its links' lengths, and its v = 0 neighbour otherwise,
/// never an upper one. Under the reference seed the count of u = 0 draws matches the sum of those
/// probabilities within its binomial spread (two-sided p ≥ 10⁻³), and each cell's slope
/// correction is its drop per metre over the hypotenuse of the two axes' slopes, bit for bit.
#[test]
fn the_receiver_is_drawn_in_proportion_to_the_drop_per_metre() {
    let (graph, mesh) = world(6);
    let side = 1_u32 << 6;
    let z: Vec<f64> = graph
        .cells()
        .iter()
        .map(|cell| f64::from(cell.i()) + 2.0 * f64::from(cell.j()))
        .collect();
    let tested: Vec<bool> = graph
        .cells()
        .iter()
        .map(|cell| {
            let (i, j) = (cell.i(), cell.j());
            (1..side - 1).contains(&i) && (1..side - 1).contains(&j) && (i + j) % 2 == 0
        })
        .collect();
    let seeds: Vec<Seed> = (0..graph.len())
        .filter(|&c| !tested[slot(c)])
        .map(|c| Seed {
            cell: c,
            level_m: z[slot(c)],
        })
        .collect();
    let routing = route(
        &mesh,
        &z,
        &seeds,
        &receiver_uniforms(reference::SEED, &graph),
    );
    let (mut cells, mut u_draws, mut mean, mut variance) = (0_u32, 0_u32, 0.0, 0.0);
    for c in (0..graph.len()).filter(|&c| tested[slot(c)]) {
        cells += 1;
        let (slope_u, slope_v) = (1.0 / mesh.link_m(c, 0), 2.0 / mesh.link_m(c, 2));
        let p = slope_u / (slope_u + slope_v);
        mean += p;
        variance += p * (1.0 - p);
        let link = routing.receiver(c).unwrap();
        let slope = match link.edge {
            Edge::UMin => {
                u_draws += 1;
                slope_u
            }
            Edge::VMin => slope_v,
            Edge::UMax | Edge::VMax => panic!("cell {c} drains uphill by {:?}", link.edge),
        };
        assert_same_bits(routing.correction(c), slope / math::hypot(slope_u, slope_v));
    }
    assert_eq!(cells, 6 * 62 * 62 / 2);
    let score = (f64::from(u_draws) - mean) / variance.sqrt();
    eprintln!("{u_draws} u = 0 draws of {cells}, {mean:.1} expected, z = {score:.2}");
    assert_p_value(
        "the u = 0 draws",
        2.0 * (1.0 - normal_cdf(score.abs())),
        ALPHA,
    );
}
