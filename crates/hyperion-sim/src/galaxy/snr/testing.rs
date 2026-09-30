//! Test helpers for the shell window (plan 09, P09.T15.a): the window table the plan's figures are
//! written against.

use super::window::{ExplosionEnergy, ShellWindow, SiteGas, shell_window};
use crate::units::{Dex, HydrogenPerCm3, KelvinPerCm3};

/// The densities of the plan's window table, cm⁻³: 10⁻³ to 10⁴, one per decade.
pub const WINDOW_TABLE_DENSITIES: [f64; 8] = [1e-3, 1e-2, 1e-1, 1.0, 10.0, 100.0, 1e3, 1e4];

/// The window of a 10⁵¹ erg supernova at solar metallicity in uniform gas at each of
/// [`WINDOW_TABLE_DENSITIES`] and the pressure `pressure`, with the density it was taken at.
///
/// # Panics
///
/// If `pressure` is not positive and finite.
#[must_use]
pub fn window_table(pressure: KelvinPerCm3) -> Vec<(HydrogenPerCm3, ShellWindow)> {
    WINDOW_TABLE_DENSITIES
        .iter()
        .map(|&n| {
            let density = HydrogenPerCm3::new(n);
            let site = SiteGas::uniform(density, pressure).expect("a positive, finite pressure");
            (
                density,
                shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0)),
            )
        })
        .collect()
}
