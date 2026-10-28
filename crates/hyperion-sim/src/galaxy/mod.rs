//! The galaxy model: from a seed's parameters to closed-form fields (plan 02).
//!
//! A universe holds one barred spiral. Its seed draws the galaxy's parameters, each from a stream
//! of its own; everything else follows from them as pure functions: the mass function and its
//! band shares, the populations' age distributions, the mean present-day mass of a system and the
//! system count, the mass model and its potential tables ([`potential`]), the density fields with
//! their ages and metallicities ([`fields`]), true upper bounds on every density over a cell
//! ([`bounds`]), the share of each layer per population ([`shares`]), the column densities the
//! galaxy map draws ([`map`]) and the gas and dust between the stars ([`gas`], plan 07). [`Galaxy`]
//! bundles them:
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
pub mod catalogue_classes;
pub mod consts;
pub mod displaced;
pub mod fates;
pub mod features;
pub mod fields;
pub mod frame;
pub mod gas;
pub mod global_list;
pub mod imf;
pub mod kinematics;
pub mod map;
pub mod motion;
pub mod params;
pub mod placement;
pub mod potential;
pub mod quad;
pub mod query;
pub mod shares;
pub mod snr;
pub mod special;
pub mod substellar;

use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

use self::features::centre::{BuildCentreError, CentreModel};
use self::features::shares::FeatureShares;
use self::fields::Fields;
use self::gas::field::GasField;
use self::gas::params::BuildGasParamsError;
use self::imf::{BandShares, Chabrier, Kroupa, MassFunction, MassFunctionKind};
use self::kinematics::KinematicTables;
use self::motion::CentreMotion;
use self::params::GalaxyParams;
use self::potential::{MassModel, PotentialTables};
use self::shares::ShareMatrix;
use self::snr::WindowCaps;
use self::substellar::{SubstellarAbundance, SubstellarParams};
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
/// model with its potential tables, the density fields, the gas field and the share matrix, all
/// pure functions of the seed (or of the parameters, for a fixture) and the generator version. The
/// fates behind the mean masses are held as their result, [`GalaxyParams::mean_system_mass`].
///
/// A galaxy is immutable and `Send + Sync`, so one build can be shared between threads behind an
/// `Arc`, as the server's galaxy cache holds it (plan 04). Its only interior mutability is the
/// parameters' black hole ([`GalaxyParams::black_hole`]), solved once from the parameters when first
/// read, which the mass model does while the galaxy is built, the centre's motion
/// ([`centre_motion`](Self::centre_motion)), built from the parameters and the mass model when
/// first read (plan 09, P09.T28.b.a), and the centre's model ([`centre`](Self::centre)), built
/// from that motion's profile when first read (plan 09, P09.T29); the sim caches nothing else. [`Galaxy::new`] takes about 130 ms, against plan 02's target of
/// 100 ms, two thirds of it the discs' vertical Jeans solve in [`Fields::new`] (plan 02, Risks,
/// R19); the gas field adds its two radial quadratures, some tens of microseconds, and nothing on
/// the heap (plan 07), and the shell window's caps their scan, about 1% (plan 09,
/// [`window_caps`](Self::window_caps)). [`heap_bytes`](Self::heap_bytes) states what one costs to
/// keep, about 1.4 MiB.
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
    gas: GasField,
    shares: ShareMatrix,
    kinematics: Option<Box<KinematicTables>>,
    feature_shares: FeatureShares,
    substellar: SubstellarAbundance,
    window_caps: WindowCaps,
    centre_motion: CentreMotionCell,
    centre: CentreModelCell,
}

