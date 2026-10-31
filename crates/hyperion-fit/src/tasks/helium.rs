//! The helium-excess correction of the stellar tracks (plan 06, P06.T17; plan 15, P15.T7): the
//! slope s(m, Z) by which a star's excess of helium ΔY scales the log of its main-sequence and
//! giant-branch timescales, and the shift of its horizontal branch's log₁₀ `T_eff`, which the
//! sim's track reads from [`tables::helium`](hyperion_sim::tables::helium) through its hook
//! (`stellar/sse/track/excess.rs`).
//!
//! **P15.T7.b, the registration.** Plan 06's P06.T17 committed the table by hand as the identity,
//! every coefficient zero, with no fit header, lock entry or `MANIFEST` row. Revision 0 registers
//! it unchanged, which changes no output (plan 15, Design note 9): `manifests/helium.toml`, whose
//! `method` is [`IDENTITY`], emits P06.T17's items verbatim, provisionally.
//!
//! **P15.T7.b's swap, held.** The fit, by [`BASTI_LEAST_SQUARES`] ([`fit`]): least squares on
//! `BaSTI`'s helium-enhanced models (Pietrinferni, Cassisi, Salaris and Castelli 2006, ApJ 642,
//! 797, with the database's helium-enhanced extension; `data/basti_helium/`, reduced by its
//! `reduce.py`) relative to plan 06's tracks, its figures measured on those tracks under the fitted
//! table. It is the task's second table, [`Helium26Task`]'s `helium_26`, from
//! `manifests/helium_26.toml` at revision 1 with the fit's fingerprint, emitted at the version in
//! force: the sim's tracks read it from version 26 on, behind the stellar rules' hold
//! (`HELIUM_VERSION`), since every member of a globular cluster's second population (ΔY > 0, plan
//! 09) moves. The bump to 26 moves `helium_26.toml`'s parameters into `helium.toml`, gives
//! [`HeliumTask`] the second table's revision, items and fingerprint, and deletes [`Helium26Task`]
//! with its table and lock entry.

pub mod cluster;
pub mod figures;
pub mod fit;
pub mod grid;

use std::num::NonZeroUsize;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision: 0, P06.T17's identity registered unchanged.
pub const VERSION: u32 = 0;

/// The second table's revision, [`Helium26Task`]'s: 1, the swap's, with the fit's fingerprint.
pub const FITTED_VERSION: u32 = 1;

/// The manifest's key that names the method.
pub const METHOD_KEY: &str = "method";

/// The method of `manifests/helium.toml`: P06.T17's identity, emitted verbatim.
pub const IDENTITY: &str = "identity";

/// The method of `manifests/helium_26.toml`: the least-squares fit to `BaSTI`'s grid ([`fit`]).
pub const BASTI_LEAST_SQUARES: &str = "basti_least_squares";

/// The table's items, in order.
pub const ITEMS: [&str; 3] = ["LOG_Z_NODES", "LIFETIME_SLOPE", "HB_TEMPERATURE_SHIFT"];

/// The fitted table's items, in order: the identity's, and the envelope mass at which the hook
/// holds the branch's shift (decision-p15-t7b-leans §8).
pub const FITTED_ITEMS: [&str; 4] = [
    "LOG_Z_NODES",
    "LIFETIME_SLOPE",
    "HB_TEMPERATURE_SHIFT",
    "HB_ENVELOPE_MAX",
];

/// The table's summary, P06.T17's.
const SUMMARY: [&str; 3] = [
    "The helium-excess correction of the stellar tracks (plan 06, P06.T17): how much a star's",
    "excess of helium, ΔY, shortens its main sequence and giant branch and moves its horizontal",
    "branch bluewards, which `stellar::sse`'s track reads through `Composition::helium_excess`.",
];

/// The expected signs, which every rendering's notes close with (P06.T17's notes).
const SIGNS: [&str; 3] = [
    "The expected signs (P06.T17): helium burns hydrogen faster, so a helium excess shortens the",
    "main sequence and the giant branch (s < 0), and a helium-rich horizontal-branch star is bluer",
    "at the same luminosity (a positive shift of log₁₀ `T_eff`, larger for a thinner envelope).",
];

