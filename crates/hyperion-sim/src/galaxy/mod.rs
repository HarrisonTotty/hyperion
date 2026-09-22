//! The galaxy model: from a seed's parameters to closed-form fields (plan 02).
//!
//! A universe holds one barred spiral. Its seed draws the galaxy's parameters, each from a stream
//! of its own; everything else follows from them as pure functions: the mass function and its
//! band shares, the populations' age distributions, the mean present-day mass of a system and the
//! system count, the mass model and its potential tables ([`potential`]), the density fields with
//! their ages and metallicities ([`fields`]), true upper bounds on every density over a cell
//! ([`bounds`]), the share of each layer per population ([`shares`]) and the column densities the
//! galaxy map draws ([`map`]). [`Galaxy`] bundles them:
//! built once per seed, immutable, and what placement ([`placement`]) and the range query
//! ([`query`]) read (plan 03).
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
pub mod frame;
pub mod imf;
pub mod map;
pub mod params;
pub mod placement;
pub mod potential;
pub mod quad;
pub mod query;
pub mod shares;
pub mod special;

use self::fields::Fields;
use self::imf::{BandShares, Chabrier, Kroupa, MassFunction, MassFunctionKind};
use self::params::GalaxyParams;
use self::potential::{MassModel, PotentialTables};
use self::shares::ShareMatrix;
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::units::SolarMasses;

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

/// One galaxy, complete: everything its seed decides before a star is placed (plan 02).
///
/// It holds the drawn and derived parameters, the mass function they were derived with, the mass
/// model with its potential tables, the density fields and the share matrix, all pure functions
/// of the seed (or of the parameters, for a fixture) and the generator version. The fates behind
/// the mean masses are held as their result, [`GalaxyParams::mean_system_mass`].
///
/// A galaxy is immutable, holds no interior mutability and is `Send + Sync`, so one build can be
/// shared between threads behind an `Arc`, as the server's galaxy cache holds it (plan 04). The
/// sim caches nothing itself. [`Galaxy::new`] takes about 130 ms, against plan 02's target of
/// 100 ms, two thirds of it the discs' vertical Jeans solve in [`Fields::new`] (plan 02, Risks,
/// R19); [`heap_bytes`](Self::heap_bytes) states what one costs to keep, about 1.4 MiB.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::imf::MassBand;
/// use hyperion_sim::galaxy::{Galaxy, PointLy};
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// // Placement thins each layer against its density: share × density, summed over components.
/// let (fields, shares) = (galaxy.fields(), galaxy.shares());
/// let sun = PointLy::new(22_516.7, 13_000.0, 50.0);
/// let layers: Vec<f64> = MassBand::ALL
///     .iter()
///     .map(|&band| fields.layer_density(shares, band, &sun))
///     .collect();
/// // Layer A, M dwarfs, holds most systems and layer E, above 8 M☉, a tiny fraction.
/// let total: f64 = layers.iter().sum();
/// assert!(layers[0] > 0.6 * total && layers[4] < 0.01 * total);
/// // The system count follows from the stellar mass and the mean mass per system.
/// assert!((0.5e11..1.8e11).contains(&galaxy.system_count()));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Galaxy {
    seed: Seed,
    params: GalaxyParams,
    mass_function: HeldMassFunction,
    model: MassModel,
    potential: PotentialTables,
    fields: Fields,
    shares: ShareMatrix,
}

impl Galaxy {
    /// The galaxy of `seed` under the default mass function, with the in-plane potential tables.
    #[must_use]
    pub fn new(seed: Seed) -> Self {
        Self::with_mass_function(seed, MassFunctionKind::default())
    }

    /// The galaxy of `seed` with the mass function `kind`, which is part of the generator version
    /// (plan 02, Design note 5): for tests, and for any version that chooses Kroupa's.
    #[must_use]
    pub fn with_mass_function(seed: Seed, kind: MassFunctionKind) -> Self {
        Self::from_params(seed, GalaxyParams::from_seed(seed, kind))
    }

    /// The galaxy of `params`, with `seed` keying the streams that place its stars (plan 03).
    ///
    /// The parameters are `params`, such as the Milky Way fixture
    /// ([`GalaxyParams::milky_way_like`]) or a builder's, not the seed's draws.
    #[must_use]
    pub fn from_params(seed: Seed, params: GalaxyParams) -> Self {
        let mass_function = HeldMassFunction::of(params.mass_function());
        let model = MassModel::new(&params);
        let potential = PotentialTables::in_plane(&model);
        let fields = Fields::new(&params, &model);
        let shares = ShareMatrix::uniform(&BandShares::of(mass_function.as_dyn()));
        Self {
            seed,
            params,
            mass_function,
            model,
            potential,
            fields,
            shares,
        }
    }

