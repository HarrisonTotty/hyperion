//! Writes hillshades of the reference Mars's synthesis about three of its trunk cells (plan R09,
//! R09.T6.c's look at its trunks' two straight chords a cell; signoff-r09-t6c-brainstorm, "For
//! main" 3), as binary PGMs (`P5`) with no dependency beyond the crate's own, for a person to look
//! at, as R09.T15's `surface_map` does the reference Earth's coarse field.
//!
//! ```text
//! cargo run -p hyperion-sim --features testing --example trunk_map [-- <directory>]
//! ```
//!
//! The images go to `<directory>`, by default the workspace's `target/trunk_map/`. It runs the
//! reference Mars's coarse pass (level 7, cells of about 38 km) and ranks the cells whose trunk
//! cuts its wet epoch's valley ([`epoch_valley`] of depth above zero) by drainage area; the three
//! it draws are the median's, the 90th percentile's and the largest's.
//!
//! - **`trunk-1.pgm` to `trunk-3.pgm`:** three mean coarse cells a side about each cell's centre,
//!   at [`FINE_PIXEL_M`] a pixel.
//! - **`first-view.pgm`:** nine mean coarse cells a side about the largest's, at
//!   [`FIRST_VIEW_PIXEL_M`] a pixel, the scale at which the scripted descent first shows the
//!   ground.
//!
//! Each image is a gnomonic map about its centre, north up and east right, at true scale. Each
//! pixel is the synthesis's height ([`Synthesiser::height_at`]) at [`BAND`] at the pixel's
//! centre, shaded from the central differences of its neighbours' heights by a sun
//! [`SUN_ELEVATION_DEG`] high at the azimuth [`SUN_AZIMUTH_DEG`], so that the shading is the
//! relief a mesh of the pixels' spacing draws. It prints each cell's valley, and the trunks' runs
//! and bends over every drawn valley: the straight run from a key point to its receiver's, and
//! the turn at a key point between an inflowing trunk's last chord and the outflow's first, its
//! largest inflow's (the course's own bend) apart from the others' (their junctions).
//!
//! It takes about three CPU-minutes in the dev profile, nearly all of it the synthesis, spread
//! over the machine's threads (20 s on a pod's 48).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::thread;

use hyperion_base::rng::DetailSeed;
use hyperion_sim::math;
use hyperion_sim::planetary::surface::{coarse_pass, reference};
use hyperion_surface::cube::{PATCH_QUADS, unit_dir};
use hyperion_surface::field::{CoarseField, FieldView, cell_at_index};
use hyperion_surface::synth::channels::{ChannelBody, Outlet, epoch_valley};
use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};

/// The detail seed the synthesis draws with: arbitrary, as the reference worlds' tests' are.
const DETAIL: DetailSeed = DetailSeed::new(0x7472_756e_6b6d_6170);

/// The band level the heights are taken at: 12, whose patches' vertices are about 20 m apart on
/// the reference Mars, so that every contribution the trunks' frames resolve is in.
const BAND: BandLevel = match BandLevel::new(12) {
    Ok(band) => band,
    Err(_) => panic!("12 is at most MAX_LEVEL, 24"),
};

/// The trunk frames' pixel, m: about 150 m (the look's scale), which the descent's main view
/// (below) reaches across the track at its frame's foot about 190 km up in its descent arc, and
/// at its centre about 120 km up.
const FINE_PIXEL_M: f64 = 150.0;

/// The first view's pixel, m: the scripted descent's first view of the ground (R05's
/// `descentProfile.ts`, its 400 km orbit coast, pitched 30° down, through R02's 60° horizontal
/// field of view across the 1,920 pixels of the main view at 1080p, a pinhole's focal length of
/// 1,663 pixels) puts 630 m a pixel across the track at the frame's centre on the reference Mars,
/// 1,047 km off, and 324 m at its foot, 567 km off; 600 m, rounded.
const FIRST_VIEW_PIXEL_M: f64 = 600.0;

/// The trunk frames' side in mean coarse cells: their 3 × 3.
const FINE_CELLS: f64 = 3.0;

/// The first view's side in mean coarse cells.
const FIRST_VIEW_CELLS: f64 = 9.0;