impl Galaxy {
    /// The galaxy of `seed` under the default mass function, with the in-plane potential tables.
    ///
    /// # Panics
    ///
    /// Never: [`from_params`](Self::from_params) refuses a galaxy only if its molecular disc and
    /// warm ionised layer leave the neutral layer nothing (plan 07, ruling 22), and since ruling 91
    /// the warm layer is clamped so that the neutral layer keeps at least half the gas wherever the
    /// molecular disc, at most 3 × 10⁶ M☉, weighs less than half of it. Every galaxy plan 02 draws
    /// has at least 2.5 × 10⁹ M☉ of gas, the least at any corner of its ranges; the proof is
    /// `gas::params`'s `the_neutral_share_is_at_least_half_at_every_corner_of_the_draws`. Nor
    /// does [`from_params`](Self::from_params)'s check of the shell window's caps fire, as it says.
    #[must_use]
    pub fn new(seed: Seed) -> Self {
        Self::with_mass_function(seed, MassFunctionKind::default())
    }

    /// The galaxy of `seed` with the mass function `kind`, which is part of the generator version
    /// (plan 02, Design note 5): for tests, and for any version that chooses Kroupa's.
    ///
    /// # Panics
    ///
    /// Never, as [`new`](Self::new) says.
    #[must_use]
    pub fn with_mass_function(seed: Seed, kind: MassFunctionKind) -> Self {
        Self::from_params(seed, GalaxyParams::from_seed(seed, kind)).expect(
            "a drawn galaxy keeps at least half its gas neutral: the warm layer is clamped to \
                 half the gas less a molecular disc of at most 3e6 M☉, and every drawn galaxy has \
                 at least 2.5e9 M☉ of gas (plan 07, ruling 91)",
        )
    }

    /// The galaxy of `params`, with `seed` keying the streams that place its stars (plan 03) and
    /// the gas field's own draws (plan 07).
    ///
    /// The parameters are `params`, such as the Milky Way fixture
    /// ([`GalaxyParams::milky_way_like`]) or a builder's, not the seed's draws.
    ///
    /// # Errors
    ///
    /// [`BuildGalaxyError::Gas`] if the gas field cannot be built: for a galaxy whose molecular
    /// disc and warm ionised layer, drawn for `seed`, outweigh its gas disc (plan 07, ruling 22).
    /// Since the warm layer's clamp (ruling 91) that needs a gas disc under 6 × 10⁶ M☉, which no
    /// galaxy the builder accepts has.
    ///
    /// # Panics
    ///
    /// If a cap of the shell window over the gas exceeds
    /// [`SHELL_WINDOW_CAP`](snr::SHELL_WINDOW_CAP) ([`WindowCaps::from_gas_params`]). It never
    /// does: the caps are largest at the lowest pressure floor a galaxy's gas has, 300 K cm⁻³,
    /// where the field's is 3.67 Myr (P09.T15.b, ruling 136.3).
    pub fn from_params(seed: Seed, params: GalaxyParams) -> Result<Self, BuildGalaxyError> {
        let mass_function = HeldMassFunction::of(params.mass_function());
        let model = MassModel::new(&params);
        let potential = PotentialTables::in_plane(&model);
        let fields = Fields::new(&params, &model);
        let gas = GasField::new(seed, &params, &fields).map_err(BuildGalaxyError::Gas)?;
        let substellar = SubstellarAbundance::from_fields(
            &fields,
            params.mean_stars_per_system(),
            SubstellarParams::generator_default(),
        );
        let shares = substellar_shares(
            ShareMatrix::uniform(&BandShares::of(mass_function.as_dyn())),
            &substellar,
        );
        let mut feature_shares = FeatureShares::new(
            &params,
            &fields,
            mass_function.as_dyn(),
            params.mean_formed_mass(),
        );
        feature_shares.set_class_shares(&crate::tables::binary::CLASS_SHARES);
        let window_caps = WindowCaps::from_gas_params(gas.params());
        Ok(Self {
            seed,
            params,
            mass_function,
            model,
            potential,
            fields,
            gas,
            shares,
            kinematics: None,
            feature_shares,
            substellar,
            window_caps,
            centre_motion: CentreMotionCell::default(),
            centre: CentreModelCell::default(),
        })
    }

