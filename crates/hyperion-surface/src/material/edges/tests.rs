//! The class edges' warp's tests (lane fix-jagged-biome-edges): its hold, its reach, its band
//! limit, its cache's neutrality, and the records' boundaries it moves off the cells' edges.

use hyperion_base::math;
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::cube::Edge;
use crate::field::{CoarseField, cell_at_index};
use crate::material::{material_at, weights_at};
use crate::synth::test_support::{SEED, world};
use crate::testing::SyntheticWorld;

/// The angle between two unit directions, rad.
fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let sin = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let cos = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    math::atan2(sin, cos)
}

/// `count` unit directions at random over the sphere.
fn directions(count: usize, rng: &mut Lcg) -> Vec<[f64; 3]> {
    (0..count)
        .map(|_| {
            let z = 2.0 * rng.next_f64() - 1.0;
            let phi = core::f64::consts::TAU * rng.next_f64();
            let r = (1.0 - z * z).max(0.0).sqrt();
            [r * math::cos(phi), r * math::sin(phi), z]
        })
        .collect()
}

/// The Earth-like world's field, a sea's coasts on it.
fn earth() -> &'static CoarseField {
    world(SyntheticWorld::EarthLike)
}

/// A band level.
fn band(level: u8) -> BandLevel {
    BandLevel::new(level).expect("a band level")
}

#[test]
fn the_warped_class_edges_wait_for_their_version() {
    // The tripwire of the hold at 32: the bump to 32 fails it. Then re-bless the class map's, the
    // bakes' and the materials' goldens at 32, the warp's own tests becoming the version's, and
    // retire `ClassEdgeRule::Cells` with this test.
    assert_eq!(
        ClassEdgeRule::of_version(crate::generator_version()),
        ClassEdgeRule::Cells,
        "generator version {} reads the class edges' warp: retire its hold",
        crate::generator_version()
    );
    assert!(crate::generator_version() < CLASS_EDGES_VERSION);
}

#[test]
fn the_rule_turns_at_its_version() {
    assert_eq!(ClassEdgeRule::of_version(31), ClassEdgeRule::Cells);
    assert_eq!(ClassEdgeRule::of_version(32), ClassEdgeRule::Warped);
    assert_eq!(ClassEdgeRule::of_version(40), ClassEdgeRule::Warped);
}

#[test]
fn before_its_version_every_point_reads_its_own_cell() {
    let field = earth();
    let synth = Synthesiser::new(field, SEED);
    assert_eq!(synth.class_edges().rule(), ClassEdgeRule::Cells);
    let mut cache = SynthCache::new();
    let mut rng = Lcg::new(0x6564_6765);
    for dir in directions(400, &mut rng) {
        for level in [0, 2, 9, 14] {
            let (own, _) = synth.cell_at(dir).expect("a whole field");
            let (read, _) = site_cell_at(&synth, &mut cache, dir, band(level)).expect("held");
            assert_eq!(read, own);
        }
    }
}

#[test]
fn the_warp_stays_on_the_sphere_within_its_reach() {
    let field = earth();
    let synth = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let edges = synth.class_edges();
    let cell = SMALLEST_CELL_SIDE / f64::from(1_u32 << field.header().level().get());
    let mut lattice = LatticeCache::new();
    let mut rng = Lcg::new(0x0072_6561_6368);
    let mut squares = 0.0;
    let points = directions(2_000, &mut rng);
    for &dir in &points {
        let moved = edges.warped(dir, band(14), &mut lattice);
        let size = (moved[0] * moved[0] + moved[1] * moved[1] + moved[2] * moved[2]).sqrt();
        assert!((size - 1.0).abs() < 1e-12, "{moved:?}");
        let a = angle(dir, moved);
        assert!(
            a <= MAX_WARP_CELLS * cell * (1.0 + 1e-9),
            "{a} rad at {dir:?}"
        );
        squares += (a / cell) * (a / cell);
    }
    #[expect(clippy::cast_precision_loss, reason = "2,000 points")]
    let rms = (squares / points.len() as f64).sqrt();
    // A quarter of a cell a component, about 0.35 along the sphere.
    assert!((0.25..0.45).contains(&rms), "the warp's RMS is {rms} cells");
}

