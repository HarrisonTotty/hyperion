//! The luminosity tables against realised cells (rendering plan R06, R06.T5.c).
//!
//! For blocks of 216 whole cells of each layer at the solar circle and in the bulge, the summed V
//! light of every realised system, each star's state from `SystemStars::state_at(t).stars()`
//! (pair-evolved since P11.T11), is held against the density field's integral over the block times
//! each component's table, within a compound-Poisson interval (α = 10⁻³, z = 3.29, with the
//! variance estimated by the realised Σ L²) plus 5% for the tables' track sampling, their median
//! draws and their reference metallicity. The ratios of realised to tabulated counts brighter than
//! M<sub>V</sub> −3 and −5 are printed per layer and not gated (decided 2026-10-02, item 3): a ratio
//! above 1.3 in a layer whose cap is set by its bright end is a finding for R06.T7.
//!
//! The tables carry R06.T5.d's pair-evolved correction in layers C, D and E. In those layers the
//! tables' deficit of pair-evolved against single-star light (`LuminosityFunction::pair_light`)
//! is held against the cells' own paired deficit, the same stars evolved each alone, within the
//! cells' compound-Poisson interval and the fit's standard error together; each layer's residual
//! against the tables without the correction is printed beside the one with it.

#[expect(
    dead_code,
    reason = "these checks use the placement helpers of tests/common alone"
)]
mod common;

use common::reference_box_component_integrals;
use hyperion_sim::Seed;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::sky::photometry::absolute_v_of_state;
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::Magnitudes;
use hyperion_sim::units::consts::SOLAR_ABSOLUTE_MAGNITUDE_V;
use hyperion_testkit::stats::{poisson_interval, poisson_two_sided_p};

/// Cells along each edge of a block: 6³ = 216 cells.
const BLOCK_CELLS: i32 = 6;

/// The two-sided normal quantile at α = 10⁻³.
const Z: f64 = 3.29;

/// The allowance for the tables' approximations, a share of the expected light.
const TRACK_SAMPLING: f64 = 0.05;

/// What a block holds: the realised light and its square sum, the light the same stars would have
/// evolved alone (each star's single-star model, which the tables assume), and the stars brighter
/// than −3 and −5.
#[derive(Debug, Default, Clone, Copy)]
struct Realised {
    systems: u64,
    light: f64,
    light_squared: f64,
    single_light: f64,
    /// The sum over systems of the square of each one's pair-evolved less single-star light.
    deficit_squared: f64,
    brighter_3: u64,
    brighter_5: u64,
}

/// A count as a float, for a ratio.
fn count_f64(k: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a count of stars, far below 2^53"
    )]
    let k = k as f64;
    k
}

/// The V light of an absolute magnitude, L☉,V.
fn light_of(v: f64) -> f64 {
    math::exp10(-0.4 * (v - SOLAR_ABSOLUTE_MAGNITUDE_V))
}

fn realise_cell(galaxy: &Galaxy, key: CellKey) -> Realised {
    let mut out = Realised::default();
    let mut records: Vec<SystemRecord> = Vec::new();
    generate_cell(galaxy, key, &mut records);
    for record in &records {
        out.systems += 1;
        let stars = SystemStars::generate(galaxy, record);
        let Some(state) = stars.state_at(UniverseTime::EPOCH) else {
            continue;
        };
        let mut system_light = 0.0;
        for star in state.stars() {
            if let Some(v) = absolute_v_of_state(star) {
                system_light += light_of(v.value());
                out.brighter_3 += u64::from(v.value() < -3.0);
                out.brighter_5 += u64::from(v.value() < -5.0);
            }
        }
        let mut single_light = 0.0;
        for model in stars.stars() {
            if let Some(v) = model
                .state_at(UniverseTime::EPOCH)
                .as_ref()
                .and_then(absolute_v_of_state)
            {
                single_light += light_of(v.value());
            }
        }
        out.single_light += single_light;
        out.deficit_squared += (system_light - single_light) * (system_light - single_light);
        out.light += system_light;
        out.light_squared += system_light * system_light;
    }
    out
}

/// Every cell of `keys` realised, on eight threads, summed in the cells' order.
fn realise(galaxy: &Galaxy, keys: &[CellKey]) -> Realised {
    const THREADS: usize = 8;
    let mut parts = vec![Realised::default(); keys.len()];
    std::thread::scope(|scope| {
        for (chunk_keys, chunk_parts) in keys
            .chunks(keys.len().div_ceil(THREADS))
            .zip(parts.chunks_mut(keys.len().div_ceil(THREADS)))
        {
            scope.spawn(move || {
                for (&key, part) in chunk_keys.iter().zip(chunk_parts) {
                    *part = realise_cell(galaxy, key);
                }
            });
        }
    });
    parts.iter().fold(Realised::default(), |a, b| Realised {
        systems: a.systems + b.systems,
        light: a.light + b.light,
        light_squared: a.light_squared + b.light_squared,
        single_light: a.single_light + b.single_light,
        deficit_squared: a.deficit_squared + b.deficit_squared,
        brighter_3: a.brighter_3 + b.brighter_3,
        brighter_5: a.brighter_5 + b.brighter_5,
    })
}

