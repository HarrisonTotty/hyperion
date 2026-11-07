//! The surface colour's census (the lane `gen-surface-colour`, answering the owner's report that
//! the planet viewer's planets look alike): `RANDOMIZE`'s first 200 draws of the viewer tests'
//! Milky Way fixture, each disc's colour under the Sun before and after the surface colour
//! (`photometry`'s module documentation, "The colour from the body's own surface").
//!
//! A disc's colour is its geometric albedo in the client's channels, R, G and B from `p_R`, `p_V`
//! and `p_B` (`fromWire.ts`'s `geometricAlbedo: [p.r, p.v, p.b]`), lit by the Sun and adapted to
//! the Sun's own white, the planet viewer's `WHITE BALANCE HOST`, so that the light divides out:
//! linear sRGB in proportion to p, scaled to the meter's 18% grey in Rec. 709 luminance and
//! sRGB-encoded to 8 bits (IEC 61966-2-1), with its CIELAB C* and h about D65 (CIE 15:2004), its
//! linear sRGB taken to XYZ by IEC 61966-2-1's primaries at ASTM E308's D65 white (0.95047, 1,
//! 1.08883), which CIELAB's white takes too, so that a grey is achromatic. A
//! colour is grey at C* below 6 and orange-brown at C* of 15 or more at h 25–85°, the planet-variety
//! census's thresholds. Each draw is generated twice, under version 29's rules (its template's
//! row) and 30's (its own surface), and nothing else differs. A census of the native build alone:
//! its 200 draws on one WebAssembly thread would take the slow suite's time for no check of its
//! bits that the golden systems do not make.

use std::collections::BTreeMap;

use hyperion_testkit::float::assert_same_bits;

use super::tests::{galaxy, grid};
use super::*;
use crate::math;
use crate::planetary::derive::photometry::{
    Bands, PhaseTemplate, SurfaceColourRules, with_surface_colour,
};
use crate::planetary::record::{Modelled, Section};
use crate::planetary::testing::in_shares;

/// The draws taken: `RANDOMIZE`'s draw seeds 1 to 200, the viewer's own sequence.
const DRAWS: u64 = 200;

/// The threads the census generates its draws on (its `threads-required` in `.config/nextest.toml`).
const CENSUS_THREADS: usize = 16;

/// The chroma below which a colour is grey, C*.
const GREY_CHROMA: f64 = 6.0;

/// One disc as one rule set draws it.
#[derive(Debug, Clone, PartialEq)]
struct Disc {
    template: PhaseTemplate,
    p: Bands,
}

/// One draw: its host, its planet and its disc under each rule set.
#[derive(Debug, Clone, PartialEq)]
struct Drawn {
    seed: u64,
    host_k: f64,
    a_au: f64,
    mass_mearth: f64,
    state: String,
    lid: String,
    maturity: Option<f64>,
    template: Disc,
    surface: Disc,
}

/// A colour: its 8-bit sRGB code, its C* and its h, degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Colour {
    code: [u8; 3],
    chroma: f64,
    hue_deg: f64,
}

/// `x`, 0 to 1, sRGB-encoded (IEC 61966-2-1).
fn encoded(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * math::powf(x, 1.0 / 2.4) - 0.055
    }
}

/// `x`, 0 to 1, to 8 bits.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "x is clamped to [0, 1], so its rounded 255 x lies in 0–255"
)]
fn byte(x: f64) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// CIELAB's f(t) (CIE 15:2004).
fn lab_f(t: f64) -> f64 {
    let delta: f64 = 6.0 / 29.0;
    if t > delta * delta * delta {
        math::cbrt(t)
    } else {
        t / (3.0 * delta * delta) + 4.0 / 29.0
    }
}

/// The colour of a disc of geometric albedo `p` under the Sun, adapted to its white (the
/// [module documentation](self)).
fn colour(p: Bands) -> Colour {
    let luminance = 0.2126 * p.r + 0.7152 * p.v + 0.0722 * p.b;
    let grey = 0.18 / luminance;
    let (red, green, blue) = (p.r * grey, p.v * grey, p.b * grey);
    let cie_x = 0.412_456_4 * red + 0.357_576_1 * green + 0.180_437_5 * blue;
    let cie_y = 0.212_672_9 * red + 0.715_152_2 * green + 0.072_175 * blue;
    let cie_z = 0.019_333_9 * red + 0.119_192 * green + 0.950_304_1 * blue;
    let (fx, fy, fz) = (
        lab_f(cie_x / 0.950_47),
        lab_f(cie_y),
        lab_f(cie_z / 1.088_83),
    );
    let (a_star, b_star) = (500.0 * (fx - fy), 200.0 * (fy - fz));
    let hue = math::atan2(b_star, a_star).to_degrees();
    Colour {
        code: [red, green, blue].map(|c| byte(encoded(c))),
        chroma: math::hypot(a_star, b_star),
        hue_deg: if hue < 0.0 { hue + 360.0 } else { hue },
    }
}

