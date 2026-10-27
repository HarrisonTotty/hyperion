//! The substance registry's golden and its public mirror (plan 14, P14.T49.a; decision-composition
//! §1.1).
//!
//! - `tests/golden/substance/registry.golden` pins every row's values by their bits, at the
//!   generator version: the elements' order, each row's identity (index, key, kind, charge,
//!   stoichiometry and molar mass), its client optics, its gas columns (the molecular constants,
//!   and `c_p` ÷ R, H̃ and S̃ at fixed temperatures; P14.T49.b, and the Rayleigh cross section at
//!   550 nm, P14.T24.h.a), its phase
//!   diagram (its points, anchors, curve, densities and enthalpies at fixed fractions of its triple
//!   and critical temperatures; P14.T49.b), and its condensed columns (grain density, transition
//!   and mechanics family; P14.T49.b), and its condensed optics (each modelled row's reference
//!   form, index file, phase law and fitted albedos; P14.T49.c). A changed value is moved output,
//!   which bumps the version. The record reads the diagram section since P14.T54.a's switch,
//!   which retired the held phase columns and their `[phase]` section; until P14.T54.a's bump
//!   nothing on the record reads the condensed and albedo sections, which re-bless at the version
//!   in force.
//! - `packages/protocol/fixtures/substances.json` mirrors the rows for the client (index, key,
//!   kind, molar mass, `heat_capacity_over_r_298k`, the three optics flags with a gas row's
//!   Rayleigh cross section at 550 nm after its `rayleigh` flag (P14.T24.h.a), and the grain
//!   density's `grain_density_variant`, `solid_grain_density` and `liquid_grain_density`), which
//!   rendering plan R08's parity check and P14.T35.e's decode tests read. `just bless`
//!   (`HYPERION_BLESS=1`) writes it, and otherwise this test holds it to the registry byte for
//!   byte. Its `heat_capacity_over_r_298k` is a gas row's statistical sum at 298.15 K, `null`
//!   without gas columns, for display and cross-check only (science-r08-gases-2.md §1.4;
//!   P14.T54.a). Its grain densities are tables of (T, ρ) nodes, linear in T, held to 10⁻⁴ of
//!   `Substance::grain_density` (decision-p14-t54a-grain-density §2): the writer is
//!   [`fixture`], the reader [`read_grain_densities`] and [`mirror_density`], and their test
//!   `substance_fixture_grain_density_holds_the_registry`. The client's reader is
//!   `apps/hyperion/src/renderer/src/view/atmosphere/grainDensity.ts`. Prettier leaves the file
//!   alone (`.prettierignore`), so that its pieces keep a line each.
//!
//! Both are append-only in their identities: a row or an element is appended, never renumbered,
//! renamed or re-kinded. Each test reads the committed file before it compares or blesses, and
//! fails unless the committed identities are a prefix of the registry's, so that not even a bless
//! can change a key a build has sent.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::aerosol::{self, FileVariant};
use hyperion_sim::substance::optics::{
    EndMember, GrainScaling, LabSpectrum, OpticsInput, PartEndMember,
};
use hyperion_sim::substance::rayleigh::rayleigh_cross_section;
use hyperion_sim::substance::{
    self, CondensedColumns, CondensedPhase, EQUILIBRIUM_HYDROGEN, Element, GrainDensity, LevelSet,
    MolecularConstants, PhaseDiagram, SolidDensity, Substance, TransitionKind,
};
use hyperion_sim::units::{Kelvin, KilogramsPerCubicMetre, Pascals};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, Mode, f64_digest};

/// The temperatures, kelvin, at which the golden pins each gas row's `c_p` ÷ R: Triton's to a
/// near-runaway world's.
const PINNED_TEMPERATURES: [f64; 7] = [30.0, 100.0, 200.0, 298.15, 500.0, 1_000.0, 2_000.0];

/// The temperature, kelvin, at which the golden pins each gas row's H̃ and S̃, and the fixture
/// gives its `c_p` ÷ R.
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

/// The golden's gas section: each gas row's molecular constants, and its `c_p` ÷ R at
/// [`PINNED_TEMPERATURES`] and H̃ and S̃ at [`STANDARD_TEMPERATURE`] (P14.T49.b).
fn gas_section(w: &mut GoldenWriter) {
    w.line("[gas]");
    for id in substance::ids() {
        let row = id.substance();
        let Some(gas) = row.gas() else {
            continue;
        };
        let label = format!("gas {} {}", id.index(), row.key());
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
        f64_or_none(
            w,
            &format!("{label}: rayleigh_cross_section_550nm_m2"),
            rayleigh_cross_section(id).map(|row| row.cross_section().value()),
        );
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
/// pressure, enthalpies and boiling point under one atmosphere (P14.T49.b), and the partition's cap
/// ([`cap_lines`]).
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
        cap_lines(w, &label, d, t_c);
    }
}