/// The block of `layer`'s cells centred on `at`: its keys, its low corner, ly, and its edge, ly.
fn block(layer: Layer, at: [f64; 3]) -> (Vec<CellKey>, [i32; 3], u32) {
    let size = i32::try_from(layer.cell_size_ly()).expect("cells are small");
    let corner = at.map(|x: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the sites are whole light-years inside the root cube"
        )]
        let x = x as i32;
        x.div_euclid(size) - BLOCK_CELLS / 2
    });
    let mut keys = Vec::new();
    for i in 0..BLOCK_CELLS {
        for j in 0..BLOCK_CELLS {
            for k in 0..BLOCK_CELLS {
                keys.push(
                    CellKey::new(layer, [corner[0] + i, corner[1] + j, corner[2] + k])
                        .expect("the block lies in the root cube"),
                );
            }
        }
    }
    let edge = u32::try_from(BLOCK_CELLS * size).expect("positive");
    (keys, corner.map(|c| c * size), edge)
}

#[test]
#[ignore = "slow: realises some 10^4 systems with their tracks"]
fn luminosity_matches_realised_cells() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let tables = LuminosityTables::build(&galaxy);
    // The cells are realised at the epoch: the tables, built at +H, read their light there.
    let now = tables.age_for(UniverseTime::EPOCH, Span::ZERO);
    let sites = [
        ("solar circle", [0.0, 26_000.0, 0.0]),
        ("bulge", [0.0, 3_000.0, 0.0]),
    ];
    let layers = [
        Layer::A,
        Layer::B,
        Layer::C,
        Layer::D,
        Layer::E,
        Layer::BrownDwarf,
    ];
    let mut failures = Vec::new();
    for (name, at) in sites {
        for layer in layers {
            let (keys, min_ly, edge) = block(layer, at);
            let integrals = reference_box_component_integrals(&galaxy, min_ly, edge, 4 * 6);
            let band = MassBand::from(layer);
            let (mut light, mut bright_3, mut bright_5, mut systems) = (0.0, 0.0, 0.0, 0.0);
            let (mut pair, mut pair_sigma) = (0.0, 0.0);
            for id in galaxy.fields().component_ids() {
                let component = galaxy.fields().component(id);
                let n = integrals[id.index()] * galaxy.shares().component_share(band, component);
                let table = tables.get_at(id, layer, &PointLy::new(at[0], at[1], at[2]));
                systems += n;
                light += n * table.total_light(now).value();
                pair += n * table.pair_light(now).value();
                pair_sigma += n * table.pair_light_sigma(now).value();
                bright_3 += n * table.count_brighter_than(Magnitudes::new(-3.0), now);
                bright_5 += n * table.count_brighter_than(Magnitudes::new(-5.0), now);
            }
            let realised = realise(&galaxy, &keys);
            let sigma = realised.light_squared.sqrt();
            let allowed = Z * sigma + TRACK_SAMPLING * light;
            let bright = |k: u64, expected: f64| {
                if expected <= 0.0 {
                    return format!("{k} against none");
                }
                // The 95% interval of the count the table expects, and the two-sided p of what was
                // realised: the ratio is recorded, not gated (decided 2026-10-02, item 3).
                let (lo, hi) = poisson_interval(expected, 0.05);
                format!(
                    "{k} against {expected:.3} (95% {lo}–{hi}), ratio {:.2}, p {:.3}",
                    count_f64(k) / expected,
                    poisson_two_sided_p(k, expected)
                )
            };
            // The single-star tables are the corrected ones less their correction.
            let single = light - pair;
            let residual = |tabulated: f64| 100.0 * (realised.light - tabulated) / tabulated;
            eprintln!(
                "{name} {layer:?}: {} systems (expected {systems:.1}); light {:.4e} against \
                 {light:.4e} (± {allowed:.3e}), {:.4e} as single stars; residual {:+.1}% against \
                 the single-star tables, {:+.1}% against the corrected; brighter than −3: {}; \
                 brighter than −5: {}",
                realised.systems,
                realised.light,
                realised.single_light,
                residual(single),
                residual(light),
                bright(realised.brighter_3, bright_3),
                bright(realised.brighter_5, bright_5),
            );
            if (realised.light - light).abs() > allowed {
                failures.push(format!(
                    "{name} {layer:?}: {} against {light} ± {allowed}",
                    realised.light
                ));
            }
            // The pair-evolved deficit: the cells' own, the same stars evolved each alone, against
            // the tables' correction, within the cells' compound-Poisson interval and the fit's
            // standard error (R06.T5.d).
            let deficit = realised.single_light - realised.light;
            let deficit_allowed = Z * math::hypot(realised.deficit_squared.sqrt(), pair_sigma);
            eprintln!(
                "{name} {layer:?}: pair-evolved deficit {deficit:.4e} ({:.1}% of the single-star \
                 light) against the tables' {:.4e} ({:.1}%) ± {deficit_allowed:.3e}",
                100.0 * deficit / realised.single_light,
                -pair,
                -100.0 * pair / single,
            );
            if matches!(layer, Layer::C | Layer::D | Layer::E)
                && (deficit + pair).abs() > deficit_allowed
            {
                failures.push(format!(
                    "{name} {layer:?}: the pair-evolved deficit {deficit} against the tables' {} \
                     ± {deficit_allowed}",
                    -pair
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
