//! The galaxy model: from a seed's parameters to closed-form fields (plan 02).
//!
//! A universe holds one barred spiral. Its seed draws the galaxy's parameters, each from a stream
//! of its own; everything else follows from them as pure functions: the mass function and its
//! band shares, the populations' age distributions, the mean present-day mass of a system and the
//! system count, the mass model and its potential tables ([`potential`]), the density fields with
//! their ages and metallicities ([`fields`]), true upper bounds on every density over a cell
//! ([`bounds`]) and the share of each layer per population ([`shares`]). No star is placed here;
//! placement reads what this module provides.
//!
//! Generation code here works in light-years, solar masses, Julian years and km/s ([`consts`]).
//! Public functions take and return [`units`](crate::units) newtypes, except on hot paths, which
//! take a [`PointLy`] and return a bare `f64` whose unit their documentation states.
//!
//! The galactic frame is plan 01's ([`coords`](crate::coords)): the origin at the centre, +x along
//! the bar, +z to galactic north, the galaxy turning counter-clockwise seen from the north.

pub mod ages;
pub mod bounds;
pub mod consts;
pub mod fates;
pub mod fields;
pub mod imf;
pub mod params;
pub mod potential;
pub mod quad;
pub mod shares;
pub mod special;

use crate::coords::GalacticPosition;

/// A point of the galactic frame in light-years, as `f64`s: the argument of every density.
///
/// A `f64` light-year resolves about 70 km at 50,000 ly, far below any scale on which a density
/// changes, so the fields can take this lossy form of a
/// [`GalacticPosition`] without harm. Positions that must be exact stay `GalacticPosition`s.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PointLy {
    /// Along the bar, light-years.
    pub x: f64,
    /// In the plane, perpendicular to the bar, light-years.
    pub y: f64,
    /// Towards galactic north, light-years.
    pub z: f64,
}

impl PointLy {
    /// The point `(x, y, z)` in light-years.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl From<&GalacticPosition> for PointLy {
    fn from(position: &GalacticPosition) -> Self {
        let [x, y, z] = position.to_light_years_f64();
        Self { x, y, z }
    }
}

/// The seven stellar populations of the brainstorm's "Populations" table.
///
/// Every system placed by the grid belongs to exactly one. The order is the table's and is part of
/// the generator version wherever populations are summed or listed ([`POPULATIONS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Population {
    /// Stars formed in the last 100 Myr and still forming, strongly bound to the arms.
    YoungThinDisc,
    /// The thin disc from 0.1 to 10 Gyr, as five sub-discs by age.
    OldThinDisc,
    /// The thick disc, 10–12 Gyr old, shorter and about three times as tall.
    ThickDisc,
    /// The boxy triaxial bulge, 8–12 Gyr old.
    Bulge,
    /// The long bar along the x axis, 6–10 Gyr old.
    LongBar,
    /// The small, dense nuclear disc at the centre, mostly over 8 Gyr old.
    NuclearDisc,
    /// The stellar halo, a marked mixture of accreted and in-situ components, 10–13 Gyr old.
    Halo,
}

/// Every population, in the order of [`Population`]'s declaration.
pub const POPULATIONS: [Population; 7] = [
    Population::YoungThinDisc,
    Population::OldThinDisc,
    Population::ThickDisc,
    Population::Bulge,
    Population::LongBar,
    Population::NuclearDisc,
    Population::Halo,
];

impl Population {
    /// The population's place in [`POPULATIONS`], 0–6.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::YoungThinDisc => 0,
            Self::OldThinDisc => 1,
            Self::ThickDisc => 2,
            Self::Bulge => 3,
            Self::LongBar => 4,
            Self::NuclearDisc => 5,
            Self::Halo => 6,
        }
    }

    /// A short snake-case name, such as `young_thin_disc`, for logs and golden files.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::YoungThinDisc => "young_thin_disc",
            Self::OldThinDisc => "old_thin_disc",
            Self::ThickDisc => "thick_disc",
            Self::Bulge => "bulge",
            Self::LongBar => "long_bar",
            Self::NuclearDisc => "nuclear_disc",
            Self::Halo => "halo",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::LyCell;
    use crate::units::consts::METRES_PER_LIGHT_YEAR;

    #[test]
    fn populations_are_listed_in_index_order() {
        for (i, p) in POPULATIONS.iter().enumerate() {
            assert_eq!(p.index(), i);
        }
        let mut names: Vec<_> = POPULATIONS.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), POPULATIONS.len());
    }

    #[test]
    fn a_galactic_position_becomes_light_years() {
        let position = GalacticPosition::new(
            LyCell::new([26_000, -3, 50]),
            [0.5 * METRES_PER_LIGHT_YEAR, 0.0, 0.0],
        )
        .unwrap();
        let point = PointLy::from(&position);
        assert_eq!(point, PointLy::new(26_000.5, -3.0, 50.0));
    }
}
