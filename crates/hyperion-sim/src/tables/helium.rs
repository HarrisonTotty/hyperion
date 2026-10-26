//! The helium-excess correction of the stellar tracks (plan 06, P06.T17): how much a star's
//! excess of helium, ΔY, shortens its main sequence and giant branch and moves its horizontal
//! branch bluewards, which `stellar::sse`'s track reads through `Composition::helium_excess`.
//!
//! @provisional by hyperion-fit 0.1.0, task `helium` revision 0 (P06.T17). Do not edit.
//! inputs-sha256: 5121b2fc5d53445cbe322d3588b391c1d62a3b734a85f55dfcd60513f2891003
//! manifest: crates/hyperion-fit/manifests/helium.toml
//! data: none
//! sim-fingerprint: none
//! since-generator-version: 23
//! source: plan 06's P06.T17, the identity, with the expected signs: helium burns hydrogen
//!   faster, so a helium excess shortens the main sequence and the giant branch, and a
//!   helium-rich horizontal-branch star is bluer at the same luminosity
//! acceptance: the identity: every coefficient zero, so no star moves; P15.T7.b's fit to the
//!   helium-enhanced models of Pietrinferni et al. (2006) is held for the swap
//!
//! Provisional (P06.T17): plan 06's identity, every coefficient zero, which P06.T17 committed by
//! hand and plan 15's P15.T7.b registered with the fitting toolchain unchanged. The track applies
//! the correction only where ΔY > 0, so every star the grid places (ΔY = 0) is bit-identical
//! whatever the table holds. P15.T7.b fits the table to the helium-enhanced stellar models of
//! Pietrinferni et al. (2006) from `manifests/helium.production.toml`, to a scratch path; the
//! swap moves that fit here, in this shape, with a generator-version bump (members of globular
//! clusters' second populations have ΔY > 0, plan 09).
//!
//! The expected signs (P06.T17): helium burns hydrogen faster, so a helium excess shortens the
//! main sequence and the giant branch (s < 0), and a helium-rich horizontal-branch star is bluer
//! at the same luminosity (a positive shift of log₁₀ `T_eff`, larger for a thinner envelope).

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
