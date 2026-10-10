//! The channel network's tests (plan R09, T6.a): Hack's law in SI, the key points, the first level
//! against the coarse flow, the later levels' joins and profiles, the network's containment in its
//! coarse cells, face edges, order and the cells it reads.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::field::{ClimateCell, CoarseCrater, CoarseField, FlowDirection, cell_at_index};
use crate::testing::{SyntheticWorld, synthetic_field};

/// The detail seed the tests draw from.
const SEED: DetailSeed = DetailSeed::new(0x6368_616e_6e65_6c73);

/// The finer levels below the first that the tests build: the first levels, L + 1 to L + 3.
const DEPTH: u8 = 3;

/// Each synthetic world, built once.
fn world(kind: SyntheticWorld) -> &'static CoarseField {
    static FIELDS: OnceLock<Vec<CoarseField>> = OnceLock::new();
    let fields = FIELDS.get_or_init(|| {
        SyntheticWorld::ALL
            .iter()
            .map(|&k| synthetic_field(k))
            .collect()
    });
    let at = SyntheticWorld::ALL
        .iter()
        .position(|&k| k == kind)
        .expect("every world is listed");
    &fields[at]
}

fn earth() -> &'static CoarseField {
    world(SyntheticWorld::EarthLike)
}

fn mars() -> &'static CoarseField {
    world(SyntheticWorld::MarsLike)
}

/// Every cell of `field`'s level, in cell-index order.
fn coarse_cells(field: &CoarseField) -> Vec<PatchKey> {
    let level = field.header().level();
    (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).unwrap())
        .collect()
}

/// About `count` of `field`'s cells with channels, spread through cell-index order, and every one
/// whose trunk crosses a face edge among the first `count` of those.
fn channelled(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let all: Vec<PatchKey> = coarse_cells(field)
        .into_iter()
        .filter(|&c| has_channels(field.cell(c).unwrap()))
        .collect();
    let step = (all.len() / count).max(1);
    let mut cells: Vec<PatchKey> = all.iter().copied().step_by(step).collect();
    cells.extend(
        all.iter()
            .copied()
            .filter(|&c| receiver(field, c).is_some_and(|r| r.face() != c.face()))
            .take(count),
    );
    cells.sort_unstable();
    cells.dedup();
    cells
}

/// The coarse cell `c`'s flow names, if any.
fn receiver(field: &CoarseField, c: PatchKey) -> Option<PatchKey> {
    field
        .cell(c)
        .unwrap()
        .flow
        .edge()
        .map(|e| c.edge_neighbour(e))
}

/// Every cell below `coarse` at the levels L + 1 to L + `depth`, level by level, each level in
/// (i, j) order.
fn cells_below(coarse: PatchKey, depth: u8) -> Vec<PatchKey> {
    let mut cells = Vec::new();
    let mut ring = vec![coarse];
    for _ in 0..depth {
        ring = ring.iter().flat_map(|c| c.children()).collect();
        ring.sort_unstable();
        cells.extend(&ring);
    }
    cells
}

/// The unit direction of the spheroid point `p` (metres) of `field`'s body.
fn dir_of(field: &CoarseField, p: [f64; 3]) -> [f64; 3] {
    let figure = field.header().figure();
    unit_dir([
        p[0] / figure.equatorial_radius_m,
        p[1] / figure.equatorial_radius_m,
        p[2] / figure.polar_radius_m,
    ])
}

/// The coarse cell that holds the spheroid point `p` of `field`'s body.
fn coarse_cell_of(field: &CoarseField, p: [f64; 3]) -> PatchKey {
    PatchKey::containing(field.header().level().get(), dir_of(field, p)).unwrap()
}

/// The bits of every value a segment holds, for comparing two segments exactly.
fn segment_bits(s: &Segment) -> Vec<u64> {
    let mut out = vec![s.cell().to_u64()];
    out.extend(s.from_m().map(bits));
    out.extend(s.to_m().map(bits));
    out.extend([bits(s.bed_from().value()), bits(s.bed_to().value())]);
    match s.outlet() {
        Outlet::Cell { receiver, exit } => out.extend([0, receiver.to_u64(), bits(exit)]),
        Outlet::Join { parent, at } => out.extend([1, parent.to_u64(), bits(at)]),
    }
    for t in [0.0, 0.25, 0.5, 1.0] {
        out.extend([
            bits(s.bed_at(t).value()),
            bits(s.area_at(t).value()),
            bits(s.main_stream_at(t).value()),
            bits(s.slope_at(t)),
        ]);
    }
    out
}