    /// The same galaxy with other substellar parameters, for tests and tuning: its abundances and
    /// the share matrix's two substellar rows are derived again, and nothing stellar changes (plan
    /// 13, P13.T2).
    #[must_use]
    pub fn with_substellar_params(self, p: SubstellarParams) -> Self {
        let substellar =
            SubstellarAbundance::from_fields(&self.fields, self.mean_stars_per_system(), p);
        let shares = substellar_shares(self.shares, &substellar);
        Self {
            shares,
            substellar,
            ..self
        }
    }

    /// This galaxy with the potential's (R, |z|) grid added, and the kinematic tables that read
    /// it (plan 08, P08.T5): what velocities, and so every position away from the epoch, need.
    ///
    /// Plans 08–10 read the potential off the plane. The grid takes about 2 s to build (plan 02,
    /// Risks, R15) and the kinematic tables some tens of milliseconds more; a galaxy that has them
    /// already is returned as it is. Nothing else changes.
    #[must_use]
    pub fn with_full_potential(self) -> Self {
        if self.potential.has_grid() && self.kinematics.is_some() {
            return self;
        }
        let potential = if self.potential.has_grid() {
            self.potential
        } else {
            self.potential.with_grid(&self.model)
        };
        let kinematics = KinematicTables::new(
            self.seed,
            &self.params,
            &self.fields,
            &self.model,
            &potential,
        );
        Self {
            potential,
            kinematics: Some(Box::new(kinematics)),
            ..self
        }
    }

    /// The velocity laws of every population (plan 08), which
    /// [`with_full_potential`](Self::with_full_potential) builds; `None` for a galaxy built
    /// without them, whose systems keep their epoch positions at every time
    /// ([`query::epoch_velocity`]).
    #[must_use]
    pub fn kinematics(&self) -> Option<&KinematicTables> {
        self.kinematics.as_deref()
    }

    /// The feature catalogue's rates and the share φ of each population in features (plan 09,
    /// P09.T2.b), built with the galaxy (plan 09, Design note 15). Nothing reads φ into the fields
    /// until P09.T2.c.
    #[must_use]
    pub fn feature_shares(&self) -> &FeatureShares {
        &self.feature_shares
    }

    /// The shell window's caps over this galaxy's gas, one per environment (plan 09, P09.T15.b),
    /// which the supernova test cuts every window at (P09.T17's `claims`).
    ///
    /// They are [`WindowCaps::from_gas_params`] of the gas's parameters, a fixed scan of some 5,000
    /// windows, computed once with the galaxy: 3.7 ms against 352 ms for the whole of
    /// [`Galaxy::new`] in the same test build (2026-10-27, on a shared machine), about 1%, where a
    /// scan on every claim would cost that for every recent death. [`WindowCaps::from_galaxy`]
    /// reads them here.
    #[must_use]
    pub const fn window_caps(&self) -> &WindowCaps {
        &self.window_caps
    }

    /// What the Kepler regime reads of the centre (plan 09, P09.T28.b.a).
    ///
    /// That is the mass about the black hole, the cluster's and the rest of the galaxy's, the
    /// influence radius where it weighs the black hole, and the loss cone. Nothing on the record
    /// reads it until P09.T28.b.b. It is held with the galaxy (plan 09, Design note 15) and built when first read, from the
    /// parameters and the mass model alone, so it is the same bits whichever caller reads it first
    /// and whenever; the galaxy's equality and its `Debug` text do not depend on whether it has
    /// been read. Building it takes about 64 ms, nearly all of it the galaxy's spherical average
    /// ([`SphericalAverage::of_extended`](potential::SphericalAverage::of_extended)), which a
    /// galaxy that never asks about the centre's motion does not pay (P09.T28.b.a as built).
    ///
    /// # Panics
    ///
    /// Never: [`CentreMotion::new`] refuses a nuclear cluster or a black hole whose mass is
    /// negative or not finite, which the parameters never derive (the cluster's is a positive
    /// share of the nuclear disc's, capped, and the black hole's the M–σ relation's at the
    /// bulge's positive dispersion), and a galaxy whose mass inside 10⁻³–10⁴ ly does not bracket
    /// the black hole's, which none of the sweep's (seeds 1–300) is.
    #[must_use]
    pub fn centre_motion(&self) -> &CentreMotion {
        self.centre_motion.0.get_or_init(|| {
            CentreMotion::new(&self.params, &self.model).expect(
                "the parameters' nuclear cluster and black hole have finite, positive masses, and \
                 the mass inside 1e-3 to 1e4 ly brackets the black hole's at every seed of \
                 P09.T28.b.a's sweep",
            )
        })
    }

