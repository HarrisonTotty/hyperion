//! The planet viewer's `RANDOMIZE` census (rendering plan R15; the owner's report, 2026-11-05:
//! "the vast majority of them have the same orange look"): 200 draws, draw seeds 1 to 200 as the
//! viewer's `RANDOMIZE` takes them in turn (`usePlanetViewer.ts`' `nextDrawSeed`), of the viewer
//! tests' Milky Way fixture. It prints, for the outbox, each draw's host (its effective temperature,
//! class and mass), its planet's air and surface (the published state, the material, the phase
//! template and the lid's substance) and two colours of its disc, each as drawn today (the client
//! maps light to sRGB with D65's white whatever the host) and adapted to its host's white:
//!
//! - **the template's**, the record's geometric albedo in B, V and R (plan 14, P14.T47), drawn as
//!   red, green and blue: the colour of the disc as a point, before its ground is surveyed, and
//!   the V albedo a surveyed disc's classes are scaled to;
//! - **the lid's**, the palette's lid entry (`CoarseInputs::lid_entry`), its normal albedo in B, V
//!   and R weathered to the ground's maturity (R10.T1.d's `weathered_entry_bvr`): the regolith a
//!   surveyed disc draws over most of an airless body.
//!
//! Each is lit by the host's light, its linear Rec. 709 colour of unit luminance (R06's
//! `HostDisc::colour`), exposed so that its luminance is the meter's 18% grey, and either encoded
//! as it is (`D65`) or first adapted by the linear Bradford transform from the host's white to
//! D65's (`HOST`, `whiteBalance.ts`' `whiteBalanceRows`, whose numbers this file repeats). Each is
//! then stated as 8-bit sRGB and as CIELAB chroma C* and hue h (D65's white), without the tone
//! curve: `AgX` at the meter's grey moves the hue by a few degrees and the chroma by less than a
//! fifth. Then it sums the shares, and says which of the three causes main read (the hosts, the
//! fixed white, the surfaces) the casts follow.
//!
//! It asserts only that every draw finds a planet with a host, finite colours, and that a host's
//! own white adapts to neutral; the figures are the census's, read in the outbox.
//!
//! Run it by name: `cargo nextest run -p hyperion-sim --cargo-profile slow-test --profile slow
//! --run-ignored only --test viewer_census --no-capture`.

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::math;
use hyperion_sim::planetary::BodyIndex;
use hyperion_sim::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use hyperion_sim::planetary::derive::photometry::THIN_ATMOSPHERE_PRESSURE;
use hyperion_sim::planetary::record::{Section, SectionState};
use hyperion_sim::planetary::surface::{CoarseInputs, ReadCoarseInputsError, SurfaceInputsError};
use hyperion_sim::planetary::viewer::{DensityGrid, ViewerSystem, draw_inputs};
use hyperion_sim::time::UniverseTime;
use hyperion_surface::material::weathered_entry_bvr;

/// The draws: `RANDOMIZE`'s first 200, draw seeds 1 to 200.
const DRAWS: u64 = 200;

/// The viewer tests' Milky Way fixture's seed (`planetary/viewer/tests.rs`).
const GALAXY_SEED: u64 = 0x0e15_0001_0000_0001;

/// A 3 × 3 matrix as its rows.
type Rows = [[f64; 3]; 3];

/// Linear sRGB (D65) to CIE XYZ, as rows: Filament's `sRGB_to_XYZ` (ColorSpaceUtils.h), as
/// `toneCurve.ts` holds it.
const SRGB_TO_XYZ: Rows = [
    [0.412_456, 0.357_576, 0.180_438],
    [0.212_673, 0.715_152, 0.072_175],
    [0.019_333_9, 0.119_192, 0.950_304],
];

