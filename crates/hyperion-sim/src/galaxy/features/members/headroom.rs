//! The galactic centre's feature-level list against its 13-bit index: what plan 11's carved binary
//! classes will put there, and the check that it fits (plan 11, P11.T8.e).
//!
//! P11.T8.f lists the five carved classes, the fast and the slow accreting white dwarfs, the X-ray
//! binaries, the stellar mergers and the neutron-star mergers, on plan 09's [`FeatureLevelList`],
//! the centre's among them, each with a count of the feature's budget times its class's row of
//! [`CLASS_SHARES`]. The list's index is 13 bits, 8,192 members with index 0 the centre's black
//! hole, and a nuclear cluster of tens of millions of systems at the galaxy-wide share of about
//! 10⁻⁴ expects thousands of hosts. This module says whether they fit, before T8.f lists any.
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
//! index 0, under 8,192, which [`FeatureLevelList::with_classes`] asserts through the same
//! function. A centre that passed a looser bound could panic as soon as T8.f built its list, so
//! the plan's five standard deviations are reported ([`CentreListForecast::margin`]), not checked.
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
//!   [`FeatureLevelList::with_classes`] asserts the bound on every list it builds.
//!
//! # Not yet called
//!
//! The plan has the check called beside plan 03's [`check_index_headroom`], which the server calls
//! when it builds a galaxy for play. Nothing calls this one yet: no list holds these classes until
//! T8.f, and a refusal now would refuse galaxies over members that no list holds. T8.f wires it
//! there, or the brainstorm owner's ruling on the centre's index changes what it checks.
//!
//! # As measured
//!
//! At `GENERATOR_VERSION` 23 the Milky Way fixture's centre fits, at 7,145 candidates with 370
//! indices to spare, but a cluster 5% heavier does not, nor does either mass function's capped
//! cluster, and 393 of the first 10³ of plan 02's seeds fail (`tests/galaxy_sweeps.rs`). Counted
//! with the primaries' mass alone, 207 still fail. Plan 11's Risks record the finding for the
//! brainstorm's owner ("The centre's feature-level index, as measured (P11.T8.e)"), and P11.T8.f
//! waits on the ruling.

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
use crate::id::MemberSlot;
use crate::stellar::binary::CarvedClass;
use crate::tables::binary::{CLASS_SHARES, ClassShareRow};
use crate::units::SolarMasses;