#[test]
fn the_warp_is_band_limited_to_the_level_asked() {
    let field = earth();
    let level = field.header().level().get();
    let synth = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let edges = synth.class_edges();
    let mut lattice = LatticeCache::new();
    let mut rng = Lcg::new(0x6261_6e64);
    for dir in directions(200, &mut rng) {
        // Below L − 6 no octave is drawn: the class map's texels read their own cells.
        for n in 0..level.saturating_sub(6) {
            assert_eq!(edges.warped(dir, band(n), &mut lattice), dir);
        }
    }
    let ClassEdges::Warped(warp) = edges else {
        panic!("the warp's rule makes a warp");
    };
    assert_eq!(warp.drawn(band(level - 6)), 1);
    assert_eq!(warp.drawn(band(level - 5)), 2);
    assert_eq!(warp.drawn(band(20)), WARP_OCTAVES);
}

#[test]
fn the_noise_gradient_s_rms_is_pinned() {
    // One component of an octave's gradient per lattice unit, over 2¹⁶ points of a box 64 units on
    // a side, against GRADIENT_RMS, to 2%.
    let synth = Synthesiser::with_class_edge_rule(earth(), SEED, ClassEdgeRule::Warped);
    let ClassEdges::Warped(warp) = synth.class_edges() else {
        panic!("the warp's rule makes a warp");
    };
    let octave = warp.octaves[0];
    let mut lattice = LatticeCache::new();
    let mut squares = 0.0;
    let count = 1_u32 << 16;
    for a in 0..count {
        let t = f64::from(a);
        let p = [
            (t * 0.754_877_666_246_692_7).fract() * 64.0,
            (t * 0.569_840_290_998_053_2).fract() * 64.0,
            (t * 0.319_104_342_773_757_4).fract() * 64.0 + 0.5,
        ]
        .map(|c| c * octave.spacing());
        let (_, g) = crate::noise::gradient_noise(p, &octave, &mut lattice);
        squares += (g[0] * octave.spacing()) * (g[0] * octave.spacing());
    }
    let rms = (squares / f64::from(count)).sqrt();
    assert!((rms / GRADIENT_RMS - 1.0).abs() < 0.02, "{rms}");
}

#[test]
fn the_warp_is_the_same_whatever_its_cache_holds() {
    let field = earth();
    let synth = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let edges = synth.class_edges();
    let mut rng = Lcg::new(0x0063_6163_6865);
    let points = directions(300, &mut rng);
    let mut cold = LatticeCache::new();
    let mut warm = LatticeCache::new();
    for &dir in &points {
        edges.cover(dir, 1e-3, band(12), &mut warm);
        let a = edges.warped(dir, band(12), &mut cold);
        let b = edges.warped(dir, band(12), &mut warm);
        assert_eq!(a.map(bits), b.map(bits));
    }
}

/// The edges between surveyed cells of the Earth-like world whose records the rules read apart:
/// a cell under water beside one that is not.
fn coasts(field: &CoarseField, count: usize) -> Vec<(PatchKey, PatchKey)> {
    let level = field.header().level();
    let mut found = Vec::new();
    for n in 0..level.cell_count() {
        let key = cell_at_index(level.get(), n).expect("a cell of the level");
        let record = field.cell(key).expect("a whole field");
        if !record.is_under_water() {
            continue;
        }
        let beside = key.edge_neighbour(Edge::ALL[0]);
        if beside.face() == key.face()
            && !field.cell(beside).expect("a whole field").is_under_water()
        {
            found.push((key, beside));
        }
        if found.len() == count {
            break;
        }
    }
    found
}

