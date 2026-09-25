//! The scale of Chabrier's high-mass branch (plan 15, P15.T4), which `galaxy::imf::Chabrier`
//! multiplies its power law above 1 M☉ by.
//!
//! @provisional by hyperion-fit 0.1.0, task `chabrier` revision 0 (P15.T4.a). Do not edit.
//! inputs-sha256: 749e2b53688e44fe5f7b8dda1bd0a497c5713835cb1a71b67fe92aae6bb758c8
//! manifest: crates/hyperion-fit/manifests/chabrier.toml
//! data: none
//! sim-fingerprint: none
//! since-generator-version: 11
//! source: plan 02's scratch value (Design note 5), bracketing the volume-complete census within
//!   20 pc (Kirkpatrick et al. 2024, ApJS 271, 55, table 4) with the published function's scale
//!   of 1 (Chabrier 2003, PASP 115, 763)
//! acceptance: none yet: a scratch value, fitted by P15.T4.b
//!
//! Provisional (P15.T4.a): plan 02's scratch value, moved here unchanged from
//! `galaxy::imf::Chabrier::PROVISIONAL_HIGH_MASS_SCALE`, which now reads it. P15.T4.b fits
//! it to the 20 pc census once plan 11's multiplicity is in place, with a bump.

/// The factor on Chabrier's (2003) power law above 1 M☉, dimensionless: 0.68 gives
/// about 71% of all stars below 0.5 M☉ with plan 02's stand-in companions, against the
/// census's 68.8% (plan 02, Design note 5; Kirkpatrick et al. 2024, table 4).
pub const HIGH_MASS_BRANCH_SCALE: f64 = 0.68;
