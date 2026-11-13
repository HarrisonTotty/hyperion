//! Writes the reference Earth's coarse field as three equirectangular images (plan R09, R09.T15):
//! its elevation, its classes and its flow, as binary PPMs (`P6`) with no dependency beyond the
//! crate's own, for a person to look at (plan 09's Verification, "By eye").
//!
//! ```text
//! cargo run -p hyperion-sim --features testing --example surface_map [-- <directory>]
//! ```
//!
//! The images go to `<directory>`, by default the workspace's `target/surface_map/`:
//! `elevation.ppm`, `class.ppm` and `flow.ppm`, each 1,440 × 720 pixels, longitude −180° to 180°
//! left to right and latitude 90° to −90° top to bottom, each pixel the field cell its centre's
//! direction falls in (the body-fixed frame: +z the rotation pole, +x the prime meridian).
//!
//! - **Elevation:** the sea's depth in blues, darkest deepest, a lake lighter; the land green,
//!   through tan and brown, to white at 6 km and above, each by its height above the sea level.
//! - **Class:** a Köppen–Geiger class in Beck et al. 2018's legend colours (`legend.txt` of
//!   their data release, Sci. Data 5, 180214), a surface-state form in its own colour: the sea
//!   navy, a lake turquoise (apart from every legend colour), a frozen sea a pale blue, ice white,
//!   frost a pale grey-blue, melt orange, organic sediment brown, regolith and rock greys; an
//!   unclassified cell black.
//! - **Flow:** the land dark and the sea black; each cell that drains more than 10⁴ km² brighter
//!   with the logarithm of its drainage area, from blue to white at 10⁶ km² and above, so that the
//!   trunks read as rivers running to the sea.
//!
//! It runs the whole coarse pass at level 8 (393,216 cells), about a minute in the dev profile.

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use hyperion_sim::math;
use hyperion_sim::planetary::surface::{coarse_pass, reference};
use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::{CoarseField, FieldView, SurfaceClass, SynthesisCell, cell_index};

/// The images' width, pixels: a quarter-degree in longitude.
const WIDTH: u32 = 1440;

/// The images' height, pixels: a quarter-degree in latitude.
const HEIGHT: u32 = 720;

/// The height at and above which the land is drawn white, m.
const SUMMIT_M: f64 = 6_000.0;

/// The depth at and below which the sea is drawn darkest, m.
const ABYSS_M: f64 = 6_000.0;

/// The drainage area above which a cell is drawn as a river, m²: 10⁴ km².
const RIVER_AREA_M2: f64 = 1e10;

/// The drainage area at and above which a river is drawn white, m²: 10⁶ km².
const TRUNK_AREA_M2: f64 = 1e12;

/// An RGB colour.
type Rgb = [u8; 3];

/// Beck et al. 2018's legend colours of Köppen–Geiger's codes 1 to 30, in code order
/// (`legend.txt`).
const KOPPEN_GEIGER: [Rgb; 30] = [
    [0, 0, 255],
    [0, 120, 255],
    [70, 170, 250],
    [255, 0, 0],
    [255, 150, 150],
    [245, 165, 0],
    [255, 220, 100],
    [255, 255, 0],
    [200, 200, 0],
    [150, 150, 0],
    [150, 255, 150],
    [100, 200, 100],
    [50, 150, 50],
    [200, 255, 80],
    [100, 255, 80],
    [50, 200, 0],
    [255, 0, 255],
    [200, 0, 200],
    [150, 50, 150],
    [150, 100, 150],
    [170, 175, 255],
    [90, 120, 220],
    [75, 80, 180],
    [50, 0, 135],
    [0, 255, 255],
    [55, 200, 255],
    [0, 125, 125],
    [0, 70, 95],
    [178, 178, 178],
    [102, 102, 102],
];

/// The surface-state forms' colours, from code 64 on.
const SURFACE_STATE: [Rgb; 9] = [
    [20, 30, 90],
    [0, 230, 170],
    [215, 230, 250],
    [255, 255, 255],
    [190, 200, 220],
    [255, 110, 0],
    [120, 80, 40],
    [150, 145, 135],
    [100, 95, 90],
];

fn main() -> io::Result<()> {
    let directory = std::env::args_os().nth(1).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/surface_map"),
        PathBuf::from,
    );
    fs::create_dir_all(&directory)?;
    let field = coarse_pass(reference::SEED, &reference::earth_like());
    let sea_level = field.header().sea_level().value();
    let cells = sample(&field);
    write_ppm(&directory.join("elevation.ppm"), &cells, |cell| {
        elevation_colour(cell, sea_level)
    })?;
    write_ppm(&directory.join("class.ppm"), &cells, |cell| {
        class_colour(cell.class)
    })?;
    write_ppm(&directory.join("flow.ppm"), &cells, flow_colour)?;
    println!(
        "wrote elevation.ppm, class.ppm and flow.ppm ({WIDTH} × {HEIGHT}) to {}",
        directory.display()
    );
    Ok(())
}

