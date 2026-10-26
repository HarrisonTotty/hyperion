//! The galactic centre's feature-level list against its 16-bit index: what plan 11's carved binary
//! classes will put there, and the check that it fits (plan 11, P11.T8.e; the index widened by
//! P11.T8.e.b).
//!
//! P11.T8.f lists the five carved classes, the fast and the slow accreting white dwarfs, the X-ray
//! binaries, the stellar mergers and the neutron-star mergers, on plan 09's [`FeatureLevelList`],
//! the centre's among them, each with a count of the feature's budget times its class's row of
//! [`CLASS_SHARES`]. A nuclear cluster of tens of millions of systems at the galaxy-wide share of
//! about 10⁻⁴ expects thousands of hosts, and up to about 18,000 candidates at plan 02's cap. The
//! centre's list index is 16 bits ([`ListIndex::Centre`]), 65,536 members with index 0 the centre's
//! black hole: this check found 13 bits too few, and the owner's ruling of 2026-10-26 widened the
//! centre's index alone (plan 11's Risks, "The centre's feature-level index, ruled"). This module
//! says whether they fit, before T8.f lists any.
//!
//! # The forecast ([`centre_list_forecast`])
//!
//! - **Formed mass**, M☉: the centre's primaries formed ([`CentreClasses::primaries_formed`])
//!   times the galaxy's initial mass formed per system ([`GalaxyParams::mean_formed_mass`],
//!   `m̄_f`, 1.096 M☉ under the default mass function at `GENERATOR_VERSION` 23, 0.821 M☉ under
//!   Kroupa's). These are the share matrix's terms: P11.T7's
//!   [`FeatureShares::set_class_shares`] gives a class a share `r m̄_f` of every system born.
//!   `m̄_f` counts companions, which the centre's members do not have yet (plan 11, ruling 144.4),
//!   so the formed mass errs high, by the companions' share of it: 30% under the default mass
//!   function, whose primaries form 0.703 of `m̄_f` (0.713 under Kroupa's).
//! - **Hosts** of each carved class: the formed mass times the class's rate, hosts per solar mass
//!   formed, the largest of its row over the seven populations ([`POPULATIONS`]). The centre is
//!   none of them. The scratch rows are equal in every population, so this is exact now and an
//!   upper bound once plan 15's P15.T10.b refits them apart.
//! - **Candidates**: each class's hosts over [`CLASS_ACCEPTANCE_FLOOR`], summed in the rows'
//!   ([`CarvedClass`]) order. A candidate that fails its class's test is thinned, and P11.T8 keeps
//!   that under one in ten, so the floor bounds what the thinning adds.
//!
//! # The bound
//!
//! The list's own: the candidates plus eight standard deviations of their Poisson count, plus
//! index 0, under the centre's 65,536, which [`FeatureLevelList::with_classes`] asserts through the
//! same function. It holds up to 63,518.8 candidates. A centre that passed a looser bound could
//! panic as soon as T8.f built its list, so the plan's five standard deviations are reported
//! ([`CentreListForecast::margin`]), not checked.
//!
//! # What it leaves out
//!
//! - **Plan 09's classes** on the centre's list, the tidal-disruption victims (P09.T30.b), the
//!   supernovae (P09.T36) and the luminous blue variables (P09.T43), are not built and register
//!   nothing ([`registered_classes`]), so they add nothing yet; plan 09 expects tens. Each must
//!   join the candidates when it registers.
//! - **Catalogue features** are not walked. A nursery's or an open cluster's initial mass is at
//!   most [`NURSERY_MASS_MAX`], 10⁵ M☉, about 11 candidates at these rates. A globular's count is
//!   P09.T9.e's encounter law under T8.f, which this module cannot forecast; T8.f brings it in.
//!   [`FeatureLevelList::with_classes`] asserts the bound, at their 13-bit index, on every list it
//!   builds.
//!
//! # Not yet called
//!
//! The plan has the check called beside plan 03's [`check_index_headroom`], which the server calls
//! when it builds a galaxy for play. Nothing calls this one yet: no list holds these classes until
//! T8.f, which lists them at the 16-bit limit and wires the check there.
//!
//! # As measured
//!
//! At `GENERATOR_VERSION` 23, against the 16-bit index, the Milky Way fixture's centre fits, at
//! 7,145.0 candidates with 57,713.8 indices to spare at eight standard deviations, and so do both
//! mass functions' capped clusters, the heaviest plan 02 draws, at 18,311.6 and 12,174.9
//! candidates (46,140.9 and 52,477.4 to spare), and every one of the first 10³ of plan 02's seeds
//! (`tests/galaxy_sweeps.rs`). The cap bounds every drawn cluster, so no seed fails while it holds.
//! Twice either cap's candidates still fit, which a test keeps as the tripwire for the ruling's
//! fallback: a refit or a new class that brings the capped forecast within a factor of two of the
//! bound is the point to widen the centre's index, by the same rule, to `[31:0]` (plan 11's Risks,
//! "P11.T8.e.b as built", after "The centre's feature-level index, ruled"). Against 13 bits, 393
//! of the 10³ seeds failed (P11.T8.e, "as measured").

