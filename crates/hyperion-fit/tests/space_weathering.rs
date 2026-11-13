//! The committed space-weathering table is exactly what `run space_weathering --since <current>`
//! would write, and its Λ meets decision-p14-t51d-weathering's addendum of 2026-10-19, as
//! decision-r10-t1d-anchor-colour amends it, on the Moon's anchors (rendering plan R10, R10.T1.d).
//!
//! The anchor tests read the committed table's Λ through the surface crate's law and the sim's
//! fresh `feldspathic` and `basalt_fe` rows at the manifest's anchor-soil `FeO_T`, at m = 1. CI
//! runs them, so a table that is stale, edited by hand, or refitted off the ruling fails.

mod common;

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::manifest::Manifest;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::space_weathering::{Params, Terrain, form_mean_path, fresh_row};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::optics::{BAND_WAVELENGTHS_NM, ReferenceForm};
use hyperion_surface::material::{PhaseLaw, weathered_normal_albedo};
use hyperion_surface::tables::space_weathering::REGOLITH_LAMBDA_M;

/// The table the surface crate compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-surface/src/tables/space_weathering.rs");

/// The task's threads: a fixed few, not the machine's.
///
/// The task ignores the count, which is fixed so that the test stays small beside the others if the
/// task ever uses it.
const THREADS: NonZeroUsize = NonZeroUsize::new(2).expect("2 is not zero");

#[test]
fn space_weathering_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("space_weathering").expect("space_weathering is registered"),
        &Workspace::repository(),
        THREADS,
        GENERATOR_VERSION.get(),
    )
    .expect("the space_weathering task runs");
    common::assert_reproduced(
        "crates/hyperion-surface/src/tables/space_weathering.rs",
        "space_weathering",
        COMMITTED,
        &planned.text,
    );
    assert!(!planned.body_changed);
}

/// The committed manifest's parameters.
fn params() -> Params {
    let manifest = Manifest::load(
        &Workspace::repository()
            .manifests_dir
            .join("space_weathering.toml"),
    )
    .expect("the committed manifest loads");
    Params::of(&manifest).expect("the manifest parses")
}

/// `terrain`'s weathered normal albedos in B, V and R at m = 1 under the committed Λ, and each
/// band's z = (model − centre) ÷ σ.
fn mature(terrain: &Terrain, spread_to_sigma: f64) -> ([f64; 3], [f64; 3]) {
    let row = fresh_row(&terrain.row).expect("the terrain's fresh row is in the registry");
    let mut albedo = [0.0; 3];
    let mut z = [0.0; 3];
    for band in 0..3 {
        albedo[band] = weathered_normal_albedo(
            row.normal[band],
            BAND_WAVELENGTHS_NM[band],
            PhaseLaw::Regolith,
            row.density_kg_m3,
            terrain.feo_mass_fraction,
            1.0,
        );
        z[band] = (albedo[band] - terrain.centres[band]) / terrain.sigma(band, spread_to_sigma);
    }
    eprintln!(
        "{} `{}`: fresh {:?}, mature {albedo:?}, z {z:?}; fresh/mature {:?}",
        terrain.name,
        terrain.row,
        row.normal,
        [0, 1, 2].map(|band| row.normal[band] / albedo[band]),
    );
    (albedo, z)
}

/// The mature highlands lie inside their 10–90% bounds in B, V and R.
#[test]
fn space_weathering_highlands_lie_inside_their_spread() {
    let p = params();
    let (albedo, _) = mature(&p.highland, p.spread_to_sigma);
    for (band, name) in ["B", "V", "R"].into_iter().enumerate() {
        assert!(
            (p.highland.p10[band]..=p.highland.p90[band]).contains(&albedo[band]),
            "highland {name}: {} outside {}–{}",
            albedo[band],
            p.highland.p10[band],
            p.highland.p90[band]
        );
    }
}

/// The least the maria's V may be: its 10% bound less `σ_V` ÷ 8, `σ_V` = (P90 − P10) ÷ the
/// manifest's spread-to-σ divisor (2.563), all as the task computes them from the manifest, with
/// no literal: 0.05677 at present. The addendum's 0.0555 was the same rule on its withdrawn
/// anchors (decision-r10-t1d-anchor-colour §4.2).
fn mare_v_floor(p: &Params) -> f64 {
    p.mare.p10[1] - p.mare.sigma(1, p.spread_to_sigma) / 8.0
}

/// The mature maria lie inside their bounds in B, and in V no lower than [`mare_v_floor`].
#[test]
fn space_weathering_maria_lie_inside_in_b_and_at_the_edge_in_v() {
    let p = params();
    let (albedo, _) = mature(&p.mare, p.spread_to_sigma);
    let floor = mare_v_floor(&p);
    eprintln!("mare V floor {floor}, margin {}", albedo[1] - floor);
    assert!(
        (p.mare.p10[0]..=p.mare.p90[0]).contains(&albedo[0]),
        "mare B: {} outside {}–{}",
        albedo[0],
        p.mare.p10[0],
        p.mare.p90[0]
    );
    assert!(
        albedo[1] >= floor && albedo[1] <= p.mare.p90[1],
        "mare V: {} against the floor {floor}",
        albedo[1]
    );
}

/// The maria's R and colour, pinned as known misses: the regolith form's dark end maps the in-situ
/// maria near the slab's opaque floor, where `A_N` barely responds to τ and the bands converge to
/// grey (D1), and the stand-in fresh rows, clean mineral separates whose fresh Θ₀ is far above a
/// real soil's (D2). Mare R's z lies within −2.3 to −1.5, and the model's B ÷ R within 0.85–0.90,
/// about 0.86 against the anchors' 0.733. A pin that breaks is a signal to refit, never to widen
/// it.
#[test]
fn space_weathering_mare_r_and_colour_are_the_pinned_known_misses() {
    let p = params();
    let (albedo, z) = mature(&p.mare, p.spread_to_sigma);
    assert!((-2.3..=-1.5).contains(&z[2]), "mare R's z {}", z[2]);
    let colour = albedo[0] / albedo[2];
    assert!((0.85..=0.90).contains(&colour), "mare B ÷ R {colour}");
}

/// Λ lies within the addendum's 2.0–2.4 µm, and the implied yield `y_sat` = Λ ÷ ⟨D⟩ within a factor
/// of 3 of Morris 1980's Eq. 4 at T51.d's X = 75, 0.0103–0.093. The fresh/mature contrasts in V
/// (about 5.4× highland and 7.2× mare at the ruled inputs) are stated by the other tests' output,
/// not asserted, against fresh craters' 1.34–1.68 at 600 nm (Dollfus and Bowell 1971, Table 4;
/// D2).
#[test]
fn space_weathering_lambda_and_its_yield_are_the_ruled_fit_s() {
    let p = params();
    let lambda_um = REGOLITH_LAMBDA_M * 1.0e6;
    let y_sat = REGOLITH_LAMBDA_M / form_mean_path(ReferenceForm::Regolith);
    eprintln!("Λ {lambda_um} µm, y_sat {y_sat}");
    assert!((2.0..=2.4).contains(&lambda_um), "Λ {lambda_um} µm");
    let centre = p.checks.yield_centre();
    assert!(
        (0.0103..=0.093).contains(&y_sat),
        "y_sat {y_sat} against {centre}"
    );
}
