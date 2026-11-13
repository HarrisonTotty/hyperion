//! The trunks' depth and the playas' floor (plan R09, T6.g; decision-r09-channel-term §9.2 and
//! §9.4) on the synthetic Earth-like world: its drawn steady-state trunk valleys meet the census
//! target, and no trunk into a playa's pit is cut below the pit's ground, the floor continuous
//! across the pits' and their donors' edges. The reference Earths' census is the sim's
//! `channel_water_worlds` test.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::field::FlowDirection;
use crate::synth::{BandLevel, Synthesiser};

/// §9.2's target for the median of the drawn steady-state trunk valleys, metres: within a factor of
/// [`TARGET_FACTOR`] of 250 m, Earth2014's 5′ census calibrated on four canyons.
const TARGET_MEDIAN_M: f64 = 250.0;

/// The factor the median may lie within of [`TARGET_MEDIAN_M`].
const TARGET_FACTOR: f64 = 2.0;

/// §9.2's target for their p90, metres: at most about 2 km.
const TARGET_P90_M: f64 = 2_000.0;

/// The census of a field's drawn steady-state trunk valleys (R09.T6.g (a)).
#[derive(Debug)]
struct Census {
    /// Dry cells with a trunk.
    trunks: usize,
    /// Of them, where the cell stands above its base level.
    above: usize,
    /// The groove's depth G at the axis: its median, p90 and largest, metres.
    median_m: f64,
    p90_m: f64,
    largest_m: f64,
    /// The median of G over the cell's height above its base level, where it stands above it.
    ratio_median: f64,
    /// The share of those whose G passes that height, where the bankfull channel sets G.
    ratio_above_one: f64,
    /// The share of trunks whose valley the hold binds.
    held: f64,
}

/// The `q` quantile of `sorted`, the nearest rank's.
fn quantile(sorted: &[f64], q: f64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a rank of a list of cells, exact in f64 and back"
    )]
    let rank = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[rank]
}

/// [`Census`] of `field`'s dry trunks.
fn census(field: &CoarseField) -> Census {
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let body = channels.body();
    let cap = groove_cap_m(body.radius.value(), first);
    let mut network = channels.network(field);
    let (mut depths, mut ratios, mut held) = (Vec::new(), Vec::new(), 0);
    for c in cells_of(field) {
        let record = field.cell(c).unwrap();
        if record.is_under_water() {
            continue;
        }
        let Some(trunk) = network.segment(c).unwrap() else {
            continue;
        };
        let Outlet::Cell { receiver, .. } = trunk.outlet() else {
            panic!("a trunk drains to a cell");
        };
        let receiver = field.cell(receiver).unwrap();
        let valley = trunk.below_top(0.5).value();
        let g = trunk_depth_m(valley, trunk.bankfull_depth().value(), cap);
        let unheld = tributary_steepness_of(record)
            .map_or(0.0, |k| valley_depth(k, body.radius, first).value());
        held += usize::from(valley < unheld);
        let room = record.water_surface().value() - base_level_m(receiver, header.sea_level());
        if room > 0.0 {
            ratios.push(g / room);
        }
        depths.push(g);
    }
    depths.sort_by(f64::total_cmp);
    ratios.sort_by(f64::total_cmp);
    let above_one = ratios.iter().filter(|&&r| r > 1.0).count();
    #[expect(clippy::cast_precision_loss, reason = "counts of cells, exact in f64")]
    let (held, ratio_above_one) = (
        held as f64 / depths.len() as f64,
        above_one as f64 / ratios.len() as f64,
    );
    Census {
        trunks: depths.len(),
        above: ratios.len(),
        median_m: quantile(&depths, 0.5),
        p90_m: quantile(&depths, 0.9),
        largest_m: *depths.last().unwrap(),
        ratio_median: quantile(&ratios, 0.5),
        ratio_above_one,
        held,
    }
}

/// Every coarse cell of `field`, in cell-index order.
fn cells_of(field: &CoarseField) -> impl Iterator<Item = PatchKey> + '_ {
    let level = field.header().level();
    (0..level.cell_count()).map(move |n| cell_at_index(level.get(), n).unwrap())
}

/// (a): the synthetic Earth's drawn steady-state trunk valleys have their median within a factor
/// of two of 250 m, their p90 at most 2 km, and the median of each over its cell's height above
/// its base level under 1 (decision-r09-channel-term §9.2's target), printed for the record.
#[test]
fn the_trunk_valleys_meet_the_census_target() {
    let census = census(earth());
    println!(
        "synthetic Earth: {} dry trunks ({} above their base level): G median {:.1} m, p90 {:.1} \
         m, largest {:.1} m; over the height above the base level, median {:.3}, {:.4} above 1; \
         the hold binds at {:.3}",
        census.trunks,
        census.above,
        census.median_m,
        census.p90_m,
        census.largest_m,
        census.ratio_median,
        census.ratio_above_one,
        census.held
    );
    assert!(
        census.trunks > 10_000 && census.above > 10_000,
        "{census:?}"
    );
    assert!(
        census.median_m >= TARGET_MEDIAN_M / TARGET_FACTOR
            && census.median_m <= TARGET_MEDIAN_M * TARGET_FACTOR,
        "{census:?}"
    );
    assert!(census.p90_m <= TARGET_P90_M, "{census:?}");
    assert!(census.ratio_median < 1.0, "{census:?}");
}

