//! Tidal and encounter stripping: the radius beyond which a system keeps nothing (plan 14, design
//! note 14, P14.T29).
//!
//! A system's strip radius ([`SystemContext::strip_radius`]) is the smaller of two radii:
//!
//! - **The tidal cut**, [`SATELLITE_STABILITY_FRACTION`] (0.4895) of the system's sphere of
//!   influence ([`SystemContext::tidal_radius`]): nothing prograde is stable out to the full
//!   radius, which about a point mass is the Hill radius (design note 14, which applies the
//!   point-mass fraction to the galaxy's and a cluster's tides too).
//! - **The encounter cut** of a feature member ([`encounter_radius`]): the semi-major axis at which
//!   one removing encounter is expected within the system's age, the rate the Maxwellian average
//!   of the gravitationally focused cross-section gives, scaled to the measured removal
//!   cross-sections ([`ENCOUNTER_REMOVAL_FACTOR`]).
//!
//! The disc is truncated at it (P14.T3.b), the cometary halo bounded by it (P14.T21.d), and
//! placement asserts it again (P14.T30.a).
//!
//! # The sphere of influence
//!
//! What a system's record gives (`Surroundings::of_record`), plan 09's rule:
//!
//! - **A grid system** takes [`tidal_radius`] of its epoch state
//!   ([`EpochState::of_grid_record`]): in the central black hole's Kepler regime, the tidal radius
//!   at its orbit's pericentre (P09.T28.c), and plan 02's at its epoch position elsewhere.
//! - **A feature member** takes the smaller of that, on its cluster's orbit (the cluster's bulk
//!   velocity; lean, 2026-10-31), and its cluster's tidal radius at its place
//!   ([`cluster_tidal_radius`]), at the system's mass.
//! - **Every other record** (a catalogue class's system, a member of the galactic centre or of the
//!   global list) keeps plan 02's radius at its epoch position, and has no encounter environment
//!   (lean, 2026-10-31). A catalogue system's stars are its class's (P09.T33.a); a centre or
//!   global-list member's are not generated yet ([`stars_of`](crate::observe::stars_of)), so no
//!   context reaches the arm for them, and the task that gives a centre member its system stage
//!   gives it its orbit's pericentre ([`EpochState::of_centre_member`]) and the nuclear cluster's
//!   environment.
//!
//! A free-floating brown dwarf (P14.T27.a, `HostKind::BrownDwarf`) is a grid record of layer F,
//! and takes the grid rule at its own mass: nothing here is keyed on the host's kind.
//!
//! # The encounter environment
//!
//! A feature member's is its cluster's at its place: the number density and the mean system mass
//! of every member there, from the interior's class profiles ([`MemberClassTable::density`] and
//! [`MemberClassTable::mean_member_mass`]), and the cluster's one-dimensional velocity
//! dispersion there ([`ClusterModel::sigma`]). Its interior comes from the caller's
//! [`FeatureInteriorCache`]; built afresh, it costs 14–17 ms (P09.T21 as built).
//!
//! A member inside its cluster's core ([`ClusterModel::profile_core_radius`]) reads them at the
//! core's edge, on the same line from the centre (lean, 2026-10-31). Its orbit carries it through
//! the core many times within its age, so the rate it meets is the core's, not the density at its
//! epoch place: a core-collapsed cluster's cusp ([`ProfileShape::Cusp`], softened only at 10⁻³ ly)
//! would otherwise give a member at its centre a density that stands for less than one neighbour,
//! and a cut 10²–10⁴ times too tight. Inside a cored profile the core's edge reads the density at
//! the centre within the shape's factor `2^(−3q′ ÷ 2)`.
//!
//! # The hold
//!
//! Everything here moves output, so it is held until generator version 26 (`STRIP_VERSION`).
//! Under its held arm a context's sphere of influence is plan 02's radius at its epoch
//! position, it has no encounter environment, and its strip radius is 0.4895 of that radius alone:
//! version 24's, bit for bit.
//!
//! # Determinism
//!
//! Pure: IEEE arithmetic on the context's figures and on the record's own closed forms. No stream
//! is opened and no tag added.
//!
//! [`MemberClassTable::density`]: crate::galaxy::features::interior::MemberClassTable::density
//! [`MemberClassTable::mean_member_mass`]: crate::galaxy::features::interior::MemberClassTable::mean_member_mass
//! [`ClusterModel::sigma`]: crate::galaxy::features::cluster::ClusterModel::sigma
//! [`ClusterModel::profile_core_radius`]: crate::galaxy::features::cluster::ClusterModel::profile_core_radius
//! [`ProfileShape::Cusp`]: crate::galaxy::features::interior::ProfileShape::Cusp