use crate::galaxy::features::centre::{BuildCentreError, CentreClasses, CentreProfile};
#[cfg(doc)]
use crate::galaxy::features::kinds::open_cluster::NURSERY_MASS_MAX;
#[cfg(doc)]
use crate::galaxy::features::shares::FeatureShares;
use crate::galaxy::imf::MassFunction;
use crate::galaxy::params::GalaxyParams;
#[cfg(doc)]
use crate::galaxy::placement::check_index_headroom;
use crate::galaxy::{Galaxy, POPULATIONS};
use crate::stellar::binary::CarvedClass;
use crate::tables::binary::{CLASS_SHARES, ClassShareRow};
use crate::units::SolarMasses;

#[cfg(doc)]
use super::FeatureLevelList;
#[cfg(doc)]
use super::level_list::registered_classes;
use super::level_list::{ListIndex, fits_list_index, list_index_margin};

/// The least share of a carved class's candidates that pass its test: 0.9.
///
/// P11.T8 (plan 11): "a candidate that fails the test is thinned like any other, and the scratch
/// tables must keep that under one in ten". The forecast's candidates are its hosts over it, the
/// most the thinning can add.
pub const CLASS_ACCEPTANCE_FLOOR: f64 = 0.9;

/// What plan 11's carved classes will put on a galaxy's centre's feature-level list (module
/// documentation): its formed mass, its hosts of each class and its expected candidates.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::centre::BuildCentreError;
/// use hyperion_sim::galaxy::features::members::{CLASS_ACCEPTANCE_FLOOR, centre_list_forecast};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::stellar::binary::CarvedClass;
///
/// // Nearly all of the Milky Way centre's hosts are accreting white dwarfs, and the indices their
/// // candidates leave say whether P11.T8.f may list them there.
/// let forecast = centre_list_forecast(&GalaxyParams::milky_way_like())?;
/// let hosts = CLASS_ACCEPTANCE_FLOOR * forecast.candidates();
/// let white_dwarfs =
///     forecast.hosts(CarvedClass::AccretingWdFast) + forecast.hosts(CarvedClass::AccretingWdSlow);
/// assert!(white_dwarfs > 0.98 * hosts);
/// assert_eq!(forecast.fits(), forecast.margin(8.0) > 0.0);
/// # Ok::<(), BuildCentreError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CentreListForecast {
    primaries_formed: f64,
    systems: f64,
    mean_formed_mass: SolarMasses,
    formed_mass: SolarMasses,
    /// Each carved class's hosts, in [`CLASS_SHARES`]' row order.
    hosts: [(CarvedClass, f64); 5],
    candidates: f64,
}