    /// This galaxy with the potential's (R, |z|) grid added, for plans 08–10.
    ///
    /// They read the potential off the plane. The grid takes about 3 s to build (plan 02, Risks,
    /// R15); a galaxy that has it already is returned as it is. Nothing else changes.
    #[must_use]
    pub fn with_full_potential(self) -> Self {
        if self.potential.has_grid() {
            return self;
        }
        let potential = self.potential.with_grid(&self.model);
        Self { potential, ..self }
    }

    /// The seed: for a galaxy built from one, the seed its parameters were drawn from, and for
    /// every galaxy the seed of the streams that place its stars.
    #[must_use]
    pub fn seed(&self) -> Seed {
        self.seed
    }

    /// The parameters, drawn and derived.
    #[must_use]
    pub fn params(&self) -> &GalaxyParams {
        &self.params
    }

    /// The mass model the potential tables and the discs' vertical profiles were built from.
    #[must_use]
    pub fn mass_model(&self) -> &MassModel {
        &self.model
    }

    /// The potential tables: in the plane, and off it after
    /// [`with_full_potential`](Self::with_full_potential).
    #[must_use]
    pub fn potential(&self) -> &PotentialTables {
        &self.potential
    }

    /// The density fields.
    #[must_use]
    pub fn fields(&self) -> &Fields {
        &self.fields
    }

    /// The share of each layer per population.
    #[must_use]
    pub fn shares(&self) -> &ShareMatrix {
        &self.shares
    }

    /// The mass function: the default Chabrier's, or Kroupa's for
    /// [`with_mass_function`](Self::with_mass_function) or parameters built with it.
    #[must_use]
    pub fn mass_function(&self) -> &dyn MassFunction {
        self.mass_function.as_dyn()
    }

    /// The expected number of systems born at the epoch, with the feature share φ at 0.
    ///
    /// 0.5–1.8 × 10¹¹ under the default mass function and 0.5–2.1 × 10¹¹ under Kroupa's; the Milky
    /// Way fixture has 1.06 × 10¹¹ (plan 02, Risks, R18).
    #[must_use]
    pub fn system_count(&self) -> f64 {
        self.params.system_count()
    }

    /// The mean present-day mass of a system of `population`: living stars, remnants and
    /// companions together.
    ///
    /// Under the default mass function 0.54–0.58 M☉ for every population but the young disc,
    /// whose 0.77–0.83 M☉ still holds its massive stars (0.45–0.52 and 0.65–0.75 M☉ under
    /// Kroupa's; plan 02, Risks, R18).
    #[must_use]
    pub fn mean_system_mass(&self, population: Population) -> SolarMasses {
        self.params.mean_system_mass(population)
    }

    /// The initial mass formed per system, with nothing dead: the denominator of rates quoted per
    /// solar mass formed.
    ///
    /// It depends on the mass function alone: 0.957 M☉ under the default, 0.836 M☉ under
    /// Kroupa's.
    #[must_use]
    pub fn mean_formed_mass(&self) -> SolarMasses {
        self.params.mean_formed_mass()
    }

    /// The bytes the galaxy owns on the heap, beyond `size_of::<Galaxy>()`: what a cache holding
    /// it pays besides the handle itself, without the allocator's own overhead.
    ///
    /// It varies little between seeds, since the parts are fixed-size tables but for the halo's
    /// three to six components. Most of it is the mass model's Gaussians (some 770, each with its
    /// quadrature nodes); the (R, |z|) grid adds 128 KiB.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.params.heap_bytes()
            + self.model.heap_bytes()
            + self.potential.heap_bytes()
            + self.fields.heap_bytes()
            + self.shares.heap_bytes()
    }
}

/// A galaxy's mass function, held by value so that [`Galaxy`] stays `Clone` and comparable.
#[derive(Debug, Clone, Copy, PartialEq)]
enum HeldMassFunction {
    Kroupa(Kroupa),
    Chabrier(Chabrier),
}

impl HeldMassFunction {
    /// The function of `kind`: the one [`MassFunctionKind::to_mass_function`] boxes.
    #[must_use]
    fn of(kind: MassFunctionKind) -> Self {
        match kind {
            MassFunctionKind::Kroupa => Self::Kroupa(Kroupa),
            MassFunctionKind::Chabrier => Self::Chabrier(Chabrier::provisional()),
        }
    }

    /// The function behind the trait's interface.
    #[must_use]
    fn as_dyn(&self) -> &dyn MassFunction {
        match self {
            Self::Kroupa(f) => f,
            Self::Chabrier(f) => f,
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

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

    #[test]
    fn the_held_mass_function_is_the_kinds_own() {
        for kind in [MassFunctionKind::Kroupa, MassFunctionKind::Chabrier] {
            let held = HeldMassFunction::of(kind);
            let boxed = kind.to_mass_function();
            assert_eq!(format!("{:?}", held.as_dyn()), format!("{boxed:?}"));
            let (a, b) = (
                BandShares::of(held.as_dyn()),
                BandShares::of(boxed.as_ref()),
            );
            for (x, y) in a.as_array().into_iter().zip(b.as_array()) {
                assert_same_bits(x, y);
            }
        }
    }
}