/// CIE XYZ to linear sRGB (D65), as rows: Filament's `XYZ_to_sRGB`, as `toneCurve.ts` holds it.
const XYZ_TO_SRGB: Rows = [
    [3.240_454_2, -1.537_138_5, -0.498_531_4],
    [-0.969_266, 1.876_010_8, 0.041_556],
    [0.055_643_4, -0.204_025_9, 1.057_225_2],
];

/// The Bradford cone response, XYZ to its sharpened LMS, as rows (Lam 1985; Lindbloom,
/// "Chromatic Adaptation"; the linear Bradford of ICC.1 Annex E and Adobe's DNG specification),
/// as `whiteBalance.ts` holds it.
const BRADFORD: Rows = [
    [0.895_1, 0.266_4, -0.161_4],
    [-0.750_2, 1.713_5, 0.036_7],
    [0.038_9, -0.068_5, 1.029_6],
];

/// The meter's grey: a disc filling the frame is exposed so that its luminance reads 18%.
const METER_GREY: f64 = 0.18;

/// The chroma C* at or above which a disc has a cast; below [`NEUTRAL_CHROMA`] it reads grey.
const CAST_CHROMA: f64 = 15.0;

/// The chroma C* below which a disc reads grey.
const NEUTRAL_CHROMA: f64 = 6.0;

/// The hues of the orange-brown cast, CIELAB h in degrees: red-orange to yellow-orange.
const ORANGE_HUES: (f64, f64) = (25.0, 85.0);

fn apply(m: &Rows, v: [f64; 3]) -> [f64; 3] {
    m.map(|row| row[0] * v[0] + row[1] * v[1] + row[2] * v[2])
}

fn times(a: &Rows, b: &Rows) -> Rows {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    out
}

/// The inverse of `m`, by its cofactors: row `i` of the inverse is the cross products of the
/// columns other than `i`, over the determinant.
fn inverse(m: &Rows) -> Rows {
    let column = |j: usize| [m[0][j], m[1][j], m[2][j]];
    let cross = |u: [f64; 3], v: [f64; 3]| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    let rows = [
        cross(column(1), column(2)),
        cross(column(2), column(0)),
        cross(column(0), column(1)),
    ];
    let det = rows[0][0] * m[0][0] + rows[0][1] * m[1][0] + rows[0][2] * m[2][0];
    rows.map(|row| row.map(|cell| cell / det))
}

/// The luminance of a linear sRGB colour: Filament's Y row.
fn luminance(rgb: [f64; 3]) -> f64 {
    apply(&SRGB_TO_XYZ, rgb)[1]
}

/// The linear Rec. 709 matrix that adapts light of the white `host` (linear Rec. 709) to D65's
/// white of the same luminance, by the linear Bradford transform (von Kries in Bradford's cone
/// space, complete adaptation): `whiteBalance.ts`' `whiteBalanceRows`.
fn white_balance_rows(host: [f64; 3]) -> Rows {
    let source = apply(&SRGB_TO_XYZ, host);
    let d65 = apply(&SRGB_TO_XYZ, [1.0; 3]);
    let target = d65.map(|channel| channel * source[1] / d65[1]);
    let (s, t) = (apply(&BRADFORD, source), apply(&BRADFORD, target));
    let gains: Rows = [
        [t[0] / s[0], 0.0, 0.0],
        [0.0, t[1] / s[1], 0.0],
        [0.0, 0.0, t[2] / s[2]],
    ];
    let lms = times(&inverse(&BRADFORD), &times(&gains, &BRADFORD));
    times(&XYZ_TO_SRGB, &times(&lms, &SRGB_TO_XYZ))
}

/// A disc's colour as the census states it: 8-bit sRGB at the meter's grey, and CIELAB C* and h.
#[derive(Debug, Clone, Copy)]
struct Shown {
    srgb: [u8; 3],
    chroma: f64,
    hue_deg: f64,
}