/// P06.T17's body, byte for byte: its three items, their doc comments and their values.
///
/// Every coefficient is zero. The identity's run emits it verbatim, so that registering the table
/// moves no byte of it.
pub const IDENTITY_BODY: &str = r"/// The metallicities at which [`LIFETIME_SLOPE`] is given, log₁₀ Z, increasing: \[Fe/H\] = −2.2,
/// −1.6, −1.0 and −0.5 at Z☉ = 0.02, the range plan 15's P15.T7 fits over.
pub const LOG_Z_NODES: [f64; 4] = [
    -3.898_970_004_336_019,
    -3.298_970_004_336_019,
    -2.698_970_004_336_018_7,
    -2.198_970_004_336_018_7,
];

/// The slope s(m, Z) of the log of the main-sequence and giant-branch timescales in ΔY, at each of
/// [`LOG_Z_NODES`]: the coefficients (c₀, c₁, c₂) of s = c₀ + c₁ x + c₂ x², x = log₁₀(m ÷ M☉). Between
/// the nodes s is interpolated linearly in log₁₀ Z, and held at the end nodes outside them. The
/// timescales are multiplied by exp(s × ΔY). All zero: the identity.
pub const LIFETIME_SLOPE: [[f64; 3]; 4] = [[0.0; 3]; 4];

/// The shift of log₁₀ `T_eff` on the horizontal branch per unit ΔY, linear in the envelope mass
/// `M_env` (M☉): (a, b) of a + b `M_env`. The radius changes to keep the luminosity. Both zero: the
/// identity.
pub const HB_TEMPERATURE_SHIFT: [f64; 2] = [0.0; 2];
";

/// The identity's table: P06.T17's summary and body, with notes that say it is registered
/// unchanged and where its fit is.
#[must_use]
pub fn identity_table() -> RustTable {
    let notes = [
        "Provisional (P06.T17): plan 06's identity, every coefficient zero, which P06.T17 committed by",
        "hand and plan 15's P15.T7.b registered with the fitting toolchain unchanged. The track applies",
        "the correction only where ΔY > 0, so every star the grid places (ΔY = 0) is bit-identical",
        "whatever the table holds. P15.T7.b fits the table to the helium-enhanced stellar models of",
        "Pietrinferni et al. (2006) from `manifests/helium.production.toml`, to a scratch path; the",
        "swap moves that fit here, in this shape, with a generator-version bump (members of globular",
        "clusters' second populations have ΔY > 0, plan 09).",
        "",
    ];
    RustTable {
        summary: SUMMARY.iter().map(|&l| l.to_owned()).collect(),
        notes: notes.iter().chain(&SIGNS).map(|&l| l.to_owned()).collect(),
        items: vec![TableItem::Source(IDENTITY_BODY.to_owned())],
    }
}

/// The identity's run: [`identity_table`], provisional by P06.T17. Its manifest holds `method`
/// alone.
fn identity(manifest: &Manifest) -> Result<TaskOutput, RunTaskError> {
    if let Some(extra) = manifest.params().keys().find(|k| *k != METHOD_KEY) {
        return Err(
            ManifestParamError::new(extra, "absent: the identity reads no such key").into(),
        );
    }
    Ok(TaskOutput {
        table: identity_table(),
        source: "plan 06's P06.T17, the identity, with the expected signs: helium burns hydrogen \
                 faster, so a helium excess shortens the main sequence and the giant branch, and \
                 a helium-rich horizontal-branch star is bluer at the same luminosity"
            .to_owned(),
        acceptance: "the identity: every coefficient zero, so no star moves; P15.T7.b's fit to \
                     the helium-enhanced models of Pietrinferni et al. (2006) is held for the swap"
            .to_owned(),
        provisional: Some("P06.T17"),
    })
}

/// The refusal of a manifest whose `method` is not `expected`, the one method a table's task runs,
/// naming where the other method's table is.
#[must_use]
fn other_method(expected: &str, other: &str) -> RunTaskError {
    ManifestParamError::new(METHOD_KEY, &format!("\"{expected}\" ({other})")).into()
}

/// The task: P15.T7.b's registration of P06.T17's identity, fast and provisional. Its fit is
/// [`Helium26Task`]'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct HeliumTask;

impl FitTask for HeliumTask {
    fn name(&self) -> &'static str {
        "helium"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "helium.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// None at revision 0: the identity reads nothing of the generator. The swap's revision 1
    /// gains the probes the fit reads of plan 06's tracks (P15.T7.b's record, "For the swap").
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    /// The identity for `method = "identity"`; any other manifest is refused, the fit's too,
    /// which is `helium_26`'s until the bump to 26.
    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        match manifest.str(METHOD_KEY)? {
            IDENTITY => identity(manifest),
            _ => Err(other_method(
                IDENTITY,
                "P06.T17's table; P15.T7.b's fit is `helium_26`'s",
            )),
        }
    }
}

