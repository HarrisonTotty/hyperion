//! R09.T11's tests of the plates step (Design note 6; `decision-r09-t5.md` item 5).

use std::sync::OnceLock;

use hyperion_surface::field::{CoarseLevel, FieldView, coarse_level};
use hyperion_surface::spheroid::Spheroid;
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

use super::*;
use crate::planetary::surface::inputs::{CoarseInputs, CoarseInputsBuilder};
use crate::planetary::surface::reference::{self, builder_of};
use crate::planetary::surface::steps::CellState;

/// A body's grids, for its passes.
struct World {
    inputs: CoarseInputs,
    grid: CellGraph,
    climate: CellGraph,
}

impl World {
    fn new(inputs: CoarseInputs) -> Self {
        let level = coarse_level(inputs.radius());
        Self {
            grid: CellGraph::for_field(level),
            climate: CellGraph::for_climate(level),
            inputs,
        }
    }

    fn pass(&self, seed: SurfaceSeed) -> Pass<'_> {
        Pass::new(seed, &self.inputs, &self.grid, &self.climate)
    }
}

/// A world's plates under one seed: its layout and the working state the step leaves.
struct Generated {
    world: World,
    layout: Layout,
    working: Working,
}

/// The plates of `inputs` under `seed`, laid out and written.
fn generate(inputs: CoarseInputs, seed: SurfaceSeed) -> Generated {
    let world = World::new(inputs);
    let pass = world.pass(seed);
    let layout = Layout::of(&pass).expect("a mobile lid has plates");
    let mut working = Working::initial(&pass);
    run(&pass, &mut working);
    Generated {
        world,
        layout,
        working,
    }
}

/// The reference Earth under the reference seed, generated once for the tests that read it.
fn earth() -> &'static Generated {
    static EARTH: OnceLock<Generated> = OnceLock::new();
    EARTH.get_or_init(|| generate(reference::earth_like(), reference::SEED))
}

/// The reference Earth's inputs, as a builder to vary.
fn earth_builder() -> CoarseInputsBuilder {
    builder_of(&reference::earth_like())
}

/// The great-circle length of the link between cells `a` and `b` on a sphere of `radius_m`.
fn edge_m(grid: &CellGraph, a: u32, b: u32, radius_m: f64) -> f64 {
    grid.arc(a, b).value() * radius_m
}

/// Each cell's edge and vertex neighbours.
fn neighbours(grid: &CellGraph, cell: u32) -> impl Iterator<Item = u32> + '_ {
    grid.edge_neighbours(cell)
        .into_iter()
        .chain(grid.vertex_neighbours(cell))
}

/// Earth's radius for hand-placed plates, metres.
const PLACED_RADIUS_M: f64 = crate::units::consts::EARTH_RADIUS_M;

/// The level of hand-placed plates' grids, 6.
const PLACED_LEVEL: u8 = 6;

/// A hand-placed plate at `seed` rotating at `omega` (radians a second), drawn as half the
/// sphere.
fn placed_plate(seed: [f64; 3], omega: [f64; 3]) -> PlateDraw {
    PlateDraw {
        seed,
        angular_velocity_rad_per_s: omega,
        area_sr: core::f64::consts::TAU,
        nucleus: seed,
        nucleus_size: 1.0,
    }
}

/// The layout of hand-placed `draws` at `weights` on `grid`, unwarped, whose cells have the crust
/// `crust_of` gives their plate.
fn placed(
    grid: &CellGraph,
    draws: &[PlateDraw],
    weights: Vec<f64>,
    crust_of: impl Fn(u8) -> Crust,
) -> Layout {
    let centres = Warped::none(grid).plates;
    let raster = Raster::new(grid, &centres);
    let seeds: Vec<[f64; 3]> = draws.iter().map(|d| d.seed).collect();
    let targets: Vec<f64> = draws.iter().map(|d| d.area_sr).collect();
    let tolerance = vec![1.0; draws.len()];
    let fitted = Fitted::at(
        &raster,
        &seeds,
        &targets,
        &tolerance,
        weights,
        Parts::Joined,
    );
    let crust = fitted.plate.iter().map(|&k| crust_of(k)).collect();
    Layout::new(grid, PLACED_RADIUS_M, &centres, draws, fitted, crust)
}

/// An angular speed of about 0.5° a million years, radians a second.
const OMEGA: f64 = 2.8e-16;

/// Two hemispheres split by the plane x = 0, plate 0 on +x and plate 1 on −x, rotating at `omega`
/// and its negative.
fn hemispheres(grid: &CellGraph, omega: [f64; 3], crust_of: impl Fn(u8) -> Crust) -> Layout {
    let draws = [
        placed_plate([1.0, 0.0, 0.0], omega),
        placed_plate([-1.0, 0.0, 0.0], omega.map(|w| -w)),
    ];
    placed(grid, &draws, vec![0.0, 0.0], crust_of)
}

