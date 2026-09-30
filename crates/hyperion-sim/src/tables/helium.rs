//! The helium-excess correction of the stellar tracks (plan 06, P06.T17): how much a star's
//! excess of helium, ΔY, shortens its main sequence and giant branch and moves its horizontal
//! branch bluewards, which `stellar::sse`'s track reads through `Composition::helium_excess`.
//!
//! @provisional, committed by hand with P06.T17 as the identity. Do not edit.
//! inputs: none
//! since-generator-version: 13
//! source: none yet; plan 15's P15.T7 fits it to published helium-enhanced stellar models
//!   (reaching Y of about 0.43 at globular-cluster metallicities) and takes the file over
//! acceptance: every coefficient zero, the identity; the track applies the correction only where
//!   ΔY > 0, so every star the grid places (ΔY = 0) is bit-identical whatever the table holds
//!
//! **Provisional** (P06.T17): plan 15's P15.T7 replaces the coefficients with its fit, in this
//! shape, and registers the table with `hyperion-fit`, with a generator-version bump (members of
//! globular clusters' second populations have ΔY > 0, plan 09).
//!
//! The expected signs, for plan 15: helium burns hydrogen faster, so ΔY = 0.1 shortens the life of
//! a 0.8 M☉ star by roughly a third (s < 0, about −4 at 0.8 M☉), and a helium-rich horizontal-branch
//! star is bluer at the same luminosity (a positive shift of log₁₀ `T_eff`, larger for a thinner
//! envelope).

/// The metallicities at which [`LIFETIME_SLOPE`] is given, log₁₀ Z, increasing: \[Fe/H\] = −2.2,
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
