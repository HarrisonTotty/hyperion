//! The substance registry's golden and its public mirror (plan 14, P14.T49.a; decision-composition
//! §1.1).
//!
//! - `tests/golden/substance/registry.golden` pins every row's values by their bits, at the
//!   generator version: the elements' order, each row's identity (index, key, kind, charge,
//!   stoichiometry and molar mass), its client optics, its gas columns (the held class, the
//!   molecular constants, and `c_p` ÷ R, H̃ and S̃ at fixed temperatures; P14.T49.b), its phase
//!   columns, its phase diagram (its points, anchors, curve, densities and enthalpies at fixed
//!   fractions of its triple and critical temperatures; P14.T49.b), and its condensed columns
//!   (grain density, transition and mechanics family; P14.T49.b), and its condensed optics (each
//!   modelled row's reference form, index file, phase law and fitted albedos; P14.T49.c). A changed
//!   value is moved output, which bumps the version; until P14.T54.a nothing on the record reads
//!   the diagram, condensed and albedo sections, which re-bless at the version in force.
//! - `packages/protocol/fixtures/substances.json` mirrors the rows for the client (index, key,
//!   kind, molar mass, `c_p` ÷ R and the three optics flags), which rendering plan R08's parity
//!   check and P14.T35.e's decode tests read. `just bless` (`HYPERION_BLESS=1`) writes it, and
//!   otherwise this test holds it to the registry byte for byte. Its `c_p` ÷ R is generator
//!   version 22's held class, `null` for the rows P14.T49.b appends; P14.T54.a's bump renames it
//!   `heat_capacity_over_r_298k`, the statistical sum's at 298.15 K, for display and cross-check
//!   only (science-r08-gases-2.md §1.4; decision-bump-plan-after-22 §1.2).
//!
//! Both are append-only in their identities: a row or an element is appended, never renumbered,
//! renamed or re-kinded. Each test reads the committed file before it compares or blesses, and
//! fails unless the committed identities are a prefix of the registry's, so that not even a bless
//! can change a key a build has sent.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::optics::{EndMember, GrainScaling, LabSpectrum, OpticsInput};
use hyperion_sim::substance::{
    self, CondensedPhase, EQUILIBRIUM_HYDROGEN, Element, GasColumns, GrainDensity, LevelSet,
    MolecularConstants, PhaseDiagram, Substance, TransitionKind,
};
use hyperion_sim::units::{Kelvin, KilogramsPerCubicMetre, Pascals};
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, Mode, f64_digest};

/// The temperatures, kelvin, at which the golden pins each gas row's `c_p` ÷ R: Triton's to a
/// near-runaway world's.
const PINNED_TEMPERATURES: [f64; 7] = [30.0, 100.0, 200.0, 298.15, 500.0, 1_000.0, 2_000.0];

/// The temperature, kelvin, at which the golden pins each gas row's H̃ and S̃.
const STANDARD_TEMPERATURE: f64 = 298.15;

/// The repository's root, two levels above this crate.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lies two levels below the repository")
        .to_path_buf()
}

/// The committed text of the file at `path`, or `None` before it is first blessed.
///
/// # Panics
///
/// If the file exists but cannot be read: only a missing file may skip the append-only check.
fn committed(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    }
}

/// Panics unless `committed`, the committed file's identities in order, is a prefix of `current`.
fn assert_append_only(what: &str, committed: &[String], current: &[String]) {
    assert!(
        committed.len() <= current.len(),
        "{what}: the registry has {} rows, fewer than the committed {}; a row is never removed",
        current.len(),
        committed.len()
    );
    for (i, (old, new)) in committed.iter().zip(current).enumerate() {
        assert_eq!(
            old, new,
            "{what}: identity line {i} changed; the registry is append-only, so a row or an \
             element is never renumbered, renamed or re-kinded and a row's molar mass never moves"
        );
    }
}

/// A row's stoichiometry, as `H 2 O 1`.
fn elements(row: &Substance) -> String {
    let mut text = String::new();
    for (element, count) in row.elements() {
        if !text.is_empty() {
            text.push(' ');
        }
        write!(text, "{} {count}", element.symbol()).expect("writing to a String cannot fail");
    }
    text
}