/// `n` as a float.
fn real(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap())
}

/// The mean of `counts` and their sample standard deviation.
fn mean_and_spread(counts: &[usize]) -> (f64, f64) {
    let n = real(counts.len());
    let mean = counts.iter().map(|&c| real(c)).sum::<f64>() / n;
    let square: f64 = counts
        .iter()
        .map(|&c| (real(c) - mean) * (real(c) - mean))
        .sum();
    (mean, (square / (n - 1.0)).sqrt())
}

/// The tail law's cumulative distribution on [`smallest`, 1 sr].
fn tail_cdf(area: f64, smallest: f64) -> f64 {
    let low = math::powf(smallest, -TAIL_EXPONENT);
    ((low - math::powf(area.clamp(smallest, 1.0), -TAIL_EXPONENT)) / (low - 1.0)).clamp(0.0, 1.0)
}

#[test]
fn every_cell_has_one_plate_and_ties_go_to_the_lower_seed() {
    let grid = CellGraph::new(5).unwrap();
    let centres = grid.centres().to_vec();
    let raster = Raster::new(&grid, &centres);
    let p = [0.6, 0.0, 0.8];
    // Plates 1 and 2 share a seed and a weight, so they tie at every cell: plate 1 takes them all.
    let seeds = [[-0.6, 0.0, -0.8], p, p];
    let mut plate = vec![9; centres.len()];
    assign(&raster, &seeds, &[0.0, 0.0, 0.0], &mut plate);
    assert!(
        plate.iter().all(|&k| k < 2),
        "plate 2 ties with plate 1 and loses"
    );
    assert!(plate.contains(&1) && plate.contains(&0));
    // With the seeds swapped, the lower index wins the tie again.
    assign(&raster, &[p, p], &[0.0, 0.0], &mut plate);
    assert!(plate.iter().all(|&k| k == 0));

    // The pruned scan is the full scan's, on the reference Earth's own fitted weights.
    let generated = earth();
    let grid = &generated.world.grid;
    let warp = Warp::draw(reference::SEED);
    let warped = warp.apply(grid);
    let raster = Raster::new(grid, &warped.plates);
    let seeds: Vec<[f64; 3]> = generated.layout.plates.iter().map(|p| p.seed).collect();
    let weights: Vec<f64> = generated.layout.plates.iter().map(|p| p.weight).collect();
    let mut pruned = vec![0; warped.plates.len()];
    assign(&raster, &seeds, &weights, &mut pruned);
    for (n, (&x, &k)) in warped.plates.iter().zip(&pruned).enumerate() {
        let mut best = 0;
        for j in 1..seeds.len() {
            let score = |i: usize| dot(x, seeds[i]) + weights[i];
            if score(j) > score(best) {
                best = j;
            }
        }
        assert_eq!(usize::from(k), best, "cell {n}");
    }
    let count = generated.layout.plates.len();
    assert!(
        generated
            .layout
            .plate
            .iter()
            .all(|&k| usize::from(k) < count)
    );
    for (k, plate) in generated.layout.plates.iter().enumerate() {
        assert!(plate.area_sr > 0.0, "plate {k} has cells");
    }
}

#[test]
fn every_plate_is_one_connected_region() {
    let generated = earth();
    let (_, parts) = parts_of(&generated.world.grid, &generated.layout.plate);
    assert_eq!(parts.len(), generated.layout.plates.len());
}

#[test]
fn boundaries_are_exactly_the_adjacencies_between_plates() {
    let generated = earth();
    let grid = &generated.world.grid;
    let plate = &generated.layout.plate;
    let mut boundaries = 0;
    for cell in 0..grid.len() {
        let n = slot(cell);
        let across = grid
            .edge_neighbours(cell)
            .iter()
            .any(|&m| plate[slot(m)] != plate[n]);
        assert_eq!(
            generated.layout.boundary[n].is_some(),
            across,
            "cell {cell}"
        );
        assert_eq!(generated.layout.contact[n].is_some(), across, "cell {cell}");
        if let Some(contact) = generated.layout.contact[n] {
            assert_ne!(contact.other, plate[n]);
            assert_eq!(plate[slot(contact.neighbour)], contact.other);
            assert!(grid.edge_neighbours(cell).contains(&contact.neighbour));
            boundaries += 1;
        }
        let state = generated.working.cells[n];
        assert_eq!(state.plate, plate[n]);
        assert_ne!(state.boundary, BoundaryKind::Absent, "cell {cell}");
    }
    assert!(boundaries > 1_000, "{boundaries} boundary cells");
}