use crate::galaxy::consts::{G, LIGHT_YEARS_PER_YEAR_PER_KM_S};
use crate::galaxy::features::catalogue::FeatureCatalogue;
use crate::galaxy::features::members::{
    FeatureInterior, FeatureInteriorCache, cluster_tidal_radius, local_offset,
};
use crate::galaxy::motion::EpochState;
use crate::galaxy::motion::tidal::tidal_radius;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::SystemIdKind;
use crate::planetary::context::{
    BuildEncounterEnvironmentError, EncounterEnvironment, SystemContext,
};
use crate::planetary::params::{ENCOUNTER_REMOVAL_FACTOR, SATELLITE_STABILITY_FRACTION};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Metres, PerCubicLightYear, SolarMasses, Years};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which P14.T29's stripping is on the record: 26, the planetary bump
/// after 25's, in plan 14's train.
///
/// It is P14.T29's one hold. Below it [`StripRules::in_force`] is [`StripRules::Held`]: every
/// context's sphere of influence is plan 02's radius at its epoch position, no context has an
/// encounter environment, and every generated value is version 24's and 25's bit for bit. 26's
/// bump deletes this hold, [`StripRules`] with its `Held` arm, the tests' switch and its tripwire
/// (`the_stripping_is_held_only_until_the_26_bump`), and adds the galaxy README's relaxation 14
/// ("no pericentre stripping within about 10 ly of the centre") to the relaxations removed.
pub(crate) const STRIP_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Whether P14.T29's stripping is in force (its hold, [`STRIP_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum StripRules {
    /// Version 25's and before: plan 02's radius at the epoch position, and no encounter cut.
    Held,
    /// Version 26's: plan 09's sphere of influence and a feature member's encounter cut.
    Stripped,
}