/// A display-linear value's 8-bit sRGB code, clamped to [0, 1] first: the sRGB transfer function of
/// IEC 61966-2-1:1999, as `tonemap.ts`' `srgbEncode` takes it.
fn srgb_code(linear: f64) -> u8 {
    let v = linear.clamp(0.0, 1.0);
    let encoded = if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * math::powf(v, 1.0 / 2.4) - 0.055
    };
    let code = (encoded * 255.0).round();
    // In [0, 255] after the clamp, so the cast loses nothing.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a rounded value in [0, 255]"
    )]
    let code = code as u8;
    code
}

/// CIELAB's f(t) of a tristimulus ratio t, with δ = 6 ÷ 29 (CIE 15:2004, §8.2.1).
fn lab_f(t: f64) -> f64 {
    let delta: f64 = 6.0 / 29.0;
    if t > math::powi(delta, 3) {
        math::cbrt(t)
    } else {
        t / (3.0 * delta * delta) + 4.0 / 29.0
    }
}

/// The light `rgb` (linear Rec. 709) exposed to the meter's grey and stated.
///
/// # Panics
///
/// For light that is not finite or has no luminance, which no lit disc returns.
fn shown(rgb: [f64; 3]) -> Shown {
    let y = luminance(rgb);
    assert!(
        rgb.iter().all(|channel| channel.is_finite()) && y > 0.0,
        "a lit disc's light {rgb:?}"
    );
    let exposed = rgb.map(|channel| (channel * METER_GREY / y).max(0.0));
    let xyz = apply(&SRGB_TO_XYZ, exposed);
    let white = apply(&SRGB_TO_XYZ, [1.0; 3]);
    let (fx, fy, fz) = (
        lab_f(xyz[0] / white[0]),
        lab_f(xyz[1] / white[1]),
        lab_f(xyz[2] / white[2]),
    );
    let (a, b) = (500.0 * (fx - fy), 200.0 * (fy - fz));
    Shown {
        srgb: exposed.map(srgb_code),
        chroma: math::hypot(a, b),
        hue_deg: math::atan2(b, a).to_degrees().rem_euclid(360.0),
    }
}

/// A host's class by its effective temperature, at the zero-subclass temperatures of Pecaut and
/// Mamajek 2013 (ApJS 208, 9; their Table 5, the dwarfs' sequence, version 2022.04.16): B0 31,400
/// K, A0 9,700 K, F0 7,220 K, G0 5,920 K, K0 5,270 K, M0 3,850 K and L0 2,250 K. Only the
/// temperature is read: a giant takes the class of its temperature.
fn host_class(teff_k: f64) -> &'static str {
    match teff_k {
        t if t >= 31_400.0 => "O",
        t if t >= 9_700.0 => "B",
        t if t >= 7_220.0 => "A",
        t if t >= 5_920.0 => "F",
        t if t >= 5_270.0 => "G",
        t if t >= 3_850.0 => "K",
        t if t >= 2_250.0 => "M",
        _ => "L",
    }
}

/// What a draw's planet is by its record: its published surface state and material, or an
/// envelope with no surface section, modelled or with its section's state.
#[derive(Debug, Clone, Copy)]
enum Ground {
    Surface {
        state: SurfaceState,
        material: SurfaceMaterial,
    },
    Envelope,
    EnvelopeSection(SectionState),
}

impl Ground {
    /// Its state as the table states it.
    fn label(self) -> String {
        match self {
            Self::Surface { state, .. } => format!("{state:?}"),
            Self::Envelope => "GasEnvelope".to_owned(),
            Self::EnvelopeSection(section) => format!("Envelope ({section:?})"),
        }
    }

    /// Its material as the table states it.
    fn material(self) -> String {
        match self {
            Self::Surface { material, .. } => format!("{material:?}"),
            Self::Envelope | Self::EnvelopeSection(_) => "-".to_owned(),
        }
    }
}

