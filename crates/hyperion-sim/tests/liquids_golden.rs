//! The registry's liquid columns' golden (plan 14, P14.T49.h; decision-r11-palette-liquids §3).
//!
//! `tests/golden/substance/liquids.golden` pins, by their bits at the generator version, the
//! record [`liquid_properties`] resolves for every row with liquid columns at its reference
//! composition, which a solution's lines print by key: at its first melt, at a quarter, a half,
//! three quarters and 0.999 of its liquid range (to the lowest critical point among its
//! components, a pure liquid's own, or 100 K above the first melt for a row that states none),
//! where the critical-point rules act; for water, at 15 and 25 °C; for Titan's seas at 80, 90,
//! 93.65 and 100 K, below and above their components' triple points; for the chloride and sulphate
//! brines at 273.15, 288.15, 298.15 and 350 K, and at those and their eutectic at a given
//! composition, 3.5 wt% salt, printed by key; for the aqueous acid at 250, 273.15, 300 and 323.15
//! K, and at those and its first melt at 80 and 84.5 wt%, its second file's composition; a
//! lithology's silicate melt at its solidus and at 1,473.15, 1,673.15, 2,073.15 and 3,000 K (its
//! range has no upper end; a temperature below its solidus pins the refusal); a row whose record
//! is refused (the perchlorate brines, for their index) by its refusal; and, in
//! `[supercooled]`, each continued viscosity
//! ([`SupercooledViscosity`](hyperion_sim::substance::liquid::SupercooledViscosity)) from its
//! triple point down to 71.15 K, Titan's seas' first melt. It is a file of
//! its own, beside `registry.golden`, so that the liquids' columns extend the registry without
//! changing a committed line of it. Nothing on the record reads them until P14.T54.a, so until
//! then a change re-blesses this golden at the version in force (decision-bump-plan-after-22
//! §1.2), each re-bless citing that; T54.a's bump commit strikes this sentence, after which a
//! changed value is moved output and rides a bump.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::{
    self, Composition, LiquidComposition, PhaseDiagram, SubstanceId, SubstanceKind, by_key,
    liquid_properties,
};
use hyperion_sim::units::Kelvin;
use hyperion_surface::field::LiquidProperties;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The span, K, a row with no critical point has its range pinned over above its first melt.
const STATED_SPAN_K: f64 = 100.0;

/// The temperatures, K, a row is pinned at beside its range's fractions: water at 15 and 25 °C;
/// Titan's seas below methane's and ethane's triple points (80 and 90 K), at Huygens's 93.65 K and
/// above it; the brines at their condensed columns' 0 °C, at water's 15 and 25 °C and at 350 K; and
/// the acid inside its sources' 0–50 °C, at its files' 300 K among them.
const EXTRA_TEMPERATURES_K: [(&str, &[f64]); 5] = [
    ("H2O", &[288.15, 298.15]),
    ("H2SO4", &ACID_TEMPERATURES_K),
    ("ch4_c2h6_n2", &[80.0, 90.0, 93.65, 100.0]),
    ("brine_nacl", &BRINE_TEMPERATURES_K),
    ("brine_mgso4", &BRINE_TEMPERATURES_K),
];

/// The temperatures, K, a lithology's silicate melt is pinned at beside its solidus: 1,200 °C,
/// 1,400 °C, the viscosity model's calculator's 1,800 °C ceiling, and a lava world's 3,000 K.
const MELT_TEMPERATURES_K: [f64; 4] = [1_473.15, 1_673.15, 2_073.15, 3_000.0];

/// See [`EXTRA_TEMPERATURES_K`].
const BRINE_TEMPERATURES_K: [f64; 4] = [273.15, 288.15, 298.15, 350.0];

/// See [`EXTRA_TEMPERATURES_K`].
const ACID_TEMPERATURES_K: [f64; 4] = [250.0, 273.15, 300.0, 323.15];

/// The given compositions pinned, each a row with its solvent, its solute, the solute's mass
/// fraction and the temperatures beside its first melt: the brines at 3.5 wt%, seawater's
/// salinity, at [`BRINE_TEMPERATURES_K`]; the acid at 80 wt%, between its files, and at 84.5 wt%,
/// its second file's, at [`ACID_TEMPERATURES_K`].
const GIVEN: [(&str, &str, &str, f64, &[f64]); 4] = [
    ("H2SO4", "H2O", "H2SO4", 0.80, &ACID_TEMPERATURES_K),
    ("H2SO4", "H2O", "H2SO4", 0.845, &ACID_TEMPERATURES_K),
    ("brine_nacl", "H2O", "NaCl", 0.035, &BRINE_TEMPERATURES_K),
    ("brine_mgso4", "H2O", "MgSO4", 0.035, &BRINE_TEMPERATURES_K),
];

/// The registry's row `key`, which the golden knows to be one.
fn row(key: &str) -> SubstanceId {
    by_key(key).expect("the golden's rows are registry rows")
}

/// Pins the record `id` resolves at `t` K and `composition` under `label`, or its refusal.
fn pin_resolved(
    w: &mut GoldenWriter,
    label: &str,
    id: SubstanceId,
    t: f64,
    composition: Composition<'_>,
) {
    match liquid_properties(id, Kelvin::new(t), composition) {
        Ok(record) => pin_record(w, label, &record),
        Err(refusal) => w.line(&format!("{label}: refused: {refusal}")),
    }
}

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

