//! The share of each layer's systems per population (brainstorm, "Sizing the layers"; plan 02,
//! P02.T9).
//!
//! A layer's density is the sum over populations of share × density, and nothing in the
//! thinning cares whether the share is one number per layer or one per layer and population. So
//! the share is a matrix from the start, keyed by mass band and column, even though every column
//! is the same in the first milestone: the band shares of the mass function. The seven
//! populations are the first columns, in [`POPULATIONS`] order; the columns after them are
//! reserved for displaced classes, which plan 08 adds (layer E alone gains about a hundred).
//!
//! Plan 13 adds a row for each substellar band after the five stellar rows. Those rows hold objects
//! per system of the column, not shares: a layer's density is `Σ row × density` either way, so the
//! thinning needs nothing else (plan 13, Design notes 2 and 3).

use super::fields::Component;
use super::imf::{BandShares, MassBand};
use super::{POPULATIONS, Population};

/// The number of population columns, the first columns of every row of a [`ShareMatrix`].
pub const POPULATION_COLUMNS: usize = POPULATIONS.len();

/// The share of each mass band's systems per column: `share(band, population)` is the fraction of
/// the population's systems whose primary's initial mass lies in the band.
///
/// Stored as band × column, one row per band: the seven populations first, then any displaced
/// classes (none before plan 08). The five stellar rows come first, in [`MassBand::ALL`] order, and
/// the two substellar rows after them ([`MassBand::SUBSTELLAR`]), whose entries are objects per
/// system and not shares.
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
/// // One row per band, the populations first.
/// assert_eq!(shares.row(MassBand::A).len(), shares.column_count());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ShareMatrix {
    /// Columns per row, at least [`POPULATION_COLUMNS`].
    columns: usize,
    /// Row-major, one row per band in [`MassBand::index`] order: `shares[band × columns + column]`.
    shares: Box<[f64]>,
}

impl ShareMatrix {
    /// Every column set to the mass function's band shares, as the brainstorm has it for the
    /// first milestone.
    ///
    /// The matrix holds the seven population columns and no displaced classes yet. Each column
    /// sums to 1 over the stellar bands, to rounding. The substellar rows are zero until
    /// [`with_substellar`](Self::with_substellar) fills them.
    #[must_use]
    pub fn uniform(bands: &BandShares) -> Self {
        let columns = POPULATION_COLUMNS;
        Self {
            columns,
            shares: MassBand::ALL
                .iter()
                .map(|&band| bands.share(band))
                .chain(MassBand::SUBSTELLAR.map(|_| 0.0))
                .flat_map(|share| std::iter::repeat_n(share, columns))
                .collect(),
        }
    }

    /// The same matrix with every population's entry of the two substellar rows set to the
    /// galaxy's objects per system (plan 13, Design note 3: one abundance for every population).
    /// The stellar rows are untouched, bit for bit.
    #[must_use]
    pub fn with_substellar(
        mut self,
        brown_dwarfs_per_system: f64,
        rogue_planets_per_system: f64,
    ) -> Self {
        for (band, per_system) in MassBand::SUBSTELLAR
            .into_iter()
            .zip([brown_dwarfs_per_system, rogue_planets_per_system])
        {
            let start = band.index() * self.columns;
            self.shares[start..start + POPULATION_COLUMNS].fill(per_system);
        }
        self
    }

    /// The share of `population`'s systems in `band`, or, for a substellar band, the population's
    /// objects of that kind per system.
    #[must_use]
    pub fn share(&self, band: MassBand, population: Population) -> f64 {
        // One index and one bounds check: placement reads this once per component per call.
        self.shares[band.index() * self.columns + population.index()]
    }

    /// The share of `component`'s systems in `band`: its population's. A component's density
    /// already holds its share of its population, so a layer's density is the sum of this times
    /// each component's density.
    #[must_use]
    pub fn component_share(&self, band: MassBand, component: &Component) -> f64 {
        self.share(band, component.population())
    }

    /// The number of columns: the [`POPULATION_COLUMNS`] populations, then the displaced classes.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns
    }

    /// Every column's share of `band`, the populations first, in [`POPULATIONS`] order.
    #[must_use]
    pub fn row(&self, band: MassBand) -> &[f64] {
        let start = band.index() * self.columns;
        &self.shares[start..start + self.columns]
    }

    /// The bytes the matrix owns on the heap.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        size_of_val(&*self.shares)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::imf::{Chabrier, Kroupa};

    #[test]
    fn every_column_sums_to_one() {
        for bands in [
            BandShares::of(&Kroupa),
            BandShares::of(&Chabrier::provisional()),
        ] {
            let shares = ShareMatrix::uniform(&bands);
            assert_eq!(shares.column_count(), POPULATION_COLUMNS);
            for column in 0..shares.column_count() {
                let total = MassBand::ALL
                    .iter()
                    .fold(0.0, |sum, &band| sum + shares.row(band)[column]);
                assert!((total - 1.0).abs() < 1e-14, "column {column}: {total}");
            }
            for population in POPULATIONS {
                for band in MassBand::ALL {
                    assert!((shares.share(band, population) - bands.share(band)).abs() < 1e-15);
                }
                for band in MassBand::SUBSTELLAR {
                    assert_same_bits(shares.share(band, population), 0.0);
                }
            }
        }
    }

    /// Plan 13's rows hold objects per system in every population, and filling them leaves the five
    /// stellar rows bit for bit as they were (P13.T2).
    #[test]
    fn the_substellar_rows_hold_objects_per_system_and_move_no_stellar_share() {
        let bands = BandShares::of(&Chabrier::provisional());
        let before = ShareMatrix::uniform(&bands);
        let after = before.clone().with_substellar(0.25, 29.5);
        for population in POPULATIONS {
            for band in MassBand::ALL {
                assert_same_bits(
                    after.share(band, population),
                    before.share(band, population),
                );
            }
            assert_same_bits(after.share(MassBand::BrownDwarf, population), 0.25);
            assert_same_bits(after.share(MassBand::RoguePlanet, population), 29.5);
        }
    }

    #[test]
    fn populations_are_the_first_columns_in_order() {
        let bands = BandShares::of(&Kroupa);
        let shares = ShareMatrix::uniform(&bands);
        for band in MassBand::ALL {
            let row = shares.row(band);
            assert_eq!(row.len(), shares.column_count());
            for population in POPULATIONS {
                assert_same_bits(row[population.index()], shares.share(band, population));
            }
        }
        assert_eq!(
            shares.heap_bytes(),
            (MassBand::ALL.len() + MassBand::SUBSTELLAR.len())
                * POPULATION_COLUMNS
                * size_of::<f64>()
        );
    }
}