/// A view of `inner` with every coarse cell's flow terminal, so that the synthesis over it draws
/// no channel and nothing else it reads changes: the field's heights without the channels, summed
/// in the same order.
struct Bare<'a> {
    inner: &'a CoarseField,
    cells: Vec<SynthesisCell>,
}

impl<'a> Bare<'a> {
    fn of(inner: &'a CoarseField) -> Self {
        let cells = cells_of(inner)
            .map(|c| SynthesisCell {
                flow: FlowDirection::Terminal,
                ..*inner.cell(c).unwrap()
            })
            .collect();
        Self { inner, cells }
    }
}

impl FieldView for Bare<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.inner.cell(cell)?;
        self.cells
            .get(usize::try_from(crate::field::cell_index(cell)).unwrap())
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.inner.craters_reaching(cell)
    }
}

/// The unit direction whose point on `figure` is `point` (metres): M⁻¹ p, normalised, the inverse
/// of [`Spheroid::point`](crate::spheroid::Spheroid::point).
fn direction_of(figure: crate::spheroid::Spheroid, point: [f64; 3]) -> [f64; 3] {
    unit_dir([
        point[0] / figure.equatorial_radius_m,
        point[1] / figure.equatorial_radius_m,
        point[2] / figure.polar_radius_m,
    ])
}

/// The dry, terminal cells of `field` above its sea level, playas' pits, and each with its dry
/// donors, the edge neighbours whose flow names it, in cell-index order.
fn pits(field: &CoarseField) -> Vec<(PatchKey, Vec<PatchKey>)> {
    let sea = field.header().sea_level().value();
    cells_of(field)
        .filter(|&c| {
            let r = field.cell(c).unwrap();
            !r.is_under_water() && r.flow.edge().is_none() && r.water_surface().value() > sea
        })
        .map(|p| {
            let donors = Edge::ALL
                .into_iter()
                .filter_map(|e| {
                    let (n, back) = p.edge_neighbour_and_back(e);
                    let d = field.cell(n).unwrap();
                    (!d.is_under_water() && d.flow.edge() == Some(back)).then_some(n)
                })
                .collect();
            (p, donors)
        })
        .collect()
}

/// (d): no point on the axis of a trunk into a playa's pit above the sea, nor of the pit's own
/// cell, is cut by the channels from at or above the pit's ground to below it, at band levels 5, 7
/// and 19 (decision-r09-channel-term §9.4's lean), and the floor holds some of them there.
#[test]
fn no_playa_s_trunk_is_cut_below_its_basin_s_floor() {
    let field = earth();
    let synth = Synthesiser::new(field, SEED);
    let bare = Bare::of(field);
    let without = Synthesiser::new(&bare, SEED);
    let mut network = synth.channels().network(field);
    let figure = field.header().figure();
    let (mut checked, mut floored, mut trunks) = (0, 0, 0);
    for (pit, donors) in pits(field) {
        let ground = field.cell(pit).unwrap().water_surface().value();
        let mut dirs = points_in(pit, 4);
        for &d in &donors {
            let Some(trunk) = network.segment(d).unwrap() else {
                continue;
            };
            trunks += 1;
            dirs.extend((0..=16).map(|k| direction_of(figure, trunk.point_m(f64::from(k) / 16.0))));
        }
        for band in [5, 7, 19] {
            let level = BandLevel::new(band).unwrap();
            let mut cache = SynthCache::new();
            let mut bare_cache = SynthCache::new();
            for &dir in &dirs {
                let h = synth.height_at(&mut cache, dir, level).unwrap().height_m;
                let h0 = without
                    .height_at(&mut bare_cache, dir, level)
                    .unwrap()
                    .height_m;
                if h0 >= ground {
                    assert!(
                        h >= ground - 1e-6,
                        "{pit:?} band {band} at {dir:?}: cut to {h} m below the pit's {ground} m"
                    );
                    floored += usize::from((h - ground).abs() <= 1e-6 && h0 > ground);
                    checked += 1;
                }
            }
        }
    }
    println!("the playas: {trunks} trunks into pits, {checked} points checked, {floored} floored");
    // The floor outside the basins: the 4 × 4 grids of the pits' neighbours that neither are a
    // basin's cell nor drain into one, where the floor's corners rise to a pit's ground.
    let basin: std::collections::BTreeSet<PatchKey> = pits(field)
        .into_iter()
        .flat_map(|(p, donors)| std::iter::once(p).chain(donors))
        .collect();
    let finest = BandLevel::new(19).unwrap();
    let (mut outside, mut lifted) = (0, 0);
    let mut cache = SynthCache::new();
    for &p in &basin {
        let around = Edge::ALL
            .into_iter()
            .map(|e| p.edge_neighbour(e))
            .chain(p.corner_neighbours().into_iter().flatten());
        for n in around.filter(|n| !basin.contains(n)) {
            if field.cell(n).unwrap().is_under_water() {
                continue;
            }
            for dir in points_in(n, 4) {
                let parts = synth.contributions(&mut cache, dir, finest).unwrap();
                let summed = parts.base.height_m
                    + parts.relief.height_m
                    + parts.channels.height_m
                    + parts.craters.height_m
                    + parts.rims.height_m;
                let h = synth.height_at(&mut cache, dir, finest).unwrap().height_m;
                let sea = field.header().sea_level().value();
                outside += 1;
                lifted += usize::from(h > summed && h > sea);
            }
        }
    }
    println!(
        "the floor outside the basins lifts {lifted} of {outside} points of their neighbours at \
         band level 19"
    );
    assert!(
        trunks > 50 && checked > 1_000 && floored > 0,
        "{trunks}, {checked}, {floored}"
    );
}