/// The field's cell under each pixel's centre, in row-major order from the north-west corner.
fn sample(field: &CoarseField) -> Vec<&SynthesisCell> {
    let level = field.header().level().get();
    let synthesis = field.synthesis();
    (0..HEIGHT)
        .flat_map(|row| (0..WIDTH).map(move |column| (row, column)))
        .map(|(row, column)| {
            let latitude = (90.0 - (f64::from(row) + 0.5) * 180.0 / f64::from(HEIGHT)).to_radians();
            let longitude =
                (-180.0 + (f64::from(column) + 0.5) * 360.0 / f64::from(WIDTH)).to_radians();
            let dir = [
                math::cos(latitude) * math::cos(longitude),
                math::cos(latitude) * math::sin(longitude),
                math::sin(latitude),
            ];
            let key = PatchKey::containing(level, dir).expect("a coarse level is a patch level");
            let index = usize::try_from(cell_index(key)).expect("a cell index fits a usize");
            &synthesis[index]
        })
        .collect()
}

/// Writes `cells` as a binary PPM at `path`, each pixel `colour`'s.
fn write_ppm(
    path: &Path,
    cells: &[&SynthesisCell],
    colour: impl Fn(&SynthesisCell) -> Rgb,
) -> io::Result<()> {
    let mut out = BufWriter::new(fs::File::create(path)?);
    write!(out, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for cell in cells {
        out.write_all(&colour(cell))?;
    }
    out.flush()
}

/// The elevation image's colour of `cell`, the sea level being `sea_level` m.
fn elevation_colour(cell: &SynthesisCell, sea_level: f64) -> Rgb {
    let ground = cell.elevation().value();
    if cell.is_under_water() {
        let surface = cell.water_surface().value();
        let lake = surface.total_cmp(&sea_level).is_ne();
        let depth = ((surface - ground) / ABYSS_M).clamp(0.0, 1.0);
        return if lake {
            blend([110, 170, 240], [40, 90, 200], depth)
        } else {
            blend([90, 150, 230], [5, 15, 70], depth.sqrt())
        };
    }
    let height = ((ground - sea_level) / SUMMIT_M).clamp(0.0, 1.0);
    let stops: [(f64, Rgb); 5] = [
        (0.0, [60, 140, 60]),
        (0.1, [150, 180, 90]),
        (0.3, [190, 160, 100]),
        (0.6, [130, 90, 60]),
        (1.0, [255, 255, 255]),
    ];
    ramp(&stops, height)
}

/// The class image's colour of `class`.
fn class_colour(class: SurfaceClass) -> Rgb {
    let code = usize::from(class.get());
    let koppen = usize::from(SurfaceClass::FIRST_KOPPEN_GEIGER);
    let state = usize::from(SurfaceClass::FIRST_SURFACE_STATE);
    if code >= state {
        SURFACE_STATE
            .get(code - state)
            .copied()
            .unwrap_or([255, 0, 128])
    } else if code >= koppen {
        KOPPEN_GEIGER
            .get(code - koppen)
            .copied()
            .unwrap_or([255, 0, 128])
    } else {
        [0, 0, 0]
    }
}

/// The flow image's colour of `cell`.
fn flow_colour(cell: &SynthesisCell) -> Rgb {
    if cell.is_under_water() {
        return [0, 0, 20];
    }
    let area = cell.drainage.area().value();
    if area <= RIVER_AREA_M2 {
        return [45, 45, 40];
    }
    let t = (math::log10(area / RIVER_AREA_M2) / math::log10(TRUNK_AREA_M2 / RIVER_AREA_M2))
        .clamp(0.0, 1.0);
    blend([40, 90, 255], [255, 255, 255], t)
}

/// The colour `t` of the way from `from` to `to`, `t` in [0, 1].
fn blend(from: Rgb, to: Rgb, t: f64) -> Rgb {
    core::array::from_fn(|k| {
        channel(f64::from(from[k]) + (f64::from(to[k]) - f64::from(from[k])) * t)
    })
}

/// The colour at `t` in [0, 1] along `stops`, each a position and its colour, in increasing
/// position from 0 to 1.
fn ramp(stops: &[(f64, Rgb)], t: f64) -> Rgb {
    stops
        .windows(2)
        .find(|pair| t <= pair[1].0)
        .map_or(stops[stops.len() - 1].1, |pair| {
            let (a, b) = (pair[0], pair[1]);
            blend(a.1, b.1, (t - a.0) / (b.0 - a.0))
        })
}

/// A colour channel of `value`, rounded and held to 0–255.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and held to 0–255 first, which a u8 holds exactly"
)]
fn channel(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}