#[test]
fn signed_distances_are_zero_on_boundaries_and_grow_by_at_most_an_edge_a_step() {
    let generated = earth();
    let grid = &generated.world.grid;
    let radius = generated.world.inputs.radius().value();
    let cells = &generated.working.cells;
    let distance = |n: usize| {
        cells[n]
            .boundary_distance
            .expect("every cell of a mobile lid has a nearest boundary")
            .value()
    };
    let mut negative = 0;
    for cell in 0..grid.len() {
        let n = slot(cell);
        let d = distance(n);
        if generated.layout.boundary[n].is_some() {
            // +0, not −0: a zero has one form on the wire.
            assert_same_bits(d, 0.0);
        } else {
            assert!(d.abs() > 0.0, "cell {cell}");
        }
        if d < 0.0 {
            negative += 1;
            assert_eq!(cells[n].boundary, BoundaryKind::Subduction, "cell {cell}");
        }
        for m in neighbours(grid, cell) {
            if generated.layout.plate[slot(m)] != generated.layout.plate[n] {
                continue;
            }
            let step = (d.abs() - distance(slot(m)).abs()).abs();
            let edge = edge_m(grid, cell, m, radius);
            assert!(
                step <= edge * (1.0 + 1e-12),
                "cells {cell} and {m}: {step} m against an edge of {edge} m"
            );
        }
    }
    assert!(negative > 0, "some floor lies on a subducting plate");
}

#[test]
fn boundary_kinds_follow_the_relative_motion_on_hand_placed_plates() {
    let grid = CellGraph::new(PLACED_LEVEL).unwrap();
    // Spinning apart about z: plate 0 on +x moves at 2Ω ẑ × r relative to plate 1, which along
    // the boundary x = 0 is −2Ω y x̂: towards plate 1 where y > 0, away where y < 0.
    let oceans = hemispheres(&grid, [0.0, 0.0, OMEGA], |_| Crust::Oceanic);
    let continents = hemispheres(&grid, [0.0, 0.0, OMEGA], |_| Crust::Continental);
    let mixed = hemispheres(&grid, [0.0, 0.0, OMEGA], |k| {
        if k == 0 {
            Crust::Continental
        } else {
            Crust::Oceanic
        }
    });
    // Sliding past each other about x: pure strike-slip.
    let sliding = hemispheres(&grid, [OMEGA, 0.0, 0.0], |_| Crust::Oceanic);
    let mut seen = [0; 5];
    let mut ocean_subducts: [Option<bool>; 2] = [None; 2];
    for cell in 0..grid.len() {
        let n = slot(cell);
        let y = grid.centre(cell)[1];
        let Some(ocean) = oceans.boundary[n] else {
            assert!(sliding.boundary[n].is_none());
            continue;
        };
        let continent = continents.boundary[n].unwrap();
        let mix = mixed.boundary[n].unwrap();
        let slide = sliding.boundary[n].unwrap();
        assert_eq!(slide.kind, BoundaryKind::Transform, "cell {cell}");
        // Strike-slip to well within the obliquity code's step of 90° ÷ 255 (6.2 × 10⁻³ rad).
        let off = (slide.obliquity_rad - core::f64::consts::FRAC_PI_2).abs();
        assert!(off < 1e-3, "cell {cell}: {off} rad off strike-slip");
        if y.abs() < 0.05 {
            continue;
        }
        // The motion's obliquity at the cell's centre, half a cell off the boundary x = 0, against
        // the boundary's normal there.
        let x = grid.centre(cell);
        let normal = unit(tangent([1.0, 0.0, 0.0], x));
        let v = cross([0.0, 0.0, 2.0 * OMEGA], x);
        let across = dot(v, normal);
        let along = norm(sub(v, scale(normal, across)));
        let expected = math::atan2(along, across.abs());
        for b in [ocean, continent, mix] {
            assert!(
                (b.obliquity_rad - expected).abs() < 1e-3,
                "cell {cell}: {} rad against {expected}",
                b.obliquity_rad
            );
        }
        if y < 0.0 {
            for b in [ocean, continent, mix] {
                assert_eq!(b.kind, BoundaryKind::Divergent, "cell {cell}");
                assert!(!b.subducts);
            }
            seen[3] += 1;
        } else {
            assert_eq!(ocean.kind, BoundaryKind::Subduction, "cell {cell}");
            assert_eq!(continent.kind, BoundaryKind::Collision, "cell {cell}");
            assert_eq!(mix.kind, BoundaryKind::Subduction, "cell {cell}");
            // The oceanic plate 1 subducts beneath the continental plate 0.
            assert_eq!(mix.subducts, oceans.plate[n] == 1, "cell {cell}");
            assert!(!continent.subducts);
            // Of two oceanic plates one side subducts all along their boundary.
            let hemisphere = usize::from(oceans.plate[n]);
            assert_eq!(
                *ocean_subducts[hemisphere].get_or_insert(ocean.subducts),
                ocean.subducts,
                "cell {cell}"
            );
            seen[1] += 1;
        }
    }
    assert!(seen[1] > 50 && seen[3] > 50, "{seen:?}");
    assert_eq!(ocean_subducts[0].map(|s| !s), ocean_subducts[1]);
}

