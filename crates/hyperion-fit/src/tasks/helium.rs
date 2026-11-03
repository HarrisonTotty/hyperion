//! The helium-excess correction of the stellar tracks (plan 06, P06.T17; plan 15, P15.T7): the
//! slope s(m, Z) by which a star's excess of helium ΔY scales the log of its main-sequence and
//! giant-branch timescales, and the shift of its horizontal branch's log₁₀ `T_eff`, which the
//! sim's track reads from [`tables::helium`](hyperion_sim::tables::helium) through its hook
//! (`stellar/sse/track/excess.rs`).
//!
//! **P15.T7.b, the registration.** Plan 06's P06.T17 committed the table by hand as the identity,
//! every coefficient zero, with no fit header, lock entry or `MANIFEST` row. Revision 0 registered
//! it unchanged, which changed no output (plan 15, Design note 9), provisionally.
//!
//! **P15.T7.b's swap.** The fit, by [`BASTI_LEAST_SQUARES`] ([`fit`]): least squares on `BaSTI`'s
//! helium-enhanced models (Pietrinferni, Cassisi, Salaris and Castelli 2006, ApJ 642, 797, with the
//! database's helium-enhanced extension; `data/basti_helium/`, reduced by its `reduce.py`) relative
//! to plan 06's tracks, its figures measured on those tracks under the fitted table. It is revision
//! 1, from `manifests/helium.toml` with the fit's fingerprint, and the sim's tracks read it at
//! every version from 26 on. It was emitted first as a second table, `helium_26`, behind the
//! stellar rules' hold (`HELIUM_VERSION`), and folded into this one, bit for bit, by P15.T12's
//! check.

pub mod cluster;
pub mod figures;
pub mod fit;
pub mod grid;

use std::num::NonZeroUsize;

use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision: 1, the swap's, with the fit's fingerprint (0 was P06.T17's identity,
/// registered unchanged).
pub const VERSION: u32 = 1;

/// The manifest's key that names the method.
pub const METHOD_KEY: &str = "method";

/// The method of `manifests/helium.toml`: the least-squares fit to `BaSTI`'s grid ([`fit`]).
pub const BASTI_LEAST_SQUARES: &str = "basti_least_squares";

/// The table's items, in order: P06.T17's three, and the envelope mass at which the hook holds the
/// branch's shift (decision-p15-t7b-leans §8).
pub const ITEMS: [&str; 4] = [
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
/// Every coefficient is zero. The fitted table keeps its `LOG_Z_NODES` item as it is here
/// ([`fit::nodes_item`]): plan 06's nodes are not fitted.
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

/// The refusal of a manifest whose `method` is not the fit's.
#[must_use]
fn other_method() -> RunTaskError {
    ManifestParamError::new(
        METHOD_KEY,
        &format!("\"{BASTI_LEAST_SQUARES}\" (P15.T7.b's fit)"),
    )
    .into()
}

/// The task: P15.T7.b's fit of the helium-excess table, fast.
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
            _ => Err(other_method()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// The parameter error of a run on `text`.
    fn param_error(text: &str) -> ManifestParamError {
        let manifest = Manifest::from_bytes(Path::new("helium.toml"), text.as_bytes().to_vec())
            .expect("the test's manifest parses");
        match HeliumTask.run(&manifest, NonZeroUsize::MIN) {
            Err(RunTaskError::Param(e)) => e,
            other => panic!("expected a parameter error, got {other:?}"),
        }
    }

    /// P06.T17's body declares the first three items, the fitted table's `LOG_Z_NODES` among
    /// them.
    #[test]
    fn p06_t17s_body_declares_the_first_three_items() {
        let declared: Vec<&str> = IDENTITY_BODY
            .lines()
            .filter_map(|l| l.strip_prefix("pub const "))
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(declared, ITEMS[..3]);
    }

    #[test]
    fn a_manifest_of_another_method_is_refused() {
        let fit = ManifestParamError::new(METHOD_KEY, "\"basti_least_squares\" (P15.T7.b's fit)");
        assert_eq!(
            param_error("task = \"helium\"\n[params]\nmethod = \"identity\"\n"),
            fit
        );
        assert_eq!(
            param_error("task = \"helium\"\n[params]\nmethod = \"spline\"\n"),
            fit
        );
        assert_eq!(
            param_error("task = \"helium\"\n[params]\n"),
            ManifestParamError::new(METHOD_KEY, "a string")
        );
    }

    #[test]
    fn the_run_refuses_another_tasks_manifest() {
        let result = HeliumTask.run(
            &Manifest::from_bytes(
                Path::new("helium.toml"),
                b"task = \"streams\"\n[params]\nmethod = \"basti_least_squares\"\n".to_vec(),
            )
            .expect("the test's manifest parses"),
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