/// Every segment of the coarse cell `c` and the cells below it to [`DEPTH`], as bits, in order.
fn network_bits(network: &mut ChannelNetwork<'_, impl FieldView>, c: PatchKey) -> Vec<Vec<u64>> {
    std::iter::once(c)
        .chain(cells_below(c, DEPTH))
        .map(|cell| {
            network
                .segment(cell)
                .unwrap()
                .map_or_else(Vec::new, |s| segment_bits(&s))
        })
        .collect()
}

#[test]
fn hack_s_law_is_in_si() {
    // 1.4 L in miles for A in square miles, in metres for square metres (the international mile).
    let converted = 1.4 * math::powf(1_609.344, 1.0 - 2.0 * HACK_EXPONENT);
    assert!(
        (converted / HACK_COEFFICIENT - 1.0).abs() < 1e-6,
        "{converted}"
    );
    for area in [1e6, 1e8, 1e10, 1e12] {
        let back = hack_area(main_stream_length(SquareMetres::new(area))).value();
        assert!((back / area - 1.0).abs() < 1e-12, "{area}: {back}");
    }
    assert_same_bits(main_stream_length(SquareMetres::ZERO).value(), 0.0);
    // The brainstorm's former 1.5, a mile-based figure, would give 1,500 km.
    let miles_figure = 1.5 * math::powf(1e10, HACK_EXPONENT);
    assert!(miles_figure > 1.4e6, "{miles_figure}");
}

#[test]
fn a_ten_thousand_square_kilometre_basin_has_a_main_stream_of_280_to_360_km() {
    let bracket = 280e3..=360e3;
    let length = main_stream_length(SquareMetres::new(1e10)).value();
    assert!(bracket.contains(&length), "{length} m");
    // Through the network's area-to-length path: every trunk of the Earth-like world draining 10⁴
    // km² to 5% (a trunk's main stream is Hack's of its area, so this checks the path, not a
    // geometric length).
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    let mut basins = 0;
    for c in coarse_cells(field) {
        let Some(trunk) = network.segment(c).unwrap() else {
            continue;
        };
        if (trunk.area_at(0.0).value() / 1e10 - 1.0).abs() > 0.05 {
            continue;
        }
        basins += 1;
        for t in [0.0, 1.0] {
            let main = trunk.main_stream_at(t).value();
            assert!(bracket.contains(&main), "{c:?}: {main} m");
        }
    }
    assert!(basins > 0, "the Earth-like world has a basin of 10⁴ km²");
}

#[test]
fn head_lengths_are_hack_s_of_a_mean_cell() {
    let earth = Metres::new(6.371e6);
    for (level, expected_m) in [(9, 40_866.0), (12, 3_370.2), (20, 4.3427)] {
        let head = head_length(earth, level).value();
        assert!((head / expected_m - 1.0).abs() < 1e-4, "{level}: {head}");
    }
    // A level's cells are a quarter of the one above's, so its head is 4^−h of it.
    let ratio = head_length(earth, 13).value() / head_length(earth, 12).value();
    assert!(
        (ratio - math::powf(0.25, HACK_EXPONENT)).abs() < 1e-12,
        "{ratio}"
    );
}

#[test]
fn the_fixed_point_layout_holds_the_margin_and_the_deepest_level() {
    assert_eq!(POSITION_BITS, u32::from(MAX_LEVEL) + CELL_BITS);
    assert_same_bits(
        POSITION_SCALE,
        f64::from(1_u32 << 25) * f64::from(1_u32 << 25),
    );
    #[expect(clippy::cast_precision_loss, reason = "2^24 and 2^26 are exact")]
    let margin = MARGIN_UNITS as f64 / f64::from(1_u32 << CELL_BITS);
    assert_same_bits(margin, KEY_POINT_MARGIN);
    // The draws span [ε, 1 − ε) exactly.
    assert_eq!(
        MARGIN_UNITS + (JITTER_STEP << JITTER_BITS),
        (1 << CELL_BITS) - MARGIN_UNITS
    );
}