/// `layout` written into a working state on its level-6 grid.
fn written(layout: Layout) -> Working {
    let inputs = reference::moon_like();
    let level = CoarseLevel::new(PLACED_LEVEL).unwrap();
    let (grid, climate) = (CellGraph::for_field(level), CellGraph::for_climate(level));
    let pass = Pass::new(reference::SEED, &inputs, &grid, &climate);
    let mut working = Working::initial(&pass);
    layout.write(&mut working);
    working
}

/// Each cell carries whether its plate subducts at its nearest boundary (R09.T12.b's contract,
/// `decision-r09-t12-coarse.md`'s addendum): a signed distance is negative exactly where it holds
/// and the distance is above +0, it implies a subduction, and on hand-placed hemispheres the
/// oceanic side's boundary cells hold it beneath a continent and the older side's of two oceans.
#[test]
fn each_cell_carries_whether_its_plate_subducts() {
    for cell in &earth().working.cells {
        let d = cell.boundary_distance.map_or(0.0, Metres::value);
        assert_eq!(d < 0.0, cell.subducts && d.abs() > 0.0, "{cell:?}");
        if cell.subducts {
            assert_eq!(cell.boundary, BoundaryKind::Subduction);
        }
    }
    assert!(earth().working.cells.iter().any(|c| c.subducts));
    let grid = CellGraph::new(PLACED_LEVEL).unwrap();
    let mixed = written(hemispheres(&grid, [0.0, 0.0, OMEGA], |k| {
        if k == 0 {
            Crust::Continental
        } else {
            Crust::Oceanic
        }
    }));
    let oceans_layout = hemispheres(&grid, [0.0, 0.0, OMEGA], |_| Crust::Oceanic);
    // The older side of the two oceans, by T11's own polarity over their floor's kinematic ages
    // (the higher index at a tie, which these mirrored hemispheres come near).
    let ages: Vec<f64> = (0..oceans_layout.plate.len())
        .map(|n| oceans_layout.seafloor(n).map_or(0.0, |o| o.age.value()))
        .collect();
    let polarity = older_sides(
        &oceans_layout.plate,
        &oceans_layout.crust,
        &oceans_layout.contact,
        &ages,
    );
    let older = u8::from(!polarity[&(0, 1)]);
    let oceans = written(oceans_layout);
    let (mut continental, mut oceanic) = (0, 0);
    for (n, (mix, ocean)) in mixed.cells.iter().zip(&oceans.cells).enumerate() {
        if mix.boundary != BoundaryKind::Subduction || !mix.on_boundary() {
            continue;
        }
        assert_eq!(mix.subducts, mix.crust == Crust::Oceanic, "cell {n}");
        match mix.crust {
            Crust::Continental => continental += 1,
            Crust::Oceanic | Crust::Lid | Crust::Province => oceanic += 1,
        }
        assert_eq!(ocean.boundary, BoundaryKind::Subduction, "cell {n}");
        assert_eq!(ocean.subducts, ocean.plate == older, "cell {n}");
    }
    assert!(continental > 20 && oceanic > 20, "{continental} {oceanic}");
}

#[test]
fn the_older_oceanic_side_subducts() {
    let plate = [0, 1, 0, 1];
    let crust = [Crust::Oceanic; 4];
    let contact = |other: u8, neighbour: u32| {
        Some(Contact {
            other,
            neighbour,
            motion: Motion::Convergent,
            obliquity_rad: 0.0,
            closing_m_per_s: 1e-9,
        })
    };
    let contacts = [contact(1, 1), contact(0, 0), None, None];
    // Plate 1's floor is the older: it subducts, so the lower-indexed plate does not.
    let older = older_sides(&plate, &crust, &contacts, &[1.0, 2.0, 0.0, 0.0]);
    assert_eq!(older.get(&(0, 1)), Some(&false));
    let older = older_sides(&plate, &crust, &contacts, &[3.0, 2.0, 0.0, 0.0]);
    assert_eq!(older.get(&(0, 1)), Some(&true));
    let older = older_sides(&plate, &crust, &contacts, &[2.0, 2.0, 0.0, 0.0]);
    assert_eq!(
        older.get(&(0, 1)),
        Some(&false),
        "a tie: the higher index subducts"
    );
}

