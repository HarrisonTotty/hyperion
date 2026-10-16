//! The registry's liquid columns' golden (plan 14, P14.T49.h; decision-r11-palette-liquids §3).
//!
//! `tests/golden/substance/liquids.golden` pins, by their bits at the generator version, the
//! record [`liquid_properties`] resolves for every row with liquid columns: at its first melt, at
//! a quarter, a half and three quarters of its liquid range (to its critical point, or 100 K above
//! the first melt for a row that states none), and, for water, at 15 and 25 °C. It is a file of
//! its own, beside `registry.golden`, so that the liquids' columns extend the registry without
//! changing a committed line of it. Nothing on the record reads them until P14.T54.a, so until
//! then a change re-blesses this golden at the version in force (decision-bump-plan-after-22
//! §1.2), each re-bless citing that; T54.a's bump commit strikes this sentence, after which a
//! changed value is moved output and rides a bump.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::{self, Composition, PhaseDiagram, liquid_properties};
use hyperion_sim::units::Kelvin;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The span, K, a row with no critical point has its range pinned over above its first melt.
const STATED_SPAN_K: f64 = 100.0;

/// Pins `values` under `label`, bin by bin, or `none`.
fn bins(w: &mut GoldenWriter, label: &str, values: Option<&[f64]>) {
    match values {
        Some(values) => {
            for (k, v) in values.iter().enumerate() {
                w.f64(&format!("{label} bin {k}"), *v);
            }
        }
        None => w.line(&format!("{label} none")),
    }
}

/// Pins `value` under `label`, or `none`.
fn scalar(w: &mut GoldenWriter, label: &str, value: Option<f64>) {
    match value {
        Some(v) => w.f64(label, v),
        None => w.line(&format!("{label} none")),
    }
}

#[test]
fn liquids_golden() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("[liquid]");
    for id in substance::ids() {
        let row = id.substance();
        if row.liquid().is_none() {
            continue;
        }
        let lowest = row
            .condensed()
            .expect("a liquid row has condensed columns")
            .transition()
            .temperature()
            .value();
        let top = row
            .phase_diagram()
            .map_or(lowest + STATED_SPAN_K, |d: &PhaseDiagram| {
                d.critical_temperature().value()
            });
        let mut temperatures: Vec<f64> = [0.0, 0.25, 0.5, 0.75]
            .iter()
            .map(|f| lowest + f * (top - lowest))
            .collect();
        if row.key().as_str() == "H2O" {
            temperatures.extend([288.15, 298.15]);
        }
        for t in temperatures {
            let record = liquid_properties(id, Kelvin::new(t), Composition::Reference)
                .expect("a liquid row's record resolves inside its range");
            let label = format!("liquid {} {} at {t:.3} K", id.index(), row.key());
            w.line(&format!("{label}: basis {:?}", record.basis));
            w.f64(&format!("{label}: temperature"), record.temperature.value());
            w.f64(&format!("{label}: density_kg_m3"), record.density_kg_m3);
            bins(
                &mut w,
                &format!("{label}: refractive_index"),
                Some(&record.refractive_index),
            );
            bins(
                &mut w,
                &format!("{label}: absorption_per_m"),
                record.absorption_per_m.as_ref().map(<[f64; 15]>::as_slice),
            );
            bins(
                &mut w,
                &format!("{label}: scattering_per_m"),
                record.scattering_per_m.as_ref().map(<[f64; 15]>::as_slice),
            );
            scalar(
                &mut w,
                &format!("{label}: surface_tension_n_per_m"),
                record.surface_tension_n_per_m,
            );
            scalar(
                &mut w,
                &format!("{label}: viscosity_pa_s"),
                record.viscosity_pa_s,
            );
        }
    }
    let text = w.finish();
    golden!("substance/liquids", &text);
}
