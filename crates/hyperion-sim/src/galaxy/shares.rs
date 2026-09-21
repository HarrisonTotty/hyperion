//! The share of each layer's systems per population (brainstorm, "Sizing the layers"; plan 02,
//! P02.T9).
//!
//! A layer's density is the sum over populations of share × density, and nothing in the
//! thinning cares whether the share is one number per layer or one per layer and population. So
//! the share is a matrix from the start, keyed by mass band and population, even though every
//! population's column is the same in the first milestone: the band shares of the mass function.
//! Later plans add columns for displaced classes.
//!
//! It exists ahead of the rest of P02.T9 because [`Fields::layer_density`](super::fields::Fields::layer_density)
//! (P02.T7.e) weights by it.

use super::fields::Component;
use super::imf::{BandShares, MassBand};
use super::{POPULATIONS, Population};

/// The share of each mass band's systems per population: `share(band, population)` is the
/// fraction of the population's systems whose primary's initial mass lies in the band.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::Population;
/// use hyperion_sim::galaxy::imf::{BandShares, Kroupa, MassBand};
/// use hyperion_sim::galaxy::shares::ShareMatrix;
///
/// let shares = ShareMatrix::uniform(&BandShares::of(&Kroupa));
/// // Three in four systems have an M dwarf primary, in every population alike in M1.
/// let m_dwarfs = shares.share(MassBand::A, Population::Halo);
/// assert!((0.75..0.77).contains(&m_dwarfs));
/// assert!((m_dwarfs - shares.share(MassBand::A, Population::Bulge)).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShareMatrix {
    /// `[band][population]`.
    shares: [[f64; POPULATIONS.len()]; 5],
}

impl ShareMatrix {
    /// Every population's column set to the mass function's band shares, as the brainstorm has
    /// it for the first milestone.
    #[must_use]
    pub fn uniform(bands: &BandShares) -> Self {
        Self {
            shares: MassBand::ALL.map(|band| [bands.share(band); POPULATIONS.len()]),
        }
    }

    /// The share of `population`'s systems in `band`.
    #[must_use]
    pub fn share(&self, band: MassBand, population: Population) -> f64 {
        self.shares[band.index()][population.index()]
    }

    /// The share of `component`'s systems in `band`: its population's. A component's density
    /// already holds its share of its population, so a layer's density is the sum of this times
    /// each component's density.
    #[must_use]
    pub fn component_share(&self, band: MassBand, component: &Component) -> f64 {
        self.share(band, component.population())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::imf::{Chabrier, Kroupa};

    #[test]
    fn every_column_sums_to_one() {
        for bands in [
            BandShares::of(&Kroupa),
            BandShares::of(&Chabrier::provisional()),
        ] {
            let shares = ShareMatrix::uniform(&bands);
            for population in POPULATIONS {
                let total = MassBand::ALL
                    .iter()
                    .fold(0.0, |sum, &band| sum + shares.share(band, population));
                assert!((total - 1.0).abs() < 1e-14, "{population:?}: {total}");
                for band in MassBand::ALL {
                    assert!((shares.share(band, population) - bands.share(band)).abs() < 1e-15);
                }
            }
        }
    }
}
