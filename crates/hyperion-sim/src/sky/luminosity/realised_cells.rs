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
//!
//! The cells are the budget's ([`budget_galaxy`]), which the tables read (R06.T16.a's item 1 lean,
//! 2026-11-01): from generator version 27 a version's grid holds the field alone, less the
//! features' share (P09.T2.c) and the catalogue classes' hosts (P09.T35), and gains plan 08's
//! displaced classes in layers D and E (P08.T12), none of which the tables' light per star is
//! about. So this test moved here from `tests/sky_luminosity.rs`, where those test-only doors are
//! out of reach, as the band's own test reads the budget (`sky::band`'s `budget_galaxy`).

use std::sync::OnceLock;

use hyperion_testkit::stats::{poisson_interval, poisson_two_sided_p};

use super::LuminosityTables;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::displaced::DisplacedRules;
use crate::galaxy::displaced::site::CarveOutRules;
use crate::galaxy::fields::MAX_COMPONENTS;
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::math;
use crate::rng::Seed;
use crate::sky::photometry::absolute_v_of_state;
use crate::stellar::system::SystemStars;
use crate::time::{Span, UniverseTime};
use crate::units::Magnitudes;
use crate::units::consts::SOLAR_ABSOLUTE_MAGNITUDE_V;

/// The galaxy's seed.
const SEED: u64 = 0x0926_0000;

/// Cells along each edge of a block: 6³ = 216 cells.
const BLOCK_CELLS: i32 = 6;

/// The two-sided normal quantile at α = 10⁻³.
const Z: f64 = 3.29;

/// The allowance for the tables' approximations, a share of the expected light.
const TRACK_SAMPLING: f64 = 0.05;

/// The most threads the cells are realised on.
const THREADS: usize = 8;

/// The Milky Way-like galaxy of [`SEED`] with its grid holding every population's whole budget, as
/// the tables read it: the field keeps the features' share (`FieldShareRules::None`) and the
/// catalogue classes' hosts (`CarveOutRules::Unclaimed`), and layers D and E hold the field alone,
/// each component at its population's whole band share (`DisplacedRules::FieldOnly`). Its density
/// fields, its components and so its tables are the version's galaxy's.
fn budget_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules {
                field_share: FieldShareRules::None,
                carve_out: CarveOutRules::Unclaimed,
                displaced: DisplacedRules::FieldOnly,
                ..GalaxyRules::of_version(crate::GENERATOR_VERSION)
            },
        )
        .expect("the Milky Way-like parameters are valid")
    })
}

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

/// Each component's number of systems in the box of whole light-years from `min_ly` with edge
/// `size_ly`: the midpoint sum of its density over `steps³` equal boxes.
fn box_component_integrals(
    galaxy: &Galaxy,
    min_ly: [i32; 3],
    size_ly: u32,
    steps: u32,
) -> [f64; MAX_COMPONENTS] {
    let fields = galaxy.fields();
    let mut integrals = [0.0; MAX_COMPONENTS];
    let mut densities = [0.0; MAX_COMPONENTS];
    let [x0, y0, z0] = min_ly.map(f64::from);
    let h = f64::from(size_ly) / f64::from(steps);
    let weight = h * h * h;
    let at = |base: f64, i: u32| base + (f64::from(i) + 0.5) * h;
    for ix in 0..steps {
        for iy in 0..steps {
            for iz in 0..steps {
                let point = PointLy::new(at(x0, ix), at(y0, iy), at(z0, iz));
                fields.densities(&point, &mut densities);
                for (integral, density) in integrals.iter_mut().zip(densities) {
                    *integral += weight * density;
                }
            }
        }
    }
    integrals
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

/// Every cell of `keys` realised, on up to [`THREADS`] threads (in turn on WebAssembly, which has
/// none), summed in the cells' order.
///
/// Each cell's part is its own and the parts are summed in the cells' order however they were
/// realised, so every number of threads gives the same bits.
fn realise(galaxy: &Galaxy, keys: &[CellKey]) -> Realised {
    let parts = crate::planetary::testing::in_shares(keys.len(), THREADS, |k| {
        realise_cell(galaxy, keys[k])
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
    let galaxy = budget_galaxy();
    let tables = LuminosityTables::build(galaxy);
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
            let integrals = box_component_integrals(galaxy, min_ly, edge, 4 * 6);
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
            let realised = realise(galaxy, &keys);
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