/// One draw's row.
#[derive(Debug, Clone)]
struct Row {
    draw_seed: u64,
    teff_k: f64,
    class: &'static str,
    host_msun: f64,
    a_au: f64,
    mass_mearth: f64,
    ground: Ground,
    pressure_pa: Option<f64>,
    template: String,
    lid: String,
    host_rgb: [f64; 3],
    template_bvr: Option<[f64; 3]>,
    lid_bvr: Option<[f64; 3]>,
}

fn draw_row(galaxy: &Galaxy, grid: &DensityGrid, draw_seed: u64) -> Row {
    let drawn = draw_inputs(galaxy, grid, draw_seed)
        .unwrap_or_else(|error| panic!("draw {draw_seed}: a planet of the galaxy: {error}"));
    let raw = *drawn.inputs.raw();
    let system = ViewerSystem::generate(drawn.inputs)
        .unwrap_or_else(|error| panic!("draw {draw_seed}: the drawn planet's system: {error}"));
    let index = BodyIndex::try_from(system.planet().body_index()).expect("a planet's index");
    let record = system
        .planets()
        .body_at(system.context(), index, UniverseTime::EPOCH)
        .unwrap_or_else(|error| panic!("draw {draw_seed}: the planet at the epoch: {error}"));
    let disc = system
        .host_discs_at(UniverseTime::EPOCH)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("draw {draw_seed}: a host disc at the epoch"));
    let colour = disc.colour();
    let [red, green] = colour.red_green();
    let host_rgb = [red, green, colour.blue()];
    let teff_k = disc.teff().value();
    let ground = match (record.surface(), record.envelope()) {
        (Section::Ok(surface), _) => Ground::Surface {
            state: surface.published_state(),
            material: surface.material(),
        },
        (_, Section::Ok(_)) => Ground::Envelope,
        // No surface section, and an envelope's not given: a sub-Neptune's or a steam envelope's
        // (`record.rs`, `BodyRecord::envelope`), by its section's state.
        (
            Section::NotResolved | Section::NotModelled | Section::NotApplicable,
            envelope @ (Section::NotResolved | Section::NotModelled | Section::NotApplicable),
        ) => Ground::EnvelopeSection(envelope.state()),
    };
    let (template, template_bvr) = match record.photometry() {
        Section::Ok(photometry) => {
            let p = photometry.geometric_albedo();
            (
                format!("{:?}", photometry.template()),
                Some([p.b, p.v, p.r]),
            )
        }
        Section::NotResolved | Section::NotModelled | Section::NotApplicable => {
            ("-".to_owned(), None)
        }
    };
    let (lid, lid_bvr, pressure_pa) = match CoarseInputs::for_body(&record, None, system.context())
    {
        Ok(inputs) => {
            let entry = inputs
                .palette()
                .get(inputs.lid_entry())
                .expect("the lid has an entry");
            let maturity = inputs.weathering().map_or(0.0, |w| w.maturity);
            (
                entry.substance.as_str().to_owned(),
                Some(weathered_entry_bvr(entry, maturity)),
                Some(inputs.surface_pressure().value()),
            )
        }
        // A planet with no solid surface has no lid; one whose surface is not modelled or is
        // withheld is said so. Any other refusal is a fault of the reading.
        Err(ReadCoarseInputsError::Section(SurfaceInputsError::NoSolidSurface)) => {
            ("-".to_owned(), None, None)
        }
        Err(ReadCoarseInputsError::Section(
            SurfaceInputsError::NotModelled(_) | SurfaceInputsError::NotResolved(_),
        )) => ("not modelled".to_owned(), None, None),
        Err(error) => panic!("draw {draw_seed}: the planet's coarse inputs: {error}"),
    };
    Row {
        draw_seed,
        teff_k,
        class: host_class(teff_k),
        host_msun: raw.star_mass_msun,
        a_au: raw.semi_major_axis_au,
        mass_mearth: raw.mass_mearth,
        ground,
        pressure_pa,
        template,
        lid,
        host_rgb,
        template_bvr,
        lid_bvr,
    }
}