/// The golden's gas section: each gas row's held class, molecular constants, and its `c_p` ÷ R at
/// [`PINNED_TEMPERATURES`] and H̃ and S̃ at [`STANDARD_TEMPERATURE`] (P14.T49.b).
fn gas_section(w: &mut GoldenWriter) {
    w.line("[gas]");
    for id in substance::ids() {
        let row = id.substance();
        let Some(gas) = row.gas() else {
            continue;
        };
        let label = format!("gas {} {}", id.index(), row.key());
        // The held class of rows 0–8, generator version 22's published value, keeps T49.a's lines.
        if let (Some(class), Some(over_r)) = (gas.heat_capacity(), gas.heat_capacity_over_r()) {
            w.line(&format!("{label}: {class:?}"));
            w.f64(&format!("{label}: heat_capacity_over_r"), over_r);
        }
        let molecular = gas.molecular();
        w.line(&format!("{label}: rotation {:?}", molecular.rotation()));
        for (k, vibration) in molecular.vibrations().iter().enumerate() {
            w.f64(
                &format!(
                    "{label}: vibration {k}, degeneracy {}, wavenumber_cm",
                    vibration.degeneracy()
                ),
                vibration.wavenumber_cm(),
            );
        }
        for (k, set) in molecular.levels().iter().enumerate() {
            let what = match *set {
                LevelSet::Listed { levels, .. } => {
                    let values: Vec<f64> = levels
                        .iter()
                        .flat_map(|level| [level.energy_cm(), level.degeneracy()])
                        .collect();
                    format!(
                        "{} levels, digest {:016x}",
                        levels.len(),
                        f64_digest(&values)
                    )
                }
                LevelSet::Rovibrational { terms, spin, .. } => {
                    w.f64(
                        &format!("{label}: level set {k}, term (0, 0) cm"),
                        terms.term_cm(0, 0),
                    );
                    format!("rovibrational {spin:?}")
                }
            };
            w.f64(
                &format!("{label}: level set {k}, {what}, weight"),
                set.weight(),
            );
        }
        pin_sums(w, &label, molecular);
    }
    // Row 0 is normal hydrogen; the recorded other bound, equilibrium hydrogen, is pinned too.
    pin_sums(w, "equilibrium hydrogen", &EQUILIBRIUM_HYDROGEN);
}

/// Pins `molecular`'s `c_p` ÷ R at [`PINNED_TEMPERATURES`] and its H̃ and S̃ at
/// [`STANDARD_TEMPERATURE`], under `label`.
fn pin_sums(w: &mut GoldenWriter, label: &str, molecular: &MolecularConstants) {
    for t in PINNED_TEMPERATURES {
        let at = Kelvin::new(t);
        let c = molecular
            .heat_capacity_over_r(at)
            .expect("a pinned temperature is positive");
        w.f64(&format!("{label}: heat_capacity_over_r at {t} K"), c);
    }
    let standard = Kelvin::new(STANDARD_TEMPERATURE);
    w.f64(
        &format!("{label}: enthalpy_over_r_k at {STANDARD_TEMPERATURE} K"),
        molecular
            .enthalpy_over_r(standard)
            .expect("298.15 K is positive")
            .value(),
    );
    w.f64(
        &format!("{label}: entropy_over_r at {STANDARD_TEMPERATURE} K"),
        molecular
            .entropy_over_r(standard)
            .expect("298.15 K is positive"),
    );
}

/// Pins `value` under `label`, or `none`.
fn f64_or_none(w: &mut GoldenWriter, label: &str, value: Option<f64>) {
    match value {
        Some(v) => w.f64(label, v),
        None => w.line(&format!("{label} none")),
    }
}