#[cfg(doc)]
use super::FeatureLevelList;
#[cfg(doc)]
use super::level_list::registered_classes;
use super::level_list::{fits_list_index, list_index_margin};

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

    /// Whether the candidates fit the list's index: plus eight standard deviations and index 0,
    /// under 8,192 (module documentation).
    #[must_use]
    pub fn fits(&self) -> bool {
        fits_list_index(self.candidates)
    }

    /// The indices left free under 8,192 once the candidates, `sigmas` standard deviations of their
    /// count and index 0 are counted: positive at eight exactly where [`fits`](Self::fits) holds.
    #[must_use]
    pub fn margin(&self, sigmas: f64) -> f64 {
        list_index_margin(self.candidates, sigmas)
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
/// deviations and index 0, under its 8,192 index (module documentation).
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
                MemberSlot::INDEX_LIMIT
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
/// their Poisson count (plan 03, Design note 6) and index 0 stay under the 8,192 index (module
/// documentation).
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
/// // much they overflow the 13-bit index.
/// match check_feature_list_headroom(&Galaxy::new(Seed::new(1))) {
///     Ok(()) => {}
///     Err(ExceedFeatureListError::CentreListTooFull { candidates }) => {
///         assert!(candidates + 8.0 * candidates.sqrt() + 1.0 >= 8_192.0);
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

    /// The largest expectation the list's bound keeps: the root of `c + 8√c + 1 = 8,192`,
    /// `(√(16 + 8,191) − 4)²`, 7,498.2.
    fn largest_fitting() -> f64 {
        let root = (16.0_f64 + 8_191.0).sqrt() - 4.0;
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
            8_192.0 - f.margin(8.0),
            f.margin(8.0),
            8_192.0 - f.margin(5.0),
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

    /// A centre's list (member zero present) whose one class expects `expected` members.
    fn list_expecting(expected: f64) -> FeatureLevelList {
        let profile = ClassProfile::new(ProfileShape::Plummer { scale: 3.0 }, 60.0, 1.0);
        // Any feature word: the bound is what is tested, not the draws.
        FeatureLevelList::with_classes(
            milky_way_galaxy(),
            0,
            &GalacticPosition::ORIGIN,
            MemberZero::Present,
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

    /// The check at the Milky Way fixture: its centre's 7,145.0 candidates fit, with 369.8 indices
    /// to spare at eight standard deviations and 623.4 at five, as measured at `GENERATOR_VERSION`
    /// 23 and pinned, the candidates to 10⁻⁵ and the margins to 10⁻³, until the brainstorm's owner
    /// rules on the centre's index (plan 11's Risks, "The centre's feature-level index, as measured
    /// (P11.T8.e)").
    #[test]
    fn the_milky_way_s_centre_list_fits() {
        let galaxy = milky_way_galaxy();
        assert_eq!(check_feature_list_headroom(galaxy), Ok(()));
        let f = galaxy_centre_forecast(galaxy).unwrap();
        assert_near(f.candidates(), 7_144.99, 1e-5, "candidates");
        assert_near(
            f.margin(8.0),
            369.8,
            1e-3,
            "margin at eight standard deviations",
        );
        assert_near(f.margin(5.0), 623.4, 1e-3, "margin at five");
    }

    /// The cluster's mass at which the check first fails, under either mass function: the
    /// fixture's cluster scaled to the largest candidates that fit, which the forecast confirms
    /// a part in 500 to either side. Measured at `GENERATOR_VERSION` 23 (pinned as
    /// `the_milky_way_s_centre_list_fits` says): 1.0494 times the fixture's 2.4747 × 10⁷ M☉
    /// under Chabrier's function, 2.597 × 10⁷ M☉, and 1.1593 times its 2.4843 × 10⁷ M☉ under
    /// Kroupa's, 2.880 × 10⁷ M☉, against caps of 6.4 and 4.7 × 10⁷ M☉.
    #[test]
    fn the_check_first_fails_just_above_the_milky_way_s_cluster() {
        for (kind, want, want_share) in [
            (MassFunctionKind::Chabrier, 1.049_44, 0.703_48),
            (MassFunctionKind::Kroupa, 1.159_28, 0.713_27),
        ] {
            let fixture = scattered(kind, 0.0);
            let f = centre_list_forecast(&fixture).unwrap();
            print(&format!("{kind:?} Milky Way"), &f);
            let ratio = largest_fitting() / f.candidates();
            let mass = fixture.nuclear_cluster().mass().value() * ratio;
            let at = |factor: f64| {
                let params = scattered(kind, crate::math::log10(ratio * factor));
                centre_list_forecast(&params).unwrap()
            };
            let (below, above) = (at(1.0 - 2e-3), at(1.0 + 2e-3));
            eprintln!(
                "{kind:?}: the check first fails at about {mass:.4e} M_sun, {ratio:.5} times the \
                 fixture's cluster: {:.1} candidates a part in 500 below, {:.1} above",
                below.candidates(),
                above.candidates()
            );
            assert!(below.fits(), "{kind:?}: {}", below.candidates());
            assert!(!above.fits(), "{kind:?}: {}", above.candidates());
            assert_near(ratio, want, 1e-5, &format!("{kind:?}'s first failing mass"));
            let share = primaries_share(&fixture);
            eprintln!(
                "{kind:?} without companions: primaries form {share:.5} of m_f, so the fixture's \
                 centre expects {:.1} and first fails near {:.4} times its cluster",
                f.candidates() * share,
                ratio / share
            );
            assert_near(
                share,
                want_share,
                1e-4,
                &format!("{kind:?}'s primaries' share"),
            );
        }
    }

    /// The check at plan 02's cap on the cluster's mass, under either mass function: a capped
    /// centre overflows the list, by 11,203 indices under Chabrier's function and 4,867 under
    /// Kroupa's at eight standard deviations, as measured at `GENERATOR_VERSION` 23 (pinned to
    /// 10⁻⁵ as `the_milky_way_s_centre_list_fits` says).
    #[test]
    fn the_cap_s_centre_list_overflows_under_either_mass_function() {
        for (kind, seed, want) in [
            (
                MassFunctionKind::Chabrier,
                0x1108_e000,
                18_311.574_542_824_812,
            ),
            (
                MassFunctionKind::Kroupa,
                0x1108_e001,
                12_174.902_962_021_803,
            ),
        ] {
            let params = scattered(kind, 1.0);
            let cap = NuclearClusterParams::mass_cap(kind);
            assert_eq!(params.nuclear_cluster().mass(), cap);
            let share = primaries_share(&params);
            let galaxy = Galaxy::from_params(Seed::new(seed), params).unwrap();
            print(
                &format!("{kind:?} at the cap"),
                &galaxy_centre_forecast(&galaxy).unwrap(),
            );
            let outcome = check_feature_list_headroom(&galaxy);
            eprintln!(
                "{kind:?} at the cap of {:.3e} M_sun: {outcome:?}",
                cap.value()
            );
            let Err(ExceedFeatureListError::CentreListTooFull { candidates }) = outcome else {
                panic!("{kind:?}: the cap's centre list fits: {outcome:?}");
            };
            assert_near(candidates, want, 1e-5, &format!("{kind:?} at the cap"));
            eprintln!(
                "{kind:?} at the cap without companions: {:.1} candidates",
                candidates * share
            );
            assert!(
                !fits_list_index(candidates * share),
                "{kind:?}: the cap's centre fits without companions"
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
        assert!(list.len() > 7_000, "{}", list.len());
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
    /// two drawn seeds (the first of which overflows the list, at 11,351 candidates, and the
    /// second fits).
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
        assert!(!centre_list_forecast(drawn[0].params()).unwrap().fits());
        assert!(centre_list_forecast(drawn[1].params()).unwrap().fits());
    }

    #[test]
    fn the_error_s_message_is_lower_case_without_trailing_punctuation() {
        let message = ExceedFeatureListError::CentreListTooFull {
            candidates: 9_000.0,
        }
        .to_string();
        assert_eq!(
            message,
            "the galactic centre's feature-level list expects 9000 candidates, within eight \
             standard deviations of its index's 8192"
        );
        assert!(!message.ends_with(['.', '!', '?']), "{message}");
    }
}