/// The light a surface of albedos `bvr` (B, V, R) returns under `host` (linear Rec. 709): each
/// band drawn as its display channel, R red, V green and B blue, as `fromWire.ts` maps them.
fn lit(bvr: [f64; 3], host: [f64; 3]) -> [f64; 3] {
    [bvr[2] * host[0], bvr[1] * host[1], bvr[0] * host[2]]
}

/// The air's class: an envelope's, an airless or tenuous surface's by its state, and any other's by
/// its surface pressure, thin below plan 14's `THIN_ATMOSPHERE_PRESSURE` (30 kPa) and thick at or
/// above it.
fn air_class(ground: Ground, pressure_pa: Option<f64>) -> &'static str {
    let by_pressure = || match pressure_pa {
        Some(p) if p < THIN_ATMOSPHERE_PRESSURE.value() => "thin",
        Some(_) => "thick",
        None => "unknown",
    };
    match ground {
        Ground::Envelope | Ground::EnvelopeSection(_) => "envelope",
        Ground::Surface { state, .. } => match state {
            SurfaceState::GasEnvelope | SurfaceState::SteamEnvelope => "envelope",
            SurfaceState::Airless => "airless",
            SurfaceState::Tenuous => "tenuous",
            SurfaceState::MagmaOcean
            | SurfaceState::RunawayGreenhouse
            | SurfaceState::Temperate
            | SurfaceState::Snowball
            | SurfaceState::Desert => by_pressure(),
        },
    }
}

fn share(count: usize, of: usize) -> String {
    // Counts of a 200-draw census are exact in f64.
    #[expect(clippy::cast_precision_loss, reason = "counts below 2^53")]
    let pct = 100.0 * count as f64 / of as f64;
    format!("{count} ({pct:.0}%)")
}

fn tally<'a>(rows: impl Iterator<Item = &'a str>) -> Vec<(String, usize)> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for key in rows {
        *counts.entry(key.to_owned()).or_default() += 1;
    }
    let mut out: Vec<_> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return f64::NAN;
    }
    values[values.len() / 2]
}

/// How a set of shown colours reads: the median chroma, and the shares that read orange-brown,
/// cast of another hue, and grey.
fn reading(colours: &[Shown]) -> String {
    let n = colours.len();
    let orange = colours
        .iter()
        .filter(|c| {
            c.chroma >= CAST_CHROMA && c.hue_deg >= ORANGE_HUES.0 && c.hue_deg <= ORANGE_HUES.1
        })
        .count();
    let other = colours
        .iter()
        .filter(|c| {
            c.chroma >= CAST_CHROMA && !(c.hue_deg >= ORANGE_HUES.0 && c.hue_deg <= ORANGE_HUES.1)
        })
        .count();
    let grey = colours.iter().filter(|c| c.chroma < NEUTRAL_CHROMA).count();
    let hues: Vec<f64> = colours.iter().map(|c| c.hue_deg).collect();
    format!(
        "median C* {:.1}, median h {:.0}°; orange-brown {}, another cast {}, grey {}",
        median(colours.iter().map(|c| c.chroma).collect()),
        median(hues),
        share(orange, n),
        share(other, n),
        share(grey, n),
    )
}

/// A surface's colour four ways: today's (`D65`) and adapted to its host (`HOST`), and had its host
/// been the Sun, at `D65` and adapted.
#[derive(Debug, Clone, Copy)]
struct Seen {
    d65: Shown,
    host: Shown,
    sun_d65: Shown,
    sun: Shown,
}

/// One draw's disc colours: its template's and its lid's, `None` for a draw with no photometry or
/// no lid.
#[derive(Debug, Clone, Copy)]
struct DrawColours {
    template: Option<Seen>,
    lid: Option<Seen>,
}