    /// The galactic centre's model: the nuclear cluster's profile, its four distribution
    /// functions, marks and classes, what its members are placed from (plan 09, Design note 15;
    /// P09.T29).
    ///
    /// Held with the galaxy and built when first read, as [`centre_motion`](Self::centre_motion)
    /// is, from that motion's profile and the galaxy's mass function: the same bits as
    /// [`CentreModel::new`] gives (a test holds them equal), whichever caller reads it first and
    /// whenever, and the galaxy's equality and `Debug` text do not depend on whether it has been
    /// read. Building it takes the motion's 64 ms, if not yet paid, and the four inversions, the
    /// marks' acceptances and the classes (`CentreModel::new`: 91 ms in release with three
    /// inversions, P09.T27 as built; not benched since ruling 144's fourth); a galaxy that never
    /// asks about its centre pays nothing. The range query's centre source and `resolve` of a
    /// centre member read it. [`with_full_potential`](Self::with_full_potential) and
    /// [`with_substellar_params`](Self::with_substellar_params) carry it, built or not, as they
    /// carry the motion: neither changes the parameters, the mass model or the mass function it
    /// reads.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError::NegativeDistribution`] if a tracer has no isotropic distribution
    /// function in the galaxy's potential, which no galaxy the builder allows does
    /// ([`CentreModel::new`]'s documentation), the same error at every read.
    ///
    /// # Panics
    ///
    /// As [`centre_motion`](Self::centre_motion), which it reads: if the mass inside 10⁻³–10⁴ ly
    /// does not bracket the black hole's, which no seed of P09.T28.b.a's sweep (1–300) does, or a
    /// mass is not finite and positive, which the parameters never derive.
    pub fn centre(&self) -> Result<&CentreModel, BuildCentreError> {
        self.centre
            .0
            .get_or_init(|| {
                CentreModel::from_parts(
                    self.centre_motion().profile().clone(),
                    self.mass_function(),
                )
                .map(Box::new)
            })
            .as_deref()
            .map_err(|e| *e)
    }

    /// The free-floating brown dwarfs and rogue planets per system, with the rogue planets' cap
    /// (plan 13, P13.T2), built with the galaxy. The share matrix's substellar rows hold these
    /// figures.
    #[must_use]
    pub fn substellar(&self) -> &SubstellarAbundance {
        &self.substellar
    }

