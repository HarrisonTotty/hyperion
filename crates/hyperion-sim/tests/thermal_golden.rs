//! The condensed phases' thermal columns and the regolith's thermal inertia, pinned (plan 14,
//! P14.T49.j; decision-p14-t49c-split, 2026-10-16).
//!
//! `tests/golden/substance/thermal.golden` pins by their bits, under the generator version's
//! header:
//!
//! - `[thermal]`: each row's thermal columns, in registry order: each column's pieces (phase and
//!   range), its provenance and its sources' bases, and its value at [`PINNED_TEMPERATURES_K`] and
//!   just past each end of each piece;
//! - `[regolith form]`: each mechanics family's reference regolith form (porosity, grain diameter,
//!   emissivity and contact constants), or none;
//! - `[inertia]`: for each row with thermal columns whose family has a form, the inertia Γ with
//!   `k_eff`, its contact and radiative terms, `ρ_bulk` and c at [`PINNED_TEMPERATURES_K`], or the
//!   error at each;
//! - `[built form]`: the same for a few layers built at a caller's porosity and grain size.
//!
//! Nothing on the record reads the module until P14.T54.a, which wires the inertia at P14.T51.c's
//! lid into P14.T24.a, so until then a change re-blesses this golden at the version in force
//! (decision-p14-t49c-split, 2026-10-16: "every value re-blesses at 22 with no bump"), the commit
//! citing that; from P14.T54.a on, a changed value is moved output, which bumps the version. It is
//! a golden of its own, not a section of `registry.golden`, so that the lanes that edit that file
//! share none with this one (main's brief, 2026-10-16).

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::thermal::{
    self, Provenance, RegolithForm, RegolithInertia, SurfaceEnergy, ThermalColumn, ThermalForm,
    reference_regolith_thermal_inertia, regolith_thermal_inertia,
};
use hyperion_sim::substance::{self, Basis, SubstanceId};
use hyperion_sim::units::{Kelvin, Metres};
use hyperion_surface::field::MechanicsFamily;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f64_digest};

/// The temperatures, kelvin, at which the golden pins each column and each inertia: a Kuiper-belt
/// night, an icy moon's, a cold asteroid's, Mars's and Ceres's day, the Moon's 273 K convention,
/// and a hot day.
const PINNED_TEMPERATURES_K: [f64; 6] = [40.0, 90.0, 150.0, 200.0, 273.15, 400.0];

/// How far past each end of each piece the golden pins the held end value, kelvin.
const PAST_THE_END_K: f64 = 1.0;

/// A form's name and its parameters, for the golden.
fn form_line(form: ThermalForm) -> String {
    match form {
        ThermalForm::Constant(value) => format!("constant, digest {:016x}", f64_digest(&[value])),
        ThermalForm::Polynomial(coefficients) => format!(
            "polynomial of {} coefficients, digest {:016x}",
            coefficients.len(),
            f64_digest(coefficients)
        ),
        ThermalForm::HalfPowers(coefficients) => format!(
            "half powers of {} coefficients, digest {:016x}",
            coefficients.len(),
            f64_digest(coefficients)
        ),
        ThermalForm::LogPolynomial(coefficients) => format!(
            "log polynomial of {} coefficients, digest {:016x}",
            coefficients.len(),
            f64_digest(coefficients)
        ),
        ThermalForm::Reciprocal { a, b } => {
            format!("reciprocal, digest {:016x}", f64_digest(&[a, b]))
        }
        ThermalForm::Points(points) => {
            let values: Vec<f64> = points.iter().flat_map(|&(t, v)| [t, v]).collect();
            format!(
                "{} read points, digest {:016x}",
                points.len(),
                f64_digest(&values)
            )
        }
        ThermalForm::IceIh => "IAPWS R10-06 ice Ih".to_owned(),
    }
}

/// A basis's name, for the golden.
fn basis_name(basis: Basis) -> String {
    match basis {
        Basis::Fact => "fact".to_owned(),
        Basis::Reduced => "reduced".to_owned(),
        Basis::Fitted => "fitted".to_owned(),
        Basis::Adopted => "adopted".to_owned(),
        Basis::Raw { licence } => format!("raw ({licence})"),
    }
}

/// One column's lines: its provenance, its pieces and its values.
fn column_section(w: &mut GoldenWriter, label: &str, column: &ThermalColumn) {
    let provenance = match column.provenance() {
        Provenance::Own => "own",
        Provenance::StandIn => "stand-in",
    };
    w.line(&format!("{label}: {provenance}"));
    for (k, piece) in column.pieces().iter().enumerate() {
        let piece_label = format!("{label}: piece {k} '{}'", piece.phase());
        w.line(&format!("{piece_label}: {}", form_line(piece.form())));
        w.f64(&format!("{piece_label}: low_k"), piece.low().value());
        w.f64(&format!("{piece_label}: high_k"), piece.high().value());
        for (what, t) in [
            ("below", piece.low().value() - PAST_THE_END_K),
            ("low", piece.low().value()),
            ("high", piece.high().value()),
            ("above", piece.high().value() + PAST_THE_END_K),
        ] {
            w.f64(
                &format!("{piece_label}: at {what}"),
                column.at(Kelvin::new(t)),
            );
        }
    }
    for t in PINNED_TEMPERATURES_K {
        w.f64(&format!("{label}: at {t} K"), column.at(Kelvin::new(t)));
    }
}