/// The golden's diagram section: each condensable's points, anchors, saturation pressure at fixed
/// fractions of its triple and critical temperatures, liquid and solid densities, melting
/// pressure, enthalpies and boiling point under one atmosphere (P14.T49.b).
fn diagram_section(w: &mut GoldenWriter) {
    w.line("[diagram]");
    for id in substance::diagram::condensables() {
        let row = id.substance();
        let d: &PhaseDiagram = row
            .phase_diagram()
            .expect("a condensable has a phase diagram");
        let label = format!("diagram {} {}", id.index(), row.key());
        let (t_t, t_c) = (
            d.triple_temperature().value(),
            d.critical_temperature().value(),
        );
        for (name, value) in [
            ("triple_temperature_k", t_t),
            ("triple_pressure_pa", d.triple_pressure().value()),
            ("critical_temperature_k", t_c),
            ("critical_pressure_pa", d.critical_pressure().value()),
            (
                "vaporisation_j_per_mol",
                d.triple_vaporisation_enthalpy_j_per_mol(),
            ),
            (
                "triple_liquid_density_kg_per_m3",
                d.triple_liquid_density().value(),
            ),
        ] {
            w.f64(&format!("{label}: {name}"), value);
        }
        f64_or_none(
            w,
            &format!("{label}: fusion_j_per_mol"),
            d.triple_fusion_enthalpy_j_per_mol(),
        );
        for (k, (piece, anchor)) in d.sublimation().iter().zip(d.anchors()).enumerate() {
            w.f64(
                &format!(
                    "{label}: piece {k} {} from {} K, anchor",
                    piece.phase(),
                    piece.lower().value()
                ),
                *anchor,
            );
        }
        let middle = f64::midpoint(t_t, t_c);
        for (name, t) in [
            ("0.3 T_t", 0.3 * t_t),
            ("0.6 T_t", 0.6 * t_t),
            ("0.9 T_t", 0.9 * t_t),
            ("T_t", t_t),
            ("mid", middle),
            ("0.99 T_c", 0.99 * t_c),
        ] {
            let at = Kelvin::new(t);
            w.f64(
                &format!("{label}: saturation_pressure_pa at {name}"),
                d.saturation_pressure(at).value(),
            );
            f64_or_none(
                w,
                &format!("{label}: clausius_clapeyron_j_per_mol at {name}"),
                d.clausius_clapeyron_enthalpy_j_per_mol(at),
            );
        }
        f64_or_none(
            w,
            &format!("{label}: liquid_density_kg_per_m3 at mid"),
            d.liquid_density(Kelvin::new(middle))
                .map(KilogramsPerCubicMetre::value),
        );
        for (name, t) in [("0.3 T_t", 0.3 * t_t), ("0.9 T_t", 0.9 * t_t)] {
            f64_or_none(
                w,
                &format!("{label}: solid_density_kg_per_m3 at {name}"),
                d.solid_density(Kelvin::new(t))
                    .map(KilogramsPerCubicMetre::value),
            );
        }
        for (name, t) in [("0.95 T_t", 0.95 * t_t), ("1.05 T_t", 1.05 * t_t)] {
            f64_or_none(
                w,
                &format!("{label}: melting_pressure_pa at {name}"),
                d.melting_pressure(Kelvin::new(t)).map(Pascals::value),
            );
        }
        f64_or_none(
            w,
            &format!("{label}: falling_melting_pressure_pa at 0.95 T_t"),
            d.falling_melting_pressure(Kelvin::new(0.95 * t_t))
                .map(Pascals::value),
        );
        f64_or_none(
            w,
            &format!("{label}: saturation_temperature_k at 101325 Pa"),
            d.saturation_temperature(Pascals::new(101_325.0))
                .map(Kelvin::value),
        );
    }
}