#[test]
fn the_half_rate_is_the_ridge_s_opening_halved_and_inherited_along_its_transform() {
    let grid = CellGraph::new(PLACED_LEVEL).unwrap();
    let omega = [0.0, 0.0, OMEGA];
    let layout = hemispheres(&grid, omega, |_| Crust::Oceanic);
    let relative = [0.0, 0.0, 2.0 * OMEGA];
    let mut ridges = 0;
    for cell in 0..grid.len() {
        let n = slot(cell);
        let origin = layout.seafloor(n).expect("every cell is oceanic");
        let Some(ridge) = layout.ridge[n] else {
            panic!("cell {cell}: the hemispheres have a ridge");
        };
        let source = layout.contact[slot(ridge.source)].unwrap();
        assert_eq!(source.motion, Motion::Divergent);
        assert_eq!(
            layout.plate[slot(ridge.source)],
            layout.plate[n],
            "the ridge is the plate's own"
        );
        // Inherited along the own-ridge transform, bit for bit.
        assert_same_bits(origin.half_rate.value(), source.half_rate_m_per_s());
        assert_same_bits(origin.ridge_distance.unwrap().value(), ridge.distance_m);
        if ridge.source == cell {
            ridges += 1;
            // |ω × r| ÷ 2 normal to the ridge, whose normal is x̂ in the tangent plane.
            let x = grid.centre(cell);
            let normal = unit(tangent([1.0, 0.0, 0.0], x));
            let expected = dot(cross(relative, x), normal).abs() * PLACED_RADIUS_M / 2.0;
            let half_rate = origin.half_rate.value();
            // The normal is the score gradient's by least squares over the four neighbours, within
            // some 10⁻⁵ rad of the plane's; near the Euler pole the motion along the ridge is as
            // large as across it, so the half-rate is held to 10⁻⁴.
            assert!(
                (half_rate - expected).abs() <= 1e-4 * expected,
                "cell {cell}: {half_rate} m/s against {expected}"
            );
            assert_eq!(origin.ridge_distance, Some(Metres::ZERO));
        }
    }
    assert!(ridges > 50, "{ridges} ridge cells");
}

#[test]
fn the_ridge_distance_is_zero_on_ridges_and_never_taken_from_a_trench() {
    let generated = earth();
    let layout = &generated.layout;
    let mut oceanic = 0;
    for (n, state) in generated.working.cells.iter().enumerate() {
        let Some(origin) = state.seafloor_origin else {
            assert_ne!(state.crust, Crust::Oceanic);
            continue;
        };
        oceanic += 1;
        assert_eq!(state.crust, Crust::Oceanic);
        if state.boundary == BoundaryKind::Divergent && layout.boundary[n].is_some() {
            assert_eq!(origin.ridge_distance, Some(Metres::ZERO), "ridge cell {n}");
        }
        if let Some(reach) = layout.ridge[n] {
            let source = layout.boundary[slot(reach.source)].unwrap();
            assert_eq!(source.kind, BoundaryKind::Divergent, "cell {n}");
            assert_eq!(layout.plate[slot(reach.source)], layout.plate[n]);
            assert_same_bits(origin.ridge_distance.unwrap().value(), reach.distance_m);
        } else {
            assert_eq!(origin.ridge_distance, None);
        }
    }
    assert!(oceanic > 100_000, "{oceanic} oceanic cells");
}

#[test]
fn the_age_is_the_ridge_distance_over_the_half_rate_capped_at_200_myr() {
    let generated = earth();
    let mut young = 0;
    for state in &generated.working.cells {
        let Some(origin) = state.seafloor_origin else {
            continue;
        };
        let age = origin.age.value();
        assert!(origin.half_rate.value() > 0.0);
        if let Some(d) = origin.ridge_distance {
            let expected = num::min(d.value() / origin.half_rate.value(), OLDEST_FLOOR.value());
            assert_same_bits(age, expected);
        } else {
            assert_same_bits(age, OLDEST_FLOOR.value());
            assert_eq!(origin.half_rate, MEAN_HALF_RATE);
        }
        if age < OLDEST_FLOOR.value() {
            young += 1;
        }
    }
    assert!(young > 50_000, "{young} cells below the cap");
    assert!(
        (OLDEST_FLOOR.value() / SECONDS_PER_MEGAYEAR - 200.0).abs() < 1e-9,
        "the cap is 200 Myr"
    );
}