/// A surface of albedos `bvr` lit by `host` and by `sun`, each balanced by its own white.
fn seen(bvr: [f64; 3], host: [f64; 3], sun: [f64; 3]) -> Seen {
    Seen {
        d65: shown(lit(bvr, host)),
        host: shown(apply(&white_balance_rows(host), lit(bvr, host))),
        sun_d65: shown(lit(bvr, sun)),
        sun: shown(apply(&white_balance_rows(sun), lit(bvr, sun))),
    }
}

fn cell(s: Option<Shown>) -> String {
    s.map_or_else(
        || "-".to_owned(),
        |s| {
            format!(
                "#{:02x}{:02x}{:02x} C*{:.0} h{:.0}",
                s.srgb[0], s.srgb[1], s.srgb[2], s.chroma, s.hue_deg
            )
        },
    )
}

/// The colours of `rows` under their hosts and under `sun`, in the rows' order, with the table's
/// lines.
fn colours_of(rows: &[Row], sun: [f64; 3]) -> (Vec<DrawColours>, Vec<String>) {
    let mut colours = Vec::with_capacity(rows.len());
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        let draw = DrawColours {
            template: row.template_bvr.map(|bvr| seen(bvr, row.host_rgb, sun)),
            lid: row.lid_bvr.map(|bvr| seen(bvr, row.host_rgb, sun)),
        };
        colours.push(draw);
        lines.push(format!(
            "| {} | {:.0} | {} | {:.3} | {:.4} | {:.3} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            row.draw_seed,
            row.teff_k,
            row.class,
            row.host_msun,
            row.a_au,
            row.mass_mearth,
            air_class(row.ground, row.pressure_pa),
            row.ground.label(),
            row.ground.material(),
            row.template,
            row.lid,
            cell(draw.template.map(|t| t.d65)),
            cell(draw.template.map(|t| t.host)),
            cell(draw.lid.map(|l| l.d65)),
            cell(draw.lid.map(|l| l.host)),
        ));
    }
    (colours, lines)
}

/// The shares of the hosts' classes and the planets' airs, states, templates and lids.
fn print_shares(rows: &[Row]) {
    let n = rows.len();
    println!();
    println!("## Hosts");
    for (class, count) in tally(rows.iter().map(|r| r.class)) {
        println!("- {class}: {}", share(count, n));
    }
    println!(
        "- median Teff {:.0} K",
        median(rows.iter().map(|r| r.teff_k).collect())
    );
    println!();
    println!("## Planets: air, state, template, lid");
    for (key, count) in tally(rows.iter().map(|r| air_class(r.ground, r.pressure_pa))) {
        println!("- air {key}: {}", share(count, n));
    }
    let states: Vec<String> = rows.iter().map(|r| r.ground.label()).collect();
    for (key, count) in tally(states.iter().map(String::as_str)) {
        println!("- state {key}: {}", share(count, n));
    }
    for (key, count) in tally(rows.iter().map(|r| r.template.as_str())) {
        println!("- template {key}: {}", share(count, n));
    }
    for (key, count) in tally(rows.iter().map(|r| r.lid.as_str())) {
        println!("- lid {key}: {}", share(count, n));
    }
}

/// One way of seeing a draw's disc, picked from its colours.
type Pick = fn(&DrawColours) -> Option<Shown>;

/// The colours `pick` picks of the draws of `rows` that `keep` keeps.
fn picked(
    rows: &[Row],
    colours: &[DrawColours],
    pick: Pick,
    keep: impl Fn(&Row) -> bool,
) -> Vec<Shown> {
    rows.iter()
        .zip(colours)
        .filter(|(row, _)| keep(row))
        .filter_map(|(_, draw)| pick(draw))
        .collect()
}