impl CentreListForecast {
    /// The forecast of the centre of `profile`, for primaries on `mass_function`, at
    /// `mean_formed_mass` formed per system (module documentation).
    #[must_use]
    fn from_parts(
        profile: &CentreProfile,
        mass_function: &dyn MassFunction,
        mean_formed_mass: SolarMasses,
    ) -> Self {
        let classes = CentreClasses::new(profile, mass_function);
        let primaries_formed = classes.primaries_formed();
        let formed = primaries_formed * mean_formed_mass.value();
        let hosts = CLASS_SHARES
            .rows()
            .map(|row| (row.class(), formed * largest_rate(&row)));
        let candidates = hosts
            .iter()
            .fold(0.0, |sum, &(_, h)| sum + h / CLASS_ACCEPTANCE_FLOOR);
        Self {
            primaries_formed,
            systems: classes.systems(),
            mean_formed_mass,
            formed_mass: SolarMasses::new(formed),
            hosts,
            candidates,
        }
    }

    /// The primaries the centre's cluster formed ([`CentreClasses::primaries_formed`]).
    #[must_use]
    pub fn primaries_formed(&self) -> f64 {
        self.primaries_formed
    }

    /// The systems the centre's cluster holds at the epoch ([`CentreClasses::systems`]), for
    /// comparison: its members, with no companions yet.
    #[must_use]
    pub fn systems(&self) -> f64 {
        self.systems
    }

    /// The galaxy's initial mass formed per system, `m̄_f`, M☉
    /// ([`GalaxyParams::mean_formed_mass`]).
    #[must_use]
    pub fn mean_formed_mass(&self) -> SolarMasses {
        self.mean_formed_mass
    }

    /// The centre's formed mass, M☉: [`primaries_formed`](Self::primaries_formed) times
    /// [`mean_formed_mass`](Self::mean_formed_mass).
    #[must_use]
    pub fn formed_mass(&self) -> SolarMasses {
        self.formed_mass
    }

    /// The expected hosts of `class` in the centre: the formed mass times its row's largest rate.
    ///
    /// # Panics
    ///
    /// Never: the forecast holds every carved class, as [`CLASS_SHARES`] does.
    #[must_use]
    pub fn hosts(&self, class: CarvedClass) -> f64 {
        self.hosts
            .iter()
            .find(|&&(c, _)| c == class)
            .map(|&(_, h)| h)
            .expect("the forecast holds a row for every carved class")
    }

    /// The expected candidates of the five classes on the centre's list: each class's hosts over
    /// [`CLASS_ACCEPTANCE_FLOOR`], summed.
    #[must_use]
    pub fn candidates(&self) -> f64 {
        self.candidates
    }

    /// Whether the candidates fit the centre's list index: plus eight standard deviations and
    /// index 0, under 65,536 ([`ListIndex::Centre`]; module documentation).
    #[must_use]
    pub fn fits(&self) -> bool {
        fits_list_index(self.candidates, ListIndex::Centre.limit())
    }

    /// The indices left free under the centre's 65,536 once the candidates, `sigmas` standard
    /// deviations of their count and index 0 are counted: positive at eight exactly where
    /// [`fits`](Self::fits) holds.
    #[must_use]
    pub fn margin(&self, sigmas: f64) -> f64 {
        list_index_margin(self.candidates, sigmas, ListIndex::Centre.limit())
    }

    /// The check's verdict on the forecast.
    fn verdict(&self) -> Result<(), ExceedFeatureListError> {
        if self.fits() {
            Ok(())
        } else {
            Err(ExceedFeatureListError::CentreListTooFull {
                candidates: self.candidates,
            })
        }
    }
}

/// A row's hosts per solar mass formed: the largest over the populations (module documentation).
///
/// A rate that is NaN, infinite or negative is a broken table, which a debug assertion catches:
/// `max` would drop a NaN, and its sign at zero is not portable.
#[must_use]
fn largest_rate(row: &ClassShareRow) -> f64 {
    POPULATIONS.iter().fold(0.0, |most, &p| {
        let rate = row.hosts_per_formed_mass(p);
        debug_assert!(
            rate.is_finite() && rate >= 0.0,
            "{:?} in {p:?}: a rate of {rate}",
            row.class()
        );
        f64::max(most, rate)
    })
}

/// A feature's feature-level list cannot keep its expected candidates, plus eight standard
/// deviations and index 0, under its index's limit, 65,536 at the centre (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExceedFeatureListError {
    /// The galactic centre's list: plan 11's carved classes expect `candidates` there.
    CentreListTooFull {
        /// The expected candidates of the carved classes on the centre's list.
        candidates: f64,
    },
}