/// Pins every member of `record` under `label`.
fn pin_record(w: &mut GoldenWriter, label: &str, record: &LiquidProperties) {
    w.line(&format!("{label}: basis {:?}", record.basis));
    w.f64(&format!("{label}: temperature"), record.temperature.value());
    w.f64(&format!("{label}: density_kg_m3"), record.density_kg_m3);
    bins(
        w,
        &format!("{label}: refractive_index"),
        Some(&record.refractive_index),
    );
    bins(
        w,
        &format!("{label}: absorption_per_m"),
        record.absorption_per_m.as_ref().map(<[f64; 15]>::as_slice),
    );
    bins(
        w,
        &format!("{label}: scattering_per_m"),
        record.scattering_per_m.as_ref().map(<[f64; 15]>::as_slice),
    );
    scalar(
        w,
        &format!("{label}: surface_tension_n_per_m"),
        record.surface_tension_n_per_m,
    );
    scalar(
        w,
        &format!("{label}: viscosity_pa_s"),
        record.viscosity_pa_s,
    );
}

/// `[liquid]`: every row with liquid columns at its reference composition.
fn pin_references(w: &mut GoldenWriter) {
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
        let columns = row.liquid().expect("the row has liquid columns");
        let components = columns.components();
        let top = if components.is_empty() {
            lowest + STATED_SPAN_K
        } else {
            components
                .iter()
                .filter_map(|c| c.substance().phase_diagram())
                .map(|d: &PhaseDiagram| d.critical_temperature().value())
                .fold(f64::INFINITY, f64::min)
        };
        let solution = components.len() > 1;
        if let (true, Some(reference)) = (solution, columns.reference_composition()) {
            for &(component, x) in reference.mole_fractions() {
                w.f64(
                    &format!(
                        "liquid {} {}: reference mole fraction {}",
                        id.index(),
                        row.key(),
                        component.substance().key()
                    ),
                    x,
                );
            }
        }
        let melt = row.kind() == SubstanceKind::Lithology;
        let mut temperatures: Vec<f64> = if melt {
            std::iter::once(lowest)
                .chain(
                    MELT_TEMPERATURES_K
                        .into_iter()
                        .filter(|t| (t - lowest).abs() > 1e-9),
                )
                .collect()
        } else {
            [0.0, 0.25, 0.5, 0.75, 0.999]
                .iter()
                .map(|f| lowest + f * (top - lowest))
                .collect()
        };
        for (key, extra) in EXTRA_TEMPERATURES_K {
            if row.key().as_str() == key {
                temperatures.extend(extra);
            }
        }
        for t in temperatures {
            let label = format!("liquid {} {} at {t:.3} K", id.index(), row.key());
            pin_resolved(w, &label, id, t, Composition::Reference);
        }
    }
}

/// `[given]`: the [`GIVEN`] compositions, each printed by key and its solute's mass fraction.
fn pin_given(w: &mut GoldenWriter) {
    w.line("[given]");
    for (key, solvent, solute, w_s, extra) in GIVEN {
        let id = row(key);
        let molar_mass = |k: &str| {
            row(k)
                .substance()
                .molar_mass_g_per_mol()
                .expect("a component has a molar mass")
        };
        let moles_s = w_s / molar_mass(solute);
        let moles_w = (1.0 - w_s) / molar_mass(solvent);
        let fractions = [
            (row(solvent), moles_w / (moles_s + moles_w)),
            (row(solute), moles_s / (moles_s + moles_w)),
        ];
        let given = LiquidComposition::new(&fractions).expect("a whole composition of species");
        let entry = format!("given {} {key}, {w_s} {solute} by mass", id.index());
        let mut names = Vec::new();
        for &(component, x) in &fractions {
            let name = component.substance().key();
            w.f64(&format!("{entry}: mole fraction {name}"), x);
            names.push(name.as_str().to_owned());
        }
        let eutectic = id
            .substance()
            .condensed()
            .expect("a liquid row has condensed columns")
            .transition()
            .temperature()
            .value();
        for t in std::iter::once(eutectic).chain(extra.iter().copied()) {
            let label = format!("{entry} ({}) at {t:.3} K", names.join(", "));
            pin_resolved(w, &label, id, t, Composition::Given(given));
        }
    }
}

/// `[supercooled]`: each continued viscosity from its triple point down to 71.15 K.
fn pin_supercooled(w: &mut GoldenWriter) {
    w.line("[supercooled]");
    for id in substance::ids() {
        let row = id.substance();
        let Some(form) = row.liquid().and_then(|c| c.supercooled_viscosity()) else {
            continue;
        };
        let triple = form.triple_temperature();
        let at_triple = liquid_properties(id, triple, Composition::Reference)
            .expect("a supercooled row resolves at its triple point")
            .viscosity_pa_s
            .expect("a supercooled row has a viscosity");
        for k in 0..=4 {
            let t = triple.value() - (triple.value() - 71.15) * f64::from(k) / 4.0;
            w.f64(
                &format!(
                    "supercooled {} {} at {t:.3} K: viscosity_pa_s",
                    id.index(),
                    row.key()
                ),
                form.viscosity_pa_s(at_triple, Kelvin::new(t)),
            );
        }
    }
}

#[test]
fn liquids_golden() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    pin_references(&mut w);
    pin_given(&mut w);
    pin_supercooled(&mut w);
    let text = w.finish();
    golden!("substance/liquids", &text);
}