/// The sun's azimuth, degrees clockwise from north: the north-west, the cartographic convention
/// (GDAL's `gdaldem hillshade` default, "the value generally used to generate shaded maps", its
/// `doc/source/programs/gdaldem.rst`).
const SUN_AZIMUTH_DEG: f64 = 315.0;

/// The sun's elevation, degrees: GDAL's `gdaldem hillshade` default (`alt = 45.0` in its
/// `apps/gdaldem_lib.cpp`).
const SUN_ELEVATION_DEG: f64 = 45.0;

/// A cell whose trunk cuts a valley.
#[derive(Debug, Clone, Copy)]
struct Valley {
    /// Its drainage area, m².
    area_m2: f64,
    /// Its cell's index in the field.
    index: u32,
}

/// A frame: its file's name, its centre's unit direction (body-fixed), its pixel in metres and
/// its side in pixels.
struct Frame {
    name: String,
    centre: [f64; 3],
    pixel_m: f64,
    side: u32,
}

fn main() -> io::Result<()> {
    let directory = std::env::args_os().nth(1).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/trunk_map"),
        PathBuf::from,
    );
    fs::create_dir_all(&directory)?;
    let field = coarse_pass(reference::SEED, &reference::mars_like());
    let header = field.header();
    let level = header.level().get();
    let radius_m = header.radius().value();
    let body = ChannelBody::of(header);
    let synth = Synthesiser::new(&field, DETAIL);
    let cells = field.synthesis();
    let mut valleys: Vec<Valley> = (0_u32..)
        .zip(cells)
        .filter(|(_, cell)| !cell.is_under_water())
        .filter_map(|(index, cell)| {
            let valley = epoch_valley(cell, body)?;
            (valley.depth().value() > 0.0).then(|| Valley {
                area_m2: cell.drainage.area().value(),
                index,
            })
        })
        .collect();
    valleys.sort_by(|a, b| a.area_m2.total_cmp(&b.area_m2).then(a.index.cmp(&b.index)));
    let count = valleys.len();
    assert!(count > 0, "the reference Mars draws valleys");
    println!("{count} trunk cells cut a valley at level {level}");
    print_runs_and_bends(&synth, &valleys);
    let cell_m = (4.0 * core::f64::consts::PI * radius_m * radius_m
        / f64::from(header.level().cell_count()))
    .sqrt();
    let side = |cells: f64, pixel_m: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a side of some hundreds of pixels, rounded"
        )]
        let px = (cells * cell_m / pixel_m).round() as u32;
        px
    };
    let mut frames = Vec::new();
    for (k, rank) in [count / 2, count * 9 / 10, count - 1]
        .into_iter()
        .enumerate()
    {
        let index = valleys[rank].index;
        let cell = cell_at_index(level, index).expect("a field's cell");
        let record = &cells[usize::try_from(index).expect("a cell index fits a usize")];
        let valley = epoch_valley(record, body).expect("a ranked cell cuts a valley");
        let centre = cell.vertex_dir(half(), half());
        let (lat, lon) = (
            math::asin(centre[2]).to_degrees(),
            math::atan2(centre[1], centre[0]).to_degrees(),
        );
        println!(
            "trunk-{}: face {} ({}, {}), {lat:.2}° {lon:.2}°, drainage {:.0} km², valley {:.1} m \
             deep, floor {:.1} m, each wall {:.0} m across, top {:.0} m",
            k + 1,
            cell.face().index(),
            cell.i(),
            cell.j(),
            record.drainage.area().value() / 1e6,
            valley.depth().value(),
            valley.floor_width().value(),
            valley.wall().value(),
            valley.floor_width().value() + 2.0 * valley.wall().value(),
        );
        frames.push(Frame {
            name: format!("trunk-{}", k + 1),
            centre,
            pixel_m: FINE_PIXEL_M,
            side: side(FINE_CELLS, FINE_PIXEL_M),
        });
        if rank == count - 1 {
            frames.push(Frame {
                name: "first-view".to_owned(),
                centre,
                pixel_m: FIRST_VIEW_PIXEL_M,
                side: side(FIRST_VIEW_CELLS, FIRST_VIEW_PIXEL_M),
            });
        }
    }
    for frame in &frames {
        let heights = heights(&synth, frame, radius_m, BAND);
        let path = directory.join(format!("{}.pgm", frame.name));
        write_pgm(
            &path,
            frame.side,
            &hillshade(&heights, frame.side, frame.pixel_m),
        )?;
        println!(
            "wrote {} ({} × {} at {} m a pixel, band level {})",
            path.display(),
            frame.side,
            frame.side,
            frame.pixel_m,
            BAND.get()
        );
    }
    Ok(())
}