impl core::fmt::Display for ExceedFeatureListError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CentreListTooFull { candidates } => write!(
                f,
                "the galactic centre's feature-level list expects {candidates:.0} candidates, \
                 within eight standard deviations of its index's {}",
                ListIndex::Centre.limit()
            ),
        }
    }
}

impl std::error::Error for ExceedFeatureListError {}

/// The forecast of the centre of a galaxy of `params`, without building the galaxy (module
/// documentation): the centre's classes as [`CentreModel::from_params`] builds them, and no
/// distribution function.
///
/// [`CentreModel::from_params`]: crate::galaxy::features::centre::CentreModel::from_params
///
/// # Errors
///
/// [`BuildCentreError`] if the black hole's or the cluster's mass is not valid, which no
/// parameters the builder allows have.
///
/// # Examples
///
/// See [`CentreListForecast`].
pub fn centre_list_forecast(params: &GalaxyParams) -> Result<CentreListForecast, BuildCentreError> {
    let mass_function = params.mass_function().to_mass_function();
    Ok(CentreListForecast::from_parts(
        &CentreProfile::from_params(params)?,
        mass_function.as_ref(),
        params.mean_formed_mass(),
    ))
}

/// The forecast of `galaxy`'s centre, from the galaxy's own mass model and mass function, as
/// [`CentreModel::new`](crate::galaxy::features::centre::CentreModel::new) builds the centre: what
/// [`centre_list_forecast`] gives for its parameters, bit for bit, without a second mass model.
///
/// # Errors
///
/// As [`centre_list_forecast`].
fn galaxy_centre_forecast(galaxy: &Galaxy) -> Result<CentreListForecast, BuildCentreError> {
    let params = galaxy.params();
    let profile = CentreProfile::new(params.nuclear_cluster(), params.black_hole().mass())?
        .in_galaxy(galaxy.mass_model());
    Ok(CentreListForecast::from_parts(
        &profile,
        galaxy.mass_function(),
        galaxy.mean_formed_mass(),
    ))
}