/// The golden's condensed section: each row's grain density (a fixed row's solid's and liquid's,
/// each with its temperature or none; a diagram row's solid at 0.9 of its triple temperature and
/// its saturated liquid at it), its transition with its kind (a eutectic's composition too) and its
/// mechanics family (P14.T49.b).
fn condensed_section(w: &mut GoldenWriter) {
    w.line("[condensed]");
    for id in substance::ids() {
        let row = id.substance();
        let Some(condensed) = row.condensed() else {
            continue;
        };
        let label = format!("condensed {} {}", id.index(), row.key());
        match condensed.grain_density() {
            GrainDensity::Fixed { solid, liquid } => {
                w.line(&format!("{label}: grain density fixed"));
                for (phase, given) in [("solid", solid), ("liquid", liquid)] {
                    match given {
                        Some(given) => {
                            w.f64(
                                &format!("{label}: {phase} grain_density_kg_per_m3"),
                                given.density().value(),
                            );
                            w.f64(
                                &format!("{label}: {phase} grain_density_at_k"),
                                given.at().value(),
                            );
                        }
                        None => w.line(&format!("{label}: {phase} grain_density_kg_per_m3 none")),
                    }
                }
            }
            GrainDensity::OfDiagram => {
                w.line(&format!("{label}: grain density of its diagram"));
                let t_t = row
                    .phase_diagram()
                    .expect("a diagram row has a phase diagram")
                    .triple_temperature();
                f64_or_none(
                    w,
                    &format!("{label}: solid grain_density_kg_per_m3 at 0.9 T_t"),
                    row.grain_density(CondensedPhase::Solid, Kelvin::new(0.9 * t_t.value()))
                        .map(KilogramsPerCubicMetre::value),
                );
                f64_or_none(
                    w,
                    &format!("{label}: liquid grain_density_kg_per_m3 at T_t"),
                    row.grain_density(CondensedPhase::Liquid, t_t)
                        .map(KilogramsPerCubicMetre::value),
                );
            }
        }
        let transition = condensed.transition();
        w.line(&format!("{label}: transition {}", transition.kind().name()));
        w.f64(
            &format!("{label}: transition_k"),
            transition.temperature().value(),
        );
        if let TransitionKind::Eutectic {
            solute_mass_fraction,
        } = transition.kind()
        {
            w.f64(
                &format!("{label}: eutectic solute_mass_fraction"),
                solute_mass_fraction,
            );
        }
        w.line(&format!("{label}: mechanics {:?}", condensed.mechanics()));
    }
}

/// The lines of a laboratory row's input under `label`: each end-member's spectra and scaling, each
/// mixture part's fraction and grain class, and each level (P14.T49.i).
fn input_lines(w: &mut GoldenWriter, label: &str, input: OpticsInput) {
    let end_member = |w: &mut GoldenWriter, label: &str, e: &EndMember| {
        let files: Vec<&str> = e.spectra().iter().map(LabSpectrum::file).collect();
        w.line(&format!("{label}: {} spectra {files:?}", e.name()));
        match e.scaling() {
            GrainScaling::Held => w.line(&format!("{label}: {} held", e.name())),
            GrainScaling::Slab {
                real_index,
                lab_grain_diameter,
            } => {
                w.f64(&format!("{label}: {} slab n", e.name()), real_index);
                w.f64(
                    &format!("{label}: {} slab lab grain m", e.name()),
                    lab_grain_diameter.value(),
                );
            }
        }
    };
    match input {
        OpticsInput::Spectrum { end_member: e } => end_member(w, label, e),
        OpticsInput::Mixture { parts } => {
            for (i, part) in parts.iter().enumerate() {
                let e = part.end_member();
                let part_label = format!("{label}: part {i}");
                w.f64(
                    &format!(
                        "{part_label} {} {:?} volume_fraction",
                        e.name(),
                        part.grain()
                    ),
                    part.volume_fraction(),
                );
                end_member(w, &part_label, e);
            }
        }
        OpticsInput::Levels { levels } => {
            let g = levels.geometry();
            w.line(&format!(
                "{label}: levels at i {} e {} azimuth {} deg",
                g.incidence_deg, g.emission_deg, g.azimuth_deg
            ));
            for (nm, value) in levels.knots() {
                w.f64(&format!("{label}: level at {nm} nm"), *value);
            }
        }
        OpticsInput::Index { .. }
        | OpticsInput::SizedIndex { .. }
        | OpticsInput::OpaqueSlab { .. } => {}
    }
}