impl StripRules {
    /// The rules this generator version gives: the stripping from [`STRIP_VERSION`], none below
    /// it; in the tests, the rules a test's switch sets on its thread ([`with_stripping`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = STRIP_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= STRIP_VERSION {
            Self::Stripped
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The stripping's rules a test sets on its thread, `None` for the version's.
    static STRIP_RULES: core::cell::Cell<Option<StripRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the stripping's rules `rules` on this thread, for the tests to reach
/// P14.T29 ahead of the bump; the version's rules are set back after, a panic in `run` included.
/// 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_stripping<R>(rules: StripRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<StripRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            STRIP_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(STRIP_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The radius inside which the system of `ctx` keeps its bodies, m: the smaller of
/// [`SATELLITE_STABILITY_FRACTION`] of its sphere of influence and, for a context with an
/// encounter environment, its [`encounter_radius`] (module documentation).
///
/// Under the hold (`STRIP_VERSION`) it is the first alone: an encounter environment a synthetic
/// host was given is carried but not applied.
#[must_use]
pub(crate) fn strip_radius(ctx: &SystemContext) -> Metres {
    let tidal = ctx.tidal_radius() * SATELLITE_STABILITY_FRACTION;
    match StripRules::in_force() {
        StripRules::Held => tidal,
        StripRules::Stripped => ctx
            .encounter_environment()
            .and_then(|environment| {
                encounter_radius(environment, ctx.system_mass(), ctx.age_at_epoch())
            })
            .map_or(
                tidal,
                |encounter| {
                    if encounter < tidal { encounter } else { tidal }
                },
            ),
    }
}

/// The semi-major axis at which a system of mass `mass` in `environment` expects one encounter
/// that removes a planet there within `age`, m, or `None` for a system no older than the epoch,
/// which has had no time to meet its neighbours.
///
/// The encounters whose closest approach `b` is within a planet's semi-major axis remove it at the
/// share [`ENCOUNTER_REMOVAL_FACTOR`] (f = 0.2). The rate of removing encounters for a system
/// among neighbours of number density `n` and mean mass `m̄`, both populations Maxwellian with
/// one-dimensional dispersion σ, is
///
/// `Γ(b) = f 4√π n σ b² (1 + G M ÷ (2 σ² b))`, M = `mass` + m̄,
///
/// the rate `n π b² (1 + 2 G M ÷ (b v²)) v` of the focused cross-section averaged over the relative
/// speeds v, whose mean is `4σ ÷ √π` (Binney and Tremaine 2008, Galactic Dynamics, 2nd ed.,
/// §7.5.7, eq. 7.195, there for two stars of equal mass m★ and radius r★ touching at b = 2r★). The
/// radius solves
/// `Γ(a) × age = 1`, a quadratic in a, `A a² + B a = 1 ÷ age` with `A = f 4√π n σ` and
/// `B = f 2√π n G M ÷ σ`, taken in the form `a = (2 ÷ age) ÷ (B + √(B² + 4A ÷ age))`, which loses no
/// digits where focusing dominates and `B²` is far above `4A ÷ age`.
///
/// The calibration is Li and Adams's (2015, MNRAS 448, 344): `b = a` is their eq. 6 at α = 1,
/// which overstates their measured removal cross-sections about fivefold
/// ([`ENCOUNTER_REMOVAL_FACTOR`]). The plan also names Adams and Laughlin (2001, Icarus 150, 151)
/// and Spurzem et al. (2009, ApJ 697, 458). None was read at source here.
///
/// In a globular core focusing dominates and the radius is about `σ ÷ (2√π f G M n age)`: some
/// 0.11 au for 10⁴ systems per cubic light-year at 10 km/s over 12 Gyr. In an open cluster of 0.3
/// per cubic light-year at 1 km/s it is some 1,400 au after 1 Gyr.
///
/// # Examples
///
/// A star in a globular cluster's core keeps far less than its galactic sphere of influence
/// would allow it:
///
/// ```
/// use hyperion_sim::planetary::context::{EncounterEnvironment, solar_neighbourhood_tidal_radius};
/// use hyperion_sim::planetary::hosts::strip::encounter_radius;
/// use hyperion_sim::planetary::params::SATELLITE_STABILITY_FRACTION;
/// use hyperion_sim::units::{
///     AstronomicalUnits, KilometresPerSecond, PerCubicLightYear, SolarMasses, Years,
/// };
///
/// let core = EncounterEnvironment::new(
///     PerCubicLightYear::new(1e4),
///     KilometresPerSecond::new(10.0),
///     SolarMasses::new(0.45),
/// )?;
/// let radius = encounter_radius(&core, SolarMasses::new(1.0), Years::new(1.2e10))
///     .ok_or("an old system has met its neighbours")?;
/// assert!(AstronomicalUnits::from(radius).value() < 1.0);
/// // A field star near the Sun keeps 0.49 of its sphere of influence, some 10⁵ au.
/// let sphere = solar_neighbourhood_tidal_radius(SolarMasses::new(1.0));
/// let field = sphere * SATELLITE_STABILITY_FRACTION;
/// assert!(AstronomicalUnits::from(field).value() > 1e4);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn encounter_radius(
    environment: &EncounterEnvironment,
    mass: SolarMasses,
    age: Years,
) -> Option<Metres> {
    let t = age.value();
    if !(t > 0.0 && t.is_finite()) {
        return None;
    }
    let n = environment.number_density().value();
    let sigma = environment.velocity_dispersion().value();
    let m = mass.value() + environment.mean_member_mass().value();
    let root_pi = core::f64::consts::PI.sqrt();
    // Per year and light-year squared (A) and per year and light-year (B): n in ly⁻³, σ in km/s,
    // turned into light-years per year.
    let f = ENCOUNTER_REMOVAL_FACTOR;
    let a2 = f * 4.0 * root_pi * n * sigma * LIGHT_YEARS_PER_YEAR_PER_KM_S;
    let a1 = f * 2.0 * root_pi * n * G * m / sigma * LIGHT_YEARS_PER_YEAR_PER_KM_S;
    let radius_ly = (2.0 / t) / (a1 + (a1 * a1 + 4.0 * a2 / t).sqrt());
    Some(Metres::new(radius_ly * METRES_PER_LIGHT_YEAR))
}

/// What a system's record gives its strip radius (module documentation): its sphere of influence
/// and, for a feature member, its encounter environment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Surroundings {
    /// The sphere of influence, m.
    pub(crate) sphere_of_influence: Metres,
    /// The encounter environment, a feature member's alone.
    pub(crate) encounter_environment: Option<EncounterEnvironment>,
}

