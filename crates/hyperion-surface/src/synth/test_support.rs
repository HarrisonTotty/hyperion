//! The views, worlds and points the synthesis's whole-function tests share (R09.T8), with views and
//! a king-move count of their own as R09.T4's interpolant tests keep theirs.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use hyperion_base::math;
use hyperion_base::rng::DetailSeed;
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

use crate::cube::{Edge, Face, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use crate::field::{ClimateCell, CoarseCrater, CoarseField, FieldHeader, FieldView, SynthesisCell};
use crate::height::HeightSample;
use crate::testing::{SyntheticWorld, synthetic_field};

/// The detail seed the whole-function tests draw from.
pub(crate) const SEED: DetailSeed = DetailSeed::new(0x6173_7365_6d62_6c79);

/// The field of `world`, built once per test process.
pub(crate) fn world(world: SyntheticWorld) -> &'static CoarseField {
    const WORLDS: usize = SyntheticWorld::ALL.len();
    static FIELDS: [OnceLock<CoarseField>; WORLDS] = [const { OnceLock::new() }; WORLDS];
    let at = SyntheticWorld::ALL
        .iter()
        .position(|&w| w == world)
        .expect("every world is listed");
    FIELDS[at].get_or_init(|| synthetic_field(world))
}

/// A view of `inner` that records every cell asked of it.
pub(crate) struct Recorder<'a> {
    pub(crate) inner: &'a CoarseField,
    reads: RefCell<Vec<PatchKey>>,
}

impl<'a> Recorder<'a> {
    pub(crate) fn new(inner: &'a CoarseField) -> Self {
        Self {
            inner,
            reads: RefCell::new(Vec::new()),
        }
    }

    /// The cells read since the last call, sorted and once each.
    pub(crate) fn take(&self) -> Vec<PatchKey> {
        let mut reads = self.reads.take();
        reads.sort_unstable();
        reads.dedup();
        reads
    }
}

impl FieldView for Recorder<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.reads.borrow_mut().push(cell);
        self.inner.cell(cell)
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.reads.borrow_mut().push(cell);
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.reads.borrow_mut().push(cell);
        self.inner.craters_reaching(cell)
    }
}

/// A view of `inner` that holds only the cells of `held`.
pub(crate) struct Held<'a> {
    pub(crate) inner: &'a CoarseField,
    pub(crate) held: BTreeSet<PatchKey>,
}

impl FieldView for Held<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.held
            .contains(&cell)
            .then(|| self.inner.cell(cell))
            .flatten()
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.held
            .contains(&cell)
            .then(|| self.inner.climate(cell))
            .flatten()
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        let held = self.held.contains(&cell);
        self.inner.craters_reaching(cell).filter(move |_| held)
    }
}

/// The cells within `radius` king moves of `cell` (edge and corner neighbours, across face edges
/// by the cube's neighbour rule), each with its distance: how
/// [`SYNTHESIS_MARGIN_CELLS`](crate::field::SYNTHESIS_MARGIN_CELLS) counts.
pub(crate) fn king_distances(cell: PatchKey, radius: u8) -> BTreeMap<PatchKey, u8> {
    let mut distances = BTreeMap::from([(cell, 0)]);
    let mut ring = vec![cell];
    for step in 1..=radius {
        let mut next = Vec::new();
        for key in ring {
            let around = Edge::ALL
                .into_iter()
                .map(|e| key.edge_neighbour(e))
                .chain(key.corner_neighbours().into_iter().flatten());
            for n in around {
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

/// The unit direction at face coordinates (`x`, `y`) of `cell`, each in [0, 1].
pub(crate) fn point_in(cell: PatchKey, x: f64, y: f64) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    let s = (f64::from(cell.i()) + x) / side;
    let t = (f64::from(cell.j()) + y) / side;
    unit_dir(face_uv_to_xyz(FaceUv {
        face: cell.face(),
        u: st_to_uv(s),
        v: st_to_uv(t),
    }))
}

/// The unit direction whose point on a sphere of radius `radius_m` lies `chord_m` metres, as a
/// chord, from `centre`'s, along the great circle towards +z: a crater's radial transect
/// (R10.T1.c's maturity tests and golden).
pub(crate) fn along_chord(centre: [f64; 3], chord_m: f64, radius_m: f64) -> [f64; 3] {
    let theta = 2.0 * math::asin(chord_m / (2.0 * radius_m));
    let towards = unit_dir([
        -centre[2] * centre[0],
        -centre[2] * centre[1],
        1.0 - centre[2] * centre[2],
    ]);
    let (s, c) = math::sin_cos(theta);
    unit_dir([0, 1, 2].map(|k| c * centre[k] + s * towards[k]))
}

/// `count` points at random in `cell`, even in its face coordinates.
pub(crate) fn random_points_in(cell: PatchKey, count: usize, rng: &mut Lcg) -> Vec<[f64; 3]> {
    (0..count)
        .map(|_| point_in(cell, rng.next_f64(), rng.next_f64()))
        .collect()
}

/// `count` cells of `level` at random on the sphere, with, first, a cell at a cube corner and two
/// on face edges, so that the cube's seams are always among them.
pub(crate) fn random_cells(level: u8, count: usize, rng: &mut Lcg) -> Vec<PatchKey> {
    let last = (1_u32 << level) - 1;
    let mut cells = vec![
        PatchKey::new(Face::PosX, level, 0, 0).expect("a cube corner's cell"),
        PatchKey::new(Face::NegY, level, last, last / 3).expect("a face edge's cell"),
        PatchKey::new(Face::PosZ, level, last / 2, 0).expect("a face edge's cell"),
    ];
    while cells.len() < count {
        let face = Face::ALL[usize::try_from(rng.next_below(6)).expect("a face index")];
        let pick = |rng: &mut Lcg| {
            u32::try_from(rng.next_below(u64::from(last) + 1)).expect("a cell index")
        };
        let (i, j) = (pick(rng), pick(rng));
        cells.push(PatchKey::new(face, level, i, j).expect("a cell of the level"));
    }
    cells.truncate(count);
    cells
}

/// The coarse cells under the rims of up to `count` of `field`'s listed craters, spread through
/// its list: each crater's cell under a point of its rim, a rim radius's chord from its centre,
/// so that the coarse craters' sharpening is among what a test samples.
pub(crate) fn rim_cells(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let header = field.header();
    let level = header.level().get();
    let radius_m = header.radius().value();
    let craters = field.craters();
    let stride = (craters.len() / count.max(1)).max(1);
    craters
        .iter()
        .step_by(stride)
        .take(count)
        .map(|crater| {
            let c = crater.centre;
            // A tangent at the centre, and the arc whose chord is the rim radius.
            let e = if c[2].abs() < 0.9 {
                [-c[1], c[0], 0.0]
            } else {
                [0.0, -c[2], c[1]]
            };
            let n = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
            let half_chord = 0.25 * crater.diameter.value() / radius_m;
            let angle = 2.0 * hyperion_base::math::asin(half_chord.min(1.0));
            let (sin, cos) = hyperion_base::math::sin_cos(angle);
            let dir = unit_dir([0, 1, 2].map(|k| cos * c[k] + sin * e[k] / n));
            PatchKey::containing(level, dir).expect("a coarse level is a level")
        })
        .collect()
}

/// The bits of a sample's height and gradient.
pub(crate) fn sample_bits(s: HeightSample) -> [u64; 4] {
    [
        bits(s.height_m),
        bits(s.gradient[0]),
        bits(s.gradient[1]),
        bits(s.gradient[2]),
    ]
}
