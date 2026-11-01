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
pub(crate) mod field_share;
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
pub(crate) mod rules;
pub mod shares;
pub mod snr;
pub mod special;
pub mod substellar;

use std::error::Error;
use std::fmt;
use std::sync::{Arc, OnceLock};

use self::ages::FieldShareRules;
use self::catalogue_classes::core_collapse::CoreCollapseProcess;
use self::catalogue_classes::type_ia::DelayTimeDistribution;
use self::displaced::class_table::{ClassTable, FormTable};
use self::displaced::forms::BuildFormError;
use self::displaced::site::CarveOutRules;
use self::displaced::{DisplacedFields, DisplacedRules};
use self::fates::{CompanionMasses, fates_for};
use self::features::centre::{BuildCentreError, CentreModel};
use self::features::shares::FeatureShares;
use self::field_share::FieldShare;
use self::fields::Fields;
use self::gas::field::GasField;
use self::gas::params::BuildGasParamsError;
use self::global_list::ListSourceRules;
use self::global_list::share::discrete_share_under;
use self::imf::{BandShares, Chabrier, Kept, Kroupa, MassFunction, MassFunctionKind};
use self::kinematics::KinematicTables;
use self::motion::CentreMotion;
use self::params::GalaxyParams;
use self::potential::{MassModel, PotentialTables};
use self::rules::GalaxyRules;
use self::shares::ShareMatrix;
use self::snr::WindowCaps;
use self::substellar::{SubstellarAbundance, SubstellarParams};
use crate::coords::GalacticPosition;
use crate::units::SolarMasses;
use crate::{GENERATOR_VERSION, Seed};

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
/// fates behind the mean masses are held as their result, [`GalaxyParams::mean_system_mass`]. It
/// also carries the arms its galaxy-side holds take, the generator version's, which its equality
/// compares (decision-galaxy-25-train §1; plan 09, P09.T28.b.b).
///
/// A galaxy is immutable and `Send + Sync`, so one build can be shared between threads behind an
/// `Arc`, as the server's galaxy cache holds it (plan 04). Its only interior mutability is the
/// parameters' black hole ([`GalaxyParams::black_hole`]), solved once from the parameters when first
/// read, which the mass model does while the galaxy is built, the centre's motion
/// ([`centre_motion`](Self::centre_motion)), built from the parameters and the mass model when
/// first read (plan 09, P09.T28.b.a), and the centre's model ([`centre`](Self::centre)) and
/// potential (`centre_potential`, P09.T28.d.b), built from that motion when first read (plan 09,
/// P09.T29); the sim caches nothing else. [`Galaxy::new`] takes about 130 ms, against plan 02's target of
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
    centre_potential: CentrePotentialCell,
    companions: CompanionsCell,
    type_ia_delays: TypeIaDelaysCell,
    core_collapse: CoreCollapseCell,
    /// The field's share of each component's budget, from generator version 26 (P09.T2.c); none
    /// under the held arm, where the field is the budget.
    field_share: Option<Box<FieldShare>>,
    /// What the field keeps of each component's layer D, in the fields' order, from generator
    /// version 26 (P09.T35): 1 less its ancient Type Ia, and with plan 08's displaced classes
    /// `1 − a ÷ stay_D` of the stay's slice (`with_displaced_classes`); none under the held arm.
    layer_d_kept: Option<Box<[f64]>>,
    rules: GalaxyRules,
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
        Self::build(seed, params, GalaxyRules::of_version(GENERATOR_VERSION))
    }

    /// [`from_params`](Self::from_params) under generator version `version`'s rules, every hold
    /// that version turns on in force: another crate's door to a later version's arms, for its
    /// tests alone (P06.T28.g's server test of the wire's 26 arm). Nothing generated builds one.
    ///
    /// # Errors
    ///
    /// As [`from_params`](Self::from_params).
    ///
    /// # Panics
    ///
    /// As [`from_params`](Self::from_params).
    #[cfg(any(test, feature = "testing"))]
    #[doc(hidden)]
    pub fn from_params_at_version(
        seed: Seed,
        params: GalaxyParams,
        version: crate::GeneratorVersion,
    ) -> Result<Self, BuildGalaxyError> {
        Self::build(seed, params, GalaxyRules::of_version(version))
    }

    /// [`from_params`](Self::from_params) under `rules` in place of the generator version's: the
    /// tests' door to a later version's galaxy-side holds (decision-galaxy-25-train §1.3), such as
    /// `GalaxyRules::every_26_hold()`.
    ///
    /// # Errors
    ///
    /// As [`from_params`](Self::from_params), and, under rules that install plan 08's displaced
    /// classes, [`BuildGalaxyError::DisplacedForm`] if a class's form cannot be built from the
    /// form table.
    ///
    /// # Panics
    ///
    /// As [`from_params`](Self::from_params).
    #[cfg(test)]
    pub(crate) fn from_params_with_rules(
        seed: Seed,
        params: GalaxyParams,
        rules: GalaxyRules,
    ) -> Result<Self, BuildGalaxyError> {
        Self::build(seed, params, rules)
    }

    /// This galaxy under `rules`, for tests that compare two arms on one galaxy without building its
    /// potential's grid twice.
    ///
    /// It is what [`from_params_with_rules`](Self::from_params_with_rules) builds from the same
    /// seed and parameters, with the builders after it, since nothing a galaxy builds when it is
    /// built reads its rules but the field share, the carve-out, the displaced classes' arm and the
    /// binary tables (below; and, through the displaced classes' table, the stellar rules): each
    /// other held call site reads them when asked, and a test holds the two equal. It keeps the centre's motion and potential if they have been read, since the motion
    /// reads only the parameters and the mass model, and the potential only the motion. It keeps
    /// the centre's model and the field's Type Ia delay-time distribution only if the stellar rules
    /// ([`GalaxyRules::stellar`]) stay as they were.
    /// The model's classes are built from the stellar law (the kick law's parameters and the
    /// turn-off), and the distribution from stellar lifetimes, so under other stellar rules both
    /// memos are emptied. The reset is pre-emptive: no
    /// stellar field reaches the model yet, and it builds again to the same bits. The first stellar
    /// hold that reaches it must also pass the galaxy's stellar rules into
    /// `CentreModel::from_parts`, since the reset alone does not route them. A galaxy-side hold
    /// whose arm reaches any of these memos must reset it here too.
    ///
    /// The field share (`GalaxyRules::field_share`, P09.T2.c), the carve-out
    /// (`GalaxyRules::carve_out`, P09.T35), plan 08's displaced classes
    /// (`GalaxyRules::displaced`, P08.T12.a), the hypervelocity survivors' weight
    /// (`GalaxyRules::hypervelocity`, P09.T34.b) and the halo's discrete share
    /// (`GalaxyRules::list_source`, P10.T3.d.b) are the holds read when the galaxy is built, so
    /// `rules` must keep this galaxy's: a test of any of their new arms builds the galaxy through
    /// [`from_params_with_rules`](Self::from_params_with_rules). A galaxy that holds the displaced
    /// classes keeps its stellar rules too, whose kick law its class table was built with.
    /// The binary tables (`GalaxyRules::binary_tables`, P15.T10.b) are read then too, into the
    /// share matrix's class shares, which this installs again under `rules`, as a build does, and
    /// into the field share, so they may change only where the field share is not in force.
    ///
    /// # Panics
    ///
    /// In debug builds, which every test profile is, if `rules` changes the field share, the
    /// carve-out, the displaced classes' arm, the survivors' weight or the list's source, or the
    /// stellar rules of a
    /// galaxy that holds the classes, or the binary tables of a galaxy under the field share.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_rules(self, rules: GalaxyRules) -> Self {
        debug_assert_eq!(
            rules.field_share, self.rules.field_share,
            "the field share is read when a galaxy is built: build it under the new rules with \
             `Galaxy::from_params_with_rules`"
        );
        debug_assert_eq!(
            rules.carve_out, self.rules.carve_out,
            "the carve-out is read when a galaxy is built: build it under the new rules with \
             `Galaxy::from_params_with_rules`"
        );
        debug_assert_eq!(
            rules.displaced, self.rules.displaced,
            "the displaced classes' arm is read when a galaxy is built: build it under the new \
             rules with `Galaxy::from_params_with_rules`"
        );
        debug_assert_eq!(
            rules.list_source, self.rules.list_source,
            "the halo's discrete share is stored when a galaxy is built: build it under the new \
             rules with `Galaxy::from_params_with_rules`"
        );
        debug_assert_eq!(
            rules.hypervelocity, self.rules.hypervelocity,
            "the hypervelocity survivors' weight is read when a galaxy's class table is built: \
             build it under the new rules with `Galaxy::from_params_with_rules`"
        );
        debug_assert!(
            self.fields.displaced().is_none() || rules.stellar == self.rules.stellar,
            "the displaced classes' table was built under the galaxy's stellar rules, so a galaxy \
             under others is built with `Galaxy::from_params_with_rules`"
        );
        let feature_shares = if rules.binary_tables == self.rules.binary_tables {
            self.feature_shares
        } else {
            debug_assert!(
                self.field_share.is_none(),
                "the binary tables are read into the field share when a galaxy is built: build it \
                 under the new rules with `Galaxy::from_params_with_rules`"
            );
            let mut shares = self.feature_shares;
            shares.set_class_shares(rules.binary_tables.class_shares());
            shares
        };
        let (centre, type_ia_delays) = if rules.stellar == self.rules.stellar {
            (self.centre, self.type_ia_delays)
        } else {
            (CentreModelCell::default(), TypeIaDelaysCell::default())
        };
        let core_collapse = if rules == self.rules {
            self.core_collapse
        } else {
            CoreCollapseCell::default()
        };
        Self {
            feature_shares,
            centre,
            type_ia_delays,
            core_collapse,
            rules,
            ..self
        }
    }

    /// [`from_params`](Self::from_params) under `rules`.
    ///
    /// This is the one `match` on the displaced classes' hold (P08.T12.a;
    /// [`DISPLACED_VERSION`](displaced::DISPLACED_VERSION), E4′ of decision-stellar-holds, lean
    /// of 2026-10-31). The field galaxy is built first, as every version has built it. Under the
    /// held arm that is the galaxy. Under the new arm the class table is built from it (some
    /// 0.4–0.5 s in a release build, plan 08's P08.T9 as built, and never kept as a static), and
    /// the classes and the share matrix's stay shares and class counts are installed, so that every
    /// reader of layers D and E reads them through the galaxy's fields and shares.
    fn build(
        seed: Seed,
        params: GalaxyParams,
        rules: GalaxyRules,
    ) -> Result<Self, BuildGalaxyError> {
        let field = Self::build_field(seed, params, rules)?;
        match rules.displaced {
            DisplacedRules::FieldOnly => Ok(field),
            DisplacedRules::Displaced => field.with_displaced_classes(),
        }
    }

    /// The field galaxy: version 25's build verbatim, with nothing of plan 08's displaced classes.
    fn build_field(
        seed: Seed,
        params: GalaxyParams,
        rules: GalaxyRules,
    ) -> Result<Self, BuildGalaxyError> {
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
        feature_shares.set_class_shares(rules.binary_tables.class_shares());
        // From version 27 the halo gives up its discrete share to the global list's streams and
        // dwarf cores, in the train whose range query first serves their members, and before the
        // field's factors are taken from the shares (plan 10, P10.T3.d.b; `LIST_SOURCE_VERSION`).
        match rules.list_source {
            ListSourceRules::Unregistered => {}
            ListSourceRules::Registered => {
                feature_shares.set_halo_discrete(discrete_share_under(
                    seed,
                    &params,
                    &potential,
                    rules.orphan_streams,
                ));
            }
        }
        let field_share = match rules.field_share {
            FieldShareRules::None => None,
            FieldShareRules::Phi => {
                Some(Box::new(FieldShare::new(&params, &fields, &feature_shares)))
            }
        };
        let window_caps = WindowCaps::from_gas_params(gas.params());
        let mut galaxy = Self {
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
            centre_potential: CentrePotentialCell::default(),
            companions: CompanionsCell::default(),
            type_ia_delays: TypeIaDelaysCell::default(),
            core_collapse: CoreCollapseCell::default(),
            field_share,
            layer_d_kept: None,
            rules,
        };
        // A second step, once the parts it reads exist: layer D's ancient Type Ia come from the
        // built galaxy's delay-time distribution (P09.T35).
        galaxy.layer_d_kept = match rules.carve_out {
            CarveOutRules::Unclaimed => None,
            CarveOutRules::Claimed => Some(field_share::layer_d_kept(&galaxy)),
        };
        Ok(galaxy)
    }

    /// This field galaxy with plan 08's displaced classes installed (P08.T12.a): the class table
    /// from it, under plan 15's committed form table, the classes in its fields and their stay
    /// shares and counts in its share matrix.
    ///
    /// The stay shares are the budget's ([`ClassTable::budget_stay_share`]): the field share's
    /// thinning (P09.T2.c), after the pick, takes φ. Layer D's carve-out (P09.T35) keeps `1 − a`
    /// of the budget's layer D, its ancient Type Ia `a` all among the dead, which stay; inside the
    /// stay's slice of the pick, which has already given the runaways and walkaways to their
    /// classes, it keeps `1 − a ÷ stay`, so that the field loses its ancient Type Ia exactly. It
    /// keeps 0 should `a` pass the stay; the carve-out's D3 test holds every component of the Milky
    /// Way fixture above that.
    fn with_displaced_classes(self) -> Result<Self, BuildGalaxyError> {
        let displaced = DisplacedFields::new(&self, &FormTable::committed())
            .map_err(BuildGalaxyError::DisplacedForm)?;
        let (stay, counts) = displaced.share_columns(&self.fields);
        let layer_d_kept = self.layer_d_kept.as_ref().map(|kept| {
            kept.iter()
                .zip(&stay)
                .map(|(&kept, &[stay_d, _])| {
                    if stay_d > 0.0 {
                        (1.0 - (1.0 - kept) / stay_d).max(0.0)
                    } else {
                        kept
                    }
                })
                .collect()
        });
        let shares = self.shares.with_displaced(&stay, &counts);
        let fields = self.fields.with_displaced(displaced);
        Ok(Self {
            fields,
            shares,
            layer_d_kept,
            // The core-collapse process reads the classes, so the field galaxy's, were it read,
            // is not this galaxy's.
            core_collapse: CoreCollapseCell::default(),
            ..self
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
    /// P09.T2.b), built with the galaxy (plan 09, Design note 15). The density fields stay the
    /// budget; from generator version 26 the field placement thins to gives φ up (P09.T2.c).
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
    /// influence radius where it weighs the black hole, and the loss cone. The record reads it from
    /// generator version 25 (P09.T28.b.b). It is held with the
    /// galaxy (plan 09, Design note 15) and built when first read, from the parameters and the mass
    /// model alone, so it is the same bits whichever caller reads it first and whenever; the
    /// galaxy's equality and its `Debug` text do not depend on whether it has been read. Building it takes about 64 ms, nearly all of it the galaxy's spherical average
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

    /// The stellar companions of systems drawn from the galaxy's mass function
    /// ([`CompanionMasses::new`] under [`fates_for`]'s, which every population's fates share).
    ///
    /// A quadrature of the mass of systems born at one age reads them, as a globular's own μ does
    /// under every 26 hold (plan 09, ruling 145.5). They are built when first read, in about 14 ms
    /// (a test build on a pod's core), and kept, as [`centre_motion`](Self::centre_motion) is:
    /// they are a pure function of the mass function, so the galaxy's equality and its `Debug`
    /// text do not depend on whether they have been read.
    #[must_use]
    pub(crate) fn companions(&self) -> &CompanionMasses {
        self.companions.0.get_or_init(|| {
            CompanionMasses::new(
                self.mass_function.as_dyn(),
                fates_for(Population::OldThinDisc),
            )
        })
    }

    /// The field's Type Ia delay-time distribution ([`DelayTimeDistribution`] over the field's
    /// systems and ages), which the Type Ia class reads (plan 09, P09.T34.a).
    ///
    /// It is built when first read, in some 31 ms (release, pod, 16 vCPU, 2026-10-31), and kept,
    /// as [`centre_motion`](Self::centre_motion) is: a class process built for every catalogue
    /// ID resolved built it twice each time. It is a pure function of the galaxy's fields and
    /// ages, so the galaxy's equality and its `Debug` text do not depend on whether it has been
    /// read.
    #[must_use]
    pub(crate) fn type_ia_delays(&self) -> &Arc<DelayTimeDistribution> {
        self.type_ia_delays
            .0
            .get_or_init(|| Arc::new(DelayTimeDistribution::of_field(self)))
    }

    /// The core-collapse class's process ([`CoreCollapseProcess::new`]), which the production
    /// lookup visits (plan 09, P09.T33.a–b).
    ///
    /// It is built when first read and kept, as [`centre_motion`](Self::centre_motion) is: under
    /// every 27 hold it costs some 0.42 ms (the runaway and walkaway terms' largest shares; release,
    /// pod, 16 vCPU, 2026-11-01), which the lookup paid at every visit and `system_of` twice. It is
    /// a pure function of the galaxy's fields, shares, window caps and rules, so the galaxy's
    /// equality and its `Debug` text do not depend on whether it has been read.
    #[must_use]
    pub(crate) fn core_collapse(&self) -> &CoreCollapseProcess {
        self.core_collapse
            .0
            .get_or_init(|| Box::new(CoreCollapseProcess::new(self)))
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
    /// inversions, P09.T27 as built, and 234 ms with ruling 144's fourth on a 4-vCPU pod, P09.T29
    /// as built); a galaxy that never asks about its centre pays nothing. The range query's centre
    /// source and `resolve` of a centre member read it. [`with_full_potential`](Self::with_full_potential) and
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

    /// The centre's potential as the exact orbits in it read it (plan 09, P09.T28.d.b): the black
    /// hole's, the cluster's and the rest of the galaxy's monopole, tabulated
    /// ([`CentrePotential`](motion::spherical::CentrePotential)).
    ///
    /// Only the Kepler regime's exact orbit reads it, from generator version 25 (P09.T28.d.b),
    /// where a trajectory is prepared with the centre's motion
    /// ([`Trajectory::new`](motion::Trajectory::new)). It is held with the galaxy and built when
    /// first read, as [`centre_motion`](Self::centre_motion) is, from that motion alone, in well
    /// under a millisecond once the motion is built: the same bits whichever caller reads it first
    /// and whenever, and the galaxy's equality and `Debug` text do not depend on whether it has
    /// been read. The builders that carry the motion carry it, built or not.
    ///
    /// # Panics
    ///
    /// As [`centre_motion`](Self::centre_motion), which it reads.
    #[must_use]
    pub(crate) fn centre_potential(&self) -> &motion::spherical::CentrePotential {
        self.centre_potential
            .0
            .get_or_init(|| motion::spherical::CentrePotential::new(self.centre_motion()))
    }

    /// The galaxy-side holds' arms this galaxy takes, which each held call site reads its field of
    /// (decision-galaxy-25-train §1).
    ///
    /// They are the generator version's, from [`from_params`](Self::from_params) and every builder
    /// that calls it, and [`with_full_potential`](Self::with_full_potential) and
    /// [`with_substellar_params`](Self::with_substellar_params) keep them.
    #[must_use]
    pub(crate) const fn rules(&self) -> GalaxyRules {
        self.rules
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

    /// The density fields, with plan 08's displaced classes after their components once the build
    /// has installed them ([`Fields::displaced`]).
    #[must_use]
    pub fn fields(&self) -> &Fields {
        &self.fields
    }

    /// The class table the displaced classes were built from (plan 08, P08.T9), held with the
    /// galaxy once its build has installed them; `None` for a galaxy that holds none, every galaxy
    /// below generator version 26 (P08.T12's hold).
    #[must_use]
    pub fn class_table(&self) -> Option<&ClassTable> {
        self.fields.displaced().map(DisplacedFields::class_table)
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
    /// model, once built, its distribution functions' tables, about 0.5 MiB (P09.T29), and the
    /// centre's potential, once read, its table and basis, about 15 kB (P09.T28.d.b), the
    /// companions, once read, 3.8 KiB, the field's Type Ia delay-time distribution, once read,
    /// its components' ages and its tables of lifetimes (P09.T34.a), and the core-collapse
    /// process, once read, its components and terms, so the figure depends on what has been read.
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
                .centre_potential
                .0
                .get()
                .map_or(0, motion::spherical::CentrePotential::heap_bytes)
            + self
                .kinematics
                .as_ref()
                .map_or(0, |k| size_of::<KinematicTables>() + k.heap_bytes())
            + self
                .companions
                .0
                .get()
                .map_or(0, CompanionMasses::heap_bytes)
            + self.type_ia_delays.0.get().map_or(0, |delays| {
                size_of::<DelayTimeDistribution>() + delays.heap_bytes()
            })
            + self.core_collapse.0.get().map_or(0, |process| {
                size_of::<CoreCollapseProcess>() + process.heap_bytes()
            })
            + self
                .field_share
                .as_ref()
                .map_or(0, |field| size_of::<FieldShare>() + field.heap_bytes())
            + self
                .layer_d_kept
                .as_ref()
                .map_or(0, |kept| size_of_val::<[f64]>(kept))
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
    /// A displaced class's form could not be built from plan 15's form table (plan 08, P08.T12.a):
    /// a parameter out of range, or an integral over the cube that is not positive and finite.
    /// Only a galaxy that holds the displaced classes builds them.
    DisplacedForm(BuildFormError),
}

impl fmt::Display for BuildGalaxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gas(e) => write!(f, "the galaxy's gas cannot be built: {e}"),
            Self::DisplacedForm(e) => {
                write!(f, "a displaced class's form cannot be built: {e}")
            }
        }
    }
}

impl Error for BuildGalaxyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Gas(e) => Some(e),
            Self::DisplacedForm(e) => Some(e),
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

/// [`Galaxy::centre_potential`]'s value, built when first read, as [`CentreMotionCell`] is.
#[derive(Clone, Default)]
struct CentrePotentialCell(OnceLock<motion::spherical::CentrePotential>);

/// Equal always, as [`CentreMotionCell`]: the potential is a pure function of the centre's
/// motion, itself one of what the galaxy's equality compares.
impl PartialEq for CentrePotentialCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// [`Galaxy::type_ia_delays`]' value, built when first read, as [`CentreMotionCell`] is.
#[derive(Clone, Default)]
struct TypeIaDelaysCell(OnceLock<Arc<DelayTimeDistribution>>);

/// Equal always, as [`CentreMotionCell`]: the distribution is a pure function of the galaxy's
/// fields and ages, which its equality compares.
impl PartialEq for TypeIaDelaysCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The same text whether or not the value has been built, as [`CentreMotionCell`]'s.
impl fmt::Debug for TypeIaDelaysCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TypeIaDelaysCell")
    }
}

