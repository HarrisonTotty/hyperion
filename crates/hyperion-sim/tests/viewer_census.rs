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
use hyperion_sim::planetary::record::Section;
use hyperion_sim::planetary::surface::CoarseInputs;
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

fn lab_f(t: f64) -> f64 {
    let delta: f64 = 6.0 / 29.0;
    if t > math::powi(delta, 3) {
        math::cbrt(t)
    } else {
        t / (3.0 * delta * delta) + 4.0 / 29.0
    }
}

/// The light `rgb` (linear Rec. 709) exposed to the meter's grey and stated.
fn shown(rgb: [f64; 3]) -> Shown {
    let y = luminance(rgb);
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

/// One draw's row.
#[derive(Debug, Clone)]
struct Row {
    draw_seed: u64,
    teff_k: f64,
    class: &'static str,
    host_msun: f64,
    a_au: f64,
    mass_mearth: f64,
    state: String,
    material: String,
    pressure_pa: Option<f64>,
    template: String,
    lid: String,
    host_rgb: [f64; 3],
    template_bvr: [f64; 3],
    lid_bvr: Option<[f64; 3]>,
}

fn draw_row(galaxy: &Galaxy, grid: &DensityGrid, draw_seed: u64) -> Row {
    let drawn = draw_inputs(galaxy, grid, draw_seed).expect("a planet of the galaxy");
    let raw = *drawn.inputs.raw();
    let system = ViewerSystem::generate(drawn.inputs).expect("a drawn planet's system");
    let index = BodyIndex::try_from(system.planet().body_index()).expect("a planet's index");
    let record = system
        .planets()
        .body_at(system.context(), index, UniverseTime::EPOCH)
        .expect("the viewer's planet at the epoch");
    let disc = system
        .host_discs_at(UniverseTime::EPOCH)
        .into_iter()
        .next()
        .expect("a host disc at the epoch");
    let colour = disc.colour();
    let [red, green] = colour.red_green();
    let host_rgb = [red, green, colour.blue()];
    let teff_k = disc.teff().value();
    let (state, material) = match record.surface() {
        Section::Ok(surface) => (
            format!("{:?}", surface.published_state()),
            format!("{:?}", surface.material()),
        ),
        _ => match record.envelope() {
            Section::Ok(_) => ("GasEnvelope".to_owned(), "-".to_owned()),
            // No surface section and no envelope modelled: a sub-Neptune's envelope.
            _ => ("Envelope (not modelled)".to_owned(), "-".to_owned()),
        },
    };
    let (template, template_bvr) = match record.photometry() {
        Section::Ok(photometry) => {
            let p = photometry.geometric_albedo();
            (format!("{:?}", photometry.template()), [p.b, p.v, p.r])
        }
        _ => ("not modelled".to_owned(), [f64::NAN; 3]),
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
        Err(_) => ("-".to_owned(), None, None),
    };
    Row {
        draw_seed,
        teff_k,
        class: host_class(teff_k),
        host_msun: raw.star_mass_msun,
        a_au: raw.semi_major_axis_au,
        mass_mearth: raw.mass_mearth,
        state,
        material,
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

/// The air's class by the surface pressure: none below 1 Pa, thin below 30 kPa (plan 14's
/// `THIN_ATMOSPHERE_PRESSURE`), thick at or above it.
fn air_class(state: &str, pressure_pa: Option<f64>) -> &'static str {
    match (state, pressure_pa) {
        ("GasEnvelope" | "Envelope (not modelled)", _) => "envelope",
        ("Airless", _) => "airless",
        ("Tenuous", _) => "tenuous",
        (_, Some(p)) if p < 30_000.0 => "thin",
        (_, Some(_)) => "thick",
        (_, None) => "unknown",
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

/// Each draw's disc colours, in the draws' order: the template's and the lid's, today's (`D65`)
/// and adapted to the host (`HOST`), and the template's and the lid's had every host been the
/// Sun, at `D65` and adapted. A draw with no lid has no lid colours.
#[derive(Debug, Default)]
struct Colours {
    template_d65: Vec<Shown>,
    template_host: Vec<Shown>,
    template_sun_d65: Vec<Shown>,
    template_sun: Vec<Shown>,
    lid_d65: Vec<Shown>,
    lid_host: Vec<Shown>,
    lid_sun: Vec<Shown>,
}

fn cell(s: Shown) -> String {
    format!(
        "#{:02x}{:02x}{:02x} C*{:.0} h{:.0}",
        s.srgb[0], s.srgb[1], s.srgb[2], s.chroma, s.hue_deg
    )
}

/// The colours of `rows` under their hosts and under `sun`, with the table's lines.
fn colours_of(rows: &[Row], sun: [f64; 3]) -> (Colours, Vec<String>) {
    let mut colours = Colours::default();
    let mut lines = Vec::with_capacity(rows.len());
    let sun_balance = white_balance_rows(sun);
    for row in rows {
        let balance = white_balance_rows(row.host_rgb);
        let t_d65 = shown(lit(row.template_bvr, row.host_rgb));
        let t_host = shown(apply(&balance, lit(row.template_bvr, row.host_rgb)));
        assert!(
            t_d65.chroma.is_finite() && t_host.chroma.is_finite(),
            "draw {}: a finite colour",
            row.draw_seed
        );
        colours.template_d65.push(t_d65);
        colours.template_host.push(t_host);
        colours
            .template_sun_d65
            .push(shown(lit(row.template_bvr, sun)));
        colours
            .template_sun
            .push(shown(apply(&sun_balance, lit(row.template_bvr, sun))));
        let (lid_d65, lid_host) = row.lid_bvr.map_or_else(
            || ("-".to_owned(), "-".to_owned()),
            |bvr| {
                let d65 = shown(lit(bvr, row.host_rgb));
                let host = shown(apply(&balance, lit(bvr, row.host_rgb)));
                colours.lid_d65.push(d65);
                colours.lid_host.push(host);
                colours
                    .lid_sun
                    .push(shown(apply(&sun_balance, lit(bvr, sun))));
                (cell(d65), cell(host))
            },
        );
        lines.push(format!(
            "| {} | {:.0} | {} | {:.3} | {:.4} | {:.3} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            row.draw_seed,
            row.teff_k,
            row.class,
            row.host_msun,
            row.a_au,
            row.mass_mearth,
            air_class(&row.state, row.pressure_pa),
            row.state,
            row.material,
            row.template,
            row.lid,
            cell(t_d65),
            cell(t_host),
            lid_d65,
            lid_host,
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
    for (key, count) in tally(rows.iter().map(|r| air_class(&r.state, r.pressure_pa))) {
        println!("- air {key}: {}", share(count, n));
    }
    for (key, count) in tally(rows.iter().map(|r| r.state.as_str())) {
        println!("- state {key}: {}", share(count, n));
    }
    for (key, count) in tally(rows.iter().map(|r| r.template.as_str())) {
        println!("- template {key}: {}", share(count, n));
    }
    for (key, count) in tally(rows.iter().map(|r| r.lid.as_str())) {
        println!("- lid {key}: {}", share(count, n));
    }
}

/// How the colours read: as a whole, by the host's class, and how many distinct 8-bit colours.
fn print_readings(rows: &[Row], colours: &Colours) {
    println!();
    println!(
        "## Disc colours (at the meter's grey; orange-brown is C* >= {CAST_CHROMA} at h {}-{}°, \
         grey C* < {NEUTRAL_CHROMA})",
        ORANGE_HUES.0, ORANGE_HUES.1
    );
    let sets: [(&str, &[Shown]); 7] = [
        ("template, D65 (today)", &colours.template_d65),
        ("template, HOST", &colours.template_host),
        (
            "template, every host the Sun, D65",
            &colours.template_sun_d65,
        ),
        (
            "template, every host the Sun, adapted",
            &colours.template_sun,
        ),
        ("lid, D65 (today)", &colours.lid_d65),
        ("lid, HOST", &colours.lid_host),
        ("lid, every host the Sun, adapted", &colours.lid_sun),
    ];
    for (name, set) in sets {
        println!("- {name}: {}", reading(set));
    }
    let by_class = |class: &str, set: &[Shown]| -> String {
        let picked: Vec<Shown> = rows
            .iter()
            .zip(set)
            .filter(|(r, _)| r.class == class)
            .map(|(_, c)| *c)
            .collect();
        format!("{class} ({}): {}", picked.len(), reading(&picked))
    };
    println!();
    println!("## Template colours by host class");
    for class in ["M", "K", "G", "F"] {
        println!("- D65 {}", by_class(class, &colours.template_d65));
        println!("- HOST {}", by_class(class, &colours.template_host));
    }
    let distinct = |set: &[Shown]| -> usize {
        let mut codes: Vec<[u8; 3]> = set.iter().map(|c| c.srgb).collect();
        codes.sort_unstable();
        codes.dedup();
        codes.len()
    };
    println!();
    println!("## Variety");
    println!(
        "- distinct 8-bit template colours: D65 {}, HOST {}, every host the Sun {}",
        distinct(&colours.template_d65),
        distinct(&colours.template_host),
        distinct(&colours.template_sun)
    );
    println!(
        "- distinct 8-bit lid colours: D65 {}, HOST {}, every host the Sun {} (of {})",
        distinct(&colours.lid_d65),
        distinct(&colours.lid_host),
        distinct(&colours.lid_sun),
        colours.lid_d65.len()
    );
}

#[test]
#[ignore = "slow: 200 RANDOMIZE draws, each a system generated"]
fn randomize_census_of_hosts_surfaces_and_disc_colours() {
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
