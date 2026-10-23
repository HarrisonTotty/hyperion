//! R09.T16's coarse goldens: the field the coarse pass computes for each of `reference`'s six
//! worlds under [`reference::SEED`], one file each in `tests/golden/planetary/surface/`, named for
//! the world's function (`earth_like`, `mars_like`, `moon_like`, `ceres_like`, `europa_like`,
//! `locked_earth_like`).
//!
//! # What is pinned
//!
//! Each file holds, in order:
//!
//! 1. The world: its name, level, cells, climate cells and coarse craters.
//! 2. The header: every member of [`FieldHeaderParts`] in its declaration order, an absent value
//!    written `none`, the spectrum's and the crater contract's every figure, with the boundary
//!    diameter the header derives.
//! 3. The palette (plan 09's T16: "the coarse goldens pin each reference world's palette"): each
//!    entry's every member, a liquid's fifteen-bin arrays by digest.
//! 4. The cells: one digest per member of [`SynthesisCell`] in its declaration order, over the
//!    cells in index order; then summaries to read: the elevation's range, the cells by crust and
//!    by boundary kind, under water and with ice, by class, the crater state's range and median,
//!    and R09.T6.c's epoch incision (its codes `NONE`, `UNCUT` and cut, and the cut codes'
//!    quantiles), which no golden pinned before (the determinism audit of T6.c).
//! 5. The climate layer: one digest per record member, the winds' azimuths and speeds apart.
//! 6. The coarse craters: their count and one digest per member, the degradation bytes among them
//!    (R09.T7.j's determinism finding: "R09.T16's coarse goldens should include a digest of the
//!    reference worlds' byte lists"), and the degradation's range.
//! 7. The payload: the whole field's, every cell at the finest code, its blocks' count and lengths,
//!    its bytes and their digest, each block's generator version (bytes 10 to 13) zeroed first, as
//!    the surface crate's wire golden does, so that a version bump moves only the header line. The
//!    sim's `tests/surface_coarse_golden.rs` reads the `payload digest` line, to tie R10's sample
//!    worlds to these fields.
//! 8. On the two worlds wet now, whose header names a main liquid (the Earth and the locked
//!    Earth), the land hypsometry relative to the sea as R09.T12.a's bands read it: on a world wet
//!    now the coarse land hypsometry is its construction's by rule (decision-r09-t14-choices, T14.c
//!    follow-up 2 §2), so a check after erosion reads T12.a's bands. The reference Earth also holds
//!    them; the locked Earth's are printed only.
//!
//! Every float is written by its bits ([`GoldenWriter::f64`]); integers and codes as text; a
//! digest is [`f64_digest`] over the values as `f64`s: an integer code, which an `f64` holds
//! exactly, as the surface crate's worker golden digests bytes, or a float as it is (the craters'
//! centres, diameters and ages), the testkit reading its bits (the sim's `clippy.toml` bans
//! reading a float's bits here). The payload's bytes go through [`byte_digest`], the same digest
//! without the `f64`s. Each record's members are listed by a pattern with no `..`
//! ([`exhaustive`]), so that a member added to a record fails to compile here until it is pinned.
//!
//! # Why in the library
//!
//! Plan 09 put these goldens in `tests/surface_coarse_golden.rs`. They are here, in the crate's
//! own tests, after R09.T9's precedent (plan 09, "Deviations in T9, as built"): R04's
//! `wasm32-wasip1` runs build the sim's tests without the `testing` feature, so an integration
//! test behind it compiles to nothing there, while the fast goldens are to run on both targets.
//! The integration test keeps R10's side: `tests/common/surface_worlds.rs`'s `sample_worlds()`,
//! each checked against its golden's payload digest.
//!
//! # Tiers
//!
//! The Ceres, the Moon, the Europa and Mars are fast (Mars about 7 s a pass, so that T6.c's epoch
//! codes are pinned in every `ci` on both targets; plan 09 had it slow). The Earth (about 100 s in
//! the test profile) and the locked Earth (about 22 s) are slow, and each runs its pass once,
//! asserting in the same test its climate's orographic solve's convergence: every month's solve
//! converged within 20 restarts (plan 09's R09.T13.e record, E-L8: "R09.T16's sample worlds carry
//! the convergence assertion, at most 20 restarts a month").