#[test]
fn key_points_lie_in_their_cells_and_three_children_draw_their_own() {
    for field in [earth(), mars()] {
        let channels = Channels::new(field.header(), SEED);
        let mut network = channels.network(field);
        for c in channelled(field, 6) {
            for cell in std::iter::once(c).chain(cells_below(c, 4)) {
                let point = network.key_point(cell);
                assert!(point.is_in(cell), "{cell:?}");
                assert_eq!(
                    PatchKey::containing(cell.level(), point.dir()).unwrap(),
                    cell,
                    "the direction's cell"
                );
                if network.is_inherited(cell) {
                    assert_eq!(point, network.key_point(cell.parent().unwrap()));
                    continue;
                }
                // A drawn point lies in its cell's central half.
                let shift = u32::from(MAX_LEVEL - cell.level());
                let within = |c: u64| (c >> shift) & ((1 << CELL_BITS) - 1);
                let half = MARGIN_UNITS..((1 << CELL_BITS) - MARGIN_UNITS);
                assert!(half.contains(&within(point.s)), "{cell:?}");
                assert!(half.contains(&within(point.t)), "{cell:?}");
            }
            for parent in std::iter::once(c).chain(cells_below(c, 3)) {
                let inherited = parent
                    .children()
                    .into_iter()
                    .filter(|&child| network.is_inherited(child))
                    .count();
                assert_eq!(inherited, 1, "{parent:?}");
            }
        }
    }
}

#[test]
fn the_first_level_is_the_coarse_flow_network() {
    for field in [earth(), mars()] {
        let channels = Channels::new(field.header(), SEED);
        let mut network = channels.network(field);
        let figure = field.header().figure();
        let (mut trunks, mut falling) = (0, 0);
        for c in coarse_cells(field) {
            let record = field.cell(c).unwrap();
            let segment = network.segment(c).unwrap();
            let Some(edge) = record.flow.edge() else {
                assert_eq!(segment, None, "{c:?}");
                continue;
            };
            let trunk = segment.expect("a cell with a flow has a trunk");
            let r = c.edge_neighbour(edge);
            trunks += 1;
            assert!(trunk.is_trunk());
            assert_eq!(trunk.level(), c.level());
            let key = |p: KeyPoint| figure.point(p.dir()).map(bits);
            assert_eq!(trunk.from_m().map(bits), key(network.key_point(c)));
            assert_eq!(trunk.to_m().map(bits), key(network.key_point(r)));
            assert_same_bits(trunk.bed_from().value(), record.water_surface().value());
            assert_same_bits(
                trunk.bed_to().value(),
                field.cell(r).unwrap().water_surface().value(),
            );
            assert_same_bits(trunk.area_at(0.5).value(), record.drainage.area().value());
            let Outlet::Cell { receiver, exit } = trunk.outlet() else {
                panic!("a trunk flows into a cell");
            };
            assert_eq!(receiver, r);
            assert!(0.0 < exit && exit < 1.0, "{c:?}: {exit}");
            assert!(trunk.bed_from().value() >= trunk.bed_to().value(), "{c:?}");
            if trunk.bed_from().value() > trunk.bed_to().value() {
                falling += 1;
            }
        }
        assert!(trunks > 1_000, "{trunks}");
        assert!(falling * 100 >= trunks * 99, "{falling} of {trunks}");
    }
}

#[test]
fn worlds_without_a_fluvial_step_have_no_channels() {
    for kind in [
        SyntheticWorld::MoonLike,
        SyntheticWorld::CeresLike,
        SyntheticWorld::Flat,
        SyntheticWorld::OneCrater,
    ] {
        let field = world(kind);
        let channels = Channels::new(field.header(), SEED);
        let mut network = channels.network(field);
        let cells = coarse_cells(field);
        for &c in cells.iter().step_by(37) {
            assert_eq!(field.cell(c).unwrap().flow, FlowDirection::Terminal);
            for cell in std::iter::once(c).chain(cells_below(c, 2)) {
                assert_eq!(network.segment(cell).unwrap(), None, "{kind:?} {cell:?}");
            }
        }
    }
}

/// Follows `segment`'s parents to the first level, returning the trunk it reaches.
fn trunk_below(network: &mut ChannelNetwork<'_, CoarseField>, segment: Segment) -> Segment {
    let mut at = segment;
    while let Outlet::Join { parent, .. } = at.outlet() {
        at = network
            .segment(parent)
            .unwrap()
            .expect("a join's parent exists");
    }
    at
}