/// (d): across the edges of every third pit and donor, at most 90 cells, on three lines each way
/// through each cell, the finest height steps by less than a centimetre between points 10⁻⁷ ÷ 2⁸
/// of a face apart: the floor is continuous.
#[test]
fn the_playas_floor_is_continuous() {
    let field = earth();
    let synth = Synthesiser::new(field, SEED);
    let finest = BandLevel::new(19).unwrap();
    let mut cache = SynthCache::new();
    let eps = 1e-7 / f64::from(1_u32 << 8);
    let (mut worst, mut pairs) = (0.0_f64, 0);
    let cells: Vec<PatchKey> = pits(field)
        .into_iter()
        .flat_map(|(p, donors)| std::iter::once(p).chain(donors))
        .step_by(3)
        .take(90)
        .collect();
    for c in cells {
        for k in 0..=8_u32 {
            let x = f64::from(k) / 8.0;
            for y in [0.137, 0.52, 0.861] {
                for (a, b) in [
                    (at(c, x - eps, y), at(c, x + eps, y)),
                    (at(c, y, x - eps), at(c, y, x + eps)),
                ] {
                    let ha = synth.height_at(&mut cache, a, finest).unwrap().height_m;
                    let hb = synth.height_at(&mut cache, b, finest).unwrap().height_m;
                    worst = worst.max((ha - hb).abs());
                    pairs += 1;
                }
            }
        }
    }
    assert!(pairs > 1_000, "{pairs}");
    assert!(worst < 0.01, "a step of {worst} m across an edge");
}

/// (c): the trunk level's closed form counts the trunks that run in a cell, its own half and its
/// donors': over 200 dry cells of the synthetic Earth, its realised variance on a 16 × 16 grid
/// over the closed form's has its median within 0.8 to 1.25 and its p10 and p90 within 0.4 to 2,
/// where R09.T6.b's cell-alone form, the trunk counted whole in its own cell, does not (median
/// 0.57, p10 0.03 on 400 cells at 24 × 24, as built). From 26 the trunks are bent and graded
/// (R09.T6.i): p10 0.428, median 0.973 and p90 1.785 over 196 cells, measured, against T6.h's
/// p10 of at least 0.5.
#[test]
fn the_trunk_level_s_closed_form_follows_each_cell_s_trunks() {
    let field = earth();
    let header = field.header();
    let first = header.level().get();
    let channels = Channels::new(header, SEED);
    let mut cache = SynthCache::new();
    let dry: Vec<PatchKey> = cells_of(field)
        .filter(|&c| !field.cell(c).unwrap().is_under_water())
        .collect();
    let side = 16;
    let mut ratios = Vec::new();
    for &cell in dry.iter().step_by(dry.len() / 200) {
        let (mut sum, mut square, mut weights) = (0.0, 0.0, 0.0);
        for row in 0..side {
            for column in 0..side {
                let x = (f64::from(column) + 0.5) / f64::from(side);
                let y = (f64::from(row) + 0.5) / f64::from(side);
                let weight = area_weight(cell, x, y);
                let levels = channels
                    .incision(field, &mut cache)
                    .levels(at(cell, x, y), first)
                    .unwrap();
                let value = levels.as_slice().first().map_or(0.0, |l| l.value);
                sum += weight * value;
                square += weight * value * value;
                weights += weight;
            }
        }
        let realised = square / weights - (sum / weights) * (sum / weights);
        let (_, terms) = synthesised_terms(field, cell).unwrap();
        let closed = terms
            .iter()
            .find(|t| t.level == first)
            .map_or(0.0, |t| t.variance);
        if closed > 1.0 {
            ratios.push(realised / closed);
        }
    }
    ratios.sort_by(f64::total_cmp);
    let (p10, median, p90) = (
        quantile(&ratios, 0.1),
        quantile(&ratios, 0.5),
        quantile(&ratios, 0.9),
    );
    println!(
        "the trunk level, realised over closed, over {} cells: p10 {p10:.3}, median {median:.3}, \
         p90 {p90:.3}",
        ratios.len()
    );
    assert!(ratios.len() > 150, "{}", ratios.len());
    assert!((0.8..=1.25).contains(&median), "{median}");
    assert!(p10 >= 0.4 && p90 <= 2.0, "{p10}, {p90}");
}