#[test]
fn a_plate_with_no_divergent_boundary_has_200_myr_and_23_mm_a_year() {
    let grid = CellGraph::new(PLACED_LEVEL).unwrap();
    let layout = hemispheres(&grid, [OMEGA, 0.0, 0.0], |_| Crust::Oceanic);
    for n in 0..layout.plate.len() {
        let origin = layout.seafloor(n).unwrap();
        assert_eq!(origin.ridge_distance, None);
        assert_eq!(origin.age, OLDEST_FLOOR);
        assert_eq!(origin.half_rate, MEAN_HALF_RATE);
    }
    let mm_a = MEAN_HALF_RATE.value() * SECONDS_PER_JULIAN_YEAR * 1e3;
    assert!((mm_a - 23.0).abs() < 1e-9, "{mm_a} mm a⁻¹");
}

#[test]
fn the_drawn_plate_areas_follow_the_tail_law_over_many_seeds() {
    let cells = CoarseLevel::MAX.cell_count();
    let smallest = smallest_plate_sr(cells);
    assert_same_bits(smallest, SMALLEST_PLATE_SR);
    let omega = angular_speed_scale_rad_per_s(EARTH_HEAT_FLOW);
    let mut tail = Vec::new();
    let mut counts = Vec::new();
    let mut drawn_tails = 0_u64;
    let mut ended = 0;
    for s in 0..500_u64 {
        let seed = SurfaceSeed::new(0x0070_1a7e_0000 + s);
        let draws = draw_plates(seed, cells, omega);
        let total: f64 = draws.iter().map(|d| d.area_sr).sum();
        assert!((total - FOUR_PI).abs() < 1e-12, "seed {s}: {total} sr");
        for (k, d) in draws.iter().enumerate() {
            if k < LARGE_PLATES {
                assert!(d.area_sr >= LARGE_PLATE_FLOOR_SR, "seed {s} plate {k}");
            } else {
                assert!((smallest..LARGE_PLATE_FLOOR_SR).contains(&d.area_sr));
                tail.push(d.area_sr);
            }
        }
        counts.push(draws.len());
        let mut count = seed.stream(SURFACE_COARSE_PLATES, ObjectKey::surface_item(0));
        count.seek(COUNT_WORD);
        let tail_count = count.poisson(expected_tail(smallest));
        drawn_tails += tail_count;
        if u64::try_from(draws.len() - LARGE_PLATES).unwrap() < tail_count {
            ended += 1;
        }
    }
    // The tail ends early where it would overflow the 4π − 7 sr the large plates leave it: about
    // one draw in fifty (the module documentation's figure, 11 of these 500), at most one in
    // twenty.
    eprintln!("tails ended early: {ended} of 500");
    assert!(ended <= 25, "{ended} tails ended early");
    let n = tail.len();
    let ks = ks_one_sample(&mut tail, |a| tail_cdf(a, smallest));
    eprintln!(
        "drawn tail areas: KS D = {:.4} over {n} plates, p = {:.3}",
        ks.statistic, ks.p_value
    );
    assert_p_value("the drawn tail's areas", ks.p_value, ALPHA);
    let (mean, spread) = mean_and_spread(&counts);
    let seeds = real(counts.len());
    eprintln!("plate count: mean {mean:.2}, spread {spread:.2} over {seeds} seeds");
    assert!(
        (41.0..=55.0).contains(&mean),
        "the mean plate count is 48 ± 7: {mean}"
    );
    // The tail's count is a Poisson draw of the law's mean, 41.06 from 0.002 sr.
    let lambda = expected_tail(smallest);
    assert!((lambda - 41.06).abs() < 0.01, "{lambda}");
    let drawn = f64::from(u32::try_from(drawn_tails).unwrap()) / seeds;
    let bound = 3.29 * (lambda / seeds).sqrt();
    assert!(
        (drawn - lambda).abs() <= bound,
        "{drawn} against {lambda} ± {bound}"
    );
}

#[test]
#[ignore = "slow: 32 reference Earths' plates at level 8"]
fn the_realised_plate_areas_follow_the_tail_law_over_many_seeds() {
    let world = World::new(reference::earth_like());
    let smallest = SMALLEST_PLATE_SR;
    let mut tail = Vec::new();
    let mut counts = Vec::new();
    let mut half_rates = Vec::new();
    for s in 0..32_u64 {
        let pass = world.pass(SurfaceSeed::new(0x5ea1_0000 + s));
        let layout = Layout::of(&pass).unwrap();
        for (k, plate) in layout.plates.iter().enumerate() {
            if k >= LARGE_PLATES {
                tail.push(plate.area_sr);
            }
        }
        counts.push(layout.plates.iter().filter(|p| p.area_sr > 0.0).count());
        let (mut sum, mut cells) = (0.0, 0.0);
        for contact in layout.contact.iter().flatten() {
            if contact.motion == Motion::Divergent {
                sum += contact.half_rate_m_per_s();
                cells += 1.0;
            }
        }
        half_rates.push(sum / cells * SECONDS_PER_JULIAN_YEAR * 1e3);
    }
    let n = tail.len();
    let ks = ks_one_sample(&mut tail, |a| tail_cdf(a, smallest));
    let (mean, spread) = mean_and_spread(&counts);
    let rate = half_rates.iter().sum::<f64>() / real(half_rates.len());
    eprintln!(
        "realised tail areas: KS D = {:.4} over {n} plates, p = {:.3}; plate count {mean:.2} ± \
         {spread:.2}; mean ridge half-rate {rate:.2} mm a⁻¹",
        ks.statistic, ks.p_value
    );
    assert_p_value("the realised tail's areas", ks.p_value, ALPHA);
    assert!(
        (41.0..=55.0).contains(&mean),
        "the mean plate count is 48 ± 7: {mean}"
    );
    assert!((rate - 23.0).abs() <= 0.2 * 23.0, "{rate} mm a⁻¹");
}