/// The unit normal of the great circle through `cell`'s `edge`.
fn edge_normal(cell: PatchKey, edge: Edge) -> [f64; 3] {
    let ((x0, y0), (x1, y1)) = match edge {
        Edge::UMin => ((0, 0), (0, 64)),
        Edge::UMax => ((64, 0), (64, 64)),
        Edge::VMin => ((0, 0), (64, 0)),
        Edge::VMax => ((0, 64), (64, 64)),
    };
    unit_dir(cross(cell.vertex_dir(x0, y0), cell.vertex_dir(x1, y1)))
}

#[test]
fn the_network_leaves_each_coarse_cell_through_the_cell_its_flow_names() {
    for (field, count) in [(earth(), 30), (mars(), 15)] {
        let channels = Channels::new(field.header(), SEED);
        let mut network = channels.network(field);
        let (mut trunks, mut held, mut held_across) = (0, 0, 0);
        for c in coarse_cells(field) {
            let Some(trunk) = network.segment(c).unwrap() else {
                continue;
            };
            let edge = field.cell(c).unwrap().flow.edge().unwrap();
            let r = c.edge_neighbour(edge);
            let Outlet::Cell { exit, .. } = trunk.outlet() else {
                panic!("a trunk flows into a cell");
            };
            trunks += 1;
            // Its first chord lies in its cell and its second in the cell its flow names.
            for step in 0..16 {
                let before = exit * f64::from(step) / 16.0;
                assert_eq!(coarse_cell_of(field, trunk.point_m(before)), c, "{c:?}");
                let after = exit + (1.0 - exit) * f64::from(step + 1) / 16.0;
                assert_eq!(coarse_cell_of(field, trunk.point_m(after)), r, "{c:?}");
            }
            // It crosses on the shared edge's great circle; the same value as the crossing, since
            // (1 − 1) a + b is b but for the sign of a zero.
            let crossing = trunk.crossing_m().unwrap();
            let value = |p: [f64; 3]| p.map(|x| bits(x + 0.0));
            assert_eq!(value(trunk.point_m(exit)), value(crossing));
            let off = dot(edge_normal(c, edge), dir_of(field, crossing));
            assert!(off.abs() < 1e-13, "{c:?}: {off}");
            // Held to the edge's central half, it leaves the great circle through the key points.
            let through = unit_dir(cross(
                dir_of(field, trunk.from_m()),
                dir_of(field, trunk.to_m()),
            ));
            if dot(through, dir_of(field, crossing)).abs() > 1e-12 {
                held += 1;
                if r.face() != c.face() {
                    held_across += 1;
                }
            }
        }
        // Held only near a few cube corners: 77 of 121,310 on the Earth-like world and 140 of
        // 98,229 on the Mars-like one, all but one of them across face edges.
        assert!(held * 200 < trunks, "{held} of {trunks}");
        assert!(held_across * 10 >= held * 9, "{held_across} of {held}");
        // And every finer segment's chain of parents exists and ends at a trunk.
        for c in channelled(field, count) {
            for cell in cells_below(c, DEPTH) {
                if let Some(segment) = network.segment(cell).unwrap() {
                    assert!(trunk_below(&mut network, segment).is_trunk(), "{cell:?}");
                }
            }
        }
    }
}

#[test]
fn every_segment_descends_to_its_parent_within_the_minimum_slope() {
    for (field, count) in [(earth(), 30), (mars(), 15)] {
        let channels = Channels::new(field.header(), SEED);
        let mut network = channels.network(field);
        for c in channelled(field, count) {
            for cell in cells_below(c, DEPTH) {
                let Some(segment) = network.segment(cell).unwrap() else {
                    continue;
                };
                let Outlet::Join { parent, at } = segment.outlet() else {
                    panic!("a finer level's segment joins");
                };
                assert!(parent.level() < cell.level(), "{cell:?}");
                let above = network.segment(parent).unwrap().expect("the parent exists");
                assert_same_bits(segment.bed_to().value(), above.bed_at(at).value());
                assert_eq!(
                    segment.to_m().map(bits),
                    above.point_m(at).map(bits),
                    "{cell:?}"
                );
                let length = segment.length().value();
                let drop = segment.bed_from().value() - segment.bed_to().value();
                assert!(length > 0.0, "{cell:?}");
                // Within 10⁻⁹, far above the rounding of the bed's sum (about 10⁻¹² of the rise).
                assert!(
                    drop >= MINIMUM_SLOPE * length * (1.0 - 1e-9),
                    "{cell:?} falls {drop} m over {length} m"
                );
                let mut last = segment.bed_from().value();
                for step in 1..=16 {
                    let bed = segment.bed_at(f64::from(step) / 16.0).value();
                    assert!(bed < last, "{cell:?} rises at {step} ÷ 16");
                    last = bed;
                }
            }
        }
    }
}

