//! The substance registry's golden and its public mirror (plan 14, P14.T49.a; decision-composition
//! §1.1).
//!
//! - `tests/golden/substance/registry.golden` pins every row's values by their bits, at the
//!   generator version: the elements' order, each row's identity (index, key, kind, charge,
//!   stoichiometry and molar mass), its client optics, its gas columns and its phase columns. A
//!   changed value is moved output, which bumps the version.
//! - `packages/protocol/fixtures/substances.json` mirrors the rows for the client (index, key,
//!   kind, molar mass, `c_p` ÷ R and the three optics flags), which rendering plan R08's parity
//!   check and P14.T35.e's decode tests read. `just bless` (`HYPERION_BLESS=1`) writes it, and
//!   otherwise this test holds it to the registry byte for byte.
//!
//! Both are append-only in their identities: a row or an element is appended, never renumbered,
//! renamed or re-kinded. Each test reads the committed file before it compares or blesses, and
//! fails unless the committed identities are a prefix of the registry's, so that not even a bless
//! can change a key a build has sent.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::{self, Element, GasColumns, Substance};
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, Mode};

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
    w.line("[gas]");
    for id in substance::ids() {
        let row = id.substance();
        if let Some(gas) = row.gas() {
            let label = format!("gas {} {}", id.index(), row.key());
            w.line(&format!("{label}: {:?}", gas.heat_capacity()));
            w.f64(
                &format!("{label}: heat_capacity_over_r"),
                gas.heat_capacity_over_r(),
            );
        }
    }
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
fn the_registry_is_pinned_and_append_only() {
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
            number(row.gas().map(GasColumns::heat_capacity_over_r)),
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
fn the_fixture_mirrors_the_registry() {
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