    /// The mean number of stars per system, 1.33–1.45
    /// ([`GalaxyParams::mean_stars_per_system`]): what turns plan 13's abundances per star into
    /// objects per system.
    #[must_use]
    pub fn mean_stars_per_system(&self) -> f64 {
        self.params.mean_stars_per_system()
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

    /// The gas and dust field (plan 07).
    #[must_use]
    pub fn gas(&self) -> &GasField {
        &self.gas
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
    /// About 0.5–1.7 × 10¹¹ under the default mass function (ruling 147.1) and 0.5–2.1 × 10¹¹ under
    /// Kroupa's; the Milky Way fixture has 1.06 × 10¹¹ (plan 02, Risks, R18).
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
    /// It depends on the mass function alone: 1.0961 M☉ under the default, 0.8215 M☉ under
    /// Kroupa's, at generator version 23 (plan 11, P11.T8.e).
    #[must_use]
    pub fn mean_formed_mass(&self) -> SolarMasses {
        self.params.mean_formed_mass()
    }

    /// The bytes the galaxy owns on the heap, beyond `size_of::<Galaxy>()`: what a cache holding
    /// it pays besides the handle itself, without the allocator's own overhead.
    ///
    /// It varies little between seeds, since the parts are fixed-size tables but for the halo's
    /// three to six components. Most of it is the mass model's Gaussians (some 770, each with its
    /// quadrature nodes); the (R, |z|) grid adds 128 KiB and the kinematic tables that read it
    /// about 0.7 MiB (plan 08). The gas field is held inline and adds nothing. The centre's
    /// motion, once read, adds its tables, about 20 KiB (plan 09, P09.T28.b.a), and the centre's
    /// model, once built, its distribution functions' tables, about 0.5 MiB (P09.T29), so the
    /// figure depends on what has been read.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.params.heap_bytes()
            + self.model.heap_bytes()
            + self.potential.heap_bytes()
            + self.fields.heap_bytes()
            + self.gas.heap_bytes()
            + self.shares.heap_bytes()
            + self
                .centre_motion
                .0
                .get()
                .map_or(0, CentreMotion::heap_bytes)
            + self
                .centre
                .0
                .get()
                // A centre that failed to build holds no tables, so it owns no heap: the error
                // is not lost, `centre` returns it at every read.
                .and_then(|built| built.as_deref().ok())
                .map_or(0, |model| size_of::<CentreModel>() + model.heap_bytes())
            + self
                .kinematics
                .as_ref()
                .map_or(0, |k| size_of::<KinematicTables>() + k.heap_bytes())
    }
}

/// `shares` with its substellar rows set to `abundance`'s objects per system.
fn substellar_shares(shares: ShareMatrix, abundance: &SubstellarAbundance) -> ShareMatrix {
    shares.with_substellar(
        abundance.brown_dwarfs_per_system(),
        abundance.rogue_planets_per_system(),
    )
}

/// A [`Galaxy`] could not be built from its parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildGalaxyError {
    /// The gas field could not be built (plan 07, ruling 22).
    Gas(BuildGasParamsError),
}

impl fmt::Display for BuildGalaxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gas(e) => write!(f, "the galaxy's gas cannot be built: {e}"),
        }
    }
}

impl Error for BuildGalaxyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Gas(e) => Some(e),
        }
    }
}

/// [`Galaxy::centre_motion`]'s value, built when first read (the sim-determinism skill's lazy
/// exception, as [`GalaxyParams::black_hole`]'s second phase is held).
#[derive(Clone, Default)]
struct CentreMotionCell(OnceLock<CentreMotion>);

/// Equal always: the centre's motion is a pure function of the parameters and the mass model,
/// which [`Galaxy`]'s equality compares, so comparing it would make two equal galaxies unequal
/// because only one of them had been read.
impl PartialEq for CentreMotionCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The same text whether or not the value has been read, so that a galaxy's `Debug` text depends
/// on what it is and not on its history.
impl fmt::Debug for CentreMotionCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CentreMotionCell")
    }
}

/// [`Galaxy::centre`]'s value, built when first read, as [`CentreMotionCell`] is; boxed, since the
/// model's tables and arrays would otherwise add some kilobytes to every galaxy moved.
#[derive(Clone, Default)]
struct CentreModelCell(OnceLock<Result<Box<CentreModel>, BuildCentreError>>);

/// Equal always, as [`CentreMotionCell`]: the model is a pure function of what the galaxy's
/// equality compares.
impl PartialEq for CentreModelCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The same text whether or not the value has been built, as [`CentreMotionCell`]'s.
impl fmt::Debug for CentreModelCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CentreModelCell")
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