use hyperion_surface::craters::{CraterParams, CraterParamsParts, Screening};
use hyperion_surface::field::{
    BoundaryKind, ClimateCell, CoarseCrater, CoarseField, Cover, CoverRange, CraterState, Crust,
    FieldHeaderParts, FieldView, LiquidProperties, LogIncision, PaletteEntry, ResolutionCode,
    SynthesisCell,
};
use hyperion_surface::wire::encode_payload;
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f64_digest};

use super::grid::CellGraph;
use super::reference;
use super::steps::climate::OrographicSolve;
use super::steps::relief::tests::Bands;
use super::{CoarseInputs, coarse_pass, coarse_pass_with_solves};
use crate::GENERATOR_VERSION;
use crate::units::MetresPerSecond;

/// The most restarts a month's orographic solve may take on a sample world (plan 09's R09.T13.e
/// record, E-L8; decision-r09-t13e-lfpm-convergence's acceptance of 20 over its generated sample),
/// two-thirds of the solver's cap of 30.
const MAX_SAMPLE_RESTARTS: u32 = 20;

/// The bytes of a payload block before its generator version: its magic, its format and its
/// length (the surface crate's `wire` frame).
const VERSION_AT: usize = 10;

/// The FNV-1a 64 offset basis, [`f64_digest`]'s.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// The FNV-1a 64 prime, [`f64_digest`]'s.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A member of [`SynthesisCell`]: its name and its code as an `f64`.
type Member = (&'static str, fn(&SynthesisCell) -> f64);

/// `values`' digest ([`f64_digest`]), each value an integer code, which an `f64` holds exactly, or
/// a float as it is.
fn digest(values: impl IntoIterator<Item = f64>) -> u64 {
    f64_digest(&values.into_iter().collect::<Vec<_>>())
}

/// `bytes`' digest as [`digest`] gives it over each byte as an `f64`, without the `f64`s: FNV-1a
/// 64 over each byte's `f64` bits as eight little-endian bytes, through a table of the 256 bytes'.
/// A level-8 field's payload is 15.1 MB, whose `f64`s would take 121 MB, in the wasip1 slow run
/// too.
fn byte_digest(bytes: &[u8]) -> u64 {
    let table: Vec<[u8; 8]> = (0..=u8::MAX)
        .map(|b| bits(f64::from(b)).to_le_bytes())
        .collect();
    bytes.iter().fold(FNV_OFFSET, |hash, &b| {
        table[usize::from(b)]
            .iter()
            .fold(hash, |h, &x| (h ^ u64::from(x)).wrapping_mul(FNV_PRIME))
    })
}

/// Lists every member of the records the goldens pin, by patterns with no `..`, so that a member
/// added to one fails to compile here until its writer pins it. Never called.
#[expect(dead_code, reason = "the patterns are the check, at compile time")]
fn exhaustive(
    cell: &SynthesisCell,
    climate: &ClimateCell,
    crater: &CoarseCrater,
    entry: &PaletteEntry,
    liquid: &LiquidProperties,
    contract: &CraterParamsParts,
) {
    let SynthesisCell {
        elevation_mm: _,
        boundary_distance_km: _,
        plate: _,
        crust: _,
        boundary: _,
        boundary_obliquity: _,
        flow: _,
        drainage: _,
        steepness: _,
        runoff: _,
        catchment_runoff: _,
        epoch_incision: _,
        water_surface_mm: _,
        ice: _,
        substances: _,
        class: _,
        crater_state: _,
        seafloor: _,
    } = cell;
    let ClimateCell {
        sea_level_temperature: _,
        month_anomaly: _,
        month_precipitation: _,
        wind: _,
    } = climate;
    let CoarseCrater {
        centre: _,
        diameter: _,
        morphology: _,
        age: _,
        degradation: _,
        reach: _,
    } = crater;
    let PaletteEntry {
        substance: _,
        role: _,
        normal_albedo_bvr: _,
        phase_row: _,
        grain_density_kg_m3: _,
        transition: _,
        mechanics: _,
        liquid: _,
        feo_mass_fraction: _,
    } = entry;
    let LiquidProperties {
        temperature: _,
        density_kg_m3: _,
        basis: _,
        refractive_index: _,
        absorption_per_m: _,
        scattering_per_m: _,
        surface_tension_n_per_m: _,
        viscosity_pa_s: _,
    } = liquid;
    let CraterParamsParts {
        n_1km: _,
        screening: _,
        gravity: _,
        k_target: _,
        impact_velocity: _,
        projectile_density: _,
        target: _,
        system_age: _,
    } = contract;
}

/// `value` as text: its bits and decimal under `label`, or `label none`.
fn optional(w: &mut GoldenWriter, label: &str, value: Option<f64>) {
    match value {
        Some(v) => w.f64(label, v),
        None => w.line(&format!("{label} none")),
    }
}

/// `entry` of index `n` of the palette: every member in its declaration order.
fn write_entry(w: &mut GoldenWriter, n: usize, entry: &PaletteEntry) {
    let at = format!("palette[{n}]");
    w.line(&format!(
        "{at} substance {} role {} ({:?})",
        entry.substance.as_str(),
        u8::from(entry.role),
        entry.role
    ));
    for (band, albedo) in ["B", "V", "R"].iter().zip(entry.normal_albedo_bvr) {
        w.f64(&format!("{at} normal albedo {band}"), albedo);
    }
    w.line(&format!("{at} phase row {}", entry.phase_row));
    w.f64(
        &format!("{at} grain density kg m-3"),
        entry.grain_density_kg_m3,
    );
    w.f64(&format!("{at} transition K"), entry.transition.value());
    w.line(&format!(
        "{at} mechanics {} ({:?})",
        u8::from(entry.mechanics),
        entry.mechanics
    ));
    match &entry.liquid {
        Some(liquid) => write_liquid(w, &at, liquid),
        None => w.line(&format!("{at} liquid none")),
    }
    optional(
        w,
        &format!("{at} FeO mass fraction"),
        entry.feo_mass_fraction,
    );
}

/// A palette entry's liquid record, every member in its declaration order.
fn write_liquid(w: &mut GoldenWriter, at: &str, liquid: &LiquidProperties) {
    w.f64(
        &format!("{at} liquid temperature K"),
        liquid.temperature.value(),
    );
    w.f64(&format!("{at} liquid density kg m-3"), liquid.density_kg_m3);
    w.line(&format!(
        "{at} liquid basis {} ({:?})",
        u8::from(liquid.basis),
        liquid.basis
    ));
    w.u64_hex(
        &format!("{at} liquid refractive index digest"),
        f64_digest(&liquid.refractive_index),
    );
    for (name, bins) in [
        ("absorption m-1", liquid.absorption_per_m),
        ("scattering m-1", liquid.scattering_per_m),
    ] {
        let label = format!("{at} liquid {name} digest");
        match bins {
            Some(bins) => w.u64_hex(&label, f64_digest(&bins)),
            None => w.line(&format!("{label} none")),
        }
    }
    optional(
        w,
        &format!("{at} liquid surface tension N m-1"),
        liquid.surface_tension_n_per_m,
    );
    optional(
        w,
        &format!("{at} liquid viscosity Pa s"),
        liquid.viscosity_pa_s,
    );
}

/// The header's crater contract: every member of its parts in their declaration order.
fn write_crater_contract(w: &mut GoldenWriter, craters: &CraterParams) {
    let c = craters.parts();
    w.f64("header craters N(>1 km) km-2", c.n_1km.value());
    match c.screening {
        Screening::None => w.line("header craters screening none"),
        Screening::Atmosphere { column_mass } => w.f64(
            "header craters screening atmosphere column mass kg m-2",
            column_mass.value(),
        ),
        Screening::Cutoff { diameter } => {
            w.f64(
                "header craters screening cutoff diameter m",
                diameter.value(),
            );
        }
    }
    w.f64("header craters gravity m s-2", c.gravity.value());
    w.f64("header craters k target", c.k_target);
    optional(
        w,
        "header craters impact velocity m s-1",
        c.impact_velocity.map(MetresPerSecond::value),
    );
    w.f64(
        "header craters projectile density kg m-3",
        c.projectile_density.value(),
    );
    w.line(&format!(
        "header craters target class {} ({:?})",
        u8::from(c.target.class),
        c.target.class
    ));
    w.f64(
        "header craters target density kg m-3",
        c.target.density.value(),
    );
    w.f64("header craters system age Gyr", c.system_age.value());
}

/// The header: every member of [`FieldHeaderParts`] in its declaration order, the palette's
/// entries after it, and the boundary diameter the header derives.
fn write_header(w: &mut GoldenWriter, field: &CoarseField) {
    let header = field.header();
    let FieldHeaderParts {
        body,
        radius,
        figure,
        sea_level,
        lapse_rate_k_per_m,
        spectrum,
        craters,
        climate_model,
        precipitation,
        realised_sigma_h,
        realised_relief,
        months,
        season_eccentricity,
        reference_temperature,
        temperature_step,
        anomaly_step,
        surface_age,
        surface_pressure,
        albedo_scale,
        palette,
        crust_palette,
        main_liquid,
        maturity,
        organic_cover,
        weathering_rate,
    } = header.parts();
    w.line(&format!(
        "header body system {} index {}",
        body.raw_system_id(),
        body.body_index()
    ));
    w.f64("header radius m", radius.value());
    w.f64(
        "header figure equatorial radius m",
        figure.equatorial_radius_m,
    );
    w.f64("header figure polar radius m", figure.polar_radius_m);
    w.f64(
        "header boundary diameter m",
        header.boundary_diameter().value(),
    );
    w.f64("header sea level m", sea_level.value());
    w.f64("header lapse rate K m-1", *lapse_rate_k_per_m);
    w.f64("header spectrum exponent", spectrum.exponent());
    w.f64(
        "header spectrum unit degree variance m2",
        spectrum.unit_degree_variance().value(),
    );
    w.f64("header spectrum break degree", spectrum.break_degree());
    w.f64(
        "header spectrum small-scale exponent",
        spectrum.small_scale_exponent(),
    );
    write_crater_contract(w, craters);
    w.line(&format!(
        "header climate model {} ({climate_model:?})",
        u8::from(*climate_model)
    ));
    w.line(&format!(
        "header precipitation {} ({precipitation:?})",
        u8::from(*precipitation)
    ));
    w.f64("header realised sigma_h m", realised_sigma_h.value());
    w.f64("header realised relief m", realised_relief.value());
    w.line(&format!("header months {months}"));
    w.f64("header season eccentricity", *season_eccentricity);
    w.f64(
        "header reference temperature K",
        reference_temperature.value(),
    );
    w.line(&format!("header temperature step {temperature_step}"));
    w.line(&format!("header anomaly step {anomaly_step}"));
    w.f64("header surface age Gyr", surface_age.value());
    w.f64("header surface pressure Pa", surface_pressure.value());
    optional(w, "header albedo scale", *albedo_scale);
    w.line(&format!(
        "header palette entries {} (below)",
        palette.entries().len()
    ));
    let entry = |e: &Option<u8>| e.map_or_else(|| "none".to_owned(), |e| e.to_string());
    w.line(&format!(
        "header crust palette {}",
        crust_palette
            .iter()
            .map(entry)
            .collect::<Vec<_>>()
            .join(",")
    ));
    w.line(&format!("header main liquid {}", entry(main_liquid)));
    optional(w, "header maturity", *maturity);
    optional(w, "header organic cover", *organic_cover);
    optional(w, "header weathering rate", *weathering_rate);
    for (n, palette_entry) in palette.entries().iter().enumerate() {
        write_entry(w, n, palette_entry);
    }
}

/// The code at rank ⌊(n − 1) q ÷ 100⌋ of `sorted`, n codes ascending: the lower q-th percentile.
fn percentile<T: Copy>(sorted: &[T], q: usize) -> T {
    sorted[(sorted.len() - 1) * q / 100]
}

/// The cells: one digest per member of [`SynthesisCell`] in its declaration order, over the cells
/// in index order, then their summaries ([`write_cell_summaries`]).
fn write_cells(w: &mut GoldenWriter, cells: &[SynthesisCell]) {
    let members: [Member; 18] = [
        ("elevation_mm", |c| f64::from(c.elevation_mm)),
        ("boundary_distance_km", |c| {
            f64::from(c.boundary_distance_km)
        }),
        ("plate", |c| f64::from(c.plate)),
        ("crust", |c| f64::from(u8::from(c.crust))),
        ("boundary", |c| f64::from(u8::from(c.boundary))),
        ("boundary_obliquity", |c| f64::from(c.boundary_obliquity)),
        ("flow", |c| f64::from(u8::from(c.flow))),
        ("drainage", |c| f64::from(c.drainage.get())),
        ("steepness", |c| f64::from(c.steepness.get())),
        ("runoff", |c| f64::from(c.runoff.get())),
        ("catchment_runoff", |c| f64::from(c.catchment_runoff.get())),
        ("epoch_incision", |c| f64::from(c.epoch_incision.get())),
        ("water_surface_mm", |c| f64::from(c.water_surface_mm)),
        ("ice", |c| f64::from(c.ice)),
        ("substances", |c| f64::from(c.substances)),
        ("class", |c| f64::from(c.class.get())),
        ("crater_state", |c| f64::from(c.crater_state)),
        ("seafloor", |c| f64::from(c.seafloor)),
    ];
    for (name, member) in members {
        w.u64_hex(
            &format!("cells {name} digest"),
            digest(cells.iter().map(member)),
        );
    }
    write_cell_summaries(w, cells);
}

/// The cells' summaries the [module documentation](self) lists: the elevation's range, the cells
/// by crust, by boundary kind, under water, with ice and by class, the crater state's range and
/// median, and the epoch incision's codes.
fn write_cell_summaries(w: &mut GoldenWriter, cells: &[SynthesisCell]) {
    let elevations = cells.iter().map(|c| c.elevation_mm);
    w.line(&format!(
        "cells elevation mm min {} max {}",
        elevations.clone().min().expect("a field has cells"),
        elevations.max().expect("a field has cells")
    ));
    let count = |keep: &dyn Fn(&SynthesisCell) -> bool| cells.iter().filter(|c| keep(c)).count();
    w.line(&format!(
        "cells by crust {}",
        [
            Crust::Continental,
            Crust::Oceanic,
            Crust::Lid,
            Crust::Province
        ]
        .map(|crust| format!("{crust:?} {}", count(&|c| c.crust == crust)))
        .join(" ")
    ));
    w.line(&format!(
        "cells by boundary {}",
        [
            BoundaryKind::Absent,
            BoundaryKind::Subduction,
            BoundaryKind::Collision,
            BoundaryKind::Divergent,
            BoundaryKind::Transform
        ]
        .map(|kind| format!("{kind:?} {}", count(&|c| c.boundary == kind)))
        .join(" ")
    ));
    w.line(&format!(
        "cells under water {}",
        count(&SynthesisCell::is_under_water)
    ));
    w.line(&format!("cells with ice {}", count(&|c| c.ice > 0)));
    let mut classes = std::collections::BTreeMap::<u8, u32>::new();
    for cell in cells {
        *classes.entry(cell.class.get()).or_insert(0) += 1;
    }
    w.line(&format!(
        "cells by class {}",
        classes
            .iter()
            .map(|(code, n)| format!("{code}:{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    let mut states: Vec<u8> = cells.iter().map(|c| c.crater_state).collect();
    states.sort_unstable();
    w.line(&format!(
        "cells crater state min {} median {} max {}; at code 1 {}",
        states[0],
        percentile(&states, 50),
        states[states.len() - 1],
        count(&|c| c.crater_state == CraterState::NONE_RETAINED.get())
    ));
    let incision = |code: LogIncision| count(&|c| c.epoch_incision == code);
    let mut cut: Vec<u8> = cells
        .iter()
        .map(|c| c.epoch_incision.get())
        .filter(|&code| code > LogIncision::UNCUT.get())
        .collect();
    cut.sort_unstable();
    let quantiles = if cut.is_empty() {
        "none".to_owned()
    } else {
        format!(
            "p10 {} p50 {} p90 {} max {}",
            percentile(&cut, 10),
            percentile(&cut, 50),
            percentile(&cut, 90),
            cut[cut.len() - 1]
        )
    };
    w.line(&format!(
        "cells epoch incision NONE {} UNCUT {} cut {}; cut codes {quantiles}",
        incision(LogIncision::NONE),
        incision(LogIncision::UNCUT),
        cut.len()
    ));
}

/// The climate layer: one digest per record member, the winds' azimuths and speeds apart, each
/// over the records in index order and their months in order.
fn write_climate(w: &mut GoldenWriter, field: &CoarseField) {
    let records = field.climate_layer();
    w.u64_hex(
        "climate sea_level_temperature digest",
        digest(records.iter().map(|r| f64::from(r.sea_level_temperature))),
    );
    w.u64_hex(
        "climate month_anomaly digest",
        digest(records.iter().flat_map(|r| r.month_anomaly.map(f64::from))),
    );
    w.u64_hex(
        "climate month_precipitation digest",
        digest(
            records
                .iter()
                .flat_map(|r| r.month_precipitation.map(|p| f64::from(p.get()))),
        ),
    );
    w.u64_hex(
        "climate wind azimuth digest",
        digest(
            records
                .iter()
                .flat_map(|r| r.wind.map(|wind| f64::from(wind.azimuth))),
        ),
    );
    w.u64_hex(
        "climate wind speed digest",
        digest(
            records
                .iter()
                .flat_map(|r| r.wind.map(|wind| f64::from(wind.speed))),
        ),
    );
}

/// The coarse craters: their count, one digest per member over the craters in order (a reach as
/// its range count, then each range's start, end and code), and the degradation's range.
fn write_craters(w: &mut GoldenWriter, field: &CoarseField) {
    let craters = field.craters();
    w.line(&format!("craters count {}", craters.len()));
    w.u64_hex(
        "craters centre digest",
        digest(craters.iter().flat_map(|c| c.centre)),
    );
    w.u64_hex(
        "craters diameter digest",
        digest(craters.iter().map(|c| c.diameter.value())),
    );
    w.u64_hex(
        "craters morphology digest",
        digest(craters.iter().map(|c| f64::from(u8::from(c.morphology)))),
    );
    w.u64_hex(
        "craters age digest",
        digest(craters.iter().map(|c| c.age.value())),
    );
    w.u64_hex(
        "craters degradation digest",
        digest(craters.iter().map(|c| f64::from(c.degradation))),
    );
    w.u64_hex(
        "craters reach digest",
        digest(craters.iter().flat_map(|c| {
            let ranges = c.reach.ranges();
            let count = u32::try_from(ranges.len()).expect("a reach's ranges fit a u32");
            std::iter::once(f64::from(count)).chain(ranges.iter().flat_map(|r| {
                [
                    f64::from(r.start()),
                    f64::from(r.end()),
                    f64::from(r.code().get()),
                ]
            }))
        })),
    );
    let degradation = craters.iter().map(|c| c.degradation);
    match (degradation.clone().min(), degradation.max()) {
        (Some(min), Some(max)) => w.line(&format!("craters degradation min {min} max {max}")),
        _ => w.line("craters degradation none"),
    }
}

/// The whole field's payload, every cell at the finest code: its blocks' count and lengths, its
/// bytes and the digest of its bytes, each block's generator version zeroed first (the surface
/// crate's `wire/tests.rs`, `write_payload`), so that a version bump moves only the header.
fn write_payload(w: &mut GoldenWriter, field: &CoarseField) {
    let count = field.header().level().cell_count();
    let cover =
        Cover::from_ranges([CoverRange::new(0, count, ResolutionCode::FINEST).unwrap()]).unwrap();
    let mut bytes = encode_payload(field, &cover, None);
    let mut lengths = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        // The block's length, its frame's third member (a little-endian u32 after the magic and
        // the format).
        let length = u32::from_le_bytes(bytes[at + 6..at + 10].try_into().unwrap());
        bytes[at + VERSION_AT..at + VERSION_AT + 4].fill(0);
        lengths.push(length.to_string());
        at += usize::try_from(length).unwrap();
    }
    assert_eq!(at, bytes.len(), "the blocks tile the payload");
    w.line(&format!(
        "payload blocks {} bytes {} lengths {}",
        lengths.len(),
        bytes.len(),
        lengths.join(",")
    ));
    w.u64_hex("payload digest", byte_digest(&bytes));
}

/// The land hypsometry of `field` relative to its sea, area-weighted over its cells' solid
/// angles: R09.T12.a's three bands ([`Bands`]).
fn hypsometry(field: &CoarseField) -> Bands {
    let sea = field.header().sea_level().value();
    let z: Vec<f64> = field
        .synthesis()
        .iter()
        .map(|c| c.elevation().value() - sea)
        .collect();
    Bands::of(
        &z,
        CellGraph::for_field(field.header().level()).solid_angles_sr(),
    )
}

/// The golden text of `field`, the world `name`'s coarse field, with its hypsometry where its
/// header names a main liquid (a world wet now).
fn text(name: &str, field: &CoarseField) -> (String, Option<Bands>) {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let header = field.header();
    w.line(&format!(
        "world {name} level {} cells {} climate cells {} craters {}",
        header.level().get(),
        field.synthesis().len(),
        field.climate_layer().len(),
        field.craters().len()
    ));
    write_header(&mut w, field);
    write_cells(&mut w, field.synthesis());
    write_climate(&mut w, field);
    write_craters(&mut w, field);
    write_payload(&mut w, field);
    let wet = header.main_liquid().map(|_| hypsometry(field));
    if let Some(h) = wet {
        w.f64("hypsometry land mode m", h.land_mode.value());
        w.f64("hypsometry ocean modal bin m", h.ocean_mode.value());
        w.f64("hypsometry mean below -2 km m", h.deep_mean.value());
    }
    (w.finish(), wet)
}

/// The fast worlds' text: `inputs`' pass under the reference seed, with no hypsometry (none is wet
/// now).
fn fast_text(name: &str, inputs: &CoarseInputs) -> String {
    let (text, wet) = text(name, &coarse_pass(reference::SEED, inputs));
    assert_eq!(wet, None, "{name} is not wet now");
    text
}

/// A world wet now's pass under the reference seed with its orographic solves: asserts that
/// every month's solve converged within [`MAX_SAMPLE_RESTARTS`], printing the restarts by month,
/// and returns its text and hypsometry.
fn wet_text(name: &str, inputs: &CoarseInputs) -> (String, Bands) {
    let (field, solves) = coarse_pass_with_solves(reference::SEED, inputs);
    let restarts: Vec<String> = solves
        .iter()
        .map(|solve| solve.map_or_else(|| "none".to_owned(), |s| s.restarts.to_string()))
        .collect();
    eprintln!(
        "{name}: the orographic solve's restarts by month {}",
        restarts.join(", ")
    );
    assert_eq!(
        solves.len(),
        usize::from(field.header().months()),
        "{name}: one solve a month"
    );
    for (month, solve) in solves.iter().enumerate() {
        let solve: &OrographicSolve = solve
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: month {month} solves its orography"));
        assert!(
            solve.converged && solve.restarts <= MAX_SAMPLE_RESTARTS,
            "{name}: month {month}'s solve {solve:?}"
        );
    }
    let (text, wet) = text(name, &field);
    (text, wet.expect("a world wet now names a main liquid"))
}

#[test]
fn ceres_like_matches_its_golden() {
    golden!(
        "planetary/surface/ceres_like",
        &fast_text("ceres_like", &reference::ceres_like())
    );
}

#[test]
fn moon_like_matches_its_golden() {
    golden!(
        "planetary/surface/moon_like",
        &fast_text("moon_like", &reference::moon_like())
    );
}

#[test]
fn europa_like_matches_its_golden() {
    golden!(
        "planetary/surface/europa_like",
        &fast_text("europa_like", &reference::europa_like())
    );
}

/// Fast, deliberately: about 7 s a pass, so that R09.T6.c's epoch codes are pinned in every `ci`
/// on both targets.
#[test]
fn mars_like_matches_its_golden() {
    golden!(
        "planetary/surface/mars_like",
        &fast_text("mars_like", &reference::mars_like())
    );
}

/// The reference Earth's field, its solves' convergence, and R09.T12.a's three bands on its
/// finished land hypsometry (`decision-r09-t12-coarse.md` item 1): the land mode within 300 m of
/// +125 m, the ocean's modal bin among the centres −5,125 to −4,125 m, and the mean below −2 km
/// from −4,731 to −3,981 m.
#[test]
#[ignore = "slow: the reference Earth's coarse pass at level 8, about 100 s in the test profile"]
fn earth_like_matches_its_golden() {
    let (text, h) = wet_text("earth_like", &reference::earth_like());
    golden!("planetary/surface/earth_like", &text);
    h.assert_earth2014_s("earth_like");
}

/// The locked Earth's field and its solve's convergence; its hypsometry is printed in the golden
/// only.
#[test]
#[ignore = "slow: the reference locked Earth's coarse pass at level 8, about 22 s in the test \
            profile"]
fn locked_earth_like_matches_its_golden() {
    let (text, h) = wet_text("locked_earth_like", &reference::locked_earth_like());
    eprintln!(
        "locked_earth_like: land mode {} m, ocean modal bin {} m, mean below −2 km {:.0} m",
        h.land_mode.value(),
        h.ocean_mode.value(),
        h.deep_mean.value()
    );
    golden!("planetary/surface/locked_earth_like", &text);
}

/// [`byte_digest`] is [`f64_digest`] over the bytes as `f64`s, for every byte value and a run of
/// them.
#[test]
fn the_byte_digest_is_the_f64_digest_of_the_bytes() {
    let bytes: Vec<u8> = (0..=u8::MAX)
        .chain((0..=u8::MAX).rev())
        .chain([0, 7, 255])
        .collect();
    for n in [0, 1, 2, bytes.len()] {
        let some = &bytes[..n];
        assert_eq!(
            byte_digest(some),
            f64_digest(&some.iter().copied().map(f64::from).collect::<Vec<_>>()),
            "{n} bytes"
        );
    }
}