/// The partition's cap above the critical point `t_c`, K, of the diagram `d` labelled `label`: its
/// line's slope and its value at 1, 1.5 and 2 `T_c` (P14.T53.b part 3, a pure function of the row
/// that its hold reads).
fn cap_lines(w: &mut GoldenWriter, label: &str, d: &PhaseDiagram, t_c: f64) {
    f64_or_none(
        w,
        &format!("{label}: cap_slope_pa_per_k"),
        d.cap_slope_pa_per_k(),
    );
    for (name, t) in [("T_c", t_c), ("1.5 T_c", 1.5 * t_c), ("2 T_c", 2.0 * t_c)] {
        w.f64(
            &format!("{label}: condensation_cap_pa at {name}"),
            d.condensation_cap(Kelvin::new(t)).value(),
        );
    }
}

/// The golden's condensed section: each row's grain density (a fixed row's solid's and liquid's,
/// each with its temperature or none; a liquid by temperature's solid with its temperature and
/// its liquid's nodes, from generator version 24, P14.T49.n; a diagram row's solid at 0.9 of its
/// triple temperature and its saturated liquid at it), its transition with its kind (a eutectic's
/// composition too) and its mechanics family (P14.T49.b).
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
            GrainDensity::MeasuredLiquid { solid, liquid } => {
                w.line(&format!(
                    "{label}: grain density fixed solid, liquid by temperature"
                ));
                w.f64(
                    &format!("{label}: solid grain_density_kg_per_m3"),
                    solid.density().value(),
                );
                w.f64(
                    &format!("{label}: solid grain_density_at_k"),
                    solid.at().value(),
                );
                for (i, (at, density)) in liquid.iter().enumerate() {
                    w.f64(&format!("{label}: liquid node {i} at_k"), at.value());
                    w.f64(
                        &format!("{label}: liquid node {i} grain_density_kg_per_m3"),
                        density.value(),
                    );
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
                match e {
                    PartEndMember::Lab(e) => end_member(w, &part_label, e),
                    PartEndMember::Opaque(e) => w.line(&format!(
                        "{part_label}: {} by the opaque slab of {}",
                        e.name(),
                        e.file()
                    )),
                }
            }
        }
        OpticsInput::Levels { levels } => {
            let g = levels.geometry();
            w.line(&format!(
                "{label}: levels at i {} e {} azimuth {} deg",
                g.incidence_deg(),
                g.emission_deg(),
                g.azimuth_deg()
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

/// The relative error within which the fixture's nodes are placed, so that linear interpolation
/// between them holds [`GRAIN_DENSITY_TOLERANCE`] at every temperature the test samples
/// (decision-p14-t54a-grain-density §2.4).
const GRAIN_DENSITY_PLACEMENT_TOLERANCE: f64 = 5e-5;

/// The relative error within which the fixture's grain density holds `Substance::grain_density`
/// wherever both give one (decision-p14-t54a-grain-density §2.4).
const GRAIN_DENSITY_TOLERANCE: f64 = 1e-4;

/// The grid, nodes a kelvin, on which a piece's interior nodes lie: 0.01 K, for readability.
const GRAIN_DENSITY_GRID_PER_K: f64 = 100.0;

/// The interior points of an interval at which the placement checks its chord.
const PLACEMENT_SAMPLES: u32 = 128;

/// The interior points of an interval at which the test checks the interpolation.
const TEST_SAMPLES: u32 = 64;

/// The fraction of the critical temperature to which the fixture holds a diagram row's liquid:
/// above it, no table of nodes on the 0.01 K grid ([`GRAIN_DENSITY_GRID_PER_K`]) holds the
/// saturated liquid's approach to the critical density within 10⁻⁴
/// (decision-p14-t54a-grain-density §2.3).
const LIQUID_CUT: f64 = 0.99;

/// One piece of a phase's grain-density table in the fixture: the temperatures it covers, kelvin,
/// `None` for no bound, and its nodes, linear in temperature between them and held beyond them.
#[derive(Debug, Clone, PartialEq)]
struct Piece {
    from_k: Option<f64>,
    to_k: Option<f64>,
    temperatures_k: Vec<f64>,
    densities_kg_per_m3: Vec<f64>,
}

/// The linear interpolation between the nodes (`t0`, `r0`) and (`t1`, `r1`) at `t`, as the
/// fixture's readers evaluate it: r0 + f (r1 − r0), f = (t − t0) ÷ (t1 − t0).
fn interpolate(t0: f64, r0: f64, t1: f64, r1: f64, t: f64) -> f64 {
    let f = (t - t0) / (t1 - t0);
    r0 + f * (r1 - r0)
}

/// Whether the chord from (`t0`, `law(t0)`) to (`t1`, `law(t1)`) holds `law` within
/// [`GRAIN_DENSITY_PLACEMENT_TOLERANCE`] at [`PLACEMENT_SAMPLES`] points inside it.
fn chord_holds(law: &dyn Fn(f64) -> f64, t0: f64, t1: f64) -> bool {
    let (r0, r1) = (law(t0), law(t1));
    let n = f64::from(PLACEMENT_SAMPLES + 1);
    (1..=PLACEMENT_SAMPLES).all(|j| {
        let t = t0 + (t1 - t0) * f64::from(j) / n;
        let exact = law(t);
        (interpolate(t0, r0, t1, r1, t) - exact).abs() <= GRAIN_DENSITY_PLACEMENT_TOLERANCE * exact
    })
}

/// The nodes' temperatures of a piece of `law` from `from` to `to`, kelvin: those two, every kink
/// of the law between them (`kinks`, rising), and between each pair as few interior nodes as
/// hold linear interpolation within [`GRAIN_DENSITY_PLACEMENT_TOLERANCE`].
///
/// The interior nodes lie on a grid of [`GRAIN_DENSITY_GRID_PER_K`] a kelvin. Each is the farthest
/// grid point from the last node to which the chord holds, found by bisection on the grid. The
/// placement uses only the law, the four arithmetic operations, `floor`, `ceil`, `f64::midpoint`
/// and comparisons (and `next_down` for a piece's end before a gap, [`grain_density_pieces`]), each
/// the same to the bit on every platform, whose `powf`, `exp` and `ln` differ.
fn place_nodes(law: &dyn Fn(f64) -> f64, from: f64, to: f64, kinks: &[f64]) -> Vec<f64> {
    let grid = |k: f64| k / GRAIN_DENSITY_GRID_PER_K;
    let mut stops = vec![from];
    stops.extend(kinks.iter().copied().filter(|&k| from < k && k < to));
    stops.push(to);
    let mut nodes = vec![from];
    for pair in stops.windows(2) {
        let (mut current, end) = (pair[0], pair[1]);
        while !chord_holds(law, current, end) {
            // The grid points strictly between the last node and the stop.
            let mut first = (current * GRAIN_DENSITY_GRID_PER_K).floor() + 1.0;
            while grid(first) <= current {
                first += 1.0;
            }
            let mut last = (end * GRAIN_DENSITY_GRID_PER_K).ceil() - 1.0;
            while grid(last) >= end {
                last -= 1.0;
            }
            if first > last {
                break;
            }
            // The chord holds to `held` and fails to `failed`, one past the last grid point
            // standing for the stop.
            let (mut held, mut failed) = (first, last + 1.0);
            if chord_holds(law, current, grid(first)) {
                while failed - held > 1.0 {
                    let middle = f64::midpoint(held, failed).floor();
                    if chord_holds(law, current, grid(middle)) {
                        held = middle;
                    } else {
                        failed = middle;
                    }
                }
            }
            current = grid(held);
            nodes.push(current);
        }
        nodes.push(end);
    }
    nodes
}

/// A piece of `law` from `from` to `to`, covering `[from_k, to_k)` (or to `to_k` for its member's
/// last piece), with its nodes placed by [`place_nodes`].
fn law_piece(law: &dyn Fn(f64) -> f64, from: f64, to: f64, kinks: &[f64]) -> Piece {
    let temperatures_k = place_nodes(law, from, to, kinks);
    let densities_kg_per_m3 = temperatures_k.iter().map(|&t| law(t)).collect();
    Piece {
        from_k: Some(from),
        to_k: Some(to),
        temperatures_k,
        densities_kg_per_m3,
    }
}

/// The kinks of a solid phase's density law: a polynomial's held end and a measured law's nodes.
fn kinks(law: &SolidDensity) -> Vec<f64> {
    match *law {
        SolidDensity::Polynomial { held_below, .. } => vec![held_below.value()],
        SolidDensity::Measured(points) => points.iter().map(|(t, _)| t.value()).collect(),
        SolidDensity::IceIh | SolidDensity::CellVolume { .. } | SolidDensity::Fixed { .. } => {
            Vec::new()
        }
    }
}

/// The fixture's grain-density pieces of `row` as `phase` (decision-p14-t54a-grain-density §2.3),
/// or `None` where it has none at any temperature.
///
/// - A [`GrainDensity::Fixed`] row's phase that was read is one piece with no bounds and one node
///   at its stated temperature, held at every temperature, as the sim holds it.
/// - A [`GrainDensity::MeasuredLiquid`] row's solid is such a piece, and its liquid one piece with
///   no bounds and the row's own nodes, which the fixture's rule reads exactly as the sim does
///   (linear between them, held beyond them).
/// - A [`GrainDensity::OfDiagram`] row's solid is a piece for each solid phase with a density,
///   from its start to the next phase's start, or to the triple temperature for the warmest; a
///   phase with no density is a gap. A last piece that ends where such a gap begins ends at the
///   double below the gap's start, so that the start itself reads none, as the sim's phases begin
///   at their starts (carbon monoxide's α, before β-CO at 61.585 K).
/// - Its liquid is one piece from the triple temperature to [`LIQUID_CUT`] of the critical
///   temperature.
fn grain_density_pieces(row: &Substance, phase: CondensedPhase) -> Option<Vec<Piece>> {
    let pieces = match row.condensed()?.grain_density() {
        GrainDensity::Fixed { solid, liquid } => {
            let given = match phase {
                CondensedPhase::Solid => solid,
                CondensedPhase::Liquid => liquid,
            }?;
            vec![Piece {
                from_k: None,
                to_k: None,
                temperatures_k: vec![given.at().value()],
                densities_kg_per_m3: vec![given.density().value()],
            }]
        }
        GrainDensity::MeasuredLiquid { solid, liquid } => match phase {
            CondensedPhase::Solid => vec![Piece {
                from_k: None,
                to_k: None,
                temperatures_k: vec![solid.at().value()],
                densities_kg_per_m3: vec![solid.density().value()],
            }],
            CondensedPhase::Liquid => vec![Piece {
                from_k: None,
                to_k: None,
                temperatures_k: liquid.iter().map(|(at, _)| at.value()).collect(),
                densities_kg_per_m3: liquid.iter().map(|(_, rho)| rho.value()).collect(),
            }],
        },
        GrainDensity::OfDiagram => {
            let diagram = row
                .phase_diagram()
                .expect("a diagram row has a phase diagram");
            let triple = diagram.triple_temperature().value();
            match phase {
                CondensedPhase::Solid => {
                    let solids = diagram.solid_phases();
                    let mut pieces = Vec::new();
                    for (i, solid) in solids.iter().enumerate() {
                        let Some(law) = solid.density() else {
                            continue;
                        };
                        let from = solid.start().value();
                        let mut to = solids
                            .get(i + 1)
                            .map_or(triple, |next| next.start().value());
                        if to < triple
                            && solids[i + 1..]
                                .iter()
                                .all(|later| later.density().is_none())
                        {
                            to = to.next_down();
                        }
                        let at = |t: f64| law.at(Kelvin::new(t)).value();
                        pieces.push(law_piece(&at, from, to, &kinks(law)));
                    }
                    pieces
                }
                CondensedPhase::Liquid => {
                    let cut = LIQUID_CUT * diagram.critical_temperature().value();
                    let at = |t: f64| {
                        row.grain_density(CondensedPhase::Liquid, Kelvin::new(t))
                            .expect("the saturated liquid has a density from T_t to T_c")
                            .value()
                    };
                    vec![law_piece(&at, triple, cut, &[])]
                }
            }
        }
    };
    (!pieces.is_empty()).then_some(pieces)
}

/// `value` as the fixture writes a number, to the bit, or `null`.
fn json_number(value: Option<f64>) -> String {
    value.map_or_else(|| "null".to_owned(), |v| format!("{v:?}"))
}

/// The fixture's text of a phase's pieces: `null`, or the pieces one a line.
fn pieces_json(pieces: Option<&[Piece]>) -> String {
    let Some(pieces) = pieces else {
        return "null".to_owned();
    };
    let list = |values: &[f64]| {
        values
            .iter()
            .map(|v| format!("{v:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let lines: Vec<String> = pieces
        .iter()
        .map(|piece| {
            format!(
                "        {{ \"from_k\": {}, \"to_k\": {}, \"temperatures_k\": [{}], \
                 \"densities_kg_per_m3\": [{}] }}",
                json_number(piece.from_k),
                json_number(piece.to_k),
                list(&piece.temperatures_k),
                list(&piece.densities_kg_per_m3),
            )
        })
        .collect();
    format!("[\n{}\n      ]", lines.join(",\n"))
}

/// The fixture's description: what it holds, and how a reader evaluates its grain density
/// (decision-p14-t54a-grain-density §2.2).
const FIXTURE_DESCRIPTION: &str = "The substance registry's public mirror (galaxy plan 14, \
     P14.T49.a and P14.T54.a; decision-composition 1.1; decision-p14-t54a-grain-density 2): each \
     row in index order, append-only, with its key, kind, molar mass (g/mol), \
     heat_capacity_over_r_298k (a gas row's ideal-gas c_p/R at 298.15 K, the sum over its \
     molecular constants, for display and cross-check only; null for a row with no gas columns), \
     the client optics the sim may ask of it, and its grain density without pore space. \
     rayleigh_cross_section_550nm_m2 is a gas row's Rayleigh scattering cross-section per molecule \
     at 550 nm in vacuum (m2; P14.T24.h.a), from its measured refractivity at its formula's \
     reference state, reduced with the real gas's number density there, and its King factor; null \
     where the registry holds no measured dispersion, so that the sim's Rayleigh floor counts no \
     scattering by it (no optics, not zero optics). The client's rayleigh.ts holds it within 1e-3 \
     (relative). \
     grain_density_variant names the index-file variant whose composition the densities are \
     (null: the substance itself). solid_grain_density and liquid_grain_density are each null, \
     where none was read at any temperature, or pieces of nodes: temperatures_k (K) strictly \
     rising, densities_kg_per_m3 (kg/m3) finite and positive, the two of equal length, at least \
     one. To evaluate a phase's density at T: if its member is null there is none; otherwise take \
     the piece with from_k <= T < to_k, the member's last piece also covering T = to_k and a null \
     bound being unbounded, and if no piece covers T there is none; within the piece the density \
     is the first node's at or below it, the last node's above it, and between the nodes T0 < T \
     <= T1 it is rho0 + f (rho1 - rho0) with f = (T - T0) / (T1 - T0). It holds the sim's \
     Substance::grain_density within 1e-4 (relative) wherever both give one, each node to the bit. \
     A row with a phase diagram gives each solid phase's density with a piece of its own, a phase \
     with none being a gap (a last piece that ends where one begins ends at the double below its \
     start, so that the start reads none), and its saturated liquid from the triple temperature \
     to 0.99 of the critical temperature only: the sim gives one above it too, to the critical \
     density, which no linear table follows. Any other row holds each phase that was read at its \
     stated temperature, one node held at every temperature. Written by \
     crates/hyperion-sim/tests/substance_golden.rs under HYPERION_BLESS=1 and checked against the \
     registry otherwise; never edit it by hand.";

/// The fixture's text: one object a row, a value a line, and a grain-density piece a line.
fn fixture() -> String {
    let mut rows = Vec::new();
    for id in substance::ids() {
        let row = id.substance();
        let optics = row.optics();
        let variant = match aerosol::density_variant(row.key().as_str()) {
            FileVariant::Substance => "null".to_owned(),
            FileVariant::Named(variant) => format!("\"{variant}\""),
        };
        let solid = grain_density_pieces(row, CondensedPhase::Solid);
        let liquid = grain_density_pieces(row, CondensedPhase::Liquid);
        rows.push(format!(
            "    {{\n      \"index\": {},\n      \"key\": \"{}\",\n      \"kind\": \"{}\",\n      \
             \"molar_mass_g_per_mol\": {},\n      \"heat_capacity_over_r_298k\": {},\n      \
             \"rayleigh\": {},\n      \"rayleigh_cross_section_550nm_m2\": {},\n      \"absorber\": {},\n      \
             \"aerosol_material\": {},\n      \
             \"grain_density_variant\": {variant},\n      \"solid_grain_density\": {},\n      \
             \"liquid_grain_density\": {}\n    }}",
            id.index(),
            row.key(),
            row.kind().name(),
            json_number(row.molar_mass_g_per_mol()),
            json_number(substance::heat_capacity_over_r_at(
                id,
                Kelvin::new(STANDARD_TEMPERATURE)
            )),
            optics.rayleigh(),
            json_number(rayleigh_cross_section(id).map(|row| row.cross_section().value())),
            optics.absorber(),
            optics.aerosol_material(),
            pieces_json(solid.as_deref()),
            pieces_json(liquid.as_deref()),
        ));
    }
    format!(
        "{{\n  \"description\": \"{FIXTURE_DESCRIPTION}\",\n  \"substances\": [\n{}\n  ]\n}}\n",
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

/// A fixture row's grain density as the test reads it back from the fixture's text.
#[derive(Debug, Clone, PartialEq)]
struct MirrorRow {
    key: String,
    variant: Option<String>,
    solid: Option<Vec<Piece>>,
    liquid: Option<Vec<Piece>>,
}

impl MirrorRow {
    /// The member of `phase`.
    fn member(&self, phase: CondensedPhase) -> Option<&[Piece]> {
        match phase {
            CondensedPhase::Solid => self.solid.as_deref(),
            CondensedPhase::Liquid => self.liquid.as_deref(),
        }
    }
}

/// The number the fixture writes as `text`, or `None` for `null`.
fn read_number(text: &str) -> Option<f64> {
    let text = text.trim();
    (text != "null").then(|| {
        text.parse()
            .unwrap_or_else(|e| panic!("{text:?} is no number: {e}"))
    })
}

/// The text of the member `name` in a piece's line `line`, up to its end `end`.
fn member_text<'a>(line: &'a str, name: &str, end: char) -> &'a str {
    let start = format!("\"{name}\": ");
    let rest = &line[line
        .find(&start)
        .unwrap_or_else(|| panic!("a piece names {name}: {line}"))
        + start.len()..];
    &rest[..rest
        .find(end)
        .unwrap_or_else(|| panic!("{name} ends: {line}"))]
}

/// A piece the fixture writes on the line `line`.
fn read_piece(line: &str) -> Piece {
    let numbers = |name: &str| -> Vec<f64> {
        member_text(line, name, ']')
            .trim_start_matches('[')
            .split(", ")
            .map(|v| read_number(v).unwrap_or_else(|| panic!("a node is a number: {line}")))
            .collect()
    };
    Piece {
        from_k: read_number(member_text(line, "from_k", ',')),
        to_k: read_number(member_text(line, "to_k", ',')),
        temperatures_k: numbers("temperatures_k"),
        densities_kg_per_m3: numbers("densities_kg_per_m3"),
    }
}

/// Every row's grain density as the fixture's text `text` writes it, in its order.
fn read_grain_densities(text: &str) -> Vec<MirrorRow> {
    let mut rows: Vec<MirrorRow> = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        let value = |name: &str| {
            line.strip_prefix(&format!("\"{name}\": "))
                .map(|rest| rest.trim_end_matches(','))
        };
        if let Some(key) = value("key") {
            rows.push(MirrorRow {
                key: key.trim_matches('"').to_owned(),
                variant: None,
                solid: None,
                liquid: None,
            });
            continue;
        }
        if let Some(variant) = value("grain_density_variant") {
            let row = rows.last_mut().expect("a row's key comes first");
            row.variant = (variant != "null").then(|| variant.trim_matches('"').to_owned());
            continue;
        }
        for (name, solid) in [
            ("solid_grain_density", true),
            ("liquid_grain_density", false),
        ] {
            let Some(rest) = value(name) else {
                continue;
            };
            let pieces = (rest == "[").then(|| {
                lines
                    .by_ref()
                    .map(str::trim)
                    .take_while(|line| !line.starts_with(']'))
                    .map(read_piece)
                    .collect::<Vec<_>>()
            });
            assert!(pieces.is_some() || rest == "null", "{name}: {line}");
            let row = rows.last_mut().expect("a row's key comes first");
            if solid {
                row.solid = pieces;
            } else {
                row.liquid = pieces;
            }
        }
    }
    rows
}

/// The fixture's grain density of a member `pieces` at `t`, kelvin, by its description's rule
/// (decision-p14-t54a-grain-density §2.2): the piece with `from_k ≤ t < to_k` (the last piece's
/// `to_k` too, a missing bound unbounded), linear between its nodes and held beyond them; `None`
/// for no member or no piece.
fn mirror_density(pieces: Option<&[Piece]>, t: f64) -> Option<f64> {
    let pieces = pieces?;
    let last = pieces.len() - 1;
    let piece = pieces.iter().enumerate().find_map(|(i, piece)| {
        let above = piece.from_k.is_none_or(|from| from <= t);
        let below = piece.to_k.is_none_or(|to| t < to || (i == last && t <= to));
        (above && below).then_some(piece)
    })?;
    let (ts, rs) = (&piece.temperatures_k, &piece.densities_kg_per_m3);
    if t <= ts[0] {
        return Some(rs[0]);
    }
    Some(
        ts.windows(2)
            .zip(rs.windows(2))
            .find(|(span, _)| t <= span[1])
            .map_or(rs[rs.len() - 1], |(span, rho)| {
                interpolate(span[0], rho[0], span[1], rho[1], t)
            }),
    )
}

/// The temperatures, kelvin, at which [`substance_fixture_grain_density_holds_the_registry`]
/// compares the fixture's grain density of `pieces` with the registry's: [`TEST_SAMPLES`] points
/// inside each interval between nodes, each node, and each bound and 10⁻⁹ K either side of it.
fn piece_samples(pieces: &[Piece]) -> Vec<f64> {
    let mut samples = Vec::new();
    let n = f64::from(TEST_SAMPLES + 1);
    for piece in pieces {
        let ts = &piece.temperatures_k;
        samples.extend(ts);
        for span in ts.windows(2) {
            samples.extend(
                (1..=TEST_SAMPLES).map(|j| span[0] + (span[1] - span[0]) * f64::from(j) / n),
            );
        }
        for bound in [piece.from_k, piece.to_k].into_iter().flatten() {
            samples.extend([bound - 1e-9, bound, bound + 1e-9]);
        }
    }
    samples
}

/// `count` + 1 temperatures evenly from `low` to `high`, kelvin.
fn sweep(low: f64, high: f64, count: u32) -> impl Iterator<Item = f64> {
    (0..=count).map(move |j| low + (high - low) * f64::from(j) / f64::from(count))
}

/// The temperatures, kelvin, at which a diagram row's or a fixed row's `phase` is compared beyond
/// its pieces' own samples: a diagram's solid phases' starts, its triple and critical temperatures
/// and 10⁻⁹ K either side of each, and a sweep of the solid over [0, 1.05 `T_t`] or of the liquid
/// over [0.9 `T_t`, `T_c`]; a fixed row's 0 K, 300 K, 3,000 K and its stated temperature; a row
/// with a liquid by temperature the same for its solid, and for its liquid 0 K, 300 K, 3,000 K,
/// its nodes and 10⁻⁹ K either side of each, and a sweep from 0.9 of its first node to 1.1 of its
/// last.
fn row_samples(row: &Substance, phase: CondensedPhase) -> Vec<f64> {
    let mut samples = Vec::new();
    match row.condensed().map(CondensedColumns::grain_density) {
        Some(GrainDensity::OfDiagram) => {
            let d = row
                .phase_diagram()
                .expect("a diagram row has a phase diagram");
            let (t_t, t_c) = (
                d.triple_temperature().value(),
                d.critical_temperature().value(),
            );
            let mut ends = vec![t_t, t_c];
            ends.extend(d.solid_phases().iter().map(|s| s.start().value()));
            for end in ends {
                samples.extend([end - 1e-9, end, end + 1e-9]);
            }
            match phase {
                CondensedPhase::Solid => samples.extend(sweep(0.0, 1.05 * t_t, 4_096)),
                CondensedPhase::Liquid => samples.extend(sweep(0.9 * t_t, t_c, 4_096)),
            }
        }
        Some(GrainDensity::Fixed { solid, liquid }) => {
            samples.extend([0.0, 300.0, 3_000.0]);
            let given = match phase {
                CondensedPhase::Solid => solid,
                CondensedPhase::Liquid => liquid,
            };
            samples.extend(given.map(|given| given.at().value()));
        }
        Some(GrainDensity::MeasuredLiquid { solid, liquid }) => {
            samples.extend([0.0, 300.0, 3_000.0]);
            match phase {
                CondensedPhase::Solid => samples.push(solid.at().value()),
                CondensedPhase::Liquid => {
                    for (at, _) in liquid {
                        let at = at.value();
                        samples.extend([at - 1e-9, at, at + 1e-9]);
                    }
                    if let (Some(first), Some(last)) = (liquid.first(), liquid.last()) {
                        let (first, last) = (first.0.value(), last.0.value());
                        samples.extend(sweep(0.9 * first, 1.1 * last, 4_096));
                    }
                }
            }
        }
        None => samples.extend([0.0, 300.0, 3_000.0]),
    }
    samples
}

/// Asserts that the fixture's `phase` of `row`, read back as `pieces`, and
/// `Substance::grain_density` give a density at the same temperatures of `samples`, within 10⁻⁴,
/// a fixed row's to the bit: the one difference a diagram row's liquid above [`LIQUID_CUT`] of its
/// critical temperature, where the fixture gives none.
fn compare_samples(
    row: &Substance,
    phase: CondensedPhase,
    pieces: Option<&[Piece]>,
    samples: &[f64],
) {
    let key = row.key();
    let diagram = match row.condensed().map(CondensedColumns::grain_density) {
        Some(GrainDensity::OfDiagram) => row.phase_diagram(),
        Some(GrainDensity::Fixed { .. } | GrainDensity::MeasuredLiquid { .. }) | None => None,
    };
    for &t in samples {
        let sim = row
            .grain_density(phase, Kelvin::new(t))
            .map(KilogramsPerCubicMetre::value);
        match (mirror_density(pieces, t), sim) {
            (Some(m), Some(s)) => {
                let error = (m - s).abs() / s;
                assert!(
                    error <= GRAIN_DENSITY_TOLERANCE,
                    "{key} {phase:?} at {t} K: {m} against {s}"
                );
                if diagram.is_none() {
                    assert_same_bits(m, s);
                }
            }
            (None, None) => {}
            (None, Some(_)) => assert!(
                phase == CondensedPhase::Liquid
                    && diagram.is_some_and(|d| {
                        let t_c = d.critical_temperature().value();
                        t > LIQUID_CUT * t_c && t <= t_c
                    }),
                "{key} {phase:?} at {t} K: the sim gives a density and the fixture none"
            ),
            (Some(m), None) => {
                panic!("{key} {phase:?} at {t} K: the fixture gives {m} and the sim none")
            }
        }
    }
}

/// Asserts the shape of `row`'s `pieces` as `phase` (decision-p14-t54a-grain-density §2.1): nodes
/// strictly rising, densities finite and positive, a bounded piece's ends its first and last
/// nodes, the pieces rising and apart; and that each node is its law's value to the bit, the
/// phase's own at the bound it shares with the next phase, where `grain_density` reads the next
/// phase's.
fn check_pieces(row: &Substance, phase: CondensedPhase, pieces: &[Piece]) {
    let key = row.key();
    let diagram = match row.condensed().map(CondensedColumns::grain_density) {
        Some(GrainDensity::OfDiagram) => row.phase_diagram(),
        Some(GrainDensity::Fixed { .. } | GrainDensity::MeasuredLiquid { .. }) | None => None,
    };
    for (i, piece) in pieces.iter().enumerate() {
        let (ts, rs) = (&piece.temperatures_k, &piece.densities_kg_per_m3);
        assert!(
            !ts.is_empty() && ts.len() == rs.len(),
            "{key} {phase:?} piece {i}"
        );
        assert!(
            ts.windows(2).all(|w| w[0] < w[1]),
            "{key} {phase:?} piece {i} rises"
        );
        assert!(
            rs.iter().all(|r| r.is_finite() && *r > 0.0),
            "{key} {phase:?} piece {i}'s densities"
        );
        if let (Some(from), Some(to)) = (piece.from_k, piece.to_k) {
            assert!(from < to, "{key} {phase:?} piece {i}'s bounds");
            assert_same_bits(ts[0], from);
            assert_same_bits(ts[ts.len() - 1], to);
        }
        if let Some(next) = pieces.get(i + 1) {
            let (Some(to), Some(from)) = (piece.to_k, next.from_k) else {
                panic!("{key} {phase:?}: a piece with a later one is bounded");
            };
            assert!(
                to <= from,
                "{key} {phase:?}: pieces {i} and {} overlap",
                i + 1
            );
        }
        for (&t, &r) in ts.iter().zip(rs) {
            let law = match (diagram, phase) {
                (Some(d), CondensedPhase::Solid) => d
                    .solid_phase_at(Kelvin::new(piece.from_k.expect("bounded")))
                    .and_then(|solid| solid.density())
                    .map(|law| law.at(Kelvin::new(t)).value()),
                (Some(_) | None, _) => row
                    .grain_density(phase, Kelvin::new(t))
                    .map(KilogramsPerCubicMetre::value),
            };
            assert_same_bits(r, law.expect("a node has its law's value"));
        }
    }
}

/// Asserts the shapes the ruling names (decision-p14-t54a-grain-density §2.6): O₂'s two solid
/// pieces about the β gap, CO's one, N₂'s and CH₄'s two; no solid for Ar, H₂S and C₂H₆; and the
/// acid's liquid alone, one node of 1,662.6 kg m⁻³ at 300 K held at every temperature, of its
/// `75wt%` variant.
fn check_named_shapes(mirror: &[MirrorRow]) {
    let read = |key: &str| {
        mirror
            .iter()
            .find(|row| row.key == key)
            .unwrap_or_else(|| panic!("{key} is a row"))
    };
    let oxygen = read("O2")
        .solid
        .as_ref()
        .expect("α- and γ-O₂ have densities");
    assert_eq!(oxygen.len(), 2, "β-O₂ is a gap");
    let (Some(alpha_end), Some(gamma_start)) = (oxygen[0].to_k, oxygen[1].from_k) else {
        panic!("O₂'s pieces are bounded: {oxygen:?}");
    };
    assert_same_bits(alpha_end, 23.14);
    assert_same_bits(gamma_start, 43.78);
    for (key, count) in [("CO", 1), ("N2", 2), ("CH4", 2)] {
        assert_eq!(read(key).solid.as_ref().map_or(0, Vec::len), count, "{key}");
    }
    for key in ["Ar", "H2S", "C2H6"] {
        assert_eq!(read(key).solid, None, "{key} has no solid density");
    }
    let acid = read("H2SO4");
    assert_eq!(acid.solid, None);
    assert_eq!(acid.variant.as_deref(), Some("75wt%"));
    let [held] = acid.liquid.as_deref().expect("the acid's liquid was read") else {
        panic!("the acid's liquid is one piece");
    };
    assert_eq!(
        (held.from_k, held.to_k),
        (None, None),
        "held at every temperature"
    );
    let ([t], [rho]) = (&held.temperatures_k[..], &held.densities_kg_per_m3[..]) else {
        panic!("the acid's liquid is one node: {held:?}");
    };
    assert_same_bits(*t, 300.0);
    assert_same_bits(*rho, 1_662.6);
}

/// The fixture's grain density holds the registry's (decision-p14-t54a-grain-density §2.6). The
/// writer's text, read back and evaluated by its description's rule ([`mirror_density`]):
///
/// - gives each node the registry's law to the bit, and `Substance::grain_density` within 10⁻⁴
///   at [`TEST_SAMPLES`] points inside each interval, at each piece bound and 10⁻⁹ K either side
///   of it;
/// - gives a diagram row none exactly where the sim gives none, its solid swept over [0,
///   1.05 `T_t`] and its liquid over [0.9 `T_t`, `T_c`], the one difference the liquid above
///   0.99 `T_c`, where the sim gives one and the fixture none;
/// - gives a fixed row its value at 0 K, at its stated temperature and at 3,000 K;
/// - names each row's variant, and has the shapes the ruling names, each piece's nodes rising and
///   its densities positive.
#[test]
fn substance_fixture_grain_density_holds_the_registry() {
    let mirror = read_grain_densities(&fixture());
    assert_eq!(
        mirror.len(),
        substance::ids().len(),
        "every row is read back"
    );
    for (id, read) in substance::ids().zip(&mirror) {
        let row = id.substance();
        let key = row.key();
        assert_eq!(read.key, key.as_str(), "the rows are in index order");
        let variant = match aerosol::density_variant(key.as_str()) {
            FileVariant::Substance => None,
            FileVariant::Named(variant) => Some(variant.to_owned()),
        };
        assert_eq!(read.variant, variant, "{key}'s variant");
        for phase in [CondensedPhase::Solid, CondensedPhase::Liquid] {
            let pieces = read.member(phase);
            let mut samples = pieces.map(piece_samples).unwrap_or_default();
            samples.extend(row_samples(row, phase));
            compare_samples(row, phase, pieces, &samples);
            if let Some(pieces) = pieces {
                check_pieces(row, phase, pieces);
            }
        }
    }
    check_named_shapes(&mirror);
}

/// The fixture's grain density between two nodes, and a fixed row's, by [`mirror_density`], to
/// the bit: the literals the client's reader is held to (`grainDensity.test.ts`), written as
/// `{:?}` prints them, so that the two evaluators of the rule cannot split silently.
#[test]
fn substance_fixture_grain_density_is_pinned_between_nodes() {
    let mirror = read_grain_densities(&fixture());
    for (key, phase, t, pinned) in [
        // Ice Ih between its nodes at 87.15 and 110.18 K.
        ("H2O", CondensedPhase::Solid, 100.0, 932.996_995_807_952),
        // Liquid water between its nodes at 295.04 and 301.51 K.
        ("H2O", CondensedPhase::Liquid, 300.0, 996.473_720_095_005_1),
        // Basalt's one node, held.
        ("basalt", CondensedPhase::Solid, 150.0, 3_010.0),
    ] {
        let row = mirror
            .iter()
            .find(|row| row.key == key)
            .unwrap_or_else(|| panic!("{key} is a row of the fixture"));
        let density = mirror_density(row.member(phase), t)
            .unwrap_or_else(|| panic!("{key} has a density as {phase:?} at {t} K"));
        assert_same_bits(density, pinned);
    }
}