/// A patch's middle vertex along a side: its centre's.
#[must_use]
fn half() -> u8 {
    u8::try_from(PATCH_QUADS / 2).expect("a patch's 64 quads a side fit a u8")
}

/// Prints the trunks' straight runs, key point to receiver's key point, and their bends at each
/// key point between an inflowing trunk's last chord and the outflow's first, over `valleys`: the
/// largest inflow's (by drainage, then index), the course's own bend, apart from the other
/// inflows', their junctions.
fn print_runs_and_bends(synth: &Synthesiser<'_, CoarseField>, valleys: &[Valley]) {
    let field = synth.field();
    let level = field.header().level().get();
    let mut network = synth.channels().network(field);
    let mut drawn = BTreeMap::new();
    for &valley in valleys {
        let cell = cell_at_index(level, valley.index).expect("a field's cell");
        if let Some(segment) = network.segment(cell).expect("the field holds its cells") {
            drawn.insert(cell.to_u64(), (valley, segment));
        }
    }
    let mut runs_km: Vec<f64> = drawn
        .values()
        .map(|(_, s)| distance(s.from_m(), s.to_m()) / 1e3)
        .collect();
    // Each receiver's inflows: their drainage, index and bend, degrees.
    let mut inflows: BTreeMap<u64, Vec<(f64, u32, f64)>> = BTreeMap::new();
    for (valley, segment) in drawn.values() {
        let receiver = match segment.outlet() {
            Outlet::Cell { receiver, .. } => receiver,
            Outlet::Join { .. } => unreachable!("a coarse cell's segment is a trunk"),
        };
        let Some((_, outflow)) = drawn.get(&receiver.to_u64()) else {
            continue;
        };
        let trunk = "a coarse cell's segment is a trunk, which has a crossing";
        let inward = direction(segment.crossing_m().expect(trunk), segment.to_m());
        let onward = direction(outflow.from_m(), outflow.crossing_m().expect(trunk));
        let cosine = (0..3).map(|a| inward[a] * onward[a]).sum::<f64>();
        let bend_deg = math::acos(cosine.clamp(-1.0, 1.0)).to_degrees();
        inflows.entry(receiver.to_u64()).or_default().push((
            valley.area_m2,
            valley.index,
            bend_deg,
        ));
    }
    let (mut courses_deg, mut junctions_deg) = (Vec::new(), Vec::new());
    for mut each in inflows.into_values() {
        each.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if let Some((_, _, bend_deg)) = each.pop() {
            courses_deg.push(bend_deg);
        }
        junctions_deg.extend(each.into_iter().map(|(_, _, bend_deg)| bend_deg));
    }
    print_quantiles("runs", &mut runs_km, "km");
    print_quantiles("bends of the courses", &mut courses_deg, "°");
    print_quantiles("bends at the junctions", &mut junctions_deg, "°");
}

/// Prints `values`' count, p10, median, p90 and maximum, in `unit`, as `label`'s.
fn print_quantiles(label: &str, values: &mut [f64], unit: &str) {
    values.sort_by(f64::total_cmp);
    let Some(last) = values.len().checked_sub(1) else {
        println!("{label}: none");
        return;
    };
    let at = |q: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a rank among some thousands of values, rounded"
        )]
        let rank = (last as f64 * q).round() as usize;
        values[rank]
    };
    println!(
        "{label} ({}): p10 {:.1}, median {:.1}, p90 {:.1}, max {:.1} {unit}",
        values.len(),
        at(0.1),
        at(0.5),
        at(0.9),
        at(1.0)
    );
}

/// The distance between points `a` and `b`, metres.
#[must_use]
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3)
        .map(|k| (b[k] - a[k]) * (b[k] - a[k]))
        .sum::<f64>()
        .sqrt()
}

/// The unit vector from `a` towards `b`.
#[must_use]
fn direction(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let d = distance(a, b);
    core::array::from_fn(|k| (b[k] - a[k]) / d)
}