/// [`Galaxy::companions`]' value, built when first read, as [`CentreMotionCell`] is.
#[derive(Clone, Default)]
struct CompanionsCell(OnceLock<CompanionMasses>);

/// Equal always, as [`CentreMotionCell`]: the companions are a pure function of the mass
/// function, which the galaxy's equality compares.
impl PartialEq for CompanionsCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The same text whether or not the value has been built, as [`CentreMotionCell`]'s.
impl fmt::Debug for CompanionsCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CompanionsCell")
    }
}

/// [`Galaxy::core_collapse`]'s value, built when first read, as [`CentreMotionCell`] is; boxed, as
/// [`CentreModelCell`]'s is.
#[derive(Clone, Default)]
struct CoreCollapseCell(OnceLock<Box<CoreCollapseProcess>>);

/// Equal always, as [`CentreMotionCell`]: the process is a pure function of what the galaxy's
/// equality compares.
impl PartialEq for CoreCollapseCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The same text whether or not the value has been built, as [`CentreMotionCell`]'s.
impl fmt::Debug for CoreCollapseCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CoreCollapseCell")
    }
}

/// The same text whether or not the value has been built, as [`CentreMotionCell`]'s.
impl fmt::Debug for CentrePotentialCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CentrePotentialCell")
    }
}

/// A galaxy's mass function, held by value so that [`Galaxy`] stays `Clone` and comparable, with
/// its bands' draws kept ([`Kept`]).
#[derive(Debug, Clone, PartialEq)]
enum HeldMassFunction {
    Kroupa(Kept<Kroupa>),
    Chabrier(Kept<Chabrier>),
}

impl HeldMassFunction {
    /// The function of `kind`: the one [`MassFunctionKind::to_mass_function`] boxes.
    #[must_use]
    fn of(kind: MassFunctionKind) -> Self {
        match kind {
            MassFunctionKind::Kroupa => Self::Kroupa(Kept::new(Kroupa)),
            MassFunctionKind::Chabrier => Self::Chabrier(Kept::new(Chabrier::provisional())),
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