/// The golden's columns: each row's, in registry order.
fn thermal_section(w: &mut GoldenWriter) {
    w.line("[thermal]");
    for (id, columns) in thermal::rows() {
        let label = format!("thermal {} {}", id.index(), id.substance().key());
        let bases: Vec<String> = columns
            .sources()
            .iter()
            .map(|s| format!("{} {}", s.covers(), basis_name(s.basis())))
            .collect();
        w.line(&format!("{label}: sources {}", bases.join("; ")));
        column_section(
            w,
            &format!("{label}: heat_capacity_j_per_kg_k"),
            columns.heat_capacity(),
        );
        column_section(
            w,
            &format!("{label}: conductivity_w_per_m_k"),
            columns.conductivity(),
        );
    }
}

/// The golden's forms: each family's reference form, or none.
fn form_section(w: &mut GoldenWriter) {
    w.line("[regolith form]");
    for &family in MechanicsFamily::ALL {
        let label = format!("form {family:?}");
        let Some(form) = RegolithForm::reference(family) else {
            w.line(&format!("{label}: none"));
            continue;
        };
        let bases: Vec<String> = form
            .sources()
            .iter()
            .map(|s| format!("{} {}", s.covers(), basis_name(s.basis())))
            .collect();
        w.line(&format!("{label}: sources {}", bases.join("; ")));
        w.f64(&format!("{label}: porosity"), form.porosity());
        w.f64(
            &format!("{label}: grain_diameter_m"),
            form.grain_diameter().value(),
        );
        w.f64(&format!("{label}: emissivity"), form.emissivity());
        let contact = form.contact();
        w.f64(
            &format!("{label}: youngs_modulus_pa"),
            contact.youngs_modulus().value(),
        );
        w.f64(&format!("{label}: poisson_ratio"), contact.poisson_ratio());
        match contact.surface_energy() {
            SurfaceEnergy::ProportionalToTemperature { j_per_m2_k } => {
                w.f64(&format!("{label}: surface_energy_j_per_m2_k"), j_per_m2_k);
            }
            SurfaceEnergy::Constant { j_per_m2 } => {
                w.f64(&format!("{label}: surface_energy_j_per_m2"), j_per_m2);
            }
        }
    }
}

/// One inertia's lines under `at`: Γ, `k_eff` with its contact and radiative terms, `ρ_bulk` and c.
fn inertia_lines(w: &mut GoldenWriter, at: &str, inertia: &RegolithInertia) {
    w.f64(
        &format!("{at}: inertia_j_per_m2_k_sqrt_s"),
        inertia.inertia_j_per_m2_k_sqrt_s(),
    );
    w.f64(
        &format!("{at}: conductivity_w_per_m_k"),
        inertia.conductivity_w_per_m_k(),
    );
    w.f64(
        &format!("{at}: contact_conductivity_w_per_m_k"),
        inertia.contact_conductivity_w_per_m_k(),
    );
    w.f64(
        &format!("{at}: radiative_conductivity_w_per_m_k"),
        inertia.radiative_conductivity_w_per_m_k(),
    );
    w.f64(
        &format!("{at}: bulk_density_kg_per_m3"),
        inertia.bulk_density().value(),
    );
    w.f64(
        &format!("{at}: heat_capacity_j_per_kg_k"),
        inertia.heat_capacity_j_per_kg_k(),
    );
}

/// The golden's inertias: each row with columns whose family has a form, at each pinned
/// temperature.
fn inertia_section(w: &mut GoldenWriter) {
    w.line("[inertia]");
    let rows: Vec<SubstanceId> = thermal::rows().map(|(id, _)| id).collect();
    for id in rows {
        let row = id.substance();
        let family = row
            .condensed()
            .expect("a row with thermal columns has condensed columns")
            .mechanics();
        if RegolithForm::reference(family).is_none() {
            continue;
        }
        let label = format!("inertia {} {}", id.index(), row.key());
        for t in PINNED_TEMPERATURES_K {
            let at = format!("{label} at {t} K");
            match reference_regolith_thermal_inertia(id, Kelvin::new(t)) {
                Ok(inertia) => inertia_lines(w, &at, &inertia),
                Err(e) => w.line(&format!("{at}: {e}")),
            }
        }
    }
}

/// The golden's built forms: a layer of each family with a form at a caller's porosity and grain
/// diameter (`RegolithForm::new`, as rendering plan R09's R09.T13.c passes its layers), at a
/// temperature of its row's solid.
fn built_form_section(w: &mut GoldenWriter) {
    w.line("[built form]");
    for (family, porosity, grain_diameter_m, key, t) in [
        (MechanicsFamily::Silicate, 0.4, 1e-3, "feldspathic", 273.15),
        (MechanicsFamily::Silicate, 0.7, 2e-5, "chondrite_cm", 200.0),
        (MechanicsFamily::WaterIce, 0.8, 3e-5, "H2O", 90.0),
    ] {
        let form = RegolithForm::new(family, porosity, Metres::new(grain_diameter_m))
            .expect("a family with a form builds a layer");
        let id = substance::by_key(key).expect("the row is a registry row");
        let at = format!("built {family:?} {porosity} {grain_diameter_m} m: {key} at {t} K");
        match regolith_thermal_inertia(id, &form, Kelvin::new(t)) {
            Ok(inertia) => inertia_lines(w, &at, &inertia),
            Err(e) => w.line(&format!("{at}: {e}")),
        }
    }
}

#[test]
fn substance_thermal_columns_and_inertias_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    thermal_section(&mut w);
    form_section(&mut w);
    inertia_section(&mut w);
    built_form_section(&mut w);
    golden!("substance/thermal", &w.finish());
}
