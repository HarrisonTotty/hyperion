//! Supernova remnants: the shell's window, its caps, its evolution and what sits inside it (plan
//! 09, phase 4: P09.T15–T16).
//!
//! A shell belongs to the star that died (brainstorm, "Supernova remnants: one route, not two").
//! Everything here is a pure function of the gas at the site, the explosion's energy and the
//! ambient metallicity, so the cells, the displaced bins and the catalogue can evaluate the same
//! function on the same marks:
//!
//! - [`shell_window`]: how long the shell stays distinct, a closed form in the site's gas
//!   ([`SiteGas`]), after Cioffi, McKee and Bertschinger (1988) with a hot-gas branch
//!   ([`window`]);
//! - [`WindowCaps`] and [`SHELL_WINDOW_CAP`]: the supremum of the window per
//!   [`ShellEnvironment`], which the catalogue's thinning envelope and the carve-out's prefilter
//!   use ([`caps`]);
//! - [`shell_state_at`]: the shell's radius, shock speed, phase and emission at an age
//!   ([`shell`]);
//! - [`remnant_offset`], [`has_bow_shock`] and [`PulsarWindNebula`]: where the compact remnant
//!   is and whether it still powers a nebula ([`remnant`]).
//!
//! Nothing here draws a random number: the explosion's energy is a mark its caller draws
//! ([`ExplosionEnergy::from_uniform`]), and nothing here reads a feature or another system, so the
//! carve-out's dependency graph stays acyclic.

pub mod caps;
pub mod remnant;
pub mod shell;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod window;

pub use caps::{SHELL_WINDOW_CAP, ShellEnvironment, WindowCaps};
pub use remnant::{
    BOW_SHOCK_SHARE, NEBULA_THRESHOLD, PulsarWindNebula, has_bow_shock, remnant_offset,
};
pub use shell::{EJECTA_MASS, ShellEmission, ShellPhase, ShellState, shell_state_at};
pub use window::{
    BuildExplosionEnergyError, BuildSiteGasError, ExplosionEnergy, MERGE_FACTOR, ShellWindow,
    SiteGas, TURBULENT_SPEED, WindowBranch, shell_window,
};
