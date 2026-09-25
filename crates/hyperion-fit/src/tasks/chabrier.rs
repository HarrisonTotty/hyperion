//! The scale of Chabrier's high-mass branch (plan 15, P15.T4), which `galaxy::imf::Chabrier`
//! multiplies its power law above 1 M☉ by.
//!
//! **P15.T4.a, the move.** Plan 02 shipped the scale as a named constant, 0.68, in
//! `galaxy::imf` (its Design note 5), and this task moves it into `tables::chabrier` at the same
//! value, which changes no output (plan 15, Design note 9): revision 0 emits the scratch value its
//! manifest records, provisionally. **P15.T4.b** replaces the run with the fit, a bisection on the
//! share of all stars below 0.5 M☉ against the 20 pc census under plan 11's multiplicity, and
//! bumps the revision and the generator version; it waits on plan 11's P11.T1.d.

use std::num::NonZeroUsize;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision: 0, the scratch value moved into the table.
pub const VERSION: u32 = 0;

/// The table's contents for the scale `scale`.
#[must_use]
pub fn render(scale: f64) -> RustTable {
    RustTable {
        summary: vec![
            "The scale of Chabrier's high-mass branch (plan 15, P15.T4), which `galaxy::imf::Chabrier`"
                .to_owned(),
            "multiplies its power law above 1 M☉ by.".to_owned(),
        ],
        notes: vec![
            "Provisional (P15.T4.a): plan 02's scratch value, moved here unchanged from".to_owned(),
            "`galaxy::imf::Chabrier::PROVISIONAL_HIGH_MASS_SCALE`, which now reads it. P15.T4.b fits"
                .to_owned(),
            "it to the 20 pc census once plan 11's multiplicity is in place, with a bump.".to_owned(),
        ],
        items: vec![TableItem::Scalar {
            name: "HIGH_MASS_BRANCH_SCALE".to_owned(),
            doc: vec![
                "The factor on Chabrier's (2003) power law above 1 M☉, dimensionless: 0.68 gives"
                    .to_owned(),
                "about 71% of all stars below 0.5 M☉ with plan 02's stand-in companions, against the"
                    .to_owned(),
                "census's 68.8% (plan 02, Design note 5; Kirkpatrick et al. 2024, table 4)."
                    .to_owned(),
            ],
            value: scale,
        }],
    }
}

/// The task: P15.T4.a's move of plan 02's scratch scale, fast and provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChabrierTask;

impl FitTask for ChabrierTask {
    fn name(&self) -> &'static str {
        "chabrier"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "chabrier.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["HIGH_MASS_BRANCH_SCALE"]
    }

    /// None yet: the scratch value reads no generator code. P15.T4.b's fit will depend on plan
    /// 11's companions and plan 02's `mean_present_mass`.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::none()
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let scale = manifest.f64("scratch_scale")?;
        if !(scale.is_finite() && scale > 0.0) {
            return Err(ManifestParamError::new("scratch_scale", "positive and finite").into());
        }
        Ok(TaskOutput {
            table: render(scale),
            source: "plan 02's scratch value (Design note 5), bracketing the volume-complete census \
                     within 20 pc (Kirkpatrick et al. 2024, ApJS 271, 55, table 4) with the published \
                     function's scale of 1 (Chabrier 2003, PASP 115, 763)"
                .to_owned(),
            acceptance: "none yet: a scratch value, fitted by P15.T4.b".to_owned(),
            provisional: Some("P15.T4.a"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_manifests_scale() {
        let table = render(0.68);
        assert_eq!(table.item_names(), ["HIGH_MASS_BRANCH_SCALE"]);
        assert!(
            table
                .body()
                .unwrap()
                .ends_with("pub const HIGH_MASS_BRANCH_SCALE: f64 = 0.68;\n")
        );
    }
}