/// The golden's albedo section: each row with condensed optics, its reference form, its input (a
/// stand-in or not; an index file and infrared set, or a laboratory input with its end-members,
/// parts and fine grain size) and its phase law, its fitted values in B, V and R: w, the normal
/// albedo, the spherical albedo and the directional-hemispherical albedo at each incidence
/// (P14.T49.c), and its spherical albedo in each spectral bin and its data's reach (P14.T49.i).
fn albedo_section(w: &mut GoldenWriter) {
    w.line("[albedo]");
    for (id, optics) in substance::optics::rows() {
        let label = format!("albedo {} {}", id.index(), id.substance().key());
        let law = id
            .substance()
            .condensed()
            .expect("a row with optics has condensed columns")
            .phase_law();
        let input = match optics.input() {
            OpticsInput::Index { file, .. } => format!("from {file}"),
            OpticsInput::SizedIndex {
                file,
                grain_diameter,
                ..
            } => format!(
                "from {file} at {} um grains",
                grain_diameter.value() * 1.0e6
            ),
            OpticsInput::OpaqueSlab { file } => format!("by the opaque slab of {file}"),
            OpticsInput::Spectrum { end_member } => format!("from {}", end_member.name()),
            OpticsInput::Mixture { parts } => format!(
                "from a mixture of {} parts, fine grains {} um",
                parts.len(),
                substance::optics::FINE_GRAIN_DIAMETER.value() * 1.0e6
            ),
            OpticsInput::Levels { .. } => "from measured levels".to_owned(),
        };
        w.line(&format!(
            "{label}: {:?} {input}{}, phase law {law:?}",
            optics.reference_form(),
            if optics.is_stand_in() {
                " (stand-in)"
            } else {
                ""
            }
        ));
        input_lines(w, &label, optics.input());
        for (name, values) in [
            ("w", optics.single_scattering_albedo_bvr()),
            ("normal_albedo", optics.normal_albedo_bvr()),
            ("spherical_albedo", optics.spherical_albedo_bvr()),
        ] {
            for (band, value) in ["B", "V", "R"].into_iter().zip(values) {
                w.f64(&format!("{label}: {name} {band}"), value);
            }
        }
        for (incidence, mu0) in substance::optics::HEMISPHERICAL_INCIDENCE_COSINES
            .into_iter()
            .enumerate()
        {
            let values = optics
                .directional_hemispherical_albedo_bvr(incidence)
                .expect("each incidence has its albedos");
            for (band, value) in ["B", "V", "R"].into_iter().zip(values) {
                w.f64(
                    &format!("{label}: hemispherical_albedo at mu0 {mu0} {band}"),
                    value,
                );
            }
        }
        w.line(&format!("{label}: infrared {:?}", optics.infrared_source()));
        let edges = substance::optics::SPECTRAL_BIN_EDGES_NM;
        for (bin, value) in optics.spherical_albedo_bins().into_iter().enumerate() {
            w.f64(
                &format!(
                    "{label}: spherical_albedo {}-{} nm",
                    edges[bin],
                    edges[bin + 1]
                ),
                value,
            );
        }
        let [lo, hi] = optics.data_reach_nm();
        w.f64(&format!("{label}: data from nm"), lo);
        w.f64(&format!("{label}: data to nm"), hi);
    }
}

/// The golden's text.
fn registry_golden() -> String {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("[elements]");
    for (i, element) in Element::ALL.into_iter().enumerate() {
        w.line(&format!("element {i} {}", element.symbol()));
    }
    w.line("[identity]");
    for id in substance::ids() {
        let row = id.substance();
        let label = format!("identity {} {}", id.index(), row.key());
        w.line(&format!(
            "{label}: {}, charge {}, elements [{}]",
            row.kind().name(),
            row.charge(),
            elements(row)
        ));
        match row.molar_mass_g_per_mol() {
            Some(m) => w.f64(&format!("{label}: molar_mass_g_per_mol"), m),
            None => w.line(&format!("{label}: molar_mass_g_per_mol none")),
        }
    }
    w.line("[optics]");
    for id in substance::ids() {
        let optics = id.substance().optics();
        w.line(&format!(
            "optics {} {}: rayleigh {}, absorber {}, aerosol_material {}",
            id.index(),
            id.substance().key(),
            optics.rayleigh(),
            optics.absorber(),
            optics.aerosol_material()
        ));
    }
    gas_section(&mut w);
    w.line("[phase]");
    for id in substance::condensables() {
        let row = id.substance();
        let phase = row.phase().expect("a condensable has phase columns");
        let label = format!("phase {} {}", id.index(), row.key());
        for (name, value) in [
            ("triple_temperature_k", phase.triple_temperature().value()),
            ("triple_pressure_pa", phase.triple_pressure().value()),
            (
                "critical_temperature_k",
                phase.critical_temperature().value(),
            ),
            ("critical_pressure_pa", phase.critical_pressure().value()),
            (
                "sublimation_j_per_mol",
                phase.sublimation_enthalpy_j_per_mol(),
            ),
            (
                "vaporisation_j_per_mol",
                phase.vaporisation_enthalpy_j_per_mol(),
            ),
            (
                "triple_liquid_density_kg_per_m3",
                phase.triple_liquid_density().value(),
            ),
        ] {
            w.f64(&format!("{label}: {name}"), value);
        }
    }
    diagram_section(&mut w);
    condensed_section(&mut w);
    albedo_section(&mut w);
    w.finish()
}