#[test]
fn the_areas_sum_to_four_pi_and_each_plate_its_drawn_area() {
    let generated = earth();
    let grid = &generated.world.grid;
    let plates = &generated.layout.plates;
    let drawn: f64 = plates.iter().map(|p| p.drawn_area_sr).sum();
    let realised: f64 = plates.iter().map(|p| p.area_sr).sum();
    assert!((drawn - FOUR_PI).abs() < 1e-12, "{drawn}");
    assert!((realised - FOUR_PI).abs() < 1e-12, "{realised}");
    let mean_cell = FOUR_PI / f64::from(grid.len());
    for (k, p) in plates.iter().enumerate() {
        let tolerance = num::max(FIT_RELATIVE * p.drawn_area_sr, FIT_CELLS * mean_cell);
        assert!(
            (p.area_sr - p.drawn_area_sr).abs() <= tolerance,
            "plate {k}: {} sr drawn, {} sr realised",
            p.drawn_area_sr,
            p.area_sr
        );
    }
    assert_eq!(generated.working.plates, *plates);
}

#[test]
fn the_continental_fraction_is_matched_to_half_a_cell() {
    let generated = earth();
    let grid = &generated.world.grid;
    let largest = grid.solid_angles_sr().iter().copied().fold(0.0, num::max);
    let continental: f64 = generated
        .working
        .cells
        .iter()
        .zip(grid.solid_angles_sr())
        .filter(|(c, _)| c.crust == Crust::Continental)
        .map(|(_, &omega)| omega)
        .sum();
    let target = generated.world.inputs.continental_fraction() * FOUR_PI;
    assert!(
        (continental - target).abs() <= 0.5 * largest,
        "{continental} sr against {target}"
    );
    for state in &generated.working.cells {
        assert!(matches!(state.crust, Crust::Continental | Crust::Oceanic));
        assert_eq!(
            state.seafloor_origin.is_some(),
            state.crust == Crust::Oceanic
        );
        assert_eq!(
            state.hill_relief,
            (state.crust == Crust::Oceanic).then_some(H_REF)
        );
    }
    // Some continent meets another plate across a boundary, and some lies inside its plate.
    let kinds: Vec<BoundaryKind> = generated.working.cells.iter().map(|c| c.boundary).collect();
    assert!(kinds.contains(&BoundaryKind::Collision));
    let layout = &generated.layout;
    let passive = (0..grid.len()).any(|cell| {
        let n = slot(cell);
        layout.crust[n] == Crust::Oceanic
            && grid.edge_neighbours(cell).iter().any(|&m| {
                layout.crust[slot(m)] == Crust::Continental
                    && layout.plate[slot(m)] == layout.plate[n]
            })
    });
    assert!(passive, "a passive margin inside a plate");

    let grid = CellGraph::new(5).unwrap();
    let draws = draw_plates(reference::SEED, grid.len(), 0.0);
    let centres = grid.centres();
    for fraction in [0.0, 0.2, 0.7, 1.0] {
        let crust = continents(centres, grid.solid_angles_sr(), &draws, fraction);
        let area: f64 = crust
            .iter()
            .zip(grid.solid_angles_sr())
            .filter(|(c, _)| **c == Crust::Continental)
            .map(|(_, &omega)| omega)
            .sum();
        let largest = grid.solid_angles_sr().iter().copied().fold(0.0, num::max);
        assert!(
            (area - fraction * FOUR_PI).abs() <= 0.5 * largest,
            "{fraction}: {area}"
        );
    }
}

/// The working state a stagnant lid's pass starts from, which the step leaves alone.
fn untouched(inputs: CoarseInputs) {
    let world = World::new(inputs);
    let pass = world.pass(reference::SEED);
    let initial = Working::initial(&pass);
    let mut working = initial.clone();
    run(&pass, &mut working);
    assert_eq!(working, initial);
    assert_eq!(working.plates, []);
    assert!(
        working
            .cells
            .iter()
            .all(|c: &CellState| c.boundary == BoundaryKind::Absent
                && c.boundary_distance.is_none()
                && c.plate == 0
                && c.crust == Crust::Lid
                && c.seafloor_origin.is_none())
    );
}