/// How the colours read: as a whole, by the host's class, and how many distinct 8-bit colours.
fn print_readings(rows: &[Row], colours: &[DrawColours]) {
    println!();
    println!(
        "## Disc colours (at the meter's grey; orange-brown is C* >= {CAST_CHROMA} at h {}-{}°, \
         grey C* < {NEUTRAL_CHROMA})",
        ORANGE_HUES.0, ORANGE_HUES.1
    );
    let template_d65: Pick = |c| c.template.map(|t| t.d65);
    let template_host: Pick = |c| c.template.map(|t| t.host);
    let template_sun: Pick = |c| c.template.map(|t| t.sun);
    let lid_d65: Pick = |c| c.lid.map(|l| l.d65);
    let lid_host: Pick = |c| c.lid.map(|l| l.host);
    let lid_sun: Pick = |c| c.lid.map(|l| l.sun);
    let sets: [(&str, Pick); 7] = [
        ("template, D65 (today)", template_d65),
        ("template, HOST", template_host),
        ("template, every host the Sun, D65", |c| {
            c.template.map(|t| t.sun_d65)
        }),
        ("template, every host the Sun, adapted", template_sun),
        ("lid, D65 (today)", lid_d65),
        ("lid, HOST", lid_host),
        ("lid, every host the Sun, adapted", lid_sun),
    ];
    for (name, pick) in sets {
        println!(
            "- {name}: {}",
            reading(&picked(rows, colours, pick, |_| true))
        );
    }
    println!();
    println!("## Template colours by host class");
    for class in ["M", "K", "G", "F"] {
        for (name, pick) in [("D65", template_d65), ("HOST", template_host)] {
            let set = picked(rows, colours, pick, |row| row.class == class);
            println!("- {name} {class} ({}): {}", set.len(), reading(&set));
        }
    }
    let distinct = |pick: Pick| -> usize {
        let mut codes: Vec<[u8; 3]> = picked(rows, colours, pick, |_| true)
            .iter()
            .map(|c| c.srgb)
            .collect();
        codes.sort_unstable();
        codes.dedup();
        codes.len()
    };
    println!();
    println!("## Variety");
    println!(
        "- distinct 8-bit template colours: D65 {}, HOST {}, every host the Sun {}",
        distinct(template_d65),
        distinct(template_host),
        distinct(template_sun)
    );
    println!(
        "- distinct 8-bit lid colours: D65 {}, HOST {}, every host the Sun {} (of {})",
        distinct(lid_d65),
        distinct(lid_host),
        distinct(lid_sun),
        picked(rows, colours, lid_d65, |_| true).len()
    );
}

#[test]
#[ignore = "slow: 200 RANDOMIZE draws, each a system generated"]
fn every_randomize_draw_lights_a_planet_whose_host_white_adapts_to_neutral() {
    let galaxy = Galaxy::from_params(Seed::new(GALAXY_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    let grid = DensityGrid::of(&galaxy);
    let rows: Vec<Row> = (1..=DRAWS)
        .map(|seed| draw_row(&galaxy, &grid, seed))
        .collect();
    assert_eq!(rows.len(), usize::try_from(DRAWS).expect("200 fits"));
    let sun = {
        let colour = hyperion_sim::sky::colour::solar_colour();
        let [red, green] = colour.red_green();
        [red, green, colour.blue()]
    };

    // A host's own white adapts to D65's neutral at its own luminance.
    for row in &rows {
        let neutral = apply(&white_balance_rows(row.host_rgb), row.host_rgb);
        let y = luminance(row.host_rgb);
        for channel in neutral {
            assert!(
                (channel - y).abs() < 1e-6,
                "draw {}: the host's white {:?} adapts to {neutral:?}",
                row.draw_seed,
                row.host_rgb
            );
        }
    }

    let (colours, lines) = colours_of(&rows, sun);
    println!("# RANDOMIZE census: draw seeds 1 to {DRAWS}, galaxy seed {GALAXY_SEED:#018x}");
    println!();
    println!(
        "| draw | host Teff K | class | host Msun | a au | mass Mearth | air | state | material | \
         template | lid | template D65 | template HOST | lid D65 | lid HOST |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for line in lines {
        println!("{line}");
    }
    print_shares(&rows);
    print_readings(&rows, &colours);
}