/// The disc of `inputs`' planet under the surface colour's rules `rules`, and its record's state,
/// lid and maturity: `None` where it has no photometry.
///
/// # Panics
///
/// If the drawn planet's system or record cannot be generated: a draw's inputs are a real
/// planet's, which the viewer regenerates.
fn disc_of(
    inputs: PlanetInputs,
    rules: SurfaceColourRules,
) -> Option<(Disc, String, String, Option<f64>)> {
    with_surface_colour(rules, || {
        let system = ViewerSystem::generate(inputs).expect("a drawn planet's system");
        let index = BodyIndex::try_from(system.planet().body_index()).expect("a planet's index");
        let record = system
            .planets()
            .body_at(system.context(), index, UniverseTime::EPOCH)
            .expect("the drawn planet's record");
        let Section::Ok(photometry) = record.photometry() else {
            return None;
        };
        let (state, lid, maturity) = match record.surface() {
            Section::Ok(surface) => (
                format!("{:?}", surface.published_state()),
                match surface.crust() {
                    Modelled::Ok(crust) => crust.lid_lithology().substance().key().to_string(),
                    Modelled::NotModelled => "-".to_owned(),
                },
                match surface.modifiers() {
                    Modelled::Ok(modifiers) => Some(modifiers.maturity()),
                    Modelled::NotModelled => None,
                },
            ),
            Section::NotModelled | Section::NotApplicable | Section::NotResolved => {
                ("envelope".to_owned(), "-".to_owned(), None)
            }
        };
        Some((
            Disc {
                template: photometry.template(),
                p: photometry.geometric_albedo(),
            },
            state,
            lid,
            maturity,
        ))
    })
}

/// Draw `seed` under both rule sets; `None` where it finds no planet or its planet no photometry.
fn drawn(seed: u64) -> Option<Drawn> {
    // A draw that finds no planet in its tries is none of the census's.
    let drawn = draw_inputs(galaxy(), grid(), seed).ok()?;
    let raw = drawn.inputs.raw();
    let (template, ..) = disc_of(drawn.inputs, SurfaceColourRules::Template)?;
    let (surface, state, lid, maturity) = disc_of(drawn.inputs, SurfaceColourRules::Surface)?;
    let system = ViewerSystem::generate(drawn.inputs).expect("a drawn planet's system");
    let host_k = system
        .summary_at(UniverseTime::EPOCH)
        .stars()
        .first()
        .map_or(0.0, |star| star.state().effective_temperature().value());
    Some(Drawn {
        seed,
        host_k,
        a_au: raw.semi_major_axis_au,
        mass_mearth: raw.mass_mearth,
        state,
        lid,
        maturity,
        template,
        surface,
    })
}

/// The statistics of `colours`, one line.
fn summary(colours: &[Colour]) -> String {
    let n = colours.len();
    let mut chroma: Vec<f64> = colours.iter().map(|c| c.chroma).collect();
    chroma.sort_by(f64::total_cmp);
    let median = chroma.get(n / 2).copied().unwrap_or(f64::NAN);
    let grey = colours.iter().filter(|c| c.chroma < GREY_CHROMA).count();
    let orange = colours
        .iter()
        .filter(|c| c.chroma >= 15.0 && (25.0..=85.0).contains(&c.hue_deg))
        .count();
    let mut codes: Vec<[u8; 3]> = colours.iter().map(|c| c.code).collect();
    codes.sort_unstable();
    codes.dedup();
    let count = |k: usize| f64::from(u32::try_from(k).expect("a census's count fits"));
    let share = |k: usize| 100.0 * count(k) / count(n);
    format!(
        "{n} discs: {} distinct 8-bit colours; grey (C* < 6) {grey} ({:.0}%); orange-brown {orange} \
         ({:.0}%); median C* {median:.1}",
        codes.len(),
        share(grey),
        share(orange),
    )
}