/// The task's second table, `helium_26`: P15.T7.b's fit, fast, which the sim's tracks read from
/// version 26 on (the stellar rules' hold, `HELIUM_VERSION`), emitted at the version in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Helium26Task;

impl FitTask for Helium26Task {
    fn name(&self) -> &'static str {
        "helium_26"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "helium_26.rs"
    }

    fn revision(&self) -> u32 {
        FITTED_VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &FITTED_ITEMS
    }

    /// The probes the fit reads of the sim (P15.T7.b's record, "For the swap" 4; Design note 7):
    /// [`fit::fingerprint_probes`].
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(fit::fingerprint_probes())
    }

    /// The fit for `method = "basti_least_squares"`; any other manifest is refused.
    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        match manifest.str(METHOD_KEY)? {
            BASTI_LEAST_SQUARES => fit::run(manifest, self.name()),
            _ => Err(other_method(BASTI_LEAST_SQUARES, "P15.T7.b's fit")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// A manifest of `text`.
    fn manifest(text: &str) -> Manifest {
        Manifest::from_bytes(Path::new("helium.toml"), text.as_bytes().to_vec())
            .expect("the test's manifest parses")
    }

    /// The parameter error of a run on `text`.
    fn param_error(text: &str) -> ManifestParamError {
        match HeliumTask.run(&manifest(text), NonZeroUsize::MIN) {
            Err(RunTaskError::Param(e)) => e,
            other => panic!("expected a parameter error, got {other:?}"),
        }
    }

    /// The identity emits P06.T17's body verbatim and declares the three items, provisionally,
    /// and reads nothing of the generator.
    #[test]
    fn the_identity_emits_p06_t17s_body_provisionally() {
        let output = HeliumTask
            .run(
                &manifest("task = \"helium\"\n[params]\nmethod = \"identity\"\n"),
                NonZeroUsize::MIN,
            )
            .unwrap();
        assert_eq!(output.provisional, Some("P06.T17"));
        assert_eq!(output.table, identity_table());
        assert_eq!(output.table.body().unwrap(), format!("\n{IDENTITY_BODY}"));
        let declared: Vec<&str> = IDENTITY_BODY
            .lines()
            .filter_map(|l| l.strip_prefix("pub const "))
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(declared, ITEMS);
        assert!(HeliumTask.fingerprint().is_empty());
    }

    #[test]
    fn the_identity_refuses_a_parameter_it_does_not_read() {
        assert_eq!(
            param_error("task = \"helium\"\n[params]\nmethod = \"identity\"\nmass_min = 0.6\n"),
            ManifestParamError::new("mass_min", "absent: the identity reads no such key")
        );
    }

    #[test]
    fn a_manifest_of_another_method_is_refused() {
        let identity = ManifestParamError::new(
            METHOD_KEY,
            "\"identity\" (P06.T17's table; P15.T7.b's fit is `helium_26`'s)",
        );
        assert_eq!(
            param_error("task = \"helium\"\n[params]\nmethod = \"spline\"\n"),
            identity
        );
        assert_eq!(
            param_error("task = \"helium\"\n[params]\nmethod = \"basti_least_squares\"\n"),
            identity
        );
        assert_eq!(
            param_error("task = \"helium\"\n[params]\n"),
            ManifestParamError::new(METHOD_KEY, "a string")
        );
        let fitted = Helium26Task.run(
            &Manifest::from_bytes(
                Path::new("helium_26.toml"),
                b"task = \"helium_26\"\n[params]\nmethod = \"identity\"\n".to_vec(),
            )
            .unwrap(),
            NonZeroUsize::MIN,
        );
        match fitted {
            Err(RunTaskError::Param(e)) => assert_eq!(
                e,
                ManifestParamError::new(METHOD_KEY, "\"basti_least_squares\" (P15.T7.b's fit)")
            ),
            other => panic!("expected a parameter error, got {other:?}"),
        }
    }

    #[test]
    fn the_run_refuses_another_tasks_manifest() {
        let result = HeliumTask.run(
            &manifest("task = \"streams\"\n[params]\nmethod = \"identity\"\n"),
            NonZeroUsize::MIN,
        );
        assert!(
            matches!(
                &result,
                Err(RunTaskError::WrongManifest { task: "helium", found }) if found == "streams"
            ),
            "{result:?}"
        );
    }
}