/// Points along the line from `a`'s centre to `b`'s, `steps` + 1 of them.
fn across(a: PatchKey, b: PatchKey, steps: u32) -> Vec<[f64; 3]> {
    let ca = a.vertex_dir(32, 32);
    let cb = b.vertex_dir(32, 32);
    (0..=steps)
        .map(|k| {
            let t = f64::from(k) / f64::from(steps);
            unit_dir([0, 1, 2].map(|i| ca[i] + t * (cb[i] - ca[i])))
        })
        .collect()
}

#[test]
fn the_warp_moves_a_record_s_boundary_off_the_cells_edge() {
    let field = earth();
    let cells = Synthesiser::new(field, SEED);
    let warped = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let mut cache = SynthCache::new();
    let pairs = coasts(field, 24);
    assert!(pairs.len() >= 12, "{} coasts", pairs.len());
    let mut off_edge = 0;
    let mut switches = Vec::new();
    for &(sea, land) in &pairs {
        let points = across(sea, land, 200);
        // Where along the line the record read first holds no water, by step.
        let turn = |synth: &Synthesiser<'_, CoarseField>, cache: &mut SynthCache| {
            points
                .iter()
                .position(|&p| {
                    !site_cell_at(synth, cache, p, band(14))
                        .expect("held")
                        .1
                        .is_under_water()
                })
                // Under the warp the land's centre itself may read a sea's cell.
                .unwrap_or(points.len())
        };
        let own = turn(&cells, &mut cache);
        let moved = turn(&warped, &mut cache);
        switches.push(moved);
        if moved.abs_diff(own) > 4 {
            off_edge += 1;
        }
        // Within its reach of the edge, MAX_WARP_CELLS of the line's 200 steps a cell.
        assert!(
            moved.abs_diff(own) <= 180,
            "{sea:?} to {land:?}: {own} against {moved}"
        );
    }
    assert!(
        off_edge * 2 >= pairs.len(),
        "{off_edge} of {} boundaries moved off the edge",
        pairs.len()
    );
    switches.sort_unstable();
    switches.dedup();
    assert!(
        switches.len() > pairs.len() / 2,
        "the boundary moves by the place: {switches:?}"
    );
}

#[test]
fn the_warped_material_is_the_finest_weights_largest() {
    let field = earth();
    let synth = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let mut cache = SynthCache::new();
    let finest = super::super::classify::finest_band(field.header());
    for (sea, land) in coasts(field, 8) {
        for p in across(sea, land, 40) {
            let at = crate::cube::xyz_to_face_uv(p);
            let material = material_at(&synth, &mut cache, at).expect("held");
            let weights = weights_at(&synth, &mut cache, at, finest).expect("held");
            assert_eq!(material, weights.largest(), "at {at:?}");
        }
    }
}