/// The golden's lines that only grow, those that begin with `prefix`: the elements' and the rows'
/// identities.
fn identity_lines(text: &str, prefix: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

#[test]
fn substance_registry_is_pinned_and_append_only() {
    let text = registry_golden();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/substance/registry.golden");
    if let Some(old) = committed(&path) {
        for prefix in ["element ", "identity "] {
            assert_append_only(
                "registry.golden",
                &identity_lines(&old, prefix),
                &identity_lines(&text, prefix),
            );
        }
    }
    golden!("substance/registry", &text);
}

/// The fixture's text: one object a row, a value a line, as Prettier keeps it.
fn fixture() -> String {
    let mut rows = Vec::new();
    for id in substance::ids() {
        let row = id.substance();
        let number =
            |value: Option<f64>| value.map_or_else(|| "null".to_owned(), |v| format!("{v:?}"));
        let optics = row.optics();
        rows.push(format!(
            "    {{\n      \"index\": {},\n      \"key\": \"{}\",\n      \"kind\": \"{}\",\n      \
             \"molar_mass_g_per_mol\": {},\n      \"heat_capacity_over_r\": {},\n      \
             \"rayleigh\": {},\n      \"absorber\": {},\n      \"aerosol_material\": {}\n    }}",
            id.index(),
            row.key(),
            row.kind().name(),
            number(row.molar_mass_g_per_mol()),
            number(row.gas().and_then(GasColumns::heat_capacity_over_r)),
            optics.rayleigh(),
            optics.absorber(),
            optics.aerosol_material(),
        ));
    }
    format!(
        "{{\n  \"description\": \"The substance registry's public mirror (galaxy plan 14, \
         P14.T49.a; decision-composition 1.1): each row in index order, append-only, with its key, \
         kind, molar mass (g/mol), c_p/R for a gas row and the client optics the sim may ask of \
         it. Written by \
         crates/hyperion-sim/tests/substance_golden.rs under HYPERION_BLESS=1 and checked against \
         the registry otherwise; never edit it by hand.\",\n  \"substances\": [\n{}\n  ]\n}}\n",
        rows.join(",\n")
    )
}

/// The fixture's identities, `index key kind` for each row in order, which only grow.
fn fixture_identities(text: &str) -> Vec<String> {
    let value = |line: &str, name: &str| {
        line.trim()
            .strip_prefix(&format!("\"{name}\": "))
            .map(|rest| rest.trim_end_matches(',').trim_matches('"').to_owned())
    };
    let mut identities = Vec::new();
    let mut current = Vec::new();
    for line in text.lines() {
        for name in ["index", "key", "kind"] {
            if let Some(v) = value(line, name) {
                current.push(v);
            }
        }
        if current.len() == 3 {
            identities.push(current.join(" "));
            current.clear();
        }
    }
    identities
}

#[test]
fn substance_fixture_mirrors_the_registry() {
    let text = fixture();
    let path = repository().join("packages/protocol/fixtures/substances.json");
    let old = committed(&path);
    if let Some(old) = &old {
        let rows = old
            .lines()
            .filter(|line| line.trim().starts_with("\"index\": "))
            .count();
        assert_eq!(
            fixture_identities(old).len(),
            rows,
            "the committed fixture's every row reads as an identity, so none escapes the check"
        );
        assert_append_only(
            "substances.json",
            &fixture_identities(old),
            &fixture_identities(&text),
        );
    }
    assert_eq!(
        fixture_identities(&text).len(),
        substance::ids().len(),
        "every row has its identity in the fixture"
    );
    match Mode::from_env() {
        Mode::Compare => {
            let old =
                old.unwrap_or_else(|| panic!("cannot read {}; run `just bless`", path.display()));
            assert_eq!(
                old,
                text,
                "{} differs from the registry; run `just bless` and commit it",
                path.display()
            );
        }
        Mode::Bless => std::fs::write(&path, &text)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display())),
        Mode::BlessForbidden => panic!(
            "refusing to write {} under CI: the fixture is blessed on a developer machine",
            path.display()
        ),
    }
}