/// Checks that `galaxy`'s centre has room on its feature-level list for plan 11's carved classes:
/// their expected candidates ([`CentreListForecast::candidates`]: the centre's formed mass times
/// [`CLASS_SHARES`]' rates, over [`CLASS_ACCEPTANCE_FLOOR`]) plus eight standard deviations of
/// their Poisson count (plan 03, Design note 6) and index 0 stay under the centre's 16-bit index,
/// 65,536 (module documentation).
///
/// It reads the galaxy's mass model and mass function, and builds the centre's classes but no
/// distribution function. Nothing calls it yet (module documentation, "Not yet called").
///
/// # Errors
///
/// [`ExceedFeatureListError::CentreListTooFull`] with the expected candidates if they do not fit.
///
/// # Panics
///
/// Never: a built galaxy's black hole and cluster have finite, non-negative masses, which its
/// parameters' builder ensures, so its centre's profile builds.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::members::{
///     ExceedFeatureListError, check_feature_list_headroom,
/// };
///
/// // Whether P11.T8.f may list the carved classes at this galaxy's centre, and if not, by how
/// // much they overflow the 16-bit index.
/// match check_feature_list_headroom(&Galaxy::new(Seed::new(1))) {
///     Ok(()) => {}
///     Err(ExceedFeatureListError::CentreListTooFull { candidates }) => {
///         assert!(candidates + 8.0 * candidates.sqrt() + 1.0 >= 65_536.0);
///     }
/// }
/// ```
pub fn check_feature_list_headroom(galaxy: &Galaxy) -> Result<(), ExceedFeatureListError> {
    galaxy_centre_forecast(galaxy)
        .expect("a built galaxy's black hole and cluster masses are finite and not negative")
        .verdict()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::coords::GalacticPosition;
    use crate::galaxy::Population;
    use crate::galaxy::catalogue_classes::ClassId;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::features::interior::{ClassProfile, ProfileShape};
    use crate::galaxy::features::members::{FeatureLevelList, ListClass, MemberZero};
    use crate::galaxy::imf::{MASS_LIMIT_HI, MASS_LIMIT_LO, MassFunctionKind};
    use crate::galaxy::params::{GalaxyParamsBuilder, NuclearClusterParams};
    use crate::galaxy::quad::gl32_log;
    use crate::units::Dex;
    use hyperion_testkit::float::assert_same_bits;

    /// The centre's index's exclusive bound, 65,536, as a double.
    const LIMIT: f64 = 65_536.0;

    /// The largest expectation the centre's list's bound keeps: the root of `c + 8√c + 1 =
    /// 65,536`, `(√(16 + 65,535) − 4)²`, 63,518.8.
    fn largest_fitting() -> f64 {
        let root = (16.0_f64 + 65_535.0).sqrt() - 4.0;
        root * root
    }

    /// The five rows' rates summed, hosts per solar mass formed: equal in every population.
    fn summed_rates() -> f64 {
        CLASS_SHARES.rows().iter().fold(0.0, |sum, row| {
            sum + row.hosts_per_formed_mass(Population::YoungThinDisc)
        })
    }

    /// Prints a forecast's figures, for the plan's records.
    fn print(label: &str, f: &CentreListForecast) {
        eprintln!(
            "{label}: primaries formed {:.5e}, systems {:.5e}, mean formed mass {:.6} M_sun, \
             formed mass {:.5e} M_sun; hosts: AWD fast {:.1}, AWD slow {:.1}, XRB {:.2}, stellar \
             mergers {:.2}, NS mergers {:.4}; candidates {:.1}; +8 sigma +1 {:.1} (margin {:.1}), \
             +5 sigma +1 {:.1} (margin {:.1})",
            f.primaries_formed(),
            f.systems(),
            f.mean_formed_mass().value(),
            f.formed_mass().value(),
            f.hosts(CarvedClass::AccretingWdFast),
            f.hosts(CarvedClass::AccretingWdSlow),
            f.hosts(CarvedClass::XrayBinary),
            f.hosts(CarvedClass::StellarMerger),
            f.hosts(CarvedClass::NeutronStarMerger),
            f.candidates(),
            LIMIT - f.margin(8.0),
            f.margin(8.0),
            LIMIT - f.margin(5.0),
            f.margin(5.0),
        );
    }

    /// The parameters of the Milky Way fixture under `kind` with its cluster's mass scattered by
    /// `scatter` dex, as plan 09's centre tests build them (`centre::members`' tests), and capped.
    fn scattered(kind: MassFunctionKind, scatter: f64) -> GalaxyParams {
        GalaxyParamsBuilder::new()
            .mass_function(kind)
            .nuclear_cluster_mass_scatter(Dex::new(scatter))
            .build()
            .unwrap()
    }

    /// A centre's list (member zero present, the 16-bit index) whose one class expects `expected`
    /// members.
    fn list_expecting(expected: f64) -> FeatureLevelList {
        let profile = ClassProfile::new(ProfileShape::Plummer { scale: 3.0 }, 60.0, 1.0);
        // Any feature word: the bound is what is tested, not the draws.
        FeatureLevelList::with_classes(
            milky_way_galaxy(),
            0,
            &GalacticPosition::ORIGIN,
            MemberZero::Present,
            ListIndex::Centre,
            vec![ListClass::new(
                ClassId::ACCRETING_WHITE_DWARF,
                expected,
                profile,
            )],
        )
    }

    /// The forecast at the Milky Way fixture is its primaries formed times its mass formed per
    /// system times the five rates, over the acceptance floor, and its classes' hosts are in the
    /// rows' proportion.
    #[test]
    fn the_milky_way_s_forecast_is_its_formed_mass_at_the_classes_rates() {
        let params = GalaxyParams::milky_way_like();
        let f = centre_list_forecast(&params).unwrap();
        print("Milky Way", &f);
        let formed = f.primaries_formed() * params.mean_formed_mass().value();
        assert!(
            (f.formed_mass().value() / formed - 1.0).abs() < 1e-15,
            "{:e} {formed:e}",
            f.formed_mass().value()
        );
        let want = formed * summed_rates() / CLASS_ACCEPTANCE_FLOOR;
        assert!(
            (f.candidates() / want - 1.0).abs() < 1e-12,
            "{} {want}",
            f.candidates()
        );
        let hosts: f64 = CLASS_SHARES
            .rows()
            .iter()
            .map(|row| f.hosts(row.class()))
            .sum();
        for row in CLASS_SHARES.rows() {
            let share = row.hosts_per_formed_mass(Population::Halo) / summed_rates();
            let got = f.hosts(row.class()) / hosts;
            assert!(
                (got / share - 1.0).abs() < 1e-12,
                "{:?}: {got} of the hosts against the rows' {share}",
                row.class()
            );
        }
    }

    /// Twice the cluster's mass, at the same shape and mass function, gives twice the
    /// candidates, to within the change in the remnants the deeper cluster keeps: from half the
    /// fixture's cluster to the fixture's, since twice the fixture's passes Kroupa's cap.
    #[test]
    fn the_forecast_is_linear_in_the_cluster_s_mass() {
        for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
            let at = |scatter: f64| {
                let params = scattered(kind, scatter);
                let f = centre_list_forecast(&params).unwrap();
                (params.nuclear_cluster().mass().value(), f.candidates())
            };
            let (m1, c1) = at(-core::f64::consts::LOG10_2);
            let (m2, c2) = at(0.0);
            assert!((m2 / m1 - 2.0).abs() < 1e-12, "{m1:e} {m2:e}");
            let ratio = (c2 / c1) / (m2 / m1);
            eprintln!(
                "{kind:?}: {m1:.4e} M_sun expects {c1:.2}, {m2:.4e} M_sun {c2:.2}; ratio over the \
                 masses' {ratio:.9}"
            );
            // Measured 0.9934 (Chabrier) and 0.9946 (Kroupa): the deeper cluster keeps more of
            // its remnants, which raises its present mass per primary.
            assert!((ratio - 1.0).abs() < 0.01, "{kind:?}: {ratio}");
        }
    }

    /// The share of `params`' formed mass per system that its primary forms alone, without its
    /// companions: `∫ m φ ÷ ∫ φ` over the stellar range of the mass function, by 32-node
    /// Gauss–Legendre in ln m between its breaks, over `m̄_f`. The forecast's candidates times it
    /// are what a centre of single members forms (lean (1) of the module documentation).
    fn primaries_share(params: &GalaxyParams) -> f64 {
        let f = params.mass_function().to_mass_function();
        let mut edges = vec![MASS_LIMIT_LO];
        edges.extend(
            f.breaks()
                .iter()
                .copied()
                .filter(|&b| b > MASS_LIMIT_LO && b < MASS_LIMIT_HI),
        );
        edges.push(MASS_LIMIT_HI);
        let (mut count, mut mass) = (0.0, 0.0);
        for panel in edges.windows(2) {
            count += gl32_log(|m| f.pdf(m), panel[0], panel[1]);
            mass += gl32_log(|m| m * f.pdf(m), panel[0], panel[1]);
        }
        mass / count / params.mean_formed_mass().value()
    }

    /// `got` within the relative tolerance `tolerance` of `want`.
    fn assert_near(got: f64, want: f64, tolerance: f64, what: &str) {
        assert!(
            (got / want - 1.0).abs() < tolerance,
            "{what}: {got} against {want}"
        );
    }

    /// The check at the Milky Way fixture, under either mass function, against the centre's 16-bit
    /// index: the default function's centre, 7,145.0 candidates, fits with 57,713.8 indices to
    /// spare at eight standard deviations, and Kroupa's, 6,468.0, with 58,423.6, as measured at
    /// `GENERATOR_VERSION` 23 and pinned to 10⁻⁵, with the primaries' share of `m̄_f` (module
    /// documentation) to 10⁻⁴.
    #[test]
    fn the_milky_way_s_centre_list_fits_under_either_mass_function() {
        let galaxy = milky_way_galaxy();
        assert_eq!(check_feature_list_headroom(galaxy), Ok(()));
        // The fixture's galaxy is the default function's fixture below, bit for bit.
        let fixture = centre_list_forecast(&scattered(MassFunctionKind::Chabrier, 0.0)).unwrap();
        let built = galaxy_centre_forecast(galaxy).unwrap();
        for (a, b) in figures(&built).into_iter().zip(figures(&fixture)) {
            assert_same_bits(a, b);
        }
        for (kind, candidates, at_eight, at_five, want_share) in [
            (
                MassFunctionKind::Chabrier,
                7_144.99,
                57_713.79,
                57_967.37,
                0.703_48,
            ),
            (
                MassFunctionKind::Kroupa,
                6_468.04,
                58_423.57,
                58_664.84,
                0.713_27,
            ),
        ] {
            let fixture = scattered(kind, 0.0);
            let f = centre_list_forecast(&fixture).unwrap();
            print(&format!("{kind:?} Milky Way"), &f);
            assert_eq!(f.verdict(), Ok(()), "{kind:?}");
            let what = |figure: &str| format!("{kind:?}'s {figure}");
            assert_near(f.candidates(), candidates, 1e-5, &what("candidates"));
            assert_near(f.margin(8.0), at_eight, 1e-5, &what("margin at eight"));
            assert_near(f.margin(5.0), at_five, 1e-5, &what("margin at five"));
            let share = primaries_share(&fixture);
            eprintln!(
                "{kind:?} without companions: primaries form {share:.5} of m_f, so the fixture's \
                 centre expects {:.1}",
                f.candidates() * share
            );
            assert_near(share, want_share, 1e-4, &what("primaries' share"));
        }
    }

    /// The check at plan 02's cap on the cluster's mass, under either mass function: a capped
    /// centre fits the 16-bit index, with 46,140.9 indices to spare under Chabrier's function and
    /// 52,477.4 under Kroupa's at eight standard deviations, as measured at `GENERATOR_VERSION` 23
    /// and pinned to 10⁻⁵ with the candidates. The cap bounds every drawn cluster.
    #[test]
    fn the_cap_s_centre_list_fits_under_either_mass_function() {
        for (kind, seed, want, at_eight) in [
            (
                MassFunctionKind::Chabrier,
                0x1108_e000,
                18_311.574_542_824_812,
                46_140.86,
            ),
            (
                MassFunctionKind::Kroupa,
                0x1108_e001,
                12_174.902_962_021_803,
                52_477.38,
            ),
        ] {
            let params = scattered(kind, 1.0);
            let cap = NuclearClusterParams::mass_cap(kind);
            assert_eq!(params.nuclear_cluster().mass(), cap);
            let share = primaries_share(&params);
            let galaxy = Galaxy::from_params(Seed::new(seed), params).unwrap();
            let f = galaxy_centre_forecast(&galaxy).unwrap();
            print(&format!("{kind:?} at the cap"), &f);
            assert_eq!(check_feature_list_headroom(&galaxy), Ok(()), "{kind:?}");
            assert_near(f.candidates(), want, 1e-5, &format!("{kind:?} at the cap"));
            assert_near(f.margin(8.0), at_eight, 1e-5, &format!("{kind:?}'s margin"));
            eprintln!(
                "{kind:?} at the cap of {:.3e} M_sun without companions: {:.1} candidates",
                cap.value(),
                f.candidates() * share
            );
        }
    }

    /// The tripwire for the ruling's 32-bit fallback (plan 11's Risks, "P11.T8.e.b as built"):
    /// twice either mass function's capped-cluster candidates still fit the centre's 65,536.
    /// Failing it is the point at which the fallback is due: a refit of `CLASS_SHARES`, a new
    /// class on the centre's list or a higher cap has brought the heaviest centre within a factor
    /// of two of the bound, and the ruling's next step is the same rule with the index at
    /// `[31:0]`. Measured at `GENERATOR_VERSION` 23: 27,380.9 indices to spare at eight standard
    /// deviations under Chabrier's function and 39,936.8 under Kroupa's.
    #[test]
    fn twice_either_cap_s_candidates_fit_the_centre_s_index() {
        let limit = ListIndex::Centre.limit();
        for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
            let f = centre_list_forecast(&scattered(kind, 1.0)).unwrap();
            let twice = 2.0 * f.candidates();
            eprintln!(
                "{kind:?}: twice the cap's candidates, {twice:.1}, leave {:.1} at eight standard \
                 deviations and {:.1} at five",
                list_index_margin(twice, 8.0, limit),
                list_index_margin(twice, 5.0, limit)
            );
            assert!(
                fits_list_index(twice, limit),
                "{kind:?}: twice the cap's {} candidates no longer fit 16 bits, so the 32-bit \
                 fallback is due",
                f.candidates()
            );
        }
    }

    /// The check's bound is the list's: an expectation just under it fits both, and one just over
    /// fails the check (the list's own panic is the next test).
    #[test]
    fn the_check_and_the_list_share_one_bound() {
        let largest = largest_fitting();
        let (under, over) = (largest * (1.0 - 1e-9), largest * (1.0 + 1e-9));
        let mut f = centre_list_forecast(milky_way_galaxy().params()).unwrap();
        f.candidates = under;
        assert!(f.fits());
        assert!(f.margin(8.0) > 0.0, "{}", f.margin(8.0));
        assert_eq!(f.verdict(), Ok(()));
        let list = list_expecting(under);
        assert!(list.len() > 60_000, "{}", list.len());
        f.candidates = over;
        assert!(!f.fits());
        assert!(f.margin(8.0) < 0.0, "{}", f.margin(8.0));
        assert_eq!(
            f.verdict(),
            Err(ExceedFeatureListError::CentreListTooFull { candidates: over })
        );
    }

    /// Just over the shared bound, the list refuses to build.
    #[test]
    #[should_panic(expected = "eight standard deviations")]
    fn the_list_refuses_what_the_check_refuses() {
        let _ = list_expecting(largest_fitting() * (1.0 + 1e-9));
    }

    /// Every figure of a forecast, in a fixed order: the five counts and masses, then the hosts.
    fn figures(f: &CentreListForecast) -> Vec<f64> {
        let mut all = vec![
            f.primaries_formed,
            f.systems,
            f.mean_formed_mass.value(),
            f.formed_mass.value(),
            f.candidates,
        ];
        all.extend(f.hosts.iter().map(|&(_, h)| h));
        all
    }

    /// The check of a built galaxy, which reads the galaxy's own mass model and mass function,
    /// forecasts what its parameters do, bit for bit, in every figure: the Milky Way fixture and
    /// two drawn seeds, both of which fit the centre's 16-bit index (the first, at 11,351
    /// candidates, overflowed the 13-bit one, which the second fits).
    #[test]
    fn a_galaxy_s_check_reads_what_its_parameters_forecast() {
        let drawn = [
            Galaxy::new(Seed::new(0x1108_e100)),
            Galaxy::new(Seed::new(0x1108_e101)),
        ];
        for galaxy in core::iter::once(milky_way_galaxy()).chain(&drawn) {
            let from_galaxy = galaxy_centre_forecast(galaxy).unwrap();
            let from_params = centre_list_forecast(galaxy.params()).unwrap();
            print(&format!("seed {:#x}", galaxy.seed().get()), &from_params);
            assert_eq!(from_galaxy, from_params);
            for (a, b) in figures(&from_galaxy).into_iter().zip(figures(&from_params)) {
                assert_same_bits(a, b);
            }
            assert_eq!(check_feature_list_headroom(galaxy), from_params.verdict());
        }
        let [first, second] = drawn
            .each_ref()
            .map(|g| centre_list_forecast(g.params()).unwrap());
        assert!(first.fits(), "{}", first.candidates());
        assert!(second.fits(), "{}", second.candidates());
        let thirteen_bits = ListIndex::Feature.limit();
        assert!(!fits_list_index(first.candidates(), thirteen_bits));
        assert!(fits_list_index(second.candidates(), thirteen_bits));
    }

    #[test]
    fn the_error_s_message_is_lower_case_without_trailing_punctuation() {
        let message = ExceedFeatureListError::CentreListTooFull {
            candidates: 64_000.0,
        }
        .to_string();
        assert_eq!(
            message,
            "the galactic centre's feature-level list expects 64000 candidates, within eight \
             standard deviations of its index's 65536"
        );
        assert!(!message.ends_with(['.', '!', '?']), "{message}");
    }
}
