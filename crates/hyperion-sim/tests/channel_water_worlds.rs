//! The channels kept out of the water on the reference Earth and the locked Earth (plan R09,
//! T6.f; decision-r09-channel-term §4, tests P1 and P2, and the census its §4 asks to record).
//!
//! On each world the cells fall into probe E's groups (the ruling's probe, reused): wet cells
//! that receive a dry cell's trunk (river mouths), lake cells (wet, their water surface off the sea
//! level by more than half a metre), cells under water and dry cells, drawn with replacement
//! (200, 200, 300 and 200 cells), and the dry cells whose trunk runs into water, 200 of them. A
//! cell's points are the centres of a 4 × 4 grid in its face coordinates, 16 a cell, at least
//! 3,200 points a group; a trunk's are 16 along its first chord, from its key point towards its
//! crossing. At band levels 5, 7 and 19 each point's height h is the synthesis's, its channel term
//! c the incision through the level's deepest channel level, and h₀ the ground without the
//! channels: the synthesis's height over the same field with every cell's flow terminal
//! ([`WithoutChannels`]), which draws no channel and sums the rest in the same order. Then:
//!
//! - **P1**: the channel term is exactly zero, height and gradient, at every point of a cell
//!   under water, so no wet point is turned to land by the channels;
//! - **P2**: the channels take no dry point more than 10 m below the sea level from at or above
//!   it (h below the sea less 10 m where h₀ is not, probe E's measure), every dry point stands at
//!   least as high as the lower of h₀ and the sea (the synthesis's floor, R09.T6.f), and no point
//!   on the axis of a trunk that runs into the sea is taken from at or above the sea to below it
//!   less the trunk's bankfull depth. Axis points of trunks into lakes above the sea that are cut
//!   so below their lake are counted, not asserted (the plan's Risks, a figure).
//!
//! The census each run prints is the plan's record: per group, the wet points the channels turned
//! to land and the dry points they cut below the sea, at each band level, and the variance of z
//! at band levels 3 and 4 (probe C's split), the omitted ground over the closed forms' RMS, for wet
//! and dry cells, which the fix brings to about one in wet cells; and the coarse pass's channel
//! share against the synthesis's (`print_the_budget`).
//!
//! The worlds are R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`), each a
//! `Tier::Slow` world with a test of its own, ignored: run with
//! `cargo nextest run -p hyperion-sim --features testing --test channel_water_worlds
//! --cargo-profile slow-test --profile slow --run-ignored only`.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the channels' tests use only the sample worlds of the shared helpers"
)]
mod common;

use std::collections::BTreeSet;

use common::surface_worlds::sample_world;
use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;
use hyperion_surface::cube::{Face, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use hyperion_surface::field::{
    ClimateCell, CoarseCrater, CoarseField, FieldHeader, FieldView, FlowDirection, LogRunoff,
    SynthesisCell, cell_at_index, cell_index,
};
use hyperion_surface::synth::channels::{ChannelBody, Outlet, channel_variance};
use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

/// The detail seed the worlds' channels are drawn with: probe E's (the R10.T2 lane's), so that
/// the census before the fix is the ruling's.
const DETAIL: DetailSeed = DetailSeed::new(0x7765_6967_6874_7377);

/// The seed of the groups' draws: probe E's.
const GROUPS: u64 = 0x5eed_0e0e;

/// The seed of probe C's points.
const SPLIT: u64 = 0x5eed_2a11;

/// The band levels checked: the trunks' first (the shelf texels' 5), a middle one and the finest.
const BANDS: [u8; 3] = [5, 7, 19];

/// How far below the sea a dry point may be cut from at or above it, metres (P2).
const BELOW_SEA_M: f64 = 10.0;

/// A cell's points a side: 4 × 4, 16 a cell.
const SIDE: u32 = 4;

/// The unit direction at face coordinates (`x`, `y`) of `cell`, each in [0, 1].
fn point_in(cell: PatchKey, x: f64, y: f64) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    let s = (f64::from(cell.i()) + x) / side;
    let t = (f64::from(cell.j()) + y) / side;
    unit_dir(face_uv_to_xyz(FaceUv {
        face: cell.face(),
        u: st_to_uv(s),
        v: st_to_uv(t),
    }))
}

/// `cell`'s points: the centres of a [`SIDE`] × [`SIDE`] grid in its face coordinates.
fn points_of(cell: PatchKey) -> Vec<[f64; 3]> {
    let n = f64::from(SIDE);
    (0..SIDE)
        .flat_map(|a| (0..SIDE).map(move |b| (a, b)))
        .map(|(a, b)| point_in(cell, (f64::from(a) + 0.5) / n, (f64::from(b) + 0.5) / n))
        .collect()
}