/// The heights of `frame`'s pixels at `band`, metres, in row-major order from its north-west
/// corner, on a sphere of `radius_m` metres about the frame's centre, over the machine's threads,
/// each pixel's the same whatever their number.
#[must_use]
fn heights(
    synth: &Synthesiser<'_, CoarseField>,
    frame: &Frame,
    radius_m: f64,
    band: BandLevel,
) -> Vec<f64> {
    let c = frame.centre;
    let east = direction([0.0; 3], [-c[1], c[0], 0.0]);
    let north = [
        c[1] * east[2] - c[2] * east[1],
        c[2] * east[0] - c[0] * east[2],
        c[0] * east[1] - c[1] * east[0],
    ];
    let side = frame.side;
    let width = usize::try_from(side).expect("a side of some hundreds of pixels fits a usize");
    let offset = |k: u32| (f64::from(k) + 0.5 - f64::from(side) / 2.0) * frame.pixel_m;
    let rows: Vec<u32> = (0..side).collect();
    let threads = thread::available_parallelism().map_or(1, usize::from);
    let chunk = rows.len().div_ceil(threads);
    thread::scope(|scope| {
        let workers: Vec<_> = rows
            .chunks(chunk)
            .map(|rows| {
                scope.spawn(move || {
                    let mut cache = SynthCache::new();
                    let mut out = Vec::with_capacity(rows.len() * width);
                    for &row in rows {
                        let north_m = -offset(row);
                        for column in 0..side {
                            let east_m = offset(column);
                            let point = core::array::from_fn(|k| {
                                c[k] * radius_m + east[k] * east_m + north[k] * north_m
                            });
                            let sample = synth
                                .height_at(&mut cache, unit_dir(point), band)
                                .expect("the field holds every cell");
                            out.push(sample.height_m);
                        }
                    }
                    out
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| {
                w.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// The grey levels of a hillshade of `heights` (row-major, `side` a side, from the north-west
/// corner) at `pixel_m` metres a pixel: the sun's cosine on the surface's normal from the central
/// differences of each pixel's neighbours (one-sided at the edges), 255 where the sun lies along
/// the normal and level ground 180 (the sine of the sun's 45°).
#[must_use]
fn hillshade(heights: &[f64], side: u32, pixel_m: f64) -> Vec<u8> {
    let n = usize::try_from(side).expect("a side of some hundreds of pixels fits a usize");
    let (azimuth, elevation) = (SUN_AZIMUTH_DEG.to_radians(), SUN_ELEVATION_DEG.to_radians());
    let sun = [
        math::sin(azimuth) * math::cos(elevation),
        math::cos(azimuth) * math::cos(elevation),
        math::sin(elevation),
    ];
    let at = |row: usize, column: usize| heights[row * n + column];
    let mut grey = Vec::with_capacity(n * n);
    for row in 0..n {
        for column in 0..n {
            let (w, e) = (column.saturating_sub(1), (column + 1).min(n - 1));
            let (north, south) = (row.saturating_sub(1), (row + 1).min(n - 1));
            #[expect(
                clippy::cast_precision_loss,
                reason = "a difference of at most two pixels"
            )]
            let (dx, dy) = ((e - w) as f64 * pixel_m, (south - north) as f64 * pixel_m);
            let slope_east = (at(row, e) - at(row, w)) / dx;
            let slope_north = (at(north, column) - at(south, column)) / dy;
            let norm = (slope_east * slope_east + slope_north * slope_north + 1.0).sqrt();
            let cosine = (-slope_east * sun[0] - slope_north * sun[1] + sun[2]) / norm;
            grey.push(channel(255.0 * cosine));
        }
    }
    grey
}

/// Writes `grey` as a binary PGM at `path`, `side` pixels a side.
fn write_pgm(path: &Path, side: u32, grey: &[u8]) -> io::Result<()> {
    let mut out = BufWriter::new(fs::File::create(path)?);
    write!(out, "P5\n{side} {side}\n255\n")?;
    out.write_all(grey)?;
    out.flush()
}

/// A grey level of `value`, rounded and held to 0–255.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and held to 0–255 first, which a u8 holds exactly"
)]
#[must_use]
fn channel(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}