#[test]
fn a_stagnant_lid_world_has_no_plates() {
    untouched(reference::mars_like());
    untouched(reference::moon_like());
    untouched(reference::ceres_like());
    // A small mobile-lid stand-in for the other regimes, which take the stagnant lid's until a
    // morphology is ruled for each.
    for regime in [
        TectonicRegime::StagnantLid,
        TectonicRegime::HeatPipe,
        TectonicRegime::Episodic,
        TectonicRegime::IceShell,
    ] {
        let inputs = earth_builder()
            .figure(Spheroid::sphere(1.0e6))
            .tectonics(regime, 0.0)
            .build()
            .unwrap();
        untouched(inputs);
    }
}

#[test]
fn a_mobile_lid_without_a_secondary_crust_gets_the_stagnant_lid_s() {
    // The reference Ceres's rock never melted: it has no secondary crust, so its plates could
    // form no oceanic floor.
    let inputs = builder_of(&reference::ceres_like())
        .tectonics(TectonicRegime::MobileLid, 0.0)
        .build()
        .unwrap();
    assert_eq!(inputs.crust_palette()[1], None);
    untouched(inputs);
}

#[test]
fn the_same_seed_gives_the_same_plates_and_another_seed_others() {
    let inputs = earth_builder()
        .figure(Spheroid::sphere(2.0e6))
        .build()
        .unwrap();
    let first = generate(inputs.clone(), reference::SEED);
    let again = generate(inputs.clone(), reference::SEED);
    let other = generate(inputs, SurfaceSeed::new(0x0dd5));
    assert_eq!(first.layout, again.layout);
    assert_eq!(first.working, again.working);
    assert_ne!(first.layout.plate, other.layout.plate);
}

#[test]
fn generation_holds_from_tiny_to_large_bodies() {
    for (radius_m, level, fraction, heat_flow) in [
        (0.6e6, 5, 0.7, 0.0),
        (1.5e6, 6, 1.0, 0.0916),
        (3.0e6, 7, 0.2, 0.3),
    ] {
        let inputs = earth_builder()
            .figure(Spheroid::sphere(radius_m))
            .tectonics(TectonicRegime::MobileLid, fraction)
            .heat_flow(WattsPerSquareMetre::new(heat_flow))
            .build()
            .unwrap();
        let generated = generate(inputs.clone(), reference::SEED);
        assert_eq!(generated.world.grid.level(), level);
        let plates = &generated.layout.plates;
        assert!(plates.len() >= LARGE_PLATES + 20, "{} plates", plates.len());
        let (_, parts) = parts_of(&generated.world.grid, &generated.layout.plate);
        let realised = plates.iter().filter(|p| p.area_sr > 0.0).count();
        assert_eq!(
            parts.len(),
            realised,
            "radius {radius_m}: each plate one region"
        );
        // The whole pass quantises the plates into a valid field.
        let field = crate::planetary::surface::coarse_pass(reference::SEED, &inputs);
        assert_eq!(field.header().level().get(), level);
        for (record, state) in field.synthesis().iter().zip(&generated.working.cells) {
            assert_eq!(record.plate, state.plate);
            assert_eq!(record.crust, state.crust);
            assert_eq!(record.boundary, state.boundary);
        }
        if heat_flow <= 0.0 {
            // Plates that do not move meet at transforms alone, and their floor is the oldest.
            assert!(
                generated
                    .working
                    .cells
                    .iter()
                    .all(|c| c.boundary == BoundaryKind::Transform)
            );
        }
    }
}

#[test]
fn the_reference_earth_s_ridges_spread_at_about_earth_s_rate() {
    let generated = earth();
    let (mut sum, mut cells) = (0.0, 0.0);
    for contact in generated.layout.contact.iter().flatten() {
        if contact.motion == Motion::Divergent {
            sum += contact.half_rate_m_per_s();
            cells += 1.0;
        }
    }
    let mm_a = sum / cells * SECONDS_PER_JULIAN_YEAR * 1e3;
    eprintln!("the reference Earth's mean ridge half-rate: {mm_a:.2} mm a⁻¹");
    assert!((mm_a - 23.0).abs() <= 0.3 * 23.0, "{mm_a} mm a⁻¹");
    let degrees_per_myr =
        EARTH_ANGULAR_SPEED_RAD_PER_S * SECONDS_PER_MEGAYEAR * 180.0 / core::f64::consts::PI;
    assert!(
        (0.27..0.29).contains(&degrees_per_myr),
        "{degrees_per_myr}° Myr⁻¹"
    );
}