/// The census: `RANDOMIZE`'s first 200 draws, each disc's colour under the Sun under version
/// 29's rules and 30's, stated draw by draw and by template, with the same template and `p_V` to
/// the bit under both. Under 30's the distinct colours are more than twice 29's (the lane's
/// acceptance, "well above 18"); its grey share, the acceptance's other figure (under half), is
/// stated, with the discs it holds grey only by R10.T1.d's terrain colour spread: a mature (m ≥
/// 0.95) airless `basalt` or `basalt_fe` lid, whose weathered colour the law returns near grey
/// where the Moon's maria are red-sloped (decision-r10-t1d-anchor-colour's recorded miss; a
/// finding).
#[test]
#[ignore = "slow: 200 viewer draws generated twice, some minutes"]
fn under_the_surface_colour_randomize_s_discs_take_their_own_colours() {
    let shares = in_shares(
        usize::try_from(DRAWS).expect("200 fits"),
        CENSUS_THREADS,
        |k| drawn(u64::try_from(k).expect("a share's index fits") + 1),
    );
    let draws: Vec<Drawn> = shares.into_iter().flatten().collect();
    let mut out = String::new();
    writeln!(
        out,
        "| draw | host K | a au | mass Me | state | lid | maturity | template | 29 p B/V/R | 29 colour | 30 p B/V/R | 30 colour |"
    )
    .unwrap();
    writeln!(out, "|---|---|---|---|---|---|---|---|---|---|---|---|").unwrap();
    let mut by_template: BTreeMap<String, (Vec<Colour>, Vec<Colour>)> = BTreeMap::new();
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for d in &draws {
        let (old, new) = (colour(d.template.p), colour(d.surface.p));
        let hex = |c: Colour| format!("#{:02x}{:02x}{:02x}", c.code[0], c.code[1], c.code[2]);
        let bands = |p: Bands| format!("{:.3}/{:.3}/{:.3}", p.b, p.v, p.r);
        writeln!(
            out,
            "| {} | {:.0} | {:.4} | {:.3} | {} | {} | {} | {:?} | {} | {} C*{:.0} h{:.0} | {} | {} C*{:.0} h{:.0} |",
            d.seed,
            d.host_k,
            d.a_au,
            d.mass_mearth,
            d.state,
            d.lid,
            d.maturity.map_or_else(|| "-".to_owned(), |m| format!("{m:.3}")),
            d.surface.template,
            bands(d.template.p),
            hex(old),
            old.chroma,
            old.hue_deg,
            bands(d.surface.p),
            hex(new),
            new.chroma,
            new.hue_deg,
        )
        .unwrap();
        assert_eq!(d.template.template, d.surface.template, "draw {}", d.seed);
        // p_V is the template's scale under both rules, to the bit.
        assert_same_bits(d.template.p.v, d.surface.p.v);
        before.push(old);
        after.push(new);
        let entry = by_template
            .entry(format!("{:?}", d.surface.template))
            .or_default();
        entry.0.push(old);
        entry.1.push(new);
    }
    writeln!(out, "\n29 (template): {}", summary(&before)).unwrap();
    writeln!(out, "30 (surface):  {}", summary(&after)).unwrap();
    for (template, (old, new)) in &by_template {
        writeln!(
            out,
            "- {template}: 29 {}; 30 {}",
            summary(old),
            summary(new)
        )
        .unwrap();
    }
    eprintln!("{out}");
    let distinct = |colours: &[Colour]| {
        let mut codes: Vec<[u8; 3]> = colours.iter().map(|c| c.code).collect();
        codes.sort_unstable();
        codes.dedup();
        codes.len()
    };
    let grey = after.iter().filter(|c| c.chroma < GREY_CHROMA).count();
    let dark_end = draws
        .iter()
        .zip(&after)
        .filter(|(d, c)| {
            c.chroma < GREY_CHROMA
                && d.surface.template == PhaseTemplate::Mercury
                && matches!(d.lid.as_str(), "basalt" | "basalt_fe")
                && d.maturity.is_some_and(|m| m >= 0.95)
        })
        .count();
    eprintln!(
        "30's grey discs: {grey} of {}, {dark_end} of them mature airless basalt or basalt_fe \
         lids, grey by the weathering law's terrain colour spread",
        after.len()
    );
    assert!(
        distinct(&after) > 2 * distinct(&before),
        "{} against {}",
        distinct(&after),
        distinct(&before)
    );
}