#[test]
fn a_tributary_s_profile_is_stream_power_with_hack_s_areas() {
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    let mut network = channels.network(field);
    let mut checked = 0;
    for c in channelled(field, 8) {
        for cell in cells_below(c, DEPTH) {
            let Some(segment) = network.segment(cell).unwrap() else {
                continue;
            };
            let k_s = segment.steepness_m0_9().unwrap();
            assert_same_bits(k_s, field.cell(c).unwrap().steepness.index_m0_9());
            let length = segment.length().value();
            for t in [0.1, 0.5, 0.9] {
                let area = segment.area_at(t).value();
                assert_same_bits(area, hack_area(segment.main_stream_at(t)).value());
                let law = k_s * math::powf(area, -LogSteepness::THETA);
                let slope = segment.slope_at(t);
                assert!((slope / law - 1.0).abs() < 1e-12, "{cell:?}: {slope} {law}");
                // The bed's fall per metre along the segment, by a central difference.
                let dt = 1e-4;
                let fall = (segment.bed_at(t - dt).value() - segment.bed_at(t + dt).value())
                    / (2.0 * dt * length);
                assert!(
                    (fall / slope - 1.0).abs() < 1e-6,
                    "{cell:?}: {fall} {slope}"
                );
            }
            let head = head_length(field.header().radius(), cell.level());
            assert_same_bits(segment.main_stream_at(0.0).value(), head.value());
            assert_same_bits(
                segment.bed_from().value(),
                segment.bed_to().value() + tributary_rise(k_s, head, segment.length()).value(),
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked}");
}

/// ∫ max(`k_s` C^¾ L^−¾, `S_min`) dL from `from` to `to` by Simpson's rule in u = L^¼, where the
/// integrand is smooth but for the floor's kink, over `n` intervals.
fn rise_by_quadrature(steepness: f64, from: f64, to: f64, n: u32) -> f64 {
    let (u0, u1) = (fourth_root(from), fourth_root(to));
    let h = (u1 - u0) / f64::from(n);
    // dL = 4u³ du.
    let f = |u: f64| tributary_slope(steepness, u * u * u * u) * 4.0 * u * u * u;
    let mut sum = f(u0) + f(u1);
    for k in 1..n {
        let weight = if k % 2 == 1 { 4.0 } else { 2.0 };
        sum += weight * f(u0 + f64::from(k) * h);
    }
    sum * h / 3.0
}

#[test]
fn the_closed_form_rise_is_the_integral_of_the_slope() {
    // The floor binds beyond the crossover: none at k_s = 50, from 1.02 km at the smallest code.
    let smallest = LogSteepness::new(1).index_m0_9();
    for (steepness, from, to) in [
        (50.0, 4.3, 120.0),
        (50.0, 40e3, 90e3),
        (3.0, 1e3, 3e5),
        (smallest, 10.0, 900.0),
        (smallest, 10.0, 5e4),
        (smallest, 2e3, 5e4),
    ] {
        let closed = rise(steepness, from, to);
        let gentle = gentle_from(steepness);
        // Split at the kink, where Simpson's rule would lose its order.
        let numeric = if from < gentle && gentle < to {
            rise_by_quadrature(steepness, from, gentle, 2_000)
                + rise_by_quadrature(steepness, gentle, to, 2_000)
        } else {
            rise_by_quadrature(steepness, from, to, 2_000)
        };
        assert!(
            (closed / numeric - 1.0).abs() < 1e-9,
            "{steepness} {from} {to}: {closed} {numeric}"
        );
        assert_same_bits(rise(steepness, to, to), 0.0);
    }
    let gentle = gentle_from(smallest);
    assert!((1.0e3..1.1e3).contains(&gentle), "{gentle}");
    let at_gentle = tributary_slope(smallest, gentle);
    assert!(
        (at_gentle / MINIMUM_SLOPE - 1.0).abs() < 1e-12,
        "{at_gentle}"
    );
}

#[test]
fn a_key_point_next_to_a_face_edge_gives_the_same_segments_from_either_face() {
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    // Coarse cells whose trunks cross a face edge, from either side.
    let crossing: Vec<(PatchKey, PatchKey)> = coarse_cells(field)
        .into_iter()
        .filter_map(|c| receiver(field, c).map(|r| (c, r)))
        .filter(|(c, r)| c.face() != r.face() && has_channels(field.cell(*c).unwrap()))
        .take(12)
        .collect();
    assert!(crossing.len() >= 6, "{}", crossing.len());
    let mut across_faces = 0;
    for (c, r) in crossing {
        // From c's face first, then from r's; and from r's face first, then from c's.
        let mut from_c = channels.network(field);
        let c_first = (network_bits(&mut from_c, c), network_bits(&mut from_c, r));
        let mut from_r = channels.network(field);
        let r_bits = network_bits(&mut from_r, r);
        let c_bits = network_bits(&mut from_r, c);
        assert_eq!(c_first, (c_bits, r_bits), "{c:?} into {r:?}");
        // Joins across the face edge, which the graph neighbourhoods reach.
        for cell in cells_below(c, DEPTH)
            .into_iter()
            .chain(cells_below(r, DEPTH))
        {
            if let Some(Outlet::Join { parent, .. }) =
                from_c.segment(cell).unwrap().map(|s| s.outlet())
                && parent.face() != cell.face()
            {
                across_faces += 1;
            }
        }
        // The trunk leaves c on the face edge, the plane through the centre between the faces.
        let trunk = from_c.segment(c).unwrap().unwrap();
        let Outlet::Cell { exit, .. } = trunk.outlet() else {
            panic!("a trunk flows into a cell");
        };
        let d = dir_of(field, trunk.point_m(exit));
        let axis = |f: Face| usize::from(f.index() % 3);
        let sign = |f: Face| if f.index() < 3 { 1.0 } else { -1.0 };
        let gap = sign(c.face()) * d[axis(c.face())] - sign(r.face()) * d[axis(r.face())];
        assert!(gap.abs() < 1e-12, "{c:?}: {gap}");
        // Every key point of c's cells next to the face edge, asked from r's side (as an edge
        // neighbour of one of r's cells), is c's own, and heads the same segment.
        for cell in cells_below(r, DEPTH) {
            for edge in Edge::ALL {
                let across = cell.edge_neighbour(edge);
                if across.face() != c.face() || ancestor(across, c.level()) != c {
                    continue;
                }
                let mut fresh = channels.network(field);
                assert_eq!(fresh.key_point(across), from_c.key_point(across));
                assert_eq!(
                    fresh.segment(across).unwrap().map(|s| segment_bits(&s)),
                    from_c.segment(across).unwrap().map(|s| segment_bits(&s)),
                    "{across:?}"
                );
            }
        }
    }
    assert!(across_faces > 0, "no tributary joins across a face edge");
}

#[test]
fn the_network_does_not_depend_on_the_order_it_is_built_in() {
    let field = earth();
    let channels = Channels::new(field.header(), SEED);
    let coarse = channelled(field, 3);
    let cells: Vec<PatchKey> = coarse
        .iter()
        .flat_map(|&c| std::iter::once(c).chain(cells_below(c, DEPTH)))
        .collect();
    // Each cell alone, in a fresh network.
    let alone: Vec<Option<Vec<u64>>> = cells
        .iter()
        .map(|&cell| {
            channels
                .network(field)
                .segment(cell)
                .unwrap()
                .map(|s| segment_bits(&s))
        })
        .collect();
    // All of them in one fresh network, in four orders: forwards, backwards, finest level first,
    // and a fixed shuffle.
    let mut finest_first: Vec<usize> = (0..cells.len()).collect();
    finest_first.sort_by_key(|&k| std::cmp::Reverse(cells[k].level()));
    let mut shuffled: Vec<usize> = (0..cells.len()).collect();
    let mut rng = hyperion_testkit::lcg::Lcg::new(0x006f_7264_6572);
    for k in (1..shuffled.len()).rev() {
        let pick = usize::try_from(rng.next_below(u64::try_from(k + 1).unwrap())).unwrap();
        shuffled.swap(k, pick);
    }
    let orders = [
        (0..cells.len()).collect::<Vec<usize>>(),
        (0..cells.len()).rev().collect(),
        finest_first,
        shuffled,
    ];
    for order in orders {
        let mut network = channels.network(field);
        let mut built = vec![None; cells.len()];
        for k in order {
            built[k] = Some(network.segment(cells[k]).unwrap().map(|s| segment_bits(&s)));
        }
        let built: Vec<Option<Vec<u64>>> = built.into_iter().map(Option::unwrap).collect();
        assert_eq!(built, alone);
    }
    // And through one warm network shared by every pass.
    let shared = RefCell::new(channels.network(field));
    assert_order_independent(&cells, |&cell| {
        shared
            .borrow_mut()
            .segment(cell)
            .unwrap()
            .map(|s| segment_bits(&s))
    });
}

#[test]
fn the_same_seed_draws_the_same_network_and_another_seed_another() {
    let field = earth();
    let c = channelled(field, 1)[0];
    let once = Channels::new(field.header(), SEED);
    let twice = Channels::new(field.header(), SEED);
    assert_eq!(
        network_bits(&mut once.network(field), c),
        network_bits(&mut twice.network(field), c)
    );
    let other = Channels::new(field.header(), DetailSeed::new(SEED.get() ^ 1));
    assert_ne!(
        once.network(field).key_point(c),
        other.network(field).key_point(c)
    );
}

/// A view of `inner` that records every cell it is asked for.
struct Recorder<'a> {
    inner: &'a CoarseField,
    reads: RefCell<BTreeSet<PatchKey>>,
}

impl FieldView for Recorder<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.reads.borrow_mut().insert(cell);
        self.inner.cell(cell)
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.reads.borrow_mut().insert(cell);
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.reads.borrow_mut().insert(cell);
        self.inner.craters_reaching(cell)
    }
}