/// The golden text of the warp (lane fix-jagged-biome-edges), `material/class_edges`: on the
/// Earth-like and Mars-like worlds under [`ClassEdgeRule::Warped`], 200 random directions'
/// warped directions at band levels L − 6, 9 and 14 (`w <face> <u> <v> <level> <x> <y> <z>`, each
/// float its IEEE 754 bits), then [`material_at`] at 21 points along each of 12 coasts' lines from
/// a sea's cell's centre to its land's (`m <face> <u> <v> <code> <class> <entry>`), so that the
/// targets agree on the warp bit for bit and the bump to 32 sees what it changes.
fn class_edges_text() -> String {
    use std::fmt::Write as _;

    use hyperion_testkit::golden::GoldenWriter;

    use crate::cube::xyz_to_face_uv;

    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let mut line = String::new();
    for w in [SyntheticWorld::EarthLike, SyntheticWorld::MarsLike] {
        let field = world(w);
        let level = field.header().level().get();
        let synth = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
        let mut cache = SynthCache::new();
        let mut lattice = LatticeCache::new();
        out.line(&format!("world {w:?}"));
        let mut rng = Lcg::new(0x676f_6c64_656e);
        for dir in directions(200, &mut rng) {
            let at = xyz_to_face_uv(dir);
            for n in [level - 6, 9, 14] {
                let moved = synth.class_edges().warped(dir, band(n), &mut lattice);
                line.clear();
                write!(
                    line,
                    "w {} 0x{:016x} 0x{:016x} {n} 0x{:016x} 0x{:016x} 0x{:016x}",
                    at.face.index(),
                    bits(at.u),
                    bits(at.v),
                    bits(moved[0]),
                    bits(moved[1]),
                    bits(moved[2])
                )
                .expect("a string takes any write");
                out.line(&line);
            }
        }
        for (sea, land) in coasts(field, 12) {
            for p in across(sea, land, 20) {
                let at = xyz_to_face_uv(p);
                let m =
                    material_at(&synth, &mut cache, at).expect("a whole field holds every cell");
                line.clear();
                write!(
                    line,
                    "m {} 0x{:016x} 0x{:016x} {} {:?} ",
                    at.face.index(),
                    bits(at.u),
                    bits(at.v),
                    u8::from(m.class()),
                    m.class()
                )
                .expect("a string takes any write");
                match m.entry() {
                    Some(entry) => write!(line, "{entry}"),
                    None => write!(line, "-"),
                }
                .expect("a string takes any write");
                out.line(&line);
            }
        }
    }
    out.finish()
}

#[test]
fn the_warp_matches_its_golden() {
    hyperion_testkit::golden!("material/class_edges", &class_edges_text());
}

#[test]
fn a_warped_point_reads_its_own_cell_or_one_a_king_move_away() {
    let field = earth();
    let level = field.header().level().get();
    let synth = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let mut lattice = LatticeCache::new();
    let mut rng = Lcg::new(0x6b69_6e67);
    for dir in directions(4_000, &mut rng) {
        let own = PatchKey::containing(level, dir).expect("a level");
        let read = PatchKey::containing(
            level,
            synth.class_edges().warped(dir, band(14), &mut lattice),
        )
        .expect("a level");
        if read.face() == own.face() {
            assert!(
                read.i().abs_diff(own.i()) <= 1 && read.j().abs_diff(own.j()) <= 1,
                "{own:?} read {read:?}"
            );
        }
    }
}

#[test]
fn the_warp_is_the_same_from_two_synthesisers_of_one_seed() {
    let field = earth();
    let a = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let b = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let mut rng = Lcg::new(0x0074_7769_6365);
    for dir in directions(300, &mut rng) {
        let one = a
            .class_edges()
            .warped(dir, band(12), &mut LatticeCache::new());
        let two = b
            .class_edges()
            .warped(dir, band(12), &mut LatticeCache::new());
        assert_eq!(one.map(bits), two.map(bits));
    }
}

#[test]
fn the_warped_classifier_answers_alike_in_any_order() {
    use hyperion_testkit::order::assert_order_independent;

    use crate::cube::xyz_to_face_uv;

    let field = earth();
    let synth = Synthesiser::with_class_edge_rule(field, SEED, ClassEdgeRule::Warped);
    let finest = super::super::classify::finest_band(field.header());
    let points: Vec<[f64; 3]> = coasts(field, 6)
        .into_iter()
        .flat_map(|(sea, land)| across(sea, land, 8))
        .collect();
    let shared = std::cell::RefCell::new(SynthCache::new());
    let answer = |cache: &mut SynthCache, p: [f64; 3]| {
        let at = xyz_to_face_uv(p);
        let m = material_at(&synth, cache, at).expect("held");
        let coarse = weights_at(&synth, cache, at, band(9)).expect("held");
        let fine = weights_at(&synth, cache, at, finest).expect("held");
        (m, coarse, fine)
    };
    assert_order_independent(&points, |&p| answer(&mut shared.borrow_mut(), p));
    for &p in &points {
        assert_eq!(
            answer(&mut SynthCache::new(), p),
            answer(&mut shared.borrow_mut(), p)
        );
    }
}