impl Surroundings {
    /// The surroundings of `record`, a system of mass `mass` in `galaxy`, under the rules in force
    /// ([`StripRules::in_force`]); a feature member's interior from `interiors`.
    ///
    /// # Panics
    ///
    /// Under the stripping's rules, as [`stars_of`](crate::observe::stars_of) does, for a feature
    /// member's record that is not of `galaxy` (whose feature does not resolve there or has no
    /// members). Never for a record of `galaxy`: a grid record's epoch state is plan 08's, and a
    /// feature member's cluster moves at its bulk velocity, finite and below light's.
    #[must_use]
    pub(crate) fn of_record(
        galaxy: &Galaxy,
        interiors: &dyn FeatureInteriorCache,
        record: &SystemRecord,
        mass: SolarMasses,
    ) -> Self {
        let here = Self {
            sphere_of_influence: galaxy
                .potential()
                .tidal_radius(mass, &PointLy::from(record.epoch_position())),
            encounter_environment: None,
        };
        match StripRules::in_force() {
            StripRules::Held => here,
            StripRules::Stripped => match record.origin() {
                SystemOrigin::Grid(_) => Self {
                    sphere_of_influence: tidal_radius(
                        galaxy,
                        &EpochState::of_grid_record(galaxy, record),
                        mass,
                    ),
                    encounter_environment: None,
                },
                SystemOrigin::FeatureMember { .. } => {
                    let SystemIdKind::FeatureMember(member) = record.id().kind() else {
                        unreachable!("a feature member's record is built with a member ID")
                    };
                    let feature = FeatureCatalogue::resolve(galaxy, member.feature().into())
                        .expect("a feature member's feature resolves in the galaxy that placed it");
                    let interior = interiors
                        .interior(galaxy, &feature)
                        .expect("a feature that placed a member has an interior");
                    member_surroundings(galaxy, &interior, record, mass)
                }
                SystemOrigin::CatalogueSystem(_)
                | SystemOrigin::CentreMember { .. }
                | SystemOrigin::GlobalListMember { .. } => here,
            },
        }
    }
}

/// A feature member's surroundings, from its feature's `interior`: the smaller of its galactic
/// radius on its cluster's orbit and its cluster's, and its cluster's encounter environment at its
/// place, or at its core's edge from inside it (module documentation).
#[must_use]
fn member_surroundings(
    galaxy: &Galaxy,
    interior: &FeatureInterior,
    record: &SystemRecord,
    mass: SolarMasses,
) -> Surroundings {
    let galactic = tidal_radius(
        galaxy,
        &EpochState::new(*record, interior.bulk_velocity())
            .expect("a cluster's bulk velocity is finite and below light's"),
        mass,
    );
    let local = local_offset(interior.feature().position(), record.epoch_position());
    let cluster = cluster_tidal_radius(interior.model(), &local, mass);
    let r = (local.x * local.x + local.y * local.y + local.z * local.z).sqrt();
    let core = interior.model().profile_core_radius().value();
    let (place, at) = if r >= core {
        (local, r)
    } else if r > 0.0 {
        let out = core / r;
        (
            PointLy::new(local.x * out, local.y * out, local.z * out),
            core,
        )
    } else {
        (PointLy::new(core, 0.0, 0.0), core)
    };
    let table = interior.table();
    let encounter_environment = match EncounterEnvironment::new(
        PerCubicLightYear::new(table.density(&place)),
        interior.model().sigma(LightYears::new(at)),
        SolarMasses::new(table.mean_member_mass(&place)),
    ) {
        Ok(environment) => Some(environment),
        // A place no profile reaches, past every class's tidal radius and its tails, has no
        // neighbours, and so no encounter cut.
        Err(BuildEncounterEnvironmentError::NumberDensityNotPositive) => None,
        Err(
            error @ (BuildEncounterEnvironmentError::VelocityDispersionNotPositive
            | BuildEncounterEnvironmentError::MeanMassNotPositive),
        ) => panic!(
            "a cluster's dispersion is positive, and so is its members' mean mass wherever they \
             have a density: {error}"
        ),
    };
    Surroundings {
        sphere_of_influence: if cluster < galactic {
            cluster
        } else {
            galactic
        },
        encounter_environment,
    }
}

#[cfg(test)]
mod tests;