/// A view of `inner` without one cell.
struct Without<'a> {
    inner: &'a CoarseField,
    missing: PatchKey,
}

impl FieldView for Without<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        (cell != self.missing)
            .then(|| self.inner.cell(cell))
            .flatten()
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        (cell != self.missing)
            .then(|| self.inner.climate(cell))
            .flatten()
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        let held = cell != self.missing;
        self.inner.craters_reaching(cell).filter(move |_| held)
    }
}

/// The cells within `radius` king moves of `cell` (edge and corner neighbours, across face edges
/// by the cube's neighbour rule), each with its distance.
fn king_distances(cell: PatchKey, radius: u8) -> std::collections::BTreeMap<PatchKey, u8> {
    let mut distances = std::collections::BTreeMap::from([(cell, 0)]);
    let mut ring = vec![cell];
    for step in 1..=radius {
        let mut next = Vec::new();
        for key in ring {
            for n in neighbourhood(key).into_iter().flatten() {
                distances.entry(n).or_insert_with(|| {
                    next.push(n);
                    step
                });
            }
        }
        ring = next;
    }
    distances
}

/// The farthest king moves from a coarse cell that its segments' reads reach, to L + 3.
const READ_RADIUS: u8 = 3;

#[test]
fn the_segments_below_a_coarse_cell_read_within_three_king_moves_of_it() {
    let mut farthest = 0;
    for field in [earth(), mars()] {
        let channels = Channels::new(field.header(), SEED);
        for c in channelled(field, 8) {
            let view = Recorder {
                inner: field,
                reads: RefCell::new(BTreeSet::new()),
            };
            let mut network = channels.network(&view);
            for cell in cells_below(c, DEPTH) {
                let _ = network.segment(cell).unwrap();
            }
            let within = king_distances(c, READ_RADIUS + 2);
            for read in view.reads.take() {
                let d = within
                    .get(&read)
                    .copied()
                    .unwrap_or_else(|| panic!("{c:?} read {read:?}, beyond 5 king moves"));
                farthest = farthest.max(d);
            }
            // Without the coarse cell itself, the first tributary names it.
            let view = Without {
                inner: field,
                missing: c,
            };
            let mut network = channels.network(&view);
            let first_failure = cells_below(c, DEPTH)
                .into_iter()
                .find_map(|cell| network.segment(cell).err());
            assert_eq!(first_failure, Some(ReadCellError { cell: c }));
            // Without a cell two king moves away, some answers fail and some do not, and every
            // answer, error or not, is the same in any order through one network.
            let view = Without {
                inner: field,
                missing: c.edge_neighbour(Edge::UMax).edge_neighbour(Edge::UMax),
            };
            let shared = RefCell::new(channels.network(&view));
            let below = cells_below(c, 2);
            assert_order_independent(&below, |&cell| {
                shared
                    .borrow_mut()
                    .segment(cell)
                    .map(|s| s.map(|s| segment_bits(&s)))
            });
        }
    }
    assert_eq!(farthest, READ_RADIUS);
}