/// Every coarse cell of `field`, in face and (i, j) order.
fn all_cells(field: &CoarseField) -> impl Iterator<Item = PatchKey> + '_ {
    let level = field.header().level().get();
    let side = 1_u32 << level;
    Face::ALL.into_iter().flat_map(move |face| {
        (0..side).flat_map(move |i| {
            (0..side).map(move |j| PatchKey::new(face, level, i, j).expect("a cell of the field"))
        })
    })
}

/// A field's view with every coarse cell's flow terminal, so that the synthesis over it draws no
/// channel: no cell has a trunk or tributaries, and nothing else the height reads reads the flow.
/// Its heights are the field's without the channels, summed in the same order.
struct WithoutChannels<'a> {
    inner: &'a CoarseField,
    cells: Vec<SynthesisCell>,
}

impl<'a> WithoutChannels<'a> {
    fn of(inner: &'a CoarseField) -> Self {
        let level = inner.header().level();
        let cells = (0..level.cell_count())
            .map(|n| {
                let key = cell_at_index(level.get(), n).expect("a cell of the field");
                SynthesisCell {
                    flow: FlowDirection::Terminal,
                    ..*inner.cell(key).expect("the field holds its cells")
                }
            })
            .collect();
        Self { inner, cells }
    }
}

impl FieldView for WithoutChannels<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.inner.cell(cell)?;
        self.cells
            .get(usize::try_from(cell_index(cell)).expect("an index fits"))
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.inner.craters_reaching(cell)
    }
}

/// `n` cells of `list`, drawn with replacement by `rng`.
fn pick(rng: &mut Lcg, list: &[PatchKey], n: usize) -> Vec<PatchKey> {
    let len = u64::try_from(list.len()).expect("a list of cells fits");
    (0..n)
        .map(|_| list[usize::try_from(rng.next_below(len)).expect("an index fits")])
        .collect()
}

/// What a group's points showed at one band level.
#[derive(Debug, Default)]
struct Tally {
    points: u32,
    wet: u32,
    /// Wet points the channels took above their water surface from at or below it.
    turned_to_land: u32,
    /// Wet points whose channel term was not exactly zero.
    wet_term: u32,
    /// Dry points the channels took more than [`BELOW_SEA_M`] below the sea from at or above it.
    cut_below_sea: u32,
    /// How far below the sea those points were cut, metres.
    deepest_cut_m: f64,
    /// Dry points lower than the lower of h₀ and the sea.
    below_the_floor: u32,
    /// Dry points the floor holds at the sea level, flat.
    on_the_floor: u32,
    /// Points on the axis of a trunk into the sea the channels took from at or above it to below
    /// it less the trunk's bankfull depth.
    axis_below_sea: u32,
    /// Points on the axis of a trunk into a lake, its surface off the sea level, that the channels
    /// took from at or above the lake's surface to below it less the trunk's bankfull depth.
    axis_below_lake: u32,
    /// How far below their receiver's surface less the bankfull depth those points were cut,
    /// metres.
    deepest_axis_m: f64,
}

/// A group's point: its unit direction and, where it lies on the axis of a trunk into water, that
/// trunk's receiver's water surface and its bankfull depth, metres.
type Point = ([f64; 3], Option<(f64, f64)>);

/// A group's points, with the dry trunk segment's base level and bankfull depth where the points
/// are along a trunk's axis.
struct Group {
    label: &'static str,
    points: Vec<Point>,
}

/// The census and the checks on the world named `name` (see the module documentation).
#[expect(
    clippy::too_many_lines,
    reason = "one world's groups, checks and census, printed in the order the plan records them"
)]
fn check_the_channels_keep_out_of_the_water(name: &str) {
    let field = sample_world(name).field();
    let header = field.header();
    let synth = Synthesiser::new(field, DETAIL);
    let sea = header.sea_level().value();
    let mut network = synth.channels().network(field);
    let (mut mouths, mut lakes, mut wet, mut dry, mut into_water) = (
        BTreeSet::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    for cell in all_cells(field) {
        let record = field.cell(cell).expect("the field holds its cells");
        if record.is_under_water() {
            wet.push(cell);
            if (record.water_surface().value() - sea).abs() > 0.5 {
                lakes.push(cell);
            }
            continue;
        }
        dry.push(cell);
        if let Some(edge) = record.flow.edge() {
            let receiver = cell.edge_neighbour(edge);
            if field.cell(receiver).expect("held").is_under_water() {
                mouths.insert(receiver);
                if record.catchment_runoff != LogRunoff::NONE {
                    into_water.push(cell);
                }
            }
        }
    }
    let mouths: Vec<PatchKey> = mouths.into_iter().collect();
    println!(
        "\n{name}: sea {sea:.1} m; wet {}, lakes {}, mouths {}, dry {}, dry trunks into water {}",
        wet.len(),
        lakes.len(),
        mouths.len(),
        dry.len(),
        into_water.len()
    );
    let mut rng = Lcg::new(GROUPS);
    let cells_of = |label: &'static str, cells: Vec<PatchKey>| Group {
        label,
        points: cells
            .into_iter()
            .flat_map(points_of)
            .map(|dir| (dir, None))
            .collect(),
    };
    let mut groups = vec![
        cells_of("mouths", pick(&mut rng, &mouths, 200)),
        cells_of("lakes", pick(&mut rng, &lakes, 200)),
        cells_of("all wet", pick(&mut rng, &wet, 300)),
        cells_of("all dry", pick(&mut rng, &dry, 200)),
    ];
    // The axes of trunks into water: 16 points along each first chord, towards the crossing.
    let mut axes = Vec::new();
    for cell in pick(&mut rng, &into_water, 200) {
        let segment = network
            .segment(cell)
            .expect("held")
            .expect("a dry cell with a receiver and water has a trunk");
        let Outlet::Cell { receiver, .. } = segment.outlet() else {
            panic!("a trunk drains to a cell");
        };
        let base = field.cell(receiver).expect("held").water_surface().value();
        let (from, to) = (
            segment.from_m(),
            segment.crossing_m().expect("a trunk's crossing"),
        );
        for k in 0..16 {
            let f = (f64::from(k) + 0.5) / 16.0;
            let p = [0, 1, 2].map(|a| from[a] + f * (to[a] - from[a]));
            axes.push((unit_dir(p), Some((base, segment.bankfull_depth().value()))));
        }
    }
    groups.push(Group {
        label: "trunk axes into water",
        points: axes,
    });
    let without = WithoutChannels::of(field);
    let bare = Synthesiser::new(&without, DETAIL);
    let mut failures = Vec::new();
    for group in &groups {
        assert!(
            group.points.len() >= 3_200,
            "{name}: only {} points in {}",
            group.points.len(),
            group.label
        );
        for band in BANDS {
            let level = BandLevel::new(band).expect("a band level");
            let through = synth.level_set(level).channels_through();
            let (mut cache, mut bare_cache) = (SynthCache::new(), SynthCache::new());
            let mut tally = Tally::default();
            for &(dir, axis) in &group.points {
                let (_, record) = synth.cell_at(dir).expect("held");
                let h = synth
                    .height_at(&mut cache, dir, level)
                    .expect("held")
                    .height_m;
                let h0 = bare
                    .height_at(&mut bare_cache, dir, level)
                    .expect("held")
                    .height_m;
                let term = through.map(|t| {
                    synth
                        .channels()
                        .incision_at(field, dir, t, &mut cache)
                        .expect("held")
                });
                tally.points += 1;
                if record.is_under_water() {
                    tally.wet += 1;
                    let surface = record.water_surface().value();
                    if h > surface && h0 <= surface {
                        tally.turned_to_land += 1;
                    }
                    if let Some(s) = term
                        && [s.height_m, s.gradient[0], s.gradient[1], s.gradient[2]].map(bits)
                            != [0; 4]
                    {
                        tally.wet_term += 1;
                    }
                    continue;
                }
                let floor = sea - BELOW_SEA_M;
                if h < floor && h0 >= floor {
                    tally.cut_below_sea += 1;
                    tally.deepest_cut_m = tally.deepest_cut_m.max(sea - h);
                }
                // The synthesis's floor, to the bit: its h₀ is this one's sum.
                if h < h0.min(sea) {
                    tally.below_the_floor += 1;
                }
                if bits(h) == bits(sea) {
                    tally.on_the_floor += 1;
                }
                if let Some((base, bankfull)) = axis {
                    let least = base - bankfull;
                    if h < least && h0 >= base {
                        tally.deepest_axis_m = tally.deepest_axis_m.max(least - h);
                        if (base - sea).abs() > 0.5 {
                            tally.axis_below_lake += 1;
                        } else {
                            tally.axis_below_sea += 1;
                        }
                    }
                }
            }
            println!(
                "  {} band {band}: {} points, wet {}: turned to land by the channels {}, channel \
                 term not zero {}; dry points cut more than {BELOW_SEA_M} m below the sea {} \
                 (deepest {:.1} m below it), below the floor {}, on it {}; axis points cut below \
                 their base into the sea {}, into lakes {} (deepest {:.1} m below it)",
                group.label,
                tally.points,
                tally.wet,
                tally.turned_to_land,
                tally.wet_term,
                tally.cut_below_sea,
                tally.deepest_cut_m,
                tally.below_the_floor,
                tally.on_the_floor,
                tally.axis_below_sea,
                tally.axis_below_lake,
                tally.deepest_axis_m
            );
            if tally.turned_to_land
                + tally.wet_term
                + tally.cut_below_sea
                + tally.below_the_floor
                + tally.axis_below_sea
                > 0
            {
                failures.push(format!("{name} {} band {band}: {tally:?}", group.label));
            }
        }
    }
    print_the_split(name, &synth);
    print_the_budget(name, field, &synth);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The coarse pass's channel share against the synthesis's, printed: the mean over every
/// sixteenth cell, unweighted, of `channel_variance` (the `σ_h` budget's `C_c`, kept to the bit by
/// R09.T6.f) and of the synthesis's own closed form, every level counted whole.
fn print_the_budget(name: &str, field: &CoarseField, synth: &Synthesiser<'_, CoarseField>) {
    let body = ChannelBody::of(field.header());
    let whole = Metres::new(1e7);
    let (mut budget, mut drawn, mut wet_budget, mut cells) = (0.0, 0.0, 0.0, 0_u32);
    for cell in all_cells(field).step_by(16) {
        let record = field.cell(cell).expect("held");
        let booked = channel_variance(record, body, whole).value();
        budget += booked;
        if record.is_under_water() {
            wet_budget += booked;
        }
        drawn += synth
            .unresolved_variance(cell, whole)
            .expect("held")
            .channels()
            .value();
        cells += 1;
    }
    let n = f64::from(cells);
    println!(
        "  {name}: the budget's channel share {:.1} m² (of it wet cells' {:.3} m²) against the \
         synthesis's {:.1} m², {:.2}% more, over {cells} cells",
        budget / n,
        wet_budget / n,
        drawn / n,
        100.0 * (budget / drawn - 1.0)
    );
}

/// Probe C's split, printed: at band levels 3 and 4, the variance of z, the ground the level
/// leaves out over its closed forms' RMS, over 600 random points, for wet and dry cells.
fn print_the_split(name: &str, synth: &Synthesiser<'_, CoarseField>) {
    let finest = BandLevel::new(19).expect("a band level");
    let mut rng = Lcg::new(SPLIT);
    for band in [3_u8, 4] {
        let level = BandLevel::new(band).expect("a band level");
        let (mut wet, mut dry) = ((0.0, 0_u32), (0.0, 0_u32));
        for _ in 0..600 {
            let face = Face::ALL[usize::try_from(rng.next_below(6)).expect("a face")];
            let dir = unit_dir(face_uv_to_xyz(FaceUv {
                face,
                u: st_to_uv(rng.next_f64()),
                v: st_to_uv(rng.next_f64()),
            }));
            let mut cache = SynthCache::new();
            let (cell, record) = synth.cell_at(dir).expect("held");
            let omitted = synth.omitted_variance(cell, level).expect("held");
            let fine = synth
                .height_at(&mut cache, dir, finest)
                .expect("held")
                .height_m;
            let at = synth
                .height_at(&mut cache, dir, level)
                .expect("held")
                .height_m;
            let z = (fine - at) / omitted.total().value().sqrt();
            let side = if record.is_under_water() {
                &mut wet
            } else {
                &mut dry
            };
            side.0 += z * z;
            side.1 += 1;
        }
        println!(
            "  {name} band {band}: variance of z, wet {:.2} over {} points, dry {:.2} over {}",
            wet.0 / f64::from(wet.1.max(1)),
            wet.1,
            dry.0 / f64::from(dry.1.max(1)),
            dry.1
        );
    }
}

#[test]
#[ignore = "slow: the reference Earth's coarse pass at level 8 and 16,000 points at three band \
            levels"]
fn the_reference_earth_s_channels_keep_out_of_the_water() {
    check_the_channels_keep_out_of_the_water("earth_like");
}

#[test]
#[ignore = "slow: the locked Earth's coarse pass at level 8 and 16,000 points at three band levels"]
fn the_locked_earth_s_channels_keep_out_of_the_water() {
    check_the_channels_keep_out_of_the_water("locked_earth_like");
}
